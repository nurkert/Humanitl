//! The check of a version directory right before it is bound (HUM-518).
//!
//! A version directory is the second mount kind of the daemon next to
//! [`crate::agent::AgentProgram`]: a directory instead of one file. Before
//! every bind, each entry passes the trust rules of `locate.rs`
//! ([`crate::agent::locate::Trust`]): owned by the user or root, no write
//! permission for others (group only for the user's private group), and a
//! symbolic link only relative and inside the tree. Sockets, FIFOs, devices
//! and set-user-ID or set-group-ID bits are refused as well.
//!
//! The walk starts at the opened descriptor's place (`/proc/self/fd/N` under
//! Linux), so it checks the directory that is bound and not whatever its path
//! names a moment later.

use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use humanitl_core::Diagnostic;

use crate::agent::locate::Trust;

use super::store::{link_stays_inside, unusable};

/// The most entries a version directory may hold; a bound tree larger than
/// that is refused rather than walked for minutes before a start.
const MAX_ENTRIES: usize = 200_000;

/// Checks every entry under `dir` with the trust rules.
///
/// `dir` is where the walk reads (the opened descriptor's place); `shown` is
/// the path the finding names.
///
/// # Errors
///
/// `AGENT_013` with the first entry that fails.
pub(crate) fn check_version_dir(
    agent: &str,
    dir: &Path,
    shown: &Path,
    trust: &Trust,
) -> Result<(), Diagnostic> {
    let meta = std::fs::metadata(dir)
        .map_err(|err| unusable(agent, shown, &format!("cannot be read ({err})")))?;
    if !trust.admits(&meta) {
        return Err(unusable(
            agent,
            shown,
            "can be changed by another user (owner or write permission)",
        ));
    }
    let mut seen = 0_usize;
    walk(agent, dir, dir, shown, trust, &mut seen)
}

fn walk(
    agent: &str,
    root: &Path,
    dir: &Path,
    shown_root: &Path,
    trust: &Trust,
    seen: &mut usize,
) -> Result<(), Diagnostic> {
    let shown = |path: &Path| {
        path.strip_prefix(root)
            .map_or_else(|_| path.to_path_buf(), |rest| shown_root.join(rest))
    };
    let entries = std::fs::read_dir(dir)
        .map_err(|err| unusable(agent, &shown(dir), &format!("cannot be listed ({err})")))?;
    for entry in entries {
        let entry = entry
            .map_err(|err| unusable(agent, &shown(dir), &format!("cannot be listed ({err})")))?;
        *seen += 1;
        if *seen > MAX_ENTRIES {
            return Err(unusable(
                agent,
                shown_root,
                &format!("holds more than {MAX_ENTRIES} entries"),
            ));
        }
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|err| unusable(agent, &shown(&path), &format!("cannot be read ({err})")))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let target = std::fs::read_link(&path).map_err(|err| {
                unusable(agent, &shown(&path), &format!("cannot be read ({err})"))
            })?;
            if !link_stays_inside(root, &path, &target) {
                return Err(unusable(
                    agent,
                    &shown(&path),
                    &format!(
                        "is a symbolic link out of the version directory, to {}",
                        target.display()
                    ),
                ));
            }
            continue;
        }
        if !kind.is_dir() && !kind.is_file() {
            return Err(unusable(
                agent,
                &shown(&path),
                "is a socket, FIFO or device; a version directory holds only files, directories \
                 and links inside it",
            ));
        }
        // A second name of the same file elsewhere (a hard link from a
        // project the agent writes) could change the program for the next
        // session past every check here (HUM-518, review).
        if kind.is_file() && meta.nlink() > 1 {
            return Err(unusable(
                agent,
                &shown(&path),
                &format!(
                    "has {} names (hard links); a file of a version directory has only its own",
                    meta.nlink()
                ),
            ));
        }
        if meta.permissions().mode() & 0o6000 != 0 {
            return Err(unusable(
                agent,
                &shown(&path),
                "carries a set-user-ID or set-group-ID bit",
            ));
        }
        if !trust.admits(&meta) {
            return Err(unusable(
                agent,
                &shown(&path),
                &format!(
                    "can be changed by another user (owner {}, mode {:04o})",
                    meta.uid(),
                    meta.mode() & 0o7777
                ),
            ));
        }
        if kind.is_dir() {
            walk(agent, root, &path, shown_root, trust, seen)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::PermissionsExt as _;

    use humanitl_core::diagnostics::codes::AGENT_013;

    use super::*;

    fn version() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(dir.path().join("claude"), b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(
            dir.path().join("claude"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        dir
    }

    fn trust() -> Trust {
        Trust::new(rustix::process::getuid().as_raw(), None)
    }

    #[test]
    fn a_tree_of_the_user_passes() {
        let dir = version();
        std::fs::create_dir(dir.path().join("lib")).unwrap();
        std::fs::set_permissions(
            dir.path().join("lib"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::os::unix::fs::symlink("../claude", dir.path().join("lib/link")).unwrap();
        check_version_dir("claude", dir.path(), dir.path(), &trust()).unwrap();
    }

    #[test]
    fn a_world_writable_entry_a_fifo_and_a_link_out_are_refused() {
        let dir = version();
        std::fs::set_permissions(
            dir.path().join("claude"),
            std::fs::Permissions::from_mode(0o702),
        )
        .unwrap();
        let found = check_version_dir("claude", dir.path(), dir.path(), &trust()).unwrap_err();
        assert_eq!(found.code, AGENT_013);

        let dir = version();
        let made = std::process::Command::new("mkfifo")
            .arg(dir.path().join("fifo"))
            .status()
            .unwrap();
        assert!(made.success());
        let found = check_version_dir("claude", dir.path(), dir.path(), &trust()).unwrap_err();
        assert!(found.why.contains("FIFO"), "{}", found.why);

        let dir = version();
        std::os::unix::fs::symlink("/etc", dir.path().join("etc")).unwrap();
        let found = check_version_dir("claude", dir.path(), dir.path(), &trust()).unwrap_err();
        assert!(
            found.why.contains("out of the version directory"),
            "{}",
            found.why
        );
    }

    /// Mutation proof: without the check of `st_nlink` this turns red.
    #[test]
    fn a_hard_link_into_the_version_directory_is_refused() {
        let dir = version();
        let outside = tempfile::tempdir().unwrap();
        std::fs::hard_link(dir.path().join("claude"), outside.path().join("planted")).unwrap();
        let found = check_version_dir("claude", dir.path(), dir.path(), &trust()).unwrap_err();
        assert_eq!(found.code, AGENT_013);
        assert!(found.why.contains("hard links"), "{}", found.why);
    }

    #[test]
    fn a_set_user_id_file_is_refused() {
        let dir = version();
        std::fs::set_permissions(
            dir.path().join("claude"),
            std::fs::Permissions::from_mode(0o4700),
        )
        .unwrap();
        let found = check_version_dir("claude", dir.path(), dir.path(), &trust()).unwrap_err();
        assert!(found.why.contains("set-user-ID"), "{}", found.why);
    }
}
