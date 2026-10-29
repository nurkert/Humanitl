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
//! Ein Feld ohne Leser trägt ein fünftes: `x-pending-issue` mit der Kennung des
//! Issues, das über den Schlüssel entscheidet (siehe [`crate::pending`], HUM-101).
//! Fehlt die Angabe, behauptet das Feld, es wirke; das Register in
//! `daemon/crates/config/tests/config_readers.rs` nagelt beide Fälle fest.
//!
//! `x-project-scope = "denied"` steht an jedem Schlüssel, den das Projekt-Profil
//! nicht setzen darf (`backlog/CONVENTIONS.md` 4.11): `llm.*`, `sandbox.*`,
//! `agent.adapter`, `agent.command`, `hold.ask_mode`,
//! `hold.hard_block_checksum_secrets`, `findings.enabled`,
//! `findings.ignored_hashes`, `findings.email_allow_domains`, `pseudonyms.*`,
//! `limits.recorder_max_body_bytes`, `resolver.*`, `tls.*`, `experimental.*`,
//! `recorder.retention_days`, `audit.*`, `terminal.*`. Eine Gruppe ist `denied`,
//! wenn jedes Blatt darunter es ist.
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
    /// Wem der Daemon auf dem Weg zum Ziel vertraut.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub tls: TlsConfig,
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
    /// Die Audit-Kette: Anker, Synchronisation, Aufbewahrung.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub audit: AuditConfig,
    /// Sprache, Erscheinungsbild und Meldungen der Oberfläche.
    #[schemars(extend("x-tier" = "basic", "x-project-scope" = "allowed"))]
    pub ui: UiConfig,
    /// Das Terminal der Kommandozeile, wenn sie an einer Sitzung hängt.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub terminal: TerminalConfig,
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
    /// Verweis auf den Zugangsschlüssel des Endpunkts, nie der Schlüssel selbst: `secret-service:humanitl/llm` für den Schlüsselbund des Systems oder `file:llm` für eine Datei mit `0600` unter `$XDG_DATA_HOME/humanitl/keys`. Gesetzt wird er mit `humanitl llm key set` oder in den Einstellungen; der Agent bekommt den Schlüssel als Datei in der Sandbox, nie über die Kommandozeile. Leer heißt: kein Schlüssel.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub api_key_ref: Option<String>,
    /// Gespeicherte Modellserver, je Name ein Eintrag mit `endpoint`, `models` und `api_key_ref` (`secret-service:humanitl/NAME`). Der Eintrag, den `llm.active` nennt, liefert `llm.endpoint`, `llm.models` und `llm.api_key_ref`; die flachen Schlüssel sind seine Ansicht. Ein Name hat 1 bis 32 Zeichen `a-z`, `0-9`, `-` und `_`. Gepflegt wird die Tabelle mit `humanitl llm server` oder in den Einstellungen; ein Wechsel gilt ab der nächsten Sitzung.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub servers: BTreeMap<String, LlmServer>,
    /// Der Name des aktiven Eintrags aus `llm.servers`. Nur sein Endpunkt bekommt den hinterlegten Zugangsschlüssel und gilt als LLM-Passthrough. Leer, solange es keine Einträge gibt.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub active: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            passthrough_paths: vec!["/v1/".to_owned(), "/api/".to_owned()],
            models: Vec::new(),
            api_key_ref: None,
            servers: BTreeMap::new(),
            active: None,
        }
    }
}

/// Ein gespeicherter Modellserver unter `llm.servers.<name>` (HUM-289).
///
/// Dieselben drei Werte wie die flachen Schlüssel unter `llm`, deren Ansicht
/// der aktive Eintrag ist. Der Zugangsschlüssel steht auch hier nie selbst in
/// der Datei, nur der Verweis darauf.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct LlmServer {
    /// OpenAI-kompatibler Endpunkt dieses Servers, wie `llm.endpoint`.
    ///
    /// Als Text und nicht als Adresse: Ein Eintrag, der nicht aktiv ist, darf
    /// eine Adresse tragen, die kein Parser annimmt, ohne dass das Laden
    /// scheitert; er ist dann nur nicht zu aktivieren (HUM-289, Review).
    pub endpoint: Option<String>,
    /// Modelle dieses Servers, das voreingestellte zuerst, wie `llm.models`.
    pub models: Vec<String>,
    /// Verweis auf den Zugangsschlüssel dieses Servers, wie `llm.api_key_ref`.
    pub api_key_ref: Option<String>,
}

