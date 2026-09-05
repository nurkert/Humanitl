//! Die Konfiguration einer Sitzung: was ein Client bestimmen darf, und wie
//! daraus Regeln werden.
//!
//! Bis HUM-067 löste `humanitld` seine Konfiguration genau einmal beim Start
//! auf und fror Regelspeicher, Haltefrist, Durchreichregel und den
//! Sandbox-Dienst darum ein. `humanitl run --profile llm-only --ask none
//! --llm http://…` hatte damit keinen Weg in den Daemon; die Flags waren
//! Dekoration. Dieses Modul ist der Weg.
//!
//! # Der Socket ist die Vertrauensgrenze
//!
//! Was hier hereinkommt, bestimmt, welche Regeln gelten, wie lange gefragt
//! wird und wohin die erklärte Durchreiche zeigt. Das ist dieselbe Grenze,
//! an der schon der Profilname und das Projektverzeichnis geprüft werden
//! (`backlog/CONVENTIONS.md` 4.17, HUM-040), und sie wird hier genauso
//! gezogen: **Erlaubnisliste, nicht Sperrliste.**
//!
//! Ein `CliOverride`, der jeden Schlüssel des Schemas setzen dürfte, wäre der
//! Schlüssel zur ganzen Sandbox. `sandbox.profile` bestimmt die Einhängefläche,
//! `agent.command` den Prozess darin, `sandbox.env` seine Umgebung,
//! `resolver.overrides` und `experimental.*`, wohin der Verkehr wirklich geht,
//! `findings.enabled` ob vor einer Entscheidung überhaupt noch nach
//! Geheimnissen gesucht wird, und `recorder.retention_days`, wie lange die
//! Aufzeichnung die Entscheidung belegt. Keiner dieser Werte gehört einem
//! Client; sie stehen in `config.toml` oder in einem globalen Profil, wo ein
//! Mensch sie geschrieben hat, und der Daemon liest sie dort.
//!
//! Übrig bleiben die zwei Schlüssel in [`SESSION_OVERRIDE_KEYS`], und beide
//! sind Flags aus `backlog/CONVENTIONS.md` 3.8:
//!
//! - `hold.timeout_secs` sagt, wie lange eine gehaltene Anfrage wartet. Er
//!   vergrößert nichts: Eine kürzere Frist blockt früher, eine längere lässt
//!   den Menschen länger warten, und null ist nicht einstellbar.
//! - `llm.endpoint` (`--llm`) benennt das Sprachmodell dieser Sitzung.
//!   **Dieser Schlüssel vergrößert sehr wohl etwas**, und das gehört
//!   ausgesprochen: `AgentAdapter::llm_passthrough` baut daraus eine Regel in
//!   Rang 1 mit `allow_private`, die nicht gehalten wird und die eigenen
//!   Block-Regeln des Nutzers überholt — für die Inferenzpfade eines
//!   beliebigen Hosts und für jeden Loopback-Port des Wirts. Er steht
//!   trotzdem auf der Liste, weil `backlog/CONVENTIONS.md` 3.8 ihn als Flag
//!   von `humanitl run` führt und weil seine Wirkung an drei Stellen sichtbar
//!   ist: als eigene Regel in der Liste, unter `http://humanitl.internal/`
//!   und in jeder Aufzeichnung. Dazu meldet [`SandboxService`] beim Start
//!   `LLM_006`, wenn der Endpunkt nach seinem Namen nicht im eigenen Netz
//!   liegt ([`humanitl_proxy::not_private_by_name`]).
//!
//! [`SandboxService`]: crate::SandboxService
//!
//! Frage-Modus, Projektverzeichnis, Arbeitsmodus, Sitzungsprofil und der
//! Befehl reisen **nicht** in dieser Liste. Sie haben je ein eigenes Feld in
//! [`crate::v1::sandbox_request::Start`] und dort ihre eigene Prüfung; zwei Wege zu
//! einem Feld wären zwei Regeln, welcher gewinnt.
//!
//! # Die Regeln einer Sitzung
//!
//! [`bundled_rules`] setzt die mitgelieferte Gruppe zusammen: die Durchreiche
//! zum Sprachmodell, dann die Regeln der beteiligten Profile, dann
//! `rules/default.yaml`. Dieselbe Funktion baut die Gruppe beim Start des
//! Daemons und beim Start jeder Sitzung — eine zweite, von Hand gepflegte
//! Reihenfolge daneben wäre genau der Fehler, an dem HUM-104 gearbeitet hat.

