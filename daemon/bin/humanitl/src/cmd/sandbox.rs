//! `humanitl sandbox run|argv|check`: die Sandbox planen, starten und prüfen.
//!
//! # Eine befristete Abweichung von ADR-018
//!
//! ADR-018 sagt: Jede Fähigkeit ist zuerst eine RPC, die Kommandozeile ist ein
//! dünner Client ohne Fachlogik. Diese drei Unterkommandos halten sich in M1
//! nicht daran. Der Grund steht im Vertrag: `Sandbox` antwortet mit
//! `UNIMPLEMENTED` und kommt erst in Sprint 3 (HUM-040). Es gibt also nichts,
//! wovon ein dünner Client hier Client sein könnte, und `humanitl sandbox
//! argv` ist genau das Kommando, mit dem man vor dem ersten Start nachsieht,
//! was die Sandbox tun wird.
//!
//! Deshalb ruft dieses Modul `humanitl-sandbox` unmittelbar auf, im Prozess
//! der Kommandozeile: dasselbe [`SandboxProfile`], derselbe
//! [`PlatformBackend`], derselbe [`LaunchPlan`](humanitl_sandbox::LaunchPlan) wie später im Daemon. Es gibt
//! keine zweite Übersetzung des Profils und keine Fachlogik, die es nur hier
//! gäbe; was hier passiert, ist Verdrahtung, die mit der `Sandbox`-RPC
//! umzieht. Die Naht ist [`Wiring`]: sie sagt, woher Proxy-Socket und CA
//! kommen. Mit der RPC verschwindet sie, `run` schickt `Sandbox(Start …)` und
//! liest den `SandboxEvent`-Strom, `argv` fragt `Sandbox(Plan)`, `check` liest
//! die Prüfungen aus dem Strom. `backlog/CONVENTIONS.md` 4.12 hält die
//! Abweichung fest, damit sie niemand für ein Versehen hält.
//!
//! Zwei Unterschiede zum späteren Weg über den Daemon sind heute unvermeidbar
//! und stehen deshalb hier:
//!
//! - `sandbox run` verlangt einen laufenden Daemon, obwohl es ihn nicht ruft:
//!   der Proxy-Socket und das CA-Bundle gehören ihm, und ohne sie hat der
//!   Agent in der Sandbox kein Netz.
//! - Der Isolations-Check läuft nach dem Start, nicht davor. Der Shim
//!   schreibt seine Prüfzeilen unmittelbar vor dem `exec`; scheitert eine
//!   Garantie, beendet `run` die Sandbox sofort und endet mit 3. Über die RPC
//!   prüft der Daemon vor dem Start des Agenten, und der Agent läuft dann gar
//!   nicht erst an.

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;

use humanitl_config::{Config, Env, Paths, Resolved};
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::{
    AdapterRegistry, AgentContext, CheckResult, INTERRUPT_GRACE, LaunchInputs, MountPolicy,
    PlatformBackend, Refusal, RefusalReason, RefusalReporting, Refusals, SANDBOX_SHELL,
    SandboxBackend, SandboxFile, SandboxHandle, SandboxProfile, SandboxView, SessionContext,
    StdioMode, WorkDirSource, shell_line,
};
use serde_json::json;
use tempfile::TempDir;
use tokio::signal::unix::{Signal, SignalKind, signal};

use crate::cli::SandboxCmd;
use crate::cmd::{Context, EXIT_CHECK, EXIT_OK, Failure, is_executable, status_diagnostic};
use crate::render::{diagnostic_block, table, tick};

/// Der Name des Shims, wie er neben der Kommandozeile oder im Systempfad liegt.
const SHIM_BINARY: &str = "humanitl-shim";

/// Wo ein installiertes Humanitl den Shim ablegt, in dieser Reihenfolge.
const SHIM_DIRS: &[&str] = &[
    "/usr/lib/humanitl",
    "/usr/libexec/humanitl",
    "/usr/local/lib/humanitl",
];

/// Der Name des CA-Bundles im CA-Verzeichnis.
///
/// Dieselbe Datei wie `humanitl_proxy::ca::BUNDLE_FILE`; der Daemon schreibt
/// sie bei jedem Start neu. Der Name steht hier als Text, damit die
/// Kommandozeile nicht die halbe Proxy-Crate mitzieht, um einen Dateinamen zu
/// erfahren.
const CA_BUNDLE_FILE: &str = "ca-bundle.crt";

/// Wo ein installiertes Humanitl die Sandbox-Profile ablegt.
///
/// Das Arbeitsverzeichnis steht bewusst nicht darunter: ein geklontes
/// Repository darf die Politik der Sandbox nicht mitbringen
/// (`backlog/CONVENTIONS.md` 4.11). Wer ein Profil aus dem Baum will, nennt
/// seinen Pfad (`--profile ./profiles/sandbox/test.toml`).
const PROFILE_DIRS: &[&str] = &["/usr/share/humanitl", "/usr/local/share/humanitl"];

/// Das Unterverzeichnis der Sandbox-Profile.
const PROFILE_SUBDIR: &str = "profiles/sandbox";

/// Wie viele Elternverzeichnisse des Binaries nach `profiles/sandbox`
/// abgesucht werden.
const TREE_DEPTH: usize = 6;

/// Der Befehl, mit dem `sandbox check` die Sandbox kurz am Leben hält.
const CHECK_COMMAND: &[&str] = &[SANDBOX_SHELL, "-c", "sleep 1"];

/// Der Exit-Code, mit dem ein durch `SIGINT` beendeter Lauf endet.
const EXIT_INTERRUPTED: u8 = 130;

/// Führt `humanitl sandbox <cmd>` aus.
///
/// # Errors
///
/// `CONFIG_001`, wenn das Profil fehlt, `SANDBOX_001` bis `SANDBOX_012`, wenn
/// der Start scheitert, `SANDBOX_013` bis `SANDBOX_016`, wenn eine Garantie
/// nicht gilt, und `DAEMON_001`, wenn `run` keinen Daemon findet.
pub async fn run(ctx: &Context, cmd: &SandboxCmd) -> Result<u8, Failure> {
    match cmd {
        SandboxCmd::Argv { cmd } => argv(ctx, cmd),
        SandboxCmd::Run {
            cmd,
            tests_dir,
            disposable,
            direct,
        } => start(ctx, cmd, tests_dir.as_deref(), *disposable, *direct).await,
        SandboxCmd::Check => check(ctx),
        SandboxCmd::Plan => crate::cmd::plan::run(ctx).await,
        SandboxCmd::Attach {
            read_only,
            detach_keys,
            shell,
        } => {
            crate::cmd::attach::run(
                ctx,
                *read_only,
                detach_keys.as_deref(),
                shell.as_deref().unwrap_or_default(),
            )
            .await
        }
        SandboxCmd::Exec { detach_keys, cmd } => {
            crate::cmd::exec::run(ctx, cmd, detach_keys.as_deref()).await
        }
        SandboxCmd::Changes { stat } => crate::cmd::changes::changes(ctx, *stat).await,
        SandboxCmd::Apply {
            ids,
            overwrite,
            yes,
        } => crate::cmd::changes::apply(ctx, ids, *overwrite, *yes).await,
        SandboxCmd::Discard { yes } => crate::cmd::changes::discard(ctx, *yes).await,
    }
}

/// Woher Proxy-Socket, CA und Arbeitsverzeichnis kommen.
///
/// Die Naht zur `Sandbox`-RPC: heute entscheidet die Kommandozeile das, später
/// der Daemon. Jede Variante liefert dieselben vier Pfade.
#[derive(Debug)]
enum Wiring {
    /// Die Dateien des laufenden Daemons (`sandbox run`).
    Daemon,
    /// Platzhalter in einem eigenen Verzeichnis (`sandbox check`): ein
    /// gebundener, unbenutzter Socket und zwei leere Dateien. Der Selbsttest
    /// darf dem laufenden Daemon nichts wegnehmen und braucht ihn nicht.
    Placeholder(Box<Placeholder>),
    /// Nur die Pfade, ohne dass es sie geben muss (`sandbox argv`).
    Preview,
}

/// Die Platzhalter eines Selbsttests; leben, solange dieser Wert lebt.
///
/// Nur das Laufzeitverzeichnis zieht um, nicht `$HOME`, `$XDG_CONFIG_HOME`
/// oder `$XDG_DATA_HOME`: die [`MountPolicy`] soll weiter die echten
/// Verzeichnisse des Nutzers schützen. Die CA-Platzhalter liegen deshalb
/// ausdrücklich unter [`Placeholder::dir`] und nicht in
/// `Paths::ca_dir`, wo die echte CA des Daemons liegt.
#[derive(Debug)]
struct Placeholder {
    /// Das Verzeichnis, das beim Aufräumen alles mitnimmt.
    dir: TempDir,
    /// Der gebundene Socket. Hinter ihm antwortet niemand.
    _socket: UnixListener,
    /// Die Pfade mit dem umgebogenen Laufzeitverzeichnis.
    paths: Paths,
}

