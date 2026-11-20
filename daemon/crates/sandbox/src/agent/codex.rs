//! The adapter for the Codex CLI of `OpenAI` (HUM-515).
//!
//! Every statement below comes from the vendor's documentation or from the
//! open-source repository `github.com/openai/codex` at commit
//! `a0a30c40e8dbcc44af52e2df64dcce64368d4422`, both read on 2026-11-20; none
//! is measured on the vendor's binary, because the real `codex` is not run in
//! Humanitl's tests. Source files are named relative to that repository.
//!
//! **Program.** The documented global install is `npm install -g
//! @openai/codex` (`README.md`, "Installing and running Codex CLI"). The
//! package's `bin/codex.js` is a Node launcher that starts the native binary
//! of a platform package beside it (`codex-cli/package.json`,
//! `codex-cli/bin/codex.js`), so the program is no single native executable
//! ([`AgentAdapter::is_native_program`] is false, as for pi): it runs from a
//! global npm install under `/usr`, which the sandbox mounts.
//!
//! **Network and CA.** Codex's HTTP clients take the proxy from
//! `HTTPS_PROXY` (then `ALL_PROXY`) and `NO_PROXY` when the platform reports
//! no system proxy, which on Linux it never does
//! (`codex-rs/http-client/src/outbound_proxy.rs`, `resolve_env_proxy_route`;
//! a client without the route-aware policy keeps reqwest's default, which
//! reads the same variables). The env kit of the launcher sets them for
//! every agent. [`ENV_CA_CERTIFICATE`] "Points to a PEM CA bundle … Takes
//! precedence over `SSL_CERT_FILE`" and applies to "HTTPS, login, and
//! WebSocket clients"
//! (<https://learn.chatgpt.com/docs/config-file/environment-variables>); the
//! bundle is added to the platform's native roots
//! (`codex-rs/http-client/src/custom_ca.rs`). The adapter points it at the
//! sandbox's CA.
//!
//! **State.** [`ENV_CODEX_HOME`] "Sets the root for Codex state, including
//! config, auth, logs, sessions, skills … If you set it, the directory must
//! already exist" (environment-variables page). It points at `<home>/.codex`
//! on the sandbox's own home, a fresh tmpfs per session; the adapter's
//! `config.toml` lies there, so the directory exists. The host's `~/.codex`
//! is never mounted. With `agents.codex.source = "depot"` the depot sets the
//! variable to the session's copy of its configuration, and that value wins
//! (`SandboxProfile::effective_env`).
//!
//! **`config.toml`** (<https://learn.chatgpt.com/docs/config-file/config-reference>):
//! `check_for_update_on_startup = false`, `analytics.enabled = false`
//! ("disables the built-in Statsig metrics exporter"), `feedback.enabled =
//! false` ("Enable feedback submission via `/feedback`"),
//! `cli_auth_credentials_store = "file"` ("file stores credentials in
//! `auth.json` under `CODEX_HOME`", <https://learn.chatgpt.com/docs/auth>,
//! never the host's keyring), [`SANDBOX_MODE`] and the project's trust
//! ([`PROJECT_TRUST`]). Under Linux the managed requirements file
//! [`REQUIREMENTS_DST`] ("System `requirements.toml` (`/etc/codex/requirements.toml`
//! on Unix systems …)",
//! <https://learn.chatgpt.com/docs/enterprise/managed-configuration>) enforces
//! the update check, the credential store and feedback for a depot session
//! too, whose `CODEX_HOME` is the depot's copy; users "can't override those
//! requirements through config.toml or CLI overrides" (auth page).
//!
//! **Codex's own sandbox** (`sandbox_mode`, [`SANDBOX_MODE`]). Its Linux
//! sandbox is bubblewrap, which "explicitly isolates the user namespace via
//! `--unshare-user`" (`codex-rs/linux-sandbox/README.md`, "Current
//! Behavior"). Humanitl's sandbox takes that from every process inside it
//! (`--disable-userns`, `crate::bwrap_args`): a nested `--unshare-user` ends
//! with "Creating new namespace failed: nesting depth or
//! /proc/sys/user/max_*_namespaces exceeded (ENOSPC)" (measured 2026-11-20
//! with bubblewrap 0.13.1 as the stand-in, HUM-515 status), and Codex then
//! only "surfaces a startup warning". `read-only` and `workspace-write`
//! would leave every command of the agent failing, and `allowed_sandbox_modes`
//! "must include 'read-only'" (`codex-rs/config/src/config_requirements.rs`),
//! so the adapter sets `danger-full-access` and the briefing says that the
//! outer boundary is Humanitl ([`OUTER_BOUNDARY_EN`]).
//!
//! **Provider and credentials.** With a key (`llm.api_key_ref`) or an own
//! endpoint (`llm.endpoint`) the adapter declares its own provider
//! [`PROVIDER_ID`] (`model_provider`, `model_providers.<id>.base_url`,
//! `.env_key`, `.wire_api = "responses"`, config reference). A custom
//! provider has `requires_openai_auth = false` by default, so "the login
//! screen is skipped, and the API key (if needed) comes from the environment
//! variable specified by `env_key`"
//! (`codex-rs/model-provider-info/src/lib.rs`), and Codex sends it as
//! `Authorization: Bearer` (`codex-rs/model-provider/src/auth.rs`,
//! `bearer_auth_for_provider`), the default header of the key's mask. It
//! also keeps the plain HTTP transport: `supports_websockets` defaults to
//! false for a custom provider, true only for the built-in `openai` one. The
//! key travels as a sandbox file from a memfd ([`API_KEY_DST`]), and the
//! [`AgentAdapter::launcher`] moves it into [`ENV_API_KEY`] inside the
//! sandbox, as for Claude Code (HUM-514): it is in no `argv` and in no
//! variable of the plan. Without key and endpoint Codex keeps its built-in
//! provider and a `ChatGPT` login, whose inference goes to
//! [`CHATGPT_CODEX_BASE_URL`] (`codex-rs/model-provider-info/src/lib.rs`,
//! `to_api_provider`).
//!
//! **What is let through.** Codex posts to `<base>/responses` and lists
//! models with `GET <base>/models` (`codex-rs/codex-api/src/lib.rs`
//! `RESPONSES_PATH`, `codex-rs/codex-api/src/endpoint/models.rs`). The
//! passthrough is two rules below the resolved base (HUM-568,
//! [`passthrough`]): `POST` on exactly [`RESPONSES_PATH`] and `GET` on
//! exactly [`MODELS_PATH`]; everything else (sign-in, plugin sync, analytics
//! a depot session did not turn off) is held like any request. The built-in
//! provider of a `ChatGPT` login also opens a WebSocket with
//! `GET <base>/responses` (`supports_websockets`,
//! `codex-rs/codex-api/src/endpoint/responses_websocket/connector.rs`,
//! `api.path = "/responses"`). A rule without an `upgrade` never matches an
//! upgrade, so neither the rule before HUM-568, which had `GET` on that
//! path, nor these two pass that WebSocket: it is held like any request, and
//! a plain `GET <base>/responses`, which no provider sends, is held too.
//!
//! **The briefing.** Under Linux it goes to the managed requirement
//! `additional_developer_instructions` ("Managed developer instructions added
//! as a separate developer message", config reference), so it holds for a
//! depot session as well, and the skill `humanitl` to the admin skill place
//! `/etc/codex/skills` (<https://learn.chatgpt.com/docs/build-skills>,
//! "Where to save skills"). Under macOS, where no session writes `/etc`
//! (as for Claude Code), the briefing is the global `AGENTS.md` in
//! `CODEX_HOME` (<https://learn.chatgpt.com/docs/agent-configuration/agents-md>)
//! and the skill a user skill under `$HOME/.agents/skills`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use humanitl_config::{Language, LlmConfig, llm_endpoint};
use humanitl_core::rule::Rule;
use humanitl_core::{Diagnostic, HostName, Method, Scheme};
use url::Url;

