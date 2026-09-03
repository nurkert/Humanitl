//! Sichtbarkeitsstufen. Drei Stufen, keine vierte.
//!
//! Die Stufe steht im erzeugten JSON-Schema unter [`TIER_KEY`] an jedem Feld.
//! Der Einstellungs-Bildschirm zeigt `basic` immer, `advanced` hinter einem
//! Schalter und `expert` nur in der Textdatei und in `docs/CONFIG.md`.

use core::fmt;

/// Der Schlüssel, unter dem die Stufe im JSON-Schema steht.
pub const TIER_KEY: &str = "x-tier";

/// Wie sichtbar eine Einstellung ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    /// Sieht jeder, ohne zu suchen.
    Basic,
    /// Hinter einem Schalter „Mehr anzeigen".
    Advanced,
    /// Nur in der Textdatei und in der Doku.
    Expert,
}

impl Tier {
    /// Alle Stufen, von der sichtbarsten zur verstecktesten.
    pub const ALL: [Self; 3] = [Self::Basic, Self::Advanced, Self::Expert];

    /// Der Wert, wie er im Schema steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Advanced => "advanced",
            Self::Expert => "expert",
        }
    }

    /// Liest eine Stufe aus dem Schema-Wert.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tier| tier.as_str() == value)
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::Tier;

    #[test]
    fn parse_round_trips_every_tier() {
        for tier in Tier::ALL {
            assert_eq!(Tier::parse(tier.as_str()), Some(tier));
        }
        assert_eq!(Tier::parse("secret"), None);
    }

    #[test]
    fn tiers_are_ordered_from_visible_to_hidden() {
        assert!(Tier::Basic < Tier::Advanced);
        assert!(Tier::Advanced < Tier::Expert);
    }
}
