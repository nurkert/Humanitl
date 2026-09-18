//! `humanitl audit verify` und `humanitl audit export` (HUM-070).
//!
//! Die Kette beweist eine Änderung, Löschung oder Umordnung vor dem letzten
//! Anker, solange der Angreifer den HMAC-Schlüssel nicht hat (`docs/SECURITY.md`,
//! „Was die Audit-Kette beweist"). Diese Zusage hängt an drei Dingen: der Kette
//! selbst, den MACs und den Ankern. Nur der Daemon hat alle drei — den
//! Schlüssel aus dem Schlüsselspeicher und die Anker aus der Tabelle
//! `audit_anchors`.
//!
//! Deshalb fragt jeder Aufruf zuerst den Daemon. Erst wenn der nicht antwortet
//! oder die Datei ausdrücklich genannt ist (`--file`), prüft die Kommandozeile
//! selbst — dann aber ohne Schlüssel und ohne Anker, und **die Ausgabe sagt
//! das**: `warnings: no HMAC key (file mode)`. Eine schwächere Prüfung, die
//! sich nicht als schwächer zu erkennen gibt, wiegt einen Menschen in
//! Sicherheit, und das ist schlimmer, als gar nicht zu prüfen.
//!
//! Der Export nimmt denselben Weg. `--since` und `--until` gibt es nur in der
//! Fassung über die Datei: Der Vertrag (`AuditRequest.Export`) trägt keine
//! Zeitgrenzen, und ein Bereich, den die Kommandozeile nachträglich aus einer
//! fertigen Datei schnitte, wäre eine zweite Wahrheit über denselben Export.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};

use humanitl_audit::{AuditRecord, AuditVerifier, BreakReason, VerifyStatus, VerifyWarning};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cli::AuditCmd;
use crate::cmd::{Context, EXIT_OK, EXIT_SECURITY, Failure, status_diagnostic};
use crate::render::{labeled, plain, shell_path};

/// Die Spalten des CSV-Exports, in dieser Reihenfolge.
///
/// Dieselben acht Felder, die ein Record hat, und keins mehr: Ein Export, der
/// etwas hinzufügte, wäre gegen die Datei nicht mehr nachzurechnen.
pub const CSV_COLUMNS: [&str; 8] = [
    "seq", "ts", "session", "kind", "data", "prev", "hash", "mac",
];

/// Führt `humanitl audit <cmd>` aus.
///
/// # Errors
///
/// `AUDIT_001`, wenn die Kette bricht (Exit 4), `AUDIT_006`, wenn sich das Log
/// nicht lesen lässt, `AUDIT_008`, wenn der Export nicht geschrieben werden
/// kann, und `CLI_004` bei einem Zeitpunkt, den niemand lesen kann.
pub async fn run(ctx: &Context, cmd: &AuditCmd) -> Result<u8, Failure> {
    match cmd {
        AuditCmd::Verify { file } => verify(ctx, file.as_deref()).await,
        AuditCmd::Export {
            format,
            out,
            since,
            until,
            file,
        } => {
            let range = Range::new(since.as_deref(), until.as_deref())?;
            export(ctx, format, out, &range, file.as_deref()).await
        }
    }
}

/// Woher die geprüfte Auskunft stammt.
#[derive(Debug, Clone)]
enum Source {
    /// Der Daemon hat mit Schlüssel und Ankern geprüft.
    Daemon,
    /// Diese Datei wurde ohne Schlüssel und ohne Anker geprüft.
    File(PathBuf),
}

impl Source {
    /// Das Wort für die Ausgabe.
    fn as_str(&self) -> String {
        match self {
            Self::Daemon => "daemon".to_owned(),
            Self::File(path) => format!("file:{}", path.display()),
        }
    }
}

/// Das Ergebnis einer Prüfung, unabhängig davon, wer sie gefahren hat.
#[derive(Debug)]
struct Summary {
    /// Wer geprüft hat.
    source: Source,
    /// Wie viele Records bestanden haben.
    records: u64,
    /// Der letzte Record, wenn die Kette hält.
    head: Option<Head>,
    /// Ob die Anker geprüft wurden: vom Daemon ja, in der Datei nie.
    anchors_checked: bool,
    /// Wo die Kette bricht, wenn sie bricht.
    broken: Option<Broken>,
    /// Was diese Prüfung nicht beweisen konnte.
    warnings: Vec<String>,
}

