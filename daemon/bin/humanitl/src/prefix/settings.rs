//! Die Einstellungen im Terminal der Sitzung, über dem Agenten (GitHub #11,
//! ADR-0024 Punkt 9).
//!
//! Präfix und `,` (oder `Ctrl+,` im kitty-Protokoll) öffnen den Bildschirm
//! aus `humanitl settings` ([`crate::tui::settings`]) im laufenden
//! Anschluss; `Esc` oder `q` kehren zurück, und der Agent ist wieder zu
//! sehen und zu bedienen. Der Ablauf, verteilt auf die Teile des
//! Anschlusses:
//!
//! 1. **Umschalten.** Der Leser der Tastatur hält an (`prefix::input`), die
//!    Ausgabe des Agenten wartet im Client (`prefix::view`, höchstens
//!    [`super::view::HELD_LIMIT`], darüber verworfen und gemerkt), und der
//!    eigene Schirm steht ([`super::view::AgentView::open_screen`]). Dann
//!    läuft der Bildschirm hier, auf einem Faden, der blockieren darf; die
//!    Schleife des Anschlusses nimmt währenddessen weiter die Ausgabe des
//!    Agenten an.
//! 2. **Zurückkehren.** Den eigenen Schirm verlassen, die zurückgehaltene
//!    Ausgabe schreiben (oder nach einem Überlauf nicht), und den Agenten
//!    über die `Terminal`-RPC neu zeichnen lassen: eine Zeile weniger und
//!    zurück auf die echte Größe ([`super::redraw`]).
//!
//! Der Agent läuft in dieser Zeit weiter. Was der Daemon über gehaltene
//! Anfragen in den Strom des Terminals schreibt (`TerminalOutput.notice`),
//! steht rechts in der Fußzeile; der Bildschirm führt keine eigene Liste
//! (ADR-018).
//!
//! Was den Bildschirm sonst verlassen will (`config.toml` im `$EDITOR`,
//! Neustart des Daemons), geht in der Sitzung nicht: Der Editor bräuchte das
//! Terminal, das der Agent gerade hat, und ein Neustart beendete die
//! Sitzung. Die Zeile sagt dann, dass `humanitl settings` in einem anderen
//! Terminal das kann.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crossterm::event::Event;
use humanitl_ipc::client::Client;
use ratatui::Frame;

use crate::cmd::settings::Worker;
use crate::tui::Theme;
use crate::tui::backend::{TermOut, TtyBackend};
use crate::tui::screen::{Flow, Screen, discard_pending_events, run_in_session};
use crate::tui::settings::{self, Exit, SettingsScreen, actions};

/// Wie lange das Ende des Anschlusses höchstens auf den Faden des
/// Bildschirms wartet: Er sieht alle 50 ms nach, ob er enden soll.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// Was rechts in der Fußzeile steht, solange der Daemon nichts gemeldet hat.
pub const RUNNING: &str = "agent keeps running";

/// Die Zeile an Editor und Neustart, wenn sie im Terminal der Sitzung
/// gewählt werden.
pub const ELSEWHERE: &str =
    "Not inside a session: run humanitl settings in another terminal for this.";

/// Der letzte Hinweis des Daemons, geteilt zwischen Schleife und Bildschirm.
type Notice = Arc<Mutex<Option<String>>>;

/// Ein laufender Bildschirm der Einstellungen über dem Agenten.
pub struct SettingsView {
    /// Der Faden des Bildschirms.
    task: tokio::task::JoinHandle<()>,
    /// Wahr, sobald der Bildschirm enden soll.
    stop: Arc<AtomicBool>,
    /// Der letzte Hinweis des Daemons.
    notice: Notice,
    /// Die Leitung zum Daemon; endet mit dem Bildschirm.
    _worker: Worker,
}

