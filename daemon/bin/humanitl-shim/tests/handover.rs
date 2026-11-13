//! Die Deskriptoren über den Socket der Übergabe statt durch Vererbung
//! (HUM-434), am gebauten Shim.
//!
//! Der Test spielt den Daemon: Er bindet den `SOCK_SEQPACKET`-Socket, nimmt
//! genau eine Verbindung an, liest das Zeichen und schickt die Schreibseite
//! des Berichts und das Leseende der Lebensleine in einer Nachricht. Der Shim
//! legt sie auf 3 und 4, schreibt seinen Bericht auf 3 und endet, wenn die
//! Lebensleine zugeht. Ohne jemanden am Socket startet er den Befehl nie.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

mod support;

// Nur die Gnadenfrist gebraucht; die übrigen Werte der Datei nicht.
#[path = "../src/contract.rs"]
#[allow(dead_code)]
mod contract;
use contract::KILL_GRACE;

const SHIM: &str = env!("CARGO_BIN_EXE_humanitl-shim");

const SHIM_VARS: [&str; 5] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
    "HUMANITL_REPORT_FD",
];

const TOKEN: &str = "00112233445566778899aabbccddeeff";

/// Ein kurzer Pfad: Ein Unix-Socket nimmt höchstens 107 Bytes.
fn socket_path(tag: &str) -> PathBuf {
    let name = format!("hl-handover-{}-{tag}", std::process::id());
    let path = std::env::temp_dir().join(&name);
    if path.as_os_str().len() < 100 {
        path
    } else {
        PathBuf::from("/tmp").join(name)
    }
}

fn pipe() -> (OwnedFd, OwnedFd) {
    let (read, write) = std::io::pipe().unwrap();
    (OwnedFd::from(read), OwnedFd::from(write))
}

/// Der lauschende Socket des Daemons, Rückstau 1.
fn listen(path: &Path) -> OwnedFd {
    let _ = std::fs::remove_file(path);
    // SAFETY: socket(2) mit festen Argumenten.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(raw >= 0, "{}", std::io::Error::last_os_error());
    // SAFETY: socket hat ihn gerade geliefert.
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: einfache Daten.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::sa_family_t::try_from(libc::AF_UNIX).unwrap();
    for (slot, byte) in address.sun_path.iter_mut().zip(path.as_os_str().as_bytes()) {
        *slot = byte.cast_signed();
    }
    let size = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_un>()).unwrap();
    // SAFETY: bind und listen auf dem eigenen Socket.
    unsafe {
        assert_eq!(
            libc::bind(socket.as_raw_fd(), (&raw const address).cast(), size),
            0,
            "{}",
            std::io::Error::last_os_error()
        );
        assert_eq!(libc::listen(socket.as_raw_fd(), 1), 0);
    }
    socket
}

