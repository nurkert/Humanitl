//! `humanitl doctor`: eine Zeile je Vorbedingung dieser Maschine (HUM-075).
//!
//! Ein dünner Client (ADR-018). Die elf Prüfungen stehen in
//! [`humanitl_sandbox::doctor`], nicht hier; dieses Modul verbindet, ruft,
//! ersetzt zwei Zeilen um das, was nur ein Client wissen kann, und formatiert.
//!
//! # Warum es zwei Quellen für denselben Bericht gibt
//!
//! Der Doctor beantwortet vor allem die Frage „warum läuft das hier nicht?",
//! und die stellt jemand, dessen Daemon gerade **nicht** läuft. Ein Kommando,
//! das dafür einen laufenden Daemon bräuchte, wäre in genau dem Fall nutzlos,
//! für den es gebaut ist. Deshalb:
//!
//! - Ist der Daemon da, kommt der Bericht über `Doctor()` von ihm. Das ist der
//!   Regelfall und derselbe Aufruf, den der Setup-Bildschirm macht.
//! - Ist er nicht da, laufen dieselben Prüfungen aus derselben Crate hier im
//!   Prozess.
//!
//! Es gibt dabei keine zweite Umsetzung — nur eine zweite Stelle, an der
//! dieselbe läuft. Welche es war, steht in der Ausgabe (`source`), denn die
//! beiden Prozesse haben verschiedene Umgebungen: `PATH` und
//! `$XDG_RUNTIME_DIR` des Daemons sind die seiner Unit, nicht die dieses
//! Terminals. Ein Bericht, der das verschwiege, behauptete über die falsche
//! Umgebung etwas.
//!
//! # Zwei Zeilen kommen von hier
//!
//! `daemon` und `llm` sind die einzigen Prüfungen, deren Tatsachen nicht aus
//! der Maschine kommen:
//!
//! - **`daemon`**: Ob ein Client den Daemon erreicht, weiß nur der Client. Der
//!   Daemon selbst schickt die Zeile als „nicht gemessen"; dieses Modul
//!   ersetzt sie durch das, was der Verbindungsversuch ergeben hat.
//! - **`llm`**: Dahinter steht eine Verbindung ins Netz. Sie wird nur mit
//!   `--probe-llm` aufgebaut, nie als Nebenwirkung; vorher steht der Endpunkt
//!   in einer Zeile auf `stderr`, damit ein Mensch **vor** dem
//!   Verbindungsaufbau liest, wohin er geht. Diese Zeile lässt sich weder mit
//!   `--json` noch mit `-q` abstellen ([`announce`]). Ohne den Schalter trägt
//!   die Zeile `DOCTOR_013` und den Befehl, der sie misst.
//!
//! Geurteilt wird über beide trotzdem in der Crate
//! ([`humanitl_sandbox::doctor::daemon_line`],
//! [`humanitl_sandbox::doctor::llm_line`]); hier stehen nur die Tatsachen.

use std::time::Duration;

use humanitl_core::{Diagnostic, FixAction};
use humanitl_ipc::client::Client;
use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, convert, v1};
use humanitl_sandbox::doctor::{
    self, CheckId, CheckOutcome, CheckStatus, DaemonFacts, LlmFacts, PROBE_LLM_COMMAND, Probe,
};
use serde_json::json;
use tokio::time::timeout;