use crate::agent::opencode::command_preflight;
use crate::agent::opencode_models::{DEFAULT_RULES, broken};
use crate::agent::passthrough::{self, Route, Target};
use crate::agent::{AgentAdapter, AgentContext, SandboxFile, briefing, skill};

/// The id of the adapter as it stands in `agent.adapter`.
pub const ADAPTER_ID: &str = "codex";

/// The command started without `agent.command`.
pub const DEFAULT_COMMAND: &str = "codex";

/// The directory of the adapter's own files in the sandbox.
pub const CODEX_DIR: &str = "/etc/humanitl/codex";

/// The access key of the model in the sandbox (HUM-140), a file from a memfd
/// that only the [`AgentAdapter::launcher`] reads.
pub const API_KEY_DST: &str = "/etc/humanitl/codex/llm-api-key";

/// The documented global install (`github.com/openai/codex`, `README.md`):
/// the npm package lands under `/usr`, which the sandbox mounts. The
/// package has no install script (`codex-cli/package.json`), so
/// `--ignore-scripts` changes nothing but keeps `sudo` from running one, as
/// for pi.
pub const INSTALL_COMMAND: &str = if cfg!(target_os = "macos") {
    "npm install -g --ignore-scripts @openai/codex"
} else {
    "sudo npm install -g --ignore-scripts @openai/codex"
};

