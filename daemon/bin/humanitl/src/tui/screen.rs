//! Die Schleife eines Bildschirms: zeichnen, Taste lesen, Signal beachten
//! (GitHub #6).
//!
//! Ein Bildschirm sagt nur, wie er aussieht ([`Screen::draw`]) und was eine
//! Eingabe bewirkt ([`Screen::on_event`]). Was für alle gleich ist, steht
//! hier: Signale, Größenänderung, ein Terminal unter dem Minimum, `Ctrl+Z`
//! und `Ctrl+C`.
//!
//! - `Ctrl+Z` als Taste hält an wie `SIGTSTP` ([`TerminalGuard::suspend`]);
//!   im Rohmodus erzeugt das Terminal das Signal nicht selbst.
//! - `Ctrl+C` als Taste beendet den Bildschirm mit [`Outcome::Interrupted`],
//!   auf jedem Bildschirm gleich, damit niemand in einem festsitzt.
//! - `SIGTERM`, `SIGHUP`, `SIGINT` beenden ihn mit [`Outcome::Signal`]; der
//!   Befehl entscheidet danach, was mit seiner Arbeit geschieht, und endet
//!   mit `128 + n`.
//!
//! Im Terminal einer Sitzung (die Einstellungen über dem Agenten, GitHub
//! #11) läuft derselbe Bildschirm in [`run_in_session`]: Dort gehören
//! Rohmodus und Signale dem Anschluss, `Ctrl+Z` hält nichts an, und `Ctrl+C`
//! schließt nur die Ansicht.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Frame;

use super::backend::TtyBackend;
use super::frame::{Theme, too_small};
use super::guard::TerminalGuard;
use super::signals::{self, Signal};
use super::size::Cells;

/// So lange wartet die Schleife höchstens auf eine Taste, bevor sie nach
/// Signalen sieht.
const TICK: Duration = Duration::from_millis(50);

/// Ob ein Bildschirm weiterläuft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Weiter.
    Continue,
    /// Fertig; die Schleife endet mit [`Outcome::Done`].
    Done,
}

/// Wie ein Bildschirm endete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Der Bildschirm selbst sagte [`Flow::Done`].
    Done,
    /// `Ctrl+C` als Taste.
    Interrupted,
    /// `SIGTERM`, `SIGHUP` oder `SIGINT` von außen.
    Signal(Signal),
}

/// Ein Bildschirm.
pub trait Screen {
    /// Zeichnet den Bildschirm in `frame`.
    fn draw(&mut self, frame: &mut Frame<'_>, theme: Theme);

    /// Nimmt eine Eingabe: Taste, Einfügen, Maus, Größe.
    fn on_event(&mut self, event: &Event) -> Flow;

    /// Wird bei jedem Schritt der Schleife gefragt, auch ohne Eingabe; `true`
    /// heißt, der Bildschirm hat sich von sich aus geändert (eine neue
    /// Antwort des Daemons zum Beispiel) und wird neu gezeichnet.
    ///
    /// Ohne das zeichnet die Schleife nur nach einer Eingabe, einem Signal
    /// oder einer Größenänderung: Ein Bild je Schritt schriebe auch ohne
    /// Änderung jedes Mal Stilfolgen und das Verstecken des Cursors.
    fn changed(&mut self) -> bool {
        false
    }
}

/// Was die Schleife mit einer Eingabe tut, bevor der Bildschirm sie sieht.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// An den Bildschirm.
    Pass,
    /// Anhalten (`Ctrl+Z`).
    Suspend,
    /// Beenden (`Ctrl+C`).
    Interrupt,
}

/// Sortiert eine Eingabe.
fn classify(event: &Event) -> Step {
    let Event::Key(key) = event else {
        return Step::Pass;
    };
    if key.kind != KeyEventKind::Press || !key.modifiers.contains(KeyModifiers::CONTROL) {
        return Step::Pass;
    }
    match key.code {
        KeyCode::Char('z' | 'Z') => Step::Suspend,
        KeyCode::Char('c' | 'C') => Step::Interrupt,
        _ => Step::Pass,
    }
}

/// Ob jetzt gezeichnet wird: nach einer Eingabe (`dirty`) oder wenn der
/// Bildschirm sich von sich aus geändert hat.
///
/// [`Screen::changed`] läuft in jedem Schritt, auch wenn ohnehin gezeichnet
/// wird: Erst die Antworten, die inzwischen kamen, dann das Bild und die
/// nächste Taste. Eine Taste trifft so den Stand, den der Daemon schon
/// gemeldet hat.
fn needs_drawing(dirty: bool, screen: &mut impl Screen) -> bool {
    let changed = screen.changed();
    dirty || changed
}

