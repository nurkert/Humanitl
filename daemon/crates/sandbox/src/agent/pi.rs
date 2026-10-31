//! The adapter for pi (HUM-274).
//!
//! pi (<https://pi.dev/>, npm package `@earendil-works/pi-coding-agent`) is a
//! Node script with a full-screen TUI. Everything it reads lives in one agent
//! directory, `$PI_CODING_AGENT_DIR`, by default `~/.pi/agent`: the custom
//! provider in `models.json`, the defaults in `settings.json` and the global
//! instructions in `AGENTS.md`. pi also writes there at every start
//! (`auth.json`, `models-store.json`, `sessions/`, lock directories), so the
//! directory stays a writable part of the tmpfs home and only the files
//! Humanitl places are read-only.
//!
//! **What pi does on its own, measured on 1.3.0.** In interactive mode, without
//! switches, it asks `pi.dev` for the latest version and for a fresh model
//! catalogue, and it downloads `fd` and `rg` from GitHub releases when they
//! are missing. `PI_OFFLINE=1` stops all of that (it also sets
//! `PI_SKIP_VERSION_CHECK` internally); a logging proxy saw no request at all
//! with it. The adapter sets it together with the two explicit switches
//! [`ENV_SKIP_VERSION_CHECK`] and [`ENV_TELEMETRY`], and `rules/default.yaml`
//! blocks the same hosts as the second lock. `PI_OFFLINE` does not touch model
//! calls: the provider below still reaches `llm.endpoint`.
//!
//! **The key.** pi resolves an `apiKey` that starts with `!` by running it
//! through `/bin/sh -c`. `models.json` says `!cat "$HUMANITL_PI_KEY_FILE"`,
//! and the file comes from a memfd like every adapter file (HUM-140). The key
//! is in no environment variable, not in `argv` of `pi` and not in
//! `models.json`; the short-lived `cat` carries only the path.
//!
//! **What a cloned project can change.** A project's `.pi/settings.json`,
//! extensions and skills load only after pi's own trust prompt, which the
//! human answers in the TUI. A trusted project can choose another default
//! model or add a provider; every request still goes through the proxy and is
//! held. `models.json` is read from the agent directory only, so a project
//! cannot rewrite the base URL of the bundled provider.
//!
//! **The skill `humanitl` (HUM-364).** Next to the briefing the adapter
//! places one read-only skill in `<agent-dir>/skills` ([`skill_dst`]), the
//! fuller guidance on waiting, refusals and the meta endpoint
//! ([`crate::agent::skill`]). It is the one exception to HUM-309's "no skills
//! shipped by Humanitl"; no further skill, extension or MCP configuration is
//! bundled. pi's `bash` has no default timeout, so there is no tool timeout
//! to raise (HUM-308).
//!
//! **Web access (HUM-309).** pi has no web tool of its own. The npm package
//! `pi-web-access` adds `web_search` and `fetch_content`. `pi install` would
//! write to `~/.pi/agent` on the host, which the sandbox does not see, so the
//! owner installs it globally next to pi ([`WEB_ACCESS_INSTALL_COMMAND`]), and
//! the adapter lists the directory it finds under `packages` in
//! `settings.json` (`agent.pi.web_access`). pi loads a local package from
//! there without installing anything, also with `PI_OFFLINE=1`. The adapter
//! writes `web-search.json` next to it with `ssrf.trustEnvProxy`: the
//! extension resolves every target itself before it fetches, and the sandbox
//! has no resolver. Its requests go through pi's own dispatcher
//! (`EnvHttpProxyAgent` on `HTTPS_PROXY`), measured with 0.46.0: a fetch
//! arrived at the proxy as `CONNECT host:80`, keyless search as
//! `CONNECT mcp.exa.ai:443`.

use std::ffi::{OsStr, OsString};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use humanitl_config::{LlmConfig, PiWebAccess, llm_endpoint};
use humanitl_core::diagnostics::codes::{AGENT_006, AGENT_007};
use humanitl_core::rule::Rule;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::agent::opencode::{
    command_preflight, install_fix_for, passthrough_authority, passthrough_rule,
    placeholder_model_finding,
};
use crate::agent::opencode_models::DEFAULT_RULES;
use crate::agent::pi_config::{
    ENV_KEY_FILE, NO_KEY_PLACEHOLDER, api_key_command, render_models, render_settings,
    render_web_search,
};
use crate::agent::{
    AgentAdapter, AgentContext, Reach, SandboxFile, SandboxLookup, briefing, find_in_path,
    is_executable_file, skill,
};

