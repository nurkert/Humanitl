//! Die Uhren auf der Verbindung zum Agenten (HUM-101, HUM-120).
//!
//! Der erste Teil dieser Datei gehört HUM-101 und der einen Leerlaufuhr, der
//! zweite HUM-120 und den drei Spannen, die bis dahin ohne Uhr liefen — dazu
//! der Obergrenze für gleichzeitige Verbindungen, ohne die eine Uhr je Spanne
//! nur die halbe Arbeit tut.
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

use bytes::Bytes;
use http_body_util::Full;
use humanitl_core::{BlockReason, Decision, FlowEvent};
use humanitl_recorder::{Dir, FlowQuery};
use hyper::{Request, StatusCode};
use support::{FakeUpstream, ProxyBuilder};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
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

// ---------------------------------------------------------------------------
// Die drei Spannen der Verbindung (HUM-120)
// ---------------------------------------------------------------------------
//
// Auch dieser Teil misst an der Wand, und das ist eine Messung und keine
// Bequemlichkeit. `#[tokio::test(start_paused = true)]` wurde versucht und ist
// hier untauglich: Die gestellte Uhr springt weiter, sobald der Ablauf nichts
// zu rechnen hat, und das schließt das Warten auf einen echten Socket ein. Ein
// Spike mit genau diesen Fällen ergab für eine konfigurierte Frist von einer
// Sekunde eine gemessene Spanne von 29,952 Sekunden bei einer Lesefrist von 30
// Sekunden und 1798,144 Sekunden bei einer von 1800 — die Uhr lief also der
// eigenen Lesefrist des Tests hinterher, nicht der Frist des Proxys, weil der
// Kopf der Anfrage noch unterwegs war, als der Ablauf sich für untätig hielt.
// Ein zwischengeschobener virtueller Schlaf half nicht (gemessen: 28,662
// Sekunden). Deshalb echte Sekunden, und deshalb dieselben Werte wie oben.

/// Die Lücke, die ein redender Peer zwischen zwei Stücken lässt.
///
/// Sie liegt zwischen [`CLOCK`] und [`SLOW_CLOCK`], und daran hängt der ganze
/// Beweis: Mit der kurzen Uhr wird derselbe Peer abgeschnitten, mit der langen
/// kommt er durch. Eine fest verdrahtete Frist bestünde höchstens eine der
/// beiden Richtungen.
const GAP: Duration = Duration::from_secs(2);

/// Eine Frist, die in diesen Fällen nie ablaufen soll: Sie gehört einer anderen
/// Spanne, und wenn sie mitmisst, misst der Test das Falsche.
const NEVER: Duration = Duration::from_secs(3600);

/// Die obere Zugabe dieser Fälle, enger als [`SLACK`].
///
/// Sie muss unter dem Abstand zu [`GAP`] bleiben: Mit einer Zugabe von
/// anderthalb Sekunden läge die Lücke des redenden Peers (2 s) noch im Fenster
/// der Ein-Sekunden-Uhr, und eine Frist, die in Wahrheit auf die Lücke wartet,
/// käme durch.
const NARROW_SLACK: Duration = Duration::from_millis(700);

/// Prüft, dass eine Spanne genau so lang war wie die konfigurierte Frist.
///
/// Beide Schranken gehören dazu. Die untere allein bestünde jede zu lange Uhr,
/// die obere allein jede zu kurze; erst zusammen, und erst mit zwei
/// verschiedenen konfigurierten Werten, binden sie die Uhr an ihren Schlüssel.
fn assert_span_was_the_clock(measured: Duration, clock: Duration, span: &str) {
    assert!(
        measured + EARLY >= clock,
        "{span} ended after {measured:?}, before its own configured timeout of {clock:?}"
    );
    assert!(
        measured < clock + NARROW_SLACK,
        "{span} ended after {measured:?}; the configured timeout is {clock:?}, so this clock does \
         not come from its configuration key"
    );
}

