//! Schlüssel, Token und private Schlüssel aus einem Regel-Set.
//!
//! Die Muster stehen in `src/rules/secrets.toml` im Format von gitleaks und
//! werden beim Bauen der Registry übersetzt.
//!
//! Jede Regel läuft als eigenes Muster über die Bytes, nicht als Glied eines
//! `RegexSet`. Das ist der schnellere Weg, obwohl es nach mehr Arbeit aussieht:
//! Jedes einzelne Muster verlangt eine feste Zeichenfolge (`AKIA`, `ghp_`,
//! `sk-ant-api03-`, `AIza`, `-----BEGIN `, `xox`, `ey`), und der `regex`-Crate
//! sucht die zuerst mit einer Zeichensuche, die pro Byte fast nichts kostet.
//! Die Vereinigung im `RegexSet` hat keine gemeinsame Zeichenfolge mehr und
//! muss deshalb ihren Automaten über jedes Byte laufen lassen. Gemessen an
//! 8 MiB JSON: 13 ms für die dreizehn Muster einzeln, 55 bis 102 ms für das
//! `RegexSet`.

use humanitl_core::diagnostics::codes::FINDINGS_001;
use humanitl_core::{Diagnostic, Finding, FindingKind, FindingLocation, Severity, Tier};
use regex::bytes::Regex;
use serde::Deserialize;

use crate::detectors::{build_regex, finding};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Das eingebaute Regel-Set, wie es im Binary liegt.
pub const BUILTIN_RULES: &str = include_str!("../rules/secrets.toml");

/// Eine Regel, so wie sie in der TOML-Datei steht.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSpec {
    /// Stabiler Name der Regel.
    pub id: String,
    /// `api_key` oder `jwt`.
    pub kind: String,
    /// Der Anbieter, nur bei `api_key`.
    #[serde(default)]
    pub provider: Option<String>,
    /// Das Muster.
    pub regex: String,
    /// Woher das Muster stammt: gitleaks-Regel-ID oder `humanitl`.
    pub source: String,
    /// Wenn gesetzt, greift die Regel nur in diesem Header.
    #[serde(default)]
    pub header_only: Option<String>,
    /// Welche Gruppe der Bereich des Fundes ist; 0 ist der ganze Treffer.
    #[serde(default)]
    pub capture: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    rules: Vec<RuleSpec>,
}

/// Liest das eingebaute Regel-Set.
///
/// # Errors
///
/// `FINDINGS_001`, wenn die Datei kein gültiges TOML ist, eine Regel keinen
/// `source` trägt oder `kind` unbekannt ist.
pub fn builtin_rules() -> Result<Vec<RuleSpec>, Diagnostic> {
    let parsed: RuleFile = toml::from_str(BUILTIN_RULES).map_err(|error| {
        Diagnostic::builder(FINDINGS_001, Severity::Error)
            .why(format!(
                "das eingebaute Regel-Set der Secret-Detektoren ist kein gültiges TOML: {error}"
            ))
            .build()
    })?;
    for rule in &parsed.rules {
        if rule.source.trim().is_empty() {
            return Err(rule_error(&rule.id, "das Feld source ist leer"));
        }
        kind_of(rule)?;
    }
    Ok(parsed.rules)
}

fn kind_of(rule: &RuleSpec) -> Result<FindingKind, Diagnostic> {
    match rule.kind.as_str() {
        "api_key" => rule
            .provider
            .clone()
            .map(FindingKind::ApiKey)
            .ok_or_else(|| rule_error(&rule.id, "kind = \"api_key\" braucht ein Feld provider")),
        "jwt" => Ok(FindingKind::Jwt),
        other => Err(rule_error(
            &rule.id,
            &format!("kind = \"{other}\" ist unbekannt, erlaubt sind api_key und jwt"),
        )),
    }
}

fn rule_error(id: &str, why: &str) -> Diagnostic {
    Diagnostic::builder(FINDINGS_001, Severity::Error)
        .why(format!("die Regel \"{id}\" ist unbrauchbar: {why}"))
        .build()
}

#[derive(Debug)]
struct CompiledRule {
    kind: FindingKind,
    regex: Regex,
    header_only: Option<String>,
    capture: usize,
}

