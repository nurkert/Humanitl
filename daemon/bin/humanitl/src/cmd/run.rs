//! `humanitl run`: eine Sitzung im Projektverzeichnis starten und ihren
//! Exit-Code weitergeben.
//!
//! Das ist der Befehl, mit dem ein Mensch dieses Werkzeug zum ersten Mal
//! benutzt. Er tut fünf Dinge, und zwar in dieser Reihenfolge:
//!
//! 1. **Das Profil der Sitzung auflösen** ([`Context::config`]) — als
//!    erstes, bevor irgendetwas läuft. Das ist der Riegel gegen ein
//!    feindliches Projekt-Profil: Ein `.humanitl/profile.toml`, das Host-Pfade
//!    einhängen will oder einen gesperrten Schlüssel setzt, verweigert hier
//!    den Start mit `CONFIG_003` (`backlog/CONVENTIONS.md` 4.23).
//! 2. **Den Daemon verbinden** und seine Vertragsversion prüfen. Ohne Daemon
//!    gibt es keinen Proxy, keine Aufzeichnung und keine Sandbox;
//!    `DAEMON_001` sagt, wie man ihn startet.
//! 3. **Die Sitzung starten**: `Sandbox(Start)` mit dem Profil dieser
//!    Sitzung, dem Projektverzeichnis, dem Frage-Modus und den
//!    Konfigurationswerten der Kommandozeile. Der Daemon löst daraufhin für
//!    genau diese Sitzung neu auf und baut Regelspeicher, Haltefrist und
//!    Durchreiche daraus (HUM-067).
//! 4. **Die drei Garantien zeigen**, sobald der Daemon sie gemessen hat. Eine
//!    rote Prüfung beendet den Lauf mit Exit 3.
//! 5. **Die Ausgabe des Agenten durchreichen** und mit seinem Exit-Code enden.
//!
//! # Ein eigenes Kommando an einem Terminal (HUM-280)
//!
//! `humanitl run -- bash` mit einem Terminal auf `stdin` hängt dieses Terminal
//! an die Sitzung wie `humanitl sandbox attach`: Rohmodus, Fenstergröße,
//! Eingabe an die Shell und die Taste zum Abhängen von HUM-279
//! (`terminal.prefix`, GitHub #10). Der Anschluss beginnt, sobald der Daemon
//! `running` meldet; die Bytes kommen dann über die `Terminal`-RPC, samt dem
//! Rückstand, und nicht mehr über den Ereignisstrom. `Ctrl+C` ist im
//! Rohmodus ein Byte für die Shell, die es als steuerndes Terminal hat;
//! `exit` beendet die Sitzung, `Ctrl+B d` hängt ab und lässt sie laufen.
//! Mit `--ask terminal` oder `--json` und ohne Terminal bleibt es beim
//! Mitlesen unten.
//!
//! # Was dieser Befehl sonst nicht tut
//!
//! Ohne eigenes Kommando oder ohne Terminal verbindet er dieses Terminal nicht
//! mit dem des Agenten. Der Daemon startet die Sitzung seit HUM-042 zwar an
//! einem Pseudoterminal, aber dieser Befehl liest nur mit: Seine Ausgabe kommt
//! als Bytes über den Ereignisstrom und geht unverändert auf `stdout` dieses
//! Prozesses; gefiltert wird sie im Daemon ([`humanitl_core::TerminalFilter`],
//! Politik `ColourOnly`). Damit gibt es dann keinen Raw-Modus, keine
//! Weiterleitung der Fenstergröße, keine Eingabe an den Agenten — das alles
//! hat `humanitl sandbox attach`, und `--ask terminal` verweigert weiterhin
//! den Dienst mit `CLI_002`.
//!
//! Zwei Dinge folgen aus dem Pseudoterminal und sind hier sichtbar: Ein
//! Terminal hat einen Strom, also kommt alles auf `stdout` und nichts auf
//! `stderr`, und seine Zeilendisziplin macht aus `\n` ein `\r\n`.
//!
//! `Ctrl+C` beendet die Sitzung über `Sandbox(Stop)`; ohne Eingabekanal
//! erreicht kein Byte den Agenten. Wer tippen will, hängt sich mit
//! `humanitl sandbox attach` an dieselbe Sitzung.

use std::ffi::OsString;

use humanitl_config::{AskMode, Config, OnClose, Resolved};
use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::session::{
    AGENT_ADAPTER_KEY, SANDBOX_MIN_ISOLATION_KEY, SESSION_OVERRIDE_KEYS, ask_mode_name,
};
use humanitl_ipc::v1;
use humanitl_sandbox::agent::{AdapterRegistry, AgentAdapter};
use serde_json::json;
use std::io::Write as _;

use crate::cli::RunArgs;

mod wish;
use crate::cmd::{Context, EXIT_CHECK, EXIT_USER, Failure, from_proto, status_diagnostic};

/// Ab dieser Minor kennt der Daemon `auto` in `Start.sandbox_mode`
/// (HUM-357).
const AUTO_MODE_MINOR: u32 = 27;

/// `Start.sandbox_mode` dieses Laufs (HUM-350, HUM-357): `--disposable` oder
/// `--direct` für diesen einen Lauf, sonst `sandbox.mode` der Konfiguration,
/// wie dieser Aufruf sie auflöst. Die Auflösung von `auto` gegen das System
/// macht der Daemon; ein Daemon vor Minor 27 kennt das Wort nicht und bekommt
/// ein leeres Feld, also seine eigene Konfiguration, die dort `direct` oder
/// `disposable` sein kann, aber nie `auto`.
fn start_mode(
    args: &RunArgs,
    wished: Option<humanitl_config::SandboxMode>,
    daemon_minor: u32,
) -> String {
    use humanitl_config::SandboxMode;
    // Nur ein Wunsch dieses Laufs reist; ohne ihn löst der Daemon auf, mit
    // der Projektwahl (HUM-532, [`wish`]).
    let mode = if args.disposable {
        SandboxMode::Disposable
    } else if args.direct {
        SandboxMode::Direct
    } else if let Some(wished) = wished {
        wished
    } else {
        return String::new();
    };
    if mode == SandboxMode::Auto && daemon_minor < AUTO_MODE_MINOR {
        return String::new();
    }
    mode.as_str().to_owned()
}

/// Führt `humanitl run` aus.
///
/// # Errors
///
/// `CONFIG_001` bis `CONFIG_003`, wenn das Profil oder ein Flag nicht stimmt,
/// `CLI_002` für `--ask terminal`, `DAEMON_001` ohne Daemon, `DAEMON_002` bei
/// einer anderen Vertrags-Major und die Befunde des Daemons für alles, was am
/// Start scheitert.
pub async fn run(ctx: &Context, args: &RunArgs) -> Result<u8, Failure> {
    launch(ctx, args, &mut Launch::default()).await
}

/// Was `humanitl go` anders macht als `humanitl run` (HUM-496), und was es
/// danach wissen muss.
///
/// `run` und `go` teilen sich [`launch`] und [`drive`]: Auflösen, Verbinden,
/// Start, Anschluss und Signale gibt es nur einmal.
#[derive(Debug, Default)]
pub(crate) struct Launch {
    /// `humanitl go`: das Terminal auch ohne eigenes Kommando anhängen und
    /// die Ladeanimation zeigen, wo sie erwünscht ist.
    pub go: bool,
    /// `humanitl go`: der Ordner, den dieser Aufruf gewählt hat, und woher
    /// er kommt (#8); `None` heißt, wie bei `run` aus der Auflösung.
    pub folder: Option<(std::path::PathBuf, humanitl_sandbox::WorkDirSource)>,
    /// Die Palette der Ladeanimation, wenn sie erscheint; [`launch`] setzt
    /// sie nach der Konfiguration.
    pub splash: Option<crate::splash::Palette>,
    /// `true`, wenn der Daemon nach dem Lauf wartende Änderungen der
    /// Wegwerf-Schicht gemeldet hat.
    pub changes_waiting: bool,
    /// `humanitl go`: Die Zeile des Erststarts steht schon da (#9); sie nennt
    /// den Frage-Modus, und der Hinweis zu Anfragen ohne Regel entfällt.
    pub first_line: bool,
    /// `humanitl go`: was `SIGHUP` mit der Sitzung tut
    /// (`terminal.on_close`, GitHub #13); [`launch`] setzt es nach der
    /// Konfiguration. Unter `run` ohne Bedeutung.
    pub on_close: OnClose,
    /// Das Signal, auf das der Befehl endete (GitHub #13). Nach `SIGHUP`
    /// ist das Terminal weg, und danach schreibt niemand mehr hinein, weder
    /// JSON noch die Frage nach den Änderungen ([`Launch::hung_up`]).
    pub signalled: Option<u8>,
    /// `humanitl go`: die Signale, die schon vor der ersten Anfrage an den
    /// Daemon standen (`go::rejoin`, GitHub #13). [`drive`] fragt sie, bevor
    /// es startet, damit ein Signal aus der Lücke bis zu seinen eigenen
    /// Strömen nicht verloren geht.
    pub early_signals: Option<crate::cmd::attach::Signals>,
}

impl Launch {
    /// Ob der Befehl auf `SIGHUP` endete: Sein Terminal ist weg.
    pub(crate) fn hung_up(&self) -> bool {
        self.signalled == Some(SIGHUP)
    }

    /// Was ein Signal an diesem Lauf mit der Sitzung tut.
    pub(crate) const fn on_signal(&self) -> OnSignal {
        OnSignal {
            go: self.go,
            on_close: self.on_close,
        }
    }
}

/// Die Nummer von `SIGHUP`: Das Terminal des Befehls ist geschlossen.
pub(crate) const SIGHUP: u8 = 1;

/// Was ein Signal an einem Lauf mit seiner Sitzung tut (HUM-280, GitHub #13).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OnSignal {
    /// `humanitl go` und nicht `humanitl run`.
    pub go: bool,
    /// `terminal.on_close`; gilt nur unter `go`.
    pub on_close: OnClose,
}

impl OnSignal {
    /// Ob das Signal `number` die Sitzung stoppt.
    ///
    /// `owns`: Dieser Befehl hat die Sitzung gestartet, oder er hängt als
    /// Schreiber an der Sitzung, an die sich ein zweites `go` gehängt hat.
    /// Ein Lauf, der seine eigene Sitzung nur noch lesend sieht, weil ein
    /// anderer Client schreibt (`TERM_001`), besitzt sie weiter (GitHub #13,
    /// Review); nur ein reiner Zuschauer (`sandbox attach --read-only`, ein
    /// zweites `go` als Leser) stoppt nie. Nein auch für `SIGHUP` an
    /// `humanitl go` mit `terminal.on_close = "detach"`: Das Terminal ist
    /// geschlossen, die Sitzung läuft im Daemon weiter. `humanitl run` liest
    /// den Schlüssel nie und stoppt auf jedes dieser Signale; `SIGTERM` und
    /// `SIGINT` stoppen auch unter `go`.
    pub(crate) const fn stops(self, owns: bool, number: u8) -> bool {
        let detaches = self.go && number == SIGHUP && matches!(self.on_close, OnClose::Detach);
        owns && !detaches
    }
}

/// [`drive`] hat die Sitzung gestartet und besitzt sie, auch wenn sein
/// Terminal nur noch zusieht ([`OnSignal::stops`]).
const STARTED: bool = true;

