//! Die Prüfung der Kette (HUM-050).
//!
//! Zeile für Zeile, und die erste Abweichung beendet die Prüfung:
//!
//! 1. Die Zeile ist JSON und hat die acht Felder eines Records.
//! 2. Sie ist Byte für Byte ihre eigene kanonische Form, sonst
//!    [`BreakReason::NonCanonicalLine`].
//! 3. `seq` folgt lückenlos auf den Vorgänger, sonst [`BreakReason::SeqGap`].
//! 4. `prev` ist der Hash des Vorgängers, sonst [`BreakReason::PrevMismatch`].
//! 5. `hash` stimmt, sonst [`BreakReason::HashMismatch`].
//! 6. Mit Schlüssel: `mac` stimmt, sonst [`BreakReason::MacMismatch`]; ohne
//!    Schlüssel die Warnung [`VerifyWarning::NoHmacKey`].
//! 7. Jeder Anker mit dieser Nummer trägt denselben Hash, sonst
//!    [`BreakReason::AnchorMismatch`].
//!
//! Nach der letzten Zeile: Ein Anker jenseits des Endes heißt, die Datei wurde
//! unter ihn gekürzt ([`BreakReason::TruncatedBelowAnchor`]). Records hinter
//! dem letzten Anker sind [`VerifyWarning::UnanchoredTail`] — sie könnten
//! fehlen, ohne dass es jemand merkt, und genau das ist die dokumentierte
//! Grenze (`docs/SECURITY.md`, „Was die Audit-Kette beweist").
//!
//! **Ein dokumentierter Anfang** (HUM-157). Beginnt die Datei nicht bei
//! Nummer 1, gilt ihr erster Record vorläufig als Anfang: Seine Nummer minus
//! eins und sein `prev` sind der Schnitt. Ein Anker unter genau dieser Nummer
//! muss denselben Hash nennen. Nach der letzten Zeile muss ein `audit.pruned`
//! in der Kette stehen, der genau diesen Schnitt nennt; dann hält die Kette,
//! mit der Warnung [`VerifyWarning::Pruned`]. Fehlt er, ist der Anfang eine
//! Lücke wie jede andere: [`BreakReason::SeqGap`] am ersten Record, und keiner
//! davor hat bestanden. Ein Anfang, den niemand dokumentiert hat, sieht damit
//! genauso aus wie vor HUM-157. Mit `until` liest die Prüfung über das
//! gemeldete Ende hinaus weiter, bis sie den dokumentierenden Record gefunden
//! hat: Ein Lauf der Aufbewahrung kann zwischen dem Melden des Endes und dem
//! Lesen die Datei ersetzt haben, und sein `audit.pruned` steht dann hinter
//! dem gemeldeten Ende.
//!
//! **Welche Nummer ein Bruch nennt.** `first_bad_seq` ist die Nummer des
//! ersten Records, der nicht besteht, so wie sie in ihm steht. Fehlt Record 4,
//! nennt die Prüfung 5: Record 5 ist der erste, der nicht passt. Ist eine
//! Zeile gar kein Record oder nicht kanonisch, gibt es keine Nummer, der zu
//! trauen wäre; dann steht dort die erwartete (`last_seq + 1`). Ist die Datei
//! gekürzt, steht dort die letzte vorhandene.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use humanitl_core::diagnostics::codes::{AUDIT_001, AUDIT_006};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::Anchor;
use crate::key::shell_quote;
use crate::record::{AuditRecord, GENESIS_PREV, mac_matches};
use crate::retention::AuditPruned;
use crate::writer::Head;

/// Das Ergebnis einer Prüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// Wie viele Records bestanden haben.
    pub records: u64,
    /// Ob die Kette hält, und wenn nicht, wo sie bricht.
    pub status: VerifyStatus,
    /// Was die Prüfung nicht beweisen konnte.
    pub warnings: Vec<VerifyWarning>,
    /// Der letzte Record, der bestanden hat; `None`, wenn keiner bestand.
    ///
    /// Bei einer Kette, die hält, ist das ihr Ende. Oberfläche und
    /// Kommandozeile zeigen seinen Hash als Kopf der Kette (HUM-156).
    pub head: Option<Head>,
}

