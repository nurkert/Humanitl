//! Die Seiten der besonderen Zeilen mit einer eigenen Liste: Modellserver,
//! wer welchen Server nutzt, und das Agent-Depot (GitHub #7).
//!
//! Jede Seite zeigt eine Antwort des Daemons (`LlmServers`, `Agents`) und
//! schreibt über dieselben RPCs wie die App. Sie prüft nichts selbst: Ob ein
//! Name, eine Adresse oder eine Quelle gilt, sagt der Daemon, und seine
//! Ablehnung steht danach auf der Seite.

use crossterm::event::{KeyCode, KeyEvent};
use humanitl_ipc::v1;

use super::actions::{self, AGENTS, SERVER_USE, SERVERS};
use super::edit::{Confirm, Input, Opt, Overlay, Picker, Target};
use super::link::Request;
use super::model;
use super::{Note, SettingsScreen};
use crate::tui::screen::Flow;

/// Eine offene Seite und ihre markierte Zeile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// Die gespeicherten Modellserver.
    Servers {
        /// Die markierte Zeile.
        cursor: usize,
    },
    /// Profile und Projektordner mit dem Server, den sie wählen.
    Usage {
        /// Die markierte Zeile.
        cursor: usize,
    },
    /// Die Agenten des Depots.
    Agents {
        /// Die markierte Zeile.
        cursor: usize,
    },
}

impl Panel {
    /// Die Kennung der Zeile, an der Ergebnisse dieser Seite stehen.
    #[must_use]
    pub const fn row(self) -> &'static str {
        match self {
            Self::Servers { .. } => SERVERS,
            Self::Usage { .. } => SERVER_USE,
            Self::Agents { .. } => AGENTS,
        }
    }

    /// Die markierte Zeile.
    #[must_use]
    pub const fn cursor(self) -> usize {
        match self {
            Self::Servers { cursor } | Self::Usage { cursor } | Self::Agents { cursor } => cursor,
        }
    }

    /// Dieselbe Seite mit einer anderen Markierung.
    const fn with_cursor(self, cursor: usize) -> Self {
        match self {
            Self::Servers { .. } => Self::Servers { cursor },
            Self::Usage { .. } => Self::Usage { cursor },
            Self::Agents { .. } => Self::Agents { cursor },
        }
    }

    /// Die Tasten der Seite für die Fußzeile.
    #[must_use]
    pub const fn keys(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Servers { .. } => &[
                ("Enter", "make default"),
                ("a", "add"),
                ("n", "rename"),
                ("e", "address"),
                ("m", "models"),
                ("x", "remove"),
                ("k", "key"),
                ("p", "check"),
                ("Esc", "back"),
            ],
            Self::Usage { .. } => &[("Enter", "choose server"), ("Esc", "back")],
            Self::Agents { .. } => &[("Enter", "choose source"), ("Esc", "back")],
        }
    }
}

/// Wem `LlmServers.Assign` einen Server zuordnet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Assignee {
    /// Ein Profil der Sitzung.
    Profile {
        /// Sein Name.
        name: String,
        /// Der Server, den es wählt; leer ohne.
        server: String,
    },
    /// Der Projektordner aus `sandbox.work_dir`.
    Project {
        /// Der Ordner.
        dir: String,
        /// Der Server, den sein Projekt-Profil wählt; leer ohne.
        server: String,
    },
}

