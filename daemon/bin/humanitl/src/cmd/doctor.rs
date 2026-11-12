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
//!
//! # Je Backend eine Gruppe (HUM-436)
//!
//! Unter den elf Zeilen steht je Sandbox-Backend eine Gruppe aus
//! [`humanitl_sandbox::doctor::backend_groups`], vom Daemon oder, ohne ihn,
//! aus diesem Prozess. `--probe` bittet um einen Probestart je zugelassenem
//! Backend; ohne den Schalter startet der Befehl nichts.

use std::time::Duration;

use humanitl_core::Diagnostic;
use humanitl_core::diagnostics::codes::LLM_002;
use humanitl_ipc::client::{self, Client};
use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, convert, v1};
use humanitl_sandbox::doctor::{
    self, CheckId, CheckOutcome, CheckStatus, DaemonFacts, LlmFacts, PROBE_LLM_COMMAND, Probe,
    ProbeMode,
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

/// Wie lange dieser Befehl auf den Verbindungsaufbau samt Weckruf wartet
/// (HUM-164).
///
/// Hinter `humanitld.socket` startet erst die Verbindung des Doctors den
/// Dienst, und der Client wartet bis zu [`client::WAKE_TIMEOUT`] auf dessen
/// Token. Mit [`CONNECT_TIMEOUT`] allein hieße der erste Start eines Dienstes
/// „hält den Socket, ohne zu antworten". Bleibt das Token aus, sagt der Befund
/// des Clients das selbst, bevor diese Frist abläuft.
pub const WAKE_CONNECT_TIMEOUT: Duration = CONNECT_TIMEOUT.saturating_add(client::WAKE_TIMEOUT);

/// Wie lange dieser Befehl auf `Doctor()` wartet.
///
/// Der Daemon fuehrt darin vier Aufrufe mit je zwei Sekunden Frist aus
/// ([`humanitl_sandbox::doctor::DEFAULT_TIMEOUT`]); zehn Sekunden lassen ihm
/// Luft und begrenzen ihn trotzdem.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// Wie lange dieser Befehl mit `--probe` auf `Doctor()` wartet (HUM-436).
///
/// Je Backend ein Start samt der Frist des Shim-Berichts und dem Ende von
/// `true`; unter einer Container-Engine dazu ihr erster Start. Zwei Minuten
/// begrenzen das, ohne einen langsamen ersten Start abzuschneiden.
pub const PROBE_REPORT_TIMEOUT: Duration = Duration::from_secs(120);

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
    .fix(humanitl_ipc::auth::no_daemon_fix_here())
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
    let resolved = ctx.config()?;
    // Was die Ladung beiseitegelegt hat, gehört in die Zeile `llm` (HUM-283).
    let load_diagnostics = resolved.diagnostics;
    let config = resolved.config;
    let socket = ctx.paths.daemon_socket();

    let (mut client, refused) = match timeout(WAKE_CONNECT_TIMEOUT, ctx.connect()).await {
        Ok(Ok(client)) => (Some(client), None),
        Ok(Err(failure)) => (None, Some(failure.diagnostic)),
        Err(_elapsed) => (
            None,
            Some(timed_out("the daemon socket", WAKE_CONNECT_TIMEOUT)),
        ),
    };
    let daemon = daemon_facts(&socket, client.as_mut(), refused).await;

    let wait = if args.probe {
        PROBE_REPORT_TIMEOUT
    } else {
        REPORT_TIMEOUT
    };
    let request = v1::DoctorRequest { probe: args.probe };
    let (mut report, source) = match client.as_mut() {
        Some(client) => match timeout(wait, client.doctor(request)).await {
            Ok(Ok(response)) => (response.into_inner(), Source::Daemon),
            Ok(Err(status)) => {
                ctx.render.note(&format!(
                    "the daemon refused Doctor ({}); running the checks in this process instead",
                    status.code().description()
                ));
                (
                    local(ctx, &config, &load_diagnostics, args.probe).await,
                    Source::Local,
                )
            }
            Err(_elapsed) => {
                ctx.render.note(&format!(
                    "the daemon did not answer Doctor within {} ms; running the checks in this \
                     process instead",
                    wait.as_millis()
                ));
                (
                    local(ctx, &config, &load_diagnostics, args.probe).await,
                    Source::Local,
                )
            }
        },
        None => (
            local(ctx, &config, &load_diagnostics, args.probe).await,
            Source::Local,
        ),
    };

    // Ein Daemon vor Minor 35 kennt weder `DoctorRequest.probe` noch
    // `DoctorReport.backends` (HUM-436): Er ließe `--probe` still fallen.
    // Die Gruppen kommen dann aus diesem Prozess, und eine Zeile sagt das.
    if source == Source::Daemon
        && let Some(minor) = older_than_backends(&daemon)
    {
        ctx.render.note(&format!(
            "the daemon speaks contract {PROTO_MAJOR}.{minor}, which has no backend lines \
             (1.{BACKENDS_MINOR}); the backend lines come from this process"
        ));
        report.backends = local_backends(ctx, &config, args.probe).await;
    }

    replace(&mut report, &doctor::daemon_line(&daemon));
    if args.probe_llm {
        let outcome = probe_llm(&config, &load_diagnostics, client.as_mut()).await;
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
        let mut diagnostic = refused.unwrap_or_else(|| {
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::DAEMON_001,
                humanitl_core::Severity::Blocking,
            )
            .why(format!("no daemon answered on {}", socket.display()))
            .build()
        });
        // Nur der Vorschlag geht, der Grund bleibt. Der Verbindungsversuch
        // schlägt `humanitld` vor, also den Daemon im Vordergrund dieses
        // Terminals; die Zeile des Doctors sagt, was auf Dauer hilft, und
        // fällt ohne eigenen Vorschlag auf `InstallService` zurück
        // (`humanitl_sandbox::doctor`, Prüfung 6) -- dieselbe Abhilfe, die die
        // Einrichtung der Anwendung für `DAEMON_001` anbietet. Das ist der
        // Zustand nach `daemon uninstall` (HUM-077, Akzeptanzkriterium 4).
        diagnostic.fix = None;
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
                // The fake daemon replays a recording and is no build of
                // its own: nothing to compare (HUM-277, as in the app).
                expected_version: if info.capabilities.iter().any(|name| name == "fake") {
                    String::new()
                } else {
                    env!("CARGO_PKG_VERSION").to_owned()
                },
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

/// Die Prüfungen in diesem Prozess, weil kein Daemon antwortet; mit `probe`
/// samt Probestart je zugelassenem Backend, mit dem Shim neben diesem
/// Programm oder aus der Installation.
async fn local(
    ctx: &Context,
    config: &humanitl_config::Config,
    load_diagnostics: &[Diagnostic],
    probe: bool,
) -> v1::DoctorReport {
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
    let llm = LlmFacts::from_config(
        config.llm.endpoint.as_ref().map(ToString::to_string),
        load_diagnostics,
        PROBE_LLM_COMMAND,
    );
    // Die Prüfungen lesen Dateien und starten kurze Programme. In einer
    // `async fn` gehört das in einen Blocking-Thread, auch in einem Prozess,
    // der sonst nichts tut: Sonst stünde die Laufzeit still, solange eine
    // Probe an ihrer Frist hängt.
    // Was die Sandbox vom Dateibaum sieht und welchen Suchpfad sie hat. Ohne
    // diese Angaben suchte der Doctor den Agenten nur im `PATH` des Hosts und
    // meldete `DOCTOR_007`, wo die Vorprüfung des Adapters schweigt, weil das
    // Kommando in der Sandbox erreichbar ist (HUM-139). Fehlt das Profil, ist
    // die Antwort dieselbe wie vorher: eine Aussage über den Host.
    let (view, sandbox_path) = sandbox_view(ctx, config);
    let report = tokio::task::spawn_blocking(move || {
        let facts = Probe::new(&env)
            .with_agent(adapter, command)
            .with_sandbox(view, sandbox_path)
            .collect(untried(), llm);
        convert::doctor_report_to_proto(&doctor::run(&facts))
    })
    .await;
    let mut report = report.unwrap_or_default();
    report.backends = local_backends(ctx, config, probe).await;
    report
}

/// Die Minor, ab der `Doctor` je Backend eine Gruppe schickt (HUM-436).
const BACKENDS_MINOR: u32 = 35;

/// Die Minor des erreichten Daemons, wenn sie älter ist als
/// [`BACKENDS_MINOR`]; sonst `None`.
const fn older_than_backends(daemon: &DaemonFacts) -> Option<u32> {
    match daemon {
        DaemonFacts::Reachable {
            proto: (major, minor),
            ..
        } if *major == PROTO_MAJOR && *minor < BACKENDS_MINOR => Some(*minor),
        _ => None,
    }
}

/// Die Gruppen je Backend aus diesem Prozess; mit `probe` samt Probestart,
/// mit dem Shim neben diesem Programm oder aus der Installation.
async fn local_backends(
    ctx: &Context,
    config: &humanitl_config::Config,
    probe: bool,
) -> Vec<v1::DoctorBackend> {
    let sandbox = config.sandbox.clone();
    let paths = ctx.paths.clone();
    let mode = if probe {
        ProbeMode::Start {
            shim: doctor::locate_shim(),
        }
    } else {
        ProbeMode::Cached
    };
    tokio::task::spawn_blocking(move || {
        convert::doctor_backends_to_proto(&doctor::backend_groups(&sandbox, &paths, &mode))
    })
    .await
    .unwrap_or_default()
}

/// Die Sicht der Sandbox auf den Dateibaum und ihr Suchpfad, aus dem Profil.
///
/// Ein leeres Ergebnis heißt „unbekannt": Lässt sich das Profil nicht lesen,
/// sagt der Doctor weiter nur, was auf dem Host liegt, statt etwas über eine
/// Sandbox zu behaupten, die er nicht kennt.
fn sandbox_view(
    ctx: &Context,
    config: &humanitl_config::Config,
) -> (humanitl_sandbox::SandboxView, Option<String>) {
    let policy = humanitl_sandbox::MountPolicy::from_paths(&ctx.paths);
    let Ok(path) = crate::cmd::sandbox::profile_path(ctx, &config.sandbox.profile) else {
        return (humanitl_sandbox::SandboxView::default(), None);
    };
    let Ok(profile) = humanitl_sandbox::SandboxProfile::load_validated(&path, &policy) else {
        return (humanitl_sandbox::SandboxView::default(), None);
    };
    // `sandbox.env` gewinnt über das `[env]` des Profils, genau wie beim
    // Start (`SandboxProfile::effective_env`), und die Sicht ist die des
    // Backends: Unter macOS gibt es keine Einhängungen (HUM-377).
    let path = humanitl_sandbox::agent::effective_sandbox_path(
        config
            .sandbox
            .env
            .get("PATH")
            .or_else(|| profile.env.get("PATH"))
            .map(String::as_str),
    );
    (
        humanitl_sandbox::SandboxView::of_platform(&profile, None),
        path,
    )
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
async fn probe_llm(
    config: &humanitl_config::Config,
    load_diagnostics: &[Diagnostic],
    client: Option<&mut Client>,
) -> CheckOutcome {
    let Some(endpoint) = config.llm.endpoint.as_ref().map(ToString::to_string) else {
        // Nicht gesetzt oder beiseitegelegt (HUM-283): Gefragt wird nichts.
        return doctor::llm_line(&LlmFacts::from_config(
            None,
            load_diagnostics,
            PROBE_LLM_COMMAND,
        ));
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
        ..v1::ProbeLlmRequest::default()
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
                answer: (!response.answer.is_empty()).then_some(response.answer),
            }
        }
        Ok(Err(status)) => {
            let diagnostic = status_diagnostic(&status, "ProbeLlm");
            // Ob ein Schlüssel hinterlegt ist, fragt der Doctor nur nach einer
            // Anmeldung; `LlmKey(Status)` sagt es, ohne ihn zu zeigen.
            let stored = if diagnostic.code == LLM_002 {
                key_stored(client).await
            } else {
                None
            };
            failure_facts(endpoint, diagnostic, stored)
        }
    };
    doctor::llm_line(&facts)
}

