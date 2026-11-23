//! Der Wächter eines Bildschirms: Rohmodus, alternativer Bildschirm, Cursor,
//! Maus und Einfügeklammern an, und auf jedem Weg hinaus wieder aus
//! (GitHub #6).
//!
//! # Die Wege hinaus
//!
//! 1. **Gewöhnliches Ende und jeder Fehlerpfad**: `Drop` von
//!    [`TerminalGuard`].
//! 2. **Panik**: ein Hook, der vor der Meldung zurückgibt. Ohne ihn stünde
//!    die Meldung im alternativen Bildschirm und verschwände mit ihm, und bei
//!    `panic = "abort"` (Profil `dist`) liefe `Drop` gar nicht.
//! 3. **`SIGTERM`, `SIGHUP`, `SIGINT`**: Die Schleife in [`super::screen`]
//!    bemerkt sie ([`super::signals`]) und endet; `Drop` gibt zurück, und der
//!    Befehl endet danach mit `128 + n`, nachdem er beendet hat, was er
//!    begonnen hat.
//! 4. **`SIGTSTP`** (und `Ctrl+Z` als Taste): [`TerminalGuard::suspend`] gibt
//!    zurück, hält an und setzt nach `SIGCONT` alles wieder und zeichnet neu.
//!    Der Wächter meldet sich bei [`super::signals`] an, bevor er das Terminal
//!    anfasst, und erst ab, nachdem alles zurück ist; dazwischen hält ein
//!    `SIGTSTP` nie mit rohem Terminal an.
//! 5. **Panik in einem anderen Thread**: Der Hook gibt zurück und schaltet
//!    die Ausgabe stumm; die Schleife bemerkt es ([`TerminalGuard::lost`])
//!    und endet mit einem Fehler, statt in ein gewöhnliches Terminal zu
//!    zeichnen.
//!
//! # Ein Besitzer der `termios`
//!
//! Den Rohmodus setzt dieser Wächter nicht selbst und auch nicht über
//! crossterm (`enable_raw_mode` hätte eine eigene Sicherung), sondern über
//! [`crate::tty::RawMode::enter_exclusive`]: Es gibt je Prozess genau eine
//! gesicherte Einstellung und genau einen Weg zurück. Prüfen und Setzen
//! geschehen dort unter einer Sperre; ein zweiter Wächter oder ein Wächter
//! über einem `RawMode` von `sandbox attach` wird abgewiesen, auch wenn zwei
//! Threads zugleich kommen. crossterm schreibt hier nur Steuerfolgen und liest
//! Tasten.
//!
//! Was zurückgeschrieben werden muss, steht vorberechnet in einem globalen
//! Platz ([`ACTIVE`]): Der Panik-Hook lebt länger als jeder Rahmen und darf
//! nichts mehr zuteilen müssen.

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::style::{Attribute, SetAttribute};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use humanitl_core::Diagnostic;
use ratatui::Terminal;

use super::backend::{TermOut, TtyBackend, write_all};
use super::signals;
use super::size::{Capabilities, Cells, unsuitable};
use crate::tty::{self, RawMode};

/// Was ein Bildschirm vom Terminal braucht.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// Kleinste Größe, mit der der Bildschirm etwas zeigen kann.
    pub min: Cells,
    /// Mausereignisse lesen. Aus, wo der Bildschirm sie nicht braucht: Mit
    /// Maus lässt sich im Terminal kein Text mehr markieren.
    pub mouse: bool,
    /// Der Weg ohne Bildschirm, den `TERM_003` nennt.
    pub without_tui: &'a str,
}

/// Warum kein Bildschirm entsteht. Das Terminal ist in jedem Fall unverändert.
#[derive(Debug)]
pub enum Unavailable {
    /// `stdin` oder `stdout` ist kein Terminal. Der Befehl nimmt den Weg ohne
    /// Bildschirm, den sein Issue nennt, ohne Befund.
    NoTerminal,
    /// Das Terminal ist zu klein oder kann keinen Cursor setzen (`TERM_003`).
    Unsuitable(Box<Diagnostic>),
    /// Es steht schon ein Bildschirm, ein anderer Rahmen hält den Rohmodus,
    /// oder das System gab keinen Deskriptor und keine Signale her.
    Failed(io::Error),
}

