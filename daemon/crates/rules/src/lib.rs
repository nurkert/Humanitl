//! YAML-Parser, Normalisierung und Auswertung des Regelsatzes. Reine Funktionen.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Regeln sind der einzige Weg, auf dem eine Anfrage ohne Menschen entschieden
//! wird (ADR-007). Deshalb liegt die Auswertung in einer eigenen Crate ohne IO,
//! ohne async und ohne Protobuf: sie lässt sich vollständig tabellengetrieben
//! prüfen, und die Tabelle steht in `backlog/sprint-2.md` unter HUM-022.
//!
//! Aufbau:
//!
//! - [`host`] Label-Vergleich für Host-Muster, Adressen und Netze
//! - [`path`] Glob und regulärer Ausdruck für den Pfad
//! - [`parse`] `rules.yaml` lesen und schreiben
//! - [`eval`] [`RuleSet`], [`RequestKey`] und [`Verdict`]
//!
//! Die drei Zusagen dieser Crate, die kein Compiler prüft:
//!
//! 1. Verglichen werden ganze Labels, nie Zeichenketten. `*.github.com` trifft
//!    weder `evil-github.com` noch `github.com.evil.io`.
//! 2. Eine IP-Adresse trifft nie ein Host-Glob. Wer eine Adresse meint,
//!    schreibt `ip:` oder `cidr:`.
//! 3. Ohne passende Regel gilt `ask`. Es gibt keinen Pfad, auf dem ein Fehler
//!    im Regelsatz zu einer stillen Freigabe führt.
//!
//! ```
//! use humanitl_core::{HostName, Method, Scheme, SessionId};
//! use humanitl_rules::{RequestKey, Verdict, parse_rules};
//!
//! let (rules, warnings) = parse_rules(
//!     "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"*.github.com\"\n",
//! )
//! .map_err(|diagnostics| format!("{diagnostics:?}"))?;
//! assert!(warnings.is_empty());
//!
//! let host = HostName::parse("api.github.com")?;
//! let key = RequestKey::new(&host, &Method::GET, "/repos", Scheme::Https, 443);
//! assert!(matches!(
//!     rules.evaluate(&key, chrono::Utc::now(), SessionId::new()),
//!     Verdict::Matched { .. }
//! ));
//!
//! let other = HostName::parse("evil-github.com")?;
//! let key = RequestKey::new(&other, &Method::GET, "/repos", Scheme::Https, 443);
//! assert_eq!(
//!     rules.evaluate(&key, chrono::Utc::now(), SessionId::new()),
//!     Verdict::Default
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod eval;
pub mod host;
pub mod parse;
pub mod path;

pub use crate::eval::{RequestKey, RuleSet, UnknownRule, Verdict, is_known_method};
pub use crate::host::{LabelPat, matches as host_matches};
pub use crate::parse::{RULES_VERSION, parse_rules, parse_rules_for_session, serialize_rules};
pub use crate::path::PathMatcher;
