//! `humanitl sandbox plan`: was ein Start in diesem Projekt bekäme, ohne zu
//! starten (HUM-400).
//!
//! Ein dünner Client (ADR-018): Der Daemon löst den Plan auf
//! (`SandboxRequest.Plan`), mit dem Projekt-Profil des Projektordners und
//! `Plan.llm_server`; hier wird nur gezeigt. Vor allem zeigt der Befehl,
//! welcher gespeicherte Modellserver gilt und woher die Wahl kommt: Ein
//! Projekt kann einen anderen Server wählen als den aktiven, und das soll
//! vor dem Start zu sehen sein.

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cmd::{Context, EXIT_OK, Failure, ProfileMeaning, from_proto, status_diagnostic};

/// Die erste Minor des Vertrags, die die Wahl des Servers je Lauf, Projekt
/// und Profil kennt (HUM-400).
pub const SERVER_CHOICE_MINOR: u32 = 28;

/// `sandbox plan`.
///
/// # Errors
///
/// `DAEMON_001` ohne Daemon, die Befunde des Plans, und `CONFIG_021`, wenn
/// der gewählte Server nicht gespeichert ist: Der Start lehnte dann ab.
pub async fn run(ctx: &Context) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    let (work_dir, work_dir_source) = ctx.work_dir(&resolved);
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;
    let llm_server = llm_server_pair(ctx).unwrap_or_default();
    // Ein älterer Daemon überginge Server und Profil der Sitzung still und
    // zeigte einen anderen Plan als den verlangten.
    let wants = !llm_server.is_empty()
        || (ctx.profile_means == ProfileMeaning::Session && ctx.profile_flag().is_some());
    if wants && info.proto_minor < SERVER_CHOICE_MINOR {
        return Err(Failure::new(old_daemon(info.proto_minor)));
    }
    let plan = plan_request(ctx, &work_dir, &work_dir_source, llm_server);
    let mut stream = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Plan(plan)),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
        .into_inner();
    let minor = info.proto_minor;
    collect(ctx, &mut stream, minor).await
}

/// Die Anfrage des Plans, mit den Flags wie unter `humanitl run` (HUM-400):
/// `--profile` ist das Profil der Sitzung, `--sandbox-profile` das Profil
/// des Sandbox-Backends, `--llm NAME` der Server des Laufs.
fn plan_request(
    ctx: &Context,
    work_dir: &std::path::Path,
    work_dir_source: &humanitl_sandbox::WorkDirSource,
    llm_server: String,
) -> v1::sandbox_request::Plan {
    let (profile, session_profile) = match ctx.profile_means {
        ProfileMeaning::Sandbox => (
            ctx.profile_flag().unwrap_or_default().to_owned(),
            String::new(),
        ),
        ProfileMeaning::Session => (
            ctx.cli_pairs()
                .into_iter()
                .find(|(path, _)| path == "sandbox.profile")
                .map(|(_, name)| name)
                .unwrap_or_default(),
            ctx.profile_flag().unwrap_or_default().to_owned(),
        ),
    };
    v1::sandbox_request::Plan {
        profile,
        work_dir: work_dir.display().to_string(),
        work_mode: String::new(),
        work_dir_source: Some(humanitl_ipc::work_dir::source_to_proto(work_dir_source)),
        llm_server,
        session_profile,
    }
}

