//! The one guard of the depot on the final plan request (ADR-0023 point 1,
//! HUM-518).
//!
//! The command of a session comes about on separate ways: the configured
//! agent beside a caller's command (`plan_configured_agent`), the adapter's
//! command (`bind_agent_program`), a caller's `run --`, the app's start
//! through the RPC, the local CLI. The guard therefore does not sit on any of
//! them; it checks what the sandbox receives, the [`PlanRequest`], and every
//! [`crate::SandboxBackend::plan`] calls it first, like
//! `refuse_stream` in the launcher. After it, no start path resolves or
//! binds a program any more.
//!
//! The rules, for every agent whose source is the depot or a lock:
//!
//! 1. No bind of a host program ([`crate::agent::AgentProgram`]) of such an
//!    agent, and none at all for the configured agent when it comes from the
//!    depot; never a host bind beside a depot mount.
//! 2. A bare program name of such an agent only when the depot mount is that
//!    agent's.
//! 3. A path whose name, or whose resolved target on the host, is such an
//!    agent's program or lies under one of its host install locations:
//!    refused. So are wrappers named after the agent and symbolic links to
//!    it.
//! 4. The agent's own command (not a caller's shell) under a depot source:
//!    only the depot program.
//!
//! 5. No bind of the plan that puts a host place under another path, a
//!    `--tests-dir`, a session bind, the project at `/work`, may be a host
//!    install location of such an agent, lie inside one or contain one: a
//!    mask covers a location only at its own path. A bind at its own path
//!    (the profile's `/usr`) is what the masks are for.
//!
//! A shell can still name a path the guard never sees (`/bin/sh -c
//! /usr/bin/claude`); the masks over the host install locations answer that
//! ([`super::DepotSession::masks`]).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use humanitl_core::Diagnostic;

use super::{DepotSession, host_program};
use crate::agent::AGENT_PROGRAM_DIR;
use crate::launcher::PlanRequest;

/// Checks the final plan request against the depot.
///
/// # Errors
///
/// `AGENT_017` when the request names or binds a host program of an agent
/// whose source is the depot.
pub fn check(request: &PlanRequest<'_>) -> Result<(), Diagnostic> {
    let session = request.session;
    let depot = &session.agent_depot;
    if depot.is_empty() {
        return Ok(());
    }
    if let Some(program) = &session.agent_program {
        let refused_for = depot
            .mount
            .as_ref()
            .map(|mount| mount.agent.clone())
            .or_else(|| depot.session_agent.clone())
            .or_else(|| reserved_by_path(depot, &program.dst))
            .or_else(|| reserved_by_path(depot, &program.host));
        if let Some(agent) = refused_for {
            return Err(host_program(&agent, program.host.as_os_str()));
        }
    }
    check_binds(request)?;
    // The agent's own command may reach the backend behind a launcher of its
    // adapter (on macOS `/bin/sh -c <script> opencode <command>`); the
    // adapter's words in front are code of Humanitl, the agent's words at the
    // end are the ones this guard judges. Any other shape is judged whole.
    let command = if depot.agent_command
        && !depot.agent_words.is_empty()
        && session.command.ends_with(&depot.agent_words)
    {
        depot.agent_words.as_slice()
    } else {
        session.command.as_slice()
    };
    let Some(first) = command.first() else {
        return Ok(());
    };
    let named = Path::new(first);
    if named.starts_with(AGENT_PROGRAM_DIR) && depot.mount.is_some() {
        return Ok(());
    }
    if !first.as_encoded_bytes().contains(&b'/') {
        let Some(agent) = depot
            .reserved
            .iter()
            .find(|agent| first == agent.program.as_str())
        else {
            return Ok(());
        };
        let mounted = depot
            .mount
            .as_ref()
            .is_some_and(|mount| mount.agent == agent.agent);
        if mounted {
            return Ok(());
        }
        return Err(host_program(&agent.agent, first));
    }
    if let Some(agent) = reserved_by_path(depot, named) {
        return Err(host_program(&agent, first));
    }
    if depot.agent_command
        && let Some(agent) = &depot.session_agent
    {
        return Err(host_program(agent, first));
    }
    Ok(())
}

/// Rule 5: every bind of the plan that moves a host place to another path.
fn check_binds(request: &PlanRequest<'_>) -> Result<(), Diagnostic> {
    let PlanRequest {
        profile,
        session,
        options,
    } = *request;
    let moved = options
        .session_ro
        .iter()
        .map(|(src, dst)| (src.as_path(), dst.as_path()))
        .chain(std::iter::once((
            session.work_src.as_path(),
            profile.mounts.work.dst.as_path(),
        )))
        .chain(
            profile
                .mounts
                .ro
                .iter()
                .chain(&profile.mounts.extra_ro)
                .chain(&profile.mounts.extra_rw)
                .map(|path| (path.as_path(), path.as_path())),
        );
    for (src, dst) in moved {
        let resolved = std::fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
        check_pinned(&session.agent_depot, &resolved, dst, src)?;
    }
    Ok(())
}

/// Rule 5 on one bind: `resolved` is the real place on the host, `dst` its
/// place in the sandbox, `shown` the path the finding names.
///
/// The guard asks it with the resolved path before planning; the backend asks
/// it again with the place of the descriptor it opened and binds
/// (`PinnedMounts::open`), so a symbolic link swapped between the two
/// reaches no mount either.
///
/// # Errors
///
/// `AGENT_017` when a bind to another path is, holds or lies inside a host
/// install of a reserved agent.
pub(crate) fn check_pinned(
    depot: &DepotSession,
    resolved: &Path,
    dst: &Path,
    shown: &Path,
) -> Result<(), Diagnostic> {
    if resolved == dst {
        return Ok(());
    }
    for agent in &depot.reserved {
        let hit = agent.host_installs.iter().any(|place| {
            let real = std::fs::canonicalize(place).unwrap_or_else(|_| place.clone());
            [place, &real]
                .into_iter()
                .any(|place| resolved.starts_with(place) || place.starts_with(resolved))
        });
        if hit {
            return Err(super::host_bind(&agent.agent, shown, dst));
        }
    }
    Ok(())
}

/// The reserved agent this path names: by its file name, by the file name of
/// its resolved target, or by a target under one of the agent's host install
/// locations.
fn reserved_by_path(depot: &DepotSession, path: &Path) -> Option<String> {
    let resolved: Option<PathBuf> = std::fs::canonicalize(path).ok();
    let names: Vec<&OsStr> = [Some(path), resolved.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(Path::file_name)
        .collect();
    depot
        .reserved
        .iter()
        .find(|agent| {
            names.iter().any(|name| *name == agent.program.as_str())
                || resolved.as_ref().is_some_and(|target| {
                    agent.host_installs.iter().any(|place| {
                        target.starts_with(place)
                            || std::fs::canonicalize(place)
                                .is_ok_and(|real| target.starts_with(real))
                    })
                })
        })
        .map(|agent| agent.agent.clone())
}
