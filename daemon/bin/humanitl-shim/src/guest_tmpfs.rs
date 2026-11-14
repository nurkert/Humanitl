//! The writable places of the guest (HUM-466).
//!
//! The guest's root comes read-only from the host. Under bwrap every
//! `--tmpfs` of the plan outside `/work` is a writable, empty file system
//! (`/tmp`, `/var/tmp`, the agent's home), and so is `/run`, which bwrap
//! creates on its own writable root; in the guest those places are empty,
//! read-only directories of the shared root until the shim mounts a tmpfs on
//! each, after the module lock and before the agent exists, as bwrap's
//! `--tmpfs` does: mode 0755, owned by the agent's user, `nosuid`, `nodev`,
//! the kernel's default size.
//!
//! bwrap mounts its tmpfs before the binds that follow it in the plan, so a
//! file of the agent's adapter under the agent's home
//! (`~/.config/<agent>/...`) lies on top of the tmpfs. In the guest the
//! tmpfs comes last and would hide such a file. The plan therefore names,
//! under `tmpfs_keep`, what lies beneath a tmpfs and must stay on top of it,
//! each with its kind:
//!
//! - `bind:<path>`: a read-only object of the shared root. The shim takes a
//!   detached copy of its mount (`open_tree(2)` with `OPEN_TREE_CLONE`)
//!   before the tmpfs hides it, and moves that copy onto a fresh mount point
//!   on the tmpfs afterwards (`move_mount(2)`). The copy keeps the read-only
//!   flag of the root's mount; the mount table check sees it as one more
//!   read-only `virtiofs` entry, which the plan expects.
//! - `link:<path>`: a symbolic link; read before, created again after.
//! - `dir:<path>`: an empty directory, a mount point of something later.
//!
//! The parents the shim creates on a tmpfs are owned by the agent's user
//! with mode 0755, as bwrap's are: the agent may add files next to its
//! adapter's, never change these.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::fd::{FromRawFd as _, OwnedFd};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use crate::json::Parser;

/// `move_mount(2)`: the source is the descriptor itself. Not in every libc
/// of the crate (the musl half lacks it), so here once.
const MOVE_MOUNT_F_EMPTY_PATH: libc::c_uint = 0x0000_0004;

/// What lies beneath a tmpfs and stays on top of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keep {
    /// A read-only object of the shared root, moved onto the tmpfs.
    Bind(PathBuf),
    /// A symbolic link, created again on the tmpfs.
    Link(PathBuf),
    /// An empty directory, created on the tmpfs.
    Dir(PathBuf),
}

impl Keep {
    fn path(&self) -> &Path {
        match self {
            Self::Bind(path) | Self::Link(path) | Self::Dir(path) => path,
        }
    }
}

/// The plan's `tmpfs` and `tmpfs_keep`, in their order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tmpfs {
    /// Where a tmpfs goes, in the order of the plan.
    pub points: Vec<PathBuf>,
    /// What stays on top, in the order of the plan.
    pub keep: Vec<Keep>,
}

/// A path the shim creates or mounts on as root: absolute, not `/`, and
/// without `.` or `..`, so that it names exactly the place the daemon
/// planned.
fn plain(key: &str, text: &str) -> Result<PathBuf, String> {
    // On the text, not on `Path::components`, which drops a `.` silently.
    let clean = text.strip_prefix('/').is_some_and(|rest| {
        rest.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
    }) && !text.contains('\0');
    if clean {
        Ok(PathBuf::from(text))
    } else {
        Err(format!("{key}:not-a-plain-path"))
    }
}