/// Der letzte Record einer haltenden Kette.
#[derive(Debug)]
struct Head {
    /// Sein Hash als Hex.
    hash: String,
    /// Seine Nummer.
    seq: u64,
    /// Sein Zeitpunkt, falls er gelesen werden konnte.
    ts: Option<String>,
}

/// Wo und warum die Kette bricht.
#[derive(Debug)]
struct Broken {
    /// Die erste Nummer, die nicht besteht.
    first_bad_seq: u64,
    /// Der Grund in `snake_case`, falls einer feststeht.
    reason: Option<String>,
    /// Der Befund, der auf `stderr` oder in das JSON gehört.
    diagnostic: Diagnostic,
}

/// `audit verify`.
async fn verify(ctx: &Context, file: Option<&Path>) -> Result<u8, Failure> {
    let summary = match file {
        Some(path) => verify_file(path, Vec::new())?,
        None => match verify_over_rpc(ctx).await {
            Ok(summary) => summary,
            Err(failure) => {
                // Der Daemon ist nicht da oder kennt den Aufruf nicht. Die
                // Datei liegt trotzdem, und eine schwächere Antwort auf die
                // Frage ist mehr wert als keine — solange sie sagt, dass sie
                // die schwächere ist. Gibt es die Datei nicht, bleibt es beim
                // Befund des Daemons: Dann ist wirklich nichts zu prüfen.
                let path = ctx.paths.audit_path();
                if !path.exists() {
                    return Err(failure);
                }
                verify_file(&path, vec![fallback_warning(&failure)])?
            }
        },
    };

    report(ctx, &summary);
    if summary.broken.is_some() {
        return Ok(EXIT_SECURITY);
    }
    Ok(EXIT_OK)
}

/// Warum die Kommandozeile selbst prüft, in einem Satz, der stimmt.
///
/// Ein Daemon, der nicht erreichbar ist, und einer, der antwortet, aber nicht
/// prüfen kann (`Audit` ist dort noch nicht gebaut), sind zwei verschiedene
/// Lagen; „did not answer" wäre für die zweite gelogen.
fn fallback_warning(failure: &Failure) -> String {
    let why = plain(&failure.diagnostic.why);
    match failure.diagnostic.code.as_str() {
        "DAEMON_001" | "DAEMON_002" | "IPC_001" => {
            format!("the daemon is not reachable ({why}), so this is the file-mode check")
        }
        _ => format!(
            "the daemon answered but did not verify ({why}), so this is the file-mode check"
        ),
    }
}

