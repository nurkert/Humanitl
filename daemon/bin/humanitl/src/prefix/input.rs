//! Der Leser der Tastatur eines Anschlusses (HUM-279, GitHub #10, #11).
//!
//! Er reicht jede Taste an den Agenten weiter, bis auf die Präfixtaste und
//! ihre Tabelle ([`PrefixFilter`]). Was die Tabelle auslöst, meldet er über
//! einen eigenen Kanal an die Schleife des Anschlusses.
//!
//! # Ein Faden mit `poll`, damit er die Tastatur loslassen kann
//!
//! Öffnet die Tabelle einen eigenen Bildschirm (die Einstellungen,
//! [`takes_screen`]), liest ab da dieser Bildschirm die Tastatur. Zwei Leser
//! an einem Terminal teilten sich die Tasten zufällig, und ein Teil davon
//! ginge an den Agenten. Deshalb hält der Leser nach diesem Ereignis selbst
//! an, bevor er wieder liest, und wartet, bis die Schleife ihn weiterlaufen
//! lässt ([`Control::resume`]).
//!
//! Ein blockierendes `read` (auch das von `tokio::io::stdin`) ließe sich
//! dabei nicht zurücknehmen: Es läge schon auf dem nächsten Byte und nähme
//! es der Ansicht weg. Der Leser ist deshalb ein eigener Faden, der mit
//! `poll` und kurzer Frist auf Eingabe wartet und erst liest, wenn etwas da
//! ist. Dieselbe Frist lässt ihn nach dem Ende des Anschlusses aufhören
//! ([`Control`] beim Fallenlassen); ein Byte, das danach kommt, gehört dem,
//! der das Terminal dann hat.
//!
//! # Lesen und Weiterreichen sind getrennt
//!
//! Liest die Sitzung ihre Eingabe nicht, staut sich ein eingefügter Text bis
//! in den Kanal zum Daemon, und ein Leser, der selbst in `send` wartete,
//! sähe die Taste zum Abhängen nie: Der Mensch säße fest. Deshalb liest der
//! Leser immer weiter und gibt die Bytes an eine eigene Warteschlange
//! ([`Backlog`]); der Weiterreicher leert sie in den Kanal. Was über
//! [`BACKLOG_LIMIT`] hinausgeht, fällt weg -- die Sitzung nimmt in diesem
//! Moment ohnehin nichts an.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use humanitl_ipc::v1;
use tokio::sync::mpsc;

use super::Keys;
use super::keys::{KeyEvent, PrefixFilter, has_ctrl_c, takes_screen};

/// Wie viel Eingabe auf einmal gelesen wird.
const INPUT_CHUNK: usize = 4096;

/// Wie lange der Leser höchstens auf Eingabe wartet, bevor er nachsieht, ob
/// der Anschluss noch besteht.
const POLL_TICK: Duration = Duration::from_millis(100);

/// Wie viele Bytes zwischen Tastatur und Kanal höchstens warten.
///
/// Ein MiB ist mehr, als ein Mensch auf einmal einfügt, und wenig genug, dass
/// eine Sitzung, die nicht liest, den Speicher nicht füllt.
pub const BACKLOG_LIMIT: usize = 1024 * 1024;

/// Wie lange das Ende des Anschlusses höchstens auf das Ende des Lesers
/// wartet: Er sieht spätestens nach [`POLL_TICK`] nach, ob er enden soll.
const FINISH_GRACE: Duration = Duration::from_millis(500);

/// Was die Schleife des Anschlusses dem Leser sagt.
///
/// Beim Fallenlassen endet der Leser, spätestens nach [`POLL_TICK`];
/// [`Control::finish`] wartet darauf.
pub struct Control {
    /// Lässt einen angehaltenen Leser weiterlaufen; `None` nach `finish`.
    resume: Option<std::sync::mpsc::Sender<()>>,
    /// Wahr, sobald der Anschluss endet.
    stop: Arc<AtomicBool>,
    /// Der Faden des Lesers.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Control {
    /// Lässt den Leser weiterlaufen, nachdem ein eigener Bildschirm zu Ende
    /// ist. Ein Leser, der nicht angehalten hat, nimmt das beim nächsten
    /// Anhalten als schon erteilt; das kommt nicht vor, denn er hält vor
    /// jedem Ereignis dieser Art an, und die Schleife antwortet genau
    /// einmal.
    pub fn resume(&self) {
        if let Some(resume) = &self.resume {
            let _ = resume.send(());
        }
    }

