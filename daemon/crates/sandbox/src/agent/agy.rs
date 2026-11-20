//! The adapter for the Antigravity CLI `agy` of Google (HUM-517).
//!
//! Every statement below comes from the vendor's documentation at
//! `antigravity.google/docs`, from its install script, or from the Gemini
//! API reference, all read on 2026-11-20; none is measured on the vendor's
//! binary, because the real `agy` is not run in Humanitl's tests. Where the
//! documentation is silent, this says so; the HUM-517 status names each gap
//! and HUM-553 point 5 what to measure.
//!
//! **Program.** The documented install is `curl -fsSL
//! https://antigravity.google/cli/install.sh | bash`, "Default binary
//! location: `~/.local/bin/agy`"
//! (<https://antigravity.google/docs/getting-started?tab=cli>). The script
//! downloads one "flat native build" per platform from a manifest and checks
//! its SHA-512 from the same manifest
//! (<https://antigravity.google/cli/install.sh>): one native executable, so
//! the daemon binds it from `~/.local/bin` (HUM-278), and
//! [`AgentAdapter::is_native_program`] stays true.
//!
//! **Network and CA.** The documentation names no proxy variable and no CA
//! setting for the CLI's own traffic; `sandboxProxy` with `certFile` in
//! `settings.json` applies "only while the terminal sandbox is on", to the
//! commands the agent runs (<https://antigravity.google/docs/settings>,
//! <https://antigravity.google/docs/cli/reference>). The install script calls
//! the binary a "Go-Native" build; Go's `net/http` takes `HTTPS_PROXY` and
//! `NO_PROXY` from the environment (<https://pkg.go.dev/net/http#ProxyFromEnvironment>)
//! and `crypto/x509` reads [`ENV_SSL_CERT_FILE`] on Unix
//! (<https://pkg.go.dev/crypto/x509#SystemCertPool>). That `agy` uses both is
//! an inference, not a documented fact: the env kit sets the proxy for every
//! agent, the adapter points [`ENV_SSL_CERT_FILE`] at the sandbox's CA, and
//! a direct connection fails closed anyway, because the sandbox has no
//! route but the proxy.
//!
//! **State and login.** `agy` keeps its settings in
//! `~/.gemini/antigravity-cli/settings.json` and its tokens "in the OS-native
//! keyring … Linux Secret Service/dbus" (<https://antigravity.google/docs/install?tab=cli>);
//! no variable moves `~/.gemini`. The host's home and its session bus never
//! reach a sandbox: `HOME` is the sandbox's own tmpfs home, and no
//! `DBUS_SESSION_BUS_ADDRESS` and no bus socket enter (`--clearenv`, and the
//! shim refuses every Unix socket but the bridge's). The adapter therefore
//! supports a login only through a Gemini API key: with a key
//! (`llm.api_key_ref`) the settings carry `"modelProvider": "gemini"`, and
//! [`ENV_API_KEY`] "is read only from the environment" (install page). The
//! key travels as a sandbox file from a memfd ([`API_KEY_DST`]), and the
//! [`AgentAdapter::launcher`] moves it into the environment inside the
//! sandbox, as for Claude Code (HUM-514): it is in no `argv` and in no
//! variable of the plan. Without a key `agy` falls back to a sign-in with a
//! Google account, which finds no keyring in the sandbox; the
//! documentation names no file fallback.
//!
//! **Settings.** The CLI writes its settings itself ("The CLI adds these
//! entries for you" for `trustedWorkspaces`, settings page; the first start
//! asks for colour scheme, rendering and workspace trust, getting-started
//! page), so a read-only file would break the first start. The adapter puts
//! its seed at [`SETTINGS_SEED_DST`], read-only, and the launcher writes it
//! to [`settings_dst`] before every start of the agent: `modelProvider` with
//! a key, `model` with a gateway and a model, `showFeedbackSurvey = false`.
//! Whatever the agent changes there lives in the session's tmpfs and ends
//! with it.
//!
//! **Side traffic.** [`ENV_DISABLE_AUTO_UPDATE`] = `true` turns off the
//! "native, statically linked self-updater that runs in the background"
//! (<https://antigravity.google/docs/cli/troubleshooting>). Telemetry has no
//! setting of its own in `settings.json`: "Enable Telemetry" in `/config` is
//! "saved with your account" (cli/reference), and with an API key there is
//! no account; whatever `agy` still sends is held like any request.
//!
//! **Models and hosts.** With an API key "model requests go directly to the
//! Gemini API" (install page): `https://generativelanguage.googleapis.com/{version}/…`,
//! the key in the header `x-goog-api-key`, `POST
//! /v1beta/models/{model}:generateContent` and `:streamGenerateContent`,
//! `GET /v1beta/models` (<https://aistudio.google.com/api-reference>). The
//! documentation does not say which API version `agy` uses, so the
//! passthrough covers both, `/v1beta` and `/v1`, at that one origin, as two
//! rules (HUM-568, [`passthrough`]): `POST` only on
//! `models/{model}:generateContent` and `:streamGenerateContent`
//! ([`GENERATE_METHODS`], a model id of letters, digits, `.`, `_` and `-`),
//! `GET` only on the model list `models` itself. Getting one model, counting
//! tokens, embedding, files, cached contents and every other path are held. [`ENV_BASE_URL`] "sends model
//! requests to a custom Gemini-compatible endpoint" (install page); the
//! adapter sets it for `llm.endpoint` and keeps the same paths below the
//! endpoint's prefix. The sign-in with a Google account reaches
//! `cloudcode-pa.googleapis.com` ("the client connects to the global
//! endpoints … during sign-in, to find licenses and load the model
//! catalog", <https://antigravity.google/docs/enterprise>); that host is
//! what the cloud notice names without a key, and it is held, never passed.
//!
//! **`agy`'s own sandbox.** Under Linux it uses "Kernel namespaces"
//! (<https://antigravity.google/docs/sandbox>), and on "Linux hosts and some
//! containers that block unprivileged user namespaces" `enableTerminalSandbox`
//! and `--sandbox` "have no effect" and auto mode is not offered. Humanitl
//! starts every session with `--disable-userns` (`crate::bwrap_args`), so
//! `agy`'s sandbox cannot start inside it, measured as for Codex in the
//! real-sandbox test (`nested_userns=refused`); the briefing says that the
//! outer boundary is Humanitl ([`OUTER_BOUNDARY_EN`]).
//!
//! **The briefing.** The CLI reads global rules from `~/.gemini/GEMINI.md`
//! among other places (<https://antigravity.google/docs/rules>) and skills
//! from `~/.gemini/antigravity-cli/skills/<skill>/`, which "the App doesn't
//! read" (<https://antigravity.google/docs/skills>). Both lie on the
//! sandbox's own home; the documentation names no system-wide place.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use humanitl_config::{Language, LlmConfig, llm_endpoint};
use humanitl_core::rule::{PathPattern, Rule};
use humanitl_core::{Diagnostic, HostName, Method, Scheme};
use url::Url;