/// Alles, was ein Start braucht: Profil, Sitzung, Backend.
struct Setup {
    /// Das gelesene und geprüfte Profil.
    profile: SandboxProfile,
    /// Die Sitzung, für die geplant wird.
    session: SessionContext,
    /// Das Backend, mit dem geplant und gestartet wird.
    backend: PlatformBackend,
    /// Hält die Platzhalter am Leben, solange die Sandbox läuft.
    _wiring: Wiring,
}

/// `sandbox argv`: die Kommandozeile, ohne zu starten.
fn argv(ctx: &Context, command: &[OsString]) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    // Ein Name ohne gespeicherten Server: dieselbe Ablehnung wie beim Start
    // (HUM-400), statt einer Zeile ohne Modell.
    if let Some(refused) = resolved
        .config
        .llm
        .choice
        .as_ref()
        .and_then(humanitl_config::servers::ServerChoice::refusal)
    {
        return Err(Failure::new(refused));
    }
    let setup = prepare(ctx, &resolved, Wiring::Preview, command)?;
    // Die Vorschau, nicht der Plan: feste Deskriptornummern, alles unter
    // `/work` als vorhanden. Der Plan legt Deskriptoren an und verlangt
    // Socket, CA und Shim; nichts davon braucht, wer nur nachsehen will.
    let mut args = vec![setup.backend.program().as_os_str().to_owned()];
    args.extend(setup.profile.to_bwrap_args(
        &setup.session,
        &LaunchInputs::preview_with_agent_files(
            setup.session.files.iter().map(|file| file.dst.clone()),
        ),
    ));
    let line = shell_line(&args);

    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "profile": setup.profile.name,
            "program": setup.backend.program().display().to_string(),
            "argv": args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<String>>(),
            "argv_line": line,
        }));
    } else {
        ctx.render.line(&line);
    }
    Ok(EXIT_OK)
}

/// `sandbox run`: starten, durchreichen, mit dem Code des Befehls enden.
async fn start(
    ctx: &Context,
    command: &[OsString],
    tests_dir: Option<&Path>,
    disposable: bool,
    direct: bool,
) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    let disposable = throwaway_wanted(disposable, direct, resolved.config.sandbox.mode);
    // Ohne das Overlay von bubblewrap gibt es keine Wegwerf-Sandbox (HUM-350).
    #[cfg(not(target_os = "linux"))]
    if disposable {
        return Err(Failure::new(humanitl_sandbox::disposable::unsupported()));
    }

    // Schritt 1 der Spezifikation: ohne Daemon kein Lauf. Er hält den
    // Proxy-Socket und das CA-Bundle; ohne beides hätte der Agent in der
    // Sandbox kein Netz, und die Sitzung stünde in keiner Aufzeichnung.
    let mut client = ctx.connect().await?;
    client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?;
    // Der Server der Sitzung (HUM-400): Dieser Befehl startet die Sandbox im
    // eigenen Prozess, und die Durchreiche des Proxys ist die, die der Daemon
    // zuletzt eingerichtet hat. Nie ein Agent für den einen Server und eine
    // Durchreiche zum anderen.
    let passthrough = daemon_passthrough(&mut client).await?;
    session_server_gate(&resolved.config, passthrough.as_ref()).map_err(Failure::new)?;

    let mut setup = prepare(ctx, &resolved, Wiring::Daemon, command)?;
    if let Some(dir) = tests_dir {
        let src = take_tests_placeholder(ctx, &mut setup.profile, dir)?;
        ctx.render
            .detail(&format!("tests dir {} -> {TESTS_DIR_DST}", src.display()));
        setup.backend = setup.backend.with_read_only_bind(src, TESTS_DIR_DST);
    }
    // The disposable sandbox (HUM-350): this command keeps no layer after it
    // ends, so the changes are listed and dropped; `humanitl run --disposable`
    // keeps them for apply.
    #[cfg(target_os = "linux")]
    let mut throwaway = if disposable {
        Some(Throwaway::prepare(ctx, &mut setup)?)
    } else {
        None
    };
    let plan = setup
        .backend
        .plan(&setup.profile, &setup.session)
        .map_err(Failure::new)?;
    ctx.render.detail(&format!("argv: {}", plan.argv_line()));

    // Die Quelle für `SIGINT` steht, bevor die Sandbox läuft. Erst das Anmelden
    // eines Handlers hebt ein geerbtes `SIG_IGN` auf, und zwischen dem Start der
    // Sandbox und dem ersten Warten liegt sonst ein Fenster, in dem ein `SIGINT`
    // spurlos verschwindet: Der Agent liefe weiter, und der Mensch hielte die
    // Tastenkombination für kaputt.
    let interrupts = signal(SignalKind::interrupt()).map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::SANDBOX_012, Severity::Blocking)
                .why(format!("SIGINT cannot be listened for: {error}"))
                .build(),
        )
    })?;

    let handle = Arc::new(setup.backend.launch(&plan).map_err(Failure::new)?);
    ctx.render
        .detail(&format!("sandbox {} pid {}", handle.id, handle.pid));
    #[cfg(target_os = "linux")]
    if let Some(throwaway) = throwaway.as_mut() {
        throwaway.watch(&handle).map_err(Failure::new)?;
    }

    enforce_isolation(ctx, &setup.backend, &handle)?;
    let ended = wait_or_interrupt(Arc::clone(&handle), interrupts).await;
    report_refusals(ctx, &handle).await;
    #[cfg(target_os = "linux")]
    {
        if let Some(throwaway) = &throwaway {
            throwaway.report(ctx, &handle);
        }
        drop(throwaway);
    }
    ended
}

