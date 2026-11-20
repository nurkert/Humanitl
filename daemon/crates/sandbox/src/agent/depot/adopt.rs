//! The takeover of a finished maintenance run into the depot (ADR-0023
//! point 5, HUM-519).
//!
//! A maintenance run leaves its result in a directory the sandbox could
//! write. The daemon trusts none of it: every name is opened relative to an
//! opened directory with `O_NOFOLLOW`, every entry is judged by its own
//! `stat`, and only what passes is copied, file by file, into a staging
//! directory of the daemon beside the depots (`agents/.staging-<id>-*`). The
//! SHA-256 of each file is computed from the bytes copied, so the manifest
//! names exactly what lies in the depot.
//!
//! The rules of the program tree:
//!
//! - only regular files, directories and relative symbolic links that stay
//!   inside the tree, with `..` only at the start of a link's target, so a
//!   chain of links cannot climb out (`link_stays_inside`);
//! - no socket, FIFO or device, no file with a second name (a hard link),
//!   no set-user-ID or set-group-ID bit, no name that is not UTF-8;
//! - at most [`MAX_PROGRAM_BYTES`] in at most [`MAX_PROGRAM_ENTRIES`]
//!   entries, at most [`MAX_DEPTH`] directories deep;
//! - no entry `manifest.json` at its top, which only the daemon writes.
//!
//! The daemon verifies the staged copy itself ([`super::verify`]); nothing
//! the sandbox reports counts. Only then does the depot get its directories
//! (a first install that fails leaves no depot behind), the staging
//! directory becomes `program/<version>/` with one `rename`, and `current`
//! switches with a second one. A journal (`.committing`) names the version
//! between the two; a crash in between is undone by `recover` before the
//! next takeover or switch, so a placed version never stays without a
//! commit. Both directories are synced before success is reported.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::fd::{AsFd as _, BorrowedFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Component, Path, PathBuf};

use humanitl_core::diagnostics::codes::AGENT_011;
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use sha2::{Digest as _, Sha256};

use super::store::{CURRENT_LINK, Depot, MANIFEST_FILE, link_stays_inside, version_name_ok};

/// The most bytes a program tree may hold.
pub const MAX_PROGRAM_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// The most entries a program tree may hold.
pub const MAX_PROGRAM_ENTRIES: usize = 50_000;

/// The deepest a directory of the program tree may lie.
pub const MAX_DEPTH: usize = 64;

/// The journal of a takeover in the depot's root: the version placed and not
/// yet made current.
pub(crate) const JOURNAL: &str = ".committing";

/// The largest small file of the result the daemon reads: the version, the
/// status lines of `gpg`, the vendor's manifest.
pub(crate) const SMALL_FILE_CAP: u64 = 1024 * 1024;

/// The step of a takeover, as `AGENT_011` names it in its `why`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The run itself (download, installer).
    Run,
    /// The result directory or the version it names.
    Result,
    /// The version is pinned: install over an installed one, or another
    /// version than asked for.
    Pin,
    /// The vendor's signature.
    Signature,
    /// The program's SHA-256 against the vendor's manifest.
    Checksum,
    /// The program tree.
    Tree,
    /// Moving the version into the depot and switching `current`.
    Commit,
}

impl Step {
    /// The word in the `why`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Result => "result",
            Self::Pin => "pin",
            Self::Signature => "signature",
            Self::Checksum => "checksum",
            Self::Tree => "tree",
            Self::Commit => "commit",
        }
    }
}

/// `AGENT_011`: the install or update failed at `step`; the depot is
/// unchanged.
pub(crate) fn failed(agent: &str, step: Step, what: &str) -> Diagnostic {
    let fix = match step {
        Step::Run | Step::Result => Some(FixAction::CopyCommand(format!(
            "humanitl agents install {agent}"
        ))),
        Step::Pin | Step::Signature | Step::Checksum | Step::Tree | Step::Commit => None,
    };
    let mut found = Diagnostic::builder(AGENT_011, Severity::Error)
        .why(format!(
            "the install or update of {agent} failed at the step {}: {what}; nothing was taken \
             over, the depot is as it was",
            step.as_str()
        ))
        .build();
    found.fix = fix;
    found
}

