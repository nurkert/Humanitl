//! Weitere Shells in einer laufenden Sandbox, die Seite des Daemons (HUM-299).
//!
//! Wie `docker exec`: Neben dem Agenten läuft ein zweiter Prozess mit einem
//! eigenen Pseudoterminal, ohne dass jemand von außen in die Namensräume
//! eintritt. Der Daemon bittet den Shim, der als PID 1 in der Sandbox sitzt,
//! um den Prozess; der Shim forkt ihn von innen, und das Kind erbt dort alles,
//! was jeder Prozess der Sandbox trägt (`bin/humanitl-shim/src/exec.rs`).
//!
//! # Die Tür
//!
//! Ein `SOCK_SEQPACKET`-Paar, das der Launcher nur für einen Start am eigenen
//! Pseudoterminal des Daemons anlegt ([`crate::StdioMode::Pty`]). Die eine
//! Seite erbt `bwrap` und reicht sie dem Shim als `--control-fd` weiter; die
//! andere bleibt in diesem Prozess, mit `CLOEXEC`, in [`ShellDoor`]. Das Paar
//! hat keinen Namen im Dateisystem, und im Shim hält nur PID 1 seine Seite.
//! Was über die Tür zurückkommt, ist eine Auskunft des Shims und wird so
//! streng gelesen wie jede Eingabe aus der Sandbox: feste Länge, bekannte
//! Kennung, sonst verworfen. Deskriptoren nimmt der Daemon über diese Tür
//! nie an; `recv(2)` ohne Kontrollpuffer lässt der Kernel sie schließen.
//!
//! # Das Format
//!
//! Steht ausführlich in `exec.rs` des Shims; hier die Kurzform. Hinein geht
//! `S`, die Kennung (vier Bytes, little endian, nie 0) und das Kommando, jedes
//! Argument mit einem NUL dahinter, dazu als `SCM_RIGHTS` genau die
//! Untertanenseite des Pseudoterminals der Shell. Heraus kommen neun Bytes:
//! `R`, `F` oder `E`, die Kennung und ein Wert (PID, Grund, Exit-Status).
//!
//! # Unter macOS: eine Start-Tür
//!
//! Seatbelt kennt keinen Namensraum, in dem ein PID 1 sitzen könnte, und
//! macOS kein `SOCK_SEQPACKET`. Dort startet die Tür jede weitere Shell
//! selbst (`SpawnSpec`): `sandbox-exec` mit demselben Profil, darin der
//! Shim ohne Bridge, der das Terminal übernimmt, das Kommando startet und
//! beim Ende dessen ganze Sitzung beendet (HUM-323). Ende und Exit-Code
//! kommen vom Prozess selbst; endet die Sandbox, bekommt jede Shell `SIGTERM`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::process::ExitStatusExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::TERM_002;
use humanitl_core::{Diagnostic, Severity};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SocketFlags, SocketType,
    recv, sendmsg, socketpair,
};
use rustix::termios::{Winsize, tcsetwinsize};

/// Wie viele weitere Shells gleichzeitig leben dürfen. Der Shim zählt
/// dieselbe Zahl noch einmal (`exec.rs`, `MAX_SHELLS`).
pub const MAX_SHELLS: usize = 4;

/// Der Exit-Code einer Shell, die mit der Sandbox endete, ohne dass der Shim
/// ihr Ende noch melden konnte: `128 + SIGKILL`, wie eine Shell es liest.
pub const ENDED_WITH_SANDBOX: i32 = 128 + 9;

/// Die längste Anfrage, wie der Shim sie annimmt.
const MAX_REQUEST: usize = 16 * 1024;

/// Die meisten Argumente eines Kommandos.
const MAX_ARGS: usize = 256;

/// Wie lange der Daemon auf `R` oder `F` wartet.
const ANSWER_WAIT: Duration = Duration::from_secs(5);

/// Wie lange nach dem Ende einer Shell noch gelesen wird, was sie zuletzt
/// geschrieben hat.
const DRAIN: Duration = Duration::from_millis(300);

/// Wie lange nach dem Ende des Terminals auf die Meldung des Shims gewartet
/// wird.
const EXIT_WAIT: Duration = Duration::from_secs(2);

const MARK_SPAWN: u8 = b'S';
const MARK_STARTED: u8 = b'R';
const MARK_REFUSED: u8 = b'F';
const MARK_EXITED: u8 = b'E';

