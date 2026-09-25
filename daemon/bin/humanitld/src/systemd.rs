//! Socket-Aktivierung und Bereitmeldung an systemd (HUM-053).
//!
//! Zwei Dinge, die systemd von einem Dienst wissen will, und beide ohne eine
//! Bibliothek dazwischen, weil jedes davon wenige Zeilen Protokoll ist:
//!
//! - **`LISTEN_FDS`.** Hält `humanitld.socket` den gRPC-Socket, übergibt
//!   systemd ihn als Deskriptor 3, zusammen mit `LISTEN_PID` und `LISTEN_FDS`
//!   in der Umgebung (`sd_listen_fds(3)`). Der Daemon übernimmt ihn dann,
//!   statt selbst zu binden, und lässt die Datei beim Ende liegen: Sie gehört
//!   systemd, und ohne sie gäbe es beim nächsten Start nichts, worauf
//!   systemd lauscht. Ohne die Variablen bindet der Daemon wie bisher selbst
//!   (Entwicklung, Archiv, `AppImage` ohne Socket-Unit).
//! - **`READY=1`.** Mit `Type=notify` gilt der Dienst erst als gestartet, wenn
//!   er das an `NOTIFY_SOCKET` schickt (`sd_notify(3)`); ohne die Meldung
//!   bricht systemd den Start nach 90 s ab. Sie geht erst, wenn Token und
//!   Socket stehen: Eine Unit, die `After=humanitld.service` sagt, soll einen
//!   Dienst vorfinden, der antwortet.
//!
//! Die Übernahme hat zwei Schritte, weil sie zu zwei Zeitpunkten sicher ist:
//!
//! 1. [`claim_from_process`] läuft in `main`, bevor die Laufzeit ihre Threads
//!    startet. Es liest `LISTEN_PID`, `LISTEN_FDS` und `LISTEN_FDNAMES`,
//!    **entfernt sie aus der Umgebung**, damit kein Kind des Daemons sie erbt
//!    und auf einen Deskriptor 3 bezieht, der dann etwas anderes ist, und
//!    dupliziert Deskriptor 3 mit `F_DUPFD_CLOEXEC`. Besessen wird nur das
//!    Duplikat; die Nummer 3 selbst bekommt `FD_CLOEXEC` und bleibt offen.
//!    Ist sie gar nicht offen, weil die Variablen lügen, antwortet der Kern
//!    mit `EBADF`, und daraus wird `DAEMON_013`, nie ein fremder Besitz.
//! 2. [`adopt`] prüft das Duplikat, sobald der Pfad des Sockets feststeht: ein
//!    Unix-Socket vom Typ `SOCK_STREAM`, der lauscht (`SO_ACCEPTCONN`), an
//!    genau dem Pfad, an dem die Clients suchen (`Paths::daemon_socket` oder
//!    `--socket`). Erst wenn das alles stimmt, geht die Nummer 3 zu, und der
//!    Socket bekommt `0600` wie ein selbst gebundener. Fällt eine Prüfung
//!    durch, bleibt Nummer 3 unberührt, und der Daemon startet nicht
//!    (`DAEMON_013`): Ein Dienst, der auf einem Socket lauscht, den niemand
//!    findet oder der keine Verbindung annimmt, wäre der Fehler, den ein
//!    Paket am spätesten zeigt.
//!
//! Das hier ist die einzige Stelle dieses Programms mit `unsafe`: Aufrufe an
//! den Kern über Deskriptornummern und das Entfernen von Umgebungsvariablen.
//! Die Begründung steht an jedem Block.

use std::fs::{self, Permissions};
use std::future::Future;
use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_ipc::server::SHUTDOWN_GRACE;
use humanitl_ipc::{IpcServer, auth, bind_socket, v1};
use tokio::net::UnixListener;
use tokio::sync::oneshot;
use tonic::codegen::tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

/// Der erste übergebene Deskriptor (`SD_LISTEN_FDS_START`).
const LISTEN_FDS_START: RawFd = 3;

/// Die Variablen der Übergabe; nach [`claim_env`] steht keine mehr da.
pub const LISTEN_VARS: [&str; 3] = ["LISTEN_PID", "LISTEN_FDS", "LISTEN_FDNAMES"];

/// Die Rechte des Sockets, gleich wer ihn angelegt hat.
const SOCKET_MODE: u32 = auth::TOKEN_MODE;

/// Was die Umgebung über übergebene Sockets sagt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handover {
    /// Kein Socket für diesen Prozess: selbst binden.
    None,
    /// Genau ein Socket, als Deskriptor 3.
    One,
}