/// Was der Panik-Hook zurückschreiben muss, solange ein Bildschirm steht.
struct Active {
    /// Eine eigene Kopie des Terminals.
    out: OwnedFd,
    /// Die Folge, die alles ausschaltet, was der Bildschirm eingeschaltet hat.
    leave: Vec<u8>,
    /// Der Stumm-Schalter der Ausgabe ([`TermOut::mute_switch`]): Nach der
    /// Rückgabe durch den Hook zeichnet ratatui nichts mehr, auch nicht aus
    /// einem anderen Thread als dem, der panikte.
    muted: Arc<AtomicBool>,
}

/// `Some`, solange ein Bildschirm steht.
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

/// Nimmt den Eintrag aus [`ACTIVE`], damit genau einer zurückschreibt.
fn take_active() -> Option<Active> {
    ACTIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

/// Gibt das Terminal jetzt zurück: Steuerfolgen, dann `termios`.
///
/// Idempotent. Der Panik-Hook ruft sie, und `Drop` danach findet nichts mehr.
fn leave_now() {
    if let Some(active) = take_active() {
        active.muted.store(true, Ordering::SeqCst);
        let _ = write_all(active.out.as_fd(), &active.leave);
    }
    tty::restore_now();
}

/// Der Panik-Hook, genau einmal je Prozess, vor dem bisherigen.
///
/// Er bleibt stehen wie der aus `tty.rs`: Solange kein Bildschirm steht, tut
/// er nichts außer `termios` zurückzuschreiben, was der Hook von `tty.rs`
/// ohnehin täte.
fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            leave_now();
            previous(info);
        }));
    });
}

/// Die Folgen zum Betreten und Verlassen, vorberechnet.
///
/// Ohne alternativen Bildschirm wird beim Betreten und beim Verlassen
/// gelöscht: Was der Bildschirm gezeichnet hat, bleibt nicht im Verlauf.
#[must_use]
pub fn sequences(caps: Capabilities, mouse: bool) -> (Vec<u8>, Vec<u8>) {
    let mut enter = Vec::new();
    let mut leave = Vec::new();
    // In einen `Vec` zu schreiben scheitert nicht.
    if caps.alternate_screen {
        let _ = crossterm::queue!(enter, EnterAlternateScreen);
    }
    let _ = crossterm::queue!(
        enter,
        Hide,
        Clear(ClearType::All),
        MoveTo(0, 0),
        EnableBracketedPaste
    );
    if mouse {
        let _ = crossterm::queue!(enter, EnableMouseCapture);
        let _ = crossterm::queue!(leave, DisableMouseCapture);
    }
    let _ = crossterm::queue!(
        leave,
        DisableBracketedPaste,
        SetAttribute(Attribute::Reset),
        Show
    );
    if caps.alternate_screen {
        let _ = crossterm::queue!(leave, LeaveAlternateScreen);
    } else {
        let _ = crossterm::queue!(leave, Clear(ClearType::All), MoveTo(0, 0));
    }
    (enter, leave)
}

/// Ein Bildschirm, solange er lebt.
pub struct TerminalGuard {
    /// Zeichnet die Bilder.
    terminal: Terminal<TtyBackend>,
    /// Der Rohmodus; `None` zwischen [`TerminalGuard::suspend`] und dem
    /// Weiterlaufen.
    raw: Option<RawMode>,
    /// Eine eigene Kopie von `stdout` für die Folgen.
    out: OwnedFd,
    /// Macht die Ausgabe von ratatui stumm, solange das Terminal
    /// zurückgegeben ist ([`TermOut::mute_switch`]).
    muted: Arc<AtomicBool>,
    /// Folge zum Betreten.
    enter: Vec<u8>,
    /// Folge zum Verlassen.
    leave: Vec<u8>,
}

