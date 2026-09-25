//! Was `daemon install` aus einem `AppImage` herauskopiert, und in welchem
//! Aufbau (HUM-070, HUM-165).
//!
//! Im Bild liegt unter `usr/lib/humanitl/` derselbe Baum wie im Archiv
//! (`packaging/appimage/build-appimage.sh`):
//!
//! ```text
//! bin/humanitld, bin/humanitl-shim
//! share/humanitl/catalog/        Domain-Katalog
//! profiles/sandbox/default.toml  Sandbox-Profil
//! ```
//!
//! Die Kopie unter `~/.local/lib/humanitl/<version>.<stempel>/` hat genau
//! diesen Aufbau. Der Daemon sucht Katalog und Profil relativ zu seinem eigenen
//! Pfad: den Katalog unter `<exe>/../../share/humanitl/catalog` (`catalog_dir`
//! in `daemon/bin/humanitld/src/main.rs`), das Profil in einem Vorfahren unter
//! `profiles/sandbox` (`tree_dirs` im Sandbox-Dienst), den Shim neben sich.
//! Fehlte einer der drei in der Kopie, liefe der Daemon mit leerem Katalog
//! oder ohne Sandbox-Profil (HUM-165). Deshalb gehören sie zur Kopie und
//! werden vorher geprüft wie die Binaries.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};

use super::not_staged;
use crate::cmd::{Failure, unit};

/// Das Unterverzeichnis mit den Binaries, im Bild wie in der Kopie.
pub(super) const BIN_DIR: &str = "bin";

/// Die Binaries, die ein `AppImage`-Lauf herauskopiert, unter [`BIN_DIR`].
pub(super) const STAGED_BINARIES: [&str; 2] = [unit::DAEMON_NAME, "humanitl-shim"];

/// Wo es ein vollständiges `AppImage` gibt: die Seite der Veröffentlichungen.
const RELEASES_URL: &str = "https://github.com/nurkert/Humanitl/releases";

/// Die Datenverzeichnisse, die mitgehen, jeweils mit der Datei, ohne die der
/// Daemon sie nicht benutzt.
///
/// Der Katalog zählt nur mit `domains.yaml` (`humanitl_catalog::DOMAINS_FILE`),
/// das Profil `default` ist das, das ohne Einstellung gilt.
const STAGED_DATA: [(&str, &str); 2] = [
    ("share/humanitl/catalog", "domains.yaml"),
    ("profiles/sandbox", "default.toml"),
];

/// Die Wurzel des Baums im Bild: das Verzeichnis über `bin/`.
///
/// `source` ist das Verzeichnis der laufenden Kommandozeile oder `--bin-dir`,
/// also `usr/lib/humanitl/bin` im Bild. Es wird erst aufgelöst: Die Eltern
/// eines Pfads wie `.` oder `x/..` sind lexikalisch `""` und `x`, nicht das
/// Verzeichnis darüber. Lässt es sich nicht auflösen, bleibt es beim Pfad,
/// wie er ist; die Prüfung danach meldet dann, was fehlt.
fn image_root(source: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    resolved
        .parent()
        .map_or_else(|| resolved.clone(), Path::to_path_buf)
}

/// Der Pfad, den `ExecStart` unter dem Verweis `current` nennt.
pub(super) fn daemon_in(copy: &Path) -> PathBuf {
    copy.join(BIN_DIR).join(unit::DAEMON_NAME)
}

/// Prüft, dass im Bild alles liegt, was die Kopie braucht, bevor irgendetwas
/// geschrieben wird.
///
/// # Errors
///
/// `DAEMON_007` für ein fehlendes Binary, `DAEMON_011` für einen fehlenden
/// Katalog oder ein fehlendes Profil.
pub(super) fn check(source: &Path) -> Result<(), Failure> {
    for name in STAGED_BINARIES {
        let from = source.join(name);
        if !crate::cmd::is_executable(&from) {
            return Err(Failure::new(unit::missing_binary(
                &from,
                "there is no such executable in the AppImage next to the running humanitl",
            )));
        }
    }
    let root = image_root(source);
    for (dir, required) in STAGED_DATA {
        let file = root.join(dir).join(required);
        if !file.is_file() {
            return Err(Failure::new(incomplete_image(&file)));
        }
    }
    Ok(())
}

