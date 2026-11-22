//! Das Terminal nach jedem Ausgang eines Bildschirms (GitHub #6).
//!
//! # Wie gemessen wird
//!
//! Wie in `tests/tty.rs` bekommt ein Lauf ein Pseudoterminal, und gemessen
//! wird von außen: die `termios` vorher und nachher (alle Felder, über ihre
//! `Debug`-Form) und die Steuerfolgen, die der Lauf geschrieben hat. „Wieder
//! wie vorher" heißt: dieselben `termios`, und von alternativem Bildschirm,
//! verstecktem Cursor, Maus und Einfügeklammern ist jeweils das letzte
//! Einschalten von einem späteren Ausschalten gefolgt.
//!
//! # Warum sich der Test selbst startet
//!
//! Es gibt noch keinen Bildschirm im Produkt, und ein Befehl, der auf Zuruf
//! panikt, wäre eine Tür, die im Produkt bliebe (siehe `tests/tty.rs`). Also
//! bindet dieses Testbinary `src/tui/` und `src/tty.rs` über `#[path]` ein,
//! genau den Code, den `humanitl` benutzt, und startet sich selbst mit
//! [`PROBE`] als Kind: ein kleiner Bildschirm, der mit `q` endet und mit `p`
//! panikt. Ohne den Testläufer von libtest (`harness = false`), damit die
//! Panik im Kind genau so abläuft wie im Produkt.
//!
//! # Gemessene Ausgänge
//!
//! Gewöhnliches Ende (`q`), Panik (`p`), Panik in einem anderen Thread
//! (`t`), `SIGTERM`, `SIGHUP`, `SIGINT`, ein zweiter Wächter (Rolle
//! `owners`), ein `NoEcho` unter einem Wächter (Rolle `noecho`), ein Terminal ohne Größe, Anhalten mit
//! `SIGTSTP` und Weiterlaufen mit `SIGCONT`, ein Terminal unter der
//! Mindestgröße (`TERM_003`, nichts geschrieben, nichts verändert) und ein
//! Lauf ohne Terminal auf `stdin` (kein Bildschirm).

// `dead_code` und `unused_imports`: Eingebunden ist das ganze Modul, benutzt
// nur ein Teil. Die Testmodule darin übersetzen mit (Cargo setzt `cfg(test)`
// auch ohne libtest), ihre `#[test]`-Funktionen aber fallen weg; sie laufen
// in den Unit-Tests des Binaries.
#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

#[path = "../src/tty.rs"]
mod tty;
#[path = "../src/tui/mod.rs"]
mod tui;

use std::cell::RefCell;
use std::os::fd::{AsFd, OwnedFd};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::widgets::Paragraph;
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcgetattr, tcsetwinsize};

use tui::{
    Cells, Flow, Key, Options, Outcome, Screen, ScreenFrame, TerminalGuard, Theme, Unavailable,
};

/// Gesetzt, wenn dieses Binary als Kind läuft.
const PROBE: &str = "HUMANITL_TUI_PROBE";

/// Die Mindestgröße des Probebildschirms.
const MIN: Cells = Cells::new(20, 5);

/// Was der Probebildschirm zeigt, sobald er steht. Ein Wort: ratatui
/// überspringt Leerzeichen auf leerem Grund und setzt den Cursor dahinter.
const READY: &[u8] = b"probe-ready";

/// So lange darf ein Schritt höchstens dauern; ein ausgelasteter Läufer
/// braucht Zeit, ein hängender Lauf fällt trotzdem auf.
const PATIENCE: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Das Kind
// ---------------------------------------------------------------------------

/// Ein Bildschirm, der mit `q` endet und mit `p` panikt.
struct Probe;

impl Screen for Probe {
    fn draw(&mut self, frame: &mut Frame<'_>, theme: Theme) {
        let body = ScreenFrame {
            title: "Probe",
            context: "",
            keys: &[Key {
                key: "q",
                action: "quit",
            }],
            search: None,
            status: None,
        }
        .render(frame, theme);
        frame.render_widget(Paragraph::new("probe-ready"), body);
    }

