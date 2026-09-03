//! HUM-023: CONNECT-Ziel, SNI und `Host` müssen dasselbe Ziel meinen, und die
//! Regel-Engine entscheidet, was ohne Menschen entschieden wird.
//!
//! Der Angriff, gegen den diese Datei geschrieben ist, heißt Domain Fronting:
//! `CONNECT github.com:443`, und darin `Host: evil.io`. Wer nur das
//! CONNECT-Ziel prüft, hat mit einem Allow für `github.com` ein Allow für
//! alles vergeben (ADR-007). Geprüft wird deshalb das Tripel aus Tunnelziel,
//! SNI und Authority der Anfrage, und ausgewertet wird ausschließlich die
//! Authority der Anfrage.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

mod support;

use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use humanitl_core::{
    Authority, BlockReason, Decision, FlowEvent, HostName, HttpRequest, Method, Scheme,
};
use hyper::{Request, StatusCode};
use support::{ECHO_BODY, FakeUpstream, ProxyBuilder, body_string, get, post};

/// Eine Anfrage in Origin-Form, wie sie in einem Tunnel steht.
fn tunnel_get(path: &str, host: Option<&str>) -> Request<Full<Bytes>> {
    let mut builder = Request::builder().uri(path);
    if let Some(host) = host {
        builder = builder.header("host", host);
    }
    builder.body(Full::new(Bytes::new())).unwrap()
}

/// Eine Entscheidung, die einen gehaltenen Flow sofort beendet, damit ein Test
/// nicht auf die Frist wartet.
fn user_block() -> Decision {
    Decision::Block {
        reason: BlockReason::User,
        note: None,
    }
}

// ---------------------------------------------------------------------------
// Das Tripel
// ---------------------------------------------------------------------------

/// Stimmen Tunnelziel, SNI und `Host` überein, geht die Anfrage den normalen
/// Weg: sie wird gehalten und ein Mensch entscheidet.
#[tokio::test(flavor = "multi_thread")]
async fn authority_ok() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(user_block());

    let mut tunnel = proxy.tls_client("github.com", 443).await;
    let response = tunnel
        .client
        .send(tunnel_get("/repos", Some("github.com")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: user"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1, "a matching triple is asked about");
    assert_eq!(
        events.received_request().authority.to_string(),
        "github.com:443"
    );
}

/// `CONNECT github.com:443` mit `Host: evil.io` darin ist keine Anfrage an
/// `evil.io`: `403`, kein `Held`, und verbucht wird das echte Ziel.
#[tokio::test(flavor = "multi_thread")]
async fn host_mismatch_blocked() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_client("github.com", 443).await;
    let response = tunnel
        .client
        .send(tunnel_get("/repos", Some("evil.io")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");
    assert!(
        body.contains("host: github.com"),
        "the block names the real target: {body}"
    );

    events.wait_for("recorded").await;
    assert_eq!(
        events.count("held"),
        0,
        "nobody is asked about a forged host"
    );
    assert_eq!(events.count("forwarded"), 0);
    assert_eq!(
        events.received_request().authority.to_string(),
        "github.com:443",
        "the flow is recorded under the real target"
    );
    let decided = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Decided { decision, .. } => Some(decision.clone()),
            _ => None,
        })
        .expect("a Decided event");
    assert_eq!(
        decided,
        Decision::Block {
            reason: BlockReason::AuthorityMismatch,
            note: None
        }
    );
    assert_eq!(
        events.count("diagnostic"),
        1,
        "the mismatch is visible in the event stream: {:?}",
        events.names()
    );
}

