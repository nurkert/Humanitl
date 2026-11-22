//! Die Signale, solange ein Bildschirm steht (GitHub #6).
//!
//! # Was hier ankommt und was daraus wird
//!
//! Im Rohmodus erzeugt das Terminal selbst keine Signale mehr: `Ctrl+C` und
//! `Ctrl+Z` kommen als Tasten an und gehen an [`super::screen`]. Signale
//! schicken dann nur noch andere: `SIGTERM` von `kill` oder systemd, `SIGHUP`
//! vom Terminal, das geschlossen wird, `SIGINT` von einem Skript, `SIGTSTP`
//! von einer Shell oder einem Fenstermanager, `SIGCONT` nach dem Anhalten.
//!
//! Diese Datei beantwortet keines davon selbst. Sie merkt sich nur, dass eines
//! kam, und die Schleife des Bildschirms fragt bei jedem Schritt nach
//! ([`take`]). Die Entscheidung bleibt damit beim Befehl, wie in `tty.rs`
//! beschrieben: Ein `humanitl go`, das unter einem Bildschirm eine Sitzung
//! hält, muss sie erst trennen oder stoppen, bevor es endet. Ein Handler, der
//! das Terminal zurückgibt und den Prozess beendet, wäre schneller als diese
//! RPC.
//!
//! # Einmal je Prozess, nie wieder abgebaut
//!
//! `signal-hook` gibt beim Abmelden die gewöhnliche Wirkung eines Signals
//! nicht zurück: Ein abgemeldetes `SIGTERM` würde danach schlicht ignoriert.
//! Deshalb melden sich die Merker beim ersten Bildschirm an und bleiben
//! stehen, so wie die Signale von `tokio` in `cmd/run.rs` auch. Ein Befehl,
//! der nach seinem Bildschirm weiterläuft, hört deshalb selbst auf seine
//! Signale (`tokio::signal`), wie er es ohne Bildschirm auch täte.
//!
//! Für `SIGTSTP` gilt eine Ausnahme: Außerhalb eines Bildschirms soll es den
//! Prozess anhalten wie immer, also läuft dort die gewöhnliche Wirkung
//! ([`signal_hook::flag::register_conditional_default`] mit dem Zähler
//! [`screen_enter`] und [`screen_leave`]). Unter einem Bildschirm hält erst
//! die Schleife an, nachdem sie das Terminal zurückgegeben hat. Angemeldet
//! wird vor der ersten Änderung am Terminal und abgemeldet erst nach der
//! letzten; ein `SIGTSTP` in einem der beiden Fenster hielte sonst mit rohem
//! Terminal an.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use signal_hook::consts::{SIGCONT, SIGHUP, SIGINT, SIGTERM, SIGTSTP};

/// Ein Signal, auf das ein Bildschirm antworten muss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// `SIGTERM`: beenden.
    Terminate,
    /// `SIGHUP`: das Terminal ist weg oder soll als weg gelten.
    HangUp,
    /// `SIGINT`: abbrechen.
    Interrupt,
    /// `SIGTSTP`: anhalten, nachdem das Terminal zurückgegeben ist.
    Stop,
    /// `SIGCONT`: weiter; der Bildschirm wird neu gezeichnet.
    Continue,
}

impl Signal {
    /// Die Nummer des Signals, für den Exit-Code `128 + n`.
    #[must_use]
    pub const fn number(self) -> i32 {
        match self {
            Self::Terminate => SIGTERM,
            Self::HangUp => SIGHUP,
            Self::Interrupt => SIGINT,
            Self::Stop => SIGTSTP,
            Self::Continue => SIGCONT,
        }
    }

    /// Alle, in der Reihenfolge, in der [`take`] sie prüft: Ende vor Anhalten,
    /// damit ein `SIGTERM` neben einem `SIGTSTP` nicht erst anhält.
    const ALL: [Self; 5] = [
        Self::Terminate,
        Self::HangUp,
        Self::Interrupt,
        Self::Stop,
        Self::Continue,
    ];
}

/// Je Signal ein Merker, und der Merker, ob gerade ein Bildschirm steht.
struct Flags {
    /// In der Reihenfolge von [`Signal::ALL`].
    seen: [Arc<AtomicBool>; 5],
    /// Wahr, solange kein Bildschirm steht; dann wirkt `SIGTSTP` gewöhnlich.
    idle: Arc<AtomicBool>,
}

/// Die Merker dieses Prozesses; `None`, wenn die Anmeldung scheiterte.
static FLAGS: OnceLock<Option<Flags>> = OnceLock::new();