/// Die Zeilen einer Seite als Text: Name, Wert, Zusatz.
#[must_use]
pub fn lines(screen: &SettingsScreen, panel: Panel) -> Vec<[String; 3]> {
    match panel {
        Panel::Servers { .. } => servers(screen)
            .iter()
            .map(|server| {
                [
                    model::clean(&server.name),
                    model::clean(&server.endpoint),
                    format!(
                        "{}{}{}",
                        if server.active { "default · " } else { "" },
                        model::clean(&server.models.join(", ")),
                        if server.key_ref.is_empty() {
                            ""
                        } else {
                            " · key"
                        }
                    ),
                ]
            })
            .collect(),
        Panel::Usage { .. } => assignees(screen)
            .into_iter()
            .map(|assignee| match assignee {
                Assignee::Profile { name, server } => [
                    format!("profile {}", model::clean(&name)),
                    server_name(&server),
                    String::new(),
                ],
                Assignee::Project { dir, server } => [
                    format!("project {}", model::clean(&dir)),
                    server_name(&server),
                    String::new(),
                ],
            })
            .collect(),
        Panel::Agents { .. } => agents(screen)
            .iter()
            .map(|agent| {
                let mut extra = Vec::new();
                if !agent.version.is_empty() {
                    extra.push(format!("depot {}", model::clean(&agent.version)));
                }
                if agent.logged_in {
                    extra.push("logged in".to_owned());
                }
                if !agent.locked_by.is_empty() {
                    extra.push(format!("locked by {}", model::clean(&agent.locked_by)));
                }
                [
                    model::clean(&agent.display_name),
                    model::clean(&agent.source),
                    extra.join(" · "),
                ]
            })
            .collect(),
    }
}

/// Der Name eines Servers, wie die Seite ihn zeigt.
fn server_name(server: &str) -> String {
    if server.is_empty() {
        "default server".to_owned()
    } else {
        model::clean(server)
    }
}

/// Die gespeicherten Server.
fn servers(screen: &SettingsScreen) -> &[v1::LlmServer] {
    screen
        .specials
        .servers
        .as_ref()
        .map_or(&[], |list| list.servers.as_slice())
}

/// Die Agenten des Depots.
fn agents(screen: &SettingsScreen) -> &[v1::AgentDepot] {
    screen
        .specials
        .agents
        .as_ref()
        .map_or(&[], |status| status.agents.as_slice())
}

/// Profile und Projektordner aus `LlmServers`.
fn assignees(screen: &SettingsScreen) -> Vec<Assignee> {
    let Some(list) = screen.specials.servers.as_ref() else {
        return Vec::new();
    };
    let mut out: Vec<Assignee> = list
        .profiles
        .iter()
        .map(|profile| Assignee::Profile {
            name: profile.name.clone(),
            server: profile.server.clone(),
        })
        .collect();
    if let Some(project) = list.project.as_ref() {
        out.push(Assignee::Project {
            dir: project.dir.clone(),
            server: project.server.clone(),
        });
    }
    out
}

/// Nimmt eine Taste auf einer offenen Seite.
pub fn key(screen: &mut SettingsScreen, key: &KeyEvent) -> Flow {
    let Some(panel) = screen.panel else {
        return Flow::Continue;
    };
    let len = lines(screen, panel).len();
    let cursor = panel.cursor().min(len.saturating_sub(1));
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => screen.panel = None,
        // Nur Pfeile: Auf der Seite der Server ist `k` der Schlüssel.
        KeyCode::Up => {
            screen.panel = Some(panel.with_cursor(cursor.saturating_sub(1)));
        }
        KeyCode::Down => {
            screen.panel = Some(panel.with_cursor((cursor + 1).min(len.saturating_sub(1))));
        }
        _ => match panel {
            Panel::Servers { .. } => server_key(screen, key, cursor),
            Panel::Usage { .. } => usage_key(screen, key, cursor),
            Panel::Agents { .. } => agent_key(screen, key, cursor),
        },
    }
    Flow::Continue
}

