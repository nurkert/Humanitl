//! The depot of one agent on disk (ADR-0023 point 2, HUM-518).
//!
//! ```text
//! $XDG_DATA_HOME/humanitl/agents/<id>/     0700
//!     program/<version>/                   0700, the agent's files as the
//!         manifest.json                    install left them, plus the manifest
//!     current -> program/<version>         a relative link, switched atomically
//!     config/                              0700, the agent's configuration
//!     credentials/                         0700, the one credential file, 0600
//! ```
//!
//! The daemon decides this layout, never the agent. [`Depot::check`] checks it
//! each time: every directory and file is owned by the daemon's user and
//! carries no permission for group or others, and no symbolic link leads out
//! of the depot. A version directory is checked a second time, entry by entry
//! with the trust rules of `locate.rs`, right before it is bound
//! (`tree::check_version_dir`).
//!
//! Nothing here executes anything from the depot. The version comes from
//! `manifest.json`, never from `--version` (ADR-0023 point 2).

use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::{AGENT_008, AGENT_013, AGENT_018};
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde::{Deserialize, Serialize};

/// The file the daemon writes into each version directory.
pub const MANIFEST_FILE: &str = "manifest.json";

/// The link at the depot's root that names the installed version.
pub const CURRENT_LINK: &str = "current";

/// How often a lock of the depot is tried, 10 ms apart, before it counts as
/// held: a lock just released can stay held for a moment by a child process
/// another thread of the daemon forked and that has not reached its `exec`
/// yet; the descriptor is `CLOEXEC`, so the window is short (HUM-519, seen
/// in parallel tests, for shared and exclusive locks alike). The cost: an
/// answer "in use" or "being removed" comes at most 70 ms later than it
/// could; a lock that is free is taken at the first try.
const LOCK_TRIES: u32 = 8;

/// The largest `manifest.json` the daemon reads.
const MANIFEST_CAP: u64 = 1024 * 1024;

/// What the daemon wrote about one installed version (HUM-519 writes it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The version, as the vendor names it.
    pub version: String,
    /// Where the install came from, a URL.
    #[serde(default)]
    pub source: String,
    /// SHA-256 of every file, by its path relative to the version directory.
    #[serde(default)]
    pub files: std::collections::BTreeMap<String, String>,
    /// The result of the signature check where the vendor signs: `verified`,
    /// `unsigned` where the vendor publishes no signature.
    #[serde(default)]
    pub signature: String,
    /// The fingerprint of the key whose signature was checked (HUM-519);
    /// empty when unsigned.
    #[serde(default)]
    pub signed_by: String,
    /// When the install was committed, RFC 3339.
    #[serde(default)]
    pub installed_at: String,
}

/// A held reservation of a depot ([`Depot::reserve_for_session`]); the lock
/// goes with this value or with the sandbox it was handed to.
///
/// Dropping it unlocks explicitly (`flock(LOCK_UN)`) before the descriptor
/// closes: a child process another thread forked meanwhile holds a copy of
/// the descriptor until its `exec`, and without the unlock the lock would
/// live on in that copy for that moment (HUM-519, seen in CI).
#[derive(Debug)]
pub struct Reservation(Option<std::os::fd::OwnedFd>);

impl Reservation {
    /// The descriptor that holds the lock; the lock then goes with it.
    /// Always `Some` for a reservation [`Depot`] made.
    #[must_use]
    pub fn into_fd(mut self) -> Option<std::os::fd::OwnedFd> {
        self.0.take()
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if let Some(fd) = self.0.take() {
            let _ = rustix::fs::flock(&fd, rustix::fs::FlockOperation::Unlock);
        }
    }
}

/// An installed version, resolved from `current`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// The version directory's name.
    pub version: String,
    /// `program/<version>/`, never the `current` link.
    pub dir: PathBuf,
    /// Its manifest.
    pub manifest: Manifest,
}

/// The depot of one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Depot {
    agent: String,
    root: PathBuf,
    uid: u32,
}

