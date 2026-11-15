//! Die Wegwerf-Sitzung der Kernel-Stufe (HUM-446).
//!
//! Wie unter bwrap (HUM-350) legt der Daemon die obere Schicht als
//! Verzeichnis der Sitzung im Arbeitsspeicher an (`upper` und `work` unter
//! einem Verzeichnis, [`crate::Layer`]). Ein dritter virtiofsd liefert dieses
//! Verzeichnis als Deskriptor aus (`--bind-fd`, [`super::lines::layer_line`]);
//! im Gast hängt der Shim `/work` als Overlay aus dem nur lesbaren
//! Projektordner und dieser Schicht ein, nachdem er das Modul `overlay` vor
//! der Modulsperre geladen hat. Der Projektordner ist in der Unit nur lesbar
//! und sein virtiofsd läuft mit `--readonly`: Ein Wegwerf-Lauf schreibt nie
//! ins Projekt, auch nicht mit Root im Gast.
//!
//! **Alles in der Schicht ist vom Angreifer geschrieben.** Root im Gast kann
//! die Schicht über den Tag `layer` erneut einhängen und darin schreiben, was
//! er will: Whiteouts, erweiterte Attribute, Rechte, Verweise, harte
//! Verweise. Gerätedateien außer dem Whiteout `0:0` legt virtiofsd ohne
//! `CAP_MKNOD` nicht an. Der Daemon liest die Schicht deshalb erst, wenn die
//! Unit fort ist ([`crate::disposable::ensure_ended`]), mit dem Scan, der
//! auch unter bwrap gilt, und der lehnt dazu jede Änderung auf, unter oder
//! über einer Maske ab: Im Gast liegen die Masken in der unteren Schicht,
//! und Root im Gast könnte die Schicht an ihnen vorbei beschreiben.

use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{SANDBOX_011, SANDBOX_030};
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{AtFlags, FileType};

use crate::disposable::OverlayDirs;
use crate::pinned::{PinnedSource, SourceKind};

use super::lines::LAYER_SHARE;

/// Der Name der oberen Schicht im Verzeichnis der Schicht.
pub(crate) const UPPER: &str = "upper";
/// Der Name des Arbeitsverzeichnisses des Overlays daneben.
pub(crate) const WORK: &str = "work";

/// Wie tief die Suche nach dem Modul `overlay` höchstens steigt.
const MAX_DEPTH: usize = 10;
/// Wie viele Einträge die Suche höchstens liest.
const MAX_ENTRIES: usize = 200_000;

/// Das Verzeichnis der Schicht, einmal geöffnet (HUM-263) und geprüft.
#[derive(Debug)]
pub(crate) struct PinnedLayer {
    dir: PinnedSource,
    property: String,
    dev: u64,
    ino: u64,
}

impl PinnedLayer {
    /// Öffnet das Verzeichnis über `upper` und `work` und prüft, dass beide
    /// darin als Verzeichnisse liegen, unter genau diesen Namen.
    ///
    /// # Errors
    ///
    /// `SANDBOX_011`, wenn die beiden nicht nebeneinander liegen, nicht so
    /// heißen, keine Verzeichnisse sind oder das Verzeichnis sich nicht öffnen
    /// lässt; `SANDBOX_006`, wenn sein Pfad nicht in die Unit taugt.
    pub(crate) fn open(dirs: &OverlayDirs) -> Result<Self, Diagnostic> {
        let parent = dirs.upper.parent();
        let named = dirs.upper.file_name() == Some(UPPER.as_ref())
            && dirs.work.file_name() == Some(WORK.as_ref());
        let Some(parent) = parent.filter(|parent| named && dirs.work.parent() == Some(*parent))
        else {
            return Err(failed(format!(
                "the disposable layer is {} with {}; the kernel level needs both as `{UPPER}` \
                 and `{WORK}` in one directory",
                dirs.upper.display(),
                dirs.work.display()
            )));
        };
        let dir = PinnedSource::open(parent, SourceKind::Directory)
            .map_err(|why| failed(format!("the disposable layer: {why}")))?;
        for name in [UPPER, WORK] {
            let stat = rustix::fs::statat(dir.fd(), name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|err| failed(format!("the disposable layer: {name}: {err}")))?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
                return Err(failed(format!(
                    "the disposable layer: {name} in {} is not a directory",
                    dir.resolved().display()
                )));
            }
        }
        let meta = std::fs::metadata(dir.proc_path())
            .map_err(|err| failed(format!("the disposable layer: {err}")))?;
        let property = super::property_path(dir.resolved())?;
        Ok(Self {
            dir,
            property,
            dev: meta.dev(),
            ino: meta.ino(),
        })
    }

    /// Der Pfad als Eigenschaft der Unit (`ReadWritePaths`).
    pub(crate) fn property(&self) -> &str {
        &self.property
    }

    /// Was die Host-Prüfung beim virtiofsd der Schicht sucht.
    pub(crate) fn shared_dir(&self) -> crate::launcher::SharedDir {
        crate::launcher::SharedDir {
            jail_path: PathBuf::from(LAYER_SHARE),
            dev: self.dev,
            ino: self.ino,
        }
    }

    /// Das geöffnete Verzeichnis, solange der Plan es hält (HUM-485).
    pub(crate) fn fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.dir.fd()
    }

    /// Der Deskriptor, der an die Unit geht.
    pub(crate) fn into_fd(self) -> OwnedFd {
        self.dir.into_fd()
    }
}

fn failed(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(why)
        .build()
}