    fn on_event(&mut self, event: &Event) -> Flow {
        match event {
            Event::Key(key) if key.code == KeyCode::Char('q') => Flow::Done,
            Event::Key(key) if key.code == KeyCode::Char('p') => panic!("the probe fell over"),
            Event::Key(key) if key.code == KeyCode::Char('t') => {
                // Eine Panik in einem anderen Thread: Der Hook gibt das
                // Terminal zurück, und die Schleife muss danach enden.
                let _ = std::thread::spawn(|| panic!("a helper thread fell over")).join();
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }
}

/// Zwei Wächter in einem Prozess: nacheinander, über einem gehaltenen
/// `RawMode`, und zugleich aus zwei Threads. Exit 7, wenn jedes Mal genau
/// einer das Terminal bekam, sonst 8.
fn child_owners(options: &Options<'_>) -> ! {
    let refused =
        |result: Result<TerminalGuard, Unavailable>| matches!(result, Err(Unavailable::Failed(_)));

    let first = TerminalGuard::enter(options).unwrap_or_else(|_| std::process::exit(9));
    let second_refused = refused(TerminalGuard::enter(options));
    drop(first);

    let raw = tty::RawMode::enter().unwrap_or_else(|| std::process::exit(9));
    let over_raw_refused = refused(TerminalGuard::enter(options));
    drop(raw);

    let barrier = std::sync::Barrier::new(2);
    let won = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                barrier.wait();
                let guard = TerminalGuard::enter(options);
                if guard.is_ok() {
                    won.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                // Beide halten, bis beide versucht haben.
                barrier.wait();
                drop(guard);
            });
        }
    });
    let one_winner = won.load(std::sync::atomic::Ordering::SeqCst) == 1;
    eprintln!(
        "second refused: {second_refused}, over raw refused: {over_raw_refused}, one winner: {one_winner}"
    );
    std::process::exit(if second_refused && over_raw_refused && one_winner {
        7
    } else {
        8
    });
}

/// Ein `NoEcho` unter einem stehenden Wächter (etwa eine Schlüsselabfrage aus
/// einem Bildschirm): erst `NoEcho` fallen lassen, dann den Wächter. Exit 0,
/// wenn beide entstanden; ob das Terminal danach steht, misst der Test.
fn child_no_echo_under_a_guard(options: &Options<'_>) -> ! {
    let guard = TerminalGuard::enter(options).unwrap_or_else(|_| std::process::exit(9));
    let hidden = tty::NoEcho::enter().unwrap_or_else(|| std::process::exit(10));
    drop(hidden);
    drop(guard);
    std::process::exit(0);
}

/// Der Lauf als Kind: Bildschirm auf, laufen lassen, Exit-Code wie ein Befehl.
fn child() -> ! {
    let options = Options {
        min: MIN,
        mouse: true,
        without_tui: "humanitl probe --plain",
    };
    if std::env::var(PROBE).as_deref() == Ok("owners") {
        child_owners(&options);
    }
    if std::env::var(PROBE).as_deref() == Ok("noecho") {
        child_no_echo_under_a_guard(&options);
    }
    let mut guard = match TerminalGuard::enter(&options) {
        Ok(guard) => guard,
        Err(Unavailable::Unsuitable(found)) => {
            eprintln!("{}: {}", found.code.as_str(), found.why);
            std::process::exit(3);
        }
        Err(Unavailable::NoTerminal) => {
            eprintln!("no terminal");
            std::process::exit(4);
        }
        Err(Unavailable::Failed(error)) => {
            eprintln!("failed: {error}");
            std::process::exit(5);
        }
    };
    let outcome = tui::run(&mut guard, &mut Probe, Theme::from_env(), MIN);
    drop(guard);
    let code = match outcome {
        Ok(Outcome::Done) => 0,
        Ok(Outcome::Interrupted) => 130,
        Ok(Outcome::Signal(signal)) => 128 + signal.number(),
        Err(error) => {
            eprintln!("{error}");
            6
        }
    };
    std::process::exit(code);
}

// ---------------------------------------------------------------------------
// Das Pseudoterminal
// ---------------------------------------------------------------------------

