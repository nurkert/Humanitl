//! Woher das Programm eines Agenten kommt: vom Host oder aus dem Depot
//! (ADR-0023, HUM-518).
//!
//! Der Abschnitt `agents` steht neben `agent`. `agent` wählt den Adapter, der
//! eine Sitzung begleitet (`agent.adapter`, `agent.command`); `agents` sagt je
//! bekanntem Agenten, ob sein Programm, seine Konfiguration und seine
//! Anmeldung vom Host kommen (`host`, die Vorgabe, das Verhalten vor dem
//! Depot) oder aus dem Depot unter dem Datenordner des Daemons (`depot`).
//!
//! Die wirksame Quelle beantwortet [`effective_source`] aus drei Eingaben, in
//! dieser Rangfolge:
//!
//! 1. die Sperre einer Firmenvorgabe ([`SourceLock`]); sie darf nur `depot`
//!    nennen,
//! 2. `agents.<id>.source`,
//! 3. `agents.source`, sonst die Vorgabe `host`.
//!
//! Die Firmen-Vorgabedatei gibt es noch nicht (BACKLOG Abschnitt 9, Punkt 21).
//! Bis dahin ist die Sperre ein Wert, den Tests setzen; [`SourceLock::of_policy`]
//! ist die eine Stelle, an der die Datei später gelesen wird. Jeder Schreibweg
//! der Einstellung ruft [`refuse_locked_write`]: `humanitl agents source`,
//! `humanitl config set`, die generische Einstellungszeile der App und
//! `ChangeSetting`.

use std::collections::BTreeMap;

use humanitl_core::diagnostics::codes::{AGENT_010, AGENT_015};
use humanitl_core::{Diagnostic, FixAction, Severity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Config;
use crate::paths::Paths;

/// Woher das Programm eines Agenten kommt.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentSource {
    /// Vom Host, wie vor dem Depot: Der Adapter sucht das Programm im `PATH`
    /// und in den Programmordnern des Nutzers (HUM-278).
    #[default]
    Host,
    /// Aus dem Depot des Daemons; nichts davon läuft je außerhalb einer
    /// Sandbox.
    Depot,
}

impl AgentSource {
    /// Der Name, wie ihn `config.toml`, die Leitung und die CLI schreiben.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Depot => "depot",
        }
    }

    /// Liest den Namen.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "host" => Some(Self::Host),
            "depot" => Some(Self::Depot),
            _ => None,
        }
    }
}

/// Erzeugt aus einer Zeile je Agent die Liste [`KNOWN_AGENTS`], das Feld
/// `agents.<id>` in [`AgentsConfig`] und dessen Zweig in
/// [`AgentsConfig::settings`]. Ein Agent kommt mit genau einer Zeile hier
/// und einem Eintrag `KnownAgent` im Rezeptverzeichnis des Sandbox-Crates
/// dazu (ADR-0023, „Adding an agent to the depot"); ein Feld je Agent statt
/// einer Tabelle, damit das Schema jedes Blatt nennt.
macro_rules! known_agents {
    ($($id:ident => $doc:literal),+ $(,)?) => {
        /// Die Agenten, die `agents.<id>` kennt, in der Reihenfolge der
        /// Anzeige; das Schema kennt jeden, auch ohne Adapter oder Rezept.
        pub const KNOWN_AGENTS: &[&str] = &[$(stringify!($id)),+];

        /// Der Abschnitt `agents`: die Quelle je Agent (ADR-0023).
        #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
        #[serde(default, deny_unknown_fields)]
        pub struct AgentsConfig {
            /// Woher die Agenten kommen, für jeden, der unter `agents.<id>.source` nichts sagt: `host` (Vorgabe) sucht das Programm auf dem Rechner, `depot` nimmt Programm, Konfiguration und Anmeldung aus dem Depot unter dem Datenordner des Daemons (`humanitl agents status`). Im Projekt-Profil gesperrt.
            #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
            pub source: AgentSource,
            /// Ob die Zugangsdateien im Depot verschlüsselt liegen (ChaCha20-Poly1305, HUM-523). Der Schlüssel liegt nur im Schlüsselbund des Systems, nie in einer Datei neben den Daten; ohne erreichbaren Schlüsselbund lehnt der Daemon `true` ab. Umschalten über `humanitl config set agents.encrypt_credentials` oder den Schalter im Abschnitt Agenten: Der Daemon verschlüsselt oder entschlüsselt dabei die vorhandenen Dateien. Der Daemon entschlüsselt für jede Sitzung in ihre Arbeitskopie. Im Projekt-Profil gesperrt.
            #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
            pub encrypt_credentials: bool,
            $(
                #[doc = $doc]
                #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
                pub $id: AgentSourceSettings,
            )+
        }

        impl AgentsConfig {
            /// Die Einstellungen eines bekannten Agenten.
            #[must_use]
            pub fn settings(&self, agent: &str) -> Option<&AgentSourceSettings> {
                match agent {
                    $(stringify!($id) => Some(&self.$id),)+
                    _ => None,
                }
            }
        }
    };
}

