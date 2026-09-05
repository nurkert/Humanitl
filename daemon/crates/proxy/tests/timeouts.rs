//! Die eine Uhr auf der Verbindung zum Agenten (HUM-101).
//!
//! Vor diesem Issue standen zwei Schlüssel für dieselbe Spanne im Schema:
//! `limits.header_timeout_secs` (Vorgabe 30) und `limits.idle_timeout_secs`
//! (Vorgabe 90). Der zweite hatte keinen Leser, und er hätte auch keinen
//! bekommen dürfen, ohne dass zugleich der Hold von ihm ausgenommen wird: Der
//! Hold sitzt in der Service-Future innerhalb `serve_connection`, und solange
//! er läuft, fließen auf der Verbindung des Agenten null Bytes. Eine
//! Leerlaufuhr mit 90 Sekunden hätte deshalb jeden gehaltenen Fluss getötet,
//! dessen Frist 300 Sekunden beträgt. Entfernt wurde der zweite Schlüssel;
//! übrig bleibt einer, und diese Datei misst, dass er die richtige Spanne
//! trifft:
//!
//! 1. Eine frische Verbindung, die nie eine Anfrage schickt, wird geschlossen.
//! 2. Eine Keep-Alive-Verbindung, die nach ihrer Antwort schweigt, ebenso —
//!    das ist die Lücke bis zur nächsten Anfrage, die zweite Hälfte derselben
//!    Uhr.
//! 3. Eine Verbindung, die schweigt, weil ihre Anfrage gehalten wird, nicht.
//! 4. Eine Verbindung, die schweigt, weil das Sprachmodell gerade streamt,
//!    auch nicht.
//!
//! Jeder Fall misst **zwei** Schranken, eine untere und eine obere, und die
//! Fälle 1 und 2 laufen mit zwei verschiedenen konfigurierten Werten. Eine
//! Zusicherung ohne obere Schranke belegt nur, dass irgendeine Uhr läuft; erst
//! die obere bindet sie an `limits.header_timeout_secs`. Eine fest verdrahtete
//! Frist von fünf oder zehn Sekunden fällt damit durch, auch wenn sie den
//! kurzen Fall zufällig überlebt.
//!
//! Und ohne die Fälle 1 und 2 belegen die anderen nichts: Eine Uhr, die
//! niemals abläuft, verschont jeden Hold.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::time::{Duration, Instant};

use humanitl_core::{BlockReason, Decision, FlowEvent};
use support::{FakeUpstream, ProxyBuilder};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;

/// Die Uhr der meisten Fälle. Sekunden sind die kleinste Einheit der
/// Einstellung.
const CLOCK: Duration = Duration::from_secs(1);

/// Der zweite konfigurierte Wert. Zwei Läufe mit verschiedenen Werten trennen
/// „die Uhr folgt der Konfiguration" von „irgendeine Uhr läuft".
const SLOW_CLOCK: Duration = Duration::from_secs(3);

/// Zugabe für Planung und Aufbau. Sie muss kleiner sein als der Abstand der
/// beiden Werte, sonst deckt das Fenster des einen den anderen mit ab.
const SLACK: Duration = Duration::from_millis(1500);

/// Der Regelsatz einer Sitzung mit einem lokalen Modell auf `127.0.0.1:PORT`,
/// wie ihn der OpenCode-Adapter baut (siehe `passthrough.rs`).
fn passthrough_rules(port: u16) -> String {
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
         \x20     path_prefixes: [\"/api/chat\"]\n\
         \x20   allow_private: true\n\
         \x20   passthrough_llm: true\n\
         \x20   note: \"LLM passthrough. Logged, never held.\"\n"
    )
}

/// Was die Uhr zu früh sein darf. Der Zeitgeber rundet auf seine eigene
/// Auflösung ab; gemessen wurden 998,5 Millisekunden für eine Sekunde.
const EARLY: Duration = Duration::from_millis(50);

/// Prüft, dass die Verbindung genau nach der konfigurierten Frist endete.
///
/// `idle` ist die Zeit, in der auf der Verbindung nichts mehr geschah; die Uhr
/// läuft genau darin. Die obere Schranke ist der Punkt: Ohne sie bestünde der
/// Test auch gegen eine fest verdrahtete Frist, die den Wert aus der
/// Konfiguration nie liest.
fn assert_closed_by_the_clock(idle: Duration, clock: Duration) {
    assert!(
        idle + EARLY >= clock,
        "the connection ended after {idle:?}, before its own timeout of {clock:?}"
    );
    assert!(
        idle < clock + SLACK,
        "the connection ended after {idle:?}; that is more than the configured {clock:?} plus \
         slack, so the clock does not come from limits.header_timeout_secs"
    );
}

