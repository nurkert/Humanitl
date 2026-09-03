//! Tabellentest des Zustandsautomaten.
//!
//! Die Tabelle [`TABLE`] ist die einzige Quelle: `allowed_transitions_table`
//! prüft, dass jede Zeile genau so funktioniert, und
//! `forbidden_transitions_are_errors` prüft, dass jedes andere Paar aus
//! Zustand und Eingabe scheitert. Nur beides zusammen fängt den typischen
//! Fehler „ein Übergang zu viel".

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant, SystemTime};

use humanitl_core::{
    Authority, BlockReason, Decision, DecisionSource, Flow, FlowId, FlowState, HostName,
    HttpRequest, Method, RuleId, Scheme, SessionId, Transition, TransitionInput, UpstreamError,
};

/// `von`, `Eingabe`, `nach`, `Ereignis`.
const TABLE: &[(&str, &str, &str, &str)] = &[
    ("received", "analyze", "analyzed", "analyzed"),
    ("analyzed", "hold", "held", "held"),
    ("analyzed", "decide:rule", "decided", "decided"),
    ("analyzed", "decide:passthrough", "decided", "decided"),
    ("held", "decide:user", "decided", "decided"),
    ("held", "decide:rule", "decided", "decided"),
    ("held", "timeout", "decided", "timed_out"),
    ("decided:allow", "forward", "forwarded", "forwarded"),
    ("decided:allow_edited", "forward", "forwarded", "forwarded"),
    ("decided:allow", "fail", "failed", "failed"),
    ("decided:allow_edited", "fail", "failed", "failed"),
    ("decided:block", "record", "recorded", "recorded"),
    ("decided:timed_out", "record", "recorded", "recorded"),
    ("forwarded", "respond", "responded", "response_headers"),
    ("forwarded", "fail", "failed", "failed"),
    ("responded", "record", "recorded", "recorded"),
    ("failed", "record", "recorded", "recorded"),
];

fn request() -> HttpRequest {
    let host = HostName::parse("api.github.com").unwrap();
    HttpRequest::new(
        Method::GET,
        Scheme::Https,
        Authority::with_scheme(host, Scheme::Https),
        "/user",
    )
}

fn states() -> Vec<(&'static str, FlowState)> {
    vec![
        ("received", FlowState::Received),
        (
            "analyzed",
            FlowState::Analyzed {
                findings: Vec::new(),
            },
        ),
        (
            "held",
            FlowState::Held {
                deadline: Instant::now() + Duration::from_secs(300),
            },
        ),
        ("decided:allow", FlowState::Decided(Decision::Allow)),
        (
            "decided:allow_edited",
            FlowState::Decided(Decision::AllowEdited {
                request: Box::new(request()),
            }),
        ),
        (
            "decided:block",
            FlowState::Decided(Decision::Block {
                reason: BlockReason::User,
                note: None,
            }),
        ),
        ("decided:timed_out", FlowState::Decided(Decision::TimedOut)),
        ("forwarded", FlowState::Forwarded),
        ("responded", FlowState::Responded { status: 200 }),
        (
            "failed",
            FlowState::Failed {
                error: UpstreamError::Dns,
            },
        ),
        ("recorded", FlowState::Recorded),
    ]
}

fn decide(source: DecisionSource) -> TransitionInput {
    TransitionInput::Decide {
        decision: Decision::Allow,
        source,
    }
}

fn inputs() -> Vec<(&'static str, TransitionInput)> {
    vec![
        (
            "analyze",
            TransitionInput::Analyze {
                findings: Vec::new(),
            },
        ),
        (
            "hold",
            TransitionInput::Hold {
                deadline: Instant::now() + Duration::from_secs(300),
                queue_bytes: 1024,
                queue_count: 3,
            },
        ),
        ("decide:user", decide(DecisionSource::User)),
        ("decide:rule", decide(DecisionSource::Rule(RuleId::new()))),
        ("decide:passthrough", decide(DecisionSource::Passthrough)),
        ("decide:timeout", decide(DecisionSource::Timeout)),
        ("decide:system", decide(DecisionSource::System)),
        ("forward", TransitionInput::Forward),
        ("respond", TransitionInput::Respond { status: 200 }),
        ("record", TransitionInput::Record),
        ("timeout", TransitionInput::Timeout),
        (
            "fail",
            TransitionInput::Fail {
                error: UpstreamError::Connect,
            },
        ),
    ]
}

fn state_named(name: &str) -> FlowState {
    states()
        .into_iter()
        .find(|(key, _)| *key == name)
        .map_or_else(|| panic!("unknown state key {name}"), |(_, state)| state)
}

fn input_named(name: &str) -> TransitionInput {
    inputs()
        .into_iter()
        .find(|(key, _)| *key == name)
        .map_or_else(|| panic!("unknown input key {name}"), |(_, input)| input)
}

#[test]
fn allowed_transitions_table() {
    let flow = FlowId::new();
    let at = SystemTime::now();

    for (from, input, to, event) in TABLE {
        let state = state_named(from);
        let transition = Transition::new(flow, at, input_named(input));
        let result = state.on(transition);
        let Ok((next, produced)) = result else {
            panic!("{from} + {input} must be allowed");
        };
        assert_eq!(
            next.name(),
            *to,
            "{from} + {input} lands in the wrong state"
        );
        assert_eq!(
            produced.name(),
            *event,
            "{from} + {input} produces the wrong event"
        );
        assert_eq!(produced.flow_id(), Some(flow));
        assert_eq!(produced.at(), Some(at));
    }
}