/// The documentation of Codex; stands in `docs` of its findings.
pub const DOCS_URL: &str = "https://learn.chatgpt.com/docs/config-file/config-reference";

/// The root of Codex's state (environment-variables page).
pub const ENV_CODEX_HOME: &str = "CODEX_HOME";

/// The CA bundle of Codex's HTTPS, login and WebSocket clients
/// (environment-variables page).
pub const ENV_CA_CERTIFICATE: &str = "CODEX_CA_CERTIFICATE";

/// The variable [`PROVIDER_ID`] reads its key from (`env_key`). Never in the
/// plan's environment: the [`KEY_START`] launcher sets it inside the
/// sandbox.
pub const ENV_API_KEY: &str = "OPENAI_API_KEY";

/// The variables of the adapter whose values the person may see; none of
/// them carries a secret. [`ENV_API_KEY`] is never in the plan.
pub const VISIBLE_ENV: &[&str] = &[ENV_CODEX_HOME, ENV_CA_CERTIFICATE];

/// The id of the provider the adapter declares in `config.toml`.
pub const PROVIDER_ID: &str = "humanitl";

/// The `OpenAI` API, the base of [`PROVIDER_ID`] with a key and without
/// `llm.endpoint` (`codex-rs/model-provider-info/src/lib.rs`,
/// `to_api_provider`).
pub const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

/// The base of the built-in provider under a `ChatGPT` login
/// (`codex-rs/model-provider-info/src/lib.rs`, `CHATGPT_CODEX_BASE_URL`).
pub const CHATGPT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

/// The path below the base Codex posts its inference to
/// (`codex-rs/codex-api/src/lib.rs` `RESPONSES_PATH`); passed for `POST`
/// only.
pub const RESPONSES_PATH: &str = "/responses";

/// The path below the base of Codex's model list
/// (`codex-rs/codex-api/src/endpoint/models.rs`); passed for `GET` only.
pub const MODELS_PATH: &str = "/models";

/// The value of `sandbox_mode`: Codex's own sandbox cannot start inside
/// Humanitl's (module documentation).
pub const SANDBOX_MODE: &str = "danger-full-access";

/// The trust of the project in `config.toml` (`projects.<path>.trust_level`):
/// "Untrusted projects skip project-scoped `.codex/` layers, including
/// project-local config, hooks, and rules" (config reference). Set, so Codex
/// does not ask and then fail to write its read-only `config.toml`; and
/// untrusted, so a repository's own hooks and rules do not run unasked.
pub const PROJECT_TRUST: &str = "untrusted";

/// Where the managed requirements lie under Linux
/// (learn.chatgpt.com/docs/enterprise/managed-configuration).
pub const REQUIREMENTS_DST: &str = "/etc/codex/requirements.toml";