use crate::agent::opencode::command_preflight;
use crate::agent::opencode_models::{DEFAULT_RULES, broken};
use crate::agent::passthrough::{self, Route, Target};
use crate::agent::{AgentAdapter, AgentContext, SandboxFile, briefing, skill};

/// The id of the adapter as it stands in `agent.adapter`.
pub const ADAPTER_ID: &str = "agy";

/// The command started without `agent.command`.
pub const DEFAULT_COMMAND: &str = "agy";

/// The directory of the adapter's own files in the sandbox.
pub const AGY_DIR: &str = "/etc/humanitl/agy";

/// The access key of the model in the sandbox (HUM-140), a file from a memfd
/// that only the [`AgentAdapter::launcher`] reads.
pub const API_KEY_DST: &str = "/etc/humanitl/agy/llm-api-key";

/// The seed of `settings.json`, read-only; the launcher copies it to
/// [`settings_dst`] (module documentation, "Settings").
pub const SETTINGS_SEED_DST: &str = "/etc/humanitl/agy/settings.json";

/// The documented install (antigravity.google/docs/getting-started, tab
/// CLI): the binary lands in `~/.local/bin`, a per-user directory the daemon
/// binds the single binary from (HUM-278).
pub const INSTALL_COMMAND: &str = "curl -fsSL https://antigravity.google/cli/install.sh | bash";

