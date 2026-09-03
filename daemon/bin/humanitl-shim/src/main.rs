//! Launcher inside the sandbox: bridges first, then the seccomp filter, then
//! the agent (HUM-012, ADR-002, `docs/SECURITY.md` Satz 3).
//!
//! The shim is the first process the launcher starts under bwrap's init. It
//! resolves the review finding "seccomp after socat" by process separation:
//! the bridge to the proxy needs `socket(2)` and lives in the parent; the
//! agent is a child that carries the filter before `exec` and can never shed
//! it. Deliberately dependency-free (`libc` and `seccompiler` only, no tokio,
//! no workspace crate) so the security-critical steps stay auditable in four
//! short files: this one (process model), `seccomp.rs` (the filter as data),
//! `bridge.rs` (the bytes), `report.rs` (the evidence).
//!
//! # Launcher <-> shim contract (binding for HUM-011, HUM-012, HUM-013)
//!
//! **Command line.** `humanitl-shim --proxy-port <port> -- <command> [args...]`,
//! the tail of the bwrap argument vector from
//! `humanitl_sandbox::SandboxProfile::to_bwrap_args`, where the shim itself
//! is `/usr/local/bin/humanitl-shim` inside the sandbox. `--proxy-port` is
//! the port the agent's `HTTP_PROXY` names; the shim refuses to start when
//! no `in` bridge listens there. `humanitl-shim --rules` prints the seccomp
//! rule table the same environment would produce, one row per line, and
//! starts nothing.
//!
//! **Environment** (set by the launcher with `--setenv` after `--clearenv`;
//! none of it reaches the agent, the child removes the five variables before
//! `exec`):
//!
//! - `HUMANITL_BRIDGES`: JSON array of `{"name","dir","listen","socket"}`
//!   from the profile's `[network].bridges` (MVP: exactly one, `dir` `in`,
//!   `listen` `127.0.0.1:3128`, `socket` `/run/humanitl/proxy.sock`). Absent
//!   means that one bridge on `--proxy-port`.
//! - `HUMANITL_SECCOMP_FAMILIES`: comma list from `[seccomp].allow_families`
//!   (absent: `AF_INET,AF_INET6`).
//! - `HUMANITL_SECCOMP_TYPES`: comma list from `allow_types` (absent:
//!   `SOCK_STREAM`).
//! - `HUMANITL_SECCOMP_DENY`: comma list of `deny_syscalls`; the floor from
//!   `seccomp::FLOOR` is always added.
//! - `HUMANITL_REPORT_FD`: optional descriptor number, the inherited write
//!   end of a pipe, on which the shim writes one line per check before
//!   `exec`: `CHECK <name> <ok|fail> <evidence>` for `bridge_listening`,
//!   `no_interfaces`, `single_socket`, `seccomp_applied`, `families`
//!   (HUM-041). Absent means
//!   no report. The descriptor is opened before anything that can fail, so
//!   that a setup failure is a failed check line and not only a message on
//!   stderr: an unreadable policy or bridge list, a port that is taken, a
//!   listener that does not accept and the parent's own filter all end in
//!   `CHECK seccomp_applied fail ...` or `CHECK bridge_listening fail ...`
//!   before the shim exits 126.
//!
//! **Behaviour, in order.**
//!
//! 1. Parse the command line, then open the report.
//! 2. Read the environment. For every `in` bridge, bind the TCP listener in the parent and
//!    connect to it once (`bridge_listening`); read the interface list
//!    (`no_interfaces`); walk the filesystem for Unix sockets that are not a
//!    bridge's (`single_socket`). The last two are reported, not enforced: the
//!    shim can neither remove an interface nor unmount a socket, bwrap's
//!    `--unshare-net` and the profile's mounts are the guarantee, and the
//!    launcher's isolation check is the enforcement point.
//! 3. Fork. The child dies with the parent (`PR_SET_PDEATHSIG`), closes
//!    every inherited descriptor but 0, 1, 2 and the report, sets
//!    `PR_SET_NO_NEW_PRIVS`, installs the filter with `TSYNC`, proves it
//!    (`seccomp_applied`, `families`), and `execvp`s the command. The parent
//!    installs its own, slightly wider filter (the agent's plus `AF_UNIX`),
//!    forwards `SIGTERM`, `SIGINT` and `SIGHUP` to the child, serves the
//!    bridges and waits.
//! 4. Exit status: 125 usage, 126 seccomp or bridge setup failed (the message
//!    names which), 127 `exec` failed, otherwise the child's status, or
//!    128 + signal when the child was killed. A bridge with direction `out`
//!    is 126 with "bridge direction out not supported yet".
//!
//! **Host side (HUM-013).** The proxy listens on
//! `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` (`Paths::proxy_socket()`,
//! directory 0700, socket 0600). The launcher bind-mounts only that socket
//! file to `/run/humanitl/proxy.sock`, the CA to `/etc/humanitl/ca.crt` and
//! this binary to `/usr/local/bin/humanitl-shim`, all read-only. The gRPC
//! socket is never mounted.
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

