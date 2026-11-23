//! Das Terminal nach jedem Ausgang: Rohmodus an, Prozess beendet, Terminal
//! zurück (HUM-042, HUM-067).
//!
//! # Warum das ein eigenes Testbinary ist
//!
//! Ein Rohmodus entsteht nur an einem echten Terminal. `tests/cli.rs` gibt
//! jedem Lauf `Stdio::null()`, und ohne Terminal setzt `humanitl` den Rohmodus
//! gar nicht erst -- ein Test dort könnte die Rückgabe also nie sehen. Hier
//! bekommt der Lauf ein Pseudoterminal, und gemessen werden die Einstellungen
//! dieses Terminals, von außen gelesen: `ICANON` und `ECHO` sind vorher an,
//! während des Laufs aus und danach wieder an.
//!
//! Vier Ausgänge sind gemessen: `SIGTERM`, `SIGHUP` und `SIGINT` (jeweils
//! `128 + n`, und das Terminal danach gewöhnlich) und das gewöhnliche Ende,
//! wenn die Eingabe endet (Exit 0; über das Terminal sagt dieser Fall nichts,
//! der Grund steht am Test).
//!
//! Gemessen wird an `humanitl sandbox attach`, und das ist eine Aussage über
//! beide Befehle: Der Rohmodus und seine Rückgabe stehen einmal in
//! `src/tty.rs`, und `humanitl run --ask terminal` benutzt dieselbe Fassung.
//! Was `run` zusätzlich tut -- erst die Sitzung stoppen, dann das Terminal
//! zurückgeben (`cmd/run.rs`) --, steht hier nicht: Der Fake beendet seine
//! Sandbox sofort, also kommt ein `run` gegen ihn gar nicht erst bis zum
//! Rohmodus (gemessen: Exit 1 mit `CLI_001`, bevor das Terminal gesetzt war).
//! Dieser halbe Weg gehört in das Demoskript mit echtem Daemon (HUM-046).
//!
//! Zwei Wege fehlen mit Grund. Die **Panik**: Es gibt kein Kommando, das auf
//! Zuruf panikt, und eines nur für den Test wäre eine Tür, die im Produkt
//! bliebe. Der **Dienst, der verschwindet**: Der Fake-Server dieses Tests
//! wartet beim Herunterfahren auf seine offenen Ströme, und der Terminal-Strom
//! ist genau so einer -- der Test hinge an seinem eigenen Aufräumen. Was `Drop`
//! auf einem Fehlerpfad tut, misst dieselbe Zeile wie bei den Signalen
//! (`src/tty.rs`).
//!
//! # Der Master gehört dem Test allein
//!
//! Das Pseudoterminal wird nicht von einem eigenen Thread leergelesen, sondern
//! bei jedem Schritt der Warteschleife, mit einem Deskriptor ohne Blockade.
//! Ein Leser-Thread hielte eine zweite Kopie des Masters, und dann endete das
//! Schließen die Eingabe des Laufs nicht: Solange irgendjemand den Master hält,
//! wartet der Lauf weiter auf eine Taste. Gemessen: mit Thread lief der
//! gewöhnliche Ausgang in die Frist, ohne ihn endet er in Millisekunden.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::fd::{AsFd, OwnedFd};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{LocalModes, tcgetattr};

mod common;

use common::{FakeServer, Harness, PATIENCE};
#[cfg(target_os = "linux")]
use common::{FirstStartTweaks, Tracking};

/// Ein Pseudoterminal: der Master, den der Test hält, und der Slave, den der
/// Lauf als sein Terminal bekommt.
struct Pty {
    /// Die Seite des Tests, ohne Blockade. `None`, sobald sie geschlossen ist;
    /// für den Lauf endet damit die Eingabe.
    master: Option<OwnedFd>,
    /// Die Seite des Laufs. Der Test hält eine Kopie, um `tcgetattr` zu rufen.
    slave: OwnedFd,
    /// Was der Lauf geschrieben hat, seit zuletzt gelesen wurde.
    ///
    /// Das Leeren des Puffers und das Lesen sind dieselbe Bewegung: Wer nur
    /// wegwirft, hat nichts mehr, wenn ein Test wissen will, was dastand.
    seen: std::cell::RefCell<Vec<u8>>,
}