/// Dieselbe Verbindung, aber der Widerspruch steckt im `ClientHello`.
///
/// Ein Client, der Zertifikate prüft, käme hier gar nicht an: Das Leaf gilt
/// für `github.com`. Der Client in der Sandbox ist aber genau der Prozess,
/// gegen den die Prüfung gerichtet ist, und der prüft nicht.
#[tokio::test(flavor = "multi_thread")]
async fn sni_mismatch_blocked() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_tunnel("github.com", 443, Some("evil.io")).await;
    let response = tunnel
        .client
        .send(tunnel_get("/repos", Some("github.com")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(events.count("forwarded"), 0);
}

/// Ohne SNI lässt sich für einen DNS-Namen nichts belegen.
#[tokio::test(flavor = "multi_thread")]
async fn missing_sni_for_a_name_is_blocked() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_tunnel("github.com", 443, None).await;
    let response = tunnel
        .client
        .send(tunnel_get("/repos", Some("github.com")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
}

/// Für ein IP-Ziel sieht TLS keine SNI vor; ihr Fehlen ist dort kein
/// Widerspruch (der LLM-Host im Heimnetz, ADR-006).
#[tokio::test(flavor = "multi_thread")]
async fn ip_connect_without_sni_ok() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(user_block());

    let mut tunnel = proxy.tls_tunnel("192.168.1.50", 11434, None).await;
    let response = tunnel
        .client
        .send(tunnel_get("/api/tags", Some("192.168.1.50:11434")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(
        body.contains("reason: user"),
        "an address without SNI is asked about, not refused: {body}"
    );

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1);
}

/// Derselbe Host, ein anderer Port: auch das ist ein anderes Ziel.
#[tokio::test(flavor = "multi_thread")]
async fn port_mismatch_blocked() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    // `Host: github.com` ohne Port heißt 443, das Tunnelziel ist 8443.
    let mut tunnel = proxy.tls_client("github.com", 8443).await;
    let response = tunnel
        .client
        .send(tunnel_get("/repos", Some("github.com")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(events.count("forwarded"), 0);
}

/// Eine Anfrage im Tunnel ohne `Host` nennt kein Ziel; das Tunnelziel allein
/// zählt nicht (RFC 9110 §7.2 verlangt den Kopf).
#[tokio::test(flavor = "multi_thread")]
async fn missing_host_h1_blocked() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_client("github.com", 443).await;
    let response = tunnel.client.send(tunnel_get("/repos", None)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
}

// ---------------------------------------------------------------------------
// Die Regel-Engine im Pfad
// ---------------------------------------------------------------------------

/// Eine `allow`-Regel entscheidet selbst: kein `Held`, direkt weitergeleitet.
#[tokio::test(flavor = "multi_thread")]
async fn rule_allow_skips_hold() {
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
    assert_eq!(body_string(response.into_body()).await, ECHO_BODY);

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0, "a rule does not ask");
    assert_eq!(upstream.hits(), 1);
    let source = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Decided { source, .. } => Some(*source),
            _ => None,
        })
        .expect("a Decided event");
    assert_eq!(source.as_str(), "rule", "the rule is named as the source");
}

/// Eine `block`-Regel antwortet mit `403` und dem Grund `rule`; nichts erreicht
/// das Ziel, und niemand wird gefragt.
#[tokio::test(flavor = "multi_thread")]
async fn rule_block_403() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules("version: 1\nrules:\n  - action: block\n    match:\n      host: \"ip:127.0.0.1\"\n")
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: rule"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(events.count("forwarded"), 0);
    assert_eq!(upstream.hits(), 0);
}

/// Ohne passende Regel gilt `ask`: der Flow wird gehalten.
#[tokio::test(flavor = "multi_thread")]
async fn default_ask_holds() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules("version: 1\nrules: []\n")
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(user_block());

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1, "an empty rule set asks");
    assert_eq!(upstream.hits(), 0);
}

/// Der Kern der Sache: Ein `allow` für den Tunnel-Host ist kein `allow` für
/// einen anderen Host darin. Dieselbe Verbindung, dieselbe Regel, zwei
/// Ausgänge.
#[tokio::test(flavor = "multi_thread")]
async fn a_rule_for_the_tunnel_target_never_covers_another_host() {
    let proxy = ProxyBuilder::new()
        .rules("version: 1\nrules:\n  - action: allow\n    match:\n      host: localhost\n")
        .start()
        .await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;
    let mut events = proxy.events();

    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    let forged = tunnel
        .client
        .send(tunnel_get("/echo", Some("evil.io")))
        .await;
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let body = body_string(forged.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");
    assert_eq!(
        upstream.hits(),
        0,
        "the allow for localhost must not carry evil.io"
    );

    // Dieselbe Regel, derselbe Tunnel, aber die Anfrage nennt das Ziel, für
    // das sie gilt.
    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    let honest = tunnel
        .client
        .send(tunnel_get(
            "/echo",
            Some(&format!("localhost:{}", upstream.port())),
        ))
        .await;
    assert_eq!(honest.status(), StatusCode::OK);
    assert_eq!(body_string(honest.into_body()).await, ECHO_BODY);

    events.wait_for_nth("recorded", 2).await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(upstream.hits(), 1);
}

/// Zwei Authorities auf einer Client-Verbindung ergeben zwei
/// Upstream-Verbindungen, auch wenn beide auf dieselbe Adresse zeigen.
///
/// Der Proxy hält keinen Pool über Authorities hinweg: Eine wiederverwendete
/// Verbindung würde die Entscheidung für den einen Host stillschweigend auf
/// den anderen ausdehnen (HUM-023, Abschnitt „Upstream-Verbindungen").
#[tokio::test(flavor = "multi_thread")]
async fn upstream_connections_are_never_shared_across_authorities() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .rules(
            "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"ip:127.0.0.1\"\n  \
             - action: allow\n    match:\n      host: localhost\n",
        )
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let first = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    let _ = body_string(first.into_body()).await;
    let second = client
        .send(get(&format!("http://localhost:{}/echo", upstream.port())))
        .await;
    assert_eq!(second.status(), StatusCode::OK);
    let _ = body_string(second.into_body()).await;

    events.wait_for_nth("recorded", 2).await;
    assert_eq!(
        proxy.egress.connects(),
        2,
        "each authority gets its own connection"
    );
    assert_eq!(upstream.hits(), 2);
}

