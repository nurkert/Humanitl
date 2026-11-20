//! The adapter for Claude Code (HUM-514).
//!
//! Claude Code (`claude`, <https://code.claude.com/docs/en/setup>) is one
//! native executable: the native installer puts a launcher under
//! `~/.local/bin`, and the npm package `@anthropic-ai/claude-code` links the
//! same native binary, which "does not itself invoke Node" (setup, "Install
//! with npm", read 2026-11-20). The daemon can therefore bind it from a
//! per-user directory like `OpenCode` (HUM-278).
//!
//! Every statement below is taken from the vendor's documentation, read on
//! 2026-11-20; none is measured on a binary, because the real `claude` is not
//! run in Humanitl's tests.
//!
//! **Network and CA** (<https://code.claude.com/docs/en/network-config>).
//! Claude Code "respects standard proxy environment variables" and "does not
//! support SOCKS proxies"; the env kit of the launcher already sets
//! `HTTPS_PROXY` to the sandbox's HTTP proxy. It trusts "both its bundled
//! Mozilla CA certificates and your operating system's certificate store"
//! (`CLAUDE_CODE_CERT_STORE`, default `bundled,system`), and
//! `NODE_EXTRA_CA_CERTS` adds the sandbox's CA. A file it cannot read in full
//! makes it warn ("Claude Code ignored all of the `NODE_EXTRA_CA_CERTS`
//! file"); the adapter points the variable at the CA the sandbox places.
//! [`ENV_NONESSENTIAL_TRAFFIC`] is set to `1`: "Set to any non-empty value
//! … to disable nonessential network traffic: auto-updates, telemetry,
//! error reporting, the `/feedback` command, … Setting it to `0` or
//! `false` still disables this traffic" (<https://code.claude.com/docs/en/env-vars>).
//! [`ENV_DISABLE_UPDATES`] blocks "all updates including manual `claude
//! update` and `claude install`" (same page); the program lies read-only in
//! the sandbox anyway.
//!
//! **What is let through** (network-config, "Network access requirements").
//! `api.anthropic.com` carries "Claude API requests, including the `WebFetch`
//! domain safety check, feature flag fetches, and telemetry event logging".
//! The adapter's passthrough rule covers only the inference endpoints of the
//! Anthropic Messages format, `/v1/messages` and `/v1/messages/count_tokens`
//! (<https://code.claude.com/docs/en/llm-gateway-protocol>, "API formats"),
//! like every passthrough (K-02: exact inference endpoints). Feature flags
//! and telemetry are off through [`ENV_NONESSENTIAL_TRAFFIC`]; the `WebFetch`
//! check and every other path of the host are held like any request, so the
//! person sees them once. The sign-in hosts `claude.ai`, `claude.com`,
//! `platform.claude.com` and `sso.claude.com` get no rule: a login inside the
//! sandbox is held at first, so the person sees what happens.
//!
//! **Credentials** (<https://code.claude.com/docs/en/authentication>,
//! "Authentication precedence"). An API key from Humanitl's keyring
//! (`llm.api_key_ref`, as for every adapter) travels as a sandbox file from a
//! memfd ([`API_KEY_DST`]), like for `OpenCode` and pi (HUM-140). Claude Code
//! reads its key only from the environment (`ANTHROPIC_API_KEY`, "Sent as
//! the `X-Api-Key` header") or from a gateway token (`ANTHROPIC_AUTH_TOKEN`,
//! "Sent as the `Authorization: Bearer` header"). The adapter therefore puts
//! a shell in front of the agent ([`AgentAdapter::launcher`]) that reads the
//! file with the built-in `read` and exports the variable before it `exec`s
//! the agent: the key is in no `argv` (`bwrap` carries `--setenv` on its own
//! command line, readable in `/proc/<pid>/cmdline`), not in the plan's
//! environment and not in a configuration file. Without `llm.endpoint` the
//! key is `ANTHROPIC_API_KEY` towards `api.anthropic.com`; with
//! `llm.endpoint` it is `ANTHROPIC_AUTH_TOKEN` towards that gateway, whose
//! address goes to `ANTHROPIC_BASE_URL`. In interactive mode Claude Code asks
//! once whether the key replaces the subscription; the answer lives in the
//! session's configuration directory.
//!
//! **No login of the host.** The sandbox mounts nothing of the host's home:
//! `~/.claude`, `~/.claude.json` and `~/.claude/.credentials.json` (the place
//! of the login on Linux, `0600`, authentication page, "Credential
//! management") stay outside. [`ENV_CONFIG_DIR`] points at
//! `<home>/.claude` on the sandbox's own home, a fresh tmpfs in every
//! session: "All settings, session history, and plugins are stored under
//! this path", and with it set "Claude Code keeps the `.credentials.json`
//! file under that directory instead". With `agents.claude.source = "depot"`
//! the depot sets the variable to the session's copy of its configuration
//! (`super::depot::DEPOT_CONFIG_DST`), and its value wins over this one
//! (`SandboxProfile::effective_env`).
//!
//! **The briefing.** Under Linux the briefing lies at the managed policy
//! place `/etc/claude-code/CLAUDE.md`, which `claudeMdExcludes` cannot
//! exclude (<https://code.claude.com/docs/en/memory>), and the skill
//! `humanitl` at the enterprise skill place
//! `/etc/claude-code/.claude/skills/humanitl/SKILL.md`
//! (<https://code.claude.com/docs/en/skills>, "Where skills live"). Both
//! stay outside the configuration directory, so they hold for a depot
//! session as well. Under macOS the managed directory lies under
//! `/Library/Application Support/ClaudeCode`, which no session can write
//! (the same reason as for `OpenCode`, HUM-371); there the briefing is the
//! user memory `~/.claude/CLAUDE.md` and the skill a personal skill. Claude
//! Code's own sandbox stays as it is; the briefing says that the outer
//! boundary is Humanitl ([`OUTER_BOUNDARY_EN`]).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use humanitl_config::{Language, LlmConfig, llm_endpoint};
use humanitl_core::rule::Rule;
use humanitl_core::{Diagnostic, HostName, Method, Scheme};
use url::Url;

