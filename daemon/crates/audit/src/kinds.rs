//! Alle Arten von Records und was jede davon in `data` trägt (HUM-050).
//!
//! Jede Art ist eine Struktur mit festen Feldern aus Ganzzahlen, Strings,
//! Booleans und Listen davon. `data` entsteht hier mit `json!` und nie aus
//! einer `HashMap` oder einem `DateTime`: Die Reihenfolge legt allein die
//! Kanonisierung fest, und jeder Zeitpunkt geht durch
//! [`format_ts`].
//!
//! **Was nie in `data` steht.** Bodies, Header, Klartext-Werte von Funden,
//! Originale von Pseudonymen, die Notiz einer Blockierung. Der Pfad einer
//! Anfrage steht nur als `path_hash`: Query-Strings tragen Tokens. Der Host
//! steht im Klartext; das ist der dokumentierte Seitenkanal aus BACKLOG.md
//! 4.2. Die Konstruktoren unten nehmen deshalb die Typen des Kerns und suchen
//! sich selbst heraus, was ins Log darf, statt es dem Aufrufer zu überlassen.
//!
//! Die Namen folgen der Tabelle aus `backlog/sprint-4.md`, `bereich.vorgang`.
//! `llm.discover` kommt aus HUM-076 dazu; dort steht es in der Prosa als
//! `llm_discover`, geschrieben, bevor es die Tabelle gab. `audit.resumed`
//! kommt aus dem Review von HUM-050 dazu: der erste Record hinter einer
//! Lücke, die ein gekürztes oder ersetztes Log hinterlässt (siehe
//! [`crate::writer`]).

use chrono::{DateTime, Utc};
use humanitl_core::rule::{Expiry, Rule};
use humanitl_core::{Decision, DecisionSource, Finding, FlowId, HttpRequest};
use serde_json::{Value, json};

use crate::record::{format_ts, sha256_hex};

/// Woher der HMAC-Schlüssel stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOrigin {
    /// Aus dem Keyring des Nutzers (HUM-048).
    Keyring,
    /// Aus einer Datei im Datenverzeichnis, bis HUM-048 den Keyring bringt.
    File,
}

impl KeyOrigin {
    /// Der Wert, wie er im Log steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::File => "file",
        }
    }
}

/// `daemon.started`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonStarted {
    /// Die Fassung des Daemons.
    pub version: String,
    /// Die Fassung des Vertrags, `major.minor`.
    pub proto_version: String,
    /// Woher der HMAC-Schlüssel stammt.
    pub key_origin: KeyOrigin,
}

/// `daemon.stopped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonStopped {
    /// Warum, zum Beispiel `sigterm`.
    pub reason: String,
}

/// `session.started`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStarted {
    /// Name des Sandbox-Profils.
    pub profile: String,
    /// Kennung des Agent-Adapters.
    pub agent: String,
    /// SHA-256 des kanonischen Arbeitsverzeichnisses; der Pfad selbst nennt
    /// Nutzer- und Projektnamen.
    pub work_dir_hash: String,
    /// `ro` oder `rw`.
    pub work_mode: String,
    /// Der Host des Sprachmodells, falls einer eingestellt ist.
    pub llm_endpoint_host: Option<String>,
    /// Das Sandbox-Backend, falls beim Start der Sitzung schon eines feststeht.
    pub sandbox_backend: Option<String>,
    /// SHA-256 der Kommandozeile der Sandbox, falls schon eine gebaut ist.
    pub argv_hash: Option<String>,
}

/// `session.ended`: die Zahlen der Sitzung.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionEnded {
    /// Alle Flows, die angekommen sind.
    pub flows_total: u64,
    /// Davon angehalten, um zu fragen.
    pub held: u64,
    /// Von einem Menschen unverändert erlaubt.
    pub allowed: u64,
    /// Von einem Menschen bearbeitet erlaubt.
    pub allowed_edited: u64,
    /// Von einem Menschen oder vom Daemon geblockt.
    pub blocked: u64,
    /// In die Frist gelaufen.
    pub timed_out: u64,
    /// Von einer Regel entschieden.
    pub auto_rule: u64,
    /// Durch die Durchreiche zum Sprachmodell.
    pub passthrough: u64,
}

