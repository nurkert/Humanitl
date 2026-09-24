//! `humanitl audit verify` und `humanitl audit export` (HUM-070, HUM-156).
//!
//! Die Kette beweist eine Änderung, Löschung oder Umordnung vor dem letzten
//! Anker, solange der Angreifer den HMAC-Schlüssel nicht hat (`docs/SECURITY.md`,
//! „Was die Audit-Kette beweist"). Diese Zusage hängt an drei Dingen: der Kette
//! selbst, den MACs und den Ankern. Nur der Daemon hat alle drei — den
//! Schlüssel aus dem Schlüsselspeicher und die Anker aus der Tabelle
//! `audit_anchors`.
//!
//! Prüfen tut nur `verify`. `export` verweigert eine Zeile, die kein Record
//! ist, auch eine leere, mit `AUDIT_001`, und kopiert (JSONL) oder formatiert
//! (CSV) sonst die Records im Zeitraum, ohne Hash, MAC oder Anker zu prüfen
//! (HUM-214); ob die exportierte Kette hält, sagt `verify`.
//!
//! Beide fragen zuerst den Daemon, und seine Antwort gilt, auch eine
//! Ablehnung: bei `verify`, weil nur er Schlüssel und Anker hat, bei `export`,
//! weil er das Log liest, an das er gerade schreibt, und weiß, bis wohin es
//! fertig ist. Nur wenn **kein** Daemon antwortet — keiner erreichbar,
//! keiner, der das Token annimmt, oder einer, der `Audit` noch nicht kennt —,
//! oder wenn die Datei ausdrücklich genannt ist (`--file`), prüft die
//! Kommandozeile selbst. Dann aber ohne Schlüssel und ohne Anker, und **die
//! Ausgabe sagt das**: `warnings: no HMAC key (file mode)`. Eine schwächere
//! Prüfung, die sich nicht als schwächer zu erkennen gibt, wiegt einen
//! Menschen in Sicherheit, und das ist schlimmer, als gar nicht zu prüfen.
//! Ein Daemon, der antwortet und ablehnt (das Log unlesbar, die Anker
//! unlesbar), bekommt keine schwächere Prüfung als Ersatz: Sein Befund ist die
//! Auskunft.
//!
//! Der Export nimmt denselben Weg und schreibt in beiden Fällen dieselbe Datei
//! (`humanitl_audit::export`): JSONL Byte für Byte die Kette, CSV mit den zwölf
//! Spalten aus HUM-050.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, DurationRound as _, TimeDelta, Utc};
use humanitl_audit::export::{self, ExportFormat};
use humanitl_audit::{
    AuditRecord, AuditVerifier, BreakReason, TimeRange, VerifyStatus, VerifyWarning,
};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::{Value, json};
use tonic::{Code, Status};

use crate::cli::AuditCmd;
use crate::cmd::{Context, EXIT_OK, EXIT_SECURITY, Failure, status_diagnostic};
use crate::render::{labeled, plain};

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

/// Warum der Daemon nicht geliefert hat.
#[derive(Debug)]
enum Refusal {
    /// Kein Daemon hat geantwortet: keiner erreichbar, keiner, der das Token
    /// annimmt, oder einer, der `Audit` nicht kennt. Nur dann prüft die
    /// Kommandozeile die Datei selbst.
    Unanswered(Failure),
    /// Der Daemon hat geantwortet und abgelehnt; sein Befund ist die Auskunft.
    Refused(Failure),
}

