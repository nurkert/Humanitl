//! Die Felder des Startvorschlags aus gesammelten Tatsachen (HUM-536).
//!
//! Nichts hier liest Dateien oder fragt das System: [`build`] setzt aus der
//! Auflösung, dem Plan des Sandbox-Dienstes, der Liste der Backends und dem
//! Stand der Agenten die Felder zusammen. Daemon und Fake rufen dieselbe
//! Funktion, damit beide dieselben Felder in derselben Form nennen.
//!
//! Die Vorgabe eines Felds ist immer der Wert, den die Auflösung ergibt, nie
//! ein Wert aus einer eigenen Tabelle: Ist dieser Wert nicht wählbar (etwa
//! eine Untergrenze, die das Backend der Vorgabe nicht erreicht), bleibt er
//! die Vorgabe, die Option sagt warum, und der Befund steht unter
//! `StartProposal.diagnostics`, wie der Start ihn gäbe.

use humanitl_config::agents::AgentSource;
use humanitl_config::{AskMode, PolicyLocks, Resolved, SandboxMode, WorkMode};
use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes::{CONFIG_003, SANDBOX_045};
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::{BackendEntry, IsolationLevel};

use crate::{convert, v1};

/// Das Feld des Agenten.
pub const AGENT: &str = "agent";
/// Das Feld der Quelle des gewählten Agenten (GitHub #15).
pub const AGENT_SOURCE: &str = "agent_source";
/// Das Feld des Ordnermodus.
pub const FOLDER_MODE: &str = "folder_mode";
/// Das Feld der Isolation.
pub const ISOLATION: &str = "isolation";
/// Das Feld des Nachfragemodus.
pub const ASK_MODE: &str = "ask_mode";
/// Das Feld des Modellservers.
pub const LLM_SERVER: &str = "llm_server";
/// Das Feld des Sandbox-Profils.
pub const PROFILE: &str = "profile";

/// Wegwerf-Schicht über dem Ordner.
pub const DISPOSABLE: &str = "disposable";
/// Direkt in den Ordner, schreibend.
pub const READ_WRITE: &str = "rw";
/// Direkt, nur lesend.
pub const READ_ONLY: &str = "ro";
/// Ein Backend über dem geteilten Kern des Hosts.
pub const SHARED_KERNEL: &str = "shared_kernel";
/// Die Kernel-Stufe.
pub const KERNEL: &str = "kernel";

/// Was ein Start mit der aufgelösten Konfiguration in diesem Ordner bekäme,
/// vom Sandbox-Dienst gebaut wie die Momentaufnahme eines `Plan`.
#[derive(Debug, Clone)]
pub struct PlanFacts {
    /// Die Stufe des Backends, das der Plan nimmt.
    pub isolation: IsolationLevel,
    /// Der Modus nach der Auflösung von `auto` (nie `auto`).
    pub mode: SandboxMode,
    /// Der Hinweis zum Modus: der Rückfall von `auto` (`SANDBOX_033`), der
    /// Befund eines ausdrücklichen `disposable`, das nicht geht, oder
    /// wartende Änderungen.
    pub mode_notice: Option<Diagnostic>,
    /// Ob `/work` schreibbar ist.
    pub work_mode: WorkMode,
    /// Das Sandbox-Profil.
    pub profile: String,
    /// Ob dieser Plan mit schreibbarem Ordner eine Wegwerf-Schicht bekäme.
    pub disposable: Result<(), Diagnostic>,
}