impl Pty {
    /// Öffnet ein Paar.
    fn open() -> Self {
        // `CLOEXEC` ist hier keine Hygiene, sondern die Bedingung des Tests:
        // Ohne sie erbt der Lauf den Master, und dann endet seine Eingabe nie,
        // weil er sie selbst offen hält. Gemessen: ohne `CLOEXEC` lief der
        // gewöhnliche Ausgang in die Frist, mit ihr endet er in 0,2 s. Der
        // Slave darf `CLOEXEC` tragen, weil `Stdio` ihn auf 0, 1 und 2 legt und
        // dabei das Flag verliert.
        // macOS nimmt `O_CLOEXEC` bei `posix_openpt` nicht an; das Flag kommt
        // danach, sonst erbte jedes gestartete Kind die Herrscherseite, und
        // sie schlösse nie (HUM-312).
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
        // Ohne Blockade: Der Test liest den Master in seiner Warteschleife und
        // darf dabei nie hängenbleiben.
        rustix::fs::fcntl_setfl(&master, rustix::fs::OFlags::NONBLOCK).expect("no blocking reads");
        Self {
            master: Some(master),
            slave,
            seen: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Die Einstellungen des Terminals, wie sie gerade gelten.
    fn modes(&self) -> LocalModes {
        tcgetattr(self.slave.as_fd())
            .expect("the terminal has settings")
            .local_modes
    }

    /// Wahr, solange das Terminal gewöhnlich ist: Zeilen und Echo.
    fn is_cooked(&self) -> bool {
        let modes = self.modes();
        modes.contains(LocalModes::ICANON) && modes.contains(LocalModes::ECHO)
    }

    /// Eine eigene Kopie der Slave-Seite für einen der drei Kanäle.
    fn stdio(&self) -> Stdio {
        Stdio::from(self.slave.try_clone().expect("a second descriptor"))
    }

    /// Alles, was der Lauf bisher geschrieben hat.
    fn read(&self) -> Vec<u8> {
        self.drain();
        self.seen.borrow().clone()
    }

    /// Nimmt weg, was der Lauf geschrieben hat, und kehrt sofort zurück.
    ///
    /// Ohne das füllt sich der Puffer des Pseudoterminals mit der
    /// Bildschirmumschaltung und der Ausgabe des Agenten, und der Lauf bliebe
    /// im Schreiben stehen, statt auf sein Signal zu warten.
    fn drain(&self) {
        let Some(master) = self.master.as_ref() else {
            return;
        };
        let mut buffer = [0u8; 4096];
        while let Ok(read) = rustix::io::read(master, &mut buffer) {
            if read == 0 {
                return;
            }
            self.seen.borrow_mut().extend_from_slice(&buffer[..read]);
        }
    }

    /// Schließt die Seite des Tests -- derselbe Weg, den ein Mensch nimmt, der
    /// sein Terminal schließt.
    fn close_master(&mut self) {
        self.master = None;
    }

    /// Wie [`Pty::write`], aber mit Frist: `false`, wenn der Lauf die Bytes
    /// bis dahin nicht abgenommen hat. Für den Fall, in dem genau das der
    /// Fehler wäre -- ein Lauf, der seine Tastatur nicht mehr liest.
    fn write_within(&self, bytes: &[u8], deadline: Instant) -> bool {
        let master = self.master.as_ref().expect("the master is still open");
        let mut written = 0;
        while written < bytes.len() {
            if Instant::now() >= deadline {
                return false;
            }
            match rustix::io::write(master, &bytes[written..]) {
                Ok(0) => panic!("the terminal takes nothing"),
                Ok(count) => written += count,
                Err(rustix::io::Errno::AGAIN) => {
                    self.drain();
                    // Auf Platz warten statt zu schlafen: Das PTY von macOS
                    // nimmt kleine Häppchen, und eine Millisekunde Schlaf je
                    // Häppchen wird auf einem ausgelasteten Läufer zu zehn,
                    // dann reicht die Frist für 4 MiB nicht mehr (HUM-312).
                    let mut fds = [rustix::event::PollFd::new(
                        master,
                        rustix::event::PollFlags::OUT,
                    )];
                    let wait = rustix::event::Timespec {
                        tv_sec: 0,
                        tv_nsec: 10_000_000,
                    };
                    let _ = rustix::event::poll(&mut fds, Some(&wait));
                }
                Err(error) => panic!("cannot write to the terminal: {error}"),
            }
        }
        true
    }

    /// Tippt für den Lauf: schreibt Bytes in das Terminal.
    fn write(&self, bytes: &[u8]) {
        let master = self.master.as_ref().expect("the master is still open");
        let mut written = 0;
        while written < bytes.len() {
            match rustix::io::write(master, &bytes[written..]) {
                Ok(0) => panic!("the terminal takes nothing"),
                Ok(count) => written += count,
                Err(rustix::io::Errno::AGAIN) => std::thread::sleep(Duration::from_millis(5)),
                Err(error) => panic!("cannot write to the terminal: {error}"),
            }
        }
    }
}

/// Startet `humanitl sandbox attach` an diesem Terminal.
fn attach(harness: &Harness, pty: &Pty) -> Child {
    attach_with(harness, pty, &[])
}

/// Wie [`attach`], mit weiteren Argumenten.
fn attach_with(harness: &Harness, pty: &Pty, extra: &[&str]) -> Child {
    harness
        .command()
        .args(["sandbox", "attach"])
        .args(extra)
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts")
}

/// Wartet, bis der Lauf das Terminal in den Rohmodus gesetzt hat.
///
/// Vorher sagt jede Messung nichts: Ein Terminal, das nie roh war, ist
/// hinterher gewöhnlich, ohne dass jemand es zurückgegeben hätte.
fn wait_for_raw(child: &mut Child, pty: &Pty) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        pty.drain();
        if !pty.modes().contains(LocalModes::ICANON) {
            return;
        }
        // Ein Lauf, der schon vorbei ist, setzt nichts mehr: Ohne diese Frage
        // wartete der Test die volle Frist ab und meldete dann den falschen
        // Grund -- „nie roh" statt „gar nicht erst gestartet".
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            panic!("the command ended with {status:?} before it took the terminal");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Der Lauf gehört zu diesem Test, auch wenn der Test scheitert; ohne das
    // bliebe er als Waise stehen.
    let _ = child.kill();
    let _ = child.wait();
    panic!("the command never put the terminal into raw mode");
}

/// Wartet auf das Ende des Laufs und gibt seinen Exit-Code zurück.
fn wait_for_exit(child: &mut Child, pty: &Pty) -> i32 {
    let deadline = Instant::now() + PATIENCE;
    loop {
        pty.drain();
        match child.try_wait().expect("the child can be polled") {
            Some(status) => return status.code().unwrap_or(-1),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the command did not end within {PATIENCE:?}");
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Schickt dem Lauf ein Signal.
fn signal(child: &Child, signal: rustix::process::Signal) {
    let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("a pid"))
        .expect("a valid pid");
    rustix::process::kill_process(pid, signal).expect("the signal is delivered");
}

/// `SIGTERM`, `SIGHUP` und `SIGINT`: Der Lauf endet mit `128 + n`, und das
/// Terminal ist danach wieder gewöhnlich.
///
/// Diese drei Wege gehören dem Befehl und nicht `src/tty.rs`: Ein
/// Signalhandler, der selbst beendet, wäre schneller als die RPC, mit der ein
/// Befehl seine Sitzung stoppt. Gemessen wird deshalb am Befehl, so wie ein
/// Mensch ihn abbricht.
#[test]
fn every_signal_gives_the_terminal_back() {
    for (name, number, expected) in [
        ("SIGTERM", rustix::process::Signal::TERM, 143),
        ("SIGHUP", rustix::process::Signal::HUP, 129),
        ("SIGINT", rustix::process::Signal::INT, 130),
    ] {
        let harness = Harness::new();
        let _server = FakeServer::start(&harness);
        let pty = Pty::open();
        assert!(pty.is_cooked(), "{name}: the terminal starts cooked");

        let mut child = attach(&harness, &pty);
        wait_for_raw(&mut child, &pty);
        signal(&child, number);

        assert_eq!(wait_for_exit(&mut child, &pty), expected, "{name}");
        assert!(
            pty.is_cooked(),
            "{name}: the terminal stayed raw, modes {:?}",
            pty.modes()
        );
    }
}

/// Das Ende der Sitzung: Der Dienst schickt `Exit`, der Strom endet, und das
/// Terminal ist wieder gewöhnlich.
///
/// Das ist der Weg, den ein Mensch am häufigsten geht: Der Agent endet, und
/// der Befehl kommt zurück, ohne dass jemand ein Signal geschickt oder ein
/// Fenster geschlossen hätte. Der Fake dieses Tests beendet seine Sitzung beim
/// Byte `0x04` (`Ctrl+D`, `daemon/crates/ipc/src/fake/mod.rs`); im Betrieb
/// endet sie, wenn der Agent endet. Das Pseudoterminal bleibt dabei stehen,
/// also lässt sich hier -- anders als beim Ende der Eingabe -- auch fragen, wie
/// das Terminal danach dasteht.
#[test]
fn the_end_of_the_session_gives_the_terminal_back() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let pty = Pty::open();
    assert!(pty.is_cooked(), "the terminal starts cooked");

    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    pty.write(&[0x04]);

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(
        pty.is_cooked(),
        "the terminal stayed raw, modes {:?}",
        pty.modes()
    );
}

/// Das Ende der Eingabe: Die andere Seite des Terminals geht zu, der Befehl
/// meldet sich beim Dienst ab und endet mit 0.
///
/// **Über das Terminal sagt dieser Fall nichts, und das ist kein Versehen.**
/// Die Eingabe eines Pseudoterminals endet nur, wenn seine andere Seite
/// zugeht, und damit ist das Terminal selbst weg: `tcgetattr` antwortet danach
/// `EIO` (gemessen). Ein Terminal, das es nicht mehr gibt, kann auch nicht im
/// Rohmodus zurückbleiben. Gemessen wird deshalb, dass der Befehl diesen Weg
/// überhaupt nimmt und mit 0 endet, statt auf eine Taste zu warten, die
/// niemand mehr drücken kann; die Rückgabe des Terminals messen die drei Fälle
/// darüber.
#[test]
fn the_ordinary_end_is_an_exit_and_not_a_wait() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let mut pty = Pty::open();

    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    pty.close_master();

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
}

/// Wartet, bis der Lauf dieses Byte geschrieben hat; der Fake wirft jede
/// Eingabe zurück, also heißt das: Es ist beim Agenten angekommen.
fn wait_for_byte(child: &mut Child, pty: &Pty, byte: u8) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        if pty.read().contains(&byte) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("the byte {byte:#04x} never came back from the session");
}

/// `Ctrl+B d` hängt ab (HUM-279): Exit 0, ein `close` beim Daemon, keine der
/// beiden Tasten beim Agenten, das Terminal gewöhnlich, und ein zweites
/// `attach` kommt wieder an die Sitzung.
///
/// Der Fake beantwortet `close` und das Ende des Stroms gleich; ob wirklich
/// `close` kam, liest der Test deshalb aus dem, was der Fake empfangen hat.
#[test]
fn ctrl_b_d_detaches_and_gives_the_terminal_back() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let pty = Pty::open();
    assert!(pty.is_cooked(), "the terminal starts cooked");

    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02d");

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(
        pty.is_cooked(),
        "the terminal stayed raw, modes {:?}",
        pty.modes()
    );
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("Detach: Ctrl+B d"), "{said:?}");
    assert!(
        said.contains("Detached; the session keeps running. Attach again: humanitl sandbox attach"),
        "{said:?}"
    );
    let inputs = server.terminal_inputs();
    assert_eq!(inputs.first(), Some(&"open"), "{inputs:?}");
    assert_eq!(inputs.last(), Some(&"close"), "{inputs:?}");
    assert!(
        !inputs.contains(&"data"),
        "the detach key reached the agent: {inputs:?}"
    );

    // Die Sitzung ist nicht vorbei: Ein zweites `attach` hängt sich wieder an.
    let mut again = attach(&harness, &pty);
    wait_for_raw(&mut again, &pty);
    pty.write(&[0x04]);
    assert_eq!(wait_for_exit(&mut again, &pty), 0);
}

