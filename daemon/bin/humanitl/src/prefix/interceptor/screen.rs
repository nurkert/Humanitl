//! Der Bildschirm des Interceptors (GitHub #12).
//!
//! Präfix und `i` öffnen ihn im laufenden Anschluss, wie `,` die
//! Einstellungen: oben die gehaltenen Anfragen mit
//! Methode, Host, Größe, Funden, Wartezeit und Frist, darunter die gewählte
//! mit dem Kopf aus dem Kasten von `--ask terminal` und dem Anfang ihres
//! Rumpfs. Entschieden wird mit einer Taste: `a` erlauben, `b` blocken, `r`
//! eine Regel in zwei Schritten und `Enter` zur Bestätigung. Mehr bietet die
//! App auch nicht an.
//!
//! # Was der Bildschirm nie tut
//!
//! - **Keine eigene Liste** (ADR-018). Die Zeilen kommen aus `ListFlows` und
//!   den Ereignissen von `Subscribe` ([`super::link`]); eine Zeile
//!   verschwindet erst, wenn der Daemon `Decided` oder `TimedOut` meldet,
//!   auch nach der eigenen Entscheidung.
//! - **Keine Entscheidung ohne Taste und nie für einen anderen Fluss.** Die
//!   Wahl hängt an der Kennung des Flusses, nicht an der Stelle in der Liste,
//!   und gewählt wird nur mit einer Pfeiltaste: nie von selbst, auch nicht
//!   die erste Zeile, die kommt (Review zu GitHub #12). Verschwindet der
//!   gewählte Fluss (anderswo entschieden, Frist abgelaufen), ist danach
//!   nichts gewählt. Ein `a`, das für den verschwundenen gemeint war, erlaubt
//!   so nie den Nachbarn, der an seine Stelle gerückt ist, und eines, das im
//!   selben Augenblick getippt wurde, in dem eine neue Anfrage kam, trifft sie
//!   nie.
//! - **Nichts Unsichtbares.** Solange das Terminal unter [`MIN`] ist, zeigt
//!   die Schleife nur „Terminal too small", und keine Taste außer `Esc`, `q`
//!   und `Ctrl+C` wirkt ([`Screen::drawn`]). Eine Regel geht erst hinaus,
//!   wenn ihr ganzer Satz auf dem Schirm stand; ein Menü nimmt seine Ziffern
//!   erst, wenn alle Zeilen zu sehen sind.
//! - **Kein fremder Text ungesäubert.** Host, Pfad, Funde und Rumpf kommen aus
//!   der Anfrage des Agenten; jedes Feld läuft durch
//!   [`crate::ask_terminal::display_text`], denselben Weg wie der Kasten von
//!   `--ask terminal`.
//! - **Nichts aus einem lesenden Anschluss.** Mit `--read-only` zeigt der
//!   Bildschirm Liste und Einzelheiten, `a`, `b` und `r` schicken nichts, und
//!   die Fußzeile sagt, wo entschieden wird.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use humanitl_ipc::v1;
use ratatui::Frame;

use super::draw;
use super::link::{Answer, Link, Request};
use crate::ask_terminal::{Held, display_text, from_detail};
pub(crate) use crate::cmd::moderate::RULE_TARGETS;
use crate::tui::Cells;
use crate::tui::frame::Theme;
use crate::tui::screen::{Flow, Screen};

/// Die kleinste Größe, in der der Interceptor zeichnet.
pub const MIN: Cells = Cells::new(60, 12);

/// Die Uhr des Bildschirms: Sekunden seit 1970. Im Produkt die Systemzeit,
/// im Test eine feste.
pub type Clock = fn() -> i64;

/// Wie groß das Terminal gerade ist, in Spalten und Zeilen. Im Produkt die
/// Größe auf `stdin`, wie sie auch das Backend misst; im Test eine feste.
pub type Measure = fn() -> (u16, u16);

/// Die Größe des Terminals auf `stdin`, wie `tui::backend` sie misst.
#[must_use]
pub fn terminal_size() -> (u16, u16) {
    let cells = Cells::from_window(crate::tty::window_size());
    (cells.cols, cells.rows)
}

/// Die Zeile, wenn das Terminal seit dem letzten Bild seine Größe geändert
/// hat: Was zu sehen war, ist nicht mehr sicher, die Taste wirkt nicht.
pub const RESIZED: &str = "the terminal changed size: look again, then press the key once more";