mod bridge;
mod report;
mod seccomp;

use std::env;
use std::ffi::{CString, OsString, c_char, c_int, c_uint};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;

use seccompiler::BpfProgram;

use crate::bridge::{Bound, Bridge};
use crate::report::{Check, Report};
use crate::seccomp::Policy;

/// Exit status for a command line the shim does not understand.
const EXIT_USAGE: i32 = 125;
/// Exit status when a bridge or the filter could not be set up.
const EXIT_SETUP: i32 = 126;
/// Exit status when `execvp` returned.
const EXIT_EXEC: i32 = 127;

const USAGE: &str = "usage: humanitl-shim --proxy-port <port> -- <command> [args...]\n\
                     \x20      humanitl-shim --rules    print the seccomp rule table for this environment\n\
                     environment: HUMANITL_BRIDGES, HUMANITL_SECCOMP_FAMILIES, HUMANITL_SECCOMP_TYPES,\n\
                     HUMANITL_SECCOMP_DENY, HUMANITL_REPORT_FD (see the crate documentation)\n";

/// The shim's own variables; removed from the agent's environment.
const SHIM_VARS: [&str; 5] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
    "HUMANITL_REPORT_FD",
];

/// The child's pid, for the signal handler.
static CHILD: AtomicI32 = AtomicI32::new(0);

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = match parse_cli(env::args_os().skip(1)) {
        Ok(cli) => cli,
        Err(CliError::Help) => {
            let _ = io::stdout().lock().write_all(USAGE.as_bytes());
            return 0;
        }
        Err(CliError::Rules) => return print_rules(),
        Err(err) => {
            say(&format!("humanitl-shim: {err}\n{USAGE}"));
            return EXIT_USAGE;
        }
    };

    // The report comes before everything that can fail, so that every setup
    // failure is a `CHECK <name> fail <evidence>` line and not just a message
    // on a stderr the launcher may not be reading. The launcher turns the
    // failed line into `SANDBOX_014` to `SANDBOX_016`; without it a shim that
    // never got as far as binding would look like a shim that wrote no report
    // at all (`SANDBOX_013`), and the two have different causes.
    let report = match Report::from_env(env::var_os("HUMANITL_REPORT_FD").as_deref()) {
        Ok(report) => report,
        Err(err) => {
            say(&format!("humanitl-shim: warning: no report: {err}\n"));
            Report::none()
        }
    };

    match prepare(&cli, &report) {
        Ok(prepared) => launch(prepared, report, &cli.command),
        Err(err) => {
            report.check(err.check(), false, &err.to_string());
            say(&format!("humanitl-shim: {err}\n"));
            EXIT_SETUP
        }
    }
}

