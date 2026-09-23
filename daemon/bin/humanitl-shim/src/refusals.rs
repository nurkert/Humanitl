//! Refused `socket(2)` calls of the agent, counted and reported (HUM-138).
//!
//! The agent's filter answers a refused `socket(2)` with
//! `SECCOMP_RET_USER_NOTIF` ([`crate::seccomp::Gate::Reported`]). The kernel
//! parks the call and hands it to whoever holds the listener: the shim's
//! parent, in [`serve`]. The parent does two things and never a third: it
//! counts the attempt in a [`Tally`] and answers `EPERM`. It never answers
//! "go ahead" (`SECCOMP_USER_NOTIF_FLAG_CONTINUE`), so the listener cannot
//! open the gate, only watch it.
//!
//! Counted, not logged. An agent in a retry loop makes thousands of attempts a
//! second; one line each would be a flood that shows nothing. The tally keeps
//! one entry per family and type, with the count and the first and last time,
//! at most [`MAX_KEYS`] of them plus one overflow entry, and the parent writes
//! the entries that changed every [`FLUSH_EVERY`] and once more when the agent
//! has ended. Each line carries the running total, so a line that arrives
//! late never undoes a newer one: the host keeps the largest count.
//!
//! The listener travels from the child, which installs the filter, to the
//! parent over a `SOCK_SEQPACKET` pair made before the fork (`channel.rs`). Over
//! the same pair the child says when its own probes are over and the agent is
//! next: the refusals the `families` check provokes on
//! purpose are the shim's, not the agent's, and are not counted.
//!
//! What this module cannot see: a `connect(2)` that fails with `ENETUNREACH`
//! is no refused syscall, the filter never hears of it, and the empty network
//! namespace stays a state, not an event (`docs/SECURITY.md`).
//!
//! Report lines, one `write(2)` each, fields without whitespace:
//!
//! ```text
//! REFUSALS on
//! REFUSALS off errno<N>
//! REFUSED socket <family> <type> <family|type|overflow> <count> <first-ms> <last-ms>
//! ```

use std::collections::BTreeMap;
use std::ffi::{c_long, c_ulong};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::report::Report;

/// How many family-and-type pairs the tally keeps apart. Every further pair
/// is counted in one overflow entry: an agent that walks through all 2^32
/// families must not be able to grow the parent's memory with it.
pub const MAX_KEYS: usize = 16;

/// How often the parent writes the entries that changed.
pub const FLUSH_EVERY: Duration = Duration::from_secs(2);

/// Consecutive failed `SECCOMP_IOCTL_NOTIF_RECV` calls after which [`serve`]
/// pauses [`BACKOFF`] before each further try. It never gives up: the
/// listener must outlive every failure (see [`serve`]).
const BACKOFF_AFTER: u32 = 100;

/// The pause between two tries once [`BACKOFF_AFTER`] failures in a row came
/// back.
const BACKOFF: Duration = Duration::from_millis(10);

/// The mask `socket(2)` applies to its type argument, as in the filter.
const TYPE_MASK: u32 = 0xff;

/// Families by number, as `<sys/socket.h>` names them.
const FAMILY_NAMES: &[(u32, &str)] = &[
    (1, "AF_UNIX"),
    (2, "AF_INET"),
    (3, "AF_AX25"),
    (4, "AF_IPX"),
    (5, "AF_APPLETALK"),
    (9, "AF_X25"),
    (10, "AF_INET6"),
    (15, "AF_KEY"),
    (16, "AF_NETLINK"),
    (17, "AF_PACKET"),
    (21, "AF_RDS"),
    (26, "AF_LLC"),
    (27, "AF_IB"),
    (28, "AF_MPLS"),
    (29, "AF_CAN"),
    (30, "AF_TIPC"),
    (31, "AF_BLUETOOTH"),
    (38, "AF_ALG"),
    (40, "AF_VSOCK"),
    (42, "AF_QIPCRTR"),
    (43, "AF_SMC"),
    (44, "AF_XDP"),
    (45, "AF_MCTP"),
];

/// Socket types by number.
const TYPE_NAMES: &[(u32, &str)] = &[
    (1, "SOCK_STREAM"),
    (2, "SOCK_DGRAM"),
    (3, "SOCK_RAW"),
    (4, "SOCK_RDM"),
    (5, "SOCK_SEQPACKET"),
    (6, "SOCK_DCCP"),
    (10, "SOCK_PACKET"),
];

