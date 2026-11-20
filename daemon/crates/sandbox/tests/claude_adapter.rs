//! The Claude Code adapter (HUM-514).
//!
//! Checked is what the adapter hands the launcher: command, launcher,
//! environment, files, preflight and passthrough rule, all pure functions of
//! the [`AgentContext`]. One test at the end starts a real sandbox with a
//! stand-in for `claude` that checks its environment, without any network
//! and without the vendor's program, beside a fake host home that carries a
//! planted `~/.claude`.

// Die Tests mit echter Sandbox laufen nur unter Linux; ihre Helfer bleiben
// sonst unbenutzt (HUM-312).
#![cfg_attr(not(target_os = "linux"), allow(unused_imports, dead_code))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::secret::ApiKey;
use humanitl_config::{AgentBriefing, Language, LlmConfig};
use humanitl_core::{FixAction, Method, SessionId};
use humanitl_sandbox::agent::claude::{
    API_KEY_DST, DOCS_URL, ENV_API_KEY, ENV_AUTH_TOKEN, ENV_BASE_URL, ENV_CONFIG_DIR,
    ENV_DISABLE_UPDATES, ENV_MODEL, ENV_NONESSENTIAL_TRAFFIC, INSTALL_COMMAND, KEY_START_API_KEY,
    KEY_START_AUTH_TOKEN, OUTER_BOUNDARY_DE, OUTER_BOUNDARY_EN, briefing_dst, file_targets,
    skill_dst,
};
use humanitl_sandbox::agent::{cloud, depot, files_on_reserved_targets, shown_program};
use humanitl_sandbox::{
    AdapterRegistry, AgentAdapter, AgentContext, AgentKey, ClaudeCodeAdapter, SandboxBackend,
    SandboxProfile, SandboxView, files_inside_work,
};

/// A key long enough for `ApiKey::new`.
const KEY: &str = "sk-ant-test-0123456789abcdef";

/// The home of the agent in the sandbox, as the shipped profile sets it.
const HOME: &str = "/home/agent";

/// What the planted host login carries; it must appear nowhere in the
/// sandbox.
const HOST_MARKER: &str = "host-login-marker-5f1c9a";

fn context(endpoint: Option<&str>) -> AgentContext {
    let llm = LlmConfig {
        endpoint: endpoint.map(|text| url::Url::parse(text).unwrap()),
        ..LlmConfig::default()
    };
    AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm)
}

fn keyed(endpoint: Option<&str>) -> AgentContext {
    context(endpoint).with_llm_key(AgentKey::Key(ApiKey::new(KEY).unwrap()))
}

fn env_map(ctx: &AgentContext) -> BTreeMap<String, String> {
    ClaudeCodeAdapter::new().env(ctx).into_iter().collect()
}

fn words(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect()
}

fn file_at(ctx: &AgentContext, dst: &Path) -> String {
    let files = ClaudeCodeAdapter::new().files(ctx).unwrap();
    let file = files
        .iter()
        .find(|file| file.dst == dst)
        .unwrap_or_else(|| panic!("{} is missing", dst.display()));
    String::from_utf8(file.content.clone()).unwrap()
}

#[test]
fn the_registry_knows_claude_code() {
    let registry = AdapterRegistry::builtin();
    let adapter = registry.get("claude").expect("claude is registered");
    assert_eq!(adapter.program(), "claude");
    assert_eq!(adapter.install_command(), INSTALL_COMMAND);
    assert_eq!(adapter.docs_url(), DOCS_URL);
    assert!(adapter.is_fullscreen_tui());
    assert!(adapter.is_native_program(), "one native executable");
    assert!(adapter.is_agent_command(&[OsString::from("/usr/local/bin/claude")]));
    assert!(!adapter.is_agent_command(&[OsString::from("bash")]));
}

#[test]
fn the_command_is_claude_or_agent_command() {
    let adapter = ClaudeCodeAdapter::new();
    assert_eq!(words(&adapter.command(&context(None))), ["claude"]);
    let own = context(None).with_command_override(Some(vec![
        OsString::from("claude"),
        OsString::from("--model"),
        OsString::from("opus"),
    ]));
    assert_eq!(words(&adapter.command(&own)), ["claude", "--model", "opus"]);
}