/// The id of the adapter as it stands in `agent.adapter`.
pub const ADAPTER_ID: &str = "pi";

/// The command started without `agent.command`.
pub const DEFAULT_COMMAND: &str = "pi";

/// The directory of the adapter's own files in the sandbox.
pub const PI_DIR: &str = "/etc/humanitl/pi";

/// The access key of the model in the sandbox (HUM-140).
///
/// A file and not an environment variable, for the reason given at
/// [`crate::agent::opencode::API_KEY_DST`]; `models.json` refers to it with
/// [`api_key_command`].
pub const API_KEY_DST: &str = "/etc/humanitl/pi/llm-api-key";

/// How pi is installed where the sandbox sees it.
///
/// A global npm install lands under `/usr/local` (or `/usr` with a
/// distribution's npm), inside the read-only `/usr` mount. The `pi.dev`
/// installer puts pi under `$HOME`, which the sandbox does not mount. On macOS
/// the npm of Homebrew writes to `/opt/homebrew` as the user, and `sudo` would
/// leave root-owned files there (HUM-377).
pub const INSTALL_COMMAND: &str = if cfg!(target_os = "macos") {
    "npm install -g --ignore-scripts @earendil-works/pi-coding-agent"
} else {
    "sudo npm install -g --ignore-scripts @earendil-works/pi-coding-agent"
};

/// The documentation of pi; stands in `docs` of its findings.
pub const DOCS_URL: &str =
    "https://github.com/earendil-works/pi/tree/main/packages/coding-agent/docs";

/// Where Node.js comes from when the machine has none; stands in the fix of
/// `AGENT_006`. pi 1.3.0 needs Node.js 22.19 or newer.
pub const NODE_DOWNLOAD_URL: &str = "https://nodejs.org/en/download";

/// The npm name of pi's web extension (HUM-309).
pub const WEB_ACCESS_PACKAGE: &str = "pi-web-access";

/// How the web extension is installed where the sandbox sees it: globally,
/// next to pi, under the read-only `/usr` mount. `--ignore-scripts` like for
/// pi itself: the package needs no install script.
pub const WEB_ACCESS_INSTALL_COMMAND: &str = if cfg!(target_os = "macos") {
    "npm install -g --ignore-scripts pi-web-access"
} else {
    "sudo npm install -g --ignore-scripts pi-web-access"
};

/// The documentation of the web extension; stands in `docs` of `AGENT_007`.
pub const WEB_ACCESS_DOCS_URL: &str = "https://github.com/nicobailon/pi-web-access";

/// Where a global npm install puts its packages, as paths in the sandbox, in
/// the order they are searched after the directory that holds pi itself. On
/// macOS Homebrew comes first, as its `bin` does on the search path.
pub const GLOBAL_NODE_MODULES: &[&str] = if cfg!(target_os = "macos") {
    &[
        "/opt/homebrew/lib/node_modules",
        "/usr/local/lib/node_modules",
        "/usr/lib/node_modules",
    ]
} else {
    &["/usr/local/lib/node_modules", "/usr/lib/node_modules"]
};

/// The agent directory of pi.
pub const ENV_AGENT_DIR: &str = "PI_CODING_AGENT_DIR";
/// No automatic network activity: version check, catalogue refresh, package
/// update check, tool download.
pub const ENV_OFFLINE: &str = "PI_OFFLINE";
/// No request to `pi.dev/api/latest-version`.
pub const ENV_SKIP_VERSION_CHECK: &str = "PI_SKIP_VERSION_CHECK";
/// No install telemetry and no attribution headers.
pub const ENV_TELEMETRY: &str = "PI_TELEMETRY";

/// The variables of this adapter that the sandbox screen may show.
///
/// None of them carries a secret; they switch off what pi would do on its
/// own, and the screen is there to prove that.
pub const VISIBLE_ENV: &[&str] = &[
    ENV_AGENT_DIR,
    ENV_OFFLINE,
    ENV_SKIP_VERSION_CHECK,
    ENV_TELEMETRY,
];

