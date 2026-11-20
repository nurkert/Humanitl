//! The Antigravity CLI adapter (HUM-517).
//!
//! Checked is what the adapter hands the launcher: command, launcher,
//! environment, files (settings seed, briefing, skill), preflight and
//! passthrough rule, all pure functions of the [`AgentContext`], and the
//! depot recipe with its refusals. One test at the end starts a real sandbox
//! with a stand-in for `agy` that checks its environment and settings,
//! without any network and without the vendor's program, beside a fake host
//! home that carries a planted `~/.gemini`, an Antigravity configuration, a
//! keyring file and a fake session bus socket.

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
use humanitl_sandbox::agent::agy::{
    ACCOUNT_ENDPOINT, API_KEY_DST, DOCS_URL, ENV_API_KEY, ENV_BASE_URL, ENV_DISABLE_AUTO_UPDATE,
    ENV_SSL_CERT_FILE, INSTALL_COMMAND, KEY_HEADER, KEY_START, MODEL_PROVIDER, OUTER_BOUNDARY_DE,
    OUTER_BOUNDARY_EN, SETTINGS_SEED_DST, briefing_dst, file_targets, is_direct_api, settings_dst,
    skill_dst,
};
use humanitl_sandbox::agent::{cloud, depot, files_on_reserved_targets, shown_program};
use humanitl_sandbox::{
    AdapterRegistry, AgentAdapter, AgentContext, AgentKey, AntigravityAdapter, SandboxBackend,
    SandboxProfile, SandboxView, files_inside_work,
};

/// A key long enough for `ApiKey::new`.
const KEY: &str = "AIza-test-0123456789abcdefghij";

/// The home of the agent in the sandbox, as the shipped profile sets it.
const HOME: &str = "/home/agent";

/// What the planted host login carries; it must appear nowhere in the
/// sandbox.
const HOST_MARKER: &str = "host-agy-marker-5c19b7";

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
    ctx.llm.api_key_ref = Some("secret-service:humanitl/gemini".to_owned());
    ctx
}

fn env_map(ctx: &AgentContext) -> BTreeMap<String, String> {
    AntigravityAdapter::new().env(ctx).into_iter().collect()
}

fn words(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect()
}

fn file_at(ctx: &AgentContext, dst: &Path) -> String {
    let files = AntigravityAdapter::new().files(ctx).unwrap();
    let file = files
        .iter()
        .find(|file| file.dst == dst)
        .unwrap_or_else(|| panic!("{} is missing", dst.display()));
    String::from_utf8(file.content.clone()).unwrap()
}

fn seed(ctx: &AgentContext) -> serde_json::Value {
    serde_json::from_str(&file_at(ctx, Path::new(SETTINGS_SEED_DST))).unwrap()
}

#[test]
fn the_registry_knows_agy() {
    let registry = AdapterRegistry::builtin();
    let adapter = registry.get("agy").expect("agy is registered");
    assert_eq!(adapter.program(), "agy");
    assert_eq!(adapter.install_command(), INSTALL_COMMAND);
    assert!(INSTALL_COMMAND.contains("https://antigravity.google/cli/install.sh"));
    assert_eq!(adapter.docs_url(), DOCS_URL);
    assert!(adapter.is_fullscreen_tui());
    assert!(adapter.is_native_program(), "one native executable");
    assert!(adapter.is_agent_command(&[OsString::from("/home/u/.local/bin/agy")]));
    assert!(!adapter.is_agent_command(&[OsString::from("bash")]));
    assert_eq!(adapter.key_header(&keyed(None).llm), KEY_HEADER);
    assert_eq!(KEY_HEADER, "x-goog-api-key");
}

#[test]
fn the_command_is_agy_or_agent_command() {
    let adapter = AntigravityAdapter::new();
    assert_eq!(words(&adapter.command(&context(None))), ["agy"]);
    let own = context(None).with_command_override(Some(vec![
        OsString::from("agy"),
        OsString::from("--model"),
        OsString::from("x"),
    ]));
    assert_eq!(words(&adapter.command(&own)), ["agy", "--model", "x"]);
}