/// The admin skill directory under Linux
/// (learn.chatgpt.com/docs/build-skills).
pub const ADMIN_SKILLS_DIR: &str = "/etc/codex/skills";

/// The launcher with [`ENV_API_KEY`]: reads [`API_KEY_DST`] with the shell's
/// built-in `read`, so no process carries the key in its `argv`, and
/// replaces itself with the agent. As Claude Code's (HUM-514): `sh -c` takes
/// the word after the script as `$0` and the rest as `$@`; the key file has
/// no newline at its end, so `read` answers 1 with the variable set.
pub const KEY_START: &str = r#"IFS= read -r OPENAI_API_KEY < /etc/humanitl/codex/llm-api-key || [ -n "$OPENAI_API_KEY" ] || { echo "humanitl: the access key file /etc/humanitl/codex/llm-api-key is empty" >&2; exit 78; }
export OPENAI_API_KEY
exec "$@"
"#;

/// The sentence the briefing gets for Codex (HUM-515 point 3).
pub const OUTER_BOUNDARY_EN: &str = "Codex: your own sandbox is off (sandbox_mode = \
\"danger-full-access\"), because it cannot start inside this one; the outer boundary is this \
sandbox, and its network rules hold for every command.";

/// The German form of [`OUTER_BOUNDARY_EN`].
pub const OUTER_BOUNDARY_DE: &str = "Codex: Deine eigene Sandbox ist aus (sandbox_mode = \
\"danger-full-access\"), weil sie in dieser nicht starten kann; die äußere Grenze ist diese \
Sandbox, und ihre Netzregeln gelten für jeden Befehl.";

/// `CODEX_HOME` under a home: `~/.codex`, the documented default.
#[must_use]
pub fn codex_home(home: &Path) -> PathBuf {
    home.join(".codex")
}

/// Where the adapter's `config.toml` lies.
#[must_use]
pub fn config_dst(home: &Path) -> PathBuf {
    codex_home(home).join("config.toml")
}

/// Where the skill `humanitl` (HUM-364) lies: an admin skill under Linux, a
/// user skill under macOS.
#[must_use]
pub fn skill_dst(home: &Path) -> PathBuf {
    let root = if cfg!(target_os = "macos") {
        home.join(".agents").join(skill::SKILLS_DIR_NAME)
    } else {
        PathBuf::from(ADMIN_SKILLS_DIR)
    };
    root.join(skill::SKILL_NAME).join(skill::SKILL_FILE_NAME)
}

/// Where the briefing lies under macOS: the global `AGENTS.md`. Under Linux
/// it stands in [`REQUIREMENTS_DST`].
#[must_use]
pub fn agents_md_dst(home: &Path) -> PathBuf {
    codex_home(home).join("AGENTS.md")
}

/// The files the adapter creates, as paths, without the key file:
/// `config.toml`, under Linux the requirements, and with the briefing its
/// file (macOS) and the skill.
#[must_use]
pub fn file_targets(home: &Path, briefing: bool) -> Vec<PathBuf> {
    let mut targets = vec![config_dst(home)];
    if cfg!(target_os = "macos") {
        if briefing {
            targets.push(agents_md_dst(home));
        }
    } else {
        targets.push(PathBuf::from(REQUIREMENTS_DST));
    }
    if briefing {
        targets.push(skill_dst(home));
    }
    targets
}

/// Whether the adapter declares its own provider: with a key or an own
/// endpoint. Otherwise Codex keeps its built-in provider and a `ChatGPT`
/// login.
#[must_use]
pub fn uses_own_provider(llm: &LlmConfig) -> bool {
    llm.endpoint.is_some() || llm.api_key_ref.is_some()
}

