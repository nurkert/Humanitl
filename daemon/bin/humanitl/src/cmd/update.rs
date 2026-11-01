//! `humanitl update check` und `humanitl update apply` (HUM-382).
//!
//! Ein gRPC-Aufruf und sonst nichts (ADR-018): Ob es ein neueres Release
//! gibt, ob sein Manifest richtig signiert ist und wie diese Installation es
//! bekommt, entscheidet der Daemon. Nur ein Paket aus apt oder dnf
//! aktualisiert der Client selbst, mit dem festen Aufruf, den der Daemon in
//! `UpdateStatus.upgrade_command` nennt und der hier gegen dieselben
//! Konstanten geprüft wird: `pkexec` braucht den Polkit-Agenten der Sitzung,
//! und den hat der Dienst nicht. Keine Shell, kein Argument von anderswo.

use humanitl_ipc::client::Client;
use humanitl_ipc::update::{apt_command, dnf_command};
use humanitl_ipc::v1;
use serde_json::json;
use tokio_stream::StreamExt as _;

use crate::cli::UpdateCmd;
use crate::cmd::{Context, EXIT_OK, EXIT_USER, Failure, from_proto, status_diagnostic};

/// Führt `humanitl update <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; `UPDATE_001` bis `UPDATE_005`
/// mit dem Befund des Daemons.
pub async fn run(ctx: &Context, cmd: &UpdateCmd) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let appimage = ctx.env.non_empty("APPIMAGE").unwrap_or_default().to_owned();
    match cmd {
        UpdateCmd::Check => {
            let status = check(&mut client, true, &appimage).await?;
            print_status(ctx, &status);
            Ok(EXIT_OK)
        }
        UpdateCmd::Apply => apply(ctx, &mut client, &appimage).await,
    }
}

/// `Update(Check)`; der eine Stand aus dem Strom.
async fn check(
    client: &mut Client,
    force: bool,
    appimage: &str,
) -> Result<v1::UpdateStatus, Failure> {
    let mut stream = client
        .update(v1::UpdateRequest {
            op: Some(v1::update_request::Op::Check(v1::update_request::Check {
                force,
                appimage: appimage.to_owned(),
            })),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Update")))?
        .into_inner();
    while let Some(event) = stream.next().await {
        let event = event.map_err(|status| Failure::new(status_diagnostic(&status, "Update")))?;
        if let Some(v1::update_event::Event::Status(status)) = event.event {
            return Ok(status);
        }
    }
    Ok(v1::UpdateStatus::default())
}

fn method_name(method: i32) -> &'static str {
    match v1::UpdateMethod::try_from(method).unwrap_or_default() {
        v1::UpdateMethod::AppBundle => "app_bundle",
        v1::UpdateMethod::Archive => "archive",
        v1::UpdateMethod::Appimage => "appimage",
        v1::UpdateMethod::Apt => "apt",
        v1::UpdateMethod::Dnf => "dnf",
        v1::UpdateMethod::Manual => "manual",
        v1::UpdateMethod::Unspecified => "unknown",
    }
}

fn print_status(ctx: &Context, status: &v1::UpdateStatus) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "current_version": status.current_version,
            "latest_version": status.latest_version,
            "available": status.available,
            "can_apply": status.can_apply,
            "method": method_name(status.method),
            "upgrade_command": status.upgrade_command,
            "size": status.size,
        }));
        return;
    }
    if status.available {
        ctx.render.line(&format!(
            "Humanitl {} is available (this is {}).",
            status.latest_version, status.current_version
        ));
        if status.can_apply {
            ctx.render.note("Install it with: humanitl update apply");
        }
    } else {
        ctx.render.line(&format!(
            "Humanitl {} is up to date.",
            status.current_version
        ));
    }
    if let Some(notice) = status.notice.as_ref().and_then(from_proto) {
        ctx.render.diagnostic(&notice);
    }
}

