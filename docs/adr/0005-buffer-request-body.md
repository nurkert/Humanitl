# ADR-0005 · Buffer the request body completely before the human decides

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

The human decides about a request. For this decision to be worth anything,
they must see what they are deciding about. In an exfiltration attempt, the
interesting part almost never stands in the headers: the host is
`api.github.com`, the method is `POST`, the path is inconspicuous — and the
body holds the content of `.env`, base64-encoded.

A proxy that checks only the headers and streams the body through while the
human is still reading therefore releases exactly the part for whose sake the
tool exists. The release must come before the first byte of body that leaves
the host.

Memory stands against this: a buffered 40 MB upload sits in the daemon, and
two hundred simultaneously held requests can run the machine into the wall.
There is also `Expect: 100-continue`: a client that asks this way sends the
body only after an interim response — whoever withholds this interim response
never gets to see the body and consequently cannot display it either.

## Decision

The request body is buffered completely before the flow goes into `Held`. Only
then is the request shown in the UI. Nothing reaches the upstream before the
decision.

- **Body cap** `limits.hold_body_cap_bytes` (alias `hold.body_cap_bytes`),
  default 32 MiB. Above the cap the request is blocked with `413`
  (`BlockReason::BodyCap`), unless a matching rule explicitly sets
  `stream: true`.
- **`Expect: 100-continue`** is answered by the proxy immediately with
  `100 Continue`. The body then flows into the proxy's hold buffer, not to the
  upstream. Only the decision opens the way out.
- **Hold memory budget** is global: `limits.hold_max_bytes` (default 256 MiB)
  and `limits.hold_max_flows` (default 200), as atomic counters in the
  `HoldQueue`. If a budget is exceeded, the *new* request is turned away with
  `503` (`BlockReason::HoldMemory` or `BlockReason::HoldMaxFlows`
  respectively). A request that is already held is never discarded.
- **Responses** are always streamed (LLM streaming and SSE work) and mirrored
  to the recorder in parallel.

Status codes per block reason, binding:

| Reason | Status |
|---|---|
| `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress` | `403` |
| `BodyCap` | `413` |
| `Timeout` (hold timeout) | `504` |
| `HoldMemory`, `HoldMaxFlows` | `503` |
| Upstream failure (`FlowState::Failed`) | `502` |
| `NoRoute` | `502` |
| `ClientTimeout` | `408` |

The response body has the same structure in all cases, `text/plain`:

```
Blocked by Humanitl.
reason: <BlockReason as snake_case>
flow: <FlowId>
host: <host>
```

## Rationale

Whoever releases only headers and streams the body does not see exactly the
part in which exfiltration happens. That is the whole reason. Everything else
in this decision is follow-on cost, paid so that this sentence holds.

The cap limits the cost. 32 MiB covers normal API calls, file uploads of
medium size and LLM prompts with context. Above that, a block with `413` is
the honest answer: "I cannot show you this, so I do not let it through." The
way out, `stream: true`, is a deliberate, named rule — the user releases a
route explicitly instead of the system silently making an exception.

The global budget protects against the second form of attack: not one large
request, but a thousand medium ones. Counters instead of eviction, because
discarding a held request would be a decision the user did not make.

The uniform body structure is agent ergonomics. An agent that learns that
`Blocked by Humanitl.` is final has to learn it exactly once, not once per
error class. It can evaluate the `reason:` line by machine and tell its user
what is going on. ADR-0014 builds on this and extends the body with `note:`.

The handling of `Expect: 100-continue` is a **correction** of an earlier
version of this ADR. It said "`100 Continue` only after the decision". That
would have been self-contradictory: without `100 Continue` the client does not
send the body, without a body there is nothing to display, without display no
decision. The immediate `100 Continue` is safe because it only means
"send me the body", not "I am forwarding it". The wording in
`backlog/CONVENTIONS.md` 4.10 is authoritative.

## Rejected alternatives

- **Hold only headers, stream the body.** The comfortable path, and the only
  one that gets by without a memory budget. Loses because it breaks the core
  promise: the human would decide about a summary while the content is already
  on its way.
- **Stream the body and analyse it afterwards.** An alarm after the outflow is
  not protection but a log. For the GDPR situation of the audience, "we noticed
  that customer data flowed out" is the damage case, not its prevention.
- **Buffer and display only the first N kilobytes.** An attacker puts their
  payload behind byte N+1. For the *display* of large bodies there is a
  staggered presentation (size, first 64 KB, scan status, HUM-030), but the
  *release* always refers to the body as it is completely present.
- **Buffer on disk instead of in memory.** Lifts the cap, but costs IO in the
  hot path, potentially writes sensitive bodies unencrypted to disk and only
  shifts the problem. Later if needed, behind the same budget.
- **Evict held requests under memory pressure.** Rejected: the user would see
  requests disappear from their queue without having decided them. `503` for
  the newcomer is the more understandable rule.

## Consequences

- The daemon's memory requirement is bounded above by `limits.hold_max_bytes`
  and therefore predictable. The counters already arise in HUM-016, not only
  in the hardening phase; HUM-057 merely tunes them.
- Very large uploads work only with an explicit `stream` rule. That is
  deliberate friction, which is explained in the UI.
- `Expect: 100-continue` is a test case of its own in the conformance matrix
  (HUM-017): `100 Continue` goes out immediately, but the upstream sees nothing
  until the decision.
- The block body is part of the public behaviour and therefore
  regression-tested. Changes to it are contract changes.
- Decompressing bodies for the preview needs its own ratio limit
  (`limits.max_decompress_ratio`, default 100) and its own preview cap
  (`limits.preview_cap_bytes`, default 8 MiB), otherwise the cap could be
  circumvented with a zip bomb.

## Affected issues

`HUM-015` (MITM proxy core with buffering and `Expect` handling), `HUM-016`
(hold queue with budget counters), `HUM-017` (conformance matrix, large bodies
and `Expect`), `HUM-030` (body views for large bodies), `HUM-057`
(resource limits and decompression ratio).
