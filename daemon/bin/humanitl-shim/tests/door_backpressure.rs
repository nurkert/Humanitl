//! Ein Daemon, der die Tür für weitere Shells nicht mehr liest, hält weder
//! die Lebensleine noch den Agenten auf (HUM-451).
//!
//! Die Antworten des Shims (`R`, `F`, `E`) gehen über eine begrenzte
//! Warteschlange, die PID 1 bei `POLLOUT` leert (HUM-430). Die Unit-Tests in
//! `src/exec.rs` prüfen die Schlange mit einem gespielten Socket; hier läuft
//! der gebaute Shim als PID 1 eines frischen PID-Namensraums, wie ihn der
//! Launcher startet, mit echter Tür (`--control-fd`), echter Lebensleine
//! (`--lifeline-fd`) und echten Pseudo-Terminals.
//!
//! Der Test spielt den Daemon und liest seine Seite der Tür nie. Er öffnet
//! erst [`MAX_SHELLS`] Shells, die bis zum Tod laufen, dann so lange weitere,
//! bis seine eigene Seite dauerhaft `EAGAIN` meldet: Der Shim nimmt keine
//! Anfrage mehr an. Eine Puffergröße wird nie angenommen, sie hängt am Kernel.
//! Mit der Warteschlange hat der Shim dann seinen Socket voll gesehen
//! (`EAGAIN` auf seiner Seite), die Schlange ist übergelaufen, und er hat die
//! Tür aufgegeben. Mit einem blockierenden `send` hängt PID 1 stattdessen im
//! `send` fest und hört die Lebensleine nicht mehr; das macht Fall 2 rot.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{self, BufRead, BufReader, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

mod support;

const SHIM: &str = env!("CARGO_BIN_EXE_humanitl-shim");

/// Die fünf Variablen des Vertrags; jeder Start beginnt ohne sie.
const SHIM_VARS: [&str; 5] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
    "HUMANITL_REPORT_FD",
];

/// Die Gnadenfrist des Shims nach dem Ende der Lebensleine;
/// `supervise::KILL_GRACE` im Binary, hier ausgeschrieben, weil ein
/// Integrationstest das Binary nicht als Bibliothek sieht.
const KILL_GRACE: Duration = Duration::from_secs(5);

/// So viele weitere Shells laufen höchstens gleichzeitig;
/// `exec::MAX_SHELLS` im Binary.
const MAX_SHELLS: u32 = 4;

/// Die Warnung, mit der der Shim die Tür aufgibt (`Shells::give_up`).
const GIVEN_UP: &str = "shell replies cannot reach the daemon";

/// So lange muss die eigene Seite der Tür nach `EAGAIN` unbeschreibbar
/// bleiben, bis sie als dauerhaft voll gilt. Ein Shim, der noch liest, nimmt
/// binnen Millisekunden die nächste Anfrage ab.
const STALLED: Duration = Duration::from_secs(3);

/// Obergrenze für das Fluten; ein Kernel, dessen Puffer so lange nicht voll
/// wird, ist ein Fehler im Test, kein grüner Lauf.
const FLOOD_LIMIT: Duration = Duration::from_secs(60);

/// Eine weitere Shell, die sich meldet und dann bis zu ihrem Tod läuft.
const SHELL: [&str; 3] = ["sh", "-c", "echo shell-up; exec sleep 600"];

/// Der Agent: meldet sich, überhört `SIGTERM` und läuft, bis ihn `SIGKILL`
/// trifft. So endet er in Fall 2 nur über die Frist der PID-1-Schleife.
const AGENT: &str = "trap '' TERM; echo up; while :; do sleep 0.1; done";

/// Der Kopf einer Anfrage, `S` (`exec.rs`, Abschnitt Wire format).
const MARK_SPAWN: u8 = b'S';

