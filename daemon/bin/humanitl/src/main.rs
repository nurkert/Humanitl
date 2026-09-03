//! Kommandozeile: dünner gRPC-Client des Dienstes. Nur Verdrahtung, keine Fachlogik.
//!
//! `humanitl` ist erstklassig (ADR-013): Alles, was die Oberfläche kann, geht
//! auch von hier, und die Escape-Tests und das Demo-Skript nehmen denselben
//! Weg wie später der Nutzer. Was der Daemon kann, ruft die Kommandozeile über
//! gRPC ab; sie entscheidet nichts selbst (ADR-018). Die einzige Ausnahme sind
//! die drei `sandbox`-Kommandos, solange es die `Sandbox`-RPC nicht gibt; der
//! Grund steht in [`cmd::sandbox`] und in `backlog/CONVENTIONS.md` 4.12.
//!
//! Zwei Ausgabewege, immer beide bedient: `stdout` trägt das Ergebnis,
//! `stderr` trägt die Befunde. Mit `--json` wandert der Befund als eine Zeile
//! JSON nach `stdout`, damit ein Skript ihn ohne Textsuche lesen kann. Der
//! Exit-Code folgt `backlog/CONVENTIONS.md` 3.8; die Zuordnung steht in
//! [`cmd::exit_code`], und `humanitl --help` schreibt sie hin.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod cli;
mod cmd;
mod render;

use std::process::ExitCode;

use clap::error::ErrorKind;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::cli::{Cmd, Invocation};
use crate::cmd::{Context, EXIT_OK, EXIT_USER, Failure, not_yet_failure};
use crate::render::Renderer;

#[tokio::main]
async fn main() -> ExitCode {
    let invocation = match cli::parse(std::env::args_os()) {
        Ok(invocation) => invocation,
        Err(error) => return report_usage(&error),
    };
    ExitCode::from(dispatch(invocation).await)
}

/// Meldet, was `clap` beim Lesen der Kommandozeile gefunden hat.
///
/// `--help` und `--version` sind kein Fehler: sie gehen nach `stdout` und
/// enden mit 0. Alles andere ist ein Fehler des Aufrufers und wird ein
/// Diagnostic wie jeder andere Fehler der CLI, mit `--json` als eine Zeile auf
/// stdout; Exit 1, nicht die 2 von `clap`: die 2 gehört hier dem nicht
/// erreichbaren Daemon (`backlog/CONVENTIONS.md` 3.8). Ob `--json` gesetzt war,
/// muss aus den rohen Argumenten kommen, weil das Parsen gerade gescheitert ist.
fn report_usage(error: &clap::Error) -> ExitCode {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        let _ = error.print();
        return ExitCode::from(EXIT_OK);
    }
    let json = std::env::args_os().any(|arg| arg == "--json");
    Renderer::new(json, 0, false).diagnostic(&usage_diagnostic(error));
    ExitCode::from(EXIT_USER)
}

/// Der Parse-Fehler von `clap` als Diagnostic: die erste nicht leere Zeile
/// der Meldung als Grund, `humanitl --help` als Abhilfe.
fn usage_diagnostic(error: &clap::Error) -> Diagnostic {
    let why = error
        .to_string()
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(
            || "the command line could not be read".to_owned(),
            |line| line.trim_start_matches("error: ").to_owned(),
        );
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why(why)
        .fix(FixAction::CopyCommand("humanitl --help".to_owned()))
        .build()
}

/// Führt das Unterkommando aus und macht aus seinem Ergebnis einen Exit-Code.
async fn dispatch(invocation: Invocation) -> u8 {
    let global = invocation.cli.global;
    let render = Renderer::new(global.json, global.verbose, global.quiet);
    let ctx = Context::new(invocation.config, global.config, render);

    match run(&ctx, invocation.cli.cmd).await {
        Ok(code) => code,
        Err(failure) => {
            ctx.render.diagnostic(&failure.diagnostic);
            failure.exit
        }
    }
}

/// Das Unterkommando selbst.
async fn run(ctx: &Context, command: Cmd) -> Result<u8, Failure> {
    match command {
        Cmd::Sandbox { cmd } => cmd::sandbox::run(ctx, &cmd).await,
        Cmd::Daemon { cmd } => cmd::daemon::run(ctx, &cmd).await,
        Cmd::Flows { cmd } => cmd::flows::run(ctx, &cmd).await,
        Cmd::Rules { cmd } => cmd::rules::run(ctx, &cmd).await,
        Cmd::Config { cmd } => cmd::config::run(ctx, &cmd),
        // Ein Platzhalter ist ein Fehlschlag wie jeder andere und geht
        // deshalb denselben Weg: [`Renderer::diagnostic`] macht daraus mit
        // `--json` eine Zeile JSON auf `stdout` und sonst den Block auf
        // `stderr`.
        Cmd::Run(_) => Err(not_yet_failure("humanitl run", "HUM-067")),
        Cmd::Audit(_) => Err(not_yet_failure("humanitl audit", "HUM-070")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use crate::cmd::{EXIT_USER, not_yet, not_yet_failure};

    #[test]
    fn a_missing_subcommand_names_its_issue() {
        assert_eq!(
            not_yet("humanitl run", "HUM-067"),
            "humanitl run arrives in HUM-067"
        );
    }

    #[test]
    fn a_missing_subcommand_ends_with_one_and_a_diagnostic() {
        let failure = not_yet_failure("humanitl audit", "HUM-070");
        assert_eq!(failure.exit, EXIT_USER);
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_003");
    }
}