/// The env kit: configuration directory in the sandbox's own home, no
/// nonessential traffic, no update, the sandbox's CA; never a key.
///
/// Mutation proofs: `NODE_EXTRA_CA_CERTS` dropped from `env` turns the CA
/// assert red ("CA missing"); `ANTHROPIC_API_KEY` with the key in `env`
/// turns the last loop red ("key as argument": `bwrap` carries every
/// variable of the plan as `--setenv` in its `argv`).
#[test]
fn the_env_kit_points_at_the_sandbox_and_carries_no_key() {
    let ctx = keyed(None);
    let env = env_map(&ctx);
    assert_eq!(env[ENV_CONFIG_DIR], format!("{HOME}/.claude"));
    assert_eq!(env[ENV_NONESSENTIAL_TRAFFIC], "1");
    assert_eq!(env[ENV_DISABLE_UPDATES], "1");
    assert_eq!(env["HOME"], HOME);
    assert_eq!(
        env["NODE_EXTRA_CA_CERTS"],
        ctx.ca_path_sandbox.to_string_lossy(),
        "the sandbox's CA joins Claude Code's own roots"
    );
    assert_eq!(
        ctx.ca_path_sandbox,
        Path::new(humanitl_sandbox::profile::CA_CERT_DST)
    );
    assert!(!env.contains_key(ENV_BASE_URL), "no gateway: {env:?}");
    assert!(!env.contains_key(ENV_MODEL), "no gateway: {env:?}");
    for (name, value) in &env {
        assert!(!value.contains(KEY), "the key reached the plan as {name}");
        assert_ne!(name, ENV_API_KEY);
        assert_ne!(name, ENV_AUTH_TOKEN);
    }
}

/// `CLAUDE_CONFIG_DIR` follows the home the agent really sees, never the
/// host's home.
#[test]
fn the_configuration_directory_follows_the_sandbox_home_and_not_the_host() {
    let ctx = context(None)
        .with_home(PathBuf::from("/home/other"))
        .with_host_home(Some(PathBuf::from("/home/u")));
    let env = env_map(&ctx);
    assert_eq!(env[ENV_CONFIG_DIR], "/home/other/.claude");
    for (name, value) in &env {
        assert!(
            !value.contains("/home/u"),
            "{name}={value} names the host's home"
        );
    }
}

/// A gateway in `llm.endpoint` becomes `ANTHROPIC_BASE_URL` without user,
/// password, query and an `OpenAI` surface; one on the loopback under the
/// reserved name the proxy maps (HUM-387); the first model goes along.
#[test]
fn a_gateway_becomes_the_base_url() {
    let mut ctx = context(Some("https://user:pw@gw.example.com/anthropic/v1?x=1"));
    ctx.llm.models = vec!["claude-sonnet".to_owned(), "other".to_owned()];
    let env = env_map(&ctx);
    assert_eq!(env[ENV_BASE_URL], "https://gw.example.com/anthropic");
    assert_eq!(env[ENV_MODEL], "claude-sonnet");

    let local = env_map(&context(Some("http://127.0.0.1:4000")));
    assert_eq!(
        local[ENV_BASE_URL],
        format!("http://{}:4000", humanitl_core::LLM_ALIAS_HOST)
    );

    // The Anthropic API in `llm.endpoint` is no gateway.
    let direct = env_map(&context(Some("https://api.anthropic.com")));
    assert!(!direct.contains_key(ENV_BASE_URL), "{direct:?}");
}

