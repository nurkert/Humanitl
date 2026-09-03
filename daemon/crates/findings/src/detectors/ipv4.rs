//! IPv4-Adressen.
//!
//! Eine Adresse im Body ist selten ein Geheimnis, aber oft ein Hinweis: der
//! Agent schickt gerade die Topologie des eigenen Netzes nach draußen. Der Fund
//! ist deshalb eine Notiz, kein Alarm. Loopback, `0.0.0.0` und die
//! Broadcast-Adresse tragen keine Information und fallen weg.

use humanitl_core::{Diagnostic, Finding, FindingKind, Tier};
use regex::bytes::Regex;

use crate::detectors::{build_regex, finding};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Das Muster einer Adresse.
pub const PATTERN: &str =
    r"\b(?:(?:25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])\b";

/// Sucht IPv4-Adressen.
#[derive(Debug)]
pub struct Ipv4Detector {
    regex: Regex,
}

impl Ipv4Detector {
    /// Baut den Detektor.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das Muster sich nicht übersetzen lässt.
    pub fn new() -> Result<Self, Diagnostic> {
        Ok(Self {
            regex: build_regex("ipv4", PATTERN)?,
        })
    }
}

impl Detector for Ipv4Detector {
    fn id(&self) -> &'static str {
        "ipv4"
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        let mut found = Vec::new();
        if !has_candidate(input.bytes) {
            return found;
        }
        for matched in self.regex.find_iter(input.bytes) {
            let Ok(value) = core::str::from_utf8(matched.as_bytes()) else {
                continue;
            };
            if is_uninteresting(value) {
                continue;
            }
            if let Some(item) = finding(
                FindingKind::Ipv4,
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

/// Ein billiger Vorfilter: steht irgendwo eine Ziffer, ein Punkt, eine Ziffer?
///
/// Jede Adresse enthält das dreimal. [`PATTERN`] hat keine feste Zeichenfolge,
/// an der der `regex`-Crate abkürzen könnte; diese Schleife kostet einen
/// Bruchteil davon und ist großzügiger als das Muster.
#[must_use]
pub fn has_candidate(bytes: &[u8]) -> bool {
    bytes
        .windows(3)
        .any(|window| window[0].is_ascii_digit() && window[1] == b'.' && window[2].is_ascii_digit())
}

/// Wahr für Adressen, die nichts über das Netz des Nutzers verraten.
///
/// `127.0.0.0/8`, `0.0.0.0` und `255.255.255.255`.
#[must_use]
pub fn is_uninteresting(value: &str) -> bool {
    value == "0.0.0.0" || value == "255.255.255.255" || value.starts_with("127.")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::FindingLocation;

    use super::Ipv4Detector;
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(bytes: &[u8]) -> Vec<String> {
        let detector = Ipv4Detector::new().unwrap();
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
    fn an_address_is_shown_in_full() {
        assert_eq!(
            scan(b"llm auf 192.168.1.20:11434"),
            vec!["192.168.1.20".to_owned()]
        );
    }

    #[test]
    fn loopback_and_wildcards_are_no_finding() {
        assert!(scan(b"127.0.0.1 0.0.0.0 255.255.255.255").is_empty());
    }

    #[test]
    fn the_prefilter_answers_the_obvious_cases() {
        // Jeder Treffer des Musters enthält Ziffer-Punkt-Ziffer.
        assert!(super::has_candidate(b"192.168.1.20"));
        assert!(!super::has_candidate(b"192-168-1-20"));
        assert!(!super::has_candidate(b"kein Punkt zwischen Ziffern: 1 . 2"));
    }

    #[test]
    fn an_octet_over_255_is_no_address() {
        assert!(scan(b"300.1.2.3").is_empty());
    }
}