/// The documentation of the CLI; stands in `docs` of its findings.
pub const DOCS_URL: &str = "https://antigravity.google/docs/install?tab=cli";

/// The Gemini API key, read only from the environment (install page).
/// Never in the plan's environment: the [`KEY_START`] launcher sets it
/// inside the sandbox.
pub const ENV_API_KEY: &str = "GEMINI_API_KEY";

/// A Gemini-compatible endpoint instead of the Gemini API (install page),
/// set only for `llm.endpoint`.
pub const ENV_BASE_URL: &str = "GOOGLE_GEMINI_BASE_URL";

/// Turns off the background self-updater (cli/troubleshooting).
pub const ENV_DISABLE_AUTO_UPDATE: &str = "AGY_CLI_DISABLE_AUTO_UPDATE";

/// The CA bundle Go's `crypto/x509` reads on Unix (module documentation,
/// "Network and CA").
pub const ENV_SSL_CERT_FILE: &str = "SSL_CERT_FILE";

/// The variables of the adapter whose values the person may see; none of
/// them carries a secret. [`ENV_API_KEY`] is never in the plan.
pub const VISIBLE_ENV: &[&str] = &[ENV_DISABLE_AUTO_UPDATE, ENV_BASE_URL, ENV_SSL_CERT_FILE];

/// The value of `modelProvider` for an API key, "the only accepted value"
/// (install page).
pub const MODEL_PROVIDER: &str = "gemini";

/// The Gemini API, where an API key sends its model requests
/// (aistudio.google.com/api-reference).
pub const GEMINI_API: &str = "https://generativelanguage.googleapis.com";

/// The host of [`GEMINI_API`].
pub const GEMINI_API_HOST: &str = "generativelanguage.googleapis.com";

/// The global endpoint of the sign-in with a Google account
/// (antigravity.google/docs/enterprise); named by the cloud notice without
/// a key, never passed through.
pub const ACCOUNT_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com";

/// The header that carries the key (aistudio.google.com/api-reference).
pub const KEY_HEADER: &str = "x-goog-api-key";

/// The model list below the base in both API versions, passed for `GET`
/// exactly (module documentation, "Models and hosts").
pub const MODEL_PATHS: &[&str] = &["/v1beta/models", "/v1/models"];

/// The methods on a model (`models/{model}:<method>`) the passthrough passes
/// for `POST` (aistudio.google.com/api-reference).
pub const GENERATE_METHODS: &[&str] = &["generateContent", "streamGenerateContent"];

/// A model id in the path of a generate call; anything else, an encoded
/// `/` among it, is held.
const MODEL_ID: &str = "[A-Za-z0-9._-]+";

/// The launcher: writes the settings seed into the agent's settings, moves
/// the key, if there is one, from its file into [`ENV_API_KEY`] with the
/// shell's built-in `read`, so no process carries it in its `argv`, and
/// replaces itself with the agent. As Claude Code's (HUM-514): `sh -c` takes
/// the word after the script as `$0` and the rest as `$@`; the key file has
/// no newline at its end, so `read` answers 1 with the variable set.
pub const KEY_START: &str = r#"mkdir -p "$HOME/.gemini/antigravity-cli" && cat /etc/humanitl/agy/settings.json > "$HOME/.gemini/antigravity-cli/settings.json" || { echo "humanitl: the settings of agy cannot be written" >&2; exit 78; }
if [ -e /etc/humanitl/agy/llm-api-key ]; then
IFS= read -r GEMINI_API_KEY < /etc/humanitl/agy/llm-api-key || [ -n "$GEMINI_API_KEY" ] || { echo "humanitl: the access key file /etc/humanitl/agy/llm-api-key is empty" >&2; exit 78; }
export GEMINI_API_KEY
fi
exec "$@"
"#;

/// The sentence the briefing gets for `agy` (HUM-517 point 2).
pub const OUTER_BOUNDARY_EN: &str = "Antigravity CLI: your own terminal sandbox cannot start \
inside this one, which refuses user namespaces, so its sandbox settings have no effect; the \
outer boundary is this sandbox, and its network rules hold for every command.";