/// The base every part of the adapter resolves to, without user, password,
/// query and fragment: the `OpenAI`-compatible base of `llm.endpoint`
/// (`…/v1`), else [`OPENAI_API_BASE`] with a key, else
/// [`CHATGPT_CODEX_BASE_URL`]. Provider, passthrough, briefing, cloud notice
/// and the key's mask all read it.
#[must_use]
pub fn resolved_base(llm: &LlmConfig) -> Option<Url> {
    match llm.endpoint.as_ref() {
        Some(endpoint) => {
            let mut base = llm_endpoint::without_secrets(endpoint);
            base.set_query(None);
            base.set_fragment(None);
            base.set_path(&llm_endpoint::openai_base_path(endpoint));
            Some(base)
        }
        None if llm.api_key_ref.is_some() => Url::parse(OPENAI_API_BASE).ok(),
        None => Url::parse(CHATGPT_CODEX_BASE_URL).ok(),
    }
}

/// The value of `model_providers.humanitl.base_url`: [`resolved_base`], on
/// the loopback under the reserved name the proxy maps (HUM-387).
#[must_use]
pub fn provider_base_url(llm: &LlmConfig) -> Option<String> {
    let base = resolved_base(llm)?;
    Some(llm_endpoint::openai_base_url_via_alias(&base))
}

/// The endpoint Codex sends its inference to: [`resolved_base`].
#[must_use]
pub fn inference_endpoint(llm: &LlmConfig) -> Option<Url> {
    resolved_base(llm)
}

/// The host of the passthrough as `host:port`, for the briefing.
#[must_use]
pub fn passthrough_authority(llm: &LlmConfig) -> Option<String> {
    let base = resolved_base(llm)?;
    Some(format!(
        "{}:{}",
        base.host_str()?,
        base.port_or_known_default()?
    ))
}

/// The passthrough rules of Codex below [`resolved_base`]: `POST` on
/// exactly [`RESPONSES_PATH`] and `GET` on exactly [`MODELS_PATH`], one rule
/// each (module documentation, "What is let through").
#[must_use]
pub fn passthrough_rules(llm: &LlmConfig) -> Vec<Rule> {
    let Some(base) = resolved_base(llm) else {
        return Vec::new();
    };
    let Some(target) = passthrough_target(llm, &base) else {
        return Vec::new();
    };
    let prefix = base.path().trim_end_matches('/').to_owned();
    let exact = |path: &str| vec![humanitl_core::exact_path_entry(&format!("{prefix}{path}"))];
    passthrough::rules(
        &target,
        vec![
            Route::new(&[Method::POST], exact(RESPONSES_PATH)),
            Route::new(&[Method::GET], exact(MODELS_PATH)),
        ],
    )
}

/// Host, scheme, port and note of the passthrough to `base`. An own endpoint
/// usually stands in the own network; the vendor's APIs never do.
fn passthrough_target(llm: &LlmConfig, base: &Url) -> Option<Target> {
    Some(Target {
        host: HostName::parse(base.host_str()?).ok()?,
        scheme: Scheme::parse(base.scheme())?,
        port: base.port_or_known_default()?,
        allow_private: llm.endpoint.is_some(),
        note: format!(
            "LLM passthrough for Codex to {}. Logged, never held.",
            llm_endpoint::display(base)
        ),
    })
}

/// The table `config.toml` under `CODEX_HOME` holds (module documentation).
#[must_use]
pub fn config_table(ctx: &AgentContext) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert("check_for_update_on_startup".into(), false.into());
    table.insert("cli_auth_credentials_store".into(), "file".into());
    table.insert("sandbox_mode".into(), SANDBOX_MODE.into());
    table.insert("analytics".into(), enabled(false));
    table.insert("feedback".into(), enabled(false));
    let mut trust = toml::Table::new();
    trust.insert("trust_level".into(), PROJECT_TRUST.into());
    let mut projects = toml::Table::new();
    projects.insert(
        ctx.work_dir_sandbox.to_string_lossy().into_owned(),
        trust.into(),
    );
    table.insert("projects".into(), projects.into());
    if uses_own_provider(&ctx.llm)
        && let Some(base_url) = provider_base_url(&ctx.llm)
    {
        let mut provider = toml::Table::new();
        provider.insert("name".into(), "Humanitl".into());
        provider.insert("base_url".into(), base_url.into());
        provider.insert("wire_api".into(), "responses".into());
        if ctx.llm_key.is_some() {
            provider.insert("env_key".into(), ENV_API_KEY.into());
        }
        let mut providers = toml::Table::new();
        providers.insert(PROVIDER_ID.into(), provider.into());
        table.insert("model_providers".into(), providers.into());
        table.insert("model_provider".into(), PROVIDER_ID.into());
        if let Some(model) = ctx.llm.models.first() {
            table.insert("model".into(), model.clone().into());
        }
    }
    table
}

