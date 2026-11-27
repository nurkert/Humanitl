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
//! - `set_encryption` (HUM-523): seals or opens every credential file of the
//!   depot ([`humanitl_sandbox::agent::depot::seal::switch`]), all or
//!   nothing, and only then writes `agents.encrypt_credentials`; without a
//!   keyring that answers it refuses with `AGENT_020`. Every answer carries
//!   the state of the encryption ([`encryption`]).
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
use humanitl_core::diagnostics::codes::{
    AGENT_008, AGENT_009, AGENT_015, AGENT_018, AGENT_020, CONFIG_003, IPC_006,
};
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::agent::depot::seal::{self, CredentialKeys};
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
/// `AGENT_013` when removing fails; `AGENT_020` when the encryption cannot be
/// switched; the findings of writing `config.toml`.
pub fn answer(
    paths: &Paths,
    lock: &SourceLock,
    in_use: Option<&str>,
    keys: &CredentialKeys,
    op: Option<v1::agents_request::Op>,
) -> Result<v1::AgentsStatus, Diagnostic> {
    // Ein unterbrochenes Umschalten der Verschlüsselung wird zu Ende
    // geführt, bevor eine Antwort einen Stand nennt (HUM-523, Review).
    let keys = keys.clone().with_setting(setting(paths));
    let recovery = seal::recover(paths, &keys).err();
    let mut state = answer_op(paths, lock, in_use, &keys, op)?;
    if let (Some(found), Some(encryption)) = (recovery, state.encryption.as_mut()) {
        encryption
            .diagnostics
            .insert(0, convert::diagnostic_to_proto(&found));
    }
    Ok(state)
}

