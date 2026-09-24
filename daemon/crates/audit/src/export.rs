//! Der Export der Kette als JSONL oder CSV (HUM-050, HUM-070, HUM-156).
//!
//! Eine Stelle für beide Wege: Der Daemon schreibt den Export für die
//! Oberfläche und die Kommandozeile (`Audit(Export)`), und die Kommandozeile
//! schreibt ihn selbst, wenn kein Daemon antwortet. Zwei Fassungen desselben
//! Exports wären zwei Wahrheiten über dieselbe Datei.
//!
//! **JSONL** ist die Kette: jede Zeile im Zeitraum Byte für Byte so, wie sie im
//! Log steht, samt `\n`. Der Export bleibt damit gegen die Datei nachrechenbar.
//!
//! **CSV** ist die Übersicht aus HUM-050 mit zwölf Spalten ([`CSV_COLUMNS`]).
//! Was ein Record in `data` nicht trägt, bleibt leer; `data` als Ganzes steht
//! nicht darin, dafür gibt es JSONL. Die letzte Spalte ist der Hash, über den
//! jede Zeile der Übersicht in der Kette wiederzufinden ist.
//!
//! **Nie überschreiben.** Ein Export ist ein Beleg. Geschrieben wird in eine
//! Nebendatei im selben Verzeichnis, mit `fsync`, und erst der fertige Export
//! bekommt seinen Namen — über `hard_link`, das wie `create_new` an einem
//! vorhandenen Pfad scheitert, auch an einem Verweis. Ein Log mit einer Zeile,
//! die kein Record ist ([`AUDIT_001`]), hinterlässt so keine halbe Datei, die
//! den nächsten Versuch mit [`AUDIT_008`] abwiese; die Nebendatei verschwindet
//! auf jedem Weg. Die Kette selbst prüft der Export nicht, das tut `verify`
//! (HUM-214).

use std::fs::File;
use std::io::{self, BufRead as _, BufReader, BufWriter, Write as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_core::diagnostics::codes::{AUDIT_001, AUDIT_006, AUDIT_008};
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::Value;

use crate::query::TimeRange;
use crate::record::AuditRecord;

/// Die Spalten des CSV-Exports, in dieser Reihenfolge (HUM-050).
pub const CSV_COLUMNS: [&str; 12] = [
    "seq", "ts", "session", "kind", "flow", "host", "method", "decision", "rule", "status", "size",
    "hash",
];

/// Wie alt eine Nebendatei mindestens ist, bevor ein anderer Export sie für
/// verwaist halten darf.
///
/// Zwischen dem Anlegen einer Nebendatei und ihrer Sperre liegt ein
/// Augenblick; eine Datei aus diesem Augenblick ist jung und bleibt.
const STALE_AFTER: Duration = Duration::from_secs(60);

/// Welches der beiden Dokumente ein Export schreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Jede Zeile der Kette, wörtlich.
    Jsonl,
    /// Eine Kopfzeile und eine Zeile je Record, zwölf Spalten.
    Csv,
}

impl ExportFormat {
    /// Liest `jsonl` oder `csv`; alles andere ist `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "jsonl" => Some(Self::Jsonl),
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }

    /// Der Name, den Vertrag und Kommandozeile tragen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jsonl => "jsonl",
            Self::Csv => "csv",
        }
    }
}

/// Exportiert die Records aus `log`, deren Zeitpunkt in `range` fällt, nach
/// `out`, und zählt sie.
///
/// `until` ist das Ende, das der Schreiber zuletzt gemeldet hat: Gelesen wird
/// bis zum Record mit dieser Nummer und nicht weiter, denn was dahinter steht,
/// entsteht gerade und ist kein Bruch (HUM-156). `None` liest bis zum Ende der
/// Datei.
///
/// `out` darf es noch nicht geben; der fertige Export bekommt seinen Namen
/// erst, wenn er vollständig auf der Platte steht.
///
/// # Errors
///
/// [`AUDIT_008`], wenn `out` schon da ist oder sich nicht schreiben lässt,
/// [`AUDIT_006`], wenn sich das Log nicht lesen lässt, [`AUDIT_001`], wenn
/// eine Zeile des Logs kein Record ist, auch eine leere. Der Export prüft
/// weder Hash noch MAC noch Anker; ob die Kette hält, sagt `verify`. Eine
/// manipulierte Zeile im Zeitraum geht deshalb unverändert in einen
/// JSONL-Export (HUM-214): So bleibt er Byte für Byte die Kette, und auch ein
/// gebrochenes Log lässt sich als Beleg übergeben. Über einen JSONL-Export des
/// ganzen Logs findet `verify` den Bruch an derselben Stelle wie im Log; ein
/// Ausschnitt nach Zeitraum oder `until` ist keine vollständige Kette mehr,
/// und das CSV schreibt die Felder neu und trägt weder `prev` noch MAC.
pub fn export(
    log: &Path,
    format: ExportFormat,
    range: &TimeRange,
    out: &Path,
    until: Option<u64>,
) -> Result<u64, Diagnostic> {
    refuse_existing(out)?;
    let file = File::open(log).map_err(|error| {
        Diagnostic::builder(AUDIT_006, Severity::Error)
            .why(format!(
                "cannot read {} to export it: {error}",
                log.display()
            ))
            .fix(FixAction::CopyCommand(format!(
                "ls -ln {}",
                shell_path(log)
            )))
            .build()
    })?;

    let staged = Staged::create(out)?;
    let result = write_export(file, log, format, range, out, until, &staged)
        .and_then(|count| publish(&staged.path, out).map(|()| count));
    // Die Nebendatei geht in jedem Fall; nach dem Verweis trägt `out` den Inhalt.
    drop(staged);
    result
}

