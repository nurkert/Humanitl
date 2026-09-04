//! Der echte Dienst und der Fake, durch dieselbe Tabelle von Anfragen.
//!
//! `humanitl.v1` hat zwei Implementierungen (ADR-0003, ADR-0018): den Dienst
//! des Daemons und den Fake, gegen den die Oberfläche gebaut und getestet
//! wird. `backlog/CONVENTIONS.md` 4.12 sagt es ausdrücklich: „Der Fake-Daemon
//! meldet dieselben Codes wie der echte, damit die Oberfläche nicht gegen ein
//! Verhalten übt, das der Daemon ablehnt."
//!
//! Genau das lief auseinander, und niemand merkte es, weil beide Seiten nur
//! gegen sich selbst geprüft wurden. Beispiele, die dieser Test jetzt festhält:
//! `Decide` ohne Flow-Id war beim echten Dienst `IPC_004` und
//! `InvalidArgument`, beim Fake ein `Ok` mit leerem Ergebnis — und die Regel
//! aus `remember` lag danach trotzdem im Regelsatz. Eine unlesbare Flow-Id war
//! hier `IPC_004`, dort `IPC_003`. Ein `sha256` mit weniger als 32 Bytes wurde
//! vom Fake mit Nullen aufgefüllt und traf damit womöglich einen fremden Body.
//!
//! Der Test fährt beide Dienste über dieselbe Schnittstelle an, die auch tonic
//! benutzt (`v1::humanitl_server::Humanitl`), ohne Socket und ohne Netz.
//! Verglichen wird, was ein Client sieht: der gRPC-Code und der
//! Diagnostic-Code in den Details. **Nicht** verglichen wird der Inhalt einer
//! erfolgreichen Antwort — der Fake spielt eine Aufzeichnung, der echte Dienst
//! einen laufenden Proxy, und ein Flow des einen ist keiner des anderen.
//!
//! Was der Fake **nicht** kann, steht in `TABLE` deshalb auch nicht: Er hat
//! keinen Regelspeicher und keine Aufzeichnung, also gibt es keine gemeinsame
//! Antwort auf `Rules{add}`, auf einen Filter der Aufzeichnung, auf einen
//! Cursor oder auf `GetBody` eines Bodys, den nur eine Datenbank kennt. Die
//! Tabelle prüft, was beide Seiten ohne Welt beantworten können: die Prüfungen
//! aus [`humanitl_ipc::validate`], die vor jeder Wirkung stehen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use humanitl_config::Config;
use humanitl_core::diagnostics::codes;
use humanitl_core::rule::{Action, HostPattern, Matcher};
use humanitl_core::{Rule, RuleId, SessionId};
use humanitl_ipc::DaemonApi as _;
use humanitl_ipc::fake::{BUNDLED_BLOCK_RULE, FakeDaemon, FakeOptions, Session};
use humanitl_ipc::v1::humanitl_server::Humanitl as _;
use humanitl_ipc::{DaemonService, IpcServer, SandboxService, diagnostic_from_status, v1};
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::{FlowRegistry, HoldQueue};
use tokio_stream::StreamExt as _;
use tonic::{Code, Request, Status};

/// Das Token, mit dem der Fake in diesem Test angesprochen wird.
///
/// Der echte Dienst prüft es im Interceptor beim Aufsetzen des Sockets und
/// nicht in der Methode; hier wird beides ohne Socket gerufen, also trägt nur
/// der Fake es in den Metadaten.
const TOKEN: &str = "parity-token";

/// Eine Flow-Id, die es in keinem der beiden Dienste gibt.
const UNKNOWN_FLOW: &str = "018f0001-0000-7000-8000-0000000f0000";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/sessions")
        .join(name)
}

