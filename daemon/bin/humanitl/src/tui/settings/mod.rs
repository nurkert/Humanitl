//! Der Bildschirm `humanitl settings` (GitHub #7, ADR-0024 Punkt 2).
//!
//! Links die Kategorien, rechts ihre Zeilen, `/` sucht über alle. Jede Zeile
//! zeigt den wirksamen Wert und seine Herkunft (`default`, `global`,
//! `project-choice`, …); eine Zeile, die eine Firmenvorgabe sperrt, ist nur
//! lesbar und nennt die Vorgabe.
//!
//! # Woher alles kommt
//!
//! Aus `GetConfig(include_catalog)`: Kategorien, Reihenfolge, Texte,
//! Bedienart, Herkunft, Sperre und ob ein Schlüssel zur Projektwahl gehört.
//! Der Bildschirm hält keine eigene Liste von Einstellungen, Optionen oder
//! Texten; ein neuer Eintrag im Katalog erscheint hier ohne Änderung. Nur die
//! Zeilen mit `control = special` bekommen ein eigenes Bedienelement, über
//! ihre Kennung zugeordnet wie in der App ([`actions`], [`panels`]).
//!
//! # Wohin geschrieben wird
//!
//! - Global: `SetConfig`, genau ein Schlüssel.
//! - Mit Projekt (`--project PATH`): nur Schlüssel mit `in_project_choice`,
//!   über `Projects.save_choice`, nie über `SetConfig`; jede andere Zeile ist
//!   dort nur lesbar.
//! - Geheimnisse nur über ihre RPC: der Zugangsschlüssel über `LlmKey`, die
//!   Zugangsdaten des Proxys über `Egress`, beide verdeckt getippt und genau
//!   einmal geschickt. Ein `secret_ref` wird nie aufgelöst gezeigt.
//!
//! Eine Ablehnung des Daemons steht an der Zeile, mit `why` und `fix`. Nach
//! `ConfigChanged` aus dem Strom lädt der Bildschirm neu.

pub mod actions;
pub mod display;
pub mod edit;
pub mod link;
pub mod model;
pub mod panels;
pub mod replies;
pub mod view;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use humanitl_core::Diagnostic;
use humanitl_ipc::v1;
use ratatui::Frame;
use serde_json::Value;

use self::edit::{Overlay, Step};
use self::link::{Answer, Call, Link, Reply, Request};
use self::panels::Panel;
use super::frame::{Search, Theme};
use super::screen::{Flow, Screen};
use super::size::Cells;

/// Die kleinste Größe, mit der der Bildschirm Spalten, Zeilen und Hilfe
/// zeigen kann.
pub const MIN: Cells = Cells::new(60, 16);

/// Der Weg ohne Bildschirm, den `TERM_003` und der Hinweis ohne Terminal
/// nennen.
pub const WITHOUT_TUI: &str = "humanitl config get";

/// Was nach dem Bildschirm zu tun ist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Beenden.
    Quit,
    /// `config.toml` im `$EDITOR` öffnen und danach weiter.
    Editor(PathBuf),
    /// Den Daemon neu starten und danach weiter.
    Restart,
}

/// Wo der Fokus steht.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// In der linken Spalte.
    Categories,
    /// In den Zeilen.
    Rows,
}

/// Was an einer Zeile steht, nachdem etwas mit ihr geschah.
#[derive(Clone, Debug)]
pub enum Note {
    /// Der Daemon hat abgelehnt.
    Refused(Diagnostic),
    /// Eine Auskunft, etwa das Ergebnis einer Prüfung.
    Info(String),
    /// Eine Auskunft aus einer Antwort samt den Befunden, die der Daemon ihr
    /// mitgab, jeder mit Code, `why` und `fix`.
    Daemon(String, Vec<v1::Diagnostic>),
}

/// Die Zeile, an der eine Ablehnung beim Laden der Projektwahl steht.
pub const PROJECT_ROW: &str = "project";

