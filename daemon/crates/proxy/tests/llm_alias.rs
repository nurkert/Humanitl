//! HUM-387 im Proxy-Pfad: `llm.humanitl.internal` erreicht das Modell auf dem
//! Loopback und sonst nichts.
//!
//! `OpenCode` 2.2.5 spricht eine Loopback-`baseURL` am Proxy vorbei an; der
//! Adapter schreibt deshalb diesen Namen hinein. Diese Datei prüft, wohin der
//! Name führt, gegen ein Ziel auf `127.0.0.1` und mit einer Verbindung ohne
//! eigenes Recht auf private Ziele, so dass nur die Durchreiche es geben kann:
//!
//! 1. Der Name mit dem Port des Endpunkts geht an genau dieses Ziel, über die
//!    Durchreiche, ungehalten, ohne Namensauflösung, und der Fluss steht unter
//!    dem echten Ziel.
//! 2. Ein anderer Port unter dem Namen wird abgelehnt, ohne Regel, ohne
//!    Auflösung, ohne Verbindung.
//! 3. Ohne Endpunkt auf dem Loopback wird jede Anfrage an den Namen abgelehnt,
//!    auch wenn eine Regel sie erlauben würde.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use humanitl_core::diagnostics::codes::PROXY_003;
use humanitl_core::{BlockReason, Decision, DecisionSource, FlowEvent, HostName, LLM_ALIAS_HOST};
use hyper::StatusCode;
use support::{FakeUpstream, Proxy, ProxyBuilder, body_string, get, post};

/// Die Durchreiche einer Sitzung mit dem Modell auf `127.0.0.1:PORT`, wie
/// `OpenCodeAdapter::llm_passthrough` sie baut.
fn passthrough(port: u16) -> String {
    format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: 01920000-0000-7000-8000-0000000000ff\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"ip:127.0.0.1\"\n\
         \x20     port: {port}\n\
         \x20     scheme: http\n\
         \x20     method: [POST, GET]\n\
         \x20     path_prefixes: [\"/v1/chat/completions\", \"/v1/models\"]\n\
         \x20   allow_private: true\n\
         \x20   passthrough_llm: true\n"
    )
}

/// Ein Proxy ohne eigenes Recht auf private Ziele und mit der Durchreiche.
async fn proxy_for(endpoint: Option<&str>, rules: &str) -> Proxy {
    let builder = ProxyBuilder::new().allow_private(false).rules(rules);
    match endpoint {
        Some(endpoint) => builder.llm_endpoint(endpoint),
        None => builder,
    }
    .start()
    .await
}