/// Der Dienst des echten Daemons, ohne Aufzeichnung, mit Regelspeicher.
///
/// Das ist die Fassung, die der Fake spiegelt: beide halten ihre Flows im
/// Speicher. Ein Daemon mit Aufzeichnung kann mehr, aber nichts anderes.
///
/// Der Regelspeicher muss sein, sonst antwortete `Rules` mit „dieser Daemon
/// hat keinen Regelspeicher", bevor er die Regel überhaupt liest — und die
/// Zeilen zu `add`, `update`, `dry_run` und `test` prüften nichts.
///
/// Der Speicher liegt in einem Wegwerf-Verzeichnis und trägt dieselbe
/// mitgelieferte Regel wie der Fake: Ohne sie prüfte die Zeile zu
/// `Rules{remove}` auf der einen Seite eine mitgelieferte Regel und auf der
/// anderen eine unbekannte Id.
fn real() -> (IpcServer, tempfile::TempDir) {
    let config = Config::default();
    let session = SessionId::new();
    let queue = Arc::new(HoldQueue::with_registry(
        &config.limits,
        Arc::new(FlowRegistry::new(&config.limits)),
    ));
    let dir = tempfile::tempdir().expect("a temporary directory");
    let (store, diagnostics) =
        RulesStore::load(&dir.path().join("rules.yaml"), &bundled(), session);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let server = IpcServer::new(queue, &config, Some(session)).with_rules(Arc::new(store), None);
    (server, dir)
}

/// Die mitgelieferte Block-Regel, die der Fake ebenfalls kennt.
fn bundled() -> Vec<Rule> {
    let id = RuleId::parse(BUNDLED_BLOCK_RULE).expect("the bundled id is a uuid");
    let host = HostPattern::parse("**.doubleclick.net").expect("a host pattern");
    vec![Rule::new(id, Action::Block, Matcher::host(host))]
}

/// Der Fake über einer mitgelieferten Sitzung, als tonic-Dienst.
fn fake() -> DaemonService<FakeDaemon> {
    let session = Session::load(&fixture("mixed.jsonl")).expect("fixture parses");
    DaemonService::new(
        Arc::new(FakeDaemon::new(session, FakeOptions::default())),
        TOKEN,
    )
}

/// Eine Regel-Id in der Form, die beide Dienste lesen können.
///
/// `update` verlangt eine lesbare Id, bevor es überhaupt zur Regel kommt; ohne
/// sie prüften die beiden Dienste verschiedene Dinge.
fn uuid_like() -> String {
    "018f0001-0000-7000-8000-0000000e0000".to_owned()
}

/// Eine Anfrage mit dem Token des Fakes in den Metadaten.
fn authed<T>(message: T) -> Request<T> {
    let mut request = Request::new(message);
    request.metadata_mut().insert(
        humanitl_ipc::TOKEN_METADATA_KEY,
        TOKEN.parse().expect("the token is ascii"),
    );
    request
}

/// Was ein Client von einem Aufruf sieht: der gRPC-Code und der Befund.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    code: Code,
    diagnostic: Option<String>,
}

impl Seen {
    /// Ein Erfolg. Der Inhalt wird bewusst nicht verglichen.
    const fn ok() -> Self {
        Self {
            code: Code::Ok,
            diagnostic: None,
        }
    }

    fn of<T>(result: Result<T, Status>) -> Self {
        match result {
            Ok(_) => Self::ok(),
            Err(status) => Self {
                code: status.code(),
                diagnostic: diagnostic_from_status(&status).map(|diagnostic| diagnostic.code),
            },
        }
    }
}

/// Erwartet einen Fehler mit genau diesem Code und diesem Befund.
fn refused(code: Code, diagnostic: humanitl_core::DiagnosticCode) -> Seen {
    Seen {
        code,
        diagnostic: Some(diagnostic.as_str().to_owned()),
    }
}

/// Eine Zeile der Tabelle: Was geschickt wird und was beide Dienste antworten.
struct Case {
    what: &'static str,
    expected: Seen,
    call: for<'a> fn(&'a IpcServer, &'a DaemonService<FakeDaemon>) -> BothFutures<'a>,
}

/// Die beiden Aufrufe einer Zeile, schon gestartet.
type BothFutures<'a> = std::pin::Pin<Box<dyn Future<Output = (Seen, Seen)> + Send + 'a>>;