use crate::agent::opencode::command_preflight;
use crate::agent::opencode_models::DEFAULT_RULES;
use crate::agent::passthrough::{self, Route, Target};
use crate::agent::{AgentAdapter, AgentContext, SandboxFile, briefing, skill};

/// The id of the adapter as it stands in `agent.adapter`.
pub const ADAPTER_ID: &str = "claude";

/// The command started without `agent.command`.
pub const DEFAULT_COMMAND: &str = "claude";

/// The directory of the adapter's own files in the sandbox.
pub const CLAUDE_DIR: &str = "/etc/humanitl/claude";

/// The access key of the model in the sandbox (HUM-140), a file from a memfd
/// that only the [`AgentAdapter::launcher`] reads.
pub const API_KEY_DST: &str = "/etc/humanitl/claude/llm-api-key";

/// How Claude Code is installed (code.claude.com/docs/en/setup, "Native
/// Install (Recommended)"): the launcher lands in `~/.local/bin`, a per-user
/// directory the daemon binds the single binary from (HUM-278).
pub const INSTALL_COMMAND: &str = "curl -fsSL https://claude.ai/install.sh | bash";

/// The documentation of Claude Code; stands in `docs` of its findings.
pub const DOCS_URL: &str = "https://code.claude.com/docs/en/setup";

/// The configuration directory (code.claude.com/docs/en/env-vars): settings,
/// session history, plugins and, on Linux, `.credentials.json`.
pub const ENV_CONFIG_DIR: &str = "CLAUDE_CONFIG_DIR";

/// Turns off auto-updates, telemetry, error reporting, `/feedback`, release
/// notes and feature-flag fetching; any non-empty value, `0` included
/// (code.claude.com/docs/en/env-vars).
pub const ENV_NONESSENTIAL_TRAFFIC: &str = "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC";

/// Blocks every update, `claude update` and `claude install` included
/// (code.claude.com/docs/en/env-vars, setup "Disable auto-updates").
pub const ENV_DISABLE_UPDATES: &str = "DISABLE_UPDATES";

/// The gateway or own endpoint (code.claude.com/docs/en/env-vars,
/// llm-gateway-connect), set only with `llm.endpoint`.
pub const ENV_BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// The model, set only with `llm.endpoint` and `llm.models`
/// (code.claude.com/docs/en/env-vars).
pub const ENV_MODEL: &str = "ANTHROPIC_MODEL";