/// The managed requirements under Linux: update check, credential store and
/// feedback, and the briefing when given. Every key is a field of
/// `ConfigRequirementsToml`, the struct `/etc/codex/requirements.toml` is
/// read into (`codex-rs/config/src/config_requirements.rs`, commit
/// `a0a30c40`: `cli_auth_credentials_store`, `check_for_update_on_startup`,
/// `feedback`, `additional_developer_instructions`); the config reference
/// lists the first two and the last under "requirements.toml" and says
/// users "can't override an enforced path, update preference, login-shell
/// policy, or feedback setting".
#[must_use]
pub fn requirements_table(briefing: Option<&str>) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert("check_for_update_on_startup".into(), false.into());
    table.insert("cli_auth_credentials_store".into(), "file".into());
    table.insert("feedback".into(), enabled(false));
    if let Some(text) = briefing {
        table.insert("additional_developer_instructions".into(), text.into());
    }
    table
}

fn enabled(on: bool) -> toml::Value {
    let mut table = toml::Table::new();
    table.insert("enabled".into(), on.into());
    table.into()
}

/// A table as TOML text; a failure is a finding (`AGENT_003`), never a
/// silent empty file.
fn render(table: &toml::Table, what: &str) -> Result<Vec<u8>, Diagnostic> {
    toml::to_string(table)
        .map(String::into_bytes)
        .map_err(|err| broken(what, &err.to_string()))
}

/// The adapter for Codex. Stateless: everything it needs is in the
/// [`AgentContext`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CodexAdapter;

impl CodexAdapter {
    /// An adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl AgentAdapter for CodexAdapter {
    fn id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn command(&self, ctx: &AgentContext) -> Vec<OsString> {
        ctx.agent_command_override
            .clone()
            .filter(|command| !command.is_empty())
            .unwrap_or_else(|| vec![OsString::from(DEFAULT_COMMAND)])
    }

    fn launcher(&self, ctx: &AgentContext) -> Vec<OsString> {
        if !ctx.llm_key.is_some() {
            return Vec::new();
        }
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
        let codex_home = codex_home(&ctx.home).to_string_lossy().into_owned();
        [
            (ENV_CODEX_HOME, codex_home),
            (ENV_CA_CERTIFICATE, ca),
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
        .collect()
    }

    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        let mut files = Vec::new();
        if ctx.llm_key.is_some() {
            files.push(SandboxFile::secret(API_KEY_DST, ctx.llm_key.file_content()));
        }
        files.push(SandboxFile::read_only(
            config_dst(&ctx.home),
            render(&config_table(ctx), "config.toml")?,
        ));
        // `agent.briefing.enabled = false` leaves out briefing and skill, no
        // text of Humanitl in the agent at all (HUM-364).
        let briefing = if ctx.briefing.enabled {
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
            let skill = skill::render(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                llm_host.as_deref(),
            )?;
            Some((format!("{text}\n{note}\n"), skill))
        } else {
            None
        };
        if cfg!(target_os = "macos") {
            if let Some((text, _)) = &briefing {
                files.push(SandboxFile::read_only(
                    agents_md_dst(&ctx.home),
                    text.clone().into_bytes(),
                ));
            }
        } else {
            files.push(SandboxFile::read_only(
                REQUIREMENTS_DST,
                render(
                    &requirements_table(briefing.as_ref().map(|(text, _)| text.as_str())),
                    "requirements.toml",
                )?,
            ));
        }
        if let Some((_, skill)) = briefing {
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

    fn is_native_program(&self) -> bool {
        false
    }

    fn is_fullscreen_tui(&self) -> bool {
        true
    }
}