/// Woher das Programm eines Agenten käme und ob es dort liegt.
#[derive(Debug, Clone)]
pub struct AgentFacts {
    /// Die Kennung des Adapters.
    pub id: String,
    /// Der Name zur Anzeige.
    pub display_name: String,
    /// Die wirksame Quelle (`agents.<id>.source`, Sperre).
    pub source: AgentSource,
    /// Ob die Quelle feststeht und der Vorschlag sie nicht übergeht (GitHub
    /// #15): gesetzt in irgendeiner Ebene (`agents.<id>.source` oder
    /// `agents.source` aus `config.toml`, Umgebung, Projektwahl, Sperre),
    /// oder das Projekt hat eine gespeicherte Wahl, mit der ein Start die
    /// wirksame Quelle nähme. Nur sonst wählt er die Quelle vor, in der der
    /// Agent bereitliegt.
    pub fixed: bool,
    /// Die Firmenvorgabe, die die Quelle sperrt (ADR-0023 Punkt 1), wenn
    /// eine es tut; dann ist `source` immer `depot`.
    pub locked_by: Option<String>,
    /// Auf dem Host: gefunden, oder warum nicht.
    pub host: Result<(), Diagnostic>,
    /// Im Depot: bereit mit Hinweisen (etwa nicht angemeldet), oder warum
    /// nicht.
    pub depot: Result<Vec<Diagnostic>, Diagnostic>,
    /// Der Hinweis auf Cloud-Inferenz mit diesem Agenten.
    pub cloud: Option<v1::CloudInference>,
}

/// Ein gespeicherter Modellserver mit dem Hinweis auf Cloud-Inferenz.
#[derive(Debug, Clone)]
pub struct ServerFacts {
    /// Der Name in `llm.servers`.
    pub name: String,
    /// Der Hinweis, wenn eine Sitzung mit diesem Server die Inferenz an einen
    /// Anbieter außerhalb dieses Rechners schickt.
    pub cloud: Option<v1::CloudInference>,
    /// Warum ein Start mit diesem Server ablehnte, etwa eine Wahl des
    /// Projekt-Profils, die für diesen Ordner nicht bestätigt ist
    /// (`CONFIG_022`, wie `SessionResolver::resolve`).
    pub refusal: Option<Diagnostic>,
}

/// Ein Sandbox-Profil, das ein Start fände, und ob es sich laden lässt.
#[derive(Debug, Clone)]
pub struct ProfileFacts {
    /// Der Name in `sandbox.profile`.
    pub name: String,
    /// Geladen und geprüft wie beim Start, oder der Befund dazu.
    pub state: Result<(), Diagnostic>,
}

/// Alles, woraus [`build`] die Felder setzt.
#[derive(Debug)]
pub struct Inputs<'a> {
    /// Die Auflösung mit Projektwahl und Teilwahl.
    pub resolved: &'a Resolved,
    /// Die Sperren der Firmenvorgabe.
    pub locks: &'a PolicyLocks,
    /// Der Plan des Sandbox-Dienstes, oder warum er nicht entstand.
    pub plan: Result<PlanFacts, Diagnostic>,
    /// Die Backends, gemessen an der Gruppe `sandbox` der Auflösung.
    pub backends: &'a [BackendEntry],
    /// Je Adapter, woher sein Programm käme.
    pub agents: Vec<AgentFacts>,
    /// Die gespeicherten Modellserver.
    pub servers: Vec<ServerFacts>,
    /// Die Sandbox-Profile, die es gibt.
    pub profiles: Vec<ProfileFacts>,
    /// Der Hinweis auf Cloud-Inferenz der Vorgabe.
    pub cloud: Option<v1::CloudInference>,
}

/// Was [`build`] setzt; Pfad, Name und Kennung trägt der Aufrufer nach.
#[derive(Debug, Clone, Default)]
pub struct Built {
    /// Die Felder in der Reihenfolge des Bildschirms.
    pub fields: Vec<v1::ProposalField>,
    /// Die Wahl der Vorgaben, wie `save_choice` sie nimmt.
    pub choice: Vec<v1::ChoiceValue>,
    /// `Defaults: …`.
    pub defaults_line: String,
    /// Was dem Start mit den Vorgaben im Weg steht oder zu sagen ist.
    pub diagnostics: Vec<v1::Diagnostic>,
    /// Der Hinweis auf Cloud-Inferenz der Vorgabe.
    pub cloud: Option<v1::CloudInference>,
}