/// Liest, bis `marker` im Empfangenen steht, und liefert die Zeit dafür.
async fn read_until(stream: &mut UnixStream, sink: &mut Vec<u8>, marker: &str) -> Duration {
    let started = Instant::now();
    let mut chunk = [0u8; 4096];
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "no {marker:?} within 20s; got {:?}",
            String::from_utf8_lossy(sink)
        );
        let read = stream.read(&mut chunk).await.expect("the connection reads");
        assert!(read > 0, "the connection ended before {marker:?}");
        sink.extend_from_slice(&chunk[..read]);
        if String::from_utf8_lossy(sink).contains(marker) {
            return started.elapsed();
        }
    }
}

/// Liest bis zum Ende der Verbindung und liefert die Zeit dafür.
async fn read_to_the_end(stream: &mut UnixStream, sink: &mut Vec<u8>) -> Duration {
    let started = Instant::now();
    tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(sink))
        .await
        .expect("the proxy closes the connection on its own")
        .expect("reading until the end of the connection works");
    started.elapsed()
}

/// Die Anfragezeile samt `Host`, von Hand: Diese Datei will die Verbindung
/// nach der Antwort in der Hand behalten und ihr Ende sehen.
fn raw_get(port: u16, path: &str) -> String {
    format!("GET http://127.0.0.1:{port}{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")
}

/// Eine Verbindung ohne Anfrage überlebt die Frist nicht — und zwar nach genau
/// der Frist, die konfiguriert ist.
///
/// Der Gegenbeweis zu den beiden Überlebens-Tests: Die Uhr läuft wirklich, und
/// sie läuft nach der Konfiguration.
#[tokio::test(flavor = "multi_thread")]
async fn an_idle_connection_does_not_survive_the_header_timeout() {
    for clock in [CLOCK, SLOW_CLOCK] {
        let proxy = ProxyBuilder::new().header_timeout(clock).start().await;
        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        let mut sink = Vec::new();
        // Kein Byte wird gesendet. Der Proxy schließt von sich aus; was er
        // dabei noch schreibt, ist gleichgültig, das Ende der Leitung ist die
        // Aussage.
        let idle = read_to_the_end(&mut stream, &mut sink).await;
        assert_closed_by_the_clock(idle, clock);
    }
}

/// Die Lücke bis zur nächsten Anfrage überlebt sie ebenso wenig.
///
/// Der erste Test misst die frische Verbindung, dieser die Keep-Alive-Lücke:
/// Erst läuft eine vollständige Anfrage samt Antwort durch, dann schweigt der
/// Client. Beide Hälften gehören derselben Uhr, und ohne diesen Fall stünde für
/// die zweite nur ein Verweis auf hypers Quelltext.
#[tokio::test(flavor = "multi_thread")]
async fn a_keep_alive_connection_does_not_survive_the_header_timeout() {
    for clock in [CLOCK, SLOW_CLOCK] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            .header_timeout(clock)
            .passthrough()
            .start()
            .await;

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        stream
            .write_all(raw_get(port, "/echo").as_bytes())
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut sink = Vec::new();
        read_until(&mut stream, &mut sink, support::ECHO_BODY).await;
        let idle = read_to_the_end(&mut stream, &mut sink).await;

        let text = String::from_utf8_lossy(&sink);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(
            !text.to_ascii_lowercase().contains("connection: close"),
            "the answer must leave the connection open, or the test measures the wrong thing: \
             {text}"
        );
        assert_closed_by_the_clock(idle, clock);
        assert_eq!(upstream.hits(), 1);
    }
}

/// Eine gehaltene Anfrage überlebt sie, auch wenn sie ein Vielfaches der Frist
/// wartet — und danach schließt dieselbe Verbindung nach genau der Frist.
///
/// Die zweite Hälfte ist der Grund, warum dieser Test mehr belegt als „keine
/// Uhr unter drei Sekunden": Sie zeigt, dass auf genau dieser Verbindung eine
/// Uhr wacht und dass sie die konfigurierte ist. Eine fest verdrahtete Frist
/// von zehn Sekunden, die jeden Hold über zehn Sekunden tötete, fiele hier
/// durch, obwohl sie den Hold von drei Sekunden überlebt.
///
/// Der Test wird ebenso rot, sobald jemand eine Leerlaufuhr auf die Verbindung
/// des Agenten legt, ohne den Hold von ihr auszunehmen. Genau das ist der
/// Grund, warum `limits.idle_timeout_secs` nicht eingebaut, sondern entfernt
/// wurde.
#[tokio::test(flavor = "multi_thread")]
async fn a_held_request_survives_the_header_timeout() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .header_timeout(CLOCK)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
    stream
        .write_all(raw_get(port, "/echo").as_bytes())
        .await
        .unwrap();
    stream.flush().await.unwrap();

    let FlowEvent::Held { flow_id, .. } = events.wait_for("held").await else {
        panic!("the queue must hold the request");
    };
    // Dreimal die Frist ohne ein Byte in beide Richtungen.
    let waiting = CLOCK * 3;
    tokio::time::sleep(waiting).await;
    assert_eq!(upstream.hits(), 0, "nothing may leave before the decision");
    proxy
        .queue
        .decide(flow_id, Decision::Allow)
        .expect("the flow is still held");

    let mut sink = Vec::new();
    let answered = read_until(&mut stream, &mut sink, support::ECHO_BODY).await;
    let idle = read_to_the_end(&mut stream, &mut sink).await;

    let text = String::from_utf8_lossy(&sink);
    assert!(
        text.starts_with("HTTP/1.1 200 OK"),
        "a decision after three times the header timeout must still reach the client: {text}"
    );
    assert!(
        answered < SLACK,
        "the answer took {answered:?} after the decision; that is not the decision's doing"
    );
    assert_closed_by_the_clock(idle, CLOCK);
    assert_eq!(
        upstream.hits(),
        1,
        "the request goes out after the decision"
    );
}