/// Liest die Antwort des Plans und gibt sie aus.
async fn collect(
    ctx: &Context,
    stream: &mut tonic::Streaming<v1::SandboxEvent>,
    minor: u32,
) -> Result<u8, Failure> {
    let mut status = None;
    while let Some(event) = stream
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
    {
        match event.event {
            Some(v1::sandbox_event::Event::Diagnostic(wire)) => {
                if let Some(diagnostic) = from_proto(&wire) {
                    if diagnostic.severity >= Severity::Error {
                        return Err(Failure::new(diagnostic));
                    }
                    ctx.render.diagnostic(&diagnostic);
                }
            }
            Some(v1::sandbox_event::Event::Status(snapshot)) => status = Some(snapshot),
            _ => {}
        }
    }
    let Some(status) = status else {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why("the daemon answered the plan without a snapshot".to_owned())
                .fix(FixAction::CopyCommand("humanitl daemon status".to_owned()))
                .build(),
        ));
    };
    if ctx.render.is_json() {
        ctx.render.value(&plan_json(&status));
    } else {
        for line in plan_lines(&status, minor) {
            ctx.render.line(&line);
        }
    }
    // Ein Name ohne gespeicherten Server: Der Plan zeigt ihn, der Start
    // lehnte ab, und das sagt der Exit-Code.
    if let Some(refusal) = status
        .llm_server
        .as_ref()
        .and_then(|choice| choice.refusal.as_ref())
        .and_then(from_proto)
    {
        return Err(Failure::new(refusal));
    }
    Ok(EXIT_OK)
}