use std::path::{Path, PathBuf};

use humanitl_config::{
    AskMode, Config, Env, Paths, Profile, ProfileSelection, Resolved, WorkMode, schema,
};
use humanitl_core::diagnostics::codes;
use humanitl_core::rule::Rule;
use humanitl_core::{Diagnostic, FixAction, SessionId, Severity};
use humanitl_rules::parse_rules_for_session;
use humanitl_sandbox::AdapterRegistry;

/// Die Konfigurationspfade, die ein Client für seine Sitzung setzen darf.
///
/// Die Begründung steht in der Modulbeschreibung. Wer diese Liste erweitert,
/// erweitert, was ein Prozess auf dem Socket über die Sandbox bestimmen darf,
/// und schreibt dazu, warum der neue Schlüssel nichts vergrößert.
pub const SESSION_OVERRIDE_KEYS: &[&str] = &["llm.endpoint", "hold.timeout_secs"];

/// Der mitgelieferte Regelsatz.
///
/// Er liegt als Datei im Baum, damit `docs/reference/rules.md` und die Tests
/// dieselbe Quelle lesen, und wird ins Binary gebunden, damit ein installierter
/// Daemon ihn ohne das Repository hat (HUM-038).
pub const BUNDLED_RULES: &str = include_str!("../../../../rules/default.yaml");

/// Was ein Client für seine Sitzung wünscht.
///
/// Alles ist optional: Ein leeres Feld heißt „nimm, was ohne Wunsch gilt", und
/// nicht „setze den Vorgabewert". Der Unterschied ist der zwischen einer
/// `Status`-Anfrage, die nichts ändern darf, und einem Start, der etwas will.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRequest {
    /// Das Profil der Sitzung (`profiles/*.toml`), zum Beispiel `llm-only`.
    pub profile: Option<String>,
    /// Das Projektverzeichnis, schon geprüft.
    pub work_dir: Option<PathBuf>,
    /// Ob `/work` schreibbar ist.
    pub work_mode: Option<WorkMode>,
    /// Wo gefragt wird.
    pub ask_mode: Option<AskMode>,
    /// Einzelne Konfigurationswerte, `(Pfad, Text)`.
    pub overrides: Vec<(String, String)>,
}

impl SessionRequest {
    /// Die Wünsche als Paare auf der Ebene der Kommandozeile.
    ///
    /// Die typisierten Felder werden dabei zu genau den Schema-Pfaden, unter
    /// denen sie ohnehin stehen; die Präzedenz entsteht in
    /// `humanitl_config::load` und nicht hier.
    fn cli_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = self.overrides.clone();
        if let Some(dir) = self.work_dir.as_ref() {
            pairs.push(("sandbox.work_dir".to_owned(), dir.display().to_string()));
        }
        if let Some(mode) = self.work_mode {
            pairs.push((
                "sandbox.work_mode".to_owned(),
                work_mode_name(mode).to_owned(),
            ));
        }
        if let Some(mode) = self.ask_mode {
            pairs.push(("hold.ask_mode".to_owned(), ask_mode_name(mode).to_owned()));
        }
        pairs
    }
}

/// Löst die Konfiguration einer Sitzung auf, so oft wie eine startet.
///
/// Der Daemon hält einen davon für seine ganze Laufzeit; er trägt die
/// Umgebung, aus der die sieben Ebenen kommen (`backlog/CONVENTIONS.md` 4.23),
/// und den Stand, der ohne jeden Wunsch gilt.
#[derive(Debug, Clone)]
pub struct SessionResolver {
    paths: Paths,
    /// Was ohne Wunsch gilt: die Auflösung des Daemon-Starts.
    base: Resolved,
}

impl SessionResolver {
    /// Ein Resolver über diesen Pfaden und diesem Grundstand.
    #[must_use]
    pub const fn new(paths: Paths, base: Resolved) -> Self {
        Self { paths, base }
    }

