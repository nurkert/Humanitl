//! Die Einstellungen, die ein Scan braucht.
//!
//! Diese Crate darf nur von `humanitl-core` abhängen (`backlog/CONVENTIONS.md`
//! 3.1, `tools/deps-allow.toml`), kennt also `humanitl_config::FindingsConfig`
//! nicht. Deshalb steht hier der Wertetyp, den der Proxy aus der Konfiguration
//! füllt: die vier Schlüssel `findings.*` und die zwei Budgets aus `limits.*`,
//! die den Body-Scan begrenzen.
//!
//! `findings.enabled` ist ein Schalter, keine Liste von Detektor-Kennungen. Die
//! Spezifikation von HUM-025 nennt eine Liste, `backlog/CONVENTIONS.md` 4.11
//! und das Schema aus HUM-062 nennen den Schalter mit der Vorgabe `true`;
//! Abschnitt 4 gewinnt. Wer einzelne Detektoren abschalten will, baut die
//! Registry mit [`crate::DetectorRegistry::empty`] und hängt mit
//! [`crate::DetectorRegistry::register`] genau die ein, die er will.

use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, Severity};

/// Vorgabe für [`FindingsSettings::cap_bytes`], entspricht `limits.preview_cap_bytes`.
pub const DEFAULT_CAP_BYTES: usize = 8 * 1024 * 1024;

/// Vorgabe für [`FindingsSettings::max_decompress_ratio`], entspricht `limits.max_decompress_ratio`.
pub const DEFAULT_MAX_DECOMPRESS_RATIO: u32 = 100;

/// Was der Scan über die Konfiguration wissen muss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingsSettings {
    /// `findings.enabled`. Aus bedeutet: keine Detektoren, keine Markierungen.
    pub enabled: bool,
    /// `findings.user_terms`: eigene Begriffe, die nie nach außen sollen.
    pub user_terms: Vec<String>,
    /// `findings.email_allow_domains`: Domains, deren Mailadressen kein Fund sind.
    pub email_allow_domains: Vec<String>,
    /// `findings.ignored_hashes`, schon aus Hex gelesen.
    pub ignored_hashes: Vec<[u8; 32]>,
    /// `limits.preview_cap_bytes`: so viele Bytes je Suchziel werden gelesen.
    pub cap_bytes: usize,
    /// `limits.max_decompress_ratio`: größtes Verhältnis von entpackten zu gepackten Bytes.
    pub max_decompress_ratio: u32,
}

impl Default for FindingsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            user_terms: Vec::new(),
            email_allow_domains: Vec::new(),
            ignored_hashes: Vec::new(),
            cap_bytes: DEFAULT_CAP_BYTES,
            max_decompress_ratio: DEFAULT_MAX_DECOMPRESS_RATIO,
        }
    }
}

impl FindingsSettings {
    /// Setzt den Schalter `findings.enabled`.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Setzt die eigenen Begriffe.
    #[must_use]
    pub fn with_user_terms<I, S>(mut self, terms: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.user_terms = terms.into_iter().map(Into::into).collect();
        self
    }

    /// Setzt die Domains, deren Mailadressen kein Fund sind.
    #[must_use]
    pub fn with_email_allow_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.email_allow_domains = domains.into_iter().map(Into::into).collect();
        self
    }

    /// Liest `findings.ignored_hashes` aus der Hex-Schreibweise.
    ///
    /// # Errors
    ///
    /// `CONFIG_003`, wenn ein Eintrag keine 64 Hex-Zeichen sind. Der Wert steht
    /// im Grund; er ist ein Hash, kein Geheimnis.
    pub fn with_ignored_hashes_hex<I, S>(mut self, hashes: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut parsed = Vec::new();
        for hash in hashes {
            parsed.push(parse_hash(hash.as_ref())?);
        }
        self.ignored_hashes = parsed;
        Ok(self)
    }

    /// Setzt die zwei Budgets aus `limits.*`.
    #[must_use]
    pub const fn with_limits(mut self, cap_bytes: usize, max_decompress_ratio: u32) -> Self {
        self.cap_bytes = cap_bytes;
        self.max_decompress_ratio = max_decompress_ratio;
        self
    }
}

/// Liest einen SHA-256 aus 64 Hex-Zeichen, Groß- und Kleinschreibung egal.
///
/// # Errors
///
/// `CONFIG_003`, wenn Länge oder Zeichen nicht stimmen.
pub fn parse_hash(text: &str) -> Result<[u8; 32], Diagnostic> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err(hash_error(text, "64 Hex-Zeichen erwartet"));
    }
    let mut out = [0u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        let (Some(high), Some(low)) = (nibble(pair[0]), nibble(pair[1])) else {
            return Err(hash_error(text, "nur 0-9, a-f und A-F erlaubt"));
        };
        out[index] = (high << 4) | low;
    }
    Ok(out)
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hash_error(value: &str, why: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "findings.ignored_hashes enthält \"{value}\": {why}"
        ))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{FindingsSettings, parse_hash};

    #[test]
    fn hashes_are_read_from_hex_in_both_cases() {
        let hex = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF";
        let parsed = parse_hash(hex).unwrap();
        assert_eq!(parsed[0], 0x00);
        assert_eq!(parsed[1], 0x11);
        assert_eq!(parsed[31], 0xff);
    }

    #[test]
    fn a_short_or_odd_hash_is_config_003() {
        assert_eq!(
            parse_hash("deadbeef").unwrap_err().code.as_str(),
            "CONFIG_003"
        );
        let wrong = "zz112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        assert_eq!(parse_hash(wrong).unwrap_err().code.as_str(), "CONFIG_003");
    }

    #[test]
    fn the_builders_fill_the_four_findings_keys() {
        let hex = "00".repeat(32);
        let settings = FindingsSettings::default()
            .with_enabled(true)
            .with_user_terms(["Acme"])
            .with_email_allow_domains(["example.com"])
            .with_ignored_hashes_hex([hex])
            .unwrap()
            .with_limits(1024, 10);
        assert!(settings.enabled);
        assert_eq!(settings.user_terms, vec!["Acme".to_owned()]);
        assert_eq!(settings.email_allow_domains, vec!["example.com".to_owned()]);
        assert_eq!(settings.ignored_hashes, vec![[0u8; 32]]);
        assert_eq!(settings.cap_bytes, 1024);
        assert_eq!(settings.max_decompress_ratio, 10);
    }
}
