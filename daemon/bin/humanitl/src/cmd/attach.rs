//! `humanitl sandbox attach`: dieses Terminal an die laufende Sitzung hängen
//! (HUM-042).
//!
//! Die Hälfte der `Terminal`-RPC auf der Kommandozeile (ADR-018: Ein neuer
//! RPC-Handler bringt sein Subkommando mit). Der Befehl ist ein dünner Client
//! und macht genau vier Dinge:
//!
//! 1. **Den Rohmodus setzen.** Ohne ihn puffert das eigene Terminal die Zeile,
//!    zeigt Getipptes doppelt und schluckt jede Pfeiltaste. Der Modus wird
//!    beim Verlassen zurückgesetzt, auch auf jedem Fehlerpfad ([`RawMode`]).
//! 2. **Öffnen und die eigene Größe mitschicken.** Die Geometrie gehört dem
//!    Schreiber; ein Leser bekommt die des Schreibers und rendert
//!    letterboxed.
//! 3. **Tastendrücke und `SIGWINCH` hinaufreichen.** `Ctrl+C` ist dabei Byte
//!    `0x03` und kein Signal: Die Sandbox läuft mit `--new-session` und hat
//!    kein steuerndes Terminal. Wer den Agenten wirklich unterbrechen will,
//!    nimmt `humanitl sandbox` … `stop` beziehungsweise beendet die Sitzung.
//! 4. **Die Bytes des Daemons ausgeben.** Sie sind dort gefiltert; dieser
//!    Befehl schreibt sie unverändert, denn ein zweiter Filter im Client wäre
//!    eine zweite Wahrheit.
//!
//! Trennen beendet den Strom, nicht die Sitzung. Der Agent läuft weiter, und
//! ein späteres `attach` zeigt den Rückstand.
//!
//! # Die Taste zum Abhängen (HUM-279)
//!
//! Wie in `screen`: Nach dem Präfix (Vorgabe `Ctrl+A`, `--detach-keys` oder
//! `terminal.detach_prefix`) hängt `d` ab. Der Befehl schickt `close`, gibt
//! das Terminal zurück und endet mit 0; die Sitzung läuft weiter. Das Präfix
//! zweimal oder sein Buchstabe (`Ctrl+A a`) schickt es einmal an den Agenten,
//! jede andere Taste geht mit dem Präfix davor unverändert weiter. Ein Präfix
//! am Ende eines Lesevorgangs wartet auf das nächste Byte, ohne Frist: Ein
//! Terminal liefert eine Taste nicht immer in einem Stück, und eine Frist
//! machte aus einem langsamen `Ctrl+A d` zwei Tasten für den Agenten.
//!
//! Die Taste gehört dieser Kommandozeile allein. Das Terminal der Oberfläche
//! liest sie nicht; dort hängt sich ab, wer den Bereich schließt.

use std::io::{IsTerminal as _, Write as _};
use std::time::Duration;

use humanitl_config::DetachKeys;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::{notice_line, v1};
use tokio::io::AsyncReadExt as _;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::tty::{RawMode, window_size};

/// Die Zeile nach dem Abhängen, auf `stderr` und auf einem gewöhnlichen
/// Terminal.
const DETACHED: &str = "Detached; the session keeps running. Attach again: humanitl sandbox attach";

/// Wie lange der Befehl nach `close` darauf wartet, dass der Daemon den Strom
/// schließt.
///
/// Ohne das Warten endete der Prozess womöglich, bevor `close` den Socket
/// verlassen hat; der Daemon sähe dann einen abgerissenen Strom statt eines
/// Abschieds. Die Frist ist eine Obergrenze und kein Takt: Der Daemon schließt
/// sofort, und ein Daemon, der hängt, soll das Abhängen nicht aufhalten.
const DETACH_GRACE: Duration = Duration::from_secs(2);

/// Wie viele Nachrichten der Eingangskanal puffert.
const INPUT_BUFFER: usize = 32;

/// Wie viel Eingabe auf einmal gelesen wird.
const INPUT_CHUNK: usize = 4096;

/// Was der Befehl beim Verlassen in das eigene Terminal schreibt.
///
/// Ein Vollbild-Agent lässt den Alternativschirm an und den Cursor
/// versteckt. Wer sich abhängt, bekäme sonst eine Shell ohne sichtbaren
/// Cursor auf einem fremden Bild. Das sind die einzigen Folgen, die dieser
/// Befehl selbst schreibt.
const LEAVE_SCREEN: &[u8] = b"\x1b[?1049l\x1b[?25h\r\n";

