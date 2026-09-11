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

/// Das Ergebnis einer Prüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// Wie viele Records bestanden haben.
    pub records: u64,
    /// Ob die Kette hält, und wenn nicht, wo sie bricht.
    pub status: VerifyStatus,
    /// Was die Prüfung nicht beweisen konnte.
    pub warnings: Vec<VerifyWarning>,
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
        let result = match File::open(path) {
            Ok(file) => Self::verify_reader(BufReader::new(file), hmac_key, anchors),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Self::verify_reader(io::empty(), hmac_key, anchors)
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
        mut reader: impl BufRead,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
    ) -> io::Result<VerifyReport> {
        let mut by_seq: BTreeMap<u64, Vec<&str>> = BTreeMap::new();
        for anchor in anchors {
            by_seq.entry(anchor.seq).or_default().push(&anchor.hash);
        }
        let mut warnings = Vec::new();
        if hmac_key.is_none() {
            warnings.push(VerifyWarning::NoHmacKey);
        }

        let mut records = 0_u64;
        let mut last_seq = 0_u64;
        let mut last_hash = GENESIS_PREV.to_owned();
        let mut buffer = Vec::with_capacity(1024);
        loop {
            buffer.clear();
            if reader.read_until(b'\n', &mut buffer)? == 0 {
                break;
            }
            let broken = |first_bad_seq, reason| VerifyReport {
                records,
                status: VerifyStatus::Broken {
                    first_bad_seq,
                    reason,
                },
                warnings: warnings.clone(),
            };
            // Eine letzte Zeile ohne `\n` ist unvollständig: Das Format endet
            // jede Zeile mit einem Umbruch.
            let Some(line) = buffer.strip_suffix(b"\n") else {
                return Ok(broken(last_seq + 1, BreakReason::NonCanonicalLine));
            };
            match check_line(line, last_seq, &last_hash, hmac_key, &by_seq) {
                Ok((seq, hash)) => {
                    records += 1;
                    last_seq = seq;
                    last_hash = hash;
                }
                Err((first_bad_seq, reason)) => return Ok(broken(first_bad_seq, reason)),
            }
        }

        if let Some((&anchor_seq, _)) = by_seq.range(last_seq + 1..).next() {
            return Ok(VerifyReport {
                records,
                status: VerifyStatus::Broken {
                    first_bad_seq: last_seq,
                    reason: BreakReason::TruncatedBelowAnchor { anchor_seq },
                },
                warnings,
            });
        }
        let anchored = by_seq
            .range(..=last_seq)
            .next_back()
            .map_or(0, |(&seq, _)| seq);
        if last_seq > anchored {
            warnings.push(VerifyWarning::UnanchoredTail {
                records: last_seq - anchored,
            });
        }
        Ok(VerifyReport {
            records,
            status: VerifyStatus::Ok,
            warnings,
        })
    }
}

/// Prüft eine Zeile und liefert Nummer und Hash, oder die Nummer und den
/// Grund des Bruchs.
fn check_line(
    line: &[u8],
    last_seq: u64,
    last_hash: &str,
    hmac_key: Option<&[u8; 32]>,
    anchors: &BTreeMap<u64, Vec<&str>>,
) -> Result<(u64, String), (u64, BreakReason)> {
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
    Ok((seq, record.hash))
}