/// Ein Pseudoterminal: der Master für den Test, der Slave für den Lauf.
struct Pty {
    /// Ohne Blockade, damit die Warteschleife nie hängt.
    master: OwnedFd,
    /// Eine Kopie für `tcgetattr` und als Terminal des Laufs.
    slave: OwnedFd,
    /// Alles, was der Lauf bisher geschrieben hat.
    seen: RefCell<Vec<u8>>,
}

impl Pty {
    /// Ein Paar mit der Größe `cols` × `rows`.
    fn open(cols: u16, rows: u16) -> Self {
        // macOS nimmt `O_CLOEXEC` bei `posix_openpt` nicht an; ohne das Flag
        // erbte der Lauf den Master, und seine Eingabe endete nie (HUM-312).
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("a pty");
        rustix::io::fcntl_setfd(&master, rustix::io::FdFlags::CLOEXEC).expect("close-on-exec");
        grantpt(&master).expect("grantpt");
        unlockpt(&master).expect("unlockpt");
        let name = ptsname(&master, Vec::new()).expect("the name of the slave");
        let slave = rustix::fs::open(
            name.to_str().expect("a printable path"),
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .expect("the slave opens");
        rustix::fs::fcntl_setfl(&master, rustix::fs::OFlags::NONBLOCK).expect("no blocking reads");
        tcsetwinsize(
            &slave,
            Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .expect("the size is set");
        Self {
            master,
            slave,
            seen: RefCell::new(Vec::new()),
        }
    }

    /// Alle Einstellungen des Terminals, vergleichbar.
    fn settings(&self) -> String {
        format!("{:?}", tcgetattr(self.slave.as_fd()).expect("settings"))
    }

    /// Eine Kopie des Slaves für einen der drei Kanäle.
    fn stdio(&self) -> Stdio {
        Stdio::from(self.slave.try_clone().expect("a second descriptor"))
    }

    /// Liest, was da ist, ohne zu warten.
    fn drain(&self) {
        let mut buffer = [0u8; 4096];
        while let Ok(read) = rustix::io::read(&self.master, &mut buffer) {
            if read == 0 {
                return;
            }
            self.seen.borrow_mut().extend_from_slice(&buffer[..read]);
        }
    }

    /// Alles bisher Geschriebene.
    fn output(&self) -> Vec<u8> {
        self.drain();
        self.seen.borrow().clone()
    }

    /// Tippt für den Lauf.
    fn write(&self, bytes: &[u8]) {
        let written = rustix::io::write(&self.master, bytes).expect("typed");
        assert_eq!(written, bytes.len(), "the terminal took every byte");
    }

    /// Wartet, bis `done` über die Ausgabe wahr ist.
    fn wait_for(&self, what: &str, done: impl Fn(&[u8]) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while !done(&self.output()) {
            assert!(
                Instant::now() < deadline,
                "{what} did not happen within {PATIENCE:?}; output: {:?}",
                String::from_utf8_lossy(&self.output())
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Startet dieses Binary als Kind am Pseudoterminal.
fn spawn(pty: &Pty) -> Child {
    spawn_as(pty, "1")
}

/// Wie [`spawn`], mit einer Rolle für das Kind.
fn spawn_as(pty: &Pty, role: &str) -> Child {
    Command::new(std::env::current_exe().expect("the test binary"))
        .env(PROBE, role)
        .env("TERM", "xterm-256color")
        .env_remove("NO_COLOR")
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the probe starts")
}

/// Wartet auf das Ende des Kindes und liest dabei weiter.
fn wait(pty: &Pty, child: &mut Child) -> ExitStatus {
    let deadline = Instant::now() + PATIENCE;
    loop {
        pty.drain();
        if let Some(status) = child.try_wait().expect("the status") {
            pty.drain();
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the probe did not end within {PATIENCE:?}; output: {:?}",
                String::from_utf8_lossy(&pty.output())
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Die Prozessnummer des Kindes.
fn pid(child: &Child) -> rustix::process::Pid {
    rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("a pid")).expect("a valid pid")
}

/// Wo `needle` zuletzt in `haystack` steht.
fn last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

/// Wie oft `needle` in `haystack` steht.
fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

/// Jede Schaltung, die der Bildschirm einschaltet, mit ihrem Gegenstück.
const SWITCHES: &[(&str, &str, &str)] = &[
    ("the alternate screen", "\x1b[?1049h", "\x1b[?1049l"),
    ("the hidden cursor", "\x1b[?25l", "\x1b[?25h"),
    ("the mouse", "\x1b[?1000h", "\x1b[?1000l"),
    ("bracketed paste", "\x1b[?2004h", "\x1b[?2004l"),
];

/// Das Terminal steht wie vorher: dieselben `termios`, und jede Schaltung
/// ist nach ihrem letzten Einschalten wieder aus.
fn assert_restored(pty: &Pty, before: &str, output: &[u8]) {
    assert_eq!(pty.settings(), before, "the termios are not as before");
    for (what, on, off) in SWITCHES {
        let on_at =
            last(output, on.as_bytes()).unwrap_or_else(|| panic!("{what} was never switched on"));
        let off_at = last(output, off.as_bytes());
        assert!(
            off_at.is_some_and(|off_at| off_at > on_at),
            "{what} is still on; output: {:?}",
            String::from_utf8_lossy(output)
        );
    }
}

// ---------------------------------------------------------------------------
// Die Fälle
// ---------------------------------------------------------------------------

/// `q`: Exit 0, Terminal wie vorher.
fn a_normal_end_restores_the_terminal() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn(&pty);
    pty.wait_for("the probe screen", |out| last(out, READY).is_some());
    assert_ne!(pty.settings(), before, "the probe runs in raw mode");
    pty.write(b"q");
    let status = wait(&pty, &mut child);
    assert_eq!(
        status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&pty.output())
    );
    assert_restored(&pty, &before, &pty.output());
}

/// `p`: Die Panik gibt das Terminal zurück, **bevor** ihre Meldung steht.
///
/// Ohne den Hook gäbe zwar `Drop` beim Abwickeln das Terminal zurück, aber
/// erst nach der Meldung: Sie stünde im alternativen Bildschirm und
/// verschwände mit ihm. Deshalb misst der Test die Reihenfolge.
fn a_panic_restores_the_terminal_before_its_message() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn(&pty);
    pty.wait_for("the probe screen", |out| last(out, READY).is_some());
    pty.write(b"p");
    let status = wait(&pty, &mut child);
    let output = pty.output();
    assert_eq!(
        status.code(),
        Some(101),
        "{:?}",
        String::from_utf8_lossy(&output)
    );
    assert_restored(&pty, &before, &output);
    let message = last(&output, b"the probe fell over").expect("the panic message is shown");
    let left = last(&output, b"\x1b[?1049l").expect("the alternate screen was left");
    // Die erste Rückgabe zählt: Der Hook schreibt sie, `Drop` danach nicht
    // noch einmal, weil der Hook den Platz geleert hat.
    let first_left = output
        .windows(8)
        .position(|window| window == b"\x1b[?1049l")
        .expect("left once");
    assert!(
        first_left < message,
        "the panic message came before the terminal was restored; output: {:?}",
        String::from_utf8_lossy(&output)
    );
    assert_eq!(first_left, left, "the terminal was restored exactly once");
}

/// Ein Signal von außen: Exit `128 + n`, Terminal wie vorher.
fn a_signal_restores_the_terminal(signal: rustix::process::Signal, code: i32) {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn(&pty);
    pty.wait_for("the probe screen", |out| last(out, READY).is_some());
    rustix::process::kill_process(pid(&child), signal).expect("the signal is delivered");
    let status = wait(&pty, &mut child);
    assert_eq!(
        status.code(),
        Some(code),
        "{:?}",
        String::from_utf8_lossy(&pty.output())
    );
    assert_restored(&pty, &before, &pty.output());
}

fn sigterm_restores_the_terminal() {
    a_signal_restores_the_terminal(rustix::process::Signal::TERM, 143);
}

fn sighup_restores_the_terminal() {
    a_signal_restores_the_terminal(rustix::process::Signal::HUP, 129);
}

/// `SIGTSTP`: Das Terminal ist zurückgegeben, solange der Lauf steht; nach
/// `SIGCONT` ist der Bildschirm wieder da, und `q` beendet gewöhnlich.
fn stop_and_continue_give_the_terminal_back_and_take_it_again() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn(&pty);
    pty.wait_for("the probe screen", |out| last(out, READY).is_some());
    rustix::process::kill_process(pid(&child), rustix::process::Signal::TSTP).expect("stopped");
    let stopped =
        rustix::process::waitpid(Some(pid(&child)), rustix::process::WaitOptions::UNTRACED)
            .expect("waitpid")
            .expect("a status");
    assert!(stopped.1.stopped(), "the probe stopped: {:?}", stopped.1);
    assert_restored(&pty, &before, &pty.output());

    let entered = count(&pty.output(), b"\x1b[?1049h");
    rustix::process::kill_process(pid(&child), rustix::process::Signal::CONT).expect("continued");
    pty.wait_for("the screen after SIGCONT", |out| {
        count(out, b"\x1b[?1049h") > entered && last(out, READY) > last(out, b"\x1b[?1049h")
    });
    assert_ne!(pty.settings(), before, "raw again after SIGCONT");
    pty.write(b"q");
    let status = wait(&pty, &mut child);
    assert_eq!(
        status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&pty.output())
    );
    assert_restored(&pty, &before, &pty.output());
}

/// Zu klein: `TERM_003`, keine einzige Steuerfolge, dieselben `termios`.
fn a_terminal_too_small_is_term_003_and_stays_untouched() {
    let pty = Pty::open(15, 4);
    let before = pty.settings();
    let mut child = spawn(&pty);
    let status = wait(&pty, &mut child);
    let output = pty.output();
    assert_eq!(
        status.code(),
        Some(3),
        "{:?}",
        String::from_utf8_lossy(&output)
    );
    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("TERM_003"), "{text:?}");
    assert!(
        text.contains("20 × 5") && text.contains("15 × 4"),
        "{text:?}"
    );
    assert!(!output.contains(&0x1b), "nothing was switched: {text:?}");
    assert_eq!(pty.settings(), before, "the termios are untouched");
}

/// Ohne Terminal auf `stdin` (eine Pipe, ein Skript): kein Bildschirm, keine
/// Steuerfolge, kein Rohmodus am Terminal auf `stdout`. crossterm öffnete
/// sonst `/dev/tty` und ginge an der Pipe vorbei.
fn without_a_terminal_on_stdin_there_is_no_screen() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = Command::new(std::env::current_exe().expect("the test binary"))
        .env(PROBE, "1")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the probe starts");
    let status = wait(&pty, &mut child);
    let output = pty.output();
    let text = String::from_utf8_lossy(&output);
    assert_eq!(status.code(), Some(4), "{text:?}");
    assert!(!output.contains(&0x1b), "nothing was switched: {text:?}");
    assert_eq!(pty.settings(), before, "the termios are untouched");
}

fn sigint_restores_the_terminal() {
    a_signal_restores_the_terminal(rustix::process::Signal::INT, 130);
}

/// Eine Panik in einem anderen Thread: Das Terminal ist zurück, danach
/// zeichnet der Bildschirm nichts mehr, und die Schleife endet mit einem
/// Fehler (Exit 6 des Kindes), statt auf `q` zu warten.
fn a_panic_in_another_thread_ends_the_view() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn(&pty);
    pty.wait_for("the probe screen", |out| last(out, READY).is_some());
    pty.write(b"t");
    let status = wait(&pty, &mut child);
    let output = pty.output();
    let text = String::from_utf8_lossy(&output);
    assert_eq!(status.code(), Some(6), "{text:?}");
    assert_restored(&pty, &before, &output);
    let message = last(&output, b"a helper thread fell over").expect("the panic message is shown");
    assert!(
        !output[message..].contains(&0x1b),
        "the view kept drawing after the terminal was given back: {text:?}"
    );
}