/// Setzt die Felder des Vorschlags.
#[must_use]
pub fn build(inputs: &Inputs<'_>) -> Built {
    let fields = vec![
        agent_field(inputs),
        source_field(inputs),
        folder_field(inputs),
        isolation_field(inputs),
        ask_field(inputs),
        server_field(inputs),
        profile_field(inputs),
    ];
    let mut diagnostics: Vec<v1::Diagnostic> = Vec::new();
    let mut add = |found: &v1::Diagnostic| {
        if !diagnostics
            .iter()
            .any(|known| known.code == found.code && known.why == found.why)
        {
            diagnostics.push(found.clone());
        }
    };
    for found in &inputs.resolved.diagnostics {
        add(&convert::diagnostic_to_proto(found));
    }
    match &inputs.plan {
        Ok(plan) => {
            if let Some(notice) = &plan.mode_notice {
                add(&convert::diagnostic_to_proto(notice));
            }
        }
        Err(refused) => add(&convert::diagnostic_to_proto(refused)),
    }
    for field in &fields {
        if let Some(chosen) = field
            .options
            .iter()
            .find(|option| option.value == field.value)
        {
            for reason in &chosen.reasons {
                add(reason);
            }
        }
    }
    // Agent und Quelle tragen beide `agents.<id>.source`, mit demselben
    // Wert; die Wahl nennt jeden Schlüssel einmal.
    let mut choice: Vec<v1::ChoiceValue> = Vec::new();
    for pair in fields
        .iter()
        .filter_map(|field| {
            field
                .options
                .iter()
                .find(|option| option.value == field.value)
        })
        .flat_map(|option| option.choice.iter())
    {
        if !choice.iter().any(|known| known.key == pair.key) {
            choice.push(pair.clone());
        }
    }
    Built {
        defaults_line: defaults_line(&fields),
        fields,
        choice,
        diagnostics,
        cloud: inputs.cloud.clone(),
    }
}

/// `Defaults: OpenCode from the host · disposable folder · …`: die Labels
/// der Vorgaben, ein Feld ohne Vorgabe mit seinem Hinweis.
fn defaults_line(fields: &[v1::ProposalField]) -> String {
    // Die Quelle steht schon beim Agenten („… from the depot").
    let parts: Vec<String> = fields
        .iter()
        .filter(|field| field.id != AGENT_SOURCE)
        .map(|field| {
            let chosen = field
                .options
                .iter()
                .find(|option| option.value == field.value);
            match (field.id.as_str(), chosen) {
                (AGENT, Some(option)) => format!("{} from the {}", option.label, option.source),
                (LLM_SERVER, Some(option)) => format!("model server {}", option.label),
                (LLM_SERVER, None) => "no saved model server".to_owned(),
                (PROFILE, Some(option)) => format!("profile {}", option.label),
                (_, Some(option)) => option.label.clone(),
                (_, None) => format!("{} unset", field.id),
            }
        })
        .collect();
    format!("Defaults: {}", parts.join(" · "))
}

/// Eine Option mit ihren Paaren, gemessen an den Sperren: Ein Paar auf
/// einem gesperrten Schlüssel fällt aus der Wahl (die Sperre ist schon eine
/// Ebene), und ein anderer Wert als der gesperrte macht die Option
/// unwählbar.
fn option(
    locks: &PolicyLocks,
    value: &str,
    label: &str,
    pairs: &[(&str, &str)],
    mut reasons: Vec<Diagnostic>,
) -> v1::ProposalOption {
    let mut choice = Vec::new();
    for (key, wanted) in pairs {
        match locks.covering(key) {
            Some((path, locked)) => {
                if locked.value != *wanted {
                    reasons.push(locked_reason(path, &locked.value, &locked.by));
                }
            }
            None => choice.push(v1::ChoiceValue {
                key: (*key).to_owned(),
                value: (*wanted).to_owned(),
            }),
        }
    }
    v1::ProposalOption {
        value: value.to_owned(),
        label: sanitize_note(label),
        available: reasons
            .iter()
            .all(|reason| reason.severity < Severity::Error),
        reasons: reasons.iter().map(convert::diagnostic_to_proto).collect(),
        choice,
        ..v1::ProposalOption::default()
    }
}