/// Liest bis zum Ende der Verbindung und liefert die Zeit dafür; wie
/// [`read_to_the_end`], aber auch für die Lesehälfte eines geteilten Stroms.
async fn read_half_to_the_end(
    stream: &mut (impl AsyncRead + Unpin),
    sink: &mut Vec<u8>,
) -> Duration {
    let started = Instant::now();
    tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(sink))
        .await
        .expect("the proxy closes the connection on its own")
        .expect("reading until the end of the connection works");
    started.elapsed()
}

/// Liest, bis `marker` im Empfangenen steht; wie [`read_until`], aber auch für
/// die Lesehälfte eines geteilten Stroms.
async fn read_half_until(
    stream: &mut (impl AsyncRead + Unpin),
    sink: &mut Vec<u8>,
    marker: &str,
) -> Duration {
    let started = Instant::now();
    let mut chunk = [0u8; 4096];
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "no {marker:?} within 20s; got {:?}",
            String::from_utf8_lossy(sink)
        );
        let read = stream.read(&mut chunk).await.expect("the connection reads");
        assert!(
            read > 0,
            "the connection ended before {marker:?}; got {:?}",
            String::from_utf8_lossy(sink)
        );
        sink.extend_from_slice(&chunk[..read]);
        if String::from_utf8_lossy(sink).contains(marker) {
            return started.elapsed();
        }
    }
}

/// Ein `POST` mit vollständigem Kopf, angekündigten 1000 Bytes und zehn
/// gesendeten. Der Rest kommt später — oder nie.
fn half_sent_post(port: u16) -> String {
    format!(
        "POST http://127.0.0.1:{port}/echo HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Length: 1000\r\n\r\n0123456789"
    )
}

/// Die Codes aller Befunde, die dieser Strom bisher gezeigt hat.
fn diagnostic_codes(events: &support::Events) -> Vec<String> {
    events
        .seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => Some(diagnostic.code.as_str().to_owned()),
            _other => None,
        })
        .collect()
}

/// Spanne 1, Richtung „abgeschnitten": Ein Client, der zehn Bytes schickt und
/// dann schweigt, bekommt `408` nach genau der konfigurierten Frist.
///
/// Vor HUM-120 lief diese Spanne ohne Uhr: Hypers Kopf-Uhr ist gelöscht, sobald
/// der Kopf geparst ist, und um `body::buffer` lag keine Frist. Gemessen wurde
/// eine Verbindung, die nach acht Sekunden noch stand, nichts lieferte und kein
/// einziges Ereignis erzeugte — kein Mensch hat sie je gesehen.
#[tokio::test(flavor = "multi_thread")]
async fn a_silent_request_body_ends_in_408_after_limits_body_timeout_secs() {
    for clock in [CLOCK, SLOW_CLOCK] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            // Die Kopf-Uhr gehört einer anderen Spanne und darf hier nicht
            // mitmessen; sonst stünde nicht fest, welche der beiden schnitt.
            .header_timeout(NEVER)
            .body_timeout(clock)
            .passthrough()
            .start()
            .await;
        let mut events = proxy.events();

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        stream
            .write_all(half_sent_post(port).as_bytes())
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut sink = Vec::new();
        let silence = read_to_the_end(&mut stream, &mut sink).await;

        let text = String::from_utf8_lossy(&sink);
        assert!(
            text.starts_with("HTTP/1.1 408 Request Timeout"),
            "a request body that goes silent must end in 408: {text}"
        );
        assert!(
            text.to_ascii_lowercase().contains("connection: close"),
            "the answer must close the connection: {text}"
        );
        assert_span_was_the_clock(silence, clock, "the request body");
        assert_eq!(
            upstream.hits(),
            0,
            "nothing may leave while the body is incomplete"
        );

        // Der Mensch sieht die Spanne. Ein Fluss entsteht nicht — es gibt keine
        // vollständige Anfrage, über die zu entscheiden wäre —, aber der Befund
        // steht im Ereignisstrom, und genau der fehlte vorher.
        events.drain();
        assert_eq!(
            events.count("received"),
            0,
            "a half-sent request never becomes a flow: {:?}",
            events.names()
        );
        assert_eq!(diagnostic_codes(&events), ["PROXY_011"]);
    }
}

