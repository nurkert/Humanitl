//! Der `Content-Type` einer Anfrage als Wert.
//!
//! Nur so viel, wie der Scan braucht: der Kern des Typs (`type/subtype`, klein
//! geschrieben, ohne Parameter) und die Frage, ob der Body als Text durchsucht
//! wird oder im „strings"-Modus. Eine eigene Abhängigkeit auf `mime` wäre eine
//! Abstraktion über eine Fremdbibliothek für zwei Vergleiche.

use core::fmt;

/// Der Kern eines `Content-Type`, klein geschrieben und ohne Parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentType {
    essence: String,
}

impl ContentType {
    /// Liest den Kopfzeilen-Wert.
    ///
    /// Alles ab dem ersten `;` ist Parameter und fällt weg; Leerraum wird
    /// entfernt, Großschreibung normalisiert.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let essence = value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        Self { essence }
    }

    /// Der Kern, zum Beispiel `application/json`.
    #[must_use]
    pub fn essence(&self) -> &str {
        &self.essence
    }

    /// Der Untertyp hinter dem `/`, zum Beispiel `json`.
    #[must_use]
    pub fn subtype(&self) -> &str {
        self.essence
            .split_once('/')
            .map_or("", |(_, subtype)| subtype)
    }

    /// Wahr für Typen, deren Body vollständig als Text durchsucht wird.
    ///
    /// `text/*`, `application/json`, `application/x-www-form-urlencoded`,
    /// `application/xml` sowie jeder Untertyp mit dem Suffix `+json` oder
    /// `+xml`.
    #[must_use]
    pub fn is_textual(&self) -> bool {
        if self.essence.starts_with("text/") {
            return true;
        }
        if matches!(
            self.essence.as_str(),
            "application/json"
                | "application/x-www-form-urlencoded"
                | "application/xml"
                | "application/javascript"
                | "application/graphql"
        ) {
            return true;
        }
        let subtype = self.subtype();
        subtype.ends_with("+json") || subtype.ends_with("+xml")
    }

    /// Wahr für `multipart/*`.
    ///
    /// Ein multipart-Body wird im „strings"-Modus durchsucht: die Textteile
    /// bleiben vollständig erhalten, die Binärteile fallen heraus, ohne dass
    /// diese Crate einen multipart-Parser braucht (Nicht-Ziel des Issues).
    #[must_use]
    pub fn is_multipart(&self) -> bool {
        self.essence.starts_with("multipart/")
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.essence)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::ContentType;

    #[test]
    fn parameters_and_case_fall_away() {
        let parsed = ContentType::parse("Application/JSON; charset=UTF-8");
        assert_eq!(parsed.essence(), "application/json");
        assert_eq!(parsed.subtype(), "json");
        assert_eq!(parsed.to_string(), "application/json");
    }

    #[test]
    fn textual_types_are_scanned_completely() {
        for value in [
            "text/plain",
            "text/html; charset=utf-8",
            "application/json",
            "application/x-www-form-urlencoded",
            "application/xml",
            "application/vnd.api+json",
            "image/svg+xml",
        ] {
            assert!(ContentType::parse(value).is_textual(), "{value}");
        }
        for value in [
            "application/octet-stream",
            "image/png",
            "multipart/form-data",
        ] {
            assert!(!ContentType::parse(value).is_textual(), "{value}");
        }
        assert!(ContentType::parse("multipart/form-data; boundary=x").is_multipart());
    }
}
