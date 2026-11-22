//! Tests des Bildschirms der Einstellungen gegen einen Fake-Daemon
//! (GitHub #7).
//!
//! Der Fake beantwortet die Nachrichten der Wire-Form, die der Bildschirm in
//! seine Leitung legt, und zeichnet jede Anfrage auf. Gezeichnet wird in ein
//! `TestBackend` von ratatui; ein echtes Terminal und ein echter Daemon
//! kommen nie vor. Mit `HUMANITL_TUI_SHOTS=<Ordner>` legt jeder
//! Schnappschuss seine Zeilen als Text dort ab.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::{Value, json};

use super::link::{self, Answer, Call, Far, Reply, Request};
use super::{Focus, SettingsScreen};
use crate::tui::frame::Theme;
use crate::tui::screen::Screen;

/// Eine Projektwahl im Fake.
struct FakeProject {
    /// Die Kennung im Register.
    id: String,
    /// Die gewählten Werte.
    chosen: Vec<(String, String)>,
}

/// Ein Daemon, der die Anfragen des Bildschirms beantwortet und aufzeichnet.
struct Fake {
    catalog: v1::SettingsCatalog,
    values: Value,
    calls: Vec<Request>,
    refuse: BTreeMap<String, Diagnostic>,
    projects: BTreeMap<String, FakeProject>,
    servers: Vec<v1::LlmServer>,
    keys: BTreeMap<String, bool>,
    egress_auth: bool,
}

fn category(id: &str, order: u32, title: &str) -> v1::SettingsCategory {
    v1::SettingsCategory {
        id: id.to_owned(),
        order,
        title: title.to_owned(),
    }
}

fn entry(id: &str, category: &str, order: u32, title: &str, control: &str) -> v1::SettingsEntry {
    v1::SettingsEntry {
        id: id.to_owned(),
        category: category.to_owned(),
        order,
        title: title.to_owned(),
        control: control.to_owned(),
        origin: "default".to_owned(),
        tier: "basic".to_owned(),
        ..v1::SettingsEntry::default()
    }
}

fn choices(pairs: &[(&str, &str)]) -> Vec<v1::SettingsChoice> {
    pairs
        .iter()
        .map(|(value, title)| v1::SettingsChoice {
            value: (*value).to_owned(),
            title: (*title).to_owned(),
        })
        .collect()
}

/// Ein Katalog mit Kategorien außer der Reihe, einer gesperrten Zeile und
/// Zeilen, die nur die App zeigt.
fn fake_catalog() -> v1::SettingsCatalog {
    let mut adapter = entry("agent.adapter", "agent", 0, "Coding agent", "choice");
    adapter.help = "The agent a new session starts in the sandbox.".to_owned();
    adapter.choices = choices(&[("opencode", "OpenCode"), ("claude", "Claude Code")]);
    adapter.next_session = true;
    let mut mode = entry(
        "sandbox.mode",
        "sandbox",
        1,
        "Where the agent writes",
        "choice",
    );
    mode.choices = choices(&[
        ("auto", "Automatic"),
        ("direct", "Direct"),
        ("disposable", "Disposable"),
    ]);
    mode.in_project_choice = true;
    mode.origin = "global".to_owned();
    let mut folder = entry("sandbox.work_dir", "sandbox", 0, "Project folder", "path");
    folder.help = "Mounted as /work. Without one, the folder Humanitl starts in.".to_owned();
    folder.in_project_choice = true;
    let mut trust = entry(
        "tls.trust_system_store",
        "network",
        0,
        "System certificates",
        "switch",
    );
    trust.help = "Trust the certificates of this system, for example a company CA".to_owned();
    let mut language = entry("ui.language", "interface", 0, "Language", "choice");
    language.choices = choices(&[("en", "English"), ("de", "Deutsch")]);
    language.origin = "policy".to_owned();
    language.lock = Some(v1::SettingLock {
        locked_by: "/etc/humanitl/policy.toml".to_owned(),
        read_only: true,
        reason: "Your company sets the language.".to_owned(),
    });
    let mut scale = entry("ui.scale", "interface", 1, "Scale", "special");
    scale.client = "app".to_owned();
    let mut shortcuts = entry("shortcuts", "shortcuts", 0, "Keyboard shortcuts", "special");
    shortcuts.client = "app".to_owned();
    let mut proxy = entry("egress.proxy", "egress", 0, "Address", "text");
    proxy.keywords = vec!["mitmproxy".to_owned()];
    let auth = entry(
        "egress.proxy_auth_ref",
        "egress",
        1,
        "Credentials",
        "secret_ref",
    );
    let servers = entry("llm.servers", "models", 0, "Model servers", "special");
    let key = entry("llm.api_key_ref", "models", 1, "Access key", "secret_ref");
    v1::SettingsCatalog {
        language: "en".to_owned(),
        categories: vec![
            category("network", 2, "Network"),
            category("agent", 0, "Agent"),
            category("models", 5, "Models"),
            category("shortcuts", 6, "Keyboard shortcuts"),
            category("sandbox", 1, "Sandbox"),
            category("egress", 4, "Decision proxy"),
            category("interface", 3, "Interface"),
        ],
        entries: vec![
            adapter, mode, folder, trust, language, scale, shortcuts, proxy, auth, servers, key,
        ],
    }
}

impl Fake {
    fn new() -> Self {
        Self::with_catalog(fake_catalog())
    }

    fn with_catalog(catalog: v1::SettingsCatalog) -> Self {
        Self {
            catalog,
            values: json!({
                "agent": {"adapter": "claude"},
                "sandbox": {"mode": "disposable", "work_dir": "/home/u/p"},
                "tls": {"trust_system_store": true},
                "ui": {"language": "en"},
                "egress": {"proxy": "http://127.0.0.1:8080"},
            }),
            calls: Vec::new(),
            refuse: BTreeMap::new(),
            projects: BTreeMap::new(),
            servers: Vec::new(),
            keys: BTreeMap::new(),
            egress_auth: false,
        }
    }