/// Die Tatsachen zu einer Probe, die mit einem Befund endete.
///
/// `LLM_002` heißt: Der Server hat geantwortet und eine Anmeldung verlangt
/// (HUM-283). Die Probe des Doctors schickt nie einen Schlüssel, also ist das
/// kein „hat nicht geantwortet", sondern [`LlmFacts::Refused`] mit dem
/// Status und dem Wissen, ob ein Schlüssel hinterlegt ist. Jeder andere
/// Befund bleibt [`LlmFacts::Silent`].
fn failure_facts(endpoint: String, diagnostic: Diagnostic, key_stored: Option<bool>) -> LlmFacts {
    if diagnostic.code == LLM_002 {
        LlmFacts::Refused {
            status: answered_status(&diagnostic.why),
            key_stored,
            diagnostic: Box::new(diagnostic),
            endpoint,
        }
    } else {
        LlmFacts::Silent {
            diagnostic: Box::new(diagnostic),
            endpoint,
        }
    }
}

/// Ob ein Zugangsschlüssel hinterlegt ist, nach dem Daemon (`LlmKey`,
/// Operation `status`); `None`, wenn er es nicht in der Frist sagt.
///
/// Die Antwort trägt nie den Schlüssel, nur `set` und den Verweis.
async fn key_stored(client: &mut Client) -> Option<bool> {
    let call = client.llm_key(v1::LlmKeyRequest {
        op: Some(v1::llm_key_request::Op::Status(
            v1::llm_key_request::Status {},
        )),
        server: String::new(),
    });
    match timeout(PROBE_TIMEOUT, call).await {
        Ok(Ok(status)) => Some(status.into_inner().set),
        Ok(Err(_)) | Err(_) => None,
    }
}

