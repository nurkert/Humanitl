//! Der Erststart-Bildschirm von `humanitl go` (GitHub #9, ADR-0024 Punkt 3).
//!
//! Ein Bildschirm, alles vorbelegt: oben die Zeile der Vorgaben, darunter
//! „Start" mit dem Fokus (`Enter`) und „Customize…" (`a`). `Esc` und `q`
//! brechen ab; nichts startet, nichts wird gespeichert. Aufgeklappt stehen
//! die Felder des Vorschlags darunter, wie im Startfenster der App
//! (HUM-538): je Feld seine Optionen, eine nicht verfügbare Option sichtbar
//! mit ihrem Grund, ein von einer Firmenvorgabe gesperrtes Feld nur lesbar
//! mit der Vorgabe als Herkunft.
//!
//! # Woher alles kommt
//!
//! Aus `Projects.proposal` (ADR-018): Felder, Optionen, Vorgaben,
//! Verfügbarkeit, Gründe, Sperren, die Zeile der Vorgaben und der Hinweis auf
//! Cloud-Inferenz. Der Bildschirm hält keine eigene Liste von Feldern oder
//! Optionen; nur die Namen der Felder stehen hier, und eine Kennung, die
//! dieser Build nicht kennt, behält den Namen des Daemons. Jede Änderung
//! fragt `proposal` neu, mit den Paaren der gewählten Option über der Wahl,
//! und zeigt, was zurückkommt. Ein `ConfigChanged` aus dem Strom fragt
//! ebenso neu.
//!
//! # Wann „Start" geht
//!
//! Nur, wenn die gezeigte Antwort die letzte ist: Solange eine Frage an den
//! Daemon unterwegs ist, wartet `Enter` auf „Start" ([`FirstStartScreen::can_start`]).
//! `Enter` in einem aufgeklappten Feld wählt die markierte Option und startet
//! nie; ein Start mit einer Wahl, die der Daemon noch nicht beantwortet hat,
//! ist damit ausgeschlossen. Ein Befund des Daemons, der den Start mit den
//! Vorgaben ablehnt, oder eine gewählte Option, die nicht verfügbar ist,
//! halten „Start" ebenso an.
//!
//! „Start" beendet den Bildschirm mit der Wahl des gezeigten Vorschlags
//! (`StartProposal.choice`, [`Exit::Start`]). Gespeichert wird danach im
//! Befehl (`cmd::go`, `Projects.save_choice`); lehnt der Daemon ab, steht
//! der Bildschirm mit dem Befund wieder da ([`FirstStartScreen::refused`]).

pub mod view;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use humanitl_core::Diagnostic;
use humanitl_ipc::v1;
use ratatui::Frame;

use super::frame::Theme;
use super::screen::{Flow, Screen};
use super::settings::link::{Answer, Call, Link, Reply, Request};
use super::size::Cells;

/// Die kleinste Größe, mit der der Bildschirm Vorgaben, Knöpfe und ein
/// aufgeklapptes Feld zeigen kann.
pub const MIN: Cells = Cells::new(60, 16);

/// Der Weg ohne Bildschirm, den `TERM_003` nennt.
pub const WITHOUT_TUI: &str = "humanitl go --yes";

/// Der Anfang der Zeilenkennung einer `proposal`-Frage; dahinter steht ihre
/// laufende Nummer.
const PROPOSAL_ROW: &str = "proposal:";

/// Was nach dem Bildschirm zu tun ist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Mit dieser Wahl speichern und starten.
    Start(Vec<v1::ChoiceValue>),
    /// Abbrechen: nichts speichern, nichts starten.
    Cancel,
}

/// Wo der Fokus steht.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Auf „Start".
    Start,
    /// Auf einem Feld, nach seiner Stelle in `StartProposal.fields`.
    Field(usize),
}

/// Der Erststart-Bildschirm.
pub struct FirstStartScreen {
    /// Die Leitung zum Daemon.
    link: Link,
    /// Der Ordner, absolut, wie `Proposal.path` ihn nimmt.
    path: String,
    /// Die Werte der Sitzung, die jede Frage mitschickt (`Proposal.cli_overrides`).
    cli_overrides: Vec<v1::sandbox_request::CliOverride>,
    /// Der zuletzt beantwortete Vorschlag.
    proposal: v1::StartProposal,
    /// Die Teilwahl dieses Bildschirms über der gespeicherten Wahl.
    values: Vec<v1::ChoiceValue>,
    /// Die Nummer der letzten Frage; nur ihre Antwort zählt.
    asked: u64,
    /// Wahr, solange die letzte Frage unbeantwortet ist.
    asking: bool,
    /// Wahr, wenn die letzte Frage scheiterte: Der gezeigte Vorschlag gehört
    /// dann nicht zur Teilwahl, und „Start" wartet, bis `r` sie neu gefragt
    /// und der Daemon geantwortet hat.
    failed: bool,
    /// Ob die Felder aufgeklappt sind.
    expanded: bool,
    /// Wo der Fokus steht.
    focus: Focus,
    /// Die markierte Option im Feld mit dem Fokus; erst `Enter` wählt sie.
    cursor: usize,
    /// Was der Daemon gegen eine Frage oder das Speichern gesagt hat.
    refusals: Vec<Diagnostic>,
    /// Was nach dem Bildschirm zu tun ist.
    exit: Option<Exit>,
}