#[test]
fn forbidden_transitions_are_errors() {
    let flow = FlowId::new();
    let at = SystemTime::now();
    let mut checked = 0_usize;

    for (state_key, state) in states() {
        for (input_key, input) in inputs() {
            if TABLE
                .iter()
                .any(|(from, name, _, _)| *from == state_key && *name == input_key)
            {
                continue;
            }
            let transition = Transition::new(flow, at, input);
            let Err(err) = state.clone().on(transition) else {
                panic!("{state_key} + {input_key} must be rejected");
            };
            assert_eq!(err.from, state.name());
            assert!(
                input_key.starts_with(err.input),
                "{input_key} reported as {}",
                err.input
            );
            checked += 1;
        }
    }

    assert!(
        checked >= 35,
        "only {checked} forbidden pairs were checked, the cross product is incomplete"
    );
    assert_eq!(checked, states().len() * inputs().len() - TABLE.len());
}

#[test]
fn analyzed_decide_only_by_rule_or_passthrough() {
    let flow = FlowId::new();
    let at = SystemTime::now();

    for source in [
        DecisionSource::User,
        DecisionSource::Timeout,
        DecisionSource::System,
    ] {
        let analyzed = FlowState::Analyzed {
            findings: Vec::new(),
        };
        let transition = Transition::decide(flow, at, Decision::Allow, source);
        assert!(
            analyzed.on(transition).is_err(),
            "{source:?} must not allow from analyzed"
        );
    }
}

#[test]
fn the_system_may_refuse_but_never_allow() {
    let flow = FlowId::new();
    let at = SystemTime::now();
    let block = Decision::Block {
        reason: BlockReason::HoldMemory,
        note: None,
    };

    let analyzed = FlowState::Analyzed {
        findings: Vec::new(),
    };
    let result = analyzed.on(Transition::decide(
        flow,
        at,
        block.clone(),
        DecisionSource::System,
    ));
    assert!(result.is_ok(), "the hold budget must be able to refuse");

    let held = FlowState::Held {
        deadline: Instant::now(),
    };
    let client_gone = Decision::Block {
        reason: BlockReason::ClientTimeout,
        note: None,
    };
    assert!(
        held.on(Transition::decide(
            flow,
            at,
            client_gone,
            DecisionSource::System
        ))
        .is_ok()
    );

    let held = FlowState::Held {
        deadline: Instant::now(),
    };
    assert!(
        held.on(Transition::decide(
            flow,
            at,
            Decision::Allow,
            DecisionSource::System
        ))
        .is_err(),
        "the system must never allow"
    );
}

#[test]
fn recorded_is_terminal() {
    let flow = FlowId::new();
    let at = SystemTime::now();
    assert!(FlowState::Recorded.is_terminal());

    for (key, input) in inputs() {
        let transition = Transition::new(flow, at, input);
        assert!(
            FlowState::Recorded.on(transition).is_err(),
            "recorded + {key} must be rejected"
        );
    }
}

#[test]
fn a_failed_upstream_never_becomes_a_response() {
    let flow = FlowId::new();
    let at = SystemTime::now();

    let (state, event) = FlowState::Forwarded
        .on(Transition::fail(flow, at, UpstreamError::Tls))
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(state.name(), "failed");
    assert_eq!(event.name(), "failed");

    assert!(
        state
            .clone()
            .on(Transition::respond(flow, at, 502))
            .is_err(),
        "a failed flow cannot answer"
    );
    assert!(
        FlowState::Forwarded
            .on(Transition::record(flow, at))
            .is_err(),
        "a forwarded flow without an answer must fail first, not vanish into recorded"
    );
    let (recorded, event) = state
        .on(Transition::record(flow, at))
        .unwrap_or_else(|err| panic!("{err}"));
    assert!(recorded.is_terminal());
    assert_eq!(event.name(), "recorded");
}

#[test]
fn hold_timeout_decides_timed_out() {
    let flow = FlowId::new();
    let at = SystemTime::now();
    let held = FlowState::Held {
        deadline: Instant::now(),
    };
    let (state, event) = held
        .on(Transition::timeout(flow, at))
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(state, FlowState::Decided(Decision::TimedOut));
    assert_eq!(event.name(), "timed_out");
}

#[test]
fn flow_apply_appends_history() {
    let received_at = SystemTime::now();
    let mut flow = Flow::new(FlowId::new(), SessionId::new(), received_at, request());
    assert_eq!(flow.history.len(), 1);
    assert_eq!(flow.received_event().name(), "received");

    let steps: Vec<TransitionInput> = vec![
        TransitionInput::Analyze {
            findings: Vec::new(),
        },
        TransitionInput::Hold {
            deadline: Instant::now() + Duration::from_secs(300),
            queue_bytes: 0,
            queue_count: 1,
        },
        TransitionInput::Decide {
            decision: Decision::Allow,
            source: DecisionSource::User,
        },
        TransitionInput::Forward,
        TransitionInput::Respond { status: 204 },
        TransitionInput::Record,
    ];

    for (step, input) in steps.into_iter().enumerate() {
        let at = received_at + Duration::from_millis(u64::try_from(step).unwrap_or(0) + 1);
        flow.apply(input, at).unwrap_or_else(|err| panic!("{err}"));
    }

    let names: Vec<&str> = flow.history.iter().map(|(_, name)| *name).collect();
    assert_eq!(
        names,
        vec![
            "received",
            "analyzed",
            "held",
            "decided",
            "forwarded",
            "responded",
            "recorded"
        ]
    );
    assert!(flow.state.is_terminal());

    let err = flow
        .apply(TransitionInput::Record, received_at)
        .expect_err("recorded is terminal");
    assert_eq!(err.from, "recorded");
    assert_eq!(
        flow.history.len(),
        7,
        "a rejected transition changes nothing"
    );
}