/// Eine Zeile von `isolation.check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    /// Die Prüfung, zum Beispiel `no_network_interface`.
    pub check: String,
    /// Ob sie bestanden ist.
    pub passed: bool,
}

/// `isolation.check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationCheck {
    /// Jede Prüfung mit ihrem Ergebnis.
    pub results: Vec<CheckOutcome>,
}

/// `flow.received`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowReceived {
    /// Der Flow.
    pub flow: String,
    /// Die Methode.
    pub method: String,
    /// Das Schema.
    pub scheme: String,
    /// Der Host im Klartext (BACKLOG.md 4.2).
    pub host: String,
    /// Der Port.
    pub port: u16,
    /// SHA-256 von Pfad und Query; der Pfad selbst kann Tokens tragen.
    pub path_hash: String,
    /// Größe des Bodys in Bytes.
    pub size: u64,
    /// Zahl der Funde.
    pub findings: u64,
    /// Die Arten der Funde, sortiert und ohne Doppelte, ohne Werte.
    pub findings_kinds: Vec<String>,
}

impl FlowReceived {
    /// Was aus einer Anfrage ins Log darf.
    ///
    /// Aus `request` werden Methode, Schema, Host, Port und Größe gelesen,
    /// der Pfad nur als Prüfsumme. Header und Body werden nicht angesehen.
    /// Aus `findings` zählen Zahl und Art; `FindingKind::as_str` nennt nie den
    /// Wert, auch nicht bei `user_term` und `custom`.
    #[must_use]
    pub fn new(flow: FlowId, request: &HttpRequest, findings: &[Finding]) -> Self {
        let mut kinds: Vec<String> = findings
            .iter()
            .map(|finding| finding.kind.as_str().to_owned())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        Self {
            flow: flow.to_string(),
            method: request.method.as_str().to_owned(),
            scheme: request.scheme.as_str().to_owned(),
            host: request.authority.host.to_string(),
            port: request.authority.port,
            path_hash: sha256_hex(request.path_and_query.as_bytes()),
            size: request.body.size,
            findings: u64::try_from(findings.len()).unwrap_or(u64::MAX),
            findings_kinds: kinds,
        }
    }

    /// Derselbe Record mit den Funden, die erst nach dem Eintreffen feststehen.
    ///
    /// Der Ereignisstrom meldet die Anfrage (`Received`) vor den Funden
    /// (`Analyzed`); geschrieben wird der Record, sobald beide da sind.
    #[must_use]
    pub fn with_findings(mut self, findings: &[Finding]) -> Self {
        let mut kinds: Vec<String> = findings
            .iter()
            .map(|finding| finding.kind.as_str().to_owned())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        self.findings = u64::try_from(findings.len()).unwrap_or(u64::MAX);
        self.findings_kinds = kinds;
        self
    }
}

/// Wie über einen Flow entschieden wurde, in der Sprache des Logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    /// Ein Mensch hat unverändert erlaubt.
    Allow,
    /// Ein Mensch hat bearbeitet erlaubt.
    AllowEdited,
    /// Ein Mensch oder der Daemon hat geblockt.
    Block,
    /// Die Frist ist abgelaufen.
    TimedOut,
    /// Eine Regel hat erlaubt.
    AutoAllow,
    /// Eine Regel hat geblockt.
    AutoBlock,
    /// Die Durchreiche zum Sprachmodell.
    Passthrough,
}

impl DecisionKind {
    /// Die Entscheidung aus Entscheidung und Quelle.
    #[must_use]
    pub const fn of(decision: &Decision, source: DecisionSource) -> Self {
        match (decision, source) {
            (_, DecisionSource::Passthrough) => Self::Passthrough,
            (Decision::TimedOut, _) | (_, DecisionSource::Timeout) => Self::TimedOut,
            (Decision::Allow | Decision::AllowEdited { .. }, DecisionSource::Rule(_)) => {
                Self::AutoAllow
            }
            (Decision::Block { .. }, DecisionSource::Rule(_)) => Self::AutoBlock,
            (Decision::Allow, _) => Self::Allow,
            (Decision::AllowEdited { .. }, _) => Self::AllowEdited,
            (Decision::Block { .. }, _) => Self::Block,
        }
    }