/// Die Projektebene: der Ordner und was das Register über ihn weiß.
#[derive(Clone, Debug, Default)]
pub struct ProjectScope {
    /// Der Ordner, absolut, wie `GetConfig.project_id` und
    /// `Projects.get.path` ihn nehmen.
    pub path: String,
    /// Die Kennung aus dem Register; leer, solange es den Ordner nicht kennt.
    pub project_id: String,
    /// Die gespeicherte Projektwahl, Schlüssel für Schlüssel, nur die
    /// gewählten.
    pub chosen: Vec<(String, String)>,
    /// Ob `Projects(get)` die Wahl schon geliefert hat. Vorher schreibt der
    /// Bildschirm nichts: `save_choice` ersetzt die ganze Wahl, und eine
    /// unbekannte Wahl ginge dabei verloren.
    pub loaded: bool,
    /// Die Zeile, deren `save_choice` noch unterwegs ist. Solange schickt
    /// der Bildschirm kein zweites: Es baute auf einer Wahl, die gleich
    /// nicht mehr stimmt.
    pub saving: Option<String>,
}

/// Was die besonderen Zeilen aus ihren eigenen RPCs wissen.
#[derive(Clone, Debug, Default)]
pub struct Specials {
    /// `LlmServers(list)`.
    pub servers: Option<v1::LlmServerList>,
    /// `LlmKey(status)` des aktiven Servers.
    pub key: Option<v1::LlmKeyStatus>,
    /// `SandboxBackends`.
    pub backends: Option<v1::SandboxBackendList>,
    /// `Egress(status)`.
    pub egress: Option<v1::EgressStatus>,
    /// `Agents(status)`.
    pub agents: Option<v1::AgentsStatus>,
    /// `GetInfo`.
    pub info: Option<v1::Info>,
    /// Der erste Stand von `Update(check)`.
    pub update: Option<v1::UpdateStatus>,
}

/// Der Bildschirm der Einstellungen.
pub struct SettingsScreen {
    /// Die Leitung zum Daemon.
    link: Link,
    /// Die Projektebene, oder `None` für die globale.
    project: Option<ProjectScope>,
    /// Der Katalog der letzten Antwort.
    catalog: Option<v1::SettingsCatalog>,
    /// Die wirksamen Werte der letzten Antwort.
    values: Value,
    /// `ConfigSnapshot.config_path`.
    config_path: String,
    /// Was das Laden scheitern ließ.
    load_error: Option<Diagnostic>,
    /// Die Antworten der besonderen Zeilen.
    specials: Specials,
    /// Was an den Zeilen steht, nach Kennung.
    notes: BTreeMap<String, Note>,
    /// Wo der Fokus steht.
    focus: Focus,
    /// Die gewählte Kategorie unter den gezeigten.
    category: usize,
    /// Die gewählte Zeile unter den gezeigten.
    row: usize,
    /// Die Suche.
    search: Search,
    /// Die Seite einer besonderen Zeile, wenn eine offen ist.
    panel: Option<Panel>,
    /// Das Bedienelement über allem, wenn eines offen ist.
    overlay: Option<Overlay>,
    /// Was nach dem Bildschirm zu tun ist.
    exit: Option<Exit>,
    /// Der Ereignisstrom ist zu Ende: Änderungen anderer Clients erscheinen
    /// erst nach `r`.
    live_ended: bool,
}

impl SettingsScreen {
    /// Ein Bildschirm an `link`, global oder für den Ordner `project`, und
    /// die ersten Anfragen.
    #[must_use]
    pub fn new(link: Link, project: Option<String>) -> Self {
        let screen = Self {
            link,
            project: project.map(|path| ProjectScope {
                path,
                ..ProjectScope::default()
            }),
            catalog: None,
            values: Value::Null,
            config_path: String::new(),
            load_error: None,
            specials: Specials::default(),
            notes: BTreeMap::new(),
            focus: Focus::Categories,
            category: 0,
            row: 0,
            search: Search::default(),
            panel: None,
            overlay: None,
            exit: None,
            live_ended: false,
        };
        screen.load();
        screen
    }

