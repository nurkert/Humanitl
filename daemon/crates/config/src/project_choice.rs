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
//! 2. ein Schlüssel, den eine Firmenvorgabe sperrt ([`PolicyLocks`]), mit
//!    welchem Wert auch immer,
//! 3. ein `sandbox.min_isolation` unter der Untergrenze der Ebenen darunter;
//!    die Wahl hebt die Untergrenze, sie senkt sie nie. Ebenso ein
//!    `llm.cloud = "allow"`, wo eine Ebene darunter `block` setzt (GitHub
//!    #113): Die Wahl schaltet die Sperre ein, nie aus.

use std::collections::BTreeMap;

use humanitl_core::diagnostics::codes::CONFIG_024;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::agents::{Locked, SOURCE_KEYS, SourceLock};
use crate::model::Config;
pub use crate::policy::{LockedValue, PolicyLocks};

/// Die Schlüssel des Erststart-Bildschirms ohne die Quelle der Agenten.
const SCREEN_KEYS: [&str; 8] = [
    "agent.adapter",
    "sandbox.mode",
    "sandbox.work_mode",
    "sandbox.backend",
    "sandbox.min_isolation",
    "hold.ask_mode",
    crate::servers::SERVER,
    "sandbox.profile",
];

/// Schlüssel der Wahl, die nur verschärfen ([`crate::floor`]) und auf keinem
/// Feld des Erststart-Bildschirms stehen: die Sperre der Cloud-Inferenz
/// (GitHub #113), die die Einstellungen je Projekt einschalten.
const STRICTER_KEYS: [&str; 1] = [crate::floor::LLM_CLOUD];

/// Die Länge von [`PROJECT_CHOICE_KEYS`].
const CHOICE_LEN: usize = SCREEN_KEYS.len() + SOURCE_KEYS.len() + STRICTER_KEYS.len();

/// Die Schlüssel, die eine Projektwahl tragen darf: genau die des
/// Erststart-Bildschirms (ADR-0024 Punkt 1), dazu je bekanntem Agenten
/// `agents.<id>.source` (GitHub #15, Nachtrag zu ADR-0023) und `llm.cloud`
/// (GitHub #113).
///
/// Wer diese Liste erweitert, erweitert, was ein Client je Projekt dauerhaft
/// bestimmen darf, und schreibt dazu, warum der neue Schlüssel keine Garantie
/// berührt. Die Quelle berührt keine: Sie wählt nur, woher das Programm
/// kommt; beide Quellen laufen in derselben Sandbox, ein Start fällt nie auf
/// die andere zurück, und eine Sperre der Firmenvorgabe auf die Quelle hält
/// sie aus der Wahl ([`check_source_lock`]). `llm.cloud` berührt keine, weil
/// über alle Ebenen der strengste Wert gilt: Die Wahl kann die Sperre nur
/// einschalten, ein `allow` unter einem `block` darunter ist `CONFIG_024`.
pub const PROJECT_CHOICE_KEYS: [&str; CHOICE_LEN] = choice_keys();

/// [`SCREEN_KEYS`], danach [`SOURCE_KEYS`] und [`STRICTER_KEYS`].
const fn choice_keys() -> [&'static str; CHOICE_LEN] {
    let mut keys = [""; CHOICE_LEN];
    let mut index = 0;
    while index < SCREEN_KEYS.len() {
        keys[index] = SCREEN_KEYS[index];
        index += 1;
    }
    let mut source = 0;
    while source < SOURCE_KEYS.len() {
        keys[index] = SOURCE_KEYS[source];
        index += 1;
        source += 1;
    }
    let mut stricter = 0;
    while stricter < STRICTER_KEYS.len() {
        keys[index] = STRICTER_KEYS[stricter];
        index += 1;
        stricter += 1;
    }
    keys
}

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
        // Ein gesperrter Schlüssel ist nur lesbar, auch mit dem Wert der
        // Vorgabe (HUM-534, wie `SetConfig`): Die Wahl trägt ihn nie.
        if let Some(locked) = locks.get(key) {
            return Err(refused(format!(
                "the project choice sets {key} = {value:?}, and the company policy {} fixes it \
                 to {:?}; a locked key is read only and stays out of the choice",
                locked.by, locked.value
            )));
        }
    }
    for (key, wanted) in choice.iter() {
        let Some(floor) = crate::floor::value_in(below, key) else {
            continue;
        };
        if let (Some(level), Some(now)) = (
            crate::floor::rank(key, wanted),
            crate::floor::rank(key, floor),
        ) && level < now
        {
            return Err(lowers_the_floor(key, wanted, floor));
        }
    }
    Ok(())
}

