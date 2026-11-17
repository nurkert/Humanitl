//! Inferenz bei einem Anbieter außerhalb dieses Rechners (HUM-516).
//!
//! Mit einem Cloud-Modell fließen über die Inferenz auf jeden Fall Daten ab:
//! Prompts, gelesene Dateien und Werkzeugausgaben gehen als Teil der
//! Anfragen an den Anbieter. Humanitl gibt die Inferenz bewusst frei und hält
//! jede andere Verbindung an; wer einen Cloud-Agenten nutzt, soll das wissen.
//!
//! Dieses Modul hält die zwei Dinge, die keinen Adapter brauchen:
//!
//! - **Die Lesart des Endpunkts** ([`provider_outside`]): Ein Endpunkt auf
//!   dem Loopback, an einer privaten Adresse, unter einem Namen des eigenen
//!   Netzes ([`crate::llm_endpoint::is_private_by_name`]) oder ein
//!   gespeicherter Server mit `local = true` ist lokal; jeder andere ist ein
//!   Anbieter außerhalb. Welchen Endpunkt ein Agent anspricht, weiß sein
//!   Adapter (`AgentAdapter::inference_endpoint` in `humanitl-sandbox`).
//! - **Die Bestätigung** je Agent und Anbieter ([`acknowledge`],
//!   [`is_acknowledged`]), im Datenverzeichnis des Daemons
//!   ([`crate::paths::Paths::cloud_inference_path`]) und nie im Projekt:
//!   Das Projekt beschreibt der Agent, und eine Bestätigung dort könnte er
//!   selbst ausstellen. Gelesen und gesetzt wird sie über die RPC des
//!   Daemons (ADR-018), nie von einer Oberfläche selbst.
//!
//! Der Hinweis blockiert nichts. Die Bestätigung entscheidet nur, ob ein
//! Client beim Start noch einmal fragt.

use humanitl_core::diagnostics::codes::{CONFIG_003, LLM_010};
use humanitl_core::{Diagnostic, FixAction, HostName, Severity};
use toml::Value as TomlValue;
use url::Url;

use crate::edit::Edit;
use crate::model::LlmConfig;
use crate::paths::Paths;

/// Die Seite in `docs/SECURITY.md`, die den Fall beschreibt.
pub const DOCS_URL: &str =
    "https://github.com/nurkert/Humanitl/blob/main/docs/SECURITY.md#cloud-inference-hum-516";

/// Längste Kennung eines Agenten, die eine Bestätigung trägt; dieselbe Form
/// wie der Name eines gespeicherten Servers.
const AGENT_MAX_CHARS: usize = 32;

/// Der Host des Anbieters, an den die Inferenz über `endpoint` geht, oder
/// `None`, wenn der Endpunkt lokal ist.
///
/// Lokal ist ein Endpunkt, der nach seinem Host allein im eigenen Netz liegt
/// ([`crate::llm_endpoint::is_private_by_name`]), und einer, dessen Herkunft
/// (Schema, Host, Port) die des gewählten gespeicherten Servers mit
/// `local = true` ist. Die Markierung gilt nur dem Server, den die Sitzung
/// gewählt hat, und nur seiner Adresse: Ein anderer Endpunkt aus einer
/// höheren Ebene (`--llm URL`) ist durch sie nicht lokal.
///
/// Der Host steht in der kanonischen Form des Proxys
/// ([`HostName::parse`]), ohne Port, Pfad und Zugangsdaten. Ein Endpunkt
/// ohne lesbaren Host hat keinen Anbieter; die Konfiguration meldet ihn
/// selbst.
#[must_use]
pub fn provider_outside(llm: &LlmConfig, endpoint: &Url) -> Option<String> {
    if crate::llm_endpoint::is_private_by_name(endpoint) || marked_local(llm, endpoint) {
        return None;
    }
    let host = HostName::parse(endpoint.host_str()?).ok()?;
    Some(host.to_string())
}

/// Ob der gewählte gespeicherte Server `local = true` trägt und dieselbe
/// Herkunft hat wie `endpoint`.
fn marked_local(llm: &LlmConfig, endpoint: &Url) -> bool {
    let Some(server) = llm.selected_server().and_then(|name| llm.servers.get(name)) else {
        return false;
    };
    server.local
        && server
            .endpoint
            .as_deref()
            .and_then(|text| Url::parse(text).ok())
            .is_some_and(|saved| {
                let origin = saved.origin();
                origin.is_tuple() && origin == endpoint.origin()
            })
}

/// Der Hinweis als Befund, für die Kommandozeile und jeden Client, der keinen
/// eigenen Text hat (`LLM_010`, Warnung, blockiert nichts).
#[must_use]
pub fn notice(agent: &str, provider: &str) -> Diagnostic {
    Diagnostic::builder(LLM_010, Severity::Warning)
        .why(notice_text(agent, provider))
        .fix(FixAction::OpenUrl(DOCS_URL.to_owned()))
        .build()
}

