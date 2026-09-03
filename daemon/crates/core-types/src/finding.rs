//! Fundstellen: was ein Detektor in einer Anfrage gefunden hat.
//!
//! Ein [`Finding`] trägt nie den gefundenen Wert. Es trägt seinen Hash, damit
//! zwei Funde als gleich erkannt werden können, und einen kurzen Anfang für die
//! Anzeige. Der Wert selbst bleibt im Body und geht weder in ein Ereignis noch
//! in die Audit-Datei.

use core::fmt;
use core::ops::Range;

use serde::{Deserialize, Serialize};

use crate::hex;
use crate::http::{HeaderName, sha256};

/// So viele Zeichen des Originals stehen höchstens in [`Finding::display_prefix`].
pub const DISPLAY_PREFIX_CHARS: usize = 8;

/// Wie sicher ein Fund ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Prüfsumme oder Struktur bestätigt den Fund (IBAN, Kreditkarte, JWT).
    Checksum,
    /// Ein Muster hat gegriffen, ohne Bestätigung.
    Regex,
    /// Ein Begriff, den der Nutzer selbst hinterlegt hat.
    UserTerm,
}

impl Tier {
    /// Kurzname in `snake_case`, wie er in Ereignissen und in der Oberfläche steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Checksum => "checksum",
            Self::Regex => "regex",
            Self::UserTerm => "user_term",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Art des Fundes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// Ein Schlüssel eines benannten Anbieters, zum Beispiel `github`.
    ApiKey(String),
    /// Ein JSON Web Token.
    Jwt,
    /// Eine E-Mail-Adresse.
    Email,
    /// Eine IBAN.
    Iban,
    /// Eine Kreditkartennummer.
    CreditCard,
    /// Eine Telefonnummer.
    Phone,
    /// Eine IPv4-Adresse.
    Ipv4,
    /// Ein Begriff aus `findings.user_terms`.
    UserTerm(String),
    /// Ein Fund aus einem Detektor ohne eigene Variante.
    Custom(String),
}

impl FindingKind {
    /// Kurzname der Art in `snake_case`, ohne den Parameter.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey(_) => "api_key",
            Self::Jwt => "jwt",
            Self::Email => "email",
            Self::Iban => "iban",
            Self::CreditCard => "credit_card",
            Self::Phone => "phone",
            Self::Ipv4 => "ipv4",
            Self::UserTerm(_) => "user_term",
            Self::Custom(_) => "custom",
        }
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(name) | Self::UserTerm(name) | Self::Custom(name) => {
                write!(f, "{}:{name}", self.as_str())
            }
            _ => f.write_str(self.as_str()),
        }
    }
}

/// Wo in der Anfrage der Fund liegt.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FindingLocation {
    /// In einem Header, benannt durch dessen Namen.
    Header(HeaderName),
    /// In der Query der URL.
    Query,
    /// Im Body.
    Body,
}

impl FindingLocation {
    /// Kurzname des Orts, bei einem Header dessen Name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Header(name) => name.as_str(),
            Self::Query => "query",
            Self::Body => "body",
        }
    }
}

impl fmt::Display for FindingLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(name) => write!(f, "header:{}", name.as_str()),
            Self::Query => f.write_str("query"),
            Self::Body => f.write_str("body"),
        }
    }
}

/// Ein einzelner Fund in einer Anfrage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Was gefunden wurde.
    pub kind: FindingKind,
    /// Byte-Bereich innerhalb des Orts.
    pub span: Range<usize>,
    /// Wo der Fund liegt.
    pub location: FindingLocation,
    /// Wie sicher der Fund ist.
    pub tier: Tier,
    /// SHA-256 über die Bytes des gefundenen Werts.
    pub value_hash: [u8; 32],
    /// Höchstens [`DISPLAY_PREFIX_CHARS`] Zeichen des Originals, danach `…`.
    pub display_prefix: String,
}

impl Finding {
    /// Baut einen Fund aus dem gefundenen Wert.
    ///
    /// Der Wert wird gehasht und gekürzt; er wird nicht gespeichert.
    #[must_use]
    pub fn new(
        kind: FindingKind,
        span: Range<usize>,
        location: FindingLocation,
        tier: Tier,
        value: &str,
    ) -> Self {
        Self {
            kind,
            span,
            location,
            tier,
            value_hash: sha256(value.as_bytes()),
            display_prefix: display_prefix(value),
        }
    }

    /// Der Hash des Werts als Kleinbuchstaben-Hex, 64 Zeichen.
    #[must_use]
    pub fn value_hash_hex(&self) -> String {
        hex::encode(&self.value_hash)
    }
}

/// Kürzt einen Wert für die Anzeige.
///
/// Höchstens [`DISPLAY_PREFIX_CHARS`] Zeichen des Originals, danach `…`. Ein
/// kürzerer Wert bleibt unverändert und bekommt kein `…`. Gezählt wird in
/// Zeichen, nicht in Bytes, damit ein mehrbyteiges Zeichen nicht zerschnitten
/// wird.
#[must_use]
pub fn display_prefix(value: &str) -> String {
    let mut out: String = value.chars().take(DISPLAY_PREFIX_CHARS).collect();
    if value.chars().nth(DISPLAY_PREFIX_CHARS).is_some() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{
        DISPLAY_PREFIX_CHARS, Finding, FindingKind, FindingLocation, Tier, display_prefix,
    };
    use crate::http::HeaderName;

    #[test]
    fn display_prefix_never_exceeds_eight_characters_plus_ellipsis() {
        assert_eq!(display_prefix("ghp_0123456789abcdef"), "ghp_0123…");
        assert_eq!(display_prefix("short"), "short");
        assert_eq!(display_prefix("12345678"), "12345678");
        assert_eq!(display_prefix("123456789"), "12345678…");
        assert_eq!(display_prefix(""), "");
        let prefix = display_prefix("äöüäöüäöüäöü");
        assert_eq!(prefix.chars().count(), DISPLAY_PREFIX_CHARS + 1);
    }

    #[test]
    fn finding_hashes_the_value_and_keeps_it_out_of_the_struct() {
        let finding = Finding::new(
            FindingKind::ApiKey("github".to_owned()),
            0..20,
            FindingLocation::Header(HeaderName::from_static("authorization")),
            Tier::Checksum,
            "ghp_0123456789abcdef",
        );
        assert_eq!(finding.value_hash_hex().len(), 64);
        assert_eq!(finding.display_prefix, "ghp_0123…");
        assert_eq!(finding.kind.as_str(), "api_key");
        assert_eq!(finding.location.as_str(), "authorization");
        assert_eq!(finding.tier.as_str(), "checksum");
        let same = Finding::new(
            FindingKind::ApiKey("github".to_owned()),
            0..20,
            FindingLocation::Query,
            Tier::Checksum,
            "ghp_0123456789abcdef",
        );
        assert_eq!(finding.value_hash, same.value_hash);
    }

    #[test]
    fn kind_display_carries_the_parameter() {
        assert_eq!(
            FindingKind::UserTerm("projektname".to_owned()).to_string(),
            "user_term:projektname"
        );
        assert_eq!(FindingKind::Jwt.to_string(), "jwt");
        assert_eq!(FindingLocation::Body.to_string(), "body");
    }
}
