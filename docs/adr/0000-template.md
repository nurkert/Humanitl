# ADR-NNNN · Short title as a statement

Applies to: Humanitl 0.1

Status: Accepted | Superseded by ADR-XXXX | Deprecated
Date: YYYY-MM-DD

## Context

Which forces are at work? Which problem forces a decision, and why can the
decision not be postponed? The section describes the situation without
anticipating the solution. It names the hard constraints (target platform,
security guarantees, team size, existing building blocks), so that a later
reader understands under which assumptions the decision was made. If these
assumptions change, that is the occasion for a successor ADR.

## Decision

One sentence in the active voice and present tense: "We do X." Then the concrete
form, precise enough to be checkable: types, file paths, configuration keys,
status codes, version numbers. Prose only where no identifier suffices.

## Rationale

Why this option resolves the forces from the context best. Every claim is
worded so that it is falsifiable. Where an assumption is uncertain, the section
says how it is checked (test, spike, milestone).

## Rejected alternatives

One paragraph or list item per alternative: what it would have been and why it
lost. An alternative without a reason for losing is not a documented
alternative but a note. "Do nothing" is a legitimate alternative and is named
when it was seriously considered.

## Consequences

What follows from the decision, positive and negative. In particular: which work
it creates, which risks it buys, which mitigation is planned for them, which
doors it closes and which it keeps open. This is also where later ADRs that
correct or refine this decision are named.

## Affected issues

`HUM-xxx`, `HUM-yyy` — the issues that implement or check this decision.
At least one. Reference to `backlog/sprint-N.md`.
