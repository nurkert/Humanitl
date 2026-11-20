//! Der Start und das Projektregister (HUM-532, ADR-0024 Punkt 1).
//!
//! Drei Schritte gehören zu jedem Start, und alle drei stehen hier:
//!
//! 1. **Kein Start ohne Projekt.** Nennt weder die Anfrage einen Ordner noch
//!    `sandbox.work_dir`, lehnt der Start mit `SANDBOX_053` ab, bevor eine
//!    Sandbox entsteht und bevor das Register etwas einträgt. Das
//!    Arbeitsverzeichnis des Daemons ist nie die Vorgabe
//!    ([`Inner::work_dir`]).
//! 2. **Die Projektwahl des Ordners**, wenn das Register ihn kennt: aus der
//!    aktuellen Zeile an seinem Pfad, aber nur, wenn Gerät und Inode noch
//!    die des ersten Starts sind. Ein anderer Ordner am alten Ort ist ein
//!    neues Projekt und startet ohne die Wahl des alten. Eine Wahl, die
//!    nicht lädt, ist `CONFIG_024` und hält den Start auf.
//! 3. **Der Eintrag**, sobald die Sandbox läuft: anlegen, ersetzen oder
//!    hochzählen, und die Sitzung bekommt im selben Schritt die
//!    `project_id`.

use std::time::SystemTime;

use humanitl_config::{PolicyLocks, ProjectChoice};
use humanitl_core::diagnostics::codes::{RECORDER_003, SANDBOX_042, SANDBOX_053};
use humanitl_core::{Diagnostic, FixAction, ProjectId, Severity};
use humanitl_recorder::{ProjectStart, RecorderError};
use tokio::sync::mpsc;

use super::{Inner, SandboxService, diagnostic_event, lock, status_event};
use crate::projects::{ProjectActivity, ProjectIdentity};
use crate::v1;
use std::sync::Arc;

/// Was ein Start über sein Projekt weiß, bevor er auflöst.
#[derive(Debug, Clone, Default)]
pub(super) struct StartProject {
    /// Der Ordner, wie das Register ihn erkennt; `None`, wenn er sich nicht
    /// lesen lässt (dann scheitert der Start ohnehin am Ordner).
    pub(super) identity: Option<ProjectIdentity>,
    /// Die Projektwahl aus dem Register, falls der Ordner bekannt ist.
    pub(super) choice: Option<ProjectChoice>,
}

impl Inner {
    /// Der Ordner dieses Starts und seine Projektwahl.
    ///
    /// # Errors
    ///
    /// `SANDBOX_053` ohne Ordner; `CONFIG_024` für eine gespeicherte Wahl,
    /// die nicht mehr lädt; der Befund der Aufzeichnung, wenn das Register
    /// sich nicht lesen lässt. Ohne die Wahl zu starten, die ein Mensch
    /// getroffen hat, wäre ein stiller Verzicht auf sie; der Start hält
    /// deshalb an.
    pub(super) async fn start_project(
        &self,
        plan: &v1::sandbox_request::Plan,
    ) -> Result<StartProject, Diagnostic> {
        let config = self.config();
        let dir = self.work_dir(&config, &self.pending_choice(), plan);
        if dir.as_os_str().is_empty() {
            return Err(no_project_folder());
        }
        self.project_in(&dir).await
    }