/// Die Zeile zur neuen Vorgabe (GitHub #10): beim ersten Anhängen ohne
/// gesetzten Wert, nicht beim zweiten, und nie mit gesetztem Wert; ein
/// gesetztes `ctrl-a` bleibt die Präfixtaste.
#[test]
#[cfg(target_os = "linux")]
fn the_new_prefix_is_announced_once() {
    const LINE: &str = "The prefix key is now Ctrl+B (was Ctrl+A). Keep the old one: humanitl \
                        config set terminal.prefix ctrl-a";
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let pty = Pty::open();
    // `read` gibt alles seit dem Öffnen; jede Runde misst nur ihren Teil.
    let mut seen = 0;
    for (round, announced) in [(1, true), (2, false)] {
        let mut child = attach(&harness, &pty);
        wait_for_raw(&mut child, &pty);
        pty.write(b"\x02d");
        assert_eq!(wait_for_exit(&mut child, &pty), 0);
        let all = pty.read();
        let said = String::from_utf8_lossy(&all[seen..]).into_owned();
        seen = all.len();
        assert_eq!(said.contains(LINE), announced, "attach {round}: {said:?}");
    }

    let set = Harness::new();
    let _other = FakeServer::start(&set);
    let mut child = set
        .command()
        .env("HUMANITL_TERMINAL__PREFIX", "ctrl-a")
        .args(["sandbox", "attach"])
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x01d");
    assert_eq!(
        wait_for_exit(&mut child, &pty),
        0,
        "ctrl-a stays the prefix"
    );
    let said = String::from_utf8_lossy(&pty.read()[seen..]).into_owned();
    assert!(!said.contains(LINE), "{said:?}");
    assert!(said.contains("Detach: Ctrl+A d"), "{said:?}");
}

/// `--read-only --prefix none` liest keine Tastatur und setzt keinen
/// Rohmodus, gibt aber das Terminal trotzdem zurück: Alternativschirm und
/// kitty-Flags kommen mit der Ausgabe des Agenten, also hängt das Aufräumen
/// an einer Ausgabe auf ein Terminal (Review zu GitHub #10). Mutationsbeweis:
/// Aufräumen nur mit Rohmodus macht ihn rot.
#[test]
#[cfg(target_os = "linux")]
fn a_watcher_without_keys_still_gives_the_screen_back() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let pty = Pty::open();
    let mut child = attach_with(&harness, &pty, &["--read-only", "--prefix", "none"]);
    let deadline = Instant::now() + PATIENCE;
    while !String::from_utf8_lossy(&pty.read()).contains("input is echoed") {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("no output: {:?}", String::from_utf8_lossy(&pty.read()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    signal(&child, rustix::process::Signal::TERM);
    assert_eq!(wait_for_exit(&mut child, &pty), 143);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("\x1b[?1049l\x1b[?25h"), "{said:?}");
}

/// `Ctrl+B ?` zeigt die Tabelle auf dem Alternativschirm, die nächste Taste
/// schließt sie, ohne den Agenten zu erreichen, und `Ctrl+B d` hängt danach
/// ab (GitHub #10).
#[test]
#[cfg(target_os = "linux")]
fn ctrl_b_question_mark_shows_the_table() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let pty = Pty::open();
    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02?");
    let deadline = Instant::now() + PATIENCE;
    while !String::from_utf8_lossy(&pty.read()).contains("Prefix keys") {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("no help: {:?}", String::from_utf8_lossy(&pty.read()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    pty.write(b"q");
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("\x1b[?1049h"), "{said:?}");
    assert!(said.contains("Ctrl+B Ctrl+B or Ctrl+B b"), "{said:?}");
    assert!(said.contains("\x1b[?1049l"), "{said:?}");
    assert!(
        !server.terminal_inputs().contains(&"data"),
        "a key of the help reached the agent: {:?}",
        server.terminal_inputs()
    );
}

/// `Ctrl+B Ctrl+B` schickt dem Agenten ein `Ctrl+B` und hängt nicht ab
/// (HUM-279).
#[test]
fn ctrl_b_twice_reaches_the_agent() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let pty = Pty::open();

    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02\x02");
    wait_for_byte(&mut child, &pty, 0x02);
    assert!(
        child.try_wait().expect("the child can be polled").is_none(),
        "Ctrl+B Ctrl+B detached"
    );

    pty.write(&[0x04]);
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(!said.contains("Detached;"), "{said:?}");
}

/// `--read-only` hängt mit derselben Taste ab (HUM-279) und gibt das
/// Terminal zurück, das er dafür in den Rohmodus gesetzt hat. `Ctrl+C` hängt
/// ihn ebenso ab: Im Rohmodus ist es kein Signal mehr, und ein lesender
/// Anschluss hat niemanden, dem er es schicken könnte.
#[test]
fn read_only_detaches_on_the_same_key() {
    for keys in [&b"\x02d"[..], &b"\x03"[..]] {
        let harness = Harness::new();
        let server = FakeServer::start(&harness);
        let pty = Pty::open();

        let mut child = attach_with(&harness, &pty, &["--read-only"]);
        wait_for_raw(&mut child, &pty);
        pty.write(keys);

        assert_eq!(wait_for_exit(&mut child, &pty), 0, "{keys:?}");
        assert!(
            pty.is_cooked(),
            "{keys:?}: the terminal stayed raw, modes {:?}",
            pty.modes()
        );
        let inputs = server.terminal_inputs();
        assert_eq!(inputs, vec!["open", "close"], "{keys:?}");
    }
}

/// Eine Sitzung, die ihre Eingabe nicht liest, hält den Menschen nicht fest
/// (HUM-279, Review): Nach einem großen Einfügen staut sich alles im
/// Transport, und `Ctrl+B d` hängt trotzdem ab, in begrenzter Zeit, mit
/// Exit 0 und einem gewöhnlichen Terminal.
///
/// Gemessen wird beides mit Frist: dass der Lauf das Eingefügte abnimmt (ein
/// Leser, der in `send` wartet, nähme es nie ab) und dass er danach endet.
#[test]
fn a_session_that_reads_nothing_still_lets_go() {
    let harness = Harness::new();
    let _server = FakeServer::start_with_stalled_terminal(&harness);
    let pty = Pty::open();

    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);

    // 4 MiB: mehr, als Transport, Kanal und Warteschlange zusammen fassen.
    let paste = vec![b'x'; 4 * 1024 * 1024];
    let taken = pty.write_within(&paste, Instant::now() + Duration::from_secs(20));
    if !taken {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the command stopped reading its keyboard behind a session that reads nothing");
    }
    let started = Instant::now();
    assert!(
        pty.write_within(b"\x02d", Instant::now() + Duration::from_secs(5)),
        "the detach key was not taken"
    );

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "detaching took {:?}",
        started.elapsed()
    );
    assert!(
        pty.is_cooked(),
        "the terminal stayed raw, modes {:?}",
        pty.modes()
    );
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(
        said.contains("Detached; the session keeps running."),
        "{said:?}"
    );
}

/// `--read-only --detach-keys ctrl-c`: Ein einzelnes `Ctrl+C` hängt ab, obwohl
/// es zugleich das Präfix ist (HUM-279, Review).
#[test]
fn read_only_ctrl_c_detaches_even_as_the_prefix() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let pty = Pty::open();

    let mut child = attach_with(&harness, &pty, &["--read-only", "--detach-keys", "ctrl-c"]);
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x03");

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());
    assert_eq!(server.terminal_inputs(), vec!["open", "close"]);
}

/// `--detach-keys none`: `Ctrl+B d` geht an den Agenten, und der Lauf hängt
/// sich nicht ab (HUM-279).
#[test]
fn detach_keys_none_forwards_ctrl_b_d() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let pty = Pty::open();

    let mut child = attach_with(&harness, &pty, &["--detach-keys", "none"]);
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02d");
    wait_for_byte(&mut child, &pty, 0x02);
    assert!(
        child.try_wait().expect("the child can be polled").is_none(),
        "--detach-keys none still detached"
    );
    pty.write(&[0x04]);
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(!said.contains("Detach: "), "{said:?}");
}

/// `--ask terminal -- opencode` ist der Vollbild-Agent und wird abgelehnt,
/// `-- bash` nicht (HUM-135).
///
/// Der Fall gehört hierher und nicht zu `tests/cli.rs`: Ohne Terminal lehnt
/// `--ask terminal` **jedes** Kommando ab, und dann misst der Test die
/// fehlende Konsole statt der Frage, um die es geht.
#[test]
fn ask_terminal_is_cli_002_for_the_named_agent() {
    let harness = Harness::new();
    let pty = Pty::open();

    let mut refused = harness
        .command()
        .args(["run", "--ask", "terminal", "--", "opencode"])
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    assert_eq!(wait_for_exit(&mut refused, &pty), 1);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("CLI_002"), "{said}");
    assert!(said.contains("--ask ui"), "{said}");

    // Ein eigenes Kommando ist kein Vollbild-Agent. Der Lauf geht dann weiter
    // und scheitert erst am fehlenden Daemon -- Exit 2, nicht 1.
    let mut allowed = harness
        .command()
        .args(["run", "--ask", "terminal", "--", "bash"])
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    assert_eq!(wait_for_exit(&mut allowed, &pty), 2);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("DAEMON_001"), "{said}");
}

