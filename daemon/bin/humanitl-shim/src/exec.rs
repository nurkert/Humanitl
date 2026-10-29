//! Further shells in the running sandbox (HUM-299).
//!
//! The daemon may ask PID 1 of the sandbox, this shim's parent, for another
//! process next to the agent: a shell with a pseudo-terminal of its own, like
//! `docker exec`. The shim forks it from inside, so it inherits what every
//! process here carries: `PR_SET_NO_NEW_PRIVS`, the bridge filter of the
//! parent, the same mounts, namespaces, environment and working directory.
//! Before `exec` the child installs the agent's own filter on top, the silent
//! gate ([`crate::seccomp::Gate::Silent`]), and proves it: `socket(AF_UNIX)`,
//! which the parent's filter still allows, must answer `EPERM`, or the shell
//! never starts. Nobody enters the sandbox from outside; there is no `setns`
//! and no `nsenter`.
//!
//! # The door, and who holds it
//!
//! The request arrives on a `SOCK_SEQPACKET` socket whose other end only the
//! daemon holds (`--control-fd`, see `main.rs`). It is an unnamed pair: there
//! is no path in the sandbox to connect to, and the `single_socket` walk
//! still finds the bridge's socket alone. The shim's end lives in PID 1 only:
//!
//! - the agent's child closes it before its filter and its `exec`
//!   (`close_inherited` without it in the keep list, and `CLOEXEC`);
//! - every shell started here closes every descriptor above 2 before `exec`;
//! - PID 1 is not dumpable, so neither `/proc/1/fd` nor `pidfd_getfd(2)`
//!   (which the floor refuses anyway) reach it.
//!
//! Nothing inside the sandbox can therefore send a request. Replies go the
//! other way and are data for the daemon, never instructions.
//!
//! # Wire format
//!
//! Daemon to shim, one message per request:
//!
//! | bytes | meaning |
//! |---|---|
//! | `S` | spawn |
//! | 4, little endian | the shell's id, chosen by the daemon, never 0 |
//! | rest | the command, each argument followed by a NUL byte |
//!
//! with exactly one descriptor as `SCM_RIGHTS`: the follower side of a
//! pseudo-terminal the daemon opened for this shell.
//!
//! Shim to daemon, five bytes of id and a four-byte value after the mark:
//!
//! - `R` id pid: started, `pid` as this namespace counts;
//! - `F` id code: refused; `code` is a positive errno of `fork(2)`,
//!   [`REFUSED_LIMIT`] or [`REFUSED_MALFORMED`];
//! - `E` id status: ended, the exit code or 128 + signal.
//!
//! At most [`MAX_SHELLS`] shells live at a time; the daemon counts as well.

use std::ffi::{CString, c_char, c_int, c_uint};
use std::io;
use std::mem::zeroed;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use seccompiler::BpfProgram;

use crate::seccomp;

/// How many further shells may live at the same time.
pub const MAX_SHELLS: usize = 4;

/// The longest request: mark, id and the command with its NUL bytes.
pub const MAX_REQUEST: usize = 16 * 1024;

/// The most arguments a command may have.
const MAX_ARGS: usize = 256;

/// Room in the control buffer, in bytes: enough for several descriptors, so
/// that a request carrying more than one arrives whole and every extra one is
/// closed. What does not fit sets `MSG_CTRUNC`, and the kernel closes it.
const CMSG_ROOM: usize = 128;

const MARK_SPAWN: u8 = b'S';
const MARK_STARTED: u8 = b'R';
const MARK_REFUSED: u8 = b'F';
const MARK_EXITED: u8 = b'E';

/// `F` code: [`MAX_SHELLS`] shells are already running.
pub const REFUSED_LIMIT: i32 = -1;
/// `F` code: the request was not one this module understands.
pub const REFUSED_MALFORMED: i32 = -2;

/// Exit status of a shell whose setup failed before `exec`.
const EXIT_SETUP: c_int = 126;
/// Exit status of a shell whose `exec` failed.
const EXIT_EXEC: c_int = 127;

/// One request, parsed and checked.
#[derive(Debug)]
pub struct Request {
    /// The daemon's id for this shell.
    pub id: u32,
    /// The command, never empty.
    pub argv: Vec<CString>,
    /// The follower side of the shell's pseudo-terminal.
    pub follower: OwnedFd,
}

