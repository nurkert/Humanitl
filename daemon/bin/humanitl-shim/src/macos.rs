//! The shim on macOS: bridge, evidence, exec (HUM-323, `backlog/sprint-6.md`).
//!
//! On macOS the launcher starts this binary under `sandbox-exec` with the
//! session's Seatbelt profile. The kernel enforces that profile on the shim and
//! on every process it starts; there is no seccomp filter to install and no
//! namespace to enter. What is left for the shim is what only something inside
//! the sandbox can do:
//!
//! 1. **The door.** Bind the TCP listener of the one `in` bridge on the
//!    session's loopback port and relay every connection to the proxy socket
//!    (`bridge.rs`, the same code as on Linux). Loopback is shared with the
//!    host on macOS, so the port is the session's own, and a port that is
//!    taken fails the start instead of sharing the door.
//! 2. **The evidence**, as the same five `CHECK` lines the launcher reads on
//!    Linux, each measured from inside the profile:
//!    - `bridge_listening`: the listener accepts a connection.
//!    - `no_interfaces` (guarantee 1, *no way out*): a TCP connection to a
//!      public IPv4 and IPv6 address and a UDP datagram end in `EPERM`.
//!    - `single_socket` (guarantee 2, *exactly one door*): the proxy socket
//!      answers, a Unix socket of the system (`/var/run/syslog`) and a
//!      loopback port that is not the session's do not.
//!    - `seccomp_applied` (guarantee 3, *the kernel enforces the policy*):
//!      `sandbox_check(2)` reports this process as sandboxed.
//!    - `families`: raw, routing and system sockets end in `EPERM`.
//! 3. **The process.** Fork; the child takes descriptor 0 as its controlling
//!    terminal when asked, resets every signal disposition, removes the
//!    shim's variables and `execvp`s the command. The parent serves the door,
//!    relays `SIGTERM`, `SIGHUP` and `SIGINT`, and waits.
//!
//! **Status.** bwrap reports `{"child-pid": N}` once the command runs and
//! `{"exit-code": N}` when it has ended on `--json-status-fd`; the launcher
//! reads exactly that to tell a command that failed from a sandbox that never
//! started. `sandbox-exec` has no such channel, so the shim writes the same
//! two lines on `--status-fd`.
//!
//! **Command line.** `humanitl-shim --proxy-port <port> [--status-fd <n>]
//! [--controlling-terminal] -- <command> [args...]`. Exit codes as on Linux:
//! 125 usage, 126 setup, 127 exec, else the command's status or 128 + signal.

use std::env;
use std::ffi::{CString, OsString};
use std::io::{self, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use libc::c_int;

use crate::bridge::{self, Bound, Bridge};

use crate::report::{Check, Report};

/// Die Variable mit dem zweiten Port der Sitzung (`ENV_AGENT_PORT`).
const AGENT_PORT_ENV: &str = "HUMANITL_AGENT_PORT";

/// Ein Port, der weder `own` noch der des Agenten ist.
fn other_port(own: u16, agent: Option<u16>) -> u16 {
    let next = |port: u16| if port == u16::MAX { 1024 } else { port + 1 };
    let mut port = next(own);
    while Some(port) == agent || port == own {
        port = next(port);
    }
    port
}

const EXIT_USAGE: i32 = 125;
const EXIT_SETUP: i32 = 126;
const EXIT_EXEC: i32 = 127;

/// The variables of the launcher contract; none of them reaches the command.
const RESERVED_ENV: [&str; 5] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
    "HUMANITL_REPORT_FD",
];

/// A public address outside every private range, for guarantee 1. Nothing
/// is sent: Seatbelt refuses the `connect(2)` before a packet exists.
const OUTSIDE_V4: ([u8; 4], u16) = ([1, 1, 1, 1], 443);
/// The same for IPv6.
const OUTSIDE_V6: ([u16; 8], u16) = ([0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111], 443);
/// A Unix socket every Mac has and that is not the door.
const FOREIGN_SOCKET: &str = "/var/run/syslog";

