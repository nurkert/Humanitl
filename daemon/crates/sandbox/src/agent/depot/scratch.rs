//! The per-session copy of an agent's configuration (ADR-0023 point 3,
//! HUM-518).
//!
//! Agents write their configuration directory all the time (Claude Code its
//! `.claude.json`), so a read-only bind would break them. The daemon copies
//! `config/` and the credential file into a scratch directory of the session
//! under the runtime directory, binds that writable as the agent's
//! configuration directory, and deletes it at the end of the session. The
//! scratch never lies under the depot: what a session writes stays in the
//! session (the guarded credential write-back is HUM-520).
//!
//! A scratch outlives its session only when the process that started it
//! dies; [`sweep_stale`] removes such a scratch the next time a daemon or a
//! local start comes by, judged by the process that wrote its owner file.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::Paths;
use humanitl_core::Diagnostic;
use humanitl_core::ids::SessionId;

use super::store::{Depot, Reservation, unusable};

/// The cap of what one session copies out of the depot.
pub const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;

/// A scratch directory that is removed when this value goes away, unless it
/// was handed to the sandbox ([`Scratch::hand_to`]).
#[derive(Debug)]
pub struct Scratch {
    dir: PathBuf,
    owner: PathBuf,
    armed: bool,
    /// The depot's reservation for this session; it goes with the sandbox.
    reservation: Option<Reservation>,
}

impl Scratch {
    /// The directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// The sandbox now owns the scratch: it goes with the sandbox's end.
    pub fn hand_to(mut self, handle: &crate::SandboxHandle) {
        handle.remove_at_exit(self.dir.clone());
        handle.remove_at_exit(self.owner.clone());
        if let Some(fd) = self
            .reservation
            .take()
            .and_then(super::store::Reservation::into_fd)
        {
            handle.hold_until_exit(fd);
        }
        self.armed = false;
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_dir_all(&self.dir);
            let _ = std::fs::remove_file(&self.owner);
        }
    }
}

/// Where the scratch of this session lies; nothing is created.
#[must_use]
pub fn scratch_path(paths: &Paths, session: SessionId) -> PathBuf {
    paths.agent_scratch_dir().join(session.to_string())
}

/// Copies `config/` and the files of `credentials/` of a checked depot into
/// a fresh scratch of this session.
///
/// # Errors
///
/// `AGENT_013` when the copy would exceed [`SCRATCH_CAP_BYTES`], an entry is
/// neither a file, a directory nor a link, or the scratch cannot be written.
pub fn create(
    paths: &Paths,
    session: SessionId,
    depot: &Depot,
    reservation: Option<Reservation>,
) -> Result<Scratch, Diagnostic> {
    let parent = paths.agent_scratch_dir();
    sweep_stale(&parent);
    make_private_dir(&parent).map_err(|err| failed(depot, &parent, &err))?;
    let dir = parent.join(session.to_string());
    let owner = parent.join(format!("{session}.pid"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::write(&owner, std::process::id().to_string())
        .map_err(|err| failed(depot, &owner, &err))?;
    let scratch = Scratch {
        dir: dir.clone(),
        owner,
        armed: true,
        reservation,
    };
    make_private_dir(&dir).map_err(|err| failed(depot, &dir, &err))?;
    let mut budget = SCRATCH_CAP_BYTES;
    if std::fs::symlink_metadata(depot.config_dir()).is_ok() {
        copy_tree(depot, &depot.config_dir(), &dir, &mut budget)?;
    }
    // The credential file lies beside the configuration, where the agent
    // looks for it (Claude Code: `$CLAUDE_CONFIG_DIR/.credentials.json`).
    if let Ok(entries) = std::fs::read_dir(depot.credentials_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                return Err(unusable(
                    depot.agent(),
                    &path,
                    "is not a regular file; credentials/ holds only the credential file",
                ));
            }
            copy_file(
                depot,
                &path,
                &dir.join(entry.file_name()),
                &meta,
                &mut budget,
            )?;
        }
    }
    Ok(scratch)
}

fn copy_tree(depot: &Depot, from: &Path, to: &Path, budget: &mut u64) -> Result<(), Diagnostic> {
    let entries = std::fs::read_dir(from).map_err(|err| failed(depot, from, &err))?;
    for entry in entries {
        let entry = entry.map_err(|err| failed(depot, from, &err))?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&path).map_err(|err| failed(depot, &path, &err))?;
        if meta.file_type().is_symlink() {
            // `Depot::check` admitted only links that stay inside `config/`
            // relative to it; the copy keeps them as links.
            let link = std::fs::read_link(&path).map_err(|err| failed(depot, &path, &err))?;
            std::os::unix::fs::symlink(&link, &target)
                .map_err(|err| failed(depot, &target, &err))?;
        } else if meta.is_dir() {
            make_private_dir(&target).map_err(|err| failed(depot, &target, &err))?;
            copy_tree(depot, &path, &target, budget)?;
        } else if meta.is_file() {
            copy_file(depot, &path, &target, &meta, budget)?;
        } else {
            return Err(unusable(
                depot.agent(),
                &path,
                "is neither a regular file, a directory nor a link",
            ));
        }
    }
    Ok(())
}

