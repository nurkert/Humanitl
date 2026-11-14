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
//! # Der Strom-Modus (HUM-443)
//!
//! In einer VM überquert kein Deskriptor die Grenze. Dort, und unter bwrap,
//! wenn der Aufrufer es verlangt ([`crate::TerminalTransport::Stream`]), ist
//! die Tür ein Strom-Socket: Der Shim öffnet die Pseudoterminals selbst, und
//! Terminal-Bytes, Größen, Shells, Exit-Codes, Unterbrechungen und
//! Lebenszeichen gehen als Rahmen hin und her ([`crate::control_frame`], dieselbe
//! Datei wie im Shim). Der Lese-Faden dieser Tür liest jeden Rahmen an einer
//! Stelle, verwirft übergroße Rahmen, unbekannte Arten und unbekannte
//! Kennungen und gibt die Bytes des Agenten (Kanal 0) an die Ausgabe der
//! Sandbox, die einer Shell an ihren [`Shell`]. Über diesen Kanal geht nie
//! ein Byte an den Proxy oder von ihm.
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
use std::sync::mpsc;
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

use crate::control_frame::{self, AGENT, Decoder, Event, Kind, MAX_PAYLOAD, Message};

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

/// Wie weitere Shells starten: über die Tür des Shims (Linux), als eigener
/// Prozess unter demselben Profil (macOS, HUM-323) oder über den Steuerkanal
/// als Strom (HUM-443).
#[derive(Debug)]
enum Door {
    Socket(OwnedFd),
    Spawn(SpawnSpec),
    Stream(StreamDoor),
}

/// Das Recht, allein auf den Strom zu schreiben; gibt es beim Fallen frei.
struct Turn<'a>(&'a StreamDoor);

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *lock(&self.0.write) = false;
        self.0.writable.notify_one();
    }
}

/// Wie viele Stücke Ausgabe einer Shell höchstens warten, bis ihr Leser sie
/// nimmt; je Stück höchstens [`MAX_PAYLOAD`] Bytes. Was darüber kommt, wird
/// verworfen und gezählt: Der eine Leser des Stroms darf an keiner Shell
/// hängen, und ein Gast darf den Speicher des Daemons nicht füllen.
const SHELL_QUEUE: usize = 256;

/// Wie lange ein Rahmen an den Shim höchstens auf Platz im Strom wartet.
const STREAM_SEND_WAIT: Duration = Duration::from_secs(10);

/// Wohin die Bytes des Agenten aus dem Strom gehen.
pub(crate) type AgentSink = Box<dyn Fn(&[u8]) + Send>;

/// Die Seite des Daemons am Steuerkanal als Strom (HUM-443).
#[derive(Debug)]
struct StreamDoor {
    /// Die Seite des Daemons; gelesen blockierend, geschrieben ohne zu
    /// blockieren. Es schreibt immer nur ein Faden ([`StreamDoor::write`]),
    /// damit sich zwei Rahmen nie mischen.
    socket: OwnedFd,
    /// Ob gerade ein Faden schreibt.
    write: Mutex<bool>,
    /// Weckt den nächsten Schreiber.
    writable: Condvar,
    /// Die Ausgabe jeder offenen Shell, je Kennung.
    outputs: Mutex<BTreeMap<u32, mpsc::SyncSender<Vec<u8>>>>,
    /// Stücke einer Shell, die verworfen wurden, weil niemand ihre volle
    /// Schlange las ([`SHELL_QUEUE`]).
    dropped: std::sync::atomic::AtomicU64,
    /// Das letzte Lebenszeichen des Shims.
    heartbeat: Mutex<Option<Instant>>,
    /// Der Exit-Code des Agenten, wie der Shim ihn meldet.
    agent_exit: Mutex<Option<i32>>,
}

impl StreamDoor {
    /// Schreibt fertige Rahmen am Stück.
    ///
    /// Nimmt der Shim sie nicht binnen [`STREAM_SEND_WAIT`], oder scheitert
    /// das Schreiben sonst, schließt der Daemon den Strom in beide
    /// Richtungen: Ein halb geschriebener Rahmen brächte den Leser des Shims
    /// aus dem Tritt, und ein Shim, der nicht mehr liest (Root im Gast kann
    /// ihn anhalten), hielte sonst den Faden des Daemons für immer fest. Das
    /// Ende des Stroms ist die Lebensleine; die Sitzung endet, sonst nichts.
    fn send(&self, frames: &[u8]) -> std::io::Result<()> {
        self.send_within(frames, STREAM_SEND_WAIT)
    }