const USAGE: &str = "usage: humanitl-shim --proxy-port <port> [--status-fd <n>] \
[--chdir <dir>] [--controlling-terminal] -- <command> [args...]\n\
environment: HUMANITL_BRIDGES, HUMANITL_REPORT_FD\n";

/// The child's pid, for the signal handler.
static CHILD: AtomicI32 = AtomicI32::new(0);

struct Cli {
    proxy_port: u16,
    status_fd: Option<RawFd>,
    /// The working directory of the command, the project; without mounts
    /// there is no `--chdir /work` of the launcher.
    chdir: Option<OsString>,
    /// A further shell next to the agent (HUM-323): no bridge, no checks,
    /// only the process with its session.
    no_bridge: bool,
    controlling_terminal: bool,
    command: Vec<OsString>,
}

/// The entry point on macOS, called from `main.rs`.
pub(crate) fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = match parse_cli(env::args_os().skip(1)) {
        Ok(cli) => cli,
        Err(None) => {
            let _ = io::stdout().lock().write_all(USAGE.as_bytes());
            return 0;
        }
        Err(Some(err)) => {
            say(&format!("humanitl-shim: {err}\n{USAGE}"));
            return EXIT_USAGE;
        }
    };
    let report = match Report::from_env(env::var_os("HUMANITL_REPORT_FD").as_deref()) {
        Ok(report) => report,
        Err(err) => {
            say(&format!("humanitl-shim: warning: no report: {err}\n"));
            Report::none()
        }
    };
    let status = cli.status_fd.and_then(take_fd);
    if cli.no_bridge {
        return launch(&cli, None, &report, status.as_ref());
    }

    let bound = match bind_door(cli.proxy_port) {
        Ok(bound) => bound,
        Err(why) => {
            report.check(Check::BridgeListening, false, &why);
            say(&format!("humanitl-shim: bridge: {why}\n"));
            return EXIT_SETUP;
        }
    };
    match bound.self_connect() {
        Ok(()) => report.check(
            Check::BridgeListening,
            true,
            &format!("{} accepts", bound.local_addr()),
        ),
        Err(err) => {
            report.check(Check::BridgeListening, false, &err.to_string());
            say(&format!("humanitl-shim: bridge: {err}\n"));
            return EXIT_SETUP;
        }
    }
    let door = bound.bridge().socket.clone();
    let (ok, evidence) = no_way_out();
    report.check(Check::NoInterfaces, ok, &evidence);
    let (ok, evidence) = single_door(&door, bound.local_addr());
    report.check(Check::SingleSocket, ok, &evidence);
    let (ok, evidence) = profile_applied();
    report.check(Check::SeccompApplied, ok, &evidence);
    let (ok, evidence) = families_refused();
    report.check(Check::Families, ok, &evidence);

    launch(&cli, Some(bound), &report, status.as_ref())
}

fn bind_door(port: u16) -> Result<Bound, String> {
    let bridges = match env::var("HUMANITL_BRIDGES") {
        Ok(text) => bridge::parse(&text).map_err(|err| err.to_string())?,
        Err(_) => vec![Bridge::default_proxy(port)],
    };
    bridge::validate(&bridges, port).map_err(|err| err.to_string())?;
    let bridge = bridges
        .into_iter()
        .next()
        .ok_or_else(|| "no bridge".to_owned())?;
    bridge::bind(bridge).map_err(|err| err.to_string())
}

/// Guarantee 1: every way out ends in `EPERM`.
fn no_way_out() -> (bool, String) {
    let v4 = SocketAddr::from(OUTSIDE_V4);
    let v6 = SocketAddr::from(OUTSIDE_V6);
    let tcp4 = refused(TcpStream::connect_timeout(&v4, Duration::from_secs(2)).map(drop));
    let tcp6 = refused(TcpStream::connect_timeout(&v6, Duration::from_secs(2)).map(drop));
    let udp = refused(udp_send(OUTSIDE_V4.0, 53));
    let ok = tcp4.0 && tcp6.0 && udp.0;
    (
        ok,
        format!(
            "tcp {v4}: {}; tcp {v6}: {}; udp 1.1.1.1:53: {}",
            tcp4.1, tcp6.1, udp.1
        ),
    )
}