    /// Der Wert, wie er im Log steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::AllowEdited => "allow_edited",
            Self::Block => "block",
            Self::TimedOut => "timed_out",
            Self::AutoAllow => "auto_allow",
            Self::AutoBlock => "auto_block",
            Self::Passthrough => "passthrough",
        }
    }
}

/// `flow.decided`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowDecided {
    /// Der Flow.
    pub flow: String,
    /// Die Entscheidung.
    pub decision: DecisionKind,
    /// Die Regel, die entschieden hat, sonst `None`.
    pub rule: Option<String>,
    /// Ob die Anfrage bearbeitet hinausging.
    pub edited: bool,
    /// Zahl der Ersetzungen im Editor. `None`, solange der Editor sie nicht
    /// meldet: Die Entscheidung trägt die bearbeitete Anfrage, nicht die Zahl.
    pub replacements: Option<u64>,
    /// Funde, die ohne Ersetzung hinausgingen. `None` aus demselben Grund.
    pub unresolved_findings: Option<u64>,
    /// Bestätigte Funde. `None`, bis die Oberfläche Bestätigungen meldet.
    pub acknowledged: Option<u64>,
    /// Werte, die mit dieser Entscheidung auf die Ausnahmeliste kamen. `None`,
    /// bis es diesen Weg gibt.
    pub allowlisted_added: Option<u64>,
    /// Wer entschieden hat: `user`, `rule`, `timeout` oder `system`.
    pub decided_by: &'static str,
}

impl FlowDecided {
    /// Was aus einer Entscheidung ins Log darf.
    ///
    /// Die Notiz einer Blockierung bleibt draußen: Sie ist Freitext eines
    /// Menschen und kann alles enthalten. Die bearbeitete Anfrage ebenso; von
    /// ihr steht nur da, dass es sie gibt.
    #[must_use]
    pub fn new(flow: FlowId, decision: &Decision, source: DecisionSource) -> Self {
        let rule = match (source, decision) {
            (DecisionSource::Rule(id), _) => Some(id.to_string()),
            (_, Decision::Block { reason, .. }) => reason.rule_id().map(|id| id.to_string()),
            _ => None,
        };
        let decided_by = match source {
            DecisionSource::User => "user",
            DecisionSource::Rule(_) | DecisionSource::Passthrough => "rule",
            DecisionSource::Timeout => "timeout",
            DecisionSource::System => "system",
        };
        Self {
            flow: flow.to_string(),
            decision: DecisionKind::of(decision, source),
            rule,
            edited: matches!(decision, Decision::AllowEdited { .. }),
            replacements: None,
            unresolved_findings: None,
            acknowledged: None,
            allowlisted_added: None,
            decided_by,
        }
    }
}

/// `flow.forwarded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowForwarded {
    /// Der Flow.
    pub flow: String,
    /// Die festgenagelte Zieladresse; bewusst geloggt, als Nachweis gegen
    /// DNS-Rebinding.
    pub upstream_ip: String,
}

/// `flow.responded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowResponded {
    /// Der Flow.
    pub flow: String,
    /// Der Status der Antwort.
    pub status: u16,
    /// Größe des Antwortkörpers in Bytes.
    pub size: u64,
    /// Dauer in ganzen Millisekunden.
    pub duration_ms: u64,
    /// Ob die Antwort gestreamt statt gepuffert durchlief.
    pub streamed: bool,
}

/// `flow.blocked_reason`: ein Block ohne Entscheidung eines Menschen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowBlockedReason {
    /// Der Flow.
    pub flow: String,
    /// Der Grund in `snake_case`, zum Beispiel `body_cap`.
    pub reason: String,
}

impl FlowBlockedReason {
    /// Der Grund eines Blocks, den der Daemon selbst ausgesprochen hat.
    ///
    /// `None` für jede andere Entscheidung: Einen Block durch Mensch, Regel
    /// oder Frist beschreibt `flow.decided` schon vollständig.
    #[must_use]
    pub fn of(flow: FlowId, decision: &Decision, source: DecisionSource) -> Option<Self> {
        match (decision, source) {
            (Decision::Block { reason, .. }, DecisionSource::System) => Some(Self {
                flow: flow.to_string(),
                reason: reason.as_str().to_owned(),
            }),
            _ => None,
        }
    }
}

