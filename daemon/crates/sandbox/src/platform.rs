//! Die Stellen, an denen Linux und macOS verschiedene Aufrufe brauchen (HUM-312).
//!
//! Alles andere in dieser Crate ist POSIX und baut auf beiden. Was hier steht,
//! hat unter Linux einen Aufruf, den macOS nicht kennt (`O_PATH`,
//! `memfd_create`, `/proc/self/fd`, `SOCK_CLOEXEC`, `MSG_NOSIGNAL`), und unter
//! macOS den nächstliegenden Ersatz. Der Ersatz ist nie stärker als das
//! Original; wo er schwächer ist, sagt der Kommentar am Ersatz, worin.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd, RawFd};
use std::path::PathBuf;

use rustix::fs::OFlags;
use rustix::net::SendFlags;

/// Öffnen, um einen Ort festzuhalten, nicht um zu lesen.
///
/// Unter Linux `O_PATH`: Der Deskriptor braucht kein Leserecht. macOS hat
/// kein `O_PATH`; dort wird zum Lesen geöffnet, ein Verzeichnis oder eine
/// Datei ohne Leserecht scheitert also schon hier statt erst später.
#[cfg(target_os = "linux")]
pub(crate) const PATH_ONLY: OFlags = OFlags::PATH;

/// Öffnen, um einen Ort festzuhalten, nicht um zu lesen (siehe Linux-Fassung).
///
/// `O_NONBLOCK`, damit eine FIFO unter dem gesuchten Namen das `open` nicht
/// anhält; die Typprüfung danach weist sie ab.
#[cfg(not(target_os = "linux"))]
pub(crate) const PATH_ONLY: OFlags = OFlags::RDONLY.union(OFlags::NONBLOCK);

/// Die Flags für `send(2)` auf einem Socket, dessen Gegenseite fehlen kann.
///
/// Unter Linux `MSG_NOSIGNAL`. macOS kennt das Flag nicht; dort genügt, dass
/// die Rust-Laufzeit `SIGPIPE` beim Start auf `SIG_IGN` setzt, und der Fehler
/// kommt als `EPIPE` zurück wie unter Linux.
#[cfg(target_os = "linux")]
pub(crate) const SEND_NO_SIGNAL: SendFlags = SendFlags::NOSIGNAL;

/// Die Flags für `send(2)` auf einem Socket, dessen Gegenseite fehlen kann.
#[cfg(not(target_os = "linux"))]
pub(crate) const SEND_NO_SIGNAL: SendFlags = SendFlags::empty();

/// Ein Pfad, über den dieser Prozess den geöffneten Deskriptor erneut öffnen
/// kann: `/proc/self/fd/N` unter Linux, `/dev/fd/N` unter macOS.
///
/// Unter macOS gilt das nur für Dateien; ein Verzeichnis unter `/dev/fd/N`
/// lässt sich nicht durchlaufen.
pub(crate) fn fd_path(fd: RawFd) -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from(format!("/proc/self/fd/{fd}"))
    } else {
        PathBuf::from(format!("/dev/fd/{fd}"))
    }
}

/// Wo das Objekt hinter einem Deskriptor jetzt liegt.
///
/// Unter Linux aus dem Verweis `/proc/self/fd/N`, unter macOS über
/// `fcntl(F_GETPATH)`.
///
/// # Errors
///
/// Der Fehler des Aufrufs.
pub(crate) fn opened_path(fd: BorrowedFd<'_>) -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd as _;
        std::fs::read_link(fd_path(fd.as_raw_fd()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        use std::os::unix::ffi::OsStringExt as _;
        let path = rustix::fs::getpath(fd)?;
        Ok(PathBuf::from(std::ffi::OsString::from_vec(
            path.into_bytes(),
        )))
    }
}

/// Eine anonyme Datei, schließt bei `exec`.
///
/// Unter Linux ein `memfd`. Unter macOS eine Datei im temporären Verzeichnis,
/// angelegt mit `O_EXCL` und Modus 0600 und sofort wieder entfernt, so dass
/// nur der Deskriptor bleibt.
///
/// # Errors
///
/// Der Fehler beim Anlegen.
pub(crate) fn anonymous_file(name: &str) -> io::Result<OwnedFd> {
    #[cfg(target_os = "linux")]
    {
        Ok(rustix::fs::memfd_create(
            name,
            rustix::fs::MemfdFlags::CLOEXEC,
        )?)
    }
    #[cfg(not(target_os = "linux"))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir();
        loop {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |at| at.subsec_nanos());
            let path = dir.join(format!(
                "{name}-{}-{}-{nanos}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => {
                    std::fs::remove_file(&path)?;
                    return Ok(OwnedFd::from(file));
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }
        }
    }
}

/// Setzt `FD_CLOEXEC` auf einem Deskriptor, den macOS ohne das Flag anlegt
/// (`posix_openpt`, `socketpair`). Unter Linux setzt der Aufruf selbst das
/// Flag, und diese Funktion tut nichts.
///
/// # Errors
///
/// Der Fehler von `fcntl(2)`.
pub(crate) fn set_cloexec(fd: impl AsFd) -> io::Result<()> {
    if cfg!(target_os = "linux") {
        return Ok(());
    }
    rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC)?;
    Ok(())
}
