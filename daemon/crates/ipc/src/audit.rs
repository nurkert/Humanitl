//! Die `Audit`-RPC: Prüfung, Ende, Seiten und Export der Kette (HUM-156).
//!
//! Die Kette schreibt der Daemon (HUM-050), und nur er hat alles, was die
//! Prüfung braucht: den HMAC-Schlüssel und die Anker aus `audit_anchors`. Die
//! Oberfläche (HUM-051) und `humanitl audit` (HUM-070) fragen deshalb ihn und
//! prüfen dieselbe Kette mit derselben Stärke.
//!
//! Vier Operationen, jede über die Datei, wie sie auf der Platte liegt:
//!
//! - `verify` prüft jeden Record mit Schlüssel und Ankern
//!   ([`humanitl_audit::AuditVerifier`]); eine gebrochene Kette ist eine
//!   Antwort mit `ok: false` und dem Befund `AUDIT_001`, kein Fehler des
//!   Aufrufs.
//! - `head` nennt das Ende der Datei und die Anker, ohne zu prüfen.
//! - `query` liefert eine Seite, neueste Nummer zuerst, mit Cursor.
//! - `export` schreibt JSONL oder CSV in eine Datei, die es noch nicht gibt.
//!
//! **Vor jeder Operation ist alles auf der Platte**, was der Daemon bis dahin
//! geschickt hat ([`AuditHandle::sync`]). Sonst sähe die Oberfläche einen
//! anderen Kopf als die Kommandozeile, je nachdem, wie weit der Schreiber
//! gerade ist.
//!
//! **Und gelesen wird nur bis zu dem Ende, das `sync` meldet.** Der Schreiber
//! hängt weiter an, während die Operation liest; eine Zeile, die gerade
//! entsteht, stünde sonst halb in der Datei und hieße für die Prüfung
//! „gebrochen", samt dem Vorschlag, das laufende Log beiseitezulegen. Was hinter
//! dem gemeldeten Ende steht, ist für diese Operation noch nicht geschrieben,
//! und Anker hinter ihm zählen aus demselben Grund nicht.
//!
//! **Ein Export geht nur in ein Ziel, das der Mensch auch sieht.** Unter
//! `PrivateTmp=yes` (`packaging/systemd/humanitld.service`) hat der Daemon ein
//! eigenes `/tmp`; ein Export dorthin landete in
//! `/tmp/systemd-private-*` und wäre für den Aufrufer nicht da. Er wird mit
//! `AUDIT_008` abgelehnt. Das Zielverzeichnis muss es geben, und es darf kein
//! Verweis im Weg liegen: Ein Agent in der Sandbox hat dieselbe Nutzerkennung
//! und könnte im Projektverzeichnis einen Verweis nach
//! `~/.config/autostart` pflanzen.
//!
//! **Eine Prüfung schreibt keinen Record.** Die Art `audit.verified` steht im
//! Register (HUM-050), aber ein Record je Prüfung verschöbe den Kopf mit jeder
//! Frage: Der Hash, den die Oberfläche nach ihrer Prüfung zeigt, wäre nie der,
//! den `humanitl audit verify --json` danach meldet.
//!
//! **Was an der Anfrage nicht stimmt**, ist `AUDIT_009` und wirkt nicht: keine
//! Operation, ein Format außer `jsonl` und `csv`, ein Zielpfad, der nicht
//! absolut ist (der Daemon läuft in einem anderen Verzeichnis als sein
//! Aufrufer), eine verlangte Host-Schwärzung, die es noch nicht gibt, ein
//! Zeitpunkt oder Cursor, der sich nicht lesen lässt. Die Prüfung steht hier
//! und nicht in [`crate::validate`]: Der Fake hat kein Audit-Log und
//! antwortet auf jede Operation mit `IPC_006`, genau wie ein Daemon ohne
//! diesen Dienst; beide lehnen also ab, bevor es etwas zu prüfen gibt.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use humanitl_audit::export::{self, ExportFormat};
use humanitl_audit::query::{self, Page, QueryFilter, TimeRange};
use humanitl_audit::{
    Anchor, AuditHandle, AuditKey, AuditVerifier, VerifyReport, VerifyStatus, VerifyWarning,
    canonical_json,
};
use humanitl_core::diagnostics::codes;
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::convert::diagnostic_to_proto;
use crate::v1;

