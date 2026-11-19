//! The agent depot (ADR-0023, HUM-518): an agent installed and logged in once
//! under the daemon's data directory, taken by every sandbox from there.
//!
//! Nothing here is on by default. `agents.source = "host"` keeps today's
//! behaviour, and a session without a depot agent carries an empty
//! [`DepotSession`], which changes no plan by a byte.
//!
//! The parts:
//!
//! - [`recipe`]: one declarative entry per agent; nothing else names an agent.
//! - [`store`]: the layout on disk and its checks (`AGENT_008`, `AGENT_013`).
//! - `tree`: the trust check of a version directory before every bind.
//! - [`scratch`]: the session's writable copy of the configuration.
//! - [`guard`]: the one check of the final plan request (`AGENT_017`) that
//!   every [`crate::SandboxBackend::plan`] calls first.
//! - [`plan_session`]: what a start path asks before it plans; the daemon and
//!   the local CLI call it alike.
//!
//! **Never on the host.** No function here starts anything from the depot,
//! not even `--version`: the version comes from `manifest.json`.

pub mod guard;
pub mod recipe;
pub mod scratch;
pub mod store;
pub(crate) mod tree;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use humanitl_config::agents::{SourceLock, effective_source};
use humanitl_config::{Config, Paths};
use humanitl_core::diagnostics::codes::{AGENT_015, AGENT_017};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};

pub use self::recipe::{DepotRecipe, KNOWN, KnownAgent, LoginFlow};
pub use self::scratch::Scratch;
pub use self::store::{Depot, Installed, Manifest, Reservation};
use crate::agent::AGENT_PROGRAM_DIR;

/// Where the session's copy of the configuration lies in the sandbox,
/// writable. Under `/run/humanitl`, which never comes from the host.
pub const DEPOT_CONFIG_DST: &str = "/run/humanitl/agent-config";

/// An agent whose program must not come from the host in this session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedAgent {
    /// Its identifier.
    pub agent: String,
    /// Its program's file name.
    pub program: String,
    /// The host install locations its recipe lists.
    pub host_installs: Vec<PathBuf>,
    /// The policy that fixes it, if one does.
    pub locked_by: Option<String>,
}

/// The depot's mount kind of the daemon (next to the single-file
/// [`crate::agent::AgentProgram`]): one resolved version directory, read-only
/// at [`AGENT_PROGRAM_DIR`], and the session's scratch, writable at
/// [`DEPOT_CONFIG_DST`]. No adapter can add it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepotMount {
    /// The agent.
    pub agent: String,
    /// The version, from `manifest.json`.
    pub version: String,
    /// `program/<version>/` on the host, resolved; never `current`.
    pub program_dir: PathBuf,
    /// The program's file name in it.
    pub program: String,
    /// The scratch on the host.
    pub scratch: PathBuf,
    /// What the agent gets in its environment: the configuration variable
    /// and the self-update switches.
    pub env: Vec<(String, String)>,
}

impl DepotMount {
    /// The program's place in the sandbox.
    #[must_use]
    pub fn program_dst(&self) -> PathBuf {
        Path::new(AGENT_PROGRAM_DIR).join(&self.program)
    }
}

/// A host install location that exists and is masked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostMask {
    /// The resolved place on the host, the one a mount covers.
    pub target: PathBuf,
    /// The location as the recipe lists it (a link, perhaps).
    pub listed: PathBuf,
    /// Whether the target is a directory.
    pub dir: bool,
}

/// What a session gets from the depot, part of
/// [`crate::profile::SessionContext`]. Empty: nothing changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DepotSession {
    /// The agents whose source is the depot or locked there.
    pub reserved: Vec<ReservedAgent>,
    /// The configured agent of the session, when it is one of `reserved`.
    pub session_agent: Option<String>,
    /// Whether the command is the agent's own (not a caller's shell).
    pub agent_command: bool,
    /// The agent's own command as this plan judged and rewrote it, before an
    /// adapter wraps it for the system (on macOS `OpenCode`'s launcher on the
    /// shared loopback puts `/bin/sh -c ...` in front). The guard judges these
    /// words when the final command ends with them; empty for a caller's
    /// command.
    pub agent_words: Vec<OsString>,
    /// The version directory and scratch of the session's depot agent.
    pub mount: Option<DepotMount>,
}