/// Reads the plan's `tmpfs` (an array of paths) and `tmpfs_keep` (an array
/// of `<kind>:<path>`); both may be absent.
///
/// # Errors
///
/// A short reason without spaces for anything but that shape.
pub fn parse(points: Option<&str>, keep: Option<&str>) -> Result<Tmpfs, String> {
    let array = |key: &str, raw: Option<&str>| -> Result<Vec<String>, String> {
        raw.map_or_else(
            || Ok(Vec::new()),
            |text| {
                Parser::new(text)
                    .string_array()
                    .map_err(|err| format!("{key}:{err}").replace(' ', "_"))
            },
        )
    };
    let points = array("tmpfs", points)?
        .iter()
        .map(|text| plain("tmpfs", text))
        .collect::<Result<Vec<_>, _>>()?;
    let keep = array("tmpfs_keep", keep)?
        .iter()
        .map(|item| {
            let (kind, path) = item
                .split_once(':')
                .ok_or_else(|| "tmpfs_keep:no-kind".to_owned())?;
            let path = plain("tmpfs_keep", path)?;
            match kind {
                "bind" => Ok(Keep::Bind(path)),
                "link" => Ok(Keep::Link(path)),
                "dir" => Ok(Keep::Dir(path)),
                _ => Err("tmpfs_keep:unknown-kind".to_owned()),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Tmpfs { points, keep })
}

/// What the shim took of a kept object before the tmpfs hid it.
enum Held {
    Tree(OwnedFd),
    Link(PathBuf),
    Dir,
}

/// Mounts every tmpfs of the plan and puts back what lies on top of it,
/// owned by `uid` and `gid`.
///
/// # Errors
///
/// The evidence of the first step that failed, without spaces.
pub fn mount_all(plan: &Tmpfs, uid: u32, gid: u32) -> Result<(), String> {
    let fail = |what: &str, path: &Path, err: &io::Error| {
        format!("{what}-{}:{err}", path.display()).replace(' ', "_")
    };
    let mut held = Vec::with_capacity(plan.keep.len());
    for keep in &plan.keep {
        held.push(match keep {
            Keep::Bind(path) => Held::Tree(clone_tree(path).map_err(|e| fail("clone", path, &e))?),
            Keep::Link(path) => {
                Held::Link(fs::read_link(path).map_err(|e| fail("readlink", path, &e))?)
            }
            Keep::Dir(_) => Held::Dir,
        });
    }
    let data = format!("mode=0755,uid={uid},gid={gid}");
    for point in &plan.points {
        make_dirs(point, uid, gid).map_err(|e| fail("mkdir", point, &e))?;
        super::guest::mount_with(
            "tmpfs",
            &point.to_string_lossy(),
            "tmpfs",
            libc::MS_NOSUID | libc::MS_NODEV,
            &data,
        )
        .map_err(|e| fail("tmpfs", point, &e))?;
    }
    for (keep, held) in plan.keep.iter().zip(held) {
        let path = keep.path();
        if let Some(parent) = path.parent() {
            make_dirs(parent, uid, gid).map_err(|e| fail("mkdir", parent, &e))?;
        }
        match held {
            Held::Tree(tree) => {
                mount_point_for(&tree, path, uid, gid).map_err(|e| fail("point", path, &e))?;
                move_tree(&tree, path).map_err(|e| fail("move", path, &e))?;
            }
            Held::Link(target) => {
                std::os::unix::fs::symlink(&target, path).map_err(|e| fail("link", path, &e))?;
                std::os::unix::fs::lchown(path, Some(uid), Some(gid))
                    .map_err(|e| fail("chown", path, &e))?;
            }
            Held::Dir => make_dirs(path, uid, gid).map_err(|e| fail("mkdir", path, &e))?,
        }
    }
    Ok(())
}

/// Creates `path` and every missing parent, each 0755 and owned by `uid`
/// and `gid`; what exists stays as it is.
fn make_dirs(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    let mut missing = Vec::new();
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(err) if err.kind() == io::ErrorKind::NotFound => missing.push(ancestor),
            Err(err) => return Err(err),
        }
    }
    for dir in missing.into_iter().rev() {
        fs::DirBuilder::new().mode(0o755).create(dir)?;
        // The mode whatever the umask, as bwrap sets it.
        fs::set_permissions(dir, fs::Permissions::from_mode(0o755))?;
        std::os::unix::fs::chown(dir, Some(uid), Some(gid))?;
    }
    Ok(())
}