/// Ob die Kette hält.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyStatus {
    /// Jeder Record besteht, und kein Anker liegt jenseits des Endes.
    Ok,
    /// Die Kette bricht.
    Broken {
        /// Die erste Nummer, die nicht besteht (siehe Modulkommentar).
        first_bad_seq: u64,
        /// Warum.
        reason: BreakReason,
    },
}

/// Warum die Kette bricht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakReason {
    /// Eine Nummer fehlt oder steht an der falschen Stelle.
    SeqGap,
    /// `prev` ist nicht der Hash des Vorgängers.
    PrevMismatch,
    /// Der Hash passt nicht zu den Feldern.
    HashMismatch,
    /// Der MAC passt nicht zum Hash; ein Neuaufbau ohne Schlüssel.
    MacMismatch,
    /// Die Zeile ist kein Record oder nicht ihre kanonische Form.
    NonCanonicalLine,
    /// Der Anker in `audit_anchors` nennt für diese Nummer einen anderen Hash.
    AnchorMismatch {
        /// Die Nummer des Ankers.
        anchor_seq: u64,
    },
    /// Die Datei endet vor einem Anker.
    TruncatedBelowAnchor {
        /// Der erste Anker jenseits des Endes.
        anchor_seq: u64,
    },
}

impl BreakReason {
    /// Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SeqGap => "seq_gap",
            Self::PrevMismatch => "prev_mismatch",
            Self::HashMismatch => "hash_mismatch",
            Self::MacMismatch => "mac_mismatch",
            Self::NonCanonicalLine => "non_canonical_line",
            Self::AnchorMismatch { .. } => "anchor_mismatch",
            Self::TruncatedBelowAnchor { .. } => "truncated_below_anchor",
        }
    }
}

/// Was die Prüfung nicht beweisen konnte, obwohl die Kette hält.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyWarning {
    /// Ohne Schlüssel sind die MACs ungeprüft; ein Neuaufbau der ganzen Kette
    /// fiele nicht auf.
    NoHmacKey,
    /// So viele Records stehen hinter dem letzten Anker; ihr Fehlen fiele
    /// nicht auf.
    UnanchoredTail {
        /// Wie viele.
        records: u64,
    },
    /// Die Records bis einschließlich `through_seq` hat ein Lauf von
    /// `audit.retention_days` gelöscht, und ein `audit.pruned` in der Kette
    /// dokumentiert es (HUM-157). Was in ihnen stand, beweist die Kette nicht
    /// mehr.
    Pruned {
        /// Die Nummer des letzten gelöschten Records.
        through_seq: u64,
    },
}

impl VerifyReport {
    /// Wahr, wenn die Kette hält.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(self.status, VerifyStatus::Ok)
    }

    /// Der Befund zu einer gebrochenen Kette ([`AUDIT_001`]); `None`, wenn sie
    /// hält.
    #[must_use]
    pub fn diagnostic(&self, path: &Path) -> Option<Diagnostic> {
        let VerifyStatus::Broken {
            first_bad_seq,
            reason,
        } = self.status
        else {
            return None;
        };
        let detail = match reason {
            BreakReason::AnchorMismatch { anchor_seq } => {
                format!("the anchor at seq {anchor_seq} names another hash")
            }
            BreakReason::TruncatedBelowAnchor { anchor_seq } => {
                format!("the log ends at seq {first_bad_seq}, below the anchor at seq {anchor_seq}")
            }
            other => format!("{} at seq {first_bad_seq}", other.as_str()),
        };
        Some(
            Diagnostic::builder(AUDIT_001, Severity::Error)
                .why(format!(
                    "{}: {detail}; {} records before it hold",
                    path.display(),
                    self.records
                ))
                .fix(set_aside_fix(path))
                .build(),
        )
    }
}