/// Führt `humanitl sandbox attach` aus.
///
/// # Errors
///
/// `CLI_004` für ein unlesbares `--detach-keys`, die Befunde des Ladens
/// (`CONFIG_003` für ein unlesbares `terminal.detach_prefix`), `DAEMON_001`
/// ohne Daemon, `IPC_006`, wenn keine Sitzung läuft, `TERM_001`, wenn schon
/// jemand schreibt, und die Befunde des Daemons für alles Weitere.
pub async fn run(ctx: &Context, read_only: bool, detach_keys: Option<&str>) -> Result<u8, Failure> {
    // Vor der Verbindung: Ein Tippfehler im Schalter soll nicht erst nach dem
    // Anhängen auffallen, und ohne Daemon soll er trotzdem auffallen.
    let keys = if let Some(spec) = detach_keys {
        DetachKeys::parse(spec).ok_or_else(|| invalid_detach_keys("--detach-keys", spec))?
    } else {
        let config = ctx.config()?.config;
        // Beim Laden geprüft (`CONFIG_003`, `validate.rs`); ein Wert, der
        // trotzdem nicht passt, wäre ein Fehler dort und kein Grund, ohne
        // Taste zum Abhängen zu starten.
        let spec = config.terminal.detach_prefix;
        DetachKeys::parse(&spec)
            .ok_or_else(|| invalid_detach_keys("terminal.detach_prefix", &spec))?
    };

    let mut client = ctx.connect().await?;
    let (cols, rows) = window_size();

    let (tx, rx) = mpsc::channel(INPUT_BUFFER);
    let open = v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Open(v1::terminal_input::Open {
            // Leer heißt „die Sitzung, die läuft"; ein Daemon führt eine.
            sandbox_id: String::new(),
            cols,
            rows,
            read_only,
        })),
    };
    tx.send(open).await.map_err(|_| closed_input())?;

    let mut stream = client
        .terminal(ReceiverStream::new(rx))
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Terminal")))?
        .into_inner();

    // Erst die Signale, dann der Rohmodus: Ein `SIGTERM` zwischen beidem nähme
    // der Kernel mit seinem Standardverhalten, und das Terminal bliebe roh.
    let mut signals = Signals::listen();

    // Ein lesender Anschluss liest die Tastatur nur, um die Taste zum
    // Abhängen zu sehen, und nur an einem Terminal: An einer Pipe gibt es
    // niemanden, der sie drückt, und ihr Ende hängte ihn sonst ab.
    let watches_keys =
        !read_only || (keys != DetachKeys::Disabled && std::io::stdin().is_terminal());
    if watches_keys && let Some(label) = keys.label() {
        ctx.render.note(&format!("Detach: {label}"));
    }

    // Der Rohmodus steht erst, wenn der Strom offen ist: Ein Befund vor dem
    // ersten Byte soll auf einem gewöhnlichen Terminal lesbar sein. Ein
    // lesender Anschluss braucht ihn für die Taste zum Abhängen; ohne sie
    // bleibt sein Terminal gewöhnlich, und `Ctrl+C` beendet ihn wie bisher.
    let raw = if watches_keys { RawMode::enter() } else { None };
    let leaves_screen = !read_only || raw.is_some();
    let (detach_tx, mut detached) = oneshot::channel();
    let forwarder = if watches_keys {
        spawn_input(tx.clone(), keys, !read_only, detach_tx)
    } else {
        None
    };
    if !read_only {
        spawn_resizes(tx.clone());
    }

    // `SIGTERM`, `SIGHUP` und `SIGINT` gehören diesem Befehl: Ohne eigenen Weg
    // nähme der Kernel den Prozess weg, bevor `Drop` das Terminal zurückgibt,
    // und die Shell des Menschen bliebe ohne Echo stehen (`tty.rs`).
    //
    // `biased` mit dem Abhängen zuerst: Endet der Strom im selben Moment, in
    // dem die Taste kommt, gilt die Taste, und Exit-Code und Zeile hängen
    // nicht vom Zufall der Reihenfolge ab.
    let ending = tokio::select! {
        biased;
        // Ein Leser, der ohne Abhängen endet, lässt den Sender fallen; das
        // ist `Err` und schaltet nur diesen Zweig ab.
        Ok(()) = &mut detached => Ending::Detached,
        number = signals.next() => Ending::Signal(number),
        outcome = pump(&mut stream) => Ending::Stream(outcome),
    };
    if matches!(ending, Ending::Detached) {
        // `close` schickt dieser Rahmen und nicht der Leser: Schickte der es
        // vor seiner Meldung, könnte das Ende des Stroms das Rennen gegen die
        // Meldung gewinnen, und der Befehl hielte das Abhängen für das Ende
        // der Sitzung. Erst gibt der Weiterreicher ab, was vor der Taste
        // getippt war, dann folgt `close`. Was der Daemon bis zum Ende des
        // Stroms noch schickt, gehört nicht mehr auf dieses Terminal.
        //
        // Der ganze Weg steht unter einer Frist: Ein Daemon, der seine
        // Eingabe nicht liest, hielte sonst schon das Abgeben oder `close`
        // fest, und das Terminal käme nie zurück.
        let _ = tokio::time::timeout(DETACH_GRACE, async {
            if let Some(forwarder) = forwarder {
                let _ = forwarder.await;
            }
            let _ = tx.send(close()).await;
            drain(&mut stream).await;
        })
        .await;
    }
    drop(raw);
    if leaves_screen {
        let mut out = std::io::stdout();
        let _ = out.write_all(LEAVE_SCREEN);
        let _ = out.flush();
    }
    match ending {
        Ending::Stream(outcome) => outcome,
        Ending::Detached => {
            ctx.render.note(DETACHED);
            Ok(EXIT_OK)
        }
        Ending::Signal(number) => Ok(128_u8.saturating_add(number)),
    }
}

