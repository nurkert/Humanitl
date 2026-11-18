//! Aufräumen und Vergessen (HUM-533): Plan, Digest, Ausführung in einer
//! Transaktion.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use humanitl_core::{ProjectId, SessionId};
use humanitl_recorder::{
    BlobStore, CleanupKind, CleanupPlan, CleanupScope, CleanupTarget, Executed, KeptReason,
    LiveState, ProjectStart, Recorder, RecorderSettings, SessionMeta, WaitingLayer,
};
use sha2::Digest as _;

struct Harness {
    _dir: tempfile::TempDir,
    db: PathBuf,
    blobs: BlobStore,
    recorder: Recorder,
    seq: std::cell::Cell<i64>,
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds)
}

fn ms(time: SystemTime) -> i64 {
    i64::try_from(time.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap()
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("humanitl.db");
        let blob_dir = dir.path().join("blobs");
        let recorder = Recorder::open(&db, &blob_dir, RecorderSettings::default()).unwrap();
        let blobs = BlobStore::open(&blob_dir).unwrap();
        Self {
            _dir: dir,
            db,
            blobs,
            recorder,
            seq: std::cell::Cell::new(0),
        }
    }

    fn conn(&self) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(&self.db).unwrap();
        conn.busy_timeout(Duration::from_secs(5)).unwrap();
        conn
    }

    fn count(&self, sql: &str) -> i64 {
        self.conn().query_row(sql, [], |row| row.get(0)).unwrap()
    }

    /// Eine Sitzung des Daemons, die in `path` (Inode `ino`) einen Lauf hatte.
    async fn session_in(
        &self,
        path: &str,
        ino: u64,
        started: SystemTime,
    ) -> (SessionId, ProjectId) {
        let id = SessionId::new();
        self.recorder.start_session(&SessionMeta {
            id,
            started_at: started,
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: String::new(),
            agent: "opencode".to_owned(),
        });
        let registered = self
            .recorder
            .register_project_start(ProjectStart {
                session: id,
                path: path.to_owned(),
                dev: 7,
                ino,
                agent: "claude".to_owned(),
                at: started,
            })
            .await
            .unwrap();
        (id, registered.project.id)
    }

    /// Eine Sitzung ohne Projekt.
    async fn unassigned_session(&self, started: SystemTime) -> SessionId {
        let id = SessionId::new();
        self.recorder.start_session(&SessionMeta {
            id,
            started_at: started,
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: String::new(),
            agent: "opencode".to_owned(),
        });
        self.recorder.flush().await;
        id
    }

    async fn end(&self, session: SessionId) {
        self.recorder.end_session(session);
        self.recorder.flush().await;
    }

    /// Ein Fluss mit einer Anfrage, deren Body inline steht, und einer
    /// Antwort, deren Body als Blob liegt.
    async fn flow(&self, session: SessionId, body: &[u8]) -> (String, [u8; 32]) {
        self.flow_at(session, body, at(100)).await
    }

    async fn flow_at(
        &self,
        session: SessionId,
        body: &[u8],
        when: SystemTime,
    ) -> (String, [u8; 32]) {
        self.recorder.flush().await;
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let flow = humanitl_core::FlowId::new().to_string();
        let sha: [u8; 32] = sha2::Sha256::digest(body).into();
        self.blobs.put(&sha, body).unwrap();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, port, \
             path, state) VALUES (?1, ?2, ?3, ?4, 'GET', 'https', 'api.example', 'api.example', \
             443, '/x', 'recorded')",
            rusqlite::params![flow, session.to_string(), seq, ms(when)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (flow_id, dir, headers_json, body_inline, size) \
             VALUES (?1, 'request', '[]', X'0102', 2)",
            [&flow],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (flow_id, dir, headers_json, blob_sha256, size) \
             VALUES (?1, 'response', '[]', ?2, ?3)",
            rusqlite::params![flow, sha.as_slice(), i64::try_from(body.len()).unwrap()],
        )
        .unwrap();
        (flow, sha)
    }

    fn flows_of(&self, session: SessionId) -> i64 {
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM flows WHERE session_id = ?1",
                [session.to_string()],
                |row| row.get(0),
            )
            .unwrap()
    }

    async fn plan(&self, scope: &CleanupScope, live: &LiveState) -> CleanupPlan {
        self.recorder
            .cleanup_plan(scope.clone(), live.clone())
            .await
            .unwrap()
    }

    async fn execute(&self, scope: &CleanupScope, live: &LiveState, digest: &str) -> Executed {
        let recorder = self.recorder.clone();
        let (scope, live, digest) = (scope.clone(), live.clone(), digest.to_owned());
        tokio::task::spawn_blocking(move || recorder.cleanup_blocking(scope, live, digest))
            .await
            .unwrap()
            .unwrap()
    }

    async fn save_choice(&self, project: ProjectId, text: &str) {
        self.recorder
            .save_project_choice(project, Some(text.to_owned()))
            .await
            .unwrap()
            .unwrap();
    }
}

