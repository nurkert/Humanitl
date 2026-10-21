//! Die Grenzen aus `limits.*` am laufenden Proxy (HUM-057).
//!
//! Vier Fälle aus der Spezifikation:
//!
//! 1. Ein Rumpf über `limits.hold_body_cap_bytes` wird geblockt, und der
//!    Fluss trägt `LIMIT_001` mit dem Vorschlag einer Regel `stream: true`
//!    (`body_cap_blocks_with_diagnostic`).
//! 2. Dieselbe Anfrage unter einer Regel mit `stream: true` geht gestreamt
//!    hinaus, vollständig, mitgeschnitten und als ungeprüft vermerkt
//!    (`body_cap_stream_rule_passes`).
//! 3. Ein Antwort-Rumpf, der verstummt, wird nach `limits.body_timeout_secs`
//!    abgeschnitten, und der Fluss trägt `LIMIT_004` (`upstream_idle_timeout`).
//! 4. Ein Client, der seinen Kopf nicht zu Ende schickt, wird nach
//!    `limits.header_timeout_secs` getrennt, ohne Fluss, mit einer
//!    Protokollzeile `LIMIT_005` (`client_header_timeout`).
//!
//! Die Größen sind kleiner als in der Spezifikation (40 MiB gegen 32 MiB),
//! wo der Rumpf wirklich fließt: Das Ziel dieser Tests ist ein axum-Endpunkt
//! mit dessen Vorgabe von 2 MiB je Rumpf, und es kommt auf das Verhältnis zum
//! Cap an, nicht auf die absolute Zahl. Wo nichts fließt (der Block auf die
//! Ankündigung), steht die Zahl der Spezifikation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::{LIMIT_001, LIMIT_004};
use humanitl_core::rule::Action;
use humanitl_core::{
    BlockReason, Decision, DecisionSource, Diagnostic, FixAction, FlowEvent, FlowId, Severity,
};
use humanitl_recorder::{Dir, FlowQuery};
use hyper::StatusCode;
use support::{FakeUpstream, ProxyBuilder, body_string, post};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;

/// Die feste Kennung der Regel in [`rules`], damit ein Test sie in
/// `DecisionSource::Rule` wiedererkennt.
const RULE_ID: &str = "01920000-0000-7000-8000-000000000057";

/// Das Cap der Tests, in denen der Rumpf wirklich fließt.
const CAP: u64 = 256 * 1024;

/// Ein Rumpf, viermal so groß wie [`CAP`] und unter der Grenze des Ziels.
const BIG: usize = 1024 * 1024;

/// Eine Regel für das Ziel der Tests auf `127.0.0.1:port`, Pfad `/sink`.
fn rules(port: u16, action: &str, stream: bool) -> String {
    format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {RULE_ID}\n\
         \x20   action: {action}\n\
         \x20   match:\n\
         \x20     host: \"ip:127.0.0.1\"\n\
         \x20     port: {port}\n\
         \x20     scheme: http\n\
         \x20     method: [POST]\n\
         \x20     path_prefixes: [\"/sink\"]\n\
         \x20   stream: {stream}\n"
    )
}

/// Alle Befunde zu einem Fluss, in Reihenfolge.
fn diagnostics_of(events: &[FlowEvent], flow: FlowId) -> Vec<Diagnostic> {
    events
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Diagnostic {
                flow_id: Some(id),
                diagnostic,
                ..
            } if *id == flow => Some((**diagnostic).clone()),
            _ => None,
        })
        .collect()
}

/// Die Kennung des ersten Flusses im Strom.
fn first_flow(events: &[FlowEvent]) -> FlowId {
    events
        .iter()
        .find_map(|event| match event {
            FlowEvent::Received { flow_id, .. } => Some(*flow_id),
            _ => None,
        })
        .expect("a Received event")
}

/// Die Entscheidung über einen Fluss samt Herkunft.
fn decided(events: &[FlowEvent]) -> (Decision, DecisionSource) {
    events
        .iter()
        .find_map(|event| match event {
            FlowEvent::Decided {
                decision, source, ..
            } => Some((decision.clone(), *source)),
            _ => None,
        })
        .expect("a Decided event")
}

/// Eine angekündigte Anfrage über dem Cap wird geblockt, und am Fluss steht
/// `LIMIT_001` mit dem Vorschlag, den ein Mensch anklicken kann.
///
/// Der Host ist durch eine Regel erlaubt: Das Cap gilt trotzdem, solange die
/// Regel kein `stream: true` trägt (ESC-4 `rule_body_over_cap`). Die Antwort
/// ist `413`, nicht `403` wie in der Spezifikation: `BlockReason::BodyCap` ist
/// im Register auf 413 abgebildet (`backlog/CONVENTIONS.md` 3.2).
#[tokio::test(flavor = "multi_thread")]
async fn body_cap_blocks_with_diagnostic() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules(&rules(port, "allow", false))
        .recording(true)
        .start()
        .await;
    let mut events = proxy.events();

    let announced = 40u64 * 1024 * 1024;
    let answer = proxy
        .raw_exchange(
            &format!(
                "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Length: {announced}\r\n\r\n"
            ),
            b"xxxxxxxxxxxxxxxx",
        )
        .await;
    assert!(answer.starts_with("HTTP/1.1 413 "), "{answer}");
    assert!(answer.contains("reason: body_cap"), "{answer}");

    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "the body never reaches the target");
    let flow = first_flow(&events.seen);
    assert_eq!(
        decided(&events.seen).0,
        Decision::Block {
            reason: BlockReason::BodyCap,
            note: None
        }
    );

    let found = diagnostics_of(&events.seen, flow);
    let limit = found
        .iter()
        .find(|diagnostic| diagnostic.code == LIMIT_001)
        .unwrap_or_else(|| panic!("LIMIT_001 at the flow, got {found:?}"));
    assert_eq!(limit.severity, Severity::Warning);
    assert!(
        limit.why.contains(&format!("{announced} bytes")),
        "{}",
        limit.why
    );
    assert!(
        limit.why.contains("limits.hold_body_cap_bytes (33554432)"),
        "{}",
        limit.why
    );
    let Some(FixAction::AddRule(rule)) = &limit.fix else {
        panic!("the fix is a rule, got {:?}", limit.fix);
    };
    assert!(rule.stream, "the suggested rule streams");
    assert_eq!(rule.action, Action::Ask, "and keeps asking");
    assert_eq!(rule.matcher.path_prefixes, vec!["/sink".to_owned()]);
    // Derselbe Befund steht am Datensatz, aus dem das Detail der Historie
    // ihn holt (`FlowDetail.diagnostics`, HUM-058 Zeile 9).
    let record = proxy.queue.registry().get(flow).expect("the record");
    assert!(
        record
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == limit),
        "{:?}",
        record.diagnostics
    );

    // In der Aufzeichnung: geblockt, mit dem Grund.
    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let rows = recorder.list_flows(&FlowQuery::new("")).await.unwrap().rows;
    let row = rows
        .iter()
        .find(|row| row.id == flow)
        .expect("the flow row");
    assert_eq!(row.decision.as_deref(), Some("block"));
    assert_eq!(row.block_reason.as_deref(), Some("body_cap"));
}

