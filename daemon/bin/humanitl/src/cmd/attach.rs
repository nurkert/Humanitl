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
//!    `0x03` und kein Signal dieses Prozesses. Im Pseudoterminal des Daemons
//!    wird es zum `SIGINT` an die Vordergrundgruppe, denn der erste Prozess
//!    der Sitzung trägt es seit HUM-280 als steuerndes Terminal; die Sandbox
//!    läuft weiter mit `--new-session`, und das Terminal dieses Befehls
//!    erreicht sie nie. Ein Programm im Rohmodus liest das Byte selbst.
//! 4. **Die Bytes des Daemons ausgeben.** Sie sind dort gefiltert; dieser
//!    Befehl schreibt sie unverändert, denn ein zweiter Filter im Client wäre
//!    eine zweite Wahrheit.
//!
//! Trennen beendet den Strom, nicht die Sitzung. Der Agent läuft weiter, und
//! ein späteres `attach` zeigt den Rückstand.
//!
//! # Die Präfixtaste (HUM-279, GitHub #10)
//!
//! Wie in tmux: Nach dem Präfix (Vorgabe `Ctrl+B`, `--prefix` oder
//! `terminal.prefix`) gilt eine feste Tabelle ([`crate::prefix`]): `d` hängt
//! ab, `,` und `i` öffnen Einstellungen und Interceptor, `?` die Hilfe. Beim
//! Abhängen schickt der Befehl `close`, gibt das Terminal zurück und endet
//! mit 0; die Sitzung läuft weiter. Das Präfix zweimal oder sein Buchstabe
//! (`Ctrl+B b`) schickt es einmal an den Agenten, jede andere Taste geht mit
//! dem Präfix davor unverändert weiter. Ein Präfix am Ende eines
//! Lesevorgangs wartet auf das nächste Byte, ohne Frist: Ein Terminal liefert
//! eine Taste nicht immer in einem Stück, und eine Frist machte aus einem
//! langsamen `Ctrl+B d` zwei Tasten für den Agenten.
//!
//! Die Taste gehört dieser Kommandozeile allein. Das Terminal der Oberfläche
//! liest sie nicht; dort hängt sich ab, wer den Bereich schließt.
//!
//! # Die Einstellungen über dem Agenten (GitHub #11)
//!
//! Präfix und `,` öffnen die Einstellungen im laufenden Anschluss
//! ([`crate::prefix::settings`]): Der Leser der Tastatur hält an, die
//! Ausgabe des Agenten wartet, und nach `Esc` lässt der Anschluss den Agenten
//! neu zeichnen ([`crate::prefix::redraw`]). Die Schleife wartet dabei nie:
//! Bildschirm, Neuzeichnen und Strom sind Zweige derselben Schleife.
//!
//! # Der Interceptor und die Glocke (GitHub #12)
//!
//! Präfix und `i` öffnen den Interceptor nach denselben Regeln
//! ([`crate::prefix::interceptor`]): Leser angehalten, keine Taste beim
//! Agenten, seine Ausgabe als ganze Stücke zurückgehalten, Neuzeichnen bei
//! der Rückkehr. Ein lesender Anschluss zeigt ihn nur zum Lesen und über
//! einem Agenten auf dem Alternativschirm gar nicht (ein `BEL` statt dessen),
//! wie die Einstellungen.
//!
//! Eine neu gehaltene Anfrage schreibt keinen Text in das Terminal des
//! Agenten: Die Hinweiszeile des Daemons (`TerminalOutput.notice`) steht dort
//! nur noch in der Fußzeile einer offenen Ansicht. Geht die Ausgabe in eine
//! Datei oder Pipe, ist sie ein Mitschnitt und kein Bild des Agenten; dort
//! steht die Zeile weiter zwischen den Bytes, in ihrer eigenen Zeile (HUM-042,
//! der Lauf `tests/e2e/m3_agent_inside` liest sie so). Statt dessen klingelt
//! der Anschluss, wenn `terminal.bell_on_hold` gilt und die Ausgabe ein Terminal
//! ist ([`crate::prefix::bell`]). Woher er von der Anfrage weiß, ist ein
//! eigenes Abonnement von `Subscribe` (`FlowEvent.held`) und nicht der Text
//! der Hinweiszeile: Die schaltet `ui.terminal_notices` ab, und ein Text ist
//! keine Art von Ereignis.