    /// Setzt einen Wert, wie `SetConfig` ihn schriebe.
    fn set(&mut self, key: &str, value: &str) {
        let parsed = match value {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            other => Value::String(other.to_owned()),
        };
        let mut place = &mut self.values;
        for part in key.split('.') {
            place = place
                .as_object_mut()
                .expect("an object")
                .entry(part.to_owned())
                .or_insert_with(|| json!({}));
        }
        *place = parsed;
    }

    /// Die Antwort von `GetConfig`, für einen Ordner mit seiner Projektwahl.
    fn snapshot(&self, project: &str) -> v1::ConfigSnapshot {
        let mut catalog = self.catalog.clone();
        let mut values = self.values.clone();
        if let Some(fake) = self.projects.get(project) {
            for (key, value) in &fake.chosen {
                let pointer = format!("/{}", key.replace('.', "/"));
                if let Some(place) = values.pointer_mut(&pointer) {
                    *place = Value::String(value.clone());
                }
                for entry in &mut catalog.entries {
                    if &entry.id == key {
                        entry.origin = "project-choice".to_owned();
                    }
                }
            }
        }
        v1::ConfigSnapshot {
            values_json: values.to_string(),
            config_path: "/home/u/.config/humanitl/config.toml".to_owned(),
            catalog: Some(catalog),
            ..v1::ConfigSnapshot::default()
        }
    }

    fn choice_fields(chosen: &[(String, String)]) -> Vec<v1::ProjectChoiceField> {
        chosen
            .iter()
            .map(|(key, chosen)| v1::ProjectChoiceField {
                key: key.clone(),
                chosen: chosen.clone(),
                ..v1::ProjectChoiceField::default()
            })
            .collect()
    }

    fn projects(&mut self, asked: &v1::ProjectsRequest) -> Answer {
        match asked.op.as_ref() {
            Some(v1::projects_request::Op::Proposal(proposal)) => {
                let project_id = self
                    .projects
                    .get(&proposal.path)
                    .map(|fake| fake.id.clone())
                    .unwrap_or_default();
                Answer::Project(v1::ProjectsResponse {
                    proposal: Some(v1::StartProposal {
                        path: proposal.path.clone(),
                        project_id,
                        ..v1::StartProposal::default()
                    }),
                    ..v1::ProjectsResponse::default()
                })
            }
            Some(v1::projects_request::Op::Get(get)) => {
                let (path, fake) = self
                    .projects
                    .iter()
                    .find(|(_, fake)| fake.id == get.project_id)
                    .expect("a known project by its id");
                Answer::Project(v1::ProjectsResponse {
                    project: Some(v1::ProjectEntry {
                        project_id: fake.id.clone(),
                        path: path.clone(),
                        ..v1::ProjectEntry::default()
                    }),
                    choice: Self::choice_fields(&fake.chosen),
                    ..v1::ProjectsResponse::default()
                })
            }
            Some(v1::projects_request::Op::SaveChoice(save)) => {
                let chosen: Vec<(String, String)> = save
                    .values
                    .iter()
                    .map(|value| (value.key.clone(), value.value.clone()))
                    .collect();
                if save.project_id.is_empty() {
                    // Wie der Daemon seit HUM-538: ein unbekannter Ordner
                    // wird über seinen Pfad eingetragen.
                    let id = format!("id-{}", self.projects.len());
                    self.projects.insert(
                        save.path.clone(),
                        FakeProject {
                            id: id.clone(),
                            chosen: chosen.clone(),
                        },
                    );
                    return Answer::Saved(v1::ProjectsResponse {
                        project: Some(v1::ProjectEntry {
                            project_id: id,
                            path: save.path.clone(),
                            ..v1::ProjectEntry::default()
                        }),
                        choice: Self::choice_fields(&chosen),
                        ..v1::ProjectsResponse::default()
                    });
                }
                let fake = self
                    .projects
                    .values_mut()
                    .find(|fake| fake.id == save.project_id)
                    .expect("a project by its id");
                fake.chosen = chosen;
                Answer::Saved(v1::ProjectsResponse {
                    choice: Self::choice_fields(&fake.chosen),
                    ..v1::ProjectsResponse::default()
                })
            }
            _ => panic!("the settings screen asks Projects only for proposal, get and save_choice"),
        }
    }

    fn servers(&mut self, asked: &v1::LlmServersRequest) -> v1::LlmServerList {
        use v1::llm_servers_request::Op;
        match asked.op.as_ref() {
            Some(Op::Add(add)) => self.servers.push(v1::LlmServer {
                name: add.name.clone(),
                endpoint: add.endpoint.clone(),
                models: add.models.clone(),
                active: self.servers.is_empty(),
                ..v1::LlmServer::default()
            }),
            Some(Op::Edit(edit)) => {
                let server = self
                    .servers
                    .iter_mut()
                    .find(|server| server.name == edit.name)
                    .expect("a saved server");
                if let Some(endpoint) = edit.endpoint.as_ref() {
                    server.endpoint.clone_from(endpoint);
                }
                if let Some(models) = edit.models.as_ref() {
                    server.models.clone_from(&models.names);
                }
            }
            Some(Op::Activate(activate)) => {
                for server in &mut self.servers {
                    server.active = server.name == activate.name;
                }
            }
            _ => {}
        }
        v1::LlmServerList {
            active: self
                .servers
                .iter()
                .find(|server| server.active)
                .map(|server| server.name.clone())
                .unwrap_or_default(),
            servers: self.servers.clone(),
            ..v1::LlmServerList::default()
        }
    }

    fn llm_key(&mut self, asked: &v1::LlmKeyRequest) -> v1::LlmKeyStatus {
        use v1::llm_key_request::Op;
        match asked.op.as_ref() {
            Some(Op::Set(_)) => {
                self.keys.insert(asked.server.clone(), true);
            }
            Some(Op::Remove(_)) => {
                self.keys.insert(asked.server.clone(), false);
            }
            _ => {}
        }
        v1::LlmKeyStatus {
            set: self.keys.get(&asked.server).copied().unwrap_or(false),
            key_ref: "secret-service:humanitl/llm".to_owned(),
            ..v1::LlmKeyStatus::default()
        }
    }

