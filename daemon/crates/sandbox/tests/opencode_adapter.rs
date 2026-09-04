//! Der OpenCode-Adapter (HUM-037).
//!
//! Geprüft wird, was der Adapter dem Launcher übergibt: Kommando, Umgebung,
//! Dateien, Vorprüfung und die Passthrough-Regel. Alles davon ist eine reine
//! Funktion über den [`AgentContext`]; keine Sandbox startet dafür.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::LlmConfig;
use humanitl_core::rule::{Action, HostPattern, PathPattern};
use humanitl_core::{HostName, Method, Scheme, SessionId, Severity};
use humanitl_rules::{RequestKey, RuleSet, Verdict};
use humanitl_sandbox::agent::opencode::{
    CONFIG_DST, MANAGED_CONFIG_DST, MODELS_DST, PLACEHOLDER_MODEL, home_config_dst, home_keep_dst,
};
use humanitl_sandbox::agent::opencode_models::PROVIDER_ID;
use humanitl_sandbox::{
    AdapterRegistry, AgentAdapter, AgentContext, OpenCodeAdapter, SandboxBackend, SandboxProfile,
    files_inside_work,
};
use serde_json::Value;

/// Ein Kontext mit einem Endpunkt und ohne Modelle.
fn context(endpoint: Option<&str>) -> AgentContext {
    let llm = LlmConfig {
        endpoint: endpoint.map(|text| url::Url::parse(text).unwrap()),
        ..LlmConfig::default()
    };
    AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm)
}

/// Die Umgebung als Map, damit ein Test nach dem Schlüssel fragen kann.
fn env_map(adapter: OpenCodeAdapter, ctx: &AgentContext) -> BTreeMap<String, String> {
    adapter.env(ctx).into_iter().collect()
}

/// Die Datei zu einem Ziel.
fn file_at(adapter: OpenCodeAdapter, ctx: &AgentContext, dst: &str) -> Vec<u8> {
    adapter
        .files(ctx)
        .unwrap()
        .into_iter()
        .find(|file| file.dst == Path::new(dst))
        .unwrap_or_else(|| panic!("no file for {dst}"))
        .content
}

#[test]
fn opencode_env_contains_required_keys() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434"));
    let env = env_map(adapter, &ctx);

    for (key, value) in [
        ("OPENCODE_DISABLE_AUTOUPDATE", "true"),
        // Abweichung von der Spezifikation, begründet im Modul-Kommentar von
        // `agent/opencode.rs`: `OPENCODE_MODELS_URL` ist eine Basis-Adresse für
        // einen HTTP-Abruf und nimmt kein `file://`.
        ("OPENCODE_MODELS_PATH", "/etc/humanitl/opencode/models.json"),
        ("OPENCODE_DISABLE_MODELS_FETCH", "true"),
        ("OPENCODE_CONFIG", "/etc/humanitl/opencode/opencode.json"),
        ("OPENCODE_AUTO_SHARE", "false"),
        ("OPENCODE_DISABLE_SHARE", "true"),
        ("OPENCODE_ENABLE_EXA", "false"),
        ("OPENCODE_ENABLE_PARALLEL", "false"),
        ("OPENCODE_DISABLE_LSP_DOWNLOAD", "true"),
        ("HOME", "/home/agent"),
        ("XDG_CONFIG_HOME", "/home/agent/.config"),
        ("XDG_DATA_HOME", "/home/agent/.local/share"),
        ("XDG_CACHE_HOME", "/home/agent/.cache"),
        ("NODE_EXTRA_CA_CERTS", "/etc/humanitl/ca.crt"),
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
        ("LANG", "C.UTF-8"),
    ] {
        assert_eq!(env.get(key).map(String::as_str), Some(value), "{key}");
    }
}

#[test]
fn opencode_env_never_points_the_catalog_at_the_network() {
    let adapter = OpenCodeAdapter::new();
    let env = env_map(adapter, &context(Some("http://192.168.1.50:11434")));
    assert!(
        !env.contains_key("OPENCODE_MODELS_URL"),
        "a base URL for the catalog would be fetched over the network"
    );
    for (key, value) in &env {
        for host in ["models.dev", "models.opencode.ai", "https://", "http://"] {
            assert!(
                !value.contains(host),
                "{key} carries an address ({value}); the adapter sets paths, not endpoints"
            );
        }
    }
}

