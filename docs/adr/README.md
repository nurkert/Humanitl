# Architecture decisions (ADRs)

Applies to: Humanitl 0.1

Every decision that carries the architecture of Humanitl has its own file
here. The short versions in `BACKLOG.md` section 2 are the origin; the files
here are the worked-out, binding version. Anyone who wants to look up a
decision reads the file — it is written so that it can be understood without
the backlog.

The format is [MADR](https://adr.github.io/madr/) in a lean version. Every
file has the same seven headings: title, then `Context`, `Decision`,
`Rationale`, `Rejected alternatives`, `Consequences`, `Affected issues`.
Below the title stand `Status:` and `Date:`. A decision that is reviewed by an
issue of its own may carry an eighth section: the addendum with the result of
that review, named after it (`Decision after Sprint N`). It stands before
`Affected issues`, and the sections it replaces get a banner pointing at it
instead of being overwritten. `check.sh` checks the seven mandatory headings
and does not mind the eighth. Template for new decisions:
[`0000-template.md`](0000-template.md).

Rules:

- **Numbers are never renamed and never reused.** The order in the backlog
  differs in one place (`ADR-010` stands there after `ADR-018`); the numbers
  follow the backlog, not the reading order.
- **Two spellings of the same number.** In the files here a decision is
  called `ADR-0007` with four digits, matching the file name. `BACKLOG.md`,
  `docs/ARCHITECTURE.md` and `backlog/CONVENTIONS.md` write the same decision
  with three digits as `ADR-007`. The same file is always meant.
- If a decision is replaced, the old file stays and gets
  `Status: Superseded by ADR-XXXX`. The new file refers back.
- Every ADR names at least one issue from `backlog/sprint-N.md`.
- `backlog/CONVENTIONS.md` section 4 is the running log of corrections; when a
  correction lands there, the affected ADR is brought up to date instead of
  leaving the deviation in place.
- `docs/adr/check.sh` checks the numbering, the completeness of the index and
  the seven headings. It is the ADR part of the documentation check and is
  built to be called from `scripts/ci/lint-docs.sh` (HUM-007), so that
  `make docs-lint` runs it too.

## Index

| No. | Title | Status | What it is about |
|---|---|---|---|
| [0001](0001-rust-hudsucker.md) | Daemon in Rust on hudsucker, not mitmproxy | Accepted | One memory-safe binary instead of a Python bundle; hudsucker provides CONNECT, TLS termination and the async hold point. |
| [0002](0002-bwrap-first.md) | bubblewrap first, Docker later as a second backend | Accepted | `--unshare-all` gives an empty network namespace; the policy is a readable command line that the UI shows. |
| [0003](0003-grpc-uds.md) | gRPC over a Unix domain socket as the only interface | Accepted | One contract (`humanitl.v1`) for UI, CLI, tests and later plugins, with streaming and backpressure. |
| [0004](0004-flow-state-machine.md) | Request lifecycle as a state machine, events derived | Accepted | `FlowState` with one transition method; every `FlowEvent` is the output of a transition that took place. |
| [0005](0005-buffer-request-body.md) | Buffer the request body completely before the human decides | Accepted | Whoever approves only headers does not see the part in which data is exfiltrated; caps, budget and a uniform block body. |
| [0006](0006-dns-after-allow.md) | DNS resolution only after allow | Accepted | A hostname leaks 63 bytes per label; it is resolved once after `allow`, and the IP is pinned. |
| [0007](0007-rule-model.md) | Rule model: ordered list, first match wins, default `ask` | Accepted | Label globs instead of substrings, Punycode normalisation, session rules first, domain fronting blocks without asking. |
| [0008](0008-storage.md) | Storage: SQLite, blob store, audit chain, pseudonym mapping | Accepted | Metadata in SQLite, large bodies content-addressed, audit as a JSONL hash chain with an honest statement about its limits. |
| [0009](0009-ui-stack.md) | UI stack: Flutter with `shadcn_flutter` behind `packages/ui` | Accepted | Data-heavy widgets from specialised packages, no WebView; HUM-035 decided against it on 2026-09-04, the project owner reversed that on the same day. |
| [0010](0010-packaging.md) | Delivery as `.deb` and AppImage, Flatpak later and only for the UI | Accepted | One artefact contains everything; systemd user unit with one click; Flatpak fails at the daemon's `bwrap` start. |
| [0011](0011-single-config-source.md) | One configuration source, three visibility tiers | Accepted | Rust type as the only source for schema, CLI flags, settings screen and docs; `basic`/`advanced`/`expert`. |
| [0012](0012-diagnostics-as-type.md) | Guided states as a type: `Diagnostic` instead of an error string | Accepted | Every error path returns `code`, `why` and an executable `FixAction`; codes stand in a register. |
| [0013](0013-cli-headless.md) | CLI as an equal client and headless operation | Accepted | The hold queue lives in the daemon; `--ask terminal` and `--ask none` as the two honest modes without an interface. |
| [0014](0014-agent-awareness.md) | Agent awareness and feedback over the one existing channel | Accepted | Briefing, note in the 403, meta endpoint `humanitl.internal` — no new channel, no new capability for the agent. |
| [0015](0015-ports-and-adapters.md) | Ports and adapters with a machine-enforced dependency direction | Accepted | Core without IO, closed list of ports, `tools/check-deps.sh` checks the Cargo graph instead of a review. |
| [0016](0016-browser-cdp.md) | Browser for the agent over CDP, watching and intervening in the UI | Accepted | Post-MVP (M7); in the MVP only the groundwork: bridge list in the shim and seccomp families per profile. |
| [0017](0017-egress-port.md) | One egress port for direct connection, upstream proxy and Tor | Accepted | Exactly one place where an outbound connection is made; CI forbids `TcpStream::connect` anywhere else. |
| [0018](0018-rpc-parity.md) | Parity: every capability is an RPC first, UI and CLI are thin clients | Accepted | Generated parity table; an RPC without a CLI row breaks the build, a UI gap is only a warning. |
| [0019](0019-seatbelt-on-macos.md) | Seatbelt is the sandbox on macOS, a Linux VM comes later | Accepted | `sandbox-exec` with a generated SBPL profile per session; the three guarantees restated for macOS and measured by the same shim. |

## Cross-references

Where one decision refines or corrects another:

| From | To | Relationship |
|---|---|---|
| ADR-0006 | ADR-0001 | Resolution after allow determines how the proxy builds upstream connections. |
| ADR-0017 | ADR-0006 | Formalises the connection setup as a port and pins the IP from ADR-0006. |
| ADR-0007 | ADR-0006 | `allow_private: true` is the named exception to the block on private target addresses. |
| ADR-0005 | ADR-0004 | Fixes the status codes per `BlockReason` and the uniform block body. |
| ADR-0014 | ADR-0005 | Extends the block body with `note:` and the header `X-Humanitl-Note`. |
| ADR-0015 | ADR-0001, ADR-0002, ADR-0008, ADR-0017 | Fixes where these adapters may live and that a second adapter does not touch the core. |
| ADR-0016 | ADR-0002 | Corrects the statement "`AF_UNIX` always forbidden": in the profile `browser` it is allowed, and there the network namespace carries the guarantee. |
| ADR-0018 | ADR-0003, ADR-0013 | Makes the equality of UI and CLI from ADR-0013 machine-checkable. |
| ADR-0019 | ADR-0002, ADR-0015 | The second adapter of `SandboxBackend` that ADR-0015 announced; on macOS it takes the place of bwrap from ADR-0002. |
| ADR-0011 | ADR-0007 | Governs the two storage locations for rules (saved and session). |
| ADR-0012 | ADR-0010 | `FixAction::InstallService` is the mechanism behind the one-click installation. |

The largest correction within a decision stands in ADR-0009, and it went back
and forth twice on 2026-09-04. HUM-035 revised the choice of the component
library and rejected `shadcn_flutter`; the project owner reversed that on the
same day, because the original version of the ADR was his requirement and an
issue may not overturn it without asking. The section "Revised on 2026-09-04
by the project owner" applies: `shadcn_flutter` is adopted, pinned and used
exclusively in `app/packages/ui`. The section "Decision after Sprint 2" stays
as history, because its measurements still hold; the sections `Decision`,
`Rationale`, `Rejected alternatives` and `Consequences` carry banners pointing
to the current state. The rest of the decision — Flutter, the encapsulation,
no WebView, one window with docked panes — stands unchanged.

Further corrections within a decision: ADR-0005 records that
`Expect: 100-continue` is answered with `100 Continue` **immediately**. The
earlier wording "only after the decision" would have made buffering the body
impossible. ADR-0004 records accordingly that the transition method
**produces** the event and does not receive it — the earlier signature
`on(self, &FlowEvent)` from `docs/ARCHITECTURE.md` 3 is obsolete.

## Checking

```sh
docs/adr/check.sh              # checks this directory
docs/adr/check.sh --self-test  # proves that every check fires on a broken copy
```

The script checks that every Markdown file except the index follows the
pattern `NNNN-kebab-title.md`, that the numbers are contiguous and unique, that
every file is in the index and every index entry has a file, that status and
date are present, that every file carries the seven headings and that every
one names at least one `HUM-` issue. `--self-test` creates thirteen copies
with exactly one defect each (wrong file name, gap, duplicate, missing
heading, lowercase status, …) and expects the matching message for each.
