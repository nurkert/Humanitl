//! `GetConfig` mit Katalog, Projektbezug und Sperren, `SetConfig` gegen
//! Projektwahl und Sperre, und `FlowEvent.config_changed` an zwei Abonnenten
//! (HUM-534).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use humanitl_config::{Config, Env, Paths, PolicyLocks};
use humanitl_core::SessionId;
use humanitl_recorder::{ProjectRow, ProjectStart, Recorder, RecorderSettings, SessionMeta};
use tokio_stream::StreamExt as _;
use tonic::Request;

use crate::projects::ProjectIdentity;
use crate::server::IpcServer;
use crate::v1;
use crate::v1::humanitl_server::Humanitl as _;

const POLICY: &str = "/etc/humanitl/policy.toml";

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

    fn server(&self, locks: PolicyLocks) -> IpcServer {
        IpcServer::over_the_recording(&Config::default(), Some(self.session))
            .with_paths(self.paths())
            .with_recorder(self.recorder.clone())
            .with_policy_locks(locks)
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

    fn config_text(&self) -> String {
        std::fs::read_to_string(self.paths().config_path()).unwrap()
    }
}

async fn save_choice(server: &IpcServer, project: &ProjectRow, mode: &str) {
    server
        .projects(Request::new(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    project_id: project.id.to_string(),
                    values: vec![v1::ChoiceValue {
                        key: "sandbox.mode".to_owned(),
                        value: mode.to_owned(),
                    }],
                },
            )),
        }))
        .await
        .unwrap();
}

async fn catalog(server: &IpcServer, project: &str, language: &str) -> v1::ConfigSnapshot {
    server
        .get_config(Request::new(v1::GetConfigRequest {
            include_schema: false,
            project_id: project.to_owned(),
            include_catalog: true,
            language: language.to_owned(),
        }))
        .await
        .unwrap()
        .into_inner()
}

fn mode(snapshot: &v1::ConfigSnapshot) -> (String, String) {
    let values: serde_json::Value = serde_json::from_str(&snapshot.values_json).unwrap();
    let origin = snapshot
        .origins
        .iter()
        .find(|origin| origin.key == "sandbox.mode")
        .map(|origin| origin.origin.clone())
        .unwrap_or_default();
    (
        values["sandbox"]["mode"].as_str().unwrap().to_owned(),
        origin,
    )
}

fn entry<'a>(snapshot: &'a v1::ConfigSnapshot, id: &str) -> &'a v1::SettingsEntry {
    snapshot
        .catalog
        .as_ref()
        .expect("the catalog was asked for")
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("{id} is in the catalog"))
}

/// Zwei Projekte mit verschiedener Wahl: `GetConfig(project = A)` und
/// `(project = B)` liefern je ihren Wert mit Herkunft `project-choice`, ohne
/// Projekt den globalen. Mutationsbeweis: Ignorierte `get_config` das
/// Projektfeld, wären A und B `direct` aus `global`, und der Test wird rot.
#[tokio::test]
async fn each_project_resolves_its_own_choice_and_without_one_the_global_value() {
    let fixture = Fixture::new("[sandbox]\nmode = \"direct\"\n");
    let server = fixture.server(PolicyLocks::none());
    let a = fixture.project("a").await;
    let b = fixture.project("b").await;
    save_choice(&server, &a, "disposable").await;
    // `rw` des Auftrags heißt hier `direct`: Der Schlüssel kennt `auto`,
    // `direct` und `disposable`.
    save_choice(&server, &b, "direct").await;

    let of_a = catalog(&server, &a.id.to_string(), "").await;
    let of_b = catalog(&server, &b.id.to_string(), "").await;
    let global = catalog(&server, "", "").await;
    assert_eq!(
        mode(&of_a),
        ("disposable".to_owned(), "project-choice".to_owned())
    );
    assert_eq!(
        mode(&of_b),
        ("direct".to_owned(), "project-choice".to_owned())
    );
    assert_eq!(mode(&global), ("direct".to_owned(), "global".to_owned()));
    // Der Ordner statt der Kennung findet dasselbe Projekt.
    let by_path = catalog(&server, &a.path, "").await;
    assert_eq!(mode(&by_path).1, "project-choice");
    // Ein unbekanntes Projekt ist kein Fehler: global, ohne Wahl.
    for unknown in [
        uuid_like(),
        "not-an-id".to_owned(),
        fixture.home.path().join("nowhere").display().to_string(),
    ] {
        let snapshot = catalog(&server, &unknown, "").await;
        assert_eq!(
            mode(&snapshot),
            ("direct".to_owned(), "global".to_owned()),
            "{unknown}"
        );
    }
}

