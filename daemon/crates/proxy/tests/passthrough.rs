//! HUM-039 im Proxy-Pfad: die eine erklärte Ausnahme und ihre Grenzen.
//!
//! Der Durchreiche zum Sprachmodell wird nicht gehalten. Das ist die einzige
//! Stelle, an der etwas den Rechner verlässt, ohne dass ein Mensch zugestimmt
//! hat, und deshalb prüft diese Datei nicht, dass sie funktioniert, sondern wo
//! sie aufhört:
//!
//! 1. Sie trifft genau den Endpunkt, und `POST /api/pull` gehört nicht dazu.
//! 2. Die Antwort strömt, der Request-Body wird gepuffert (ADR-005), und beides
//!    steht vollständig in der Aufzeichnung.
//! 3. Funde halten sie nicht auf, aber sie erzeugen `LLM_005` — einmal je Fluss,
//!    mit Zahl und Host und ohne den gefundenen Wert.
//! 4. Das Recht auf private Zieladressen gehört der Regel und endet mit ihr.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use humanitl_core::{Decision, DecisionSource, FlowEvent};
use humanitl_findings::FindingsSettings;
use humanitl_proxy::{Scanner, Tier1Scanner};
use hyper::{Method, Request, StatusCode};
use support::{FakeUpstream, ProxyBuilder, body_string, post};

/// Ein GitHub-Token in der Form, die der Detektor kennt: `ghp_` und 36 Zeichen.
const TOKEN: &str = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";

/// Der Regelsatz einer Sitzung mit einem lokalen Modell auf `127.0.0.1:PORT`.
///
/// Genau so, wie `OpenCodeAdapter::llm_passthrough` sie baut: Host als
/// IP-Literal, Port, Schema, `GET` und `POST`, und einzelne Inferenz-Endpunkte
/// — weder das nackte `/api/` noch das nackte `/v1/`.
fn rules(port: u16) -> String {
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
         \x20     path_prefixes: [\"/v1/chat/completions\", \"/v1/models\", \"/api/chat\", \
         \"/api/tags\"]\n\
         \x20   allow_private: true\n\
         \x20   passthrough_llm: true\n\
         \x20   note: \"LLM passthrough. Logged, never held.\"\n"
    )
}

/// Die echten Detektoren mit den Vorgabe-Einstellungen.
fn tier1() -> Arc<dyn Scanner> {
    Arc::new(Tier1Scanner::new(&FindingsSettings::default()).unwrap())
}

/// Die Durchreiche trifft den Endpunkt und nichts daneben.
///
/// Alles, was sie nicht trifft, landet in der Warteschlange, und die
/// Entscheidung dort ist in diesem Test `Block`. Ein `403` heißt deshalb: Es
/// wurde gefragt. Ein `200` an einer Stelle, an der nicht gefragt werden soll,
/// wäre der Fehler, den dieser Test sucht.
#[tokio::test(flavor = "multi_thread")]
async fn passthrough_matches_only_the_endpoint() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules(&rules(port))
        // Die Verbindung selbst darf keine privaten Ziele: Was durchkommt,
        // kommt allein wegen `allow_private` an der Regel durch.
        .allow_private(false)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: None,
    });

    for (method, path, expected) in [
        (Method::POST, "/v1/chat/completions", StatusCode::OK),
        (Method::GET, "/v1/models", StatusCode::OK),
        (Method::POST, "/api/chat", StatusCode::OK),
        (Method::GET, "/api/tags", StatusCode::OK),
        // Nachladen und Löschen ändern den Server; sie gehören in die
        // Warteschlange, nicht in die Durchreiche.
        (Method::POST, "/api/pull", StatusCode::FORBIDDEN),
        // Und auch nicht über einen Umweg, den erst der Server auflöst.
        (Method::POST, "/api/chat/../pull", StatusCode::FORBIDDEN),
        (Method::POST, "/echo", StatusCode::FORBIDDEN),
        // `/v1/` ist eine Fläche und kein Endpunkt: Dateien ablegen gehört
        // nicht zur Inferenz.
        (Method::POST, "/v1/files", StatusCode::FORBIDDEN),
    ] {
        let mut client = proxy.client().await;
        let uri = format!("http://127.0.0.1:{port}{path}?count=1&interval_ms=1");
        let request = Request::builder()
            .method(method.clone())
            .uri(&uri)
            .header("host", format!("127.0.0.1:{port}"))
            .body(Full::new(Bytes::new()))
            .unwrap();
        let response = client.send(request).await;
        assert_eq!(response.status(), expected, "{method} {path}");
    }
}