impl LlmConfig {
    /// Ob der hinterlegte Schlüssel (`llm.api_key_ref`) zusammen mit
    /// `llm.endpoint` hinausgehen darf (HUM-289).
    ///
    /// Die eine Regel, die beim Laden ([`crate::servers`]) schon gilt und an
    /// jeder Stelle, die den Schlüssel benutzt, noch einmal geprüft wird:
    /// Ein Verweis wirkt nur zusammen mit dem Endpunkt **desselben**
    /// Eintrags. Ohne gespeicherte Server sind das die flachen Schlüssel;
    /// mit ihnen muss der Eintrag, den `llm.active` nennt, genau diesen
    /// Verweis tragen, kein anderer Eintrag denselben, und sein Endpunkt
    /// dieselbe Herkunft (Schema, Host, Port) haben wie `llm.endpoint`.
    #[must_use]
    pub fn key_goes_with_endpoint(&self) -> bool {
        if self.servers.is_empty() {
            return true;
        }
        let Some(reference) = self.api_key_ref.as_deref() else {
            return true;
        };
        let Some(entry) = self.active.as_ref().and_then(|name| self.servers.get(name)) else {
            return false;
        };
        if entry.api_key_ref.as_deref() != Some(reference) {
            return false;
        }
        let shared = self
            .servers
            .values()
            .filter(|other| other.api_key_ref.as_deref() == Some(reference))
            .count();
        if shared != 1 {
            return false;
        }
        let entry_origin = entry
            .endpoint
            .as_deref()
            .and_then(|text| url::Url::parse(text).ok())
            .map(|url| url.origin());
        match (&self.endpoint, entry_origin) {
            (Some(endpoint), Some(origin)) => endpoint.origin() == origin,
            _ => false,
        }
    }

    /// Host und Port des Endpunkts, normalisiert wie der Proxy jede Anfrage
    /// normalisiert ([`humanitl_core::HostName::parse`]), mit dem
    /// Standardport des Schemas.
    ///
    /// Die eine Quelle für das Ziel des Modells: die Statuszeile des
    /// Meta-Endpunkts und das Ziel des Zugangsschlüssels, an dem er in
    /// `Authorization` kein Fund ist (HUM-140). Ein Endpunkt mit Punkt am Ende
    /// (`model.lan.`) oder als IPv6-Literal ergibt damit dieselbe Form wie die
    /// Anfrage, die der Agent an ihn stellt; ein eigener Weg über `url` hätte
    /// den Schlüssel dort zu einem Fund gemacht.
    ///
    /// `None` ohne Endpunkt, ohne Host oder bei einem Host, den der Proxy
    /// nicht annähme.
    #[must_use]
    pub fn endpoint_authority(&self) -> Option<humanitl_core::Authority> {
        let endpoint = self.endpoint.as_ref()?;
        let host = humanitl_core::HostName::parse(endpoint.host_str()?).ok()?;
        let port = endpoint.port_or_known_default()?;
        Some(humanitl_core::Authority::new(host, port))
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
    /// Lässt Anfragen mit prüfsummen-sicheren Geheimnissen nur bearbeitet hinaus; keine Regel und keine Freigabe sendet sie unverändert.
    // Gesperrt für das Projekt-Profil (HUM-208): Ein geklontes Repository soll
    // die harte Sperre des Nutzers nicht still aufheben können.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
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
    /// Größte Anfrage, deren Body für die Entscheidung im Speicher gehalten wird. Darüber antwortet der Proxy mit 413, außer eine Regel `allow` oder `ask` mit `stream: true` trifft die Anfrage und `hold.hard_block_checksum_secrets` ist aus: Dann geht der Body gestreamt und ungeprüft hinaus.
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
    /// Sekunden, in denen der Client seine Anfrage-Kopfzeilen gesendet haben muss. Auf einer Keep-Alive-Verbindung ist das zugleich die Frist bis zur nächsten Anfrage, also die einzige Leerlaufgrenze der Verbindung zum Agenten; während eine Anfrage gehalten wird, läuft sie nicht.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub header_timeout_secs: u64,
    /// Sekunden Stille zwischen zwei Stücken eines Bodys, nicht seine Gesamtdauer. Gilt in beide Richtungen: für den Anfrage-Rumpf des Clients (danach 408) und für den gestreamten Antwort-Rumpf des Ziels (danach wird der Strom abgebrochen und die Aufzeichnung als gekürzt vermerkt). Ein großer Upload und ein langer Modell-Strom dürfen deshalb beliebig lange dauern, solange sie nicht verstummen.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub body_timeout_secs: u64,
    /// Größter Body, den die Aufzeichnung als Blob ablegt. Alles darüber wird nur mit Prüfsumme vermerkt.
    // Gesperrt für das Projekt-Profil (HUM-209): Der Wert bestimmt, wie viel
    // die Aufzeichnung belegt und was der Mensch beim Halten vom Body sieht.
    // Ein geklontes Repository könnte ihn sonst auf 1 KiB senken, während
    // `Allow` den ganzen Body schickt.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub recorder_max_body_bytes: u64,
    /// Größte Zahl gleichzeitiger Verbindungen aus der Sandbox je Sitzung. Darüber antwortet der Proxy mit 503 und schließt; eine Uhr je Spanne allein hindert einen Prozess nicht daran, dieselben Ressourcen über viele Verbindungen zu binden.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "allowed"))]
    pub max_client_connections: u32,
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
            recorder_max_body_bytes: 32 * MIB,
            max_client_connections: 256,
        }
    }
}