    /// [`Inner::start_project`] for this folder: its identity and its saved
    /// choice, checked against the resolution of this folder.
    ///
    /// # Errors
    ///
    /// As [`Inner::start_project`], without `SANDBOX_053`.
    pub(super) async fn project_in(
        &self,
        dir: &std::path::Path,
    ) -> Result<StartProject, Diagnostic> {
        let identity = ProjectIdentity::of(dir).ok();
        let choice = match (self.recorder.as_ref(), identity.as_ref()) {
            (Some(recorder), Some(identity)) => {
                let row = recorder
                    .current_project_at(identity.path_text(), Some((identity.dev, identity.ino)))
                    .await
                    .map_err(RecorderError::into_diagnostic)?;
                // Ein anderer Ordner am alten Pfad: ein neues Projekt, ohne
                // die Wahl des alten (HUM-532, Wiedererkennen).
                let same_folder = row.as_ref().is_some_and(|row| {
                    row.dev
                        .zip(row.ino)
                        .is_none_or(|known| known == (identity.dev, identity.ino))
                });
                match row.and_then(|row| row.choice).filter(|_| same_folder) {
                    Some(text) => {
                        let choice = ProjectChoice::from_json(&text)?;
                        // Geprüft gegen die Auflösung dieses Ordners ohne
                        // Wahl, nicht gegen die Konfiguration der letzten
                        // Sitzung: Die trüge die Wahl eines anderen Projekts.
                        let paths = self.paths.clone();
                        let path = identity.path_text();
                        let checked = choice.clone();
                        tokio::task::spawn_blocking(move || {
                            crate::projects::check_for(
                                &paths,
                                &PolicyLocks::of_policy(&paths),
                                &path,
                                &checked,
                            )
                            .map(|_resolved| ())
                        })
                        .await
                        .map_err(|error| super::joined_failed(&error))??;
                        Some(choice)
                    }
                    None => None,
                }
            }
            _ => None,
        };
        Ok(StartProject { identity, choice })
    }

    /// [`Inner::start_project`], und bei einem Befund: Befund und
    /// `Status(failed)` in den Strom, `None` zurück.
    pub(super) async fn project_or_refuse(
        &self,
        plan: &v1::sandbox_request::Plan,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) -> Option<StartProject> {
        match self.start_project(plan).await {
            Ok(project) => Some(project),
            Err(diagnostic) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                let _ = tx
                    .send(status_event(self.failed_status(plan, &diagnostic)))
                    .await;
                None
            }
        }
    }

    /// Der Schritt des Projekts, sobald die Sandbox den Ordner eingehängt
    /// hat: Der Ordner am Pfad ist noch der, für den die Wahl geladen wurde,
    /// und der Start steht im Register. Gilt eines davon nicht, endet die
    /// Sandbox mit dem Befund, und `false` kommt zurück (fail closed): Ein
    /// Lauf, der nicht im Register steht, wäre für Zuletzt, Aufräumen und
    /// die nächste Projektwahl unsichtbar, und ein Ordner, der zwischen dem
    /// Laden der Wahl und dem Einhängen getauscht wurde, liefe mit der Wahl
    /// eines anderen.
    pub(super) async fn settle_project(
        self: &Arc<Self>,
        plan: &v1::sandbox_request::Plan,
        project: &StartProject,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) -> bool {
        let Some(recorder) = self.recorder.as_ref() else {
            return true;
        };
        let refused = match self.record_project_start(recorder, project).await {
            Ok(id) => {
                *lock(&self.project) = Some(id);
                return true;
            }
            Err(refused) => refused,
        };
        self.stop_after_failed_check(plan, &refused, tx).await;
        false
    }

    /// Prüft den Ordner noch einmal und trägt den Start ein.
    async fn record_project_start(
        &self,
        recorder: &humanitl_recorder::Recorder,
        project: &StartProject,
    ) -> Result<ProjectId, Diagnostic> {
        let Some(identity) = project.identity.as_ref() else {
            return Err(unregistered(
                "the project folder could not be read before the start",
            ));
        };
        let now = ProjectIdentity::of(&identity.path).ok();
        if now.as_ref() != Some(identity) {
            return Err(folder_swapped(identity, now.as_ref()));
        }
        let start = ProjectStart {
            session: self.session,
            path: identity.path_text(),
            dev: identity.dev,
            ino: identity.ino,
            agent: self.config().agent.adapter.clone(),
            at: SystemTime::now(),
        };
        recorder
            .register_project_start(start)
            .await
            .map(|registered| registered.project.id)
            .map_err(|err| unregistered(&err.into_diagnostic().why))
    }

    /// Das Projekt des letzten Starts samt Lauf und wartenden Änderungen.
    fn project_activity(&self) -> Option<ProjectActivity> {
        let project: ProjectId = (*lock(&self.project))?;
        Some(ProjectActivity {
            project,
            running: self.is_running(),
            changes_waiting: lock(&self.disposable).is_some(),
            session: self.session,
        })
    }
}