use std::io::{IsTerminal as _, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use humanitl_config::DetachKeys;
use humanitl_config::prefix::PrefixAction;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::{notice_line, v1};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::prefix::Keys;
use crate::prefix::bell::HoldBell;
use crate::prefix::input::{self, Control, Reader};
use crate::prefix::interceptor::InterceptorView;
use crate::prefix::keys::{KeyEvent, takes_screen};
use crate::prefix::redraw::{Redraw, resize};
use crate::prefix::settings::SettingsView;
use crate::prefix::stream::StreamState;
use crate::prefix::view::AgentView;
use crate::tty::{RawMode, window_size};
use crate::tui::Theme;

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

/// Führt `humanitl sandbox attach` aus.
///
/// # Errors
///
/// `CLI_004` für ein unlesbares `--prefix`, die Befunde des Ladens
/// (`CONFIG_003` für ein unlesbares `terminal.prefix`), `DAEMON_001`
/// ohne Daemon, `IPC_006`, wenn keine Sitzung läuft, `TERM_001`, wenn schon
/// jemand schreibt, und die Befunde des Daemons für alles Weitere.
///
/// `shell` ist leer für das Terminal des Agenten, sonst die Kennung einer
/// weiteren Shell aus `humanitl sandbox exec` (HUM-299).
pub async fn run(
    ctx: &Context,
    read_only: bool,
    prefix: Option<&str>,
    shell: &str,
) -> Result<u8, Failure> {
    // Vor der Verbindung: Ein Tippfehler im Schalter soll nicht erst nach dem
    // Anhängen auffallen, und ohne Daemon soll er trotzdem auffallen.
    let keys = resolve_keys(ctx, prefix)?;
    let ending = attach_to(ctx, read_only, keys, shell).await?;
    if shell.is_empty() {
        finish(ctx, ending)
    } else {
        finish_with(ctx, ending, &shell_detached(shell))
    }
}

/// Die Zeile nach dem Abhängen von einer weiteren Shell (HUM-299).
pub(crate) fn shell_detached(shell: &str) -> String {
    format!(
        "Detached; the shell keeps running. Attach again: humanitl sandbox attach --shell {shell}"
    )
}

/// Die Präfixtaste: der Schalter (`--prefix`, alter Name `--detach-keys`),
/// sonst `terminal.prefix`; dazu `terminal.direct_keys` und ob die Zeile
/// zur neuen Vorgabe fällig sein kann (GitHub #10).
///
/// # Errors
///
/// `CLI_004` für ein unlesbares `--prefix`, die Befunde des Ladens.
pub(crate) fn resolve_keys(ctx: &Context, prefix: Option<&str>) -> Result<Keys, Failure> {
    // Der Schalter zuerst: Ein Tippfehler darin fällt vor allem anderen auf.
    let flag = prefix
        .map(|spec| DetachKeys::parse(spec).ok_or_else(|| invalid_prefix("--prefix", spec)))
        .transpose()?;
    let resolved = ctx.config()?;
    let terminal = &resolved.config.terminal;
    let keys = if let Some(keys) = flag {
        keys
    } else {
        // Beim Laden geprüft (`CONFIG_003`, `validate.rs`); ein Wert, der
        // trotzdem nicht passt, wäre ein Fehler dort und kein Grund, ohne
        // Präfixtaste zu starten.
        DetachKeys::parse(&terminal.prefix)
            .ok_or_else(|| invalid_prefix("terminal.prefix", &terminal.prefix))?
    };
    Ok(Keys {
        prefix: keys,
        direct: terminal.direct_keys,
        migration: prefix.is_none()
            && humanitl_config::prefix::migration_due(&ctx.paths, &resolved),
        bell: terminal.bell_on_hold,
    })
}

/// Die Zeile zur neuen Vorgabe, einmal je Nutzer (GitHub #10): nur an einem
/// Anschluss, der die Tastatur liest, und nur, solange der Merker im
/// Datenverzeichnis fehlt. Ein Merker, der sich nicht schreiben lässt,
/// bringt die Zeile beim nächsten Mal wieder; das ist die harmlose Seite.
fn announce_new_prefix(ctx: &Context, keys: Keys) {
    // Nur wer den Merker anlegt, zeigt die Zeile: Zwei erste Anschlüsse im
    // selben Augenblick zeigen sie einmal. Der Merker liegt im
    // Datenverzeichnis, geschrieben von der Kommandozeile und nicht über
    // einen RPC; das ist ein Hinweis ohne Entscheidung (ADR-018, ein RPC
    // dafür ist ein eigenes Issue).
    if keys.migration && humanitl_config::prefix::claim_migration(&ctx.paths).unwrap_or(true) {
        ctx.render.note(humanitl_config::prefix::MIGRATION_LINE);
    }
}

/// Der Exit-Code eines Anschlusses, und nach dem Abhängen die Zeile dazu.
///
/// # Errors
///
/// Der Befund, mit dem der Strom endete.
pub(crate) fn finish(ctx: &Context, ending: Ending) -> Result<u8, Failure> {
    finish_with(ctx, ending, DETACHED)
}

/// Wie [`finish`], mit einer eigenen Zeile nach dem Abhängen.
///
/// # Errors
///
/// Der Befund, mit dem der Strom endete.
pub(crate) fn finish_with(ctx: &Context, ending: Ending, detached: &str) -> Result<u8, Failure> {
    match ending {
        Ending::Stream(outcome) => outcome,
        Ending::Detached => {
            // Ein Terminal, das weg ist, bekommt keine Zeile mehr (GitHub #13).
            if !crate::tty::terminal_gone() {
                ctx.render.note(detached);
            }
            Ok(EXIT_OK)
        }
        Ending::Signal(number) => Ok(128_u8.saturating_add(number)),
    }
}

/// Hängt dieses Terminal an die laufende Sitzung, bis der Strom endet, die
/// Taste zum Abhängen kommt oder ein Signal.
///
/// Dieselbe Hälfte für `humanitl sandbox attach` und für `humanitl run` an
/// einem Terminal (HUM-280); was danach zu sagen ist, entscheidet der
/// Aufrufer ([`finish`]). Das Terminal ist beim Rückkehren wieder gewöhnlich.
///
/// # Errors
///
/// `DAEMON_001` ohne Daemon und die Befunde der `Terminal`-RPC vor dem
/// ersten Byte.
pub(crate) async fn attach(ctx: &Context, read_only: bool, keys: Keys) -> Result<Ending, Failure> {
    attach_to(ctx, read_only, keys, "").await
}

/// Wie [`attach`], an das Terminal einer weiteren Shell, wenn `shell` eine
/// Kennung nennt (HUM-299); leer ist das Terminal des Agenten.
///
/// # Errors
///
/// Wie [`attach`]; `IPC_006` für eine Shell, die es nicht (mehr) gibt.
pub(crate) async fn attach_to(
    ctx: &Context,
    read_only: bool,
    keys: Keys,
    shell: &str,
) -> Result<Ending, Failure> {
    attach_at(ctx, read_only, keys, shell, "").await
}

/// Wie [`attach`], an das Terminal genau der Sandbox `sandbox` (HUM-563:
/// der Wartungslauf, den `humanitl agents install` gestartet hat). Läuft
/// inzwischen eine andere, lehnt der Daemon mit `IPC_006` ab.
///
/// # Errors
///
/// Wie [`attach`].
pub(crate) async fn attach_sandbox(
    ctx: &Context,
    read_only: bool,
    keys: Keys,
    sandbox: &str,
) -> Result<Ending, Failure> {
    attach_at(ctx, read_only, keys, "", sandbox).await
}

async fn attach_at(
    ctx: &Context,
    read_only: bool,
    keys: Keys,
    shell: &str,
    sandbox: &str,
) -> Result<Ending, Failure> {
    // Erst die Signale, dann Verbindung und Rohmodus: Ein Signal während des
    // Aufbaus nähme der Kernel sonst mit seinem Standardverhalten, und ein
    // Signal zwischen Rohmodus und Schleife ließe das Terminal roh
    // (GitHub #13).
    let mut signals = Signals::listen();
    attach_with(ctx, read_only, keys, (shell, sandbox), &mut signals).await
}

/// Wie [`attach_sandbox`] (leer heißt die laufende Sitzung), mit Signalen,
/// die der Aufrufer schon vor seinen
/// eigenen Anfragen angemeldet hat (GitHub #13: ein zweites `humanitl go`
/// hört ab dem ersten RPC). Ein Signal, das schon wartet, beendet den
/// Anschluss, bevor er etwas schreibt.
///
/// # Errors
///
/// Wie [`attach`]; `IPC_006`, wenn `sandbox` nicht mehr die laufende
/// Sitzung ist.
pub(crate) async fn attach_pinned(
    ctx: &Context,
    read_only: bool,
    keys: Keys,
    sandbox: &str,
    signals: &mut Signals,
) -> Result<Ending, Failure> {
    attach_with(ctx, read_only, keys, ("", sandbox), signals).await
}

/// Das `Open` eines Anschlusses, mit der Größe dieses Terminals.
fn open_message(read_only: bool, shell: &str, sandbox: &str) -> v1::TerminalInput {
    let (cols, rows) = window_size();
    v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Open(v1::terminal_input::Open {
            // Leer heißt „die Sitzung, die läuft"; ein Daemon führt eine.
            sandbox_id: sandbox.to_owned(),
            cols,
            rows,
            read_only,
            shell_id: shell.to_owned(),
        })),
    }
}