impl Depot {
    /// The depot of `agent` under the daemon's data directory, owned by the
    /// user this process runs as.
    ///
    /// The data directory is resolved once: a `$XDG_DATA_HOME` behind a
    /// symbolic link is a setup of the person, and processes of the same
    /// user are outside what the depot defends against (K-36). From the
    /// resolved place on, [`Depot::check`] follows no link: neither `agents/`
    /// nor `agents/<id>/` nor anything below may be one.
    #[must_use]
    pub fn of(paths: &Paths, agent: &str) -> Self {
        let data = std::fs::canonicalize(paths.data_dir()).unwrap_or_else(|_| paths.data_dir());
        Self::at(
            data.join("agents").join(agent),
            agent,
            rustix::process::getuid().as_raw(),
        )
    }

    /// A depot at an explicit place and owner; for tests.
    #[must_use]
    pub fn at(root: PathBuf, agent: &str, uid: u32) -> Self {
        Self {
            agent: agent.to_owned(),
            root,
            uid,
        }
    }

    /// The agent's identifier.
    #[must_use]
    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// `agents/<id>/`.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `program/`.
    #[must_use]
    pub fn program_dir(&self) -> PathBuf {
        self.root.join("program")
    }

    /// `config/`.
    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    /// `credentials/`.
    #[must_use]
    pub fn credentials_dir(&self) -> PathBuf {
        self.root.join("credentials")
    }

    /// Whether anything lies at the depot's place.
    #[must_use]
    pub fn exists(&self) -> bool {
        std::fs::symlink_metadata(&self.root).is_ok()
    }

    /// Checks owner, modes and links of the whole depot (ADR-0023 point 2).
    ///
    /// # Errors
    ///
    /// `AGENT_013` with the first path that fails.
    pub fn check(&self) -> Result<(), Diagnostic> {
        let parent = self.root.parent().unwrap_or(&self.root);
        for dir in [parent, self.root.as_path()] {
            self.check_entry(dir, true)?;
        }
        for dir in [
            self.program_dir(),
            self.config_dir(),
            self.credentials_dir(),
        ] {
            if std::fs::symlink_metadata(&dir).is_ok() {
                self.check_entry(&dir, true)?;
                self.walk(&dir)?;
            }
        }
        let current = self.root.join(CURRENT_LINK);
        if let Ok(meta) = std::fs::symlink_metadata(&current) {
            if !meta.file_type().is_symlink() {
                return Err(self.unusable(&current, "is not a symbolic link"));
            }
            self.check_owner(&current, &meta)?;
            self.current_target()?;
        }
        Ok(())
    }

    /// The installed version, from `current` and its manifest.
    ///
    /// # Errors
    ///
    /// `AGENT_008` without an installed version, `AGENT_013` for a depot or
    /// manifest that fails its checks.
    pub fn installed(&self) -> Result<Installed, Diagnostic> {
        if !self.exists() {
            return Err(self.not_installed("the depot does not exist"));
        }
        self.check()?;
        if std::fs::symlink_metadata(self.root.join(CURRENT_LINK)).is_err() {
            return Err(self.not_installed("the depot names no current version"));
        }
        let version = self.current_target()?;
        let dir = self.program_dir().join(&version);
        let meta = std::fs::symlink_metadata(&dir)
            .map_err(|_| self.not_installed("the current version has no directory"))?;
        if !meta.is_dir() {
            return Err(self.unusable(&dir, "is not a directory"));
        }
        let manifest = self.manifest_of(&version)?;
        Ok(Installed {
            version,
            dir,
            manifest,
        })
    }

