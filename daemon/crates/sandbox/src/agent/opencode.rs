//! Der Adapter für `OpenCode` (HUM-037).
//!
//! `OpenCode` braucht eine Provider-Konfiguration für einen OpenAI-kompatiblen
//! Endpunkt und tut beim Start von sich aus Dinge im Netz: es holt seinen
//! Modellkatalog, prüft auf neue Versionen, lädt Provider-Pakete per npm nach
//! und hat einen gehosteten `websearch`. Nichts davon hat ein Mensch ausgelöst,
//! und der erste gehaltene Fluss, den er zu sehen bekommt, soll einer sein, den
//! er versteht (BACKLOG.md Abschnitt 5). Der Adapter stellt das vor dem Start
//! ab: über die Umgebung ([`OpenCodeAdapter::env`]), über die mitgelieferte
//! Konfiguration ([`OpenCodeAdapter::files`]) und über den mitgelieferten
//! Regelsatz als zweites Schloss ([`OpenCodeAdapter::default_rules`]).
//!
//! **Abgleich mit der installierten Fassung.** Die Werte unten sind an `OpenCode`
//! 1.18.25 geprüft, nicht aus der Dokumentation übernommen. Zwei Stellen weichen
//! deshalb von `backlog/sprint-3.md` ab:
//!
//! - `OPENCODE_MODELS_URL` taugt nicht als Zeiger auf eine Datei. Der Wert ist
//!   eine Basis-Adresse (Vorgabe `https://models.opencode.ai`), an die `OpenCode`
//!   `/api.json` anhängt und die es über seinen HTTP-Client abruft; ein
//!   `file://`-Schema kommt dort nicht an. Der Adapter setzt stattdessen
//!   [`ENV_MODELS_PATH`] auf die mitgelieferte Datei und
//!   [`ENV_DISABLE_MODELS_FETCH`], das auch die stündliche Aktualisierung
//!   abschaltet. Die zweite Bridge auf Port 3129, die Fallstrick 1 als Ausweg
//!   beschreibt, wird damit nicht gebraucht — und die Sandbox behält ihre eine
//!   Tür.
//! - Der Katalog liegt in dieser Fassung auf `models.opencode.ai`, nicht auf
//!   `models.dev`. `rules/default.yaml` blockt beide Hosts.
//!
//! **Wie sich die Konfiguration gegen ein geklontes Repository durchsetzt.**
//! `OpenCode` mergt seine Konfigurationsquellen in dieser Reihenfolge, spätere
//! gewinnen: Konfigurationsverzeichnis, [`ENV_CONFIG`], **dann** `opencode.json`
//! aus dem Projektbaum, `.opencode`-Verzeichnisse, `OPENCODE_CONFIG_CONTENT`,
//! die Konfiguration einer angemeldeten Organisation, zuletzt das verwaltete
//! Verzeichnis (`managedConfigDir()`, unter Linux `/etc/opencode`). Danach
//! wird `OPENCODE_PERMISSION` über den Block `permission` gemergt.
//!
//! Ein geklontes Repository steht damit **über** [`ENV_CONFIG`] und kann
//! `model`, `share` und `permission` umschreiben. Gemessen an 1.18.25 mit
//! `opencode debug config`: mit Projektdatei allein gewinnt sie, mit
//! [`MANAGED_CONFIG_DST`] und [`ENV_PERMISSION`] gewinnt Humanitl. Der Adapter
//! legt seine Konfiguration deshalb an drei Orte und setzt zusätzlich
//! [`ENV_PERMISSION`].
//!
//! Für `provider` gilt das **nicht**: der Merge ist additiv. Ein Projekt kann
//! einen eigenen Provider mit eigener `baseURL` hinzufügen, und `opencode
//! models` listet ihn dann; ebenso einen weiteren Modelleintrag unter
//! `humanitl-local`. Es fällt dabei keine Garantie, weil jeder Verkehr dorthin
//! durch den Proxy geht und gehalten wird — aber wer sich fragt, was der Agent
//! ansprechen kann, muss das wissen. Welches Modell voreingestellt ist,
//! bestimmt weiterhin Humanitl.
//!
//! `OPENCODE_DISABLE_PROJECT_CONFIG` wäre der vierte Weg, schaltet aber
//! dieselbe Variable auch die `AGENTS.md` des Projekts ab, also die Datei, aus
//! der der Agent seine Arbeitsregeln liest. Der Adapter setzt sie deshalb
//! nicht. Was ein Projekt darüber hinaus darf, entscheidet HUM-043
//! („`/work`-Härtung"). Neue Ziele, die eine Projekt-Konfiguration hinzufügt,
//! gehen ohnehin durch den Proxy und werden gehalten.

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::LlmConfig;
use humanitl_core::diagnostics::codes::{AGENT_001, AGENT_002, AGENT_004, LLM_004};
use humanitl_core::rule::{Action, HostPattern, Matcher, Rule};
use humanitl_core::{Diagnostic, FixAction, HostName, Method, RuleId, Scheme, Severity};
use url::Url;
use uuid::Uuid;

