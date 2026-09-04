//! Der Fake-Daemon gegen die beiden mitgelieferten Sitzungen.
//!
//! Die Zeit steht: `#[tokio::test(start_paused = true)]` lässt tokio die Uhr
//! selbst vorstellen, sobald keine Aufgabe mehr rechnet. Zwanzig Sekunden
//! Sitzung dauern damit Millisekunden, und nichts wackelt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use humanitl_core::FlowId;
use humanitl_ipc::fake::{FakeDaemon, FakeOptions, Session};
use humanitl_ipc::server_stub::BoxStream;
use humanitl_ipc::{DaemonApi, DaemonService, diagnostic_from_status, v1};
use tokio_stream::StreamExt as _;
use tonic::{Code, Request};

/// Der zweite Flow der Sitzung `mixed.jsonl`, den eine Regel blockt.
const MIXED_BLOCKED: &str = "018f0001-0000-7000-8000-000000020000";
/// Der vierte Flow der Sitzung `mixed.jsonl`, dessen Frist nach 5 s abläuft.
const MIXED_TIMEOUT: &str = "018f0001-0000-7000-8000-000000040000";

/// Der Pfad einer mitgelieferten Sitzung.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/sessions")
        .join(name)
}

/// Ein Fake über einer mitgelieferten Sitzung, noch ohne laufenden Abspieler.
fn daemon(name: &str, options: FakeOptions) -> Arc<FakeDaemon> {
    let session = Session::load(&fixture(name)).expect("fixture parses");
    Arc::new(FakeDaemon::new(session, options))
}

/// Ein Abonnement auf alles, auch auf die Durchreiche.
fn subscribe(daemon: &Arc<FakeDaemon>) -> BoxStream<v1::FlowEvent> {
    daemon.subscribe(v1::SubscribeRequest {
        since_flow_id: String::new(),
        include_passthrough: true,
    })
}

/// Der Kurzname eines Ereignisses, für lesbare Zusicherungen.
fn name(event: &v1::FlowEvent) -> &'static str {
    use v1::flow_event::Event;
    match event.event {
        Some(Event::Received(_)) => "received",
        Some(Event::Analyzed(_)) => "analyzed",
        Some(Event::Held(_)) => "held",
        Some(Event::Decided(_)) => "decided",
        Some(Event::Forwarded(_)) => "forwarded",
        Some(Event::ResponseHeaders(_)) => "response_headers",
        Some(Event::ResponseChunk(_)) => "response_chunk",
        Some(Event::Failed(_)) => "failed",
        Some(Event::Recorded(_)) => "recorded",
        Some(Event::TimedOut(_)) => "timed_out",
        Some(Event::Lagged(_)) => "lagged",
        Some(Event::Diagnostic(_)) => "diagnostic",
        Some(Event::FlowDiagnostic(_)) => "flow_diagnostic",
        Some(Event::RulesChanged(_)) => "rules_changed",
        Some(Event::AgentAsk(_)) => "agent_ask",
        None => "empty",
    }
}

/// Der Flow eines Ereignisses, soweit es einen trägt.
fn flow_of(event: &v1::FlowEvent) -> String {
    use v1::flow_event::Event;
    match &event.event {
        Some(Event::Received(received)) => received
            .summary
            .as_ref()
            .map(|summary| summary.flow_id.clone())
            .unwrap_or_default(),
        Some(Event::Analyzed(analyzed)) => analyzed.flow_id.clone(),
        Some(Event::Held(held)) => held.flow_id.clone(),
        Some(Event::Decided(decided)) => decided.flow_id.clone(),
        Some(Event::ResponseHeaders(headers)) => headers.flow_id.clone(),
        Some(Event::ResponseChunk(chunk)) => chunk.flow_id.clone(),
        Some(
            Event::Forwarded(reference) | Event::Recorded(reference) | Event::TimedOut(reference),
        ) => reference.flow_id.clone(),
        _ => String::new(),
    }
}

/// Sammelt Ereignisse, bis die virtuelle Frist abläuft oder der Strom endet.
async fn collect_for(stream: &mut BoxStream<v1::FlowEvent>, span: Duration) -> Vec<v1::FlowEvent> {
    let deadline = tokio::time::Instant::now() + span;
    let mut events = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, stream.next()).await {
            Ok(Some(event)) => events.push(event),
            Ok(None) | Err(_) => return events,
        }
    }
}

/// Wartet auf das erste Ereignis, das zum Prädikat passt.
async fn wait_for(
    stream: &mut BoxStream<v1::FlowEvent>,
    span: Duration,
    predicate: impl Fn(&v1::FlowEvent) -> bool,
) -> Option<v1::FlowEvent> {
    let deadline = tokio::time::Instant::now() + span;
    loop {
        match tokio::time::timeout_at(deadline, stream.next()).await {
            Ok(Some(event)) if predicate(&event) => return Some(event),
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => return None,
        }
    }
}

