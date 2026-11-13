//! Deskriptoren an den Shim über einen Socket statt durch Vererbung (HUM-434).
//!
//! Ein Backend, dessen Client nicht mehr Kind des Daemons ist, sondern als
//! vorübergehende Unit des Dienstverwalters des Nutzers läuft
//! ([`crate::user_unit`]), kann dem Shim nichts vererben: Zwischen Daemon und
//! Shim steht ein Prozess, den der Dienstverwalter startet, und der erbt nur
//! seine drei Standardströme. Bericht, Lebensleine, Umgebung, Mounttabelle,
//! die Tür zu weiteren Shells und das Terminal kommen deshalb über einen
//! Socket je Sitzung, den der Shim als Erstes anspricht.
//!
//! **Der Ablauf.**
//!
//! 1. Der Plan bindet einen `SOCK_SEQPACKET`-Socket in einem eigenen,
//!    sonst leeren Verzeichnis der Sitzung (`0700`, unter dem
//!    Laufzeitverzeichnis von Humanitl), Rechte `0600`, Rückstau 1, und
//!    zieht ein Einmal-Zeichen von 128 Bit ([`HandoverListener::bind`]).
//!    Die Sandbox bekommt das **Verzeichnis** als nur lesbaren Bind, nicht
//!    den Socket: Ein Bind zeigt das Verzeichnis lebendig, und nach der
//!    Übergabe liegt darin nichts mehr. Pfad und Zeichen stehen in der Zeile
//!    des Shims.
//! 2. Der Start wartet höchstens die Frist des Berichts auf genau **eine**
//!    Verbindung und schließt den lauschenden Socket sofort nach dem
//!    `accept`, noch vor jeder Prüfung, und entfernt die Socket-Datei, bevor
//!    irgendein Deskriptor geht ([`HandoverListener::serve`]): Wenn der Shim
//!    seine Deskriptoren hat, gibt es in der Sandbox keinen Socket außer dem
//!    des Proxys mehr, und die zweite Garantie bleibt wörtlich. Wer
//!    danach kommt, auch der echte Shim nach einem Fremden, bekommt
//!    `ECONNREFUSED`: Die Sitzung scheitert, statt die Deskriptoren an den
//!    Falschen zu geben.
//! 3. Die Gegenseite muss dieselbe Nutzerkennung tragen wie der Daemon
//!    (`SO_PEERCRED`, [`admit`]) und als erste Nachricht genau das Zeichen
//!    schicken. Erst dann gehen alle Deskriptoren in **einer** Nachricht mit
//!    `SCM_RIGHTS`; ihr Inhalt ist ein Byte mit ihrer Zahl. Der Shim legt sie
//!    der Reihe nach auf 3, 4, 5, ..., genau wie früher `--preserve-fds`; was
//!    danach kommt (versiegelter memfd der Umgebung, Prüfung der
//!    Mounttabelle, Lebensleine), ist unverändert.
//!
//! **Die Lebensleine.** Ihr Leseende reist mit. Stirbt der Daemon vor der
//! Übergabe, gibt es niemanden mehr, der lauscht: Der Shim bekommt
//! `ECONNREFUSED` oder ein leeres Ende und startet den Agenten nie. Stirbt er
//! danach, sieht der Shim `POLLHUP` wie bisher.
//!
//! **Was das Zeichen schützt, und was nicht.** Ein anderer Nutzer erreicht den
//! Socket nicht (Verzeichnis `0700`, Prüfung der Kennung). Ein Prozess
//! derselben Kennung kann die Zeile lesen und das Zeichen nennen; gegen ihn
//! schützt nichts hier, wie gegen ihn auch sonst keine Grenze des Daemons
//! schützt (`docs/THREAT-MODEL.md`). Das Zeichen und die eine Verbindung
//! verhindern, dass eine verirrte oder zu frühe Verbindung die Deskriptoren
//! bekommt, und sie machen jeden Fehlversuch zu einem Abbruch, nie zu einem
//! zweiten Versuch.

use std::io::{self, IoSlice};
use std::mem::MaybeUninit;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SocketAddrUnix,
    SocketFlags, SocketType, accept_with, bind, listen, recv, sendmsg, socket_with,
};