/// `F`-Grund des Shims: schon [`MAX_SHELLS`] Shells.
const REFUSED_LIMIT: i32 = -1;
/// `F`-Grund des Shims: eine Anfrage, die er nicht versteht.
const REFUSED_MALFORMED: i32 = -2;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `SOCK_CLOEXEC` gibt es unter macOS nicht; dort setzt [`pair`] das Flag
/// danach (HUM-312).
#[cfg(target_os = "linux")]
const PAIR_FLAGS: SocketFlags = SocketFlags::CLOEXEC;
#[cfg(not(target_os = "linux"))]
const PAIR_FLAGS: SocketFlags = SocketFlags::empty();

/// Das Paar für eine neue Sandbox: `(Seite des Daemons, Seite des Shims)`,
/// beide mit `CLOEXEC`. Die Seite des Shims verliert es erst im Kind
/// (`inherit.rs`).
///
/// # Errors
///
/// Der Fehler von `socketpair(2)`.
pub(crate) fn pair() -> std::io::Result<(OwnedFd, OwnedFd)> {
    socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, PAIR_FLAGS, None)
        .map_err(std::io::Error::from)
        .and_then(|(one, two)| {
            crate::platform::set_cloexec(&one)?;
            crate::platform::set_cloexec(&two)?;
            Ok((one, two))
        })
}

/// Wo eine Shell steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Die Anfrage ist unterwegs.
    Asked,
    /// Der Shim hat sie gestartet.
    Started,
    /// Der Shim hat abgelehnt, mit seinem Grund.
    Refused(i32),
    /// Sie ist beendet, mit diesem Code.
    Exited(i32),
    /// Ihr [`Shell`] ist fort, der Prozess womöglich nicht; er zählt weiter
    /// gegen die Grenze, bis der Shim sein Ende meldet.
    Orphaned,
}

#[derive(Debug, Default)]
struct DoorState {
    next_id: u32,
    shells: BTreeMap<u32, Phase>,
    /// Die Seite des Shims ist zu: Die Sandbox ist zu Ende.
    closed: bool,
}

/// Wie weitere Shells starten: über die Tür des Shims (Linux) oder als
/// eigener Prozess unter demselben Profil (macOS, HUM-323).
#[derive(Debug)]
enum Door {
    Socket(OwnedFd),
    Spawn(SpawnSpec),
}

/// Was eine Start-Tür braucht: das Programm, die Argumente bis vor das
/// Kommando, die Umgebung.
#[derive(Debug, Clone)]
pub(crate) struct SpawnSpec {
    /// `sandbox-exec`.
    pub(crate) program: PathBuf,
    /// Alles zwischen Programm und `--` vor dem Kommando.
    pub(crate) args: Vec<OsString>,
    /// Die Umgebung der Shell.
    pub(crate) env: Vec<(String, String)>,
}

/// Was der Launcher für die Tür zu weiteren Shells mitgibt.
#[derive(Debug)]
pub(crate) enum DoorSeed {
    /// Die Seite des Daemons am `SOCK_SEQPACKET`-Paar (Linux).
    Socket(OwnedFd),
    /// Wie eine Shell startet (macOS).
    Spawn(SpawnSpec),
}

impl DoorSeed {
    /// Die Tür dazu.
    ///
    /// # Errors
    ///
    /// Der Fehler beim Start des Lese-Fadens einer Socket-Tür.
    pub(crate) fn open(self) -> std::io::Result<Arc<ShellDoor>> {
        match self {
            Self::Socket(socket) => ShellDoor::start(socket),
            Self::Spawn(spec) => Ok(ShellDoor::spawning(spec)),
        }
    }
}

/// Die Seite des Daemons an der Tür zu weiteren Shells einer Sandbox.
#[derive(Debug)]
pub struct ShellDoor {
    door: Door,
    state: Mutex<DoorState>,
    changed: Condvar,
    /// Die Prozesse einer Start-Tür, je Kennung.
    children: Mutex<BTreeMap<u32, u32>>,
}

impl ShellDoor {
    /// Nimmt die Seite des Daemons und liest die Antworten des Shims auf
    /// einem eigenen Faden, bis die Sandbox endet.
    ///
    /// # Errors
    ///
    /// Der Faden ließ sich nicht starten.
    pub(crate) fn start(socket: OwnedFd) -> std::io::Result<Arc<Self>> {
        let door = Arc::new(Self {
            door: Door::Socket(socket),
            state: Mutex::new(DoorState {
                next_id: 1,
                ..DoorState::default()
            }),
            changed: Condvar::new(),
            children: Mutex::new(BTreeMap::new()),
        });
        let reader = Arc::clone(&door);
        thread::Builder::new()
            .name("sandbox-shells".to_owned())
            .spawn(move || reader.read_answers())?;
        Ok(door)
    }