    fn egress(&mut self, asked: &v1::EgressRequest) -> v1::EgressStatus {
        use v1::egress_request::Op;
        let mut diagnostics = Vec::new();
        match asked.op.as_ref() {
            Some(Op::SetAuth(_)) => self.egress_auth = true,
            Some(Op::RemoveAuth(_)) => self.egress_auth = false,
            Some(Op::CheckConnection(_)) => diagnostics.push(v1::Diagnostic {
                code: "PROXY_014".to_owned(),
                why: "the proxy at 127.0.0.1:8080 refused the tunnel".to_owned(),
                fix: Some(v1::FixAction {
                    action: Some(v1::fix_action::Action::CopyCommand(
                        "humanitl egress check".to_owned(),
                    )),
                }),
                ..v1::Diagnostic::default()
            }),
            _ => {}
        }
        v1::EgressStatus {
            auth_set: self.egress_auth,
            diagnostics,
            ..v1::EgressStatus::default()
        }
    }

    /// Beantwortet eine Anfrage und zeichnet sie auf.
    fn answer(&mut self, call: Call) -> Reply {
        let result = match &call.request {
            Request::GetConfig(asked) => Ok(Answer::Config(self.snapshot(&asked.project_id))),
            Request::SetConfig(asked) => {
                if let Some(refusal) = self.refuse.get(&asked.key) {
                    Err(refusal.clone())
                } else {
                    self.set(&asked.key, &asked.value);
                    Ok(Answer::Written)
                }
            }
            Request::Projects(asked) => Ok(self.projects(asked)),
            Request::LlmServers(asked) => Ok(Answer::Servers(self.servers(asked))),
            Request::LlmKey(asked) => Ok(Answer::Key(self.llm_key(asked))),
            Request::Egress(asked) => Ok(Answer::Egress(self.egress(asked))),
            Request::ProbeLlm(_) => Ok(Answer::Probe(v1::ProbeLlmResponse::default())),
            Request::SandboxBackends(_) => Ok(Answer::Backends(v1::SandboxBackendList::default())),
            Request::Agents(_) => Ok(Answer::Agents(v1::AgentsStatus::default())),
            Request::GetInfo => Ok(Answer::Info(v1::Info {
                daemon_version: "0.5.0".to_owned(),
                ..v1::Info::default()
            })),
            Request::Update(_) => Ok(Answer::Update(v1::UpdateStatus::default())),
        };
        self.calls.push(call.request);
        Reply {
            row: call.row,
            result,
        }
    }

    /// Vergisst die bisherigen Anfragen.
    fn forget(&mut self) {
        self.calls.clear();
    }

    /// Jede `SetConfig`, in Reihenfolge.
    fn set_configs(&self) -> Vec<&v1::SetConfigRequest> {
        self.calls
            .iter()
            .filter_map(|call| match call {
                Request::SetConfig(asked) => Some(asked),
                _ => None,
            })
            .collect()
    }

    /// Jede Anfrage, die etwas schreibt oder auslöst, ohne das Laden.
    fn writes(&self) -> usize {
        self.calls
            .iter()
            .filter(|call| match call {
                Request::GetConfig(_)
                | Request::GetInfo
                | Request::Update(_)
                | Request::SandboxBackends(_)
                | Request::ProbeLlm(_) => false,
                Request::Projects(asked) => {
                    !matches!(asked.op, Some(v1::projects_request::Op::Get(_)))
                }
                Request::LlmServers(asked) => {
                    !matches!(asked.op, Some(v1::llm_servers_request::Op::List(_)))
                }
                Request::LlmKey(asked) => {
                    !matches!(asked.op, Some(v1::llm_key_request::Op::Status(_)))
                }
                Request::Egress(asked) => {
                    !matches!(asked.op, Some(v1::egress_request::Op::Status(_)))
                }
                Request::Agents(asked) => {
                    !matches!(asked.op, Some(v1::agents_request::Op::Status(_)))
                }
                Request::SetConfig(_) => true,
            })
            .count()
    }
}

/// Ein Bildschirm am Fake, schon geladen.
fn open(fake: &mut Fake, project: Option<&str>) -> (SettingsScreen, Far) {
    let (link, mut far) = link::pair();
    let mut screen = SettingsScreen::new(link, project.map(str::to_owned));
    pump(&mut screen, &mut far, fake);
    (screen, far)
}

/// Lässt Bildschirm und Fake reden, bis keiner mehr etwas zu sagen hat.
fn pump(screen: &mut SettingsScreen, far: &mut Far, fake: &mut Fake) {
    loop {
        let mut busy = false;
        while let Ok(call) = far.calls.try_recv() {
            busy = true;
            far.replies
                .send(fake.answer(call))
                .expect("the screen listens");
        }
        if screen.changed() {
            busy = true;
        }
        if !busy {
            return;
        }
    }
}

fn press(screen: &mut SettingsScreen, code: KeyCode) {
    screen.on_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}

fn typing(screen: &mut SettingsScreen, text: &str) {
    for c in text.chars() {
        press(screen, KeyCode::Char(c));
    }
}

/// Zeichnet den Bildschirm ohne Farbe und gibt die Zeilen als Text zurück;
/// mit `HUMANITL_TUI_SHOTS` auch als Datei `<name>.txt`.
fn shot(screen: &mut SettingsScreen, name: &str, size: (u16, u16)) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).expect("a terminal");
    terminal
        .draw(|frame| {
            screen.draw(frame, Theme::from_no_color(Some(std::ffi::OsStr::new("1"))));
        })
        .expect("drawn");
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    let lines: Vec<String> = (area.top()..area.bottom())
        .map(|y| {
            // Ein Zeichen doppelter Breite belegt zwei Zellen; die zweite
            // gehört zu ihm und wird übersprungen.
            let mut line = String::new();
            let mut x = area.left();
            while x < area.right() {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                let width = u16::try_from(ratatui::text::Span::raw(symbol).width()).unwrap_or(1);
                x += width.max(1);
            }
            line.trim_end().to_owned()
        })
        .collect();
    if let Some(dir) = std::env::var_os("HUMANITL_TUI_SHOTS") {
        let path = std::path::Path::new(&dir).join(format!("{name}.txt"));
        std::fs::write(path, lines.join("\n") + "\n").expect("the shot is written");
    }
    lines
}