/// Der Name des Sockets im Verzeichnis der Sitzung. Gebunden wird über das
/// geöffnete Verzeichnis ([`HandoverListener::bind`]); die Länge seines
/// absoluten Pfads spielt dann keine Rolle.
pub(crate) const SOCKET_NAME: &str = "fds";

/// Wie lang das Zeichen ist: 32 Hex-Ziffern.
pub(crate) const TOKEN_LEN: usize = 32;

/// Höchstens so viele Deskriptoren gehen über einen Socket: Bericht,
/// Lebensleine, Umgebung, Mounttabelle, Tür, Terminal, und Luft.
pub(crate) const MAX_FDS: usize = 8;

/// `SOCK_CLOEXEC` gleich beim Anlegen, wo es das gibt; unter macOS setzt
/// [`cloexec`] es danach.
#[cfg(target_os = "linux")]
const SOCKET_FLAGS: SocketFlags = SocketFlags::CLOEXEC;
#[cfg(not(target_os = "linux"))]
const SOCKET_FLAGS: SocketFlags = SocketFlags::empty();

/// Setzt `FD_CLOEXEC`, wo [`SOCKET_FLAGS`] es nicht schon tat.
#[cfg(target_os = "linux")]
#[allow(clippy::unnecessary_wraps)]
fn cloexec(fd: OwnedFd) -> rustix::io::Result<OwnedFd> {
    Ok(fd)
}

/// Setzt `FD_CLOEXEC`, wo [`SOCKET_FLAGS`] es nicht schon tat.
#[cfg(not(target_os = "linux"))]
fn cloexec(fd: OwnedFd) -> rustix::io::Result<OwnedFd> {
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC)?;
    Ok(fd)
}

/// Wie lange die angenommene Verbindung höchstens auf das Zeichen wartet.
const TOKEN_WAIT: Duration = Duration::from_secs(5);

/// Die lauschende Seite einer Übergabe, im Daemon.
#[derive(Debug)]
pub(crate) struct HandoverListener {
    listener: OwnedFd,
    path: PathBuf,
    token: String,
}

impl HandoverListener {
    /// Bindet den Socket unter `path`, Rechte `0600`, und zieht das Zeichen.
    ///
    /// # Errors
    ///
    /// Der Fehler von `socket(2)`, `bind(2)`, `chmod(2)`, `listen(2)` oder
    /// beim Lesen von `/dev/urandom`, auch für einen Pfad, der für einen
    /// Unix-Socket zu lang ist.
    pub(crate) fn bind(path: &Path) -> io::Result<Self> {
        let listener = cloexec(socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SOCKET_FLAGS,
            None,
        )?)?;
        // Unter Linux nimmt `bind(2)` die Rechte der Datei vom Inode des
        // Sockets (abzüglich `umask`): `fchmod` davor legt sie gleich mit
        // `0600` an, ohne Fenster, in dem sie weiter offen stünde.
        #[cfg(target_os = "linux")]
        rustix::fs::fchmod(&listener, rustix::fs::Mode::from_raw_mode(0o600))?;
        bind_at(&listener, path)?;
        // Woanders (und als Zusicherung auch hier) danach; das Verzeichnis
        // der Sitzung ist `0700`, niemand sonst erreicht die Datei vorher.
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        listen(&listener, 1)?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
            token: new_token()?,
        })
    }

    /// Das Einmal-Zeichen, 32 Hex-Ziffern.
    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    /// Wo der Socket liegt.
    #[cfg_attr(not(all(test, target_os = "linux")), allow(dead_code))]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Wartet höchstens `wait` auf genau eine Verbindung, schließt den
    /// lauschenden Socket, prüft Kennung und Zeichen und gibt dann `fds` in
    /// einer Nachricht weiter. Die eigenen Kopien bleiben beim Aufrufer.
    ///
    /// # Errors
    ///
    /// Ein Satz für den Befund: keine Verbindung in der Frist, eine fremde
    /// Kennung, ein falsches oder fehlendes Zeichen, zu viele Deskriptoren
    /// oder ein Fehler beim Senden. Der lauschende Socket ist danach immer
    /// zu, und kein Deskriptor ist gegangen.
    pub(crate) fn serve(
        self,
        fds: &[BorrowedFd<'_>],
        wait: Duration,
        expected_uid: u32,
    ) -> Result<(), String> {
        if fds.len() > MAX_FDS {
            return Err(format!(
                "the shim would need {} descriptors and the handover carries at most {MAX_FDS}",
                fds.len()
            ));
        }
        let Self {
            listener,
            path,
            token,
        } = self;
        wait_readable(&listener, wait)?;
        let accepted = accept_with(&listener, SOCKET_FLAGS).and_then(cloexec);
        // Genau eine Verbindung: Der lauschende Socket geht zu, bevor irgend
        // etwas an der angenommenen geprüft wird. Jede weitere bekommt
        // `ECONNREFUSED`.
        drop(listener);
        // Die Datei geht, bevor ein Deskriptor geht: Der Shim kann erst
        // weiter, wenn in der Sandbox kein zweiter Socket mehr liegt.
        let removed = std::fs::remove_file(&path);
        let connection =
            accepted.map_err(|err| format!("the handover socket did not accept: {err}"))?;
        removed.map_err(|err| {
            format!(
                "the handover socket {} could not be removed: {err}",
                path.display()
            )
        })?;
        let peer = peer_uid(&connection)
            .map_err(|err| format!("the handover cannot ask who connected: {err}"))?;
        let _ = rustix::net::sockopt::set_socket_timeout(
            &connection,
            rustix::net::sockopt::Timeout::Recv,
            Some(TOKEN_WAIT),
        );
        let mut given = [0u8; 2 * TOKEN_LEN];
        let received = recv(&connection, &mut given[..], RecvFlags::empty())
            .map(|(n, _)| n)
            .map_err(|err| format!("the handover got no token: {err}"))?;
        admit(peer, expected_uid, &given[..received], &token)?;
        let count = u8::try_from(fds.len()).unwrap_or(u8::MAX);
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
            return Err("the descriptors do not fit into one handover message".to_owned());
        }
        sendmsg(
            &connection,
            &[IoSlice::new(&[count])],
            &mut control,
            crate::platform::SEND_NO_SIGNAL,
        )
        .map_err(|err| format!("the descriptors did not reach the shim: {err}"))?;
        Ok(())
    }
}