/// Zeichnet ein Bild: den Bildschirm oder, unter dem Minimum, den Hinweis.
fn draw(
    terminal: &mut ratatui::Terminal<TtyBackend>,
    screen: &mut impl Screen,
    theme: Theme,
    min: Cells,
) -> io::Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        if Cells::new(area.width, area.height).holds(min) {
            screen.draw(frame, theme);
        } else {
            too_small(frame, min, theme);
        }
    })?;
    Ok(())
}

/// Überschreibt den Text eines Einfügens, nachdem der Bildschirm ihn hatte:
/// Er kann ein Geheimnis sein (ein eingefügter Zugangsschlüssel). Die
/// Lesepuffer von crossterm erreicht das nicht.
fn forget(event: Event) -> Event {
    use zeroize::Zeroize as _;

    match event {
        Event::Paste(mut text) => {
            text.zeroize();
            Event::Paste(text)
        }
        other => other,
    }
}

/// Lässt `screen` laufen, bis er fertig ist oder ein Signal ihn beendet.
///
/// # Errors
///
/// Wenn das Terminal nicht mehr zu beschreiben oder zu lesen ist, oder
/// wenn eine Panik in einem anderen Thread es schon zurückgegeben hat. Der
/// Wächter gibt es beim Fallenlassen trotzdem zurück.
pub fn run(
    guard: &mut TerminalGuard,
    screen: &mut impl Screen,
    theme: Theme,
    min: Cells,
) -> io::Result<Outcome> {
    let mut dirty = true;
    loop {
        if guard.lost() {
            return Err(io::Error::other(
                "a panic in another thread gave the terminal back; the view ends",
            ));
        }
        match signals::take() {
            Some(Signal::Stop) => {
                guard.suspend()?;
                dirty = true;
            }
            Some(Signal::Continue) => {
                guard.redraw()?;
                dirty = true;
            }
            Some(signal) => return Ok(Outcome::Signal(signal)),
            None => {}
        }
        if needs_drawing(dirty, screen) {
            draw(guard.terminal(), screen, theme, min)?;
            dirty = false;
        }
        if !crossterm::event::poll(TICK)? {
            continue;
        }
        let event = crossterm::event::read()?;
        dirty = true;
        let flow = match classify(&event) {
            Step::Suspend => {
                guard.suspend()?;
                Flow::Continue
            }
            Step::Interrupt => return Ok(Outcome::Interrupted),
            Step::Pass => screen.on_event(&event),
        };
        drop(forget(event));
        if flow == Flow::Done {
            return Ok(Outcome::Done);
        }
    }
}

/// Lässt `screen` im Terminal einer laufenden Sitzung laufen, über dem
/// Agenten (GitHub #11), bis er fertig ist, `Ctrl+C` kommt oder `stop`
/// gesetzt wird.
///
/// Anders als [`run`] hält kein [`TerminalGuard`] das Terminal: Rohmodus,
/// Signale und das Aufräumen gehören dem Anschluss, der den Schirm vorher
/// aufgesetzt hat (`prefix::view`). Deshalb:
///
/// - `Ctrl+Z` hält nichts an: Das Terminal gehört in dieser Zeit dem
///   Anschluss, und ein Anhalten ließe es roh zurück.
/// - `Ctrl+C` endet mit [`Outcome::Interrupted`]; der Anschluss schließt
///   damit nur die Ansicht.
/// - `SIGTERM` und die anderen Signale hört der Anschluss selbst; er setzt
///   `stop`, und die Schleife endet spätestens nach einem Takt.
///
/// # Errors
///
/// Wenn das Terminal nicht mehr zu beschreiben oder zu lesen ist.
pub fn run_in_session(
    terminal: &mut ratatui::Terminal<TtyBackend>,
    screen: &mut impl Screen,
    theme: Theme,
    min: Cells,
    stop: &AtomicBool,
) -> io::Result<Outcome> {
    let mut dirty = true;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(Outcome::Done);
        }
        if needs_drawing(dirty, screen) {
            draw(terminal, screen, theme, min)?;
            dirty = false;
        }
        if !crossterm::event::poll(TICK)? {
            continue;
        }
        let event = crossterm::event::read()?;
        dirty = true;
        let flow = match classify(&event) {
            Step::Suspend => Flow::Continue,
            Step::Interrupt => return Ok(Outcome::Interrupted),
            Step::Pass => screen.on_event(&event),
        };
        drop(forget(event));
        if flow == Flow::Done {
            return Ok(Outcome::Done);
        }
    }
}

/// Wie viele Ereignisse [`discard_pending_events`] höchstens verwirft.
const DISCARD_LIMIT: usize = 64;