/// Eine Firmenvorgabe legt den Schlüssel auf einen anderen Wert fest.
fn locked_reason(path: &str, value: &str, by: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Blocking)
        .why(format!(
            "the company policy {} fixes {path} to {value}; this option is not available",
            sanitize_note(by)
        ))
        .build()
}

/// Ein Feld über diesen Schlüsseln mit Vorgabe, Herkunft und Sperre.
fn field(
    inputs: &Inputs<'_>,
    id: &str,
    keys: &[&str],
    value: &str,
    options: Vec<v1::ProposalOption>,
) -> v1::ProposalField {
    let lock = keys
        .iter()
        .find_map(|key| inputs.locks.covering(key))
        .map(|(path, locked)| v1::SettingLock {
            locked_by: sanitize_note(&locked.by),
            read_only: true,
            reason: sanitize_note(&format!(
                "the company policy fixes {path} to {}",
                locked.value
            )),
        });
    v1::ProposalField {
        id: id.to_owned(),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        value: value.to_owned(),
        origin: keys
            .first()
            .and_then(|key| inputs.resolved.origin(key))
            .map(|origin| origin.kind().to_owned())
            .unwrap_or_default(),
        lock,
        options,
    }
}

/// Der Schlüssel der Quelle eines Agenten in der Projektwahl.
fn source_key(agent: &str) -> String {
    format!("agents.{agent}.source")
}

/// Was eine Quelle eines Agenten hat: bereit mit Hinweisen, oder warum
/// nicht.
fn source_state(agent: &AgentFacts, source: AgentSource) -> Result<Vec<Diagnostic>, Diagnostic> {
    match source {
        AgentSource::Host => agent.host.clone().map(|()| Vec::new()),
        AgentSource::Depot => agent.depot.clone(),
    }
}

/// Die Quelle, die der Vorschlag für einen Agenten vorwählt (GitHub #15,
/// Nachtrag zu ADR-0023):
///
/// 1. die Sperre der Firmenvorgabe, immer `depot`;
/// 2. eine feststehende Quelle, so wie der Start sie nähme
///    ([`AgentFacts::fixed`]): gesetzt in irgendeiner Ebene, oder das
///    Projekt hat schon eine gespeicherte Wahl;
/// 3. sonst, für eine ungesetzte Quelle, die Vorgabe `host`, wenn der Agent
///    dort bereitliegt, und wenn nicht, das Depot, wenn er dort bereitliegt;
///    liegt er nirgends, bleibt `host`.
///
/// Gespeichert gilt die Wahl für den Start; ohne Projektwahl gilt ADR-0023
/// unverändert, ohne Rückfall auf die andere Quelle.
#[must_use]
pub fn preselected(agent: &AgentFacts) -> AgentSource {
    if agent.locked_by.is_some() {
        return humanitl_config::agents::Locked::SOURCE;
    }
    if agent.fixed || source_state(agent, agent.source).is_ok() {
        return agent.source;
    }
    let other = match agent.source {
        AgentSource::Host => AgentSource::Depot,
        AgentSource::Depot => AgentSource::Host,
    };
    if source_state(agent, other).is_ok() {
        other
    } else {
        agent.source
    }
}

/// Das Paar einer Quelle eines Agenten, außer die Sperre der Quelle hält
/// den Schlüssel aus der Wahl. Einen gesperrten Schlüssel der Firmenvorgabe
/// nimmt [`option`] heraus, wie bei jedem Feld.
fn source_pair(agent: &AgentFacts, source: AgentSource) -> Option<(String, &'static str)> {
    agent
        .locked_by
        .is_none()
        .then(|| (source_key(&agent.id), source.as_str()))
}

