//! DNS erst nach der Freigabe (HUM-024, ADR-006).
//!
//! Der Mock-Resolver des Harness schreibt jeden Namen mit, den er gefragt
//! wird. Ein leeres [`MockResolver::hosts`] nach einer Antwort ist deshalb
//! kein „wahrscheinlich nichts gefragt", sondern der Beleg: Der einzige Weg
//! des Proxys zum Namensdienst führt über diesen Mock, und er hat nichts
//! gesehen. Ein Test, der einen Namen dort findet, nennt genau den Namen, der
//! geleakt wäre.
//!
//! Die Datei prüft drei Dinge:
//!
//! 1. Kein Weg des Proxys löst vor einer Entscheidung auf: geblockt, gehalten,
//!    abgelaufen, wegen widersprüchlicher Authority abgelehnt, über dem
//!    Body-Cap, über dem Halte-Budget, und der reine CONNECT-Tunnel.
//! 2. Nach `Allow` wird genau einmal aufgelöst, die Adresse wird angeheftet,
//!    und das Zertifikat wird gegen den Namen geprüft, nicht gegen die
//!    Adresse.
//! 3. Eine Antwort, die in ein privates Netz zeigt, wird ohne
//!    TCP-Verbindung abgelehnt (Rebinding).
//!
//! Dazu kommt eine Prüfung am Quelltext: Außer dem Resolver-Modul fasst
//! niemand im Proxy einen Namensdienst an.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use bytes::Bytes;
use humanitl_config::ResolverConfig;
use humanitl_core::{BlockReason, Decision, FlowEvent};
use hyper::{Request, StatusCode};

use support::{ECHO_BODY, FakeUpstream, ProxyBuilder, body_string, get, post};

/// Ein Regelsatz, der genau diesen Host blockt.
fn block_rule(host: &str) -> String {
    format!("version: 1\nrules:\n  - action: block\n    match:\n      host: {host}\n")
}

/// Ein Regelsatz, der genau diesen Host erlaubt.
fn allow_rule(host: &str) -> String {
    format!("version: 1\nrules:\n  - action: allow\n    match:\n      host: {host}\n")
}

fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

// ---------------------------------------------------------------------------
// 1. Nichts wird vor der Entscheidung aufgelöst
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn blocked_flow_never_resolves() {
    let proxy = ProxyBuilder::new()
        .rules(&block_rule("evil.example"))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://evil.example/steal")).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: rule"), "{body}");

    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "a blocked host must never reach the name service; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.egress.connects(), 0);
    assert_eq!(proxy.port.stats().lookups, 0);
    assert_eq!(proxy.port.stats().answers(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn timeout_never_resolves() {
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://slow.example/steal")).await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);

    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "a flow that nobody decided must not resolve; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.port.stats().lookups, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_flow_resolves_nothing_while_it_waits() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let request = get(&format!("http://held.example:{}/echo", upstream.port()));
    let send = tokio::spawn(async move { client.send(request).await });

    let FlowEvent::Held { flow_id, .. } = events.wait_for("held").await else {
        unreachable!()
    };
    // Der Flow liegt in der Warteschlange, der Mensch schaut ihn an: Bis hier
    // darf kein Byte den Rechner verlassen haben.
    assert!(
        proxy.resolver.hosts().is_empty(),
        "asked while held: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.egress.connects(), 0);

    proxy
        .queue
        .decide(
            flow_id,
            Decision::Block {
                reason: BlockReason::User,
                note: None,
            },
        )
        .unwrap();
    let response = send.await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "a flow blocked by a human must not resolve either; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(upstream.hits(), 0);
}

/// Die Wege, die an der Warteschlange vorbeigehen: Sie enden alle vor dem
/// Resolver.
///
/// Dieser Test ist die Umkehrung des Versprechens. Käme irgendwo im Proxy eine
/// Auflösung vor der Entscheidung dazu — im Handler, in der Authority-Prüfung,
/// im Aufräumen eines abgelehnten Flows —, würde hier ein Name auftauchen.
#[tokio::test(flavor = "multi_thread")]
async fn no_other_path_resolves_before_a_decision() {
    // Widersprüchliche Authority: `Host` gegen die Zeile der Anfrage.
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let request = Request::builder()
        .uri("http://real.example/echo")
        .header("host", "evil.example")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = client.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "the authority check must not resolve; asked: {:?}",
        proxy.resolver.hosts()
    );

    // Body über dem Cap: abgelehnt, bevor irgendjemand fragt.
    let proxy = ProxyBuilder::new().body_cap(1024).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);
    let mut client = proxy.client().await;
    let response = client
        .send(post("http://big.example/sink", vec![b'x'; 2048]))
        .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "a body over the cap must not resolve; asked: {:?}",
        proxy.resolver.hosts()
    );

    // Halte-Budget erschöpft: 503, ohne Frage an den Namensdienst.
    let proxy = ProxyBuilder::new().hold_max_flows(0).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);
    let mut client = proxy.client().await;
    let response = client.send(get("http://budget.example/echo")).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "an exhausted budget must not resolve; asked: {:?}",
        proxy.resolver.hosts()
    );

    // Ein reiner CONNECT-Tunnel ohne Anfrage darin: Das CONNECT-Ziel allein
    // ist keine Entscheidung und darf nichts auflösen.
    let proxy = ProxyBuilder::new().start().await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;
    let tunnel = proxy.tls_client("localhost", upstream.port()).await;
    drop(tunnel);
    assert!(
        proxy.resolver.hosts().is_empty(),
        "a bare CONNECT must not resolve; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(upstream.hits(), 0);
}

