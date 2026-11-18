//! Tests of `Projects` (HUM-532).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::SystemTime;

use humanitl_config::{Env, Paths, PolicyLocks};
use humanitl_core::SessionId;
use humanitl_recorder::{ProjectRow, ProjectStart, Recorder, RecorderSettings, SessionMeta};

use super::{Context, ProjectActivity, ProjectIdentity, answer};
use crate::v1;

struct Fixture {
    home: tempfile::TempDir,
    recorder: Recorder,
    session: SessionId,
}

impl Fixture {
    fn new(config_toml: &str) -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config/humanitl");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("config.toml"), config_toml).unwrap();
        let recorder = Recorder::open(
            &home.path().join("data/humanitl/humanitl.db"),
            &home.path().join("data/humanitl/blobs"),
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
        ]))
    }

    fn context(&self, locks: PolicyLocks, activity: Option<ProjectActivity>) -> Context {
        Context {
            recorder: Some(self.recorder.clone()),
            paths: self.paths(),
            locks,
            activity,
            sandbox: None,
            audit: None,
        }
    }

    async fn project(&self, name: &str) -> ProjectRow {
        let dir = self.home.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let identity = ProjectIdentity::of(&dir).unwrap();
        self.recorder
            .register_project_start(ProjectStart {
                session: self.session,
                path: identity.path_text(),
                dev: identity.dev,
                ino: identity.ino,
                agent: "claude".to_owned(),
                at: SystemTime::now(),
            })
            .await
            .unwrap()
            .project
    }
}

fn save(project: &ProjectRow, pairs: &[(&str, &str)]) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::SaveChoice(
            v1::projects_request::SaveChoice {
                project_id: project.id.to_string(),
                values: pairs
                    .iter()
                    .map(|(key, value)| v1::ChoiceValue {
                        key: (*key).to_owned(),
                        value: (*value).to_owned(),
                    })
                    .collect(),
            },
        )),
    }
}

fn get(project_id: &str) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
            project_id: project_id.to_owned(),
            path: String::new(),
        })),
    }
}

/// Unter dem globalen `sandbox.min_isolation`, ein fremder Schlüssel, ein
/// durch die Firmenvorgabe gesperrter Wert: je `CONFIG_024`, und nichts ist
/// gespeichert.
#[tokio::test]
async fn save_choice_refuses_with_config_024_and_saves_nothing() {
    let fixture = Fixture::new("[sandbox]\nmin_isolation = \"namespaces\"\n");
    let project = fixture.project("repo").await;
    let locks = PolicyLocks::none().with("hold.ask_mode", "terminal", "/etc/humanitl/policy.toml");

    for pairs in [
        &[("sandbox.min_isolation", "policy")][..],
        &[("limits.hold_body_cap_bytes", "1")][..],
        &[("hold.ask_mode", "none")][..],
        // Eine Untergrenze, die kein Backend der Wahl erreicht: die
        // Kernel-Stufe über dem Vorgabe-Backend.
        &[("sandbox.min_isolation", "kernel")][..],
    ] {
        let refused = answer(fixture.context(locks.clone(), None), save(&project, pairs))
            .await
            .expect_err("refused");
        assert_eq!(
            refused.code.as_str(),
            "CONFIG_024",
            "{pairs:?}: {}",
            refused.why
        );
        let stored = fixture
            .recorder
            .get_project(project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.choice, None, "{pairs:?}");
        assert_eq!(stored.version, project.version, "{pairs:?}");
    }
}

/// Eine erlaubte Wahl wird gespeichert, und `get` nennt je Schlüssel Wahl,
/// Wert und Herkunft `project-choice`; `GetConfig` mit der Kennung ebenso.
#[tokio::test]
async fn a_saved_choice_shows_with_the_origin_project_choice() {
    let fixture = Fixture::new("");
    let project = fixture.project("repo").await;
    let saved = answer(
        fixture.context(PolicyLocks::none(), None),
        save(
            &project,
            &[
                ("hold.ask_mode", "none"),
                ("sandbox.min_isolation", "policy"),
            ],
        ),
    )
    .await
    .unwrap();
    let entry = saved.project.expect("the project comes back");
    assert!(entry.has_choice);
    assert!(entry.version > project.version);
    let field = |key: &str| {
        saved
            .choice
            .iter()
            .find(|field| field.key == key)
            .cloned()
            .unwrap()
    };
    assert_eq!(field("hold.ask_mode").chosen, "none");
    assert_eq!(field("hold.ask_mode").effective, "none");
    assert_eq!(field("hold.ask_mode").origin, "project-choice");
    assert_eq!(field("sandbox.backend").chosen, "");
    assert_eq!(field("sandbox.backend").origin, "default");
    assert_eq!(
        saved.choice.len(),
        humanitl_config::PROJECT_CHOICE_KEYS.len()
    );

    let row = fixture
        .recorder
        .get_project(project.id)
        .await
        .unwrap()
        .unwrap();
    let snapshot =
        crate::config_rpc::project_snapshot(&fixture.paths(), &PolicyLocks::none(), &row);
    assert!(
        snapshot
            .origins
            .iter()
            .any(|origin| origin.key == "hold.ask_mode" && origin.origin == "project-choice"),
        "{:?}",
        snapshot.origins
    );

    let again = answer(
        fixture.context(PolicyLocks::none(), None),
        get(&project.id.to_string()),
    )
    .await
    .unwrap();
    assert_eq!(again.choice, saved.choice);
}