fn uuid_like() -> String {
    "00000000-0000-4000-8000-000000000000".to_owned()
}

/// `SetConfig` auf `sandbox.mode` ändert die globale Datei, nie die Wahl von
/// A oder B; `save_choice` für A ändert nur A.
#[tokio::test]
async fn set_config_writes_the_global_file_and_save_choice_only_its_project() {
    let fixture = Fixture::new("");
    let server = fixture.server(PolicyLocks::none());
    let a = fixture.project("a").await;
    let b = fixture.project("b").await;
    save_choice(&server, &a, "disposable").await;
    save_choice(&server, &b, "direct").await;

    server
        .set_config(Request::new(v1::SetConfigRequest {
            key: "sandbox.mode".to_owned(),
            value: "auto".to_owned(),
            expected_server: String::new(),
        }))
        .await
        .unwrap();
    assert!(
        fixture.config_text().contains("mode = \"auto\""),
        "{}",
        fixture.config_text()
    );
    assert_eq!(
        mode(&catalog(&server, &a.id.to_string(), "").await).0,
        "disposable"
    );
    assert_eq!(
        mode(&catalog(&server, &b.id.to_string(), "").await).0,
        "direct"
    );

    save_choice(&server, &a, "direct").await;
    assert_eq!(
        mode(&catalog(&server, &a.id.to_string(), "").await).0,
        "direct"
    );
    assert_eq!(
        mode(&catalog(&server, &b.id.to_string(), "").await).0,
        "direct"
    );
    let stored_b = fixture.recorder.get_project(b.id).await.unwrap().unwrap();
    assert_eq!(
        stored_b.choice.as_deref(),
        Some(r#"{"sandbox.mode":"direct"}"#)
    );
    assert!(fixture.config_text().contains("mode = \"auto\""));
}

/// Herkunft und Sperre je Eintrag: Ein gesperrter Schlüssel hat `lock` mit
/// der Vorgabe, seine Herkunft ist `policy`, und `SetConfig` darauf wird mit
/// `AGENT_010` abgelehnt, ohne dass die Datei sich ändert. Mutationsbeweis:
/// Prüfte `set_locked` die Sperre nicht, ginge das Schreiben durch.
#[tokio::test]
async fn a_locked_key_carries_its_lock_and_set_config_on_it_is_refused() {
    let fixture = Fixture::new("[hold]\ntimeout_secs = 30\n");
    let locks = PolicyLocks::none().with("hold.ask_mode", "terminal", POLICY);
    let server = fixture.server(locks);
    let snapshot = catalog(&server, "", "").await;

    let locked = entry(&snapshot, "hold.ask_mode");
    let lock = locked
        .lock
        .as_ref()
        .expect("the policy locks hold.ask_mode");
    assert_eq!(lock.locked_by, POLICY);
    assert!(lock.read_only);
    assert_eq!(locked.origin, "policy");
    assert!(locked.in_project_choice);
    let values: serde_json::Value = serde_json::from_str(&snapshot.values_json).unwrap();
    assert_eq!(values["hold"]["ask_mode"], "terminal");

    let free = entry(&snapshot, "hold.timeout_secs");
    assert!(free.lock.is_none());
    assert_eq!(free.origin, "global");
    assert!(!free.in_project_choice);
    assert_eq!(entry(&snapshot, "daemon.restart").origin, "");

    let before = fixture.config_text();
    let refused = server
        .set_config(Request::new(v1::SetConfigRequest {
            key: "hold.ask_mode".to_owned(),
            value: "terminal".to_owned(),
            expected_server: String::new(),
        }))
        .await
        .unwrap_err();
    assert!(
        refused.message().contains("AGENT_010"),
        "{}",
        refused.message()
    );
    assert_eq!(fixture.config_text(), before);
}

/// Die Texte in der Sprache der Anfrage, sonst in `ui.language`; Kategorien
/// in ihrer Reihenfolge; ohne `sandbox.disposable` kein `sandbox.mode`.
#[tokio::test]
async fn the_catalog_speaks_the_asked_or_configured_language() {
    let fixture = Fixture::new("[ui]\nlanguage = \"de\"\n");
    let server = fixture.server(PolicyLocks::none());
    let german = catalog(&server, "", "").await;
    let english = catalog(&server, "", "en").await;
    let catalog_de = german.catalog.as_ref().unwrap();
    assert_eq!(catalog_de.language, "de");
    assert_eq!(catalog_de.categories[0].title, "Modelle");
    assert_eq!(
        english.catalog.as_ref().unwrap().categories[0].title,
        "Models"
    );
    let orders: Vec<u32> = catalog_de.categories.iter().map(|c| c.order).collect();
    assert_eq!(orders, (0..10).collect::<Vec<_>>());
    // Ohne Sandbox-Dienst kann dieser Server keine Wegwerf-Sandbox.
    assert!(
        catalog_de
            .entries
            .iter()
            .all(|entry| entry.id != "sandbox.mode")
    );
    let scale = entry(&german, "ui.scale");
    assert_eq!(scale.client, "app");
    assert_eq!(entry(&german, "llm.api_key_ref").control, "secret_ref");
}

/// Die Adapter des Katalogs sind die des Registers der Sandbox-Crate.
#[test]
fn the_agent_choices_are_the_adapters_of_the_registry() {
    let registry = humanitl_sandbox::AdapterRegistry::builtin();
    assert_eq!(
        humanitl_config::catalog::AGENT_ADAPTERS.to_vec(),
        registry.ids(),
        "catalog and adapter registry drifted"
    );
}

async fn next_config_change(
    stream: &mut crate::server_stub::BoxStream<Result<v1::FlowEvent, tonic::Status>>,
) -> v1::flow_event::ConfigChanged {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("a change arrives")
            .expect("the stream goes on")
            .unwrap();
        if let Some(v1::flow_event::Event::ConfigChanged(changed)) = event.event {
            return changed;
        }
    }
}