/// The env kit: the updater off, the sandbox's CA for Go's TLS, the sandbox's
/// home; never a key and never the host's home.
///
/// Mutation proofs: `SSL_CERT_FILE` dropped from `env` turns the CA assert
/// red ("CA missing"); `GEMINI_API_KEY` with the key in `env` turns the last
/// loop red (`bwrap` carries every variable of the plan as `--setenv` in its
/// `argv`).
#[test]
fn the_env_kit_points_at_the_sandbox_and_carries_no_key() {
    let ctx = keyed(None).with_host_home(Some(PathBuf::from("/home/u")));
    let env = env_map(&ctx);
    assert_eq!(env[ENV_DISABLE_AUTO_UPDATE], "true");
    assert_eq!(env["HOME"], HOME);
    assert_eq!(
        env[ENV_SSL_CERT_FILE],
        ctx.ca_path_sandbox.to_string_lossy(),
        "the sandbox's CA for Go's crypto/x509"
    );
    assert_eq!(
        ctx.ca_path_sandbox,
        Path::new(humanitl_sandbox::profile::CA_CERT_DST)
    );
    assert!(!env.contains_key(ENV_BASE_URL), "the Gemini API is direct");
    assert!(!env.contains_key("DBUS_SESSION_BUS_ADDRESS"));
    for (name, value) in &env {
        assert!(!value.contains(KEY), "the key reached the plan as {name}");
        assert_ne!(name, ENV_API_KEY);
        assert!(
            !value.contains("/home/u"),
            "{name}={value} names the host's home"
        );
    }
}

/// A gateway gets `GOOGLE_GEMINI_BASE_URL` without user, password, query and
/// API version, on the loopback under the reserved name; without a key the
/// variable is not set, because only `modelProvider = "gemini"` reads it.
#[test]
fn the_base_url_follows_a_gateway_with_a_key() {
    let env = env_map(&keyed(Some(
        "https://user:pw@gw.example.com/gemini/v1beta?x=1",
    )));
    assert_eq!(env[ENV_BASE_URL], "https://gw.example.com/gemini");
    let env = env_map(&keyed(Some("http://127.0.0.1:4000")));
    assert_eq!(
        env[ENV_BASE_URL],
        format!("http://{}:4000", humanitl_core::LLM_ALIAS_HOST)
    );
    let env = env_map(&keyed(Some(
        "https://generativelanguage.googleapis.com/v1beta",
    )));
    assert!(!env.contains_key(ENV_BASE_URL), "the Gemini API is direct");
    let env = env_map(&context(Some("https://gw.example.com")));
    assert!(!env.contains_key(ENV_BASE_URL), "no key, no API mode");
}

#[test]
fn only_the_gemini_api_itself_is_direct() {
    for direct in [
        "https://generativelanguage.googleapis.com",
        "https://generativelanguage.googleapis.com/",
        "https://generativelanguage.googleapis.com/v1beta",
        "https://generativelanguage.googleapis.com:443/v1",
    ] {
        assert!(is_direct_api(&url::Url::parse(direct).unwrap()), "{direct}");
    }
    for gateway in [
        "http://generativelanguage.googleapis.com",
        "https://generativelanguage.googleapis.com:8443",
        "https://generativelanguage.googleapis.com/proxy",
        "https://generativelanguage.googleapis.com/?key=x",
        "https://u:p@generativelanguage.googleapis.com",
        "https://gw.example.com",
    ] {
        assert!(
            !is_direct_api(&url::Url::parse(gateway).unwrap()),
            "{gateway}"
        );
    }
}

/// The settings seed: `modelProvider = "gemini"` only with a key, the model
/// only with a gateway, the feedback survey off; never the key.
#[test]
fn the_settings_seed_selects_the_api_key_only_with_a_key() {
    let with_key = seed(&keyed(None));
    assert_eq!(with_key["modelProvider"], MODEL_PROVIDER);
    assert_eq!(with_key["showFeedbackSurvey"], false);
    assert!(with_key.get("model").is_none());
    assert!(!with_key.to_string().contains(KEY), "the key in the seed");

    let without = seed(&context(None));
    assert!(
        without.get("modelProvider").is_none(),
        "without a key agy would exit at start: {without}"
    );
    assert_eq!(without["showFeedbackSurvey"], false);

    let mut gateway = keyed(Some("https://gw.example.com"));
    gateway.llm.models = vec!["gemini-x".to_owned(), "other".to_owned()];
    assert_eq!(seed(&gateway)["model"], "gemini-x");
}