/// Führt einen Lauf aus, für `humanitl run` und `humanitl go`.
///
/// # Errors
///
/// Wie [`run`].
pub(crate) async fn launch(
    ctx: &Context,
    args: &RunArgs,
    launch: &mut Launch,
) -> Result<u8, Failure> {
    // Zuerst und immer: Das Projekt-Profil wird gelesen, bevor irgendetwas
    // startet.
    let resolved = ctx.config()?;
    ctx.render.detail(&session_lines(&resolved, args));

    // Beide Seiten müssen ein Terminal sein: Die Tasten kommen von `stdin`,
    // und der Kasten geht nach `stderr`. Ein Lauf mit `2>log` schriebe ihn
    // samt seiner Steuerfolgen in eine Datei, und niemand sähe die Frage.
    let on_terminal = std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stderr());
    let named_config = ctx.config_file.is_some();
    // Vor dem Verbinden nur, was keine Projektwahl mehr ändert: ein Wunsch
    // dieses Laufs. Ein Frage-Modus aus `config.toml` kann die Wahl des
    // Projekts überstimmen und wird erst danach geprüft (HUM-532); ohne
    // Daemon gilt die Ablehnung trotzdem, statt `DAEMON_001`.
    let local = terminal_refusals(&resolved.config, launch.go, args, on_terminal);
    let deferred = match local {
        Err(refused) if wish::per_run(&resolved, "hold.ask_mode", named_config) => {
            return Err(refused);
        }
        Err(refused) => Some(refused),
        Ok(()) => None,
    };
    let mut client = ctx
        .connect()
        .await
        .map_err(|unreachable| deferred.unwrap_or(unreachable))?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;

    let (work_dir, work_dir_source) = folder_of(ctx, &resolved, launch);
    let here = (work_dir.as_path(), info.proto_minor, on_terminal);
    if let Some(code) = Box::pin(rejoin(ctx, &mut client, &resolved, args, launch, here)).await? {
        return Ok(code);
    }
    // Die Werte der Sitzung stehen vor dem Erststart fest: Ein Wert, den
    // dieser Daemon ablehnte, beendet den Lauf, bevor eine Wahl gespeichert
    // ist, und der Vorschlag rechnet mit genau den Werten des Starts.
    let cli_overrides = session_overrides(ctx, &resolved, info.proto_minor)?;
    let first = (
        work_dir.as_path(),
        cli_overrides.as_slice(),
        info.proto_minor,
    );
    if let Some(code) = Box::pin(first_start(ctx, &mut client, launch, args, first)).await? {
        return Ok(code);
    }
    let chosen = Box::pin(wish::chosen_ask_mode(
        &mut client,
        &resolved,
        named_config,
        &work_dir,
        info.proto_minor,
    ))
    .await;
    let effective = with_chosen_ask_mode(&resolved, chosen, launch.go, args, on_terminal)?;
    let config = &effective;

    // Der gespeicherte Ordner, `--work` für diesen einen Aufruf, und das
    // aktuelle Verzeichnis nur, wenn beides fehlt (HUM-293). Die Quelle geht
    // mit, damit der Daemon sie in einem `SANDBOX_006` nennt.
    let start = v1::sandbox_request::Start {
        // Das bwrap-Profil bleibt der Konfiguration überlassen; unter
        // `humanitl run` benennt `--profile` das Profil der Sitzung
        // (`backlog/CONVENTIONS.md` 4.23).
        profile: String::new(),
        work_dir: work_dir.display().to_string(),
        work_mode: wish::work_mode(&resolved, named_config),
        command: args
            .cmd
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        session_profile: ctx.profile_flag().unwrap_or_default().to_owned(),
        ask_mode: wish::ask_mode(&resolved, named_config),
        cli_overrides,
        work_dir_source: Some(humanitl_ipc::work_dir::source_to_proto(&work_dir_source)),
        sandbox_mode: start_mode(args, wish::mode(&resolved, named_config), info.proto_minor),
        // `--llm NAME`: der gespeicherte Server für genau diesen Lauf
        // (HUM-400). Ein älterer Daemon kennt das Feld nicht und nähme die
        // Wahl ohne ihn; das wäre ein anderer Server als der verlangte.
        llm_server: run_server(ctx, info.proto_minor)?,
        // Was der Mensch zum Hinweis auf Cloud-Inferenz gesagt hat, setzt
        // `cloud::prepare_start` (HUM-516).
        // `project_id` bleibt leer: `humanitl run` startet mit dem Ordner.
        ..v1::sandbox_request::Start::default()
    };

    let session = session_value(ctx, args, &work_dir, config);

    // Vor dem ersten Start mit einem Agenten, dessen Inferenz an einen
    // Anbieter außerhalb dieses Rechners geht, einmal nachfragen (HUM-516).
    // Ob das so ist und ob es schon bestätigt ist, sagt der Daemon für genau
    // diesen Start, `--llm URL` eingeschlossen; er lehnt einen Start ohne
    // Antwort selbst ab (`LLM_011`).
    let mut start = start;
    let mode = crate::cmd::cloud::mode(args.yes, ctx.render.is_json(), on_terminal);
    if let Some(code) = crate::cmd::cloud::prepare_start(
        &mut client,
        &mut start,
        info.proto_minor,
        mode,
        (&ctx.paths, config),
    )
    .await?
    {
        return Ok(code);
    }

    if !launch.first_line {
        ctx.render.note(&where_decisions_happen(config));
    }
    let moderate = config.hold.ask_mode == AskMode::Terminal;
    let keys = detach_keys(ctx, args, launch.go, moderate)?;
    // Die Ladeanimation nur für `go` und nie neben dem Kasten von
    // `--ask terminal`, der dasselbe Terminal braucht (HUM-496).
    if launch.go && !moderate {
        launch.splash = crate::cmd::go::splash_palette(config, ctx.render.is_json());
    }
    let code = drive(
        ctx,
        &mut client,
        start,
        moderate,
        keys,
        info.proto_minor,
        launch,
    )
    .await?;
    report(ctx, launch, session, code);
    Ok(code)
}

/// Der JSON-Wert am Ende eines Laufs mit `--json`. Nach `SIGHUP` gibt es
/// kein Terminal mehr, in das etwas ginge (GitHub #13).
fn report(ctx: &Context, launch: &Launch, mut session: serde_json::Value, code: u8) {
    if ctx.render.is_json() && !launch.hung_up() {
        session["exit_code"] = json!(code);
        ctx.render.value(&session);
    }
}

/// Setzt [`Launch::on_close`] aus der Konfiguration.
///
/// `humanitl go` in einem Ordner, dessen Sitzung läuft, hängt sich an sie an,
/// statt zu starten; wartende Änderungen dort fragt es vorher ab (GitHub
/// #13, [`crate::cmd::go::rejoin`]). Ob die Sitzung hier läuft, sagt der
/// Daemon. Unter `run` nichts. `Some(code)`: fertig, nichts startet.
async fn rejoin(
    ctx: &Context,
    client: &mut Client,
    resolved: &Resolved,
    args: &RunArgs,
    launch: &mut Launch,
    (work_dir, minor, on_terminal): (&std::path::Path, u32, bool),
) -> Result<Option<u8>, Failure> {
    launch.on_close = resolved.config.terminal.on_close;
    if !launch.go {
        return Ok(None);
    }
    let here = crate::cmd::go::Here {
        work_dir,
        minor,
        attaching: attaches(
            args,
            true,
            resolved.config.hold.ask_mode == AskMode::Terminal,
            ctx.render.is_json(),
            stdin_is_terminal(),
        ),
        asking: on_terminal && !ctx.render.is_json(),
        wishes: !args.cmd.is_empty()
            || args.disposable
            || args.direct
            || ctx
                .cli_config
                .iter()
                .any(|(key, _)| key == AGENT_ADAPTER_KEY || key == SANDBOX_MIN_ISOLATION_KEY),
    };
    Box::pin(crate::cmd::go::rejoin(ctx, client, launch, here)).await
}

/// Die Präfixtaste, wenn dieser Lauf sein Terminal anhängt (GitHub #10).
///
/// A command of the caller at a terminal works like `sandbox attach`
/// (HUM-280). The key is read before the start, so a typo in
/// `terminal.prefix` stops nothing that already runs.
fn detach_keys(
    ctx: &Context,
    args: &RunArgs,
    go: bool,
    moderate: bool,
) -> Result<Option<crate::prefix::Keys>, Failure> {
    if attaches(
        args,
        go,
        moderate,
        ctx.render.is_json(),
        stdin_is_terminal(),
    ) {
        Ok(Some(crate::cmd::attach::resolve_keys(ctx, None)?))
    } else {
        Ok(None)
    }
}

/// Der erste `go` in einem Ordner, den das Register nicht kennt: der
/// Erststart-Bildschirm oder die eine Zeile, und die Wahl gespeichert, bevor
/// irgendetwas sie liest (#9). `Some(code)`: abgebrochen, nichts startet.
/// Unter `run` nichts.
async fn first_start(
    ctx: &Context,
    client: &mut Client,
    launch: &mut Launch,
    args: &RunArgs,
    (work_dir, overrides, minor): (&std::path::Path, &[v1::sandbox_request::CliOverride], u32),
) -> Result<Option<u8>, Failure> {
    use crate::cmd::go::Next;

    if !launch.go {
        return Ok(None);
    }
    match crate::cmd::go::first_start(ctx, client, (work_dir, overrides), minor, args.yes).await? {
        Next::Start { said } => {
            launch.first_line = said;
            Ok(None)
        }
        Next::Stop(code) => Ok(Some(code)),
    }
}

/// Der Ordner dieses Laufs und woher er kommt: unter `go` der, den der
/// Aufruf gewählt hat (das aktuelle Verzeichnis oder `--dir`, nicht der
/// gespeicherte Ordner, #8); sonst wie [`Context::work_dir`].
fn folder_of(
    ctx: &Context,
    resolved: &Resolved,
    launch: &Launch,
) -> (std::path::PathBuf, humanitl_sandbox::WorkDirSource) {
    launch
        .folder
        .clone()
        .unwrap_or_else(|| ctx.work_dir(resolved))
}

/// Ob `stdin` ein Terminal ist.
fn stdin_is_terminal() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdin())
}

/// Ob dieser Lauf sein Terminal an die Sitzung hängt (HUM-280): ein eigenes
/// Kommando, ein Terminal auf `stdin`, kein Kasten von `--ask terminal`, der
/// dieselbe Tastatur bräuchte, und keine JSON-Ausgabe auf `stdout`.
///
/// `humanitl go` hängt auch den Agenten der Konfiguration an (`any`,
/// HUM-496): Man landet in seinem Terminal wie nach einem nativen Start.
// Vier Wahrheitswerte sind hier die Frage selbst: Ein Aufzählungstyp je
// Bedingung machte den Aufruf länger und nicht klarer.
#[allow(clippy::fn_params_excessive_bools)]
const fn attaches(args: &RunArgs, any: bool, moderate: bool, json: bool, terminal: bool) -> bool {
    (any || !args.cmd.is_empty()) && terminal && !moderate && !json
}

/// Der Anschluss an das Terminal der Sitzung, solange er läuft.
type Attached<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<crate::cmd::attach::Ending, Failure>> + 'a>,
>;

/// Das Ende des Anschlusses, oder nie.
async fn next_attached(
    attached: &mut Option<Attached<'_>>,
) -> Result<crate::cmd::attach::Ending, Failure> {
    match attached {
        Some(future) => future.await,
        None => std::future::pending().await,
    }
}

/// Ob dieses Ereignis sagt, dass die Sitzung läuft.
fn is_running(event: &v1::SandboxEvent) -> bool {
    matches!(
        event.event.as_ref(),
        Some(v1::sandbox_event::Event::Status(status))
            if status.state == v1::SandboxState::Running as i32
    )
}

