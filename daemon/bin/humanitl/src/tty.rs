//! Das Terminal dieses Prozesses: Rohmodus, Größe, und die Rückgabe auf jedem
//! Ausgang (HUM-042, HUM-067).
//!
//! # Warum das hier steht und nicht bei einem Befehl
//!
//! Zwei Befehle setzen das Terminal in den Rohmodus: `humanitl sandbox attach`
//! reicht jede Taste an den Agenten weiter, und `humanitl run --ask terminal`
//! liest einzelne Tasten für seinen Prompt. Beide müssen es hinterher
//! zurückgeben, und zwar auf **jedem** Weg hinaus, nicht nur auf dem
//! gewöhnlichen: Ein Terminal, das im Rohmodus zurückbleibt, zeigt kein Echo
//! mehr und nimmt `Ctrl+C` nicht mehr an -- die Shell des Menschen sieht dann
//! aus wie abgestürzt, und `reset` weiß nicht jeder.
//!
//! Zwei Wege hinaus deckt [`RawMode`] selbst ab:
//!
//! 1. **Gewöhnliches Ende und jeder Fehlerpfad**: `Drop`.
//! 2. **Panik**: ein Panik-Hook, der vor dem gewöhnlichen läuft. Ohne ihn
//!    liefe zwar auch `Drop` beim Abwickeln, aber nicht bei
//!    `panic = "abort"` und nicht, wenn die Panik in einem anderen Thread
//!    steht.
//!
//! **Den dritten -- `SIGTERM`, `SIGHUP`, `SIGINT` -- deckt der Befehl ab, und
//! zwar er allein.** Ein Signalhandler hier, der das Terminal zurückgibt und
//! den Prozess beendet, wäre schneller als die RPC, mit der `humanitl run`
//! seine Sitzung stoppt: Der Prozess wäre weg, die Sandbox liefe weiter, und
//! niemand könnte sie noch beenden. Deshalb hört jeder Befehl selbst auf seine
//! Signale, beendet, was er begonnen hat, lässt `RawMode` fallen und endet mit
//! `128 + n`. [`restore_now`] steht für den Fall, dass ein Befund noch auf ein
//! gewöhnliches Terminal soll, bevor das passiert.
//!
//! Die Einstellungen des Terminals liegen in einem globalen Platz: Ein
//! Panik-Hook lebt länger als jeder Rahmen.

use std::os::fd::BorrowedFd;
use std::sync::Mutex;
use std::sync::OnceLock;

use rustix::termios::{OptionalActions, Termios, tcgetattr, tcgetwinsize, tcsetattr};

/// Die Größe, mit der gerechnet wird, wenn das Terminal keine nennt.
pub const FALLBACK_SIZE: (u32, u32) = (80, 24);

/// Die Einstellungen, die zurückgeschrieben werden müssen.
///
/// Global, weil ein Signalhandler und ein Panik-Hook sie brauchen und beide
/// nichts ausleihen können. `Mutex` statt `RwLock`, weil hier nie mehr als ein
/// Rahmen schreibt.
static SAVED: OnceLock<Mutex<Option<Termios>>> = OnceLock::new();

/// Der Platz für die gesicherten Einstellungen.
fn saved() -> &'static Mutex<Option<Termios>> {
    SAVED.get_or_init(|| Mutex::new(None))
}

/// Die eigene Eingabe: Deskriptor `0`, mit der Lebensdauer des Prozesses.
fn stdin_fd() -> BorrowedFd<'static> {
    rustix::stdio::stdin()
}

/// Schreibt die gesicherten Einstellungen zurück, wenn es welche gibt.
///
/// Idempotent und ohne Zuteilung: Sie läuft auch aus einem Signalhandler.
fn restore() {
    let guard = saved()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(termios) = guard.as_ref() {
        let _ = tcsetattr(stdin_fd(), OptionalActions::Flush, termios);
    }
}

/// Schreibt die gesicherten Einstellungen jetzt zurück.
///
/// Für den Weg, auf dem ein Befehl sein Signal selbst beantwortet und danach
/// noch etwas Gewöhnliches schreibt.
pub fn restore_now() {
    restore();
}

/// Der Rohmodus dieses Terminals, solange der Befehl läuft.
pub struct RawMode {
    /// Wahr, wenn dieser Rahmen den Modus gesetzt hat und ihn zurückgeben muss.
    entered: bool,
}

impl RawMode {
    /// Setzt den Rohmodus; `None`, wenn die Eingabe kein Terminal ist (eine
    /// Pipe zum Beispiel).
    ///
    /// Mit dem ersten Rohmodus dieses Prozesses entsteht auch der Panik-Hook.
    /// Er bleibt danach stehen: Er tut nichts, solange nichts gesichert ist,
    /// und ein Hook, den jemand wieder abbaut, wäre ein Fenster, in dem eine
    /// Panik das Terminal behielte. Die Signale gehören dem Befehl (siehe
    /// oben).
    #[must_use]
    pub fn enter() -> Option<Self> {
        let current = tcgetattr(stdin_fd()).ok()?;
        let mut raw = current.clone();
        raw.make_raw();
        tcsetattr(stdin_fd(), OptionalActions::Flush, &raw).ok()?;
        {
            let mut guard = saved()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *guard = Some(current);
        }
        install_hooks();
        Some(Self { entered: true })
    }

    /// Gibt das Terminal jetzt zurück, statt beim Fallenlassen.
    ///
    /// Für den Weg, auf dem danach noch etwas Gewöhnliches geschrieben wird:
    /// Ein Diagnostic-Block im Rohmodus steht ohne Zeilenumbrüche da.
    pub fn leave(&mut self) {
        if !self.entered {
            return;
        }
        self.entered = false;
        restore();
        let mut guard = saved()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = None;
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        self.leave();
    }
}

/// Der Panik-Hook, genau einmal je Prozess.
fn install_hooks() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
    });
}

/// Die Größe dieses Terminals, oder [`FALLBACK_SIZE`] ohne Terminal.
#[must_use]
pub fn window_size() -> (u32, u32) {
    tcgetwinsize(stdin_fd()).map_or(FALLBACK_SIZE, |size| {
        let cols = if size.ws_col == 0 {
            FALLBACK_SIZE.0
        } else {
            u32::from(size.ws_col)
        };
        let rows = if size.ws_row == 0 {
            FALLBACK_SIZE.1
        } else {
            u32::from(size.ws_row)
        };
        (cols, rows)
    })
}