/// Liest `LISTEN_PID` und `LISTEN_FDS`, ohne einen Deskriptor anzufassen.
///
/// `var` ist die Umgebung, `pid` die eigene Prozessnummer; beides ist ein
/// Parameter, damit die Regeln ohne systemd prüfbar sind.
///
/// - Fehlt `LISTEN_PID` oder nennt es einen anderen Prozess, gilt die
///   Übergabe nicht diesem: Sie war für einen Elternprozess gedacht und ist
///   nur mit der Umgebung vererbt worden (`sd_listen_fds(3)`).
/// - `LISTEN_FDS=0` oder ein fehlendes `LISTEN_FDS` heißt: nichts übergeben.
/// - Mehr als ein Socket ist ein Fehler: `humanitld.socket` hat genau ein
///   `ListenStream`, und ein zweiter Deskriptor, den der Daemon nicht
///   bedient, wäre ein Socket, auf dem Clients vergeblich warten.
///
/// # Errors
///
/// `DAEMON_013`, wenn `LISTEN_PID` diesen Prozess nennt und `LISTEN_FDS` keine
/// Zahl oder größer als eins ist.
pub fn handover(var: impl Fn(&str) -> Option<String>, pid: u32) -> Result<Handover, Diagnostic> {
    let Some(listen_pid) = var("LISTEN_PID") else {
        return Ok(Handover::None);
    };
    if listen_pid.trim().parse::<u32>().ok() != Some(pid) {
        return Ok(Handover::None);
    }
    let Some(count) = var("LISTEN_FDS") else {
        return Ok(Handover::None);
    };
    match count.trim().parse::<u32>() {
        Ok(0) => Ok(Handover::None),
        Ok(1) => Ok(Handover::One),
        Ok(more) => Err(unusable(format!(
            "systemd passed {more} sockets (LISTEN_FDS={more}), and humanitld serves exactly one; \
             humanitld.socket has a single ListenStream"
        ))),
        Err(_) => Err(unusable(format!(
            "LISTEN_FDS={count:?} is not a number, so it is unclear which descriptors systemd \
             passed"
        ))),
    }
}

/// Eine Umgebung, aus der die Übergabe gelesen und entfernt wird.
///
/// Die Prozessumgebung ist [`ProcessEnv`]; Tests setzen eine Tabelle ein, weil
/// das Ändern der echten Umgebung in einem Testprozess mit mehreren Threads
/// nicht erlaubt ist.
pub trait ListenEnv {
    /// Der Wert einer Variable, wenn sie gesetzt ist.
    fn get(&self, key: &str) -> Option<String>;
    /// Entfernt eine Variable.
    fn remove(&mut self, key: &str);
}

/// Liest die Übergabe und entfernt ihre Variablen, in jedem Fall.
///
/// `sd_listen_fds(3)` mit `unset_environment` tut dasselbe: Die Variablen
/// gelten genau einem Prozess, und jedes Kind, das sie erbte, bezöge sie auf
/// einen Deskriptor 3, der bei ihm etwas anderes ist. Entfernt wird auch,
/// wenn die Übergabe einem anderen Prozess galt oder nicht zu lesen ist.
///
/// # Errors
///
/// Wie [`handover`].
pub fn claim_env(env: &mut impl ListenEnv, pid: u32) -> Result<Handover, Diagnostic> {
    let result = handover(|key| env.get(key), pid);
    for key in LISTEN_VARS {
        env.remove(key);
    }
    result
}

/// Die Umgebung dieses Prozesses.
struct ProcessEnv;

impl ListenEnv for ProcessEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    #[allow(unsafe_code)]
    fn remove(&mut self, key: &str) {
        // SAFETY: `ProcessEnv` entsteht nur in `claim_from_process`, und das
        // darf laut seinem Vertrag nur aufgerufen werden, solange der Prozess
        // einen einzigen Thread hat (in `main`, vor dem Bau der Laufzeit).
        // Niemand liest oder schreibt die Umgebung also gleichzeitig.
        unsafe { std::env::remove_var(key) }
    }
}

/// Der Deskriptor, den systemd übergeben hat, als eigenes Duplikat.
///
/// Noch nicht geprüft; das tut [`adopt`], sobald der Pfad feststeht.
#[derive(Debug)]
pub struct Passed {
    /// Das Duplikat, mit `FD_CLOEXEC`.
    fd: OwnedFd,
    /// Die Nummer, die systemd übergeben hat; sie wird erst nach bestandener
    /// Prüfung geschlossen und nie über ein `OwnedFd` besessen.
    raw: RawFd,
}

