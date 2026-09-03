//! Der Blob-Speicher: große Bodies als Dateien, benannt nach ihrem Inhalt.
//!
//! Ein Body über `recorder.inline_max_bytes` steht nicht in der Datenbank,
//! sondern unter `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<hex>` (ADR-008,
//! `backlog/CONVENTIONS.md` 3.4). Der Name ist der `SHA-256` des Inhalts, also
//! legt derselbe Body nie zwei Dateien an.
//!
//! # Reihenfolge und Absturz
//!
//! Geschrieben wird immer zuerst der Blob, dann die Zeile in `messages`. Ein
//! Absturz dazwischen hinterlässt eine Datei, auf die niemand zeigt: Platz,
//! kein Datenverlust und keine Zeile, die auf einen fehlenden Body verweist.
//! Die umgekehrte Reihenfolge hätte den schlimmeren Fall.
//!
//! Aufgeräumt wird beides:
//!
//! - Eine abgebrochene Temp-Datei (`.tmp-…`) entfernt [`BlobStore::sweep_temp`]
//!   beim Öffnen.
//! - Eine fertige, aber unreferenzierte Datei entfernt
//!   [`BlobStore::sweep_orphans`] beim Öffnen, sobald sie älter ist als
//!   [`ORPHAN_GRACE`]. Die Frist schützt einen Blob, den ein zweiter Prozess
//!   gerade geschrieben hat, dessen Zeile aber noch nicht committet ist.
//!
//! # Rechte
//!
//! Verzeichnisse `0700`, Dateien `0600`. Ein Body ist der Inhalt einer
//! Anfrage, die der Nutzer moderiert hat; niemand sonst auf dem Rechner soll
//! ihn lesen.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{ErrorKind, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bytes::Bytes;

use crate::error::{RecorderError, blob_failed, blob_failed_at};

/// Rechte der Verzeichnisse des Blob-Speichers.
const DIR_MODE: u32 = 0o700;

/// Rechte einer Blob-Datei.
const FILE_MODE: u32 = 0o600;

/// Präfix, an dem eine noch nicht fertige Datei zu erkennen ist.
const TEMP_PREFIX: &str = ".tmp-";

/// So lange bleibt eine unreferenzierte Datei beim Öffnen unangetastet.
pub const ORPHAN_GRACE: Duration = Duration::from_secs(24 * 60 * 60);

