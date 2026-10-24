//! The pi adapter (HUM-274).
//!
//! Checked is what the adapter hands the launcher: command, environment,
//! files, preflight and passthrough rule, all pure functions of the
//! [`AgentContext`]. One test at the end starts a real sandbox with a fake
//! `pi` that does at start what pi 1.3.0 does, without any network.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::secret::ApiKey;
use humanitl_config::{AgentBriefing, LlmConfig};
use humanitl_core::{FixAction, SessionId, Severity};
use humanitl_sandbox::agent::files_on_reserved_targets;
use humanitl_sandbox::agent::opencode_models::{PLACEHOLDER_MODEL, PROVIDER_ID};
use humanitl_sandbox::agent::pi::{
    API_KEY_DST, DOCS_URL, INSTALL_COMMAND, NODE_DOWNLOAD_URL, VISIBLE_ENV, briefing_dst,
    file_targets, models_dst, settings_dst,
};
use humanitl_sandbox::agent::pi_config::NO_KEY_PLACEHOLDER;
use humanitl_sandbox::{
    AdapterRegistry, AgentAdapter, AgentContext, AgentKey, OpenCodeAdapter, PiAdapter,
    SandboxBackend, SandboxProfile, SandboxView, files_inside_work,
};
use serde_json::Value;

/// A key long enough for `ApiKey::new`.
const KEY: &str = "sk-pi-test-0123456789abcdef";

/// The home of the agent in the sandbox, as the shipped profile sets it.
const HOME: &str = "/home/agent";

/// A context with an endpoint and one model.
fn context(endpoint: Option<&str>) -> AgentContext {
    let llm = LlmConfig {
        endpoint: endpoint.map(|text| url::Url::parse(text).unwrap()),
        ..LlmConfig::default()
    };
    AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm)
        .with_models(vec!["qwen3".to_owned()])
}

fn env_map(ctx: &AgentContext) -> BTreeMap<String, String> {
    PiAdapter::new().env(ctx).into_iter().collect()
}

/// The content of the file at `dst`, as text.
fn file_at(ctx: &AgentContext, dst: &Path) -> String {
    let files = PiAdapter::new().files(ctx).unwrap();
    let file = files
        .iter()
        .find(|file| file.dst == dst)
        .unwrap_or_else(|| panic!("no file for {}", dst.display()));
    String::from_utf8(file.content.clone()).unwrap()
}

fn models_json(ctx: &AgentContext) -> Value {
    serde_json::from_str(&file_at(ctx, &models_dst(Path::new(HOME)))).unwrap()
}

fn provider(ctx: &AgentContext) -> Value {
    models_json(ctx)["providers"][PROVIDER_ID].clone()
}

#[test]
fn the_registry_knows_pi_next_to_opencode() {
    let registry = AdapterRegistry::builtin();
    assert_eq!(registry.ids(), vec!["opencode", "pi"]);
    let adapter = registry.get("pi").expect("the pi adapter");
    assert_eq!(adapter.id(), "pi");
    assert_eq!(adapter.program(), "pi");
    assert!(
        adapter.is_fullscreen_tui(),
        "pi draws the whole screen; `--ask terminal` refuses it (CLI_002)"
    );
    for named in ["pi", "/usr/local/bin/pi", "./pi"] {
        assert!(
            adapter.is_agent_command(&[OsString::from(named)]),
            "{named}"
        );
    }
    for other in ["pip", "pint", "opencode", "node"] {
        assert!(
            !adapter.is_agent_command(&[OsString::from(other)]),
            "{other}"
        );
    }
}

#[test]
fn pi_env_switches_off_everything_pi_does_on_its_own() {
    let env = env_map(&context(Some("http://192.168.1.50:11434")));
    assert_eq!(env["PI_OFFLINE"], "1");
    assert_eq!(env["PI_SKIP_VERSION_CHECK"], "1");
    assert_eq!(env["PI_TELEMETRY"], "0");
    assert_eq!(env["PI_CODING_AGENT_DIR"], "/home/agent/.pi/agent");
    assert_eq!(env["HOME"], HOME);
    assert_eq!(env["NODE_EXTRA_CA_CERTS"], "/etc/humanitl/ca.crt");
    assert_eq!(env["TERM"], "xterm-256color");
    for name in VISIBLE_ENV {
        assert!(
            env.contains_key(*name),
            "{name} is on the visible list but never set"
        );
    }
    for name in env.keys().filter(|name| name.starts_with("PI_")) {
        assert!(
            VISIBLE_ENV.contains(&name.as_str()),
            "{name} is set but the sandbox screen would hide it"
        );
    }
}