/// Only the Anthropic API itself is direct: `https`, port 443, root or
/// `/v1`. The same host on another port or below a path is a gateway, and
/// every part of the adapter names that one target (HUM-514, review).
///
/// Mutation proof: `is_direct_api` that compares only the host turns the
/// second block red.
#[test]
fn only_the_anthropic_api_itself_is_direct() {
    use humanitl_sandbox::agent::claude::{
        inference_endpoint, is_direct_api, key_header, passthrough_authority, uses_gateway,
    };
    for direct in [
        "https://api.anthropic.com",
        "https://api.anthropic.com/",
        "https://api.anthropic.com/v1",
        "https://api.anthropic.com:443/v1/",
    ] {
        let url = url::Url::parse(direct).unwrap();
        assert!(is_direct_api(&url), "{direct}");
        let llm = context(Some(direct)).llm;
        assert!(!uses_gateway(&llm), "{direct}");
        assert_eq!(key_header(&llm), "x-api-key");
        assert_eq!(
            passthrough_authority(&llm).as_deref(),
            Some("api.anthropic.com:443")
        );
    }
    for gateway in [
        "https://api.anthropic.com:8443/tenant/v1",
        "https://api.anthropic.com/tenant",
        "http://api.anthropic.com",
        "https://api.anthropic.com/v1?x=1",
        "https://user:pw@api.anthropic.com",
    ] {
        let url = url::Url::parse(gateway).unwrap();
        assert!(!is_direct_api(&url), "{gateway}");
        let ctx = keyed(Some(gateway));
        assert!(uses_gateway(&ctx.llm), "{gateway}");
        assert_eq!(key_header(&ctx.llm), "authorization");
        assert_eq!(inference_endpoint(&ctx.llm).as_ref(), Some(&url));
        assert!(env_map(&ctx).contains_key(ENV_BASE_URL), "{gateway}");
        assert_eq!(
            words(&ClaudeCodeAdapter::new().launcher(&ctx))[2],
            KEY_START_AUTH_TOKEN
        );
    }
    let odd = context(Some("https://api.anthropic.com:8443/tenant/v1")).llm;
    let rule = ClaudeCodeAdapter::new().llm_passthrough(&odd).unwrap();
    assert_eq!(rule.matcher.port, Some(8443));
    assert_eq!(
        passthrough_authority(&odd).as_deref(),
        Some("api.anthropic.com:8443")
    );
}

/// Without a key no launcher; with one a shell in front of the agent that
/// reads the key file, `ANTHROPIC_API_KEY` towards the Anthropic API and
/// `ANTHROPIC_AUTH_TOKEN` towards a gateway. The key itself stands in none
/// of the words.
///
/// Mutation proof ("key as argument"): a launcher that writes the key into
/// its script turns the last assert red.
#[test]
fn the_launcher_moves_the_key_from_its_file_into_the_environment() {
    let adapter = ClaudeCodeAdapter::new();
    assert!(adapter.launcher(&context(None)).is_empty());

    let direct = words(&adapter.launcher(&keyed(None)));
    assert_eq!(direct, ["/bin/sh", "-c", KEY_START_API_KEY, "claude"]);
    assert!(KEY_START_API_KEY.contains(API_KEY_DST));
    assert!(KEY_START_API_KEY.contains("export ANTHROPIC_API_KEY"));

    let gateway = words(&adapter.launcher(&keyed(Some("https://gw.example.com"))));
    assert_eq!(gateway, ["/bin/sh", "-c", KEY_START_AUTH_TOKEN, "claude"]);
    assert!(KEY_START_AUTH_TOKEN.contains("export ANTHROPIC_AUTH_TOKEN"));

    // A plan announces the key without reading it: the same words.
    let withheld = context(None).with_llm_key(AgentKey::Withheld);
    assert_eq!(words(&adapter.launcher(&withheld)), direct);

    for word in direct.iter().chain(&gateway) {
        assert!(!word.contains(KEY), "the key reached argv: {word}");
    }
}

/// The program behind the launcher is what the sessions list shows.
#[test]
fn the_sessions_list_sees_through_the_launcher() {
    let adapter = ClaudeCodeAdapter::new();
    let mut command = adapter.launcher(&keyed(None));
    command.push(OsString::from("/run/humanitl/agent/claude"));
    command.push(OsString::from("--resume"));
    assert_eq!(
        shown_program(&command).map(|program| program.to_string_lossy().into_owned()),
        Some("/run/humanitl/agent/claude".to_owned())
    );
}