    /// Ein Resolver, dessen Grundstand genau diese Konfiguration ist.
    ///
    /// Ohne Profil-Ebenen und ohne Herkunft. Für den Fake-Modus und für
    /// Tests, die eine Konfiguration von Hand bauen; ein Start löst trotzdem
    /// gegen die Umgebung der Pfade auf, denn genau das ist der Weg, den ein
    /// Test prüfen soll.
    #[must_use]
    pub fn for_config(paths: Paths, config: Config) -> Self {
        Self::new(
            paths,
            Resolved {
                config,
                origins: std::collections::BTreeMap::new(),
                profiles: Vec::new(),
                diagnostics: Vec::new(),
            },
        )
    }

    /// Die Pfade, gegen die aufgelöst wird.
    #[must_use]
    pub const fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Die Umgebung, aus der die Ebenen kommen.
    #[must_use]
    pub fn env(&self) -> &Env {
        self.paths.env()
    }

    /// Der Stand ohne jeden Wunsch.
    #[must_use]
    pub const fn base(&self) -> &Resolved {
        &self.base
    }

    /// Löst die Konfiguration für diesen Wunsch auf.
    ///
    /// Streng wie `humanitl run`: Ein Profil, das es nicht gibt oder sich
    /// nicht lesen lässt, ist `CONFIG_001` und kein stiller Start mit dem
    /// Vorgabeprofil.
    ///
    /// # Errors
    ///
    /// `CONFIG_003` für einen Pfad außerhalb von [`SESSION_OVERRIDE_KEYS`]
    /// und für einen Wert außerhalb seines Bereichs, `CONFIG_001` für ein
    /// Profil, das es nicht gibt, `CONFIG_002` für einen unbekannten
    /// Schlüssel.
    pub fn resolve(&self, request: &SessionRequest) -> Result<Resolved, Diagnostic> {
        for (path, _) in &request.overrides {
            check_override_key(path)?;
        }
        let selection = ProfileSelection {
            name: request.profile.clone(),
        };
        // `work_dir` ist der Rückfall für die Suche nach dem Projekt-Profil;
        // gilt ein anderes Verzeichnis, steht es zugleich als Paar auf der
        // Kommandozeilen-Ebene und gewinnt dort.
        let cwd = request
            .work_dir
            .clone()
            .or_else(|| self.base.config.sandbox.work_dir.clone());
        humanitl_config::resolve(&selection, cwd.as_deref(), self.env(), &request.cli_pairs())
    }
}

/// Prüft einen Pfad, den ein Client für seine Sitzung setzen will.
///
/// # Errors
///
/// `CONFIG_003`, wenn der Pfad nicht in [`SESSION_OVERRIDE_KEYS`] steht — mit
/// dem Unterschied im Text, ob es den Schlüssel überhaupt gibt oder ob er nur
/// hier nicht gesetzt werden darf. Ein „unbekannter Schlüssel" für einen, der
/// im Schema steht, wäre die irreführendste Antwort, die möglich ist.
pub fn check_override_key(path: &str) -> Result<(), Diagnostic> {
    if SESSION_OVERRIDE_KEYS.contains(&path) {
        return Ok(());
    }
    let known = schema::known_paths().contains(path);
    let why = if known {
        format!(
            "{path} is a setting, but a client does not set it for a session: it would decide \
             what the sandbox mounts, what runs inside it or where the traffic really goes. \
             Write it in config.toml or in a global profile, where a person wrote it and the \
             daemon reads it. A session may set: {}",
            SESSION_OVERRIDE_KEYS.join(", ")
        )
    } else {
        format!(
            "there is no setting {path}. A session may set: {}",
            SESSION_OVERRIDE_KEYS.join(", ")
        )
    };
    Err(Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
        .why(why)
        .fix(FixAction::CopyCommand("humanitl config schema".to_owned()))
        .build())
}

