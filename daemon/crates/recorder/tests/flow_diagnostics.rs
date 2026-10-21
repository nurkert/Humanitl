//! Die Befunde eines Flusses in der Aufzeichnung (HUM-256).
//!
//! Ein Befund am Fluss (`LIMIT_001` mit dem Vorschlag einer Regel,
//! `FINDINGS_002`, `LIMIT_004` ...) lebte bis hierher nur in der Registry des
//! laufenden Daemons. Diese Tests prüfen gegen eine echte Datenbank, dass er
//! einen Neustart übersteht, in Reihenfolge und mit seinem Vorschlag, dass die
//! Grenze der Registry auch hier gilt, dass die Aufbewahrung ihn mit seinem
//! Fluss löscht, dass ein Geheimnis im `why` maskiert ankommt und dass eine
//! Datenbank aus der Zeit vor der Migration lesbar bleibt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use humanitl_core::diagnostics::codes::{FINDINGS_002, LIMIT_001, LIMIT_004};
use humanitl_core::rule::{Action, HostPattern, Matcher, Rule};
use humanitl_core::{
    Authority, Diagnostic, FixAction, FlowEvent, FlowId, HostName, HttpRequest,
    MAX_FLOW_DIAGNOSTICS, Method, RuleId, Scheme, SessionId, Severity,
};
use humanitl_recorder::{Recorder, RecorderSettings, SessionMeta};

/// Ein Datenordner, in dem nacheinander mehrere Daemons laufen können.
struct Data {
    _dir: tempfile::TempDir,
    db: PathBuf,
    blobs: PathBuf,
}

impl Data {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("data").join("humanitl.db");
        let blobs = dir.path().join("data").join("blobs");
        Self {
            _dir: dir,
            db,
            blobs,
        }
    }

    /// Ein Daemon auf diesem Ordner, mit einer laufenden Sitzung.
    fn open(&self, settings: RecorderSettings, session: SessionId) -> Recorder {
        let recorder = Recorder::open(&self.db, &self.blobs, settings).unwrap();
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/home/x/projekt".to_owned(),
            agent: "opencode".to_owned(),
        });
        recorder
    }
}

/// Ein angekommener Flow an `host`.
fn arrive(recorder: &Recorder, host: &str, at: SystemTime) -> FlowId {
    let flow = FlowId::new();
    let host = HostName::parse(host).unwrap();
    recorder.apply(&FlowEvent::Received {
        flow_id: flow,
        at,
        request: Box::new(HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(host, Scheme::Https),
            "/upload",
        )),
    });
    flow
}

/// Die Regel, die `LIMIT_001` vorschlägt, in ihrer Form.
fn stream_rule() -> Rule {
    Rule::new(
        RuleId::new(),
        Action::Ask,
        Matcher::host(HostPattern::Exact(
            HostName::parse("upload.example.com").unwrap(),
        ))
        .with_path_prefixes(vec!["/upload".to_owned()]),
    )
    .with_stream(true)
}

/// Ein Befund mit diesem Code, Grund und Vorschlag.
fn diagnostic(
    code: humanitl_core::DiagnosticCode,
    why: &str,
    fix: Option<FixAction>,
) -> Diagnostic {
    let builder = Diagnostic::builder(code, Severity::Warning).why(why.to_owned());
    match fix {
        Some(fix) => builder.fix(fix).build(),
        None => builder.build(),
    }
}

/// Ein Stellvertreter für `humanitl_rules::serialize_rules`, den diese Crate
/// nicht kennt: Er schreibt nur die Id, und [`decode`] findet die Regel daran
/// wieder. Geprüft wird hier der Weg durch die Spalten; die echte
/// Serialisierung prüfen der Proxy und `humanitl-ipc`.
fn encode(rule: &Rule) -> String {
    format!("rule {}", rule.id)
}

/// Die Umkehrung von [`encode`] gegen eine bekannte Regel.
fn decode(known: &Rule) -> impl Fn(&str) -> Option<Rule> + '_ {
    move |text| (text == encode(known)).then(|| known.clone())
}