/// Die Systemzeit in Sekunden seit 1970.
#[must_use]
pub fn system_clock() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// Die Dauern einer Regel; `[1] once` legt keine Regel an.
pub const RULE_DURATIONS: [&str; 3] = ["[1] once", "[2] this session", "[3] forever"];

/// Die Zeile, wenn ein lesender Anschluss entscheiden will.
pub const READ_ONLY: &str = "read-only attach: decide in the writing terminal, in the app or with humanitl run --ask terminal";

/// Die Zeile, wenn eine Taste entscheiden soll, aber nichts gewählt ist.
const NOTHING_SELECTED: &str = "nothing selected: choose a request with the arrow keys";

/// Die Zeile, wenn eine Frage der Regel nicht ganz auf den Schirm passt.
pub const ENLARGE: &str =
    "enlarge the terminal: the rule does not fit, and its keys wait until it does";

/// Wo die Regel gerade steht.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleStep {
    /// Keine Regel in Arbeit.
    Idle,
    /// Das Ziel wird gewählt.
    Target,
    /// Die Dauer wird gewählt; das Ziel steht.
    Duration(usize),
    /// Wartet auf `Enter`: die fertige Anfrage und ihr Satz.
    Confirm {
        /// Die Entscheidung, wie sie nach `Enter` hinausgeht.
        request: Box<v1::DecideRequest>,
        /// Was sie bewirkt, in einer Zeile.
        sentence: String,
    },
}

/// Die Einzelheiten des gewählten Flusses.
#[derive(Debug, Clone)]
pub struct Detail {
    /// Der Kopf, wie der Kasten von `--ask terminal` ihn zeigt.
    pub held: Held,
    /// Der Anfang des Rumpfs (`FlowDetail.body_preview`), Zeile für Zeile,
    /// noch nicht gesäubert; gesäubert wird beim Zeichnen.
    pub body: Vec<String>,
}

/// Was beim letzten Zeichnen zu sehen war.
#[derive(Debug, Clone, Copy, Default)]
struct Seen {
    /// Der ganze Bildschirm, nicht der Hinweis „Terminal too small"
    /// ([`Screen::drawn`]).
    screen: bool,
    /// Die Frage der Regel vollständig: alle Zeilen des Menüs oder der ganze
    /// Satz vor der Bestätigung. Zurückgesetzt mit jedem Schritt der Regel.
    prompt: bool,
}

/// Der Bildschirm des Interceptors.
pub struct InterceptorScreen {
    /// Die Leitung zum Daemon.
    link: Link,
    /// Die gehaltenen Flüsse, ältester zuerst, wie der Daemon sie meldet.
    pub(super) rows: Vec<v1::FlowSummary>,
    /// Die Kennung des gewählten Flusses; gewählt wird nur mit einer
    /// Pfeiltaste.
    selected: Option<String>,
    /// Die Einzelheiten des gewählten Flusses, sobald `GetFlow` antwortet.
    pub(super) detail: Option<Detail>,
    /// Die Regel in Arbeit.
    pub(super) rule: RuleStep,
    /// Der Fluss, für den zuletzt eine Entscheidung hinausging.
    sent: Option<String>,
    /// Ob dieser Anschluss nur liest.
    pub(super) read_only: bool,
    /// Ob die erste Liste schon da ist.
    pub(super) loaded: bool,
    /// Ob der Ereignisstrom noch läuft.
    pub(super) live: bool,
    /// Was beim letzten Zeichnen zu sehen war.
    seen: Seen,
    /// Die Zeile rechts in der Fußzeile.
    pub(super) status: Option<String>,
    /// Die Uhr.
    clock: Clock,
    /// Die Größe des Terminals jetzt.
    measure: Measure,
    /// Die Größe des letzten Bildes; `None` vor dem ersten.
    frame: Option<(u16, u16)>,
    /// Die Sekunde, in der zuletzt gezeichnet wurde: Die Uhren der Fristen
    /// gehen weiter, auch ohne Taste.
    second: i64,
}

impl InterceptorScreen {
    /// Ein Bildschirm an `link`; `read_only` für einen lesenden Anschluss.
    #[must_use]
    pub fn new(link: Link, read_only: bool, clock: Clock) -> Self {
        Self {
            link,
            rows: Vec::new(),
            selected: None,
            detail: None,
            rule: RuleStep::Idle,
            sent: None,
            read_only,
            loaded: false,
            live: true,
            seen: Seen::default(),
            status: read_only.then(|| READ_ONLY.to_owned()),
            clock,
            measure: terminal_size,
            frame: None,
            second: clock(),
        }
    }