#[test]
fn opencode_config_is_valid_json_and_points_to_llm() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434")).with_models(vec!["qwen3".to_owned()]);
    let text = file_at(adapter, &ctx, CONFIG_DST);
    let doc: Value = serde_json::from_slice(&text).unwrap();

    assert_eq!(
        doc["provider"][PROVIDER_ID]["options"]["baseURL"],
        Value::String("http://192.168.1.50:11434/v1".to_owned())
    );
    assert_eq!(
        doc["provider"][PROVIDER_ID]["npm"],
        Value::String("@ai-sdk/openai-compatible".to_owned())
    );
    assert_eq!(
        doc["model"],
        Value::String("humanitl-local/qwen3".to_owned())
    );
    assert_eq!(doc["autoupdate"], Value::Bool(false));
    assert_eq!(doc["share"], Value::String("disabled".to_owned()));
    assert_eq!(
        doc["permission"]["websearch"],
        Value::String("deny".to_owned())
    );
    assert_eq!(
        doc["permission"]["webfetch"],
        Value::String("ask".to_owned())
    );
    assert_eq!(
        doc["permission"]["external_directory"],
        Value::String("deny".to_owned())
    );
}

#[test]
fn opencode_config_appends_v1_only_once() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://x:1/v1")).with_models(vec!["m".to_owned()]);
    let doc: Value = serde_json::from_slice(&file_at(adapter, &ctx, CONFIG_DST)).unwrap();
    assert_eq!(
        doc["provider"][PROVIDER_ID]["options"]["baseURL"],
        Value::String("http://x:1/v1".to_owned())
    );
}

#[test]
fn opencode_models_placeholder_when_empty() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434"));

    let config: Value = serde_json::from_slice(&file_at(adapter, &ctx, CONFIG_DST)).unwrap();
    assert_eq!(
        config["model"],
        Value::String(format!("{PROVIDER_ID}/{PLACEHOLDER_MODEL}"))
    );
    assert!(
        config["provider"][PROVIDER_ID]["models"][PLACEHOLDER_MODEL].is_object(),
        "the placeholder model has to exist in the provider"
    );

    let catalog: Value = serde_json::from_slice(&file_at(adapter, &ctx, MODELS_DST)).unwrap();
    assert_eq!(
        catalog[PROVIDER_ID]["models"][PLACEHOLDER_MODEL]["id"],
        Value::String(PLACEHOLDER_MODEL.to_owned())
    );

    let diagnostics = adapter.preflight(&ctx);
    let llm_004 = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "LLM_004")
        .expect("LLM_004 is missing");
    assert_eq!(llm_004.severity, Severity::Warning);
    assert!(llm_004.fix.is_some(), "LLM_004 offers a setting to change");
}

#[test]
fn opencode_preflight_missing_binary() {
    let adapter = OpenCodeAdapter::new();
    let empty = tempfile::tempdir().unwrap();
    let ctx = context(Some("http://192.168.1.50:11434"))
        .with_models(vec!["qwen3".to_owned()])
        .with_host_path(Some(OsString::from(empty.path())));

    let diagnostics = adapter.preflight(&ctx);
    let agent_001 = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_001")
        .expect("AGENT_001 is missing");
    assert_eq!(agent_001.severity, Severity::Blocking);
    assert!(
        agent_001.docs.is_some() && agent_001.fix.is_some(),
        "a blocking finding names a way out"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "LLM_004"),
        "with a model configured there is no LLM_004"
    );
}

#[test]
fn opencode_preflight_is_quiet_when_everything_is_there() {
    let adapter = OpenCodeAdapter::new();
    let (dir, keep) = bin_dir_with_opencode();
    let ctx = context(Some("http://192.168.1.50:11434"))
        .with_models(vec!["qwen3".to_owned()])
        .with_host_path(Some(dir));
    assert_eq!(adapter.preflight(&ctx), Vec::new());
    drop(keep);
}

#[test]
fn opencode_preflight_warns_about_an_unusable_override() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434"))
        .with_models(vec!["qwen3".to_owned()])
        .with_command_override(Some(vec![OsString::from("/nonexistent/opencode")]));

    let diagnostics = adapter.preflight(&ctx);
    let agent_002 = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "AGENT_002")
        .expect("AGENT_002 is missing");
    assert_eq!(agent_002.severity, Severity::Warning);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "AGENT_001"),
        "with an override the missing default command is not the problem"
    );
}

