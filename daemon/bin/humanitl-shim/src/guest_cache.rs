//! The guest's file cache, bounded (HUM-476).
//!
//! Both file daemons of the kernel level hold one descriptor for every inode
//! the guest kernel knows, and the unit gives each of them a fixed number
//! (`FILE_DAEMON_NOFILE` in the sandbox crate, HUM-469). With the file cache
//! on (`--cache auto`) the guest keeps every entry it has looked up until its
//! memory runs short, and with a few hundred MiB that is far beyond the
//! limit: a plain `ls -lR` over 81 000 entries ended with `ENFILE` in
//! HUM-476. Nothing else lets an entry go early: the daemons know no option
//! that bounds their inodes, `vfs_cache_pressure` acts only when memory is
//! short (measured: no change), and handles instead of descriptors need a
//! capability on the host.
//!
//! So PID 1 bounds the cache itself. Every [`PERIOD`] it reads how many
//! dentries and inodes the guest kernel keeps without anybody using them
//! (`/proc/sys/fs/dentry-state` and `inode-state`, the second field each),
//! and when their sum exceeds [`UNUSED_LIMIT`] it writes `2` to
//! `/proc/sys/vm/drop_caches`: the kernel frees every unused dentry and
//! inode, and for each inode of a file daemon it sends `FORGET`, upon which
//! the daemon closes the descriptor. What is in use stays: open files, working
//! directories, mapped libraries. Those the agent pins also under
//! `--cache never`, and the limit of the unit binds them as before.
//!
//! **What the agent can do with it.** The three files are opened once at
//! boot, before the agent exists, with `O_CLOEXEC`, and only this process
//! holds them: the agent's child closes every inherited descriptor, and PID 1
//! is not dumpable, so neither `/proc/1/fd` nor `pidfd_getfd(2)` reach them.
//! The thread reads two numbers and writes one constant; nothing the agent
//! writes is parsed beyond two integers, and the only effect is that the
//! guest kernel forgets cached entries. The agent can make the thread trim
//! often, which costs only its own speed, and it can still pin entries by
//! holding them, which the unit's limit stops on the host. The bound is the
//! guest's own comfort, not a guarantee: should it fail, the cache grows
//! until the daemon answers `ENFILE`, and the host stays untouched.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::FileExt as _;
use std::sync::Arc;
use std::time::Duration;

use crate::report::Report;

/// How many unused dentries and inodes together the guest may keep before
/// PID 1 lets them go: half of the file daemons' descriptor limit (65 536).
/// Both daemons together never hold more than the guest knows, so each
/// stays below the limit by the other half, less what the guest grows in one
/// [`PERIOD`] and what it holds open. Large enough that the working set of
/// `git status` over a tree of 13 000 files stays cached between two runs.
pub const UNUSED_LIMIT: u64 = 32_768;

/// How often PID 1 looks. A walk with a warm daemon adds about 40 000
/// entries a second (measured in HUM-476), so a tenth of a second is a few
/// thousand at most beyond [`UNUSED_LIMIT`].
pub const PERIOD: Duration = Duration::from_millis(100);

const DENTRY_STATE: &str = "/proc/sys/fs/dentry-state";
const INODE_STATE: &str = "/proc/sys/fs/inode-state";
const DROP_CACHES: &str = "/proc/sys/vm/drop_caches";

/// `drop_caches`: free the reclaimable slab objects, dentries and inodes.
const DROP_SLAB: &[u8] = b"2\n";

/// `drop_caches`: from now on without a line in the kernel log each time.
const QUIET: &[u8] = b"4\n";

/// The files of the bound, opened at boot.
#[derive(Debug)]
pub struct Bound {
    dentries: File,
    inodes: File,
    drop: File,
}

