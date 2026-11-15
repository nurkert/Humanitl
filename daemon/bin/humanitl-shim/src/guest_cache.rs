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
//! the daemon closes the descriptor. A freed entry also lets its directory
//! go when it was the last thing holding it, so a chain of directories
//! leaves at once with its lowest member. What is in use stays: open files,
//! working directories, mapped libraries. Those the agent pins also under
//! `--cache never`, and the limit of the unit binds them as before.
//!
//! **Why the unused count is not enough** (HUM-483). The kernel counts an
//! entry as unused from the moment its last reference goes and keeps
//! counting it when a reference comes back, until a trim looks at it; a
//! trim takes an entry that is in use off the count for good. A directory
//! held by a child that is cached, but not in use, is therefore missed when
//! a trim met it while it was in use: a chain of directories that a working
//! directory or an open descriptor held during the trim, and that was let
//! go afterwards. Only its lowest member counts again, and the chain stays
//! until the next trim, which comes only after [`UNUSED_LIMIT`] new unused
//! entries. So PID 1 also reads how many inodes the guest keeps in all
//! (`inode-state`, the first field), and trims when that exceeds
//! [`HELD_LIMIT`] and has grown by [`HELD_STEP`] since the last trim, but at
//! [`HELD_CEIL`] at the latest, below the daemons' limit. The step keeps
//! PID 1 from trimming every [`PERIOD`] when the guest holds many inodes
//! that no trim frees: files in a tmpfs, open descriptors, working
//! directories; above the ceiling a trim for the held inodes that freed
//! fewer than [`HELD_STEP`] is repeated only after
//! [`HELD_COOLDOWN_LOOKS`] looks. What is in use the bound never reaches; a guest that deliberately
//! holds as many inodes of a daemon as its limit gets `ENFILE`, the host
//! stays untouched.
//!
//! **What the agent can do with it.** The three files are opened once at
//! boot, before the agent exists, with `O_CLOEXEC`, and only this process
//! holds them: the agent's child drops them and closes every inherited
//! descriptor, and PID 1 is not dumpable, so neither `/proc/1/fd` nor
//! `pidfd_getfd(2)` reach them (the KVM test
//! `the_agent_holds_none_of_pid_1s_cache_descriptors` of the sandbox crate,
//! HUM-483). The thread reads three numbers and writes one constant;
//! nothing the agent writes is parsed beyond three integers, and the only
//! effect is that the guest kernel forgets cached entries. The agent can
//! make the thread trim often, which costs only its own speed, and it can
//! still pin entries by holding them, which the unit's limit stops on the
//! host. The bound is the
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

/// How many inodes the guest may hold in all, of every file system, before
/// PID 1 trims although few are unused: three quarters of the file daemons'
/// descriptor limit (HUM-483). Above [`UNUSED_LIMIT`], so a cache full of
/// entries in use, such as the working set of a build, is not trimmed for
/// nothing.
pub const HELD_LIMIT: u64 = 49_152;

/// How much the inodes held in all must have grown since the last trim
/// before PID 1 trims again for [`HELD_LIMIT`]: what the last trim left is
/// what it could not free, and trimming again frees only what came since.
pub const HELD_STEP: u64 = 8_192;

/// The highest the threshold of the held inodes ever goes, however much the
/// last trim left: the descriptor limit (65 536) less [`HELD_STEP`]. Without
/// it a trim that left 60 000 held inodes would set the next one at 68 192,
/// beyond the limit, and a chain built after the guest let go of them would
/// reach `ENFILE` first (review of HUM-483).
pub const HELD_CEIL: u64 = 65_536 - HELD_STEP;

const _: () = assert!(HELD_LIMIT < HELD_CEIL && UNUSED_LIMIT < HELD_LIMIT);

