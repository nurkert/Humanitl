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
| [0020](0020-podman-sandbox-backend.md) | Podman as a selectable second sandbox backend on Linux | Accepted | One `SandboxBackend` made neutral, Podman behind it with a shared-kernel level (`crun`), the kernel level moved to ADR-0021; `sandbox.backend` per user, a raise-only `sandbox.min_isolation` per project. |
| [0021](0021-qemu-microvm-kernel-level.md) | The kernel level runs as a QEMU micro-VM from Debian packages | Accepted | Backend `qemu` instead of the runtime `krun`: QEMU `-M microvm`, virtiofsd and vhost-device-vsock, each in a bwrap jail inside a hardened user unit; exactly one vsock port reaches the proxy. |
| [0022](0022-decision-proxy-behind-humanitl.md) | A decision proxy of the user's choice can stand behind Humanitl | Accepted | Egress adapter `HttpProxy` (ADR-0017) with `CONNECT` to the address Humanitl resolved and checked, TLS opened in Humanitl and again in that proxy; `hold.ask_mode = "upstream"` forwards requests without a rule instead of holding them. |
| [0023](0023-agent-depot.md) | Agent depot: an agent installed and logged in once, inside Humanitl only | Proposed | Optional store per agent under the daemon's data directory (`agents.source = "depot"`, default `"host"`); program read-only and configuration as a discarded copy in every session, only the checked credential file flows back into the depot; install and login run in a sandbox. |
| [0024](0024-terminal-client-and-start-window.md) | A start window, a terminal client for settings and views, and sessions that outlive their terminal | Accepted | Project register and settings catalog in the daemon, rendered by the app's start window, the session sheet and a ratatui TUI in `humanitl`; `humanitl go [AGENT]` with a first-start screen, a prefix key table for views, detach on close. Addendum 2026-11-23 (#14): what the build changed, among others the redraw by a resize to one row less and back, `terminal.direct_keys`, the migration marker written by the CLI, a rejoined `go` that never stops. |
| [0025](0025-settings-model-levels-presets-sections.md) | Settings show five levels, a narrow session level and three presets, in nine sections | Accepted | Default, Mine, This project, Only this session, Company as a mapping of the existing origins plus a preset layer; every project command writes the project choice; session level for model server, isolation (stricter only), folder mode and network mode; presets Strict (factory, `sandbox.min_isolation = "highest"`), Balanced, Open; nine catalog sections with a simple and an expert view; `config set` requires its level. |
| [0026](0026-session-status-and-why-chain.md) | A status per session and a why chain from an agent failure to its cause | Accepted | RPC `Sessions` streams one complete snapshot per session (holds with the nearest deadline, agent, model server with level and reach, isolation and minimum, folder and network mode, waiting changes, last block, expiries) for a status line and a chip row; RPC `Why` returns the ordered steps from policy to result with a `Diagnostic` and a narrow rule suggestion; a passed deadline is never silent; the agent reads only a coarse vocabulary (`not_permitted`, `timeout`, …) and a fixed `detail:` line. |
| [0027](0027-rule-content-conditions.md) | Rules gain content conditions that can narrow a rule and never widen one (rule format 2) | Accepted | Optional `when:` beside `match` with body present or absent, size bounds, Content-Type, header and query present or absent, `query_equals`, finding classes, `body_contains` (`aho-corasick`) and `body_matches` (`regex::bytes`, linear time); each condition holds, fails or is undetermined; first match wins stays, a failing condition skips the rule, an undetermined one blocks a `block` rule and holds any other, also during a pause, never allows; file stays `version: 1` so an older daemon degrades per rule; older clients cannot change a conditional rule; fixed limits and a 100 ms budget; the agent sees only `conditional`. |
| [0028](0028-cards-explain-waiting-requests.md) | A request that waits for a decision gets a fixed head, and two kinds get a card that explains and never decides | Accepted | A fixed head for every such request (A-label host, method, scheme, total size, cookie and authorization present, findings above all); RPC `Interpret` serves a pure, deterministic renderer in `humanitl-render` without network or outside knowledge; every byte is head, explained field or remainder, a card only up to 64 unexplained bytes; allowlists `web-search` (Google, Bing, DuckDuckGo, Brave) and `package-download` (npm, PyPI, crates.io), `GET`/`HEAD` only, fail closed to the raw view; no trust signal; raw view one key away and the default with findings, marks or a first contact; `hold.cards` turns cards off; the decision is bound to `request_sha256`, and `flow.decided` names renderer, version and view. |
| [0029](0029-tools-on-request-off-by-default.md) | The agent asks for a tool from a signed catalog, a person decides, and the tool arrives as a sealed read-only layer; off by default | Accepted | `tools.enabled` off by default and on only per project; the agent asks over `humanitl.internal` for an entry of a closed catalog (Ed25519 over the exact bytes, as the self-update, monotonic serial, expiry, revocation) and reads one of `pending`, `ready`, `denied`, `unavailable`; downloads by the background service from six fixed hosts, never because of a request; unpacking in a sandbox without a door, sealing on the host, a content-addressed read-only `nosuid,nodev` layer at `/opt/humanitl/tools`, last on `PATH`, collisions refused, mounted at session start; a card with deny as the default, bound to a plan digest, approvals only in the register; 25 Linux entries; no new port; design in `docs/design/tools-on-request.md`. |
| [0030](0030-several-sessions-per-daemon.md) | One background service runs several sessions, each with its own listener, state and limits | Accepted | A session id per start instead of per daemon process; one proxy socket per session at `proxy/<session-id>/proxy.sock`, the session of a flow is the listener that took it, never a tag; settings, bundled and session rules, holds, decisions, pause, meta answers and caches per session, shared only the user's configuration, the CA, recorder, one audit chain and read-only content; `limits.max_sessions` (default 4, company can lock lower), a hold reserve per session with admission at start and a descriptor budget; sprint 9 puts `session_id` on every `FlowEvent` and keys status line, bell, tray, notifications and session list by session; five build stages for sprint 10 with the statements for `SECURITY.md` and `THREAT-MODEL.md`; owner decisions of 2026-11-24 on the default, one session per folder and the pause per session. |

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
| ADR-0020 | ADR-0002, ADR-0010, ADR-0015 | Podman instead of Docker as the second Linux backend; amends all three (addenda of HUM-434 and HUM-441). |
| ADR-0021 | ADR-0020, ADR-0002, ADR-0010, ADR-0015 | Replaces the kernel level of ADR-0020 (Podman with `krun`) by an adapter of its own; the plain level stays as it is. Amends ADR-0002, ADR-0010 and ADR-0015 once the level is admitted (addenda of HUM-447). |
| ADR-0023 | ADR-0018, ADR-0015 | The depot is controlled only through RPCs (ADR-0018) and mounted through the neutral `LaunchPlan`; no new port (ADR-0015). |
| ADR-0011 | ADR-0007 | Governs the two storage locations for rules (saved and session). |
| ADR-0025 | ADR-0011, ADR-0020, ADR-0024 | Makes the tiers of ADR-0011 drive a simple and an expert view, keeps the raise-only floor of ADR-0020 with a preset as its bottom, and extends the project choice and the catalog of ADR-0024. |
| ADR-0012 | ADR-0010 | `FixAction::InstallService` is the mechanism behind the one-click installation. |
| ADR-0026 | ADR-0005, ADR-0014, ADR-0018, ADR-0024, ADR-0025 | Adds a fifth line `detail:` and an agent vocabulary to the block body of ADR-0005 and the answer of `/why` of ADR-0014; both RPCs follow ADR-0018; the status line is drawn by the client of ADR-0024 between filtered chunks; status values carry the levels of ADR-0025, whose note of network mode Block becomes the `detail:` line. |
| ADR-0027 | ADR-0005, ADR-0007, ADR-0014, ADR-0018, ADR-0025, ADR-0026 | Adds content conditions to the rule model of ADR-0007 with first match wins unchanged; a rule with `when` never streams over the cap of ADR-0005; the rule listing of ADR-0014 marks it `conditional`; new options follow ADR-0018; a repository profile keeps the `block`/`ask` limit of ADR-0025; `RuleStep` of ADR-0026 gains condition results and skipped rules. |
| ADR-0028 | ADR-0018, ADR-0025, ADR-0026, ADR-0027 | `Interpret` and the field `seen` follow ADR-0018; `hold.cards` is a setting of ADR-0025 that a project profile may only turn off; adds `BlockReason::RequestChanged` to the agent table of ADR-0026 point 5 and the fields of point 8 to its `HoldStep`; findings, also those ADR-0027 conditions read, stand above every card and force the raw view. |
| ADR-0029 | ADR-0014, ADR-0015, ADR-0017, ADR-0018, ADR-0023, ADR-0025, ADR-0026 | Adds the paths `/tools` to the meta host of ADR-0014 and the four tool states to the agent vocabulary of ADR-0026 point 5, amending its "no new path"; downloads through the egress `Direct` of ADR-0017; no new port under ADR-0015 (the broker uses `Egress`, `SandboxBackend` and `AuditSink`); RPCs follow ADR-0018; shares the tree checks of the depot of ADR-0023 and none of its state; `tools.enabled` is a project-choice key of ADR-0025 that the repository file may only switch off. |
| ADR-0030 | ADR-0018, ADR-0022, ADR-0024, ADR-0025, ADR-0026, ADR-0029 | Carries out point 12 of ADR-0024 (several sessions) and keeps its point 10 (a start in a running session's folder attaches); builds on the per-session shape of `Sessions` and `Why` of ADR-0026 without changing it and adds `FlowEvent.session_id`; `limits.max_sessions` is locked by the company level of ADR-0025 point 9 and notifications follow its point 8; amends ADR-0025 point 6 by moving `limits.hold_max_flows` and `limits.hold_max_bytes` from the direction `lower` to `ignored`; the decision proxy of ADR-0022 sees all sessions without a marker; the tool links of ADR-0029 stay per session; every new option is an RPC (ADR-0018). |

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
one names at least one `HUM-` issue. `--self-test` creates fifteen copies:
two that must pass (unchanged, and `Status: Proposed`) and thirteen with
exactly one defect each (wrong file name, gap, duplicate, missing heading,
lowercase or unknown status, …), and expects the matching message for each.