/// Der Server aus `--llm NAME` oder `--llm-server NAME`, wenn einer dasteht.
pub fn llm_server_pair(ctx: &Context) -> Option<String> {
    ctx.cli_pairs()
        .into_iter()
        .find(|(path, _)| path == humanitl_config::servers::SERVER)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Woher die Wahl kommt, als Satzteil: `this run`, `the project profile
/// PATH`, `the profile NAME` oder `llm.active`.
#[must_use]
pub fn source_text(choice: &v1::LlmServerChoice) -> String {
    use v1::llm_server_choice::Source;
    match Source::try_from(choice.source).unwrap_or(Source::Unspecified) {
        Source::Run => "this run".to_owned(),
        Source::Project => format!("the project profile {}", choice.detail),
        Source::Profile => format!("the profile {}", choice.detail),
        // `llm.server` in `config.toml` travels as the default with this
        // detail (HUM-400); without it the default is `llm.active`.
        Source::Default if choice.detail == "config.toml" => "llm.server in config.toml".to_owned(),
        Source::Default => "llm.active".to_owned(),
        Source::Unspecified => "an unknown place".to_owned(),
    }
}

/// Die Zeile über den Server: Name und Herkunft, oder dass keiner gilt.
///
/// `minor` ist die Vertrags-Minor des Daemons: Vor 1.28 kennt er das Feld
/// nicht, und ein fehlendes heißt dann „unbekannt", nicht „keiner".
#[must_use]
pub fn server_line(choice: Option<&v1::LlmServerChoice>, minor: u32) -> String {
    if choice.is_none() && minor < SERVER_CHOICE_MINOR {
        return format!("llm server: unknown (daemon 1.{minor})");
    }
    match choice {
        Some(choice) if choice.refusal.is_some() => format!(
            "llm server: {} (chosen by {}, not saved: the start refuses)",
            choice.name,
            source_text(choice)
        ),
        Some(choice) => format!(
            "llm server: {} (chosen by {})",
            choice.name,
            source_text(choice)
        ),
        None => "llm server: none saved applies".to_owned(),
    }
}

/// Die Zeilen des Plans für Menschen.
fn plan_lines(status: &v1::sandbox_event::Status, minor: u32) -> Vec<String> {
    let mut lines = vec![
        format!("sandbox profile: {}", status.profile),
        format!("work dir: {} ({})", status.work_dir, status.work_mode),
    ];
    if !status.sandbox_mode.is_empty() {
        lines.push(format!("mode: {}", status.sandbox_mode));
    }
    lines.push(server_line(status.llm_server.as_ref(), minor));
    lines.push(format!(
        "llm endpoint: {}",
        if status.llm_endpoint.is_empty() {
            "-"
        } else {
            status.llm_endpoint.as_str()
        }
    ));
    lines
}

/// Der Plan als JSON.
fn plan_json(status: &v1::sandbox_event::Status) -> serde_json::Value {
    let server = status.llm_server.as_ref().map(|choice| {
        use v1::llm_server_choice::Source;
        let source = match Source::try_from(choice.source).unwrap_or(Source::Unspecified) {
            Source::Run => "run",
            Source::Project => "project",
            Source::Profile => "profile",
            Source::Default => "default",
            Source::Unspecified => "",
        };
        json!({
            "name": choice.name,
            "source": source,
            "detail": choice.detail,
            "saved": choice.refusal.is_none(),
        })
    });
    json!({
        "profile": status.profile,
        "work_dir": status.work_dir,
        "work_mode": status.work_mode,
        "sandbox_mode": status.sandbox_mode,
        "llm_server": server,
        "llm_endpoint": status.llm_endpoint,
    })
}

/// `CLI_004` für einen Daemon, der die Wahl je Lauf nicht kennt.
fn old_daemon(minor: u32) -> Diagnostic {
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why(format!(
            "the daemon speaks contract 1.{minor}, and choosing a saved server for one run needs \
             1.{SERVER_CHOICE_MINOR}"
        ))
        .fix(FixAction::CopyCommand(
            "systemctl --user restart humanitld.service".to_owned(),
        ))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn choice(source: v1::llm_server_choice::Source, detail: &str) -> v1::LlmServerChoice {
        v1::LlmServerChoice {
            name: "work".to_owned(),
            source: source.into(),
            detail: detail.to_owned(),
            refusal: None,
        }
    }

    #[test]
    fn the_server_line_names_server_and_source() {
        use v1::llm_server_choice::Source;
        assert_eq!(
            server_line(
                Some(&choice(Source::Project, "/p/.humanitl/profile.toml")),
                28
            ),
            "llm server: work (chosen by the project profile /p/.humanitl/profile.toml)"
        );
        assert_eq!(
            server_line(Some(&choice(Source::Profile, "team")), 28),
            "llm server: work (chosen by the profile team)"
        );
        assert_eq!(
            server_line(Some(&choice(Source::Run, "")), 28),
            "llm server: work (chosen by this run)"
        );
        assert_eq!(
            server_line(Some(&choice(Source::Default, "")), 28),
            "llm server: work (chosen by llm.active)"
        );
        assert_eq!(server_line(None, 28), "llm server: none saved applies");
    }

    /// Ein Daemon vor 1.28 sagt nichts über den Server; das heißt nicht,
    /// dass keiner gilt.
    #[test]
    fn an_older_daemon_reads_as_unknown_and_not_as_none() {
        use v1::llm_server_choice::Source;
        assert_eq!(
            server_line(Some(&choice(Source::Default, "config.toml")), 28),
            "llm server: work (chosen by llm.server in config.toml)"
        );
        assert_eq!(server_line(None, 27), "llm server: unknown (daemon 1.27)");
        assert_eq!(server_line(None, 28), "llm server: none saved applies");
    }

    #[test]
    fn an_unsaved_server_says_the_start_refuses() {
        let mut refused = choice(v1::llm_server_choice::Source::Project, "/p");
        refused.refusal = Some(v1::Diagnostic::default());
        assert!(server_line(Some(&refused), 28).contains("the start refuses"));
    }

    #[test]
    fn the_json_carries_name_source_and_whether_it_is_saved() {
        let status = v1::sandbox_event::Status {
            llm_server: Some(choice(v1::llm_server_choice::Source::Project, "/p")),
            llm_endpoint: "https://llm.work.example/v1".to_owned(),
            ..Default::default()
        };
        let value = plan_json(&status);
        assert_eq!(value["llm_server"]["name"], "work");
        assert_eq!(value["llm_server"]["source"], "project");
        assert_eq!(value["llm_server"]["saved"], true);
        assert_eq!(value["llm_endpoint"], "https://llm.work.example/v1");
    }
}
