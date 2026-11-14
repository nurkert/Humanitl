//! Launcher inside the sandbox: bridges first, then the seccomp filter, then
//! the agent (HUM-012, ADR-002, `docs/SECURITY.md` Satz 3).
//!
//! The shim is PID 1 of the sandbox: the launcher starts bwrap with
//! `--as-pid-1`, so no init of bwrap sits above it (HUM-203). That init carried
//! no filter and was dumpable; the shim's parent carries the bridge filter and
//! makes itself non-dumpable before the agent starts. As PID 1 it also reaps
//! every orphan of the sandbox and relays `SIGTERM` and `SIGHUP`, which the
//! kernel would otherwise drop at a namespace init without a handler. It
//! resolves the review finding "seccomp after socat" by process separation:
//! the bridge to the proxy needs `socket(2)` and lives in the parent; the
//! agent is a child that carries the filter before `exec` and can never shed
//! it. Deliberately dependency-free (`libc` and `seccompiler` only, no tokio,
//! no workspace crate) so the security-critical steps stay auditable in seven
//! short files: this one (process model), `seccomp.rs` (the filter as data),
//! `bridge.rs` (the bytes), `report.rs` (the evidence), `refusals.rs` (what
//! the filter refused, counted), `channel.rs` (how its listener reaches
//! the parent), `exec.rs` (further shells the daemon asks for, HUM-299),
//! `supervise.rs` (PID 1's one loop: children, lifeline, door, HUM-430),
//! `envfd.rs` (the agent's environment through a descriptor, HUM-430),
//! `stream.rs` and `frame.rs` (the terminals as frames on one stream
//! socket, HUM-443) and,
//! for the shim as PID 1 of a guest kernel, `guest.rs`, `modules.rs` and
//! `vsock.rs` (HUM-465).
//!
//! # Launcher <-> shim contract (binding for HUM-011, HUM-012, HUM-013)
//!
//! **Command line.** `humanitl-shim --proxy-port <port> [--controlling-terminal]
//! [--control-fd <n> | --control-stream-fd <n>] [--lifeline-fd <n>]
//! [--env-fd <n>] [--stdio-fd <n>] [--pins-fd <n>]
//! [--handover <path> --handover-token <hex>] -- <command> [args...]`,
//! the tail of the bwrap argument vector from
//! `humanitl_sandbox::SandboxProfile::to_bwrap_args`, where the shim itself
//! is `/usr/local/bin/humanitl-shim` inside the sandbox. `--proxy-port` is
//! the port the agent's `HTTP_PROXY` names; the shim refuses to start when
//! no `in` bridge listens there. `--controlling-terminal` (HUM-280) makes the
//! child the leader of a new session with descriptor 0 as its controlling
//! terminal; the launcher passes it only when descriptor 0 is the daemon's
//! own pseudo-terminal, never the user's terminal (see
//! [`take_controlling_terminal`]). `--control-fd` (HUM-299) names an
//! inherited `SOCK_SEQPACKET` socket whose other end only the daemon holds;
//! over it the daemon asks for further shells next to the agent
//! (`exec.rs`). Only PID 1 keeps it: the agent's child closes it before its
//! filter, and a missing or unusable descriptor means no further shells,
//! never a failed start. `--control-stream-fd` (HUM-443) names an inherited
//! `SOCK_STREAM` socket whose other end only the daemon holds: the stream
//! mode. The shim then opens the agent's pseudo-terminal itself, hands its
//! follower to the agent's child alone as descriptors 0, 1, 2 and controlling
//! terminal (PID 1 keeps its own standard streams), waits briefly
//! for the daemon's first frame (the terminal's size), and serves terminal
//! bytes, sizes, further shells, exits, interrupts and heartbeats as frames
//! on that socket (`stream.rs`, `frame.rs`); the end of the socket ends the
//! sandbox like the lifeline. It excludes `--control-fd` and `--stdio-fd`,
//! and a descriptor that is no stream socket refuses the start. In the guest
//! mode the same channel is the host's second connection on the report port
//! (`guest.rs`). `--lifeline-fd` (HUM-430) names the inherited read
//! end of a pipe whose only write end the daemon holds; when it reports
//! `POLLHUP`, PID 1 ends the sandbox as on `SIGTERM` (`supervise.rs`). A
//! number that names no pipe refuses the start. `--env-fd` (HUM-430) names a
//! sealed memfd with the agent's environment as one JSON object; it replaces
//! the environment the shim was started with, except the five variables
//! below, and a broken one refuses the start (`envfd.rs`). Both descriptors
//! are closed before the agent's `exec`, like the report's: the agent never
//! holds either. `--stdio-fd` (HUM-433) names an inherited descriptor that
//! becomes 0, 1 and 2 before anything else of the start; a container engine
//! does not hand its own standard streams to the container, so the daemon's
//! pseudo-terminal travels as an inherited descriptor instead. A number that
//! names no open descriptor refuses the start. `--pins-fd` (HUM-437) names a
//! sealed memfd with the expected mount table and the device and inode of
//! every checked folder; the shim compares both before anything else and
//! refuses the start on any difference (`mounts.rs`). Every Linux backend
//! passes it; without it the launcher sees no `mounts_match` line and holds
//! the second guarantee as unproven. `--handover` and `--handover-token`
//! (HUM-434) come together: before anything else, even before the report,
//! the shim connects to that socket, names the token, and lays the
//! descriptors it gets on 3, 4, 5, ... (`handover.rs`); every number the
//! other flags name refers to that layout. A start whose handover fails
//! never runs the command (exit 126, message on stderr, no report, because
//! the report descriptor is one of those that did not arrive). The daemon
//! removes the handover socket before it sends the descriptors and binds
//! only its directory, so no second socket is left for `single_socket` to
//! find; there is no exemption.
//! `humanitl-shim --rules` prints the seccomp
//! rule table the same environment would produce, one row per line, and
//! starts nothing. `humanitl-shim --guest <path>`, exactly these two
//! arguments, is the guest mode (HUM-465): the shim is PID 1 of a guest
//! kernel and takes everything else from the guest plan at `<path>`
//! (`guest.rs`); as any other process it refuses with exit 125.
//! `humanitl-shim --fds-then-exec <path> <token> -- <command> [args...]` is
//! no sandbox at all (HUM-444): it is the first process of the kernel level's
//! unit on the host, which inherits nothing from the daemon. It takes the
//! descriptors of the handover at `<path>` with `<token>`, lays them on 3,
//! 4, 5, ... (`handover.rs`) and executes the command with them; no bridge,
//! no filter, no report. Any failure is exit 126 with a message on stderr,
//! and the command never runs.
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
//!   before the shim exits 126. While the agent runs, the parent writes
//!   `REFUSALS on|off <why>` and `REFUSED socket ...` lines on the same
//!   descriptor (HUM-138, `refusals.rs`).
//!
//! **Behaviour, in order.**
//!
//! 1. Parse the command line, then open the report. With `--env-fd`, read
//!    the agent's environment next, before anything else, and make it the
//!    process environment; a refused one is `CHECK seccomp_applied fail
//!    environment_setup_failed:_--env-fd...` and exit 126. With
//!    `--lifeline-fd`, take the descriptor; one that is no pipe is `CHECK
//!    seccomp_applied fail lifeline_setup_failed:...` and exit 126. With
//!    `--pins-fd`, write `CHECK mounts_match` and `CHECK mounts_pinned`;
//!    either `fail` is exit 126 before any other line (HUM-437).
//! 2. Read the environment. For every `in` bridge, bind the TCP listener in the parent and
//!    connect to it once (`bridge_listening`); read the interface list
//!    (`no_interfaces`); walk the filesystem for Unix sockets that are not a
//!    bridge's (`single_socket`). The last two are reported, not enforced: the
//!    shim can neither remove an interface nor unmount a socket, bwrap's
//!    `--unshare-net` and the profile's mounts are the guarantee, and the
//!    launcher's isolation check is the enforcement point.
//! 3. Before the fork the shim closes every descriptor it inherited except
//!    0, 1, 2, the report, the exec gate and the bridge listeners, so
//!    neither process holds what a careless launcher leaked without
//!    `CLOEXEC`; then it makes the refusal channel (HUM-138). Fork. The
//!    child dies with the parent (`PR_SET_PDEATHSIG`), closes every inherited
//!    descriptor but 0, 1, 2, the report, the gate and its end of the refusal
//!    channel, sets `PR_SET_NO_NEW_PRIVS`, installs the filter with a
//!    listener and hands the listener to the parent (without a listener the
//!    same filter with `TSYNC`), proves it (`seccomp_applied`, `families`),
//!    tells the parent the agent is next, waits at the gate, and `execvp`s
//!    the command. With `--controlling-terminal` it first calls `setsid` and
//!    takes descriptor 0 as its controlling terminal, before the filter. The
//!    parent, PID 1 of the sandbox, makes itself non-dumpable as its first
//!    step, installs its own, slightly wider filter (the agent's plus
//!    `AF_UNIX`), counts and answers the refused sockets of the agent
//!    (`refusals.rs`), serves the bridges, opens the gate, forwards `SIGTERM`
//!    and `SIGHUP` to the child (`SIGINT` reaches the agent through its
//!    process group or, with a controlling terminal, through the terminal's
//!    foreground group, and is ignored here), and then waits in one `poll`
//!    loop (`supervise.rs`): it reaps every orphan while it waits for its own
//!    child (`waitpid(-1)` on `SIGCHLD`), serves the door for further shells,
//!    and ends the sandbox when the lifeline ends. Last it writes the last
//!    tally.
//! 4. Exit status: 125 usage, 126 seccomp or bridge setup failed (the message
//!    names which), 127 `exec` failed (and the child writes `EXEC fail
//!    errno=<n>` to the report first, see `report.rs`), otherwise the
//!    child's status, or
//!    128 + signal when the child was killed. A bridge with direction `out`
//!    is 126 with "bridge direction out not supported yet".
//!
//! **Host side (HUM-013).** The proxy listens on
//! `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` (`Paths::proxy_socket()`,
//! directory 0700, socket 0600). The launcher bind-mounts only that socket
//! file to `/run/humanitl/proxy.sock`, the CA to `/etc/humanitl/ca.crt` and
//! this binary to `/usr/local/bin/humanitl-shim`, all read-only. The gRPC
//! socket is never mounted.

