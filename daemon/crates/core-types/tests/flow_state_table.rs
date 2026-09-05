//! Tabellentest des Zustandsautomaten.
//!
//! Die Tabelle [`TABLE`] ist die einzige Quelle: `allowed_transitions_table`
//! prüft, dass jede Zeile genau so funktioniert, und
//! `forbidden_transitions_are_errors` prüft, dass jedes andere Paar aus
//! Zustand und Eingabe scheitert. Nur beides zusammen fängt den typischen
//! Fehler „ein Übergang zu viel".
//!
//! **Eine Eingabe steht nicht in der Tabelle: `Answer`.** Ihr Nachweis
//! `MetaAnswer` ist außerhalb der Crate nicht baubar, und das ist der Punkt —
//! ein Nachweis, den man mitbringen kann, ließe sich auf einen fremden Fluss
//! anwenden (HUM-103). Von hier führt deshalb nur die eine Tür `Flow::answer`
//! hinein, und der Abschnitt am Ende dieser Datei zählt dieselben zwei Hälften
//! auf: aus welchem Zustand sie öffnet und aus welchen nicht, und für welchen
//! Fluss sie überhaupt öffnet. Die Gegenprobe innerhalb der Crate — ein echter
//! Nachweis an einem fremden Fluss — steht in
//! `flow.rs::tests::a_witness_does_not_open_a_foreign_flow`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant, SystemTime};

use humanitl_core::{
    AnswerRefused, Authority, BlockReason, Decision, DecisionSource, Flow, FlowEvent, FlowId,
    FlowState, HostName, HttpRequest, META_HOST, Method, RuleId, Scheme, SessionId, Transition,
    TransitionInput, UpstreamError,
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
    request_to("api.github.com", "/user")
}

