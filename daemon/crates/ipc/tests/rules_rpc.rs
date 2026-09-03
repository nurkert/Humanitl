//! Der Regel-RPC über einen echten Socket (HUM-027).
//!
//! Die Tests sprechen den Daemon so an, wie Oberfläche und CLI es tun: über
//! den Client, über den Socket, mit dem Token. Geprüft wird dabei immer beides,
//! die Antwort **und** die Datei: eine Regel, die der Daemon meldet, aber nicht
//! schreibt, wäre am nächsten Morgen weg, und eine Sitzungsregel, die er
//! schreibt, gälte länger als die Sitzung.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use humanitl_config::{Config, Limits};
use humanitl_core::rule::{Action, HostPattern, Matcher, Rule};
use humanitl_core::{
    Authority, BodyRef, Flow, FlowEvent, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
    TransitionInput,
};
use humanitl_ipc::client::Client;
use humanitl_ipc::{IpcServer, client, v1};
use humanitl_proxy::registry::FlowRecord;
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::{ConnMeta, FlowRegistry, HoldQueue};
use humanitl_recorder::{Recorder, RecorderSettings, SessionMeta};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tonic::Code;

/// Ein laufender Daemon mit Regelspeicher und Aufzeichnung.
struct Daemon {
    _dir: tempfile::TempDir,
    rules_path: PathBuf,
    session: SessionId,
    queue: Arc<HoldQueue>,
    store: Arc<RulesStore>,
    recorder: Recorder,
    client: Client,
    stop: Option<oneshot::Sender<()>>,
    join: JoinHandle<Result<(), humanitl_core::Diagnostic>>,
}

impl Daemon {
    /// Startet den Dienst mit den mitgelieferten Regeln `bundled`.
    async fn start(bundled: &[Rule]) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("daemon.sock");
        let token_path = dir.path().join("token");
        let rules_path = dir.path().join("config").join("rules.yaml");
        let session = SessionId::new();

        let limits = Limits::default();
        let queue = Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ));
        let (store, diagnostics) = RulesStore::load(&rules_path, bundled, session);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let store = Arc::new(store);

        let recorder = Recorder::open(
            &dir.path().join("data").join("humanitl.db"),
            &dir.path().join("data").join("blobs"),
            RecorderSettings::default(),
        )
        .expect("recorder");
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/home/x/projekt".to_owned(),
            agent: "opencode".to_owned(),
        });

        let config = Config {
            limits,
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, Some(session))
            .with_rules(Arc::clone(&store), Some(recorder.clone()));

        let (stop, wait) = oneshot::channel();
        let join = {
            let socket = socket.clone();
            let token_path = token_path.clone();
            tokio::spawn(async move {
                humanitl_ipc::serve(&socket, &token_path, server, async move {
                    let _ = wait.await;
                })
                .await
            })
        };
        await_file(&socket).await;
        await_file(&token_path).await;
        let token = humanitl_ipc::auth::read_token(&token_path).expect("token");
        let client = client::connect_at(&socket, &token).await.expect("connect");

        Self {
            _dir: dir,
            rules_path,
            session,
            queue,
            store,
            recorder,
            client,
            stop: Some(stop),
            join,
        }
    }

    /// Eine Regel-Operation über den Socket.
    async fn rules(
        &mut self,
        op: v1::rules_request::Op,
    ) -> Result<v1::RulesResponse, tonic::Status> {
        self.client
            .rules(v1::RulesRequest { op: Some(op) })
            .await
            .map(tonic::Response::into_inner)
    }

    /// Der Inhalt von `rules.yaml`, oder `None`, wenn es die Datei nicht gibt.
    fn file(&self) -> Option<String> {
        std::fs::read_to_string(&self.rules_path).ok()
    }

    /// Die Warteschlange als eigenes Handle, damit ein gehaltener Flow den
    /// Daemon nicht ausleiht: `hold` borgt die Warteschlange, solange der Flow
    /// wartet, und der Test ruft in der Zeit weiter RPCs auf.
    fn queue(&self) -> Arc<HoldQueue> {
        Arc::clone(&self.queue)
    }

    async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = self.join.await;
    }
}

/// Wartet, bis eine Datei da ist; der Dienst legt sie beim Start an.
async fn await_file(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{} never appeared", path.display());
}

/// Eine Regel in ihrer Wire-Form.
fn wire_rule(host: &str, action: v1::RuleAction) -> v1::Rule {
    v1::Rule {
        action: action as i32,
        matcher: Some(v1::RuleMatcher {
            host: host.to_owned(),
            ..v1::RuleMatcher::default()
        }),
        ..v1::Rule::default()
    }
}