/// Ein am Cap geblockter Fluss zeigt nach einem Neustart des Daemons
/// `LIMIT_001` mit dem Regelvorschlag im Detail der Historie (HUM-256).
///
/// Der neue Daemon hat eine leere Registry; was er über den Fluss weiß,
/// kommt aus der Aufzeichnung. Der Vorschlag muss dieselbe Regel sein, die
/// der laufende Daemon angeboten hat, gelesen mit dem Leser der `rules.yaml`,
/// in deren Form sie abgelegt ist.
///
/// Hier wird der Rundweg im Kern geprüft (`FixAction` gegen `FixAction`); die
/// Wire-Form, die `GetFlow` nach dem Neustart liefert, prüft
/// `get_flow_after_a_restart_takes_the_diagnostics_from_the_recording` in
/// `humanitl-ipc`.
#[tokio::test(flavor = "multi_thread")]
async fn a_flow_blocked_at_the_cap_keeps_limit_001_after_a_restart() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let first = ProxyBuilder::new()
        .rules(&rules(port, "allow", false))
        .recording(true)
        .start()
        .await;
    let mut events = first.events();
    let answer = first
        .raw_exchange(
            &format!(
                "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Length: {}\r\n\r\n",
                40u64 * 1024 * 1024
            ),
            b"xxxxxxxxxxxxxxxx",
        )
        .await;
    assert!(answer.starts_with("HTTP/1.1 413 "), "{answer}");
    events.wait_for("recorded").await;
    let flow = first_flow(&events.seen);
    let live = diagnostics_of(&events.seen, flow);
    let offered = live
        .iter()
        .find(|diagnostic| diagnostic.code == LIMIT_001)
        .unwrap_or_else(|| panic!("LIMIT_001 at the flow, got {live:?}"))
        .clone();
    assert!(
        matches!(offered.fix, Some(FixAction::AddRule(_))),
        "{offered:?}"
    );
    first
        .recorder
        .as_ref()
        .expect("recording is on")
        .flush()
        .await;

    let restarted = ProxyBuilder::new().restart_of(&first).start().await;
    assert_eq!(
        restarted.queue.registry().get(flow),
        None,
        "a fresh daemon starts with an empty registry"
    );
    let detail = restarted
        .recorder
        .as_ref()
        .expect("a restart records")
        .get_flow(flow)
        .await
        .unwrap()
        .expect("the flow is in the history");
    let codes: Vec<&str> = detail.diagnostics.iter().map(|d| d.code.as_str()).collect();
    let expected: Vec<&str> = live.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, expected, "the same diagnostics in the same order");

    let recorded = detail
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "LIMIT_001")
        .expect("LIMIT_001 survived the restart");
    assert_eq!(recorded.why, offered.why);
    assert_eq!(recorded.severity(), Some(Severity::Warning));
    let session = detail.summary.session;
    let fix = recorded.fix(|yaml| {
        humanitl_rules::parse_rules_for_session(yaml, session)
            .ok()
            .and_then(|(set, _)| set.iter().next().cloned())
    });
    assert_eq!(fix, offered.fix, "the same suggestion, rule for rule");
}

/// Ohne angekündigte Länge fällt das Cap erst beim Puffern auf; derselbe
/// Block, und der Befund sagt „mehr als".
#[tokio::test(flavor = "multi_thread")]
async fn a_chunked_body_over_the_cap_is_blocked_with_limit_001() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .body_cap(16 * 1024)
        .rules(&rules(port, "allow", false))
        .start()
        .await;
    let mut events = proxy.events();

    let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let chunk = vec![b'c'; 20 * 1024];
    let mut message = format!(
        "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Transfer-Encoding: chunked\r\n\r\n{:x}\r\n",
        chunk.len()
    )
    .into_bytes();
    message.extend_from_slice(&chunk);
    message.extend_from_slice(b"\r\n0\r\n\r\n");
    stream.write_all(&message).await.unwrap();
    let mut answer = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut answer))
        .await
        .expect("the proxy answers in time")
        .expect("readable");
    let answer = String::from_utf8_lossy(&answer);
    assert!(answer.starts_with("HTTP/1.1 413 "), "{answer}");

    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0);
    let flow = first_flow(&events.seen);
    let found = diagnostics_of(&events.seen, flow);
    let limit = found
        .iter()
        .find(|diagnostic| diagnostic.code == LIMIT_001)
        .unwrap_or_else(|| panic!("LIMIT_001 at the flow, got {found:?}"));
    assert!(limit.why.contains("more than 16384 bytes"), "{}", limit.why);
}