/// Übernimmt, was systemd diesem Prozess übergeben hat: die Variablen aus der
/// Umgebung, Deskriptor 3 als Duplikat.
///
/// # Errors
///
/// `DAEMON_013`, wenn die Variablen nicht zu lesen sind ([`handover`]) oder
/// Deskriptor 3 nicht offen ist.
///
/// # Safety
///
/// Nur aufrufen, solange der Prozess einen einzigen Thread hat: Die Funktion
/// entfernt Variablen aus der Umgebung (`std::env::remove_var`). `main` ruft
/// sie vor dem Bau der Tokio-Laufzeit, einmal.
#[allow(unsafe_code)]
pub unsafe fn claim_from_process() -> Result<Option<Passed>, Diagnostic> {
    match claim_env(&mut ProcessEnv, std::process::id())? {
        Handover::None => Ok(None),
        Handover::One => duplicate(LISTEN_FDS_START).map(Some),
    }
}

/// Dupliziert die Deskriptornummer `raw` mit `F_DUPFD_CLOEXEC` und setzt auf
/// `raw` selbst `FD_CLOEXEC`.
///
/// Auf `raw` wird nur über den Kern zugegriffen, nie über einen Besitz: Die
/// Nummer stammt aus einer Umgebungsvariable, der niemand trauen muss. Ist sie
/// nicht offen, antwortet `fcntl` mit `EBADF`; ist sie eine fremde Datei,
/// fällt das in [`adopt`] auf, und sie bleibt offen.
#[allow(unsafe_code)]
fn duplicate(raw: RawFd) -> Result<Passed, Diagnostic> {
    // SAFETY: `fcntl(F_DUPFD_CLOEXEC)` auf eine Nummer liest keinen Speicher
    // und übernimmt keinen Besitz; eine geschlossene Nummer ergibt -1/EBADF.
    let dup = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 0) };
    if dup < 0 {
        let error = io::Error::last_os_error();
        return Err(unusable(format!(
            "LISTEN_FDS names descriptor {raw}, but it cannot be duplicated ({error}); \
             nothing was passed that humanitld could serve"
        )));
    }
    // SAFETY: wie oben, nur Flags der Nummer; kein Speicher, kein Besitz.
    // Ohne `FD_CLOEXEC` erbte jedes Kind die Nummer bis zu ihrer Prüfung.
    unsafe {
        let flags = libc::fcntl(raw, libc::F_GETFD);
        if flags >= 0 {
            libc::fcntl(raw, libc::F_SETFD, flags | libc::FD_CLOEXEC);
        }
    }
    // SAFETY: `dup` ist eben vom Kern angelegt worden und gehört niemandem
    // sonst in diesem Prozess.
    let fd = unsafe { OwnedFd::from_raw_fd(dup) };
    Ok(Passed { fd, raw })
}

/// Eine ganzzahlige Socket-Option (`SOL_SOCKET`) des Deskriptors.
#[allow(unsafe_code)]
fn socket_option(fd: &OwnedFd, option: libc::c_int) -> io::Result<libc::c_int> {
    let mut value: libc::c_int = 0;
    let mut len = libc::socklen_t::try_from(std::mem::size_of::<libc::c_int>())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `fd` ist offen (besessen), `value` und `len` zeigen auf lokalen
    // Speicher in der Größe, die `len` nennt.
    let rc = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            option,
            (&raw mut value).cast(),
            &raw mut len,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(value)
}

/// Prüft, dass das Duplikat ein lauschender Stream-Socket ist.
fn check_listening(fd: &OwnedFd, raw: RawFd) -> Result<(), Diagnostic> {
    let kind = socket_option(fd, libc::SO_TYPE).map_err(|error| {
        unusable(format!(
            "descriptor {raw} is not a socket humanitld can serve ({error})"
        ))
    })?;
    if kind != libc::SOCK_STREAM {
        return Err(unusable(format!(
            "descriptor {raw} is a socket of type {kind}, not a stream socket; \
             humanitld.socket must use ListenStream"
        )));
    }
    let listening = socket_option(fd, libc::SO_ACCEPTCONN).map_err(|error| {
        unusable(format!(
            "descriptor {raw} does not say whether it listens ({error})"
        ))
    })?;
    if listening != 1 {
        return Err(unusable(format!(
            "descriptor {raw} is a stream socket that does not listen, so no client \
             connection would ever be accepted"
        )));
    }
    Ok(())
}

