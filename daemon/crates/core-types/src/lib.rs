//! Werttypen des Kerns: typisierte IDs, Flow-Zustandsautomat, Regeln, Findings,
//! Diagnostics. Kein IO, kein async, kein Protobuf.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Aufbau:
//!
//! - [`ids`] typisierte Identifikatoren (UUID v7)
//! - [`host`] Normalisierung von Hostnamen
//! - [`http`] Anfrage, Authority, Body-Verweis
//! - [`finding`] Funde der Detektoren
//! - [`rule`] Regeln als reine Werttypen
//! - [`flow`] Zustandsautomat und Entscheidungen
//! - [`event`] der Ereignisstrom
//! - [`block`] die Antwort an einen geblockten Client
//! - [`diagnostics`] Befunde mit Code, Grund und Behebung
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod hex;

pub mod block;
pub mod diagnostics;
pub mod event;
pub mod finding;
pub mod flow;
pub mod host;
pub mod http;
pub mod ids;
pub mod rule;

/// Zweiter Name für [`diagnostics`], wie ihn HUM-063 verwendet.
pub use crate::diagnostics as diag;

pub use crate::block::{
    BlockResponse, block_response, failed_response, note_header_value, sanitize_note,
};
pub use crate::diagnostics::{
    Diagnostic, DiagnosticBuilder, DiagnosticCode, FixAction, Severity, lookup,
};
pub use crate::event::FlowEvent;
pub use crate::finding::{Finding, FindingKind, FindingLocation, Tier};
pub use crate::flow::{
    BlockReason, Decision, DecisionSource, Flow, FlowState, InvalidTransition, Transition,
    TransitionInput, UpstreamError,
};
pub use crate::host::{HostName, HostParseError, ip_is_private};
pub use crate::http::{
    Authority, BodyRef, HeaderMap, HeaderName, HttpRequest, Method, Scheme, Upgrade, Version,
};
pub use crate::ids::{FlowId, IdParseError, RuleId, SandboxId, SessionId};
pub use crate::rule::{Action, Expiry, HostPattern, Matcher, PathPattern, Rule};