/// Dieselbe Anfrage unter einer Regel `allow` mit `stream: true`: Sie geht
/// nach der Entscheidung über den Kopf gestreamt hinaus, vollständig, ohne
/// Halt, mitgeschnitten, und am Fluss steht, dass der Rumpf ungeprüft ging.
///
/// Beide Wege werden gefahren: mit angekündigter Länge (der Proxy liest
/// vorher nichts) und `chunked` (der Proxy hat das Cap schon gelesen und
/// muss es vor den Rest setzen). Das Ziel antwortet mit der Zahl der Bytes,
/// die es bekam; ein verlorenes Präfix fiele daran auf.
#[tokio::test(flavor = "multi_thread")]
async fn body_cap_stream_rule_passes() {
    for chunked in [false, true] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            .body_cap(CAP)
            .rules(&rules(port, "allow", true))
            .recording(true)
            .start()
            .await;
        let mut events = proxy.events();

        let answer = if chunked {
            send_chunked(&proxy.socket, port, BIG).await
        } else {
            let mut client = proxy.client().await;
            let response = client
                .send(post(
                    &format!("http://127.0.0.1:{port}/sink"),
                    vec![b's'; BIG],
                ))
                .await;
            assert_eq!(response.status(), StatusCode::OK, "chunked: {chunked}");
            body_string(response.into_body()).await
        };
        assert_eq!(
            answer.trim(),
            BIG.to_string(),
            "the target got every byte (chunked: {chunked})"
        );

        events.wait_for("recorded").await;
        assert_eq!(upstream.hits(), 1);
        assert_eq!(events.count("held"), 0, "an allow rule does not hold");
        let flow = first_flow(&events.seen);
        let (decision, source) = decided(&events.seen);
        assert_eq!(decision, Decision::Allow);
        assert_eq!(
            source,
            DecisionSource::Rule(RULE_ID.parse().unwrap()),
            "the rule decided"
        );

        // Der Fluss ist als gestreamt erkennbar: Rumpf ohne Inhalt und als
        // gekürzt markiert, ungeprüft in der Registry, und ein `LIMIT_001` als
        // `Info` daneben.
        let request = events.received_request();
        assert!(request.body.truncated, "the body was never held");
        assert!(request.body.inline.is_none());
        let record = proxy.queue.registry().get(flow).expect("the record");
        assert!(record.findings_truncated, "nothing was scanned");
        let found = diagnostics_of(&events.seen, flow);
        let streamed = found
            .iter()
            .find(|diagnostic| diagnostic.code == LIMIT_001)
            .unwrap_or_else(|| panic!("LIMIT_001 at the flow, got {found:?}"));
        assert_eq!(streamed.severity, Severity::Info);
        assert!(streamed.why.contains(RULE_ID), "{}", streamed.why);
        assert!(streamed.fix.is_none());

        // Die Aufzeichnung hat den Rumpf, so wie er hinausging.
        let recorder = proxy.recorder.as_ref().expect("recording is on");
        recorder.flush().await;
        let detail = recorder.get_flow(flow).await.unwrap().expect("the flow");
        let sent = detail
            .messages
            .iter()
            .find(|message| message.dir == Dir::Request)
            .expect("the streamed request is recorded");
        assert_eq!(sent.body.size, BIG as u64, "chunked: {chunked}");
        assert!(!sent.body.truncated, "chunked: {chunked}");
        let bytes = recorder.read_body(&sent.body).await.unwrap();
        assert_eq!(bytes.len(), BIG);
        assert!(bytes.iter().all(|byte| *byte == b's'));
    }
}

/// `ask` mit `stream: true`: Der Kopf wird gehalten, und erst die Freigabe
/// eines Menschen lässt den Rumpf fließen. Ein Block lässt ihn nie fließen.
#[tokio::test(flavor = "multi_thread")]
async fn a_stream_rule_with_ask_holds_the_head_and_streams_only_after_allow() {
    for decision in [
        Decision::Allow,
        Decision::Block {
            reason: BlockReason::User,
            note: None,
        },
    ] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            .body_cap(CAP)
            .rules(&rules(port, "ask", true))
            .start()
            .await;
        let mut events = proxy.events();
        let _decider = proxy.decide_with(decision.clone());

        let allowed = decision == Decision::Allow;
        if allowed {
            let mut client = proxy.client().await;
            let response = client
                .send(post(
                    &format!("http://127.0.0.1:{port}/sink"),
                    vec![b'a'; BIG],
                ))
                .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                body_string(response.into_body()).await.trim(),
                BIG.to_string()
            );
        } else {
            // Von Hand und ohne den ganzen Rumpf: Der Block antwortet, ohne den
            // Rumpf zu lesen, und schließt. Ein hyper-Client, der dann noch
            // schreibt, bekommt `BrokenPipe` statt der Antwort, obwohl sie
            // längst auf der Leitung steht (dasselbe Rennen wie bei
            // `Proxy::raw_exchange`); unter Last trat es auf.
            let stream = UnixStream::connect(&proxy.socket).await.unwrap();
            let (mut read_half, mut write_half) = stream.into_split();
            let head = format!(
                "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Length: {BIG}\r\n\r\n"
            );
            write_half.write_all(head.as_bytes()).await.unwrap();
            let answer = read_head(&mut read_half).await;
            assert!(answer.starts_with("HTTP/1.1 403 "), "{answer}");
            drop(write_half);
        }
        events.wait_for("recorded").await;
        assert_eq!(events.count("held"), 1, "the head was held");
        assert_eq!(upstream.hits(), usize::from(allowed));
    }
}

