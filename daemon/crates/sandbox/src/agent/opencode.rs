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
//! **Keine zweite Frage, kein vorzeitiger Abbruch (HUM-308).** In der Sandbox
//! ist Humanitl die Schranke, nicht `OpenCode`. Die Vorlage erlaubt deshalb
//! `webfetch` und `bash` ohne Rückfrage; `edit` erlaubt `OpenCode` ohnehin
//! (Vorgabe `"*": "allow"`), und ein ausdrückliches `"edit": "allow"` stünde
//! als Letztes über dem `edit: deny` des eingebauten Plan-Agenten und machte
//! den Plan-Modus schreibend. Warum die Rückfrage des Agenten in der Sandbox
//! nichts schützt, steht in `docs/SECURITY.md`. Die Frist von `bash` hebt
//! [`bash_default_timeout_ms`] über `hold.timeout_secs`; die von `webfetch`
//! steht in 1.18.31 fest im Code (Vorgabe 30, höchstens 120 Sekunden) und
//! lässt sich nicht einstellen. Für sie sagt das Briefing, dass ein Werkzeug,
//! das früher aufgibt, die Frage zurückzieht und `curl` wartet; der Skill
//! `humanitl` ([`skill_dst`], HUM-364) nennt dazu die Frist für
//! `curl --max-time`.
//!
//! **`Ctrl+C` beendet `OpenCode` nicht (HUM-353).** Im Terminal von Humanitl
//! geht `Ctrl+C` als `0x03` an den Agenten, und `OpenCode` 1.18.31 bindet
//! `app_exit` in der Vorgabe an `ctrl+c,ctrl+d,<leader>q`. Der Adapter legt
//! deshalb [`tui_dst`] an, mit `keybinds.app_exit` ohne `ctrl+c`. Die
//! Tastenbelegung steht in dieser Fassung in `tui.json`; `keybinds` in
//! `opencode.json` wirft `OpenCode` aus der Server-Konfiguration. Die Datei
//! liegt nur im Konfigurationsverzeichnis und nicht im verwalteten: `OpenCode`
//! liest `tui.json` von dort, dann `OPENCODE_TUI_CONFIG`, dann aus dem
//! Projektbaum, und eine eigene Belegung im Projekt gewinnt. Das ist gewollt,
//! denn die Belegung ist eine Vorgabe und keine Schranke. Die Ladereihenfolge
//! von `tui.json` und das Verwerfen von `keybinds` in `opencode.json` sind an
//! den Zeichenketten des Binärprogramms von 1.18.31 abgelesen, nicht in einem
//! Lauf beobachtet.
//!
//! `OPENCODE_DISABLE_PROJECT_CONFIG` wäre der vierte Weg, schaltet aber
//! dieselbe Variable auch die `AGENTS.md` des Projekts ab, also die Datei, aus
//! der der Agent seine Arbeitsregeln liest. Der Adapter setzt sie deshalb
//! nicht. Was ein Projekt darüber hinaus darf, entscheidet HUM-043
//! („`/work`-Härtung"). Neue Ziele, die eine Projekt-Konfiguration hinzufügt,
//! gehen ohnehin durch den Proxy und werden gehalten.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use humanitl_config::llm_endpoint;
use humanitl_config::{AskMode, HoldConfig, LlmConfig};
use humanitl_core::diagnostics::codes::{AGENT_001, AGENT_002, AGENT_004, LLM_004};
use humanitl_core::rule::{Action, HostPattern, Matcher, Rule};
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, HostName, Method, RuleId, Scheme, Severity};
use url::Url;
use uuid::Uuid;

use crate::agent::briefing;
use crate::agent::locate;
use crate::agent::opencode_models::{
    DEFAULT_RULES, PROVIDER_ID, effective_models, permission_json, render_config, render_models,
    render_tui,
};
use crate::agent::skill;
use crate::agent::{
    AgentAdapter, AgentContext, SandboxFile, SandboxLookup, find_in_path, is_executable_file,
};

pub use crate::agent::opencode_models::PLACEHOLDER_MODEL;

/// Die Kennung des Adapters, wie sie in `agent.adapter` steht.
pub const ADAPTER_ID: &str = "opencode";

/// Das Kommando, das ohne `agent.command` gestartet wird.
pub const DEFAULT_COMMAND: &str = "opencode";

/// Der Start von `OpenCode` 2 auf geteiltem Loopback (HUM-365).
///
/// Ab Version 2 spricht die Oberfläche mit einem lokalen Server: dem
/// Hintergrunddienst auf dem festen Port 49374 oder mit `--standalone` auf
/// einem beliebigen. Beides verbietet das Seatbelt-Profil, und die Oberfläche
/// bleibt ohne Meldung leer. Den festen Port zu erlauben ließe die Sandbox an
/// einen Dienst des Hosts, also startet dieses Skript den Server selbst auf
/// dem zufälligen Port der Sitzung, mit einem Passwort, weil jeder lokale
/// Prozess diesen Port erreicht (K-19), und beendet ihn mit der Oberfläche.
/// Der Server bleibt am Proxy; nur die Oberfläche, die allein mit ihm spricht,
/// geht an ihm vorbei. Fassungen vor 2 und andere Programme kennen `--server`
/// nicht und starten unverändert.
pub const SHARED_LOOPBACK_START: &str = r#"program="$1"; shift
case "$("$program" --version 2>/dev/null)" in
  v[2-9].*|[2-9].*|v[1-9][0-9].*|[1-9][0-9].*) ;;
  *) exec "$program" "$@" ;;
esac
address="127.0.0.1:$HUMANITL_AGENT_PORT"
OPENCODE_SERVER_PASSWORD="$(od -An -N24 -tx1 /dev/urandom | tr -d ' 
')"
export OPENCODE_SERVER_PASSWORD
"$program" serve --hostname 127.0.0.1 --port "$HUMANITL_AGENT_PORT" >/dev/null 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null' EXIT
tries=0
until /usr/bin/curl --noproxy '*' --silent --output /dev/null "http://$address/"; do
  tries=$((tries + 1))
  [ "$tries" -lt 100 ] || break
  sleep 0.1