/// The mount point of a kept copy: a directory for a directory, an empty
/// file for anything else, as bwrap creates them.
fn mount_point_for(tree: &OwnedFd, path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    use std::os::fd::AsRawFd as _;
    // SAFETY: stat is plain data; all zero is a valid value.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: a descriptor this function borrows and a valid stat buffer.
    if unsafe { libc::fstat(tree.as_raw_fd(), &raw mut stat) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if stat.st_mode & libc::S_IFMT == libc::S_IFDIR {
        return make_dirs(path, uid, gid);
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o444)
        .open(path)?;
    std::os::unix::fs::chown(path, Some(uid), Some(gid))
}

/// A detached copy of the mount at `path` (`open_tree(2)`, not recursive:
/// the shared root is one mount in the guest).
fn clone_tree(path: &Path) -> io::Result<OwnedFd> {
    let path = c_path(path)?;
    let flags: libc::c_uint = libc::OPEN_TREE_CLONE | libc::OPEN_TREE_CLOEXEC;
    // SAFETY: a NUL-terminated path that outlives the call, and integers.
    let fd = unsafe { libc::syscall(libc::SYS_open_tree, libc::AT_FDCWD, path.as_ptr(), flags) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = i32::try_from(fd).map_err(|_| io::Error::from_raw_os_error(libc::EBADF))?;
    // SAFETY: open_tree just returned this descriptor, and nothing else owns it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Moves the detached copy `tree` onto `path` (`move_mount(2)`).
fn move_tree(tree: &OwnedFd, path: &Path) -> io::Result<()> {
    use std::os::fd::AsRawFd as _;
    let path = c_path(path)?;
    // SAFETY: two NUL-terminated strings that outlive the call, a descriptor
    // this function borrows, and integers.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            tree.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_FDCWD,
            path.as_ptr(),
            MOVE_MOUNT_F_EMPTY_PATH,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn c_path(path: &Path) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt as _;
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL byte"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn absent_keys_mean_no_tmpfs() {
        assert_eq!(parse(None, None).unwrap(), Tmpfs::default());
    }

    #[test]
    fn the_documented_shape_reads_in_order() {
        let plan = parse(
            Some(r#"["/tmp", "/var/tmp", "/home/agent"]"#),
            Some(
                r#"["bind:/home/agent/.config/a.json", "link:/home/agent/l", "dir:/home/agent/w"]"#,
            ),
        )
        .unwrap();
        assert_eq!(
            plan.points,
            ["/tmp", "/var/tmp", "/home/agent"].map(PathBuf::from)
        );
        assert_eq!(
            plan.keep,
            [
                Keep::Bind(PathBuf::from("/home/agent/.config/a.json")),
                Keep::Link(PathBuf::from("/home/agent/l")),
                Keep::Dir(PathBuf::from("/home/agent/w")),
            ]
        );
    }

    #[test]
    fn every_deviation_is_refused_with_its_key() {
        let cases = [
            (Some(r#"["tmp"]"#), None, "tmpfs:not-a-plain-path"),
            (Some(r#"["/"]"#), None, "tmpfs:not-a-plain-path"),
            (Some(r#"["/tmp/../etc"]"#), None, "tmpfs:not-a-plain-path"),
            (Some(r#"["/tmp/./x"]"#), None, "tmpfs:not-a-plain-path"),
            (Some(r#"["/tmp//x"]"#), None, "tmpfs:not-a-plain-path"),
            (Some(r#"["/tmp/"]"#), None, "tmpfs:not-a-plain-path"),
            (Some(r#""/tmp""#), None, "tmpfs:"),
            (None, Some(r#"["/home/agent/x"]"#), "tmpfs_keep:no-kind"),
            (None, Some(r#"["file:/x"]"#), "tmpfs_keep:unknown-kind"),
            (None, Some(r#"["bind:x"]"#), "tmpfs_keep:not-a-plain-path"),
            (
                None,
                Some(r#"["bind:/a/../b"]"#),
                "tmpfs_keep:not-a-plain-path",
            ),
        ];
        for (points, keep, needle) in cases {
            let err = parse(points, keep).unwrap_err();
            assert!(err.starts_with(needle), "{points:?} {keep:?}: {err}");
            assert!(!err.contains(' '), "{err}");
        }
    }

    /// `make_dirs` creates only what is missing, each level 0755 whatever
    /// the umask, owned by the given ids.
    #[test]
    fn make_dirs_creates_the_missing_parents() {
        use std::os::unix::fs::MetadataExt as _;
        let dir = std::env::temp_dir().join(format!("hum466-tmpfs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        // SAFETY: getuid and getgid take nothing and cannot fail.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        let deep = dir.join("a/b/c");
        make_dirs(&deep, uid, gid).unwrap();
        for level in ["a", "a/b", "a/b/c"] {
            let meta = fs::metadata(dir.join(level)).unwrap();
            assert!(meta.is_dir(), "{level}");
            assert_eq!(meta.permissions().mode() & 0o7777, 0o755, "{level}");
            assert_eq!((meta.uid(), meta.gid()), (uid, gid), "{level}");
        }
        make_dirs(&deep, uid, gid).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }
}
