//! The shim as a process: exit codes, the filter on the agent, the bridge
//! through the binary, the report, signals.
//!
//! These tests run the built binary on the host, not in a sandbox, so the
//! `no_interfaces` check reports `fail` here and the seccomp filter is the
//! only isolation in play. Everything the sandbox adds is ESC-1 and ESC-2's
//! business (`tests/escape/`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const SHIM: &str = env!("CARGO_BIN_EXE_humanitl-shim");

/// How many syscalls the floor of `seccomp::FLOOR` refuses. The binary is a
/// separate crate here, so the number is written out; the unit test
/// `the_hardening_syscalls_are_on_the_floor` guards the list itself.
const FLOOR_LEN: usize = 17;

/// The five variables of the contract; every test starts without them.
const SHIM_VARS: [&str; 5] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
    "HUMANITL_REPORT_FD",
];

fn shim() -> Command {
    let mut command = Command::new(SHIM);
    for var in SHIM_VARS {
        command.env_remove(var);
    }
    command.stdin(Stdio::null());
    command
}

/// `humanitl-shim --proxy-port 0 -- <command...>` with a bridge on an
/// ephemeral port to `socket`, so nothing in the suite needs a fixed port.
fn shim_with_bridge(socket: &Path, command: &[&str]) -> Command {
    let mut cmd = shim();
    cmd.env(
        "HUMANITL_BRIDGES",
        format!(
            r#"[{{"name":"proxy","dir":"in","listen":"127.0.0.1:0","socket":"{}"}}]"#,
            socket.display()
        ),
    );
    cmd.args(["--proxy-port", "0", "--"]).args(command);
    cmd
}

fn code(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap())
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn socket_path(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("humanitl-shim-it-{}-{tag}-{n}.sock", std::process::id());
    let path = std::env::temp_dir().join(&name);
    if path.as_os_str().len() < 100 {
        path
    } else {
        PathBuf::from("/tmp").join(name)
    }
}

/// A socket path directly under `/tmp`, whatever `TMPDIR` says: the shim's
/// socket walk looks three levels deep, and a temporary directory further down
/// would sit outside it.
fn shallow_socket_path(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    PathBuf::from("/tmp").join(format!(
        "humanitl-shim-it-{}-{tag}-{n}.sock",
        std::process::id()
    ))
}

/// Serves `path` with an upper-casing echo: a reply in capitals can only
/// have come through the Unix socket.
fn shouting_server(path: &Path) {
    let _ = fs::remove_file(path);
    let listener = UnixListener::bind(path).unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let reply: Vec<u8> = buf[..n].iter().map(u8::to_ascii_uppercase).collect();
                    if stream.write_all(&reply).is_err() {
                        break;
                    }
                }
            });
        }
    });
}

/// A pipe whose write end the shim inherits, named in `HUMANITL_REPORT_FD`.
///
/// Returns `(read end, write end)`. The caller drops the write end after
/// spawning, so that the read end sees EOF once the shim's copies are gone.
fn report_pipe(command: &mut Command) -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid two-element array for pipe2.
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    let (read_end, write_end) = fds.into();
    command.env("HUMANITL_REPORT_FD", write_end.to_string());
    // SAFETY: runs in the forked child before exec and only clears CLOEXEC
    // on the write end, so exactly that descriptor survives into the shim.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(write_end, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    // SAFETY: both descriptors were just created and belong to this test.
    unsafe {
        (
            OwnedFd::from_raw_fd(read_end),
            OwnedFd::from_raw_fd(write_end),
        )
    }
}

fn read_report(read_end: OwnedFd) -> Vec<String> {
    let mut text = String::new();
    fs::File::from(read_end).read_to_string(&mut text).unwrap();
    text.lines().map(str::to_owned).collect()
}

/// Reads report lines until the one for `check` arrives.
fn wait_for_line(reader: &mut BufReader<fs::File>, check: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut line = String::new();
    loop {
        line.clear();
        assert!(Instant::now() < deadline, "no {check} line within 10 s");
        let n = reader.read_line(&mut line).unwrap();
        assert!(n > 0, "the report ended before {check}");
        if line.starts_with(&format!("CHECK {check} ")) {
            return line.trim_end().to_owned();
        }
    }
}

