//! Der echte gRPC-Dienst über einen echten Unix-Socket (HUM-018).
//!
//! Diese Tests sprechen den Daemon so an, wie die Oberfläche und die CLI es
//! tun: über [`humanitl_ipc::client`], über den Socket, mit dem Token aus der
//! Token-Datei. Was hier grün ist, ist am fertigen Vertrag grün und nicht nur
//! an einer Rust-Signatur.
//!
//! Kein Proxy und keine Sandbox: die Flows entstehen direkt in Registry und
//! Halte-Warteschlange. Der Weg Client, Proxy, Hold, Entscheidung, Antwort
//! gehört zu HUM-021 (Demo-Skript M1) und den Escape-Tests.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use humanitl_config::{Config, Limits};
use humanitl_core::{
    Authority, BodyRef, Decision, Flow, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
    TransitionInput,
};
use humanitl_ipc::client::Client;
use humanitl_ipc::{IpcServer, auth, client, v1};
use humanitl_proxy::registry::FlowRecord;
use humanitl_proxy::{ConnMeta, FlowRegistry, HoldQueue};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt as _;
use tonic::Code;

/// Ein laufender Daemon auf einem Socket in einem Wegwerf-Verzeichnis.
struct Daemon {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    token_path: PathBuf,
    queue: Arc<HoldQueue>,
    stop: Option<oneshot::Sender<()>>,
    join: JoinHandle<Result<(), humanitl_core::Diagnostic>>,
}

impl Daemon {
    /// Startet den Dienst und wartet, bis Socket und Token da sind.
    async fn start(limits: &Limits) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("daemon.sock");
        let token_path = dir.path().join("token");
        let queue = Arc::new(HoldQueue::with_registry(
            limits,
            Arc::new(FlowRegistry::new(limits)),
        ));
        let config = Config {
            limits: limits.clone(),
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, Some(SessionId::new()));

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
        Self {
            _dir: dir,
            socket,
            token_path,
            queue,
            stop: Some(stop),
            join,
        }
    }

    /// Der Daemon mit den Vorgabewerten.
    async fn new() -> Self {
        Self::start(&Limits::default()).await
    }

    /// Das Token dieses Laufs.
    fn token(&self) -> String {
        auth::read_token(&self.token_path).unwrap()
    }

    /// Ein Client mit dem richtigen Token.
    async fn client(&self) -> Client {
        client::connect_at(&self.socket, &self.token())
            .await
            .unwrap()
    }

    /// Gibt das Signal und wartet, bis der Dienst aufgeräumt hat.
    ///
    /// Der Abbau ist bewusst geordnet (`serve_with_incoming_shutdown`): der
    /// Dienst wartet auf offene Verbindungen. Jeder Test lässt seinen Client
    /// deshalb vorher fallen; bleibt einer offen, läuft hier die Frist ab und
    /// der Test scheitert, statt für immer zu hängen.
    async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        tokio::time::timeout(Duration::from_secs(10), self.join)
            .await
            .expect("the daemon must stop within ten seconds")
            .unwrap()
            .unwrap();
    }
}

/// Wartet, bis eine Datei auftaucht; höchstens fünf Sekunden.
async fn await_file(path: &Path) {
    for _ in 0..500 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{} never appeared", path.display());
}

/// Ein gerade angekommener Flow.
fn received(session: SessionId, host: &str) -> Flow {
    let request = HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(HostName::Dns(host.to_owned()), Scheme::Https),
        "/v1/chat",
    )
    .with_body(BodyRef::detached([0; 32], 11));
    Flow::new(FlowId::new(), session, SystemTime::now(), request)
}

/// Ein analysierter Flow, bereit zum Halten.
fn analyzed(session: SessionId, host: &str) -> Flow {
    let mut flow = received(session, host);
    flow.apply(
        TransitionInput::Analyze { findings: vec![] },
        SystemTime::now(),
    )
    .unwrap();
    flow
}

/// Der Name der Variante eines Wire-Ereignisses.
fn event_name(event: &v1::FlowEvent) -> &'static str {
    use v1::flow_event::Event;

    match event.event.as_ref() {
        Some(Event::Received(_)) => "received",
        Some(Event::Analyzed(_)) => "analyzed",
        Some(Event::Held(_)) => "held",
        Some(Event::Decided(_)) => "decided",
        Some(Event::Forwarded(_)) => "forwarded",
        Some(Event::ResponseHeaders(_)) => "response_headers",
        Some(Event::ResponseChunk(_)) => "response_chunk",
        Some(Event::Recorded(_)) => "recorded",
        Some(Event::TimedOut(_)) => "timed_out",
        Some(Event::Lagged(_)) => "lagged",
        Some(Event::Diagnostic(_)) => "diagnostic",
        Some(Event::RulesChanged(_)) => "rules_changed",
        Some(Event::AgentAsk(_)) => "agent_ask",
        Some(Event::Failed(_)) => "failed",
        None => "none",
    }
}

