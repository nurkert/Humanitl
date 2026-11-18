//! Die Projektwahl: was ein Mensch für ein Projekt gewählt hat (HUM-532,
//! ADR-0024 Punkt 1).
//!
//! Die Wahl ist eine Ebene auf dem Host. Der Daemon hält sie in seinem
//! Projektregister und schreibt sie nur über `Projects.save_choice`; sie liegt
//! nie im Projektordner. Das Projekt-Profil `<projekt>/.humanitl/profile.toml`
//! kommt dagegen mit jedem Klon und ist in einer Sitzung mit schreibbarem
//! Ordner für den Agenten schreibbar. Deshalb steht die Wahl über dem
//! Projekt-Profil und unter Umgebung und Kommandozeile (Herkunft
//! [`Origin::ProjectChoice`](crate::Origin::ProjectChoice), `project-choice`).
//!
//! # Die Grenze ist die Erlaubnisliste
//!
//! Die Wahl trägt genau die Schlüssel des Erststart-Bildschirms
//! ([`PROJECT_CHOICE_KEYS`]) und keinen anderen. Das Schema-Attribut
//! `x-project-scope` gilt für das Projekt-Profil aus dem Repository, nicht
//! für sie: `sandbox.backend` ist dort gesperrt und hier erlaubt, und ein
//! Schlüssel, den das Projekt-Profil setzen darf, ist hier trotzdem fremd,
//! wenn er nicht auf der Liste steht.
//!
//! Drei Fälle sind `CONFIG_024`, beim Speichern und beim Laden, und nie still
//! verworfen:
//!
//! 1. ein Schlüssel außerhalb der Liste,
//! 2. ein Wert, den eine Firmenvorgabe sperrt ([`PolicyLocks`]),
//! 3. ein `sandbox.min_isolation` unter der Untergrenze der Ebenen darunter;
//!    die Wahl hebt die Untergrenze, sie senkt sie nie.

use std::collections::BTreeMap;

use humanitl_core::diagnostics::codes::CONFIG_024;
use humanitl_core::{Diagnostic, Severity};

use crate::model::{Config, MinIsolation};
use crate::paths::Paths;

/// Die Schlüssel, die eine Projektwahl tragen darf: genau die des
/// Erststart-Bildschirms (ADR-0024 Punkt 1).
///
/// Wer diese Liste erweitert, erweitert, was ein Client je Projekt dauerhaft
/// bestimmen darf, und schreibt dazu, warum der neue Schlüssel keine Garantie
/// berührt.
pub const PROJECT_CHOICE_KEYS: [&str; 8] = [
    "agent.adapter",
    "sandbox.mode",
    "sandbox.work_mode",
    "sandbox.backend",
    "sandbox.min_isolation",
    "hold.ask_mode",
    crate::servers::SERVER,
    "sandbox.profile",
];

/// Der Schlüssel der Untergrenze, die eine Wahl nur heben darf.
const MIN_ISOLATION: &str = "sandbox.min_isolation";

/// Ob die Projektwahl diesen Schlüssel tragen darf.
#[must_use]
pub fn is_choice_key(key: &str) -> bool {
    PROJECT_CHOICE_KEYS.contains(&key)
}

/// Die Wahl eines Projekts: Schlüssel und Wert als Text, wie auf der
/// Kommandozeile; das Laden liest den Text nach dem Typ des Feldes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectChoice {
    values: BTreeMap<String, String>,
}

impl ProjectChoice {
    /// Eine leere Wahl.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Eine Wahl aus Paaren.
    ///
    /// # Errors
    ///
    /// `CONFIG_024` für einen Schlüssel außerhalb von [`PROJECT_CHOICE_KEYS`].
    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut values = BTreeMap::new();
        for (key, value) in pairs {
            let key = key.into();
            if !is_choice_key(&key) {
                return Err(foreign_key(&key));
            }
            values.insert(key, value.into());
        }
        Ok(Self { values })
    }

    /// Liest die Wahl, wie das Register sie speichert: ein JSON-Objekt aus
    /// Schlüssel und Text.
    ///
    /// # Errors
    ///
    /// `CONFIG_024`, wenn der Text kein solches Objekt ist oder einen
    /// Schlüssel außerhalb der Liste nennt.
    pub fn from_json(text: &str) -> Result<Self, Diagnostic> {
        let values: BTreeMap<String, String> = serde_json::from_str(text).map_err(|err| {
            refused(format!(
                "the stored project choice is not an object of keys and texts ({err}); save the \
                 choice again"
            ))
        })?;
        Self::from_pairs(values)
    }

    /// Die Wahl als JSON-Objekt, so wie das Register sie speichert.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.values).unwrap_or_else(|_err| "{}".to_owned())
    }

    /// Der gewählte Wert eines Schlüssels.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Alle Paare, nach Schlüssel geordnet.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// Ob nichts gewählt ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Ein Wert, den eine Firmenvorgabe festlegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedValue {
    /// Der einzige Wert, den der Schlüssel haben darf.
    pub value: String,
    /// Wer sperrt, so wie die Clients es nennen: die Vorgabedatei.
    pub by: String,
}

/// Die Sperren einer Firmenvorgabe über Schlüssel der Projektwahl.
///
/// Die Vorgabedatei gibt es noch nicht (BACKLOG Abschnitt 9, Punkt 21); bis
/// dahin ist das immer leer, wie [`crate::agents::SourceLock::of_policy`].
/// Tests setzen eine Sperre mit [`PolicyLocks::with`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyLocks {
    locks: BTreeMap<String, LockedValue>,
}