known_agents! {
    opencode => "`OpenCode`.",
    pi => "pi.",
    claude => "Claude Code.",
    codex => "Codex.",
    agy => "Die Antigravity CLI (`agy`).",
}

/// Was `agents.<id>` für einen Agenten sagt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AgentSourceSettings {
    /// Woher dieser Agent kommt, `host` oder `depot`; ungesetzt gilt `agents.source`. Zurück auf `host` behält das Depot, bis `humanitl agents remove` es entfernt. Im Projekt-Profil gesperrt.
    #[schemars(
        schema_with = "optional_source_schema",
        extend("x-tier" = "advanced", "x-project-scope" = "denied")
    )]
    pub source: Option<AgentSource>,
    /// Ob eine Sitzung die geprüfte Zugangsdatei des Agenten ins Depot zurückschreiben darf, damit eine erneuerte Anmeldung die Sitzung überdauert. Der Wächter lässt nur diese eine Datei durch, im Format des Adapters, nie älter und nie von einem anderen Konto (HUM-520); mit `false` bleibt nichts einer Sitzung im Depot. Im Projekt-Profil gesperrt.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub credential_writeback: bool,
}

/// Das Schema von `agents.<id>.source`: eine Zeichenkette aus der Aufzählung
/// oder nichts, als ein Knoten ohne `anyOf`, damit der Durchlauf des Schemas
/// ihn als Blatt sieht (`tests/config_readers.rs`).
fn optional_source_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": ["string", "null"],
        "enum": ["host", "depot", null],
    })
}

impl Default for AgentSourceSettings {
    fn default() -> Self {
        Self {
            source: None,
            credential_writeback: true,
        }
    }
}

/// Wofür eine Sperre gilt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LockScope {
    /// Für jeden Agenten.
    All,
    /// Für einen Agenten, mit seiner Kennung.
    Agent(String),
}

/// Eine Sperre der Quelle durch eine Vorgabe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locked {
    /// Wer sperrt, so wie Status, App und CLI es nennen: die Vorgabedatei.
    pub by: String,
}

/// Die Sperren einer Firmenvorgabe. Jede nennt `depot`; eine auf `host` lässt
/// sich nicht bauen ([`SourceLock::lock`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceLock {
    all: Option<Locked>,
    agents: BTreeMap<String, Locked>,
}

impl SourceLock {
    /// Keine Sperre.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Die Sperren aus der Firmen-Vorgabedatei.
    ///
    /// Die Datei gibt es noch nicht (BACKLOG Abschnitt 9, Punkt 21); bis dahin
    /// ist das immer [`SourceLock::none`]. Daemon und CLI lesen die Sperre nur
    /// hier, damit die Datei später an genau einer Stelle dazukommt.
    #[must_use]
    pub fn of_policy(paths: &Paths) -> Self {
        let _ = paths;
        Self::none()
    }