fn projects(ids: &[ProjectId]) -> CleanupScope {
    CleanupScope {
        target: CleanupTarget::Projects(ids.to_vec()),
        before: None,
        include_waiting: false,
    }
}

fn kinds(plan: &CleanupPlan) -> Vec<(CleanupKind, Option<ProjectId>)> {
    plan.items
        .iter()
        .map(|item| (item.kind, item.project))
        .collect()
}

/// Ein Projekt mit zwei Sitzungen: Flüsse, Bodies und Größe stimmen mit der
/// Datenbank.
#[tokio::test]
async fn the_plan_of_a_project_matches_the_database() {
    let h = Harness::new();
    let (first, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.flow(first, b"one body").await;
    h.flow(first, b"another body").await;
    h.end(first).await;
    let (second, same) = h.session_in("/home/u/alpha", 11, at(50)).await;
    assert_eq!(same, alpha);
    h.flow(second, b"third body!").await;
    h.end(second).await;

    let plan = h.plan(&projects(&[alpha]), &LiveState::default()).await;
    let flows = plan
        .items
        .iter()
        .find(|item| item.kind == CleanupKind::RecordedFlows)
        .unwrap();
    assert_eq!(flows.count, 3);
    assert_eq!(flows.sessions, 2);
    let rows: i64 = h
        .count("SELECT SUM(LENGTH(headers_json) + COALESCE(LENGTH(body_inline), 0)) FROM messages")
        + h.count("SELECT SUM(LENGTH(host) + LENGTH(path)) FROM flows");
    let blob_bytes =
        u64::try_from(b"one body".len() + b"another body".len() + b"third body!".len()).unwrap();
    assert_eq!(flows.bytes, u64::try_from(rows).unwrap() + blob_bytes);
    assert_eq!(plan.digest.len(), 64);
    assert_eq!(
        kinds(&plan),
        [
            (CleanupKind::RecordedFlows, Some(alpha)),
            (CleanupKind::RegisterEntry, Some(alpha)),
        ]
    );
}

/// Die Ausführung entfernt genau die Flüsse des Projekts; die eines anderen
/// Projekts, die Anker der Audit-Kette und die Zeilen der Sitzungen bleiben.
///
/// Mutationsbeweis: Ohne den Filter auf das Projekt in der Auswahl der
/// Sitzungen (`project_id = ?1` entfernt) fallen die Flüsse von `beta` mit,
/// und der Test wird rot.
#[tokio::test]
async fn cleanup_removes_only_the_flows_of_the_project() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    let (_flow, gone) = h.flow(a, b"alpha body").await;
    h.end(a).await;
    let (b, _beta) = h.session_in("/home/u/beta", 12, at(10)).await;
    let (_flow, kept) = h.flow(b, b"beta body").await;
    h.end(b).await;
    h.conn()
        .execute(
            "INSERT INTO audit_anchors (seq, hash, ts) VALUES (1, 'abc', '2026-11-18')",
            [],
        )
        .unwrap();

    let scope = projects(&[alpha]);
    let plan = h.plan(&scope, &LiveState::default()).await;
    let Executed::Done { outcomes, .. } =
        h.execute(&scope, &LiveState::default(), &plan.digest).await
    else {
        panic!("the plan was unchanged");
    };
    assert!(outcomes.iter().all(|outcome| outcome.deleted));
    assert_eq!(h.flows_of(a), 0);
    assert_eq!(h.flows_of(b), 1);
    assert!(!h.blobs.contains(&gone), "the body of alpha is gone");
    assert!(h.blobs.contains(&kept), "the body of beta stays");
    assert_eq!(h.count("SELECT COUNT(*) FROM audit_anchors"), 1);
    assert_eq!(h.count("SELECT COUNT(*) FROM sessions"), 2);
    // Der Registereintrag fiel; seine Sitzung ist jetzt nicht zugeordnet.
    assert!(h.recorder.get_project(alpha).await.unwrap().is_none());
    assert_eq!(
        h.count("SELECT COUNT(*) FROM sessions WHERE project_id IS NULL"),
        1
    );
}