/// Wie ein Anschluss endet.
enum Ending {
    /// Der Strom ist zu Ende: die Sitzung, ein Befund, oder das Ende der
    /// Eingabe an einer Pipe.
    Stream(Result<u8, Failure>),
    /// Der Mensch hat sich mit der Taste abgehängt.
    Detached,
    /// Ein Signal mit dieser Nummer.
    Signal(u8),
}

/// Liest den Strom bis zu seinem Ende und verwirft, was kommt.
async fn drain(stream: &mut tonic::Streaming<v1::TerminalOutput>) {
    while let Ok(Some(_)) = stream.message().await {}
}

/// Der Befund für eine Angabe, die keine Taste zum Abhängen nennt; `source`
/// ist der Schalter oder der Schlüssel, aus dem sie kam.
fn invalid_detach_keys(source: &str, spec: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!(
                "{source} {spec:?} names no detach key; allowed is {}",
                DetachKeys::ALLOWED
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl sandbox attach --detach-keys {}",
                DetachKeys::DEFAULT_SPEC
            )))
            .build(),
    )
}

/// Liest den Strom, bis er endet, und gibt den Exit-Code der Sitzung zurück.
async fn pump(stream: &mut tonic::Streaming<v1::TerminalOutput>) -> Result<u8, Failure> {
    let mut code = EXIT_OK;
    loop {
        let message = match stream.message().await {
            Ok(Some(message)) => message,
            Ok(None) => return Ok(code),
            Err(status) => {
                return Err(Failure::new(status_diagnostic(&status, "Terminal")));
            }
        };
        match message.output {
            Some(v1::terminal_output::Output::Data(data)) => {
                let mut out = std::io::stdout();
                if out.write_all(&data).is_err() || out.flush().is_err() {
                    return Ok(code);
                }
            }
            // Die Hinweiszeile des Daemons steht neben den Bytes und nicht
            // darin (`ipc/src/terminal.rs`). Wer am Terminal sitzt, hat keine
            // zweite Fläche dafür, also schreibt dieser Befehl sie zwischen
            // die Bytes des Agenten -- an der Grenze, an der der Daemon sie
            // geschickt hat, und in ihre eigene Zeile.
            Some(v1::terminal_output::Output::Notice(text)) => {
                let mut out = std::io::stdout();
                if out.write_all(&notice_line(&text)).is_err() || out.flush().is_err() {
                    return Ok(code);
                }
            }
            Some(v1::terminal_output::Output::Exit(exit)) => {
                code = u8::try_from(exit.code).unwrap_or(1);
            }
            // Ein Befund beendet den Strom des Clients; der Daemon schickt
            // ihn genau dann, wenn er nichts mehr liefern wird. Einen Code,
            // den diese Fassung nicht kennt, meldet sie nicht als eigenen
            // Fehlschlag — dieselbe Regel wie in `humanitl run`.
            Some(v1::terminal_output::Output::Diagnostic(diagnostic)) => {
                match from_proto(&diagnostic) {
                    Some(rebuilt) => return Err(Failure::new(rebuilt)),
                    None => return Ok(code),
                }
            }
            // Die Geometrie des Schreibers. Ein Leser rendert letterboxed;
            // dieser Befehl schreibt in ein Terminal, das seine eigene Größe
            // hat, und lässt sie deshalb stehen.
            Some(v1::terminal_output::Output::Resize(_)) | None => {}
        }
    }
}