/// Startet die Sitzung und begleitet sie bis zum Ende des Agenten.
///
/// Mit `keys` hängt sich der Lauf an das Terminal der Sitzung, sobald sie
/// läuft (HUM-280); die Ausgabe kommt dann von dort und nicht vom
/// Ereignisstrom.
#[allow(clippy::too_many_lines)]
async fn drive(
    ctx: &Context,
    client: &mut Client,
    start: v1::sandbox_request::Start,
    moderate: bool,
    keys: Option<crate::prefix::Keys>,
    minor: u32,
    launch: &mut Launch,
) -> Result<u8, Failure> {
    // **Erst die Signale, dann alles andere.** `open_moderation` setzt für
    // `--ask terminal` den Rohmodus (`cmd/moderate.rs`), und danach dauert der
    // Start der Sandbox Hunderte Millisekunden. Ein `SIGTERM` in dieser Lücke
    // nähme der Kernel mit seinem Standardverhalten: Der Prozess wäre weg, das
    // Terminal bliebe roh, und die Shell des Menschen stünde ohne Echo da --
    // genau das, was `src/tty.rs` ausschließt. Die Ströme stehen deshalb,
    // bevor irgendjemand das Terminal anfasst (HUM-135).
    let mut terminate = signal_stream(tokio::signal::unix::SignalKind::terminate());
    let mut hangup = signal_stream(tokio::signal::unix::SignalKind::hangup());
    // **Einmal angelegt und nicht je Umlauf.** `tokio::signal::ctrl_c()` baut
    // bei jedem Aufruf einen frischen Empfänger; ein `SIGINT`, das eintrifft,
    // während die Schleife gerade arbeitet -- ein Schwall Ausgabe, ein
    // offener Editor --, fällt zwischen den alten und den neuen Empfänger und
    // wird nie gesehen. Ein Strom, der von Anfang an steht, puffert es.
    let mut interrupt = signal_stream(tokio::signal::unix::SignalKind::interrupt());

    // **Erst das Abonnement, dann der Start.** Der Daemon liefert keinen
    // Rückstand: `Subscribe` mit leerem `since_flow_id` beginnt bei jetzt.
    // Ein Agent, der seine erste Anfrage in der ersten Sekunde stellt --
    // OpenCode ruft beim Start seinen Katalog ab --, hielte sie also, bevor
    // dieser Strom stünde, und niemand am Terminal erführe davon; der Agent
    // liefe in die Frist.
    let (mut flows, mut moderation) = open_moderation(client, moderate).await?;

    // Ein Signal, das bis hierher kam, beendet den Befehl, bevor
    // `Sandbox(Start)` hinausgeht: aus den Merkern eines Bildschirms (#9),
    // oder aus den Strömen oben, während die Moderation aufgebaut wurde.
    // Dazu die Signale, die `go` vor seiner ersten Anfrage angemeldet hat
    // (GitHub #13); danach stehen die eigenen.
    let early = launch
        .early_signals
        .take()
        .and_then(|mut early| early.pending());
    if let Some(code) = stop_before_start(
        crate::tui::signals::take(),
        [
            arrived(&mut terminate),
            arrived(&mut hangup),
            arrived(&mut interrupt),
        ],
    )
    .or(early.map(|number| 128_u8.saturating_add(number)))
    {
        // Nach `SIGHUP` schreibt auch `--json` nichts mehr.
        launch.signalled = Some(code.saturating_sub(128));
        return Ok(code);
    }

    // Die Ladeanimation von `humanitl go` steht ab hier, nach den Signalen
    // und vor dem Start; solange sie steht, hält `hold` die Ereignisse des
    // Starts zurück (HUM-496, `cmd/hold.rs`).
    let mut hold = crate::cmd::hold::Hold::start(launch.splash);

    let mut events = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(start)),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox(Start)")))?
        .into_inner();

    let mut failure: Option<Failure> = None;
    let mut exit: Option<i32> = None;
    let mut last_refusals: Option<v1::sandbox_event::Refusals> = None;
    let mut interrupted = false;
    // HUM-280: the key waits here until the session runs, then the attach
    // takes this terminal. While it runs, the output comes through it.
    let mut pending_keys = keys;
    let mut attached: Option<Attached<'_>> = None;
    let mut through_terminal = pending_keys.is_some();
    let mut events_open = true;
    let mut attach_code: Option<u8> = None;
    // A writer that is already there (the app's pane, `TERM_001`) leaves this
    // terminal a reader: it still shows the backlog and the session, and the
    // detach key still works (HUM-280). The session stays this run's: a
    // signal stops it as if it still wrote (GitHub #13, review).
    let mut reader_keys = keys;

    let mut clock = tokio::time::interval(std::time::Duration::from_secs(1));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Ein zweiter Client für den Stopp: Der erste hält den Ereignisstrom, und
    // ein `&mut` daran wäre für die Dauer der Schleife geliehen.
    let mut stopper = client.clone();
    // **Gelesen wird bis zum Ende des Stroms, nicht bis zum Exit-Code.** Der
    // Daemon schickt nach dem Exit noch, was der Lauf im Projekt hinterlassen
    // hat: die Befunde `SANDBOX_022` bis `SANDBOX_026` und die Zusammenfassung
    // selbst (HUM-043). Wer beim Exit-Code aufhört, beendet den Strom, bevor
    // sie kommen — dann steht kein Wort darüber im Terminal, dass der Agent
    // einen Git-Hook geschrieben hat. Das kostet die Zeit des zweiten
    // Schnappschusses; der Strom endet unmittelbar danach.
    loop {
        let deadline = hold.deadline();
        let next = if let Some(event) = hold.release() {
            Ok(Some(event))
        } else if !events_open && attached.is_none() && !hold.showing() {
            break;
        } else {
            tokio::select! {
            event = events.message(), if events_open => event,
            // Die Mindestzeit der Ladeanimation ist um (HUM-496).
            () = crate::cmd::hold::until(deadline), if deadline.is_some() => {
                hold.finish().await;
                continue;
            }
            // Der Anschluss an das Terminal ist zu Ende (HUM-280).
            ended = next_attached(&mut attached), if attached.is_some() => {
                attached = None;
                let ended = crate::cmd::attach::Ending::or_hung_up(ended);
                match ended {
                    // Abgehängt: Die Sitzung läuft weiter, und dieser Befehl
                    // endet, ohne sie zu stoppen.
                    Ok(crate::cmd::attach::Ending::Detached) => {
                        return crate::cmd::attach::finish(
                            ctx,
                            crate::cmd::attach::Ending::Detached,
                        );
                    }
                    Ok(crate::cmd::attach::Ending::Signal(number)) => {
                        return Ok(
                            on_signal(ctx, &mut stopper, None, launch, STARTED, number).await,
                        );
                    }
                    // Die Sitzung ist zu Ende; der Ereignisstrom bringt noch
                    // den Exit-Code und die Zusammenfassung.
                    Ok(crate::cmd::attach::Ending::Stream(Ok(code))) => {
                        attach_code = Some(code);
                    }
                    // Schon jemand schreibt: Dieser Lauf hängt sich lesend an
                    // und sieht den Rückstand, statt ihn zu verlieren.
                    Ok(crate::cmd::attach::Ending::Stream(Err(found))) | Err(found)
                        if found.diagnostic.code == codes::TERM_001 && reader_keys.is_some() =>
                    {
                        ctx.render
                            .note(crate::render::diagnostic_block(&found.diagnostic).trim_end());
                        ctx.render.note(
                            "[humanitl] another client writes to this session; this terminal \
                             only watches",
                        );
                        if let Some(keys) = reader_keys.take() {
                            attached =
                                Some(Box::pin(crate::cmd::attach::attach(ctx, true, keys)));
                        }
                        continue;
                    }
                    // Kein Anschluss aus einem anderen Grund: Der Lauf liest
                    // weiter mit wie ohne Terminal.
                    Ok(crate::cmd::attach::Ending::Stream(Err(found))) | Err(found) => {
                        ctx.render
                            .note(crate::render::diagnostic_block(&found.diagnostic).trim_end());
                        failure.get_or_insert(found);
                        through_terminal = false;
                    }
                }
                if !events_open {
                    break;
                }
                continue;
            }
            // Ein Ereignis der Warteschlange: gehalten, entschieden,
            // abgelaufen. Ohne Moderation wartet dieser Zweig für immer.
            flow = next_flow(flows.as_mut()) => {
                if !moderated_flow(moderation.as_mut(), flow).await {
                    // Endet der Strom oder bricht er ab, läuft die Sitzung
                    // weiter: Der Agent arbeitet, und ohne Moderation wird
                    // gehalten, bis die Frist entscheidet.
                    flows = None;
                }
                continue;
            }
            // Eine Taste des Menschen.
            key = next_key(moderation.as_mut()) => {
                if moderated_key(&mut moderation, key).await {
                    // Im Rohmodus ist `ISIG` aus: `Ctrl+C` kommt als Byte und
                    // nicht als Signal, also braucht dieser Weg denselben
                    // zweiten Schritt wie der Signalzweig. Ohne ihn wäre das
                    // zweite `Ctrl+C` verschluckt, und der Befehl bliebe
                    // hängen, wenn der Agent auf das erste nicht hört.
                    if interrupted {
                        return Ok(end_on_signal(ctx, &mut stopper, moderation.as_mut(), 2).await);
                    }
                    interrupted = true;
                    stop(&mut stopper).await;
                }
                continue;
            }
            // Die Uhr im Kopf des Kastens.
            _ = clock.tick(), if moderation.is_some() => {
                if let Some(moderation) = moderation.as_mut() {
                    moderation.tick();
                }
                continue;
            }
            // `Ctrl+C` beendet die Sitzung. Ein Byte an den Agenten gibt es
            // nicht — dafür bräuchte es das PTY aus HUM-042 —, also ist das
            // Ende der Sitzung die ehrliche Antwort auf das Signal.
            // Ohne Bedingung, aus demselben Grund wie der Zweig darunter: Ein
            // zweites `Ctrl+C` kommt, weil das erste nichts bewirkt hat, und
            // dann endet dieser Befehl, statt es zu verschlucken.
            () = next_of(&mut interrupt) => {
                // Die Zeile über das Stoppen soll auf dem gewöhnlichen
                // Bildschirm stehen, nicht auf dem der Animation.
                hold.stop();
                if !launch.on_signal().stops(STARTED, 2) {
                    return Ok(leave_on_signal(2));
                }
                if interrupted {
                    return Ok(end_on_signal(ctx, &mut stopper, moderation.as_mut(), 2).await);
                }
                interrupted = true;
                ask_to_stop(ctx, moderation.as_mut(), &mut stopper).await;
                continue;
            }
            // `SIGTERM` und `SIGHUP` beendet dieser Befehl selbst, damit die
            // Sitzung nicht weiterläuft, wenn er weg ist. Ein Signalhandler,
            // der den Prozess sofort beendete, wäre schneller als diese RPC --
            // dann bliebe eine Sandbox stehen, die niemand mehr beenden kann
            // (`tty.rs` erklärt, warum der Rohmodus deshalb keinen eigenen
            // Handler mitbringt).
            // Ohne Bedingung: Ein `SIGTERM`, das nach einem `Ctrl+C` kommt --
            // weil der Agent auf das erste nicht hört --, muss noch ankommen.
            // Sonst hinge der Befehl bis zu einem `SIGKILL`, und das ließe das
            // Terminal im Rohmodus zurück.
            // `SIGHUP` an `humanitl go` hängt mit `terminal.on_close =
            // "detach"` nur ab (GitHub #13); das Terminal ist dann weg, und
            // auch die Animation schreibt nichts mehr hinein.
            number = next_signal(&mut terminate, &mut hangup) => {
                if number == SIGHUP {
                    hold.abandon();
                } else {
                    hold.stop();
                }
                return Ok(
                    on_signal(ctx, &mut stopper, moderation.as_mut(), launch, STARTED, number)
                        .await,
                );
            }
            }
        };
        let event = match next {
            Ok(Some(event)) => event,
            Ok(None) => {
                events_open = false;
                // Der Strom endet: Lief die Sitzung nie, geht die Animation
                // sofort; was sie zurückhielt, kommt noch.
                hold.stream_ended();
                // A running attach keeps the command alive: the session is
                // there until its terminal says it ended.
                if attached.is_some() || hold.has_held() {
                    continue;
                }
                break;
            }
            Err(status) => {
                failure.get_or_insert_with(|| {
                    Failure::new(status_diagnostic(&status, "Sandbox(Start)"))
                });
                events_open = false;
                hold.stop();
                if attached.is_some() || hold.has_held() {
                    continue;
                }
                break;
            }
        };
        let Some(event) = hold.keep(event) else {
            continue;
        };
        if matches!(
            event.event.as_ref(),
            Some(v1::sandbox_event::Event::Changes(_))
        ) {
            launch.changes_waiting = true;
        }
        // The session runs, and nothing went wrong on the way: this terminal
        // takes it over (HUM-280). A refused start also reports `running`,
        // the state of the other session, after its finding.
        if failure.is_none()
            && is_running(&event)
            && let Some(keys) = pending_keys.take()
        {
            attached = Some(Box::pin(crate::cmd::attach::attach(ctx, false, keys)));
        }
        if through_terminal
            && matches!(
                event.event.as_ref(),
                Some(v1::sandbox_event::Event::Output(_))
            )
        {
            continue;
        }
        if let Some(v1::sandbox_event::Event::Refusals(refusals)) = event.event.as_ref() {
            last_refusals = Some(refusals.clone());
        }
        if let Some(v1::sandbox_event::Event::Exit(ended)) = event.event.as_ref() {
            exit = Some(ended.code);
            // Nach dem Lauf, für jeden und nicht nur mit `-v`: was die
            // Sandbox dem Agenten verweigert hat (HUM-138). Der Daemon schickt
            // den letzten Stand vor dem Exit-Code.
            if let Some(line) = last_refusals.as_ref().and_then(refusal_summary) {
                ctx.render.note(&line);
            }
            continue;
        }
        // Solange ein Kasten steht, hält die Moderation die Ausgabe des
        // Agenten an: Sie überschriebe ihn sonst.
        if let (Some(moderation), Some(v1::sandbox_event::Event::Output(chunk))) =
            (moderation.as_mut(), event.event.as_ref())
        {
            moderation.output(&chunk.data);
            continue;
        }
        if let Some(found) = handle(ctx, event, minor)
            && let Some(shown) = keep_failure(&mut failure, found)
        {
            ctx.render.note(&crate::render::diagnostic_block(&shown));
        }
    }

    if let Some(moderation) = moderation.as_mut() {
        moderation.finish();
    }

    let exit = exit.or(attach_code.map(i32::from));
    // Mit einem Exit-Code bleibt ein Befund, der keine Garantie ist, ohne
    // Fehlschlag; gezeigt wird er trotzdem, genau einmal.
    if exit.is_some()
        && let Some(kept) = failure.as_ref().filter(|kept| kept.exit != EXIT_CHECK)
    {
        ctx.render
            .note(&crate::render::diagnostic_block(&kept.diagnostic));
    }
    outcome(exit, failure)
}

/// Merkt sich den ersten Befund der Sitzung, der den Lauf scheitern lässt;
/// den zeigt am Ende der Aufrufer. Ein späterer, anderer Befund kommt zum
/// Zeigen zurück, ein gleicher nicht (HUM-529): Eine Mounttabelle außerhalb
/// des Plans kam als Ergebnis der Prüfung und noch einmal als Befund des
/// Starts, und `humanitl go` druckte `SANDBOX_049` zweimal.
fn keep_failure(kept: &mut Option<Failure>, found: Failure) -> Option<Diagnostic> {
    match kept {
        None => {
            *kept = Some(found);
            None
        }
        Some(first) if first.diagnostic == found.diagnostic => None,
        Some(_) => Some(found.diagnostic),
    }
}

/// Der Exit-Code der Sitzung, oder der Befund, an dem sie hängen blieb.
fn outcome(exit: Option<i32>, failure: Option<Failure>) -> Result<u8, Failure> {
    if let Some(code) = exit {
        // Eine rote Garantie beendet den Lauf, auch wenn danach noch ein
        // Exit-Code käme: Die Zusage, dass ohne die drei Garantien nichts
        // weiterläuft, wäre sonst nur die halbe.
        if let Some(failure) = failure.filter(|failure| failure.exit == EXIT_CHECK) {
            return Err(failure);
        }
        return Ok(u8::try_from(code).unwrap_or(EXIT_USER));
    }
    Err(failure.unwrap_or_else(|| {
        Failure::new(
            Diagnostic::builder(codes::CLI_001, Severity::Error)
                .why(
                    "the session ended without the agent reporting an exit code; the daemon log \
                     says why",
                )
                .fix(FixAction::CopyCommand(
                    // `daemon logs` liest unter Linux das Journal und unter
                    // macOS die Datei des launchd-Agenten (HUM-324).
                    "humanitl daemon logs -n 50".to_owned(),
                ))
                .build(),
        )
    }))
}

/// Der Ereignisstrom der Flüsse und der Kasten, für `--ask terminal`.
///
/// Ohne Moderation ist beides `None`, und die Schleife in [`drive`] ist
/// dieselbe wie vor diesem Issue. Der Strom wird **vor** dem Rohmodus
/// geöffnet: Scheitert er, soll der Befund auf einem gewöhnlichen Terminal
/// lesbar sein.
async fn open_moderation(
    client: &mut Client,
    moderate: bool,
) -> Result<
    (
        Option<tonic::Streaming<v1::FlowEvent>>,
        Option<crate::cmd::moderate::Moderation>,
    ),
    Failure,