/// Der `403`-Body nennt Grund, Flow und Host — und sonst nichts Internes
/// (`backlog/CONVENTIONS.md` 3.5, Fallstrick HUM-023).
#[tokio::test(flavor = "multi_thread")]
async fn the_block_body_names_reason_flow_and_host_only() {
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(5))
        .start()
        .await;
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_client("github.com", 443).await;
    let response = tunnel
        .client
        .send(tunnel_get("/repos", Some("evil.io")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    let mut lines = body.lines();
    assert_eq!(lines.next(), Some("Blocked by Humanitl."));
    assert_eq!(lines.next(), Some("reason: authority_mismatch"));
    assert!(lines.next().unwrap().starts_with("flow: "), "{body}");
    assert_eq!(lines.next(), Some("host: github.com"));
    assert!(
        !body.contains("evil.io"),
        "no forged name is echoed: {body}"
    );
}

// ---------------------------------------------------------------------------
// Schema und Ziel bleiben, wofür entschieden wurde
// ---------------------------------------------------------------------------

/// Klartext im Tunnel ist eine Herabstufung: Der Client hat TLS aufgebaut, die
/// Anfrage darin verlangt `http`, und der Weiterleiter richtet sich nach dem
/// Schema der Anfrage. Aus dem `CONNECT` würde ein Klartext-Egress.
#[tokio::test(flavor = "multi_thread")]
async fn a_cleartext_scheme_inside_the_tunnel_is_refused() {
    let proxy = ProxyBuilder::new().start().await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    let request = Request::builder()
        .uri(format!("http://localhost:{}/echo", upstream.port()))
        .header("host", format!("localhost:{}", upstream.port()))
        .body(Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(events.count("forwarded"), 0);
    assert_eq!(upstream.hits(), 0, "nothing may leave in cleartext");
}

/// `https://` ohne `CONNECT` überspränge den Handschlag, an dem die SNI
/// verglichen wird.
#[tokio::test(flavor = "multi_thread")]
async fn a_tls_scheme_without_a_tunnel_is_refused() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("https://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(upstream.hits(), 0);
}

/// Absolut-Form ohne `Host`: HTTP/1.1 verlangt den Kopf, und ohne ihn lassen
/// sich Entscheidung und Ursprung auf zwei Ziele schicken.
#[tokio::test(flavor = "multi_thread")]
async fn an_absolute_form_without_a_host_header_is_refused() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let request = Request::builder()
        .uri(format!("http://127.0.0.1:{}/echo", upstream.port()))
        .body(Full::new(Bytes::new()))
        .unwrap();
    let response = client.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(upstream.hits(), 0);
}

/// Eine bearbeitete Freigabe darf das Schema so wenig ändern wie den Host:
/// derselbe Host im Klartext ist nicht das, wofür entschieden wurde.
#[tokio::test(flavor = "multi_thread")]
async fn allow_edited_cannot_downgrade_the_scheme() {
    let proxy = ProxyBuilder::new().start().await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;
    let mut events = proxy.events();

    let authority = Authority::new(HostName::parse("localhost").unwrap(), upstream.port());
    let downgraded = HttpRequest::new(Method::GET, Scheme::Http, authority, "/echo");
    let _decider = proxy.decide_with(Decision::AllowEdited {
        request: Box::new(downgraded),
    });

    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    let response = tunnel
        .client
        .send(tunnel_get(
            "/echo",
            Some(&format!("localhost:{}", upstream.port())),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: authority_mismatch"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "nothing may leave in cleartext");
}

/// Ein Body über `limits.hold_body_cap_bytes` wird geblockt, bevor ein Byte
/// gelesen wird; niemand wird gefragt, und das Ziel sieht nichts.
///
/// Der Status ist `413`, nicht `403`: `BlockReason::BodyCap` ist im Register
/// (`backlog/CONVENTIONS.md` 3.2, `BlockReason::http_status`) auf 413
/// abgebildet, und HUM-015 prüft das seit dem ersten Tag. Der Klammersatz in
/// `sprint-2.md` HUM-023 („403, nicht 413") widerspricht dem Register; das
/// Register gilt.
#[tokio::test(flavor = "multi_thread")]
async fn body_cap_blocks() {
    let upstream = FakeUpstream::plain().await;
    // Ein kleiner Cap statt der Vorgabe von 32 MiB. Der Proxy antwortet auf ein
    // angekuendigtes Content-Length ueber dem Cap sofort mit 413, ohne den Body
    // zu lesen; schickt der Test dann noch Dutzende MiB hinterher, bricht die
    // Leitung mitten im Schreiben, und der Fehlschlag haengt am Zeitablauf
    // statt an der Aussage. Mit 64 KiB Cap ist der Body in einem Rutsch weg.
    let proxy = ProxyBuilder::new().body_cap(64 * 1024).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let oversized = vec![b'x'; 64 * 1024 + 1];
    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            oversized,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: body_cap"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(
        events.count("held"),
        0,
        "an oversized body is not asked about"
    );
    assert_eq!(events.count("forwarded"), 0);
    assert_eq!(upstream.hits(), 0, "the body never reaches the target");
    let decided = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Decided { decision, .. } => Some(decision.clone()),
            _ => None,
        })
        .expect("a Decided event");
    assert_eq!(
        decided,
        Decision::Block {
            reason: BlockReason::BodyCap,
            note: None
        }
    );
}
