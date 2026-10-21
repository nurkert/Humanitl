//! Deskriptoren, die nur das Kind erbt (HUM-263).
//!
//! Der Launcher reicht `bwrap` Pipes, versiegelte memfds und die geöffneten
//! Verzeichnisse unter ihrer eigenen Nummer weiter. Bis HUM-263 nahm er dafür
//! im eigenen Prozess `FD_CLOEXEC` weg und hielt das Fenster bis zum `spawn`
//! mit einem Riegel klein. Der Riegel galt aber nur für diesen Launcher: Ein
//! anderer Thread, der im selben Fenster etwas startet (die Proben des
//! Doctors, der Secret-Helper der Konfiguration, ein Werkzeug der
//! Zusammenfassung), erbte die Deskriptoren mit. Für einen
//! `O_PATH`-Deskriptor auf den Projektordner hieße das: ein fremder Prozess
//! hielte einen Weg in den Dateibaum des Hosts, der nie in seine Hände sollte.
//!
//! [`inherit_in_child`] lässt `FD_CLOEXEC` deshalb im Elternprozess stehen
//! und nimmt es erst im Kind weg, zwischen `fork` und `exec`
//! (`CommandExt::pre_exec`). Das ist die eine Stelle dieser Crate, an der
//! `unsafe` erlaubt ist: Die Standardbibliothek hat keinen sicheren Weg, einen
//! Deskriptor gezielt an ein Kind zu vererben, und eine Bibliothek wie
//! `command-fds` täte an derselben Stelle dasselbe.
//!
//! Jeder vererbte Deskriptor liegt über den Standardströmen ([`above_stdio`]):
//! 0, 1 und 2 richtet die Standardbibliothek für das Kind per `dup2` ein,
//! bevor der Hook läuft. Läge ein vererbter Deskriptor dort, weil der Daemon
//! einen Standardstrom geschlossen hatte und `open` die freie Nummer vergab,
//! zeigte die Nummer im Kind auf etwas anderes, der Hook nähme `FD_CLOEXEC`
//! vom Falschen, und `bwrap` bände das Falsche.

#![allow(unsafe_code)]