/// Why a message is not a request.
#[derive(Debug, PartialEq, Eq)]
pub struct Malformed {
    /// The id, when the message got as far as naming one; otherwise 0.
    pub id: u32,
    /// What was wrong, for the shim's stderr.
    pub why: &'static str,
}

/// Parses one message. The descriptors are consumed: on success the one
/// follower is kept, on failure every one of them is closed.
///
/// # Errors
///
/// [`Malformed`] for a wrong mark, a short message, an empty command, too
/// many arguments, an argument that is not terminated, or a descriptor count
/// other than one.
pub fn parse(payload: &[u8], mut fds: Vec<OwnedFd>) -> Result<Request, Malformed> {
    if payload.len() < 5 || payload[0] != MARK_SPAWN {
        return Err(Malformed {
            id: 0,
            why: "not a spawn request",
        });
    }
    let id = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
    let bad = |why| Err(Malformed { id, why });
    if id == 0 {
        return bad("shell id 0");
    }
    if fds.len() != 1 {
        return bad("a spawn request carries exactly one descriptor");
    }
    let body = &payload[5..];
    if body.is_empty() {
        return bad("empty command");
    }
    if body.last() != Some(&0) {
        return bad("the last argument is not terminated");
    }
    let parts: Vec<&[u8]> = body[..body.len() - 1].split(|byte| *byte == 0).collect();
    if parts.len() > MAX_ARGS {
        return bad("too many arguments");
    }
    if parts.first().is_none_or(|first| first.is_empty()) {
        return bad("empty program name");
    }
    let argv: Vec<CString> = parts
        .into_iter()
        .filter_map(|part| CString::new(part).ok())
        .collect();
    let Some(follower) = fds.pop() else {
        return bad("a spawn request carries exactly one descriptor");
    };
    Ok(Request { id, argv, follower })
}

/// What one receive brought.
enum Incoming {
    Spawn(Request),
    Malformed(Malformed),
    Closed,
}

/// The further shells of this sandbox, served from a thread of PID 1.
pub struct Shells {
    control: OwnedFd,
    /// The agent's filter, answering `EPERM` itself.
    program: BpfProgram,
    /// The environment of the shells: the shim's own without its variables.
    env: Vec<CString>,
    /// This process, for the child's check against `PR_SET_PDEATHSIG`.
    parent: libc::pid_t,
    /// Live shells as (id, pid). Held across `fork` and the `R` reply, so
    /// that the reaper finds a shell that ended at once, and so that its `E`
    /// never overtakes the `R`.
    live: Mutex<Vec<(u32, libc::pid_t)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shells {
    /// The shells of this process, over the daemon's socket.
    ///
    /// `hidden` are the variables the agent never sees (`main.rs`,
    /// `SHIM_VARS`); the shells do not see them either.
    pub fn new(control: OwnedFd, program: BpfProgram, hidden: &[&str]) -> Self {
        let env = std::env::vars_os()
            .filter(|(key, _)| !hidden.iter().any(|hide| key.as_bytes() == hide.as_bytes()))
            .filter_map(|(key, value)| {
                let mut pair = key.as_bytes().to_vec();
                pair.push(b'=');
                pair.extend_from_slice(value.as_bytes());
                CString::new(pair).ok()
            })
            .collect();
        Self {
            control,
            program,
            env,
            // SAFETY: getpid has no preconditions.
            parent: unsafe { libc::getpid() },
            live: Mutex::new(Vec::new()),
        }
    }

    /// Serves requests until the daemon closes its end.
    ///
    /// Every shell started here dies with this thread (`PR_SET_PDEATHSIG`
    /// belongs to the thread that forked): when the daemon lets go of the
    /// socket, nobody is left to show the shell, and it ends.
    pub fn serve(&self) {
        loop {
            match self.receive() {
                Incoming::Closed => return,
                Incoming::Malformed(malformed) => {
                    write_raw(2, b"humanitl-shim: warning: shell request refused: ");
                    write_raw(2, malformed.why.as_bytes());
                    write_raw(2, b"\n");
                    self.reply(MARK_REFUSED, malformed.id, REFUSED_MALFORMED);
                }
                Incoming::Spawn(request) => self.spawn(request),
            }
        }
    }

    /// The reaper's hook: `pid` ended with `status`. Sends `E` when it was
    /// one of the shells.
    pub fn reaped(&self, pid: libc::pid_t, status: c_int) {
        let mut live = lock(&self.live);
        let Some(index) = live.iter().position(|(_, shell)| *shell == pid) else {
            return;
        };
        let (id, _) = live.remove(index);
        let code = if libc::WIFSIGNALED(status) {
            128 + libc::WTERMSIG(status)
        } else {
            libc::WEXITSTATUS(status)
        };
        self.reply(MARK_EXITED, id, code);
    }

    /// Kills every shell that still lives; for the end of the sandbox.
    pub fn kill_all(&self) {
        for (_, pid) in lock(&self.live).iter() {
            // SAFETY: kill takes two integers.
            unsafe {
                libc::kill(*pid, libc::SIGKILL);
            }
        }
    }

    fn spawn(&self, request: Request) {
        let mut live = lock(&self.live);
        if live.len() >= MAX_SHELLS {
            self.reply(MARK_REFUSED, request.id, REFUSED_LIMIT);
            return;
        }
        if live.iter().any(|(id, _)| *id == request.id) {
            self.reply(MARK_REFUSED, request.id, REFUSED_MALFORMED);
            return;
        }
        // Everything the child needs exists before the fork: this process
        // has other threads, and the child may only make calls that are
        // async-signal-safe.
        let argv = pointers(&request.argv);
        let envp = pointers(&self.env);
        // SAFETY: the child runs only `child`, which allocates nothing and
        // calls only async-signal-safe functions until `execvpe`.
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            let errno = io::Error::last_os_error().raw_os_error().unwrap_or(0);
            self.reply(MARK_REFUSED, request.id, errno);
            return;
        }
        if pid == 0 {
            child(
                self.parent,
                request.follower.as_raw_fd(),
                &argv,
                &envp,
                &self.program,
            );
        }
        live.push((request.id, pid));
        drop(request.follower);
        self.reply(MARK_STARTED, request.id, pid);
        drop(live);
    }