/// Reicht jede Taste an den Agenten weiter, bis auf die Taste zum Abhängen.
///
/// Endet die Eingabe — an einem Terminal nie, an einer Pipe sofort —, schickt
/// der Befehl `close` und hängt sich ab. Die Sitzung läuft weiter; das ist der
/// Unterschied zwischen „ich sehe nicht mehr zu" und „hör auf".
///
/// Kommt die Taste zum Abhängen, meldet der Leser es über `detach` und endet;
/// `close` schickt der Aufrufer, nachdem der Weiterreicher die Bytes davor
/// abgegeben hat. Ohne `forward` (ein lesender Anschluss) geht nichts hinauf,
/// und das Ende der Eingabe beendet nur das Lesen.
///
/// **Lesen und Weiterreichen sind getrennt.** Liest die Sitzung ihre Eingabe
/// nicht, staut sich ein eingefügter Text bis in den Kanal zum Daemon, und
/// ein Leser, der selbst in `send` wartete, sähe die Taste zum Abhängen nie:
/// Der Mensch säße fest. Deshalb liest der Leser immer weiter und gibt die
/// Bytes an eine eigene Warteschlange ([`Backlog`]); der Weiterreicher leert
/// sie in den Kanal. Was über [`BACKLOG_LIMIT`] hinausgeht, fällt weg -- die
/// Sitzung nimmt in diesem Moment ohnehin nichts an.
///
/// Gibt den Weiterreicher zurück, damit der Aufrufer vor `close` auf ihn
/// warten kann; ohne `forward` gibt es keinen.
fn spawn_input(
    tx: mpsc::Sender<v1::TerminalInput>,
    keys: DetachKeys,
    forward: bool,
    detach: oneshot::Sender<()>,
) -> Option<tokio::task::JoinHandle<()>> {
    let (backlog, forwarder) = if forward {
        let (backlog, queued) = Backlog::new();
        (
            Some(backlog),
            Some(tokio::spawn(forward_backlog(queued, tx))),
        )
    } else {
        (None, None)
    };
    tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buffer = vec![0u8; INPUT_CHUNK];
        let mut filter = DetachFilter::new(keys);
        loop {
            let read = match stdin.read(&mut buffer).await {
                Ok(read) if read > 0 => read,
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
            // im Rohmodus ist es ein Byte, und es hängt ab wie die Taste --
            // geprüft vor dem Filter, damit es auch dann abhängt, wenn
            // `Ctrl+C` selbst das Präfix ist.
            if !forward && chunk.contains(&CTRL_C) {
                let _ = detach.send(());
                return;
            }
            let filtered = filter.feed(chunk);
            if let Some(backlog) = &backlog {
                backlog.push(filtered.forward);
            }
            if filtered.detach {
                // Die Warteschlange endet hier, ohne `close`: Das schickt der
                // Aufrufer, nachdem sie leer ist.
                drop(backlog);
                let _ = detach.send(());
                return;
            }
        }
    });
    forwarder
}

/// Wie viele Bytes zwischen Tastatur und Kanal höchstens warten.
///
/// Ein MiB ist mehr, als ein Mensch auf einmal einfügt, und wenig genug, dass
/// eine Sitzung, die nicht liest, den Speicher nicht füllt.
const BACKLOG_LIMIT: usize = 1024 * 1024;