/// Die Zeile im Bild, die `needle` enthält.
fn line_with<'a>(lines: &'a [String], needle: &str) -> &'a str {
    lines
        .iter()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in\n{}", lines.join("\n")))
}

/// Geht in der linken Spalte zur Kategorie mit diesem Namen und in die
/// Zeilen.
fn go_to_category(screen: &mut SettingsScreen, title: &str) {
    press(screen, KeyCode::Left);
    for _ in 0..20 {
        press(screen, KeyCode::Up);
    }
    for _ in 0..20 {
        let current = screen.categories()[screen.category].title.clone();
        if current == title {
            press(screen, KeyCode::Enter);
            assert_eq!(screen.focus, Focus::Rows);
            return;
        }
        press(screen, KeyCode::Down);
    }
    panic!("no category {title}");
}

/// Geht in den Zeilen zur Zeile mit dieser Kennung.
fn go_to_row(screen: &mut SettingsScreen, id: &str) {
    for _ in 0..20 {
        press(screen, KeyCode::Up);
    }
    for _ in 0..20 {
        if screen.selected().is_some_and(|entry| entry.id == id) {
            return;
        }
        press(screen, KeyCode::Down);
    }
    panic!("no row {id}");
}

/// Schnappschuss: Kategorien in der Reihenfolge des Katalogs, nicht der
/// Liste; die Kategorie, die nur Zeilen der App hat, fehlt; jede Zeile mit
/// Herkunft und Wert.
#[test]
fn the_list_shows_categories_in_catalog_order_with_origin_and_value() {
    let mut fake = Fake::new();
    let (mut screen, _far) = open(&mut fake, None);
    go_to_category(&mut screen, "Sandbox");
    let lines = shot(&mut screen, "list", (80, 16));
    assert_eq!(
        lines,
        [
            " humanitl · Settings                                                     global",
            " Agent           │› Project folder         default         /home/u/p",
            " Sandbox         │  Where the agent writes global          Disposable",
            " Network         │",
            " Interface       │",
            " Decision proxy  │",
            " Models          │",
            "                 │",
            "                 │──────────────────────────────────────────────────────────────",
            "                 │Mounted as /work. Without one, the folder Humanitl starts in.",
            "                 │",
            "                 │",
            "                 │",
            "                 │",
            "                 │",
            " ↑↓ move  ←→ pane  Enter change  / search  q quit",
        ]
    );
}

/// Schnappschuss: Die Suche findet über alle Kategorien, auch über
/// Stichwörter, die nie gezeigt werden.
#[test]
fn search_finds_rows_across_categories_and_keywords() {
    let mut fake = Fake::new();
    let (mut screen, _far) = open(&mut fake, None);
    typing(&mut screen, "/mitm");
    let lines = shot(&mut screen, "search", (80, 12));
    assert!(line_with(&lines, "Address").contains("http://127.0.0.1:8080"));
    assert!(lines.iter().any(|line| line.starts_with("/mitm")));
    assert!(
        !lines.iter().any(|line| line.contains("Coding agent")),
        "{lines:#?}"
    );
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Esc);
    let lines = shot(&mut screen, "search-cleared", (80, 12));
    assert!(!lines.iter().any(|line| line.starts_with("/mitm")));
}

/// Eine gesperrte Zeile ist nur lesbar: Herkunft `policy`, die Vorgabe und
/// ihr Grund stehen da, und weder `Enter` noch Leertaste noch `x` schicken
/// etwas.
#[test]
fn a_locked_row_is_read_only_and_names_the_policy() {
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Interface");
    fake.forget();
    for code in [KeyCode::Enter, KeyCode::Char(' '), KeyCode::Char('x')] {
        press(&mut screen, code);
        assert!(screen.overlay.is_none(), "no control opens on a locked row");
    }
    pump(&mut screen, &mut far, &mut fake);
    assert_eq!(fake.writes(), 0, "a locked row sends nothing");
    let lines = shot(&mut screen, "locked", (80, 14));
    assert!(line_with(&lines, "Language").contains("policy, locked"));
    assert!(
        lines
            .join(" ")
            .contains("Locked by /etc/humanitl/policy.toml"),
        "{lines:#?}"
    );
    assert!(line_with(&lines, "sets the language.").contains('│'));
    assert!(
        !lines.iter().any(|line| line.contains("Scale")),
        "client = app stays out"
    );
}

/// Kategorie wählen, Schalter umlegen: genau ein `SetConfig` mit Schlüssel
/// und Wert.
#[test]
fn choosing_a_category_and_flipping_a_switch_sends_one_set_config() {
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, None);
    fake.forget();
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Char(' '));
    pump(&mut screen, &mut far, &mut fake);
    let sets = fake.set_configs();
    assert_eq!(sets.len(), 1, "exactly one SetConfig");
    assert_eq!(sets[0].key, "tls.trust_system_store");
    assert_eq!(sets[0].value, "false");
    assert_eq!(fake.writes(), 1);
    let lines = shot(&mut screen, "switch-flipped", (80, 12));
    assert!(line_with(&lines, "System certificates").ends_with("off"));
}