/// The agent directory of pi under a home directory.
///
/// Under the home the agent really sees ([`AgentContext::home`]), so the
/// files land where [`ENV_AGENT_DIR`] points.
#[must_use]
pub fn agent_dir(home: &Path) -> PathBuf {
    home.join(".pi").join("agent")
}

/// `models.json` in the agent directory.
#[must_use]
pub fn models_dst(home: &Path) -> PathBuf {
    agent_dir(home).join("models.json")
}

/// `settings.json` in the agent directory.
#[must_use]
pub fn settings_dst(home: &Path) -> PathBuf {
    agent_dir(home).join("settings.json")
}

/// `web-search.json` in the agent directory, the configuration of
/// `pi-web-access` (HUM-309).
#[must_use]
pub fn web_search_dst(home: &Path) -> PathBuf {
    agent_dir(home).join("web-search.json")
}

/// The briefing (HUM-071): pi reads `<agent-dir>/AGENTS.md` at every start,
/// before the `AGENTS.md` files of the project.
#[must_use]
pub fn briefing_dst(home: &Path) -> PathBuf {
    agent_dir(home).join("AGENTS.md")
}

/// The skill `humanitl` (HUM-364), in pi's user skills directory.
///
/// pi 1.4.0 scans `<agent-dir>/skills` recursively for `SKILL.md`
/// (`userDirs.skills = join(agentDir, "skills")` in its bundle) and lists
/// name and description in the system prompt. A skill there needs no trust
/// prompt; only the project's `.pi/skills` does.
#[must_use]
pub fn skill_dst(home: &Path) -> PathBuf {
    agent_dir(home)
        .join(skill::SKILLS_DIR_NAME)
        .join(skill::SKILL_NAME)
        .join(skill::SKILL_FILE_NAME)
}

/// The files the adapter creates, as paths, in the order of
/// [`AgentAdapter::files`] without the key file. `briefing` is
/// `agent.briefing.enabled` and covers the briefing and the skill.
#[must_use]
pub fn file_targets(home: &Path, briefing: bool, web_access: bool) -> Vec<PathBuf> {
    let mut targets = vec![models_dst(home), settings_dst(home)];
    if web_access {
        targets.push(web_search_dst(home));
    }
    if briefing {
        targets.push(briefing_dst(home));
        targets.push(skill_dst(home));
    }
    targets
}

/// The adapter for pi. Stateless: everything it needs is in the
/// [`AgentContext`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PiAdapter;