/// `--rules`: the effective rule table for this environment, one row per
/// line on stdout, so an auditor can ask the binary in the sandbox what its
/// filter does instead of trusting a document. Exit 126 when the environment
/// does not describe a valid policy.
fn print_rules() -> i32 {
    match policy_from_env() {
        Ok(policy) => {
            let mut out = io::stdout().lock();
            for rule in policy.rules() {
                let _ = writeln!(out, "{rule}");
            }
            0
        }
        Err(err) => {
            say(&format!("humanitl-shim: {err}\n"));
            EXIT_SETUP
        }
    }
}

/// Everything that exists before the fork.
struct Prepared {
    policy: Policy,
    agent_program: BpfProgram,
    bridge_program: BpfProgram,
    bound: Vec<Bound>,
}

/// Steps 1 and 2: the policy, the programs, the bridges and their evidence.
///
/// Every failure here is reported by the caller as a failed check line; the
/// report is already open when this starts.
fn prepare(cli: &Cli, report: &Report) -> Result<Prepared, SetupError> {
    let policy = policy_from_env()?;
    let agent_program = policy.program().map_err(SetupError::seccomp)?;
    let bridge_program = policy.for_bridge().program().map_err(SetupError::seccomp)?;

    let bridges = match env_utf8("HUMANITL_BRIDGES").map_err(SetupError::bridge)? {
        Some(json) => bridge::parse(&json).map_err(SetupError::bridge)?,
        None => vec![Bridge::default_proxy(cli.proxy_port)],
    };
    bridge::validate(&bridges, cli.proxy_port).map_err(SetupError::bridge)?;

    let mut bound = Vec::with_capacity(bridges.len());
    for bridge in bridges {
        bound.push(bridge::bind(bridge).map_err(SetupError::bridge)?);
    }
    let mut evidence = Vec::with_capacity(bound.len());
    for listener in &bound {
        listener.self_connect().map_err(SetupError::bridge)?;
        evidence.push(format!(
            "{}={}->{}",
            listener.bridge().name,
            listener.local_addr(),
            listener.bridge().socket.display()
        ));
    }
    report.check(Check::BridgeListening, true, &evidence.join(";"));

    match report::interfaces() {
        Ok(names) => {
            let only_lo = names.iter().all(|name| name == "lo");
            report.check(Check::NoInterfaces, only_lo, &names.join(","));
            if !only_lo {
                say(&format!(
                    "humanitl-shim: warning: interfaces besides lo: {}\n",
                    names.join(",")
                ));
            }
        }
        Err(err) => report.check(Check::NoInterfaces, false, &format!("unreadable:{err}")),
    }

    check_single_socket(&bound, report);

    Ok(Prepared {
        policy,
        agent_program,
        bridge_program,
        bound,
    })
}

/// The second guarantee, measured instead of assumed: no Unix socket in the
/// sandbox's filesystem but the ones the bridges serve.
///
/// `bridge_listening` proves the one door is open and answers; it says nothing
/// about a second one. So the shim walks the filesystem before the agent
/// exists ([`report::sockets`]) and names everything it found. A socket that
/// is not a bridge's is a `fail`: it is reported and warned about, not
/// enforced, the same way `no_interfaces` is. The launcher turns the failed
/// line into `SANDBOX_015` and refuses to let the run continue (HUM-011); the
/// shim's job is the evidence, the decision belongs to the host.
fn check_single_socket(bound: &[Bound], report: &Report) {
    let walk = report::sockets();
    let expected: Vec<String> = bound
        .iter()
        .map(|listener| listener.bridge().socket.to_string_lossy().into_owned())
        .collect();
    let unexpected: Vec<&str> = walk
        .sockets
        .iter()
        .filter(|found| !expected.iter().any(|path| path == *found))
        .map(String::as_str)
        .collect();
    let evidence = format!(
        "sockets={};unexpected={};entries={};limit={}",
        list(walk.sockets.iter().map(String::as_str)),
        list(unexpected.iter().copied()),
        walk.entries,
        walk.limit.unwrap_or("none"),
    );
    report.check(Check::SingleSocket, unexpected.is_empty(), &evidence);
    if !unexpected.is_empty() {
        say(&format!(
            "humanitl-shim: warning: unix sockets besides the bridge: {}\n",
            list(unexpected.into_iter())
        ));
    }
}