/// Der Vorschlag für eine gebrochene Kette: die Datei samt Zeitstempel
/// beiseitelegen. Sie bleibt als Beleg liegen.
pub(crate) fn set_aside_fix(path: &Path) -> FixAction {
    let quoted = shell_quote(&path.display().to_string());
    FixAction::CopyCommand(format!(
        "mv {quoted} {quoted}.broken-$(date -u +%Y%m%dT%H%M%SZ)"
    ))
}

/// Prüft eine Kette.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditVerifier;

impl AuditVerifier {
    /// Prüft die Datei `path` gegen `anchors`, mit Schlüssel, wenn einer da ist.
    ///
    /// Eine fehlende Datei ist eine leere Kette; gibt es Anker, ist sie unter
    /// sie gekürzt.
    ///
    /// # Errors
    ///
    /// [`AUDIT_006`], wenn sich die Datei nicht lesen lässt. Eine Kette, die
    /// niemand lesen konnte, ist nicht geprüft, und ein `Ok` wäre gelogen.
    pub fn verify(
        path: &Path,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
    ) -> Result<VerifyReport, Diagnostic> {
        Self::verify_until(path, hmac_key, anchors, None)
    }

    /// Wie [`AuditVerifier::verify`], aber nur bis zum Record mit der Nummer
    /// `until`, dem Ende, das der Schreiber zuletzt gemeldet hat (HUM-156).
    ///
    /// Ein Daemon, der seine eigene, laufende Kette prüft, liest sie, während
    /// der Schreiber weiter anhängt. Was hinter `until` steht, ist noch nicht
    /// geschrieben und kein Bruch: Eine halbe Zeile dort ist eine, die gerade
    /// entsteht. Anker jenseits von `until` zählen aus demselben Grund nicht.
    /// `None` liest bis zum Ende der Datei.
    ///
    /// # Errors
    ///
    /// Wie [`AuditVerifier::verify`].
    pub fn verify_until(
        path: &Path,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
        until: Option<u64>,
    ) -> Result<VerifyReport, Diagnostic> {
        let result = match File::open(path) {
            Ok(file) => Self::verify_reader_until(BufReader::new(file), hmac_key, anchors, until),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Self::verify_reader_until(io::empty(), hmac_key, anchors, until)
            }
            Err(err) => Err(err),
        };
        result.map_err(|err| {
            Diagnostic::builder(AUDIT_006, Severity::Error)
                .why(format!(
                    "cannot read {} to verify it: {err}",
                    path.display()
                ))
                .fix(FixAction::CopyCommand(format!(
                    "ls -ln {}",
                    shell_quote(&path.display().to_string())
                )))
                .build()
        })
    }

    /// Wie [`AuditVerifier::verify`], über einem beliebigen Leser.
    ///
    /// # Errors
    ///
    /// Der Fehler des Lesers.
    pub fn verify_reader(
        reader: impl BufRead,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
    ) -> io::Result<VerifyReport> {
        Self::verify_reader_until(reader, hmac_key, anchors, None)
    }

    /// Wie [`AuditVerifier::verify_until`], über einem beliebigen Leser.
    ///
    /// # Errors
    ///
    /// Der Fehler des Lesers.
    pub fn verify_reader_until(
        mut reader: impl BufRead,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
        until: Option<u64>,
    ) -> io::Result<VerifyReport> {
        let mut by_seq: BTreeMap<u64, Vec<&str>> = BTreeMap::new();
        for anchor in anchors
            .iter()
            .filter(|anchor| until.is_none_or(|until| anchor.seq <= until))
        {
            by_seq.entry(anchor.seq).or_default().push(&anchor.hash);
        }
        let mut warnings = Vec::new();
        if hmac_key.is_none() {
            warnings.push(VerifyWarning::NoHmacKey);
        }

        let mut records = 0_u64;
        let mut last_seq = 0_u64;
        let mut last_hash = GENESIS_PREV.to_owned();
        // Der Schnitt eines Anfangs hinter Nummer 1, und ob ein
        // `audit.pruned` ihn schon dokumentiert hat (siehe Modulkommentar).
        let mut start: Option<Start> = None;
        let mut buffer = Vec::with_capacity(1024);
        loop {
            let past_until = until.is_some_and(|until| last_seq >= until);
            if past_until && start.as_ref().is_none_or(|start| start.documented) {
                // Das gemeldete Ende ist erreicht; der Rest entsteht gerade.
                break;
            }
            buffer.clear();
            if reader.read_until(b'\n', &mut buffer)? == 0 {
                break;
            }
            if records == 0
                && let Some(first) = cut_start(&buffer)
            {
                // Der Anfang einer gekürzten Kette. Ein Anker auf dem Schnitt
                // nennt den Hash des letzten gelöschten Records.
                if by_seq.get(&first.through_seq).is_some_and(|hashes| {
                    hashes
                        .iter()
                        .any(|anchored| *anchored != first.through_hash)
                }) {
                    return Ok(VerifyReport {
                        records,
                        status: VerifyStatus::Broken {
                            first_bad_seq: first.through_seq + 1,
                            reason: BreakReason::AnchorMismatch {
                                anchor_seq: first.through_seq,
                            },
                        },
                        warnings,
                        head: None,
                    });
                }
                last_seq = first.through_seq;
                last_hash.clone_from(&first.through_hash);
                start = Some(first);
            }
            let broken = |first_bad_seq, reason| match start.as_ref() {
                // Ein Anfang, den noch kein Record dokumentiert hat, ist der
                // erste Befund, auch wenn danach noch einer käme.
                Some(start) if !start.documented => undocumented(start, warnings.clone()),
                _ => VerifyReport {
                    records,
                    status: VerifyStatus::Broken {
                        first_bad_seq,
                        reason,
                    },
                    warnings: warnings.clone(),
                    head: head_of(records, last_seq, &last_hash),
                },
            };
            // Eine letzte Zeile ohne `\n` ist unvollständig: Das Format endet
            // jede Zeile mit einem Umbruch.
            let Some(line) = buffer.strip_suffix(b"\n") else {
                if past_until {
                    // Hinter dem gemeldeten Ende entsteht diese Zeile gerade.
                    break;
                }
                return Ok(broken(last_seq + 1, BreakReason::NonCanonicalLine));
            };
            match check_line(line, last_seq, &last_hash, hmac_key, &by_seq) {
                Ok(record) => {
                    if let Some(start) = start.as_mut().filter(|start| !start.documented) {
                        start.documented =
                            AuditPruned::documents(&record, start.through_seq, &start.through_hash);
                    }
                    records += 1;
                    last_seq = record.body.seq;
                    last_hash = record.hash;
                }
                Err((first_bad_seq, reason)) => return Ok(broken(first_bad_seq, reason)),
            }
        }

        Ok(conclude(
            Tail {
                records,
                last_seq,
                last_hash,
            },
            start,
            &by_seq,
            warnings,
        ))
    }
}