/// The German form of [`OUTER_BOUNDARY_EN`].
pub const OUTER_BOUNDARY_DE: &str = "Antigravity CLI: Deine eigene Terminal-Sandbox kann in \
dieser nicht starten, weil diese Nutzer-Namensräume verweigert; ihre Einstellungen wirken \
deshalb nicht. Die äußere Grenze ist diese Sandbox, und ihre Netzregeln gelten für jeden \
Befehl.";

/// `~/.gemini` under a home, where `agy` keeps settings, rules and skills.
#[must_use]
pub fn gemini_dir(home: &Path) -> PathBuf {
    home.join(".gemini")
}

/// Where the launcher writes the settings: the documented
/// `~/.gemini/antigravity-cli/settings.json`.
#[must_use]
pub fn settings_dst(home: &Path) -> PathBuf {
    gemini_dir(home)
        .join("antigravity-cli")
        .join("settings.json")
}

/// Where the briefing lies: the global rules `~/.gemini/GEMINI.md`.
#[must_use]
pub fn briefing_dst(home: &Path) -> PathBuf {
    gemini_dir(home).join("GEMINI.md")
}

/// Where the skill `humanitl` (HUM-364) lies: the CLI's own skill place.
#[must_use]
pub fn skill_dst(home: &Path) -> PathBuf {
    gemini_dir(home)
        .join("antigravity-cli")
        .join(skill::SKILLS_DIR_NAME)
        .join(skill::SKILL_NAME)
        .join(skill::SKILL_FILE_NAME)
}

/// The files the adapter creates, as paths, without the key file: the
/// settings seed, and with the briefing its file and the skill.
#[must_use]
pub fn file_targets(home: &Path, briefing: bool) -> Vec<PathBuf> {
    let mut targets = vec![PathBuf::from(SETTINGS_SEED_DST)];
    if briefing {
        targets.push(briefing_dst(home));
        targets.push(skill_dst(home));
    }
    targets
}

/// Whether `agy` runs with an API key: only then it speaks to the Gemini
/// API or `llm.endpoint` (module documentation, "State and login").
#[must_use]
pub fn uses_api_key(llm: &LlmConfig) -> bool {
    llm.api_key_ref.is_some()
}

/// Whether `endpoint` is the Gemini API itself: exactly `https`, host
/// [`GEMINI_API_HOST`], port 443, the root or an API version as path, and
/// no user, password, query or fragment. Anything else is a gateway that
/// gets [`ENV_BASE_URL`], so passthrough and mask never name another target
/// than the one the key goes to.
#[must_use]
pub fn is_direct_api(endpoint: &Url) -> bool {
    let path = endpoint.path().trim_end_matches('/');
    endpoint.scheme() == "https"
        && endpoint.host_str() == Some(GEMINI_API_HOST)
        && endpoint.port_or_known_default() == Some(443)
        && (path.is_empty() || path == "/v1beta" || path == "/v1")
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.query().is_none()
        && endpoint.fragment().is_none()
}

/// Whether `llm.endpoint` names a gateway rather than the Gemini API.
#[must_use]
pub fn uses_gateway(llm: &LlmConfig) -> bool {
    llm.endpoint
        .as_ref()
        .is_some_and(|endpoint| !is_direct_api(endpoint))
}

/// The one endpoint every part of the adapter resolves to: with a key the
/// gateway in `llm.endpoint`, else [`GEMINI_API`]; without a key
/// [`ACCOUNT_ENDPOINT`]. Environment, passthrough, briefing, cloud notice
/// and the key's mask all read it.
#[must_use]
pub fn resolved_endpoint(llm: &LlmConfig) -> Option<Url> {
    if !uses_api_key(llm) {
        return Url::parse(ACCOUNT_ENDPOINT).ok();
    }
    match llm.endpoint.as_ref() {
        Some(endpoint) if uses_gateway(llm) => Some(endpoint.clone()),
        _ => Url::parse(GEMINI_API).ok(),
    }
}