/// Whether `path` is a relative path of plain names, inside whatever it is
/// relative to.
#[must_use]
pub fn relative_inside(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Opens the directory `name` under `dir` without following a link.
pub(crate) fn open_dir_at(dir: BorrowedFd<'_>, name: &OsStr) -> rustix::io::Result<OwnedFd> {
    rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

/// Opens the directory at `path` as the root of a walk; the last component
/// must not be a link.
pub(crate) fn open_dir(path: &Path) -> rustix::io::Result<OwnedFd> {
    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

/// Opens the directory `rel` under `root`, component by component, never
/// following a link.
pub(crate) fn open_dir_beneath(root: BorrowedFd<'_>, rel: &Path) -> rustix::io::Result<OwnedFd> {
    let mut current = rustix::io::dup(root)?;
    for part in rel.components() {
        let Component::Normal(name) = part else {
            return Err(rustix::io::Errno::INVAL);
        };
        current = open_dir_at(current.as_fd(), name)?;
    }
    Ok(current)
}

/// Opens the regular file `name` under `dir` for reading: no link, no FIFO
/// that would block, and the opened descriptor must be the regular file
/// `stat` saw.
fn open_file_at(dir: BorrowedFd<'_>, name: &OsStr) -> Result<(OwnedFd, rustix::fs::Stat), String> {
    let fd = rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
        Mode::empty(),
    )
    .map_err(|err| format!("cannot be opened ({err})"))?;
    let stat = rustix::fs::fstat(&fd).map_err(|err| format!("cannot be read ({err})"))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err("is not a regular file".to_owned());
    }
    Ok((fd, stat))
}

/// Reads a small regular file `rel` under `root`: no link on the way, at
/// most `cap` bytes. `Ok(None)` when it does not exist.
pub(crate) fn read_small(
    root: BorrowedFd<'_>,
    rel: &Path,
    cap: u64,
) -> Result<Option<Vec<u8>>, String> {
    let parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let Some(name) = rel.file_name() else {
        return Err(format!("{} names no file", rel.display()));
    };
    let dir = match open_dir_beneath(root, parent) {
        Ok(dir) => dir,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(err) => return Err(format!("{} cannot be opened ({err})", parent.display())),
    };
    let (fd, stat) = match open_file_at(dir.as_fd(), name) {
        Ok(opened) => opened,
        Err(_) if rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW).is_err() => {
            return Ok(None);
        }
        Err(what) => return Err(format!("{} {what}", rel.display())),
    };
    if u64::try_from(stat.st_size).unwrap_or(u64::MAX) > cap {
        return Err(format!("{} is larger than {cap} bytes", rel.display()));
    }
    let mut bytes = Vec::new();
    File::from(fd)
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| format!("{} cannot be read ({err})", rel.display()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return Err(format!("{} is larger than {cap} bytes", rel.display()));
    }
    Ok(Some(bytes))
}

/// The SHA-256 of a regular file `rel` under `root`, read like
/// [`read_small`] (no link on the way, no FIFO) but hashed as it streams, at
/// most `cap` bytes: the archive of a run (HUM-566), too large to hold.
/// `Ok(None)` when it does not exist.
pub(crate) fn sha256_at(
    root: BorrowedFd<'_>,
    rel: &Path,
    cap: u64,
) -> Result<Option<String>, String> {
    let parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let Some(name) = rel.file_name() else {
        return Err(format!("{} names no file", rel.display()));
    };
    let dir = match open_dir_beneath(root, parent) {
        Ok(dir) => dir,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(err) => return Err(format!("{} cannot be opened ({err})", parent.display())),
    };
    let (fd, _) = match open_file_at(dir.as_fd(), name) {
        Ok(opened) => opened,
        Err(_) if rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW).is_err() => {
            return Ok(None);
        }
        Err(what) => return Err(format!("{} {what}", rel.display())),
    };
    let mut input = File::from(fd).take(cap.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = input
            .read(&mut buf)
            .map_err(|err| format!("{} cannot be read ({err})", rel.display()))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > cap {
            return Err(format!("{} is larger than {cap} bytes", rel.display()));
        }
        hasher.update(buf.get(..read).unwrap_or_default());
    }
    Ok(Some(hex(&hasher.finalize())))
}