/// Macht aus dem Duplikat einen geprüften Listener für `expected`.
///
/// Erst nach allen Prüfungen geht die übergebene Nummer zu; schlägt eine
/// fehl, bleibt sie, wie sie ist (mit `FD_CLOEXEC`), und das Duplikat geht
/// mit dem Fehler.
///
/// # Errors
///
/// `DAEMON_013`, wenn der Deskriptor kein lauschender Unix-Stream-Socket an
/// `expected` ist oder sich nicht übernehmen lässt.
pub fn adopt(passed: Passed, expected: &Path) -> Result<UnixListener, Diagnostic> {
    let Passed { fd, raw } = passed;
    check_listening(&fd, raw)?;
    let listener = std::os::unix::net::UnixListener::from(fd);
    let address = listener.local_addr().map_err(|error| {
        unusable(format!(
            "descriptor {raw} is not a unix socket humanitld can serve ({error})"
        ))
    })?;
    let Some(bound) = address.as_pathname().map(Path::to_path_buf) else {
        return Err(unusable(
            "the socket systemd passed has no path, and the clients look for a file".to_owned(),
        ));
    };
    check_path(&bound, expected)?;
    fs::set_permissions(&bound, Permissions::from_mode(SOCKET_MODE)).map_err(|error| {
        unusable(format!(
            "cannot set 0600 on the socket {} systemd passed: {error}",
            bound.display()
        ))
    })?;
    listener.set_nonblocking(true).map_err(|error| {
        unusable(format!(
            "the socket systemd passed cannot be switched to non-blocking: {error}"
        ))
    })?;
    let listener = UnixListener::from_std(listener).map_err(|error| {
        unusable(format!(
            "the socket systemd passed cannot be served: {error}"
        ))
    })?;
    close_passed(raw);
    Ok(listener)
}

/// Schließt die übergebene Nummer, nachdem ihr Duplikat bestanden hat.
#[allow(unsafe_code)]
fn close_passed(raw: RawFd) {
    // SAFETY: Die Nummer wurde von `claim_from_process` nie in einen Besitz
    // genommen, und ihr Duplikat ist eben als der lauschende Socket geprüft,
    // den systemd übergeben hat. Kein anderer Teil des Programms hat sie
    // seither geöffnet: Eine offene Nummer bekommt keine neue Datei.
    unsafe {
        libc::close(raw);
    }
}

/// Weist einen Socket ab, der nicht dort liegt, wo die Clients suchen.
fn check_path(bound: &Path, expected: &Path) -> Result<(), Diagnostic> {
    if bound == expected {
        return Ok(());
    }
    Err(Diagnostic::builder(codes::DAEMON_013, Severity::Blocking)
        .why(format!(
            "systemd passed a socket at {}, but the clients of this daemon look for it at {}; \
             ListenStream of humanitld.socket and the runtime directory disagree",
            bound.display(),
            expected.display()
        ))
        .fix(humanitl_sandbox::doctor::command_fix(&[
            "systemctl",
            "--user",
            "cat",
            "humanitld.socket",
        ]))
        .build())
}

/// `DAEMON_013` mit diesem Grund.
fn unusable(why: String) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_013, Severity::Blocking)
        .why(why)
        .fix(humanitl_sandbox::doctor::command_fix(&[
            "systemctl",
            "--user",
            "cat",
            "humanitld.socket",
        ]))
        .build()
}

/// Schickt `state` an `NOTIFY_SOCKET`, wenn systemd eines gesetzt hat.
///
/// Ohne die Variable läuft der Daemon nicht unter `Type=notify`, und es gibt
/// niemanden zu benachrichtigen. Ein Fehlschlag beim Senden wird
/// protokolliert und hält nichts auf: Der Dienst läuft, und systemd sagt
/// selbst, wenn die Meldung ausbleibt.
pub fn notify(state: &str) {
    let Some(target) = std::env::var_os("NOTIFY_SOCKET") else {
        return;
    };
    let target = PathBuf::from(target);
    if let Err(error) = send_notification(&target, state) {
        tracing::warn!(
            %error,
            socket = %target.display(),
            state,
            "cannot tell systemd about the state of the daemon"
        );
    }
}

/// Eine Nachricht an den Benachrichtigungs-Socket von systemd.
///
/// Ein Pfad mit führendem `@` ist ein abstrakter Socket (`sd_notify(3)`); in
/// ihm steht statt des `@` ein Nullbyte.
fn send_notification(target: &Path, state: &str) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;

    let socket = UnixDatagram::unbound()?;
    let bytes = target.as_os_str().as_bytes();
    if let Some(name) = bytes.strip_prefix(b"@") {
        use std::os::linux::net::SocketAddrExt as _;

        let address = std::os::unix::net::SocketAddr::from_abstract_name(name)?;
        socket.send_to_addr(state.as_bytes(), &address)?;
    } else {
        socket.send_to(state.as_bytes(), target)?;
    }
    Ok(())
}