/// Die mitgelieferte Gruppe des Regelspeichers für diese Sitzung.
///
/// Drei Quellen, in dieser Reihenfolge:
///
/// 1. die erklärte Durchreiche zum Sprachmodell, falls es einen Endpunkt und
///    einen Adapter gibt,
/// 2. die Regeln der beteiligten Profile — `[rules].inline` und
///    `[rules].files` —, Rang 4 nach `backlog/CONVENTIONS.md` 4.5,
/// 3. `rules/default.yaml`.
///
/// Die Reihenfolge innerhalb der Gruppe ist die der Anzeige; ihren Vorrang
/// trägt die Durchreiche an sich selbst (`passthrough_llm`, HUM-104) und nicht
/// an ihrem Platz.
///
/// Ein Regelsatz, den die Engine ablehnt, wird zum leeren Regelsatz: Eine
/// kaputte Datei darf nie zu einer Freigabe führen, die niemand gegeben hat.
/// Die Befunde kommen mit zurück, damit der Aufrufer sie meldet.
#[must_use]
pub fn bundled_rules(
    config: &Config,
    profiles: &[Profile],
    session: SessionId,
) -> (BundledRules, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let passthrough = llm_passthrough_rule(config, &mut diagnostics);
    let mut rest = Vec::new();

    for profile in profiles {
        if let Some(document) = profile.rules_document() {
            let (parsed, found) = read_rules(&document, session, &profile.name);
            rest.extend(parsed);
            diagnostics.extend(found);
        }
        for path in profile.rule_files() {
            let (parsed, found) = read_rule_file(&path, session);
            rest.extend(parsed);
            diagnostics.extend(found);
        }
    }
    let (parsed, found) = read_rules(BUNDLED_RULES, session, "rules/default.yaml");
    rest.extend(parsed);
    diagnostics.extend(found);

    let (group, refused) = BundledRules::new(passthrough, rest);
    diagnostics.extend(refused);
    (group, diagnostics)
}

/// Die mitgelieferte Gruppe einer Sitzung, mit der Durchreiche getrennt.
///
/// Die Trennung steht im Typ und nicht in einem Kommentar, weil an ihr eine
/// Rangordnung hängt. [`RuleSet::evaluate`](humanitl_rules::RuleSet::evaluate)
/// prüft eine Regel in Rang 1 — vor jeder Sitzungs-, Nutzer- und Profilregel
/// —, wenn **beide** Vermerke stehen: `bundled` und `passthrough_llm`. Den
/// ersten setzt der Regelspeicher auf jede Regel dieser Gruppe. Den zweiten
/// darf deshalb nur tragen, was `llm_passthrough_rule` baut, also was aus
/// `llm.endpoint` und dem Agent-Adapter.
///
/// Ohne diese Trennung genügte ein globales Profil mit
///
/// ```toml
/// [rules]
/// inline = [{ action = "allow", passthrough_llm = true,
///             match = { host = "exfil.example" } }]
/// ```
///
/// um für einen beliebigen Host einen ungehaltenen Weg nach draußen zu
/// öffnen, der die eigenen Block-Regeln des Nutzers überholt — unsichtbar,
/// weil eine Durchreiche niemanden fragt. `humanitl_rules::parse_rules`
/// verwirft `bundled` aus einer Datei, `passthrough_llm` aber nicht; die
/// zweite Hälfte des Rangs fällt deshalb hier
/// (`backlog/CONVENTIONS.md` 4.5, HUM-104).
#[derive(Debug, Clone, Default)]
pub struct BundledRules {
    /// Die eine erklärte Durchreiche zum Sprachmodell, falls es einen
    /// Endpunkt und einen Adapter gibt.
    passthrough: Option<Rule>,
    /// Alles andere: die Regeln der beteiligten Profile und
    /// `rules/default.yaml`. Keine davon trägt `passthrough_llm`.
    rest: Vec<Rule>,
}

impl BundledRules {
    /// Baut die Gruppe und nimmt jeder Regel außer der Durchreiche den
    /// Vermerk `passthrough_llm`.
    ///
    /// Zurück kommen die Befunde zu den Regeln, die ihn gesetzt hatten: Ein
    /// stiller Entzug ließe den Verfasser glauben, seine Regel gelte in Rang 1.
    #[must_use]
    pub fn new(passthrough: Option<Rule>, rest: Vec<Rule>) -> (Self, Vec<Diagnostic>) {
        let mut refused = Vec::new();
        let rest = rest
            .into_iter()
            .map(|rule| {
                if !rule.passthrough_llm {
                    return rule;
                }
                refused.push(
                    Diagnostic::builder(codes::RULES_010, Severity::Warning)
                        .why(format!(
                            "the rule {} declares passthrough_llm, and a file does not declare \
                             the passthrough to the language model: it is built from \
                             llm.endpoint when the session starts. The mark is dropped, and the \
                             rule keeps its place in the order.",
                            rule.id
                        ))
                        .build(),
                );
                rule.passthrough_llm(false)
            })
            .collect();
        (Self { passthrough, rest }, refused)
    }

