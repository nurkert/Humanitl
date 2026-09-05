//! Ob ein Schlüssel heute wirkt oder auf ein Issue wartet.
//!
//! `docs/CONFIG.md` entsteht aus dem Schema und ist deshalb nie veraltet, was
//! die **Existenz** eines Schlüssels angeht. Über seine **Wirkung** sagt der
//! Generator von sich aus nichts, und ein Schlüssel, der beschrieben, geprüft
//! und von niemandem gelesen wird, sieht von außen genauso aus wie ein
//! wirksamer (HUM-101). Bei einer Ressourcengrenze ist das der Unterschied
//! zwischen einer Zusage und einer Behauptung.
//!
//! Deshalb trägt jedes Blattfeld ohne Leser im Schema unter
//! [`PENDING_ISSUE_KEY`] die Kennung des Issues, das über den Schlüssel
//! entscheidet — durch Einbau oder durch Streichung. Fehlt die Angabe, gilt
//! [`Readiness::Effective`]. Die Einstufung ist die Behauptung eines Menschen,
//! keine Messung: Sie soll den **vergessenen** Schlüssel finden, und wer beim
//! Anlegen „wirkt" schreibt, ohne zu verdrahten, hat nicht übersehen, sondern
//! gelogen.
//!
//! Die zweite Hälfte ist das Register in
//! `daemon/crates/config/tests/config_readers.rs`: eine Zeile je Schema-Pfad,
//! genau eine Einstufung. Sein Test hält beide Seiten zusammen und wird rot,
//! sobald das Schema einen Pfad kennt, den das Register nicht kennt, sobald das
//! Register einen Pfad nennt, den es nicht mehr gibt, oder sobald eine
//! Einstufung von der Angabe im Schema abweicht.

use core::fmt;

/// Der Schlüssel, unter dem das offene Issue im JSON-Schema steht.
pub const PENDING_ISSUE_KEY: &str = "x-pending-issue";

/// Das Präfix jeder Issue-Kennung dieses Repositories.
const ISSUE_PREFIX: &str = "HUM-";

/// Die Zahl der Ziffern hinter [`ISSUE_PREFIX`].
///
/// Drei heute, mehr später: Die Nummern laufen fortlaufend weiter, und eine
/// Prüfung, die bei `HUM-1000` scheitert, macht das Register an dem Tag rot, an
/// dem sie gebraucht wird. Nach oben bleibt eine Grenze, damit die Prüfung noch
/// eine ist und nicht jede Ziffernfolge durchlässt.
const ISSUE_DIGITS: std::ops::RangeInclusive<usize> = 3..=6;

/// Ob ein Schlüssel heute einen Leser hat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Readiness {
    /// Der Schlüssel wird gelesen und wirkt.
    Effective,
    /// Der Schlüssel hat heute keinen Leser; das genannte Issue entscheidet
    /// ihn, durch Einbau oder durch Streichung.
    Pending(String),
}

impl Readiness {
    /// Der Wert `effective`, wie er im Register steht.
    pub const EFFECTIVE: &'static str = "effective";

    /// Die Einstufung aus der Angabe im Schema.
    ///
    /// `None` heißt: keine Angabe, also wirksam.
    #[must_use]
    pub fn from_issue(issue: Option<&str>) -> Self {
        issue.map_or(Self::Effective, |issue| Self::Pending(issue.to_owned()))
    }

    /// Liest eine Einstufung, wie sie im Register steht: `effective` oder
    /// `pending(HUM-079)`.
    ///
    /// Ein Text mit einer Kennung, die nicht wie eine Issue-Kennung aussieht,
    /// ist keine Einstufung: Ein Verweis, der ins Leere geht, ist schlechter
    /// als keiner.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text == Self::EFFECTIVE {
            return Some(Self::Effective);
        }
        let issue = text.strip_prefix("pending(")?.strip_suffix(')')?;
        is_issue_id(issue).then(|| Self::Pending(issue.to_owned()))
    }

    /// Das Issue, das den Schlüssel entscheidet, falls er noch keinen Leser
    /// hat.
    #[must_use]
    pub fn issue(&self) -> Option<&str> {
        match self {
            Self::Effective => None,
            Self::Pending(issue) => Some(issue),
        }
    }

    /// Wahr, solange der Schlüssel keinen Leser hat.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Die Spalte „Wirkung" in `docs/CONFIG.md`.
    #[must_use]
    pub fn doc_label(&self) -> String {
        match self {
            Self::Effective => "ja".to_owned(),
            Self::Pending(issue) => format!("offen ({issue})"),
        }
    }
}

impl fmt::Display for Readiness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Effective => f.write_str(Self::EFFECTIVE),
            Self::Pending(issue) => write!(f, "pending({issue})"),
        }
    }
}

/// Ob `text` eine Issue-Kennung dieses Repositories ist: `HUM-` und drei bis
/// sechs Ziffern.
#[must_use]
pub fn is_issue_id(text: &str) -> bool {
    let Some(number) = text.strip_prefix(ISSUE_PREFIX) else {
        return false;
    };
    ISSUE_DIGITS.contains(&number.len()) && number.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Readiness, is_issue_id};

    #[test]
    fn parse_round_trips_both_shapes() {
        for text in ["effective", "pending(HUM-079)"] {
            let readiness = Readiness::parse(text).expect("the register knows this shape");
            assert_eq!(readiness.to_string(), text);
        }
    }

    #[test]
    fn a_reference_that_is_no_issue_is_no_readiness() {
        assert_eq!(Readiness::parse("pending(soon)"), None);
        assert_eq!(Readiness::parse("pending(HUM-79)"), None);
        assert_eq!(Readiness::parse("pending()"), None);
        assert_eq!(Readiness::parse("pending(HUM-079"), None);
        assert_eq!(Readiness::parse("later"), None);
    }

    #[test]
    fn a_missing_note_means_effective() {
        assert_eq!(Readiness::from_issue(None), Readiness::Effective);
        assert_eq!(
            Readiness::from_issue(Some("HUM-079")),
            Readiness::Pending("HUM-079".to_owned())
        );
        assert!(!Readiness::Effective.is_pending());
        assert!(Readiness::from_issue(Some("HUM-079")).is_pending());
        assert_eq!(Readiness::Effective.issue(), None);
    }

    #[test]
    fn the_doc_label_names_the_issue() {
        assert_eq!(Readiness::Effective.doc_label(), "ja");
        assert_eq!(
            Readiness::Pending("HUM-087".to_owned()).doc_label(),
            "offen (HUM-087)"
        );
    }

    #[test]
    fn issue_ids_have_a_prefix_and_enough_digits() {
        assert!(is_issue_id("HUM-101"));
        // Die Nummern laufen weiter: Eine Prüfung, die genau drei Ziffern
        // verlangt, macht das Register am Tag von HUM-1000 rot.
        assert!(is_issue_id("HUM-1000"));
        assert!(is_issue_id("HUM-999999"));
        assert!(!is_issue_id("HUM-10"));
        assert!(!is_issue_id("HUM-1234567"));
        assert!(!is_issue_id("hum-101"));
        assert!(!is_issue_id("HUM-abc"));
        assert!(!is_issue_id("HUM-"));
        assert!(!is_issue_id("101"));
    }

    #[test]
    fn a_four_digit_issue_parses_as_a_readiness() {
        let readiness = Readiness::parse("pending(HUM-1000)").expect("four digits are an issue");
        assert_eq!(readiness.issue(), Some("HUM-1000"));
        assert_eq!(readiness.to_string(), "pending(HUM-1000)");
        assert_eq!(readiness.doc_label(), "offen (HUM-1000)");
    }
}