impl SettingsView {
    /// Startet den Bildschirm an `client`, der Verbindung des Anschlusses,
    /// mit dem letzten Hinweis des Daemons in der Fußzeile, wenn es einen
    /// gab. `None` ohne Terminal auf `stdin` und `stdout`: Dann gibt es
    /// keinen Bildschirm, und der Anschluss lässt den Leser gleich
    /// weiterlaufen.
    #[must_use]
    pub fn start(client: Client, theme: Theme, notice: Option<String>) -> Option<Self> {
        if !crate::tui::available() {
            return None;
        }
        let (link, worker) = crate::cmd::settings::spawn(client);
        let screen = SettingsScreen::new(link, None);
        let stop = Arc::new(AtomicBool::new(false));
        let notice: Notice = Arc::new(Mutex::new(notice));
        let task = tokio::task::spawn_blocking({
            let stop = Arc::clone(&stop);
            let notice = Arc::clone(&notice);
            move || show(screen, &notice, &stop, theme)
        });
        Some(Self {
            task,
            stop,
            notice,
            _worker: worker,
        })
    }

    /// Ein Hinweis des Daemons aus dem Strom des Terminals, für die
    /// Fußzeile.
    pub fn notice(&self, text: &str) {
        *self.notice.lock().unwrap_or_else(PoisonError::into_inner) = Some(text.to_owned());
    }

    /// Wird fertig, wenn der Bildschirm zu Ende ist.
    pub async fn finished(&mut self) {
        let _ = (&mut self.task).await;
    }

    /// Beendet den Bildschirm und wartet, bis sein Faden nicht mehr zeichnet;
    /// höchstens [`STOP_GRACE`].
    pub async fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = tokio::time::timeout(STOP_GRACE, &mut self.task).await;
    }
}

/// Zeigt den Bildschirm, bis er endet; auf dem Faden, der blockieren darf.
///
/// Ein Fehler des Terminals beendet nur die Ansicht; der Anschluss kehrt
/// danach zum Agenten zurück wie nach `Esc`.
fn show(screen: SettingsScreen, notice: &Notice, stop: &AtomicBool, theme: Theme) {
    let Ok(out) = rustix::stdio::stdout().try_clone_to_owned() else {
        return;
    };
    let out = TermOut::new(out);
    let muted = out.mute_switch();
    let Ok(mut terminal) = ratatui::Terminal::new(TtyBackend::new(out)) else {
        return;
    };
    let mut screen = InSession::new(screen, Arc::clone(notice));
    let _ = run_in_session(&mut terminal, &mut screen, theme, settings::MIN, stop);
    // ratatui zeigt beim Fallenlassen den Cursor wieder; welchen Cursor der
    // Agent will, stellt der Anschluss beim Zurückkehren her.
    muted.store(true, Ordering::SeqCst);
    drop(terminal);
    discard_pending_events();
}

/// Der Bildschirm der Einstellungen im Terminal einer Sitzung: mit dem
/// Stand in der Fußzeile und ohne Editor und Neustart.
struct InSession {
    /// Der Bildschirm aus `humanitl settings`.
    inner: SettingsScreen,
    /// Der letzte Hinweis des Daemons.
    notice: Notice,
    /// Was gerade in der Fußzeile steht.
    shown: Option<String>,
}

impl InSession {
    fn new(mut inner: SettingsScreen, notice: Notice) -> Self {
        inner.set_status(Some(RUNNING.to_owned()));
        Self {
            inner,
            notice,
            shown: None,
        }
    }
}

impl Screen for InSession {
    fn draw(&mut self, frame: &mut Frame<'_>, theme: Theme) {
        self.inner.draw(frame, theme);
    }

    fn on_event(&mut self, event: &Event) -> Flow {
        if self.inner.on_event(event) == Flow::Continue {
            return Flow::Continue;
        }
        match self.inner.take_exit() {
            Exit::Quit => Flow::Done,
            Exit::Editor(_) => {
                self.inner.note(actions::CONFIG_FILE, ELSEWHERE.to_owned());
                Flow::Continue
            }
            Exit::Restart => {
                self.inner.note(actions::RESTART, ELSEWHERE.to_owned());
                Flow::Continue
            }
        }
    }

    fn changed(&mut self) -> bool {
        let latest = self
            .notice
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let noticed = latest.is_some() && latest != self.shown;
        if noticed {
            self.shown.clone_from(&latest);
            self.inner.set_status(latest);
        }
        // Beide fragen: Die Antworten des Daemons nimmt der Bildschirm auch
        // dann, wenn ohnehin neu gezeichnet wird.
        let replies = self.inner.changed();
        noticed || replies
    }
}