/// Der ganze Text des Hinweises, sachlich und in einem Zug.
#[must_use]
pub fn notice_text(agent: &str, provider: &str) -> String {
    format!(
        "Cloud inference: {agent} sends its requests to the model at {provider}. What the \
         agent reads goes to {provider} as part of those requests. Humanitl isolates the \
         system and holds every other connection; the inference itself is deliberately \
         allowed."
    )
}

/// Die kurze Zeile für Tag, Statuszeile und Start.
#[must_use]
pub fn short_line(provider: &str) -> String {
    format!("Cloud inference: data goes to {provider}")
}

/// Prüft eine Kennung des Agenten, wie sie in der Datei der Bestätigungen
/// steht: 1 bis 32 Zeichen `a-z`, `0-9`, `-` und `_`.
///
/// # Errors
///
/// `CONFIG_003`, wenn die Kennung diese Form nicht hat.
pub fn check_agent(agent: &str) -> Result<(), Diagnostic> {
    let ok = !agent.is_empty()
        && agent.len() <= AGENT_MAX_CHARS
        && agent.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if ok {
        return Ok(());
    }
    Err(Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{:?} is not the id of an agent: 1 to {AGENT_MAX_CHARS} characters a-z, 0-9, - and _",
            agent.chars().take(AGENT_MAX_CHARS + 8).collect::<String>()
        ))
        .fix(FixAction::ChangeSetting {
            key: "agent.adapter".to_owned(),
            value: "opencode".to_owned(),
        })
        .build())
}

/// Prüft und normalisiert den Host eines Anbieters.
///
/// # Errors
///
/// `CONFIG_003`, wenn der Text kein Host ist, den der Proxy annähme.
pub fn check_provider(provider: &str) -> Result<String, Diagnostic> {
    HostName::parse(provider)
        .map(|host| host.to_string())
        .map_err(|error| {
            Diagnostic::builder(CONFIG_003, Severity::Error)
                .why(format!("the provider is not a host name: {error}"))
                .fix(FixAction::ChangeSetting {
                    key: "llm.endpoint".to_owned(),
                    value: "https://api.example.com/v1".to_owned(),
                })
                .build()
        })
}

/// Merkt, dass der Mensch den Hinweis für `agent` mit `provider` bestätigt
/// hat. Ein zweites Mal ändert nichts.
///
/// # Errors
///
/// `CONFIG_003` für eine Kennung oder einen Host, die [`check_agent`]
/// beziehungsweise [`check_provider`] ablehnen; die Befunde von
/// [`crate::edit::edit_with`], wenn die Datei nicht geschrieben wurde.
pub fn acknowledge(paths: &Paths, agent: &str, provider: &str) -> Result<(), Diagnostic> {
    check_agent(agent)?;
    let provider = check_provider(provider)?;
    crate::edit::edit_with(
        &paths.cloud_inference_path(),
        |old| {
            let mut providers = acknowledged_in(old, agent);
            if providers.contains(&provider) {
                return Ok(Vec::new());
            }
            providers.push(provider.clone());
            providers.sort();
            Ok(vec![Edit::new(
                vec![
                    "agents".to_owned(),
                    agent.to_owned(),
                    "providers".to_owned(),
                ],
                Some(TomlValue::Array(
                    providers.into_iter().map(TomlValue::String).collect(),
                )),
            )])
        },
        |_| Ok(()),
    )?;
    Ok(())
}

/// Ob der Hinweis für `agent` mit `provider` bestätigt ist. Eine Datei, die
/// sich nicht lesen lässt, bestätigt nichts; dann fragt der nächste Start
/// noch einmal, und das ist die sichere Seite.
#[must_use]
pub fn is_acknowledged(paths: &Paths, agent: &str, provider: &str) -> bool {
    let Ok(provider) = check_provider(provider) else {
        return false;
    };
    let file = paths.cloud_inference_path();
    let Ok(text) = std::fs::read_to_string(&file) else {
        return false;
    };
    let Ok(table) = crate::edit::parse_text(&file, &text) else {
        return false;
    };
    acknowledged_in(&table, agent).contains(&provider)
}