/// Der Dienst hinter `Audit`: wo die Kette liegt, wo die Anker liegen, womit
/// die MACs gerechnet werden.
///
/// Billig zu klonen; der Schlüssel liegt hinter einem `Arc` und wird nie
/// kopiert.
#[derive(Clone)]
pub struct AuditService {
    /// `audit.jsonl`.
    log: PathBuf,
    /// Die Datenbank der Aufzeichnung mit der Tabelle `audit_anchors`.
    db: PathBuf,
    /// Der Schlüssel, mit dem der Schreiber die MACs rechnet.
    key: Arc<AuditKey>,
    /// Der laufende Schreiber, falls es einen gibt; vor jeder Operation wird
    /// auf ihn gewartet.
    writer: Option<AuditHandle>,
    /// Die Einhängepunkte, gegen die ein Exportziel geprüft wird; `None`
    /// liest `/proc/self/mountinfo` bei jedem Export.
    mounts: Option<String>,
}

impl std::fmt::Debug for AuditService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditService")
            .field("log", &self.log)
            .field("db", &self.db)
            .field("writer", &self.writer.is_some())
            .finish_non_exhaustive()
    }
}

/// Eine gelesene Anfrage, bereit zur Ausführung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditOp {
    /// Die ganze Prüfung.
    Verify,
    /// Das Ende der Kette.
    Head,
    /// Eine Seite.
    Query {
        /// Welche Records.
        filter: QueryFilter,
        /// Wie viele höchstens; `0` heißt die Vorgabe.
        limit: usize,
        /// Nur Records unter dieser Nummer.
        before: Option<u64>,
    },
    /// Ein Export.
    Export {
        /// JSONL oder CSV.
        format: ExportFormat,
        /// Der absolute Zielpfad.
        out: PathBuf,
        /// Der Zeitraum.
        range: TimeRange,
    },
}

impl AuditService {
    /// Der Dienst über der Kette in `log`, den Ankern in `db` und dem
    /// Schlüssel `key`.
    #[must_use]
    pub fn new(log: PathBuf, db: PathBuf, key: Arc<AuditKey>) -> Self {
        Self {
            log,
            db,
            key,
            writer: None,
            mounts: None,
        }
    }

    /// Derselbe Dienst mit einer festen Tabelle der Einhängepunkte statt
    /// `/proc/self/mountinfo`; für Tests, die `PrivateTmp` nachstellen.
    #[must_use]
    pub fn with_mountinfo(mut self, mountinfo: String) -> Self {
        self.mounts = Some(mountinfo);
        self
    }

    /// Derselbe Dienst, der vor jeder Operation auf den Schreiber wartet.
    #[must_use]
    pub fn with_writer(mut self, writer: AuditHandle) -> Self {
        self.writer = Some(writer);
        self
    }

    /// Die Kette.
    #[must_use]
    pub fn log(&self) -> &Path {
        &self.log
    }

    /// Beantwortet eine Anfrage.
    ///
    /// # Errors
    ///
    /// `AUDIT_009` für eine Anfrage, die so nicht gilt; `AUDIT_006`, wenn sich
    /// das Log nicht lesen lässt; `RECORDER_00x`, wenn die Anker nicht lesbar
    /// sind; `AUDIT_008` und `AUDIT_001` aus dem Export.
    pub async fn answer(
        &self,
        request: &v1::AuditRequest,
    ) -> Result<v1::AuditResponse, Diagnostic> {
        let op = parse(request)?;
        let service = self.clone();
        tokio::task::spawn_blocking(move || service.run(op))
            .await
            .map_err(|error| {
                Diagnostic::builder(codes::AUDIT_006, Severity::Error)
                    .why(format!("the audit operation did not finish: {error}"))
                    .build()
            })?
    }

