//! Das Ende des Steuerkanals beendet die Sandbox, auch wenn alle Terminals
//! voll sind (HUM-443, Review).
//!
//! Der gebaute Shim läuft als PID 1 eines frischen PID-Namensraums unter
//! bwrap, im Strom-Modus (`--control-stream-fd`). Der Test spielt den Daemon:
//! Er gibt die Größe, wartet, bis der Agent sich meldet, öffnet eine weitere
//! Shell und liest danach nichts mehr. Agent und Shell fluten ihr Terminal
//! und überhören `SIGTERM`. Der Ausgang des Shims läuft voll, er liest die
//! Terminals nicht mehr, und beide Prozesse blockieren im `write`. Dann
//! schließt der Test seine Seite. Die Sandbox muss binnen `KILL_GRACE` plus
//! Rand enden: PID 1 schreibt nie in ein Terminal, sendet das Signal vor jeder
//! Meldung und liest die Terminals nach dem Ende des Stroms leer.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

mod support;

#[allow(dead_code)]
#[path = "../src/contract.rs"]
mod contract;
#[allow(dead_code)]
#[path = "../src/frame.rs"]
mod frame;

use contract::KILL_GRACE;
use frame::{Decoder, Event, Message};

const SHIM: &str = env!("CARGO_BIN_EXE_humanitl-shim");

/// Die fünf Variablen des Vertrags; jeder Start beginnt ohne sie.
const SHIM_VARS: [&str; 5] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
    "HUMANITL_REPORT_FD",
];

/// Der Agent: meldet sich, überhört `SIGTERM` und flutet sein Terminal.
const AGENT: &str = "trap '' TERM; echo up; exec yes agent-flood";

/// Die weitere Shell: ebenso.
const SHELL: [&str; 3] = ["sh", "-c", "trap '' TERM; exec yes shell-flood"];

/// `(daemon, shim)`: ein Strom-Paar, beide mit `CLOEXEC`.
fn stream_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` ist ein gültiges Feld aus zwei Einträgen für socketpair.
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    assert_eq!(rc, 0, "{}", io::Error::last_os_error());
    // SAFETY: socketpair hat beide eben angelegt, niemand sonst besitzt sie.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn send(fd: RawFd, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        // SAFETY: Zeiger und Länge beschreiben `rest`.
        let n = unsafe { libc::send(fd, rest.as_ptr().cast(), rest.len(), libc::MSG_NOSIGNAL) };
        assert!(n > 0, "send: {}", io::Error::last_os_error());
        rest = &rest[usize::try_from(n).unwrap()..];
    }
}

/// Liest Rahmen, bis `done` eine Nachricht annimmt, höchstens `timeout`.
fn read_until(
    fd: RawFd,
    decoder: &mut Decoder,
    timeout: Duration,
    mut done: impl FnMut(&Message) -> bool,
) -> bool {
    let deadline = Instant::now() + timeout;
    let mut buffer = [0u8; 16 * 1024];
    let mut found = false;
    while !found && Instant::now() < deadline {
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: ein pollfd, gültig für den Aufruf.
        if unsafe { libc::poll(&raw mut poll, 1, 100) } <= 0 {
            continue;
        }
        // SAFETY: der Puffer ist für seine Länge gültig.
        let n = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        let Ok(n) = usize::try_from(n) else { break };
        if n == 0 {
            break;
        }
        decoder.push(&buffer[..n], |event| {
            if let Event::Frame(frame) = event
                && let Ok(message) = Message::parse(&frame)
                && done(&message)
            {
                found = true;
            }
        });
    }
    found
}

/// Startet den Shim als PID 1 unter bwrap im Strom-Modus über `shim_fd`.
fn spawn_shim(shim_fd: RawFd) -> std::process::Child {
    let socket =
        std::env::temp_dir().join(format!("humanitl-shim-stream-{}.sock", std::process::id()));
    let mut command = Command::new("bwrap");
    for var in SHIM_VARS {
        command.env_remove(var);
    }
    command.env(
        "HUMANITL_BRIDGES",
        format!(
            r#"[{{"name":"proxy","dir":"in","listen":"127.0.0.1:0","socket":"{}"}}]"#,
            socket.display()
        ),
    );
    command
        .args([
            "--unshare-user",
            "--unshare-pid",
            "--as-pid-1",
            "--die-with-parent",
            "--dev-bind",
            "/",
            "/",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--",
            SHIM,
            "--proxy-port",
            "0",
            "--control-stream-fd",
            &shim_fd.to_string(),
            "--",
            "sh",
            "-c",
            AGENT,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // SAFETY: läuft im Kind vor exec und nimmt nur diesem Deskriptor
    // `CLOEXEC`.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(shim_fd, libc::F_SETFD, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().unwrap()
}

#[test]
fn the_end_of_the_stream_ends_a_sandbox_whose_terminals_are_full() {
    if !support::bwrap_as_pid_1_usable() {
        return;
    }
    let (daemon, shim) = stream_pair();
    // Die erste Nachricht des Daemons: die Größe.
    let mut size = Vec::new();
    frame::encode_size(frame::AGENT, 80, 24, &mut size);
    send(daemon.as_raw_fd(), &size);
    let mut child = spawn_shim(shim.as_raw_fd());
    drop(shim);
    let stderr = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&stderr);
    let mut pipe = child.stderr.take().unwrap();
    thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            sink.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push_str(&String::from_utf8_lossy(&chunk[..n]));
        }
    });
    let shown = || {
        stderr
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    };

    let mut decoder = Decoder::new();
    let mut seen = Vec::new();
    assert!(
        read_until(
            daemon.as_raw_fd(),
            &mut decoder,
            Duration::from_secs(20),
            |m| {
                if let Message::Data { channel: 0, bytes } = m {
                    seen.extend_from_slice(bytes);
                }
                seen.windows(2).any(|w| w == b"up")
            }
        ),
        "the agent did not start ({:?}, terminal {:?}): {}",
        {
            thread::sleep(Duration::from_millis(500));
            child.try_wait()
        },
        String::from_utf8_lossy(&seen),
        shown()
    );
    let mut open = Vec::new();
    let argv: Vec<&[u8]> = SHELL.iter().map(|arg| arg.as_bytes()).collect();
    frame::encode_open(1, 80, 24, &argv, &mut open).unwrap();
    send(daemon.as_raw_fd(), &open);
    assert!(
        read_until(
            daemon.as_raw_fd(),
            &mut decoder,
            Duration::from_secs(20),
            |m| { matches!(m, Message::Started { channel: 1, .. }) }
        ),
        "the shell did not start: {}",
        shown()
    );
    // Nichts mehr lesen: Ausgang des Shims und beide Terminals laufen voll.
    thread::sleep(Duration::from_secs(3));
    drop(daemon);
    let started = Instant::now();
    let deadline = started + KILL_GRACE + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the sandbox outlived its stream by {:?}: {}",
                started.elapsed(),
                shown()
            );
        }
        thread::sleep(Duration::from_millis(20));
    };
    eprintln!("the sandbox ended {:?} after the stream", started.elapsed());
    let code = status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap());
    assert_eq!(code, 128 + libc::SIGKILL, "{}", shown());
    assert!(
        shown().contains("the control stream ended"),
        "PID 1 said why: {}",
        shown()
    );
}