/// The API key, sent as `X-Api-Key` (code.claude.com/docs/en/authentication).
/// Never in the plan's environment: the [`KEY_START_API_KEY`] launcher sets
/// it inside the sandbox.
pub const ENV_API_KEY: &str = "ANTHROPIC_API_KEY";

/// The gateway token, sent as `Authorization: Bearer`
/// (code.claude.com/docs/en/authentication). Never in the plan's
/// environment: the [`KEY_START_AUTH_TOKEN`] launcher sets it.
pub const ENV_AUTH_TOKEN: &str = "ANTHROPIC_AUTH_TOKEN";

/// The variables of the adapter whose values the person may see; none of
/// them carries a secret. The two key variables are not among them: they are
/// never in the plan.
pub const VISIBLE_ENV: &[&str] = &[
    ENV_CONFIG_DIR,
    ENV_NONESSENTIAL_TRAFFIC,
    ENV_DISABLE_UPDATES,
    ENV_BASE_URL,
    ENV_MODEL,
];

/// The Anthropic API, Claude Code's provider without `llm.endpoint`
/// (code.claude.com/docs/en/network-config).
pub const DIRECT_API: &str = "https://api.anthropic.com";

/// The host of [`DIRECT_API`].
pub const DIRECT_API_HOST: &str = "api.anthropic.com";

/// The inference endpoints of the Anthropic Messages format, relative to the
/// base (code.claude.com/docs/en/llm-gateway-protocol, "API formats":
/// "`/v1/messages`, `/v1/messages/count_tokens` (optional)"). Inference
/// posts to `/v1/messages?beta=true`; a rule compares the path without the
/// query.
pub const MESSAGES_PATHS: &[&str] = &["/v1/messages", "/v1/messages/count_tokens"];

/// The launcher with `ANTHROPIC_API_KEY`: reads [`API_KEY_DST`] with the
/// shell's built-in `read`, so no process carries the key in its `argv`, and
/// replaces itself with the agent.
///
/// `sh -c` takes the word after the script as `$0` and the rest as `$@`, the
/// agent's command. The key file has no newline at its end; `read` then
/// answers 1 but has set the variable, hence the second test.
pub const KEY_START_API_KEY: &str = r#"IFS= read -r ANTHROPIC_API_KEY < /etc/humanitl/claude/llm-api-key || [ -n "$ANTHROPIC_API_KEY" ] || { echo "humanitl: the access key file /etc/humanitl/claude/llm-api-key is empty" >&2; exit 78; }
export ANTHROPIC_API_KEY
exec "$@"
"#;

/// The launcher with `ANTHROPIC_AUTH_TOKEN`, for a gateway in `llm.endpoint`;
/// otherwise as [`KEY_START_API_KEY`].
pub const KEY_START_AUTH_TOKEN: &str = r#"IFS= read -r ANTHROPIC_AUTH_TOKEN < /etc/humanitl/claude/llm-api-key || [ -n "$ANTHROPIC_AUTH_TOKEN" ] || { echo "humanitl: the access key file /etc/humanitl/claude/llm-api-key is empty" >&2; exit 78; }
export ANTHROPIC_AUTH_TOKEN
exec "$@"
"#;

/// The sentence the briefing gets for Claude Code (HUM-514 point 5).
pub const OUTER_BOUNDARY_EN: &str = "Claude Code: your own sandbox setting stays as it is; the \
outer boundary is this sandbox, and its network rules hold for every command.";

/// The German form of [`OUTER_BOUNDARY_EN`].
pub const OUTER_BOUNDARY_DE: &str = "Claude Code: Deine eigene Sandbox-Einstellung bleibt, wie \
sie ist; die äußere Grenze ist diese Sandbox, und ihre Netzregeln gelten für jeden Befehl.";

/// The managed policy directory of Claude Code under Linux
/// (code.claude.com/docs/en/managed-settings, memory, skills).
pub const MANAGED_DIR: &str = "/etc/claude-code";

/// The configuration directory under a home: `~/.claude`, the documented
/// default of [`ENV_CONFIG_DIR`].
#[must_use]
pub fn config_dir(home: &Path) -> PathBuf {
    home.join(".claude")
}