done
NO_PROXY=127.0.0.1 no_proxy=127.0.0.1 "$program" --server "http://$address" "$@"
"#;

/// Das Programm eines Kommandos, auch wenn [`SHARED_LOOPBACK_START`] es trägt.
#[must_use]
pub fn shown_program(command: &[OsString]) -> Option<&OsString> {
    match command {
        [_, _, script, _, program, ..] if script == SHARED_LOOPBACK_START => Some(program),
        _ => command.first(),
    }
}

/// Das Verzeichnis, in dem die Dateien dieses Adapters in der Sandbox liegen.
pub const OPENCODE_DIR: &str = "/etc/humanitl/opencode";

/// Die Konfiguration des Agenten in der Sandbox.
pub const CONFIG_DST: &str = "/etc/humanitl/opencode/opencode.json";

/// Der mitgelieferte Modellkatalog in der Sandbox.
pub const MODELS_DST: &str = "/etc/humanitl/opencode/models.json";

/// Der Zugangsschlüssel des Modells in der Sandbox (HUM-140).
///
/// Eine Datei und keine Umgebungsvariable: Die Umgebung des Agenten setzt
/// `bwrap` mit `--setenv NAME WERT`, also über seine eigene Kommandozeile, und
/// die liest jeder Prozess desselben Nutzers in `/proc/<pid>/cmdline`. Die
/// Datei kommt aus einem memfd (`--ro-bind-data`); `opencode.json` verweist
/// mit `{file:…}` darauf, und `OpenCode` liest sie selbst.
pub const API_KEY_DST: &str = "/etc/humanitl/opencode/llm-api-key";

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

