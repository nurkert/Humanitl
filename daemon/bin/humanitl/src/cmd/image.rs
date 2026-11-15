//! `humanitl sandbox image pull` (HUM-439).
//!
//! Die Kommandozeile ist ein dünner Client (ADR-018): Ziehen und Prüfen des
//! Digests macht der Daemon über `SandboxRequest.image_pull`. Hier steht nur,
//! was ein Mensch vorher wissen muss: Das Ziehen nutzt das Netz direkt, nicht
//! durch den Proxy. Der Satz geht immer nach stderr, auch mit `--yes`; ohne
//! `--yes` fragt der Befehl danach.
//!
//! `Ctrl+C` während des Ziehens bricht es ab (HUM-457): Der Befehl wählt die
//! Kennung seines Ziehens selbst, schickt bei `SIGINT` den Abbruch
//! `SandboxRequest.cancel_image_pull` mit genau dieser Kennung, wartet auf
//! die Antwort des Daemons und endet mit 130 wie `humanitl run`. Hat der
//! Daemon nichts mehr unterbrochen, weil das Ziehen gerade von selbst fertig
//! wurde, gilt dessen Ergebnis. Beendet wird nur die Prozessgruppe des
//! Ziehens; was sich daraus mit `setsid` löst, bleibt.

use std::io::{BufRead as _, IsTerminal as _, Write as _};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};

/// Die erste Minor des Vertrags, die `image_pull` kennt.
pub const IMAGE_MINOR: u32 = 36;

/// Die erste Minor, die ein Ziehen abbrechen kann (HUM-457). Ein älterer
/// Daemon zieht trotzdem; nur `Ctrl+C` hält ihn dann nicht auf.
pub const CANCEL_MINOR: u32 = 37;

/// Die erste Minor, die mit dem Backend der Sitzung zieht
/// (`ImagePull.plan`, HUM-500). Ein älterer Daemon zöge mit seiner
/// Einstellung; mit `--backend` oder `HUMANITL_SANDBOX__BACKEND` lehnt der
/// Befehl ihn deshalb ab.
pub const PLAN_MINOR: u32 = 40;

/// Der Exit-Code nach `Ctrl+C`: `128 + SIGINT`, wie `humanitl run`.
const EXIT_INTERRUPTED: u8 = 130;

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
    // Das Backend wie bei `humanitl run` (HUM-500): `--backend` vor der
    // Umgebung, sonst entscheidet der Daemon mit seiner Konfiguration.
    let resolved = ctx.config()?;
    let backend = crate::cmd::run::session_backend(ctx.cli_pairs(), &resolved);
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
    check_minor(info.proto_minor, backend.as_deref())?;
    // **Erst der Empfänger, dann das Ziehen.** Ein Strom, der vor dem
    // Senden steht, puffert ein `SIGINT`, das kommt, bevor `select!` zum
    // ersten Mal wartet; ohne ihn beendete der Kernel den Befehl, und das
    // Ziehen liefe im Daemon weiter.
    let mut interrupt =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).ok();
    let cancellable = info.proto_minor >= CANCEL_MINOR;
    let pull_id = if cancellable {
        uuid::Uuid::now_v7().to_string()
    } else {
        String::new()
    };
    let request = pull_request(reference, pull_id.clone(), backend);
    // Der Abbruch geht über eine zweite Anfrage auf demselben Kanal.
    let mut canceller = client.clone();
    let mut stream = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::ImagePull(request)),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
        .into_inner();
    let mut pulled = None;
    loop {
        let event = tokio::select! {
            message = stream.message() => message
                .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?,
            () = next_interrupt(interrupt.as_mut()) => {
                match cancel(ctx, &mut canceller, cancellable.then_some(pull_id.as_str())).await? {
                    Some(code) => return Ok(code),
                    // Nichts abgebrochen: Das Ergebnis des Ziehens folgt auf
                    // seinem eigenen Strom und gilt.
                    None => continue,
                }
            }
        };
        let Some(event) = event else { break };
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

/// Ob der Daemon mit Vertrag 1.`minor` dieses Ziehen versteht: überhaupt
/// (1.36) und, mit einem gewählten `backend`, mit dem Backend der Sitzung
/// (1.40, HUM-500).
///
/// # Errors
///
/// `CLI_004` für einen Daemon, der zu alt ist.
fn check_minor(minor: u32, backend: Option<&str>) -> Result<(), Failure> {
    if minor < IMAGE_MINOR {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{minor}, and pulling an image needs 1.{IMAGE_MINOR}"
                ))
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        ));
    }
    if let Some(backend) = backend
        && minor < PLAN_MINOR
    {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{minor}, and pulling for the backend \
                     {backend:?} needs 1.{PLAN_MINOR}; this daemon would pull with its own setting"
                ))
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        ));
    }
    Ok(())
}

