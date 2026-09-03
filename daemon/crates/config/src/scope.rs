//! Die Vertrauensgrenze des Projekt-Profils. Zwei Werte, kein dritter.
//!
//! `<projekt>/.humanitl/profile.toml` liegt im geklonten Repository und ist
//! damit Angreifer-beeinflusst: Wer ein Repository klont, führt dessen Profil
//! aus. Vertrauensrelevante Schlüssel dürfen deshalb nur aus eingebauten
//! Vorgabewerten, globaler Config, globalem Profil, Umgebung oder Kommandozeile
//! kommen, nie aus dem Projekt-Profil (`backlog/CONVENTIONS.md` 4.11).
//!
//! Die Entscheidung steht am Feld, im erzeugten JSON-Schema unter
//! [`PROJECT_SCOPE_KEY`], so wie die Sichtbarkeitsstufe unter `x-tier`. Der
//! Test `every_node_has_a_project_scope` hält fest, dass kein Feld ohne
//! Entscheidung bleibt; ein Feld ohne Wert gilt beim Durchlauf als gesperrt.

use core::fmt;

/// Der Schlüssel, unter dem die Vertrauensgrenze im JSON-Schema steht.
pub const PROJECT_SCOPE_KEY: &str = "x-project-scope";

/// Ob ein Projekt-Profil eine Einstellung setzen darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectScope {
    /// Das Projekt-Profil darf den Wert setzen.
    Allowed,
    /// Nur Vorgabewerte, globale Config, globales Profil, Umgebung oder
    /// Kommandozeile dürfen den Wert setzen; im Projekt-Profil ist er `CONFIG_003`.
    Denied,
}

impl ProjectScope {
    /// Beide Werte.
    pub const ALL: [Self; 2] = [Self::Allowed, Self::Denied];

    /// Der Wert, wie er im Schema steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
        }
    }

    /// Liest die Vertrauensgrenze aus dem Schema-Wert.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scope| scope.as_str() == value)
    }
}

impl fmt::Display for ProjectScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::ProjectScope;

    #[test]
    fn parse_round_trips_both_values() {
        for scope in ProjectScope::ALL {
            assert_eq!(ProjectScope::parse(scope.as_str()), Some(scope));
        }
        assert_eq!(ProjectScope::parse("maybe"), None);
    }
}