/// The key file, the briefing at the managed place with the sentence about
/// the outer boundary, and the skill; nothing in `/work`, nothing on a
/// reserved target.
#[test]
fn the_files_are_key_briefing_and_skill() {
    let ctx = keyed(None);
    let files = ClaudeCodeAdapter::new().files(&ctx).unwrap();
    let key = files.iter().find(|file| file.secret).expect("the key file");
    assert_eq!(key.dst, Path::new(API_KEY_DST));
    assert_eq!(key.content, KEY.as_bytes());
    assert_eq!(files.iter().filter(|file| file.secret).count(), 1);

    let home = Path::new(HOME);
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(briefing_dst(home), Path::new("/etc/claude-code/CLAUDE.md"));
        assert_eq!(
            skill_dst(home),
            Path::new("/etc/claude-code/.claude/skills/humanitl/SKILL.md")
        );
    }
    #[cfg(target_os = "macos")]
    {
        assert_eq!(
            briefing_dst(home),
            Path::new("/home/agent/.claude/CLAUDE.md")
        );
        assert_eq!(
            skill_dst(home),
            Path::new("/home/agent/.claude/skills/humanitl/SKILL.md")
        );
    }
    let mut targets: Vec<PathBuf> = files
        .iter()
        .filter(|file| !file.secret)
        .map(|file| file.dst.clone())
        .collect();
    targets.sort();
    let mut expected = file_targets(home, true);
    expected.sort();
    assert_eq!(targets, expected);

    let briefing = file_at(&ctx, &briefing_dst(home));
    assert!(briefing.contains("Blocked by Humanitl."), "{briefing}");
    assert!(briefing.contains(OUTER_BOUNDARY_EN), "{briefing}");
    assert!(
        briefing.contains("api.anthropic.com:443"),
        "the briefing names the passthrough: {briefing}"
    );
    let skill = file_at(&ctx, &skill_dst(home));
    assert!(skill.starts_with("---\nname: humanitl\n"), "{skill}");

    let german = keyed(None).with_language(Language::De);
    assert!(file_at(&german, &briefing_dst(home)).contains(OUTER_BOUNDARY_DE));

    assert!(files_inside_work(&files, Path::new("/work")).is_empty());
    assert!(files_on_reserved_targets(&files).is_empty());
    for file in &files {
        assert!(
            !file.dst.starts_with("/home/u"),
            "{} lies in the host's home",
            file.dst.display()
        );
    }
}

#[test]
fn without_key_and_briefing_there_are_no_files() {
    let ctx = context(None).with_briefing(AgentBriefing { enabled: false });
    assert!(ClaudeCodeAdapter::new().files(&ctx).unwrap().is_empty());
    assert!(file_targets(Path::new(HOME), false).is_empty());
}

/// The passthrough: only the Messages endpoints of `api.anthropic.com`, or
/// of the gateway below its path prefix; `POST` only.
#[test]
fn the_passthrough_covers_the_messages_endpoints_only() {
    let adapter = ClaudeCodeAdapter::new();
    let direct = adapter.llm_passthrough(&context(None).llm).expect("a rule");
    assert!(direct.passthrough_llm);
    assert!(
        !direct.allow_private,
        "the Anthropic API is no private address"
    );
    assert_eq!(
        direct.matcher.host,
        humanitl_core::rule::HostPattern::Exact(
            humanitl_core::HostName::parse("api.anthropic.com").unwrap()
        )
    );
    assert_eq!(
        direct.matcher.path_prefixes,
        ["/v1/messages?", "/v1/messages/count_tokens?"]
    );
    assert_eq!(direct.matcher.methods, Some(vec![Method::POST]));
    assert_eq!(direct.matcher.port, Some(443));

    let gateway = adapter
        .llm_passthrough(&context(Some("http://10.0.0.5:4000/anthropic")).llm)
        .expect("a rule");
    assert!(gateway.allow_private);
    assert_eq!(
        gateway.matcher.path_prefixes,
        [
            "/anthropic/v1/messages?",
            "/anthropic/v1/messages/count_tokens?"
        ]
    );
    assert_eq!(gateway.matcher.port, Some(4000));
}

/// Without `llm.endpoint` the inference goes to `api.anthropic.com`, so the
/// notice of HUM-516 appears; a local gateway keeps it here.
#[test]
fn the_inference_goes_to_anthropic_without_a_gateway() {
    let mut config = humanitl_config::Config::default();
    config.agent.adapter = "claude".to_owned();
    let found = cloud::cloud_inference(&config).expect("a cloud provider");
    assert_eq!(found.provider, "api.anthropic.com");
    config.llm.endpoint = Some(url::Url::parse("http://127.0.0.1:4000").unwrap());
    assert_eq!(cloud::cloud_inference(&config), None);
}