> {
    if !moderate {
        return Ok((None, None));
    }
    let flows = client
        .subscribe(v1::SubscribeRequest {
            since_flow_id: String::new(),
            include_passthrough: false,
            // Das CLI ist nicht die Desktop-App (HUM-539).
            desktop_app: false,
        })
        .await
        .map_err(|status| crate::cmd::moderate::subscribe_failed(&status))?
        .into_inner();
    Ok((
        Some(flows),
        Some(crate::cmd::moderate::Moderation::new(client.clone())),
    ))
}

/// Sagt, dass die Sitzung endet, und schickt `Sandbox(Stop)`.
///
/// Steht ein Kasten, geht die Zeile über ihn: `Renderer::note` schreibt ein
/// nacktes `\n`, und in einem Terminal im Rohmodus ist `OPOST` aus -- die
/// Zeile stiege dann treppenförmig, und der nächste Kasten begänne nicht in
/// Spalte 0.
async fn ask_to_stop(
    ctx: &Context,
    moderation: Option<&mut crate::cmd::moderate::Moderation>,
    stopper: &mut Client,
) {
    match moderation {
        Some(open) => open.say_stopping(),
        None => ctx.render.note("[humanitl] stopping the session"),
    }
    stop(stopper).await;
}

/// Das nächste Signal dieses Stroms, oder nie.
async fn next_of(stream: &mut Option<tokio::signal::unix::Signal>) {
    match stream {
        Some(stream) => {
            stream.recv().await;
        }
        None => std::future::pending().await,
    }
}

/// Endet auf ein Signal hin: Die Sitzung stoppt oder bleibt, wie
/// [`OnSignal::stops`] es sagt, und der Exit-Code ist `128 + n`.
///
/// Der Lauf merkt sich das Signal; nach `SIGHUP` ist sein Terminal weg
/// ([`Launch::hung_up`]).
pub(crate) async fn on_signal(
    ctx: &Context,
    stopper: &mut Client,
    moderation: Option<&mut crate::cmd::moderate::Moderation>,
    launch: &mut Launch,
    owns: bool,
    number: u8,
) -> u8 {
    launch.signalled = Some(number);
    if launch.on_signal().stops(owns, number) {
        end_on_signal(ctx, stopper, moderation, number).await
    } else {
        leave_on_signal(number)
    }
}

/// Endet auf ein Signal hin, ohne die Sitzung anzufassen: Terminal zurück,
/// Exit-Code `128 + n`.
///
/// Das Zurückgeben ist `tcsetattr` und kein Schreiben: Nach `SIGHUP` gibt es
/// das Terminal nicht mehr, und ein Schreiben bekäme `EIO` (GitHub #13).
pub(crate) fn leave_on_signal(number: u8) -> u8 {
    crate::tty::restore_now();
    128_u8.saturating_add(number)
}

/// Beendet die Sitzung auf ein Signal hin und liefert den Exit-Code `128 + n`.
///
/// Nach `SIGHUP` schreibt dieser Weg nichts, weder die Zeile noch den Puffer
/// des Kastens: Das Terminal ist weg, ein Schreiben bekäme `EIO`, und
/// `eprintln!` machte daraus eine Panik, bevor `Sandbox(Stop)` hinausginge
/// (GitHub #13).
pub(crate) async fn end_on_signal(
    ctx: &Context,
    stopper: &mut Client,
    moderation: Option<&mut crate::cmd::moderate::Moderation>,
    number: u8,
) -> u8 {
    if number != SIGHUP {
        if let Some(open) = moderation {
            open.finish();
        }
        ctx.render.note("[humanitl] stopping the session");
    }
    stop(stopper).await;
    crate::tty::restore_now();
    128_u8.saturating_add(number)
}

/// Reicht eine Taste an den Kasten; `true`, wenn die Sitzung enden soll.
///
/// Endet die Eingabe -- eine Pipe zum Beispiel --, geht die Moderation weg,
/// und was im Puffer steht, geht vorher auf den Schirm: Ohne `finish`
/// verschwänden bis zu 256 KiB Ausgabe des Agenten mitsamt dem gezeichneten
/// Kasten.
async fn moderated_key(
    moderation: &mut Option<crate::cmd::moderate::Moderation>,
    key: Option<u8>,
) -> bool {
    match (key, moderation.as_mut()) {
        // `Ctrl+C` ohne stehenden Kasten beendet die Sitzung. Im Rohmodus
        // kommt es als Byte und nicht als Signal.
        (Some(byte), Some(open)) => {
            if open.pressed(byte).await {
                open.say_stopping();
                return true;
            }
            false
        }
        (None, _) => {
            if let Some(open) = moderation.as_mut() {
                open.finish();
            }
            *moderation = None;
            false
        }
        (Some(_), None) => false,
    }
}

/// Reicht ein Ereignis der Warteschlange an den Kasten; `false`, wenn der
/// Strom vorbei ist.
async fn moderated_flow(
    moderation: Option<&mut crate::cmd::moderate::Moderation>,
    flow: Result<Option<v1::FlowEvent>, tonic::Status>,
) -> bool {
    match (flow, moderation) {
        (Ok(Some(event)), Some(moderation)) => {
            moderation.flow_event(&event).await;
            true
        }
        (Ok(Some(_)), None) => true,
        (Ok(None) | Err(_), _) => false,
    }
}

/// Ein Signalstrom, oder `None`, wenn der Handler nicht anzulegen ist.
fn signal_stream(kind: tokio::signal::unix::SignalKind) -> Option<tokio::signal::unix::Signal> {
    tokio::signal::unix::signal(kind).ok()
}

/// Ob auf `stream` schon ein Signal wartet, ohne zu warten.
fn arrived(stream: &mut Option<tokio::signal::unix::Signal>) -> bool {
    stream.as_mut().is_some_and(|stream| {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        stream.poll_recv(&mut context).is_ready()
    })
}

/// Der Exit-Code, wenn vor dem Start ein Signal kam: aus den Merkern eines
/// Bildschirms (`flag`) oder aus den Strömen `[SIGTERM, SIGHUP, SIGINT]`;
/// `None`, wenn keines kam.
fn stop_before_start(
    flag: Option<crate::tui::signals::Signal>,
    [terminate, hangup, interrupt]: [bool; 3],
) -> Option<u8> {
    crate::cmd::go::signalled(flag).or(if terminate {
        Some(143)
    } else if hangup {
        Some(129)
    } else if interrupt {
        Some(130)
    } else {
        None
    })
}

/// Das nächste `SIGTERM` oder `SIGHUP`, als Signalnummer.
///
/// Ohne Handler wartet dieser Zweig für immer: Dann gilt das Standardverhalten
/// des Signals, und der Prozess endet, wie er ohne diesen Befehl endete.
async fn next_signal(
    terminate: &mut Option<tokio::signal::unix::Signal>,
    hangup: &mut Option<tokio::signal::unix::Signal>,
) -> u8 {
    match (terminate.as_mut(), hangup.as_mut()) {
        (Some(term), Some(hup)) => tokio::select! {
            _ = term.recv() => 15,
            _ = hup.recv() => 1,
        },
        (Some(term), None) => {
            term.recv().await;
            15
        }
        (None, Some(hup)) => {
            hup.recv().await;
            1
        }
        (None, None) => std::future::pending().await,
    }
}

/// Das nächste Ereignis der Warteschlange, oder nie.
///
/// `pending` statt eines Zweigs, den es nicht gibt: Ein `select!` braucht in
/// jedem Arm eine Zukunft, und eine Sitzung ohne Moderation hat hier keine.
async fn next_flow(
    flows: Option<&mut tonic::Streaming<v1::FlowEvent>>,
) -> Result<Option<v1::FlowEvent>, tonic::Status> {
    match flows {
        Some(stream) => stream.message().await,
        None => std::future::pending().await,
    }
}

/// Die nächste Taste, oder nie.
async fn next_key(moderation: Option<&mut crate::cmd::moderate::Moderation>) -> Option<u8> {
    match moderation {
        Some(moderation) => moderation.key().await,
        None => std::future::pending().await,
    }
}

/// Beendet die laufende Sitzung.
async fn stop(client: &mut Client) {
    let _ = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Stop(())),
        })
        .await;
}

/// Verarbeitet ein Ereignis der Sitzung; ein Befund kommt als Fehlschlag
/// zurück.
/// `minor` ist die Vertrags-Minor des Daemons; vor 1.28 kennt er die Wahl
/// des Servers nicht (HUM-400).
fn handle(ctx: &Context, event: v1::SandboxEvent, minor: u32) -> Option<Failure> {
    use v1::sandbox_event::Event;

    match event.event? {
        Event::Check(result) => {
            ctx.render.note(&check_line(&result));
            let diagnostic = result.diagnostic.as_ref().and_then(from_proto)?;
            Some(Failure::with_exit(diagnostic, EXIT_CHECK))
        }
        Event::Diagnostic(diagnostic) => {
            let diagnostic = from_proto(&diagnostic)?;
            // Ein Befund, der den Lauf scheitern lässt, zeigt der Aufrufer
            // genau einmal ([`keep_failure`]).
            if diagnostic.severity >= Severity::Error {
                return Some(Failure::new(diagnostic));
            }
            ctx.render
                .note(&crate::render::diagnostic_block(&diagnostic));
            None
        }
        Event::Output(chunk) => {
            write_output(&chunk);
            None
        }
        Event::Log(line) => {
            ctx.render.detail(&format!("[humanitl] {}", line.line));
            None
        }
        Event::Status(status) => {
            ctx.render
                .detail(&format!("[humanitl] sandbox {}", state_name(status.state)));
            // Der Server der Sitzung steht beim Start da, auch ohne `-v`:
            // Ein Projekt kann einen anderen wählen als den aktiven (HUM-400).
            if status.state == v1::SandboxState::Starting as i32 {
                ctx.render.note(&format!(
                    "[humanitl] {}",
                    crate::cmd::plan::server_line(status.llm_server.as_ref(), minor)
                ));
                // Wohin die Inferenz geht, bei jedem Start und in Orange
                // (HUM-516); auch mit `-q`, denn es ist die eine Zeile, die
                // sagt, was den Rechner verlässt.
                if let Some(cloud) = status.cloud_inference.as_ref() {
                    crate::cmd::cloud::print_short_line(cloud);
                }
            }
            None
        }
        Event::ArgvLine(line) => {
            ctx.render.detail(&line);
            None
        }
        // Die Befunde der Zusammenfassung (`SANDBOX_022` bis `SANDBOX_026`)
        // kommen als eigene `Diagnostic`-Ereignisse und stehen also schon da.
        // Hier bleibt die Zeile, die zur ganzen Liste führt: Wer wissen will,
        // welche Dateien es waren, tippt einen Befehl und liest keine
        // Tabelle, die er nicht angefordert hat.
        Event::Summary(summary) => {
            if summary.changes.is_empty() {
                ctx.render
                    .detail("[humanitl] the agent left the project unchanged");
            } else {
                ctx.render.note(&format!(
                    "{} file(s) changed in the project; humanitl sessions summary {}",
                    summary.changes.len(),
                    sanitize_note(&summary.sandbox_id)
                ));
            }
            None
        }
        // Der ganze Stand der verweigerten Versuche, jedes Mal (HUM-138). Er
        // unterbricht das Terminal des Agenten nicht; wer ihn laufend sehen
        // will, nimmt `-v`. Die Zusammenfassung nach dem Lauf schreibt die
        // Schleife beim Exit (`refusal_summary`). Meldet die Sandbox nicht,
        // kam `SANDBOX_019` schon als eigener Befund.
        Event::Refusals(refusals) => {
            if let Some(line) = refusal_summary(&refusals) {
                ctx.render.detail(&format!("[humanitl] {line}"));
            }
            None
        }
        // Die Änderungen einer Wegwerf-Sandbox warten (HUM-350). Die Zeile
        // führt zu den Befehlen; Diff und Auswahl gehören zu ihnen.
        Event::Changes(changes) => {
            ctx.render.note(&format!(
                "{} change(s) wait in the disposable layer of sandbox {}; the project folder \
                 is untouched. Review: humanitl sandbox changes   Apply: humanitl sandbox apply   \
                 Discard: humanitl sandbox discard",
                changes.changes.len(),
                sanitize_note(&changes.sandbox_id)
            ));
            None
        }
        // `Retire` ist die Antwort auf `retire_if_idle` (HUM-284), das
        // Ergebnis einer Übernahme die auf `apply_changes` (HUM-350); auf dem
        // Strom eines Starts kommt beides nie.
        Event::Exit(_)
        | Event::Retire(_)
        | Event::ChangesOutcome(_)
        | Event::ImagePulled(_)
        | Event::ImagePullCancelled(_)
        | Event::CloudInferenceAcknowledged(_)
        | Event::CloudInferenceCheck(_) => None,
    }
}