/// How many looks PID 1 waits after a trim for the held inodes that freed
/// fewer than [`HELD_STEP`], before it trims for them again: ten, one second
/// at [`PERIOD`]. Above [`HELD_CEIL`] a guest that holds its inodes in use
/// would otherwise make PID 1 trim every look for nothing, and each trim
/// throws away the whole unused cache, which the guest then reads anew from
/// the daemons. Once a second costs at most a tenth of that, and a guest
/// that already holds more than [`HELD_CEIL`] inodes in use is at the limit
/// on purpose, which the unit binds on the host. The unused count trims
/// regardless.
pub const HELD_COOLDOWN_LOOKS: u32 = 10;

/// How much the held inodes must grow during the cooldown, above
/// [`HELD_CEIL`], before PID 1 trims anyway: what the guest built since the
/// stalled trim can be freed, and a walk adds a few thousand entries per look,
/// so waiting the rest of the second could let it reach `ENFILE` first (delta
/// reviews of HUM-483). Below the ceiling the cooldown ends a
/// [`HELD_STEP`] above what the trim left, as without one. A guest whose
/// count does not grow stays at once a second.
pub const HELD_REGROW: u64 = 512;

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

/// The guest's cache at one look: unused dentries plus unused inodes, and
/// the inodes held in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counts {
    unused: u64,
    held: u64,
}

/// When to trim (HUM-476, HUM-483), apart from reading the counts.
#[derive(Debug)]
struct Trigger {
    /// What the guest held right after the last trim, lowered whenever it
    /// holds fewer.
    floor: u64,
    /// Looks since the last trim.
    looks: u32,
    /// The last trim was one for the held inodes and freed fewer than
    /// [`HELD_STEP`]: the next waits [`HELD_COOLDOWN_LOOKS`].
    stalled: bool,
    /// What the guest held when [`Trigger::wants`] last said yes, and
    /// whether for the held inodes.
    asked: Option<(u64, bool)>,
}

impl Trigger {
    /// Starts from what the guest holds at boot.
    const fn new(held: u64) -> Self {
        Self {
            floor: held,
            looks: 0,
            stalled: false,
            asked: None,
        }
    }

    /// Where the held inodes call for a trim: above [`HELD_LIMIT`] and
    /// [`HELD_STEP`] above the floor, never above [`HELD_CEIL`].
    fn held_threshold(&self) -> u64 {
        HELD_LIMIT
            .max(self.floor.saturating_add(HELD_STEP))
            .min(HELD_CEIL)
    }

    /// Where the held inodes end a cooldown: [`HELD_STEP`] above the floor,
    /// but above [`HELD_CEIL`] already [`HELD_REGROW`] above it.
    fn urgent_at(&self) -> u64 {
        self.floor
            .saturating_add(HELD_STEP)
            .min(self.floor.saturating_add(HELD_REGROW).max(HELD_CEIL))
    }

    /// Whether these counts call for a trim; one call per look.
    fn wants(&mut self, counts: Counts) -> bool {
        self.looks = self.looks.saturating_add(1);
        self.floor = self.floor.min(counts.held);
        let held = if counts.unused > UNUSED_LIMIT {
            false
        } else if counts.held > self.held_threshold()
            && (!self.stalled
                || self.looks >= HELD_COOLDOWN_LOOKS
                || counts.held >= self.urgent_at())
        {
            true
        } else {
            return false;
        };
        self.asked = Some((counts.held, held));
        true
    }

    /// Records what the guest still holds after the trim that
    /// [`Trigger::wants`] asked for.
    fn trimmed(&mut self, held: u64) {
        let (before, for_held) = self.asked.take().unwrap_or((held, false));
        self.stalled = for_held && before.saturating_sub(held) < HELD_STEP;
        self.floor = held;
        self.looks = 0;
    }
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
        bound.counts().map_err(|err| fail("read", &err))?;
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

    /// The counts, now.
    fn counts(&self) -> io::Result<Counts> {
        let dentries = read(&self.dentries)?;
        let inodes = read(&self.inodes)?;
        Ok(Counts {
            unused: field(&dentries, 1)?.saturating_add(field(&inodes, 1)?),
            held: field(&inodes, 0)?,
        })
    }