/// Plan, dann eine neue Sitzung im Projekt, dann die Ausführung mit dem alten
/// Digest: veraltet, nichts gelöscht.
///
/// Mutationsbeweis: Ohne den Vergleich des Digests in `execute` löscht der
/// Lauf, und der Test wird rot.
#[tokio::test]
async fn a_new_session_makes_the_plan_stale() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.flow(a, b"alpha body").await;
    h.end(a).await;
    let scope = projects(&[alpha]);
    let plan = h.plan(&scope, &LiveState::default()).await;

    let (later, _) = h.session_in("/home/u/alpha", 11, at(60)).await;
    h.flow(later, b"later body").await;
    h.end(later).await;

    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Stale(_)), "{executed:?}");
    assert_eq!(h.flows_of(a), 1);
    assert_eq!(h.flows_of(later), 1);
    assert!(h.recorder.get_project(alpha).await.unwrap().is_some());
}

/// Plan, dann die Projektwahl durch eine gleich lange andere ersetzt, dann
/// die Ausführung: veraltet, die neue Wahl bleibt.
///
/// Mutationsbeweis: Bindet der Digest nur Art, Anzahl und Größe (die Zeilen
/// der Identität aus `digest_of` entfernt), passt der alte Digest, die neue
/// Wahl fällt, und der Test wird rot.
#[tokio::test]
async fn a_choice_of_the_same_size_makes_the_plan_stale() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.end(a).await;
    let ro = r#"{"sandbox.mode":"ro"}"#;
    let rw = r#"{"sandbox.mode":"rw"}"#;
    assert_eq!(ro.len(), rw.len());
    h.save_choice(alpha, ro).await;
    let scope = projects(&[alpha]);
    let plan = h.plan(&scope, &LiveState::default()).await;
    assert!(kinds(&plan).contains(&(CleanupKind::ProjectChoice, Some(alpha))));

    h.save_choice(alpha, rw).await;
    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Stale(_)), "{executed:?}");
    let row = h.recorder.get_project(alpha).await.unwrap().unwrap();
    assert_eq!(row.choice.as_deref(), Some(rw));
}

