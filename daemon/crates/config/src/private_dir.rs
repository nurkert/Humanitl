//! Das private Laufzeitverzeichnis: anlegen auf der Seite des Daemons, prüfen
//! auf der Seite der Clients (HUM-212).
//!
//! Ohne `XDG_RUNTIME_DIR` und ohne `/run/user/<uid>` liegt das
//! Laufzeitverzeichnis unter `$TMPDIR/humanitl-<uid>` ([`crate::Paths::runtime_dir`]).
//! Der Name ist vorhersagbar, und `/tmp` teilen sich alle Konten. Ein anderer
//! Nutzer kann das Verzeichnis also vor dem ersten Start anlegen, mit eigenem
//! Socket und eigenem Token, oder einen Symlink an seine Stelle legen. Beide
//! Seiten verlassen sich deshalb nicht darauf, dass ein vorhandenes Verzeichnis
//! ihnen gehört:
//!
//! - [`ensure_private_dir`] legt das Verzeichnis an oder übernimmt ein
//!   vorhandenes nur, wenn es kein Symlink ist und dem eigenen Konto gehört,
//!   und setzt danach `0700` über den geöffneten Deskriptor
//!   (`DAEMON_004` sonst).
//! - [`check_private`] prüft vor dem Lesen des Tokens Verzeichnis und
//!   Token-Datei per `lstat`: eigene UID, kein Symlink, keine Rechte für Gruppe
//!   oder Andere (`DAEMON_001` sonst).

use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
use std::path::Path;

use humanitl_core::diagnostics::codes;
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

use crate::paths::DIR_MODE;

/// Die reale Nutzerkennung des laufenden Prozesses.
#[must_use]
pub fn process_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Was an einem Pfad erwartet wird, den [`check_private`] prüft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Ein Verzeichnis, etwa das Laufzeitverzeichnis.
    Dir,
    /// Eine reguläre Datei, etwa das Token.
    File,
}

/// Legt `dir` mit `0700` an oder übernimmt ein vorhandenes Verzeichnis, das
/// `uid` gehört, und setzt es auf `0700`.
///
/// Das Verzeichnis wird mit `O_NOFOLLOW | O_DIRECTORY` geöffnet; Besitzer und
/// Rechte laufen über diesen Deskriptor (`fstat`, `fchmod`). Ein Symlink an der
/// Stelle scheitert beim Öffnen, statt dass ihm gefolgt würde, und zwischen
/// Prüfung und `chmod` kann niemand den Eintrag austauschen.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn sich das Verzeichnis nicht anlegen
/// lässt, an seiner Stelle ein Symlink oder etwas anderes als ein Verzeichnis
/// liegt, oder es einem anderen Konto gehört.
pub fn ensure_private_dir(dir: &Path, uid: u32) -> Result<(), Diagnostic> {
    DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .map_err(|error| {
            // Ein Symlink ins Leere an der Stelle scheitert hier mit `EEXIST`,
            // nicht erst beim Öffnen; er ist derselbe Befund wie jeder Symlink.
            if fs::symlink_metadata(dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
                symlink_refusal(dir, &error.to_string())
            } else {
                io_refusal("create the runtime directory", dir, &error)
            }
        })?;
    let fd = rustix::fs::open(
        dir,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|errno| match errno {
        Errno::LOOP | Errno::NOTDIR => symlink_refusal(dir, &errno.to_string()),
        _ => refusal(&format!(
            "cannot open {} ({errno}); it may belong to another account",
            dir.display()
        )),
    })?;
    let stat = rustix::fs::fstat(&fd)
        .map_err(|errno| io_refusal("read the runtime directory", dir, &errno.into()))?;
    if stat.st_uid != uid {
        return Err(refusal(&format!(
            "the runtime directory {} belongs to uid {}, not to you (uid {uid}); \
                 another account may have created it to receive your socket and token",
            dir.display(),
            stat.st_uid
        )));
    }
    rustix::fs::fchmod(&fd, Mode::from_raw_mode(DIR_MODE))
        .map_err(|errno| io_refusal("set 0700 on the runtime directory", dir, &errno.into()))
}