/// Woher eine Änderung an den Regeln kam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOrigin {
    /// Aus der Oberfläche.
    Ui,
    /// Von der Kommandozeile.
    Cli,
    /// Mitgeliefert.
    Bundled,
    /// Aus `DecideRequest.remember`.
    Remember,
    /// Über den `Rules`-RPC, dessen Aufrufer sich nicht ausweist: Oberfläche
    /// und Kommandozeile schicken dieselbe Nachricht.
    Rpc,
}

impl RuleOrigin {
    /// Der Wert, wie er im Log steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Cli => "cli",
            Self::Bundled => "bundled",
            Self::Remember => "remember",
            Self::Rpc => "rpc",
        }
    }
}

/// `rule.added`, `rule.updated`, `rule.removed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleChange {
    /// Die Regel.
    pub rule: String,
    /// `allow`, `block`, `ask` oder `redact`.
    pub action: String,
    /// Das Host-Muster.
    pub match_host: String,
    /// Die Methoden; leer heißt jede.
    pub match_method: Vec<String>,
    /// `never`, `session` oder ein Zeitpunkt.
    pub expires: String,
    /// Der Flow, aus dem die Regel entstand.
    pub created_from: Option<String>,
    /// Woher die Änderung kam.
    pub origin: RuleOrigin,
}

impl RuleChange {
    /// Was aus einer Regel ins Log darf.
    ///
    /// Das Pfadmuster und die Notiz bleiben draußen; beide sind Text, den ein
    /// Mensch geschrieben hat und der mehr verraten kann als der Host.
    #[must_use]
    pub fn new(rule: &Rule, origin: RuleOrigin) -> Self {
        Self {
            rule: rule.id.to_string(),
            action: rule.action.as_str().to_owned(),
            match_host: rule.matcher.host.to_string(),
            match_method: rule
                .matcher
                .methods
                .as_ref()
                .map(|methods| {
                    methods
                        .iter()
                        .map(|method| method.as_str().to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            expires: expiry_text(&rule.expires),
            created_from: rule.created_from.map(|flow| flow.to_string()),
            origin,
        }
    }
}

fn expiry_text(expires: &Expiry) -> String {
    match expires {
        Expiry::Never => "never".to_owned(),
        Expiry::Session(_) => "session".to_owned(),
        Expiry::At(at) => format_ts(*at),
    }
}

/// `config.changed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChanged {
    /// Der Schlüssel, zum Beispiel `hold.timeout_secs`.
    pub key: String,
    /// Die Ebene, auf der er geändert wurde.
    pub origin: String,
    /// Ob der Wert geheim ist; dann steht er nicht im Log.
    pub secret: bool,
    /// Der neue Wert, nur wenn er nicht geheim ist.
    pub value: Option<String>,
}

/// `pseudonym.created`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudonymCreated {
    /// Das Pseudonym, nie das Original.
    pub pseudonym: String,
    /// Die Art des ersetzten Werts.
    pub kind: String,
}

/// `finding.allowlisted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingAllowlisted {
    /// Die Art des Funds.
    pub kind: String,
    /// Wofür die Ausnahme gilt.
    pub scope: String,
}

/// `audit.anchor`: Der Record, der selbst der Anker ist.
///
/// Er nennt seinen Vorgänger; sein eigener Hash steht mit seiner Nummer in
/// `audit_anchors` (siehe [`crate::writer`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorData {
    /// Die Nummer des Vorgängers.
    pub anchored_seq: u64,
    /// Der Hash des Vorgängers.
    pub anchored_hash: String,
}

/// `audit.verified`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditVerified {
    /// `ok` oder `broken`.
    pub result: String,
    /// Die erste fehlerhafte Nummer, falls die Kette gebrochen ist.
    pub first_bad_seq: Option<u64>,
    /// Wie viele Records geprüft wurden.
    pub records: u64,
}

/// `llm.discover`: eine Suche nach Sprachmodellen im eigenen Netz (HUM-076).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmDiscover {
    /// Das Netz in CIDR-Schreibweise.
    pub subnet: String,
    /// Die gefragten Ports.
    pub ports: Vec<u16>,
    /// Wie viele Server geantwortet haben, bevor die Suche endete.
    pub found: u64,
}