/// Namensauflösung. Aufgelöst wird erst nach der Entscheidung, und nur hier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ResolverConfig {
    /// Nameserver als `IP:Port`, ohne Port gilt 53. Gesetzt fragt der Daemon nach einer Freigabe nur noch diesen Server, über UDP mit TCP-Rückfall, und weder `/etc/resolv.conf` noch `/etc/hosts`. Leer bedeutet: der Namensdienst des Systems.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub nameserver: Option<String>,
    /// Feste Zuordnungen von Hostname zu Adresse, vor jeder Abfrage. Nicht leer meldet der Start als `CONFIG_016`, damit der Testhebel nicht unbemerkt in einem Alltagslauf steht.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub overrides: BTreeMap<String, String>,
    /// Sekunden, die eine Antwort im Zwischenspeicher bleibt.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub cache_ttl_secs: u64,
    /// Welche Adressfamilie bevorzugt wird, wenn beide vorliegen.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub prefer: IpPreference,
    /// Zusätzliche CA für Tests, als PEM und mit absolutem Pfad. Gilt nur, wenn
    /// der Daemon mit `--allow-test-ca` gestartet wurde; ohne das Flag wird der
    /// Schlüssel nicht angenommen, und ein nicht absoluter Pfad beendet den
    /// Start, statt gegen das Arbeitsverzeichnis aufgelöst zu werden. Nur in
    /// Testläufen setzen, nie im Alltag.
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

/// Vertrauen auf dem Weg zum Ziel (HUM-257).
///
/// Betrifft nur die Verbindungen des Daemons zum Ziel und zum LLM-Endpunkt.
/// Die CA, die der Proxy für die Sandbox ausstellt, ist davon unberührt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TlsConfig {
    /// Vertraut beim Ziel den Wurzeln aus der Zertifikatsablage des Systems (`/etc/ssl/certs`), wie `curl` und der Browser: Eine Firmen-CA oder eine mit `update-ca-certificates` installierte CA wird angenommen. Aus gelten nur die öffentlichen Wurzeln aus `webpki-roots`. Wirkt nach dem Neustart des Daemons; das Projekt-Profil kann den Schlüssel nicht setzen.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub trust_system_store: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            trust_system_store: true,
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
    #[schemars(extend(
        "x-tier" = "advanced",
        "x-project-scope" = "denied",
        "x-pending-issue" = "HUM-079"
    ))]
    pub translate_responses: bool,
    /// Größte Antwort, die für den Rücktausch gepuffert wird. Alles darüber läuft unverändert durch.
    #[schemars(extend(
        "x-tier" = "expert",
        "x-project-scope" = "denied",
        "x-pending-issue" = "HUM-079"
    ))]
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
    /// Einstellungen, die nur der Adapter `pi` liest (HUM-309).
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub pi: PiAgentConfig,
}

impl Default for AgentRef {
    fn default() -> Self {
        Self {
            adapter: "opencode".to_owned(),
            command: None,
            briefing: AgentBriefing::default(),
            pi: PiAgentConfig::default(),
        }
    }
}