/// `DAEMON_011`: Im Bild fehlt Katalog oder Profil.
///
/// Nicht [`not_staged`]: Dessen Fix ist ein `ls` auf das Verzeichnis, und das
/// liegt hier unter dem Einhängepunkt `/tmp/.mount_*`, den es nach dem Lauf
/// nicht mehr gibt. Ein Bild, dem eine Datei fehlt, repariert niemand von
/// Hand; es hilft nur ein vollständiges.
fn incomplete_image(file: &Path) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_011, Severity::Blocking)
        .why(format!(
            "{} is missing in the AppImage; without it the copied daemon would run with an \
             empty domain catalog or without a sandbox profile, so nothing was copied and no \
             unit was written. The AppImage is incomplete; download it again",
            file.display()
        ))
        .fix(FixAction::OpenUrl(RELEASES_URL.to_owned()))
        .build()
}

/// `DAEMON_011`: Eine Datei oder ein Verzeichnis im Bild ließ sich nicht
/// lesen.
///
/// Wie [`incomplete_image`] ohne `ls` auf den Einhängepunkt: Ein Bild, das
/// sich nicht lesen lässt, ist beschädigt, und es hilft nur ein neues.
fn unreadable_image(path: &Path, error: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_011, Severity::Blocking)
        .why(format!(
            "{} in the AppImage cannot be read ({error}); the copy was taken back and no unit \
             was written. The AppImage is damaged or incomplete; download it again",
            path.display()
        ))
        .fix(FixAction::OpenUrl(RELEASES_URL.to_owned()))
        .build()
}

/// Kopiert Binaries, Katalog und Profil aus dem Bild nach `copy`, im Aufbau
/// des Bildes.
///
/// `copy` gibt es schon und ist leer; es gehört diesem Lauf allein
/// ([`super::stage`]). Aufräumen bei einem Fehlschlag ist Sache des Aufrufers.
///
/// # Errors
///
/// `DAEMON_011` mit dem Pfad, der sich nicht schreiben oder lesen ließ.
pub(super) fn copy_into(source: &Path, copy: &Path) -> Result<(), Diagnostic> {
    let bin = copy.join(BIN_DIR);
    make_dir(&bin)?;
    for name in STAGED_BINARIES {
        copy_file(&source.join(name), &bin.join(name))?;
    }
    legacy_link(copy)?;
    let root = image_root(source);
    for (dir, _) in STAGED_DATA {
        copy_tree(&root.join(dir), &copy.join(dir))?;
    }
    Ok(())
}

/// Legt `humanitld` neben `bin/` als Verweis auf `bin/humanitld` an.
///
/// Vor HUM-165 nannte die Unit `current/humanitld`. [`super::stage`] hängt
/// `current` um, bevor die Unit mit `current/bin/humanitld` geschrieben ist;
/// bricht der Lauf genau dazwischen ab, startete die alte Unit beim nächsten
/// Anmelden sonst ins Leere. Der Verweis ist relativ, damit er in jeder Kopie
/// auf deren eigenes Binary zeigt. Der Daemon sieht als eigenen Pfad das Ziel
/// (`/proc/self/exe`), findet Katalog und Profil also auch über diesen Weg.
fn legacy_link(copy: &Path) -> Result<(), Diagnostic> {
    let link = copy.join(unit::DAEMON_NAME);
    std::os::unix::fs::symlink(Path::new(BIN_DIR).join(unit::DAEMON_NAME), &link)
        .map_err(|error| not_staged(&link, &format!("cannot be linked: {error}")))
}

/// Kopiert ein Verzeichnis samt Unterverzeichnissen.
///
/// Verzeichnisse werden nachgebaut, alles andere mit [`std::fs::copy`]
/// kopiert; das folgt einem Verweis auf eine Datei und scheitert an einem
/// Verweis auf ein Verzeichnis. Einem solchen Verweis zu folgen könnte im Kreis
/// führen, und im Bild gibt es keinen.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Diagnostic> {
    make_dir(to)?;
    let entries = std::fs::read_dir(from).map_err(|error| unreadable_image(from, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| unreadable_image(from, &error))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if is_dir {
            copy_tree(&source, &target)?;
        } else {
            copy_file(&source, &target)?;
        }
    }
    Ok(())
}

/// Legt ein Verzeichnis samt fehlender Eltern an.
fn make_dir(dir: &Path) -> Result<(), Diagnostic> {
    std::fs::create_dir_all(dir)
        .map_err(|error| not_staged(dir, &format!("cannot be created: {error}")))
}

/// Kopiert eine Datei.
///
/// Die Quelle wird vorher zum Lesen geöffnet: [`std::fs::copy`] sagt im
/// Fehler nicht, welche Seite scheiterte. Ein Lesefehler liegt am Bild
/// ([`unreadable_image`]), ein Fehler danach am Ziel ([`not_staged`]).
fn copy_file(from: &Path, to: &Path) -> Result<(), Diagnostic> {
    std::fs::File::open(from).map_err(|error| unreadable_image(from, &error))?;
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| not_staged(to, &format!("cannot be written: {error}")))
}
