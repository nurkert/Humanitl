//! Die zusätzliche Wurzel wirkt, und ohne sie scheitert derselbe Weg (HUM-087).
//!
//! Das Paar ist der Kern des Issues. Ein Test, der nur nachsieht, ob eine
//! Datei gelesen wurde, belegt nichts über den Verbindungsstapel: Er wäre auch
//! grün, wenn die gelesenen Wurzeln danach im Nichts verschwänden — genau der
//! Zustand, den dieses Issue behebt. Gemessen wird deshalb an einem echten
//! TLS-Server mit einem Blatt aus einer fremden CA, zweimal derselbe Aufbau
//! und nur ein Unterschied:
//!
//! - mit [`ProxyBuilder::trust`]: `200`, der Rumpf des Ziels, ein Treffer beim
//!   Ziel;
//! - ohne: `502` mit `reason: upstream_tls`, und **kein** Treffer beim Ziel.
//!
//! Der zweite Teil hält den Normalfall fest: Eine zusätzliche Wurzel ändert
//! weder das ALPN-Angebot noch die Wurzeln, die ohne sie gelten. Das Loch, das
//! `--allow-test-ca` aufmacht, soll genau so groß sein wie diese eine Wurzel.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};

use bytes::Bytes;
use humanitl_proxy::{ClientTls, roots_from_pem};
use hyper::{Request, StatusCode};

use support::{ECHO_BODY, FakeUpstream, ProxyBuilder, UpstreamCa, body_string};

/// Der Name, auf den das Blatt des Fake-Upstreams ausgestellt wird.
///
/// Ein Name und keine Adresse: Der Proxy heftet die aufgelöste Adresse an und
/// prüft das Zertifikat gegen den Namen. Aufgelöst wird er nur im Mock.
const HOST: &str = "upstream.test";

/// Ein Regelsatz, der genau diesen Host erlaubt; gehalten wird hier nichts.
fn allow_rule(host: &str) -> String {
    format!("version: 1\nrules:\n  - action: allow\n    match:\n      host: {host}\n")
}

fn loopback() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// Die eine Anfrage des Paars, durch den CONNECT-Tunnel des Proxys.
async fn fetch_echo(proxy: &support::Proxy, port: u16) -> (StatusCode, String) {
    let mut tunnel = proxy.tls_client(HOST, port).await;
    let request = Request::builder()
        .uri("/echo")
        .header("host", format!("{HOST}:{port}"))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    let status = response.status();
    (status, body_string(response.into_body()).await)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_trusted_test_root_reaches_the_target() {
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::tls_named(upstream_ca.store(), HOST).await;
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule(HOST))
        .resolve_host(HOST, vec![loopback()])
        .trust(upstream_ca.cert_der())
        .start()
        .await;

    let (status, body) = fetch_echo(&proxy, upstream.port()).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, ECHO_BODY);
    assert_eq!(upstream.hits(), 1, "the request arrived at the target");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_target_without_the_root_is_a_gateway_error() {
    // Derselbe Aufbau, dieselbe CA, dasselbe Blatt — nur ohne `trust`. Wäre
    // der Unterschied woanders, träfe dieser Test ihn und nicht die Wurzel.
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::tls_named(upstream_ca.store(), HOST).await;
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule(HOST))
        .resolve_host(HOST, vec![loopback()])
        .start()
        .await;

    let (status, body) = fetch_echo(&proxy, upstream.port()).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
    assert!(body.contains("reason: upstream_tls"), "{body}");
    assert_eq!(
        upstream.hits(),
        0,
        "a failed handshake must not deliver a request"
    );
}

#[test]
fn a_test_root_changes_nothing_but_the_root_list() {
    // Der Normalfall bleibt der Normalfall: Das ALPN-Angebot hängt allein an
    // `experimental.h2_upstream`, nicht daran, ob eine Testwurzel dazukam.
    let upstream_ca = UpstreamCa::new();
    let root = upstream_ca.cert_der();
    for h2 in [false, true] {
        let plain = ClientTls::new(&[], h2).unwrap();
        let with_root = ClientTls::new(std::slice::from_ref(&root), h2).unwrap();
        assert_eq!(plain.alpn(), with_root.alpn(), "h2 = {h2}");
    }
    assert_eq!(
        ClientTls::new(&[], false).unwrap().alpn(),
        vec![b"http/1.1".to_vec()],
        "http/1.1 only, as long as experimental.h2_upstream is off"
    );
}

#[test]
fn a_pem_without_a_certificate_yields_no_root() {
    // Der Leser, den `humanitld` benutzt. Leere Rückgabe heißt „kein
    // Zertifikat in dieser Datei"; das Binary macht daraus `CONFIG_010`.
    assert!(roots_from_pem(b"").is_empty());
    assert!(roots_from_pem(b"not a certificate\n").is_empty());
    assert!(
        roots_from_pem(b"-----BEGIN CERTIFICATE-----\nnot base64\n-----END CERTIFICATE-----\n")
            .is_empty()
    );

    let ca = UpstreamCa::new();
    let pem = std::fs::read(ca.store().cert_path()).unwrap();
    assert_eq!(roots_from_pem(&pem), vec![ca.cert_der()]);
}