#[test]
fn opencode_command_is_the_override_when_there_is_one() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(None);
    assert_eq!(adapter.command(&ctx), vec![OsString::from("opencode")]);

    let ctx = ctx.with_command_override(Some(vec![
        OsString::from("/opt/opencode"),
        OsString::from("--flag"),
    ]));
    assert_eq!(
        adapter.command(&ctx),
        vec![OsString::from("/opt/opencode"), OsString::from("--flag")]
    );
}

#[test]
fn opencode_writes_nothing_into_work() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434"));
    let files = adapter.files(&ctx).unwrap();

    assert!(
        files_inside_work(&files, Path::new("/work")).is_empty(),
        "an adapter never writes into the project directory"
    );
    let targets: Vec<&Path> = files.iter().map(|file| file.dst.as_path()).collect();
    assert_eq!(
        targets,
        vec![
            Path::new(CONFIG_DST),
            Path::new(MODELS_DST),
            // Fallstrick 4: dieselbe Konfiguration auch dort, wo OpenCode sie
            // ohne `OPENCODE_CONFIG` sucht.
            home_config_dst(&ctx.home).as_path(),
            home_keep_dst(&ctx.home).as_path(),
            // Das verwaltete Verzeichnis: die einzige Quelle, die nach der
            // Konfiguration eines geklonten Projekts gemergt wird.
            Path::new(MANAGED_CONFIG_DST),
        ]
    );
    for file in &files {
        assert_eq!(file.mode, 0o444, "{:?} is writable", file.dst);
    }
}

#[test]
fn opencode_outranks_a_config_from_the_cloned_project() {
    // `OpenCode` 1.18.25 mergt die Konfiguration des Projekts NACH der Datei
    // aus `OPENCODE_CONFIG`. Nur zwei Quellen stehen danach: das verwaltete
    // Verzeichnis und `OPENCODE_PERMISSION`. Gemessen mit
    // `opencode debug config`; ohne beides gewinnt das geklonte Repository.
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434")).with_models(vec!["qwen3".to_owned()]);

    let managed: Value = serde_json::from_slice(&file_at(adapter, &ctx, MANAGED_CONFIG_DST))
        .expect("the managed config is JSON");
    let explicit: Value =
        serde_json::from_slice(&file_at(adapter, &ctx, CONFIG_DST)).expect("the config is JSON");
    assert_eq!(managed, explicit, "both places carry the same document");

    let env = env_map(adapter, &ctx);
    let permission: Value = serde_json::from_str(
        env.get("OPENCODE_PERMISSION")
            .expect("OPENCODE_PERMISSION is set"),
    )
    .expect("OPENCODE_PERMISSION is JSON");
    assert_eq!(
        permission, explicit["permission"],
        "the environment carries the same permissions as the file"
    );
    assert_eq!(
        permission["websearch"],
        Value::String("deny".to_owned()),
        "the hosted web search stays off, whatever a project config says"
    );
}

#[test]
fn opencode_default_rules_are_bundled() {
    let adapter = OpenCodeAdapter::new();
    let (rules, warnings) =
        humanitl_rules::parse_rules(adapter.default_rules()).expect("the bundled rules parse");
    assert!(
        warnings.is_empty(),
        "the bundled rules are free of warnings: {warnings:?}"
    );
    assert!(!rules.is_empty());
    for rule in rules.iter() {
        assert!(rule.bundled, "rule {} is not marked bundled", rule.id);
    }
}