/// Die Prüfung durch den Daemon: mit Schlüssel und Ankern, also die ganze.
async fn verify_over_rpc(ctx: &Context) -> Result<Summary, Failure> {
    let mut client = ctx.connect().await?;
    let answer = client
        .audit(v1::AuditRequest {
            op: Some(v1::audit_request::Op::Verify(())),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Audit(Verify)")))?
        .into_inner();

    let hash = hex::encode(&answer.head_hash);
    let head = (!answer.head_hash.is_empty()).then_some(Head {
        hash,
        seq: answer.entries,
        ts: None,
    });
    let broken = (!answer.ok).then(|| {
        let diagnostic = answer
            .diagnostic
            .as_ref()
            .and_then(crate::cmd::from_proto)
            .unwrap_or_else(|| {
                Diagnostic::builder(codes::AUDIT_001, Severity::Error)
                    .why(format!(
                        "the daemon reports a break at seq {}",
                        answer.first_bad_seq
                    ))
                    .build()
            });
        Broken {
            first_bad_seq: answer.first_bad_seq,
            reason: None,
            diagnostic,
        }
    });
    Ok(Summary {
        source: Source::Daemon,
        records: answer.entries,
        head: if broken.is_some() { None } else { head },
        anchors_checked: true,
        broken,
        warnings: Vec::new(),
    })
}

/// Die Prüfung einer Datei: Kette und Kanonik, kein Schlüssel, keine Anker.
fn verify_file(path: &Path, mut warnings: Vec<String>) -> Result<Summary, Failure> {
    let report = AuditVerifier::verify(path, None, &[]).map_err(Failure::new)?;
    for warning in &report.warnings {
        warnings.push(match *warning {
            VerifyWarning::NoHmacKey => "no HMAC key (file mode)".to_owned(),
            VerifyWarning::UnanchoredTail { records } => {
                format!("unanchored tail: {records} records")
            }
        });
    }
    // Die Anker stehen in der Aufzeichnung des Daemons, nicht in der Datei.
    // Wer nur die Datei hat, prüft sie nicht — und erfährt es.
    warnings.push("no anchors (file mode)".to_owned());

    let broken = match report.status {
        VerifyStatus::Ok => None,
        VerifyStatus::Broken {
            first_bad_seq,
            reason,
        } => Some(Broken {
            first_bad_seq,
            reason: Some(reason_word(reason)),
            diagnostic: report.diagnostic(path).unwrap_or_else(|| {
                Diagnostic::builder(codes::AUDIT_001, Severity::Error)
                    .why(format!("{} breaks at seq {first_bad_seq}", path.display()))
                    .build()
            }),
        }),
    };
    Ok(Summary {
        source: Source::File(path.to_path_buf()),
        records: report.records,
        head: if broken.is_some() {
            None
        } else {
            head_of(path)
        },
        anchors_checked: false,
        broken,
        warnings,
    })
}

/// Der Grund eines Bruchs als ein Wort, mit der Nummer des Ankers, wo es eine gibt.
fn reason_word(reason: BreakReason) -> String {
    match reason {
        BreakReason::AnchorMismatch { anchor_seq }
        | BreakReason::TruncatedBelowAnchor { anchor_seq } => {
            format!("{} (anchor at seq {anchor_seq})", reason.as_str())
        }
        other => other.as_str().to_owned(),
    }
}

/// Der letzte Record der Datei, für Hash, Nummer und Zeitpunkt des Kopfes.
///
/// Wird nur gerufen, wenn die Prüfung gehalten hat; dann ist die letzte Zeile
/// ein Record, und ein `None` heißt: die Kette ist leer.
fn head_of(path: &Path) -> Option<Head> {
    let file = File::open(path).ok()?;
    let mut last = Vec::new();
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let trimmed = line.strip_suffix(b"\n").unwrap_or(&line);
        if !trimmed.is_empty() {
            last = trimmed.to_vec();
        }
    }
    let record = AuditRecord::from_line(&last).ok()?;
    Some(Head {
        hash: record.hash,
        seq: record.body.seq,
        ts: Some(record.body.ts),
    })
}

/// Schreibt das Ergebnis: ein JSON-Objekt oder der Block aus der Spezifikation.
fn report(ctx: &Context, summary: &Summary) {
    if ctx.render.is_json() {
        ctx.render.value(&summary_json(summary));
        return;
    }

    let mut rows: Vec<(&str, String)> = Vec::new();
    match summary.broken.as_ref() {
        None => rows.push(("audit chain", "OK".to_owned())),
        Some(broken) => {
            let reason = broken
                .reason
                .as_ref()
                .map_or_else(String::new, |reason| format!(" ({reason})"));
            rows.push((
                "audit chain",
                format!("BROKEN at seq {}{reason}", broken.first_bad_seq),
            ));
        }
    }
    rows.push(("records", summary.records.to_string()));
    if let Some(head) = summary.head.as_ref() {
        rows.push(("head", head_line(head)));
    }
    rows.push((
        "anchors",
        if summary.anchors_checked {
            "checked by the daemon".to_owned()
        } else {
            "not checked".to_owned()
        },
    ));
    if !summary.warnings.is_empty() {
        rows.push(("warnings", summary.warnings.join("; ")));
    }
    rows.push(("checked by", summary.source.as_str()));
    ctx.render.line(labeled(&rows).trim_end());

    if let Some(broken) = summary.broken.as_ref() {
        ctx.render.diagnostic(&broken.diagnostic);
    }
}

/// Die Kopfzeile: der Hash gekürzt, die Nummer und der Zeitpunkt dahinter.
fn head_line(head: &Head) -> String {
    let mut line = format!("{} (seq {}", short_hash(&head.hash), head.seq);
    if let Some(ts) = head.ts.as_ref() {
        let _ = write!(line, ", {ts}");
    }
    line.push(')');
    line
}

/// Ein Hash, wie ein Mensch ihn vergleicht: die ersten und die letzten vier
/// Zeichen.
fn short_hash(hash: &str) -> String {
    if hash.chars().count() <= 12 {
        return hash.to_owned();
    }
    let head: String = hash.chars().take(4).collect();
    let tail: String = hash
        .chars()
        .skip(hash.chars().count().saturating_sub(4))
        .collect();
    format!("{head}…{tail}")
}

/// Das Ergebnis als ein JSON-Objekt: alles, was der Block zeigt, und der
/// Befund dazu.
fn summary_json(summary: &Summary) -> Value {
    let mut value = json!({
        "chain": if summary.broken.is_some() { "broken" } else { "ok" },
        "records": summary.records,
        "mode": if summary.anchors_checked { "full" } else { "file" },
        "anchors": if summary.anchors_checked { "checked" } else { "not_checked" },
        "warnings": summary.warnings,
        "checked_by": summary.source.as_str(),
    });
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    if let Some(head) = summary.head.as_ref() {
        object.insert(
            "head".to_owned(),
            json!({ "hash": head.hash, "seq": head.seq, "ts": head.ts }),
        );
    }
    if let Some(broken) = summary.broken.as_ref() {
        object.insert("first_bad_seq".to_owned(), json!(broken.first_bad_seq));
        object.insert("reason".to_owned(), json!(broken.reason));
        object.insert(
            "diagnostic".to_owned(),
            crate::render::diagnostic_json(&broken.diagnostic),
        );
    }
    value
}

/// Die beiden Zeitgrenzen eines Exports, schon in der Form des Logs.
#[derive(Debug, Default)]
struct Range {
    /// Ab diesem Zeitpunkt, einschließlich.
    since: Option<String>,
    /// Bis zu diesem Zeitpunkt, ausschließlich.
    until: Option<String>,
}

impl Range {
    /// Liest beide Grenzen als RFC 3339.
    ///
    /// # Errors
    ///
    /// `CLI_004`, wenn ein Zeitpunkt sich nicht lesen lässt.
    fn new(since: Option<&str>, until: Option<&str>) -> Result<Self, Failure> {
        Ok(Self {
            since: since.map(parse_ts).transpose()?,
            until: until.map(parse_ts).transpose()?,
        })
    }

    /// Ob eine Grenze gesetzt ist.
    const fn is_set(&self) -> bool {
        self.since.is_some() || self.until.is_some()
    }

    /// Ob ein Zeitpunkt des Logs in den Bereich fällt.
    ///
    /// Der Vergleich ist der über die Zeichen: `TS_FORMAT` ist UTC mit fester
    /// Breite, und dort ist die lexikalische Ordnung die zeitliche.
    fn contains(&self, ts: &str) -> bool {
        self.since.as_deref().is_none_or(|since| ts >= since)
            && self.until.as_deref().is_none_or(|until| ts < until)
    }
}

/// Ein Zeitpunkt der Kommandozeile in der Schreibweise des Logs.
fn parse_ts(text: &str) -> Result<String, Failure> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text).map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!("{text} is not an RFC 3339 timestamp: {error}"))
                .fix(FixAction::CopyCommand(
                    "humanitl audit export --since 2026-09-01T00:00:00Z".to_owned(),
                ))
                .build(),
        )
    })?;
    Ok(humanitl_audit::format_ts(parsed.to_utc()))
}

