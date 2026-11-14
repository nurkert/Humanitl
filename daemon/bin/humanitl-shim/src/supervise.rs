//! PID 1's one loop while the agent runs: children, the lifeline, the door
//! for further shells (HUM-430).
//!
//! The parent waits on three things at once, in one `poll(2)` and no second
//! loop, because the shim has no runtime and wants none:
//!
//! - **children.** A `SIGCHLD` handler writes one byte into a pipe whose read
//!   end is in the loop (the self-pipe trick); every wake-up reaps with
//!   `waitpid(-1, WNOHANG)` until nothing is left. As PID 1 the shim reaps
//!   every orphan of the sandbox (HUM-203); only its own child decides the
//!   exit status, a further shell's end goes to [`Shells::reaped`].
//! - **the lifeline** (`--lifeline-fd`). The read end of a pipe whose only
//!   write end the daemon holds, with `CLOEXEC`. When the daemon ends, also
//!   through `SIGKILL`, the kernel closes that end, and the read end reports
//!   `POLLHUP`. The shim then ends the sandbox the way it ends on `SIGTERM`:
//!   `SIGTERM` to the agent, after [`KILL_GRACE`] `SIGKILL` to the agent, to
//!   every further shell and, as PID 1, to every process of the namespace.
//!   This replaces bwrap's `--die-with-parent` for a backend that starts the
//!   sandbox detached from the daemon (a container engine's monitor, HUM-292 P7). The
//!   read end may be open in other processes too (the engine's client
//!   inherits it); what counts is only that the write end lies in the daemon
//!   alone.
//! - **the door for further shells** (`--control-fd`, HUM-299): a request
//!   is served when it arrives; when the daemon closes its end, every shell
//!   still running is killed, because nobody is left to show it.
//! - **the stream** (`--control-stream-fd` or the guest's connection,
//!   HUM-443): the socket, every pseudo-terminal and the heartbeat
//!   (`stream.rs`). Its end is a lifeline like the one above: the agent gets
//!   `SIGTERM`, every further shell `SIGKILL`, and after [`KILL_GRACE`]
//!   everything else.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use crate::linux::exec::Shells;
use crate::linux::stream::Session;

/// How long the agent has after the lifeline ends before everything is
/// killed ([`super::contract::KILL_GRACE`]).
pub use super::contract::KILL_GRACE;

/// Exit status when the wait itself failed (`ECHILD` without the child).
const EXIT_SETUP: i32 = 126;

/// The write end of the wake-up pipe, for the `SIGCHLD` handler.
static WAKE: AtomicI32 = AtomicI32::new(-1);