use crate::agent::briefing;
use crate::agent::opencode_models::{
    DEFAULT_RULES, PROVIDER_ID, effective_models, permission_json, render_config, render_models,
};
use crate::agent::{AgentAdapter, AgentContext, SandboxFile, find_in_path};

pub use crate::agent::opencode_models::PLACEHOLDER_MODEL;

/// Die Kennung des Adapters, wie sie in `agent.adapter` steht.
pub const ADAPTER_ID: &str = "opencode";

/// Das Kommando, das ohne `agent.command` gestartet wird.
pub const DEFAULT_COMMAND: &str = "opencode";

/// Das Verzeichnis, in dem die Dateien dieses Adapters in der Sandbox liegen.
pub const OPENCODE_DIR: &str = "/etc/humanitl/opencode";

/// Die Konfiguration des Agenten in der Sandbox.
pub const CONFIG_DST: &str = "/etc/humanitl/opencode/opencode.json";

/// Der mitgelieferte Modellkatalog in der Sandbox.
pub const MODELS_DST: &str = "/etc/humanitl/opencode/models.json";

/// Das Verzeichnis, in dem `OpenCode` seine eigene Konfiguration sucht.
///
/// Unter dem `XDG_CONFIG_HOME`, das der Agent wirklich sieht
/// ([`AgentContext::config_home`]) — nicht unter einer Konstante und nicht
/// unter dem `HOME` des Profils. Setzt `sandbox.env` ein eigenes
/// `XDG_CONFIG_HOME`, gewinnt es in der Umgebung des Agenten, und die Dateien
/// müssen dorthin, sonst liegen sie da, wo niemand sie liest.
#[must_use]
pub fn agent_config_dir(config_home: &Path) -> PathBuf {
    config_home.join("opencode")
}

/// Die zweite Ablage derselben Konfiguration, im Verzeichnis des Agenten.
///
/// `OpenCode` liest `$XDG_CONFIG_HOME/opencode/opencode.json` immer und
/// `OPENCODE_CONFIG` zusätzlich; beide werden zusammengeführt. Derselbe Inhalt
/// an beiden Orten ist deshalb wirkungsgleich und deckt den Fall ab, dass eine
/// Fassung `OPENCODE_CONFIG` nicht beachtet (Fallstrick 4).
#[must_use]
pub fn config_dst(config_home: &Path) -> PathBuf {
    agent_config_dir(config_home).join("opencode.json")
}

/// Eine leere Datei, damit das Konfigurationsverzeichnis des Agenten existiert.
#[must_use]
pub fn keep_dst(config_home: &Path) -> PathBuf {
    agent_config_dir(config_home).join(".keep")
}

/// Der Name der Instruktionsdatei, die `OpenCode` als Erstes liest.
///
/// Gemessen an 1.18.25: `InstructionContext` setzt die Liste der Regeldateien
/// aus `join(<Konfigurationsverzeichnis>, "AGENTS.md")` und danach den
/// `AGENTS.md` des Projektbaums zusammen. Die globale Datei steht also vorn und
/// wird auch dann gelesen, wenn `OPENCODE_DISABLE_PROJECT_CONFIG` gesetzt ist —
/// diese Variable schaltet nur den Lauf durch den Projektbaum ab.
pub const BRIEFING_FILE_NAME: &str = "AGENTS.md";

/// Die Einweisung des Agenten in der Sandbox (HUM-071).
///
/// Unter dem Konfigurationsverzeichnis im Heimatverzeichnis, nie unter `/work`:
/// `OpenCode` liest die `AGENTS.md` des Projekts zusätzlich, und eine Datei,
/// die Humanitl dort ablegte, landete im Diff des Nutzers und irgendwann in
/// einem fremden Repository (ADR-0014).
#[must_use]
pub fn briefing_dst(config_home: &Path) -> PathBuf {
    agent_config_dir(config_home).join(BRIEFING_FILE_NAME)
}

/// Die dritte Ablage derselben Konfiguration: das verwaltete Verzeichnis.
///
/// `managedConfigDir()` liefert unter Linux `/etc/opencode`, und diese Quelle
/// wird nach der Konfiguration des Projekts gemergt. Nur sie setzt `model`,
/// `provider` und `share` gegen ein geklontes Repository durch.
pub const MANAGED_CONFIG_DST: &str = "/etc/opencode/opencode.json";

/// Die Adresse, unter der `OpenCode` installiert wird; steht im Fix von `AGENT_001`.
pub const INSTALL_COMMAND: &str = "curl -fsSL https://opencode.ai/install | bash";

