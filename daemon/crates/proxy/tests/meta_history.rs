//! Meta-Anfragen in der Historie (HUM-103).
//!
//! `daemon/crates/proxy/tests/meta.rs` prüft die Weiche: Was an
//! `humanitl.internal` geht, wird selbst beantwortet und nie aufgelöst. Hier
//! geht es um die andere Hälfte: Was davon bleibt.
//!
//! Ein Lauf mit je einer Anfrage an `/`, `/why/<id>` und `/ask` hinterlässt
//! drei Zeilen in der Aufzeichnung. Sie tragen den Vermerk `meta` und **keine**
//! Entscheidung — über eine Meta-Anfrage entscheidet niemand —, sie sind mit
//! `meta:true` zu finden und mit `meta:false` auszuschließen, und keine
//! Zählung über Entscheidungen sieht sie.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::path::Path;
use std::time::Duration;

use humanitl_core::FlowId;
use humanitl_recorder::{Dir, FlowQuery, FlowSummary, Recorder};
use hyper::StatusCode;

use support::{Proxy, ProxyBuilder, body_string, get, header, post};

/// Der Text, den der Agent in `/ask` schreibt.
const ASK_TEXT: &str = "bitte https://pypi.org/simple/ freischalten";

/// Ein Stück der Antwort von `/`, das in keiner Aufzeichnung stehen darf.
const STATUS_MARKER: &str = "rules (first match wins):";

/// Der eine gewöhnliche Flow des Laufs.
struct Decided {
    id: FlowId,
    text: String,
}

/// Fährt den Lauf: ein gewöhnlicher Flow und die drei Meta-Anfragen.
///
/// Der gewöhnliche Flow läuft in die Zeitüberschreitung; er ist zugleich das
/// Ziel von `/why/<id>` und die Gegenprobe, an der sich zeigt, dass die
/// Zählungen über Entscheidungen unverändert bleiben.
async fn run() -> (Proxy, Decided) {
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .recording(true)
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://slow.example/steal")).await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let text = header(&response, "x-humanitl-flow")
        .expect("the block response names its flow")
        .to_owned();
    events.wait_for("recorded").await;

    let mut client = proxy.client().await;
    let response = client.send(get("http://humanitl.internal/")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let status_body = body_string(response.into_body()).await;
    assert!(status_body.contains(STATUS_MARKER), "{status_body}");

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://humanitl.internal/why/{text}")))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_string(response.into_body()).await,
        "decision=timed_out reason=timeout note=\n"
    );

    let mut client = proxy.client().await;
    let response = client
        .send(post("http://humanitl.internal/ask", ASK_TEXT))
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(body_string(response.into_body()).await, "queued\n");

    let id = FlowId::parse(&text).expect("the header carries a flow id");
    (proxy, Decided { id, text })
}

/// Die Zeilen, die dieser Filter trifft.
async fn rows(recorder: &Recorder, filter: &str) -> Vec<FlowSummary> {
    recorder.flush().await;
    recorder
        .list_flows(&FlowQuery::new(filter))
        .await
        .unwrap_or_else(|err| panic!("{filter}: {err}"))
        .rows
}

#[tokio::test(flavor = "multi_thread")]
async fn three_meta_requests_are_three_entries() {
    let (proxy, decided) = run().await;
    let recorder = proxy.recorder.as_ref().expect("recording is on");

    let all = rows(recorder, "").await;
    assert_eq!(all.len(), 4, "one decided flow and the three meta requests");

    let meta = rows(recorder, "meta:true").await;
    assert_eq!(meta.len(), 3);
    let mut paths: Vec<String> = meta.iter().map(|row| row.path.clone()).collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "/".to_owned(),
            "/ask".to_owned(),
            format!("/why/{}", decided.text),
        ]
    );

    for row in &meta {
        assert!(row.meta, "{}", row.path);
        assert_eq!(
            row.decision, None,
            "{} has no decision; nobody decided about it",
            row.path
        );
        assert_eq!(row.block_reason, None, "{}", row.path);
        assert_eq!(row.rule_id, None, "{}", row.path);
        assert_eq!(row.state, "recorded", "{}", row.path);
        assert_eq!(row.host, "humanitl.internal", "{}", row.path);
        assert!(!row.passthrough, "{}", row.path);
        assert_eq!(row.held_ms, None, "a meta request is never held");
    }

    // Der Statuscode ist der, den der Proxy selbst geschrieben hat.
    let mut statuses: Vec<u16> = meta.iter().filter_map(|row| row.status).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, vec![200, 200, 202]);

    // Die Methode steht auch da: `/ask` ist das eine `POST`.
    let ask = meta
        .iter()
        .find(|row| row.path == "/ask")
        .expect("the ask is in the history");
    assert_eq!(ask.method, "POST");
}