/// The name of a family, or `AF_<n>` for one without a name.
#[must_use]
pub fn family_name(number: u32) -> String {
    FAMILY_NAMES
        .iter()
        .find(|(n, _)| *n == number)
        .map_or_else(|| format!("AF_{number}"), |(_, name)| (*name).to_owned())
}

/// The name of a socket type, or `SOCK_<n>` for one without a name.
#[must_use]
pub fn type_name(number: u32) -> String {
    TYPE_NAMES
        .iter()
        .find(|(n, _)| *n == number)
        .map_or_else(|| format!("SOCK_{number}"), |(_, name)| (*name).to_owned())
}

/// What an entry of the tally is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// One family and one type (after the mask).
    Socket {
        /// arg0, low word.
        family: u32,
        /// arg1 & 0xff.
        sock_type: u32,
    },
    /// Every pair beyond [`MAX_KEYS`].
    Overflow,
}

/// Which half of the gate refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The family is not in `allow_families`.
    Family,
    /// The family is allowed, the type is not in `allow_types`.
    Type,
    /// The overflow entry; its pairs have either reason.
    Overflow,
}

impl Reason {
    const fn name(self) -> &'static str {
        match self {
            Self::Family => "family",
            Self::Type => "type",
            Self::Overflow => "overflow",
        }
    }
}

/// One entry of the tally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Which half of the gate refused.
    pub reason: Reason,
    /// Attempts so far.
    pub count: u64,
    /// The first attempt, milliseconds since the epoch.
    pub first_ms: u64,
    /// The last attempt, milliseconds since the epoch.
    pub last_ms: u64,
}

#[derive(Debug, Default)]
struct State {
    entries: BTreeMap<Key, Entry>,
    changed: bool,
}

/// The refused attempts of one run, by family and type.
#[derive(Debug)]
pub struct Tally {
    /// The families the agent's policy allows; a refusal of one of them was
    /// a refusal of its type.
    allowed_families: Vec<u32>,
    state: Mutex<State>,
}

impl Tally {
    /// An empty tally for a policy that allows `allowed_families`.
    #[must_use]
    pub fn new(allowed_families: Vec<u32>) -> Self {
        Self {
            allowed_families,
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Counts one refused `socket(family, raw_type, ...)` at `now_ms`.
    pub fn record(&self, family: u32, raw_type: u32, now_ms: u64) {
        let sock_type = raw_type & TYPE_MASK;
        let reason = if self.allowed_families.contains(&family) {
            Reason::Type
        } else {
            Reason::Family
        };
        let mut state = self.lock();
        let mut key = Key::Socket { family, sock_type };
        let mut reason = reason;
        if !state.entries.contains_key(&key) && state.entries.len() >= MAX_KEYS {
            key = Key::Overflow;
            reason = Reason::Overflow;
        }
        let entry = state.entries.entry(key).or_insert(Entry {
            reason,
            count: 0,
            first_ms: now_ms,
            last_ms: now_ms,
        });
        entry.count = entry.count.saturating_add(1);
        entry.first_ms = entry.first_ms.min(now_ms);
        entry.last_ms = entry.last_ms.max(now_ms);
        state.changed = true;
    }

    /// Every entry, when something changed since the last call; `None`
    /// otherwise.
    pub fn take_changed(&self) -> Option<Vec<(Key, Entry)>> {
        let mut state = self.lock();
        if !state.changed {
            return None;
        }
        state.changed = false;
        Some(state.entries.iter().map(|(k, e)| (*k, *e)).collect())
    }

    /// Every entry, in key order. Read by the tests; the shim itself only
    /// flushes what changed.
    #[cfg(test)]
    #[must_use]
    pub fn snapshot(&self) -> Vec<(Key, Entry)> {
        self.lock().entries.iter().map(|(k, e)| (*k, *e)).collect()
    }
}

/// The report line of one entry.
#[must_use]
pub fn line(key: Key, entry: Entry) -> String {
    let (family, sock_type) = match key {
        Key::Socket { family, sock_type } => (family_name(family), type_name(sock_type)),
        Key::Overflow => ("*".to_owned(), "*".to_owned()),
    };
    format!(
        "REFUSED socket {family} {sock_type} {} {} {} {}",
        entry.reason.name(),
        entry.count,
        entry.first_ms,
        entry.last_ms
    )
}

/// Writes every entry of `tally` to `report`, if anything changed.
pub fn flush(report: &Report, tally: &Tally) {
    if let Some(entries) = tally.take_changed() {
        for (key, entry) in entries {
            report.line(&line(key, entry));
        }
    }
}

/// [`flush`] every [`FLUSH_EVERY`], for as long as the process lives.
pub fn flush_every(report: &Report, tally: &Tally) -> ! {
    loop {
        std::thread::sleep(FLUSH_EVERY);
        flush(report, tally);
    }
}

/// Now, in milliseconds since the epoch.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Whether this kernel's notification structures fit the ones this binary
/// was built with; the errno otherwise (`EOVERFLOW` for a kernel with larger
/// ones).
///
/// `SECCOMP_IOCTL_NOTIF_RECV` writes the kernel's `struct seccomp_notif`; a
/// larger one than ours would write past the end of our buffer. Allocates
/// nothing, so the child may ask before it picks its gate.
pub fn kernel_fits() -> Result<(), i32> {
    // SAFETY: plain data; all-zero is a valid value.
    let mut sizes: libc::seccomp_notif_sizes = unsafe { zeroed() };
    // SAFETY: SECCOMP_GET_NOTIF_SIZES writes one `seccomp_notif_sizes` to the
    // pointer, which is valid for that.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            c_ulong::from(libc::SECCOMP_GET_NOTIF_SIZES),
            0 as c_ulong,
            &raw mut sizes,
        )
    };
    if rc < 0 {
        return Err(errno());
    }
    if usize::from(sizes.seccomp_notif) > size_of::<libc::seccomp_notif>()
        || usize::from(sizes.seccomp_notif_resp) > size_of::<libc::seccomp_notif_resp>()
    {
        return Err(libc::EOVERFLOW);
    }
    Ok(())
}

