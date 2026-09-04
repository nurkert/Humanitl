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
//! [`BwrapBackend`], derselbe [`LaunchPlan`](humanitl_sandbox::LaunchPlan) wie später im Daemon. Es gibt
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

use humanitl_config::{Config, Env, Paths};
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::{
    AdapterRegistry, AgentContext, BwrapBackend, CheckResult, INTERRUPT_GRACE, LaunchInputs,
    MIN_BWRAP_VERSION, MountPolicy, SANDBOX_SHELL, SandboxBackend, SandboxFile, SandboxHandle,
    SandboxProfile, SessionContext, StdioMode, shell_line,
};
use serde_json::json;
use tempfile::TempDir;

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
        SandboxCmd::Run { cmd, tests_dir } => start(ctx, cmd, tests_dir.as_deref()).await,
        SandboxCmd::Check => check(ctx),
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
    backend: BwrapBackend,
    /// Hält die Platzhalter am Leben, solange die Sandbox läuft.
    _wiring: Wiring,
}

/// `sandbox argv`: die Kommandozeile, ohne zu starten.
fn argv(ctx: &Context, command: &[OsString]) -> Result<u8, Failure> {
    let config = ctx.config()?.config;
    let setup = prepare(ctx, &config, Wiring::Preview, command)?;
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
) -> Result<u8, Failure> {
    let config = ctx.config()?.config;

    // Schritt 1 der Spezifikation: ohne Daemon kein Lauf. Er hält den
    // Proxy-Socket und das CA-Bundle; ohne beides hätte der Agent in der
    // Sandbox kein Netz, und die Sitzung stünde in keiner Aufzeichnung.
    let mut client = ctx.connect().await?;
    client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?;

    let setup = prepare(ctx, &config, Wiring::Daemon, command)?;
    let mut plan = setup
        .backend
        .plan(&setup.profile, &setup.session)
        .map_err(Failure::new)?;
    if let Some(dir) = tests_dir {
        bind_tests_dir(ctx, &mut plan.argv, dir)?;
    }
    ctx.render.detail(&format!("argv: {}", plan.argv_line()));

    let handle = Arc::new(setup.backend.launch(&plan).map_err(Failure::new)?);
    ctx.render
        .detail(&format!("sandbox {} pid {}", handle.id, handle.pid));

    enforce_isolation(ctx, &setup.backend, &handle)?;
    wait_or_interrupt(handle).await
}

/// `sandbox check`: eine kurzlebige Sandbox und die drei Garantien.
fn check(ctx: &Context) -> Result<u8, Failure> {
    let config = ctx.config()?.config;
    let command: Vec<OsString> = CHECK_COMMAND.iter().map(OsString::from).collect();
    let wiring = Wiring::Placeholder(Box::new(placeholder()?));
    let setup = prepare(ctx, &config, wiring, &command)?;

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
    backend: &BwrapBackend,
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

/// Der Platzhalter, den `profiles/sandbox/test.toml` in `mounts.extra_ro` nennt.
const TESTS_DIR_DST: &str = "/tests/escape";

/// Zieht die Quelle des Binds nach [`TESTS_DIR_DST`] auf `dir`.
///
/// # Errors
///
/// `SANDBOX_010`, wenn das Profil den Platzhalter nicht nennt: dann zeigt
/// `--tests-dir` auf nichts, und ein Lauf, der die Skripte nicht sieht,
/// bewiese nichts.
fn bind_tests_dir(ctx: &Context, argv: &mut [OsString], dir: &Path) -> Result<(), Failure> {
    let src = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        ctx.cwd.join(dir)
    };
    if !rebind_source(argv, Path::new(TESTS_DIR_DST), &src) {
        return Err(Failure::new(
            Diagnostic::builder(codes::SANDBOX_010, Severity::Blocking)
                .why(format!(
                    "the sandbox profile does not name {TESTS_DIR_DST} in mounts.extra_ro, so                      --tests-dir {} has nothing to point at",
                    src.display()
                ))
                .fix(FixAction::ChangeSetting {
                    key: "sandbox.profile".to_owned(),
                    value: "test".to_owned(),
                })
                .build(),
        ));
    }
    ctx.render
        .detail(&format!("tests dir {} -> {TESTS_DIR_DST}", src.display()));
    Ok(())
}