/// Warum [`check_private`] einem Pfad nicht traut.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Der Pfad fehlt: Es läuft kein Daemon. Der Vorschlag ist, ihn zu
    /// starten, auch im `/tmp`-Rückfall.
    Missing(Diagnostic),
    /// Der Pfad ist da, aber fremd, ein Symlink oder offen.
    Untrusted(Diagnostic),
}

impl Refusal {
    /// Der Befund, im `/tmp`-Rückfall (`fallback`) für einen vorhandenen,
    /// aber nicht vertrauenswürdigen Pfad mit [`with_own_runtime_dir_fix`].
    #[must_use]
    pub fn into_diagnostic(self, fallback: bool) -> Diagnostic {
        match self {
            Self::Untrusted(diagnostic) if fallback => with_own_runtime_dir_fix(diagnostic),
            Self::Missing(diagnostic) | Self::Untrusted(diagnostic) => diagnostic,
        }
    }
}

/// Prüft `path` per `lstat`, bevor ein Client dem Inhalt traut.
///
/// Verlangt wird die Art aus `entry` ohne Symlink, der Besitzer `uid` und keine
/// Rechte für Gruppe oder Andere. Ein fremdes oder offenes Verzeichnis kann ein
/// anderer Nutzer vorbereitet haben; wer dort Token und Socket liest, schickte
/// ihm Tastenanschläge und Einstellungen.
///
/// Der Vorschlag hängt am Befund: für offene Rechte `chmod`, für fremden
/// Besitzer oder einen Symlink keiner, denn beides kann das eigene Konto nicht
/// beheben. Liegt der Pfad im `/tmp`-Rückfall, ersetzt der Aufrufer ihn durch
/// [`with_own_runtime_dir_fix`].
///
/// # Errors
///
/// [`Refusal::Missing`] mit `DAEMON_001`, wenn der Pfad fehlt;
/// [`Refusal::Untrusted`] mit `DAEMON_001`, wenn er ein Symlink ist, die falsche
/// Art hat, einem anderen Konto gehört oder für Gruppe oder Andere offen ist.
pub fn check_private(path: &Path, entry: Entry, uid: u32) -> Result<(), Refusal> {
    let untrusted = |why: String| {
        Refusal::Untrusted(
            Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
                .why(why)
                .build(),
        )
    };
    // Fehlt der Pfad, läuft kein Daemon; das ist kein Befund über ein fremdes
    // Verzeichnis, und der Vorschlag bleibt, den Daemon zu starten.
    let meta = fs::symlink_metadata(path).map_err(|error| {
        Refusal::Missing(
            Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
                .why(format!("cannot stat {}: {error}", path.display()))
                .fix(FixAction::CopyCommand("humanitld".to_owned()))
                .build(),
        )
    })?;
    let kind_ok = match entry {
        Entry::Dir => meta.file_type().is_dir(),
        Entry::File => meta.file_type().is_file(),
    };
    if !kind_ok {
        let expected = match entry {
            Entry::Dir => "a directory",
            Entry::File => "a regular file",
        };
        return Err(untrusted(format!(
            "{} is not {expected} (a symlink here is refused, not followed)",
            path.display()
        )));
    }
    if meta.uid() != uid {
        return Err(untrusted(format!(
            "{} belongs to uid {}, not to you (uid {uid}); the socket and token there are \
             not trusted",
            path.display(),
            meta.uid()
        )));
    }
    let mode = meta.mode() & 0o777;
    if mode & 0o077 != 0 {
        // Ein Zeilenumbruch im Pfad überstünde das Kopieren aus dem Befund
        // nicht; dann gibt es keinen Befehl, wie in der Oberfläche
        // (`exportRefusal`).
        let bytes = path.as_os_str().as_encoded_bytes();
        let fix = (!bytes.contains(&b'\n') && !bytes.contains(&b'\r'))
            .then(|| FixAction::CopyCommand(format!("chmod go-rwx {}", shell_path(path))));
        let builder = Diagnostic::builder(codes::DAEMON_001, Severity::Blocking).why(format!(
            "{} is mode {mode:04o}; the daemon keeps its runtime directory at 0700 and \
             the token at 0600, so an open one was not made by your daemon",
            path.display()
        ));
        let diagnostic = match fix {
            Some(fix) => builder.fix(fix).build(),
            None => builder.build(),
        };
        return Err(Refusal::Untrusted(diagnostic));
    }
    Ok(())
}

