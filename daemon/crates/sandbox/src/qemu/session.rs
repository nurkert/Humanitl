//! Was der Daemon für eine laufende Sitzung der Kernel-Stufe auf dem Host
//! hält (HUM-444): das Laufzeitverzeichnis, die eine Tür zum Proxy und die
//! Verbindung zum Gast über das vsock-Gerät.
//!
//! **Die Tür.** Der Daemon lauscht an genau einem Socket, `<uds>_<Tür-Port>`
//! ([`super::lines::Layout::door_listener`]); verbindet sich der Gast mit CID
//! 2 und diesem Port, verbindet das vsock-Gerät ihn dorthin, und der Daemon
//! reicht jede Verbindung an den Proxy-Socket der Sitzung weiter. Ein
//! anderer Port findet keinen Lauscher, und der Gast bekommt nach einer
//! Frist ein Timeout (HUM-442 Q2).
//!
//! **Bericht und Steuerkanal.** Der Shim im Gast lauscht auf dem
//! Berichts-Port und nimmt genau zwei Verbindungen an (HUM-465, HUM-443):
//! die erste ist der Bericht, die zweite der Steuerkanal der Terminals. Der
//! Daemon öffnet beide über `CONNECT <port>` am vsock-Gerät, schreibt den
//! Bericht in die Pipe, die der Start wie unter bwrap liest, und verbindet den
//! Steuerkanal mit der Seite des Shims am Strom-Paar ([`crate::shell::stream_pair`]),
//! dessen andere Seite die Tür des Daemons liest. Endet der Gast, bevor er
//! einen Bericht gab, steht in der Pipe [`NO_REPORT`]: eine gescheiterte
//! Prüfung mit eigenem Grund, nie ein stilles Nichts.
//!
//! Jeder Faden endet mit der Sitzung: Er wartet zugleich auf das Leseende der
//! Lebensleine, deren Schreibende der Zustand der Sandbox hält.

use std::io::{BufRead as _, BufReader, PipeWriter, Read, Write};
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, poll};

/// Die längste Zeile des Berichts, die der Daemon annimmt; eine längere
/// beendet das Lesen als gescheiterter Bericht ([`OVERLONG`]).
pub(crate) const MAX_REPORT_LINE: u64 = 64 * 1024;

/// Die Zeile für einen Bericht mit einer überlangen Zeile.
pub(crate) const OVERLONG: &str =
    "CHECK seccomp_applied fail report:a_line_of_the_guest_is_longer_than_64_KiB";

/// Die Zeile, die für einen Gast ohne Bericht steht: eine gescheiterte
/// Prüfung mit Grund.
pub(crate) const NO_REPORT: &str = "CHECK seccomp_applied fail no-report:the_guest_ended_\
    without_a_report;_its_vsock_modules_or_the_report_channel_failed";

/// Die längste Pfadangabe eines Unix-Sockets (`sun_path`).
pub(crate) const SUN_PATH_MAX: usize = 108;

/// Wie oft ein Faden nachsieht, ob die Lebensleine noch hält.
const TICK: Duration = Duration::from_millis(200);

/// Ob die Lebensleine zu ist: Ihr Schreibende ist fort, das Leseende
/// meldet `POLLHUP` oder liest das Ende.
pub(crate) fn lifeline_closed(lifeline: &OwnedFd, wait: Duration) -> bool {
    let millis = i32::try_from(wait.as_millis()).unwrap_or(i32::MAX);
    let mut fds = [PollFd::new(lifeline, PollFlags::IN)];
    let timeout = rustix::event::Timespec {
        tv_sec: i64::from(millis / 1000),
        tv_nsec: i64::from(millis % 1000) * 1_000_000,
    };
    loop {
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => return false,
            Ok(_) => return !fds[0].revents().is_empty(),
            // Ein Signal unterbricht nur das Warten, es schließt nichts.
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => return true,
        }
    }
}