/// Ein Ziel, das nach dem ersten Stück schweigt, wird nach
/// `limits.body_timeout_secs` abgeschnitten; am Fluss steht `LIMIT_004`, und
/// die Aufzeichnung vermerkt die Antwort als gekürzt.
///
/// Die Spezifikation nennt `response_idle_timeout_secs` und ein
/// `Responded{504}`. Beides ist mit HUM-120 anders entschieden
/// (`backlog/CONVENTIONS.md` 4.4): Ein Schlüssel `limits.body_timeout_secs`
/// für beide Rumpf-Richtungen, und die Kopfzeilen mit `200` sind beim
/// Abbruch längst beim Client, ein `504` wäre eine Unwahrheit über die
/// Leitung. Der Fluss bleibt beim Status des Ziels und wird als gekürzt
/// aufgezeichnet.
#[tokio::test(flavor = "multi_thread")]
async fn upstream_idle_timeout() {
    const IDLE: Duration = Duration::from_secs(1);
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .header_timeout(Duration::from_secs(3600))
        .body_timeout(IDLE)
        .passthrough()
        .recording(true)
        .start()
        .await;
    let mut events = proxy.events();

    let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
    // Das Ziel schickt ein Stück und schweigt dann 400 Sekunden.
    let request = format!(
        "POST http://127.0.0.1:{port}/api/chat?count=2&interval_ms=400000 HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\nContent-Length: 2\r\n\
         Content-Type: application/json\r\n\r\n{{}}"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut sink = Vec::new();
    let mut buf = [0u8; 4096];
    while !String::from_utf8_lossy(&sink).contains("\"index\":0") {
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf))
            .await
            .expect("the first chunk arrives")
            .unwrap();
        assert!(read > 0, "closed before the first chunk");
        sink.extend_from_slice(&buf[..read]);
    }
    let silent_since = Instant::now();
    let _ = tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut sink))
        .await
        .expect("the proxy cuts the stream long before the target speaks again");
    let silence = silent_since.elapsed();
    assert!(
        silence + Duration::from_millis(50) >= IDLE,
        "cut after {silence:?}, before the clock"
    );
    assert!(
        silence < IDLE + Duration::from_millis(1500),
        "cut after {silence:?}, the clock is limits.body_timeout_secs"
    );
    assert!(!String::from_utf8_lossy(&sink).contains("[DONE]"));

    events.wait_for("recorded").await;
    let flow = first_flow(&events.seen);
    let found = diagnostics_of(&events.seen, flow);
    let limit = found
        .iter()
        .find(|diagnostic| diagnostic.code == LIMIT_004)
        .unwrap_or_else(|| panic!("LIMIT_004 at the flow, got {found:?}"));
    assert!(
        limit.why.contains("1s (limits.body_timeout_secs)"),
        "{}",
        limit.why
    );
    let names = events.names();
    let at = |name: &str| names.iter().rposition(|seen| *seen == name);
    assert!(
        at("diagnostic") < at("recorded"),
        "the reason stands before the flow is closed: {names:?}"
    );

    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let detail = recorder.get_flow(flow).await.unwrap().expect("the flow");
    let response = detail
        .messages
        .iter()
        .find(|message| message.dir == Dir::Response)
        .expect("the answer is recorded");
    assert!(response.body.truncated, "the cut answer is marked");
}

/// Sammelt, was `tracing` schreibt.
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Ein Client schickt eine halbe Anfragezeile und schweigt: Nach
/// `limits.header_timeout_secs` ist die Verbindung zu, es gibt keinen Fluss
/// (kein Kopf, also kein `Received`), und die Protokollzeile trägt `LIMIT_005`.
///
/// Auf einer Laufzeit mit einem Faden, damit jede Aufgabe des Proxys unter
/// dem Protokoll-Abonnenten dieses Tests läuft.
#[tokio::test(flavor = "current_thread")]
async fn client_header_timeout() {
    const HEAD: Duration = Duration::from_secs(1);
    let log = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let proxy = ProxyBuilder::new().header_timeout(HEAD).start().await;
    let mut events = proxy.events();
    let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
    stream.write_all(b"GET / HTTP/1.1\r\n").await.unwrap();
    let started = Instant::now();
    let mut rest = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut rest))
        .await
        .expect("the proxy closes the connection");
    let elapsed = started.elapsed();
    assert!(
        elapsed + Duration::from_millis(50) >= HEAD,
        "closed after {elapsed:?}"
    );
    assert!(
        elapsed < HEAD + Duration::from_millis(1500),
        "closed after {elapsed:?}, the clock is limits.header_timeout_secs"
    );

    events.drain();
    assert_eq!(events.count("received"), 0, "no head, no flow");
    // Die Zeile entsteht in der Aufgabe der Verbindung, nach dem Schließen;
    // ein paar Runden der Laufzeit reichen ihr dafür.
    let give_up = Instant::now() + Duration::from_secs(5);
    let text = loop {
        tokio::task::yield_now().await;
        let text = String::from_utf8_lossy(&log.0.lock().unwrap()).into_owned();
        if text.contains("LIMIT_005") || Instant::now() > give_up {
            break text;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert!(
        text.contains("LIMIT_005"),
        "no LIMIT_005 in the log: {text}"
    );
    assert!(text.contains("timeout_secs=1"), "{text}");
}

/// Schickt `len` Bytes `s` als `chunked` an `/sink` und liest die Antwort.
async fn send_chunked(socket: &std::path::Path, port: u16, len: usize) -> String {
    let mut stream = UnixStream::connect(socket).await.unwrap();
    let head = format!(
        "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    let piece = vec![b's'; 64 * 1024];
    let mut left = len;
    while left > 0 {
        let take = left.min(piece.len());
        stream
            .write_all(format!("{take:x}\r\n").as_bytes())
            .await
            .unwrap();
        stream.write_all(&piece[..take]).await.unwrap();
        stream.write_all(b"\r\n").await.unwrap();
        left -= take;
    }
    stream.write_all(b"0\r\n\r\n").await.unwrap();
    let mut answer = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), stream.read_to_end(&mut answer))
        .await
        .expect("the answer arrives")
        .unwrap();
    let text = String::from_utf8_lossy(&answer).into_owned();
    assert!(text.starts_with("HTTP/1.1 200 "), "{text}");
    // Der Rumpf der Antwort: nach der Leerzeile, ohne `chunked`-Rahmen.
    let body = text.split("\r\n\r\n").nth(1).unwrap_or_default();
    body.lines()
        .find(|line| line.chars().all(|c| c.is_ascii_digit()) && line.len() > 3)
        .unwrap_or_default()
        .to_owned()
}