/// Polls `/proc/<pid>/status` until it contains `needle`; the parent installs
/// its filter right after the fork, concurrently with the child's report.
fn wait_for_status(pid: u32, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
        if status.contains(needle) {
            return status;
        }
        assert!(Instant::now() < deadline, "{needle:?} not in:\n{status}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_with_timeout(child: &mut Child) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the shim did not exit within 10 s"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

// ---- exit codes ---------------------------------------------------------------

#[test]
fn usage_errors_exit_125() {
    for args in [
        &[][..],
        &["--proxy-port", "3128"],
        &["--", "true"],
        &["--proxy-port", "x", "--", "true"],
        &["--bogus", "--proxy-port", "1", "--", "true"],
    ] {
        let output = shim().args(args).output().unwrap();
        assert_eq!(code(output.status), 125, "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("usage: humanitl-shim"),
            "{args:?}: {stderr}"
        );
    }
}

#[test]
fn help_exits_0() {
    let output = shim().arg("--help").output().unwrap();
    assert_eq!(code(output.status), 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("--proxy-port"));
}

#[test]
fn rules_prints_the_effective_table() {
    let output = shim().arg("--rules").output().unwrap();
    assert_eq!(code(output.status), 0);
    let table = String::from_utf8_lossy(&output.stdout);
    let rows: Vec<&str> = table.lines().collect();
    assert_eq!(rows.len(), 4 + FLOOR_LEN, "{table}");
    assert!(
        rows[2].contains("family (arg0) not in {AF_INET, AF_INET6}"),
        "{table}"
    );
    assert!(
        rows[3].contains("type (arg1 & 0xff) not in {SOCK_STREAM}"),
        "{table}"
    );
    assert!(rows[4].starts_with("ptrace"), "{table}");
    // The hardening list of the specification is part of the floor, so it is
    // in the table without any profile asking for it.
    for name in [
        "kexec_load",
        "kexec_file_load",
        "init_module",
        "finit_module",
        "delete_module",
        "bpf",
        "perf_event_open",
        "userfaultfd",
    ] {
        assert!(
            rows.iter().any(|row| row.starts_with(name)),
            "{name} is missing from the default table:\n{table}"
        );
    }

    // A name the floor already carries is not repeated; a new one is appended.
    let output = shim()
        .env("HUMANITL_SECCOMP_DENY", "bpf,mount")
        .env("HUMANITL_SECCOMP_FAMILIES", "AF_UNIX,AF_INET")
        .arg("--rules")
        .output()
        .unwrap();
    let table = String::from_utf8_lossy(&output.stdout);
    assert_eq!(table.lines().count(), 4 + FLOOR_LEN + 1, "{table}");
    assert!(table.contains("{AF_UNIX, AF_INET}"), "{table}");
    assert!(
        table.lines().last().unwrap().starts_with("mount"),
        "{table}"
    );

    let output = shim()
        .env("HUMANITL_SECCOMP_DENY", "frobnicate")
        .arg("--rules")
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
}

#[test]
fn exit_code_passthrough() {
    let socket = socket_path("exit");
    let seven = shim_with_bridge(&socket, &["sh", "-c", "exit 7"])
        .status()
        .unwrap();
    assert_eq!(code(seven), 7);
    let killed = shim_with_bridge(&socket, &["sh", "-c", "kill -9 $$"])
        .status()
        .unwrap();
    assert_eq!(code(killed), 137, "a signalled agent is 128 + signal");
    let zero = shim_with_bridge(&socket, &["true"]).status().unwrap();
    assert_eq!(code(zero), 0);
}

#[test]
fn exec_failure_exits_127() {
    let socket = socket_path("exec");
    let output = shim_with_bridge(&socket, &["/nonexistent/humanitl-no-such-agent"])
        .output()
        .unwrap();
    assert_eq!(code(output.status), 127);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exec failed"), "{stderr}");
    assert!(
        stderr.contains("/nonexistent/humanitl-no-such-agent"),
        "{stderr}"
    );
}

#[test]
fn bridge_direction_out_exits_126() {
    let output = shim()
        .env(
            "HUMANITL_BRIDGES",
            r#"[{"name":"cdp","dir":"out","listen":"127.0.0.1:9222","socket":"/run/humanitl/cdp.sock"}]"#,
        )
        .args(["--proxy-port", "9222", "--", "true"])
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bridge setup failed"), "{stderr}");
    assert!(
        stderr.contains("bridge direction out not supported yet"),
        "{stderr}"
    );
}

#[test]
fn proxy_port_without_a_bridge_exits_126() {
    let output = shim()
        .env(
            "HUMANITL_BRIDGES",
            r#"[{"name":"proxy","dir":"in","listen":"127.0.0.1:0","socket":"/nowhere.sock"}]"#,
        )
        .args(["--proxy-port", "3128", "--", "true"])
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bridge setup failed"), "{stderr}");
    assert!(stderr.contains("--proxy-port 3128"), "{stderr}");
}

#[test]
fn unknown_seccomp_names_exit_126() {
    let socket = socket_path("names");
    let output = shim_with_bridge(&socket, &["true"])
        .env("HUMANITL_SECCOMP_DENY", "ptrace,frobnicate")
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("seccomp setup failed"), "{stderr}");
    assert!(stderr.contains("frobnicate"), "{stderr}");

    let output = shim_with_bridge(&socket, &["true"])
        .env("HUMANITL_SECCOMP_FAMILIES", "AF_INET,AF_BOGUS")
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
    assert!(String::from_utf8_lossy(&output.stderr).contains("AF_BOGUS"));

    let output = shim_with_bridge(&socket, &["true"])
        .env("HUMANITL_BRIDGES", "[{")
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
    assert!(String::from_utf8_lossy(&output.stderr).contains("HUMANITL_BRIDGES"));
}

// ---- the agent ------------------------------------------------------------------

#[test]
fn agent_carries_the_filter_and_no_new_privs() {
    let socket = socket_path("status");
    let output = shim_with_bridge(&socket, &["cat", "/proc/self/status"])
        .output()
        .unwrap();
    assert_eq!(code(output.status), 0);
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(status.contains("Seccomp:\t2\n"), "{status}");
    assert!(status.contains("NoNewPrivs:\t1\n"), "{status}");
}

#[test]
fn agent_is_refused_the_forbidden_families() {
    let socket = socket_path("refuse");
    // sh cannot call socket(2); python3 can, when it exists.
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipped: no python3");
        return;
    }
    let script = "import socket, errno\n\
                  def probe(*a):\n\
                  \ttry:\n\
                  \t\tsocket.socket(*a).close(); return 'ok'\n\
                  \texcept OSError as e:\n\
                  \t\treturn errno.errorcode.get(e.errno, str(e.errno))\n\
                  print(probe(socket.AF_UNIX, socket.SOCK_STREAM))\n\
                  print(probe(socket.AF_INET, socket.SOCK_DGRAM))\n\
                  print(probe(socket.AF_INET, socket.SOCK_STREAM))\n\
                  l, r = socket.socketpair(); l.close(); r.close(); print('pair')\n";
    let output = shim_with_bridge(&socket, &["python3", "-c", script])
        .output()
        .unwrap();
    assert_eq!(
        code(output.status),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "EPERM\nEPERM\nok\npair\n"
    );
}

