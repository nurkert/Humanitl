//! Die Aufzeichnung von außen: Schema, Schreiben, Blobs, Filter, Blättern.
//!
//! Alle Tests laufen gegen eine echte Datenbank in einem Temp-Verzeichnis; es
//! gibt keinen Mock der Datenbank, weil genau ihr Verhalten (WAL, Fremdschlüssel,
//! Keyset-Vergleich) geprüft werden soll.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use humanitl_core::http::HeaderValue;
use humanitl_core::{
    Authority, BlockReason, Decision, DecisionSource, Finding, FindingKind, FindingLocation,
    FlowEvent, FlowId, HeaderMap, HostName, HttpRequest, Method, Scheme, SessionId, Tier,
};
use humanitl_recorder::{Dir, FlowQuery, Recorder, RecorderSettings, SessionMeta, SortKey, millis};

/// Eine Aufzeichnung in einem eigenen Temp-Verzeichnis.
struct Harness {
    _dir: tempfile::TempDir,
    recorder: Recorder,
    session: SessionId,
    db: std::path::PathBuf,
    blobs: std::path::PathBuf,
}

impl Harness {
    fn open() -> Self {
        Self::with(RecorderSettings::default())
    }

    fn with(settings: RecorderSettings) -> Self {
        let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
        let db = dir.path().join("data").join("humanitl.db");
        let blobs = dir.path().join("data").join("blobs");
        let recorder = Recorder::open(&db, &blobs, settings).unwrap_or_else(|err| panic!("{err}"));
        let session = SessionId::new();
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: Some("http://192.168.1.20:11434".to_owned()),
            work_dir: "/home/x/projekt".to_owned(),
            agent: "opencode".to_owned(),
        });
        Self {
            _dir: dir,
            recorder,
            session,
            db,
            blobs,
        }
    }
}

/// Eine Anfrage an einen Host.
fn request(host: &str, path: &str) -> HttpRequest {
    let host = HostName::parse(host).unwrap_or_else(|err| panic!("{err}"));
    HttpRequest::new(
        Method::GET,
        Scheme::Https,
        Authority::with_scheme(host, Scheme::Https),
        path,
    )
}

/// Ein angekommener Flow zu einem Zeitpunkt.
fn received(flow: FlowId, host: &str, path: &str, at: SystemTime) -> FlowEvent {
    FlowEvent::Received {
        flow_id: flow,
        at,
        request: Box::new(request(host, path)),
    }
}

/// Schiebt einen vollständigen, erlaubten Flow durch die Aufzeichnung.
fn full_flow(recorder: &Recorder, host: &str, path: &str, at: SystemTime) -> FlowId {
    let flow = FlowId::new();
    recorder.apply(&received(flow, host, path, at));
    recorder.apply(&FlowEvent::Analyzed {
        flow_id: flow,
        at,
        findings: Vec::new(),
    });
    recorder.apply(&FlowEvent::Held {
        flow_id: flow,
        at,
        deadline: Instant::now() + Duration::from_secs(300),
        queue_bytes: 0,
        queue_count: 1,
    });
    recorder.apply(&FlowEvent::Decided {
        flow_id: flow,
        at: at + Duration::from_millis(1_200),
        decision: Decision::Allow,
        source: DecisionSource::User,
    });
    recorder.apply(&FlowEvent::Forwarded { flow_id: flow, at });
    recorder.apply(&FlowEvent::ResponseHeaders {
        flow_id: flow,
        at,
        status: 200,
    });
    recorder.apply(&FlowEvent::Recorded {
        flow_id: flow,
        at: at + Duration::from_millis(1_500),
    });
    flow
}

/// Die Kopfzeilen einer JSON-Anfrage.
fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("api.github.com"));
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers
}

#[tokio::test]
async fn wal_enabled() {
    let harness = Harness::open();
    let mode = harness
        .recorder
        .journal_mode()
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(mode.to_ascii_lowercase(), "wal");

    // Ein zweites Öffnen derselben Datei muss durchgehen, ohne zu migrieren.
    let again = Recorder::open(&harness.db, &harness.blobs, RecorderSettings::default())
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(
        again
            .journal_mode()
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .to_ascii_lowercase(),
        "wal"
    );
}

#[tokio::test]
async fn database_and_blob_directory_are_private() {
    use std::os::unix::fs::PermissionsExt as _;

    let harness = Harness::open();
    let flow = FlowId::new();
    harness.recorder.apply(&received(
        flow,
        "api.github.com",
        "/user",
        SystemTime::now(),
    ));
    harness
        .recorder
        .store_message(
            flow,
            Dir::Request,
            &json_headers(),
            Bytes::from_static(b"x"),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let db_mode = std::fs::metadata(&harness.db)
        .unwrap_or_else(|err| panic!("{err}"))
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(db_mode, 0o600, "the database must not be world readable");

    let dir_mode = std::fs::metadata(&harness.blobs)
        .unwrap_or_else(|err| panic!("{err}"))
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700, "the blob directory must not be listable");
}