impl FirstStartScreen {
    /// Ein Bildschirm für `path` mit dem Vorschlag, den der Befehl schon
    /// gefragt hat, und den Werten der Sitzung, die jede neue Frage mitnimmt.
    #[must_use]
    pub fn new(
        link: Link,
        path: String,
        proposal: v1::StartProposal,
        cli_overrides: Vec<v1::sandbox_request::CliOverride>,
    ) -> Self {
        Self {
            link,
            path,
            cli_overrides,
            proposal,
            values: Vec::new(),
            asked: 0,
            asking: false,
            failed: false,
            expanded: false,
            focus: Focus::Start,
            cursor: 0,
            refusals: Vec::new(),
            exit: None,
        }
    }

    /// Was nach dem Bildschirm zu tun ist; `Cancel`, wenn nichts gesagt
    /// wurde.
    pub fn take_exit(&mut self) -> Exit {
        self.exit.take().unwrap_or(Exit::Cancel)
    }

    /// Der Daemon hat das Speichern abgelehnt: Der Befund steht auf dem
    /// Bildschirm, der danach wieder erscheint.
    pub fn refused(&mut self, diagnostic: Diagnostic) {
        self.refusals = vec![diagnostic];
    }

    /// Der gezeigte Vorschlag.
    #[must_use]
    pub const fn proposal(&self) -> &v1::StartProposal {
        &self.proposal
    }

    /// Ob die letzte Frage an den Daemon scheiterte.
    #[must_use]
    pub const fn failed(&self) -> bool {
        self.failed
    }

    /// Ob „Start" jetzt geht: die letzte Frage ist beantwortet, und zwar mit
    /// einem Vorschlag, kein Befund des Daemons lehnt den Start mit den
    /// Vorgaben ab, und jede gewählte Option ist verfügbar.
    #[must_use]
    pub fn can_start(&self) -> bool {
        !self.asking
            && !self.failed
            && !refuses(&self.proposal)
            && self
                .proposal
                .fields
                .iter()
                .all(|field| chosen(field).is_none_or(|option| option.available))
    }