/// Wo die Prüfung nach der letzten Zeile steht.
struct Tail {
    /// Wie viele Records bestanden haben.
    records: u64,
    /// Die Nummer des letzten.
    last_seq: u64,
    /// Sein Hash.
    last_hash: String,
}

/// Das Urteil nach der letzten Zeile: ein Anfang ohne Beleg, ein Anker hinter
/// dem Ende, oder eine Kette, die hält, mit ihren Warnungen.
fn conclude(
    tail: Tail,
    start: Option<Start>,
    by_seq: &BTreeMap<u64, Vec<&str>>,
    mut warnings: Vec<VerifyWarning>,
) -> VerifyReport {
    let Tail {
        records,
        last_seq,
        last_hash,
    } = tail;
    let mut cut = 0_u64;
    if let Some(start) = start {
        if !start.documented {
            return undocumented(&start, warnings);
        }
        cut = start.through_seq;
        warnings.push(VerifyWarning::Pruned {
            through_seq: start.through_seq,
        });
    }

    if let Some((&anchor_seq, _)) = by_seq.range(last_seq + 1..).next() {
        return VerifyReport {
            records,
            status: VerifyStatus::Broken {
                first_bad_seq: last_seq,
                reason: BreakReason::TruncatedBelowAnchor { anchor_seq },
            },
            warnings,
            head: head_of(records, last_seq, &last_hash),
        };
    }
    // Ein Anker unter dem Schnitt verankert keinen Record, der noch da ist.
    let anchored = by_seq
        .range(..=last_seq)
        .next_back()
        .map_or(0, |(&seq, _)| seq)
        .max(cut);
    if last_seq > anchored {
        warnings.push(VerifyWarning::UnanchoredTail {
            records: last_seq - anchored,
        });
    }
    VerifyReport {
        records,
        status: VerifyStatus::Ok,
        warnings,
        head: head_of(records, last_seq, &last_hash),
    }
}