/// Der Anschluss selbst. Ein Signal aus dem Aufbau beendet ihn, bevor er
/// irgendetwas schreibt (GitHub #13).
async fn attach_with(
    ctx: &Context,
    read_only: bool,
    keys: Keys,
    (shell, sandbox): (&str, &str),
    signals: &mut Signals,
) -> Result<Ending, Failure> {
    let mut client = ctx.connect().await?;
    let (tx, rx) = mpsc::channel(INPUT_BUFFER);
    tx.send(open_message(read_only, shell, sandbox))
        .await
        .map_err(|_| closed_input())?;
    let mut stream = client
        .terminal(ReceiverStream::new(rx))
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Terminal")))?
        .into_inner();
    if let Some(number) = signals.pending() {
        return Ok(Ending::Signal(number));
    }

    // Ein lesender Anschluss liest die Tastatur nur, um die Präfixtaste zu
    // sehen, und nur an einem Terminal: An einer Pipe gibt es niemanden, der
    // sie drückt, und ihr Ende hängte ihn sonst ab.
    let watches_keys = !read_only || (!keys.disabled() && std::io::stdin().is_terminal());
    if watches_keys && let Some(hint) = keys.hint() {
        announce_new_prefix(ctx, keys);
        ctx.render.note(&hint);
    }

    // Der Rohmodus steht erst, wenn der Strom offen ist: Ein Befund vor dem
    // ersten Byte soll auf einem gewöhnlichen Terminal lesbar sein. Ein
    // lesender Anschluss braucht ihn für die Präfixtaste; ohne sie bleibt
    // sein Terminal gewöhnlich, und `Ctrl+C` beendet ihn wie bisher.
    let raw = if watches_keys { RawMode::enter() } else { None };
    // Aufgeräumt wird, wo die Ausgabe auf ein Terminal geht, unabhängig von
    // Rohmodus, `--read-only` und `--prefix none`: Alternativschirm und
    // Flags des kitty-Protokolls kommen mit der Ausgabe des Agenten, nicht
    // mit der Tastatur (Review zu GitHub #10).
    let leaves_screen = std::io::stdout().is_terminal();
    // Die Glocke klingelt nur in ein Terminal: In einer Pipe wäre ein `BEL`
    // ein fremdes Byte in der Ausgabe des Agenten. Kommt das Abonnement
    // nicht zustande, bleibt sie still; das Anhängen scheitert daran nicht.
    let bell = HoldBell::new(keys.bell && leaves_screen);
    let holds = if bell.enabled() {
        client
            .subscribe(v1::SubscribeRequest {
                since_flow_id: String::new(),
                include_passthrough: false,
                // Die Kommandozeile ist nicht die Desktop-App (HUM-539).
                desktop_app: false,
            })
            .await
            .ok()
            .map(tonic::Response::into_inner)
    } else {
        None
    };
    let mut watch = HoldWatch { holds, bell };
    // Ob der Agent das kitty-Protokoll eingeschaltet hat: geschrieben vom
    // Leser der Ausgabe, gelesen vom Leser der Tastatur.
    let kitty = Arc::new(AtomicBool::new(false));
    let theme = Theme::from_env();
    let mut view = AgentView::new(
        std::io::stdout(),
        StreamState::new(Arc::clone(&kitty)),
        keys.prefix,
        theme,
    );
    let (events_tx, mut events) = mpsc::unbounded_channel();
    let (control, forwarder) = if watches_keys {
        let Reader { control, forwarder } =
            input::spawn(tx.clone(), keys, !read_only, events_tx, kitty);
        (Some(control), forwarder)
    } else {
        (None, None)
    };
    if !read_only {
        spawn_resizes(tx.clone());
    }

    let mut screen = None;
    let ending = follow(
        &mut view,
        &mut stream,
        &mut events,
        signals,
        &mut screen,
        &mut watch,
        &Wiring {
            tx: &tx,
            writes: !read_only,
            client: &client,
            reader: control.as_ref(),
            theme,
            terminal: leaves_screen,
        },
    )
    .await;
    // Ein Bildschirm, der beim Ende noch steht (ein Signal, das Ende der
    // Sitzung), zeichnet nicht mehr, bevor die Ansicht schließt.
    if let Some(open) = screen.take() {
        open.stop().await;
    }
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
            let _ = tx.send(input::close()).await;
            drain(&mut stream).await;
        })
        .await;
    }
    // Der Leser hört auf, bevor das Terminal zurückgeht: Ein Byte danach
    // gehört dem, der das Terminal dann hat, der Shell des Menschen etwa.
    if let Some(control) = control {
        control.finish().await;
    }
    drop(raw);
    close_view(&mut view, &ending, leaves_screen);
    Ok(ending)
}