/// `SANDBOX_030` mit dem Rückweg auf `direct`.
fn refused(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        .why(why)
        .fix(FixAction::ChangeSetting {
            key: "sandbox.mode".to_owned(),
            value: "direct".to_owned(),
        })
        .build()
}

/// Ein Klon als Schicht gibt es nur unter macOS (HUM-374).
pub(crate) fn clone_refused() -> Diagnostic {
    refused(
        "the kernel level takes a disposable layer as an overlay directory, not as a clone of \
         the project"
            .to_owned(),
    )
}

/// Ob der Gastkernel das Modul `overlay` hat: als Datei unter `modules`
/// (`overlay.ko`, `.ko.xz` oder `.ko.zst`, wie der Shim sie sucht) oder
/// eingebaut (`modules.builtin`).
///
/// # Errors
///
/// `SANDBOX_030` mit dem Verzeichnis, wenn es nicht da ist.
pub(crate) fn overlay_module(modules: &Path) -> Result<(), Diagnostic> {
    if has_overlay_module(modules) {
        return Ok(());
    }
    Err(refused(format!(
        "the guest kernel's module directory {} has no overlay module, which a disposable \
         sandbox of the kernel level needs; set sandbox.mode = \"direct\", or set \
         sandbox.qemu.modules to the modules of a kernel with overlayfs",
        modules.display()
    )))
}

/// [`overlay_module`] ohne Befund.
pub(crate) fn has_overlay_module(modules: &Path) -> bool {
    let builtin = std::fs::read_to_string(modules.join("modules.builtin")).is_ok_and(|text| {
        text.lines()
            .any(|line| line.trim().rsplit('/').next() == Some("overlay.ko"))
    });
    builtin || find_module(modules, "overlay")
}

/// Sucht `<name>.ko`, `.ko.xz` oder `.ko.zst` unter `dir`, ohne einem
/// Verweis zu folgen, höchstens [`MAX_DEPTH`] tief und über
/// [`MAX_ENTRIES`] Einträge.
fn find_module(dir: &Path, name: &str) -> bool {
    let files = ["ko", "ko.xz", "ko.zst"].map(|ending| format!("{name}.{ending}"));
    let mut stack = vec![(dir.to_path_buf(), 0)];
    let mut seen = 0;
    while let Some((current, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return false;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let file = entry.file_name();
            if kind.is_file() && files.iter().any(|wanted| file == wanted.as_str()) {
                return true;
            }
            if kind.is_dir() && depth + 1 < MAX_DEPTH {
                stack.push((entry.path(), depth + 1));
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Das Modul wird als Datei in jeder Endung gefunden, oder als
    /// eingebaut; fehlt beides, ist es `SANDBOX_030` mit dem Rückweg.
    #[test]
    fn the_overlay_module_is_found_as_a_file_or_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let err = overlay_module(dir.path()).expect_err("an empty directory");
        assert_eq!(err.code.as_str(), "SANDBOX_030");
        assert!(err.why.contains("no overlay module"), "{}", err.why);
        // Ein ähnlicher Name zählt nicht.
        let deep = dir.path().join("kernel/fs/overlayfs");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("overlayx.ko.xz"), b"").unwrap();
        assert!(!has_overlay_module(dir.path()));
        for ending in ["ko", "ko.xz", "ko.zst"] {
            let file = deep.join(format!("overlay.{ending}"));
            std::fs::write(&file, b"").unwrap();
            assert!(has_overlay_module(dir.path()), "{ending}");
            std::fs::remove_file(&file).unwrap();
        }
        std::fs::write(
            dir.path().join("modules.builtin"),
            "kernel/fs/ext4/ext4.ko\nkernel/fs/overlayfs/overlay.ko\n",
        )
        .unwrap();
        overlay_module(dir.path()).expect("builtin");
    }

    /// Die Schicht muss `upper` und `work` in einem Verzeichnis sein, beide
    /// Verzeichnisse; ein Verweis an ihrer Stelle wird abgelehnt.
    #[test]
    fn the_layer_is_upper_and_work_in_one_directory() {
        let dir = tempfile::tempdir().unwrap();
        let layer = dir.path().join("l");
        std::fs::create_dir_all(layer.join(UPPER)).unwrap();
        std::fs::create_dir_all(layer.join(WORK)).unwrap();
        let dirs = OverlayDirs {
            upper: layer.join(UPPER),
            work: layer.join(WORK),
        };
        let pinned = PinnedLayer::open(&dirs).expect("a layer");
        let shared = pinned.shared_dir();
        let meta = std::fs::metadata(&layer).unwrap();
        assert_eq!((shared.dev, shared.ino), (meta.dev(), meta.ino()));
        assert_eq!(shared.jail_path, Path::new(LAYER_SHARE));
        assert_eq!(
            Path::new(pinned.property()),
            std::fs::canonicalize(&layer).unwrap()
        );

        let apart = OverlayDirs {
            upper: layer.join(UPPER),
            work: dir.path().join(WORK),
        };
        assert_eq!(
            PinnedLayer::open(&apart).unwrap_err().code.as_str(),
            "SANDBOX_011"
        );
        let renamed = OverlayDirs {
            upper: layer.join("up"),
            work: layer.join(WORK),
        };
        assert!(PinnedLayer::open(&renamed).is_err());
        std::fs::remove_dir(layer.join(WORK)).unwrap();
        std::os::unix::fs::symlink(layer.join(UPPER), layer.join(WORK)).unwrap();
        let err = PinnedLayer::open(&dirs).unwrap_err();
        assert!(err.why.contains("not a directory"), "{}", err.why);
    }
}