#[test]
fn agent_inherits_no_descriptors_and_none_of_the_shim_variables() {
    let socket = socket_path("fds");
    let mut command = shim_with_bridge(&socket, &["sh", "-c", "ls /proc/self/fd; env"]);
    command.env("HUMANITL_TEST", "1");
    let (read_end, write_end) = report_pipe(&mut command);
    let output = command.output().unwrap();
    drop(write_end);
    assert_eq!(
        code(output.status),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    // `ls` opens the directory itself, that is descriptor 3.
    let fds: Vec<&str> = lines
        .by_ref()
        .take_while(|line| !line.contains('='))
        .collect();
    assert_eq!(fds, ["0", "1", "2", "3"], "{stdout}");
    let env: Vec<&str> = std::iter::once(stdout.lines().find(|l| l.contains('=')).unwrap())
        .chain(lines)
        .collect();
    for var in SHIM_VARS {
        assert!(
            !env.iter().any(|line| line.starts_with(&format!("{var}="))),
            "{var} reached the agent: {stdout}"
        );
    }
    assert!(
        env.contains(&"HUMANITL_TEST=1"),
        "other variables pass: {stdout}"
    );
    // The report descriptor was used before it was closed.
    assert_eq!(read_report(read_end).len(), 5);
}

// ---- the report -----------------------------------------------------------------

#[test]
fn report_has_one_line_per_check() {
    let socket = socket_path("report");
    let mut command = shim_with_bridge(&socket, &["true"]);
    let (read_end, write_end) = report_pipe(&mut command);
    let status = command.status().unwrap();
    drop(write_end);
    assert_eq!(code(status), 0);
    let lines = read_report(read_end);
    assert_eq!(lines.len(), 5, "{lines:?}");
    assert!(
        lines[0].starts_with("CHECK bridge_listening ok proxy=127.0.0.1:"),
        "{}",
        lines[0]
    );
    assert!(
        lines[0].ends_with(&format!("->{}", socket.display())),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("CHECK no_interfaces ok lo")
            || lines[1].starts_with("CHECK no_interfaces fail "),
        "{}",
        lines[1]
    );
    // On the host the walk finds the machine's own sockets, so only the shape
    // is fixed here; that the line answers the question is
    // `single_socket_names_every_socket_the_bridges_do_not_serve`.
    assert!(
        lines[2].starts_with("CHECK single_socket ok sockets=")
            || lines[2].starts_with("CHECK single_socket fail sockets="),
        "{}",
        lines[2]
    );
    assert!(lines[2].contains(";entries="), "{}", lines[2]);
    assert!(lines[2].contains(";limit="), "{}", lines[2]);
    assert_eq!(lines[3], "CHECK seccomp_applied ok Seccomp:2;NoNewPrivs:1");
    assert!(lines[4].starts_with("CHECK families ok "), "{}", lines[4]);
    for needle in [
        "socket(AF_UNIX,SOCK_STREAM)=EPERM",
        "socket(AF_INET,SOCK_DGRAM)=EPERM",
        "x32:socket=EPERM",
        "io_uring_setup=EPERM",
        "socket(AF_INET,SOCK_STREAM)=ok",
    ] {
        assert!(
            lines[4].contains(needle),
            "{needle} missing in {}",
            lines[4]
        );
    }
    for line in &lines {
        assert_eq!(
            line.split(' ').count(),
            4,
            "evidence carries no spaces: {line}"
        );
    }
}

/// The second guarantee, measured: the shim names every Unix socket it finds
/// and marks the line `fail` when one of them is not a bridge's.
///
/// `bridge_listening` only shows that the one door is open; the walk is what
/// shows there is no second one (Review-Befund vom 2026-09-03). It runs on the
/// host here, where the machine's own sockets are in the way, so the test
/// plants a socket of its own and asks two questions of the line: does it name
/// the intruder, and does it leave the bridge's own socket out of
/// `unexpected`.
#[test]
fn single_socket_names_every_socket_the_bridges_do_not_serve() {
    // Beide direkt unter /tmp: der Suchlauf des Shims reicht drei Ebenen tief,
    // und ein TMPDIR weiter unten im Dateisystem läge außerhalb.
    let served = shallow_socket_path("walkserved");
    shouting_server(&served);
    let intruder = shallow_socket_path("walkintruder");
    let _ = fs::remove_file(&intruder);
    let _listener = UnixListener::bind(&intruder).unwrap();

    let mut command = shim_with_bridge(&served, &["true"]);
    let (read_end, write_end) = report_pipe(&mut command);
    let status = command.status().unwrap();
    drop(write_end);
    assert_eq!(code(status), 0);

    let lines = read_report(read_end);
    let line = lines
        .iter()
        .find(|line| line.starts_with("CHECK single_socket "))
        .unwrap_or_else(|| panic!("no single_socket line: {lines:?}"));
    assert!(line.starts_with("CHECK single_socket fail "), "{line}");
    let unexpected = line
        .split(';')
        .find(|field| field.starts_with("unexpected="))
        .unwrap_or_else(|| panic!("no unexpected field: {line}"));
    assert!(
        unexpected.contains(intruder.to_str().unwrap()),
        "the socket nobody serves is named: {line}"
    );
    assert!(
        !unexpected.contains(served.to_str().unwrap()),
        "the bridge's own socket is expected: {line}"
    );
    assert!(
        line.contains(served.to_str().unwrap()),
        "the evidence names every socket found, the bridge's included: {line}"
    );
    let _ = fs::remove_file(&intruder);
    let _ = fs::remove_file(&served);
}

/// Runs a shim that is expected to fail during setup and returns its exit
/// code, the report lines and stderr.
fn failed_setup(mut command: Command) -> (i32, Vec<String>, String) {
    let (read_end, write_end) = report_pipe(&mut command);
    let output = command.output().unwrap();
    drop(write_end);
    (
        code(output.status),
        read_report(read_end),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Every setup failure is a failed check line, not only a line on stderr.
///
/// The launcher reads the report, not the shim's stderr: without these lines
/// a shim that never got as far as binding would look exactly like a shim
/// that wrote no report at all (`SANDBOX_013`), and the two have different
/// causes and different fixes.
#[test]
fn a_setup_failure_is_a_failed_check_line() {
    let socket = socket_path("failline");

    // An environment that does not describe a policy: the agent will never
    // carry a filter.
    let mut command = shim_with_bridge(&socket, &["true"]);
    command.env("HUMANITL_SECCOMP_DENY", "frobnicate");
    let (status, lines, stderr) = failed_setup(command);
    assert_eq!(status, 126, "{stderr}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("CHECK seccomp_applied fail ")
                && line.contains("frobnicate")),
        "{lines:?}"
    );

    // A bridge list that is not the shape the launcher writes.
    let mut command = shim_with_bridge(&socket, &["true"]);
    command.env("HUMANITL_BRIDGES", "[{");
    let (status, lines, stderr) = failed_setup(command);
    assert_eq!(status, 126, "{stderr}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("CHECK bridge_listening fail ")
                && line.contains("HUMANITL_BRIDGES")),
        "{lines:?}"
    );

    // A bridge the shim cannot serve.
    let mut command = shim();
    command
        .env(
            "HUMANITL_BRIDGES",
            r#"[{"name":"cdp","dir":"out","listen":"127.0.0.1:9222","socket":"/run/humanitl/cdp.sock"}]"#,
        )
        .args(["--proxy-port", "9222", "--", "true"]);
    let (status, lines, stderr) = failed_setup(command);
    assert_eq!(status, 126, "{stderr}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("CHECK bridge_listening fail ")
                && line.contains("out_not_supported_yet")),
        "{lines:?}"
    );

    // A port that is already taken: bind fails, and the line says so.
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let mut command = shim();
    command
        .env(
            "HUMANITL_BRIDGES",
            format!(
                r#"[{{"name":"proxy","dir":"in","listen":"127.0.0.1:{port}","socket":"/s.sock"}}]"#
            ),
        )
        .args(["--proxy-port", &port.to_string(), "--", "true"]);
    let (status, lines, stderr) = failed_setup(command);
    assert_eq!(status, 126, "{stderr}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("CHECK bridge_listening fail ")
                && line.contains("cannot_listen")),
        "{lines:?}"
    );
    drop(taken);

    // The evidence stays one word, whatever the message was.
    for line in lines {
        assert_eq!(
            line.split(' ').count(),
            4,
            "evidence carries spaces: {line}"
        );
    }
}