    /// Hängt den Bildschirm an eine neue Leitung, etwa nach einem Neustart
    /// des Daemons, und lädt neu. Kategorie und Zeile bleiben.
    pub fn relink(&mut self, link: Link) {
        self.link = link;
        self.exit = None;
        self.live_ended = false;
        // Was ein anderer Client geändert hat, während Editor oder Neustart
        // liefen, kennt der Bildschirm nicht: Die Projektwahl gilt als nicht
        // geladen, bis die neue Antwort da ist.
        if let Some(scope) = self.project.as_mut() {
            scope.saving = None;
            scope.loaded = false;
            scope.project_id.clear();
            scope.chosen.clear();
        }
        self.load();
    }

    /// Was nach dem Bildschirm zu tun ist; `Quit`, wenn nichts gesagt wurde.
    pub fn take_exit(&mut self) -> Exit {
        self.exit.take().unwrap_or(Exit::Quit)
    }

    /// Schreibt eine Auskunft an eine Zeile, etwa nach einem Neustart.
    pub fn note(&mut self, row: &str, text: String) {
        self.notes.insert(row.to_owned(), Note::Info(text));
    }

    /// Schickt eine Anfrage.
    fn send(&self, row: Option<&str>, request: Request) {
        // Ist die Gegenseite weg, kommt keine Antwort; die Zeile zeigt dann
        // weiter den alten Stand, und der nächste Start verbindet neu.
        let _ = self.link.send(Call {
            row: row.map(str::to_owned),
            request,
        });
    }

    /// Fragt alles ab, was der Bildschirm zeigt.
    fn load(&self) {
        let project_path = self
            .project
            .as_ref()
            .map(|scope| scope.path.clone())
            .unwrap_or_default();
        self.send(
            None,
            Request::GetConfig(v1::GetConfigRequest {
                include_schema: false,
                project_id: project_path.clone(),
                include_catalog: true,
                language: String::new(),
            }),
        );
        if !project_path.is_empty() {
            // Ob das Register den Ordner kennt, sagt `proposal` eindeutig
            // (`StartProposal.project_id` leer heißt unbekannt); `get` gäbe
            // für einen unbekannten Ordner nur einen Fehler, der sich von
            // anderen nicht unterscheiden lässt.
            self.send(
                Some(PROJECT_ROW),
                Request::Projects(v1::ProjectsRequest {
                    op: Some(v1::projects_request::Op::Proposal(
                        v1::projects_request::Proposal {
                            path: project_path,
                            ..v1::projects_request::Proposal::default()
                        },
                    )),
                }),
            );
        }
        replies::load_specials(self);
    }

    /// Nimmt eine Antwort.
    fn take_reply(&mut self, reply: Reply) {
        let row = reply.row;
        if reply.result.is_err()
            && let Some(scope) = self.project.as_mut()
            && row.is_some()
            && scope.saving == row
        {
            scope.saving = None;
        }
        match reply.result {
            Err(diagnostic) => match row {
                Some(row) => {
                    self.notes.insert(row, Note::Refused(diagnostic));
                }
                None => self.load_error = Some(diagnostic),
            },
            Ok(answer) => self.take_answer(row, answer),
        }
    }

    /// Nimmt eine gelungene Antwort.
    fn take_answer(&mut self, row: Option<String>, answer: Answer) {
        match answer {
            Answer::Config(snapshot) => self.take_snapshot(snapshot),
            Answer::Project(response) => self.take_project(&response),
            Answer::Written => {
                if let Some(row) = row {
                    self.notes.remove(&row);
                }
                self.load();
            }
            Answer::Saved(response) => {
                if let Some(scope) = self.project.as_mut() {
                    scope.saving = None;
                }
                self.take_project(&response);
                if let Some(row) = row {
                    self.notes.remove(&row);
                }
                self.load();
            }
            Answer::Changed(changed) => {
                if self.concerns(&changed) {
                    self.load();
                }
            }
            Answer::LiveEnded => self.live_ended = true,
            other => replies::take_special(self, row, other),
        }
    }