/// `audit export`.
async fn export(
    ctx: &Context,
    format: &str,
    out: &Path,
    range: &Range,
    file: Option<&Path>,
) -> Result<u8, Failure> {
    refuse_existing(out)?;

    // Der Daemon exportiert nur ohne Zeitgrenzen: Sein Vertrag kennt keine.
    // Er läuft in einem anderen Verzeichnis als dieser Aufruf; ein relativer
    // Pfad landete bei ihm woanders.
    if file.is_none() && !range.is_set() {
        match export_over_rpc(ctx, format, &ctx.cwd.join(out)).await {
            Ok(count) => {
                written(ctx, count, out, "daemon");
                return Ok(EXIT_OK);
            }
            Err(failure) => {
                let path = ctx.paths.audit_path();
                if !path.exists() {
                    return Err(failure);
                }
                ctx.render.note(&format!(
                    "{}; {} is exported instead",
                    fallback_warning(&failure).trim_end_matches(", so this is the file-mode check"),
                    path.display()
                ));
            }
        }
    }

    let path = file.map_or_else(|| ctx.paths.audit_path(), Path::to_path_buf);
    let count = export_file(&path, format, out, range)?;
    written(ctx, count, out, &format!("file:{}", path.display()));
    Ok(EXIT_OK)
}

