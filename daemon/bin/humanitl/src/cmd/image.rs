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

/// Die erste Minor, die im Plan des Ziehens auch die übrigen Werte der
/// Sitzung sieht (`Plan.cli_overrides`, seit HUM-559 wie im Plan), vor allem
/// die Untergrenze und die Laufzeit der Container-Engine (HUM-567). Seit 1.57 lädt
/// der Daemon dazu die gespeicherte Projektwahl des Ordners.
pub const VALUES_MINOR: u32 = 55;

/// Die erste Minor, die beim Ziehen die gespeicherte Projektwahl des Ordners
/// lädt (HUM-567). Davor lehnt der Befehl ab, wenn diese Wahl das Backend
/// mitentscheiden kann ([`refuse_without_choice`]).
pub const CHOICE_MINOR: u32 = 57;

/// Ab dieser Minor kennt der Daemon `Projects` (HUM-532); davor gibt es kein
/// Register und keine gespeicherte Projektwahl.
const PROJECTS_MINOR: u32 = 45;

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
    // Umgebung, sonst entscheidet der Daemon mit seiner Konfiguration. Dazu
    // dieselben Werte der Sitzung wie beim Start (HUM-567), vor allem die
    // Untergrenze: Sie entscheidet mit, welches Backend der Start nimmt.
    let resolved = ctx.config()?;
    let backend = crate::cmd::run::session_backend(ctx.cli_pairs(), &resolved);
    let overrides = crate::cmd::run::client_overrides(ctx, &resolved);
    // Der Ordner, in dem `humanitl run` dieses Aufrufs startete: Sein
    // Projekt-Profil und seine gespeicherte Projektwahl entscheiden das
    // Backend mit (HUM-567).
    let folder = ctx.work_dir(&resolved);
    let shown = reference.map_or_else(|| "the configured image".to_owned(), str::to_owned);
    eprintln!("{NETWORK_NOTE}");
    if !yes {
        confirm(
            ctx,
            &format!("Pull {shown}?"),
            &pull_command(reference, backend.as_deref()),
        )?;
    }
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;
    check_minor(info.proto_minor, &overrides)?;
    // Ein Daemon vor 1.57 zöge ohne die gespeicherte Projektwahl des Ordners
    // (HUM-567). Bestimmt sie das Backend mit, zöge er womöglich für ein
    // anderes als der Start; dann lieber nicht.
    let chosen = saved_backend_choice(&mut client, &folder.0, info.proto_minor).await?;
    refuse_without_choice(info.proto_minor, &chosen)?;
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
    let request = pull_request(
        reference,
        pull_id.clone(),
        sent_folder(
            (&folder.0, &folder.1),
            start_refuses(ctx, &resolved, &folder),
        ),
        overrides,
    );
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
/// (1.36), mit einem gewählten Backend (1.40, HUM-500) und mit den Werten
/// der Sitzung, die das Backend mitentscheiden (1.55, HUM-567:
/// [`backend_values`]).
///
/// # Errors
///
/// `CLI_004` für einen Daemon, der zu alt ist; der Befund nennt die Werte,
/// die er still überginge, und die Minor, die sie brauchen.
fn check_minor(minor: u32, overrides: &[v1::sandbox_request::CliOverride]) -> Result<(), Failure> {
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
    let unseen: Vec<(&v1::sandbox_request::CliOverride, u32)> = overrides
        .iter()
        .filter_map(|pair| backend_values(&pair.path).map(|needed| (pair, needed)))
        .filter(|(_, needed)| minor < *needed)
        .collect();
    if let Some(needed) = unseen.iter().map(|(_, needed)| *needed).max() {
        let named: Vec<String> = unseen
            .iter()
            .map(|(pair, _)| format!("{} = {:?}", pair.path, pair.value))
            .collect();
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{minor}, and pulling with {} needs \
                     1.{needed}; this daemon would pull for another backend than the start \
                     takes",
                    named.join(", ")
                ))
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        ));
    }
    // Den Agenten der Sitzung nimmt ein Daemon erst ab 1.61 an (#8).
    crate::cmd::run::agent_reaches_the_daemon(overrides, minor)
}

/// Ob der Start den Ordner `folder` ablehnte: dieselbe Prüfung wie im
/// Daemon ([`humanitl_ipc::work_dir::check_requested`]).
fn start_refuses(
    ctx: &Context,
    resolved: &humanitl_config::Resolved,
    folder: &(std::path::PathBuf, humanitl_sandbox::WorkDirSource),
) -> bool {
    humanitl_ipc::work_dir::check_requested(
        &ctx.paths,
        resolved.config.sandbox.work_dir.as_deref(),
        &folder.0,
        &folder.1,
    )
    .is_err()
}