/// The version the vendor's manifest names at `pointer`, a valid version
/// name.
///
/// # Errors
///
/// What failed: no JSON, no entry, no version name.
pub(crate) fn vendor_version(manifest: &[u8], pointer: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(manifest)
        .map_err(|err| format!("the vendor's manifest is no JSON ({err})"))?;
    let version = value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("the vendor's manifest names no version at {pointer}"))?;
    if !version_name_ok(version) {
        return Err(format!(
            "the vendor's manifest names {version:?}, which is no version name"
        ));
    }
    Ok(version.to_owned())
}

/// The program's SHA-256 as the vendor's manifest names it, under
/// `pointer` with `{platform}` replaced.
///
/// # Errors
///
/// What failed: no JSON, no entry, no 64 hex digits.
pub(crate) fn vendor_checksum(
    manifest: &[u8],
    pointer: &str,
    platform: &str,
) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(manifest)
        .map_err(|err| format!("the vendor's manifest is no JSON ({err})"))?;
    let pointer = pointer.replace("{platform}", platform);
    let sum = value
        .pointer(&pointer)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("the vendor's manifest names no checksum at {pointer}"))?;
    if sum.len() != 64 || !sum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "the checksum at {pointer} in the vendor's manifest is no SHA-256"
        ));
    }
    Ok(sum.to_ascii_lowercase())
}

/// Lower-case hex of a hash.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// Counters of a copy against the caps.
#[derive(Debug, Default)]
struct Budget {
    bytes: u64,
    entries: usize,
}

/// Copies the program tree under `src` into `dst` (a fresh, empty directory
/// of the depot), checking every entry, and records the SHA-256 of every
/// copied file by its relative path.
///
/// # Errors
///
/// `AGENT_011` at the step `tree` with the first entry that fails.
pub(crate) fn take_tree(
    agent: &str,
    src: BorrowedFd<'_>,
    dst: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), Diagnostic> {
    let mut budget = Budget::default();
    copy_dir(agent, src, dst, Path::new(""), files, &mut budget)
}

fn copy_dir(
    agent: &str,
    src: BorrowedFd<'_>,
    dst: &Path,
    rel: &Path,
    files: &mut BTreeMap<String, String>,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let refuse = |path: &Path, what: &str| {
        failed(
            agent,
            Step::Tree,
            &format!("program/{} {what}", path.display()),
        )
    };
    if rel.components().count() >= MAX_DEPTH {
        return Err(refuse(
            rel,
            &format!("lies deeper than {MAX_DEPTH} directories"),
        ));
    }
    let (names, _) = crate::worktree::dir_names(src, MAX_PROGRAM_ENTRIES + 1)
        .map_err(|err| refuse(rel, &format!("cannot be listed ({err})")))?;
    for name in names {
        // A name that is not UTF-8 would collide with another in the
        // manifest, whose keys are text.
        if name.to_str().is_none() {
            return Err(refuse(&rel.join(&name), "has a name that is not UTF-8"));
        }
        budget.entries += 1;
        if budget.entries > MAX_PROGRAM_ENTRIES {
            return Err(failed(
                agent,
                Step::Tree,
                &format!("the program tree holds more than {MAX_PROGRAM_ENTRIES} entries"),
            ));
        }
        let child = rel.join(&name);
        if rel.as_os_str().is_empty() && name == OsStr::new(MANIFEST_FILE) {
            return Err(refuse(
                &child,
                "is the name of the manifest, which only the daemon writes",
            ));
        }
        let stat = rustix::fs::statat(src, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|err| refuse(&child, &format!("cannot be read ({err})")))?;
        // `RawMode` is `u32` on Linux and `u16` on macOS.
        #[allow(clippy::useless_conversion)]
        let mode = u32::from(stat.st_mode);
        let target = dst.join(&name);
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => {
                if mode & 0o6000 != 0 {
                    return Err(refuse(&child, "carries a set-user-ID or set-group-ID bit"));
                }
                let dir = open_dir_at(src, &name)
                    .map_err(|err| refuse(&child, &format!("cannot be opened ({err})")))?;
                make_dir(&target)
                    .map_err(|err| refuse(&child, &format!("cannot be copied ({err})")))?;
                copy_dir(agent, dir.as_fd(), &target, &child, files, budget)?;
            }
            FileType::Symlink => {
                let link = rustix::fs::readlinkat(src, name.as_os_str(), Vec::new())
                    .map_err(|err| refuse(&child, &format!("cannot be read ({err})")))?;
                let link = PathBuf::from(OsString::from_vec(link.into_bytes()));
                let root = Path::new("/program");
                if !link_stays_inside(root, &root.join(&child), &link) {
                    return Err(refuse(
                        &child,
                        &format!(
                            "is a symbolic link out of the program tree, to {}",
                            link.display()
                        ),
                    ));
                }
                std::os::unix::fs::symlink(&link, &target)
                    .map_err(|err| refuse(&child, &format!("cannot be copied ({err})")))?;
            }
            FileType::RegularFile => {
                if stat.st_nlink > 1 {
                    return Err(refuse(
                        &child,
                        &format!(
                            "has {} names (hard links); a file of the program tree has only its \
                             own",
                            stat.st_nlink
                        ),
                    ));
                }
                if mode & 0o6000 != 0 {
                    return Err(refuse(&child, "carries a set-user-ID or set-group-ID bit"));
                }
                let sum = copy_file(agent, src, &name, &child, &target, mode, budget)?;
                files.insert(child.to_string_lossy().into_owned(), sum);
            }
            _ => {
                return Err(refuse(
                    &child,
                    "is a socket, FIFO or device; the program tree holds only files, \
                     directories and links inside it",
                ));
            }
        }
    }
    // Leaf up: a directory is synced once everything in it is, so the tree
    // is on disk before the `rename` that publishes it.
    sync_dir(dst).map_err(|err| refuse(rel, &format!("cannot be synced ({err})")))
}

