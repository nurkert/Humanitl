//! Eine Mount-Quelle, einmal geöffnet und als Deskriptor gehalten (HUM-263).
//!
//! Vorher prüfte der Launcher den Projektordner per Pfad, und `bwrap` löste
//! denselben Pfad beim Einhängen ein zweites Mal auf. Wer einen Elternordner
//! beschreiben kann, tauschte den Ordner dazwischen gegen einen Symlink auf
//! `~/.ssh` oder `~/.config/humanitl`, und die Sandbox sah genau das, was die
//! Prüfung ausschließen sollte. Das kann ein anderes Konto mit Schreibrecht auf
//! den Elternordner sein, oder der Agent einer anderen Sitzung, wenn der neue
//! Projektordner in deren beschreibbarem `/work` liegt.
//!
//! [`PinnedSource::open`] öffnet die Quelle deshalb genau einmal mit
//! `O_PATH | O_NOFOLLOW | O_CLOEXEC` (beim Projektordner zusätzlich
//! `O_DIRECTORY`) und liest danach aus `/proc/self/fd/N`, wo das geöffnete
//! Objekt wirklich liegt. Die [`crate::MountPolicy`] prüft diesen Pfad, und
//! `bwrap` bekommt den Deskriptor, nicht den Pfad. Ein Tausch nach dem Öffnen
//! ändert nichts mehr: Der Deskriptor zeigt weiter auf das geprüfte
//! Verzeichnis. Ein Tausch zwischen Auflösen und Öffnen fällt am Vergleich
//! auf, und der Start bricht ab.
//!
//! **Der Deskriptor darf den Agenten nie erreichen.** Ein `O_PATH`-Deskriptor
//! auf einen Ordner des Hosts ist ein Ausweg aus der Sandbox:
//! `openat(fd, "..")` läuft im Dateibaum des Hosts nach oben, an jedem
//! Mount-Namensraum vorbei. `--bind-fd` schließt ihn in `bwrap` selbst (ab
//! [`crate::bwrap::BIND_FD_VERSION`]); beim Rückfall über `/proc/self/fd/N`
//! schließt ihn der Shim, bevor er den Agenten startet (`close_inherited`).

use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};

use rustix::fs::{FileType, Mode, OFlags, fstat, open};

/// Was für eine Quelle geöffnet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    /// Nur ein Verzeichnis, etwa der Projektordner.
    Directory,
    /// Ein Verzeichnis oder eine gewöhnliche Datei, etwa ein Eintrag aus
    /// `mounts.extra_ro`.
    DirectoryOrFile,
}

/// Eine geöffnete Mount-Quelle und der Ort, an dem sie liegt.
#[derive(Debug)]
pub(crate) struct PinnedSource {
    fd: OwnedFd,
    resolved: PathBuf,
}

impl PinnedSource {
    /// Löst `path` auf, öffnet das Ergebnis ohne Symlink am Ende und
    /// vergleicht den Ort des Deskriptors mit dem aufgelösten Pfad.
    ///
    /// # Errors
    ///
    /// Ein Satz für das `why` eines Befunds, wenn die Quelle fehlt, nicht von
    /// der verlangten Art ist, sich nicht öffnen lässt oder zwischen Auflösen
    /// und Öffnen getauscht wurde.
    pub(crate) fn open(path: &Path, kind: SourceKind) -> Result<Self, String> {
        let shown = path.display();
        let canonical = std::fs::canonicalize(path)
            .map_err(|err| format!("{shown} cannot be resolved: {err}"))?;
        let mut flags = OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        if kind == SourceKind::Directory {
            flags |= OFlags::DIRECTORY;
        }
        let fd = open(&canonical, flags, Mode::empty()).map_err(|err| {
            format!(
                "{shown} cannot be opened as {}: {err}; it may have been replaced after it was resolved to {}",
                kind.noun(),
                canonical.display()
            )
        })?;
        // Nie auf 0, 1 oder 2: Dort ersetzte die stdio des Kindes ihn, bevor
        // er vererbt wird (`inherit.rs`).
        let fd = crate::inherit::above_stdio(fd)
            .map_err(|err| format!("{shown} cannot be moved above the standard streams: {err}"))?;
        // `O_PATH | O_NOFOLLOW` öffnet einen Symlink, statt abzulehnen. Ein
        // Symlink an dieser Stelle ist ein Tausch nach dem Auflösen, und
        // `bwrap` folgte ihm womöglich beim Einhängen.
        let stat = fstat(fd.as_fd())
            .map_err(|err| format!("{shown} cannot be inspected after opening: {err}"))?;
        let file_type = FileType::from_raw_mode(stat.st_mode);
        let acceptable = match kind {
            SourceKind::Directory => file_type == FileType::Directory,
            SourceKind::DirectoryOrFile => {
                matches!(file_type, FileType::Directory | FileType::RegularFile)
            }
        };
        if !acceptable {
            return Err(format!(
                "{shown} is not {} once opened ({file_type:?}); it was replaced after it was resolved to {}",
                kind.noun(),
                canonical.display()
            ));
        }
        let resolved = std::fs::read_link(proc_path(fd.as_raw_fd())).map_err(|err| {
            format!("cannot read where the opened {shown} lies (/proc/self/fd): {err}")
        })?;
        if resolved != canonical {
            return Err(format!(
                "{shown} changed while it was opened: it resolved to {}, but the opened {} lies at {}",
                canonical.display(),
                kind.noun(),
                resolved.display()
            ));
        }
        Ok(Self { fd, resolved })
    }