/// Unter `hold.hard_block_checksum_secrets` greift `stream: true` nicht: Keine
/// Regel hebt die harte Sperre auf (`docs/SECURITY.md`), und ein ungeprüfter
/// Strom kann sie nicht einhalten. Dieselbe Anfrage wie in
/// `body_cap_stream_rule_passes` wird geblockt, kein Byte erreicht das Ziel,
/// und `LIMIT_001` nennt den Grund und schlägt das Cap vor, nie eine Regel.
#[tokio::test(flavor = "multi_thread")]
async fn the_hard_block_overrides_a_stream_rule() {
    for chunked in [false, true] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            .body_cap(CAP)
            .rules(&rules(port, "allow", true))
            .hard_block_checksum_secrets(true)
            .start()
            .await;
        let mut events = proxy.events();

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        // Kopf und ein Stück über dem Cap, ohne den Rest: Der Proxy darf nicht
        // weiterlesen, und ein Client, der alles schriebe, bekäme eine
        // gebrochene Leitung statt der Antwort.
        let body = vec![b'h'; usize::try_from(CAP).unwrap() + 1024];
        let mut message = if chunked {
            format!(
                "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Transfer-Encoding: chunked\r\n\r\n{:x}\r\n",
                body.len()
            )
        } else {
            format!(
                "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Length: {BIG}\r\n\r\n"
            )
        }
        .into_bytes();
        if chunked {
            message.extend_from_slice(&body);
            message.extend_from_slice(b"\r\n");
        }
        stream.write_all(&message).await.unwrap();
        let mut answer = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut answer))
            .await
            .expect("the proxy answers in time")
            .expect("readable");
        let answer = String::from_utf8_lossy(&answer);
        assert!(
            answer.starts_with("HTTP/1.1 413 "),
            "chunked: {chunked}: {answer}"
        );

        events.wait_for("recorded").await;
        assert_eq!(
            upstream.hits(),
            0,
            "chunked: {chunked}: not a byte at the target"
        );
        assert_eq!(events.count("forwarded"), 0);
        let flow = first_flow(&events.seen);
        assert_eq!(
            decided(&events.seen).0,
            Decision::Block {
                reason: BlockReason::BodyCap,
                note: None
            }
        );
        let found = diagnostics_of(&events.seen, flow);
        let limit = found
            .iter()
            .find(|diagnostic| diagnostic.code == LIMIT_001)
            .unwrap_or_else(|| panic!("LIMIT_001 at the flow, got {found:?}"));
        assert_eq!(limit.severity, Severity::Warning);
        assert!(
            limit.why.contains("hold.hard_block_checksum_secrets"),
            "{}",
            limit.why
        );
        assert!(
            matches!(limit.fix, Some(FixAction::ChangeSetting { ref key, .. }) if key == "limits.hold_body_cap_bytes"),
            "the fix is the cap, never a stream rule: {:?}",
            limit.fix
        );
    }
}

/// Ein Regelsatz für `127.0.0.1:port` mit einer Regel der Kennung `id`.
fn rule_set(id: &str, port: u16, stream: bool) -> humanitl_rules::RuleSet {
    let yaml = format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {id}\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"ip:127.0.0.1\"\n\
         \x20     port: {port}\n\
         \x20   stream: {stream}\n"
    );
    humanitl_rules::parse_rules(&yaml)
        .expect("the rules parse")
        .0
}

/// Die Erlaubnis zu streamen und die Entscheidung kommen aus **einem**
/// Schnappschuss der Regeln (Review zu HUM-057): Lädt ein anderer Task
/// zwischen `stream_grant` und der Entscheidung einen Satz, in dem eine
/// andere Regel ohne `stream` trifft, entscheidet trotzdem die Regel, die das
/// Streamen erlaubt hat, und keine, die es nie erlaubt hätte.
///
/// Ohne Zeitglück: Der Regelsatz wird zwischen beiden Aufrufen von Hand
/// ausgetauscht, genau an der Stelle, an der es ein Neuladen täte.
#[tokio::test]
async fn the_stream_grant_and_the_decision_come_from_one_snapshot() {
    use std::sync::RwLock;

    use humanitl_config::Limits;
    use humanitl_core::{Authority, Flow, HostName, HttpRequest, Method, Scheme, SessionId};
    use humanitl_proxy::pipeline::ConnectionContext;
    use humanitl_proxy::{FlowPipeline, HoldQueue, PassthroughPipeline, RulesPipeline};

    const STREAMING: &str = "01920000-0000-7000-8000-0000000000a1";
    const RELOADED: &str = "01920000-0000-7000-8000-0000000000b2";
    let queue = Arc::new(HoldQueue::new(&Limits::default()));
    let rules = Arc::new(RwLock::new(rule_set(STREAMING, 8080, true)));
    let inner: Arc<dyn FlowPipeline> = Arc::new(PassthroughPipeline::new(Arc::clone(&queue)));
    let pipeline = RulesPipeline::new(Arc::clone(&queue), Arc::clone(&rules), inner);

    let session = SessionId::new();
    let request = HttpRequest::new(
        Method::POST,
        Scheme::Http,
        Authority::new(HostName::Ip([127, 0, 0, 1].into()), 8080),
        "/upload",
    );
    let mut flow = Flow::new(
        humanitl_core::FlowId::new(),
        session,
        std::time::SystemTime::now(),
        request,
    );
    flow.apply(
        humanitl_core::TransitionInput::Analyze { findings: vec![] },
        std::time::SystemTime::now(),
    )
    .unwrap();

    let grant = pipeline
        .stream_grant(&flow)
        .expect("the streaming rule grants it");
    assert_eq!(grant.rule(), STREAMING.parse().unwrap());

    // Das Neuladen dazwischen: Jetzt träfe eine andere Regel, ohne `stream`.
    *rules.write().unwrap() = rule_set(RELOADED, 8080, false);
    assert!(
        pipeline.stream_grant(&flow).is_none(),
        "the reloaded set grants no streaming"
    );

    let mut rx = queue.subscribe();
    let decision = pipeline
        .decide_streamed(&mut flow, &ConnectionContext::plain(session), grant)
        .await;
    assert_eq!(decision, Decision::Allow);
    let source = std::iter::from_fn(|| rx.try_recv().ok())
        .find_map(|event| match event {
            FlowEvent::Decided { source, .. } => Some(source),
            _ => None,
        })
        .expect("a Decided event");
    assert_eq!(
        source,
        DecisionSource::Rule(STREAMING.parse().unwrap()),
        "the rule that granted the stream decides, not the one loaded afterwards"
    );
}

