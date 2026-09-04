//! Die Konfigurations-Typen. Eine Quelle für TOML, Kommandozeile, Schema und
//! Oberfläche.
//!
//! Jedes Blattfeld trägt vier Dinge: einen Doku-Kommentar (wird von `schemars`
//! zur `description`), eine Sichtbarkeitsstufe (`x-tier`, siehe [`crate::tier`]),
//! eine Vertrauensgrenze (`x-project-scope`, siehe [`crate::scope`]) und einen
//! Vorgabewert aus `impl Default`. Die Tests `every_leaf_has_tier_and_description`
//! und `every_node_has_a_project_scope` halten das fest, damit der
//! Einstellungs-Bildschirm (HUM-069) und `docs/CONFIG.md` (HUM-070) vollständig
//! bleiben, ohne dass jemand sie von Hand pflegt.
//!
//! `x-project-scope = "denied"` steht an jedem Schlüssel, den das Projekt-Profil
//! nicht setzen darf (`backlog/CONVENTIONS.md` 4.11): `llm.*`, `sandbox.*`,
//! `agent.adapter`, `agent.command`, `hold.ask_mode`, `findings.enabled`,
//! `findings.ignored_hashes`, `findings.email_allow_domains`, `pseudonyms.*`,
//! `resolver.*`, `experimental.*`, `recorder.retention_days`. Eine Gruppe ist
//! `denied`, wenn jedes Blatt darunter es ist.
//!
//! Gruppen sind flach und nach Zuständigkeit geschnitten. Caps und Zeitgrenzen
//! wohnen ausnahmslos in [`Limits`] (`backlog/CONVENTIONS.md` 4.4); die alten
//! Schlüssel aus 3.7 leben als Alias weiter, siehe [`crate::alias`].

use std::collections::BTreeMap;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 1 KiB in Bytes, für die Vorgabewerte unten.
const KIB: u64 = 1024;
/// 1 MiB in Bytes, für die Vorgabewerte unten.
const MIB: u64 = 1024 * KIB;

/// Die vollständige Konfiguration eines Humanitl-Laufs.
///
/// `deny_unknown_fields` zusammen mit `default` ist Absicht: fehlende Gruppen
/// sind erlaubt, Tippfehler nicht.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Der lokale LLM-Endpunkt und was als Passthrough gilt.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "denied"))]
    pub llm: LlmConfig,
    /// Wie lange und auf welchem Weg gefragt wird, bevor eine Anfrage weiterläuft.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub hold: HoldConfig,
    /// Alle Caps und Zeitgrenzen an einer Stelle.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub limits: Limits,
    /// Namensauflösung nach der Entscheidung.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub resolver: ResolverConfig,
    /// Erkennung von Geheimnissen und persönlichen Daten.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub findings: FindingsConfig,
    /// Rücktausch von Pseudonymen in Antworten.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub pseudonyms: PseudonymConfig,
    /// Welches Sandbox-Profil mit welchem Arbeitsverzeichnis startet.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "denied"))]
    pub sandbox: SandboxRef,
    /// Welcher Agent in der Sandbox läuft.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub agent: AgentRef,
    /// Aufzeichnung der Flows.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub recorder: RecorderConfig,
    /// Sprache, Erscheinungsbild und Meldungen der Oberfläche.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub ui: UiConfig,
    /// Schalter für unfertige Wege. Alles hier darf ohne Ankündigung wegfallen.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub experimental: Experimental,
}

/// Der OpenAI-kompatible Endpunkt im eigenen Netz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct LlmConfig {
    /// OpenAI-kompatibler Endpunkt im LAN. Verkehr dorthin wird nicht angehalten, aber protokolliert.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "denied"), with = "Option<String>")]
    pub endpoint: Option<url::Url>,
    /// Pfadpräfixe, die als LLM-Passthrough gelten. Ein Präfix soll einen Endpunkt benennen, keine ganze API-Fläche: Der Agent-Adapter ersetzt `/v1/` und `/api/` deshalb durch die Endpunkte, die Inferenz machen, damit `POST /api/pull` und `POST /v1/files` nicht ungefragt hinausgehen. Ein Pfad, der mehr nennt, bleibt stehen, wie er hier steht.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub passthrough_paths: Vec<String>,
    /// Modelle, die der Endpunkt anbietet. Leer heißt: der Agent bekommt ein Platzhalter-Modell und eine Warnung.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "denied"))]
    pub models: Vec<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            passthrough_paths: vec!["/v1/".to_owned(), "/api/".to_owned()],
            models: Vec::new(),
        }
    }
}