#[test]
fn a_bad_report_descriptor_is_a_warning_not_a_failure() {
    let socket = socket_path("badfd");
    let output = shim_with_bridge(&socket, &["true"])
        .env("HUMANITL_REPORT_FD", "199")
        .output()
        .unwrap();
    assert_eq!(code(output.status), 0);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning: no report"), "{stderr}");
}

// ---- the bridge -----------------------------------------------------------------

#[test]
fn bridge_forwards_to_the_unix_socket_and_signals_reach_the_agent() {
    let socket = socket_path("bridge");
    shouting_server(&socket);
    let mut command = shim_with_bridge(&socket, &["sleep", "30"]);
    let (read_end, write_end) = report_pipe(&mut command);
    let mut child = command.spawn().unwrap();
    drop(write_end);
    let mut reader = BufReader::new(fs::File::from(read_end));
    let line = wait_for_line(&mut reader, "bridge_listening");
    // CHECK bridge_listening ok proxy=127.0.0.1:PORT->/path
    let addr = line
        .split(' ')
        .nth(3)
        .unwrap()
        .trim_start_matches("proxy=")
        .split("->")
        .next()
        .unwrap()
        .to_owned();

    // The bridge is in the parent, which carries its own filter.
    let parent = wait_for_status(child.id(), "Seccomp:\t2\n");
    assert!(parent.contains("NoNewPrivs:\t1\n"), "{parent}");

    let mut client = TcpStream::connect(&addr).unwrap();
    client.write_all(b"hello through the bridge").unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut reply = String::new();
    client.read_to_string(&mut reply).unwrap();
    assert_eq!(reply, "HELLO THROUGH THE BRIDGE");

    // SAFETY: kill with the pid of the child this test spawned.
    unsafe {
        libc::kill(libc::pid_t::try_from(child.id()).unwrap(), libc::SIGTERM);
    }
    let status = wait_with_timeout(&mut child);
    assert_eq!(
        code(status),
        128 + libc::SIGTERM,
        "sleep died of the forwarded SIGTERM"
    );
    let _ = fs::remove_file(&socket);
}