    /// Die Kennung des gewählten Flusses.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Die Kennungen der Zeilen, in ihrer Reihenfolge.
    #[cfg(test)]
    #[must_use]
    pub fn flow_ids(&self) -> Vec<&str> {
        self.rows.iter().map(|row| row.flow_id.as_str()).collect()
    }

    /// Die Stelle des gewählten Flusses in der Liste.
    pub(super) fn position(&self) -> Option<usize> {
        let selected = self.selected()?;
        self.rows.iter().position(|row| row.flow_id == selected)
    }

    /// Die Größe des Bildes, das gerade gezeichnet wurde ([`draw::draw`]).
    pub(super) const fn set_frame(&mut self, size: (u16, u16)) {
        self.frame = Some(size);
    }

    /// Mit einer anderen Messung der Terminalgröße, für Tests.
    #[cfg(test)]
    #[must_use]
    pub const fn with_measure(mut self, measure: Measure) -> Self {
        self.measure = measure;
        self
    }

    /// Ob das Terminal noch die Größe des letzten Bildes hat. Sonst gilt
    /// nichts mehr als gesehen: Schrumpft es, und die Taste kommt vor dem
    /// Ereignis der Größenänderung, beschriebe das letzte Bild einen
    /// Schirm, den es nicht mehr gibt (Review zu GitHub #12). Die Schleife
    /// zeichnet nach der Taste neu.
    fn still_the_same_size(&mut self) -> bool {
        if self.frame == Some((self.measure)()) {
            return true;
        }
        self.seen = Seen::default();
        self.status = Some(RESIZED.to_owned());
        false
    }

    /// Ob das letzte Bild die Frage der Regel ganz zeigte; das Zeichnen
    /// setzt es ([`draw::draw`]).
    pub(super) const fn set_prompt_seen(&mut self, seen: bool) {
        self.seen.prompt = seen;
    }

    /// Wählt `flow_id` und holt seine Einzelheiten.
    fn select(&mut self, flow_id: String) {
        if self.selected() == Some(flow_id.as_str()) {
            return;
        }
        self.detail = None;
        self.set_rule(RuleStep::Idle);
        let _ = self.link.send(Request::Detail(flow_id.clone()));
        self.selected = Some(flow_id);
    }

    /// Ein neuer Schritt der Regel; seine Frage gilt erst als gesehen, wenn
    /// sie gezeichnet wurde.
    fn set_rule(&mut self, step: RuleStep) {
        self.rule = step;
        self.seen.prompt = false;
    }

    /// Eine Pfeiltaste: eine Zeile weiter (`forward`) oder zurück. Ohne Wahl
    /// beginnt sie bei der ersten Zeile.
    fn step(&mut self, forward: bool) {
        if self.rows.is_empty() {
            return;
        }
        let next = match self.position() {
            None => 0,
            Some(at) if forward => (at + 1).min(self.rows.len() - 1),
            Some(at) => at.saturating_sub(1),
        };
        let flow_id = self.rows[next].flow_id.clone();
        self.select(flow_id);
    }

    /// Die Kennung des gewählten Flusses, wenn er noch in der Liste steht.
    fn current(&self) -> Option<String> {
        self.position().map(|at| self.rows[at].flow_id.clone())
    }

    /// `a` oder `b`: die Entscheidung für genau den gewählten Fluss.
    fn decide(&mut self, decision: v1::decide_request::Decision, verb: &str) {
        if self.read_only {
            self.status = Some(READ_ONLY.to_owned());
            return;
        }
        let Some(flow_id) = self.current() else {
            self.status = Some(NOTHING_SELECTED.to_owned());
            return;
        };
        self.send_decision(
            Box::new(v1::DecideRequest {
                flow_ids: vec![flow_id],
                decision: Some(decision),
                remember: None,
                acknowledged_findings: Vec::new(),
            }),
            verb,
        );
    }

    /// Schickt eine Entscheidung und sagt es in der Fußzeile.
    fn send_decision(&mut self, request: Box<v1::DecideRequest>, verb: &str) {
        let flow_id = request.flow_ids.first().cloned().unwrap_or_default();
        if self.link.send(Request::Decide(request)) {
            self.status = Some(format!("{verb} sent; the daemon confirms it"));
            self.sent = Some(flow_id);
        } else {
            self.status = Some("the connection to the daemon is gone".to_owned());
        }
    }