    /// [`StreamDoor::send`] mit einer festen Frist für alle Rahmen zusammen:
    /// Geschrieben wird ohne zu blockieren, gewartet mit `poll` auf die
    /// Restzeit. Ein Shim, der vor jedem Ablauf ein Byte nimmt, hält den
    /// Faden so nicht länger als `wait` fest (Review HUM-443).
    fn send_within(&self, frames: &[u8], wait: Duration) -> std::io::Result<()> {
        // Die Frist beginnt vor dem Warten auf den Schreiber davor: Wer
        // hinter einem langsamen Rahmen ansteht, wartet insgesamt nicht
        // länger als `wait` (Review HUM-443).
        let deadline = Instant::now() + wait;
        let Some(_turn) = self.turn(deadline) else {
            let _ = rustix::net::shutdown(&self.socket, rustix::net::Shutdown::Both);
            return Err(std::io::ErrorKind::TimedOut.into());
        };
        let mut rest = frames;
        let flags = crate::platform::SEND_NO_SIGNAL | rustix::net::SendFlags::DONTWAIT;
        while !rest.is_empty() {
            // Jeder Versuch prüft die Frist zuerst: Ein Rahmen, der erst nach
            // ihr ganz hinausginge, gilt als nicht gesendet (Review HUM-443).
            if Instant::now() >= deadline {
                let _ = rustix::net::shutdown(&self.socket, rustix::net::Shutdown::Both);
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            let failed = match rustix::net::send(&self.socket, rest, flags) {
                Ok(0) => std::io::ErrorKind::WriteZero.into(),
                Ok(sent) => {
                    rest = &rest[sent.min(rest.len())..];
                    continue;
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(rustix::io::Errno::AGAIN) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        std::io::ErrorKind::TimedOut.into()
                    } else {
                        let left = Timespec {
                            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
                            tv_nsec: i64::from(left.subsec_nanos()),
                        };
                        let mut fds = [PollFd::new(&self.socket, PollFlags::OUT)];
                        match poll(&mut fds, Some(&left)) {
                            Ok(_) | Err(rustix::io::Errno::INTR) => continue,
                            Err(err) => std::io::Error::from(err),
                        }
                    }
                }
                Err(err) => std::io::Error::from(err),
            };
            let _ = rustix::net::shutdown(&self.socket, rustix::net::Shutdown::Both);
            return Err(failed);
        }
        Ok(())
    }

    /// Wartet bis `deadline`, bis kein anderer Faden schreibt, und schreibt
    /// dann allein, bis der zurückgegebene Wert fällt; `None` nach Ablauf.
    fn turn(&self, deadline: Instant) -> Option<Turn<'_>> {
        let mut busy = lock(&self.write);
        while *busy {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            busy = self
                .writable
                .wait_timeout(busy, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        // Auch wer das Recht erst nach Ablauf bekäme, bekommt es nicht.
        if Instant::now() >= deadline {
            return None;
        }
        *busy = true;
        Some(Turn(self))
    }

    fn send_data(&self, channel: u32, bytes: &[u8]) -> std::io::Result<()> {
        let mut frames = Vec::with_capacity(bytes.len() + control_frame::HEADER);
        control_frame::encode_data(channel, bytes, &mut frames);
        self.send(&frames)
    }

    fn send_size(&self, channel: u32, cols: u16, rows: u16) -> std::io::Result<()> {
        let mut frames = Vec::new();
        control_frame::encode_size(channel, cols, rows, &mut frames);
        self.send(&frames)
    }

    fn send_empty(&self, kind: Kind, channel: u32) -> std::io::Result<()> {
        let mut frames = Vec::new();
        let _ = control_frame::encode(kind, channel, &[], &mut frames);
        self.send(&frames)
    }
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
    /// Die Seite des Daemons am Steuerkanal als Strom (HUM-443); sie trägt
    /// auch das Terminal des Agenten.
    Stream(OwnedFd),
}

impl DoorSeed {
    /// Ob dies der Strom-Modus ist: Dann trägt die Tür das Terminal des
    /// Agenten, und die Sandbox bekommt kein Pseudoterminal des Daemons.
    pub(crate) const fn is_stream(&self) -> bool {
        matches!(self, Self::Stream(_))
    }