/// Die Dokumentation von `OpenCode`; steht in `docs` von `AGENT_001`.
pub const DOCS_URL: &str = "https://opencode.ai/docs/";

/// Ein Endpunkt in der Form, die `llm.endpoint` erwartet; steht im Vorschlag
/// von `LLM_004`, wenn noch gar kein Endpunkt konfiguriert ist.
pub const EXAMPLE_ENDPOINT: &str = "http://192.168.1.50:11434";

/// Kein Versions-Check gegen GitHub.
pub const ENV_DISABLE_AUTOUPDATE: &str = "OPENCODE_DISABLE_AUTOUPDATE";
/// Der Modellkatalog kommt aus dieser Datei statt aus dem Netz.
pub const ENV_MODELS_PATH: &str = "OPENCODE_MODELS_PATH";
/// Auch die stündliche Aktualisierung des Katalogs unterbleibt.
pub const ENV_DISABLE_MODELS_FETCH: &str = "OPENCODE_DISABLE_MODELS_FETCH";
/// Die Konfiguration liegt außerhalb von `/work`.
pub const ENV_CONFIG: &str = "OPENCODE_CONFIG";
/// Kein Teilen der Sitzung von allein.
pub const ENV_AUTO_SHARE: &str = "OPENCODE_AUTO_SHARE";
/// Kein Teilen der Sitzung überhaupt.
pub const ENV_DISABLE_SHARE: &str = "OPENCODE_DISABLE_SHARE";
/// Kein gehosteter Websearch (Exa).
pub const ENV_ENABLE_EXA: &str = "OPENCODE_ENABLE_EXA";
/// Kein gehosteter Websearch (Parallel).
pub const ENV_ENABLE_PARALLEL: &str = "OPENCODE_ENABLE_PARALLEL";
/// Keine Sprachserver aus dem Netz nachladen.
pub const ENV_DISABLE_LSP_DOWNLOAD: &str = "OPENCODE_DISABLE_LSP_DOWNLOAD";
/// Die Berechtigungen; wird als Letztes über alle Konfigurationsquellen gemergt.
pub const ENV_PERMISSION: &str = "OPENCODE_PERMISSION";

/// Die Id der Passthrough-Regel; fest, damit sie überall dieselbe ist.
const PASSTHROUGH_RULE_ID: u128 = 0x0192_0000_0000_7000_8000_0000_0000_00ff;

/// Das Präfix, unter dem Ollama seine gesamte eigene API anbietet.
///
/// Es steht in der Vorgabe von `llm.passthrough_paths`, taugt aber nicht als
/// Grenze: Unter ihm liegen neben der Inferenz auch `POST /api/pull`,
/// `POST /api/create` und `DELETE /api/delete`. Ein Agent könnte damit
/// ungefragt Modelle nachladen und löschen, und zwar über die eine Regel, die
/// nicht gehalten wird und private Adressen erlauben darf. Der Adapter ersetzt
/// dieses Präfix deshalb durch [`OLLAMA_INFERENCE_PATHS`]
/// ([`passthrough_prefixes`]).
pub const OLLAMA_API_PREFIX: &str = "/api/";

/// Die Endpunkte der Ollama-API, die Inferenz machen oder auskunftgeben und
/// nichts am Server ändern.
///
/// Gemessen an der API-Dokumentation von Ollama: `generate`, `chat`, `embed`
/// und `embeddings` rechnen, `tags`, `show`, `ps` und `version` geben
/// Auskunft. Nicht dabei: `pull`, `push`, `create`, `copy`, `delete` und
/// `blobs` — sie ändern den Bestand des Servers, und dafür gibt es die
/// Warteschlange. Wer sie trotzdem ohne Rückfrage will, schreibt den Pfad
/// selbst in `llm.passthrough_paths`.
pub const OLLAMA_INFERENCE_PATHS: &[&str] = &[
    "/api/chat",
    "/api/generate",
    "/api/embed",
    "/api/embeddings",
    "/api/tags",
    "/api/show",
    "/api/ps",
    "/api/version",
];

/// Das Präfix, unter dem eine OpenAI-kompatible API ihre ganze Fläche anbietet.
///
/// Genau wie [`OLLAMA_API_PREFIX`] taugt es nicht als Grenze, und aus demselben
/// Grund. Der Name `/v1/` sagt nichts darüber, was darunter liegt: der Anbieter
/// selbst bietet dort `POST /v1/files`, `/v1/uploads`, `/v1/vector_stores` und
/// `/v1/fine_tuning/jobs` an, vLLM `POST /v1/load_lora_adapter` und
/// `/v1/unload_lora_adapter`. Alles davon ist `POST` und würde von einem
/// Flächen-Präfix gedeckt: ungehalten, mit `allow_private`. Der Adapter ersetzt
/// das Präfix deshalb durch [`OPENAI_INFERENCE_PATHS`]
/// ([`passthrough_prefixes`]).
pub const OPENAI_PREFIX: &str = "/v1/";