/// `ask` mit `stream: true` und eine angekündigte Größe über
/// `limits.hold_max_bytes`: Der Kopf wird gehalten, denn er reserviert nur die
/// Bytes, die wirklich im Speicher liegen, und die Freigabe streamt den Rumpf
/// (Review zu HUM-057). Vorher endete das sofort in `HoldMemory`, und der
/// Vorschlag aus `LIMIT_001` bewirkte nichts.
///
/// Verkleinert wie die übrigen Tests dieser Datei: 1 MiB angekündigt gegen ein
/// Budget von 512 KiB statt 300 MiB gegen 256 MiB.
#[tokio::test(flavor = "multi_thread")]
async fn a_held_head_reserves_only_what_is_in_memory() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .body_cap(CAP)
        .hold_max_bytes(512 * 1024)
        .rules(&rules(port, "ask", true))
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{port}/sink"),
            vec![b'a'; BIG],
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_string(response.into_body()).await.trim(),
        BIG.to_string()
    );
    events.wait_for("recorded").await;
    let held = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Held { queue_bytes, .. } => Some(*queue_bytes),
            _ => None,
        })
        .expect("the head was held");
    assert!(held < CAP, "the held head reserved {held} bytes");
    assert_eq!(proxy.queue.queue_bytes(), 0, "everything is released");
}

/// Ein Ziel auf `127.0.0.1`, das annimmt und sich dann so verhält, wie
/// `behave` es sagt. Liefert den Port.
async fn silent_target(read_everything: bool) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                if read_everything {
                    // Liest alles, was kommt, und antwortet nie.
                    let mut sink = vec![0u8; 64 * 1024];
                    while socket.read(&mut sink).await.is_ok_and(|n| n > 0) {}
                }
                // Hält die Verbindung offen, ohne zu lesen oder zu antworten.
                tokio::time::sleep(Duration::from_secs(600)).await;
                drop(socket);
            });
        }
    });
    port
}

/// Ein gestreamter Upload, der beim Ziel hängen bleibt, endet nach der
/// passenden Frist mit `LIMIT_004` (Review zu HUM-057): Das Ziel nimmt den
/// ganzen Rumpf und antwortet nie (`limits.header_timeout_secs`), oder es
/// liest nicht mehr (`limits.body_timeout_secs`). Vorher hing der Flow
/// unbegrenzt.
#[tokio::test(flavor = "multi_thread")]
async fn a_stalled_streamed_upload_ends_with_limit_004() {
    const CLOCK: Duration = Duration::from_secs(1);
    const LONG: Duration = Duration::from_secs(60);
    for (reads, limit, size) in [
        (true, "limits.header_timeout_secs", BIG),
        (false, "limits.body_timeout_secs", 48 * 1024 * 1024),
    ] {
        let port = silent_target(reads).await;
        let (head, idle) = if reads { (CLOCK, LONG) } else { (LONG, CLOCK) };
        let proxy = ProxyBuilder::new()
            .body_cap(CAP)
            .header_timeout(head)
            .body_timeout(idle)
            .rules(&rules(port, "allow", true))
            .start()
            .await;
        let mut events = proxy.events();

        let stream = UnixStream::connect(&proxy.socket).await.unwrap();
        let (mut read_half, mut write_half) = stream.into_split();
        let head_line = format!(
            "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
             Content-Length: {size}\r\n\r\n"
        );
        // Geschrieben wird nebenher: Liest das Ziel nicht, bleibt das
        // Schreiben irgendwann stehen, und gelesen werden muss trotzdem. Die
        // Schreibhälfte bleibt offen: Fiele sie weg, sähe der Proxy ein
        // halbes Schließen des Clients und gäbe die Verbindung auf, bevor das
        // Ziel überhaupt hängen kann.
        let (keep, mut parked) = tokio::sync::mpsc::channel::<tokio::net::unix::OwnedWriteHalf>(1);
        let writer = tokio::spawn(async move {
            let _ = write_half.write_all(head_line.as_bytes()).await;
            let piece = vec![b'u'; 64 * 1024];
            let mut left = size;
            while left > 0 {
                let take = left.min(piece.len());
                if write_half.write_all(&piece[..take]).await.is_err() {
                    break;
                }
                left -= take;
            }
            let _ = keep.send(write_half).await;
        });
        let mut answer = Vec::new();
        tokio::time::timeout(Duration::from_secs(20), read_half.read_to_end(&mut answer))
            .await
            .unwrap_or_else(|_| panic!("reads={reads}: the proxy gives up on the target"))
            .expect("readable");
        writer.abort();
        drop(parked.try_recv());
        let answer = String::from_utf8_lossy(&answer);
        assert!(
            answer.starts_with("HTTP/1.1 502 "),
            "reads={reads}: {answer}"
        );

        events.wait_for("recorded").await;
        let flow = first_flow(&events.seen);
        let found = diagnostics_of(&events.seen, flow);
        let stalled = found
            .iter()
            .find(|diagnostic| diagnostic.code == LIMIT_004)
            .unwrap_or_else(|| panic!("reads={reads}: LIMIT_004 at the flow, got {found:?}"));
        assert!(stalled.why.contains(limit), "{}", stalled.why);
    }
}