/// Die mitgelieferte Durchreiche, die der Daemon gerade hält, als Regel;
/// `None` ohne eine (HUM-400).
async fn daemon_passthrough(
    client: &mut humanitl_ipc::client::Client,
) -> Result<Option<humanitl_core::rule::Rule>, Failure> {
    let response = client
        .rules(humanitl_ipc::v1::RulesRequest {
            op: Some(humanitl_ipc::v1::rules_request::Op::List(())),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Rules")))?
        .into_inner();
    let Some(wire) = response.rules.iter().find(|rule| rule.passthrough_llm) else {
        return Ok(None);
    };
    humanitl_ipc::convert::rule_from_proto(wire, SessionId::new())
        .map(Some)
        .map_err(|error| {
            Failure::new(
                Diagnostic::builder(codes::CONFIG_021, Severity::Error)
                    .why(format!(
                        "the daemon's passthrough to the language model cannot be read ({error}); \
                         without it this command cannot tell whether the proxy lets the agent's \
                         server through"
                    ))
                    .fix(FixAction::CopyCommand("humanitl run".to_owned()))
                    .build(),
            )
        })
}

/// Lässt `sandbox run` nur zu, wenn der Proxy genau den Server durchlässt,
/// für den der Agent eingerichtet wird (HUM-400).
///
/// `CONFIG_021` für einen Namen ohne gespeicherten Server, wie beim Start
/// über den Daemon. Sonst wird die Durchreiche, die diese Konfiguration
/// bekäme, mit derselben Funktion gebaut, mit der der Daemon sie baut
/// ([`humanitl_ipc::session::session_passthrough`]), und als Bedingung mit
/// der Regel verglichen, die der Daemon hält (`daemon`, aus
/// [`daemon_passthrough`]): Schema, Host, Port, Methoden und die genauen
/// Inferenzpfade, typisiert und nicht als Text. Das gilt für jede Wahl, auch
/// für `llm.active` aus einer anderen `--config` und für einen Endpunkt ohne
/// gespeicherten Server: Dieser Befehl kann die Sitzung des Daemons nicht
/// umstellen, und sonst spräche der Agent mit dem einen Server, während der
/// Proxy den anderen durchließe. Der Weg mit eigenem Server ist
/// `humanitl run`.
///
/// # Errors
///
/// `CONFIG_021` in beiden Fällen.
fn session_server_gate(
    config: &Config,
    daemon: Option<&humanitl_core::rule::Rule>,
) -> Result<(), Diagnostic> {
    if let Some(refused) = config
        .llm
        .choice
        .as_ref()
        .and_then(humanitl_config::servers::ServerChoice::refusal)
    {
        return Err(refused);
    }
    let wanted = humanitl_ipc::session::session_passthrough(config);
    let same = match (&wanted, daemon) {
        (None, None) => true,
        (Some(wanted), Some(held)) => {
            wanted.matcher == held.matcher && wanted.action == held.action
        }
        _ => false,
    };
    if same {
        return Ok(());
    }
    let chosen = config.llm.choice.as_ref().map_or_else(
        || "this configuration".to_owned(),
        |choice| format!("{} (the model server {})", choice.source, choice.name),
    );
    Err(Diagnostic::builder(codes::CONFIG_021, Severity::Error)
        .why(format!(
            "{chosen} sets up the agent for {}, and the daemon passes model traffic to {}; \
             humanitl sandbox run starts the sandbox in this process and cannot give the \
             daemon's session another server, so the agent would talk to one server while the \
             proxy lets another through. Start the session with humanitl run, which resolves \
             its own server",
            describe_passthrough(wanted.as_ref()),
            describe_passthrough(daemon)
        ))
        .fix(FixAction::CopyCommand("humanitl run".to_owned()))
        .build())
}

/// Eine Durchreiche für den Satz eines Befunds; verglichen wird nie dieser
/// Text, sondern die Bedingung selbst.
fn describe_passthrough(rule: Option<&humanitl_core::rule::Rule>) -> String {
    let Some(rule) = rule else {
        return "no server".to_owned();
    };
    let matcher = &rule.matcher;
    format!(
        "{}://{}{} ({})",
        matcher
            .scheme
            .map_or_else(|| "any".to_owned(), |scheme| scheme.to_string()),
        matcher.host,
        matcher
            .port
            .map_or_else(String::new, |port| format!(" port {port}")),
        matcher.path_prefixes.join(", ")
    )
}

/// Ob `sandbox run` eine Wegwerf-Sandbox startet (HUM-357).
///
/// Die Flags gewinnen; ohne sie nur ein ausdrückliches `sandbox.mode =
/// "disposable"`. Die Vorgabe `auto` heißt hier `direct`: Dieser Befehl hält
/// keine Schicht über sein Ende hinaus, und eine Vorgabe, die alles verwirft,
/// was der Lauf schreibt, nähme der Fluchtprüfung (`tests/escape`) und den
/// Demos (`tests/e2e`) ihre Ergebnisdateien unter `/work`. Wer die Änderungen
/// prüfen und übernehmen will, nimmt `humanitl run`.
fn throwaway_wanted(disposable: bool, direct: bool, mode: humanitl_config::SandboxMode) -> bool {
    if disposable || direct {
        return disposable;
    }
    mode == humanitl_config::SandboxMode::Disposable
}

/// The layer of `sandbox run --disposable`; dropped, it is discarded.
///
/// It carries what the daemon's run carries (HUM-350): the project folder
/// opened once before the start, the baseline taken on it before the start,
/// and a size guard that stops the sandbox when the layer grows over its cap.
#[cfg(target_os = "linux")]
struct Throwaway {
    layer: Option<humanitl_sandbox::Layer>,
    lower: std::os::fd::OwnedFd,
    baseline: humanitl_sandbox::worktree::TreeSnapshot,
    guard: Option<humanitl_sandbox::SizeGuard>,
    child_start: Option<u64>,
}

#[cfg(target_os = "linux")]
impl Throwaway {
    /// Opens the project folder, takes the baseline and creates the layer
    /// before the plan, so the plan mounts exactly the opened folder.
    fn prepare(ctx: &Context, setup: &mut Setup) -> Result<Self, Failure> {
        use std::os::fd::{AsFd as _, AsRawFd as _};

        let lower =
            humanitl_sandbox::worktree::open_root(&setup.session.work_src).map_err(Failure::new)?;
        let resolved =
            std::fs::read_link(format!("/proc/self/fd/{}", lower.as_raw_fd())).map_err(|err| {
                Failure::new(
                    Diagnostic::builder(codes::SANDBOX_030, Severity::Blocking)
                        .why(format!(
                            "where the opened project folder lies cannot be read ({err})"
                        ))
                        .build(),
                )
            })?;
        let stat = rustix::fs::fstat(&lower).map_err(|err| {
            Failure::new(
                Diagnostic::builder(codes::SANDBOX_030, Severity::Blocking)
                    .why(format!(
                        "the opened project folder cannot be inspected ({err})"
                    ))
                    .build(),
            )
        })?;
        let baseline = humanitl_sandbox::disposable::baseline_at(lower.as_fd());
        let layer =
            humanitl_sandbox::Layer::create(&ctx.paths.runtime_dir().path).map_err(Failure::new)?;
        setup.backend = setup
            .backend
            .clone()
            .with_approved_work_dir(resolved)
            .with_expected_work_inode(stat.st_dev, stat.st_ino)
            .with_overlay(layer.dirs());
        Ok(Self {
            layer: Some(layer),
            lower,
            baseline,
            guard: None,
            child_start: None,
        })
    }

    /// Starts the size guard for the running sandbox and notes the start
    /// time of its first process.
    fn watch(&mut self, handle: &Arc<SandboxHandle>) -> Result<(), Diagnostic> {
        let Some(layer) = self.layer.as_ref() else {
            return Ok(());
        };
        let stop = Arc::clone(handle);
        self.guard = Some(humanitl_sandbox::SizeGuard::start(layer, move || {
            let _ = stop.kill();
        })?);
        self.child_start = handle
            .child_pid()
            .and_then(|pid| humanitl_sandbox::disposable::start_ticks(Path::new("/proc"), pid));
        Ok(())
    }

    /// Says what the run changed before the layer goes.
    fn report(&self, ctx: &Context, handle: &SandboxHandle) {
        use std::os::fd::AsFd as _;

        if let Some(guard) = &self.guard {
            guard.stop();
            if let Some(tripped) = guard.tripped() {
                ctx.render.diagnostic(&tripped);
            }
        }
        let Some(layer) = self.layer.as_ref() else {
            return;
        };
        if let Err(diagnostic) =
            humanitl_sandbox::disposable::ensure_ended(handle, self.child_start)
        {
            ctx.render.diagnostic(&diagnostic);
            return;
        }
        let Ok(upper) = humanitl_sandbox::worktree::open_root(&layer.upper()) else {
            return;
        };
        let set =
            humanitl_sandbox::disposable::scan(upper.as_fd(), self.lower.as_fd(), &self.baseline);
        ctx.render.note(&format!(
            "disposable: {} change(s) discarded, the project folder is untouched{}",
            set.changes.len(),
            if set.truncated {
                " (list cut short)"
            } else {
                ""
            }
        ));
        for change in &set.changes {
            ctx.render.note(&format!(
                "  {:<8} {}",
                change.kind.as_str(),
                change.display()
            ));
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for Throwaway {
    fn drop(&mut self) {
        if let Some(layer) = self.layer.take()
            && let Err((layer, diagnostic)) = layer.discard()
        {
            eprintln!(
                "the upper layer {} stays until the daemon starts again: {}",
                layer.dir().display(),
                diagnostic.why
            );
        }
    }
}

/// Wie lange nach dem Ende auf die letzten Zeilen über verweigerte Versuche
/// gewartet wird; der Shim schreibt sie, nachdem der Agent gegangen ist.
const REFUSALS_DRAIN: std::time::Duration = std::time::Duration::from_secs(2);

/// Sagt nach dem Lauf, was die Sandbox dem Befehl an Sockets verweigert hat
/// (HUM-138).
///
/// Die Zahlen stammen aus dem Bericht des Shims, dieselben, die der Daemon
/// als `SandboxEvent.refusals` sendet; hier wird nur gezeigt. Meldet die
/// Sandbox nicht, steht dort `SANDBOX_019` statt einer leeren Liste. Eine
/// Zeile je Paar aus Familie und Typ, damit ein Mensch und `tests/escape/`
/// dieselbe Zeile lesen.
async fn report_refusals(ctx: &Context, handle: &Arc<SandboxHandle>) {
    let waiting = Arc::clone(handle);
    let Ok((refusals, _)) =
        tokio::task::spawn_blocking(move || waiting.wait_refusals(u64::MAX, REFUSALS_DRAIN)).await
    else {
        return;
    };
    if let Some(diagnostic) = refusals.unreported() {
        ctx.render.diagnostic(&diagnostic);
        return;
    }
    match refusal_notes(&refusals) {
        RefusalNotes::Quiet(line) => ctx.render.detail(&line),
        RefusalNotes::Loud(lines) => {
            for line in &lines {
                ctx.render.note(line);
            }
        }
    }
}

/// Was nach dem Lauf über die verweigerten Versuche zu sagen ist.
#[derive(Debug, PartialEq, Eq)]
enum RefusalNotes {
    /// Eine Zeile für `-v`: nichts verweigert, oder nichts gemeldet.
    Quiet(String),
    /// Zeilen für jeden: Es wurde verweigert.
    Loud(Vec<String>),
}

/// Die Zeilen zu `refusals`, ohne den Fall `off`, den `SANDBOX_019` sagt.
///
/// „Nichts verweigert" darf nur stehen, wenn die Sandbox gesagt hat, dass sie
/// zählt. Hat sie gar nichts gesagt (`Unknown`), ist das eine eigene Zeile:
/// Ein Bericht, der ausblieb, ist kein Bericht über null Versuche.
fn refusal_notes(refusals: &Refusals) -> RefusalNotes {
    if refusals.entries.is_empty() {
        return RefusalNotes::Quiet(match refusals.reporting {
            RefusalReporting::On => "the sandbox refused no socket outside the proxy".to_owned(),
            RefusalReporting::Unknown | RefusalReporting::Off(_) => {
                "the sandbox did not say whether it counts refused sockets".to_owned()
            }
        });
    }
    let mut lines = vec![format!(
        "the sandbox refused {} attempt(s) to open a socket outside the proxy:",
        refusals.total()
    )];
    lines.extend(refusals.entries.iter().map(refusal_line));
    RefusalNotes::Loud(lines)
}

/// Eine Zeile je Paar: `  3x socket(AF_UNIX, SOCK_STREAM)  family not allowed`.
fn refusal_line(entry: &Refusal) -> String {
    let why = match entry.reason {
        RefusalReason::Family => "family not allowed",
        RefusalReason::Type => "type not allowed",
        RefusalReason::Overflow => "further pairs, counted together",
    };
    format!(
        "  {}x {}({}, {})  {why}",
        entry.count, entry.syscall, entry.family, entry.socket_type
    )
}

/// `sandbox check`: eine kurzlebige Sandbox und die drei Garantien.
fn check(ctx: &Context) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    let command: Vec<OsString> = CHECK_COMMAND.iter().map(OsString::from).collect();
    let wiring = Wiring::Placeholder(Box::new(placeholder()?));
    let setup = prepare(ctx, &resolved, wiring, &command)?;

    // Gesammelt statt durchgereicht: was der Prüfbefehl sagt, gehört nicht in
    // die Tabelle.
    let backend = setup.backend.with_stdio(StdioMode::Capture);
    let plan = backend
        .plan(&setup.profile, &setup.session)
        .map_err(Failure::new)?;
    ctx.render.detail(&format!("argv: {}", plan.argv_line()));

    let handle = backend.launch(&plan).map_err(Failure::new)?;
    let results = backend.isolation_check(&handle);
    handle.kill();
    let _ = handle.wait();

    report(ctx, &results);
    if results.iter().all(|result| result.passed) {
        Ok(EXIT_OK)
    } else {
        Err(check_failure(&results))
    }
}

/// Die Tabelle der drei Garantien, oder ihr JSON.
fn report(ctx: &Context, results: &[CheckResult]) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "passed": results.iter().all(|result| result.passed),
            "checks": results
                .iter()
                .map(|result| json!({
                    "check": result.check.as_str(),
                    "passed": result.passed,
                    "evidence": result.evidence,
                    "diagnostic": result
                        .diagnostic
                        .as_ref()
                        .map(crate::render::diagnostic_json),
                }))
                .collect::<Vec<serde_json::Value>>(),
        }));
        return;
    }
    let rows: Vec<Vec<String>> = results
        .iter()
        .map(|result| {
            vec![
                tick(result.passed).to_owned(),
                result.check.as_str().to_owned(),
                crate::render::one_line(&result.evidence),
            ]
        })
        .collect();
    print!("{}", table(&["", "CHECK", "EVIDENCE"], &rows));
}