/// Parks, counts, answers `EPERM`; never returns.
///
/// `control` is the parent's end of [`crate::channel::channel`]; attempts
/// count from the moment the child's marker (or its end of the pair closing)
/// is visible there. The check happens after the notification is received,
/// and the agent's first call comes after the marker was sent, so no attempt
/// of the agent can be mistaken for one of the probes.
///
/// **The listener stays open until the process ends, whatever `RECV`
/// answers.** A listener that closed would give the agent nothing directly --
/// the kernel then answers parked calls with `ENOSYS` -- but it used to lift
/// the one thing that kept the agent from installing a listener of its own
/// (`has_duplicate_listener`). Both gates refuse that flag outright now
/// (`seccomp.rs`); keeping this one open is the second layer. An agent that
/// makes `RECV` fail on purpose, by parking threads in `socket(2)` and
/// killing them (`ENOENT`), only earns a back-off here, never an exit.
pub fn serve(listener: &OwnedFd, control: &OwnedFd, tally: &Tally) -> ! {
    let mut armed = false;
    let mut errors = 0u32;
    loop {
        // SAFETY: plain data; the kernel wants it zeroed before RECV.
        let mut notif: libc::seccomp_notif = unsafe { zeroed() };
        // SAFETY: RECV writes one `seccomp_notif` (`kernel_fits` checked the
        // size) into `notif`.
        let rc = unsafe {
            libc::ioctl(
                listener.as_raw_fd(),
                libc::SECCOMP_IOCTL_NOTIF_RECV,
                &raw mut notif,
            )
        };
        if rc < 0 {
            if errno() != libc::EINTR {
                // `ENOENT`: the caller vanished between parking and our
                // `RECV`. Anything else: nothing to answer. Either way the
                // listener stays; a run of failures only slows the loop down,
                // so a broken descriptor cannot burn a core.
                errors = errors.saturating_add(1);
                if errors >= BACKOFF_AFTER {
                    std::thread::sleep(BACKOFF);
                }
            }
            continue;
        }
        errors = 0;
        if !armed {
            armed = crate::channel::exec_marker_arrived(control);
        }
        if armed && c_long::from(notif.data.nr) == libc::SYS_socket {
            tally.record(
                low_word(notif.data.args[0]),
                low_word(notif.data.args[1]),
                now_ms(),
            );
        }
        answer_eperm(listener, notif.id);
    }
}

/// `int` arguments live in the low 32 bits of the register.
fn low_word(value: u64) -> u32 {
    u32::try_from(value & 0xffff_ffff).unwrap_or(u32::MAX)
}

fn answer_eperm(listener: &OwnedFd, id: u64) {
    // SAFETY: plain data; all-zero is flags 0 and value 0.
    let mut resp: libc::seccomp_notif_resp = unsafe { zeroed() };
    resp.id = id;
    resp.error = -libc::EPERM;
    // SAFETY: SEND reads one `seccomp_notif_resp` from the pointer. `ENOENT`
    // (the caller is gone) needs no handling: nobody waits for the answer.
    unsafe {
        libc::ioctl(
            listener.as_raw_fd(),
            libc::SECCOMP_IOCTL_NOTIF_SEND,
            &raw mut resp,
        );
    }
}