/// Was der Adapter `pi` zusätzlich in die Sandbox bringt (HUM-309).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PiAgentConfig {
    /// Lädt die pi-Erweiterung `pi-web-access` (Websuche und Abruf von Seiten), wenn sie global installiert ist (`sudo npm install -g --ignore-scripts pi-web-access`). `auto` lädt sie, wenn die Sandbox sie findet, `on` ebenso und meldet `AGENT_007`, wenn sie fehlt, `off` lädt sie nie. Jede Anfrage der Erweiterung geht wie jede andere durch den Proxy. Im Projekt-Profil gesperrt: fremden Code in den Agenten zu laden, entscheidet der Nutzer.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub web_access: PiWebAccess,
}

/// Ob der Adapter `pi` die Erweiterung `pi-web-access` lädt (HUM-309).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PiWebAccess {
    /// Laden, wenn die Sandbox das Paket findet; sonst still ohne.
    #[default]
    Auto,
    /// Laden; fehlt das Paket, sagt es ein Befund (`AGENT_007`).
    On,
    /// Nie laden.
    Off,
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
    /// Tage, die eine Aufzeichnung (Anfragen, Antworten, Bodies) aufgehoben wird; 0 heißt nie löschen. Ein täglicher Lauf löscht, was älter ist, und vermerkt jede Löschung im Audit-Log (`recorder.retention_applied`). Die Frist setzt die Speicherbegrenzung aus DSGVO Art. 5 Abs. 1 lit. e um: Personenbezogenes in Anfragen bleibt nicht länger als nötig liegen. Die Audit-Kette löscht dieser Lauf nie.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub retention_days: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            inline_max_bytes: 256 * KIB,
            // 180 Tage, festgelegt in HUM-051. Bis dahin standen hier 90.
            retention_days: 180,
        }
    }
}

/// Die Audit-Kette in `audit.jsonl` (HUM-050).
///
/// Die ganze Gruppe ist für das Projekt-Profil gesperrt: Ein geklontes
/// Repository, das `anchor_every` hochsetzt, vergrößerte das Fenster, in dem
/// ein gekürztes Log unbemerkt bleibt, und eines, das `fsync_every` hochsetzt,
/// das, in dem ein Absturz Records kostet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AuditConfig {
    /// Jeder wievielte Record ein Anker ist, in der Datei und zugleich in der Datenbank. Records hinter dem letzten Anker kann jemand mit Zugriff auf die Datei am Ende abschneiden, ohne dass die Prüfung es merkt; ein kleinerer Wert macht dieses Fenster kleiner. Beim Beenden des Daemons wird immer geankert.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub anchor_every: u32,
    /// Tage, die ein Record im Audit-Log aufgehoben wird; 0 heißt für immer. Ein täglicher Lauf löscht die Records am Anfang der Kette, die älter sind, und dokumentiert den Schnitt mit einem Record `audit.pruned` samt Anker; die Prüfung erkennt den so dokumentierten Anfang an, einen Anfang ohne ihn meldet sie weiter als Bruch. Eine Kette, die nicht hält, wird nicht gekürzt. Was in den gelöschten Records stand, beweist die Kette danach nicht mehr; die Anker in `audit_anchors` bleiben stehen.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub retention_days: u32,
    /// Obergrenze für die Größe des Audit-Logs in Bytes; 0 heißt keine Grenze, sonst 2097152 (2 MiB) bis 1099511627776 (1 TiB). Derselbe Lauf wie bei `audit.retention_days`, beim Start und danach täglich, löscht ganze Records am Anfang der Kette, das Älteste zuerst, bis die Datei samt dem Record `audit.pruned` und seinem Anker unter der Grenze liegt; zwischen zwei Läufen kann sie darüber wachsen. Der neueste Record bleibt immer stehen; ist er länger als 1 MiB, kann das Log mit ihm über der Grenze bleiben, und der Daemon meldet das im Protokoll. Eine Kette, die nicht hält, wird nicht gekürzt. Der Lauf braucht kurz Platz für eine Kopie des Rests neben der Datei; fehlt er, bleibt das Log, wie es war, und der nächste Lauf versucht es erneut.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub max_bytes: u64,
    /// Nach wie vielen Records der Daemon das Audit-Log auf die Platte zwingt (fsync). Spätestens nach einer Sekunde und vor jedem Anker geschieht es ohnehin.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub fsync_every: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            anchor_every: 100,
            retention_days: 0,
            max_bytes: 0,
            fsync_every: 50,
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
    /// Hinweiszeile des Daemons, wenn eine Anfrage des Agenten auf eine Entscheidung wartet oder entschieden ist. Sie geht als eigener Rahmen an jeden Client, nie in den Bytestrom des Agenten: Wer am Terminal sitzt, bekommt sie zwischen die Bytes geschrieben, und die Oberfläche zeigt sie im Streifen über dem Terminal. Der Streifen bleibt in jedem Fall, weil er aus dem Ereignisstrom kommt; dieser Schalter betrifft nur den Rahmen.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "allowed"))]
    pub terminal_notices: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: Language::En,
            theme: Theme::Dark,
            notifications: true,
            terminal_notices: true,
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