/// Takes the descriptor of `--lifeline-fd` when it is an open pipe or socket;
/// gives it `CLOEXEC` and `O_NONBLOCK`.
///
/// # Errors
///
/// A short reason when the number names no pipe or socket. The caller refuses
/// the start: a lifeline that cannot be watched would let the sandbox outlive
/// the daemon without anyone noticing.
pub fn take_lifeline(fd: RawFd) -> Result<OwnedFd, String> {
    // SAFETY: plain data; fstat fills it.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat on a descriptor number with a valid pointer.
    if fd < 3 || unsafe { libc::fstat(fd, &raw mut stat) } != 0 {
        return Err(format!("not-open:{fd}"));
    }
    let kind = stat.st_mode & libc::S_IFMT;
    if kind != libc::S_IFIFO && kind != libc::S_IFSOCK {
        return Err(format!("not-a-pipe:{fd}"));
    }
    // SAFETY: F_SETFD and F_SETFL change flags of one descriptor.
    unsafe {
        libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags >= 0 {
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    // SAFETY: the launcher handed this descriptor to the shim alone, and
    // nothing else in this process owns it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

extern "C" fn on_sigchld(_: libc::c_int) {
    let fd = WAKE.load(Ordering::SeqCst);
    if fd < 0 {
        return;
    }
    // SAFETY: errno is thread-local; saving and restoring it keeps the
    // interrupted code's errno intact. write(2) is async-signal-safe, and a
    // full pipe (O_NONBLOCK) only means a wake-up is already pending.
    unsafe {
        let errno = *libc::__errno_location();
        let byte = 0u8;
        libc::write(fd, (&raw const byte).cast(), 1);
        *libc::__errno_location() = errno;
    }
}

/// The wake-up pipe as `(read, write)`, both non-blocking with `CLOEXEC`,
/// and the `SIGCHLD` handler that writes into it.
fn install_wake() -> io::Result<OwnedFd> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid two-element array for pipe2.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe2 just created both descriptors. The write end is kept for
    // the life of the process in `WAKE`, on purpose: the handler may run at
    // any time until exit.
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    WAKE.store(fds[1], Ordering::SeqCst);
    // SAFETY: sigaction is plain data; the handler calls only write(2).
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_sigchld as extern "C" fn(libc::c_int) as usize;
        action.sa_flags = libc::SA_RESTART | libc::SA_NOCLDSTOP;
        libc::sigemptyset(&raw mut action.sa_mask);
        if libc::sigaction(libc::SIGCHLD, &raw const action, ptr::null_mut()) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(read)
}

/// Reads whatever is in a non-blocking descriptor; `true` when it is at end
/// of file.
fn drain(fd: RawFd) -> bool {
    let mut buf = [0u8; 64];
    loop {
        // SAFETY: the buffer outlives the call and its length is passed.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n == 0 {
            return true;
        }
        if n < 0 {
            return false;
        }
    }
}

/// The status of `status` as an exit code: the code, or 128 + signal.
fn exit_code(status: libc::c_int) -> Option<i32> {
    if libc::WIFEXITED(status) {
        Some(libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        Some(128 + libc::WTERMSIG(status))
    } else {
        None
    }
}

/// What one sweep of `waitpid` found.
enum Swept {
    /// The agent ended with this code.
    Child(i32),
    /// Nothing left to reap right now.
    Pending,
    /// No child at all, and the agent was never reaped.
    Lost,
}

/// Reaps every child that has ended, without waiting.
fn sweep(child: libc::pid_t, shells: Option<&Shells>, mut stream: Option<&mut Session>) -> Swept {
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: waitpid on any child of this process with a valid status
        // pointer.
        let waited = unsafe { libc::waitpid(-1, &raw mut status, libc::WNOHANG) };
        if waited == child {
            if let Some(code) = exit_code(status) {
                return Swept::Child(code);
            }
            continue;
        }
        if waited > 0 {
            // A further shell reports its end; an orphan of the agent is
            // reaped and forgotten.
            if let Some(shells) = shells {
                shells.reaped(waited, status);
            }
            if let Some(stream) = stream.as_deref_mut() {
                stream.reaped(waited, status);
            }
            continue;
        }
        if waited == 0 {
            return Swept::Pending;
        }
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Swept::Lost;
        }
    }
}

/// `SIGKILL` to the agent, to every further shell, and as PID 1 to every
/// process of the namespace. Outside a PID namespace (the tests on the host)
/// `kill(-1)` would reach the user's own processes, so it is only sent when
/// this process is PID 1.
fn kill_everything(child: libc::pid_t, shells: Option<&Shells>) {
    // SAFETY: kill takes two integers.
    unsafe {
        libc::kill(child, libc::SIGKILL);
    }
    if let Some(shells) = shells {
        shells.kill_all();
    }
    // SAFETY: getpid has no preconditions; kill(-1) from PID 1 of a PID
    // namespace reaches the processes of that namespace and never this one.
    unsafe {
        if libc::getpid() == 1 {
            libc::kill(-1, libc::SIGKILL);
        }
    }
}

/// The daemon is gone (`what` says how it showed): `SIGTERM` to the agent,
/// and the moment at which everything left gets `SIGKILL`.
fn end_sandbox(child: libc::pid_t, what: &str) -> Instant {
    // The signal first, the message after: should the message ever wait,
    // the agent has its `SIGTERM` already (HUM-443).
    // SAFETY: kill takes two integers.
    unsafe {
        libc::kill(child, libc::SIGTERM);
    }
    let deadline = Instant::now() + KILL_GRACE;
    crate::linux::say(&format!(
        "humanitl-shim: {what}; the daemon is gone, ending the sandbox\n"
    ));
    deadline
}

