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
//! **Erst die Bereitschaft, dann `CONNECT` (HUM-471).** Der Daemon fragt am
//! vsock-Gerät erst, wenn der Shim an der Tür gesagt hat, dass er lauscht
//! ([`GUEST_READY`]). Ein `CONNECT`, das das Gerät erreicht, während der
//! Treiber des Gasts es gerade einrichtet (der Speicher des Gasts ist
//! bekannt, die Warteschlangen sind es noch nicht), hält vhost-device-vsock
//! 0.3.0 für immer an: Sein Ereignisfaden scheitert an der noch nicht
//! bereiten Warteschlange und endet, der Prozess lebt weiter, und kein
//! späteres `CONNECT` bekommt je eine Antwort. Von außen ist dieser Moment
//! nicht zu erkennen; eine Verbindung, die der Gast selbst öffnet, gibt es
//! aber erst, wenn die Warteschlangen laufen. Bis zur Zeile reicht die Tür
//! nichts an den Proxy weiter: Vor ihr läuft im Gast noch kein Agent.
//!
//! Jeder Faden endet mit der Sitzung: Er wartet zugleich auf das Leseende der
//! Lebensleine, deren Schreibende der Zustand der Sandbox hält.

use std::io::{BufRead as _, BufReader, PipeWriter, Read, Write};
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
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

/// Die Zeile, mit der der Shim im Gast an der Tür sagt, dass er am
/// Berichts-Port lauscht (HUM-471); dieselben Bytes wie `GUEST_READY` in
/// `humanitl-shim/src/guest.rs`.
pub(crate) const GUEST_READY: &[u8] = b"HUMANITL-GUEST-READY\n";

/// Wie lange die Tür vor der Bereitschaft auf die erste Zeile einer
/// Verbindung wartet.
const READY_LINE_WAIT: Duration = Duration::from_secs(1);

/// Ob der Gast an der Tür bereit gemeldet hat; die Tür setzt es, der Faden
/// zum Gast wartet darauf.
#[derive(Debug, Clone, Default)]
pub(crate) struct GuestReady(Arc<(Mutex<bool>, Condvar)>);

impl GuestReady {
    /// Der Gast hat [`GUEST_READY`] geschrieben.
    pub(crate) fn set(&self) {
        let (ready, changed) = &*self.0;
        *ready.lock().unwrap_or_else(PoisonError::into_inner) = true;
        changed.notify_all();
    }

    /// Ob der Gast schon bereit gemeldet hat.
    pub(crate) fn is_set(&self) -> bool {
        *self.0.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wartet auf die Bereitschaft, bis zur Frist oder bis `ended` sagt, dass
    /// die Sitzung vorbei ist; `true`, wenn sie kam.
    pub(crate) fn wait(&self, deadline: Instant, ended: &mut dyn FnMut() -> bool) -> bool {
        let (ready, changed) = &*self.0;
        loop {
            {
                let set = ready.lock().unwrap_or_else(PoisonError::into_inner);
                if *set {
                    return true;
                }
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return false;
                }
                let (set, _) = changed
                    .wait_timeout(set, left.min(TICK))
                    .unwrap_or_else(PoisonError::into_inner);
                if *set {
                    return true;
                }
            }
            if ended() {
                return false;
            }
        }
    }
}