    /// Führt eine gelesene Operation aus. Blockiert.
    fn run(&self, op: AuditOp) -> Result<v1::AuditResponse, Diagnostic> {
        // Das Ende, bis zu dem gelesen wird. `None` ohne Schreiber oder mit
        // einem, der schon beendet ist: Dann wächst die Datei nicht mehr, und
        // alles, was er je schrieb, steht bereits darin.
        let until = self
            .writer
            .as_ref()
            .and_then(AuditHandle::sync)
            .map(|head| head.seq);
        match op {
            AuditOp::Verify => self.verify(until),
            AuditOp::Head => self.head(until),
            AuditOp::Query {
                filter,
                limit,
                before,
            } => self.query(&filter, limit, before, until),
            AuditOp::Export { format, out, range } => {
                let mounts = self.mounts.clone().unwrap_or_else(mount_table);
                self.export(format, &out, &range, until, &mounts)
            }
        }
    }

    fn verify(&self, until: Option<u64>) -> Result<v1::AuditResponse, Diagnostic> {
        let anchors = self.anchors(until)?;
        let report =
            AuditVerifier::verify_until(&self.log, Some(self.key.bytes()), &anchors, until)?;
        Ok(verify_response(&report, &self.log, &anchors))
    }

    fn head(&self, until: Option<u64>) -> Result<v1::AuditResponse, Diagnostic> {
        let anchors = self.anchors(until)?;
        let tail = query::tail(&self.log, until)?;
        let mut answer = anchored(&anchors);
        answer.ok = true;
        answer.entries = tail.records;
        if let Some(last) = tail.last {
            answer.head_seq = last.body.seq;
            // Ein Feld, das kein Hex ist, gibt keinen Kopf: Was die Datei an
            // dieser Stelle trägt, sagt die Prüfung, nicht der Kopf.
            answer.head_hash = last.hash_bytes().map(Vec::from).unwrap_or_default();
        }
        Ok(answer)
    }

    fn query(
        &self,
        filter: &QueryFilter,
        limit: usize,
        before: Option<u64>,
        until: Option<u64>,
    ) -> Result<v1::AuditResponse, Diagnostic> {
        let Page {
            entries,
            total,
            next_before,
        } = query::query(&self.log, filter, limit, before, until)?;
        Ok(v1::AuditResponse {
            ok: true,
            entries: total,
            records: entries
                .into_iter()
                .map(|entry| v1::AuditEntry {
                    seq: entry.record.body.seq,
                    ts: entry.record.body.ts,
                    kind: entry.record.body.kind,
                    session: entry.record.body.session,
                    // `data` ist aus einer Zeile gelesen, die kanonisch war;
                    // es gibt also keinen Float, und der Rückfall greift nie.
                    data_json: canonical_json(&entry.record.body.data).map_or_else(
                        |_| entry.record.body.data.to_string(),
                        |bytes| String::from_utf8_lossy(&bytes).into_owned(),
                    ),
                    line: entry.line,
                })
                .collect(),
            next_cursor: next_before.map(|seq| seq.to_string()).unwrap_or_default(),
            ..v1::AuditResponse::default()
        })
    }

    fn export(
        &self,
        format: ExportFormat,
        out: &Path,
        range: &TimeRange,
        until: Option<u64>,
        mounts: &str,
    ) -> Result<v1::AuditResponse, Diagnostic> {
        refuse_private_tmp(out, mounts)?;
        refuse_linked_parent(out)?;
        let count = export::export(&self.log, format, range, out, until)?;
        Ok(v1::AuditResponse {
            ok: true,
            entries: count,
            out_path: out.display().to_string(),
            ..v1::AuditResponse::default()
        })
    }

    /// Die Anker aus `audit_anchors` bis zum gemeldeten Ende, aufsteigend.
    fn anchors(&self, until: Option<u64>) -> Result<Vec<Anchor>, Diagnostic> {
        Ok(humanitl_recorder::read_anchors(&self.db)?
            .into_iter()
            .filter(|anchor| until.is_none_or(|until| anchor.seq <= until))
            .map(|anchor| Anchor {
                seq: anchor.seq,
                hash: anchor.hash,
                ts: anchor.ts,
            })
            .collect())
    }
}