/// Ersetzt Vorschlag und Satz eines Befunds über das Laufzeitverzeichnis, wenn
/// es der `/tmp`-Rückfall ist ([`crate::RuntimeDir::diagnostic`] gesetzt).
///
/// Nur dort hilft ein eigenes Verzeichnis unter dem Heimatverzeichnis. In einer
/// Sitzung mit `/run/user/<uid>` zerlegte ein anderes `XDG_RUNTIME_DIR` die
/// grafische Sitzung: Wayland, `PipeWire`, D-Bus und `systemctl --user` fänden
/// ihre Sockets nicht mehr.
#[must_use]
pub fn with_own_runtime_dir_fix(mut diagnostic: Diagnostic) -> Diagnostic {
    diagnostic.why = format!("{}; {OWN_RUNTIME_DIR_HINT}", diagnostic.why);
    diagnostic.fix = Some(own_runtime_dir_fix());
    diagnostic
}

/// Der Vorschlag für den `/tmp`-Rückfall: der Abschnitt der Installation, der
/// erklärt, wie man `XDG_RUNTIME_DIR` für die ganze Sitzung auf ein eigenes
/// Verzeichnis unter dem Heimatverzeichnis setzt.
///
/// `/run/user/<uid>` taugt dafür nicht. Der Rückfall greift gerade dann, wenn
/// es fehlt, und anlegen kann es nur root. Das fremde Verzeichnis unter `/tmp`
/// kann das eigene Konto nicht entfernen.
///
/// Ein Link und kein Befehl: Welche Anmeldedatei eine Sitzung liest, hängt von
/// Shell und Display-Manager ab, und ein falscher Befehl in einer solchen Datei
/// kann die Sitzung beschädigen. Die Anleitung nennt die Fälle für `bash` und
/// `zsh`; der Mensch trägt die Zeile selbst ein.
#[must_use]
pub fn own_runtime_dir_fix() -> FixAction {
    FixAction::OpenUrl(OWN_RUNTIME_DIR_DOC_URL.to_owned())
}

/// Der Abschnitt hinter [`own_runtime_dir_fix`].
pub const OWN_RUNTIME_DIR_DOC_URL: &str =
    "https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md#xdg_runtime_dir-ohne-logind";

/// Der Satz, den jeder Befund mit [`own_runtime_dir_fix`] im `why` trägt.
pub const OWN_RUNTIME_DIR_HINT: &str = "daemon, CLI and app must all see the same XDG_RUNTIME_DIR, set for the whole session (see the linked section for bash and zsh), and it takes effect after logging in again; HUM-222 will let the clients find the directory themselves";

/// Ein Verzeichnis, das der Daemon nicht übernimmt (`DAEMON_004`).
///
/// Ohne Vorschlag: fremder Besitzer und Symlink kann das eigene Konto nicht
/// beheben. Im `/tmp`-Rückfall setzt der Aufrufer [`with_own_runtime_dir_fix`].
fn refusal(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
        .title("Laufzeitverzeichnis nicht privat")
        .why(why)
        .build()
}

/// Ein Symlink an der Stelle des Laufzeitverzeichnisses (`DAEMON_004`).
fn symlink_refusal(dir: &Path, detail: &str) -> Diagnostic {
    refusal(&format!(
        "{} is a symlink or not a directory ({detail}); a runtime directory is never \
         followed through a link",
        dir.display()
    ))
}