fn errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]

    use std::ffi::c_int;

    use super::*;
    use crate::channel::{Received, channel, receive, send_exec_marker, send_listener};
    use crate::seccomp::{Gate, Policy, probe_socket};

    const AF_UNIX: u32 = libc::AF_UNIX as u32;
    const AF_INET: u32 = libc::AF_INET as u32;
    const AF_INET6: u32 = libc::AF_INET6 as u32;
    const STREAM: u32 = libc::SOCK_STREAM as u32;
    const DGRAM: u32 = libc::SOCK_DGRAM as u32;

    #[test]
    fn names_follow_the_header_and_numbers_stay_numbers() {
        assert_eq!(family_name(AF_UNIX), "AF_UNIX");
        assert_eq!(family_name(AF_INET6), "AF_INET6");
        assert_eq!(family_name(libc::AF_VSOCK as u32), "AF_VSOCK");
        assert_eq!(family_name(77), "AF_77");
        assert_eq!(type_name(libc::SOCK_SEQPACKET as u32), "SOCK_SEQPACKET");
        assert_eq!(type_name(99), "SOCK_99");
        for (number, name) in FAMILY_NAMES {
            assert!(name.starts_with("AF_") && !name.contains(' '), "{number}");
        }
    }

    #[test]
    fn the_tally_counts_by_pair_and_says_which_half_refused() {
        let tally = Tally::new(vec![AF_INET, AF_INET6]);
        tally.record(AF_UNIX, STREAM, 100);
        tally.record(AF_UNIX, STREAM | libc::SOCK_CLOEXEC as u32, 50);
        tally.record(AF_INET, DGRAM, 300);
        let entries = tally.snapshot();
        assert_eq!(
            entries,
            [
                (
                    Key::Socket {
                        family: AF_UNIX,
                        sock_type: STREAM
                    },
                    Entry {
                        reason: Reason::Family,
                        count: 2,
                        first_ms: 50,
                        last_ms: 100
                    }
                ),
                (
                    Key::Socket {
                        family: AF_INET,
                        sock_type: DGRAM
                    },
                    Entry {
                        reason: Reason::Type,
                        count: 1,
                        first_ms: 300,
                        last_ms: 300
                    }
                ),
            ]
        );
        assert_eq!(
            line(entries[0].0, entries[0].1),
            "REFUSED socket AF_UNIX SOCK_STREAM family 2 50 100"
        );
        assert_eq!(
            line(entries[1].0, entries[1].1),
            "REFUSED socket AF_INET SOCK_DGRAM type 1 300 300"
        );
    }

    #[test]
    fn a_flood_of_pairs_ends_in_one_overflow_entry() {
        let tally = Tally::new(vec![AF_INET]);
        for family in 100..(100 + MAX_KEYS as u32 + 50) {
            tally.record(family, STREAM, 1);
        }
        // A pair the tally already knows keeps its own entry after the cap.
        tally.record(100, STREAM, 2);
        let entries = tally.snapshot();
        assert_eq!(entries.len(), MAX_KEYS + 1);
        let (key, overflow) = entries.last().unwrap();
        assert_eq!(*key, Key::Overflow);
        assert_eq!(overflow.count, 50);
        assert_eq!(overflow.reason, Reason::Overflow);
        assert_eq!(entries[0].1.count, 2);
        assert_eq!(line(*key, *overflow), "REFUSED socket * * overflow 50 1 1");
    }

    #[test]
    fn only_a_change_is_flushed() {
        let tally = Tally::new(vec![AF_INET]);
        assert!(tally.take_changed().is_none());
        tally.record(AF_UNIX, STREAM, 1);
        assert_eq!(tally.take_changed().unwrap().len(), 1);
        assert!(tally.take_changed().is_none());
        tally.record(AF_UNIX, STREAM, 2);
        assert_eq!(tally.take_changed().unwrap()[0].1.count, 2);
    }

    #[test]
    fn this_kernel_fits_the_structures() {
        assert_eq!(kernel_fits(), Ok(()));
    }

    /// How many refused calls the forked child makes after its marker.
    const AFTER_UNIX: usize = 1000;
    const AFTER_DGRAM: usize = 500;
    /// Refused calls before the marker: the shim's own probes.
    const BEFORE: usize = 2;

    fn clock_ns() -> u64 {
        // SAFETY: plain data, written by clock_gettime.
        let mut ts: libc::timespec = unsafe { zeroed() };
        // SAFETY: CLOCK_MONOTONIC with a valid pointer.
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut ts) };
        (ts.tv_sec as u64) * 1_000_000_000 + ts.tv_nsec as u64
    }

    /// The whole path in one process tree: the child installs the reported
    /// gate and hands over the listener, the parent serves it on a thread,
    /// and the child measures what a refused call costs.
    ///
    /// The child exits 0 when every refused call came back `EPERM` and every
    /// allowed one succeeded; it writes the nanoseconds of its timed calls to
    /// a pipe.
    #[test]
    fn the_listener_counts_the_agent_and_answers_eperm_quickly() {
        let program = Policy::from_env(None, None, None)
            .unwrap()
            .program(Gate::Reported)
            .unwrap();
        let (parent_end, child_end) = channel().unwrap();
        let mut pipe = [0 as c_int; 2];
        // SAFETY: valid two-element array.
        assert_eq!(
            unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );

        // SAFETY: the child only calls prctl, seccomp, sendmsg, socket,
        // clock_gettime, write and _exit, none of which allocates.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            let control = child_end.as_raw_fd();
            let Ok(listener) = crate::seccomp::apply_reporting(&program) else {
                unsafe { libc::_exit(10) }
            };
            if !send_listener(control, listener.as_raw_fd()) {
                unsafe { libc::_exit(11) }
            }
            drop(listener);
            let mut bad = 0;
            for _ in 0..BEFORE {
                bad += i32::from(probe_socket(AF_UNIX.into(), STREAM.into()) != Err(libc::EPERM));
            }
            send_exec_marker(control);
            let start = clock_ns();
            for _ in 0..AFTER_UNIX {
                bad += i32::from(probe_socket(AF_UNIX.into(), STREAM.into()) != Err(libc::EPERM));
            }
            for _ in 0..AFTER_DGRAM {
                bad += i32::from(probe_socket(AF_INET.into(), DGRAM.into()) != Err(libc::EPERM));
            }
            let elapsed = clock_ns() - start;
            bad += i32::from(probe_socket(AF_INET.into(), STREAM.into()).is_err());
            let bytes = elapsed.to_le_bytes();
            // SAFETY: the pointer and length describe `bytes`.
            unsafe { libc::write(pipe[1], bytes.as_ptr().cast(), bytes.len()) };
            unsafe { libc::_exit(if bad == 0 { 0 } else { 12 }) }
        }
        drop(child_end);
        // SAFETY: the write end belongs to the child now.
        unsafe { libc::close(pipe[1]) };

        let Received::Listener(listener) = receive(&parent_end) else {
            panic!("the child sent no listener");
        };
        let tally = std::sync::Arc::new(Tally::new(vec![AF_INET, AF_INET6]));
        let served = std::sync::Arc::clone(&tally);
        std::thread::spawn(move || serve(&listener, &parent_end, &served));

        let mut status = 0;
        // SAFETY: our own child, valid pointer.
        assert_eq!(unsafe { libc::waitpid(pid, &raw mut status, 0) }, pid);
        assert!(libc::WIFEXITED(status));
        assert_eq!(libc::WEXITSTATUS(status), 0, "child status");
        let mut bytes = [0u8; 8];
        // SAFETY: read end of our pipe, valid buffer.
        let n = unsafe { libc::read(pipe[0], bytes.as_mut_ptr().cast(), 8) };
        unsafe { libc::close(pipe[0]) };
        assert_eq!(n, 8);
        let per_call_ns = u64::from_le_bytes(bytes) / (AFTER_UNIX + AFTER_DGRAM) as u64;
        println!("refused socket(2) through the listener: {per_call_ns} ns per call");
        // A refusal parked and answered costs two context switches, not a
        // wait: measured about 10 to 40 us on the development machine. A
        // millisecond would already be a brake on an agent in a loop.
        assert!(per_call_ns < 1_000_000, "{per_call_ns} ns per refused call");

        let counts: Vec<(Key, u64)> = tally
            .snapshot()
            .into_iter()
            .map(|(key, entry)| (key, entry.count))
            .collect();
        assert_eq!(
            counts,
            [
                (
                    Key::Socket {
                        family: AF_UNIX,
                        sock_type: STREAM
                    },
                    AFTER_UNIX as u64
                ),
                (
                    Key::Socket {
                        family: AF_INET,
                        sock_type: DGRAM
                    },
                    AFTER_DGRAM as u64
                ),
            ],
            "the probes before the marker are not counted, the allowed call is not either"
        );
    }
}