/// Woher der gRPC-Socket dieses Laufs kommt.
#[derive(Debug)]
pub enum Socket {
    /// systemd hat ihn übergeben; die Datei gehört systemd und bleibt liegen.
    Activated(UnixListener),
    /// Der Daemon bindet ihn selbst, nachdem das Token steht, und entfernt ihn
    /// am Ende.
    Bind,
}

/// Bedient den Vertrag, bis `shutdown` fertig ist, und meldet sich bei
/// systemd bereit, sobald Token und Socket stehen.
///
/// Dieselben Schritte wie [`humanitl_ipc::serve`], mit zwei Unterschieden:
/// Der Socket kann von systemd kommen, und dann bleibt seine Datei am Ende
/// liegen; und zwischen Binden und Bedienen geht `READY=1` hinaus. Das Token
/// wird immer zuerst geschrieben, und es verschwindet immer am Ende: Eine
/// liegen gebliebene Token-Datei wäre ein Schlüssel zu einem Dienst, den es
/// nicht mehr gibt.
///
/// # Errors
///
/// `DAEMON_004`, wenn Token oder Socket nicht angelegt werden können, und
/// `DAEMON_001`, wenn tonic den Dienst abbricht.
pub async fn serve(
    socket: Socket,
    path: &Path,
    token_path: &Path,
    server: IpcServer,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Diagnostic> {
    let token = auth::new_token()?;
    auth::write_token(token_path, &token)?;
    let activated = matches!(socket, Socket::Activated(_));
    let listener = match socket {
        Socket::Activated(listener) => listener,
        Socket::Bind => match bind_socket(path) {
            Ok(listener) => listener,
            Err(diagnostic) => {
                let _ = fs::remove_file(token_path);
                return Err(diagnostic);
            }
        },
    };
    tracing::info!(
        socket = %path.display(),
        token = %token_path.display(),
        activated,
        "listening"
    );
    notify("READY=1");

    let service =
        v1::humanitl_server::HumanitlServer::with_interceptor(server, auth::TokenAuth::new(token));
    let (fired, started) = oneshot::channel();
    let signal = async move {
        shutdown.await;
        notify("STOPPING=1");
        let _ = fired.send(());
    };
    let serving = Server::builder()
        .add_service(service)
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), signal);
    tokio::pin!(serving);

    let outcome = tokio::select! {
        result = &mut serving => result,
        _ = started => drain(&mut serving).await,
    };
    let result = outcome.map_err(|error| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .title("gRPC-Server abgebrochen")
            .why(format!("serving {} failed: {error}", path.display()))
            .build()
    });
    if !activated {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_file(token_path);
    if activated {
        tracing::info!("stopped, token removed; the socket stays with systemd");
    } else {
        tracing::info!("stopped, socket and token removed");
    }
    result
}

