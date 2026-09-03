//! Telefonnummern, nur in internationaler Schreibweise.
//!
//! Nationale Schreibweisen ohne Vorwahl-Präfix wären in einem JSON nicht von
//! einer Bestellnummer zu unterscheiden. Deshalb greift der Detektor nur bei
//! `+` oder `00` am Anfang und prüft danach die Zahl der Ziffern (E.164 erlaubt
//! höchstens 15).

use humanitl_core::{Diagnostic, Finding, FindingKind, Tier};
use regex::bytes::Regex;

use crate::detectors::{build_regex, finding, preceded_by_iban_head};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Das Muster eines Kandidaten.
pub const PATTERN: &str = r"(?:\+|00)[1-9][0-9 .\-/()]{6,20}[0-9]";

/// Kürzeste und längste erlaubte Ziffernzahl nach dem Entfernen der Trenner.
pub const DIGIT_RANGE: core::ops::RangeInclusive<usize> = 8..=15;

/// Sucht Telefonnummern.
#[derive(Debug)]
pub struct PhoneDetector {
    regex: Regex,
}

impl PhoneDetector {
    /// Baut den Detektor.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das Muster sich nicht übersetzen lässt.
    pub fn new() -> Result<Self, Diagnostic> {
        Ok(Self {
            regex: build_regex("phone", PATTERN)?,
        })
    }
}

impl Detector for PhoneDetector {
    fn id(&self) -> &'static str {
        "phone"
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        let mut found = Vec::new();
        for matched in self.regex.find_iter(input.bytes) {
            // Eine IBAN trägt oft eine Gruppe, die mit 00 beginnt; das ist
            // keine Auslandsvorwahl.
            if preceded_by_iban_head(input.bytes, matched.start()) {
                continue;
            }
            let digits = matched
                .as_bytes()
                .iter()
                .filter(|byte| byte.is_ascii_digit())
                .count();
            if !DIGIT_RANGE.contains(&digits) {
                continue;
            }
            if let Some(item) = finding(
                FindingKind::Phone,
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

    use super::PhoneDetector;
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(bytes: &[u8]) -> Vec<String> {
        let detector = PhoneDetector::new().unwrap();
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
    fn an_international_number_is_found_and_shortened() {
        assert_eq!(scan(b"Tel +49 30 1234567 privat"), vec!["+49 …".to_owned()]);
        assert_eq!(scan(b"Tel 0049 (30) 1234-567").len(), 1);
    }

    #[test]
    fn a_national_number_is_no_finding() {
        assert!(scan(b"Tel 030 1234567").is_empty());
    }

    #[test]
    fn digits_inside_an_iban_are_no_number() {
        assert!(scan("Bitte an DE89 3704 0044 0532 0130 00 überweisen".as_bytes()).is_empty());
    }

    #[test]
    fn too_few_or_too_many_digits_are_no_finding() {
        assert!(scan(b"+49 30 12").is_empty());
        assert!(scan(b"+4912345678901234567").is_empty());
    }
}