impl SandboxService {
    /// Das Projekt des letzten Starts dieses Dienstes: ob seine Sandbox
    /// läuft und ob Änderungen warten (HUM-532, `Projects.list`). `None`,
    /// solange nichts gestartet ist.
    #[must_use]
    pub fn project_activity(&self) -> Option<ProjectActivity> {
        self.inner.project_activity()
    }
}

/// `SANDBOX_042`: Am Pfad liegt nach dem Einhängen ein anderer Ordner als
/// der, für den die Projektwahl geladen wurde.
fn folder_swapped(was: &ProjectIdentity, now: Option<&ProjectIdentity>) -> Diagnostic {
    Diagnostic::builder(SANDBOX_042, Severity::Blocking)
        .why(format!(
            "the project folder {} changed while the session started: its device and inode were \
             {}:{} when its project choice was loaded and are {} now; the session was stopped",
            was.path.display(),
            was.dev,
            was.ino,
            now.map_or_else(
                || "unreadable".to_owned(),
                |now| format!("{}:{}", now.dev, now.ino)
            )
        ))
        .build()
}

/// Der Start ließ sich nicht ins Projektregister eintragen; die Sitzung
/// endet (`RECORDER_003`).
fn unregistered(why: &str) -> Diagnostic {
    Diagnostic::builder(RECORDER_003, Severity::Blocking)
        .why(format!(
            "the session was stopped because the project register could not record its start: \
             {why}"
        ))
        .build()
}