/// Ein anderer Port und ein anderer Host treffen die Regel nicht.
#[tokio::test(flavor = "multi_thread")]
async fn passthrough_does_not_cover_a_neighbour() {
    let upstream = FakeUpstream::ollama().await;
    let other = FakeUpstream::ollama().await;
    let proxy = ProxyBuilder::new()
        .rules(&rules(upstream.port()))
        .allow_private(false)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: None,
    });

    let mut client = proxy.client().await;
    let response = client
        .send(support::get(&format!(
            "http://127.0.0.1:{}/api/tags",
            other.port()
        )))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "a second server on the same machine is not the declared endpoint"
    );
    assert_eq!(other.hits(), 0, "and nothing reached it");
}

/// Die Antwort strömt: Das erste Stück ist beim Client, lange bevor der Server
/// mit dem letzten fertig ist. Die Aufzeichnung hat trotzdem alles.
#[tokio::test(flavor = "multi_thread")]
async fn passthrough_streams_sse_and_records_the_whole_body() {
    const CHUNKS: usize = 50;
    const INTERVAL_MS: u64 = 20;

    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules(&rules(port))
        .allow_private(false)
        .recording(true)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let sent = Instant::now();
    let mut response = client
        .send(post(
            &format!(
                "http://127.0.0.1:{port}/v1/chat/completions\
                 ?count={CHUNKS}&interval_ms={INTERVAL_MS}"
            ),
            r#"{"model":"qwen","stream":true}"#,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let first = response
        .body_mut()
        .frame()
        .await
        .expect("a first frame")
        .unwrap();
    let elapsed = sent.elapsed();
    assert!(
        elapsed < Duration::from_millis(100),
        "the first chunk arrived after {elapsed:?}; a buffered response would take at least {}ms",
        u64::try_from(CHUNKS).unwrap_or(0) * INTERVAL_MS
    );
    assert!(
        first
            .data_ref()
            .is_some_and(|data| data.starts_with(b"data:")),
        "the first frame carries the first event"
    );

    let rest = body_string(response.into_body()).await;
    assert!(
        rest.ends_with("data: [DONE]\n\n"),
        "the stream ran to its end"
    );

    let decided = events.wait_for("decided").await;
    let FlowEvent::Decided {
        flow_id, source, ..
    } = decided
    else {
        panic!("decided is decided");
    };
    assert_eq!(
        source,
        DecisionSource::Passthrough,
        "the recorder marks the flow by this source, and only this source"
    );
    assert_eq!(events.count("held"), 0, "a passthrough is never held");
    events.wait_for("recorded").await;

    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow_id)
        .await
        .unwrap()
        .expect("the passthrough is in the history");
    assert_eq!(detail.summary.decision.as_deref(), Some("allow"));
    assert!(
        detail.summary.passthrough,
        "the history has to be able to show it in amber"
    );

    let response_record = detail
        .messages
        .iter()
        .find(|message| message.dir == humanitl_recorder::Dir::Response)
        .expect("the response is recorded");
    let mirrored = recorder.read_body(&response_record.body).await.unwrap();
    let mirrored = String::from_utf8(mirrored.to_vec()).unwrap();
    assert_eq!(
        mirrored.matches("data: ").count(),
        CHUNKS + 1,
        "every chunk and the terminator are in the recording"
    );

    let request_record = detail
        .messages
        .iter()
        .find(|message| message.dir == humanitl_recorder::Dir::Request)
        .expect("the request is recorded");
    let sent_body = recorder.read_body(&request_record.body).await.unwrap();
    assert_eq!(
        sent_body,
        Bytes::from_static(br#"{"model":"qwen","stream":true}"#),
        "the request body was buffered and recorded, never streamed (ADR-005)"
    );
}

/// Funde halten die Durchreiche nicht auf — sie erzeugen genau eine Warnung.
#[tokio::test(flavor = "multi_thread")]
async fn passthrough_findings_emit_llm_005() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules(&rules(port))
        .allow_private(false)
        .recording(true)
        .scanner(tier1())
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{port}/v1/chat/completions?count=1&interval_ms=1"),
            format!(r#"{{"prompt":"here is my token {TOKEN}"}}"#),
        ))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a finding warns, it does not hold"
    );
    let _ = body_string(response.into_body()).await;

    events.wait_for("recorded").await;
    let diagnostics: Vec<_> = events
        .seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Diagnostic {
                flow_id,
                diagnostic,
                ..
            } if diagnostic.code.as_str() == "LLM_005" => Some((*flow_id, diagnostic.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(diagnostics.len(), 1, "exactly one warning per flow");
    let (flow_id, diagnostic) = &diagnostics[0];
    assert!(flow_id.is_some(), "the warning opens the flow details");
    assert_eq!(diagnostic.severity, humanitl_core::Severity::Warning);
    assert!(
        diagnostic.why.contains('1') && diagnostic.why.contains("127.0.0.1"),
        "the warning names the count and the host: {}",
        diagnostic.why
    );
    assert!(
        !diagnostic.why.contains(TOKEN) && !diagnostic.why.contains("ghp_"),
        "and never the value itself: {}",
        diagnostic.why
    );

    let names = events.names();
    assert_eq!(
        names.iter().filter(|name| **name == "held").count(),
        0,
        "still not held: {names:?}"
    );
    assert!(
        names.contains(&"forwarded"),
        "the flow reached the model: {names:?}"
    );

    // Der Fund selbst ist aufgezeichnet, mit Hash und Ort statt mit dem Wert.
    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let flow = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Received { flow_id, .. } => Some(*flow_id),
            _ => None,
        })
        .unwrap();
    let detail = recorder.get_flow(flow).await.unwrap().unwrap();
    assert_eq!(detail.findings.len(), 1, "{:?}", detail.findings);
    assert!(detail.findings[0].kind.starts_with("api_key"));
    assert_ne!(
        detail.findings[0].value_hash, [0_u8; 32],
        "the finding is recorded by its hash, never by its value"
    );
    assert!(
        !detail.findings[0].display_prefix.contains("Q7r8"),
        "and the preview is only a beginning: {}",
        detail.findings[0].display_prefix
    );
}

