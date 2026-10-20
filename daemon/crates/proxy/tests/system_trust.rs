//! Der Daemon vertraut der Zertifikatsablage des Systems (HUM-257).
//!
//! Gemessen an einem echten TLS-Ziel mit einem Blatt aus einer eigenen CA,
//! nicht an der Zahl geladener Wurzeln: Eine Zahl wäre auch dann richtig,
//! wenn die Wurzeln danach nie im Verbindungsstapel ankämen.
//!
//! # Die Ablage per Umgebung
//!
//! `rustls-native-certs` liest statt der Ablage des Rechners die Datei aus
//! `SSL_CERT_FILE`, wenn die Variable gesetzt ist. Eine Umgebungsvariable im
//! laufenden Testprozess zu setzen ist in Rust 2024 `unsafe` und mit
//! parallelen Tests ein Wettlauf. Deshalb startet jeder Fall dieses Binary ein
//! zweites Mal, mit genau einem Test ([`child_connects`]) und der Umgebung,
//! die der Fall braucht. Das Ziel läuft im Elternprozess und zählt, ob eine
//! Anfrage ankam; das Kind verbindet, prüft das Ergebnis und endet mit seinem
//! Exit-Code.
//!
//! Die Paare:
//!
//! - CA in der Ablage: das Ziel antwortet, ein Treffer;
//! - eine andere CA in der Ablage: `Tls` mit unbekannter Stelle, kein Treffer;
//! - CA in der Ablage, aber `tls.trust_system_store = false`: abgelehnt, mit
//!   dem Vorschlag, den Schlüssel einzuschalten, kein Treffer.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use humanitl_config::ResolverConfig;
use humanitl_core::{Authority, HostName, HttpRequest, Method, Scheme, UpstreamError};
use humanitl_proxy::{
    ClientTls, Direct, Resolver, ResolverPort, TrustAnchors, TrustSource, Upstream,
};

use support::{FakeUpstream, ForeignCa, MockResolver, UpstreamCa};

/// Der Name, auf den das Blatt des Ziels ausgestellt wird.
const HOST: &str = "upstream.test";

/// Die Variablen, über die Eltern und Kind sprechen.
const ENV_PORT: &str = "HUM257_PORT";
const ENV_MODE: &str = "HUM257_MODE";
const ENV_EXPECT: &str = "HUM257_EXPECT";

fn loopback() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// Ein Weiterleiter mit diesen Ankern über den direkten Egress. Aufgelöst
/// wird nichts: `forward_to` bekommt die Adresse.
fn upstream_with(anchors: &TrustAnchors) -> Upstream {
    let config = ResolverConfig::default();
    let resolver = Arc::new(
        ResolverPort::over(
            Arc::new(MockResolver::answering(loopback())) as Arc<dyn Resolver>,
            &config,
        )
        .unwrap(),
    );
    Upstream::new(
        Arc::new(Direct::default()),
        resolver as Arc<dyn Resolver>,
        ClientTls::new(anchors, &[], false).unwrap(),
        config.prefer,
        Duration::from_secs(5),
    )
}

fn echo_request(port: u16) -> HttpRequest {
    HttpRequest::new(
        Method::GET,
        Scheme::Https,
        Authority::new(HostName::Dns(HOST.to_owned()), port),
        "/echo".to_owned(),
    )
}

/// Startet [`child_connects`] mit `store` als `SSL_CERT_FILE`.
///
/// `SSL_CERT_DIR` wird entfernt, damit nur die eine Datei die Ablage ist und
/// nicht zusätzlich ein Verzeichnis aus der Umgebung des Testlaufs.
fn run_child(store: &Path, port: u16, mode: &str, expect: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_connects", "--ignored", "--nocapture"])
        .env("SSL_CERT_FILE", store)
        .env_remove("SSL_CERT_DIR")
        .env(ENV_PORT, port.to_string())
        .env(ENV_MODE, mode)
        .env(ENV_EXPECT, expect)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "child ({mode}, expecting {expect}) failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the child must have run exactly the one test:\n{stdout}"
    );
}