/// Die Antwort auf eine Prüfung.
///
/// Der Kopf ist der letzte Record, der bestanden hat; bei einer Kette, die
/// hält, ihr Ende und damit derselbe Hash, den `head` nennt.
#[must_use]
pub fn verify_response(report: &VerifyReport, log: &Path, anchors: &[Anchor]) -> v1::AuditResponse {
    let mut answer = anchored(anchors);
    answer.ok = report.is_ok();
    answer.entries = report.records;
    if let Some(head) = report.head.as_ref() {
        answer.head_seq = head.seq;
        answer.head_hash = hex::decode(&head.hash).unwrap_or_default();
    }
    if let VerifyStatus::Broken {
        first_bad_seq,
        reason,
    } = report.status
    {
        answer.first_bad_seq = first_bad_seq;
        reason.as_str().clone_into(&mut answer.break_reason);
    }
    answer.warnings = report
        .warnings
        .iter()
        .map(|warning| match *warning {
            VerifyWarning::NoHmacKey => v1::AuditWarning {
                kind: "no_hmac_key".to_owned(),
                records: 0,
            },
            VerifyWarning::UnanchoredTail { records } => v1::AuditWarning {
                kind: "unanchored_tail".to_owned(),
                records,
            },
            // `records` trägt hier die Nummer des letzten gelöschten Records
            // (HUM-157); der Vertrag hat für Warnungen nur diese eine Zahl.
            VerifyWarning::Pruned { through_seq } => v1::AuditWarning {
                kind: "pruned".to_owned(),
                records: through_seq,
            },
        })
        .collect();
    answer.diagnostic = report.diagnostic(log).as_ref().map(diagnostic_to_proto);
    answer
}

/// Die Einhängepunkte dieses Prozesses; leer, wenn sie sich nicht lesen
/// lassen.
fn mount_table() -> String {
    std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default()
}

/// Ob `/tmp` oder `/var/tmp` in `mountinfo` ein privates Verzeichnis von
/// systemd ist (`PrivateTmp=yes`).
///
/// systemd hängt dafür ein Unterverzeichnis `systemd-private-*` über `/tmp`;
/// in `mountinfo` steht es als Wurzel (viertes Feld) des Einhängepunkts
/// (fünftes Feld). Ein gewöhnliches `tmpfs` auf `/tmp` hat die Wurzel `/` und
/// ist für jeden Prozess dasselbe.
#[must_use]
pub fn private_tmp(mountinfo: &str, dir: &str) -> bool {
    mountinfo.lines().any(|line| {
        let mut fields = line.split(' ');
        let root = fields.nth(3);
        let point = fields.next();
        point == Some(dir) && root.is_some_and(|root| root.contains("/systemd-private-"))
    })
}

/// `AUDIT_008`, wenn `out` unter einem `/tmp` liegt, das nur dieser Daemon
/// sieht.
///
/// # Errors
///
/// `AUDIT_008` mit dem Grund und einem Ziel unter `$HOME` als Vorschlag.
pub fn refuse_private_tmp(out: &Path, mountinfo: &str) -> Result<(), Diagnostic> {
    for dir in ["/tmp", "/var/tmp"] {
        if out.starts_with(dir) && private_tmp(mountinfo, dir) {
            return Err(Diagnostic::builder(codes::AUDIT_008, Severity::Error)
                .why(format!(
                    "{} lies under {dir}, and this daemon runs with PrivateTmp: its {dir} is a \
                     directory of its own under /tmp/systemd-private-*, so the export would \
                     not be where you look for it; nothing was written",
                    out.display()
                ))
                .fix(FixAction::CopyCommand(
                    "humanitl audit export --format jsonl --out ~/humanitl-audit.jsonl".to_owned(),
                ))
                .build());
        }
    }
    Ok(())
}