/// `audit.resumed`: Die Kette läuft hinter einem Anker weiter, den das Log
/// beim Start nicht mehr erreichte (HUM-050).
///
/// Der Record steht als erster hinter der Lücke. Seine Nummer folgt auf den
/// Anker, sein `prev` ist dessen Hash; `verify` meldet die Lücke davor weiter
/// als Bruch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditResumed {
    /// Die letzte Nummer, die das Log beim Start noch trug; `0` für ein
    /// leeres oder fehlendes Log.
    pub log_seq: u64,
    /// Die Nummer des letzten Ankers in `audit_anchors`.
    pub anchor_seq: u64,
}

/// Eine Art von Record samt ihren Daten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordKind {
    /// Der Daemon ist gestartet.
    DaemonStarted(DaemonStarted),
    /// Der Daemon endet.
    DaemonStopped(DaemonStopped),
    /// Eine Sitzung beginnt.
    SessionStarted(SessionStarted),
    /// Eine Sitzung endet.
    SessionEnded(SessionEnded),
    /// Die Garantien der Sandbox wurden gemessen.
    IsolationCheck(IsolationCheck),
    /// Eine Anfrage ist angekommen.
    FlowReceived(FlowReceived),
    /// Über eine Anfrage ist entschieden.
    FlowDecided(FlowDecided),
    /// Eine Anfrage ist zum Ziel unterwegs.
    FlowForwarded(FlowForwarded),
    /// Die Antwort ist durch.
    FlowResponded(FlowResponded),
    /// Der Daemon hat selbst geblockt.
    FlowBlockedReason(FlowBlockedReason),
    /// Eine Regel ist dazugekommen.
    RuleAdded(RuleChange),
    /// Eine Regel wurde geändert.
    RuleUpdated(RuleChange),
    /// Eine Regel ist weg.
    RuleRemoved(RuleChange),
    /// Eine Einstellung wurde geändert.
    ConfigChanged(ConfigChanged),
    /// Ein Pseudonym ist entstanden.
    PseudonymCreated(PseudonymCreated),
    /// Ein Fund steht auf der Ausnahmeliste.
    FindingAllowlisted(FindingAllowlisted),
    /// Ein Anker; schreibt nur der Schreiber selbst.
    AuditAnchor(AnchorData),
    /// Die Kette wurde geprüft.
    AuditVerified(AuditVerified),
    /// Eine Suche nach Sprachmodellen im eigenen Netz.
    LlmDiscover(LlmDiscover),
    /// Die Kette läuft hinter einem Anker weiter, den das Log nicht mehr
    /// erreichte.
    AuditResumed(AuditResumed),
}

