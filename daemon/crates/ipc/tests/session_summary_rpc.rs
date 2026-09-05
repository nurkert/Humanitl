//! `GetSessionSummary`: die abgelegte Zusammenfassung eines Sandbox-Laufs
//! (HUM-043).
//!
//! Der Weg, den `humanitl sessions summary <id>` nimmt, von der Aufzeichnung
//! bis zur Wire-Form. Er ist der zweite von zwei Wegen zu derselben Auskunft:
//! Der erste ist das Ereignis am Ende eines Laufs
//! (`tests/sandbox_start.rs::a_finished_run_says_what_the_agent_left_in_the_project`),
//! der zweite ist diese RPC, und sie muss dasselbe sagen, wenn der Lauf längst
//! beendet ist.
//!
//! Geprüft wird hier vor allem, was zwischen dem Schreiben und dem Lesen
//! liegt: die Serialisierung nach `JSON`, die Zeile in `session_summaries`,
//! das Zurücklesen und die Befunde, die der Dienst aus der gelesenen
//! Zusammenfassung **neu** rechnet, statt sie mitzuspeichern.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use humanitl_config::Config;
use humanitl_core::SessionId;
use humanitl_core::ids::SandboxId;
use humanitl_ipc::v1::humanitl_server::Humanitl as _;
use humanitl_ipc::{IpcServer, v1};
use humanitl_proxy::{FlowRegistry, HoldQueue};
use humanitl_recorder::{Recorder, RecorderSettings, SessionMeta};
use humanitl_sandbox::summary::SessionSummary;
use humanitl_sandbox::worktree::{SnapshotLimits, snapshot};
use tonic::{Code, Request};

/// Ein Dienst mit Aufzeichnung und eine Zusammenfassung darin.
struct Fixture {
    _dir: tempfile::TempDir,
    server: IpcServer,
    session: SessionId,
    sandbox: SandboxId,
    /// Das Projektverzeichnis, über dem die Zusammenfassung entstand.
    work: PathBuf,
}

impl Fixture {
    /// Legt einen Baum an, verändert ihn wie ein Agent und schreibt die
    /// Zusammenfassung in die Aufzeichnung.
    ///
    /// Die Zusammenfassung entsteht aus zwei echten Schnappschüssen und nicht
    /// aus einem Literal: Ein Literal prüfte nur, dass `serde` funktioniert.
    async fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let recorder = Recorder::open(
            &dir.path().join("humanitl.db"),
            &dir.path().join("blobs"),
            RecorderSettings::default(),
        )
        .expect("the recording opens");

        let session = SessionId::new();
        let sandbox = SandboxId::new();
        let work = dir.path().join("project");
        std::fs::create_dir_all(&work).expect("project");
        std::fs::write(work.join("keep.txt"), b"unchanged\n").expect("keep");
        let limits = SnapshotLimits::default();
        let before = snapshot(&work, &limits).expect("the first snapshot");

        // Was ein Agent hinterlässt: eine neue Datei, ein Symlink nach
        // draußen, dessen Ziel eine Terminalfolge trägt, und ein Hook unter
        // einem Pfad, über dem keine Maske lag.
        std::fs::write(work.join("new.txt"), b"hello\n").expect("new");
        std::os::unix::fs::symlink("/etc\u{1b}]8;;http://evil\u{7}", work.join("out"))
            .expect("symlink");
        std::fs::create_dir_all(work.join(".git/hooks")).expect("hooks");
        std::fs::write(work.join(".git/hooks/pre-commit"), b"#!/bin/sh\n").expect("hook");
        let after = snapshot(&work, &limits).expect("the second snapshot");

        let mut summary = SessionSummary::new(session, sandbox, &work);
        summary.set_unprotected(&[PathBuf::from(".git/hooks")]);
        let _candidates = summary.add_changes(&work, &before, &after);

        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: work.display().to_string(),
            agent: "opencode".to_owned(),
        });
        recorder.store_session_summary(
            session,
            sandbox,
            &serde_json::to_string(&summary).expect("the summary serialises"),
        );
        recorder.flush().await;

        let queue = Arc::new(HoldQueue::with_registry(
            &Config::default().limits,
            Arc::new(FlowRegistry::new(&Config::default().limits)),
        ));
        let server = IpcServer::new(queue, &Config::default(), Some(session))
            .with_recorder(recorder.clone());
        Self {
            _dir: dir,
            server,
            session,
            sandbox,
            work,
        }
    }

    /// Fragt die RPC nach einer Kennung.
    async fn get(&self, id: &str) -> Result<v1::SessionSummary, tonic::Status> {
        self.server
            .get_session_summary(Request::new(v1::SessionSummaryRef {
                sandbox_id: id.to_owned(),
            }))
            .await
            .map(tonic::Response::into_inner)
    }
}

