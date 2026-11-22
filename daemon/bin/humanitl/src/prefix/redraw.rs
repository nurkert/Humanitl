//! Den Agenten nach einer Ansicht neu zeichnen lassen (GitHub #11, ADR-0024
//! Punkt 9).
//!
//! Der Daemon hält kein Bild des Schirms, nur einen Rückstand roher Bytes.
//! Ein Agent zeichnet neu, wenn sich die Größe seines Terminals ändert
//! (`SIGWINCH`). Nach einer Ansicht bittet der Anschluss deshalb über die
//! `Terminal`-RPC um eine Zeile weniger und danach wieder um die echte
//! Größe, einmal je Rückkehr.
//!
//! # Warum nicht dieselbe Größe noch einmal
//!
//! Der Kernel schickt `SIGWINCH` nur, wenn sich die Größe wirklich ändert;
//! dieselbe Größe ist kein Signal (GitHub #63, Punkt 2). Und der Daemon
//! reicht höchstens eine Größe je [`RESIZE_INTERVAL`] weiter, die letzte
//! gewinnt: Kämen beide Größen zu schnell hintereinander, bliebe nur die
//! echte übrig, und der Agent sähe keine Änderung. Zwischen beiden wartet
//! [`Redraw`] deshalb [`GAP`].
//!
//! # Warum die Schleife nie wartet
//!
//! Liest die Sitzung ihre Eingabe nicht, ist der Kanal zum Daemon voll. Ein
//! `send(..).await` in der Schleife hielte dann auch die Taste zum Abhängen
//! fest (Review zu GitHub #10), und ein `try_send`, das bei vollem Kanal
//! aufgibt, verlöre die Bitte (GitHub #63, Punkt 2). [`Redraw`] hält die
//! Bitte stattdessen, bis Platz ist: [`Redraw::ready`] ist ein Zweig der
//! Schleife, der erst fertig wird, wenn der Kanal eine Nachricht nimmt.
//!
//! Die Größe zum Schluss ist die, die das eigene Terminal dann hat; eine
//! Größenänderung während der Ansicht erreicht den Agenten so spätestens
//! mit der Rückkehr (Punkt 3).

use std::time::Duration;

use humanitl_ipc::terminal::RESIZE_INTERVAL;
use humanitl_ipc::v1;
use tokio::sync::mpsc::{Permit, Sender};
use tokio::time::Instant;

/// Wie lange zwischen der kleineren und der echten Größe gewartet wird:
/// zweimal die Drossel des Daemons, damit die kleinere Größe nie von der
/// echten überholt wird.
pub const GAP: Duration = RESIZE_INTERVAL.saturating_mul(2);

/// Wo die Bitte um ein neues Bild steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Nichts zu tun.
    Idle,
    /// Die kleinere Größe wartet auf Platz im Kanal.
    Shrink,
    /// Die kleinere Größe ist geschickt; bis zu diesem Zeitpunkt wird
    /// gewartet.
    Waiting(Instant),
    /// Die echte Größe wartet auf Platz im Kanal.
    Grow,
}

/// Die Bitte um ein neues Bild, bis sie ganz geschickt ist.
#[derive(Debug)]
pub struct Redraw {
    /// Wo sie steht.
    state: State,
    /// Die Pause zwischen beiden Größen.
    gap: Duration,
}

impl Default for Redraw {
    fn default() -> Self {
        Self::new(GAP)
    }
}

impl Redraw {
    /// Keine Bitte; zwischen beiden Größen liegt `gap`.
    #[must_use]
    pub const fn new(gap: Duration) -> Self {
        Self {
            state: State::Idle,
            gap,
        }
    }

    /// Bittet um ein neues Bild. Läuft schon eine Bitte, beginnt sie von
    /// vorn und wird nicht verdoppelt: einmal je Rückkehr.
    pub const fn request(&mut self) {
        self.state = State::Shrink;
    }

    /// Ob noch etwas aussteht.
    #[cfg(test)]
    #[must_use]
    pub const fn pending(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Wird fertig, sobald die nächste Größe geschickt werden kann: nach der
    /// Pause und wenn der Kanal Platz hat. Ohne Bitte nie. `None`, wenn der
    /// Kanal zu ist.
    ///
    /// Wartet nur in sich selbst, nie in der Schleife: Als Zweig von
    /// `select!` lässt sie jeden anderen Zweig vor.
    pub async fn ready<'a>(
        &self,
        tx: &'a Sender<v1::TerminalInput>,
    ) -> Option<Permit<'a, v1::TerminalInput>> {
        match self.state {
            State::Idle => std::future::pending().await,
            State::Waiting(until) => tokio::time::sleep_until(until).await,
            State::Shrink | State::Grow => {}
        }
        tx.reserve().await.ok()
    }

    /// Schickt die nächste Größe über `permit`, für ein Terminal der Größe
    /// `size` (Spalten, Zeilen). Ein Terminal ohne Größe bekommt keine
    /// Bitte. Während der Pause geht nichts; der Platz im Kanal wird wieder
    /// frei.
    fn send(&mut self, permit: Permit<'_, v1::TerminalInput>, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 {
            self.state = State::Idle;
            return;
        }
        let next = match self.state {
            State::Idle | State::Waiting(_) => return,
            State::Shrink => {
                self.state = State::Waiting(Instant::now() + self.gap);
                (size.0, shrunk(size.1))
            }
            State::Grow => {
                self.state = State::Idle;
                size
            }
        };
        permit.send(resize(next));
    }

    /// Nach der Pause ist die echte Größe dran.
    fn due(&mut self) {
        if let State::Waiting(until) = self.state
            && Instant::now() >= until
        {
            self.state = State::Grow;
        }
    }

    /// Schickt, was [`Redraw::ready`] möglich gemacht hat: erst prüfen, ob
    /// die Pause um ist, dann die nächste Größe.
    pub fn step(&mut self, permit: Permit<'_, v1::TerminalInput>, size: (u32, u32)) {
        self.due();
        self.send(permit, size);
    }

    /// Gibt eine Bitte auf, etwa weil der Kanal zu ist.
    pub const fn cancel(&mut self) {
        self.state = State::Idle;
    }
}