#[test]
fn opencode_passthrough_rule_matches_only_the_endpoint() {
    let adapter = OpenCodeAdapter::new();
    let llm = LlmConfig {
        endpoint: Some(url::Url::parse("http://192.168.1.50:11434").unwrap()),
        ..LlmConfig::default()
    };
    let rule = adapter
        .llm_passthrough(&llm)
        .expect("a rule for an endpoint");

    assert_eq!(rule.action, Action::Allow);
    assert!(rule.bundled);
    assert!(
        rule.allow_private,
        "the model usually lives on a private address; without this the proxy refuses it"
    );
    assert!(!rule.stream, "the request body is buffered, never streamed");
    assert_eq!(
        rule.id.to_string(),
        "01920000-0000-7000-8000-0000000000ff",
        "the id is fixed so that every part of the system means the same rule"
    );
    assert_eq!(
        rule.matcher.host,
        HostPattern::Exact(HostName::parse("192.168.1.50").unwrap())
    );
    assert_eq!(rule.matcher.port, Some(11434));
    assert_eq!(rule.matcher.scheme, Some(Scheme::Http));
    assert_eq!(
        rule.matcher.path,
        Some(PathPattern::Regex("^(?:/v1/|/api/)".to_owned()))
    );

    let set = RuleSet::from_rules([rule]);
    let now = chrono::Utc::now();
    let session = SessionId::nil();
    let endpoint = HostName::parse("192.168.1.50").unwrap();
    let other_host = HostName::parse("192.168.1.51").unwrap();

    for (host, method, path, port, expected_allow) in [
        (&endpoint, Method::POST, "/v1/chat/completions", 11434, true),
        (&endpoint, Method::GET, "/v1/models", 11434, true),
        (&endpoint, Method::POST, "/api/chat", 11434, true),
        (&endpoint, Method::POST, "/admin", 11434, false),
        (&endpoint, Method::POST, "/v1/x", 8080, false),
        (&other_host, Method::POST, "/v1/x", 11434, false),
        (&endpoint, Method::DELETE, "/v1/x", 11434, false),
    ] {
        let key = RequestKey::new(host, &method, path, Scheme::Http, port);
        let verdict = set.evaluate(&key, now, session);
        let allowed = matches!(
            verdict,
            Verdict::Matched {
                action: Action::Allow,
                ..
            }
        );
        assert_eq!(
            allowed, expected_allow,
            "{method} http://{host}:{port}{path} came out as {verdict:?}"
        );
    }
}

#[test]
fn opencode_passthrough_needs_an_endpoint() {
    let adapter = OpenCodeAdapter::new();
    assert!(
        adapter.llm_passthrough(&LlmConfig::default()).is_none(),
        "without llm.endpoint there is nothing to let through"
    );
}

#[test]
fn the_registry_knows_opencode() {
    let registry = AdapterRegistry::builtin();
    assert_eq!(registry.ids(), vec!["opencode"]);
    let adapter = registry.get("opencode").expect("the builtin adapter");
    assert_eq!(adapter.id(), "opencode");
    assert!(
        adapter.is_fullscreen_tui(),
        "OpenCode is a full-screen TUI; `--ask terminal` refuses it (CLI_002)"
    );
    assert!(registry.get("aider").is_none());
}