/// Nimmt eine Verbindung an, liest das Zeichen und schickt `fds`, wenn es
/// stimmt; gibt das gelesene Zeichen zurück.
fn serve(listener: &OwnedFd, path: &Path, fds: &[RawFd]) -> String {
    // SAFETY: accept auf dem eigenen Socket.
    let raw = unsafe {
        libc::accept4(
            listener.as_raw_fd(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        )
    };
    assert!(raw >= 0, "{}", std::io::Error::last_os_error());
    // SAFETY: accept hat ihn gerade geliefert.
    let connection = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut token = [0u8; 64];
    // SAFETY: recv in einen Puffer seiner eigenen Länge.
    let n = unsafe {
        libc::recv(
            connection.as_raw_fd(),
            token.as_mut_ptr().cast(),
            token.len(),
            0,
        )
    };
    let token = String::from_utf8_lossy(&token[..usize::try_from(n).unwrap()]).into_owned();
    // Wie der Daemon: Die Datei geht, bevor ein Deskriptor geht.
    let _ = std::fs::remove_file(path);
    if token != TOKEN {
        return token;
    }
    let mut count = [u8::try_from(fds.len()).unwrap()];
    let mut iov = libc::iovec {
        iov_base: count.as_mut_ptr().cast(),
        iov_len: 1,
    };
    let mut control = [0u64; 16];
    let bytes = u32::try_from(std::mem::size_of_val(fds)).unwrap();
    // SAFETY: eine SCM_RIGHTS-Nachricht in einem ausreichend großen,
    // ausgerichteten Puffer, mit den Makros dafür.
    unsafe {
        let mut message: libc::msghdr = std::mem::zeroed();
        message.msg_iov = &raw mut iov;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        // `socklen_t` on musl, `usize` on glibc.
        #[allow(clippy::useless_conversion)]
        {
            message.msg_controllen = libc::CMSG_SPACE(bytes).try_into().unwrap();
        }
        let header = libc::CMSG_FIRSTHDR(&raw const message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        // `socklen_t` on musl, `usize` on glibc.
        #[allow(clippy::useless_conversion)]
        {
            (*header).cmsg_len = libc::CMSG_LEN(bytes).try_into().unwrap();
        }
        std::ptr::copy_nonoverlapping(
            fds.as_ptr().cast::<u8>(),
            libc::CMSG_DATA(header),
            std::mem::size_of_val(fds),
        );
        assert_eq!(
            libc::sendmsg(connection.as_raw_fd(), &raw const message, 0),
            1
        );
    }
    token
}

/// Stirbt der Daemon vor der Übergabe, lauscht niemand mehr: Der Shim
/// startet den Befehl nie und endet mit 126.
#[test]
fn without_anyone_at_the_handover_socket_the_command_never_runs() {
    let dir = std::env::temp_dir().join(format!("hl-handover-gone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("ran");
    let mut command = Command::new(SHIM);
    for var in SHIM_VARS {
        command.env_remove(var);
    }
    let output = command
        .args([
            "--proxy-port",
            "0",
            "--handover",
            &dir.join("gone").display().to_string(),
            "--handover-token",
            TOKEN,
            "--lifeline-fd",
            "4",
            "--",
            "touch",
            &marker.display().to_string(),
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(126), "{stderr}");
    assert!(stderr.contains("did not arrive"), "{stderr}");
    assert!(!marker.exists(), "the command ran without its descriptors");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Bericht und Lebensleine kommen über die Übergabe an: Der Bericht steht
/// auf 3, und das Ende der Lebensleine beendet die Sandbox binnen der Frist.
#[test]
#[allow(clippy::too_many_lines)]
fn the_lifeline_through_the_handover_ends_the_sandbox() {
    if !support::bwrap_as_pid_1_usable() {
        return;
    }
    let path = socket_path("lifeline");
    let listener = listen(&path);
    let (report_read, report_write) = pipe();
    let (line_read, line_write) = pipe();
    let bridge = socket_path("bridge");
    let mut command = Command::new("bwrap");
    for var in SHIM_VARS {
        command.env_remove(var);
    }
    command
        .env(
            "HUMANITL_BRIDGES",
            format!(
                r#"[{{"name":"proxy","dir":"in","listen":"127.0.0.1:0","socket":"{}"}}]"#,
                bridge.display()
            ),
        )
        .env("HUMANITL_REPORT_FD", "3")
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
            "--",
            SHIM,
            "--proxy-port",
            "0",
            "--handover",
            &path.display().to_string(),
            "--handover-token",
            TOKEN,
            "--lifeline-fd",
            "4",
            "--",
            "sh",
            "-c",
            "echo up; exec sleep 600",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let token = serve(
        &listener,
        &path,
        &[report_write.as_raw_fd(), line_read.as_raw_fd()],
    );
    assert_eq!(token, TOKEN);
    // Ab hier hält nur die Sandbox Schreibseite des Berichts und Leseende
    // der Lebensleine.
    drop((listener, report_write, line_read));
    let _ = std::fs::remove_file(&path);

    let mut up = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut up)
        .unwrap();
    let mut errors = child.stderr.take().unwrap();
    assert_eq!(up, "up\n", "the agent did not start");
    let report = thread::spawn(move || {
        let mut text = String::new();
        let _ = std::fs::File::from(report_read).read_to_string(&mut text);
        text
    });

    drop(line_write);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > KILL_GRACE + Duration::from_secs(2) {
            let _ = child.kill();
            panic!("the sandbox outlived its lifeline");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let mut stderr = String::new();
    let _ = errors.read_to_string(&mut stderr);
    eprintln!(
        "ended {:?} after the lifeline with {status:?}",
        started.elapsed()
    );
    assert!(
        status.code().is_some() || status.signal().is_some(),
        "{stderr}"
    );
    let report = report.join().unwrap();
    assert!(
        report
            .lines()
            .any(|line| line.starts_with("CHECK bridge_listening ")),
        "the report did not arrive on descriptor 3: {report:?}\n{stderr}"
    );
    let single = report
        .lines()
        .find(|line| line.starts_with("CHECK single_socket "))
        .unwrap_or_default();
    // Der Socket der Übergabe ist weg, bevor der Shim weitermacht; die
    // Zeile nennt ihn nicht und kennt keine Ausnahme für ihn.
    assert!(!path.exists());
    let name = path.display().to_string();
    assert!(!single.contains(&name), "{single}");
    assert!(!single.contains("handover"), "{single}");
}