/// The launcher stands in front of the agent with and without a key: it
/// writes the seed into the settings and, if the key file is there, moves the
/// key into `GEMINI_API_KEY`. The key itself stands in none of the words.
///
/// Mutation proof ("key as argument"): a launcher that writes the key into
/// its script turns the last loop red.
#[test]
fn the_launcher_writes_the_settings_and_moves_the_key() {
    let adapter = AntigravityAdapter::new();
    let launcher = words(&adapter.launcher(&keyed(None)));
    assert_eq!(launcher, ["/bin/sh", "-c", KEY_START, "agy"]);
    assert_eq!(words(&adapter.launcher(&context(None))), launcher);
    let withheld = context(None).with_llm_key(AgentKey::Withheld);
    assert_eq!(words(&adapter.launcher(&withheld)), launcher);
    assert!(KEY_START.contains(API_KEY_DST));
    assert!(KEY_START.contains(SETTINGS_SEED_DST));
    assert!(KEY_START.contains("$HOME/.gemini/antigravity-cli/settings.json"));
    assert_eq!(
        settings_dst(Path::new("$HOME")),
        Path::new("$HOME/.gemini/antigravity-cli/settings.json")
    );
    assert!(KEY_START.contains("export GEMINI_API_KEY"));
    for word in &launcher {
        assert!(!word.contains(KEY), "the key reached argv: {word}");
    }
    let mut command = adapter.launcher(&keyed(None));
    command.push(OsString::from("/run/humanitl/agent/agy"));
    assert_eq!(
        shown_program(&command).map(|program| program.to_string_lossy().into_owned()),
        Some("/run/humanitl/agent/agy".to_owned())
    );
}