    /// The manifest of `program/<version>/`, checked to name that version.
    fn manifest_of(&self, version: &str) -> Result<Manifest, Diagnostic> {
        let manifest_path = self.program_dir().join(version).join(MANIFEST_FILE);
        let manifest_meta = std::fs::symlink_metadata(&manifest_path)
            .map_err(|_| self.not_installed(&format!("version {version} has no manifest.json")))?;
        if !manifest_meta.is_file() || manifest_meta.len() > MANIFEST_CAP {
            return Err(self.unusable(&manifest_path, "is not a regular file of at most 1 MiB"));
        }
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|err| self.unusable(&manifest_path, &format!("cannot be read ({err})")))?;
        let manifest: Manifest = serde_json::from_str(&text)
            .map_err(|err| self.unusable(&manifest_path, &format!("is not a manifest ({err})")))?;
        if manifest.version != version {
            return Err(self.unusable(
                &manifest_path,
                &format!(
                    "names version {:?}, and it lies in the directory of version {version:?}",
                    manifest.version
                ),
            ));
        }
        Ok(manifest)
    }

    /// Whether `credentials/` holds a credential file.
    #[must_use]
    pub fn logged_in(&self) -> bool {
        std::fs::read_dir(self.credentials_dir()).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                std::fs::symlink_metadata(entry.path()).is_ok_and(|meta| meta.is_file())
            })
        })
    }

    /// The versions under `program/` an update switches to without a run
    /// (HUM-521): directories with a valid name and a manifest that names
    /// them, newest first. Links and anything else are left out; a depot
    /// that fails [`Depot::check`] holds none.
    #[must_use]
    pub fn versions(&self) -> Vec<String> {
        if self.check().is_err() {
            return Vec::new();
        }
        let Ok(entries) = std::fs::read_dir(self.program_dir()) else {
            return Vec::new();
        };
        let mut versions: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| version_name_ok(name))
            .filter(|name| {
                std::fs::symlink_metadata(self.program_dir().join(name))
                    .is_ok_and(|meta| meta.is_dir())
            })
            .filter(|name| self.manifest_of(name).is_ok())
            .collect();
        versions.sort_by(|a, b| newer_first(a, b));
        versions
    }

    /// The bytes of every regular file in the depot; links are not followed.
    #[must_use]
    pub fn size(&self) -> u64 {
        tree_size(&self.root)
    }

    /// Removes the whole depot. Links inside are removed, never followed.
    ///
    /// # Errors
    ///
    /// `AGENT_013` when the depot's place is a symbolic link or cannot be
    /// removed.
    pub fn remove(&self) -> Result<(), Diagnostic> {
        let Ok(meta) = std::fs::symlink_metadata(&self.root) else {
            return Ok(());
        };
        // Held across the removal: no start reserves the depot meanwhile.
        let _reservation = self.reserve(true)?;
        if !meta.is_dir() {
            return Err(self.unusable(&self.root, "is not a directory"));
        }
        std::fs::remove_dir_all(&self.root)
            .map_err(|err| self.unusable(&self.root, &format!("cannot be removed ({err})")))
    }

    /// The reservation of a start (HUM-518): a shared lock on
    /// `agents/<id>.lock`, taken before the depot is read and held until the
    /// sandbox ends ([`super::Scratch::hand_to`]). [`Depot::remove`] takes it
    /// exclusively, so a removal and a start never overlap, also across a
    /// daemon and a local `humanitl sandbox run`.
    ///
    /// # Errors
    ///
    /// `AGENT_018` while a removal holds the depot; `AGENT_013` when the lock
    /// file cannot be opened.
    pub fn reserve_for_session(&self) -> Result<Reservation, Diagnostic> {
        self.reserve(false)
    }

    /// Whether a session holds this depot now.
    #[must_use]
    pub fn in_use(&self) -> bool {
        std::fs::symlink_metadata(self.lock_path()).is_ok() && self.reserve(true).is_err()
    }

    /// The version `current` names, without any other check; `None` without
    /// a valid link.
    pub(crate) fn current_version(&self) -> Option<String> {
        self.current_target().ok()
    }

    fn lock_path(&self) -> PathBuf {
        let parent = self.root.parent().unwrap_or(&self.root);
        parent.join(format!("{}.lock", self.agent))
    }

    /// An exclusive lock on `agents/<id>.<name>.lock`, without waiting
    /// (HUM-519: one maintenance run per agent).
    ///
    /// # Errors
    ///
    /// `AGENT_018` while another holds it, `AGENT_013` when the lock file
    /// cannot be opened.
    pub(crate) fn lock_named(&self, name: &str) -> Result<Reservation, Diagnostic> {
        let parent = self.root.parent().unwrap_or(&self.root);
        self.lock_at(
            &parent.join(format!("{}.{name}.lock", self.agent)),
            true,
            LOCK_TRIES,
        )
    }

    /// The depot's reservation; shared for a session or a takeover,
    /// exclusive for a removal or a pruning ([`LOCK_TRIES`]).
    pub(crate) fn reserve(&self, exclusive: bool) -> Result<Reservation, Diagnostic> {
        self.lock_at(&self.lock_path(), exclusive, LOCK_TRIES)
    }

    fn lock_at(&self, path: &Path, exclusive: bool, tries: u32) -> Result<Reservation, Diagnostic> {
        use rustix::fs::{FlockOperation, Mode, OFlags};
        let path = path.to_path_buf();
        if let Some(parent) = path.parent() {
            // `agents/` on a first use: created with its parents synced, so a
            // crash cannot lose the entry of a depot made right after.
            super::adopt::create_synced(parent)
                .map_err(|err| self.unusable(parent, &format!("cannot be created ({err})")))?;
        }
        let fd = rustix::fs::open(
            &path,
            OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|err| self.unusable(&path, &format!("cannot be opened ({err})")))?;
        let operation = if exclusive {
            FlockOperation::NonBlockingLockExclusive
        } else {
            FlockOperation::NonBlockingLockShared
        };
        let mut taken = rustix::fs::flock(&fd, operation);
        for _ in 1..tries {
            if taken.is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            taken = rustix::fs::flock(&fd, operation);
        }
        taken.map_err(|_| {
            Diagnostic::builder(AGENT_018, Severity::Error)
                .why(if exclusive {
                    format!(
                        "a session uses the depot of {}; it is not removed",
                        self.agent
                    )
                } else {
                    format!(
                        "the depot of {} is being removed; the session does not start",
                        self.agent
                    )
                })
                .fix(FixAction::CopyCommand("humanitl agents status".to_owned()))
                .build()
        })?;
        Ok(Reservation(Some(fd)))
    }

    /// The version `current` names: a relative link `program/<version>`.
    fn current_target(&self) -> Result<String, Diagnostic> {
        let current = self.root.join(CURRENT_LINK);
        let target = std::fs::read_link(&current)
            .map_err(|err| self.unusable(&current, &format!("cannot be read ({err})")))?;
        let parts: Vec<Component<'_>> = target.components().collect();
        match parts.as_slice() {
            [Component::Normal(program), Component::Normal(version)] if *program == "program" => {
                version
                    .to_str()
                    .filter(|name| version_name_ok(name))
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| self.unusable(&current, "names no valid version"))
            }
            _ => Err(self.unusable(
                &current,
                &format!(
                    "points at {}, and it may only point at program/<version>",
                    target.display()
                ),
            )),
        }
    }

    /// Every entry under `dir`: owned by the user, no permission for group or
    /// others, links only relative and inside the depot.
    fn walk(&self, dir: &Path) -> Result<(), Diagnostic> {
        let entries = std::fs::read_dir(dir)
            .map_err(|err| self.unusable(dir, &format!("cannot be listed ({err})")))?;
        for entry in entries {
            let entry =
                entry.map_err(|err| self.unusable(dir, &format!("cannot be listed ({err})")))?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path)
                .map_err(|err| self.unusable(&path, &format!("cannot be read ({err})")))?;
            if meta.file_type().is_symlink() {
                self.check_owner(&path, &meta)?;
                let target = std::fs::read_link(&path)
                    .map_err(|err| self.unusable(&path, &format!("cannot be read ({err})")))?;
                if !link_stays_inside(&self.root, &path, &target) {
                    return Err(self.unusable(
                        &path,
                        &format!(
                            "is a symbolic link out of the depot, to {}",
                            target.display()
                        ),
                    ));
                }
                continue;
            }
            self.check_entry(&path, meta.is_dir())?;
            if meta.is_file() && meta.nlink() > 1 {
                return Err(self.unusable(&path, "has more than one name (a hard link)"));
            }
            if meta.is_dir() {
                self.walk(&path)?;
            }
        }
        Ok(())
    }

    fn check_entry(&self, path: &Path, dir: bool) -> Result<(), Diagnostic> {
        let meta = std::fs::symlink_metadata(path)
            .map_err(|err| self.unusable(path, &format!("cannot be read ({err})")))?;
        if meta.file_type().is_symlink() {
            return Err(self.unusable(path, "is a symbolic link"));
        }
        if dir && !meta.is_dir() {
            return Err(self.unusable(path, "is not a directory"));
        }
        if !dir && !meta.is_file() {
            return Err(self.unusable(path, "is neither a regular file nor a directory"));
        }
        self.check_owner(path, &meta)?;
        let mode = meta.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(self.unusable(
                path,
                &format!(
                    "has the mode {:04o}; the depot keeps every entry for its owner alone \
                     (0700 or 0600)",
                    mode & 0o7777
                ),
            ));
        }
        if mode & 0o6000 != 0 {
            return Err(self.unusable(path, "carries a set-user-ID or set-group-ID bit"));
        }
        Ok(())
    }

    fn check_owner(&self, path: &Path, meta: &std::fs::Metadata) -> Result<(), Diagnostic> {
        if meta.uid() != self.uid {
            return Err(self.unusable(
                path,
                &format!(
                    "is owned by user {}, and the depot belongs to user {}",
                    meta.uid(),
                    self.uid
                ),
            ));
        }
        Ok(())
    }

    fn unusable(&self, path: &Path, what: &str) -> Diagnostic {
        unusable(&self.agent, path, what)
    }

    fn not_installed(&self, what: &str) -> Diagnostic {
        Diagnostic::builder(AGENT_008, Severity::Blocking)
            .why(format!(
                "the source of {} is the depot, and {what} ({}); nothing of the host is started \
                 instead",
                self.agent,
                self.root.display()
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl agents install {}",
                self.agent
            )))
            .build()
    }
}