/// Ein Fehler des Dateisystems beim Anlegen (`DAEMON_004`).
fn io_refusal(what: &str, path: &Path, error: &io::Error) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
        .why(format!("cannot {what} {}: {error}", path.display()))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt as _;

    use humanitl_core::diagnostics::codes;

    use humanitl_core::FixAction;

    use super::{
        Entry, OWN_RUNTIME_DIR_HINT, Refusal, check_private, ensure_private_dir,
        own_runtime_dir_fix, process_uid,
    };

    fn mode(path: &std::path::Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn creates_a_missing_dir_at_0700() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("a").join("humanitl");
        ensure_private_dir(&dir, process_uid()).unwrap();
        assert_eq!(mode(&dir), 0o700);
    }

    #[test]
    fn tightens_an_own_open_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o755)).unwrap();
        ensure_private_dir(&dir, process_uid()).unwrap();
        assert_eq!(mode(&dir), 0o700);
    }

    /// Der Weg des Befunds: ein anderes Konto hat das Verzeichnis vorab
    /// angelegt. Die fremde UID wird simuliert, indem der Daemon eine andere
    /// als die tatsächliche erwartet.
    #[test]
    fn refuses_a_dir_of_another_account() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o755)).unwrap();
        let error = ensure_private_dir(&dir, process_uid() + 1).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_004);
        assert!(error.why.contains("belongs to uid"), "{}", error.why);
        assert_eq!(mode(&dir), 0o755, "a foreign directory is not touched");
    }

    #[test]
    fn refuses_a_symlink_in_place_of_the_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("elsewhere");
        fs::create_dir(&target).unwrap();
        fs::set_permissions(&target, Permissions::from_mode(0o755)).unwrap();
        let dir = tmp.path().join("humanitl");
        std::os::unix::fs::symlink(&target, &dir).unwrap();
        let error = ensure_private_dir(&dir, process_uid()).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_004);
        assert!(error.why.contains("symlink"), "{}", error.why);
        assert_eq!(mode(&target), 0o755, "the link target is not touched");
    }

    #[test]
    fn refuses_a_dangling_symlink_in_place_of_the_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        std::os::unix::fs::symlink(tmp.path().join("nowhere"), &dir).unwrap();
        let error = ensure_private_dir(&dir, process_uid()).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_004);
        assert!(error.why.contains("symlink"), "{}", error.why);
        assert!(
            !tmp.path().join("nowhere").exists(),
            "the link is not followed"
        );
    }

    /// Ein Verzeichnis, das sich nicht öffnen lässt, ist kein Symlink; der
    /// Befund sagt das nicht fälschlich.
    #[test]
    fn an_unopenable_dir_is_not_called_a_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o000)).unwrap();
        let result = ensure_private_dir(&dir, process_uid());
        fs::set_permissions(&dir, Permissions::from_mode(0o700)).unwrap();
        if rustix::process::geteuid().is_root() {
            return; // root öffnet auch 0000
        }
        let error = result.unwrap_err();
        assert_eq!(error.code, codes::DAEMON_004);
        assert!(error.why.contains("cannot open"), "{}", error.why);
        assert!(!error.why.contains("symlink"), "{}", error.why);
    }

    /// Ohne Wissen um den Rückfall schlägt der Befund nichts vor, was die
    /// Sitzung ändert: kein Vorschlag bei fremdem Besitzer, `chmod` bei offenen
    /// Rechten, mit dem Pfad als ein Wort der Shell, auch mit `'` darin.
    #[test]
    fn the_fix_depends_on_the_finding() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("it's humanitl");
        fs::create_dir(&dir).unwrap();
        let error = ensure_private_dir(&dir, process_uid() + 1).unwrap_err();
        assert_eq!(error.fix, None);
        let error = check_private(&dir, Entry::Dir, process_uid() + 1)
            .unwrap_err()
            .into_diagnostic(false);
        assert_eq!(error.fix, None);
        fs::set_permissions(&dir, Permissions::from_mode(0o755)).unwrap();
        let error = check_private(&dir, Entry::Dir, process_uid())
            .unwrap_err()
            .into_diagnostic(false);
        assert_eq!(
            error.fix,
            Some(FixAction::CopyCommand(format!(
                "chmod go-rwx '{}/it'\\''s humanitl'",
                tmp.path().display()
            )))
        );
        let error = check_private(&dir, Entry::Dir, process_uid())
            .unwrap_err()
            .into_diagnostic(true);
        assert_eq!(error.fix, Some(own_runtime_dir_fix()));
        assert_eq!(error.why.matches(OWN_RUNTIME_DIR_HINT).count(), 1);
    }

    /// Ein fehlender Pfad heißt: kein Daemon. Auch im Rückfall bleibt der
    /// Vorschlag, ihn zu starten, statt auf die Anleitung zu verweisen.
    #[test]
    fn a_missing_path_keeps_the_start_proposal() {
        let tmp = tempfile::tempdir().unwrap();
        let refusal =
            check_private(&tmp.path().join("gone"), Entry::Dir, process_uid()).unwrap_err();
        assert!(matches!(refusal, Refusal::Missing(_)), "{refusal:?}");
        let error = refusal.into_diagnostic(true);
        assert_eq!(
            error.fix,
            Some(FixAction::CopyCommand("humanitld".to_owned()))
        );
    }

    /// Link und Satz, wörtlich; `app/test/core/ipc/private_path_test.dart`
    /// prüft dieselben Literale gegen die Dart-Seite.
    #[test]
    fn the_fallback_fix_links_the_install_section() {
        assert_eq!(
            own_runtime_dir_fix(),
            FixAction::OpenUrl(
                "https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md#xdg_runtime_dir-ohne-logind".to_owned()
            )
        );
        assert_eq!(
            OWN_RUNTIME_DIR_HINT,
            "daemon, CLI and app must all see the same XDG_RUNTIME_DIR, set for the whole session (see the linked section for bash and zsh), and it takes effect after logging in again; HUM-222 will let the clients find the directory themselves"
        );
    }

    /// Ein Zeilenumbruch im Pfad: kein `chmod`-Vorschlag, wie in der
    /// Oberfläche.
    #[test]
    fn a_newline_in_the_path_gets_no_chmod() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["a\nb", "a\rb"] {
            let dir = tmp.path().join(name);
            fs::create_dir(&dir).unwrap();
            fs::set_permissions(&dir, Permissions::from_mode(0o755)).unwrap();
            let error = check_private(&dir, Entry::Dir, process_uid())
                .unwrap_err()
                .into_diagnostic(false);
            assert!(
                error.why.contains("is mode 0755"),
                "{name:?}: {}",
                error.why
            );
            assert_eq!(error.fix, None, "{name:?}");
        }
    }

    #[test]
    fn accepts_an_own_private_dir_and_token() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        ensure_private_dir(&dir, process_uid()).unwrap();
        let token = dir.join("token");
        fs::write(&token, "t").unwrap();
        fs::set_permissions(&token, Permissions::from_mode(0o600)).unwrap();
        check_private(&dir, Entry::Dir, process_uid()).unwrap();
        check_private(&token, Entry::File, process_uid()).unwrap();
    }

    #[test]
    fn client_refuses_a_foreign_dir_and_token() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        ensure_private_dir(&dir, process_uid()).unwrap();
        let token = dir.join("token");
        fs::write(&token, "t").unwrap();
        fs::set_permissions(&token, Permissions::from_mode(0o600)).unwrap();
        for (path, entry) in [(&dir, Entry::Dir), (&token, Entry::File)] {
            let error = check_private(path, entry, process_uid() + 1)
                .unwrap_err()
                .into_diagnostic(false);
            assert_eq!(error.code, codes::DAEMON_001);
            assert!(error.why.contains("belongs to uid"), "{}", error.why);
        }
    }

    #[test]
    fn client_refuses_open_modes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("humanitl");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o755)).unwrap();
        let token = tmp.path().join("token");
        fs::write(&token, "t").unwrap();
        fs::set_permissions(&token, Permissions::from_mode(0o644)).unwrap();
        for (path, entry) in [(&dir, Entry::Dir), (&token, Entry::File)] {
            let error = check_private(path, entry, process_uid())
                .unwrap_err()
                .into_diagnostic(false);
            assert_eq!(error.code, codes::DAEMON_001);
            assert!(error.why.contains("is mode"), "{}", error.why);
        }
    }

    #[test]
    fn client_refuses_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        ensure_private_dir(&real, process_uid()).unwrap();
        let file = real.join("file");
        fs::write(&file, "t").unwrap();
        fs::set_permissions(&file, Permissions::from_mode(0o600)).unwrap();
        let dir_link = tmp.path().join("humanitl");
        let token_link = real.join("token");
        std::os::unix::fs::symlink(&real, &dir_link).unwrap();
        std::os::unix::fs::symlink(&file, &token_link).unwrap();
        for (path, entry) in [(&dir_link, Entry::Dir), (&token_link, Entry::File)] {
            let error = check_private(path, entry, process_uid())
                .unwrap_err()
                .into_diagnostic(false);
            assert_eq!(error.code, codes::DAEMON_001);
            assert!(error.why.contains("symlink"), "{}", error.why);
        }
    }
}