/// Reicht jede Verbindung an `listener` an `proxy` weiter, bis die
/// Lebensleine zugeht.
pub(crate) fn serve_door(listener: &UnixListener, proxy: &Path, lifeline: &OwnedFd) {
    loop {
        let mut fds = [
            PollFd::new(listener, PollFlags::IN),
            PollFd::new(lifeline, PollFlags::IN),
        ];
        let timeout = rustix::event::Timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
        }
        if !fds[1].revents().is_empty() {
            return;
        }
        if fds[0].revents().is_empty() {
            continue;
        }
        let Ok((guest, _)) = listener.accept() else {
            continue;
        };
        match UnixStream::connect(proxy) {
            Ok(upstream) => splice(guest, upstream),
            Err(_) => drop(guest),
        }
    }
}

/// Kopiert in beide Richtungen, je auf einem eigenen Faden; endet eine
/// Seite, geht die andere zum Schreiben zu.
pub(crate) fn splice(a: UnixStream, b: UnixStream) {
    let (Ok(a_reader), Ok(b_reader)) = (a.try_clone(), b.try_clone()) else {
        return;
    };
    let forward = move |mut from: UnixStream, mut to: UnixStream| {
        let _ = std::io::copy(&mut from, &mut to);
        let _ = to.shutdown(std::net::Shutdown::Write);
        let _ = from.shutdown(std::net::Shutdown::Read);
    };
    let _ = thread::Builder::new()
        .name("vm-splice".to_owned())
        .spawn(move || forward(a_reader, b));
    let _ = thread::Builder::new()
        .name("vm-splice".to_owned())
        .spawn(move || forward(b_reader, a));
}

/// Was der Faden zum Gast braucht.
pub(crate) struct GuestLink {
    /// `--uds-path` des vsock-Geräts.
    pub(crate) device: PathBuf,
    /// Der Berichts-Port des Shims im Gast.
    pub(crate) report_port: u32,
    /// Bis wann der Gast antworten muss.
    pub(crate) deadline: Instant,
    /// Das Schreibende der Berichts-Pipe.
    pub(crate) report: PipeWriter,
    /// Die Seite des Shims am Strom-Paar.
    pub(crate) control: OwnedFd,
    /// Ein Leseende der Lebensleine.
    pub(crate) lifeline: OwnedFd,
    /// Wo die Verbindung des Berichts für das Urteil des Daemons hinkommt.
    pub(crate) admission: crate::launcher::Admission,
}

impl GuestLink {
    /// Verbindet Bericht und Steuerkanal, auf einem eigenen Faden.
    pub(crate) fn start(self) -> std::io::Result<()> {
        thread::Builder::new()
            .name("vm-guest-link".to_owned())
            .spawn(move || self.run())
            .map(drop)
    }

    fn run(self) {
        let Self {
            device,
            report_port,
            deadline,
            mut report,
            control,
            lifeline,
            admission,
        } = self;
        let ended = || lifeline_closed(&lifeline, Duration::ZERO);
        let Some(first) = connect(&device, report_port, deadline, &ended) else {
            let _ = writeln!(report, "{NO_REPORT}");
            return;
        };
        // Auf der Verbindung des Berichts geht später das Urteil des Daemons
        // in den Gast (`ADMIT` oder `DENY`); ohne sie startet der Agent nie.
        if let Ok(verdict) = first.try_clone() {
            admission.offer(verdict);
        }
        let second = connect(&device, report_port, deadline, &ended);
        match second {
            Some(stream) => splice(UnixStream::from(control), stream),
            None => drop(control),
        }
        copy_report(first, report);
    }
}

/// Der Bericht Zeile für Zeile in die Pipe, bis zu seinem Ende. Kam keine
/// einzige Zeile, steht [`NO_REPORT`] darin; war eine länger als
/// [`MAX_REPORT_LINE`], [`OVERLONG`], und gelesen wird nicht weiter.
pub(crate) fn copy_report(stream: impl Read, mut pipe: impl Write) {
    let mut reader = BufReader::new(stream);
    let mut any = false;
    loop {
        let mut line = Vec::new();
        match (&mut reader)
            .take(MAX_REPORT_LINE + 1)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if !line.ends_with(b"\n") && u64::try_from(line.len()).unwrap_or(u64::MAX) > MAX_REPORT_LINE
        {
            let _ = writeln!(pipe, "{OVERLONG}");
            return;
        }
        any = true;
        let text = String::from_utf8_lossy(&line);
        if writeln!(pipe, "{}", text.trim_end_matches(['\r', '\n'])).is_err() {
            return;
        }
    }
    if !any {
        let _ = writeln!(pipe, "{NO_REPORT}");
    }
}

