//! E-Mail-Adressen.
//!
//! Adressen der eigenen Domains sind kein Fund: `findings.email_allow_domains`
//! nennt sie, der Vergleich läuft über die Domain hinter dem letzten `@`, ohne
//! Rücksicht auf Groß- und Kleinschreibung.

use humanitl_core::{Diagnostic, Finding, FindingKind, Tier};
use regex::bytes::Regex;

use crate::detectors::{build_regex, finding};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Das Muster einer Adresse.
pub const PATTERN: &str = r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b";

/// Sucht E-Mail-Adressen.
#[derive(Debug)]
pub struct EmailDetector {
    regex: Regex,
    allow_domains: Vec<String>,
}

impl EmailDetector {
    /// Baut den Detektor.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das Muster sich nicht übersetzen lässt.
    pub fn new(allow_domains: &[String]) -> Result<Self, Diagnostic> {
        Ok(Self {
            regex: build_regex("email", PATTERN)?,
            allow_domains: allow_domains
                .iter()
                .map(|domain| domain.trim().to_ascii_lowercase())
                .filter(|domain| !domain.is_empty())
                .collect(),
        })
    }

    fn is_allowed(&self, value: &str) -> bool {
        let Some((_, domain)) = value.rsplit_once('@') else {
            return false;
        };
        let domain = domain.to_ascii_lowercase();
        self.allow_domains.contains(&domain)
    }
}

impl Detector for EmailDetector {
    fn id(&self) -> &'static str {
        "email"
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        let mut found = Vec::new();
        for matched in self.regex.find_iter(input.bytes) {
            let Ok(value) = core::str::from_utf8(matched.as_bytes()) else {
                continue;
            };
            if self.is_allowed(value) {
                continue;
            }
            if let Some(item) = finding(
                FindingKind::Email,
                matched.start()..matched.end(),
                &input.location,
                Tier::Regex,
                input.bytes,
            ) {
                found.push(item);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::FindingLocation;

    use super::EmailDetector;
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(allow: &[&str], bytes: &[u8]) -> Vec<String> {
        let domains: Vec<String> = allow.iter().map(|item| (*item).to_owned()).collect();
        let detector = EmailDetector::new(&domains).unwrap();
        detector
            .scan(&ScanInput {
                location: FindingLocation::Body,
                bytes,
                content_type: None,
            })
            .into_iter()
            .map(|found| found.display_prefix)
            .collect()
    }

    #[test]
    fn an_address_is_found_and_masked() {
        assert_eq!(
            scan(&[], b"schreib an vorname.nachname@kunde.de bitte"),
            vec!["v***@kunde.de".to_owned()]
        );
    }

    #[test]
    fn an_allowed_domain_is_no_finding() {
        assert!(scan(&["example.com"], b"x@example.com").is_empty());
        assert!(scan(&["EXAMPLE.com"], b"x@Example.COM").is_empty());
        assert_eq!(scan(&["example.com"], b"x@other.example").len(), 1);
    }
}