fn copy_file(
    agent: &str,
    src: BorrowedFd<'_>,
    name: &OsStr,
    rel: &Path,
    target: &Path,
    mode: u32,
    budget: &mut Budget,
) -> Result<String, Diagnostic> {
    let refuse = |what: &str| {
        failed(
            agent,
            Step::Tree,
            &format!("program/{} {what}", rel.display()),
        )
    };
    let (fd, stat) = open_file_at(src, name).map_err(|what| refuse(&what))?;
    // The descriptor is checked again: a file that changed its kind, gained
    // a name or a set-ID bit between `statat` and `openat` is refused too.
    #[allow(clippy::useless_conversion)]
    let opened_mode = u32::from(stat.st_mode);
    if stat.st_nlink > 1 || opened_mode & 0o6000 != 0 {
        return Err(refuse("changed while it was read"));
    }
    let executable = (mode | opened_mode) & 0o111 != 0;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode_private(executable)
        .open(target)
        .map_err(|err| refuse(&format!("cannot be copied ({err})")))?;
    let mut input = File::from(fd);
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buf)
            .map_err(|err| refuse(&format!("cannot be read ({err})")))?;
        if read == 0 {
            break;
        }
        budget.bytes = budget
            .bytes
            .saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if budget.bytes > MAX_PROGRAM_BYTES {
            return Err(failed(
                agent,
                Step::Tree,
                &format!("the program tree is larger than {MAX_PROGRAM_BYTES} bytes"),
            ));
        }
        let chunk = buf.get(..read).unwrap_or_default();
        hasher.update(chunk);
        out.write_all(chunk)
            .map_err(|err| refuse(&format!("cannot be copied ({err})")))?;
    }
    out.sync_all()
        .map_err(|err| refuse(&format!("cannot be copied ({err})")))?;
    Ok(hex(&hasher.finalize()))
}

/// Opens a new file `0700` (executable) or `0600`.
trait PrivateMode {
    fn mode_private(&mut self, executable: bool) -> &mut Self;
}

impl PrivateMode for std::fs::OpenOptions {
    fn mode_private(&mut self, executable: bool) -> &mut Self {
        use std::os::unix::fs::OpenOptionsExt as _;
        self.mode(if executable { 0o700 } else { 0o600 })
    }
}

