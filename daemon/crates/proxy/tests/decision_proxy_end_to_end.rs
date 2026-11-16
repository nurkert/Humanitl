//! Ende zu Ende: Agent, Humanitl und ein Entscheidungs-Proxy, der sich wie
//! mitmproxy benimmt (ADR-0022, HUM-513).
//!
//! Der Agent spricht HTTPS mit Humanitl (`CONNECT`, TLS gegen die
//! Humanitl-CA). Humanitl steht im Frage-Modus `upstream` und schickt jede
//! Anfrage ohne Regel über `egress.proxy` an den Stellvertreter aus
//! [`support::stand_in`], gestartet mit
//! [`StandIn::start_intercepting`]: Er nimmt `CONNECT` an (mit
//! `egress.proxy_resolve = "humanitl"` zu einer Adresse), stellt sein Blatt
//! auf den Namen aus SNI aus, entscheidet die Anfrage im Tunnel wie ein
//! Mensch in mitmproxy und leitet eine weitergelassene Anfrage per TLS an
//! das Ziel weiter. Humanitl vertraut seiner CA nur über `egress.proxy_ca`.
//!
//! Abgedeckt:
//!
//! - `"humanitl"`: `CONNECT IP:PORT`, SNI mit dem Namen, Humanitl löst
//!   genau einmal auf; die Antwort des Ziels erreicht den Agenten.
//! - `"proxy"`: `CONNECT host:port`, Humanitl löst nichts auf.
//! - Weiter (`200` vom Ziel), Ablehnung (`403` des Proxys, das Ziel sieht
//!   nichts), Kappen (`502` von Humanitl).
//! - Eine Regel `block` erreicht den Stellvertreter nie.
//!
//! Mutationsbeweise (von Hand, je eine Zeile, danach zurückgesetzt):
//!
//! - `daemon/crates/proxy/src/upstream.rs`, `ClientTls::new`: `config.enable_sni
//!   = false`. Der Stellvertreter stellt sein Blatt dann auf die Adresse aus
//!   der `CONNECT`-Zeile aus, Humanitl prüft gegen den Namen, der Agent
//!   bekommt `502`: alle Tests, in denen der Stellvertreter eine Anfrage im
//!   Tunnel sieht, werden rot.
//! - `daemon/crates/proxy/src/egress/http_proxy.rs`: `all_roots.extend(roots)`
//!   gestrichen, die CA aus `egress.proxy_ca` also nicht benutzt: das Blatt des
//!   Stellvertreters ist unbekannt, alle Tests mit Weiterleitung werden rot.
//! - `egress/http_proxy.rs`, `target_text`: mit `"humanitl"` der Name statt
//!   der Adresse in der `CONNECT`-Zeile:
//!   `humanitl_resolve_connects_to_the_address_and_names_the_host_in_sni`
//!   wird rot.
//! - `daemon/crates/proxy/src/pipeline.rs`: `Matched { Block }` an die innere
//!   Strategie statt zu sperren: `a_block_rule_never_reaches_the_stand_in`
//!   wird rot.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;

use humanitl_config::{AskMode, ProxyResolve};
use humanitl_core::{Decision, DecisionSource, FlowEvent};
use humanitl_proxy::egress::from_config;
use hyper::StatusCode;
use support::stand_in::{CaFile, HOST, Intercept, PUBLIC, StandIn, Verdict, ca_file, config};
use support::{ECHO_BODY, Events, FakeUpstream, ForeignCa, Proxy, ProxyBuilder, body_string, get};

/// Eine Regel `block` auf eigenem Namen; alles andere trifft keine Regel
/// und geht im Modus `upstream` an den Stellvertreter.
const RULES: &str = "version: 1\n\
     rules:\n\
     \x20 - action: block\n\
     \x20   match:\n\
     \x20     host: blocked.test\n";

/// Was der Mensch am Stellvertreter tut, nach dem Pfad: `/refuse` lehnt er
/// mit `403` ab (in mitmproxy `block_list`), `/kill` kappt er, alles
/// andere lässt er weiter.
fn by_path(line: &str) -> Verdict {
    if line.contains(" /refuse ") {
        Verdict::Refuse(403)
    } else if line.contains(" /kill ") {
        Verdict::Kill
    } else {
        Verdict::Resume
    }
}

/// Ziel, Stellvertreter und Humanitl davor, im Modus `upstream`.
struct Setup {
    target: FakeUpstream,
    stand_in: StandIn,
    proxy: Proxy,
    _ca: CaFile,
}

async fn setup(resolve: ProxyResolve) -> Setup {
    // Das Ziel mit einem Blatt für `HOST` aus seiner eigenen CA, der nur der
    // Stellvertreter vertraut; Humanitl kennt sie nicht.
    let target_ca = ForeignCa::new();
    let target = FakeUpstream::tls_foreign(&target_ca, HOST).await;
    let proxy_ca = Arc::new(ForeignCa::new());
    let file = ca_file(&proxy_ca);
    let stand_in = StandIn::start_intercepting(
        Arc::clone(&proxy_ca),
        Intercept::reaching(target.addr, &target_ca, by_path),
    )
    .await;
    let config = config(stand_in.addr, &file.path, resolve);
    let egress = from_config(&config, &[], None).unwrap().egress;
    let proxy = ProxyBuilder::new()
        .ask(std::time::Duration::from_secs(2))
        .ask_mode(AskMode::Upstream)
        .rules(RULES)
        .allow_private(false)
        .resolve_host(HOST, vec![PUBLIC])
        .egress(egress)
        .start()
        .await;
    Setup {
        target,
        stand_in,
        proxy,
        _ca: file,
    }
}

