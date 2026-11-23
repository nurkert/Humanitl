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
            proposal: crate::projects::proposal::Ports::default(),
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
                ..Default::default()
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
        terminal_clients: 2,
        sandbox: "sbx-new".to_owned(),
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
    // Die Clients am Terminal der laufenden Sitzung (#13).
    assert_eq!(first.terminal_clients, 2);
    assert_eq!(first.sandbox_id, "sbx-new");
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
    assert_eq!(rest.projects[0].terminal_clients, 0);
    assert!(rest.projects[0].sandbox_id.is_empty());
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

fn save_folder(path: &std::path::Path, pairs: &[(&str, &str)]) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::SaveChoice(
            v1::projects_request::SaveChoice {
                path: path.display().to_string(),
                values: pairs
                    .iter()
                    .map(|(key, value)| v1::ChoiceValue {
                        key: (*key).to_owned(),
                        value: (*value).to_owned(),
                    })
                    .collect(),
                ..Default::default()
            },
        )),
    }
}

/// Der Erststart-Bildschirm speichert vor dem ersten Start (HUM-538): Die
/// Wahl eines Ordners, den das Register nicht kennt, legt seinen Eintrag ohne
/// Start an; ein zweites Speichern am selben Ordner trifft dieselbe Zeile.
///
/// Mutationsbeweis: `save_choice` ohne den Zweig für den Ordner sucht die
/// leere Kennung, antwortet `CONFIG_003`, und der Test wird rot.
#[tokio::test]
async fn a_choice_by_folder_registers_an_unknown_folder() {
    let fixture = Fixture::new("");
    let dir = fixture.home.path().join("fresh");
    std::fs::create_dir_all(&dir).unwrap();
    let saved = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(&dir, &[("hold.ask_mode", "none")]),
    )
    .await
    .unwrap();
    let entry = saved.project.expect("the project comes back");
    assert!(entry.has_choice);
    assert_eq!(entry.runs, 0);
    assert_eq!(entry.last_started, None);
    assert_eq!(entry.display_name, "fresh");

    let again = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(&dir, &[("hold.ask_mode", "terminal")]),
    )
    .await
    .unwrap();
    assert_eq!(again.project.unwrap().project_id, entry.project_id);
    let identity = ProjectIdentity::of(&dir).unwrap();
    let row = fixture
        .recorder
        .current_project_at(identity.path_text(), Some((identity.dev, identity.ino)))
        .await
        .unwrap()
        .expect("the folder is known now");
    assert_eq!(
        row.choice.as_deref(),
        Some(r#"{"hold.ask_mode":"terminal"}"#)
    );
}

/// Eine Wahl, die nicht gilt, legt auch für einen unbekannten Ordner nichts
/// an; ein fehlender Ordner und Kennung neben Ordner sind `CONFIG_003`.
#[tokio::test]
async fn a_refused_choice_by_folder_registers_nothing() {
    let fixture = Fixture::new("[sandbox]\nmin_isolation = \"namespaces\"\n");
    let dir = fixture.home.path().join("fresh");
    std::fs::create_dir_all(&dir).unwrap();
    let refused = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(&dir, &[("sandbox.min_isolation", "policy")]),
    )
    .await
    .expect_err("below the floor");
    assert_eq!(refused.code.as_str(), "CONFIG_024");
    assert!(
        fixture
            .recorder
            .list_projects(10, 0)
            .await
            .unwrap()
            .is_empty()
    );

    let missing = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(&fixture.home.path().join("gone"), &[]),
    )
    .await
    .expect_err("no folder");
    assert_eq!(missing.code.as_str(), "CONFIG_003");

    let project = fixture.project("repo").await;
    let mut both = save(&project, &[]);
    if let Some(v1::projects_request::Op::SaveChoice(save)) = both.op.as_mut() {
        save.path = dir.display().to_string();
    }
    let refused = answer(fixture.context(PolicyLocks::none(), None), both)
        .await
        .expect_err("id and folder");
    assert_eq!(refused.code.as_str(), "CONFIG_003");
}

/// The folder exactly as sent: a choice for `repo ` (trailing space) is a
/// project of its own, not the one at `repo` (HUM-538).
///
/// Mutation proof: trimming `SaveChoice.path` in `save_folder_choice`
/// again saves onto the row of `repo` and makes this test red.
#[tokio::test]
async fn a_folder_name_ending_in_a_space_gets_its_own_entry() {
    let fixture = Fixture::new("");
    let project = fixture.project("repo").await;
    let spaced = fixture.home.path().join("repo ");
    std::fs::create_dir_all(&spaced).unwrap();
    let saved = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(&spaced, &[("hold.ask_mode", "none")]),
    )
    .await
    .unwrap()
    .project
    .expect("the project comes back");
    assert_ne!(saved.project_id, project.id.to_string());
    let untouched = fixture
        .recorder
        .get_project(project.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.choice, None);
}

/// The same folder test as a start (`check_work_dir`) runs before anything
/// is written: the home folder itself is `SANDBOX_006`, and the register
/// stays empty (HUM-538).
///
/// Mutation proof: without the call of `work_dir::check_requested` in
/// `save_folder_choice` the home folder is registered and this test is red.
#[tokio::test]
async fn a_choice_for_the_home_folder_is_refused_like_a_start() {
    let fixture = Fixture::new("");
    let refused = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(fixture.home.path(), &[]),
    )
    .await
    .expect_err("the home folder is no project");
    assert_eq!(refused.code.as_str(), "SANDBOX_006", "{}", refused.why);
    assert!(
        fixture
            .recorder
            .list_projects(10, 0)
            .await
            .unwrap()
            .is_empty()
    );
}

/// A folder outside the home directory that the configuration does not
/// declare is `SANDBOX_006`, as a start answers, and nothing is registered:
/// the folder of the request does not declare itself (HUM-538).
///
/// Mutation proof: checking against the resolution of the asked folder
/// (`check_for(..)?.config.sandbox.work_dir`) instead of the configured
/// `sandbox.work_dir` lets the folder through and makes this test red.
#[tokio::test]
async fn an_undeclared_folder_outside_home_is_refused_like_a_start() {
    let fixture = Fixture::new("");
    let outside = tempfile::tempdir().unwrap();
    let refused = answer(
        fixture.context(PolicyLocks::none(), None),
        save_folder(outside.path(), &[]),
    )
    .await
    .expect_err("an undeclared folder outside home is no project");
    assert_eq!(refused.code.as_str(), "SANDBOX_006", "{}", refused.why);
    assert!(
        fixture
            .recorder
            .list_projects(10, 0)
            .await
            .unwrap()
            .is_empty()
    );
}