    /// Die ganze Gruppe, die Durchreiche zuerst.
    ///
    /// Die Reihenfolge ist die der Anzeige; ihren Vorrang trägt die
    /// Durchreiche an sich selbst und nicht an ihrem Platz.
    #[must_use]
    pub fn all(&self) -> Vec<Rule> {
        let mut all = Vec::with_capacity(self.rest.len() + 1);
        all.extend(self.passthrough.clone());
        all.extend(self.rest.iter().cloned());
        all
    }

    /// Wie viele Regeln die Gruppe trägt.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rest.len() + usize::from(self.passthrough.is_some())
    }

    /// Ob die Gruppe leer ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Die erklärte Durchreiche, falls es eine gibt.
    #[must_use]
    pub const fn passthrough(&self) -> Option<&Rule> {
        self.passthrough.as_ref()
    }
}

/// Die Durchreichregel zum Sprachmodell, falls es einen Endpunkt gibt.
///
/// Ohne sie hielte der Proxy jede Inferenz an, und `DecisionSource::Passthrough`
/// wie `LLM_005` blieben toter Code (HUM-039). Sie entsteht im Agent-Adapter,
/// weil nur er weiß, welche Pfade sein Agent für Inferenz braucht.
///
/// `None` heißt in jedem Fall: es wird gefragt. Ohne `llm.endpoint` gibt es
/// nichts durchzulassen, und ein unbekannter Adapter bekommt keine erfundene
/// Regel — er bekommt einen Befund.
fn llm_passthrough_rule(config: &Config, diagnostics: &mut Vec<Diagnostic>) -> Option<Rule> {
    config.llm.endpoint.as_ref()?;
    let registry = AdapterRegistry::builtin();
    let Some(adapter) = registry.get(&config.agent.adapter) else {
        diagnostics.push(
            Diagnostic::builder(codes::CONFIG_003, Severity::Warning)
                .why(format!(
                    "agent.adapter is {:?}, and no adapter of that name exists; the LLM \
                     endpoint gets no passthrough rule and every inference will be held. \
                     Known adapters: {}",
                    config.agent.adapter,
                    registry.ids().join(", ")
                ))
                .fix(FixAction::ChangeSetting {
                    key: "agent.adapter".to_owned(),
                    value: registry
                        .ids()
                        .first()
                        .map_or_else(String::new, |id| (*id).to_owned()),
                })
                .build(),
        );
        return None;
    };
    adapter.llm_passthrough(&config.llm)
}

/// Liest ein Regeldokument und macht aus einem Fehlschlag einen leeren Satz.
fn read_rules(document: &str, session: SessionId, source: &str) -> (Vec<Rule>, Vec<Diagnostic>) {
    match parse_rules_for_session(document, session) {
        Ok((set, diagnostics)) => (
            set.iter().cloned().collect(),
            with_source(diagnostics, source),
        ),
        Err(diagnostics) => (Vec::new(), with_source(diagnostics, source)),
    }
}

/// Liest eine Regeldatei eines Profils.
fn read_rule_file(path: &Path, session: SessionId) -> (Vec<Rule>, Vec<Diagnostic>) {
    let source = path.display().to_string();
    match std::fs::read_to_string(path) {
        Ok(text) => read_rules(&text, session, &source),
        Err(error) => (
            Vec::new(),
            vec![
                Diagnostic::builder(codes::RULES_001, Severity::Warning)
                    .why(format!(
                        "the profile names the rule file {source}, and it cannot be read: \
                         {error}. The session starts without those rules; without a rule every \
                         request is held."
                    ))
                    .build(),
            ],
        ),
    }
}