/// A directory of the depot, `0700`.
pub(crate) fn make_dir(path: &Path) -> std::io::Result<()> {
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let meta = std::fs::symlink_metadata(path)?;
            if !meta.is_dir() {
                return Err(err);
            }
        }
        Err(err) => return Err(err),
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

/// Points `current` at `program/<version>` with one `rename`: a reader sees
/// the old link or the new one, never none.
///
/// # Errors
///
/// What failed, as text for the `why` of `AGENT_011`.
pub(crate) fn switch_current(depot: &Depot, version: &str) -> Result<(), String> {
    if !version_name_ok(version) {
        return Err(format!("{version:?} is no valid version name"));
    }
    let temp = depot
        .root()
        .join(format!(".current-{}", uuid::Uuid::now_v7().simple()));
    let target = Path::new("program").join(version);
    std::os::unix::fs::symlink(&target, &temp)
        .and_then(|()| std::fs::rename(&temp, depot.root().join(CURRENT_LINK)))
        .and_then(|()| sync_dir(depot.root()))
        .map_err(|err| {
            let _ = std::fs::remove_file(&temp);
            format!("current cannot be switched to {version} ({err})")
        })
}

/// A private directory of a takeover beside the depots
/// (`agents/.<kind>-<id>-<random>`), on the depot's file system, removed
/// unless it was moved into place. It never lies inside a depot, so a
/// takeover that fails leaves no trace there.
#[derive(Debug)]
pub(crate) struct Staging {
    path: PathBuf,
    armed: bool,
}

/// The prefixes of the private directories of a takeover.
const STAGING_KINDS: &[&str] = &["staging", "verify"];

impl Staging {
    /// A fresh staging directory for the program tree.
    pub(crate) fn new(depot: &Depot) -> std::io::Result<Self> {
        Self::of_kind(depot, "staging")
    }

    /// A fresh directory for the files the daemon verifies.
    pub(crate) fn for_verify(depot: &Depot) -> std::io::Result<Self> {
        Self::of_kind(depot, "verify")
    }

    fn of_kind(depot: &Depot, kind: &str) -> std::io::Result<Self> {
        let parent = depot.root().parent().unwrap_or(depot.root());
        create_synced(parent)?;
        let path = parent.join(format!(
            ".{kind}-{}-{}",
            depot.agent(),
            uuid::Uuid::now_v7().simple()
        ));
        std::fs::create_dir(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self { path, armed: true })
    }

