//! Der MITM-Proxy-Kern von außen (HUM-015): Unix-Socket, Halten und
//! Weiterleiten, Block-Antworten, Body-Cap, `CONNECT` mit TLS-Terminierung,
//! DNS erst nach der Entscheidung, private Adressen, Budget.
//!
//! Alles läuft im Prozess: der Proxy auf einem Socket im Temp-Verzeichnis,
//! der Upstream als axum-Server auf `127.0.0.1`, der Resolver als Zähler.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::PermissionsExt as _;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::BodyExt as _;
use humanitl_core::{
    Authority, BlockReason, BodyRef, Decision, FlowEvent, HostName, HttpRequest, Method, Scheme,
};
use humanitl_proxy::ClientTls;
use hyper::{Request, StatusCode};

use support::{ECHO_BODY, FakeUpstream, ProxyBuilder, body_bytes, body_string, get, header, post};

/// Der verbindliche Block-Body (`backlog/CONVENTIONS.md` 3.5).
fn canonical_block(reason: &str, flow: &str, host: &str, note: Option<&str>) -> String {
    let mut body = format!("Blocked by Humanitl.\nreason: {reason}\nflow: {flow}\nhost: {host}\n");
    if let Some(note) = note {
        use std::fmt::Write as _;
        let _ = writeln!(body, "note: {note}");
    }
    body
}