/// Ein Verzeichnis mit einer ausführbaren Datei `opencode` darin.
///
/// Gibt den Pfad und das `TempDir` zurück; das Verzeichnis verschwindet, sobald
/// der zweite Wert fällt.
fn bin_dir_with_opencode() -> (OsString, tempfile::TempDir) {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("opencode");
    std::fs::write(&binary, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    (OsString::from(dir.path()), dir)
}

// --- Integration: eine echte Sandbox mit den Dateien des Adapters ------------

/// Startet die Sandbox mit dem Beitrag des Adapters und sieht nach, was drin
/// ankommt (HUM-037, Abschnitt Tests).
///
/// Kein `#[ignore]`: `bwrap` ist auf dem Entwicklerrechner und im
/// Escape-Job da, und ohne diesen Test prüft nichts im Repository, dass eine
/// Sandbox **mit** Adapter-Dateien die drei Garantien hält. Fehlt `bwrap`,
/// sagt der Test das auf `stderr` und endet grün — genauso wie die übrigen
/// Launcher-Tests.
#[test]
fn the_adapter_files_arrive_in_a_real_sandbox_and_the_guarantees_hold() {
    let Some(fx) = Fixture::new() else { return };
    let Some(backend) = fx.backend() else { return };

    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434")).with_models(vec!["qwen3".to_owned()]);
    let mut session = fx.context(&[
        "sh",
        "-c",
        "cat /etc/humanitl/opencode/opencode.json; echo ---MARK---; env | grep OPENCODE_ | sort",
    ]);
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
        "{status}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let (config, env) = stdout.split_once("---MARK---\n").expect("the marker");
    let parsed: Value = serde_json::from_str(config.trim()).expect("the config arrived as JSON");
    assert_eq!(
        parsed["provider"][PROVIDER_ID]["options"]["baseURL"],
        Value::String("http://192.168.1.50:11434/v1".to_owned()),
        "the endpoint of the user is in the file the agent reads"
    );
    assert!(
        env.contains("OPENCODE_DISABLE_AUTOUPDATE=true"),
        "the environment of the adapter arrived: {env}"
    );
    assert!(
        env.contains("OPENCODE_MODELS_PATH=/etc/humanitl/opencode/models.json"),
        "the catalog points at the bundled file: {env}"
    );
    assert!(
        !env.contains("OPENCODE_MODELS_URL"),
        "nothing points the catalog at the network: {env}"
    );

    // Der eigentliche Punkt: die drei Garantien halten auch mit den fünf
    // zusätzlichen Bindungen des Adapters.
    assert_eq!(checks.len(), 3, "{checks:?}");
    for check in &checks {
        assert!(
            check.passed,
            "guarantee {:?} failed with adapter files: {check:?}",
            check.check
        );
    }
}

/// Die Adapter-Dateien sind in der Sandbox nicht beschreibbar, und `/work`
/// bleibt frei von ihnen.
#[test]
fn the_adapter_files_are_read_only_and_work_stays_clean() {
    let Some(fx) = Fixture::new() else { return };
    let Some(backend) = fx.backend() else { return };

    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434")).with_models(vec!["qwen3".to_owned()]);
    let mut session = fx.context(&[
        "sh",
        "-c",
        "echo x > /etc/humanitl/opencode/opencode.json 2>&1; echo \"rc=$?\"; ls -A /work",
    ]);
    session.files = adapter.files(&ctx).expect("the adapter renders its files");

    let plan = backend
        .plan(&fx.profile, &session)
        .expect("the plan builds");
    let handle = backend.launch(&plan).expect("bwrap starts");
    let _ = handle
        .wait_timeout(std::time::Duration::from_secs(30))
        .expect("the sandbox ends in time");
    let output = handle.output().expect("captured");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    assert!(
        !stdout.contains("rc=0"),
        "the agent must not be able to rewrite its own configuration: {stdout}"
    );
    assert!(
        !stdout.contains("opencode.json"),
        "nothing of the adapter lands in the project directory: {stdout}"
    );
}

/// Die Umgebung für einen echten Lauf: Platzhalter für Socket, CA und Shim.
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
    /// `None`, wenn das Profil oder der Shim fehlen; der Grund steht auf `stderr`.
    fn new() -> Option<Self> {
        let dir = tempfile::tempdir().ok()?;
        let root = dir.path().to_path_buf();
        let work = root.join("work");
        std::fs::create_dir_all(&work).ok()?;
        // Die eine Tür liegt im Proxy-Verzeichnis der Laufzeit und trägt
        // 0600; alles andere lehnt der Planer mit `SANDBOX_006` ab.
        let paths = humanitl_config::Paths::new(
            humanitl_config::Env::from_process()
                .with("XDG_RUNTIME_DIR", root.join("run").to_string_lossy()),
        );
        let socket = paths.proxy_socket();
        std::fs::create_dir_all(socket.parent()?).ok()?;
        drop(std::os::unix::net::UnixListener::bind(&socket).ok()?);
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).ok()?;
        let ca = root.join("ca.crt");
        std::fs::write(&ca, b"# ca\n").ok()?;
        let ca_bundle = root.join("ca-bundle.crt");
        std::fs::write(&ca_bundle, b"# bundle\n").ok()?;

        let shim = shim_binary()?;
        let profile_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
        let profile = match SandboxProfile::load(&profile_path) {
            Ok(profile) => profile,
            Err(err) => {
                eprintln!("skipping: the default profile does not load: {err}");
                return None;
            }
        };
        Some(Self {
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
        }
    }

    /// Das echte `bwrap`, oder `None` mit Begründung auf `stderr`.
    fn backend(&self) -> Option<humanitl_sandbox::BwrapBackend> {
        match humanitl_sandbox::BwrapBackend::detect(self.paths.clone()) {
            Ok(backend) => Some(backend.with_stdio(humanitl_sandbox::StdioMode::Capture)),
            Err(err) => {
                eprintln!("skipping: no usable bwrap: {err}");
                None
            }
        }
    }
}

/// Der gebaute Shim neben dem Testbinary.
fn shim_binary() -> Option<PathBuf> {
    let mut dir = std::env::current_exe().ok()?;
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let shim = dir.join("humanitl-shim");
    if shim.is_file() {
        Some(shim)
    } else {
        eprintln!("skipping: {} is not built", shim.display());
        None
    }
}