use std::os::fd::{AsRawFd as _, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt as _;
use std::process::Command;

use rustix::io::{Errno, FdFlags, fcntl_dupfd_cloexec, fcntl_setfd};

/// Die kleinste Nummer, die ein vererbter Deskriptor tragen darf.
const FIRST_INHERITED_FD: RawFd = 3;

/// Der Fehler, mit dem der Hook im Kind scheitert.
///
/// `fcntl(F_SETFD)` kennt nur `EBADF`, und `execve(2)` liefert es nie; ein
/// `spawn`, der mit dieser Nummer scheitert, ist deshalb der Hook und nicht
/// ein fehlendes oder nicht ausführbares Programm.
pub(crate) const HOOK_ERRNO: i32 = Errno::BADF.raw_os_error();

/// Legt einen Deskriptor über die Standardströme: Trägt er 0, 1 oder 2, kommt
/// eine Kopie ab Nummer 3 mit `FD_CLOEXEC` zurück, und das Original geht zu.
/// Sonst kommt er unverändert zurück.
///
/// # Errors
///
/// Der Fehler von `fcntl(F_DUPFD_CLOEXEC)`.
pub(crate) fn above_stdio(fd: OwnedFd) -> std::io::Result<OwnedFd> {
    if fd.as_raw_fd() >= FIRST_INHERITED_FD {
        return Ok(fd);
    }
    fcntl_dupfd_cloexec(&fd, FIRST_INHERITED_FD).map_err(std::io::Error::from)
}

/// Sorgt dafür, dass das Kind von `command` genau diese Deskriptoren unter
/// ihrer Nummer erbt, während sie im Elternprozess `FD_CLOEXEC` behalten.
///
/// Die Ausleihe verlangt, dass jeder Deskriptor beim Aufruf offen ist; der
/// Aufrufer hält ihn außerdem als `OwnedFd`, bis `command.spawn()`
/// zurückkehrt.
///
/// # Errors
///
/// `InvalidInput`, wenn einer auf 0, 1 oder 2 liegt; dann wird kein Hook
/// gesetzt. Solche Deskriptoren legt [`above_stdio`] vorher um.
pub(crate) fn inherit_in_child(
    command: &mut Command,
    fds: &[BorrowedFd<'_>],
) -> std::io::Result<()> {
    let numbers: Vec<RawFd> = fds.iter().map(std::os::fd::AsRawFd::as_raw_fd).collect();
    if let Some(low) = numbers.iter().find(|&&fd| fd < FIRST_INHERITED_FD) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "descriptor {low} sits on a standard stream; the child's stdio would replace it \
                 before it is handed over"
            ),
        ));
    }
    let hook = move || -> std::io::Result<()> {
        for &fd in &numbers {
            // SAFETY: Läuft im Kind zwischen `fork` und `exec`. Erlaubt ist
            // dort nur, was async-signal-sicher ist: `fcntl(2)` ist es, und
            // die Schleife liest einen Vektor, der vor dem `fork` gebaut
            // wurde, ohne zu allozieren. Jede Nummer kam aus einer
            // `BorrowedFd`, war beim Aufruf also offen, der Aufrufer hält sie
            // bis nach dem `spawn` offen, und sie liegt über den
            // Standardströmen, die die Standardbibliothek vorher umbiegt.
            let fd = unsafe { BorrowedFd::borrow_raw(fd) };
            fcntl_setfd(fd, FdFlags::empty())
                .map_err(|_| std::io::Error::from_raw_os_error(HOOK_ERRNO))?;
        }
        Ok(())
    };
    // SAFETY: Der Hook hält sich an die Regeln für `pre_exec`: kein Lock,
    // keine Allokation, nur ein async-signal-sicherer Systemaufruf je
    // Deskriptor (siehe oben).
    unsafe {
        command.pre_exec(hook);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Read as _;
    use std::os::fd::{AsFd as _, AsRawFd as _, FromRawFd as _, OwnedFd};
    use std::process::Command;

    use rustix::io::{FdFlags, fcntl_getfd};

    use super::{above_stdio, inherit_in_child};
    use crate::pinned::{PinnedSource, SourceKind};

    /// Setzt der Elterntest, damit [`stdio_slot_child`] im Kindprozess
    /// wirklich etwas tut.
    const CHILD_ENV: &str = "HUMANITL_TEST_HUM263_STDIO_CHILD";

    /// Das Kind schreibt in die geerbte Pipe; im Elternprozess trägt sie
    /// dabei und danach weiter `FD_CLOEXEC`.
    #[test]
    fn the_child_inherits_and_the_parent_keeps_cloexec() {
        let (mut reader, writer) = std::io::pipe().unwrap();
        let writer = OwnedFd::from(writer);
        let number = writer.as_raw_fd();
        assert!(
            fcntl_getfd(writer.as_fd())
                .unwrap()
                .contains(FdFlags::CLOEXEC)
        );

        // Über `/proc/self/fd/N`: `dash` kennt `>&N` nur für eine Ziffer.
        let mut command = Command::new("/bin/sh");
        command.args(["-c", &format!("echo inherited > /proc/self/fd/{number}")]);
        inherit_in_child(&mut command, &[writer.as_fd()]).expect("above the standard streams");
        let status = command.status().expect("sh runs");
        assert!(status.success(), "{status}");

        assert!(
            fcntl_getfd(writer.as_fd())
                .unwrap()
                .contains(FdFlags::CLOEXEC),
            "the parent's descriptor stays close-on-exec"
        );
        drop(writer);
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, "inherited\n");
    }

    /// Ohne den Hook erbt das Kind nichts: Das zeigt, dass der Test oben die
    /// Vererbung misst und nicht einen Deskriptor, der ohnehin offen wäre.
    #[test]
    fn without_the_hook_the_child_inherits_nothing() {
        let (_reader, writer) = std::io::pipe().unwrap();
        let writer = OwnedFd::from(writer);
        let number = writer.as_raw_fd();
        let status = Command::new("/bin/sh")
            .args(["-c", &format!("echo x > /proc/self/fd/{number}")])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("sh runs");
        assert!(!status.success(), "the descriptor is closed in the child");
    }

    /// Mit geschlossenem Deskriptor 0 bekommt kein vererbter Deskriptor eine
    /// Nummer unter 3. Läuft im Kindprozess ([`stdio_slot_child`]), damit
    /// dieser Testprozess seine Standardeingabe behält.
    #[test]
    fn a_closed_standard_stream_is_never_handed_on() {
        let exe = std::env::current_exe().unwrap();
        let output = Command::new(exe)
            .args([
                "--exact",
                "inherit::tests::stdio_slot_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, "1")
            .output()
            .expect("the test binary runs");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("stdio slot ok"), "{stdout}\n{stderr}");
    }

    /// Die Hälfte im Kind: schließt 0, öffnet dann, was der Launcher öffnet.
    #[test]
    fn stdio_slot_child() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        // SAFETY: Nur im eigenen Kindprozess, einfädig (`--test-threads=1`),
        // und niemand sonst hält Deskriptor 0 als `OwnedFd`.
        drop(unsafe { OwnedFd::from_raw_fd(0) });

        let dir = tempfile::tempdir().unwrap();
        let pinned = PinnedSource::open(dir.path(), SourceKind::Directory).expect("pins");
        assert!(pinned.raw_fd() >= 3, "pinned on {}", pinned.raw_fd());
        drop(pinned);

        // Die Lese-Seite bekäme die freie 0.
        let (reader, _writer) = std::io::pipe().unwrap();
        assert_eq!(reader.as_raw_fd(), 0, "the free slot is 0");
        let reader = above_stdio(OwnedFd::from(reader)).unwrap();
        assert!(reader.as_raw_fd() >= 3, "pipe on {}", reader.as_raw_fd());

        // Was doch auf 0 liegt, nimmt der Helfer nicht an.
        let low = std::fs::File::open("/dev/null").unwrap();
        assert_eq!(low.as_raw_fd(), 0, "the free slot is 0");
        let err = inherit_in_child(&mut Command::new("/bin/true"), &[low.as_fd()])
            .expect_err("a descriptor on 0 is refused");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        println!("stdio slot ok");
    }
}
