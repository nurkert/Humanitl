//! Der Adapter des Audit-Logs: die Datei `audit.jsonl`, ihr Schreiber und die
//! Datei des Schlüssels (HUM-050, als eigene Crate seit HUM-155).
//!
//! `humanitl-audit` ist Kern und öffnet keine Datei (`docs/ARCHITECTURE.md`
//! 2). Was die Kette bedeutet, steht dort: Kanonisierung, Record, Arten,
//! Prüfung über einen Leser, die Formate des Exports und die Grenzen der
//! Aufbewahrung. Hier steht alles, was dafür eine Datei öffnet, sperrt,
//! synchronisiert oder umbenennt:
//!
//! - [`writer`] der eine Schreiber mit fsync-Politik, Sperre und Wiederaufnahme
//! - [`key`] die Datei des HMAC-Schlüssels, bis HUM-048 den Keyring bringt
//! - [`verify`] die Prüfung einer Kette, die als Datei vorliegt
//! - [`query`] Ende und Seiten der Kette aus der Datei
//! - [`export`] der Export in eine Datei, der nie etwas überschreibt
//! - [`retention`] Schnitt und Umschreiben der Datei bei der Aufbewahrung
//!
//! Kein neuer Port (HUM-155, Nicht-Ziel): Der Schreiber bleibt der einzige
//! Adapter. Die beiden Erweiterungs-Traits [`AuditKeyFile`] und
//! [`VerifyFile`] halten nur die Aufrufe `AuditKey::load_or_create_file` und
//! `AuditVerifier::verify` an ihrer Stelle.
//!
//! Siehe `backlog/CONVENTIONS.md` Abschnitt 3.1 für die erlaubten
//! Abhängigkeiten dieser Crate.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod export;
pub mod key;
pub mod query;
pub mod retention;
pub mod verify;
pub mod writer;

pub use crate::key::AuditKeyFile;
pub use crate::verify::VerifyFile;
pub use crate::writer::{AnchorMirror, AuditHandle, AuditWriter, Head, WriterOptions};