/// A comma list for one field of the evidence; never empty, never with a space
/// in it, because a reader splits the line on spaces.
fn list<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let joined = values.collect::<Vec<_>>().join(",");
    if joined.is_empty() {
        "none".to_owned()
    } else {
        joined
    }
}

/// Step 3 and 4: fork, and each side's life.
fn launch(prepared: Prepared, report: Report, command: &[OsString]) -> i32 {
    // SAFETY: getpid has no preconditions.
    let parent_pid = unsafe { libc::getpid() };
    // SAFETY: the process is single-threaded here (the bridge threads start
    // after the fork), so the child inherits a consistent image.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let err = io::Error::last_os_error();
        report.check(Check::SeccompApplied, false, &format!("fork-failed:{err}"));
        say(&format!("humanitl-shim: fork failed: {err}\n"));
        return EXIT_SETUP;
    }
    if pid == 0 {
        child(
            parent_pid,
            &prepared.agent_program,
            &prepared.policy,
            &report,
            command,
        )
    }
    parent(pid, &prepared.bridge_program, prepared.bound, report)
}

// ---- the child --------------------------------------------------------------

/// The child: die with the parent, drop what was inherited, install the
/// filter, prove it, become the agent. Never returns.
fn child(
    parent_pid: libc::pid_t,
    program: &BpfProgram,
    policy: &Policy,
    report: &Report,
    command: &[OsString],
) -> ! {
    // SAFETY: prctl with PR_SET_PDEATHSIG only records a signal number.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
    }
    // The parent may have died between fork and prctl; then nobody would
    // ever send the signal.
    // SAFETY: getppid has no preconditions.
    if unsafe { libc::getppid() } != parent_pid {
        exit_now(EXIT_SETUP);
    }

    close_inherited(report.fd());
    reset_signal_mask();

    if let Err(err) = seccomp::apply(program) {
        report.check(Check::SeccompApplied, false, &err.to_string());
        say(&format!("humanitl-shim: seccomp setup failed: {err}\n"));
        exit_now(EXIT_SETUP);
    }

    let mode = seccomp::seccomp_mode();
    let nnp = seccomp::no_new_privs();
    let evidence = format!("Seccomp:{};NoNewPrivs:{}", show(mode), show(nnp));
    report.check(
        Check::SeccompApplied,
        mode == Some(2) && nnp == Some(1),
        &evidence,
    );
    if matches!(mode, Some(m) if m != 2) || matches!(nnp, Some(n) if n != 1) {
        say(&format!(
            "humanitl-shim: seccomp setup failed: the kernel accepted the filter but /proc/self/status shows {evidence}\n"
        ));
        exit_now(EXIT_SETUP);
    }

    match probe_families(policy) {
        Ok(evidence) => report.check(Check::Families, true, &evidence),
        Err(Probe { evidence, fatal }) => {
            report.check(Check::Families, false, &evidence);
            if fatal {
                say(&format!(
                    "humanitl-shim: seccomp setup failed: the filter does not refuse what it must: {evidence}\n"
                ));
                exit_now(EXIT_SETUP);
            }
        }
    }

    for var in SHIM_VARS {
        // SAFETY: the child is single-threaded; nothing reads the environment
        // concurrently.
        unsafe {
            env::remove_var(var);
        }
    }
    // Rust ignores SIGPIPE at start-up and an ignored disposition survives
    // exec; the agent gets the default back. Last, so the report writes above
    // cannot kill the child when the launcher stopped reading.
    // SAFETY: restoring the default disposition of one signal.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let err = exec(command);
    say(&format!(
        "humanitl-shim: exec failed: {}: {err}\n",
        command
            .first()
            .map(|c| c.to_string_lossy())
            .unwrap_or_default()
    ));
    exit_now(EXIT_EXEC)
}