    /// Fügt eine Sperre hinzu.
    ///
    /// # Errors
    ///
    /// `AGENT_010`, wenn die Sperre `host` nennt: Eine Vorgabe darf den
    /// Agenten nur ins Depot zwingen, nie auf den Host (ADR-0023 Punkt 1).
    pub fn lock(
        mut self,
        scope: LockScope,
        source: AgentSource,
        by: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        let by = by.into();
        if source != AgentSource::Depot {
            return Err(Diagnostic::builder(AGENT_010, Severity::Error)
                .why(format!(
                    "the company policy {by} fixes an agent source to {}; a policy may only fix \
                     it to the depot, so this lock is refused",
                    source.as_str()
                ))
                .build());
        }
        let locked = Locked { by };
        match scope {
            LockScope::All => self.all = Some(locked),
            LockScope::Agent(agent) => {
                self.agents.insert(agent, locked);
            }
        }
        Ok(self)
    }

    /// Die Sperre, die für diesen Agenten gilt: die eigene vor der für alle.
    #[must_use]
    pub fn for_agent(&self, agent: &str) -> Option<&Locked> {
        self.agents.get(agent).or(self.all.as_ref())
    }

    /// Ob irgendeine Sperre besteht.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.all.is_none() && self.agents.is_empty()
    }
}

/// Welche Stelle die wirksame Quelle bestimmt hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    /// Nichts gesetzt: die Vorgabe `host`.
    Default,
    /// `agents.source`.
    AllAgents,
    /// `agents.<id>.source`.
    Agent,
    /// Die Sperre einer Vorgabe.
    Lock,
}

impl SourceOrigin {
    /// Der Name, wie ihn Status und Leitung führen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AllAgents => "agents.source",
            Self::Agent => "agents.<id>.source",
            Self::Lock => "policy",
        }
    }
}

/// Die wirksame Quelle eines Agenten, mit Herkunft und Sperre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSource {
    /// Host oder Depot.
    pub source: AgentSource,
    /// Welche Stelle das entschieden hat.
    pub origin: SourceOrigin,
    /// Die Sperre, wenn eine gilt; dann ist `source` immer `depot`.
    pub locked: Option<Locked>,
}

impl EffectiveSource {
    /// Ob das Programm dieses Agenten aus dem Depot kommen muss.
    #[must_use]
    pub fn is_depot(&self) -> bool {
        self.source == AgentSource::Depot
    }
}

/// Die wirksame Quelle eines Agenten (ADR-0023 Punkt 1): die Sperre vor
/// `agents.<id>.source` vor `agents.source`, sonst `host`.
///
/// Eine unbekannte Kennung hat keine eigene Einstellung; für sie gelten
/// `agents.source` und eine Sperre für alle.
#[must_use]
pub fn effective_source(agent: &str, config: &Config, lock: &SourceLock) -> EffectiveSource {
    if let Some(locked) = lock.for_agent(agent) {
        return EffectiveSource {
            source: AgentSource::Depot,
            origin: SourceOrigin::Lock,
            locked: Some(locked.clone()),
        };
    }
    if let Some(source) = config
        .agents
        .settings(agent)
        .and_then(|settings| settings.source)
    {
        return EffectiveSource {
            source,
            origin: SourceOrigin::Agent,
            locked: None,
        };
    }
    let origin = if config.agents.source == AgentSource::default() {
        SourceOrigin::Default
    } else {
        SourceOrigin::AllAgents
    };
    EffectiveSource {
        source: config.agents.source,
        origin,
        locked: None,
    }
}

/// Der Befund für einen Start oder Schreibweg, den eine Sperre ablehnt.
#[must_use]
pub fn locked_refusal(agent: &str, locked: &Locked, what: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_010, Severity::Blocking)
        .why(format!(
            "the company policy {} fixes the source of the agent {agent} to the depot; {what} is \
             refused",
            locked.by
        ))
        .build()
}