/// Ein `SHA-256` als 64 Kleinbuchstaben-Hex-Zeichen.
///
/// `humanitl-core` hält dieselbe Umwandlung, hat sie aber nicht veröffentlicht
/// (`BodyRef::sha256_hex` ist der einzige Weg dorthin). Zwölf Zeilen hier sind
/// besser als ein Verweis auf ein privates Modul.
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Die Dateien der großen Bodies.
#[derive(Debug)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    /// Öffnet den Speicher und legt das Wurzelverzeichnis an, falls es fehlt.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn das Verzeichnis nicht anlegbar ist oder
    /// seine Rechte nicht gesetzt werden können.
    pub fn open(root: &Path) -> Result<Self, RecorderError> {
        create_dir(root)?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Das Wurzelverzeichnis.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Der Pfad zu einem Blob: `<root>/<hex[0..2]>/<hex>`.
    #[must_use]
    pub fn path(&self, sha256: &[u8; 32]) -> PathBuf {
        let hex = hex_encode(sha256);
        let shard = hex.get(..2).unwrap_or(hex.as_str()).to_owned();
        self.root.join(shard).join(hex)
    }

    /// Wahr, wenn der Blob schon da ist.
    #[must_use]
    pub fn contains(&self, sha256: &[u8; 32]) -> bool {
        self.path(sha256).is_file()
    }

    /// Legt einen Blob ab: Temp-Datei im Zielverzeichnis, `fsync`, `rename`.
    ///
    /// Existiert die Datei bereits, passiert nichts: der Name ist der Inhalt.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`] bei jedem Fehler des Dateisystems.
    pub fn put(&self, sha256: &[u8; 32], data: &[u8]) -> Result<(), RecorderError> {
        let target = self.path(sha256);
        if target.is_file() {
            return Ok(());
        }
        let dir = self.shard_dir(&target)?;
        let (mut file, temp) = Self::temp_in(&dir)?;
        let written = file
            .write_all(data)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(err) = written {
            let _ignored = fs::remove_file(&temp);
            return Err(blob_failed(format!(
                "could not write the blob {} ({err})",
                temp.display()
            )));
        }
        Self::publish(&temp, &target, &dir)
    }

    /// Legt einen Blob ab, dessen Inhalt schon in einer Temp-Datei steht.
    ///
    /// Der Aufrufer hat die Datei mit [`BlobStore::temp`] im richtigen
    /// Verzeichnis erzeugt und geschrieben; hier wird nur noch synchronisiert
    /// und umbenannt.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`] bei jedem Fehler des Dateisystems.
    pub fn put_temp(
        &self,
        sha256: &[u8; 32],
        mut file: File,
        temp: &Path,
    ) -> Result<(), RecorderError> {
        let synced = file.flush().and_then(|()| file.sync_all());
        drop(file);
        if let Err(err) = synced {
            let _ignored = fs::remove_file(temp);
            return Err(blob_failed(format!(
                "could not finish the blob {} ({err})",
                temp.display()
            )));
        }
        let target = self.path(sha256);
        if target.is_file() {
            let _ignored = fs::remove_file(temp);
            return Ok(());
        }
        let dir = self.shard_dir(&target)?;
        Self::publish(temp, &target, &dir)
    }

    /// Eine leere Temp-Datei im Verzeichnis, in dem dieser Blob landen wird.
    ///
    /// Die Datei trägt das Präfix `.tmp-` und die Rechte einer Blob-Datei; sie
    /// wird beim nächsten Öffnen weggeräumt, wenn niemand sie fertigschreibt.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn Verzeichnis oder Datei nicht anlegbar sind.
    pub fn temp(&self, sha256_hint: &[u8; 32]) -> Result<(File, PathBuf), RecorderError> {
        let dir = self.shard_dir(&self.path(sha256_hint))?;
        Self::temp_in(&dir)
    }

    /// Eine Temp-Datei im Verzeichnis für den Schlüssel `00`.
    ///
    /// Wer streamt, kennt den Hash erst am Ende. Die Datei liegt deshalb im
    /// Verzeichnis `00` und wird von dort in ihr Zielverzeichnis umbenannt;
    /// beides liegt im selben Dateisystem, also bleibt das Umbenennen atomar.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn Verzeichnis oder Datei nicht anlegbar sind.
    pub fn temp_staging(&self) -> Result<(File, PathBuf), RecorderError> {
        let dir = self.root.join("00");
        create_dir(&dir)?;
        Self::temp_in(&dir)
    }

    /// Liest einen Blob.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn die Datei fehlt oder nicht lesbar ist.
    pub fn read(&self, sha256: &[u8; 32]) -> Result<Bytes, RecorderError> {
        let path = self.path(sha256);
        match fs::read(&path) {
            Ok(data) => Ok(Bytes::from(data)),
            Err(err) => Err(blob_failed_at(
                &path,
                format!("could not read the blob {} ({err})", path.display()),
            )),
        }
    }

    /// Entfernt einen Blob. Fehlt er schon, ist das kein Fehler.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn die Datei da ist, sich aber nicht löschen
    /// lässt.
    pub fn remove(&self, sha256: &[u8; 32]) -> Result<bool, RecorderError> {
        let path = self.path(sha256);
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
            Err(err) => Err(blob_failed(format!(
                "could not remove the blob {} ({err})",
                path.display()
            ))),
        }
    }

    /// Entfernt liegengebliebene Temp-Dateien und meldet, wie viele es waren.
    ///
    /// Läuft beim Öffnen. Ein Fehler beim Lesen eines Verzeichnisses hält den
    /// Start nicht auf: aufräumen ist Kür, aufzeichnen ist Pflicht.
    #[must_use]
    pub fn sweep_temp(&self) -> u64 {
        let mut removed = 0;
        for entry in self.entries() {
            if is_temp(&entry) && fs::remove_file(&entry).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    /// Entfernt fertige Dateien, auf die keine Zeile zeigt und die älter sind
    /// als [`ORPHAN_GRACE`].
    ///
    /// `referenced` ist die Menge aller `messages.blob_sha256`. Die Frist
    /// schützt einen Blob, dessen Zeile noch nicht geschrieben ist.
    #[must_use]
    pub fn sweep_orphans(&self, referenced: &HashSet<[u8; 32]>, now: SystemTime) -> u64 {
        let mut removed = 0;
        for entry in self.entries() {
            if is_temp(&entry) {
                continue;
            }
            let Some(sha256) = sha_from_path(&entry) else {
                continue;
            };
            if referenced.contains(&sha256) {
                continue;
            }
            if !is_older_than(&entry, now, ORPHAN_GRACE) {
                continue;
            }
            if fs::remove_file(&entry).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    /// Legt das Verzeichnis an, in dem dieser Blob landet.
    fn shard_dir(&self, target: &Path) -> Result<PathBuf, RecorderError> {
        let dir = target.parent().unwrap_or(&self.root).to_path_buf();
        create_dir(&dir)?;
        Ok(dir)
    }

    /// Eine Temp-Datei in einem vorhandenen Verzeichnis.
    fn temp_in(dir: &Path) -> Result<(File, PathBuf), RecorderError> {
        match tempfile::Builder::new()
            .prefix(TEMP_PREFIX)
            .permissions(fs::Permissions::from_mode(FILE_MODE))
            .tempfile_in(dir)
        {
            Ok(temp) => match temp.keep() {
                Ok((file, path)) => Ok((file, path)),
                Err(err) => Err(blob_failed(format!(
                    "could not keep the temporary blob in {} ({err})",
                    dir.display()
                ))),
            },
            Err(err) => Err(blob_failed(format!(
                "could not create a temporary blob in {} ({err})",
                dir.display()
            ))),
        }
    }

    /// Benennt die fertige Temp-Datei auf ihren Inhaltsnamen um.
    fn publish(temp: &Path, target: &Path, dir: &Path) -> Result<(), RecorderError> {
        if let Err(err) = fs::rename(temp, target) {
            let _ignored = fs::remove_file(temp);
            return Err(blob_failed(format!(
                "could not move the blob to {} ({err})",
                target.display()
            )));
        }
        sync_dir(dir);
        Ok(())
    }

    /// Alle Dateien in allen Unterverzeichnissen, ohne Rekursion in die Tiefe.
    fn entries(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(shards) = fs::read_dir(&self.root) else {
            return out;
        };
        for shard in shards.flatten() {
            if !shard.path().is_dir() {
                continue;
            }
            let Ok(files) = fs::read_dir(shard.path()) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if path.is_file() {
                    out.push(path);
                }
            }
        }
        out
    }
}

/// Legt ein Verzeichnis mit [`DIR_MODE`] an, falls es fehlt.
fn create_dir(dir: &Path) -> Result<(), RecorderError> {
    if let Err(err) = fs::create_dir_all(dir) {
        return Err(blob_failed_at(
            dir,
            format!(
                "could not create the blob directory {} ({err})",
                dir.display()
            ),
        ));
    }
    if let Err(err) = fs::set_permissions(dir, fs::Permissions::from_mode(DIR_MODE)) {
        return Err(blob_failed_at(
            dir,
            format!(
                "could not set the mode of {} to {DIR_MODE:o} ({err})",
                dir.display()
            ),
        ));
    }
    Ok(())
}

/// Schreibt das Verzeichnis auf die Platte, damit das `rename` überlebt.
///
/// Scheitert das, ist der Blob trotzdem da; nur seine Haltbarkeit über einen
/// Stromausfall hinweg ist nicht mehr zugesichert. Das ist kein Grund, die
/// Aufzeichnung abzubrechen.
fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ignored = handle.sync_all();
    }
}