/// Baut die beiden Aufrufe einer Zeile aus einer Anfrage.
macro_rules! both {
    ($method:ident, $message:expr) => {{
        fn call<'a>(real: &'a IpcServer, fake: &'a DaemonService<FakeDaemon>) -> BothFutures<'a> {
            // Beide Dienste bekommen dieselbe Nachricht; nur die Metadaten
            // unterscheiden sich, weil der echte Dienst sein Token im
            // Interceptor prüft und nicht in der Methode.
            let real = real.$method(Request::new($message));
            let fake = fake.$method(authed($message));
            Box::pin(async move { (Seen::of(real.await), Seen::of(fake.await)) })
        }
        call
    }};
}

// Eine Tabelle ist lang; jede Zeile ist ein Fall, und sie in Hilfsfunktionen
// zu zerlegen machte sie schlechter lesbar, nicht besser.
#[allow(clippy::too_many_lines)]
fn table() -> Vec<Case> {
    vec![
        // ---- Decide: die Anfrage als Ganzes -------------------------------
        Case {
            what: "decide without a flow id",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: Vec::new(),
                    decision: Some(v1::decide_request::Decision::Allow(())),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            // Der Fake legte die Regel vorher an und antwortete `Ok`.
            what: "decide without a flow id but with a rule to remember",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: Vec::new(),
                    decision: Some(v1::decide_request::Decision::Allow(())),
                    remember: Some(v1::Rule {
                        rule_id: String::new(),
                        action: v1::RuleAction::Allow as i32,
                        ..v1::Rule::default()
                    }),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            what: "decide without a decision",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: vec![UNKNOWN_FLOW.to_owned()],
                    decision: None,
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            what: "decide with an unreadable flow id",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: vec!["not-a-flow-id".to_owned()],
                    decision: Some(v1::decide_request::Decision::Allow(())),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            what: "decide with an unreadable edited request",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: vec![UNKNOWN_FLOW.to_owned()],
                    decision: Some(v1::decide_request::Decision::AllowEdited(
                        v1::EditedRequest {
                            method: v1::Method::Get as i32,
                            url: "github.com/no-scheme".to_owned(),
                            ..v1::EditedRequest::default()
                        }
                    )),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            what: "decide with an edited body over limits.hold_body_cap_bytes",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: vec![UNKNOWN_FLOW.to_owned()],
                    decision: Some(v1::decide_request::Decision::AllowEdited(
                        v1::EditedRequest {
                            method: v1::Method::Get as i32,
                            url: "https://github.com/".to_owned(),
                            body: vec![
                                0u8;
                                usize::try_from(
                                    humanitl_config::Limits::default().hold_body_cap_bytes + 1
                                )
                                .unwrap_or(usize::MAX)
                            ],
                            ..v1::EditedRequest::default()
                        }
                    )),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            // Der Vertrag verlangt hier kein Fehlerstatus, sondern ein
            // Ergebnis je Flow mit `IPC_002`. Beide Seiten antworten `Ok`;
            // `allow_edited_for_two_flows_refuses_each_the_same_way` sieht sich
            // die Ergebnisse an.
            what: "allow_edited for two flows",
            expected: Seen::ok(),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: vec![UNKNOWN_FLOW.to_owned(), UNKNOWN_FLOW.to_owned()],
                    decision: Some(v1::decide_request::Decision::AllowEdited(
                        v1::EditedRequest {
                            method: v1::Method::Get as i32,
                            url: "https://github.com/".to_owned(),
                            ..v1::EditedRequest::default()
                        }
                    )),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        Case {
            // Ein Flow, den keiner der beiden kennt: nichts wurde entschieden,
            // also ist der Aufruf gescheitert und kein leerer Erfolg.
            what: "decide a flow nobody holds",
            expected: refused(Code::FailedPrecondition, codes::IPC_003),
            call: both!(
                decide,
                v1::DecideRequest {
                    flow_ids: vec![UNKNOWN_FLOW.to_owned()],
                    decision: Some(v1::decide_request::Decision::Allow(())),
                    ..v1::DecideRequest::default()
                }
            ),
        },
        // ---- GetFlow -----------------------------------------------------
        Case {
            what: "get_flow with an unreadable id",
            expected: refused(Code::InvalidArgument, codes::IPC_004),
            call: both!(
                get_flow,
                v1::FlowRef {
                    flow_id: "not-a-flow-id".to_owned(),
                }
            ),
        },
        Case {
            // `IPC_003` heißt sonst `FailedPrecondition`; aus `GetFlow` heißt
            // es „den gibt es nicht" und damit `NOT_FOUND`. Die Ausnahme steht
            // in [`humanitl_ipc::server_stub::get_flow_status`], an der einen
            // Stelle, an der übersetzt wird — der Fake antwortete vorher
            // `FailedPrecondition`, der echte Dienst `NOT_FOUND` mit einem
            // nackten String statt eines Befunds.
            what: "get_flow with an id nobody knows",
            expected: refused(Code::NotFound, codes::IPC_003),
            call: both!(
                get_flow,
                v1::FlowRef {
                    flow_id: UNKNOWN_FLOW.to_owned(),
                }
            ),
        },
        // ---- GetBody -----------------------------------------------------
        Case {
            what: "get_body with a sha256 that is not 32 bytes",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                get_body,
                v1::BodyRef {
                    sha256: vec![1, 2, 3],
                    size: 0,
                    truncated: false,
                    content_type: String::new(),
                }
            ),
        },
        // ---- ListFlows ---------------------------------------------------
        Case {
            what: "list_flows with a sort key nobody can sort by",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                list_flows,
                v1::ListFlowsRequest {
                    order_by: "cost desc".to_owned(),
                    ..v1::ListFlowsRequest::default()
                }
            ),
        },
        Case {
            what: "list_flows in the default order",
            expected: Seen::ok(),
            call: both!(list_flows, v1::ListFlowsRequest::default()),
        },
        // ---- Rules -------------------------------------------------------
        Case {
            what: "rules without an operation",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(rules, v1::RulesRequest { op: None }),
        },
        Case {
            // Der Weg neben `DecideRequest.remember`: `add` legte im Fake die
            // Nachricht der Leitung wörtlich ab, ohne den Leser, der `bundled`
            // und `passthrough_llm` auf `false` zwingt. Eine Regel ohne
            // Matcher ist derselbe Weg, nur sichtbar.
            what: "rules add with a rule that has no matcher",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Add(v1::Rule {
                        rule_id: String::new(),
                        action: v1::RuleAction::Allow as i32,
                        matcher: None,
                        ..v1::Rule::default()
                    })),
                }
            ),
        },
        Case {
            what: "rules add with a rule that has no action",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Add(v1::Rule {
                        rule_id: String::new(),
                        action: v1::RuleAction::Unspecified as i32,
                        matcher: Some(v1::RuleMatcher {
                            host: "**.github.com".to_owned(),
                            ..v1::RuleMatcher::default()
                        }),
                        ..v1::Rule::default()
                    })),
                }
            ),
        },
        Case {
            // Ein Host-Muster ist ein Fehler *in der Regel*, nicht in der
            // Anfrage: `RULES_003`, nicht `IPC_005`. Der Fake bildete jede
            // Leseart pauschal auf `IPC_005` ab.
            what: "rules add with a host pattern nobody can read",
            expected: refused(Code::InvalidArgument, codes::RULES_003),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Add(v1::Rule {
                        rule_id: String::new(),
                        action: v1::RuleAction::Allow as i32,
                        matcher: Some(v1::RuleMatcher {
                            host: "*foo.com".to_owned(),
                            ..v1::RuleMatcher::default()
                        }),
                        ..v1::Rule::default()
                    })),
                }
            ),
        },
        Case {
            what: "rules update with a rule that has no matcher",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Update(v1::Rule {
                        rule_id: uuid_like(),
                        action: v1::RuleAction::Allow as i32,
                        matcher: None,
                        ..v1::Rule::default()
                    })),
                }
            ),
        },
        Case {
            // Ein Probelauf ohne Regel hat nichts, wogegen er liefe. Der Fake
            // lieferte still eine leere Trefferliste, und die sieht aus wie
            // „diese Regel trifft nichts".
            what: "rules dry_run without a rule",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::DryRun(v1::rules_request::DryRun {
                        rule: None,
                        limit: 10,
                    })),
                }
            ),
        },
        Case {
            what: "rules test with a url that is not a request url",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Test(v1::rules_request::Test {
                        method: v1::Method::Get as i32,
                        url: "github.com/no-scheme".to_owned(),
                        upgrade: v1::Upgrade::None as i32,
                    })),
                }
            ),
        },
        Case {
            // Eine mitgelieferte Regel ist unlöschbar. Der Fake behielt sie
            // still und antwortete `Ok`; der echte Dienst sagt `RULES_010`.
            what: "rules remove of a bundled rule",
            expected: refused(Code::FailedPrecondition, codes::RULES_010),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Remove(BUNDLED_BLOCK_RULE.to_owned())),
                }
            ),
        },
        Case {
            what: "rules test without a method",
            expected: refused(Code::InvalidArgument, codes::IPC_005),
            call: both!(
                rules,
                v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Test(v1::rules_request::Test {
                        method: v1::Method::Unspecified as i32,
                        url: "https://github.com/".to_owned(),
                        upgrade: v1::Upgrade::None as i32,
                    })),
                }
            ),
        },
        // ---- ProbeLlm ----------------------------------------------------
        Case {
            what: "probe_llm with an endpoint that is not a URL",
            expected: refused(Code::Internal, codes::LLM_007),
            call: both!(
                probe_llm,
                v1::ProbeLlmRequest {
                    endpoint: "not a url".to_owned(),
                    timeout_ms: 0,
                }
            ),
        },
        Case {
            what: "probe_llm without an endpoint",
            expected: refused(Code::Internal, codes::LLM_007),
            call: both!(
                probe_llm,
                v1::ProbeLlmRequest {
                    endpoint: String::new(),
                    timeout_ms: 0,
                }
            ),
        },
    ]
}

