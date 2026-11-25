//! Wohin `OpenCode` und pi ihre Inferenz schicken (GitHub #112).
//!
//! Die allgemeinen Wege (`inference::finding`, `session_passthrough`,
//! `session_endpoint`) prüft der Modultest in `src/agent/inference.rs` mit
//! einem Testadapter; hier steht, was daraus für die beiden Adapter folgt,
//! die über `llm.endpoint` sprechen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use humanitl_config::{AgentInference, Config, LlmConfig};
use humanitl_core::rule::Action;
use humanitl_core::{FixAction, HostName, Method, Scheme, SessionId, Severity};
use humanitl_rules::{RequestKey, RuleSet, Verdict};
use humanitl_sandbox::agent::opencode::{
    CONFIG_DST, MANAGED_CONFIG_DST, MODELS_DST, ZEN_HOST, config_dst,
};
use humanitl_sandbox::agent::{AgentKey, inference, pi};
use humanitl_sandbox::{AgentAdapter, AgentContext, OpenCodeAdapter, PiAdapter};
use serde_json::Value;

/// Ein Schlüssel, lang genug für `ApiKey::new`.
const KEY: &str = "sk-test-0123456789abcdef";

/// Ein Kontext mit diesem Endpunkt und dieser Einstellung, mit Schlüssel.
fn context(endpoint: Option<&str>, mode: AgentInference) -> AgentContext {
    let llm = LlmConfig {
        endpoint: endpoint.map(|text| url::Url::parse(text).unwrap()),
        models: vec!["qwen3".to_owned()],
        ..LlmConfig::default()
    };
    AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm)
        .with_inference(mode)
        .with_llm_key(AgentKey::Key(
            humanitl_config::secret::ApiKey::new(KEY).unwrap(),
        ))
}

/// Eine Konfiguration mit diesem Adapter, Endpunkt und dieser Einstellung.
fn config(adapter: &str, endpoint: Option<&str>, mode: AgentInference) -> Config {
    let mut config = Config::default();
    adapter.clone_into(&mut config.agent.adapter);
    config.llm.endpoint = endpoint.map(|text| url::Url::parse(text).unwrap());
    match adapter {
        "opencode" => config.agents.opencode.inference = mode,
        "pi" => config.agents.pi.inference = mode,
        other => panic!("no test for {other}"),
    }
    config
}

fn files(adapter: &dyn AgentAdapter, ctx: &AgentContext) -> BTreeMap<PathBuf, Vec<u8>> {
    adapter
        .files(ctx)
        .unwrap()
        .into_iter()
        .map(|file| (file.dst.clone(), file.content.clone()))
        .collect()
}

fn opencode_configs(ctx: &AgentContext) -> Vec<Value> {
    let all = files(&OpenCodeAdapter::new(), ctx);
    let home = ctx.config_home();
    [
        PathBuf::from(CONFIG_DST),
        config_dst(&home),
        PathBuf::from(MANAGED_CONFIG_DST),
    ]
    .iter()
    .map(|dst| {
        serde_json::from_slice(
            all.get(dst)
                .unwrap_or_else(|| panic!("no {}", dst.display())),
        )
        .unwrap()
    })
    .collect()
}

/// Kein Pfad und keine Datei nennt den Port des Proxys als Ziel des Modells.
fn assert_no_proxy_port(adapter: &dyn AgentAdapter, ctx: &AgentContext) {
    let needle = format!("127.0.0.1:{}", ctx.proxy_port);
    for (dst, content) in files(adapter, ctx) {
        let text = String::from_utf8_lossy(&content);
        assert!(
            !text.contains(&needle),
            "{} names the proxy port as a model: {text}",
            dst.display()
        );
    }
}

#[test]
fn opencode_with_own_providers_writes_no_provider_no_model_and_no_catalog_path() {
    // Mit einem Endpunkt: `own` gewinnt, Humanitl schreibt auch dann nichts.
    for endpoint in [None, Some("http://192.168.1.50:11434")] {
        let ctx = context(endpoint, AgentInference::Own);
        for doc in opencode_configs(&ctx) {
            assert!(doc.get("provider").is_none(), "{doc}");
            assert!(doc.get("model").is_none(), "{doc}");
            // Was bleibt: die Schalter, die keinen Provider betreffen.
            assert_eq!(doc["share"], "disabled");
            assert_eq!(doc["autoupdate"], false);
            assert_eq!(doc["permission"]["external_directory"], "deny");
        }
        let all = files(&OpenCodeAdapter::new(), &ctx);
        assert!(!all.contains_key(Path::new(MODELS_DST)), "no catalog file");
        assert!(
            !all.contains_key(Path::new(humanitl_sandbox::agent::opencode::API_KEY_DST)),
            "no key of Humanitl for the agent's own providers"
        );
        let env: BTreeMap<String, String> = OpenCodeAdapter::new().env(&ctx).into_iter().collect();
        assert!(!env.contains_key("OPENCODE_MODELS_PATH"), "{env:?}");
        for (key, value) in [
            ("OPENCODE_DISABLE_MODELS_FETCH", "true"),
            ("OPENCODE_DISABLE_AUTOUPDATE", "true"),
            ("OPENCODE_DISABLE_SHARE", "true"),
            ("OPENCODE_AUTO_SHARE", "false"),
            ("OPENCODE_ENABLE_EXA", "false"),
            ("OPENCODE_ENABLE_PARALLEL", "false"),
            ("OPENCODE_DISABLE_LSP_DOWNLOAD", "true"),
            ("OPENCODE_CONFIG", CONFIG_DST),
        ] {
            assert_eq!(env.get(key).map(String::as_str), Some(value), "{key}");
        }
        assert!(env.contains_key("OPENCODE_PERMISSION"));
    }
}