/// Eine Zeile über die verweigerten Versuche, oder `None`, wenn nichts
/// verweigert wurde (HUM-138).
fn refusal_summary(refusals: &v1::sandbox_event::Refusals) -> Option<String> {
    let total = refusals
        .entries
        .iter()
        .fold(0u64, |sum, entry| sum.saturating_add(entry.count));
    (total > 0).then(|| {
        format!(
            "the sandbox refused {total} socket(s) outside the proxy: {}",
            refusals
                .entries
                .iter()
                .map(|entry| format!(
                    "{}x {}({}, {})",
                    entry.count,
                    sanitize_note(&entry.syscall),
                    sanitize_note(&entry.family),
                    sanitize_note(&entry.socket_type)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// Schreibt ein Stück Ausgabe des Agenten dorthin, wo es hingehört.
///
/// Ungepuffert und sofort: Wer `humanitl run -- sh -c '…'` tippt, will die
/// Zeile sehen, wenn sie entsteht, und nicht, wenn der Puffer voll ist. Die
/// Bytes sind schon gefiltert; dieser Prozess schreibt sie nur weiter.
fn write_output(chunk: &v1::sandbox_event::OutputChunk) {
    if chunk.stream == v1::OutputStream::Stderr as i32 {
        let mut out = std::io::stderr().lock();
        let _ = out.write_all(&chunk.data);
        let _ = out.flush();
    } else {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(&chunk.data);
        let _ = out.flush();
    }
}

/// Die Zeile zu einer der drei Garantien.
fn check_line(result: &v1::CheckResult) -> String {
    let mark = if result.passed { "ok  " } else { "FAIL" };
    format!(
        "[{mark}] {} {}",
        check_name(result.check),
        crate::render::one_line(&result.evidence)
    )
}

/// Der Name einer Garantie, wie ihn ein Mensch liest.
const fn check_name(check: i32) -> &'static str {
    match check {
        1 => "no network interface",
        2 => "one door",
        3 => "seccomp active",
        _ => "isolation",
    }
}

/// Der Name eines Sandbox-Zustands.
const fn state_name(state: i32) -> &'static str {
    match state {
        1 => "starting",
        2 => "running",
        3 => "stopping",
        4 => "stopped",
        5 => "failed",
        _ => "unknown",
    }
}

/// Wo über eine gehaltene Anfrage entschieden wird, als eine Zeile.
///
/// Sie steht vor dem ersten Byte des Agenten und nicht danach: Ohne sie sieht
/// ein Lauf, dessen erste Anfrage gehalten wird, wie ein Hänger aus — der
/// Agent wartet, das Terminal bleibt still, und niemand sagt, worauf. Die
/// einzelne Zeile je gehaltener Anfrage (`[humanitl] request held: …`) braucht
/// den `Subscribe`-Strom und die Säuberung der Werte, die der Agent schickt;
/// sie kommt mit dem Terminal (HUM-042).
///
/// Mit `upstream` (HUM-512) nennt sie den Entscheidungs-Proxy und wer dort
/// auflöst, aus derselben Konfiguration, mit der der Start den Daemon fragt;
/// geht der Daemon noch direkt hinaus, lehnt er den Start ab (`CONFIG_003`).
fn where_decisions_happen(config: &Config) -> String {
    match config.hold.ask_mode {
        AskMode::Ui => "[humanitl] a request without a rule waits for a decision in the app; \
                        without one it is blocked when the hold timeout is over"
            .to_owned(),
        AskMode::None => {
            "[humanitl] nobody is asked: a request without a rule is blocked right away".to_owned()
        }
        // Seit HUM-067 ist dieser Zweig der Normalfall und nicht mehr
        // unerreichbar: Der Kasten steht in genau diesem Terminal. Bis zum
        // 2026-09-07 sagte die Zeile hier „--ask terminal is not available
        // yet" -- eine Auskunft, die der Lauf im selben Atemzug widerlegte,
        // weil er die Frage danach stellte.
        AskMode::Terminal => "[humanitl] a request without a rule is asked right here; \
                              without an answer it is blocked when the hold timeout is over"
            .to_owned(),
        AskMode::Upstream => {
            let proxy = config
                .egress
                .proxy_address()
                .unwrap_or_else(|| "-".to_owned());
            let resolves = match config.egress.proxy_resolve {
                humanitl_config::ProxyResolve::Proxy => {
                    "it resolves names, so a public name that points into your network is its \
                     to refuse"
                }
                humanitl_config::ProxyResolve::Humanitl => {
                    "Humanitl resolves and checks every address first, so the name leaves as a \
                     DNS query before the decision there"
                }
            };
            format!(
                "[humanitl] without a rule: decided by {proxy}; nothing is held here, block rules \
                 still stop a request before it; {resolves}"
            )
        }
    }
}

/// `--ask terminal` mit einem Vollbild-Agenten: `CLI_002`.
///
/// Der Prompt teilt sich den Schirm mit der Ausgabe des Agenten. Ein
/// zeilenorientiertes Kommando schreibt dabei nach unten weiter, und der
/// Kasten steht darüber; ein Vollbild-TUI dagegen zeichnet den ganzen Schirm
/// neu, sooft es will -- der Kasten wäre nach dem ersten Bild weg, und die
/// Tasten, die ein Mensch darauf drückt, gingen an eine Frage, die er nicht
/// mehr sieht (`backlog/CONVENTIONS.md` 4.10).
///
/// Entschieden wird am **wirksamen** Kommando: Wer `-- bash` schreibt, startet
/// kein TUI, auch wenn der Adapter der Sitzung `opencode` heißt.
/// Was `--json` über diesen Lauf sagt, bevor er startet.
fn session_value(
    ctx: &Context,
    args: &RunArgs,
    work_dir: &std::path::Path,
    config: &Config,
) -> serde_json::Value {
    json!({
        "work_dir": work_dir.display().to_string(),
        "profile": ctx.profile_flag().unwrap_or("default"),
        "ask_mode": ask_mode_name(config.hold.ask_mode),
        "command": args
            .cmd
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    })
}

/// Die Konfiguration dieses Laufs mit dem Frage-Modus der Projektwahl, wenn
/// sie einen festlegt (HUM-532), und die Ablehnungen von `terminal` gegen
/// genau diesen Modus: `terminal` aus `config.toml` mit der Wahl `ui` startet,
/// die Wahl `terminal` unter `go` nicht.
fn with_chosen_ask_mode(
    resolved: &Resolved,
    chosen: Option<AskMode>,
    go: bool,
    args: &RunArgs,
    on_terminal: bool,
) -> Result<Box<Config>, Failure> {
    let mut config = Box::new(resolved.config.clone());
    if let Some(chosen) = chosen {
        config.hold.ask_mode = chosen;
    }
    terminal_refusals(&config, go, args, on_terminal)?;
    Ok(config)
}

/// `CLI_002` für einen Frage-Modus `terminal`, den dieser Lauf nicht tragen
/// kann: unter `go` nie, sonst nicht ohne Terminal.
fn terminal_refusals(
    config: &Config,
    go: bool,
    args: &RunArgs,
    on_terminal: bool,
) -> Result<(), Failure> {
    if go {
        crate::cmd::go::refuse_ask_terminal(config)?;
    }
    refuse_terminal_ask(config, args, on_terminal)
}

fn refuse_terminal_ask(config: &Config, args: &RunArgs, on_terminal: bool) -> Result<(), Failure> {
    if config.hold.ask_mode != AskMode::Terminal {
        return Ok(());
    }
    // Ohne Terminal gibt es keinen Prompt: Aus einer Pipe kämen Bytes, die
    // niemand als Antwort gemeint hat, und ein `b` in einem Skript blockte
    // einen Fluss.
    if !on_terminal {
        return Err(Failure::with_exit(
            Diagnostic::builder(codes::CLI_002, Severity::Error)
                .why(
                    "--ask terminal needs a terminal on standard input and on standard error, \
                     and this run has none. Use --ask ui and decide in the app, or --ask none \
                     and let every request without a rule be blocked.",
                )
                .fix(FixAction::CopyCommand("humanitl run --ask ui".to_owned()))
                .build(),
            EXIT_USER,
        ));
    }
    // Entschieden wird am wirksamen Kommando: `-- bash` startet kein TUI, auch
    // wenn der Adapter der Sitzung `opencode` heißt -- und `-- opencode`
    // startet eines, auch wenn es genannt wurde (HUM-135).
    let fullscreen = AdapterRegistry::builtin()
        .get(&config.agent.adapter)
        .is_some_and(AgentAdapter::is_fullscreen_tui)
        && humanitl_sandbox::agent::command_is_the_agent(config, &args.cmd);
    if !fullscreen {
        return Ok(());
    }
    Err(Failure::with_exit(
        Diagnostic::builder(codes::CLI_002, Severity::Error)
            .why(format!(
                "the agent {} draws the whole screen, and a prompt in the same terminal would be \
                 gone with its next frame. Use --ask ui and decide in the app, --ask none and let \
                 every request without a rule be blocked, or run a line-oriented command with \
                 `-- <command>`.",
                config.agent.adapter
            ))
            // Ein Befehl zum Abtippen, kein Schlüssel: `humanitl config set`
            // gibt es nicht, und ein Vorschlag, der nicht läuft, ist keiner.
            .fix(FixAction::CopyCommand("humanitl run --ask ui".to_owned()))
            .build(),
        EXIT_USER,
    ))
}

/// Die Konfigurationswerte der Kommandozeile, die für die Sitzung gelten
/// sollen.
///
/// Genau die Pfade, die der Daemon annimmt
/// ([`humanitl_ipc::session::SESSION_OVERRIDE_KEYS`]). Jeder andere wird dort
/// mit `CONFIG_003` abgelehnt — die Regel steht am Socket und nicht hier —,
/// aber ihn gar nicht erst zu schicken erspart dem Nutzer einen Befund über
/// etwas, das für diese Sitzung ohnehin schon gilt: `--work`, `--work-mode`
/// und `--ask` reisen in ihren eigenen Feldern.
///
/// Dazu die Wahl des Sandbox-Backends aus der Umgebung dieses Aufrufs
/// ([`environment_backend_choice`]), ein Flag vor ihr, und die strengste
/// Untergrenze, die nur der Client sieht ([`client_floor`]).
///
/// # Errors
///
/// `CLI_004` aus [`floor_reaches_the_daemon`], wenn der Daemon `minor` zu alt
/// für die Untergrenze ist.
fn session_overrides(
    ctx: &Context,
    resolved: &Resolved,
    minor: u32,
) -> Result<Vec<v1::sandbox_request::CliOverride>, Failure> {
    let overrides = floor_reaches_the_daemon(client_overrides(ctx, resolved), minor)?;
    agent_reaches_the_daemon(&overrides, minor)?;
    Ok(overrides)
}

/// Die Werte der Sitzung, die dieser Aufruf einem Start schicken würde, ohne
/// die Prüfung der Minor: [`start_overrides`] mit den Paaren und der Datei
/// aus `--config` dieses Aufrufs.
///
/// Gemeinsam mit der Vorschau (`humanitl sandbox plan`, HUM-559), damit Plan
/// und Start dieselben Werte und vor allem dieselbe Untergrenze sehen, und
/// mit `humanitl projects proposal`, das sagt, welche davon der Vorschlag
/// nicht kennt.
pub(crate) fn client_overrides(
    ctx: &Context,
    resolved: &Resolved,
) -> Vec<v1::sandbox_request::CliOverride> {
    start_overrides(ctx.cli_pairs(), resolved, ctx.config_file.is_some())
}

/// [`session_overrides`] ohne den Kontext und ohne die Prüfung der Minor:
/// [`merge_overrides`] und dazu die Untergrenze dieses Aufrufs
/// ([`client_floor`]).
fn start_overrides(
    cli_pairs: Vec<(String, String)>,
    resolved: &Resolved,
    named_config: bool,
) -> Vec<v1::sandbox_request::CliOverride> {
    let mut overrides = merge_overrides(cli_pairs, resolved);
    if let Some(level) = client_floor(resolved, named_config) {
        overrides.push(v1::sandbox_request::CliOverride {
            path: SANDBOX_MIN_ISOLATION_KEY.to_owned(),
            value: level.to_owned(),
        });
    }
    overrides
}

/// Die Untergrenze, die dieser Aufruf dem Daemon schickt (HUM-489): der
/// aufgelöste Wert von `sandbox.min_isolation`, wenn ihn eine Quelle gesetzt
/// hat, die nur der Client sieht, also `--sandbox-min-isolation`,
/// `HUMANITL_SANDBOX__MIN_ISOLATION` oder eine Datei, die `--config` nennt.
///
/// Geschickt wird der aufgelöste Wert und nie der rohe Wert des Flags. Beim
/// Mischen gilt der strengste Wert aller Ebenen, und die Herkunft nennt die
/// Ebene dieses Werts. Ein schwächeres Flag neben einer strengeren Variable
/// schickt deshalb die Variable, statt sie zu verdrängen. Kommt der strengste
/// Wert aus `config.toml` oder einem Profil, liest der Daemon ihn selbst, und
/// nichts reist mit.
fn client_floor(resolved: &Resolved, named_config: bool) -> Option<&'static str> {
    wish::per_run(resolved, SANDBOX_MIN_ISOLATION_KEY, named_config)
        .then(|| resolved.config.sandbox.min_isolation.as_str())
}

/// Die Paare der Kommandozeile und die Wahl des Backends aus der Umgebung,
/// ein Flag vor der Umgebung. Die Untergrenze bleibt hier draußen; sie
/// setzt [`start_overrides`] aus der Auflösung ([`client_floor`]).
fn merge_overrides(
    cli_pairs: Vec<(String, String)>,
    resolved: &Resolved,
) -> Vec<v1::sandbox_request::CliOverride> {
    let mut pairs: Vec<(String, String)> = cli_pairs
        .into_iter()
        .filter(|(path, _)| {
            SESSION_OVERRIDE_KEYS.contains(&path.as_str()) && path != SANDBOX_MIN_ISOLATION_KEY
        })
        .collect();
    for (path, value) in environment_backend_choice(resolved) {
        if !pairs.iter().any(|(given, _)| *given == path) {
            pairs.push((path, value));
        }
    }
    pairs
        .into_iter()
        .map(|(path, value)| v1::sandbox_request::CliOverride { path, value })
        .collect()
}

/// Das Backend, das dieser Aufruf als `sandbox.backend` in
/// `Start.cli_overrides` schicken würde: `--backend` vor
/// `HUMANITL_SANDBOX__BACKEND`, `None` ohne beides (HUM-500). So wählt
/// `humanitl sandbox image pull` dasselbe Backend wie `humanitl run`.
pub(crate) fn session_backend(
    cli_pairs: Vec<(String, String)>,
    resolved: &Resolved,
) -> Option<String> {
    merge_overrides(cli_pairs, resolved)
        .into_iter()
        .find(|pair| pair.path == "sandbox.backend")
        .map(|pair| pair.value)
}

/// `sandbox.backend` und `sandbox.podman.runtime`, wo sie in der Auflösung
/// dieses Aufrufs aus der Umgebung kommen (`HUMANITL_SANDBOX__BACKEND`,
/// `HUMANITL_SANDBOX__PODMAN__RUNTIME`, HUM-432). Die Untergrenze aus der
/// Umgebung reist über [`client_floor`].
///
/// Der Daemon läuft mit seiner eigenen Umgebung, meist als Nutzer-Unit ohne
/// diese Variablen. Bliebe die Wahl hier, startete er still sein Backend
/// statt des gewünschten, und ein `SANDBOX_034` oder `SANDBOX_048` fiele
/// unter den Tisch. Über `cli_overrides` kommt sie als Wert der
/// Kommandozeile an, und der Daemon prüft sie wie ein Flag:
/// Untergrenze, Zulassung und `runtime_fits_backend`.
fn environment_backend_choice(resolved: &Resolved) -> Vec<(String, String)> {
    let sandbox = &resolved.config.sandbox;
    [
        (
            "sandbox.backend",
            humanitl_sandbox::backend_name(sandbox.backend),
        ),
        ("sandbox.podman.runtime", sandbox.podman.runtime.as_str()),
    ]
    .into_iter()
    .filter(|(path, _)| matches!(resolved.origin(path), Some(humanitl_config::Origin::Env(_))))
    .map(|(path, value)| (path.to_owned(), value.to_owned()))
    .collect()
}

/// Ab dieser Minor nimmt der Daemon `sandbox.min_isolation` in
/// `Start.cli_overrides` an (HUM-489).
pub(crate) const SESSION_FLOOR_MINOR: u32 = 49;

/// Die Wünsche des Starts, wenn der Daemon jeden davon annimmt.
///
/// Ein Daemon vor Vertrag 1.49 lehnt die Untergrenze in `cli_overrides` mit
/// `CONFIG_003` ab, als dürfe eine Sitzung sie nie setzen. Das ist falsch
/// und führt in die Irre; hier sagt `CLI_004` stattdessen, dass der Daemon
/// zu alt ist und wie er neu startet.
///
/// # Errors
///
/// `CLI_004`, wenn die Wünsche `sandbox.min_isolation` tragen und der Daemon
/// vor Vertrag 1.49 spricht.
fn floor_reaches_the_daemon(
    overrides: Vec<v1::sandbox_request::CliOverride>,
    minor: u32,
) -> Result<Vec<v1::sandbox_request::CliOverride>, Failure> {
    let Some(floor) = overrides
        .iter()
        .find(|pair| pair.path == SANDBOX_MIN_ISOLATION_KEY)
    else {
        return Ok(overrides);
    };
    if minor >= SESSION_FLOOR_MINOR {
        return Ok(overrides);
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!(
                "the daemon speaks contract 1.{minor}, and {SANDBOX_MIN_ISOLATION_KEY} = {:?} for \
                 this run needs 1.{SESSION_FLOOR_MINOR}; restart the daemon so that it applies \
                 the floor, or set the floor in config.toml",
                floor.value
            ))
            .fix(FixAction::CopyCommand(
                "systemctl --user restart humanitld.service".to_owned(),
            ))
            .build(),
    ))
}

/// Ab dieser Minor nimmt der Daemon `agent.adapter` in
/// `Start.cli_overrides` an (#8).
pub(crate) const SESSION_AGENT_MINOR: u32 = 61;

/// Ob der Daemon den Agenten unter den Werten der Sitzung annimmt, vor dem
/// Senden von Start, Vorschau, Ziehen oder Vorschlag.
///
/// Ein Daemon vor Vertrag 1.61 lehnt `agent.adapter` in `cli_overrides` mit
/// `CONFIG_003` ab, als dürfe eine Sitzung ihren Agenten nie wählen; hier
/// sagt `CLI_004` stattdessen, dass der Daemon zu alt ist.
///
/// # Errors
///
/// `CLI_004`, wenn die Werte `agent.adapter` tragen und der Daemon vor
/// Vertrag 1.61 spricht.
pub(crate) fn agent_reaches_the_daemon(
    overrides: &[v1::sandbox_request::CliOverride],
    minor: u32,
) -> Result<(), Failure> {
    let Some(agent) = overrides.iter().find(|pair| pair.path == AGENT_ADAPTER_KEY) else {
        return Ok(());
    };
    if minor >= SESSION_AGENT_MINOR {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!(
                "the daemon speaks contract 1.{minor}, and the agent {:?} for this run needs \
                 1.{SESSION_AGENT_MINOR}; restart the daemon so that it takes this agent",
                agent.value
            ))
            .fix(FixAction::CopyCommand(
                humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
            ))
            .build(),
    ))
}