/// One datagram from an unbound socket, so that the profile has to refuse
/// `sendto(2)` itself (a `bind` first would be refused before it).
fn udp_send(address: [u8; 4], port: u16) -> io::Result<()> {
    // SAFETY: socket(2) takes three integers; the descriptor is owned below.
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd was just returned by socket(2).
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let target = libc::sockaddr_in {
        sin_len: u8::try_from(std::mem::size_of::<libc::sockaddr_in>()).unwrap_or(16),
        sin_family: u8::try_from(libc::AF_INET).unwrap_or(2),
        sin_port: port.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(address),
        },
        sin_zero: [0; 8],
    };
    // SAFETY: a one-byte buffer and a complete sockaddr_in with its length.
    let sent = unsafe {
        libc::sendto(
            socket.as_raw_fd(),
            b"x".as_ptr().cast(),
            1,
            0,
            (&raw const target).cast(),
            libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_in>()).unwrap_or(16),
        )
    };
    if sent < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Guarantee 2: the door answers, nothing else does.
fn single_door(door: &Path, own: SocketAddr) -> (bool, String) {
    let reached = UnixStream::connect(door).map(drop);
    let door_ok = reached.is_ok();
    let door_text = match reached {
        Ok(()) => "answers".to_owned(),
        Err(err) => format!("does not answer: {err}"),
    };
    let foreign = refused(UnixStream::connect(FOREIGN_SOCKET).map(drop));
    // Der zweite Port der Sitzung gehört dem Agenten (HUM-365) und taugt nicht
    // als Beispiel für einen fremden.
    let agent_port = env::var(AGENT_PORT_ENV)
        .ok()
        .and_then(|port| port.parse::<u16>().ok());
    let other_port = other_port(own.port(), agent_port);
    let other = SocketAddr::from(([127, 0, 0, 1], other_port));
    let loopback = refused(TcpStream::connect_timeout(&other, Duration::from_secs(2)).map(drop));
    (
        door_ok && foreign.0 && loopback.0,
        format!(
            "door {}: {door_text}; {FOREIGN_SOCKET}: {}; tcp {other}: {}",
            door.display(),
            foreign.1,
            loopback.1
        ),
    )
}

/// Guarantee 3: the kernel holds this process to a profile.
fn profile_applied() -> (bool, String) {
    unsafe extern "C" {
        // libsystem_sandbox; the variadic tail is unused with
        // SANDBOX_FILTER_NONE.
        fn sandbox_check(
            pid: libc::pid_t,
            operation: *const libc::c_char,
            kind: c_int,
            ...
        ) -> c_int;
    }
    // SAFETY: a null operation with SANDBOX_FILTER_NONE (0) only asks whether
    // the process is sandboxed at all; no further arguments are read.
    let answer = unsafe { sandbox_check(libc::getpid(), std::ptr::null(), 0) };
    (
        answer == 1,
        format!("sandbox_check(getpid(), NULL, 0) = {answer}"),
    )
}

/// The socket families no agent needs end in `EPERM` at `socket(2)`.
fn families_refused() -> (bool, String) {
    let probes = [
        (
            "AF_INET SOCK_RAW",
            libc::AF_INET,
            libc::SOCK_RAW,
            libc::IPPROTO_ICMP,
        ),
        ("AF_ROUTE", libc::AF_ROUTE, libc::SOCK_RAW, 0),
        (
            "AF_SYSTEM",
            libc::AF_SYSTEM,
            libc::SOCK_DGRAM,
            libc::SYSPROTO_CONTROL,
        ),
        ("AF_NDRV", libc::AF_NDRV, libc::SOCK_RAW, 0),
    ];
    let mut ok = true;
    let mut seen = Vec::new();
    for (name, family, kind, protocol) in probes {
        // SAFETY: socket(2) takes three integers; a descriptor that comes
        // back is owned and closed at once.
        let fd = unsafe { libc::socket(family, kind, protocol) };
        if fd >= 0 {
            // SAFETY: fd was just returned by socket(2) and is owned here.
            drop(unsafe { OwnedFd::from_raw_fd(fd) });
            ok = false;
            seen.push(format!("{name} opened"));
        } else {
            let err = io::Error::last_os_error();
            ok &= err.raw_os_error() == Some(libc::EPERM);
            seen.push(format!("{name} {err}"));
        }
    }
    (ok, seen.join("; "))
}