/// Zwei Abonnenten am selben Daemon (Fake-App und Fake-TUI): Ein `SetConfig`
/// erreicht beide mit dem Schlüssel; ein `save_choice` für A nennt A.
/// Mutationsbeweis: Sendete der Dienst nur bei `SetConfig`, bliebe der
/// zweite Fall stumm, und der Test wird rot.
#[tokio::test]
async fn config_changed_reaches_both_subscribers() {
    let fixture = Fixture::new("");
    let server = fixture.server(PolicyLocks::none());
    let a = fixture.project("a").await;
    let mut app = server
        .subscribe(Request::new(v1::SubscribeRequest::default()))
        .await
        .unwrap()
        .into_inner();
    let mut tui = server
        .subscribe(Request::new(v1::SubscribeRequest::default()))
        .await
        .unwrap()
        .into_inner();

    server
        .set_config(Request::new(v1::SetConfigRequest {
            key: "hold.timeout_secs".to_owned(),
            value: "45".to_owned(),
            expected_server: String::new(),
        }))
        .await
        .unwrap();
    for stream in [&mut app, &mut tui] {
        let changed = next_config_change(stream).await;
        assert_eq!(changed.keys, vec!["hold.timeout_secs".to_owned()]);
        assert_eq!(changed.project_id, "");
    }

    save_choice(&server, &a, "disposable").await;
    for stream in [&mut app, &mut tui] {
        let changed = next_config_change(stream).await;
        assert_eq!(changed.project_id, a.id.to_string());
        assert!(changed.keys.contains(&"sandbox.mode".to_owned()));
    }
}