/// Die Konfiguration der Oberfläche, im Verzeichnis des Agenten (HUM-353).
///
/// Nur hier und nicht im verwalteten Verzeichnis: Eine `tui.json` des
/// Projekts wird danach gelesen und gewinnt, wer dort `app_exit` anders
/// belegt, behält seine Belegung.
#[must_use]
pub fn tui_dst(config_home: &Path) -> PathBuf {
    agent_config_dir(config_home).join("tui.json")
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

/// Der Skill `humanitl` (HUM-364), dort, wo `OpenCode` 1.18.31 globale
/// Skills sucht.
///
/// `Skill.discovery` durchsucht jedes Konfigurationsverzeichnis mit
/// `{skill,skills}/**/SKILL.md`, und das erste ist `Global.Path.config`,
/// dasselbe Verzeichnis wie [`briefing_dst`]. Nie unter `/work`: ein Skill
/// in `.opencode/skills` des Projekts landete im Diff des Nutzers.
#[must_use]
pub fn skill_dst(config_home: &Path) -> PathBuf {
    agent_config_dir(config_home)
        .join(skill::SKILLS_DIR_NAME)
        .join(skill::SKILL_NAME)
        .join(skill::SKILL_FILE_NAME)
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
/// Die Vorgabefrist des Werkzeugs `bash` in Millisekunden (HUM-308).
///
/// `OpenCode` 1.18.31 liest sie als positive ganze Zahl; alles andere
/// verwirft es still und nimmt [`OPENCODE_BASH_DEFAULT_TIMEOUT_MS`]. Das
/// Modell kann je Aufruf eine eigene Frist mitgeben; die Variable setzt nur
/// den Wert, der gilt, wenn es das nicht tut.
pub const ENV_BASH_DEFAULT_TIMEOUT_MS: &str = "OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS";

/// Die Frist, die `OpenCode` 1.18.31 ohne [`ENV_BASH_DEFAULT_TIMEOUT_MS`]
/// einem Befehl gibt: 120 Sekunden.
pub const OPENCODE_BASH_DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// So viel Zeit bekommt ein Befehl über `hold.timeout_secs` hinaus: für den
/// Verbindungsaufbau vor dem Halten und die Antwort danach.
pub const BASH_TIMEOUT_MARGIN_SECS: u64 = 60;

/// Der größte Wert, den ein Zeitgeber in JavaScript fasst, abzüglich der
/// 100 Millisekunden, die `OpenCode` selbst auf die Frist aufschlägt.
/// Darüber feuert `setTimeout` sofort statt nie.
const JS_TIMER_MAX_MS: u64 = 2_147_483_647 - 100;

/// Die Vorgabefrist für `bash`, die eine gehaltene Anfrage abwartet (HUM-308).
///
/// Ein `curl` im Werkzeug `bash` wartet, solange der Proxy hält. Mit den
/// 120 Sekunden von `OpenCode` bräche das Werkzeug den Befehl nach zwei
/// Minuten ab, während die Karte bis zu `hold.timeout_secs` (Vorgabe 300)
/// auf den Menschen wartet; der Abbruch schließt die Verbindung, und der
/// Proxy beendet den Fluss als `ClientTimeout`, bevor jemand entschieden hat.
///
/// Gilt nur, wenn jemand gefragt wird. Mit `ask_mode = none` endet eine
/// Anfrage ohne Regel sofort, und `None` lässt die Vorgabe von `OpenCode`
/// stehen. Nie kleiner als diese Vorgabe, nie größer als ein
/// JavaScript-Zeitgeber fasst.
#[must_use]
pub fn bash_default_timeout_ms(hold: &HoldConfig) -> Option<u64> {
    match hold.ask_mode {
        AskMode::None => None,
        AskMode::Ui | AskMode::Terminal => Some(
            hold.timeout_secs
                .saturating_add(BASH_TIMEOUT_MARGIN_SECS)
                .saturating_mul(1000)
                .clamp(OPENCODE_BASH_DEFAULT_TIMEOUT_MS, JS_TIMER_MAX_MS),
        ),
    }
}

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
/// Form davon, `embeddings` bettet ein, `models` gibt Auskunft. Jeder gilt
/// genau, nicht als Präfix (HUM-261): `models/<id>` und `models/delete` gehen
/// durch die Warteschlange. Nicht dabei: alles, was auf dem Server
/// etwas anlegt, ablegt oder umbaut — Dateien, Uploads, Vektorspeicher,
/// Feinabstimmung, LoRA-Adapter. Auch Bild- und Audio-Endpunkte fehlen; sie
/// erzeugen Artefakte und gehören nicht zu dem, wofür die Durchreiche da ist.
/// Wer sie braucht, schreibt ihren Pfad selbst in `llm.passthrough_paths`.
///
/// Die Pfade stehen hier in der Form für einen Endpunkt an der Wurzel des
/// Servers. Hat der Endpunkt ein Pfad-Präfix (`https://host/api/v1/`), liegen
/// sie unter dessen Basis, also `/api/v1/chat/completions` und so fort
/// ([`passthrough_prefixes`], HUM-261).
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
/// Die Endpunkte, die an die Stelle von `/v1/` und `/api/` treten, sind
/// genaue Einträge ([`humanitl_core::EXACT_PATH_MARK`], HUM-261): Sie treffen
/// genau ihren Pfad mit beliebiger Query, aber keinen Unterpfad wie
/// `/v1/models/delete`. Was der Mensch selbst in `llm.passthrough_paths`
/// schreibt, bleibt ein Präfix; ein Eintrag mit `?` fällt weg.
///
/// Die Endpunkte, die an die Stelle von `/v1/` treten, liegen unter der Basis
/// des Endpunkts, gelesen wie die `baseURL` von `OpenCode`
/// ([`llm_endpoint::openai_path`], HUM-261): bei `https://host/api/v1/` also
/// `/api/v1/chat/completions`, denn dorthin schickt der Agent. Die
/// Ollama-Endpunkte für `/api/` gibt es nur bei einem Endpunkt an der Wurzel
/// des Servers ([`llm_endpoint::serves_native_ollama`]); hinter einem
/// Pfad-Präfix fallen sie weg, statt an der Wurzel eines Hosts zu gelten, die
/// womöglich einem anderen Dienst gehört.
///
/// Doppelte Einträge fallen weg, die Reihenfolge der Konfiguration bleibt.
#[must_use]
pub fn passthrough_prefixes(endpoint: &Url, prefixes: &[String]) -> Vec<String> {
    fn push(out: &mut Vec<String>, prefix: &str) {
        if !out.iter().any(|existing| existing == prefix) {
            out.push(prefix.to_owned());
        }
    }

    // Die Inferenz-Endpunkte dieses Endpunkts: die OpenAI-kompatiblen unter
    // seiner Basis, die von Ollama nur an der Wurzel des Servers. Jeder als
    // genauer Eintrag (HUM-261): `/v1/models` trifft nicht
    // `/v1/models/delete`.
    let openai: Vec<String> = OPENAI_INFERENCE_PATHS
        .iter()
        .map(|path| {
            let relative = path
                .strip_prefix(llm_endpoint::OPENAI_SURFACE)
                .unwrap_or(path);
            humanitl_core::exact_path_entry(&llm_endpoint::openai_path(endpoint, relative))
        })
        .collect();
    let ollama: Vec<String> = if llm_endpoint::serves_native_ollama(endpoint) {
        OLLAMA_INFERENCE_PATHS
            .iter()
            .map(|path| humanitl_core::exact_path_entry(path))
            .collect()
    } else {
        Vec::new()
    };

    // Die Endpunkte, die an die Stelle eines Flächen-Präfixes treten.
    //
    // Mit und ohne abschließenden `/` ist dasselbe gemeint, und ohne ihn ist
    // es sogar breiter: `/api` träfe auch `/apifoo`.
    let expansion_of = |prefix: &str| -> Option<&[String]> {
        let bare = prefix.trim_end_matches('/');
        if bare == OLLAMA_API_PREFIX.trim_end_matches('/') {
            return Some(&ollama);
        }
        if bare == OPENAI_PREFIX.trim_end_matches('/') {
            return Some(&openai);
        }
        None
    };

    let mut out: Vec<String> = Vec::new();
    for prefix in prefixes {
        let prefix = prefix.trim();
        // Ein `?` kommt im verglichenen Pfad nie vor; ein solcher Eintrag hat
        // vor HUM-261 nichts getroffen und trifft auch jetzt nichts, statt
        // als genauer Eintrag zu gelten. Genau sind nur die Endpunkte, die
        // der Adapter selbst erzeugt.
        if !humanitl_core::path_prefix_is_valid(prefix)
            || prefix.contains(humanitl_core::EXACT_PATH_MARK)
        {
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
        for path in openai.iter().chain(&ollama) {
            push(&mut out, path);
        }
    }
    out
}

/// Der Befund zu einem Kommando, das auf diesem Rechner nicht ausführbar ist.
///
/// Der Satz nennt, woher das Kommando kommt, und der Vorschlag folgt daraus:
/// `agent.command` zu ändern hilft nur dem, der es gesetzt hat; wer sein
/// Kommando hinter `--` geschrieben hat, ändert seine eigene Zeile (HUM-135).
fn not_executable(command: &OsString, from_caller: bool, default_command: &str) -> Diagnostic {
    let named = command.to_string_lossy();
    let finding = Diagnostic::builder(AGENT_002, Severity::Warning).why(if from_caller {
        format!(
            "the command {named:?} of this run is not an executable file on this machine; the \
             sandbox will still start, because the path inside it can be a different one"
        )
    } else {
        format!(
            "agent.command points at {named:?}, which is not an executable file on this machine; \
             the sandbox will still start, because the path inside it can be a different one"
        )
    });
    if from_caller {
        return finding.build();
    }
    // Ein leerer Wert wird von `humanitl_config::validate` abgewiesen
    // (`agent.command` ist entweder nichts oder mindestens das Programm). Der
    // Vorschlag ist deshalb die Vorgabe des Adapters.
    finding
        .fix(FixAction::ChangeSetting {
            key: "agent.command".to_owned(),
            value: format!("[\"{default_command}\"]"),
        })
        .build()
}

/// Der Befund, wenn das Kommando auf dem Host liegt und die Sandbox es nicht
/// erreicht (`AGENT_004`).
///
/// Genannt werden beide Seiten: was auf dem Host liegt, was die Sandbox
/// einhängt und welcher Suchpfad drinnen gilt. Ohne den Suchpfad stünde dort
/// ein Pfad des Hosts und keine Antwort auf die Frage, die der Befund stellt
/// (HUM-139).
fn not_reachable_in_sandbox(ctx: &AgentContext, binary: &Path, docs: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_004, Severity::Blocking)
        .why(native_unreachable_why(ctx, binary))
        // Der Name am Ziel ist der des Binaries und nicht der
        // des Adapters: Ein `agent.command = ["mycode"]` würde
        // sonst zu `install … /usr/local/bin/opencode`, und der
        // Vorschlag benennte die Datei beim Kopieren um.
        .fix(FixAction::CopyCommand(install_fix(binary)))
        .docs(docs)
        .build()
}

/// `AGENT_004` for an adapter whose program is a script (HUM-274).
///
/// Copying the entry script of an npm package to `/usr/local/bin` leaves its
/// package files behind, and the start ends in `ERR_MODULE_NOT_FOUND`. The
/// fix is the adapter's installer, which puts the whole package under
/// `/usr/local`.
fn script_not_reachable_in_sandbox(
    ctx: &AgentContext,
    binary: &Path,
    adapter: &dyn AgentAdapter,
) -> Diagnostic {
    Diagnostic::builder(AGENT_004, Severity::Blocking)
        .why(format!(
            "{} is on this machine, but the sandbox mounts only {} and its PATH is {}, so the \
         command is not there and the exec would fail after the start. {} is a script that \
         needs its package files beside it; copying the one file does not help. Install it \
         under /usr/local, which the sandbox mounts: {}",
            binary.display(),
            mounts_of(ctx),
            ctx.sandbox_path_display(),
            adapter.program(),
            adapter.install_command()
        ))
        .fix(FixAction::CopyCommand(adapter.install_command().to_owned()))
        .docs(adapter.docs_url())
        .build()
}

/// Whether this command is the adapter's own program, by its file name
/// (HUM-274): only then is the adapter's installer a fix.
fn starts_own_program(named: &OsStr, adapter: &dyn AgentAdapter) -> bool {
    Path::new(named).file_name() == Some(OsStr::new(adapter.program()))
}

/// The sentence of `AGENT_004` for a native program.
fn native_unreachable_why(ctx: &AgentContext, binary: &Path) -> String {
    format!(
        "{} is on this machine, but the sandbox mounts only {} and its PATH is {}, so the \
             command is not there and the exec would fail after the start. Put the binary under \
             one of those paths, or add its directory to `[mounts].extra_ro` of the sandbox \
             profile **and** to the PATH the sandbox uses (`[env].PATH` or `sandbox.env.PATH`) \
             — mounting alone does not put it on the search path.",
        binary.display(),
        mounts_of(ctx),
        ctx.sandbox_path_display()
    )
}

/// Der Befehl, der ein Binary an eine Stelle legt, die die Sandbox einhängt.
///
/// Quelle und Ziel sind Pfade und werden aus ihren Bytes zitiert
/// (HUM-215): Ein Name mit Leerzeichen oder einem Byte, das kein UTF-8 ist,
/// nennte sonst eine andere Datei. Das Ziel entsteht deshalb als Pfad und
/// nicht als Text.
pub(crate) fn install_fix(binary: &Path) -> String {
    let destination =
        Path::new("/usr/local/bin").join(binary.file_name().unwrap_or(OsStr::new(DEFAULT_COMMAND)));
    format!(
        "sudo install -m 0755 {} {}",
        shell_path(binary),
        shell_path(&destination)
    )
}

/// Der Befehl, der ein Binary ausführbar macht, mit dem Pfad aus seinen Bytes.
fn chmod_fix(binary: &Path) -> String {
    format!("chmod +x {}", shell_path(binary))
}

/// Der Befund, wenn die Datei im Suchpfad der Sandbox liegt und nicht startet
/// (`AGENT_002`).
///
/// Genannt wird der aufgelöste Pfad auf dem Host und der Suchpfad, über den
/// die Sandbox dorthin kommt. Ohne diesen Zweig stünde hier `AGENT_001`, „nicht
/// gefunden" — eine Aussage über eine Datei, die sehr wohl da ist (HUM-139).
fn not_executable_in_sandbox(ctx: &AgentContext, binary: &Path, docs: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_002, Severity::Warning)
        .why(format!(
            "{} lies in the sandbox PATH {}, and this user cannot execute it; the exec would \
             fail after the start",
            binary.display(),
            ctx.sandbox_path_display()
        ))
        .fix(FixAction::CopyCommand(chmod_fix(binary)))
        .docs(docs)
        .build()
}

/// Die Einhängungen der Sandbox als Text, für die Befunde.
fn mounts_of(ctx: &AgentContext) -> String {
    ctx.sandbox_view
        .mounts
        .iter()
        .map(|mount| mount.dst.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Der Befund, wenn das Verzeichnis eingehängt ist und der Suchpfad der
/// Sandbox es nicht nennt (`AGENT_004`).
///
/// Entscheidbar und deshalb kein Fall für das Schweigen: Der Suchpfad ist
/// bekannt, die Einhängungen sind bekannt, und die Suche darin war
/// vollständig. Drinnen sucht `execvp` nur in diesem Pfad, und der Start endet
/// mit 127. Genau hierher läuft, wer dem Vorschlag von
/// [`not_reachable_in_sandbox`] folgt und nur `[mounts].extra_ro` ergänzt
/// (HUM-139).
fn not_on_the_sandbox_path(ctx: &AgentContext, binary: &Path, docs: &str) -> Diagnostic {
    let dir = binary.parent().unwrap_or(Path::new("/")).display();
    Diagnostic::builder(AGENT_004, Severity::Blocking)
        .why(format!(
            "{} is on this machine and the sandbox mounts {}, but its PATH is {} and does not \
             name {dir}; inside, execvp searches only that PATH, so the exec would fail after \
             the start.",
            binary.display(),
            mounts_of(ctx),
            ctx.sandbox_path_display()
        ))
        // Der nackte Pfad, nicht die erklärte Fassung: Der Wert wird kopiert
        // und eingetragen, und ein Satz in Klammern gehört nicht in die
        // Konfiguration des Nutzers.
        .fix(FixAction::ChangeSetting {
            key: "sandbox.env.PATH".to_owned(),
            value: format!("{dir}:{}", ctx.sandbox_path_value()),
        })
        .docs(docs)
        .build()
}

/// The sentence of `AGENT_001`: where the daemon looked and what it passed
/// over (HUM-278).
///
/// The per-user directories are named because they are what a user checks
/// first; a candidate that exists and was not taken is named with its reason,
/// so "not found" never stands next to a file the user can see.
///
/// The program and its installer come from the adapter (HUM-274), so a
/// finding about pi names pi.
fn not_found_why(
    ctx: &AgentContext,
    path: Option<&OsStr>,
    program: &str,
    install: &str,
    native: bool,
) -> String {
    let search = locate::host_search(ctx, OsStr::new(program));
    // The reasons of HUM-278 advise the native binary; a script adapter has
    // none, and its own sentence below says where it must lie (HUM-274).
    let skipped = if native {
        search.skipped_display()
    } else {
        String::new()
    };
    let searched = if search.searched.is_empty() {
        format!(
            "PATH={}",
            path.map(OsStr::to_string_lossy).unwrap_or_default()
        )
    } else {
        search.searched_display()
    };
    // A script is never bound (HUM-278): for it only a place under the
    // sandbox's mounts helps, which the installer provides (HUM-274).
    let advice = if native {
        format!(
            "If it is installed elsewhere, set agent.command to the absolute path of its binary; \
             if it is not installed: {install}"
        )
    } else {
        format!(
            "{program} is a script that the sandbox only finds under one of its mounts, such as \
             /usr/local: {install}"
        )
    };
    format!(
        "{program} is not in {searched} and agent.command is not set{}; the sandbox \
         searches PATH={}. {advice}",
        if skipped.is_empty() {
            String::new()
        } else {
            format!(" ({skipped})")
        },
        ctx.sandbox_path_display()
    )
}

/// Why the daemon does not bind the command it found on the host (HUM-278),
/// for `AGENT_002` and `AGENT_004`; empty when nothing was passed over.
fn passed_over(ctx: &AgentContext, named: &OsStr) -> String {
    if ctx.agent_command_from_caller {
        return String::new();
    }
    let path = Path::new(named);
    if path.is_absolute() {
        let trust = ctx.trust.unwrap_or_else(locate::Trust::of_this_process);
        return match locate::check_program(path, &trust) {
            Err(skip) if path.exists() => {
                format!("{} was passed over: {}", path.display(), skip.describe())
            }
            _ => String::new(),
        };
    }
    if path.components().count() != 1 {
        return String::new();
    }
    locate::host_search(ctx, named).skipped_display()
}

/// Die Vorprüfung des Kommandos: Ist der Agent dort, wo er startet?
///
/// Gesucht wird zuerst im Suchpfad der Sandbox, denn dort sucht der Agent
/// selbst. Liegt das Kommando in einem Verzeichnis, das dieser Pfad nennt
/// **und** eine Nur-Lese-Einhängung deckt, ist die Frage beantwortet und es
/// gibt nichts zu melden. Erst wenn dort nichts liegt, gilt der Weg über den
/// Host: Was dort gefunden wird, aber keine Einhängung deckt, ist `AGENT_004`
/// — der Fall, den HUM-135 gebaut hat.
///
/// **Warum nicht nur über den Host.** Bis zum 2026-09-07 löste die Vorprüfung
/// ein nacktes Kommando gegen den `$PATH` des Hosts auf und fragte dann, ob
/// **diese** Datei in der Sandbox sichtbar ist. Auf dem Rechner des Nutzers lag
/// unter `~/.local/bin/opencode` ein Wrapper-Skript, das keine Einhängung
/// deckt, während das eingehängte `~/.opencode/bin` im PATH der Sandbox stand:
/// Der Start hätte funktioniert, und ein blockierender Befund verhinderte ihn
/// (HUM-139).
///
/// Shared by every adapter (HUM-274): the program, its installer and its
/// documentation come from the adapter, so a finding about pi names pi.
pub(crate) fn command_preflight(ctx: &AgentContext, adapter: &dyn AgentAdapter) -> Vec<Diagnostic> {
    let program = adapter.program();
    let docs = adapter.docs_url();
    let mut diagnostics = Vec::new();
    let path = ctx.host_path.as_deref();
    let mut resolved: Option<PathBuf> = None;

    let requested = ctx.agent_command_override.as_ref().and_then(|c| c.first());
    // Erst die Sandbox: Dort startet das Kommando, und dort sucht `execvp` es
    // über den Suchpfad des Profils und das Arbeitsverzeichnis.
    let named = requested.map_or_else(|| OsStr::new(program), OsString::as_os_str);
    let lookup = ctx.look_up_in_sandbox_path(named);
    // Vollständig gesucht und nichts gefunden: Der Suchpfad ist bekannt, die
    // Einhängungen sind bekannt, und das Kommando trägt keinen Pfad.
    let searched_the_path =
        lookup == SandboxLookup::Missing && Path::new(named).components().count() == 1;
    match lookup {
        // Erreichbar: Die Frage der Vorprüfung ist beantwortet.
        //
        // Unentscheidbar: Der Host gibt über eine Stelle des Weges keine
        // Auskunft. Dann wird ebenfalls nichts gemeldet — weder über die
        // Sandbox noch über den Host (`backlog/CONVENTIONS.md` 4.13).
        // Scheitert das `exec` doch, meldet HUM-137 es mit seinem Exit-Code.
        SandboxLookup::Startable(_) | SandboxLookup::Unknown => return diagnostics,
        // Da und ohne Ausführungsrecht. Das ist kein „nicht gefunden": Das
        // `exec` findet die Datei und endet mit 126.
        SandboxLookup::NotExecutable(binary) => {
            diagnostics.push(not_executable_in_sandbox(ctx, &binary, docs));
            return diagnostics;
        }
        SandboxLookup::Missing => {}
    }

    // Not inside, but a native program the daemon binds read-only into the
    // sandbox itself: from the daemon's PATH, a per-user bin directory, or an
    // absolute `agent.command` (HUM-278). The start will find it, so there is
    // nothing to report.
    if locate::plan_agent_program(ctx, named).is_some() {
        return diagnostics;
    }

    match requested {
        Some(command) => {
            let found = find_in_path(command, path);
            resolved.clone_from(&found);
            if !found.as_deref().is_some_and(is_executable_file) {
                diagnostics.push(not_executable(
                    command,
                    ctx.agent_command_from_caller,
                    program,
                ));
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
            let found = find_in_path(OsStr::new(program), path);
            resolved.clone_from(&found);
            match found.as_deref() {
                None => diagnostics.push(
                    Diagnostic::builder(AGENT_001, Severity::Blocking)
                        .why(not_found_why(
                            ctx,
                            path,
                            program,
                            adapter.install_command(),
                            adapter.is_native_program(),
                        ))
                        // Naming the path helps wherever a native agent
                        // lies; the install command stands in the sentence.
                        // A script is never bound (HUM-278), so for it the
                        // installer is the fix (HUM-274).
                        .fix(FixAction::CopyCommand(if adapter.is_native_program() {
                            locate::agent_command_fix(program)
                        } else {
                            adapter.install_command().to_owned()
                        }))
                        .docs(docs)
                        .build(),
                ),
                // Eine Datei, die da ist, aber nicht startet, galt bis zum
                // 2026-09-13 als gefunden: Es kam kein einziger Befund heraus,
                // und das `exec` endete drinnen mit 126.
                Some(binary) if !is_executable_file(binary) => diagnostics.push(
                    Diagnostic::builder(AGENT_002, Severity::Warning)
                        .why(format!(
                            "{} is the first {program} in PATH, and this user cannot \
                             execute it; the exec would fail after the start",
                            binary.display()
                        ))
                        .fix(FixAction::CopyCommand(chmod_fix(binary)))
                        .docs(docs)
                        .build(),
                ),
                Some(_) => {}
            }
        }
    }

    // Auf dem Host gefunden heißt nicht: in der Sandbox erreichbar. Die
    // Sandbox hängt `/usr` und was sonst in `[mounts]` steht nur lesbar
    // ein; ein Programm unter `$HOME` ist dort nicht da, und das `exec`
    // scheiterte erst nach dem Start.
    if let Some(binary) = resolved.as_deref()
        && !ctx.sandbox_view.mounts.is_empty()
    {
        if ctx.reaches_program(binary) {
            // Die Datei ist drinnen unter demselben Pfad da, und trotzdem
            // findet `execvp` sie nicht: Ihr Verzeichnis steht nicht im
            // Suchpfad der Sandbox.
            if searched_the_path {
                diagnostics.push(not_on_the_sandbox_path(ctx, binary, docs));
            }
        } else if adapter.is_native_program() || !starts_own_program(named, adapter) {
            // Another command than the adapter's own program keeps the
            // generic finding: the adapter's installer is not its fix.
            diagnostics.push(not_reachable_in_sandbox(ctx, binary, docs));
        } else {
            diagnostics.push(script_not_reachable_in_sandbox(ctx, binary, adapter));
        }
    }

    // A candidate the daemon found and will not bind is named with its
    // reason, so a wrapper script never looks like the answer (HUM-278).
    // For a script adapter such as pi the advice of `Skip::Script` (point
    // agent.command at the native binary) leads nowhere; its findings name
    // the installer instead (HUM-274).
    let note = if adapter.is_native_program() {
        passed_over(ctx, named)
    } else {
        String::new()
    };
    if !note.is_empty() {
        for diagnostic in &mut diagnostics {
            if [AGENT_002, AGENT_004].contains(&diagnostic.code) {
                diagnostic.why.push_str(" The sandbox does not get it: ");
                diagnostic.why.push_str(&note);
                diagnostic.why.push('.');
            }
        }
    }

    diagnostics
}

impl AgentAdapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn program(&self) -> &'static str {
        DEFAULT_COMMAND
    }

    fn install_command(&self) -> &'static str {
        INSTALL_COMMAND
    }

    fn docs_url(&self) -> &'static str {
        DOCS_URL
    }

    fn command(&self, ctx: &AgentContext) -> Vec<OsString> {
        ctx.agent_command_override
            .clone()
            .filter(|command| !command.is_empty())
            .unwrap_or_else(|| vec![OsString::from(DEFAULT_COMMAND)])
    }

    fn on_shared_loopback(&self, command: Vec<OsString>) -> Vec<OsString> {
        let mut wrapped = vec![
            OsString::from("/bin/sh"),
            OsString::from("-c"),
            OsString::from(SHARED_LOOPBACK_START),
            // `sh -c` nimmt das erste Argument als `$0`.
            OsString::from(DEFAULT_COMMAND),
        ];
        wrapped.extend(command);
        wrapped
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
        .chain(
            bash_default_timeout_ms(&ctx.hold)
                .map(|ms| (ENV_BASH_DEFAULT_TIMEOUT_MS.to_owned(), ms.to_string())),
        )
        .collect()
    }

    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        let base = ctx.llm.endpoint.as_ref().map_or_else(
            || format!("http://127.0.0.1:{}/v1", ctx.proxy_port),
            llm_endpoint::openai_base_url,
        );
        let key_file = ctx.llm_key.is_some().then_some(API_KEY_DST);
        let config = render_config(&base, &ctx.llm.models, key_file)?;
        let models = render_models(&ctx.llm.models)?;
        let tui = render_tui()?;

        let config_home = ctx.config_home();
        let mut files = vec![
            SandboxFile::read_only(CONFIG_DST, config.clone().into_bytes()),
            SandboxFile::read_only(MODELS_DST, models.into_bytes()),
            SandboxFile::read_only(config_dst(&config_home), config.clone().into_bytes()),
            SandboxFile::read_only(keep_dst(&config_home), Vec::new()),
            SandboxFile::read_only(MANAGED_CONFIG_DST, config.into_bytes()),
            SandboxFile::read_only(tui_dst(&config_home), tui.into_bytes()),
        ];
        // Der Schlüssel als eigene Datei, nie in der Konfiguration selbst: Die
        // steht dreimal in der Sandbox und ist für den Menschen lesbar gedacht.
        if let Some(dst) = key_file {
            files.push(SandboxFile::secret(dst, ctx.llm_key.file_content()));
        }
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
            let llm_host = passthrough_authority(&ctx.llm);
            let text = briefing::render(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                llm_host.as_deref(),
            )?;
            files.push(SandboxFile::read_only(
                briefing_dst(&config_home),
                text.into_bytes(),
            ));
            // Der Skill gehört zum Briefing (HUM-364): das Briefing verweist
            // auf ihn, und wer die Einweisung abschaltet, will auch keinen
            // zweiten Text von Humanitl im Agenten.
            let skill = skill::render(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                llm_host.as_deref(),
            )?;
            files.push(SandboxFile::read_only(
                skill_dst(&config_home),
                skill.into_bytes(),
            ));
        }
        Ok(files)
    }

    fn default_rules(&self) -> &'static str {
        DEFAULT_RULES
    }

    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule> {
        passthrough_rule(llm)
    }

    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic> {
        let mut diagnostics = command_preflight(ctx, self);
        diagnostics.extend(placeholder_model_finding(&ctx.llm));
        diagnostics
    }

    fn is_fullscreen_tui(&self) -> bool {
        true
    }
}