/// Whether a result is the refusal of the profile, and how it reads.
fn refused(result: io::Result<()>) -> (bool, String) {
    match result {
        Ok(()) => (false, "OPEN".to_owned()),
        Err(err) if err.raw_os_error() == Some(libc::EPERM) => (true, "EPERM".to_owned()),
        Err(err) => (false, err.to_string()),
    }
}

fn launch(cli: &Cli, bound: Option<Bound>, report: &Report, status: Option<&OwnedFd>) -> i32 {
    // SAFETY: fork(2) in a process that has started no threads yet: the
    // bridge thread starts only in the parent, below.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        say(&format!(
            "humanitl-shim: fork: {}\n",
            io::Error::last_os_error()
        ));
        return EXIT_SETUP;
    }
    if pid == 0 {
        drop(bound);
        child(cli, report);
    }
    CHILD.store(pid, Ordering::SeqCst);
    relay_signals();
    if let Some(fd) = status {
        write_line(fd, &format!("{{\"child-pid\": {pid}}}"));
    }
    if let Some(bound) = bound {
        std::thread::spawn(move || bound.serve());
    }
    std::thread::spawn(move || watchdog(pid));
    let code = wait_for(pid);
    // Whatever the command left running in its session ends with it: there
    // is no PID namespace that would take it along (HUM-323).
    end_session(pid, libc::SIGKILL);
    // Also after a signal: the command ran, and a missing line would read
    // as a sandbox that never started (`SandboxHandle::verdict`).
    if let Some(fd) = status {
        write_line(fd, &format!("{{\"exit-code\": {}}}", code.status));
    }
    code.status
}

fn child(cli: &Cli, report: &Report) -> ! {
    if let Some(dir) = &cli.chdir
        && let Err(err) = env::set_current_dir(dir)
    {
        report.exec_failed(&err);
        say(&format!(
            "humanitl-shim: chdir {}: {err}\n",
            Path::new(dir).display()
        ));
        // SAFETY: see below.
        unsafe { libc::_exit(EXIT_EXEC) }
    }
    // Always a session of its own, so that the shim can end every process the
    // command starts, and never the session of the daemon above it.
    // SAFETY: setsid(2) has no preconditions; the forked child is no group
    // leader, so it cannot fail with EPERM.
    if unsafe { libc::setsid() } < 0 {
        say("humanitl-shim: warning: setsid failed\n");
    }
    if cli.controlling_terminal {
        take_controlling_terminal();
    }
    reset_signal_dispositions();
    for name in RESERVED_ENV {
        // SAFETY: the child is single-threaded after fork.
        unsafe { env::remove_var(name) };
    }
    let err = exec(&cli.command);
    report.exec_failed(&err);
    say(&format!(
        "humanitl-shim: exec {}: {err}\n",
        cli.command
            .first()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    ));
    // SAFETY: _exit(2) ends the forked child without running the parent's
    // destructors twice.
    unsafe { libc::_exit(EXIT_EXEC) }
}

fn take_controlling_terminal() {
    // SAFETY: isatty and ioctl(TIOCSCTTY) take integers only.
    unsafe {
        if libc::isatty(0) != 1 {
            return;
        }
        if libc::ioctl(0, libc::TIOCSCTTY.into(), 0 as c_int) != 0 {
            say("humanitl-shim: warning: TIOCSCTTY failed\n");
        }
    }
}