/// Meldet die Merker an, beim ersten Aufruf je Prozess.
///
/// `false`, wenn das nicht ging; dann gibt es keinen Bildschirm, denn einer,
/// der `SIGTERM` nicht bemerkt, könnte das Terminal nicht zurückgeben.
pub fn install() -> bool {
    FLAGS.get_or_init(register).is_some()
}

/// Die eigentliche Anmeldung.
fn register() -> Option<Flags> {
    let seen = Signal::ALL.map(|_| Arc::new(AtomicBool::new(false)));
    let idle = Arc::new(AtomicBool::new(true));
    for (signal, flag) in Signal::ALL.iter().zip(&seen) {
        signal_hook::flag::register(signal.number(), Arc::clone(flag)).ok()?;
    }
    signal_hook::flag::register_conditional_default(SIGTSTP, Arc::clone(&idle)).ok()?;
    Some(Flags { seen, idle })
}

/// Wie viele Wächter gerade einen Bildschirm halten oder halten wollen.
///
/// Ein Zähler und kein Schalter: Zwei Threads, die zugleich einen Bildschirm
/// öffnen wollen, melden sich beide an, und der, der verliert, darf beim
/// Abmelden den Bildschirm des anderen nicht für beendet erklären.
static SCREENS: Mutex<usize> = Mutex::new(0);

/// Ein Wächter meldet sich an, **bevor** er das Terminal anfasst: Ab hier
/// hält `SIGTSTP` nicht mehr sofort an, sondern erst, nachdem die Schleife
/// das Terminal zurückgegeben hat.
pub fn screen_enter() {
    let mut screens = SCREENS.lock().unwrap_or_else(PoisonError::into_inner);
    *screens += 1;
    store_idle(*screens == 0);
}

/// Ein Wächter meldet sich ab, **nachdem** das Terminal ganz zurückgegeben
/// ist. Ohne Bildschirm hält `SIGTSTP` den Prozess wie gewohnt an.
pub fn screen_leave() {
    let mut screens = SCREENS.lock().unwrap_or_else(PoisonError::into_inner);
    *screens = screens.saturating_sub(1);
    store_idle(*screens == 0);
}

/// Schreibt den Merker, den die gewöhnliche Wirkung von `SIGTSTP` liest.
fn store_idle(idle: bool) {
    if let Some(Some(flags)) = FLAGS.get() {
        flags.idle.store(idle, Ordering::SeqCst);
    }
}

/// Vergisst ein `SIGTSTP` oder `SIGCONT`, das vor diesem Bildschirm kam.
///
/// Ein `SIGTSTP` außerhalb eines Bildschirms hat den Prozess schon angehalten
/// und steht trotzdem im Merker; der nächste Bildschirm hielte sonst sofort
/// noch einmal an. Ein `SIGTERM` von vorher bleibt stehen: Es ist nicht
/// beantwortet, und der Bildschirm endet daran bei seinem ersten Schritt.
pub fn forget_stops() {
    if let Some(Some(flags)) = FLAGS.get() {
        for (signal, flag) in Signal::ALL.iter().zip(&flags.seen) {
            if matches!(signal, Signal::Stop | Signal::Continue) {
                flag.store(false, Ordering::SeqCst);
            }
        }
    }
}

/// Das dringendste Signal, das seit dem letzten Aufruf kam, und vergisst es.
#[must_use]
pub fn take() -> Option<Signal> {
    let Some(Some(flags)) = FLAGS.get() else {
        return None;
    };
    Signal::ALL
        .iter()
        .zip(&flags.seen)
        .find(|(_, flag)| flag.swap(false, Ordering::SeqCst))
        .map(|(signal, _)| *signal)
}

#[cfg(test)]
mod tests {
    use std::sync::PoisonError;

    use super::{SCREENS, Signal, screen_enter, screen_leave};

    /// Der Exit-Code nach einem Signal ist `128 + n`; die Nummern sind die
    /// des Systems, nicht geraten.
    #[test]
    fn the_numbers_are_the_system_ones() {
        assert_eq!(Signal::Terminate.number(), 15);
        assert_eq!(Signal::HangUp.number(), 1);
        assert_eq!(Signal::Interrupt.number(), 2);
    }

    /// Zwei Wächter angemeldet, einer ab: Es steht noch ein Bildschirm, und
    /// `SIGTSTP` darf nicht sofort anhalten. Der Verlierer eines Wettlaufs
    /// erklärt den Bildschirm des Gewinners nicht für beendet.
    #[test]
    fn a_leaving_guard_does_not_end_another_screen() {
        let count = || *SCREENS.lock().unwrap_or_else(PoisonError::into_inner);
        let start = count();
        screen_enter();
        screen_enter();
        screen_leave();
        assert_eq!(count(), start + 1, "one screen still stands");
        screen_leave();
        assert_eq!(count(), start);
    }
}
