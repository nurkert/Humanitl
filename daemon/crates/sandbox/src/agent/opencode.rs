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
use humanitl_core::rule::{Action, HostPattern, Matcher, PathPattern, Rule};
use humanitl_core::{Diagnostic, FixAction, HostName, Method, RuleId, Scheme, Severity};
use url::Url;
use uuid::Uuid;

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

/// Die zweite Ablage derselben Konfiguration, unterhalb des Heimatverzeichnisses.
///
/// `OpenCode` liest `$XDG_CONFIG_HOME/opencode/opencode.json` immer und
/// `OPENCODE_CONFIG` zusätzlich; beide werden zusammengeführt. Derselbe Inhalt
/// an beiden Orten ist deshalb wirkungsgleich und deckt den Fall ab, dass eine
/// Fassung `OPENCODE_CONFIG` nicht beachtet (Fallstrick 4). Der Pfad hängt am
/// [`AgentContext::home`] des Profils, nicht an einer Konstante.
#[must_use]
pub fn home_config_dst(home: &Path) -> PathBuf {
    home.join(".config/opencode/opencode.json")
}

/// Eine leere Datei, damit das Konfigurationsverzeichnis des Agenten existiert.
#[must_use]
pub fn home_keep_dst(home: &Path) -> PathBuf {
    home.join(".config/opencode/.keep")
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

/// Die Pfadpräfixe, die gelten, wenn `llm.passthrough_paths` keinen brauchbaren
/// Eintrag hat.
///
/// Dieselben Werte wie die Vorgabe des Schlüssels in `humanitl-config`; hier
/// noch einmal, damit ein leerer oder unbrauchbarer Eintrag nicht in eine Regel
/// ohne Pfadgrenze mündet.
pub const FALLBACK_PASSTHROUGH_PATHS: &[&str] = &["/v1/", "/api/"];

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

/// Setzt vor jedes Zeichen mit Bedeutung im regulären Ausdruck einen
/// Rückstrich.
///
/// Dieselbe Zeichenmenge, die `regex::escape` schützt. Der Adapter baut das
/// Muster selbst, statt die Crate `regex` zu ziehen: er braucht sie sonst
/// nirgends, und die Zeichenmenge ist Teil der Sprache, nicht der Bibliothek.
fn escape_regex(input: &str) -> String {
    const SPECIAL: &str = r"\.+*?()|[]{}^$#&-~";
    let mut out = String::with_capacity(input.len() * 2);
    for character in input.chars() {
        if SPECIAL.contains(character) {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

/// Das Pfadmuster der Passthrough-Regel aus den Präfixen der Konfiguration.
///
/// Ein Präfix zählt, wenn es mit `/` beginnt und mindestens zwei Zeichen lang
/// ist; ein leeres Präfix würde jeden Pfad treffen und damit die Grenze
/// aufheben, die die Regel setzen soll (HUM-039, Fallstricke). Bleibt kein
/// gültiges Präfix übrig, gelten [`FALLBACK_PASSTHROUGH_PATHS`]: eine engere
/// Regel ist ein erklärbarer Zustand, eine unbegrenzte nicht.
///
/// Das Ergebnis ist ein regulärer Ausdruck, weil ein [`Matcher`] genau ein
/// Pfadmuster trägt. HUM-039 ersetzt ihn durch das Feld `path_prefixes`.
#[must_use]
pub fn passthrough_path_pattern(prefixes: &[String]) -> PathPattern {
    let valid: Vec<String> = prefixes
        .iter()
        .map(|prefix| prefix.trim())
        .filter(|prefix| prefix.starts_with('/') && prefix.chars().count() >= 2)
        .map(escape_regex)
        .collect();

    let alternatives = if valid.is_empty() {
        FALLBACK_PASSTHROUGH_PATHS
            .iter()
            .map(|prefix| escape_regex(prefix))
            .collect::<Vec<_>>()
    } else {
        valid
    };

    PathPattern::Regex(format!("^(?:{})", alternatives.join("|")))
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
            ("XDG_CONFIG_HOME", format!("{home}/.config")),
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

        Ok(vec![
            SandboxFile::read_only(CONFIG_DST, config.clone().into_bytes()),
            SandboxFile::read_only(MODELS_DST, models.into_bytes()),
            SandboxFile::read_only(home_config_dst(&ctx.home), config.clone().into_bytes()),
            SandboxFile::read_only(home_keep_dst(&ctx.home), Vec::new()),
            SandboxFile::read_only(MANAGED_CONFIG_DST, config.into_bytes()),
        ])
    }

    fn default_rules(&self) -> &'static str {
        DEFAULT_RULES
    }

    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule> {
        let url = llm.endpoint.as_ref()?;
        let host = HostName::parse(url.host_str()?).ok()?;
        let scheme = Scheme::parse(url.scheme())?;
        let port = url.port_or_known_default()?;

        let matcher = Matcher::host(HostPattern::Exact(host))
            // `GET` ist dabei, weil OpenCode die Modellliste über
            // `GET /v1/models` holt.
            .with_methods(vec![Method::POST, Method::GET])
            .with_path(passthrough_path_pattern(&llm.passthrough_paths))
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
/// [`AgentAdapter::files`].
#[must_use]
pub fn file_targets(home: &Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from(CONFIG_DST),
        PathBuf::from(MODELS_DST),
        home_config_dst(home),
        home_keep_dst(home),
        PathBuf::from(MANAGED_CONFIG_DST),
    ]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::rule::PathPattern;
    use url::Url;

    use super::{base_url, escape_regex, passthrough_path_pattern};

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

    #[test]
    fn regex_special_characters_are_escaped() {
        assert_eq!(escape_regex("/v1/"), "/v1/");
        assert_eq!(escape_regex("/a.b*"), r"/a\.b\*");
    }

    #[test]
    fn an_empty_prefix_does_not_widen_the_rule() {
        let pattern = passthrough_path_pattern(&[String::new(), "/".to_owned()]);
        assert_eq!(
            pattern,
            PathPattern::Regex("^(?:/v1/|/api/)".to_owned()),
            "an unusable prefix list falls back, it never matches everything"
        );
    }
}