/// Lehnt ab, was eine Sperre umginge: `host` für `agents.source` oder für
/// `agents.<id>.source` eines gesperrten Agenten (ADR-0023 Punkt 1).
///
/// `path` ist der kanonische Schlüssel, `value` der neue Wert als Text, `None`
/// für das Entfernen. Jeder Schreibweg ruft genau diese Funktion: `humanitl
/// agents source`, `humanitl config set`, die Einstellungszeile der App und
/// `ChangeSetting` über die RPC.
///
/// # Errors
///
/// `AGENT_010`, dessen `why` die Vorgabe nennt; ohne `fix`.
pub fn refuse_locked_write(
    path: &str,
    value: Option<&str>,
    lock: &SourceLock,
) -> Result<(), Diagnostic> {
    if lock.is_empty() || value.map(str::trim) != Some(AgentSource::Host.as_str()) {
        return Ok(());
    }
    let what = format!("setting {path} to host");
    if path == "agents.source" {
        let first = lock
            .all
            .iter()
            .map(|locked| ("every agent", locked))
            .chain(
                lock.agents
                    .iter()
                    .map(|(agent, locked)| (agent.as_str(), locked)),
            )
            .next();
        if let Some((agent, locked)) = first {
            return Err(locked_refusal(agent, locked, &what));
        }
        return Ok(());
    }
    let Some(agent) = path
        .strip_prefix("agents.")
        .and_then(|rest| rest.strip_suffix(".source"))
    else {
        return Ok(());
    };
    match lock.for_agent(agent) {
        Some(locked) => Err(locked_refusal(agent, locked, &what)),
        None => Ok(()),
    }
}