/// Verwirft, was crossterm schon gelesen, aber noch nicht abgegeben hat,
/// nachdem ein Bildschirm in einer Sitzung zu Ende ist: Tasten, die im
/// selben Lesevorgang hinter `Esc` standen, gehören weder der nächsten
/// Ansicht noch dem Agenten.
///
/// **Begrenzt.** Höchstens [`DISCARD_LIMIT`] Ereignisse, jedes ohne Warten:
/// Ein Strom von Tasten, der nicht abreißt, hielte sonst die Rückkehr zum
/// Agenten für immer auf und äße auch Tasten späterer Lesevorgänge (Review
/// zu GitHub #11). Was danach kommt, liest wieder der Leser des Anschlusses.
pub fn discard_pending_events() {
    discard_bounded(
        || crossterm::event::poll(Duration::ZERO).unwrap_or(false),
        || crossterm::event::read().is_ok(),
    );
}

/// Der Kern von [`discard_pending_events`], ohne Terminal prüfbar: `ready`
/// sagt ohne Warten, ob ein Ereignis wartet, `take` nimmt es; `false` von
/// `take` beendet das Verwerfen.
fn discard_bounded(mut ready: impl FnMut() -> bool, mut take: impl FnMut() -> bool) {
    for _ in 0..DISCARD_LIMIT {
        if !ready() || !take() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::{Flow, Screen, Step, classify, forget, needs_drawing};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    /// `Ctrl+Z` hält an, `Ctrl+C` beendet; alles andere, auch `z` und `c`
    /// ohne `Ctrl`, geht an den Bildschirm.
    #[test]
    fn control_z_suspends_and_control_c_interrupts() {
        assert_eq!(
            classify(&key(KeyCode::Char('z'), KeyModifiers::CONTROL)),
            Step::Suspend
        );
        assert_eq!(
            classify(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Step::Interrupt
        );
        assert_eq!(
            classify(&key(KeyCode::Char('z'), KeyModifiers::NONE)),
            Step::Pass
        );
        assert_eq!(
            classify(&key(KeyCode::Char('c'), KeyModifiers::SHIFT)),
            Step::Pass
        );
        assert_eq!(classify(&Event::Resize(80, 24)), Step::Pass);
        let release = Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Release,
        ));
        assert_eq!(classify(&release), Step::Pass, "only a press counts");
    }

    /// Ein Bildschirm, der zählt, wie oft nach Antworten gefragt wurde.
    struct Counting(usize);

    impl Screen for Counting {
        fn draw(&mut self, _frame: &mut ratatui::Frame<'_>, _theme: super::Theme) {}

        fn on_event(&mut self, _event: &Event) -> Flow {
            Flow::Continue
        }

        fn changed(&mut self) -> bool {
            self.0 += 1;
            false
        }
    }

    /// Auch wenn ohnehin gezeichnet wird, nimmt der Bildschirm zuerst die
    /// Antworten, die inzwischen kamen (#9, Review).
    ///
    /// Mutation proof: `dirty || screen.changed()` skips `changed` after an
    /// input, and the count stays 0.
    #[test]
    fn replies_are_taken_before_every_frame() {
        let mut screen = Counting(0);
        assert!(needs_drawing(true, &mut screen));
        assert_eq!(screen.0, 1, "changed ran although dirty");
        assert!(!needs_drawing(false, &mut screen));
        assert_eq!(screen.0, 2);
    }

    /// Auch ein Strom von Ereignissen, der nie abreißt, hält das Verwerfen
    /// nach einem Bildschirm nicht auf: Es endet nach der Grenze. Gemessen in
    /// einem eigenen Faden mit Frist. Mutationsbeweis: ohne Grenze verwirft
    /// es für immer, und der Test wird rot.
    #[test]
    #[allow(clippy::expect_used)]
    fn discarding_stops_even_under_endless_input() {
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut taken = 0_usize;
            super::discard_bounded(
                || true,
                || {
                    taken += 1;
                    true
                },
            );
            let _ = done.send(taken);
        });
        let taken = finished
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("discarding ended within the deadline");
        assert_eq!(taken, super::DISCARD_LIMIT);
    }

    /// Der Text eines Einfügens ist danach überschrieben und leer; andere
    /// Ereignisse bleiben, wie sie sind.
    #[test]
    fn a_paste_is_wiped_after_the_screen_had_it() {
        assert_eq!(
            forget(Event::Paste("sk-pasted".to_owned())),
            Event::Paste(String::new())
        );
        assert_eq!(forget(Event::Resize(80, 24)), Event::Resize(80, 24));
    }
}