/// Liest [`GUEST_READY`] Byte für Byte, solange sie dazu passen. Für die
/// ganze Zeile gilt eine Frist von [`READY_LINE_WAIT`], damit ein Gast, der
/// sie langsam tropfen lässt, die Tür nicht länger hält. Auf ein Ende danach
/// wartet die Tür nicht: Über das vsock-Gerät kam es in den KVM-Tests nicht
/// innerhalb der Frist an, und was nach der Zeile kommt, liest niemand.
pub(crate) fn is_ready_line(stream: &mut UnixStream) -> bool {
    let deadline = Instant::now() + READY_LINE_WAIT;
    let mut seen = 0;
    let mut byte = [0u8; 1];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            return false;
        }
        match stream.read(&mut byte) {
            Ok(1) if byte[0] == GUEST_READY[seen] => {
                seen += 1;
                if seen == GUEST_READY.len() {
                    return true;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            _ => return false,
        }
    }
}

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
/// Lebensleine zugeht. Bis der Gast bereit gemeldet hat, nimmt die Tür nur
/// diese eine Zeile ([`GUEST_READY`]) und setzt `ready`; jede andere
/// Verbindung davor geht zu, ohne den Proxy zu erreichen.
pub(crate) fn serve_door(
    listener: &UnixListener,
    proxy: &Path,
    lifeline: &OwnedFd,
    ready: &GuestReady,
) {
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
        let Ok((mut guest, _)) = listener.accept() else {
            continue;
        };
        if !ready.is_set() {
            if is_ready_line(&mut guest) {
                ready.set();
            }
            continue;
        }
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
    /// Wo die Verbindung des Berichts für das Urteil des Daemons hinkommt;
    /// dort steht auch, wie weit der Weg zum Gast kam.
    pub(crate) admission: crate::launcher::Admission,
    /// Die Bereitschaft des Gasts, die die Tür setzt.
    pub(crate) ready: GuestReady,
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
            ready,
        } = self;
        let started = Instant::now();
        let since = || format!("{:.1}s", started.elapsed().as_secs_f64());
        let mut ended = || lifeline_closed(&lifeline, Duration::ZERO);
        admission.note_link("no ready line from the guest on the door yet".to_owned());
        if !ready.wait(deadline, &mut ended) {
            admission.note_link(if ended() {
                format!(
                    "the session ended after {} before the guest said ready on the door",
                    since()
                )
            } else {
                format!(
                    "no ready line from the guest on the door within {}; its vsock modules or \
                     its vsock device did not come up",
                    since()
                )
            });
            let _ = writeln!(report, "{NO_REPORT}");
            return;
        }
        admission.note_link(format!(
            "the guest said ready after {}; the report connection has no answer yet",
            since()
        ));
        let Some(first) = connect(&device, report_port, deadline, &mut ended) else {
            let _ = writeln!(report, "{NO_REPORT}");
            return;
        };
        admission.note_link(format!(
            "report connection open after {}; the control stream has no answer yet",
            since()
        ));
        // Auf der Verbindung des Berichts geht später das Urteil des Daemons
        // in den Gast (`ADMIT` oder `DENY`); ohne sie startet der Agent nie.
        if let Ok(verdict) = first.try_clone() {
            admission.offer(verdict);
        }
        let second = connect(&device, report_port, deadline, &mut ended);
        match second {
            Some(stream) => {
                admission.note_link(format!("both connections open after {}", since()));
                splice(UnixStream::from(control), stream);
            }
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
/// <port>`, dann `OK`. Antwortet das Gerät mit etwas anderem oder schließt
/// es die Verbindung (der Gast lauscht noch nicht), wird es wiederholt, bis
/// zur Frist oder bis `ended` sagt, dass die Sitzung vorbei ist. Die Antwort
/// des Geräts wird Byte für Byte gelesen: Was danach kommt, gehört schon dem
/// Shim.
///
/// **Eine gestellte Anfrage wird nie aufgegeben (HUM-470).** Der Gast nimmt
/// genau zwei Verbindungen in ihrer Reihenfolge (Bericht, Steuerkanal). Gäbe
/// der Host eine Anfrage auf, deren Antwort noch unterwegs ist, und fragte
/// neu, könnte der Gast die aufgegebene als Bericht nehmen, und der Bericht
/// ginge ins Leere. Deshalb wartet jede Anfrage auf ihre Antwort, bis zur
/// Frist der Sitzung oder ihrem Ende, und erst eine Absage des Geräts führt
/// zu einer neuen. Gemessen (HUM-470, vhost-device-vsock 0.3.0): Lauscht der
/// Gast noch nicht, sagt das Gerät binnen Millisekunden ab; die alte
/// Lesefrist von fünf Sekunden lief dabei nie ab. Die Regel ist also eine
/// Vorsorge, keine belegte Ursache der Fehlschläge in der CI; die Ursache
/// war ein `CONNECT` vor der Bereitschaft des Geräts (HUM-471, Kopf dieses
/// Moduls), und [`GuestLink`] fragt deshalb erst nach [`GUEST_READY`].
pub(crate) fn connect(
    device: &Path,
    port: u32,
    deadline: Instant,
    ended: &mut dyn FnMut() -> bool,
) -> Option<UnixStream> {
    while Instant::now() < deadline && !ended() {
        let Ok(mut stream) = UnixStream::connect(device) else {
            thread::sleep(TICK / 2);
            continue;
        };
        if stream
            .write_all(format!("CONNECT {port}\n").as_bytes())
            .is_err()
        {
            thread::sleep(TICK / 2);
            continue;
        }
        match answer(&mut stream, deadline, ended) {
            Answer::Accepted => {
                let _ = stream.set_read_timeout(None);
                return Some(stream);
            }
            Answer::Refused => thread::sleep(TICK / 2),
            Answer::Over => return None,
        }
    }
    None
}

/// Was das Gerät auf ein `CONNECT` sagte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    /// `OK`: Der Gast hat die Verbindung angenommen.
    Accepted,
    /// Eine andere Zeile, oder das Gerät schloss die Verbindung: Der Gast
    /// lauscht nicht, eine neue Anfrage ist nötig.
    Refused,
    /// Die Frist ist um oder die Sitzung zu Ende, bevor eine Antwort kam.
    Over,
}