/// Where the briefing lies: the managed `CLAUDE.md` under Linux, the user
/// memory `~/.claude/CLAUDE.md` under macOS (see the module documentation).
#[must_use]
pub fn briefing_dst(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        config_dir(home).join("CLAUDE.md")
    } else {
        Path::new(MANAGED_DIR).join("CLAUDE.md")
    }
}

/// Where the skill `humanitl` (HUM-364) lies: an enterprise skill under
/// Linux, a personal one under macOS.
#[must_use]
pub fn skill_dst(home: &Path) -> PathBuf {
    let root = if cfg!(target_os = "macos") {
        config_dir(home)
    } else {
        Path::new(MANAGED_DIR).join(".claude")
    };
    root.join(skill::SKILLS_DIR_NAME)
        .join(skill::SKILL_NAME)
        .join(skill::SKILL_FILE_NAME)
}

/// The files the adapter creates, as paths, in the order of
/// [`AgentAdapter::files`] without the key file. `briefing` is
/// `agent.briefing.enabled` and covers the briefing and the skill.
#[must_use]
pub fn file_targets(home: &Path, briefing: bool) -> Vec<PathBuf> {
    if briefing {
        vec![briefing_dst(home), skill_dst(home)]
    } else {
        Vec::new()
    }
}

/// Whether `endpoint` is the Anthropic API itself, where Claude Code sends
/// `ANTHROPIC_API_KEY` without `ANTHROPIC_BASE_URL`: exactly `https`, host
/// [`DIRECT_API_HOST`], port 443, the root or `/v1` as path, and no user,
/// password, query or fragment. Anything else, another port or a path
/// below the host included, is a gateway that gets `ANTHROPIC_BASE_URL`;
/// otherwise the key would go to the default API on port 443 while
/// passthrough and mask named another target (HUM-514, review).
#[must_use]
pub fn is_direct_api(endpoint: &Url) -> bool {
    let path = endpoint.path().trim_end_matches('/');
    endpoint.scheme() == "https"
        && endpoint.host_str() == Some(DIRECT_API_HOST)
        && endpoint.port_or_known_default() == Some(443)
        && (path.is_empty() || path == llm_endpoint::OPENAI_SURFACE)
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.query().is_none()
        && endpoint.fragment().is_none()
}

/// Whether `llm.endpoint` names a gateway rather than the Anthropic API
/// ([`is_direct_api`]).
#[must_use]
pub fn uses_gateway(llm: &LlmConfig) -> bool {
    llm.endpoint
        .as_ref()
        .is_some_and(|endpoint| !is_direct_api(endpoint))
}

/// The one endpoint every part of the adapter resolves to: the gateway in
/// `llm.endpoint`, else [`DIRECT_API`]. Environment, passthrough, briefing,
/// cloud notice and the key's mask all read it.
#[must_use]
pub fn resolved_endpoint(llm: &LlmConfig) -> Option<Url> {
    match llm.endpoint.as_ref() {
        Some(endpoint) if uses_gateway(llm) => Some(endpoint.clone()),
        _ => Url::parse(DIRECT_API).ok(),
    }
}

/// The header that carries the key towards [`resolved_endpoint`]:
/// `X-Api-Key` for `ANTHROPIC_API_KEY`, `Authorization` for
/// `ANTHROPIC_AUTH_TOKEN` (code.claude.com/docs/en/authentication).
#[must_use]
pub fn key_header(llm: &LlmConfig) -> &'static str {
    if uses_gateway(llm) {
        "authorization"
    } else {
        "x-api-key"
    }
}

/// The value of [`ENV_BASE_URL`] for a gateway: scheme, host, port and the
/// path before an `OpenAI` surface (`/v1`), without user, password, query and
/// fragment, because the value stands in the environment the person sees.
/// A gateway on the loopback gets the reserved name the proxy maps to it
/// (HUM-387), as for `OpenCode`.
#[must_use]
pub fn base_url(endpoint: &Url) -> String {
    let mut base = llm_endpoint::without_secrets(endpoint);
    base.set_query(None);
    base.set_fragment(None);
    base.set_path(&llm_endpoint::path_prefix(endpoint));
    if llm_endpoint::is_loopback(&base) {
        // Fails only for an address without a host, which is no loopback.
        let _ = base.set_host(Some(humanitl_core::LLM_ALIAS_HOST));
    }
    base.as_str().trim_end_matches('/').to_owned()
}

