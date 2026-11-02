//! `humanitl intercept status|pause|resume` (HUM-407).
//!
//! Ein gRPC-Aufruf und sonst nichts (ADR-018): Was die Pause bedeutet, wie
//! lange sie gilt und wer sie sehen darf, entscheidet der Daemon. Die
//! Kommandozeile sagt nur, was gilt, und bei `pause` in zwei Sätzen, was
//! geschieht und was bleibt.

use humanitl_ipc::v1;
use serde_json::json;

use crate::cli::InterceptCmd;
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic};

/// Führt `humanitl intercept <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; `IPC_006`, wenn der Daemon
/// keine Proxy-Sitzung hat.
pub async fn run(ctx: &Context, cmd: &InterceptCmd) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let op = match cmd {
        InterceptCmd::Status => v1::intercept_request::Op::Status(v1::intercept_request::Status {}),
        InterceptCmd::Pause => v1::intercept_request::Op::Pause(v1::intercept_request::Pause {}),
        InterceptCmd::Resume => v1::intercept_request::Op::Resume(v1::intercept_request::Resume {}),
    };
    let status = client
        .intercept(v1::InterceptRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Intercept")))?
        .into_inner();
    print_status(ctx, &status, matches!(cmd, InterceptCmd::Pause));
    Ok(EXIT_OK)
}

/// Der Beginn der Pause als RFC 3339, falls es einen gibt.
fn since_text(status: &v1::InterceptStatus) -> Option<String> {
    let since = status.paused_since.as_ref()?;
    let nanos = u32::try_from(since.nanos).unwrap_or(0);
    chrono::DateTime::from_timestamp(since.seconds, nanos)
        .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn print_status(ctx: &Context, status: &v1::InterceptStatus, just_paused: bool) {
    let since = since_text(status);
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "paused": status.paused,
            "paused_since": since,
        }));
        return;
    }
    if !status.paused {
        ctx.render
            .line("Intercept is ON: every request no rule decides waits for your decision.");
        return;
    }
    ctx.render.line(&format!(
        "Intercept is PAUSED since {}: requests that would wait for your decision go out at once.",
        since.as_deref().unwrap_or("now")
    ));
    if just_paused {
        ctx.render.note(
            "Block rules, the hard block on verified secrets, the sandbox and the recording stay. \
             Every request that passes is recorded with the reason \"paused\". The pause ends \
             with `humanitl intercept resume`, with the next session and with the daemon.",
        );
    } else {
        ctx.render.note("Resume with: humanitl intercept resume");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::since_text;

    #[test]
    fn the_start_of_a_pause_reads_as_rfc3339_and_none_without_a_pause() {
        let status = v1::InterceptStatus {
            paused: true,
            paused_since: Some(prost_types::Timestamp {
                seconds: 1_793_491_200,
                nanos: 0,
            }),
            revision: 1,
            instance: String::new(),
        };
        assert_eq!(since_text(&status).as_deref(), Some("2026-11-01T00:00:00Z"));
        assert_eq!(since_text(&v1::InterceptStatus::default()), None);
    }
}