/// Das Terminal der Kommandozeile, wenn sie an einer Sitzung hängt
/// (`humanitl sandbox attach`, HUM-279).
///
/// Die Oberfläche hat ihr eigenes Terminal und liest hier nichts: Dort hängt
/// sich ab, wer den Bereich schließt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalConfig {
    /// Präfix der Taste, mit der sich `humanitl sandbox attach` abhängt: `ctrl-a` bis `ctrl-z`, `ctrl-\`, `ctrl-]`, `ctrl-^`, `ctrl-_`, oder `none`, dann gibt es keine. Nach dem Präfix hängt `d` ab und lässt die Sitzung laufen; das Präfix zweimal oder sein Buchstabe schickt es einmal an den Agenten, jede andere Taste geht mit dem Präfix unverändert weiter. `ctrl-[` ist ausgenommen, weil jede Pfeiltaste mit diesem Byte beginnt, `ctrl-h`, `ctrl-i`, `ctrl-j` und `ctrl-m`, weil Rücktaste, Tab und Enter dieselben Bytes schicken. `--detach-keys` überschreibt den Wert für einen Aufruf.
    #[schemars(extend("x-tier" = "advanced", "x-project-scope" = "denied"))]
    pub detach_prefix: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            detach_prefix: DetachKeys::DEFAULT_SPEC.to_owned(),
        }
    }
}

/// Die Taste, mit der sich ein angehängtes Terminal abhängt: ein Präfix oder
/// keine.
///
/// Eine Lesart für `terminal.detach_prefix` und `--detach-keys`, damit die
/// Prüfung beim Laden und die der Kommandozeile dieselben Werte annehmen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachKeys {
    /// Dieses Steuerbyte leitet die Taste ein (`0x01` für `ctrl-a`).
    Prefix(u8),
    /// Kein Abhängen über die Tastatur; jedes Byte geht an den Agenten.
    Disabled,
}

impl DetachKeys {
    /// Die Vorgabe, wie in `screen`.
    pub const DEFAULT_SPEC: &'static str = "ctrl-a";

    /// Was ein Befund als erlaubte Werte nennt.
    pub const ALLOWED: &'static str = "ctrl-a to ctrl-z except ctrl-h, ctrl-i, ctrl-j and ctrl-m, or \
                                       ctrl-\\, ctrl-], ctrl-^, ctrl-_ or none (ctrl-[ is excluded \
                                       because every arrow key starts with it, ctrl-h, ctrl-i, \
                                       ctrl-j and ctrl-m because Backspace, Tab and Enter send \
                                       them)";

    /// Liest eine Angabe wie `ctrl-a` oder `none`; `None`, wenn sie keine ist.
    ///
    /// Groß- und Kleinschreibung zählen nicht (`Ctrl-A` geht auch).
    /// `ctrl-[` ist `ESC` und fehlt mit Absicht: Jede Pfeiltaste und jede
    /// Funktionstaste beginnt mit diesem Byte, und ein Präfix dort hielte jede
    /// davon an, bis die nächste Taste kommt.
    #[must_use]
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim().to_ascii_lowercase();
        if spec == "none" {
            return Some(Self::Disabled);
        }
        let key = spec.strip_prefix("ctrl-")?;
        let mut chars = key.chars();
        let (Some(key), None) = (chars.next(), chars.next()) else {
            return None;
        };
        match key {
            // `ctrl-h`, `ctrl-i`, `ctrl-j` und `ctrl-m` schickt das Terminal
            // auch für Rücktaste, Tab und Enter: Jede dieser Tasten wartete
            // sonst auf eine zweite.
            'h' | 'i' | 'j' | 'm' => None,
            'a'..='z' | '\\' | ']' | '^' | '_' => {
                // Das Steuerbyte ist der Großbuchstabe ohne Bit 6: `A` (0x41)
                // wird 0x01, `]` (0x5d) wird 0x1d.
                let upper = u8::try_from(key.to_ascii_uppercase()).ok()?;
                Some(Self::Prefix(upper & 0x1f))
            }
            _ => None,
        }
    }

    /// Wie die Taste heißt, für den Hinweis beim Anhängen: `Ctrl+A d`.
    /// `None` ohne Präfix.
    #[must_use]
    pub fn label(self) -> Option<String> {
        match self {
            Self::Prefix(byte) => Some(format!("Ctrl+{} d", char::from(byte | 0x40))),
            Self::Disabled => None,
        }
    }
}

