//! Das Projektregister (HUM-532): Migration, Wiedererkennen, Zuordnung der
//! Sitzungen über die Projekt-`id`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use humanitl_core::{ProjectId, SessionId};
use humanitl_recorder::{ProjectStart, Recorder, RecorderSettings, SessionMeta};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

struct Data {
    _dir: tempfile::TempDir,
    db: PathBuf,
    blobs: PathBuf,
}

fn data() -> Data {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("humanitl.db");
    let blobs = dir.path().join("blobs");
    Data {
        _dir: dir,
        db,
        blobs,
    }
}

fn open(data: &Data) -> Recorder {
    Recorder::open(&data.db, &data.blobs, RecorderSettings::default()).unwrap()
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds)
}

/// Eine Sitzung des Daemons, eingetragen wie beim Start von `humanitld`.
fn session(recorder: &Recorder, started: SystemTime) -> SessionId {
    let id = SessionId::new();
    recorder.start_session(&SessionMeta {
        id,
        started_at: started,
        sandbox_profile: "default".to_owned(),
        llm_endpoint: None,
        work_dir: "/daemon/cwd".to_owned(),
        agent: "opencode".to_owned(),
    });
    id
}

fn start(
    session: SessionId,
    path: &str,
    inode: u64,
    agent: &str,
    when: SystemTime,
) -> ProjectStart {
    ProjectStart {
        session,
        path: path.to_owned(),
        dev: 7,
        ino: inode,
        agent: agent.to_owned(),
        at: when,
    }
}

fn rows(db: &Path, sql: &str) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

/// Ein Start legt an, ein zweiter zählt hoch, `list` sortiert nach dem
/// letzten Start.
#[tokio::test]
async fn a_start_registers_and_a_second_counts_up() {
    let data = data();
    let recorder = open(&data);
    let first = session(&recorder, at(0));

    let alpha = recorder
        .register_project_start(start(first, "/home/u/alpha", 11, "claude", at(10)))
        .await
        .unwrap();
    assert!(alpha.created);
    assert_eq!(alpha.project.runs, 1);
    assert_eq!(alpha.project.display_name, "alpha");
    assert_eq!(alpha.project.last_agent.as_deref(), Some("claude"));

    let beta = recorder
        .register_project_start(start(first, "/home/u/beta", 12, "codex", at(20)))
        .await
        .unwrap();
    let again = recorder
        .register_project_start(start(first, "/home/u/alpha", 11, "opencode", at(30)))
        .await
        .unwrap();
    assert!(!again.created);
    assert_eq!(again.project.id, alpha.project.id);
    assert_eq!(again.project.runs, 2);
    assert_eq!(again.project.last_agent.as_deref(), Some("opencode"));
    assert!(again.project.version > alpha.project.version);

    let listed = recorder.list_projects(10, 0).await.unwrap();
    let order: Vec<ProjectId> = listed.iter().map(|row| row.id).collect();
    assert_eq!(order, [alpha.project.id, beta.project.id]);
    assert_eq!(
        recorder.list_projects(1, 1).await.unwrap()[0].id,
        beta.project.id
    );
}