/// Der Ordner, den das Ziehen schickt (HUM-567): der Ordner des Starts
/// dieses Aufrufs, mit seinem Projekt-Profil und, ab 1.57, seiner
/// gespeicherten Projektwahl; auch ein älterer Daemon löst mit dem
/// Projekt-Profil auf. Keiner, wenn der Ordner nur das aktuelle Verzeichnis
/// ist und der Start ihn ablehnt (`refused`, etwa das Heimatverzeichnis
/// selbst): Ein Daemon vor 1.57 prüfte ihn wie beim Start und lehnte ein
/// Ziehen ab, das bisher ging; ab 1.57 wählte er für ihn ohnehin kein
/// Projekt.
fn sent_folder<'a>(
    folder: (&'a std::path::Path, &'a humanitl_sandbox::WorkDirSource),
    refused: bool,
) -> Option<(&'a std::path::Path, &'a humanitl_sandbox::WorkDirSource)> {
    let only_cwd = matches!(folder.1, humanitl_sandbox::WorkDirSource::CurrentDir);
    (!(only_cwd && refused)).then_some(folder)
}

/// Die Schlüssel `sandbox.*`, die die gespeicherte Projektwahl des Ordners
/// `folder` setzt, gefragt nur bei einem Daemon mit Register und vor 1.57
/// (HUM-567). Sie können das Backend des Starts mitentscheiden (Backend,
/// Untergrenze, Sandbox-Profil, Laufzeit), und ein solcher Daemon zöge ohne
/// sie.
///
/// # Errors
///
/// Der Befund des Daemons, außer für einen Ordner, den das Register nicht
/// kennt (`CONFIG_003`), oder einen Daemon ohne Register (`IPC_006`): Dann
/// gibt es keine Wahl. Jeder andere Befund hält das Ziehen auf, statt eine
/// Wahl zu übergehen, die es geben könnte.
async fn saved_backend_choice(
    client: &mut humanitl_ipc::client::Client,
    folder: &std::path::Path,
    minor: u32,
) -> Result<Vec<String>, Failure> {
    if !(PROJECTS_MINOR..CHOICE_MINOR).contains(&minor) {
        return Ok(Vec::new());
    }
    let answer = client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                project_id: String::new(),
                path: folder.display().to_string(),
            })),
        })
        .await;
    match answer {
        Ok(shown) => Ok(backend_choice_keys(&shown.into_inner())),
        Err(status) => {
            let found = status_diagnostic(&status, "Projects");
            if found.code == codes::CONFIG_003 || found.code == codes::IPC_006 {
                Ok(Vec::new())
            } else {
                Err(Failure::new(found))
            }
        }
    }
}

/// Die Schlüssel `sandbox.*` mit gewähltem Wert in der Antwort auf
/// `Projects.get`.
fn backend_choice_keys(shown: &v1::ProjectsResponse) -> Vec<String> {
    shown
        .choice
        .iter()
        .filter(|field| field.key.starts_with("sandbox.") && !field.chosen.trim().is_empty())
        .map(|field| format!("{} = {:?}", field.key, field.chosen))
        .collect()
}

/// `CLI_004`, wenn ein Daemon der Minor `minor` ohne die gespeicherte
/// Projektwahl `chosen` zöge (vor 1.57, HUM-567).
///
/// # Errors
///
/// `CLI_004`, der die gewählten Werte und die Minor 1.57 nennt.
fn refuse_without_choice(minor: u32, chosen: &[String]) -> Result<(), Failure> {
    if minor >= CHOICE_MINOR || chosen.is_empty() {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!(
                "the daemon speaks contract 1.{minor}, and the saved project choice of this \
                 folder ({}) needs 1.{CHOICE_MINOR} to reach a pull; this daemon would pull \
                 for another backend than the start takes",
                chosen.join(", ")
            ))
            .fix(FixAction::CopyCommand(
                humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
            ))
            .build(),
    ))
}

/// Die Minor, ab der ein Daemon diesen Wert der Sitzung im Plan des Ziehens
/// sieht, wenn er mitentscheidet, welches Backend zieht; `None` für einen
/// Wert, der das nicht tut (`llm.endpoint`, `hold.timeout_secs`). Ein
/// älterer Daemon überginge ihn still; für das Ziehen zählt das nur beim
/// Backend.
fn backend_values(path: &str) -> Option<u32> {
    match path {
        "sandbox.backend" => Some(PLAN_MINOR),
        // Die Untergrenze und die Laufzeit der Container-Engine: jeder
        // andere Wert der Sitzung unter `sandbox.`.
        other if other.starts_with("sandbox.") => Some(VALUES_MINOR),
        _ => None,
    }
}