/// Der Status `401` oder `403` aus dem Satz von `LLM_002` („answered 401
/// at …"); `None`, wenn der Satz ihn nicht in dieser Form nennt.
fn answered_status(why: &str) -> Option<u16> {
    let (_, rest) = why.split_once("answered ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    match digits.parse::<u16>() {
        Ok(status @ (401 | 403)) => Some(status),
        _ => None,
    }
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
    // Die Pfade kommen aus derselben Regel wie die Probe (HUM-261): hinter
    // einem Pfad-Präfix ist es eine Anfrage, nie eine an `/api/tags`.
    eprintln!(
        "contacting {endpoint} now: {}, no credentials, no redirects",
        humanitl_config::llm_endpoint::probe_plan(endpoint)
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

/// Die Zeilen, die über den Bericht entscheiden: die der Maschine und die
/// des Backends, das `sandbox.backend` wählt (HUM-436).
///
/// Die Gruppen der anderen Backends stehen in Tabelle und JSON, zählen aber
/// weder für Status und Exit-Code noch bekommen sie einen Befundblock: Auf
/// einem gesunden Rechner hieße sonst jede Stufe, die noch nicht zugelassen
/// ist, „warn“, obwohl kein Start an ihr hängt.
fn counted_checks(report: &v1::DoctorReport) -> impl Iterator<Item = &v1::DoctorCheck> {
    report.checks.iter().chain(
        report
            .backends
            .iter()
            .filter(|group| group.selected)
            .flat_map(|group| group.checks.iter()),
    )
}

/// Jede Zeile, die die Ausgabe zeigt: die der Maschine und die jedes
/// Backends.
fn shown_checks(report: &v1::DoctorReport) -> impl Iterator<Item = &v1::DoctorCheck> {
    report
        .checks
        .iter()
        .chain(report.backends.iter().flat_map(|group| group.checks.iter()))
}

/// Wahr, wenn eine Zeile `fail` trägt, die zählt ([`counted_checks`]).
fn has_failure(report: &v1::DoctorReport) -> bool {
    counted_checks(report).any(|check| check.status == v1::CheckStatus::Fail as i32)
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
            "checks": report.checks.iter().map(check_json).collect::<Vec<_>>(),
            "backends": report
                .backends
                .iter()
                .map(|group| {
                    json!({
                        "name": group.name,
                        "stage": group.stage,
                        "selected": group.selected,
                        "checks": group.checks.iter().map(check_json).collect::<Vec<_>>(),
                    })
                })
                .collect::<Vec<_>>(),
        }));
        return;
    }

    print!(
        "{}",
        table(&["", "CHECK", "EVIDENCE"], &rows(&report.checks))
    );
    for group in &report.backends {
        println!();
        println!("{}", group_heading(group));
        print!(
            "{}",
            table(&["", "CHECK", "EVIDENCE"], &rows(&group.checks))
        );
    }

    // Jede nicht-grüne Zeile bekommt ihren Block, nicht nur die erste: Der
    // Doctor ist die Liste dessen, was zu tun ist, und eine Liste, die nach
    // dem ersten Eintrag abbricht, schickt einen Menschen elfmal hierher
    // zurück. Auch die Zeilen eines nicht gewählten Backends bekommen ihren
    // Block: Sie zählen nicht für Status und Exit-Code, aber wer dieses
    // Backend wählen will, muss lesen können, was ihm fehlt (HUM-436,
    // Akzeptanzkriterium 1).
    for check in shown_checks(report) {
        if check.status == v1::CheckStatus::Ok as i32 {
            continue;
        }
        if let Some(diagnostic) = rebuilt(check) {
            println!();
            print!("{}", diagnostic_block(&diagnostic));
        }
    }
}