/// The agent directory follows the home of the profile, and the variable
/// points where the files are (HUM-274, pitfall 4).
#[test]
fn pi_files_follow_an_overridden_home() {
    let ctx = context(Some("http://192.168.1.50:11434")).with_home(PathBuf::from("/home/other"));
    assert_eq!(
        env_map(&ctx)["PI_CODING_AGENT_DIR"],
        "/home/other/.pi/agent"
    );
    let targets: Vec<PathBuf> = PiAdapter::new()
        .files(&ctx)
        .unwrap()
        .into_iter()
        .map(|file| file.dst.clone())
        .collect();
    assert_eq!(targets, file_targets(Path::new("/home/other"), true));
}

#[test]
fn pi_models_point_at_the_endpoint_of_the_user() {
    for (endpoint, base) in [
        ("http://192.168.1.50:11434", "http://192.168.1.50:11434/v1"),
        (
            "http://192.168.1.50:11434/v1",
            "http://192.168.1.50:11434/v1",
        ),
        (
            "http://192.168.1.50:11434/v1/",
            "http://192.168.1.50:11434/v1",
        ),
        // A path prefix stays (HUM-261), and `/v1` is not added twice.
        (
            "https://llm.example.net/api/v1/",
            "https://llm.example.net/api/v1",
        ),
    ] {
        let provider = provider(&context(Some(endpoint)));
        assert_eq!(
            provider["baseUrl"],
            Value::String(base.to_owned()),
            "{endpoint}"
        );
        assert_eq!(provider["api"], "openai-completions", "{endpoint}");
        assert_eq!(provider["models"][0]["id"], "qwen3", "{endpoint}");
    }
}

#[test]
fn pi_reads_the_key_through_a_file_and_nowhere_else() {
    let adapter = PiAdapter::new();
    let ctx = context(Some("https://api.example.com/v1"))
        .with_llm_key(AgentKey::Key(ApiKey::new(KEY).unwrap()));

    assert_eq!(
        provider(&ctx)["apiKey"],
        Value::String(format!("!cat {API_KEY_DST}")),
        "models.json names the command that reads the key file"
    );
    let files = adapter.files(&ctx).unwrap();
    let secret: Vec<_> = files.iter().filter(|file| file.secret).collect();
    assert_eq!(secret.len(), 1, "exactly one file carries the key");
    assert_eq!(secret[0].dst, Path::new(API_KEY_DST));
    assert_eq!(secret[0].content, KEY.as_bytes());
    for file in files.iter().filter(|file| !file.secret) {
        assert!(
            !String::from_utf8_lossy(&file.content).contains(KEY),
            "the key stands in {}",
            file.dst.display()
        );
    }
    for (name, value) in adapter.env(&ctx) {
        assert!(
            !value.contains(KEY),
            "the key reached the environment as {name}"
        );
    }
    for word in adapter.command(&ctx) {
        assert!(
            !word.to_string_lossy().contains(KEY),
            "the key reached argv"
        );
    }
}

/// A plan names the same files as a start, the key file empty; without a key
/// pi still needs a non-empty `apiKey`, or it offers no model at all.
#[test]
fn pi_without_a_key_gets_the_placeholder_and_a_plan_an_empty_file() {
    let none = context(Some("http://192.168.1.50:11434"));
    assert_eq!(provider(&none)["apiKey"], NO_KEY_PLACEHOLDER);
    assert!(
        !PiAdapter::new()
            .files(&none)
            .unwrap()
            .iter()
            .any(|file| file.secret || file.dst == Path::new(API_KEY_DST)),
        "no key, no key file"
    );

    let plan = context(Some("http://192.168.1.50:11434")).with_llm_key(AgentKey::Withheld);
    assert_eq!(
        provider(&plan)["apiKey"],
        Value::String(format!("!cat {API_KEY_DST}"))
    );
    let files = PiAdapter::new().files(&plan).unwrap();
    let secret = files
        .iter()
        .find(|file| file.dst == Path::new(API_KEY_DST))
        .expect("the plan names the key file");
    assert!(secret.secret && secret.content.is_empty());
}