/// The finding about an empty `llm.models` (`LLM_004`), shared by every
/// adapter that falls back to [`PLACEHOLDER_MODEL`] (HUM-274).
#[must_use]
pub(crate) fn placeholder_model_finding(llm: &LlmConfig) -> Option<Diagnostic> {
    if effective_models(&llm.models) != vec![PLACEHOLDER_MODEL.to_owned()] {
        return None;
    }
    // Der Vorschlag ist der Weg zu den Namen, nicht ein erfundener
    // Name: welche Modelle der Server anbietet, weiß nur er. Sobald
    // die Probe aus HUM-039 steht, füllt sie `llm.models` selbst.
    let fix = llm.endpoint.as_ref().map_or_else(
        || FixAction::ChangeSetting {
            key: "llm.endpoint".to_owned(),
            value: EXAMPLE_ENDPOINT.to_owned(),
        },
        |endpoint| {
            // Ohne Zugangsdaten aus `llm.endpoint` und als ein Wort
            // der Shell (HUM-261).
            let base = llm_endpoint::openai_base_url(&llm_endpoint::without_secrets(endpoint));
            FixAction::CopyCommand(format!(
                "curl -sS {}",
                humanitl_core::shell::shell_word(&format!("{base}/models"))
            ))
        },
    );
    Some(
        Diagnostic::builder(LLM_004, Severity::Warning)
            .why(format!(
                "llm.models is empty; the agent is configured with the placeholder model \
                 {PROVIDER_ID}/{PLACEHOLDER_MODEL}, and whether the server knows a model \
                 of that name is unknown. Put the ids the server reports into llm.models."
            ))
            .fix(fix)
            .build(),
    )
}