/// Die Ansicht schließt auf jedem Weg; den Schirm räumt der Befehl nur auf,
/// wo die Ausgabe ein Terminal ist (`terminal`). Nach `SIGHUP` gibt es das
/// Terminal nicht mehr: Dann schreibt der Anschluss nichts, denn jedes
/// Schreiben bekäme `EIO` (GitHub #13); der Rohmodus geht davor mit
/// `tcsetattr` zurück, ohne zu schreiben.
fn close_view<W: Write>(view: &mut AgentView<W>, ending: &Ending, terminal: bool) {
    if !matches!(ending, Ending::Signal(crate::cmd::run::SIGHUP)) && !crate::tty::terminal_gone() {
        view.leave(terminal);
    }
}

/// Was die Schleife eines Anschlusses außer Terminal und Strom braucht.
struct Wiring<'a> {
    /// Der Kanal zum Daemon.
    tx: &'a mpsc::Sender<v1::TerminalInput>,
    /// Ob dieser Anschluss schreibt; nur dann bittet er den Agenten nach
    /// einer Ansicht, neu zu zeichnen.
    writes: bool,
    /// Die Verbindung, über die auch die Einstellungen sprechen.
    client: &'a Client,
    /// Der Leser der Tastatur, wenn es einen gibt.
    reader: Option<&'a Control>,
    /// Mit oder ohne Farbe.
    theme: Theme,
    /// Ob die Ausgabe ein Terminal ist; nur dann bleibt die Hinweiszeile des
    /// Daemons aus dem Bild des Agenten (GitHub #12).
    terminal: bool,
}