    /// Its path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Moves it to `program/<version>`; it then stays.
    pub(crate) fn place(mut self, depot: &Depot, version: &str) -> Result<PathBuf, Diagnostic> {
        let dst = depot.program_dir().join(version);
        if std::fs::symlink_metadata(&dst).is_ok() {
            return Err(failed(
                depot.agent(),
                Step::Commit,
                &format!("the depot holds version {version} already"),
            ));
        }
        std::fs::rename(&self.path, &dst).map_err(|err| {
            failed(
                depot.agent(),
                Step::Commit,
                &format!("the version cannot be moved into the depot ({err})"),
            )
        })?;
        self.armed = false;
        // Both directories of the `rename`: the new entry and the gone one.
        let source = depot.root().parent().unwrap_or(depot.root()).to_path_buf();
        for dir in [depot.program_dir(), source] {
            sync_dir(&dir).map_err(|err| {
                failed(
                    depot.agent(),
                    Step::Commit,
                    &format!("{} cannot be synced ({err})", dir.display()),
                )
            })?;
        }
        Ok(dst)
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Undoes what a crashed takeover left (HUM-519): its private directories
/// beside the depots, candidate links in the depot's root, and a version
/// placed under `program/` that `current` never came to name, which the
/// journal names. A journal whose version is current only goes.
pub(crate) fn recover(depot: &Depot) {
    let parent = depot.root().parent().unwrap_or(depot.root());
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let ours = STAGING_KINDS.iter().any(|kind| {
                name.as_bytes()
                    .starts_with(format!(".{kind}-{}-", depot.agent()).as_bytes())
            });
            if ours && std::fs::symlink_metadata(entry.path()).is_ok_and(|meta| meta.is_dir()) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(depot.root()) {
        for entry in entries.flatten() {
            if entry.file_name().as_bytes().starts_with(b".current-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let journal = depot.root().join(JOURNAL);
    let Ok(meta) = std::fs::symlink_metadata(&journal) else {
        return;
    };
    if meta.is_file() && meta.len() <= 256 {
        let placed = std::fs::read_to_string(&journal).unwrap_or_default();
        let placed = placed.trim();
        if version_name_ok(placed) && depot.current_version().as_deref() != Some(placed) {
            remove_unused_version(depot, placed);
        }
    }
    let _ = std::fs::remove_file(&journal);
    let _ = sync_dir(depot.root());
}

/// Removes `program/<version>` only under the depot's exclusive lock: a
/// session may have started on it while `current` named it. When a session
/// holds the depot, the version stays, and a later pruning removes it.
/// Answers whether it was removed.
pub(crate) fn remove_unused_version(depot: &Depot, version: &str) -> bool {
    let Ok(_exclusive) = depot.reserve(true) else {
        return false;
    };
    let dir = depot.program_dir().join(version);
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return false;
    }
    let removed = std::fs::remove_dir_all(&dir).is_ok();
    let _ = sync_dir(&depot.program_dir());
    removed
}

/// Removes a depot this takeover created, only under its exclusive lock,
/// like [`remove_unused_version`].
pub(crate) fn remove_unused_depot(depot: &Depot) -> bool {
    let Ok(_exclusive) = depot.reserve(true) else {
        return false;
    };
    let removed = std::fs::remove_dir_all(depot.root()).is_ok();
    if let Some(parent) = depot.root().parent() {
        let _ = sync_dir(parent);
    }
    removed
}

/// Creates `dir` (`0700`) and every missing directory above it, and syncs
/// the parent of each one it creates, so the new entry survives a crash
/// (the data directory above `agents/` included).
pub(crate) fn create_synced(dir: &Path) -> std::io::Result<()> {
    let mut missing = Vec::new();
    let mut at = Some(dir);
    while let Some(path) = at {
        if std::fs::symlink_metadata(path).is_ok() {
            break;
        }
        missing.push(path.to_path_buf());
        at = path.parent();
    }
    for path in missing.iter().rev() {
        // Only `dir` itself becomes `0700`; a directory above it (the data
        // directory of the user) keeps the mode it gets by default.
        if path == dir {
            make_dir(path)?;
        } else {
            match std::fs::create_dir(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }
        }
        if let Some(parent) = path.parent() {
            sync_dir(parent)?;
        }
    }
    make_dir(dir)
}

/// Writes the journal: `version` is about to be placed.
pub(crate) fn begin_journal(depot: &Depot, version: &str) -> Result<(), Diagnostic> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let journal = depot.root().join(JOURNAL);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&journal)
        .and_then(|mut file| {
            file.write_all(version.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| sync_dir(depot.root()))
        .map_err(|err| {
            failed(
                depot.agent(),
                Step::Commit,
                &format!("the journal cannot be written ({err})"),
            )
        })
}

/// Removes the journal after `current` names the placed version.
pub(crate) fn end_journal(depot: &Depot) -> Result<(), String> {
    std::fs::remove_file(depot.root().join(JOURNAL))
        .and_then(|()| sync_dir(depot.root()))
        .map_err(|err| format!("the journal cannot be removed ({err})"))
}

/// `fsync` of a directory, so a `rename` in it survives a crash.
pub(crate) fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    sync_probe::seen(dir)?;
    File::open(dir)?.sync_all()
}

/// Sets `current` back to `previous` after a switch that may have happened
/// although it reported a failure (the `rename` done, the `fsync` after it
/// not): to the previous version, or without one, no `current` at all.
///
/// # Errors
///
/// What failed; `current` may then still name the new version.
pub(crate) fn roll_back(depot: &Depot, previous: Option<&str>) -> Result<(), String> {
    if let Some(previous) = previous {
        return switch_current(depot, previous);
    }
    match std::fs::remove_file(depot.root().join(CURRENT_LINK)) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("current cannot be removed ({err})")),
    }
    sync_dir(depot.root()).map_err(|err| format!("the depot cannot be synced ({err})"))
}

/// `AGENT_011` for a commit whose undoing failed too: the depot may be
/// switched, and the sentence says so instead of claiming it unchanged.
pub(crate) fn uncertain(agent: &str, version: &str, what: &str, undo: &str) -> Diagnostic {
    let mut found = Diagnostic::builder(AGENT_011, Severity::Error)
        .why(format!(
            "the install or update of {agent} failed at the step commit: {what}; setting \
             current back failed as well ({undo}), so current may name version {version} \
             now; the status shows which version is current"
        ))
        .build();
    found.fix = Some(FixAction::CopyCommand(format!(
        "humanitl agents status {agent}"
    )));
    found
}

/// The test hook of [`sync_dir`]: records every directory synced, and fails
/// chosen calls for one directory.
#[cfg(test)]
pub(crate) mod sync_probe {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    #[derive(Default)]
    struct Probe {
        seen: Vec<PathBuf>,
        fail: Option<(PathBuf, Vec<usize>)>,
        calls: usize,
    }