mod channel;
mod contract;
mod envfd;
mod exec;
mod guest;
mod handover;
mod modules;
mod mounts;
mod refusals;
mod seccomp;
mod stream;
mod supervise;
pub(crate) mod vsock;

use std::env;
use std::ffi::{CString, OsString, c_char, c_int, c_uint};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;

use seccompiler::BpfProgram;

use crate::bridge::{self, Bound, Bridge, Upstream};
use crate::linux::channel::Received;
use crate::linux::refusals::Tally;
use crate::linux::seccomp::{Gate, Policy};
use crate::report::{self, Check, Report};

/// Exit status for a command line the shim does not understand.
const EXIT_USAGE: i32 = 125;
/// Exit status when a bridge or the filter could not be set up.
const EXIT_SETUP: i32 = 126;
/// Exit status when `execvp` returned.
const EXIT_EXEC: i32 = 127;

const USAGE: &str = "usage: humanitl-shim --proxy-port <port> [--controlling-terminal] [--control-fd <n>]\n\
                     \x20      [--control-stream-fd <n>] [--lifeline-fd <n>] [--env-fd <n>] [--stdio-fd <n>] [--pins-fd <n>]\n\
                     \x20      [--handover <path> --handover-token <hex>]\n\
                     \x20      -- <command> [args...]\n\
                     \x20      humanitl-shim --rules    print the seccomp rule table for this environment\n\
                     \x20      humanitl-shim --guest <plan>    PID 1 of a guest kernel (HUM-465)\n\
                     \x20      humanitl-shim --fds-then-exec <path> <token> -- <command> [args...]\n\
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

/// The entry point on Linux, called from `main.rs`.
pub(crate) fn main() {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    if let [flag, path] = args.as_slice()
        && flag == "--guest"
    {
        // SAFETY: getpid has no preconditions.
        if unsafe { libc::getpid() } != 1 {
            say("humanitl-shim: --guest is for PID 1 of a guest kernel only\n");
            std::process::exit(EXIT_USAGE);
        }
        let code = run_guest(Path::new(path));
        // PID 1 of a kernel must not return; see `guest.rs`.
        guest::power_off();
        std::process::exit(code);
    }
    if let [flag, path, token, separator, command @ ..] = args.as_slice()
        && flag == "--fds-then-exec"
        && separator == "--"
        && !command.is_empty()
    {
        std::process::exit(fds_then_exec(Path::new(path), token, command));
    }
    std::process::exit(run(args));
}

/// `--fds-then-exec` (HUM-444): the handover's descriptors on 3, 4, 5, ...,
/// then the command. Only an exit status comes back, and only on failure.
fn fds_then_exec(path: &Path, token: &OsString, command: &[OsString]) -> i32 {
    let Some(token) = token.to_str() else {
        say("humanitl-shim: --fds-then-exec: the token is not text\n");
        return EXIT_SETUP;
    };
    if let Err(why) = handover::receive(path, token).and_then(handover::place) {
        say(&format!("humanitl-shim: --fds-then-exec: {why}\n"));
        return EXIT_SETUP;
    }
    let err = exec(command);
    say(&format!(
        "humanitl-shim: --fds-then-exec: cannot execute {}: {err}\n",
        command.first().map_or_else(String::new, |program| program
            .to_string_lossy()
            .into_owned())
    ));
    EXIT_EXEC
}

/// The guest mode (HUM-465): the steps of `guest.rs`, the mount table, then
/// the same launch as under bwrap, with the door over vsock.
fn run_guest(path: &Path) -> i32 {
    let Some(guest::Booted {
        report,
        plan,
        control,
    }) = guest::boot(path)
    else {
        return EXIT_SETUP;
    };
    if !mounts::verify_pins(&plan.pins, &report) {
        guest::linger(&report);
        return EXIT_SETUP;
    }
    // The terminals over the host's second connection (HUM-443): in a guest
    // the stream mode is the only one.
    let stream = match stream::nonblocking(control.as_raw_fd())
        .map_err(|err| format!("flags:{err}").replace(' ', "_"))
        .and_then(|()| stream_session(control))
    {
        Ok(session) => session,
        Err(why) => {
            let code = refuse_setup(&report, &SetupError::control_stream(&why));
            guest::linger(&report);
            return code;
        }
    };
    let cli = Cli {
        proxy_port: plan.proxy_port,
        command: plan.command,
        vsock_door: Some(plan.door_port),
        socket_walk: plan.socket_walk,
        controlling_terminal: true,
        terminal_fd: stream.follower_fd(),
        ..Cli::default()
    };
    // A second descriptor on the report's connection: PID 1 drops the report
    // when the agent is gone, and the last lines must still reach the host
    // before the guest powers off. On the same connection the host's verdict
    // arrives (HUM-444); without the descriptor there is no gate, so nothing
    // starts.
    let (linger, start) = match guest_start(guest::keep(&report)) {
        Ok(pair) => pair,
        Err(why) => {
            report.check(Check::SeccompApplied, false, &why);
            say(&format!("humanitl-shim: guest: {why}\n"));
            guest::linger(&report);
            return EXIT_SETUP;
        }
    };
    let code = match prepare(&cli, &report) {
        Ok(prepared) => launch(
            prepared,
            report,
            &cli,
            Inherited {
                control: None,
                lifeline: None,
                stream: Some(stream),
                start,
            },
        ),
        Err(err) => refuse_setup(&report, &err),
    };
    guest::linger_fd(linger.as_raw_fd());
    code
}

/// How the agent may start (HUM-444).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentStart {
    /// As soon as PID 1 is ready: bwrap and the container engine.
    Direct,
    /// Only after the host's `ADMIT` on this descriptor, a copy of the
    /// report connection that PID 1 also keeps past the report: a guest.
    Admitted(RawFd),
}

