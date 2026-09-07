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

/// Ein Pseudoterminal: der Master, den der Test hält, und der Slave, den der
/// Lauf als sein Terminal bekommt.
struct Pty {
    /// Die Seite des Tests, ohne Blockade. `None`, sobald sie geschlossen ist;
    /// für den Lauf endet damit die Eingabe.
    master: Option<OwnedFd>,
    /// Die Seite des Laufs. Der Test hält eine Kopie, um `tcgetattr` zu rufen.
    slave: OwnedFd,
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
        let master =
            openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC).expect("a pty");
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
        }
    }

    /// Schließt die Seite des Tests -- derselbe Weg, den ein Mensch nimmt, der
    /// sein Terminal schließt.
    fn close_master(&mut self) {
        self.master = None;
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
    harness
        .command()
        .args(["sandbox", "attach"])
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
