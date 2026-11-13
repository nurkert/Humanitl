//! `humanitl sandbox image pull` (HUM-439).
//!
//! Die Kommandozeile ist ein dünner Client (ADR-018): Ziehen und Prüfen des
//! Digests macht der Daemon über `SandboxRequest.image_pull`. Hier steht nur,
//! was ein Mensch vorher wissen muss: Das Ziehen nutzt das Netz direkt, nicht
//! durch den Proxy. Der Satz geht immer nach stderr, auch mit `--yes`; ohne
//! `--yes` fragt der Befehl danach.

use std::io::{BufRead as _, IsTerminal as _, Write as _};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};

/// Die erste Minor des Vertrags, die `image_pull` kennt.
pub const IMAGE_MINOR: u32 = 36;

/// Was vor jedem Ziehen dasteht.
pub const NETWORK_NOTE: &str = "Pulling downloads the image from the registry its name \
     names, directly over the network of the daemon: not through the Humanitl proxy, without \
     its rules and its record, and without the proxy variables of this shell. Only the digest \
     decides which image is accepted.";

/// `sandbox image pull`: das Abbild ziehen, nachdem der Mensch weiß, dass das
/// Netz dabei nicht durch den Proxy geht.
///
/// # Errors
///
/// `DAEMON_001` ohne Daemon, `CLI_004` für einen Daemon vor Vertrag 1.36 oder
/// ohne Bestätigung, und die Befunde des Daemons: `CONFIG_003`,
/// `SANDBOX_034`, `SANDBOX_043`, `SANDBOX_048`.
pub async fn pull(ctx: &Context, reference: Option<&str>, yes: bool) -> Result<u8, Failure> {
    let shown = reference.map_or_else(|| "the configured image".to_owned(), str::to_owned);
    eprintln!("{NETWORK_NOTE}");
    if !yes {
        confirm(ctx, &format!("Pull {shown}?"))?;
    }
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;
    if info.proto_minor < IMAGE_MINOR {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{}, and pulling an image needs 1.{IMAGE_MINOR}",
                    info.proto_minor
                ))
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        ));
    }
    let request = v1::sandbox_request::ImagePull {
        reference: reference.unwrap_or_default().to_owned(),
    };
    let mut stream = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::ImagePull(request)),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
        .into_inner();
    let mut pulled = None;
    while let Some(event) = stream
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
    {
        match event.event {
            Some(v1::sandbox_event::Event::ImagePulled(image)) => pulled = Some(image),
            Some(v1::sandbox_event::Event::Diagnostic(wire)) => {
                if let Some(diagnostic) = from_proto(&wire) {
                    if matches!(diagnostic.severity, Severity::Info | Severity::Warning) {
                        ctx.render.diagnostic(&diagnostic);
                    } else {
                        return Err(Failure::new(diagnostic));
                    }
                }
            }
            _ => {}
        }
    }
    let image = pulled.ok_or_else(|| {
        Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why("the daemon answered without the pulled image")
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        )
    })?;
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "reference": image.reference,
            "digest": image.digest,
            "image_id": image.image_id,
        }));
    } else {
        println!("pulled {}", image.reference);
        println!("  digest  {}", image.digest);
        println!("  id      {}", image.image_id);
    }
    Ok(EXIT_OK)
}

/// Fragt auf dem Terminal nach; ohne Terminal ist `--yes` nötig.
fn confirm(ctx: &Context, question: &str) -> Result<(), Failure> {
    let stdin = std::io::stdin();
    if ctx.render.is_json() || !stdin.is_terminal() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(
                    "pulling uses the network directly, not through the proxy, so it asks \
                     first, and there is no terminal to ask on",
                )
                .fix(FixAction::CopyCommand(
                    "humanitl sandbox image pull --yes".to_owned(),
                ))
                .build(),
        ));
    }
    let mut stderr = std::io::stderr();
    let _ = write!(stderr, "{question} [y/N] ");
    let _ = stderr.flush();
    let mut answer = String::new();
    let _ = stdin.lock().read_line(&mut answer);
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Info)
                .why("nothing was pulled; the answer was not yes")
                .fix(FixAction::CopyCommand(
                    "humanitl sandbox image pull".to_owned(),
                ))
                .build(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Satz vor dem Ziehen sagt, dass das Netz nicht durch den Proxy geht
    /// (HUM-439, Spezifikation 2).
    #[test]
    fn the_note_says_the_pull_bypasses_the_proxy() {
        assert!(NETWORK_NOTE.contains("directly"));
        assert!(NETWORK_NOTE.contains("not through the Humanitl proxy"));
    }
}