#[tokio::test(start_paused = true)]
async fn plays_npm_session_in_order() {
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let events = collect_for(&mut stream, Duration::from_secs(20)).await;
    let held: Vec<String> = events
        .iter()
        .filter(|event| name(event) == "held")
        .map(flow_of)
        .collect();

    assert_eq!(
        held.len(),
        15,
        "fifteen flows are held within twenty seconds"
    );
    let ids: Vec<FlowId> = held
        .iter()
        .map(|text| FlowId::parse(text).expect("flow id"))
        .collect();
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "held flow ids arrive in time order: {ids:?}"
    );
    assert!(
        events.iter().all(|event| name(event) != "lagged"),
        "a reader that keeps up never lags"
    );

    let page = daemon
        .list_flows(v1::ListFlowsRequest::default())
        .await
        .expect("list");
    assert_eq!(page.total, 15);
    assert!(
        page.flows
            .iter()
            .all(|flow| flow.state == v1::FlowState::Held as i32),
        "every flow of the npm session waits for a decision"
    );
}

#[tokio::test(start_paused = true)]
async fn decide_allow_emits_forward_and_response() {
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let held = wait_for(&mut stream, Duration::from_secs(5), |event| {
        name(event) == "held"
    })
    .await
    .expect("a flow is held");
    let flow_id = flow_of(&held);

    let response = daemon
        .decide(v1::DecideRequest {
            flow_ids: vec![flow_id.clone()],
            decision: Some(v1::decide_request::Decision::Allow(())),
            ..v1::DecideRequest::default()
        })
        .await
        .expect("decide");
    assert_eq!(response.results.len(), 1);
    assert!(response.results[0].applied, "{:?}", response.results[0]);

    let events = collect_for(&mut stream, Duration::from_millis(10)).await;
    let order: Vec<&'static str> = events
        .iter()
        .filter(|event| flow_of(event) == flow_id)
        .map(name)
        .filter(|name| {
            matches!(
                *name,
                "decided" | "forwarded" | "response_headers" | "recorded"
            )
        })
        .collect();
    assert_eq!(
        order,
        vec!["decided", "forwarded", "response_headers", "recorded"]
    );

    let id = FlowId::parse(&flow_id).expect("flow id");
    let detail = daemon.get_flow(id).await.expect("detail");
    let summary = detail.summary.expect("summary");
    assert_eq!(summary.state, v1::FlowState::Recorded as i32);
    assert_eq!(summary.decision, v1::DecisionKind::Allow as i32);
    assert_eq!(summary.decision_source, v1::DecisionSource::User as i32);
    assert_eq!(summary.status, 200);
    assert!(summary.response_size > 0, "the fixture ships a body");
}

#[tokio::test(start_paused = true)]
async fn timeout_blocks() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let events = collect_for(&mut stream, Duration::from_secs(15)).await;
    let order: Vec<&'static str> = events
        .iter()
        .filter(|event| flow_of(event) == MIXED_TIMEOUT)
        .map(name)
        .collect();
    assert_eq!(
        order,
        vec!["received", "analyzed", "held", "timed_out", "recorded"],
        "an unanswered hold runs out and is recorded"
    );

    let page = daemon
        .list_flows(v1::ListFlowsRequest {
            include_passthrough: true,
            ..v1::ListFlowsRequest::default()
        })
        .await
        .expect("list");
    let timed_out = page
        .flows
        .iter()
        .find(|flow| flow.flow_id == MIXED_TIMEOUT)
        .expect("the timed out flow is listed");
    assert_eq!(timed_out.decision, v1::DecisionKind::TimedOut as i32);
    assert_eq!(
        timed_out.decision_source,
        v1::DecisionSource::Timeout as i32
    );
    assert_eq!(timed_out.block_reason, v1::BlockReason::Timeout as i32);
    assert_eq!(timed_out.state, v1::FlowState::Recorded as i32);

    let blocked = page
        .flows
        .iter()
        .find(|flow| flow.flow_id == MIXED_BLOCKED)
        .expect("the rule-blocked flow is listed");
    assert_eq!(blocked.decision, v1::DecisionKind::Block as i32);
    assert_eq!(blocked.decision_source, v1::DecisionSource::Rule as i32);
    assert_eq!(blocked.rule_id, humanitl_ipc::fake::BUNDLED_BLOCK_RULE);
}