/// Dieselbe Regel, aber nur für diese Sitzung.
fn session_scoped(mut rule: v1::Rule) -> v1::Rule {
    rule.expires = Some(v1::RuleExpiry {
        expiry: Some(v1::rule_expiry::Expiry::Session(())),
    });
    rule
}

/// Ein analysierter Flow, bereit zum Warten.
fn analyzed(session: SessionId, host: &str) -> Flow {
    let request = HttpRequest::new(
        Method::GET,
        Scheme::Https,
        Authority::with_scheme(HostName::parse(host).expect("host"), Scheme::Https),
        "/repos",
    )
    .with_body(BodyRef::detached([0; 32], 0));
    let mut flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
    flow.apply(
        TransitionInput::Analyze {
            findings: Vec::new(),
        },
        SystemTime::now(),
    )
    .expect("analyze");
    flow
}

/// Eine Regel im Kern, für den mitgelieferten Satz.
fn core_rule(host: &str, action: Action) -> Rule {
    let pattern = HostPattern::parse(host).expect("pattern");
    Rule::new(humanitl_core::RuleId::new(), action, Matcher::host(pattern))
}

#[tokio::test]
async fn add_session_rule_not_persisted() {
    let mut daemon = Daemon::start(&[]).await;

    let response = daemon
        .rules(v1::rules_request::Op::Add(session_scoped(wire_rule(
            "api.github.com",
            v1::RuleAction::Allow,
        ))))
        .await
        .expect("add");

    assert_eq!(response.rules.len(), 1);
    assert_eq!(
        response.rules[0].expires,
        Some(v1::RuleExpiry {
            expiry: Some(v1::rule_expiry::Expiry::Session(()))
        })
    );
    assert_eq!(response.rules[0].position, 1);
    assert!(
        daemon.file().is_none(),
        "a session rule must not create rules.yaml"
    );

    // Und sie gilt sofort für die Auswertung im Proxy.
    let host = HostName::parse("api.github.com").expect("host");
    let key = humanitl_rules::RequestKey::new(&host, &Method::GET, "/repos", Scheme::Https, 443);
    let verdict = daemon
        .store
        .effective()
        .evaluate(&key, chrono::Utc::now(), daemon.session);
    assert_eq!(verdict.action(), Action::Allow);

    daemon.shutdown().await;
}

#[tokio::test]
async fn add_persistent_writes_file() {
    let mut daemon = Daemon::start(&[]).await;

    let response = daemon
        .rules(v1::rules_request::Op::Add(wire_rule(
            "**.github.com",
            v1::RuleAction::Allow,
        )))
        .await
        .expect("add");
    let id = response.rules[0].rule_id.clone();
    assert!(!id.is_empty(), "the daemon hands out the id");

    let text = daemon.file().expect("rules.yaml exists");
    assert!(text.contains("**.github.com"), "{text}");
    assert!(text.contains("expires: never"), "{text}");

    // Roundtrip: was geschrieben wurde, liest die Engine wieder ein.
    let (parsed, warnings) = humanitl_rules::parse_rules(&text).expect("the file parses");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed.iter().next().expect("rule").id.to_string(), id);

    daemon.shutdown().await;
}

#[tokio::test]
async fn remember_atomic() {
    let mut daemon = Daemon::start(&[]).await;
    let queue = daemon.queue();
    let mut held_flow = analyzed(daemon.session, "api.github.com");
    let flow = held_flow.id;
    queue.registry().insert(FlowRecord::new(
        &held_flow,
        &ConnMeta::plain(daemon.session),
    ));
    let held = queue
        .hold(&mut held_flow, Instant::now() + Duration::from_secs(30))
        .expect("hold");

    // `host: "*bad"` ist kein Muster: ein Stern muss ein ganzes Label sein.
    let status = daemon
        .client
        .decide(v1::DecideRequest {
            flow_ids: vec![flow.to_string()],
            decision: Some(v1::decide_request::Decision::Allow(())),
            remember: Some(wire_rule("*bad.example.com", v1::RuleAction::Allow)),
            ..v1::DecideRequest::default()
        })
        .await
        .expect_err("a broken rule stops the decision");

    assert_eq!(status.code(), Code::InvalidArgument);
    assert!(status.message().contains("RULES_003"), "{status}");
    assert!(daemon.store.list().is_empty(), "no rule was created");
    assert!(queue.deadline(flow).is_some(), "the flow is still held");

    // Mit einer gültigen Regel geht beides: erst die Regel, dann der Flow.
    let response = daemon
        .client
        .decide(v1::DecideRequest {
            flow_ids: vec![flow.to_string()],
            decision: Some(v1::decide_request::Decision::Allow(())),
            remember: Some(wire_rule("api.github.com", v1::RuleAction::Allow)),
            ..v1::DecideRequest::default()
        })
        .await
        .expect("decide")
        .into_inner();
    let created = response.created_rule.expect("the rule comes back");
    assert_eq!(response.created_rule_id, created.rule_id);
    assert_eq!(daemon.store.list().len(), 1);
    assert_eq!(held.await, humanitl_core::Decision::Allow);

    daemon.shutdown().await;
}