/// Die Hinweise beim Laden: `depot` für einen Agenten, dessen Adapter noch
/// kein Rezept für das Depot hat. Gültig bleibt die Einstellung; der Start
/// antwortet mit `AGENT_015`. Ob es ein Rezept gibt, sagt `has_recipe`, die
/// Schicht, die das Rezeptverzeichnis kennt (`humanitl_sandbox`); diese Crate
/// führt keine Liste davon.
#[must_use]
pub fn depot_notices(config: &Config, has_recipe: impl Fn(&str) -> bool) -> Vec<Diagnostic> {
    KNOWN_AGENTS
        .iter()
        .filter(|agent| !has_recipe(agent))
        .filter(|agent| {
            config
                .agents
                .settings(agent)
                .and_then(|settings| settings.source)
                == Some(AgentSource::Depot)
        })
        .map(|agent| {
            Diagnostic::builder(AGENT_015, Severity::Warning)
                .why(format!(
                    "agents.{agent}.source is depot, and the adapter of {agent} brings no recipe \
                     for the depot yet; a start with this agent stops with AGENT_015"
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl agents source {agent} host"
                )))
                .build()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn config(all: Option<AgentSource>, claude: Option<AgentSource>) -> Config {
        let mut config = Config::default();
        if let Some(all) = all {
            config.agents.source = all;
        }
        config.agents.claude.source = claude;
        config
    }

    fn locked_all() -> SourceLock {
        SourceLock::none()
            .lock(
                LockScope::All,
                AgentSource::Depot,
                "/etc/humanitl/policy.toml",
            )
            .unwrap()
    }

    #[test]
    fn the_default_is_host() {
        let found = effective_source("claude", &Config::default(), &SourceLock::none());
        assert_eq!(found.source, AgentSource::Host);
        assert_eq!(found.origin, SourceOrigin::Default);
        assert_eq!(found.locked, None);
    }

    #[test]
    fn agents_source_applies_to_every_agent_and_the_own_setting_wins() {
        let all_depot = config(Some(AgentSource::Depot), None);
        for agent in KNOWN_AGENTS {
            let found = effective_source(agent, &all_depot, &SourceLock::none());
            assert_eq!(found.source, AgentSource::Depot, "{agent}");
            assert_eq!(found.origin, SourceOrigin::AllAgents);
        }
        let own = config(Some(AgentSource::Depot), Some(AgentSource::Host));
        let found = effective_source("claude", &own, &SourceLock::none());
        assert_eq!(found.source, AgentSource::Host);
        assert_eq!(found.origin, SourceOrigin::Agent);
        assert_eq!(
            effective_source("codex", &own, &SourceLock::none()).source,
            AgentSource::Depot
        );
    }

    /// Mutationsbeweis: Wer die Sperre in `effective_source` übergeht, färbt
    /// diesen Test rot.
    #[test]
    fn the_lock_beats_both_settings() {
        let host_everywhere = config(Some(AgentSource::Host), Some(AgentSource::Host));
        let found = effective_source("claude", &host_everywhere, &locked_all());
        assert_eq!(found.source, AgentSource::Depot);
        assert_eq!(found.origin, SourceOrigin::Lock);
        assert_eq!(
            found.locked.map(|locked| locked.by).as_deref(),
            Some("/etc/humanitl/policy.toml")
        );
        let only_codex = SourceLock::none()
            .lock(
                LockScope::Agent("codex".to_owned()),
                AgentSource::Depot,
                "policy",
            )
            .unwrap();
        assert!(effective_source("codex", &host_everywhere, &only_codex).is_depot());
        assert!(!effective_source("claude", &host_everywhere, &only_codex).is_depot());
    }

    #[test]
    fn a_lock_on_host_is_refused() {
        let refused = SourceLock::none()
            .lock(LockScope::All, AgentSource::Host, "policy")
            .unwrap_err();
        assert_eq!(refused.code, AGENT_010);
        assert!(
            refused.why.contains("only fix it to the depot"),
            "{}",
            refused.why
        );
    }

    #[test]
    fn every_write_of_host_is_refused_under_the_lock_and_names_the_policy() {
        let lock = locked_all();
        for path in [
            "agents.source",
            "agents.claude.source",
            "agents.opencode.source",
        ] {
            let refused = refuse_locked_write(path, Some("host"), &lock).unwrap_err();
            assert_eq!(refused.code, AGENT_010, "{path}");
            assert!(refused.why.contains("/etc/humanitl/policy.toml"));
            assert!(refused.fix.is_none(), "no way around the policy");
        }
        assert_eq!(
            refuse_locked_write("agents.claude.source", Some(" host "), &lock)
                .unwrap_err()
                .code,
            AGENT_010,
            "a value with spaces is the same host"
        );
        assert!(refuse_locked_write("agents.claude.source", Some("depot"), &lock).is_ok());
        assert!(
            refuse_locked_write("agents.claude.credential_writeback", Some("host"), &lock).is_ok()
        );
        assert!(refuse_locked_write("agents.source", Some("host"), &SourceLock::none()).is_ok());
    }

    #[test]
    fn a_lock_for_one_agent_refuses_its_key_and_the_key_for_all() {
        let lock = SourceLock::none()
            .lock(
                LockScope::Agent("claude".to_owned()),
                AgentSource::Depot,
                "policy",
            )
            .unwrap();
        assert!(refuse_locked_write("agents.claude.source", Some("host"), &lock).is_err());
        assert!(refuse_locked_write("agents.codex.source", Some("host"), &lock).is_ok());
        assert!(refuse_locked_write("agents.source", Some("host"), &lock).is_err());
    }

    #[test]
    fn depot_without_a_recipe_is_a_notice_at_load() {
        let mut config = Config::default();
        config.agents.opencode.source = Some(AgentSource::Depot);
        config.agents.claude.source = Some(AgentSource::Depot);
        let notices = depot_notices(&config, |agent| agent == "claude");
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].code, AGENT_015);
        assert_eq!(notices[0].severity, Severity::Warning);
        assert!(notices[0].why.contains("agents.opencode.source"));
    }
}