    /// Fragt `proposal` mit der aktuellen Teilwahl neu.
    fn ask(&mut self) {
        self.asked += 1;
        self.asking = true;
        self.failed = false;
        let request = Request::Projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Proposal(
                v1::projects_request::Proposal {
                    path: self.path.clone(),
                    project_id: String::new(),
                    values: self.values.clone(),
                    cli_overrides: self.cli_overrides.clone(),
                },
            )),
        });
        // Ist die Gegenseite weg, kommt keine Antwort; „Start" wartet dann,
        // und `Esc` bricht ab.
        let _ = self.link.send(Call {
            row: Some(format!("{PROPOSAL_ROW}{}", self.asked)),
            request,
        });
    }

    /// Wählt die markierte Option des Felds `index` und fragt neu.
    ///
    /// Die Paare der Option ersetzen jeden Schlüssel des Felds; ein
    /// Schlüssel, den die Option nicht trägt, geht leer hinaus und fällt
    /// damit aus der Wahl (`Proposal.values`). Eine nicht verfügbare Option,
    /// ein gesperrtes Feld und die schon gewählte Option nimmt der
    /// Bildschirm nicht.
    fn choose(&mut self, index: usize) {
        let Some(field) = self.proposal.fields.get(index) else {
            return;
        };
        let Some(option) = field.options.get(self.cursor) else {
            return;
        };
        if field.lock.is_some() || !option.available || option.value == field.value {
            return;
        }
        let mut pairs: Vec<(String, String)> = self
            .values
            .iter()
            .map(|pair| (pair.key.clone(), pair.value.clone()))
            .collect();
        let mut put = |key: &str, value: &str| {
            pairs.retain(|(given, _)| given != key);
            pairs.push((key.to_owned(), value.to_owned()));
        };
        for key in &field.keys {
            put(key, "");
        }
        for pair in &option.choice {
            put(&pair.key, &pair.value);
        }
        self.values = pairs
            .into_iter()
            .map(|(key, value)| v1::ChoiceValue { key, value })
            .collect();
        self.refusals.clear();
        self.ask();
    }

    /// „Start": beendet den Bildschirm mit der Wahl des gezeigten
    /// Vorschlags, wenn [`Self::can_start`].
    fn start(&mut self) -> Flow {
        if !self.can_start() {
            return Flow::Continue;
        }
        self.exit = Some(Exit::Start(self.proposal.choice.clone()));
        Flow::Done
    }

    /// Klappt die Felder auf oder zu; zu steht der Fokus wieder auf „Start".
    fn toggle(&mut self) {
        self.expanded = !self.expanded;
        if !self.expanded {
            self.focus = Focus::Start;
        }
    }

    /// Ein Feld weiter oder zurück; vor dem ersten steht „Start".
    fn step(&mut self, down: bool) {
        if !self.expanded {
            return;
        }
        let fields = self.proposal.fields.len();
        self.focus = match (self.focus, down) {
            (Focus::Start, true) if fields > 0 => Focus::Field(0),
            (Focus::Start, _) | (Focus::Field(0), false) => Focus::Start,
            (Focus::Field(index), false) => Focus::Field(index - 1),
            (Focus::Field(index), true) => Focus::Field((index + 1).min(fields - 1)),
        };
        self.reset_cursor();
    }

    /// Die Markierung eine Option weiter oder zurück.
    fn slide(&mut self, right: bool) {
        let Focus::Field(index) = self.focus else {
            return;
        };
        let Some(field) = self.proposal.fields.get(index) else {
            return;
        };
        if field.lock.is_some() || field.options.is_empty() {
            return;
        }
        self.cursor = if right {
            (self.cursor + 1).min(field.options.len() - 1)
        } else {
            self.cursor.saturating_sub(1)
        };
    }

    /// Stellt die Markierung auf die gewählte Option des Felds mit dem Fokus.
    fn reset_cursor(&mut self) {
        self.cursor = match self.focus {
            Focus::Field(index) => self
                .proposal
                .fields
                .get(index)
                .and_then(|field| field.options.iter().position(|o| o.value == field.value))
                .unwrap_or(0),
            Focus::Start => 0,
        };
    }

    /// Nimmt eine Antwort.
    fn take_reply(&mut self, reply: Reply) {
        match reply.result {
            Ok(Answer::Changed(_)) => self.ask(),
            Ok(Answer::Project(response)) => {
                if !self.current(reply.row.as_deref()) {
                    return;
                }
                self.asking = false;
                if let Some(proposal) = response.proposal {
                    self.refusals.clear();
                    self.proposal = proposal;
                    self.clamp();
                } else {
                    // Eine Antwort ohne Vorschlag ist keine Antwort auf die
                    // Teilwahl: „Start" wartet wie nach einem Fehler.
                    self.failed = true;
                }
            }
            Err(diagnostic) => {
                if !self.current(reply.row.as_deref()) {
                    return;
                }
                // Scheitert die Frage, bleibt der letzte beantwortete
                // Vorschlag zur Ansicht stehen, aber er gehört nicht zur
                // Teilwahl: „Start" wartet, bis `r` neu gefragt hat, und der
                // Befund sagt, warum.
                self.asking = false;
                self.failed = true;
                self.refusals = vec![diagnostic];
            }
            Ok(_) => {}
        }
    }

    /// Ob eine Antwort zur letzten Frage gehört.
    fn current(&self, row: Option<&str>) -> bool {
        row.and_then(|row| row.strip_prefix(PROPOSAL_ROW))
            .and_then(|number| number.parse::<u64>().ok())
            .is_some_and(|number| number == self.asked)
    }

    /// Hält den Fokus in den Feldern des neuen Vorschlags.
    fn clamp(&mut self) {
        if let Focus::Field(index) = self.focus {
            let fields = self.proposal.fields.len();
            self.focus = if fields == 0 {
                Focus::Start
            } else {
                Focus::Field(index.min(fields - 1))
            };
        }
        self.reset_cursor();
    }

    /// Nimmt eine Taste.
    fn key(&mut self, key: &KeyEvent) -> Flow {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.exit = Some(Exit::Cancel);
                Flow::Done
            }
            KeyCode::Char('a') => {
                self.toggle();
                Flow::Continue
            }
            KeyCode::Char('r') if self.failed => {
                self.refusals.clear();
                self.ask();
                Flow::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.focus {
                Focus::Start => self.start(),
                Focus::Field(index) => {
                    self.choose(index);
                    Flow::Continue
                }
            },
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                self.step(false);
                Flow::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.step(true);
                Flow::Continue
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.slide(false);
                Flow::Continue
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.slide(true);
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }
}

/// Die gewählte Option eines Felds, wenn sie unter seinen Optionen steht.
#[must_use]
pub fn chosen(field: &v1::ProposalField) -> Option<&v1::ProposalOption> {
    field
        .options
        .iter()
        .find(|option| option.value == field.value)
}

/// Ob ein Befund des Vorschlags den Start mit den Vorgaben ablehnt.
#[must_use]
pub fn refuses(proposal: &v1::StartProposal) -> bool {
    proposal.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == v1::Severity::Error as i32
            || diagnostic.severity == v1::Severity::Blocking as i32
    })
}

impl Screen for FirstStartScreen {
    fn draw(&mut self, frame: &mut Frame<'_>, theme: Theme) {
        view::draw(self, frame, theme);
    }

    fn on_event(&mut self, event: &Event) -> Flow {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.key(key),
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