/// `update apply`: der Weg, den der Daemon für diese Installation nennt.
async fn apply(ctx: &Context, client: &mut Client, appimage: &str) -> Result<u8, Failure> {
    let status = check(client, true, appimage).await?;
    if !status.available {
        print_status(ctx, &status);
        return Ok(EXIT_OK);
    }
    match v1::UpdateMethod::try_from(status.method).unwrap_or_default() {
        v1::UpdateMethod::Apt | v1::UpdateMethod::Dnf => Ok(package(ctx, &status)),
        v1::UpdateMethod::AppBundle | v1::UpdateMethod::Archive | v1::UpdateMethod::Appimage => {
            install(ctx, client, &status.latest_version, appimage).await
        }
        v1::UpdateMethod::Manual | v1::UpdateMethod::Unspecified => {
            print_status(ctx, &status);
            Ok(EXIT_USER)
        }
    }
}

/// apt oder dnf über `pkexec`, nur mit einem der beiden festen Aufrufe.
fn package(ctx: &Context, status: &v1::UpdateStatus) -> u8 {
    let wanted = &status.upgrade_command;
    if *wanted != apt_command() && *wanted != dnf_command() {
        ctx.render.note(&format!(
            "the daemon named an upgrade command this client does not run: {}",
            wanted.join(" ")
        ));
        return EXIT_USER;
    }
    let Some((program, args)) = wanted.split_first() else {
        return EXIT_USER;
    };
    ctx.render.note(&format!("running {}", wanted.join(" ")));
    let ran = std::process::Command::new(program).args(args).status();
    match ran {
        Ok(exit) if exit.success() => {
            ctx.render
                .line(&format!("Humanitl {} is installed.", status.latest_version));
            ctx.render.note(
                "The daemon restarts with: humanitl daemon retire-if-idle, then \
                 systemctl --user restart humanitld.service",
            );
            EXIT_OK
        }
        Ok(exit) => {
            ctx.render
                .note(&format!("{} exited with {exit}", wanted.join(" ")));
            EXIT_USER
        }
        Err(err) => {
            ctx.render
                .note(&format!("{program} could not be started: {err}"));
            EXIT_USER
        }
    }
}

/// `Update(Apply)` im Daemon, mit Fortschritt auf stderr.
async fn install(
    ctx: &Context,
    client: &mut Client,
    version: &str,
    appimage: &str,
) -> Result<u8, Failure> {
    let mut stream = client
        .update(v1::UpdateRequest {
            op: Some(v1::update_request::Op::Apply(v1::update_request::Apply {
                version: version.to_owned(),
                appimage: appimage.to_owned(),
            })),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Update")))?
        .into_inner();
    let mut last_percent = None;
    while let Some(event) = stream.next().await {
        let event = event.map_err(|status| Failure::new(status_diagnostic(&status, "Update")))?;
        match event.event {
            Some(v1::update_event::Event::Progress(progress)) => {
                let stage = v1::UpdateStage::try_from(progress.stage).unwrap_or_default();
                let line = match stage {
                    v1::UpdateStage::Downloading => {
                        let percent = progress
                            .done_bytes
                            .saturating_mul(100)
                            .checked_div(progress.total_bytes)
                            .unwrap_or(0);
                        if last_percent == Some(percent / 10) {
                            continue;
                        }
                        last_percent = Some(percent / 10);
                        format!("downloading {percent}%")
                    }
                    v1::UpdateStage::Verifying => "verifying".to_owned(),
                    v1::UpdateStage::Installing => "installing".to_owned(),
                    v1::UpdateStage::Unspecified => continue,
                };
                ctx.render.note(&line);
            }
            Some(v1::update_event::Event::Installed(installed)) => {
                if ctx.render.is_json() {
                    ctx.render.value(&json!({
                        "installed": installed.version,
                        "app_path": installed.app_path,
                    }));
                } else {
                    ctx.render.line(&format!(
                        "Humanitl {} is installed at {}.",
                        installed.version, installed.app_path
                    ));
                    ctx.render.note(
                        "Restart the app; the daemon follows with humanitl daemon \
                         retire-if-idle and humanitl daemon restart (macOS) or systemctl \
                         --user restart humanitld.service (Linux).",
                    );
                }
                return Ok(EXIT_OK);
            }
            _ => {}
        }
    }
    Ok(EXIT_USER)
}