/// Eine Zeile als JSON-Wert, mit ihrem Befund, wenn sie einen trägt.
fn check_json(check: &v1::DoctorCheck) -> serde_json::Value {
    let mut value = json!({
        "id": check.id,
        "status": status_name(check.status),
        "evidence": check.evidence,
    });
    if let (Some(object), Some(diagnostic)) = (value.as_object_mut(), rebuilt(check)) {
        object.insert("diagnostic".to_owned(), diagnostic_json(&diagnostic));
    }
    value
}

/// Die Zeilen einer Tabelle: Marke, Kennung, Beleg.
fn rows(checks: &[v1::DoctorCheck]) -> Vec<Vec<String>> {
    checks
        .iter()
        .map(|check| {
            vec![
                mark(check.status),
                check.id.clone(),
                crate::render::one_line(&check.evidence),
            ]
        })
        .collect()
}

/// Die Überschrift über der Tabelle eines Backends, etwa
/// `backend engine (engine/plain)` oder `backend engine (engine/plain, selected)`.
fn group_heading(group: &v1::DoctorBackend) -> String {
    let stage = crate::render::one_line(&group.stage);
    let name = crate::render::one_line(&group.name);
    if group.selected {
        format!("backend {name} ({stage}, selected)")
    } else {
        format!("backend {name} ({stage})")
    }
}