#[tokio::test]
async fn the_wal_and_shm_files_are_private_too() {
    use std::os::unix::fs::PermissionsExt as _;

    // Frische Datenbank: `-wal` und `-shm` entstehen erst mit der ersten
    // Transaktion, also mit den Migrationen, und zwar mit der Standardmaske des
    // Prozesses. Sie tragen den frischesten Teil der Aufzeichnung.
    let harness = Harness::open();
    let flow = FlowId::new();
    harness.recorder.apply(&received(
        flow,
        "api.github.com",
        "/user",
        SystemTime::now(),
    ));
    harness
        .recorder
        .store_message(
            flow,
            Dir::Request,
            &json_headers(),
            Bytes::from_static(b"geheim"),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    for suffix in ["", "-wal", "-shm"] {
        let mut name = harness.db.as_os_str().to_os_string();
        name.push(suffix);
        let path = std::path::PathBuf::from(name);
        assert!(path.is_file(), "{} is missing", path.display());
        let mode = std::fs::metadata(&path)
            .unwrap_or_else(|err| panic!("{err}"))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode,
            0o600,
            "{} is readable by others (mode {mode:o})",
            path.display()
        );
    }
}

#[tokio::test]
async fn request_size_comes_from_the_stored_body_not_from_the_event() {
    let harness = Harness::open();
    let flow = FlowId::new();
    let at = SystemTime::now();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/user", at));
    harness.recorder.flush().await;

    // `FlowEvent::Received` entsteht, bevor der Body gelesen ist.
    let before = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(before.summary.request_size, 0);

    harness
        .recorder
        .store_message(
            flow,
            Dir::Request,
            &json_headers(),
            Bytes::from(vec![b'r'; 4_096]),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let after = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(after.summary.request_size, 4_096);

    // Die bearbeitete Anfrage ist die, die hinausgeht, also zählt ihre Größe.
    harness
        .recorder
        .store_message(
            flow,
            Dir::RequestEdited,
            &json_headers(),
            Bytes::from(vec![b'e'; 100]),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let edited = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(edited.summary.request_size, 100);
}

#[tokio::test]
async fn sorting_by_size_uses_the_real_sizes() {
    let harness = Harness::open();
    let base = SystemTime::now();
    for (index, size) in [10_usize, 3_000, 300].into_iter().enumerate() {
        let flow = FlowId::new();
        harness.recorder.apply(&received(
            flow,
            "api.github.com",
            "/x",
            base + Duration::from_millis(index as u64),
        ));
        harness
            .recorder
            .store_message(
                flow,
                Dir::Request,
                &json_headers(),
                Bytes::from(vec![b'x'; size]),
            )
            .await
            .unwrap_or_else(|err| panic!("{err}"));
    }
    harness.recorder.flush().await;

    let page = harness
        .recorder
        .list_flows(&FlowQuery {
            sort: SortKey::Size,
            ..FlowQuery::default()
        })
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    let sizes: Vec<u64> = page.rows.iter().map(|row| row.request_size).collect();
    assert_eq!(sizes, vec![3_000, 300, 10]);
}

#[tokio::test]
async fn a_cursor_that_does_not_match_the_sort_is_refused() {
    let harness = Harness::open();
    harness.recorder.flush().await;

    let err = harness
        .recorder
        .list_flows(&FlowQuery {
            sort: SortKey::Host,
            cursor: Some(humanitl_recorder::Cursor::new(0, "x")),
            ..FlowQuery::default()
        })
        .await
        .err()
        .unwrap_or_else(|| panic!("a cursor without a sort value must not be accepted"));
    assert_eq!(err.diagnostic().code.as_str(), "RECORDER_002");
    assert!(
        err.diagnostic().why.contains("host"),
        "{}",
        err.diagnostic().why
    );

    let err = harness
        .recorder
        .list_flows(&FlowQuery {
            sort: SortKey::Ts,
            cursor: Some(humanitl_recorder::Cursor {
                ts: 0,
                id: "x".to_owned(),
                sort: Some(humanitl_recorder::CursorKey::Int(1)),
            }),
            ..FlowQuery::default()
        })
        .await
        .err()
        .unwrap_or_else(|| panic!("a stale sort value must not be accepted"));
    assert_eq!(err.diagnostic().code.as_str(), "RECORDER_002");
}

#[tokio::test]
async fn a_dropped_response_sink_records_what_it_had() {
    let harness = Harness::with(RecorderSettings::new(64, 1_048_576, 90));
    let flow = FlowId::new();
    harness.recorder.apply(&received(
        flow,
        "api.github.com",
        "/stream",
        SystemTime::now(),
    ));

    let staging = harness.blobs.join("00");
    let expected = humanitl_core::http::sha256(&vec![b'q'; 4_096]);
    {
        let mut sink = harness.recorder.begin_response(flow, &HeaderMap::new());
        sink.chunk(&vec![b'q'; 4_096]);
        assert!(
            temporary_files(&staging) > 0,
            "the sink should have spilled to a temporary file"
        );
        // Der Handler bricht ab, ohne finish oder abort: der Sink fällt.
    }
    harness.recorder.flush().await;

    assert_eq!(
        temporary_files(&staging),
        0,
        "a dropped sink must not leave a .tmp- file behind"
    );

    // Und vor allem: was durchlief, steht in der Datenbank, als gekürzt
    // gekennzeichnet. Stillschweigend verschwinden darf es nicht.
    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    let message = detail
        .messages
        .iter()
        .find(|message| message.dir == Dir::Response)
        .unwrap_or_else(|| panic!("a dropped sink recorded nothing at all"));
    assert!(message.body.truncated, "the body must be marked truncated");
    assert_eq!(message.body.size, 4_096);
    assert_eq!(message.body.sha256, expected);
    assert_eq!(detail.summary.response_size, Some(4_096));
    assert_eq!(
        detail.summary.status, None,
        "a dropped sink knows no status and must not invent one"
    );

    let body = harness
        .recorder
        .read_body(&message.body)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(body.len(), 4_096);
}

/// Wie viele angefangene Blob-Dateien in einem Verzeichnis liegen.
fn temporary_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".tmp-"))
        })
        .count()
}

