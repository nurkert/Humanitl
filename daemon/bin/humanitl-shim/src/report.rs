//! The check report: one line per isolation check, written to the descriptor
//! the launcher names in `HUMANITL_REPORT_FD`.
//!
//! The launcher (HUM-011) inherits the write end of a pipe into the shim and
//! reads the other end; the isolation panel (HUM-041) turns the lines into
//! `IsolationCheck` results. Evidence that comes from inside the sandbox is
//! the point: a host claiming something about a sandbox it cannot see would
//! be no proof (`docs/SECURITY.md`).
//!
//! Line format, one line per check, each written with a single `write(2)`
//! so lines from the parent and from the child never interleave:
//!
//! ```text
//! CHECK <name> <ok|fail> <evidence>
//! ```
//!
//! `name` is one of [`Check`]'s names; `evidence` is free text without
//! whitespace (the report sanitises it), so a reader may split on spaces.
//! Without `HUMANITL_REPORT_FD` there is no report and nothing is written.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

/// The five checks the shim reports, in the order they are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// The `in` bridges accept connections (parent, before the fork).
    BridgeListening,
    /// No network interface but `lo` exists (parent, before the fork).
    NoInterfaces,
    /// No Unix socket exists in the sandbox's filesystem but the ones the
    /// bridges serve (parent, before the fork; see [`sockets`]).
    SingleSocket,
    /// The child carries `Seccomp: 2` and `NoNewPrivs: 1` (child, after the
    /// filter, before `exec`).
    SeccompApplied,
    /// `socket(2)` refuses what the policy refuses and allows what it allows
    /// (child, after the filter, before `exec`).
    Families,
}

impl Check {
    /// The name in the report line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::BridgeListening => "bridge_listening",
            Self::NoInterfaces => "no_interfaces",
            Self::SingleSocket => "single_socket",
            Self::SeccompApplied => "seccomp_applied",
            Self::Families => "families",
        }
    }
}

/// Where the report goes: a descriptor the launcher opened, or nowhere.
#[derive(Debug)]
pub struct Report {
    fd: Option<OwnedFd>,
}

/// Why `HUMANITL_REPORT_FD` could not be used. Never fatal: the shim warns
/// and runs without a report, and the launcher notices the missing lines
/// (`SANDBOX_013`).
#[derive(Debug)]
pub enum Error {
    /// The value is not a decimal descriptor number.
    NotANumber(String),
    /// The descriptor is 0, 1 or 2.
    Reserved(RawFd),
    /// The descriptor is not open.
    NotOpen(RawFd, io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotANumber(value) => {
                write!(f, "HUMANITL_REPORT_FD={value:?} is not a descriptor number")
            }
            Self::Reserved(fd) => write!(f, "HUMANITL_REPORT_FD={fd} is a standard stream"),
            Self::NotOpen(fd, err) => write!(f, "HUMANITL_REPORT_FD={fd} is not open: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl Report {
    /// No report.
    #[must_use]
    pub const fn none() -> Self {
        Self { fd: None }
    }

    /// The report the launcher asked for, from the value of
    /// `HUMANITL_REPORT_FD`; `None` (variable absent) means no report.
    ///
    /// Takes ownership of the descriptor: it is closed when the parent drops
    /// the report after the fork, and by `exec` in the child (`CLOEXEC`).
    pub fn from_env(value: Option<&OsStr>) -> Result<Self, Error> {
        let Some(value) = value else {
            return Ok(Self::none());
        };
        let text = value.to_string_lossy();
        let fd: RawFd = text
            .trim()
            .parse()
            .map_err(|_| Error::NotANumber(text.into_owned()))?;
        if fd < 3 {
            return Err(Error::Reserved(fd));
        }
        // SAFETY: F_GETFD reads a flag of the descriptor and touches nothing.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(Error::NotOpen(fd, io::Error::last_os_error()));
        }
        // SAFETY: the launcher handed this descriptor to the shim for exactly
        // this purpose; nothing else in the process refers to it.
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(Self { fd: Some(owned) })
    }

    /// The descriptor, when there is a report.
    #[must_use]
    pub fn fd(&self) -> Option<RawFd> {
        self.fd.as_ref().map(AsRawFd::as_raw_fd)
    }

    /// Writes one `CHECK` line. Errors are ignored: a launcher that stopped
    /// reading is its own problem, and the shim's job is the agent.
    pub fn check(&self, check: Check, ok: bool, evidence: &str) {
        let Some(fd) = self.fd.as_ref() else {
            return;
        };
        let line = format!(
            "CHECK {} {} {}\n",
            check.name(),
            if ok { "ok" } else { "fail" },
            sanitize(evidence)
        );
        write_all(fd.as_raw_fd(), line.as_bytes());
    }
}

/// Evidence without whitespace or control characters, never empty.
fn sanitize(evidence: &str) -> String {
    let cleaned: String = evidence
        .chars()
        .map(|c| match c {
            c if c.is_whitespace() => '_',
            c if c.is_control() => '?',
            c => c,
        })
        .collect();
    if cleaned.is_empty() {
        "-".to_owned()
    } else {
        cleaned
    }
}

fn write_all(fd: RawFd, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        // SAFETY: the pointer and length describe `bytes`, which outlives the
        // call; the descriptor is owned by the report.
        let n = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        match usize::try_from(n) {
            Ok(0) => return,
            Ok(n) => bytes = &bytes[n.min(bytes.len())..],
            Err(_) => {
                if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    return;
                }
            }
        }
    }
}