/// The value of [`ENV_BASE_URL`] for a gateway: scheme, host, port and the
/// path before an API version, without user, password, query and fragment,
/// because the value stands in the environment the person sees; a gateway
/// on the loopback under the reserved name the proxy maps (HUM-387).
#[must_use]
pub fn base_url(endpoint: &Url) -> String {
    let mut base = llm_endpoint::without_secrets(endpoint);
    base.set_query(None);
    base.set_fragment(None);
    base.set_path(&path_prefix(endpoint));
    if llm_endpoint::is_loopback(&base) {
        // Fails only for an address without a host, which is no loopback.
        let _ = base.set_host(Some(humanitl_core::LLM_ALIAS_HOST));
    }
    base.as_str().trim_end_matches('/').to_owned()
}

/// The path of `endpoint` before an API version (`/v1beta`, `/v1`), without
/// a trailing slash.
#[must_use]
pub fn path_prefix(endpoint: &Url) -> String {
    let path = endpoint.path().trim_end_matches('/');
    let path = path
        .strip_suffix("/v1beta")
        .or_else(|| path.strip_suffix("/v1"))
        .unwrap_or(path);
    path.trim_end_matches('/').to_owned()
}

/// The passthrough rules of `agy`, only with a key, at the Gemini API or the
/// gateway: `POST` on the generate calls of a model ([`GENERATE_METHODS`])
/// and `GET` on exactly the model list ([`MODEL_PATHS`]), in both API
/// versions (module documentation, "Models and hosts").
#[must_use]
pub fn passthrough_rules(llm: &LlmConfig) -> Vec<Rule> {
    if !uses_api_key(llm) {
        return Vec::new();
    }
    let gateway = uses_gateway(llm);
    let Some(endpoint) = resolved_endpoint(llm) else {
        return Vec::new();
    };
    let Some(target) = passthrough_target(&endpoint, gateway) else {
        return Vec::new();
    };
    let prefix = if gateway {
        path_prefix(&endpoint)
    } else {
        String::new()
    };
    let list = MODEL_PATHS
        .iter()
        .map(|path| humanitl_core::exact_path_entry(&format!("{prefix}{path}")))
        .collect();
    // The prefixes name the two model trees; the pattern narrows them to a
    // generate call on one model. Both must hold.
    let models = MODEL_PATHS
        .iter()
        .map(|path| format!("{prefix}{path}/"))
        .collect();
    let generate = PathPattern::Regex(format!(
        "^{}/v1(?:beta)?/models/{MODEL_ID}:(?:{})$",
        passthrough::regex_literal(&prefix),
        GENERATE_METHODS.join("|")
    ));
    passthrough::rules(
        &target,
        vec![
            Route::new(&[Method::POST], models).with_pattern(generate),
            Route::new(&[Method::GET], list),
        ],
    )
}

/// Host, scheme, port and note of the passthrough to `endpoint`. A gateway
/// usually stands in the own network; the Gemini API never does.
fn passthrough_target(endpoint: &Url, gateway: bool) -> Option<Target> {
    Some(Target {
        host: HostName::parse(endpoint.host_str()?).ok()?,
        scheme: Scheme::parse(endpoint.scheme())?,
        port: endpoint.port_or_known_default()?,
        allow_private: gateway,
        note: format!(
            "LLM passthrough for the Antigravity CLI to {}. Logged, never held.",
            llm_endpoint::display(endpoint)
        ),
    })
}

/// The host of the passthrough as `host:port`, for the briefing; none
/// without a key.
#[must_use]
pub fn passthrough_authority(llm: &LlmConfig) -> Option<String> {
    if !uses_api_key(llm) {
        return None;
    }
    let endpoint = resolved_endpoint(llm)?;
    Some(format!(
        "{}:{}",
        endpoint.host_str()?,
        endpoint.port_or_known_default()?
    ))
}

/// The endpoint `agy` sends its inference to: [`resolved_endpoint`].
#[must_use]
pub fn inference_endpoint(llm: &LlmConfig) -> Option<Url> {
    resolved_endpoint(llm)
}

/// The settings seed (module documentation, "Settings"): `modelProvider`
/// with a key, `model` with a gateway and a model, the feedback survey off.
#[must_use]
pub fn settings(ctx: &AgentContext) -> serde_json::Map<String, serde_json::Value> {
    let mut settings = serde_json::Map::new();
    settings.insert("showFeedbackSurvey".into(), false.into());
    if ctx.llm_key.is_some() {
        settings.insert("modelProvider".into(), MODEL_PROVIDER.into());
        if uses_gateway(&ctx.llm)
            && let Some(model) = ctx.llm.models.first()
        {
            settings.insert("model".into(), model.clone().into());
        }
    }
    settings
}

