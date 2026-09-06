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
//! # Was dieser Befehl nicht tut
//!
//! Er verbindet dieses Terminal nicht mit dem des Agenten. Der Daemon startet
//! die Sitzung seit HUM-042 zwar an einem Pseudoterminal, aber dieser Befehl
//! liest nur mit: Seine Ausgabe kommt als Bytes über den Ereignisstrom und
//! geht unverändert auf `stdout` dieses Prozesses; gefiltert wird sie im
//! Daemon ([`humanitl_core::TerminalFilter`], Politik `ColourOnly`). Damit
//! gibt es hier keinen Raw-Modus, keine Weiterleitung der Fenstergröße, keine
//! Eingabe an den Agenten und kein `Ctrl+]`-Menü — das alles hat
//! `humanitl sandbox attach`, und `--ask terminal` verweigert weiterhin den
//! Dienst mit `CLI_002`.
//!
//! Zwei Dinge folgen aus dem Pseudoterminal und sind hier sichtbar: Ein
//! Terminal hat einen Strom, also kommt alles auf `stdout` und nichts auf
//! `stderr`, und seine Zeilendisziplin macht aus `\n` ein `\r\n`.
//!
//! `Ctrl+C` beendet die Sitzung über `Sandbox(Stop)`; ohne Eingabekanal
//! erreicht kein Byte den Agenten. Wer tippen will, hängt sich mit
//! `humanitl sandbox attach` an dieselbe Sitzung.

use std::ffi::OsString;

use humanitl_config::{AskMode, Config};
use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::session::{SESSION_OVERRIDE_KEYS, ask_mode_name};
use humanitl_ipc::v1;
use humanitl_sandbox::agent::{AdapterRegistry, AgentAdapter};
use serde_json::json;
use std::io::Write as _;

use crate::cli::RunArgs;
use crate::cmd::{Context, EXIT_CHECK, EXIT_USER, Failure, from_proto, status_diagnostic};

/// Führt `humanitl run` aus.
///
/// # Errors
///
/// `CONFIG_001` bis `CONFIG_003`, wenn das Profil oder ein Flag nicht stimmt,
/// `CLI_002` für `--ask terminal`, `DAEMON_001` ohne Daemon, `DAEMON_002` bei
/// einer anderen Vertrags-Major und die Befunde des Daemons für alles, was am
/// Start scheitert.
pub async fn run(ctx: &Context, args: &RunArgs) -> Result<u8, Failure> {
    // Zuerst und immer: Das Projekt-Profil wird gelesen, bevor irgendetwas
    // startet.
    let resolved = ctx.config()?;
    let config = &resolved.config;
    ctx.render.detail(&session_lines(&resolved, args));

    // Beide Seiten müssen ein Terminal sein: Die Tasten kommen von `stdin`,
    // und der Kasten geht nach `stderr`. Ein Lauf mit `2>log` schriebe ihn
    // samt seiner Steuerfolgen in eine Datei, und niemand sähe die Frage.
    let on_terminal = std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stderr());
    refuse_terminal_ask(config, args, on_terminal)?;

    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;

    let work_dir = config
        .sandbox
        .work_dir
        .clone()
        .unwrap_or_else(|| ctx.cwd.clone());
    let start = v1::sandbox_request::Start {
        // Das bwrap-Profil bleibt der Konfiguration überlassen; unter
        // `humanitl run` benennt `--profile` das Profil der Sitzung
        // (`backlog/CONVENTIONS.md` 4.23).
        profile: String::new(),
        work_dir: work_dir.display().to_string(),
        work_mode: work_mode_name(config.sandbox.work_mode).to_owned(),
        command: args
            .cmd
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        session_profile: ctx.profile_flag().unwrap_or_default().to_owned(),
        ask_mode: ask_mode_name(config.hold.ask_mode).to_owned(),
        cli_overrides: session_overrides(ctx),
    };

    let session = json!({
        "work_dir": work_dir.display().to_string(),
        "profile": ctx.profile_flag().unwrap_or("default"),
        "ask_mode": ask_mode_name(config.hold.ask_mode),
        "command": args
            .cmd
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    });

    ctx.render
        .note(&where_decisions_happen(config.hold.ask_mode));
    let moderate = config.hold.ask_mode == AskMode::Terminal;
    let code = drive(ctx, &mut client, start, moderate).await?;
    if ctx.render.is_json() {
        let mut value = session;
        value["exit_code"] = json!(code);
        ctx.render.value(&value);
    }
    Ok(code)
}

