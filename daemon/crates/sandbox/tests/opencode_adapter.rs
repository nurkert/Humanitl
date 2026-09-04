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

use humanitl_config::{AgentBriefing, AskMode, HoldConfig, Language, LlmConfig};
use humanitl_core::rule::{Action, HostPattern};
use humanitl_core::{HostName, Method, Scheme, SessionId, Severity};
use humanitl_rules::{RequestKey, RuleSet, Verdict};
use humanitl_sandbox::agent::briefing;
use humanitl_sandbox::agent::opencode::{
    CONFIG_DST, MANAGED_CONFIG_DST, MODELS_DST, OLLAMA_INFERENCE_PATHS, OPENAI_INFERENCE_PATHS,
    PLACEHOLDER_MODEL, briefing_dst, config_dst, keep_dst, passthrough_authority,
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
            config_dst(&ctx.config_home()).as_path(),
            keep_dst(&ctx.config_home()).as_path(),
            // Das verwaltete Verzeichnis: die einzige Quelle, die nach der
            // Konfiguration eines geklonten Projekts gemergt wird.
            Path::new(MANAGED_CONFIG_DST),
            // Die Einweisung (HUM-071), unter dem Konfigurationsverzeichnis
            // des Agenten und nicht im Projekt.
            briefing_dst(&ctx.config_home()).as_path(),
        ]
    );
    for file in &files {
        assert_eq!(file.mode, 0o444, "{:?} is writable", file.dst);
    }
}

// --- die Einweisung (HUM-071) ------------------------------------------------

/// Ein langer Endpunkt, damit die Zeichengrenze für den ungünstigsten Fall
/// gilt und nicht für den bequemsten. Derselbe Wert wie in
/// `tools/briefing-tokens.py`.
const WORST_ENDPOINT: &str = "http://ollama.services.example.internal:11434";

/// So viele Zeichen darf die gerenderte Einweisung höchstens haben.
///
/// Die Grenze des Issues ist eine Token-Grenze
/// ([`briefing::TOKEN_BUDGET`](humanitl_sandbox::agent::briefing::TOKEN_BUDGET)),
/// und in dieser Crate liegt kein Tokenisierer. Diese Zahl ist der billige
/// Stolperdraht daneben: gemessen am 2026-09-04 mit `tools/briefing-tokens.py`
/// braucht der längste Fall (Deutsch, `ask_mode = ui`, langer Endpunkt,
/// vierstellige Frist) 731 Zeichen und 184 Token in `o200k_base`. Wer den Text
/// über diese Grenze wachsen lässt, zählt vorher nach; die Zahl hier wird
/// nicht heraufgesetzt, ohne dass das Skript grün ist.
const BRIEFING_MAX_CHARS: usize = 780;

/// Die Einweisung liegt im Heimatverzeichnis des Agenten, nie unter `/work`.
///
/// `OpenCode` liest die `AGENTS.md` des Projekts zusätzlich; eine Datei, die
/// Humanitl dort ablegte, stünde im Diff des Nutzers und irgendwann in einem
/// fremden Repository (ADR-0014).
#[test]
fn briefing_written_outside_work() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some(WORST_ENDPOINT));
    let files = adapter.files(&ctx).unwrap();

    let briefing_file = files
        .iter()
        .find(|file| file.dst == briefing_dst(&ctx.config_home()))
        .expect("the briefing is among the files of the adapter");
    assert_eq!(
        briefing_file.dst,
        Path::new("/home/agent/.config/opencode/AGENTS.md"),
        "the path OpenCode reads first"
    );
    assert_eq!(briefing_file.mode, 0o444, "the agent may not rewrite it");
    assert!(
        files_inside_work(&files, Path::new("/work")).is_empty(),
        "nothing of the adapter is in the project directory"
    );
    for file in &files {
        assert!(
            !file.dst.starts_with("/work"),
            "{:?} would land in the project",
            file.dst
        );
    }
}