#[tokio::test(flavor = "multi_thread")]
async fn meta_true_and_meta_false_split_the_history() {
    let (proxy, decided) = run().await;
    let recorder = proxy.recorder.as_ref().expect("recording is on");

    let yes: Vec<FlowId> = rows(recorder, "meta:true")
        .await
        .iter()
        .map(|row| row.id)
        .collect();
    let no: Vec<FlowId> = rows(recorder, "meta:false")
        .await
        .iter()
        .map(|row| row.id)
        .collect();

    assert_eq!(yes.len(), 3);
    assert_eq!(no, vec![decided.id], "meta:false is exactly the rest");
    assert!(yes.iter().all(|id| !no.contains(id)));
    assert_eq!(yes.len() + no.len(), rows(recorder, "").await.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn no_count_over_decisions_changes() {
    let (proxy, decided) = run().await;
    let recorder = proxy.recorder.as_ref().expect("recording is on");

    // Der eine entschiedene Flow ist der Zeitablauf, und er ist es allein.
    let timed_out = rows(recorder, "decision:timed_out").await;
    assert_eq!(timed_out.len(), 1);
    assert_eq!(timed_out[0].id, decided.id);

    for term in ["decision:allow", "decision:block", "decision:allow_edited"] {
        assert!(
            rows(recorder, term).await.is_empty(),
            "{term} counts nothing in this run"
        );
    }
    // Und die Meta-Flüsse sind wirklich da, während keine dieser Zählungen
    // sie sieht: ohne diese Zeile wäre die Schleife darunter leer und grün.
    assert_eq!(rows(recorder, "meta:true").await.len(), 3);
    for term in [
        "decision:timed_out",
        "decision:allow",
        "decision:block",
        "reason:timeout",
    ] {
        assert!(
            rows(recorder, term).await.iter().all(|row| !row.meta),
            "{term} must never see a meta flow"
        );
    }
}

/// Kein Rumpf einer Meta-Antwort landet in der Aufzeichnung.
///
/// Aufgezeichnet wird die Anfrage, und bei `/ask` der gesäuberte Text — er ist
/// ohnehin schon als Ereignis durch die Oberfläche gegangen. Was der Endpunkt
/// *antwortet*, bleibt draußen: weder als Nachricht am Flow noch irgendwo in
/// den Dateien der Aufzeichnung.
#[tokio::test(flavor = "multi_thread")]
async fn no_body_of_a_meta_answer_is_recorded() {
    let (proxy, _decided) = run().await;
    let recorder = proxy.recorder.as_ref().expect("recording is on");

    let meta = rows(recorder, "meta:true").await;
    assert_eq!(
        meta.len(),
        3,
        "without rows this test would pass by having nothing to look at"
    );
    for row in meta {
        let detail = recorder
            .get_flow(row.id)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .expect("the meta flow is in the history");
        assert!(
            detail
                .messages
                .iter()
                .all(|message| message.dir != Dir::Response),
            "{} must not carry the answer of the endpoint",
            row.path
        );
        assert_eq!(
            detail.summary.response_size, None,
            "{} never had a recorded answer",
            row.path
        );
        // Die Bitte trägt den gesäuberten Text, die beiden `GET` nichts.
        let request = detail
            .messages
            .iter()
            .find(|message| message.dir == Dir::Request)
            .expect("the request itself is recorded");
        let body = request.body.inline.as_deref().unwrap_or_default();
        if row.path == "/ask" {
            assert_eq!(String::from_utf8_lossy(body), ASK_TEXT);
        } else {
            assert!(body.is_empty(), "{} has no body", row.path);
        }
    }

    // Und der Text der Antwort steht in keiner Datei der Aufzeichnung.
    recorder.flush().await;
    let database = recorder.database_path().to_path_buf();
    let root = database.parent().expect("the database has a directory");
    let hits = files_containing(root, STATUS_MARKER.as_bytes());
    assert!(
        hits.is_empty(),
        "the answer of the meta endpoint must not be stored: {hits:?}"
    );
}

/// Alle Dateien unter `root`, die diese Bytefolge enthalten.
fn files_containing(root: &Path, needle: &[u8]) -> Vec<String> {
    let mut hits = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if bytes.windows(needle.len()).any(|window| window == needle) {
                hits.push(path.display().to_string());
            }
        }
    }
    hits
}