impl Refusal {
    /// Ein Fehler des Aufrufs, eingeordnet nach seinem gRPC-Code.
    fn of(status: &Status, what: &str) -> Self {
        let failure = Failure::new(status_diagnostic(status, what));
        match status.code() {
            Code::Unavailable | Code::Unauthenticated | Code::Unimplemented => {
                Self::Unanswered(failure)
            }
            _ => Self::Refused(failure),
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

/// Die Anker, wie der Daemon sie gemeldet hat.
#[derive(Debug)]
struct Anchors {
    /// Wie viele in `audit_anchors` stehen.
    count: u64,
    /// Wann der letzte entstand, im Format des Logs.
    last_at: Option<String>,
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
    /// Die Anker, wenn der Daemon geprüft hat; in der Datei stehen keine.
    anchors: Option<Anchors>,
    /// Wo die Kette bricht, wenn sie bricht.
    broken: Option<Broken>,
    /// Was diese Prüfung nicht beweisen konnte.
    warnings: Vec<String>,
}

impl Summary {
    /// Ob Schlüssel und Anker in die Prüfung gingen: nur beim Daemon.
    const fn full(&self) -> bool {
        matches!(self.source, Source::Daemon)
    }
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
            Err(Refusal::Refused(failure)) => return Err(failure),
            Err(Refusal::Unanswered(failure)) => {
                // Kein Daemon hat geantwortet. Die Datei liegt trotzdem, und
                // eine schwächere Antwort auf die Frage ist mehr wert als
                // keine — solange sie sagt, dass sie die schwächere ist. Gibt
                // es die Datei nicht, bleibt es beim Befund: Dann ist wirklich
                // nichts zu prüfen.
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
/// Ein Daemon, der nicht erreichbar ist, und einer, der antwortet, aber
/// `Audit` nicht kennt (eine Fassung vor HUM-156), sind zwei verschiedene
/// Lagen; „is not reachable" wäre für die zweite gelogen.
fn fallback_warning(failure: &Failure) -> String {
    let why = plain(&failure.diagnostic.why);
    match failure.diagnostic.code.as_str() {
        "DAEMON_001" | "DAEMON_002" | "IPC_001" => {
            format!("the daemon is not reachable ({why}), so this is the file-mode check")
        }
        _ => format!("the daemon does not answer Audit ({why}), so this is the file-mode check"),
    }
}

/// Die Prüfung durch den Daemon: mit Schlüssel und Ankern, also die ganze.
async fn verify_over_rpc(ctx: &Context) -> Result<Summary, Refusal> {
    let mut client = ctx.connect().await.map_err(Refusal::Unanswered)?;
    let answer = client
        .audit(v1::AuditRequest {
            op: Some(v1::audit_request::Op::Verify(())),
        })
        .await
        .map_err(|status| Refusal::of(&status, "Audit(Verify)"))?
        .into_inner();

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
            reason: (!answer.break_reason.is_empty()).then(|| answer.break_reason.clone()),
            diagnostic,
        }
    });
    let head = (broken.is_none() && !answer.head_hash.is_empty()).then(|| Head {
        hash: hex::encode(&answer.head_hash),
        seq: answer.head_seq,
        ts: None,
    });
    let anchors = answer.anchors_reported.then(|| Anchors {
        count: answer.anchors,
        last_at: answer.last_anchor_at.as_ref().and_then(|at| {
            u32::try_from(at.nanos)
                .ok()
                .and_then(|nanos| DateTime::from_timestamp(at.seconds, nanos))
                .map(humanitl_audit::format_ts)
        }),
    });
    let warnings = answer
        .warnings
        .iter()
        .map(|warning| match warning.kind.as_str() {
            "no_hmac_key" => "no HMAC key".to_owned(),
            "unanchored_tail" => format!("unanchored tail: {} records", warning.records),
            "pruned" => pruned_warning(warning.records),
            other => other.to_owned(),
        })
        .collect();
    Ok(Summary {
        source: Source::Daemon,
        records: answer.entries,
        head,
        anchors,
        broken,
        warnings,
    })
}

/// Die Warnung einer Kette, deren Anfang `audit.retention_days` gelöscht hat
/// (HUM-157): bis zu welcher Nummer, und dass deren Inhalt unbewiesen bleibt.
fn pruned_warning(through_seq: u64) -> String {
    format!("pruned: records 1 to {through_seq} deleted by audit.retention_days, unproven")
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
            VerifyWarning::Pruned { through_seq } => pruned_warning(through_seq),
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
        anchors: None,
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
        "hmac key",
        if summary.full() {
            "checked by the daemon".to_owned()
        } else {
            "not checked".to_owned()
        },
    ));
    rows.push(("anchors", anchors_line(summary)));
    if !summary.warnings.is_empty() {
        rows.push(("warnings", summary.warnings.join("; ")));
    }
    rows.push(("checked by", summary.source.as_str()));
    ctx.render.line(labeled(&rows).trim_end());

    if let Some(broken) = summary.broken.as_ref() {
        ctx.render.diagnostic(&broken.diagnostic);
    }
}