// ---------------------------------------------------------------------------
// 2. Nach Allow: genau einmal, angeheftet, gegen den Namen geprüft
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn allow_resolves_once() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("api.github.test"))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!(
            "http://api.github.test:{}/echo",
            upstream.port()
        )))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_string(response.into_body()).await, ECHO_BODY);

    events.wait_for("recorded").await;
    assert_eq!(proxy.resolver.hosts(), vec!["api.github.test".to_owned()]);
    assert_eq!(proxy.egress.connects(), 1);
    let stats = proxy.port.stats();
    assert_eq!(stats.lookups, 1);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.failures, 0);
    assert_eq!(stats.answers(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn ip_literal_no_resolve() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules("version: 1\nrules:\n  - action: allow\n    match:\n      host: \"ip:127.0.0.1\"\n")
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "an address needs no name service"
    );
    assert_eq!(proxy.port.stats().answers(), 0);
    assert_eq!(proxy.egress.connects(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn pinned_addr_used() {
    // Der Upstream lauscht auf 127.0.0.1, sein Zertifikat gilt für
    // `pinned.test`. Der Name ist nirgends auflösbar außer im Mock: Wenn die
    // Antwort ankommt, hat der Proxy genau die angeheftete Adresse benutzt und
    // das Zertifikat gegen den Namen geprüft.
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("pinned.test"))
        .resolve_host("pinned.test", vec![v4(127, 0, 0, 1)])
        .start()
        .await;
    let upstream = FakeUpstream::tls_named(&proxy.ca, "pinned.test").await;
    let mut events = proxy.events();

    let mut tunnel = proxy.tls_client("pinned.test", upstream.port()).await;
    let request = Request::builder()
        .uri("/echo")
        .header("host", format!("pinned.test:{}", upstream.port()))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
    assert_eq!(upstream.hits(), 1);

    events.wait_for("recorded").await;
    assert_eq!(proxy.resolver.hosts(), vec!["pinned.test".to_owned()]);
    assert_eq!(proxy.egress.connects(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_certificate_for_the_wrong_name_fails_instead_of_passing() {
    // Dieselbe Anordnung, nur trägt das Zertifikat des Upstreams den falschen
    // Namen. Der Proxy prüft gegen `pinned.test` und muss scheitern; ein
    // Erfolg hieße, er prüft gegen die Adresse oder gar nicht.
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("pinned.test"))
        .resolve_host("pinned.test", vec![v4(127, 0, 0, 1)])
        .start()
        .await;
    let upstream = FakeUpstream::tls_named(&proxy.ca, "someone.else.test").await;

    let mut tunnel = proxy.tls_client("pinned.test", upstream.port()).await;
    let request = Request::builder()
        .uri("/echo")
        .header("host", format!("pinned.test:{}", upstream.port()))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: upstream_tls"), "{body}");
    assert_eq!(upstream.hits(), 0);
}

// ---------------------------------------------------------------------------
// 3. Rebinding
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn rebinding_to_loopback_rejected() {
    // Ein Fake-Upstream auf Loopback zählt mit: Er darf keine einzige
    // Verbindung sehen.
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("evil.example"))
        .allow_private(false)
        .resolve_host("evil.example", vec![v4(127, 0, 0, 1)])
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!(
            "http://evil.example:{}/echo",
            upstream.port()
        )))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: upstream_private_address"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(proxy.resolver.hosts(), vec!["evil.example".to_owned()]);
    assert_eq!(
        proxy.egress.connects(),
        0,
        "a private address is never connected"
    );
    assert_eq!(upstream.hits(), 0, "the loopback upstream saw nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_mixed_answer_with_one_private_address_is_refused_too() {
    // Die Präferenz hätte die öffentliche Adresse gewählt. Genau darauf darf
    // sich ein Angreifer nicht verlassen können.
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("mixed.example"))
        .allow_private(false)
        .resolve_host(
            "mixed.example",
            vec![v4(93, 184, 216, 34), v4(169, 254, 169, 254)],
        )
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://mixed.example/echo")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: upstream_private_address"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(proxy.egress.connects(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_name_that_does_not_resolve_ends_as_upstream_dns() {
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("nowhere.example"))
        .resolve_fails("nowhere.example")
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://nowhere.example/echo")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: upstream_dns"), "{body}");

    events.wait_for("recorded").await;
    // Genau ein Versuch: kein Rückfall auf einen zweiten Resolver.
    assert_eq!(proxy.resolver.hosts(), vec!["nowhere.example".to_owned()]);
    let stats = proxy.port.stats();
    assert_eq!(stats.lookups, 1);
    assert_eq!(stats.failures, 1);
    assert_eq!(proxy.egress.connects(), 0);
}