fn alias_url(port: u16, path: &str) -> String {
    format!("http://{LLM_ALIAS_HOST}:{port}{path}")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_name_reaches_the_loopback_model_through_the_passthrough() {
    let upstream = FakeUpstream::openai().await;
    let port = upstream.port();
    let proxy = proxy_for(
        Some(&format!("http://127.0.0.1:{port}/v1")),
        &passthrough(port),
    )
    .await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;

    let response = client
        .send(post(
            &alias_url(port, "/v1/chat/completions"),
            r#"{"model":"m","messages":[]}"#,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = body_string(response.into_body()).await;
    events.wait_for("recorded").await;

    assert_eq!(upstream.hits(), 1, "the model got the request");
    assert_eq!(
        proxy.resolver.calls(),
        0,
        "neither the name nor the target was resolved: {:?}",
        proxy.resolver.hosts()
    );
    let names = events.names();
    assert!(!names.contains(&"held"), "never held: {names:?}");
    assert!(
        events.seen.iter().any(|event| matches!(
            event,
            FlowEvent::Decided {
                decision: Decision::Allow,
                source: DecisionSource::Passthrough,
                ..
            }
        )),
        "decided by the passthrough: {names:?}"
    );
    // Der Fluss steht unter dem Ziel, zu dem die Bytes gingen; der Name, den
    // der Agent benutzt hat, steht in seinem `Host`.
    let request = events.received_request();
    assert_eq!(
        request.authority.host,
        HostName::parse("127.0.0.1").unwrap()
    );
    assert_eq!(request.authority.port, port);
    let host = request
        .headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(host, format!("{LLM_ALIAS_HOST}:{port}"));

    // Auch die Modellliste, die `OpenCode` mit `GET` holt.
    let response = client.send(get(&alias_url(port, "/v1/models"))).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(upstream.hits(), 2);
}

/// Ein `CONNECT` zum Namen mit Klartext darin, wie undici mit `proxyTunnel`
/// (HUM-364), geht denselben Weg.
#[tokio::test(flavor = "multi_thread")]
async fn a_cleartext_tunnel_to_the_name_reaches_the_model_too() {
    let upstream = FakeUpstream::openai().await;
    let port = upstream.port();
    let proxy = proxy_for(
        Some(&format!("http://127.0.0.1:{port}/v1")),
        &passthrough(port),
    )
    .await;
    let target = format!("{LLM_ALIAS_HOST}:{port}");
    let answer = proxy
        .cleartext_tunnel(
            &target,
            &format!("GET /v1/models HTTP/1.1\r\nHost: {target}\r\nConnection: close\r\n\r\n"),
        )
        .await;
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert_eq!(upstream.hits(), 1);
    assert_eq!(proxy.resolver.calls(), 0);
}

/// Ein anderer Port unter dem Namen erreicht nichts: kein Dienst des Hosts
/// neben dem Modell, auch nicht, wenn eine Regel ihn erlaubte.
#[tokio::test(flavor = "multi_thread")]
async fn another_port_under_the_name_is_refused() {
    let upstream = FakeUpstream::openai().await;
    let other = FakeUpstream::openai().await;
    let port = upstream.port();
    // Eine Regel, die auch den anderen Port durchließe.
    let rules = format!(
        "{}\
         \x20 - action: allow\n\
         \x20   match:\n\
         \x20     host: \"ip:127.0.0.1\"\n\
         \x20     port: {}\n\
         \x20   allow_private: true\n",
        passthrough(port),
        other.port()
    );
    let proxy = proxy_for(Some(&format!("http://127.0.0.1:{port}/v1")), &rules).await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;

    let response = client
        .send(get(&alias_url(other.port(), "/v1/models")))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    events.wait_for("recorded").await;
    assert_refused(&proxy, &events.seen, other.port());
    assert_eq!(other.hits(), 0);
    assert_eq!(upstream.hits(), 0);
}

/// Ohne Endpunkt auf dem Loopback hat der Name kein Ziel: weder ohne
/// Endpunkt noch mit einem im eigenen Netz.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_loopback_endpoint_the_name_is_refused() {
    let upstream = FakeUpstream::openai().await;
    let port = upstream.port();
    for endpoint in [None, Some("http://192.168.1.50:11434")] {
        let proxy = proxy_for(endpoint, &passthrough(port)).await;
        let mut events = proxy.events();
        let mut client = proxy.client().await;
        let response = client.send(get(&alias_url(port, "/v1/models"))).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{endpoint:?}");
        events.wait_for("recorded").await;
        assert_refused(&proxy, &events.seen, port);
        assert_eq!(upstream.hits(), 0, "{endpoint:?}");
    }
}

/// Abgelehnt ohne Frage, ohne Auflösung, ohne Verbindung, mit `PROXY_003`
/// und unter dem Namen, denn die Anfrage ging nirgendwohin.
fn assert_refused(proxy: &Proxy, seen: &[FlowEvent], port: u16) {
    let names: Vec<&str> = seen.iter().map(FlowEvent::name).collect();
    assert!(!names.contains(&"held"), "never held: {names:?}");
    assert!(!names.contains(&"forwarded"), "never forwarded: {names:?}");
    assert_eq!(proxy.resolver.calls(), 0, "the name is never resolved");
    assert_eq!(proxy.egress.connects(), 0, "nothing is connected");
    assert!(
        seen.iter().any(|event| matches!(
            event,
            FlowEvent::Decided {
                decision: Decision::Block {
                    reason: BlockReason::NoRoute,
                    ..
                },
                source: DecisionSource::System,
                ..
            }
        )),
        "{names:?}"
    );
    assert!(
        seen.iter().any(|event| matches!(
            event,
            FlowEvent::Diagnostic { diagnostic, .. } if diagnostic.code == PROXY_003
        )),
        "{names:?}"
    );
    let request = seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Received { request, .. } => Some(request),
            _ => None,
        })
        .expect("a Received event");
    assert!(request.authority.host.is_llm_alias());
    assert_eq!(request.authority.port, port);
}