/// Die Anfrage des Ziehens: mit dem Plan des Starts, für den gezogen wird
/// (HUM-500). Der Plan trägt die Werte der Sitzung wie ein Start dieses
/// Aufrufs (HUM-567, [`crate::cmd::run::client_overrides`]) in
/// `Plan.cli_overrides` und das Backend darunter zusätzlich in
/// `Plan.backend`, das ein Daemon seit 1.40 kennt; ohne Backend nimmt der
/// Daemon `sandbox.backend` seiner Konfiguration, misst aber wie beim Start
/// auch das Sandbox-Profil, die Untergrenze und die Projektwahl.
///
/// `folder` ist der Ordner eines Starts dieses Aufrufs mit seiner Herkunft,
/// wie ihn `humanitl sandbox plan` schickt: Der Daemon löst mit seinem
/// Projekt-Profil und lädt seine gespeicherte Projektwahl. `None` lässt
/// den Daemon den eingestellten Ordner nehmen.
fn pull_request(
    reference: Option<&str>,
    pull_id: String,
    folder: Option<(&std::path::Path, &humanitl_sandbox::WorkDirSource)>,
    overrides: Vec<v1::sandbox_request::CliOverride>,
) -> v1::sandbox_request::ImagePull {
    let backend = overrides
        .iter()
        .find(|pair| pair.path == "sandbox.backend")
        .map(|pair| pair.value.clone())
        .unwrap_or_default();
    v1::sandbox_request::ImagePull {
        reference: reference.unwrap_or_default().to_owned(),
        pull_id,
        plan: Some(v1::sandbox_request::Plan {
            work_dir: folder
                .map(|(dir, _)| dir.display().to_string())
                .unwrap_or_default(),
            work_dir_source: folder
                .map(|(_, source)| humanitl_ipc::work_dir::source_to_proto(source)),
            backend,
            cli_overrides: overrides,
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

/// Der Befehl, der dieses Ziehen wiederholt, mit der Referenz und dem
/// Backend, die der Mensch nannte (HUM-505); ohne sie zöge ein kopierter Fix
/// das eingestellte Abbild statt des gemeinten.
fn pull_command(reference: Option<&str>, backend: Option<&str>) -> String {
    let mut command = "humanitl sandbox image pull".to_owned();
    if let Some(backend) = backend {
        command.push_str(" --backend ");
        command.push_str(&crate::render::shell_word(backend));
    }
    if let Some(reference) = reference {
        command.push(' ');
        command.push_str(&crate::render::shell_word(reference));
    }
    command
}

/// Fragt auf dem Terminal nach; ohne Terminal ist `--yes` nötig. Der Fix
/// nennt `command`, ohne Terminal mit `--yes`.
fn confirm(ctx: &Context, question: &str, command: &str) -> Result<(), Failure> {
    let stdin = std::io::stdin();
    if ctx.render.is_json() || !stdin.is_terminal() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(
                    "pulling uses the network directly, not through the proxy, so it asks \
                     first, and there is no terminal to ask on",
                )
                .fix(FixAction::CopyCommand(with_yes(command)))
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
                .fix(FixAction::CopyCommand(command.to_owned()))
                .build(),
        ))
    }
}

/// `command` mit `--yes` gleich nach `pull`, vor den Argumenten.
fn with_yes(command: &str) -> String {
    command.replacen(
        "humanitl sandbox image pull",
        "humanitl sandbox image pull --yes",
        1,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// Der Fix nennt die Referenz und das Backend, die der Mensch nannte,
    /// und `--yes` steht vor ihnen (HUM-505). Mutationsbeweis: der feste
    /// Befehl von früher fällt durch.
    #[test]
    fn the_fix_repeats_the_reference_and_the_backend() {
        let reference = "docker.io/library/debian@sha256:abc";
        assert_eq!(
            with_yes(&pull_command(Some(reference), Some("x"))),
            format!("humanitl sandbox image pull --yes --backend x {reference}")
        );
        assert_eq!(pull_command(None, None), "humanitl sandbox image pull");
        assert_eq!(
            pull_command(Some("a b"), None),
            "humanitl sandbox image pull 'a b'"
        );
    }

    /// Der Satz vor dem Ziehen sagt, dass das Netz nicht durch den Proxy geht
    /// (HUM-439, Spezifikation 2).
    #[test]
    fn the_note_says_the_pull_bypasses_the_proxy() {
        assert!(NETWORK_NOTE.contains("directly"));
        assert!(NETWORK_NOTE.contains("not through the Humanitl proxy"));
    }

    /// Ein Kontext für `humanitl sandbox image pull --yes` mit diesen
    /// Argumenten und dieser Umgebung, in einem leeren Zuhause.
    fn pull_context(home: &std::path::Path, args: &[&str], extra: &[(&str, &str)]) -> Context {
        let matches = crate::cli::command()
            .try_get_matches_from(
                ["humanitl", "sandbox", "image", "pull", "--yes"]
                    .iter()
                    .chain(args),
            )
            .expect("the command line parses");
        let mut pairs = vec![
            ("HOME".to_owned(), home.display().to_string()),
            (
                "XDG_CONFIG_HOME".to_owned(),
                home.join("cfg").display().to_string(),
            ),
        ];
        pairs.extend(
            extra
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
        );
        let env = humanitl_config::Env::from_pairs(pairs);
        Context {
            paths: humanitl_config::Paths::new(env.clone()),
            env,
            cwd: home.join("project"),
            cli_config: crate::cli::config_pairs(&matches),
            config_file: None,
            profile: None,
            profile_means: crate::cmd::ProfileMeaning::Session,
            render: crate::render::Renderer::new(false, 0, true),
            source_lock: humanitl_config::agents::SourceLock::none(),
        }
    }

    /// Der Plan des Ziehens, das dieser Kontext schickte.
    fn sent_plan(ctx: &Context) -> v1::sandbox_request::Plan {
        let resolved = ctx.config().expect("the client resolves");
        let (dir, source) = ctx.work_dir(&resolved);
        pull_request(
            None,
            String::new(),
            Some((&dir, &source)),
            crate::cmd::run::client_overrides(ctx, &resolved),
        )
        .plan
        .expect("the pull carries a plan")
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
        let home = tempfile::tempdir().expect("tempdir");
        let sent = |args: &[&str], env: &[(&str, &str)]| {
            sent_plan(&pull_context(home.path(), args, env)).backend
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
        let request = pull_request(Some("r@sha256:x"), "id".to_owned(), None, Vec::new());
        assert_eq!(
            (request.reference.as_str(), request.pull_id.as_str()),
            ("r@sha256:x", "id")
        );
    }

    /// Das Ziehen schickt genau die Werte der Sitzung eines Starts desselben
    /// Aufrufs (HUM-567), vor allem die Untergrenze aus Flag oder Umgebung,
    /// und den Ordner, in dem dieser Start geschähe, damit der Daemon dessen
    /// Projektwahl lädt.
    ///
    /// Mutation proof: `pull_request` mit leeren `cli_overrides` oder ohne
    /// `work_dir` lässt eine Zusicherung scheitern.
    #[test]
    fn the_pull_sends_the_session_values_and_the_folder_of_a_start() {
        let home = tempfile::tempdir().expect("tempdir");
        for (args, env) in [
            (&["--sandbox-min-isolation", "kernel"][..], &[][..]),
            (
                &[][..],
                &[("HUMANITL_SANDBOX__MIN_ISOLATION", "kernel")][..],
            ),
        ] {
            let ctx = pull_context(home.path(), args, env);
            let plan = sent_plan(&ctx);
            let start = crate::cmd::run::client_overrides(&ctx, &ctx.config().expect("resolves"));
            assert!(
                start
                    .iter()
                    .any(|pair| pair.path == "sandbox.min_isolation" && pair.value == "kernel"),
                "{start:?}"
            );
            assert_eq!(plan.cli_overrides, start, "{args:?} {env:?}");
            assert_eq!(
                plan.work_dir,
                home.path().join("project").display().to_string()
            );
        }
    }

    /// Das Ziehen schickt den Ordner des Starts, damit auch ein älterer
    /// Daemon sein Projekt-Profil sieht (HUM-567), außer er ist nur das
    /// aktuelle Verzeichnis und der Start lehnt ihn ab; ein genannter Ordner
    /// geht immer mit, und der Daemon nennt dann den Befund des Starts.
    /// Vor 1.57 lehnt das Ziehen ab, wenn die gespeicherte Projektwahl des
    /// Ordners einen Schlüssel `sandbox.*` setzt: Ein solcher Daemon zöge
    /// ohne sie. Eine Wahl ohne solche Schlüssel hält nichts auf.
    ///
    /// Mutation proof: `sent_folder`, das nie einen Ordner schickt, oder
    /// eines ohne die Ausnahme für das abgelehnte aktuelle Verzeichnis
    /// (oder ohne den Blick auf die Herkunft) lässt eine der ersten
    /// Zusicherungen scheitern; `refuse_without_choice`, das immer `Ok`
    /// gibt, oder `backend_choice_keys` ohne den Filter auf den gewählten
    /// Wert lassen eine spätere scheitern.
    #[test]
    fn the_folder_goes_along_unless_it_is_a_refused_current_directory() {
        use humanitl_sandbox::WorkDirSource;

        let dir = std::path::Path::new("/home/someone/project");
        let cwd = WorkDirSource::CurrentDir;
        let flag = WorkDirSource::Flag;
        assert_eq!(
            sent_folder((dir, &cwd), false).map(|(dir, _)| dir),
            Some(dir)
        );
        assert!(sent_folder((dir, &cwd), true).is_none());
        assert_eq!(
            sent_folder((dir, &flag), true).map(|(dir, _)| dir),
            Some(dir)
        );

        let field = |key: &str, chosen: &str| v1::ProjectChoiceField {
            key: key.to_owned(),
            chosen: chosen.to_owned(),
            ..v1::ProjectChoiceField::default()
        };
        let shown = v1::ProjectsResponse {
            choice: vec![
                field("sandbox.backend", humanitl_sandbox::VM_BACKEND),
                field("sandbox.min_isolation", ""),
                field("hold.ask_mode", "none"),
            ],
            ..v1::ProjectsResponse::default()
        };
        let chosen = backend_choice_keys(&shown);
        assert_eq!(
            chosen,
            [format!(
                "sandbox.backend = {:?}",
                humanitl_sandbox::VM_BACKEND
            )]
        );
        let refused =
            refuse_without_choice(CHOICE_MINOR - 1, &chosen).expect_err("an older daemon");
        assert_eq!(refused.diagnostic.code, codes::CLI_004);
        assert!(
            refused.diagnostic.why.contains("1.57"),
            "{}",
            refused.diagnostic.why
        );
        assert!(refuse_without_choice(CHOICE_MINOR, &chosen).is_ok());
        let ask_only = v1::ProjectsResponse {
            choice: vec![field("hold.ask_mode", "none")],
            ..v1::ProjectsResponse::default()
        };
        assert!(refuse_without_choice(CHOICE_MINOR - 1, &backend_choice_keys(&ask_only)).is_ok());
    }

    /// Ein älterer Daemon überginge Werte, die das Backend mitentscheiden:
    /// das Backend vor 1.40, Untergrenze und Laufzeit vor 1.55 (HUM-567).
    /// Er wird dann abgelehnt, mit der Minor, die der Wert braucht; Werte,
    /// die das Backend nicht berühren, halten das Ziehen nie auf.
    ///
    /// Mutation proof: `check_minor` ohne die Prüfung der Werte, oder
    /// `backend_values` mit `None` für die Untergrenze, lässt das Ziehen mit
    /// der Untergrenze an einem Daemon 1.54 durch.
    #[test]
    fn an_older_daemon_is_refused_for_values_that_choose_the_backend() {
        let pair = |path: &str, value: &str| v1::sandbox_request::CliOverride {
            path: path.to_owned(),
            value: value.to_owned(),
        };
        let backend = [pair("sandbox.backend", "auto")];
        let floor = [pair("sandbox.min_isolation", "kernel")];
        let other = [pair("hold.timeout_secs", "30")];

        let refused = check_minor(PLAN_MINOR - 1, &backend).expect_err("an older daemon");
        assert_eq!(refused.diagnostic.code, codes::CLI_004);
        assert!(check_minor(PLAN_MINOR, &backend).is_ok());
        assert!(check_minor(PLAN_MINOR - 1, &[]).is_ok());

        let refused = check_minor(VALUES_MINOR - 1, &floor).expect_err("no floor below 1.55");
        assert!(
            refused.diagnostic.why.contains("sandbox.min_isolation")
                && refused.diagnostic.why.contains("1.55"),
            "{}",
            refused.diagnostic.why
        );
        assert!(check_minor(VALUES_MINOR, &floor).is_ok());
        assert!(check_minor(IMAGE_MINOR, &other).is_ok());
        assert!(check_minor(IMAGE_MINOR - 1, &[]).is_err());
    }
}
