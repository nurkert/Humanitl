//! Der Hinweis auf Cloud-Inferenz über RPC (HUM-516).
//!
//! Ob ein Agent seine Inferenz an einen Anbieter außerhalb dieses Rechners
//! schickt, beantwortet [`humanitl_sandbox::agent::cloud::cloud_inference`];
//! ob der Mensch den Hinweis für diese Kombination aus Agent und Anbieter
//! bestätigt hat, steht im Datenverzeichnis des Daemons
//! ([`humanitl_config::inference`]). Hier wird beides zu den Nachrichten des
//! Vertrags: `SandboxEvent.Status.cloud_inference`, die Antwort auf
//! `SandboxRequest.acknowledge_cloud_inference` und die Zeile `inference` von
//! `humanitl doctor`. Oberfläche und Kommandozeile entscheiden nichts davon
//! selbst (ADR-018).

use humanitl_config::{Config, Paths};
use humanitl_core::diagnostics::codes::LLM_011;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::agent::cloud::{CloudInference, cloud_inference};

use crate::{convert, v1};

/// Die Kennung der Zeile in `humanitl doctor`.
pub const DOCTOR_ID: &str = "inference";

/// `Status.cloud_inference` für diese Konfiguration, mit dem Stand der
/// Bestätigung; `None`, wenn die Inferenz lokal bleibt.
#[must_use]
pub fn status(paths: &Paths, config: &Config) -> Option<v1::CloudInference> {
    cloud_inference(config).map(|found| to_proto(paths, &found))
}

/// Die Nachricht zu einem gefundenen Anbieter.
#[must_use]
pub fn to_proto(paths: &Paths, found: &CloudInference) -> v1::CloudInference {
    v1::CloudInference {
        agent: found.agent.clone(),
        provider: found.provider.clone(),
        acknowledged: humanitl_config::inference::is_acknowledged(
            paths,
            &found.agent,
            &found.provider,
        ),
        notice: Some(convert::diagnostic_to_proto(&found.notice())),
    }
}

/// Führt `SandboxRequest.acknowledge_cloud_inference` aus: genau ein
/// Ereignis, die gespeicherte Bestätigung oder der Befund, warum nicht.
///
/// Blockiert, weil es eine Datei schreibt.
#[must_use]
pub fn acknowledge(
    paths: &Paths,
    request: &v1::sandbox_request::AcknowledgeCloudInference,
) -> v1::SandboxEvent {
    let event =
        humanitl_config::inference::check_provider(&request.provider).and_then(|provider| {
            humanitl_config::inference::acknowledge(paths, &request.agent, &provider).map(|()| {
                v1::sandbox_event::Event::CloudInferenceAcknowledged(
                    v1::sandbox_event::CloudInferenceAcknowledged {
                        agent: request.agent.clone(),
                        provider,
                    },
                )
            })
        });
    v1::SandboxEvent {
        event: Some(event.unwrap_or_else(|diagnostic| {
            v1::sandbox_event::Event::Diagnostic(convert::diagnostic_to_proto(&diagnostic))
        })),
    }
}

/// Lässt einen Start nur durch, wenn seine Inferenz lokal bleibt, die
/// Kombination aus Agent und Anbieter bestätigt ist, der Start genau sie
/// jetzt bestätigt (`Start.acknowledge_cloud_inference`, wird gemerkt) oder
/// der Client den Hinweis für genau sie ohne Frage gezeigt hat
/// (`Start.cloud_notice_shown`, wird nicht gemerkt). Sonst `LLM_011` mit dem
/// Anbieter, den der Start wirklich hätte (HUM-516).
///
/// Die eine Stelle, an der ein Client, der den Hinweis vergisst, sichtbar
/// scheitert statt still zu starten (ADR-018).
///
/// # Errors
///
/// `LLM_011` für einen unbestätigten Start; die Befunde von
/// [`humanitl_config::inference::acknowledge`], wenn die Bestätigung nicht
/// geschrieben wurde.
pub fn gate(
    paths: &Paths,
    config: &Config,
    start: &v1::sandbox_request::Start,
) -> Result<(), Diagnostic> {
    let Some(found) = cloud_inference(config) else {
        return Ok(());
    };
    if humanitl_config::inference::is_acknowledged(paths, &found.agent, &found.provider) {
        return Ok(());
    }
    if answers(start.acknowledge_cloud_inference.as_ref(), &found) {
        return humanitl_config::inference::acknowledge(paths, &found.agent, &found.provider);
    }
    if answers(start.cloud_notice_shown.as_ref(), &found) {
        return Ok(());
    }
    Err(unconfirmed(&found))
}