#[tokio::test]
async fn one_large_chunk_does_not_land_in_memory() {
    let harness = Harness::open();
    let flow = FlowId::new();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/big", SystemTime::now()));

    let mut sink = harness.recorder.begin_response(flow, &HeaderMap::new());
    // Ein einziges Stück, sechzehnmal so groß wie die Inline-Grenze.
    sink.chunk(&vec![b'w'; 4 * 1024 * 1024]);
    assert!(
        sink.buffered_bytes() <= 256 * 1024,
        "one large frame kept {} bytes in memory",
        sink.buffered_bytes()
    );
    let body = sink.finish(200).unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;
    assert_eq!(body.size, 4 * 1024 * 1024);
    assert!(harness.recorder.blobs().contains(&body.sha256));
}

#[tokio::test]
async fn an_old_database_gets_its_host_keys_backfilled() {
    // Eine Datenbank aus der Zeit vor V3: nur V1 und V2 gelaufen, eine Zeile
    // drin, `host_rev` gibt es noch nicht. Nach dem Öffnen muss der Filter
    // `host:` sie finden, sonst verlöre ein Bestand beim Update seine History.
    let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
    let db = dir.path().join("humanitl.db");
    let session = SessionId::new();
    let flow = FlowId::new();
    {
        let conn = rusqlite::Connection::open(&db).unwrap_or_else(|err| panic!("{err}"));
        for migration in humanitl_recorder::MIGRATIONS.iter().take(2) {
            conn.execute_batch(migration.sql)
                .unwrap_or_else(|err| panic!("{err}"));
        }
        conn.execute_batch("PRAGMA user_version = 2;")
            .unwrap_or_else(|err| panic!("{err}"));
        conn.execute(
            "INSERT INTO sessions (id, started_at, sandbox_profile, work_dir, agent) \
             VALUES (?1, 0, 'default', '/w', 'opencode')",
            rusqlite::params![session.to_string()],
        )
        .unwrap_or_else(|err| panic!("{err}"));
        conn.execute(
            "INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, \
             port, path, state, request_size) VALUES (?1, ?2, 1, 1, 'GET', 'https', \
             'api.github.com', 'api.github.com', 443, '/user', 'recorded', 0)",
            rusqlite::params![flow.to_string(), session.to_string()],
        )
        .unwrap_or_else(|err| panic!("{err}"));
    }

    let recorder = Recorder::open(&db, &dir.path().join("blobs"), RecorderSettings::default())
        .unwrap_or_else(|err| panic!("{err}"));
    let page = recorder
        .list_flows(&FlowQuery::new("host:github.com"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(
        page.rows.len(),
        1,
        "a row written before V3 must keep its host filter"
    );
    assert_eq!(page.rows.first().map(|row| row.id), Some(flow));
}

#[tokio::test]
async fn a_block_for_a_secret_keeps_its_reason() {
    let harness = Harness::open();
    let at = SystemTime::now();
    let flow = FlowId::new();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/user", at));
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: flow,
        at,
        decision: Decision::Block {
            reason: BlockReason::Secret,
            note: None,
        },
        source: DecisionSource::System,
    });
    harness.recorder.flush().await;

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(detail.summary.decision.as_deref(), Some("block"));
    assert_eq!(detail.summary.block_reason.as_deref(), Some("secret"));

    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("reason:secret"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.rows.len(), 1);
}

#[tokio::test]
async fn seq_monotonic_per_session() {
    let harness = Harness::open();
    let base = SystemTime::now();
    let mut flows = Vec::new();
    for index in 0..25_u64 {
        let flow = FlowId::new();
        harness.recorder.apply(&received(
            flow,
            "api.github.com",
            "/user",
            base + Duration::from_millis(index),
        ));
        flows.push(flow);
    }
    harness.recorder.flush().await;

    let mut seen = Vec::new();
    for flow in &flows {
        let detail = harness
            .recorder
            .get_flow(*flow)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .unwrap_or_else(|| panic!("flow {flow} is missing"));
        assert_eq!(detail.summary.session, harness.session);
        seen.push(detail.summary.seq);
    }
    seen.sort_unstable();
    assert_eq!(seen, (1..=25).collect::<Vec<i64>>(), "seq has a gap");
}

#[tokio::test]
async fn a_thousand_events_land_within_a_second() {
    let harness = Harness::open();
    let base = SystemTime::now();
    let started = Instant::now();
    for index in 0..1_000_u64 {
        let flow = FlowId::new();
        harness.recorder.apply(&received(
            flow,
            "api.github.com",
            "/user",
            base + Duration::from_millis(index),
        ));
    }
    harness.recorder.flush().await;
    let elapsed = started.elapsed();

    let page = harness
        .recorder
        .list_flows(&FlowQuery {
            limit: 1_000,
            ..FlowQuery::default()
        })
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.rows.len(), 1_000);
    println!("1000 events persisted in {elapsed:?}");

    // Die Vorgabe aus `backlog/sprint-2.md` ist eine Sekunde. Sie gilt für das
    // Profil, mit dem der Daemon läuft; im Debug-Profil liegt `SQLite` mit
    // `-O0` und die übrige Testreihe rechnet nebenher auf denselben Kernen.
    // Gemessen am 2026-09-03: Release 0,18 s, Debug 0,5 bis 0,9 s.
    let budget = if cfg!(debug_assertions) {
        Duration::from_secs(3)
    } else {
        Duration::from_secs(1)
    };
    assert!(
        elapsed < budget,
        "1000 events took {elapsed:?}, the budget is {budget:?}"
    );
}