/// Zwei Projekte an einem Pfad (Ordner ersetzt): Das Aufräumen des neuen
/// löscht nur dessen Flüsse.
///
/// Mutationsbeweis: Wählte die Auswahl über `work_dir` statt `project_id`,
/// fielen die Flüsse der alten Sitzung mit, und der Test wird rot.
#[tokio::test]
async fn a_replaced_folder_keeps_the_flows_of_the_old_project() {
    let h = Harness::new();
    let (old_session, old) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.flow(old_session, b"old body").await;
    h.end(old_session).await;
    let (new_session, new) = h.session_in("/home/u/alpha", 99, at(50)).await;
    assert_ne!(old, new);
    h.flow(new_session, b"new body").await;
    h.end(new_session).await;

    let scope = projects(&[new]);
    let plan = h.plan(&scope, &LiveState::default()).await;
    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Done { .. }), "{executed:?}");
    assert_eq!(h.flows_of(new_session), 0);
    assert_eq!(h.flows_of(old_session), 1);
    assert!(h.recorder.get_project(old).await.unwrap().is_some());
}

/// Nicht zugeordnete Sitzungen stehen in keinem Projektplan, nur im Plan
/// `unassigned`.
#[tokio::test]
async fn unassigned_sessions_appear_only_in_their_own_plan() {
    let h = Harness::new();
    let loose = h.unassigned_session(at(0)).await;
    h.flow(loose, b"loose body").await;
    h.end(loose).await;
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(10)).await;
    h.flow(a, b"alpha body").await;
    h.end(a).await;

    for scope in [
        projects(&[alpha]),
        CleanupScope {
            target: CleanupTarget::All,
            before: None,
            include_waiting: false,
        },
    ] {
        let plan = h.plan(&scope, &LiveState::default()).await;
        assert!(
            plan.items.iter().all(|item| item.project.is_some()),
            "{plan:?}"
        );
        assert_eq!(
            plan.items
                .iter()
                .filter(|item| item.kind == CleanupKind::RecordedFlows)
                .map(|item| item.count)
                .sum::<u64>(),
            1
        );
    }
    let scope = CleanupScope {
        target: CleanupTarget::Unassigned,
        before: None,
        include_waiting: false,
    };
    let plan = h.plan(&scope, &LiveState::default()).await;
    assert_eq!(kinds(&plan), [(CleanupKind::RecordedFlows, None)]);
    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Done { .. }));
    assert_eq!(h.flows_of(loose), 0);
    assert_eq!(h.flows_of(a), 1);
}

/// Die laufende Sitzung steht in keinem Plan, auch nicht mit „alles"; ihr
/// Projekt behält Wahl und Eintrag.
#[tokio::test]
async fn the_running_session_is_never_planned() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.flow(a, b"old run").await;
    h.end(a).await;
    let (running, same) = h.session_in("/home/u/alpha", 11, at(50)).await;
    assert_eq!(same, alpha);
    h.flow(running, b"live run").await;
    let live = LiveState {
        running_session: Some(running),
        running_project: Some(alpha),
        waiting: None,
    };
    let scope = CleanupScope {
        target: CleanupTarget::All,
        before: None,
        include_waiting: true,
    };
    let plan = h.plan(&scope, &live).await;
    assert_eq!(kinds(&plan), [(CleanupKind::RecordedFlows, Some(alpha))]);
    assert_eq!(plan.items[0].count, 1, "only the flow of the ended session");
    assert!(
        plan.kept
            .iter()
            .any(|kept| kept.kind == CleanupKind::RegisterEntry
                && kept.reason == KeptReason::Running)
    );
    let executed = h.execute(&scope, &live, &plan.digest).await;
    assert!(matches!(executed, Executed::Done { .. }));
    assert_eq!(h.flows_of(running), 1);
    assert_eq!(h.flows_of(a), 0);
}