/// Ob die Antwort des Clients genau dem Agenten und Anbieter gilt, die die
/// Auflösung dieses Starts ergibt (HUM-516, Review). Eine Antwort auf einen
/// anderen Anbieter, etwa nach einem Wechsel von `llm.endpoint` zwischen
/// Frage und Start, bestätigt nichts.
fn answers(answer: Option<&v1::CloudNoticeFor>, found: &CloudInference) -> bool {
    answer.is_some_and(|answer| {
        answer.agent == found.agent
            && humanitl_config::inference::check_provider(&answer.provider)
                .is_ok_and(|provider| provider == found.provider)
    })
}

/// Der Befund eines Starts, dessen Cloud-Inferenz niemand bestätigt hat.
#[must_use]
pub fn unconfirmed(found: &CloudInference) -> Diagnostic {
    Diagnostic::builder(LLM_011, Severity::Error)
        .why(format!(
            "{} would send its inference to {}, outside this machine, and this combination \
             of agent and provider was never confirmed; nothing started. {}",
            found.agent,
            found.provider,
            found.notice().why
        ))
        .fix(FixAction::CopyCommand("humanitl run --yes".to_owned()))
        .build()
}

/// Die Zeile `inference` von `humanitl doctor` (HUM-516).
///
/// `OK` mit „local", solange die Inferenz auf diesem Rechner oder im eigenen
/// Netz bleibt oder es keinen Endpunkt gibt; `WARN` mit `LLM_010`, wenn sie
/// an einen Anbieter außerhalb geht. Eine Warnung hält keinen Start auf.
#[must_use]
pub fn doctor_line(config: &Config) -> v1::DoctorCheck {
    match cloud_inference(config) {
        None => v1::DoctorCheck {
            id: DOCTOR_ID.to_owned(),
            status: v1::CheckStatus::Ok.into(),
            evidence: format!(
                "{}: inference stays on this machine or in your network, or no endpoint is set",
                config.agent.adapter
            ),
            diagnostic: None,
        },
        Some(found) => v1::DoctorCheck {
            id: DOCTOR_ID.to_owned(),
            status: v1::CheckStatus::Warn.into(),
            evidence: format!("{}: {}", found.agent, found.short_line()),
            diagnostic: Some(convert::diagnostic_to_proto(&found.notice())),
        },
    }
}