/// Eine Durchreiche zum Sprachmodell, die lange streamt, überlebt sie
/// ebenfalls — und danach schließt dieselbe Verbindung nach genau der Frist.
///
/// Das Modell antwortet in fünf Stücken mit je 600 Millisekunden Abstand, also
/// über mehr als das Doppelte der Frist. Auf der Verbindung des Agenten kommt
/// in dieser Zeit kein Byte an; nur hinaus fließt etwas. Die zweite Hälfte
/// bindet auch hier die Uhr an die Konfiguration, statt nur „keine Uhr unter
/// drei Sekunden" zu belegen.
#[tokio::test(flavor = "multi_thread")]
async fn a_streaming_passthrough_survives_the_header_timeout() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules(&passthrough_rules(port))
        // Die Verbindung selbst darf keine privaten Ziele: Was durchkommt,
        // kommt allein wegen `allow_private` an der Regel durch.
        .allow_private(false)
        .header_timeout(CLOCK)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    // Alles, was die Durchreiche nicht trifft, wird geblockt; ein `200` kann
    // deshalb nur aus der Durchreiche kommen.
    let _decider = proxy.decide_with(Decision::Block {
        reason: BlockReason::User,
        note: None,
    });

    let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let path = "/api/chat?count=5&interval_ms=600";
    let request = format!(
        "POST http://127.0.0.1:{port}{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Length: 2\r\nContent-Type: application/json\r\n\r\n{{}}"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();

    let mut sink = Vec::new();
    let streaming = read_until(&mut stream, &mut sink, "[DONE]").await;
    let idle = read_to_the_end(&mut stream, &mut sink).await;

    let text = String::from_utf8_lossy(&sink);
    assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
    assert!(
        streaming > CLOCK * 2,
        "the stream was faster than twice the timeout, so the test proves nothing: {streaming:?}"
    );
    assert_eq!(text.matches("data: {").count(), 5, "{text}");
    assert_closed_by_the_clock(idle, CLOCK);
}

/// Der Client hört mitten in der Anfrage auf — und niemand schließt die
/// Verbindung.
///
/// Kein Regressionstest, sondern ein festgehaltener Zustand: Hypers Kopf-Uhr
/// ist gelöscht, sobald der Kopf geparst ist, und um `body::buffer` liegt keine
/// Frist. Die Verbindung bleibt offen, und mit ihr eine Aufgabe und ein
/// Dateideskriptor. HUM-120 schließt diese Spanne; dann wird dieser Test rot
/// und beschreibt das neue Verhalten (`backlog/CONVENTIONS.md` 4.25,
/// `docs/SECURITY.md`).
#[tokio::test(flavor = "multi_thread")]
async fn a_half_sent_request_body_is_not_watched_by_any_clock() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .header_timeout(CLOCK)
        .passthrough()
        .start()
        .await;

    let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
    // Der Kopf ist vollständig, der Rumpf kündigt 1000 Bytes an und bleibt bei
    // zehn stehen.
    let head = format!(
        "POST http://127.0.0.1:{port}/echo HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Length: 1000\r\n\r\n0123456789"
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();

    let quiet = CLOCK * 4;
    let outcome = tokio::time::timeout(quiet, stream.read(&mut [0u8; 64])).await;
    assert!(
        outcome.is_err(),
        "the connection ended after less than {quiet:?}; if HUM-120 built the clock for the \
         request body, this test describes the old state and has to be rewritten"
    );
    assert_eq!(
        upstream.hits(),
        0,
        "nothing may leave while the body is incomplete"
    );
}
