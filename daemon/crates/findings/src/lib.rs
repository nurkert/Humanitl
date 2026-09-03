//! Detektoren für Secrets und personenbezogene Daten. Reine Funktionen über Bytes.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Der Mensch entscheidet, aber ein Schlüssel in einem 40-KB-JSON fällt ihm
//! nicht auf. Diese Crate zerlegt eine Anfrage in Suchziele (jeder Header, die
//! Query, der Body), lässt die Tier-1-Detektoren darüber laufen und liefert
//! [`Finding`](humanitl_core::Finding)s mit Byte-Bereichen zurück (HUM-025).
//!
//! Drei Eigenschaften, die diese Crate garantiert:
//!
//! - **Kein Wert verlässt die Crate.** Ein [`Finding`](humanitl_core::Finding)
//!   trägt Hash, Ort, Bereich und einen maskierten Anfang für die Anzeige, nie
//!   den gefundenen Text. Wer den Wert braucht, schneidet ihn selbst aus dem
//!   Body; Meldungen und Diagnostics bekommen ihn nie.
//! - **Der Hash ist stabil.** `value_hash` ist SHA-256 über genau die Bytes des
//!   Treffers. Derselbe Wert an derselben Stelle ergibt immer denselben Hash,
//!   damit `findings.ignored_hashes` über Sitzungen hinweg trägt.
//! - **Die Laufzeit bleibt linear.** Alle Muster laufen über `regex::bytes`
//!   (endliche Automaten, kein Backtracking) mit gesetztem `size_limit`, die
//!   Prüfsummen-Detektoren prüfen jeden Kandidaten in konstanter Zeit, und die
//!   Eingabe ist auf `limits.preview_cap_bytes` gedeckelt. Ein feindseliger
//!   Body kostet damit höchstens `O(cap)`.
//!
//! Aufbau:
//!
//! - [`settings`] die Einstellungen, die der Scan braucht
//! - [`content_type`] der `Content-Type` als Wert
//! - [`decode`] Budget, Prozent-Dekodierung, Body-Dekodierung
//! - [`input`] Zerlegung einer Anfrage in Suchziele
//! - [`registry`] der `Detector`-Trait und die Registry
//! - [`display`] der maskierte Anfang eines Funds
//! - [`detectors`] die Tier-1-Detektoren
//!
//! ```
//! use humanitl_core::{Authority, HostName, HttpRequest, Method, Scheme};
//! use humanitl_findings::{DetectorRegistry, FindingsSettings};
//!
//! let settings = FindingsSettings::default();
//! let registry = DetectorRegistry::tier1(&settings)?;
//! let host = HostName::Dns("api.example.com".to_owned());
//! let request = HttpRequest::new(
//!     Method::POST,
//!     Scheme::Https,
//!     Authority::with_scheme(host, Scheme::Https),
//!     "/v1/chat",
//! );
//! let findings = registry.scan_request(&request, b"iban GB82 WEST 1234 5698 7654 32");
//!
//! assert_eq!(findings.len(), 1);
//! assert_eq!(findings[0].display_prefix, "GB82 …");
//! # Ok::<(), humanitl_core::Diagnostic>(())
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod content_type;
pub mod decode;
pub mod detectors;
pub mod display;
pub mod input;
pub mod registry;
pub mod settings;

pub use crate::content_type::ContentType;
pub use crate::decode::{
    Budget, ContentEncoding, DecodedBody, InflateError, Inflated, percent_decode,
};
pub use crate::display::display_prefix;
pub use crate::input::{ScanInput, ScanTarget, ScanTargets, SpanMap};
pub use crate::registry::{Detector, DetectorRegistry, ScanReport, TIER1_DETECTOR_IDS};
pub use crate::settings::FindingsSettings;