    fn receive(&self) -> Incoming {
        let mut payload = vec![0u8; MAX_REQUEST + 1];
        let mut iov = libc::iovec {
            iov_base: payload.as_mut_ptr().cast(),
            iov_len: payload.len(),
        };
        let mut buf = CmsgBuf([0; CMSG_ROOM]);
        loop {
            // SAFETY: plain data.
            let mut msg: libc::msghdr = unsafe { zeroed() };
            msg.msg_iov = &raw mut iov;
            msg.msg_iovlen = 1;
            msg.msg_control = buf.0.as_mut_ptr().cast();
            #[allow(clippy::useless_conversion)]
            {
                msg.msg_controllen = buf.0.len().try_into().unwrap_or_default();
            }
            // SAFETY: `msg` describes `payload` and `buf`, both valid for
            // writes and alive for the call.
            let n = unsafe {
                libc::recvmsg(
                    self.control.as_raw_fd(),
                    &raw mut msg,
                    libc::MSG_CMSG_CLOEXEC,
                )
            };
            if n < 0 {
                if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Incoming::Closed;
            }
            if n == 0 {
                return Incoming::Closed;
            }
            let fds = received_fds(&msg);
            let received = usize::try_from(n).unwrap_or(usize::MAX);
            // Longer than a request may be is malformed, even when it fit the
            // buffer's one spare byte.
            let truncated =
                msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 || received > MAX_REQUEST;
            let len = received.min(MAX_REQUEST);
            if truncated {
                drop(fds);
                let id = if len >= 5 {
                    u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]])
                } else {
                    0
                };
                return Incoming::Malformed(Malformed {
                    id,
                    why: "the request does not fit",
                });
            }
            return match parse(&payload[..len], fds) {
                Ok(request) => Incoming::Spawn(request),
                Err(malformed) => Incoming::Malformed(malformed),
            };
        }
    }

    fn reply(&self, mark: u8, id: u32, value: i32) {
        let id = id.to_le_bytes();
        let value = value.to_le_bytes();
        let message = [
            mark, id[0], id[1], id[2], id[3], value[0], value[1], value[2], value[3],
        ];
        // SAFETY: the pointer and length describe `message`.
        unsafe {
            libc::send(
                self.control.as_raw_fd(),
                message.as_ptr().cast(),
                message.len(),
                libc::MSG_NOSIGNAL,
            );
        }
    }
}