/// The adapter for the Antigravity CLI. Stateless: everything it needs is in
/// the [`AgentContext`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AntigravityAdapter;

impl AntigravityAdapter {
    /// An adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl AgentAdapter for AntigravityAdapter {
    fn id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn command(&self, ctx: &AgentContext) -> Vec<OsString> {
        ctx.agent_command_override
            .clone()
            .filter(|command| !command.is_empty())
            .unwrap_or_else(|| vec![OsString::from(DEFAULT_COMMAND)])
    }

    /// Always: the settings must be written before every start, with or
    /// without a key.
    fn launcher(&self, _ctx: &AgentContext) -> Vec<OsString> {
        vec![
            OsString::from("/bin/sh"),
            OsString::from("-c"),
            OsString::from(KEY_START),
            // `sh -c` takes the first argument as `$0`.
            OsString::from(DEFAULT_COMMAND),
        ]
    }

    fn env(&self, ctx: &AgentContext) -> Vec<(String, String)> {
        let ca = ctx.ca_path_sandbox.to_string_lossy().into_owned();
        let home = ctx.home.to_string_lossy().into_owned();
        let config_home = ctx.config_home().to_string_lossy().into_owned();
        let mut env: Vec<(String, String)> = [
            (ENV_DISABLE_AUTO_UPDATE, "true".to_owned()),
            (ENV_SSL_CERT_FILE, ca),
            ("HOME", home.clone()),
            ("XDG_CONFIG_HOME", config_home),
            ("XDG_DATA_HOME", format!("{home}/.local/share")),
            ("XDG_CACHE_HOME", format!("{home}/.cache")),
            ("TERM", "xterm-256color".to_owned()),
            ("COLORTERM", "truecolor".to_owned()),
            ("LANG", "C.UTF-8".to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
        if uses_api_key(&ctx.llm)
            && let Some(endpoint) = ctx.llm.endpoint.as_ref().filter(|_| uses_gateway(&ctx.llm))
        {
            env.push((ENV_BASE_URL.to_owned(), base_url(endpoint)));
        }
        env
    }

    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        let mut files = Vec::new();
        if ctx.llm_key.is_some() {
            files.push(SandboxFile::secret(API_KEY_DST, ctx.llm_key.file_content()));
        }
        let seed = serde_json::to_vec_pretty(&serde_json::Value::Object(settings(ctx)))
            .map_err(|err| broken("settings.json", &err.to_string()))?;
        files.push(SandboxFile::read_only(SETTINGS_SEED_DST, seed));
        // `agent.briefing.enabled = false` leaves out briefing and skill, no
        // text of Humanitl in the agent at all (HUM-364).
        if ctx.briefing.enabled {
            let llm_host = passthrough_authority(&ctx.llm);
            let text = briefing::render_for_session(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                llm_host.as_deref(),
            )?;
            let note = match ctx.language {
                Language::En => OUTER_BOUNDARY_EN,
                Language::De => OUTER_BOUNDARY_DE,
            };
            files.push(SandboxFile::read_only(
                briefing_dst(&ctx.home),
                format!("{text}\n{note}\n").into_bytes(),
            ));
            let skill = skill::render(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                llm_host.as_deref(),
            )?;
            files.push(SandboxFile::read_only(
                skill_dst(&ctx.home),
                skill.into_bytes(),
            ));
        }
        Ok(files)
    }

    fn default_rules(&self) -> &'static str {
        DEFAULT_RULES
    }

    fn llm_passthrough(&self, llm: &LlmConfig) -> Vec<Rule> {
        passthrough_rules(llm)
    }

    fn inference_endpoint(&self, llm: &LlmConfig) -> Option<Url> {
        inference_endpoint(llm)
    }

    fn key_header(&self, _llm: &LlmConfig) -> &'static str {
        KEY_HEADER
    }

    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic> {
        command_preflight(ctx, self)
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

    fn is_fullscreen_tui(&self) -> bool {
        true
    }
}