/// A failed `families` probe: the evidence line and whether a refusal that
/// did not happen makes the child refuse to `exec`.
struct Probe {
    evidence: String,
    fatal: bool,
}

/// Proves the filter from inside: one family the policy refuses, one type it
/// refuses, the x32 number of `socket`, `io_uring_setup` from the floor, and
/// the first allowed family and type as the positive control.
///
/// A refusal that does not come back as `EPERM` is fatal: the kernel said the
/// filter is installed, and the filter is ours, so this cannot happen, and if
/// it does the agent must not run. A positive control that fails (a kernel
/// without the family) is reported, not fatal: the agent would only be more
/// locked in, not less.
fn probe_families(policy: &Policy) -> Result<String, Probe> {
    const DENIED_FAMILIES: [(&str, u32); 4] = [
        ("AF_UNIX", libc::AF_UNIX.unsigned_abs()),
        ("AF_NETLINK", libc::AF_NETLINK.unsigned_abs()),
        ("AF_PACKET", libc::AF_PACKET.unsigned_abs()),
        ("AF_VSOCK", libc::AF_VSOCK.unsigned_abs()),
    ];
    const DENIED_TYPES: [(&str, u32); 2] = [
        ("SOCK_DGRAM", libc::SOCK_DGRAM.unsigned_abs()),
        ("SOCK_RAW", libc::SOCK_RAW.unsigned_abs()),
    ];
    let Some(family) = policy.families().first() else {
        return Err(Probe {
            evidence: "no-allowed-family".to_owned(),
            fatal: true,
        });
    };
    let Some(sock_type) = policy.types().first() else {
        return Err(Probe {
            evidence: "no-allowed-type".to_owned(),
            fatal: true,
        });
    };
    let mut evidence = Vec::new();
    let mut fatal = false;

    let mut expect_eperm = |label: String, result: Result<(), i32>| {
        let refused = result == Err(libc::EPERM);
        fatal |= !refused;
        evidence.push(format!("{label}={}", errno_name(result)));
    };
    if let Some((name, number)) = DENIED_FAMILIES
        .iter()
        .find(|(_, number)| !policy.families().iter().any(|f| f.number == *number))
    {
        expect_eperm(
            format!("socket({name},{})", sock_type.name),
            seccomp::probe_socket(u64::from(*number), u64::from(sock_type.number)),
        );
    }
    if let Some((name, number)) = DENIED_TYPES
        .iter()
        .find(|(_, number)| !policy.types().iter().any(|t| t.number == *number))
    {
        expect_eperm(
            format!("socket({},{name})", family.name),
            seccomp::probe_socket(u64::from(family.number), u64::from(*number)),
        );
    }
    expect_eperm(
        "x32:socket".to_owned(),
        seccomp::probe_socket_x32(u64::from(family.number), u64::from(sock_type.number)),
    );
    expect_eperm("io_uring_setup".to_owned(), seccomp::probe_io_uring_setup());

    let control = seccomp::probe_socket(
        u64::from(family.number),
        u64::from(sock_type.number | libc::SOCK_CLOEXEC.unsigned_abs()),
    );
    let control_ok = control.is_ok();
    evidence.push(format!(
        "socket({},{})={}",
        family.name,
        sock_type.name,
        errno_name(control)
    ));

    let evidence = evidence.join(";");
    if fatal || !control_ok {
        Err(Probe { evidence, fatal })
    } else {
        Ok(evidence)
    }
}

fn errno_name(result: Result<(), i32>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(errno) if errno == libc::EPERM => "EPERM".to_owned(),
        Err(errno) => format!("errno{errno}"),
    }
}

fn show(value: Option<u32>) -> String {
    value.map_or_else(|| "?".to_owned(), |v| v.to_string())
}