#[test]
fn an_open_bridge_connection_does_not_keep_the_shim_alive_or_kill_it() {
    let socket = socket_path("open");
    shouting_server(&socket);
    let mut command = shim_with_bridge(&socket, &["sh", "-c", "sleep 0.3; exit 5"]);
    let (read_end, write_end) = report_pipe(&mut command);
    let mut child = command.spawn().unwrap();
    drop(write_end);
    let mut reader = BufReader::new(fs::File::from(read_end));
    let line = wait_for_line(&mut reader, "bridge_listening");
    let addr = line.split(' ').nth(3).unwrap()["proxy=".len()..]
        .split("->")
        .next()
        .unwrap()
        .to_owned();
    let mut client = TcpStream::connect(&addr).unwrap();
    client.write_all(b"ping").unwrap();
    let mut four = [0u8; 4];
    client.read_exact(&mut four).unwrap();
    assert_eq!(&four, b"PING");
    // The agent exits while the connection is open: the shim answers with
    // the agent's status, not with SIGPIPE's.
    let status = wait_with_timeout(&mut child);
    assert_eq!(code(status), 5);
    let mut rest = Vec::new();
    let _ = client.read_to_end(&mut rest);
    assert!(rest.is_empty());
    let _ = fs::remove_file(&socket);
}

#[test]
fn bridge_with_a_dead_socket_still_lets_the_agent_run() {
    let socket = socket_path("dead");
    let _ = fs::remove_file(&socket);
    let status = shim_with_bridge(&socket, &["sh", "-c", "exit 3"])
        .status()
        .unwrap();
    assert_eq!(code(status), 3);
}

#[test]
fn a_port_that_is_taken_is_a_bridge_setup_failure() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let output = shim()
        .env(
            "HUMANITL_BRIDGES",
            format!(
                r#"[{{"name":"proxy","dir":"in","listen":"127.0.0.1:{port}","socket":"/s.sock"}}]"#
            ),
        )
        .args(["--proxy-port", &port.to_string(), "--", "true"])
        .output()
        .unwrap();
    assert_eq!(code(output.status), 126);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bridge setup failed"), "{stderr}");
    assert!(stderr.contains("cannot listen"), "{stderr}");
    drop(taken);
}