/// Eine Verbindung zum Port des Shims über das vsock-Gerät: `CONNECT
/// <port>`, dann `OK`. Antwortet der Gast noch nicht, wird es wiederholt,
/// bis zur Frist oder bis `ended` sagt, dass die Sitzung vorbei ist. Die
/// Antwort des Geräts wird Byte für Byte gelesen: Was danach kommt, gehört
/// schon dem Shim.
pub(crate) fn connect(
    device: &Path,
    port: u32,
    deadline: Instant,
    ended: &dyn Fn() -> bool,
) -> Option<UnixStream> {
    while Instant::now() < deadline && !ended() {
        let Ok(mut stream) = UnixStream::connect(device) else {
            thread::sleep(TICK / 2);
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        if stream
            .write_all(format!("CONNECT {port}\n").as_bytes())
            .is_err()
        {
            thread::sleep(TICK / 2);
            continue;
        }
        let mut first = Vec::new();
        let mut byte = [0u8; 1];
        while first.len() < 64 && stream.read(&mut byte).ok() == Some(1) {
            if byte[0] == b'\n' {
                break;
            }
            first.push(byte[0]);
        }
        if !first.starts_with(b"OK") {
            thread::sleep(TICK / 2);
            continue;
        }
        let _ = stream.set_read_timeout(None);
        return Some(stream);
    }
    None
}

/// Ein Leseende der Lebensleine mehr, für einen Faden des Daemons.
pub(crate) fn lifeline_copy(read: &OwnedFd) -> std::io::Result<OwnedFd> {
    read.as_fd().try_clone_to_owned()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Read as _;

    use super::*;

    /// Die Tür reicht an den Proxy weiter und endet mit der Lebensleine.
    #[test]
    fn the_door_forwards_to_the_proxy_and_ends_with_the_lifeline() {
        let dir = tempfile::tempdir().unwrap();
        let proxy_path = dir.path().join("proxy.sock");
        let proxy = UnixListener::bind(&proxy_path).unwrap();
        let door_path = dir.path().join("door_5000");
        let door = UnixListener::bind(&door_path).unwrap();
        let (lifeline, read) = crate::shim_fds::Lifeline::new().unwrap();
        let server = thread::spawn(move || serve_door(&door, &proxy_path, &read));

        let mut guest = UnixStream::connect(&door_path).unwrap();
        guest.write_all(b"ping").unwrap();
        guest.shutdown(std::net::Shutdown::Write).unwrap();
        let (mut upstream, _) = proxy.accept().unwrap();
        let mut seen = Vec::new();
        upstream.read_to_end(&mut seen).unwrap();
        assert_eq!(seen, b"ping");
        upstream.write_all(b"pong").unwrap();
        drop(upstream);
        let mut back = Vec::new();
        guest.read_to_end(&mut back).unwrap();
        assert_eq!(back, b"pong");

        drop(lifeline);
        let started = Instant::now();
        server.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    /// Ohne Gast steht in der Berichts-Pipe [`NO_REPORT`], und der Faden
    /// endet mit der Lebensleine, nicht erst mit der Frist.
    #[test]
    fn without_a_guest_the_report_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let (mut reader, writer) = std::io::pipe().unwrap();
        let (_daemon, shim) = crate::shell::stream_pair(80, 24).unwrap();
        let (lifeline, read) = crate::shim_fds::Lifeline::new().unwrap();
        let link = GuestLink {
            device: dir.path().join("door"),
            report_port: 5001,
            deadline: Instant::now() + Duration::from_secs(60),
            report: writer,
            control: shim,
            lifeline: read,
            admission: crate::launcher::Admission::default(),
        };
        let started = Instant::now();
        link.start().unwrap();
        thread::sleep(Duration::from_millis(300));
        drop(lifeline);
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, format!("{NO_REPORT}\n"));
        assert!(started.elapsed() < Duration::from_secs(5));
        let check = crate::bridge_env::parse_check_line(NO_REPORT).unwrap();
        assert!(!check.ok);
    }
}