/// Wartende Änderungen: ohne ausdrückliche Wahl nicht im Plan, und das
/// Projekt behält Wahl und Eintrag; mit ihr ein eigener Eintrag, dessen
/// Kennung der Digest bindet.
#[tokio::test]
async fn waiting_changes_need_the_explicit_choice() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.end(a).await;
    let live = LiveState {
        running_session: None,
        running_project: None,
        waiting: Some(WaitingLayer {
            project: alpha,
            layer: "layer-1".to_owned(),
            count: 3,
            bytes: 120,
            complete: true,
        }),
    };
    let without = h.plan(&projects(&[alpha]), &live).await;
    assert!(without.items.is_empty(), "{without:?}");
    assert!(
        without
            .kept
            .iter()
            .any(|kept| kept.kind == CleanupKind::WaitingChanges
                && kept.reason == KeptReason::ChangesWaiting)
    );

    let scope = CleanupScope {
        include_waiting: true,
        ..projects(&[alpha])
    };
    let with = h.plan(&scope, &live).await;
    assert_eq!(
        kinds(&with),
        [
            (CleanupKind::WaitingChanges, Some(alpha)),
            (CleanupKind::RegisterEntry, Some(alpha)),
        ]
    );
    assert_eq!((with.items[0].count, with.items[0].bytes), (3, 120));

    // Dieselbe Schicht mit einer anderen Zahl von Änderungen oder einer
    // unvollständigen Liste ist ein anderer Plan (Review HUM-533).
    let mut grown = live.clone();
    if let Some(waiting) = grown.waiting.as_mut() {
        waiting.count = 4;
    }
    assert!(matches!(
        h.execute(&scope, &grown, &with.digest).await,
        Executed::Stale(_)
    ));
    let mut cut = live.clone();
    if let Some(waiting) = cut.waiting.as_mut() {
        waiting.complete = false;
    }
    assert!(matches!(
        h.execute(&scope, &cut, &with.digest).await,
        Executed::Stale(_)
    ));

    // Eine andere Schicht ist ein anderer Plan.
    let mut other = live.clone();
    if let Some(waiting) = other.waiting.as_mut() {
        waiting.layer = "layer-2".to_owned();
    }
    let executed = h.execute(&scope, &other, &with.digest).await;
    assert!(matches!(executed, Executed::Stale(_)));
}

/// Ein Blob, auf den auch ein Fluss eines anderen Projekts zeigt, bleibt.
#[tokio::test]
async fn a_blob_shared_with_another_project_stays() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    let (_flow, shared) = h.flow(a, b"same body").await;
    h.end(a).await;
    let (b, _beta) = h.session_in("/home/u/beta", 12, at(10)).await;
    h.flow(b, b"same body").await;
    h.end(b).await;

    let scope = projects(&[alpha]);
    let plan = h.plan(&scope, &LiveState::default()).await;
    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Done { .. }));
    assert_eq!(h.flows_of(a), 0);
    assert!(h.blobs.contains(&shared), "beta still names the body");
}

/// Vergessen: Der Plan nennt genau Projektwahl und Registereintrag des einen
/// Projekts, die Flüsse bleiben. Eine inzwischen geänderte Wahl macht ihn
/// veraltet.
#[tokio::test]
async fn forgetting_plans_choice_and_entry_only() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    h.flow(a, b"alpha body").await;
    h.end(a).await;
    let (b, beta) = h.session_in("/home/u/beta", 12, at(10)).await;
    h.end(b).await;
    h.save_choice(alpha, r#"{"sandbox.mode":"ro"}"#).await;
    h.save_choice(beta, r#"{"sandbox.mode":"ro"}"#).await;

    let scope = CleanupScope {
        target: CleanupTarget::Forget(alpha),
        before: None,
        include_waiting: false,
    };
    let plan = h.plan(&scope, &LiveState::default()).await;
    assert_eq!(
        kinds(&plan),
        [
            (CleanupKind::ProjectChoice, Some(alpha)),
            (CleanupKind::RegisterEntry, Some(alpha)),
        ]
    );

    h.save_choice(alpha, r#"{"sandbox.mode":"rw"}"#).await;
    let stale = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(stale, Executed::Stale(_)));
    assert!(h.recorder.get_project(alpha).await.unwrap().is_some());

    let plan = h.plan(&scope, &LiveState::default()).await;
    let done = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(done, Executed::Done { .. }));
    assert!(h.recorder.get_project(alpha).await.unwrap().is_none());
    assert!(h.recorder.get_project(beta).await.unwrap().is_some());
    assert_eq!(h.flows_of(a), 1, "forgetting keeps the recorded flows");
}

