//! Die Werte eines Katalogeintrags, so wie sie in `catalog/domains.yaml` stehen.
//!
//! Ein Eintrag ist eine Behauptung über einen Dienst, und eine Behauptung ohne
//! Beleg ist im Domain-Panel wertlos (`backlog/CONVENTIONS.md` 4.13). Deshalb
//! trägt jeder Eintrag ein Pflichtfeld [`CatalogEntry::source`]: die Adresse
//! beim Betreiber selbst, unter der nachlesbar ist, was der Eintrag über den
//! Dienst sagt. Wer der Karte nicht glaubt, kann sie prüfen.
//!
//! Die Texte stehen zweisprachig als [`Text`]. Sie sind Daten, keine
//! Oberflächen-Strings: Sie wachsen mit dem Katalog auf zweihundert Einträge,
//! sie gehören zum Datensatz und nicht zur Anwendung, und die Oberfläche liest
//! dieselbe Datei als Asset. In `app_en.arb` und `app_de.arb` stehen deshalb
//! die Beschriftungen der Karte, nie die Beschreibung eines Dienstes.

use serde::{Deserialize, Serialize};

/// Wofür ein Dienst da ist.
///
/// Die Kategorie ist die einzige Einordnung, die der Katalog vornimmt. Sie ist
/// keine Bewertung: `ai` sagt, dass dort ein Modell antwortet, nicht dass das
/// gefährlich oder harmlos wäre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Quellcode-Hosting.
    Scm,
    /// Paket-Registry einer Sprache oder für Container-Images.
    Registry,
    /// Dokumentation, Referenz, Frage-und-Antwort.
    Docs,
    /// Bau- und Testdienste.
    Ci,
    /// Allgemeine Cloud-Dienste.
    Cloud,
    /// Modelle, Modell-Endpunkte, Modell-Kataloge.
    Ai,
    /// Auslieferungsnetz für Dateien.
    Cdn,
    /// Websuche.
    Search,
    /// Paketquellen einer Linux-Distribution.
    Os,
    /// Alles, was in keine der Kategorien darüber passt.
    Other,
}

impl Category {
    /// Alle Kategorien, in der Reihenfolge der Deklaration.
    pub const ALL: [Self; 10] = [
        Self::Scm,
        Self::Registry,
        Self::Docs,
        Self::Ci,
        Self::Cloud,
        Self::Ai,
        Self::Cdn,
        Self::Search,
        Self::Os,
        Self::Other,
    ];

    /// Kurzname in `snake_case`, wie er in `domains.yaml` steht.
    ///
    /// Die Oberfläche hängt ihn an `catalogCategory` an und bekommt so den
    /// ARB-Schlüssel der Beschriftung.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scm => "scm",
            Self::Registry => "registry",
            Self::Docs => "docs",
            Self::Ci => "ci",
            Self::Cloud => "cloud",
            Self::Ai => "ai",
            Self::Cdn => "cdn",
            Self::Search => "search",
            Self::Os => "os",
            Self::Other => "other",
        }
    }
}

impl core::fmt::Display for Category {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein Satz Text in beiden Sprachen der Oberfläche.
///
/// `en` ist die Quelle, `de` die Übersetzung, wie bei den ARB-Dateien. Beide
/// sind Pflicht: ein fehlender Text würde in der einen Sprache eine leere Zeile
/// zeigen, und eine leere Zeile liest sich wie „nichts bekannt", obwohl etwas
/// bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    /// Englisch, die Quellsprache.
    pub en: String,
    /// Deutsch.
    pub de: String,
}

impl Text {
    /// Der Text in der gewünschten Sprache, sonst der englische.
    ///
    /// `lang` ist ein Sprach-Tag wie `de` oder `de-AT`; verglichen wird nur das
    /// erste Teilstück.
    #[must_use]
    pub fn get(&self, lang: &str) -> &str {
        let primary = lang.split(['-', '_']).next().unwrap_or(lang);
        if primary.eq_ignore_ascii_case("de") {
            &self.de
        } else {
            &self.en
        }
    }
}

/// Ein Eintrag des Katalogs.
///
/// Die Feldnamen sind die Schlüssel in `catalog/domains.yaml`; das JSON-Schema
/// daneben (`catalog/domains.schema.json`) beschreibt dieselbe Form für die
/// Prüfung in CI und für die Oberfläche, die die Datei als Asset liest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    /// Stabiler Bezeichner, `[a-z0-9-]+`. Er reist im Ereignisstrom als
    /// `DomainInfo.catalog_id`; die Oberfläche schlägt damit den Eintrag in
    /// ihrer eigenen Kopie der Datei nach.
    pub id: String,
    /// Der Name, unter dem der Dienst sich selbst führt.
    pub name: String,
    /// Host-Muster in der Schreibweise der Regeln (`backlog/CONVENTIONS.md`
    /// 3.3): ein Name ohne Stern trifft exakt, `**.example.com` trifft
    /// `example.com` und jede Unterdomain. Verglichen werden ganze Labels, nie
    /// Zeichenketten.
    pub hosts: Vec<String>,
    /// Wofür der Dienst da ist.
    pub category: Category,
    /// Ein Satz: was der Dienst ist.
    pub description: Text,
    /// Womit ein Agent hier landet, in seiner eigenen Schreibweise
    /// (`npm install`, `git clone`). Keine Prosa, keine Übersetzung.
    pub typical: Vec<String>,
    /// Dateiname unter `catalog/icons/`. Fehlt die Datei, zeigt die Oberfläche
    /// den Globus.
    pub icon: String,
    /// Startseite des Dienstes.
    pub homepage: String,
    /// Wo die Beschreibung herkommt: eine Seite des Betreibers, auf der steht,
    /// was dieser Eintrag behauptet. Pflicht, damit die Karte prüfbar bleibt.
    pub source: String,
    /// Was ein Mensch wissen sollte, bevor er hier etwas freigibt. Nur gesetzt,
    /// wo es etwas Konkretes zu sagen gibt; kein Feld für allgemeine Warnungen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_note: Option<Text>,
}

/// Der Inhalt von `catalog/domains.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFile {
    /// Format-Version der Datei. Zurzeit ist nur `1` bekannt.
    pub version: u32,
    /// Die Einträge, in der Reihenfolge der Datei. Bei zwei passenden Mustern
    /// gewinnt der erste Eintrag.
    pub entries: Vec<CatalogEntry>,
}

/// Die Format-Version, die diese Fassung des Codes lesen kann.
pub const FORMAT_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Category, Text};

    #[test]
    fn category_round_trips_through_its_short_name() {
        for category in Category::ALL {
            let yaml = serde_yaml_ng::to_string(&category).unwrap();
            assert_eq!(yaml.trim(), category.as_str());
            let back: Category = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(back, category);
        }
    }

    #[test]
    fn text_falls_back_to_english() {
        let text = Text {
            en: "English".to_owned(),
            de: "Deutsch".to_owned(),
        };
        assert_eq!(text.get("de"), "Deutsch");
        assert_eq!(text.get("de-AT"), "Deutsch");
        assert_eq!(text.get("DE"), "Deutsch");
        assert_eq!(text.get("en"), "English");
        assert_eq!(text.get("fr"), "English");
    }
}