    /// Liest Antworten, bis die Seite des Shims zu ist.
    /// Eine Start-Tür (macOS, HUM-323); sie braucht keinen Lese-Faden.
    pub(crate) fn spawning(spec: SpawnSpec) -> Arc<Self> {
        Arc::new(Self {
            door: Door::Spawn(spec),
            state: Mutex::new(DoorState {
                next_id: 1,
                ..DoorState::default()
            }),
            changed: Condvar::new(),
            children: Mutex::new(BTreeMap::new()),
        })
    }

    /// Die Sandbox ist zu Ende: keine neuen Shells mehr, und die einer
    /// Start-Tür bekommen `SIGTERM`. Eine Socket-Tür schließt der Shim selbst.
    pub(crate) fn close(&self) {
        lock(&self.state).closed = true;
        self.changed.notify_all();
        for &pid in lock(&self.children).values() {
            if let Some(pid) = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap_or(0)) {
                let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
            }
        }
    }

    fn read_answers(&self) {
        let Door::Socket(socket) = &self.door else {
            return;
        };
        let mut buffer = [0u8; 64];
        loop {
            match recv(socket, &mut buffer[..], RecvFlags::empty()) {
                Ok((0, _)) => break,
                Ok((n, full)) => {
                    if n == 9 && full == 9 {
                        self.apply(&buffer[..9]);
                    }
                }
                Err(rustix::io::Errno::INTR) => {}
                Err(_) => break,
            }
        }
        lock(&self.state).closed = true;
        self.changed.notify_all();
    }

    /// Trägt eine Antwort ein. Eine Kennung, nach der niemand gefragt hat,
    /// und ein Übergang, den es nicht gibt, ändern nichts.
    fn apply(&self, answer: &[u8]) {
        let id = u32::from_le_bytes([answer[1], answer[2], answer[3], answer[4]]);
        let value = i32::from_le_bytes([answer[5], answer[6], answer[7], answer[8]]);
        let mut state = lock(&self.state);
        let Some(phase) = state.shells.get(&id).copied() else {
            return;
        };
        let next = match (answer[0], phase) {
            (MARK_STARTED, Phase::Asked) => Some(Phase::Started),
            (MARK_REFUSED, Phase::Asked) => Some(Phase::Refused(value)),
            (MARK_EXITED, Phase::Started) => Some(Phase::Exited(value)),
            (MARK_EXITED, Phase::Orphaned) => None,
            _ => return,
        };
        match next {
            Some(next) => {
                state.shells.insert(id, next);
            }
            None => {
                state.shells.remove(&id);
            }
        }
        drop(state);
        self.changed.notify_all();
    }

    /// Bittet den Shim um eine Shell mit diesem Kommando an einem neuen
    /// Pseudoterminal der Größe `cols` mal `rows`.
    ///
    /// Blockiert, bis der Shim antwortet, höchstens fünf Sekunden.
    ///
    /// # Errors
    ///
    /// `TERM_002`: ein leeres oder zu langes Kommando, ein Argument mit NUL,
    /// schon [`MAX_SHELLS`] Shells, eine beendete Sandbox, ein
    /// Pseudoterminal, das sich nicht öffnen lässt, oder ein Shim, der
    /// ablehnt oder nicht antwortet.
    pub fn open(
        self: &Arc<Self>,
        command: &[String],
        cols: u16,
        rows: u16,
    ) -> Result<Shell, Diagnostic> {
        let body = encode_command(command)?;
        let id = {
            let mut state = lock(&self.state);
            if state.closed {
                return Err(refused("the sandbox has ended"));
            }
            let live = state
                .shells
                .values()
                .filter(|phase| matches!(phase, Phase::Asked | Phase::Started | Phase::Orphaned))
                .count();
            if live >= MAX_SHELLS {
                return Err(limit_reached());
            }
            let id = state.next_id;
            state.next_id = state.next_id.wrapping_add(1).max(1);
            state.shells.insert(id, Phase::Asked);
            id
        };
        let asked = match &self.door {
            Door::Socket(socket) => self.ask(socket, id, &body, cols, rows),
            Door::Spawn(spec) => self.spawn(spec, id, command, cols, rows),
        };
        match asked {
            Ok(master) => Ok(Shell {
                id,
                master,
                door: Arc::clone(self),
            }),
            Err(diagnostic) => {
                let mut state = lock(&self.state);
                // Hat der Shim doch noch gestartet, zählt der Prozess weiter,
                // bis sein Ende gemeldet ist.
                if state.shells.get(&id) == Some(&Phase::Started) {
                    state.shells.insert(id, Phase::Orphaned);
                } else {
                    state.shells.remove(&id);
                }
                Err(diagnostic)
            }
        }
    }

    /// Startet die Shell selbst, unter demselben Profil (macOS, HUM-323).
    fn spawn(
        self: &Arc<Self>,
        spec: &SpawnSpec,
        id: u32,
        command: &[String],
        cols: u16,
        rows: u16,
    ) -> Result<OwnedFd, Diagnostic> {
        let pty = crate::bwrap::open_pty(cols, rows)?;
        let (stdin, stdout, stderr) = pty.stdio()?;
        let mut child = std::process::Command::new(&spec.program)
            .args(&spec.args)
            .arg("--")
            .args(command)
            .env_clear()
            .envs(spec.env.iter().map(|(key, value)| (key, value)))
            .stdin(stdin)
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .map_err(|err| refused(&format!("cannot start {}: {err}", spec.program.display())))?;
        drop(pty.slave);
        drop(pty.reader);
        lock(&self.children).insert(id, child.id());
        let ended = {
            let mut state = lock(&self.state);
            state.shells.insert(id, Phase::Started);
            state.closed
        };
        // Endete die Sandbox zwischen der Prüfung in `open` und hier, hat
        // `close` diese Shell nicht mehr gesehen (Review HUM-323).
        if ended {
            let _ = child.kill();
        }
        self.changed.notify_all();
        let door = Arc::clone(self);
        thread::Builder::new()
            .name(format!("sandbox-shell-{id}"))
            .spawn(move || {
                let code = child.wait().map_or(ENDED_WITH_SANDBOX, |status| {
                    status
                        .code()
                        .or_else(|| status.signal().map(|signal| 128 + signal))
                        .unwrap_or(ENDED_WITH_SANDBOX)
                });
                lock(&door.children).remove(&id);
                let mut answer = [MARK_EXITED, 0, 0, 0, 0, 0, 0, 0, 0];
                answer[1..5].copy_from_slice(&id.to_le_bytes());
                answer[5..9].copy_from_slice(&code.to_le_bytes());
                door.apply(&answer);
            })
            .map_err(|err| refused(&format!("cannot watch the shell: {err}")))?;
        Ok(pty.master)
    }

    fn ask(
        &self,
        socket: &OwnedFd,
        id: u32,
        body: &[u8],
        cols: u16,
        rows: u16,
    ) -> Result<OwnedFd, Diagnostic> {
        let pty = crate::bwrap::open_pty(cols, rows)?;
        let mut message = Vec::with_capacity(5 + body.len());
        message.push(MARK_SPAWN);
        message.extend_from_slice(&id.to_le_bytes());
        message.extend_from_slice(body);
        {
            let follower = pty.slave.as_fd();
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut control = SendAncillaryBuffer::new(&mut space);
            let fds = [follower];
            if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
                return Err(refused("cannot attach the terminal to the request"));
            }
            sendmsg(
                socket,
                &[IoSlice::new(&message)],
                &mut control,
                crate::platform::SEND_NO_SIGNAL,
            )
            .map_err(|err| refused(&format!("the sandbox does not take the request: {err}")))?;
        }
        // Die eigene Kopie der Untertanenseite geht sofort zu: Sonst sähe die
        // Herrscherseite nie das Ende der Shell.
        drop(pty.slave);
        drop(pty.reader);
        let deadline = Instant::now() + ANSWER_WAIT;
        let mut state = lock(&self.state);
        loop {
            match state.shells.get(&id).copied() {
                Some(Phase::Started | Phase::Exited(_)) => return Ok(pty.master),
                Some(Phase::Refused(code)) => return Err(refusal(code)),
                _ if state.closed => return Err(refused("the sandbox has ended")),
                _ => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(refused(&format!(
                    "the sandbox did not answer within {ANSWER_WAIT:?}"
                )));
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Ob diese Shell beendet ist oder die Sandbox.
    fn ended(&self, id: u32) -> bool {
        let state = lock(&self.state);
        state.closed || matches!(state.shells.get(&id), Some(Phase::Exited(_)))
    }

    /// Der Exit-Code dieser Shell; wartet höchstens `timeout` auf die Meldung.
    /// Ohne Meldung, weil die Sandbox endete: [`ENDED_WITH_SANDBOX`].
    fn wait_exit(&self, id: u32, timeout: Duration) -> i32 {
        let deadline = Instant::now() + timeout;
        let mut state = lock(&self.state);
        loop {
            if let Some(Phase::Exited(code)) = state.shells.get(&id) {
                return *code;
            }
            let now = Instant::now();
            if state.closed || now >= deadline {
                return ENDED_WITH_SANDBOX;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Der [`Shell`] ist fort.
    fn release(&self, id: u32) {
        let mut state = lock(&self.state);
        match state.shells.get(&id) {
            Some(Phase::Started) => {
                state.shells.insert(id, Phase::Orphaned);
            }
            Some(_) => {
                state.shells.remove(&id);
            }
            None => {}
        }
    }

    /// Wie viele Shells gegen die Grenze zählen; für Tests.
    #[must_use]
    pub fn live(&self) -> usize {
        lock(&self.state)
            .shells
            .values()
            .filter(|phase| matches!(phase, Phase::Asked | Phase::Started | Phase::Orphaned))
            .count()
    }
}

/// Das Kommando im Format des Shims: jedes Argument mit NUL dahinter.
fn encode_command(command: &[String]) -> Result<Vec<u8>, Diagnostic> {
    if command.first().is_none_or(String::is_empty) {
        return Err(refused("the command is empty"));
    }
    if command.len() > MAX_ARGS {
        return Err(refused(&format!(
            "the command has {} arguments; at most {MAX_ARGS} are allowed",
            command.len()
        )));
    }
    let mut body = Vec::new();
    for arg in command {
        if arg.contains('\0') {
            return Err(refused("an argument contains a NUL byte"));
        }
        body.extend_from_slice(arg.as_bytes());
        body.push(0);
    }
    if body.len() + 5 > MAX_REQUEST {
        return Err(refused(&format!(
            "the command is {} bytes long; at most {} are allowed",
            body.len(),
            MAX_REQUEST - 5
        )));
    }
    Ok(body)
}

fn refused(why: &str) -> Diagnostic {
    Diagnostic::builder(TERM_002, Severity::Error)
        .why(format!("cannot open another shell in the sandbox: {why}"))
        .build()
}

fn limit_reached() -> Diagnostic {
    refused(&format!(
        "{MAX_SHELLS} further shells are already open; leave one (exit) and try again"
    ))
}

fn refusal(code: i32) -> Diagnostic {
    match code {
        REFUSED_LIMIT => limit_reached(),
        REFUSED_MALFORMED => refused("the sandbox did not understand the request"),
        errno => refused(&format!(
            "the sandbox could not start it: {}",
            std::io::Error::from_raw_os_error(errno)
        )),
    }
}

/// Eine weitere Shell, solange sie lebt.
///
/// Ihr Pseudoterminal gehört diesem Wert. Fällt er, geht die Herrscherseite
/// zu, und was in der Shell noch läuft, bekommt ein Auflegen.
#[derive(Debug)]
pub struct Shell {
    id: u32,
    master: OwnedFd,
    door: Arc<ShellDoor>,
}

impl Shell {
    /// Die Kennung dieser Shell in ihrer Sandbox, ab 1.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Schreibt Tastendrücke in die Shell. Blockiert, wenn sie nicht liest.
    ///
    /// # Errors
    ///
    /// `TERM_002`, wenn das Terminal nichts mehr annimmt.
    pub fn write_input(&self, bytes: &[u8]) -> Result<(), Diagnostic> {
        let mut rest = bytes;
        while !rest.is_empty() {
            match rustix::io::write(&self.master, rest) {
                Ok(0) => return Err(gone("the shell accepted no more input")),
                Ok(written) => rest = &rest[written.min(rest.len())..],
                Err(rustix::io::Errno::INTR) => {}
                Err(err) => return Err(gone(&format!("cannot write to the shell: {err}"))),
            }
        }
        Ok(())
    }

    /// Setzt die Geometrie. Die Shell trägt ihr Terminal als steuerndes, der
    /// Kernel schickt ihr das `SIGWINCH` selbst.
    ///
    /// # Errors
    ///
    /// `TERM_002`, wenn der Kernel die Größe nicht annimmt.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Diagnostic> {
        tcsetwinsize(
            &self.master,
            Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .map_err(|err| gone(&format!("cannot set the shell to {cols}x{rows}: {err}")))
    }

    /// Liest die Ausgabe, bis die Shell endet, und gibt ihren Exit-Code
    /// zurück. Blockiert; gehört auf einen eigenen Faden.
    ///
    /// Endet die Shell, wird noch kurz (300 ms) gelesen, was sie zuletzt
    /// schrieb; danach endet das Lesen auch dann, wenn ein Hintergrundprozess
    /// das Terminal noch offen hält. Endet die Sandbox, endet es ebenso.
    pub fn pump(&self, mut sink: impl FnMut(&[u8])) -> i32 {
        let mut buffer = [0u8; 8192];
        let mut drain_until: Option<Instant> = None;
        let slice = Timespec {
            tv_sec: 0,
            tv_nsec: 100_000_000,
        };
        loop {
            if drain_until.is_none() && self.door.ended(self.id) {
                drain_until = Some(Instant::now() + DRAIN);
            }
            if drain_until.is_some_and(|until| Instant::now() >= until) {
                break;
            }
            let mut fds = [PollFd::new(&self.master, PollFlags::IN)];
            match poll(&mut fds, Some(&slice)) {
                Ok(0) | Err(rustix::io::Errno::INTR) => continue,
                Ok(_) => {}
                Err(_) => break,
            }
            match rustix::io::read(&self.master, &mut buffer[..]) {
                Ok(0) => break,
                Ok(n) => sink(&buffer[..n]),
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
                // `EIO`: Niemand hält die Untertanenseite mehr.
                Err(_) => break,
            }
        }
        self.door.wait_exit(self.id, EXIT_WAIT)
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.door.release(self.id);
    }
}

fn gone(why: &str) -> Diagnostic {
    Diagnostic::builder(TERM_002, Severity::Error)
        .why(why.to_owned())
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    // Alle Tests bis auf den letzten fahren `SOCK_SEQPACKET` mit `SCM_RIGHTS`,
    // die Tür des Linux-Shims (HUM-299); unter macOS gibt es sie nicht
    // (HUM-312).
    #![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]

    use super::*;
    #[cfg(target_os = "linux")]
    use rustix::net::SendFlags;

    fn answer(mark: u8, id: u32, value: i32) -> [u8; 9] {
        let id = id.to_le_bytes();
        let value = value.to_le_bytes();
        [
            mark, id[0], id[1], id[2], id[3], value[0], value[1], value[2], value[3],
        ]
    }

    #[cfg(target_os = "linux")]
    /// A fake shim on the other end of the pair: answers every request with
    /// `reply` and keeps the received terminal descriptors.
    fn fake_shim(
        shim: OwnedFd,
        reply: impl Fn(u32) -> Vec<[u8; 9]> + Send + 'static,
    ) -> std::sync::mpsc::Receiver<(u32, Vec<String>)> {
        let (seen, requests) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut buffer = vec![0u8; MAX_REQUEST];
            loop {
                let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
                let mut control = rustix::net::RecvAncillaryBuffer::new(&mut space);
                let mut iov = [std::io::IoSliceMut::new(&mut buffer)];
                let Ok(msg) = rustix::net::recvmsg(
                    &shim,
                    &mut iov,
                    &mut control,
                    rustix::net::RecvFlags::CMSG_CLOEXEC,
                ) else {
                    break;
                };
                if msg.bytes == 0 {
                    break;
                }
                let fds: usize = control
                    .drain()
                    .map(|message| match message {
                        rustix::net::RecvAncillaryMessage::ScmRights(fds) => fds.count(),
                        _ => 0,
                    })
                    .sum();
                assert_eq!(fds, 1, "every request carries one terminal");
                let id = u32::from_le_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]);
                let args = buffer[5..msg.bytes - 1]
                    .split(|byte| *byte == 0)
                    .map(|part| String::from_utf8_lossy(part).into_owned())
                    .collect();
                let _ = seen.send((id, args));
                for message in reply(id) {
                    rustix::net::send(&shim, &message, SendFlags::NOSIGNAL).unwrap();
                }
            }
        });
        requests
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_started_shell_carries_its_command_and_its_exit_code() {
        let (daemon, shim) = pair().unwrap();
        let shim = fake_shim(shim, |id| {
            vec![answer(MARK_STARTED, id, 42), answer(MARK_EXITED, id, 3)]
        });
        let door = ShellDoor::start(daemon).unwrap();
        let shell = door
            .open(&["bash".to_owned(), "-i".to_owned()], 80, 24)
            .unwrap();
        assert_eq!(shell.id(), 1);
        assert_eq!(shell.pump(|_| {}), 3);
        drop(shell);
        assert_eq!(door.live(), 0);
        let seen = shim.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(seen, (1, vec!["bash".to_owned(), "-i".to_owned()]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_door_counts_to_four_and_no_further() {
        let (daemon, shim) = pair().unwrap();
        let _shim = fake_shim(shim, |id| vec![answer(MARK_STARTED, id, 10)]);
        let door = ShellDoor::start(daemon).unwrap();
        let shells: Vec<Shell> = (0..MAX_SHELLS)
            .map(|_| door.open(&["sh".to_owned()], 80, 24).unwrap())
            .collect();
        let refused = door.open(&["sh".to_owned()], 80, 24).unwrap_err();
        assert_eq!(refused.code, TERM_002);
        assert!(refused.why.contains("4 further shells"), "{}", refused.why);
        // A dropped shell whose process still runs keeps counting.
        drop(shells);
        assert_eq!(door.live(), MAX_SHELLS);
        assert!(door.open(&["sh".to_owned()], 80, 24).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_refusal_of_the_shim_is_a_diagnostic() {
        let (daemon, shim) = pair().unwrap();
        let _shim = fake_shim(shim, |id| vec![answer(MARK_REFUSED, id, REFUSED_LIMIT)]);
        let door = ShellDoor::start(daemon).unwrap();
        let err = door.open(&["sh".to_owned()], 80, 24).unwrap_err();
        assert!(err.why.contains("already open"), "{}", err.why);
        assert_eq!(door.live(), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn answers_for_ids_nobody_asked_for_change_nothing() {
        let (daemon, shim) = pair().unwrap();
        let _shim = fake_shim(shim, |id| {
            vec![
                answer(MARK_EXITED, id + 7, 0),
                answer(MARK_STARTED, id + 1, 0),
                answer(MARK_STARTED, id, 5),
            ]
        });
        let door = ShellDoor::start(daemon).unwrap();
        let shell = door.open(&["sh".to_owned()], 80, 24).unwrap();
        assert_eq!(door.live(), 1);
        drop(shell);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_shell_ends_with_its_sandbox() {
        let (daemon, shim) = pair().unwrap();
        let door = ShellDoor::start(daemon).unwrap();
        // A shim that starts the shell and then ends, as the sandbox does:
        // its side of the pair closes without an `E` for the shell.
        let gone = thread::spawn(move || {
            let mut buffer = vec![0u8; MAX_REQUEST];
            let (n, _) = recv(&shim, &mut buffer[..], RecvFlags::empty()).unwrap();
            assert!(n > 5);
            let id = u32::from_le_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]);
            rustix::net::send(&shim, &answer(MARK_STARTED, id, 10), SendFlags::NOSIGNAL).unwrap();
        });
        let shell = door.open(&["sh".to_owned()], 80, 24).unwrap();
        gone.join().unwrap();
        let started = Instant::now();
        assert_eq!(shell.pump(|_| {}), ENDED_WITH_SANDBOX);
        assert!(started.elapsed() < EXIT_WAIT, "{:?}", started.elapsed());
        let err = door.open(&["sh".to_owned()], 80, 24).unwrap_err();
        assert!(err.why.contains("the sandbox has ended"), "{}", err.why);
    }

    #[test]
    fn a_bad_command_never_reaches_the_shim() {
        assert!(encode_command(&[]).is_err());
        assert!(encode_command(&[String::new()]).is_err());
        assert!(encode_command(&["a\0b".to_owned()]).is_err());
        let many = vec!["x".to_owned(); MAX_ARGS + 1];
        assert!(encode_command(&many).is_err());
        assert!(encode_command(&["x".repeat(MAX_REQUEST)]).is_err());
        assert_eq!(
            encode_command(&["sh".to_owned(), "-c".to_owned()]).unwrap(),
            b"sh\0-c\0"
        );
    }
}