#[test]
fn opencode_with_a_model_server_keeps_its_provider() {
    let ctx = context(
        Some("http://192.168.1.50:11434"),
        AgentInference::ModelServer,
    );
    for doc in opencode_configs(&ctx) {
        assert_eq!(
            doc["provider"]["humanitl-local"]["options"]["baseURL"],
            "http://192.168.1.50:11434/v1"
        );
        assert_eq!(doc["model"], "humanitl-local/qwen3");
    }
    let env: BTreeMap<String, String> = OpenCodeAdapter::new().env(&ctx).into_iter().collect();
    assert_eq!(
        env.get("OPENCODE_MODELS_PATH").map(String::as_str),
        Some(MODELS_DST)
    );
}

/// Ohne Modellserver schreibt kein Adapter den Port des Proxys als Ziel
/// des Modells, und die Vorprüfung blockt (Mutationsbeweis: die alte
/// Rückfallregel `http://127.0.0.1:<port>/v1` macht diesen Test rot).
#[test]
fn without_a_model_server_nothing_points_at_the_proxy_and_the_start_is_refused() {
    let ctx = context(None, AgentInference::ModelServer);
    for adapter in [
        &OpenCodeAdapter::new() as &dyn AgentAdapter,
        &PiAdapter::new() as &dyn AgentAdapter,
    ] {
        assert_no_proxy_port(adapter, &ctx);
        let found = inference::preflight(adapter, &ctx);
        let llm_012 = found
            .iter()
            .find(|diagnostic| diagnostic.code.as_str() == "LLM_012")
            .unwrap_or_else(|| panic!("{}: no LLM_012 in {found:?}", adapter.id()));
        assert_eq!(llm_012.severity, Severity::Blocking);
        assert!(
            matches!(
                &llm_012.fix,
                Some(FixAction::ChangeSetting { key, .. }) if key == "llm.endpoint"
            ),
            "{}: the fix sets a model server: {llm_012:?}",
            adapter.id()
        );
        assert!(
            !found
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == "LLM_004"),
            "{}: no second finding about the model of a server that is not there",
            adapter.id()
        );
    }
    // OpenCode kennt eigene Provider und nennt sie als zweiten Weg; pi nicht.
    let opencode = inference::finding(&OpenCodeAdapter::new(), &ctx).unwrap();
    assert!(
        opencode.why.contains("agents.opencode.inference") && opencode.why.contains(ZEN_HOST),
        "{}",
        opencode.why
    );
    let pi = inference::finding(&PiAdapter::new(), &ctx).unwrap();
    assert!(!pi.why.contains("agents.pi.inference"), "{}", pi.why);
}

/// Eine Shell des Aufrufers startet ohne Provider von Humanitl.
#[test]
fn a_shell_of_the_caller_starts_without_a_model_server() {
    let ctx =
        context(None, AgentInference::ModelServer).with_command_from_caller(vec!["bash".into()]);
    for adapter in [
        &OpenCodeAdapter::new() as &dyn AgentAdapter,
        &PiAdapter::new() as &dyn AgentAdapter,
    ] {
        assert_eq!(inference::finding(adapter, &ctx), None, "{}", adapter.id());
        assert_no_proxy_port(adapter, &ctx);
    }
    let settings: Value = serde_json::from_slice(
        files(&PiAdapter::new(), &ctx)
            .get(&pi::settings_dst(&ctx.home))
            .unwrap(),
    )
    .unwrap();
    assert!(settings.get("defaultProvider").is_none(), "{settings}");
    assert!(settings.get("defaultModel").is_none(), "{settings}");
    assert!(
        !files(&PiAdapter::new(), &ctx).contains_key(&pi::models_dst(&ctx.home)),
        "no provider of Humanitl for pi"
    );
}

#[test]
fn pi_refuses_own_providers_it_does_not_know() {
    let ctx = context(Some("http://192.168.1.50:11434"), AgentInference::Own);
    let found = inference::finding(&PiAdapter::new(), &ctx).expect("LLM_013");
    assert_eq!(found.code.as_str(), "LLM_013");
    assert_eq!(found.severity, Severity::Blocking);
    assert_eq!(
        found.fix,
        Some(FixAction::ChangeSetting {
            key: "agents.pi.inference".to_owned(),
            value: "model-server".to_owned(),
        })
    );
    let config = config("pi", Some("http://192.168.1.50:11434"), AgentInference::Own);
    assert!(
        inference::session_passthrough(&PiAdapter::new(), &config).is_empty(),
        "never a silent fallback to the model server"
    );
}