/// Bindet `listener` an `path`, unabhängig von dessen Länge: Unter Linux
/// über das geöffnete Verzeichnis als `/proc/self/fd/<n>/<name>`, ein Pfad
/// von wenigen Dutzend Bytes, der dieselbe Datei am selben Ort anlegt (ein
/// Unix-Socket nimmt höchstens 107 Bytes, und ein tiefes
/// Laufzeitverzeichnis überschreitet das). Anderswo direkt.
///
/// # Errors
///
/// Der Fehler von `open(2)` oder `bind(2)`; ein Pfad, der auch so nicht
/// geht, mit einem Satz, der die Länge nennt.
fn bind_at(listener: &OwnedFd, path: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd as _;

        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} names no file in a directory", path.display()),
            ));
        };
        let dir = rustix::fs::open(
            dir,
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?;
        let short = Path::new("/proc/self/fd")
            .join(dir.as_raw_fd().to_string())
            .join(name);
        let address = SocketAddrUnix::new(&short).map_err(|err| too_long(path, &err.into()))?;
        bind(listener, &address)?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let address = SocketAddrUnix::new(path).map_err(|err| too_long(path, &err.into()))?;
        bind(listener, &address)?;
        Ok(())
    }
}

/// Der Fehler, wenn sich kein Socket-Pfad bilden lässt.
fn too_long(path: &Path, err: &io::Error) -> io::Error {
    io::Error::new(
        err.kind(),
        format!(
            "no Unix socket address fits {} ({} bytes): {err}",
            path.display(),
            path.as_os_str().len()
        ),
    )
}

/// Ob die Gegenseite die Deskriptoren bekommt: dieselbe Kennung wie der
/// Daemon, und genau das Zeichen, nicht mehr und nicht weniger.
///
/// # Errors
///
/// Der Satz für den Befund; er nennt nie das Zeichen selbst.
pub(crate) fn admit(
    peer_uid: u32,
    expected_uid: u32,
    given: &[u8],
    token: &str,
) -> Result<(), String> {
    if peer_uid != expected_uid {
        return Err(format!(
            "a process of uid {peer_uid} connected to the handover socket, and only uid \
             {expected_uid} may; the descriptors were not handed over"
        ));
    }
    if !same(given, token.as_bytes()) {
        return Err(
            "the process at the handover socket did not name the session's token; the \
             descriptors were not handed over"
                .to_owned(),
        );
    }
    Ok(())
}