/// Wie eine Anfrage angehalten und beantwortet wird.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HoldConfig {
    /// Sekunden, die eine angehaltene Anfrage auf eine Entscheidung wartet, bevor sie als Zeitüberschreitung endet.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub timeout_secs: u64,
    /// Wo gefragt wird: in der Oberfläche, im Terminal oder gar nicht.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub ask_mode: AskMode,
    /// Blockt Anfragen mit prüfsummen-sicheren Geheimnissen sofort, ohne zu fragen.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub hard_block_checksum_secrets: bool,
}

impl Default for HoldConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 300,
            ask_mode: AskMode::Ui,
            hard_block_checksum_secrets: false,
        }
    }
}

/// Wo gefragt wird, wenn eine Anfrage angehalten ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AskMode {
    /// Die Oberfläche zeigt die Karte.
    Ui,
    /// Die Frage erscheint im Terminal des Agenten.
    Terminal,
    /// Es wird nicht gefragt; jede Anfrage ohne Regel läuft in die Zeitüberschreitung.
    None,
}

/// Alle Caps und Zeitgrenzen (`backlog/CONVENTIONS.md` 4.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    /// Größte Anfrage, deren Body für die Entscheidung im Speicher gehalten wird. Darüber antwortet der Proxy mit 413.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub hold_body_cap_bytes: u64,
    /// Größte Menge Body, die die Oberfläche als Vorschau bekommt.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub preview_cap_bytes: u64,
    /// Länge der Ereignis-Warteschlange je Client. Läuft sie über, meldet der Daemon `Lagged`.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub event_buffer: usize,
    /// Höchstes erlaubtes Verhältnis von entpackten zu gepackten Bytes einer Vorschau.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub max_decompress_ratio: u32,
    /// Größte Zahl gleichzeitig angehaltener Flows. Darüber antwortet der Proxy mit 503.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub hold_max_flows: u32,
    /// Größte Summe der Bodies aller angehaltenen Flows. Darüber antwortet der Proxy mit 503.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub hold_max_bytes: u64,
    /// Sekunden bis zum Aufbau der Verbindung zum Ziel.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub connect_timeout_secs: u64,
    /// Sekunden, in denen der Client seine Anfrage-Kopfzeilen gesendet haben muss.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub header_timeout_secs: u64,
    /// Sekunden, in denen ein Body vollständig übertragen sein muss.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub body_timeout_secs: u64,
    /// Sekunden ohne Bytes, nach denen eine offene Verbindung geschlossen wird.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub idle_timeout_secs: u64,
    /// Größter Body, den die Aufzeichnung als Blob ablegt. Alles darüber wird nur mit Prüfsumme vermerkt.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub recorder_max_body_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            hold_body_cap_bytes: 32 * MIB,
            preview_cap_bytes: 8 * MIB,
            event_buffer: 1024,
            max_decompress_ratio: 100,
            hold_max_flows: 200,
            hold_max_bytes: 256 * MIB,
            connect_timeout_secs: 10,
            header_timeout_secs: 30,
            body_timeout_secs: 300,
            idle_timeout_secs: 90,
            recorder_max_body_bytes: 32 * MIB,
        }
    }
}

/// Namensauflösung. Aufgelöst wird erst nach der Entscheidung, und nur hier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ResolverConfig {
    /// Nameserver als `IP:Port`. Leer bedeutet: die Einstellung des Systems.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub nameserver: Option<String>,
    /// Feste Zuordnungen von Hostname zu Adresse, vor jeder Abfrage.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub overrides: BTreeMap<String, String>,
    /// Sekunden, die eine Antwort im Zwischenspeicher bleibt.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub cache_ttl_secs: u64,
    /// Welche Adressfamilie bevorzugt wird, wenn beide vorliegen.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub prefer: IpPreference,
    /// Zusätzliche CA für Tests. Nur in Testläufen setzen, nie im Alltag.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub test_ca: Option<PathBuf>,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            nameserver: None,
            overrides: BTreeMap::new(),
            cache_ttl_secs: 300,
            prefer: IpPreference::Ipv4,
            test_ca: None,
        }
    }
}

/// Welche Adressfamilie zuerst versucht wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IpPreference {
    /// IPv4 zuerst.
    Ipv4,
    /// IPv6 zuerst.
    Ipv6,
}

/// Erkennung von Geheimnissen und persönlichen Daten in Anfragen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct FindingsConfig {
    /// Schaltet die Erkennung ganz ab. Aus bedeutet: keine Markierungen, keine Pseudonyme.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub enabled: bool,
    /// Eigene Begriffe, die als Fund gelten, zum Beispiel ein Projektname oder ein Kundenname.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub user_terms: Vec<String>,
    /// Domains, deren Mailadressen kein Fund sind, zum Beispiel die eigene Firma.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub email_allow_domains: Vec<String>,
    /// Prüfsummen (SHA-256, hex) einzelner Werte, die nie wieder als Fund erscheinen.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub ignored_hashes: Vec<String>,
}