fn reset_signal_dispositions() {
    // NSIG is 32 on macOS; libc does not export it there.
    for signal in 1..32 {
        if signal == libc::SIGKILL || signal == libc::SIGSTOP {
            continue;
        }
        // SAFETY: signal(2) with SIG_DFL; an unknown number answers SIG_ERR.
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
        }
    }
}

fn exec(command: &[OsString]) -> io::Error {
    let argv: Result<Vec<CString>, _> = command
        .iter()
        .map(|arg| CString::new(arg.as_bytes()))
        .collect();
    let Ok(argv) = argv else {
        return io::Error::new(io::ErrorKind::InvalidInput, "argument contains a NUL byte");
    };
    let Some(file) = argv.first() else {
        return io::Error::new(io::ErrorKind::InvalidInput, "empty command");
    };
    let mut pointers: Vec<*const libc::c_char> = argv.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());
    // SAFETY: file and every pointer are NUL-terminated and outlive the call,
    // and the array ends in a null pointer.
    unsafe { libc::execvp(file.as_ptr(), pointers.as_ptr()) };
    io::Error::last_os_error()
}

/// How long the command may take after `SIGTERM` or `SIGHUP` before the
/// shim ends its whole session; the daemon's own `SIGKILL` reaches only the
/// shim, so this is what takes the rest along.
const SESSION_GRACE: Duration = Duration::from_secs(3);

/// Set by the signal handler when an end was asked for.
static ENDING: AtomicI32 = AtomicI32::new(0);