/// Die bestätigten Anbieter von `agent` in der Tabelle der Datei.
fn acknowledged_in(table: &toml::Table, agent: &str) -> Vec<String> {
    table
        .get("agents")
        .and_then(TomlValue::as_table)
        .and_then(|agents| agents.get(agent))
        .and_then(TomlValue::as_table)
        .and_then(|entry| entry.get("providers"))
        .and_then(TomlValue::as_array)
        .map(|providers| {
            providers
                .iter()
                .filter_map(TomlValue::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::model::LlmServer;
    use crate::{Env, Paths};

    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn a_public_host_is_a_provider_outside() {
        let llm = LlmConfig::default();
        assert_eq!(
            provider_outside(&llm, &url("https://api.anthropic.com/v1")).as_deref(),
            Some("api.anthropic.com")
        );
        assert_eq!(
            provider_outside(&llm, &url("https://API.OpenAI.com./v1")).as_deref(),
            Some("api.openai.com")
        );
        assert_eq!(
            provider_outside(&llm, &url("http://203.0.113.7:8080/v1")).as_deref(),
            Some("203.0.113.7")
        );
    }

    #[test]
    fn loopback_lan_and_local_names_are_not() {
        let llm = LlmConfig::default();
        for local in [
            "http://localhost:11434",
            "http://127.0.0.1:11434/v1",
            "http://[::1]:8080/v1",
            "http://192.168.1.20:11434",
            "http://10.0.0.5/v1",
            "http://[fd00::5]:11434",
            "http://gpu.lan:11434",
            "http://box.local:11434",
            "http://llm.humanitl.internal:11434",
            "http://nas.home.arpa/v1",
        ] {
            assert_eq!(provider_outside(&llm, &url(local)), None, "{local}");
        }
    }

    fn llm_with_server(endpoint: &str, local: bool) -> LlmConfig {
        let mut servers = BTreeMap::new();
        servers.insert(
            "office".to_owned(),
            LlmServer {
                endpoint: Some(endpoint.to_owned()),
                local,
                ..LlmServer::default()
            },
        );
        LlmConfig {
            endpoint: Some(url(endpoint)),
            servers,
            active: Some("office".to_owned()),
            ..LlmConfig::default()
        }
    }

    #[test]
    fn a_saved_server_marked_local_is_not_a_provider_outside() {
        let marked = llm_with_server("https://llm.example.org/v1", true);
        assert_eq!(
            provider_outside(&marked, &url("https://llm.example.org/v1")),
            None
        );
        let unmarked = llm_with_server("https://llm.example.org/v1", false);
        assert_eq!(
            provider_outside(&unmarked, &url("https://llm.example.org/v1")).as_deref(),
            Some("llm.example.org")
        );
    }

    #[test]
    fn the_mark_covers_only_the_address_of_its_server() {
        let marked = llm_with_server("https://llm.example.org/v1", true);
        // Ein anderer Endpunkt aus einer höheren Ebene (`--llm URL`).
        assert_eq!(
            provider_outside(&marked, &url("https://api.openai.com/v1")).as_deref(),
            Some("api.openai.com")
        );
        // Dieselbe Adresse auf einem anderen Port ist eine andere Herkunft.
        assert_eq!(
            provider_outside(&marked, &url("https://llm.example.org:8443/v1")).as_deref(),
            Some("llm.example.org")
        );
    }

    #[test]
    fn an_acknowledgement_is_per_agent_and_provider() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        assert!(!is_acknowledged(&paths, "opencode", "api.openai.com"));
        acknowledge(&paths, "opencode", "API.OpenAI.com").unwrap();
        assert!(is_acknowledged(&paths, "opencode", "api.openai.com"));
        assert!(!is_acknowledged(&paths, "pi", "api.openai.com"));
        assert!(!is_acknowledged(&paths, "opencode", "api.anthropic.com"));
        // Ein zweites Mal ändert nichts, ein zweiter Anbieter kommt dazu.
        acknowledge(&paths, "opencode", "api.openai.com").unwrap();
        acknowledge(&paths, "opencode", "api.anthropic.com").unwrap();
        let text = std::fs::read_to_string(paths.cloud_inference_path()).unwrap();
        assert_eq!(text.matches("api.openai.com").count(), 1, "{text}");
        assert!(is_acknowledged(&paths, "opencode", "api.anthropic.com"));
        // Im Datenverzeichnis, nie neben einem Projekt.
        assert!(
            paths
                .cloud_inference_path()
                .starts_with(dir.path().join("data"))
        );
    }

    #[test]
    fn junk_is_refused_and_never_written() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        for agent in ["", "Open Code", "../x", &"a".repeat(33)] {
            let refused = acknowledge(&paths, agent, "api.openai.com").unwrap_err();
            assert_eq!(refused.code, CONFIG_003, "{agent}");
        }
        let refused = acknowledge(&paths, "opencode", "not a host").unwrap_err();
        assert_eq!(refused.code, CONFIG_003);
        assert!(!paths.cloud_inference_path().exists());
    }

    #[test]
    fn the_notice_names_the_provider_and_blocks_nothing() {
        let notice = notice("claude", "api.anthropic.com");
        assert_eq!(notice.code, LLM_010);
        assert_eq!(notice.severity, Severity::Warning);
        assert!(notice.why.contains("api.anthropic.com"), "{}", notice.why);
        assert!(
            notice.why.contains("deliberately allowed"),
            "{}",
            notice.why
        );
        assert!(notice.fix.is_some());
    }
}