/// Beide Dienste antworten auf dieselbe Anfrage dasselbe.
#[tokio::test]
async fn the_fake_answers_like_the_daemon() {
    let (real, _dir) = real();
    let fake = fake();

    for case in table() {
        let (from_real, from_fake) = (case.call)(&real, &fake).await;
        assert_eq!(
            from_real, case.expected,
            "the daemon answers {:?} differently than the table says",
            case.what
        );
        assert_eq!(
            from_fake, from_real,
            "the fake and the daemon disagree about {:?}",
            case.what
        );
    }
}

/// Die drei Garantien: Was keiner der beiden gemessen hat, meldet auch keiner.
///
/// Der Fake behauptet drei bestandene Prüfungen und schreibt in jede Evidenz,
/// dass nichts gemessen wurde (CONVENTIONS 4.7). Der echte Dienst kann das
/// nicht behaupten, solange keine Sandbox läuft — und schickt deshalb gar kein
/// Ergebnis statt drei graue. Genau diese Richtung ist die, die niemand
/// lockern darf: „unbekannt" darf nie wie „bestanden" aussehen
/// (CONVENTIONS 4.13, `backlog/sprint-3.md` HUM-041).
///
/// Gemeinsam ist beiden die Form: drei Garantien in der Reihenfolge der
/// Varianten, jede mit einer Evidenz, die nicht leer ist, und ein roter Befund
/// nie ohne `Diagnostic`.
#[tokio::test]
async fn the_isolation_check_reports_only_what_was_measured() {
    let checks = |events: Vec<v1::SandboxEvent>| -> Vec<v1::CheckResult> {
        events
            .into_iter()
            .filter_map(|event| match event.event {
                Some(v1::sandbox_event::Event::Check(check)) => Some(check),
                _ => None,
            })
            .collect()
    };

    let from_fake = checks(sandbox_events(&fake(), isolation_check()).await);
    assert_eq!(
        from_fake
            .iter()
            .map(|check| check.check)
            .collect::<Vec<_>>(),
        vec![
            v1::IsolationCheck::NoNetworkInterface as i32,
            v1::IsolationCheck::SingleSocket as i32,
            v1::IsolationCheck::SeccompActive as i32,
        ],
        "the fake answers the three guarantees in the order of the variants"
    );
    for check in &from_fake {
        assert!(!check.evidence.is_empty(), "a check without evidence");
        assert!(
            check.passed || check.diagnostic.is_some(),
            "a red check without a finding: {check:?}"
        );
    }

    let (real, _dir) = real_with_sandbox();
    let from_real = checks(sandbox_events(&real, isolation_check()).await);
    assert!(
        from_real.is_empty(),
        "no sandbox ran, so nothing is proven: {from_real:?}"
    );
}