/// Die Warteschlange zwischen Leser und Weiterreicher, mit einer Obergrenze in
/// Bytes.
struct Backlog {
    /// Die Nachrichten in ihrer Reihenfolge; unbeschränkt, weil die Grenze in
    /// Bytes über `queued` gilt und der Leser nie warten darf.
    sender: mpsc::UnboundedSender<v1::TerminalInput>,
    /// Wie viele Bytes gerade warten.
    queued: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// Die Seite des Weiterreichers.
struct Queued {
    receiver: mpsc::UnboundedReceiver<v1::TerminalInput>,
    queued: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Backlog {
    fn new() -> (Self, Queued) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queued = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        (
            Self {
                sender,
                queued: std::sync::Arc::clone(&queued),
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
        use std::sync::atomic::Ordering;

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
    use std::sync::atomic::Ordering;

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

/// `Ctrl+C` im Rohmodus.
const CTRL_C: u8 = 0x03;

/// Tastendrücke als Nachricht.
fn data(bytes: Vec<u8>) -> v1::TerminalInput {
    v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Data(bytes)),
    }
}

/// Was von einem Lesevorgang hinaufgeht, und ob danach abgehängt wird.
#[derive(Debug, PartialEq, Eq)]
struct Filtered {
    /// Die Bytes für den Agenten, in ihrer Reihenfolge.
    forward: Vec<u8>,
    /// Wahr, wenn in diesem Stück die Taste zum Abhängen stand. Was danach
    /// kam, ist verworfen: Der Mensch geht.
    detach: bool,
}

/// Sucht die Taste zum Abhängen im Strom der Tastendrücke.
///
/// Eine reine Zustandsmaschine ohne Ein- und Ausgabe, damit jeder Fall ohne
/// Terminal prüfbar ist. Der einzige Zustand ist, ob das letzte Byte das
/// Präfix war: Das Präfix kann am Ende eines Lesevorgangs stehen und die
/// Taste dahinter im nächsten.
#[derive(Debug)]
struct DetachFilter {
    /// Das Präfix, oder `None`, wenn es keine Taste zum Abhängen gibt.
    prefix: Option<u8>,
    /// Wahr, wenn das Präfix gelesen ist und das Byte danach noch aussteht.
    pending: bool,
}

impl DetachFilter {
    /// Ein Filter für diese Taste, ohne gelesenes Präfix.
    const fn new(keys: DetachKeys) -> Self {
        let prefix = match keys {
            DetachKeys::Prefix(byte) => Some(byte),
            DetachKeys::Disabled => None,
        };
        Self {
            prefix,
            pending: false,
        }
    }

    /// Filtert ein gelesenes Stück.
    fn feed(&mut self, chunk: &[u8]) -> Filtered {
        let Some(prefix) = self.prefix else {
            return Filtered {
                forward: chunk.to_vec(),
                detach: false,
            };
        };
        // Der Buchstabe des Präfixes: `a` für `Ctrl+A`, `]` für `Ctrl+]`.
        let letter = (prefix | 0x40).to_ascii_lowercase();
        let mut forward = Vec::with_capacity(chunk.len());
        for &byte in chunk {
            if self.pending {
                self.pending = false;
                if byte == b'd' {
                    return Filtered {
                        forward,
                        detach: true,
                    };
                }
                forward.push(prefix);
                if byte != prefix && byte != letter {
                    forward.push(byte);
                }
            } else if byte == prefix {
                self.pending = true;
            } else {
                forward.push(byte);
            }
        }
        Filtered {
            forward,
            detach: false,
        }
    }

    /// Was am Ende der Eingabe noch aussteht: ein Präfix ohne Taste danach.
    fn finish(&mut self) -> Vec<u8> {
        match (self.pending, self.prefix) {
            (true, Some(prefix)) => {
                self.pending = false;
                vec![prefix]
            }
            _ => Vec::new(),
        }
    }
}

/// Die Signale, auf die dieser Befehl selbst hört.
///
/// **Der Handler steht, bevor das Terminal roh wird.** Ein Signal, das in der
/// Lücke dazwischen ankommt, nähme der Kernel mit seinem Standardverhalten --
/// der Prozess wäre weg, das Terminal bliebe im Rohmodus, und die Shell des
/// Menschen stünde ohne Echo da. Genau diese Lücke ist am 2026-09-07 im Test
/// `every_signal_gives_the_terminal_back` aufgefallen: unter Last endete der
/// Lauf mit einem Signal statt mit `128 + n`.
struct Signals {
    /// Die Ströme und ihre Nummern, in der Reihenfolge `TERM`, `HUP`, `INT`.
    streams: Vec<(tokio::signal::unix::Signal, u8)>,
}

impl Signals {
    /// Meldet die Handler an. Was sich nicht anmelden lässt, fehlt still; dann
    /// gilt für dieses eine Signal das Standardverhalten.
    fn listen() -> Self {
        use tokio::signal::unix::{SignalKind, signal};

        Self {
            streams: [
                (SignalKind::terminate(), 15),
                (SignalKind::hangup(), 1),
                (SignalKind::interrupt(), 2),
            ]
            .into_iter()
            .filter_map(|(kind, number)| signal(kind).ok().map(|stream| (stream, number)))
            .collect(),
        }
    }

    /// Wartet auf das erste Signal und nennt seine Nummer.
    ///
    /// `SIGINT` gehört dazu, obwohl `Ctrl+C` im Rohmodus als Byte `0x03`
    /// ankommt und kein Signal auslöst: Ein `kill -INT` von außen gibt es
    /// trotzdem.
    ///
    /// Ohne Handler wartet sie für immer.
    async fn next(&mut self) -> u8 {
        match self.streams.as_mut_slice() {
            [(term, first), (hup, second), (int, third)] => tokio::select! {
                _ = term.recv() => *first,
                _ = hup.recv() => *second,
                _ = int.recv() => *third,
            },
            [(only, number)] => {
                only.recv().await;
                *number
            }
            [(one, first), (two, second)] => tokio::select! {
                _ = one.recv() => *first,
                _ = two.recv() => *second,
            },
            _ => std::future::pending().await,
        }
    }
}

/// Das Ende dieses Stroms, nicht das der Sitzung.
fn close() -> v1::TerminalInput {
    v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Close(())),
    }
}

/// Reicht jede neue Fenstergröße weiter.
///
/// Der Daemon drosselt sie auf eine je 50 ms; dieser Befehl schickt, was das
/// Terminal meldet.
fn spawn_resizes(tx: mpsc::Sender<v1::TerminalInput>) {
    tokio::spawn(async move {
        let Ok(mut winch) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())
        else {
            return;
        };
        while winch.recv().await.is_some() {
            let (cols, rows) = window_size();
            let message = v1::TerminalInput {
                input: Some(v1::terminal_input::Input::Resize(
                    v1::terminal_input::Resize { cols, rows },
                )),
            };
            if tx.send(message).await.is_err() {
                return;
            }
        }
    });
}

