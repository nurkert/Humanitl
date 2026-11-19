//! The profile's masks over the disposable `/work` of the kernel level
//! (HUM-481).
//!
//! Under bwrap a disposable session mounts the overlay on `/work` and then
//! the masks on top of it: an empty, writable tmpfs over every masked
//! directory that exists (`--tmpfs`, `.git/hooks`, `.vscode`, ...) and an
//! empty, read-only file over every masked file that exists
//! (`--ro-bind-data`, `.env`, `.git/config`, ...). A write to a masked file
//! fails, and what the agent puts into a masked directory never reaches the
//! upper layer.
//!
//! In the guest the masks of the host lie in the lower layer only (in the
//! jail of the project's file daemon), and the overlay copies a masked file
//! up as soon as the agent writes it. The scan never applies such a change
//! (`scan_masked`), but the agent's view would differ from bwrap's. So PID 1
//! lays the same masks over the overlay, after it sits on `/work` and before
//! anything else runs, in the plan's order (`work_masks`):
//!
//! - `tmpfs:<relative>`: a tmpfs as bwrap's `--tmpfs` (mode 0755, owned by
//!   the agent's user, `nosuid`, `nodev`), with the plan's size
//!   (`mask_mib`), the same as the tmpfs over that place on the host.
//! - `empty:<relative>`: a bind of one empty file (mode 0600, owned by the
//!   agent's user, as bwrap's `--ro-bind-data` creates it) of the overlay's
//!   staging tmpfs, read-only, `nosuid` and `nodev` as bwrap's. The copies are taken while the staging tmpfs is
//!   still mounted; each outlives its detach.
//!
//! Each target is opened beneath `/work` with `openat2(2)`
//! (`RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`) and
//! mounted on through that descriptor, so a mask never lands outside the
//! overlay, whatever the layers hold. A tmpfs mask needs a directory there,
//! a file mask anything but a directory; anything else refuses the start,
//! as a failed `--tmpfs` or `--ro-bind-data` refuses bwrap's.
//!
//! The daemon names only masks that exist in the project (as for bwrap,
//! `present_under_work`), and the mount table it expects names each of
//! them; the agent's filter refuses `mount` and `umount2`, and the agent has
//! no capability to remove one.

use std::fs;
use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use crate::json::Parser;
use crate::linux::guest_tmpfs::{self, c_path};

/// The empty file on the staging tmpfs that every file mask binds.
const EMPTY: &str = "empty";

/// The mode of that file: bwrap's default for `--ro-bind-data`.
const EMPTY_MODE: u32 = 0o600;

/// One mask over `/work`, relative to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mask {
    /// An empty, writable tmpfs over a directory (bwrap's `--tmpfs`).
    Tmpfs(PathBuf),
    /// An empty, read-only file over a file (bwrap's `--ro-bind-data`).
    Empty(PathBuf),
}

impl Mask {
    /// The path relative to `/work`.
    pub fn relative(&self) -> &Path {
        match self {
            Self::Tmpfs(path) | Self::Empty(path) => path,
        }
    }
}

/// A path relative to `/work` that names exactly one place below it: not
/// empty, no leading `/`, no `.`, `..` or empty component, no NUL.
fn relative(text: &str) -> Result<PathBuf, String> {
    let clean = !text.is_empty()
        && !text.contains('\0')
        && text
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if clean {
        Ok(PathBuf::from(text))
    } else {
        Err("work_masks:not-a-relative-path".to_owned())
    }
}

/// Reads the plan's `work_masks`, an array of `tmpfs:<relative>` and
/// `empty:<relative>`; absent means none.
///
/// # Errors
///
/// A short reason without spaces for anything but that shape.
pub fn parse(raw: Option<&str>) -> Result<Vec<Mask>, String> {
    let Some(text) = raw else {
        return Ok(Vec::new());
    };
    let mut parser = Parser::new(text);
    let items = parser
        .string_array()
        .and_then(|items| parser.end().map(|()| items))
        .map_err(|err| format!("work_masks:{err}").replace(' ', "_"))?;
    items
        .iter()
        .map(|item| {
            let (kind, path) = item
                .split_once(':')
                .ok_or_else(|| "work_masks:no-kind".to_owned())?;
            let path = relative(path)?;
            match kind {
                "tmpfs" => Ok(Mask::Tmpfs(path)),
                "empty" => Ok(Mask::Empty(path)),
                _ => Err("work_masks:unknown-kind".to_owned()),
            }
        })
        .collect()
}