/// Die Glocke eines Anschlusses und das Abonnement, aus dem sie von neu
/// gehaltenen Anfragen erfährt; ohne Glocke ohne Abonnement.
struct HoldWatch {
    /// `Subscribe`, solange es läuft.
    holds: Option<tonic::Streaming<v1::FlowEvent>>,
    /// Die Glocke mit ihrer Drossel.
    bell: HoldBell,
}

/// Ein ganzer Bildschirm über dem Agenten: die Einstellungen (GitHub #11)
/// oder der Interceptor (GitHub #12).
enum SessionScreen {
    /// Präfix und `,`.
    Settings(SettingsView),
    /// Präfix und `i`.
    Interceptor(InterceptorView),
}

impl SessionScreen {
    /// Ein Hinweis des Daemons, für die Fußzeile der Einstellungen; der
    /// Interceptor zeigt die Anfrage selbst.
    fn notice(&self, text: &str) {
        if let Self::Settings(open) = self {
            open.notice(text);
        }
    }

    /// Wird fertig, wenn der Bildschirm zu Ende ist.
    async fn finished(&mut self) {
        match self {
            Self::Settings(open) => open.finished().await,
            Self::Interceptor(open) => open.finished().await,
        }
    }

    /// Beendet den Bildschirm, bevor das Terminal zurückgeht.
    async fn stop(self) {
        match self {
            Self::Settings(open) => open.stop().await,
            Self::Interceptor(open) => open.stop().await,
        }
    }
}

/// Die Schleife eines Anschlusses: Ereignisse der Tastatur, Signale, ein
/// Bildschirm über dem Agenten, das Neuzeichnen, neu gehaltene Anfragen und
/// der Strom des Daemons, bis einer davon ihn beendet.
///
/// `SIGTERM`, `SIGHUP` und `SIGINT` gehören diesem Befehl: Ohne eigenen Weg
/// nähme der Kernel den Prozess weg, bevor `Drop` das Terminal zurückgibt,
/// und die Shell des Menschen bliebe ohne Echo stehen (`tty.rs`).
///
/// `biased` mit der Tastatur zuerst: Endet der Strom im selben Moment, in dem
/// die Taste zum Abhängen kommt, gilt die Taste, und Exit-Code und Zeile
/// hängen nicht vom Zufall der Reihenfolge ab. Neuzeichnen und Glocke stehen
/// vor dem Strom, damit ein Agent, der ohne Pause schreibt, sie nicht
/// verdrängt.
///
/// **Die Glocke steht zwischen zwei Stücken.** Jeder Zweig läuft zu Ende,
/// bevor der nächste dran ist, und ein Stück des Agenten geht in einem Zweig
/// ganz hinaus. Das `BEL` aus dem Zweig der gehaltenen Anfragen kann deshalb
/// nie in einem Stück stehen, und ein Stück des Daemons endet nie in einer
/// halben Folge.
///
/// Ein Bildschirm, der beim Ende noch steht, bleibt in `screen`; der
/// Aufrufer beendet ihn, bevor er das Terminal zurückgibt.
async fn follow<W: Write>(
    view: &mut AgentView<W>,
    stream: &mut tonic::Streaming<v1::TerminalOutput>,
    events: &mut mpsc::UnboundedReceiver<KeyEvent>,
    signals: &mut Signals,
    screen: &mut Option<SessionScreen>,
    watch: &mut HoldWatch,
    wiring: &Wiring<'_>,
) -> Ending {
    let mut code = EXIT_OK;
    let mut redraw = Redraw::default();
    // Der letzte Hinweis des Daemons, für die Fußzeile einer Ansicht, die
    // erst danach aufgeht: Eine schon gehaltene Anfrage steht dann sofort da.
    let mut last_notice: Option<String> = None;
    loop {
        tokio::select! {
            biased;
            // Ein Leser, der ohne Abhängen endet, lässt den Sender fallen;
            // dann passt das Muster nicht, und nur dieser Zweig ruht.
            Some(event) = events.recv() => match event {
                KeyEvent::Detach => return Ending::Detached,
                KeyEvent::Open(action) if takes_screen(action) => {
                    open_screen(view, screen, wiring, action, last_notice.clone());
                }
                KeyEvent::Open(_) => {
                    let _ = view.open_help(window_size());
                }
                // Über einem Vollbild-Agenten: neu zeichnen lassen
                // (ADR-0024 Punkt 9).
                KeyEvent::Close => {
                    if matches!(view.close(), Ok(true)) {
                        redraw.request();
                    }
                }
            },
            number = signals.next() => return Ending::Signal(number),
            () = finished(screen) => {
                *screen = None;
                // Auch wenn das Terminal beim Zurückkehren nicht mehr
                // schrieb: Der Agent soll sein Bild trotzdem neu zeichnen.
                if view.close().unwrap_or(true) {
                    redraw.request();
                }
                if let Some(reader) = wiring.reader {
                    reader.resume();
                }
            }
            permit = redraw.ready(wiring.tx), if wiring.writes => match permit {
                Some(permit) => redraw.step(permit, window_size()),
                None => redraw.cancel(),
            },
            event = next_hold(&mut watch.holds) => match event {
                Some(v1::FlowEvent {
                    event: Some(v1::flow_event::Event::Held(_)),
                    ..
                }) => {
                    if watch.bell.held(view, std::time::Instant::now()).is_err() {
                        return Ending::Stream(Ok(code));
                    }
                }
                Some(_) => {}
                // Ohne Abonnement schweigt die Glocke; der Anschluss läuft.
                None => watch.holds = None,
            },
            message = stream.message() => {
                if let Ok(Some(v1::TerminalOutput {
                    output: Some(v1::terminal_output::Output::Notice(text)),
                })) = &message
                {
                    last_notice = Some(text.clone());
                    if let Some(open) = screen.as_ref() {
                        open.notice(text);
                        continue;
                    }
                    // Kein Text in das Terminal des Agenten (GitHub #12): Der
                    // Hinweis steht nur in der Fußzeile einer Ansicht, und eine
                    // neu gehaltene Anfrage klingelt. In einen Mitschnitt
                    // schreibt `take` ihn weiter.
                    if wiring.terminal {
                        continue;
                    }
                }
                if let Some(outcome) = take(view, message, &mut code) {
                    return Ending::Stream(outcome);
                }
                // Eine aufgeschobene Glocke, die dieses Stück hinausschrieb,
                // zählt für die Drossel ab jetzt.
                watch.bell.settle(view, std::time::Instant::now());
            }
        }
    }
}