/// Der Satz einer Sperre der Quelle: Die Firmenvorgabe legt die Quelle des
/// Agenten auf das Depot fest.
fn source_locked(agent: &AgentFacts, by: &str) -> Diagnostic {
    humanitl_config::agents::locked_refusal(
        &agent.id,
        &humanitl_config::agents::Locked { by: by.to_owned() },
        "the source host",
    )
}

fn agent_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let options = inputs
        .agents
        .iter()
        .map(|agent| {
            let chosen = preselected(agent);
            let (host_ok, mut host_reasons) = split(&source_state(agent, AgentSource::Host));
            let (depot_ok, depot_reasons) = split(&source_state(agent, AgentSource::Depot));
            if let Some(by) = &agent.locked_by {
                host_reasons.push(convert::diagnostic_to_proto(&source_locked(agent, by)));
            }
            let sources = vec![
                v1::AgentSourceOption {
                    source: AgentSource::Host.as_str().to_owned(),
                    available: host_ok && agent.locked_by.is_none(),
                    reasons: host_reasons,
                    selected: chosen == AgentSource::Host,
                },
                v1::AgentSourceOption {
                    source: AgentSource::Depot.as_str().to_owned(),
                    available: depot_ok,
                    reasons: depot_reasons,
                    selected: chosen == AgentSource::Depot,
                },
            ];
            // Die Gründe der vorgewählten Quelle: Ein Start nimmt nie die
            // andere (ADR-0023, `AGENT_008`).
            let state = source_state(agent, chosen);
            let ready = state.is_ok();
            let reasons = state.unwrap_or_else(|missing| vec![missing]);
            // Die Paare tragen die Quelle (GitHub #15): Wer diesen Agenten
            // wählt und speichert, startet ihn aus der vorgewählten Quelle.
            let source = source_pair(agent, chosen);
            let mut pairs: Vec<(&str, &str)> = vec![("agent.adapter", agent.id.as_str())];
            if let Some((key, value)) = &source {
                pairs.push((key.as_str(), value));
            }
            let mut built = option(
                inputs.locks,
                &agent.id,
                &agent.display_name,
                &pairs,
                reasons,
            );
            built.available &= ready;
            chosen.as_str().clone_into(&mut built.source);
            built.sources = sources;
            built.cloud_inference.clone_from(&agent.cloud);
            built
        })
        .collect();
    let mut built = field(
        inputs,
        AGENT,
        &["agent.adapter"],
        &inputs.resolved.config.agent.adapter,
        options,
    );
    // Wer einen anderen Agenten wählt, nimmt die Quelle des bisherigen aus
    // der Wahl: Das Feld schreibt auch jeden Schlüssel einer Quelle. Die
    // Sperre des Felds bleibt die von `agent.adapter`.
    built
        .keys
        .extend(inputs.agents.iter().map(|agent| source_key(&agent.id)));
    built
}

/// Die Quelle des gewählten Agenten als eigenes Feld (GitHub #15): `host`
/// und `depot`, jede mit ihren Paaren, ihrer Verfügbarkeit und ihrem Grund.
/// Eine Sperre der Quelle macht das Feld nur lesbar, mit der Vorgabe als
/// Herkunft; `host` ist dann nicht wählbar und sagt warum.
fn source_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let adapter = &inputs.resolved.config.agent.adapter;
    let key = source_key(adapter);
    let Some(agent) = inputs.agents.iter().find(|agent| &agent.id == adapter) else {
        // Ein Agent ohne Tatsachen (kein Adapter dieses Namens): keine
        // Option; der Start scheitert ohnehin am Adapter.
        return field(inputs, AGENT_SOURCE, &[key.as_str()], "", Vec::new());
    };
    let chosen = preselected(agent);
    let options = [
        (AgentSource::Host, "host"),
        (AgentSource::Depot, "agent depot"),
    ]
    .into_iter()
    .map(|(source, label)| {
        let state = source_state(agent, source);
        let ready = state.is_ok();
        let mut reasons = state.unwrap_or_else(|missing| vec![missing]);
        if source == AgentSource::Host
            && let Some(by) = &agent.locked_by
        {
            reasons.push(source_locked(agent, by));
        }
        let pairs: Vec<(String, &str)> = source_pair(agent, source).into_iter().collect();
        let pairs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        let mut built = option(inputs.locks, source.as_str(), label, &pairs, reasons);
        built.available &= ready;
        built
    })
    .collect();
    let mut built = field(
        inputs,
        AGENT_SOURCE,
        &[key.as_str()],
        chosen.as_str(),
        options,
    );
    if let Some(by) = &agent.locked_by {
        built.lock = Some(v1::SettingLock {
            locked_by: sanitize_note(by),
            read_only: true,
            reason: sanitize_note(&format!(
                "the company policy fixes {key} to {}",
                humanitl_config::agents::Locked::SOURCE.as_str()
            )),
        });
        "policy".clone_into(&mut built.origin);
    }
    built
}