/// Die Endpunkte der OpenAI-kompatiblen Oberfläche, die ein Coding-Agent für
/// Inferenz braucht.
///
/// `chat/completions` und `completions` rechnen, `responses` ist die neuere
/// Form davon, `embeddings` bettet ein, `models` gibt Auskunft und deckt über
/// das Präfix auch `models/<id>` ab. Nicht dabei: alles, was auf dem Server
/// etwas anlegt, ablegt oder umbaut — Dateien, Uploads, Vektorspeicher,
/// Feinabstimmung, LoRA-Adapter. Auch Bild- und Audio-Endpunkte fehlen; sie
/// erzeugen Artefakte und gehören nicht zu dem, wofür die Durchreiche da ist.
/// Wer sie braucht, schreibt ihren Pfad selbst in `llm.passthrough_paths`.
pub const OPENAI_INFERENCE_PATHS: &[&str] = &[
    "/v1/chat/completions",
    "/v1/completions",
    "/v1/responses",
    "/v1/embeddings",
    "/v1/models",
];

/// Host, Schema und Port, für die die Durchreiche gilt.
///
/// `None`, wenn daraus keine Regel entsteht: kein `llm.endpoint`, ein Wert
/// ohne Host, ein Host, den [`HostName::parse`] ablehnt, ein unbekanntes
/// Schema oder ein Schema ohne Vorgabeport. Genau diese Prüfungen macht
/// [`AgentAdapter::llm_passthrough`], und sie stehen hier, damit die Regel und
/// der Text des Briefings nicht auseinanderlaufen können: Ein Endpunkt wie
/// `http://good.test`#x`:11434` überlebt `Url::parse`, aber nicht
/// [`HostName::parse`] — die Regel entsteht dann nicht, und das Briefing darf
/// den Host dann auch nicht nennen (HUM-071, Review vom 2026-09-04).
#[must_use]
pub fn passthrough_target(llm: &LlmConfig) -> Option<(HostName, Scheme, u16)> {
    let url = llm.endpoint.as_ref()?;
    let host = HostName::parse(url.host_str()?).ok()?;
    let scheme = Scheme::parse(url.scheme())?;
    let port = url.port_or_known_default()?;
    Some((host, scheme, port))
}

/// Der Host der Durchreiche als `host:port`, für das Briefing.
///
/// Derselbe Port, auf den die Regel passt, also der Vorgabeport des Schemas,
/// wenn der Endpunkt keinen nennt. Ohne Regel kein Host: siehe
/// [`passthrough_target`].
#[must_use]
pub fn passthrough_authority(llm: &LlmConfig) -> Option<String> {
    let (host, _, port) = passthrough_target(llm)?;
    Some(format!("{host}:{port}"))
}

/// Der Adapter für `OpenCode`.
///
/// Zustandslos: alles, was er wissen muss, steht im [`AgentContext`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenCodeAdapter;