    /// Beendet den Leser und wartet, bis sein Faden zu Ende ist, höchstens
    /// [`FINISH_GRACE`]. Danach liest er kein Byte mehr: Was der Mensch
    /// schon für seine Shell tippt, bleibt im Terminal (Review zu GitHub
    /// #11). Ein angehaltener Leser endet, weil niemand ihn mehr
    /// weiterlaufen lassen kann.
    pub async fn finish(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.resume = None;
        let Some(thread) = self.thread.take() else {
            return;
        };
        let deadline = tokio::time::Instant::now() + FINISH_GRACE;
        while !thread.is_finished() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Der gestartete Leser.
pub struct Reader {
    /// Was die Schleife ihm sagt.
    pub control: Control,
    /// Der Weiterreicher; der Aufrufer wartet vor `close` auf ihn. Ohne
    /// `forward` gibt es keinen.
    pub forwarder: Option<tokio::task::JoinHandle<()>>,
}

/// Startet den Leser.
///
/// Endet die Eingabe — an einem Terminal nie, an einer Pipe sofort —, schickt
/// der Leser `close`, und der Anschluss hängt sich ab. Die Sitzung läuft
/// weiter; das ist der Unterschied zwischen „ich sehe nicht mehr zu" und „hör
/// auf".
///
/// Nach dem Abhängen endet er; `close` schickt der Aufrufer, nachdem der
/// Weiterreicher die Bytes davor abgegeben hat. Ohne `forward` (ein lesender
/// Anschluss) geht nichts hinauf, und das Ende der Eingabe beendet nur das
/// Lesen. `kitty` sagt, ob der Agent das kitty-Tastaturprotokoll gerade
/// eingeschaltet hat.
pub fn spawn(
    tx: mpsc::Sender<v1::TerminalInput>,
    keys: Keys,
    forward: bool,
    events: mpsc::UnboundedSender<KeyEvent>,
    kitty: Arc<AtomicBool>,
) -> Reader {
    let (backlog, forwarder) = if forward {
        let (backlog, queued) = Backlog::new();
        (
            Some(backlog),
            Some(tokio::spawn(forward_backlog(queued, tx))),
        )
    } else {
        (None, None)
    };
    let (resume, resumed) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let reading = Reading {
        keys,
        forward,
        events,
        kitty,
        resumed,
        stop: Arc::clone(&stop),
    };
    let thread = std::thread::spawn(move || reading.run(backlog));
    let control = Control {
        resume: Some(resume),
        stop,
        thread: Some(thread),
    };
    Reader { control, forwarder }
}

/// Was der Faden des Lesers braucht.
struct Reading {
    keys: Keys,
    forward: bool,
    events: mpsc::UnboundedSender<KeyEvent>,
    kitty: Arc<AtomicBool>,
    resumed: std::sync::mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
}

impl Reading {
    /// Liest, bis die Eingabe endet, der Mensch sich abhängt oder der
    /// Anschluss endet.
    fn run(self, backlog: Option<Backlog>) {
        let mut buffer = vec![0u8; INPUT_CHUNK];
        let mut filter = PrefixFilter::new(self.keys.prefix, self.keys.direct);
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return;
            }
            if !readable(POLL_TICK) {
                continue;
            }
            // Auch nach dem Warten: Endete der Anschluss inzwischen, gehört
            // das Byte schon dem, der das Terminal danach hat.
            if self.stop.load(Ordering::SeqCst) {
                return;
            }
            let read = match rustix::io::read(rustix::stdio::stdin(), &mut buffer) {
                Ok(read) if read > 0 => read,
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
                Ok(_) | Err(_) => {
                    if let Some(backlog) = backlog {
                        // Ein Präfix ganz am Ende war keines: Es gehört dem
                        // Agenten wie jedes andere Byte.
                        backlog.push(filter.finish());
                        backlog.close();
                    }
                    return;
                }
            };
            let chunk = &buffer[..read];
            // Ein lesender Anschluss schickt nichts hinauf, also hat `Ctrl+C`
            // dort keinen Empfänger. Vor HUM-279 beendete es ihn als Signal;
            // im Rohmodus ist es ein Byte (oder im kitty-Protokoll eine
            // Folge), und es hängt ab wie die Taste -- geprüft vor dem
            // Filter, damit es auch dann abhängt, wenn `Ctrl+C` selbst das
            // Präfix ist.
            if !self.forward && has_ctrl_c(chunk) {
                let _ = self.events.send(KeyEvent::Detach);
                return;
            }
            let filtered = filter.feed(chunk, self.kitty.load(Ordering::SeqCst));
            if let Some(backlog) = &backlog {
                backlog.push(filtered.forward);
            }
            for event in filtered.events {
                if event == KeyEvent::Detach {
                    // Die Warteschlange endet hier, ohne `close`: Das schickt
                    // der Aufrufer, nachdem sie leer ist.
                    drop(backlog);
                    let _ = self.events.send(KeyEvent::Detach);
                    return;
                }
                let hands_over = matches!(event, KeyEvent::Open(action) if takes_screen(action));
                let _ = self.events.send(event);
                // Ab hier liest der Bildschirm der Ansicht; der Leser wartet,
                // bis sie zu Ende ist. Ohne Schleife (Anschluss vorbei) endet
                // er.
                if hands_over {
                    if self.resumed.recv().is_err() || self.stop.load(Ordering::SeqCst) {
                        return;
                    }
                    filter.resumed();
                }
            }
        }
    }
}