/// Der Befund der ersten gescheiterten Garantie, mit Exit-Code 3.
fn check_failure(results: &[CheckResult]) -> Failure {
    let failed = results.iter().find(|result| !result.passed);
    let diagnostic = failed
        .and_then(|result| result.diagnostic.clone())
        .unwrap_or_else(|| {
            Diagnostic::builder(codes::SANDBOX_004, Severity::Blocking)
                .why(failed.map_or_else(
                    || "the sandbox reported no isolation check at all".to_owned(),
                    |result| {
                        format!(
                            "{} failed: {}",
                            result.check.as_str(),
                            crate::render::one_line(&result.evidence)
                        )
                    },
                ))
                .build()
        });
    Failure::with_exit(diagnostic, EXIT_CHECK)
}

/// Liest die drei Garantien aus der laufenden Sandbox und bricht ab, wenn eine
/// fehlt.
///
/// Eine Sandbox, deren Isolation nicht belegt ist, ist keine; sie hier weiter
/// laufen zu lassen hieße, dem Agenten ein Netz zu geben, von dem niemand
/// weiß, wohin es führt (derselbe Grund wie in `escape-launch`,
/// `backlog/CONVENTIONS.md` 4.12).
fn enforce_isolation(
    ctx: &Context,
    backend: &PlatformBackend,
    handle: &SandboxHandle,
) -> Result<(), Failure> {
    let results = backend.isolation_check(handle);
    for result in &results {
        ctx.render.detail(&format!(
            "check {} {}: {}",
            result.check.as_str(),
            if result.passed { "pass" } else { "FAIL" },
            crate::render::one_line(&result.evidence)
        ));
    }
    if results.iter().all(|result| result.passed) {
        return Ok(());
    }
    handle.kill();
    let _ = handle.wait();
    Err(check_failure(&results))
}

// --- Das Testverzeichnis ----------------------------------------------------
//
// `profiles/sandbox/test.toml` nennt in `mounts.extra_ro` den Pfad
// `/tests/escape`. Der ist ein Platzhalter: er steht für den Ort in der
// Sandbox, nicht für einen Ort auf dem Host. `--tests-dir DIR` zieht die
// Quelle genau dieses einen Binds auf ein Verzeichnis des Arbeitsbaums, damit
// die Escape-Tests nach `backlog/CONVENTIONS.md` 3.11 über
// `humanitl sandbox run --profile test -- /tests/escape/esc-N-<name>.sh`
// laufen und nicht über ein Ad-hoc-Skript.
//
// Das umgeht die Mount-Allowlist bewusst, genau wie das Projektverzeichnis:
// beides kommt aus dem Aufruf, nicht aus dem Profil (siehe
// [`MountPolicy`]). Derselbe Eingriff steht in
// `daemon/crates/sandbox/src/bin/escape-launch.rs`; mit der `Sandbox`-RPC
// (HUM-040) wandert er hinter sie, als Feld der Sitzung.
//
// Seit HUM-263 öffnet der Plan jede Quelle aus `mounts.extra_ro` und bindet
// sie per Deskriptor; einen Platzhalter, den es auf dem Host nicht gibt, lehnt
// er ab. Deshalb nimmt `--tests-dir` den Platzhalter vor dem Planen aus dem
// Profil und gibt das Verzeichnis dem Backend als nur lesbaren Bind der
// Sitzung (`with_read_only_bind` des Backends): geöffnet, gegen die
// `MountPolicy` geprüft und per Deskriptor gebunden wie die Erweiterungen,
// an derselben Stelle der Zeile.

/// Der Platzhalter, den `profiles/sandbox/test.toml` in `mounts.extra_ro` nennt.
const TESTS_DIR_DST: &str = "/tests/escape";

/// Nimmt [`TESTS_DIR_DST`] aus `mounts.extra_ro` und liefert die Quelle, die
/// an seine Stelle tritt.
///
/// # Errors
///
/// `SANDBOX_010`, wenn das Profil den Platzhalter nicht nennt: dann zeigt
/// `--tests-dir` auf nichts, und ein Lauf, der die Skripte nicht sieht,
/// bewiese nichts.
fn take_tests_placeholder(
    ctx: &Context,
    profile: &mut SandboxProfile,
    dir: &Path,
) -> Result<PathBuf, Failure> {
    let src = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        ctx.cwd.join(dir)
    };
    let before = profile.mounts.extra_ro.len();
    profile
        .mounts
        .extra_ro
        .retain(|path| path.as_path() != Path::new(TESTS_DIR_DST));
    if profile.mounts.extra_ro.len() == before {
        return Err(Failure::new(
            Diagnostic::builder(codes::SANDBOX_010, Severity::Blocking)
                .why(format!(
                    "the sandbox profile does not name {TESTS_DIR_DST} in mounts.extra_ro, so \
                     --tests-dir {} has nothing to point at",
                    src.display()
                ))
                .fix(FixAction::ChangeSetting {
                    key: "sandbox.profile".to_owned(),
                    value: "test".to_owned(),
                })
                .build(),
        ));
    }
    Ok(src)
}