/// Kein Platzhalter bleibt stehen, und die Werte im Text sind die der
/// Konfiguration.
#[test]
fn briefing_placeholders_replaced() {
    let adapter = OpenCodeAdapter::new();
    for language in [Language::En, Language::De] {
        let ctx = context(Some(WORST_ENDPOINT))
            .with_language(language)
            .with_hold(HoldConfig {
                timeout_secs: 90,
                ..HoldConfig::default()
            });
        let text = String::from_utf8(file_at(
            adapter,
            &ctx,
            "/home/agent/.config/opencode/AGENTS.md",
        ))
        .expect("the briefing is UTF-8");

        assert!(
            !text.contains('{'),
            "{language:?}: a placeholder is left: {text}"
        );
        assert!(
            !text.contains("<!--"),
            "{language:?}: a comment leaked: {text}"
        );
        assert!(
            !text.contains("-->"),
            "{language:?}: a comment leaked: {text}"
        );
        assert!(
            text.contains("ollama.services.example.internal:11434"),
            "{language:?}: the endpoint of the configuration is in the text: {text}"
        );
        assert!(
            text.contains("90"),
            "{language:?}: the deadline of the configuration is in the text: {text}"
        );
        assert!(
            !text.contains("300"),
            "{language:?}: no deadline that is not configured: {text}"
        );
        assert!(
            text.contains("Blocked by Humanitl."),
            "{language:?}: the banner is quoted verbatim: {text}"
        );
        assert!(text.ends_with('\n'), "{language:?}: one newline at the end");
    }
}

/// Der Text nennt den Meta-Endpunkt und im selben Atemzug seine Grenze.
///
/// `POST /ask` erzeugt eine Karte in der Oberfläche und **nie** eine Regel
/// (ADR-0014, HUM-073 „Nicht-Ziel"). Ein Briefing, das das offenließe, legte
/// dem Agenten nahe, er könne sich selbst Zugang verschaffen.
#[test]
fn briefing_names_the_meta_endpoint_and_its_limit() {
    let adapter = OpenCodeAdapter::new();
    for (language, only_the_user) in [
        (Language::En, "Only the user can add a rule."),
        (Language::De, "Nur der Nutzer kann eine Regel anlegen."),
    ] {
        let ctx = context(Some(WORST_ENDPOINT)).with_language(language);
        let text = String::from_utf8(file_at(
            adapter,
            &ctx,
            "/home/agent/.config/opencode/AGENTS.md",
        ))
        .expect("the briefing is UTF-8");
        assert!(
            text.contains("http://humanitl.internal/"),
            "{language:?}: the status path is in the text: {text}"
        );
        assert!(
            text.contains("`POST /ask`"),
            "{language:?}: the way to the user is in the text: {text}"
        );
        assert!(
            text.contains("202"),
            "{language:?}: the answer of /ask is 202, and the text says so: {text}"
        );
        assert!(
            text.contains(only_the_user),
            "{language:?}: the limit of /ask is in the text: {text}"
        );
    }
}