    /// `r`: die Regel beginnen, sobald die Einzelheiten da sind.
    fn start_rule(&mut self) {
        if self.read_only {
            self.status = Some(READ_ONLY.to_owned());
            return;
        }
        if self.current().is_none() {
            self.status = Some(NOTHING_SELECTED.to_owned());
            return;
        }
        if self.detail.is_none() {
            self.status = Some("the request is still loading".to_owned());
            return;
        }
        self.set_rule(RuleStep::Target);
    }

    /// Eine Taste, solange die Regel in Arbeit ist. Ziffern und `Enter`
    /// wirken nur, wenn die Frage beim letzten Zeichnen ganz zu sehen war.
    fn rule_key(&mut self, code: KeyCode) {
        let digit = match code {
            KeyCode::Char(c) => c.to_digit(10).and_then(|d| usize::try_from(d).ok()),
            _ => None,
        };
        let seen = self.seen.prompt;
        let step = std::mem::replace(&mut self.rule, RuleStep::Idle);
        let next = match (step, code, digit) {
            (_, KeyCode::Esc, _) => RuleStep::Idle,
            (step @ (RuleStep::Target | RuleStep::Duration(_)), _, Some(_)) if !seen => {
                self.status = Some(ENLARGE.to_owned());
                self.rule = step;
                return;
            }
            (step @ RuleStep::Confirm { .. }, KeyCode::Enter, _) if !seen => {
                self.status = Some(ENLARGE.to_owned());
                self.rule = step;
                return;
            }
            (RuleStep::Target, _, Some(target)) if (1..=RULE_TARGETS.len()).contains(&target) => {
                RuleStep::Duration(target)
            }
            (RuleStep::Duration(target), _, Some(duration))
                if (1..=RULE_DURATIONS.len()).contains(&duration) =>
            {
                self.rule_ready(target, duration)
            }
            (RuleStep::Confirm { request, .. }, KeyCode::Enter, _) => {
                self.send_decision(request, "allow with the rule");
                RuleStep::Idle
            }
            // Jede andere Taste in der Bestätigung verwirft die Regel, wie in
            // `--ask terminal`; in der Auswahl bleibt die Frage stehen.
            (RuleStep::Confirm { .. }, _, _) => {
                self.status = Some("rule dropped".to_owned());
                RuleStep::Idle
            }
            (step, _, _) => {
                self.rule = step;
                return;
            }
        };
        self.set_rule(next);
    }

    /// Ziel und Dauer stehen: die Anfrage bauen, oder sagen, warum es keine
    /// ehrliche Regel gibt.
    fn rule_ready(&mut self, target: usize, duration: usize) -> RuleStep {
        let (Some(flow_id), Some(detail)) = (self.current(), self.detail.as_ref()) else {
            return RuleStep::Idle;
        };
        let held = &detail.held;
        if held.host.is_empty() {
            self.status = Some("this request has no host to build a rule from".to_owned());
            return RuleStep::Idle;
        }
        let remember = if duration == 1 {
            None
        } else {
            match crate::cmd::moderate::rule_for(held, &held.host, target, duration) {
                Ok(rule) => Some(rule),
                Err(diagnostic) => {
                    self.status = Some(display_text(&diagnostic.why));
                    return RuleStep::Idle;
                }
            }
        };
        let sentence = remember.as_ref().map_or_else(
            || "allow once, without a rule".to_owned(),
            |rule| {
                format!(
                    "rule: allow {} - {}",
                    crate::cmd::moderate::describe_matcher(rule.matcher.as_ref()),
                    crate::cmd::moderate::describe_expiry(rule.expires.as_ref())
                )
            },
        );
        RuleStep::Confirm {
            request: Box::new(v1::DecideRequest {
                flow_ids: vec![flow_id],
                decision: Some(v1::decide_request::Decision::Allow(())),
                remember,
                acknowledged_findings: Vec::new(),
            }),
            sentence,
        }
    }

