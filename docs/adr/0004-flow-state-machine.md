# ADR-0004 · Request lifecycle as a state machine, events are derived

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

In Humanitl a request passes through more stations than in an ordinary
proxy: it arrives, is examined for findings, evaluated against rules,
possibly held for minutes, decided, forwarded, answered and recorded. Five
consumers want to know at any moment where it stands: the proxy handler
itself, the hold queue, the UI, the recorder and the audit log.

Without explicit modelling, exactly the kind of code emerges that can no
longer be corrected after two years: scattered boolean flags (`is_held`,
`was_decided`, `response_started`) whose allowed combinations nobody writes
down, and states that exist in the UI but not in the daemon. On top of that
comes the question of whether failures after the release (DNS fails, TCP
connect fails, TLS handshake fails) count as a "response". If they do, an
audit reader sees a `502` and cannot tell whether the server said that or we
did.

## Decision

The lifecycle is an `enum FlowState` in `humanitl-core` with exactly one
method that changes it. `Flow` holds state and history and calls it:

```rust
pub struct Transition {
    pub flow_id: FlowId,
    pub at: SystemTime,
    pub input: TransitionInput,
}

impl FlowState {
    pub fn on(self, t: Transition) -> Result<(FlowState, FlowEvent), InvalidTransition>;
}

impl Flow {
    pub fn apply(&mut self, input: TransitionInput, at: SystemTime)
        -> Result<FlowEvent, InvalidTransition>;
}
```

`Transition` is an envelope: `TransitionInput` says what is to happen;
`flow_id` and `at` come from the caller, because the pure state machine may
neither invent IDs nor read a clock, yet the generated event carries both.
`TransitionInput` has the variants `Analyze` (with the findings of the
detectors), `Hold { deadline, queue_bytes, queue_count }`,
`Decide { decision, source }`, `Forward`, `Respond { status }`, `Record`,
`Answer(MetaAnswer)`, `Timeout` and `Fail { error }`; plus constructors `Transition::analyze(..)`
etc. `Flow::apply` knows its own ID, calls `on` and replaces state and
history; on an invalid transition both stay unchanged
(`backlog/CONVENTIONS.md` 4.11). `FlowEvent` is the derived output.
States:

```
Received → Analyzed{findings} → Held{deadline} → Decided(Allow | AllowEdited | Block | TimedOut)
         → Forwarded → Responded{status} → Recorded
```

In addition `Failed{error: UpstreamError}` with
`UpstreamError = Dns | Connect | Tls | PrivateAddress(IpAddr) | Timeout`,
reachable from `Decided(Allow | AllowEdited)` and from `Forwarded`, that is,
from all states after the release. `Failed` goes to `Recorded`.

Allowed are exactly: `Received→Analyzed→Held→Decided→Forwarded→Responded→Recorded`,
`Held→Decided(TimedOut)→Recorded`, `Decided(Block)→Recorded`,
`Analyzed→Decided` (automatic rule decision, skips `Held`),
`Decided(Allow|AllowEdited)→Failed`, `Forwarded→Failed`, `Failed→Recorded`.
Everything else returns `InvalidTransition { from, input }`.

### Addendum (HUM-103, 2026-09-05): `Received→Recorded` for the meta endpoint

There is exactly one further allowed transition: `Received→Recorded` via
`Answer(MetaAnswer)`. It applies to requests to the reserved name
`humanitl.internal`, which the proxy answers itself (ADR-0014). Such a
request goes nowhere, holds nothing up, and nobody decides about it.
It therefore has no path via `Held` and none via `Decided`: `decision`
states how a human or a rule decided about a request, and a record with an
invented decision would be a claim about a human who did nothing
(`backlog/CONVENTIONS.md` 4.13). Without this transition the request would
remain invisible, and the history would show less than really happened.

The transition does not soften the promise that every ordinary request is
shown to a human. It applies solely to the flow **whose own request** went to
the reserved name, and this is checked on that flow, at the moment of
completion: `Flow::apply` rejects `Answer` as soon as `Flow::is_meta` does not
hold. The only door there is `Flow::answer`; it names the reason (`PROXY_009`
for the wrong host, `InvalidTransition` for the wrong state) and does not hand
out the witness. `MetaAnswer` itself is `#[non_exhaustive]`, so it cannot be
built outside the crate, and carries no data.

That the witness must not travel is the core: a value that only proves that
*some* request went to the reserved name could be applied to a foreign flow
still standing in `Received` — every request stands there after arrival, and
the flow would land in `Recorded` without a human ever having seen it.
`flow_state_table.rs` enumerates both halves (from which state the path opens
and for which flow); the crate-internal counter-check with a real witness on a
foreign flow is in
`flow.rs::tests::a_witness_does_not_open_a_foreign_flow`.

The record of a meta flow carries the column `flows.meta` and **no**
`decision`. It counts in no evaluation that talks about decisions;
the filter term `meta:` separates it from the others.

