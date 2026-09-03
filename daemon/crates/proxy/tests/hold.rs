//! Die Halte-Warteschlange von außen: Entscheidung, Frist, Budget, Ereignisse.
//!
//! Zeitkritische Fälle laufen mit angehaltener Uhr (`start_paused`), damit
//! 1000 Fristen keine 1000 echten Millisekunden kosten; wo die Messung selbst
//! der Test ist (`hold_times_out`), läuft echte Zeit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use humanitl_config::{IpPreference, Limits};
use humanitl_core::diagnostics::codes::PROXY_005;
use humanitl_core::{
    Authority, BlockReason, BodyRef, Decision, DecisionSource, Flow, FlowEvent, FlowId, FlowState,
    HostName, HttpRequest, InvalidTransition, Method, RuleId, Scheme, SessionId, Severity,
    TransitionInput,
};
use humanitl_proxy::ca::{CaStore, LeafCache};
use humanitl_proxy::handler::serve_connection;
use humanitl_proxy::hold::{HoldError, HoldQueue, NotHeld, next_event};
use humanitl_proxy::{
    AsyncStream, ClientTls, ConnMeta, Egress, FlowFilter, FlowHandler, FlowPipeline, FlowRecord,
    FlowRegistry, ProxyLimits, ResolveError, Resolver, Upstream,
};
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

/// Ein Flow in `Analyzed` mit einem Body von `body_bytes` Bytes.
fn analyzed_flow(body_bytes: u64) -> Flow {
    let mut flow = received_flow(FlowId::new(), body_bytes);
    flow.apply(
        TransitionInput::Analyze { findings: vec![] },
        SystemTime::now(),
    )
    .expect("received -> analyzed");
    flow
}

/// Ein Flow in `Received` mit vorgegebener Id.
fn received_flow(id: FlowId, body_bytes: u64) -> Flow {
    let host = HostName::Dns("api.example.com".to_owned());
    let request = HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(host, Scheme::Https),
        "/v1/things",
    )
    .with_body(BodyRef::detached([0; 32], body_bytes));
    Flow::new(id, SessionId::new(), SystemTime::now(), request)
}

fn limits(max_flows: u32, max_bytes: u64, event_buffer: usize) -> Limits {
    Limits {
        hold_max_flows: max_flows,
        hold_max_bytes: max_bytes,
        event_buffer,
        ..Limits::default()
    }
}

fn block(reason: BlockReason) -> Decision {
    Decision::Block { reason, note: None }
}

fn in_(duration: Duration) -> Instant {
    Instant::now() + duration
}

/// Alles, was schon im Kanal liegt, ohne zu warten.
fn drain(rx: &mut broadcast::Receiver<FlowEvent>) -> Vec<FlowEvent> {
    let mut events = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(event) => events.push(event),
            Err(broadcast::error::TryRecvError::Lagged(n)) => events.push(FlowEvent::Lagged { n }),
            Err(_) => break,
        }
    }
    events
}

fn names(events: &[FlowEvent]) -> Vec<&'static str> {
    events.iter().map(FlowEvent::name).collect()
}

/// Wartet, bis `count` Flows gehalten werden.
async fn until_held(queue: &HoldQueue, count: u32) {
    let give_up = Instant::now() + Duration::from_secs(5);
    while queue.queue_count() != count {
        assert!(
            Instant::now() < give_up,
            "only {} held",
            queue.queue_count()
        );
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn hold_resolves_on_decide() {
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let mut flow = received_flow(FlowId::new(), 12);
    let id = flow.id;
    queue.publish(flow.received_event());
    let analyzed = flow
        .apply(
            TransitionInput::Analyze { findings: vec![] },
            SystemTime::now(),
        )
        .unwrap();
    queue.publish(analyzed);

    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();
    assert_eq!(queue.pending_ids(), vec![id]);
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 12));

    let started = Instant::now();
    queue.decide(id, Decision::Allow).unwrap();
    let decision = held.await;
    assert!(started.elapsed() < Duration::from_millis(10));
    assert_eq!(decision, Decision::Allow);
    assert_eq!(flow.state, FlowState::Decided(Decision::Allow));
    assert!(queue.pending_ids().is_empty());
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));

    let events = drain(&mut rx);
    assert_eq!(names(&events), ["received", "analyzed", "held", "decided"]);
    assert!(events.iter().all(|event| event.flow_id() == Some(id)));
    match &events[2] {
        FlowEvent::Held {
            queue_bytes,
            queue_count,
            ..
        } => assert_eq!((*queue_bytes, *queue_count), (12, 1)),
        other => panic!("expected Held, got {other:?}"),
    }
    match &events[3] {
        FlowEvent::Decided {
            decision, source, ..
        } => {
            assert_eq!(decision, &Decision::Allow);
            assert_eq!(source, &DecisionSource::User);
        }
        other => panic!("expected Decided, got {other:?}"),
    }
}

