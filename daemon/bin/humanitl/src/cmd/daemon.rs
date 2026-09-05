//! `humanitl daemon status`: die Selbstauskunft des Dienstes.
//!
//! Ein dünner Client (ADR-018): verbinden, `GetInfo` rufen, ausgeben. Was der
//! Daemon kann, sagt er selbst; die Kommandozeile erfindet nichts dazu.

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, v1};
use serde_json::json;

use crate::cli::DaemonCmd;
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic};
use crate::render::table;

/// Führt `humanitl daemon <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `DAEMON_002`, wenn er eine
/// andere Major-Version des Vertrags spricht.
pub async fn run(ctx: &Context, cmd: &DaemonCmd) -> Result<u8, Failure> {
    match cmd {
        DaemonCmd::Status => status(ctx).await,
    }
}

/// `daemon status`.
async fn status(ctx: &Context) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();

    check_proto(&info)?;

    let socket = ctx.paths.daemon_socket().display().to_string();
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "socket": socket,
            "daemon_version": info.daemon_version,
            "proto_major": info.proto_major,
            "proto_minor": info.proto_minor,
            "capabilities": info.capabilities,
            "session_id": info.session_id,
        }));
        return Ok(EXIT_OK);
    }

    let session = if info.session_id.is_empty() {
        "-".to_owned()
    } else {
        info.session_id.clone()
    };
    let rows = vec![
        vec!["socket".to_owned(), socket],
        vec!["daemon".to_owned(), info.daemon_version.clone()],
        vec![
            "proto".to_owned(),
            format!("{}.{}", info.proto_major, info.proto_minor),
        ],
        vec!["session".to_owned(), session],
        vec![
            "capabilities".to_owned(),
            if info.capabilities.is_empty() {
                "-".to_owned()
            } else {
                info.capabilities.join(", ")
            },
        ],
    ];
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    Ok(EXIT_OK)
}

/// Prüft, ob Client und Daemon dieselbe Major-Version sprechen.
///
/// Eine andere Major heißt: Nachrichten, die der eine schickt, versteht der
/// andere nicht mehr. Eine kleinere Minor beim Daemon ist dagegen kein
/// Fehler, nur eine Notiz: additive Änderungen bleiben lesbar.
pub fn check_proto(info: &v1::Info) -> Result<(), Failure> {
    if info.proto_major == PROTO_MAJOR {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::DAEMON_002, Severity::Blocking)
            .why(format!(
                "the daemon speaks contract {}.{}, this humanitl speaks {PROTO_MAJOR}.{PROTO_MINOR}",
                info.proto_major, info.proto_minor
            ))
            .fix(FixAction::CopyCommand(
                "systemctl --user restart humanitld".to_owned(),
            ))
            .build(),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, v1};

    use super::check_proto;
    use crate::cmd::EXIT_DAEMON;

    fn info(major: u32) -> v1::Info {
        v1::Info {
            daemon_version: "0.0.0".to_owned(),
            proto_major: major,
            proto_minor: PROTO_MINOR,
            capabilities: vec!["hold".to_owned()],
            session_id: String::new(),
        }
    }

    #[test]
    fn the_same_major_is_accepted() {
        assert!(check_proto(&info(PROTO_MAJOR)).is_ok());
    }

    #[test]
    fn another_major_is_daemon_002_with_exit_two() {
        let failure = check_proto(&info(PROTO_MAJOR + 1)).expect_err("a newer daemon is refused");
        assert_eq!(failure.diagnostic.code.as_str(), "DAEMON_002");
        assert_eq!(failure.exit, EXIT_DAEMON);
    }
}