/// Wartet auf das Ende der Sandbox und reicht `SIGINT` weiter.
///
/// Die Sandbox läuft mit `--new-session`, hat also kein steuerndes Terminal
/// und bekommt das `SIGINT` der Tastatur nicht. Die Kommandozeile bekommt es
/// und gibt es weiter, statt die Sandbox zu erschlagen: `SIGINT` ist die
/// Bitte, selbst aufzuhören, und der Agent hat dafür eigene Handler, die
/// aufräumen und mit einem eigenen Code enden. Deshalb endet der Lauf danach
/// mit dem Code des Agenten und nicht pauschal mit 130.
///
/// Die Bitte ist befristet. Wer nach [`INTERRUPT_GRACE`] noch läuft, hat sie
/// entweder nicht gehört oder ignoriert sie; dann folgt
/// [`SandboxHandle::kill`] (`SIGTERM`, dann `SIGKILL`), und der Lauf endet mit
/// [`EXIT_INTERRUPTED`]. In beiden Fällen wird auf das Ende gewartet, bevor
/// die Kommandozeile selbst endet: ein halb beendeter Agent hinterließe
/// Dateien im Projekt.
async fn wait_or_interrupt(
    handle: Arc<SandboxHandle>,
    mut interrupts: Signal,
) -> Result<u8, Failure> {
    let waiting = Arc::clone(&handle);
    let mut waiter = tokio::task::spawn_blocking(move || waiting.wait());

    tokio::select! {
        joined = &mut waiter => finish(joined),
        signal = interrupts.recv() => {
            if signal.is_none() {
                // Die Quelle ist geschlossen; dann bleibt nur warten, und der
                // Lauf endet mit dem Code des Befehls wie ohne Unterbrechung.
                return finish(waiter.await);
            }
            let stopping = Arc::clone(&handle);
            let stopped = tokio::task::spawn_blocking(move || {
                let stopped = stopping.interrupt(INTERRUPT_GRACE);
                if !stopped {
                    stopping.kill();
                }
                stopped
            })
            .await;
            let joined = waiter.await;
            // Nur wer selbst aufgehört hat, hat einen eigenen Exit-Code.
            // Nach `kill` ist der Status der des Signals, und der sagt über
            // den Agenten nichts mehr aus.
            if matches!(stopped, Ok(true)) {
                finish(joined)
            } else {
                Ok(EXIT_INTERRUPTED)
            }
        }
    }
}

/// Das Ergebnis des wartenden Threads als Exit-Code.
fn finish(
    joined: Result<Result<ExitStatus, Diagnostic>, tokio::task::JoinError>,
) -> Result<u8, Failure> {
    match joined {
        Ok(Ok(status)) => Ok(exit_code_of(status)),
        Ok(Err(diagnostic)) => Err(Failure::new(diagnostic)),
        Err(error) => Err(Failure::new(
            Diagnostic::builder(codes::SANDBOX_012, Severity::Blocking)
                .why(format!(
                    "the thread waiting for the sandbox failed: {error}"
                ))
                .build(),
        )),
    }
}

/// Der Exit-Code eines Prozesses: sein eigener, oder `128 + Signal`.
///
/// Über 255 gibt es keinen; `ExitStatus::code` liefert nie mehr, und ein
/// Signal wird nach POSIX-Sitte auf `128 + n` abgebildet.
#[must_use]
fn exit_code_of(status: ExitStatus) -> u8 {
    use std::os::unix::process::ExitStatusExt as _;

    if let Some(code) = status.code() {
        return u8::try_from(code).unwrap_or(1);
    }
    status
        .signal()
        .and_then(|signal| u8::try_from(128 + signal).ok())
        .unwrap_or(1)
}

/// Profil, Sitzung und Backend für einen Lauf.
fn prepare(
    ctx: &Context,
    resolved: &Resolved,
    wiring: Wiring,
    command: &[OsString],
) -> Result<Setup, Failure> {
    let config = &resolved.config;
    let paths = wiring.paths(ctx);
    let policy = MountPolicy::from_paths(&paths);
    let profile_path = profile_path(ctx, &config.sandbox.profile)?;
    let profile = SandboxProfile::load_validated(&profile_path, &policy).map_err(Failure::new)?;
    ctx.render
        .detail(&format!("profile {}", profile_path.display()));

    let id = SessionId::new();
    let (work_src, work_source) = wiring.work_dir(ctx, resolved);
    // Der gespeicherte Ordner, `--work` für diesen Aufruf, sonst das aktuelle
    // Verzeichnis (HUM-293), und vor dem Plan geprüft: Der Plan des Backends
    // stellt dieselben Fragen, kennt aber die Quelle nicht, und der Befund
    // soll sagen, woher der Pfad kam. Die Vorschau startet nichts und prüft
    // deshalb nicht.
    if matches!(wiring, Wiring::Daemon) {
        humanitl_sandbox::check_project_dir(&policy, &work_src, &work_source)
            .map_err(Failure::new)?;
    }
    // Ein gespeicherter Ordner geht hier wie im Daemon durch alle drei Stufen,
    // gegen seinen heutigen aufgelösten Pfad (HUM-294): Sonst hängte dieser
    // Weg ohne Daemon einen Ordner ein, dessen Elternverzeichnis inzwischen
    // ein Verweis nach außerhalb des Heimatverzeichnisses ist. Der
    // freigegebene Ort geht an das Backend, das nur einen Deskriptor an genau
    // diesem Ort bindet. `--work` und das aktuelle Verzeichnis bleiben bei der
    // Politik allein, wie bisher auf diesem Weg.
    let approved = match (&wiring, &work_source) {
        (
            Wiring::Daemon,
            WorkDirSource::ConfigFile(_) | WorkDirSource::Profile(_) | WorkDirSource::Env(_),
        ) => Some(
            humanitl_ipc::work_dir::check_requested(
                &paths,
                Some(&work_src),
                &work_src,
                &work_source,
            )
            .map_err(Failure::new)?,
        ),
        _ => None,
    };
    // Der Beitrag des Agent-Adapters gilt dem Agenten, und zwar am wirksamen
    // Kommando: `sandbox run -- /bin/sh` (so laufen die Escape-Tests) ist keiner
    // und bekommt nichts, `sandbox run -- opencode` ist einer und bekommt
    // dasselbe wie `humanitl run` (HUM-135). Der Daemon gibt seit HUM-280
    // jedem Kommando den Beitrag und bindet den konfigurierten Agenten
    // daneben, damit eine Shell aus `humanitl run -- bash` ihn findet. Dieser
    // Weg ohne Daemon bleibt bei der alten Regel: Er ist der der Escape-Suiten,
    // die genau die Sandbox ohne Agentendateien prüfen, und er startet am
    // geerbten Terminal, nie an einem eigenen Pseudoterminal.
    let agent =
        if command.is_empty() || humanitl_sandbox::agent::command_is_the_agent(config, command) {
            agent_contribution(ctx, config, &wiring, id, &work_src, &profile, command)?
        } else {
            AgentContribution::default()
        };
    let mut session_env = vec![("HUMANITL_SESSION".to_owned(), id.to_string())];
    session_env.extend(agent.env);
    // `sandbox.env` zuletzt: Was der Mensch in seiner Konfiguration setzt,
    // gilt vor dem `[env]` des Profils und vor dem Beitrag des Adapters. Das
    // ist der Weg, auf dem `FixAction::SetEnv` wirkt (HUM-045): Ein Werkzeug,
    // das die Vorgabe des Profils nicht mag, bekommt hier einen anderen Wert.
    // Die fünf Variablen des Shims bleiben unberührt, sie werden in
    // `SandboxProfile::effective_env` zuletzt gesetzt
    // (`humanitl_sandbox::bridge_env::RESERVED_ENV`).
    session_env.extend(
        config
            .sandbox
            .env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    let session = SessionContext {
        session: id,
        work_src,
        work_mode: config.sandbox.work_mode,
        proxy_socket_src: wiring.proxy_socket(ctx),
        ca_cert_src: wiring.ca_cert(ctx),
        ca_bundle_src: wiring.ca_bundle(ctx),
        shim_src: shim_path(&wiring)?,
        // Nur die Variable, die allein die Sitzung kennt, plus die des
        // Adapters; die übrigen Paare des Env-Kits stehen im `[env]` des
        // Profils (`humanitl_proxy::ca::ENV_KIT`).
        session_env,
        // A bound program rewrote the adapter's command; the caller's own
        // words would name the host path the sandbox does not have.
        command: if agent.program.is_some() {
            agent.command
        } else {
            agent_command(config, command, &agent.command)
        },
        files: agent.files,
        agent_program: agent.program,
    };

    let backend = match PlatformBackend::detect(paths.clone()) {
        Ok(backend) => backend,
        // Die Vorschau braucht kein `bwrap`; der Start schon.
        Err(_) if matches!(wiring, Wiring::Preview) => PlatformBackend::fallback(paths),
        Err(diagnostic) => return Err(Failure::new(diagnostic)),
    };
    let backend = match approved {
        Some(dir) => backend.with_approved_work_dir(dir),
        None => backend,
    };

    Ok(Setup {
        profile,
        session,
        backend: backend.with_stdio(StdioMode::Inherit),
        _wiring: wiring,
    })
}

/// Was der Agent-Adapter zu einer Sitzung beiträgt.
#[derive(Debug, Default)]
struct AgentContribution {
    /// Das Kommando, das der Shim nach seccomp startet; leer, wenn kein
    /// Adapter beiträgt.
    command: Vec<OsString>,
    /// The agent program bound from outside the mounts (HUM-278); with it,
    /// `command` already names its place in the sandbox.
    program: Option<humanitl_sandbox::agent::AgentProgram>,
    /// Die Umgebungsvariablen des Agenten.
    env: Vec<(String, String)>,
    /// Die Dateien, die vor dem `exec` in der Sandbox liegen müssen.
    files: Vec<SandboxFile>,
}

/// Fragt den Adapter aus `agent.adapter` nach Umgebung und Dateien.
///
/// Die Vorprüfung läuft vorher und fasst das Netz nicht an. Ein blockierender
/// Befund verhindert den Start; alles Darunter wird angezeigt, und der Start
/// läuft weiter.
///
/// Trägt in den Kontext ein, was die Sandbox vom Dateibaum sieht.
///
/// Die Sicht des Profils (`SandboxView::of_profile`: Einhängungen, Verweise,
/// Überdeckungen), der wirksame Suchpfad und das Arbeitsverzeichnis
/// (`[mounts].work.dst`). Ein nacktes Kommando löst der Agent in der Sandbox
/// gegen diese Angaben auf, und die Vorprüfung muss dieselbe Frage stellen;
/// ohne sie suchte sie nur im `PATH` des Hosts und verweigerte einen Start,
/// der funktioniert (HUM-139).
fn with_sandbox_view(ctx: AgentContext, config: &Config, profile: &SandboxProfile) -> AgentContext {
    ctx.with_sandbox_view(SandboxView::of_profile(profile))
        .with_sandbox_path(effective_sandbox_path(config, profile).map(OsString::from))
        .with_work_dir_sandbox(profile.mounts.work.dst.clone())
}

/// Der Suchpfad, der in der Sandbox wirklich gilt.
///
/// `sandbox.env` gewinnt über das `[env]` des Profils, weil
/// `SandboxProfile::effective_env` die Sitzungsumgebung zuletzt einträgt. Wer
/// hier nur das Profil läse,
/// prüfte gegen einen Pfad, den der Start gar nicht setzt: Ein
/// `sandbox.env.PATH` auf ein eingehängtes Verzeichnis wäre ein Befund, obwohl
/// es läuft, und einer auf ein leeres Verzeichnis bliebe unbemerkt, obwohl das
/// `exec` mit 127 endet (HUM-139).
fn effective_sandbox_path<'a>(
    config: &'a Config,
    profile: &'a SandboxProfile,
) -> Option<&'a String> {
    config
        .sandbox
        .env
        .get("PATH")
        .or_else(|| profile.env.get("PATH"))
}