/// The key file, the seed, briefing and skill under the sandbox's home;
/// nothing in `/work`, nothing on a reserved target, nothing in the host's
/// home.
#[test]
fn the_files_are_key_seed_briefing_and_skill() {
    let ctx = keyed(None).with_host_home(Some(PathBuf::from("/home/u")));
    let files = AntigravityAdapter::new().files(&ctx).unwrap();
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
    assert_eq!(
        briefing_dst(home),
        Path::new("/home/agent/.gemini/GEMINI.md")
    );
    assert_eq!(
        skill_dst(home),
        Path::new("/home/agent/.gemini/antigravity-cli/skills/humanitl/SKILL.md")
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
    assert!(
        files.iter().all(|file| file.dst != settings_dst(home)),
        "the settings are written by the launcher, writable"
    );

    let silent = context(None).with_briefing(AgentBriefing { enabled: false });
    let quiet: Vec<PathBuf> = AntigravityAdapter::new()
        .files(&silent)
        .unwrap()
        .into_iter()
        .map(|file| file.dst.clone())
        .collect();
    assert_eq!(quiet, file_targets(home, false));
}

/// The briefing names the passthrough and the sentence about the outer
/// boundary, in the person's language.
#[test]
fn the_briefing_names_the_outer_boundary() {
    let text = file_at(&keyed(None), &briefing_dst(Path::new(HOME)));
    assert!(text.contains("Blocked by Humanitl."), "{text}");
    assert!(text.contains(OUTER_BOUNDARY_EN), "{text}");
    assert!(
        text.contains("generativelanguage.googleapis.com:443"),
        "the briefing names the passthrough: {text}"
    );
    let german = keyed(None).with_language(Language::De);
    assert!(file_at(&german, &briefing_dst(Path::new(HOME))).contains(OUTER_BOUNDARY_DE));
    let account = file_at(&context(None), &briefing_dst(Path::new(HOME)));
    assert!(
        !account.contains("cloudcode-pa.googleapis.com"),
        "nothing passes without a key: {account}"
    );
}

/// The passthrough: `POST` and `GET` on the model resources of the Gemini
/// API with a key, of the gateway below its prefix with an endpoint; none
/// without a key, where the sign-in is held.
#[test]
fn the_passthrough_covers_the_model_resources_with_a_key_only() {
    let adapter = AntigravityAdapter::new();
    let direct = adapter.llm_passthrough(&keyed(None).llm).expect("a rule");
    assert!(direct.passthrough_llm);
    assert!(
        !direct.allow_private,
        "the Gemini API is no private address"
    );
    assert_eq!(
        direct.matcher.host,
        humanitl_core::rule::HostPattern::Exact(
            humanitl_core::HostName::parse("generativelanguage.googleapis.com").unwrap()
        )
    );
    assert_eq!(
        direct.matcher.path_prefixes,
        [
            "/v1beta/models?",
            "/v1beta/models/",
            "/v1/models?",
            "/v1/models/"
        ]
    );
    assert_eq!(
        direct.matcher.methods,
        Some(vec![Method::POST, Method::GET])
    );
    assert_eq!(direct.matcher.port, Some(443));
    assert!(direct.matcher.path.is_none());

    let gateway = adapter
        .llm_passthrough(&keyed(Some("http://10.0.0.5:4000/gemini/v1beta")).llm)
        .expect("a rule");
    assert!(gateway.allow_private);
    assert_eq!(
        gateway.matcher.path_prefixes,
        [
            "/gemini/v1beta/models?",
            "/gemini/v1beta/models/",
            "/gemini/v1/models?",
            "/gemini/v1/models/"
        ]
    );
    assert_eq!(gateway.matcher.port, Some(4000));

    assert!(adapter.llm_passthrough(&context(None).llm).is_none());
    assert!(
        adapter
            .llm_passthrough(&context(Some("http://10.0.0.5:4000")).llm)
            .is_none(),
        "an endpoint without a key: agy signs in with an account"
    );
}

/// The notice of HUM-516 names the provider the inference really goes to:
/// the Gemini API with a key, Google's account endpoint without; none for a
/// local endpoint with a key.
#[test]
fn the_cloud_notice_names_the_resolved_provider() {
    let mut config = humanitl_config::Config::default();
    config.agent.adapter = "agy".to_owned();
    let found = cloud::cloud_inference(&config).expect("a cloud provider");
    assert_eq!(
        found.provider,
        url::Url::parse(ACCOUNT_ENDPOINT)
            .unwrap()
            .host_str()
            .unwrap()
    );
    config.llm.api_key_ref = Some("secret-service:humanitl/gemini".to_owned());
    let found = cloud::cloud_inference(&config).expect("a cloud provider");
    assert_eq!(found.provider, "generativelanguage.googleapis.com");
    config.llm.endpoint = Some(url::Url::parse("http://127.0.0.1:11434").unwrap());
    assert_eq!(cloud::cloud_inference(&config), None);
}

/// The preflight names `agy` and the vendor's installer when it is missing.
#[test]
fn the_preflight_names_the_installer_when_agy_is_missing() {
    let empty = tempfile::tempdir().unwrap();
    let dir = empty.path().display().to_string();
    let ctx = context(None)
        .with_host_path(Some(OsString::from(&dir)))
        .with_sandbox_view(SandboxView::same_path(
            vec![empty.path().to_path_buf()],
            Vec::new(),
        ))
        .with_sandbox_path(Some(OsString::from(&dir)));
    let found = AntigravityAdapter::new().preflight(&ctx);
    let missing = found
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_001")
        .unwrap_or_else(|| panic!("AGENT_001 in {found:?}"));
    // A native program: the fix points `agent.command` at it, and the
    // finding names the installer beside it (HUM-278).
    assert!(
        matches!(&missing.fix, Some(FixAction::CopyCommand(command)) if command.contains("agy")),
        "{:?}",
        missing.fix
    );
    assert!(
        format!("{missing:?}").contains(INSTALL_COMMAND),
        "the installer is named: {missing:?}"
    );
    assert_eq!(missing.docs.as_deref(), Some(DOCS_URL));
}

/// The depot recipe: API key only, no configuration variable, the updater
/// off, no credential file, no install recipe, with its reason (HUM-566).
#[test]
fn the_depot_recipe_is_api_key_only() {
    let recipe = AntigravityAdapter::new()
        .depot_recipe()
        .expect("a depot recipe");
    assert_eq!(recipe.login, depot::LoginFlow::ApiKeyOnly);
    assert_eq!(recipe.config_env, None, "no variable moves ~/.gemini");
    assert_eq!(recipe.update_off, &[(ENV_DISABLE_AUTO_UPDATE, "true")]);
    assert_eq!(recipe.credential_file, None);
    assert_eq!(recipe.credential_format, None);
    assert_eq!(recipe.login_run, None);
    // Decided in HUM-566: no pinned, checkable release; the reason is named.
    assert!(recipe.install.is_none());
    assert!(
        recipe
            .install_refused
            .is_some_and(|why| why.contains("no way to fetch a chosen one")),
        "{:?}",
        recipe.install_refused
    );
    let known = depot::recipe::known("agy").unwrap();
    assert!(!depot::account::login_declared(known));
}

/// `agents login agy` answers `AGENT_014` with why and the command that
/// stores a key; a configuration run `AGENT_015`, because nothing points
/// `agy` at the depot's copy. Neither touches the depot.
#[test]
fn login_and_configuration_of_agy_in_the_depot_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
        "XDG_DATA_HOME",
        root.path().join("data").display().to_string(),
    )]));
    let known = depot::recipe::known("agy").unwrap();
    let Err(login) = depot::account::prepare(
        &paths,
        known,
        depot::MaintenanceTask::Login,
        SessionId::nil(),
        None,
    ) else {
        panic!("a login of agy was prepared");
    };
    assert_eq!(login.code.as_str(), "AGENT_014");
    assert!(login.why.contains("API key"), "{}", login.why);
    assert!(login.why.contains("session bus"), "{}", login.why);
    assert!(
        matches!(&login.fix, Some(FixAction::CopyCommand(command)) if command == "humanitl llm key set"),
        "{:?}",
        login.fix
    );
    let Err(config) = depot::account::prepare(
        &paths,
        known,
        depot::MaintenanceTask::Config,
        SessionId::nil(),
        None,
    ) else {
        panic!("a configuration run of agy was prepared");
    };
    assert_eq!(config.code.as_str(), "AGENT_015");
    assert!(
        !root.path().join("data/humanitl/agents").exists(),
        "a refusal touches no depot"
    );
}