use crate::cli::DoctorArgs;
use crate::cmd::{Context, EXIT_CHECK, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::render::{diagnostic_block, diagnostic_json, table};

/// Wie lange dieser Befehl auf den Verbindungsaufbau und auf `GetInfo`
/// wartet.
///
/// Ohne Frist haengt `humanitl doctor` fuer immer, sobald irgendein Prozess
/// den erwarteten Socket bindet, die Verbindung annimmt und nie antwortet —
/// und erreicht dann **nie** den Rueckfall auf den eigenen Prozess. Damit
/// waere genau die Eigenschaft hin, die den Rueckfall begruendet: dass der
/// Doctor auch dann arbeitet, wenn der Daemon nicht kann.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Wie lange dieser Befehl auf `Doctor()` wartet.
///
/// Der Daemon fuehrt darin vier Aufrufe mit je zwei Sekunden Frist aus
/// ([`humanitl_sandbox::doctor::DEFAULT_TIMEOUT`]); zehn Sekunden lassen ihm
/// Luft und begrenzen ihn trotzdem.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// Wie lange dieser Befehl auf `ProbeLlm` wartet.
///
/// Die Probe klemmt ihre eigene Frist auf `MAX_TIMEOUT_MS` (30 s); diese
/// Frist liegt darueber, damit sie erst greift, wenn der Daemon selbst nicht
/// mehr antwortet, und nicht schon bei einem langsamen Endpunkt.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(35);

/// Ein Befund fuer einen Weg zum Daemon, der in seine Frist gelaufen ist.
///
/// `DAEMON_001` wie bei jedem anderen „nicht erreichbar": Ein Daemon, der die
/// Verbindung annimmt und schweigt, ist fuer diesen Befehl dasselbe wie
/// keiner.
fn timed_out(what: &str, after: Duration) -> Diagnostic {
    Diagnostic::builder(
        humanitl_core::diagnostics::codes::DAEMON_001,
        humanitl_core::Severity::Blocking,
    )
    .why(format!(
        "{what} did not answer within {} ms; something holds the socket without speaking the \
         contract",
        after.as_millis()
    ))
    .fix(FixAction::CopyCommand("humanitld".to_owned()))
    .build()
}

/// Woher der Bericht kam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Vom laufenden Daemon, über `Doctor()`.
    Daemon,
    /// Aus diesem Prozess, weil kein Daemon antwortete.
    Local,
}

impl Source {
    /// Der Kurzname für die Ausgabe.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Local => "local",
        }
    }
}

/// Führt `humanitl doctor` aus.
///
/// # Errors
///
/// `CONFIG_001` bis `CONFIG_003`, wenn die Konfiguration nicht lesbar ist. Ein
/// fehlender Daemon ist **kein** Fehlschlag dieses Befehls: Er ist eine Zeile
/// des Berichts.
pub async fn run(ctx: &Context, args: &DoctorArgs) -> Result<u8, Failure> {
    let config = ctx.config()?.config;
    let socket = ctx.paths.daemon_socket();

    let (mut client, refused) = match timeout(CONNECT_TIMEOUT, ctx.connect()).await {
        Ok(Ok(client)) => (Some(client), None),
        Ok(Err(failure)) => (None, Some(failure.diagnostic)),
        Err(_elapsed) => (None, Some(timed_out("the daemon socket", CONNECT_TIMEOUT))),
    };
    let daemon = daemon_facts(&socket, client.as_mut(), refused).await;

    let (mut report, source) = match client.as_mut() {
        Some(client) => match timeout(REPORT_TIMEOUT, client.doctor(())).await {
            Ok(Ok(response)) => (response.into_inner(), Source::Daemon),
            Ok(Err(status)) => {
                ctx.render.note(&format!(
                    "the daemon refused Doctor ({}); running the checks in this process instead",
                    status.code().description()
                ));
                (local(ctx, &config).await, Source::Local)
            }
            Err(_elapsed) => {
                ctx.render.note(&format!(
                    "the daemon did not answer Doctor within {} ms; running the checks in this \
                     process instead",
                    REPORT_TIMEOUT.as_millis()
                ));
                (local(ctx, &config).await, Source::Local)
            }
        },
        None => (local(ctx, &config).await, Source::Local),
    };

    replace(&mut report, &doctor::daemon_line(&daemon));
    if args.probe_llm {
        let outcome = probe_llm(&config, client.as_mut()).await;
        replace(&mut report, &outcome);
    }

    let failed = has_failure(&report);
    print(ctx, &report, source);
    if failed { Ok(EXIT_CHECK) } else { Ok(EXIT_OK) }
}

