//! Die Tier-1-Detektoren und ihr gemeinsames Handwerkszeug.
//!
//! Tier 1 heißt: inline-tauglich. Nur endliche Automaten (`regex::bytes`) und
//! Prüfsummen, keine Modelle, keine Netzwerkabfragen. Jeder Detektor bekommt
//! ein Suchziel und gibt Funde mit Bereichen relativ zu dessen Bytes zurück.
//!
//! Zwei Regeln gelten für alle:
//!
//! - **Linear in der Eingabe.** Muster werden mit gesetztem
//!   [`REGEX_SIZE_LIMIT`] übersetzt, tragen keine verschachtelten
//!   Quantifizierer über unbegrenzten Klassen und laufen ohne Backtracking. Die
//!   Prüfsummen-Detektoren prüfen jeden Kandidaten in konstanter Zeit.
//! - **Kein Wert nach außen.** Ein Fund entsteht nur über [`finding`]; dort
//!   wird gehasht und maskiert.

pub mod card;
pub mod email;
pub mod iban;
pub mod ipv4;
pub mod phone;
pub mod secrets;
pub mod user_terms;

use core::ops::Range;

use humanitl_core::diagnostics::codes::FINDINGS_001;
use humanitl_core::{Diagnostic, Finding, FindingKind, FindingLocation, Severity, Tier};
use regex::bytes::{Regex, RegexBuilder};

use crate::display::display_prefix;
use crate::registry::Detector;
use crate::settings::FindingsSettings;

/// Obergrenze für den übersetzten Automaten eines Musters.
///
/// Der `regex`-Crate läuft linear, aber ein unglückliches Muster kann einen
/// großen Automaten erzeugen. Mit der Grenze schlägt die Übersetzung fehl
/// (`FINDINGS_001`), statt zur Laufzeit Speicher zu fressen.
pub const REGEX_SIZE_LIMIT: usize = 4 * 1024 * 1024;

/// Obergrenze für den Cache des lazy DFA je Muster.
pub const REGEX_DFA_SIZE_LIMIT: usize = 8 * 1024 * 1024;

/// Baut die sieben Tier-1-Detektoren in der Reihenfolge aus [`crate::TIER1_DETECTOR_IDS`].
///
/// # Errors
///
/// `FINDINGS_001`, wenn ein Muster sich nicht übersetzen lässt oder das
/// eingebaute Regel-Set der Secrets nicht lesbar ist.
pub fn tier1(settings: &FindingsSettings) -> Result<Vec<Box<dyn Detector>>, Diagnostic> {
    Ok(vec![
        Box::new(secrets::SecretsDetector::builtin()?),
        Box::new(email::EmailDetector::new(&settings.email_allow_domains)?),
        Box::new(iban::IbanDetector::new()?),
        Box::new(card::CardDetector::new()?),
        Box::new(phone::PhoneDetector::new()?),
        Box::new(ipv4::Ipv4Detector::new()?),
        Box::new(user_terms::UserTermsDetector::new(&settings.user_terms)),
    ])
}

/// Übersetzt ein Muster mit gesetzten Grenzen.
///
/// # Errors
///
/// `FINDINGS_001` mit Regelname und Meldung des Übersetzers.
pub fn build_regex(rule: &str, pattern: &str) -> Result<Regex, Diagnostic> {
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .map_err(|error| regex_error(rule, &error.to_string()))
}

/// Ein Befund für ein Muster, das sich nicht übersetzen ließ.
#[must_use]
pub fn regex_error(rule: &str, message: &str) -> Diagnostic {
    Diagnostic::builder(FINDINGS_001, Severity::Error)
        .why(format!(
            "das Muster der Regel \"{rule}\" ließ sich nicht übersetzen: {message}"
        ))
        .build()
}

/// Baut einen Fund aus einem Bereich des Suchziels.
///
/// `None`, wenn die Bytes des Bereichs kein gültiges UTF-8 sind. Der Hash geht
/// über genau diese Bytes, deshalb wird nichts ersetzt und nichts geraten: ein
/// Fund, dessen Wert sich nicht verlustfrei lesen lässt, entsteht gar nicht
/// erst.
#[must_use]
pub fn finding(
    kind: FindingKind,
    span: Range<usize>,
    location: &FindingLocation,
    tier: Tier,
    bytes: &[u8],
) -> Option<Finding> {
    let value = core::str::from_utf8(bytes.get(span.clone())?).ok()?;
    let mut found = Finding::new(kind, span, location.clone(), tier, value);
    found.display_prefix = display_prefix(&found.kind, value);
    Some(found)
}

/// Das Zeichen unmittelbar vor `index`, wenn es sich lesen lässt.
///
/// `\b` in `regex::bytes` ist eine ASCII-Wortgrenze; ein `ü` gilt dort als
/// Nicht-Wort. Für eigene Begriffe reicht das nicht, deshalb wird der Nachbar
/// als UTF-8 gelesen (Fallstrick des Issues).
#[must_use]
pub fn char_before(bytes: &[u8], index: usize) -> Option<char> {
    let start = index.saturating_sub(4);
    let slice = bytes.get(start..index)?;
    for take in 1..=slice.len() {
        if let Ok(text) = core::str::from_utf8(&slice[slice.len() - take..]) {
            return text.chars().next_back();
        }
    }
    None
}

/// Das Zeichen unmittelbar ab `index`, wenn es sich lesen lässt.
#[must_use]
pub fn char_after(bytes: &[u8], index: usize) -> Option<char> {
    let end = (index + 4).min(bytes.len());
    let slice = bytes.get(index..end)?;
    for take in 1..=slice.len() {
        if let Ok(text) = core::str::from_utf8(&slice[..take]) {
            return text.chars().next();
        }
    }
    None
}