/// Setzt die Zeile `inference` hinter die Zeile `egress`, sonst hinter
/// `llm`, sonst ans Ende.
pub fn insert_doctor_line(report: &mut v1::DoctorReport, config: &Config) {
    let line = doctor_line(config);
    let after = |id: &str| report.checks.iter().position(|check| check.id == id);
    let at = after("egress")
        .or_else(|| after("llm"))
        .map_or(report.checks.len(), |index| index + 1);
    report.checks.insert(at, line);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use humanitl_config::Env;

    use super::*;

    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    fn config(endpoint: &str) -> Config {
        let mut config = Config::default();
        config.llm.endpoint = Some(url::Url::parse(endpoint).unwrap());
        config
    }

    /// Erst unbestätigt, nach der Bestätigung bestätigt; ein anderer
    /// Anbieter fragt erneut, ein lokaler Endpunkt nie (HUM-516, Tests).
    #[test]
    fn the_notice_asks_once_per_provider_and_never_for_a_local_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let cloud = config("https://api.openai.com/v1");
        let first = status(&paths, &cloud).expect("a cloud endpoint has a notice");
        assert_eq!(first.provider, "api.openai.com");
        assert!(!first.acknowledged);
        assert_eq!(
            first.notice.as_ref().map(|notice| notice.code.as_str()),
            Some("LLM_010")
        );

        let answer = acknowledge(
            &paths,
            &v1::sandbox_request::AcknowledgeCloudInference {
                agent: first.agent.clone(),
                provider: first.provider.clone(),
            },
        );
        assert!(
            matches!(
                answer.event,
                Some(v1::sandbox_event::Event::CloudInferenceAcknowledged(ref stored))
                    if stored.provider == "api.openai.com" && stored.agent == "opencode"
            ),
            "{answer:?}"
        );
        assert!(status(&paths, &cloud).unwrap().acknowledged);

        let other = status(&paths, &config("https://api.mistral.ai/v1")).unwrap();
        assert!(!other.acknowledged, "another provider asks again");

        assert_eq!(status(&paths, &config("http://192.168.1.20:11434")), None);
        assert_eq!(status(&paths, &config("http://localhost:11434/v1")), None);
    }

    fn notice(provider: &str) -> v1::CloudNoticeFor {
        v1::CloudNoticeFor {
            agent: "opencode".to_owned(),
            provider: provider.to_owned(),
        }
    }

    fn start_with(acknowledge: bool, shown: bool) -> v1::sandbox_request::Start {
        v1::sandbox_request::Start {
            acknowledge_cloud_inference: acknowledge.then(|| notice("api.openai.com")),
            cloud_notice_shown: shown.then(|| notice("api.openai.com")),
            ..v1::sandbox_request::Start::default()
        }
    }

    /// Die Antwort gilt nur dem Gezeigten (HUM-516, Review): Die Frage nannte
    /// A, die Konfiguration wechselt zu B, und der Start mit der Bestätigung
    /// für A wird mit B abgelehnt; B ist danach nicht gemerkt.
    #[test]
    fn an_answer_for_another_provider_confirms_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let shown_a = config("https://api.openai.com/v1");
        let check = status(&paths, &shown_a).expect("A is cloud");
        let answer_for_a = v1::sandbox_request::Start {
            acknowledge_cloud_inference: Some(v1::CloudNoticeFor {
                agent: check.agent.clone(),
                provider: check.provider.clone(),
            }),
            cloud_notice_shown: Some(v1::CloudNoticeFor {
                agent: check.agent,
                provider: check.provider,
            }),
            ..v1::sandbox_request::Start::default()
        };
        let switched_to_b = config("https://api.anthropic.com");
        let refused = gate(&paths, &switched_to_b, &answer_for_a).unwrap_err();
        assert_eq!(refused.code, LLM_011);
        assert!(refused.why.contains("api.anthropic.com"), "{}", refused.why);
        assert!(!humanitl_config::inference::is_acknowledged(
            &paths,
            "opencode",
            "api.anthropic.com"
        ));
        assert!(!humanitl_config::inference::is_acknowledged(
            &paths,
            "opencode",
            "api.openai.com"
        ));
        // Ein anderer Agent mit demselben Anbieter ebenso nicht.
        let other_agent = v1::sandbox_request::Start {
            acknowledge_cloud_inference: Some(v1::CloudNoticeFor {
                agent: "pi".to_owned(),
                provider: "api.openai.com".to_owned(),
            }),
            ..v1::sandbox_request::Start::default()
        };
        assert_eq!(
            gate(&paths, &shown_a, &other_agent).unwrap_err().code,
            LLM_011
        );
    }

    /// Das Tor des Starts (HUM-516): Ein unbestätigter Cloud-Start ohne
    /// Antwort des Clients ist `LLM_011`; die Zeile ohne Terminal lässt ihn
    /// durch, ohne etwas zu merken; die Bestätigung merkt ihn; ein lokaler
    /// Endpunkt geht nie durch das Tor.
    #[test]
    fn the_gate_refuses_a_start_that_skipped_the_notice() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let cloud = config("https://api.openai.com/v1");

        let refused = gate(&paths, &cloud, &start_with(false, false)).unwrap_err();
        assert_eq!(refused.code, LLM_011);
        assert!(refused.why.contains("api.openai.com"), "{}", refused.why);
        assert!(refused.fix.is_some());

        gate(&paths, &cloud, &start_with(false, true)).expect("the notice was shown");
        assert!(
            !humanitl_config::inference::is_acknowledged(&paths, "opencode", "api.openai.com"),
            "shown is no confirmation"
        );
        assert_eq!(
            gate(&paths, &cloud, &start_with(false, false))
                .unwrap_err()
                .code,
            LLM_011,
            "the next start asks again"
        );

        gate(&paths, &cloud, &start_with(true, false)).expect("confirmed");
        assert!(humanitl_config::inference::is_acknowledged(
            &paths,
            "opencode",
            "api.openai.com"
        ));
        gate(&paths, &cloud, &start_with(false, false)).expect("confirmed once, silent after");

        gate(
            &paths,
            &config("http://192.168.1.20:11434"),
            &start_with(false, false),
        )
        .expect("a local endpoint never waits");
    }

    #[test]
    fn junk_is_a_diagnostic_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let answer = acknowledge(
            &paths,
            &v1::sandbox_request::AcknowledgeCloudInference {
                agent: "../../etc".to_owned(),
                provider: "api.openai.com".to_owned(),
            },
        );
        assert!(
            matches!(answer.event, Some(v1::sandbox_event::Event::Diagnostic(ref found)) if found.code == "CONFIG_003"),
            "{answer:?}"
        );
        assert!(!paths.cloud_inference_path().exists());
    }

    #[test]
    fn the_doctor_line_warns_for_cloud_and_stands_after_egress() {
        let line = doctor_line(&config("https://api.anthropic.com"));
        assert_eq!(line.status, v1::CheckStatus::Warn as i32);
        assert!(
            line.evidence.contains("api.anthropic.com"),
            "{}",
            line.evidence
        );
        let local = doctor_line(&config("http://127.0.0.1:11434"));
        assert_eq!(local.status, v1::CheckStatus::Ok as i32);
        assert!(local.diagnostic.is_none());

        let mut report = v1::DoctorReport {
            checks: ["llm", "egress", "tray"]
                .into_iter()
                .map(|id| v1::DoctorCheck {
                    id: id.to_owned(),
                    ..v1::DoctorCheck::default()
                })
                .collect(),
            backends: Vec::new(),
        };
        insert_doctor_line(&mut report, &Config::default());
        let ids: Vec<&str> = report
            .checks
            .iter()
            .map(|check| check.id.as_str())
            .collect();
        assert_eq!(ids, ["llm", "egress", "inference", "tray"]);
    }
}