/// Eine Anfrage an `host` mit diesem Pfad.
fn request_to(host: &str, path: &str) -> HttpRequest {
    let host = HostName::parse(host).unwrap();
    HttpRequest::new(
        Method::GET,
        Scheme::Https,
        Authority::with_scheme(host, Scheme::Https),
        path,
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

// ---------------------------------------------------------------------------
// Der Weg am Menschen vorbei, der keiner ist (HUM-103)
// ---------------------------------------------------------------------------
//
// `Received → Recorded` ist der einzige Weg in den Endzustand, der über keine
// Entscheidung führt. Er hat zwei Hälften, und beide gehören geprüft:
//
// 1. Er öffnet nur aus `Received` — aus jedem anderen Zustand wird er
//    abgelehnt, auch für einen Fluss an den reservierten Namen.
// 2. Er öffnet nur für einen Fluss, dessen **eigene** Anfrage an den
//    reservierten Namen ging. Ein Nachweis, den man von anderswo mitbringt,
//    ist gar nicht erst zu haben: `MetaAnswer` lässt sich außerhalb der Crate
//    nicht bauen, und `Flow::answer` gibt keinen heraus. Die Gegenprobe mit
//    einem echten Nachweis an einem fremden Fluss steht deshalb innerhalb der
//    Crate (`flow.rs::tests::a_witness_does_not_open_a_foreign_flow`).

/// Ein Fluss an den reservierten Namen, frisch in `Received`.
fn meta_flow() -> Flow {
    flow_to(META_HOST, "/")
}

/// Ein Fluss an `host`, frisch in `Received`.
fn flow_to(host: &str, path: &str) -> Flow {
    Flow::new(
        FlowId::new(),
        SessionId::new(),
        SystemTime::now(),
        request_to(host, path),
    )
}

/// Nur ein Fluss an den reservierten Namen wird selbst beantwortet.
///
/// Die Namen laufen durch `HostName::parse` und sind danach derselbe Name; ein
/// Name, der nur so *aussieht*, ist ein eigener und gehört nicht hierher.
#[test]
fn only_a_flow_to_the_reserved_name_is_answered() {
    for host in [META_HOST, "HUMANITL.INTERNAL", "humanitl.internal."] {
        let mut flow = flow_to(host, "/");
        let event = flow
            .answer(SystemTime::now())
            .unwrap_or_else(|err| panic!("{host} is the reserved name: {err}"));
        assert_eq!(event.name(), "recorded");
        assert_eq!(flow.state.name(), "recorded");
    }

    for host in [
        "api.github.com",
        "evil-humanitl.internal",
        "sub.humanitl.internal",
        "humanitl.internal.evil.io",
        "127.0.0.1",
    ] {
        let mut flow = flow_to(host, "/");
        let Err(AnswerRefused::NotMeta(diagnostic)) = flow.answer(SystemTime::now()) else {
            panic!("{host} must never be answered by the proxy itself");
        };
        assert_eq!(diagnostic.code.as_str(), "PROXY_009");
        assert!(
            diagnostic.why.contains(host) || diagnostic.why.contains(META_HOST),
            "the finding has to name what was refused: {}",
            diagnostic.why
        );
        assert_eq!(flow.state.name(), "received", "the state is untouched");
        assert_eq!(flow.history.len(), 1, "and so is the history");
    }
}

/// Eine gewöhnliche Anfrage kommt aus `Received` nur über `Analyze` heraus.
#[test]
fn an_ordinary_flow_is_never_answered() {
    let mut flow = flow_to("api.github.com", "/user");
    assert!(
        matches!(
            flow.answer(SystemTime::now()),
            Err(AnswerRefused::NotMeta(_))
        ),
        "without this there would be a way from Received to Recorded for every request"
    );

    // Der Weg, den dieser Fluss wirklich nehmen kann, ist der über `Analyzed`.
    let event = flow
        .apply(
            TransitionInput::Analyze {
                findings: Vec::new(),
            },
            SystemTime::now(),
        )
        .expect("an ordinary request is analysed first");
    assert_eq!(event.name(), "analyzed");
    assert_eq!(flow.state.name(), "analyzed");
}

/// `Flow::answer` lehnt ab, weil der Zustand nicht passt — nicht, weil der Host
/// nicht passt.
fn expect_state_refusal(flow: &mut Flow, from: &str) {
    let history = flow.history.len();
    let Err(err) = flow.answer(SystemTime::now()) else {
        panic!("the meta path must be closed from {from}");
    };
    let AnswerRefused::State(err) = err else {
        panic!("{from} is a meta flow; the refusal has to be about the state");
    };
    assert_eq!(err.from, from);
    assert_eq!(err.input, "answer");
    assert_eq!(flow.state.name(), from, "the state is untouched");
    assert_eq!(flow.history.len(), history, "and so is the history");
}

/// Der Weg öffnet aus `Received` und aus keinem anderen Zustand — auch nicht
/// für einen Fluss an den reservierten Namen.
#[test]
fn the_meta_path_opens_only_from_received() {
    let analyze = || TransitionInput::Analyze {
        findings: Vec::new(),
    };
    let decide = || TransitionInput::Decide {
        decision: Decision::Allow,
        source: DecisionSource::Rule(RuleId::new()),
    };

    // `Analyzed`.
    let mut flow = meta_flow();
    flow.apply(analyze(), SystemTime::now()).expect("analyze");
    expect_state_refusal(&mut flow, "analyzed");

    // `Held`.
    let mut flow = meta_flow();
    flow.apply(analyze(), SystemTime::now()).expect("analyze");
    flow.apply(
        TransitionInput::Hold {
            deadline: Instant::now() + Duration::from_secs(300),
            queue_bytes: 0,
            queue_count: 1,
        },
        SystemTime::now(),
    )
    .expect("hold");
    expect_state_refusal(&mut flow, "held");

    // `Decided`, `Forwarded`, `Responded`, `Recorded`: der gewöhnliche Weg,
    // Station für Station.
    let mut flow = meta_flow();
    flow.apply(analyze(), SystemTime::now()).expect("analyze");
    flow.apply(decide(), SystemTime::now()).expect("decide");
    expect_state_refusal(&mut flow, "decided");
    flow.apply(TransitionInput::Forward, SystemTime::now())
        .expect("forward");
    expect_state_refusal(&mut flow, "forwarded");
    flow.apply(TransitionInput::Respond { status: 200 }, SystemTime::now())
        .expect("respond");
    expect_state_refusal(&mut flow, "responded");
    flow.apply(TransitionInput::Record, SystemTime::now())
        .expect("record");
    expect_state_refusal(&mut flow, "recorded");

    // `Failed`.
    let mut flow = meta_flow();
    flow.apply(analyze(), SystemTime::now()).expect("analyze");
    flow.apply(decide(), SystemTime::now()).expect("decide");
    flow.apply(
        TransitionInput::Fail {
            error: UpstreamError::Dns,
        },
        SystemTime::now(),
    )
    .expect("fail");
    expect_state_refusal(&mut flow, "failed");
}

/// Der Meta-Weg endet wirklich im Endzustand und trägt keine Entscheidung.
#[test]
fn the_meta_path_ends_recorded_without_a_decision() {
    let mut flow = flow_to(META_HOST, "/why/0199c0ff-ee00-7000-8000-8000deadbeef");
    let event = flow
        .answer(SystemTime::now())
        .expect("the reserved name may be answered by the proxy itself");

    assert_eq!(event.name(), "recorded");
    assert!(flow.state.is_terminal());
    assert_eq!(flow.state.name(), "recorded");
    assert!(
        !matches!(flow.state, FlowState::Decided(_)),
        "nobody decided about a meta request"
    );
    // Die Historie zeigt beide Stationen und keine dazwischen.
    let names: Vec<&str> = flow.history.iter().map(|(_, name)| *name).collect();
    assert_eq!(names, vec!["received", "recorded"]);
}