impl Default for FindingsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            user_terms: Vec::new(),
            email_allow_domains: Vec::new(),
            ignored_hashes: Vec::new(),
        }
    }
}

/// Rücktausch von Pseudonymen in den Antworten des LLM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PseudonymConfig {
    /// Ersetzt Pseudonyme in Text-Antworten wieder durch den Originalwert.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub translate_responses: bool,
    /// Größte Antwort, die für den Rücktausch gepuffert wird. Alles darüber läuft unverändert durch.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub max_response_bytes: u64,
}

impl Default for PseudonymConfig {
    fn default() -> Self {
        Self {
            translate_responses: true,
            max_response_bytes: 8 * MIB,
        }
    }
}

/// Verweis auf das Sandbox-Profil und das Arbeitsverzeichnis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxRef {
    /// Name des Profils unter `profiles/sandbox/`, ohne Endung.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub profile: String,
    /// Projektverzeichnis, das als `/work` eingehängt wird. Leer bedeutet: das aktuelle Verzeichnis.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "denied"))]
    pub work_dir: Option<PathBuf>,
    /// Ob der Agent im Projektverzeichnis schreiben darf.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "denied"))]
    pub work_mode: WorkMode,
    /// Zusätzliche Umgebungsvariablen für die Sandbox; sie überschreiben gleichnamige Einträge aus dem `[env]` des Profils. Der Schlüssel lässt sich aus der Umgebung des Prozesses setzen und ist damit nur so vertrauenswürdig wie die Shell, aus der Humanitl startet; die Variablen des dynamischen Linkers (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`) werden deshalb abgelehnt, sie liefen vor dem seccomp-Filter des Shims.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub env: BTreeMap<String, String>,
}

impl Default for SandboxRef {
    fn default() -> Self {
        Self {
            profile: "default".to_owned(),
            work_dir: None,
            work_mode: WorkMode::Rw,
            env: BTreeMap::new(),
        }
    }
}

/// Ob das Projektverzeichnis schreibbar eingehängt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkMode {
    /// Nur lesen.
    Ro,
    /// Lesen und schreiben.
    Rw,
}

/// Welcher Agent startet und wie er begrüßt wird.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AgentRef {
    /// Kennung des Adapters, zum Beispiel `opencode`.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub adapter: String,
    /// Ersetzt die Kommandozeile des Adapters vollständig. Leer bedeutet: die des Adapters.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub command: Option<Vec<String>>,
    /// Die Instruktionsdatei, die der Agent in der Sandbox vorfindet.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub briefing: AgentBriefing,
}

impl Default for AgentRef {
    fn default() -> Self {
        Self {
            adapter: "opencode".to_owned(),
            command: None,
            briefing: AgentBriefing::default(),
        }
    }
}

/// Die kurze Einweisung, die der Agent in der Sandbox vorfindet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AgentBriefing {
    /// Legt die Instruktionsdatei des Agenten in der Sandbox an.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub enabled: bool,
}

impl Default for AgentBriefing {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Aufzeichnung der Flows in Datenbank und Blob-Speicher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RecorderConfig {
    /// Bodies bis zu dieser Größe stehen in der Datenbank, größere als Datei im Blob-Speicher.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub inline_max_bytes: u64,
    /// Tage, die eine Aufzeichnung aufgehoben wird.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub retention_days: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            inline_max_bytes: 256 * KIB,
            retention_days: 90,
        }
    }
}

/// Sprache, Erscheinungsbild und Meldungen der Oberfläche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    /// Sprache der Oberfläche.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub language: Language,
    /// Erscheinungsbild der Oberfläche.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub theme: Theme,
    /// Meldung des Systems, wenn eine Anfrage wartet und das Fenster nicht vorn ist.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub notifications: bool,
    /// Ton zur Meldung. Im MVP ohne Wirkung: der Schlüssel wird gelesen, aber
    /// kein Ton gespielt (HUM-034).
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub sound: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: Language::En,
            theme: Theme::Dark,
            notifications: true,
            sound: false,
        }
    }
}

/// Sprache der Oberfläche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    /// Englisch.
    En,
    /// Deutsch.
    De,
}

/// Erscheinungsbild der Oberfläche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Dunkel.
    Dark,
    /// Hell.
    Light,
    /// Der Einstellung des Systems folgen.
    System,
}

/// Schalter für unfertige Wege.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Experimental {
    /// Bietet dem Ziel HTTP/2 an. In M1 spricht der Proxy nach oben nur HTTP/1.1.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub h2_upstream: bool,
    /// Hält auch WebSocket-Upgrades an, statt sie über eine Regel zu entscheiden.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub ws_hold: bool,
    /// Lenkt einen Zielport auf einen anderen um, Schlüssel und Wert als Portnummer. Nur für Tests.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub upstream_port_map: BTreeMap<String, u16>,
}