/// Ein neuer Ordner am alten Pfad: Die alte Zeile bleibt mit `replaced_at`
/// und ihrer Wahl, die neue ist aktuell, mit neuer `id` und ohne Wahl.
///
/// Mutationsbeweis: Ohne den Vergleich von `dev`/`ino` in
/// `register_inside` zählte der zweite Start die alte Zeile hoch, und der
/// Test wird rot.
#[tokio::test]
async fn a_replaced_folder_is_a_new_project_and_keeps_the_old_one() {
    let data = data();
    let recorder = open(&data);
    let daemon = session(&recorder, at(0));
    let old = recorder
        .register_project_start(start(daemon, "/home/u/repo", 100, "claude", at(10)))
        .await
        .unwrap()
        .project;
    recorder
        .save_project_choice(old.id, Some(r#"{"hold.ask_mode":"app"}"#.to_owned()))
        .await
        .unwrap()
        .expect("the current row takes a choice");

    let new = recorder
        .register_project_start(start(daemon, "/home/u/repo", 200, "claude", at(20)))
        .await
        .unwrap();
    assert!(new.created);
    assert_eq!(new.replaced, Some(old.id));
    assert_ne!(new.project.id, old.id);
    assert_eq!(new.project.choice, None);
    assert_eq!(new.project.runs, 1);

    let history = recorder.get_project(old.id).await.unwrap().unwrap();
    assert!(history.replaced_at.is_some());
    assert_eq!(
        history.choice.as_deref(),
        Some(r#"{"hold.ask_mode":"app"}"#)
    );
    assert_eq!(
        recorder
            .current_project_at("/home/u/repo".to_owned(), None)
            .await
            .unwrap()
            .map(|row| row.id),
        Some(new.project.id)
    );
    // Eine ersetzte Zeile ist Verlauf und nimmt keine Wahl mehr.
    assert_eq!(
        recorder.save_project_choice(old.id, None).await.unwrap(),
        None
    );
}

/// Zwei aktuelle Zeilen für denselben Pfad gibt es nicht, und ein zweiter
/// Ersatz ergibt zwei Verlaufszeilen und eine aktuelle.
///
/// Mutationsbeweis: `UNIQUE (path)` statt des Teilindex lässt schon den
/// ersten Ersatz scheitern, und der Test wird rot.
#[tokio::test]
async fn one_current_row_per_path_and_history_for_each_replacement() {
    let data = data();
    let recorder = open(&data);
    let daemon = session(&recorder, at(0));
    for (inode, second) in [(1, 10), (2, 20), (3, 30)] {
        recorder
            .register_project_start(start(daemon, "/home/u/twice", inode, "claude", at(second)))
            .await
            .unwrap();
    }
    assert_eq!(
        rows(
            &data.db,
            "SELECT COUNT(*) FROM projects WHERE path = '/home/u/twice'"
        ),
        3
    );
    assert_eq!(
        rows(
            &data.db,
            "SELECT COUNT(*) FROM projects WHERE path = '/home/u/twice' AND replaced_at IS NULL"
        ),
        1
    );

    let conn = rusqlite::Connection::open(&data.db).unwrap();
    let second_current = conn.execute(
        "INSERT INTO projects (id, path, display_name, first_seen) \
         VALUES ('x', '/home/u/twice', 'twice', 0)",
        [],
    );
    assert!(
        second_current.is_err(),
        "the partial index allows one current row"
    );
}

/// Zwei Projekte an einem Pfad: Die Sitzungen vor dem Ersatz gehören zur
/// alten `id`, die danach zur neuen, und die Läufe zählen getrennt.
///
/// Mutationsbeweis: Wählte `sessions_of_project` über `work_dir` statt über
/// `project_id`, gehörten alle drei Sitzungen beiden Projekten, und der Test
/// wird rot.
#[tokio::test]
async fn sessions_belong_to_the_project_id_not_to_the_path() {
    let data = data();
    let recorder = open(&data);
    let one = session(&recorder, at(0));
    let old = recorder
        .register_project_start(start(one, "/home/u/same", 1, "claude", at(1)))
        .await
        .unwrap()
        .project;
    let two = session(&recorder, at(2));
    recorder
        .register_project_start(start(two, "/home/u/same", 1, "claude", at(3)))
        .await
        .unwrap();
    let three = session(&recorder, at(4));
    let new = recorder
        .register_project_start(start(three, "/home/u/same", 9, "claude", at(5)))
        .await
        .unwrap()
        .project;

    assert_eq!(
        recorder.sessions_of_project(old.id).await.unwrap(),
        [two, one]
    );
    assert_eq!(recorder.sessions_of_project(new.id).await.unwrap(), [three]);
    assert_eq!(recorder.get_project(old.id).await.unwrap().unwrap().runs, 2);
    assert_eq!(new.runs, 1);
    let listed = recorder.list_projects(10, 0).await.unwrap();
    assert_eq!(listed.len(), 1, "only the current row is listed");
    assert_eq!(listed[0].id, new.id);
    // Die Sitzung trägt jetzt den Ordner und den Agenten ihres Starts, nicht
    // das Arbeitsverzeichnis des Daemons.
    assert_eq!(
        rows(
            &data.db,
            &format!(
                "SELECT COUNT(*) FROM sessions WHERE id = '{three}' AND work_dir = '/home/u/same'"
            )
        ),
        1
    );
}

/// Die Migration füllt `projects` aus den Sitzungen von gestern (Pfad,
/// letzter Start, Agent, Läufe) und gibt jeder Sitzung die `project_id`
/// ihres Projekts; eine Sitzung ohne eindeutige Zuordnung bleibt leer.
#[tokio::test]
async fn the_migration_fills_the_register_from_yesterdays_sessions() {
    let data = data();
    let yesterday = SystemTime::now() - DAY;
    let millis = |offset: u64| {
        i64::try_from(
            (yesterday + Duration::from_secs(offset))
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    };
    let sessions = [
        (SessionId::new(), millis(0), "/home/u/alpha", "opencode"),
        (SessionId::new(), millis(60), "/home/u/alpha", "claude"),
        (SessionId::new(), millis(120), "/home/u/beta", "codex"),
        // Kein absoluter Pfad: kein Projekt, keine Zuordnung.
        (SessionId::new(), millis(180), ".", "claude"),
        // Der Daemon lief hier, aber keine Sandbox: kein Beleg, kein Projekt.
        (SessionId::new(), millis(240), "/home/u", "opencode"),
    ];
    // Belegt ist ein Lauf durch eine Zusammenfassung (die beiden in alpha
    // und die ohne absoluten Pfad) oder durch Verkehr des Agenten (beta).
    seed_v12(&data.db, &sessions, &[0, 1, 3], 2);

    let recorder = open(&data);
    let listed = recorder.list_projects(10, 0).await.unwrap();
    assert_eq!(
        listed.len(),
        2,
        "only folders where a sandbox ran: {listed:?}"
    );
    let alpha = listed
        .iter()
        .find(|row| row.path == "/home/u/alpha")
        .unwrap();
    assert_eq!(alpha.display_name, "alpha");
    assert_eq!(alpha.runs, 2);
    assert_eq!(alpha.last_agent.as_deref(), Some("claude"));
    assert_eq!(
        alpha.last_started.map(humanitl_recorder::millis),
        Some(millis(60))
    );
    assert_eq!(
        alpha.dev, None,
        "the folder is identified at the next start"
    );
    let beta = listed
        .iter()
        .find(|row| row.path == "/home/u/beta")
        .unwrap();
    assert_eq!(listed[0].id, beta.id, "the latest start first");

    assert_eq!(
        recorder.sessions_of_project(alpha.id).await.unwrap(),
        [sessions[1].0, sessions[0].0]
    );
    assert_eq!(
        recorder.sessions_of_project(beta.id).await.unwrap(),
        [sessions[2].0]
    );
    assert_eq!(
        rows(
            &data.db,
            &format!(
                "SELECT COUNT(*) FROM sessions WHERE id = '{}' AND project_id IS NULL",
                sessions[3].0
            )
        ),
        1
    );

    // Der nächste Start übernimmt das Paar des Ordners, statt ein neues
    // Projekt anzulegen.
    let daemon = session(&recorder, SystemTime::now());
    let next = recorder
        .register_project_start(start(
            daemon,
            "/home/u/alpha",
            5,
            "claude",
            SystemTime::now(),
        ))
        .await
        .unwrap();
    assert!(!next.created);
    assert_eq!(next.project.id, alpha.id);
    assert_eq!((next.project.dev, next.project.ino), (Some(7), Some(5)));
}

/// Die Migration ordnet nur eindeutig zu: Gab es am Pfad schon einen Ersatz
/// nach dem Start der Sitzung, bleibt sie ohne Projekt.
#[test]
fn the_migration_leaves_an_ambiguous_session_unassigned() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    for migration in humanitl_recorder::MIGRATIONS.iter().take(12) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute_batch(
        "INSERT INTO sessions (id, started_at, sandbox_profile, work_dir, agent) \
         VALUES ('s1', 5, 'default', '/p', 'claude');\n\
         INSERT INTO session_summaries (sandbox_id, session_id, created, json) \
         VALUES ('r1', 's1', 6, x'7b7d');",
    )
    .unwrap();
    // Ein Register, das schon zwei Zeilen am Pfad kennt (eine ersetzt nach
    // dem Start der Sitzung): Die Zuordnung ist nicht eindeutig.
    let v13 = humanitl_recorder::MIGRATIONS[12].sql;
    let (create, _fill) = v13
        .split_once("-- Nachfüllen")
        .expect("the migration has its fill part");
    conn.execute_batch(create).unwrap();
    conn.execute_batch(
        "INSERT INTO projects (id, path, display_name, first_seen, replaced_at) \
         VALUES ('old', '/p', 'p', 1, 10);\n\
         INSERT INTO projects (id, path, display_name, first_seen) VALUES ('new', '/p', 'p', 10);",
    )
    .unwrap();
    let assign = v13
        .split_once("-- Eine Sitzung gehört")
        .expect("the migration has its assignment")
        .1;
    conn.execute_batch(&format!("-- {assign}")).unwrap();
    let assigned: Option<String> = conn
        .query_row(
            "SELECT project_id FROM sessions WHERE id = 's1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(assigned, None);
}

/// Eine Datenbank im Stand von V12 mit diesen Sitzungen; die unter
/// `summarized` bekommen eine Zusammenfassung, die unter `flowing` einen Flow.
fn seed_v12(
    db: &Path,
    sessions: &[(SessionId, i64, &str, &str)],
    summarized: &[usize],
    flowing: usize,
) {
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    let conn = rusqlite::Connection::open(db).unwrap();
    for migration in humanitl_recorder::MIGRATIONS.iter().take(12) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute_batch("PRAGMA user_version = 12;").unwrap();
    for (id, started, dir, agent) in sessions {
        conn.execute(
            "INSERT INTO sessions (id, started_at, ended_at, sandbox_profile, work_dir, \
             agent) VALUES (?1, ?2, ?2, 'default', ?3, ?4)",
            rusqlite::params![id.to_string(), started, dir, agent],
        )
        .unwrap();
    }
    for index in summarized {
        conn.execute(
            "INSERT INTO session_summaries (sandbox_id, session_id, created, json) \
             VALUES (?1, ?2, 0, x'7b7d')",
            rusqlite::params![SessionId::new().to_string(), sessions[*index].0.to_string()],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, \
         host_rev, port, path, state, request_size) VALUES ('f1', ?1, 1, 1, 'GET', 'https', \
         'a.example', 'a.example', 'example.a.', 443, '/', 'recorded', 0)",
        rusqlite::params![sessions[flowing].0.to_string()],
    )
    .unwrap();
}
