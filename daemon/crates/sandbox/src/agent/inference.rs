//! Wohin ein Agent seine Inferenz schickt (GitHub #112).
//!
//! `agents.<id>.inference` hat zwei Werte. `model-server`, die Vorgabe,
//! schickt die Inferenz nur an den Modellserver, den Humanitl einstellt
//! (`llm.endpoint`). `own` lässt den Agenten seine eigenen Provider nutzen,
//! soweit sein Adapter sie kennt.
//!
//! Dieses Modul ist die eine Stelle, an der Einstellung und Adapter
//! zusammenkommen. Der Adapter erklärt nur zwei Dinge:
//!
//! - [`AgentAdapter::uses_model_server`]: ob sein Agent ein Modell nur über
//!   `llm.endpoint` erreicht. Dann startet er mit `model-server` ohne
//!   Endpunkt nicht ([`finding`], `LLM_012`), statt mit einem Provider zu
//!   starten, der ins Leere zeigt.
//! - [`AgentAdapter::own_inference`]: die Endpunkte seiner eigenen
//!   Provider, Host und genaue Pfade je Methode ([`OwnInference`]). Daraus
//!   folgen die Durchreiche ([`session_passthrough`]), der Hinweis auf
//!   Cloud-Inferenz und seine Bestätigung ([`session_endpoint`], `LLM_010`,
//!   `LLM_011`) und das Angebot im Befund `LLM_012`. Ohne Erklärung lehnt
//!   [`finding`] `own` mit `LLM_013` ab; es gibt keinen stillen Rückfall auf
//!   den Modellserver und keine Durchreiche auf einen ganzen Host.
//!
//! Mit `llm.cloud = "block"` (GitHub #113) lässt [`session_passthrough`]
//! keine Regel zu einem Host außerhalb stehen; den Start lehnt das Tor des
//! Daemons ab (`LLM_014`, `humanitl_ipc::cloud::gate`).
//!
//! Was der Adapter in `own` weglässt (Provider, Modell, Katalog), entscheidet
//! er selbst an [`AgentContext::model_server`]; dieses Modul schreibt keine
//! Datei.

use std::net::IpAddr;

use humanitl_config::{AgentInference, Config, LlmConfig};
use humanitl_core::diagnostics::codes::{LLM_012, LLM_013};
use humanitl_core::rule::{HostPattern, Rule};
use humanitl_core::{Diagnostic, HostName, Scheme, Severity};
use url::Url;

use crate::agent::opencode::EXAMPLE_ENDPOINT;
use crate::agent::passthrough::{self, Route, Target};
use crate::agent::{AgentAdapter, AgentContext};

/// Die eigenen Provider eines Agenten, wie sein Adapter sie erklärt
/// ([`AgentAdapter::own_inference`]).
#[derive(Debug, Clone)]
pub struct OwnInference {
    /// Der Endpunkt, den der Hinweis auf Cloud-Inferenz nennt (`LLM_010`)
    /// und dessen Host die Bestätigung trägt (`LLM_011`).
    pub endpoint: Url,
    /// Host, Schema, Port und Notiz der Durchreiche. `allow_private` ist für
    /// den Anbieter eines Agenten immer falsch.
    pub target: Target,
    /// Je Satz von Methoden die genauen Pfade (HUM-568); höchstens so viele,
    /// wie [`passthrough::PASSTHROUGH_RULE_IDS`] Kennungen hat.
    pub routes: Vec<Route>,
}

/// Die wirksame Einstellung für den Adapter der Sitzung.
fn mode(adapter: &dyn AgentAdapter, config: &Config) -> AgentInference {
    config.agents.inference(adapter.id())
}

/// Die Durchreiche einer Sitzung mit dieser Konfiguration.
///
/// Mit `model-server` die des Modellservers
/// ([`AgentAdapter::llm_passthrough`]), mit `own` genau die erklärten Pfade
/// der eigenen Provider und sonst nichts, auch nicht `llm.endpoint`. Ein
/// Adapter ohne eigene Provider bekommt mit `own` keine Regel; sein Start
/// scheitert an `LLM_013`.
///
/// Mit `llm.cloud = "block"` (GitHub #113) bleibt jede Regel draußen, deren
/// Ziel nicht lokal ist ([`stays_local`]): die zweite Sperre neben der
/// Ablehnung des Starts (`LLM_014`), damit auch ein Weg am Tor vorbei keine
/// Durchreiche hinaus findet. Keine Regel heißt: jede Anfrage dorthin wird
/// gehalten.
#[must_use]
pub fn session_passthrough(adapter: &dyn AgentAdapter, config: &Config) -> Vec<Rule> {
    let mut rules = match mode(adapter, config) {
        AgentInference::ModelServer => adapter.llm_passthrough(&config.llm),
        AgentInference::Own => adapter
            .own_inference()
            .map(|own| passthrough::rules(&own.target, own.routes))
            .unwrap_or_default(),
    };
    if config.llm.cloud.is_block() {
        rules.retain(|rule| stays_local(&config.llm, rule));
    }
    rules
}