impl AgentStart {
    /// The descriptor PID 1 keeps besides the report, if any.
    const fn descriptor(self) -> Option<RawFd> {
        match self {
            Self::Direct => None,
            Self::Admitted(fd) => Some(fd),
        }
    }
}

/// The guest mode's start: it needs the copy of the report connection, on
/// which the host's verdict arrives. Without it the agent must never start,
/// so there is no fallback to [`AgentStart::Direct`].
///
/// # Errors
///
/// The evidence for the report when the copy could not be made.
fn guest_start(copy: Option<OwnedFd>) -> Result<(OwnedFd, AgentStart), String> {
    let copy = copy.ok_or_else(|| "admission:no-descriptor-for-the-hosts-verdict".to_owned())?;
    let start = AgentStart::Admitted(copy.as_raw_fd());
    Ok((copy, start))
}

fn run(args: Vec<OsString>) -> i32 {
    let mut cli = match parse_cli(args.into_iter()) {
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

    // The descriptors from the daemon's handover socket (HUM-434), before
    // anything else: the report, the environment, the lifeline and the mount
    // table are among them. Without them nothing may start; the message goes
    // to stderr, the only stream there is.
    if let (Some(path), Some(token)) = (cli.handover.as_deref(), cli.handover_token.as_deref()) {
        let taken = handover::receive(path, token).and_then(handover::place);
        if let Err(why) = taken {
            say(&format!(
                "humanitl-shim: the descriptors did not arrive through {}: {why}\n",
                path.display()
            ));
            return EXIT_SETUP;
        }
    }

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

    // The terminal of the session as the standard streams (HUM-433), before
    // anything writes to them: a message of the shim from here on goes to the
    // daemon's pseudo-terminal, not to the engine's empty streams.
    if let Some(fd) = cli.stdio_fd
        && let Err(why) = take_stdio(fd)
    {
        return refuse_setup(&report, &SetupError::stdio(&why));
    }

    // The agent's environment from the daemon (HUM-430), before anything
    // else reads the environment: the policy, the bridges and the shells all
    // see the result. Only the report comes earlier, so that a refused
    // environment is a failed check line.
    if let Some(fd) = cli.env_fd
        && let Err(err) = environment_from_fd(fd)
    {
        return refuse_setup(&report, &err);
    }
    // The daemon's lifeline (HUM-430). Unlike the door below, a lifeline that
    // cannot be watched refuses the start: the sandbox would outlive the
    // daemon, and nobody would see it.
    let lifeline = match cli.lifeline_fd.map(supervise::take_lifeline).transpose() {
        Ok(lifeline) => lifeline,
        Err(why) => return refuse_setup(&report, &SetupError::lifeline(&why)),
    };

    // The mount table and the checked folders (HUM-437), after the
    // environment and before anything else: the bridges, the socket walk
    // and the agent all stand on these mounts. Its lines come first in the
    // report, so a launcher that has read the five checks has read these.
    if let Some(fd) = cli.pins_fd
        && !mounts::verify(fd, &report)
    {
        return EXIT_SETUP;
    }

    // The stream mode (HUM-443): the daemon's socket, the agent's terminal
    // as the standard streams, the first size. Refused like the lifeline:
    // without it the agent has no terminal and nobody notices the daemon's
    // end.
    let stream = match cli.control_stream_fd {
        None => None,
        Some(fd) => match stream::take_control(fd).and_then(stream_session) {
            Ok(session) => {
                cli.controlling_terminal = true;
                cli.terminal_fd = session.follower_fd();
                Some(session)
            }
            Err(why) => return refuse_setup(&report, &SetupError::control_stream(&why)),
        },
    };

    // The daemon's door for further shells (HUM-299). Taken before the
    // descriptors are closed in `launch`, and never a reason to refuse the
    // start: without it there are simply no further shells.
    let control = cli.control_fd.and_then(take_control);

    match prepare(&cli, &report) {
        Ok(prepared) => launch(
            prepared,
            report,
            &cli,
            Inherited {
                control,
                lifeline,
                stream,
                start: AgentStart::Direct,
            },
        ),
        Err(err) => refuse_setup(&report, &err),
    }
}

/// The size the agent's terminal has until the daemon names one.
const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// How long the shim waits for the daemon's first frame, the size of the
/// agent's terminal, before the agent starts without it.
const SIZE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The stream mode's session over `control` (HUM-443): opens the agent's
/// pseudo-terminal and waits for the daemon's first size. The follower goes
/// to the agent's child alone ([`Cli::terminal_fd`]); PID 1 keeps the
/// standard streams it was started with and never writes to a terminal, so
/// that a full terminal cannot hold up the lifeline. Still single-threaded.
fn stream_session(control: OwnedFd) -> Result<stream::Session, String> {
    let pty = stream::open_pty(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
        .map_err(|err| format!("pty:{err}").replace(' ', "_"))?;
    stream::nonblocking(pty.master.as_raw_fd())
        .map_err(|err| format!("pty-flags:{err}").replace(' ', "_"))?;
    let mut session = stream::Session::new(control, pty, &SHIM_VARS);
    session.await_size(SIZE_WAIT);
    Ok(session)
}

/// A setup failure before the fork: the failed check line, the message, 126.
fn refuse_setup(report: &Report, err: &SetupError) -> i32 {
    report.check(err.check(), false, &err.to_string());
    say(&format!("humanitl-shim: {err}\n"));
    EXIT_SETUP
}

/// `--stdio-fd`: the descriptor becomes 0, 1 and 2 and is closed under its
/// own number. Still single-threaded, so no other thread writes to a standard
/// stream in between.
fn take_stdio(fd: RawFd) -> Result<(), String> {
    // SAFETY: fcntl(F_GETFD) only asks whether the number is open.
    if fd < 3 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(format!("not-open:{fd}"));
    }
    for target in 0..3 {
        // SAFETY: dup2 between two descriptor numbers; the target is a
        // standard stream this process owns.
        if unsafe { libc::dup2(fd, target) } < 0 {
            return Err(format!(
                "dup2:{fd}->{target}:{}",
                io::Error::last_os_error()
            ));
        }
    }
    // SAFETY: the launcher handed this number to the shim alone; its three
    // copies stay.
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

/// `--env-fd`: reads, checks and closes the descriptor, then makes its pairs
/// the process environment, the shim's own five variables kept.
fn environment_from_fd(fd: RawFd) -> Result<(), SetupError> {
    let pairs = envfd::read(fd, &SHIM_VARS).map_err(SetupError::environment)?;
    // SAFETY: still single-threaded: no bridge, refusal or shell thread
    // exists before `launch`.
    unsafe { envfd::apply(&pairs, &SHIM_VARS) };
    Ok(())
}

/// The daemon's descriptors that only PID 1 keeps; the agent holds neither.
struct Inherited {
    /// `--control-fd` (HUM-299).
    control: Option<OwnedFd>,
    /// `--lifeline-fd` (HUM-430).
    lifeline: Option<OwnedFd>,
    /// The stream mode (HUM-443): the socket and the agent's terminal.
    stream: Option<stream::Session>,
    /// How the agent starts; in the guest mode with a copy of the report's
    /// descriptor that PID 1 keeps open past the report (HUM-465) and reads
    /// the host's verdict from (HUM-444); never the agent's.
    start: AgentStart,
}

/// `--rules`: the effective rule table for this environment, one row per
/// line on stdout, so an auditor can ask the binary in the sandbox what its
/// filter does instead of trusting a document. Exit 126 when the environment
/// does not describe a valid policy.
fn print_rules() -> i32 {
    match policy_from_env() {
        Ok(policy) => {
            let mut out = io::stdout().lock();
            for rule in policy.rules(Gate::Reported) {
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
    /// The agent's filter, refused sockets reported ([`Gate::Reported`]).
    agent_program: BpfProgram,
    /// The same filter answering `EPERM` itself, for a kernel that refuses
    /// the agent a listener.
    agent_fallback: BpfProgram,
    bridge_program: BpfProgram,
    bound: Vec<Bound>,
}

/// Steps 1 and 2: the policy, the programs, the bridges and their evidence.
///
/// Every failure here is reported by the caller as a failed check line; the
/// report is already open when this starts.
fn prepare(cli: &Cli, report: &Report) -> Result<Prepared, SetupError> {
    let policy = policy_from_env()?;
    // In a guest kernel the agent never gets `AF_UNIX` (HUM-465); the guest
    // plan refused a profile that asks for it already.
    let policy = if cli.vsock_door.is_some() {
        policy.for_guest_agent()
    } else {
        policy
    };
    let agent_program = policy
        .program(Gate::Reported)
        .map_err(SetupError::seccomp)?;
    let agent_fallback = policy.program(Gate::Silent).map_err(SetupError::seccomp)?;
    // In a guest kernel the bridge reaches the door over vsock, and its
    // filter allows that family instead of `AF_UNIX` (HUM-465).
    let bridge_policy = if cli.vsock_door.is_some() {
        policy.for_bridge_over_vsock()
    } else {
        policy.for_bridge()
    };
    let bridge_program = bridge_policy
        .program(Gate::Silent)
        .map_err(SetupError::seccomp)?;

    let bridges = match env_utf8("HUMANITL_BRIDGES").map_err(SetupError::bridge)? {
        Some(json) => bridge::parse(&json).map_err(SetupError::bridge)?,
        None => vec![Bridge::default_proxy(cli.proxy_port)],
    };
    bridge::validate(&bridges, cli.proxy_port).map_err(SetupError::bridge)?;

    let mut bound = Vec::with_capacity(bridges.len());
    for bridge in bridges {
        let listener = bridge::bind(bridge).map_err(SetupError::bridge)?;
        bound.push(match cli.vsock_door {
            Some(port) => listener.over_vsock(port),
            None => listener,
        });
    }
    let mut evidence = Vec::with_capacity(bound.len());
    for listener in &bound {
        listener.self_connect().map_err(SetupError::bridge)?;
        evidence.push(format!(
            "{}={}->{}",
            listener.bridge().name,
            listener.local_addr(),
            listener.upstream()
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

    check_single_socket(&bound, &cli.socket_walk, report);

    Ok(Prepared {
        policy,
        agent_program,
        agent_fallback,
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
///
/// The handover socket (HUM-434) gets no exemption: the daemon removes it
/// before it sends the descriptors, and its directory is bound, not the
/// socket, so after the handover it is simply gone.
///
/// In a guest kernel the walk starts at the plan's roots instead of `/`
/// (`socket_walk`, HUM-444, `report::sockets_under`).
fn check_single_socket(bound: &[Bound], roots: &[PathBuf], report: &Report) {
    let walk = if roots.is_empty() {
        report::sockets()
    } else {
        report::sockets_under(roots)
    };
    let expected: Vec<String> = bound
        .iter()
        .filter_map(|listener| match listener.upstream() {
            Upstream::Unix(path) => Some(path.to_string_lossy().into_owned()),
            Upstream::Vsock { .. } => None,
        })
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

/// Takes the descriptor of `--control-fd` when it is an open socket; gives
/// it `CLOEXEC` so that no `exec` carries it on.
fn take_control(fd: RawFd) -> Option<OwnedFd> {
    // SAFETY: plain data; fstat fills it.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat on a descriptor number with a valid pointer.
    let rc = unsafe { libc::fstat(fd, &raw mut stat) };
    if fd < 3 || rc != 0 || stat.st_mode & libc::S_IFMT != libc::S_IFSOCK {
        say("humanitl-shim: warning: --control-fd is not an open socket; no further shells\n");
        return None;
    }
    // SAFETY: F_SETFD with FD_CLOEXEC changes one flag of one descriptor.
    unsafe {
        libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    // SAFETY: the launcher handed this descriptor to the shim alone, and
    // nothing else in this process owns it.
    Some(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Step 3 and 4: fork, and each side's life.
fn launch(prepared: Prepared, report: Report, cli: &Cli, inherited: Inherited) -> i32 {
    let Inherited {
        control,
        lifeline,
        stream,
        start,
    } = inherited;
    // SAFETY: getpid has no preconditions.
    let parent_pid = unsafe { libc::getpid() };
    // The gate the child waits at before `exec` (HUM-137): it opens when the
    // parent has closed its write end. The parent closes it once nothing it
    // holds is reachable for the agent any more: its own filter is on and it
    // is no longer dumpable (HUM-138), so neither the report writer nor the
    // listener of the refusals can be opened through `/proc/<pid>/fd` or
    // `pidfd_getfd(2)`, and the agent cannot forge `EXEC fail`.
    let Some((gate_read, gate_write)) = gate() else {
        let err = io::Error::last_os_error();
        report.check(Check::SeccompApplied, false, &format!("gate-failed:{err}"));
        say(&format!(
            "humanitl-shim: cannot create the exec gate: {err}\n"
        ));
        return EXIT_SETUP;
    };
    // Whatever the launcher left open without `CLOEXEC` goes before the
    // fork, in both processes at once: the agent could otherwise reopen it
    // through `/proc/<ppid>/fd`, and a stray line in a pipe of the launcher
    // is a line in someone else's protocol. Kept are the report, the gate
    // and the bridge listeners; the bridges open their Unix sockets per
    // connection. Single-threaded here, so nothing opens a descriptor while
    // the range closes.
    let mut keep: Vec<c_int> = prepared.bound.iter().map(Bound::listener_fd).collect();
    keep.extend([
        report.fd().unwrap_or(-1),
        gate_read.as_raw_fd(),
        gate_write.as_raw_fd(),
        control.as_ref().map_or(-1, AsRawFd::as_raw_fd),
        lifeline.as_ref().map_or(-1, AsRawFd::as_raw_fd),
        start.descriptor().unwrap_or(-1),
    ]);
    keep.extend(stream.iter().flat_map(stream::Session::fds));
    close_inherited(&keep);
    // The pair over which the child hands its listener to the parent
    // (HUM-138), made after the close above and right before the fork, so
    // that close cannot take it. Without it the agent still gets its filter,
    // only one that answers `EPERM` itself and tells nobody.
    let pair = match channel::channel() {
        Ok(pair) => Some(pair),
        Err(err) => {
            say(&format!(
                "humanitl-shim: warning: refused sockets will not be reported: {err}\n"
            ));
            None
        }
    };
    let (parent_end, child_end) = pair.map_or((None, None), |(p, c)| (Some(p), Some(c)));
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
        // Explicitly, not only through `close_inherited`: should that fail
        // to close it, the child would hold a write end of its own gate and
        // wait there for ever, and the parent with it in `supervise::wait`.
        drop(gate_write);
        drop(parent_end);
        // The daemon's door for further shells stays with PID 1 (HUM-299),
        // and so does its lifeline (HUM-430): both closed here explicitly,
        // and `close_inherited` in `child` does not keep them either. The
        // agent never holds them.
        drop(control);
        drop(lifeline);
        // The stream's socket and the masters as well (HUM-443): not
        // dropped here, because the follower must stay open until `child`
        // has made it descriptors 0, 1 and 2; `close_inherited` there
        // closes all of them, and `child` never returns.
        child(
            parent_pid,
            &AgentFilter {
                reported: &prepared.agent_program,
                fallback: &prepared.agent_fallback,
                control: child_end,
            },
            &prepared.policy,
            &report,
            gate_read,
            cli,
        )
    }
    drop(gate_read);
    drop(child_end);
    let shells = control
        .map(|control| exec::Shells::new(control, prepared.agent_fallback.clone(), &SHIM_VARS));
    let stream = stream.map(|mut session| {
        session.release_follower();
        session.start(pid, prepared.agent_fallback);
        session
    });
    let tally = Tally::new(
        prepared
            .policy
            .families()
            .iter()
            .map(|family| family.number)
            .collect(),
    );
    parent(
        pid,
        &prepared.bridge_program,
        prepared.bound,
        report,
        gate_write,
        &Refusals {
            control: parent_end,
            tally: Arc::new(tally),
        },
        Waited {
            shells,
            lifeline,
            stream,
            // Only a guest waits for the host's admission (HUM-444); bwrap
            // and the container engine start the agent as before.
            start,
        },
    )
}

/// What PID 1's loop serves besides its child (`supervise.rs`).
struct Waited {
    /// The further shells, when the daemon gave a door.
    shells: Option<exec::Shells>,
    /// The daemon's lifeline, when it gave one.
    lifeline: Option<OwnedFd>,
    /// The stream mode, when the daemon chose it (HUM-443).
    stream: Option<stream::Session>,
    /// How the agent may start: in a guest only after the host's verdict
    /// on the report connection (HUM-444).
    start: AgentStart,
}

/// The agent's two filters and the child's end of the channel to the parent.
struct AgentFilter<'a> {
    reported: &'a BpfProgram,
    fallback: &'a BpfProgram,
    control: Option<OwnedFd>,
}

impl AgentFilter<'_> {
    /// Installs the reported gate and hands its listener to the parent; falls
    /// back to the silent gate when the kernel refuses a listener, and says
    /// so over the channel (HUM-138).
    ///
    /// The fallback is not a weaker filter: both answer `EPERM` to the same
    /// calls, and both refuse the agent a listener of its own. What it loses
    /// is the report, and the parent turns the reason into a `REFUSALS off`
    /// line the host shows.
    fn install(&self) -> Result<(), seccomp::Error> {
        let Some(control) = self.control.as_ref().map(AsRawFd::as_raw_fd) else {
            return seccomp::apply(self.fallback);
        };
        let listener = refusals::kernel_fits()
            .map_err(|errno| seccomp::Error::Listener(io::Error::from_raw_os_error(errno)))
            .and_then(|()| seccomp::apply_reporting(self.reported));
        match listener {
            Ok(listener) => {
                // If the hand-over fails, the listener dies with this scope
                // and the kernel answers the parked calls with `ENOSYS`; the
                // `families` probe then refuses to start the agent.
                channel::send_listener(control, listener.as_raw_fd());
                Ok(())
            }
            Err(err) => {
                let errno = match &err {
                    seccomp::Error::Listener(io) | seccomp::Error::NoNewPrivs(io) => {
                        io.raw_os_error().unwrap_or(0)
                    }
                    _ => 0,
                };
                seccomp::apply(self.fallback)?;
                channel::send_unavailable(control, errno);
                Ok(())
            }
        }
    }

    /// Tells the parent that the probes are over; the next refusal is the
    /// agent's.
    fn before_exec(&self) {
        if let Some(control) = self.control.as_ref() {
            channel::send_exec_marker(control.as_raw_fd());
        }
    }

    fn control_fd(&self) -> Option<RawFd> {
        self.control.as_ref().map(AsRawFd::as_raw_fd)
    }
}

/// The parent's half of the report on refused sockets.
struct Refusals {
    control: Option<OwnedFd>,
    tally: Arc<Tally>,
}

/// A pipe with `CLOEXEC` on both ends, as `(read, write)`.
fn gate() -> Option<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as c_int; 2];
    // SAFETY: `fds` is a valid two-element array for pipe2.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return None;
    }
    // SAFETY: pipe2 just created both descriptors, and nothing else owns them.
    Some(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Blocks until every write end of the gate is closed.
fn wait_at_gate(gate: &OwnedFd) {
    let mut byte = [0u8; 1];
    loop {
        // SAFETY: the buffer is one byte long and outlives the call.
        let n = unsafe { libc::read(gate.as_raw_fd(), byte.as_mut_ptr().cast(), 1) };
        if n == 0 {
            return;
        }
        if n < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return;
        }
    }
}

// ---- the child --------------------------------------------------------------

/// The child: die with the parent, drop what was inherited, install the
/// filter, prove it, become the agent. Never returns.
fn child(
    parent_pid: libc::pid_t,
    filter: &AgentFilter<'_>,
    policy: &Policy,
    report: &Report,
    gate: OwnedFd,
    cli: &Cli,
) -> ! {
    let command = cli.command.as_slice();
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

    // The stream mode's terminal (HUM-443) becomes this child's standard
    // streams, and only this child's: PID 1 keeps its own.
    if let Some(fd) = cli.terminal_fd {
        for target in 0..3 {
            // SAFETY: dup2 between two descriptor numbers this process owns.
            if unsafe { libc::dup2(fd, target) } < 0 {
                exit_now(EXIT_SETUP);
            }
        }
    }
    let keep: Vec<c_int> = [report.fd(), Some(gate.as_raw_fd()), filter.control_fd()]
        .into_iter()
        .flatten()
        .collect();
    close_inherited(&keep);
    reset_signal_mask();
    // Before the filter, so the filter never has to allow it.
    if cli.controlling_terminal {
        take_controlling_terminal();
    }

    if let Err(err) = filter.install() {
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

    match probe_families(policy, cli.vsock_door.is_some()) {
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
    // From here on every refused socket is the agent's. The channel itself
    // carries `CLOEXEC` and closes with the `exec`; the agent never holds it,
    // and never the listener, which went to the parent in `install`.
    filter.before_exec();
    // The agent starts with the defaults. Last, and not a line earlier: the
    // report writes above must survive a launcher that stopped reading, and
    // they do that because Rust ignores SIGPIPE at start-up. Resetting the
    // dispositions before them would kill the child on the first write into
    // a closed pipe.
    reset_signal_dispositions();

    // Not a line earlier than here, and not skipped: see `launch`.
    wait_at_gate(&gate);
    drop(gate);
    let err = exec(command);
    // Out of band first: the terminal line below is for the person, the
    // report line is the launcher's proof that the agent never ran (HUM-137).
    report.exec_failed(&err);
    say(&format!(
        "humanitl-shim: exec failed: {}: {err}\n",
        command
            .first()
            .map(|c| c.to_string_lossy())
            .unwrap_or_default()
    ));
    exit_now(EXIT_EXEC)
}

/// Makes the child the leader of a new session with descriptor 0 as its
/// controlling terminal (HUM-280).
///
/// Job control needs it: without a controlling terminal, `Ctrl+C` in an
/// interactive shell is a byte and not `SIGINT` to the foreground program,
/// and the shell reports "no job control". bwrap's `--new-session` stays in
/// every command line; it detaches the whole sandbox from whatever terminal
/// started bwrap. This step only attaches the session's first process to the
/// terminal descriptor 0 already is.
///
/// Why this cannot reach the user's terminal:
///
/// - The launcher passes `--controlling-terminal` only when descriptor 0 is
///   the follower side of a pseudo-terminal the daemon opened for this
///   session (`StdioMode::Pty`), never with inherited descriptors.
/// - `TIOCSCTTY` is called with argument 0: it never steals a terminal that
///   is another session's controlling terminal. A user's terminal always is
///   (the shell that started the command holds it), so the call would fail
///   with `EPERM`; stealing needs `CAP_SYS_ADMIN` in the initial user
///   namespace, which the sandbox never has.
///
/// A failure is reported on stderr and is not fatal: the agent then runs as
/// before this issue, without job control, and every guarantee holds.
fn take_controlling_terminal() {
    // SAFETY: isatty only inspects descriptor 0.
    if unsafe { libc::isatty(0) } != 1 {
        say("humanitl-shim: warning: no controlling terminal: descriptor 0 is not a terminal\n");
        return;
    }
    // SAFETY: setsid has no preconditions. The forked child is never a
    // process group leader, so the call cannot fail with EPERM.
    if unsafe { libc::setsid() } < 0 {
        let err = io::Error::last_os_error();
        say(&format!(
            "humanitl-shim: warning: no controlling terminal: setsid failed: {err}\n"
        ));
        return;
    }
    // SAFETY: TIOCSCTTY takes an int; 0 means "do not steal".
    if unsafe { libc::ioctl(0, libc::TIOCSCTTY, 0 as c_int) } != 0 {
        let err = io::Error::last_os_error();
        say(&format!(
            "humanitl-shim: warning: no controlling terminal: TIOCSCTTY failed: {err}\n"
        ));
    }
}

/// A failed `families` probe: the evidence line and whether a refusal that
/// did not happen makes the child refuse to `exec`.
struct Probe {
    evidence: String,
    fatal: bool,
}

/// Proves the filter from inside: one family the policy refuses, one type it
/// refuses, the x32 number of `socket`, `io_uring_setup` from the floor, and
/// the first allowed family and type as the positive control. In a guest
/// kernel (`guest`, HUM-465) also `AF_VSOCK`, the family of the door, which
/// PID 1 may use and the agent never.
///
/// A refusal that does not come back as `EPERM` is fatal: the kernel said the
/// filter is installed, and the filter is ours, so this cannot happen, and if
/// it does the agent must not run. A positive control that fails (a kernel
/// without the family) is reported, not fatal: the agent would only be more
/// locked in, not less.
fn probe_families(policy: &Policy, guest: bool) -> Result<String, Probe> {
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
    if guest {
        expect_eperm(
            format!("socket(AF_VSOCK,{})", sock_type.name),
            seccomp::probe_socket(
                u64::from(seccomp::VSOCK.number),
                u64::from(sock_type.number),
            ),
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

/// Closes every descriptor from 3 upwards except those in `keep`, which get
/// `CLOEXEC` so that `exec` closes them.
///
/// `close_range(2)` (Linux 5.9) through `syscall(2)`, so the binary does not
/// depend on the C library's version; on `ENOSYS` the fallback walks
/// `/proc/self/fd`.
fn close_inherited(keep: &[c_int]) {
    let mut kept: Vec<c_int> = keep.iter().copied().filter(|fd| *fd >= 3).collect();
    kept.sort_unstable();
    kept.dedup();
    let mut first = 3;
    for &fd in kept.iter().chain(std::iter::once(&c_int::MAX)) {
        let last = if fd == c_int::MAX { fd } else { fd - 1 };
        if first <= last && !close_range(first, last) {
            close_by_listing(first, last, &kept);
        }
        first = fd.saturating_add(1);
    }
    for &fd in &kept {
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

fn close_by_listing(first: c_int, last: c_int, keep: &[c_int]) {
    let Ok(entries) = fs::read_dir("/proc/self/fd") else {
        return;
    };
    let fds: Vec<c_int> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect();
    for fd in fds {
        if fd >= first && fd <= last && !keep.contains(&fd) {
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

/// Gives the agent the default disposition for every signal it can handle.
///
/// `execve` resets a handler to the default, but it keeps `SIG_IGN`. An
/// ignored signal therefore travels from whoever started Humanitl all the way
/// into the sandbox: a service under systemd, a background job of a shell
/// without job control, anything started through `nohup`. The agent then
/// cannot install a handler for it at all -- POSIX says a non-interactive
/// shell may not trap a signal that was ignored on entry -- and
/// `SandboxHandle::interrupt` asks a process that cannot hear the question.
/// Measured on 2026-09-06: with `SIGINT` ignored in the launcher, `sh -c
/// 'trap "exit 42" INT; sleep 60'` in the sandbox stayed alive through the
/// whole grace period and was killed instead of ending with its own code.
///
/// The sandbox init decides this, not the launcher. `SIGKILL` and `SIGSTOP`
/// are left out because they cannot be set; the real-time signals are
/// included because a queued one would be ignored just as silently.
///
/// Called immediately before `execvp` and nowhere earlier. `SIGPIPE` is one of
/// the dispositions it resets, and Rust's ignored `SIGPIPE` is what keeps the
/// report writes of this child alive when the launcher has stopped reading.
fn reset_signal_dispositions() {
    let last = libc::SIGRTMAX();
    for signal in 1..=last {
        if signal == libc::SIGKILL || signal == libc::SIGSTOP {
            continue;
        }
        // SAFETY: signal(2) with SIG_DFL takes two integers and has no
        // preconditions; an unknown number answers with SIG_ERR and is
        // ignored here on purpose.
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
        }
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
    gate: OwnedFd,
    refusals: &Refusals,
    waited: Waited,
) -> i32 {
    let Waited {
        shells,
        lifeline,
        mut stream,
        start,
    } = waited;
    // The parent holds the listener and the report descriptor while the agent
    // runs. Neither may be reachable from the agent through
    // `/proc/<pid>/fd` or `pidfd_getfd(2)`: with the listener it could answer
    // its own refused calls, with the report it could write lines of its own.
    // A process that is not dumpable passes `ptrace_may_access` for nobody
    // without `CAP_SYS_PTRACE`, which the sandbox has dropped. After the fork,
    // so the child and its `exec` keep the default, and first of all, well
    // before `drop(gate)` (HUM-138). Since HUM-203 this process is PID 1 of
    // the sandbox, and the same flag is what keeps `/proc/1/mem` and
    // `pidfd_getfd(2)` on PID 1 closed to the agent: an init that stays
    // dumpable would be a process without the agent's filter that the agent
    // could write into.
    let report = Arc::new(report);
    if let Err(err) = set_undumpable() {
        return refuse_to_open(child, report, &format!("set:{err}"));
    }
    CHILD.store(child, Ordering::SeqCst);
    forward_signals();

    // The parent keeps its copy of the report descriptor while the agent runs:
    // the refusals of the agent are written on it, the last tally after
    // `supervise::wait` (HUM-138). The child waits at the gate until this process is
    // non-dumpable (above) and wears its own filter (below). Non-dumpable
    // replaces HUM-137's "no report writer exists when the agent runs": a
    // writer still exists, but the agent can no longer reach it through
    // `/proc/<pid>/fd` or `pidfd_getfd(2)`, so it still cannot forge a line.
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
    watch_refusals(&report, refusals);
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
    // Only now may the agent start: this process is non-dumpable, wears its
    // filter and serves the listener (HUM-137, HUM-138). The gate never opens
    // on a premise that is not proven: the flag is read back right here.
    if let Err(err) = check_undumpable() {
        return refuse_to_open(child, report, &format!("get:{err}"));
    }
    // In a guest the host decides as well (HUM-444): it reads the `CHECK`
    // lines, all written by now, checks the machine from outside, and only
    // its `ADMIT` opens the gate. Without it the agent never runs.
    if let AgentStart::Admitted(fd) = start
        && let Err(why) = guest::await_admission(fd, guest::ADMISSION_WAIT)
    {
        say(&format!(
            "humanitl-shim: guest: the host did not admit the agent: {why}\n"
        ));
        drop(report);
        kill_and_reap(child);
        return EXIT_SETUP;
    }
    // The door for further shells is served from the loop below, under the
    // same premises as the gate: this process wears its filter and is not
    // dumpable, and every shell forked from here inherits both (HUM-299).
    drop(gate);
    let code = supervise::wait(child, shells.as_ref(), lifeline, stream.as_mut());
    if let Some(shells) = &shells {
        // The namespace ends with this process anyway; the shells go first
        // and on purpose, not as a side effect.
        shells.kill_all();
    }
    if let Some(stream) = &stream {
        stream.kill_shells();
    }
    // The last word on the refusals: everything that changed since the last
    // periodic flush, once the agent can make no further attempt.
    refusals::flush(&report, &refusals.tally);
    drop(report);
    code
}

/// Makes this process non-dumpable (HUM-138).
fn set_undumpable() -> Result<(), io::Error> {
    // SAFETY: prctl with PR_SET_DUMPABLE only sets a flag of this process.
    let rc = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Reads the flag back: the gate opens only when this process is proven
/// non-dumpable, not merely asked to be (HUM-138).
fn check_undumpable() -> Result<(), String> {
    // SAFETY: prctl with PR_GET_DUMPABLE only reads a flag of this process.
    let rc = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
    match rc {
        0 => Ok(()),
        -1 => Err(io::Error::last_os_error().to_string()),
        other => Err(format!("still-{other}")),
    }
}

/// Ends the run before the gate opens, because a premise of the gate does not
/// hold: the failed check goes on the report, the child dies unstarted.
fn refuse_to_open(child: libc::pid_t, report: Arc<Report>, why: &str) -> i32 {
    report.check(Check::SeccompApplied, false, &format!("dumpable:{why}"));
    say(&format!(
        "humanitl-shim: setup failed: the parent is not non-dumpable: {why}\n"
    ));
    drop(report);
    kill_and_reap(child);
    EXIT_SETUP
}

/// Takes the listener from the child and starts counting (HUM-138).
///
/// Writes `REFUSALS on` when the listener is served, `REFUSALS off <why>`
/// when it is not, and nothing when the child ended before it said either:
/// then the child has written its own failed check line.
fn watch_refusals(report: &Arc<Report>, refusals: &Refusals) {
    let Some(control) = refusals.control.as_ref() else {
        report.line("REFUSALS off no-channel");
        return;
    };
    let listener = match channel::receive(control) {
        Received::Listener(listener) => listener,
        Received::Unavailable(errno) => {
            report.line(&format!("REFUSALS off errno{errno}"));
            say(&format!(
                "humanitl-shim: warning: refused sockets will not be reported: {}\n",
                io::Error::from_raw_os_error(errno)
            ));
            return;
        }
        Received::Closed => return,
    };
    let Ok(control) = control.try_clone() else {
        report.line("REFUSALS off no-channel");
        return;
    };
    let tally = Arc::clone(&refusals.tally);
    let served = thread::Builder::new()
        .name("refusals".to_owned())
        .spawn(move || refusals::serve(&listener, &control, &tally));
    if let Err(err) = served {
        // The listener went down with the closure; the kernel now answers the
        // parked calls with `ENOSYS`, and the child's `families` probe stops
        // the agent from starting. Nor could the agent claim a listener of
        // its own: both gates refuse the flag (`seccomp.rs`).
        report.line(&format!("REFUSALS off thread:{err}"));
        return;
    }
    report.line("REFUSALS on");
    let flusher_report = Arc::clone(report);
    let tally = Arc::clone(&refusals.tally);
    let _ = thread::Builder::new()
        .name("refusals-flush".to_owned())
        .spawn(move || refusals::flush_every(&flusher_report, &tally));
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
    // Tragend, seit der Shim PID 1 der Sandbox ist (HUM-203): Der Kernel stellt
    // einem Namensraum-Init kein Signal zu, für das es keinen Handler hat,
    // außer SIGKILL und SIGSTOP von außerhalb. Ohne diese Weiterleitung
    // verpuffte ein SIGTERM an den Shim, und der Agent hörte es nie.
    // SIGINT fehlt hier mit Absicht: Ein Terminal schickt es an die ganze
    // Vordergrund-Prozessgruppe, und der Launcher (SandboxHandle::interrupt)
    // an die Prozessgruppe der Sandbox; der Agent hat es dann schon. Reichte
    // der Shim es zusaetzlich weiter, bekaeme der Agent zwei SIGINT wenige
    // Millisekunden hintereinander, und viele Agenten deuten das zweite als
    // "sofort abbrechen". SIGTERM und SIGHUP kommen meist an den Shim allein
    // (systemd, kill <pid>) und werden deshalb weitergereicht.
    // Der Shim selbst darf an SIGINT nicht sterben: Es kommt an die ganze
    // Gruppe, und ein toter Shim hiesse ein totes bwrap mit Exit 130, bevor
    // der Agent seinen Handler ausfuehren konnte. Nur der Elternprozess
    // ignoriert es; das Kind hat die Vorgabe geerbt, weil dies nach dem Fork
    // geschieht.
    // SAFETY: SIG_IGN fuer ein Signal des eigenen Prozesses zu setzen hat
    // keine Vorbedingungen.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
    }
    for signal in [libc::SIGTERM, libc::SIGHUP] {
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
    supervise::reap_blocking(child);
}

// ---- command line and small helpers -----------------------------------------

#[derive(Default)]
struct Cli {
    proxy_port: u16,
    /// `--controlling-terminal` (HUM-280).
    controlling_terminal: bool,
    /// `--control-fd` (HUM-299).
    control_fd: Option<RawFd>,
    /// `--control-stream-fd` (HUM-443).
    control_stream_fd: Option<RawFd>,
    /// `--lifeline-fd` (HUM-430).
    lifeline_fd: Option<RawFd>,
    /// `--env-fd` (HUM-430).
    env_fd: Option<RawFd>,
    /// `--stdio-fd` (HUM-433).
    stdio_fd: Option<RawFd>,
    /// `--pins-fd` (HUM-437).
    pins_fd: Option<RawFd>,
    /// `--handover` (HUM-434).
    handover: Option<PathBuf>,
    /// `--handover-token` (HUM-434).
    handover_token: Option<String>,
    command: Vec<OsString>,
    /// The guest mode's door (HUM-465): the bridge forwards to this vsock
    /// port of the host. Never set from the command line.
    vsock_door: Option<u32>,
    /// The stream mode's follower of the agent's terminal (HUM-443): the
    /// agent's child makes it descriptors 0, 1 and 2 before anything else.
    /// Never set from the command line.
    terminal_fd: Option<RawFd>,
    /// Where the socket walk starts in a guest (HUM-444); empty means `/`.
    /// Never set from the command line.
    socket_walk: Vec<PathBuf>,
}

#[derive(Debug)]
enum CliError {
    Help,
    Rules,
    MissingProxyPort,
    BadProxyPort(String),
    /// A descriptor flag whose value is no number above 2.
    BadFd(&'static str, String),
    MissingValue(&'static str),
    UnknownOption(String),
    /// A flag that the stream mode excludes (HUM-443).
    Conflict(&'static str),
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
            Self::BadFd(flag, value) => {
                write!(f, "{flag} {value:?} is not a descriptor above 2")
            }
            Self::MissingValue(flag) => write!(f, "{flag} needs a value"),
            Self::UnknownOption(arg) => write!(f, "unknown option {arg:?}"),
            Self::Conflict(flag) => write!(f, "--control-stream-fd excludes {flag}"),
            Self::Positional(arg) => write!(f, "unexpected argument {arg:?} before --"),
            Self::MissingCommand => write!(f, "no command after --"),
        }
    }
}

fn parse_cli(mut args: impl Iterator<Item = OsString>) -> Result<Cli, CliError> {
    let mut proxy_port = None;
    let mut controlling_terminal = false;
    let mut control_fd = None;
    let mut control_stream_fd = None;
    let mut lifeline_fd = None;
    let mut env_fd = None;
    let mut stdio_fd = None;
    let mut pins_fd = None;
    let mut handover = None;
    let mut handover_token = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => {
                let command: Vec<OsString> = args.collect();
                if command.is_empty() {
                    return Err(CliError::MissingCommand);
                }
                let proxy_port = proxy_port.ok_or(CliError::MissingProxyPort)?;
                // Path and token only together: one without the other is a
                // launcher bug, never a start without the handover.
                match (&handover, &handover_token) {
                    (Some(_), None) => return Err(CliError::MissingValue("--handover-token")),
                    (None, Some(_)) => return Err(CliError::MissingValue("--handover")),
                    _ => {}
                }
                // The stream mode brings its own terminal and its own door
                // (HUM-443): a second of either is a launcher bug.
                if control_stream_fd.is_some() {
                    if control_fd.is_some() {
                        return Err(CliError::Conflict("--control-fd"));
                    }
                    if stdio_fd.is_some() {
                        return Err(CliError::Conflict("--stdio-fd"));
                    }
                }
                return Ok(Cli {
                    proxy_port,
                    controlling_terminal,
                    control_fd,
                    control_stream_fd,
                    lifeline_fd,
                    env_fd,
                    stdio_fd,
                    pins_fd,
                    handover,
                    handover_token,
                    command,
                    vsock_door: None,
                    terminal_fd: None,
                    socket_walk: Vec::new(),
                });
            }
            Some("--help" | "-h") => return Err(CliError::Help),
            Some("--rules") => return Err(CliError::Rules),
            Some("--controlling-terminal") => controlling_terminal = true,
            Some("--control-fd") => control_fd = Some(fd_value(&mut args, "--control-fd")?),
            Some("--control-stream-fd") => {
                control_stream_fd = Some(fd_value(&mut args, "--control-stream-fd")?);
            }
            Some("--lifeline-fd") => lifeline_fd = Some(fd_value(&mut args, "--lifeline-fd")?),
            Some("--env-fd") => env_fd = Some(fd_value(&mut args, "--env-fd")?),
            Some("--stdio-fd") => stdio_fd = Some(fd_value(&mut args, "--stdio-fd")?),
            Some("--pins-fd") => pins_fd = Some(fd_value(&mut args, "--pins-fd")?),
            Some("--handover") => {
                let value = args.next().ok_or(CliError::MissingValue("--handover"))?;
                handover = Some(PathBuf::from(value));
            }
            Some("--handover-token") => {
                let value = args
                    .next()
                    .ok_or(CliError::MissingValue("--handover-token"))?;
                handover_token = Some(value.to_string_lossy().into_owned());
            }
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

/// The value of a descriptor flag: a number above 2.
fn fd_value(
    args: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
) -> Result<RawFd, CliError> {
    let value = args.next().ok_or(CliError::MissingValue(flag))?;
    value
        .to_str()
        .and_then(|text| text.parse::<RawFd>().ok())
        .filter(|fd| *fd > 2)
        .ok_or_else(|| CliError::BadFd(flag, value.to_string_lossy().into_owned()))
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

    /// The environment behind `--env-fd` was refused (HUM-430). Reported as
    /// `seccomp_applied`, like every failure of the shim's own process
    /// model: without its environment the shim cannot say what it starts.
    fn environment(err: impl fmt::Display) -> Self {
        Self {
            area: "environment",
            check: Check::SeccompApplied,
            why: err.to_string(),
        }
    }

    /// The descriptor of `--stdio-fd` is not open (HUM-433). Reported as
    /// `seccomp_applied`, like every failure of the shim's own process model.
    fn stdio(why: &str) -> Self {
        Self {
            area: "stdio",
            check: Check::SeccompApplied,
            why: why.to_owned(),
        }
    }

    /// The stream mode could not be set up (HUM-443): no stream socket, or
    /// no pseudo-terminal. Without it there is neither a terminal nor a
    /// lifeline, so nothing starts.
    fn control_stream(why: &str) -> Self {
        Self {
            area: "control-stream",
            check: Check::SeccompApplied,
            why: why.to_owned(),
        }
    }

    /// The descriptor of `--lifeline-fd` is no pipe (HUM-430).
    fn lifeline(why: &str) -> Self {
        Self {
            area: "lifeline",
            check: Check::SeccompApplied,
            why: why.to_owned(),
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
pub(crate) fn say(text: &str) {
    let _ = io::stderr().lock().write_all(text.as_bytes());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use super::*;

    /// HUM-444: a guest without the copy of the report connection never
    /// starts its agent; with it the start waits for the host's verdict on
    /// exactly that descriptor.
    #[test]
    fn a_guest_without_the_admission_descriptor_refuses() {
        let err = guest_start(None).unwrap_err();
        assert!(err.starts_with("admission:"), "{err}");
        assert!(!err.contains(' '), "{err}");
        let (read, _write) = std::io::pipe().unwrap();
        let copy = OwnedFd::from(read);
        let raw = copy.as_raw_fd();
        let (kept, start) = guest_start(Some(copy)).unwrap();
        assert_eq!(start, AgentStart::Admitted(raw));
        assert_eq!(kept.as_raw_fd(), raw);
        assert_eq!(start.descriptor(), Some(raw));
        assert_eq!(AgentStart::Direct.descriptor(), None);
    }

    fn args(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// The stream mode (HUM-443) brings its own terminal and door; a second
    /// door or an inherited terminal next to it is a launcher bug.
    #[test]
    fn the_stream_mode_excludes_the_other_door_and_the_inherited_terminal() {
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--control-stream-fd",
            "9",
            "--",
            "sh",
        ]))
        .unwrap();
        assert_eq!(cli.control_stream_fd, Some(9));
        assert_eq!(cli.control_fd, None);
        for other in ["--control-fd", "--stdio-fd"] {
            let err = parse_cli(args(&[
                "--proxy-port",
                "3128",
                "--control-stream-fd",
                "9",
                other,
                "8",
                "--",
                "sh",
            ]))
            .err()
            .unwrap();
            assert!(
                matches!(err, CliError::Conflict(flag) if flag == other),
                "{err}"
            );
        }
        assert!(matches!(
            parse_cli(args(&[
                "--proxy-port",
                "3128",
                "--control-stream-fd",
                "2",
                "--",
                "sh"
            ])),
            Err(CliError::BadFd("--control-stream-fd", _))
        ));
    }

    #[test]
    fn command_line_has_exactly_one_shape() {
        let cli = parse_cli(args(&["--proxy-port", "3128", "--", "sh", "-c", "exit 7"])).unwrap();
        assert_eq!(cli.proxy_port, 3128);
        assert!(!cli.controlling_terminal);
        assert_eq!(cli.command, ["sh", "-c", "exit 7"].map(OsString::from));
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--controlling-terminal",
            "--",
            "bash",
        ]))
        .unwrap();
        assert!(cli.controlling_terminal);
        assert_eq!(cli.control_fd, None);
        assert_eq!(cli.command, [OsString::from("bash")]);
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--control-fd",
            "9",
            "--",
            "sh",
        ]))
        .unwrap();
        assert_eq!(cli.control_fd, Some(9));
        assert_eq!((cli.lifeline_fd, cli.env_fd), (None, None));
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--lifeline-fd",
            "7",
            "--env-fd",
            "8",
            "--",
            "sh",
        ]))
        .unwrap();
        assert_eq!((cli.lifeline_fd, cli.env_fd), (Some(7), Some(8)));
        assert_eq!(cli.stdio_fd, None);
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--stdio-fd",
            "9",
            "--",
            "sh",
        ]))
        .unwrap();
        assert_eq!(cli.stdio_fd, Some(9));
        assert_eq!(cli.pins_fd, None);
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--pins-fd",
            "6",
            "--",
            "sh",
        ]))
        .unwrap();
        assert_eq!(cli.pins_fd, Some(6));
        assert!(matches!(
            parse_cli(args(&[
                "--proxy-port",
                "3128",
                "--stdio-fd",
                "1",
                "--",
                "sh"
            ])),
            Err(CliError::BadFd("--stdio-fd", _))
        ));
        // Arguments after -- are the agent's, whatever they look like.
        let cli = parse_cli(args(&["--proxy-port", "0", "--", "--help"])).unwrap();
        assert_eq!(cli.command, [OsString::from("--help")]);
        assert_eq!(cli.handover, None);
    }

    /// The handover (HUM-434): path and token, only together.
    #[test]
    fn the_handover_takes_a_path_and_a_token_only_together() {
        let cli = parse_cli(args(&[
            "--proxy-port",
            "3128",
            "--handover",
            "/run/humanitl-handover/fds",
            "--handover-token",
            "0123456789abcdef0123456789abcdef",
            "--lifeline-fd",
            "4",
            "--",
            "sh",
        ]))
        .unwrap();
        assert_eq!(
            cli.handover.as_deref(),
            Some(Path::new("/run/humanitl-handover/fds"))
        );
        assert_eq!(
            cli.handover_token.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        for half in [
            &["--handover", "/run/humanitl-handover/fds"][..],
            &["--handover-token", "0123456789abcdef0123456789abcdef"][..],
        ] {
            let mut line = vec!["--proxy-port", "3128"];
            line.extend_from_slice(half);
            line.extend_from_slice(&["--", "sh"]);
            assert!(
                matches!(parse_cli(args(&line)), Err(CliError::MissingValue(_))),
                "{line:?}"
            );
        }
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
            (
                &["--proxy-port", "1", "--control-fd", "2", "--", "sh"],
                "--control-fd \"2\" is not a descriptor above 2",
            ),
            (
                &["--proxy-port", "1", "--control-fd", "x", "--", "sh"],
                "is not a descriptor above 2",
            ),
            (
                &["--proxy-port", "1", "--control-fd"],
                "--control-fd needs a value",
            ),
            (
                &["--proxy-port", "1", "--lifeline-fd", "1", "--", "sh"],
                "--lifeline-fd \"1\" is not a descriptor above 2",
            ),
            (
                &["--proxy-port", "1", "--env-fd", "-4", "--", "sh"],
                "--env-fd \"-4\" is not a descriptor above 2",
            ),
            (&["--proxy-port", "1", "--env-fd"], "--env-fd needs a value"),
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
