//! The stream mode: every terminal of the sandbox in the shim, the daemon at
//! the other end of one stream socket (HUM-443).
//!
//! In a micro-VM no descriptor crosses the boundary: the daemon cannot hand
//! the agent a pseudo-terminal, `SCM_RIGHTS` cannot carry the follower of a
//! further shell (HUM-299), and no signal of the host reaches a process of
//! the guest. In this mode the shim opens the pseudo-terminals itself, and
//! terminal bytes, sizes, shells, exits and interrupts travel as frames
//! (`frame.rs`) over one stream socket: `AF_VSOCK` in the guest, where the
//! host connects to the shim's one listening port (`guest.rs`), an inherited
//! Unix socket under bwrap (`--control-stream-fd`), where the mode exists so
//! that both modes face the same terminal tests.
//!
//! # Who holds what
//!
//! Only PID 1, the shim's parent, holds the socket and the masters of the
//! pseudo-terminals, all with `CLOEXEC`. The agent's child closes them before
//! its filter (`close_inherited` keeps neither), every further shell closes
//! every descriptor above 2 before its `exec` (`exec.rs`), and PID 1 is not
//! dumpable. Nothing in the sandbox can therefore write a frame. The agent's
//! filter refuses `AF_VSOCK` and `AF_UNIX` sockets of its own; the channel
//! exists before that filter and is never the agent's. It carries terminal
//! bytes and control, never a byte for the proxy: the door is the bridge's
//! alone.
//!
//! # One loop
//!
//! The shim has no runtime. The socket, every master and the heartbeat are
//! served from PID 1's one `poll` loop (`supervise.rs`), so that the reading
//! of frames happens in one place. Nothing here blocks: the socket and the
//! masters are non-blocking, bytes for the daemon wait in a bounded outbox
//! ([`OUT_HIGH`]), bytes for a terminal in a bounded queue ([`IN_HIGH`]).
//! A daemon that stops reading stops the reading of the masters, as a daemon
//! that stops reading a master would in the descriptor mode; the control
//! frames, the lifeline and the children are still served. A terminal whose
//! queue is full loses further input for it (counted, warned about once)
//! instead of holding up the frames of every other channel. An open that
//! arrives while the outbox is full is dropped unanswered, so a flood of
//! them cannot grow the outbox.
//!
//! PID 1 never writes to a terminal itself: its standard streams stay those
//! it was started with, and only the agent's child takes the follower as
//! its own. A full terminal can therefore never hold up a message of PID 1,
//! and with it the lifeline.
//!
//! # The end of the stream is the lifeline
//!
//! When the daemon's end closes, PID 1 ends the sandbox the way it does when
//! `--lifeline-fd` ends (`supervise.rs`). In the guest this is defence in
//! depth: the host stops the micro-VM itself (HUM-444).

use std::ffi::{CStr, CString, c_char};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, Instant};

use seccompiler::BpfProgram;

use crate::frame::{self, AGENT, Decoder, Event, Kind, MAX_PAYLOAD, Message};
use crate::linux::contract::MAX_SHELLS;
use crate::linux::exec;

/// How often the shim says it is alive while the outbox is empty.
pub const HEARTBEAT: Duration = Duration::from_secs(5);

/// The most bytes that may wait for the daemon before the masters are no
/// longer read.
pub const OUT_HIGH: usize = 256 * 1024;

/// The most bytes that may wait for one terminal before the socket is no
/// longer read.
pub const IN_HIGH: usize = 64 * 1024;

/// How long the last frames may take to leave once the agent has ended.
const FINAL_FLUSH: Duration = Duration::from_secs(2);

/// `F` code: [`MAX_SHELLS`] shells are already running (as in `exec.rs`).
const REFUSED_LIMIT: i32 = -1;
/// `F` code: the request was not one the shim understands.
const REFUSED_MALFORMED: i32 = -2;

/// A pseudo-terminal the shim opened.
#[derive(Debug)]
pub struct Pty {
    /// The side PID 1 keeps, with `CLOEXEC`.
    pub master: OwnedFd,
    /// The side the process gets, with `CLOEXEC` and without becoming this
    /// process's controlling terminal.
    pub follower: OwnedFd,
}

fn last_error() -> io::Error {
    io::Error::last_os_error()
}

fn errno() -> i32 {
    last_error().raw_os_error().unwrap_or(0)
}