#[tokio::test]
async fn inline_vs_blob() {
    let harness = Harness::open();
    let base = SystemTime::now();

    let small = FlowId::new();
    harness
        .recorder
        .apply(&received(small, "api.github.com", "/small", base));
    let small_body = Bytes::from(vec![b'a'; 100 * 1024]);
    let small_ref = harness
        .recorder
        .store_message(small, Dir::Request, &json_headers(), small_body.clone())
        .await
        .unwrap_or_else(|err| panic!("{err}"));

    let large = FlowId::new();
    harness
        .recorder
        .apply(&received(large, "api.github.com", "/large", base));
    let large_body = Bytes::from(vec![b'b'; 300 * 1024]);
    let large_ref = harness
        .recorder
        .store_message(large, Dir::Request, &json_headers(), large_body.clone())
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let small_detail = harness
        .recorder
        .get_flow(small)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("small flow missing"));
    let message = small_detail
        .messages
        .first()
        .unwrap_or_else(|| panic!("no message"));
    assert_eq!(message.dir, Dir::Request);
    assert!(message.body.inline.is_some(), "100 KB must stay inline");
    assert_eq!(message.body.size, 100 * 1024);
    assert!(!harness.recorder.blobs().contains(&small_ref.sha256));
    assert_eq!(
        message.headers,
        vec![
            ("host".to_owned(), "api.github.com".to_owned()),
            ("content-type".to_owned(), "application/json".to_owned()),
        ]
    );

    let large_detail = harness
        .recorder
        .get_flow(large)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("large flow missing"));
    let message = large_detail
        .messages
        .first()
        .unwrap_or_else(|| panic!("no message"));
    assert!(message.body.inline.is_none(), "300 KB must go to a blob");
    assert_eq!(message.body.size, 300 * 1024);
    assert_eq!(message.body.sha256, large_ref.sha256);
    assert!(
        harness.recorder.blobs().contains(&large_ref.sha256),
        "the blob file is missing"
    );

    let read_back = harness
        .recorder
        .read_body(&message.body)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(read_back, large_body);
}

#[tokio::test]
async fn a_body_over_the_cap_is_stored_truncated_and_marked() {
    let harness = Harness::with(RecorderSettings::new(1_024, 4_096, 90));
    let flow = FlowId::new();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/big", SystemTime::now()));
    let body = Bytes::from(vec![b'c'; 10_000]);
    let body_ref = harness
        .recorder
        .store_message(flow, Dir::Request, &json_headers(), body)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    assert!(body_ref.truncated);
    assert_eq!(body_ref.size, 10_000, "size stays the size on the wire");

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    let message = detail
        .messages
        .first()
        .unwrap_or_else(|| panic!("no message"));
    assert!(message.body.truncated);
    assert_eq!(message.body.size, 10_000);
    let stored = harness
        .recorder
        .read_body(&message.body)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(stored.len(), 4_096, "only the cap is kept");
}

#[tokio::test]
async fn response_sink_streaming() {
    let harness = Harness::open();
    let flow = FlowId::new();
    harness.recorder.apply(&received(
        flow,
        "api.github.com",
        "/stream",
        SystemTime::now(),
    ));

    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/event-stream"),
    );
    let mut sink = harness.recorder.begin_response(flow, &headers);

    let chunk = vec![b'z'; 1024];
    let mut peak = 0;
    for _ in 0..(10 * 1024) {
        sink.chunk(&chunk);
        peak = peak.max(sink.buffered_bytes());
    }
    assert!(
        peak < 1024 * 1024,
        "the sink kept {peak} bytes in memory, more than 1 MiB"
    );

    let body = sink.finish(200).unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    assert_eq!(body.size, 10 * 1024 * 1024);
    assert!(!body.truncated);
    let expected = humanitl_core::http::sha256(&vec![b'z'; 10 * 1024 * 1024]);
    assert_eq!(body.sha256, expected, "the hash covers every chunk");
    assert!(harness.recorder.blobs().contains(&body.sha256));

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(detail.summary.response_size, Some(10 * 1024 * 1024));
    assert_eq!(detail.summary.status, Some(200));
    let message = detail
        .messages
        .iter()
        .find(|message| message.dir == Dir::Response)
        .unwrap_or_else(|| panic!("no response"));
    assert_eq!(message.content_type, Some("text/event-stream".to_owned()));
}

#[tokio::test]
async fn an_aborted_response_is_recorded_as_truncated() {
    let harness = Harness::open();
    let flow = FlowId::new();
    harness.recorder.apply(&received(
        flow,
        "api.github.com",
        "/abort",
        SystemTime::now(),
    ));
    let mut sink = harness.recorder.begin_response(flow, &HeaderMap::new());
    sink.chunk(b"half a response");
    let body = sink.abort().unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    assert!(body.truncated);
    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    let message = detail
        .messages
        .iter()
        .find(|message| message.dir == Dir::Response)
        .unwrap_or_else(|| panic!("no response"));
    assert!(message.body.truncated);
    assert_eq!(message.body.size, 15);
}