impl OpenCodeAdapter {
    /// Ein Adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Die Basis-Adresse, die in `opencode.json` steht.
///
/// `llm.endpoint` plus `/v1`, es sei denn, der Endpunkt endet schon darauf.
/// Der Pfadanteil des Endpunkts bleibt erhalten; im Matcher der
/// Passthrough-Regel wird er dagegen ignoriert (HUM-039, Fallstricke).
#[must_use]
pub fn base_url(endpoint: &Url) -> String {
    let text = endpoint.as_str().trim_end_matches('/');
    if text.ends_with("/v1") {
        text.to_owned()
    } else {
        format!("{text}/v1")
    }
}

/// Die Pfadpräfixe der Durchreichregel aus `llm.passthrough_paths`.
///
/// Drei Schritte, in dieser Reihenfolge:
///
/// 1. **Aussortieren.** Ein Präfix zählt nur, wenn es
///    [`humanitl_core::path_prefix_is_valid`] besteht, also mit `/` beginnt und
///    mindestens zwei Zeichen lang ist. `""` und `/` träfen jeden Pfad und
///    höben genau die Grenze auf, die die Liste ziehen soll (HUM-039,
///    Fallstricke).
/// 2. **Verengen.** Ein Präfix soll einen Endpunkt benennen, keine ganze
///    API-Fläche. [`OLLAMA_API_PREFIX`] deckt neben der Inferenz auch
///    `POST /api/pull`, `POST /api/create` und `DELETE /api/delete`;
///    [`OPENAI_PREFIX`] deckt neben der Inferenz `POST /v1/files`,
///    `/v1/uploads`, `/v1/vector_stores`, `/v1/fine_tuning/jobs` und bei vLLM
///    `/v1/load_lora_adapter`. Der Adapter setzt an ihre Stelle die Endpunkte
///    aus [`OLLAMA_INFERENCE_PATHS`] beziehungsweise
///    [`OPENAI_INFERENCE_PATHS`]. Damit gilt die Durchreiche für das, wofür sie
///    da ist — Inferenz und die Modellliste —, und alles Übrige an demselben
///    Server geht durch die Warteschlange, wird also einem Menschen gezeigt.
///    Wer eine dieser Anfragen ohne Rückfrage will, schreibt ihren Pfad selbst
///    in `llm.passthrough_paths` (`"/api/pull"`, `"/v1/files"`); ein Präfix,
///    das mehr nennt als das nackte `/api/` oder `/v1/`, bleibt unverändert
///    stehen.
/// 3. **Zurückfallen.** Bleibt nichts übrig, gelten
///    [`OPENAI_INFERENCE_PATHS`] und [`OLLAMA_INFERENCE_PATHS`]. Eine engere
///    Regel ist ein erklärbarer Zustand, eine unbegrenzte nicht.
///
/// Doppelte Einträge fallen weg, die Reihenfolge der Konfiguration bleibt.
#[must_use]
pub fn passthrough_prefixes(prefixes: &[String]) -> Vec<String> {
    fn push(out: &mut Vec<String>, prefix: &str) {
        if !out.iter().any(|existing| existing == prefix) {
            out.push(prefix.to_owned());
        }
    }

    /// Die Endpunkte, die an die Stelle eines Flächen-Präfixes treten.
    ///
    /// Mit und ohne abschließenden `/` ist dasselbe gemeint, und ohne ihn ist
    /// es sogar breiter: `/api` träfe auch `/apifoo`.
    fn expansion_of(prefix: &str) -> Option<&'static [&'static str]> {
        let bare = prefix.trim_end_matches('/');
        if bare == OLLAMA_API_PREFIX.trim_end_matches('/') {
            return Some(OLLAMA_INFERENCE_PATHS);
        }
        if bare == OPENAI_PREFIX.trim_end_matches('/') {
            return Some(OPENAI_INFERENCE_PATHS);
        }
        None
    }

    let mut out: Vec<String> = Vec::new();
    for prefix in prefixes {
        let prefix = prefix.trim();
        if !humanitl_core::path_prefix_is_valid(prefix) {
            continue;
        }
        match expansion_of(prefix) {
            Some(paths) => {
                for path in paths {
                    push(&mut out, path);
                }
            }
            None => push(&mut out, prefix),
        }
    }

    if out.is_empty() {
        for path in OPENAI_INFERENCE_PATHS.iter().chain(OLLAMA_INFERENCE_PATHS) {
            push(&mut out, path);
        }
    }
    out
}