/// Die abgelegte Zusammenfassung kommt vollständig zurück, mit Befunden.
#[tokio::test]
async fn a_stored_summary_comes_back_whole() {
    let fixture = Fixture::new().await;
    let summary = fixture
        .get(&fixture.sandbox.to_string())
        .await
        .expect("the summary is there");

    assert_eq!(summary.sandbox_id, fixture.sandbox.to_string());
    assert_eq!(summary.session_id, fixture.session.to_string());
    assert_eq!(summary.work_dir, fixture.work.display().to_string());
    assert!(summary.created.is_some(), "the row carries its time");

    let paths: Vec<&str> = summary
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    assert!(paths.contains(&"new.txt"), "{paths:?}");
    assert!(paths.contains(&".git/hooks/pre-commit"), "{paths:?}");
    assert!(
        !paths.contains(&"keep.txt"),
        "an untouched file is no change: {paths:?}"
    );

    let hook = summary
        .changes
        .iter()
        .find(|change| change.path == ".git/hooks/pre-commit")
        .expect("the hook is listed");
    assert_eq!(hook.unprotected_by, ".git/hooks");
    assert!(!hook.git_metadata, "a hook is not tucked away as metadata");

    // Die Befunde rechnet der Dienst aus der gelesenen Zusammenfassung; sie
    // stehen nicht als zweite Wahrheit in der Datenbank.
    let codes: Vec<&str> = summary
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"SANDBOX_022"), "{codes:?}");
    assert!(codes.contains(&"SANDBOX_025"), "{codes:?}");
}

/// Ein Symlink-Ziel mit `ESC ]` erscheint gesäubert.
///
/// Der Agent wählt das Ziel; ohne Säuberung stünde eine `OSC 8`-Folge in der
/// Tabelle der Kommandozeile und machte aus der Zeile einen anklickbaren
/// Verweis (BACKLOG.md 4.2). Gesäubert wird im Daemon, bevor die
/// Zusammenfassung abgelegt wird — der gespeicherte Text ist schon der
/// gezeigte.
#[tokio::test]
async fn an_escape_sequence_in_a_symlink_target_never_reaches_the_wire() {
    let fixture = Fixture::new().await;
    let summary = fixture
        .get(&fixture.sandbox.to_string())
        .await
        .expect("the summary is there");

    let link = summary.symlinks.first().expect("one symlink");
    assert!(link.escapes, "an absolute target leaves the project");
    assert!(link.target.starts_with("/etc"), "{:?}", link.target);
    assert!(!link.target.contains('\u{1b}'), "{:?}", link.target);
    assert!(!link.target.contains('\u{7}'), "{:?}", link.target);
    assert!(link.mangled, "and the row says the name was changed");
    assert_eq!(
        link.path_hash.len(),
        16,
        "the hash keeps two look-alike names apart"
    );
}

/// Zu einem Lauf, den es hier nicht gibt, gibt es nichts — und das ist
/// `NOT_FOUND`, nicht eine leere Zusammenfassung.
#[tokio::test]
async fn an_unknown_run_is_not_found_and_not_an_empty_summary() {
    let fixture = Fixture::new().await;
    let status = fixture
        .get(&SandboxId::new().to_string())
        .await
        .expect_err("an unknown run has no summary");
    assert_eq!(status.code(), Code::NotFound);
    let diagnostic =
        humanitl_ipc::diagnostic_from_status(&status).expect("the finding travels in the details");
    assert_eq!(diagnostic.code, "SANDBOX_027");
}

/// Eine Kennung, die keine ist, ist eine unlesbare Anfrage und kein fehlender
/// Lauf.
#[tokio::test]
async fn an_unreadable_id_is_an_invalid_argument() {
    let fixture = Fixture::new().await;
    for text in ["", "not-an-id", "0123"] {
        let Err(status) = fixture.get(text).await else {
            panic!("{text:?} is not a sandbox id and must be refused");
        };
        assert_eq!(status.code(), Code::InvalidArgument, "{text:?}");
        let diagnostic = humanitl_ipc::diagnostic_from_status(&status)
            .unwrap_or_else(|| panic!("{text:?}: the finding travels in the details"));
        assert_eq!(diagnostic.code, "IPC_005", "{text:?}");
    }
}

/// Ein Daemon ohne Aufzeichnung sagt, dass er keine hat.
#[tokio::test]
async fn a_daemon_without_a_recording_says_so() {
    let queue = Arc::new(HoldQueue::with_registry(
        &Config::default().limits,
        Arc::new(FlowRegistry::new(&Config::default().limits)),
    ));
    let server = IpcServer::new(queue, &Config::default(), Some(SessionId::new()));
    let status = server
        .get_session_summary(Request::new(v1::SessionSummaryRef {
            sandbox_id: SandboxId::new().to_string(),
        }))
        .await
        .expect_err("without a recording nothing was kept");
    let diagnostic =
        humanitl_ipc::diagnostic_from_status(&status).expect("the finding travels in the details");
    assert_eq!(diagnostic.code, "RECORDER_001");
}
