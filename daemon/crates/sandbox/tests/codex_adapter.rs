//! The Codex adapter (HUM-515).
//!
//! Checked is what the adapter hands the launcher: command, launcher,
//! environment, files (`config.toml` under `CODEX_HOME`, the managed
//! requirements, briefing and skill), preflight and passthrough rule, all
//! pure functions of the [`AgentContext`]. One test at the end starts a real
//! sandbox with a stand-in for `codex` that checks its environment and
//! configuration, without any network and without the vendor's program,
//! beside a fake host home that carries a planted `~/.codex`.

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
use humanitl_sandbox::agent::codex::{
    API_KEY_DST, CHATGPT_CODEX_BASE_URL, DOCS_URL, ENV_API_KEY, ENV_CA_CERTIFICATE, ENV_CODEX_HOME,
    INSTALL_COMMAND, KEY_START, OUTER_BOUNDARY_DE, OUTER_BOUNDARY_EN, PROJECT_TRUST, PROVIDER_ID,
    SANDBOX_MODE, config_dst, file_targets, skill_dst,
};
use humanitl_sandbox::agent::{cloud, depot, files_on_reserved_targets, shown_program};
use humanitl_sandbox::{
    AdapterRegistry, AgentAdapter, AgentContext, AgentKey, CodexAdapter, SandboxBackend,
    SandboxProfile, SandboxView, files_inside_work,
};

/// A key long enough for `ApiKey::new`.
const KEY: &str = "sk-proj-test-0123456789abcdef";

/// The home of the agent in the sandbox, as the shipped profile sets it.
const HOME: &str = "/home/agent";

/// What the planted host login carries; it must appear nowhere in the
/// sandbox.
const HOST_MARKER: &str = "host-codex-marker-7d2e41";

fn context(endpoint: Option<&str>) -> AgentContext {
    let llm = LlmConfig {
        endpoint: endpoint.map(|text| url::Url::parse(text).unwrap()),
        ..LlmConfig::default()
    };
    AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm)
}

/// A context with a key from the keyring, as the daemon builds it: the key
/// and its reference.
fn keyed(endpoint: Option<&str>) -> AgentContext {
    let mut ctx = context(endpoint).with_llm_key(AgentKey::Key(ApiKey::new(KEY).unwrap()));
    ctx.llm.api_key_ref = Some("secret-service:humanitl/openai".to_owned());
    ctx
}

fn env_map(ctx: &AgentContext) -> BTreeMap<String, String> {
    CodexAdapter::new().env(ctx).into_iter().collect()
}

fn words(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect()
}

fn file_at(ctx: &AgentContext, dst: &Path) -> String {
    let files = CodexAdapter::new().files(ctx).unwrap();
    let file = files
        .iter()
        .find(|file| file.dst == dst)
        .unwrap_or_else(|| panic!("{} is missing", dst.display()));
    String::from_utf8(file.content.clone()).unwrap()
}

fn config_toml(ctx: &AgentContext) -> toml::Table {
    toml::from_str(&file_at(ctx, &config_dst(Path::new(HOME)))).unwrap()
}

fn value<'a>(table: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
    let mut parts = path.split('.');
    let mut current = table.get(parts.next()?)?;
    for part in parts {
        current = current.as_table()?.get(part)?;
    }
    Some(current)
}

#[test]
fn the_registry_knows_codex() {
    let registry = AdapterRegistry::builtin();
    let adapter = registry.get("codex").expect("codex is registered");
    assert_eq!(adapter.program(), "codex");
    assert_eq!(adapter.install_command(), INSTALL_COMMAND);
    assert!(INSTALL_COMMAND.contains("npm install -g --ignore-scripts @openai/codex"));
    assert_eq!(adapter.docs_url(), DOCS_URL);
    assert!(adapter.is_fullscreen_tui());
    assert!(
        !adapter.is_native_program(),
        "the npm package is a Node launcher"
    );
    assert!(adapter.is_agent_command(&[OsString::from("/usr/bin/codex")]));
    assert!(!adapter.is_agent_command(&[OsString::from("bash")]));
}

#[test]
fn the_command_is_codex_or_agent_command() {
    let adapter = CodexAdapter::new();
    assert_eq!(words(&adapter.command(&context(None))), ["codex"]);
    let own = context(None).with_command_override(Some(vec![
        OsString::from("codex"),
        OsString::from("--model"),
        OsString::from("o4"),
    ]));
    assert_eq!(words(&adapter.command(&own)), ["codex", "--model", "o4"]);
}