/// Eine Sandbox mit Tür und Lebensleine; der Test hält die Seite des Daemons.
struct Sandbox {
    child: Child,
    /// Die Seite des Daemons an der Tür; der Test liest sie nie.
    door: OwnedFd,
    /// Das Schreibende der Lebensleine, nur hier.
    lifeline: Option<OwnedFd>,
    /// Was der Shim und die Shells auf stderr schreiben.
    stderr: Arc<Mutex<String>>,
    /// Die nächste freie Shell-Id.
    next_id: u32,
}

impl Drop for Sandbox {
    /// Räumt auch nach einem fehlgeschlagenen Assert ab: Mit bwrap endet
    /// der ganze Namensraum, wenn der äußere Prozess endet.
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl Sandbox {
    fn stderr(&self) -> String {
        self.stderr
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

fn code(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap())
}

/// `(read, write)` einer Pipe, beide mit `CLOEXEC`.
fn pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` ist ein gültiges Feld aus zwei Einträgen für pipe2.
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    // SAFETY: pipe2 hat beide eben angelegt, niemand sonst besitzt sie.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

/// `(daemon, shim)`: das `SOCK_SEQPACKET`-Paar der Tür, beide mit `CLOEXEC`.
fn door_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` ist ein gültiges Feld aus zwei Einträgen für socketpair.
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    assert_eq!(rc, 0, "{}", io::Error::last_os_error());
    // SAFETY: socketpair hat beide eben angelegt, niemand sonst besitzt sie.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

/// `(leader, follower)` eines frischen Pseudo-Terminals, beide mit
/// `CLOEXEC`, der Follower mit `O_NOCTTY` wie beim Daemon.
fn pty() -> (OwnedFd, OwnedFd) {
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC;
    // SAFETY: posix_openpt nimmt Flags und legt einen neuen Deskriptor an.
    let raw = unsafe { libc::posix_openpt(flags) };
    assert!(raw >= 0, "posix_openpt: {}", io::Error::last_os_error());
    // SAFETY: eben angelegt, niemand sonst besitzt ihn.
    let leader = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: unlockpt auf dem eigenen Leader.
    assert_eq!(unsafe { libc::unlockpt(leader.as_raw_fd()) }, 0);
    // SAFETY: TIOCGPTPEER öffnet den Follower des eigenen Leaders mit den
    // genannten Flags und nimmt sie als int.
    let raw = unsafe { libc::ioctl(leader.as_raw_fd(), libc::TIOCGPTPEER, flags) };
    assert!(raw >= 0, "TIOCGPTPEER: {}", io::Error::last_os_error());
    // SAFETY: eben angelegt, niemand sonst besitzt ihn.
    (leader, unsafe { OwnedFd::from_raw_fd(raw) })
}

/// Lässt `fds` unter ihren Nummern in das Kind von `command` durch.
fn inherit(command: &mut Command, fds: [RawFd; 2]) {
    // SAFETY: läuft im geforkten Kind vor exec und nimmt nur diesen
    // Deskriptoren `CLOEXEC`.
    unsafe {
        command.pre_exec(move || {
            for fd in fds {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

/// Startet den Shim als PID 1 unter bwrap, mit Tür und Lebensleine, und
/// wartet, bis der Agent sich meldet. `None`, wo bwrap nicht nutzbar ist.
fn start(tag: &str) -> Option<Sandbox> {
    if !support::bwrap_as_pid_1_usable() {
        return None;
    }
    let (door, shim_door) = door_pair();
    let (line_read, line_write) = pipe();
    // Eine Brücke auf einen Socket, den niemand bedient: der Shim verlangt
    // `--proxy-port`, und eine tote Brücke stört den Agenten nicht.
    let socket = std::env::temp_dir().join(format!(
        "humanitl-shim-door-{}-{tag}.sock",
        std::process::id()
    ));
    let socket = if socket.as_os_str().len() < 100 {
        socket
    } else {
        PathBuf::from("/tmp").join(socket.file_name().unwrap())
    };
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
            "--",
            SHIM,
            "--proxy-port",
            "0",
            "--control-fd",
            &shim_door.as_raw_fd().to_string(),
            "--lifeline-fd",
            &line_read.as_raw_fd().to_string(),
            "--",
            "sh",
            "-c",
            AGENT,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    inherit(&mut command, [shim_door.as_raw_fd(), line_read.as_raw_fd()]);
    let mut child = command.spawn().unwrap();
    // Ab hier hält nur noch die Sandbox das Shim-Ende der Tür und das
    // Leseende der Lebensleine, nur der Test das Schreibende.
    drop((shim_door, line_read));

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
    let mut sandbox = Sandbox {
        child,
        door,
        lifeline: Some(line_write),
        stderr,
        next_id: 1,
    };
    let mut up = String::new();
    BufReader::new(sandbox.child.stdout.take().unwrap())
        .read_line(&mut up)
        .unwrap();
    assert_eq!(up, "up\n", "the agent did not start: {}", sandbox.stderr());
    Some(sandbox)
}

/// Eine Anfrage `S`: Id, dann jedes Argument mit NUL.
fn spawn_request(id: u32, command: &[&str]) -> Vec<u8> {
    let mut payload = vec![MARK_SPAWN];
    payload.extend_from_slice(&id.to_le_bytes());
    for arg in command {
        payload.extend_from_slice(arg.as_bytes());
        payload.push(0);
    }
    payload
}

/// Steuerpuffer für genau einen Deskriptor, ausgerichtet für `cmsghdr`.
#[repr(C, align(8))]
struct CmsgBuf([u8; 64]);

/// Schickt eine Anfrage mit `follower` als `SCM_RIGHTS`, ohne zu blockieren.
fn send_request(door: RawFd, payload: &[u8], follower: RawFd) -> io::Result<()> {
    let mut iov = libc::iovec {
        iov_base: payload.as_ptr().cast_mut().cast(),
        iov_len: payload.len(),
    };
    let mut buf = CmsgBuf([0; 64]);
    let one = u32::try_from(size_of::<libc::c_int>()).unwrap();
    // SAFETY: CMSG_SPACE rechnet nur.
    let space = usize::try_from(unsafe { libc::CMSG_SPACE(one) }).unwrap();
    assert!(space <= buf.0.len());
    // SAFETY: einfache Daten; alle Zeiger werden unten gesetzt.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &raw mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = buf.0.as_mut_ptr().cast();
    #[allow(clippy::useless_conversion)]
    {
        msg.msg_controllen = space.try_into().unwrap();
    }
    // SAFETY: `msg` beschreibt `buf` mit Platz für einen Kopf und einen
    // Deskriptor; die Makros bleiben in `msg_controllen`.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&raw const msg);
        assert!(!cmsg.is_null());
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        #[allow(clippy::useless_conversion)]
        {
            (*cmsg).cmsg_len = libc::CMSG_LEN(one).try_into().unwrap();
        }
        std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<libc::c_int>(), follower);
    }
    loop {
        // SAFETY: `msg` beschreibt `payload` und `buf`, beide leben über den
        // Aufruf hinaus.
        let sent = unsafe {
            libc::sendmsg(
                door,
                &raw const msg,
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent >= 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

/// `revents` von `fd` für `events`, höchstens `timeout` lang gewartet.
fn poll_one(fd: RawFd, events: libc::c_short, timeout: Duration) -> libc::c_short {
    let deadline = Instant::now() + timeout;
    loop {
        let mut pfd = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let left = deadline.saturating_duration_since(Instant::now());
        let ms = libc::c_int::try_from(left.as_millis()).unwrap_or(libc::c_int::MAX);
        // SAFETY: ein gültiger Eintrag, der den Aufruf überlebt.
        let ready = unsafe { libc::poll(&raw mut pfd, 1, ms) };
        if ready >= 0 {
            return pfd.revents;
        }
        assert_eq!(
            io::Error::last_os_error().kind(),
            io::ErrorKind::Interrupted,
            "poll"
        );
    }
}

/// Was eine Anfrage an der Tür erreicht hat.
enum Delivery {
    /// Der Kernel hat sie angenommen.
    Sent,
    /// Die Seite des Daemons meldet `EAGAIN` und bleibt [`STALLED`] lang
    /// unbeschreibbar: der Shim nimmt nichts mehr ab.
    Stalled,
}

/// Öffnet eine Shell mit `command` über die Tür. Gibt den Leader des
/// Pseudo-Terminals zurück; der Follower ist mit der Anfrage unterwegs.
fn open_shell(sandbox: &mut Sandbox, command: &[&str]) -> (Delivery, OwnedFd) {
    let id = sandbox.next_id;
    sandbox.next_id += 1;
    let (leader, follower) = pty();
    let payload = spawn_request(id, command);
    loop {
        match send_request(sandbox.door.as_raw_fd(), &payload, follower.as_raw_fd()) {
            Ok(()) => return (Delivery::Sent, leader),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                if poll_one(sandbox.door.as_raw_fd(), libc::POLLOUT, STALLED) & libc::POLLOUT == 0 {
                    return (Delivery::Stalled, leader);
                }
            }
            Err(err) => panic!("request {id} failed: {err}; stderr: {}", sandbox.stderr()),
        }
    }
}

/// Liest vom Leader, bis `needle` kam; `false` nach `timeout` oder wenn das
/// Terminal vorher auflegt.
fn read_until(leader: &OwnedFd, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        let revents = poll_one(
            leader.as_raw_fd(),
            libc::POLLIN,
            deadline.saturating_duration_since(Instant::now()),
        );
        if revents & libc::POLLIN == 0 {
            return false;
        }
        let mut chunk = [0u8; 256];
        // SAFETY: Zeiger und Länge beschreiben `chunk`.
        let n = unsafe { libc::read(leader.as_raw_fd(), chunk.as_mut_ptr().cast(), chunk.len()) };
        let Ok(n) = usize::try_from(n) else {
            return false;
        };
        if n == 0 {
            return false;
        }
        seen.extend_from_slice(&chunk[..n]);
        if String::from_utf8_lossy(&seen).contains(needle) {
            return true;
        }
    }
    false
}

/// Ob kein Prozess mehr den Follower hält: Der Leader meldet dann `POLLHUP`.
fn hung_up(leader: &OwnedFd, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let revents = poll_one(leader.as_raw_fd(), libc::POLLIN, Duration::from_millis(100));
        if revents & libc::POLLHUP != 0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        if revents & libc::POLLIN != 0 {
            // Was die Shell noch geschrieben hat, wird verworfen, damit
            // `POLLIN` das `POLLHUP` nicht verdeckt.
            let mut chunk = [0u8; 256];
            // SAFETY: Zeiger und Länge beschreiben `chunk`; der Leader ist
            // nach `POLLIN` lesbar, also blockiert read nicht.
            unsafe {
                libc::read(leader.as_raw_fd(), chunk.as_mut_ptr().cast(), chunk.len());
            }
        }
    }
}

/// Öffnet [`MAX_SHELLS`] Shells, die bis zu ihrem Tod laufen, und wartet,
/// bis jede sich gemeldet hat. Dann öffnet sie weitere, die der Shim wegen
/// der Grenze mit `F` ablehnt, ohne je eine Antwort zu lesen, bis die Tür
/// dauerhaft voll ist. Gibt die Leader der laufenden Shells zurück.
fn flood(sandbox: &mut Sandbox) -> Vec<OwnedFd> {
    let mut live = Vec::new();
    for _ in 0..MAX_SHELLS {
        let (delivery, leader) = open_shell(sandbox, &SHELL);
        assert!(
            matches!(delivery, Delivery::Sent),
            "the door was full before the first shells: {}",
            sandbox.stderr()
        );
        live.push(leader);
    }
    for (index, leader) in live.iter().enumerate() {
        assert!(
            read_until(leader, "shell-up", Duration::from_secs(10)),
            "shell {} did not start: {}",
            index + 1,
            sandbox.stderr()
        );
    }
    let started = Instant::now();
    let mut sent = 0usize;
    loop {
        let (delivery, leader) = open_shell(sandbox, &SHELL);
        // Diese Shells lehnt der Shim ab; ihr Leader wird nicht gebraucht.
        drop(leader);
        match delivery {
            Delivery::Sent => sent += 1,
            Delivery::Stalled => break,
        }
        assert!(
            started.elapsed() < FLOOD_LIMIT,
            "the door did not fill within {FLOOD_LIMIT:?} after {sent} requests: {}",
            sandbox.stderr()
        );
    }
    eprintln!(
        "door full after {} requests in {:?}; the shim gave up: {}",
        sent + live.len(),
        started.elapsed(),
        sandbox.stderr().contains(GIVEN_UP)
    );
    live
}

/// Wartet bis `timeout`, ob stderr `needle` enthält.
fn stderr_shows(sandbox: &Sandbox, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if sandbox.stderr().contains(needle) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Fall 1: Der Daemon liest nie, die Schlange läuft über. Der Shim gibt die
/// Tür auf und beendet jede weitere Shell; der Agent läuft weiter.
#[test]
fn a_door_nobody_reads_ends_the_further_shells_and_spares_the_agent() {
    let Some(mut sandbox) = start("case1") else {
        return;
    };
    let live = flood(&mut sandbox);
    assert!(
        stderr_shows(&sandbox, GIVEN_UP, Duration::from_secs(10)),
        "the door is full, but the shim never gave up on it: {}",
        sandbox.stderr()
    );
    for (index, leader) in live.iter().enumerate() {
        assert!(
            hung_up(leader, Duration::from_secs(5)),
            "shell {} still runs after the shim gave up the door: {}",
            index + 1,
            sandbox.stderr()
        );
    }
    // Eine Sekunde Abstand: ein Agent, der mit den Shells stürbe, wäre
    // jetzt tot.
    thread::sleep(Duration::from_secs(1));
    assert!(
        sandbox.child.try_wait().unwrap().is_none(),
        "the agent ended with the further shells: {}",
        sandbox.stderr()
    );
}

/// Fall 2: Danach endet die Lebensleine. Der Agent überhört `SIGTERM` und
/// endet trotzdem binnen `KILL_GRACE` plus einer Sekunde, weil die
/// PID-1-Schleife nie in einem `send` an den Daemon feststeckt.
#[test]
fn a_door_nobody_reads_does_not_delay_the_lifeline() {
    let Some(mut sandbox) = start("case2") else {
        return;
    };
    let _live = flood(&mut sandbox);
    drop(sandbox.lifeline.take());
    let started = Instant::now();
    let deadline = started + KILL_GRACE + Duration::from_secs(1);
    let status = loop {
        if let Some(status) = sandbox.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() <= deadline,
            "the agent outlived its lifeline by {:?}: {}",
            started.elapsed(),
            sandbox.stderr()
        );
        thread::sleep(Duration::from_millis(20));
    };
    let elapsed = started.elapsed();
    eprintln!("the sandbox ended {elapsed:?} after the lifeline");
    assert_eq!(
        code(status),
        128 + libc::SIGKILL,
        "{elapsed:?}: {}",
        sandbox.stderr()
    );
    // Erst nach dem Ende geprüft, damit die Mutation (blockierendes `send`)
    // an der Frist der Lebensleine scheitert und nicht schon hier: Der Fall
    // gilt nur, wenn die Tür wirklich übergelaufen war und der Shim sie
    // aufgegeben hatte, nicht wenn er nur kurz keine Zeit zum Lesen bekam.
    assert!(
        stderr_shows(&sandbox, GIVEN_UP, Duration::from_secs(1)),
        "the lifeline ended the sandbox, but the door never overflowed: {}",
        sandbox.stderr()
    );
}
