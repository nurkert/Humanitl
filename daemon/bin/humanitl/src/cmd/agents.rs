//! `humanitl agents status|source|remove` (HUM-518, ADR-0023).
//!
//! Ein gRPC-Aufruf je Befehl und sonst nichts (ADR-018): welche Quelle gilt,
//! ob eine Firmenvorgabe sie sperrt, welche Version im Depot liegt und ob eine
//! Sitzung es nutzt, weiß der Daemon. Die Befehle nennen keinen Agenten
//! selbst; sie zeigen, was der Daemon je registriertem Agenten meldet.

use std::io::{BufRead as _, IsTerminal as _};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cli::AgentsCmd;
use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};

/// Führt `humanitl agents <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; der Befund des Daemons, wenn er
/// ablehnt (`AGENT_010` unter einer Sperre, `AGENT_018` für ein Depot in
/// Gebrauch); `CLI_004`, wenn `remove` ohne Terminal und ohne `--yes` nicht
/// fragen kann oder die Antwort nicht ja war.
pub async fn run(ctx: &Context, cmd: &AgentsCmd) -> Result<u8, Failure> {
    let (op, only) = match cmd {
        AgentsCmd::Status { agent } => (
            v1::agents_request::Op::Status(v1::agents_request::Status {}),
            agent.clone(),
        ),
        AgentsCmd::Source { agent, source } => (
            v1::agents_request::Op::SetSource(v1::agents_request::SetSource {
                agent: agent.clone(),
                source: source.clone(),
            }),
            Some(agent.clone()),
        ),
        AgentsCmd::Remove { agent, yes } => {
            if !yes {
                confirm(ctx, agent)?;
            }
            (
                v1::agents_request::Op::Remove(v1::agents_request::Remove {
                    agent: agent.clone(),
                }),
                Some(agent.clone()),
            )
        }
    };
    let mut client = ctx.connect().await?;
    let status = client
        .agents(v1::AgentsRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?
        .into_inner();
    let shown: Vec<&v1::AgentDepot> = status
        .agents
        .iter()
        .filter(|entry| only.as_deref().is_none_or(|agent| entry.agent == agent))
        .collect();
    if let Some(agent) = only.as_deref()
        && shown.is_empty()
    {
        return Err(Failure::new(
            Diagnostic::builder(codes::CONFIG_003, Severity::Error)
                .why(format!("{agent:?} is no agent of the depot"))
                .fix(FixAction::CopyCommand("humanitl agents status".to_owned()))
                .build(),
        ));
    }
    print(ctx, &shown);
    Ok(EXIT_OK)
}

/// Fragt vor `remove` auf dem Terminal nach; ohne Terminal ist `--yes` nötig.
fn confirm(ctx: &Context, agent: &str) -> Result<(), Failure> {
    use std::io::Write as _;

    let stdin = std::io::stdin();
    if ctx.render.is_json() || !stdin.is_terminal() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "removing the depot of {agent} deletes its program, configuration and login \
                     and cannot be undone, so it asks first, and there is no terminal to ask on"
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl agents remove {agent} --yes"
                )))
                .build(),
        ));
    }
    let mut stderr = std::io::stderr();
    let _ = write!(
        stderr,
        "Remove the depot of {agent} with its program, configuration and login? [y/N] "
    );
    let _ = stderr.flush();
    let mut answer = String::new();
    let _ = stdin.lock().read_line(&mut answer);
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Info)
            .why(format!(
                "nothing was removed; the answer was not yes, so the depot of {agent} stays"
            ))
            .build(),
    ))
}

fn print(ctx: &Context, shown: &[&v1::AgentDepot]) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "agents": shown.iter().map(|entry| json!({
                "agent": entry.agent,
                "name": entry.display_name,
                "source": entry.source,
                "origin": entry.origin,
                "locked_by": entry.locked_by,
                "depot_supported": entry.depot_supported,
                "version": entry.version,
                "signature": entry.signature,
                "logged_in": entry.logged_in,
                "last_writeback": entry.last_writeback,
                "size_bytes": entry.size_bytes,
                "in_use": entry.in_use,
                "diagnostics": entry.diagnostics.iter().map(|found| json!({
                    "code": found.code,
                    "why": found.why,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }));
        return;
    }
    for entry in shown {
        ctx.render.line(&line(entry));
        for diagnostic in entry.diagnostics.iter().filter_map(from_proto) {
            ctx.render.diagnostic(&diagnostic);
        }
    }
}

/// Eine Zeile je Agent: Quelle mit Herkunft oder Sperre, dann das Depot.
fn line(entry: &v1::AgentDepot) -> String {
    let origin = if entry.locked_by.is_empty() {
        entry.origin.clone()
    } else {
        format!("fixed by your company: {}", entry.locked_by)
    };
    let depot = if entry.version.is_empty() {
        "not installed".to_owned()
    } else {
        format!(
            "{} ({}), {}{}",
            entry.version,
            if entry.signature.is_empty() {
                "signature unknown"
            } else {
                entry.signature.as_str()
            },
            if entry.logged_in {
                "logged in"
            } else {
                "not logged in"
            },
            if entry.in_use { ", in use" } else { "" }
        )
    };
    format!(
        "{} ({}): source {} ({origin}); depot {depot}; {} bytes",
        entry.display_name, entry.agent, entry.source, entry.size_bytes
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_locked_source_names_the_policy() {
        let entry = v1::AgentDepot {
            agent: "claude".to_owned(),
            display_name: "Claude Code".to_owned(),
            source: "depot".to_owned(),
            origin: "policy".to_owned(),
            locked_by: "/etc/humanitl/policy.toml".to_owned(),
            version: "2.1.0".to_owned(),
            signature: "verified".to_owned(),
            ..v1::AgentDepot::default()
        };
        assert_eq!(
            line(&entry),
            "Claude Code (claude): source depot (fixed by your company: \
             /etc/humanitl/policy.toml); depot 2.1.0 (verified), not logged in; 0 bytes"
        );
    }
}