#[tokio::test]
async fn hold_times_out() {
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let mut flow = analyzed_flow(0);
    let id = flow.id;

    let started = Instant::now();
    let held = queue
        .hold(&mut flow, in_(Duration::from_millis(200)))
        .unwrap();
    let decision = held.await;
    let elapsed = started.elapsed();

    assert_eq!(decision, Decision::TimedOut);
    assert!(
        elapsed >= Duration::from_millis(200) && elapsed < Duration::from_secs(1),
        "timed out after {elapsed:?}"
    );
    assert_eq!(decision.http_status(), Some(504), "timeout answers 504");
    assert_eq!(BlockReason::Timeout.http_status(), 504);
    assert_eq!(flow.state, FlowState::Decided(Decision::TimedOut));
    assert_eq!(names(&drain(&mut rx)), ["held", "timed_out"]);
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
    assert_eq!(
        queue.decide(id, Decision::Allow),
        Err(NotHeld::Unknown { id }),
        "after the timeout a decision comes too late"
    );
}

#[tokio::test]
async fn past_deadline_times_out_immediately() {
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(0);
    let started = Instant::now();
    let decision = queue
        .hold(
            &mut flow,
            Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
        )
        .unwrap()
        .await;
    assert_eq!(decision, Decision::TimedOut);
    assert!(started.elapsed() < Duration::from_millis(50));
    assert_eq!(queue.queue_count(), 0);
}

#[tokio::test]
async fn decide_unknown_is_error() {
    let queue = HoldQueue::new(&Limits::default());
    let id = FlowId::new();
    let err = queue.decide(id, Decision::Allow).unwrap_err();
    assert_eq!(err, NotHeld::Unknown { id });
    assert_eq!(err.id(), id);
    assert!(err.to_string().contains(&id.to_string()));
    assert_eq!(
        queue.extend(id, Duration::from_secs(1)),
        Err(NotHeld::Unknown { id })
    );
}

#[tokio::test]
async fn decide_twice_is_error() {
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();
    queue.decide(id, Decision::Allow).unwrap();
    assert_eq!(
        queue.decide(id, block(BlockReason::User)),
        Err(NotHeld::Unknown { id })
    );
    assert_eq!(held.await, Decision::Allow, "the first decision stands");
}

#[tokio::test]
async fn extend_moves_deadline() {
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    let deadline = in_(Duration::from_millis(200));
    let held = queue.hold(&mut flow, deadline).unwrap();

    let moved = queue.extend(id, Duration::from_secs(1)).unwrap();
    assert_eq!(moved, deadline + Duration::from_secs(1));
    assert_eq!(queue.deadline(id), Some(moved));

    let started = Instant::now();
    let late = async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        queue.decide(id, Decision::Allow)
    };
    let (decision, decided) = tokio::join!(held, late);
    decided.expect("still held after the original deadline");
    assert_eq!(decision, Decision::Allow);
    assert!(started.elapsed() >= Duration::from_millis(500));
    assert_eq!(queue.deadline(id), None);
}

#[tokio::test(start_paused = true)]
async fn timeout_never_allows() {
    const FLOWS: u32 = 1000;
    let flows = usize::try_from(FLOWS).unwrap();
    let queue = Arc::new(HoldQueue::new(&limits(FLOWS, u64::MAX, 4096)));
    let mut rx = queue.subscribe();
    let deadline = in_(Duration::from_millis(50));

    let mut tasks = JoinSet::new();
    for _ in 0..FLOWS {
        let queue = Arc::clone(&queue);
        tasks.spawn(async move {
            let mut flow = analyzed_flow(1);
            let decision = queue.hold(&mut flow, deadline).unwrap().await;
            (decision, flow.state)
        });
    }

    let mut timed_out = 0;
    let mut allowed = 0;
    while let Some(result) = tasks.join_next().await {
        let (decision, state) = result.unwrap();
        match decision {
            Decision::TimedOut => timed_out += 1,
            Decision::Allow | Decision::AllowEdited { .. } => allowed += 1,
            Decision::Block { .. } => panic!("no budget refusal expected"),
        }
        assert_eq!(state, FlowState::Decided(Decision::TimedOut));
    }
    assert_eq!((timed_out, allowed), (flows, 0));
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
    assert!(queue.pending_ids().is_empty());

    let events = drain(&mut rx);
    let held = events.iter().filter(|e| e.name() == "held").count();
    let expired = events.iter().filter(|e| e.name() == "timed_out").count();
    assert_eq!((held, expired), (flows, flows));
    assert!(!events.iter().any(|e| e.name() == "decided"));
}