/// Parität mit der App: Jede Kategorie und jeder Eintrag des echten
/// Katalogs steht im Bildschirm, ausgenommen genau die Einträge mit
/// `client = app` und Kategorien, die nur solche haben.
#[test]
fn every_category_and_entry_of_the_catalog_appears() {
    let snapshot = v1::ConfigSnapshot {
        values_json: "{}".to_owned(),
        ..v1::ConfigSnapshot::default()
    };
    let catalog = humanitl_ipc::config_rpc::catalog_for(
        &snapshot,
        "en",
        &humanitl_ipc::config_rpc::CatalogScope {
            offers_disposable: true,
            ..humanitl_ipc::config_rpc::CatalogScope::default()
        },
    );
    let expected: Vec<String> = catalog
        .entries
        .iter()
        .filter(|entry| entry.client != "app")
        .map(|entry| entry.id.clone())
        .collect();
    assert!(
        catalog.entries.iter().any(|entry| entry.client == "app"),
        "the catalog marks app-only rows"
    );
    let mut fake = Fake::with_catalog(catalog.clone());
    let (mut screen, _far) = open(&mut fake, None);
    let mut seen = Vec::new();
    let categories: Vec<String> = screen
        .categories()
        .iter()
        .map(|category| category.title.clone())
        .collect();
    for (index, title) in categories.iter().enumerate() {
        screen.category = index;
        screen.row = 0;
        let lines = shot(&mut screen, &format!("parity-{index}"), (120, 40));
        assert!(line_with(&lines, title).contains(title.as_str()));
        for entry in screen.rows() {
            assert!(
                lines.iter().any(|line| line.contains(&entry.title)),
                "{} is not drawn",
                entry.id
            );
            seen.push(entry.id.clone());
        }
    }
    let mut seen_sorted = seen.clone();
    seen_sorted.sort();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    assert_eq!(seen_sorted, expected_sorted);
    for category in &catalog.categories {
        let has_row = catalog
            .entries
            .iter()
            .any(|entry| entry.category == category.id && entry.client != "app");
        assert_eq!(
            categories.contains(&category.title),
            has_row,
            "{}",
            category.id
        );
    }
}

/// Eine Ablehnung des Daemons steht an der Zeile, mit `why` und `fix`.
#[test]
fn a_refusal_of_the_daemon_appears_at_the_row() {
    let mut fake = Fake::new();
    fake.refuse.insert(
        "egress.proxy".to_owned(),
        Diagnostic::builder(codes::CONFIG_003, Severity::Error)
            .why("egress.proxy takes http://HOST:PORT, not ftp://x".to_owned())
            .fix(FixAction::CopyCommand(
                "humanitl config set egress.proxy http://127.0.0.1:8080".to_owned(),
            ))
            .build(),
    );
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Decision proxy");
    go_to_row(&mut screen, "egress.proxy");
    press(&mut screen, KeyCode::Enter);
    let lines = shot(&mut screen, "edit", (80, 16));
    assert!(line_with(&lines, "> http://127.0.0.1:8080").contains('>'));
    press(&mut screen, KeyCode::Char('u'));
    screen.on_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    typing(&mut screen, "ftp://x");
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    assert_eq!(
        fake.set_configs().last().map(|set| set.value.as_str()),
        Some("ftp://x")
    );
    let lines = shot(&mut screen, "refused", (80, 16));
    let text = lines.join(" ");
    assert!(line_with(&lines, "Address").contains("refused"));
    assert!(text.contains("CONFIG_003"), "{lines:#?}");
    assert!(text.contains("why: egress.proxy takes http://HOST:PORT"));
    assert!(text.contains("fix: humanitl config set egress.proxy http://127.0.0.1:8080"));
}

/// Projektebene mit zwei Projekten: `--project A` zeigt den Wert von A mit
/// Herkunft `project-choice`, B den seinen; `sandbox.mode` umschalten
/// schickt `save_choice` für A und kein `SetConfig`. Eine Zeile außerhalb
/// der Projektwahl ist dort nur lesbar.
#[test]
fn the_project_layer_reads_and_writes_the_project_choice() {
    let mut fake = Fake::new();
    fake.projects.insert(
        "/p/a".to_owned(),
        FakeProject {
            id: "id-a".to_owned(),
            chosen: vec![("sandbox.mode".to_owned(), "disposable".to_owned())],
        },
    );
    fake.projects.insert(
        "/p/b".to_owned(),
        FakeProject {
            id: "id-b".to_owned(),
            chosen: vec![("sandbox.mode".to_owned(), "direct".to_owned())],
        },
    );
    let (mut b, _far_b) = open(&mut fake, Some("/p/b"));
    go_to_category(&mut b, "Sandbox");
    let lines = shot(&mut b, "project-b", (80, 12));
    let row = line_with(&lines, "Where the agent writes");
    assert!(
        row.contains("project-choice") && row.ends_with("Direct"),
        "{row}"
    );

    let (mut a, mut far_a) = open(&mut fake, Some("/p/a"));
    go_to_category(&mut a, "Sandbox");
    let lines = shot(&mut a, "project-a", (80, 12));
    assert!(lines[0].ends_with("project /p/a"));
    let row = line_with(&lines, "Where the agent writes");
    assert!(
        row.contains("project-choice") && row.ends_with("Disposable"),
        "{row}"
    );

    fake.forget();
    go_to_row(&mut a, "sandbox.mode");
    press(&mut a, KeyCode::Enter);
    press(&mut a, KeyCode::Up);
    press(&mut a, KeyCode::Enter);
    pump(&mut a, &mut far_a, &mut fake);
    assert!(
        fake.set_configs().is_empty(),
        "never SetConfig in a project"
    );
    let saves: Vec<&v1::projects_request::SaveChoice> = fake
        .calls
        .iter()
        .filter_map(|call| match call {
            Request::Projects(v1::ProjectsRequest {
                op: Some(v1::projects_request::Op::SaveChoice(save)),
            }) => Some(save),
            _ => None,
        })
        .collect();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].project_id, "id-a");
    assert_eq!(
        saves[0]
            .values
            .iter()
            .map(|value| (value.key.as_str(), value.value.as_str()))
            .collect::<Vec<_>>(),
        [("sandbox.mode", "direct")]
    );
    let lines = shot(&mut a, "project-a-saved", (80, 12));
    assert!(line_with(&lines, "Where the agent writes").ends_with("Direct"));

    fake.forget();
    go_to_category(&mut a, "Agent");
    press(&mut a, KeyCode::Enter);
    pump(&mut a, &mut far_a, &mut fake);
    assert!(a.overlay.is_none());
    assert_eq!(
        fake.writes(),
        0,
        "outside the project choice nothing is written"
    );
}