fn copy_file(
    depot: &Depot,
    from: &Path,
    to: &Path,
    meta: &std::fs::Metadata,
    budget: &mut u64,
) -> Result<(), Diagnostic> {
    if meta.len() > *budget {
        return Err(unusable(
            depot.agent(),
            from,
            &format!(
                "would take the copy for the session over its cap of {} MiB",
                SCRATCH_CAP_BYTES / (1024 * 1024)
            ),
        ));
    }
    *budget -= meta.len();
    std::fs::copy(from, to).map_err(|err| failed(depot, to, &err))?;
    // The agent's own file, readable and writable for the session alone.
    let mode = if meta.permissions().mode() & 0o100 == 0 {
        0o600
    } else {
        0o700
    };
    std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode))
        .map_err(|err| failed(depot, to, &err))
}

fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

fn failed(depot: &Depot, path: &Path, err: &std::io::Error) -> Diagnostic {
    unusable(
        depot.agent(),
        path,
        &format!("cannot be copied into the session ({err})"),
    )
}

/// Removes every scratch whose owner process has ended: after a crash of the
/// daemon or of a local start, the next one cleans up.
pub fn sweep_stale(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.extension().is_some_and(|ext| ext == "pid") {
            continue;
        }
        let owner = parent.join(format!("{name}.pid"));
        let alive = std::fs::read_to_string(&owner)
            .ok()
            .and_then(|text| text.trim().parse::<i32>().ok())
            .and_then(rustix::process::Pid::from_raw)
            .is_some_and(|pid| {
                // `EPERM` still means: the process exists.
                !matches!(
                    rustix::process::test_kill_process(pid),
                    Err(rustix::io::Errno::SRCH)
                )
            });
        if !alive {
            let _ = std::fs::remove_dir_all(&path);
            let _ = std::fs::remove_file(&owner);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes::AGENT_013;

    use super::*;
    use crate::agent::depot::store::tests::filled;

    fn paths(dir: &Path) -> Paths {
        Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.join("home").to_str().unwrap()),
            ("XDG_RUNTIME_DIR", dir.join("run").to_str().unwrap()),
            ("XDG_DATA_HOME", dir.join("data").to_str().unwrap()),
        ]))
    }

    #[test]
    fn the_copy_is_writable_and_goes_away_with_its_value() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        std::fs::write(depot.credentials_dir().join(".credentials.json"), b"{}").unwrap();
        let paths = paths(base.path());
        let session = SessionId::new();
        let scratch = create(&paths, session, &depot, None).unwrap();
        assert!(scratch.path().starts_with(paths.agent_scratch_dir()));
        assert!(!scratch.path().starts_with(depot.root()));
        assert!(scratch.path().join("settings.json").is_file());
        assert!(scratch.path().join(".credentials.json").is_file());
        // What the session writes stays in the copy.
        std::fs::write(scratch.path().join("settings.json"), b"{\"hooks\":1}").unwrap();
        std::fs::write(scratch.path().join("new"), b"x").unwrap();
        assert_eq!(
            std::fs::read(depot.config_dir().join("settings.json")).unwrap(),
            b"{}\n"
        );
        assert!(!depot.config_dir().join("new").exists());
        let dir = scratch.path().to_path_buf();
        drop(scratch);
        assert!(!dir.exists());
    }

    #[test]
    fn a_copy_over_the_cap_is_unusable() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        let big = std::fs::File::create(depot.config_dir().join("big")).unwrap();
        big.set_len(SCRATCH_CAP_BYTES + 1).unwrap();
        let found = create(&paths(base.path()), SessionId::new(), &depot, None).unwrap_err();
        assert_eq!(found.code, AGENT_013);
        assert!(found.why.contains("64 MiB"), "{}", found.why);
    }

    #[test]
    fn a_scratch_of_an_ended_process_is_swept() {
        let base = tempfile::tempdir().unwrap();
        let parent = base.path().join("agent-scratch");
        std::fs::create_dir_all(parent.join("dead")).unwrap();
        std::fs::write(parent.join("dead.pid"), "999999999").unwrap();
        std::fs::create_dir_all(parent.join("alive")).unwrap();
        std::fs::write(parent.join("alive.pid"), std::process::id().to_string()).unwrap();
        std::fs::create_dir_all(parent.join("orphan")).unwrap();
        sweep_stale(&parent);
        assert!(!parent.join("dead").exists());
        assert!(!parent.join("dead.pid").exists());
        assert!(!parent.join("orphan").exists());
        assert!(parent.join("alive").exists());
    }
}
