//! Append-only Audit-Log mit Hash-Kette und Prüfung (HUM-050).
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Jeder relevante Vorgang im Daemon wird ein Record in
//! `$XDG_DATA_HOME/humanitl/audit/audit.jsonl`: eine Zeile, kanonisches JSON,
//! mit dem Hash des Vorgängers, dem eigenen Hash und einem HMAC darüber. Alle
//! `audit.anchor_every` Records und beim Beenden steht ein Anker in der Datei
//! und derselbe in der Tabelle `audit_anchors` der Aufzeichnung.
//!
//! Aufbau:
//!
//! - [`canonical`] die eine Serialisierung, die je in die Datei geht
//! - [`record`] ein Record, sein Hash, sein MAC, seine Zeile
//! - [`kinds`] alle Arten von Records und was jede davon tragen darf
//! - [`key`] der HMAC-Schlüssel, bis HUM-048 den Keyring bringt eine Datei
//! - [`writer`] der eine Schreiber mit fsync-Politik, Sperre und Wiederaufnahme
//! - [`verify`] die Prüfung mit jedem Grund, aus dem eine Kette bricht
//! - [`query`] das Ende der Kette und Seiten ihrer Records, ohne Prüfung
//! - [`export`] der Export als JSONL oder CSV, der nie etwas überschreibt
//! - [`retention`] das Löschen des Anfangs nach `audit.retention_days` und
//!   der Record, der es dokumentiert
//!
//! Was die Kette beweist und was nicht, steht in `docs/SECURITY.md` unter „Was
//! die Audit-Kette beweist". Kurz: Sie zeigt eine Änderung, Löschung oder
//! Umordnung vor dem letzten Anker, solange der Angreifer den Schlüssel nicht
//! hat. Sie zeigt nicht, dass der Daemon ehrlich war, dass nichts fehlt, was
//! nie geschrieben wurde, oder dass die letzten Records hinter dem letzten
//! Anker noch da sind. Nach einem Lauf von `audit.retention_days` gilt das für
//! die Records ab dem Schnitt; über die gelöschten davor beweist sie nur, dass
//! der Daemon sie mit dem Schlüssel als gelöscht verbucht hat (HUM-157).
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod canonical;
pub mod export;
pub mod key;
pub mod kinds;
pub mod query;
pub mod record;
pub mod retention;
pub mod verify;
pub mod writer;

pub use crate::canonical::{CanonicalError, canonical_json};
pub use crate::export::{CSV_COLUMNS, ExportFormat};
pub use crate::key::AuditKey;
pub use crate::kinds::{KeyOrigin, RecordKind};
pub use crate::query::{QueryFilter, TimeRange};
pub use crate::record::{AuditRecord, GENESIS_PREV, NO_SESSION, RecordBody, format_ts, sha256_hex};
pub use crate::retention::{AuditPruned, PruneReport};
pub use crate::verify::{AuditVerifier, BreakReason, VerifyReport, VerifyStatus, VerifyWarning};
pub use crate::writer::{AnchorMirror, AuditHandle, AuditWriter, Head, WriterOptions};

/// Ein Anker: Nummer und Hash eines Records, außerhalb der Datei abgelegt.
///
/// Wer nur `audit.jsonl` ändert, scheitert an ihm; wer auch ihn ändert,
/// braucht zusätzlich den HMAC-Schlüssel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// Die Nummer des verankerten Records.
    pub seq: u64,
    /// Sein Hash.
    pub hash: String,
    /// Wann der Anker entstand, nach [`record::TS_FORMAT`].
    pub ts: String,
}