/// The preflight names `claude` and the native installer when it is missing.
#[test]
fn the_preflight_names_the_installer_when_claude_is_missing() {
    let empty = tempfile::tempdir().unwrap();
    let dir = empty.path().display().to_string();
    let ctx = context(None)
        .with_host_path(Some(OsString::from(&dir)))
        .with_sandbox_view(SandboxView::same_path(
            vec![empty.path().to_path_buf()],
            Vec::new(),
        ))
        .with_sandbox_path(Some(OsString::from(&dir)));
    let found = ClaudeCodeAdapter::new().preflight(&ctx);
    let missing = found
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_001")
        .unwrap_or_else(|| panic!("AGENT_001 in {found:?}"));
    // A native program: the fix points `agent.command` at it, and the
    // finding names the installer beside it (HUM-278).
    assert!(
        matches!(&missing.fix, Some(FixAction::CopyCommand(command)) if command.contains("claude")),
        "{:?}",
        missing.fix
    );
    assert!(
        format!("{missing:?}").contains(INSTALL_COMMAND),
        "the installer is named: {missing:?}"
    );
    assert_eq!(missing.docs.as_deref(), Some(DOCS_URL));
}

/// The depot recipe: the keys that run a command, as the settings reference
/// lists them, and no credential format nor login run until the owner
/// walkthrough has measured them (HUM-553).
#[test]
fn the_depot_recipe_lists_the_command_keys_and_declares_no_format() {
    let recipe = ClaudeCodeAdapter::new()
        .depot_recipe()
        .expect("a depot recipe");
    for key in [
        "hooks",
        "apiKeyHelper",
        "statusLine",
        "mcpServers",
        "env",
        "awsAuthRefresh",
        "awsCredentialExport",
        "gcpAuthRefresh",
        "otelHeadersHelper",
        "fileSuggestion",
        "processWrapper",
    ] {
        assert!(recipe.command_keys.contains(&key), "{key} is missing");
    }
    assert_eq!(recipe.config_env, Some(ENV_CONFIG_DIR));
    assert_eq!(recipe.update_off, &[(ENV_DISABLE_UPDATES, "1")]);
    assert_eq!(recipe.credential_file, Some(".credentials.json"));
    assert_eq!(
        recipe.credential_format, None,
        "the format is not published"
    );
    assert_eq!(recipe.login_run, None);
    assert_eq!(recipe.login, depot::LoginFlow::PastedCode);
}

// --- Integration: a real sandbox with a stand-in for claude ----------------

/// What the stand-in checks, as far as Humanitl is concerned: the key in its
/// environment, the configuration directory on the sandbox's home and empty,
/// the CA readable, the briefing and skill, nothing of the host's login.
/// The fake host home comes as `$1`. The script names only the key's prefix:
/// the whole key in it would stand in `argv`.
const STAND_IN: &str = r#"
host_home=$1
case $ANTHROPIC_API_KEY in sk-ant-test-*) [ "$ANTHROPIC_API_KEY" = "$(cat /etc/humanitl/claude/llm-api-key)" ] && echo key=env ;; *) echo key=missing ;; esac
[ -z "$ANTHROPIC_AUTH_TOKEN" ] && echo token=unset
[ -z "$ANTHROPIC_BASE_URL" ] && echo base=unset
echo "config=$CLAUDE_CONFIG_DIR"
echo "nonessential=$CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC updates=$DISABLE_UPDATES"
grep -q 'BEGIN CERTIFICATE' "$NODE_EXTRA_CA_CERTS" && echo ca=readable
grep -q 'Blocked by Humanitl.' /etc/claude-code/CLAUDE.md && echo briefing=ok
grep -q 'outer boundary' /etc/claude-code/CLAUDE.md && echo boundary=ok
head -n 2 /etc/claude-code/.claude/skills/humanitl/SKILL.md | grep -q '^name: humanitl$' && echo skill=ok
if [ -z "$(ls -A "$CLAUDE_CONFIG_DIR" 2>/dev/null)" ]; then echo config_dir=empty; else echo "config_dir=$(ls -A "$CLAUDE_CONFIG_DIR")"; fi
[ ! -e "$HOME/.claude.json" ] && [ ! -e "$HOME/.claude/.credentials.json" ] && echo host_login=absent
[ ! -e "$host_home" ] && echo host_home=absent
if grep -rqs -D skip host-login-marker-5f1c9a /home /etc /tmp /work /run 2>/dev/null; then echo marker=found; else echo marker=absent; fi
mkdir -p "$CLAUDE_CONFIG_DIR" && echo '{}' > "$CLAUDE_CONFIG_DIR/.claude.json" && echo config_dir=writable
if echo x > /etc/humanitl/claude/llm-api-key 2>/dev/null; then echo key_file=writable; else echo key_file=read_only; fi
"#;