/// The passthrough rules of Claude Code: `POST`, the only method it uses
/// there, on exactly the Messages endpoints ([`MESSAGES_PATHS`]) of the
/// Anthropic API, or of the gateway in `llm.endpoint` below its path prefix;
/// one rule (HUM-568, [`passthrough`]).
#[must_use]
pub fn passthrough_rules(llm: &LlmConfig) -> Vec<Rule> {
    let gateway = uses_gateway(llm);
    let Some(endpoint) = resolved_endpoint(llm) else {
        return Vec::new();
    };
    let Some(target) = passthrough_target(&endpoint, gateway) else {
        return Vec::new();
    };
    let prefix = if gateway {
        llm_endpoint::path_prefix(&endpoint)
    } else {
        String::new()
    };
    let paths = MESSAGES_PATHS
        .iter()
        .map(|path| humanitl_core::exact_path_entry(&format!("{prefix}{path}")))
        .collect();
    passthrough::rules(&target, vec![Route::new(&[Method::POST], paths)])
}

/// Host, scheme, port and note of the passthrough to `endpoint`. A gateway
/// usually stands in the own network; the Anthropic API never does, and gets
/// no private address.
fn passthrough_target(endpoint: &Url, gateway: bool) -> Option<Target> {
    Some(Target {
        host: HostName::parse(endpoint.host_str()?).ok()?,
        scheme: Scheme::parse(endpoint.scheme())?,
        port: endpoint.port_or_known_default()?,
        allow_private: gateway,
        note: format!(
            "LLM passthrough for Claude Code to {}. Logged, never held.",
            llm_endpoint::display(endpoint)
        ),
    })
}

/// The host of the passthrough as `host:port`, for the briefing.
#[must_use]
pub fn passthrough_authority(llm: &LlmConfig) -> Option<String> {
    let endpoint = inference_endpoint(llm)?;
    Some(format!(
        "{}:{}",
        endpoint.host_str()?,
        endpoint.port_or_known_default()?
    ))
}

/// The endpoint Claude Code sends its inference to: [`resolved_endpoint`].
#[must_use]
pub fn inference_endpoint(llm: &LlmConfig) -> Option<Url> {
    resolved_endpoint(llm)
}

/// The adapter for Claude Code. Stateless: everything it needs is in the
/// [`AgentContext`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    /// An adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
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
        let script = if uses_gateway(&ctx.llm) {
            KEY_START_AUTH_TOKEN
        } else {
            KEY_START_API_KEY
        };
        vec![
            OsString::from("/bin/sh"),
            OsString::from("-c"),
            OsString::from(script),
            // `sh -c` takes the first argument as `$0`.
            OsString::from(DEFAULT_COMMAND),
        ]
    }

    fn env(&self, ctx: &AgentContext) -> Vec<(String, String)> {
        let ca = ctx.ca_path_sandbox.to_string_lossy().into_owned();
        let home = ctx.home.to_string_lossy().into_owned();
        let config_home = ctx.config_home().to_string_lossy().into_owned();
        let config_dir = config_dir(&ctx.home).to_string_lossy().into_owned();
        let mut env: Vec<(String, String)> = [
            (ENV_CONFIG_DIR, config_dir),
            (ENV_NONESSENTIAL_TRAFFIC, "1".to_owned()),
            (ENV_DISABLE_UPDATES, "1".to_owned()),
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
        .collect();
        if let Some(endpoint) = ctx.llm.endpoint.as_ref().filter(|_| uses_gateway(&ctx.llm)) {
            env.push((ENV_BASE_URL.to_owned(), base_url(endpoint)));
            if let Some(model) = ctx.llm.models.first() {
                env.push((ENV_MODEL.to_owned(), model.clone()));
            }
        }
        env
    }

    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        let mut files = Vec::new();
        if ctx.llm_key.is_some() {
            files.push(SandboxFile::secret(API_KEY_DST, ctx.llm_key.file_content()));
        }
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

    fn key_header(&self, llm: &LlmConfig) -> &'static str {
        key_header(llm)
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