/// `SANDBOX_053`: der Start nennt keinen Projektordner.
fn no_project_folder() -> Diagnostic {
    Diagnostic::builder(SANDBOX_053, Severity::Blocking)
        .why(
            "the start names no project folder: the request has no work_dir and sandbox.work_dir \
             is not set; the daemon does not take its own working directory or your home \
             directory instead. Choose a folder in the start window of the app, run humanitl go \
             or humanitl run inside the project, or pass --work DIR"
                .to_owned(),
        )
        .fix(FixAction::CopyCommand(
            "humanitl run --work <dir>".to_owned(),
        ))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::SystemTime;

    use humanitl_config::{AskMode, Config, Env, Paths};
    use humanitl_core::SessionId;
    use humanitl_recorder::{ProjectStart, Recorder, RecorderSettings, SessionMeta};
    use tokio_stream::StreamExt as _;

    use crate::projects::ProjectIdentity;
    use crate::sandbox::{SandboxPorts, SandboxService};
    use crate::session::SessionResolver;
    use crate::v1;

    struct Fixture {
        home: tempfile::TempDir,
        recorder: Recorder,
        session: SessionId,
    }

    impl Fixture {
        fn new() -> Self {
            let home = tempfile::tempdir().unwrap();
            let recorder = Recorder::open(
                &home.path().join("data/humanitl.db"),
                &home.path().join("data/blobs"),
                RecorderSettings::default(),
            )
            .unwrap();
            let session = SessionId::new();
            recorder.start_session(&SessionMeta {
                id: session,
                started_at: SystemTime::now(),
                sandbox_profile: "default".to_owned(),
                llm_endpoint: None,
                work_dir: "/daemon/cwd".to_owned(),
                agent: "opencode".to_owned(),
            });
            Self {
                home,
                recorder,
                session,
            }
        }

        fn paths(&self) -> Paths {
            let home = self.home.path();
            Paths::new(Env::from_pairs([
                ("HOME", home.display().to_string()),
                ("XDG_CONFIG_HOME", home.join("config").display().to_string()),
                ("XDG_DATA_HOME", home.join("data").display().to_string()),
                ("XDG_RUNTIME_DIR", home.join("run").display().to_string()),
            ]))
        }

        /// Ein Dienst ohne `sandbox.work_dir`, mit dieser Aufzeichnung.
        fn service(&self) -> SandboxService {
            let mut config = Config::default();
            config.sandbox.work_dir = None;
            SandboxService::new(
                SessionResolver::for_config(self.paths(), config),
                self.session,
                SandboxPorts::none().with_recorder(self.recorder.clone()),
            )
        }

        /// Wie [`Fixture::service`], aber mit einem Resolver, der wie der
        /// Daemon von der Platte liest; nur so löst ein `Plan` frisch auf.
        fn live_service(&self) -> SandboxService {
            let paths = self.paths();
            let base = humanitl_config::resolve(
                &humanitl_config::ProfileSelection::default(),
                None,
                paths.env(),
                &[],
            )
            .unwrap();
            SandboxService::new(
                SessionResolver::new(paths, base),
                self.session,
                SandboxPorts::none().with_recorder(self.recorder.clone()),
            )
        }

        fn folder(&self, name: &str) -> std::path::PathBuf {
            let dir = self.home.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }
    }

    /// Der Fall des Eigentümers (`SANDBOX_049` aus einem Starter, HUM-532):
    /// Die App, aus einem Starter des Desktops geöffnet, schickt einen Start
    /// ohne Ordner, und `sandbox.work_dir` ist leer. Bis HUM-532 nahm der
    /// Daemon dann sein eigenes Arbeitsverzeichnis (hier: das der Tests, ein
    /// echter Ordner), und die Sitzung lief in einem Ordner, den niemand
    /// gewählt hatte. Jetzt: `SANDBOX_053` mit `why` und `fix`, keine
    /// Sandbox, kein Eintrag im Register.
    ///
    /// Mutationsbeweis: Der Rückfall auf das Arbeitsverzeichnis des Daemons in
    /// [`super::Inner::work_dir`] wiederhergestellt färbt den Test rot.
    #[tokio::test]
    async fn a_start_without_a_project_folder_is_sandbox_053_and_registers_nothing() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let start = v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
                profile: "default".to_owned(),
                command: vec!["/bin/true".to_owned()],
                sandbox_mode: "direct".to_owned(),
                ..v1::sandbox_request::Start::default()
            })),
        };
        let events: Vec<v1::SandboxEvent> = service.stream(start).collect().await;

        let refused = events
            .iter()
            .find_map(|event| match &event.event {
                Some(v1::sandbox_event::Event::Diagnostic(found)) => Some(found.clone()),
                _ => None,
            })
            .expect("the start says why it stopped");
        assert_eq!(refused.code, "SANDBOX_053", "{events:?}");
        assert!(!refused.why.is_empty());
        assert!(refused.fix.is_some(), "the finding names a way to a folder");
        assert!(
            !events.iter().any(|event| matches!(
                &event.event,
                Some(v1::sandbox_event::Event::Status(status))
                    if status.state == v1::SandboxState::Running as i32
                        || status.state == v1::SandboxState::Starting as i32
            )),
            "no sandbox: {events:?}"
        );
        assert!(service.project_activity().is_none());
        assert!(
            fixture
                .recorder
                .list_projects(10, 0)
                .await
                .unwrap()
                .is_empty()
        );
    }

    /// Ein bekannter Ordner `name` mit dieser gespeicherten Wahl.
    async fn known(fixture: &Fixture, name: &str, choice: &str) -> std::path::PathBuf {
        let dir = fixture.folder(name);
        let identity = ProjectIdentity::of(&dir).unwrap();
        let row = fixture
            .recorder
            .register_project_start(ProjectStart {
                session: fixture.session,
                path: identity.path_text(),
                dev: identity.dev,
                ino: identity.ino,
                agent: "opencode".to_owned(),
                at: SystemTime::now(),
            })
            .await
            .unwrap()
            .project;
        fixture
            .recorder
            .save_project_choice(row.id, Some(choice.to_owned()))
            .await
            .unwrap();
        dir
    }

    fn plan_for(dir: &std::path::Path) -> v1::sandbox_request::Plan {
        v1::sandbox_request::Plan {
            work_dir: dir.display().to_string(),
            ..v1::sandbox_request::Plan::default()
        }
    }

    /// Die Projektwahl eines bekannten Ordners erreicht die Auflösung des
    /// Starts, auch für Schlüssel, die ein Start ohne Schalter leer lässt
    /// (`work_mode`, `ask_mode`); ein anderer Ordner am alten Pfad (neuer
    /// Inode, sicher verschieden) startet ohne sie.
    #[tokio::test]
    async fn the_choice_of_a_known_folder_reaches_the_start_and_a_replaced_one_does_not() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let dir = known(
            &fixture,
            "project",
            r#"{"hold.ask_mode":"none","sandbox.work_mode":"ro"}"#,
        )
        .await;
        let plan = plan_for(&dir);
        let inner = &service.inner;
        let work_dir = inner.remember(&plan).unwrap();
        let project = inner.start_project(&plan).await.unwrap();
        let choice = project.choice.clone().expect("the stored choice applies");
        assert_eq!(choice.get("hold.ask_mode"), Some("none"));
        // Ein Start ohne Schalter, wie `humanitl run` ihn seit HUM-532
        // schickt: leere Felder.
        inner
            .apply_session_with_choice(
                &v1::sandbox_request::Start::default(),
                work_dir,
                Some(choice),
            )
            .unwrap();
        assert_eq!(inner.config().hold.ask_mode, AskMode::None);
        assert_eq!(
            inner.config().sandbox.work_mode,
            humanitl_config::WorkMode::Ro
        );

        // Ein anderer Ordner am selben Pfad: Er entsteht, solange der alte
        // noch da ist, und wird dann darübergeschoben; sein Inode ist ein
        // anderer.
        let other = fixture.folder("elsewhere");
        std::fs::rename(&other, &dir).unwrap();
        let replaced = inner.start_project(&plan).await.unwrap();
        assert!(replaced.choice.is_none(), "a replaced folder has no choice");
    }

    /// Die Wahl von B wird gegen die Auflösung von B geprüft, nicht gegen die
    /// Konfiguration der letzten Sitzung, die die strengere Wahl von A trägt.
    ///
    /// Mutationsbeweis: Prüfte `start_project` wieder gegen
    /// `self.config()`, lehnte es B mit `CONFIG_024` ab, und der Test wird rot.
    #[tokio::test]
    async fn the_choice_of_b_is_not_judged_by_the_session_of_a() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let inner = &service.inner;
        let a = fixture.folder("a");
        let strict =
            humanitl_config::ProjectChoice::from_pairs([("sandbox.min_isolation", "namespaces")])
                .unwrap();
        inner.remember(&plan_for(&a)).unwrap();
        inner
            .apply_session_with_choice(
                &v1::sandbox_request::Start::default(),
                Some(a.clone()),
                Some(strict),
            )
            .unwrap();
        assert_eq!(
            inner.config().sandbox.min_isolation,
            humanitl_config::MinIsolation::Namespaces
        );

        let b = known(&fixture, "b", r#"{"sandbox.min_isolation":"policy"}"#).await;
        let plan = plan_for(&b);
        inner.remember(&plan).unwrap();
        let project = inner.start_project(&plan).await.expect("B starts");
        assert!(project.choice.is_some());
    }

    /// Die Vorschau löst mit der gespeicherten Projektwahl auf wie der Start
    /// (HUM-559): Die Wahl hebt die Untergrenze auf `kernel` (mit dem VM-Backend, so
    /// dass sie lädt), die Kommandozeile schickt `policy` und `auto`. Der
    /// Start behält `kernel` und lehnt `auto` mit `SANDBOX_035` ab; der Plan
    /// muss dasselbe zeigen, nicht `policy`.
    ///
    /// Mutationsbeweis: Ohne die Wahl in `resolve_plan` (oder ohne
    /// `plan_project_choice` im Arm von `Op::Plan`) zeigt der Plan `auto`
    /// ohne `SANDBOX_035`, und der Test wird rot.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_preview_resolves_with_the_saved_choice_like_the_start() {
        let fixture = Fixture::new();
        let service = fixture.live_service();
        let dir = known(
            &fixture,
            "project",
            &format!(
                r#"{{"sandbox.min_isolation":"kernel","sandbox.backend":"{}"}}"#,
                humanitl_sandbox::VM_BACKEND
            ),
        )
        .await;
        let overrides = vec![
            v1::sandbox_request::CliOverride {
                path: "sandbox.min_isolation".to_owned(),
                value: "policy".to_owned(),
            },
            v1::sandbox_request::CliOverride {
                path: "sandbox.backend".to_owned(),
                value: humanitl_sandbox::AUTO_BACKEND.to_owned(),
            },
        ];
        let codes = |events: &[v1::SandboxEvent]| -> Vec<String> {
            events
                .iter()
                .filter_map(|event| match &event.event {
                    Some(v1::sandbox_event::Event::Diagnostic(found)) => Some(found.code.clone()),
                    _ => None,
                })
                .collect()
        };
        let plan: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Plan(v1::sandbox_request::Plan {
                    cli_overrides: overrides.clone(),
                    ..plan_for(&dir)
                })),
            })
            .collect()
            .await;
        assert!(
            codes(&plan).contains(&"SANDBOX_035".to_owned()),
            "{:?}",
            codes(&plan)
        );

        let start: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
                    work_dir: dir.display().to_string(),
                    command: vec!["/bin/true".to_owned()],
                    sandbox_mode: "direct".to_owned(),
                    cli_overrides: overrides,
                    ..v1::sandbox_request::Start::default()
                })),
            })
            .collect()
            .await;
        assert!(
            codes(&start).contains(&"SANDBOX_035".to_owned()),
            "{:?}",
            codes(&start)
        );
    }

    /// Die Ereignisse eines `Plan`, und der Status darin.
    async fn plan_answer(
        service: &SandboxService,
        plan: v1::sandbox_request::Plan,
    ) -> (Vec<String>, Option<v1::sandbox_event::Status>) {
        let events: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Plan(plan)),
            })
            .collect()
            .await;
        let mut codes = Vec::new();
        let mut status = None;
        for event in events {
            match event.event {
                Some(v1::sandbox_event::Event::Diagnostic(found)) => codes.push(found.code),
                Some(v1::sandbox_event::Event::Status(found)) => status = Some(found),
                _ => {}
            }
        }
        (codes, status)
    }

    /// Eine gespeicherte Wahl, die nicht mehr lädt, hält den Plan auf wie den
    /// Start (HUM-559): `CONFIG_024`, statt ohne die Wahl eine schwächere
    /// Untergrenze zu zeigen.
    ///
    /// Mutationsbeweis: `plan_project_choice` mit `.ok()` statt `?` (der
    /// Befund verworfen) zeigt den Plan ohne `CONFIG_024`, und der Test wird
    /// rot.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_choice_that_no_longer_loads_stops_the_preview_like_the_start() {
        let fixture = Fixture::new();
        let service = fixture.live_service();
        let dir = known(&fixture, "floor", r#"{"sandbox.min_isolation":"kernel"}"#).await;
        let (codes, status) = plan_answer(&service, plan_for(&dir)).await;
        assert!(codes.contains(&"CONFIG_024".to_owned()), "{codes:?}");
        assert_eq!(
            status.expect("a status follows").state,
            v1::SandboxState::Failed as i32
        );
    }

    /// Ohne Ordner in der Anfrage lädt der Plan die Wahl des Ordners, den die
    /// Konfiguration jetzt nennt, nicht den aus dem Zwischenstand des Daemons
    /// (HUM-559): `sandbox.work_dir` wechselt von A (mit Wahl `ask_mode =
    /// none`) zu B (ohne Wahl), und der Plan für B zeigt die Wahl von A nicht.
    ///
    /// Mutationsbeweis: `plan_project_choice` mit dem Ordner aus
    /// `self.config()` statt `stored_work_dir` lädt die Wahl von A, und der
    /// Plan zeigt `none`.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_preview_loads_the_choice_of_the_folder_configured_now() {
        let fixture = Fixture::new();
        let a = known(&fixture, "a", r#"{"hold.ask_mode":"none"}"#).await;
        let b = fixture.folder("b");
        let config = fixture.paths().config_path();
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        let point_at = |dir: &std::path::Path| {
            std::fs::write(
                &config,
                format!("[sandbox]\nwork_dir = {:?}\n", dir.display().to_string()),
            )
            .unwrap();
        };
        point_at(&a);
        let service = fixture.live_service();
        let (_, at_a) = plan_answer(&service, v1::sandbox_request::Plan::default()).await;
        assert_eq!(at_a.expect("a status").ask_mode, "none");

        point_at(&b);
        let (codes, at_b) = plan_answer(&service, v1::sandbox_request::Plan::default()).await;
        let at_b = at_b.expect("a status");
        assert_eq!(at_b.work_dir, b.display().to_string(), "{codes:?}");
        assert_ne!(at_b.ask_mode, "none", "{codes:?}");
    }

    /// Eine gespeicherte Wahl, die nicht mehr lädt, hält den Start auf
    /// (`CONFIG_024`), statt still ohne sie zu starten: ein fremder
    /// Schlüssel, und eine Untergrenze, die kein gewähltes Backend erreicht.
    ///
    /// Mutationsbeweis: Ohne den Schritt mit `choose_backend` in
    /// `crate::projects::check_for` startete die Wahl `kernel` mit dem
    /// Vorgabe-Backend, und der Test wird rot.
    #[tokio::test]
    async fn a_stored_choice_that_no_longer_loads_stops_the_start() {
        let fixture = Fixture::new();
        let service = fixture.service();
        for (name, stored) in [
            ("foreign", r#"{"agent.command":"sh"}"#),
            ("floor", r#"{"sandbox.min_isolation":"kernel"}"#),
        ] {
            let dir = known(&fixture, name, stored).await;
            let plan = plan_for(&dir);
            service.inner.remember(&plan).unwrap();
            let refused = service.inner.start_project(&plan).await.unwrap_err();
            assert_eq!(
                refused.code.as_str(),
                "CONFIG_024",
                "{name}: {}",
                refused.why
            );
        }
    }

    /// Ändert sich der Ordner zwischen dem Laden der Wahl und dem Eintrag,
    /// endet der Start (`SANDBOX_042`), und nichts steht im Register.
    ///
    /// Mutationsbeweis: Ohne den zweiten Blick auf den Ordner in
    /// `record_project_start` stünde der Start im Register, und der Test wird
    /// rot.
    #[tokio::test]
    async fn a_folder_swapped_after_the_lookup_is_not_registered() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let dir = fixture.folder("swapped");
        let plan = plan_for(&dir);
        service.inner.remember(&plan).unwrap();
        let project = service.inner.start_project(&plan).await.unwrap();
        let other = fixture.folder("intruder");
        std::fs::rename(&other, &dir).unwrap();

        let refused = service
            .inner
            .record_project_start(&fixture.recorder, &project)
            .await
            .unwrap_err();
        assert_eq!(refused.code.as_str(), "SANDBOX_042");
        assert!(
            fixture
                .recorder
                .list_projects(10, 0)
                .await
                .unwrap()
                .is_empty()
        );

        // Derselbe Ordner: eingetragen.
        let same = service.inner.start_project(&plan).await.unwrap();
        service
            .inner
            .record_project_start(&fixture.recorder, &same)
            .await
            .unwrap();
        assert_eq!(
            fixture.recorder.list_projects(10, 0).await.unwrap().len(),
            1
        );
    }
}