/// Der Befund, wenn der Eingangskanal des Stroms schon zu ist.
fn closed_input() -> Failure {
    Failure::new(
        Diagnostic::builder(codes::DAEMON_001, Severity::Error)
            .why("the terminal stream closed before it opened".to_owned())
            .build(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use humanitl_config::DetachKeys;

    use super::{BACKLOG_LIMIT, Backlog, DetachFilter, Filtered};

    /// An der Grenze der Warteschlange geht mit, was passt, und nur der Rest
    /// fällt weg (HUM-279, Review): Mit 1 MiB weniger einem Byte in der
    /// Schlange kommt von zwei Bytes das erste an.
    #[test]
    fn the_backlog_takes_what_fits_and_drops_only_the_rest() {
        use std::sync::atomic::Ordering;

        let (backlog, mut queued) = Backlog::new();
        backlog.push(vec![b'x'; BACKLOG_LIMIT - 1]);
        backlog.push(b"ab".to_vec());
        backlog.push(b"c".to_vec());
        assert_eq!(queued.queued.load(Ordering::Acquire), BACKLOG_LIMIT);

        let first = queued.receiver.try_recv().expect("the first read");
        let second = queued.receiver.try_recv().expect("the part that fit");
        assert!(matches!(
            first.input,
            Some(super::v1::terminal_input::Input::Data(bytes)) if bytes.len() == BACKLOG_LIMIT - 1
        ));
        assert!(matches!(
            second.input,
            Some(super::v1::terminal_input::Input::Data(bytes)) if bytes == b"a"
        ));
        assert!(
            queued.receiver.try_recv().is_err(),
            "a full backlog takes nothing more"
        );
    }

    /// `Ctrl+A`, wie die Vorgabe es nennt.
    const CTRL_A: u8 = 0x01;

    /// Ein Filter mit der Vorgabe `Ctrl+A`.
    fn ctrl_a() -> DetachFilter {
        DetachFilter::new(DetachKeys::Prefix(CTRL_A))
    }

    /// Was hinaufgeht, ohne abzuhängen.
    fn forwarded(bytes: &[u8]) -> Filtered {
        Filtered {
            forward: bytes.to_vec(),
            detach: false,
        }
    }

    /// `Ctrl+A d` hängt ab; was davor stand, geht noch hinauf, das Präfix
    /// und das `d` nicht.
    #[test]
    fn ctrl_a_d_detaches() {
        let mut filter = ctrl_a();
        assert_eq!(
            filter.feed(b"ls\x01d"),
            Filtered {
                forward: b"ls".to_vec(),
                detach: true,
            }
        );
        assert_eq!(
            ctrl_a().feed(b"\x01d"),
            Filtered {
                forward: Vec::new(),
                detach: true,
            }
        );
    }

    /// `Ctrl+A Ctrl+A` und `Ctrl+A a` schicken genau ein `Ctrl+A` an den
    /// Agenten, und danach steht kein Präfix mehr aus.
    #[test]
    fn ctrl_a_ctrl_a_sends_one_prefix() {
        let mut filter = ctrl_a();
        assert_eq!(filter.feed(b"\x01\x01"), forwarded(&[CTRL_A]));
        assert_eq!(filter.feed(b"d"), forwarded(b"d"), "no prefix is pending");
        assert_eq!(ctrl_a().feed(b"\x01a"), forwarded(&[CTRL_A]));
    }

    /// Das Präfix am Ende eines Lesevorgangs wartet auf den nächsten; das
    /// `d` darin hängt trotzdem ab.
    #[test]
    fn a_prefix_split_across_reads_still_detaches() {
        let mut filter = ctrl_a();
        assert_eq!(filter.feed(b"x\x01"), forwarded(b"x"));
        assert_eq!(
            filter.feed(b"d"),
            Filtered {
                forward: Vec::new(),
                detach: true,
            }
        );
        // Und das Präfix allein, ohne Taste davor, ebenso.
        let mut filter = ctrl_a();
        assert_eq!(filter.feed(&[CTRL_A]), forwarded(b""));
        assert!(filter.feed(b"d").detach);
    }

    /// Jede andere Taste nach dem Präfix geht mit dem Präfix davor
    /// unverändert hinauf, auch über eine Lesegrenze.
    #[test]
    fn an_unknown_key_after_the_prefix_is_forwarded() {
        assert_eq!(ctrl_a().feed(b"\x01x"), forwarded(b"\x01x"));
        assert_eq!(ctrl_a().feed(b"\x01D"), forwarded(b"\x01D"));
        assert_eq!(ctrl_a().feed(b"\x01\x03"), forwarded(b"\x01\x03"));
        let mut filter = ctrl_a();
        assert_eq!(filter.feed(b"\x01"), forwarded(b""));
        assert_eq!(filter.feed(b"q"), forwarded(b"\x01q"));
        // Ein Präfix, auf das nichts mehr folgt, gehört dem Agenten.
        let mut filter = ctrl_a();
        assert_eq!(filter.feed(b"\x01"), forwarded(b""));
        assert_eq!(filter.finish(), vec![CTRL_A]);
    }

    /// `none`: Jedes Byte geht hinauf, `Ctrl+A d` eingeschlossen.
    #[test]
    fn none_disables_detaching() {
        let mut filter = DetachFilter::new(DetachKeys::Disabled);
        assert_eq!(filter.feed(b"\x01d"), forwarded(b"\x01d"));
        assert_eq!(filter.feed(b"\x01"), forwarded(b"\x01"));
        assert!(filter.finish().is_empty());
    }

    /// Ein anderes Präfix: `Ctrl+]` mit `]` als seinem Buchstaben, und
    /// `Ctrl+A` ist dann eine gewöhnliche Taste.
    #[test]
    fn another_prefix_has_its_own_letter() {
        let mut filter = DetachFilter::new(DetachKeys::Prefix(0x1d));
        assert_eq!(filter.feed(b"\x01d"), forwarded(b"\x01d"));
        assert_eq!(filter.feed(b"\x1d]"), forwarded(b"\x1d"));
        assert!(filter.feed(b"\x1dd").detach);
    }
}