/// Der Host im Text kommt aus derselben Quelle wie die Durchreichregel.
///
/// Codex-Befund vom 2026-09-04: `http://good.test`#x`:11434` übersteht
/// `Url::parse`, aber nicht `HostName::parse`. `llm_passthrough` baut dafür
/// keine Regel — und dann darf das Briefing den Host auch nicht nennen, sonst
/// verspricht es eine Durchreiche, die es nicht gibt.
#[test]
fn briefing_names_a_host_only_when_a_passthrough_rule_exists() {
    let adapter = OpenCodeAdapter::new();
    // Ohne diesen Zähler wäre der Test still leer, wenn `Url::parse` alle drei
    // Werte selbst ablehnte: er prüfte dann nichts und meldete grün.
    let mut without_a_rule = 0_usize;
    for endpoint in [
        "http://good.test`#x`:11434",
        "http://[::1]x:11434",
        "file:///etc/passwd",
    ] {
        let Ok(url) = url::Url::parse(endpoint) else {
            continue;
        };
        let llm = LlmConfig {
            endpoint: Some(url),
            ..LlmConfig::default()
        };
        let ctx = AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm.clone());
        let text = String::from_utf8(file_at(
            adapter,
            &ctx,
            "/home/agent/.config/opencode/AGENTS.md",
        ))
        .expect("the briefing is UTF-8");

        let rule = adapter.llm_passthrough(&llm);
        assert_eq!(
            rule.is_some(),
            passthrough_authority(&llm).is_some(),
            "{endpoint}: rule and briefing disagree about the passthrough"
        );
        if rule.is_none() {
            without_a_rule += 1;
            assert!(
                !text.contains("11434"),
                "{endpoint}: no rule, so the briefing names no host: {text}"
            );
            assert!(
                !text.contains("good.test"),
                "{endpoint}: no rule, so the briefing names no host: {text}"
            );
        }
    }
    assert!(
        without_a_rule > 0,
        "at least one of the endpoints must survive Url::parse and still yield no rule, \
         otherwise this test proves nothing"
    );

    // Die Gegenprobe: ein brauchbarer Endpunkt steht im Text, mit dem Port,
    // auf den die Regel passt.
    let llm = LlmConfig {
        endpoint: Some(url::Url::parse("http://ollama.lan").unwrap()),
        ..LlmConfig::default()
    };
    assert_eq!(
        passthrough_authority(&llm).as_deref(),
        Some("ollama.lan:80"),
        "the default port of the scheme is the port of the rule"
    );
    let ctx = AgentContext::new(SessionId::nil(), PathBuf::from("/home/u/proj"), llm);
    let text = String::from_utf8(file_at(
        adapter,
        &ctx,
        "/home/agent/.config/opencode/AGENTS.md",
    ))
    .expect("the briefing is UTF-8");
    assert!(text.contains("ollama.lan:80"), "{text}");
}

/// `sandbox.env` kann `XDG_CONFIG_HOME` überschreiben; die Dateien folgen.
///
/// `sandbox.env` wird nach dem Beitrag des Adapters in die Umgebung gelegt und
/// gewinnt. Ohne diesen Weg hängte die Sandbox die Einweisung nach
/// `/home/agent/.config`, während `OpenCode` in `/etc/xdg` läse: die Datei
/// wäre da und würde nie gelesen (Codex-Befund vom 2026-09-04).
#[test]
fn briefing_follows_an_overridden_config_home() {
    let adapter = OpenCodeAdapter::new();
    let ctx =
        context(Some(WORST_ENDPOINT)).with_config_home(Some(PathBuf::from("/etc/xdg-of-the-user")));

    let files = adapter.files(&ctx).unwrap();
    let targets: Vec<&Path> = files.iter().map(|file| file.dst.as_path()).collect();
    assert!(
        targets.contains(&Path::new("/etc/xdg-of-the-user/opencode/AGENTS.md")),
        "the briefing follows the configuration directory: {targets:?}"
    );
    assert!(
        targets.contains(&Path::new("/etc/xdg-of-the-user/opencode/opencode.json")),
        "and so does the configuration: {targets:?}"
    );
    assert!(
        !targets.contains(&Path::new("/home/agent/.config/opencode/AGENTS.md")),
        "nothing stays behind in the derived directory: {targets:?}"
    );

    // Und die Umgebung zeigt auf denselben Ort, sonst läge die Datei wieder
    // dort, wo niemand sie liest.
    let env = env_map(adapter, &ctx);
    assert_eq!(
        env.get("XDG_CONFIG_HOME").map(String::as_str),
        Some("/etc/xdg-of-the-user")
    );

    // Ein relativer Pfad ist kein Ort: der abgeleitete bleibt.
    let relative = context(None).with_config_home(Some(PathBuf::from("config")));
    assert_eq!(relative.config_home(), Path::new("/home/agent/.config"));
}