    /// Der Ort des geöffneten Objekts, wie `/proc/self/fd/N` ihn nennt; der
    /// Pfad, gegen den die Politik prüft.
    pub(crate) fn resolved(&self) -> &Path {
        &self.resolved
    }

    /// `/proc/self/fd/N` für diesen Deskriptor: ein Weg zum geöffneten
    /// Objekt, den niemand außer diesem Prozess umbiegen kann.
    pub(crate) fn proc_path(&self) -> PathBuf {
        proc_path(self.fd.as_raw_fd())
    }

    /// Die Nummer des Deskriptors.
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// Gibt den Deskriptor heraus, damit der Launcher ihn vererbt.
    pub(crate) fn into_fd(self) -> OwnedFd {
        self.fd
    }
}

impl SourceKind {
    const fn noun(self) -> &'static str {
        match self {
            Self::Directory => "a directory",
            Self::DirectoryOrFile => "a directory or a regular file",
        }
    }
}

fn proc_path(fd: RawFd) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{fd}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::net::UnixListener;

    use super::{PinnedSource, SourceKind};

    #[test]
    fn the_descriptor_keeps_the_checked_directory_after_a_swap() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let protected = dir.path().join("protected");
        std::fs::create_dir(&work).unwrap();
        std::fs::create_dir(&protected).unwrap();
        std::fs::write(work.join("marker"), "checked\n").unwrap();
        std::fs::write(protected.join("marker"), "protected\n").unwrap();

        let pinned = PinnedSource::open(&work, SourceKind::Directory).expect("opens");
        assert_eq!(pinned.resolved(), std::fs::canonicalize(&work).unwrap());

        std::fs::rename(&work, dir.path().join("work.old")).unwrap();
        std::os::unix::fs::symlink(&protected, &work).unwrap();

        let seen = std::fs::read_to_string(pinned.proc_path().join("marker")).unwrap();
        assert_eq!(
            seen, "checked\n",
            "the descriptor still names the old directory"
        );
    }

    #[test]
    fn a_symlinked_project_folder_resolves_to_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let pinned = PinnedSource::open(&link, SourceKind::Directory).expect("opens");
        assert_eq!(
            pinned.resolved(),
            std::fs::canonicalize(&target).unwrap(),
            "the policy sees where the folder really is"
        );
    }

    #[test]
    fn missing_files_and_sockets_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let err = PinnedSource::open(&missing, SourceKind::DirectoryOrFile).unwrap_err();
        assert!(err.contains("cannot be resolved"), "{err}");

        let file = dir.path().join("file");
        std::fs::write(&file, "x").unwrap();
        let err = PinnedSource::open(&file, SourceKind::Directory).unwrap_err();
        assert!(err.contains("a directory"), "{err}");
        PinnedSource::open(&file, SourceKind::DirectoryOrFile).expect("a file may be an extra");

        let socket = dir.path().join("sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        let err = PinnedSource::open(&socket, SourceKind::DirectoryOrFile).unwrap_err();
        assert!(err.contains("not a directory or a regular file"), "{err}");
    }
}