/// Die Schreibwege neben `SetConfig` halten die Sperre ebenso (Review
/// HUM-534): `LlmServers`, `LlmKey`, `Egress` und `Agents` lehnen mit
/// `AGENT_010` ab, bevor sie `config.toml` oder den Schlüsselbund anfassen.
/// Mutationsbeweis: Ohne `policy_guard::refuse` im Dienst ginge `Add` durch
/// und `config.toml` trüge den Server.
#[tokio::test]
async fn every_other_writer_refuses_a_locked_key_before_writing() {
    let fixture = Fixture::new("# untouched\n");
    let locks = PolicyLocks::none()
        .with("llm.endpoint", "http://10.0.0.1:11434/", POLICY)
        .with("llm.api_key_ref", "-", POLICY)
        .with("egress.proxy_auth_ref", "-", POLICY)
        .with("agents.source", "depot", POLICY);
    let server = fixture.server(locks);
    let refused_code = |status: tonic::Status| {
        assert!(
            status.message().contains("AGENT_010"),
            "{}",
            status.message()
        );
    };

    refused_code(
        server
            .llm_servers(Request::new(v1::LlmServersRequest {
                op: Some(v1::llm_servers_request::Op::Add(
                    v1::llm_servers_request::Add {
                        name: "lab".to_owned(),
                        endpoint: "http://192.168.1.22:11434/".to_owned(),
                        models: vec!["m".to_owned()],
                        ..v1::llm_servers_request::Add::default()
                    },
                )),
            }))
            .await
            .unwrap_err(),
    );
    refused_code(
        server
            .llm_key(Request::new(v1::LlmKeyRequest {
                op: Some(v1::llm_key_request::Op::Set(v1::llm_key_request::Set {
                    key: "sk-test-0123456789abcdef".to_owned(),
                })),
                ..v1::LlmKeyRequest::default()
            }))
            .await
            .unwrap_err(),
    );
    refused_code(
        server
            .egress(Request::new(v1::EgressRequest {
                op: Some(v1::egress_request::Op::SetAuth(
                    v1::egress_request::SetAuth {
                        credentials: "user:pass".to_owned(),
                    },
                )),
            }))
            .await
            .unwrap_err(),
    );
    refused_code(
        server
            .agents(Request::new(v1::AgentsRequest {
                op: Some(v1::agents_request::Op::SetSource(
                    v1::agents_request::SetSource {
                        agent: "claude".to_owned(),
                        source: "host".to_owned(),
                    },
                )),
            }))
            .await
            .unwrap_err(),
    );
    assert_eq!(fixture.config_text(), "# untouched\n");
    // Lesen bleibt erlaubt.
    server
        .llm_servers(Request::new(v1::LlmServersRequest {
            op: Some(v1::llm_servers_request::Op::List(
                v1::llm_servers_request::List::default(),
            )),
        }))
        .await
        .unwrap();
}

/// `save_choice` mit dem Wert der Vorgabe ist trotzdem abgelehnt (Review
/// HUM-534): Ein gesperrter Schlüssel steht in keiner Projektwahl.
#[tokio::test]
async fn save_choice_refuses_a_locked_key_even_with_the_locked_value() {
    let fixture = Fixture::new("");
    let server = fixture.server(PolicyLocks::none().with("sandbox.mode", "direct", POLICY));
    let a = fixture.project("a").await;
    let refused = server
        .projects(Request::new(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    project_id: a.id.to_string(),
                    values: vec![v1::ChoiceValue {
                        key: "sandbox.mode".to_owned(),
                        value: "direct".to_owned(),
                    }],
                },
            )),
        }))
        .await
        .unwrap_err();
    assert!(
        refused.message().contains("CONFIG_024"),
        "{}",
        refused.message()
    );
    let stored = fixture.recorder.get_project(a.id).await.unwrap().unwrap();
    assert_eq!(stored.choice, None);
}