/// Der Schnitt am Anfang einer gekürzten Kette.
struct Start {
    /// Die Nummer des letzten gelöschten Records.
    through_seq: u64,
    /// Sein Hash, wie ihn der erste Record als `prev` nennt.
    through_hash: String,
    /// Ob ein `audit.pruned` in der Kette genau diesen Schnitt nennt.
    documented: bool,
}

/// Kein Record nennt diesen Schnitt: ein Anfang, der fehlt. Derselbe Befund
/// wie vor HUM-157, als jeder Anfang hinter Nummer 1 eine Lücke war.
fn undocumented(start: &Start, warnings: Vec<VerifyWarning>) -> VerifyReport {
    VerifyReport {
        records: 0,
        status: VerifyStatus::Broken {
            first_bad_seq: start.through_seq + 1,
            reason: BreakReason::SeqGap,
        },
        warnings,
        head: None,
    }
}

/// Der Schnitt, wenn `line` ein Record hinter Nummer 1 ist; sonst `None`, und
/// die Prüfung beginnt wie immer bei 1 und [`GENESIS_PREV`].
fn cut_start(line: &[u8]) -> Option<Start> {
    let record = AuditRecord::from_line(line.strip_suffix(b"\n")?).ok()?;
    (record.body.seq > 1).then(|| Start {
        through_seq: record.body.seq - 1,
        through_hash: record.body.prev,
        documented: false,
    })
}

/// Das Ende des geprüften Teils; `None`, solange nichts bestanden hat.
fn head_of(records: u64, last_seq: u64, last_hash: &str) -> Option<Head> {
    (records > 0).then(|| Head {
        seq: last_seq,
        hash: last_hash.to_owned(),
    })
}

/// Prüft eine Zeile und liefert den Record, oder die Nummer und den Grund des
/// Bruchs.
fn check_line(
    line: &[u8],
    last_seq: u64,
    last_hash: &str,
    hmac_key: Option<&[u8; 32]>,
    anchors: &BTreeMap<u64, Vec<&str>>,
) -> Result<AuditRecord, (u64, BreakReason)> {
    let expected = last_seq + 1;
    let record =
        AuditRecord::from_line(line).map_err(|_| (expected, BreakReason::NonCanonicalLine))?;
    match record.to_line() {
        Ok(canonical) if canonical == line => {}
        _ => return Err((expected, BreakReason::NonCanonicalLine)),
    }
    let seq = record.body.seq;
    if seq != expected {
        return Err((seq, BreakReason::SeqGap));
    }
    if record.body.prev != last_hash {
        return Err((seq, BreakReason::PrevMismatch));
    }
    let hash = record
        .body
        .hash()
        .map_err(|_| (seq, BreakReason::NonCanonicalLine))?;
    if hex::encode(hash) != record.hash {
        return Err((seq, BreakReason::HashMismatch));
    }
    if let Some(key) = hmac_key
        && !mac_matches(key, &hash, &record.mac)
    {
        return Err((seq, BreakReason::MacMismatch));
    }
    if let Some(hashes) = anchors.get(&seq)
        && hashes.iter().any(|anchored| *anchored != record.hash)
    {
        return Err((seq, BreakReason::AnchorMismatch { anchor_seq: seq }));
    }
    Ok(record)
}