/// The passthrough rule for the LLM endpoint of the user (HUM-039).
///
/// Shared by every adapter that talks to `llm.endpoint` through the
/// OpenAI-compatible surface (HUM-274): the rule depends on the endpoint and
/// on `llm.passthrough_paths`, not on the agent, and it keeps its fixed id.
#[must_use]
pub(crate) fn passthrough_rule(llm: &LlmConfig) -> Option<Rule> {
    let url = llm.endpoint.as_ref()?;
    let (host, scheme, port) = passthrough_target(llm)?;

    let matcher = Matcher::host(HostPattern::Exact(host))
        // `GET` ist dabei, weil OpenCode die Modellliste über
        // `GET /v1/models` holt.
        .with_methods(vec![Method::POST, Method::GET])
        .with_path_prefixes(passthrough_prefixes(url, &llm.passthrough_paths))
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
        // Ohne Benutzer, Passwort und Query: Die Notiz steht in der
        // Oberfläche und in `rules.yaml` (HUM-261).
        .with_note(format!(
            "LLM passthrough for {}. Logged, never held.",
            llm_endpoint::display(url)
        )),
    )
}

/// Die Dateien, die der Adapter anlegt, als Pfade.
///
/// Für die Anzeige und für Tests; die Reihenfolge ist die von
/// [`AgentAdapter::files`]. `briefing` ist `agent.briefing.enabled`: steht es
/// aus, fehlen die Instruktionsdatei und der Skill (HUM-364) auch hier.
#[must_use]
pub fn file_targets(config_home: &Path, briefing: bool) -> Vec<PathBuf> {
    let mut targets = vec![
        PathBuf::from(CONFIG_DST),
        PathBuf::from(MODELS_DST),
        config_dst(config_home),
        keep_dst(config_home),
        PathBuf::from(MANAGED_CONFIG_DST),
        tui_dst(config_home),
    ];
    if briefing {
        targets.push(briefing_dst(config_home));
        targets.push(skill_dst(config_home));
    }
    targets
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use url::Url;

    use super::{
        OLLAMA_INFERENCE_PATHS, OPENAI_INFERENCE_PATHS, chmod_fix, install_fix,
        passthrough_prefixes as prefixes_for,
    };

    /// Ein Endpunkt an der Wurzel des Servers, wie bis HUM-261 jeder Test.
    fn root() -> Url {
        Url::parse("http://192.168.1.50:11434").unwrap()
    }

    /// Die Durchreiche für einen Endpunkt an der Wurzel.
    fn passthrough_prefixes(prefixes: &[String]) -> Vec<String> {
        prefixes_for(&root(), prefixes)
    }

    /// Die Vorschläge nennen das Binary aus seinen Bytes (HUM-215): Ein Name
    /// mit Leerzeichen bleibt ein Wort, und ein Byte, das kein UTF-8 ist,
    /// bleibt dieses Byte statt `U+FFFD`.
    #[test]
    fn on_shared_loopback_the_program_reaches_the_script_as_first_argument() {
        use std::ffi::OsString;

        use crate::agent::AgentAdapter as _;
        use crate::agent::opencode::{DEFAULT_COMMAND, OpenCodeAdapter, SHARED_LOOPBACK_START};

        let wrapped = OpenCodeAdapter.on_shared_loopback(vec![
            OsString::from("/run/agent/opencode"),
            OsString::from("--x"),
        ]);
        assert_eq!(&wrapped[..2], ["/bin/sh", "-c"]);
        assert_eq!(wrapped[3], DEFAULT_COMMAND);
        assert_eq!(wrapped[4], "/run/agent/opencode");
        assert_eq!(wrapped[5], "--x");
        assert!(!SHARED_LOOPBACK_START.contains("49374"));
        assert_eq!(
            crate::agent::opencode::shown_program(&wrapped).unwrap(),
            "/run/agent/opencode"
        );
    }

    #[test]
    fn the_fixes_quote_the_binary_from_its_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;
        use std::path::Path;

        let spaced = Path::new("/opt/My Tools/opencode");
        assert_eq!(chmod_fix(spaced), "chmod +x '/opt/My Tools/opencode'");
        assert_eq!(
            install_fix(spaced),
            "sudo install -m 0755 '/opt/My Tools/opencode' /usr/local/bin/opencode"
        );

        let bad = Path::new(OsStr::from_bytes(b"/opt/bin/my code\xff"));
        assert_eq!(chmod_fix(bad), r"chmod +x $'/opt/bin/my\x20code\xff'");
        assert_eq!(
            install_fix(bad),
            r"sudo install -m 0755 $'/opt/bin/my\x20code\xff' $'/usr/local/bin/my\x20code\xff'"
        );
    }

    /// Die Vorgabe, so wie sie aus `llm.passthrough_paths` entsteht: jeder
    /// Endpunkt als genauer Eintrag (HUM-261).
    fn default_prefixes() -> Vec<String> {
        exact(OPENAI_INFERENCE_PATHS.iter().chain(OLLAMA_INFERENCE_PATHS))
    }

    /// Die genauen Einträge zu diesen Pfaden.
    fn exact<'a>(paths: impl IntoIterator<Item = &'a &'a str>) -> Vec<String> {
        paths
            .into_iter()
            .map(|path| humanitl_core::exact_path_entry(path))
            .collect()
    }

    /// Wahr, wenn die Einträge den Pfad treffen, mit dem Vergleich der Engine.
    fn covers(entries: &[String], path: &str) -> bool {
        humanitl_rules::path::prefix_matches(entries, path)
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
        // Unter einem erlaubten Endpunkt, aber nicht er selbst (HUM-261).
        "/v1/models/delete",
        "/v1/models/m",
        "/v1/chat/completions/x",
        "/api/tags/x",
    ];

    /// Ein Endpunkt hinter einem Pfad-Präfix (HUM-261): Die Inferenz liegt
    /// unter seiner Basis, dorthin schickt der Agent; die Ollama-Endpunkte an
    /// der Wurzel des Hosts fallen weg, und die Wurzel-Form `/v1/…` gilt auch
    /// nicht, weil der Agent sie nie fragt.
    #[test]
    fn behind_a_path_prefix_inference_lies_under_the_base() {
        let defaults = ["/v1/".to_owned(), "/api/".to_owned()];
        for endpoint in [
            "https://llm.example.net/api/v1/",
            "https://llm.example.net/api/v1",
            "https://llm.example.net/api",
        ] {
            let endpoint = Url::parse(endpoint).unwrap();
            let expected = exact(&[
                "/api/v1/chat/completions",
                "/api/v1/completions",
                "/api/v1/responses",
                "/api/v1/embeddings",
                "/api/v1/models",
            ]);
            assert_eq!(
                prefixes_for(&endpoint, &defaults),
                expected,
                "for {endpoint}"
            );
            assert_eq!(
                prefixes_for(&endpoint, &[]),
                expected,
                "the fallback follows the same rule, for {endpoint}"
            );
            assert_eq!(
                prefixes_for(&endpoint, &["/api/".to_owned()]),
                expected,
                "the Ollama surface alone gives nothing behind a prefix and falls back, \
                 for {endpoint}"
            );
        }
        // Ein abschließendes `/v1` ist kein Präfix: dieselbe Regel wie ohne.
        assert_eq!(
            prefixes_for(
                &Url::parse("http://192.168.1.50:11434/v1").unwrap(),
                &defaults
            ),
            passthrough_prefixes(&defaults)
        );
    }

    /// Beide Flächen-Präfixe der Vorgabe werden zu Endpunkten.
    #[test]
    fn both_surface_prefixes_are_narrowed_to_inference_endpoints() {
        let prefixes = passthrough_prefixes(&["/v1/".to_owned(), "/api/".to_owned()]);
        assert_eq!(prefixes, default_prefixes());
        assert!(
            covers(&prefixes, "/v1/chat/completions?x=1"),
            "inference stays: {prefixes:?}"
        );
        assert!(
            covers(&prefixes, "/api/chat"),
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
                    !covers(&prefixes, path),
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
            assert_eq!(prefixes, exact(expected), "for {written}");
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

    /// Ein Eintrag des Menschen mit `?` hat nie etwas getroffen und wird auch
    /// nicht zum genauen Eintrag (HUM-261); genau sind nur die erzeugten.
    #[test]
    fn a_written_entry_with_a_question_mark_is_dropped() {
        assert_eq!(
            passthrough_prefixes(&["/v1/files?".to_owned(), "/v1/files".to_owned()]),
            vec!["/v1/files".to_owned()]
        );
        assert_eq!(
            passthrough_prefixes(&["/x?".to_owned()]),
            default_prefixes(),
            "nothing usable is left, so the fallback applies"
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
