//! Schlüssel, deren Wert über alle Ebenen nur strenger wird.
//!
//! Für die meisten Schlüssel gewinnt beim Mischen die oberste Ebene. Für die
//! Schlüssel hier gewinnt der strengste Wert, gleich aus welcher Ebene
//! (`crate::load`): Eine höhere Ebene kann ihn verschärfen, nie lockern.
//! Sonst lockerte ein Projekt-Profil, das der Agent beschreiben kann, eine
//! Umgebungsvariable oder ein Wunsch über den Start, was der Mensch in
//! `config.toml` oder eine Firmenvorgabe verlangt hat.
//!
//! - `sandbox.min_isolation` (HUM-431): `policy` < `namespaces` < `kernel`.
//! - `llm.cloud` (GitHub #113): `allow` < `block`.

use crate::model::{Config, LlmCloud, MinIsolation};

/// Die Untergrenze der Isolation (HUM-431).
pub(crate) const MIN_ISOLATION: &str = "sandbox.min_isolation";

/// Die Sperre der Cloud-Inferenz (GitHub #113).
pub(crate) const LLM_CLOUD: &str = LlmCloud::KEY;

/// Ob für `path` der strengste Wert aller Ebenen gilt.
#[must_use]
pub(crate) fn is_floor(path: &str) -> bool {
    path == MIN_ISOLATION || path == LLM_CLOUD
}

/// Wie streng ein Wert eines solchen Schlüssels ist, größer heißt strenger;
/// `None` für einen anderen Schlüssel oder einen Wert, den er nicht kennt.
#[must_use]
pub(crate) fn rank(path: &str, value: &str) -> Option<u8> {
    match path {
        MIN_ISOLATION => MinIsolation::parse(value).map(|level| level as u8),
        LLM_CLOUD => LlmCloud::parse(value).map(|value| value as u8),
        _ => None,
    }
}

/// Der Wert eines solchen Schlüssels in `config`.
#[must_use]
pub(crate) fn value_in(config: &Config, path: &str) -> Option<&'static str> {
    match path {
        MIN_ISOLATION => Some(config.sandbox.min_isolation.as_str()),
        LLM_CLOUD => Some(config.llm.cloud.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stricter_values_rank_higher() {
        assert!(rank(MIN_ISOLATION, "kernel") > rank(MIN_ISOLATION, "namespaces"));
        assert!(rank(MIN_ISOLATION, "namespaces") > rank(MIN_ISOLATION, "policy"));
        assert!(rank(LLM_CLOUD, "block") > rank(LLM_CLOUD, "allow"));
        assert_eq!(rank(LLM_CLOUD, "deny"), None);
        assert_eq!(rank("hold.ask_mode", "block"), None);
        assert!(!is_floor("llm.server"));
        let mut config = Config::default();
        config.llm.cloud = LlmCloud::Block;
        assert_eq!(value_in(&config, LLM_CLOUD), Some("block"));
    }
}
