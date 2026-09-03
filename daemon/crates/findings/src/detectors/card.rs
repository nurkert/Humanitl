//! Kreditkartennummern mit Luhn-Prüfung (ISO/IEC 7812).
//!
//! Das Muster liefert Ziffernfolgen mit Leerzeichen oder Bindestrichen; erst
//! Luhn und ein bekanntes Präfix machen daraus einen Fund. Ohne diese zwei
//! Prüfungen wäre jede längere Zahl ein Treffer, und der Mensch würde die
//! Markierung nach dem dritten Fehlalarm ignorieren.

use humanitl_core::{Diagnostic, Finding, FindingKind, Tier};
use regex::bytes::Regex;

use crate::detectors::{build_regex, finding, preceded_by_iban_head};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Das Muster eines Kandidaten.
pub const PATTERN: &str = r"\b(?:[0-9][ -]?){12,18}[0-9]\b";

/// Kürzeste und längste erlaubte Ziffernzahl.
pub const LENGTH_RANGE: core::ops::RangeInclusive<usize> = 13..=19;

/// Sucht Kreditkartennummern.
#[derive(Debug)]
pub struct CardDetector {
    regex: Regex,
}

impl CardDetector {
    /// Baut den Detektor.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das Muster sich nicht übersetzen lässt.
    pub fn new() -> Result<Self, Diagnostic> {
        Ok(Self {
            regex: build_regex("card", PATTERN)?,
        })
    }
}

impl Detector for CardDetector {
    fn id(&self) -> &'static str {
        "card"
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        let mut found = Vec::new();
        if !has_candidate(input.bytes) {
            return found;
        }
        for matched in self.regex.find_iter(input.bytes) {
            // Die Ziffern einer IBAN gehören dem IBAN-Detektor, auch wenn sie
            // zufällig Luhn erfüllen und mit 37 wie eine Amex-Karte beginnen.
            if preceded_by_iban_head(input.bytes, matched.start()) {
                continue;
            }
            let digits: Vec<u8> = matched
                .as_bytes()
                .iter()
                .filter(|byte| byte.is_ascii_digit())
                .map(|byte| byte - b'0')
                .collect();
            if !LENGTH_RANGE.contains(&digits.len()) || !luhn(&digits) || !known_prefix(&digits) {
                continue;
            }
            if let Some(item) = finding(
                FindingKind::CreditCard,
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

/// Ein billiger Vorfilter: gibt es überhaupt genug Ziffern am Stück?
///
/// [`PATTERN`] hat keine feste Zeichenfolge, an der der `regex`-Crate den
/// Automaten abkürzen könnte; über einem großen JSON kostet er deshalb ein
/// Vielfaches dieser Schleife. Der Vorfilter ist bewusst großzügiger als das
/// Muster: Er zählt Ziffern, zwischen denen höchstens ein Leerzeichen oder ein
/// Bindestrich steht. Was das Muster findet, findet auch er.
#[must_use]
pub fn has_candidate(bytes: &[u8]) -> bool {
    let mut digits = 0usize;
    let mut separator = false;
    for byte in bytes {
        if byte.is_ascii_digit() {
            digits += 1;
            separator = false;
            if digits >= *LENGTH_RANGE.start() {
                return true;
            }
        } else if matches!(byte, b' ' | b'-') && digits > 0 && !separator {
            separator = true;
        } else {
            digits = 0;
            separator = false;
        }
    }
    false
}

/// Die Luhn-Prüfung: von rechts jede zweite Ziffer verdoppeln, Summe durch 10.
#[must_use]
pub fn luhn(digits: &[u8]) -> bool {
    if digits.is_empty() {
        return false;
    }
    let mut sum: u32 = 0;
    for (index, digit) in digits.iter().rev().enumerate() {
        let mut value = u32::from(*digit);
        if index % 2 == 1 {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
    }
    sum.is_multiple_of(10)
}

/// Wahr für die Präfixe der gängigen Kartensysteme.
///
/// Visa `4`, Mastercard `51`-`55` und `2221`-`2720`, Amex `34` und `37`,
/// Discover `6011` und `65`, JCB `35`.
#[must_use]
pub fn known_prefix(digits: &[u8]) -> bool {
    let number = |count: usize| -> Option<u32> {
        let head = digits.get(..count)?;
        Some(
            head.iter()
                .fold(0u32, |acc, digit| acc * 10 + u32::from(*digit)),
        )
    };
    let (Some(one), Some(two), Some(four)) = (number(1), number(2), number(4)) else {
        return false;
    };
    one == 4
        || (51..=55).contains(&two)
        || (2221..=2720).contains(&four)
        || two == 34
        || two == 37
        || four == 6011
        || two == 65
        || two == 35
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::FindingLocation;

    use super::{CardDetector, luhn};
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(bytes: &[u8]) -> Vec<String> {
        let detector = CardDetector::new().unwrap();
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
    fn a_valid_number_is_found_and_masked() {
        assert_eq!(
            scan(b"Karte 4111 1111 1111 1111 ok"),
            vec!["**** 1111".to_owned()]
        );
        assert_eq!(scan(b"Karte 4111-1111-1111-1111 ok").len(), 1);
    }

    #[test]
    fn a_wrong_check_digit_is_no_finding() {
        assert!(scan(b"Karte 4111 1111 1111 1112 ok").is_empty());
    }

    #[test]
    fn the_prefilter_never_hides_a_match() {
        // Der Vorfilter darf großzügiger sein als das Muster, nie strenger.
        // Deshalb hier 2000 Ziffernfolgen mit Trennern gegen beide Wege.
        let regex = crate::detectors::build_regex("card", super::PATTERN).unwrap();
        let alphabet = b"0123456789 -x";
        let mut state = 0x2026_0903_u64 | 1;
        for _ in 0..2000 {
            let mut candidate = Vec::new();
            for _ in 0..24 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let index = usize::try_from(state % alphabet.len() as u64).unwrap_or(0);
                candidate.push(alphabet[index]);
            }
            if regex.find(&candidate).is_some() {
                assert!(
                    super::has_candidate(&candidate),
                    "Vorfilter verwirft {:?}",
                    String::from_utf8_lossy(&candidate)
                );
            }
        }
    }

    #[test]
    fn the_prefilter_answers_the_obvious_cases() {
        assert!(super::has_candidate(b"4111 1111 1111 1111"));
        assert!(super::has_candidate(b"4111-1111-1111-1111"));
        assert!(super::has_candidate(b"1234567890123"));
        assert!(!super::has_candidate(b"{\"id\":123,\"note\":\"kurz\"}"));
        assert!(!super::has_candidate(b"1234 5678 90-12"));
    }

    #[test]
    fn digits_inside_an_iban_belong_to_the_iban() {
        // Luhn-gültig, Präfix 37, und trotzdem keine Karte: der Lauf beginnt
        // mit einer IBAN-Kopfgruppe.
        assert!(scan(b"Bitte an DE89 3704 0044 0532 0130 01 danke").is_empty());
    }

    #[test]
    fn an_unknown_prefix_is_no_finding() {
        // Luhn-gültig, aber kein Kartensystem beginnt mit 9.
        assert!(luhn(&[9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 5]));
        assert!(scan(b"9999 9999 9999 9995").is_empty());
    }
}