/// `humanitl run -- bash` at a terminal attaches like `sandbox attach`:
/// raw mode once the session runs, the detach key of HUM-279, the session
/// left running, and the terminal given back (HUM-280).
#[test]
fn run_with_a_command_at_a_terminal_attaches() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let pty = Pty::open();
    assert!(pty.is_cooked(), "the terminal starts cooked");

    let mut child = harness
        .command()
        .args(["run", "--", "bash"])
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02d");

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(
        pty.is_cooked(),
        "the terminal stayed raw, modes {:?}",
        pty.modes()
    );
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("Detach: Ctrl+B d"), "{said:?}");
    assert!(
        said.contains("Detached; the session keeps running"),
        "{said:?}"
    );
    let inputs = server.terminal_inputs();
    assert_eq!(inputs.first(), Some(&"open"), "{inputs:?}");
    assert_eq!(inputs.last(), Some(&"close"), "{inputs:?}");
}

/// Without a terminal on `stdin`, `humanitl run -- bash` only reads along,
/// as before HUM-280: no raw mode and no `Terminal` stream.
#[test]
fn run_with_a_command_without_a_terminal_does_not_attach() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let pty = Pty::open();

    let mut child = harness
        .command()
        .args(["run", "--", "bash"])
        .stdin(Stdio::null())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    let deadline = Instant::now() + PATIENCE;
    let code = loop {
        pty.drain();
        assert!(
            pty.is_cooked(),
            "the run set raw mode without a terminal on stdin: {:?}",
            pty.modes()
        );
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            break status.code().unwrap_or(-1);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the run did not end within {PATIENCE:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    // The fake's start ends without an exit code: the run ends with
    // `CLI_001`, as it always did there.
    assert_ne!(code, 0, "{said:?}");
    assert!(!said.contains("Detach:"), "{said:?}");
    assert!(
        server.terminal_inputs().is_empty(),
        "no Terminal stream without a terminal: {:?}",
        server.terminal_inputs()
    );
}

/// `humanitl sandbox exec` asks the daemon for a further shell, attaches to
/// that shell's terminal (not the agent's) like `sandbox attach`, and
/// `Ctrl+B d` detaches with a line that names the shell (HUM-299).
#[test]
fn sandbox_exec_opens_a_shell_and_attaches_to_its_terminal() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let pty = Pty::open();

    let mut child = harness
        .command()
        .args(["sandbox", "exec", "--", "sh", "-c", "true"])
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02d");

    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(
        pty.is_cooked(),
        "the terminal stayed raw: {:?}",
        pty.modes()
    );
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(said.contains("Shell 1 in sandbox fake"), "{said:?}");
    assert!(
        said.contains(
            "Detached; the shell keeps running. Attach again: humanitl sandbox attach --shell 1"
        ),
        "{said:?}"
    );
    let inputs = server.terminal_inputs();
    assert_eq!(
        inputs.first().copied(),
        Some("exec-command"),
        "the command went with Exec: {inputs:?}"
    );
    assert_eq!(
        inputs.get(1).copied(),
        Some("open-shell-1"),
        "the shell's terminal, not the agent's: {inputs:?}"
    );
    assert_eq!(inputs.last(), Some(&"close"), "{inputs:?}");

    // `sandbox attach --shell 1` finds the same terminal again.
    let mut again = attach_with(&harness, &pty, &["--shell", "1"]);
    wait_for_raw(&mut again, &pty);
    pty.write(&[0x04]);
    assert_eq!(wait_for_exit(&mut again, &pty), 0);
    assert_eq!(
        server
            .terminal_inputs()
            .iter()
            .filter(|kind| **kind == "open-shell-1")
            .count(),
        2,
        "{:?}",
        server.terminal_inputs()
    );
}

/// Startet `humanitl go -- bash` an diesem Terminal, mit `extra` vor dem
/// Unterkommando und `no_color` als `NO_COLOR` (HUM-496).
fn go_at(harness: &Harness, pty: &Pty, extra: &[&str], no_color: Option<&str>) -> Child {
    let mut command = harness.command();
    command
        .args(extra)
        .args(["go", "--", "bash"])
        .env_remove("NO_COLOR")
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio());
    if let Some(value) = no_color {
        command.env("NO_COLOR", value);
    }
    command.spawn().expect("the binary starts")
}

/// What the boot animation enters and gives back (`src/splash/mod.rs`).
const SPLASH_ENTER: &str = "\x1b[?1049h\x1b[?25l";
const SPLASH_RESTORE: &str = "\x1b[0m\x1b[?25h\x1b[?1049l";

/// `humanitl go -- bash` at a terminal (HUM-496): the boot animation stands
/// at least one second although the fake starts at once, the lines of the
/// start come after it on the ordinary screen, the session's terminal is
/// handed over, and colours, cursor and screen come back.
#[test]
fn go_shows_the_splash_for_a_second_and_gives_the_terminal_back() {
    let harness = Harness::new();
    // Ein bekanntes Projekt: Ein neues bekäme zuerst den Erststart (#9).
    let server = FakeServer::start_with_known_project(&harness, &work(&harness));
    let pty = Pty::open();

    let begun = Instant::now();
    let mut child = go_at(&harness, &pty, &[], None);
    wait_for_raw(&mut child, &pty);
    let took = begun.elapsed();
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());

    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(
        took >= Duration::from_secs(1),
        "the terminal was handed over after {took:?}, before the splash stood one second"
    );
    let enter = said.find(SPLASH_ENTER).expect("the splash was shown");
    let leave = said
        .find(SPLASH_RESTORE)
        .expect("the splash gave the terminal back");
    assert!(enter < leave);
    assert!(
        said.contains("\x1b[38;2;"),
        "the fist is yellow in true colour"
    );
    // Held while the splash stood, shown after it: the three guarantees and
    // the detach key are on the ordinary screen.
    let check = said
        .find("no network interface")
        .expect("the checks are shown");
    let detach = said
        .find("Detach: Ctrl+B d")
        .expect("the attach is announced");
    assert!(leave < check && leave < detach, "{said:?}");
    assert_eq!(server.terminal_inputs().first(), Some(&"open"));
}

/// Under `NO_COLOR` the splash has no colour; `--terminal-splash=false`
/// turns it off (HUM-496).
#[test]
fn go_without_colour_and_without_splash() {
    for (extra, no_color, splash, colour) in [
        (&[][..], Some("1"), true, false),
        (&["--terminal-splash=false"][..], None, false, false),
    ] {
        let harness = Harness::new();
        let _server = FakeServer::start_with_known_project(&harness, &work(&harness));
        let pty = Pty::open();
        let mut child = go_at(&harness, &pty, extra, no_color);
        wait_for_raw(&mut child, &pty);
        pty.write(b"\x02d");
        assert_eq!(wait_for_exit(&mut child, &pty), 0, "{extra:?}");
        let said = String::from_utf8_lossy(&pty.read()).into_owned();
        assert_eq!(
            said.contains(SPLASH_ENTER),
            splash,
            "{extra:?} {no_color:?}"
        );
        assert_eq!(
            said.contains("\x1b[38;2;"),
            colour,
            "{extra:?} {no_color:?}"
        );
        assert!(pty.is_cooked(), "{extra:?}");
    }
}

/// Das Arbeitsverzeichnis der Harness, wie der Prozess es sieht.
fn work(harness: &Harness) -> std::path::PathBuf {
    std::fs::canonicalize(harness.path("work")).expect("the work directory")
}

// ---------------------------------------------------------------------------
// Der Erststart-Bildschirm von `humanitl go` (#9)
// ---------------------------------------------------------------------------