/// Ends the command's session once [`SESSION_GRACE`] has passed after an
/// asked-for end.
fn watchdog(child: libc::pid_t) {
    loop {
        if ENDING.load(Ordering::SeqCst) != 0 {
            std::thread::sleep(SESSION_GRACE);
            end_session(child, libc::SIGKILL);
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Sends `signal` to every process in the session `sid`, the command's own
/// (the child made it with `setsid`). A process may fork while a pass runs,
/// so the passes repeat until one finds nobody, at most [`END_PASSES`] times.
/// A process that started yet another session escapes this; it stays under
/// the profile, but outlives the stop (`docs/SECURITY.md` section 12).
fn end_session(sid: libc::pid_t, signal: c_int) {
    for _ in 0..END_PASSES {
        if end_session_once(sid, signal) == 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// How often [`end_session`] looks again.
const END_PASSES: usize = 10;

/// One pass of [`end_session`]; how many processes it signalled.
fn end_session_once(sid: libc::pid_t, signal: c_int) -> usize {
    let own = std::process::id();
    let mut pids = vec![0 as libc::pid_t; 8192];
    let bytes = c_int::try_from(pids.len() * std::mem::size_of::<libc::pid_t>()).unwrap_or(0);
    // SAFETY: the buffer holds `bytes` bytes of pid_t.
    let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    let count = usize::try_from(count).unwrap_or(0).min(pids.len());
    let mut signalled = 0;
    for &pid in &pids[..count] {
        if pid <= 1 || u32::try_from(pid).is_ok_and(|pid| pid == own) {
            continue;
        }
        // SAFETY: getsid(2) and kill(2) take integers only.
        unsafe {
            if libc::getsid(pid) == sid && libc::kill(pid, signal) == 0 {
                signalled += 1;
            }
        }
    }
    signalled
}

extern "C" fn forward(signal: c_int) {
    if signal != libc::SIGINT {
        ENDING.store(1, Ordering::SeqCst);
    }
    let child = CHILD.load(Ordering::SeqCst);
    if child > 0 {
        // SAFETY: kill(2) is async-signal-safe.
        unsafe {
            libc::kill(child, signal);
        }
    }
}

fn relay_signals() {
    for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
        // SAFETY: the handler only reads an atomic and calls kill(2).
        unsafe {
            libc::signal(
                signal,
                forward as extern "C" fn(c_int) as libc::sighandler_t,
            );
        }
    }
}

struct Ended {
    /// What the shim exits with: the command's code, or 128 + signal.
    status: i32,
}

fn wait_for(pid: libc::pid_t) -> Ended {
    loop {
        let mut status: c_int = 0;
        // SAFETY: waitpid(2) writes the status into a valid int.
        let rc = unsafe { libc::waitpid(pid, &raw mut status, 0) };
        if rc == pid {
            if libc::WIFEXITED(status) {
                let code = libc::WEXITSTATUS(status);
                return Ended { status: code };
            }
            if libc::WIFSIGNALED(status) {
                return Ended {
                    status: 128 + libc::WTERMSIG(status),
                };
            }
            continue;
        }
        if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return Ended { status: EXIT_SETUP };
        }
    }
}

fn take_fd(fd: RawFd) -> Option<OwnedFd> {
    // SAFETY: fcntl(F_GETFD) only asks whether the number is open.
    if fd < 3 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        say(&format!(
            "humanitl-shim: warning: status descriptor {fd} is not open\n"
        ));
        return None;
    }
    // SAFETY: the launcher passed this descriptor for the shim alone.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: FD_CLOEXEC keeps it from the command.
    unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    Some(owned)
}

fn write_line(fd: &OwnedFd, line: &str) {
    let mut bytes = format!("{line}\n").into_bytes();
    let mut rest = bytes.as_mut_slice();
    while !rest.is_empty() {
        // SAFETY: rest is a valid buffer of the given length.
        let n = unsafe { libc::write(fd.as_raw_fd(), rest.as_ptr().cast(), rest.len()) };
        if n <= 0 {
            return;
        }
        rest = &mut rest[n.unsigned_abs()..];
    }
}

fn say(text: &str) {
    let _ = io::stderr().lock().write_all(text.as_bytes());
}

fn parse_cli(mut args: impl Iterator<Item = OsString>) -> Result<Cli, Option<String>> {
    let mut proxy_port = None;
    let mut status_fd = None;
    let mut chdir = None;
    let mut no_bridge = false;
    let mut controlling_terminal = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => {
                let command: Vec<OsString> = args.collect();
                if command.is_empty() {
                    return Err(Some("no command after --".to_owned()));
                }
                let proxy_port = match proxy_port {
                    Some(port) => port,
                    // A further shell has no bridge and needs no port.
                    None if no_bridge => 0,
                    None => return Err(Some("--proxy-port is missing".to_owned())),
                };
                return Ok(Cli {
                    proxy_port,
                    status_fd,
                    chdir,
                    no_bridge,
                    controlling_terminal,
                    command,
                });
            }
            Some("--help" | "-h") => return Err(None),
            Some("--controlling-terminal") => controlling_terminal = true,
            Some("--no-bridge") => no_bridge = true,
            Some("--chdir") => {
                chdir = Some(
                    args.next()
                        .ok_or_else(|| Some("--chdir needs a value".to_owned()))?,
                );
            }
            Some(flag @ ("--proxy-port" | "--status-fd")) => {
                let value = args
                    .next()
                    .and_then(|value| value.to_str().map(str::to_owned))
                    .ok_or_else(|| Some(format!("{flag} needs a value")))?;
                if flag == "--proxy-port" {
                    proxy_port = Some(
                        value
                            .parse::<u16>()
                            .map_err(|_| Some(format!("--proxy-port {value} is not a port")))?,
                    );
                } else {
                    status_fd = Some(
                        value
                            .parse::<RawFd>()
                            .map_err(|_| Some(format!("--status-fd {value} is not a number")))?,
                    );
                }
            }
            _ => {
                return Err(Some(format!(
                    "unexpected argument {}",
                    arg.to_string_lossy()
                )));
            }
        }
    }
    Err(Some("no command after --".to_owned()))
}

#[cfg(test)]
mod other_port_tests {
    use super::other_port;

    #[test]
    fn the_foreign_port_is_never_the_agents() {
        assert_eq!(other_port(4000, None), 4001);
        assert_eq!(other_port(4000, Some(4001)), 4002);
        assert_eq!(other_port(u16::MAX, Some(1024)), 1025);
    }
}