/// A stand-in for `claude` behind the adapter's launcher in a real sandbox:
/// the key arrives in its environment and in no `argv`, the CA and the
/// briefing are there, and nothing of a planted host `~/.claude` is visible
/// (HUM-514, acceptance criterion 2).
///
/// Mutation proofs: an adapter whose `files` copies
/// `<host_home>/.claude/.credentials.json` into the configuration directory
/// ("host login mounted") turns `marker=absent` and `config_dir=empty` red;
/// a launcher that exports a literal key, or `env` with `ANTHROPIC_API_KEY`
/// ("key as argument"), turns the `argv` assert red.
#[cfg(target_os = "linux")]
#[test]
fn a_stand_in_finds_its_key_and_nothing_of_the_host_login_in_a_real_sandbox() {
    let Some((fx, backend)) =
        sandbox_or_skip("a_stand_in_finds_its_key_and_nothing_of_the_host_login_in_a_real_sandbox")
    else {
        return;
    };
    // The fake host home with a planted login; never the developer's own.
    let host_home = fx.root.join("host-home");
    std::fs::create_dir_all(host_home.join(".claude")).unwrap();
    std::fs::write(
        host_home.join(".claude/.credentials.json"),
        format!("{{\"token\": \"{HOST_MARKER}\"}}"),
    )
    .unwrap();
    std::fs::write(
        host_home.join(".claude.json"),
        format!("{{\"oauthAccount\": \"{HOST_MARKER}\"}}"),
    )
    .unwrap();

    let adapter = ClaudeCodeAdapter::new();
    let ctx = keyed(None).with_host_home(Some(host_home.clone()));
    let mut command = adapter.launcher(&ctx);
    assert!(!command.is_empty(), "a key brings the launcher");
    command.extend(
        ["sh", "-c", STAND_IN, "claude"]
            .iter()
            .map(OsString::from)
            .chain(std::iter::once(host_home.clone().into_os_string())),
    );
    let mut session = fx.context(&[]);
    session.command = command;
    session.files = adapter.files(&ctx).expect("the adapter renders its files");
    session.session_env.extend(adapter.env(&ctx));

    let plan = backend
        .plan(&humanitl_sandbox::PlanRequest::new(
            &fx.profile,
            &session,
            &captured(),
        ))
        .expect("the plan builds");
    for word in &plan.argv {
        assert!(
            !word.to_string_lossy().contains(KEY),
            "the key reached argv: {}",
            word.to_string_lossy()
        );
    }
    for (name, value) in &plan.env {
        assert!(!value.contains(KEY), "the key reached the plan as {name}");
    }
    let handle = backend.launch(&plan).expect("bwrap starts");
    let checks = backend.isolation_check(&handle);
    let status = handle
        .wait_timeout(std::time::Duration::from_secs(30))
        .expect("the sandbox ends in time")
        .expect("bwrap ran the command");
    let output = handle.output().expect("captured");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        status.success(),
        "{status}: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for line in [
        "key=env",
        "token=unset",
        "base=unset",
        "config=/home/agent/.claude",
        "nonessential=1 updates=1",
        "ca=readable",
        "briefing=ok",
        "boundary=ok",
        "skill=ok",
        "config_dir=empty",
        "host_login=absent",
        "host_home=absent",
        "marker=absent",
        "config_dir=writable",
        "key_file=read_only",
    ] {
        assert!(stdout.contains(line), "{line} is missing in:\n{stdout}");
    }
    assert_eq!(checks.len(), 3, "{checks:?}");
    for check in &checks {
        assert!(
            check.passed,
            "guarantee {:?} failed with Claude Code's files: {check:?}",
            check.check
        );
    }
}

/// Skips without a usable `bwrap` or shim, and refuses to skip under CI;
/// the same pattern as in `pi_adapter.rs`.
fn sandbox_or_skip(test: &str) -> Option<(Fixture, humanitl_sandbox::Backend)> {
    let fixture = match Fixture::new() {
        Ok(fixture) => fixture,
        Err(why) => return skip(test, &why),
    };
    match fixture.backend() {
        Ok(backend) => Some((fixture, backend)),
        Err(why) => skip(test, &why),
    }
}