/// Die Zeilen einer Tabelle, direkt aus der Datei gezählt.
fn rows(db: &Path, sql: &str) -> i64 {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Ein am Cap geblockter Fluss behält nach einem Neustart seine Befunde, in
/// der Reihenfolge ihres Eintreffens und mit demselben Vorschlag.
#[tokio::test]
async fn the_diagnostics_of_a_flow_survive_a_restart_in_order_with_their_fix() {
    let data = Data::new();
    let session = SessionId::new();
    let rule = stream_rule();
    let flow = {
        let recorder = data.open(RecorderSettings::default(), session);
        let flow = arrive(&recorder, "upload.example.com", SystemTime::now());
        for diagnostic in [
            diagnostic(
                FINDINGS_002,
                "the scan stopped early",
                Some(FixAction::ChangeSetting {
                    key: "limits.scan_max_bytes".to_owned(),
                    value: "2097152".to_owned(),
                }),
            ),
            diagnostic(
                LIMIT_001,
                "the request body is 41943040 bytes",
                Some(FixAction::AddRule(Box::new(rule.clone()))),
            ),
            diagnostic(LIMIT_004, "the response went silent", None),
        ] {
            recorder.store_diagnostic(flow, &diagnostic, encode);
        }
        recorder.flush().await;
        flow
    };

    // Der neue Daemon: leere Registry, dieselbe Datei.
    let recorder = data.open(RecorderSettings::default(), SessionId::new());
    let detail = recorder.get_flow(flow).await.unwrap().unwrap();
    let codes: Vec<&str> = detail.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["FINDINGS_002", "LIMIT_001", "LIMIT_004"]);
    let indices: Vec<u32> = detail.diagnostics.iter().map(|d| d.idx).collect();
    assert_eq!(indices, [0, 1, 2]);

    let limit = &detail.diagnostics[1];
    assert_eq!(limit.severity(), Some(Severity::Warning));
    assert_eq!(limit.why, "the request body is 41943040 bytes");
    assert_eq!(
        limit.title,
        humanitl_core::lookup(LIMIT_001).unwrap().title,
        "the title of the register"
    );
    assert_eq!(
        limit.fix(decode(&rule)),
        Some(FixAction::AddRule(Box::new(rule.clone()))),
        "the suggestion is the rule that was offered"
    );
    assert_eq!(
        detail.diagnostics[0].fix(decode(&rule)),
        Some(FixAction::ChangeSetting {
            key: "limits.scan_max_bytes".to_owned(),
            value: "2097152".to_owned(),
        })
    );
    assert_eq!(detail.diagnostics[2].fix(decode(&rule)), None);
}

/// Höchstens [`MAX_FLOW_DIAGNOSTICS`] Befunde je Fluss, die ersten, wie in
/// der Registry; ein zweiter Fluss hat seine eigene Grenze.
#[tokio::test]
async fn a_flow_keeps_at_most_the_first_max_flow_diagnostics() {
    let data = Data::new();
    let recorder = data.open(RecorderSettings::default(), SessionId::new());
    let busy = arrive(&recorder, "upload.example.com", SystemTime::now());
    let quiet = arrive(&recorder, "upload.example.com", SystemTime::now());
    for n in 0..2 * MAX_FLOW_DIAGNOSTICS {
        recorder.store_diagnostic(busy, &diagnostic(LIMIT_004, &format!("#{n}"), None), encode);
    }
    recorder.store_diagnostic(quiet, &diagnostic(LIMIT_001, "once", None), encode);
    recorder.flush().await;

    let detail = recorder.get_flow(busy).await.unwrap().unwrap();
    let whys: Vec<String> = detail.diagnostics.iter().map(|d| d.why.clone()).collect();
    let expected: Vec<String> = (0..MAX_FLOW_DIAGNOSTICS).map(|n| format!("#{n}")).collect();
    assert_eq!(whys, expected, "the first ones, in order");
    assert_eq!(
        rows(
            &data.db,
            &format!("SELECT COUNT(*) FROM diagnostics WHERE flow_id = '{busy}'")
        ),
        i64::try_from(MAX_FLOW_DIAGNOSTICS).unwrap(),
        "nothing over the limit reaches the table"
    );
    assert_eq!(
        recorder
            .get_flow(quiet)
            .await
            .unwrap()
            .unwrap()
            .diagnostics
            .len(),
        1
    );
    assert!(
        recorder.outage().is_empty(),
        "a dropped diagnostic is no write failure"
    );
}

/// Die Aufbewahrung löscht die Befunde mit ihrem Fluss, und nur die.
#[tokio::test]
async fn retention_removes_the_diagnostics_with_their_flow() {
    let data = Data::new();
    let recorder = data.open(RecorderSettings::new(64, 4_096, 1), SessionId::new());
    let now = SystemTime::now();
    let old = arrive(
        &recorder,
        "old.example",
        now - Duration::from_secs(3 * 86_400),
    );
    let fresh = arrive(&recorder, "fresh.example", now);
    for flow in [old, fresh] {
        recorder.store_diagnostic(flow, &diagnostic(LIMIT_001, "at the cap", None), encode);
        recorder.store_diagnostic(flow, &diagnostic(LIMIT_004, "silent", None), encode);
    }
    recorder.flush().await;

    let report = recorder.purge_expired(now).await.unwrap();
    assert_eq!(report.flows, 1);
    assert_eq!(report.diagnostics, 2);
    assert_eq!(
        rows(
            &data.db,
            "SELECT COUNT(*) FROM diagnostics WHERE flow_id NOT IN (SELECT id FROM flows)"
        ),
        0,
        "no orphan"
    );
    assert_eq!(
        recorder
            .get_flow(fresh)
            .await
            .unwrap()
            .unwrap()
            .diagnostics
            .len(),
        2,
        "the fresh flow keeps its own"
    );
}