/// Ein Wächter je Prozess: ein zweiter, einer über einem gehaltenen
/// `RawMode` und der zweite von zwei gleichzeitigen Threads werden
/// abgewiesen; das Terminal ist danach wie vorher.
fn only_one_guard_holds_the_terminal() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn_as(&pty, "owners");
    let status = wait(&pty, &mut child);
    let output = pty.output();
    assert_eq!(
        status.code(),
        Some(7),
        "{:?}",
        String::from_utf8_lossy(&output)
    );
    assert_eq!(pty.settings(), before, "the termios are not as before");
    for (what, on, off) in SWITCHES {
        if let Some(on_at) = last(&output, on.as_bytes()) {
            assert!(
                last(&output, off.as_bytes()).is_some_and(|off_at| off_at > on_at),
                "{what} is still on"
            );
        }
    }
}

/// Ein Terminal, das keine Größe meldet (0 × 0), ist `TERM_003` und wird
/// nicht als 80 × 24 gerechnet.
fn a_terminal_without_size_is_term_003() {
    let pty = Pty::open(0, 0);
    let before = pty.settings();
    let mut child = spawn(&pty);
    let status = wait(&pty, &mut child);
    let output = pty.output();
    let text = String::from_utf8_lossy(&output);
    assert_eq!(status.code(), Some(3), "{text:?}");
    assert!(
        text.contains("TERM_003") && text.contains("reports no size"),
        "{text:?}"
    );
    assert!(text.contains("stty rows"), "{text:?}");
    assert!(!output.contains(&0x1b), "nothing was switched: {text:?}");
    assert_eq!(pty.settings(), before, "the termios are untouched");
}