/// Die Meldung nach einem Export, als Zeile oder als JSON-Objekt.
fn written(ctx: &Context, records: u64, out: &Path, source: &str) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "exported": records,
            "out": out.display().to_string(),
            "source": source,
        }));
        return;
    }
    ctx.render
        .line(&format!("exported {records} records to {}", out.display()));
}

/// Der Export über den Daemon; die Zahl der Records aus seiner Antwort.
async fn export_over_rpc(ctx: &Context, format: &str, out: &Path) -> Result<u64, Failure> {
    let mut client = ctx.connect().await?;
    let answer = client
        .audit(v1::AuditRequest {
            op: Some(v1::audit_request::Op::Export(v1::audit_request::Export {
                format: format.to_owned(),
                out_path: out.display().to_string(),
                redact_hosts: false,
            })),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Audit(Export)")))?
        .into_inner();
    Ok(answer.entries)
}

/// Der Export aus der Datei.
///
/// Geschrieben wird in eine Nebendatei im selben Verzeichnis, mit `fsync`, und
/// erst der fertige Export bekommt seinen Namen — über `hard_link`, das wie
/// `create_new` an einem vorhandenen Pfad scheitert, auch an einem Verweis.
/// Ein Log, das mittendrin bricht, hinterlässt so keine halbe Datei, die den
/// nächsten Versuch mit `AUDIT_008` abwiese; die Nebendatei verschwindet auf
/// jedem Weg.
fn export_file(path: &Path, format: &str, out: &Path, range: &Range) -> Result<u64, Failure> {
    let file = File::open(path).map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::AUDIT_006, Severity::Error)
                .why(format!(
                    "cannot read {} to export it: {error}",
                    path.display()
                ))
                .build(),
        )
    })?;

    let staged = Staged::create(out)?;
    let result = write_export(file, path, format, out, range, &staged).and_then(|count| {
        publish(&staged.path, out)?;
        Ok(count)
    });
    // Die Nebendatei geht in jedem Fall; nach dem Verweis trägt `out` den Inhalt.
    drop(staged);
    result
}

/// Schreibt die Zeilen in die Nebendatei und zählt sie.
fn write_export(
    file: File,
    path: &Path,
    format: &str,
    out: &Path,
    range: &Range,
    staged: &Staged,
) -> Result<u64, Failure> {
    let csv = format == "csv";
    // RFC 4180 schließt jede Zeile mit CRLF; JSON-Zeilen enden mit LF.
    let end = if csv { "\r\n" } else { "\n" };
    let mut sink = std::io::BufWriter::new(&staged.file);
    let mut put = |text: &str| {
        sink.write_all(text.as_bytes())
            .and_then(|()| sink.write_all(end.as_bytes()))
            .map_err(|error| Failure::new(unwritable(out, &error.to_string())))
    };
    if csv {
        put(&CSV_COLUMNS.join(","))?;
    }

    let mut count = 0_u64;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|error| {
            Failure::new(
                Diagnostic::builder(codes::AUDIT_006, Severity::Error)
                    .why(format!(
                        "{} stops at line {}: {error}",
                        path.display(),
                        index + 1
                    ))
                    .build(),
            )
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let record = AuditRecord::from_line(line.as_bytes())
            .map_err(|error| Failure::new(not_a_record(path, index + 1, &error.to_string())))?;
        if !range.contains(&record.body.ts) {
            continue;
        }
        let row = if csv { csv_row(&record) } else { line };
        put(&row)?;
        count += 1;
    }
    sink.flush()
        .map_err(|error| Failure::new(unwritable(out, &error.to_string())))?;
    drop(sink);
    staged
        .file
        .sync_all()
        .map_err(|error| Failure::new(unwritable(out, &error.to_string())))?;
    Ok(count)
}

