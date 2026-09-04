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
    Authority, BlockReason, Decision, DecisionSource, Flow, FlowEvent, FlowId, FlowState, HostName,
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

/// Der Tabellenschlüssel eines Zustands.
///
/// **Der Zeuge dieser Datei.** Das `match` ist erschöpfend über [`FlowState`]
/// *und* über [`Decision`], ohne `_`-Zweig. Eine neue Variante an einer der
/// beiden Stellen ist hier ein Compilerfehler, und wer sie einträgt, kommt an
/// [`states`] und an der Fail-closed-Tabelle nicht vorbei.
///
/// Ohne diesen Zeugen verglichen die Tests nur zwei von Hand getippte Listen
/// derselben Datei: eine zwölfte Variante von [`FlowState`] wäre in beiden
/// gefehlt, alle Tests wären grün geblieben, und `Flow::fail_closed` hätte aus
/// ihr nichts getan — der Flow hinge für immer in der Registry.
fn key_of(state: &FlowState) -> &'static str {
    match state {
        FlowState::Received => "received",
        FlowState::Analyzed { .. } => "analyzed",
        FlowState::Held { .. } => "held",
        FlowState::Decided(decision) => match decision {
            Decision::Allow => "decided:allow",
            Decision::AllowEdited { .. } => "decided:allow_edited",
            Decision::Block { .. } => "decided:block",
            Decision::TimedOut => "decided:timed_out",
        },
        FlowState::Forwarded => "forwarded",
        FlowState::Responded { .. } => "responded",
        FlowState::Failed { .. } => "failed",
        FlowState::Recorded => "recorded",
    }
}

/// Ein Beispiel je Variante, mit dem Schlüssel aus [`key_of`].
///
/// Die Schlüssel stehen hier nicht noch einmal als Literale: sie kommen aus dem
/// erschöpfenden `match`, damit ein Tippfehler unmöglich ist und eine neue
/// Variante auffällt.
fn states() -> Vec<(&'static str, FlowState)> {
    [
        FlowState::Received,
        FlowState::Analyzed {
            findings: Vec::new(),
        },
        FlowState::Held {
            deadline: Instant::now() + Duration::from_secs(300),
        },
        FlowState::Decided(Decision::Allow),
        FlowState::Decided(Decision::AllowEdited {
            request: Box::new(request()),
        }),
        FlowState::Decided(Decision::Block {
            reason: BlockReason::User,
            note: None,
        }),
        FlowState::Decided(Decision::TimedOut),
        FlowState::Forwarded,
        FlowState::Responded { status: 200 },
        FlowState::Failed {
            error: UpstreamError::Dns,
        },
        FlowState::Recorded,
    ]
    .into_iter()
    .map(|state| (key_of(&state), state))
    .collect()
}