/// The env kit: `CODEX_HOME` in the sandbox's own home and the sandbox's CA;
/// never a key.
///
/// Mutation proofs: `CODEX_CA_CERTIFICATE` dropped from `env` turns the CA
/// assert red ("CA missing"); `OPENAI_API_KEY` with the key in `env` turns
/// the last loop red (`bwrap` carries every variable of the plan as
/// `--setenv` in its `argv`).
#[test]
fn the_env_kit_points_at_the_sandbox_and_carries_no_key() {
    let ctx = keyed(None);
    let env = env_map(&ctx);
    assert_eq!(env[ENV_CODEX_HOME], format!("{HOME}/.codex"));
    assert_eq!(env["HOME"], HOME);
    assert_eq!(
        env[ENV_CA_CERTIFICATE],
        ctx.ca_path_sandbox.to_string_lossy(),
        "the sandbox's CA joins Codex's native roots"
    );
    assert_eq!(
        ctx.ca_path_sandbox,
        Path::new(humanitl_sandbox::profile::CA_CERT_DST)
    );
    for (name, value) in &env {
        assert!(!value.contains(KEY), "the key reached the plan as {name}");
        assert_ne!(name, ENV_API_KEY);
    }
}

/// `CODEX_HOME` follows the home the agent really sees, never the host's.
///
/// Mutation proof ("host `CODEX_HOME`"): `CODEX_HOME` from the host's home
/// turns the first assert red.
#[test]
fn codex_home_follows_the_sandbox_home_and_not_the_host() {
    let ctx = context(None)
        .with_home(PathBuf::from("/home/other"))
        .with_host_home(Some(PathBuf::from("/home/u")));
    let env = env_map(&ctx);
    assert_eq!(env[ENV_CODEX_HOME], "/home/other/.codex");
    for (name, value) in &env {
        assert!(
            !value.contains("/home/u"),
            "{name}={value} names the host's home"
        );
    }
    let files = CodexAdapter::new().files(&ctx).unwrap();
    assert!(
        files
            .iter()
            .any(|file| file.dst == Path::new("/home/other/.codex/config.toml"))
    );
}

/// `config.toml` under `CODEX_HOME` turns off update check, analytics and
/// feedback, keeps credentials in a file there, sets `danger-full-access`
/// and the project's trust.
///
/// Mutation proof ("update check on"): `check_for_update_on_startup = true`
/// turns the first assert red.
#[test]
fn config_toml_turns_off_the_side_traffic() {
    let config = config_toml(&context(None));
    assert_eq!(
        value(&config, "check_for_update_on_startup"),
        Some(&toml::Value::Boolean(false))
    );
    assert_eq!(
        value(&config, "analytics.enabled"),
        Some(&toml::Value::Boolean(false))
    );
    assert_eq!(
        value(&config, "feedback.enabled"),
        Some(&toml::Value::Boolean(false))
    );
    assert_eq!(
        value(&config, "cli_auth_credentials_store").and_then(toml::Value::as_str),
        Some("file")
    );
    assert_eq!(
        value(&config, "sandbox_mode").and_then(toml::Value::as_str),
        Some(SANDBOX_MODE)
    );
    assert_eq!(SANDBOX_MODE, "danger-full-access");
    let trust = config["projects"]["/work"]["trust_level"].as_str();
    assert_eq!(trust, Some(PROJECT_TRUST));
    assert!(
        !config.contains_key("model_provider"),
        "without key and endpoint Codex keeps its own provider: {config:?}"
    );
}