// --- Integration: a real sandbox with a stand-in for agy -------------------

/// What the stand-in checks, as far as Humanitl is concerned: the key in its
/// environment, the settings written by the launcher and writable, the
/// updater off, the CA readable, briefing and skill, nothing of the host's
/// login, no session bus, and that a nested user namespace, which `agy`'s
/// own sandbox needs, is refused. The fake host home comes as `$1`, the fake
/// bus socket as `$2`. The script names only the key's prefix: the whole key
/// in it would stand in `argv`.
const STAND_IN: &str = r#"
host_home=$1
fake_bus=$2
settings="$HOME/.gemini/antigravity-cli/settings.json"
case $GEMINI_API_KEY in AIza-test-*) [ "$GEMINI_API_KEY" = "$(cat /etc/humanitl/agy/llm-api-key)" ] && echo key=env ;; *) echo key=missing ;; esac
grep -q '"modelProvider": "gemini"' "$settings" && echo model_provider=gemini
grep -q '"showFeedbackSurvey": false' "$settings" && echo survey=off
[ "$AGY_CLI_DISABLE_AUTO_UPDATE" = true ] && echo update=off
grep -q 'BEGIN CERTIFICATE' "$SSL_CERT_FILE" && echo ca=readable
grep -q 'Blocked by Humanitl.' "$HOME/.gemini/GEMINI.md" && echo briefing=ok
head -n 2 "$HOME/.gemini/antigravity-cli/skills/humanitl/SKILL.md" | grep -q '^name: humanitl$' && echo skill=ok
[ -z "$DBUS_SESSION_BUS_ADDRESS" ] && echo dbus_env=absent
[ ! -e "$fake_bus" ] && [ ! -e /run/user ] && echo dbus_socket=absent
[ ! -e "$host_home" ] && echo host_home=absent
[ ! -e "$HOME/.gemini/oauth_creds.json" ] && [ ! -e "$HOME/.local/share/keyrings" ] && echo host_login=absent
if grep -rqs -D skip host-agy-marker-5c19b7 /home /etc /tmp /work /run 2>/dev/null; then echo marker=found; else echo marker=absent; fi
echo '{"trustedWorkspaces":["/work"]}' > "$settings" && echo settings=writable
mkdir -p "$HOME/.gemini/antigravity-cli/updater" && echo gemini_dir=writable
if echo x > /etc/humanitl/agy/settings.json 2>/dev/null; then echo seed=writable; else echo seed=read_only; fi
if echo x > /etc/humanitl/agy/llm-api-key 2>/dev/null; then echo key_file=writable; else echo key_file=read_only; fi
if command -v unshare >/dev/null 2>&1; then
    if unshare --user true 2>/dev/null; then echo nested_userns=allowed; else echo nested_userns=refused; fi
else
    echo nested_userns=refused
fi
"#;