/// Die erste Zeile des Geräts, gelesen in Scheiben von [`TICK`], damit das
/// Ende der Sitzung das Warten beendet; die Anfrage selbst bleibt dabei
/// offen. Frist und Ende der Sitzung gelten vor jedem Lesen, ob Bytes kamen
/// oder nicht, und noch einmal vor dem Annehmen: Ein `OK`, dessen Zeile erst
/// nach der Frist vollständig ist, gilt nicht mehr.
pub(crate) fn answer(
    stream: &mut UnixStream,
    deadline: Instant,
    ended: &mut dyn FnMut() -> bool,
) -> Answer {
    let _ = stream.set_read_timeout(Some(TICK));
    let mut first = Vec::new();
    let mut byte = [0u8; 1];
    let mut over = || Instant::now() >= deadline || ended();
    while first.len() < 64 {
        if over() {
            return Answer::Over;
        }
        match stream.read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => break,
            Ok(1) => first.push(byte[0]),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            // Das Ende der Verbindung (`Ok(0)`) oder ein Fehler: Das Gerät
            // hat abgesagt.
            Ok(_) | Err(_) => return Answer::Refused,
        }
    }
    if over() {
        return Answer::Over;
    }
    if first.starts_with(b"OK") {
        Answer::Accepted
    } else {
        Answer::Refused
    }
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
        let ready = GuestReady::default();
        let door_ready = ready.clone();
        let server = thread::spawn(move || serve_door(&door, &proxy_path, &read, &door_ready));

        // Vor der Bereitschaft erreicht keine Verbindung den Proxy; die
        // Zeile des Shims setzt sie und geht selbst nicht weiter.
        let mut early = UnixStream::connect(&door_path).unwrap();
        early.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
        let mut nothing = Vec::new();
        // Die Tür schließt, ohne zu lesen, was noch ansteht: ein Ende oder ein
        // Reset, nie eine Antwort.
        let _ = early.read_to_end(&mut nothing);
        assert!(nothing.is_empty());
        assert!(!ready.is_set());
        let mut knock = UnixStream::connect(&door_path).unwrap();
        knock.write_all(GUEST_READY).unwrap();
        // Wie der Shim: Nach der Zeile geht die Schreibseite zu.
        knock.shutdown(std::net::Shutdown::Write).unwrap();
        knock.read_to_end(&mut nothing).unwrap();
        assert!(ready.is_set());
        proxy.set_nonblocking(true).unwrap();
        assert!(
            proxy.accept().is_err(),
            "a line before the ready one reached the proxy"
        );
        proxy.set_nonblocking(false).unwrap();

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

    /// Ein Gerät, das je Verbindung erst `CONNECT <port>` liest und dann
    /// tut, was `reply` für die Nummer der Verbindung sagt: `Some((Wartezeit,
    /// Antwort))` antwortet nach der Wartezeit, `None` schließt sofort.
    /// Zurück kommt die Zahl der Verbindungen, die es sah.
    fn fake_device(
        path: &Path,
        reply: impl Fn(usize) -> Option<(Duration, &'static [u8])> + Send + 'static,
    ) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        let listener = UnixListener::bind(path).unwrap();
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&seen);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let number = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut line = String::new();
                let _ = BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
                assert!(line.starts_with("CONNECT 5001"), "{line:?}");
                let mut stream = stream;
                match reply(number) {
                    Some((wait, bytes)) => {
                        thread::sleep(wait);
                        let _ = stream.write_all(bytes);
                        // Offen halten, wie eine angenommene Verbindung.
                        thread::spawn(move || {
                            let mut sink = Vec::new();
                            let _ = stream.read_to_end(&mut sink);
                        });
                    }
                    None => drop(stream),
                }
            }
        });
        seen
    }

    /// HUM-470: Hält das Gerät ein `CONNECT` länger fest, als die alte
    /// Lesefrist von fünf Sekunden erlaubte (der Treiber im Gast lief noch
    /// nicht), wartet der Host auf genau diese Anfrage und stellt keine
    /// zweite. Eine zweite, die der Gast später ebenfalls annähme, nähme ihm
    /// die Reihenfolge von Bericht und Steuerkanal. Mutationsbeweis: mit der
    /// alten Lesefrist von fünf Sekunden und neuer Anfrage danach sieht das
    /// Gerät zwei Verbindungen.
    #[test]
    fn a_pending_connect_is_never_given_up_for_a_second() {
        let dir = tempfile::tempdir().unwrap();
        let device = dir.path().join("door");
        let seen = fake_device(&device, |_| {
            Some((Duration::from_millis(6500), b"OK 1073741824\n".as_slice()))
        });
        let started = Instant::now();
        let stream = connect(
            &device,
            5001,
            Instant::now() + Duration::from_secs(30),
            &mut || false,
        );
        assert!(stream.is_some(), "the guest's late OK was not taken");
        assert!(started.elapsed() >= Duration::from_millis(6500));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the host gave up a pending CONNECT and asked again"
        );
    }

    /// Eine Antwort, die über die Frist reicht (`OK` davor, das Zeilenende danach),
    /// gilt nicht: Der Host kehrt ohne Verbindung zurück, statt sie verspätet
    /// anzunehmen. Mutationsbeweis: Prüft `answer` Frist und Ende nur nach
    /// einer abgelaufenen Lesefrist, nimmt er das späte `OK` an.
    #[test]
    fn an_answer_completed_after_the_deadline_is_not_taken() {
        let dir = tempfile::tempdir().unwrap();
        let device = dir.path().join("door");
        let listener = UnixListener::bind(&device).unwrap();
        thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut line = String::new();
            let _ = BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
            let mut stream = stream;
            // Ein Byte je 50 ms: nie eine Lesefrist von 200 ms, aber die
            // Zeile wird erst nach der Frist fertig.
            for byte in b"OK..............................\n" {
                if stream.write_all(&[*byte]).is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let mut sink = Vec::new();
            let _ = stream.read_to_end(&mut sink);
        });
        let started = Instant::now();
        let stream = connect(
            &device,
            5001,
            Instant::now() + Duration::from_millis(600),
            &mut || false,
        );
        assert!(
            stream.is_none(),
            "an OK completed after the deadline was taken"
        );
        assert!(started.elapsed() < Duration::from_millis(1200));
    }

    /// Schließt das Gerät eine Anfrage (der Gast lauscht noch nicht), fragt
    /// der Host neu; die Antwort der zweiten nimmt er.
    #[test]
    fn a_refused_connect_is_asked_again() {
        let dir = tempfile::tempdir().unwrap();
        let device = dir.path().join("door");
        let seen = fake_device(&device, |number| {
            (number > 0).then_some((Duration::ZERO, b"OK 7\n".as_slice()))
        });
        let stream = connect(
            &device,
            5001,
            Instant::now() + Duration::from_secs(10),
            &mut || false,
        );
        assert!(stream.is_some());
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    /// Endet die Sitzung, während eine Anfrage noch offen ist, kehrt der
    /// Host zurück, ohne Verbindung und ohne die Frist abzuwarten.
    #[test]
    fn a_pending_connect_ends_with_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let device = dir.path().join("door");
        let _seen = fake_device(&device, |_| {
            Some((Duration::from_secs(60), b"OK 1\n".as_slice()))
        });
        let started = Instant::now();
        let mut polls = 0;
        let stream = connect(
            &device,
            5001,
            Instant::now() + Duration::from_secs(60),
            &mut || {
                polls += 1;
                polls > 5
            },
        );
        assert!(stream.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
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
            ready: GuestReady::default(),
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
        assert!(check.evidence.starts_with(crate::bwrap::NO_REPORT_REASON));
    }

    /// HUM-471: Vor der Bereitschaft des Gasts stellt der Faden zum Gast kein
    /// einziges `CONNECT`; vhost-device-vsock 0.3.0 hält für immer an, wenn
    /// eines eintrifft, während der Treiber des Gasts das Gerät einrichtet.
    /// Erst die Bereitschaft öffnet beide Verbindungen, und der Stand steht
    /// in der Zulassung. Mutationsbeweis: Ohne das Warten auf die
    /// Bereitschaft (`ready.wait` in `GuestLink::run`) sieht das Gerät
    /// Verbindungen, bevor der Gast bereit ist.
    #[test]
    fn no_connect_reaches_the_device_before_the_guest_is_ready() {
        let dir = tempfile::tempdir().unwrap();
        let device = dir.path().join("door");
        let seen = fake_device(&device, |_| Some((Duration::ZERO, b"OK 9\n".as_slice())));
        let (_reader, writer) = std::io::pipe().unwrap();
        let (_daemon, shim) = crate::shell::stream_pair(80, 24).unwrap();
        let (_lifeline, read) = crate::shim_fds::Lifeline::new().unwrap();
        let admission = crate::launcher::Admission::default();
        let ready = GuestReady::default();
        let link = GuestLink {
            device,
            report_port: 5001,
            deadline: Instant::now() + Duration::from_secs(30),
            report: writer,
            control: shim,
            lifeline: read,
            admission: admission.clone(),
            ready: ready.clone(),
        };
        link.start().unwrap();
        thread::sleep(Duration::from_millis(800));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the host asked the device before the guest was ready"
        );
        assert!(
            admission.link().contains("no ready line"),
            "{}",
            admission.link()
        );
        ready.set();
        let started = Instant::now();
        while seen.load(std::sync::atomic::Ordering::SeqCst) < 2
            && started.elapsed() < Duration::from_secs(5)
        {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 2);
        while !admission.link().contains("both connections open")
            && started.elapsed() < Duration::from_secs(5)
        {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            admission.link().contains("both connections open"),
            "{}",
            admission.link()
        );
    }

    /// Ohne Bereitschaft endet der Faden mit der Lebensleine, schreibt
    /// [`NO_REPORT`] und hält fest, dass die Sitzung vor der Zeile endete.
    #[test]
    fn without_the_ready_line_the_link_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let (mut reader, writer) = std::io::pipe().unwrap();
        let (_daemon, shim) = crate::shell::stream_pair(80, 24).unwrap();
        let (lifeline, read) = crate::shim_fds::Lifeline::new().unwrap();
        let admission = crate::launcher::Admission::default();
        let link = GuestLink {
            device: dir.path().join("door"),
            report_port: 5001,
            deadline: Instant::now() + Duration::from_secs(60),
            report: writer,
            control: shim,
            lifeline: read,
            admission: admission.clone(),
            ready: GuestReady::default(),
        };
        link.start().unwrap();
        thread::sleep(Duration::from_millis(300));
        drop(lifeline);
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, format!("{NO_REPORT}\n"));
        assert!(
            admission.link().starts_with("the session ended after "),
            "{}",
            admission.link()
        );
    }

    /// Nur genau die Zeile setzt die Bereitschaft; eine andere, eine
    /// kürzere oder eine stumme Verbindung nicht.
    #[test]
    fn only_the_exact_ready_line_counts() {
        for (bytes, expected) in [
            (GUEST_READY, true),
            (b"HUMANITL-GUEST-READ".as_slice(), false),
            (b"GET / HTTP/1.0\r\n".as_slice(), false),
            (b"".as_slice(), false),
        ] {
            let (mut ours, mut theirs) = UnixStream::pair().unwrap();
            theirs.write_all(bytes).unwrap();
            drop(theirs);
            assert_eq!(is_ready_line(&mut ours), expected, "{bytes:?}");
        }
    }

    /// Die Frist gilt für die ganze Zeile, nicht je Byte: Ein Gast, der die
    /// richtigen Bytes langsam schickt, hält die Tür nicht länger als
    /// [`READY_LINE_WAIT`] und ein wenig.
    #[test]
    fn a_dripped_ready_line_runs_into_one_deadline() {
        let (mut ours, mut theirs) = UnixStream::pair().unwrap();
        let dripper = std::thread::spawn(move || {
            for byte in GUEST_READY {
                if theirs.write_all(&[*byte]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        let started = Instant::now();
        assert!(!is_ready_line(&mut ours));
        assert!(started.elapsed() < READY_LINE_WAIT + Duration::from_millis(500));
        drop(ours);
        dripper.join().unwrap();
    }
}