/// With a key the adapter declares its own provider towards the `OpenAI`
/// API that reads the key from `OPENAI_API_KEY`; with an endpoint the base
/// is the endpoint's `OpenAI` surface without user, password and query, on
/// the loopback under the reserved name; without a key no `env_key`.
#[test]
fn the_provider_follows_key_and_endpoint() {
    let direct = config_toml(&keyed(None));
    assert_eq!(direct["model_provider"].as_str(), Some(PROVIDER_ID));
    let provider = &direct["model_providers"][PROVIDER_ID];
    assert_eq!(
        provider["base_url"].as_str(),
        Some("https://api.openai.com/v1")
    );
    assert_eq!(provider["env_key"].as_str(), Some(ENV_API_KEY));
    assert_eq!(provider["wire_api"].as_str(), Some("responses"));
    assert!(!direct.to_string().contains(KEY), "the key in config.toml");

    let mut gateway = keyed(Some("https://user:pw@gw.example.com/openai?x=1"));
    gateway.llm.models = vec!["gpt-5-codex".to_owned(), "other".to_owned()];
    let config = config_toml(&gateway);
    let provider = &config["model_providers"][PROVIDER_ID];
    assert_eq!(
        provider["base_url"].as_str(),
        Some("https://gw.example.com/openai/v1")
    );
    assert_eq!(config["model"].as_str(), Some("gpt-5-codex"));

    let local = config_toml(&context(Some("http://127.0.0.1:11434/v1")));
    let provider = &local["model_providers"][PROVIDER_ID];
    assert_eq!(
        provider["base_url"].as_str(),
        Some(format!("http://{}:11434/v1", humanitl_core::LLM_ALIAS_HOST).as_str())
    );
    assert!(provider.get("env_key").is_none(), "no key, no env_key");
}

/// Under Linux the managed requirements enforce update check, credential
/// store and feedback for a depot session too, and carry the briefing with
/// the sentence about the outer boundary.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_requirements_carry_the_enforced_keys_and_the_briefing() {
    use humanitl_sandbox::agent::codex::REQUIREMENTS_DST;
    let ctx = keyed(None);
    let text = file_at(&ctx, Path::new(REQUIREMENTS_DST));
    let requirements: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(
        value(&requirements, "check_for_update_on_startup"),
        Some(&toml::Value::Boolean(false))
    );
    assert_eq!(
        value(&requirements, "cli_auth_credentials_store").and_then(toml::Value::as_str),
        Some("file")
    );
    assert_eq!(
        value(&requirements, "feedback.enabled"),
        Some(&toml::Value::Boolean(false))
    );
    let briefing = requirements["additional_developer_instructions"]
        .as_str()
        .unwrap();
    assert!(briefing.contains("Blocked by Humanitl."), "{briefing}");
    assert!(briefing.contains(OUTER_BOUNDARY_EN), "{briefing}");
    assert!(
        briefing.contains("api.openai.com:443"),
        "the briefing names the passthrough: {briefing}"
    );
    let german = keyed(None).with_language(Language::De);
    assert!(file_at(&german, Path::new(REQUIREMENTS_DST)).contains(OUTER_BOUNDARY_DE));

    let silent = context(None).with_briefing(AgentBriefing { enabled: false });
    let requirements: toml::Table =
        toml::from_str(&file_at(&silent, Path::new(REQUIREMENTS_DST))).unwrap();
    assert!(!requirements.contains_key("additional_developer_instructions"));
    assert_eq!(
        value(&requirements, "check_for_update_on_startup"),
        Some(&toml::Value::Boolean(false)),
        "the enforced keys hold without a briefing"
    );
}

/// Without a key no launcher; with one a shell in front of the agent that
/// reads the key file into `OPENAI_API_KEY`. The key itself stands in none
/// of the words.
///
/// Mutation proof ("key as argument"): a launcher that writes the key into
/// its script turns the last assert red.
#[test]
fn the_launcher_moves_the_key_from_its_file_into_the_environment() {
    let adapter = CodexAdapter::new();
    assert!(adapter.launcher(&context(None)).is_empty());
    let launcher = words(&adapter.launcher(&keyed(None)));
    assert_eq!(launcher, ["/bin/sh", "-c", KEY_START, "codex"]);
    assert!(KEY_START.contains(API_KEY_DST));
    assert!(KEY_START.contains("export OPENAI_API_KEY"));
    let withheld = context(None).with_llm_key(AgentKey::Withheld);
    assert_eq!(words(&adapter.launcher(&withheld)), launcher);
    for word in &launcher {
        assert!(!word.contains(KEY), "the key reached argv: {word}");
    }
    let mut command = adapter.launcher(&keyed(None));
    command.push(OsString::from("/usr/bin/codex"));
    assert_eq!(
        shown_program(&command).map(|program| program.to_string_lossy().into_owned()),
        Some("/usr/bin/codex".to_owned())
    );
}