/// Spanne 1, das Gegenstück: Derselbe Client mit derselben Pause kommt durch,
/// sobald die konfigurierte Frist länger ist als seine Pause.
///
/// Die Pause ist beide Male [`GAP`]; nur der Schlüssel ändert sich. Damit
/// scheitert jede Frist, die nicht aus `limits.body_timeout_secs` kommt: Eine
/// zu kurze schneidet auch den langsamen Lauf ab, eine zu lange lässt auch den
/// schnellen durch. Und weil die Pause zwischen den beiden Werten liegt, fällt
/// auch eine Frist durch, die in Wahrheit die Gesamtdauer misst.
#[tokio::test(flavor = "multi_thread")]
async fn a_request_body_that_pauses_shorter_than_its_clock_runs_to_the_end() {
    for (clock, arrives) in [(CLOCK, false), (SLOW_CLOCK, true)] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            .header_timeout(NEVER)
            .body_timeout(clock)
            .passthrough()
            .start()
            .await;

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        // Geliehene Hälften, nicht `into_split`: Eine eigene `OwnedWriteHalf`
        // schließt beim Fallenlassen die Schreibrichtung, und der Proxy sähe
        // ein Dateiende, bevor der Test seine Antwort gelesen hat.
        let (mut reader, mut writer) = stream.split();
        let head = half_sent_post(port);
        let feeding = async {
            let _ignored = writer.write_all(head.as_bytes()).await;
            let _ignored = writer.flush().await;
            tokio::time::sleep(GAP).await;
            // Nach dem Abschneiden schlägt dieses Schreiben fehl. Das ist der
            // Fall und kein Fehler des Tests.
            let _ignored = writer.write_all(&[b'x'; 990]).await;
            let _ignored = writer.flush().await;
        };

        let mut sink = Vec::new();
        let reading = async {
            if arrives {
                // Der Fake-Upstream spiegelt die Länge zurück: `X-Echo-Len: 1000`
                // heißt, dass beide Hälften des Rumpfs bei ihm ankamen.
                read_half_until(&mut reader, &mut sink, "X-Echo-Len: 1000").await
            } else {
                read_half_to_the_end(&mut reader, &mut sink).await
            }
        };
        let (_fed, span) = tokio::join!(feeding, reading);

        if arrives {
            let text = String::from_utf8_lossy(&sink);
            assert!(
                text.starts_with("HTTP/1.1 200 OK"),
                "a body whose pause is shorter than {clock:?} must go through: {text}"
            );
            assert!(
                span + EARLY >= GAP,
                "the answer came after {span:?}, so the second half was never awaited"
            );
            assert_eq!(upstream.hits(), 1, "the complete request goes out");
        } else {
            let text = String::from_utf8_lossy(&sink);
            assert!(
                text.starts_with("HTTP/1.1 408 Request Timeout"),
                "a pause of {GAP:?} is longer than {clock:?} and must be cut: {text}"
            );
            assert_span_was_the_clock(span, clock, "the request body");
            assert_eq!(upstream.hits(), 0, "an incomplete request stays here");
        }
    }
}