#[tokio::test]
async fn the_state_machine_is_mirrored_into_the_columns() {
    let harness = Harness::open();
    let base = SystemTime::now();

    let allowed = full_flow(&harness.recorder, "api.github.com", "/user", base);
    let blocked = FlowId::new();
    harness
        .recorder
        .apply(&received(blocked, "evil.example", "/exfil", base));
    harness.recorder.apply(&FlowEvent::Held {
        flow_id: blocked,
        at: base,
        deadline: Instant::now() + Duration::from_secs(300),
        queue_bytes: 0,
        queue_count: 1,
    });
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: blocked,
        at: base + Duration::from_millis(900),
        decision: Decision::Block {
            reason: BlockReason::User,
            note: Some("nein".to_owned()),
        },
        source: DecisionSource::User,
    });
    harness.recorder.apply(&FlowEvent::Recorded {
        flow_id: blocked,
        at: base + Duration::from_millis(950),
    });
    harness.recorder.flush().await;

    let allowed = harness
        .recorder
        .get_flow(allowed)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(allowed.summary.state, "recorded");
    assert_eq!(allowed.summary.decision.as_deref(), Some("allow"));
    assert_eq!(allowed.summary.status, Some(200));
    assert_eq!(allowed.summary.held_ms, Some(1_200));
    assert_eq!(allowed.summary.duration_ms, Some(1_500));
    assert!(!allowed.summary.edited);
    assert!(!allowed.summary.passthrough);

    let blocked = harness
        .recorder
        .get_flow(blocked)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(blocked.summary.state, "recorded");
    assert_eq!(blocked.summary.decision.as_deref(), Some("block"));
    assert_eq!(blocked.summary.block_reason.as_deref(), Some("user"));
    assert_eq!(blocked.summary.held_ms, Some(900));
    assert_eq!(blocked.summary.status, None);
    assert!(
        !format!("{blocked:?}").contains("nein"),
        "the note of the user has no column and must not leak into one"
    );
}

#[tokio::test]
async fn a_rule_decision_keeps_the_rule_and_a_passthrough_is_marked() {
    let harness = Harness::open();
    let rule = humanitl_core::RuleId::new();
    let base = SystemTime::now();

    let by_rule = FlowId::new();
    harness
        .recorder
        .apply(&received(by_rule, "registry.npmjs.org", "/react", base));
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: by_rule,
        at: base,
        decision: Decision::Allow,
        source: DecisionSource::Rule(rule),
    });

    let passthrough = FlowId::new();
    harness.recorder.apply(&received(
        passthrough,
        "llm.example",
        "/v1/chat/completions",
        base,
    ));
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: passthrough,
        at: base,
        decision: Decision::Allow,
        source: DecisionSource::Passthrough,
    });
    harness.recorder.flush().await;

    let by_rule = harness
        .recorder
        .get_flow(by_rule)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(
        by_rule.summary.rule_id.as_deref(),
        Some(rule.to_string().as_str())
    );

    let passthrough = harness
        .recorder
        .get_flow(passthrough)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert!(passthrough.summary.passthrough);

    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("passthrough:true"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.rows.len(), 1);
}

#[tokio::test]
async fn findings_are_stored_without_their_value() {
    let harness = Harness::open();
    let flow = FlowId::new();
    harness.recorder.apply(&received(
        flow,
        "api.github.com",
        "/user",
        SystemTime::now(),
    ));
    let finding = Finding::new(
        FindingKind::ApiKey("github".to_owned()),
        0..20,
        FindingLocation::Header(humanitl_core::HeaderName::from_static("authorization")),
        Tier::Checksum,
        "ghp_0123456789abcdef",
    );
    harness
        .recorder
        .store_findings(flow, core::slice::from_ref(&finding));
    harness.recorder.flush().await;

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(detail.summary.findings_count, 1);
    let stored = detail
        .findings
        .first()
        .unwrap_or_else(|| panic!("no finding"));
    assert_eq!(stored.kind, "api_key:github");
    assert_eq!(stored.location, "header:authorization");
    assert_eq!(stored.tier, "checksum");
    assert_eq!(stored.value_hash, finding.value_hash);
    assert_eq!(stored.display_prefix, "ghp_0123…");
    assert!(
        !format!("{detail:?}").contains("456789abcdef"),
        "the secret itself must never be stored"
    );
}

#[tokio::test]
async fn filter_host_suffix() {
    let harness = Harness::open();
    let base = SystemTime::now();
    for (index, host) in [
        "api.github.com",
        "github.com",
        "evil-github.com",
        "github.com.evil.io",
    ]
    .into_iter()
    .enumerate()
    {
        let at = base + Duration::from_millis(index as u64);
        full_flow(&harness.recorder, host, "/x", at);
    }
    harness.recorder.flush().await;

    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("host:github.com"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    let hosts: Vec<&str> = page.rows.iter().map(|row| row.host.as_str()).collect();
    assert!(hosts.contains(&"api.github.com"));
    assert!(hosts.contains(&"github.com"));
    assert!(!hosts.contains(&"evil-github.com"));
    assert!(!hosts.contains(&"github.com.evil.io"));
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.total_estimate, 2);
}