/// Closes every descriptor from 3 upwards except `keep`, which gets
/// `CLOEXEC` so that `exec` closes it.
///
/// `close_range(2)` (Linux 5.9) through `syscall(2)`, so the binary does not
/// depend on the C library's version; on `ENOSYS` the fallback walks
/// `/proc/self/fd`.
fn close_inherited(keep: Option<c_int>) {
    let ranges: Vec<(c_int, c_int)> = match keep {
        Some(fd) if fd >= 3 => vec![(3, fd - 1), (fd + 1, c_int::MAX)],
        _ => vec![(3, c_int::MAX)],
    };
    for (first, last) in ranges {
        if first > last {
            continue;
        }
        if !close_range(first, last) {
            close_by_listing(first, last, keep);
        }
    }
    if let Some(fd) = keep {
        // SAFETY: F_SETFD with FD_CLOEXEC changes one flag of one descriptor.
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
        }
    }
}

fn close_range(first: c_int, last: c_int) -> bool {
    let first = c_uint::try_from(first).unwrap_or(3);
    let last = c_uint::try_from(last).unwrap_or(c_uint::MAX);
    // SAFETY: close_range takes three integers; closing descriptors the
    // child does not use has no memory-safety implications.
    unsafe { libc::syscall(libc::SYS_close_range, first, last, 0 as c_uint) == 0 }
}

fn close_by_listing(first: c_int, last: c_int, keep: Option<c_int>) {
    let Ok(entries) = fs::read_dir("/proc/self/fd") else {
        return;
    };
    let fds: Vec<c_int> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect();
    for fd in fds {
        if fd >= first && fd <= last && Some(fd) != keep {
            // SAFETY: closing a descriptor the child inherited and does not
            // use; a stale number (the listing's own directory) is EBADF.
            unsafe {
                libc::close(fd);
            }
        }
    }
}

fn reset_signal_mask() {
    // SAFETY: sigset_t is plain data; an all-zero value is what sigemptyset
    // produces, and sigprocmask reads it.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&raw mut set);
        libc::sigprocmask(libc::SIG_SETMASK, &raw const set, ptr::null_mut());
    }
}

/// `execvp(command[0], command)`; returns only on failure.
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
    let mut pointers: Vec<*const c_char> = argv.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(ptr::null());
    // SAFETY: `pointers` is a NUL-terminated array of pointers to
    // NUL-terminated strings that outlive the call; execvp reads them.
    unsafe {
        libc::execvp(file.as_ptr(), pointers.as_ptr());
    }
    io::Error::last_os_error()
}

fn exit_now(code: i32) -> ! {
    // SAFETY: _exit ends the process without unwinding or running the
    // parent's destructors in the child's copy.
    unsafe { libc::_exit(code) }
}

// ---- the parent -------------------------------------------------------------

/// The parent: forward signals, wear the bridge filter, serve the bridges,
/// wait for the child and answer with its status.
fn parent(
    child: libc::pid_t,
    bridge_program: &BpfProgram,
    bound: Vec<Bound>,
    report: Report,
) -> i32 {
    CHILD.store(child, Ordering::SeqCst);
    forward_signals();

    // The parent keeps its copy of the report descriptor until its own filter
    // is on and the bridges are served: a failure here is the last one that
    // can still be told, and telling it after the descriptor was closed would
    // mean losing it. The child holds its own copy until `exec`, so the
    // reader sees EOF once the agent runs either way.
    if let Err(err) = seccomp::apply(bridge_program) {
        report.check(
            Check::SeccompApplied,
            false,
            &format!("bridge-filter:{err}"),
        );
        say(&format!(
            "humanitl-shim: seccomp setup failed: bridge filter: {err}\n"
        ));
        drop(report);
        kill_and_reap(child);
        return EXIT_SETUP;
    }
    for listener in bound {
        let bridge_name = listener.bridge().name.clone();
        let name = format!("bridge-{bridge_name}");
        if let Err(err) = thread::Builder::new()
            .name(name)
            .spawn(move || listener.serve())
        {
            report.check(
                Check::BridgeListening,
                false,
                &format!("{bridge_name}=no-accept-thread:{err}"),
            );
            say(&format!(
                "humanitl-shim: bridge setup failed: cannot start the accept thread: {err}\n"
            ));
            drop(report);
            kill_and_reap(child);
            return EXIT_SETUP;
        }
    }
    drop(report);
    reap(child)
}