/// Die Zeile zu den Ankern: wie viele, der letzte, und wer sie geprüft hat.
fn anchors_line(summary: &Summary) -> String {
    if !summary.full() {
        return "not checked".to_owned();
    }
    match summary.anchors.as_ref() {
        None => "checked by the daemon".to_owned(),
        Some(anchors) => {
            let mut line = anchors.count.to_string();
            if let Some(at) = anchors.last_at.as_ref() {
                let _ = write!(line, " (last at {at})");
            }
            line.push_str(", checked by the daemon");
            line
        }
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
    let checked = |yes: bool| if yes { "checked" } else { "not_checked" };
    let mut value = json!({
        "chain": if summary.broken.is_some() { "broken" } else { "ok" },
        "records": summary.records,
        "mode": if summary.full() { "full" } else { "file" },
        "hmac": checked(summary.full()),
        "anchors": checked(summary.full()),
        "warnings": summary.warnings,
        "checked_by": summary.source.as_str(),
    });
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    if let Some(anchors) = summary.anchors.as_ref() {
        object.insert("anchor_count".to_owned(), json!(anchors.count));
        object.insert("last_anchor_at".to_owned(), json!(anchors.last_at));
    }
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

/// Die beiden Zeitgrenzen eines Exports: `since` gehört dazu, `until` nicht.
///
/// Halboffen, damit ein Record nie in zwei aufeinanderfolgenden Exporten
/// steht. Der Vertrag (`AuditRequest.Export`) schließt beide Grenzen ein; das
/// Log schreibt Mikrosekunden, also ist „vor `until`" dasselbe wie „höchstens
/// die letzte ganze Mikrosekunde vor `until`", und genau die geht als obere
/// Grenze über die Leitung.
#[derive(Debug, Default)]
struct Range {
    /// Ab diesem Zeitpunkt, einschließlich.
    since: Option<DateTime<Utc>>,
    /// Die letzte Mikrosekunde vor `--until`, einschließlich.
    last: Option<DateTime<Utc>>,
}

impl Range {
    /// Liest beide Grenzen als RFC 3339.
    ///
    /// # Errors
    ///
    /// `CLI_004`, wenn ein Zeitpunkt sich nicht lesen lässt.
    fn new(since: Option<&str>, until: Option<&str>) -> Result<Self, Failure> {
        let micro = TimeDelta::microseconds(1);
        Ok(Self {
            since: since.map(parse_ts).transpose()?,
            last: until
                .map(parse_ts)
                .transpose()?
                .map(|until| until.duration_round_up(micro).unwrap_or(until) - micro),
        })
    }

    /// Derselbe Zeitraum als [`TimeRange`] des Logs.
    fn time_range(&self) -> TimeRange {
        TimeRange::new(self.since, self.last)
    }

    /// Ein Zeitpunkt für die Leitung.
    fn wire(at: Option<DateTime<Utc>>) -> Option<prost_types::Timestamp> {
        at.map(|at| prost_types::Timestamp {
            seconds: at.timestamp(),
            nanos: i32::try_from(at.timestamp_subsec_nanos()).unwrap_or(0),
        })
    }
}

/// Ein Zeitpunkt der Kommandozeile.
fn parse_ts(text: &str) -> Result<DateTime<Utc>, Failure> {
    let parsed = DateTime::parse_from_rfc3339(text).map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!("{text} is not an RFC 3339 timestamp: {error}"))
                .fix(FixAction::CopyCommand(
                    "humanitl audit export --since 2026-09-01T00:00:00Z".to_owned(),
                ))
                .build(),
        )
    })?;
    Ok(parsed.to_utc())
}

