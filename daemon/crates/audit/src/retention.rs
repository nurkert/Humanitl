//! Die Aufbewahrung der Kette: `audit.retention_days` (HUM-157).
//!
//! Aus einer Hash-Kette zu löschen bricht sie: Der erste Record, der bleibt,
//! nennt in `prev` den Hash eines Records, den es nicht mehr gibt. Eine
//! Löschung ist deshalb nur ehrlich, wenn die Kette sie selbst dokumentiert.
//! Das tut der Record `audit.pruned`: Er nennt Nummer und Hash des letzten
//! gelöschten Records, und [`crate::verify`] erkennt einen Anfang, dessen
//! `prev` genau darauf zeigt, als erlaubten Neuanfang. Ein Anfang ohne einen
//! solchen Record bleibt ein Bruch (`SeqGap`), wie vor HUM-157.
//!
//! **Ein Lauf, in dieser Reihenfolge**, im Schreib-Thread und damit ohne
//! parallelen Schreiber:
//!
//! 1. Alles Geschickte auf die Platte, dann die ganze Kette prüfen, mit
//!    Schlüssel und Ankern. Eine Kette, die nicht hält, wird nicht gekürzt:
//!    Die Löschung würde den Beleg der Manipulation gleich mitlöschen und eine
//!    rote Kette grün machen. Der Lauf endet dann mit `AUDIT_001`.
//! 2. Von vorn die Records suchen, deren `ts` vor der Grenze liegt, bis zum
//!    ersten, der es nicht tut (`find_cut`).
//! 3. `audit.pruned` anhängen und gleich dahinter einen Anker, in der Datei
//!    und in `audit_anchors`, und beides auf die Platte. Der dokumentierende
//!    Record steht damit nie im unverankerten Ende, das jemand unbemerkt
//!    abschneiden könnte.
//! 4. Den Rest der Datei in eine neue Datei daneben kopieren, sie sperren,
//!    auf die Platte bringen und über `audit.jsonl` umbenennen (`rewrite`).
//!    Der Schreiber hängt danach an die neue Datei an.
//!
//! Ein Absturz zwischen 3 und 4 lässt eine vollständige Kette mit einem
//! `audit.pruned` zurück, dessen Schnitt nicht stattfand. Das ist kein Bruch;
//! der nächste Lauf schneidet erneut.
//!
//! **Die Anker bleiben.** `audit_anchors` wird nicht gekürzt: Die Anker sind
//! der Beleg gegen das Kürzen der Datei, und wer sie mitlöscht, löscht den
//! Beleg. Anker unter dem Schnitt prüft `verify` nicht mehr, weil ihre Records
//! fehlen; ein Anker genau auf dem letzten gelöschten Record muss dessen Hash
//! nennen, und das prüft `verify` weiter.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead as _, BufReader, Read, Write as _};
use std::os::unix::fs::{FileExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::record::{AuditRecord, format_ts};
use crate::writer::LOG_MODE;

/// Der Name der Art, wie er in `kind` steht.
pub const PRUNED_KIND: &str = "audit.pruned";

/// `audit.pruned`: Ein Lauf von `audit.retention_days` hat den Anfang der
/// Kette gelöscht.
///
/// Der Record steht **hinter** dem Schnitt, am Ende der Kette, und ist wie
/// jeder andere gehasht und mit dem Schlüssel versiegelt; ohne den Schlüssel
/// lässt er sich nicht fälschen. Was in den gelöschten Records stand, nennt er
/// nicht: Hosts und Sitzungen der alten Records sind eben die Daten, die der
/// Lauf loswerden sollte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditPruned {
    /// Die Nummer des letzten gelöschten Records.
    pub through_seq: u64,
    /// Sein Hash: der `prev` des ersten Records, der bleibt.
    pub through_hash: String,
    /// Wie viele Records der Lauf gelöscht hat.
    pub records: u64,
    /// Die Grenze: Gelöscht ist, was davor geschrieben wurde, nach
    /// [`format_ts`].
    pub cutoff: String,
}

impl AuditPruned {
    /// `data` dieses Records.
    #[must_use]
    pub fn data(&self) -> Value {
        json!({
            "through_seq": self.through_seq,
            "through_hash": self.through_hash,
            "records": self.records,
            "cutoff": self.cutoff,
        })
    }

    /// Ob `record` ein `audit.pruned` ist, der genau diesen Schnitt nennt:
    /// letzter gelöschter Record `through_seq` mit dem Hash `through_hash`.
    #[must_use]
    pub fn documents(record: &AuditRecord, through_seq: u64, through_hash: &str) -> bool {
        record.body.kind == PRUNED_KIND
            && record.body.data.get("through_seq").and_then(Value::as_u64) == Some(through_seq)
            && record.body.data.get("through_hash").and_then(Value::as_str) == Some(through_hash)
    }
}