/// Die Anfrage, die die drei Garantien holt.
fn isolation_check() -> v1::SandboxRequest {
    v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::IsolationCheck(())),
    }
}

/// Der echte Dienst mit Sandbox-RPC; ohne sie antwortete er „dieser Daemon hat
/// keine Sandbox", bevor er die Anfrage überhaupt liest.
fn real_with_sandbox() -> (IpcServer, tempfile::TempDir) {
    let (server, dir) = real();
    let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
        "HOME",
        dir.path().to_string_lossy(),
    )]));
    let service = SandboxService::new(Config::default(), paths, SessionId::new());
    (server.with_sandbox(service), dir)
}

/// Alle Ereignisse einer Sandbox-Operation, in der Reihenfolge des Stroms.
async fn sandbox_events<S>(service: &S, request: v1::SandboxRequest) -> Vec<v1::SandboxEvent>
where
    S: v1::humanitl_server::Humanitl,
{
    let stream = service
        .sandbox(authed(request))
        .await
        .expect("the sandbox rpc answers")
        .into_inner();
    let mut stream = std::pin::pin!(stream);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.expect("no stream error"));
    }
    events
}

/// `allow_edited` für mehrere Flows: beide lehnen jeden einzeln mit `IPC_002`
/// ab, und keiner legt dabei eine Regel an.
///
/// Der Fake prüfte die Zahl der Flows erst in der Schleife, nach dem Anlegen
/// der Regel und nach dem Lesen der Id — eine unlesbare Id kam damit als
/// `IPC_003` heraus, wo der echte Dienst `IPC_002` sagte.
#[tokio::test]
async fn allow_edited_for_two_flows_refuses_each_the_same_way() {
    let request = || v1::DecideRequest {
        flow_ids: vec!["not-a-flow-id".to_owned(), UNKNOWN_FLOW.to_owned()],
        decision: Some(v1::decide_request::Decision::AllowEdited(
            v1::EditedRequest {
                method: v1::Method::Get as i32,
                url: "https://github.com/".to_owned(),
                ..v1::EditedRequest::default()
            },
        )),
        remember: Some(v1::Rule {
            rule_id: String::new(),
            action: v1::RuleAction::Allow as i32,
            ..v1::Rule::default()
        }),
        ..v1::DecideRequest::default()
    };

    let (real, _dir) = real();
    let from_real = real
        .decide(Request::new(request()))
        .await
        .expect("the call itself succeeds")
        .into_inner();
    let from_fake = fake()
        .decide(authed(request()))
        .await
        .expect("the call itself succeeds")
        .into_inner();

    for (side, response) in [("daemon", &from_real), ("fake", &from_fake)] {
        assert_eq!(response.results.len(), 2, "{side}");
        for result in &response.results {
            assert!(!result.applied, "{side}: nothing is decided: {result:?}");
            let diagnostic = result
                .diagnostic
                .as_ref()
                .unwrap_or_else(|| panic!("{side}: a diagnostic per flow"));
            assert_eq!(diagnostic.code, codes::IPC_002.as_str(), "{side}");
        }
        assert!(
            response.created_rule_id.is_empty(),
            "{side}: a request that decides nothing creates no rule"
        );
        assert!(response.created_rule.is_none(), "{side}");
    }
}