    /// One look: trims when the [`Trigger`] says so.
    fn look(&self, trigger: &mut Trigger) -> io::Result<()> {
        if trigger.wants(self.counts()?) {
            self.drop.write_at(DROP_SLAB, 0)?;
            trigger.trimmed(self.counts()?.held);
        }
        Ok(())
    }

    /// Starts the thread that looks every [`PERIOD`], for as long as PID 1
    /// lives. Writes `CACHE bound <unused limit> <held limit>` on the
    /// report, or `CACHE off <why>` when the thread cannot start or a look
    /// fails; then the cache grows unbounded until the daemon's limit, as
    /// the module documentation says.
    pub fn start(self, report: &Arc<Report>) {
        let thread_report = Arc::clone(report);
        let started = std::thread::Builder::new()
            .name("file-cache".to_owned())
            .spawn(move || {
                let mut trigger = Trigger::new(match self.counts() {
                    Ok(counts) => counts.held,
                    Err(err) => {
                        thread_report.line(&format!("CACHE off look:{}", spaceless(&err)));
                        return;
                    }
                });
                loop {
                    std::thread::sleep(PERIOD);
                    if let Err(err) = self.look(&mut trigger) {
                        thread_report.line(&format!("CACHE off look:{}", spaceless(&err)));
                        return;
                    }
                }
            });
        match started {
            Ok(_) => report.line(&format!("CACHE bound {UNUSED_LIMIT} {HELD_LIMIT}")),
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

/// Field `index` of `dentry-state` or `inode-state`, counted from 0: the
/// first is how many the guest holds, the second how many of them are
/// unused.
fn field(text: &str, index: usize) -> io::Result<u64> {
    text.split_ascii_whitespace()
        .nth(index)
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no such field"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn the_first_field_is_held_and_the_second_unused() {
        // dentry-state: nr_dentry nr_unused age_limit want_pages nr_negative dummy
        assert_eq!(field("51234\t40000\t45\t0\t1200\t0\n", 1).unwrap(), 40_000);
        // inode-state: nr_inodes nr_free_inodes preshrink 0 0 0 0
        assert_eq!(field("9000\t12\t0\t0\t0\t0\t0\n", 0).unwrap(), 9_000);
        assert_eq!(field("9000\t12\t0\t0\t0\t0\t0\n", 1).unwrap(), 12);
        assert!(field("", 0).is_err());
        assert!(field("1", 1).is_err());
        assert!(field("1 x 3", 1).is_err());
    }

    fn counts(unused: u64, held: u64) -> Counts {
        Counts { unused, held }
    }

    #[test]
    fn many_unused_entries_trim() {
        let mut trigger = Trigger::new(2_000);
        assert!(!trigger.wants(counts(UNUSED_LIMIT, 40_000)));
        assert!(trigger.wants(counts(UNUSED_LIMIT + 1, 40_000)));
    }

    /// HUM-483: a chain that a trim met while it was held, and that was let
    /// go afterwards, counts as one unused entry; the inodes held in all
    /// still call for the trim that frees it.
    #[test]
    fn many_held_inodes_trim_though_few_are_unused() {
        let mut trigger = Trigger::new(2_000);
        assert!(!trigger.wants(counts(100, HELD_LIMIT)));
        assert!(trigger.wants(counts(100, HELD_LIMIT + 1)));
    }

    /// What the last trim could not free does not trim again every look,
    /// only once the guest holds [`HELD_STEP`] more.
    #[test]
    fn what_a_trim_left_trims_again_only_after_a_step() {
        let mut trigger = Trigger::new(2_000);
        assert!(trigger.wants(counts(100, 62_000)));
        trigger.trimmed(45_000);
        assert!(!trigger.wants(counts(100, 45_000)));
        assert!(!trigger.wants(counts(100, 45_000 + HELD_STEP)));
        assert!(trigger.wants(counts(100, 45_000 + HELD_STEP + 1)));
        // The unused count trims regardless.
        assert!(trigger.wants(counts(UNUSED_LIMIT + 1, 45_000)));
    }

    /// HUM-483 review: however much the last trim left, the held inodes
    /// trim at [`HELD_CEIL`] at the latest.
    #[test]
    fn a_high_floor_still_trims_below_the_limit() {
        let mut trigger = Trigger::new(2_000);
        trigger.trimmed(60_000);
        assert!(!trigger.wants(counts(100, HELD_CEIL)));
        assert!(trigger.wants(counts(100, HELD_CEIL + 1)));
    }

    /// A trim for the held inodes that frees next to nothing is repeated
    /// only after [`HELD_COOLDOWN_LOOKS`] looks; one that freed enough, or
    /// one for the unused entries, is not held back.
    #[test]
    fn a_trim_that_frees_nothing_waits_before_the_next() {
        let mut trigger = Trigger::new(2_000);
        assert!(trigger.wants(counts(100, 62_000)));
        trigger.trimmed(61_000);
        // The guest holds what it holds: no trim for nine looks.
        for _ in 1..HELD_COOLDOWN_LOOKS {
            assert!(!trigger.wants(counts(100, 61_000)));
        }
        assert!(trigger.wants(counts(100, 61_000)));
        trigger.trimmed(60_900);
        // Stalled again; the unused count still trims at once.
        assert!(trigger.wants(counts(UNUSED_LIMIT + 1, 60_900)));
        trigger.trimmed(60_800);
        // After a trim for the unused entries the held inodes may trim.
        assert!(trigger.wants(counts(100, 60_800)));
        trigger.trimmed(30_000);
        // That one freed plenty: the next follows without waiting.
        assert!(trigger.wants(counts(100, HELD_LIMIT + 1)));
    }

    /// A guest that builds new entries during the cooldown ends it: above
    /// the ceiling [`HELD_REGROW`] past what the stalled trim left, so a fast
    /// walk cannot reach the limit while PID 1 waits; a count that stays
    /// waits the second. Mutation: without the clause the trim after the
    /// growth stays quiet.
    #[test]
    fn growth_during_the_cooldown_trims_at_once() {
        let mut trigger = Trigger::new(2_000);
        assert!(trigger.wants(counts(100, 58_500)));
        trigger.trimmed(58_000);
        // Nothing grew: the cooldown holds.
        assert!(!trigger.wants(counts(100, 58_000)));
        assert!(!trigger.wants(counts(100, 58_000 + HELD_REGROW - 1)));
        // A walk built more: it trims at once, long before the limit.
        assert!(trigger.wants(counts(100, 58_000 + HELD_REGROW)));
        trigger.trimmed(63_000);
        // Stalled near the limit with a count that stays: no trim each look.
        for _ in 1..HELD_COOLDOWN_LOOKS {
            assert!(!trigger.wants(counts(100, 63_000)));
        }
    }

    /// Once the guest holds fewer, the step starts from there.
    #[test]
    fn the_floor_follows_the_guest_down() {
        let mut trigger = Trigger::new(2_000);
        trigger.trimmed(58_000);
        assert!(!trigger.wants(counts(100, 50_000)));
        assert!(trigger.wants(counts(100, 50_000 + HELD_STEP + 1)));
        assert!(!trigger.wants(counts(100, 40_000)));
        assert!(trigger.wants(counts(100, HELD_LIMIT + 1)));
    }

    #[test]
    fn the_limits_leave_part_of_the_descriptors() {
        // FILE_DAEMON_NOFILE of the sandbox crate; the shim does not depend
        // on it, so the number is repeated here and in the documentation.
        assert_eq!(UNUSED_LIMIT * 2, 65_536);
        assert_eq!(HELD_LIMIT * 4, 65_536 * 3);
        const { assert!(HELD_LIMIT + HELD_STEP < 65_536) };
    }
}