/// Control buffer of [`CMSG_ROOM`] bytes, aligned for `cmsghdr`.
#[repr(C, align(8))]
struct CmsgBuf([u8; CMSG_ROOM]);

/// Every descriptor in `msg`'s control buffer, owned so that the unused ones
/// close.
fn received_fds(msg: &libc::msghdr) -> Vec<OwnedFd> {
    let mut fds = Vec::new();
    // SAFETY: `msg` was filled by recvmsg; the macros walk its control buffer
    // within `msg_controllen`, and every descriptor in an `SCM_RIGHTS` header
    // was installed for this process by the kernel.
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
                let header = libc::CMSG_LEN(0) as usize;
                let len = ((*cmsg).cmsg_len as usize).saturating_sub(header);
                // Every descriptor of the header, to its end: each one is
                // owned below and closed unless it is the one expected.
                let count = len / size_of::<c_int>();
                // Read with `read_unaligned` below, so the alignment of the
                // cast pointer never matters.
                #[allow(clippy::cast_ptr_alignment)]
                let data = libc::CMSG_DATA(cmsg).cast::<c_int>();
                for index in 0..count {
                    let fd = ptr::read_unaligned(data.add(index));
                    if fd >= 0 {
                        fds.push(OwnedFd::from_raw_fd(fd));
                    }
                }
            }
            cmsg = libc::CMSG_NXTHDR(msg, cmsg);
        }
    }
    fds
}

/// A NUL-terminated array of pointers into `strings`.
fn pointers(strings: &[CString]) -> Vec<*const c_char> {
    strings
        .iter()
        .map(|string| string.as_ptr())
        .chain(std::iter::once(ptr::null()))
        .collect()
}

/// `write(2)` without a lock: the child of a threaded process must not take
/// the lock of `std::io::stderr`, which another thread may have held at the
/// fork.
fn write_raw(fd: RawFd, bytes: &[u8]) {
    // SAFETY: the pointer and length describe `bytes`.
    unsafe {
        libc::write(fd, bytes.as_ptr().cast(), bytes.len());
    }
}

fn fail(why: &[u8], code: c_int) -> ! {
    write_raw(2, b"humanitl-shim: shell setup failed: ");
    write_raw(2, why);
    write_raw(2, b"\n");
    // SAFETY: _exit ends the child without running the parent's destructors.
    unsafe { libc::_exit(code) }
}

/// The child: a session of its own on the follower, nothing inherited above
/// descriptor 2, the agent's filter proven, then the command. Never returns.
///
/// Allocates nothing: the parent has other threads, and a lock one of them
/// held at the fork stays held here for ever.
fn child(
    parent: libc::pid_t,
    follower: RawFd,
    argv: &[*const c_char],
    envp: &[*const c_char],
    program: &BpfProgram,
) -> ! {
    // SAFETY: prctl with PR_SET_PDEATHSIG only records a signal number.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
    }
    // SAFETY: getppid has no preconditions.
    if unsafe { libc::getppid() } != parent {
        fail(b"the shim ended before the shell started", EXIT_SETUP);
    }
    // SAFETY: setsid has no preconditions; a forked child is never a group
    // leader.
    if unsafe { libc::setsid() } < 0 {
        fail(b"setsid", EXIT_SETUP);
    }
    // The follower becomes the controlling terminal of the new session, as
    // for the agent with `--controlling-terminal` (HUM-280). Argument 0: it
    // never steals a terminal that belongs to another session, and this one
    // is the daemon's, opened with `O_NOCTTY`, never the user's.
    // SAFETY: TIOCSCTTY takes an int.
    if unsafe { libc::ioctl(follower, libc::TIOCSCTTY, 0 as c_int) } != 0 {
        write_raw(
            2,
            b"humanitl-shim: warning: the shell has no controlling terminal\n",
        );
    }
    for target in 0..=2 {
        // SAFETY: dup2 takes two integers.
        if unsafe { libc::dup2(follower, target) } < 0 {
            fail(b"dup2", EXIT_SETUP);
        }
    }
    close_above_stderr();
    // SAFETY: sigset_t is plain data; an all-zero value is what sigemptyset
    // produces, and sigprocmask reads it.
    unsafe {
        let mut set: libc::sigset_t = zeroed();
        libc::sigemptyset(&raw mut set);
        libc::sigprocmask(libc::SIG_SETMASK, &raw const set, ptr::null_mut());
    }
    if seccomp::apply(program).is_err() {
        fail(b"the agent's filter was refused", EXIT_SETUP);
    }
    // The proof: the parent's filter allows `AF_UNIX` for the bridge, the
    // agent's does not. A shell that could open one would have more than the
    // agent, so it never starts.
    let unix = libc::AF_UNIX.unsigned_abs();
    let stream = (libc::SOCK_STREAM | libc::SOCK_CLOEXEC).unsigned_abs();
    if seccomp::probe_socket(u64::from(unix), u64::from(stream)) != Err(libc::EPERM) {
        fail(b"the agent's filter does not refuse AF_UNIX", EXIT_SETUP);
    }
    crate::reset_signal_dispositions();
    // SAFETY: `argv` and `envp` are NUL-terminated arrays of pointers to
    // NUL-terminated strings that the parent keeps alive; this copy of them
    // lives until the exec.
    unsafe {
        libc::execvpe(argv[0], argv.as_ptr(), envp.as_ptr());
    }
    fail(b"exec failed", EXIT_EXEC)
}

