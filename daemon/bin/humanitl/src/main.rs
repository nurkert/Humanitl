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

use crate::cli::{Cmd, Invocation};
use crate::cmd::{Context, EXIT_OK, EXIT_USER, Failure, not_yet};
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
/// enden mit 0. Alles andere ist ein Fehler des Aufrufers und endet mit 1,
/// nicht mit der 2 von `clap`: die 2 gehört hier dem nicht erreichbaren
/// Daemon (`backlog/CONVENTIONS.md` 3.8).
fn report_usage(error: &clap::Error) -> ExitCode {
    let _ = error.print();
    match error.kind() {
        // Wer nach der Hilfe fragt, bekommt sie und keinen Fehler.
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::from(EXIT_OK),
        // Ein Aufruf ohne Unterkommando zeigt die Hilfe und hat trotzdem
        // nichts getan.
        _ => ExitCode::from(EXIT_USER),
    }
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
        Cmd::Config { cmd } => cmd::config::run(ctx, &cmd),
        Cmd::Run(_) => Ok(unimplemented("humanitl run", "HUM-067")),
        Cmd::Rules(_) => Ok(unimplemented("humanitl rules", "HUM-065")),
        Cmd::Audit(_) => Ok(unimplemented("humanitl audit", "HUM-070")),
    }
}

/// Ein Unterkommando, das der Vertrag kennt und dieses Binary noch nicht.
///
/// Kein [`humanitl_core::Diagnostic`]: wie im Daemon ist das kein Fehlschlag
/// der Anfrage, sondern der Stand der Umsetzung. Die Meldung nennt das Issue,
/// damit klar ist, worauf man wartet. Der Exit-Code ist trotzdem 1, denn
/// getan hat der Aufruf nichts.
fn unimplemented(what: &str, arrives: &str) -> u8 {
    eprintln!("humanitl: {}", not_yet(what, arrives));
    EXIT_USER
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use crate::cmd::not_yet;

    #[test]
    fn a_missing_subcommand_names_its_issue() {
        assert_eq!(
            not_yet("humanitl run", "HUM-067"),
            "humanitl run arrives in HUM-067"
        );
    }
}
