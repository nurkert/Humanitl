//! Die Weiche zum Meta-Endpunkt `humanitl.internal` (HUM-073, ADR-014).
//!
//! Die Einheitentests des Endpunkts stehen in `src/meta.rs`; hier geht es um
//! die eine Zeile im Handler, die entscheidet, ob eine Anfrage überhaupt
//! dorthin kommt. Sie ist die heikelste Stelle des Issues, und sie hat zwei
//! Seiten:
//!
//! 1. Der reservierte Name wird selbst beantwortet — auf beiden Wegen, im
//!    Klartext und durch einen `CONNECT`-Tunnel —, und zwar **vor** jeder
//!    Regelauswertung und **vor** jeder Namensauflösung. Der zählende
//!    Mock-Resolver des Harness ist der Zeuge dafür: Ein leeres
//!    [`MockResolver::hosts`] ist kein „wahrscheinlich nichts gefragt",
//!    sondern der Beleg.
//! 2. Ein Name, der nur so *aussieht*, wird nicht beantwortet, sondern läuft
//!    durch die Regeln wie jeder andere Host.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::time::Duration;

use bytes::Bytes;
use humanitl_core::FlowEvent;
use hyper::{Request, StatusCode};

use support::{FakeUpstream, ProxyBuilder, body_string, get, header, post};

/// Ein Regelsatz, der jeden `*.internal`-Namen erlaubt.
///
/// Er trifft `humanitl.internal` genauso wie `evil-humanitl.internal`. Genau
/// darum steht er hier: Was trotzdem nicht hinausgeht, geht deshalb nicht
/// hinaus, weil die Weiche vor der Regel liegt — und nicht, weil keine Regel
/// gepasst hätte.
const ALLOW_INTERNAL: &str =
    "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"**.internal\"\n";

/// Derselbe Satz mit `block` statt `allow`.
const BLOCK_INTERNAL: &str =
    "version: 1\nrules:\n  - action: block\n    match:\n      host: \"**.internal\"\n";

// ---------------------------------------------------------------------------
// Der reservierte Name wird selbst beantwortet
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn status_over_plain_http() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal/")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header(&response, "content-type"),
        Some("text/plain; charset=utf-8")
    );
    let body = body_string(response.into_body()).await;
    assert!(
        body.starts_with(&format!("humanitl session={}", proxy.session)),
        "{body}"
    );
    assert!(body.contains("rules (first match wins):"), "{body}");

    assert!(
        proxy.resolver.hosts().is_empty(),
        "the reserved name is never resolved; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.port.stats().lookups, 0);
    assert_eq!(proxy.egress.connects(), 0);
    // Kein Flow: Die Anfrage geht nirgendwo hin, und niemand entscheidet über
    // sie. Ein `Received` hier hieße, dass sie in der Warteschlange steht.
    events.drain();
    assert_eq!(events.names(), Vec::<&str>::new(), "no flow was started");
}