/// Modellserver: Anlegen schickt `LlmServers.Add`; der Schlüssel steht
/// weder im Bild noch in einer Auskunft, und er geht genau einmal über
/// `LlmKey`.
#[test]
fn adding_a_server_and_its_key_never_shows_the_key() {
    let secret = "sk-very-secret-42";
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Models");
    go_to_row(&mut screen, "llm.servers");
    press(&mut screen, KeyCode::Enter);
    fake.forget();
    press(&mut screen, KeyCode::Char('a'));
    typing(&mut screen, "lab");
    press(&mut screen, KeyCode::Enter);
    typing(&mut screen, "http://10.0.0.5:11434");
    press(&mut screen, KeyCode::Enter);
    typing(&mut screen, "qwen, llama");
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    let adds: Vec<&v1::llm_servers_request::Add> = fake
        .calls
        .iter()
        .filter_map(|call| match call {
            Request::LlmServers(v1::LlmServersRequest {
                op: Some(v1::llm_servers_request::Op::Add(add)),
            }) => Some(add),
            _ => None,
        })
        .collect();
    assert_eq!(adds.len(), 1);
    assert_eq!(adds[0].name, "lab");
    assert_eq!(adds[0].endpoint, "http://10.0.0.5:11434");
    assert_eq!(adds[0].models, ["qwen", "llama"]);
    assert!(!adds[0].activate);
    let lines = shot(&mut screen, "servers", (80, 14));
    assert!(line_with(&lines, "lab").contains("http://10.0.0.5:11434"));

    fake.forget();
    press(&mut screen, KeyCode::Char('k'));
    typing(&mut screen, secret);
    let typing_shot = shot(&mut screen, "server-key-typing", (80, 14));
    assert!(typing_shot.join("\n").contains("Access key for lab"));
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    let after = shot(&mut screen, "server-key-stored", (80, 14));
    for lines in [&typing_shot, &after] {
        let text = lines.join("\n");
        assert!(!text.contains(secret), "the key is on screen:\n{text}");
        assert!(
            !text.contains("sk-very"),
            "part of the key is on screen:\n{text}"
        );
    }
    for note in screen.notes.values() {
        assert!(!format!("{note:?}").contains(secret));
    }
    let keys: Vec<&v1::LlmKeyRequest> = fake
        .calls
        .iter()
        .filter_map(|call| match call {
            Request::LlmKey(asked) => Some(asked),
            _ => None,
        })
        .filter(|asked| matches!(asked.op, Some(v1::llm_key_request::Op::Set(_))))
        .collect();
    assert_eq!(keys.len(), 1, "the key is sent once");
    assert_eq!(keys[0].server, "lab");
    assert!(matches!(&keys[0].op, Some(v1::llm_key_request::Op::Set(set)) if set.key == secret));
    assert!(fake.set_configs().is_empty());
}

/// `ConfigChanged` von einem anderen Client lädt neu: Die Zeile zeigt den
/// neuen Wert.
#[test]
fn config_changed_from_another_client_reloads_the_row() {
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Decision proxy");
    let before = shot(&mut screen, "before-change", (80, 12));
    assert!(line_with(&before, "Address").ends_with("http://127.0.0.1:8080"));
    fake.set("egress.proxy", "http://other:3128");
    far.replies
        .send(Reply {
            row: None,
            result: Ok(Answer::Changed(v1::flow_event::ConfigChanged {
                keys: vec!["egress.proxy".to_owned()],
                project_id: String::new(),
            })),
        })
        .expect("sent");
    pump(&mut screen, &mut far, &mut fake);
    let after = shot(&mut screen, "after-change", (80, 12));
    assert!(line_with(&after, "Address").ends_with("http://other:3128"));
}

/// Zugangsdaten des Proxys: Eingabe schickt genau ein `Egress.SetAuth` und
/// kein `SetConfig`, „Entfernen" genau ein `Egress.RemoveAuth`; das
/// Passwort steht nie im Bild.
#[test]
fn proxy_credentials_go_only_through_egress() {
    let credentials = "alice:pw-hunter2";
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Decision proxy");
    go_to_row(&mut screen, "egress.proxy_auth_ref");
    fake.forget();
    press(&mut screen, KeyCode::Enter);
    typing(&mut screen, credentials);
    let typing_shot = shot(&mut screen, "proxy-auth-typing", (80, 14));
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    let stored = shot(&mut screen, "proxy-auth-stored", (80, 14));
    assert!(line_with(&stored, "Credentials").ends_with("stored"));
    for lines in [&typing_shot, &stored] {
        let text = lines.join("\n");
        assert!(
            !text.contains("hunter2"),
            "the password is on screen:\n{text}"
        );
    }
    let egress = |fake: &Fake, set: bool| {
        fake.calls
            .iter()
            .filter(|call| match call {
                Request::Egress(v1::EgressRequest {
                    op: Some(v1::egress_request::Op::SetAuth(auth)),
                }) => set && auth.credentials == credentials,
                Request::Egress(v1::EgressRequest {
                    op: Some(v1::egress_request::Op::RemoveAuth(_)),
                }) => !set,
                _ => false,
            })
            .count()
    };
    assert_eq!(egress(&fake, true), 1, "exactly one SetAuth");
    assert!(
        fake.set_configs().is_empty(),
        "never SetConfig for credentials"
    );

    fake.forget();
    press(&mut screen, KeyCode::Char('x'));
    press(&mut screen, KeyCode::Char('y'));
    pump(&mut screen, &mut far, &mut fake);
    assert_eq!(egress(&fake, false), 1, "exactly one RemoveAuth");
    assert_eq!(fake.writes(), 1);
    let removed = shot(&mut screen, "proxy-auth-removed", (80, 14));
    assert!(line_with(&removed, "Credentials").ends_with("not stored"));
}

/// Eine lange Hilfe wird nach Anzeigebreite umbrochen: Kein Wort mit
/// Umlauten oder Zeichen doppelter Breite wird zerrissen.
#[test]
fn long_help_wraps_by_display_width() {
    let mut catalog = fake_catalog();
    let word = "Größenänderung";
    let wide = "設定変更";
    let help = format!("{word} {wide} ").repeat(6);
    catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id == "tls.trust_system_store")
        .expect("the switch")
        .help = help;
    let mut fake = Fake::with_catalog(catalog);
    let (mut screen, _far) = open(&mut fake, None);
    go_to_category(&mut screen, "Network");
    let lines = shot(&mut screen, "wrapped", (60, 20));
    let text = lines.join("\n");
    assert_eq!(text.matches(word).count(), 6, "{text}");
    assert_eq!(text.matches(wide).count(), 6, "{text}");
}