/// Der Agent: `CONNECT` an Humanitl, TLS gegen die Humanitl-CA, dann
/// `GET path`. Status und Rumpf der Antwort.
async fn agent_get(proxy: &Proxy, host: &str, path: &str) -> (StatusCode, String) {
    let mut tls = proxy.tls_client(host, 443).await;
    let response = tls.client.send(get(&format!("https://{host}{path}"))).await;
    let status = response.status();
    (status, body_string(response.into_body()).await)
}

fn decisions(events: &Events) -> Vec<(Decision, DecisionSource)> {
    events
        .seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Decided {
                decision, source, ..
            } => Some((decision.clone(), *source)),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn humanitl_resolve_connects_to_the_address_and_names_the_host_in_sni() {
    let setup = setup(ProxyResolve::Humanitl).await;
    let mut events = setup.proxy.events();

    let (status, body) = agent_get(&setup.proxy, HOST, "/echo").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body, ECHO_BODY, "the target's answer, relayed twice");
    events.wait_for("response_headers").await;

    let seen = setup.stand_in.seen();
    assert_eq!(seen.lines, ["CONNECT 93.184.216.34:443 HTTP/1.1"]);
    assert_eq!(seen.sni, [HOST], "the name travels only in SNI");
    assert_eq!(seen.inner_lines, ["GET /echo HTTP/1.1"]);
    assert_eq!(seen.inner_hosts, [HOST]);
    assert_eq!(
        seen.dialed,
        ["93.184.216.34:443"],
        "dialed the CONNECT target"
    );
    assert_eq!(setup.target.hits(), 1);
    assert_eq!(
        setup.proxy.resolver.hosts(),
        [HOST],
        "Humanitl resolved exactly once"
    );
    assert_eq!(events.count("held"), 0, "nothing waits in Humanitl");
    assert_eq!(
        decisions(&events),
        [(Decision::Allow, DecisionSource::Upstream)]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn proxy_resolve_connects_to_the_name_and_humanitl_resolves_nothing() {
    let setup = setup(ProxyResolve::Proxy).await;

    let (status, body) = agent_get(&setup.proxy, HOST, "/echo").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body, ECHO_BODY);

    let seen = setup.stand_in.seen();
    assert_eq!(seen.lines, ["CONNECT upstream.test:443 HTTP/1.1"]);
    assert_eq!(seen.sni, [HOST]);
    assert_eq!(seen.dialed, ["upstream.test:443"]);
    assert_eq!(setup.target.hits(), 1);
    assert!(
        setup.proxy.resolver.hosts().is_empty(),
        "no DNS question from Humanitl: {:?}",
        setup.proxy.resolver.hosts()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_at_the_stand_in_reaches_the_agent_as_403_and_the_target_sees_nothing() {
    for resolve in [ProxyResolve::Humanitl, ProxyResolve::Proxy] {
        let setup = setup(resolve).await;
        let mut events = setup.proxy.events();

        let (status, body) = agent_get(&setup.proxy, HOST, "/refuse").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{resolve:?}");
        assert_eq!(body, "refused", "the stand-in's own answer, unchanged");
        events.wait_for("response_headers").await;

        assert_eq!(setup.stand_in.seen().inner_lines, ["GET /refuse HTTP/1.1"]);
        assert!(setup.stand_in.seen().dialed.is_empty());
        assert_eq!(setup.target.hits(), 0, "{resolve:?}");
        assert_eq!(events.statuses(), [403]);
        assert_eq!(
            decisions(&events),
            [(Decision::Allow, DecisionSource::Upstream)],
            "Humanitl forwarded; the refusal is the proxy's"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_killed_request_reaches_the_agent_as_502() {
    let setup = setup(ProxyResolve::Humanitl).await;

    let (status, _body) = agent_get(&setup.proxy, HOST, "/kill").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(setup.stand_in.seen().inner_lines, ["GET /kill HTTP/1.1"]);
    assert_eq!(setup.target.hits(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_block_rule_never_reaches_the_stand_in() {
    for resolve in [ProxyResolve::Humanitl, ProxyResolve::Proxy] {
        let setup = setup(resolve).await;

        let (status, _body) = agent_get(&setup.proxy, "blocked.test", "/echo").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{resolve:?}");

        let seen = setup.stand_in.seen();
        assert!(seen.lines.is_empty(), "{resolve:?}: {:?}", seen.lines);
        assert_eq!(setup.target.hits(), 0);
        assert!(
            setup.proxy.resolver.hosts().is_empty(),
            "a blocked name is never resolved: {:?}",
            setup.proxy.resolver.hosts()
        );
    }
}