/// Jeder Schlüssel, den [`key_of`] überhaupt liefern kann, hat ein Beispiel in
/// [`states`].
///
/// Das schließt die Lücke, die der Zeuge allein lässt: Der Compilerfehler
/// zwingt zu einem neuen Arm in [`key_of`], dieser Test zwingt zum Beispiel in
/// [`states`]. Die Liste unten ist die Aufzählung der Arme; sie ist die einzige
/// Stelle, an der ein Schlüssel als Literal steht.
#[test]
fn every_arm_of_the_witness_has_an_example() {
    const ARMS: [&str; 11] = [
        "received",
        "analyzed",
        "held",
        "decided:allow",
        "decided:allow_edited",
        "decided:block",
        "decided:timed_out",
        "forwarded",
        "responded",
        "failed",
        "recorded",
    ];
    let keys: Vec<&str> = states().into_iter().map(|(key, _)| key).collect();
    assert_eq!(
        keys, ARMS,
        "states() and the arms of key_of have drifted apart"
    );
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

/// Das System darf eine Freigabe vor dem Weiterleiten in eine Sperre
/// verwandeln, nie umgekehrt und nie von einer anderen Quelle aus: Der Proxy
/// nutzt das, wenn eine bearbeitete Anfrage ein anderes Ziel traegt als das,
/// fuer das entschieden wurde.
#[test]
fn the_system_may_turn_an_allow_into_a_block_before_forwarding() {
    let flow = FlowId::new();
    let at = SystemTime::now();
    let block = Decision::Block {
        reason: BlockReason::AuthorityMismatch,
        note: None,
    };
    for allowed in [
        FlowState::Decided(Decision::Allow),
        FlowState::Decided(Decision::AllowEdited {
            request: Box::new(request()),
        }),
    ] {
        let (state, event) = allowed
            .clone()
            .on(Transition::decide(
                flow,
                at,
                block.clone(),
                DecisionSource::System,
            ))
            .expect("the system may tighten an allow into a block");
        assert_eq!(state, FlowState::Decided(block.clone()));
        assert!(matches!(event, humanitl_core::FlowEvent::Decided { .. }));
        assert!(
            allowed
                .clone()
                .on(Transition::decide(
                    flow,
                    at,
                    block.clone(),
                    DecisionSource::User
                ))
                .is_err(),
            "only the system may revise a decision after the fact"
        );
        assert!(
            allowed
                .on(Transition::decide(
                    flow,
                    at,
                    Decision::Allow,
                    DecisionSource::System
                ))
                .is_err(),
            "a second allow is not a revision"
        );
    }
    assert!(
        FlowState::Forwarded
            .on(Transition::decide(flow, at, block, DecisionSource::System))
            .is_err(),
        "once bytes left, nothing is revised"
    );
}

/// Fail-closed endet aus jedem Zustand in `Recorded`, und eine Freigabe wird
/// dabei zur Sperre.
///
/// Die Zeile `forwarded` ist die einzige, die `failed` erzeugt: von dort kennt
/// der Automat nur `Respond` und `Fail`, und `Respond` hieße, das Ziel habe
/// geantwortet. Welcher Upstream-Fehler das ist, sagt der Aufrufer — hier
/// `Tls`, damit der Test zeigt, dass nichts erfunden wird; der Proxy nennt
/// seinen eigenen Wert und schickt ihn auch an den Client.
///
/// [`Flow::fail_closed`] ist die einzige Stelle, die weiß, wie ein Flow nach
/// einem abgelehnten Übergang zu Ende geht. Vorher stand dieselbe Tabelle ein
/// zweites Mal im Proxy-Handler und war dort anderer Meinung: Aus
/// `Decided(Allow)` machte sie `Failed { Connect }` und schrieb damit einen
/// Verbindungsfehler ins Protokoll, den es nie gab. Die Zeile
/// `decided:allow` hält fest, dass daraus `decided` wird und nicht `failed`.
#[test]
fn fail_closed_ends_recorded_from_every_state() {
    /// Zustand und die Ereignisse, die fail-closed aus ihm erzeugt.
    const EXPECTED: &[(&str, &[&str])] = &[
        ("received", &["analyzed", "decided", "recorded"]),
        ("analyzed", &["decided", "recorded"]),
        ("held", &["decided", "recorded"]),
        ("decided:allow", &["decided", "recorded"]),
        ("decided:allow_edited", &["decided", "recorded"]),
        ("decided:block", &["recorded"]),
        ("decided:timed_out", &["recorded"]),
        ("forwarded", &["failed", "recorded"]),
        ("responded", &["recorded"]),
        ("failed", &["recorded"]),
        ("recorded", &[]),
    ];

    let at = SystemTime::now();
    assert_eq!(
        EXPECTED.len(),
        states().len(),
        "every state of the machine is listed here"
    );

    for (key, state) in states() {
        let Some((_, want)) = EXPECTED.iter().find(|(name, _)| *name == key) else {
            panic!("state {key} is missing from the fail-closed table");
        };
        let mut flow = Flow::new(FlowId::new(), SessionId::new(), at, request());
        flow.state = state;
        let events = flow.fail_closed(BlockReason::NoRoute, UpstreamError::Tls, at);
        let names: Vec<&str> = events.iter().map(FlowEvent::name).collect();
        assert_eq!(names, *want, "fail-closed from {key} takes the wrong path");
        assert!(
            flow.state.is_terminal(),
            "fail-closed from {key} must end in recorded, it ended in {}",
            flow.state.name()
        );
        // Der Aufrufer nennt den Upstream-Fehler; der Automat erfindet keinen.
        // Wer das Argument ignorierte, käme hier mit `connect` heraus.
        if want.first() == Some(&"failed") {
            let Some(FlowEvent::Failed { error, .. }) = events.first() else {
                panic!("fail-closed from {key} must report the caller's error: {events:?}");
            };
            assert_eq!(*error, UpstreamError::Tls);
        }
    }
}

/// Aus einer Freigabe wird beim Fail-closed eine Sperre mit dem Grund, den der
/// Aufrufer nennt — nicht ein Upstream-Fehler, den niemand erlebt hat.
#[test]
fn fail_closed_turns_an_allow_into_the_block_it_was_asked_for() {
    let at = SystemTime::now();
    let mut flow = Flow::new(FlowId::new(), SessionId::new(), at, request());
    flow.state = FlowState::Decided(Decision::Allow);

    let events = flow.fail_closed(BlockReason::NoRoute, UpstreamError::Tls, at);
    let Some(FlowEvent::Decided {
        decision, source, ..
    }) = events.first()
    else {
        panic!("fail-closed from an allow must decide, not fail: {events:?}");
    };
    assert_eq!(
        *decision,
        Decision::Block {
            reason: BlockReason::NoRoute,
            note: None
        }
    );
    assert_eq!(*source, DecisionSource::System);
    assert_eq!(
        flow.history
            .iter()
            .map(|(_, name)| *name)
            .collect::<Vec<_>>(),
        vec!["received", "decided", "recorded"]
    );
}