/// Was der Verbindungsversuch über den Daemon ergeben hat.
///
/// Ohne Client ist er unerreichbar; mit Client entscheidet `GetInfo`, denn
/// erst dessen Antwort beweist, dass am anderen Ende ein Daemon steht und
/// nicht nur ein Socket.
async fn daemon_facts(
    socket: &std::path::Path,
    client: Option<&mut Client>,
    refused: Option<Diagnostic>,
) -> DaemonFacts {
    let Some(client) = client else {
        // Der Befund des Verbindungsversuchs, nicht ein neuer: Er weiß, ob das
        // Token fehlte, der Socket fehlte oder niemand antwortete.
        let diagnostic = refused.unwrap_or_else(|| {
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::DAEMON_001,
                humanitl_core::Severity::Blocking,
            )
            .why(format!("no daemon answered on {}", socket.display()))
            .build()
        });
        return DaemonFacts::Unreachable {
            socket: socket.to_path_buf(),
            diagnostic: Box::new(diagnostic),
        };
    };
    match timeout(CONNECT_TIMEOUT, client.get_info(())).await {
        Ok(Ok(response)) => {
            let info = response.into_inner();
            DaemonFacts::Reachable {
                socket: socket.to_path_buf(),
                version: info.daemon_version,
                proto: (info.proto_major, info.proto_minor),
                expected_proto: (PROTO_MAJOR, PROTO_MINOR),
            }
        }
        Ok(Err(status)) => DaemonFacts::Unreachable {
            socket: socket.to_path_buf(),
            diagnostic: Box::new(status_diagnostic(&status, "GetInfo")),
        },
        Err(_elapsed) => DaemonFacts::Unreachable {
            socket: socket.to_path_buf(),
            diagnostic: Box::new(timed_out("GetInfo", CONNECT_TIMEOUT)),
        },
    }
}

/// Die Prüfungen in diesem Prozess, weil kein Daemon antwortet.
async fn local(ctx: &Context, config: &humanitl_config::Config) -> v1::DoctorReport {
    let env = ctx.env.clone();
    let adapter = config.agent.adapter.clone();
    let command = config
        .agent
        .command
        .as_ref()
        .and_then(|words| words.first())
        .cloned();
    // Derselbe Endpunkt, den der Daemon in seine Zeile schreibt, und dieselbe
    // Aussage darüber: dass ihn niemand angesprochen hat.
    let llm = LlmFacts::not_contacted(
        config.llm.endpoint.as_ref().map(ToString::to_string),
        PROBE_LLM_COMMAND,
    );
    // Die Prüfungen lesen Dateien und starten kurze Programme. In einer
    // `async fn` gehört das in einen Blocking-Thread, auch in einem Prozess,
    // der sonst nichts tut: Sonst stünde die Laufzeit still, solange eine
    // Probe an ihrer Frist hängt.
    let facts = tokio::task::spawn_blocking(move || {
        Probe::new(&env)
            .with_agent(adapter, command)
            .collect(untried(), llm)
    })
    .await;
    let Ok(facts) = facts else {
        return v1::DoctorReport::default();
    };
    convert::doctor_report_to_proto(&doctor::run(&facts))
}

/// Die Tatsache, die beide Zeilen aus dem Client später ersetzen.
fn untried() -> DaemonFacts {
    DaemonFacts::NotTried {
        socket: std::path::PathBuf::new(),
        why: "the connection attempt is reported in its own line".to_owned(),
    }
}

/// Misst die Erreichbarkeit des Sprachmodells — auf ausdrücklichen Wunsch.
///
/// Der einzige Weg dieses Befehls ins Netz, und er geht über den Daemon:
/// `ProbeLlm` ist die Fähigkeit, die Kommandozeile ihr Client (ADR-018, und
/// `tools/check-deps.sh` lässt eine ausgehende Verbindung außerhalb des
/// `Egress`-Ports ohnehin nicht zu).
async fn probe_llm(config: &humanitl_config::Config, client: Option<&mut Client>) -> CheckOutcome {
    let Some(endpoint) = config.llm.endpoint.as_ref().map(ToString::to_string) else {
        return doctor::llm_line(&LlmFacts::NoEndpoint);
    };
    let Some(client) = client else {
        return CheckOutcome::unmeasured(
            CheckId::Llm,
            format!(
                "{endpoint} was not contacted: the endpoint probe lives in the daemon, and no \
                 daemon answered"
            ),
            &["humanitl", "doctor", "--probe-llm"],
        );
    };

    announce(&endpoint);

    let call = client.probe_llm(v1::ProbeLlmRequest {
        endpoint: endpoint.clone(),
        timeout_ms: 0,
    });
    let facts = match timeout(PROBE_TIMEOUT, call).await {
        Err(_elapsed) => {
            return CheckOutcome::unmeasured(
                CheckId::Llm,
                format!(
                    "the daemon did not answer ProbeLlm for {endpoint} within {} ms",
                    PROBE_TIMEOUT.as_millis()
                ),
                &["humanitl", "doctor", "--probe-llm"],
            );
        }
        Ok(Ok(response)) => {
            let response = response.into_inner();
            LlmFacts::Answered {
                endpoint,
                flavor: flavor_name(response.flavor).to_owned(),
                models: response.models.len(),
                latency_ms: response.latency_ms,
                diagnostics: response.diagnostics.iter().filter_map(from_proto).collect(),
            }
        }
        Ok(Err(status)) => LlmFacts::Silent {
            diagnostic: Box::new(status_diagnostic(&status, "ProbeLlm")),
            endpoint,
        },
    };
    doctor::llm_line(&facts)
}