impl TerminalGuard {
    /// Setzt das Terminal für einen Bildschirm.
    ///
    /// Geprüft wird alles, bevor sich etwas ändert: Terminal auf `stdin` und
    /// `stdout` (sonst [`Unavailable::NoTerminal`]; `/dev/tty` wird nie
    /// geöffnet), Cursorsteuerung und Größe (sonst `TERM_003`).
    ///
    /// # Errors
    ///
    /// [`Unavailable`] mit dem Grund; das Terminal ist dann unverändert.
    pub fn enter(options: &Options<'_>) -> Result<Self, Unavailable> {
        if !super::available() {
            return Err(Unavailable::NoTerminal);
        }
        let caps = Capabilities::from_env(
            std::env::var("TERM").ok().as_deref(),
            std::env::var("TERM_PROGRAM").ok().as_deref(),
        );
        if let Some(found) = unsuitable(caps, Cells::reported(), options.min, options.without_tui) {
            return Err(Unavailable::Unsuitable(Box::new(found)));
        }
        if !signals::install() {
            return Err(Unavailable::Failed(io::Error::other(
                "the signal handlers of the full-screen view could not be installed",
            )));
        }
        let out = rustix::stdio::stdout()
            .try_clone_to_owned()
            .map_err(Unavailable::Failed)?;
        let backend_out = TermOut::new(out.try_clone().map_err(Unavailable::Failed)?);
        let muted = backend_out.mute_switch();
        let terminal = Terminal::new(TtyBackend::new(backend_out)).map_err(Unavailable::Failed)?;
        let (enter, leave) = sequences(caps, options.mouse);
        let mut guard = Self {
            terminal,
            raw: None,
            out,
            muted,
            enter,
            leave,
        };
        // Ab hier gibt `Drop` alles zurück, was schon gesetzt ist.
        signals::screen_enter();
        guard.set().map_err(Unavailable::Failed)?;
        Ok(guard)
    }

    /// Setzt Rohmodus und Folgen, beim Betreten und nach dem Anhalten.
    ///
    /// Der Aufrufer hat sich schon bei [`signals::screen_enter`] angemeldet.
    /// Gelingt der Rohmodus nicht, meldet `set` ihn wieder ab: Ohne Rohmodus
    /// gibt `Drop` nichts zurück und meldet auch nichts ab.
    fn set(&mut self) -> io::Result<()> {
        let Some(raw) = RawMode::enter_exclusive() else {
            signals::screen_leave();
            return Err(io::Error::other(
                "the terminal is held by another view of this process, or it no longer takes the raw mode",
            ));
        };
        self.raw = Some(raw);
        {
            let mut active = ACTIVE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *active = Some(Active {
                out: self.out.try_clone()?,
                leave: self.leave.clone(),
                muted: Arc::clone(&self.muted),
            });
        }
        install_panic_hook();
        self.muted.store(false, Ordering::SeqCst);
        signals::forget_stops();
        write_all(self.out.as_fd(), &self.enter)?;
        // Das nächste Bild zeichnet alles neu, nicht nur, was sich änderte.
        self.terminal.clear()
    }

    /// Gibt alles zurück, was [`TerminalGuard::set`] gesetzt hat; ohne
    /// Rohmodus nichts.
    ///
    /// `stay_registered` gilt für das Anhalten: Der Wächter bleibt bei
    /// [`signals`] angemeldet, damit ein echtes `SIGTSTP` zwischen Rückgabe
    /// und eigenem Anhalten nicht ein zweites Mal anhält. Sonst meldet er sich
    /// erst ab, wenn auch die `termios` zurück sind.
    fn reset(&mut self, stay_registered: bool) {
        let Some(raw) = self.raw.take() else {
            return;
        };
        self.muted.store(true, Ordering::SeqCst);
        if let Some(active) = take_active() {
            let _ = write_all(active.out.as_fd(), &active.leave);
        }
        // `RawMode` schreibt beim Fallenlassen die `termios` zurück.
        drop(raw);
        if !stay_registered {
            signals::screen_leave();
        }
    }