/// Was ein Lauf gelöscht hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneReport {
    /// Die Nummer des letzten gelöschten Records.
    pub through_seq: u64,
    /// Wie viele Records gelöscht wurden.
    pub records: u64,
    /// Wie viele Bytes die Datei danach kürzer ist, ohne den Record des Laufs
    /// und seinen Anker.
    pub bytes: u64,
}

/// Wo geschnitten wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cut {
    /// Der letzte Record, der geht.
    pub through_seq: u64,
    /// Sein Hash.
    pub through_hash: String,
    /// Wie viele Records gehen.
    pub records: u64,
    /// Die Stelle in der Datei, an der der erste bleibende Record beginnt.
    pub offset: u64,
}

impl Cut {
    /// Der Record, der diesen Schnitt dokumentiert.
    pub(crate) fn record(&self, cutoff: DateTime<Utc>) -> AuditPruned {
        AuditPruned {
            through_seq: self.through_seq,
            through_hash: self.through_hash.clone(),
            records: self.records,
            cutoff: format_ts(cutoff),
        }
    }
}

/// Liest `file` von vorn bis `len` und findet den Schnitt: alle Records vor
/// dem ersten, dessen `ts` nicht vor `cutoff` liegt. `None`, wenn schon der
/// erste Record jung genug ist.
///
/// Die Suche endet am ersten Record, der jung genug ist, auch wenn später ein
/// älterer Zeitstempel folgt (eine zurückgestellte Uhr): Geschnitten wird nur
/// ein zusammenhängender Anfang. Eine Zeile, die sich nicht lesen lässt, endet
/// die Suche ebenso; die Kette ist vorher geprüft, und was dort nicht passt,
/// wird nicht gelöscht.
///
/// # Errors
///
/// Der Lesefehler.
pub(crate) fn find_cut(file: &File, len: u64, cutoff: DateTime<Utc>) -> io::Result<Option<Cut>> {
    let mut reader = BufReader::new(Positional {
        file,
        at: 0,
        end: len,
    });
    let mut cut: Option<Cut> = None;
    let mut offset = 0_u64;
    let mut line = Vec::with_capacity(1024);
    loop {
        line.clear();
        let read = reader.read_until(b'\n', &mut line)?;
        let Some(text) = line.strip_suffix(b"\n") else {
            break;
        };
        let Ok(record) = AuditRecord::from_line(text) else {
            break;
        };
        let old = DateTime::parse_from_rfc3339(&record.body.ts)
            .is_ok_and(|ts| ts.with_timezone(&Utc) < cutoff);
        if !old {
            break;
        }
        offset += u64::try_from(read).unwrap_or(u64::MAX);
        cut = Some(Cut {
            through_seq: record.body.seq,
            through_hash: record.hash,
            records: cut.as_ref().map_or(0, |cut| cut.records) + 1,
            offset,
        });
    }
    Ok(cut)
}

/// Die neue Datei nach dem Umbenennen.
pub(crate) struct Rewritten {
    /// Die Datei, die jetzt `audit.jsonl` heißt, offen zum Anhängen und
    /// gesperrt.
    pub file: File,
    /// Der Fehler beim Synchronisieren des Verzeichnisses, falls einer kam.
    /// Die Datei ist dann schon ersetzt; offen ist nur, ob der neue Name einen
    /// Absturz übersteht.
    pub dir_sync: Option<io::Error>,
}

/// Schreibt die Bytes `from..len` von `file` in eine neue Datei neben `path`,
/// sperrt sie, bringt sie auf die Platte und benennt sie in `path` um.
///
/// Die Sperre steht, **bevor** die neue Datei den Namen bekommt: Ein zweiter
/// Daemon, der `audit.jsonl` in diesem Augenblick öffnet, findet sie belegt
/// (`AUDIT_004`), und die alte Datei hält ihre Sperre, bis der Aufrufer sie
/// schließt.
///
/// **Das Umbenennen ist der Punkt, ab dem es gilt.** Danach kommt immer die
/// neue Datei zurück, auch wenn das Synchronisieren des Verzeichnisses
/// scheitert ([`Rewritten::dir_sync`]): Die alte Datei hat dann keinen Namen
/// mehr, und wer an sie anhängte, schriebe ins Leere.
///
/// # Errors
///
/// Der Fehler beim Anlegen, Kopieren, Synchronisieren oder Umbenennen, also
/// nur vor dem Umbenennen. Die neue Datei ist dann wieder entfernt, und `path`
/// ist unverändert.
pub(crate) fn rewrite(file: &File, path: &Path, from: u64, len: u64) -> io::Result<Rewritten> {
    rewrite_with(file, path, from, len, sync_dir)
}