#[tokio::test]
async fn budget_refuses_by_flows_and_never_evicts() {
    let queue = HoldQueue::new(&limits(2, u64::MAX, 64));
    let mut rx = queue.subscribe();
    let mut first = analyzed_flow(0);
    let mut second = analyzed_flow(0);
    let mut third = analyzed_flow(0);
    let (first_id, second_id, third_id) = (first.id, second.id, third.id);
    let far = in_(Duration::from_secs(60));

    let held_first = queue.hold(&mut first, far).unwrap();
    let held_second = queue.hold(&mut second, far).unwrap();
    assert_eq!(queue.queue_count(), 2);
    drain(&mut rx);

    let refused = queue.hold(&mut third, far).unwrap();
    let decision = refused.await;
    assert_eq!(decision, block(BlockReason::HoldMaxFlows));
    assert_eq!(decision.http_status(), Some(503));
    assert_eq!(
        third.state,
        FlowState::Decided(block(BlockReason::HoldMaxFlows))
    );

    let events = drain(&mut rx);
    assert_eq!(names(&events), ["decided"], "a refused flow is never Held");
    match &events[0] {
        FlowEvent::Decided {
            flow_id, source, ..
        } => {
            assert_eq!(*flow_id, third_id);
            assert_eq!(*source, DecisionSource::System);
        }
        other => panic!("expected Decided, got {other:?}"),
    }

    assert_eq!(queue.queue_count(), 2, "the held flows stay held");
    assert_eq!(queue.pending_ids().len(), 2);
    assert_eq!(
        queue.decide(third_id, Decision::Allow),
        Err(NotHeld::Unknown { id: third_id })
    );

    queue.decide(first_id, Decision::Allow).unwrap();
    assert_eq!(held_first.await, Decision::Allow);
    assert_eq!(queue.queue_count(), 1, "a decision frees its slot");

    let mut fourth = analyzed_flow(0);
    let fourth_id = fourth.id;
    let held_fourth = queue.hold(&mut fourth, far).unwrap();
    assert_eq!(queue.queue_count(), 2);
    queue.decide(fourth_id, block(BlockReason::User)).unwrap();
    queue.decide(second_id, block(BlockReason::User)).unwrap();
    assert_eq!(held_fourth.await, block(BlockReason::User));
    assert_eq!(held_second.await, block(BlockReason::User));
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
}

#[tokio::test]
async fn budget_refuses_by_bytes() {
    let queue = HoldQueue::new(&limits(10, 1024, 64));
    let mut rx = queue.subscribe();
    let far = in_(Duration::from_secs(60));

    let mut big = analyzed_flow(600);
    let big_id = big.id;
    let held_big = queue.hold(&mut big, far).unwrap();
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 600));

    let mut too_big = analyzed_flow(600);
    let decision = queue.hold(&mut too_big, far).unwrap().await;
    assert_eq!(decision, block(BlockReason::HoldMemory));
    assert_eq!(decision.http_status(), Some(503));
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 600));

    let mut fits = analyzed_flow(424);
    let fits_id = fits.id;
    let held_fits = queue.hold(&mut fits, far).unwrap();
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (2, 1024));

    let mut one_over = analyzed_flow(1);
    assert_eq!(
        queue.hold(&mut one_over, far).unwrap().await,
        block(BlockReason::HoldMemory)
    );
    let mut empty = analyzed_flow(0);
    let empty_id = empty.id;
    let held_empty = queue.hold(&mut empty, far).unwrap();
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (3, 1024));

    let events = drain(&mut rx);
    assert_eq!(
        names(&events),
        ["held", "decided", "held", "decided", "held"]
    );

    queue.decide(big_id, Decision::Allow).unwrap();
    assert_eq!(held_big.await, Decision::Allow);
    queue.decide(fits_id, Decision::Allow).unwrap();
    assert_eq!(held_fits.await, Decision::Allow);
    queue.decide(empty_id, Decision::Allow).unwrap();
    assert_eq!(held_empty.await, Decision::Allow);
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
}

