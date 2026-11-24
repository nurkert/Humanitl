# ADR-0030 · One background service runs several sessions, each with its own listener, state and limits

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-24

## Context

The background service runs one session at a time. GitHub #16 (HUM-549,
candidate since sprint 8) asks for several at once, so that a person can keep
two agents in two projects running, detach from both and attach to either,
the way `screen` keeps several windows. The owner decided for sprint 9 that
this is designed now and built in sprint 10. Sprint 9 builds the status RPC,
the status line, the chip row, the tray, the notifications and the session
list (ADR-0026, tracking issue #86); they must not need rebuilding when the
second session arrives.

What exists today, on `main` at `a02ad4e8`:

- **A session id belongs to the daemon process, not to a start.**
  `humanitld` creates one `SessionId` at start
  (`daemon/bin/humanitld/src/main.rs`, `let session = SessionId::new()`) and
  uses it for the recorder row (`recorder.start_session`), the audit records
  `session.started` and `session.ended` (`start_audit`, `stop_audit`), the
  rule store (`load_rules`), the proxy listener (`proxy.start_session`) and
  the `SandboxService`. Every sandbox the daemon starts in its lifetime runs
  under that one id; the run itself has its own `sandbox_id`.
  `backlog/CONVENTIONS.md` 3.8 states it ("a daemon process has exactly one
  `SessionId` but starts any number of sandboxes in it"), and 4.30 relies on
  it (`/why` answers `404` after a restart). The glossary
  (`docs/GLOSSARY.md`) defines a session differently: "one agent running in
  one sandbox, from its start until it ends". A rule "for the session"
  (`Expiry::Session(SessionId)`) therefore outlives the sandbox it was made
  in and holds for the next start in the same daemon run.
- **A second start is refused.** `Inner::claim_start`
  (`daemon/crates/ipc/src/sandbox.rs`) takes one claim under one lock;
  `running: Mutex<Option<Running>>` holds one handle. A second start gets
  `CLI_005` (`second_start_refused`), whose fix since GitHub #13 names
  `humanitl go --dir FOLDER`. A `go` in the folder of the running session
  attaches to it instead (ADR-0024 point 10). The `Inner` also holds one
  `config`, one `disposable` slot of waiting changes, one `project` and one
  `stopped_by`.
- **The proxy is already keyed by session.** `ProxyCore`
  (`daemon/crates/proxy/src/core.rs`) keeps `DashMap<SessionId,
  SessionProxy>`, one socket path and one accept loop per entry, and every
  accepted connection carries `ConnectionContext::plain(session)` from the
  listener that took it. Only one entry is ever started, at the fixed path
  `Paths::proxy_socket()` = `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock`,
  which every backend binds into its sandbox as `/run/humanitl/proxy.sock`
  (`LaunchPlan.proxy_socket_src`).
- **Shared per daemon today:** one `HoldQueue` with `limits.hold_max_flows`
  (200) and `limits.hold_max_bytes` (256 MiB) over all waiting flows; one
  `RulesStore` with a saved group, a bundled group that each start replaces
  (`store.set_bundled`) and a session group; one `SessionSettings` (ask
  mode, deadline, model alias, LLM warning memory) that each start replaces;
  one `InterceptPause` "for the whole daemon", which a new session ends;
  one `MetaEndpoint` whose ask counter is already a `HashMap<SessionId, _>`;
  one `ResolverPort` with one answer cache; one `LeafCache` of TLS leaf
  certificates; one CA; one egress route; one recorder database; one audit
  chain (`audit.jsonl`), whose records each carry `session`.
- **Events carry the session only in part.** `FlowSummary.session_id` exists,
  but `FlowEvent.Held` carries `flow_id`, `deadline` and the daemon-wide
  `queue_count` and `queue_bytes`, and no session. The terminal client rings
  its bell on every `Held` of the daemon's `Subscribe` stream
  (`daemon/bin/humanitl/src/cmd/attach.rs`). `Info.session_id` is the
  daemon's one id. `Sessions` and `Why` (ADR-0026) already carry a
  `session_id` in every message and list every session of the daemon.
- **Per session already:** the depot scratch (`agent/depot/scratch.rs`,
  removed at the end of the session), the credential watch
  (`agent/depot/writeback.rs`), the link directory of the tools
  (`tools/sessions/<id>/bin/`, ADR-0029), the QEMU vsock device process
  with its own group and the cap of 64 door connections (K-31),
  `limits.max_client_connections` (256) per session, and the terminal hub
  with its filter.
- **The budget question is already open.** HUM-479 (`backlog/sprint-7.md`)
  decided that no shared cap of door connections is needed while one
  session runs, and left to HUM-549 a cap on sessions, the daemon's file
  descriptor limit (the unit sets no `LimitNOFILE`; systemd's soft default is
  1024, and three full doors with their proxy connections reach it) and, if
  needed, a guaranteed share per session with a test that a full session
  does not lock another's door.
- **A company level without a file.** The origin `Policy` exists and
  resolves (ADR-0025 point 9); where the policy file lives is a later issue.

The owner set the frame on 2026-11-23: secure by default; behaviour is
fixed and presentation is flexible; design in sprint 9, build in sprint 10;
sessions never share a proxy listener's identity, a hold, a decision or a
mount, and nothing of one session leaks to another. "Item N.M" is the
numbering of sprint 9's tracking issue, as in ADR-0025 and ADR-0026.

Hard constraints: the three sandbox guarantees, `tests/escape/` and the
terminal output filter stay as they are. Every capability is an RPC first
(ADR-0018). No new port (ADR-0015). The daemon serves one user; the sessions
of one daemon all belong to that user, and the boundary this decision draws
is between the agents of two sessions, not between two people.

## Decision

We make a session what the glossary says it is, one start of one agent in
one sandbox with an id of its own, and run up to `limits.max_sessions` of
them in one background service. Each session gets its own proxy listener,
its own settings, rules of the session, holds, decisions, pause and mounts;
the daemon tells them apart only by the listener a connection arrived on.
What the sessions share is named in point 1 and is either the user's own
configuration or read-only content. Sprint 9 puts the session id on every
status and hold event; sprint 10 builds the rest in five stages.

### 1. What "several sessions" means for each part

**Session identity.** `Sandbox{start}` creates the `SessionId`; the daemon
process no longer has one. A session and its sandbox are one to one; both
ids stay, because backends name units, containers and VMs by `sandbox_id`.
Records of the daemon itself (`daemon.started`, `daemon.stopped`, a rule
change from the host, `intercept.*` before stage 2) carry
`humanitl_audit::NO_SESSION` as they do today when no session applies.
`Info.session_id` names the running session while exactly one runs and is
empty otherwise; clients read sessions from `Sessions`. The process keeps an
instance id of its own, created once at start, for what today compares
against `Info.session_id`: `InterceptStatus.instance` and the revision of the
pause.

**Proxy: one listener per session, no tagging.** Every session gets its own
Unix socket at `$XDG_RUNTIME_DIR/humanitl/proxy/<session-id>/proxy.sock`, in
a directory of its own with mode `0700`, the socket file itself `0600`
(`SOCKET_MODE` in `daemon/crates/proxy/src/listener.rs`), created by
`ProxyCore::start_session`
before the sandbox starts and removed when the session ends. The backend
binds that **file** into the sandbox at the unchanged path
`/run/humanitl/proxy.sock`; never the directory, never the parent `proxy/`.
The session of a flow is the `ConnectionContext.session` of the accept loop
that took the connection. No header, host name, path, source address or
peer credential the sandbox controls can name or change it. Each listener
gets a `FlowHandler` built over that session's state (below), so the
pipeline never looks a session up by anything the request says.

**Per session, never shared** (a `SessionContext` the start builds and the
end drops):

| State | Today | Per session from sprint 10 |
|---|---|---|
| Settings the proxy reads per flow | one `SessionSettings`, replaced by each start | one per session: ask mode, deadline, model alias, LLM warning memory |
| Rules | bundled and session group of the one `RulesStore` | the bundled group (profiles, passthrough to the session's model server) and the session group per session; `Expiry::Session(id)` ends with that session |
| Holds | one `HoldQueue` | one queue; every hold carries its session, and the quota of point 3 reserves a share per session |
| Decisions | by flow id | by flow id; a `Decide` batch must lie in one session, otherwise a new `IPC_` code (the next free number, `IPC_008` today; `IPC_002` stays "AllowEdited only for exactly one flow"); a notification action carries session and flow and is refused on a mismatch |
| Pause of intercepting | one `InterceptPause` per daemon | one per session; it ends when that session ends; pausing several sessions is one pause each, each audited |
| Meta endpoint | one `MetaEndpoint`, ask counter per session | `GET /` lists the saved rules, this session's bundled and session rules and none of another session; `GET /why/<flow-id>` and `/tools` answer for flows and entries of the asking session only, and a flow of another session gets the same `404` as an unknown one |
| Pseudonyms | per session in memory (`docs/SECURITY.md` 5.6 item 7) | unchanged, never shared |
| First contact (`DomainInfo.seen_count`, ADR-0028) | per daemon run | per session |
| Resolver answers | one cache in `ResolverPort` | the cache keyed by session and host; configuration and counters stay one |
| TLS leaf certificates | one `LeafCache` | one per session with the same bound; the CA stays one |
| Upstream connections | none pooled | stays so; a future pool is keyed by session and authority |
| Waiting changes of a working copy | one `disposable` slot | one per project, keyed by project id |
| Terminal | one hub | one hub per sandbox, filter unchanged |
| Depot scratch, credential watch | per session | unchanged; a write-back whose base no longer matches the depot (another session wrote back first) is refused, never merged or overwritten |
| Tool links (ADR-0029) | per session | unchanged |

**Shared on purpose**, because it is the user's own configuration, host-side
evidence or read-only content: the daemon socket and its token; the CA and
its key (only `ca.crt` reaches a sandbox); saved rules; `config.toml`, the
profiles and the project register; the egress route and the decision proxy
behind (ADR-0022), which sees requests of all sessions and gets no marker
of which; the recorder database; the audit chain; the depot's program trees
and sealed tool layers, bound read-only; the read-only system binds of each
backend. Nothing writable is bound into two sandboxes.

**Sandbox backends.** Each session is a sandbox of its own at its own
isolation level, resolved through all setting levels at its start, with
`sandbox.min_isolation` checked per session:

- bubblewrap: one `bwrap` with its own namespaces, its own runtime directory
  and its own socket file;
- Podman: one container in its own transient unit `humanitl-*` of the
  user's service manager, with a unit and container name unique per
  sandbox;
- QEMU: one micro-VM with its own vsock device process, group and door, the
  door forwarding to that session's socket; the cap of 64 connections stays
  per session;
- Seatbelt on macOS: one generated profile per session that names only its
  own socket file, its own door port and its own server port, all chosen
  free per session (K-19).

**Recorder.** One `sessions` row per start (with `project_id`, agent,
folder, start and end), so `flows.session_id` points to the session that
sent the flow. `session_summaries` stays per sandbox run, now one to one with
the session. A migration keeps old rows as they are: a row of an old daemon
run that covered several sandboxes stays one session in the history.

**Audit chain.** One chain per daemon, one writer, one key, one anchor
sequence. Records of all sessions interleave in the order the writer gets
them; each carries its `session`. `session.started` and `session.ended` are
written per session at its start and end, `daemon.started` and
`daemon.stopped` once per process. `humanitl audit verify` checks the whole
chain; `humanitl audit export` gains `--session ID`, a filter applied
after the check.

**Project register.** At most one running session per project. A `go`, a
start from the start window or `Sandbox{start}` in the folder of a running
session attaches to it (ADR-0024 point 10, unchanged); a start in another
folder starts a second session instead of `CLI_005`, up to the limit of
point 3. `Projects.list` names the running session per project from a map,
not from one slot. Cleanup refuses a project whose session runs, as now.

**The three guarantees** stay word for word and hold per sandbox: no
network interface, exactly one door (that sandbox's socket, which leads to
that session's listener), no new door. `tests/escape/` runs unchanged
against each backend. That two sandboxes cannot reach each other follows
from the guarantees (no interface, one socket each, which leads only to the
daemon); sprint 10 adds tests for the daemon side of it (point 5, stage 4),
not a new guarantee.

### 2. Holds, the bell and notifications belong to their session

- **Holds.** A hold is created by the pipeline of the session's listener and
  carries that `SessionId` in `FlowSummary.session_id` and in every
  `FlowEvent` (point 4). Its deadline is that session's `hold.timeout_secs`;
  an expired deadline blocks (ADR-0026 point 4) and counts in that session's
  `expiries`, never in another's.
- **Who decides.** The app's Decisions view lists the waiting requests of
  all sessions, grouped by session with the project name as group title; a
  card belongs to one flow, and "allow for the session" writes a rule of
  that flow's session. The terminal client's interceptor view and
  `--ask terminal` list and decide only the flows of their own session.
  A `Decide` batch across sessions is refused with the new `IPC_` code of
  point 1. `humanitl
  intercept pause` and `resume` take `--session`; without it they act on
  the session of the current folder, else on the only running one, else
  they refuse and list the sessions.
- **The bell.** A terminal client attached to session S rings `BEL`
  (`terminal.bell_on_hold`, ADR-0024 point 11) only for a `Held` whose
  `session_id` is S. Holds of other sessions show in its status line as
  "+N in other sessions" from `DaemonStatus.holds` (ADR-0026 point 1);
  they never ring.
- **Notifications** (`ui.notifications`, ADR-0025 point 8):
  - `count` (default): one notification for the daemon, the sum: "3
    requests wait for a decision", and with more than one session "… in 2
    sessions". No project name, no host.
  - `content` and `actions`: one notification per session, replaced in
    place, with the project's display name, host, method and path. An
    action decides exactly the flow it shows; it sends session and flow, and
    the daemon refuses it if the flow is not of that session or no longer
    waits.
  - `off`: none; status, tray and history still show everything.
- **The tray** shows the sums of `DaemonStatus`; its menu lists the
  sessions from `Sessions` with their count of waiting requests.

### 3. Limits, resources and what a company can restrict

- **`limits.max_sessions`**, default `4` (owner, 2026-11-24), range `1` to
  `16`, tier `advanced`,
  `x-project-scope = "denied"`, not part of the project choice and not a
  session-level key. A start above it is refused with a new `LIMIT_` code
  (the next free number, `LIMIT_007` today) whose fix lists the running
  sessions and the command to stop or attach to one. `1` is today's
  behaviour.
- **Hold quota with a reserve per session.** `limits.hold_max_flows` and
  `limits.hold_max_bytes` stay daemon-wide totals. The totals and the
  reserves are computed only from the daemon's own levels (default,
  `config.toml`, company). This is a deliberate tightening: today a project
  profile may lower both keys (`x-project-scope = "allowed"` in
  `daemon/crates/config/src/model.rs`; the test
  `the_rest_of_limits_stays_open_to_a_project_profile` in
  `daemon/crates/config/tests/project_scope.rs` loads
  `limits.hold_max_flows = 50` from one), and ADR-0025 point 6 lists them
  under the direction `lower`. From stage 3 both move to the direction
  `ignored` of that table: a value from the repository file or from a
  profile it selects is reported with ADR-0025's warning ("from the
  repository, ignored") and not applied, and it never refuses a start. The
  project choice and the session level do not carry them. A per-session
  lowering of the hold cap is dropped: a session's cap is its reserve plus
  what it may borrow, nothing else. Stage 3 changes that test (it keeps
  `recorder.inline_max_bytes` and moves the hold key to an "ignored with a
  warning" case) and the direction table of ADR-0025 point 6 gets an
  addendum naming this ADR.
  `limits.max_sessions`, `limits.hold_max_flows` and `limits.hold_max_bytes`
  take effect after a restart of the background service (`effect =
  restart`, ADR-0025), so the totals and `max_sessions` never change under
  running sessions.
  Each session's reserve is fixed at its start as `R = total /
  max_sessions`, for flows and for bytes, and kept until it ends. One rule
  governs both directions:
  - A session may hold a request while it uses less than `R`, or while the
    total in use plus the unused reserves of all other running sessions
    stays below the total. It may therefore borrow only from capacity that
    no running session has reserved.
  - A start is admitted only when the capacity that is neither in use nor
    reserved by a running session is at least `R`. Otherwise it is refused
    with the `LIMIT_` code below, whose why names the waiting requests of
    the full session and whose fix is to decide them or wait for their
    deadline. A session that borrowed up to the total thus delays the next
    start; it never takes the reserve of a session that already runs.
  - Within its reserve a running session's hold never answers `503`
    (`hold_max_flows`, `hold_memory`) because of another session; beyond
    its reserve it can, as today when the queue is full.
  - `validate.rs` refuses a configuration whose byte reserve
    (`hold_max_bytes / max_sessions`) is below `limits.hold_body_cap_bytes`
    or whose flow reserve is below 1, with the existing configuration codes.
    With the defaults (256 MiB, 32 MiB) that allows up to 8 sessions; more
    need a larger `hold_max_bytes`.
- **File descriptors.** The daemon sets its soft `RLIMIT_NOFILE` to the hard
  limit at start, and the unit gets an explicit `LimitNOFILE=32768`. Each
  session is budgeted at `2 × max_client_connections + 4 × 64 + 64`
  descriptors (proxy connections in and out, the kernel level's door, the
  rest), computed from the `limits.max_client_connections` resolved for
  that session at its start; with the default 256 that is 832. The daemon
  keeps 512 for itself. 16 sessions at the default need 13,824. A start
  whose budget, added to the budgets of the running sessions and the 512,
  would exceed the soft limit is refused with the same `LIMIT_` code before
  anything is created. The door cap of 64 per session
  (K-31) stays per session; no shared door cap is needed, because each door
  is bounded alone and the descriptor budget covers their sum.
- **Memory.** Every session keeps its existing bounds (hold reserve, body
  caps, the kernel level's unit with `MemoryMax`). A kernel-level start is
  refused with the same `LIMIT_` code when the guest memory of the running
  kernel-level sessions plus its own exceeds the `MemAvailable` the host
  reports, and also when `MemAvailable` cannot be read. The check runs under
  the same lock as the claim of the start, so two starts cannot both pass
  it on the same free memory. `limits.registry_max_finished_flows` stays one bound for the
  daemon.
- **Company policy.** The company level (origin `Policy`) can lock
  `limits.max_sessions` to a lower value, `1` included. A lock below the
  number of sessions running when it arrives ends none of them; every new
  start is refused until the count is below the lock. Every other lock
  applies to every session, because each session resolves through the same
  layers at its own start; no session level and no project choice can raise
  above a lock. A company cannot see or decide one user's sessions from
  outside; the daemon socket stays the only way in.

### 4. What sprint 9 must already carry

None of this is built in sprint 9, but every sprint-9 surface keys its
state by session, so that sprint 10 only adds sessions:

1. **`FlowEvent.session_id`** (`string session_id = 20`, outside the
   `oneof`), set on every event that belongs to a session: `received`,
   `analyzed`, `held`, `decided`, `forwarded`, `response_headers`,
   `response_chunk`, `recorded`, `timed_out`, `failed`, `flow_diagnostic`,
   `agent_ask`, and a `diagnostic` that a session's listener produced. Empty
   only on events of the daemon (`lagged`, `config_changed`,
   `open_in_app`, `rules_changed` of the saved group). Built with item 1.2
   in the contract change of phase 1. A client treats an empty `session_id`
   on a flow event of an older daemon as "the only session".
   The same contract change adds `string session_id = 5` to
   `InterceptStatus`, so that `Intercept{status}` and `FlowEvent.intercept`
   need no second change in stage 2. Until stage 2 the daemon leaves it
   empty, and clients treat an empty value as a pause of the whole daemon,
   which it is until then.
2. **Item 1.2, the status RPC**, decided in ADR-0026 point 1 and built in
   sprint 9: one snapshot
   per `session_id`, `ended_session_id` when a session ends, sums in
   `DaemonStatus`. No client derives "the session" from `Info.session_id` or
   from `SandboxEvent.Status`.
3. **Item 1.3, the status line**, filters `holds`, `last_block` and
   `expiries` by its own session and takes "+N in other sessions" from
   `DaemonStatus`; the bell rings only on a `Held` with its own
   `session_id`.
4. **Item 1.4, chip row and tray**: chips show the attached session; the
   tray shows `DaemonStatus` sums, never one snapshot's numbers.
5. **Item 1.5, `Why`**, decided in ADR-0026 point 3 and built in sprint 9:
   `LastFailure.session_id`; with
   several sessions and none named, the answer lists them.
6. **Item 3.5, notifications**: under `count` the sum from
   `DaemonStatus.holds`; under `content` and `actions` one notification per
   session id, and every action carries session and flow.
7. **Item 4.2, the session list**: rows come from `Sessions`, one per
   `session_id`, with the four states; `humanitl sandbox attach` without an
   argument takes the session of the current folder, else the only running
   one, else refuses and lists them. The same rule for `humanitl why`
   without `--session`.
8. **The Decisions view** (app and the interceptor view) groups by
   `session_id` and sends `Decide` batches per session only.

### 5. Staged build in sprint 10, and what the security documents must say

Each stage lands on its own, with tests, and keeps `limits.max_sessions`
at an effective `1` until stage 5; tests of stages 3 and 4 set it higher in
their own configuration. The cross-session tests, the security documents
and K-40 land in stage 4, before the default changes in stage 5. Every stage
that changes a statement in `docs/SECURITY.md` changes the paired entry of
`docs/THREAT-MODEL.md` in the same commit; the pairs stand in the last
column.

| Stage | What | Tests | Statements (SECURITY and THREAT-MODEL) |
|---|---|---|---|
| 1. Session per start | `SessionId` from `Sandbox{start}`; recorder row and `session.started`/`ended` per start; `Expiry::Session` ends with the sandbox; `Info.session_id` and the instance id as in point 1; `CONVENTIONS.md` 3.8 and 4.30 updated | a rule "for the session" no longer applies after a restart of the sandbox in the same daemon run; two starts give two recorder rows and two `session.started` | section 8 with K-14 and K-11 |
| 2. Per-session state and listener | `SessionContext` (table of point 1), socket at `proxy/<session-id>/proxy.sock`, `FlowHandler` per listener, pause per session, `InterceptStatus.session_id` filled | `GET /` and `/why` of session A never show B's rules or flows (two contexts in one daemon); the pause of A leaves B held; a request through A's socket carries A's id end to end | sections 1 and 3 with B2 and K-03; section 6 "Pausing intercept" with K-26; section 5.6 item 7 with K-11 |
| 3. Plural sandbox service | `running` as a map by sandbox id, claim per project folder, waiting changes per project, terminal hubs per sandbox, the quota, the descriptor budget and the memory check of point 3, the new `LIMIT_` and `IPC_` codes with their ARB titles and `diagnostic_l10n.dart` entries; `max_sessions` effective `1` | B starts while A is full: refused with the `LIMIT_` code, admitted once A's holds are decided, and from then on A's borrowing never makes B's holds within its reserve answer `503`; a start over the descriptor budget is refused before any resource exists; two kernel-level starts racing for the same free memory: one is refused; two sessions of one depot agent: the second write-back with a stale base is refused | section 6 "Service hardening" (`LimitNOFILE`) with K-31 and K-38 |
| 4. Cross-session tests and documents | the escape suite unchanged, run in CI with two sessions alive on each Linux backend; the new section of SECURITY 6, B5 and K-40 | two sandboxes of different backends running at once, each sees exactly one socket and it is its own; every attempt listed under K-40 fails | section 6 "Several sessions in one background service" with trust boundary B5, K-40 and the residual risks |
| 5. Clients and the default | `max_sessions` default `4`; start window, `go`, `attach`, `why`, `intercept pause` with sessions; app tabs; notifications per session; `CLI_005` only for the claim race | screenshot and design check; a second `go` in another folder starts a session, in the same folder attaches | section 6 "A session without a terminal" with K-39 |

Statements `docs/SECURITY.md` will need (stage in brackets):

- **Section 1, the claim** [2]: "Its only ways in and out are the project
  directory `/work` and exactly one Unix socket to the proxy" stays; add
  that each session's socket is its own and leads to that session's
  listener, and that no sandbox contains another session's socket.
- **Section 3** [2]: the host path becomes
  `$XDG_RUNTIME_DIR/humanitl/proxy/<session-id>/proxy.sock`, mode `0700`
  per directory and `0600` for the file; the file is bound, never its
  directory or `proxy/`.
- **Section 6, "Pausing intercept"** [2]: the pause belongs to one session
  and ends when that session ends; starting another session no longer ends
  it.
- **Section 6, "Service hardening"** [3]: the explicit `LimitNOFILE` and
  the descriptor budget per session.
- **Section 6, "A session without a terminal"** [5]: several sessions can
  run detached; a start beyond `limits.max_sessions` is refused.
- **Section 6, new "Several sessions in one background service"** [4]:
  the two lists of point 1 (separate and shared), the rule that a
  connection's session is its listener, the quota reserve and the
  descriptor budget, the company lock.
- **Section 5.6 item 7** [2]: pseudonym mappings are per session and never
  shared.
- **Section 8** [1]: one chain for all sessions, every record names its
  session, `session.started`/`ended` per session.

Statements `docs/THREAT-MODEL.md` will need:

- **Trust boundaries** [4]: a boundary B5 between the agents of two sessions
  of one daemon; the attacker is (a) or (b) in session A, the asset is
  everything of session B.
- **K-03** [2]: one socket directory per session.
- **K-11** [1, 2]: queue and rule metadata are per session; what the meta
  endpoint shows of rules and flows; pseudonym mappings per session, never
  shared.
- **K-14** [1]: the one audit file holds the records of all sessions, each
  with its session.
- **K-26** [2]: the pause per session.
- **K-31** [3]: N doors under one descriptor budget; the note of HUM-479
  closed.
- **K-38** [3]: the attacker gains a second way to exhaust memory, filling
  the shared hold queue; extended by the reserve per session, the admission
  rule of point 3 and its stage-3 test; `registry_max_finished_flows` stays
  one bound for all sessions.
- **K-39** [5]: several detached sessions.
- **New K-40, several sessions in one daemon** [4]: what A could try (read
  B's flows through `/why`, see B's session rules through `GET /`, fill the
  hold queue or the descriptors so B's requests fail, learn B's hosts
  through the timing of the resolver or leaf cache, reach B's socket or
  door, keep holds above its share so that later starts are refused with
  `LIMIT_007`), the mitigation per point, and the residual risk below. For
  the last: the deadline of each hold expires it, and the why of the
  refusal names the full session.
- **Residual risks** [4]: all sessions share one daemon process; whoever
  takes over the proxy has every session's flows. CPU and memory pressure
  of one session slows another; a session that keeps holds above its share
  blocks later starts with `LIMIT_007` until its holds are decided or their
  deadline expires them, and the why names that session; no timing side channel beyond the two
  caches is closed. On macOS a process outside every sandbox can still
  reach any session's door (K-19, HUM-331 open).

## Rationale

- **The listener is the only identity the sandbox cannot forge.** A tag in
  a header is written by the agent. A source address is the shim's loopback
  in every sandbox. Peer credentials show the same user for every sandbox
  under bubblewrap, and the vsock forwarder under QEMU. The accept loop
  knows which socket it serves before a byte arrives; `ProxyCore` already
  works that way, and `ConnectionContext.session` is set there today.
  Checkable: a test sends a request with every plausible session header
  through A's socket and finds it in A's queue.
- **A session per start removes a quiet surprise.** Today "allow for the
  session" survives a stop and a new start in another project inside the
  same daemon run. With an id per start the word means what the glossary
  and the UI say, and the rule's lifetime shrinks, which is the safe
  direction.
- **One audit chain is enough and stronger.** Separate chains per session
  would need a key and anchors each, and a deleted session file would leave
  no gap in any other chain. In one chain every session's records sit
  between anchors the database also holds; filtering by `session` is a view.
- **A reserve per session answers the HUM-479 note with a test.** Borrowing
  keeps a single session at today's 200 flows and 256 MiB when it runs
  alone. The reserve keeps a busy or hostile session from starving one that
  already runs; the admission rule moves the only remaining conflict to the
  start of the next session, where it is refused with a reason instead of
  failing requests later. The stage-3 test "B starts while A is full" shows
  both.
- **Default 4.** Two or three projects at once is the case the owner
  described; 4 leaves room and keeps the descriptor budget at 3,840 of
  32,768. The kernel level is bounded by memory, not by the count.
- **Sprint-9 surfaces key by session now.** A `session_id` on every event
  and a sum in `DaemonStatus` cost one field and a filter; adding them after
  the status line, tray and notifications are built would touch each again.

## Rejected alternatives

- **One shared listener with a session tag.** The tag would come from the
  sandbox (a header, a path prefix, a port the shim picks), so an agent
  could write another session's tag and land its requests in that
  session's queue under that session's rules. Rejected: a listener's
  identity must not depend on what the sandbox says.
- **One daemon per session.** Each session as its own `humanitld` with its
  own socket would separate perfectly, but the app and CLI would have to
  find and talk to N daemons, the audit chain and recorder would split, the
  CA and decision proxy would be duplicated, and ADR-0026's `Sessions`
  would become a client-side merge, which ADR-0018 forbids.
- **One audit chain per session.** See Rationale; more keys and anchors, and
  a removed session file leaves no gap elsewhere.
- **A CA per session** (`BACKLOG.md` 4.4 names it as an option). It would
  hide nothing between sessions, because the key never enters a sandbox,
  and it would change the trust installed in every sandbox. Kept as an
  option for later, not part of this decision.
- **A fixed share without borrowing** (200 / 4 flows per session). Simple,
  but it would cut a single running session to 50 waiting flows and 64 MiB,
  a regression for the common case.
- **Borrowing capped at the total minus the other reserves.** With
  `R = total / max_sessions` that cap equals `R`, so it is the fixed share
  under another name.
- **Several sessions in one project folder.** Two agents writing the same
  folder directly race each other; with working copies the second review
  would apply onto changes the first already applied. Left out; a second
  start in the same folder attaches (owner, 2026-11-24).
- **Do nothing**, keep `CLI_005`. It contradicts the owner's vision of
  screen-like sessions and leaves the "session" of the code different from
  the "session" of the UI.

## Consequences

- Sprint 9 adds `FlowEvent.session_id` and keys every new surface by
  session (point 4); nothing else of this decision is built before sprint
  10.
- Sprint 10 builds five stages (point 5), touches `humanitld` start-up, the
  proxy wiring, `SandboxService`, the rule store, the recorder schema, the
  audit kinds and every client, and updates `backlog/CONVENTIONS.md` 3.8 and
  4.30, `docs/SECURITY.md` and `docs/THREAT-MODEL.md` in the stages named.
- Behaviour change for users: a rule "for the session" ends with the
  sandbox it was made in (stage 1); a pause ends with its session and no
  longer with the next start (stage 2); a start can be refused while
  another session holds more than its reserve (stage 3).
- Amends ADR-0025 point 6: `limits.hold_max_flows` and
  `limits.hold_max_bytes` move from the direction `lower` to `ignored`, so a
  repository can no longer lower them (stage 3); `limits.max_sessions` is a
  new key with `effect = restart`.
- Risk: more state per session means more places to forget the session.
  Mitigation: the per-session state lives in one `SessionContext` that the
  handler is built over, so code in the pipeline has no way to reach
  another session's state; the cross-session tests of stages 2 and 4 send
  through two contexts in one daemon.
- Risk: N sessions multiply the descriptors, threads and memory one daemon
  binds. Mitigation: the budget of point 3, refused before a start creates
  anything; threads of the kernel level's door (at most 128 per session)
  are measured in stage 3, and the unit gets a `TasksMax` if the
  measurement asks for it.
- Risk: the decision proxy behind (ADR-0022) sees the requests of all
  sessions mixed. Accepted: a marker would reach the destination too; the
  history in Humanitl names the session.
- Doors kept open: a CA per session; several sessions in one folder once a
  working copy can be rebased; a company view over sessions, which needs the
  policy file (BACKLOG section 9, item 21) first.
- ADR-0024 point 12 ("one session per daemon stays for this sprint") is
  carried out by this decision; ADR-0026 point 1 needs no change.

## Owner decisions

The owner decided the three open points on 2026-11-24, each as recommended:

1. **Default of `limits.max_sessions`:** `4`; a company can lock it lower.
2. **A second start in the same project folder** attaches to the running
   session; there is never more than one running session per project.
3. **The pause of intercepting** belongs to one session and ends with that
   session.

## Affected issues

GitHub #85 (this decision) and #16 (HUM-549, the build in sprint 10), with
the note of `HUM-479` on a shared door cap and the descriptor budget. Builds
on ADR-0024 point 10 and 12 (`HUM-530` to `HUM-548`), ADR-0026 point 1
(GitHub #81) and ADR-0025 point 8 (notifications) and 9 (company level).
Sprint 9 items that must carry the session id: 1.2, 1.3, 1.4, 1.5, 3.5 and
4.2 (point 4). Sprint 10: stages 1 to 5 of point 5 as issues of their own
under #16.