/// Ob eine Quelle bereit ist, und ihre Sätze: Fehlt das Programm, der
/// Befund; liegt es da, die Hinweise dazu.
fn split(state: &Result<Vec<Diagnostic>, Diagnostic>) -> (bool, Vec<v1::Diagnostic>) {
    match state {
        Ok(notes) => (
            true,
            notes.iter().map(convert::diagnostic_to_proto).collect(),
        ),
        Err(missing) => (false, vec![convert::diagnostic_to_proto(missing)]),
    }
}

fn folder_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let config = &inputs.resolved.config;
    let (value, disposable) = match &inputs.plan {
        Ok(plan) => (
            folder_value(plan.mode, plan.work_mode),
            plan.disposable.clone().err(),
        ),
        // Ohne Plan steht dem Start ohnehin dessen Befund im Weg; der Modus
        // ist dann der der Konfiguration, und über die Schicht lässt sich
        // nichts sagen.
        Err(_) => (
            folder_value(
                if config.sandbox.mode == SandboxMode::Direct {
                    SandboxMode::Direct
                } else {
                    SandboxMode::Disposable
                },
                config.sandbox.work_mode,
            ),
            None,
        ),
    };
    // Sperrt die Firmenvorgabe `sandbox.mode = "auto"`, gilt für den
    // Vergleich je Option der Modus, zu dem ein Start `auto` mit deren
    // Schreibmodus auflöste: schreibbar und mit Wegwerf-Schicht `disposable`,
    // sonst `direct`. Der gesperrte Schlüssel bleibt aus jedem Paar.
    let layer_ok = disposable.is_none();
    let rw = folder_locks(inputs.locks, WorkMode::Rw, layer_ok);
    let ro = folder_locks(inputs.locks, WorkMode::Ro, layer_ok);
    let mut layer = option(
        &rw,
        DISPOSABLE,
        "disposable folder",
        &[("sandbox.mode", DISPOSABLE), ("sandbox.work_mode", "rw")],
        disposable.into_iter().collect(),
    );
    layer.available &= layer_ok;
    let options = vec![
        layer,
        option(
            &rw,
            READ_WRITE,
            "folder read-write",
            &[("sandbox.mode", "direct"), ("sandbox.work_mode", "rw")],
            Vec::new(),
        ),
        option(
            &ro,
            READ_ONLY,
            "folder read-only",
            &[("sandbox.mode", "direct"), ("sandbox.work_mode", "ro")],
            Vec::new(),
        ),
    ];
    field(
        inputs,
        FOLDER_MODE,
        &["sandbox.mode", "sandbox.work_mode"],
        value,
        options,
    )
}

/// Die Sperren für den Vergleich einer Ordner-Option: eine Sperre auf
/// `sandbox.mode = "auto"` steht für den Modus, zu dem ein Start `auto` mit
/// dem Schreibmodus `work` der Option auflöste (HUM-357): `disposable` nur
/// mit schreibbarem Ordner und einer Wegwerf-Schicht, sonst `direct`.
fn folder_locks(locks: &PolicyLocks, work: WorkMode, layer_ok: bool) -> PolicyLocks {
    match locks.get("sandbox.mode") {
        Some(locked) if SandboxMode::parse(&locked.value) == Some(SandboxMode::Auto) => {
            let resolved = if work == WorkMode::Rw && layer_ok {
                SandboxMode::Disposable
            } else {
                SandboxMode::Direct
            };
            locks
                .clone()
                .with("sandbox.mode", resolved.as_str(), locked.by.clone())
        }
        _ => locks.clone(),
    }
}