/// Eine Taste auf der Seite der Server.
fn server_key(screen: &mut SettingsScreen, key: &KeyEvent, cursor: usize) {
    use v1::llm_servers_request::{Activate, Op};

    let selected = servers(screen).get(cursor).cloned();
    match (key.code, selected) {
        (KeyCode::Char('a'), _) => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                "Name of the new server".to_owned(),
                String::new(),
                Target::AddName,
            )));
        }
        (KeyCode::Enter, Some(server)) if !server.active => {
            servers_call(screen, Op::Activate(Activate { name: server.name }));
        }
        (KeyCode::Char('n'), Some(server)) => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                format!("New name for {}", model::clean(&server.name)),
                server.name.clone(),
                Target::Rename(server.name),
            )));
        }
        (KeyCode::Char('e'), Some(server)) => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                format!(
                    "Address of {} (http://HOST:PORT)",
                    model::clean(&server.name)
                ),
                server.endpoint.clone(),
                Target::EditEndpoint(server.name),
            )));
        }
        (KeyCode::Char('m'), Some(server)) => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                format!(
                    "Models of {} (comma separated, default first)",
                    model::clean(&server.name)
                ),
                server.models.join(", "),
                Target::EditModels(server.name),
            )));
        }
        (KeyCode::Char('x') | KeyCode::Delete, Some(server)) => remove_server(screen, &server),
        (KeyCode::Char('k'), Some(server)) => {
            screen.overlay = Some(Overlay::Input(Input::secret(
                format!("Access key for {} (hidden)", model::clean(&server.name)),
                Target::LlmKey(server.name),
            )));
        }
        (KeyCode::Char('p'), Some(server)) => {
            screen.notes.insert(
                SERVERS.to_owned(),
                Note::Info(format!("Checking {}…", model::clean(&server.name))),
            );
            screen.send(
                Some(SERVERS),
                Request::ProbeLlm(v1::ProbeLlmRequest {
                    endpoint: server.endpoint,
                    timeout_ms: 0,
                    api_key: String::new(),
                    use_stored_key: true,
                    server: server.name,
                }),
            );
        }
        _ => {}
    }
}

/// Entfernen: für den aktiven Server erst der Nachfolger, dann die
/// Rückfrage; für jeden anderen gleich die Rückfrage.
fn remove_server(screen: &mut SettingsScreen, server: &v1::LlmServer) {
    let others: Vec<Opt> = servers(screen)
        .iter()
        .filter(|other| other.name != server.name)
        .map(|other| Opt::new(other.name.clone(), model::clean(&other.name)))
        .collect();
    let target = Target::RemoveServer {
        name: server.name.clone(),
        replacement: String::new(),
    };
    if server.active && !others.is_empty() {
        screen.overlay = Some(Overlay::Picker(Picker::new(
            format!("Default after removing {}", model::clean(&server.name)),
            others,
            "",
            target,
        )));
    } else {
        screen.overlay = Some(Overlay::Confirm(Confirm {
            question: remove_question(&server.name),
            target,
        }));
    }
}

/// Die Rückfrage vor dem Entfernen, ausformuliert.
fn remove_question(name: &str) -> String {
    format!(
        "Remove the server {} and its access key, unless another server uses the same key?",
        model::clean(name)
    )
}

/// Eine Taste auf der Seite der Zuordnungen.
fn usage_key(screen: &mut SettingsScreen, key: &KeyEvent, cursor: usize) {
    if key.code != KeyCode::Enter {
        return;
    }
    let Some(assignee) = assignees(screen).into_iter().nth(cursor) else {
        return;
    };
    let (label, current, target) = match assignee {
        Assignee::Profile { name, server } => (
            format!("Server for the profile {}", model::clean(&name)),
            server,
            Target::AssignProfile(name),
        ),
        Assignee::Project { dir, server } => (
            format!("Server for the project {}", model::clean(&dir)),
            server,
            Target::AssignProject(dir),
        ),
    };
    let options = actions::server_options(screen);
    screen.overlay = Some(Overlay::Picker(Picker::new(
        label, options, &current, target,
    )));
}

/// Eine Taste auf der Seite der Agenten.
fn agent_key(screen: &mut SettingsScreen, key: &KeyEvent, cursor: usize) {
    if key.code != KeyCode::Enter {
        return;
    }
    let Some(agent) = agents(screen).get(cursor).cloned() else {
        return;
    };
    if !agent.locked_by.is_empty() {
        screen.notes.insert(
            AGENTS.to_owned(),
            Note::Info(format!(
                "The source of {} is locked by {}, read only.",
                model::clean(&agent.display_name),
                model::clean(&agent.locked_by)
            )),
        );
        return;
    }
    let actions = agent.actions.unwrap_or_default();
    let mut host = Opt::new("host", "This machine");
    host.enabled = actions.source_host || agent.source == "host";
    let mut depot = Opt::new("depot", "Depot");
    depot.enabled = actions.source_depot || agent.source == "depot";
    if !agent.depot_supported {
        "no depot for this agent".clone_into(&mut depot.reason);
    }
    screen.overlay = Some(Overlay::Picker(Picker::new(
        format!("Source of {}", model::clean(&agent.display_name)),
        vec![host, depot],
        &agent.source,
        Target::AgentSource(agent.agent),
    )));
}