Who may decide depends on the `DecisionSource`. `DecisionSource::System`
(the daemon itself, for instance when the budget is exhausted) may only refuse
from `Analyzed` and `Held` (`Block`, `TimedOut`), never allow. Only in this way
can `HoldMemory`, `HoldMaxFlows`, `BodyCap` and `ClientTimeout` be expressed
(`backlog/CONVENTIONS.md` 4.11).

An upstream failure is never booked as `Responded`. In this case the client
gets `502` with the uniform `Blocked by Humanitl.` body and
`reason: upstream_*`. The status codes per block reason belong to this
modelling and are identical to ADR-0005:

| Reason | Status |
|---|---|
| `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress` | `403` |
| `BodyCap` | `413` |
| `Timeout` (hold timeout) | `504` |
| `HoldMemory`, `HoldMaxFlows` | `503` |
| `NoRoute` | `502` |
| `ClientTimeout` | `408` |
| Upstream failure (`FlowState::Failed`) | `502` |

Every transition produces a `FlowEvent`, which feeds both the gRPC stream and
the audit log. This is deliberately *not* event sourcing: the in-memory state
is not reconstructed from events. The truth is the state machine plus SQLite;
events are output.

Held requests never block on the UI. The hold queue is a
`DashMap<FlowId, oneshot::Sender<Decision>>` with a deadline timer; the
proxy handler `await`s the receiver.

## Rationale

An `enum` with a single changing method makes invalid states not merely
unlikely but unconstructible. The list of allowed transitions is finite and
small enough to enumerate it completely in a table-driven test — including
all forbidden combinations. A bug in this area is therefore a missing test
case, not a riddle.

That the event is the *output* of the transition and not its input is the
point on which the matter turns. If the event were the input, a caller could
invent an event that never took place, and the state would follow. As it is,
the caller can only *attempt* a transition; the event demonstrably arises
from a transition that took place. For an audit log whose entire purpose is
traceability, this direction is the only defensible one.

The handler *drives* the state machine, it does not *own* it: it calls
`Flow::apply`, publishes the event and, if needed, waits for the decision.
The handler thereby stays thin and the logic testable without a network.

`Failed` as a state of its own instead of `Responded{502}` keeps the recording
honest. In the UI and in the export it is distinguishable whether a server
answered or whether we never got there — and if so, what caused it (DNS,
connect, TLS, private destination address, timeout). For diagnosis on the
user's side, that is the difference between "the service is broken" and "your
DNS is broken".

Timeouts are a transition like any other (`TransitionInput::Timeout` →
`Decided(TimedOut)`), not a special path. The same recording, the same audit
line and the same UI presentation therefore apply to them automatically.

## Rejected alternatives

- **Boolean flags and `Option` fields on a `Flow` struct.** The obvious path.
  Loses because the set of valid combinations is written down nowhere and grows
  exponentially with every new flag.
- **Full event sourcing.** The in-memory state would be a fold over the event
  stream. Sounds clean, but costs replay logic, snapshot management and schema
  migrations for events — for a process that holds a few hundred flows per
  session and is not supposed to restore any in-memory state on restart
  anyway. The event stream remains complete nonetheless, only outwards and not
  inwards.
- **State only in SQLite, every step an `UPDATE`.** Every transition would
  cost IO, the core would no longer be IO-free (violation of ADR-0015), and a
  held request would hang on a database transaction.
- **Upstream failures as `Responded{502}`.** Would be less code, but makes the
  recording ambiguous. Explicitly rejected in the review of 2026-09-02
  (`backlog/CONVENTIONS.md` 4.10).
- **The transition method takes the event as input (`on(self, &FlowEvent)`).**
  This form stood in an early version of `docs/ARCHITECTURE.md` 3 and was
  corrected: the event is output, not input
  (`backlog/CONVENTIONS.md` 4.2).

## Consequences

- The state machine lives in `humanitl-core`, without IO, without async,
  without Protobuf. It can be run through completely in a unit test.
- The test for forbidden transitions is a mandatory part of HUM-004: every
  combination of state and `TransitionInput` that is not in the list
  must return `InvalidTransition`.
- `FlowEvent` is the only path that UI, recorder and audit hang on. A new
  state without a new event is impossible.
- The `Held` event additionally carries `queue_bytes` and `queue_count`, so
  that the hold budget is visible in the UI (`backlog/CONVENTIONS.md` 4.2,
  HUM-057).
- The status codes for the individual block reasons are in the table above and
  once more in ADR-0005; both tables must stay identical.

## Affected issues

`HUM-004` (core-types with state machine and transition tests), `HUM-016`
(hold queue, runs through the state machine), `HUM-024` (DNS/connect failures
as `Failed`), `HUM-026` (recorder as event consumer), `HUM-018` (`Subscribe`
stream), `HUM-057` (`queue_bytes`/`queue_count` in the `Held` event).