const SERVERS_CONFIG: &str = r#"[llm]
active = "home"

[llm.servers.home]
endpoint = "http://192.168.1.20:11434/"
"#;

/// Review HUM-534, Runde 2. `SetConfig(llm.endpoint)` für den aktiven Server
/// schreibt auch seinen Eintrag unter `llm.servers`; eine Sperre dort lehnt
/// es ab. Eine Sperre auf `sandbox.env` deckt `sandbox.env.NAME`.
/// Mutationsbeweis: Prüfte `set_locked` nur den einen Schlüssel, gingen beide
/// durch.
#[tokio::test]
async fn set_config_refuses_a_lock_on_a_touched_table_or_an_ancestor() {
    let fixture = Fixture::new(SERVERS_CONFIG);
    let locks =
        PolicyLocks::none()
            .with("llm.servers", "{}", POLICY)
            .with("sandbox.env", "{}", POLICY);
    let server = fixture.server(locks);
    for (key, value, expected) in [
        ("llm.endpoint", "http://192.168.1.21:11434/", "home"),
        ("sandbox.env.CURL_CA_BUNDLE", "/etc/humanitl/ca.crt", ""),
    ] {
        let refused = server
            .set_config(Request::new(v1::SetConfigRequest {
                key: key.to_owned(),
                value: value.to_owned(),
                expected_server: expected.to_owned(),
            }))
            .await
            .unwrap_err();
        assert!(
            refused.message().contains("AGENT_010"),
            "{key}: {}",
            refused.message()
        );
    }
    assert_eq!(fixture.config_text(), SERVERS_CONFIG);
}

/// Review HUM-534, Runde 2: `llm.active` hat keine Zeile, sperrt aber jede
/// Änderung der Server; die Zeilen der Server tragen die Sperre. Mutations-
/// beweis: Fragte `lock_of` nur die eigene Kennung, fehlte sie.
#[tokio::test]
async fn the_server_rows_carry_the_lock_of_a_hidden_server_key() {
    let fixture = Fixture::new(SERVERS_CONFIG);
    let server = fixture.server(PolicyLocks::none().with("llm.active", "home", POLICY));
    let snapshot = catalog(&server, "", "").await;
    for id in ["llm.servers", "llm.server"] {
        let lock = entry(&snapshot, id)
            .lock
            .as_ref()
            .unwrap_or_else(|| panic!("{id}"));
        assert_eq!(lock.locked_by, POLICY);
        assert!(lock.reason.contains("llm.active"), "{}", lock.reason);
    }
    assert!(entry(&snapshot, "hold.timeout_secs").lock.is_none());
}

/// Review HUM-534, Runde 2: `Agents.SetSource` meldet den Schlüssel, den es
/// geschrieben hat. Mutationsbeweis: Mit leerer Liste wird der Test rot.
#[tokio::test]
async fn set_source_names_its_key_in_config_changed() {
    let fixture = Fixture::new("");
    let server = fixture.server(PolicyLocks::none());
    let mut stream = server
        .subscribe(Request::new(v1::SubscribeRequest::default()))
        .await
        .unwrap()
        .into_inner();
    server
        .agents(Request::new(v1::AgentsRequest {
            op: Some(v1::agents_request::Op::SetSource(
                v1::agents_request::SetSource {
                    agent: "claude".to_owned(),
                    source: "depot".to_owned(),
                },
            )),
        }))
        .await
        .unwrap();
    let changed = next_config_change(&mut stream).await;
    assert_eq!(changed.keys, vec!["agents.claude.source".to_owned()]);
}