#[tokio::test]
async fn filter_since_relative() {
    let harness = Harness::open();
    let now = SystemTime::now();
    full_flow(
        &harness.recorder,
        "old.example",
        "/x",
        now - Duration::from_secs(3_600),
    );
    full_flow(
        &harness.recorder,
        "fresh.example",
        "/x",
        now - Duration::from_secs(60),
    );
    harness.recorder.flush().await;

    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("since:10m"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.rows.len(), 1);
    assert_eq!(
        page.rows.first().map(|row| row.host.as_str()),
        Some("fresh.example")
    );

    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("since:2h"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.rows.len(), 2);
}

#[tokio::test]
async fn filter_unknown_key_diag() {
    let harness = Harness::open();
    let err = harness
        .recorder
        .list_flows(&FlowQuery::new("foo:bar"))
        .await
        .err()
        .unwrap_or_else(|| panic!("an unknown key must not be ignored"));
    let diagnostic = err.diagnostic();
    assert_eq!(diagnostic.code.as_str(), "RECORDER_002");
    assert!(diagnostic.why.contains("foo"), "{}", diagnostic.why);
    assert!(diagnostic.why.contains("host"), "{}", diagnostic.why);
    assert!(matches!(err, humanitl_recorder::RecorderError::Filter(_)));
}

#[tokio::test]
async fn cursor_paging_no_dupes_no_gaps() {
    let harness = Harness::open();
    let base = SystemTime::now() - Duration::from_secs(600);
    for index in 0..500_u64 {
        let flow = FlowId::new();
        harness.recorder.apply(&received(
            flow,
            "api.github.com",
            "/user",
            base + Duration::from_millis(index),
        ));
    }
    harness.recorder.flush().await;

    let mut seen = std::collections::HashSet::new();
    let mut query = FlowQuery {
        limit: 200,
        ..FlowQuery::default()
    };
    let mut pages = 0;
    loop {
        let page = harness
            .recorder
            .list_flows(&query)
            .await
            .unwrap_or_else(|err| panic!("{err}"));
        pages += 1;
        for row in &page.rows {
            assert!(seen.insert(row.id), "the flow {} came twice", row.id);
        }
        match page.next {
            Some(cursor) if !page.rows.is_empty() => query.cursor = Some(cursor),
            _ => break,
        }
        assert!(pages < 10, "paging does not terminate");
    }
    assert_eq!(seen.len(), 500);
    assert_eq!(pages, 3);
}

