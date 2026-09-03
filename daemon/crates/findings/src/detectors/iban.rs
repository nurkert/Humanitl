//! IBAN mit Prüfung nach mod 97 (ISO 13616).
//!
//! Das Muster liefert nur Kandidaten; erst die Prüfziffer macht daraus einen
//! Fund. Deshalb trägt eine IBAN [`Tier::Checksum`]: sie ist nicht geraten.
//!
//! Der Bereich deckt den Kandidaten so ab, wie er im Text steht, also
//! einschließlich der Leerzeichen in `DE89 3704 …`. Der Hash geht über
//! dieselben Bytes; wer eine IBAN mit „immer ignorieren" wegdrückt, drückt
//! damit genau diese Schreibweise weg.

use humanitl_core::{Diagnostic, Finding, FindingKind, Tier};
use regex::bytes::Regex;

use crate::detectors::{build_regex, finding};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Das Muster eines Kandidaten.
///
/// Gegenüber der Spezifikation ist der letzte Block als
/// `(?:[ ]?[0-9A-Z]{1,4})?` geschrieben: ohne das optionale Leerzeichen davor
/// fände `DE89 3704 0044 0532 0130 00` keinen Treffer, weil der Rest `00` hinter
/// einem Leerzeichen steht.
pub const PATTERN: &str = r"\b[A-Z]{2}[0-9]{2}(?:[ ]?[0-9A-Z]{4}){2,7}(?:[ ]?[0-9A-Z]{1,4})?\b";

/// Kürzeste und längste erlaubte Länge ohne Leerzeichen (ISO 13616).
pub const LENGTH_RANGE: core::ops::RangeInclusive<usize> = 15..=34;

/// Sucht IBANs.
#[derive(Debug)]
pub struct IbanDetector {
    regex: Regex,
}

impl IbanDetector {
    /// Baut den Detektor.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das Muster sich nicht übersetzen lässt.
    pub fn new() -> Result<Self, Diagnostic> {
        Ok(Self {
            regex: build_regex("iban", PATTERN)?,
        })
    }
}

impl Detector for IbanDetector {
    fn id(&self) -> &'static str {
        "iban"
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
            if !is_valid(value) {
                continue;
            }
            if let Some(item) = finding(
                FindingKind::Iban,
                matched.start()..matched.end(),
                &input.location,
                Tier::Checksum,
                input.bytes,
            ) {
                found.push(item);
            }
        }
        found
    }
}

/// Ein billiger Vorfilter: steht irgendwo zwei Großbuchstaben plus zwei Ziffern?
///
/// Jede IBAN beginnt so. [`PATTERN`] hat keine feste Zeichenfolge, an der der
/// `regex`-Crate abkürzen könnte; diese Schleife kostet einen Bruchteil davon
/// und ist großzügiger als das Muster.
#[must_use]
pub fn has_candidate(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|window| {
        window[0].is_ascii_uppercase()
            && window[1].is_ascii_uppercase()
            && window[2].is_ascii_digit()
            && window[3].is_ascii_digit()
    })
}

/// Prüft eine IBAN nach mod 97: gültig, wenn der Rest 1 ist.
#[must_use]
pub fn is_valid(candidate: &str) -> bool {
    let compact: Vec<char> = candidate.chars().filter(|item| *item != ' ').collect();
    if !LENGTH_RANGE.contains(&compact.len()) {
        return false;
    }
    let mut rest: u32 = 0;
    // Die ersten vier Zeichen wandern ans Ende (Land plus Prüfziffer).
    for item in compact[4..].iter().chain(compact[..4].iter()) {
        let value = match item {
            '0'..='9' => u32::from(*item as u8 - b'0'),
            'A'..='Z' => u32::from(*item as u8 - b'A') + 10,
            _ => return false,
        };
        rest = if value >= 10 {
            (rest * 100 + value) % 97
        } else {
            (rest * 10 + value) % 97
        };
    }
    rest == 1
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{FindingLocation, Tier};

    use super::{IbanDetector, is_valid};
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(bytes: &[u8]) -> Vec<(core::ops::Range<usize>, Tier, String)> {
        let detector = IbanDetector::new().unwrap();
        detector
            .scan(&ScanInput {
                location: FindingLocation::Body,
                bytes,
                content_type: None,
            })
            .into_iter()
            .map(|found| (found.span, found.tier, found.display_prefix))
            .collect()
    }

    #[test]
    fn a_valid_iban_is_found_with_its_spaces() {
        let body = b"IBAN DE89 3704 0044 0532 0130 00 danke";
        let found = scan(body);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 5..32);
        assert_eq!(
            &body[found[0].0.clone()],
            &b"DE89 3704 0044 0532 0130 00"[..]
        );
        assert_eq!(found[0].1, Tier::Checksum);
        assert_eq!(found[0].2, "DE89 …");
    }

    #[test]
    fn a_wrong_check_digit_is_no_finding() {
        assert!(scan(b"IBAN DE89 3704 0044 0532 0130 01 danke").is_empty());
    }

    #[test]
    fn the_prefilter_answers_the_obvious_cases() {
        // Jeder Treffer des Musters beginnt mit zwei Großbuchstaben und zwei
        // Ziffern; genau danach sucht der Vorfilter.
        assert!(super::has_candidate(b"DE89 3704 0044 0532 0130 00"));
        assert!(super::has_candidate(b"xxGB82WEST"));
        assert!(!super::has_candidate(b"de89 3704"));
        assert!(!super::has_candidate(b"D8E9 3704"));
        assert!(!super::has_candidate(b"DEF9 3704"));
    }

    #[test]
    fn the_check_accepts_the_compact_form_too() {
        assert!(is_valid("DE89370400440532013000"));
        assert!(is_valid("GB82 WEST 1234 5698 7654 32"));
        assert!(!is_valid("DE89370400440532013001"));
        assert!(!is_valid("DE89"));
    }
}