impl AgentAdapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn command(&self, ctx: &AgentContext) -> Vec<OsString> {
        ctx.agent_command_override
            .clone()
            .filter(|command| !command.is_empty())
            .unwrap_or_else(|| vec![OsString::from(DEFAULT_COMMAND)])
    }

    fn env(&self, ctx: &AgentContext) -> Vec<(String, String)> {
        let ca = ctx.ca_path_sandbox.to_string_lossy().into_owned();
        let home = ctx.home.to_string_lossy().into_owned();
        // Dasselbe Verzeichnis, in das `files()` schreibt. Setzt `sandbox.env`
        // ein eigenes `XDG_CONFIG_HOME`, steht es schon in `ctx.config_home`,
        // und beide Seiten meinen weiter denselben Ort.
        let config_home = ctx.config_home().to_string_lossy().into_owned();
        [
            (ENV_DISABLE_AUTOUPDATE, "true".to_owned()),
            (ENV_MODELS_PATH, MODELS_DST.to_owned()),
            (ENV_DISABLE_MODELS_FETCH, "true".to_owned()),
            (ENV_CONFIG, CONFIG_DST.to_owned()),
            (ENV_AUTO_SHARE, "false".to_owned()),
            (ENV_DISABLE_SHARE, "true".to_owned()),
            (ENV_ENABLE_EXA, "false".to_owned()),
            (ENV_ENABLE_PARALLEL, "false".to_owned()),
            (ENV_DISABLE_LSP_DOWNLOAD, "true".to_owned()),
            ("HOME", home.clone()),
            ("XDG_CONFIG_HOME", config_home),
            ("XDG_DATA_HOME", format!("{home}/.local/share")),
            ("XDG_CACHE_HOME", format!("{home}/.cache")),
            ("NODE_EXTRA_CA_CERTS", ca),
            ("TERM", "xterm-256color".to_owned()),
            ("COLORTERM", "truecolor".to_owned()),
            ("LANG", "C.UTF-8".to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        // Scheitert die Vorlage, bleibt die Variable weg statt leer zu stehen:
        // `OPENCODE_PERMISSION=""` wäre ungültiges JSON, und `OpenCode` würde
        // es mit einer Warnung verwerfen. Denselben Fehler meldet `files()`
        // dann als `AGENT_003`, und der Start bricht dort ab.
        .chain(
            permission_json()
                .ok()
                .map(|json| (ENV_PERMISSION.to_owned(), json)),
        )
        .collect()
    }

    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        let base = ctx.llm.endpoint.as_ref().map_or_else(
            || format!("http://127.0.0.1:{}/v1", ctx.proxy_port),
            base_url,
        );
        let config = render_config(&base, &ctx.llm.models)?;
        let models = render_models(&ctx.llm.models)?;

        let config_home = ctx.config_home();
        let mut files = vec![
            SandboxFile::read_only(CONFIG_DST, config.clone().into_bytes()),
            SandboxFile::read_only(MODELS_DST, models.into_bytes()),
            SandboxFile::read_only(config_dst(&config_home), config.clone().into_bytes()),
            SandboxFile::read_only(keep_dst(&config_home), Vec::new()),
            SandboxFile::read_only(MANAGED_CONFIG_DST, config.into_bytes()),
        ];
        // `agent.briefing.enabled = false` unterdrückt die Datei ganz, statt
        // eine leere anzulegen: `OpenCode` überspringt eine fehlende globale
        // Instruktionsdatei, und eine leere stünde als Aussage da, die niemand
        // geschrieben hat.
        if ctx.briefing.enabled {
            // Der Host kommt aus derselben geprüften Quelle wie die
            // Durchreichregel ([`passthrough_target`]). Stünde dort ein Wert,
            // für den `llm_passthrough` keine Regel baut, nennte das Briefing
            // einen Host, der gar nicht durchgereicht wird — und der Agent
            // wunderte sich, warum sein Modell in der Warteschlange steht.
            let text = briefing::render(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                passthrough_authority(&ctx.llm).as_deref(),
            )?;
            files.push(SandboxFile::read_only(
                briefing_dst(&config_home),
                text.into_bytes(),
            ));
        }
        Ok(files)
    }

    fn default_rules(&self) -> &'static str {
        DEFAULT_RULES
    }

    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule> {
        let url = llm.endpoint.as_ref()?;
        let (host, scheme, port) = passthrough_target(llm)?;

        let matcher = Matcher::host(HostPattern::Exact(host))
            // `GET` ist dabei, weil OpenCode die Modellliste über
            // `GET /v1/models` holt.
            .with_methods(vec![Method::POST, Method::GET])
            .with_path_prefixes(passthrough_prefixes(&llm.passthrough_paths))
            .with_scheme(scheme)
            .with_port(port);

        Some(
            Rule::new(
                RuleId::from_uuid(Uuid::from_u128(PASSTHROUGH_RULE_ID)),
                Action::Allow,
                matcher,
            )
            // Das Modell steht üblicherweise im eigenen Netz; ohne dieses Recht
            // verweigerte der Proxy die aufgelöste Adresse (ADR-006).
            .with_allow_private(true)
            .bundled(true)
            // Erst dieses Merkmal macht aus der Regel die erklärte Ausnahme:
            // Der Proxy hält solche Flüsse nicht, zeichnet sie vollständig auf
            // und warnt vor Funden, statt sie aufzuhalten (`LLM_005`).
            .passthrough_llm(true)
            .with_note(format!("LLM passthrough for {url}. Logged, never held.")),
        )
    }

    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let path = ctx.host_path.as_deref();
        let mut resolved: Option<PathBuf> = None;

        match ctx.agent_command_override.as_ref().and_then(|c| c.first()) {
            Some(command) => {
                let found = find_in_path(command, path);
                resolved.clone_from(&found);
                let executable = found.as_ref().is_some_and(|full| {
                    std::fs::metadata(full).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
                });
                if !executable {
                    diagnostics.push(
                        Diagnostic::builder(AGENT_002, Severity::Warning)
                            .why(format!(
                                "agent.command points at {:?}, which is not an executable file on \
                                 this machine; the sandbox will still start, because the path \
                                 inside it can be a different one",
                                command.to_string_lossy()
                            ))
                            // Ein leerer Wert wird von
                            // `humanitl_config::validate` abgewiesen
                            // (`agent.command` ist entweder nichts oder
                            // mindestens das Programm). Der Vorschlag ist
                            // deshalb die Vorgabe des Adapters.
                            .fix(FixAction::ChangeSetting {
                                key: "agent.command".to_owned(),
                                value: format!("[\"{DEFAULT_COMMAND}\"]"),
                            })
                            .build(),
                    );
                }
            }
            // Ohne Suchpfad gibt es nichts zu durchsuchen. „Pfad unbekannt"
            // ist nicht „Programm fehlt": ein Befund braucht einen Beleg, und
            // was der Daemon nicht weiß, steht nicht als Fehler da
            // (`backlog/CONVENTIONS.md` 4.13). Der Aufrufer reicht
            // `AgentContext::host_path` herein; tut er es nicht, unterbleibt
            // die Prüfung, und das `exec` in der Sandbox entscheidet.
            None if path.is_none() => {}
            None => {
                let found = find_in_path(std::ffi::OsStr::new(DEFAULT_COMMAND), path);
                resolved.clone_from(&found);
                if found.is_none() {
                    diagnostics.push(
                        Diagnostic::builder(AGENT_001, Severity::Blocking)
                            .why(format!(
                                "{DEFAULT_COMMAND} is not in PATH={} and agent.command is not set",
                                path.map(|p| p.to_string_lossy().into_owned())
                                    .unwrap_or_default()
                            ))
                            .fix(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
                            .docs(DOCS_URL)
                            .build(),
                    );
                }
            }
        }

        // Auf dem Host gefunden heißt nicht: in der Sandbox erreichbar. Die
        // Sandbox hängt `/usr` und was sonst in `[mounts]` steht nur lesbar
        // ein; ein Programm unter `$HOME` ist dort nicht da, und das `exec`
        // scheiterte erst nach dem Start.
        if let Some(binary) = resolved.as_deref()
            && !ctx.sandbox_ro_paths.is_empty()
            && !ctx.is_visible_in_sandbox(binary)
        {
            diagnostics.push(
                Diagnostic::builder(AGENT_004, Severity::Blocking)
                    .why(format!(
                        "{} is on this machine, but the sandbox mounts only {} read-only, \
                         so the command is not there and the exec would fail after the start. \
                         Put the binary under one of those paths, or add its directory to \
                         `[mounts].extra_ro` of the sandbox profile.",
                        binary.display(),
                        ctx.sandbox_ro_paths
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .fix(FixAction::CopyCommand(format!(
                        "sudo install -m 0755 {} /usr/local/bin/{DEFAULT_COMMAND}",
                        binary.display()
                    )))
                    .docs(DOCS_URL)
                    .build(),
            );
        }

        if effective_models(&ctx.llm.models) == vec![PLACEHOLDER_MODEL.to_owned()] {
            // Der Vorschlag ist der Weg zu den Namen, nicht ein erfundener
            // Name: welche Modelle der Server anbietet, weiß nur er. Sobald
            // die Probe aus HUM-039 steht, füllt sie `llm.models` selbst.
            let fix = ctx.llm.endpoint.as_ref().map_or_else(
                || FixAction::ChangeSetting {
                    key: "llm.endpoint".to_owned(),
                    value: EXAMPLE_ENDPOINT.to_owned(),
                },
                |endpoint| {
                    FixAction::CopyCommand(format!("curl -sS {}/models", base_url(endpoint)))
                },
            );
            diagnostics.push(
                Diagnostic::builder(LLM_004, Severity::Warning)
                    .why(format!(
                        "llm.models is empty; the agent is configured with the placeholder model \
                         {PROVIDER_ID}/{PLACEHOLDER_MODEL}, and whether the server knows a model \
                         of that name is unknown. Put the ids the server reports into llm.models."
                    ))
                    .fix(fix)
                    .build(),
            );
        }

        diagnostics
    }

    fn is_fullscreen_tui(&self) -> bool {
        true
    }
}

/// Die Dateien, die der Adapter anlegt, als Pfade.
///
/// Für die Anzeige und für Tests; die Reihenfolge ist die von
/// [`AgentAdapter::files`]. `briefing` ist `agent.briefing.enabled`: steht es
/// aus, fehlt die Instruktionsdatei auch hier.
#[must_use]
pub fn file_targets(config_home: &Path, briefing: bool) -> Vec<PathBuf> {
    let mut targets = vec![
        PathBuf::from(CONFIG_DST),
        PathBuf::from(MODELS_DST),
        config_dst(config_home),
        keep_dst(config_home),
        PathBuf::from(MANAGED_CONFIG_DST),
    ];
    if briefing {
        targets.push(briefing_dst(config_home));
    }
    targets
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use url::Url;

    use super::{OLLAMA_INFERENCE_PATHS, OPENAI_INFERENCE_PATHS, base_url, passthrough_prefixes};

    /// Die Vorgabe, so wie sie aus `llm.passthrough_paths` entsteht.
    fn default_prefixes() -> Vec<String> {
        OPENAI_INFERENCE_PATHS
            .iter()
            .chain(OLLAMA_INFERENCE_PATHS)
            .map(|path| (*path).to_owned())
            .collect()
    }

    /// Pfade, die keine Inferenz sind und den Server verändern oder belegen.
    ///
    /// Ollama aus `server/routes.go`, die OpenAI-kompatiblen und die
    /// vLLM-Pfade aus ihren API-Referenzen. Keiner davon darf von der Vorgabe
    /// gedeckt sein.
    const MUTATING: &[&str] = &[
        "/api/pull",
        "/api/push",
        "/api/create",
        "/api/copy",
        "/api/delete",
        "/api/blobs/sha256:aa",
        "/v1/files",
        "/v1/uploads",
        "/v1/vector_stores",
        "/v1/fine_tuning/jobs",
        "/v1/batches",
        "/v1/assistants",
        "/v1/load_lora_adapter",
        "/v1/unload_lora_adapter",
    ];

    #[test]
    fn v1_is_appended_once() {
        for (endpoint, expected) in [
            ("http://192.168.1.50:11434", "http://192.168.1.50:11434/v1"),
            ("http://192.168.1.50:11434/", "http://192.168.1.50:11434/v1"),
            ("http://x:1/v1", "http://x:1/v1"),
            ("http://x:1/v1/", "http://x:1/v1"),
        ] {
            let url = Url::parse(endpoint).unwrap();
            assert_eq!(base_url(&url), expected, "for {endpoint}");
        }
    }

    /// Beide Flächen-Präfixe der Vorgabe werden zu Endpunkten.
    #[test]
    fn both_surface_prefixes_are_narrowed_to_inference_endpoints() {
        let prefixes = passthrough_prefixes(&["/v1/".to_owned(), "/api/".to_owned()]);
        assert_eq!(prefixes, default_prefixes());
        assert!(
            prefixes.iter().any(|p| p == "/v1/chat/completions"),
            "inference stays: {prefixes:?}"
        );
        assert!(
            prefixes.iter().any(|p| p == "/api/chat"),
            "inference stays: {prefixes:?}"
        );
    }

    /// Kein Pfad, der den Server verändert, wird von der Vorgabe gedeckt.
    ///
    /// Das ist die eigentliche Zusage der Verengung, und sie gilt für beide
    /// Flächen. Ohne sie deckte `/v1/` `POST /v1/files` und `/api/`
    /// `POST /api/pull`, beides ungehalten und mit `allow_private`.
    #[test]
    fn no_mutating_path_is_covered_by_the_default() {
        for prefixes in [
            passthrough_prefixes(&["/v1/".to_owned(), "/api/".to_owned()]),
            passthrough_prefixes(&[]),
            passthrough_prefixes(&[String::new(), "/".to_owned()]),
        ] {
            for path in MUTATING {
                assert!(
                    !prefixes.iter().any(|prefix| path.starts_with(prefix)),
                    "{path} is covered by {prefixes:?}"
                );
            }
        }
    }

    /// Auch ohne abschließenden `/` sind `/api` und `/v1` die ganze Fläche —
    /// und träfen obendrein `/apifoo` und `/v1beta`.
    #[test]
    fn a_surface_prefix_is_narrowed_with_and_without_the_trailing_slash() {
        for (written, expected) in [
            ("/api", OLLAMA_INFERENCE_PATHS),
            ("/api/", OLLAMA_INFERENCE_PATHS),
            ("/v1", OPENAI_INFERENCE_PATHS),
            ("/v1/", OPENAI_INFERENCE_PATHS),
        ] {
            let prefixes = passthrough_prefixes(&[written.to_owned()]);
            let expected: Vec<String> = expected.iter().map(|p| (*p).to_owned()).collect();
            assert_eq!(prefixes, expected, "for {written}");
        }
    }

    /// Wer den Pfad selbst hinschreibt, bekommt ihn: die Vorgabe ist die
    /// sichere Seite, nicht die einzige.
    #[test]
    fn an_explicit_path_stays_even_when_it_changes_the_server() {
        assert_eq!(
            passthrough_prefixes(&["/api/chat".to_owned(), "/api/pull".to_owned()]),
            vec!["/api/chat".to_owned(), "/api/pull".to_owned()]
        );
        assert_eq!(
            passthrough_prefixes(&["/v1/files".to_owned()]),
            vec!["/v1/files".to_owned()]
        );
    }

    #[test]
    fn an_empty_prefix_does_not_widen_the_rule() {
        assert_eq!(
            passthrough_prefixes(&[String::new(), "/".to_owned()]),
            default_prefixes(),
            "an unusable prefix list falls back, it never matches everything"
        );
        assert_eq!(
            passthrough_prefixes(&[]),
            default_prefixes(),
            "an empty list falls back the same way"
        );
    }

    #[test]
    fn duplicates_collapse_and_the_order_of_the_configuration_stays() {
        let prefixes = passthrough_prefixes(&[
            "/api/chat".to_owned(),
            "/v1/models".to_owned(),
            "/api/chat".to_owned(),
        ]);
        assert_eq!(
            prefixes,
            vec!["/api/chat".to_owned(), "/v1/models".to_owned()]
        );
    }
}