/// Wahr für eine noch nicht fertige Datei.
fn is_temp(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(TEMP_PREFIX))
}

/// Der Hash aus dem Dateinamen, falls er einer ist.
fn sha_from_path(path: &Path) -> Option<[u8; 32]> {
    let name = path.file_name()?.to_str()?;
    if name.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    let bytes = name.as_bytes();
    for (index, slot) in out.iter_mut().enumerate() {
        let high = hex_value(*bytes.get(index * 2)?)?;
        let low = hex_value(*bytes.get(index * 2 + 1)?)?;
        *slot = (high << 4) | low;
    }
    Some(out)
}

/// Der Wert einer Hex-Ziffer in Kleinbuchstaben.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Wahr, wenn die Datei älter ist als die Frist.
///
/// Ohne lesbare Änderungszeit gilt sie als jung: lieber eine Datei zu viel
/// behalten als eine zu früh löschen.
fn is_older_than(path: &Path, now: SystemTime, grace: Duration) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    now.duration_since(modified).is_ok_and(|age| age >= grace)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::HashSet;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::time::{Duration, SystemTime};

    use humanitl_core::http::sha256;

    use super::{BlobStore, sha_from_path};

    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
        let store =
            BlobStore::open(&dir.path().join("blobs")).unwrap_or_else(|err| panic!("{err}"));
        (dir, store)
    }

    #[test]
    fn a_blob_lands_under_its_first_two_hex_digits_with_mode_0600() {
        let (_dir, store) = store();
        let data = b"hello".as_slice();
        let sha = sha256(data);
        store.put(&sha, data).unwrap_or_else(|err| panic!("{err}"));

        let path = store.path(&sha);
        let hex = super::hex_encode(&sha);
        assert!(path.ends_with(format!("{}/{hex}", &hex[..2])));
        assert_eq!(store.read(&sha).unwrap_or_default().as_ref(), data);

        let mode = fs::metadata(&path)
            .unwrap_or_else(|err| panic!("{err}"))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = fs::metadata(path.parent().unwrap_or_else(|| panic!("no parent")))
            .unwrap_or_else(|err| panic!("{err}"))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }

    #[test]
    fn the_same_content_is_written_once() {
        let (_dir, store) = store();
        let data = b"twice".as_slice();
        let sha = sha256(data);
        store.put(&sha, data).unwrap_or_else(|err| panic!("{err}"));
        let first = fs::metadata(store.path(&sha))
            .unwrap_or_else(|err| panic!("{err}"))
            .modified()
            .unwrap_or_else(|err| panic!("{err}"));
        store.put(&sha, data).unwrap_or_else(|err| panic!("{err}"));
        let second = fs::metadata(store.path(&sha))
            .unwrap_or_else(|err| panic!("{err}"))
            .modified()
            .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(first, second);
    }

    #[test]
    fn sweep_removes_temporary_files_but_keeps_finished_ones() {
        let (_dir, store) = store();
        let data = b"keep".as_slice();
        let sha = sha256(data);
        store.put(&sha, data).unwrap_or_else(|err| panic!("{err}"));
        let (file, temp) = store.temp(&sha).unwrap_or_else(|err| panic!("{err}"));
        drop(file);
        assert!(temp.is_file());

        assert_eq!(store.sweep_temp(), 1);
        assert!(!temp.is_file());
        assert!(store.contains(&sha));
    }

    #[test]
    fn orphans_are_removed_only_after_the_grace_period() {
        let (_dir, store) = store();
        let data = b"orphan".as_slice();
        let sha = sha256(data);
        store.put(&sha, data).unwrap_or_else(|err| panic!("{err}"));

        let now = SystemTime::now();
        assert_eq!(store.sweep_orphans(&HashSet::new(), now), 0);
        assert!(store.contains(&sha));

        let later = now + Duration::from_secs(48 * 60 * 60);
        let mut referenced = HashSet::new();
        referenced.insert(sha);
        assert_eq!(store.sweep_orphans(&referenced, later), 0);
        assert!(store.contains(&sha));

        assert_eq!(store.sweep_orphans(&HashSet::new(), later), 1);
        assert!(!store.contains(&sha));
    }

    #[test]
    fn only_a_full_hex_name_is_read_back_as_a_hash() {
        assert!(sha_from_path(std::path::Path::new("/x/ab")).is_none());
        let hex = "a".repeat(64);
        assert!(sha_from_path(std::path::Path::new(&format!("/x/{hex}"))).is_some());
        let odd = format!("{}z", "a".repeat(63));
        assert!(sha_from_path(std::path::Path::new(&format!("/x/{odd}"))).is_none());
    }
}