impl DepotSession {
    /// Whether the depot plays no part in this session.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reserved.is_empty() && self.mount.is_none()
    }

    /// The host install locations to mask: those of every reserved agent that
    /// exist on the host, resolved. A missing one is skipped: there is nothing
    /// to reach, and the backend could not create a mount point under a
    /// read-only `/usr` (`EROFS`). Files come before directories, so a file
    /// whose link leads into a masked directory is covered before the
    /// directory is.
    #[must_use]
    pub fn masks(&self) -> Vec<HostMask> {
        let mut masks: Vec<HostMask> = self
            .reserved
            .iter()
            .flat_map(|agent| agent.host_installs.iter())
            .filter_map(|listed| {
                let meta = std::fs::metadata(listed).ok()?;
                let target = std::fs::canonicalize(listed).ok()?;
                Some(HostMask {
                    target,
                    listed: listed.clone(),
                    dir: meta.is_dir(),
                })
            })
            .collect();
        masks.sort_by(|a, b| (a.dir, &a.target).cmp(&(b.dir, &b.target)));
        masks.dedup_by(|a, b| a.target == b.target);
        masks
    }

    /// The variables the depot adds to the session's environment.
    #[must_use]
    pub fn env(&self) -> &[(String, String)] {
        self.mount
            .as_ref()
            .map_or(&[], |mount| mount.env.as_slice())
    }
}

/// Whether a plan only shows the session or starts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A preview or snapshot: no scratch is created.
    Preview,
    /// A start: the scratch is created.
    Start,
}

/// What a start path hands [`plan_session`].
#[derive(Debug)]
pub struct DepotInput<'a> {
    /// The resolved configuration.
    pub config: &'a Config,
    /// The lock of a company policy.
    pub lock: &'a SourceLock,
    /// The daemon's paths.
    pub paths: &'a Paths,
    /// The session.
    pub session: SessionId,
    /// `agent.adapter`.
    pub adapter: &'a str,
    /// The command as the start path built it.
    pub command: &'a [OsString],
    /// Whether the command came from the caller (`run --`).
    pub from_caller: bool,
    /// Preview or start.
    pub purpose: Purpose,
}

/// What [`plan_session`] answers.
#[derive(Debug, Default)]
pub struct DepotPlan {
    /// For [`crate::profile::SessionContext::agent_depot`].
    pub session: DepotSession,
    /// The command, with a bare depot program name pointed at its place.
    pub command: Vec<OsString>,
    /// Whether the start path may bind its configured agent from the host
    /// (HUM-278, HUM-280); false when that agent comes from the depot.
    pub host_bind: bool,
    /// The session's scratch, for a start with a mount.
    pub scratch: Option<Scratch>,
    /// Notices for the session.
    pub warnings: Vec<Diagnostic>,
}

/// Which agents are reserved, which one the session takes from the depot,
/// and its mount (ADR-0023 points 1 and 3).
///
/// Without any agent on `depot` the answer is empty and the command stays as
/// it is: the plan is the one of today, byte for byte.
///
/// # Errors
///
/// `AGENT_015` when the session's own agent comes from the depot and has no
/// recipe, `AGENT_008` when it is not installed, `AGENT_013` when its depot
/// fails a check, `AGENT_017` when the agent's own command names a host
/// program of a depot agent.
pub fn plan_session(input: &DepotInput<'_>) -> Result<DepotPlan, Diagnostic> {
    let reserved: Vec<ReservedAgent> = KNOWN
        .iter()
        .filter_map(|agent| {
            let source = effective_source(agent.id, input.config, input.lock);
            source.is_depot().then(|| ReservedAgent {
                agent: agent.id.to_owned(),
                program: agent.program.to_owned(),
                host_installs: agent.host_installs.iter().map(PathBuf::from).collect(),
                locked_by: source.locked.map(|locked| locked.by),
            })
        })
        .collect();
    let mut plan = DepotPlan {
        command: input.command.to_vec(),
        host_bind: true,
        ..DepotPlan::default()
    };
    if reserved.is_empty() {
        return Ok(plan);
    }
    let is_reserved = |id: &str| reserved.iter().any(|agent| agent.agent == id);
    // The agent the start path would bind from the host: `agent.command`'s
    // program when it names a known agent, otherwise the adapter's.
    let configured = input
        .config
        .agent
        .command
        .as_ref()
        .and_then(|parts| parts.first())
        .and_then(|first| Path::new(first).file_name())
        .and_then(recipe::by_program)
        .map_or(input.adapter, |agent| agent.id);
    let session_agent = is_reserved(configured).then(|| configured.to_owned());
    plan.host_bind = session_agent.is_none();

    let first = input.command.first().map(OsString::as_os_str);
    let bare = first.filter(|word| !word.as_encoded_bytes().contains(&b'/'));
    let named = bare
        .and_then(recipe::by_program)
        .filter(|agent| is_reserved(agent.id));
    let agent_command = !input.from_caller;

    let target = if let Some(agent) = named {
        // The command names a depot agent by its bare name. A missing or
        // broken depot is then its own finding; only a named agent without a
        // recipe is left to the guard (`AGENT_017`, a host program).
        Some((agent, agent_command || agent.recipe.is_some()))
    } else if let Some(id) = session_agent.as_deref() {
        if agent_command && let Some(word) = first {
            // The agent's own command names something else than its depot
            // program: a host path, a wrapper, another program.
            return Err(host_program(id, word));
        }
        // A caller's shell gets the configured agent beside it, if it can.
        recipe::known(id).map(|agent| (agent, agent_command))
    } else {
        None
    };

    if let Some((agent, required)) = target {
        match mount_for(input, agent, &mut plan) {
            Ok(()) => {}
            Err(found) if required => return Err(found),
            Err(mut found) => {
                found.severity = Severity::Warning;
                plan.warnings.push(found);
            }
        }
        if let (Some(mount), Some(word)) = (&plan.session.mount, bare)
            && word == OsStr::new(&mount.program)
            && let Some(slot) = plan.command.first_mut()
        {
            *slot = mount.program_dst().into_os_string();
        }
    }
    plan.session.reserved = reserved;
    plan.session.session_agent = session_agent;
    plan.session.agent_command = agent_command;
    if agent_command {
        plan.session.agent_words.clone_from(&plan.command);
    }
    Ok(plan)
}

