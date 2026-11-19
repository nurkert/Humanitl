//! `Agents`: the agent depot (HUM-518, ADR-0023).
//!
//! The operations, every one answered with the state afterwards:
//!
//! - `status`: per registered agent the effective source with origin and
//!   lock, the installed version and signature from `manifest.json`, whether
//!   a login lies there, the last write-back (HUM-520), the size, and whether
//!   the running session uses it. Nothing here starts a program from the
//!   depot (ADR-0023 point 2).
//! - `set_source`: writes `agents.<id>.source` through the same path as
//!   `SetConfig`, with the same refusal under a company lock (`AGENT_010`).
//! - `remove`: removes the depot of one agent, refused while the running
//!   session has it bound (`AGENT_018`); the source stays as it is.
//! - `install` and `update` (HUM-563) are a maintenance session of the
//!   sandbox service (`crate::sandbox`); the server hands them there and
//!   answers with the state once the run stands. Every answer carries the
//!   last run of each agent ([`with_maintenance`]).
//!
//! The doctor's lines per agent with the source `depot` come from
//! [`insert_doctor_lines`]. Every function iterates over the registered
//! recipes; none names an agent.

use humanitl_config::agents::{AgentSource, SourceLock, effective_source};
use humanitl_config::{Config, Paths};
use humanitl_core::diagnostics::codes::{AGENT_009, AGENT_015, AGENT_018, CONFIG_003, IPC_006};
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::agent::depot::{Depot, KNOWN, KnownAgent, MaintenanceTask, account};

use crate::{convert, v1};

/// Answers one request.
///
/// Blocking: reads and writes files. The service calls it in
/// [`tokio::task::spawn_blocking`].
///
/// # Errors
///
/// `CONFIG_003` without an operation, for an unknown agent or source;
/// `AGENT_010` for `host` under a lock; `AGENT_018` for a depot in use;
/// `AGENT_013` when removing fails; the findings of writing `config.toml`.
pub fn answer(
    paths: &Paths,
    lock: &SourceLock,
    in_use: Option<&str>,
    op: Option<v1::agents_request::Op>,
) -> Result<v1::AgentsStatus, Diagnostic> {
    match op {
        Some(v1::agents_request::Op::Status(_)) => {}
        Some(v1::agents_request::Op::SetSource(set)) => {
            let agent = known(&set.agent)?;
            let source = AgentSource::parse(&set.source).ok_or_else(|| {
                refused(&format!(
                    "{:?} is no agent source; it takes host or depot",
                    set.source
                ))
            })?;
            let key = format!("agents.{}.source", agent.id);
            crate::config_rpc::set_locked(
                paths,
                &key,
                source.as_str(),
                "",
                lock,
                &humanitl_config::PolicyLocks::of_policy(paths),
            )?;
        }
        Some(v1::agents_request::Op::Remove(remove)) => {
            let agent = known(&remove.agent)?;
            if in_use == Some(agent.id) {
                return Err(Diagnostic::builder(AGENT_018, Severity::Error)
                    .why(format!(
                        "the running session has the depot of {} bound; it is not removed",
                        agent.display_name
                    ))
                    .fix(FixAction::CopyCommand("humanitl sandbox stop".to_owned()))
                    .build());
            }
            Depot::of(paths, agent.id).remove()?;
        }
        // The sandbox service runs them (HUM-563, HUM-520); the server
        // hands them there before it asks this function for the state
        // afterwards.
        Some(
            v1::agents_request::Op::Install(_)
            | v1::agents_request::Op::Update(_)
            | v1::agents_request::Op::Login(_)
            | v1::agents_request::Op::Config(_)
            | v1::agents_request::Op::DeliverCallback(_),
        ) => {
            return Err(refused(
                "install, update, login, configuration and a callback run in a maintenance \
                 session of the daemon's sandbox service, not here",
            ));
        }
        // HUM-520: the confirmation of a configuration run, or its discard.
        Some(v1::agents_request::Op::ConfigAccept(wish)) => {
            let agent = known(&wish.agent)?;
            account::accept_config(paths, agent.id)?;
        }
        Some(v1::agents_request::Op::ConfigDiscard(wish)) => {
            let agent = known(&wish.agent)?;
            account::discard_config(paths, agent.id);
        }
        Some(v1::agents_request::Op::ConfigShow(wish)) => {
            let agent = known(&wish.agent)?;
            let mut state = status(paths, &configured(paths)?, lock, in_use);
            if let Some(entry) = state
                .agents
                .iter_mut()
                .find(|entry| entry.agent == agent.id)
            {
                entry.config_files = account::show_config(paths, agent)
                    .iter()
                    .map(crate::sandbox::config_change_to_proto)
                    .collect();
            }
            return Ok(state);
        }
        None => return Err(refused("the request names no operation")),
    }
    Ok(status(paths, &configured(paths)?, lock, in_use))
}