/// Opens a pseudo-terminal of `cols` by `rows`.
///
/// # Errors
///
/// The error of the step that failed: `posix_openpt`, `grantpt`,
/// `unlockpt`, `ptsname_r`, the `open` of the follower or the size.
pub fn open_pty(cols: u16, rows: u16) -> io::Result<Pty> {
    // SAFETY: posix_openpt takes flags and returns a new descriptor or -1.
    let fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(last_error());
    }
    // SAFETY: the descriptor was just opened and belongs to nobody else.
    let master = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: grantpt and unlockpt take the master's descriptor.
    if unsafe { libc::grantpt(master.as_raw_fd()) } != 0
        || unsafe { libc::unlockpt(master.as_raw_fd()) } != 0
    {
        return Err(last_error());
    }
    let mut name = [0 as c_char; 128];
    // SAFETY: the buffer is valid for its length, which is passed.
    let rc = unsafe { libc::ptsname_r(master.as_raw_fd(), name.as_mut_ptr(), name.len()) };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc));
    }
    // SAFETY: ptsname_r wrote a NUL-terminated string into the buffer.
    let path = unsafe { CStr::from_ptr(name.as_ptr()) };
    // SAFETY: open with a NUL-terminated path and constant flags.
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(last_error());
    }
    // SAFETY: the descriptor was just opened and belongs to nobody else.
    let follower = unsafe { OwnedFd::from_raw_fd(fd) };
    set_size(master.as_raw_fd(), cols, rows)?;
    Ok(Pty { master, follower })
}

/// Sets the size of a terminal; the kernel sends `SIGWINCH` to the
/// foreground group of a controlling terminal itself.
fn set_size(master: RawFd, cols: u16, rows: u16) -> io::Result<()> {
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCSWINSZ reads one winsize from a valid pointer.
    if unsafe { libc::ioctl(master, libc::TIOCSWINSZ as libc::Ioctl, &raw const size) } != 0 {
        return Err(last_error());
    }
    Ok(())
}

/// Hands a terminal's follower to `uid` and `gid` (HUM-445).
fn give_terminal(fd: RawFd, uid: u32, gid: u32) -> io::Result<()> {
    // SAFETY: fchown takes a descriptor this process owns and two integers.
    if unsafe { libc::fchown(fd, uid, gid) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Sets `O_NONBLOCK` and `FD_CLOEXEC` on a descriptor.
pub fn nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl with F_GETFL/F_SETFL/F_SETFD changes flags of one
    // descriptor.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(last_error());
        }
        if libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) < 0 {
            return Err(last_error());
        }
    }
    Ok(())
}

/// Takes the descriptor of `--control-stream-fd` (or the guest's accepted
/// connection): an open stream socket, made non-blocking with `CLOEXEC`.
///
/// # Errors
///
/// A short reason without spaces when the number names no stream socket.
pub fn take_control(fd: RawFd) -> Result<OwnedFd, String> {
    let mut kind: libc::c_int = 0;
    let mut len = libc::socklen_t::try_from(size_of::<libc::c_int>()).unwrap_or(4);
    // SAFETY: getsockopt writes at most `len` bytes into `kind`.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&raw mut kind).cast(),
            &raw mut len,
        )
    };
    if fd < 3 || rc != 0 {
        return Err(format!("not-a-socket:{fd}"));
    }
    if kind != libc::SOCK_STREAM {
        return Err(format!("not-a-stream-socket:{fd}"));
    }
    nonblocking(fd).map_err(|err| format!("flags:{fd}:{err}").replace(' ', "_"))?;
    // SAFETY: the launcher handed this descriptor to the shim alone, and
    // nothing else in this process owns it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// One terminal: the agent's or a further shell's.
#[derive(Debug)]
struct Terminal {
    channel: u32,
    /// The process on it; 0 until a further shell started.
    pid: libc::pid_t,
    /// `None` once the daemon closed it (`C`).
    master: Option<OwnedFd>,
    /// Bytes for the terminal that it did not take yet.
    input: Vec<u8>,
    /// The master reads end of file or `EIO`: nobody holds the follower.
    drained: bool,
}

impl Terminal {
    fn master_fd(&self) -> Option<RawFd> {
        self.master.as_ref().map(AsRawFd::as_raw_fd)
    }
}

/// Which slots of PID 1's `poll` array belong to the stream.
#[derive(Debug, Default)]
pub struct Slots {
    control: Option<usize>,
    terminals: Vec<(u32, usize)>,
}

/// The stream mode of one sandbox, served from PID 1's loop.
pub struct Session {
    control: OwnedFd,
    decoder: Decoder,
    /// Bytes for the daemon, oldest first.
    out: Vec<u8>,
    /// The agent's terminal first, then every further shell.
    terminals: Vec<Terminal>,
    /// The agent's filter, answering `EPERM` itself, for further shells;
    /// `None` until the agent runs ([`Session::start`]).
    program: Option<BpfProgram>,
    /// The environment of further shells.
    env: Vec<CString>,
    /// This process, for the shell child's check against
    /// `PR_SET_PDEATHSIG`.
    parent: libc::pid_t,
    /// The daemon's end is still open.
    open: bool,
    /// The daemon has sized the agent's terminal at least once.
    sized: bool,
    /// The follower of the agent's terminal until the agent's child took
    /// it ([`Session::release_follower`]).
    follower: Option<OwnedFd>,
    /// Bytes of input dropped because a terminal's queue was full.
    dropped_input: u64,
    /// Opens dropped unanswered because the outbox was full.
    dropped_opens: u64,
    next_heartbeat: Instant,
    /// The user and group further shells run as in a guest kernel
    /// (HUM-445); `None` under bwrap and the container engine, where the
    /// sandbox itself holds no root.
    identity: Option<(u32, u32)>,
}