#[tokio::test]
async fn a_remembered_rule_does_not_outlive_a_decision_that_never_happened() {
    let mut daemon = Daemon::start(&[]).await;

    let status = daemon
        .client
        .decide(v1::DecideRequest {
            flow_ids: vec![FlowId::new().to_string()],
            decision: Some(v1::decide_request::Decision::Allow(())),
            remember: Some(wire_rule("api.github.com", v1::RuleAction::Allow)),
            ..v1::DecideRequest::default()
        })
        .await
        .expect_err("an unknown flow cannot be decided");

    assert_eq!(status.code(), Code::FailedPrecondition);
    assert!(
        daemon.store.list().is_empty(),
        "the rule of a decision that did not happen is rolled back"
    );
    assert!(daemon.file().is_none_or(|text| !text.contains("github")));

    daemon.shutdown().await;
}

#[tokio::test]
async fn bundled_remove_rejected() {
    let bundled = core_rule("models.dev", Action::Block);
    let mut daemon = Daemon::start(std::slice::from_ref(&bundled)).await;

    let listed = daemon
        .rules(v1::rules_request::Op::List(()))
        .await
        .expect("list");
    assert_eq!(listed.rules.len(), 1);
    assert!(listed.rules[0].bundled, "a bundled rule says so");

    let status = daemon
        .rules(v1::rules_request::Op::Remove(bundled.id.to_string()))
        .await
        .expect_err("a bundled rule stays");
    assert_eq!(status.code(), Code::FailedPrecondition);
    assert!(status.message().contains("RULES_010"), "{status}");

    let diagnostic = humanitl_ipc::diagnostic_from_status(&status).expect("details");
    let fix = diagnostic.fix.expect("the refusal names a way out");
    assert!(
        matches!(fix.action, Some(v1::fix_action::Action::AddRule(rule)) if rule.action == v1::RuleAction::Ask as i32),
        "the fix is an own `ask` rule in front of the bundled one"
    );
    assert_eq!(daemon.store.list().len(), 1);

    daemon.shutdown().await;
}

#[tokio::test]
async fn dry_run_hits() {
    let mut daemon = Daemon::start(&[]).await;

    for index in 0..20 {
        let host = if index % 4 == 0 {
            "api.github.com"
        } else {
            "registry.npmjs.org"
        };
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(HostName::parse(host).expect("host"), Scheme::Https),
            "/x",
        );
        daemon.recorder.apply(&FlowEvent::Received {
            flow_id: FlowId::new(),
            at: SystemTime::now(),
            request: Box::new(request),
        });
    }
    daemon.recorder.flush().await;

    let response = daemon
        .rules(v1::rules_request::Op::DryRun(v1::rules_request::DryRun {
            rule: Some(wire_rule("api.github.com", v1::RuleAction::Block)),
            limit: 0,
        }))
        .await
        .expect("dry run");

    assert_eq!(response.dry_run_scanned, 20);
    assert_eq!(response.dry_run_matches.len(), 5);
    assert!(
        response.dry_run_matches.iter().all(|row| row
            .authority
            .as_ref()
            .is_some_and(|a| a.host == "api.github.com")),
        "only the matching host is reported"
    );
    assert!(
        response.rules.is_empty() && daemon.file().is_none(),
        "a dry run changes nothing"
    );

    daemon.shutdown().await;
}

#[tokio::test]
async fn reload_invalid_keeps_old() {
    let mut daemon = Daemon::start(&[]).await;
    daemon
        .rules(v1::rules_request::Op::Add(wire_rule(
            "api.github.com",
            v1::RuleAction::Allow,
        )))
        .await
        .expect("add");
    let before = daemon.store.effective();

    std::fs::write(&daemon.rules_path, "version: 2\nrules: []\n").expect("write");
    let response = daemon
        .rules(v1::rules_request::Op::Reload(()))
        .await
        .expect("reload answers, it does not fail");

    assert!(
        response
            .diagnostics
            .iter()
            .any(|d| d.code == "RULES_006" && d.severity == v1::Severity::Error as i32),
        "{:?}",
        response.diagnostics
    );
    assert_eq!(
        response.diagnostic.as_ref().map(|d| d.code.clone()),
        Some("RULES_006".to_owned())
    );
    assert_eq!(response.rules.len(), 1, "the old rules stay in force");
    assert_eq!(daemon.store.effective(), before);

    // Eine gültige Datei wird dagegen übernommen und der Bericht nennt, was
    // sich geändert hat.
    std::fs::write(&daemon.rules_path, "version: 1\nrules: []\n").expect("write");
    let response = daemon
        .rules(v1::rules_request::Op::Reload(()))
        .await
        .expect("reload");
    assert_eq!(
        response.diagnostic.as_ref().map(|d| d.code.clone()),
        Some("RULES_011".to_owned())
    );
    assert!(response.rules.is_empty());

    daemon.shutdown().await;
}