/// Spanne 2, beide Richtungen: Ein Ziel, dessen nächstes Stück zu spät kommt,
/// wird abgeschnitten; kommt es rechtzeitig, läuft der Strom zu Ende.
///
/// Und die Aufzeichnung sagt, was geschah: Der abgeschnittene Strom steht als
/// **gekürzt** in der Historie, der vollständige nicht. Eine halbe Antwort, die
/// wie eine ganze aussieht, wäre schlimmer als gar keine
/// (`backlog/CONVENTIONS.md` 4.13).
///
/// Das Ziel ist hier das Sprachmodell: Genau seine Antwort ist der Strom, den
/// eine Uhr über der Gesamtdauer zerrissen hätte.
#[tokio::test(flavor = "multi_thread")]
async fn a_response_body_that_goes_silent_is_cut_and_recorded_as_truncated() {
    for (clock, complete) in [(CLOCK, false), (SLOW_CLOCK, true)] {
        let upstream = FakeUpstream::ollama().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            .header_timeout(NEVER)
            .body_timeout(clock)
            .passthrough()
            .recording(true)
            .start()
            .await;
        let mut events = proxy.events();

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        // Zwei Stücke im Abstand von `GAP`, danach `[DONE]`.
        let path = format!("/api/chat?count=2&interval_ms={}", GAP.as_millis());
        let request = format!(
            "POST http://127.0.0.1:{port}{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
             Content-Length: 2\r\nContent-Type: application/json\r\n\r\n{{}}"
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();

        // Das erste Stück kommt sofort; ab ihm läuft die Frist, denn sie misst
        // die Lücke zwischen zwei Stücken und nicht die Gesamtdauer.
        let mut sink = Vec::new();
        read_until(&mut stream, &mut sink, "\"index\":0").await;
        assert!(
            String::from_utf8_lossy(&sink).starts_with("HTTP/1.1 200 OK"),
            "{:?}",
            String::from_utf8_lossy(&sink)
        );

        if complete {
            let span = read_until(&mut stream, &mut sink, "[DONE]").await;
            assert!(
                span + EARLY >= GAP,
                "the second chunk cannot have arrived within {span:?}"
            );
        } else {
            let silence = read_to_the_end(&mut stream, &mut sink).await;
            assert_span_was_the_clock(silence, clock, "the streamed response body");
            let text = String::from_utf8_lossy(&sink);
            assert!(
                !text.contains("[DONE]"),
                "a cut stream must not carry the end of the answer: {text}"
            );
            assert_eq!(
                text.matches("data: ").count(),
                1,
                "only the first chunk may have arrived: {text}"
            );
        }

        // `Recorded` kommt aus `TeeBody::finish`, und dort wird der Mitschnitt
        // unmittelbar davor geschlossen: Wer das Ereignis gesehen hat, findet
        // die Zeile nach dem Leeren der Warteschlange vor.
        events.wait_for("recorded").await;
        let recorder = proxy.recorder.as_ref().expect("recording is on");
        recorder.flush().await;
        let rows = recorder
            .list_flows(&FlowQuery::new(""))
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .rows;
        let row = rows.first().expect("the flow is in the history");
        let detail = recorder
            .get_flow(row.id)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .expect("the flow is in the history");
        let response = detail
            .messages
            .iter()
            .find(|message| message.dir == Dir::Response)
            .expect("the answer is recorded");
        assert_eq!(
            response.body.truncated, !complete,
            "a cut answer must be recorded as truncated and a complete one must not"
        );
    }
}

/// Spanne 3, Richtung „abgeschnitten": Ein `CONNECT`, das mit `200` bestätigt
/// wurde und nie ein `ClientHello` schickt, endet nach genau der konfigurierten
/// Kopf-Frist.
///
/// Still, ohne Antwort und ohne Fluss: Der Client hat sein `200` längst, und
/// eine Anfrage stand nie im Tunnel. Was bleibt, ist eine Protokollzeile — wie
/// bei jedem gescheiterten Handschlag.
#[tokio::test(flavor = "multi_thread")]
async fn a_connect_without_a_client_hello_ends_after_limits_header_timeout_secs() {
    for clock in [CLOCK, SLOW_CLOCK] {
        let proxy = ProxyBuilder::new().header_timeout(clock).start().await;

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        stream
            .write_all(b"CONNECT example.test:443 HTTP/1.1\r\nHost: example.test:443\r\n\r\n")
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut sink = Vec::new();
        read_until(&mut stream, &mut sink, "200 OK").await;
        // Kein Byte mehr. Der Tunnel steht, der Handschlag beginnt nie.
        let silence = read_to_the_end(&mut stream, &mut sink).await;
        assert_span_was_the_clock(silence, clock, "the tls handshake after CONNECT");
    }
}

/// Spanne 3, das Gegenstück: Ein regulärer Handschlag wird nicht abgeschnitten,
/// und die Anfrage darin läuft durch.
///
/// Ohne diesen Fall bewiese der vorige nur, dass irgendetwas den Tunnel
/// schließt — auch eine Frist von null täte das.
#[tokio::test(flavor = "multi_thread")]
async fn a_real_handshake_survives_the_header_timeout() {
    let proxy = ProxyBuilder::new()
        .header_timeout(CLOCK)
        .passthrough()
        .start()
        .await;
    let upstream = FakeUpstream::tls(&proxy.ca).await;

    let mut tunnel = proxy.tls_client("localhost", upstream.port()).await;
    let request = Request::builder()
        .uri("/echo")
        .header("host", format!("localhost:{}", upstream.port()))
        .body(Full::new(Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(upstream.hits(), 1);
}

/// Die Menge, nicht die Dauer: Über `limits.max_client_connections` hinaus wird
/// abgelehnt, statt angenommen und liegen gelassen.
///
/// Drei Uhren schließen die Spannen, aber nicht die Menge. Wer in einer Spanne
/// stehenbleibt, bindet je Verbindung eine Aufgabe und einen Dateideskriptor;
/// ohne Obergrenze bindet derselbe Angriff dieselben Ressourcen, nur kürzer und
/// dafür öfter. Deshalb gehören beide in dasselbe Issue.
///
/// Beide Läufe öffnen dieselben drei Verbindungen; nur der konfigurierte Wert
/// ändert sich, und mit ihm das Schicksal der dritten.
#[tokio::test(flavor = "multi_thread")]
async fn the_connection_over_limits_max_client_connections_is_refused() {
    for (limit, refused) in [(2_u32, true), (3_u32, false)] {
        let upstream = FakeUpstream::plain().await;
        let port = upstream.port();
        let proxy = ProxyBuilder::new()
            // Während des Tests darf nichts von selbst zugehen; gemessen wird
            // die Zahl der Verbindungen und sonst nichts.
            .header_timeout(NEVER)
            .max_client_connections(limit)
            .passthrough()
            .start()
            .await;
        let mut events = proxy.events();

        // Zwei Verbindungen, die einen halben Kopf schicken und dann warten.
        // Sie sind angenommen und halten ihren Platz.
        let mut held = Vec::new();
        for _ in 0..2 {
            let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
            stream.write_all(b"GET ").await.unwrap();
            stream.flush().await.unwrap();
            held.push(stream);
        }

        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        // Die Anfrage geht sofort hinaus, wie bei jedem gewöhnlichen Client.
        // Damit liegen beim Ablehnen ungelesene Bytes im Empfangspuffer des
        // Proxys — und genau dann entscheidet sich, ob die `503` ankommt oder
        // ob der Kern beim Schließen ein `RST` schickt und sie mitnimmt.
        stream
            .write_all(raw_get(port, "/echo").as_bytes())
            .await
            .unwrap();
        stream.flush().await.unwrap();
        let mut sink = Vec::new();
        if refused {
            read_to_the_end(&mut stream, &mut sink).await;
            let text = String::from_utf8_lossy(&sink);
            assert!(
                text.starts_with("HTTP/1.1 503 Service Unavailable"),
                "over the limit the proxy answers 503 instead of dropping the connection: {text}"
            );
            assert!(
                text.to_ascii_lowercase().contains("connection: close"),
                "{text}"
            );
            assert!(
                text.contains("reason: max_client_connections"),
                "the answer must name the limit it hit: {text}"
            );
            assert_eq!(
                upstream.hits(),
                0,
                "a refused connection never carried a request"
            );
            let event = events.wait_for("diagnostic").await;
            let FlowEvent::Diagnostic {
                flow_id,
                diagnostic,
                ..
            } = event
            else {
                panic!("wait_for returned the wrong event");
            };
            assert_eq!(diagnostic.code.as_str(), "PROXY_010");
            assert_eq!(flow_id, None, "no request was read, so there is no flow");
        } else {
            read_until(&mut stream, &mut sink, support::ECHO_BODY).await;
            let text = String::from_utf8_lossy(&sink);
            assert!(
                text.starts_with("HTTP/1.1 200 OK"),
                "under the limit the third connection is served: {text}"
            );
            assert_eq!(upstream.hits(), 1);
        }
        drop(held);
    }
}

/// Wie weit ein Tunnel für diesen Test gedieh, bevor er stehenblieb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TunnelStage {
    /// `CONNECT` bestätigt, danach kein `ClientHello`. Der Tunnel hängt in
    /// `tls::accept`, also in der dritten Spanne dieses Issues.
    BeforeClientHello,
    /// Handschlag fertig, Tunnel steht, keine Anfrage darin.
    Established,
}

/// Öffnet einen Tunnel und lässt ihn offen; der Rückgabewert hält ihn.
///
/// `BeforeClientHello` liefert den rohen Strom, über den nichts mehr geht;
/// `Established` liefert den fertigen Client des Tunnels. Beide Male lebt die
/// Verbindung, solange der Wert lebt.
async fn open_tunnel(
    proxy: &support::Proxy,
    stage: TunnelStage,
) -> (Option<UnixStream>, Option<support::TlsClient>) {
    match stage {
        TunnelStage::BeforeClientHello => {
            let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
            stream
                .write_all(b"CONNECT localhost:443 HTTP/1.1\r\nHost: localhost:443\r\n\r\n")
                .await
                .unwrap();
            stream.flush().await.unwrap();
            let mut sink = Vec::new();
            read_until(&mut stream, &mut sink, "200 OK").await;
            (Some(stream), None)
        }
        TunnelStage::Established => (None, Some(proxy.tls_client("localhost", 443).await)),
    }
}

/// Ein `CONNECT`-Tunnel behält seinen Platz, solange er offen ist.
///
/// **Der Fall, für den die Grenze am wichtigsten ist.** Bei einem `CONNECT`
/// übergibt hyper die Verbindung an `hyper::upgrade::on`; `serve_connection`
/// kehrt dabei zurück, während der Tunnel in einer eigenen Aufgabe weiterlebt.
/// Hängt der Platz an der Lebensdauer von `serve_connection`, fällt er genau
/// dort — und `limits.max_client_connections` gilt dann nur für einfaches HTTP
/// und ausgerechnet nicht für die TLS-Tunnel, über die der Agent den
/// überwiegenden Teil seines Verkehrs führt. Der erste Entwurf dieses Issues
/// hatte genau diesen Fehler, und der erste Entwurf dieses Tests hat ihn nicht
/// gesehen: Er hielt nur halb gesendete gewöhnliche Anfragen.
///
/// Beide Stufen gehören dazu. `BeforeClientHello` ist der schärfere Fall — er
/// trifft die Handschlag-Spanne und die Mengengrenze zugleich, und dort steht
/// noch nicht einmal ein entschlüsselter Strom, an dem etwas hängen könnte.
/// `Established` zeigt, dass auch der fertige Tunnel zählt.
#[tokio::test(flavor = "multi_thread")]
async fn a_connect_tunnel_holds_its_place_until_it_is_closed() {
    for stage in [TunnelStage::BeforeClientHello, TunnelStage::Established] {
        for (limit, refused) in [(2_u32, true), (3_u32, false)] {
            let upstream = FakeUpstream::plain().await;
            let port = upstream.port();
            let proxy = ProxyBuilder::new()
                // Nichts darf von selbst zugehen: Ohne diese Zeile schnitte die
                // Handschlag-Uhr die hängenden Tunnel weg, und der Test mäße,
                // dass eine Frist läuft, statt dass ein Platz gehalten wird.
                .header_timeout(NEVER)
                .max_client_connections(limit)
                .passthrough()
                .start()
                .await;
            let mut events = proxy.events();

            let mut tunnels = Vec::new();
            for _ in 0..2 {
                tunnels.push(open_tunnel(&proxy, stage).await);
            }

            let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
            let mut sink = Vec::new();
            if refused {
                read_to_the_end(&mut stream, &mut sink).await;
                let text = String::from_utf8_lossy(&sink);
                assert!(
                    text.starts_with("HTTP/1.1 503 Service Unavailable"),
                    "two open tunnels ({stage:?}) fill a limit of {limit}, so the next connection \
                     is refused: {text}"
                );
                assert!(
                    text.contains("reason: max_client_connections"),
                    "the refusal must arrive with its reason and not as a reset: {text}"
                );
                let event = events.wait_for("diagnostic").await;
                let FlowEvent::Diagnostic { diagnostic, .. } = event else {
                    panic!("wait_for returned the wrong event");
                };
                assert_eq!(diagnostic.code.as_str(), "PROXY_010");
            } else {
                stream
                    .write_all(raw_get(port, "/echo").as_bytes())
                    .await
                    .unwrap();
                stream.flush().await.unwrap();
                read_until(&mut stream, &mut sink, support::ECHO_BODY).await;
                let text = String::from_utf8_lossy(&sink);
                assert!(
                    text.starts_with("HTTP/1.1 200 OK"),
                    "under the limit the connection beside the two tunnels is served: {text}"
                );
                assert_eq!(upstream.hits(), 1);
            }

            // Und der Platz kommt zurück, sobald die Tunnel fallen. Ohne diese
            // Hälfte bewiese der Test nur, dass irgendetwas ablehnt — auch eine
            // Grenze, die nie wieder aufmacht, täte das.
            drop(stream);
            tunnels.clear();
            wait_for_a_free_place(&proxy, port).await;
        }
    }
}

/// Wartet, bis wieder eine Verbindung bedient wird.
///
/// Mit Wiederholung, und das ist kein Nachgeben gegenüber einer wackligen
/// Zusicherung: Der Platz wird frei, wenn die Aufgabe des Tunnels endet, und
/// die endet erst, nachdem sie das Dateiende ihres Stroms gesehen hat. Ein
/// einziger Versuch unmittelbar nach dem Schließen misst die Laufzeit des
/// Ablaufplaners und nicht die Grenze. Was der Test zusichert, ist „der Platz
/// kommt zurück", nicht „er kommt in derselben Mikrosekunde zurück".
async fn wait_for_a_free_place(proxy: &support::Proxy, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut stream = UnixStream::connect(&proxy.socket).await.unwrap();
        stream
            .write_all(raw_get(port, "/echo").as_bytes())
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut sink = Vec::new();
        let mut chunk = [0u8; 4096];
        let served = loop {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break false,
                Ok(read) => {
                    sink.extend_from_slice(&chunk[..read]);
                    let text = String::from_utf8_lossy(&sink);
                    if text.contains(support::ECHO_BODY) {
                        break true;
                    }
                    if text.contains("503 Service Unavailable") {
                        break false;
                    }
                }
            }
        };
        if served {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "no place came back within 10s after the tunnels were closed; got {:?}",
            String::from_utf8_lossy(&sink)
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