/// The file masks' sources, taken from the staging tmpfs before it goes.
pub struct Held {
    /// One detached, read-only copy of the empty file per file mask, in
    /// the order of the plan.
    files: Vec<OwnedFd>,
}

/// Creates the empty file on the staging tmpfs `staging`, owned by `uid`
/// and `gid` with mode 0600, binds it onto itself read-only, `nosuid` and
/// `nodev` as bwrap's `--ro-bind-data`, and takes one detached copy of that
/// bind per file mask.
///
/// # Errors
///
/// The step that failed.
pub fn hold(staging: &Path, masks: &[Mask], uid: u32, gid: u32) -> io::Result<Held> {
    let count = masks
        .iter()
        .filter(|mask| matches!(mask, Mask::Empty(_)))
        .count();
    if count == 0 {
        return Ok(Held { files: Vec::new() });
    }
    let empty = staging.join(EMPTY);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(EMPTY_MODE)
        .open(&empty)?;
    fs::set_permissions(&empty, fs::Permissions::from_mode(EMPTY_MODE))?;
    std::os::unix::fs::chown(&empty, Some(uid), Some(gid))?;
    let text = empty.to_string_lossy().into_owned();
    super::guest::mount_with(&text, &text, "", libc::MS_BIND, "")?;
    // A bind takes the flags of its source (here `noexec` too); the remount
    // sets exactly read-only, `nosuid` and `nodev`, and the copies keep the
    // flags of the mount they copy.
    super::guest::mount_with(
        "",
        &text,
        "",
        libc::MS_REMOUNT | libc::MS_BIND | libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
        "",
    )?;
    let files = (0..count)
        .map(|_| guest_tmpfs::clone_tree(&empty))
        .collect::<io::Result<Vec<_>>>()?;
    Ok(Held { files })
}

/// Lays `masks` over the overlay at `work`, in their order (module
/// documentation).
///
/// # Errors
///
/// The evidence of the first mask that failed, without spaces.
pub fn lay(
    work: &Path,
    masks: &[Mask],
    held: Held,
    mib: u64,
    uid: u32,
    gid: u32,
) -> Result<(), String> {
    let fail = |what: &str, mask: &Mask, err: &io::Error| {
        format!("mask-{what}:{}:{err}", mask.relative().display()).replace(' ', "_")
    };
    let root = open_dir(work).map_err(|err| format!("mask-root:{err}").replace(' ', "_"))?;
    let mut files = held.files.into_iter();
    let data = format!("mode=0755,uid={uid},gid={gid},size={mib}m");
    let mut laid: Vec<&Path> = Vec::new();
    for mask in masks {
        if let Some(base) = tmpfs_base(&laid, mask.relative()) {
            make_point(&root, base, mask, uid, gid).map_err(|e| fail("point", mask, &e))?;
        }
        let target = open_target(&root, mask).map_err(|e| fail("target", mask, &e))?;
        match mask {
            Mask::Tmpfs(_) => {
                super::guest::mount_with(
                    "tmpfs",
                    &format!("/proc/self/fd/{}", target.as_raw_fd()),
                    "tmpfs",
                    libc::MS_NOSUID | libc::MS_NODEV,
                    &data,
                )
                .map_err(|e| fail("tmpfs", mask, &e))?;
                note_tmpfs(&mut laid, mask.relative());
            }
            Mask::Empty(_) => {
                let tree = files.next().ok_or_else(|| {
                    fail("bind", mask, &io::Error::from_raw_os_error(libc::EBADF))
                })?;
                guest_tmpfs::move_tree_onto(&tree, &target).map_err(|e| fail("bind", mask, &e))?;
            }
        }
    }
    Ok(())
}

/// The tmpfs mask laid before that holds `relative`, the deepest one, if
/// any: under bwrap a later mask inside an earlier `--tmpfs` gets its mount
/// point created in that fresh tmpfs (`/work/custom` and then
/// `/work/custom/secret`). Only tmpfs this shim laid itself count, never a
/// place of the overlay, and only earlier ones, as bwrap's order has it.
fn tmpfs_base<'a>(laid: &[&'a Path], relative: &Path) -> Option<&'a Path> {
    laid.iter()
        .copied()
        .filter(|base| relative != *base && relative.starts_with(base))
        .max_by_key(|base| base.components().count())
}

/// Notes the tmpfs just laid at `relative`. Every earlier one at or below
/// it is hidden under the fresh tmpfs now and no longer counts: a later
/// mask below it gets its mount point in the visible tmpfs, as under bwrap.
fn note_tmpfs<'a>(laid: &mut Vec<&'a Path>, relative: &'a Path) {
    laid.retain(|earlier| !earlier.starts_with(relative));
    laid.push(relative);
}