#[tokio::test]
async fn held_event_carries_the_queue_totals() {
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let far = in_(Duration::from_secs(60));
    let mut first = analyzed_flow(100);
    let mut second = analyzed_flow(50);
    let held_first = queue.hold(&mut first, far).unwrap();
    let held_second = queue.hold(&mut second, far).unwrap();

    let totals: Vec<(u64, u32)> = drain(&mut rx)
        .into_iter()
        .filter_map(|event| match event {
            FlowEvent::Held {
                queue_bytes,
                queue_count,
                ..
            } => Some((queue_bytes, queue_count)),
            _ => None,
        })
        .collect();
    assert_eq!(totals, [(100, 1), (150, 2)]);
    assert_eq!(queue.queue_bytes(), 150);

    drop(held_first);
    drop(held_second);
}

#[tokio::test]
async fn dropped_hold_ends_as_client_timeout_and_frees_the_budget() {
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let mut flow = analyzed_flow(77);
    let id = flow.id;

    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 77));
    drop(held);

    assert_eq!(
        flow.state,
        FlowState::Decided(block(BlockReason::ClientTimeout))
    );
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
    assert!(queue.pending_ids().is_empty());
    assert_eq!(
        queue.decide(id, Decision::Allow),
        Err(NotHeld::Unknown { id })
    );

    let events = drain(&mut rx);
    assert_eq!(names(&events), ["held", "decided"]);
    match &events[1] {
        FlowEvent::Decided {
            decision, source, ..
        } => {
            assert_eq!(decision, &block(BlockReason::ClientTimeout));
            assert_eq!(source, &DecisionSource::System);
            assert_eq!(decision.http_status(), Some(408));
        }
        other => panic!("expected Decided, got {other:?}"),
    }
}

#[tokio::test]
async fn a_decision_that_arrives_after_the_client_left_is_discarded() {
    // Die Entscheidung landet im Kanal, aber das Future wird danach nie
    // gepollt, sondern fallen gelassen: der Client ist weg, der Flow endet
    // als ClientTimeout, nicht als Allow.
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();
    queue.decide(id, Decision::Allow).unwrap();
    drop(held);
    assert_eq!(
        flow.state,
        FlowState::Decided(block(BlockReason::ClientTimeout))
    );
    assert_eq!(queue.queue_count(), 0);
}

#[tokio::test]
async fn timeout_from_outside_is_forbidden() {
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();

    let err = queue.decide(id, Decision::TimedOut).unwrap_err();
    assert_eq!(
        err,
        NotHeld::Forbidden {
            id,
            decision: "timed_out",
            by: DecisionSource::User,
        }
    );
    assert_eq!(err.id(), id);
    assert!(err.to_string().contains("timed_out"), "{err}");
    assert_eq!(queue.pending_ids(), vec![id], "still held");

    assert_eq!(
        queue.decide_as(id, Decision::Allow, DecisionSource::System),
        Err(NotHeld::Forbidden {
            id,
            decision: "allow",
            by: DecisionSource::System,
        }),
        "the daemon itself may never allow"
    );
    assert_eq!(
        queue.decide_as(id, Decision::TimedOut, DecisionSource::Timeout),
        Err(NotHeld::Forbidden {
            id,
            decision: "timed_out",
            by: DecisionSource::Timeout,
        })
    );

    queue.decide(id, Decision::Allow).unwrap();
    assert_eq!(held.await, Decision::Allow);
}

#[tokio::test]
async fn decision_source_travels_with_the_event() {
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let rule = RuleId::new();
    let far = in_(Duration::from_secs(60));

    let mut by_rule = analyzed_flow(0);
    let by_rule_id = by_rule.id;
    let held = queue.hold(&mut by_rule, far).unwrap();
    queue
        .decide_as(
            by_rule_id,
            block(BlockReason::Rule(rule)),
            DecisionSource::Rule(rule),
        )
        .unwrap();
    assert_eq!(held.await, block(BlockReason::Rule(rule)));

    let mut by_system = analyzed_flow(0);
    let by_system_id = by_system.id;
    let held = queue.hold(&mut by_system, far).unwrap();
    queue
        .decide_as(
            by_system_id,
            block(BlockReason::ClientTimeout),
            DecisionSource::System,
        )
        .unwrap();
    assert_eq!(held.await, block(BlockReason::ClientTimeout));

    let sources: Vec<DecisionSource> = drain(&mut rx)
        .into_iter()
        .filter_map(|event| match event {
            FlowEvent::Decided { source, .. } => Some(source),
            _ => None,
        })
        .collect();
    assert_eq!(
        sources,
        [DecisionSource::Rule(rule), DecisionSource::System]
    );
}