/// Die Id des ersten Flows, den der Fake hält.
///
/// Der Abspieler muss dafür laufen: vor ihm ist die Sitzung nur eine Datei.
async fn held_flow(daemon: &Arc<FakeDaemon>) -> String {
    let mut stream = daemon.subscribe(v1::SubscribeRequest {
        since_flow_id: String::new(),
        include_passthrough: true,
    });
    daemon.start();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let next = tokio::time::timeout_at(deadline, stream.next())
            .await
            .expect("the session holds a flow within its span");
        let Some(event) = next else {
            panic!("the event stream ended before a flow was held");
        };
        if let Some(v1::flow_event::Event::Held(held)) = event.event {
            return held.flow_id;
        }
    }
}

/// Keine Tür lässt eine Regel `bundled` oder `passthrough_llm` von der Leitung
/// mitbringen — weder `DecideRequest.remember` noch `Rules{add}` noch
/// `Rules{update}`.
///
/// Beide Felder zwingt [`humanitl_ipc::validate::rule`] auf `false`. Eine
/// mitgelieferte Regel ist unlöschbar, die Durchreiche zum Sprachmodell wird
/// gestreamt statt gehalten; beides darf sich kein Client selbst geben
/// (`proto/humanitl/v1/rules.proto`, HUM-039). Der Fake legte die
/// Wire-Nachricht wörtlich ab: erst in `remember`, und nachdem das geschlossen
/// war, immer noch in `add` und `update`. Deshalb steht hier jede Tür einzeln.
#[tokio::test(start_paused = true)]
async fn no_door_lets_a_rule_claim_to_be_bundled_or_passthrough() {
    /// Eine Regel, die sich mehr nimmt, als der Vertrag hergibt.
    fn overreaching(rule_id: &str) -> v1::Rule {
        v1::Rule {
            rule_id: rule_id.to_owned(),
            action: v1::RuleAction::Allow as i32,
            matcher: Some(v1::RuleMatcher {
                host: "**.github.com".to_owned(),
                ..v1::RuleMatcher::default()
            }),
            bundled: true,
            passthrough_llm: true,
            ..v1::Rule::default()
        }
    }

    /// Die Regel mit dieser Id, so wie der Dienst sie abgelegt hat.
    fn stored(response: &v1::RulesResponse, rule_id: &str) -> v1::Rule {
        response
            .rules
            .iter()
            .find(|rule| rule.rule_id == rule_id)
            .cloned()
            .unwrap_or_else(|| panic!("the rule {rule_id} is not in the set"))
    }

    fn check(side: &str, door: &str, rule: &v1::Rule) {
        assert!(
            !rule.bundled,
            "{side}: {door} let a rule from the wire become bundled"
        );
        assert!(
            !rule.passthrough_llm,
            "{side}: {door} let a rule from the wire become the passthrough to the language model"
        );
    }

    // ---- Tür 1: Rules{add} und Rules{update}, auf beiden Seiten ----------
    let (real, _dir) = real();
    let doors = fake();
    let id = uuid_like();
    for (side, added, updated) in [
        (
            "daemon",
            real.rules(Request::new(v1::RulesRequest {
                op: Some(v1::rules_request::Op::Add(overreaching(&id))),
            }))
            .await
            .expect("add works")
            .into_inner(),
            real.rules(Request::new(v1::RulesRequest {
                op: Some(v1::rules_request::Op::Update(overreaching(&id))),
            }))
            .await
            .expect("update works")
            .into_inner(),
        ),
        (
            "fake",
            doors
                .rules(authed(v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Add(overreaching(&id))),
                }))
                .await
                .expect("add works")
                .into_inner(),
            doors
                .rules(authed(v1::RulesRequest {
                    op: Some(v1::rules_request::Op::Update(overreaching(&id))),
                }))
                .await
                .expect("update works")
                .into_inner(),
        ),
    ] {
        check(side, "add", &stored(&added, &id));
        check(side, "update", &stored(&updated, &id));
    }

    // ---- Tür 2: DecideRequest.remember, am Fake mit einem gehaltenen Flow -
    let fake = fake();
    let daemon = Arc::clone(fake.api());
    let held = held_flow(&daemon).await;
    let response = fake
        .decide(authed(v1::DecideRequest {
            flow_ids: vec![held],
            decision: Some(v1::decide_request::Decision::Allow(())),
            remember: Some(overreaching("")),
            ..v1::DecideRequest::default()
        }))
        .await
        .expect("deciding a held flow works")
        .into_inner();
    let created = response.created_rule.expect("the rule was remembered");
    check("fake", "remember", &created);

    // Und so, wie sie angelegt wurde, steht sie auch im Regelsatz.
    let rules = fake
        .rules(authed(v1::RulesRequest {
            op: Some(v1::rules_request::Op::List(())),
        }))
        .await
        .expect("listing rules works")
        .into_inner();
    check(
        "fake",
        "remember (stored)",
        &stored(&rules, &created.rule_id),
    );
}