/// Der schlimmste Status im Bericht, als Kurzname.
fn worst(report: &v1::DoctorReport) -> &'static str {
    let mut worst = CheckStatus::Ok;
    for check in counted_checks(report) {
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

    use super::{
        Diagnostic, LLM_002, Source, answered_status, doctor, failure_facts, group_heading,
        has_failure, mark, replace, status_name, timed_out, worst,
    };

    /// Ein Weg zum Daemon, der in seine Frist läuft, schlägt denselben
    /// dauerhaften Start vor wie jeder andere „kein Daemon", nicht
    /// `humanitld` im Vordergrund (HUM-189).
    #[test]
    fn a_timeout_proposes_a_lasting_start_not_the_foreground_daemon() {
        let diagnostic = timed_out("GetInfo", std::time::Duration::from_secs(3));
        let fix = diagnostic.fix.expect("a fix");
        assert_ne!(
            fix,
            humanitl_core::FixAction::CopyCommand("humanitld".to_owned())
        );
        assert!(
            matches!(&fix, humanitl_core::FixAction::InstallService)
                || matches!(&fix, humanitl_core::FixAction::CopyCommand(c)
                    if c.starts_with("systemctl --user start humanitld.")),
            "{fix:?}"
        );
    }

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
            backends: Vec::new(),
        }
    }

    /// Eine Gruppe mit einer Zeile dieses Status.
    fn group(name: &str, selected: bool, status: v1::CheckStatus) -> v1::DoctorBackend {
        v1::DoctorBackend {
            name: name.to_owned(),
            stage: name.to_owned(),
            selected,
            checks: vec![v1::DoctorCheck {
                id: "available".to_owned(),
                status: status as i32,
                evidence: "x".to_owned(),
                diagnostic: None,
            }],
        }
    }

    /// Eine rote Zeile in der Gruppe eines Backends färbt den Bericht rot und
    /// setzt den Exit-Code, eine gelbe nicht (HUM-436).
    #[test]
    fn a_failing_backend_line_fails_the_report_and_a_warning_does_not() {
        let mut warned = v1::DoctorReport::default();
        warned
            .backends
            .push(group("chosen", true, v1::CheckStatus::Warn));
        assert!(!has_failure(&warned));
        assert_eq!(worst(&warned), "warn");

        let mut failed = v1::DoctorReport::default();
        failed
            .backends
            .push(group("chosen", true, v1::CheckStatus::Fail));
        assert!(has_failure(&failed));
        assert_eq!(worst(&failed), "fail");
    }

    /// Ein gesunder Rechner mit einer Stufe, die noch nicht zugelassen ist:
    /// Ihre Gruppe steht da, gelb mit `SANDBOX_048`, und der Bericht bleibt
    /// `ok` mit Exit 0.
    #[test]
    fn a_healthy_machine_with_an_unadmitted_backend_is_ok() {
        let mut report = v1::DoctorReport {
            checks: CheckId::ALL
                .into_iter()
                .map(|id| convert::doctor_check_to_proto(&CheckOutcome::ok(id, "fine")))
                .collect(),
            backends: Vec::new(),
        };
        report
            .backends
            .push(group("chosen", true, v1::CheckStatus::Ok));
        let mut other = group("other", false, v1::CheckStatus::Warn);
        other.checks[0].id = "admitted".to_owned();
        other.checks[0].diagnostic = Some(v1::Diagnostic {
            code: "SANDBOX_048".to_owned(),
            why: "not admitted yet".to_owned(),
            ..v1::Diagnostic::default()
        });
        report.backends.push(other);
        assert_eq!(worst(&report), "ok");
        assert!(!has_failure(&report));
        assert_eq!(
            super::counted_checks(&report).count(),
            CheckId::ALL.len() + 1
        );
        // Gezählt wird sie nicht, gezeigt schon: Ihr Befund bekommt einen
        // Block, damit lesbar ist, was diesem Backend fehlt.
        assert!(super::shown_checks(&report).any(|check| {
            check
                .diagnostic
                .as_ref()
                .is_some_and(|diagnostic| diagnostic.code == "SANDBOX_048")
        }));
    }

    #[test]
    fn only_a_daemon_before_minor_35_is_too_old_for_backend_lines() {
        let reachable = |minor| DaemonFacts::Reachable {
            socket: std::path::PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            version: "0.0.0".to_owned(),
            expected_version: "0.0.0".to_owned(),
            proto: (humanitl_ipc::PROTO_MAJOR, minor),
            expected_proto: (humanitl_ipc::PROTO_MAJOR, humanitl_ipc::PROTO_MINOR),
        };
        assert_eq!(super::older_than_backends(&reachable(34)), Some(34));
        assert_eq!(super::older_than_backends(&reachable(35)), None);
        assert_eq!(super::older_than_backends(&reachable(36)), None);
    }

    #[test]
    fn the_heading_of_a_group_names_stage_and_choice() {
        let mut chosen = group("engine", true, v1::CheckStatus::Ok);
        chosen.stage = "engine/plain".to_owned();
        assert_eq!(
            group_heading(&chosen),
            "backend engine (engine/plain, selected)"
        );
        chosen.selected = false;
        assert_eq!(group_heading(&chosen), "backend engine (engine/plain)");
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
                expected_version: "0.0.0".to_owned(),
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

    /// Der Status steht im Satz von `LLM_002`; nur `401` und `403` gelten
    /// (HUM-283).
    #[test]
    fn the_status_of_a_refusal_is_read_from_llm_002() {
        assert_eq!(
            answered_status("the LLM server answered 401 at https://h/v1/models. It requires"),
            Some(401)
        );
        assert_eq!(
            answered_status("the LLM server answered 403 at https://h/v1/models to the key"),
            Some(403)
        );
        assert_eq!(answered_status("the LLM server answered 500 at x"), None);
        assert_eq!(answered_status("no number here"), None);
    }

    /// Der Fall des Besitzers vom 2026-10-23: Die Probe ohne Schlüssel bekommt
    /// `401`. Die Zeile sagt nicht „did not answer", sondern nennt den Status,
    /// dass der Doctor keinen Schlüssel schickt, und ob einer hinterlegt ist
    /// (HUM-283).
    #[test]
    fn a_401_to_the_keyless_probe_is_a_refusal_not_silence() {
        let refusal = || {
            Diagnostic::builder(LLM_002, humanitl_core::Severity::Error)
                .why(
                    "the LLM server answered 401 at https://llm.example.net/api/v1/models. It \
                     requires an access key, and none was sent.",
                )
                .fix(humanitl_core::FixAction::ChangeSetting {
                    key: "llm.api_key_ref".to_owned(),
                    value: String::new(),
                })
                .build()
        };
        let endpoint = "https://llm.example.net/api/v1/".to_owned();
        let line = doctor::llm_line(&failure_facts(endpoint.clone(), refusal(), Some(true)));
        let evidence = line.evidence();
        assert!(!evidence.contains("did not answer"), "{evidence}");
        assert!(evidence.contains("answered 401"), "{evidence}");
        assert!(evidence.contains("sends no access key"), "{evidence}");
        assert!(evidence.contains("an access key is stored"), "{evidence}");
        assert_ne!(
            line.diagnostic().and_then(|finding| finding.fix.clone()),
            refusal().fix,
            "with a key stored, the fix is not `humanitl llm key set`"
        );

        let none = doctor::llm_line(&failure_facts(endpoint, refusal(), Some(false)));
        assert!(none.evidence().contains("no access key is stored"));
        assert_eq!(
            none.diagnostic().and_then(|finding| finding.fix.clone()),
            refusal().fix
        );
    }
}