/// Ob das Ziel einer Regel lokal ist, mit derselben Lesart wie der Hinweis
/// auf Cloud-Inferenz ([`humanitl_config::inference::provider_outside`]):
/// Schema, Host und Port der Regel als Adresse; eine Regel ohne Schema muss
/// mit beiden lokal sein. Ein Muster statt eines Hosts (Glob, Netz) ist nie
/// lokal; eine Durchreiche nennt immer genau einen Host.
#[must_use]
pub fn stays_local(llm: &LlmConfig, rule: &Rule) -> bool {
    let host = match &rule.matcher.host {
        HostPattern::Exact(HostName::Ip(IpAddr::V6(ip))) | HostPattern::Ip(IpAddr::V6(ip)) => {
            format!("[{ip}]")
        }
        HostPattern::Exact(host) => host.to_string(),
        HostPattern::Ip(ip) => ip.to_string(),
        HostPattern::Glob(_) | HostPattern::Cidr { .. } => return false,
    };
    let port = rule
        .matcher
        .port
        .map_or_else(String::new, |port| format!(":{port}"));
    let schemes = rule
        .matcher
        .scheme
        .map_or_else(|| vec![Scheme::Http, Scheme::Https], |scheme| vec![scheme]);
    schemes.into_iter().all(|scheme| {
        Url::parse(&format!("{}://{host}{port}/", scheme.as_str()))
            .is_ok_and(|url| humanitl_config::inference::provider_outside(llm, &url).is_none())
    })
}

/// Der Endpunkt der Inferenz einer Sitzung mit dieser Konfiguration, aus
/// derselben Quelle wie [`session_passthrough`]: Hinweis auf
/// Cloud-Inferenz, Bestätigung und Ort des Schlüssels fragen nur hier.
#[must_use]
pub fn session_endpoint(adapter: &dyn AgentAdapter, config: &Config) -> Option<Url> {
    match mode(adapter, config) {
        AgentInference::ModelServer => adapter.inference_endpoint(&config.llm),
        AgentInference::Own => adapter.own_inference().map(|own| own.endpoint),
    }
}

/// Der Befund, wenn der Agent, den der Daemon selbst startet, keinen Ort
/// für seine Inferenz hat; sonst `None`.
///
/// - `LLM_012`: `model-server`, ein Adapter, der nur über `llm.endpoint` ein
///   Modell erreicht, und kein Endpunkt. Der Vorschlag ist ein Modellserver;
///   kennt der Adapter eigene Provider, nennt der Satz `own` als zweiten Weg.
/// - `LLM_013`: `own` für einen Adapter ohne eigene Provider.
///
/// Ein Kommando des Aufrufers, etwa eine Shell, ist nie betroffen: Es
/// startet ohne Provider von Humanitl, und wer darin den Agenten von Hand
/// startet, sieht dessen Anfragen in der Warteschlange.
#[must_use]
pub fn finding(adapter: &dyn AgentAdapter, ctx: &AgentContext) -> Option<Diagnostic> {
    if ctx.agent_command_from_caller {
        return None;
    }
    let id = adapter.id();
    let program = adapter.program();
    let key = format!("agents.{id}.inference");
    match ctx.inference {
        AgentInference::Own => {
            if adapter.own_inference().is_some() {
                return None;
            }
            Some(
                Diagnostic::builder(LLM_013, Severity::Blocking)
                    .why(format!(
                        "{key} is own, and Humanitl knows no own providers for {program} yet: \
                         it cannot name the hosts and paths a passthrough would need, so every \
                         request to the model would be held. Nothing started. Set {key} to \
                         model-server{}.",
                        if adapter.uses_model_server() {
                            " and choose a model server"
                        } else {
                            "; for this agent that already means its own vendor"
                        }
                    ))
                    .maybe_fix(humanitl_config::value::change_setting(
                        &key,
                        AgentInference::ModelServer.as_str(),
                    ))
                    .build(),
            )
        }
        AgentInference::ModelServer => {
            if !adapter.uses_model_server() || ctx.llm.endpoint.is_some() {
                return None;
            }
            let own = adapter.own_inference().map_or_else(String::new, |own| {
                format!(
                    " Or let {program} use its own providers: set {key} to own \
                     (`humanitl config set {key} own`); its inference then goes to {}, after \
                     the cloud inference notice.",
                    own.target.host
                )
            });
            Some(
                Diagnostic::builder(LLM_012, Severity::Blocking)
                    .why(format!(
                        "{program} reaches a model only through the model server Humanitl sets, \
                         and none is set: llm.endpoint is empty and no saved server is chosen. \
                         It would start and fail at every prompt, so nothing started. Set a \
                         model server: `humanitl llm server add NAME URL`, or llm.endpoint in \
                         the settings.{own}"
                    ))
                    .maybe_fix(humanitl_config::value::change_setting(
                        "llm.endpoint",
                        EXAMPLE_ENDPOINT,
                    ))
                    .build(),
            )
        }
    }
}