#[tokio::test(start_paused = true)]
async fn allow_edited_with_two_ids_is_ipc_002() {
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let mut ids = Vec::new();
    while ids.len() < 2 {
        let held = wait_for(&mut stream, Duration::from_secs(5), |event| {
            name(event) == "held"
        })
        .await
        .expect("two flows are held");
        ids.push(flow_of(&held));
    }

    let response = daemon
        .decide(v1::DecideRequest {
            flow_ids: ids.clone(),
            decision: Some(v1::decide_request::Decision::AllowEdited(
                v1::EditedRequest::default(),
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .expect("the call itself succeeds");

    assert_eq!(response.results.len(), 2);
    for result in &response.results {
        assert!(!result.applied, "nothing is decided: {result:?}");
        let diagnostic = result.diagnostic.as_ref().expect("a diagnostic per flow");
        assert_eq!(diagnostic.code, "IPC_002");
        assert!(diagnostic.why.contains('2'), "{}", diagnostic.why);
    }

    for text in &ids {
        let id = FlowId::parse(text).expect("flow id");
        let detail = daemon.get_flow(id).await.expect("detail");
        assert_eq!(
            detail.summary.expect("summary").state,
            v1::FlowState::Held as i32,
            "the flow keeps waiting"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn an_unreadable_edited_request_is_refused_not_allowed() {
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let held = wait_for(&mut stream, Duration::from_secs(5), |event| {
        name(event) == "held"
    })
    .await
    .expect("a flow is held");
    let flow_id = flow_of(&held);

    // Unlesbar heisst: die Methode oder die URL ergeben keine Anfrage. Beide
    // Faelle enden mit `IPC_004` und dem Grund, nie mit einem `Allow`.
    let unreadable = [
        (
            "a url without a scheme",
            v1::EditedRequest {
                method: v1::Method::Get as i32,
                url: "github.com/no-scheme".to_owned(),
                ..v1::EditedRequest::default()
            },
        ),
        (
            "an unspecified method",
            v1::EditedRequest {
                method: v1::Method::Unspecified as i32,
                url: "https://github.com/".to_owned(),
                ..v1::EditedRequest::default()
            },
        ),
    ];
    for (case, edited) in unreadable {
        let response = daemon
            .decide(v1::DecideRequest {
                flow_ids: vec![flow_id.clone()],
                decision: Some(v1::decide_request::Decision::AllowEdited(edited)),
                ..v1::DecideRequest::default()
            })
            .await
            .expect("the call itself succeeds");

        let [result] = response.results.as_slice() else {
            panic!("{case}: one flow, one result: {:?}", response.results);
        };
        assert!(
            !result.applied,
            "{case}: nothing is let through: {result:?}"
        );
        let diagnostic = result.diagnostic.as_ref().expect("a diagnostic");
        assert_eq!(diagnostic.code, "IPC_004", "{case}");
        assert!(
            diagnostic.why.contains("not readable"),
            "{case}: {}",
            diagnostic.why
        );
    }

    let events = collect_for(&mut stream, Duration::from_millis(10)).await;
    assert!(
        !events
            .iter()
            .any(|event| flow_of(event) == flow_id && name(event) == "decided"),
        "no decision was made for the flow"
    );
    let id = FlowId::parse(&flow_id).expect("flow id");
    let detail = daemon.get_flow(id).await.expect("detail");
    assert_eq!(
        detail.summary.expect("summary").state,
        v1::FlowState::Held as i32,
        "the flow keeps waiting for a readable decision"
    );
}

/// Liest einen Body vollstaendig ueber `GetBody`.
async fn read_body(daemon: &Arc<FakeDaemon>, body: v1::BodyRef) -> Vec<u8> {
    let mut chunks = daemon.get_body(body);
    let mut data = Vec::new();
    while let Some(chunk) = chunks.next().await {
        data.extend_from_slice(&chunk.data);
    }
    data
}

#[tokio::test(start_paused = true)]
async fn an_edited_request_carries_its_body_into_the_fake() {
    // `EditedRequest` traegt den Body selbst (nicht nur einen `BodyRef`);
    // der Fake legt ihn ab, zeigt ihn in `FlowDetail.edited_request` als
    // Verweis und liefert ihn ueber `GetBody`. `FlowDetail.body_preview`
    // zeigt weiterhin den urspruenglichen Request-Body.
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let held = wait_for(&mut stream, Duration::from_secs(5), |event| {
        name(event) == "held"
    })
    .await
    .expect("a flow is held");
    let flow_id = flow_of(&held);
    let id = FlowId::parse(&flow_id).expect("flow id");
    let before = daemon.get_flow(id).await.expect("detail before");
    let original = read_body(
        &daemon,
        before.request.expect("request").body.expect("body"),
    )
    .await;
    assert_eq!(before.body_preview, String::from_utf8_lossy(&original));

    let body = b"{\"edited\":true,\"raw\":\"\xff\"}".to_vec();
    let response = daemon
        .decide(v1::DecideRequest {
            flow_ids: vec![flow_id.clone()],
            decision: Some(v1::decide_request::Decision::AllowEdited(
                v1::EditedRequest {
                    method: v1::Method::Put as i32,
                    method_raw: String::new(),
                    url: "https://registry.npmjs.org/-/edited?x=1".to_owned(),
                    headers: vec![v1::Header {
                        name: "content-type".to_owned(),
                        value: b"application/json".to_vec(),
                    }],
                    body: body.clone(),
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .expect("the call itself succeeds");
    let [result] = response.results.as_slice() else {
        panic!("one flow, one result: {:?}", response.results);
    };
    assert!(result.applied, "the edited request is accepted: {result:?}");

    let decided = wait_for(&mut stream, Duration::from_secs(1), |event| {
        flow_of(event) == flow_id && name(event) == "decided"
    })
    .await
    .expect("a decided event follows");
    let Some(v1::flow_event::Event::Decided(decided)) = decided.event else {
        panic!("expected Decided");
    };
    assert_eq!(decided.kind, v1::DecisionKind::AllowEdited as i32);

    let detail = daemon.get_flow(id).await.expect("detail after");
    let summary = detail.summary.expect("summary");
    assert!(summary.edited, "the summary marks the flow as edited");
    assert_eq!(summary.decision, v1::DecisionKind::AllowEdited as i32);

    let edited = detail.edited_request.expect("the edited request is shown");
    assert_eq!(edited.method, v1::Method::Put as i32);
    assert_eq!(edited.path_and_query, "/-/edited?x=1");
    assert_eq!(
        edited.authority.as_ref().map(|a| a.host.as_str()),
        Some("registry.npmjs.org")
    );
    let edited_body = edited
        .body
        .expect("the edited body is referenced, not inlined");
    assert_eq!(edited_body.size, body.len() as u64);
    assert_eq!(edited_body.content_type, "application/json");
    assert_eq!(read_body(&daemon, edited_body).await, body);

    // Die Vorschau bleibt die des Originals; der bearbeitete Body kommt nur
    // ueber `GetBody`.
    assert_eq!(detail.body_preview, String::from_utf8_lossy(&original));
}

/// Eine Sitzung, die ohne den Menschen zu Ende kommt: eine Regel erlaubt,
/// eine Regel blockt, eine Durchreiche läuft durch. Jede Zeile nach der
/// `request`-Zeile muss im zweiten Durchlauf denselben Flow treffen.
const AUTO_SESSION: &str = r#"
{"t_ms":0,"type":"session","session_id":"018f0002-0000-7000-8000-000000000001"}
{"t_ms":100,"type":"request","flow_id":"018f0002-0000-7000-8000-000000010000","method":"GET","host":"registry.npmjs.org","path":"/lodash"}
{"t_ms":120,"type":"findings","flow_id":"018f0002-0000-7000-8000-000000010000","findings":[]}
{"t_ms":140,"type":"auto","flow_id":"018f0002-0000-7000-8000-000000010000","source":"rule","rule_id":"018f0000-0000-7000-8000-0000000000a3","kind":"allow"}
{"t_ms":200,"type":"response","flow_id":"018f0002-0000-7000-8000-000000010000","status":200,"headers":[["content-type","application/json"]],"body":"{}"}
{"t_ms":300,"type":"request","flow_id":"018f0002-0000-7000-8000-000000020000","method":"GET","host":"models.dev","path":"/api.json"}
{"t_ms":320,"type":"auto","flow_id":"018f0002-0000-7000-8000-000000020000","source":"rule","rule_id":"018f0000-0000-7000-8000-0000000000a1","kind":"block","note":"local catalog"}
{"t_ms":400,"type":"passthrough","flow_id":"018f0002-0000-7000-8000-000000030000","method":"POST","host":"192.168.1.50","port":11434,"path":"/api/chat","body":"{}","response_status":200,"response_body":"{}"}
"#;

#[tokio::test(start_paused = true)]
async fn the_second_loop_pass_replays_every_flow_to_recorded() {
    let session = Session::parse(AUTO_SESSION).expect("the session parses");
    let file_ids: Vec<String> = session
        .flow_id_texts()
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(file_ids.len(), 3);
    let daemon = Arc::new(FakeDaemon::new(
        session,
        FakeOptions {
            repeat: true,
            ..FakeOptions::default()
        },
    ));
    let mut stream = subscribe(&daemon);
    daemon.start();

    // Erster Durchlauf bis 400 ms, 500 ms Pause, zweiter Durchlauf bis
    // 1300 ms; der dritte begänne bei 1800 ms. Bis dahin ist Schluss, damit
    // nur zwei Durchläufe im Zustand liegen.
    let events = collect_for(&mut stream, Duration::from_millis(1500)).await;
    let received = events
        .iter()
        .filter(|event| name(event) == "received")
        .count();
    assert_eq!(received, 6, "two passes, three flows each");

    let page = daemon
        .list_flows(v1::ListFlowsRequest {
            include_passthrough: true,
            ..v1::ListFlowsRequest::default()
        })
        .await
        .expect("list");
    assert_eq!(page.total, 6);

    let second_pass: Vec<&v1::FlowSummary> = page
        .flows
        .iter()
        .filter(|flow| !file_ids.contains(&flow.flow_id))
        .collect();
    assert_eq!(second_pass.len(), 3, "the second pass has new ids");
    for flow in second_pass {
        assert_eq!(
            flow.state,
            v1::FlowState::Recorded as i32,
            "every flow of the second pass runs to the end: {flow:?}"
        );
        let id = FlowId::parse(&flow.flow_id).expect("flow id");
        assert!(
            file_ids
                .iter()
                .all(|text| FlowId::parse(text).expect("flow id") < id),
            "ids of the second pass sort after the first: {}",
            flow.flow_id
        );
    }
}

#[tokio::test]
async fn invalid_token_is_unauthenticated() {
    use v1::humanitl_server::Humanitl as _;

    let daemon = daemon("mixed.jsonl", FakeOptions::default());
    let service = DaemonService::new(daemon, "the-real-token");

    let mut request = Request::new(());
    request
        .metadata_mut()
        .insert("x-humanitl-token", "not-the-token".parse().expect("ascii"));
    let status = service
        .get_info(request)
        .await
        .expect_err("must be refused");
    assert_eq!(status.code(), Code::Unauthenticated);
    let diagnostic = diagnostic_from_status(&status).expect("details carry the diagnostic");
    assert_eq!(diagnostic.code, "IPC_001");

    let status = service
        .get_info(Request::new(()))
        .await
        .expect_err("a missing token is refused too");
    assert_eq!(status.code(), Code::Unauthenticated);

    let mut request = Request::new(());
    request
        .metadata_mut()
        .insert("x-humanitl-token", "the-real-token".parse().expect("ascii"));
    let info = service
        .get_info(request)
        .await
        .expect("the right token works");
    assert!(info.into_inner().capabilities.contains(&"fake".to_owned()));
}

#[tokio::test]
async fn lagged_when_subscriber_slow() {
    let daemon = daemon(
        "mixed.jsonl",
        FakeOptions {
            event_buffer: 4,
            ..FakeOptions::default()
        },
    );
    let mut stream = subscribe(&daemon);

    let diagnostic = humanitl_core::Diagnostic::builder(
        humanitl_core::diagnostics::codes::TLS_001,
        humanitl_core::Severity::Warning,
    )
    .why("the subscriber is not reading")
    .build();
    for _ in 0..20 {
        daemon
            .state()
            .emit_diagnostic(&diagnostic, std::time::SystemTime::now());
    }

    let event = stream.next().await.expect("the stream stays open");
    let v1::flow_event::Event::Lagged(lagged) = event.event.expect("an event") else {
        panic!("a slow subscriber is told what it missed, not disconnected");
    };
    assert!(lagged.dropped > 0, "{lagged:?}");

    let next = stream
        .next()
        .await
        .expect("the stream continues after a lag");
    assert_eq!(name(&next), "diagnostic");
}

#[tokio::test]
async fn get_body_chunks_and_answers_an_empty_body() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());

    let empty = humanitl_core::http::BodyRef::empty();
    let chunks: Vec<v1::BodyChunk> = daemon
        .get_body(v1::BodyRef {
            sha256: empty.sha256.to_vec(),
            size: 0,
            truncated: false,
            content_type: String::new(),
        })
        .collect()
        .await;
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].last);
    assert!(chunks[0].data.is_empty());

    let unknown: Vec<v1::BodyChunk> = daemon.get_body(v1::BodyRef::default()).collect().await;
    assert_eq!(unknown.len(), 1, "an unknown body is empty, not an error");
}

/// Der Fake hat nichts gemessen und sagt das auch: die Latenz ist 0, jedes
/// Modell trägt den Vermerk, und die Domain-Anzeige kennt keinen Rang. Was
/// aus der Datei kommt (Endpunkt, Apex) bleibt.
#[tokio::test(start_paused = true)]
async fn discover_and_domain_info_do_not_look_measured() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());

    let found: Vec<v1::DiscoverResult> = daemon
        .discover_llm(v1::DiscoverRequest::default())
        .collect()
        .await;
    let [result] = found.as_slice() else {
        panic!("one endpoint from the session header, got {found:?}");
    };
    assert_eq!(
        result.host, "192.168.1.50",
        "the host comes from the fixture"
    );
    assert_eq!(result.port, 11434);
    assert_eq!(result.latency_ms, 0, "no latency was measured");
    assert!(!result.models.is_empty());
    for model in &result.models {
        assert!(
            model.starts_with("fake daemon: nothing was measured"),
            "a fake model must not look like a listed one: {model}"
        );
    }

    let mut stream = subscribe(&daemon);
    daemon.start();
    let received = wait_for(&mut stream, Duration::from_secs(5), |event| {
        name(event) == "received"
    })
    .await
    .expect("the first flow arrives");
    let id = FlowId::parse(&flow_of(&received)).expect("flow id");
    let detail = daemon.get_flow(id).await.expect("detail");
    let domain = detail.domain.expect("domain info");
    assert_eq!(
        domain.apex, "github.com",
        "the apex derives from the fixture host"
    );
    assert_eq!(domain.tranco_rank, 0, "no rank was looked up");
    assert!(domain.catalog_id.is_empty(), "no catalog entry exists");
}

/// Eine Fixture-Zeile mit kaputtem `body_b64` hält den Start an: `CONFIG_001`
/// mit Zeile und Feld, statt eines stillen leeren Bodys.
#[test]
fn a_malformed_body_b64_line_in_a_fixture_is_config_001() {
    let mut text = std::fs::read_to_string(fixture("mixed.jsonl")).expect("fixture reads");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let bad_line = text.lines().count() + 1;
    text.push_str(
        r#"{"t_ms":99000,"type":"request","flow_id":"018f0001-0000-7000-8000-0000000ff000","method":"POST","host":"api.github.com","path":"/graphql","body_b64":"%%not-base64%%"}"#,
    );
    let dir = std::env::temp_dir().join(format!("humanitl-fake-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("broken.jsonl");
    std::fs::write(&path, text).expect("write");

    let error = Session::load(&path).expect_err("the broken line is refused");
    let diagnostic = error.diagnostic();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(diagnostic.code.as_str(), "CONFIG_001");
    assert!(
        diagnostic.why.contains(&format!(
            "session line {bad_line}: body_b64 is not valid base64"
        )),
        "{}",
        diagnostic.why
    );
}

#[test]
fn shipped_fixtures_use_uuid_v7_ids() {
    for name in ["npm-install.jsonl", "mixed.jsonl"] {
        let session = Session::load(&fixture(name)).expect("fixture parses");
        let ids = session.flow_id_texts();
        assert!(!ids.is_empty(), "{name} has flows");
        for text in ids {
            let id = FlowId::parse(text).unwrap_or_else(|err| panic!("{name}: {err}"));
            assert_eq!(
                id.as_uuid().get_version_num(),
                7,
                "{name}: {text} is not a UUIDv7"
            );
        }
        let meta = session.meta().expect("every fixture names its session");
        assert_eq!(meta.id.as_uuid().get_version_num(), 7);
    }
}

/// Der Fake über einem echten Unix-Socket, so wie `humanitld --fake` ihn öffnet.
///
/// Das ist der Weg, den die Oberfläche geht: Socket, Token, `GetInfo`,
/// `Subscribe`. Der Test läuft ohne angehaltene Uhr, weil er echtes IO macht;
/// damit er trotzdem schnell bleibt, wird die Sitzung fünfzigfach gerafft.
#[tokio::test]
async fn serves_the_contract_over_a_unix_socket() {
    use tonic::transport::{Endpoint, Server};

    let dir = std::env::temp_dir().join(format!("humanitl-fake-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let socket = dir.join("daemon.sock");

    let daemon = daemon(
        "npm-install.jsonl",
        FakeOptions {
            speed: 50.0,
            ..FakeOptions::default()
        },
    );
    let service = v1::humanitl_server::HumanitlServer::new(DaemonService::new(
        Arc::clone(&daemon),
        "token-42",
    ));

    let listener = tokio::net::UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(service)
            .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(listener))
            .await;
    });

    let channel = Endpoint::from_shared(format!("unix:{}", socket.display()))
        .expect("endpoint")
        .connect()
        .await
        .expect("connect");
    let mut client = v1::humanitl_client::HumanitlClient::new(channel);

    let info = client
        .get_info(with_token(()))
        .await
        .expect("GetInfo over the socket")
        .into_inner();
    assert_eq!(info.proto_major, 1);
    assert!(info.capabilities.contains(&"fake".to_owned()));

    let mut stream = client
        .subscribe(with_token(v1::SubscribeRequest {
            include_passthrough: true,
            ..v1::SubscribeRequest::default()
        }))
        .await
        .expect("Subscribe over the socket")
        .into_inner();
    daemon.start();

    let mut names = Vec::new();
    let mut held = 0usize;
    while held < 15 {
        let event = tokio::time::timeout(Duration::from_secs(20), stream.message())
            .await
            .expect("events keep arriving")
            .expect("no transport error")
            .expect("the stream stays open");
        if name(&event) == "held" {
            held += 1;
        }
        names.push(name(&event));
    }
    assert_eq!(names.first().copied(), Some("received"));
    assert_eq!(names.iter().filter(|name| **name == "received").count(), 15);
    assert_eq!(names.iter().filter(|name| **name == "analyzed").count(), 15);

    let refused = v1::humanitl_client::HumanitlClient::new(
        Endpoint::from_shared(format!("unix:{}", socket.display()))
            .expect("endpoint")
            .connect()
            .await
            .expect("connect"),
    )
    .get_info(Request::new(()))
    .await
    .expect_err("without a token nothing is served");
    assert_eq!(refused.code(), Code::Unauthenticated);

    server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Baut eine Anfrage mit dem Token, das der Testdienst erwartet.
fn with_token<T>(message: T) -> Request<T> {
    let mut request = Request::new(message);
    request
        .metadata_mut()
        .insert("x-humanitl-token", "token-42".parse().expect("ascii"));
    request
}

#[tokio::test]
async fn config_answers_and_keeps_accepted_overrides() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());

    let snapshot = daemon
        .get_config(v1::GetConfigRequest {
            include_schema: true,
        })
        .await
        .expect("config");
    assert!(snapshot.toml.contains("[hold]"), "{}", snapshot.toml);
    assert!(snapshot.json_schema.contains("hold"));
    assert!(
        snapshot
            .origins
            .iter()
            .any(|origin| origin.key == "hold.timeout_secs" && origin.origin == "default")
    );

    let changed = daemon
        .set_config(v1::SetConfigRequest {
            key: "hold.timeout_secs".to_owned(),
            value: "42".to_owned(),
        })
        .await
        .expect("set");
    assert!(
        changed.toml.contains("timeout_secs = 42"),
        "{}",
        changed.toml
    );
    assert!(
        changed
            .origins
            .iter()
            .any(|origin| origin.key == "hold.timeout_secs" && origin.origin == "cli")
    );

    let refused = daemon
        .set_config(v1::SetConfigRequest {
            key: "hold.no_such_key".to_owned(),
            value: "1".to_owned(),
        })
        .await
        .expect_err("an unknown key is refused");
    assert!(refused.code.as_str().starts_with("CONFIG_"), "{refused}");
    let still = daemon
        .get_config(v1::GetConfigRequest::default())
        .await
        .expect("config");
    assert!(
        still.toml.contains("timeout_secs = 42"),
        "the refusal changed nothing"
    );
}

#[tokio::test]
async fn rules_list_the_bundled_ones_and_accept_a_new_one() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());

    let rules = daemon
        .rules(v1::RulesRequest {
            op: Some(v1::rules_request::Op::List(())),
        })
        .await
        .expect("rules");
    assert_eq!(rules.rules.len(), 2, "the fake ships two bundled rules");
    assert!(rules.rules.iter().all(|rule| rule.bundled));

    let added = daemon
        .rules(v1::RulesRequest {
            op: Some(v1::rules_request::Op::Add(v1::Rule {
                action: v1::RuleAction::Allow as i32,
                matcher: Some(v1::RuleMatcher {
                    host: "**.npmjs.org".to_owned(),
                    ..v1::RuleMatcher::default()
                }),
                ..v1::Rule::default()
            })),
        })
        .await
        .expect("add");
    assert_eq!(added.rules.len(), 3);
    let new_rule = added.rules.last().expect("the new rule");
    assert!(!new_rule.rule_id.is_empty(), "the daemon hands out the id");
    // Positionen zählen ab eins (`proto/humanitl/v1/rules.proto`,
    // `Rule.position`); die dritte Regel steht also auf Platz 3.
    assert_eq!(new_rule.position, 3);
    assert_eq!(
        added.rules.first().expect("the first rule").position,
        1,
        "the first rule is at position 1, not 0"
    );
}

#[tokio::test]
async fn sandbox_checks_and_argv_are_marked_as_fake() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());

    let events: Vec<v1::SandboxEvent> = daemon
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::IsolationCheck(())),
        })
        .collect()
        .await;
    assert_eq!(events.len(), 3, "three guarantees, three checks");
    for event in &events {
        let Some(v1::sandbox_event::Event::Check(check)) = &event.event else {
            panic!("expected a check, got {event:?}");
        };
        assert!(check.passed, "the fake is always green: {check:?}");
        assert!(
            check
                .evidence
                .starts_with("fake daemon: nothing was measured (would run: "),
            "fake evidence must not look like a measurement: {}",
            check.evidence
        );
    }

    let argv: Vec<v1::SandboxEvent> = daemon
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Argv(())),
        })
        .collect()
        .await;
    assert!(!argv.is_empty());
    for event in &argv {
        let Some(v1::sandbox_event::Event::ArgvLine(line)) = &event.event else {
            panic!("expected an argv line, got {event:?}");
        };
        assert!(
            line.starts_with("fake daemon: nothing was measured (would run: "),
            "{line}"
        );
    }
}