/// `AUDIT_008`, wenn das Zielverzeichnis fehlt oder ein Verweis auf dem Weg
/// dorthin liegt.
///
/// Verglichen wird das Verzeichnis mit seiner aufgelösten Form. Zwischen
/// Prüfung und Schreiben bleibt ein Fenster; der Export legt aber keine
/// Verzeichnisse an und überschreibt keine Datei, und wer im Fenster einen
/// Verweis tauscht, braucht Schreibrecht auf den Weg, den der Mensch selbst
/// genannt hat.
///
/// # Errors
///
/// `AUDIT_008` mit dem Grund.
pub fn refuse_linked_parent(out: &Path) -> Result<(), Diagnostic> {
    let parent = out
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("/"));
    let refused = |why: String| {
        Diagnostic::builder(codes::AUDIT_008, Severity::Error)
            .why(why)
            .fix(FixAction::CopyCommand(format!(
                "ls -ld {}",
                shell_path(parent)
            )))
            .build()
    };
    let resolved = std::fs::canonicalize(parent).map_err(|error| {
        refused(format!(
            "the directory {} for the export is not usable ({error}); the daemon creates no \
             directories, nothing was written",
            parent.display()
        ))
    })?;
    if resolved != parent {
        return Err(refused(format!(
            "the path to {} goes through a symbolic link (it resolves to {}); the daemon writes \
             an export only where the named path leads, nothing was written",
            parent.display(),
            resolved.display()
        )));
    }
    Ok(())
}

/// Eine Antwort mit Zahl und Zeitpunkt der Anker, sonst leer.
fn anchored(anchors: &[Anchor]) -> v1::AuditResponse {
    v1::AuditResponse {
        anchors: u64::try_from(anchors.len()).unwrap_or(u64::MAX),
        last_anchor_at: anchors
            .iter()
            .max_by_key(|anchor| anchor.seq)
            .and_then(|anchor| DateTime::parse_from_rfc3339(&anchor.ts).ok())
            .map(|at| prost_types::Timestamp {
                seconds: at.timestamp(),
                nanos: i32::try_from(at.timestamp_subsec_nanos()).unwrap_or(0),
            }),
        anchors_reported: true,
        ..v1::AuditResponse::default()
    }
}

/// Liest eine Anfrage.
///
/// # Errors
///
/// `AUDIT_009` mit dem Grund; siehe Modulkommentar.
pub fn parse(request: &v1::AuditRequest) -> Result<AuditOp, Diagnostic> {
    use v1::audit_request::Op;
    match request.op.as_ref() {
        None => Err(invalid(
            "the request names no operation; Audit takes verify, head, query or export".to_owned(),
        )),
        Some(Op::Verify(())) => Ok(AuditOp::Verify),
        Some(Op::Head(())) => Ok(AuditOp::Head),
        Some(Op::Query(query)) => {
            let before = match query.cursor.as_str() {
                "" => None,
                text => Some(text.parse::<u64>().map_err(|_| {
                    invalid(format!(
                        "the cursor {text:?} is not one this daemon handed out; start again \
                         with an empty cursor"
                    ))
                })?),
            };
            Ok(AuditOp::Query {
                filter: QueryFilter {
                    kind_prefix: query.kind_prefix.clone(),
                    session: query.session.clone(),
                    range: range(query.from.as_ref(), query.to.as_ref())?,
                },
                limit: usize::try_from(query.limit).unwrap_or(usize::MAX),
                before,
            })
        }
        Some(Op::Export(export)) => {
            let format = ExportFormat::parse(&export.format).ok_or_else(|| {
                invalid(format!(
                    "{:?} is not an export format; Audit(Export) writes jsonl or csv",
                    export.format
                ))
            })?;
            let out = PathBuf::from(&export.out_path);
            if !out.is_absolute() {
                return Err(invalid(format!(
                    "the export path {:?} is not absolute; the daemon runs in another directory \
                     than its caller and would write somewhere else",
                    export.out_path
                )));
            }
            if export.redact_hosts {
                // Still ignoriert, stünden die Hosts im Export, die jemand
                // geschwärzt haben wollte.
                return Err(invalid(
                    "redact_hosts is not supported yet; nothing was written, and no export \
                     leaves out hosts that it would name"
                        .to_owned(),
                ));
            }
            Ok(AuditOp::Export {
                format,
                out,
                range: range(export.from.as_ref(), export.to.as_ref())?,
            })
        }
    }
}

/// Ein Zeitraum aus zwei Zeitpunkten der Leitung.
fn range(
    from: Option<&prost_types::Timestamp>,
    to: Option<&prost_types::Timestamp>,
) -> Result<TimeRange, Diagnostic> {
    Ok(TimeRange::new(
        from.map(instant).transpose()?,
        to.map(instant).transpose()?,
    ))
}