/// Beide Sprachen bleiben in der Grenze, in jedem Ask-Modus.
#[test]
fn briefing_stays_within_the_budget() {
    for language in [Language::En, Language::De] {
        for ask_mode in [AskMode::Ui, AskMode::Terminal, AskMode::None] {
            let llm = LlmConfig {
                endpoint: Some(url::Url::parse(WORST_ENDPOINT).unwrap()),
                ..LlmConfig::default()
            };
            let text = briefing::render(
                language,
                ask_mode,
                3600,
                passthrough_authority(&llm).as_deref(),
            )
            .expect("the bundled template renders");
            assert!(
                text.chars().count() <= BRIEFING_MAX_CHARS,
                "{language:?}/{ask_mode:?} is {} characters, the limit is {BRIEFING_MAX_CHARS}; \
                 count the tokens with tools/briefing-tokens.py before raising it",
                text.chars().count()
            );
        }
    }
}

/// `agent.briefing.enabled = false` unterdrückt die Datei, und sonst nichts.
#[test]
fn briefing_can_be_switched_off() {
    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some(WORST_ENDPOINT)).with_briefing(AgentBriefing { enabled: false });
    let files = adapter.files(&ctx).unwrap();

    assert!(
        !files
            .iter()
            .any(|file| file.dst == briefing_dst(&ctx.config_home())),
        "the briefing is gone"
    );
    assert_eq!(
        files.len(),
        5,
        "the configuration of the agent stays: {:?}",
        files.iter().map(|file| &file.dst).collect::<Vec<_>>()
    );
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
    assert!(
        rule.passthrough_llm,
        "the proxy recognises the declared exception by this flag, not by the id"
    );
    assert_eq!(
        rule.matcher.path, None,
        "the boundary is the prefix list, not a regex"
    );
    assert_eq!(
        rule.matcher.path_prefixes,
        OPENAI_INFERENCE_PATHS
            .iter()
            .chain(OLLAMA_INFERENCE_PATHS)
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
        "both surface prefixes of llm.passthrough_paths are narrowed to inference endpoints"
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
        // Modelle nachladen, anlegen und löschen ändern den Server. Sie
        // gehören nicht in die eine Regel, die nicht gehalten wird.
        (&endpoint, Method::POST, "/api/pull", 11434, false),
        (&endpoint, Method::POST, "/api/create", 11434, false),
        (&endpoint, Method::DELETE, "/api/delete", 11434, false),
        (
            &endpoint,
            Method::POST,
            "/api/blobs/sha256:aa",
            11434,
            false,
        ),
        // Dasselbe unter `/v1/`: Dateien ablegen, Vektorspeicher anlegen,
        // feinabstimmen und LoRA-Adapter laden sind keine Inferenz.
        (&endpoint, Method::POST, "/v1/files", 11434, false),
        (&endpoint, Method::POST, "/v1/vector_stores", 11434, false),
        (
            &endpoint,
            Method::POST,
            "/v1/fine_tuning/jobs",
            11434,
            false,
        ),
        (
            &endpoint,
            Method::POST,
            "/v1/load_lora_adapter",
            11434,
            false,
        ),
        (&endpoint, Method::POST, "/v1/models/../files", 11434, false),
        // Und auch nicht über einen Umweg, den erst der Server auflöst.
        (&endpoint, Method::POST, "/api/chat/../pull", 11434, false),
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
    let Some((fx, backend)) =
        sandbox_or_skip("the_adapter_files_arrive_in_a_real_sandbox_and_the_guarantees_hold")
    else {
        return;
    };

    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434")).with_models(vec!["qwen3".to_owned()]);
    let mut session = fx.context(&[
        "sh",
        "-c",
        "cat /etc/humanitl/opencode/opencode.json; echo ---MARK---; env | grep OPENCODE_ | sort; \
         echo ---BRIEFING---; cat /home/agent/.config/opencode/AGENTS.md",
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

    let (config, rest) = stdout.split_once("---MARK---\n").expect("the marker");
    let (env, briefing_text) = rest
        .split_once("---BRIEFING---\n")
        .expect("the second marker");
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

    // Die Einweisung liegt dort, wo `OpenCode` seine globale Instruktionsdatei
    // sucht, und trägt die Werte dieser Sitzung (HUM-071).
    assert!(
        briefing_text.contains("Blocked by Humanitl."),
        "the briefing arrived in the sandbox: {briefing_text:?}"
    );
    assert!(
        briefing_text.contains("192.168.1.50:11434"),
        "the briefing names the endpoint of this session: {briefing_text:?}"
    );
    assert!(
        !briefing_text.contains('{'),
        "no placeholder survives into the sandbox: {briefing_text:?}"
    );

    // Der eigentliche Punkt: die drei Garantien halten auch mit den sechs
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
    let Some((fx, backend)) =
        sandbox_or_skip("the_adapter_files_are_read_only_and_work_stays_clean")
    else {
        return;
    };

    // Ohne eine Datei im Projekt bewiese ein leeres `ls -A /work` nichts: die
    // Zusicherungen unten wären auch dann wahr, wenn die Sandbox gar nicht
    // gelaufen wäre.
    std::fs::write(fx.work.join("canary.txt"), b"canary\n").expect("canary");

    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434")).with_models(vec!["qwen3".to_owned()]);
    let mut session = fx.context(&[
        "sh",
        "-c",
        "echo x > /etc/humanitl/opencode/opencode.json 2>&1; echo \"config_rc=$?\"; \
         echo y > /home/agent/.config/opencode/AGENTS.md 2>&1; echo \"briefing_rc=$?\"; \
         ls -A /work",
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
        stdout.contains("canary.txt"),
        "the sandbox really ran and really saw the project: {stdout:?}"
    );
    assert!(
        !stdout.contains("config_rc=0"),
        "the agent must not be able to rewrite its own configuration: {stdout}"
    );
    // Dasselbe für die Einweisung: sie ist der Text, an dem der Agent sein
    // Bild der Umgebung ausrichtet, und ein Agent, der ihn umschreiben kann,
    // kann sich selbst etwas anderes erzählen (HUM-071).
    assert!(
        !stdout.contains("briefing_rc=0"),
        "the agent must not be able to rewrite its own briefing: {stdout}"
    );
    assert!(
        !stdout.contains("opencode.json"),
        "nothing of the adapter lands in the project directory: {stdout}"
    );
    assert!(
        !stdout.contains("AGENTS.md"),
        "the briefing never lands in the project directory: {stdout}"
    );
}

/// Nach dem Start ist das Projektverzeichnis byte-identisch.
///
/// Das ist das Akzeptanzkriterium des Issues und zugleich der Grund, warum die
/// Einweisung im Heimatverzeichnis liegt: `OpenCode` liest die `AGENTS.md` des
/// Projekts zusätzlich, und was Humanitl dort ablegte, käme in den nächsten
/// Commit des Nutzers (ADR-0014).
#[test]
fn the_briefing_leaves_the_project_directory_byte_identical() {
    let Some((fx, backend)) =
        sandbox_or_skip("the_briefing_leaves_the_project_directory_byte_identical")
    else {
        return;
    };

    std::fs::create_dir_all(fx.work.join("src")).expect("src");
    std::fs::write(fx.work.join("README.md"), b"# project\n").expect("readme");
    std::fs::write(fx.work.join("src/main.rs"), b"fn main() {}\n").expect("source");
    let before = tree_snapshot(&fx.work);

    let adapter = OpenCodeAdapter::new();
    let ctx = context(Some("http://192.168.1.50:11434"));
    let mut session = fx.context(&[
        "sh",
        "-c",
        "cat /home/agent/.config/opencode/AGENTS.md >/dev/null; ls -A /work",
    ]);
    session.files = adapter.files(&ctx).expect("the adapter renders its files");

    let plan = backend
        .plan(&fx.profile, &session)
        .expect("the plan builds");
    let handle = backend.launch(&plan).expect("bwrap starts");
    let status = handle
        .wait_timeout(std::time::Duration::from_secs(30))
        .expect("the sandbox ends in time")
        .expect("bwrap ran the command");
    let output = handle.output().expect("captured");
    assert!(
        status.success(),
        "{status}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("README.md") && stdout.contains("src"),
        "the sandbox really ran and really saw the project: {stdout:?}"
    );
    assert_eq!(
        tree_snapshot(&fx.work),
        before,
        "the project directory changed while the sandbox ran"
    );
}

/// Jeder Pfad unter `root` mit seinem Inhalt, sortiert.
///
/// Der Vergleich ist byteweise und nicht über eine Prüfsumme: die Bäume hier
/// sind winzig, und ein Unterschied soll in der Meldung des Tests stehen und
/// nicht als zwei ungleiche Zahlen.
fn tree_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).expect("the work directory is readable");
        for entry in entries {
            let entry = entry.expect("a directory entry");
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("below the root")
                .to_path_buf();
            if entry.file_type().expect("a file type").is_dir() {
                out.push((relative, Vec::new()));
                stack.push(path);
            } else {
                out.push((relative, std::fs::read(&path).expect("a readable file")));
            }
        }
    }
    out.sort();
    out
}