/// The manifest of `program/<version>/` of `depot`, checked like the one of
/// the current version.
///
/// # Errors
///
/// `AGENT_008` without the version's manifest, `AGENT_013` for one that fails
/// its checks or a depot that does.
pub(crate) fn read_manifest(depot: &Depot, version: &str) -> Result<Manifest, Diagnostic> {
    if !version_name_ok(version) {
        return Err(depot.unusable(
            &depot.program_dir(),
            &format!("holds no version {version:?}"),
        ));
    }
    depot.check()?;
    depot.manifest_of(version)
}

/// `AGENT_013` for this agent and path.
pub(crate) fn unusable(agent: &str, path: &Path, what: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_013, Severity::Blocking)
        .why(format!(
            "the depot of {agent} is not usable: {} {what}",
            path.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents remove {agent}"
        )))
        .build()
}

/// `$XDG_DATA_HOME/humanitl/agents`.
#[must_use]
pub fn agents_dir(paths: &Paths) -> PathBuf {
    paths.agents_dir()
}

/// A version directory's name: letters, digits, `.`, `-`, `_`, `+`, not
/// starting with a dot.
#[must_use]
pub fn version_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

/// Orders two version names newest first: part by part along `.`, `-`, `_`
/// and `+`, numbers as numbers, a number before a word, and on a tie the
/// longer name first (`1.2.0` before `1.2`). The order of the list only; no
/// decision rests on it.
fn newer_first(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let split = |name: &str| -> Vec<String> {
        name.split(['.', '-', '_', '+'])
            .map(ToOwned::to_owned)
            .collect()
    };
    let (left, right) = (split(a), split(b));
    for (x, y) in left.iter().zip(right.iter()) {
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            (Ok(_), Err(_)) => Ordering::Greater,
            (Err(_), Ok(_)) => Ordering::Less,
            (Err(_), Err(_)) => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order.reverse();
        }
    }
    right.len().cmp(&left.len()).then_with(|| b.cmp(a))
}