#[test]
fn pi_settings_select_the_first_model_and_the_placeholder_warns() {
    let ctx = context(Some("http://192.168.1.50:11434"))
        .with_models(vec!["first".to_owned(), "second".to_owned()]);
    let settings: Value =
        serde_json::from_str(&file_at(&ctx, &settings_dst(Path::new(HOME)))).unwrap();
    assert_eq!(settings["defaultProvider"], PROVIDER_ID);
    assert_eq!(settings["defaultModel"], "first");
    assert_eq!(settings["enableInstallTelemetry"], false);

    let empty = context(Some("http://192.168.1.50:11434")).with_models(Vec::new());
    assert_eq!(provider(&empty)["models"][0]["id"], PLACEHOLDER_MODEL);
    let warning = PiAdapter::new()
        .preflight(&empty)
        .into_iter()
        .find(|diagnostic| diagnostic.code.as_str() == "LLM_004")
        .expect("an empty model list is LLM_004");
    assert_eq!(warning.severity, Severity::Warning);
}

#[test]
fn pi_briefing_lies_in_the_agent_directory_and_can_be_switched_off() {
    let ctx = context(Some("http://192.168.1.50:11434"));
    let text = file_at(&ctx, &briefing_dst(Path::new(HOME)));
    assert!(text.contains("Blocked by Humanitl."), "{text}");
    assert!(text.contains("192.168.1.50:11434"), "{text}");

    let off =
        context(Some("http://192.168.1.50:11434")).with_briefing(AgentBriefing { enabled: false });
    assert!(
        !PiAdapter::new()
            .files(&off)
            .unwrap()
            .iter()
            .any(|file| file.dst.ends_with("AGENTS.md")),
        "a switched-off briefing is no file at all"
    );
}

#[test]
fn pi_writes_nothing_into_work_and_covers_no_sandbox_path() {
    for key in [AgentKey::None, AgentKey::Withheld] {
        let files = PiAdapter::new()
            .files(&context(Some("http://192.168.1.50:11434")).with_llm_key(key))
            .unwrap();
        assert!(files_inside_work(&files, Path::new("/work")).is_empty());
        assert!(files_on_reserved_targets(&files).is_empty());
    }
}

/// The passthrough is the one of `OpenCode`: same id, same paths, same flags.
#[test]
fn pi_passthrough_is_the_shared_rule() {
    for endpoint in [
        "http://192.168.1.50:11434",
        "https://llm.example.net/api/v1/",
    ] {
        let llm = context(Some(endpoint)).llm;
        let pi = PiAdapter::new().llm_passthrough(&llm).expect("a rule");
        let opencode = OpenCodeAdapter::new()
            .llm_passthrough(&llm)
            .expect("a rule");
        assert_eq!(pi, opencode, "{endpoint}");
        assert!(pi.passthrough_llm && pi.bundled, "{endpoint}");
    }
    assert!(
        PiAdapter::new()
            .llm_passthrough(&context(None).llm)
            .is_none()
    );
}

/// An executable file with this text.
fn write_executable(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The script pi ships as `dist/bundle/cli.js`, as far as the preflight reads it.
const PI_SCRIPT: &str = "#!/usr/bin/env node\nimport \"./cli-runtime.js\";\n";

/// A context whose sandbox mounts exactly `mounted` and searches `sandbox_path`.
fn sandboxed(mounted: &[&Path], sandbox_path: &str, host_path: &str) -> AgentContext {
    context(Some("http://192.168.1.50:11434"))
        .with_host_path(Some(OsString::from(host_path)))
        .with_sandbox_view(SandboxView::same_path(
            mounted.iter().map(|dir| dir.to_path_buf()).collect(),
            Vec::new(),
        ))
        .with_sandbox_path(Some(OsString::from(sandbox_path)))
}

#[test]
fn pi_preflight_names_pi_and_its_installer_when_pi_is_missing() {
    // An empty, mounted directory as the whole world: a `pi` of the machine
    // (Debian has a package of that name) must not decide this test.
    let empty = tempfile::tempdir().unwrap();
    let dir = empty.path().display().to_string();
    let ctx = sandboxed(&[empty.path()], &dir, &dir);
    let diagnostics = PiAdapter::new().preflight(&ctx);
    let missing = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_001")
        .expect("AGENT_001");
    assert_eq!(missing.severity, Severity::Blocking);
    assert!(missing.why.starts_with("pi is not in "), "{}", missing.why);
    assert!(missing.why.contains(INSTALL_COMMAND), "{}", missing.why);
    assert_eq!(
        missing.fix,
        Some(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
    );
    assert_eq!(missing.docs.as_deref(), Some(DOCS_URL));
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "AGENT_006"),
        "no second finding about Node for a pi that is not there"
    );
}