/// Vergleich in gleicher Zeit für gleich lange Eingaben.
fn same(given: &[u8], expected: &[u8]) -> bool {
    given.len() == expected.len()
        && given
            .iter()
            .zip(expected)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// 128 Bit aus dem Zufall des Kerns (`/dev/urandom`), als Hex-Ziffern.
fn new_token() -> io::Result<String> {
    use std::io::Read as _;

    let mut bytes = [0u8; TOKEN_LEN / 2];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes
        .iter()
        .fold(String::with_capacity(TOKEN_LEN), |mut hex, byte| {
            let _ = std::fmt::Write::write_fmt(&mut hex, format_args!("{byte:02x}"));
            hex
        }))
}

/// Wartet, bis `listener` eine Verbindung hat, höchstens `wait`.
fn wait_readable(listener: &OwnedFd, wait: Duration) -> Result<(), String> {
    let deadline = Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "the shim did not come for its descriptors within {}s",
                wait.as_secs()
            ));
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = [PollFd::new(listener, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => {}
            Ok(_) => return Ok(()),
            Err(err) => return Err(format!("the handover socket cannot be watched: {err}")),
        }
    }
}

/// Die Kennung der Gegenseite, im Nutzer-Namensraum des Daemons.
#[cfg(target_os = "linux")]
fn peer_uid(connection: &OwnedFd) -> io::Result<u32> {
    Ok(rustix::net::sockopt::socket_peercred(connection)?
        .uid
        .as_raw())
}