/// The key file, `config.toml`, the requirements (Linux) and the skill;
/// nothing in `/work`, nothing on a reserved target, nothing in the host's
/// home.
#[test]
fn the_files_are_key_config_requirements_and_skill() {
    let ctx = keyed(None);
    let files = CodexAdapter::new().files(&ctx).unwrap();
    let key = files.iter().find(|file| file.secret).expect("the key file");
    assert_eq!(key.dst, Path::new(API_KEY_DST));
    assert_eq!(key.content, KEY.as_bytes());
    assert_eq!(files.iter().filter(|file| file.secret).count(), 1);

    let home = Path::new(HOME);
    let mut targets: Vec<PathBuf> = files
        .iter()
        .filter(|file| !file.secret)
        .map(|file| file.dst.clone())
        .collect();
    targets.sort();
    let mut expected = file_targets(home, true);
    expected.sort();
    assert_eq!(targets, expected);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        skill_dst(home),
        Path::new("/etc/codex/skills/humanitl/SKILL.md")
    );
    let skill = file_at(&ctx, &skill_dst(home));
    assert!(skill.starts_with("---\nname: humanitl\n"), "{skill}");

    assert!(files_inside_work(&files, Path::new("/work")).is_empty());
    assert!(files_on_reserved_targets(&files).is_empty());
    for file in &files {
        assert!(
            !file.dst.starts_with("/home/u"),
            "{} lies in the host's home",
            file.dst.display()
        );
    }
    let silent = context(None).with_briefing(AgentBriefing { enabled: false });
    let mut quiet: Vec<PathBuf> = CodexAdapter::new()
        .files(&silent)
        .unwrap()
        .into_iter()
        .map(|file| file.dst.clone())
        .collect();
    quiet.sort();
    let mut expected = file_targets(home, false);
    expected.sort();
    assert_eq!(quiet, expected);
}

/// The passthrough: `POST` and `GET` on exactly `/responses` and `/models`
/// below the resolved base: the `OpenAI` API with a key, the endpoint's
/// surface with an endpoint, the `ChatGPT` backend for a `ChatGPT` login.
#[test]
fn the_passthrough_covers_responses_and_models_only() {
    let adapter = CodexAdapter::new();
    let direct = adapter.llm_passthrough(&keyed(None).llm).expect("a rule");
    assert!(direct.passthrough_llm);
    assert!(
        !direct.allow_private,
        "the OpenAI API is no private address"
    );
    assert_eq!(
        direct.matcher.host,
        humanitl_core::rule::HostPattern::Exact(
            humanitl_core::HostName::parse("api.openai.com").unwrap()
        )
    );
    assert_eq!(
        direct.matcher.path_prefixes,
        ["/v1/responses?", "/v1/models?"]
    );
    assert_eq!(
        direct.matcher.methods,
        Some(vec![Method::POST, Method::GET])
    );
    assert_eq!(direct.matcher.port, Some(443));

    let gateway = adapter
        .llm_passthrough(&context(Some("http://10.0.0.5:4000/openai")).llm)
        .expect("a rule");
    assert!(gateway.allow_private);
    assert_eq!(
        gateway.matcher.path_prefixes,
        ["/openai/v1/responses?", "/openai/v1/models?"]
    );
    assert_eq!(gateway.matcher.port, Some(4000));

    let chatgpt = adapter.llm_passthrough(&context(None).llm).expect("a rule");
    assert_eq!(
        chatgpt.matcher.path_prefixes,
        [
            "/backend-api/codex/responses?",
            "/backend-api/codex/models?"
        ]
    );
    assert!(CHATGPT_CODEX_BASE_URL.starts_with("https://chatgpt.com/"));
}

/// The method-path pairs the agent needs, and what the one rule lets pass
/// besides (HUM-515, review): an own provider posts to `/responses` and gets
/// `/models`; the built-in provider of a `ChatGPT` login also opens its
/// WebSocket with `GET /responses`. The matcher pairs no method with a path,
/// so `POST /models` and `GET /responses` pass at the same origin, and
/// nothing else: no other method, no other path, no subpath.
#[test]
fn the_passthrough_pairs_are_the_ones_codex_needs_at_one_origin() {
    for ctx in [
        keyed(None),
        context(Some("http://10.0.0.5:4000")),
        context(None),
    ] {
        let rule = CodexAdapter::new().llm_passthrough(&ctx.llm).unwrap();
        let methods = rule.matcher.methods.clone().unwrap();
        assert_eq!(methods, [Method::POST, Method::GET], "no other method");
        assert_eq!(rule.matcher.path_prefixes.len(), 2, "no other path");
        for entry in &rule.matcher.path_prefixes {
            assert!(
                entry.ends_with("/responses?") || entry.ends_with("/models?"),
                "{entry}"
            );
            assert!(entry.ends_with('?'), "exact, no subpath: {entry}");
        }
        assert!(rule.matcher.path.is_none());
    }
}