/// Setzt die Quelle des ersten Binds nach `dst` auf `src`; `false`, wenn es
/// ihn nicht gibt.
fn rebind_source(args: &mut [OsString], dst: &Path, src: &Path) -> bool {
    let bind = std::ffi::OsStr::new("--bind");
    let ro_bind = std::ffi::OsStr::new("--ro-bind");
    for index in 0..args.len().saturating_sub(2) {
        let flag = args[index].as_os_str();
        if (flag == bind || flag == ro_bind) && Path::new(&args[index + 2]) == dst {
            args[index + 1] = src.as_os_str().to_os_string();
            return true;
        }
    }
    false
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
async fn wait_or_interrupt(handle: Arc<SandboxHandle>) -> Result<u8, Failure> {
    let waiting = Arc::clone(&handle);
    let mut waiter = tokio::task::spawn_blocking(move || waiting.wait());

    tokio::select! {
        joined = &mut waiter => finish(joined),
        signal = tokio::signal::ctrl_c() => {
            if signal.is_err() {
                // Ohne Signalquelle bleibt nur warten; der Lauf endet dann
                // mit dem Code des Befehls wie ohne Unterbrechung.
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
    config: &Config,
    wiring: Wiring,
    command: &[OsString],
) -> Result<Setup, Failure> {
    let paths = wiring.paths(ctx);
    let policy = MountPolicy::from_paths(&paths);
    let profile_path = profile_path(ctx, &config.sandbox.profile)?;
    let profile = SandboxProfile::load_validated(&profile_path, &policy).map_err(Failure::new)?;
    ctx.render
        .detail(&format!("profile {}", profile_path.display()));

    let id = SessionId::new();
    let work_src = wiring.work_dir(ctx, config);
    // Der Beitrag des Agent-Adapters gilt dem Agenten. Steht auf der
    // Kommandozeile ein eigener Befehl (`sandbox run -- CMD`, so laufen die
    // Escape-Tests), läuft nicht der Agent, und der Adapter trägt nichts bei.
    let agent = if command.is_empty() {
        agent_contribution(ctx, config, &wiring, id, &work_src, &profile)?
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
        command: agent_command(config, command, &agent.command),
        files: agent.files,
    };

    let backend = match BwrapBackend::detect(paths.clone()) {
        Ok(backend) => backend,
        // Die Vorschau braucht kein `bwrap`; der Start schon.
        Err(_) if matches!(wiring, Wiring::Preview) => {
            BwrapBackend::unchecked("bwrap", MIN_BWRAP_VERSION, paths)
        }
        Err(diagnostic) => return Err(Failure::new(diagnostic)),
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
        .with_command_override(
            config
                .agent
                .command
                .as_ref()
                .map(|parts| parts.iter().map(OsString::from).collect()),
        )
        .with_host_path(ctx.env.non_empty("PATH").map(OsString::from))
        .with_language(config.ui.language)
        // Frist und Ask-Modus stehen im Briefing (HUM-071); sie müssen die
        // sein, nach denen der Proxy arbeitet, sonst nennt der Text dem Agenten
        // eine Wartezeit, die es nicht gibt.
        .with_hold(config.hold.clone())
        .with_briefing(config.agent.briefing.clone())
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
        .with_config_home(config.sandbox.env.get("XDG_CONFIG_HOME").map(PathBuf::from))
        // Nur-Lese-Einhängungen mit gleicher Quelle und gleichem Ziel: nur
        // darunter findet der Agent sein eigenes Programm wieder.
        .with_sandbox_ro_paths(
            profile
                .mounts
                .ro
                .iter()
                .chain(&profile.mounts.extra_ro)
                .cloned()
                .collect(),
        );

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

    Ok(AgentContribution {
        command: adapter.command(&agent_ctx),
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

    /// Das Verzeichnis, das als `/work` eingehängt wird.
    fn work_dir(&self, ctx: &Context, config: &Config) -> PathBuf {
        if let Self::Placeholder(placeholder) = self {
            // Ein Selbsttest fasst das Projekt des Nutzers nicht an.
            return placeholder.dir.path().join("work");
        }
        config
            .sandbox
            .work_dir
            .clone()
            .unwrap_or_else(|| ctx.cwd.clone())
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
fn profile_path(ctx: &Context, name: &str) -> Result<PathBuf, Failure> {
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

    use std::ffi::OsString;
    use std::os::unix::process::ExitStatusExt as _;
    use std::path::Path;
    use std::process::ExitStatus;

    use humanitl_config::Config;

    use super::{
        CHECK_COMMAND, TESTS_DIR_DST, agent_command, exit_code_of, placeholder, rebind_source,
    };

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
    fn the_tests_directory_moves_the_source_of_that_one_bind() {
        let mut argv: Vec<OsString> = [
            "bwrap",
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind",
            TESTS_DIR_DST,
            TESTS_DIR_DST,
            "--bind",
            "/home/agent/project",
            "/work",
        ]
        .iter()
        .map(OsString::from)
        .collect();

        assert!(rebind_source(
            &mut argv,
            Path::new(TESTS_DIR_DST),
            Path::new("/src/tests/escape"),
        ));
        assert_eq!(argv[5], OsString::from("/src/tests/escape"));
        assert_eq!(argv[6], OsString::from(TESTS_DIR_DST));
        // Kein anderer Bind wurde angefasst.
        assert_eq!(argv[2], OsString::from("/usr"));
        assert_eq!(argv[8], OsString::from("/home/agent/project"));
    }

    #[test]
    fn a_profile_without_the_placeholder_has_nothing_to_rebind() {
        let mut argv: Vec<OsString> = ["bwrap", "--ro-bind", "/usr", "/usr"]
            .iter()
            .map(OsString::from)
            .collect();

        assert!(!rebind_source(
            &mut argv,
            Path::new(TESTS_DIR_DST),
            Path::new("/src/tests/escape"),
        ));
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
}