/// Wartet höchstens `timeout` darauf, dass die eigene Eingabe etwas hat;
/// auch das Ende einer Pipe und ein geschlossenes Terminal zählen, damit
/// das folgende `read` sie meldet.
fn readable(timeout: Duration) -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};

    let stdin = rustix::stdio::stdin();
    let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
    let deadline = Timespec {
        tv_sec: i64::try_from(timeout.as_secs()).unwrap_or(0),
        tv_nsec: i64::from(timeout.subsec_nanos()),
    };
    poll(&mut fds, Some(&deadline)).is_ok_and(|ready| ready > 0)
}

/// Die Warteschlange zwischen Leser und Weiterreicher, mit einer Obergrenze in
/// Bytes.
pub struct Backlog {
    /// Die Nachrichten in ihrer Reihenfolge; unbeschränkt, weil die Grenze in
    /// Bytes über `queued` gilt und der Leser nie warten darf.
    sender: mpsc::UnboundedSender<v1::TerminalInput>,
    /// Wie viele Bytes gerade warten.
    queued: Arc<AtomicUsize>,
}

/// Die Seite des Weiterreichers.
pub struct Queued {
    receiver: mpsc::UnboundedReceiver<v1::TerminalInput>,
    queued: Arc<AtomicUsize>,
}

impl Backlog {
    fn new() -> (Self, Queued) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queued = Arc::new(AtomicUsize::new(0));
        (
            Self {
                sender,
                queued: Arc::clone(&queued),
            },
            Queued { receiver, queued },
        )
    }

    /// Stellt Tastendrücke an. Was in die Obergrenze passt, geht mit; nur der
    /// Rest dahinter fällt weg.
    ///
    /// Nur der Leser stellt an, der Weiterreicher zieht nur ab: Der Platz,
    /// den `load` sieht, kann bis zum `fetch_add` nur größer werden.
    fn push(&self, mut bytes: Vec<u8>) {
        let room = BACKLOG_LIMIT.saturating_sub(self.queued.load(Ordering::Acquire));
        bytes.truncate(room);
        if bytes.is_empty() {
            return;
        }
        self.queued.fetch_add(bytes.len(), Ordering::AcqRel);
        let _ = self.sender.send(data(bytes));
    }

    /// Das Ende der Eingabe: `close` hinter allen Bytes.
    fn close(self) {
        let _ = self.sender.send(close());
    }
}

/// Leert die Warteschlange in den Kanal zum Daemon, bis sie endet.
async fn forward_backlog(mut queued: Queued, tx: mpsc::Sender<v1::TerminalInput>) {
    while let Some(message) = queued.receiver.recv().await {
        let len = match &message.input {
            Some(v1::terminal_input::Input::Data(bytes)) => bytes.len(),
            _ => 0,
        };
        if tx.send(message).await.is_err() {
            return;
        }
        queued.queued.fetch_sub(len, Ordering::AcqRel);
    }
}

/// Tastendrücke als Nachricht.
fn data(bytes: Vec<u8>) -> v1::TerminalInput {
    v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Data(bytes)),
    }
}

/// Das Ende dieses Stroms, nicht das der Sitzung.
#[must_use]
pub fn close() -> v1::TerminalInput {
    v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Close(())),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::atomic::Ordering;

    use super::{BACKLOG_LIMIT, Backlog, v1};

    /// An der Grenze der Warteschlange geht mit, was passt, und nur der Rest
    /// fällt weg (HUM-279, Review): Mit 1 MiB weniger einem Byte in der
    /// Schlange kommt von zwei Bytes das erste an.
    #[test]
    fn the_backlog_takes_what_fits_and_drops_only_the_rest() {
        let (backlog, mut queued) = Backlog::new();
        backlog.push(vec![b'x'; BACKLOG_LIMIT - 1]);
        backlog.push(b"ab".to_vec());
        backlog.push(b"c".to_vec());
        assert_eq!(queued.queued.load(Ordering::Acquire), BACKLOG_LIMIT);

        let first = queued.receiver.try_recv().expect("the first read");
        let second = queued.receiver.try_recv().expect("the part that fit");
        assert!(matches!(
            first.input,
            Some(v1::terminal_input::Input::Data(bytes)) if bytes.len() == BACKLOG_LIMIT - 1
        ));
        assert!(matches!(
            second.input,
            Some(v1::terminal_input::Input::Data(bytes)) if bytes == b"a"
        ));
        assert!(
            queued.receiver.try_recv().is_err(),
            "a full backlog takes nothing more"
        );
    }
}