/// Das nächste Ereignis des Stroms, mit Frist.
async fn next_event(stream: &mut tonic::Streaming<v1::FlowEvent>) -> v1::FlowEvent {
    tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("an event must arrive within a second")
        .expect("the stream must not end")
        .expect("the stream must not fail")
}

#[tokio::test]
async fn get_info_requires_a_token() {
    let daemon = Daemon::new().await;

    // Ohne Kopfzeile.
    let channel = client::channel(&daemon.socket).await.unwrap();
    let mut bare = v1::humanitl_client::HumanitlClient::new(channel);
    let error = bare.get_info(()).await.unwrap_err();
    assert_eq!(error.code(), Code::Unauthenticated);
    let diagnostic = humanitl_ipc::diagnostic_from_status(&error).expect("details");
    assert_eq!(diagnostic.code, "IPC_001");

    // Mit falscher Kopfzeile.
    let mut wrong = client::connect_at(&daemon.socket, "00").await.unwrap();
    assert_eq!(
        wrong.get_info(()).await.unwrap_err().code(),
        Code::Unauthenticated
    );

    // Mit der richtigen.
    let info = daemon
        .client()
        .await
        .get_info(())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.proto_major, humanitl_ipc::PROTO_MAJOR);
    assert!(!info.session_id.is_empty());

    drop(bare);
    drop(wrong);
    daemon.shutdown().await;
}

#[tokio::test]
async fn socket_and_token_are_private_to_the_user() {
    let daemon = Daemon::new().await;

    assert_eq!(auth::file_mode(&daemon.socket).unwrap(), 0o600);
    assert_eq!(auth::file_mode(&daemon.token_path).unwrap(), 0o600);
    assert_eq!(daemon.token().len(), auth::TOKEN_BYTES * 2);

    daemon.shutdown().await;
}

#[tokio::test]
async fn shutdown_removes_socket_and_token() {
    let daemon = Daemon::new().await;
    let socket = daemon.socket.clone();
    let token = daemon.token_path.clone();

    daemon.shutdown().await;

    assert!(!socket.exists(), "the socket must not outlive the daemon");
    assert!(!token.exists(), "a stray token is a key to nothing");
}