/// Lässt laufenden Aufrufen die Frist aus [`SHUTDOWN_GRACE`] und endet dann.
async fn drain<F>(serving: &mut F) -> Result<(), tonic::transport::Error>
where
    F: Future<Output = Result<(), tonic::transport::Error>> + Unpin,
{
    if let Ok(result) = tokio::time::timeout(SHUTDOWN_GRACE, serving).await {
        return result;
    }
    tracing::warn!(
        grace_secs = SHUTDOWN_GRACE.as_secs(),
        "a client kept its connection open past the grace period; closing anyway"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::HashMap;
    use std::io;
    use std::path::Path;

    use std::os::fd::{AsRawFd as _, IntoRawFd as _, OwnedFd, RawFd};

    use super::{
        Handover, LISTEN_VARS, ListenEnv, adopt, check_path, claim_env, duplicate, handover,
        send_notification,
    };

    /// Eine Umgebung als Tabelle, damit die echte unberührt bleibt.
    #[derive(Default)]
    struct TableEnv(HashMap<String, String>);

    impl ListenEnv for TableEnv {
        fn get(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn remove(&mut self, key: &str) {
            self.0.remove(key);
        }
    }

    fn table(pairs: &[(&str, &str)]) -> TableEnv {
        TableEnv(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        )
    }

    /// Nach der Übernahme steht keine Variable der Übergabe mehr in der
    /// Umgebung: für diesen Prozess, für einen anderen und bei einem Fehler.
    #[test]
    fn the_listen_variables_are_gone_after_the_claim() {
        let all = [
            ("LISTEN_PID", "42"),
            ("LISTEN_FDS", "1"),
            ("LISTEN_FDNAMES", "humanitld.socket"),
            ("HOME", "/home/tester"),
        ];
        for (pid, count) in [("42", "1"), ("41", "1"), ("42", "two")] {
            let mut env = table(&all);
            env.0.insert("LISTEN_PID".to_owned(), pid.to_owned());
            env.0.insert("LISTEN_FDS".to_owned(), count.to_owned());
            let _ = claim_env(&mut env, 42);
            for key in LISTEN_VARS {
                assert!(
                    env.get(key).is_none(),
                    "{key} is still set after LISTEN_PID={pid} LISTEN_FDS={count}"
                );
            }
            assert_eq!(env.get("HOME").as_deref(), Some("/home/tester"));
        }
    }

    /// Ob eine Deskriptornummer in diesem Prozess offen ist.
    fn is_open(raw: RawFd) -> bool {
        #[allow(unsafe_code)]
        // SAFETY: `F_GETFD` auf eine Nummer liest nur ihre Flags.
        let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
        flags >= 0
    }

    /// Eine Nummer, die in diesem Prozess nicht offen ist.
    ///
    /// Hoch gewählt, damit die parallel laufenden Tests sie nicht belegen;
    /// geprüft, bevor sie benutzt wird.
    fn closed_number() -> RawFd {
        (900..1000)
            .find(|raw| !is_open(*raw))
            .expect("a closed descriptor number")
    }

    /// Eine geschlossene Nummer in `LISTEN_FDS`: `DAEMON_013`, keine Panik.
    #[test]
    fn a_closed_descriptor_is_refused() {
        let raw = closed_number();
        let error = duplicate(raw).expect_err("nothing to duplicate");
        assert_eq!(error.code.as_str(), "DAEMON_013");
        assert!(error.why.contains("cannot be duplicated"), "{}", error.why);
    }

    /// Eine gewöhnliche Datei als Deskriptor: `DAEMON_013`, und die Datei
    /// bleibt offen, denn sie gehört nicht der Übernahme.
    #[test]
    fn a_plain_file_is_refused_and_stays_open() {
        let dir = tempfile::tempdir().unwrap();
        let file = std::fs::File::create(dir.path().join("plain")).unwrap();
        let raw = file.as_raw_fd();
        let passed = duplicate(raw).expect("an open file can be duplicated");
        let error = adopt(passed, &dir.path().join("daemon.sock")).expect_err("a file is refused");
        assert_eq!(error.code.as_str(), "DAEMON_013");
        assert!(is_open(raw), "the refused descriptor is not closed");
        drop(file);
    }

    /// Ein Datagramm-Socket am richtigen Pfad: `DAEMON_013`, bevor irgendwer
    /// `READY=1` sagt.
    #[test]
    fn a_datagram_socket_at_the_right_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let socket = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
        let raw = socket.as_raw_fd();
        let error = adopt(duplicate(raw).unwrap(), &path).expect_err("a datagram is refused");
        assert_eq!(error.code.as_str(), "DAEMON_013");
        assert!(error.why.contains("not a stream socket"), "{}", error.why);
        assert!(is_open(raw), "the refused descriptor is not closed");
    }

    /// Ein Stream-Socket, der gebunden ist, aber nicht lauscht.
    #[test]
    fn a_stream_socket_that_does_not_listen_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let socket = bound_stream_socket(&path);
        let raw = socket.as_raw_fd();
        let error = adopt(duplicate(raw).unwrap(), &path).expect_err("no listen, no daemon");
        assert_eq!(error.code.as_str(), "DAEMON_013");
        assert!(error.why.contains("does not listen"), "{}", error.why);
        assert!(is_open(raw), "the refused descriptor is not closed");
    }

    /// Führt `body` auf einem eigenen Faden aus, dessen Deskriptor-Tabelle
    /// privat ist (`close_range(2)` mit `CLOSE_RANGE_UNSHARE`), und lässt
    /// dort eine eigene Ein-Faden-`tokio`-Laufzeit über `body` laufen.
    ///
    /// Die Test-Harness ist mehrfädig; andere Tests desselben Binaries öffnen
    /// und schließen Deskriptoren parallel. `is_open(raw)` für „geschlossen“
    /// ist deshalb ein Wettlauf: Schließt die Übernahme die übergebene
    /// Nummer, kann ein paralleler Test dieselbe Nummer im selben Moment neu
    /// vergeben bekommen, und `is_open` sähe dann einen fremden Deskriptor
    /// statt eines geschlossenen. Gemessen in HUM-233 an
    /// `a_listening_socket_is_adopted_and_the_passed_number_closed`, CI-Lauf
    /// 36137925763. Für „offen geblieben“ reicht die Prüfung im eigenen
    /// Faden, weil dort kein `close` denselben Wettlauf öffnet; diese
    /// Funktion schützt nur den einen Test, der „geschlossen“ prüft. Das
    /// Muster stammt aus `daemon/bin/humanitl-shim/src/channel.rs`
    /// (`in_private_descriptor_table`, HUM-224); anders als dort wird die
    /// Kopie der Tabelle hier nicht geleert (kein `close_range` über einen
    /// echten Bereich): `tokio` hält einen Deskriptor für seine
    /// Prozesssignale prozessweit in einer globalen Registry (`OnceLock` in
    /// `tokio::runtime::signal::registry`, ein `UnixStream`-Paar), den ein
    /// anderer, schon gelaufener Test dieses Binaries angelegt haben kann
    /// (etwa `the_signal_bars_a_new_sandbox_before_the_farewell_task_runs`);
    /// leerte dieser Faden seine Kopie, verlöre `enable_all()` beim Bau der
    /// eigenen Laufzeit genau diesen Deskriptor und scheiterte mit `EBADF`.
    /// Ein Bereich, der nichts Offenes trifft (`u32::MAX..=u32::MAX`), löst
    /// trotzdem das `CLOSE_RANGE_UNSHARE`: Der Kern verlangt nur einen
    /// gültigen Bereich, keinen, der etwas Offenes enthält.
    ///
    /// Umgekehrt reicht das Nicht-Leeren allein nicht: Läuft dieser Test als
    /// erster Runtime-Bau des Prozesses (etwa mit einem Testfilter, der ihn
    /// isoliert, oder bei anderer Reihenfolge), legt ohne ein Aufwärmen vorher
    /// niemand die globale Registry an, bevor `close_range` läuft. `body`s
    /// eigenes `enable_all()` liefe dann erst *nach* `close_range`, also in
    /// der schon privaten Tabelle dieses Fadens — das `UnixStream`-Paar
    /// entstünde dort und stürbe mit dem Faden, sobald er endet. Jede
    /// spätere Laufzeit oder jeder Signal-Handler im geteilten Rest des
    /// Prozesses träfe dann auf `EBADF` oder eine inzwischen fremd vergebene
    /// Nummer. Deshalb baut die aufrufende Seite vor `close_range` eine
    /// Wegwerf-Laufzeit und verwirft sie sofort wieder: Sie legt die globale
    /// Registry, falls nötig, in der noch geteilten Tabelle an, bevor der
    /// private Faden entsteht, und lebt dort fort, unberührt vom
    /// `close_range` des privaten Fadens.
    fn in_private_descriptor_table<F>(body: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // Wegwerf-Laufzeit in der geteilten Tabelle: legt `tokio`s globale
        // Signal-Registry an, falls noch kein Test dieses Binaries sie schon
        // hat. Ohne dieses Aufwärmen könnte `body`s eigene Laufzeit sie in
        // der privaten Tabelle anlegen, die mit dem Faden gleich wieder
        // verschwindet (siehe Doc-Kommentar oben).
        drop(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a throwaway runtime to warm up tokio's signal globals"),
        );
        let outcome = std::thread::spawn(move || {
            let no_open_descriptor_here: libc::c_uint = libc::c_uint::MAX;
            #[allow(unsafe_code)]
            // SAFETY: close_range nimmt drei Ganzzahlen; der Bereich trifft
            // keinen offenen Deskriptor, unshare macht die Kopie trotzdem
            // privat, und sie ist die frische Kopie dieses Fadens, die sonst
            // niemand nutzt.
            let rc = unsafe {
                libc::syscall(
                    libc::SYS_close_range,
                    no_open_descriptor_here,
                    libc::c_uint::MAX,
                    libc::CLOSE_RANGE_UNSHARE,
                )
            };
            assert_eq!(rc, 0, "close_range: {}", io::Error::last_os_error());
            body();
        })
        .join();
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
    }

    /// Ein lauschender Socket am richtigen Pfad wird übernommen, und erst dann
    /// geht die übergebene Nummer zu.
    #[test]
    fn a_listening_socket_is_adopted_and_the_passed_number_closed() {
        in_private_descriptor_table(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a current-thread runtime");
            runtime.block_on(async {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("daemon.sock");
                // Den Besitz abgeben: Die Nummer gehört ab hier der
                // Übernahme, wie Deskriptor 3 im Daemon.
                let raw = std::os::unix::net::UnixListener::bind(&path)
                    .unwrap()
                    .into_raw_fd();
                let listener = adopt(duplicate(raw).unwrap(), &path).expect("a listening socket");
                assert!(
                    !is_open(raw),
                    "the passed number is closed after the adoption"
                );
                let _client = tokio::net::UnixStream::connect(&path).await.unwrap();
                let _ = listener.accept().await.unwrap();
            });
        });
    }

    /// Ein Unix-Stream-Socket, gebunden an `path`, ohne `listen`.
    #[allow(unsafe_code)]
    fn bound_stream_socket(path: &Path) -> OwnedFd {
        use std::os::fd::FromRawFd as _;
        use std::os::unix::ffi::OsStrExt as _;

        // SAFETY: `socket(2)` legt einen neuen Deskriptor an, den sofort ein
        // `OwnedFd` besitzt; `bind(2)` bekommt eine vollständig gefüllte
        // `sockaddr_un` und ihre Größe.
        unsafe {
            let raw = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0);
            assert!(raw >= 0, "socket(2) failed");
            let fd = OwnedFd::from_raw_fd(raw);
            let mut address: libc::sockaddr_un = std::mem::zeroed();
            address.sun_family = libc::sa_family_t::try_from(libc::AF_UNIX).unwrap();
            let bytes = path.as_os_str().as_bytes();
            assert!(
                bytes.len() < address.sun_path.len(),
                "the path fits sun_path"
            );
            for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
                *slot = libc::c_char::from_ne_bytes([*byte]);
            }
            let len = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_un>()).unwrap();
            let rc = libc::bind(
                fd.as_raw_fd(),
                (&raw const address).cast::<libc::sockaddr>(),
                len,
            );
            assert_eq!(rc, 0, "bind(2) failed");
            fd
        }
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn without_listen_pid_nothing_was_passed() {
        let got = handover(env(&[("LISTEN_FDS", "1")]), 42).unwrap();
        assert_eq!(got, Handover::None);
    }

    /// `LISTEN_PID` eines anderen Prozesses: Die Variablen sind nur vererbt,
    /// Deskriptor 3 gehört hier niemandem, der ihn übergeben hätte.
    #[test]
    fn a_handover_for_another_process_is_ignored() {
        let got = handover(env(&[("LISTEN_PID", "41"), ("LISTEN_FDS", "1")]), 42).unwrap();
        assert_eq!(got, Handover::None);
    }

    #[test]
    fn one_socket_for_this_process_is_taken() {
        let got = handover(env(&[("LISTEN_PID", "42"), ("LISTEN_FDS", "1")]), 42).unwrap();
        assert_eq!(got, Handover::One);
    }

    #[test]
    fn zero_sockets_mean_bind_yourself() {
        let got = handover(env(&[("LISTEN_PID", "42"), ("LISTEN_FDS", "0")]), 42).unwrap();
        assert_eq!(got, Handover::None);
    }

    #[test]
    fn two_sockets_are_refused() {
        let error = handover(env(&[("LISTEN_PID", "42"), ("LISTEN_FDS", "2")]), 42)
            .expect_err("a second socket nobody serves is refused");
        assert_eq!(error.code.as_str(), "DAEMON_013");
    }

    #[test]
    fn a_count_that_is_not_a_number_is_refused() {
        let error = handover(env(&[("LISTEN_PID", "42"), ("LISTEN_FDS", "one")]), 42)
            .expect_err("garbage is refused");
        assert_eq!(error.code.as_str(), "DAEMON_013");
    }

    #[test]
    fn a_socket_at_another_path_is_refused() {
        assert!(check_path(Path::new("/run/a.sock"), Path::new("/run/a.sock")).is_ok());
        let error = check_path(Path::new("/run/b.sock"), Path::new("/run/a.sock"))
            .expect_err("a socket nobody looks for is refused");
        assert_eq!(error.code.as_str(), "DAEMON_013");
        assert!(error.why.contains("/run/b.sock"), "{}", error.why);
    }

    /// Die Nachricht kommt an einem Pfad-Socket an, Byte für Byte.
    #[test]
    fn a_notification_reaches_a_path_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notify");
        let receiver = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
        send_notification(&path, "READY=1").unwrap();
        let mut buffer = [0_u8; 64];
        let read = receiver.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..read], b"READY=1");
    }

    /// Ein führendes `@` ist ein abstrakter Socket, kein Dateiname.
    #[test]
    fn a_notification_reaches_an_abstract_socket() {
        use std::os::linux::net::SocketAddrExt as _;

        let name = format!("humanitl-notify-test-{}", std::process::id());
        let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
        let receiver = std::os::unix::net::UnixDatagram::bind_addr(&address).unwrap();
        send_notification(Path::new(&format!("@{name}")), "READY=1").unwrap();
        let mut buffer = [0_u8; 64];
        let read = receiver.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..read], b"READY=1");
    }
}