/// Zwei gespeicherte Server, `a` der Standard, `b` nicht.
fn two_servers(fake: &mut Fake) {
    for (name, active) in [("a", true), ("b", false)] {
        fake.servers.push(v1::LlmServer {
            name: name.to_owned(),
            endpoint: format!("http://{name}.lan:11434"),
            models: vec!["qwen".to_owned()],
            active,
            ..v1::LlmServer::default()
        });
    }
}

/// Jedes `LlmServers` außer `list`.
fn server_changes(fake: &Fake) -> Vec<&v1::llm_servers_request::Op> {
    fake.calls
        .iter()
        .filter_map(|call| match call {
            Request::LlmServers(asked) => asked.op.as_ref(),
            _ => None,
        })
        .filter(|op| !matches!(op, v1::llm_servers_request::Op::List(_)))
        .collect()
}

/// Ein gespeicherter Server, der nicht der Standard ist, bekommt eine neue
/// Adresse und neue Modelle über `LlmServers.Edit` für genau ihn; der
/// Standard bleibt, und `SetConfig` geht nicht hinaus.
#[test]
fn editing_a_saved_server_that_is_not_the_default_sends_edit_for_it() {
    let mut fake = Fake::new();
    two_servers(&mut fake);
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Models");
    go_to_row(&mut screen, "llm.servers");
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Down);
    fake.forget();
    press(&mut screen, KeyCode::Char('e'));
    screen.on_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    typing(&mut screen, "http://b2.lan:8000");
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    press(&mut screen, KeyCode::Char('m'));
    screen.on_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    typing(&mut screen, "llama, qwen");
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    let changes = server_changes(&fake);
    assert_eq!(changes.len(), 2, "one Edit per change and nothing else");
    for change in &changes {
        let v1::llm_servers_request::Op::Edit(edit) = change else {
            panic!("only Edit goes out");
        };
        assert_eq!(edit.name, "b");
    }
    assert!(fake.set_configs().is_empty());
    let b = fake
        .servers
        .iter()
        .find(|server| server.name == "b")
        .expect("b");
    assert_eq!(b.endpoint, "http://b2.lan:8000");
    assert_eq!(b.models, ["llama", "qwen"]);
    assert!(
        fake.servers
            .iter()
            .any(|server| server.name == "a" && server.active)
    );
    let lines = shot(&mut screen, "server-edited", (100, 14));
    assert!(line_with(&lines, "› b").contains("http://b2.lan:8000"));
}

/// Ein Projekt mit Wahl, dessen `Projects(get)` der Test zurückhält.
fn project_fake() -> Fake {
    let mut fake = Fake::new();
    fake.projects.insert(
        "/p/a".to_owned(),
        FakeProject {
            id: "id-a".to_owned(),
            chosen: vec![("agent.adapter".to_owned(), "opencode".to_owned())],
        },
    );
    fake
}