/// Schreibt die Zeilen in die Nebendatei und zählt sie.
fn write_export(
    file: File,
    log: &Path,
    format: ExportFormat,
    range: &TimeRange,
    out: &Path,
    until: Option<u64>,
    staged: &Staged,
) -> Result<u64, Diagnostic> {
    let mut sink = BufWriter::new(&staged.file);
    let write_failed = |error: &io::Error| unwritable(out, &error.to_string());
    if format == ExportFormat::Csv {
        // RFC 4180 schließt jede Zeile mit CRLF.
        sink.write_all(CSV_COLUMNS.join(",").as_bytes())
            .and_then(|()| sink.write_all(b"\r\n"))
            .map_err(|error| write_failed(&error))?;
    }

    let mut reader = BufReader::new(file);
    let mut buffer = Vec::with_capacity(1024);
    let mut count = 0_u64;
    let mut number = 0_usize;
    // Die Nummer des zuletzt gelesenen Records.
    let mut number_seq = 0_u64;
    loop {
        if until.is_some_and(|until| number_seq >= until) {
            // Das gemeldete Ende ist erreicht; der Rest entsteht gerade.
            break;
        }
        buffer.clear();
        let read = reader.read_until(b'\n', &mut buffer).map_err(|error| {
            Diagnostic::builder(AUDIT_006, Severity::Error)
                .why(format!(
                    "{} stops after line {number}: {error}",
                    log.display()
                ))
                .build()
        })?;
        if read == 0 {
            break;
        }
        number += 1;
        // Auch eine leere Zeile ist kein Record: `verify` wertet sie als
        // Bruch, und ein Export, der sie wegließe, wäre nicht mehr Byte für
        // Byte die Kette (HUM-214).
        let line = buffer.strip_suffix(b"\n").unwrap_or(&buffer);
        let record = AuditRecord::from_line(line)
            .map_err(|error| not_a_record(log, number, &error.to_string()))?;
        number_seq = record.body.seq;
        if !range.contains(&record.body.ts) {
            continue;
        }
        let written = match format {
            // Die Zeile selbst, nicht eine neu geschriebene: Nur so ist der
            // Export Byte für Byte die Kette.
            ExportFormat::Jsonl => sink.write_all(line).and_then(|()| sink.write_all(b"\n")),
            ExportFormat::Csv => sink
                .write_all(csv_row(&record).as_bytes())
                .and_then(|()| sink.write_all(b"\r\n")),
        };
        written.map_err(|error| write_failed(&error))?;
        count += 1;
    }
    sink.flush().map_err(|error| write_failed(&error))?;
    drop(sink);
    staged
        .file
        .sync_all()
        .map_err(|error| write_failed(&error))?;
    Ok(count)
}

/// Eine Zeile des CSV: die zwölf Spalten aus [`CSV_COLUMNS`].
///
/// Die acht Spalten zwischen `kind` und `hash` kommen aus `data`, und nur
/// dann, wenn der Record das Feld trägt. Ein Text steht als er selbst da, eine
/// Zahl in Ziffern, `null` als leeres Feld, alles andere als JSON.
#[must_use]
pub fn csv_row(record: &AuditRecord) -> String {
    let data = |name: &str| match record.body.data.get(name) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    };
    [
        record.body.seq.to_string(),
        record.body.ts.clone(),
        record.body.session.clone(),
        record.body.kind.clone(),
        data("flow"),
        data("host"),
        data("method"),
        data("decision"),
        data("rule"),
        data("status"),
        data("size"),
        record.hash.clone(),
    ]
    .iter()
    .map(|field| csv_field(field))
    .collect::<Vec<_>>()
    .join(",")
}