/// Das nächste Ereignis des Abonnements; `None`, wenn es endet oder
/// scheitert. Ohne Abonnement nie.
async fn next_hold(holds: &mut Option<tonic::Streaming<v1::FlowEvent>>) -> Option<v1::FlowEvent> {
    match holds {
        Some(stream) => stream.message().await.ok().flatten(),
        None => std::future::pending().await,
    }
}

/// Öffnet einen Bildschirm über dem Agenten: die Einstellungen oder den
/// Interceptor. Erst steht der eigene Schirm, dann zeichnet der Bildschirm.
/// Ohne Terminal gibt es keinen, und der Leser der Tastatur läuft gleich
/// weiter; er hat nach dem Ereignis angehalten und wartet auf genau eine
/// Antwort. `notice` ist der letzte Hinweis des Daemons, für die Fußzeile der
/// Einstellungen.
///
/// Kann dieser Anschluss den Agenten danach nicht neu zeichnen lassen und
/// steht der Agent auf dem Alternativschirm, öffnet sich nichts
/// ([`settings_allowed`]); ein `BEL` sagt, dass die Taste hier nicht geht.
/// Ein lesender Anschluss bekommt den Interceptor sonst nur zum Lesen.
fn open_screen<W: Write>(
    view: &mut AgentView<W>,
    screen: &mut Option<SessionScreen>,
    wiring: &Wiring<'_>,
    action: PrefixAction,
    notice: Option<String>,
) {
    if screen.is_some() {
        return;
    }
    if !settings_allowed(wiring.writes, view.agent_alt_screen()) {
        let _ = view.bell();
    } else if crate::tui::available() {
        let _ = view.open_screen();
        *screen = match action {
            PrefixAction::Interceptor => {
                InterceptorView::start(wiring.client.clone(), wiring.theme, !wiring.writes)
                    .map(SessionScreen::Interceptor)
            }
            _ => SettingsView::start(wiring.client.clone(), wiring.theme, notice)
                .map(SessionScreen::Settings),
        };
        if screen.is_some() {
            return;
        }
        let _ = view.close();
    }
    if let Some(reader) = wiring.reader {
        reader.resume();
    }
}