#[tokio::test(flavor = "multi_thread")]
async fn status_over_connect() {
    // Derselbe Weg, den ein Agent mit `https://humanitl.internal/` nimmt:
    // `CONNECT humanitl.internal:443`, TLS gegen ein Leaf aus der eigenen CA,
    // und darin die gewöhnliche Anfrage.
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    let mut tunnel = proxy.tls_client("humanitl.internal", 443).await;
    let request = Request::builder()
        .uri("/")
        .header("host", "humanitl.internal")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(
        body.starts_with(&format!("humanitl session={}", proxy.session)),
        "{body}"
    );
    assert!(
        proxy.resolver.hosts().is_empty(),
        "not even the tunnel resolves the reserved name; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.port.stats().lookups, 0);
    assert_eq!(proxy.egress.connects(), 0);
    events.drain();
    assert_eq!(events.names(), Vec::<&str>::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_switch_lies_before_the_rules() {
    // Eine Regel, die `**.internal` erlaubt, ist die schärfste Probe: Käme die
    // Anfrage bei ihr an, würde der Proxy auflösen und verbinden. Er tut
    // beides nicht, weil die Weiche vorher liegt.
    let proxy = ProxyBuilder::new().rules(ALLOW_INTERNAL).start().await;
    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal/")).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.starts_with("humanitl session="), "{body}");
    assert!(
        proxy.resolver.hosts().is_empty(),
        "an allow rule must not make the reserved name resolvable; asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.egress.connects(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_block_rule_cannot_take_the_meta_host_away() {
    // Die Gegenprobe: Auch eine Regel, die `**.internal` blockt, entscheidet
    // hier nichts. Der Agent behält seinen einen Kanal zum Menschen, ganz
    // gleich, was in `rules.yaml` steht.
    let proxy = ProxyBuilder::new().rules(BLOCK_INTERNAL).start().await;
    let mut client = proxy.client().await;
    let response = client
        .send(post("http://humanitl.internal/ask", "bitte"))
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test(flavor = "multi_thread")]
async fn any_port_of_the_reserved_name_is_the_meta_host() {
    // Reserviert ist der Name, nicht ein Dienst auf einem Port (ADR-014).
    // Ginge ein anderer Port durch die Regeln, könnte eine Freigabe dafür
    // einen Lookup auslösen — genau das, was der ADR ausschließt.
    let proxy = ProxyBuilder::new().rules(ALLOW_INTERNAL).start().await;
    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal:8080/")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        proxy.resolver.hosts().is_empty(),
        "asked: {:?}",
        proxy.resolver.hosts()
    );
    assert_eq!(proxy.egress.connects(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_name_is_the_same_name_however_it_is_written() {
    // `HostName::parse` normalisiert Groß-/Kleinschreibung und den Punkt am
    // Ende weg, bevor irgendetwas verglichen wird. Beides ist derselbe Name;
    // ihn hier anders zu behandeln, hieße, dass eine Freigabe für
    // `humanitl.internal.` einen Lookup auslösen könnte.
    let proxy = ProxyBuilder::new().rules(ALLOW_INTERNAL).start().await;
    for host in ["HUMANITL.INTERNAL", "humanitl.internal."] {
        let mut client = proxy.client().await;
        let response = client.send(get(&format!("http://{host}/"))).await;
        assert_eq!(response.status(), StatusCode::OK, "{host}");
        let body = body_string(response.into_body()).await;
        assert!(body.starts_with("humanitl session="), "{host}: {body}");
    }
    assert!(
        proxy.resolver.hosts().is_empty(),
        "asked: {:?}",
        proxy.resolver.hosts()
    );
}

// ---------------------------------------------------------------------------
// Was nur so aussieht, geht den gewöhnlichen Weg
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn look_alike_hosts_go_through_the_rules() {
    let proxy = ProxyBuilder::new().rules(BLOCK_INTERNAL).start().await;
    let mut events = proxy.events();

    for host in [
        "evil-humanitl.internal",
        "humanitl.internal.evil.internal",
        "sub.humanitl.internal",
        "humanitl-internal.internal",
    ] {
        let mut client = proxy.client().await;
        let response = client.send(get(&format!("http://{host}/"))).await;
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{host} only looks like the reserved name"
        );
        let body = body_string(response.into_body()).await;
        assert!(body.contains("reason: rule"), "{host}: {body}");
        events.wait_for("recorded").await;
    }
    // Vier gewöhnliche Flows, keine Meta-Antwort.
    assert_eq!(events.count("received"), 4);
    assert_eq!(
        proxy.egress.connects(),
        0,
        "the rule blocked them, so nothing connected"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_look_alike_that_is_allowed_really_goes_out() {
    // Die stärkere Hälfte derselben Aussage: Ein ähnlicher Name ist ein
    // gewöhnlicher Host. Er wird aufgelöst, verbunden und beantwortet — die
    // Weiche fasst ihn nicht an.
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().rules(ALLOW_INTERNAL).start().await;
    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!(
            "http://evil-humanitl.internal:{}/echo",
            upstream.port()
        )))
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        proxy.resolver.hosts(),
        vec!["evil-humanitl.internal".to_owned()]
    );
    assert_eq!(proxy.egress.connects(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connect_to_another_host_cannot_borrow_the_meta_host() {
    // Domain-Fronting gegen die Weiche: Der Tunnel führt nach `github.test`,
    // die Anfrage darin nennt `humanitl.internal`. Das ist ein Widerspruch,
    // und `check_authority` lehnt ihn ab, bevor die Weiche überhaupt
    // gefragt wird. Andernfalls wäre die Weiche über den `Host`-Kopf
    // steuerbar.
    let proxy = ProxyBuilder::new().start().await;
    let mut tunnel = proxy.tls_client("github.test", 443).await;
    let request = Request::builder()
        .uri("/")
        .header("host", "humanitl.internal")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");
    assert!(!body.contains("rules (first match wins)"), "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_endpoint_the_reserved_name_is_an_ordinary_host() {
    // Ohne Meta-Endpunkt entscheidet nur noch die Regel. Der Test hält fest,
    // dass die Weiche und nichts anderes den Unterschied macht.
    let proxy = ProxyBuilder::new()
        .meta(false)
        .rules(BLOCK_INTERNAL)
        .start()
        .await;
    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal/")).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: rule"), "{body}");
}

// ---------------------------------------------------------------------------
// Die drei Pfade über die Leitung
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn an_ask_becomes_one_event_and_no_flow() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            "http://humanitl.internal/ask",
            "bitte https://pypi.org/simple/ freischalten",
        ))
        .await;

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(body_string(response.into_body()).await, "queued\n");

    let FlowEvent::AgentAsk {
        text,
        suggested_host,
        ..
    } = events.wait_for("agent_ask").await
    else {
        unreachable!()
    };
    assert_eq!(text, "bitte https://pypi.org/simple/ freischalten");
    assert_eq!(suggested_host.as_deref(), Some("pypi.org"));
    events.drain();
    assert_eq!(
        events.count("received"),
        0,
        "an ask is a request to the human, not a flow"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ask_over_the_cap_is_refused_before_the_bytes_flow() {
    let proxy = ProxyBuilder::new().start().await;
    let mut client = proxy.client().await;
    let response = client
        .send(post("http://humanitl.internal/ask", vec![b'x'; 4096]))
        .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test(flavor = "multi_thread")]
async fn why_answers_for_a_flow_of_this_session() {
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .start()
        .await;
    let mut events = proxy.events();

    // Ein gewöhnlicher Flow, der in die Zeitüberschreitung läuft.
    let mut client = proxy.client().await;
    let response = client.send(get("http://slow.example/steal")).await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let flow = header(&response, "x-humanitl-flow")
        .expect("the block response names its flow")
        .to_owned();
    events.wait_for("recorded").await;

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://humanitl.internal/why/{flow}")))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_string(response.into_body()).await,
        "decision=timed_out reason=timeout note=\n"
    );

    let mut client = proxy.client().await;
    let response = client
        .send(get(
            "http://humanitl.internal/why/00000000-0000-0000-0000-000000000000",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn other_paths_and_methods_are_refused() {
    let proxy = ProxyBuilder::new().start().await;

    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal/secrets")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let mut client = proxy.client().await;
    let response = client.send(post("http://humanitl.internal/", "x")).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header(&response, "allow"), Some("GET"));

    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal/ask")).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header(&response, "allow"), Some("POST"));
}