/// Ein Pseudoterminal mit Größe: Ohne sie gäbe es statt des Bildschirms
/// `TERM_003` und die Zeile.
#[cfg(target_os = "linux")]
fn sized_pty() -> Pty {
    let pty = Pty::open();
    rustix::termios::tcsetwinsize(
        &pty.slave,
        rustix::termios::Winsize {
            ws_row: 30,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .expect("the size is set");
    pty
}

/// Alle Einstellungen des Terminals, vergleichbar.
#[cfg(target_os = "linux")]
fn termios(pty: &Pty) -> String {
    format!("{:?}", tcgetattr(pty.slave.as_fd()).expect("settings"))
}

/// Startet `humanitl go -- bash` an diesem Terminal, mit `TERM` und ohne
/// `NO_COLOR`.
#[cfg(target_os = "linux")]
fn go_first(harness: &Harness, pty: &Pty) -> Child {
    harness
        .command()
        .args(["go", "--", "bash"])
        .env("TERM", "xterm-256color")
        .env_remove("NO_COLOR")
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts")
}

/// Wartet, bis `text` in der Ausgabe steht; mit Frist, und ein Lauf, der
/// vorher endet, ist ein Fehler mit seiner Ausgabe.
#[cfg(target_os = "linux")]
fn wait_for_text(child: &mut Child, pty: &Pty, text: &str) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        let said = String::from_utf8_lossy(&pty.read()).into_owned();
        if said.contains(text) {
            return;
        }
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            panic!("the command ended with {status:?} before {text:?}: {said:?}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{text:?} did not appear within {PATIENCE:?}: {said:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Wo `needle` zuletzt in `haystack` steht.
#[cfg(target_os = "linux")]
fn last(haystack: &str, needle: &str) -> Option<usize> {
    haystack.rfind(needle)
}

/// Erster `go` in einem neuen Ordner an einem Terminal: der Bildschirm,
/// `Enter` genügt, die Wahl ist am Ordner gespeichert und der Agent
/// startet. Der zweite `go` im selben Ordner zeigt keinen Bildschirm.
///
/// Mutation proof: dropping the `save` call after `Exit::Start` (the screen
/// ends, nothing is stored) lets the second run show the screen again, and
/// the test fails on `saved_paths` and on `Customize` in the second output.
#[cfg(target_os = "linux")]
#[test]
fn go_first_start_enter_saves_and_the_second_go_shows_no_screen() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);

    let pty = sized_pty();
    let before = termios(&pty);
    let mut child = go_first(&harness, &pty);
    wait_for_text(&mut child, &pty, "Customize");
    assert!(
        server.starts().is_empty(),
        "nothing starts under the screen"
    );
    pty.write(b"\r");
    wait_for_text(&mut child, &pty, "Detach: Ctrl+B d");
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert_eq!(termios(&pty), before, "the terminal is as before");
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(
        !said.contains("first start in"),
        "the screen, not the line: {said:?}"
    );
    assert_eq!(server.saved_paths(), [work(&harness)]);
    assert_eq!(server.starts().len(), 1, "{:?}", server.starts());

    let pty = sized_pty();
    let mut child = go_first(&harness, &pty);
    wait_for_text(&mut child, &pty, "Detach: Ctrl+B d");
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(!said.contains("Customize"), "a second screen: {said:?}");
    assert!(!said.contains("first start in"), "a line: {said:?}");
    assert_eq!(server.saved_choices(), 1, "the second start saves nothing");
    assert_eq!(server.starts().len(), 2, "{:?}", server.starts());
}

/// `Esc` und `q` auf dem Bildschirm: nichts startet, nichts ist gespeichert,
/// Exit 130, und das Terminal steht wie vorher: dieselben `termios`, und
/// alternativer Bildschirm, versteckter Cursor und Einfügeklammern sind nach
/// ihrem letzten Einschalten wieder aus (wie in `tests/tui.rs`, #6).
///
/// Mutation proof: mapping `Esc` to `Exit::Start` saves and starts, and the
/// counters fail.
#[cfg(target_os = "linux")]
#[test]
fn go_first_start_escape_starts_nothing_and_restores_the_terminal() {
    for key in [&b"\x1b"[..], b"q"] {
        let harness = Harness::new();
        let server = FakeServer::start(&harness);
        let pty = sized_pty();
        let before = termios(&pty);
        let mut child = go_first(&harness, &pty);
        wait_for_text(&mut child, &pty, "Customize");
        pty.write(key);
        assert_eq!(wait_for_exit(&mut child, &pty), 130, "{key:?}");
        assert_eq!(termios(&pty), before, "{key:?}: the terminal is as before");
        let said = String::from_utf8_lossy(&pty.read()).into_owned();
        for (what, on, off) in [
            ("the alternate screen", "\x1b[?1049h", "\x1b[?1049l"),
            ("the hidden cursor", "\x1b[?25l", "\x1b[?25h"),
            ("bracketed paste", "\x1b[?2004h", "\x1b[?2004l"),
        ] {
            let on_at = last(&said, on).unwrap_or_else(|| panic!("{what} was never on"));
            assert!(
                last(&said, off).is_some_and(|off_at| off_at > on_at),
                "{key:?}: {what} is still on: {said:?}"
            );
        }
        assert!(said.contains("not started; nothing was saved"), "{said:?}");
        assert!(server.starts().is_empty(), "{key:?}: {:?}", server.starts());
        assert_eq!(server.saved_choices(), 0, "{key:?}");
    }
}

/// Ein Fake, der jedes `save_choice` eine Sekunde hält: Zeit für eine Taste
/// oder ein Signal zwischen Bildschirm und Start. Mit `cloud` fragt `go`
/// danach nach der Cloud-Inferenz (HUM-516).
#[cfg(target_os = "linux")]
fn slow_saving(harness: &Harness, cloud: bool) -> FakeServer {
    FakeServer::start_with_first_start(
        harness,
        FirstStartTweaks {
            slow_save: Some(Duration::from_secs(1)),
            cloud,
            ..FirstStartTweaks::default()
        },
    )
}

/// Der Bildschirm hat das Terminal zurückgegeben: Seine Umschaltung aus dem
/// alternativen Bildschirm steht nach „Customize".
#[cfg(target_os = "linux")]
fn wait_for_screen_end(child: &mut Child, pty: &Pty) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        let said = String::from_utf8_lossy(&pty.read()).into_owned();
        if let Some(screen) = said.find("Customize")
            && said[screen..].contains("\x1b[?1049l")
        {
            return;
        }
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            panic!("the command ended with {status:?} under the screen: {said:?}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the screen did not end within {PATIENCE:?}: {said:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Ein zweites `Enter`, das während des Speicherns kommt, beantwortet die
/// Frage zur Cloud-Inferenz nicht und erreicht den Agenten nicht: Die
/// Eingabe wird erst nach dem Speichern verworfen, unmittelbar vor der
/// Übergabe (#9, Review). Die Frage wartet auf die Antwort des Menschen.
///
/// Mutation proof: dropping `flush_input` after the save (or flushing before
/// it, as before the review) lets the second `Enter` answer the question
/// with no; the command ends with "not confirmed" before `y` is typed.
#[cfg(target_os = "linux")]
#[test]
fn go_first_start_a_second_enter_during_the_save_reaches_nobody() {
    let harness = Harness::new();
    let server = slow_saving(&harness, true);
    let pty = sized_pty();
    let mut child = go_first(&harness, &pty);
    wait_for_text(&mut child, &pty, "Customize");
    pty.write(b"\r");
    wait_for_screen_end(&mut child, &pty);
    pty.write(b"\r");
    wait_for_text(&mut child, &pty, "[y/N]");
    // Hätte das zweite `Enter` geantwortet, endete der Lauf jetzt; eine
    // kurze, feste Frist reicht, das zu sehen.
    let settle = Instant::now() + Duration::from_millis(500);
    while Instant::now() < settle {
        pty.drain();
        std::thread::sleep(Duration::from_millis(10));
    }
    pty.write(b"y\r");
    wait_for_text(&mut child, &pty, "Detach: Ctrl+B d");
    // Was durchgereicht würde, käme gleich nach dem Anschluss; eine kurze,
    // feste Frist reicht, ihn zu sehen.
    let settle = Instant::now() + Duration::from_millis(500);
    while Instant::now() < settle {
        pty.drain();
        std::thread::sleep(Duration::from_millis(10));
    }
    let inputs = server.terminal_inputs();
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(
        !inputs.contains(&"data"),
        "the second Enter reached the agent: {inputs:?}"
    );
    assert_eq!(server.saved_paths(), [work(&harness)]);
}

/// `SIGTERM` während des Speicherns: Die Merker des Bildschirms fangen es,
/// und der Befehl endet nach dem Speichern mit 143, ohne zu starten (#9,
/// Review).
///
/// Mutation proof: without the look at `signals::take()` after the save the
/// signal is lost, the session starts, and the exit code is not 143.
#[cfg(target_os = "linux")]
#[test]
fn go_first_start_a_signal_during_the_save_starts_nothing() {
    let harness = Harness::new();
    let server = slow_saving(&harness, false);
    let pty = sized_pty();
    let before = termios(&pty);
    let mut child = go_first(&harness, &pty);
    wait_for_text(&mut child, &pty, "Customize");
    pty.write(b"\r");
    wait_for_screen_end(&mut child, &pty);
    signal(&child, rustix::process::Signal::TERM);
    assert_eq!(wait_for_exit(&mut child, &pty), 143);
    assert!(server.starts().is_empty(), "{:?}", server.starts());
    assert_eq!(termios(&pty), before);
}

// ---------------------------------------------------------------------------
// Die Einstellungen über dem Agenten (GitHub #11)
// ---------------------------------------------------------------------------

/// Wartet, bis `text` hinter der Stelle `from` der Ausgabe steht, und gibt
/// zurück, wo; mit Frist, wie [`wait_for_text`].
#[cfg(target_os = "linux")]
fn wait_for_after(child: &mut Child, pty: &Pty, from: usize, text: &str) -> usize {
    let deadline = Instant::now() + PATIENCE;
    loop {
        let all = pty.read();
        let tail = &all[from.min(all.len())..];
        if let Some(found) = tail
            .windows(text.len())
            .position(|window| window == text.as_bytes())
        {
            return from + found;
        }
        let said = String::from_utf8_lossy(tail).into_owned();
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            panic!("the command ended with {status:?} before {text:?}: {said:?}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{text:?} did not appear within {PATIENCE:?}: {said:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Was die Fußzeile ohne Hinweis sagt (`prefix::settings::RUNNING`).
#[cfg(target_os = "linux")]
const fn super_running() -> &'static str {
    "agent keeps running"
}

/// Eine kurze, feste Pause, in der der Test weiterliest.
#[cfg(target_os = "linux")]
fn settle(pty: &Pty, pause: Duration) {
    let until = Instant::now() + pause;
    while Instant::now() < until {
        pty.drain();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Die Nummern aller `tick N` in `bytes`, in ihrer Reihenfolge.
#[cfg(target_os = "linux")]
fn ticks(bytes: &[u8]) -> Vec<u64> {
    String::from_utf8_lossy(bytes)
        .split("tick ")
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// `Ctrl+B ,` öffnet die Einstellungen über einem Vollbild-Agenten, der sein
/// Bild nur bei einer echten Größenänderung neu zeichnet (wie auf
/// `SIGWINCH`), und `Esc` kehrt zurück (GitHub #11):
///
/// - Die Einstellungen stehen, und in ihrer Fußzeile der Hinweis des
///   Daemons auf eine gehaltene Anfrage.
/// - Keine Taste aus der Ansicht erreicht den Agenten.
/// - Die Ausgabe des Agenten während der Ansicht kommt nach der Rückkehr,
///   vollständig und in ihrer Reihenfolge, und nicht vorher.
/// - Nach der Rückkehr steht das Bild des Agenten wieder, in der echten
///   Größe, und er nimmt Tasten an.
///
/// Mutationsbeweis: ohne die Größenänderung bei der Rückkehr (oder mit
/// derselben Größe statt einer Zeile weniger) zeichnet der Agent nicht neu,
/// und der Test wird rot.
#[cfg(target_os = "linux")]
#[test]
fn ctrl_b_comma_opens_the_settings_and_the_agent_stands_after_esc() {
    let harness = Harness::new();
    let server = FakeServer::start_with_winch_agent(&harness);
    let pty = sized_pty();
    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    wait_for_text(&mut child, &pty, "SCREEN 1 100x30");
    // Ein Hinweis vor der Ansicht (an der zehnten Zeile): Er steht von Anfang
    // an in ihrer Fußzeile, aber nicht im Terminal des Agenten (GitHub #12).
    wait_for_text(&mut child, &pty, "tick 11\r\n");
    assert!(
        !String::from_utf8_lossy(&pty.read()).contains("example.test/first"),
        "a notice reached the agent's terminal"
    );

    let before = pty.read().len();
    pty.write(b"\x02,");
    let opened = wait_for_after(&mut child, &pty, before, "Settings");
    // Der Hinweis kommt an jeder zehnten Zeile, also während der Ansicht.
    // Der zweite Hinweis kommt während der Ansicht und steht in der
    // Fußzeile; danach kommt keiner mehr.
    wait_for_after(&mut child, &pty, opened, "second");
    // Tasten der Ansicht, auch eine Suche und ihr Abbruch; das zweite `Esc`
    // kehrt zurück. Jedes `Esc` kommt allein, damit es keine Folge beginnt.
    pty.write(b"jjk");
    settle(&pty, Duration::from_millis(150));
    pty.write(b"/ab");
    settle(&pty, Duration::from_millis(150));
    // Unter Last kann crossterm zwei `Esc` in einem Lesevorgang als ein
    // `Alt+Esc` lesen; dann braucht es eines mehr. Höchstens fünf, und nach
    // der Rückkehr keines mehr, damit keines den Agenten erreicht.
    let leave: &[u8] = b"\x1b[0m\x1b[H\x1b[2J\x1b8";
    for _ in 0..5 {
        pty.write(b"\x1b");
        settle(&pty, Duration::from_millis(400));
        if pty.read()[opened..]
            .windows(leave.len())
            .any(|window| window == leave)
        {
            break;
        }
    }
    let left = wait_for_after(&mut child, &pty, opened, "\x1b[0m\x1b[H\x1b[2J\x1b8");
    let redrawn = wait_for_after(&mut child, &pty, left, "SCREEN 3 100x30");

    let all = pty.read();
    assert!(
        String::from_utf8_lossy(&all[left..redrawn]).contains("100x29"),
        "one row less first: {:?}",
        String::from_utf8_lossy(&all[left..])
    );
    let during = ticks(&all[opened..left]);
    assert!(
        during.is_empty(),
        "output during the view reached the screen: {during:?}"
    );
    let numbers = ticks(&all);
    assert!(
        numbers.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "output lost or out of order: {numbers:?}"
    );
    let last_before = ticks(&all[..opened])
        .last()
        .copied()
        .expect("ticks before the view");
    let first_after = ticks(&all[left..])
        .first()
        .copied()
        .expect("ticks after the view");
    assert_eq!(first_after, last_before + 1, "the held output follows on");
    let held = ticks(&all[left..redrawn]).len();
    assert!(
        held >= 5,
        "the agent kept running during the view: {held} lines held"
    );
    let view = String::from_utf8_lossy(&all[before..left]);
    assert!(
        !view.contains(super_running()),
        "the footer started without the notice seen before the view"
    );
    assert!(
        view.contains("example.test/first"),
        "the footer shows the notice that came before the view"
    );
    assert!(
        !String::from_utf8_lossy(&all[left..]).contains("example.test/second"),
        "a notice the footer showed was replayed after the view"
    );

    // Der Agent ist wieder zu bedienen.
    pty.write(b"z");
    wait_for_after(&mut child, &pty, redrawn, "echo:z");
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());
    let inputs = server.terminal_inputs();
    assert_eq!(
        inputs.iter().filter(|kind| **kind == "data").count(),
        1,
        "a key of the view reached the agent: {inputs:?}"
    );
}

/// Ein lesender Anschluss über einem Agenten auf dem Alternativschirm
/// öffnet die Einstellungen nicht: Er könnte den Agenten danach nicht neu
/// zeichnen lassen. Ein `BEL` sagt es, und der Anschluss bleibt bedienbar
/// (Review zu GitHub #11). Mutationsbeweis: ohne die Prüfung öffnen sich die
/// Einstellungen, und der Test wird rot.
#[cfg(target_os = "linux")]
#[test]
fn read_only_over_a_full_screen_agent_rings_instead_of_opening_the_settings() {
    let harness = Harness::new();
    let _server = FakeServer::start_with_winch_agent(&harness);
    let pty = sized_pty();
    let mut child = attach_with(&harness, &pty, &["--read-only"]);
    wait_for_raw(&mut child, &pty);
    wait_for_text(&mut child, &pty, "SCREEN 1 100x30");
    let before = pty.read().len();
    pty.write(b"\x02,");
    wait_for_after(&mut child, &pty, before, "\x07");
    settle(&pty, Duration::from_millis(500));
    let said = pty.read();
    assert!(
        !String::from_utf8_lossy(&said[before..]).contains("Settings"),
        "the settings opened over a full-screen agent without a redraw"
    );
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());
}

/// Tasten, die ohne Pause weiterkommen, halten die Rückkehr zum Agenten
/// nicht auf: Nach `Ctrl+C` in den Einstellungen steht der Agent wieder,
/// während der Strom noch läuft (Review zu GitHub #11). Mutationsbeweis: ein
/// Verwerfen ohne Grenze kehrt erst nach Sekunden zurück, und der
/// Test wird rot.
#[cfg(target_os = "linux")]
#[test]
fn the_return_happens_while_keys_keep_coming() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    const STREAM: Duration = Duration::from_secs(4);
    // Ohne Strom dauert die Rückkehr Millisekunden; die Grenze lässt Platz
    // für einen ausgelasteten Rechner und liegt weit unter dem Strom.
    const RETURN_WITHIN: Duration = Duration::from_millis(1500);
    let harness = Harness::new();
    let _server = FakeServer::start_with_winch_agent(&harness);
    let pty = sized_pty();
    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    wait_for_text(&mut child, &pty, "SCREEN 1 100x30");
    let before = pty.read().len();
    pty.write(b"\x02,");
    let opened = wait_for_after(&mut child, &pty, before, "Settings");

    let master = pty
        .master
        .as_ref()
        .expect("the master is open")
        .try_clone()
        .expect("a second descriptor for the stream of keys");
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let keys = std::thread::spawn({
        let stop = Arc::clone(&stop);
        let done = Arc::clone(&done);
        move || {
            // Erst `Ctrl+C`, dann ohne Pause weitere Tasten, aus demselben
            // Faden: Die Ansicht schließt, während der Strom läuft.
            let until = Instant::now() + STREAM;
            let stream = [b'j'; 4096];
            let mut interrupted = false;
            while !stop.load(Ordering::SeqCst) && Instant::now() < until {
                let keys: &[u8] = if interrupted { &stream } else { b"\x03" };
                match rustix::io::write(&master, keys) {
                    Ok(_) => interrupted = true,
                    Err(rustix::io::Errno::AGAIN) => std::thread::yield_now(),
                    Err(_) => break,
                }
            }
            done.store(true, Ordering::SeqCst);
        }
    });
    let left = wait_for_after(&mut child, &pty, opened, "\x1b[0m\x1b[H\x1b[2J\x1b8");
    let took = started.elapsed();
    let returned_while_streaming = !done.load(Ordering::SeqCst);
    stop.store(true, Ordering::SeqCst);
    keys.join().expect("the stream of keys ends");
    assert!(
        returned_while_streaming && took < RETURN_WITHIN,
        "the return waited for the keys: {took:?}"
    );
    wait_for_after(&mut child, &pty, left, "SCREEN 3 100x30");
    assert!(
        pty.write_within(b"\x02d", Instant::now() + PATIENCE),
        "the detach key was not taken"
    );
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
}

// ---------------------------------------------------------------------------
// Der Interceptor über dem Agenten (GitHub #12)
// ---------------------------------------------------------------------------

/// `Ctrl+B i` öffnet den Interceptor nach den Regeln der Einstellungen:
/// Keine Taste aus der Ansicht erreicht den Agenten (auch `a` und `b` nicht,
/// die dort entscheiden), seine Ausgabe während der Ansicht kommt erst nach
/// der Rückkehr, und danach zeichnet er neu und nimmt Tasten an.
/// Mutationsbeweis: den Interceptor als Kasten öffnen (`takes_screen` ohne
/// ihn) gibt die Ausgabe nach der ersten Taste frei, und der Test wird rot.
#[cfg(target_os = "linux")]
#[test]
fn ctrl_b_i_opens_the_interceptor_and_the_agent_stands_after_esc() {
    let harness = Harness::new();
    let server = FakeServer::start_with_winch_agent(&harness);
    let pty = sized_pty();
    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    wait_for_text(&mut child, &pty, "SCREEN 1 100x30");

    let before = pty.read().len();
    pty.write(b"\x02i");
    let opened = wait_for_after(&mut child, &pty, before, "Interceptor");
    settle(&pty, Duration::from_millis(300));
    pty.write(b"jab");
    settle(&pty, Duration::from_millis(300));
    let leave: &[u8] = b"\x1b[0m\x1b[H\x1b[2J\x1b8";
    for _ in 0..5 {
        pty.write(b"\x1b");
        settle(&pty, Duration::from_millis(400));
        if pty.read()[opened..]
            .windows(leave.len())
            .any(|window| window == leave)
        {
            break;
        }
    }
    let left = wait_for_after(&mut child, &pty, opened, "\x1b[0m\x1b[H\x1b[2J\x1b8");
    let redrawn = wait_for_after(&mut child, &pty, left, "SCREEN 3 100x30");
    let all = pty.read();
    let during = ticks(&all[opened..left]);
    assert!(
        during.is_empty(),
        "output during the view reached the screen: {during:?}"
    );
    let numbers = ticks(&all);
    assert!(
        numbers.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "output lost or out of order: {numbers:?}"
    );

    pty.write(b"z");
    wait_for_after(&mut child, &pty, redrawn, "echo:z");
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());
    let inputs = server.terminal_inputs();
    assert_eq!(
        inputs.iter().filter(|kind| **kind == "data").count(),
        1,
        "a key of the interceptor reached the agent: {inputs:?}"
    );
}

/// Ein echtes `FlowEvent.held` aus `Subscribe` läuft durch die Schleife des
/// Anschlusses und gibt genau ein `BEL` auf dem Terminal, nie innerhalb einer
/// CSI-Folge des Agenten, der währenddessen weiter schreibt; Text erscheint
/// dafür keiner (GitHub #12). Mutationsbeweis: ohne den Zweig der gehaltenen
/// Anfragen klingelt nichts, und der Test wird rot.
#[cfg(target_os = "linux")]
#[test]
fn a_held_request_rings_exactly_once_between_chunks() {
    let harness = Harness::new();
    let _server = FakeServer::start_with_winch_agent_and_a_hold(&harness);
    let pty = sized_pty();
    let mut child = attach(&harness, &pty);
    wait_for_raw(&mut child, &pty);
    wait_for_text(&mut child, &pty, "SCREEN 1 100x30");
    wait_for_text(&mut child, &pty, "\x07");
    // Weiterlesen, während der Agent schreibt: Eine zweite Glocke käme hier.
    settle(&pty, Duration::from_millis(1500));
    let all = pty.read();
    let bells = all.split(|byte| *byte == 0x07).count() - 1;
    assert_eq!(bells, 1, "{:?}", String::from_utf8_lossy(&all));
    let at = all.iter().position(|byte| *byte == 0x07).expect("the bell");
    let before = &all[..at];
    let csi_open = before
        .windows(2)
        .rposition(|pair| pair == b"\x1b[")
        .is_some_and(|start| {
            !before[start + 2..]
                .iter()
                .any(|b| (0x40..=0x7e).contains(b))
        });
    assert!(!csi_open, "the bell stands inside an ESC [ sequence");
    assert!(
        !String::from_utf8_lossy(&all).contains("held-by-the-fake"),
        "text about the hold reached the agent's terminal"
    );
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());
}

// ---------------------------------------------------------------------------
// Das Schließen des Terminals von `humanitl go` (GitHub #13)
// ---------------------------------------------------------------------------

/// Wartet höchstens `limit` auf das Ende des Laufs: `None`, wenn er dann
/// noch steht; er wird danach beendet, damit keine Waise bleibt.
#[cfg(target_os = "linux")]
fn exit_within(child: &mut Child, pty: &Pty, limit: Duration) -> Option<i32> {
    let deadline = Instant::now() + limit;
    loop {
        pty.drain();
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            return Some(status.code().unwrap_or(-1));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Der Zustand der Sitzung, wie `humanitl --json sessions` ihn vom Daemon
/// liest.
#[cfg(target_os = "linux")]
fn session_state(harness: &Harness) -> String {
    let output = harness
        .run_bounded(["--json", "sessions"], PATIENCE)
        .expect("sessions ends");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("sessions answers in JSON");
    value["state"].as_str().expect("a state").to_owned()
}

/// Ein `SIGHUP` an diesem Lauf, und danach geht das Terminal weg wie bei
/// einem geschlossenen Fenster: Der Lauf endet binnen 2 s mit 129. Den
/// Zustand des Terminals danach kann niemand mehr prüfen; dass er endet,
/// heißt, dass er nichts in ein Terminal schrieb, das es nicht mehr gibt,
/// und dass nichts ihn am Leben hält.
#[cfg(target_os = "linux")]
fn hang_up(child: &mut Child, pty: &mut Pty) {
    signal(child, rustix::process::Signal::HUP);
    pty.close_master();
    assert_eq!(
        exit_within(child, pty, Duration::from_secs(2)),
        Some(129),
        "the command did not end within 2 s of SIGHUP with 129"
    );
}

/// `SIGHUP` an `humanitl go` mit der Vorgabe `terminal.on_close =
/// "detach"`: Der Befehl endet, die Sitzung läuft weiter, kein
/// `Sandbox(Stop)`. Ein zweites `go` im selben Ordner hängt sich an sie an,
/// ohne Ladeanimation und ohne zweiten Start.
///
/// Mutation proof: inverting the `on_close` condition in `OnSignal::stops`
/// sends a stop here and fails `stops() == 0`, and fails the `stop` test
/// below; dropping the running check in `go::rejoin` starts a second time.
#[cfg(target_os = "linux")]
#[test]
fn sighup_detaches_go_and_a_second_go_attaches() {
    let harness = Harness::new();
    let server = FakeServer::start_tracking_sessions(&harness, &work(&harness));
    let mut pty = Pty::open();
    let mut child = go_at(&harness, &pty, &[], None);
    wait_for_raw(&mut child, &pty);
    hang_up(&mut child, &mut pty);

    assert_eq!(server.stops(), 0, "SIGHUP stopped the session");
    assert_eq!(server.starts().len(), 1);
    assert_eq!(session_state(&harness), "running");

    let pty = Pty::open();
    let mut again = go_at(&harness, &pty, &[], None);
    wait_for_raw(&mut again, &pty);
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut again, &pty), 0);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert!(
        said.contains("Attached to the running session in work"),
        "{said:?}"
    );
    assert!(
        !said.contains(SPLASH_ENTER),
        "no splash for an attach: {said:?}"
    );
    assert_eq!(server.starts().len(), 1, "the second go started nothing");
    assert_eq!(server.stops(), 0);
    assert_eq!(
        server
            .terminal_inputs()
            .iter()
            .filter(|kind| **kind == "open")
            .count(),
        2,
        "{:?}",
        server.terminal_inputs()
    );
}

/// `terminal.on_close = "stop"`: `SIGHUP` an `humanitl go` stoppt die
/// Sitzung wie vor GitHub #13.
#[cfg(target_os = "linux")]
#[test]
fn sighup_stops_go_with_on_close_stop() {
    let harness = Harness::new();
    let server = FakeServer::start_tracking_sessions(&harness, &work(&harness));
    let mut pty = Pty::open();
    let mut child = go_at(&harness, &pty, &["--terminal-on-close", "stop"], None);
    wait_for_raw(&mut child, &pty);
    hang_up(&mut child, &mut pty);

    assert_eq!(server.stops(), 1, "SIGHUP did not stop the session");
    assert_eq!(session_state(&harness), "stopped");
}

/// `humanitl run -- bash` an einem Terminal stoppt auf `SIGHUP`, auch mit
/// `terminal.on_close = "detach"`: Der Schlüssel gilt nur für `go`.
///
/// Mutation proof: evaluating the key for `run` too (dropping `self.go &&`
/// in `OnSignal::stops`) sends no stop and fails this test.
#[cfg(target_os = "linux")]
#[test]
fn sighup_stops_run_whatever_the_key_says() {
    let harness = Harness::new();
    let server = FakeServer::start_tracking_sessions(&harness, &work(&harness));
    let mut pty = Pty::open();
    let mut child = harness
        .command()
        .args(["--terminal-on-close", "detach", "run", "--", "bash"])
        .stdin(pty.stdio())
        .stdout(pty.stdio())
        .stderr(pty.stdio())
        .spawn()
        .expect("the binary starts");
    wait_for_raw(&mut child, &pty);
    hang_up(&mut child, &mut pty);

    assert_eq!(server.stops(), 1, "run left its session running");
}

/// Ein Lauf, der seine eigene Sitzung nur noch lesend sieht, weil schon ein
/// anderer Client schreibt (die Oberfläche etwa, `TERM_001`), besitzt sie
/// weiter: `SIGHUP` an `run` und an `go` mit `terminal.on_close = "stop"`
/// stoppt sie (GitHub #13, Review).
///
/// Mutation proof: passing `false` for the owner in `drive` (the rule of
/// HUM-280, a watcher never stops) sends no stop and fails both rows.
#[cfg(target_os = "linux")]
#[test]
fn a_run_that_only_watches_still_stops_its_own_session() {
    for (extra, sub) in [
        (&[][..], &["run", "--", "bash"][..]),
        (
            &["--terminal-on-close", "stop"][..],
            &["go", "--", "bash"][..],
        ),
    ] {
        let harness = Harness::new();
        let server = FakeServer::start_tracking(
            &harness,
            &work(&harness),
            Tracking {
                writer_taken: true,
                ..Tracking::default()
            },
        );
        let mut pty = Pty::open();
        let mut child = harness
            .command()
            .args(extra)
            .args(sub)
            .stdin(pty.stdio())
            .stdout(pty.stdio())
            .stderr(pty.stdio())
            .spawn()
            .expect("the binary starts");
        wait_for_raw(&mut child, &pty);
        hang_up(&mut child, &mut pty);
        assert_eq!(
            server.stops(),
            1,
            "{sub:?}: the watching run left its session"
        );
    }
}

/// Ein zweites `go` hängt sich nur an die Sitzung, die der Daemon für den
/// Ordner genannt hat: Ist sie inzwischen eine andere, lehnt der Anschluss
/// mit `IPC_006` ab, und nichts startet oder stoppt (GitHub #13, Review).
///
/// Mutation proof: attaching with an empty sandbox id in `go::rejoin`
/// attaches to whatever runs and fails the exit and `IPC_006` assertions.
#[cfg(target_os = "linux")]
#[test]
fn a_second_go_attaches_only_to_the_session_the_daemon_named() {
    let harness = Harness::new();
    let server = FakeServer::start_tracking(
        &harness,
        &work(&harness),
        Tracking {
            stale_sandbox: true,
            ..Tracking::default()
        },
    );
    let pty = Pty::open();
    let mut child = go_at(&harness, &pty, &[], None);
    wait_for_raw(&mut child, &pty);
    pty.write(b"\x02d");
    assert_eq!(wait_for_exit(&mut child, &pty), 0);

    let pty = Pty::open();
    let mut again = go_at(&harness, &pty, &[], None);
    let code = wait_for_exit(&mut again, &pty);
    let said = String::from_utf8_lossy(&pty.read()).into_owned();
    assert_ne!(code, 0, "{said:?}");
    assert!(said.contains("IPC_006"), "{said:?}");
    assert_eq!(server.starts().len(), 1, "the second go started nothing");
    assert_eq!(server.stops(), 0);
    assert!(pty.is_cooked(), "modes {:?}", pty.modes());
}

/// Ein zweites `go` hat die Sitzung nicht gestartet und stoppt sie nie:
/// weder auf `SIGTERM` noch mit `terminal.on_close = "stop"` auf `SIGHUP`
/// (GitHub #13, Entscheidung aus dem Review). Es hängt nur ab.
///
/// Mutation proof: `REJOINED_OWNS = true` in `go::rejoin` sends a stop and
/// fails both rows.
#[cfg(target_os = "linux")]
#[test]
fn a_rejoined_go_never_stops_the_session() {
    for (extra, number, code) in [
        (&[][..], rustix::process::Signal::TERM, 143),
        (
            &["--terminal-on-close", "stop"][..],
            rustix::process::Signal::HUP,
            129,
        ),
    ] {
        let harness = Harness::new();
        let server = FakeServer::start_tracking_sessions(&harness, &work(&harness));
        let pty = Pty::open();
        let mut first = go_at(&harness, &pty, &[], None);
        wait_for_raw(&mut first, &pty);
        pty.write(b"\x02d");
        assert_eq!(wait_for_exit(&mut first, &pty), 0);

        let pty = Pty::open();
        let mut again = go_at(&harness, &pty, extra, None);
        wait_for_raw(&mut again, &pty);
        let said = String::from_utf8_lossy(&pty.read()).into_owned();
        assert!(said.contains("Attached to the running session"), "{said:?}");
        signal(&again, number);
        assert_eq!(
            exit_within(&mut again, &pty, Duration::from_secs(2)),
            Some(code),
            "{extra:?}"
        );
        assert_eq!(server.starts().len(), 1, "{extra:?}");
        assert_eq!(
            server.stops(),
            0,
            "{extra:?}: the rejoined go stopped the session"
        );
        assert_eq!(session_state(&harness), "running", "{extra:?}");
    }
}