/// Gibt dem fertigen Export seinen Namen, ohne je etwas zu überschreiben.
///
/// Erst `hard_link`: Es scheitert an einem vorhandenen Pfad, auch an einem
/// Verweis. Dateisysteme ohne harte Verweise (vfat, exFAT) antworten mit einem
/// anderen Fehler; dann `renameat2` mit `RENAME_NOREPLACE`, das dieselbe
/// Zusage macht. Ein vorhandenes Ziel bleibt in beiden Fällen `AUDIT_008`.
fn publish(staged: &Path, out: &Path) -> Result<(), Failure> {
    match std::fs::hard_link(staged, out) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(Failure::new(unwritable(out, &error.to_string())))
        }
        Err(_) => rustix::fs::renameat_with(
            rustix::fs::CWD,
            staged,
            rustix::fs::CWD,
            out,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|error| Failure::new(unwritable(out, &error.to_string()))),
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
    fn create(out: &Path) -> Result<Self, Failure> {
        let dir = out
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        std::fs::create_dir_all(&dir)
            .map_err(|error| Failure::new(unwritable(out, &error.to_string())))?;
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
            .map_err(|error| Failure::new(unwritable(out, &error.to_string())))?;
        // Die Sperre hält, solange die Datei offen ist, also bis der Export
        // seinen Namen hat oder aufgegeben ist; ein anderer Export sieht an
        // ihr, dass diese Nebendatei nicht verwaist ist.
        rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
            .map_err(|error| Failure::new(unwritable(out, &error.to_string())))?;
        let staged = Self { path, file };
        if let Ok(meta) = staged.file.metadata() {
            use std::os::unix::fs::MetadataExt as _;
            sweep_stale(&dir, &name, &staged.path, meta.uid());
        }
        Ok(staged)
    }
}

/// Wie alt eine Nebendatei mindestens ist, bevor ein anderer Export sie für
/// verwaist halten darf.
///
/// Zwischen dem Anlegen einer Nebendatei und ihrer Sperre liegt ein
/// Augenblick; eine Datei aus diesem Augenblick ist jung und bleibt.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(60);

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
    use std::os::unix::fs::MetadataExt as _;

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

impl Drop for Staged {
    fn drop(&mut self) {
        // Best effort: Ein Rest bliebe nur, wenn das Verzeichnis inzwischen
        // nicht mehr schreibbar ist, und dann sagt der Befund ohnehin mehr.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Eine Zeile des CSV: die acht Felder in der Reihenfolge von [`CSV_COLUMNS`].
fn csv_row(record: &AuditRecord) -> String {
    let data = record.body.data.to_string();
    [
        record.body.seq.to_string(),
        record.body.ts.clone(),
        record.body.session.clone(),
        record.body.kind.clone(),
        data,
        record.body.prev.clone(),
        record.hash.clone(),
        record.mac.clone(),
    ]
    .iter()
    .map(|field| csv_field(field))
    .collect::<Vec<_>>()
    .join(",")
}

/// Ein Feld nach RFC 4180: in Anführungszeichen, sobald es welche braucht.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// Eine Datei, die schon da ist, wird nicht überschrieben.
///
/// Ein Export ist ein Beleg. Wer ihn zweimal in denselben Pfad schreibt, will
/// fast nie den ersten verlieren, und ein `--force` gibt es aus demselben
/// Grund nicht wie bei `daemon install`.
fn refuse_existing(out: &Path) -> Result<(), Failure> {
    // `symlink_metadata` und nicht `exists`: Ein Verweis ins Leere ist ein
    // Pfad, der schon da ist, und bekommt denselben Befund samt Vorschlag.
    if std::fs::symlink_metadata(out).is_err() {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::AUDIT_008, Severity::Error)
            .why(format!(
                "{} is already there; the export writes no file over a file that exists",
                out.display()
            ))
            // `-n` überschreibt kein vorhandenes Ziel, `--` lässt einen Namen
            // mit `-` am Anfang einen Namen sein, und der Zeitstempel macht das
            // Ziel eindeutig: Ein älterer Beleg wird so nie überschrieben.
            .fix(FixAction::CopyCommand(format!(
                "mv -n -- {} {}.$(date -u +%Y%m%dT%H%M%SZ)",
                shell_path(out),
                shell_path(out)
            )))
            .build(),
    ))
}

