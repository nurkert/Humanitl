//! Die Datei-Seite der Aufbewahrung (HUM-157, HUM-194; aus `humanitl-audit`
//! hierher gezogen mit HUM-155).
//!
//! Was ein Lauf tut, in welcher Reihenfolge und mit welchem Beleg, steht in
//! `humanitl_audit::retention`, ebenso der Record `audit.pruned` und die
//! Grenzen. Hier stehen die Schritte, die die Datei brauchen: den Schnitt
//! suchen (`find_cut`, `find_byte_cut`) und den Rest in eine neue Datei
//! schreiben und umbenennen (`rewrite`).

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead as _, BufReader, Read, Write as _};
use std::os::unix::fs::{FileExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use humanitl_audit::record::{AuditRecord, format_ts};
use humanitl_audit::retention::AuditPruned;

use crate::writer::LOG_MODE;

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
    /// Der Record, der diesen Schnitt dokumentiert, mit den Grenzen des
    /// Laufs.
    pub(crate) fn record(&self, cutoff: Option<DateTime<Utc>>, max_bytes: u64) -> AuditPruned {
        AuditPruned {
            through_seq: self.through_seq,
            through_hash: self.through_hash.clone(),
            records: self.records,
            cutoff: cutoff.map(format_ts),
            max_bytes: (max_bytes > 0).then_some(max_bytes),
        }
    }

    /// Der weitere von zwei Schnitten. Beide sind ein Anfang derselben Datei,
    /// also enthält der weitere den anderen.
    pub(crate) fn further(first: Option<Self>, second: Option<Self>) -> Option<Self> {
        match (first, second) {
            (Some(first), Some(second)) => Some(if second.offset > first.offset {
                second
            } else {
                first
            }),
            (first, second) => first.or(second),
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
    scan(file, len, |record, _, _| {
        DateTime::parse_from_rfc3339(&record.body.ts)
            .is_ok_and(|ts| ts.with_timezone(&Utc) < cutoff)
    })
}

/// Liest `file` von vorn bis `len` und findet den Schnitt nach Bytes: so
/// viele Records von vorn, dass der Rest samt `reserve` Bytes für den Record
/// des Laufs und seinen Anker höchstens `max_bytes` groß ist. `None`, wenn
/// schon der ganze Rest passt.
///
/// Der letzte Record vor `len` geht nie: Der neueste Record bleibt, auch wenn
/// er allein größer ist als die Grenze. Eine Zeile, die sich nicht lesen
/// lässt, endet die Suche wie in [`find_cut`].
///
/// # Errors
///
/// Der Lesefehler.
pub(crate) fn find_byte_cut(
    file: &File,
    len: u64,
    max_bytes: u64,
    reserve: u64,
) -> io::Result<Option<Cut>> {
    scan(file, len, |_, start, end| {
        end < len && (len - start).saturating_add(reserve) > max_bytes
    })
}

/// Liest `file` von vorn bis `len` und schneidet, solange `goes` für den
/// Record, seinen Anfang und sein Ende in der Datei `true` sagt. Endet am
/// ersten Record, für den es `false` sagt, und an einer Zeile, die kein Record
/// ist oder kein Zeilenende hat.
fn scan(
    file: &File,
    len: u64,
    mut goes: impl FnMut(&AuditRecord, u64, u64) -> bool,
) -> io::Result<Option<Cut>> {
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
        let end = offset.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if !goes(&record, offset, end) {
            break;
        }
        offset = end;
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