/// The network interfaces of this network namespace, sorted.
///
/// `/sys/class/net` first; the sandbox has no `/sys`, so `/proc/net/dev` is
/// the path that matters there.
pub fn interfaces() -> io::Result<Vec<String>> {
    let mut names: Vec<String> = match fs::read_dir("/sys/class/net") {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => fs::read_to_string("/proc/net/dev")?
            .lines()
            .skip(2)
            .filter_map(|line| line.split(':').next())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect(),
    };
    names.sort();
    Ok(names)
}

/// How deep the socket walk goes: an entry in `/a/b/c` sits at depth 3 and is
/// still looked at, deeper ones are not.
///
/// The same bound the host-side profile check uses
/// (`humanitl_sandbox::SOCKET_WALK_MAX_DEPTH`); the shim has no dependency on
/// that crate, so the number is written out here. Three levels reach
/// `/run/humanitl/proxy.sock`, the one socket the sandbox is supposed to have.
pub const SOCKET_WALK_MAX_DEPTH: usize = 3;

/// How many directory entries the socket walk looks at before it stops.
///
/// The same bound as `humanitl_sandbox::SOCKET_WALK_MAX_ENTRIES`. A sandbox
/// with `/usr` bound read-only has far more entries than this, so the walk
/// normally stops early; that it stopped is part of the evidence, never
/// silently dropped ([`SocketWalk::limit`]).
pub const SOCKET_WALK_MAX_ENTRIES: usize = 2000;

/// Directories the socket walk does not enter.
///
/// `/proc` and `/sys` are kernel interfaces, not the filesystem the sandbox
/// carries, and walking them costs a lot for nothing; `/dev` is bwrap's
/// minimal device filesystem. None of the three can hold a bind-mounted host
/// socket, because the profile's mount denylist refuses all three as sources
/// (`humanitl_sandbox::FORBIDDEN_MOUNTS`).
pub const SOCKET_WALK_SKIP: [&str; 3] = ["/proc", "/sys", "/dev"];

/// The one place under [`SOCKET_WALK_SKIP`] the walk does look at.
///
/// `/dev/shm` is a writable tmpfs and the one spot under `/dev` where an agent
/// can bind a socket of its own; skipping it with the rest of `/dev` would
/// hide exactly what the walk is for. `tests/escape/lib.sh` learned the same
/// thing the hard way and says so at `esc_find_sockets`.
pub const SOCKET_WALK_ALSO: [&str; 1] = ["/dev/shm"];

/// What the socket walk saw.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SocketWalk {
    /// Every Unix socket the walk found, sorted, as printable paths.
    pub sockets: Vec<String>,
    /// How many directory entries were looked at.
    pub entries: usize,
    /// Which bound ended the walk, if one did: `"entries"` or `"depth"`.
    /// A bound that was hit weakens the evidence, so it is reported.
    pub limit: Option<&'static str>,
}