impl Session {
    /// The session over the daemon's socket, with the agent's terminal on
    /// channel 0. `hidden` are the shim's own variables, which no shell
    /// sees. Shells open only once the agent runs ([`Session::start`]).
    pub fn new(control: OwnedFd, pty: Pty, hidden: &[&str]) -> Self {
        let Pty {
            master: agent_master,
            follower,
        } = pty;
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
            decoder: Decoder::new(),
            out: Vec::new(),
            terminals: vec![Terminal {
                channel: AGENT,
                pid: 0,
                master: Some(agent_master),
                input: Vec::new(),
                drained: false,
            }],
            program: None,
            env,
            // SAFETY: getpid has no preconditions.
            parent: unsafe { libc::getpid() },
            open: true,
            sized: false,
            follower: Some(follower),
            dropped_input: 0,
            dropped_opens: 0,
            next_heartbeat: Instant::now(),
            identity: None,
        }
    }

    /// The agent and further shells give up root and become `uid` and
    /// `gid` before their filter (HUM-445, a guest kernel only). Their
    /// terminals belong to that user, as a terminal opened by bwrap's user
    /// does, so that a program can open `/dev/pts/N` again by name; the
    /// agent's terminal now, every further one when it is opened.
    ///
    /// # Errors
    ///
    /// `fchown(2)` of the agent's terminal failed.
    pub fn run_as(&mut self, uid: u32, gid: u32) -> io::Result<()> {
        self.identity = Some((uid, gid));
        match self.follower_fd() {
            Some(fd) => give_terminal(fd, uid, gid),
            None => Ok(()),
        }
    }

    /// The agent runs as `agent`; further shells get `program`, the agent's
    /// filter that answers `EPERM` itself.
    pub fn start(&mut self, agent: libc::pid_t, program: BpfProgram) {
        if let Some(terminal) = self.terminal(AGENT) {
            terminal.pid = agent;
        }
        self.program = Some(program);
    }

    /// The descriptors PID 1 keeps across its close of everything inherited.
    pub fn fds(&self) -> Vec<RawFd> {
        self.terminals
            .iter()
            .filter_map(Terminal::master_fd)
            .chain([self.control.as_raw_fd()])
            .chain(self.follower_fd())
            .collect()
    }

    /// The follower of the agent's terminal, for the agent's child, which
    /// makes it descriptors 0, 1 and 2 and its controlling terminal.
    pub fn follower_fd(&self) -> Option<RawFd> {
        self.follower.as_ref().map(AsRawFd::as_raw_fd)
    }

    /// The agent's child has its copy; PID 1 lets go of its own, so that it
    /// never holds the agent's terminal.
    pub fn release_follower(&mut self) {
        self.follower = None;
    }

    /// Waits at most `wait` for the daemon's first frame, the size of the
    /// agent's terminal, so that the agent starts with it. Frames before it
    /// are served as usual; a daemon that sends none leaves the size the
    /// terminal was opened with.
    pub fn await_size(&mut self, wait: Duration) {
        let deadline = Instant::now() + wait;
        while self.open && !self.sized {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            let mut poll = libc::pollfd {
                fd: self.control.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let millis = i32::try_from(left.as_millis().saturating_add(1)).unwrap_or(i32::MAX);
            // SAFETY: one pollfd, valid for the call.
            if unsafe { libc::poll(&raw mut poll, 1, millis) } > 0 {
                self.read_control();
            }
        }
    }

    /// Whether the daemon's end has closed: the lifeline ended.
    pub const fn closed(&self) -> bool {
        !self.open
    }

    /// Milliseconds until the next heartbeat, for `poll`.
    pub fn timeout_ms(&self) -> i32 {
        if !self.open {
            return -1;
        }
        let left = self
            .next_heartbeat
            .saturating_duration_since(Instant::now());
        i32::try_from(left.as_millis().saturating_add(1)).unwrap_or(i32::MAX)
    }

    /// Adds the stream's descriptors to PID 1's `poll` array.
    pub fn register(&self, fds: &mut Vec<libc::pollfd>) -> Slots {
        let mut slots = Slots::default();
        let mut slot = |fd: RawFd, events: libc::c_short| {
            fds.push(libc::pollfd {
                fd,
                events,
                revents: 0,
            });
            fds.len() - 1
        };
        if self.open {
            // Always read: a terminal with a full queue loses its own input
            // ([`Session::act`]), never the frames of the other channels.
            let mut events = libc::POLLIN;
            if !self.out.is_empty() {
                events |= libc::POLLOUT;
            }
            slots.control = Some(slot(self.control.as_raw_fd(), events));
        }
        for terminal in &self.terminals {
            let Some(fd) = terminal.master_fd() else {
                continue;
            };
            let mut events = 0;
            // Backpressure towards the terminals: no new bytes while the
            // daemon has not taken what waits. Once the stream has ended,
            // every terminal is read and discarded, so that no process
            // blocks on a full terminal while the sandbox ends.
            if !terminal.drained && (!self.open || self.out.len() < OUT_HIGH) {
                events |= libc::POLLIN;
            }
            if !terminal.input.is_empty() {
                events |= libc::POLLOUT;
            }
            if events != 0 {
                slots.terminals.push((terminal.channel, slot(fd, events)));
            }
        }
        slots
    }

    /// Serves whatever `poll` found ready, then the heartbeat.
    pub fn serve(&mut self, fds: &[libc::pollfd], slots: &Slots) {
        for (channel, index) in &slots.terminals {
            let revents = fds[*index].revents;
            if revents & libc::POLLOUT != 0 {
                self.write_terminal(*channel);
            }
            if revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                self.read_terminal(*channel);
            }
        }
        if let Some(index) = slots.control {
            let revents = fds[index].revents;
            if revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                self.read_control();
            }
        }
        if self.open && Instant::now() >= self.next_heartbeat {
            if self.out.is_empty() {
                let _ = frame::encode(Kind::Heartbeat, AGENT, &[], &mut self.out);
            }
            self.next_heartbeat = Instant::now() + HEARTBEAT;
        }
        self.flush();
    }

    /// A child ended; when it was a further shell, its last bytes and its
    /// `E` go to the daemon. `false` when the pid is none of the shells.
    pub fn reaped(&mut self, pid: libc::pid_t, status: libc::c_int) -> bool {
        let Some(index) = self
            .terminals
            .iter()
            .position(|t| t.channel != AGENT && t.pid == pid)
        else {
            return false;
        };
        let channel = self.terminals[index].channel;
        self.drain_terminal(channel);
        let terminal = self.terminals.remove(index);
        drop(terminal);
        if self.open {
            frame::encode_value(Kind::Exit, channel, exit_code(status), &mut self.out);
        }
        self.flush();
        true
    }

    /// Kills every further shell; for the end of the sandbox.
    pub fn kill_shells(&self) {
        for terminal in &self.terminals {
            if terminal.channel != AGENT && terminal.pid > 0 {
                // SAFETY: kill takes two integers.
                unsafe {
                    libc::kill(terminal.pid, libc::SIGKILL);
                }
            }
        }
    }

    /// The agent ended with `code`: its last bytes, then its `E`, then
    /// every frame that waits, with at most [`FINAL_FLUSH`] for the daemon
    /// to take them.
    pub fn finish(&mut self, code: i32) {
        if !self.open {
            return;
        }
        self.drain_terminal(AGENT);
        frame::encode_value(Kind::Exit, AGENT, code, &mut self.out);
        let deadline = Instant::now() + FINAL_FLUSH;
        while self.open && !self.out.is_empty() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            let mut poll = libc::pollfd {
                fd: self.control.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            let millis = i32::try_from(left.as_millis().saturating_add(1)).unwrap_or(i32::MAX);
            // SAFETY: one pollfd, valid for the call.
            if unsafe { libc::poll(&raw mut poll, 1, millis) } < 0 && errno() != libc::EINTR {
                break;
            }
            self.flush();
        }
    }

    /// The daemon's end is gone: nothing more leaves, and what waits for it
    /// is dropped.
    fn close(&mut self) {
        self.open = false;
        self.out = Vec::new();
    }

    fn terminal(&mut self, channel: u32) -> Option<&mut Terminal> {
        self.terminals.iter_mut().find(|t| t.channel == channel)
    }

    /// Reads the control socket once and acts on every frame it completed.
    fn read_control(&mut self) {
        let mut buffer = [0u8; MAX_PAYLOAD];
        // SAFETY: the buffer is valid for its length and outlives the call.
        let n = unsafe {
            libc::read(
                self.control.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        let read = match usize::try_from(n) {
            Ok(0) => {
                self.close();
                return;
            }
            Ok(read) => read,
            Err(_) => {
                let err = errno();
                if err != libc::EAGAIN && err != libc::EINTR {
                    self.close();
                }
                return;
            }
        };
        // Every frame is acted on as it completes, so that the reader never
        // holds more than the one frame it is collecting.
        let mut decoder = std::mem::take(&mut self.decoder);
        decoder.push(&buffer[..read], |event| match event {
            Event::Frame(frame) => match Message::parse(&frame) {
                Ok(message) => self.act(message),
                Err(frame::Malformed(why)) => warn("a malformed frame was dropped: ", why),
            },
            Event::Dropped(_) => warn("a frame was dropped: ", "oversized or unknown kind"),
        });
        self.decoder = decoder;
    }

    /// One frame from the daemon. Frames that travel the other way, and
    /// every channel that is not open, are dropped.
    fn act(&mut self, message: Message) {
        match message {
            Message::Data { channel, bytes } => {
                let mut full = false;
                if let Some(terminal) = self.terminal(channel)
                    && terminal.master.is_some()
                {
                    if terminal.input.len() >= IN_HIGH {
                        full = true;
                    } else {
                        terminal.input.extend_from_slice(&bytes);
                    }
                }
                if full {
                    if self.dropped_input == 0 {
                        warn(
                            "input for a terminal was dropped: ",
                            "its process does not read",
                        );
                    }
                    self.dropped_input += u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                } else {
                    self.write_terminal(channel);
                }
            }
            Message::Size {
                channel,
                cols,
                rows,
            } => {
                if let Some(fd) = self.terminal(channel).and_then(|t| t.master_fd()) {
                    let _ = set_size(fd, cols, rows);
                    self.sized |= channel == AGENT;
                }
            }
            Message::Open {
                channel,
                cols,
                rows,
                argv,
            } => self.open_shell(channel, cols, rows, &argv),
            Message::Close { channel } => {
                // The hang-up of the terminal ends the shell; its `E` comes
                // when it is reaped.
                if let Some(terminal) = self.terminal(channel) {
                    terminal.master = None;
                    terminal.input.clear();
                }
            }
            Message::Interrupt { channel } => {
                if let Some(terminal) = self.terminals.iter().find(|t| t.channel == channel) {
                    interrupt(terminal);
                }
            }
            Message::Heartbeat
            | Message::Started { .. }
            | Message::Refused { .. }
            | Message::Exit { .. } => {}
        }
    }

    /// Opens a further shell on its own pseudo-terminal, as the door of
    /// `exec.rs` does with the daemon's.
    fn open_shell(&mut self, channel: u32, cols: u16, rows: u16, argv: &[Vec<u8>]) {
        // A daemon that sends opens and reads nothing must not grow the
        // outbox with their answers: dropped unanswered, the daemon's wait
        // for an answer ends on its own.
        if self.out.len() >= OUT_HIGH {
            if self.dropped_opens == 0 {
                warn("a shell request was dropped: ", "the daemon reads nothing");
            }
            self.dropped_opens += 1;
            return;
        }
        let Some(program) = self.program.as_ref() else {
            frame::encode_value(Kind::Refused, channel, REFUSED_MALFORMED, &mut self.out);
            return;
        };
        let live = self.terminals.iter().filter(|t| t.channel != AGENT).count();
        if live >= MAX_SHELLS {
            frame::encode_value(Kind::Refused, channel, REFUSED_LIMIT, &mut self.out);
            return;
        }
        if self.terminals.iter().any(|t| t.channel == channel) {
            frame::encode_value(Kind::Refused, channel, REFUSED_MALFORMED, &mut self.out);
            return;
        }
        let argv: Vec<CString> = argv
            .iter()
            .filter_map(|arg| CString::new(arg.as_slice()).ok())
            .collect();
        let identity = self.identity;
        let pty = match open_pty(cols, rows).and_then(|pty| {
            nonblocking(pty.master.as_raw_fd())?;
            if let Some((uid, gid)) = identity {
                give_terminal(pty.follower.as_raw_fd(), uid, gid)?;
            }
            Ok(pty)
        }) {
            Ok(pty) => pty,
            Err(err) => {
                let code = err.raw_os_error().unwrap_or(libc::EIO);
                frame::encode_value(Kind::Refused, channel, code, &mut self.out);
                return;
            }
        };
        // Everything the child needs exists before the fork: this process
        // has other threads, and the child may only make calls that are
        // async-signal-safe.
        let argv_ptrs = exec::pointers(&argv);
        let envp = exec::pointers(&self.env);
        // SAFETY: the child runs only `exec::child`, which allocates nothing
        // and calls only async-signal-safe functions until `execvpe`.
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            frame::encode_value(Kind::Refused, channel, errno(), &mut self.out);
            return;
        }
        if pid == 0 {
            exec::child(
                self.parent,
                pty.follower.as_raw_fd(),
                &argv_ptrs,
                &envp,
                program,
                self.identity,
            );
        }
        drop(pty.follower);
        self.terminals.push(Terminal {
            channel,
            pid,
            master: Some(pty.master),
            input: Vec::new(),
            drained: false,
        });
        frame::encode_value(Kind::Started, channel, pid, &mut self.out);
    }

    /// Writes what waits for a terminal, as far as it takes it.
    fn write_terminal(&mut self, channel: u32) {
        let Some(terminal) = self.terminal(channel) else {
            return;
        };
        let Some(fd) = terminal.master_fd() else {
            terminal.input.clear();
            return;
        };
        while !terminal.input.is_empty() {
            // SAFETY: the pointer and length describe the queue.
            let n =
                unsafe { libc::write(fd, terminal.input.as_ptr().cast(), terminal.input.len()) };
            match usize::try_from(n) {
                Ok(0) => break,
                Ok(written) => {
                    terminal.input.drain(..written.min(terminal.input.len()));
                }
                Err(_) => {
                    if errno() != libc::EINTR {
                        if errno() != libc::EAGAIN {
                            // The terminal is gone; nothing will take it.
                            terminal.input.clear();
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Reads one chunk of a terminal into the outbox.
    fn read_terminal(&mut self, channel: u32) {
        let mut buffer = [0u8; MAX_PAYLOAD];
        let open = self.open;
        let Some(terminal) = self.terminal(channel) else {
            return;
        };
        let Some(fd) = terminal.master_fd() else {
            return;
        };
        // SAFETY: the buffer is valid for its length and outlives the call.
        let n = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        match usize::try_from(n) {
            Ok(0) => terminal.drained = true,
            // The stream has ended: read on, so that nobody blocks on a
            // full terminal, and keep nothing.
            Ok(_) if !open => {}
            Ok(read) => frame::encode_data(channel, &buffer[..read], &mut self.out),
            Err(_) => {
                let err = errno();
                // `EIO`: nobody holds the follower any more.
                if err != libc::EAGAIN && err != libc::EINTR {
                    terminal.drained = true;
                }
            }
        }
    }

    /// Reads whatever a terminal still holds, without waiting; for its end.
    fn drain_terminal(&mut self, channel: u32) {
        for _ in 0..(OUT_HIGH / MAX_PAYLOAD) {
            let before = self.out.len();
            self.read_terminal(channel);
            if self.out.len() == before {
                break;
            }
        }
    }

    /// Sends what waits, as far as the socket takes it.
    fn flush(&mut self) {
        while self.open && !self.out.is_empty() {
            // SAFETY: the pointer and length describe the outbox.
            let n = unsafe {
                libc::send(
                    self.control.as_raw_fd(),
                    self.out.as_ptr().cast(),
                    self.out.len(),
                    libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
                )
            };
            if let Ok(sent) = usize::try_from(n) {
                self.out.drain(..sent.min(self.out.len()));
                continue;
            }
            let err = errno();
            if err == libc::EINTR {
                continue;
            }
            if err != libc::EAGAIN {
                self.close();
            }
            return;
        }
    }
}

/// `SIGINT` to the foreground group of a terminal, else to the group of its
/// process: the byte `0x03` reaches a program in raw mode as a byte, and the
/// host's signals do not reach a guest at all.
fn interrupt(terminal: &Terminal) {
    let group = terminal
        .master_fd()
        // SAFETY: tcgetpgrp takes a descriptor.
        .map(|fd| unsafe { libc::tcgetpgrp(fd) })
        .filter(|group| *group > 0)
        .unwrap_or(terminal.pid);
    if group > 0 {
        // SAFETY: kill takes two integers; a negative pid names a group.
        unsafe {
            libc::kill(-group, libc::SIGINT);
        }
    }
}

/// The exit code of a wait status: the code, or 128 + signal.
fn exit_code(status: libc::c_int) -> i32 {
    if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        libc::WEXITSTATUS(status)
    }
}

/// One line on stderr: the reason a frame was dropped.
fn warn(what: &str, why: &str) {
    crate::linux::say(&format!("humanitl-shim: warning: {what}{why}\n"));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A stream pair, both ends non-blocking: (the shim's, the daemon's).
    fn pair() -> (OwnedFd, OwnedFd) {
        let mut fds = [0 as libc::c_int; 2];
        // SAFETY: `fds` is a valid two-element array for socketpair.
        let rc = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        };
        assert_eq!(rc, 0);
        // SAFETY: both were just created.
        let (a, b) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        nonblocking(a.as_raw_fd()).unwrap();
        (a, b)
    }

    fn session() -> (Session, OwnedFd, OwnedFd) {
        let (shim, daemon) = pair();
        let pty = open_pty(80, 24).unwrap();
        nonblocking(pty.master.as_raw_fd()).unwrap();
        let follower = pty.follower.try_clone().unwrap();
        let mut session = Session::new(shim, pty, &[]);
        session.release_follower();
        (session, daemon, follower)
    }

    /// One round of the loop, as `supervise.rs` runs it.
    fn round(session: &mut Session) {
        let mut fds = Vec::new();
        let slots = session.register(&mut fds);
        let count = libc::nfds_t::try_from(fds.len()).unwrap();
        // SAFETY: `fds` holds `count` entries.
        unsafe { libc::poll(fds.as_mut_ptr(), count, 50) };
        session.serve(&fds, &slots);
    }

    fn send(fd: &OwnedFd, bytes: &[u8]) {
        // SAFETY: the pointer and length describe `bytes`.
        let n = unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        assert_eq!(usize::try_from(n).unwrap(), bytes.len());
    }

    fn read_some(fd: RawFd) -> Vec<u8> {
        let mut buffer = [0u8; 4096];
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one pollfd; the buffer outlives the read.
        unsafe {
            if libc::poll(&raw mut poll, 1, 2000) <= 0 {
                return Vec::new();
            }
            let n = libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len());
            buffer[..usize::try_from(n).unwrap_or(0)].to_vec()
        }
    }

    /// Hostile frames from the daemon's side: oversized, unknown kinds,
    /// unknown channels, malformed payloads, frames that travel the other
    /// way. Each is dropped; the stream stays open and the frame after them
    /// still reaches the terminal.
    #[test]
    fn hostile_frames_are_dropped_and_the_session_stays_in_step() {
        let (mut session, daemon, follower) = session();
        let mut wire = Vec::new();
        // Oversized: declared far beyond the limit, and the bytes follow.
        let big = 2 * MAX_PAYLOAD;
        wire.push(b'D');
        wire.extend_from_slice(&0u32.to_le_bytes());
        wire.extend_from_slice(&u32::try_from(big).unwrap().to_le_bytes());
        wire.extend(std::iter::repeat_n(b'z', big));
        // Unknown kind with a payload.
        wire.extend_from_slice(&[b'Q', 0, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c']);
        // Data for a channel nobody opened, a size for one, a close of one.
        frame::encode_data(77, b"lost", &mut wire);
        frame::encode_size(77, 1, 1, &mut wire);
        frame::encode(Kind::Close, 77, &[], &mut wire).unwrap();
        // Malformed: a size of three bytes, an open on channel 0.
        frame::encode(Kind::Size, AGENT, b"abc", &mut wire).unwrap();
        let mut open = vec![b'O', 0, 0, 0, 0, 6, 0, 0, 0];
        open.extend_from_slice(b"\x50\0\x18\0s\0");
        wire.extend(open);
        // Frames that travel the other way.
        frame::encode_value(Kind::Exit, AGENT, 9, &mut wire);
        frame::encode_value(Kind::Started, 5, 1, &mut wire);
        // And then the one that counts.
        frame::encode_data(AGENT, b"after\n", &mut wire);
        for chunk in wire.chunks(4096) {
            send(&daemon, chunk);
            round(&mut session);
        }
        for _ in 0..5 {
            round(&mut session);
        }
        assert!(!session.closed(), "a hostile frame never ends the stream");
        assert!(session.decoder.buffered() <= frame::HEADER + MAX_PAYLOAD);
        assert_eq!(session.terminals.len(), 1, "no shell came of it");
        let mut seen = Vec::new();
        for _ in 0..20 {
            seen.extend(read_some(follower.as_raw_fd()));
            if seen.windows(5).any(|w| w == b"after") {
                break;
            }
        }
        let text = String::from_utf8_lossy(&seen);
        assert!(text.contains("after"), "{text:?}");
        assert!(!text.contains("lost") && !text.contains('z'), "{text:?}");
    }

    /// Terminal bytes go out as `D` frames of channel 0, and a `W` sets the
    /// size of the agent's terminal.
    #[test]
    fn output_leaves_as_frames_and_a_size_reaches_the_terminal() {
        let (mut session, daemon, follower) = session();
        send(&follower, b"hello");
        let mut wire = Vec::new();
        frame::encode_size(AGENT, 132, 43, &mut wire);
        send(&daemon, &wire);
        let mut decoder = Decoder::new();
        let mut got = Vec::new();
        for _ in 0..20 {
            round(&mut session);
            decoder.push(&read_some(daemon.as_raw_fd()), |event| got.push(event));
            if got
                .iter()
                .any(|e| matches!(e, Event::Frame(f) if f.kind == Kind::Data))
            {
                break;
            }
        }
        let data: Vec<u8> = got
            .iter()
            .filter_map(|e| match e {
                Event::Frame(f) if f.kind == Kind::Data && f.channel == AGENT => {
                    Some(f.payload.clone())
                }
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(data, b"hello");
        let mut size: libc::winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCGWINSZ fills one winsize.
        unsafe {
            libc::ioctl(
                follower.as_raw_fd(),
                libc::TIOCGWINSZ as libc::Ioctl,
                &raw mut size,
            );
        }
        assert_eq!((size.ws_col, size.ws_row), (132, 43));
    }

    /// Raw mode on a follower: no echo, no line discipline in the way.
    fn raw(follower: &OwnedFd) {
        // SAFETY: termios is plain data; tcgetattr fills it, cfmakeraw and
        // tcsetattr read it.
        unsafe {
            let mut mode: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(follower.as_raw_fd(), &raw mut mode), 0);
            libc::cfmakeraw(&raw mut mode);
            assert_eq!(
                libc::tcsetattr(follower.as_raw_fd(), libc::TCSANOW, &raw const mode),
                0
            );
        }
    }

    /// Every message in what the session wants to send.
    fn messages(out: &[u8]) -> Vec<Message> {
        let mut decoder = Decoder::new();
        let mut found = Vec::new();
        decoder.push(out, |event| {
            if let Event::Frame(frame) = event {
                found.push(Message::parse(&frame).unwrap());
            }
        });
        found
    }

    /// Review finding 2: a terminal whose process does not read loses its
    /// own input once its queue is full; an interrupt and a size for the
    /// same terminal still arrive, because the stream is still read.
    #[test]
    fn a_full_input_queue_loses_its_own_input_and_never_the_control_frames() {
        let (mut session, daemon, follower) = session();
        raw(&follower);
        nonblocking(daemon.as_raw_fd()).unwrap();
        let mut sleeper = std::process::Command::new("sleep");
        std::os::unix::process::CommandExt::process_group(sleeper.arg("30"), 0);
        let mut sleeper = sleeper.spawn().unwrap();
        let pid = libc::pid_t::try_from(sleeper.id()).unwrap();
        session.start(pid, BpfProgram::new());
        let chunk = vec![b'a'; MAX_PAYLOAD];
        let mut frames = Vec::new();
        frame::encode_data(AGENT, &chunk, &mut frames);
        for _ in 0..(2 * IN_HIGH / MAX_PAYLOAD + 8) {
            send(&daemon, &frames);
            round(&mut session);
        }
        round(&mut session);
        assert!(session.dropped_input > 0, "nothing was dropped");
        assert!(session.terminals[0].input.len() <= IN_HIGH + MAX_PAYLOAD);
        let mut control = Vec::new();
        frame::encode_size(AGENT, 111, 33, &mut control);
        frame::encode(Kind::Interrupt, AGENT, &[], &mut control).unwrap();
        send(&daemon, &control);
        for _ in 0..10 {
            round(&mut session);
        }
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCGWINSZ fills one winsize.
        unsafe {
            libc::ioctl(
                follower.as_raw_fd(),
                libc::TIOCGWINSZ as libc::Ioctl,
                &raw mut size,
            );
        }
        assert_eq!((size.ws_col, size.ws_row), (111, 33), "the size arrived");
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = sleeper.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "the interrupt did not arrive");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(libc::SIGINT)
        );
    }

    /// Review finding 3: while the outbox is full, a flood of opens grows
    /// neither the outbox nor the shells.
    #[test]
    fn a_flood_of_opens_cannot_grow_the_outbox() {
        let (mut session, _daemon, _follower) = session();
        session.start(0, BpfProgram::new());
        session.out = vec![0; OUT_HIGH];
        for channel in 1..=1000 {
            session.act(Message::Open {
                channel,
                cols: 80,
                rows: 24,
                argv: vec![b"true".to_vec()],
            });
        }
        assert_eq!(session.out.len(), OUT_HIGH);
        assert_eq!(session.terminals.len(), 1);
        assert_eq!(session.dropped_opens, 1000);
    }

    /// Review finding 4: a valid open starts a shell and answers `R`; the
    /// fifth live shell is refused with the limit, a second open of a live
    /// id as malformed, an open before the agent runs as malformed; every
    /// shell's end becomes its `E`.
    #[test]
    fn opens_start_shells_up_to_the_limit_and_refuse_the_rest() {
        let (mut session, daemon, _follower) = session();
        let open = |channel| Message::Open {
            channel,
            cols: 80,
            rows: 24,
            argv: vec![b"/bin/sleep".to_vec(), b"30".to_vec()],
        };
        session.act(open(9));
        assert_eq!(
            messages(&session.out),
            [Message::Refused {
                channel: 9,
                code: REFUSED_MALFORMED
            }],
            "no shell before the agent runs"
        );
        session.out.clear();
        session.start(0, BpfProgram::new());
        for channel in 1..=u32::try_from(MAX_SHELLS).unwrap() {
            session.act(open(channel));
        }
        session.act(open(1));
        session.act(open(77));
        let answers = messages(&session.out);
        session.out.clear();
        let mut pids = Vec::new();
        for (index, answer) in answers.iter().take(MAX_SHELLS).enumerate() {
            let Message::Started { channel, pid } = answer else {
                panic!("{answers:?}");
            };
            assert_eq!(*channel as usize, index + 1);
            pids.push(*pid);
        }
        assert_eq!(
            answers[MAX_SHELLS..],
            [
                Message::Refused {
                    channel: 1,
                    code: REFUSED_LIMIT
                },
                Message::Refused {
                    channel: 77,
                    code: REFUSED_LIMIT
                },
            ]
        );
        // Each shell child fails before `exec` here (an empty filter is no
        // filter it accepts) and ends; its reap becomes its `E`.
        session.kill_shells();
        for pid in pids {
            let mut status = 0;
            // SAFETY: waitpid on our own child with a valid pointer.
            assert_eq!(unsafe { libc::waitpid(pid, &raw mut status, 0) }, pid);
            assert!(session.reaped(pid, status));
        }
        // `reaped` sends at once; what did not fit waits in the outbox.
        let mut sent = Vec::new();
        for _ in 0..20 {
            let chunk = read_some(daemon.as_raw_fd());
            if chunk.is_empty() {
                break;
            }
            sent.extend(chunk);
        }
        sent.extend_from_slice(&session.out);
        // The last bytes of a shell come before its `E`.
        let ends: Vec<Message> = messages(&sent)
            .into_iter()
            .filter(|m| matches!(m, Message::Exit { .. }))
            .collect();
        assert_eq!(ends.len(), MAX_SHELLS, "{ends:?}");
        assert_eq!(session.terminals.len(), 1);
    }

    /// The end of the daemon's side is seen as the end of the lifeline.
    #[test]
    fn the_end_of_the_stream_closes_the_session() {
        let (mut session, daemon, _follower) = session();
        drop(daemon);
        round(&mut session);
        assert!(session.closed());
        let mut fds = Vec::new();
        let slots = session.register(&mut fds);
        assert!(slots.control.is_none(), "a closed stream is not polled");
    }
}