impl Bound {
    /// Opens the three files and silences the kernel's log line for each
    /// trim. Call it once `/proc` is mounted and before the agent exists.
    ///
    /// # Errors
    ///
    /// The reason, without spaces, for the report.
    pub fn open() -> Result<Self, String> {
        let fail = |what: &str, err: &io::Error| format!("{what}:{}", spaceless(err));
        let dentries = File::open(DENTRY_STATE).map_err(|err| fail("dentry-state", &err))?;
        let inodes = File::open(INODE_STATE).map_err(|err| fail("inode-state", &err))?;
        let drop = OpenOptions::new()
            .write(true)
            .open(DROP_CACHES)
            .map_err(|err| fail("drop_caches", &err))?;
        drop.write_at(QUIET, 0)
            .map_err(|err| fail("drop_caches-quiet", &err))?;
        let bound = Self {
            dentries,
            inodes,
            drop,
        };
        bound.unused().map_err(|err| fail("read", &err))?;
        Ok(bound)
    }

    /// The descriptors, for the list of those that survive the close before
    /// the fork.
    #[must_use]
    pub fn fds(&self) -> [RawFd; 3] {
        [
            self.dentries.as_raw_fd(),
            self.inodes.as_raw_fd(),
            self.drop.as_raw_fd(),
        ]
    }

    /// Unused dentries plus unused inodes, now.
    fn unused(&self) -> io::Result<u64> {
        Ok(unused_of(&read(&self.dentries)?)?.saturating_add(unused_of(&read(&self.inodes)?)?))
    }

    /// One look: trims when the cache holds more than [`UNUSED_LIMIT`]
    /// unused entries.
    fn look(&self) -> io::Result<()> {
        if self.unused()? > UNUSED_LIMIT {
            self.drop.write_at(DROP_SLAB, 0)?;
        }
        Ok(())
    }

    /// Starts the thread that looks every [`PERIOD`], for as long as PID 1
    /// lives. Writes `CACHE bound <limit>` on the report, or `CACHE off
    /// <why>` when the thread cannot start or a look fails; then the cache
    /// grows unbounded until the daemon's limit, as the module documentation
    /// says.
    pub fn start(self, report: &Arc<Report>) {
        let thread_report = Arc::clone(report);
        let started = std::thread::Builder::new()
            .name("file-cache".to_owned())
            .spawn(move || {
                loop {
                    std::thread::sleep(PERIOD);
                    if let Err(err) = self.look() {
                        thread_report.line(&format!("CACHE off look:{}", spaceless(&err)));
                        return;
                    }
                }
            });
        match started {
            Ok(_) => report.line(&format!("CACHE bound {UNUSED_LIMIT}")),
            Err(err) => report.line(&format!("CACHE off thread:{}", spaceless(&err))),
        }
    }
}

/// An error as one field of a report line.
fn spaceless(err: &io::Error) -> String {
    err.to_string().replace(' ', "_")
}

/// The content of a sysctl file, read from its start.
fn read(file: &File) -> io::Result<String> {
    let mut buffer = [0_u8; 128];
    let n = file.read_at(&mut buffer, 0)?;
    Ok(String::from_utf8_lossy(&buffer[..n]).into_owned())
}

/// The second field of `dentry-state` or `inode-state`: the unused ones.
fn unused_of(text: &str) -> io::Result<u64> {
    text.split_ascii_whitespace()
        .nth(1)
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no second field"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn the_second_field_is_the_unused_count() {
        // dentry-state: nr_dentry nr_unused age_limit want_pages nr_negative dummy
        assert_eq!(unused_of("51234\t40000\t45\t0\t1200\t0\n").unwrap(), 40_000);
        // inode-state: nr_inodes nr_free_inodes preshrink 0 0 0 0
        assert_eq!(unused_of("9000\t12\t0\t0\t0\t0\t0\n").unwrap(), 12);
        assert!(unused_of("").is_err());
        assert!(unused_of("1").is_err());
        assert!(unused_of("1 x 3").is_err());
    }

    #[test]
    fn the_limit_leaves_half_the_descriptors() {
        // FILE_DAEMON_NOFILE of the sandbox crate; the shim does not depend
        // on it, so the number is repeated here and in the documentation.
        assert_eq!(UNUSED_LIMIT * 2, 65_536);
    }
}