const fn folder_value(mode: SandboxMode, work: WorkMode) -> &'static str {
    match (mode, work) {
        (SandboxMode::Direct, WorkMode::Ro) => READ_ONLY,
        (SandboxMode::Direct, WorkMode::Rw) => READ_WRITE,
        _ => DISPOSABLE,
    }
}

fn isolation_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let sandbox = &inputs.resolved.config.sandbox;
    let configured = humanitl_sandbox::backend_name(sandbox.backend);
    // Die gemeinsame Stufe ist das Backend, das die Konfiguration nennt,
    // solange es eines über dem geteilten Kern ist; sonst das des Systems.
    let shared = if configured == humanitl_sandbox::VM_BACKEND {
        humanitl_sandbox::AUTO_BACKEND
    } else {
        configured
    };
    let shared_name = if shared == humanitl_sandbox::AUTO_BACKEND {
        humanitl_sandbox::PLATFORM_BACKEND_NAME
    } else {
        shared
    };
    let entry = |name: &str| inputs.backends.iter().find(|entry| entry.name == name);
    let shared_reasons = entry(shared_name).map_or_else(
        || {
            vec![not_listed(
                shared_name,
                "this backend is not offered on this system",
            )]
        },
        |entry| entry.diagnostics.clone(),
    );
    let kernel_reasons = entry(humanitl_sandbox::VM_BACKEND).map_or_else(
        || {
            vec![not_listed(
                humanitl_sandbox::VM_BACKEND,
                "this kind of system has no backend of the kernel level",
            )]
        },
        |entry| entry.diagnostics.clone(),
    );
    let level = match &inputs.plan {
        Ok(plan) => plan.isolation,
        Err(_) => humanitl_sandbox::choose_backend(sandbox, humanitl_sandbox::AUTO_BACKEND)
            .map_or_else(
                |_| {
                    if configured == humanitl_sandbox::VM_BACKEND {
                        IsolationLevel::Kernel
                    } else {
                        IsolationLevel::Namespaces
                    }
                },
                |choice| choice.isolation,
            ),
    };
    let value = if level >= IsolationLevel::Kernel {
        KERNEL
    } else {
        SHARED_KERNEL
    };
    // Wie in der Liste der Backends: Jeder Befund, auch „wird geprüft"
    // (`DOCTOR_012`, eine Warnung), macht eine Stufe unwählbar.
    let level_option = |value: &str, label: &str, backend: &str, reasons: Vec<Diagnostic>| {
        let blocked = !reasons.is_empty();
        let mut built = option(
            inputs.locks,
            value,
            label,
            &[("sandbox.backend", backend)],
            reasons,
        );
        built.available &= !blocked;
        built
    };
    let options = vec![
        level_option(SHARED_KERNEL, "shared kernel", shared, shared_reasons),
        level_option(
            KERNEL,
            "kernel level",
            humanitl_sandbox::VM_BACKEND,
            kernel_reasons,
        ),
    ];
    field(inputs, ISOLATION, &["sandbox.backend"], value, options)
}

/// Ein Backend, das die Liste nicht nennt.
fn not_listed(name: &str, why: &str) -> Diagnostic {
    let code = if name == humanitl_sandbox::VM_BACKEND {
        SANDBOX_045
    } else {
        CONFIG_003
    };
    Diagnostic::builder(code, Severity::Blocking)
        .why(why.to_owned())
        .fix(FixAction::CopyCommand(
            "humanitl sandbox backends".to_owned(),
        ))
        .build()
}