#[tokio::test]
async fn paging_by_host_and_by_size_is_also_gapless() {
    let harness = Harness::open();
    let base = SystemTime::now();
    for index in 0..120_u64 {
        let flow = FlowId::new();
        harness.recorder.apply(&received(
            flow,
            &format!("host{index:03}.example"),
            "/x",
            base + Duration::from_millis(index),
        ));
    }
    harness.recorder.flush().await;

    for sort in [SortKey::Host, SortKey::Size, SortKey::Duration] {
        let mut seen = std::collections::HashSet::new();
        let mut query = FlowQuery {
            sort,
            limit: 50,
            ..FlowQuery::default()
        };
        for _round in 0..5 {
            let page = harness
                .recorder
                .list_flows(&query)
                .await
                .unwrap_or_else(|err| panic!("{err}"));
            for row in &page.rows {
                assert!(seen.insert(row.id), "duplicate while sorting by {sort:?}");
            }
            match page.next {
                Some(cursor) if !page.rows.is_empty() => query.cursor = Some(cursor),
                _ => break,
            }
        }
        assert_eq!(seen.len(), 120, "sorting by {sort:?} lost rows");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_read_during_write() {
    let harness = Harness::open();
    let base = SystemTime::now();

    let writer = harness.recorder.clone();
    let writing = tokio::task::spawn_blocking(move || {
        for index in 0..10_000_u64 {
            let flow = FlowId::new();
            writer.apply(&received(
                flow,
                "api.github.com",
                "/user",
                base + Duration::from_millis(index),
            ));
        }
    });

    let mut readers = Vec::new();
    for _reader in 0..50 {
        let recorder = harness.recorder.clone();
        readers.push(tokio::spawn(async move {
            recorder
                .list_flows(&FlowQuery {
                    limit: 50,
                    ..FlowQuery::default()
                })
                .await
        }));
    }

    for reader in readers {
        let page = reader
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .unwrap_or_else(|err| panic!("SQLITE_BUSY or worse: {err}"));
        assert!(page.rows.len() <= 50);
    }
    writing.await.unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let page = harness
        .recorder
        .list_flows(&FlowQuery {
            limit: 1,
            ..FlowQuery::default()
        })
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(
        page.total_estimate, 10_000,
        "the count is exact below the ceiling"
    );
}

#[tokio::test]
async fn retention_deletes_what_it_promises_and_nothing_else() {
    let harness = Harness::with(RecorderSettings::new(64, 4_096, 1));
    let now = SystemTime::now();
    let old_at = now - Duration::from_secs(3 * 24 * 60 * 60);

    let old = FlowId::new();
    harness
        .recorder
        .apply(&received(old, "old.example", "/x", old_at));
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
    harness.recorder.store_findings(
        old,
        &[Finding::new(
            FindingKind::Email,
            0..5,
            FindingLocation::Body,
            Tier::Regex,
            "a@b.de",
        )],
    );

    let fresh = FlowId::new();
    harness
        .recorder
        .apply(&received(fresh, "fresh.example", "/x", now));
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

    assert!(harness.recorder.blobs().contains(&old_body.sha256));
    assert!(harness.recorder.blobs().contains(&fresh_body.sha256));

    let report = harness
        .recorder
        .purge_expired(now)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(report.flows, 1);
    assert_eq!(report.messages, 1);
    assert_eq!(report.findings, 1);
    assert_eq!(report.blobs, 1);

    assert!(
        harness
            .recorder
            .get_flow(old)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .is_none()
    );
    assert!(
        harness
            .recorder
            .get_flow(fresh)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .is_some()
    );
    assert!(!harness.recorder.blobs().contains(&old_body.sha256));
    assert!(
        harness.recorder.blobs().contains(&fresh_body.sha256),
        "retention must not touch a blob that is still referenced"
    );
}

#[tokio::test]
async fn a_shared_blob_survives_the_deletion_of_one_of_its_flows() {
    let harness = Harness::with(RecorderSettings::new(64, 4_096, 1));
    let now = SystemTime::now();
    let old_at = now - Duration::from_secs(3 * 24 * 60 * 60);
    let body = Bytes::from(vec![b's'; 1_000]);

    let old = FlowId::new();
    harness
        .recorder
        .apply(&received(old, "old.example", "/x", old_at));
    let old_ref = harness
        .recorder
        .store_message(old, Dir::Request, &json_headers(), body.clone())
        .await
        .unwrap_or_else(|err| panic!("{err}"));

    let fresh = FlowId::new();
    harness
        .recorder
        .apply(&received(fresh, "fresh.example", "/x", now));
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
    assert_eq!(report.flows, 1);
    assert_eq!(
        report.blobs, 0,
        "the blob is still in use by the fresh flow"
    );
    assert!(harness.recorder.blobs().contains(&old_ref.sha256));
}

#[tokio::test]
async fn a_blob_without_a_row_is_swept_on_the_next_start() {
    let harness = Harness::open();
    harness.recorder.flush().await;

    // Ein Absturz zwischen Blob und Zeile: die Datei ist da, die Zeile nicht.
    let orphan = humanitl_core::http::sha256(b"orphan");
    harness
        .recorder
        .blobs()
        .put(&orphan, b"orphan")
        .unwrap_or_else(|err| panic!("{err}"));
    let path = harness.recorder.blobs().path(&orphan);
    let old = std::time::SystemTime::now() - Duration::from_secs(48 * 60 * 60);
    let file = std::fs::File::open(&path).unwrap_or_else(|err| panic!("{err}"));
    file.set_times(std::fs::FileTimes::new().set_modified(old))
        .unwrap_or_else(|err| panic!("{err}"));
    drop(file);

    // Eine abgebrochene Temp-Datei aus demselben Absturz.
    let (temp_file, temp_path) = harness
        .recorder
        .blobs()
        .temp(&orphan)
        .unwrap_or_else(|err| panic!("{err}"));
    drop(temp_file);
    assert!(temp_path.is_file());

    drop(harness.recorder);
    let restarted = Recorder::open(&harness.db, &harness.blobs, RecorderSettings::default())
        .unwrap_or_else(|err| panic!("{err}"));
    assert!(
        !restarted.blobs().contains(&orphan),
        "a blob nobody points at must not survive a restart"
    );
    assert!(!temp_path.is_file(), "a temporary file must not survive");
}

#[tokio::test]
async fn a_rule_snapshot_outlives_the_rule() {
    let harness = Harness::open();
    let rule = humanitl_core::RuleId::new();
    harness
        .recorder
        .snapshot_rule(rule, "action: allow\nmatch:\n  host: \"*.github.com\"\n");
    harness.recorder.forget_rule(rule);
    harness.recorder.flush().await;

    // Der Schnappschuss bleibt lesbar; geprüft über das Aufräumen, das ihn
    // erst mit der Frist entfernt.
    let report = harness
        .recorder
        .purge_before(SystemTime::now() - Duration::from_secs(3_600))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(report.flows, 0);
}

#[tokio::test]
async fn an_edited_request_is_marked_and_stored_separately() {
    let harness = Harness::open();
    let flow = FlowId::new();
    let at = SystemTime::now();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/user", at));
    harness
        .recorder
        .store_message(
            flow,
            Dir::Request,
            &json_headers(),
            Bytes::from_static(b"original"),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: flow,
        at,
        decision: Decision::AllowEdited {
            request: Box::new(request("api.github.com", "/user")),
        },
        source: DecisionSource::User,
    });
    harness
        .recorder
        .store_message(
            flow,
            Dir::RequestEdited,
            &json_headers(),
            Bytes::from_static(b"redacted"),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    harness.recorder.flush().await;

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert!(detail.summary.edited);
    assert_eq!(detail.summary.decision.as_deref(), Some("allow_edited"));
    assert_eq!(detail.messages.len(), 2);

    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("edited:true"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.rows.len(), 1);
}

#[tokio::test]
async fn a_flow_arriving_without_a_session_becomes_a_diagnostic() {
    let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
    let recorder = Recorder::open(
        &dir.path().join("humanitl.db"),
        &dir.path().join("blobs"),
        RecorderSettings::default(),
    )
    .unwrap_or_else(|err| panic!("{err}"));
    let mut diagnostics = recorder.diagnostics();

    recorder.apply(&received(
        FlowId::new(),
        "api.github.com",
        "/user",
        SystemTime::now(),
    ));
    recorder.flush().await;

    let diagnostic = diagnostics
        .try_recv()
        .unwrap_or_else(|err| panic!("no diagnostic: {err}"));
    assert_eq!(diagnostic.code.as_str(), "RECORDER_003");
    assert!(diagnostic.why.contains("session"), "{}", diagnostic.why);

    // Der Thread lebt weiter und schreibt danach normal.
    let session = SessionId::new();
    recorder.start_session(&SessionMeta {
        id: session,
        started_at: SystemTime::now(),
        sandbox_profile: "default".to_owned(),
        llm_endpoint: None,
        work_dir: "/tmp".to_owned(),
        agent: "opencode".to_owned(),
    });
    let flow = FlowId::new();
    recorder.apply(&received(
        flow,
        "api.github.com",
        "/after",
        SystemTime::now(),
    ));
    recorder.flush().await;
    assert!(
        recorder
            .get_flow(flow)
            .await
            .unwrap_or_else(|err| panic!("{err}"))
            .is_some(),
        "the writer thread must survive a failed row"
    );
}

#[tokio::test]
async fn timestamps_are_unix_milliseconds() {
    let harness = Harness::open();
    let at = SystemTime::now();
    let flow = FlowId::new();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/user", at));
    harness.recorder.flush().await;

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(detail.summary.ts, millis(at));
}

#[tokio::test]
async fn a_flow_can_carry_the_reason_it_failed() {
    // HUM-045: Der Client in der Sandbox bricht den TLS-Handschlag zum Proxy
    // ab. Es gibt keine Anfrage, niemand hat entschieden, und trotzdem soll die
    // History den Versuch samt Grund zeigen.
    let harness = Harness::open();
    let at = SystemTime::now();
    let flow = FlowId::new();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/user", at));
    harness
        .recorder
        .set_flow_error(flow, "tls_handshake_failed");
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: flow,
        at,
        decision: Decision::Block {
            reason: BlockReason::NoRoute,
            note: None,
        },
        source: DecisionSource::System,
    });
    harness
        .recorder
        .apply(&FlowEvent::Recorded { flow_id: flow, at });
    harness.recorder.flush().await;

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(
        detail.summary.error.as_deref(),
        Some("tls_handshake_failed")
    );
    // Der Grund steht neben der Entscheidung, nicht an ihrer Stelle.
    assert_eq!(detail.summary.block_reason.as_deref(), Some("no_route"));

    let page = harness
        .recorder
        .list_flows(&FlowQuery::default())
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(
        page.rows.first().and_then(|row| row.error.as_deref()),
        Some("tls_handshake_failed"),
        "the flow list carries the reason as well"
    );
}