#[tokio::test]
async fn hold_needs_an_analyzed_flow() {
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let mut flow = received_flow(FlowId::new(), 5);
    let err = queue
        .hold(&mut flow, in_(Duration::from_secs(60)))
        .err()
        .expect("received cannot be held");
    assert_eq!(
        err,
        HoldError::InvalidTransition(InvalidTransition {
            from: "received",
            input: "hold",
        })
    );
    assert_eq!(flow.state, FlowState::Received, "the flow is untouched");
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
    assert!(drain(&mut rx).is_empty(), "nothing is published");

    // Auch die Budget-Ablehnung braucht `Analyzed`: sie ist ein Übergang.
    let tight = HoldQueue::new(&limits(1, u64::MAX, 8));
    let mut occupant = analyzed_flow(0);
    let held = tight
        .hold(&mut occupant, in_(Duration::from_secs(60)))
        .unwrap();
    let mut raw = received_flow(FlowId::new(), 0);
    assert!(matches!(
        tight.hold(&mut raw, in_(Duration::from_secs(60))).err(),
        Some(HoldError::InvalidTransition(_))
    ));
    assert_eq!(raw.state, FlowState::Received);
    drop(held);
}

#[tokio::test]
async fn the_same_id_cannot_be_held_twice() {
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(3);
    let id = flow.id;
    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();

    let mut twin = received_flow(id, 3);
    twin.apply(
        TransitionInput::Analyze { findings: vec![] },
        SystemTime::now(),
    )
    .unwrap();
    let err = queue
        .hold(&mut twin, in_(Duration::from_secs(60)))
        .err()
        .expect("second hold of the same id");
    assert_eq!(err, HoldError::AlreadyHeld { id });
    assert!(matches!(twin.state, FlowState::Analyzed { .. }));
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 3));

    queue.decide(id, Decision::Allow).unwrap();
    assert_eq!(held.await, Decision::Allow, "the first hold is unharmed");
}

#[tokio::test]
async fn pending_ids_are_sorted_by_deadline() {
    let queue = HoldQueue::new(&Limits::default());
    let now = Instant::now();
    let mut a = analyzed_flow(0);
    let mut b = analyzed_flow(0);
    let mut c = analyzed_flow(0);
    let (a_id, b_id, c_id) = (a.id, b.id, c.id);
    let held_a = queue.hold(&mut a, now + Duration::from_secs(3)).unwrap();
    let held_b = queue.hold(&mut b, now + Duration::from_secs(1)).unwrap();
    let held_c = queue.hold(&mut c, now + Duration::from_secs(2)).unwrap();

    assert_eq!(queue.pending_ids(), vec![b_id, c_id, a_id]);
    assert_eq!(queue.deadline(b_id), Some(now + Duration::from_secs(1)));

    queue.extend(b_id, Duration::from_secs(5)).unwrap();
    assert_eq!(queue.pending_ids(), vec![c_id, a_id, b_id]);

    drop((held_a, held_b, held_c));
    assert!(queue.pending_ids().is_empty());
}