/// Schreibt die Quelle in den Grund, damit ein Befund sagt, welche Datei ihn
/// ausgelöst hat.
fn with_source(diagnostics: Vec<Diagnostic>, source: &str) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let why = format!("{source}: {}", diagnostic.why);
            let mut builder = Diagnostic::builder(diagnostic.code, diagnostic.severity).why(why);
            if let Some(fix) = diagnostic.fix {
                builder = builder.fix(fix);
            }
            builder.build()
        })
        .collect()
}

/// Der Name eines Arbeitsmodus, wie ihn Schema und Kommandozeile schreiben.
#[must_use]
pub const fn work_mode_name(mode: WorkMode) -> &'static str {
    match mode {
        WorkMode::Ro => "ro",
        WorkMode::Rw => "rw",
    }
}

/// Liest einen Arbeitsmodus aus dem Text der Leitung; leer heißt `None`.
#[must_use]
pub fn parse_work_mode(text: &str) -> Option<WorkMode> {
    match text.trim().to_ascii_lowercase().as_str() {
        "ro" => Some(WorkMode::Ro),
        "rw" => Some(WorkMode::Rw),
        _ => None,
    }
}

/// Der Name eines Frage-Modus, wie ihn Schema und Kommandozeile schreiben.
#[must_use]
pub const fn ask_mode_name(mode: AskMode) -> &'static str {
    match mode {
        AskMode::Ui => "ui",
        AskMode::Terminal => "terminal",
        AskMode::None => "none",
    }
}