#[tokio::test]
async fn a_change_reaches_the_event_stream_as_rules_changed() {
    use tokio_stream::StreamExt as _;

    let mut daemon = Daemon::start(&[]).await;
    let mut events = daemon
        .client
        .subscribe(v1::SubscribeRequest::default())
        .await
        .expect("subscribe")
        .into_inner();

    daemon
        .rules(v1::rules_request::Op::Add(wire_rule(
            "api.github.com",
            v1::RuleAction::Allow,
        )))
        .await
        .expect("add");

    let event = tokio::time::timeout(Duration::from_secs(5), events.next())
        .await
        .expect("an event arrives")
        .expect("the stream stays open")
        .expect("no status error");
    let Some(v1::flow_event::Event::RulesChanged(changed)) = event.event else {
        panic!("a rule change is announced as RulesChanged, got {event:?}");
    };
    assert_eq!(changed.revision, daemon.store.revision());

    daemon.shutdown().await;
}

#[tokio::test]
async fn a_session_rule_can_be_made_permanent() {
    let mut daemon = Daemon::start(&[]).await;
    let added = daemon
        .rules(v1::rules_request::Op::Add(session_scoped(wire_rule(
            "api.github.com",
            v1::RuleAction::Allow,
        ))))
        .await
        .expect("add");
    let id = added.rules[0].rule_id.clone();

    let response = daemon
        .rules(v1::rules_request::Op::MakePermanent(id.clone()))
        .await
        .expect("make permanent");

    assert_eq!(
        response.rules[0].expires,
        Some(v1::RuleExpiry {
            expiry: Some(v1::rule_expiry::Expiry::Never(()))
        })
    );
    let text = daemon.file().expect("rules.yaml exists");
    assert!(text.contains("api.github.com"), "{text}");

    daemon.shutdown().await;
}

#[tokio::test]
async fn an_unknown_rule_id_is_refused_and_changes_nothing() {
    let mut daemon = Daemon::start(&[]).await;
    let unknown = humanitl_core::RuleId::new().to_string();

    let status = daemon
        .rules(v1::rules_request::Op::Remove(unknown))
        .await
        .expect_err("there is no such rule");
    assert_eq!(status.code(), Code::InvalidArgument);
    assert!(status.message().contains("IPC_005"), "{status}");

    let status = daemon
        .rules(v1::rules_request::Op::Remove("not-a-uuid".to_owned()))
        .await
        .expect_err("that is not even an id");
    assert!(status.message().contains("IPC_005"), "{status}");

    daemon.shutdown().await;
}

#[tokio::test]
async fn the_order_of_the_persistent_rules_survives_a_reorder() {
    let mut daemon = Daemon::start(&[]).await;
    let mut ids = Vec::new();
    for host in ["a.example.com", "b.example.com", "c.example.com"] {
        let response = daemon
            .rules(v1::rules_request::Op::Add(wire_rule(
                host,
                v1::RuleAction::Block,
            )))
            .await
            .expect("add");
        ids.push(
            response
                .rules
                .iter()
                .find(|rule| {
                    rule.matcher
                        .as_ref()
                        .is_some_and(|matcher| matcher.host == host)
                })
                .expect("the rule is listed")
                .rule_id
                .clone(),
        );
    }

    let reversed: Vec<String> = ids.iter().rev().cloned().collect();
    let response = daemon
        .rules(v1::rules_request::Op::Reorder(v1::rules_request::Reorder {
            rule_ids_in_order: reversed.clone(),
        }))
        .await
        .expect("reorder");

    let listed: Vec<String> = response
        .rules
        .iter()
        .map(|rule| rule.rule_id.clone())
        .collect();
    assert_eq!(listed, reversed);
    assert_eq!(
        response
            .rules
            .iter()
            .map(|rule| rule.position)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    // Und die Datei trägt dieselbe Reihenfolge, nicht nur der Speicher.
    let text = daemon.file().expect("rules.yaml exists");
    let order: Vec<usize> = reversed
        .iter()
        .filter_map(|id| text.find(id.as_str()))
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "the file keeps the order of the list: {text}"
    );

    daemon.shutdown().await;
}