/// Mit Alter: nur Flüsse, die vor der Grenze ankamen, und nur Projekte,
/// deren letzter Start davor liegt, verlieren ihren Eintrag.
#[tokio::test]
async fn an_age_limit_keeps_what_is_younger() {
    let h = Harness::new();
    let ten_days_ago = SystemTime::now() - Duration::from_secs(10 * 24 * 3_600);
    let (old_session, alpha) = h.session_in("/home/u/alpha", 11, ten_days_ago).await;
    h.flow_at(old_session, b"old", ten_days_ago).await;
    h.end(old_session).await;
    let (young, _) = h.session_in("/home/u/alpha", 11, SystemTime::now()).await;
    h.flow_at(young, b"young", SystemTime::now()).await;
    h.end(young).await;

    let scope = CleanupScope {
        target: CleanupTarget::Projects(vec![alpha]),
        before: Some(SystemTime::now() - Duration::from_secs(3_600)),
        include_waiting: false,
    };
    let plan = h.plan(&scope, &LiveState::default()).await;
    assert_eq!(kinds(&plan), [(CleanupKind::RecordedFlows, Some(alpha))]);
    assert_eq!(plan.items[0].count, 1);
}

/// Review HUM-533: Eine Zeile, die nach dem Plan an einem geplanten Fluss
/// dazukommt (ein Befund, eine späte Antwort), macht den Plan veraltet; der
/// Fluss und seine Zeilen bleiben.
///
/// Mutationsbeweis: Bindet der Digest nur die Kennung des Flusses
/// (`flow_line` durch `flow <id>` ersetzt), löscht derselbe Digest den Befund
/// mit, und der Test wird rot.
#[tokio::test]
async fn a_row_added_to_a_planned_flow_makes_the_plan_stale() {
    let h = Harness::new();
    let (a, alpha) = h.session_in("/home/u/alpha", 11, at(0)).await;
    let (flow, _sha) = h.flow(a, b"alpha body").await;
    h.end(a).await;
    let scope = projects(&[alpha]);
    let plan = h.plan(&scope, &LiveState::default()).await;

    h.conn()
        .execute(
            "INSERT INTO diagnostics (flow_id, idx, code, severity, title, why) \
             VALUES (?1, 0, 'LIMIT_001', 'warning', 'late', 'a late finding')",
            [&flow],
        )
        .unwrap();
    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Stale(_)), "{executed:?}");
    assert_eq!(h.flows_of(a), 1);
    assert_eq!(h.count("SELECT COUNT(*) FROM diagnostics"), 1);

    // Ebenso eine Antwort, die eine Nachricht ersetzt.
    let plan = h.plan(&scope, &LiveState::default()).await;
    h.conn()
        .execute(
            "UPDATE messages SET size = size + 1 WHERE flow_id = ?1 AND dir = 'response'",
            [&flow],
        )
        .unwrap();
    let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
    assert!(matches!(executed, Executed::Stale(_)), "{executed:?}");

    // Codex-Delta: ein Body und Kopfzeilen gleicher Länge mit anderem Inhalt.
    for change in [
        "UPDATE messages SET body_inline = X'0201' WHERE flow_id = ?1 AND dir = 'request'",
        "UPDATE messages SET headers_json = '{}' WHERE flow_id = ?1 AND dir = 'request'",
    ] {
        let plan = h.plan(&scope, &LiveState::default()).await;
        h.conn().execute(change, [&flow]).unwrap();
        let executed = h.execute(&scope, &LiveState::default(), &plan.digest).await;
        assert!(
            matches!(executed, Executed::Stale(_)),
            "{change}: {executed:?}"
        );
    }
    assert_eq!(h.flows_of(a), 1);
}