fn ask_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let config = &inputs.resolved.config;
    let upstream_missing = config.egress.proxy.is_none().then(|| {
        Diagnostic::builder(CONFIG_003, Severity::Blocking)
            .why(
                "asking the upstream proxy needs egress.proxy, and none is set; requests \
                 without a rule would have no one to decide them"
                    .to_owned(),
            )
            .fix(FixAction::CopyCommand(
                "humanitl config set egress.proxy http://proxy.example:8080".to_owned(),
            ))
            .build()
    });
    let options = [
        (AskMode::Ui, "ui", "ask in the app", None),
        (AskMode::Terminal, "terminal", "ask in the terminal", None),
        (AskMode::None, "none", "do not ask", None),
        (
            AskMode::Upstream,
            "upstream",
            "ask the upstream proxy",
            upstream_missing,
        ),
    ];
    let value = options
        .iter()
        .find(|(mode, ..)| *mode == config.hold.ask_mode)
        .map_or("", |(_, name, ..)| *name);
    let options = options
        .into_iter()
        .map(|(_, name, label, missing)| {
            option(
                inputs.locks,
                name,
                label,
                &[("hold.ask_mode", name)],
                missing.into_iter().collect(),
            )
        })
        .collect();
    field(inputs, ASK_MODE, &["hold.ask_mode"], value, options)
}

fn server_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let chosen = inputs
        .resolved
        .config
        .llm
        .choice
        .as_ref()
        .map(|choice| choice.name.clone())
        .unwrap_or_default();
    let mut options: Vec<v1::ProposalOption> = inputs
        .servers
        .iter()
        .map(|server| {
            let mut built = option(
                inputs.locks,
                &server.name,
                &server.name,
                &[(humanitl_config::servers::SERVER, server.name.as_str())],
                server.refusal.clone().into_iter().collect(),
            );
            built.cloud_inference.clone_from(&server.cloud);
            built
        })
        .collect();
    // Ein Name, unter dem nichts gespeichert ist, bleibt die Vorgabe; der
    // Start lehnt ihn ab (`CONFIG_021`), und die Option sagt es.
    if !chosen.is_empty() && !options.iter().any(|option| option.value == chosen) {
        let refusal = inputs
            .resolved
            .config
            .llm
            .choice
            .as_ref()
            .and_then(humanitl_config::servers::ServerChoice::refusal);
        options.push(option(
            inputs.locks,
            &chosen,
            &chosen,
            &[(humanitl_config::servers::SERVER, chosen.as_str())],
            refusal.into_iter().collect(),
        ));
    }
    field(
        inputs,
        LLM_SERVER,
        &[humanitl_config::servers::SERVER],
        &chosen,
        options,
    )
}

fn profile_field(inputs: &Inputs<'_>) -> v1::ProposalField {
    let value = match &inputs.plan {
        Ok(plan) => plan.profile.clone(),
        Err(_) => inputs.resolved.config.sandbox.profile.clone(),
    };
    let mut profiles = inputs.profiles.clone();
    if !profiles.iter().any(|profile| profile.name == value) {
        profiles.push(ProfileFacts {
            name: value.clone(),
            state: Err(Diagnostic::builder(
                humanitl_core::diagnostics::codes::CONFIG_001,
                Severity::Blocking,
            )
            .why(format!("no sandbox profile {value} is installed"))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.profile".to_owned(),
                value: "default".to_owned(),
            })
            .build()),
        });
    }
    let options = profiles
        .iter()
        .map(|profile| {
            // Ein Profil, das sich nicht laden lässt, ist nie wählbar,
            // gleich wie schwer sein Befund wiegt.
            let broken = profile.state.is_err();
            let mut built = option(
                inputs.locks,
                &profile.name,
                &profile.name,
                &[("sandbox.profile", profile.name.as_str())],
                profile.state.clone().err().into_iter().collect(),
            );
            built.available &= !broken;
            built
        })
        .collect();
    field(inputs, PROFILE, &["sandbox.profile"], &value, options)
}