/// Der Server aus `--llm NAME` für `Start.llm_server` (HUM-400).
///
/// # Errors
///
/// `CLI_004`, wenn ein Server genannt ist und der Daemon vor Vertrag 1.28
/// spricht: Er überginge das Feld still.
fn run_server(ctx: &Context, minor: u32) -> Result<String, Failure> {
    let Some(server) = crate::cmd::plan::llm_server_pair(ctx) else {
        return Ok(String::new());
    };
    if minor < crate::cmd::plan::SERVER_CHOICE_MINOR {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{minor}, and --llm {server} needs 1.{}; it \
                     would start with another server",
                    crate::cmd::plan::SERVER_CHOICE_MINOR
                ))
                .fix(FixAction::CopyCommand(
                    "systemctl --user restart humanitld.service".to_owned(),
                ))
                .build(),
        ));
    }
    Ok(server)
}

/// `ro` oder `rw`, wie das Protokoll den Modus schreibt.
const fn work_mode_name(mode: humanitl_config::WorkMode) -> &'static str {
    match mode {
        humanitl_config::WorkMode::Ro => "ro",
        humanitl_config::WorkMode::Rw => "rw",
    }
}

/// Die aufgelöste Sitzung als Text, eine Zeile je Aussage.
fn session_lines(resolved: &humanitl_config::Resolved, args: &RunArgs) -> String {
    let config = &resolved.config;
    let mut lines = vec![format!("profiles: {}", chain(resolved))];
    lines.push(format!("ask mode: {:?}", config.hold.ask_mode));
    lines.push(format!("hold timeout: {} s", config.hold.timeout_secs));
    lines.push(format!("sandbox profile: {}", config.sandbox.profile));
    lines.push(format!("work mode: {:?}", config.sandbox.work_mode));
    lines.push(format!(
        "work dir: {}",
        config.sandbox.work_dir.as_ref().map_or_else(
            || "the current directory".to_owned(),
            |dir| dir.display().to_string()
        )
    ));
    lines.push(format!("agent: {}", config.agent.adapter));
    lines.push(format!(
        "llm server: {}",
        config.llm.choice.as_ref().map_or_else(
            || "-".to_owned(),
            |choice| format!("{} (from {})", choice.name, choice.source)
        )
    ));
    lines.push(format!(
        "llm endpoint: {}",
        config
            .llm
            .endpoint
            .as_ref()
            .map_or_else(|| "-".to_owned(), ToString::to_string)
    ));
    lines.push(format!("rule files: {}", rule_files(resolved).join(", ")));
    lines.push(format!("profile rules: {}", inline_rules(resolved)));
    if !args.cmd.is_empty() {
        lines.push(format!("command: {}", quoted(&args.cmd)));
    }
    lines.join("\n")
}

