//! Woher ein Wert kommt.
//!
//! Ohne Herkunft ist eine aufgelöste Konfiguration eine Behauptung: Der Nutzer
//! sieht 42 und weiß nicht, ob er das selbst eingetragen hat, ob es aus einem
//! Profil kommt oder ob eine Umgebungsvariable aus einem Skript mitredet. Jede
//! Auflösung merkt sich deshalb für jedes Blattfeld die Ebene, die den Wert
//! zuletzt gesetzt hat.

use core::fmt;
use std::collections::BTreeMap;
use std::path::PathBuf;

use humanitl_core::Diagnostic;

use crate::model::Config;

/// Die Ebene, aus der ein Wert stammt.
///
/// Die Reihenfolge der Varianten ist die Rangfolge: eine spätere Variante
/// überschreibt eine frühere. [`Origin::rank`] macht das vergleichbar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// Der eingebaute Vorgabewert aus `impl Default`.
    Default,
    /// Die globale `config.toml`.
    Global,
    /// Ein Profil aus dem Konfigurationsverzeichnis, mit seinem Namen.
    ProfileGlobal(String),
    /// Das Profil des Projekts, mit seinem Pfad.
    ProfileProject(PathBuf),
    /// Eine Umgebungsvariable, mit ihrem Namen.
    Env(String),
    /// Ein Argument der Kommandozeile.
    Cli,
}

impl Origin {
    /// Der Rang der Ebene, klein bedeutet: wird von allem darüber überschrieben.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Default => 0,
            Self::Global => 1,
            Self::ProfileGlobal(_) => 2,
            Self::ProfileProject(_) => 3,
            Self::Env(_) => 4,
            Self::Cli => 5,
        }
    }

    /// Kurzname der Ebene in `snake_case`, ohne den veränderlichen Teil.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Global => "global",
            Self::ProfileGlobal(_) => "profile_global",
            Self::ProfileProject(_) => "profile_project",
            Self::Env(_) => "env",
            Self::Cli => "cli",
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::Global => f.write_str("config.toml"),
            Self::ProfileGlobal(name) => write!(f, "profile {name}"),
            Self::ProfileProject(path) => write!(f, "project profile {}", path.display()),
            Self::Env(key) => write!(f, "env {key}"),
            Self::Cli => f.write_str("command line"),
        }
    }
}

/// Eine aufgelöste Konfiguration mit Herkunft je Blattfeld.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    /// Die zusammengesetzte Konfiguration.
    pub config: Config,
    /// Für jedes Blattfeld des Schemas die Ebene, die den Wert gesetzt hat.
    pub origins: BTreeMap<String, Origin>,
    /// Befunde, die das Laden überlebt hat: veraltete Schlüssel, Ersatzpfade.
    pub diagnostics: Vec<Diagnostic>,
}

impl Resolved {
    /// Die Herkunft eines Feldes, angesprochen mit seinem Pfad, zum Beispiel
    /// `hold.timeout_secs`.
    ///
    /// Ein Pfad, den das Schema nicht kennt, hat keine Herkunft.
    #[must_use]
    pub fn origin(&self, path: &str) -> Option<&Origin> {
        self.origins.get(path)
    }

    /// Alle Felder, die nicht mehr auf dem Vorgabewert stehen, in Pfad-Reihenfolge.
    #[must_use]
    pub fn changed(&self) -> Vec<(&str, &Origin)> {
        self.origins
            .iter()
            .filter(|(_, origin)| **origin != Origin::Default)
            .map(|(path, origin)| (path.as_str(), origin))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use super::Origin;

    #[test]
    fn ranks_follow_the_precedence_of_conventions_44() {
        let ladder = [
            Origin::Default,
            Origin::Global,
            Origin::ProfileGlobal("work".to_owned()),
            Origin::ProfileProject(PathBuf::from("/p/.humanitl/profile.toml")),
            Origin::Env("HUMANITL_HOLD__TIMEOUT_SECS".to_owned()),
            Origin::Cli,
        ];
        for pair in ladder.windows(2) {
            assert!(
                pair[0].rank() < pair[1].rank(),
                "{:?} must rank below {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn display_names_the_concrete_source() {
        assert_eq!(Origin::Global.to_string(), "config.toml");
        assert_eq!(
            Origin::Env("HUMANITL_UI__THEME".to_owned()).to_string(),
            "env HUMANITL_UI__THEME"
        );
        assert_eq!(Origin::Cli.kind(), "cli");
    }
}