/// The mount of `agent` from its depot, with the scratch for a start.
fn mount_for(
    input: &DepotInput<'_>,
    agent: &KnownAgent,
    plan: &mut DepotPlan,
) -> Result<(), Diagnostic> {
    let Some(recipe) = agent.recipe else {
        return Err(no_recipe(agent));
    };
    let depot = Depot::of(input.paths, agent.id);
    // A start reserves the depot before it reads it, and holds the
    // reservation until the sandbox ends: `remove` waits for it.
    let reservation = match input.purpose {
        Purpose::Preview => None,
        Purpose::Start => Some(depot.reserve_for_session()?),
    };
    let installed = depot.installed()?;
    let scratch_path = scratch::scratch_path(input.paths, input.session);
    let scratch = match reservation {
        None => None,
        Some(reservation) => Some(scratch::create(
            input.paths,
            input.session,
            &depot,
            Some(reservation),
        )?),
    };
    let mut env = vec![(recipe.config_env.to_owned(), DEPOT_CONFIG_DST.to_owned())];
    env.extend(
        recipe
            .update_off
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
    );
    plan.session.mount = Some(DepotMount {
        agent: agent.id.to_owned(),
        version: installed.version,
        program_dir: installed.dir,
        program: recipe.program.to_owned(),
        scratch: scratch_path,
        env,
    });
    plan.scratch = scratch;
    Ok(())
}

/// `AGENT_015` for an agent without a recipe.
fn no_recipe(agent: &KnownAgent) -> Diagnostic {
    Diagnostic::builder(AGENT_015, Severity::Blocking)
        .why(format!(
            "the source of {} is the depot, and its adapter brings no recipe for the depot yet; \
             nothing of the host is started instead",
            agent.display_name
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents source {} host",
            agent.id
        )))
        .build()
}

/// `AGENT_017` for a command that names a host program of a depot agent.
pub(crate) fn host_program(agent: &str, named: &OsStr) -> Diagnostic {
    let program = recipe::known(agent).map_or(agent, |known| known.program);
    Diagnostic::builder(AGENT_017, Severity::Blocking)
        .why(format!(
            "the source of {agent} is the depot, and the command names {}, a program of the \
             host; only the depot program, by its bare name {program}, starts",
            named.to_string_lossy()
        ))
        .maybe_fix(crate::agent::command_fix(program))
        .build()
}

/// The notices at load (HUM-518): `depot` for an agent without a recipe is
/// valid, says `AGENT_015` as a warning, and a start with it stops there.
/// Daemon and CLI add them to what loading the configuration reports.
#[must_use]
pub fn load_notices(config: &Config) -> Vec<Diagnostic> {
    humanitl_config::agents::depot_notices(config, |agent| {
        recipe::known(agent).is_some_and(|known| known.recipe.is_some())
    })
}

/// `AGENT_017` for a bind that brings a host install of a depot agent into
/// the sandbox under another path.
pub(crate) fn host_bind(agent: &str, src: &Path, dst: &Path) -> Diagnostic {
    Diagnostic::builder(AGENT_017, Severity::Blocking)
        .why(format!(
            "the source of {agent} is the depot, and the plan binds {} at {}, which is or holds \
             a host install of it; only the depot program starts",
            src.display(),
            dst.display()
        ))
        .build()
}

/// The agents as the status lists them: every registered one.
#[must_use]
pub fn registered() -> &'static [KnownAgent] {
    KNOWN
}

#[cfg(test)]
mod tests;