/// Synchronisiert das Verzeichnis, in dem `path` liegt.
///
/// # Errors
///
/// Der Fehler beim Öffnen oder Synchronisieren des Verzeichnisses.
pub(crate) fn sync_dir(path: &Path) -> io::Result<()> {
    match path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        Some(dir) => File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}

/// [`rewrite`] mit einem eigenen Schritt für das Verzeichnis, damit ein Test
/// dessen Scheitern hervorrufen kann.
fn rewrite_with(
    file: &File,
    path: &Path,
    from: u64,
    len: u64,
    sync_dir: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<Rewritten> {
    let fresh = sibling(path);
    let result = (|| -> io::Result<File> {
        // Ein Rest eines Laufs, der zwischen Anlegen und Umbenennen abbrach.
        // Der Schreiber hält die Sperre auf das Log, also schreibt niemand
        // sonst an diesem Namen.
        match fs::remove_file(&fresh) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        let mut out = OpenOptions::new()
            .read(true)
            .append(true)
            .create_new(true)
            .mode(LOG_MODE)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&fresh)?;
        rustix::fs::flock(&out, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        io::copy(
            &mut Positional {
                file,
                at: from,
                end: len,
            },
            &mut out,
        )?;
        out.flush()?;
        out.sync_all()?;
        fs::rename(&fresh, path)?;
        Ok(out)
    })();
    match result {
        Ok(file) => Ok(Rewritten {
            file,
            dir_sync: sync_dir(path).err(),
        }),
        Err(err) => {
            // Vor dem Umbenennen gescheitert: Die Nebendatei geht wieder.
            let _ = fs::remove_file(&fresh);
            Err(err)
        }
    }
}

/// Der Name der neuen Datei: immer `audit.jsonl.prune`. Ein fester Name, damit
/// der nächste Lauf den Rest eines abgebrochenen findet und entfernt, statt
/// dass sich Dateien mit alten Records ansammeln.
fn sibling(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".prune");
    PathBuf::from(name)
}

/// Ein Leser über einen Ausschnitt einer Datei, der ihre Position nicht
/// bewegt: Die Datei des Schreibers ist mit `O_APPEND` offen, und eine
/// geteilte Position wäre eine Falle für den nächsten, der liest.
struct Positional<'a> {
    file: &'a File,
    at: u64,
    end: u64,
}

impl Read for Positional<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let left = usize::try_from(self.end.saturating_sub(self.at)).unwrap_or(usize::MAX);
        let size = buffer.len().min(left);
        if size == 0 {
            return Ok(0);
        }
        let read = self.file.read_at(&mut buffer[..size], self.at)?;
        self.at += u64::try_from(read).unwrap_or(u64::MAX);
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::fs::{self, OpenOptions};
    use std::io::{self, Write as _};
    use std::os::unix::fs::MetadataExt as _;

    use super::{rewrite_with, sibling};

    /// Scheitert das Synchronisieren des Verzeichnisses nach dem Umbenennen,
    /// ist die Datei trotzdem ersetzt, und zurück kommt die neue, nicht ein
    /// Fehler, nach dem der Schreiber an die alte, namenlose Datei anhinge.
    #[test]
    fn a_failed_directory_sync_after_the_rename_still_hands_over_the_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        fs::write(&path, b"old line\nkept line\n").unwrap();
        let old = OpenOptions::new()
            .read(true)
            .append(true)
            .open(&path)
            .unwrap();

        let rewritten = rewrite_with(&old, &path, 9, 19, |_| {
            Err(io::Error::other("directory sync refused"))
        })
        .expect("after the rename there is no error to return");

        assert!(rewritten.dir_sync.is_some(), "the failed sync is reported");
        assert_eq!(fs::read(&path).unwrap(), b"kept line\n");
        assert_eq!(
            rewritten.file.metadata().unwrap().ino(),
            fs::metadata(&path).unwrap().ino(),
            "the handed-over file is the one that carries the name"
        );
        let mut file = rewritten.file;
        file.write_all(b"appended\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"kept line\nappended\n");
    }

    /// Der Rest eines Laufs, der vor dem Umbenennen abbrach, blockiert den
    /// nächsten nicht und bleibt nicht liegen.
    #[test]
    fn a_leftover_of_an_aborted_run_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        fs::write(&path, b"old line\nkept line\n").unwrap();
        fs::write(sibling(&path), b"stale records of an aborted run\n").unwrap();
        let old = OpenOptions::new()
            .read(true)
            .append(true)
            .open(&path)
            .unwrap();

        let rewritten = rewrite_with(&old, &path, 9, 19, |_| Ok(())).unwrap();

        assert!(rewritten.dir_sync.is_none());
        assert_eq!(fs::read(&path).unwrap(), b"kept line\n");
        assert!(
            !sibling(&path).exists(),
            "no leftover stays next to the log"
        );
    }
}