/// Die Anfrage des Ziehens: mit dem Plan des Starts, für den gezogen wird
/// (HUM-500). Der Plan trägt nur das Backend der Sitzung; leer heißt, der
/// Daemon nimmt `sandbox.backend` seiner Konfiguration, misst aber wie beim
/// Start auch das Sandbox-Profil und die Untergrenze.
fn pull_request(
    reference: Option<&str>,
    pull_id: String,
    backend: Option<String>,
) -> v1::sandbox_request::ImagePull {
    v1::sandbox_request::ImagePull {
        reference: reference.unwrap_or_default().to_owned(),
        pull_id,
        plan: Some(v1::sandbox_request::Plan {
            backend: backend.unwrap_or_default(),
            ..v1::sandbox_request::Plan::default()
        }),
    }
}

/// Das nächste `SIGINT`. Ohne Handler wartet der Zweig für immer; dann gilt
/// das Standardverhalten des Signals, wie ohne diesen Befehl.
async fn next_interrupt(interrupt: Option<&mut tokio::signal::unix::Signal>) {
    match interrupt {
        Some(stream) => {
            stream.recv().await;
        }
        None => std::future::pending::<()>().await,
    }
}

/// Bricht das Ziehen mit der Kennung `pull_id` ab, nach `Ctrl+C` (HUM-457).
///
/// Wartet auf die Antwort des Daemons; sie kommt, wenn das Ziehen
/// zurückgekehrt ist. `Some(130)`: Der Daemon hat die Engine unterbrochen;
/// liefen danach noch Prozesse des Ziehens, sagt eine Warnung wie viele.
/// `None`: Unterbrochen wurde nichts, weil das Ziehen schon von selbst zu
/// Ende war; dann gilt sein eigenes Ergebnis, und der Aufrufer liest es
/// weiter. Ohne Kennung (ein Daemon vor 1.37) sagt der Befehl, dass der
/// Daemon weiterzieht, statt einen Abbruch zu behaupten.
///
/// # Errors
///
/// Die Befunde des Daemons auf den Abbruch, oder `DAEMON_001`, wenn die
/// Anfrage ihn nicht erreicht.
async fn cancel(
    ctx: &Context,
    client: &mut humanitl_ipc::client::Client,
    pull_id: Option<&str>,
) -> Result<Option<u8>, Failure> {
    eprintln!();
    let Some(pull_id) = pull_id else {
        ctx.render.diagnostic(
            &Diagnostic::builder(codes::CLI_004, Severity::Warning)
                .why(
                    "this daemon cannot cancel a pull, so it keeps pulling after this command \
                     ended",
                )
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        );
        return Ok(Some(EXIT_INTERRUPTED));
    };
    let mut answer = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::CancelImagePull(
                v1::sandbox_request::CancelImagePull {
                    pull_id: pull_id.to_owned(),
                },
            )),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
        .into_inner();
    while let Some(event) = answer
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
    {
        match event.event {
            Some(v1::sandbox_event::Event::ImagePullCancelled(answer)) => {
                if !answer.cancelled {
                    ctx.render
                        .note("the pull had already ended on its own; its result follows");
                    return Ok(None);
                }
                if answer.still_running == 0 {
                    ctx.render
                        .note("the pull was cancelled; no process of it runs any more");
                } else {
                    ctx.render.diagnostic(
                        &Diagnostic::builder(codes::SANDBOX_043, Severity::Warning)
                            .why(format!(
                                "the pull was cancelled, but {} process(es) of it still ran \
                                 after the container engine was killed",
                                answer.still_running
                            ))
                            .build(),
                    );
                }
                return Ok(Some(EXIT_INTERRUPTED));
            }
            Some(v1::sandbox_event::Event::Diagnostic(wire)) => {
                if let Some(diagnostic) = from_proto(&wire) {
                    return Err(Failure::new(diagnostic));
                }
            }
            _ => {}
        }
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why("the daemon answered the cancel of the pull without saying whether it ended")
            .fix(FixAction::CopyCommand(
                humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
            ))
            .build(),
    ))
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
    #![allow(clippy::expect_used)]

    use super::*;

    /// Der Satz vor dem Ziehen sagt, dass das Netz nicht durch den Proxy geht
    /// (HUM-439, Spezifikation 2).
    #[test]
    fn the_note_says_the_pull_bypasses_the_proxy() {
        assert!(NETWORK_NOTE.contains("directly"));
        assert!(NETWORK_NOTE.contains("not through the Humanitl proxy"));
    }

    /// Das Ziehen schickt das Backend, das `humanitl run` mit denselben
    /// Flags und derselben Umgebung als `sandbox.backend` in
    /// `cli_overrides` schickte (HUM-500): `--backend` am Befehl selbst, die
    /// Variable `HUMANITL_SANDBOX__BACKEND`, ein Flag vor der Variable. Ohne
    /// beides geht ein Plan ohne Wahl mit, und der Daemon nimmt seine
    /// Einstellung.
    ///
    /// Mutation proof: `pull_request` ohne `backend` im Plan lässt die erste
    /// Zusicherung scheitern.
    #[test]
    fn the_pull_sends_the_backend_a_start_would_send() {
        use humanitl_config::{Env, ProfileSelection, resolve};

        let home = tempfile::tempdir().expect("tempdir");
        let resolved = |extra: &[(&str, &str)]| {
            let mut pairs = vec![
                ("HOME".to_owned(), home.path().display().to_string()),
                (
                    "XDG_CONFIG_HOME".to_owned(),
                    home.path().join("cfg").display().to_string(),
                ),
            ];
            pairs.extend(
                extra
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
            );
            resolve(&ProfileSelection::any(), None, &Env::from_pairs(pairs), &[])
                .expect("the client resolves")
        };
        let flags = |args: &[&str]| {
            let matches = crate::cli::command()
                .try_get_matches_from(
                    ["humanitl", "sandbox", "image", "pull", "--yes"]
                        .iter()
                        .chain(args),
                )
                .expect("the command line parses");
            crate::cli::config_pairs(&matches)
        };
        let sent = |args: &[&str], env: &[(&str, &str)]| {
            let backend = crate::cmd::run::session_backend(flags(args), &resolved(env));
            pull_request(None, String::new(), backend)
                .plan
                .expect("the pull carries a plan")
                .backend
        };
        // Ein Name des Schemas, den jeder Build kennt; welcher, ist hier gleich.
        let chosen = humanitl_sandbox::PLATFORM_BACKEND_NAME;

        assert_eq!(sent(&["--backend", chosen], &[]), chosen);
        assert_eq!(sent(&[], &[("HUMANITL_SANDBOX__BACKEND", chosen)]), chosen);
        assert_eq!(
            sent(
                &["--backend", "auto"],
                &[("HUMANITL_SANDBOX__BACKEND", chosen)]
            ),
            "auto"
        );
        assert_eq!(sent(&[], &[]), "");
        // Ein Daemon vor 1.40 zöge mit seiner Einstellung: mit einer Wahl
        // abgelehnt, ohne Wahl zieht er wie bisher.
        let refused = check_minor(PLAN_MINOR - 1, Some(chosen)).expect_err("an older daemon");
        assert_eq!(refused.diagnostic.code, codes::CLI_004);
        assert!(check_minor(PLAN_MINOR - 1, None).is_ok());
        assert!(check_minor(PLAN_MINOR, Some(chosen)).is_ok());
        let request = pull_request(Some("r@sha256:x"), "id".to_owned(), None);
        assert_eq!(
            (request.reference.as_str(), request.pull_id.as_str()),
            ("r@sha256:x", "id")
        );
    }
}