// ---------------------------------------------------------------------------
// Zwischenspeicher und Zähler
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_second_request_is_answered_from_the_cache_and_counted_as_such() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("cached.example"))
        .resolver_config(ResolverConfig {
            cache_ttl_secs: 60,
            ..ResolverConfig::default()
        })
        .start()
        .await;
    let mut events = proxy.events();

    let url = format!("http://cached.example:{}/echo", upstream.port());
    for _ in 0..3 {
        let mut client = proxy.client().await;
        let response = client.send(get(&url)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
    }

    events.wait_for_nth("recorded", 3).await;
    assert_eq!(
        proxy.resolver.hosts(),
        vec!["cached.example".to_owned()],
        "the name service is asked once, not three times"
    );
    let stats = proxy.port.stats();
    assert_eq!(stats.lookups, 1, "only one query left the machine");
    assert_eq!(stats.cache_hits, 2, "a cache hit is not a lookup");
    assert_eq!(stats.answers(), 3);
    // Der Zwischenspeicher spart die Abfrage, nicht die Verbindung: Jede
    // Anfrage hat ihre eigene Verbindung zur angehefteten Adresse.
    assert_eq!(proxy.egress.connects(), 3);
    assert_eq!(upstream.hits(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cached_address_still_faces_the_private_check() {
    // Der Zwischenspeicher hält Adressen, keine Entscheidungen: Auch ein
    // Treffer läuft durch dieselbe Prüfung und wird abgelehnt.
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("evil.example"))
        .allow_private(false)
        .resolve_host("evil.example", vec![v4(10, 0, 0, 1)])
        .resolver_config(ResolverConfig {
            cache_ttl_secs: 60,
            ..ResolverConfig::default()
        })
        .start()
        .await;
    let mut events = proxy.events();

    for _ in 0..2 {
        let mut client = proxy.client().await;
        let response = client.send(get("http://evil.example/echo")).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = body_string(response.into_body()).await;
        assert!(body.contains("reason: upstream_private_address"), "{body}");
    }

    events.wait_for_nth("recorded", 2).await;
    assert_eq!(proxy.egress.connects(), 0, "not once, not the second time");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_override_answers_without_a_lookup() {
    let upstream = FakeUpstream::plain().await;
    let mut overrides = std::collections::BTreeMap::new();
    overrides.insert("fixed.example".to_owned(), "127.0.0.1".to_owned());
    let proxy = ProxyBuilder::new()
        .rules(&allow_rule("fixed.example"))
        .resolver_config(ResolverConfig {
            overrides,
            ..ResolverConfig::default()
        })
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!(
            "http://fixed.example:{}/echo",
            upstream.port()
        )))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    events.wait_for("recorded").await;
    assert!(
        proxy.resolver.hosts().is_empty(),
        "an override never asks the name service"
    );
    let stats = proxy.port.stats();
    assert_eq!(stats.overrides, 1);
    assert_eq!(stats.lookups, 0);
}

// ---------------------------------------------------------------------------
// Der Quelltext selbst
// ---------------------------------------------------------------------------

/// Das Akzeptanzkriterium aus HUM-024 als Test, nicht als Zeile in einem
/// Dokument.
///
/// `grep -rn "lookup_host\|getaddrinfo\|to_socket_addrs"` über
/// `daemon/crates/proxy/src` darf nur `resolver.rs` treffen. Wer einen
/// Namensdienst an einer anderen Stelle aufmacht — im Connector, im Handler,
/// beim Aufräumen —, fällt hier auf, auch wenn kein Laufzeittest ihn trifft.
#[test]
fn only_the_resolver_module_touches_the_name_service() {
    const NEEDLES: [&str; 3] = ["lookup_host", "getaddrinfo", "to_socket_addrs"];
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for (path, text) in rust_sources(&src) {
        if path.file_name().is_some_and(|name| name == "resolver.rs") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            if NEEDLES.iter().any(|needle| line.contains(needle)) {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "only resolver.rs may reach a name service:\n{}",
        offenders.join("\n")
    );
}

/// Alle `.rs`-Dateien unter `dir`, mit ihrem Inhalt.
fn rust_sources(dir: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("the source directory is readable") {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).expect("a readable source file");
                found.push((path, text));
            }
        }
    }
    assert!(
        !found.is_empty(),
        "no sources found under {}",
        dir.display()
    );
    found
}