/// Ein Geheimnis im `why` kommt maskiert an, wie in Kopfzeilen und Pfaden,
/// auch als Label eines Hostnamens in anderer Schreibung.
#[tokio::test]
async fn a_secret_in_the_why_is_masked_like_headers_and_paths() {
    const KEY: &str = "sk-Hum256SecretKeyValue0123456789";
    let data = Data::new();
    let recorder = data.open(RecorderSettings::default(), SessionId::new());
    recorder.mask_secret(KEY, Some("llm.example:443"));
    let flow = arrive(&recorder, "upload.example.com", SystemTime::now());
    let why = format!(
        "the request body to {}.evil.example is too large; path /v1?key={KEY}",
        KEY.to_ascii_lowercase()
    );
    recorder.store_diagnostic(flow, &diagnostic(LIMIT_001, &why, None), encode);
    recorder.flush().await;

    let stored = &recorder.get_flow(flow).await.unwrap().unwrap().diagnostics[0].why;
    assert!(!stored.contains(KEY), "{stored}");
    assert!(!stored.contains(&KEY.to_ascii_lowercase()), "{stored}");
    assert_eq!(
        stored, "the request body to ••••.evil.example is too large; path /v1?key=••••",
        "masked where the key stood, the rest unchanged"
    );
    assert_eq!(
        rows(
            &data.db,
            &format!(
                "SELECT COUNT(*) FROM diagnostics WHERE instr(lower(why), lower('{KEY}')) > 0"
            )
        ),
        0,
        "not even the file holds it"
    );
}

/// Ein Befund zu einem Fluss ohne Zeile fällt still weg, statt als
/// Schreibfehler die Aufzeichnung für gestört zu erklären.
#[tokio::test]
async fn a_diagnostic_of_an_unknown_flow_is_dropped_without_an_outage() {
    let data = Data::new();
    let recorder = data.open(RecorderSettings::default(), SessionId::new());
    let mut diagnostics = recorder.diagnostics();
    recorder.store_diagnostic(
        FlowId::new(),
        &diagnostic(LIMIT_001, "nowhere", None),
        encode,
    );
    recorder.flush().await;
    assert_eq!(rows(&data.db, "SELECT COUNT(*) FROM diagnostics"), 0);
    assert!(recorder.outage().is_empty());
    assert!(diagnostics.try_recv().is_err(), "nothing was reported");
}

/// Eine Datenbank aus der Zeit vor `V10__flow_diagnostics.sql` wird vorwärts
/// migriert; ihre Flüsse bleiben lesbar, ohne Befunde, und ein neuer Befund
/// landet danach an ihnen.
#[tokio::test]
async fn an_old_database_migrates_forward_and_stays_readable() {
    let data = Data::new();
    let session = SessionId::new();
    let flow = FlowId::new();
    std::fs::create_dir_all(data.db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&data.db).unwrap();
        for migration in humanitl_recorder::MIGRATIONS.iter().take(9) {
            conn.execute_batch(migration.sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 9;").unwrap();
        conn.execute(
            "INSERT INTO sessions (id, started_at, sandbox_profile, work_dir, agent) \
             VALUES (?1, 0, 'default', '/w', 'opencode')",
            rusqlite::params![session.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, \
             host_rev, port, path, state, decision, block_reason, request_size) VALUES (?1, ?2, \
             1, 1, 'POST', 'https', 'upload.example.com', 'upload.example.com', \
             'com.example.upload.', 443, '/upload', 'recorded', 'block', 'body_cap', 0)",
            rusqlite::params![flow.to_string(), session.to_string()],
        )
        .unwrap();
    }

    let recorder = data.open(RecorderSettings::default(), session);
    let detail = recorder.get_flow(flow).await.unwrap().unwrap();
    assert_eq!(detail.summary.block_reason.as_deref(), Some("body_cap"));
    assert!(detail.diagnostics.is_empty(), "an old row has none");
    assert_eq!(
        rows(&data.db, "PRAGMA user_version"),
        i64::from(humanitl_recorder::latest_version())
    );

    recorder.store_diagnostic(flow, &diagnostic(LIMIT_001, "later", None), encode);
    recorder.flush().await;
    assert_eq!(
        recorder.get_flow(flow).await.unwrap().unwrap().diagnostics[0].why,
        "later"
    );
}