/// Walks the filesystem for Unix sockets, breadth first, within the bounds.
///
/// The evidence for the second guarantee has to come from inside the sandbox:
/// the listener accepting a connection (`bridge_listening`) only shows that
/// the one door is open, not that there is no second one. So the shim looks,
/// before it hands the process over to the agent.
///
/// Deliberately narrow: symbolic links are never followed (a link into a
/// directory outside the walk would leave the sandbox's own filesystem and
/// could loop), [`SOCKET_WALK_SKIP`] is not entered except for
/// [`SOCKET_WALK_ALSO`], and the two bounds keep the walk short enough to sit
/// in front of every launch. Directories are read in sorted order and level by
/// level, so that the shallow, interesting places (`/run/humanitl`) come
/// before the deep, dull ones (`/usr/bin`) when the entry budget runs out.
///
/// The type of an entry comes from `lstat(2)`, never from the directory entry:
/// bwrap bind-mounts the proxy socket over an empty regular file, and
/// `readdir`'s `d_type` still says "regular file" for that mount point. ESC-2
/// hit the same trap and answers it with `find -xtype s`
/// (`tests/escape/lib.sh`).
#[must_use]
pub fn sockets() -> SocketWalk {
    let mut walk = SocketWalk::default();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((PathBuf::from("/"), 0));
    for path in SOCKET_WALK_ALSO {
        let depth = Path::new(path).components().count().saturating_sub(1);
        queue.push_back((PathBuf::from(path), depth));
    }

    while let Some((dir, depth)) = queue.pop_front() {
        // Entries of this directory sit one level deeper; below the bound
        // nothing is read, and that the bound was reached is evidence.
        if depth + 1 > SOCKET_WALK_MAX_DEPTH {
            walk.limit.get_or_insert("depth");
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        names.sort();

        for path in names {
            walk.entries += 1;
            if walk.entries > SOCKET_WALK_MAX_ENTRIES {
                walk.entries = SOCKET_WALK_MAX_ENTRIES;
                walk.limit = Some("entries");
                queue.clear();
                break;
            }
            // `lstat`, not the directory entry: it describes the link itself,
            // so a symlink is never followed, and it sees the socket behind a
            // bind mount whose `d_type` still says "regular file".
            let Ok(kind) = fs::symlink_metadata(&path).map(|meta| meta.file_type()) else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_socket() {
                walk.sockets.push(path.to_string_lossy().into_owned());
            } else if kind.is_dir() && !SOCKET_WALK_SKIP.iter().any(|skip| path == Path::new(skip))
            {
                queue.push_back((path, depth + 1));
            }
        }
    }
    walk.sockets.sort();
    walk
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::OsString;
    use std::io::Read;

    use super::*;

    fn pipe() -> (OwnedFd, OwnedFd) {
        let mut fds = [0 as libc::c_int; 2];
        // SAFETY: `fds` is a valid two-element array for pipe2.
        let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(rc, 0);
        // SAFETY: both descriptors were just created and belong to this test.
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
    }

    #[test]
    fn writes_one_line_per_check_and_sanitises_evidence() {
        let (reader, writer) = pipe();
        let value = OsString::from(writer.as_raw_fd().to_string());
        let report = Report::from_env(Some(&value)).unwrap();
        // The report owns the descriptor now.
        std::mem::forget(writer);
        assert!(report.fd().is_some());
        report.check(
            Check::BridgeListening,
            true,
            "proxy=127.0.0.1:3128->/run/humanitl/proxy.sock",
        );
        report.check(Check::NoInterfaces, false, "lo eth0\tbad\n");
        report.check(Check::SeccompApplied, true, "");
        drop(report);
        let mut text = String::new();
        fs::File::from(reader).read_to_string(&mut text).unwrap();
        assert_eq!(
            text,
            "CHECK bridge_listening ok proxy=127.0.0.1:3128->/run/humanitl/proxy.sock\n\
             CHECK no_interfaces fail lo_eth0_bad_\n\
             CHECK seccomp_applied ok -\n"
        );
    }

    #[test]
    fn absent_means_no_report_and_bad_values_are_errors() {
        assert!(Report::from_env(None).unwrap().fd().is_none());
        Report::none().check(Check::Families, true, "nothing happens");
        assert!(matches!(
            Report::from_env(Some(OsStr::new("seven"))),
            Err(Error::NotANumber(_))
        ));
        assert!(matches!(
            Report::from_env(Some(OsStr::new("2"))),
            Err(Error::Reserved(2))
        ));
        let (reader, writer) = pipe();
        let closed = writer.as_raw_fd();
        drop(writer);
        let value = OsString::from(closed.to_string());
        assert!(matches!(
            Report::from_env(Some(&value)),
            Err(Error::NotOpen(fd, _)) if fd == closed
        ));
        drop(reader);
    }

    /// The walk sees a socket in a place it reaches, keeps its bounds and
    /// stays out of `/proc`.
    #[test]
    fn the_socket_walk_finds_a_socket_within_its_bounds() {
        // Direkt unter /tmp, nicht unter TMPDIR: der Suchlauf reicht drei
        // Ebenen tief, und ein TMPDIR tiefer im Dateisystem läge außerhalb.
        let path =
            Path::new("/tmp").join(format!("humanitl-shim-walk-{}.sock", std::process::id()));
        let _ = fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();

        let walk = sockets();
        assert!(
            walk.sockets.iter().any(|found| Path::new(found) == path),
            "{:?} not among {} sockets",
            path,
            walk.sockets.len()
        );
        assert!(walk.entries <= SOCKET_WALK_MAX_ENTRIES);
        assert!(
            !walk.sockets.iter().any(|found| found.starts_with("/proc/")),
            "the walk does not enter /proc: {:?}",
            walk.sockets
        );
        assert!(
            walk.sockets.windows(2).all(|pair| pair[0] <= pair[1]),
            "the evidence is sorted"
        );

        drop(listener);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn interfaces_include_loopback() {
        let names = interfaces().unwrap();
        assert!(names.iter().any(|name| name == "lo"), "{names:?}");
        assert!(names.windows(2).all(|w| w[0] <= w[1]));
    }
}