impl RecordKind {
    /// Der Name, wie er in `kind` steht.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::DaemonStarted(_) => "daemon.started",
            Self::DaemonStopped(_) => "daemon.stopped",
            Self::SessionStarted(_) => "session.started",
            Self::SessionEnded(_) => "session.ended",
            Self::IsolationCheck(_) => "isolation.check",
            Self::FlowReceived(_) => "flow.received",
            Self::FlowDecided(_) => "flow.decided",
            Self::FlowForwarded(_) => "flow.forwarded",
            Self::FlowResponded(_) => "flow.responded",
            Self::FlowBlockedReason(_) => "flow.blocked_reason",
            Self::RuleAdded(_) => "rule.added",
            Self::RuleUpdated(_) => "rule.updated",
            Self::RuleRemoved(_) => "rule.removed",
            Self::ConfigChanged(_) => "config.changed",
            Self::PseudonymCreated(_) => "pseudonym.created",
            Self::FindingAllowlisted(_) => "finding.allowlisted",
            Self::AuditAnchor(_) => "audit.anchor",
            Self::AuditVerified(_) => "audit.verified",
            Self::LlmDiscover(_) => "llm.discover",
            Self::AuditResumed(_) => "audit.resumed",
        }
    }

    /// Alle Namen, in der Reihenfolge der Tabelle.
    pub const NAMES: [&'static str; 20] = [
        "daemon.started",
        "daemon.stopped",
        "session.started",
        "session.ended",
        "isolation.check",
        "flow.received",
        "flow.decided",
        "flow.forwarded",
        "flow.responded",
        "flow.blocked_reason",
        "rule.added",
        "rule.updated",
        "rule.removed",
        "config.changed",
        "pseudonym.created",
        "finding.allowlisted",
        "audit.anchor",
        "audit.verified",
        "llm.discover",
        "audit.resumed",
    ];

    /// `data` dieser Art.
    #[must_use]
    pub fn data(&self) -> Value {
        match self {
            Self::DaemonStarted(d) => json!({
                "version": d.version,
                "proto_version": d.proto_version,
                "key_origin": d.key_origin.as_str(),
            }),
            Self::DaemonStopped(d) => json!({ "reason": d.reason }),
            Self::SessionStarted(d) => json!({
                "profile": d.profile,
                "agent": d.agent,
                "work_dir_hash": d.work_dir_hash,
                "work_mode": d.work_mode,
                "llm_endpoint_host": d.llm_endpoint_host,
                "sandbox_backend": d.sandbox_backend,
                "argv_hash": d.argv_hash,
            }),
            Self::SessionEnded(d) => json!({
                "flows_total": d.flows_total,
                "held": d.held,
                "allowed": d.allowed,
                "allowed_edited": d.allowed_edited,
                "blocked": d.blocked,
                "timed_out": d.timed_out,
                "auto_rule": d.auto_rule,
                "passthrough": d.passthrough,
            }),
            Self::IsolationCheck(d) => json!({
                "results": d
                    .results
                    .iter()
                    .map(|result| json!({ "check": result.check, "passed": result.passed }))
                    .collect::<Vec<Value>>(),
            }),
            Self::FlowReceived(d) => json!({
                "flow": d.flow,
                "method": d.method,
                "scheme": d.scheme,
                "host": d.host,
                "port": d.port,
                "path_hash": d.path_hash,
                "size": d.size,
                "findings": d.findings,
                "findings_kinds": d.findings_kinds,
            }),
            Self::FlowDecided(d) => json!({
                "flow": d.flow,
                "decision": d.decision.as_str(),
                "rule": d.rule,
                "edited": d.edited,
                "replacements": d.replacements,
                "unresolved_findings": d.unresolved_findings,
                "acknowledged": d.acknowledged,
                "allowlisted_added": d.allowlisted_added,
                "decided_by": d.decided_by,
            }),
            Self::FlowForwarded(d) => json!({ "flow": d.flow, "upstream_ip": d.upstream_ip }),
            Self::FlowResponded(d) => json!({
                "flow": d.flow,
                "status": d.status,
                "size": d.size,
                "duration_ms": d.duration_ms,
                "streamed": d.streamed,
            }),
            Self::FlowBlockedReason(d) => json!({ "flow": d.flow, "reason": d.reason }),
            Self::RuleAdded(d) | Self::RuleUpdated(d) | Self::RuleRemoved(d) => json!({
                "rule": d.rule,
                "action": d.action,
                "match_host": d.match_host,
                "match_method": d.match_method,
                "expires": d.expires,
                "created_from": d.created_from,
                "origin": d.origin.as_str(),
            }),
            Self::ConfigChanged(d) => {
                // Ein geheimer Wert hat nicht einmal einen leeren Platz im
                // Log: Der Schlüssel `value` fehlt ganz.
                if d.secret {
                    json!({ "key": d.key, "origin": d.origin, "secret": true })
                } else {
                    json!({ "key": d.key, "origin": d.origin, "secret": false, "value": d.value })
                }
            }
            Self::PseudonymCreated(d) => json!({ "pseudonym": d.pseudonym, "kind": d.kind }),
            Self::FindingAllowlisted(d) => json!({ "kind": d.kind, "scope": d.scope }),
            Self::AuditAnchor(d) => json!({
                "anchored_seq": d.anchored_seq,
                "anchored_hash": d.anchored_hash,
            }),
            Self::AuditVerified(d) => json!({
                "result": d.result,
                "first_bad_seq": d.first_bad_seq,
                "records": d.records,
            }),
            Self::LlmDiscover(d) => json!({
                "subnet": d.subnet,
                "ports": d.ports,
                "found": d.found,
            }),
            Self::AuditResumed(d) => json!({ "log_seq": d.log_seq, "anchor_seq": d.anchor_seq }),
        }
    }
}