fn answer_op(
    paths: &Paths,
    lock: &SourceLock,
    in_use: Option<&str>,
    keys: &CredentialKeys,
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
        Some(v1::agents_request::Op::SetEncryption(set)) => {
            set_encryption(paths, keys, set.enabled)?;
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
            let mut state = status(paths, &configured(paths)?, lock, in_use, keys);
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
    Ok(status(paths, &configured(paths)?, lock, in_use, keys))
}

/// Switches the encryption of the credential files (HUM-523): one
/// transaction of [`seal::switch`] under one lock, the files, the state and
/// `agents.encrypt_credentials` in `config.toml` together; a session start
/// that reconciles waits for the same lock and reads the value written.
///
/// # Errors
///
/// `AGENT_020` without a keyring that answers or for a file that does not
/// open; `AGENT_012` while a write-back holds a depot; the findings of
/// writing `config.toml` (then the files are switched back).
pub fn set_encryption(
    paths: &Paths,
    keys: &CredentialKeys,
    enabled: bool,
) -> Result<(), Diagnostic> {
    let keys = keys.clone().with_setting(setting(paths));
    seal::switch(paths, &keys, enabled).map(|_switched| ())
}

/// `agents.encrypt_credentials` as the daemon reads and writes it
/// (HUM-523): read from `config.toml` without a project, written through
/// the same check as `SetConfig`; `false` removes the line.
#[must_use]
pub fn setting(paths: &Paths) -> seal::Setting {
    let read_paths = paths.clone();
    let write_paths = paths.clone();
    seal::Setting {
        read: std::sync::Arc::new(move || {
            configured(&read_paths).map(|config| config.agents.encrypt_credentials)
        }),
        write: std::sync::Arc::new(move |enabled| {
            let file = write_paths.config_path();
            let segments: Vec<String> = seal::SETTING.split('.').map(ToOwned::to_owned).collect();
            let value = enabled.then_some(toml::Value::Boolean(true));
            humanitl_config::edit::set_value(&file, &segments, value.as_ref(), |candidate| {
                crate::config_rpc::probe(&write_paths, &file, candidate, seal::SETTING)
            })?;
            // Durable before the switch goes on: switching off writes plain
            // files only after `false` is on disk (HUM-523, review).
            if let Some(dir) = file.parent() {
                std::fs::File::open(dir)
                    .and_then(|dir| dir.sync_all())
                    .map_err(|err| {
                        seal::unavailable(
                            &format!("{} cannot be synced ({err})", dir.display()),
                            None,
                        )
                    })?;
            }
            Ok(())
        }),
    }
}

/// The state of the encryption of the credential files (HUM-523): as the
/// depot says on disk, as `config.toml` says, and whether a keyring is
/// there to ask.
#[must_use]
pub fn encryption(
    paths: &Paths,
    config: &Config,
    keys: &CredentialKeys,
) -> v1::CredentialEncryption {
    let enabled = seal::enabled(paths);
    let configured = config.agents.encrypt_credentials;
    let keyring = keys.keyring_available();
    let mut findings = Vec::new();
    if !keyring {
        findings.push(seal::unavailable(
            "this system has no keyring the daemon can ask (on Linux secret-tool of \
             libsecret-tools), and the key lies only there, never in a file next to the data",
            (!cfg!(target_os = "macos"))
                .then(|| FixAction::CopyCommand("sudo apt-get install libsecret-tools".to_owned())),
        ));
    }
    if configured != enabled {
        findings.push(
            Diagnostic::builder(AGENT_020, Severity::Warning)
                .why(format!(
                    "config.toml says {} = {configured}, and the credential files of the depot \
                     are {}; the next start of a session from the depot follows config.toml, or \
                     switch it now",
                    seal::SETTING,
                    if enabled {
                        "encrypted"
                    } else {
                        "not encrypted"
                    }
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl config set {} {configured}",
                    seal::SETTING
                )))
                .build(),
        );
    }
    v1::CredentialEncryption {
        enabled,
        configured,
        keyring,
        diagnostics: findings.iter().map(convert::diagnostic_to_proto).collect(),
    }
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
    keys: &CredentialKeys,
) -> v1::AgentsStatus {
    v1::AgentsStatus {
        agents: KNOWN
            .iter()
            .map(|agent| agent_status(paths, config, lock, in_use, agent))
            .collect(),
        encryption: Some(encryption(paths, config, keys)),
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
    let mut unusable = false;
    if depot.exists() || source.source == AgentSource::Depot {
        match depot.installed() {
            Ok(installed) => {
                out.version = installed.version;
                out.signature = installed.manifest.signature;
                out.logged_in = depot.logged_in();
                out.credentials_encrypted = seal::files_sealed(&depot);
                out.last_writeback = depot
                    .last_writeback()
                    .map(|last| format!("{} session {}", last.at, last.session))
                    .unwrap_or_default();
                if !out.logged_in && agent.recipe.is_some() {
                    findings.push(not_logged_in(agent));
                }
            }
            Err(found) => {
                // Dieselbe Prüfung wie Installation und Update: Ein Depot,
                // das sie nicht besteht, nimmt keinen Lauf an (AGENT_013).
                unusable = found.code != AGENT_008;
                findings.push(found);
            }
        }
        out.size_bytes = depot.size();
        out.versions = depot.versions();
    }
    out.credential_writeback = config
        .agents
        .settings(agent.id)
        .is_none_or(|settings| settings.credential_writeback);
    // Nach einem Neustart weiß der Daemon von keinem Lauf mehr; eine
    // Konfiguration, die noch auf der Platte wartet, steht trotzdem da
    // (HUM-521). Ein Lauf aus dem Speicher geht in `with_maintenance` vor.
    out.maintenance = account::pending(paths, agent).map(|changes| v1::AgentMaintenance {
        task: MaintenanceTask::Config.as_str().to_owned(),
        state: CONFIRM.to_owned(),
        config_changes: changes
            .iter()
            .map(crate::sandbox::config_change_to_proto)
            .collect(),
        ..v1::AgentMaintenance::default()
    });
    out.diagnostics = findings.iter().map(convert::diagnostic_to_proto).collect();
    out.actions = Some(actions_of(&out, agent, depot.exists(), unusable));
    out
}

/// The state of a configuration run that waits for its confirmation.
const CONFIRM: &str = "confirm";

/// The state of a configuration run the person discarded (HUM-521).
pub const DISCARDED: &str = "discarded";

/// What a client may offer for one agent (HUM-521), before a running
/// maintenance run is known ([`with_maintenance`] takes back what it
/// blocks). The client decides nothing of it itself.
///
/// Whether a session could start now is not known here either;
/// [`with_startable`] takes the runs back when it could not.
fn actions_of(
    entry: &v1::AgentDepot,
    agent: &KnownAgent,
    exists: bool,
    unusable: bool,
) -> v1::AgentActions {
    let installs = agent
        .recipe
        .as_ref()
        .is_some_and(|recipe| recipe.install.is_some());
    let installed = !entry.version.is_empty();
    v1::AgentActions {
        install: installs && !installed && !unusable,
        update: installs && installed,
        login: installed && account::login_declared(agent),
        // A configuration run only where a variable points the agent at
        // the depot's copy (`agy` keeps its configuration in the session,
        // HUM-517).
        config: installed
            && agent.recipe.is_some_and(|recipe| {
                recipe.config_env.is_some() && recipe.config_refused.is_none()
            }),
        config_show: installed,
        remove: exists && !entry.in_use,
        source_host: entry.locked_by.is_empty(),
        // `set_source` nimmt `depot` immer an; ohne Rezept mit AGENT_015.
        source_depot: true,
    }
}

/// Takes back every maintenance run when the daemon could not start a
/// session now (HUM-521): another session runs, a start is being claimed,
/// or the daemon has no sandbox service. The run would be refused
/// (`CLI_005`, `IPC_006`).
pub fn with_startable(status: &mut v1::AgentsStatus, startable: bool) {
    if startable {
        return;
    }
    for actions in status
        .agents
        .iter_mut()
        .filter_map(|entry| entry.actions.as_mut())
    {
        actions.install = false;
        actions.update = false;
        actions.login = false;
        actions.config = false;
    }
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

/// The agent of a decision on a waiting configuration (HUM-520), and
/// whether it was accepted (HUM-521).
#[must_use]
pub fn config_decision_of(op: Option<&v1::agents_request::Op>) -> Option<(String, bool)> {
    match op {
        Some(v1::agents_request::Op::ConfigAccept(wish)) => {
            Some((wish.agent.trim().to_owned(), true))
        }
        Some(v1::agents_request::Op::ConfigDiscard(wish)) => {
            Some((wish.agent.trim().to_owned(), false))
        }
        _ => None,
    }
}

/// Puts the last maintenance run of each agent into its entry (HUM-563).
pub fn with_maintenance(
    status: &mut v1::AgentsStatus,
    runs: &std::collections::BTreeMap<String, v1::AgentMaintenance>,
) {
    for entry in &mut status.agents {
        // A configuration waiting on disk stays visible beside an older run
        // of this daemon that has ended (HUM-521).
        let waiting = entry
            .maintenance
            .as_ref()
            .is_some_and(|run| run.state == CONFIRM);
        if let Some(run) = runs.get(&entry.agent)
            && (!waiting || run.state == "running" || run.state == CONFIRM)
        {
            entry.maintenance = Some(run.clone());
        }
        // One run per agent at a time, and none under a removal (HUM-521).
        let running = entry
            .maintenance
            .as_ref()
            .is_some_and(|run| run.state == "running");
        if let (true, Some(actions)) = (running, entry.actions.as_mut()) {
            actions.install = false;
            actions.update = false;
            actions.login = false;
            actions.config = false;
            actions.remove = false;
        }
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