/// `audit export`.
async fn export(
    ctx: &Context,
    format: &str,
    out: &Path,
    range: &Range,
    file: Option<&Path>,
) -> Result<u8, Failure> {
    let format = ExportFormat::parse(format).ok_or_else(|| {
        Failure::new(
            Diagnostic::builder(codes::AUDIT_009, Severity::Error)
                .why(format!(
                    "{format:?} is not an export format; use jsonl or csv"
                ))
                .fix(FixAction::CopyCommand(
                    "humanitl audit export --format jsonl --out audit.jsonl".to_owned(),
                ))
                .build(),
        )
    })?;
    // Der Daemon läuft in einem anderen Verzeichnis als dieser Aufruf; ein
    // relativer Pfad landete bei ihm woanders.
    let absolute = ctx.cwd.join(out);
    // Das Verzeichnis legt die Kommandozeile an, wie seit HUM-070: Sie läuft
    // als der Mensch, der den Pfad genannt hat. Der Daemon legt keines an
    // (HUM-156), und der Export selbst auch nicht.
    if let Some(dir) = absolute.parent() {
        std::fs::create_dir_all(dir).map_err(|error| {
            Failure::new(
                Diagnostic::builder(codes::AUDIT_008, Severity::Error)
                    .why(format!(
                        "the directory {} for the export cannot be created: {error}",
                        dir.display()
                    ))
                    .fix(FixAction::CopyCommand(format!(
                        "ls -ld {}",
                        crate::render::shell_path(dir)
                    )))
                    .build(),
            )
        })?;
    }

    if file.is_none() {
        match export_over_rpc(ctx, format, &absolute, range).await {
            Ok(count) => {
                // Der Daemon schreibt in seine eigene Sicht der Dateien. Sieht
                // dieser Aufruf die Datei danach nicht, ist sie für den Menschen
                // nicht da, was auch immer der Daemon meldet.
                if std::fs::symlink_metadata(&absolute).is_err() {
                    return Err(Failure::new(not_visible(&absolute)));
                }
                written(ctx, count, out, "daemon");
                return Ok(EXIT_OK);
            }
            Err(Refusal::Refused(failure)) => return Err(failure),
            Err(Refusal::Unanswered(failure)) => {
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
    let count = export::export(&path, format, &range.time_range(), &absolute, None)
        .map_err(Failure::new)?;
    written(ctx, count, out, &format!("file:{}", path.display()));
    Ok(EXIT_OK)
}

/// `AUDIT_008`: Der Daemon meldet einen Export, den dieser Aufruf nicht sieht.
fn not_visible(out: &Path) -> Diagnostic {
    Diagnostic::builder(codes::AUDIT_008, Severity::Error)
        .why(format!(
            "the daemon reports {} as written, but it is not there for this command; the \
             daemon sees another file system here, for example its own /tmp under PrivateTmp",
            out.display()
        ))
        .fix(FixAction::CopyCommand(
            "humanitl audit export --format jsonl --out ~/humanitl-audit.jsonl".to_owned(),
        ))
        .build()
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
async fn export_over_rpc(
    ctx: &Context,
    format: ExportFormat,
    out: &Path,
    range: &Range,
) -> Result<u64, Refusal> {
    let mut client = ctx.connect().await.map_err(Refusal::Unanswered)?;
    let answer = client
        .audit(v1::AuditRequest {
            op: Some(v1::audit_request::Op::Export(v1::audit_request::Export {
                format: format.as_str().to_owned(),
                out_path: out.display().to_string(),
                redact_hosts: false,
                from: Range::wire(range.since),
                to: Range::wire(range.last),
            })),
        })
        .await
        .map_err(|status| Refusal::of(&status, "Audit(Export)"))?
        .into_inner();
    if !answer.ok {
        // Eine Ablehnung ohne gRPC-Fehler ist trotzdem eine Ablehnung. Sie als
        // Erfolg zu lesen hieße, „exported" über eine Datei zu schreiben, die
        // es nicht gibt.
        let diagnostic = answer
            .diagnostic
            .as_ref()
            .and_then(crate::cmd::from_proto)
            .unwrap_or_else(|| {
                Diagnostic::builder(codes::AUDIT_008, Severity::Error)
                    .why(format!(
                        "the daemon did not write {} and did not say why",
                        out.display()
                    ))
                    .build()
            });
        return Err(Refusal::Refused(Failure::new(diagnostic)));
    }
    Ok(answer.entries)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use tonic::Status;

    use super::{Range, Refusal, short_hash};

    /// Die Grenzen sind halboffen: `since` gehört dazu, `until` nicht. Sonst
    /// stünde ein Record in zwei aufeinanderfolgenden Exporten.
    #[test]
    fn the_range_is_half_open() {
        let range = Range::new(Some("2026-09-02T10:00:00Z"), Some("2026-09-02T11:00:00Z"))
            .expect("both bounds parse")
            .time_range();
        assert!(range.is_bounded());
        assert!(range.contains("2026-09-02T10:00:00.000000Z"));
        assert!(range.contains("2026-09-02T10:59:59.999999Z"));
        assert!(!range.contains("2026-09-02T11:00:00.000000Z"));
        assert!(!range.contains("2026-09-02T09:59:59.999999Z"));
    }

    /// Eine obere Grenze zwischen zwei Mikrosekunden lässt die frühere drin.
    #[test]
    fn an_until_between_two_microseconds_keeps_the_earlier_one() {
        let range = Range::new(None, Some("2026-09-02T11:00:00.0000005Z"))
            .expect("the bound parses")
            .time_range();
        assert!(range.contains("2026-09-02T11:00:00.000000Z"));
        assert!(!range.contains("2026-09-02T11:00:00.000001Z"));
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

    /// Nur ein Daemon, der nicht antwortet, lässt die Datei prüfen; einer, der
    /// ablehnt, hat damit geantwortet.
    #[test]
    fn only_a_silent_daemon_leads_to_the_file() {
        for silent in [
            Status::unavailable("gone"),
            Status::unauthenticated("token"),
            Status::unimplemented("Audit arrives later"),
        ] {
            assert!(matches!(
                Refusal::of(&silent, "Audit(Verify)"),
                Refusal::Unanswered(_)
            ));
        }
        for answered in [
            Status::internal("cannot read the log"),
            Status::invalid_argument("bad cursor"),
            Status::failed_precondition("no audit log"),
        ] {
            assert!(matches!(
                Refusal::of(&answered, "Audit(Verify)"),
                Refusal::Refused(_)
            ));
        }
    }
}