    thread_local! {
        static PROBE: RefCell<Probe> = RefCell::new(Probe::default());
    }

    pub(crate) fn seen(dir: &Path) -> std::io::Result<()> {
        PROBE.with_borrow_mut(|probe| {
            probe.seen.push(dir.to_path_buf());
            let Some((path, calls)) = &probe.fail else {
                return Ok(());
            };
            if path != dir {
                return Ok(());
            }
            let call = probe.calls;
            probe.calls += 1;
            if calls.contains(&call) {
                Err(std::io::Error::other("fsync failure injected by the test"))
            } else {
                Ok(())
            }
        })
    }

    /// Fails the calls numbered `calls` (from 0) of `sync_dir(dir)` on this
    /// thread, from now on.
    pub(crate) fn fail(dir: &Path, calls: &[usize]) {
        PROBE.with_borrow_mut(|probe| {
            probe.fail = Some((dir.to_path_buf(), calls.to_vec()));
            probe.calls = 0;
        });
    }

    /// Every directory synced on this thread since the last call; ends any
    /// failure.
    pub(crate) fn take() -> Vec<PathBuf> {
        PROBE.with_borrow_mut(|probe| {
            probe.fail = None;
            std::mem::take(&mut probe.seen)
        })
    }
}

/// Removes every version under `program/` but `keep`, links not followed.
/// Returns the versions removed.
pub(crate) fn prune(depot: &Depot, keep: &[&str]) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(depot.program_dir()) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if keep.contains(&name) {
            continue;
        }
        let path = entry.path();
        let gone = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path).is_ok(),
            Ok(_) => std::fs::remove_file(&path).is_ok(),
            Err(_) => false,
        };
        if gone {
            removed.push(name.to_owned());
        }
    }
    removed.sort();
    removed
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_vendor_checksum_is_read_by_platform() {
        let sum = "AB".repeat(32);
        let manifest =
            format!(r#"{{"version":"1","platforms":{{"linux-x64":{{"checksum":"{sum}"}}}}}}"#);
        assert_eq!(
            vendor_checksum(
                manifest.as_bytes(),
                "/platforms/{platform}/checksum",
                "linux-x64"
            )
            .unwrap(),
            "ab".repeat(32)
        );
        assert!(
            vendor_checksum(
                manifest.as_bytes(),
                "/platforms/{platform}/checksum",
                "darwin-x64"
            )
            .is_err()
        );
        assert!(
            vendor_checksum(
                b"{\"platforms\":{\"p\":{\"checksum\":\"xyz\"}}}",
                "/platforms/{platform}/checksum",
                "p"
            )
            .is_err()
        );
    }

    #[test]
    fn relative_inside_takes_only_plain_names() {
        assert!(relative_inside("claude"));
        assert!(relative_inside("bin/claude"));
        assert!(!relative_inside("../claude"));
        assert!(!relative_inside("/usr/bin/claude"));
        assert!(!relative_inside("./claude"));
        assert!(!relative_inside(""));
    }
}