/// Liest einen Frage-Modus aus dem Text der Leitung.
///
/// # Errors
///
/// `CONFIG_003` für ein Wort, das keiner der drei Modi ist. Leer ist kein
/// Fehler und ergibt `None`: „nimm den Wert des Profils".
pub fn parse_ask_mode(text: &str) -> Result<Option<AskMode>, Diagnostic> {
    match text.trim().to_ascii_lowercase().as_str() {
        "" => Ok(None),
        "ui" => Ok(Some(AskMode::Ui)),
        "terminal" => Ok(Some(AskMode::Terminal)),
        "none" => Ok(Some(AskMode::None)),
        other => Err(Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
            .why(format!(
                "{other:?} is not an ask mode; it is one of ui, terminal, none"
            ))
            .fix(FixAction::ChangeSetting {
                key: "hold.ask_mode".to_owned(),
                value: "ui".to_owned(),
            })
            .build()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{AskMode, Config, Env, Paths, Resolved, WorkMode};
    use humanitl_core::SessionId;

    use super::{
        SESSION_OVERRIDE_KEYS, SessionRequest, SessionResolver, bundled_rules, check_override_key,
        parse_ask_mode,
    };

    fn session_resolver(home: &std::path::Path) -> SessionResolver {
        let env = Env::from_pairs([
            ("HOME", home.display().to_string()),
            ("XDG_CONFIG_HOME", home.join("cfg").display().to_string()),
        ]);
        let paths = Paths::new(env.clone());
        let base =
            humanitl_config::resolve(&humanitl_config::ProfileSelection::any(), None, &env, &[])
                .expect("the default profile resolves");
        SessionResolver::new(paths, base)
    }

    #[test]
    fn the_two_allowed_keys_pass_and_everything_else_does_not() {
        // Die Zahl steht im Test, weil die Aufzählung darunter sonst nichts
        // hielte: Ein dritter Schlüssel in der Liste bliebe grün, solange er
        // nur nicht zufällig einer der neun genannten ist. Wer die Liste
        // erweitert, ändert diese Zahl und schreibt in
        // `backlog/CONVENTIONS.md` 4.25, warum der neue Schlüssel nichts
        // vergrößert.
        assert_eq!(
            SESSION_OVERRIDE_KEYS.len(),
            2,
            "a client sets llm.endpoint and hold.timeout_secs, and nothing else: {SESSION_OVERRIDE_KEYS:?}"
        );
        for key in SESSION_OVERRIDE_KEYS {
            assert!(check_override_key(key).is_ok(), "{key} should be allowed");
        }
        // Genau die Schlüssel, mit denen ein Client die Sandbox aufmachen
        // würde. Jeder einzelne ist ein eigener Weg hinaus.
        for key in [
            "sandbox.profile",
            "sandbox.work_dir",
            "sandbox.env",
            "agent.adapter",
            "agent.command",
            "hold.ask_mode",
            "findings.enabled",
            "recorder.retention_days",
            "resolver.overrides",
        ] {
            let diagnostic = check_override_key(key).expect_err("{key} must be refused");
            assert_eq!(diagnostic.code.as_str(), "CONFIG_003", "{key}");
        }
    }

    #[test]
    fn a_key_that_is_not_a_setting_says_so() {
        let diagnostic = check_override_key("nope.nothing").expect_err("no such setting");
        assert!(
            diagnostic.why.contains("there is no setting"),
            "{}",
            diagnostic.why
        );
    }

    #[test]
    fn a_refused_override_never_reaches_the_resolution() {
        let home = tempfile::tempdir().expect("tempdir");
        let under_test = session_resolver(home.path());
        let request = SessionRequest {
            overrides: vec![("sandbox.profile".to_owned(), "loose".to_owned())],
            ..SessionRequest::default()
        };

        let diagnostic = under_test.resolve(&request).expect_err("refused");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    }

    #[test]
    fn the_session_profile_and_the_ask_mode_reach_the_resolution() {
        let home = tempfile::tempdir().expect("tempdir");
        let under_test = session_resolver(home.path());
        let request = SessionRequest {
            profile: Some("llm-only".to_owned()),
            ask_mode: Some(AskMode::Ui),
            work_mode: Some(WorkMode::Ro),
            overrides: vec![("hold.timeout_secs".to_owned(), "42".to_owned())],
            ..SessionRequest::default()
        };

        let resolved: Resolved = under_test.resolve(&request).expect("resolves");
        // Das Profil setzt `none`; die Kommandozeile steht darüber.
        assert_eq!(resolved.config.hold.ask_mode, AskMode::Ui);
        assert_eq!(resolved.config.hold.timeout_secs, 42);
        assert_eq!(resolved.config.sandbox.work_mode, WorkMode::Ro);
        assert!(
            resolved
                .profiles
                .iter()
                .any(|profile| profile.name == "llm-only"),
            "the chain carries llm-only"
        );
    }

    #[test]
    fn an_unknown_profile_is_config_001() {
        let home = tempfile::tempdir().expect("tempdir");
        let under_test = session_resolver(home.path());
        let request = SessionRequest {
            profile: Some("does-not-exist".to_owned()),
            ..SessionRequest::default()
        };

        let diagnostic = under_test.resolve(&request).expect_err("no such profile");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_001");
    }

    #[test]
    fn the_profile_rules_are_in_the_bundled_group() {
        let home = tempfile::tempdir().expect("tempdir");
        let under_test = session_resolver(home.path());
        let request = SessionRequest {
            profile: Some("llm-only".to_owned()),
            ..SessionRequest::default()
        };
        let resolved = under_test.resolve(&request).expect("resolves");

        let (rules, diagnostics) =
            bundled_rules(&resolved.config, &resolved.profiles, SessionId::new());
        assert!(
            rules
                .all()
                .iter()
                .any(|rule| rule.matcher.host.to_string() == "**"),
            "llm-only brings its block rule: {:#?}",
            rules.all()
        );
        assert!(
            diagnostics.is_empty(),
            "the bundled rules parse: {diagnostics:#?}"
        );
    }

    #[test]
    fn without_an_endpoint_there_is_no_passthrough() {
        let config = Config::default();
        assert!(config.llm.endpoint.is_none());
        let (rules, _) = bundled_rules(&config, &[], SessionId::new());
        assert!(
            rules.passthrough().is_none(),
            "nothing to pass through without llm.endpoint"
        );
        assert!(
            !rules.all().iter().any(|rule| rule.passthrough_llm),
            "and no other rule of the group carries the mark either"
        );
    }

    #[test]
    fn an_ask_mode_that_is_not_one_is_refused() {
        assert_eq!(parse_ask_mode("").expect("empty is no wish"), None);
        assert_eq!(
            parse_ask_mode("TERMINAL").expect("case does not matter"),
            Some(AskMode::Terminal)
        );
        let diagnostic = parse_ask_mode("sometimes").expect_err("no such mode");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    }
}