/// Fixture und `bwrap`, oder ein ausdrücklicher Übersprung.
///
/// Ein Test, der still nichts prüft, ist schlimmer als keiner: Er meldet
/// grün und belegt nichts, und niemand sieht den Unterschied. Fehlt hier
/// etwas, steht der Grund als `SKIP <test>: <grund>` auf `stderr`, und unter
/// CI ist es ein Fehlschlag — dort sind `bwrap` und der Shim zugesagt
/// (`.github/workflows/ci.yml`, Job `rust-check` installiert `bubblewrap`).
/// Dasselbe Muster wie `sandbox_required` in `bin/humanitl/tests/cli.rs`
/// (Codex-Befund vom 2026-09-04).
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

/// Meldet den Übersprung und verlangt ihn unter CI nicht zu geben.
fn skip<T>(test: &str, why: &str) -> Option<T> {
    assert!(
        std::env::var_os("CI").is_none(),
        "under CI this test must run, it is the only place that proves the adapter files \
         reach a real sandbox: {test} would skip because {why}. Install bubblewrap \
         (apt-get install -y bubblewrap), allow unprivileged user namespaces \
         (sysctl -w kernel.apparmor_restrict_unprivileged_userns=0) and build the \
         workspace (cargo build --workspace) so humanitl-shim sits next to the test binary."
    );
    eprintln!("SKIP {test}: {why}");
    None
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
    /// Die Umgebung, oder der Grund, warum es sie hier nicht gibt.
    fn new() -> Result<Self, String> {
        let why = |what: &str, err: &dyn std::fmt::Display| format!("{what}: {err}");
        let dir = tempfile::tempdir().map_err(|err| why("no temporary directory", &err))?;
        let root = dir.path().to_path_buf();
        let work = root.join("work");
        std::fs::create_dir_all(&work).map_err(|err| why("no work directory", &err))?;
        // Die eine Tür liegt im Proxy-Verzeichnis der Laufzeit und trägt
        // 0600; alles andere lehnt der Planer mit `SANDBOX_006` ab.
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
        }
    }

    /// Das echte `bwrap`, oder der Grund, warum es hier keines gibt.
    fn backend(&self) -> Result<humanitl_sandbox::BwrapBackend, String> {
        humanitl_sandbox::BwrapBackend::detect(self.paths.clone())
            .map(|backend| backend.with_stdio(humanitl_sandbox::StdioMode::Capture))
            .map_err(|err| format!("no usable bwrap: {err}"))
    }
}

/// Der gebaute Shim neben dem Testbinary.
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