/// So weit schaut ein Detektor nach links, um eine IBAN-Kopfgruppe zu finden.
///
/// Eine IBAN ist höchstens 34 Zeichen lang und trägt bis zu acht Leerzeichen;
/// 48 Bytes decken jede Schreibweise ab und halten die Prüfung an einer festen
/// Grenze, damit ein feindseliger Body sie nicht quadratisch macht.
pub const IBAN_LOOKBACK: usize = 48;

/// Wahr, wenn `start` in einer Zeichenkette liegt, die wie eine IBAN beginnt.
///
/// `DE89 3704 0044 0532 0130 00` enthält eine Ziffernfolge, die das Muster für
/// Kreditkarten und das für internationale Rufnummern ebenfalls annehmen.
/// Beide Funde wären für den Menschen nichts als Lärm, und Lärm ist der Grund,
/// warum jemand ohne Lesen auf „Erlauben" klickt. Deshalb halten sich `card`
/// und `phone` zurück, sobald ihr Kandidat in einem Lauf steht, der mit zwei
/// Großbuchstaben und zwei Ziffern an einer Wortgrenze beginnt und bis zum
/// Kandidaten nur aus Großbuchstaben, Ziffern und einzelnen Leerzeichen
/// besteht. Ob die Prüfziffer der IBAN stimmt, spielt dabei keine Rolle: Die
/// Form allein gehört dem IBAN-Detektor.
#[must_use]
pub fn preceded_by_iban_head(bytes: &[u8], start: usize) -> bool {
    let window_start = start.saturating_sub(IBAN_LOOKBACK);
    let Some(window) = bytes.get(window_start..start) else {
        return false;
    };
    for offset in 0..window.len().saturating_sub(3) {
        let head = &window[offset..offset + 4];
        if !(head[0].is_ascii_uppercase()
            && head[1].is_ascii_uppercase()
            && head[2].is_ascii_digit()
            && head[3].is_ascii_digit())
        {
            continue;
        }
        let absolute = window_start + offset;
        if absolute > 0 && bytes[absolute - 1].is_ascii_alphanumeric() {
            continue;
        }
        if window[offset + 4..]
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b' ')
        {
            return true;
        }
    }
    false
}

/// Wahr, wenn links und rechts des Bereichs kein alphanumerisches Zeichen steht.
#[must_use]
pub fn at_word_boundary(bytes: &[u8], span: &Range<usize>) -> bool {
    let left = char_before(bytes, span.start).is_none_or(|found| !found.is_alphanumeric());
    let right = char_after(bytes, span.end).is_none_or(|found| !found.is_alphanumeric());
    left && right
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{FindingKind, FindingLocation, Tier};

    use super::{at_word_boundary, char_after, char_before, finding, preceded_by_iban_head};

    #[test]
    fn neighbours_are_read_as_utf8_not_as_bytes() {
        let bytes = "xüy".as_bytes();
        assert_eq!(char_before(bytes, 1), Some('x'));
        assert_eq!(char_before(bytes, 3), Some('ü'));
        assert_eq!(char_after(bytes, 1), Some('ü'));
        assert_eq!(char_after(bytes, 3), Some('y'));
        assert_eq!(char_before(bytes, 0), None);
        assert_eq!(char_after(bytes, 4), None);
    }

    #[test]
    fn a_word_boundary_counts_umlauts_as_letters() {
        let bytes = "Müller-Projekt".as_bytes();
        assert!(at_word_boundary(bytes, &(0..7)));
        assert!(!at_word_boundary(bytes, &(0..6)));
    }

    #[test]
    fn an_iban_shaped_run_is_recognised_from_the_left() {
        let body = "Bitte an DE89 3704 0044 0532 0130 00 überweisen".as_bytes();
        let group = body
            .windows(4)
            .position(|window| window == b"0044")
            .unwrap();
        assert!(preceded_by_iban_head(body, group));
        assert!(preceded_by_iban_head(body, 9 + 5));

        // Eine Karte hinter einem gewöhnlichen Wort bleibt eine Karte.
        let card = b"Karte 4111 1111 1111 1111";
        assert!(!preceded_by_iban_head(card, 6));
        // Ohne Wortgrenze links ist es keine Kopfgruppe.
        assert!(!preceded_by_iban_head(b"xxDE89 3704 0044", 12));
        // Und ein fremdes Zeichen im Lauf bricht ihn ab.
        assert!(!preceded_by_iban_head(b"DE89 3704, 0044", 11));
    }

    #[test]
    fn a_finding_never_carries_its_value() {
        // Zur Laufzeit zusammengesetzt: Der Push-Schutz von GitHub blockiert ein
        // echt geformtes Token im Quelltext (CONVENTIONS 4.13).
        let text = format!(
            "key {}{} rest",
            "ghp", "_0123456789abcdefghijklmnopqrstuvwx"
        );
        let bytes = text.as_bytes();
        let found = finding(
            FindingKind::ApiKey("github".to_owned()),
            4..42,
            &FindingLocation::Body,
            Tier::Regex,
            bytes,
        )
        .unwrap();
        assert_eq!(found.display_prefix, "ghp_01…");
        assert_eq!(found.value_hash.len(), 32);
        assert!(!format!("{found:?}").contains("789abcdef"));
    }

    #[test]
    fn invalid_utf8_produces_no_finding() {
        let bytes = [0xffu8, 0xfe, 0xfd, 0xfc];
        assert!(
            finding(
                FindingKind::Ipv4,
                0..4,
                &FindingLocation::Body,
                Tier::Regex,
                &bytes,
            )
            .is_none()
        );
    }
}