extern "C" fn relay(signal: c_int) {
    let child = CHILD.load(Ordering::SeqCst);
    if child > 0 {
        // SAFETY: kill is async-signal-safe and takes two integers.
        unsafe {
            libc::kill(child, signal);
        }
    }
}

fn forward_signals() {
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: sigaction is plain data; the handler is an extern "C" fn
        // that calls only kill(2).
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = relay as extern "C" fn(c_int) as usize;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&raw mut action.sa_mask);
            libc::sigaction(signal, &raw const action, ptr::null_mut());
        }
    }
}

fn kill_and_reap(child: libc::pid_t) {
    // SAFETY: kill takes two integers.
    unsafe {
        libc::kill(child, libc::SIGKILL);
    }
    reap(child);
}

/// Waits for the child and turns its status into ours: the exit code, or
/// 128 + signal.
fn reap(child: libc::pid_t) -> i32 {
    loop {
        let mut status: c_int = 0;
        // SAFETY: waitpid on our own child with a valid status pointer.
        let waited = unsafe { libc::waitpid(child, &raw mut status, 0) };
        if waited == child {
            if libc::WIFEXITED(status) {
                return libc::WEXITSTATUS(status);
            }
            if libc::WIFSIGNALED(status) {
                return 128 + libc::WTERMSIG(status);
            }
            continue;
        }
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return EXIT_SETUP;
        }
    }
}

// ---- command line and small helpers -----------------------------------------

struct Cli {
    proxy_port: u16,
    command: Vec<OsString>,
}

#[derive(Debug)]
enum CliError {
    Help,
    Rules,
    MissingProxyPort,
    BadProxyPort(String),
    MissingValue(&'static str),
    UnknownOption(String),
    Positional(String),
    MissingCommand,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help => write!(f, "help"),
            Self::Rules => write!(f, "rules"),
            Self::MissingProxyPort => write!(f, "--proxy-port is required"),
            Self::BadProxyPort(value) => write!(f, "--proxy-port {value:?} is not a port"),
            Self::MissingValue(flag) => write!(f, "{flag} needs a value"),
            Self::UnknownOption(arg) => write!(f, "unknown option {arg:?}"),
            Self::Positional(arg) => write!(f, "unexpected argument {arg:?} before --"),
            Self::MissingCommand => write!(f, "no command after --"),
        }
    }
}

fn parse_cli(mut args: impl Iterator<Item = OsString>) -> Result<Cli, CliError> {
    let mut proxy_port = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => {
                let command: Vec<OsString> = args.collect();
                if command.is_empty() {
                    return Err(CliError::MissingCommand);
                }
                let proxy_port = proxy_port.ok_or(CliError::MissingProxyPort)?;
                return Ok(Cli {
                    proxy_port,
                    command,
                });
            }
            Some("--help" | "-h") => return Err(CliError::Help),
            Some("--rules") => return Err(CliError::Rules),
            Some("--proxy-port") => {
                let value = args.next().ok_or(CliError::MissingValue("--proxy-port"))?;
                let port = value
                    .to_str()
                    .and_then(|text| text.parse::<u16>().ok())
                    .ok_or_else(|| CliError::BadProxyPort(value.to_string_lossy().into_owned()))?;
                proxy_port = Some(port);
            }
            Some(other) if other.starts_with('-') => {
                return Err(CliError::UnknownOption(other.to_owned()));
            }
            _ => return Err(CliError::Positional(arg.to_string_lossy().into_owned())),
        }
    }
    Err(CliError::MissingCommand)
}

/// A setup failure: which area, and why. Always exit 126.
struct SetupError {
    area: &'static str,
    /// The check whose line reports this failure, because the launcher reads
    /// names, not prose. A policy the environment does not describe means the
    /// agent will never carry a filter (`seccomp_applied`); a bridge list that
    /// cannot be read, bound or reached means nothing listens
    /// (`bridge_listening`).
    check: Check,
    why: String,
}