impl CompiledRule {
    /// Die Bereiche, die diese Regel in den Bytes findet.
    ///
    /// Ohne Gruppe läuft `find_iter`, das den endlichen Automaten benutzt. Erst
    /// eine Gruppe zwingt den `regex`-Crate auf die Maschine, die
    /// Gruppengrenzen mitschreibt, und die ist um Größenordnungen langsamer.
    /// Die zwei Regeln mit Gruppe greifen nur in einer Kopfzeile, also über ein
    /// paar hundert Bytes.
    fn spans(&self, bytes: &[u8]) -> Vec<core::ops::Range<usize>> {
        if self.capture == 0 {
            return self
                .regex
                .find_iter(bytes)
                .map(|matched| matched.start()..matched.end())
                .collect();
        }
        self.regex
            .captures_iter(bytes)
            .filter_map(|captures| {
                captures
                    .get(self.capture)
                    .map(|matched| matched.start()..matched.end())
            })
            .collect()
    }

    fn applies_to(&self, location: &FindingLocation) -> bool {
        match &self.header_only {
            None => true,
            Some(name) => {
                matches!(location, FindingLocation::Header(header) if header.as_str() == name)
            }
        }
    }
}

/// Sucht Schlüssel, Token und private Schlüssel.
#[derive(Debug)]
pub struct SecretsDetector {
    rules: Vec<CompiledRule>,
}

impl SecretsDetector {
    /// Baut den Detektor aus dem eingebauten Regel-Set.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, siehe [`builtin_rules`] und [`crate::detectors::build_regex`].
    pub fn builtin() -> Result<Self, Diagnostic> {
        Self::from_rules(&builtin_rules()?)
    }

    /// Baut den Detektor aus einem eigenen Regel-Set.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn ein Muster sich nicht übersetzen lässt.
    pub fn from_rules(specs: &[RuleSpec]) -> Result<Self, Diagnostic> {
        let mut rules = Vec::with_capacity(specs.len());
        for spec in specs {
            rules.push(CompiledRule {
                kind: kind_of(spec)?,
                regex: build_regex(&spec.id, &spec.regex)?,
                header_only: spec.header_only.clone(),
                capture: spec.capture,
            });
        }
        Ok(Self { rules })
    }
}

impl Detector for SecretsDetector {
    fn id(&self) -> &'static str {
        "secrets"
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        let mut found = Vec::new();
        for rule in &self.rules {
            if !rule.applies_to(&input.location) {
                continue;
            }
            for span in rule.spans(input.bytes) {
                if let Some(item) = finding(
                    rule.kind.clone(),
                    span,
                    &input.location,
                    Tier::Regex,
                    input.bytes,
                ) {
                    found.push(item);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::http::HeaderName;
    use humanitl_core::{FindingKind, FindingLocation};

    use super::{SecretsDetector, builtin_rules};
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(location: FindingLocation, bytes: &[u8]) -> Vec<(String, String)> {
        let detector = SecretsDetector::builtin().unwrap();
        detector
            .scan(&ScanInput {
                location,
                bytes,
                content_type: None,
            })
            .into_iter()
            .map(|found| (found.kind.to_string(), found.display_prefix))
            .collect()
    }

    #[test]
    fn every_rule_names_its_source() {
        let rules = builtin_rules().unwrap();
        assert_eq!(rules.len(), 13);
        for rule in &rules {
            assert!(!rule.source.trim().is_empty(), "{}", rule.id);
            assert!(
                rule.source.starts_with("gitleaks:") || rule.source == "humanitl",
                "{}: {}",
                rule.id,
                rule.source
            );
        }
    }

    #[test]
    fn a_github_pat_needs_exactly_thirty_six_characters() {
        let long = format!("token ghp_{} end", "a".repeat(36));
        let short = format!("token ghp_{} end", "a".repeat(35));
        assert_eq!(scan(FindingLocation::Body, long.as_bytes()).len(), 1);
        assert!(scan(FindingLocation::Body, short.as_bytes()).is_empty());
    }

    #[test]
    fn bearer_only_matches_in_the_authorization_header() {
        let value = b"Bearer abcdefghijklmnopqrstuvwxyz0123";
        let header = scan(
            FindingLocation::Header(HeaderName::from_static("authorization")),
            value,
        );
        assert_eq!(header.len(), 1);
        assert_eq!(header[0].0, "api_key:bearer");
        assert!(scan(FindingLocation::Body, value).is_empty());
    }

    #[test]
    fn the_kinds_carry_their_provider() {
        let body = b"AKIAIOSFODNN7EXAMPLE";
        let found = scan(FindingLocation::Body, body);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].0,
            FindingKind::ApiKey("aws".to_owned()).to_string()
        );
        assert_eq!(found[0].1, "AKIAIO…");
    }
}