#[tokio::test]
async fn terminal_echoes_and_audit_is_empty() {
    let daemon = daemon("mixed.jsonl", FakeOptions::default());

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut output = daemon.terminal(Box::pin(
        tokio_stream::wrappers::UnboundedReceiverStream::new(rx),
    ));
    tx.send(v1::TerminalInput {
        input: Some(v1::terminal_input::Input::Data(b"echo hi\r".to_vec())),
    })
    .expect("send");
    let echoed = output.next().await.expect("the terminal answers");
    assert_eq!(
        echoed.output,
        Some(v1::terminal_output::Output::Data(b"echo hi\r".to_vec()))
    );

    let audit = daemon
        .audit(v1::AuditRequest {
            op: Some(v1::audit_request::Op::Verify(())),
        })
        .await
        .expect("audit");
    assert!(audit.ok);
    assert_eq!(audit.entries, 0, "the fake records nothing");
}

#[tokio::test(start_paused = true)]
async fn a_block_note_is_sanitised_before_it_reaches_the_flow() {
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();

    let held = wait_for(&mut stream, Duration::from_secs(5), |event| {
        name(event) == "held"
    })
    .await
    .expect("a flow is held");
    let flow_id = flow_of(&held);

    let response = daemon
        .decide(v1::DecideRequest {
            flow_ids: vec![flow_id.clone()],
            decision: Some(v1::decide_request::Decision::Block(
                v1::decide_request::Block {
                    note: "use\npypi\u{200b} instead\r\nX-Injected: 1".to_owned(),
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .expect("decide");
    assert!(response.results[0].applied, "{:?}", response.results[0]);

    let decided = wait_for(&mut stream, Duration::from_secs(1), |event| {
        name(event) == "decided" && flow_of(event) == flow_id
    })
    .await
    .expect("decided event");
    let note = match decided.event {
        Some(v1::flow_event::Event::Decided(decided)) => decided.note,
        other => panic!("unexpected event {other:?}"),
    };
    assert_eq!(note, "use pypi instead X-Injected: 1");
    assert!(!note.contains('\n') && !note.contains('\r') && !note.contains('\u{200b}'));
}

#[tokio::test(start_paused = true)]
async fn descending_pages_continue_before_the_cursor() {
    let daemon = daemon("npm-install.jsonl", FakeOptions::default());
    let mut stream = subscribe(&daemon);
    daemon.start();
    // Let the whole session arrive (15 flows in 20 s of paused time).
    let _ = collect_for(&mut stream, Duration::from_secs(25)).await;

    let first = daemon
        .list_flows(v1::ListFlowsRequest {
            include_passthrough: true,
            limit: 2,
            order_by: "received_at desc".to_owned(),
            ..v1::ListFlowsRequest::default()
        })
        .await
        .expect("first page");
    assert_eq!(first.flows.len(), 2);
    assert!(!first.next_cursor.is_empty(), "more than two flows exist");
    assert!(
        first.flows[0].flow_id > first.flows[1].flow_id,
        "newest first"
    );

    let second = daemon
        .list_flows(v1::ListFlowsRequest {
            include_passthrough: true,
            limit: 2,
            order_by: "received_at desc".to_owned(),
            cursor: first.next_cursor.clone(),
            ..v1::ListFlowsRequest::default()
        })
        .await
        .expect("second page");
    assert_eq!(second.flows.len(), 2, "the second page is not empty");
    assert!(
        second.flows[0].flow_id < first.flows[1].flow_id,
        "the second page continues before the cursor, not after it"
    );
    let overlap = second
        .flows
        .iter()
        .any(|s| first.flows.iter().any(|f| f.flow_id == s.flow_id));
    assert!(!overlap, "pages do not overlap");
}