/// Die Durchreiche von `OpenCode` mit `own` vor dem mitgelieferten
/// Regelsatz, wie der Daemon beide als mitgeliefert einhängt.
fn own_rules_with_the_defaults() -> RuleSet {
    let adapter = OpenCodeAdapter::new();
    let config = config("opencode", None, AgentInference::Own);
    let passthrough = inference::session_passthrough(&adapter, &config);
    assert!(!passthrough.is_empty(), "own needs a passthrough");
    for rule in &passthrough {
        assert_eq!(rule.action, Action::Allow);
        assert!(rule.passthrough_llm);
        assert!(
            !rule.allow_private,
            "a vendor never lives in the own network"
        );
        assert_eq!(rule.matcher.methods, Some(vec![Method::POST]));
        assert!(
            !rule.matcher.path_prefixes.is_empty(),
            "never the whole host"
        );
    }
    let (defaults, _) = humanitl_rules::parse_rules(adapter.default_rules()).unwrap();
    let mut set = RuleSet::new();
    set.add_bundled(passthrough);
    set.add_bundled(defaults.iter().cloned());
    set
}

/// Mit `own` reicht `OpenCode` genau die Inferenz von `OpenCode Zen` durch,
/// nur mit `POST`, und die mitgelieferten Sperren auf `opencode.ai`
/// bleiben (Mutationsbeweis: ohne die Erklärung eigener Provider wird
/// dieser Test rot).
#[test]
fn opencode_own_passthrough_matches_only_the_zen_inference_paths() {
    let set = own_rules_with_the_defaults();

    let zen = HostName::parse(ZEN_HOST).unwrap();
    let catalog = HostName::parse("models.opencode.ai").unwrap();
    let other = HostName::parse("api.deepseek.com").unwrap();
    let now = jiff::Timestamp::now();
    for (host, method, path, expected) in [
        (&zen, Method::POST, "/zen/v1/chat/completions", Some(true)),
        (&zen, Method::POST, "/zen/v1/responses", Some(true)),
        (&zen, Method::POST, "/zen/v1/messages", Some(true)),
        (
            &zen,
            Method::POST,
            "/inference/openai/v1/chat/completions",
            Some(true),
        ),
        (
            &zen,
            Method::POST,
            "/inference/openai/v1/responses",
            Some(true),
        ),
        (
            &zen,
            Method::POST,
            "/inference/anthropic/v1/messages",
            Some(true),
        ),
        (
            &zen,
            Method::POST,
            "/inference/google/v1beta/models/gemini-3-flash:streamGenerateContent",
            Some(true),
        ),
        (
            &zen,
            Method::POST,
            "/inference/google/v1beta/models/gemini-3-flash:generateContent",
            Some(true),
        ),
        // Nur `POST`, nur genau diese Pfade.
        (&zen, Method::GET, "/zen/v1/chat/completions", None),
        (&zen, Method::GET, "/zen/v1/models", None),
        (&zen, Method::POST, "/zen/v1/chat/completions/x", None),
        (&zen, Method::POST, "/zen/v1/files", None),
        (
            &zen,
            Method::POST,
            "/zen/v1/chat/completions/../../../share/x",
            None,
        ),
        (
            &zen,
            Method::POST,
            "/inference/google/v1beta/models/x:delete",
            None,
        ),
        (
            &zen,
            Method::POST,
            "/inference/google/v1beta/models/a/b:generateContent",
            None,
        ),
        (&zen, Method::POST, "/", None),
        (&zen, Method::GET, "/auth", None),
        // Die mitgelieferten Sperren bleiben.
        (&zen, Method::GET, "/share/abc", Some(false)),
        (&zen, Method::POST, "/share/abc", Some(false)),
        (&catalog, Method::GET, "/api.json", Some(false)),
        // Ein mit `/connect` verbundener Anbieter wird gehalten.
        (&other, Method::POST, "/v1/chat/completions", None),
    ] {
        let key = RequestKey::new(host, &method, path, Scheme::Https, 443);
        let verdict = set.evaluate(&key, now, SessionId::nil());
        let got = match verdict {
            Verdict::Matched {
                action: Action::Allow,
                ..
            } => Some(true),
            Verdict::Matched {
                action: Action::Block,
                ..
            } => Some(false),
            _ => None,
        };
        assert_eq!(got, expected, "{method} https://{host}{path}: {verdict:?}");
    }
}

#[test]
fn opencode_own_providers_get_the_cloud_notice() {
    let config = config("opencode", None, AgentInference::Own);
    let found = humanitl_sandbox::agent::cloud::cloud_inference(&config).expect("the notice");
    assert_eq!(found.agent, "opencode");
    assert_eq!(found.provider, ZEN_HOST);
    // Mit einem lokalen Modellserver und `model-server` bleibt es still.
    let local = self::config(
        "opencode",
        Some("http://192.168.1.50:11434"),
        AgentInference::ModelServer,
    );
    assert_eq!(
        humanitl_sandbox::agent::cloud::cloud_inference(&local),
        None
    );
}