#[tokio::test]
async fn subscribe_delivers_received_analyzed_and_held() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;
    let mut stream = client
        .subscribe(v1::SubscribeRequest::default())
        .await
        .unwrap()
        .into_inner();

    let session = SessionId::new();
    let mut flow = received(session, "api.example.com");
    let id = flow.id;
    daemon
        .queue
        .registry()
        .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
    daemon.queue.publish(flow.received_event());
    let analyzed = flow
        .apply(
            TransitionInput::Analyze { findings: vec![] },
            SystemTime::now(),
        )
        .unwrap();
    daemon.queue.publish(analyzed);
    let held = daemon
        .queue
        .hold(&mut flow, Instant::now() + Duration::from_secs(30))
        .unwrap();

    assert_eq!(event_name(&next_event(&mut stream).await), "received");
    assert_eq!(event_name(&next_event(&mut stream).await), "analyzed");
    let event = next_event(&mut stream).await;
    assert_eq!(event_name(&event), "held");
    let Some(v1::flow_event::Event::Held(details)) = event.event else {
        panic!("held");
    };
    assert_eq!(details.flow_id, id.to_string());
    assert!(details.deadline.is_some(), "a held flow shows its deadline");
    assert_eq!(details.queue_count, 1);

    drop(held);
    drop(stream);
    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn decide_allow_wakes_the_held_flow_and_shows_up_in_the_stream() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;
    let mut stream = client
        .subscribe(v1::SubscribeRequest::default())
        .await
        .unwrap()
        .into_inner();

    let session = SessionId::new();
    let mut flow = analyzed(session, "api.example.com");
    let id = flow.id;
    daemon
        .queue
        .registry()
        .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
    let held = daemon
        .queue
        .hold(&mut flow, Instant::now() + Duration::from_secs(30))
        .unwrap();
    assert_eq!(event_name(&next_event(&mut stream).await), "held");

    let request = v1::DecideRequest {
        flow_ids: vec![id.to_string()],
        decision: Some(v1::decide_request::Decision::Allow(())),
        ..v1::DecideRequest::default()
    };
    let (decision, response) = tokio::join!(held, client.decide(request));

    assert_eq!(decision, Decision::Allow);
    let response = response.unwrap().into_inner();
    assert_eq!(response.results.len(), 1);
    assert!(response.results[0].applied);
    assert!(
        response.created_rule.is_none(),
        "remember arrives with HUM-027"
    );

    let event = next_event(&mut stream).await;
    assert_eq!(event_name(&event), "decided");
    let Some(v1::flow_event::Event::Decided(details)) = event.event else {
        panic!("decided");
    };
    assert_eq!(details.kind, v1::DecisionKind::Allow as i32);
    assert_eq!(details.source, v1::DecisionSource::User as i32);

    drop(stream);
    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn decide_block_carries_the_note_to_the_stream() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;
    let mut stream = client
        .subscribe(v1::SubscribeRequest::default())
        .await
        .unwrap()
        .into_inner();

    let session = SessionId::new();
    let mut flow = analyzed(session, "models.dev");
    let id = flow.id;
    daemon
        .queue
        .registry()
        .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
    let held = daemon
        .queue
        .hold(&mut flow, Instant::now() + Duration::from_secs(30))
        .unwrap();
    assert_eq!(event_name(&next_event(&mut stream).await), "held");

    let request = v1::DecideRequest {
        flow_ids: vec![id.to_string()],
        decision: Some(v1::decide_request::Decision::Block(
            v1::decide_request::Block {
                // Der Zeilenumbruch darf den Header nicht erreichen (HUM-072).
                note: "kein Katalog\nbitte".to_owned(),
            },
        )),
        ..v1::DecideRequest::default()
    };
    let (decision, response) = tokio::join!(held, client.decide(request));
    assert!(matches!(decision, Decision::Block { .. }));
    assert!(response.unwrap().into_inner().results[0].applied);

    let event = next_event(&mut stream).await;
    let Some(v1::flow_event::Event::Decided(details)) = event.event else {
        panic!("decided");
    };
    assert_eq!(details.kind, v1::DecisionKind::Block as i32);
    assert_eq!(details.block_reason, v1::BlockReason::User as i32);
    assert_eq!(details.note, "kein Katalog bitte");

    drop(stream);
    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn decide_for_a_flow_that_is_not_held_is_failed_precondition() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;

    let error = client
        .decide(v1::DecideRequest {
            flow_ids: vec![FlowId::new().to_string()],
            decision: Some(v1::decide_request::Decision::Allow(())),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap_err();

    assert_eq!(error.code(), Code::FailedPrecondition);
    let diagnostic = humanitl_ipc::diagnostic_from_status(&error).expect("details");
    assert_eq!(diagnostic.code, "IPC_003");

    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn a_decide_without_a_decision_or_a_flow_id_is_ipc_004() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;

    // Fehlende Entscheidung: wird nie zu `Allow` ergaenzt.
    let error = client
        .decide(v1::DecideRequest {
            flow_ids: vec![FlowId::new().to_string()],
            decision: None,
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    let diagnostic = humanitl_ipc::diagnostic_from_status(&error).expect("details");
    assert_eq!(diagnostic.code, "IPC_004");
    assert!(
        diagnostic.why.contains("without a decision"),
        "{diagnostic:?}"
    );

    // Keine Flow-Id: die Anfrage meint niemanden.
    let error = client
        .decide(v1::DecideRequest {
            flow_ids: Vec::new(),
            decision: Some(v1::decide_request::Decision::Allow(())),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    let diagnostic = humanitl_ipc::diagnostic_from_status(&error).expect("details");
    assert_eq!(diagnostic.code, "IPC_004");

    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn an_edited_request_over_the_body_cap_is_invalid_argument() {
    let limits = Limits {
        hold_body_cap_bytes: 32,
        ..Limits::default()
    };
    let daemon = Daemon::start(&limits).await;
    let mut client = daemon.client().await;

    let error = client
        .decide(v1::DecideRequest {
            flow_ids: vec![FlowId::new().to_string()],
            decision: Some(v1::decide_request::Decision::AllowEdited(
                v1::EditedRequest {
                    method: v1::Method::Post as i32,
                    url: "https://api.example.com/v1/chat".to_owned(),
                    body: vec![b'x'; 33],
                    ..v1::EditedRequest::default()
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap_err();

    assert_eq!(error.code(), Code::InvalidArgument);
    let diagnostic = humanitl_ipc::diagnostic_from_status(&error).expect("details");
    assert_eq!(diagnostic.code, "IPC_004");
    assert!(
        diagnostic.why.contains("hold_body_cap_bytes"),
        "{diagnostic:?}"
    );

    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn an_unreadable_edited_request_is_refused_not_turned_into_an_allow() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;

    let error = client
        .decide(v1::DecideRequest {
            flow_ids: vec![FlowId::new().to_string()],
            decision: Some(v1::decide_request::Decision::AllowEdited(
                v1::EditedRequest {
                    method: v1::Method::Unspecified as i32,
                    url: "https://api.example.com/".to_owned(),
                    ..v1::EditedRequest::default()
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap_err();

    assert_eq!(error.code(), Code::InvalidArgument);
    let diagnostic = humanitl_ipc::diagnostic_from_status(&error).expect("details");
    assert_eq!(diagnostic.code, "IPC_004");
    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn an_edited_request_for_more_than_one_flow_is_refused_per_flow() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;
    let ids = vec![FlowId::new().to_string(), FlowId::new().to_string()];

    // Der Vertrag verlangt hier ein Ergebnis je Flow, keinen Fehler für den
    // ganzen Aufruf; entschieden wird trotzdem nichts.
    let response = client
        .decide(v1::DecideRequest {
            flow_ids: ids.clone(),
            decision: Some(v1::decide_request::Decision::AllowEdited(
                v1::EditedRequest {
                    method: v1::Method::Post as i32,
                    url: "https://api.example.com/v1/chat".to_owned(),
                    ..v1::EditedRequest::default()
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.results.len(), 2);
    for (result, id) in response.results.iter().zip(&ids) {
        assert!(!result.applied, "nothing is decided");
        assert_eq!(&result.flow_id, id);
        assert_eq!(result.diagnostic.as_ref().unwrap().code, "IPC_002");
    }

    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn list_flows_answers_from_the_registry() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;
    let session = SessionId::new();
    for host in ["a.example.com", "b.example.com", "b.example.com"] {
        let flow = analyzed(session, host);
        daemon
            .queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
    }

    let page = client
        .list_flows(v1::ListFlowsRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(page.total, 3);
    assert_eq!(page.flows.len(), 3);
    assert!(page.next_cursor.is_empty());
    assert_eq!(
        page.flows[0].state,
        v1::FlowState::Analyzed as i32,
        "the state comes from the registry, not from a guess"
    );
    assert_eq!(page.flows[0].session_id, session.to_string());

    let filtered = client
        .list_flows(v1::ListFlowsRequest {
            filter: "host:b.example.com".to_owned(),
            ..v1::ListFlowsRequest::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(filtered.total, 2);

    drop(client);
    daemon.shutdown().await;
}

#[tokio::test]
async fn every_other_rpc_says_which_issue_brings_it() {
    let daemon = Daemon::new().await;
    let mut client = daemon.client().await;

    let mut refusals = Vec::new();
    refusals.push(
        client
            .get_flow(v1::FlowRef::default())
            .await
            .map(|_| ())
            .unwrap_err(),
    );
    refusals.push(
        client
            .audit(v1::AuditRequest::default())
            .await
            .map(|_| ())
            .unwrap_err(),
    );
    refusals.push(
        client
            .get_config(v1::GetConfigRequest::default())
            .await
            .map(|_| ())
            .unwrap_err(),
    );
    refusals.push(client.doctor(()).await.map(|_| ()).unwrap_err());
    refusals.push(
        client
            .get_body(v1::BodyRef::default())
            .await
            .map(|_| ())
            .unwrap_err(),
    );

    for error in refusals {
        assert_eq!(error.code(), Code::Unimplemented);
        assert!(error.message().contains("arrives in"), "{error}");
    }

    // `Rules` gibt es seit HUM-027. Dieser Daemon läuft nur ohne
    // Regelspeicher, und das sagt er, statt eine leere Liste zu liefern;
    // `tests/rules_rpc.rs` prüft den Fall mit Speicher.
    let without_store = client
        .rules(v1::RulesRequest {
            op: Some(v1::rules_request::Op::List(())),
        })
        .await
        .map(|_| ())
        .unwrap_err();
    assert_eq!(without_store.code(), Code::InvalidArgument);
    assert!(
        without_store.message().contains("rule store"),
        "{without_store}"
    );

    drop(client);
    daemon.shutdown().await;
}
