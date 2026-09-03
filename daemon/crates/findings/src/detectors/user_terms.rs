//! Eigene Begriffe aus `findings.user_terms`.
//!
//! Kundennamen, Projektnamen, Codenamen: alles, was nie nach außen soll. Der
//! Nutzer hat sie selbst hinterlegt, deshalb wiegt ein Treffer schwerer als
//! jedes Muster ([`Tier::UserTerm`]).
//!
//! Gesucht wird mit Aho-Corasick über alle Begriffe in einem Durchlauf,
//! `leftmost-longest` und ohne Rücksicht auf Groß- und Kleinschreibung im
//! ASCII-Bereich. Ein Treffer zählt nur an einer Wortgrenze, und die wird über
//! UTF-8 geprüft: `Müller` in `Müllerstraße` ist kein eigenständiger Begriff,
//! obwohl `\b` das Gegenteil behaupten würde.

use aho_corasick::{AhoCorasick, MatchKind};
use humanitl_core::{Finding, FindingKind, Tier};

use crate::detectors::{at_word_boundary, finding};
use crate::input::ScanInput;
use crate::registry::Detector;

/// Sucht die Begriffe des Nutzers.
#[derive(Debug)]
pub struct UserTermsDetector {
    automaton: Option<AhoCorasick>,
    terms: Vec<String>,
}

impl UserTermsDetector {
    /// Baut den Detektor.
    ///
    /// Leere Begriffe und reiner Leerraum fallen weg; ohne Begriffe bleibt der
    /// Detektor stumm, statt jede Anfrage zu markieren.
    #[must_use]
    pub fn new(terms: &[String]) -> Self {
        let terms: Vec<String> = terms
            .iter()
            .map(|term| term.trim().to_owned())
            .filter(|term| !term.is_empty())
            .collect();
        let automaton = if terms.is_empty() {
            None
        } else {
            AhoCorasick::builder()
                .ascii_case_insensitive(true)
                .match_kind(MatchKind::LeftmostLongest)
                .build(&terms)
                .ok()
        };
        Self { automaton, terms }
    }
}

impl Detector for UserTermsDetector {
    fn id(&self) -> &'static str {
        "user_terms"
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        let Some(automaton) = self.automaton.as_ref() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for matched in automaton.find_iter(input.bytes) {
            let span = matched.start()..matched.end();
            if !at_word_boundary(input.bytes, &span) {
                continue;
            }
            let Some(term) = self.terms.get(matched.pattern().as_usize()) else {
                continue;
            };
            if let Some(item) = finding(
                FindingKind::UserTerm(term.clone()),
                span,
                &input.location,
                Tier::UserTerm,
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

    use super::UserTermsDetector;
    use crate::input::ScanInput;
    use crate::registry::Detector;

    fn scan(terms: &[&str], bytes: &[u8]) -> Vec<(String, String)> {
        let terms: Vec<String> = terms.iter().map(|term| (*term).to_owned()).collect();
        let detector = UserTermsDetector::new(&terms);
        detector
            .scan(&ScanInput {
                location: FindingLocation::Body,
                bytes,
                content_type: None,
            })
            .into_iter()
            .map(|found| (found.kind.to_string(), found.display_prefix))
            .collect()
    }

    #[test]
    fn a_term_is_found_at_a_word_boundary() {
        assert_eq!(
            scan(&["Acme"], b"Acme Corp"),
            vec![("user_term:Acme".to_owned(), "Acme".to_owned())]
        );
        assert!(scan(&["Acme"], b"Acmeified").is_empty());
    }

    #[test]
    fn the_case_does_not_matter_but_the_value_is_kept() {
        let found = scan(&["Acme"], b"ACME und acme");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, "ACME");
        assert_eq!(found[1].1, "acme");
    }

    #[test]
    fn a_term_with_umlauts_respects_its_neighbours() {
        assert_eq!(scan(&["Müller"], "Kunde Müller, ja".as_bytes()).len(), 1);
        assert!(scan(&["Müller"], "Müllerstraße".as_bytes()).is_empty());
    }

    #[test]
    fn without_terms_nothing_is_found() {
        assert!(scan(&[], b"Acme").is_empty());
        assert!(scan(&["   "], b"Acme").is_empty());
    }
}