impl SetupError {
    fn seccomp(err: impl fmt::Display) -> Self {
        Self {
            area: "seccomp",
            check: Check::SeccompApplied,
            why: err.to_string(),
        }
    }

    fn bridge(err: impl fmt::Display) -> Self {
        Self {
            area: "bridge",
            check: Check::BridgeListening,
            why: err.to_string(),
        }
    }

    const fn check(&self) -> Check {
        self.check
    }
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} setup failed: {}", self.area, self.why)
    }
}

/// A variable that is not valid UTF-8.
struct NotUtf8(&'static str);

impl fmt::Display for NotUtf8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not valid UTF-8", self.0)
    }
}

/// The agent's policy from the three `HUMANITL_SECCOMP_*` variables.
fn policy_from_env() -> Result<Policy, SetupError> {
    let families = env_utf8("HUMANITL_SECCOMP_FAMILIES").map_err(SetupError::seccomp)?;
    let types = env_utf8("HUMANITL_SECCOMP_TYPES").map_err(SetupError::seccomp)?;
    let deny = env_utf8("HUMANITL_SECCOMP_DENY").map_err(SetupError::seccomp)?;
    Policy::from_env(families.as_deref(), types.as_deref(), deny.as_deref())
        .map_err(SetupError::seccomp)
}

fn env_utf8(name: &'static str) -> Result<Option<String>, NotUtf8> {
    match env::var_os(name) {
        None => Ok(None),
        Some(value) => value.into_string().map(Some).map_err(|_| NotUtf8(name)),
    }
}

/// One message on stderr. Never panics, whatever stderr is.
fn say(text: &str) {
    let _ = io::stderr().lock().write_all(text.as_bytes());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn command_line_has_exactly_one_shape() {
        let cli = parse_cli(args(&["--proxy-port", "3128", "--", "sh", "-c", "exit 7"])).unwrap();
        assert_eq!(cli.proxy_port, 3128);
        assert_eq!(cli.command, ["sh", "-c", "exit 7"].map(OsString::from));
        // Arguments after -- are the agent's, whatever they look like.
        let cli = parse_cli(args(&["--proxy-port", "0", "--", "--help"])).unwrap();
        assert_eq!(cli.command, [OsString::from("--help")]);
    }

    #[test]
    fn command_line_mistakes_are_usage_errors() {
        let cases: &[(&[&str], &str)] = &[
            (&[], "no command after --"),
            (&["--proxy-port", "3128"], "no command after --"),
            (&["--proxy-port", "3128", "--"], "no command after --"),
            (&["--", "sh"], "--proxy-port is required"),
            (&["--proxy-port"], "--proxy-port needs a value"),
            (&["--proxy-port", "http", "--", "sh"], "not a port"),
            (&["--proxy-port", "70000", "--", "sh"], "not a port"),
            (
                &["--verbose", "--proxy-port", "1", "--", "sh"],
                "unknown option \"--verbose\"",
            ),
            (&["sh", "--", "sh"], "unexpected argument \"sh\" before --"),
        ];
        for (list, needle) in cases {
            let err = match parse_cli(args(list)) {
                Ok(_) => panic!("{list:?} parsed"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains(needle), "{list:?}: {err}");
        }
        assert!(matches!(parse_cli(args(&["--help"])), Err(CliError::Help)));
        assert!(matches!(
            parse_cli(args(&["-h", "--", "sh"])),
            Err(CliError::Help)
        ));
    }

    #[test]
    fn errno_names_are_short_and_eperm_is_spelled_out() {
        assert_eq!(errno_name(Ok(())), "ok");
        assert_eq!(errno_name(Err(libc::EPERM)), "EPERM");
        assert_eq!(
            errno_name(Err(libc::EINVAL)),
            format!("errno{}", libc::EINVAL)
        );
        assert_eq!(show(Some(2)), "2");
        assert_eq!(show(None), "?");
    }
}