/// Eine Wahl, die nach dem Speichern nicht mehr gilt (die Untergrenze in
/// `config.toml` stieg), steht in `get` als `CONFIG_024`; die Werte sind die
/// ohne sie.
#[tokio::test]
async fn a_choice_that_no_longer_loads_is_named_not_dropped() {
    let fixture = Fixture::new("");
    let project = fixture.project("repo").await;
    answer(
        fixture.context(PolicyLocks::none(), None),
        save(&project, &[("sandbox.min_isolation", "policy")]),
    )
    .await
    .unwrap();
    std::fs::write(
        fixture.home.path().join("config/humanitl/config.toml"),
        "[sandbox]\nmin_isolation = \"namespaces\"\n",
    )
    .unwrap();
    let shown = answer(
        fixture.context(PolicyLocks::none(), None),
        get(&project.id.to_string()),
    )
    .await
    .unwrap();
    assert_eq!(
        shown.diagnostics.first().map(|found| found.code.as_str()),
        Some("CONFIG_024")
    );
    let floor = shown
        .choice
        .iter()
        .find(|field| field.key == "sandbox.min_isolation")
        .unwrap();
    assert_eq!(floor.chosen, "policy");
    assert_eq!(floor.effective, "namespaces");
}

/// `list`: neueste zuerst, mit laufender Sitzung, wartenden Änderungen,
/// fehlendem Ordner und Seiten.
#[tokio::test]
async fn list_names_the_running_session_and_a_missing_folder() {
    let fixture = Fixture::new("");
    let old = fixture.project("old").await;
    let new = fixture.project("new").await;
    std::fs::remove_dir(fixture.home.path().join("old")).unwrap();
    let activity = ProjectActivity {
        project: new.id,
        running: true,
        changes_waiting: true,
        session: fixture.session,
    };
    let list = |cursor: &str, limit: u32| v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::List(v1::projects_request::List {
            limit,
            cursor: cursor.to_owned(),
        })),
    };
    let page = answer(
        fixture.context(PolicyLocks::none(), Some(activity.clone())),
        list("", 1),
    )
    .await
    .unwrap();
    assert_eq!(page.projects.len(), 1);
    let first = &page.projects[0];
    assert_eq!(first.project_id, new.id.to_string());
    assert!(first.running && first.changes_waiting && !first.missing);
    assert_eq!(first.session_id, fixture.session.to_string());
    assert_eq!(first.last_agent, "claude");
    assert_eq!(first.runs, 1);
    assert!(!page.next_cursor.is_empty());

    let rest = answer(
        fixture.context(PolicyLocks::none(), Some(activity)),
        list(&page.next_cursor, 1),
    )
    .await
    .unwrap();
    assert_eq!(rest.projects.len(), 1);
    assert_eq!(rest.projects[0].project_id, old.id.to_string());
    assert!(rest.projects[0].missing);
    assert!(!rest.projects[0].running);
    assert!(rest.projects[0].session_id.is_empty());
    assert!(rest.next_cursor.is_empty());
}

/// `get` an einem Ordner: kanonisiert, wie der Start ihn einträgt.
#[tokio::test]
async fn get_by_folder_finds_the_current_entry() {
    let fixture = Fixture::new("");
    let project = fixture.project("repo").await;
    let found = answer(
        fixture.context(PolicyLocks::none(), None),
        v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                project_id: String::new(),
                path: format!("{}/./repo", fixture.home.path().display()),
            })),
        },
    )
    .await
    .unwrap();
    assert_eq!(found.project.unwrap().project_id, project.id.to_string());

    let unknown = answer(fixture.context(PolicyLocks::none(), None), get("not-an-id"))
        .await
        .unwrap_err();
    assert_eq!(unknown.code.as_str(), "CONFIG_003");
}

/// Ohne Aufzeichnung gibt es kein Register: `IPC_006`.
#[tokio::test]
async fn without_a_recording_the_register_is_ipc_006() {
    let fixture = Fixture::new("");
    let ctx = Context {
        recorder: None,
        ..fixture.context(PolicyLocks::none(), None)
    };
    let refused = answer(ctx, get("x")).await.unwrap_err();
    assert_eq!(refused.code.as_str(), "IPC_006");
}

/// Ein Name mit Steuerzeichen verlässt den Daemon gesäubert.
#[tokio::test]
async fn control_characters_in_a_folder_name_are_cleaned() {
    let fixture = Fixture::new("");
    let project = fixture.project("bad\u{1b}[31mname").await;
    let shown = answer(
        fixture.context(PolicyLocks::none(), None),
        get(&project.id.to_string()),
    )
    .await
    .unwrap()
    .project
    .unwrap();
    assert!(
        !shown.display_name.contains('\u{1b}'),
        "{:?}",
        shown.display_name
    );
    assert!(!shown.path.contains('\u{1b}'), "{:?}", shown.path);
}
