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

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

/// The four checks the shim reports, in the order they are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// The `in` bridges accept connections (parent, before the fork).
    BridgeListening,
    /// No network interface but `lo` exists (parent, before the fork).
    NoInterfaces,
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

    #[test]
    fn interfaces_include_loopback() {
        let names = interfaces().unwrap();
        assert!(names.iter().any(|name| name == "lo"), "{names:?}");
        assert!(names.windows(2).all(|w| w[0] <= w[1]));
    }
}