#[tokio::test]
async fn broadcast_lag_maps() {
    let queue = HoldQueue::new(&limits(10, u64::MAX, 8));
    let mut rx = queue.subscribe();
    let id = FlowId::new();
    for _ in 0..20 {
        queue.publish(FlowEvent::Recorded {
            flow_id: id,
            at: SystemTime::now(),
        });
    }

    let first = next_event(&mut rx).await;
    match first {
        Some(FlowEvent::Lagged { n }) => assert!(n >= 12, "lagged by {n}"),
        other => panic!("expected Lagged, got {other:?}"),
    }
    let mut remaining = 0;
    while let Ok(event) = rx.try_recv() {
        assert_eq!(event.name(), "recorded");
        remaining += 1;
    }
    assert_eq!(remaining, 8, "the newest events survive");

    drop(queue);
    assert_eq!(next_event(&mut rx).await, None, "closed stream ends");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queue_does_not_block_proxy() {
    const HELD: u32 = 50;
    let queue = Arc::new(HoldQueue::new(&Limits::default()));
    let far = in_(Duration::from_secs(60));

    let mut tasks = JoinSet::new();
    for _ in 0..HELD {
        let queue = Arc::clone(&queue);
        tasks.spawn(async move {
            let mut flow = analyzed_flow(1024);
            queue.hold(&mut flow, far).unwrap().await
        });
    }
    until_held(&queue, HELD).await;

    // Ein weiterer Request, der sofort entschieden wird, wartet nicht auf
    // die fünfzig anderen.
    let mut prompt = analyzed_flow(0);
    let prompt_id = prompt.id;
    let started = Instant::now();
    let held = queue.hold(&mut prompt, far).unwrap();
    queue.decide(prompt_id, Decision::Allow).unwrap();
    assert_eq!(held.await, Decision::Allow);
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "took {:?}",
        started.elapsed()
    );

    for id in queue.pending_ids() {
        queue.decide(id, block(BlockReason::User)).unwrap();
    }
    while let Some(result) = tasks.join_next().await {
        assert_eq!(result.unwrap(), block(BlockReason::User));
    }
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_holds_each_get_their_own_decision() {
    const HELD: u32 = 50;
    let queue = Arc::new(HoldQueue::new(&Limits::default()));
    let mut rx = queue.subscribe();
    let far = in_(Duration::from_secs(60));

    let mut tasks = JoinSet::new();
    for _ in 0..HELD {
        let queue = Arc::clone(&queue);
        tasks.spawn(async move {
            let mut flow = analyzed_flow(1);
            let decision = queue.hold(&mut flow, far).unwrap().await;
            (flow.id, decision, flow.state)
        });
    }
    until_held(&queue, HELD).await;

    // Entscheidungen aus einer anderen Task, abwechselnd Allow und Block.
    let deciding = {
        let queue = Arc::clone(&queue);
        tokio::spawn(async move {
            let mut expected = BTreeMap::new();
            for (index, id) in queue.pending_ids().into_iter().enumerate() {
                let decision = if index % 2 == 0 {
                    Decision::Allow
                } else {
                    block(BlockReason::User)
                };
                queue.decide(id, decision.clone()).unwrap();
                expected.insert(id, decision);
            }
            expected
        })
    };
    let expected = deciding.await.unwrap();
    let held = usize::try_from(HELD).unwrap();
    assert_eq!(expected.len(), held);

    let mut seen = 0;
    while let Some(result) = tasks.join_next().await {
        let (id, decision, state) = result.unwrap();
        assert_eq!(Some(&decision), expected.get(&id), "flow {id}");
        assert_eq!(state, FlowState::Decided(decision));
        seen += 1;
    }
    assert_eq!(seen, HELD);
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));

    let events = drain(&mut rx);
    let decided_events = events.iter().filter(|e| e.name() == "decided").count();
    let held_events = events.iter().filter(|e| e.name() == "held").count();
    assert_eq!((held_events, decided_events), (held, held));
}

#[tokio::test(start_paused = true)]
async fn a_decision_after_the_deadline_is_refused() {
    // Die Frist läuft ab, während niemand das Halte-Future pollt: der Eintrag
    // liegt noch in der Warteschlange, aber entscheiden darf ihn keiner mehr.
    // Ein Ablauf blockt, und eine verspätete Entscheidung überholt ihn nicht.
    let queue = HoldQueue::new(&Limits::default());
    let mut rx = queue.subscribe();
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    let held = queue
        .hold(&mut flow, in_(Duration::from_millis(200)))
        .unwrap();

    tokio::time::advance(Duration::from_millis(300)).await;
    assert_eq!(queue.pending_ids(), vec![id], "still in the queue");

    assert_eq!(
        queue.decide(id, Decision::Allow),
        Err(NotHeld::Unknown { id }),
        "a decision after the deadline comes too late"
    );
    assert_eq!(
        queue.extend(id, Duration::from_secs(60)),
        Err(NotHeld::Unknown { id }),
        "an expired deadline cannot be extended either"
    );

    let decision = held.await;
    assert_eq!(decision, Decision::TimedOut);
    assert_eq!(decision.http_status(), Some(504), "a timeout blocks");
    assert_eq!(flow.state, FlowState::Decided(Decision::TimedOut));
    assert_eq!(names(&drain(&mut rx)), ["held", "timed_out"]);
    assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
}

#[tokio::test(start_paused = true)]
async fn a_decision_just_before_the_deadline_still_wins() {
    // Die Gegenprobe: solange die Frist läuft, kommt die Entscheidung durch.
    let queue = HoldQueue::new(&Limits::default());
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    let held = queue
        .hold(&mut flow, in_(Duration::from_millis(200)))
        .unwrap();

    tokio::time::advance(Duration::from_millis(150)).await;
    queue.decide(id, Decision::Allow).unwrap();
    assert_eq!(held.await, Decision::Allow);
}

// ---------------------------------------------------------------------------
// FlowRegistry
// ---------------------------------------------------------------------------