/// Ob ein Bildschirm über dem Agenten aufgehen darf: die Einstellungen und
/// der Interceptor.
///
/// Über einem Agenten auf dem Alternativschirm zeichnen sie über sein Bild
/// und löschen es beim Schließen; erst sein Neuzeichnen bringt es zurück.
/// Ein lesender Anschluss (`--read-only`) darf die Größe nicht ändern und
/// kann darum nicht neu zeichnen lassen: Über einem Agenten, der gerade
/// nichts schreibt, bliebe ihm ein leerer Schirm (Review zu GitHub #11).
/// Über einem Agenten auf dem gewöhnlichen Schirm stellt der eigene
/// Alternativschirm das Bild ohnehin wieder her.
const fn settings_allowed(writes: bool, agent_alt_screen: bool) -> bool {
    writes || !agent_alt_screen
}

/// Wird fertig, wenn der Bildschirm über dem Agenten zu Ende ist; ohne
/// Bildschirm nie.
async fn finished(screen: &mut Option<SessionScreen>) {
    match screen {
        Some(open) => open.finished().await,
        None => std::future::pending().await,
    }
}

/// Wie ein Anschluss endet.
pub(crate) enum Ending {
    /// Der Strom ist zu Ende: die Sitzung, ein Befund, oder das Ende der
    /// Eingabe an einer Pipe.
    Stream(Result<u8, Failure>),
    /// Der Mensch hat sich mit der Taste abgehängt.
    Detached,
    /// Ein Signal mit dieser Nummer.
    Signal(u8),
}

impl Ending {
    /// Ein Ende, das nur so aussieht, weil das Terminal weg ist, ist
    /// `SIGHUP` (GitHub #13): Geht das Terminal zu, kann das Ende der Eingabe
    /// (ein lesender Anschluss hängt dann ab) oder ein Schreiben, das nicht
    /// mehr geht, vor dem Signal ankommen. Danach schreibt niemand mehr
    /// hinein, und das Signal entscheidet, ob die Sitzung stoppt.
    ///
    /// Das gilt auch für einen Befund: Ein `TERM_001`, das erst ankommt,
    /// wenn das Terminal schon zu ist, führte sonst in den lesenden
    /// Anschluss, und dessen Hinweis an ein Terminal, das es nicht mehr gibt,
    /// bekäme `EIO` (GitHub #73).
    ///
    /// Für `humanitl run` und `humanitl go`, deren Sitzung am Signal hängt;
    /// `sandbox attach` endet an einem verschwundenen Terminal weiter mit 0,
    /// schreibt aber auch nichts mehr ([`finish_with`]).
    pub(crate) fn or_hung_up(ended: Result<Self, Failure>) -> Result<Self, Failure> {
        Self::hung_up_when(ended, crate::tty::terminal_gone())
    }

    /// [`Ending::or_hung_up`] mit der Antwort auf die Frage, ob das Terminal
    /// weg ist (`gone`). Ein Signal bleibt, was es ist.
    fn hung_up_when(ended: Result<Self, Failure>, gone: bool) -> Result<Self, Failure> {
        match ended {
            Ok(Self::Signal(number)) => Ok(Self::Signal(number)),
            _ if gone => Ok(Self::Signal(crate::cmd::run::SIGHUP)),
            other => other,
        }
    }
}

/// Liest den Strom bis zu seinem Ende und verwirft, was kommt.
async fn drain(stream: &mut tonic::Streaming<v1::TerminalOutput>) {
    while let Ok(Some(_)) = stream.message().await {}
}

/// Der Befund für eine Angabe, die keine Präfixtaste nennt; `source` ist der
/// Schalter oder der Schlüssel, aus dem sie kam.
fn invalid_prefix(source: &str, spec: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!(
                "{source} {spec:?} names no prefix key; allowed is {}",
                DetachKeys::ALLOWED
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl sandbox attach --prefix {}",
                DetachKeys::DEFAULT_SPEC
            )))
            .build(),
    )
}