/// Die Vorschau ([`Wiring::Preview`]) hält kein Befund auf. `sandbox argv` ist
/// genau das Kommando, mit dem man vor der Installation nachsieht, was passieren
/// wird; es wäre die falsche Stelle, um einen fehlenden Agenten zu melden. Die
/// Befunde stehen trotzdem auf `stderr`, außer mit `-q` oder `--json`.
fn agent_contribution(
    ctx: &Context,
    config: &Config,
    wiring: &Wiring,
    session: SessionId,
    work_src: &Path,
    profile: &SandboxProfile,
    command: &[OsString],
) -> Result<AgentContribution, Failure> {
    let registry = AdapterRegistry::builtin();
    let adapter = registry.get(&config.agent.adapter).ok_or_else(|| {
        Failure::new(
            Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
                .why(format!(
                    "agent.adapter is {:?}, and no adapter of that name exists; known: {}",
                    config.agent.adapter,
                    registry.ids().join(", ")
                ))
                .fix(FixAction::ChangeSetting {
                    key: "agent.adapter".to_owned(),
                    value: registry
                        .ids()
                        .first()
                        .map_or_else(String::new, |id| (*id).to_owned()),
                })
                .build(),
        )
    })?;

    let agent_ctx = AgentContext::new(session, work_src.to_path_buf(), config.llm.clone())
        // Das Kommando, das wirklich läuft, steht weiter unten: Es hängt daran,
        // ob der Aufrufer eines genannt hat, und die Herkunft geht mit.
        .with_host_path(ctx.env.non_empty("PATH").map(OsString::from))
        .with_host_home(ctx.env.non_empty("HOME").map(PathBuf::from))
        .with_language(config.ui.language)
        // Frist und Ask-Modus stehen im Briefing (HUM-071); sie müssen die
        // sein, nach denen der Proxy arbeitet, sonst nennt der Text dem Agenten
        // eine Wartezeit, die es nicht gibt.
        .with_hold(config.hold.clone())
        .with_briefing(config.agent.briefing.clone())
        .with_pi_web_access(config.agent.pi.web_access)
        // Das Heimatverzeichnis, das der Agent tatsächlich vorfindet, und das
        // Konfigurationsverzeichnis dazu.
        //
        // Die Reihenfolge ist die von `SandboxProfile::effective_env`: das
        // `[env]` des Profils zuerst, dann `session_env` — darin der Beitrag
        // des Adapters und **danach** `sandbox.env`. Wer dort `HOME` oder
        // `XDG_CONFIG_HOME` setzt, verschiebt damit das Verzeichnis, in dem
        // der Agent seine Konfiguration und seine Einweisung sucht. Nähme der
        // Adapter statt dessen den Wert des Profils, hängte die Sandbox die
        // Dateien an einen Ort, den niemand liest, und nichts fiele auf
        // (HUM-071, Review vom 2026-09-04).
        .with_home(
            config
                .sandbox
                .env
                .get("HOME")
                .or_else(|| profile.env.get("HOME"))
                .map_or_else(
                    || PathBuf::from(humanitl_sandbox::DEFAULT_HOME),
                    PathBuf::from,
                ),
        )
        // Nur `sandbox.env`: ein `XDG_CONFIG_HOME` im `[env]` des Profils
        // überschreibt der Adapter selbst und kann es deshalb nicht verlieren.
        .with_config_home(config.sandbox.env.get("XDG_CONFIG_HOME").map(PathBuf::from));
    // Was die Sandbox vom Dateibaum sieht, ihr Suchpfad und ihr
    // Arbeitsverzeichnis.
    let agent_ctx = with_sandbox_view(agent_ctx, config, profile);
    let agent_ctx = if command.is_empty() {
        agent_ctx.with_command_override(
            config
                .agent
                .command
                .as_ref()
                .map(|parts| parts.iter().map(OsString::from).collect()),
        )
    } else {
        agent_ctx.with_command_from_caller(command.to_vec())
    };

    let preview = matches!(wiring, Wiring::Preview);
    for diagnostic in adapter.preflight(&agent_ctx) {
        if diagnostic.severity == Severity::Blocking && !preview {
            return Err(Failure::new(diagnostic));
        }
        // Nicht `detail`: das druckt nur mit `-v`, und ein Befund, den niemand
        // sieht, ist keiner (`backlog/CONVENTIONS.md` 4.13). `note` schreibt
        // auf `stderr`, lässt also `stdout` für das Ergebnis frei.
        ctx.render.note(diagnostic_block(&diagnostic).trim_end());
    }

    let mut command = adapter.command(&agent_ctx);
    let program = humanitl_sandbox::agent::bind_agent_program(&agent_ctx, &mut command);
    if cfg!(target_os = "macos") {
        command = adapter.on_shared_loopback(command);
    }
    Ok(AgentContribution {
        command,
        program,
        env: adapter.env(&agent_ctx),
        files: adapter.files(&agent_ctx).map_err(Failure::new)?,
    })
}

/// Der Befehl in der Sandbox: der von der Kommandozeile, sonst der des
/// Adapters, sonst der aus `agent.command`, sonst die Shell.
///
/// Der Adapter kennt `agent.command` selbst
/// ([`AgentContext::agent_command_override`]); steht dort etwas, gibt er es
/// unverändert zurück. Die beiden letzten Zweige greifen nur, wenn gar kein
/// Adapter beiträgt — dann ist die Shell die einzige sinnvolle Antwort.
fn agent_command(
    config: &Config,
    command: &[OsString],
    from_adapter: &[OsString],
) -> Vec<OsString> {
    if !command.is_empty() {
        return command.to_vec();
    }
    if !from_adapter.is_empty() {
        return from_adapter.to_vec();
    }
    config
        .agent
        .command
        .as_ref()
        .filter(|command| !command.is_empty())
        .map_or_else(
            || vec![OsString::from(SANDBOX_SHELL)],
            |command| command.iter().map(OsString::from).collect(),
        )
}

impl Wiring {
    /// Die Pfade, unter denen Socket und Laufzeitverzeichnis liegen.
    fn paths(&self, ctx: &Context) -> Paths {
        match self {
            Self::Placeholder(placeholder) => placeholder.paths.clone(),
            Self::Daemon | Self::Preview => ctx.paths.clone(),
        }
    }

    /// Der Unix-Socket, den der Agent in der Sandbox erreicht.
    fn proxy_socket(&self, ctx: &Context) -> PathBuf {
        self.paths(ctx).proxy_socket()
    }

    /// Das CA-Zertifikat auf dem Host.
    fn ca_cert(&self, ctx: &Context) -> PathBuf {
        match self {
            Self::Placeholder(placeholder) => placeholder.dir.path().join("ca.crt"),
            Self::Daemon | Self::Preview => ctx.paths.ca_cert_path(),
        }
    }

    /// Das CA-Bundle auf dem Host.
    fn ca_bundle(&self, ctx: &Context) -> PathBuf {
        match self {
            Self::Placeholder(placeholder) => placeholder.dir.path().join(CA_BUNDLE_FILE),
            Self::Daemon | Self::Preview => ctx.paths.ca_dir().join(CA_BUNDLE_FILE),
        }
    }