#[test]
fn pi_preflight_is_quiet_when_pi_and_node_are_in_the_sandbox() {
    let bin = tempfile::tempdir().unwrap();
    write_executable(&bin.path().join("pi"), PI_SCRIPT);
    write_executable(&bin.path().join("node"), "#!/bin/sh\nexit 0\n");
    let dir = bin.path().display().to_string();
    let ctx = sandboxed(&[bin.path()], &dir, &dir);
    assert_eq!(PiAdapter::new().preflight(&ctx), Vec::new());
}

#[test]
fn pi_preflight_reports_node_missing_in_the_sandbox() {
    let bin = tempfile::tempdir().unwrap();
    write_executable(&bin.path().join("pi"), PI_SCRIPT);
    let dir = bin.path().display().to_string();

    // No Node anywhere: the download page.
    let ctx = sandboxed(&[bin.path()], &dir, &dir);
    let finding = PiAdapter::new()
        .preflight(&ctx)
        .into_iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_006")
        .expect("AGENT_006");
    assert_eq!(finding.severity, Severity::Blocking);
    assert!(finding.why.contains("asks for node"), "{}", finding.why);
    assert!(
        finding.why.contains(&dir),
        "names the sandbox PATH: {}",
        finding.why
    );
    assert_eq!(
        finding.fix,
        Some(FixAction::OpenUrl(NODE_DOWNLOAD_URL.to_owned()))
    );

    // Node on the host under a directory the sandbox does not mount: copy it.
    let host_only = tempfile::tempdir().unwrap();
    write_executable(&host_only.path().join("node"), "#!/bin/sh\nexit 0\n");
    let ctx = sandboxed(
        &[bin.path()],
        &dir,
        &format!("{dir}:{}", host_only.path().display()),
    );
    let finding = PiAdapter::new()
        .preflight(&ctx)
        .into_iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_006")
        .expect("AGENT_006");
    let Some(FixAction::CopyCommand(command)) = &finding.fix else {
        panic!("an unmounted node is copied: {:?}", finding.fix);
    };
    assert!(command.starts_with("sudo install -m 0755 "), "{command}");
    assert!(command.ends_with("/usr/local/bin/node"), "{command}");

    // Node mounted, but not on the sandbox path: change the path.
    let mounted = tempfile::tempdir().unwrap();
    write_executable(&mounted.path().join("node"), "#!/bin/sh\nexit 0\n");
    let other = mounted.path().display().to_string();
    let ctx = sandboxed(
        &[bin.path(), mounted.path()],
        &dir,
        &format!("{dir}:{other}"),
    );
    let finding = PiAdapter::new()
        .preflight(&ctx)
        .into_iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_006")
        .expect("AGENT_006");
    assert_eq!(
        finding.fix,
        Some(FixAction::ChangeSetting {
            key: "sandbox.env.PATH".to_owned(),
            value: format!("{other}:{dir}"),
        })
    );
}

/// The Node check is about Node: a command that is no Node script, and a
/// lookup the host cannot answer, give nothing.
#[test]
fn pi_preflight_says_nothing_about_node_for_other_commands() {
    let bin = tempfile::tempdir().unwrap();
    write_executable(&bin.path().join("pi"), "#!/bin/sh\nexec true\n");
    let dir = bin.path().display().to_string();
    assert_eq!(
        PiAdapter::new().preflight(&sandboxed(&[bin.path()], &dir, &dir)),
        Vec::new()
    );
    // No mounts known: the preflight cannot decide and stays silent
    // (`backlog/CONVENTIONS.md` 4.13).
    write_executable(&bin.path().join("pi"), PI_SCRIPT);
    let unknown =
        context(Some("http://192.168.1.50:11434")).with_host_path(Some(OsString::from(&dir)));
    assert_eq!(PiAdapter::new().preflight(&unknown), Vec::new());
}