impl PolicyLocks {
    /// Keine Sperre.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Die Sperren aus der Firmen-Vorgabedatei; heute keine.
    #[must_use]
    pub fn of_policy(paths: &Paths) -> Self {
        let _ = paths;
        Self::none()
    }

    /// Fügt eine Sperre hinzu.
    #[must_use]
    pub fn with(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        by: impl Into<String>,
    ) -> Self {
        self.locks.insert(
            key.into(),
            LockedValue {
                value: value.into(),
                by: by.into(),
            },
        );
        self
    }

    /// Die Sperre eines Schlüssels.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&LockedValue> {
        self.locks.get(key)
    }
}

/// Prüft eine Wahl gegen die Konfiguration ohne sie (`below`) und gegen die
/// Sperren einer Firmenvorgabe.
///
/// Die Erlaubnisliste hält schon [`ProjectChoice`]; hier stehen die beiden
/// Prüfungen, die die Ebenen darunter brauchen. Der Typ eines Wertes ist
/// Sache des Ladens (`CONFIG_003`).
///
/// # Errors
///
/// `CONFIG_024` für einen gesperrten Wert und für eine Untergrenze unter der
/// von `below`.
pub fn check_choice(
    choice: &ProjectChoice,
    below: &Config,
    locks: &PolicyLocks,
) -> Result<(), Diagnostic> {
    for (key, value) in choice.iter() {
        if let Some(locked) = locks.get(key)
            && locked.value != value
        {
            return Err(refused(format!(
                "the project choice sets {key} = {value:?}, and the company policy {} fixes it \
                 to {:?}",
                locked.by, locked.value
            )));
        }
    }
    if let Some(wanted) = choice.get(MIN_ISOLATION)
        && let Some(level) = MinIsolation::parse(wanted)
        && level < below.sandbox.min_isolation
    {
        return Err(lowers_the_floor(
            wanted,
            below.sandbox.min_isolation.as_str(),
        ));
    }
    Ok(())
}

/// `CONFIG_024` für einen Schlüssel außerhalb der Liste.
#[must_use]
pub fn foreign_key(key: &str) -> Diagnostic {
    refused(format!(
        "the project choice may not carry {key}; it carries only {}",
        PROJECT_CHOICE_KEYS.join(", ")
    ))
}

/// `CONFIG_024` für eine Untergrenze, die die Wahl senken würde.
#[must_use]
pub fn lowers_the_floor(wanted: &str, floor: &str) -> Diagnostic {
    refused(format!(
        "the project choice sets {MIN_ISOLATION} = {wanted:?} below {floor:?}, which a layer \
         beneath it requires; a project choice only raises the floor"
    ))
}

fn refused(why: String) -> Diagnostic {
    Diagnostic::builder(CONFIG_024, Severity::Error)
        .why(why)
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{PolicyLocks, ProjectChoice, check_choice};
    use crate::model::{Config, MinIsolation};

    /// Mutationsbeweis: Prüfte [`super::is_choice_key`] nur, ob der
    /// Schlüssel im Projekt-Profil gesperrt ist („alles außer denied"),
    /// ginge `limits.hold_body_cap_bytes` durch, und der Test wird rot.
    #[test]
    fn a_key_outside_the_list_is_config_024_even_if_the_project_profile_may_set_it() {
        for key in [
            "limits.hold_body_cap_bytes",
            "hold.timeout_secs",
            "agent.command",
        ] {
            let refused = ProjectChoice::from_pairs([(key, "1")]).expect_err(key);
            assert_eq!(refused.code.as_str(), "CONFIG_024", "{key}");
            assert!(refused.why.contains(key), "{}", refused.why);
        }
        let stored = r#"{"sandbox.work_dir":"/etc"}"#;
        assert_eq!(
            ProjectChoice::from_json(stored).unwrap_err().code.as_str(),
            "CONFIG_024"
        );
    }

    #[test]
    fn the_choice_survives_the_round_trip_through_json() {
        let choice =
            ProjectChoice::from_pairs([("sandbox.work_mode", "ro"), ("hold.ask_mode", "terminal")])
                .unwrap();
        assert_eq!(ProjectChoice::from_json(&choice.to_json()).unwrap(), choice);
        assert_eq!(choice.get("sandbox.work_mode"), Some("ro"));
    }

    #[test]
    fn a_lower_floor_and_a_locked_value_are_config_024() {
        let mut below = Config::default();
        below.sandbox.min_isolation = MinIsolation::Namespaces;
        let lower = ProjectChoice::from_pairs([("sandbox.min_isolation", "policy")]).unwrap();
        let refused = check_choice(&lower, &below, &PolicyLocks::none()).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_024");
        let higher = ProjectChoice::from_pairs([("sandbox.min_isolation", "kernel")]).unwrap();
        check_choice(&higher, &below, &PolicyLocks::none()).unwrap();

        let locks =
            PolicyLocks::none().with("hold.ask_mode", "terminal", "/etc/humanitl/policy.toml");
        let other = ProjectChoice::from_pairs([("hold.ask_mode", "none")]).unwrap();
        let refused = check_choice(&other, &below, &locks).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_024");
        assert!(
            refused.why.contains("/etc/humanitl/policy.toml"),
            "{}",
            refused.why
        );
        let same = ProjectChoice::from_pairs([("hold.ask_mode", "terminal")]).unwrap();
        check_choice(&same, &below, &locks).unwrap();
    }
}