    /// Das Verzeichnis, das als `/work` eingehängt wird, und woher es kommt.
    fn work_dir(&self, ctx: &Context, resolved: &Resolved) -> (PathBuf, WorkDirSource) {
        if let Self::Placeholder(placeholder) = self {
            // Ein Selbsttest fasst das Projekt des Nutzers nicht an.
            return (placeholder.dir.path().join("work"), WorkDirSource::Request);
        }
        ctx.work_dir(resolved)
    }
}

/// Legt Socket, CA-Platzhalter und Arbeitsverzeichnis eines Selbsttests an.
fn placeholder() -> Result<Placeholder, Failure> {
    let dir =
        TempDir::new().map_err(|error| placeholder_failed("a temporary directory", &error))?;
    // Das Laufzeitverzeichnis liegt neben dem Arbeitsverzeichnis, nicht
    // darüber: die `MountPolicy` verbietet jeden Mount unterhalb des
    // Laufzeitverzeichnisses, und `/work` wäre sonst genau das.
    let runtime = dir.path().join("runtime");
    std::fs::create_dir_all(&runtime)
        .map_err(|error| placeholder_failed("the runtime directory", &error))?;
    let paths = Paths::new(
        Env::from_process().with("XDG_RUNTIME_DIR", runtime.to_string_lossy().into_owned()),
    );

    let work = dir.path().join("work");
    std::fs::create_dir_all(&work)
        .map_err(|error| placeholder_failed("the work directory", &error))?;

    let socket_dir = paths.proxy_socket_dir();
    std::fs::create_dir_all(&socket_dir)
        .map_err(|error| placeholder_failed("the proxy directory", &error))?;
    let socket_path = paths.proxy_socket();
    let socket = UnixListener::bind(&socket_path)
        .map_err(|error| placeholder_failed("the proxy socket", &error))?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| placeholder_failed("the mode of the proxy socket", &error))?;

    for file in [dir.path().join("ca.crt"), dir.path().join(CA_BUNDLE_FILE)] {
        std::fs::write(&file, b"")
            .map_err(|error| placeholder_failed("a CA placeholder", &error))?;
    }

    Ok(Placeholder {
        dir,
        _socket: socket,
        paths,
    })
}

/// Der Befund, wenn ein Platzhalter nicht angelegt werden kann.
fn placeholder_failed(what: &str, error: &std::io::Error) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::SANDBOX_011, Severity::Blocking)
            .why(format!(
                "cannot create {what} for the isolation check: {error}"
            ))
            .build(),
    )
}

/// Die Datei des Sandbox-Profils.
///
/// Der Name ist ein Name, kein Pfad: `humanitl_config` lehnt einen
/// Pfadtrenner in `sandbox.profile` mit `CONFIG_003` ab, damit keine Datei aus
/// einem geklonten Repository die Politik der Sandbox stellen kann. Gesucht
/// wird deshalb an drei Orten, in dieser Reihenfolge:
///
/// 1. `$XDG_CONFIG_HOME/humanitl/profiles/sandbox/<name>.toml`, wo der Nutzer
///    seine eigenen Profile ablegt,
/// 2. `profiles/sandbox/<name>.toml` neben dem Binary, in einem der
///    Elternverzeichnisse: das ist der Quellbaum beim Entwickeln,
/// 3. die Verzeichnisse einer Installation ([`PROFILE_DIRS`]).
///
/// Das Arbeitsverzeichnis steht nicht darunter. Der zweite Ort hängt am Ort
/// des Binaries, nicht am Ort des Aufrufs; wer ein fremdes Repository klont
/// und darin `humanitl` aufruft, bringt damit kein Profil mit.
pub(crate) fn profile_path(ctx: &Context, name: &str) -> Result<PathBuf, Failure> {
    let file = format!("{name}.toml");
    let mut candidates = vec![ctx.paths.profiles_dir().join("sandbox").join(&file)];
    candidates.extend(tree_dirs().map(|dir| dir.join(&file)));
    candidates.extend(
        PROFILE_DIRS
            .iter()
            .map(|dir| Path::new(dir).join(PROFILE_SUBDIR).join(&file)),
    );
    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| missing_profile(name, &candidates))
}

/// Die Kandidaten für `profiles/sandbox` im Baum über dem Binary.
///
/// `target/debug/humanitl` und `target/<ziel>/debug/humanitl` liegen
/// unterschiedlich tief; deshalb wird eine begrenzte Zahl von
/// Elternverzeichnissen abgeklopft statt einer festen Tiefe.
fn tree_dirs() -> impl Iterator<Item = PathBuf> {
    std::env::current_exe().ok().into_iter().flat_map(|exe| {
        exe.ancestors()
            .skip(1)
            .take(TREE_DEPTH)
            .map(|dir| dir.join(PROFILE_SUBDIR))
            .collect::<Vec<PathBuf>>()
    })
}

/// Der Befund für ein Profil, das nirgends liegt.
fn missing_profile(name: &str, candidates: &[PathBuf]) -> Failure {
    let looked = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Failure::new(
        Diagnostic::builder(codes::CONFIG_001, Severity::Blocking)
            .why(format!(
                "no sandbox profile {name}; looked at {looked}. A profile from the working \
                 directory is not searched, only one named by its path"
            ))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.profile".to_owned(),
                value: "default".to_owned(),
            })
            .build(),
    )
}

