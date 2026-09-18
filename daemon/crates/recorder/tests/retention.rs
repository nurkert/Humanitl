//! Die Aufbewahrung, gemessen an einer echten Datenbank (HUM-051).
//!
//! Vier Fragen, und jede davon ist eine Zusage an einen Menschen, der wissen
//! will, was von ihm gespeichert bleibt:
//!
//! 1. Wird wirklich nur gelöscht, was älter ist als die Grenze?
//! 2. Fallen die Blobs mit, auf die danach niemand mehr zeigt?
//! 3. Heißt `0` wirklich „nie" und nicht „alles"?
//! 4. Bleibt die Audit-Kette unberührt?
//!
//! Die dritte ist der Grund, aus dem es [`humanitl_recorder::Retention`] gibt:
//! `jetzt − 0 Tage` ist `jetzt`, und ein Lauf mit dieser Grenze löschte die
//! ganze Aufzeichnung.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use bytes::Bytes;
use humanitl_core::http::HeaderValue;
use humanitl_core::{
    Authority, FlowEvent, FlowId, HeaderMap, HostName, HttpRequest, Method, Scheme, SessionId,
};
use humanitl_recorder::{
    AnchorStore, AuditAnchor, Dir, Recorder, RecorderSettings, SessionMeta, read_anchors,
};

/// Eine Aufzeichnung mit dieser Aufbewahrungsfrist, in einem Temp-Verzeichnis.
struct Harness {
    _dir: tempfile::TempDir,
    recorder: Recorder,
    db: std::path::PathBuf,
}

impl Harness {
    fn with_days(retention_days: u32) -> Self {
        let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
        let db = dir.path().join("data").join("humanitl.db");
        let blobs = dir.path().join("data").join("blobs");
        let recorder = Recorder::open(
            &db,
            &blobs,
            RecorderSettings::new(64, 4_096, retention_days),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        let session = SessionId::new();
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/home/x/projekt".to_owned(),
            agent: "opencode".to_owned(),
        });
        Self {
            _dir: dir,
            recorder,
            db,
        }
    }
}

/// Ein angekommener Flow zu einem Zeitpunkt.
fn received(flow: FlowId, host: &str, at: SystemTime) -> FlowEvent {
    let host = HostName::parse(host).unwrap_or_else(|err| panic!("{err}"));
    FlowEvent::Received {
        flow_id: flow,
        at,
        request: Box::new(HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(host, Scheme::Https),
            "/x",
        )),
    }
}

/// Die Kopfzeilen einer JSON-Anfrage.
fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers
}

/// Tage in Sekunden.
fn days(count: u64) -> Duration {
    Duration::from_secs(count * 24 * 60 * 60)
}

#[tokio::test]
async fn deletes_older_than_cutoff_only() {
    let harness = Harness::with_days(180);
    let now = SystemTime::now();

    let old = FlowId::new();
    harness
        .recorder
        .apply(&received(old, "old.example", now - days(200)));
    // Einen Tag jünger als die Grenze: Der Lauf darf ihn nicht anfassen.
    let inside = FlowId::new();
    harness
        .recorder
        .apply(&received(inside, "inside.example", now - days(179)));
    harness.recorder.flush().await;

    let report = harness
        .recorder
        .purge_expired(now)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(report.flows, 1, "only the flow beyond 180 days goes");

    assert!(
        harness
            .recorder
            .get_flow(old)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .is_none(),
        "a flow recorded 200 days ago is gone after the run"
    );
    assert!(
        harness
            .recorder
            .get_flow(inside)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .is_some(),
        "a flow one day inside the window stays"
    );
}

#[tokio::test]
async fn orphan_blobs_removed() {
    let harness = Harness::with_days(180);
    let now = SystemTime::now();

    let old = FlowId::new();
    harness
        .recorder
        .apply(&received(old, "old.example", now - days(200)));
    let old_body = harness
        .recorder
        .store_message(
            old,
            Dir::Request,
            &json_headers(),
            Bytes::from(vec![b'o'; 1_000]),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));

    let fresh = FlowId::new();
    harness
        .recorder
        .apply(&received(fresh, "fresh.example", now));
    let fresh_body = harness
        .recorder
        .store_message(
            fresh,
            Dir::Request,
            &json_headers(),
            Bytes::from(vec![b'f'; 1_000]),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let report = harness
        .recorder
        .purge_expired(now)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(report.blobs, 1);
    assert!(
        !harness.recorder.blobs().contains(&old_body.sha256),
        "the body of the deleted flow has nobody pointing at it any more"
    );
    assert!(
        harness.recorder.blobs().contains(&fresh_body.sha256),
        "the body of the flow that stays is still referenced"
    );
}

#[tokio::test]
async fn zero_means_never() {
    let harness = Harness::with_days(0);
    let now = SystemTime::now();

    let ancient = FlowId::new();
    harness
        .recorder
        .apply(&received(ancient, "ancient.example", now - days(3_650)));
    harness.recorder.flush().await;

    let report = harness
        .recorder
        .purge_expired(now)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(
        report,
        humanitl_recorder::PurgeReport::default(),
        "`recorder.retention_days = 0` deletes nothing at all"
    );
    assert!(
        harness
            .recorder
            .get_flow(ancient)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .is_some(),
        "a flow ten years old survives a retention of zero days"
    );
}

#[tokio::test]
async fn audit_untouched() {
    let harness = Harness::with_days(180);
    let now = SystemTime::now();

    let anchor = AuditAnchor {
        seq: 100,
        hash: "a3f9c2e1".to_owned(),
        ts: "2020-01-01T00:00:00.000000Z".to_owned(),
    };
    {
        let store = AnchorStore::open(&harness.db).unwrap_or_else(|err| panic!("{err}"));
        store.put(&anchor).unwrap_or_else(|err| panic!("{err}"));
    }

    let old = FlowId::new();
    harness
        .recorder
        .apply(&received(old, "old.example", now - days(200)));
    harness.recorder.flush().await;

    let report = harness
        .recorder
        .purge_expired(now)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(report.flows, 1, "the old flow did go");

    assert_eq!(
        read_anchors(&harness.db).unwrap_or_else(|err| panic!("{err}")),
        vec![anchor],
        "the anchor of the audit chain survives every retention pass, however \
         old it is; `audit.retention_days` is a second, separate number"
    );
}

#[tokio::test]
async fn orphan_blobs_removed_only_when_unreferenced() {
    // Die zweite Hälfte der Zusage: Ein Blob fällt nur, wenn niemand mehr auf
    // ihn zeigt. Derselbe Body in einem alten und einem frischen Flow liegt als
    // eine Datei; der alte Flow geht, die Datei bleibt.
    let harness = Harness::with_days(180);
    let now = SystemTime::now();
    let body = Bytes::from(vec![b's'; 1_000]);

    let old = FlowId::new();
    harness
        .recorder
        .apply(&received(old, "old.example", now - days(200)));
    let shared = harness
        .recorder
        .store_message(old, Dir::Request, &json_headers(), body.clone())
        .await
        .unwrap_or_else(|err| panic!("{err}"));

    let fresh = FlowId::new();
    harness
        .recorder
        .apply(&received(fresh, "fresh.example", now));
    harness
        .recorder
        .store_message(fresh, Dir::Request, &json_headers(), body)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let report = harness
        .recorder
        .purge_expired(now)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(report.flows, 1, "the old flow went");
    assert_eq!(
        report.blobs, 0,
        "its body is still referenced by the fresh one"
    );
    assert!(
        harness.recorder.blobs().contains(&shared.sha256),
        "a blob that a remaining row points at stays"
    );
}