/// Sagt auf `stderr`, wohin gleich eine Verbindung geht und was sie tut.
///
/// **Geht mit Absicht nicht durch [`crate::render::Renderer`].** Dessen `note`
/// schweigt unter `--json` und unter `-q`; die Ankündigung einer Verbindung
/// darf sich aber nicht durch einen Ausgabeschalter abstellen lassen. Sie ist
/// Teil der Handlung und nicht ihre Verzierung: Dieses Produkt handelt davon,
/// dass nichts unbemerkt hinausgeht (BACKLOG.md Prinzip 1). `stdout` bleibt
/// dabei unberührt, ein einziger JSON-Wert also weiterhin ein einziger.
fn announce(endpoint: &str) {
    eprintln!(
        "contacting {endpoint} now: two GET requests, /api/tags then /v1/models, \
         no credentials, no redirects"
    );
}

/// Der Kurzname der erkannten API, wie ihn die Wire-Form führt.
///
/// Ein Wert, den dieser Client nicht kennt, wird nicht geraten: `unknown` sagt
/// dasselbe wie `LLM_PRODUCT_UNKNOWN` und behauptet nichts über die API.
fn flavor_name(flavor: i32) -> &'static str {
    match v1::LlmProduct::try_from(flavor) {
        Ok(v1::LlmProduct::Ollama) => "ollama",
        Ok(v1::LlmProduct::OpenaiCompatible) => "openai_compatible",
        Ok(v1::LlmProduct::Unknown | v1::LlmProduct::Unspecified) | Err(_) => "unknown",
    }
}

/// Ersetzt die Zeile mit derselben Kennung; die Reihenfolge bleibt.
///
/// Kennt der Bericht die Kennung nicht — ein Daemon, der älter ist als dieser
/// Client —, wird die Zeile angehängt statt verschluckt.
fn replace(report: &mut v1::DoctorReport, outcome: &CheckOutcome) {
    let line = convert::doctor_check_to_proto(outcome);
    match report
        .checks
        .iter_mut()
        .find(|check| check.id == outcome.id().as_str())
    {
        Some(slot) => *slot = line,
        None => report.checks.push(line),
    }
}

/// Wahr, wenn eine Zeile `fail` trägt.
fn has_failure(report: &v1::DoctorReport) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.status == v1::CheckStatus::Fail as i32)
}

/// Der Kurzname eines Status aus der Wire-Form.
///
/// `CHECK_STATUS_UNSPECIFIED` kann nur von einem Daemon kommen, der einen Wert
/// schickt, den dieser Client nicht kennt. Er wird nicht geraten: `unknown`
/// ist die ehrliche Antwort, und die Zeile bleibt sichtbar.
fn status_name(status: i32) -> &'static str {
    match v1::CheckStatus::try_from(status) {
        Ok(v1::CheckStatus::Ok) => CheckStatus::Ok.as_str(),
        Ok(v1::CheckStatus::Warn) => CheckStatus::Warn.as_str(),
        Ok(v1::CheckStatus::Fail) => CheckStatus::Fail.as_str(),
        Ok(v1::CheckStatus::Unspecified) | Err(_) => "unknown",
    }
}

/// Die Marke am Zeilenanfang, in derselben Breite wie `sandbox check`.
fn mark(status: i32) -> String {
    match status_name(status) {
        "ok" => "[ok]".to_owned(),
        "warn" => "[warn]".to_owned(),
        "fail" => "[FAIL]".to_owned(),
        other => format!("[{other}]"),
    }
}