/// Closes every descriptor above 2: `close_range(2)`, else one by one.
fn close_above_stderr() {
    let first: c_uint = 3;
    // SAFETY: close_range takes three integers.
    let rc = unsafe { libc::syscall(libc::SYS_close_range, first, c_uint::MAX, 0 as c_uint) };
    if rc != 0 {
        for fd in 3..65_536 {
            // SAFETY: closing a descriptor number; EBADF for unused ones.
            unsafe {
                libc::close(fd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn fd() -> OwnedFd {
        let (a, _b) = crate::channel::channel().unwrap();
        a
    }

    fn request(id: u32, args: &[&str]) -> Vec<u8> {
        let mut out = vec![MARK_SPAWN];
        out.extend_from_slice(&id.to_le_bytes());
        for arg in args {
            out.extend_from_slice(arg.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn a_request_names_its_id_its_command_and_one_terminal() {
        let parsed = parse(&request(7, &["bash", "-i"]), vec![fd()]).unwrap();
        assert_eq!(parsed.id, 7);
        let argv: Vec<&[u8]> = parsed.argv.iter().map(CString::as_bytes).collect();
        assert_eq!(argv, [b"bash".as_slice(), b"-i".as_slice()]);
    }

    #[test]
    fn anything_else_is_refused_and_closes_what_came_with_it() {
        let cases: Vec<(Vec<u8>, usize, Malformed)> = vec![
            (
                b"X".to_vec(),
                1,
                Malformed {
                    id: 0,
                    why: "not a spawn request",
                },
            ),
            (
                request(0, &["sh"]),
                1,
                Malformed {
                    id: 0,
                    why: "shell id 0",
                },
            ),
            (
                request(3, &["sh"]),
                0,
                Malformed {
                    id: 3,
                    why: "a spawn request carries exactly one descriptor",
                },
            ),
            (
                request(3, &["sh"]),
                2,
                Malformed {
                    id: 3,
                    why: "a spawn request carries exactly one descriptor",
                },
            ),
            (
                request(3, &[]),
                1,
                Malformed {
                    id: 3,
                    why: "empty command",
                },
            ),
            (
                {
                    let mut unterminated = request(3, &["sh"]);
                    unterminated.pop();
                    unterminated
                },
                1,
                Malformed {
                    id: 3,
                    why: "the last argument is not terminated",
                },
            ),
            (
                request(3, &["", "x"]),
                1,
                Malformed {
                    id: 3,
                    why: "empty program name",
                },
            ),
        ];
        for (payload, fds, expected) in cases {
            let fds: Vec<OwnedFd> = (0..fds).map(|_| fd()).collect();
            let err = parse(&payload, fds).unwrap_err();
            assert_eq!(err, expected, "{payload:?}");
        }
        let many: Vec<String> = (0..=MAX_ARGS).map(|n| n.to_string()).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        assert_eq!(
            parse(&request(4, &many), vec![fd()]).unwrap_err().why,
            "too many arguments"
        );
    }
}