/// Eine Zeile weniger; ein Terminal mit einer Zeile bekommt eine mehr, denn
/// null hieße „nicht gesetzt".
const fn shrunk(rows: u32) -> u32 {
    if rows > 1 { rows - 1 } else { rows + 1 }
}

/// Eine Fenstergröße als Nachricht.
#[must_use]
pub fn resize((cols, rows): (u32, u32)) -> v1::TerminalInput {
    v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Resize(
            v1::terminal_input::Resize { cols, rows },
        )),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use std::time::Duration;

    use humanitl_ipc::v1;
    use tokio::sync::mpsc;

    use super::Redraw;

    /// Die Größe einer Nachricht, oder `None`.
    fn size(message: &v1::TerminalInput) -> Option<(u32, u32)> {
        match &message.input {
            Some(v1::terminal_input::Input::Resize(resize)) => Some((resize.cols, resize.rows)),
            _ => None,
        }
    }

    /// Ein Schritt der Schleife: warten, bis [`Redraw::ready`] fertig ist,
    /// höchstens `within`, dann schicken. `false`, wenn nichts fertig wurde.
    async fn turn(
        redraw: &mut Redraw,
        tx: &mpsc::Sender<v1::TerminalInput>,
        within: Duration,
    ) -> bool {
        match tokio::time::timeout(within, redraw.ready(tx)).await {
            Ok(Some(permit)) => {
                redraw.step(permit, (80, 24));
                true
            }
            Ok(None) => panic!("the channel closed"),
            Err(_) => false,
        }
    }

    /// Eine Rückkehr: erst eine Zeile weniger, nach der Pause die echte
    /// Größe, danach nichts mehr. Zwei Bitten hintereinander werden eine.
    #[tokio::test]
    async fn one_row_less_then_the_real_size_once_per_return() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut redraw = Redraw::new(Duration::from_millis(30));
        redraw.request();
        redraw.request();
        let started = tokio::time::Instant::now();
        assert!(turn(&mut redraw, &tx, Duration::from_secs(5)).await);
        assert!(turn(&mut redraw, &tx, Duration::from_secs(5)).await);
        assert!(
            started.elapsed() >= Duration::from_millis(30),
            "the real size came before the gap"
        );
        assert!(!redraw.pending());
        assert!(
            !turn(&mut redraw, &tx, Duration::from_millis(50)).await,
            "nothing more after one return"
        );
        let first = rx.try_recv().expect("the smaller size");
        let second = rx.try_recv().expect("the real size");
        assert_eq!(size(&first), Some((80, 23)));
        assert_eq!(size(&second), Some((80, 24)));
        assert!(rx.try_recv().is_err(), "exactly two resizes");
    }

    /// Ein voller Kanal hält die Schleife nicht fest und verliert die Bitte
    /// nicht: Solange kein Platz ist, wird der Zweig nicht fertig; sobald
    /// der Kanal Platz hat, kommen beide Größen an. Mutationsbeweis: die
    /// Bitte bei vollem Kanal aufgeben (`try_reserve` statt `reserve`, das
    /// Ergebnis verworfen) macht ihn rot.
    #[tokio::test]
    async fn a_full_channel_keeps_the_redraw_until_there_is_room() {
        let (tx, mut rx) = mpsc::channel(1);
        tx.try_send(super::resize((1, 1)))
            .expect("fills the channel");
        let mut redraw = Redraw::new(Duration::from_millis(10));
        redraw.request();
        assert!(
            !turn(&mut redraw, &tx, Duration::from_millis(100)).await,
            "nothing fits into a full channel"
        );
        assert!(redraw.pending(), "the request is kept");
        let _ = rx.recv().await;
        assert!(turn(&mut redraw, &tx, Duration::from_secs(5)).await);
        assert_eq!(rx.recv().await.as_ref().and_then(size), Some((80, 23)));
        assert!(turn(&mut redraw, &tx, Duration::from_secs(5)).await);
        assert_eq!(rx.recv().await.as_ref().and_then(size), Some((80, 24)));
    }

    /// Ein Terminal mit einer Zeile bekommt eine mehr statt null.
    #[test]
    fn one_row_grows_instead_of_shrinking() {
        assert_eq!(super::shrunk(1), 2);
        assert_eq!(super::shrunk(24), 23);
    }
}