impl PiAdapter {
    /// An adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// How many bytes of a script are read to find its `#!` line.
const SHEBANG_LIMIT: u64 = 256;

/// The interpreter a script asks for in its `#!` line, if it is Node.js.
///
/// `#!/usr/bin/env node` gives `node` (looked up on the sandbox path),
/// `#!/usr/bin/env -S node --flag` the same, `#!/opt/node/bin/node` the
/// absolute path. Anything else, including a binary without `#!`, gives
/// `None`: this check is about Node, and the exec of other interpreters is
/// HUM-137's business.
fn node_interpreter(script: &Path) -> Option<OsString> {
    let mut head = Vec::new();
    std::fs::File::open(script)
        .ok()?
        .take(SHEBANG_LIMIT)
        .read_to_end(&mut head)
        .ok()?;
    let line = head.strip_prefix(b"#!")?;
    let line = line.split(|&b| b == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?;
    let mut words = line.split_whitespace();
    let interpreter = words.next()?;
    let is_node = |word: &str| {
        Path::new(word)
            .file_name()
            .is_some_and(|name| name == "node" || name == "nodejs")
    };
    if Path::new(interpreter).file_name() == Some(OsStr::new("env")) {
        // Skip the options of `env`, the value of `-u`/`-C`, and `NAME=value`
        // assignments; the first remaining word is the program.
        let mut program = None;
        while let Some(word) = words.next() {
            match word {
                "-u" | "--unset" | "-C" | "--chdir" => {
                    words.next();
                }
                // A cleared environment or an own `PATH` changes where `env`
                // looks for node; the sandbox PATH no longer answers that, so
                // the lookup stays undecided (`backlog/CONVENTIONS.md` 4.13).
                "-" | "--ignore-environment" => return None,
                _ if word.starts_with("PATH=") => return None,
                _ if word.starts_with('-')
                    && !word.starts_with("--")
                    && word[1..].contains('i') =>
                {
                    return None;
                }
                _ if word.starts_with('-') || word.contains('=') => {}
                _ => {
                    program = Some(word);
                    break;
                }
            }
        }
        let program = program?;
        return is_node(program).then(|| OsString::from(program));
    }
    is_node(interpreter).then(|| OsString::from(interpreter))
}

/// The file that starts in the sandbox for this command, on the host.
///
/// `None` when the command preflight has something to say about it, or when
/// the answer is unknown: then there is nothing to read a `#!` line from.
fn started_script(ctx: &AgentContext, command: &OsStr) -> Option<PathBuf> {
    let named = Path::new(command);
    if named.is_absolute() {
        return (ctx.reaches_program(named) && is_executable_file(named))
            .then(|| named.to_path_buf());
    }
    match ctx.look_up_in_sandbox_path(command) {
        SandboxLookup::Startable(host) => Some(host),
        SandboxLookup::NotExecutable(_) | SandboxLookup::Missing | SandboxLookup::Unknown => None,
    }
}

/// `AGENT_006`: the script asks for Node.js, and the sandbox has none where
/// the script looks for it.
///
/// Decided like the command itself ([`AgentContext::look_up_in_sandbox_path`]):
/// a bare `node` on the sandbox path, an absolute one through the mounts.
/// Unknown is silent (`backlog/CONVENTIONS.md` 4.13). The fix follows from
/// where a `node` lies on the host: mounted but not on the sandbox path gives
/// a `ChangeSetting` of `sandbox.env.PATH`, unmounted gives the `install`
/// command that copies it under `/usr/local/bin`, none at all gives the
/// download page. An absolute interpreter outside the mounts gives the
/// installer of pi: a copy of node elsewhere would not change the `#!` line.
fn node_preflight(ctx: &AgentContext, command: &OsStr) -> Option<Diagnostic> {
    let script = started_script(ctx, command)?;
    // pi's installer is the answer only for pi itself (HUM-274).
    let own_program = Path::new(command).file_name() == Some(OsStr::new(DEFAULT_COMMAND));
    let interpreter = node_interpreter(&script)?;
    let absolute = Path::new(&interpreter).is_absolute();
    let lookup = if absolute {
        ctx.search_view().look_up(Path::new(&interpreter))
    } else {
        ctx.look_up_in_sandbox_path(&interpreter)
    };
    match lookup {
        SandboxLookup::Startable(_) | SandboxLookup::Unknown => return None,
        SandboxLookup::NotExecutable(_) | SandboxLookup::Missing => {}
    }

    let shown = interpreter.to_string_lossy();
    let on_host = if absolute {
        Some(PathBuf::from(&interpreter)).filter(|path| is_executable_file(path))
    } else {
        find_in_path(&interpreter, ctx.host_path.as_deref()).filter(|path| is_executable_file(path))
    };
    let seatbelt = ctx.sandbox_view.seatbelt;
    let (where_host, fix) = match on_host {
        // An absolute `#!` line names the one path the kernel starts; a copy
        // of node elsewhere does not change it. Reinstalling pi with an npm
        // whose node lies under a mount writes a line the sandbox can start.
        _ if absolute && own_program && ctx.sandbox_view.seatbelt => (
            format!(
                "the #! line names that absolute path, and the Seatbelt profile does not let the \
                 sandbox read it; reinstall pi with a Node.js outside your home directory, such \
                 as the one of Homebrew, whose npm writes a #! line the sandbox can start: \
                 {INSTALL_COMMAND}"
            ),
            FixAction::CopyCommand(INSTALL_COMMAND.to_owned()),
        ),
        _ if absolute && own_program => (
            format!(
                "the #! line names that absolute path, and the sandbox does not mount it; \
                 reinstall pi with a Node.js under /usr or /usr/local, whose npm writes a \
                 #! line the sandbox can start: {INSTALL_COMMAND}"
            ),
            FixAction::CopyCommand(INSTALL_COMMAND.to_owned()),
        ),
        // Another command than pi with such a line: copying node elsewhere
        // does not change the line, and pi's installer is not its fix.
        _ if absolute => (
            if seatbelt {
                "the #! line names that absolute path, and the Seatbelt profile does not let the \
                 sandbox read it; point the line at a Node.js outside your home directory"
                    .to_owned()
            } else {
                "the #! line names that absolute path, and the sandbox does not mount it; point \
                 the line at a Node.js under one of the mounts"
                    .to_owned()
            },
            FixAction::OpenUrl(NODE_DOWNLOAD_URL.to_owned()),
        ),
        Some(node) if ctx.reaches_program(&node) => {
            let dir = node
                .parent()
                .unwrap_or(Path::new("/"))
                .display()
                .to_string();
            (
                format!(
                    "{} is {}, but the sandbox PATH does not name {dir}",
                    node.display(),
                    if seatbelt { "readable" } else { "mounted" }
                ),
                FixAction::ChangeSetting {
                    key: "sandbox.env.PATH".to_owned(),
                    value: format!("{dir}:{}", ctx.sandbox_path_value()),
                },
            )
        }
        Some(node) => (
            format!(
                "{} lies on this machine where the sandbox {}",
                node.display(),
                if seatbelt {
                    "may not read it"
                } else {
                    "does not mount it"
                }
            ),
            FixAction::CopyCommand(install_fix_for(ctx, &node)),
        ),
        None => (
            "this machine has no Node.js in PATH either".to_owned(),
            FixAction::OpenUrl(NODE_DOWNLOAD_URL.to_owned()),
        ),
    };
    Some(
        Diagnostic::builder(AGENT_006, Severity::Blocking)
            .why(format!(
                "{} starts with #! and asks for {shown}, and the sandbox has no {shown} to start: \
                 its PATH is {}; {where_host}. The exec would fail with exit 127 after the start. \
                 pi needs Node.js 22.19 or newer inside the sandbox.",
                script.display(),
                ctx.sandbox_path_display(),
            ))
            .fix(fix)
            .docs(DOCS_URL)
            .build(),
    )
}

/// The command that starts in the sandbox: `agent.command`, else `pi`.
fn started_command(ctx: &AgentContext) -> OsString {
    ctx.agent_command_override
        .as_ref()
        .and_then(|command| command.first())
        .map_or_else(|| OsString::from(DEFAULT_COMMAND), Clone::clone)
}

/// What the search for `pi-web-access` found (HUM-309).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebAccessLookup {
    /// The package directory, as a path in the sandbox.
    Found(PathBuf),
    /// No candidate holds the package.
    Missing,
    /// A candidate could not be read; nothing is claimed
    /// (`backlog/CONVENTIONS.md` 4.13).
    Unknown,
}

/// The `node_modules` directory that holds pi itself, if pi is a script
/// under one: a global install of pi lies at
/// `<prefix>/lib/node_modules/@earendil-works/pi-coding-agent/...`, and a
/// global install of the extension next to it. Taken as a path in the
/// sandbox; the lookup of `package.json` decides whether the sandbox reaches
/// it there.
fn node_modules_of_pi(ctx: &AgentContext) -> Option<PathBuf> {
    let script = started_script(ctx, &started_command(ctx))?;
    let real = std::fs::canonicalize(&script).unwrap_or(script);
    real.ancestors()
        .find(|dir| dir.file_name() == Some(OsStr::new("node_modules")))
        .map(Path::to_path_buf)
}

/// More than any `package.json` needs; a larger file is not read to its end.
const PACKAGE_JSON_LIMIT: u64 = 256 * 1024;

/// Whether a `package.json` names the web extension; `None` when it cannot be
/// read or parsed.
///
/// Only a regular file is read. A FIFO named `package.json` would block an
/// `open` until someone writes to it, and with it the preflight of the
/// daemon (review of HUM-309). So the type is checked before the `open`, the
/// `open` itself does not block (`O_NONBLOCK`), and the open descriptor is
/// checked again, in case the file was swapped in between.
fn names_web_access(host: &Path) -> Option<bool> {
    use rustix::fs::{Mode, OFlags};

    if !std::fs::symlink_metadata(host).ok()?.is_file() {
        return Some(false);
    }
    let fd = rustix::fs::open(
        host,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .ok()?;
    let file = std::fs::File::from(fd);
    if !file.metadata().ok()?.is_file() {
        return Some(false);
    }
    let mut text = Vec::new();
    file.take(PACKAGE_JSON_LIMIT).read_to_end(&mut text).ok()?;
    let doc: serde_json::Value = serde_json::from_slice(&text).ok()?;
    Some(doc.get("name").and_then(serde_json::Value::as_str) == Some(WEB_ACCESS_PACKAGE))
}

/// Searches `pi-web-access` where a global npm install puts it: next to pi,
/// then [`GLOBAL_NODE_MODULES`].
///
/// A candidate counts only when the sandbox reaches its `package.json`
/// through the mounts ([`AgentContext::search_view`]) and the file names the
/// package; a directory of that name with another package in it is not the
/// extension.
#[must_use]
pub fn find_web_access(ctx: &AgentContext) -> WebAccessLookup {
    let mut dirs: Vec<PathBuf> = node_modules_of_pi(ctx).into_iter().collect();
    for dir in GLOBAL_NODE_MODULES {
        let dir = PathBuf::from(dir);
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    let view = ctx.search_view();
    let mut unknown = false;
    for dir in dirs {
        let package = dir.join(WEB_ACCESS_PACKAGE);
        match view.resolve(&package.join("package.json")) {
            Reach::Host(host) => match names_web_access(&host) {
                Some(true) => return WebAccessLookup::Found(package),
                Some(false) => {}
                None => unknown = true,
            },
            Reach::Unknown => unknown = true,
            Reach::Absent => {}
        }
    }
    if unknown {
        WebAccessLookup::Unknown
    } else {
        WebAccessLookup::Missing
    }
}

/// The package directory pi loads in this sandbox, or `None`: switched off,
/// or not found (`agent.pi.web_access`, HUM-309).
#[must_use]
pub fn web_access_package(ctx: &AgentContext) -> Option<PathBuf> {
    match ctx.pi_web_access {
        PiWebAccess::Off => None,
        PiWebAccess::Auto | PiWebAccess::On => match find_web_access(ctx) {
            WebAccessLookup::Found(package) => Some(package),
            WebAccessLookup::Missing | WebAccessLookup::Unknown => None,
        },
    }
}

/// `AGENT_007`: `agent.pi.web_access` is `on`, and the package is missing.
///
/// A warning, not a stop: pi runs without the web tools, and the user sees
/// why. `auto` says nothing, and neither does an undecidable lookup.
fn web_access_preflight(ctx: &AgentContext) -> Option<Diagnostic> {
    if ctx.pi_web_access != PiWebAccess::On || find_web_access(ctx) != WebAccessLookup::Missing {
        return None;
    }
    Some(
        Diagnostic::builder(AGENT_007, Severity::Warning)
            .why(format!(
                "agent.pi.web_access is on, and the sandbox finds no package {WEB_ACCESS_PACKAGE} \
                 next to pi or under {}; pi starts without web_search and fetch_content. \
                 `pi install` does not help: it writes to ~/.pi/agent on this machine, which the \
                 sandbox does not see. Install it globally: {WEB_ACCESS_INSTALL_COMMAND}",
                GLOBAL_NODE_MODULES.join(" or "),
            ))
            .fix(FixAction::CopyCommand(
                WEB_ACCESS_INSTALL_COMMAND.to_owned(),
            ))
            .docs(WEB_ACCESS_DOCS_URL)
            .build(),
    )
}

impl AgentAdapter for PiAdapter {
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

    fn is_native_program(&self) -> bool {
        false
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
        let agent = agent_dir(&ctx.home).to_string_lossy().into_owned();
        let config_home = ctx.config_home().to_string_lossy().into_owned();
        [
            (ENV_AGENT_DIR, agent),
            (ENV_KEY_FILE, API_KEY_DST.to_owned()),
            (ENV_OFFLINE, "1".to_owned()),
            (ENV_SKIP_VERSION_CHECK, "1".to_owned()),
            (ENV_TELEMETRY, "0".to_owned()),
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
        .collect()
    }

    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        let base = ctx.llm.endpoint.as_ref().map_or_else(
            || format!("http://127.0.0.1:{}/v1", ctx.proxy_port),
            llm_endpoint::openai_base_url,
        );
        let api_key = if ctx.llm_key.is_some() {
            api_key_command()
        } else {
            NO_KEY_PLACEHOLDER.to_owned()
        };
        let models = render_models(&base, &ctx.llm.models, &api_key)?;
        let web_access = web_access_package(ctx);
        let package = web_access
            .as_ref()
            .map(|package| package.to_string_lossy().into_owned());
        let settings = render_settings(&ctx.llm.models, package.as_deref())?;

        let mut files = vec![
            SandboxFile::read_only(models_dst(&ctx.home), models.into_bytes()),
            SandboxFile::read_only(settings_dst(&ctx.home), settings.into_bytes()),
        ];
        if ctx.llm_key.is_some() {
            files.push(SandboxFile::secret(API_KEY_DST, ctx.llm_key.file_content()));
        }
        // Read-only like the others: the extension's own commands that save
        // a provider choice fail, and the agent cannot switch the DNS check
        // back on or point the extension at another proxy (HUM-309).
        if web_access.is_some() {
            files.push(SandboxFile::read_only(
                web_search_dst(&ctx.home),
                render_web_search()?.into_bytes(),
            ));
        }
        // The same briefing as for OpenCode, with the host from the same
        // checked source as the passthrough rule (HUM-071).
        if ctx.briefing.enabled {
            let llm_host = passthrough_authority(&ctx.llm);
            let text = briefing::render_for_session(
                ctx.language,
                ctx.hold.ask_mode,
                ctx.hold.timeout_secs,
                llm_host.as_deref(),
            )?;
            files.push(SandboxFile::read_only(
                briefing_dst(&ctx.home),
                text.into_bytes(),
            ));
            // The skill belongs to the briefing (HUM-364): the briefing
            // points to it, and switching the briefing off means no text of
            // Humanitl in the agent at all.
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

    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule> {
        passthrough_rule(llm)
    }

    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic> {
        let mut diagnostics = command_preflight(ctx, self);
        // Only when the command itself is fine: a finding about Node for a
        // command that is not there would be a second answer to one question.
        if diagnostics.is_empty() {
            diagnostics.extend(node_preflight(ctx, &started_command(ctx)));
        }
        diagnostics.extend(web_access_preflight(ctx));
        diagnostics.extend(placeholder_model_finding(&ctx.llm));
        diagnostics
    }

    fn is_fullscreen_tui(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::OsString;
    use std::path::Path;

    use super::node_interpreter;

    #[test]
    #[cfg(target_os = "macos")]
    fn on_macos_homebrew_is_searched_and_no_fix_says_sudo() {
        assert_eq!(
            super::GLOBAL_NODE_MODULES.first(),
            Some(&"/opt/homebrew/lib/node_modules")
        );
        assert!(!super::INSTALL_COMMAND.contains("sudo"));
        assert!(!super::WEB_ACCESS_INSTALL_COMMAND.contains("sudo"));
    }

    fn script(dir: &Path, text: &[u8]) -> std::path::PathBuf {
        let path = dir.join("pi");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn the_shebang_names_node_directly_or_through_env() {
        let dir = tempfile::tempdir().unwrap();
        for (text, expected) in [
            (&b"#!/usr/bin/env node\nimport x;\n"[..], Some("node")),
            (b"#!/usr/bin/env -S node --no-warnings\n", Some("node")),
            (b"#!/usr/bin/env -u NODE_OPTIONS node\n", Some("node")),
            (
                b"#!/usr/bin/env -S NODE_ENV=production node\n",
                Some("node"),
            ),
            (b"#!/usr/bin/env -S PATH=/missing node\n", None),
            (b"#!/usr/bin/env -i node\n", None),
            (b"#!/usr/bin/env -iS node\n", None),
            (b"#!/usr/bin/env --ignore-environment node\n", None),
            (b"#! /usr/bin/env nodejs\n", Some("nodejs")),
            (b"#!/opt/node/bin/node\n", Some("/opt/node/bin/node")),
            (b"#!/bin/sh\nexec node x\n", None),
            (b"#!/usr/bin/env python3\n", None),
            (b"\x7fELF\x02\x01\x01", None),
            (b"", None),
        ] {
            assert_eq!(
                node_interpreter(&script(dir.path(), text)),
                expected.map(OsString::from),
                "for {:?}",
                String::from_utf8_lossy(text)
            );
        }
    }
}