/// `allow_private` gehört der Regel und wirkt nicht über sie hinaus.
///
/// Zwei Regeln auf demselben Host: die Durchreiche mit `allow_private: true`
/// und eine gewöhnliche Freigabe ohne. Die zweite muss am privaten Ziel
/// scheitern, auch wenn die erste kurz davor durchgekommen ist — und zwar auf
/// derselben Verbindung, damit ein Zustand, der an der Verbindung klebte,
/// auffiele.
#[tokio::test(flavor = "multi_thread")]
async fn allow_private_belongs_to_the_rule_and_ends_with_it() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let yaml = format!(
        "{}\
         \x20 - id: 01920000-0000-7000-8000-0000000000fe\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"ip:127.0.0.1\"\n\
         \x20     port: {port}\n\
         \x20     path: \"/echo\"\n\
         \x20   note: \"an ordinary allow without allow_private\"\n",
        rules(port)
    );
    let proxy = ProxyBuilder::new()
        .rules(&yaml)
        .allow_private(false)
        .ask(Duration::from_secs(30))
        .start()
        .await;

    let mut client = proxy.client().await;
    let first = client
        .send(support::get(&format!("http://127.0.0.1:{port}/api/tags")))
        .await;
    assert_eq!(
        first.status(),
        StatusCode::OK,
        "the passthrough rule may talk to a private address"
    );
    let _ = body_string(first.into_body()).await;

    let second = client
        .send(support::get(&format!("http://127.0.0.1:{port}/echo")))
        .await;
    assert_eq!(
        second.status(),
        StatusCode::BAD_GATEWAY,
        "the next rule on the same connection may not"
    );
    let body = body_string(second.into_body()).await;
    assert!(
        body.contains("upstream_private_address"),
        "and it says why: {body}"
    );
}