/// Der Befehl als eine Zeile.
fn quoted(command: &[OsString]) -> String {
    command
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Die Profil-Kette als eine Zeile.
fn chain(resolved: &humanitl_config::Resolved) -> String {
    let chain: Vec<String> = resolved
        .profile_chain()
        .iter()
        .map(humanitl_config::Origin::to_string)
        .collect();
    if chain.is_empty() {
        "none".to_owned()
    } else {
        chain.join(" then ")
    }
}

/// Die Regeldateien aller beteiligten Profile, schon aufgelöst.
fn rule_files(resolved: &humanitl_config::Resolved) -> Vec<String> {
    let files: Vec<String> = resolved
        .profiles
        .iter()
        .flat_map(humanitl_config::Profile::rule_files)
        .map(|path| path.display().to_string())
        .collect();
    if files.is_empty() {
        vec!["-".to_owned()]
    } else {
        files
    }
}

/// Wie viele Regeln die Profile selbst mitbringen.
fn inline_rules(resolved: &humanitl_config::Resolved) -> usize {
    resolved
        .profiles
        .iter()
        .map(|profile| profile.rules.inline.len())
        .sum()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{AskMode, Env, ProfileSelection, resolve};

    /// Ein Signal vor dem Start beendet mit `128 + n`, ob es in den Merkern
    /// eines Bildschirms steht oder in einem Strom von `tokio` wartet; ohne
    /// Signal geht der Start (#9, Review).
    ///
    /// Mutation proof: `stop_before_start` returning `None` for a waiting
    /// stream fails the second to fourth assertions.
    #[test]
    fn a_signal_before_the_start_stops_it() {
        use crate::tui::signals::Signal;

        assert_eq!(
            stop_before_start(Some(Signal::Interrupt), [false; 3]),
            Some(130)
        );
        assert_eq!(stop_before_start(None, [true, false, false]), Some(143));
        assert_eq!(stop_before_start(None, [false, true, false]), Some(129));
        assert_eq!(stop_before_start(None, [false, false, true]), Some(130));
        assert_eq!(stop_before_start(Some(Signal::Continue), [false; 3]), None);
        assert_eq!(stop_before_start(None, [false; 3]), None);
    }
    use humanitl_ipc::session::SESSION_OVERRIDE_KEYS;
    use humanitl_ipc::v1;

    use super::{
        Config, OnClose, OnSignal, SIGHUP, attaches, chain, check_line, inline_rules,
        refusal_summary, refuse_terminal_ask, session_lines, start_mode, state_name,
        stop_before_start,
    };
    use crate::cli::RunArgs;

    /// HUM-529: Derselbe Befund aus der Prüfung und aus dem Start wird nur
    /// einmal gezeigt; der erste bleibt der Fehlschlag, ein anderer kommt zum
    /// Zeigen zurück. Mutationsbeweis: `keep_failure`, das jeden späteren
    /// Befund zurückgibt, zeigt `SANDBOX_049` zweimal, wie unter 0.4.0.
    #[test]
    fn the_same_finding_twice_is_shown_once() {
        use humanitl_core::diagnostics::codes::{SANDBOX_015, SANDBOX_049};
        use humanitl_core::{Diagnostic, Severity};

        use super::{EXIT_CHECK, Failure, keep_failure};

        let finding = |code, why: &str| {
            Diagnostic::builder(code, Severity::Blocking)
                .why(why.to_owned())
                .build()
        };
        let mounts = finding(SANDBOX_049, "mounts_match FAIL: extra=/usr/lib/modules");
        let mut kept = None;
        assert_eq!(
            keep_failure(&mut kept, Failure::with_exit(mounts.clone(), EXIT_CHECK)),
            None
        );
        assert_eq!(keep_failure(&mut kept, Failure::new(mounts.clone())), None);
        let other = finding(SANDBOX_015, "something else");
        assert_eq!(
            keep_failure(&mut kept, Failure::new(other.clone())),
            Some(other)
        );
        let kept = kept.expect("the first finding stays");
        assert_eq!(kept.diagnostic, mounts);
        assert_eq!(kept.exit, EXIT_CHECK);
    }
    use crate::cmd::{EXIT_CHECK, EXIT_USER};

    /// Only a command of the caller at a terminal attaches, and neither the
    /// box of `--ask terminal` nor `--json` share the terminal with it
    /// (HUM-280).
    #[test]
    fn only_a_callers_command_at_a_terminal_attaches() {
        let bash = RunArgs {
            cmd: vec![std::ffi::OsString::from("bash")],
            disposable: false,
            direct: false,
            yes: false,
        };
        let agent = RunArgs {
            cmd: Vec::new(),
            disposable: false,
            direct: false,
            yes: false,
        };
        assert!(attaches(&bash, false, false, false, true));
        assert!(!attaches(&bash, false, false, false, false), "no terminal");
        assert!(!attaches(&bash, false, true, false, true), "--ask terminal");
        assert!(!attaches(&bash, false, false, true, true), "--json");
        assert!(
            !attaches(&agent, false, false, false, true),
            "the agent itself"
        );
        // `humanitl go` lands in the agent's terminal, also without a
        // command of its own, but never without a terminal, with --json or
        // next to the box of --ask terminal (HUM-496).
        assert!(attaches(&agent, true, false, false, true), "go: the agent");
        assert!(
            !attaches(&agent, true, false, false, false),
            "go: no terminal"
        );
        assert!(!attaches(&agent, true, false, true, true), "go: --json");
        assert!(
            !attaches(&agent, true, true, false, true),
            "go: --ask terminal"
        );
    }

    /// A pure watcher never stops the session on a signal; a run that owns
    /// it does (HUM-280), also when it fell back to watching (GitHub #13,
    /// review). `SIGHUP` leaves the session of `go` running under the
    /// default `terminal.on_close = "detach"`, never that of `run`.
    ///
    /// Mutation proof: dropping `self.go &&` from `OnSignal::stops` fails
    /// the `run` row; inverting the `on_close` match fails both `go` rows.
    #[test]
    fn a_watcher_leaves_the_session_on_a_signal() {
        let run = OnSignal::default();
        for number in [SIGHUP, 2, 15] {
            assert!(run.stops(true, number), "run, signal {number}");
            assert!(!run.stops(false, number), "watcher, signal {number}");
        }
        let run_with_detach = OnSignal {
            go: false,
            on_close: OnClose::Detach,
        };
        assert!(run_with_detach.stops(true, SIGHUP), "run ignores the key");
        let detach = OnSignal {
            go: true,
            on_close: OnClose::Detach,
        };
        assert!(!detach.stops(true, SIGHUP), "go detaches on SIGHUP");
        assert!(detach.stops(true, 15), "SIGTERM stops go");
        assert!(detach.stops(true, 2), "SIGINT stops go");
        let stop = OnSignal {
            go: true,
            on_close: OnClose::Stop,
        };
        assert!(stop.stops(true, SIGHUP), "go with stop stops on SIGHUP");
        assert!(!stop.stops(false, SIGHUP), "a watcher never stops");
    }

    /// The summary after the run names every pair and the total, and says
    /// nothing when nothing was refused (HUM-138).
    #[test]
    fn the_refusal_summary_counts_and_stays_quiet_on_zero() {
        use humanitl_ipc::v1::sandbox_event::{Refusal, Refusals};

        assert_eq!(refusal_summary(&Refusals::default()), None);
        let refusals = Refusals {
            entries: vec![
                Refusal {
                    syscall: "socket".to_owned(),
                    family: "AF_UNIX".to_owned(),
                    socket_type: "SOCK_STREAM".to_owned(),
                    count: 3,
                    ..Refusal::default()
                },
                Refusal {
                    syscall: "socket".to_owned(),
                    family: "AF_INET".to_owned(),
                    socket_type: "SOCK_DGRAM".to_owned(),
                    count: 2,
                    ..Refusal::default()
                },
            ],
            ..Refusals::default()
        };
        assert_eq!(
            refusal_summary(&refusals).as_deref(),
            Some(
                "the sandbox refused 5 socket(s) outside the proxy: \
                 3x socket(AF_UNIX, SOCK_STREAM), 2x socket(AF_INET, SOCK_DGRAM)"
            )
        );
    }

    fn resolved(name: &str) -> humanitl_config::Resolved {
        let empty = tempfile::tempdir().expect("tempdir");
        let env = Env::from_pairs([
            ("HOME", empty.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                empty.path().join("cfg").display().to_string(),
            ),
        ]);
        resolve(&ProfileSelection::named(name), None, &env, &[]).expect("the profile resolves")
    }

    #[test]
    fn the_lines_name_the_chain_and_the_session() {
        let resolved = resolved("llm-only");
        let text = session_lines(
            &resolved,
            &RunArgs {
                cmd: Vec::new(),
                disposable: false,
                direct: false,
                yes: false,
            },
        );

        assert!(text.contains("profile builtin default"), "{text}");
        assert!(text.contains("profile builtin llm-only"), "{text}");
        assert!(text.contains("ask mode: None"), "{text}");
        assert_eq!(inline_rules(&resolved), 1);
        assert_eq!(
            chain(&resolved),
            "profile builtin default then profile builtin llm-only"
        );
    }

    /// Eine Konfiguration mit diesem Frage-Modus und diesem Adapter.
    fn with(ask_mode: AskMode, adapter: &str) -> Config {
        let mut config = Config::default();
        config.hold.ask_mode = ask_mode;
        config.agent.adapter = adapter.to_owned();
        config
    }

    /// Kein Kommando hinter `--`.
    /// HUM-532, Codex: `terminal` aus `config.toml` und die Projektwahl
    /// `ui`: `go` startet, denn die Wahl gewinnt; ohne Wahl bleibt es
    /// `CLI_002`, und die Wahl `terminal` lehnt `go` ebenso ab.
    ///
    /// Mutationsbeweis: Prüfte `with_chosen_ask_mode` den lokalen Modus statt
    /// des gewählten, käme `CLI_002`, und der Test wird rot.
    #[test]
    fn the_chosen_ask_mode_decides_the_terminal_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[hold]\nask_mode = \"terminal\"\n").unwrap();
        let resolved = humanitl_config::load(&humanitl_config::Sources {
            global_toml: Some(file),
            ..humanitl_config::Sources::empty()
        })
        .unwrap();
        let go = true;
        let chosen = super::with_chosen_ask_mode(&resolved, Some(AskMode::Ui), go, &no_cmd(), true)
            .expect("the project choice wins over config.toml");
        assert_eq!(chosen.hold.ask_mode, AskMode::Ui);
        let refused = super::with_chosen_ask_mode(&resolved, None, go, &no_cmd(), true)
            .expect_err("terminal from config.toml without a choice");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_002");
        let ui = humanitl_config::load(&humanitl_config::Sources::empty()).unwrap();
        let refused =
            super::with_chosen_ask_mode(&ui, Some(AskMode::Terminal), go, &no_cmd(), true)
                .expect_err("a chosen terminal under go");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_002");
    }

    fn no_cmd() -> RunArgs {
        RunArgs {
            cmd: Vec::new(),
            disposable: false,
            direct: false,
            yes: false,
        }
    }

    #[test]
    fn ask_terminal_is_cli_002_and_names_both_ways_out() {
        let config = with(AskMode::Terminal, "opencode");
        let failure = refuse_terminal_ask(&config, &no_cmd(), true)
            .expect_err("opencode draws the whole screen");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_002");
        assert_eq!(failure.exit, EXIT_USER);
        assert!(
            failure.diagnostic.why.contains("--ask ui"),
            "{}",
            failure.diagnostic.why
        );
        assert!(
            failure.diagnostic.why.contains("--ask none"),
            "{}",
            failure.diagnostic.why
        );
    }

    #[test]
    fn every_ask_mode_says_where_a_decision_happens() {
        assert!(
            super::where_decisions_happen(&with(AskMode::Ui, "opencode")).contains("in the app"),
            "ui points at the app"
        );
        assert!(
            super::where_decisions_happen(&with(AskMode::None, "opencode"))
                .contains("blocked right away"),
            "none says that nobody is asked"
        );
        // HUM-512: `upstream` nennt den Proxy und wer auflöst.
        let mut upstream = with(AskMode::Upstream, "opencode");
        upstream.egress.proxy = "http://127.0.0.1:8080".parse().ok();
        let line = super::where_decisions_happen(&upstream);
        assert!(
            line.contains("decided by 127.0.0.1:8080") && line.contains("its to refuse"),
            "{line}"
        );
        upstream.egress.proxy_resolve = humanitl_config::ProxyResolve::Humanitl;
        let line = super::where_decisions_happen(&upstream);
        assert!(line.contains("DNS query before the decision"), "{line}");
    }

    #[test]
    fn the_other_two_ask_modes_start() {
        for mode in [AskMode::Ui, AskMode::None] {
            let config = with(mode, "opencode");
            assert!(
                refuse_terminal_ask(&config, &no_cmd(), true).is_ok(),
                "{mode:?}"
            );
        }
    }

    /// Ein eigenes Kommando ist nicht der Vollbild-Agent.
    ///
    /// Wer `humanitl run --ask terminal -- bash` schreibt, startet eine Shell,
    /// die zeilenweise schreibt; die Verweigerung hängt am wirksamen Kommando
    /// und nicht am Namen des Adapters, der für diese Sitzung ohnehin nichts
    /// startet.
    #[test]
    fn a_command_of_its_own_is_not_the_fullscreen_agent() {
        let config = with(AskMode::Terminal, "opencode");
        // Auch die Namen, die dem Agenten ähneln, und der Umweg über eine
        // Shell: Wer `sh -c "opencode"` schreibt, startet eine Shell.
        for own in ["bash", "opencoded", "myopencode"] {
            let args = RunArgs {
                cmd: vec![std::ffi::OsString::from(own)],
                disposable: false,
                direct: false,
                yes: false,
            };
            assert!(
                refuse_terminal_ask(&config, &args, true).is_ok(),
                "{own} is a command of its own"
            );
        }
        let shell = RunArgs {
            cmd: vec![
                std::ffi::OsString::from("sh"),
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from("opencode"),
            ],
            disposable: false,
            direct: false,
            yes: false,
        };
        assert!(refuse_terminal_ask(&config, &shell, true).is_ok());
    }

    /// Der genannte Agent ist der Agent: Wer ihn beim Namen ruft, bekommt
    /// dieselbe Absage wie der, der ihn gar nicht nennt (HUM-135).
    #[test]
    fn the_named_agent_is_still_the_fullscreen_agent() {
        let config = with(AskMode::Terminal, "opencode");
        for named in ["opencode", "/usr/local/bin/opencode", "./opencode"] {
            for cmd in [
                vec![std::ffi::OsString::from(named)],
                // Mit Argumenten, denn so ruft ein Mensch ihn wirklich auf.
                vec![
                    std::ffi::OsString::from(named),
                    std::ffi::OsString::from("run"),
                    std::ffi::OsString::from("say hello"),
                ],
            ] {
                let args = RunArgs {
                    cmd,
                    disposable: false,
                    direct: false,
                    yes: false,
                };
                let failure = refuse_terminal_ask(&config, &args, true)
                    .expect_err(&format!("{named} draws the whole screen"));
                assert_eq!(failure.diagnostic.code.as_str(), "CLI_002", "{named}");
            }
        }
    }

    /// Auch der Agent, den die Konfiguration selbst benennt, ist der Agent
    /// (HUM-135): Wer `agent.command` setzt und dasselbe Programm hinter `--`
    /// schreibt, bekommt dieselbe Absage.
    #[test]
    fn the_configured_agent_counts_under_its_own_name() {
        let mut config = with(AskMode::Terminal, "opencode");
        config.agent.command = Some(vec!["mycode".to_owned()]);
        let args = RunArgs {
            cmd: vec![std::ffi::OsString::from("/opt/bin/mycode")],
            disposable: false,
            direct: false,
            yes: false,
        };
        let failure =
            refuse_terminal_ask(&config, &args, true).expect_err("mycode is the agent here");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_002");

        // Und ein anderes Programm bleibt ein anderes.
        let other = RunArgs {
            cmd: vec![std::ffi::OsString::from("bash")],
            disposable: false,
            direct: false,
            yes: false,
        };
        assert!(refuse_terminal_ask(&config, &other, true).is_ok());
    }

    /// Ein Adapter, den es nicht gibt, zeichnet keinen Vollbildschirm.
    ///
    /// Der Start scheitert daran später mit dem Befund des Dienstes; hier
    /// scheitert er nicht vorher an einer Annahme über einen Namen, den
    /// niemand kennt.
    #[test]
    fn an_unknown_adapter_is_not_refused_here() {
        let config = with(AskMode::Terminal, "there-is-no-such-adapter");
        assert!(refuse_terminal_ask(&config, &no_cmd(), true).is_ok());
    }

    /// Ohne Terminal gibt es keinen Prompt, und dann auch keinen Start.
    ///
    /// Aus einer Pipe kämen Bytes, die niemand als Antwort gemeint hat: Ein
    /// `b` in einem Skript blockte einen Fluss, ohne dass ein Mensch die
    /// Frage gesehen hätte.
    #[test]
    fn without_a_terminal_ask_terminal_is_cli_002() {
        let config = with(AskMode::Terminal, "there-is-no-such-adapter");
        let failure =
            refuse_terminal_ask(&config, &no_cmd(), false).expect_err("no terminal, no prompt");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_002");
        assert!(
            failure
                .diagnostic
                .why
                .contains("terminal on standard input"),
            "{}",
            failure.diagnostic.why
        );
    }

    #[test]
    fn a_failed_check_is_exit_three() {
        // Die Zuordnung steht in `cmd::exit_code`; hier wird nur festgehalten,
        // dass eine rote Garantie sie bekommt und nicht Exit 1.
        assert_eq!(EXIT_CHECK, 3);
    }

    #[test]
    fn a_check_line_says_pass_or_fail_and_the_name() {
        let ok = check_line(&humanitl_ipc::v1::CheckResult {
            check: 1,
            passed: true,
            evidence: "lo only".to_owned(),
            diagnostic: None,
        });
        assert!(ok.starts_with("[ok  ] no network interface"), "{ok}");

        let bad = check_line(&humanitl_ipc::v1::CheckResult {
            check: 3,
            passed: false,
            evidence: "Seccomp:\t0".to_owned(),
            diagnostic: None,
        });
        assert!(bad.starts_with("[FAIL] seccomp active"), "{bad}");
    }

    #[test]
    fn the_states_have_names() {
        assert_eq!(state_name(2), "running");
        assert_eq!(state_name(5), "failed");
        assert_eq!(state_name(99), "unknown");
    }

    /// Nur die Pfade, die der Daemon annimmt, verlassen die Kommandozeile.
    ///
    /// Der Filter hier ist Bequemlichkeit; die Regel steht am Socket
    /// (`humanitl_ipc::session::check_override_key`). Beide Seiten müssen aber
    /// dieselbe Liste meinen, sonst schickt die Kommandozeile etwas, das der
    /// Daemon ablehnt, oder verschweigt etwas, das er annähme.
    #[test]
    fn the_allowed_paths_are_the_ones_the_daemon_names() {
        assert!(SESSION_OVERRIDE_KEYS.contains(&"llm.endpoint"));
        assert!(SESSION_OVERRIDE_KEYS.contains(&"hold.timeout_secs"));
        assert!(!SESSION_OVERRIDE_KEYS.contains(&"sandbox.profile"));
    }

    /// `humanitl run` follows `sandbox.mode`, by default `auto`; `--disposable`
    /// and `--direct` decide for one run. A daemon before minor 27 does not
    /// know `auto` and gets an empty field (HUM-357).
    #[test]
    fn the_start_mode_follows_the_flags_then_the_configuration() {
        use humanitl_config::SandboxMode;
        let args = |disposable: bool, direct: bool| RunArgs {
            cmd: Vec::new(),
            disposable,
            direct,
            yes: false,
        };
        assert_eq!(
            start_mode(&args(false, false), Some(SandboxMode::Auto), 27),
            "auto"
        );
        assert_eq!(
            start_mode(&args(false, false), Some(SandboxMode::Auto), 26),
            ""
        );
        assert_eq!(
            start_mode(&args(false, false), Some(SandboxMode::Direct), 27),
            "direct"
        );
        assert_eq!(
            start_mode(&args(false, false), Some(SandboxMode::Disposable), 26),
            "disposable"
        );
        assert_eq!(
            start_mode(&args(true, false), Some(SandboxMode::Direct), 27),
            "disposable"
        );
        assert_eq!(
            start_mode(&args(false, true), Some(SandboxMode::Auto), 27),
            "direct"
        );
        assert_eq!(
            start_mode(&args(false, true), Some(SandboxMode::Disposable), 26),
            "direct"
        );
        assert_eq!(SandboxMode::default(), SandboxMode::Auto);
    }

    /// Die Wahl des Backends aus der Umgebung des Clients reist mit dem
    /// Start, als Wert der Kommandozeile, und ein Flag gewinnt über sie
    /// (HUM-432). Ein Daemon, der ohne diese Variablen läuft, lehnt dann ab,
    /// was der Client verlangt hat, statt still sein Backend zu starten:
    /// `SANDBOX_034` für eine Laufzeit ohne Podman, `SANDBOX_048` für Podman.
    #[test]
    fn the_backend_choice_of_the_client_environment_reaches_the_daemon() {
        use humanitl_ipc::session::{SessionRequest, SessionResolver};

        let home = tempfile::tempdir().expect("tempdir");
        let base = [
            ("HOME", home.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                home.path().join("cfg").display().to_string(),
            ),
        ];
        let client = |extra: &[(&str, &str)]| {
            let mut pairs: Vec<(String, String)> = base
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect();
            pairs.extend(
                extra
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
            );
            resolve(&ProfileSelection::any(), None, &Env::from_pairs(pairs), &[])
                .expect("the client resolves")
        };
        // Der Daemon: dieselben Pfade, keine der Variablen.
        let daemon_env = Env::from_pairs(base.clone());
        let daemon = SessionResolver::new(
            humanitl_config::Paths::new(daemon_env.clone()),
            resolve(&ProfileSelection::any(), None, &daemon_env, &[]).expect("the daemon resolves"),
        );
        let start = |overrides: Vec<humanitl_ipc::v1::sandbox_request::CliOverride>| {
            daemon.resolve(&SessionRequest {
                overrides: overrides.into_iter().map(|o| (o.path, o.value)).collect(),
                ..SessionRequest::default()
            })
        };

        let runtime = client(&[("HUMANITL_SANDBOX__PODMAN__RUNTIME", "crun")]);
        let sent = super::merge_overrides(Vec::new(), &runtime);
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(
            (sent[0].path.as_str(), sent[0].value.as_str()),
            ("sandbox.podman.runtime", "crun")
        );
        let refused = start(sent).expect_err("a runtime without podman");
        assert_eq!(refused.code.as_str(), "SANDBOX_034", "{}", refused.why);

        let backend = client(&[("HUMANITL_SANDBOX__BACKEND", "podman")]);
        let sent = super::merge_overrides(Vec::new(), &backend);
        let resolved = start(sent).expect("the wish resolves");
        let chosen = humanitl_sandbox::choose_backend(
            &resolved.config.sandbox,
            humanitl_sandbox::AUTO_BACKEND,
        )
        .expect("the shared stage is admitted (HUM-448)");
        assert_eq!(chosen.name, humanitl_sandbox::PODMAN_BACKEND);

        // Ein Flag gewinnt über die Umgebung, und nichts kommt doppelt.
        let sent = super::merge_overrides(
            vec![("sandbox.backend".to_owned(), "auto".to_owned())],
            &backend,
        );
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(sent[0].value, "auto");
        // Ohne Variable reist nichts mit, was der Daemon nicht ohnehin hat.
        assert!(super::merge_overrides(Vec::new(), &client(&[])).is_empty());
        assert!(SESSION_OVERRIDE_KEYS.contains(&"sandbox.podman.runtime"));
    }

    /// The environment of client and daemon in `home`, the client's with
    /// these extra variables.
    fn floor_env(home: &std::path::Path, extra: &[(&str, &str)]) -> Env {
        let mut pairs = vec![
            ("HOME".to_owned(), home.display().to_string()),
            (
                "XDG_CONFIG_HOME".to_owned(),
                home.join("cfg").display().to_string(),
            ),
        ];
        pairs.extend(
            extra
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        );
        Env::from_pairs(pairs)
    }

    /// What `humanitl run ARGS` sends as `cli_overrides`, resolved as
    /// `Context::sources` does: with `--config` the named file replaces the
    /// global `config.toml`.
    fn floor_sent(
        home: &std::path::Path,
        args: &[&str],
        extra: &[(&str, &str)],
        named: Option<&std::path::Path>,
    ) -> Vec<v1::sandbox_request::CliOverride> {
        let matches = crate::cli::command()
            .try_get_matches_from(["humanitl", "run"].iter().chain(args))
            .expect("the command line parses");
        let pairs = crate::cli::config_pairs(&matches);
        let env = floor_env(home, extra);
        let selection = ProfileSelection::any();
        let sources = match named {
            Some(file) => humanitl_config::sources_with_global(
                &selection,
                None,
                &env,
                &pairs,
                Some(file.to_path_buf()),
            ),
            None => humanitl_config::sources_for(&selection, None, &env, &pairs),
        }
        .expect("the sources resolve");
        let client = humanitl_config::load(&sources).expect("the client resolves");
        super::start_overrides(pairs, &client, named.is_some())
    }

    /// The daemon's resolution of a start with these overrides: the same
    /// paths, none of the client's variables, its `config.toml` read afresh.
    fn floor_started(
        home: &std::path::Path,
        overrides: Vec<v1::sandbox_request::CliOverride>,
    ) -> humanitl_config::MinIsolation {
        use humanitl_ipc::session::{SessionRequest, SessionResolver};
        let env = floor_env(home, &[]);
        let daemon = SessionResolver::new(
            humanitl_config::Paths::new(env.clone()),
            resolve(&ProfileSelection::any(), None, &env, &[]).expect("the daemon resolves"),
        );
        let resolved = daemon
            .resolve(&SessionRequest {
                overrides: overrides.into_iter().map(|o| (o.path, o.value)).collect(),
                ..SessionRequest::default()
            })
            .expect("the wish resolves");
        if resolved.config.sandbox.min_isolation == humanitl_config::MinIsolation::Kernel {
            let refused = humanitl_sandbox::choose_backend(
                &resolved.config.sandbox,
                humanitl_sandbox::AUTO_BACKEND,
            )
            .expect_err("auto never reaches the kernel level");
            assert_eq!(refused.code.as_str(), "SANDBOX_035", "{}", refused.why);
        }
        resolved.config.sandbox.min_isolation
    }

    /// `humanitl run --sandbox-min-isolation kernel` raises the floor of the
    /// session (HUM-489): the floor travels in `cli_overrides`, and the
    /// daemon's start with `auto` ends with `SANDBOX_035` instead of running
    /// below it. What travels is the strictest floor of the sources only the
    /// client sees: the flag, `HUMANITL_SANDBOX__MIN_ISOLATION` and a file
    /// named by `--config`; a weaker flag never suppresses a stronger one.
    ///
    /// Mutation proof: without the key in `SESSION_OVERRIDE_KEYS` the
    /// daemon refuses the floor with `CONFIG_003`; with the topmost value
    /// instead of the strictest in `humanitl_config::load`, or with the raw
    /// flag instead of the resolved floor, `policy` beats the variable
    /// `kernel`; without `named_config` in `client_floor` the floor of the
    /// named file stays behind.
    #[test]
    fn the_floor_of_the_command_line_reaches_the_daemon_and_only_raises() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = home.path();
        let named = home.join("other.toml");
        std::fs::write(&named, "[sandbox]\nmin_isolation = \"kernel\"\n").expect("write");
        let kernel_env = [("HUMANITL_SANDBOX__MIN_ISOLATION", "kernel")];
        let policy = ["--sandbox-min-isolation", "policy"];

        for (args, extra, file) in [
            (&["--sandbox-min-isolation", "kernel"][..], &[][..], None),
            (&[][..], &kernel_env[..], None),
            // Ein schwächeres Flag verdrängt die strengere Variable nicht.
            (&policy[..], &kernel_env[..], None),
            // Die Datei aus `--config`, die der Daemon nie liest, auch unter
            // einem schwächeren Flag.
            (&[][..], &[][..], Some(named.as_path())),
            (&policy[..], &[][..], Some(named.as_path())),
        ] {
            let overrides = floor_sent(home, args, extra, file);
            let floors: Vec<&str> = overrides
                .iter()
                .filter(|o| o.path == "sandbox.min_isolation")
                .map(|o| o.value.as_str())
                .collect();
            assert_eq!(floors, ["kernel"], "{args:?} {extra:?} {file:?}");
            assert_eq!(
                floor_started(home, overrides),
                humanitl_config::MinIsolation::Kernel,
                "{args:?} {extra:?} {file:?}"
            );
        }
        // Ohne Quelle des Clients reist keine Untergrenze mit.
        assert!(floor_sent(home, &[], &[], None).is_empty());
    }

    /// A weaker floor never lowers a higher one from the daemon's
    /// `config.toml` (HUM-489): the client, which reads the same file, sends
    /// nothing, and a weaker floor that arrives anyway stays out.
    #[test]
    fn a_weaker_floor_does_not_lower_the_floor_of_the_configuration() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = home.path();
        let config = humanitl_config::Paths::new(floor_env(home, &[])).config_path();
        std::fs::create_dir_all(config.parent().expect("a directory")).expect("mkdir");
        std::fs::write(&config, "[sandbox]\nmin_isolation = \"namespaces\"\n").expect("write");

        assert!(floor_sent(home, &["--sandbox-min-isolation", "policy"], &[], None).is_empty());
        let lowered = vec![v1::sandbox_request::CliOverride {
            path: "sandbox.min_isolation".to_owned(),
            value: "policy".to_owned(),
        }];
        assert_eq!(
            floor_started(home, lowered),
            humanitl_config::MinIsolation::Namespaces
        );
    }

    /// A daemon before contract 1.49 would refuse the floor with a
    /// misleading `CONFIG_003`; the client says `CLI_004` instead, and only
    /// when the floor travels (HUM-489).
    #[test]
    fn an_old_daemon_gets_no_floor_it_would_misread() {
        let floor = vec![humanitl_ipc::v1::sandbox_request::CliOverride {
            path: humanitl_ipc::session::SANDBOX_MIN_ISOLATION_KEY.to_owned(),
            value: "kernel".to_owned(),
        }];
        let refused = super::floor_reaches_the_daemon(floor.clone(), 48)
            .expect_err("the old daemon is too old");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_004");
        assert_eq!(
            super::floor_reaches_the_daemon(floor.clone(), super::SESSION_FLOOR_MINOR)
                .expect("a new daemon takes it"),
            floor
        );
        assert!(
            super::floor_reaches_the_daemon(Vec::new(), 48)
                .expect("nothing to refuse")
                .is_empty()
        );
    }

    /// Ein Daemon vor Vertrag 1.61 lehnte `agent.adapter` mit `CONFIG_003`
    /// ab, als dürfe eine Sitzung ihren Agenten nie wählen; der Client sagt
    /// stattdessen `CLI_004` (#8).
    ///
    /// Mutation proof: with `SESSION_FLOOR_MINOR` (1.49) as the bar, contract
    /// 1.60 passes and the first assertion fails.
    #[test]
    fn an_old_daemon_gets_no_agent_it_would_misread() {
        let agent = vec![humanitl_ipc::v1::sandbox_request::CliOverride {
            path: humanitl_ipc::session::AGENT_ADAPTER_KEY.to_owned(),
            value: "claude".to_owned(),
        }];
        let refused =
            super::agent_reaches_the_daemon(&agent, 60).expect_err("the old daemon is too old");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_004");
        assert!(
            refused.diagnostic.why.contains("claude"),
            "{}",
            refused.diagnostic.why
        );
        super::agent_reaches_the_daemon(&agent, super::SESSION_AGENT_MINOR)
            .expect("a new daemon takes it");
    }
}