/// Ein Feld nach RFC 4180: in Anführungszeichen, sobald es welche braucht.
#[must_use]
pub fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// Gibt dem fertigen Export seinen Namen, ohne je etwas zu überschreiben.
///
/// Erst `hard_link`: Es scheitert an einem vorhandenen Pfad, auch an einem
/// Verweis. Dateisysteme ohne harte Verweise (vfat, exFAT) antworten mit einem
/// anderen Fehler; dann `renameat2` mit `RENAME_NOREPLACE`, das dieselbe
/// Zusage macht. Ein vorhandenes Ziel bleibt in beiden Fällen [`AUDIT_008`].
fn publish(staged: &Path, out: &Path) -> Result<(), Diagnostic> {
    match std::fs::hard_link(staged, out) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(unwritable(out, &error.to_string()))
        }
        Err(_) => rustix::fs::renameat_with(
            rustix::fs::CWD,
            staged,
            rustix::fs::CWD,
            out,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|error| unwritable(out, &error.to_string())),
    }
}

/// Die Nebendatei eines Exports; sie verschwindet, wenn der Wert fällt.
struct Staged {
    /// Wo sie liegt.
    path: PathBuf,
    /// Offen zum Schreiben.
    file: File,
}

impl Staged {
    /// Legt `.<name>.tmp-<zufall>` neben `out` an, ohne etwas zu
    /// überschreiben; der Rest des Namens sind zwölf zufällige Zeichen.
    fn create(out: &Path) -> Result<Self, Diagnostic> {
        let dir = out
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        // Angelegt wird kein Verzeichnis: Ein Export geht dorthin, wo es den
        // Ort schon gibt, und eine Kette neuer Verzeichnisse hinter einem
        // Verweis wäre ein Weg an einen Ort, den niemand genannt hat.
        if !dir.is_dir() {
            return Err(unwritable(
                out,
                &format!(
                    "the directory {} does not exist; the export creates no directories",
                    dir.display()
                ),
            ));
        }
        let name = out.file_name().map_or_else(
            || "export".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        // Ein zufälliger Rest im Namen und nicht die Prozessnummer: Eine
        // Nebendatei mit derselben Nummer aus einem anderen PID-Namensraum
        // hielte sonst jeden Export mit „File exists" auf.
        let (file, path) = tempfile::Builder::new()
            .prefix(&format!(".{name}.tmp-"))
            .rand_bytes(12)
            .tempfile_in(&dir)
            .and_then(|named| named.keep().map_err(|error| error.error))
            .map_err(|error| unwritable(out, &error.to_string()))?;
        // Die Sperre hält, solange die Datei offen ist, also bis der Export
        // seinen Namen hat oder aufgegeben ist; ein anderer Export sieht an
        // ihr, dass diese Nebendatei nicht verwaist ist.
        let staged = Self { path, file };
        rustix::fs::flock(&staged.file, rustix::fs::FlockOperation::LockExclusive)
            .map_err(|error| unwritable(out, &error.to_string()))?;
        if let Ok(meta) = staged.file.metadata() {
            sweep_stale(&dir, &name, &staged.path, meta.uid());
        }
        Ok(staged)
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        // Best effort: Ein Rest bliebe nur, wenn das Verzeichnis inzwischen
        // nicht mehr schreibbar ist, und dann sagt der Befund ohnehin mehr.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Räumt Nebendateien früherer Exporte weg, die niemand mehr schreibt.
///
/// Ein Export, der mit `SIGTERM` oder `SIGINT` endet, kommt nicht mehr zu
/// seinem `Drop`, und seine Nebendatei trüge Audit-Daten, die niemand
/// bestellt hat. Der nächste Export in dasselbe Ziel nimmt sie mit.
///
/// Ob noch jemand schreibt, sagt die Sperre und nicht `/proc`: Ein Export in
/// einem anderen PID-Namensraum oder unter `hidepid` ist dort unsichtbar. Jeder
/// Export hält auf seiner Nebendatei ein exklusives `flock`, solange sie offen
/// ist; nur eine Datei, deren Sperre sich ohne Warten nehmen lässt, ist
/// verwaist. Dazu kommen drei Bedingungen: eine reguläre Datei und kein
/// Verweis, dasselbe Konto wie dieser Export, und älter als [`STALE_AFTER`].
///
/// Gefegt wird nur in einem Verzeichnis, das diesem Konto gehört und in das
/// weder Gruppe noch andere schreiben dürfen. Gelöscht wird am Ende ein Name;
/// in einem Verzeichnis, in das ein anderer schreiben darf, kann er den
/// Eintrag nach der letzten Prüfung austauschen, und keine Prüfung davor
/// schützt dann die Datei, die der Name inzwischen meint. Dort bleiben
/// verwaiste Nebendateien liegen.
fn sweep_stale(dir: &Path, name: &str, own: &Path, uid: u32) {
    let private =
        std::fs::metadata(dir).is_ok_and(|meta| meta.uid() == uid && meta.mode() & 0o022 == 0);
    if !private {
        return;
    }
    let prefix = format!(".{name}.tmp-");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let ours = file_name
            .to_str()
            .is_some_and(|text| text.len() > prefix.len() && text.starts_with(&prefix));
        if !ours || path == own {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let old = meta
            .modified()
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age >= STALE_AFTER);
        if !meta.file_type().is_file() || meta.uid() != uid || !old {
            continue;
        }
        // Geöffnet wird ohne einem Verweis zu folgen und ohne zu warten; die
        // Sperre gilt dem geöffneten Inode, gelöscht wird aber ein Name. Nur
        // wenn unter dem Namen nach der Sperre noch derselbe Inode liegt, ist
        // es die Datei, deren Sperre genommen wurde, und nur wenn es derselbe
        // Inode ist, den die Prüfung von Art, Konto und Alter oben gesehen
        // hat, gelten diese Prüfungen für sie.
        let Ok(fd) = rustix::fs::open(
            &path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        ) else {
            continue;
        };
        if rustix::fs::flock(&fd, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_err() {
            continue;
        }
        let (Ok(locked), Ok(named)) = (rustix::fs::fstat(&fd), std::fs::symlink_metadata(&path))
        else {
            continue;
        };
        let checked = locked.st_dev == meta.dev() && locked.st_ino == meta.ino();
        if checked && locked.st_dev == named.dev() && locked.st_ino == named.ino() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Eine Datei, die schon da ist, wird nicht überschrieben.
///
/// Ein Export ist ein Beleg. Wer ihn zweimal in denselben Pfad schreibt, will
/// fast nie den ersten verlieren, und ein `--force` gibt es aus demselben
/// Grund nicht wie bei `daemon install`.
fn refuse_existing(out: &Path) -> Result<(), Diagnostic> {
    // `symlink_metadata` und nicht `exists`: Ein Verweis ins Leere ist ein
    // Pfad, der schon da ist, und bekommt denselben Befund samt Vorschlag.
    if std::fs::symlink_metadata(out).is_err() {
        return Ok(());
    }
    let quoted = shell_path(out);
    Err(Diagnostic::builder(AUDIT_008, Severity::Error)
        .why(format!(
            "{} is already there; the export writes no file over a file that exists",
            out.display()
        ))
        // `-n` überschreibt kein vorhandenes Ziel, `--` lässt einen Namen mit
        // `-` am Anfang einen Namen sein, und der Zeitstempel macht das Ziel
        // eindeutig: Ein älterer Beleg wird so nie überschrieben.
        .fix(FixAction::CopyCommand(format!(
            "mv -n -- {quoted} {quoted}.$(date -u +%Y%m%dT%H%M%SZ)"
        )))
        .build())
}

/// [`AUDIT_008`]: Der Export ließ sich nicht schreiben.
fn unwritable(out: &Path, why: &str) -> Diagnostic {
    let dir = out
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Diagnostic::builder(AUDIT_008, Severity::Error)
        .why(format!("{} could not be written: {why}", out.display()))
        .fix(FixAction::CopyCommand(format!(
            "ls -ld {}",
            shell_path(dir)
        )))
        .build()
}

/// [`AUDIT_001`]: Im Log steht eine Zeile, die kein Record ist.
fn not_a_record(log: &Path, line: usize, why: &str) -> Diagnostic {
    Diagnostic::builder(AUDIT_001, Severity::Error)
        .why(format!(
            "{} line {line} is not an audit record ({why}); the export stops and writes \
             nothing",
            log.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl audit verify --file {}",
            shell_path(log)
        )))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::json;

    use super::{CSV_COLUMNS, ExportFormat, csv_field, csv_row};
    use crate::record::{AuditRecord, GENESIS_PREV, RecordBody};

    fn record(kind: &str, data: serde_json::Value) -> AuditRecord {
        RecordBody {
            seq: 7,
            ts: "2026-09-02T10:42:01.000000Z".to_owned(),
            session: "-".to_owned(),
            kind: kind.to_owned(),
            data,
            prev: GENESIS_PREV.to_owned(),
        }
        .seal(&[7_u8; 32])
        .expect("the record seals")
    }

    /// Jede Spalte bekommt ihr Feld aus `data`, und ein Feld, das der Record
    /// nicht trägt, bleibt leer.
    #[test]
    fn a_csv_row_takes_each_column_from_its_own_field() {
        let received = record(
            "flow.received",
            json!({ "flow": "f-1", "host": "api.example", "method": "GET", "size": 12,
                    "path_hash": "x" }),
        );
        assert_eq!(
            csv_row(&received),
            format!(
                "7,2026-09-02T10:42:01.000000Z,-,flow.received,f-1,api.example,GET,,,,12,{}",
                received.hash
            )
        );
        let decided = record(
            "flow.decided",
            json!({ "flow": "f-1", "decision": "block", "rule": null }),
        );
        assert_eq!(
            csv_row(&decided),
            format!(
                "7,2026-09-02T10:42:01.000000Z,-,flow.decided,f-1,,,block,,,,{}",
                decided.hash
            )
        );
        let responded = record("flow.responded", json!({ "flow": "f-1", "status": 403 }));
        assert!(csv_row(&responded).contains(",403,"));
        assert_eq!(CSV_COLUMNS.len(), 12);
        assert_eq!(CSV_COLUMNS[11], "hash");
    }

    #[test]
    fn a_field_with_a_quote_is_doubled_and_wrapped() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    /// Ein Export legt kein Verzeichnis an: Fehlt das Ziel, ist das
    /// `AUDIT_008`, und danach gibt es das Verzeichnis weiterhin nicht.
    #[test]
    fn a_missing_directory_is_audit_008_and_stays_missing() {
        let dir = tempfile::tempdir().expect("a directory");
        let log = dir.path().join("audit.jsonl");
        std::fs::write(&log, b"").expect("an empty log");
        let out = dir.path().join("gone").join("deeper").join("x.jsonl");
        let refused = super::export(
            &log,
            ExportFormat::Jsonl,
            &crate::query::TimeRange::ALL,
            &out,
            None,
        )
        .expect_err("no directory, no export");
        assert_eq!(refused.code.as_str(), "AUDIT_008");
        assert!(!dir.path().join("gone").exists(), "no directory is created");
    }

    /// Der Weg des Befunds aus HUM-215: Ein Ziel in einem Verzeichnis mit
    /// zwei Leerzeichen ist schon da. Der Vorschlag muss genau diese Datei
    /// beiseiteschieben, und `bash` findet sie damit.
    #[test]
    fn the_fix_for_an_existing_target_moves_exactly_that_file() {
        let dir = tempfile::tempdir().expect("a directory");
        let spaced = dir.path().join("Audit  2026");
        std::fs::create_dir(&spaced).expect("the spaced directory");
        // Die Falle: der Name, den ein gefaltetes Leerzeichen daraus machte.
        let folded = dir.path().join("Audit 2026");
        std::fs::create_dir(&folded).expect("the folded directory");
        std::fs::write(folded.join("a.jsonl"), b"other").expect("a decoy");
        let out = spaced.join("a.jsonl");
        std::fs::write(&out, b"first").expect("the first export");

        let refused = super::refuse_existing(&out).expect_err("the target exists");
        let Some(humanitl_core::FixAction::CopyCommand(command)) = refused.fix else {
            panic!("expected a command, got {:?}", refused.fix);
        };
        assert!(command.contains(r"Audit\x20\x202026"), "{command}");
        let status = std::process::Command::new("bash")
            .arg("-c")
            .arg(&command)
            .env("LC_ALL", "C")
            .status()
            .expect("bash runs");
        assert!(status.success(), "{command}");
        assert!(!out.exists(), "{command} moved the export away");
        let moved = std::fs::read_dir(&spaced)
            .expect("the spaced directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("a.jsonl."))
            .count();
        assert_eq!(moved, 1, "{command} kept the export next to itself");
        assert!(folded.join("a.jsonl").is_file(), "the decoy is untouched");
    }

    #[test]
    fn only_the_two_formats_parse() {
        assert_eq!(ExportFormat::parse("jsonl"), Some(ExportFormat::Jsonl));
        assert_eq!(ExportFormat::parse("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::parse("CSV"), None);
        assert_eq!(ExportFormat::parse(""), None);
        assert_eq!(ExportFormat::Csv.as_str(), "csv");
    }
}