/// Jedes `save_choice`, in Reihenfolge.
fn saves(fake: &Fake) -> Vec<Vec<(String, String)>> {
    fake.calls
        .iter()
        .filter_map(|call| match call {
            Request::Projects(v1::ProjectsRequest {
                op: Some(v1::projects_request::Op::SaveChoice(save)),
            }) => Some(
                save.values
                    .iter()
                    .map(|value| (value.key.clone(), value.value.clone()))
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

/// Bevor `Projects(get)` die Wahl geliefert hat, schreibt der Bildschirm
/// nichts: `save_choice` ersetzt die ganze Wahl und verlöre die bekannte.
#[test]
fn an_edit_before_the_choice_has_loaded_sends_nothing() {
    let mut fake = project_fake();
    let (link, mut far) = link::pair();
    let mut screen = SettingsScreen::new(link, Some("/p/a".to_owned()));
    let mut held = Vec::new();
    loop {
        let mut busy = false;
        while let Ok(call) = far.calls.try_recv() {
            busy = true;
            if matches!(&call.request, Request::Projects(_)) {
                held.push(call);
            } else {
                far.replies.send(fake.answer(call)).expect("listens");
            }
        }
        busy |= screen.changed();
        if !busy {
            break;
        }
    }
    go_to_category(&mut screen, "Sandbox");
    go_to_row(&mut screen, "sandbox.mode");
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    assert!(
        saves(&fake).is_empty(),
        "nothing before the choice is known"
    );
    let lines = shot(&mut screen, "project-not-loaded", (80, 14));
    assert!(lines.join(" ").contains("has not loaded yet"), "{lines:#?}");

    for call in held {
        far.replies.send(fake.answer(call)).expect("listens");
    }
    pump(&mut screen, &mut far, &mut fake);
    go_to_row(&mut screen, "sandbox.mode");
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    assert_eq!(
        saves(&fake),
        [vec![
            ("agent.adapter".to_owned(), "opencode".to_owned()),
            ("sandbox.mode".to_owned(), "direct".to_owned()),
        ]]
    );
}

/// Zwei Änderungen schnell hintereinander: Die zweite wartet, bis die
/// erste gespeichert ist, und baut auf ihr auf; beide Schlüssel bleiben.
#[test]
fn two_edits_in_a_row_keep_both_keys() {
    let mut fake = project_fake();
    let (mut screen, mut far) = open(&mut fake, Some("/p/a"));
    go_to_category(&mut screen, "Sandbox");
    go_to_row(&mut screen, "sandbox.mode");
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Enter);
    // Ohne den Daemon dazwischen zu Wort kommen zu lassen: der Ordner. Öffnet
    // sich ein Feld, wird es gefüllt; solange gespeichert wird, öffnet sich
    // keines.
    go_to_row(&mut screen, "sandbox.work_dir");
    let folder = |screen: &mut SettingsScreen| {
        press(screen, KeyCode::Enter);
        if screen.overlay.is_some() {
            screen.on_event(&Event::Key(KeyEvent::new(
                KeyCode::Char('u'),
                KeyModifiers::CONTROL,
            )));
            typing(screen, "/srv/a");
            press(screen, KeyCode::Enter);
        }
    };
    folder(&mut screen);
    pump(&mut screen, &mut far, &mut fake);
    if saves(&fake)
        .iter()
        .all(|save| !save.iter().any(|(key, _)| key == "sandbox.work_dir"))
    {
        folder(&mut screen);
        pump(&mut screen, &mut far, &mut fake);
    }
    let all = saves(&fake);
    let last = all.last().expect("a save");
    for key in ["agent.adapter", "sandbox.mode", "sandbox.work_dir"] {
        assert!(
            last.iter().any(|(saved, _)| saved == key),
            "{key} lost in {all:?}"
        );
    }
    let chosen = &fake.projects["/p/a"].chosen;
    assert_eq!(chosen.len(), 3, "{chosen:?}");
}

/// Ein Befund in einer Antwort (hier `Egress(check)`) steht ganz an der
/// Zeile: Code, `why` und `fix`.
#[test]
fn a_finding_in_an_answer_shows_code_why_and_fix() {
    let mut fake = Fake::new();
    let mut catalog = fake_catalog();
    catalog
        .entries
        .push(entry("egress.check", "egress", 2, "Connection", "special"));
    fake.catalog = catalog;
    let (mut screen, mut far) = open(&mut fake, None);
    go_to_category(&mut screen, "Decision proxy");
    go_to_row(&mut screen, "egress.check");
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    let lines = shot(&mut screen, "egress-check", (80, 16));
    let text = lines.join(" ");
    assert!(text.contains("PROXY_014"), "{lines:#?}");
    assert!(text.contains("why: the proxy at 127.0.0.1:8080 refused the tunnel"));
    assert!(text.contains("fix: humanitl egress check"));
}

/// Endet der Ereignisstrom, sagt die Kopfzeile, dass Änderungen anderer
/// Clients erst nach `r` erscheinen.
#[test]
fn the_end_of_live_updates_is_shown() {
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, None);
    let lines = shot(&mut screen, "live", (100, 12));
    assert!(!lines[0].contains("no live updates"));
    far.replies
        .send(Reply {
            row: None,
            result: Ok(Answer::LiveEnded),
        })
        .expect("sent");
    pump(&mut screen, &mut far, &mut fake);
    let lines = shot(&mut screen, "live-ended", (100, 12));
    assert!(
        lines[0].contains("no live updates, r reloads"),
        "{}",
        lines[0]
    );
}

/// Kennung, Pfad und Werte eines `save_choice`.
type SavedChoice = (String, String, Vec<(String, String)>);

/// Jedes `save_choice` mit Kennung und Pfad.
fn saves_with_target(fake: &Fake) -> Vec<SavedChoice> {
    fake.calls
        .iter()
        .filter_map(|call| match call {
            Request::Projects(v1::ProjectsRequest {
                op: Some(v1::projects_request::Op::SaveChoice(save)),
            }) => Some((
                save.project_id.clone(),
                save.path.clone(),
                save.values
                    .iter()
                    .map(|value| (value.key.clone(), value.value.clone()))
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// Ein gültiger Ordner, den das Register noch nicht kennt: Die Wahl ist
/// leer und änderbar, und eine Änderung geht als genau ein `save_choice`
/// über den Pfad.
#[test]
fn an_unregistered_folder_saves_its_choice_by_path() {
    let mut fake = Fake::new();
    let (mut screen, mut far) = open(&mut fake, Some("/p/new"));
    go_to_category(&mut screen, "Sandbox");
    go_to_row(&mut screen, "sandbox.mode");
    fake.forget();
    press(&mut screen, KeyCode::Enter);
    assert!(
        screen.overlay.is_some(),
        "the choice of a new folder can be changed"
    );
    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    assert_eq!(
        saves_with_target(&fake),
        [(
            String::new(),
            "/p/new".to_owned(),
            vec![("sandbox.mode".to_owned(), "direct".to_owned())]
        )]
    );
    assert!(fake.set_configs().is_empty());
}

/// Nach `relink` (Editor, Neustart) gilt die Projektwahl als nicht geladen,
/// bis die neue Antwort da ist; danach baut eine Änderung auf dem, was ein
/// anderer Client inzwischen gespeichert hat.
#[test]
fn after_relink_the_choice_is_loaded_again_before_a_write() {
    let mut fake = project_fake();
    let (mut screen, _old) = open(&mut fake, Some("/p/a"));
    go_to_category(&mut screen, "Sandbox");
    go_to_row(&mut screen, "sandbox.mode");
    // Ein anderer Client, während der Editor offen war.
    fake.projects
        .get_mut("/p/a")
        .expect("a")
        .chosen
        .push(("sandbox.work_dir".to_owned(), "/srv/other".to_owned()));
    let (link, mut far) = link::pair();
    screen.relink(link);
    fake.forget();
    press(&mut screen, KeyCode::Enter);
    if screen.overlay.is_some() {
        press(&mut screen, KeyCode::Up);
        press(&mut screen, KeyCode::Enter);
    }
    let pending: Vec<Call> = std::iter::from_fn(|| far.calls.try_recv().ok()).collect();
    let early = pending
        .iter()
        .filter(|call| {
            matches!(
                call.request,
                Request::Projects(v1::ProjectsRequest {
                    op: Some(v1::projects_request::Op::SaveChoice(_))
                })
            )
        })
        .count();
    assert_eq!(early, 0, "nothing is written before the new choice arrived");
    for call in pending {
        far.replies.send(fake.answer(call)).expect("listens");
    }
    pump(&mut screen, &mut far, &mut fake);
    assert!(saves(&fake).is_empty());
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Enter);
    pump(&mut screen, &mut far, &mut fake);
    let last = saves(&fake).pop().expect("a save");
    assert!(
        last.iter()
            .any(|(key, value)| key == "sandbox.work_dir" && value == "/srv/other"),
        "the other client's change survives: {last:?}"
    );
}