/// Creates the mount point of `mask` inside the tmpfs mask `base` it lies
/// in, as bwrap does: missing parents as directories 0755, the point
/// itself as a directory 0755 for a tmpfs or an empty file 0600 for a file,
/// all owned by the agent's user. Every step goes through `openat2` beneath
/// that tmpfs without a symbolic link, and a file is created with
/// `O_EXCL`; what exists already stays (the kind is checked after).
fn make_point(root: &OwnedFd, base: &Path, mask: &Mask, uid: u32, gid: u32) -> io::Result<()> {
    let rest = mask
        .relative()
        .strip_prefix(base)
        .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    let parts: Vec<&std::ffi::OsStr> = rest.iter().collect();
    let mut dir = open_beneath(root, base)?;
    for (index, part) in parts.iter().enumerate() {
        let name = Path::new(part);
        let last = index + 1 == parts.len();
        if last && matches!(mask, Mask::Empty(_)) {
            let flags = libc::O_CREAT | libc::O_EXCL | libc::O_WRONLY | libc::O_CLOEXEC;
            match open_at(&dir, name, flags, EMPTY_MODE) {
                Ok(file) => {
                    let file = fs::File::from(file);
                    file.set_permissions(fs::Permissions::from_mode(EMPTY_MODE))?;
                    std::os::unix::fs::fchown(&file, Some(uid), Some(gid))?;
                }
                Err(err) if err.raw_os_error() == Some(libc::EEXIST) => {}
                Err(err) => return Err(err),
            }
            return Ok(());
        }
        let text = c_path(name)?;
        // SAFETY: a descriptor this function borrows and a NUL-terminated
        // name that outlives the call.
        if unsafe { libc::mkdirat(dir.as_raw_fd(), text.as_ptr(), 0o755) } == 0 {
            // SAFETY: as above, with a plain mode.
            if unsafe { libc::fchmodat(dir.as_raw_fd(), text.as_ptr(), 0o755, 0) } != 0 {
                return Err(io::Error::last_os_error());
            }
            chown_at(&dir, name, uid, gid)?;
        } else {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EEXIST) {
                return Err(err);
            }
        }
        dir = open_beneath(&dir, name)?;
    }
    Ok(())
}