fn skip<T>(test: &str, why: &str) -> Option<T> {
    assert!(
        std::env::var_os("CI").is_none(),
        "under CI this test must run: {test} would skip because {why}"
    );
    eprintln!("SKIP {test}: {why}");
    None
}

/// Placeholders for socket, CA and shim, and the shipped profile.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    profile: SandboxProfile,
    work: PathBuf,
    socket: PathBuf,
    ca: PathBuf,
    ca_bundle: PathBuf,
    shim: PathBuf,
    paths: humanitl_config::Paths,
}

impl Fixture {
    fn new() -> Result<Self, String> {
        let why = |what: &str, err: &dyn std::fmt::Display| format!("{what}: {err}");
        let dir = tempfile::tempdir().map_err(|err| why("no temporary directory", &err))?;
        let root = dir.path().to_path_buf();
        let work = root.join("work");
        std::fs::create_dir_all(&work).map_err(|err| why("no work directory", &err))?;
        let paths = humanitl_config::Paths::new(
            humanitl_config::Env::from_process()
                .with("XDG_RUNTIME_DIR", root.join("run").to_string_lossy())
                .with("HOME", root.join("host-home").to_string_lossy()),
        );
        let socket = paths.proxy_socket();
        let socket_dir = socket
            .parent()
            .ok_or_else(|| "the proxy socket has no directory".to_owned())?;
        std::fs::create_dir_all(socket_dir).map_err(|err| why("no runtime directory", &err))?;
        drop(
            std::os::unix::net::UnixListener::bind(&socket)
                .map_err(|err| why("no placeholder socket", &err))?,
        );
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
            .map_err(|err| why("the placeholder socket keeps its mode", &err))?;
        let ca = root.join("ca.crt");
        std::fs::write(
            &ca,
            b"-----BEGIN CERTIFICATE-----\nMIIBplaceholder\n-----END CERTIFICATE-----\n",
        )
        .map_err(|err| why("no placeholder CA", &err))?;
        let ca_bundle = root.join("ca-bundle.crt");
        std::fs::write(&ca_bundle, b"# bundle\n")
            .map_err(|err| why("no placeholder CA bundle", &err))?;
        let shim = shim_binary()?;
        let profile_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
        let profile = SandboxProfile::load(&profile_path)
            .map_err(|err| format!("the default profile does not load: {err}"))?;
        Ok(Self {
            _dir: dir,
            root,
            profile,
            work,
            socket,
            ca,
            ca_bundle,
            shim,
            paths,
        })
    }

    fn context(&self, command: &[&str]) -> humanitl_sandbox::SessionContext {
        humanitl_sandbox::SessionContext {
            session: SessionId::nil(),
            work_src: self.work.clone(),
            work_mode: humanitl_config::WorkMode::Rw,
            proxy_socket_src: self.socket.clone(),
            ca_cert_src: self.ca.clone(),
            ca_bundle_src: self.ca_bundle.clone(),
            shim_src: self.shim.clone(),
            session_env: vec![("HUMANITL_SESSION".to_owned(), SessionId::nil().to_string())],
            command: command.iter().map(OsString::from).collect(),
            files: Vec::new(),
            agent_depot: humanitl_sandbox::agent::depot::DepotSession::default(),
            agent_program: None,
        }
    }

    fn backend(&self) -> Result<humanitl_sandbox::Backend, String> {
        humanitl_sandbox::Backend::detect(
            humanitl_sandbox::BWRAP_BACKEND,
            self.paths.clone(),
            &humanitl_config::SandboxRef::default(),
        )
        .map_err(|err| format!("no usable bwrap: {err}"))
    }
}

/// The built shim next to the test binary.
fn shim_binary() -> Result<PathBuf, String> {
    let mut dir =
        std::env::current_exe().map_err(|err| format!("the test binary has no path: {err}"))?;
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let shim = dir.join("humanitl-shim");
    if shim.is_file() {
        Ok(shim)
    } else {
        Err(format!(
            "{} is not built; run cargo build --workspace",
            shim.display()
        ))
    }
}

fn captured() -> humanitl_sandbox::LaunchOptions {
    humanitl_sandbox::LaunchOptions {
        stdio: humanitl_sandbox::StdioMode::Capture,
        ..humanitl_sandbox::LaunchOptions::default()
    }
}