    /// Gibt den Rohmodus zurück, ohne zu schreiben: nach `SIGHUP`, wenn es das
    /// Terminal nicht mehr gibt und jedes Schreiben `EIO` bekäme (GitHub #13).
    /// Die `termios` gehen mit `tcsetattr` zurück, die Folge zum Verlassen
    /// des Schirms nicht. Danach tut `Drop` nichts mehr.
    pub fn abandon(&mut self) {
        let Some(raw) = self.raw.take() else {
            return;
        };
        self.muted.store(true, Ordering::SeqCst);
        drop(take_active());
        drop(raw);
        signals::screen_leave();
    }

    /// Gibt das Terminal zurück, hält den Prozess an wie `SIGTSTP` es täte,
    /// und setzt nach `SIGCONT` alles wieder.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal danach nicht wieder zu setzen ist; der Bildschirm
    /// endet dann, das Terminal bleibt gewöhnlich.
    pub fn suspend(&mut self) -> io::Result<()> {
        self.reset(true);
        // Hält hier an; nach `SIGCONT` geht es in der nächsten Zeile weiter.
        // Ein `SIGTSTP`, das inzwischen kam, steht nur im Merker (der Wächter
        // ist noch angemeldet), und `set` vergisst es.
        let _ = signal_hook::low_level::emulate_default_handler(signal_hook::consts::SIGTSTP);
        self.set()
    }

    /// Wahr, wenn ein Panik-Hook das Terminal zurückgegeben hat, während der
    /// Wächter es noch hält: Ein anderer Thread ist gefallen. Die Schleife
    /// endet dann, statt in ein gewöhnliches Terminal zu zeichnen.
    #[must_use]
    pub fn lost(&self) -> bool {
        self.raw.is_some()
            && ACTIVE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
    }

    /// Zeichnet beim nächsten Bild alles neu, nach `SIGCONT` von außen.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr zu beschreiben ist.
    pub fn redraw(&mut self) -> io::Result<()> {
        self.terminal.clear()
    }

    /// Das Terminal von ratatui, zum Zeichnen.
    pub const fn terminal(&mut self) -> &mut Terminal<TtyBackend> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.reset(false);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Capabilities, sequences};

    fn text(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).expect("ASCII")
    }

    /// Mit alternativem Bildschirm: hinein zuerst, hinaus zuletzt, und alles,
    /// was eingeschaltet wurde, wird ausgeschaltet.
    #[test]
    fn leaving_undoes_everything_entering_did() {
        let caps = Capabilities::from_term(Some("xterm-256color"));
        let (enter, leave) = sequences(caps, true);
        let (enter, leave) = (text(&enter), text(&leave));
        assert!(enter.starts_with("\x1b[?1049h"), "{enter:?}");
        assert!(enter.contains("\x1b[?25l"), "cursor hidden: {enter:?}");
        assert!(enter.contains("\x1b[?2004h"), "bracketed paste: {enter:?}");
        assert!(enter.contains("\x1b[?1000h"), "mouse: {enter:?}");
        assert!(leave.ends_with("\x1b[?1049l"), "{leave:?}");
        assert!(leave.contains("\x1b[?25h"), "cursor shown: {leave:?}");
        assert!(
            leave.contains("\x1b[?2004l"),
            "bracketed paste off: {leave:?}"
        );
        assert!(leave.contains("\x1b[?1000l"), "mouse off: {leave:?}");
        assert!(leave.contains("\x1b[0m"), "attributes reset: {leave:?}");
    }

    /// Ohne alternativen Bildschirm keine Umschaltung, dafür gelöscht am
    /// Ende; ohne Maus wird sie weder ein- noch ausgeschaltet.
    #[test]
    fn without_the_alternate_screen_the_view_clears_at_the_end() {
        let caps = Capabilities::from_term(Some("linux"));
        let (enter, leave) = sequences(caps, false);
        let (enter, leave) = (text(&enter), text(&leave));
        assert!(!enter.contains("1049") && !leave.contains("1049"));
        assert!(leave.contains("\x1b[2J"), "{leave:?}");
        assert!(!enter.contains("\x1b[?1000h") && !leave.contains("\x1b[?1000l"));
    }
}