    /// Nimmt eine Antwort des Daemons. Gewählt wird dabei nie etwas.
    fn take(&mut self, answer: Answer) {
        match answer {
            Answer::Listed(rows) => {
                self.loaded = true;
                self.rows = rows;
                if self.selected().is_some() && self.position().is_none() {
                    self.lose();
                }
            }
            Answer::Arrived(row) => {
                if !self.rows.iter().any(|known| known.flow_id == row.flow_id) {
                    self.rows.push(row);
                }
            }
            Answer::Gone(flow_id) => {
                self.rows.retain(|row| row.flow_id != flow_id);
                if self.selected() == Some(flow_id.as_str()) {
                    self.lose();
                    if self.sent.as_deref() == Some(flow_id.as_str()) {
                        self.status =
                            Some("decided; choose the next request with the arrow keys".to_owned());
                    }
                }
            }
            Answer::Detail(detail) => {
                let held = from_detail(&detail);
                if self.selected() == Some(held.flow_id.as_str()) {
                    let body = detail.body_preview.lines().map(str::to_owned).collect();
                    self.detail = Some(Detail { held, body });
                }
            }
            Answer::Decided(response) => {
                if let Some(refused) = response.results.iter().find(|result| !result.applied) {
                    let why = refused
                        .diagnostic
                        .as_ref()
                        .map_or("the daemon did not apply it", |diagnostic| {
                            diagnostic.why.as_str()
                        });
                    self.status = Some(format!("not decided: {}", display_text(why)));
                }
            }
            Answer::Failed(diagnostic) => {
                self.status = Some(display_text(&format!(
                    "{}: {}",
                    diagnostic.code, diagnostic.why
                )));
            }
            Answer::LiveEnded => self.live = false,
        }
    }

    /// Der gewählte Fluss ist weg: nichts mehr gewählt, bis eine Pfeiltaste
    /// kommt.
    fn lose(&mut self) {
        self.selected = None;
        self.detail = None;
        self.set_rule(RuleStep::Idle);
        self.status = Some(
            "the selected request was decided elsewhere or timed out; choose another".to_owned(),
        );
    }

    /// Die Sekunden bis zur Frist eines Flusses, oder `None` ohne Frist.
    pub(super) fn left(&self, row: &v1::FlowSummary) -> Option<i64> {
        row.deadline
            .as_ref()
            .map(|deadline| deadline.seconds.saturating_sub(self.second).max(0))
    }

    /// Die Sekunden, die ein Fluss schon wartet.
    pub(super) fn waiting(&self, row: &v1::FlowSummary) -> i64 {
        row.received_at
            .as_ref()
            .map_or(0, |at| self.second.saturating_sub(at.seconds).max(0))
    }
}

impl Screen for InterceptorScreen {
    fn draw(&mut self, frame: &mut Frame<'_>, theme: Theme) {
        draw::draw(self, frame, theme);
    }

    fn drawn(&mut self, visible: bool) {
        self.seen.screen = visible;
    }

    fn on_event(&mut self, event: &Event) -> Flow {
        let Event::Key(key) = event else {
            return Flow::Continue;
        };
        if key.kind != KeyEventKind::Press
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Flow::Continue;
        }
        // Unter der Mindestgröße steht nur „Terminal too small": Was der
        // Mensch nicht sieht, entscheidet er nicht. Nur Schließen wirkt.
        if !self.seen.screen {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') => Flow::Done,
                _ => Flow::Continue,
            };
        }
        // Jede Taste, die entscheidet oder eine Regel weiterführt, prüft
        // vorher, ob das letzte Bild noch gilt; Pfeiltasten und Schließen
        // entscheiden nichts.
        let decides = match key.code {
            KeyCode::Char('a' | 'b' | 'r') => self.rule == RuleStep::Idle,
            KeyCode::Esc => false,
            _ => self.rule != RuleStep::Idle,
        };
        if decides && !self.still_the_same_size() {
            return Flow::Continue;
        }
        if self.rule != RuleStep::Idle {
            self.rule_key(key.code);
            return Flow::Continue;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Flow::Done,
            KeyCode::Down | KeyCode::Char('j') => self.step(true),
            KeyCode::Up | KeyCode::Char('k') => self.step(false),
            KeyCode::Char('a') => self.decide(v1::decide_request::Decision::Allow(()), "allow"),
            KeyCode::Char('b') => self.decide(
                v1::decide_request::Decision::Block(v1::decide_request::Block::default()),
                "block",
            ),
            KeyCode::Char('r') => self.start_rule(),
            _ => {}
        }
        Flow::Continue
    }

    fn changed(&mut self) -> bool {
        let mut changed = false;
        while let Some(answer) = self.link.try_answer() {
            self.take(answer);
            changed = true;
        }
        let now = (self.clock)();
        if now != self.second {
            self.second = now;
            changed = true;
        }
        changed
    }
}