/// Die ganze Vorprüfung vor einem Start: die des Adapters
/// ([`AgentAdapter::preflight`]) und dazu [`finding`].
#[must_use]
pub fn preflight(adapter: &dyn AgentAdapter, ctx: &AgentContext) -> Vec<Diagnostic> {
    let mut diagnostics = adapter.preflight(ctx);
    diagnostics.extend(finding(adapter, ctx));
    diagnostics
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::OsString;
    use std::path::PathBuf;

    use humanitl_config::{AgentInference, Config, LlmConfig};
    use humanitl_core::rule::{HostPattern, Rule};
    use humanitl_core::{Diagnostic, HostName, Method, Scheme, SessionId};
    use url::Url;

    use super::{OwnInference, finding, session_endpoint, session_passthrough};
    use crate::agent::passthrough::{Route, Target};
    use crate::agent::{AgentAdapter, AgentContext, SandboxFile};

    /// Ein Adapter, der nur erklärt, was dieses Modul fragt.
    struct Probe {
        model_server: bool,
        own: bool,
    }

    impl AgentAdapter for Probe {
        fn id(&self) -> &'static str {
            "opencode"
        }
        fn command(&self, _ctx: &AgentContext) -> Vec<OsString> {
            vec![OsString::from("probe")]
        }
        fn env(&self, _ctx: &AgentContext) -> Vec<(String, String)> {
            Vec::new()
        }
        fn files(&self, _ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
            Ok(Vec::new())
        }
        fn default_rules(&self) -> &'static str {
            ""
        }
        fn llm_passthrough(&self, llm: &LlmConfig) -> Vec<Rule> {
            crate::agent::opencode::passthrough_rules(llm)
        }
        fn uses_model_server(&self) -> bool {
            self.model_server
        }
        fn own_inference(&self) -> Option<OwnInference> {
            self.own.then(|| OwnInference {
                endpoint: Url::parse("https://provider.test/v1").unwrap(),
                target: Target {
                    host: HostName::parse("provider.test").unwrap(),
                    scheme: Scheme::Https,
                    port: 443,
                    allow_private: false,
                    note: "probe".to_owned(),
                },
                routes: vec![Route::new(
                    &[Method::POST],
                    vec![humanitl_core::exact_path_entry("/v1/chat")],
                )],
            })
        }
        fn preflight(&self, _ctx: &AgentContext) -> Vec<Diagnostic> {
            Vec::new()
        }
        fn program(&self) -> &'static str {
            "probe"
        }
        fn install_command(&self) -> &'static str {
            "true"
        }
        fn docs_url(&self) -> &'static str {
            "https://example.test/"
        }
        fn is_fullscreen_tui(&self) -> bool {
            false
        }
    }

    fn ctx(endpoint: Option<&str>, mode: AgentInference) -> AgentContext {
        let llm = LlmConfig {
            endpoint: endpoint.map(|text| Url::parse(text).unwrap()),
            ..LlmConfig::default()
        };
        AgentContext::new(SessionId::nil(), PathBuf::from("/p"), llm).with_inference(mode)
    }

    fn config(endpoint: Option<&str>, mode: AgentInference) -> Config {
        let mut config = Config::default();
        config.llm.endpoint = endpoint.map(|text| Url::parse(text).unwrap());
        config.agents.opencode.inference = mode;
        config
    }

    #[test]
    fn a_model_server_adapter_without_an_endpoint_does_not_start() {
        let probe = Probe {
            model_server: true,
            own: false,
        };
        let found = finding(&probe, &ctx(None, AgentInference::ModelServer)).expect("LLM_012");
        assert_eq!(found.code.as_str(), "LLM_012");
        assert!(
            !found.why.contains("inference to own") && !found.why.contains(".inference"),
            "no own mode offered: {}",
            found.why
        );
        // Mit Endpunkt, mit einem Kommando des Aufrufers oder für einen
        // Adapter mit eigenem Anbieter: kein Befund.
        assert_eq!(
            finding(
                &probe,
                &ctx(Some("http://10.0.0.2:8080"), AgentInference::ModelServer)
            ),
            None
        );
        assert_eq!(
            finding(
                &probe,
                &ctx(None, AgentInference::ModelServer)
                    .with_command_from_caller(vec![OsString::from("bash")])
            ),
            None
        );
        let vendor = Probe {
            model_server: false,
            own: false,
        };
        assert_eq!(
            finding(&vendor, &ctx(None, AgentInference::ModelServer)),
            None
        );
    }

    #[test]
    fn the_own_mode_is_offered_only_where_the_adapter_declares_it() {
        let probe = Probe {
            model_server: true,
            own: true,
        };
        let found = finding(&probe, &ctx(None, AgentInference::ModelServer)).expect("LLM_012");
        assert!(
            found.why.contains("agents.opencode.inference") && found.why.contains("provider.test"),
            "{}",
            found.why
        );
        assert_eq!(finding(&probe, &ctx(None, AgentInference::Own)), None);
    }

    #[test]
    fn own_without_a_declaration_is_refused_and_lets_nothing_through() {
        let probe = Probe {
            model_server: true,
            own: false,
        };
        let found = finding(
            &probe,
            &ctx(Some("http://10.0.0.2:8080"), AgentInference::Own),
        )
        .expect("LLM_013");
        assert_eq!(found.code.as_str(), "LLM_013");
        let config = config(Some("http://10.0.0.2:8080"), AgentInference::Own);
        assert!(session_passthrough(&probe, &config).is_empty());
        assert_eq!(session_endpoint(&probe, &config), None);
    }

    #[test]
    fn own_replaces_the_model_server_in_passthrough_and_notice() {
        let probe = Probe {
            model_server: true,
            own: true,
        };
        let own = config(Some("http://10.0.0.2:8080"), AgentInference::Own);
        let rules = session_passthrough(&probe, &own);
        let [rule] = rules.as_slice() else {
            panic!("one route, one rule: {rules:?}");
        };
        assert_eq!(
            rule.matcher.host,
            HostPattern::Exact(HostName::parse("provider.test").unwrap())
        );
        assert!(!rule.allow_private);
        assert_eq!(
            session_endpoint(&probe, &own).unwrap().host_str(),
            Some("provider.test")
        );
        let server = config(Some("http://10.0.0.2:8080"), AgentInference::ModelServer);
        assert_eq!(
            session_endpoint(&probe, &server).unwrap().host_str(),
            Some("10.0.0.2")
        );
        let passthrough = session_passthrough(&probe, &server);
        assert!(!passthrough.is_empty());
        assert!(
            passthrough.iter().all(|rule| rule.allow_private),
            "the model server's own passthrough"
        );
    }

    /// Die zweite Sperre (GitHub #113): Mit `llm.cloud = "block"` enthält
    /// die Durchreiche keine Regel zu einem Host außerhalb, weder zu einem
    /// Modellserver in der Cloud noch zu den eigenen Providern; ein lokaler
    /// Modellserver behält seine Durchreiche unverändert.
    ///
    /// Mutationsbeweis: Ohne `retain` in [`session_passthrough`] stünden
    /// die Regeln zu `api.openai.com` und `provider.test` da, und der Test
    /// wird rot.
    #[test]
    fn the_cloud_lock_leaves_no_passthrough_outside() {
        let probe = Probe {
            model_server: true,
            own: true,
        };
        let locked = |endpoint: &str, mode| {
            let mut config = config(Some(endpoint), mode);
            config.llm.cloud = humanitl_config::LlmCloud::Block;
            config
        };
        let cloud = config(
            Some("https://api.openai.com/v1"),
            AgentInference::ModelServer,
        );
        assert!(!session_passthrough(&probe, &cloud).is_empty(), "allow");
        assert_eq!(
            session_passthrough(
                &probe,
                &locked("https://api.openai.com/v1", AgentInference::ModelServer)
            ),
            Vec::<Rule>::new()
        );
        assert_eq!(
            session_passthrough(&probe, &locked("http://10.0.0.2:8080", AgentInference::Own)),
            Vec::<Rule>::new()
        );
        let local = config(Some("http://10.0.0.2:8080"), AgentInference::ModelServer);
        let kept = session_passthrough(
            &probe,
            &locked("http://10.0.0.2:8080", AgentInference::ModelServer),
        );
        assert!(!kept.is_empty());
        assert_eq!(kept, session_passthrough(&probe, &local), "unchanged");
        for loopback in [
            "http://127.0.0.1:11434",
            "http://[::1]:11434",
            "http://ollama.lan:11434",
        ] {
            let rules = session_passthrough(&probe, &locked(loopback, AgentInference::ModelServer));
            assert!(!rules.is_empty(), "{loopback} is local");
        }
    }
}