/// The notice of HUM-516 names the provider the inference really goes to:
/// the `OpenAI` API with a key, `ChatGPT` without; none for a local endpoint.
#[test]
fn the_cloud_notice_names_the_resolved_provider() {
    let mut config = humanitl_config::Config::default();
    config.agent.adapter = "codex".to_owned();
    let found = cloud::cloud_inference(&config).expect("a cloud provider");
    assert_eq!(found.provider, "chatgpt.com");
    config.llm.api_key_ref = Some("secret-service:humanitl/openai".to_owned());
    let found = cloud::cloud_inference(&config).expect("a cloud provider");
    assert_eq!(found.provider, "api.openai.com");
    config.llm.endpoint = Some(url::Url::parse("http://127.0.0.1:11434").unwrap());
    assert_eq!(cloud::cloud_inference(&config), None);
}

/// The preflight names `codex` and the npm install when it is missing.
#[test]
fn the_preflight_names_the_installer_when_codex_is_missing() {
    let empty = tempfile::tempdir().unwrap();
    let dir = empty.path().display().to_string();
    let ctx = context(None)
        .with_host_path(Some(OsString::from(&dir)))
        .with_sandbox_view(SandboxView::same_path(
            vec![empty.path().to_path_buf()],
            Vec::new(),
        ))
        .with_sandbox_path(Some(OsString::from(&dir)));
    let found = CodexAdapter::new().preflight(&ctx);
    let missing = found
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_001")
        .unwrap_or_else(|| panic!("AGENT_001 in {found:?}"));
    assert!(
        matches!(&missing.fix, Some(FixAction::CopyCommand(command)) if command == INSTALL_COMMAND),
        "{:?}",
        missing.fix
    );
    assert_eq!(missing.docs.as_deref(), Some(DOCS_URL));
}

/// The depot recipe: `CODEX_HOME`, `auth.json` with its format from the
/// source, the device login against `auth.openai.com` without a callback,
/// the keys that run a command, and no install recipe until measured.
#[test]
fn the_depot_recipe_declares_home_login_and_format() {
    let recipe = CodexAdapter::new().depot_recipe().expect("a depot recipe");
    assert_eq!(recipe.config_env, Some(ENV_CODEX_HOME));
    assert_eq!(recipe.credential_file, Some("auth.json"));
    assert!(recipe.credential_format.is_some());
    assert_eq!(recipe.login, depot::LoginFlow::DeviceCode);
    assert!(recipe.install.is_none(), "not measured (HUM-553)");
    let login = recipe.login_run.expect("a login run");
    assert_eq!(login.args, ["login", "--device-auth"]);
    let hosts: Vec<&str> = login.hosts.iter().map(|host| host.host).collect();
    assert_eq!(hosts, ["auth.openai.com"]);
    assert!(login.hosts[0].evidence.starts_with("https://"));
    assert_eq!(login.callback, None, "the device flow opens no listener");
    assert!(login.device_flow_setting.is_some());
    for key in [
        "notify",
        "mcp_servers",
        "model_providers",
        "hooks",
        "profiles",
    ] {
        assert!(recipe.command_keys.contains(&key), "{key} is missing");
    }
}

// --- Integration: a real sandbox with a stand-in for codex -----------------

