//! `humanitl go`: eine Wegwerf-Sitzung mit einem Befehl, bis zum Prompt des
//! Agenten (HUM-496).
//!
//! `go` ist `humanitl run --disposable` mit drei Unterschieden, und nur
//! diese stehen hier; Auflösen, Start, Anschluss und Signale teilt es mit
//! `run` ([`crate::cmd::run::launch`]):
//!
//! 1. **Das Terminal geht an den Agenten**, sobald die Sitzung läuft, auch
//!    ohne eigenes Kommando, also mit dem Agenten der Konfiguration. Man
//!    landet in seinem Terminal wie nach einem nativen Start. Ohne Terminal
//!    auf `stdin` oder mit `--json` liest `go` nur mit, wie `run`.
//! 2. **Die Ladeanimation** ([`crate::splash`]) steht, solange die Sitzung
//!    startet, mindestens eine Sekunde und nicht länger als der Start.
//! 3. **Am Ende die Frage**, was mit den wartenden Änderungen geschieht
//!    ([`crate::cmd::changes::review`]): übernehmen, verwerfen oder warten
//!    lassen. Nur auf einem Terminal und nur, wenn der Daemon nach dem Lauf
//!    Änderungen gemeldet hat; nach dem Abhängen läuft die Sitzung weiter,
//!    und es gibt noch nichts zu fragen.
//!
//! Der Befehl entscheidet nichts selbst (ADR-018): Start, Diff, Übernehmen
//! und Verwerfen sind die `Sandbox`-RPC wie bei `run` und `sandbox apply`.

use std::io::IsTerminal as _;

use humanitl_config::{AskMode, Config};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::cli::{GoArgs, RunArgs};
use crate::cmd::run::{Launch, launch};
use crate::cmd::{Context, EXIT_USER, Failure};
use crate::splash::{self, Palette};

/// Führt `humanitl go` aus.
///
/// # Errors
///
/// Wie `humanitl run --disposable`.
pub async fn run(ctx: &Context, args: &GoArgs) -> Result<u8, Failure> {
    let run_args = RunArgs {
        cmd: args.cmd.clone(),
        disposable: true,
        direct: false,
        yes: args.yes,
    };
    let mut state = Launch {
        go: true,
        ..Launch::default()
    };
    let code = launch(ctx, &run_args, &mut state).await?;
    if state.changes_waiting && asks(ctx.render.is_json()) {
        // Der Exit-Code bleibt der des Agenten: Eine Übernahme, die der
        // Daemon ablehnt, steht als Befund da, und die Änderungen warten
        // weiter.
        match crate::cmd::changes::review(ctx).await {
            Ok(None) => {}
            // Ein Signal an der Frage: nichts angefasst, Exit wie bei jedem
            // anderen Signal `128 + n`.
            Ok(Some(number)) => return Ok(128_u8.saturating_add(number)),
            Err(found) => ctx.render.diagnostic(&found.diagnostic),
        }
    }
    Ok(code)
}

/// Ob nach dem Lauf gefragt wird: Tasten von `stdin`, die Frage auf
/// `stderr`, und keine JSON-Ausgabe.
fn asks(json: bool) -> bool {
    !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Die Palette der Ladeanimation, oder `None`, wenn sie nicht erscheint.
pub(crate) fn splash_palette(config: &Config, json: bool) -> Option<Palette> {
    splash::wanted(
        config.terminal.splash,
        json,
        std::io::stdout().is_terminal() && std::io::stderr().is_terminal(),
    )
    .then(|| splash::palette(std::env::var_os("NO_COLOR").as_deref()))
}

/// `humanitl go` mit `--ask terminal`: `CLI_002`, bevor irgendetwas startet.
///
/// In `go` gehört dieses Terminal dem Agenten. Der Kasten von
/// `--ask terminal` bräuchte dieselbe Tastatur, und `go` startete dann den
/// Agenten, ohne dass man in seinem Terminal landete. Gleich, ob der Modus vom
/// Schalter, aus der Umgebung oder aus der Konfiguration kommt.
///
/// # Errors
///
/// `CLI_002`, wenn `hold.ask_mode` für diesen Aufruf `terminal` ist.
pub(crate) fn refuse_ask_terminal(config: &Config) -> Result<(), Failure> {
    if config.hold.ask_mode != AskMode::Terminal {
        return Ok(());
    }
    Err(Failure::with_exit(
        Diagnostic::builder(codes::CLI_002, Severity::Error)
            .why(
                "humanitl go hands this terminal to the agent, and --ask terminal (hold.ask_mode \
                 = \"terminal\") would need the same terminal for its questions. Decide in the \
                 app with --ask ui, or use humanitl run --ask terminal to answer here without \
                 the agent's terminal.",
            )
            .fix(FixAction::CopyCommand("humanitl go --ask ui".to_owned()))
            .build(),
        EXIT_USER,
    ))
}