/// `fchownat(2)` of `name` in `dir`, never through a symbolic link.
fn chown_at(dir: &OwnedFd, name: &Path, uid: u32, gid: u32) -> io::Result<()> {
    let text = c_path(name)?;
    // SAFETY: a descriptor this function borrows, a NUL-terminated name that
    // outlives the call, and integers.
    if unsafe {
        libc::fchownat(
            dir.as_raw_fd(),
            text.as_ptr(),
            uid,
            gid,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// The place of `mask` beneath `root`, opened as `O_PATH` without a
/// symbolic link on the way or at the end, and of the mask's kind: a
/// directory for a tmpfs, anything else for a file. A missing place, a
/// symbolic link or the wrong kind is an error, and the start fails closed.
fn open_target(root: &OwnedFd, mask: &Mask) -> io::Result<OwnedFd> {
    let target = open_beneath(root, mask.relative())?;
    let kind = file_type(&target)?;
    let refused = match (mask, kind) {
        // Defense in depth: `openat2` already refuses a link (`ELOOP`).
        (_, libc::S_IFLNK) => Some(libc::ELOOP),
        (Mask::Tmpfs(_), libc::S_IFDIR) => None,
        (Mask::Tmpfs(_), _) => Some(libc::ENOTDIR),
        (Mask::Empty(_), libc::S_IFDIR) => Some(libc::EISDIR),
        (Mask::Empty(_), _) => None,
    };
    refused.map_or(Ok(target), |errno| Err(io::Error::from_raw_os_error(errno)))
}

/// `open(2)` of a directory as `O_PATH`.
fn open_dir(path: &Path) -> io::Result<OwnedFd> {
    let path = c_path(path)?;
    // SAFETY: a NUL-terminated path that outlives the call, and integers.
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: open just returned this descriptor, and nothing else owns it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// `struct open_how` of `openat2(2)`; the libc crate's is not constructible
/// outside it.
#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

/// `openat2(2)` of `relative` beneath `root` as `O_PATH`, through no
/// symbolic link and never out of `root`.
fn open_beneath(root: &OwnedFd, relative: &Path) -> io::Result<OwnedFd> {
    // No `O_NOFOLLOW`: with `O_PATH` it would hand out a descriptor of a
    // trailing symbolic link itself instead of `ELOOP`.
    open_at(root, relative, libc::O_PATH | libc::O_CLOEXEC, 0)
}

/// `openat2(2)` of `relative` beneath `root` with `flags` and `mode`,
/// through no symbolic link and never out of `root`.
fn open_at(root: &OwnedFd, relative: &Path, flags: libc::c_int, mode: u32) -> io::Result<OwnedFd> {
    let path = c_path(relative)?;
    let how = OpenHow {
        flags: u64::try_from(flags).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?,
        mode: u64::from(mode),
        resolve: libc::RESOLVE_BENEATH | libc::RESOLVE_NO_SYMLINKS | libc::RESOLVE_NO_MAGICLINKS,
    };
    // SAFETY: a descriptor this function borrows, a NUL-terminated path and
    // a struct that outlive the call, and its exact size.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            root.as_raw_fd(),
            path.as_ptr(),
            &raw const how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = i32::try_from(fd).map_err(|_| io::Error::from_raw_os_error(libc::EBADF))?;
    // SAFETY: openat2 just returned this descriptor, and nothing else owns it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The file type bits of the object behind `fd`.
fn file_type(fd: &OwnedFd) -> io::Result<libc::mode_t> {
    // SAFETY: stat is plain data; all zero is a valid value.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: a descriptor this function borrows and a valid stat buffer.
    if unsafe { libc::fstat(fd.as_raw_fd(), &raw mut stat) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(stat.st_mode & libc::S_IFMT)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn absent_means_no_masks() {
        assert_eq!(parse(None), Ok(Vec::new()));
        assert_eq!(parse(Some("[]")), Ok(Vec::new()));
    }

    #[test]
    fn the_documented_shape_reads_in_order() {
        assert_eq!(
            parse(Some(r#"["tmpfs:.git/hooks","empty:.env","empty:a b/c:d"]"#)),
            Ok(vec![
                Mask::Tmpfs(PathBuf::from(".git/hooks")),
                Mask::Empty(PathBuf::from(".env")),
                Mask::Empty(PathBuf::from("a b/c:d")),
            ])
        );
    }

    /// A mask names exactly one place beneath `/work`: never `/work`
    /// itself, never above it, never an absolute path.
    #[test]
    fn every_deviation_is_refused() {
        for (text, why) in [
            (r#"["tmpfs"]"#, "work_masks:no-kind"),
            (r#"["bind:.env"]"#, "work_masks:unknown-kind"),
            (r#"["empty:"]"#, "work_masks:not-a-relative-path"),
            (r#"["empty:/etc/passwd"]"#, "work_masks:not-a-relative-path"),
            (r#"["empty:../x"]"#, "work_masks:not-a-relative-path"),
            (r#"["empty:a/../../x"]"#, "work_masks:not-a-relative-path"),
            (r#"["tmpfs:."]"#, "work_masks:not-a-relative-path"),
            (r#"["tmpfs:a//b"]"#, "work_masks:not-a-relative-path"),
            (r#"["tmpfs:a/"]"#, "work_masks:not-a-relative-path"),
        ] {
            assert_eq!(parse(Some(text)), Err(why.to_owned()), "{text}");
        }
        assert!(parse(Some(r#"{"a":"b"}"#)).is_err());
        assert!(parse(Some(r#"["empty:.env"] x"#)).is_err());
        assert!(parse(Some(r#"["empty:a\u0000b"]"#)).is_err());
    }

    /// A project with every shape a mask can meet.
    fn project() -> (PathBuf, OwnedFd) {
        let dir = std::env::temp_dir().join(format!("hum481-masks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let base = dir.as_path();
        fs::create_dir_all(base.join(".git/hooks")).unwrap();
        fs::write(base.join(".env"), "SECRET\n").unwrap();
        fs::create_dir(base.join("outside")).unwrap();
        fs::write(base.join("outside/f"), "").unwrap();
        std::os::unix::fs::symlink("outside", base.join("link-dir")).unwrap();
        std::os::unix::fs::symlink("outside/f", base.join("link-file")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", base.join("link-abs")).unwrap();
        let root = open_dir(base).unwrap();
        (dir, root)
    }

    fn errno(result: io::Result<OwnedFd>) -> Option<i32> {
        result.err().and_then(|err| err.raw_os_error())
    }

    /// HUM-481: A mask lands on its planned place or the start fails
    /// closed: a directory for a tmpfs, a file for an empty file, nothing
    /// missing, no symbolic link at the end or on the way.
    #[test]
    fn a_target_of_the_wrong_kind_missing_or_through_a_link_is_refused() {
        let (dir, root) = project();
        let tmpfs = |rel: &str| Mask::Tmpfs(PathBuf::from(rel));
        let empty = |rel: &str| Mask::Empty(PathBuf::from(rel));
        assert!(open_target(&root, &tmpfs(".git/hooks")).is_ok());
        assert!(open_target(&root, &empty(".env")).is_ok());
        assert_eq!(
            errno(open_target(&root, &tmpfs(".env"))),
            Some(libc::ENOTDIR)
        );
        assert_eq!(
            errno(open_target(&root, &empty(".git/hooks"))),
            Some(libc::EISDIR)
        );
        assert_eq!(
            errno(open_target(&root, &empty("missing"))),
            Some(libc::ENOENT)
        );
        for (mask, why) in [
            (tmpfs("link-dir"), "a link to a directory"),
            (empty("link-file"), "a link to a file"),
            (empty("link-dir/f"), "a link on the way"),
            (empty("link-abs"), "an absolute link"),
        ] {
            assert_eq!(errno(open_target(&root, &mask)), Some(libc::ELOOP), "{why}");
        }
        drop(root);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// HUM-481: A later mask inside an earlier tmpfs mask gets its mount
    /// point in that tmpfs, the deepest one, as under bwrap; a sibling, the
    /// same path or a later tmpfs do not count.
    #[test]
    fn a_mask_inside_an_earlier_tmpfs_finds_that_tmpfs() {
        let custom = Path::new("custom");
        let inner = Path::new("custom/inner");
        let laid = [custom, inner];
        assert_eq!(tmpfs_base(&laid, Path::new("custom/secret")), Some(custom));
        assert_eq!(tmpfs_base(&laid, Path::new("custom/inner/x")), Some(inner));
        assert_eq!(tmpfs_base(&laid, Path::new("custom")), None);
        assert_eq!(tmpfs_base(&laid, Path::new("customer/x")), None);
        assert_eq!(tmpfs_base(&laid, Path::new(".env")), None);
        assert_eq!(tmpfs_base(&[], Path::new("custom/secret")), None);
        // A tmpfs laid over an earlier one hides it: the visible one counts.
        let mut laid = Vec::new();
        note_tmpfs(&mut laid, inner);
        note_tmpfs(&mut laid, Path::new("other"));
        note_tmpfs(&mut laid, custom);
        assert_eq!(laid, [Path::new("other"), custom]);
        assert_eq!(
            tmpfs_base(&laid, Path::new("custom/inner/secret")),
            Some(custom)
        );
    }

    /// HUM-481: Inside the tmpfs (here a plain directory) the mount point
    /// of a nested file mask is an empty file 0600 and of a nested tmpfs a
    /// directory 0755, with missing parents; `open_target` then finds the
    /// right kind. A symbolic link on the way is refused, and an existing
    /// point stays as it is.
    #[test]
    fn a_nested_mask_gets_its_mount_point_inside_the_tmpfs() {
        use std::os::unix::fs::MetadataExt as _;
        let dir = std::env::temp_dir().join(format!("hum481-nested-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("custom")).unwrap();
        std::os::unix::fs::symlink("/tmp", dir.join("custom/link")).unwrap();
        let root = open_dir(&dir).unwrap();
        // SAFETY: getuid and getgid take nothing and cannot fail.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        let base = Path::new("custom");
        let file = Mask::Empty(PathBuf::from("custom/a/secret"));
        let deep = Mask::Tmpfs(PathBuf::from("custom/inner/deep"));
        make_point(&root, base, &file, uid, gid).unwrap();
        make_point(&root, base, &deep, uid, gid).unwrap();
        make_point(&root, base, &file, uid, gid).unwrap();
        assert!(open_target(&root, &file).is_ok());
        assert!(open_target(&root, &deep).is_ok());
        let meta = fs::symlink_metadata(dir.join("custom/a/secret")).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.mode() & 0o7777, 0o600);
        assert_eq!((meta.uid(), meta.gid()), (uid, gid));
        for made in ["custom/a", "custom/inner", "custom/inner/deep"] {
            let meta = fs::symlink_metadata(dir.join(made)).unwrap();
            assert!(meta.is_dir(), "{made}");
            assert_eq!(meta.mode() & 0o7777, 0o755, "{made}");
        }
        let through = Mask::Empty(PathBuf::from("custom/link/x"));
        assert_eq!(
            make_point(&root, base, &through, uid, gid)
                .err()
                .and_then(|err| err.raw_os_error()),
            Some(libc::ELOOP)
        );
        drop(root);
        fs::remove_dir_all(&dir).unwrap();
    }
}