/// Ein Zeitpunkt als Text für `data`, über denselben Formatter wie `ts`.
#[must_use]
pub fn data_ts(at: DateTime<Utc>) -> String {
    format_ts(at)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{
        Authority, BlockReason, BodyRef, Decision, DecisionSource, FlowId, HostName, HttpRequest,
        Method, RuleId, Scheme,
    };

    use super::{DecisionKind, FlowBlockedReason, FlowDecided, FlowReceived, RecordKind};
    use crate::canonical::canonical_json;

    fn request(path: &str) -> HttpRequest {
        let mut request = HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
            path.to_owned(),
        );
        request.body = BodyRef::from_bytes(bytes::Bytes::from_static(b"{\"mail\":\"a@b.de\"}"));
        request
    }

    #[test]
    fn every_kind_has_the_name_of_the_table_and_canonical_data() {
        assert_eq!(RecordKind::NAMES.len(), 20);
        for name in RecordKind::NAMES {
            let (area, verb) = name.split_once('.').unwrap();
            assert!(!area.is_empty() && !verb.is_empty(), "{name}");
        }
        let received = RecordKind::FlowReceived(FlowReceived::new(
            FlowId::new(),
            &request("/x?token=geheim"),
            &[],
        ));
        assert!(RecordKind::NAMES.contains(&received.name()));
        assert!(canonical_json(&received.data()).is_ok());
    }

    #[test]
    fn a_received_flow_carries_no_path_and_no_body() {
        let flow = FlowId::new();
        let record = FlowReceived::new(flow, &request("/v1/x?token=geheim-42"), &[]);
        let data = String::from_utf8(
            canonical_json(&RecordKind::FlowReceived(record.clone()).data()).unwrap(),
        )
        .unwrap();
        assert!(!data.contains("geheim-42"), "{data}");
        assert!(!data.contains("/v1/x"), "{data}");
        assert!(!data.contains("a@b.de"), "{data}");
        assert_eq!(record.path_hash.len(), 64);
        assert_eq!(record.size, 17);
        assert_eq!(record.host, "api.example.com");
        assert_eq!(record.port, 443);
    }

    #[test]
    fn decisions_map_to_the_words_of_the_table() {
        let rule = RuleId::new();
        let block = Decision::Block {
            reason: BlockReason::User,
            note: Some("bitte nicht".to_owned()),
        };
        let cases = [
            (Decision::Allow, DecisionSource::User, "allow", "user"),
            (
                Decision::Allow,
                DecisionSource::Rule(rule),
                "auto_allow",
                "rule",
            ),
            (block.clone(), DecisionSource::User, "block", "user"),
            (
                block.clone(),
                DecisionSource::Rule(rule),
                "auto_block",
                "rule",
            ),
            (
                Decision::TimedOut,
                DecisionSource::Timeout,
                "timed_out",
                "timeout",
            ),
            (
                Decision::Allow,
                DecisionSource::Passthrough,
                "passthrough",
                "rule",
            ),
        ];
        for (decision, source, word, by) in cases {
            let decided = FlowDecided::new(FlowId::new(), &decision, source);
            assert_eq!(decided.decision.as_str(), word);
            assert_eq!(decided.decided_by, by);
            assert_eq!(DecisionKind::of(&decision, source).as_str(), word);
        }
        let with_note = RecordKind::FlowDecided(FlowDecided::new(
            FlowId::new(),
            &block,
            DecisionSource::User,
        ));
        let text = String::from_utf8(canonical_json(&with_note.data()).unwrap()).unwrap();
        assert!(!text.contains("bitte nicht"), "the note stays out: {text}");
    }

    #[test]
    fn only_a_block_of_the_daemon_has_a_reason_record() {
        let flow = FlowId::new();
        let cap = Decision::Block {
            reason: BlockReason::BodyCap,
            note: None,
        };
        let reason = FlowBlockedReason::of(flow, &cap, DecisionSource::System).unwrap();
        assert_eq!(reason.reason, "body_cap");
        let user = Decision::Block {
            reason: BlockReason::User,
            note: None,
        };
        assert!(FlowBlockedReason::of(flow, &user, DecisionSource::User).is_none());
    }
}
