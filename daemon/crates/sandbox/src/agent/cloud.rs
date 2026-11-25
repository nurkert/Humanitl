//! Ob ein Agent seine Inferenz an einen Anbieter außerhalb dieses Rechners
//! schickt (HUM-516).
//!
//! Die eine Stelle, an der Adapter und Endpunkt zusammenkommen: Der Adapter
//! nennt den Endpunkt ([`AgentAdapter::inference_endpoint`]), die
//! Konfiguration sagt, ob er lokal ist
//! ([`humanitl_config::inference::provider_outside`]). Schnappschuss der
//! Sitzung, Doctor, Start in der Kommandozeile und Einstellungen lesen den
//! Hinweis hier, damit er überall derselbe ist. Jeder spätere Adapter (Claude
//! Code, Codex, `agy`, `OpenCode` oder pi mit einem Cloud-Endpunkt) bekommt
//! ihn, ohne selbst etwas dafür zu tun.

use humanitl_config::Config;
use humanitl_core::Diagnostic;

use crate::agent::{AdapterRegistry, AgentAdapter};

/// Ein Agent, dessen Inferenz an einen Anbieter außerhalb dieses Rechners
/// geht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudInference {
    /// Die Kennung des Adapters (`agent.adapter`), etwa `opencode`.
    pub agent: String,
    /// Der Host des Anbieters, etwa `api.anthropic.com`.
    pub provider: String,
}

impl CloudInference {
    /// Der Hinweis als Befund (`LLM_010`).
    #[must_use]
    pub fn notice(&self) -> Diagnostic {
        humanitl_config::inference::notice(&self.agent, &self.provider)
    }

    /// Die kurze Zeile für Tag, Statuszeile und Start.
    #[must_use]
    pub fn short_line(&self) -> String {
        humanitl_config::inference::short_line(&self.provider)
    }
}

/// Der Hinweis für den Agenten dieser Konfiguration, oder `None`, wenn seine
/// Inferenz lokal bleibt oder er keinen Endpunkt hat.
///
/// Ein Adapter, den das Register nicht kennt, startet nicht; für ihn gilt
/// trotzdem `llm.endpoint`, denn die Durchreiche darauf steht auch ohne ihn.
#[must_use]
pub fn cloud_inference(config: &Config) -> Option<CloudInference> {
    let registry = AdapterRegistry::builtin();
    cloud_inference_with(registry.get(&config.agent.adapter), config)
}

/// [`cloud_inference`] mit einem gegebenen Adapter; für Tests und für
/// Adapter außerhalb des Registers.
#[must_use]
pub fn cloud_inference_with(
    adapter: Option<&dyn AgentAdapter>,
    config: &Config,
) -> Option<CloudInference> {
    let endpoint = adapter.map_or_else(
        || config.llm.endpoint.clone(),
        |adapter| crate::agent::inference::session_endpoint(adapter, config),
    )?;
    let provider = humanitl_config::inference::provider_outside(&config.llm, &endpoint)?;
    Some(CloudInference {
        agent: config.agent.adapter.clone(),
        provider,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn config(adapter: &str, endpoint: Option<&str>) -> Config {
        let mut config = Config::default();
        config.agent.adapter = adapter.to_owned();
        config.llm.endpoint = endpoint.map(|text| url::Url::parse(text).unwrap());
        config
    }

    #[test]
    fn a_cloud_endpoint_names_agent_and_provider() {
        for adapter in ["opencode", "pi"] {
            let found = cloud_inference(&config(adapter, Some("https://api.openai.com/v1")))
                .expect("a cloud endpoint");
            assert_eq!(found.agent, adapter);
            assert_eq!(found.provider, "api.openai.com");
            assert_eq!(
                found.short_line(),
                "Cloud inference: data goes to api.openai.com"
            );
            assert!(found.notice().why.contains("api.openai.com"));
        }
    }

    #[test]
    fn a_local_endpoint_or_none_gives_no_notice() {
        assert_eq!(
            cloud_inference(&config("opencode", Some("http://192.168.1.20:11434"))),
            None
        );
        assert_eq!(
            cloud_inference(&config("pi", Some("http://localhost:11434/v1"))),
            None
        );
        assert_eq!(cloud_inference(&config("opencode", None)), None);
    }

    /// Ein Adapter mit eigenem Anbieter, wie ihn HUM-514 bringt, bekommt den
    /// Hinweis ohne `llm.endpoint`.
    #[test]
    fn an_adapter_with_its_own_provider_gets_the_notice() {
        struct OwnProvider(crate::agent::OpenCodeAdapter);
        impl AgentAdapter for OwnProvider {
            fn id(&self) -> &'static str {
                "claude"
            }
            fn command(&self, ctx: &crate::agent::AgentContext) -> Vec<std::ffi::OsString> {
                self.0.command(ctx)
            }
            fn env(&self, ctx: &crate::agent::AgentContext) -> Vec<(String, String)> {
                self.0.env(ctx)
            }
            fn files(
                &self,
                ctx: &crate::agent::AgentContext,
            ) -> Result<Vec<crate::agent::SandboxFile>, Diagnostic> {
                self.0.files(ctx)
            }
            fn default_rules(&self) -> &'static str {
                self.0.default_rules()
            }
            fn llm_passthrough(
                &self,
                llm: &humanitl_config::LlmConfig,
            ) -> Vec<humanitl_core::rule::Rule> {
                self.0.llm_passthrough(llm)
            }
            fn inference_endpoint(&self, llm: &humanitl_config::LlmConfig) -> Option<url::Url> {
                llm.endpoint
                    .clone()
                    .or_else(|| url::Url::parse("https://api.anthropic.com").ok())
            }
            fn preflight(&self, ctx: &crate::agent::AgentContext) -> Vec<Diagnostic> {
                self.0.preflight(ctx)
            }
            fn program(&self) -> &'static str {
                "claude"
            }
            fn install_command(&self) -> &'static str {
                self.0.install_command()
            }
            fn docs_url(&self) -> &'static str {
                self.0.docs_url()
            }
            fn is_fullscreen_tui(&self) -> bool {
                true
            }
        }
        let adapter = OwnProvider(crate::agent::OpenCodeAdapter::new());
        let found = cloud_inference_with(Some(&adapter), &config("claude", None))
            .expect("its own provider");
        assert_eq!(found.provider, "api.anthropic.com");
        // Mit einem lokalen Gateway in `llm.endpoint` bleibt die Inferenz hier.
        assert_eq!(
            cloud_inference_with(
                Some(&adapter),
                &config("claude", Some("http://127.0.0.1:4000"))
            ),
            None
        );
    }
}
