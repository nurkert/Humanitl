//! Bildschirme im Terminal: das Fundament für Einstellungen, Erststart,
//! Interceptor und die Hilfe der Präfixtaste (GitHub #6, ADR-0024 Punkt 6).
//!
//! # Was hier steht
//!
//! - [`guard`]: [`TerminalGuard`] setzt Rohmodus, alternativen Bildschirm,
//!   Cursor, Maus und Einfügeklammern und gibt alles auf jedem Weg hinaus
//!   zurück, Panik eingeschlossen.
//! - [`signals`]: `SIGTERM`, `SIGHUP`, `SIGINT`, `SIGTSTP`/`SIGCONT`, als
//!   Merker für die Schleife.
//! - [`size`]: Mindestgröße und Fähigkeiten aus `TERM`, sonst `TERM_003`.
//! - [`frame`]: Kopf, Fuß mit Tasten, Suche, und die Farben nach `NO_COLOR`.
//! - [`screen`]: die Schleife, in der jeder Bildschirm läuft.
//! - [`backend`]: das Backend von ratatui mit der Größe von `stdin`.
//! - [`settings`]: der Bildschirm `humanitl settings` (GitHub #7).
//! - [`first_start`]: der Erststart-Bildschirm von `humanitl go` (GitHub #9).
//!
//! # Ohne Terminal
//!
//! Ohne Terminal auf `stdin` und `stdout` gibt es keinen Bildschirm
//! ([`available`], [`Unavailable::NoTerminal`]), und `/dev/tty` wird nie
//! geöffnet: crossterm täte das sonst von selbst und ginge an einer Pipe
//! vorbei. Jeder Bildschirm nennt in seinem Issue den Weg ohne ihn.
//!
//! # Was ein Bildschirm nicht tut
//!
//! Er entscheidet nichts und hält keine eigene Liste (ADR-0018): Er zeigt
//! Antworten von RPCs und schreibt über RPCs. Die Ausgabe des Agenten geht
//! nie durch diese Module; sie bleibt beim Filter `FullScreen`.

pub mod backend;
pub mod first_start;
pub mod frame;
pub mod guard;
pub mod screen;
pub mod settings;
pub mod signals;
pub mod size;

pub use frame::{Key, ScreenFrame, Search, Theme};
pub use guard::{Options, TerminalGuard, Unavailable};
pub use screen::{Flow, Outcome, Screen, run};
pub use size::Cells;

/// Ob ein Bildschirm überhaupt in Frage kommt: `stdin` und `stdout` sind
/// beide ein Terminal.
///
/// `stdin`, weil dort die Tasten herkommen und crossterm sonst `/dev/tty`
/// öffnete; `stdout`, weil dorthin gezeichnet wird und ein Bildschirm in
/// einer Datei nur Steuerfolgen wäre.
#[must_use]
pub fn available() -> bool {
    rustix::termios::isatty(rustix::stdio::stdin())
        && rustix::termios::isatty(rustix::stdio::stdout())
}