/// Ohne `SO_PEERCRED` gibt es keine Prüfung, also keine Übergabe; ein
/// Backend, das sie braucht, gibt es nur unter Linux.
#[cfg(not(target_os = "linux"))]
fn peer_uid(_connection: &OwnedFd) -> io::Result<u32> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no SO_PEERCRED on this system",
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Read as _;
    use std::os::fd::AsFd as _;
    use std::thread;

    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, SendFlags, connect, recvmsg};

    use super::*;

    fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn listener() -> (tempfile::TempDir, HandoverListener) {
        let dir = tempfile::tempdir().unwrap();
        let listener = HandoverListener::bind(&dir.path().join(SOCKET_NAME)).unwrap();
        (dir, listener)
    }

    fn connect_to(path: &Path) -> io::Result<OwnedFd> {
        let socket = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )?;
        connect(&socket, &SocketAddrUnix::new(path)?)?;
        Ok(socket)
    }

    /// Was der Shim tut: Zeichen schicken, Deskriptoren nehmen. Leer, wenn
    /// der Daemon ohne Deskriptoren schloss.
    fn take(path: &Path, token: &[u8]) -> Vec<OwnedFd> {
        let socket = connect_to(path).unwrap();
        rustix::net::send(&socket, token, SendFlags::NOSIGNAL).unwrap();
        let mut count = [0u8; 1];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let message = recvmsg(
            &socket,
            &mut [io::IoSliceMut::new(&mut count)],
            &mut control,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        let mut fds = Vec::new();
        for item in control.drain() {
            if let RecvAncillaryMessage::ScmRights(rights) = item {
                fds.extend(rights);
            }
        }
        if message.bytes == 1 {
            assert_eq!(usize::from(count[0]), fds.len());
        } else {
            assert_eq!(message.bytes, 0, "nothing but the count is sent");
        }
        fds
    }

    #[test]
    fn the_socket_is_private_and_the_token_is_fresh_hex() {
        use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};

        let (_dir, first) = listener();
        let meta = std::fs::symlink_metadata(first.path()).unwrap();
        assert!(meta.file_type().is_socket());
        assert_eq!(meta.mode() & 0o777, 0o600);
        assert_eq!(first.token().len(), TOKEN_LEN);
        assert!(first.token().bytes().all(|b| b.is_ascii_hexdigit()));
        let (_dir, second) = listener();
        assert_ne!(first.token(), second.token());
    }

    /// Die Lebensleine kommt über die Übergabe an, und ihr Ende kommt mit:
    /// Fällt das Schreibende im Daemon, sieht die Gegenseite das Ende der
    /// Pipe, wie der Shim es mit `POLLHUP` sieht.
    #[test]
    fn the_lifeline_arrives_and_ends_when_the_daemon_side_drops() {
        let (_dir, listener) = listener();
        let path = listener.path().to_path_buf();
        let token = listener.token().to_owned();
        let (lifeline, read_end) = crate::shim_fds::Lifeline::new().unwrap();
        let (report_read, report_write) = std::io::pipe().unwrap();
        let server = thread::spawn(move || {
            let report = OwnedFd::from(report_write);
            listener.serve(
                &[report.as_fd(), read_end.as_fd()],
                Duration::from_secs(10),
                uid(),
            )
        });
        let fds = take(&path, token.as_bytes());
        server.join().unwrap().unwrap();
        assert_eq!(fds.len(), 2, "report and lifeline, in this order");
        // Nach der Übergabe ist das Verzeichnis leer: In der Sandbox, die es
        // gebunden sieht, liegt kein zweiter Socket.
        let left: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert!(left.is_empty(), "{left:?}");
        // Die erste ist die Schreibseite des Berichts: Was hineingeht, liest
        // der Daemon.
        let mut report_writer = std::fs::File::from(fds[0].try_clone().unwrap());
        std::io::Write::write_all(&mut report_writer, b"CHECK x ok y\n").unwrap();
        drop(report_writer);
        let mut fds = fds;
        let lifeline_read = fds.pop().unwrap();
        drop(fds);
        let mut line = String::new();
        let mut reader = report_read;
        reader.read_to_string(&mut line).unwrap();
        assert_eq!(line, "CHECK x ok y\n");
        // Solange der Daemon lebt, ist die Pipe offen und still.
        let mut probe = [PollFd::new(&lifeline_read, PollFlags::IN)];
        let none = Timespec {
            tv_sec: 0,
            tv_nsec: 50_000_000,
        };
        assert_eq!(poll(&mut probe, Some(&none)).unwrap(), 0);
        drop(lifeline);
        let mut probe = [PollFd::new(&lifeline_read, PollFlags::IN)];
        let wait = Timespec {
            tv_sec: 5,
            tv_nsec: 0,
        };
        assert_eq!(poll(&mut probe, Some(&wait)).unwrap(), 1);
        assert!(probe[0].revents().contains(PollFlags::HUP));
    }

    /// Ein falsches Zeichen bekommt nichts, und danach nimmt der Socket
    /// niemanden mehr an, auch nicht den mit dem richtigen.
    #[test]
    fn a_wrong_token_gets_nothing_and_closes_the_door_for_everyone() {
        let (_dir, listener) = listener();
        let path = listener.path().to_path_buf();
        let token = listener.token().to_owned();
        let (_read, write) = std::io::pipe().unwrap();
        let server = thread::spawn(move || {
            let write = OwnedFd::from(write);
            listener.serve(&[write.as_fd()], Duration::from_secs(10), uid())
        });
        let wrong = "0".repeat(TOKEN_LEN);
        assert_ne!(wrong, token);
        assert!(take(&path, wrong.as_bytes()).is_empty());
        let err = server.join().unwrap().unwrap_err();
        assert!(err.contains("token"), "{err}");
        assert!(!err.contains(&token), "the sentence names the token: {err}");
        // Der Socket ist nach der ersten Verbindung gelöscht: Ein später
        // Versuch findet nichts mehr, nicht einmal einen geschlossenen Socket.
        assert!(!path.exists(), "the socket outlives its one connection");
        let late = connect_to(&path).unwrap_err();
        assert_eq!(late.kind(), std::io::ErrorKind::NotFound, "{late}");
    }

    /// Mutationsbeweis für die Prüfung der Kennung: Erwartet der Daemon eine
    /// andere Kennung als die der Gegenseite, gehen keine Deskriptoren,
    /// auch mit dem richtigen Zeichen.
    #[test]
    fn a_peer_of_another_uid_gets_nothing_even_with_the_token() {
        let (_dir, listener) = listener();
        let path = listener.path().to_path_buf();
        let token = listener.token().to_owned();
        let (_read, write) = std::io::pipe().unwrap();
        let server = thread::spawn(move || {
            let write = OwnedFd::from(write);
            listener.serve(
                &[write.as_fd()],
                Duration::from_secs(10),
                uid().wrapping_add(1),
            )
        });
        assert!(take(&path, token.as_bytes()).is_empty());
        let err = server.join().unwrap().unwrap_err();
        assert!(err.contains("uid"), "{err}");
    }

    /// Genau eine Verbindung: Nach einer gelungenen Übergabe nimmt der
    /// Socket keine zweite an.
    #[test]
    fn after_one_handover_the_socket_takes_no_second_connection() {
        let (_dir, listener) = listener();
        let path = listener.path().to_path_buf();
        let token = listener.token().to_owned();
        let server = thread::spawn(move || listener.serve(&[], Duration::from_secs(10), uid()));
        assert!(take(&path, token.as_bytes()).is_empty());
        server.join().unwrap().unwrap();
        // Der Socket ist nach der ersten Verbindung gelöscht: Ein später
        // Versuch findet nichts mehr, nicht einmal einen geschlossenen Socket.
        assert!(!path.exists(), "the socket outlives its one connection");
        let late = connect_to(&path).unwrap_err();
        assert_eq!(late.kind(), std::io::ErrorKind::NotFound, "{late}");
    }

    #[test]
    fn nobody_coming_ends_the_wait_and_the_socket() {
        let (_dir, listener) = listener();
        let path = listener.path().to_path_buf();
        let started = Instant::now();
        let err = listener
            .serve(&[], Duration::from_millis(200), uid())
            .unwrap_err();
        assert!(err.contains("did not come"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(5));
        let late = connect_to(&path).unwrap_err();
        assert_eq!(late.raw_os_error(), Some(libc_econnrefused()));
    }

    /// Ein Verzeichnis, dessen absoluter Pfad länger ist, als ein
    /// Unix-Socket fasst: Der Socket entsteht trotzdem dort, nur für den
    /// Nutzer, und die Übergabe gelingt über genau diesen Pfad (im Test
    /// über das Verzeichnis, wie der Shim über seinen eigenen kurzen Pfad).
    #[test]
    fn a_session_directory_deeper_than_a_socket_address_still_hands_over() {
        use std::os::fd::AsRawFd as _;
        use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};

        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("d".repeat(60)).join("e".repeat(60));
        std::fs::create_dir_all(&deep).unwrap();
        let path = deep.join(SOCKET_NAME);
        assert!(path.as_os_str().len() > 108, "{}", path.display());
        let listener = HandoverListener::bind(&path).unwrap();
        let meta = std::fs::symlink_metadata(&path).unwrap();
        assert!(meta.file_type().is_socket());
        assert_eq!(meta.mode() & 0o777, 0o600);
        let token = listener.token().to_owned();
        let (_read, write) = std::io::pipe().unwrap();
        let server = thread::spawn(move || {
            let write = OwnedFd::from(write);
            listener.serve(&[write.as_fd()], Duration::from_secs(10), uid())
        });
        let held = std::fs::File::open(&deep).unwrap();
        let short = PathBuf::from(format!("/proc/self/fd/{}/{SOCKET_NAME}", held.as_raw_fd()));
        assert_eq!(take(&short, token.as_bytes()).len(), 1);
        server.join().unwrap().unwrap();
    }

    #[test]
    fn admit_takes_only_the_same_uid_and_exactly_the_token() {
        let token = "ab".repeat(TOKEN_LEN / 2);
        assert!(admit(7, 7, token.as_bytes(), &token).is_ok());
        assert!(admit(8, 7, token.as_bytes(), &token).is_err());
        assert!(admit(7, 7, &token.as_bytes()[1..], &token).is_err());
        let longer = format!("{token}0");
        assert!(admit(7, 7, longer.as_bytes(), &token).is_err());
        assert!(admit(7, 7, b"", &token).is_err());
    }

    #[test]
    fn too_many_descriptors_are_refused_before_anyone_connects() {
        let (_dir, listener) = listener();
        let (_read, write) = std::io::pipe().unwrap();
        let write = OwnedFd::from(write);
        let many: Vec<BorrowedFd<'_>> = (0..=MAX_FDS).map(|_| write.as_fd()).collect();
        let err = listener
            .serve(&many, Duration::from_millis(10), uid())
            .unwrap_err();
        assert!(err.contains("at most"), "{err}");
    }

    fn libc_econnrefused() -> i32 {
        rustix::io::Errno::CONNREFUSED.raw_os_error()
    }
}