/// Schalter für unfertige Wege.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Experimental {
    /// Bietet dem Ziel HTTP/2 an. In M1 spricht der Proxy nach oben nur HTTP/1.1.
    #[schemars(extend("x-tier" = "expert", "x-project-scope" = "denied"))]
    pub h2_upstream: bool,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{Authority, HostName};

    use super::{DetachKeys, LlmConfig};

    /// Jede Angabe wird zu dem Byte, das das Terminal für diese Taste
    /// schickt, und zu dem Namen, den der Hinweis beim Anhängen nennt
    /// (HUM-279).
    #[test]
    fn a_detach_spec_names_the_byte_the_terminal_sends() {
        for (spec, byte, label) in [
            ("ctrl-a", 0x01, "Ctrl+A d"),
            ("CTRL-B", 0x02, "Ctrl+B d"),
            ("ctrl-z", 0x1a, "Ctrl+Z d"),
            ("ctrl-\\", 0x1c, "Ctrl+\\ d"),
            ("ctrl-]", 0x1d, "Ctrl+] d"),
            ("ctrl-^", 0x1e, "Ctrl+^ d"),
            ("ctrl-_", 0x1f, "Ctrl+_ d"),
        ] {
            let keys = DetachKeys::parse(spec);
            assert_eq!(keys, Some(DetachKeys::Prefix(byte)), "{spec}");
            assert_eq!(keys.and_then(DetachKeys::label).as_deref(), Some(label));
        }
        assert_eq!(DetachKeys::parse("none"), Some(DetachKeys::Disabled));
        assert_eq!(DetachKeys::Disabled.label(), None);
        for refused in [
            "ctrl-[", "ctrl-h", "ctrl-i", "ctrl-j", "ctrl-m", "Ctrl-M", "ctrl-@", "ctrl-1",
            "ctrl-", "ctrl-aa", "^a", "",
        ] {
            assert_eq!(DetachKeys::parse(refused), None, "{refused}");
        }
    }

    fn authority_of(endpoint: &str) -> Option<String> {
        LlmConfig {
            endpoint: Some(url::Url::parse(endpoint).unwrap()),
            ..LlmConfig::default()
        }
        .endpoint_authority()
        .map(|authority| authority.to_string())
    }

    /// Das Ziel des Endpunkts steht so da, wie der Proxy die Anfrage des
    /// Agenten an ihn normalisiert: ohne Punkt am Ende, klein, mit
    /// Standardport, ein IPv6-Literal wie `HostName` es schreibt (HUM-140,
    /// Review).
    #[test]
    fn the_endpoint_authority_is_normalised_like_a_proxied_request() {
        let proxied = |host: &str, port: u16| {
            Authority::new(HostName::parse(host).unwrap(), port).to_string()
        };
        assert_eq!(
            authority_of("http://Model.LAN.:11434/v1"),
            Some(proxied("model.lan.", 11434))
        );
        assert_eq!(
            authority_of("http://model.lan.:11434/").as_deref(),
            Some("model.lan:11434")
        );
        assert_eq!(
            authority_of("https://api.example.com/v1").as_deref(),
            Some("api.example.com:443")
        );
        assert_eq!(
            authority_of("http://192.168.1.20/").as_deref(),
            Some("192.168.1.20:80")
        );
        assert_eq!(
            authority_of("https://[::ffff:192.168.1.20]/"),
            Some(proxied("[::ffff:c0a8:114]", 443))
        );
        assert_eq!(LlmConfig::default().endpoint_authority(), None);
    }
}