/// `NoEcho` unter einem Wächter überschreibt dessen gesicherte Einstellungen
/// nicht: Nach beiden ist das Terminal wie vorher und nicht roh.
fn no_echo_under_a_guard_leaves_the_terminal_as_before() {
    let pty = Pty::open(80, 24);
    let before = pty.settings();
    let mut child = spawn_as(&pty, "noecho");
    let status = wait(&pty, &mut child);
    let output = pty.output();
    assert_eq!(
        status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&output)
    );
    assert_restored(&pty, &before, &output);
}

fn main() {
    if std::env::var_os(PROBE).is_some() {
        child();
    }
    // Unter macOS hing der Lauf in der CI (GitHub #6, Lauf 44956967584: der
    // Job lief bis zur Frist). Bis die Pseudoterminal-Fälle dort geprüft und
    // begrenzt sind (GitHub #56), laufen sie nur unter Linux.
    if cfg!(not(target_os = "linux")) {
        println!("tui: pseudo-terminal cases run on Linux only (GitHub #56)");
        return;
    }
    let cases: &[(&str, fn())] = &[
        (
            "a_normal_end_restores_the_terminal",
            a_normal_end_restores_the_terminal,
        ),
        (
            "a_panic_restores_the_terminal_before_its_message",
            a_panic_restores_the_terminal_before_its_message,
        ),
        (
            "sigterm_restores_the_terminal",
            sigterm_restores_the_terminal,
        ),
        ("sighup_restores_the_terminal", sighup_restores_the_terminal),
        ("sigint_restores_the_terminal", sigint_restores_the_terminal),
        (
            "a_panic_in_another_thread_ends_the_view",
            a_panic_in_another_thread_ends_the_view,
        ),
        (
            "only_one_guard_holds_the_terminal",
            only_one_guard_holds_the_terminal,
        ),
        (
            "no_echo_under_a_guard_leaves_the_terminal_as_before",
            no_echo_under_a_guard_leaves_the_terminal_as_before,
        ),
        (
            "a_terminal_without_size_is_term_003",
            a_terminal_without_size_is_term_003,
        ),
        (
            "stop_and_continue_give_the_terminal_back_and_take_it_again",
            stop_and_continue_give_the_terminal_back_and_take_it_again,
        ),
        (
            "a_terminal_too_small_is_term_003_and_stays_untouched",
            a_terminal_too_small_is_term_003_and_stays_untouched,
        ),
        (
            "without_a_terminal_on_stdin_there_is_no_screen",
            without_a_terminal_on_stdin_there_is_no_screen,
        ),
    ];
    // Wie bei libtest: Wörter ohne `-` filtern nach Namen.
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .collect();
    let mut failed = 0;
    for (name, case) in cases {
        if !filters.is_empty() && !filters.iter().any(|filter| name.contains(filter.as_str())) {
            continue;
        }
        let result = std::panic::catch_unwind(case);
        println!(
            "test {name} ... {}",
            if result.is_ok() { "ok" } else { "FAILED" }
        );
        failed += usize::from(result.is_err());
    }
    if failed > 0 {
        eprintln!("{failed} tui test(s) failed");
        std::process::exit(1);
    }
}