/// Schreibt den Bericht: JSON oder Tabelle samt den Blöcken der Befunde.
fn print(ctx: &Context, report: &v1::DoctorReport, source: Source) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "source": source.as_str(),
            "status": worst(report),
            "checks": report
                .checks
                .iter()
                .map(|check| {
                    let mut value = json!({
                        "id": check.id,
                        "status": status_name(check.status),
                        "evidence": check.evidence,
                    });
                    if let (Some(object), Some(diagnostic)) =
                        (value.as_object_mut(), rebuilt(check))
                    {
                        object.insert("diagnostic".to_owned(), diagnostic_json(&diagnostic));
                    }
                    value
                })
                .collect::<Vec<serde_json::Value>>(),
        }));
        return;
    }

    let rows: Vec<Vec<String>> = report
        .checks
        .iter()
        .map(|check| {
            vec![
                mark(check.status),
                check.id.clone(),
                crate::render::one_line(&check.evidence),
            ]
        })
        .collect();
    print!("{}", table(&["", "CHECK", "EVIDENCE"], &rows));

    // Jede nicht-grüne Zeile bekommt ihren Block, nicht nur die erste: Der
    // Doctor ist die Liste dessen, was zu tun ist, und eine Liste, die nach
    // dem ersten Eintrag abbricht, schickt einen Menschen elfmal hierher
    // zurück.
    for check in &report.checks {
        if check.status == v1::CheckStatus::Ok as i32 {
            continue;
        }
        if let Some(diagnostic) = rebuilt(check) {
            println!();
            print!("{}", diagnostic_block(&diagnostic));
        }
    }
}

/// Der schlimmste Status im Bericht, als Kurzname.
fn worst(report: &v1::DoctorReport) -> &'static str {
    let mut worst = CheckStatus::Ok;
    for check in &report.checks {
        let status = match status_name(check.status) {
            "warn" => CheckStatus::Warn,
            "fail" => CheckStatus::Fail,
            _ => CheckStatus::Ok,
        };
        worst = worst.max(status);
    }
    worst.as_str()
}

/// Der Befund einer Zeile, aus der Wire-Form zurückgebaut.
fn rebuilt(check: &v1::DoctorCheck) -> Option<Diagnostic> {
    check.diagnostic.as_ref().and_then(from_proto)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::{convert, v1};
    use humanitl_sandbox::doctor::{CheckId, CheckOutcome, DaemonFacts};

    use super::{Source, has_failure, mark, replace, status_name, worst};

    fn report() -> v1::DoctorReport {
        v1::DoctorReport {
            checks: CheckId::ALL
                .into_iter()
                .map(|id| {
                    convert::doctor_check_to_proto(&CheckOutcome::unmeasured(
                        id,
                        "a test",
                        &["true"],
                    ))
                })
                .collect(),
        }
    }

    #[test]
    fn the_daemon_line_of_the_report_is_replaced_in_place() {
        let mut report = report();
        let before = report.checks.len();
        let position = report
            .checks
            .iter()
            .position(|check| check.id == "daemon")
            .expect("the daemon line");

        replace(
            &mut report,
            &super::doctor::daemon_line(&DaemonFacts::Reachable {
                socket: std::path::PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
                version: "0.0.0".to_owned(),
                proto: (1, 4),
                expected_proto: (1, 4),
            }),
        );

        assert_eq!(report.checks.len(), before, "no line was added");
        let line = &report.checks[position];
        assert_eq!(line.id, "daemon", "the order is the display order");
        assert_eq!(line.status, v1::CheckStatus::Ok as i32);
        assert!(line.diagnostic.is_none());
    }

    #[test]
    fn a_line_this_client_knows_and_the_daemon_does_not_is_appended() {
        let mut report = v1::DoctorReport::default();
        replace(
            &mut report,
            &CheckOutcome::ok(CheckId::Daemon, "an older daemon knew no such line"),
        );
        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].id, "daemon");
    }

    #[test]
    fn a_report_of_warnings_alone_does_not_fail() {
        let report = report();
        assert!(!has_failure(&report));
        assert_eq!(worst(&report), "warn");
    }

    #[test]
    fn one_failing_line_decides_the_exit_code() {
        let mut report = report();
        report.checks[0].status = v1::CheckStatus::Fail as i32;
        assert!(has_failure(&report));
        assert_eq!(worst(&report), "fail");
    }

    #[test]
    fn a_status_this_client_does_not_know_is_not_guessed_to_be_ok() {
        assert_eq!(status_name(v1::CheckStatus::Unspecified as i32), "unknown");
        assert_eq!(status_name(99), "unknown");
        assert_eq!(mark(99), "[unknown]");
        assert_eq!(mark(v1::CheckStatus::Fail as i32), "[FAIL]");
    }

    #[test]
    fn the_source_of_the_report_is_named() {
        assert_eq!(Source::Daemon.as_str(), "daemon");
        assert_eq!(Source::Local.as_str(), "local");
    }
}
