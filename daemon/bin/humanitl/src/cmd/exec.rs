//! `humanitl sandbox exec`: eine weitere Shell in der laufenden Sandbox
//! (HUM-299).
//!
//! Die Hälfte der `Exec`-RPC auf der Kommandozeile (ADR-018). Der Befehl ist
//! ein dünner Client: Er bittet den Daemon um die Shell, nennt ihre Kennung
//! und hängt sich danach an ihr Terminal wie `humanitl sandbox attach`
//! (Rohmodus, Größe, Taste zum Abhängen aus HUM-279). Was in der Shell
//! erlaubt ist, entscheidet nicht dieser Befehl, sondern die Sandbox: Der Shim
//! startet die Shell dort mit denselben Grenzen wie den Agenten.
//!
//! Ohne Terminal auf `stdin` sieht der Befehl nur zu, wie `humanitl run` vor
//! HUM-280: Er schreibt die Ausgabe der Shell, bis sie endet, und endet mit
//! ihrem Exit-Code. Abhängen lässt die Shell weiterlaufen;
//! `humanitl sandbox attach --shell ID` hängt sich wieder an.

use std::ffi::OsString;
use std::io::IsTerminal as _;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_ipc::v1;

use crate::cmd::attach::{attach_to, finish_with, resolve_detach_keys, shell_detached};
use crate::cmd::{Context, Failure, status_diagnostic};
use crate::tty::window_size;

/// Führt `humanitl sandbox exec` aus.
///
/// # Errors
///
/// `CLI_004` für ein unlesbares `--detach-keys` oder ein Argument, das kein
/// UTF-8 ist; `DAEMON_001` ohne Daemon; `IPC_006`, wenn keine Sitzung läuft;
/// `TERM_002`, wenn die Sandbox keine weitere Shell öffnet (Grenze erreicht,
/// beendet, abgelehnt).
pub async fn run(
    ctx: &Context,
    cmd: &[OsString],
    detach_keys: Option<&str>,
) -> Result<u8, Failure> {
    let keys = resolve_detach_keys(ctx, detach_keys)?;
    let command = utf8_command(cmd)?;
    let mut client = ctx.connect().await?;
    let (cols, rows) = window_size();
    let response = client
        .exec(v1::ExecRequest {
            sandbox_id: String::new(),
            command,
            cols,
            rows,
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Exec")))?
        .into_inner();
    ctx.render.note(&format!(
        "Shell {} in sandbox {}",
        response.shell_id, response.sandbox_id
    ));
    // An einem Terminal schreibt dieser Befehl, an einer Pipe sieht er zu:
    // Ohne Terminal gibt es niemanden, der in die Shell tippt, und das Ende
    // einer Pipe hängte ihn sonst sofort wieder ab.
    let read_only = !std::io::stdin().is_terminal();
    let ending = attach_to(ctx, read_only, keys, &response.shell_id).await?;
    finish_with(ctx, ending, &shell_detached(&response.shell_id))
}

/// Das Kommando als Text: Der Vertrag trägt `string`, und ein Argument, das
/// kein UTF-8 ist, würde unterwegs verfälscht.
fn utf8_command(cmd: &[OsString]) -> Result<Vec<String>, Failure> {
    cmd.iter()
        .map(|part| {
            part.to_str().map(str::to_owned).ok_or_else(|| {
                Failure::new(
                    Diagnostic::builder(codes::CLI_004, Severity::Error)
                        .why(format!(
                            "the argument {} is not valid UTF-8; `humanitl sandbox exec` passes \
                             text only",
                            part.to_string_lossy()
                        ))
                        .build(),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::ffi::OsStringExt as _;

    use super::*;

    #[test]
    fn a_command_passes_as_text_and_bytes_that_are_not_text_are_refused() {
        let plain = [OsString::from("ls"), OsString::from("-la")];
        assert_eq!(utf8_command(&plain).unwrap(), ["ls", "-la"]);
        assert!(utf8_command(&[]).unwrap().is_empty());
        let broken = [OsString::from_vec(vec![0x66, 0xff])];
        let Err(failure) = utf8_command(&broken) else {
            panic!("bytes that are not UTF-8 pass");
        };
        assert!(format!("{failure:?}").contains("CLI_004"), "{failure:?}");
    }
}