/// Die Seite des Kindprozesses. Läuft nur, wenn ein Fall dieser Datei sie
/// startet; allein aufgerufen fehlt ihr die Umgebung, und sie scheitert laut.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "started by the other tests of this file, with SSL_CERT_FILE set"]
async fn child_connects() {
    let port: u16 = std::env::var(ENV_PORT)
        .expect("started without HUM257_PORT")
        .parse()
        .unwrap();
    let mode = std::env::var(ENV_MODE).unwrap();
    let expect = std::env::var(ENV_EXPECT).unwrap();

    // Der Weg, den der Daemon nimmt: `configured` mit dem Wert des Schlüssels.
    let anchors = TrustAnchors::configured(mode == "system");
    let result = upstream_with(&anchors)
        .forward_to(&echo_request(port), Bytes::new(), loopback())
        .await;

    match expect.as_str() {
        "ok" => {
            assert_eq!(anchors.source(), TrustSource::SystemStore);
            assert!(
                anchors
                    .redirected()
                    .iter()
                    .any(|entry| entry.starts_with("SSL_CERT_FILE=")),
                "the start line has to say that the environment redirected the store: {:?}",
                anchors.redirected()
            );
            let response = result.expect("the CA from SSL_CERT_FILE must be trusted");
            assert_eq!(response.status(), 200);
        }
        "untrusted" => {
            let failure = result.expect_err("a CA outside the anchors must be refused");
            assert_eq!(failure.error, UpstreamError::Tls);
            let expected = if mode == "system" {
                TrustSource::SystemStore
            } else {
                TrustSource::BundledByConfig
            };
            assert_eq!(failure.untrusted_issuer, Some(expected));
        }
        other => panic!("unknown expectation {other}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ca_in_the_system_store_reaches_the_target() {
    let ca = UpstreamCa::new();
    let target = FakeUpstream::tls_named(ca.store(), HOST).await;

    run_child(&ca.store().cert_path(), target.port(), "system", "ok");

    assert_eq!(target.hits(), 1, "the request arrived at the target");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_store_without_the_ca_refuses_the_target() {
    // Dasselbe Ziel, eine Ablage mit einer anderen CA. Wäre der Unterschied
    // woanders als in der Ablage, träfe dieser Test ihn nicht.
    let ca = UpstreamCa::new();
    let target = FakeUpstream::tls_named(ca.store(), HOST).await;
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other.pem");
    std::fs::write(&other, ForeignCa::new().pem()).unwrap();

    run_child(&other, target.port(), "system", "untrusted");

    assert_eq!(target.hits(), 0, "a failed handshake delivers nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_opt_out_ignores_the_system_store() {
    // Die CA steht in der Ablage wie im ersten Fall; nur der Schlüssel ist
    // aus. Dann gelten allein die öffentlichen Wurzeln.
    let ca = UpstreamCa::new();
    let target = FakeUpstream::tls_named(ca.store(), HOST).await;

    run_child(
        &ca.store().cert_path(),
        target.port(),
        "opt_out",
        "untrusted",
    );

    assert_eq!(target.hits(), 0, "the opt-out must not reach the target");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_broken_entry_in_the_store_costs_nothing_but_itself() {
    // Eine Ablage mit drei Einträgen: die echte CA, ein PEM-Block mit
    // gültigem Base64, aber ohne Zertifikat darin, und ein Block, der gar
    // kein Base64 ist. rustls lehnt den zweiten ab, der Leser den dritten.
    // Der Stapel entsteht trotzdem, und die echte CA wirkt.
    let ca = UpstreamCa::new();
    let target = FakeUpstream::tls_named(ca.store(), HOST).await;
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store.pem");
    let mut pem = std::fs::read(ca.store().cert_path()).unwrap();
    pem.extend_from_slice(
        b"-----BEGIN CERTIFICATE-----\nMAMCAQA=\n-----END CERTIFICATE-----\n\
          -----BEGIN CERTIFICATE-----\n!!! not base64 !!!\n-----END CERTIFICATE-----\n",
    );
    std::fs::write(&store, pem).unwrap();

    let anchors = TrustAnchors::from_paths(Some(&store), None);

    assert_eq!(anchors.source(), TrustSource::SystemStore);
    assert_eq!(anchors.len(), 1, "only the real CA is an anchor");
    assert!(
        anchors.rejected() >= 1,
        "the broken entries are counted, not silently dropped"
    );
    let response = upstream_with(&anchors)
        .forward_to(&echo_request(target.port()), Bytes::new(), loopback())
        .await
        .expect("the real CA still works");
    assert_eq!(response.status(), 200);
    assert_eq!(target.hits(), 1);
}