/// The configuration without a project, as it lies on disk.
///
/// # Errors
///
/// The finding of loading, when `config.toml` cannot be read.
pub fn configured(paths: &Paths) -> Result<Config, Diagnostic> {
    humanitl_config::resolve(
        &humanitl_config::ProfileSelection::default(),
        None,
        paths.env(),
        &[],
    )
    .map(|resolved| resolved.config)
}

/// The state of every registered agent.
#[must_use]
pub fn status(
    paths: &Paths,
    config: &Config,
    lock: &SourceLock,
    in_use: Option<&str>,
) -> v1::AgentsStatus {
    v1::AgentsStatus {
        agents: KNOWN
            .iter()
            .map(|agent| agent_status(paths, config, lock, in_use, agent))
            .collect(),
    }
}

fn agent_status(
    paths: &Paths,
    config: &Config,
    lock: &SourceLock,
    in_use: Option<&str>,
    agent: &KnownAgent,
) -> v1::AgentDepot {
    let source = effective_source(agent.id, config, lock);
    let depot = Depot::of(paths, agent.id);
    let mut out = v1::AgentDepot {
        agent: agent.id.to_owned(),
        display_name: agent.display_name.to_owned(),
        source: source.source.as_str().to_owned(),
        origin: source.origin.as_str().to_owned(),
        locked_by: source.locked.map(|locked| locked.by).unwrap_or_default(),
        depot_supported: agent.recipe.is_some(),
        // The running session of this daemon, or any start that holds the
        // depot's reservation, a local `humanitl sandbox run` included.
        in_use: in_use == Some(agent.id) || depot.in_use(),
        ..v1::AgentDepot::default()
    };
    let mut findings = Vec::new();
    if source.source == AgentSource::Depot && agent.recipe.is_none() {
        findings.push(no_recipe(agent));
    }
    if depot.exists() || source.source == AgentSource::Depot {
        match depot.installed() {
            Ok(installed) => {
                out.version = installed.version;
                out.signature = installed.manifest.signature;
                out.logged_in = depot.logged_in();
                out.last_writeback = depot
                    .last_writeback()
                    .map(|last| format!("{} session {}", last.at, last.session))
                    .unwrap_or_default();
                if !out.logged_in && agent.recipe.is_some() {
                    findings.push(not_logged_in(agent));
                }
            }
            Err(found) => findings.push(found),
        }
        out.size_bytes = depot.size();
    }
    out.diagnostics = findings.iter().map(convert::diagnostic_to_proto).collect();
    out
}

/// One doctor line per agent with the source `depot`: version and login, or
/// what is missing. Behind the line `egress`, or at the end.
pub fn insert_doctor_lines(
    report: &mut v1::DoctorReport,
    paths: &Paths,
    config: &Config,
    lock: &SourceLock,
) {
    let lines: Vec<v1::DoctorCheck> = KNOWN
        .iter()
        .filter(|agent| effective_source(agent.id, config, lock).is_depot())
        .map(|agent| {
            let state = agent_status(paths, config, lock, None, agent);
            let evidence = if state.version.is_empty() {
                format!("{} from the depot: not installed", agent.display_name)
            } else {
                format!(
                    "{} {} from the depot, {}",
                    agent.display_name,
                    state.version,
                    if state.logged_in {
                        "logged in"
                    } else {
                        "not logged in"
                    }
                )
            };
            let worst = state
                .diagnostics
                .iter()
                .max_by_key(|found| found.severity)
                .cloned();
            let status = match &worst {
                None => v1::CheckStatus::Ok,
                Some(found) if found.severity == v1::Severity::Warning as i32 => {
                    v1::CheckStatus::Warn
                }
                Some(_) => v1::CheckStatus::Fail,
            };
            v1::DoctorCheck {
                id: format!("agent-depot-{}", agent.id),
                status: status.into(),
                evidence,
                diagnostic: worst,
            }
        })
        .collect();
    let at = report
        .checks
        .iter()
        .position(|check| check.id == "egress")
        .map_or(report.checks.len(), |index| index + 1);
    for (offset, line) in lines.into_iter().enumerate() {
        report.checks.insert(at + offset, line);
    }
}