/// Prüft die Quellen einer Wahl gegen die Sperre der Quelle einer
/// Firmenvorgabe (ADR-0023 Punkt 1, Nachtrag GitHub #15): Ein
/// `agents.<id>.source` eines gesperrten Agenten ist nur lesbar und steht nie
/// in der Wahl, mit welchem Wert auch immer, wie ein gesperrter Schlüssel in
/// [`check_choice`].
///
/// # Errors
///
/// `CONFIG_024`, dessen `why` die Vorgabe nennt.
pub fn check_source_lock(choice: &ProjectChoice, lock: &SourceLock) -> Result<(), Diagnostic> {
    for (key, value) in choice.iter() {
        let Some(agent) = key
            .strip_prefix("agents.")
            .and_then(|rest| rest.strip_suffix(".source"))
        else {
            continue;
        };
        if let Some(locked) = lock.for_agent(agent) {
            // Der Ausweg: die Wahl neu speichern, ohne den gesperrten
            // Schlüssel. Der Erststart-Bildschirm und die Einstellungen
            // schreiben ihn unter der Sperre gar nicht erst.
            return Err(Diagnostic::builder(CONFIG_024, Severity::Error)
                .why(format!(
                    "the project choice sets {key} = {value:?}, and the company policy {} fixes \
                     the source of the agent {agent} to {}; a locked key is read only and stays \
                     out of the choice",
                    locked.by,
                    Locked::SOURCE.as_str()
                ))
                .fix(FixAction::CopyCommand("humanitl settings".to_owned()))
                .build());
        }
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
pub fn lowers_the_floor(key: &str, wanted: &str, floor: &str) -> Diagnostic {
    refused(format!(
        "the project choice sets {key} = {wanted:?} below {floor:?}, which a layer beneath it \
         requires; a project choice only makes {key} stricter, never looser"
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

    /// Die Quelle jedes bekannten Agenten steht auf der Liste, und eine
    /// Sperre der Quelle hält sie aus der Wahl, auch mit dem Wert der
    /// Vorgabe (GitHub #15).
    ///
    /// Mutationsbeweis: Ließe [`super::check_source_lock`] den gesperrten
    /// Wert `depot` durch, ginge die zweite Wahl durch, und der Test wird rot.
    #[test]
    fn the_source_of_every_agent_is_a_choice_key_and_a_source_lock_keeps_it_out() {
        use crate::agents::{AgentSource, KNOWN_AGENTS, LockScope, SourceLock};

        for agent in KNOWN_AGENTS {
            assert!(super::is_choice_key(&format!("agents.{agent}.source")));
        }
        assert!(!super::is_choice_key("agents.source"));
        assert!(!super::is_choice_key("agents.claude.credential_writeback"));

        let lock = SourceLock::none()
            .lock(
                LockScope::Agent("claude".to_owned()),
                AgentSource::Depot,
                "/etc/humanitl/policy.toml",
            )
            .unwrap();
        for value in ["host", "depot"] {
            let choice = ProjectChoice::from_pairs([("agents.claude.source", value)]).unwrap();
            let refused = super::check_source_lock(&choice, &lock).unwrap_err();
            assert_eq!(refused.code.as_str(), "CONFIG_024");
            assert!(refused.fix.is_some(), "a way out: {refused:?}");
            assert!(
                refused.why.contains("/etc/humanitl/policy.toml"),
                "{}",
                refused.why
            );
        }
        let other = ProjectChoice::from_pairs([("agents.codex.source", "host")]).unwrap();
        super::check_source_lock(&other, &lock).unwrap();
        super::check_source_lock(
            &ProjectChoice::from_pairs([("agents.claude.source", "host")]).unwrap(),
            &SourceLock::none(),
        )
        .unwrap();
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
        // Auch der Wert der Vorgabe ist abgelehnt (HUM-534). Mutationsbeweis:
        // Prüfte die Schleife wieder `locked.value != value`, ginge er durch.
        let same = ProjectChoice::from_pairs([("hold.ask_mode", "terminal")]).unwrap();
        let refused = check_choice(&same, &below, &locks).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_024");
        let other_key = ProjectChoice::from_pairs([("sandbox.work_mode", "ro")]).unwrap();
        check_choice(&other_key, &below, &locks).unwrap();
    }

    /// Die Wahl schaltet die Sperre der Cloud-Inferenz ein, nie aus
    /// (GitHub #113): `allow` unter einem `block` darunter ist `CONFIG_024`.
    #[test]
    fn the_choice_switches_the_cloud_lock_on_and_never_off() {
        assert!(super::is_choice_key("llm.cloud"));
        let mut below = Config::default();
        let on = ProjectChoice::from_pairs([("llm.cloud", "block")]).unwrap();
        check_choice(&on, &below, &PolicyLocks::none()).unwrap();
        let off = ProjectChoice::from_pairs([("llm.cloud", "allow")]).unwrap();
        check_choice(&off, &below, &PolicyLocks::none()).unwrap();

        below.llm.cloud = crate::model::LlmCloud::Block;
        let refused = check_choice(&off, &below, &PolicyLocks::none()).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_024");
        assert!(refused.why.contains("llm.cloud"), "{}", refused.why);
        check_choice(&on, &below, &PolicyLocks::none()).unwrap();
    }
}
