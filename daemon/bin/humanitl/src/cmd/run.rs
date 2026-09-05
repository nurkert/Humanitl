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
//! Er hängt kein PTY an. Der Agent bekommt kein Terminal, seine Ausgabe kommt
//! als Bytes über den Ereignisstrom und geht unverändert auf `stdout` und
//! `stderr` dieses Prozesses; gefiltert wird sie im Daemon
//! ([`humanitl_core::TerminalFilter`]). Damit gibt es keinen Raw-Modus, keine
//! Weiterleitung der Fenstergröße, keine Eingabe an den Agenten und kein
//! `Ctrl+]`-Menü. Das alles ist HUM-042, und bis dahin verweigert
//! `--ask terminal` den Dienst mit `CLI_002` — für jeden Agenten, nicht nur
//! für die Vollbild-TUIs, für die es ohnehin gilt.
//!
//! `Ctrl+C` beendet die Sitzung über `Sandbox(Stop)`; ohne Eingabekanal
//! erreicht kein Byte den Agenten.

use std::ffi::OsString;

use humanitl_config::AskMode;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::session::{SESSION_OVERRIDE_KEYS, ask_mode_name};
use humanitl_ipc::v1;
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

    refuse_terminal_ask(config.hold.ask_mode)?;

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
    let code = drive(ctx, &mut client, start).await?;
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
) -> Result<u8, Failure> {
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

    // Ein zweiter Client für den Stopp: Der erste hält den Ereignisstrom, und
    // ein `&mut` daran wäre für die Dauer der Schleife geliehen.
    let mut stopper = client.clone();
    while exit.is_none() {
        let next = tokio::select! {
            event = events.message() => event,
            // `Ctrl+C` beendet die Sitzung. Ein Byte an den Agenten gibt es
            // nicht — dafür bräuchte es das PTY aus HUM-042 —, also ist das
            // Ende der Sitzung die ehrliche Antwort auf das Signal.
            signal = tokio::signal::ctrl_c(), if !interrupted => {
                if signal.is_ok() {
                    interrupted = true;
                    ctx.render.note("[humanitl] stopping the session");
                    stop(&mut stopper).await;
                }
                continue;
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
        if let Some(found) = handle(ctx, event) {
            failure.get_or_insert(found);
        }
    }

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

/// `--ask terminal` gibt es noch nicht.
///
/// Die Spezifikation gibt `CLI_002` für Vollbild-TUI-Agenten; solange es kein
/// PTY gibt (HUM-042), gilt dieselbe Antwort für jeden Agenten. Der
/// Unterschied ist gering: Ohne PTY hätte auch ein zeilenorientierter Agent
/// keinen Prompt, den ein Mensch beantworten könnte.
fn refuse_terminal_ask(ask_mode: AskMode) -> Result<(), Failure> {
    if ask_mode != AskMode::Terminal {
        return Ok(());
    }
    Err(Failure::with_exit(
        Diagnostic::builder(codes::CLI_002, Severity::Error)
            .why(
                "--ask terminal needs a terminal of its own, and this humanitl does not attach \
                 one yet (HUM-042). Use --ask ui and decide in the app, or --ask none and let \
                 every request without a rule be blocked.",
            )
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

    use super::{chain, check_line, inline_rules, refuse_terminal_ask, session_lines, state_name};
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

    #[test]
    fn ask_terminal_is_cli_002_and_names_both_ways_out() {
        let failure = refuse_terminal_ask(AskMode::Terminal).expect_err("no terminal yet");
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
            assert!(refuse_terminal_ask(mode).is_ok(), "{mode:?}");
        }
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