/// Eine Nachricht des Stroms: Bytes auf das Terminal (über `view`), der
/// Exit-Code in `code`. `Some` mit dem Ausgang, wenn der Strom damit endet.
fn take<W: Write>(
    view: &mut AgentView<W>,
    message: Result<Option<v1::TerminalOutput>, tonic::Status>,
    code: &mut u8,
) -> Option<Result<u8, Failure>> {
    let message = match message {
        Ok(Some(message)) => message,
        Ok(None) => return Some(Ok(*code)),
        Err(status) => {
            return Some(Err(Failure::new(status_diagnostic(&status, "Terminal"))));
        }
    };
    match message.output {
        // Ein Terminal, das nicht mehr schreibt, beendet den Anschluss.
        Some(v1::terminal_output::Output::Data(data)) => view.agent(&data).err().map(|_| Ok(*code)),
        // Die Hinweiszeile des Daemons steht neben den Bytes und nicht darin
        // (`ipc/src/terminal.rs`). Hierher kommt sie nur noch, wenn die
        // Ausgabe kein Terminal ist ([`follow`], GitHub #12): Ein Mitschnitt
        // hat keine zweite Fläche, also steht sie zwischen den Bytes des
        // Agenten, an der Grenze, an der der Daemon sie geschickt hat, und in
        // ihrer eigenen Zeile.
        Some(v1::terminal_output::Output::Notice(text)) => {
            view.agent(&notice_line(&text)).err().map(|_| Ok(*code))
        }
        Some(v1::terminal_output::Output::Exit(exit)) => {
            *code = u8::try_from(exit.code).unwrap_or(1);
            None
        }
        // Ein Befund beendet den Strom des Clients; der Daemon schickt ihn
        // genau dann, wenn er nichts mehr liefern wird. Einen Code, den diese
        // Fassung nicht kennt, meldet sie nicht als eigenen Fehlschlag --
        // dieselbe Regel wie in `humanitl run`.
        Some(v1::terminal_output::Output::Diagnostic(diagnostic)) => {
            Some(from_proto(&diagnostic).map_or(Ok(*code), |rebuilt| Err(Failure::new(rebuilt))))
        }
        // Die Geometrie des Schreibers. Ein Leser rendert letterboxed; dieser
        // Befehl schreibt in ein Terminal, das seine eigene Größe hat, und
        // lässt sie deshalb stehen.
        Some(v1::terminal_output::Output::Resize(_)) | None => None,
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
#[derive(Debug)]
pub(crate) struct Signals {
    /// Die Ströme und ihre Nummern, in der Reihenfolge `TERM`, `HUP`, `INT`.
    streams: Vec<(tokio::signal::unix::Signal, u8)>,
}

impl Signals {
    /// Meldet die Handler an. Was sich nicht anmelden lässt, fehlt still; dann
    /// gilt für dieses eine Signal das Standardverhalten.
    pub(crate) fn listen() -> Self {
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

    /// Das Signal, das schon wartet, ohne zu warten; `None` ohne.
    pub(crate) fn pending(&mut self) -> Option<u8> {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        self.streams.iter_mut().find_map(|(stream, number)| {
            stream.poll_recv(&mut context).is_ready().then_some(*number)
        })
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
            if tx.send(resize(window_size())).await.is_err() {
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
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes;
    use humanitl_core::{Diagnostic, Severity};

    use super::{Ending, settings_allowed};
    use crate::cmd::Failure;
    use crate::cmd::run::SIGHUP;

    fn taken() -> Failure {
        Failure::new(
            Diagnostic::builder(codes::TERM_001, Severity::Error)
                .why("another client writes".to_owned())
                .build(),
        )
    }

    fn signal(ended: &Result<Ending, Failure>) -> Option<u8> {
        match ended {
            Ok(Ending::Signal(number)) => Some(*number),
            _ => None,
        }
    }

    /// Ist das Terminal weg, ist jedes Ende `SIGHUP`, auch ein Befund wie
    /// `TERM_001`; sonst führte er in den lesenden Anschluss und dessen
    /// Hinweis an ein Terminal, das es nicht mehr gibt (GitHub #73). Ein
    /// Signal bleibt, und bei stehendem Terminal bleibt jedes Ende.
    ///
    /// Mutation proof: leaving `Err` and `Stream(Err)` out of the gone arm of
    /// `hung_up_when` keeps the findings and fails the first two asserts.
    #[test]
    fn every_ending_at_a_gone_terminal_is_a_hangup() {
        let gone = |ended| signal(&Ending::hung_up_when(ended, true));
        assert_eq!(gone(Err(taken())), Some(SIGHUP));
        assert_eq!(gone(Ok(Ending::Stream(Err(taken())))), Some(SIGHUP));
        assert_eq!(gone(Ok(Ending::Stream(Ok(0)))), Some(SIGHUP));
        assert_eq!(gone(Ok(Ending::Detached)), Some(SIGHUP));
        assert_eq!(gone(Ok(Ending::Signal(15))), Some(15));

        let kept = |ended| Ending::hung_up_when(ended, false);
        assert!(
            matches!(kept(Err(taken())), Err(found) if found.diagnostic.code == codes::TERM_001)
        );
        assert!(matches!(kept(Ok(Ending::Detached)), Ok(Ending::Detached)));
        assert!(matches!(
            kept(Ok(Ending::Stream(Ok(3)))),
            Ok(Ending::Stream(Ok(3)))
        ));
    }

    /// Ein schreibender Anschluss öffnet die Einstellungen immer, ein
    /// lesender nur über einem Agenten auf dem gewöhnlichen Schirm (Review
    /// zu GitHub #11).
    #[test]
    fn a_reader_opens_the_settings_only_over_the_ordinary_screen() {
        assert!(settings_allowed(true, true));
        assert!(settings_allowed(true, false));
        assert!(settings_allowed(false, false));
        assert!(!settings_allowed(false, true));
    }
}