    /// Die Tür dazu. `agent` bekommt im Strom-Modus die Bytes des Agenten.
    ///
    /// # Errors
    ///
    /// Der Fehler beim Start des Lese-Fadens einer Socket- oder Strom-Tür.
    pub(crate) fn open(self, agent: AgentSink) -> std::io::Result<Arc<ShellDoor>> {
        match self {
            Self::Socket(socket) => ShellDoor::start(socket),
            Self::Spawn(spec) => Ok(ShellDoor::spawning(spec)),
            Self::Stream(socket) => ShellDoor::start_stream(socket, agent),
        }
    }
}

/// Das Paar für den Strom-Modus unter bwrap: `(Seite des Daemons, Seite des
/// Shims)`, beide mit `CLOEXEC`. Auf der Seite des Daemons liegt schon der
/// erste Rahmen, die Größe des Terminals des Agenten; der Shim wartet kurz
/// darauf, damit der Agent mit ihr startet.
///
/// # Errors
///
/// Der Fehler von `socketpair(2)` oder des ersten Schreibens.
pub(crate) fn stream_pair(cols: u16, rows: u16) -> std::io::Result<(OwnedFd, OwnedFd)> {
    let (daemon, shim) = socketpair(AddressFamily::UNIX, SocketType::STREAM, PAIR_FLAGS, None)
        .map_err(std::io::Error::from)?;
    crate::platform::set_cloexec(&daemon)?;
    crate::platform::set_cloexec(&shim)?;
    let mut size = Vec::new();
    control_frame::encode_size(AGENT, cols, rows, &mut size);
    let mut rest = size.as_slice();
    while !rest.is_empty() {
        let sent = rustix::net::send(&daemon, rest, crate::platform::SEND_NO_SIGNAL)?;
        rest = &rest[sent.min(rest.len())..];
    }
    Ok((daemon, shim))
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

    /// Nimmt die Seite des Daemons am Steuerkanal als Strom (HUM-443) und
    /// liest jeden Rahmen auf einem eigenen Faden, bis die Seite des Shims zu
    /// ist; die Bytes des Agenten gehen an `agent`.
    ///
    /// # Errors
    ///
    /// Der Faden ließ sich nicht starten.
    pub(crate) fn start_stream(socket: OwnedFd, agent: AgentSink) -> std::io::Result<Arc<Self>> {
        let door = Arc::new(Self {
            door: Door::Stream(StreamDoor {
                socket,
                write: Mutex::new(false),
                writable: Condvar::new(),
                outputs: Mutex::new(BTreeMap::new()),
                dropped: std::sync::atomic::AtomicU64::new(0),
                heartbeat: Mutex::new(None),
                agent_exit: Mutex::new(None),
            }),
            state: Mutex::new(DoorState {
                next_id: 1,
                ..DoorState::default()
            }),
            changed: Condvar::new(),
            children: Mutex::new(BTreeMap::new()),
        });
        let reader = Arc::clone(&door);
        thread::Builder::new()
            .name("sandbox-stream".to_owned())
            .spawn(move || reader.read_stream(&agent))?;
        Ok(door)
    }

    /// Liest den Strom, bis die Seite des Shims zu ist. Die eine Stelle, an
    /// der Rahmen aus der Sandbox gelesen werden: Der Leser behält nie mehr
    /// als einen Rahmen ([`control_frame::Decoder`]), und was keine bekannte
    /// Art, keine erwartete Form oder keine offene Kennung hat, ändert
    /// nichts.
    fn read_stream(&self, agent: &AgentSink) {
        let Door::Stream(stream) = &self.door else {
            return;
        };
        let mut decoder = Decoder::new();
        let mut buffer = vec![0u8; MAX_PAYLOAD];
        loop {
            match rustix::io::read(&stream.socket, &mut buffer[..]) {
                Ok(0) => break,
                // Jeder Rahmen wird verarbeitet, sobald er vollständig ist:
                // Der Leser hält nie mehr als den einen, den er sammelt.
                Ok(n) => decoder.push(&buffer[..n], |event| {
                    if let Event::Frame(frame) = event
                        && let Ok(message) = Message::parse(&frame)
                    {
                        self.take(stream, message, agent);
                    }
                }),
                Err(rustix::io::Errno::INTR) => {}
                Err(_) => break,
            }
        }
        // Die Shells sehen das Ende ihrer Ausgabe, wer wartet, das der Tür.
        lock(&stream.outputs).clear();
        lock(&self.state).closed = true;
        self.changed.notify_all();
    }

    /// Ein Rahmen des Shims. Was in die andere Richtung gehört, ist hier
    /// keine Nachricht und wird verworfen.
    fn take(&self, stream: &StreamDoor, message: Message, agent: &AgentSink) {
        match message {
            Message::Data { channel, bytes } if channel == AGENT => agent(&bytes),
            Message::Data { channel, bytes } => {
                if let Some(output) = lock(&stream.outputs).get(&channel)
                    && let Err(mpsc::TrySendError::Full(_)) = output.try_send(bytes)
                {
                    stream
                        .dropped
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Message::Exit { channel, code } if channel == AGENT => {
                *lock(&stream.agent_exit) = Some(code);
            }
            Message::Started {
                channel,
                pid: value,
            }
            | Message::Refused {
                channel,
                code: value,
            }
            | Message::Exit {
                channel,
                code: value,
            } => {
                let mark = match message {
                    Message::Started { .. } => MARK_STARTED,
                    Message::Refused { .. } => MARK_REFUSED,
                    _ => MARK_EXITED,
                };
                let mut answer = [mark, 0, 0, 0, 0, 0, 0, 0, 0];
                answer[1..5].copy_from_slice(&channel.to_le_bytes());
                answer[5..9].copy_from_slice(&value.to_le_bytes());
                // Nach dem Ende kommt nichts mehr; die Shell liest noch, was
                // vorher kam, und sieht dann das Ende des Kanals. Nur wenn
                // das Ende angenommen wurde: Ein `E` vor dem `R` ändert
                // nichts, auch nicht den Kanal der Ausgabe.
                if self.apply(&answer) && mark == MARK_EXITED {
                    lock(&stream.outputs).remove(&channel);
                }
            }
            Message::Heartbeat => *lock(&stream.heartbeat) = Some(Instant::now()),
            Message::Size { .. }
            | Message::Open { .. }
            | Message::Close { .. }
            | Message::Interrupt { .. } => {}
        }
    }

    /// Wann der Shim zuletzt ein Lebenszeichen über den Strom gab; `None`
    /// ohne Strom oder vor dem ersten (HUM-443).
    #[must_use]
    pub fn last_heartbeat(&self) -> Option<Instant> {
        match &self.door {
            Door::Stream(stream) => *lock(&stream.heartbeat),
            Door::Socket(_) | Door::Spawn(_) => None,
        }
    }

    /// Der Exit-Code des Agenten, wie der Shim ihn über den Strom meldete;
    /// `None` ohne Strom oder vor seinem Ende (HUM-443).
    #[must_use]
    pub fn agent_exit(&self) -> Option<i32> {
        match &self.door {
            Door::Stream(stream) => *lock(&stream.agent_exit),
            Door::Socket(_) | Door::Spawn(_) => None,
        }
    }

    /// Legt den Strom auf, als wäre der Daemon fort; nur für Tests der
    /// Lebensleine im Gast (HUM-443).
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn hang_up(&self) {
        if let Door::Stream(stream) = &self.door {
            let _ = rustix::net::shutdown(&stream.socket, rustix::net::Shutdown::Both);
        }
    }

    /// Ob diese Tür der Strom ist, der auch das Terminal des Agenten trägt.
    pub(crate) const fn is_stream(&self) -> bool {
        matches!(self.door, Door::Stream(_))
    }

    /// Bytes an das Terminal des Agenten (Strom-Modus).
    pub(crate) fn agent_input(&self, bytes: &[u8]) -> std::io::Result<()> {
        self.stream()?.send_data(AGENT, bytes)
    }

    /// Die Größe des Terminals des Agenten (Strom-Modus).
    pub(crate) fn agent_size(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.stream()?.send_size(AGENT, cols, rows)
    }

    /// `SIGINT` an die Vordergrundgruppe des Agenten, vom Shim aus
    /// (Strom-Modus): Ein Signal des Hosts erreicht keinen Gast.
    pub(crate) fn agent_interrupt(&self) -> std::io::Result<()> {
        self.stream()?.send_empty(Kind::Interrupt, AGENT)
    }

    fn stream(&self) -> std::io::Result<&StreamDoor> {
        match &self.door {
            Door::Stream(stream) => Ok(stream),
            Door::Socket(_) | Door::Spawn(_) => Err(std::io::ErrorKind::Unsupported.into()),
        }
    }

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
    /// `true`, wenn die Antwort einen Übergang auslöste.
    fn apply(&self, answer: &[u8]) -> bool {
        let id = u32::from_le_bytes([answer[1], answer[2], answer[3], answer[4]]);
        let value = i32::from_le_bytes([answer[5], answer[6], answer[7], answer[8]]);
        let mut state = lock(&self.state);
        let Some(phase) = state.shells.get(&id).copied() else {
            return false;
        };
        let next = match (answer[0], phase) {
            (MARK_STARTED, Phase::Asked) => Some(Phase::Started),
            (MARK_REFUSED, Phase::Asked) => Some(Phase::Refused(value)),
            (MARK_EXITED, Phase::Started) => Some(Phase::Exited(value)),
            (MARK_EXITED, Phase::Orphaned) => None,
            _ => return false,
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
        true
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
            Door::Socket(socket) => self
                .ask(socket, id, &body, cols, rows)
                .map(Terminal::Master),
            Door::Spawn(spec) => self
                .spawn(spec, id, command, cols, rows)
                .map(Terminal::Master),
            Door::Stream(stream) => self.ask_stream(stream, id, command, cols, rows),
        };
        match asked {
            Ok(terminal) => Ok(Shell {
                id,
                terminal,
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

    /// Bittet den Shim über den Strom um eine Shell (HUM-443). Der Kanal
    /// ihrer Ausgabe steht, bevor die Anfrage hinausgeht: Was die Shell
    /// sofort schreibt, geht nicht verloren.
    fn ask_stream(
        &self,
        stream: &StreamDoor,
        id: u32,
        command: &[String],
        cols: u16,
        rows: u16,
    ) -> Result<Terminal, Diagnostic> {
        let (output, receiver) = mpsc::sync_channel(SHELL_QUEUE);
        lock(&stream.outputs).insert(id, output);
        let argv: Vec<&[u8]> = command.iter().map(String::as_bytes).collect();
        let mut frames = Vec::new();
        let asked = control_frame::encode_open(id, cols, rows, &argv, &mut frames)
            .map_err(|control_frame::Malformed(why)| refused(why))
            .and_then(|()| {
                stream.send(&frames).map_err(|err| {
                    refused(&format!("the sandbox does not take the request: {err}"))
                })
            })
            .and_then(|()| self.wait_answer(id));
        match asked {
            Ok(()) => Ok(Terminal::Stream(Mutex::new(receiver))),
            Err(diagnostic) => {
                lock(&stream.outputs).remove(&id);
                Err(diagnostic)
            }
        }
    }

    /// Wartet höchstens [`ANSWER_WAIT`] auf `R` oder `F` für `id`.
    fn wait_answer(&self, id: u32) -> Result<(), Diagnostic> {
        let deadline = Instant::now() + ANSWER_WAIT;
        let mut state = lock(&self.state);
        loop {
            match state.shells.get(&id).copied() {
                Some(Phase::Started | Phase::Exited(_)) => return Ok(()),
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
        self.wait_answer(id).map(|()| pty.master)
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

/// Wo das Terminal einer Shell liegt.
#[derive(Debug)]
enum Terminal {
    /// Die Herrscherseite beim Daemon (Deskriptor-Modus).
    Master(OwnedFd),
    /// Im Shim; ihre Ausgabe kommt vom Lese-Faden des Stroms (HUM-443).
    Stream(Mutex<mpsc::Receiver<Vec<u8>>>),
}

/// Eine weitere Shell, solange sie lebt.
///
/// Ihr Pseudoterminal gehört diesem Wert. Fällt er, geht die Herrscherseite
/// zu, und was in der Shell noch läuft, bekommt ein Auflegen; im Strom-Modus
/// legt der Shim auf, wenn der Daemon `C` schickt.
#[derive(Debug)]
pub struct Shell {
    id: u32,
    terminal: Terminal,
    door: Arc<ShellDoor>,
}

impl Shell {
    /// Die Kennung dieser Shell in ihrer Sandbox, ab 1.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Ob ihr Terminal im Shim liegt und über den Strom reist (HUM-443).
    #[must_use]
    pub const fn streamed(&self) -> bool {
        matches!(self.terminal, Terminal::Stream(_))
    }

    /// Schreibt Tastendrücke in die Shell. Blockiert, wenn sie nicht liest.
    ///
    /// # Errors
    ///
    /// `TERM_002`, wenn das Terminal nichts mehr annimmt.
    pub fn write_input(&self, bytes: &[u8]) -> Result<(), Diagnostic> {
        let master = match &self.terminal {
            Terminal::Master(master) => master,
            Terminal::Stream(_) => {
                return self
                    .door
                    .stream()
                    .and_then(|stream| stream.send_data(self.id, bytes))
                    .map_err(|err| gone(&format!("cannot write to the shell: {err}")));
            }
        };
        let mut rest = bytes;
        while !rest.is_empty() {
            match rustix::io::write(master, rest) {
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
        let master = match &self.terminal {
            Terminal::Master(master) => master,
            Terminal::Stream(_) => {
                return self
                    .door
                    .stream()
                    .and_then(|stream| stream.send_size(self.id, cols, rows))
                    .map_err(|err| gone(&format!("cannot set the shell to {cols}x{rows}: {err}")));
            }
        };
        tcsetwinsize(
            master,
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
        let master = match &self.terminal {
            Terminal::Master(master) => master,
            Terminal::Stream(output) => return self.pump_stream(&lock(output), sink),
        };
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
            let mut fds = [PollFd::new(master, PollFlags::IN)];
            match poll(&mut fds, Some(&slice)) {
                Ok(0) | Err(rustix::io::Errno::INTR) => continue,
                Ok(_) => {}
                Err(_) => break,
            }
            match rustix::io::read(master, &mut buffer[..]) {
                Ok(0) => break,
                Ok(n) => sink(&buffer[..n]),
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
                // `EIO`: Niemand hält die Untertanenseite mehr.
                Err(_) => break,
            }
        }
        self.door.wait_exit(self.id, EXIT_WAIT)
    }

    /// [`Shell::pump`] im Strom-Modus: Die Bytes kommen in der Reihenfolge
    /// des Stroms, also vor dem `E` der Shell; endet sie oder die Sandbox,
    /// wird noch gelesen, was schon da ist.
    fn pump_stream(&self, output: &mpsc::Receiver<Vec<u8>>, mut sink: impl FnMut(&[u8])) -> i32 {
        loop {
            if self.door.ended(self.id) {
                while let Ok(bytes) = output.try_recv() {
                    sink(&bytes);
                }
                break;
            }
            match output.recv_timeout(Duration::from_millis(100)) {
                Ok(bytes) => sink(&bytes),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        self.door.wait_exit(self.id, EXIT_WAIT)
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        // Im Strom-Modus legt der Shim auf; wie beim Schließen der
        // Herrscherseite endet damit die Shell.
        if matches!(self.terminal, Terminal::Stream(_))
            && let Ok(stream) = self.door.stream()
        {
            let _ = stream.send_empty(Kind::Close, self.id);
            lock(&stream.outputs).remove(&self.id);
        }
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

    /// Der Strom-Modus von der Seite des Daemons (HUM-443): Ein feindlicher
    /// Shim schickt übergroße Rahmen, unbekannte Arten, Antworten für
    /// Kennungen, nach denen niemand fragte, und Rahmen der Gegenrichtung.
    /// Nichts davon ändert etwas; die Bytes des Agenten und die der Shell
    /// kommen an, und das `E` der Shell beendet ihr Lesen mit ihrem Code.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_stream_drops_what_it_does_not_expect_and_carries_the_rest() {
        let (daemon, shim) =
            socketpair(AddressFamily::UNIX, SocketType::STREAM, PAIR_FLAGS, None).unwrap();
        let agent = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&agent);
        let door =
            ShellDoor::start_stream(daemon, Box::new(move |bytes| lock(&seen).extend(bytes)))
                .unwrap();
        let push = |bytes: &[u8]| {
            let mut rest = bytes;
            while !rest.is_empty() {
                let taken = rustix::net::send(&shim, rest, SendFlags::NOSIGNAL).unwrap();
                rest = &rest[taken..];
            }
        };
        let mut wire = Vec::new();
        // Übergroß, und die angekündigten Bytes folgen wirklich.
        wire.push(b'D');
        wire.extend_from_slice(&0u32.to_le_bytes());
        wire.extend_from_slice(&u32::try_from(3 * MAX_PAYLOAD).unwrap().to_le_bytes());
        wire.extend(std::iter::repeat_n(b'z', 3 * MAX_PAYLOAD));
        wire.extend_from_slice(&[b'?', 1, 0, 0, 0, 2, 0, 0, 0, b'x', b'y']);
        control_frame::encode_value(Kind::Started, 9, 4, &mut wire);
        control_frame::encode_value(Kind::Exit, 9, 0, &mut wire);
        control_frame::encode_data(9, b"nobody", &mut wire);
        control_frame::encode_size(AGENT, 1, 1, &mut wire);
        control_frame::encode(Kind::Started, 1, b"abc", &mut wire).unwrap();
        control_frame::encode_data(AGENT, b"agent-bytes", &mut wire);
        control_frame::encode(Kind::Heartbeat, AGENT, &[], &mut wire).unwrap();
        push(&wire);

        // Die Shell: Der Shim liest das `O` und antwortet.
        let answering = thread::spawn(move || {
            let mut decoder = Decoder::new();
            let mut buffer = [0u8; 4096];
            let mut open = None;
            while open.is_none() {
                let n = rustix::io::read(&shim, &mut buffer[..]).unwrap();
                assert!(n > 0, "the daemon sends an open");
                decoder.push(&buffer[..n], |event| {
                    if let Event::Frame(frame) = event
                        && let Ok(Message::Open { channel, argv, .. }) = Message::parse(&frame)
                    {
                        open = Some((channel, argv));
                    }
                });
            }
            let (channel, argv) = open.unwrap();
            assert_eq!(argv, [b"sh".to_vec()]);
            let mut wire = Vec::new();
            control_frame::encode_value(Kind::Started, channel, 77, &mut wire);
            control_frame::encode_data(channel, b"shell-bytes", &mut wire);
            control_frame::encode_value(Kind::Exit, channel, 5, &mut wire);
            let mut rest = wire.as_slice();
            while !rest.is_empty() {
                let sent = rustix::net::send(&shim, rest, SendFlags::NOSIGNAL).unwrap();
                rest = &rest[sent..];
            }
            shim
        });
        let shell = door.open(&["sh".to_owned()], 80, 24).unwrap();
        assert!(shell.streamed());
        let mut output = Vec::new();
        assert_eq!(shell.pump(|bytes| output.extend_from_slice(bytes)), 5);
        assert_eq!(output, b"shell-bytes");
        let shim = answering.join().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !lock(&agent).ends_with(b"agent-bytes") && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            lock(&agent).as_slice(),
            b"agent-bytes",
            "nothing else reached the agent"
        );
        assert!(door.last_heartbeat().is_some());
        assert_eq!(door.live(), 0, "the answers for id 9 opened nothing");
        drop(shell);
        // Das Ende des Shims ist das Ende der Tür.
        drop(shim);
        let err = loop {
            match door.open(&["sh".to_owned()], 80, 24) {
                Err(err) if err.why.contains("ended") => break err,
                _ if Instant::now() > deadline => panic!("the door never saw the end"),
                _ => thread::sleep(Duration::from_millis(10)),
            }
        };
        assert_eq!(err.code, TERM_002);
    }

    /// A fake shim on a stream pair: answers the first open with the
    /// frames `answer` makes for its id, then holds its end until dropped.
    #[cfg(target_os = "linux")]
    fn stream_shim(
        answer: impl FnOnce(u32) -> Vec<u8> + Send + 'static,
    ) -> (
        Arc<ShellDoor>,
        Arc<Mutex<Vec<u8>>>,
        thread::JoinHandle<OwnedFd>,
    ) {
        let (daemon, shim) =
            socketpair(AddressFamily::UNIX, SocketType::STREAM, PAIR_FLAGS, None).unwrap();
        let agent = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&agent);
        let door =
            ShellDoor::start_stream(daemon, Box::new(move |bytes| lock(&seen).extend(bytes)))
                .unwrap();
        let shim = thread::spawn(move || {
            let mut decoder = Decoder::new();
            let mut buffer = [0u8; 4096];
            let mut id = None;
            while id.is_none() {
                let n = rustix::io::read(&shim, &mut buffer[..]).unwrap();
                assert!(n > 0, "the daemon sends an open");
                decoder.push(&buffer[..n], |event| {
                    if let Event::Frame(frame) = event
                        && let Ok(Message::Open { channel, .. }) = Message::parse(&frame)
                    {
                        id = Some(channel);
                    }
                });
            }
            let wire = answer(id.unwrap());
            let mut rest = wire.as_slice();
            while !rest.is_empty() {
                let taken = rustix::net::send(&shim, rest, SendFlags::NOSIGNAL).unwrap();
                rest = &rest[taken..];
            }
            shim
        });
        (door, agent, shim)
    }

    /// Review finding 6: an `E` for a shell that is still being asked for
    /// changes nothing, not even its output: the `R` after it starts the
    /// shell, and the bytes after the `R` arrive.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_exit_before_the_start_keeps_the_shells_output() {
        let (door, _agent, shim) = stream_shim(|id| {
            let mut wire = Vec::new();
            control_frame::encode_value(Kind::Exit, id, 9, &mut wire);
            control_frame::encode_value(Kind::Started, id, 1, &mut wire);
            control_frame::encode_data(id, b"kept", &mut wire);
            control_frame::encode_value(Kind::Exit, id, 0, &mut wire);
            wire
        });
        let shell = door.open(&["sh".to_owned()], 80, 24).unwrap();
        let mut output = Vec::new();
        assert_eq!(shell.pump(|bytes| output.extend_from_slice(bytes)), 0);
        assert_eq!(output, b"kept");
        drop(shim.join().unwrap());
    }

    /// Review finding 5: a shell whose reader stalls keeps at most
    /// [`SHELL_QUEUE`] pieces; the rest is dropped and counted, and the one
    /// reader of the stream keeps serving the agent meanwhile.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_stalled_shell_reader_holds_up_neither_memory_nor_the_agent() {
        let flood = SHELL_QUEUE * 4;
        let (door, agent, shim) = stream_shim(move |id| {
            let mut wire = Vec::new();
            control_frame::encode_value(Kind::Started, id, 1, &mut wire);
            for _ in 0..flood {
                control_frame::encode_data(id, &[b'x'; 64], &mut wire);
            }
            control_frame::encode_data(AGENT, b"agent-after", &mut wire);
            control_frame::encode_value(Kind::Exit, id, 0, &mut wire);
            wire
        });
        let shell = door.open(&["sh".to_owned()], 80, 24).unwrap();
        // Nobody pumps the shell yet; the agent's bytes still arrive.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !lock(&agent).ends_with(b"agent-after") {
            assert!(Instant::now() < deadline, "the reader hung on the shell");
            thread::sleep(Duration::from_millis(10));
        }
        let mut pieces = 0;
        assert_eq!(shell.pump(|_| pieces += 1), 0);
        assert!(pieces <= SHELL_QUEUE, "{pieces} pieces were kept");
        let Door::Stream(stream) = &door.door else {
            panic!("not a stream");
        };
        let dropped = stream.dropped.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(usize::try_from(dropped).unwrap() + pieces, flood);
        drop(shim.join().unwrap());
    }

    /// Review finding 7: one deadline for all frames of a send. A shim that
    /// takes one byte a second never lets the send finish, and the send
    /// ends at its deadline with the stream shut, not after one timeout per
    /// byte.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_shim_that_reads_a_byte_a_second_does_not_hold_a_send_for_ever() {
        let (daemon, shim) =
            socketpair(AddressFamily::UNIX, SocketType::STREAM, PAIR_FLAGS, None).unwrap();
        let door = StreamDoor {
            socket: daemon,
            write: Mutex::new(false),
            writable: Condvar::new(),
            outputs: Mutex::new(BTreeMap::new()),
            dropped: std::sync::atomic::AtomicU64::new(0),
            heartbeat: Mutex::new(None),
            agent_exit: Mutex::new(None),
        };
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = Arc::clone(&done);
        let trickle = thread::spawn(move || {
            let mut byte = [0u8; 1];
            while !stop.load(std::sync::atomic::Ordering::Relaxed)
                && let Ok(1) = rustix::io::read(&shim, &mut byte[..])
            {
                thread::sleep(Duration::from_secs(1));
            }
        });
        let started = Instant::now();
        let sent = door.send_within(&vec![0u8; 8 * 1024 * 1024], Duration::from_secs(2));
        let took = started.elapsed();
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(sent.is_err(), "eight megabytes went through a trickle");
        assert!(took < Duration::from_secs(4), "the send took {took:?}");
        trickle.join().unwrap();
    }

    /// Codex delta 1: the deadline of a send starts before it queues behind
    /// another writer. One writer holds the stream against a shim that
    /// reads nothing; a second caller with a short deadline returns its
    /// error within it, not when the first one gives up.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_send_behind_a_slow_one_keeps_its_own_deadline() {
        let (daemon, shim) =
            socketpair(AddressFamily::UNIX, SocketType::STREAM, PAIR_FLAGS, None).unwrap();
        let door = Arc::new(StreamDoor {
            socket: daemon,
            write: Mutex::new(false),
            writable: Condvar::new(),
            outputs: Mutex::new(BTreeMap::new()),
            dropped: std::sync::atomic::AtomicU64::new(0),
            heartbeat: Mutex::new(None),
            agent_exit: Mutex::new(None),
        });
        let first = Arc::clone(&door);
        let slow = thread::spawn(move || {
            first.send_within(&vec![0u8; 8 * 1024 * 1024], Duration::from_secs(4))
        });
        // The first writer holds the stream before the second one asks.
        let held = Instant::now() + Duration::from_secs(2);
        while !*lock(&door.write) {
            assert!(Instant::now() < held, "the first writer never started");
            thread::sleep(Duration::from_millis(5));
        }
        let started = Instant::now();
        let second = door.send_within(b"tiny", Duration::from_millis(500));
        let took = started.elapsed();
        assert!(second.is_err(), "the second send went through");
        assert!(
            took < Duration::from_millis(1500),
            "the second send waited {took:?}"
        );
        assert!(slow.join().unwrap().is_err());
        drop(shim);
    }

    /// Codex delta 2: a send whose deadline has passed sends nothing, even
    /// when the stream would take the bytes at once, and closes the stream.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_send_past_its_deadline_sends_nothing_and_closes_the_stream() {
        let (daemon, shim) =
            socketpair(AddressFamily::UNIX, SocketType::STREAM, PAIR_FLAGS, None).unwrap();
        let door = StreamDoor {
            socket: daemon,
            write: Mutex::new(false),
            writable: Condvar::new(),
            outputs: Mutex::new(BTreeMap::new()),
            dropped: std::sync::atomic::AtomicU64::new(0),
            heartbeat: Mutex::new(None),
            agent_exit: Mutex::new(None),
        };
        let sent = door.send_within(b"late", Duration::ZERO);
        assert_eq!(
            sent.map_err(|err| err.kind()),
            Err(std::io::ErrorKind::TimedOut)
        );
        let mut got = [0u8; 8];
        let read = rustix::io::read(&shim, &mut got).unwrap();
        assert_eq!(
            read, 0,
            "bytes went out after the deadline, or the stream stayed open"
        );
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