// --- Integration: a real sandbox with a fake pi ----------------------------

/// What pi 1.3.0 does at start, as far as Humanitl is concerned: read its
/// agent directory, resolve `apiKey` through `/bin/sh -c` when it starts with
/// `!`, write into the agent directory, fail on the read-only settings.
const FAKE_PI: &str = r#"
A="$PI_CODING_AGENT_DIR"
echo "offline=$PI_OFFLINE skip=$PI_SKIP_VERSION_CHECK telemetry=$PI_TELEMETRY"
command=$(sed -n 's/^ *"apiKey": "!\(.*\)",*$/\1/p' "$A/models.json")
key=$(/bin/sh -c "$command")
if [ "$key" = "sk-pi-test-0123456789abcdef" ]; then echo key=arrived; else echo "key=missing [$command]"; fi
grep -q '"baseUrl": "http://192.168.1.50:11434/v1"' "$A/models.json" && echo base=ok
grep -q '"defaultModel": "qwen3"' "$A/settings.json" && echo settings=ok
grep -q 'Blocked by Humanitl.' "$A/AGENTS.md" && echo briefing=ok
if echo '{}' > "$A/settings.json" 2>/dev/null; then echo settings_rc=0; else echo settings_rc=1; fi
mkdir -p "$A/sessions" && echo '{}' > "$A/auth.json" && echo agent_dir=writable
if env | grep -q sk-pi-test; then echo env_key=1; else echo env_key=0; fi
ls -A /work
"#;

#[test]
fn a_fake_pi_finds_its_configuration_and_key_in_a_real_sandbox() {
    let Some((fx, backend)) =
        sandbox_or_skip("a_fake_pi_finds_its_configuration_and_key_in_a_real_sandbox")
    else {
        return;
    };
    std::fs::write(fx.work.join("canary.txt"), b"canary\n").unwrap();

    let adapter = PiAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434"))
        .with_llm_key(AgentKey::Key(ApiKey::new(KEY).unwrap()));
    let mut session = fx.context(&["sh", "-c", FAKE_PI]);
    session.files = adapter.files(&ctx).expect("the adapter renders its files");
    session.session_env.extend(adapter.env(&ctx));

    let plan = backend
        .plan(&fx.profile, &session)
        .expect("the plan builds");
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
        "offline=1 skip=1 telemetry=0",
        "key=arrived",
        "base=ok",
        "settings=ok",
        "briefing=ok",
        "settings_rc=1",
        "agent_dir=writable",
        "env_key=0",
        "canary.txt",
    ] {
        assert!(stdout.contains(line), "{line} is missing in:\n{stdout}");
    }
    assert!(
        !stdout.contains("models.json") && !stdout.contains("AGENTS.md"),
        "nothing of the adapter lands in the project directory: {stdout}"
    );
    assert_eq!(checks.len(), 3, "{checks:?}");
    for check in &checks {
        assert!(
            check.passed,
            "guarantee {:?} failed with pi's files: {check:?}",
            check.check
        );
    }
}