/// A stand-in for `agy` behind the adapter's launcher in a real sandbox: the
/// key arrives in its environment and in no `argv`, the settings carry
/// `modelProvider = "gemini"` and are writable, the CA, briefing and skill
/// are there, nothing of a planted host `~/.gemini`, Antigravity
/// configuration or keyring is visible, and neither the session bus address
/// nor a bus socket enters (HUM-517, acceptance criterion 2); a nested user
/// namespace is refused, which is why `agy`'s own sandbox stays off.
///
/// Mutation proofs: an adapter whose `files` copies
/// `<host_home>/.gemini/oauth_creds.json` into the sandbox's `~/.gemini`
/// ("host login mounted") turns `host_login=absent` and `marker=absent`
/// red; a launcher with the key as a word turns the `argv` assert red
/// ("key as argument"); `DBUS_SESSION_BUS_ADDRESS` in the adapter's `env`
/// turns the plan's environment assert red, and a read-only mount of the
/// fake bus socket in the profile is refused with `SANDBOX_006` (the
/// runtime directory), so the plan does not build ("D-Bus socket
/// visible"). "CA missing" is the env kit test above: the shipped profile
/// carries `SSL_CERT_FILE` as well.
#[cfg(target_os = "linux")]
#[test]
fn a_stand_in_finds_its_key_and_nothing_of_the_host_login_in_a_real_sandbox() {
    let Some((fx, backend)) =
        sandbox_or_skip("a_stand_in_finds_its_key_and_nothing_of_the_host_login_in_a_real_sandbox")
    else {
        return;
    };
    let (host_home, fake_bus, _bus) = plant_host_login(&fx.root);

    let adapter = AntigravityAdapter::new();
    let ctx = keyed(None).with_host_home(Some(host_home.clone()));
    let mut command = adapter.launcher(&ctx);
    command.extend(
        ["sh", "-c", STAND_IN, "agy"]
            .iter()
            .map(OsString::from)
            .chain([
                host_home.clone().into_os_string(),
                fake_bus.clone().into_os_string(),
            ]),
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
        let word = word.to_string_lossy();
        assert!(!word.contains(KEY), "the key reached argv: {word}");
    }
    // The bus path stands once, as the stand-in's last argument; any other
    // word naming it would be a bind or a variable.
    let bus = fake_bus.to_string_lossy();
    let naming: Vec<_> = plan
        .argv
        .iter()
        .filter(|word| word.to_string_lossy().contains(&*bus))
        .collect();
    assert_eq!(naming, [&fake_bus.clone().into_os_string()], "{naming:?}");
    assert_eq!(plan.argv.last(), Some(&fake_bus.clone().into_os_string()));
    for (name, value) in &plan.env {
        assert!(!value.contains(KEY), "the key reached the plan as {name}");
        assert_ne!(name, "DBUS_SESSION_BUS_ADDRESS");
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
        "model_provider=gemini",
        "survey=off",
        "update=off",
        "ca=readable",
        "briefing=ok",
        "skill=ok",
        "dbus_env=absent",
        "dbus_socket=absent",
        "host_home=absent",
        "host_login=absent",
        "marker=absent",
        "settings=writable",
        "gemini_dir=writable",
        "seed=read_only",
        "key_file=read_only",
        "nested_userns=refused",
    ] {
        assert!(stdout.contains(line), "{line} is missing in:\n{stdout}");
    }
    assert_eq!(checks.len(), 3, "{checks:?}");
    for check in &checks {
        assert!(
            check.passed,
            "guarantee {:?} failed with agy's files: {check:?}",
            check.check
        );
    }
}

/// The fake host home with a planted login, an Antigravity configuration
/// and a keyring file, and a fake session bus next to the proxy socket in
/// the fake runtime directory; never the developer's own home or
/// `/run/user/<uid>/bus`. The listener keeps the socket alive.
fn plant_host_login(root: &Path) -> (PathBuf, PathBuf, std::os::unix::net::UnixListener) {
    // The fake host home with a planted login; never the developer's own.
    let host_home = root.join("host-home");
    for (file, content) in [
        (
            ".gemini/antigravity-cli/settings.json",
            format!("{{\"model\": \"{HOST_MARKER}\"}}"),
        ),
        (
            ".gemini/oauth_creds.json",
            format!("{{\"refresh_token\": \"{HOST_MARKER}\"}}"),
        ),
        (".gemini/GEMINI.md", HOST_MARKER.to_owned()),
        (
            ".config/Antigravity/User/settings.json",
            format!("{{\"x\": \"{HOST_MARKER}\"}}"),
        ),
        (
            ".local/share/keyrings/login.keyring",
            HOST_MARKER.to_owned(),
        ),
    ] {
        let path = host_home.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }
    // A fake session bus next to the proxy socket, in the fake runtime
    // directory; never the developer's own `/run/user/<uid>/bus`.
    let fake_bus = root.join("run/bus");
    std::fs::create_dir_all(fake_bus.parent().unwrap()).unwrap();
    let bus = std::os::unix::net::UnixListener::bind(&fake_bus).unwrap();

    (host_home, fake_bus, bus)
}

/// Skips without a usable sandbox or shim, and refuses to skip under CI;
/// the same pattern as in `codex_adapter.rs`.
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
