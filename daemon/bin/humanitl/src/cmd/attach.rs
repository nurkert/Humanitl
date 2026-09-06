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

use std::io::Write as _;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_ipc::v1;
use tokio::io::AsyncReadExt as _;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::tty::{RawMode, window_size};

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
/// `DAEMON_001` ohne Daemon, `IPC_006`, wenn keine Sitzung läuft, `TERM_001`,
/// wenn schon jemand schreibt, und die Befunde des Daemons für alles Weitere.
pub async fn run(ctx: &Context, read_only: bool) -> Result<u8, Failure> {
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

    // Der Rohmodus steht erst, wenn der Strom offen ist: Ein Befund vor dem
    // ersten Byte soll auf einem gewöhnlichen Terminal lesbar sein.
    let raw = if read_only { None } else { RawMode::enter() };
    if !read_only {
        spawn_input(tx.clone());
        spawn_resizes(tx.clone());
    }

    // `SIGTERM`, `SIGHUP` und `SIGINT` gehören diesem Befehl: Ohne eigenen Weg
    // nähme der Kernel den Prozess weg, bevor `Drop` das Terminal zurückgibt,
    // und die Shell des Menschen bliebe ohne Echo stehen (`tty.rs`).
    let outcome = tokio::select! {
        outcome = pump(&mut stream) => outcome,
        number = terminated() => {
            drop(raw);
            if !read_only {
                let mut out = std::io::stdout();
                let _ = out.write_all(LEAVE_SCREEN);
                let _ = out.flush();
            }
            return Ok(128_u8.saturating_add(number));
        }
    };
    drop(raw);
    if !read_only {
        let mut out = std::io::stdout();
        let _ = out.write_all(LEAVE_SCREEN);
        let _ = out.flush();
    }
    outcome
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

/// Reicht jede Taste an den Agenten weiter.
///
/// Endet die Eingabe — an einem Terminal nie, an einer Pipe sofort —, schickt
/// der Befehl `close` und hängt sich ab. Die Sitzung läuft weiter; das ist der
/// Unterschied zwischen „ich sehe nicht mehr zu" und „hör auf".
fn spawn_input(tx: mpsc::Sender<v1::TerminalInput>) {
    tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buffer = vec![0u8; INPUT_CHUNK];
        loop {
            let Ok(read) = stdin.read(&mut buffer).await else {
                let _ = tx.send(close()).await;
                return;
            };
            if read == 0 {
                let _ = tx.send(close()).await;
                return;
            }
            let message = v1::TerminalInput {
                input: Some(v1::terminal_input::Input::Data(buffer[..read].to_vec())),
            };
            if tx.send(message).await.is_err() {
                return;
            }
        }
    });
}

/// Wartet auf `SIGTERM`, `SIGHUP` oder `SIGINT` und nennt die Nummer.
///
/// `SIGINT` gehört dazu, obwohl `Ctrl+C` im Rohmodus als Byte `0x03` ankommt
/// und kein Signal auslöst: Ein `kill -INT` von außen gibt es trotzdem, und
/// ohne diesen Zweig nähme der Kernel den Prozess weg, bevor der Rohmodus
/// zurückgegeben ist -- die Shell des Menschen bliebe ohne Echo stehen.
///
/// Ohne Handler wartet sie für immer; dann gilt das Standardverhalten des
/// Signals.
async fn terminated() -> u8 {
    use tokio::signal::unix::{SignalKind, signal};

    let mut streams: Vec<(tokio::signal::unix::Signal, u8)> = [
        (SignalKind::terminate(), 15),
        (SignalKind::hangup(), 1),
        (SignalKind::interrupt(), 2),
    ]
    .into_iter()
    .filter_map(|(kind, number)| signal(kind).ok().map(|stream| (stream, number)))
    .collect();
    match streams.as_mut_slice() {
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