/// Startet die Sitzung und begleitet sie bis zum Ende des Agenten.
async fn drive(
    ctx: &Context,
    client: &mut Client,
    start: v1::sandbox_request::Start,
    moderate: bool,
) -> Result<u8, Failure> {
    // **Erst das Abonnement, dann der Start.** Der Daemon liefert keinen
    // Rückstand: `Subscribe` mit leerem `since_flow_id` beginnt bei jetzt.
    // Ein Agent, der seine erste Anfrage in der ersten Sekunde stellt --
    // OpenCode ruft beim Start seinen Katalog ab --, hielte sie also, bevor
    // dieser Strom stünde, und niemand am Terminal erführe davon; der Agent
    // liefe in die Frist.
    let (mut flows, mut moderation) = open_moderation(client, moderate).await?;

    let mut events = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(start)),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox(Start)")))?
        .into_inner();

    let mut failure: Option<Failure> = None;
    let mut exit: Option<i32> = None;
    let mut interrupted = false;

    let mut clock = tokio::time::interval(std::time::Duration::from_secs(1));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut terminate = signal_stream(tokio::signal::unix::SignalKind::terminate());
    let mut hangup = signal_stream(tokio::signal::unix::SignalKind::hangup());
    // **Einmal angelegt und nicht je Umlauf.** `tokio::signal::ctrl_c()` baut
    // bei jedem Aufruf einen frischen Empfänger; ein `SIGINT`, das eintrifft,
    // während die Schleife gerade arbeitet -- ein Schwall Ausgabe, ein
    // offener Editor --, fällt zwischen den alten und den neuen Empfänger und
    // wird nie gesehen. Ein Strom, der von Anfang an steht, puffert es.
    let mut interrupt = signal_stream(tokio::signal::unix::SignalKind::interrupt());

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
        let next = tokio::select! {
            event = events.message() => event,
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
            number = next_signal(&mut terminate, &mut hangup) => {
                return Ok(end_on_signal(ctx, &mut stopper, moderation.as_mut(), number).await);
            }
        };
        let event = match next {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(status) => {
                failure.get_or_insert_with(|| {
                    Failure::new(status_diagnostic(&status, "Sandbox(Start)"))
                });
                break;
            }
        };
        if let Some(v1::sandbox_event::Event::Exit(ended)) = event.event.as_ref() {
            exit = Some(ended.code);
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
        if let Some(found) = handle(ctx, event) {
            failure.get_or_insert(found);
        }
    }

    if let Some(moderation) = moderation.as_mut() {
        moderation.finish();
    }

    outcome(exit, failure)
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
                    "journalctl --user -u humanitld -n 50".to_owned(),
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

/// Beendet die Sitzung auf ein Signal hin und liefert den Exit-Code `128 + n`.
async fn end_on_signal(
    ctx: &Context,
    stopper: &mut Client,
    moderation: Option<&mut crate::cmd::moderate::Moderation>,
    number: u8,
) -> u8 {
    if let Some(open) = moderation {
        open.finish();
    }
    ctx.render.note("[humanitl] stopping the session");
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
fn handle(ctx: &Context, event: v1::SandboxEvent) -> Option<Failure> {
    use v1::sandbox_event::Event;

    match event.event? {
        Event::Check(result) => {
            ctx.render.note(&check_line(&result));
            let diagnostic = result.diagnostic.as_ref().and_then(from_proto)?;
            Some(Failure::with_exit(diagnostic, EXIT_CHECK))
        }
        Event::Diagnostic(diagnostic) => {
            let diagnostic = from_proto(&diagnostic)?;
            ctx.render
                .note(&crate::render::diagnostic_block(&diagnostic));
            (diagnostic.severity >= Severity::Error).then(|| Failure::new(diagnostic))
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
        Event::Exit(_) => None,
    }
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
fn where_decisions_happen(ask_mode: AskMode) -> String {
    match ask_mode {
        AskMode::Ui => "[humanitl] a request without a rule waits for a decision in the app; \
                        without one it is blocked when the hold timeout is over"
            .to_owned(),
        AskMode::None => {
            "[humanitl] nobody is asked: a request without a rule is blocked right away".to_owned()
        }
        // Unerreichbar: `refuse_terminal_ask` hat vorher abgebrochen. Die Zeile
        // steht trotzdem da, damit ein künftiger Zweig nicht stillschweigend
        // nichts sagt.
        AskMode::Terminal => "[humanitl] --ask terminal is not available yet".to_owned(),
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
    if !args.cmd.is_empty() {
        return Ok(());
    }
    let registry = AdapterRegistry::builtin();
    let fullscreen = registry
        .get(&config.agent.adapter)
        .is_some_and(AgentAdapter::is_fullscreen_tui);
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
fn session_overrides(ctx: &Context) -> Vec<v1::sandbox_request::CliOverride> {
    ctx.cli_pairs()
        .into_iter()
        .filter(|(path, _)| SESSION_OVERRIDE_KEYS.contains(&path.as_str()))
        .map(|(path, value)| v1::sandbox_request::CliOverride { path, value })
        .collect()
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
    use humanitl_ipc::session::SESSION_OVERRIDE_KEYS;

    use super::{
        Config, chain, check_line, inline_rules, refuse_terminal_ask, session_lines, state_name,
    };
    use crate::cli::RunArgs;
    use crate::cmd::{EXIT_CHECK, EXIT_USER};

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
        let text = session_lines(&resolved, &RunArgs { cmd: Vec::new() });

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
    fn no_cmd() -> RunArgs {
        RunArgs { cmd: Vec::new() }
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
            super::where_decisions_happen(AskMode::Ui).contains("in the app"),
            "ui points at the app"
        );
        assert!(
            super::where_decisions_happen(AskMode::None).contains("blocked right away"),
            "none says that nobody is asked"
        );
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
        let args = RunArgs {
            cmd: vec![std::ffi::OsString::from("bash")],
        };
        assert!(refuse_terminal_ask(&config, &args, true).is_ok());
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
}