/// Whether the relative link at `link` with this target stays under `root`.
/// An absolute target never does: the sandbox sees the tree elsewhere.
pub(crate) fn link_stays_inside(root: &Path, link: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let Some(parent) = link.parent() else {
        return false;
    };
    let Ok(start) = parent.strip_prefix(root) else {
        return false;
    };
    let mut depth: Vec<&std::ffi::OsStr> = start
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    // `..` only before the first name: after a name, that name may itself be
    // a link, and `..` would then climb from wherever the link leads, which
    // a lexical check cannot see (`up -> ..`, `out -> up/..`; HUM-519,
    // review). With `..` only at the start, every link lands inside by
    // itself, and so does every chain of them.
    let mut named = false;
    for part in target.components() {
        match part {
            Component::Normal(name) => {
                named = true;
                depth.push(name);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if named || depth.pop().is_none() {
                    return false;
                }
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// The bytes of the regular files under `dir`, links not followed.
pub(crate) fn tree_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match std::fs::symlink_metadata(entry.path()) {
            Ok(meta) if meta.is_dir() => tree_size(&entry.path()),
            Ok(meta) if meta.is_file() => meta.len(),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    pub(crate) fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn private_dir(path: &Path) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    /// A depot filled by hand, as the tests of HUM-518 do until HUM-519
    /// installs: version `1.0.0` with one program that writes `marker` next to
    /// the depot when it ever runs on the host.
    pub(crate) fn filled(base: &Path, agent: &str, program: &str) -> Depot {
        let agents = base.join("agents");
        private_dir(&agents);
        let depot = Depot::at(agents.join(agent), agent, uid());
        let version = depot.program_dir().join("1.0.0");
        for dir in [
            depot.root().to_path_buf(),
            depot.program_dir(),
            version.clone(),
            depot.config_dir(),
            depot.credentials_dir(),
        ] {
            private_dir(&dir);
        }
        let marker = base.join("marker");
        let script = format!(
            "#!/bin/sh\ntouch {}\necho depot-program \"$@\"\n",
            marker.display()
        );
        std::fs::write(version.join(program), script).unwrap();
        std::fs::set_permissions(
            version.join(program),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let manifest = Manifest {
            version: "1.0.0".to_owned(),
            source: "https://example.invalid/".to_owned(),
            files: std::collections::BTreeMap::new(),
            signature: "unsigned".to_owned(),
            signed_by: String::new(),
            installed_at: "2026-11-17T00:00:00Z".to_owned(),
        };
        std::fs::write(
            version.join(MANIFEST_FILE),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::set_permissions(
            version.join(MANIFEST_FILE),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::fs::write(depot.config_dir().join("settings.json"), b"{}\n").unwrap();
        std::fs::set_permissions(
            depot.config_dir().join("settings.json"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::os::unix::fs::symlink("program/1.0.0", depot.root().join(CURRENT_LINK)).unwrap();
        depot
    }

    #[test]
    fn a_filled_depot_names_its_version_from_the_manifest() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        let installed = depot.installed().unwrap();
        assert_eq!(installed.version, "1.0.0");
        assert_eq!(installed.dir, depot.program_dir().join("1.0.0"));
        assert_eq!(installed.manifest.signature, "unsigned");
        assert!(!depot.logged_in());
        assert!(depot.size() > 0);
    }

    #[test]
    fn versions_lists_every_held_version_newest_first() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        for version in ["1.10.0", "1.9.2"] {
            let dir = depot.program_dir().join(version);
            private_dir(&dir);
            let manifest = format!("{{\"version\":\"{version}\"}}");
            std::fs::write(dir.join(MANIFEST_FILE), manifest).unwrap();
            std::fs::set_permissions(
                dir.join(MANIFEST_FILE),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        // Ohne Manifest oder mit falschem Namen: keine Version.
        private_dir(&depot.program_dir().join("2.0.0"));
        private_dir(&depot.program_dir().join(".committing"));
        assert_eq!(depot.versions(), vec!["1.10.0", "1.9.2", "1.0.0"]);
        let missing = Depot::at(base.path().join("agents/codex"), "codex", uid());
        assert!(missing.versions().is_empty());
    }

    #[test]
    fn newer_first_orders_numbers_as_numbers() {
        let mut names = vec!["1.2", "1.10.0", "1.2.0", "1.2.0-beta", "0.9"];
        names.sort_by(|a, b| newer_first(a, b));
        assert_eq!(names, vec!["1.10.0", "1.2.0-beta", "1.2.0", "1.2", "0.9"]);
    }

    #[test]
    fn without_current_the_agent_is_not_installed() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        std::fs::remove_file(depot.root().join(CURRENT_LINK)).unwrap();
        assert_eq!(depot.installed().unwrap_err().code, AGENT_008);
        let missing = Depot::at(base.path().join("agents/codex"), "codex", uid());
        let found = missing.installed().unwrap_err();
        assert_eq!(found.code, AGENT_008);
        assert!(
            matches!(found.fix, Some(FixAction::CopyCommand(ref line)) if line == "humanitl agents install codex")
        );
    }

    /// `0755` on a directory of the depot is `AGENT_013` (HUM-518, tests).
    #[test]
    fn a_mode_for_others_is_unusable() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        std::fs::set_permissions(depot.config_dir(), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let found = depot.installed().unwrap_err();
        assert_eq!(found.code, AGENT_013);
        assert!(found.why.contains("0755"), "{}", found.why);
    }

    /// A foreign owner is `AGENT_013`; the test changes the expected owner,
    /// since a test cannot give a file away.
    #[test]
    fn a_foreign_owner_is_unusable() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        let foreign = Depot::at(depot.root().to_path_buf(), "claude", uid() + 1);
        let found = foreign.installed().unwrap_err();
        assert_eq!(found.code, AGENT_013);
        assert!(found.why.contains("owned by user"), "{}", found.why);
    }

    /// A link to `/home` is `AGENT_013`.
    #[test]
    fn a_link_out_of_the_depot_is_unusable() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        std::os::unix::fs::symlink("/home", depot.config_dir().join("home")).unwrap();
        let found = depot.installed().unwrap_err();
        assert_eq!(found.code, AGENT_013);
        assert!(found.why.contains("out of the depot"), "{}", found.why);
        std::fs::remove_file(depot.config_dir().join("home")).unwrap();
        std::os::unix::fs::symlink("../../../..", depot.config_dir().join("up")).unwrap();
        assert_eq!(depot.installed().unwrap_err().code, AGENT_013);
    }

    #[test]
    fn current_may_only_name_a_version_under_program() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        std::fs::remove_file(depot.root().join(CURRENT_LINK)).unwrap();
        std::os::unix::fs::symlink("/usr/bin", depot.root().join(CURRENT_LINK)).unwrap();
        assert_eq!(depot.installed().unwrap_err().code, AGENT_013);
    }

    #[test]
    fn a_relative_link_inside_stays_and_removal_never_follows_a_link() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        let version = depot.program_dir().join("1.0.0");
        std::os::unix::fs::symlink("claude", version.join("alias")).unwrap();
        assert!(depot.installed().is_ok());
        let outside = base.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("keep"), b"x").unwrap();
        // A link planted after the check is removed as a link.
        std::os::unix::fs::symlink(&outside, depot.config_dir().join("planted")).unwrap();
        depot.remove().unwrap();
        assert!(!depot.exists());
        assert!(outside.join("keep").exists());
    }

    #[test]
    fn links_are_judged_against_the_root() {
        let root = Path::new("/d");
        assert!(link_stays_inside(root, Path::new("/d/a/b"), Path::new("c")));
        assert!(link_stays_inside(
            root,
            Path::new("/d/a/b"),
            Path::new("../c")
        ));
        assert!(!link_stays_inside(
            root,
            Path::new("/d/a/b"),
            Path::new("../../c")
        ));
        assert!(!link_stays_inside(
            root,
            Path::new("/d/a/b"),
            Path::new("/d/c")
        ));
        // `..` after a name: the name may be a link (HUM-519, review).
        assert!(!link_stays_inside(
            root,
            Path::new("/d/escape"),
            Path::new("sub/up/..")
        ));
        assert!(!link_stays_inside(
            root,
            Path::new("/d/l"),
            Path::new("d/a/../..")
        ));
        assert!(link_stays_inside(
            root,
            Path::new("/d/sub/up"),
            Path::new("..")
        ));
    }

    /// A dropped reservation unlocks although a copy of its descriptor
    /// lives on, as in a child process another thread forked and that has
    /// not reached its `exec` yet. Mutation proof (HUM-519, CI): without the
    /// explicit unlock in `Drop` this turns red.
    #[test]
    fn a_dropped_reservation_unlocks_while_a_copy_lives() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        let held = depot.lock_named("maintenance").unwrap();
        let copy = held
            .0
            .as_ref()
            .map(|fd| rustix::io::dup(fd).unwrap())
            .unwrap();
        drop(held);
        let again = depot.lock_at(
            &depot
                .root()
                .parent()
                .unwrap()
                .join("claude.maintenance.lock"),
            true,
            1,
        );
        assert!(again.is_ok(), "the copy still holds the lock");
        drop(copy);
    }

    /// A chain of links that each look inside and together lead out is
    /// refused by the depot's walk. Mutation proof: without the rule on `..`
    /// after a name this turns red.
    #[test]
    fn a_chain_of_links_out_of_the_depot_is_unusable() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "claude", "claude");
        let config = depot.config_dir();
        std::fs::create_dir(config.join("sub")).unwrap();
        std::fs::set_permissions(config.join("sub"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        std::os::unix::fs::symlink("..", config.join("sub/up")).unwrap();
        std::os::unix::fs::symlink("sub/up/../../..", config.join("escape")).unwrap();
        let found = depot.installed().unwrap_err();
        assert_eq!(found.code, AGENT_013);
        assert!(found.why.contains("escape"), "{}", found.why);
    }
}