/// Waits for the agent while serving the lifeline and the door; returns the
/// agent's exit code, or 128 + signal.
///
/// Without a working wake-up pipe the loop still ends: it then polls with a
/// short timeout instead of waiting for the byte.
pub fn wait(
    child: libc::pid_t,
    shells: Option<&Shells>,
    lifeline: Option<OwnedFd>,
    mut stream: Option<&mut Session>,
) -> i32 {
    let wake = match install_wake() {
        Ok(wake) => Some(wake),
        Err(err) => {
            crate::linux::say(&format!(
                "humanitl-shim: warning: no SIGCHLD wake-up, polling instead: {err}\n"
            ));
            None
        }
    };
    let mut lifeline = lifeline;
    let mut door_open = shells.is_some();
    let mut deadline: Option<Instant> = None;
    let mut stream_ended = false;
    loop {
        match sweep(child, shells, stream.as_deref_mut()) {
            Swept::Child(code) => {
                // The agent's last bytes and its exit reach the daemon
                // before this process ends.
                if let Some(stream) = stream.as_deref_mut() {
                    stream.finish(code);
                }
                return code;
            }
            Swept::Lost => return EXIT_SETUP,
            Swept::Pending => {}
        }
        let mut fds: Vec<libc::pollfd> = Vec::with_capacity(3);
        let mut slot = |fd: RawFd, events: libc::c_short| {
            fds.push(libc::pollfd {
                fd,
                events,
                revents: 0,
            });
            fds.len() - 1
        };
        let wake_slot = wake.as_ref().map(|fd| slot(fd.as_raw_fd(), libc::POLLIN));
        let lifeline_slot = lifeline
            .as_ref()
            .map(|fd| slot(fd.as_raw_fd(), libc::POLLIN));
        // Replies that wait ask for `POLLOUT`; the other slots never wait on
        // them, so the lifeline is heard whatever the daemon reads.
        let door_slot = shells.filter(|_| door_open).map(|shells| {
            let events = if shells.pending() {
                libc::POLLIN | libc::POLLOUT
            } else {
                libc::POLLIN
            };
            slot(shells.control_fd(), events)
        });
        let stream_slots = stream.as_deref().map(|stream| stream.register(&mut fds));
        let timeout = match (deadline, &wake) {
            (Some(at), _) => {
                let left = at.saturating_duration_since(Instant::now());
                i32::try_from(left.as_millis().saturating_add(1)).unwrap_or(i32::MAX)
            }
            (None, Some(_)) => -1,
            (None, None) => 100,
        };
        // The heartbeat is due at the latest then.
        let timeout = match stream.as_deref().map(Session::timeout_ms) {
            Some(beat) if beat >= 0 && (timeout < 0 || beat < timeout) => beat,
            _ => timeout,
        };
        let count = libc::nfds_t::try_from(fds.len()).unwrap_or(0);
        // SAFETY: `fds` holds `count` initialised entries and outlives the
        // call.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), count, timeout) };
        if ready < 0 {
            // EINTR from a signal (SIGCHLD, a relayed SIGTERM): sweep again.
            continue;
        }
        if let Some(at) = deadline
            && Instant::now() >= at
        {
            deadline = None;
            kill_everything(child, shells);
        }
        let revents = |slot: Option<usize>| slot.map_or(0, |index| fds[index].revents);
        if let (Some(fd), true) = (&wake, revents(wake_slot) != 0) {
            drain(fd.as_raw_fd());
        }
        let line = revents(lifeline_slot);
        if line != 0 {
            // A pipe whose last writer closed reports `POLLHUP` alone (with
            // nothing buffered); a socket reports `POLLIN` and reads end of
            // file. Bytes the daemon never sends are read and dropped.
            let ended = line & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
                || (line & libc::POLLIN != 0
                    && lifeline.as_ref().is_some_and(|fd| drain(fd.as_raw_fd())));
            if ended {
                lifeline = None;
                deadline = Some(end_sandbox(child, "the lifeline ended"));
            }
        }
        if let (Some(stream), Some(slots)) = (stream.as_deref_mut(), stream_slots.as_ref()) {
            stream.serve(&fds, slots);
            if stream.closed() && !stream_ended {
                // The daemon's end of the stream closed: the lifeline of the
                // stream mode (HUM-443). Every further shell goes at once,
                // the agent as on the end of `--lifeline-fd`.
                stream_ended = true;
                stream.kill_shells();
                deadline = Some(end_sandbox(child, "the control stream ended"));
            }
        }
        if let Some(shells) = shells.filter(|_| door_open) {
            door_open = serve_door(shells, revents(door_slot));
        }
    }
}

/// Serves the door for further shells after `poll` said `door`; `false`
/// once it is shut.
fn serve_door(shells: &Shells, door: libc::c_short) -> bool {
    if door & libc::POLLOUT != 0 {
        shells.flush();
    }
    let readable = door & (libc::POLLIN | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL);
    let closed = readable != 0 && !shells.serve_one();
    // A reply that cannot arrive leaves the daemon waiting for a shell it
    // will never hear of again: end them all, door shut.
    if closed || shells.broken() {
        shells.kill_all();
        return false;
    }
    true
}

/// Waits for `child` alone, blocking; for the paths that killed it before
/// the agent ever ran. Orphans are reaped on the way.
pub fn reap_blocking(child: libc::pid_t) -> i32 {
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: waitpid on any child of this process with a valid status
        // pointer.
        let waited = unsafe { libc::waitpid(-1, &raw mut status, 0) };
        if waited == child {
            if let Some(code) = exit_code(status) {
                return code;
            }
            continue;
        }
        if waited > 0 {
            continue;
        }
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return EXIT_SETUP;
        }
    }
}