/// Ein Zeitpunkt der Leitung als UTC.
fn instant(at: &prost_types::Timestamp) -> Result<DateTime<Utc>, Diagnostic> {
    u32::try_from(at.nanos)
        .ok()
        .and_then(|nanos| DateTime::from_timestamp(at.seconds, nanos))
        .ok_or_else(|| {
            invalid(format!(
                "the time {}s + {}ns is not a point in time this daemon can compare",
                at.seconds, at.nanos
            ))
        })
}

/// `AUDIT_009`: Die Anfrage gilt so nicht; es ist nichts geschehen.
fn invalid(why: String) -> Diagnostic {
    Diagnostic::builder(codes::AUDIT_009, Severity::Error)
        .why(why)
        .fix(FixAction::CopyCommand("humanitl audit --help".to_owned()))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{AuditOp, parse};
    use crate::v1;
    use crate::v1::audit_request::{Export, Op, Query};

    fn request(op: Op) -> v1::AuditRequest {
        v1::AuditRequest { op: Some(op) }
    }

    /// Jede Anfrage, die nicht gilt, ist `AUDIT_009` und keine Wirkung.
    #[test]
    fn a_request_that_does_not_hold_is_audit_009() {
        let export = |format: &str, out: &str| {
            request(Op::Export(Export {
                format: format.to_owned(),
                out_path: out.to_owned(),
                ..Export::default()
            }))
        };
        let refused = [
            v1::AuditRequest { op: None },
            export("xml", "/tmp/a.xml"),
            export("csv", "relative.csv"),
            export("csv", ""),
            request(Op::Export(Export {
                format: "csv".to_owned(),
                out_path: "/tmp/a.csv".to_owned(),
                redact_hosts: true,
                ..Export::default()
            })),
            request(Op::Query(Query {
                cursor: "page-2".to_owned(),
                ..Query::default()
            })),
            request(Op::Query(Query {
                from: Some(prost_types::Timestamp {
                    seconds: 0,
                    nanos: -1,
                }),
                ..Query::default()
            })),
        ];
        for wire in refused {
            let diagnostic = parse(&wire).expect_err("the request must be refused");
            assert_eq!(diagnostic.code.as_str(), "AUDIT_009", "{wire:?}");
            assert!(!diagnostic.why.is_empty());
        }
    }

    /// Die Zeile, die systemd für `PrivateTmp=yes` in `mountinfo` schreibt,
    /// und die eines gewöhnlichen `tmpfs` auf `/tmp` (Debian 13).
    const PRIVATE: &str = "33 25 0:30 / / rw,relatime shared:1 - ext4 /dev/sda1 rw\n\
        412 33 0:30 /tmp/systemd-private-4f1e-humanitld.service-Q2c3/tmp /tmp rw,nosuid \
        shared:210 - ext4 /dev/sda1 rw\n\
        413 33 0:30 /var/tmp/systemd-private-4f1e-humanitld.service-X1a9/tmp /var/tmp rw \
        shared:211 - ext4 /dev/sda1 rw\n";
    const SHARED: &str = "33 25 0:30 / / rw,relatime shared:1 - ext4 /dev/sda1 rw\n\
        44 33 0:39 / /tmp rw,nosuid,nodev shared:23 - tmpfs tmpfs rw\n";

    /// Unter `PrivateTmp` wird ein Export nach `/tmp` oder `/var/tmp`
    /// abgelehnt, mit einem Ziel unter `$HOME` als Vorschlag; ohne es nicht,
    /// auch nicht bei einem eigenen `tmpfs` auf `/tmp`.
    #[test]
    fn an_export_into_a_private_tmp_is_audit_008() {
        use std::path::Path;

        for out in ["/tmp/audit.jsonl", "/var/tmp/x/audit.csv"] {
            let refused = super::refuse_private_tmp(Path::new(out), PRIVATE)
                .expect_err("a private /tmp is refused");
            assert_eq!(refused.code.as_str(), "AUDIT_008");
            assert!(refused.why.contains("PrivateTmp"), "{}", refused.why);
            assert!(
                matches!(&refused.fix, Some(humanitl_core::FixAction::CopyCommand(c)) if c.contains("~/")),
                "{:?}",
                refused.fix
            );
        }
        assert!(super::refuse_private_tmp(Path::new("/home/n/audit.jsonl"), PRIVATE).is_ok());
        assert!(super::refuse_private_tmp(Path::new("/tmpfoo/audit.jsonl"), PRIVATE).is_ok());
        assert!(super::refuse_private_tmp(Path::new("/tmp/audit.jsonl"), SHARED).is_ok());
        assert!(super::refuse_private_tmp(Path::new("/tmp/audit.jsonl"), "").is_ok());
    }

    /// Ein Verweis im Weg zum Ziel und ein fehlendes Verzeichnis sind
    /// `AUDIT_008`; ein echtes Verzeichnis nicht.
    #[test]
    fn a_linked_or_missing_parent_is_audit_008() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let real = root.join("real");
        std::fs::create_dir(&real).unwrap();
        let link = root.join("exports");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert!(super::refuse_linked_parent(&real.join("a.jsonl")).is_ok());
        let linked = super::refuse_linked_parent(&link.join("a.jsonl")).unwrap_err();
        assert_eq!(linked.code.as_str(), "AUDIT_008");
        assert!(linked.why.contains("symbolic link"), "{}", linked.why);
        let missing = super::refuse_linked_parent(&root.join("gone/a.jsonl")).unwrap_err();
        assert_eq!(missing.code.as_str(), "AUDIT_008");
        assert!(!root.join("gone").exists(), "no directory is created");
    }

    #[test]
    fn a_cursor_is_the_number_below_which_the_next_page_starts() {
        let op = parse(&request(Op::Query(Query {
            cursor: "150".to_owned(),
            limit: 20,
            kind_prefix: "flow.".to_owned(),
            ..Query::default()
        })))
        .unwrap();
        let AuditOp::Query {
            filter,
            limit,
            before,
        } = op
        else {
            panic!("a query");
        };
        assert_eq!(before, Some(150));
        assert_eq!(limit, 20);
        assert_eq!(filter.kind_prefix, "flow.");
        assert!(!filter.range.is_bounded());
    }

    /// Eine gekürzte Kette hält, und die Warnung `pruned` trägt in `records`
    /// die Nummer des letzten gelöschten Records (HUM-157).
    #[test]
    fn a_pruned_chain_is_ok_with_the_warning_pruned() {
        use humanitl_audit::{Head, VerifyReport, VerifyStatus, VerifyWarning};

        let report = VerifyReport {
            records: 5,
            status: VerifyStatus::Ok,
            warnings: vec![VerifyWarning::Pruned { through_seq: 10 }],
            head: Some(Head {
                seq: 15,
                hash: "ab".repeat(32),
            }),
        };
        let answer = super::verify_response(&report, std::path::Path::new("/x/audit.jsonl"), &[]);
        assert!(answer.ok);
        assert!(answer.diagnostic.is_none());
        assert_eq!(
            answer.warnings,
            [v1::AuditWarning {
                kind: "pruned".to_owned(),
                records: 10,
            }]
        );
    }

    /// Der Vorschlag nennt das Zielverzeichnis aus seinen Bytes (HUM-215).
    #[test]
    fn a_missing_export_directory_is_named_from_its_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;
        use std::path::Path;

        let fix = |out: &Path| {
            super::refuse_linked_parent(out)
                .expect_err("the directory does not exist")
                .fix
        };
        assert_eq!(
            fix(Path::new("/nonexistent-hum215/Audit 2026/a.jsonl")),
            Some(humanitl_core::FixAction::CopyCommand(
                "ls -ld '/nonexistent-hum215/Audit 2026'".to_owned()
            ))
        );
        assert_eq!(
            fix(Path::new(OsStr::from_bytes(
                b"/nonexistent-hum215/a\xff/a.jsonl"
            ))),
            Some(humanitl_core::FixAction::CopyCommand(
                r"ls -ld $'/nonexistent-hum215/a\xff'".to_owned()
            ))
        );
    }
}