/// `AUDIT_008`: Der Export ließ sich nicht schreiben.
fn unwritable(out: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(codes::AUDIT_008, Severity::Error)
        .why(format!("{} could not be written: {why}", out.display()))
        .build()
}

/// `AUDIT_001`: Im Log steht eine Zeile, die kein Record ist.
fn not_a_record(path: &Path, line: usize, why: &str) -> Diagnostic {
    Diagnostic::builder(codes::AUDIT_001, Severity::Error)
        .why(format!(
            "{} line {line} is not an audit record ({why}); nothing is exported from a log that \
             does not hold",
            path.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl audit verify --file {}",
            shell_path(path)
        )))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_audit::{AuditRecord, GENESIS_PREV, RecordBody};
    use serde_json::json;

    use super::{CSV_COLUMNS, Range, csv_field, csv_row, short_hash};

    fn record(seq: u64, ts: &str) -> AuditRecord {
        RecordBody {
            seq,
            ts: ts.to_owned(),
            session: "-".to_owned(),
            kind: "flow.decided".to_owned(),
            data: json!({ "note": "a, \"quoted\" note" }),
            prev: GENESIS_PREV.to_owned(),
        }
        .seal(&[7_u8; 32])
        .expect("the record seals")
    }

    #[test]
    fn a_csv_row_has_one_field_per_column() {
        let row = csv_row(&record(1, "2026-09-02T10:42:01.000000Z"));
        // Nicht an Kommas zählen: `data` trägt selbst welche und steht
        // deshalb in Anführungszeichen.
        assert!(row.starts_with("1,2026-09-02T10:42:01.000000Z,-,flow.decided,\""));
        assert_eq!(CSV_COLUMNS.len(), 8);
        assert!(row.ends_with(&record(1, "2026-09-02T10:42:01.000000Z").mac));
    }

    #[test]
    fn a_field_with_a_quote_is_doubled_and_wrapped() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    /// Die Grenzen sind halboffen: `since` gehört dazu, `until` nicht. Sonst
    /// stünde ein Record in zwei aufeinanderfolgenden Exporten.
    #[test]
    fn the_range_is_half_open() {
        let range = Range::new(Some("2026-09-02T10:00:00Z"), Some("2026-09-02T11:00:00Z"))
            .expect("both bounds parse");
        assert!(range.is_set());
        assert!(range.contains("2026-09-02T10:00:00.000000Z"));
        assert!(range.contains("2026-09-02T10:59:59.999999Z"));
        assert!(!range.contains("2026-09-02T11:00:00.000000Z"));
        assert!(!range.contains("2026-09-02T09:59:59.999999Z"));
    }

    #[test]
    fn an_unreadable_timestamp_is_cli_004() {
        let failure = Range::new(Some("yesterday"), None).expect_err("that is no timestamp");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_004");
        assert_eq!(failure.exit, crate::cmd::EXIT_USER);
    }

    #[test]
    fn a_hash_is_shortened_at_both_ends() {
        assert_eq!(short_hash("a3f90000000000c2e1"), "a3f9…c2e1");
        assert_eq!(short_hash("abc"), "abc");
    }
}