fn known(agent: &str) -> Result<&'static KnownAgent, Diagnostic> {
    humanitl_sandbox::agent::depot::recipe::known(agent).ok_or_else(|| {
        refused(&format!(
            "{agent:?} is no agent of the depot; known: {}",
            KNOWN
                .iter()
                .map(|agent| agent.id)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}

/// The maintenance task an operation asks for, with its wish (HUM-563);
/// `None` for every other operation.
#[must_use]
pub fn maintenance_of(
    op: Option<&v1::agents_request::Op>,
) -> Option<(MaintenanceTask, v1::agents_request::Maintain)> {
    match op {
        Some(v1::agents_request::Op::Install(wish)) => {
            Some((MaintenanceTask::Install, wish.clone()))
        }
        Some(v1::agents_request::Op::Update(wish)) => Some((MaintenanceTask::Update, wish.clone())),
        Some(v1::agents_request::Op::Login(wish)) => Some((MaintenanceTask::Login, wish.clone())),
        Some(v1::agents_request::Op::Config(wish)) => Some((MaintenanceTask::Config, wish.clone())),
        _ => None,
    }
}

/// The version a wish names; `None` for the vendor's current one.
#[must_use]
pub fn wanted_version(version: &str) -> Option<String> {
    let version = version.trim();
    (!version.is_empty()).then(|| version.to_owned())
}

/// `IPC_006`: a daemon without a sandbox service runs no maintenance.
#[must_use]
pub fn no_sandbox_service() -> Diagnostic {
    Diagnostic::builder(IPC_006, Severity::Error)
        .why(
            "this daemon runs no sandboxes, so it cannot install or update an agent in the depot"
                .to_owned(),
        )
        .build()
}

/// Puts the last refused write-back of each agent into its findings
/// (HUM-520), until the next one is taken over.
pub fn with_writeback_refusals(
    status: &mut v1::AgentsStatus,
    refusals: &std::collections::BTreeMap<String, Diagnostic>,
) {
    for entry in &mut status.agents {
        if let Some(found) = refusals.get(&entry.agent) {
            entry.diagnostics.push(convert::diagnostic_to_proto(found));
        }
    }
}

/// The agent of a decision on a waiting configuration (HUM-520).
#[must_use]
pub fn config_decision_of(op: Option<&v1::agents_request::Op>) -> Option<String> {
    match op {
        Some(
            v1::agents_request::Op::ConfigAccept(wish)
            | v1::agents_request::Op::ConfigDiscard(wish),
        ) => Some(wish.agent.trim().to_owned()),
        _ => None,
    }
}

/// Puts the last maintenance run of each agent into its entry (HUM-563).
pub fn with_maintenance(
    status: &mut v1::AgentsStatus,
    runs: &std::collections::BTreeMap<String, v1::AgentMaintenance>,
) {
    for entry in &mut status.agents {
        entry.maintenance = runs.get(&entry.agent).cloned();
    }
}

fn refused(why: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(why.to_owned())
        .build()
}

fn no_recipe(agent: &KnownAgent) -> Diagnostic {
    Diagnostic::builder(AGENT_015, Severity::Warning)
        .why(format!(
            "the source of {} is the depot, and its adapter brings no recipe for the depot yet",
            agent.display_name
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents source {} host",
            agent.id
        )))
        .build()
}

fn not_logged_in(agent: &KnownAgent) -> Diagnostic {
    Diagnostic::builder(AGENT_009, Severity::Warning)
        .why(format!(
            "the depot holds {} and no login for it; the agent asks for one in the session",
            agent.display_name
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents login {}",
            agent.id
        )))
        .build()
}

#[cfg(test)]
mod tests;
