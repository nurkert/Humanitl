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
use crate::profile::Profile;

/// Die Ebene, aus der ein Wert stammt.
///
/// [`Origin::rank`] fasst die Varianten zu Bändern zusammen; innerhalb eines
/// Bandes sagt der Rang nichts, weil zwei Profil-Ebenen dasselbe Band teilen.
/// Wer wissen muss, welche von zwei Ebenen gewonnen hat, liest den
/// Ebenen-Index aus [`mod@crate::load`], nicht den Rang.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// Der eingebaute Vorgabewert aus `impl Default`.
    Default,
    /// Die globale `config.toml`.
    Global,
    /// Ein mitgeliefertes Profil in seiner eingebetteten Fassung, mit seinem Namen.
    ProfileBuiltin(String),
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
    /// Das Band, in dem die Ebene liegt; klein bedeutet: wird von jedem
    /// höheren Band überschrieben.
    ///
    /// Ein Band, keine Ordnung über alle Ebenen: Die beiden Profil-Ebenen
    /// (`default` und das gewählte Profil) teilen sich den Rang 2, denn welche
    /// von ihnen aus einer Datei und welche aus der Einbettung kommt, sagt
    /// nichts darüber, welche höher liegt — ein eigenes `default.toml` unter
    /// einem eingebetteten `llm-only` ist der Normalfall. Wer wissen muss,
    /// welche von zwei Ebenen gewonnen hat, fragt nicht den Rang, sondern die
    /// Reihenfolge, in der [`mod@crate::load`] sie auflegt; dort wird der
    /// Ebenen-Index mitgeführt.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Default => 0,
            Self::Global => 1,
            Self::ProfileBuiltin(_) | Self::ProfileGlobal(_) => 2,
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
            Self::ProfileBuiltin(_) => "profile_builtin",
            Self::ProfileGlobal(_) => "profile_global",
            Self::ProfileProject(_) => "profile_project",
            Self::Env(_) => "env",
            Self::Cli => "cli",
        }
    }

    /// Ob die Ebene ein Profil ist.
    #[must_use]
    pub const fn is_profile(&self) -> bool {
        matches!(
            self,
            Self::ProfileBuiltin(_) | Self::ProfileGlobal(_) | Self::ProfileProject(_)
        )
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::Global => f.write_str("config.toml"),
            Self::ProfileBuiltin(name) => write!(f, "profile builtin {name}"),
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
    /// Die Profile, die gewirkt haben, von der niedrigsten Ebene zur höchsten;
    /// das Projekt-Profil steht zuletzt.
    pub profiles: Vec<Profile>,
    /// Befunde, die das Laden überlebt hat: veraltete Schlüssel, Ersatzpfade.
    pub diagnostics: Vec<Diagnostic>,
}

impl Resolved {
    /// Die Kette der Profil-Ebenen, von der niedrigsten zur höchsten.
    ///
    /// Die Antwort auf „warum ist diese Sandbox so": Sie nennt jede Ebene, die
    /// ein Profil beigesteuert hat, in der Reihenfolge, in der sie aufgelegt
    /// wurden.
    #[must_use]
    pub fn profile_chain(&self) -> Vec<Origin> {
        self.profiles
            .iter()
            .map(|profile| profile.source.origin())
            .collect()
    }

    /// Das Profil einer Ebene, an seinem Namen.
    #[must_use]
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|profile| profile.source.name() == name)
    }

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

    /// Die beiden Profil-Ebenen liegen im selben Band. Ein Rang, der eine
    /// eingebettete Ebene grundsätzlich unter eine Datei stellte, wäre falsch:
    /// ein eigenes `default.toml` (Ebene 3) unter einem eingebetteten
    /// `llm-only` (Ebene 4) ist der Normalfall.
    #[test]
    fn the_two_profile_layers_share_one_band() {
        assert_eq!(
            Origin::ProfileBuiltin("llm-only".to_owned()).rank(),
            Origin::ProfileGlobal("default".to_owned()).rank()
        );
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
