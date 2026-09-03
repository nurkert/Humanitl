//! Konfigurations-Typen mit Schema und Sichtbarkeitsstufen; speist TOML, CLI und
//! Settings-Oberfläche.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Eine Konfigurationsquelle, drei Sichtbarkeitsstufen (ADR-011). Die
//! Einstellungen sind Rust-Typen; daraus entstehen das TOML-Schema, die
//! Prüfung, der Einstellungs-Bildschirm und `docs/CONFIG.md`. Nichts davon wird
//! doppelt gepflegt.
//!
//! Aufbau:
//!
//! - [`model`] die Typen mit ihren Vorgabewerten
//! - [`tier`] die drei Sichtbarkeitsstufen
//! - [`scope`] die Vertrauensgrenze des Projekt-Profils
//! - [`schema`] das JSON-Schema und der Durchlauf durch seine Felder
//! - [`alias`] alte Schlüsselnamen, die weiter funktionieren
//! - [`mod@env`] die Umgebung als Wert statt als globaler Zustand
//! - [`paths`] die Pfade nach XDG
//! - [`mod@load`] die sechs Ebenen der Präzedenz
//! - [`origin`] die Herkunft je Feld
//!
//! ```
//! use humanitl_config::{Env, Sources, load};
//!
//! let sources = Sources::empty()
//!     .with_env(Env::from_pairs([("HUMANITL_HOLD__TIMEOUT_SECS", "42")]));
//! let resolved = load(&sources)?;
//!
//! assert_eq!(resolved.config.hold.timeout_secs, 42);
//! assert_eq!(
//!     resolved.origin("hold.timeout_secs").map(ToString::to_string),
//!     Some("env HUMANITL_HOLD__TIMEOUT_SECS".to_owned())
//! );
//! # Ok::<(), humanitl_core::Diagnostic>(())
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod alias;
pub mod env;
pub mod load;
pub mod model;
pub mod origin;
pub mod paths;
pub mod schema;
pub mod scope;
pub mod tier;
mod validate;

pub use crate::alias::{ALIASES, Alias};
pub use crate::env::Env;
pub use crate::load::{
    DEFAULT_ENV_PREFIX, ENV_SEPARATOR, PROFILE_PASSTHROUGH, PROFILE_SECTION, Sources, discover,
    discover_with, load,
};
pub use crate::model::{
    AgentBriefing, AgentRef, AskMode, Config, Experimental, FindingsConfig, HoldConfig,
    IpPreference, Language, Limits, LlmConfig, PseudonymConfig, RecorderConfig, ResolverConfig,
    SandboxRef, Theme, UiConfig, WorkMode,
};
pub use crate::origin::{Origin, Resolved};
pub use crate::paths::{APP_DIR, DIR_MODE, FILE_MODE, Paths, RuntimeDir};
pub use crate::schema::{Field, json_schema};
pub use crate::scope::{PROJECT_SCOPE_KEY, ProjectScope};
pub use crate::tier::{TIER_KEY, Tier};