#[tokio::test(flavor = "multi_thread")]
async fn socket_is_0600_in_a_0700_directory() {
    let proxy = ProxyBuilder::new().start().await;
    let file = std::fs::metadata(&proxy.socket).unwrap();
    let dir = std::fs::metadata(proxy.socket.parent().unwrap()).unwrap();
    assert_eq!(file.permissions().mode() & 0o777, 0o600);
    assert_eq!(dir.permissions().mode() & 0o777, 0o700);
    assert_eq!(
        proxy.core.socket_path(proxy.session).as_deref(),
        Some(proxy.socket.as_path())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_held_then_allowed_returns_the_upstream_body() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-upstream"), Some("fake"));
    assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
    assert_eq!(upstream.hits(), 1);

    events.wait_for("recorded").await;
    let names = events.names();
    assert_eq!(
        &names[..6],
        &[
            "received",
            "analyzed",
            "held",
            "decided",
            "forwarded",
            "response_headers"
        ],
        "{names:?}"
    );
    assert!(events.count("response_chunk") >= 1, "{names:?}");
    assert_eq!(names.last(), Some(&"recorded"), "{names:?}");
    let request = events.received_request();
    assert_eq!(request.scheme, Scheme::Http);
    assert_eq!(request.authority.port, upstream.port());
    assert_eq!(request.path_and_query, "/echo");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_request_on_the_same_connection_is_its_own_flow() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let url = format!("http://127.0.0.1:{}/echo", upstream.port());

    for _ in 0..2 {
        let response = client.send(get(&url)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
        events.wait_for("recorded").await;
    }
    assert_eq!(upstream.hits(), 2);
    assert_eq!(events.count("received"), 2);
    assert_eq!(events.count("recorded"), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn block_returns_403_with_the_canonical_body_and_note() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Block {
        reason: BlockReason::User,
        note: Some("Nutze PyPI\r\nstatt GitHub".to_owned()),
    });

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        header(&response, "content-type"),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(header(&response, "connection"), Some("close"));
    assert_eq!(
        header(&response, "x-humanitl-note"),
        Some("Nutze PyPI statt GitHub")
    );
    let flow = header(&response, "x-humanitl-flow").unwrap().to_owned();
    let body = body_string(response.into_body()).await;
    assert_eq!(
        body,
        canonical_block("user", &flow, "127.0.0.1", Some("Nutze PyPI statt GitHub"))
    );

    assert_eq!(
        upstream.hits(),
        0,
        "a blocked request never reaches the upstream"
    );
    assert_eq!(
        proxy.resolver.calls(),
        0,
        "a blocked request never resolves"
    );
    assert_eq!(proxy.egress.connects(), 0);
    events.wait_for("recorded").await;
    assert_eq!(
        events.names(),
        vec!["received", "analyzed", "held", "decided", "recorded"]
    );
    assert!(matches!(
        events.seen[3],
        FlowEvent::Decided {
            decision: Decision::Block {
                reason: BlockReason::User,
                ..
            },
            ..
        }
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn hold_timeout_returns_504() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let flow = header(&response, "x-humanitl-flow").unwrap().to_owned();
    let body = body_string(response.into_body()).await;
    assert_eq!(body, canonical_block("timeout", &flow, "127.0.0.1", None));
    assert_eq!(upstream.hits(), 0);
    assert_eq!(proxy.resolver.calls(), 0);

    events.wait_for("recorded").await;
    assert_eq!(
        events.names(),
        vec!["received", "analyzed", "held", "timed_out", "recorded"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn body_over_the_cap_returns_413_without_holding() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().body_cap(1024).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/echo", upstream.port()),
            vec![b'x'; 2048],
        ))
        .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let flow = header(&response, "x-humanitl-flow").unwrap().to_owned();
    let body = body_string(response.into_body()).await;
    assert_eq!(body, canonical_block("body_cap", &flow, "127.0.0.1", None));
    assert_eq!(upstream.hits(), 0);

    events.wait_for("recorded").await;
    assert_eq!(
        events.names(),
        vec!["received", "analyzed", "decided", "recorded"],
        "over the cap the flow is refused by the system, never held"
    );
    let request = events.received_request();
    assert_eq!(request.body.size, 2048);
    assert!(request.body.truncated);
    assert_eq!(request.body.inline, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn body_exactly_at_the_cap_is_held_and_forwarded() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().body_cap(1024).start().await;
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/echo", upstream.port()),
            vec![b'y'; 1024],
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-echo-len"), Some("1024"));
    assert_eq!(body_bytes(response.into_body()).await.len(), 1024);
}

#[tokio::test(flavor = "multi_thread")]
async fn expect_100_continue_body_lands_in_the_hold_buffer_before_the_decision() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let payload = Bytes::from(vec![b'z'; 64 * 1024]);

    let mut client = proxy.client().await;
    let mut request = post(
        &format!("http://127.0.0.1:{}/echo", upstream.port()),
        payload.clone(),
    );
    request
        .headers_mut()
        .insert("expect", "100-continue".parse().unwrap());
    let send = tokio::spawn(async move { client.send(request).await });

    // Gehalten heißt: der ganze Body ist da, und nichts ist hinausgegangen.
    let held = events.wait_for("held").await;
    let FlowEvent::Held { flow_id, .. } = held else {
        unreachable!()
    };
    assert_eq!(events.received_request().body.size, payload.len() as u64);
    assert_eq!(
        upstream.hits(),
        0,
        "nothing reaches the upstream before the decision"
    );
    assert_eq!(proxy.resolver.calls(), 0);
    assert_eq!(proxy.egress.connects(), 0);

    proxy.queue.decide(flow_id, Decision::Allow).unwrap();
    let response = send.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response.into_body()).await, payload);
    assert_eq!(upstream.hits(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_tunnel_terminates_tls_with_a_leaf_from_our_ca() {
    let proxy = ProxyBuilder::new().start().await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    assert_eq!(
        tunnel.alpn.as_deref(),
        Some(&b"http/1.1"[..]),
        "the proxy offers the client only http/1.1"
    );

    let request = Request::builder()
        .uri("/echo")
        .header("host", format!("localhost:{}", upstream.port()))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
    assert_eq!(upstream.hits(), 1);
    assert_eq!(
        upstream.negotiated_alpn().as_deref(),
        Some(&b"http/1.1"[..]),
        "upstream is forced to h1 even though it offers h2"
    );

    events.wait_for("recorded").await;
    let request = events.received_request();
    assert_eq!(request.scheme, Scheme::Https);
    assert_eq!(
        request.authority.host,
        HostName::Dns("localhost".to_owned())
    );
    assert_eq!(request.authority.port, upstream.port());
    assert_eq!(
        proxy.resolver.calls(),
        1,
        "localhost is resolved through the port, once"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_to_an_unknown_ca_fails_the_client_handshake() {
    // Ein Client, der unserer CA nicht vertraut, bricht den Handschlag ab:
    // kein Fronting, kein stilles Durchleiten.
    let proxy = ProxyBuilder::new().start().await;
    let mut client = proxy.client().await;
    let connect = Request::builder()
        .method(hyper::Method::CONNECT)
        .uri("example.com:443")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = client.send(connect).await;
    assert_eq!(response.status(), StatusCode::OK);
    let upgraded = hyper::upgrade::on(response).await.unwrap();

    let roots = rustls::RootCertStore::empty();
    let config = rustls::ClientConfig::builder_with_provider(proxy.ca.provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
    let name = rustls::pki_types::ServerName::try_from("example.com").unwrap();
    let result = connector
        .connect(name, hyper_util::rt::TokioIo::new(upgraded))
        .await;
    assert!(
        result.is_err(),
        "a client without our CA must not complete the handshake"
    );
    assert_eq!(proxy.resolver.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn dns_is_resolved_only_after_allow_and_the_address_is_pinned() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let request = get(&format!("http://held.test:{}/echo", upstream.port()));
    let send = tokio::spawn(async move { client.send(request).await });

    let held = events.wait_for("held").await;
    let FlowEvent::Held { flow_id, .. } = held else {
        unreachable!()
    };
    assert_eq!(proxy.resolver.calls(), 0, "no DNS before the decision");
    assert_eq!(proxy.egress.connects(), 0, "no egress before the decision");
    assert_eq!(upstream.hits(), 0);

    proxy.queue.decide(flow_id, Decision::Allow).unwrap();
    let response = send.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
    assert_eq!(
        proxy.resolver.calls(),
        1,
        "resolved exactly once, after Allow"
    );
    assert_eq!(
        proxy.egress.connects(),
        1,
        "connected once, to the pinned address"
    );
    assert_eq!(upstream.hits(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_private_address_after_resolution_is_refused_without_a_connect() {
    let proxy = ProxyBuilder::new()
        .passthrough()
        .allow_private(false)
        .resolve_to(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://held.test/echo")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let flow = header(&response, "x-humanitl-flow").unwrap().to_owned();
    let body = body_string(response.into_body()).await;
    assert_eq!(
        body,
        canonical_block("upstream_private_address", &flow, "held.test", None)
    );
    assert_eq!(proxy.resolver.calls(), 1);
    assert_eq!(
        proxy.egress.connects(),
        0,
        "a private address is never connected"
    );

    events.wait_for("recorded").await;
    // Zwischen `failed` und `recorded` steht der Befund `PROXY_008`: Die
    // Ablehnung nennt die Adresse und schlaegt eine Regel vor, die das Ziel
    // oeffnet und die Anfrage weiterhin haelt (HUM-102). Was er enthaelt,
    // prueft `tests/private_address.rs`; hier zaehlt nur, dass er im Strom
    // steht und an dieser Stelle.
    assert_eq!(
        events.names(),
        vec![
            "received",
            "analyzed",
            "decided",
            "forwarded",
            "failed",
            "diagnostic",
            "recorded"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_private_literal_is_refused_too_unless_the_hook_allows_it() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .allow_private(false)
        .start()
        .await;
    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(
        body_string(response.into_body())
            .await
            .contains("reason: upstream_private_address\n")
    );
    assert_eq!(proxy.resolver.calls(), 0, "a literal needs no resolver");
    assert_eq!(proxy.egress.connects(), 0);
    assert_eq!(upstream.hits(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn exhausted_hold_budget_returns_503() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().hold_max_flows(0).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let flow = header(&response, "x-humanitl-flow").unwrap().to_owned();
    let body = body_string(response.into_body()).await;
    assert_eq!(
        body,
        canonical_block("hold_max_flows", &flow, "127.0.0.1", None)
    );
    assert_eq!(upstream.hits(), 0);

    events.wait_for("recorded").await;
    assert_eq!(
        events.names(),
        vec!["received", "analyzed", "decided", "recorded"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_response_is_streamed_not_buffered() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/stream", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let started = Instant::now();
    let mut body = response.into_body();

    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(first, Bytes::from_static(b"first\n"));
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "the first chunk must arrive before the upstream sends the second"
    );
    let mut rest = Vec::new();
    while let Some(frame) = body.frame().await {
        if let Ok(data) = frame.unwrap().into_data() {
            rest.extend_from_slice(&data);
        }
    }
    assert_eq!(rest, b"second\n");

    events.wait_for("recorded").await;
    assert_eq!(events.count("response_chunk"), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_without_a_host_is_400_before_any_flow() {
    let proxy = ProxyBuilder::new().start().await;
    let events = proxy.events();
    let mut client = proxy.client().await;
    let request = Request::builder()
        .uri("/echo")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = client.send(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_string(response.into_body()).await, "missing host\n");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(events.seen.is_empty());
    assert_eq!(proxy.queue.queue_count(), 0);
}

#[test]
fn h2_upstream_flag_switches_the_alpn_offer() {
    let h1 = ClientTls::new(&[], false).unwrap();
    assert_eq!(h1.alpn(), vec![b"http/1.1".to_vec()]);
    let h2 = ClientTls::new(&[], true).unwrap();
    assert_eq!(h2.alpn(), vec![b"h2".to_vec(), b"http/1.1".to_vec()]);
}

/// Eine bearbeitete Freigabe nimmt den neuen Body; das Ziel darf sie nicht
/// aendern (HUM-015 `allow_edited_changes_body`).
fn edited_post(authority: Authority, body: &'static str) -> HttpRequest {
    HttpRequest::new(Method::POST, Scheme::Http, authority, "/echo")
        .with_body(BodyRef::from_bytes(Bytes::from_static(body.as_bytes())))
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_edited_changes_body() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let authority = Authority {
        host: HostName::parse("127.0.0.1").unwrap(),
        port: upstream.port(),
    };
    let _decider = proxy.decide_with(Decision::AllowEdited {
        request: Box::new(edited_post(authority, "edited body")),
    });

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/echo", upstream.port()),
            "original body",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_string(response.into_body()).await,
        "edited body",
        "the upstream must see the edited body, not the original"
    );
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_edited_cannot_change_the_authority() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    // Ein anderes Ziel als das, fuer das entschieden wurde: derselbe Host,
    // ein anderer Port genuegt, um die Pruefung zu treffen.
    let elsewhere = Authority {
        host: HostName::parse("127.0.0.1").unwrap(),
        port: upstream.port().wrapping_add(1).max(1),
    };
    let _decider = proxy.decide_with(Decision::AllowEdited {
        request: Box::new(edited_post(elsewhere, "edited body")),
    });

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/echo", upstream.port()),
            "original body",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(
        body.contains("reason: authority_mismatch"),
        "the block names the reason: {body}"
    );
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "nothing may reach any upstream");
}

/// Ein CONNECT nach A mit `Host: B` darin ist keine Anfrage an B: Der Proxy
/// lehnt sie ohne Rueckfrage ab und verbucht sie mit dem Tunnelziel als
/// Authority (ESC-3 `host_mismatch_blocked`, Vorstufe von HUM-023).
#[tokio::test(flavor = "multi_thread")]
async fn a_host_header_that_contradicts_the_tunnel_is_refused_unasked() {
    let proxy = ProxyBuilder::new().start().await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    let request = Request::builder()
        .uri("/echo")
        .header("host", "evil.test")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");
    assert!(
        body.contains("host: localhost"),
        "the block names the real target: {body}"
    );
    events.wait_for("recorded").await;
    assert_eq!(
        events.count("held"),
        0,
        "nobody is asked about a forged host"
    );
    assert_eq!(upstream.hits(), 0);
}

/// Dasselbe ohne Tunnel: Absolut-Form und `Host` muessen zusammenpassen.
#[tokio::test(flavor = "multi_thread")]
async fn a_host_header_that_contradicts_the_request_line_is_refused_unasked() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let request = Request::builder()
        .uri(format!("http://127.0.0.1:{}/echo", upstream.port()))
        .header("host", "evil.test")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = client.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");
    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(upstream.hits(), 0);
}