/// Skips without a usable `bwrap` or shim, and refuses to skip under CI;
/// the same pattern as in `opencode_adapter.rs`.
fn sandbox_or_skip(test: &str) -> Option<(Fixture, humanitl_sandbox::BwrapBackend)> {
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
                .with("XDG_RUNTIME_DIR", root.join("run").to_string_lossy()),
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
        std::fs::write(&ca, b"# ca\n").map_err(|err| why("no placeholder CA", &err))?;
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
            agent_program: None,
        }
    }

    fn backend(&self) -> Result<humanitl_sandbox::BwrapBackend, String> {
        humanitl_sandbox::BwrapBackend::detect(self.paths.clone())
            .map(|backend| backend.with_stdio(humanitl_sandbox::StdioMode::Capture))
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

/// A pi on the host where the sandbox does not look is reinstalled, not
/// copied: the entry script needs its package files beside it, and a copy in
/// `/usr/local/bin` would end in `ERR_MODULE_NOT_FOUND` (HUM-274).
#[test]
fn pi_outside_the_mounts_is_reinstalled_not_copied() {
    let mounted = tempfile::tempdir().unwrap();
    let host_only = tempfile::tempdir().unwrap();
    write_executable(&host_only.path().join("pi"), PI_SCRIPT);
    let inside = mounted.path().display().to_string();
    let ctx = sandboxed(
        &[mounted.path()],
        &inside,
        &format!("{inside}:{}", host_only.path().display()),
    );
    let diagnostics = PiAdapter::new().preflight(&ctx);
    let finding = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_004")
        .unwrap_or_else(|| panic!("AGENT_004 in {diagnostics:?}"));
    assert_eq!(
        finding.fix,
        Some(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
    );
    assert_eq!(finding.docs.as_deref(), Some(DOCS_URL));
    assert!(finding.why.contains("is a script"), "{}", finding.why);
    assert!(
        !finding.why.contains("native binary"),
        "no advice that leads nowhere for pi: {}",
        finding.why
    );
}

/// An absolute `#!` line names the one node the kernel starts. A node copied
/// elsewhere does not change it, so the fix is reinstalling pi (HUM-274).
#[test]
fn pi_with_an_absolute_node_outside_the_mounts_is_reinstalled() {
    let bin = tempfile::tempdir().unwrap();
    let node_dir = tempfile::tempdir().unwrap();
    let node = node_dir.path().join("node");
    write_executable(&node, "#!/bin/sh\nexit 0\n");
    write_executable(
        &bin.path().join("pi"),
        &format!("#!{}\nimport \"./cli-runtime.js\";\n", node.display()),
    );
    let dir = bin.path().display().to_string();
    let ctx = sandboxed(&[bin.path()], &dir, &dir);
    let finding = PiAdapter::new()
        .preflight(&ctx)
        .into_iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_006")
        .expect("AGENT_006");
    assert_eq!(
        finding.fix,
        Some(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
    );
    assert!(
        finding.why.contains(&node.display().to_string()),
        "{}",
        finding.why
    );
}

/// Another command under the pi adapter keeps the generic findings: pi's
/// installer is no fix for a program that is not pi (HUM-274).
#[test]
fn another_command_under_pi_does_not_get_pis_installer() {
    let mounted = tempfile::tempdir().unwrap();
    let host_only = tempfile::tempdir().unwrap();
    write_executable(&host_only.path().join("mytool"), PI_SCRIPT);
    let inside = mounted.path().display().to_string();
    let ctx = sandboxed(
        &[mounted.path()],
        &inside,
        &format!("{inside}:{}", host_only.path().display()),
    )
    .with_command_override(Some(vec![OsString::from("mytool")]));
    let diagnostics = PiAdapter::new().preflight(&ctx);
    let finding = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_004")
        .unwrap_or_else(|| panic!("AGENT_004 in {diagnostics:?}"));
    assert_ne!(
        finding.fix,
        Some(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
    );
    assert!(!finding.why.contains(INSTALL_COMMAND), "{}", finding.why);

    // The same for AGENT_006 with an absolute node outside the mounts.
    let bin = tempfile::tempdir().unwrap();
    let node_dir = tempfile::tempdir().unwrap();
    let node = node_dir.path().join("node");
    write_executable(&node, "#!/bin/sh\nexit 0\n");
    write_executable(
        &bin.path().join("mytool"),
        &format!("#!{}\n", node.display()),
    );
    let dir = bin.path().display().to_string();
    let ctx = sandboxed(&[bin.path()], &dir, &dir)
        .with_command_override(Some(vec![OsString::from("mytool")]));
    let finding = PiAdapter::new()
        .preflight(&ctx)
        .into_iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_006")
        .expect("AGENT_006");
    assert_eq!(
        finding.fix,
        Some(FixAction::OpenUrl(NODE_DOWNLOAD_URL.to_owned()))
    );
    assert!(!finding.why.contains(INSTALL_COMMAND), "{}", finding.why);
}