/// Der Shim auf dem Host.
///
/// Zuerst neben der Kommandozeile (derselbe `target/debug` im Baum), dann in
/// den Verzeichnissen einer Installation. Die Vorschau nimmt den ersten
/// Kandidaten, auch wenn es ihn nicht gibt: sie zeigt die Zeile, sie startet
/// sie nicht.
fn shim_path(wiring: &Wiring) -> Result<PathBuf, Failure> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(SHIM_BINARY));
    }
    candidates.extend(SHIM_DIRS.iter().map(|dir| Path::new(dir).join(SHIM_BINARY)));

    if let Some(found) = candidates.iter().find(|path| is_executable(path)) {
        return Ok(found.clone());
    }
    if matches!(wiring, Wiring::Preview) {
        return Ok(candidates
            .first()
            .cloned()
            .unwrap_or_else(|| Path::new(SHIM_DIRS[0]).join(SHIM_BINARY)));
    }

    let looked = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(Failure::new(
        Diagnostic::builder(codes::SANDBOX_011, Severity::Blocking)
            .why(format!(
                "no executable {SHIM_BINARY}; looked at {looked}. Without it the sandbox has no \
                 bridge to the proxy and reports no isolation check"
            ))
            .fix(FixAction::CopyCommand(
                "cargo build -p humanitl-shim".to_owned(),
            ))
            .build(),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    /// `sandbox run` lehnt einen unbekannten Server ab und jede
    /// Konfiguration, deren Durchreiche nicht die des Daemons ist (HUM-400).
    ///
    /// Mutationsbeweise: Ein Abgleich nur für Projekt und Lauf ließe den Fall
    /// `llm.active` aus einer anderen `--config` durch; ein Abgleich nur auf
    /// `host:port` ließe `/a/v1` gegen `/b/v1` durch.
    #[test]
    fn sandbox_run_never_splits_agent_and_passthrough() {
        use humanitl_config::servers::{ServerChoice, ServerSource};

        let config = |endpoint: Option<&str>, choice: Option<ServerChoice>| {
            let mut config = humanitl_config::Config::default();
            config.agent.adapter = "opencode".to_owned();
            config.llm.endpoint = endpoint.map(|text| text.parse().unwrap());
            config.llm.choice = choice;
            config
        };
        let saved = |name: &str, source: ServerSource| ServerChoice {
            name: name.to_owned(),
            source,
            known: true,
            saved: vec!["a".to_owned(), "b".to_owned()],
        };
        // The daemon's rule as `Rules(List)` carries it over the wire.
        let daemon_on = |endpoint: &str| {
            let rule = humanitl_ipc::session::session_passthrough(&config(Some(endpoint), None))
                .expect("a passthrough");
            humanitl_ipc::convert::rule_from_proto(
                &humanitl_ipc::convert::rule_to_proto(&rule),
                humanitl_core::ids::SessionId::new(),
            )
            .expect("the rule crosses the wire")
        };
        let gate = |config: &humanitl_config::Config, held: Option<&humanitl_core::rule::Rule>| {
            super::session_server_gate(config, held)
        };

        let unknown = gate(
            &config(
                None,
                Some(ServerChoice {
                    known: false,
                    ..saved("gone", ServerSource::Run)
                }),
            ),
            None,
        )
        .expect_err("an unsaved server is refused");
        assert_eq!(unknown.code.as_str(), "CONFIG_021");
        assert!(unknown.why.contains("gone"), "{}", unknown.why);

        let a = daemon_on("https://model.lan/a/v1");
        // `llm.active` from another --config: B for the agent, A at the proxy.
        let split = gate(
            &config(
                Some("https://other.lan/v1"),
                Some(saved("b", ServerSource::Default)),
            ),
            Some(&a),
        )
        .expect_err("the active server of another config is refused");
        assert!(split.why.contains("humanitl run"), "{}", split.why);
        // Same host and port, other inference paths.
        assert!(
            gate(&config(Some("https://model.lan/b/v1"), None), Some(&a)).is_err(),
            "another path on the same host is another server"
        );
        // Same host and path, other scheme.
        assert!(gate(&config(Some("http://model.lan:443/a/v1"), None), Some(&a)).is_err());
        // An endpoint without any passthrough at the daemon.
        assert!(gate(&config(Some("https://model.lan/a/v1"), None), None).is_err());
        // IPv6, written two ways: the typed rule is the same.
        let v6 = daemon_on("http://[::1]:8080/v1");
        assert!(
            gate(
                &config(Some("http://[0:0:0:0:0:0:0:1]:8080/v1"), None),
                Some(&v6)
            )
            .is_ok()
        );

        assert!(
            gate(
                &config(
                    Some("https://model.lan/a/v1"),
                    Some(saved("a", ServerSource::Project("/p".into())))
                ),
                Some(&a)
            )
            .is_ok(),
            "the daemon passes the chosen server: no mismatch"
        );
        assert!(gate(&config(None, None), None).is_ok(), "no model at all");
    }

    use std::ffi::OsString;
    use std::os::unix::process::ExitStatusExt as _;
    use std::path::Path;
    use std::process::ExitStatus;

    use humanitl_config::Config;

    use super::{
        CHECK_COMMAND, RefusalNotes, agent_command, exit_code_of, placeholder, refusal_notes,
        throwaway_wanted, with_sandbox_view,
    };

    /// „Nichts verweigert" steht nur, wenn die Sandbox gesagt hat, dass sie
    /// zählt; ein Bericht, der ausblieb, ist eine eigene Zeile (HUM-138).
    #[test]
    fn nothing_refused_is_said_only_when_the_sandbox_counts() {
        use humanitl_sandbox::{RefusalLine, RefusalReporting, Refusals, parse_refusal_line};

        let mut refusals = Refusals::default();
        assert_eq!(
            refusal_notes(&refusals),
            RefusalNotes::Quiet(
                "the sandbox did not say whether it counts refused sockets".to_owned()
            )
        );
        refusals.apply(RefusalLine::Reporting(RefusalReporting::On));
        assert_eq!(
            refusal_notes(&refusals),
            RefusalNotes::Quiet("the sandbox refused no socket outside the proxy".to_owned())
        );
        let line = parse_refusal_line("REFUSED socket AF_UNIX SOCK_STREAM family 3 1 2").unwrap();
        refusals.apply(line);
        assert_eq!(
            refusal_notes(&refusals),
            RefusalNotes::Loud(vec![
                "the sandbox refused 3 attempt(s) to open a socket outside the proxy:".to_owned(),
                "  3x socket(AF_UNIX, SOCK_STREAM)  family not allowed".to_owned(),
            ])
        );
    }

    /// Der Kontext des Adapters trägt, was das Profil über die Sandbox sagt
    /// (HUM-139).
    ///
    /// Ohne diese Angaben löste die Vorprüfung ein nacktes Kommando gegen den
    /// `PATH` des Hosts auf und verweigerte den Start eines Agenten, den die
    /// Sandbox erreicht.
    #[test]
    fn the_agent_context_carries_the_view_of_the_profile() {
        let profile = humanitl_sandbox::SandboxProfile::parse(
            concat!(
                "version = 1\n",
                "name = \"test\"\n",
                "[mounts]\n",
                "symlinks = [[\"usr/bin\", \"/bin\"]]\n",
                "extra_rw = [\"/opt/tools\"]\n",
                "[mounts.work]\n",
                "dst = \"/workspace\"\n",
                "[env]\n",
                "PATH = \"/usr/local/bin:/usr/bin:/bin\"\n",
            ),
            Path::new("<test>"),
        )
        .unwrap();

        let ctx = with_sandbox_view(
            humanitl_sandbox::AgentContext::new(
                humanitl_core::SessionId::nil(),
                std::path::PathBuf::from("/home/u/proj"),
                humanitl_config::LlmConfig::default(),
            ),
            &Config::default(),
            &profile,
        );

        assert_eq!(
            ctx.sandbox_path,
            Some(OsString::from("/usr/local/bin:/usr/bin:/bin"))
        );
        assert_eq!(
            ctx.sandbox_view.symlinks,
            vec![(
                std::path::PathBuf::from("/bin"),
                std::path::PathBuf::from("usr/bin")
            )]
        );
        // Schreibbar eingehängt ist eingehängt: `extra_rw` gehört in die Sicht
        // (HUM-139).
        assert!(
            ctx.sandbox_view
                .mounts
                .iter()
                .any(|mount| mount.dst == Path::new("/opt/tools")),
            "extra_rw belongs to what the sandbox sees: {:?}",
            ctx.sandbox_view.mounts
        );
        assert_eq!(ctx.work_dir_sandbox, Path::new("/workspace"));
    }

    /// `sandbox.env` gewinnt über das Profil, und die Vorprüfung prüft gegen
    /// den Pfad, den der Start wirklich setzt (HUM-139).
    #[test]
    fn the_sandbox_env_decides_the_search_path_of_the_precheck() {
        let profile = humanitl_sandbox::SandboxProfile::parse(
            "version = 1\nname = \"test\"\n[env]\nPATH = \"/usr/bin\"\n",
            Path::new("<test>"),
        )
        .unwrap();
        let mut config = Config::default();
        config
            .sandbox
            .env
            .insert("PATH".to_owned(), "/mounted/bin".to_owned());

        let ctx = with_sandbox_view(
            humanitl_sandbox::AgentContext::new(
                humanitl_core::SessionId::nil(),
                std::path::PathBuf::from("/home/u/proj"),
                humanitl_config::LlmConfig::default(),
            ),
            &config,
            &profile,
        );

        assert_eq!(ctx.sandbox_path, Some(OsString::from("/mounted/bin")));
    }

    #[test]
    fn the_exit_code_is_the_one_of_the_command() {
        assert_eq!(exit_code_of(ExitStatus::from_raw(5 << 8)), 5);
        assert_eq!(exit_code_of(ExitStatus::from_raw(0)), 0);
    }

    #[test]
    fn a_signal_becomes_128_plus_its_number() {
        // SIGTERM ist 15, SIGKILL ist 9.
        assert_eq!(exit_code_of(ExitStatus::from_raw(15)), 143);
        assert_eq!(exit_code_of(ExitStatus::from_raw(9)), 137);
    }

    #[test]
    fn the_command_line_wins_over_the_agent_and_the_agent_over_the_shell() {
        let mut config = Config::default();
        config.agent.command = Some(vec!["opencode".to_owned()]);
        let from_adapter = vec![OsString::from("opencode")];

        assert_eq!(
            agent_command(
                &config,
                &[OsString::from("sh"), OsString::from("-c")],
                &from_adapter
            ),
            ["sh", "-c"],
            "an explicit command wins over everything"
        );
        assert_eq!(
            agent_command(&config, &[], &from_adapter),
            ["opencode"],
            "without one the adapter decides; it knows agent.command itself"
        );

        config.agent.command = None;
        assert_eq!(
            agent_command(&config, &[], &from_adapter),
            ["opencode"],
            "the adapter also brings its own default command"
        );
        assert_eq!(
            agent_command(&config, &[], &[]),
            ["/bin/sh"],
            "only without any adapter is the shell the answer"
        );
        assert_eq!(CHECK_COMMAND[0], "/bin/sh");
    }

    #[test]
    fn a_placeholder_binds_a_socket_and_two_ca_files() {
        let placeholder = placeholder().expect("the placeholder is created");
        let socket = placeholder.paths.proxy_socket();

        assert!(socket.exists(), "{} is missing", socket.display());
        assert!(placeholder.dir.path().join("ca.crt").is_file());
        assert!(placeholder.dir.path().join("work").is_dir());
        assert!(placeholder.dir.path().join("ca-bundle.crt").is_file());
    }

    /// `sandbox run` keeps no layer, so the default `auto` is direct here;
    /// only `--disposable` or an explicit `sandbox.mode = "disposable"` make
    /// it a throwaway run, and `--direct` wins over the configuration
    /// (HUM-357).
    #[test]
    fn sandbox_run_is_disposable_only_when_asked() {
        use humanitl_config::SandboxMode;
        assert!(!throwaway_wanted(false, false, SandboxMode::Auto));
        assert!(!throwaway_wanted(false, false, SandboxMode::Direct));
        assert!(throwaway_wanted(false, false, SandboxMode::Disposable));
        assert!(throwaway_wanted(true, false, SandboxMode::Direct));
        assert!(!throwaway_wanted(false, true, SandboxMode::Disposable));
    }
}