/// Das Byte-Budget gilt schon beim Puffern (Review zu HUM-057): Drei Clients
/// halten je 300 KiB im Puffer an, ein vierter passt nicht mehr ins Budget von
/// 1 MiB und bekommt `503` mit `LIMIT_002`. Brechen die drei ab, ist das
/// Budget wieder leer.
#[tokio::test(flavor = "multi_thread")]
async fn the_byte_budget_counts_bodies_while_they_are_buffered() {
    const PART: usize = 300 * 1024;
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .body_cap(512 * 1024)
        .hold_max_bytes(1024 * 1024)
        .rules(&rules(port, "allow", false))
        .start()
        .await;
    let mut events = proxy.events();
    let head = format!(
        "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Length: {}\r\n\r\n",
        400 * 1024
    );

    let mut paused = Vec::new();
    for count in 1..=3u64 {
        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        stream.write_all(head.as_bytes()).await.unwrap();
        stream.write_all(&vec![b'p'; PART]).await.unwrap();
        paused.push(stream);
        let give_up = Instant::now() + Duration::from_secs(10);
        while proxy.queue.queue_bytes() < count * PART as u64 {
            assert!(
                Instant::now() < give_up,
                "{} buffered",
                proxy.queue.queue_bytes()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let mut fourth = UnixStream::connect(&proxy.socket).await.unwrap();
    fourth.write_all(head.as_bytes()).await.unwrap();
    // Der Proxy lehnt ab, sobald das Budget erschöpft ist, liest nicht weiter
    // und schließt; das Schreiben des Rests darf deshalb scheitern. Die
    // Antwort steht trotzdem auf der Leitung (unter Last beobachtet).
    let _ = fourth.write_all(&vec![b'p'; PART]).await;
    let mut answer = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), fourth.read_to_end(&mut answer))
        .await
        .expect("the fourth is refused in time")
        .expect("readable");
    let answer = String::from_utf8_lossy(&answer);
    assert!(answer.starts_with("HTTP/1.1 503 "), "{answer}");
    events.wait_for("recorded").await;
    let flow = first_flow(&events.seen);
    let found = diagnostics_of(&events.seen, flow);
    let full = found
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "LIMIT_002")
        .unwrap_or_else(|| panic!("LIMIT_002 at the flow, got {found:?}"));
    assert!(full.why.contains("limits.hold_max_bytes"), "{}", full.why);
    assert_eq!(upstream.hits(), 0);

    drop(paused);
    let give_up = Instant::now() + Duration::from_secs(10);
    while proxy.queue.queue_bytes() != 0 {
        assert!(
            Instant::now() < give_up,
            "{} bytes still reserved after the clients left",
            proxy.queue.queue_bytes()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Kopf, Query und Kopfzeilen eines gestreamten Flusses werden gescannt; nur
/// der Rumpf bleibt ungeprüft (Review zu HUM-057).
#[tokio::test(flavor = "multi_thread")]
async fn the_head_of_a_streamed_flow_is_scanned() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let scanner: Arc<dyn humanitl_proxy::Scanner> = Arc::new(
        humanitl_proxy::Tier1Scanner::new(&humanitl_findings::FindingsSettings::default()).unwrap(),
    );
    let proxy = ProxyBuilder::new()
        .body_cap(CAP)
        .scanner(scanner)
        .rules(&rules(port, "allow", true))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let mut request = post(&format!("http://127.0.0.1:{port}/sink"), vec![b's'; BIG]);
    request.headers_mut().insert(
        "x-token",
        "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8".parse().unwrap(),
    );
    let response = client.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    events.wait_for("recorded").await;
    let findings = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Analyzed { findings, .. } => Some(findings.clone()),
            _ => None,
        })
        .expect("an Analyzed event");
    assert!(
        !findings.is_empty(),
        "the token in the header is a finding of the streamed flow"
    );
}

/// Schickt den Kopf `head` und danach `size` Bytes in Stücken als
/// `chunked`-Rumpf, nebenher; die Schreibhälfte bleibt offen, bis der Test sie
/// über den zurückgegebenen Empfänger fallen lässt.
fn chunked_writer(
    mut write_half: tokio::net::unix::OwnedWriteHalf,
    head: String,
    size: usize,
) -> tokio::sync::mpsc::Receiver<tokio::net::unix::OwnedWriteHalf> {
    let (keep, parked) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let _ = write_half.write_all(head.as_bytes()).await;
        let piece = vec![b'c'; 64 * 1024];
        let mut left = size;
        while left > 0 {
            let take = left.min(piece.len());
            let framed = [format!("{take:x}\r\n").as_bytes(), &piece[..take], b"\r\n"].concat();
            if write_half.write_all(&framed).await.is_err() {
                break;
            }
            left -= take;
        }
        let _ = write_half.write_all(b"0\r\n\r\n").await;
        let _ = keep.send(write_half).await;
    });
    parked
}

/// Liest, bis die Antwort-Kopfzeilen da sind, und gibt sie zurück.
async fn read_head(read_half: &mut tokio::net::unix::OwnedReadHalf) -> String {
    let mut seen = Vec::new();
    let mut buf = [0u8; 4096];
    while !String::from_utf8_lossy(&seen).contains("\r\n\r\n") {
        let read = tokio::time::timeout(Duration::from_secs(20), read_half.read(&mut buf))
            .await
            .expect("the answer arrives")
            .expect("readable");
        assert!(
            read > 0,
            "closed before the answer: {:?}",
            String::from_utf8_lossy(&seen)
        );
        seen.extend_from_slice(&buf[..read]);
    }
    String::from_utf8_lossy(&seen).into_owned()
}