/// Schickt einen Auftrag an `LlmServers`.
fn servers_call(screen: &SettingsScreen, op: v1::llm_servers_request::Op) {
    screen.send(
        Some(SERVERS),
        Request::LlmServers(v1::LlmServersRequest { op: Some(op) }),
    );
}

/// Die Modelle aus einer Eingabe mit Kommas, das voreingestellte zuerst.
fn models_of(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Ein Schritt beim Anlegen eines Servers: Name, Adresse, Modelle, dann
/// `LlmServers.Add`. Ein leerer Name bricht ab.
fn add_step(screen: &mut SettingsScreen, target: Target, text: String) {
    use v1::llm_servers_request::{Add, Op};

    match target {
        Target::AddName if !text.is_empty() => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                format!("Address of {} (http://HOST:PORT)", model::clean(&text)),
                String::new(),
                Target::AddEndpoint { name: text },
            )));
        }
        Target::AddEndpoint { name } => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                format!(
                    "Models of {} (comma separated, default first)",
                    model::clean(&name)
                ),
                String::new(),
                Target::AddModels {
                    name,
                    endpoint: text,
                },
            )));
        }
        Target::AddModels { name, endpoint } => servers_call(
            screen,
            Op::Add(Add {
                name,
                endpoint,
                models: models_of(&text),
                activate: false,
            }),
        ),
        _ => {}
    }
}

/// Führt aus, was eine Eingabe oder Wahl auf einer Seite abgeschlossen hat.
pub fn commit(screen: &mut SettingsScreen, target: Target, text: String) {
    use v1::llm_servers_request::{Assign, Edit, Op, Remove, Rename, assign, edit};

    match target {
        target @ (Target::AddName | Target::AddEndpoint { .. } | Target::AddModels { .. }) => {
            add_step(screen, target, text);
        }
        Target::EditEndpoint(name) => servers_call(
            screen,
            Op::Edit(Edit {
                name,
                endpoint: Some(text),
                models: None,
            }),
        ),
        Target::EditModels(name) => servers_call(
            screen,
            Op::Edit(Edit {
                name,
                endpoint: None,
                models: Some(edit::Models {
                    names: models_of(&text),
                }),
            }),
        ),
        Target::Rename(name) => servers_call(
            screen,
            Op::Rename(Rename {
                name,
                new_name: text,
            }),
        ),
        Target::RemoveServer { name, replacement }
            if replacement.is_empty() && !text.is_empty() =>
        {
            screen.overlay = Some(Overlay::Confirm(Confirm {
                question: remove_question(&name),
                target: Target::RemoveServer {
                    name,
                    replacement: text,
                },
            }));
        }
        Target::RemoveServer { name, replacement } => servers_call(
            screen,
            Op::Remove(Remove {
                name,
                delete_key: true,
                replacement,
            }),
        ),
        Target::AssignProfile(profile) => servers_call(
            screen,
            Op::Assign(Assign {
                server: text,
                target: Some(assign::Target::Profile(profile)),
            }),
        ),
        Target::AssignProject(dir) => servers_call(
            screen,
            Op::Assign(Assign {
                server: text,
                target: Some(assign::Target::ProjectDir(dir)),
            }),
        ),
        Target::AgentSource(agent) => screen.send(
            Some(AGENTS),
            Request::Agents(v1::AgentsRequest {
                op: Some(v1::agents_request::Op::SetSource(
                    v1::agents_request::SetSource {
                        agent,
                        source: text,
                    },
                )),
            }),
        ),
        _ => {}
    }
}