/// Die Registry läuft mit, wenn die Warteschlange Ereignisse veröffentlicht.
#[tokio::test]
async fn registry_follows_the_queue() {
    let registry = Arc::new(FlowRegistry::new(&Limits::default()));
    let queue = HoldQueue::with_registry(&Limits::default(), Arc::clone(&registry));
    assert!(
        std::ptr::eq(Arc::as_ptr(queue.registry()), Arc::as_ptr(&registry)),
        "queue and registry are the same pair"
    );

    let session = SessionId::new();
    let mut flow = analyzed_flow(42);
    flow.session = session;
    let id = flow.id;
    registry.insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));

    let deadline = in_(Duration::from_secs(60));
    let held = queue.hold(&mut flow, deadline).unwrap();
    let record = registry.get(id).unwrap();
    assert_eq!(record.state, FlowState::Held { deadline });
    assert_eq!(record.deadline, Some(deadline));
    assert_eq!(record.session, session);

    queue.decide(id, Decision::Allow).unwrap();
    assert_eq!(held.await, Decision::Allow);
    let record = registry.get(id).unwrap();
    assert_eq!(record.state, FlowState::Decided(Decision::Allow));
    assert_eq!(record.decision, Some(Decision::Allow));
    assert_eq!(record.deadline, None);

    // Die Fortsetzung gehört dem Handler; sie läuft durch denselben Kanal.
    queue.publish(
        flow.apply(TransitionInput::Forward, SystemTime::now())
            .unwrap(),
    );
    queue.publish(
        flow.apply(TransitionInput::Respond { status: 204 }, SystemTime::now())
            .unwrap(),
    );
    let record = registry.get(id).unwrap();
    assert_eq!(record.state, FlowState::Responded { status: 204 });
    assert_eq!(record.response_status, Some(204));
}

/// Ereignisstrom und Registry hängen an einem Kanal, nicht an zweien.
#[tokio::test]
async fn registry_and_queue_share_one_stream() {
    let registry = Arc::new(FlowRegistry::new(&Limits::default()));
    let queue = HoldQueue::with_registry(&Limits::default(), Arc::clone(&registry));
    let mut from_queue = queue.subscribe();
    let mut from_registry = registry.subscribe();

    let session = SessionId::new();
    let mut flow = analyzed_flow(0);
    let id = flow.id;
    registry.insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
    let held = queue.hold(&mut flow, in_(Duration::from_secs(60))).unwrap();
    queue.decide(id, Decision::Allow).unwrap();
    assert_eq!(held.await, Decision::Allow);

    assert_eq!(names(&drain(&mut from_queue)), ["held", "decided"]);
    assert_eq!(names(&drain(&mut from_registry)), ["held", "decided"]);

    // Auch was die Registry selbst treibt, sehen beide.
    registry
        .transition(id, TransitionInput::Forward)
        .expect("decided(allow) -> forwarded");
    assert_eq!(names(&drain(&mut from_queue)), ["forwarded"]);
    assert_eq!(names(&drain(&mut from_registry)), ["forwarded"]);
    assert_eq!(registry.get(id).unwrap().state, FlowState::Forwarded);
}

/// `list` liefert die gehaltenen Flows nach Frist aufsteigend
/// (Akzeptanzkriterium HUM-016).
#[tokio::test]
async fn registry_lists_held_flows_by_deadline() {
    let registry = Arc::new(FlowRegistry::new(&Limits::default()));
    let queue = HoldQueue::with_registry(&Limits::default(), Arc::clone(&registry));
    let session = SessionId::new();
    let now = Instant::now();

    let mut flows = Vec::new();
    let mut holds = Vec::new();
    for secs in [5_u64, 1, 3] {
        let mut flow = analyzed_flow(0);
        flow.session = session;
        registry.insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        flows.push((secs, flow.id));
        holds.push((flow, now + Duration::from_secs(secs)));
    }
    let mut futures = Vec::new();
    for (flow, deadline) in &mut holds {
        futures.push(queue.hold(flow, *deadline).unwrap());
    }

    let rows = registry.list(&FlowFilter::held());
    let by_deadline: Vec<FlowId> = rows.iter().map(|row| row.id).collect();
    flows.sort_unstable_by_key(|(secs, _)| *secs);
    let expected: Vec<FlowId> = flows.iter().map(|(_, id)| *id).collect();
    assert_eq!(by_deadline, expected, "held flows come deadline first");
    assert_eq!(by_deadline, queue.pending_ids(), "same order as the queue");
    assert!(rows.iter().all(|row| row.session == session));
    assert_eq!(
        registry.list(&FlowFilter::session(SessionId::new())),
        vec![]
    );

    drop(futures);
    assert!(
        registry.list(&FlowFilter::held()).is_empty(),
        "a cancelled hold leaves the queue"
    );
    assert_eq!(registry.list(&FlowFilter::default()).len(), 3);
}