/// Antwortet das Ziel, bevor es den Upload gelesen hat, und hört dann auf zu
/// lesen, bleibt der Rumpf gezählt, solange er noch im Speicher liegt, und die
/// Verbindung wird nach `limits.body_timeout_secs` abgebrochen; danach ist das
/// Budget frei (Review 2 zu HUM-057).
#[tokio::test(flavor = "multi_thread")]
async fn an_early_answer_keeps_the_upload_counted_and_watched() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                // Erst den Kopf der Anfrage lesen, dann antworten und nie
                // wieder lesen: die frühe Antwort eines echten Servers, der
                // den Upload ablehnt oder nicht braucht. Eine Antwort, bevor
                // überhaupt eine Anfrage auf der Leitung war, ist keine
                // Antwort auf diese Anfrage; hyper verwirft sie als
                // `UnexpectedMessage`, und der Proxy antwortet zu Recht `502`.
                // Genau das traf die CI, wenn das Ziel unter Last schneller
                // schrieb als der Proxy den Kopf.
                let mut seen = Vec::new();
                let mut buf = [0u8; 4096];
                while !seen.windows(4).any(|window| window == b"\r\n\r\n") {
                    match socket.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => seen.extend_from_slice(&buf[..read]),
                    }
                }
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .await;
                tokio::time::sleep(Duration::from_secs(600)).await;
                drop(socket);
            });
        }
    });
    let proxy = ProxyBuilder::new()
        .body_cap(CAP)
        .body_timeout(Duration::from_secs(1))
        .rules(&rules(port, "allow", true))
        .start()
        .await;
    let mut events = proxy.events();

    let stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let (mut read_half, write_half) = stream.into_split();
    let head = format!(
        "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Transfer-Encoding: chunked\r\n\r\n"
    );
    let parked = chunked_writer(write_half, head, 48 * 1024 * 1024);
    let answer = read_head(&mut read_half).await;
    assert!(answer.starts_with("HTTP/1.1 200 "), "{answer}");
    assert!(
        proxy.queue.queue_bytes() > 0,
        "the unfinished upload is still in memory and must stay counted"
    );

    let give_up = Instant::now() + Duration::from_secs(10);
    while proxy.queue.queue_bytes() != 0 {
        assert!(
            Instant::now() < give_up,
            "the stalled upload still holds {} bytes",
            proxy.queue.queue_bytes()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Und der Abbruch nach der Antwort steht als `LIMIT_004` am Fluss
    // (Review 3 zu HUM-057).
    let stalled = loop {
        if let FlowEvent::Diagnostic {
            diagnostic,
            flow_id: Some(_),
            ..
        } = events.wait_for("diagnostic").await
            && diagnostic.code == LIMIT_004
        {
            break diagnostic;
        }
    };
    assert!(
        stalled.why.contains("limits.body_timeout_secs"),
        "{}",
        stalled.why
    );
    drop(parked);
}

/// Ein großes Präfix über eine langsame Leitung ist kein Stillstand: Es geht
/// in Scheiben hinaus, jede Scheibe ist Fortschritt, und ein gesunder Upload
/// läuft durch, auch wenn er viel länger dauert als `limits.body_timeout_secs`
/// (Review 2 zu HUM-057).
#[tokio::test(flavor = "multi_thread")]
async fn a_large_prefix_over_a_slow_line_is_not_a_stall() {
    // Groß genug, dass ein Präfix als ein einziges Stück länger als die Frist
    // in Puffern und auf der Leitung hinge; das Ziel liest gut 2 MiB je
    // Sekunde, schneller als hyper seinen eigenen Schreibpuffer je Frist leert.
    const PREFIX_CAP: u64 = 16 * 1024 * 1024;
    const TOTAL: usize = 17 * 1024 * 1024;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                // Gedrosselt: höchstens 128 KiB alle 50 ms.
                let mut buf = vec![0u8; 128 * 1024];
                let mut total = 0usize;
                let mut tail = Vec::new();
                loop {
                    let Ok(read) = socket.read(&mut buf).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    total += read;
                    tail.extend_from_slice(&buf[..read]);
                    let keep = tail.len().saturating_sub(8);
                    tail.drain(..keep);
                    if total > TOTAL && tail.ends_with(b"0\r\n\r\n") {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndone")
                    .await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            });
        }
    });
    let proxy = ProxyBuilder::new()
        .body_cap(PREFIX_CAP)
        .hold_max_bytes(64 * 1024 * 1024)
        .body_timeout(Duration::from_secs(1))
        .rules(&rules(port, "allow", true))
        .start()
        .await;

    let stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let (mut read_half, write_half) = stream.into_split();
    let head = format!(
        "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Transfer-Encoding: chunked\r\n\r\n"
    );
    let parked = chunked_writer(write_half, head, TOTAL);
    let answer = read_head(&mut read_half).await;
    assert!(
        answer.starts_with("HTTP/1.1 200 "),
        "the slow upload went through: {answer}"
    );
    drop(parked);
}

/// Schweigt der Client mitten in einem gestreamten Upload, ist das kein Fehler
/// des Ziels: `408` und `PROXY_011` am Fluss, wie `docs/SECURITY.md` es für
/// einen stehenden Anfrage-Rumpf zusagt (Review 2 zu HUM-057).
#[tokio::test(flavor = "multi_thread")]
async fn a_silent_client_in_a_streamed_upload_gets_408() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .body_cap(CAP)
        .body_timeout(Duration::from_secs(1))
        .rules(&rules(port, "allow", true))
        .start()
        .await;
    let mut events = proxy.events();

    let stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let (mut read_half, mut write_half) = stream.into_split();
    let head = format!(
        "POST http://127.0.0.1:{port}/sink HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Length: {BIG}\r\n\r\n"
    );
    write_half.write_all(head.as_bytes()).await.unwrap();
    write_half.write_all(&vec![b's'; 300 * 1024]).await.unwrap();
    // Und dann nichts mehr; die Schreibhälfte bleibt offen.
    let answer = read_head(&mut read_half).await;
    assert!(answer.starts_with("HTTP/1.1 408 "), "{answer}");

    events.wait_for("recorded").await;
    let flow = first_flow(&events.seen);
    let found = diagnostics_of(&events.seen, flow);
    let idle = found
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "PROXY_011")
        .unwrap_or_else(|| panic!("PROXY_011 at the flow, got {found:?}"));
    // Der Satz sagt, dass schon ein Teil beim Ziel war, und der Fluss trägt
    // die `408`, die der Client bekam, keinen Fehler des Ziels (Review 3).
    assert!(
        idle.why.contains("already gone to the target"),
        "{}",
        idle.why
    );
    assert_eq!(events.statuses(), vec![408]);
    assert_eq!(events.count("failed"), 0, "{:?}", events.names());
    drop(write_half);
}