#[tokio::test]
async fn a_failed_upstream_names_itself_in_the_error_column() {
    let harness = Harness::open();
    let at = SystemTime::now();
    let flow = FlowId::new();
    harness
        .recorder
        .apply(&received(flow, "api.github.com", "/user", at));
    harness.recorder.apply(&FlowEvent::Failed {
        flow_id: flow,
        at,
        error: humanitl_core::UpstreamError::Dns,
    });
    harness.recorder.flush().await;

    let detail = harness
        .recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(detail.summary.state, "failed");
    assert_eq!(detail.summary.error.as_deref(), Some("upstream_dns"));
}

#[tokio::test]
async fn an_old_database_gets_the_error_column() {
    // Eine Datenbank aus der Zeit vor V4: Die Migration darf die vorhandenen
    // Zeilen nicht anfassen und muss die Spalte leer nachrüsten.
    let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
    let db = dir.path().join("humanitl.db");
    let session = SessionId::new();
    let flow = FlowId::new();
    {
        let conn = rusqlite::Connection::open(&db).unwrap_or_else(|err| panic!("{err}"));
        for migration in humanitl_recorder::MIGRATIONS.iter().take(3) {
            conn.execute_batch(migration.sql)
                .unwrap_or_else(|err| panic!("{err}"));
        }
        conn.execute_batch("PRAGMA user_version = 3;")
            .unwrap_or_else(|err| panic!("{err}"));
        conn.execute(
            "INSERT INTO sessions (id, started_at, sandbox_profile, work_dir, agent) \
             VALUES (?1, 0, 'default', '/w', 'opencode')",
            rusqlite::params![session.to_string()],
        )
        .unwrap_or_else(|err| panic!("{err}"));
        conn.execute(
            "INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, \
             host_rev, port, path, state, request_size) VALUES (?1, ?2, 1, 1, 'GET', 'https', \
             'api.github.com', 'api.github.com', 'com.github.api.', 443, '/user', 'recorded', 0)",
            rusqlite::params![flow.to_string(), session.to_string()],
        )
        .unwrap_or_else(|err| panic!("{err}"));
    }

    let recorder = Recorder::open(&db, &dir.path().join("blobs"), RecorderSettings::default())
        .unwrap_or_else(|err| panic!("{err}"));
    let detail = recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(detail.summary.error, None, "an old row keeps its data");

    recorder.set_flow_error(flow, "tls_handshake_failed");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow)
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap_or_else(|| panic!("flow missing"));
    assert_eq!(
        detail.summary.error.as_deref(),
        Some("tls_handshake_failed")
    );
}