/// What the stand-in checks, as far as Humanitl is concerned: the key in its
/// environment, `CODEX_HOME` on the sandbox's home with the adapter's
/// `config.toml` and nothing else, the CA readable, requirements and skill,
/// nothing of the host's login, and that a nested user namespace, which
/// Codex's own sandbox needs, is refused. The fake host home comes as `$1`.
/// The script names only the key's prefix: the whole key in it would stand
/// in `argv`.
const STAND_IN: &str = r#"
host_home=$1
case $OPENAI_API_KEY in sk-proj-test-*) [ "$OPENAI_API_KEY" = "$(cat /etc/humanitl/codex/llm-api-key)" ] && echo key=env ;; *) echo key=missing ;; esac
echo "home=$CODEX_HOME"
grep -q '^check_for_update_on_startup = false$' "$CODEX_HOME/config.toml" && echo update_check=off
grep -q '^env_key = "OPENAI_API_KEY"$' "$CODEX_HOME/config.toml" && echo env_key=ok
grep -q '^sandbox_mode = "danger-full-access"$' "$CODEX_HOME/config.toml" && echo sandbox_mode=full
grep -q 'BEGIN CERTIFICATE' "$CODEX_CA_CERTIFICATE" && echo ca=readable
grep -q 'Blocked by Humanitl.' /etc/codex/requirements.toml && echo briefing=ok
head -n 2 /etc/codex/skills/humanitl/SKILL.md | grep -q '^name: humanitl$' && echo skill=ok
[ "$(ls -A "$CODEX_HOME")" = "config.toml" ] && echo codex_home=only_config
[ ! -e "$CODEX_HOME/auth.json" ] && [ ! -e "$HOME/.codex/auth.json" ] && echo host_login=absent
[ ! -e "$host_home" ] && echo host_home=absent
if grep -rqs -D skip host-codex-marker-7d2e41 /home /etc /tmp /work /run 2>/dev/null; then echo marker=found; else echo marker=absent; fi
mkdir -p "$CODEX_HOME/sessions" && echo '{}' > "$CODEX_HOME/auth.json" && echo codex_home=writable
if echo x > "$CODEX_HOME/config.toml" 2>/dev/null; then echo config=writable; else echo config=read_only; fi
if echo x > /etc/humanitl/codex/llm-api-key 2>/dev/null; then echo key_file=writable; else echo key_file=read_only; fi
if command -v unshare >/dev/null 2>&1; then
    if unshare --user true 2>/dev/null; then echo nested_userns=allowed; else echo nested_userns=refused; fi
else
    echo nested_userns=refused
fi
"#;

/// A stand-in for `codex` behind the adapter's launcher in a real sandbox:
/// the key arrives in its environment and in no `argv`, `config.toml` with
/// the side traffic off lies in `CODEX_HOME`, the CA, requirements and skill
/// are there, nothing of a planted host `~/.codex` is visible (HUM-515,
/// acceptance criterion 2), and a nested user namespace is refused, which
/// is why Codex's own sandbox stays off.
///
/// Mutation proofs: an adapter whose `files` copies
/// `<host_home>/.codex/auth.json` into `CODEX_HOME` ("host `CODEX_HOME`
/// mounted") turns the stand-in's checks red, `codex_home=only_config`
/// first; `CODEX_HOME` pointed at the host's `~/.codex`
/// turns `home=` red; without `CODEX_CA_CERTIFICATE` `ca=readable` is red;
/// `check_for_update_on_startup = true` turns `update_check=off` red.
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
    std::fs::create_dir_all(host_home.join(".codex")).unwrap();
    std::fs::write(
        host_home.join(".codex/auth.json"),
        format!("{{\"OPENAI_API_KEY\": \"{HOST_MARKER}\"}}"),
    )
    .unwrap();
    std::fs::write(
        host_home.join(".codex/config.toml"),
        format!("model = \"{HOST_MARKER}\"\n"),
    )
    .unwrap();

    let adapter = CodexAdapter::new();
    let ctx = keyed(None).with_host_home(Some(host_home.clone()));
    let mut command = adapter.launcher(&ctx);
    assert!(!command.is_empty(), "a key brings the launcher");
    command.extend(
        ["sh", "-c", STAND_IN, "codex"]
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
    let handle = backend.launch(&plan).expect("the sandbox starts");
    let checks = backend.isolation_check(&handle);
    let status = handle
        .wait_timeout(std::time::Duration::from_secs(30))
        .expect("the sandbox ends in time")
        .expect("the sandbox ran the command");
    let output = handle.output().expect("captured");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        status.success(),
        "{status}: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for line in [
        "key=env",
        "home=/home/agent/.codex",
        "update_check=off",
        "env_key=ok",
        "sandbox_mode=full",
        "ca=readable",
        "briefing=ok",
        "skill=ok",
        "codex_home=only_config",
        "host_login=absent",
        "host_home=absent",
        "marker=absent",
        "codex_home=writable",
        "config=read_only",
        "key_file=read_only",
        "nested_userns=refused",
    ] {
        assert!(stdout.contains(line), "{line} is missing in:\n{stdout}");
    }
    assert_eq!(checks.len(), 3, "{checks:?}");
    for check in &checks {
        assert!(
            check.passed,
            "guarantee {:?} failed with Codex's files: {check:?}",
            check.check
        );
    }
}

/// Skips without a usable sandbox or shim, and refuses to skip under CI;
/// the same pattern as in `claude_adapter.rs`.
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
        .map_err(|err| format!("no usable sandbox backend: {err}"))
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