    /// Übernimmt Werte und Katalog.
    fn take_snapshot(&mut self, snapshot: v1::ConfigSnapshot) {
        self.values = serde_json::from_str(&snapshot.values_json).unwrap_or(Value::Null);
        self.config_path = snapshot.config_path;
        self.load_error = None;
        if let Some(catalog) = snapshot.catalog {
            self.catalog = Some(catalog);
        }
        self.clamp();
    }

    /// Übernimmt, was `Projects` über den Ordner sagt.
    ///
    /// - `proposal` ohne Kennung: Das Register kennt den Ordner nicht. Die
    ///   Wahl ist leer und gilt als geladen; gespeichert wird über den Pfad.
    /// - `proposal` mit Kennung: Die Wahl kommt mit `get` an dieser Kennung.
    /// - `get` oder `save_choice`: Kennung und Wahl.
    fn take_project(&mut self, response: &v1::ProjectsResponse) {
        let Some(scope) = self.project.as_mut() else {
            return;
        };
        if let Some(proposal) = response.proposal.as_ref() {
            scope.project_id.clone_from(&proposal.project_id);
            if proposal.project_id.is_empty() {
                scope.chosen.clear();
                scope.loaded = true;
                return;
            }
            let project_id = proposal.project_id.clone();
            self.send(
                Some(PROJECT_ROW),
                Request::Projects(v1::ProjectsRequest {
                    op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                        project_id,
                        path: String::new(),
                    })),
                }),
            );
            return;
        }
        if let Some(project) = response.project.as_ref() {
            scope.project_id.clone_from(&project.project_id);
        }
        scope.loaded = true;
        scope.chosen = response
            .choice
            .iter()
            .filter(|field| !field.chosen.is_empty())
            .map(|field| (field.key.clone(), field.chosen.clone()))
            .collect();
    }

    /// Ob eine Änderung aus dem Strom diese Ansicht betrifft: die globale
    /// Ebene immer, eine Projektwahl nur, wenn es die gezeigte ist.
    fn concerns(&self, changed: &v1::flow_event::ConfigChanged) -> bool {
        if changed.project_id.is_empty() {
            return true;
        }
        self.project.as_ref().is_some_and(|scope| {
            scope.project_id.is_empty() || scope.project_id == changed.project_id
        })
    }

    /// Hält Kategorie und Zeile in den Grenzen dessen, was gezeigt wird.
    fn clamp(&mut self) {
        let categories = self.categories().len();
        self.category = self.category.min(categories.saturating_sub(1));
        let rows = self.rows().len();
        self.row = self.row.min(rows.saturating_sub(1));
    }

    /// Die gezeigten Kategorien.
    #[must_use]
    pub fn categories(&self) -> Vec<&v1::SettingsCategory> {
        self.catalog
            .as_ref()
            .map(model::categories)
            .unwrap_or_default()
    }

    /// Die gezeigten Zeilen: mit Suchtext die Treffer aus allen Kategorien,
    /// sonst die der gewählten Kategorie.
    #[must_use]
    pub fn rows(&self) -> Vec<&v1::SettingsEntry> {
        let Some(catalog) = self.catalog.as_ref() else {
            return Vec::new();
        };
        if !self.search.query().is_empty() {
            return model::all_entries(catalog)
                .into_iter()
                .filter(|entry| self.search.matches(&model::haystack(catalog, entry)))
                .collect();
        }
        self.categories()
            .get(self.category)
            .map(|category| model::entries(catalog, &category.id))
            .unwrap_or_default()
    }

    /// Die markierte Zeile.
    #[must_use]
    pub fn selected(&self) -> Option<&v1::SettingsEntry> {
        self.rows().get(self.row).copied()
    }

    /// Die Kennung der markierten Zeile.
    fn selected_id(&self) -> Option<String> {
        self.selected().map(|entry| entry.id.clone())
    }

    /// Nimmt eine Taste, wenn nichts darüber offen ist.
    fn browse(&mut self, key: &KeyEvent) -> Flow {
        if self.search.handle(key) {
            self.row = 0;
            self.focus = Focus::Rows;
            return Flow::Continue;
        }
        match key.code {
            KeyCode::Esc if !self.search.query().is_empty() => {
                self.search = Search::default();
                self.row = 0;
                Flow::Continue
            }
            KeyCode::Char('q') | KeyCode::Esc => self.quit(),
            KeyCode::Up | KeyCode::Char('k') => {
                self.step(false);
                Flow::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.step(true);
                Flow::Continue
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.focus = Focus::Categories;
                Flow::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.focus = Focus::Rows;
                Flow::Continue
            }
            KeyCode::Char('r') => {
                self.load();
                Flow::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.focus == Focus::Categories {
                    self.focus = Focus::Rows;
                } else if let Some(id) = self.selected_id() {
                    actions::activate(self, &id);
                }
                self.done()
            }
            KeyCode::Delete | KeyCode::Char('x') if self.focus == Focus::Rows => {
                if let Some(id) = self.selected_id() {
                    actions::remove(self, &id);
                }
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    /// Eine Zeile oder Kategorie weiter oder zurück.
    fn step(&mut self, down: bool) {
        let (index, len) = match self.focus {
            Focus::Categories => (self.category, self.categories().len()),
            Focus::Rows => (self.row, self.rows().len()),
        };
        let next = if down {
            (index + 1).min(len.saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        match self.focus {
            Focus::Categories => {
                self.category = next;
                self.row = 0;
            }
            Focus::Rows => self.row = next,
        }
    }

    /// Beenden.
    fn quit(&mut self) -> Flow {
        self.exit = Some(Exit::Quit);
        Flow::Done
    }

    /// `Done`, wenn eine Aktion den Bildschirm verlassen will.
    const fn done(&self) -> Flow {
        if self.exit.is_some() {
            Flow::Done
        } else {
            Flow::Continue
        }
    }

    /// Nimmt eine Taste für das offene Bedienelement.
    fn overlay_key(&mut self, key: &KeyEvent) -> Flow {
        let Some(mut overlay) = self.overlay.take() else {
            return Flow::Continue;
        };
        match &mut overlay {
            Overlay::Input(input) => match input.key(key) {
                Step::Open => self.overlay = Some(overlay),
                Step::Cancel => {}
                Step::Done(()) => {
                    let target = input.target.clone();
                    let text = input.take();
                    actions::commit(self, target, text);
                }
            },
            Overlay::Picker(picker) => match picker.key(key) {
                Step::Open => self.overlay = Some(overlay),
                Step::Cancel => {}
                Step::Done(value) => {
                    let target = picker.target.clone();
                    actions::commit(self, target, zeroize::Zeroizing::new(value));
                }
            },
            Overlay::Confirm(confirm) => match edit::Confirm::key(key) {
                Step::Open => self.overlay = Some(overlay),
                Step::Cancel => {}
                Step::Done(()) => {
                    let target = confirm.target.clone();
                    actions::commit(self, target, zeroize::Zeroizing::new(String::new()));
                }
            },
        }
        self.done()
    }
}

impl Screen for SettingsScreen {
    fn draw(&mut self, frame: &mut Frame<'_>, theme: Theme) {
        view::draw(self, frame, theme);
    }

    fn on_event(&mut self, event: &Event) -> Flow {
        match event {
            Event::Paste(text) => {
                if let Some(Overlay::Input(input)) = self.overlay.as_mut() {
                    input.paste(text);
                }
                Flow::Continue
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if self.overlay.is_some() {
                    self.overlay_key(key)
                } else if self.panel.is_some() {
                    panels::key(self, key)
                } else {
                    self.browse(key)
                }
            }
            _ => Flow::Continue,
        }
    }

    fn changed(&mut self) -> bool {
        let mut changed = false;
        while let Some(reply) = self.link.try_reply() {
            self.take_reply(reply);
            changed = true;
        }
        changed
    }
}