// ---------------------------------------------------------------------------
// Fail-closed bei ungültigem Übergang (PROXY_005)
// ---------------------------------------------------------------------------

/// Eine Pipeline, die `Allow` sagt, ohne den Flow zu entscheiden.
///
/// Der Handler versucht danach `Forward` aus `Analyzed`; genau dieses Paar
/// kennt der Automat nicht. So lässt sich der Fehlerpfad erzwingen, ohne einen
/// Übergang von Hand zu fälschen.
struct AllowWithoutDeciding;

#[async_trait]
impl FlowPipeline for AllowWithoutDeciding {
    async fn decide(&self, _flow: &mut Flow, _meta: &ConnMeta) -> Decision {
        Decision::Allow
    }
}

/// Ein Egress, den der Test nie erreichen darf.
struct NoEgress;

#[async_trait]
impl Egress for NoEgress {
    async fn connect(
        &self,
        authority: &Authority,
        _resolved: Option<std::net::IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, humanitl_core::Diagnostic> {
        panic!("a blocked flow must never reach {authority}");
    }
}

/// Ein Resolver, den der Test nie erreichen darf.
struct NoResolver;

#[async_trait]
impl Resolver for NoResolver {
    async fn resolve(&self, host: &str) -> Result<Vec<std::net::IpAddr>, ResolveError> {
        panic!("a blocked flow must never resolve {host}");
    }
}

#[tokio::test]
async fn invalid_transition_blocks() {
    let tmp = tempfile::tempdir().unwrap();
    let queue = Arc::new(HoldQueue::new(&Limits::default()));
    let mut rx = queue.subscribe();
    let ca = Arc::new(CaStore::load_or_create(&tmp.path().join("ca")).unwrap());
    let upstream = Upstream::new(
        Arc::new(NoEgress) as Arc<dyn Egress>,
        Arc::new(NoResolver) as Arc<dyn Resolver>,
        ClientTls::new(&[], false).unwrap(),
        IpPreference::Ipv4,
    );
    let handler = FlowHandler::new(
        Arc::clone(&queue),
        Arc::new(AllowWithoutDeciding),
        upstream,
        Arc::new(LeafCache::new(ca, 4)),
        ProxyLimits::default(),
    );

    let session = SessionId::new();
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let serving = tokio::spawn(serve_connection(
        handler,
        server_io,
        ConnMeta::plain(session),
    ));
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(client_io))
        .await
        .unwrap();
    let driving = tokio::spawn(async move {
        let _ = conn.await;
    });

    let request = Request::builder()
        .uri("/v1/things")
        .header("host", "api.example.com")
        .body(Full::new(Bytes::new()))
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(10), sender.send_request(request))
        .await
        .expect("the proxy answers in time")
        .expect("the proxy answers");

    assert_eq!(
        response.status().as_u16(),
        BlockReason::NoRoute.http_status(),
        "an invalid transition ends the flow with a block, it does not forward"
    );
    let body = response.into_body();
    let text = String::from_utf8_lossy(&body.collect().await.unwrap().to_bytes()).into_owned();
    assert!(
        text.contains("reason: no_route"),
        "the client is told it was blocked: {text}"
    );

    let events = drain(&mut rx);
    assert_eq!(
        names(&events),
        ["received", "analyzed", "diagnostic", "decided", "recorded"],
        "PROXY_005 comes before the block, and the flow is closed"
    );
    let diagnostic = events
        .iter()
        .find_map(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => Some(diagnostic),
            _ => None,
        })
        .expect("PROXY_005 reaches the stream");
    assert_eq!(diagnostic.code, PROXY_005);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(
        diagnostic.why.contains("analyzed") && diagnostic.why.contains("forward"),
        "the finding names state and transition: {}",
        diagnostic.why
    );

    match &events[3] {
        FlowEvent::Decided {
            decision, source, ..
        } => {
            assert_eq!(decision, &block(BlockReason::NoRoute));
            assert_eq!(source, &DecisionSource::System);
        }
        other => panic!("expected Decided, got {other:?}"),
    }
    assert!(
        events.iter().all(|event| event.name() != "forwarded"),
        "nothing was forwarded"
    );

    drop(sender);
    let _ = driving.await;
    let _ = serving.await;
}
