# ADR-0026 · A status per session and a why chain from an agent failure to its cause

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-24

## Context

Two things are missing from every surface of Humanitl, and the baseline of
sprint 9 ([`UX-BASELINE.md`](../UX-BASELINE.md), GitHub #78, W4, W7, W8 and
W12) names both:

1. **A status that is always visible.** What a session runs with is spread
   over five places: the isolation level stands in the header of the sandbox
   page, the folder mode in the first-start card, the network mode in
   `humanitl sessions`, the model server and its origin in
   `humanitl sandbox plan`, the agent's version and source in
   `humanitl agents status`. The terminal of a session shows none of it. It
   rings `BEL` for a new request that waits for a decision
   (`terminal.bell_on_hold`, ADR-0024 point 11) and shows nothing else: no
   count, no remaining time, no request whose deadline passed.
2. **The cause of a failure.** When a request of the agent does not reach its
   destination, the agent sees a status code and, at best, the four lines of
   the block body; the human sees a row in the History. Six different causes
   look alike from both sides: a rule or a person blocked, the decision
   deadline passed, the hard block of a checksum-confirmed secret, a name that
   did not resolve, a TLS handshake that failed (to the local proxy or to the
   destination), and a decision proxy that closed without an answer. To find
   out which, the human filters the History, opens the row, looks up the rule
   and reads `flows.error`; there is no single question "why".

What exists today, on `main` at `c33b8cec`:

- **`SandboxEvent.Status`** (`proto/humanitl/v1/humanitl.proto`), the answer
  to `Sandbox{status}` and the first event of a start: state, ids, backend,
  `isolation_level`, `sandbox_mode` with `sandbox_mode_notice`, `work_mode`,
  `llm_server` (`LlmServerChoice` with the sources `run`, `project`,
  `profile`, `default`), `ask_mode`, `egress`, `cloud_inference`,
  `terminal_clients`, plus the mounts, the environment and the command line.
  It describes one sandbox, the only one a daemon runs today (`CLI_005`), it
  arrives on the stream of a start, and it carries no hold, no block and no
  waiting change.
- **Flow events** (`Subscribe`): `Held` with `deadline`, `queue_count` and
  `queue_bytes`, `Decided` with `kind`, `source`, `block_reason`, `rule_id`
  and `note`, `timed_out`, `Failed` with `UpstreamError`, `FlowDiagnostic`.
  `FlowSummary` carries `block_reason`, `rule_id`, `upstream_error`, `error`
  (`tls_handshake_failed:<class>`, `upstream_*`), `decision_note` and
  `send_refusal` (`HOLD_004`). `Rule.position` is the 1-based place of a rule
  in its group, as `RuleSet::evaluate` walks it.
- **The block body** (`daemon/crates/core-types/src/block.rs`,
  `backlog/CONVENTIONS.md` 3.5, ADR-0005, ADR-0014): `Blocked by Humanitl.`,
  `reason: <BlockReason as snake_case>`, `flow:`, `host:`, and an optional
  `note:`. `BlockReason` has twelve values; their statuses are `403`, `413`
  (`body_cap`), `504` (`timeout`), `503` (`hold_memory`, `hold_max_flows`),
  `502` (`no_route`, every `upstream_*`), `408` (`client_timeout`). The hard
  block puts a sentence of the machine into `note:`
  (`HARD_BLOCK_NOTE` in `daemon/crates/proxy/src/findings.rs`: "a
  checksum-confirmed secret was found in this request and
  hold.hard_block_checksum_secrets is on").
- **The meta endpoint** `humanitl.internal` (ADR-0014,
  `daemon/crates/proxy/src/meta.rs`): `GET /` prints the session line
  `humanitl session=<id> ask=<ui|terminal|none|upstream> timeout=<secs>
  llm=<host:port|none>` (`status_body`) and below it the rules that apply,
  one row each with action, methods, host, path and duration (`rule_rows`).
  `GET /why/<flow-id>` prints `decision=`, `reason=` and `note=` for a flow of
  the same session: for an allow `reason=` is the source (`user`, `rule`,
  `passthrough`), for a flow nobody has decided yet the name of its state
  (`received`, `analyzed`, `held`).
- **What the agent may learn**, decided by the owner on 2026-10-31
  (HUM-364): nothing the agent reads says that a person decides or that a
  request waits for a person; latency is a property of the environment. The
  briefing and the skill keep to it ("`504`: no permission granted in
  time"). HUM-401 is the open issue that makes the proxy's own answers
  neutral; `reason: user` still stands in the block body and in `/why`.
- **The terminal belongs to the agent.** Since GitHub #12 the daemon writes
  no notice line into a session's terminal output; whatever the client adds
  (the `BEL`, the views) it adds between whole filtered chunks
  (ADR-0024 point 11).

The owner decided on 2026-11-23 (sprint 9, E1 and E6, and the brief of #81):

1. Secure by default. Behaviour is fixed, presentation is flexible: an
   expired deadline blocks, isolation is never lowered silently, nothing is
   sent home. Only wording, level of detail, surface and notification style
   are configurable.
2. Notifications show a count only by default; content and action buttons
   are opt-in (`ui.notifications`, ADR-0025 point 8).
3. An expired deadline is never silent.
4. The levels of a value are those of ADR-0025: Default, Mine (all
   projects), This project, Only this session, Company (locked).

"Item N.M" below is the numbering of the sprint's tracking issue on GitHub,
as in ADR-0025; "#N" is a GitHub issue.

Hard constraints: the three sandbox guarantees, `tests/escape/` and the
terminal output filter stay as they are. No new door out of the sandbox and
no new path on `humanitl.internal`. Every capability is an RPC first
(ADR-0018); app, terminal views and CLI render what the daemon says and
decide nothing. No new port (ADR-0015). One session per daemon stays for now
(ADR-0024 point 12), but nothing here may need reshaping when several come
(#85).

## Decision

We add two RPCs on the daemon's socket: `Sessions`, a stream of complete
status snapshots, one per session, and `Why`, the ordered chain of decisions
behind one request. The status line in the terminal and the chip row in the
app render `Sessions`; `humanitl why` and the "Why?" button render `Why`.
An expired deadline always changes the status, the notification count and
the session's history, and the agent gets a block body with a fixed
`detail:` line. What the agent reads from a block is a closed, coarse
vocabulary; everything that would tell it how to get around a block stays
with the human.

### 1. The status RPC

```proto
// The status of every session of this daemon, and of the daemon itself.
rpc Sessions(SessionsRequest) returns (stream SessionsEvent);

message SessionsRequest {
  // Empty: every session of this daemon (today at most one).
  repeated string session_ids = 1;
  // false: one `snapshot` per session and one `daemon`, then the stream ends.
  // true: the same, then a new event whenever something in it changes.
  bool follow = 2;
}

message SessionsEvent {
  oneof event {
    // Complete; replaces every earlier snapshot of the same session_id.
    SessionStatus snapshot = 1;
    // The session ended; clients drop it. The recording keeps it.
    string ended_session_id = 2;
    // The background service and the sums over all sessions.
    DaemonStatus daemon = 3;
  }
}
```

**Snapshots, not deltas.** Every `snapshot` is the whole status of one
session (well under 4 KiB). The daemon keeps only the newest snapshot per
session and subscriber, so a slow client skips states instead of falling
behind, and there is no `Lagged` to recover from. Changes are coalesced to at
most four snapshots per second per session. The countdown of a deadline is
not a change: clients count down from `deadline` and correct their clock with
`server_time`.

```proto
message SessionStatus {
  string session_id = 1;
  string sandbox_id = 2;
  // The project of the register (ADR-0024) and its folder.
  string project_id = 3;
  string project_dir = 4;
  SandboxState state = 5;
  google.protobuf.Timestamp started_at = 6;
  // Terminals attached now; 0 means it keeps running in the background.
  uint32 terminal_clients = 7;
  // Monotonic per session; a client ignores a snapshot older than its last.
  uint64 revision = 8;
  google.protobuf.Timestamp server_time = 9;

  Holds holds = 10;
  AgentStatus agent = 11;
  ModelServerStatus model_server = 12;
  IsolationStatus isolation = 13;
  FolderStatus folder = 14;
  NetworkStatus network = 15;
  repeated PendingChanges changes = 16;
  LastBlock last_block = 17;
  Expiries expiries = 18;
}
```

The fields, each with what it carries and where the value comes from:

| Field | Carries | Source |
|---|---|---|
| `holds` | `count` (requests of this session that wait for a decision), `nearest_deadline` (Timestamp), `nearest_flow_id`, `queue_bytes` | the hold queue, filtered by `FlowSummary.session_id` |
| `agent` | `id` (`claude`, `opencode`, …), `version`, `source` (`AGENT_SOURCE_HOST` "on this computer" or `AGENT_SOURCE_DEPOT` "installed in isolation"), `source_level` and `source_mark` | the adapter of the start, `manifest.json` of the depot (ADR-0023), the resolution of `agents.<id>.source` |
| `model_server` | `name`, `level` and `mark`, `reach` (`REACH_LOCAL`, `REACH_LAN`, `REACH_CLOUD`, `REACH_UNKNOWN`), `cloud_inference` (`CloudInference`, HUM-516), `problem` (`Diagnostic`, for example `LLM_005`) | `LlmServerChoice` of the start, the passthrough's last connection |
| `isolation` | `level` (`IsolationLevel`), `backend`, `min_level` (the effective minimum isolation level), `min_level_source` and `min_level_source_mark`, `floor_met` | the start's resolution, `SANDBOX_035` |
| `folder` | `mode` (`FOLDER_MODE_WORKING_COPY`, `_DIRECT`, `_READ_ONLY`), `level` and `mark`, `notice` (`SANDBOX_033` for a resolved `auto`) | `sandbox.mode`, `sandbox.work_mode`, `sandbox_mode_notice` |
| `network` | `mode` (`NETWORK_MODE_ASK`, `_BLOCK`, `_DECISION_PROXY`), `level` and `mark`, `ask_where` (`ui`, `terminal`), `paused` (the pause of HUM-407), `decision_deadline` (Duration) | `hold.ask_mode`, `hold.timeout_secs`, `InterceptStatus` |
| `changes` | per layer: `count`, `since`, `truncated`, `reviewable`, `from_session_id` | the working copy of this session; waiting changes an earlier session of the same project left |
| `last_block` | `flow_id`, `host`, `at`, `outcome` (`WhyOutcome`, point 3), `code` (the code of the chain's `Diagnostic`) | the newest flow of the session that Humanitl did not deliver |
| `expiries` | `count` (deadlines passed in this session), `last_at`, `since_detach` (`at`, `expired`, `blocked`, `waiting_now`) | the decisions `timed_out` of the session; `at` is when `terminal_clients` last fell to 0 |

```proto
message DaemonStatus {
  string version = 1;
  ServiceState state = 2;          // SERVICE_STATE_RUNNING, SERVICE_STATE_DEGRADED
  repeated Diagnostic problems = 3; // most severe first
  uint32 sessions = 4;
  uint32 holds = 5;                // sum over all sessions
  uint32 expired_since_detach = 6; // sum over all sessions
  uint32 pending_changes = 7;      // sum over all sessions
}

message Holds {
  uint32 count = 1;
  google.protobuf.Timestamp nearest_deadline = 2;
  string nearest_flow_id = 3;
  uint64 queue_bytes = 4;
}

message AgentStatus {
  string id = 1;
  string version = 2;               // empty: unknown, never guessed
  AgentSource source = 3;
  SettingLevel source_level = 4;
  string source_mark = 5;
}

message ModelServerStatus {
  string name = 1;
  SettingLevel level = 2;
  string mark = 3;
  Reach reach = 4;
  CloudInference cloud_inference = 5; // existing message (HUM-516)
  Diagnostic problem = 6;
}

message IsolationStatus {
  IsolationLevel level = 1;         // existing enum
  string backend = 2;
  IsolationLevel min_level = 3;
  SettingLevel min_level_source = 4;
  string min_level_source_mark = 5;
  bool floor_met = 6;
}

message FolderStatus {
  FolderMode mode = 1;
  SettingLevel level = 2;
  string mark = 3;
  Diagnostic notice = 4;            // SANDBOX_033 for a resolved `auto`
}

message NetworkStatus {
  NetworkMode mode = 1;
  SettingLevel level = 2;
  string mark = 3;
  string ask_where = 4;             // `ui`, `terminal`; empty otherwise
  bool paused = 5;
  google.protobuf.Duration decision_deadline = 6;
}

message PendingChanges {
  uint32 count = 1;
  google.protobuf.Timestamp since = 2;
  bool truncated = 3;
  bool reviewable = 4;
  string from_session_id = 5;
}

message LastBlock {
  string flow_id = 1;
  string host = 2;
  google.protobuf.Timestamp at = 3;
  WhyOutcome outcome = 4;           // point 3
  string code = 5;
}

message Expiries {
  uint32 count = 1;
  google.protobuf.Timestamp last_at = 2;
  SinceDetach since_detach = 3;
  message SinceDetach {
    google.protobuf.Timestamp at = 1;
    uint32 expired = 2;
    uint32 blocked = 3;
    uint32 waiting_now = 4;
  }
}

enum ServiceState {
  SERVICE_STATE_UNSPECIFIED = 0;
  SERVICE_STATE_RUNNING = 1;
  SERVICE_STATE_DEGRADED = 2;
}

enum AgentSource {
  AGENT_SOURCE_UNSPECIFIED = 0;
  AGENT_SOURCE_HOST = 1;            // "on this computer"
  AGENT_SOURCE_DEPOT = 2;           // "installed in isolation"
}

enum Reach {
  REACH_UNSPECIFIED = 0;
  REACH_LOCAL = 1;
  REACH_LAN = 2;
  REACH_CLOUD = 3;
  REACH_UNKNOWN = 4;
}

enum FolderMode {
  FOLDER_MODE_UNSPECIFIED = 0;
  FOLDER_MODE_WORKING_COPY = 1;
  FOLDER_MODE_DIRECT = 2;
  FOLDER_MODE_READ_ONLY = 3;
}

enum NetworkMode {
  NETWORK_MODE_UNSPECIFIED = 0;
  NETWORK_MODE_ASK = 1;
  NETWORK_MODE_BLOCK = 2;
  NETWORK_MODE_DECISION_PROXY = 3;
}

// Shared with GetConfig (ADR-0025 point 11).
enum SettingLevel {
  SETTING_LEVEL_UNSPECIFIED = 0;
  SETTING_LEVEL_DEFAULT = 1;
  SETTING_LEVEL_MINE = 2;
  SETTING_LEVEL_THIS_PROJECT = 3;
  SETTING_LEVEL_THIS_SESSION = 4;
  SETTING_LEVEL_COMPANY = 5;
}
```

Existing types reused as they are: `SandboxState`, `IsolationLevel`,
`CloudInference`, `Diagnostic`. `AgentSource` is new because the `Agents`
RPC carries the source as a string (`"host"`, `"depot"`); it maps those two
values one to one.

Rules for the values:

- **`level` and `mark`** (enum `SettingLevel`: `DEFAULT`, `MINE`,
  `THIS_PROJECT`, `THIS_SESSION`, `COMPANY`) are the visible level and the
  mark of ADR-0025 point 1, computed by the daemon from the `Origin` of the value as for the
  catalog; the four session settings (ADR-0025 point 2) show "Only this
  session" when a start chose them. The enum is the one the implementing
  issue of ADR-0025 point 11 gives `GetConfig`; this RPC reuses it.
- **`reach`** is never guessed in the friendly direction. `REACH_LOCAL` for a
  loopback address, `REACH_LAN` for a private, link-local or unique-local
  address, `REACH_CLOUD` for a public address or a known provider of
  `CloudInference`. The address is the one the passthrough last connected to,
  or the literal of the endpoint. Before the first connection of a name the
  value is `REACH_UNKNOWN` and shows as "not checked yet", never as local.
- **`version`** is empty when the daemon does not know it. For an agent
  installed in isolation it comes from the depot's manifest; for an agent on
  this computer only from what the adapter learns without running the
  program. The daemon never runs an agent's program to learn its version.
- **`floor_met`** is computed at the start and never assumed: a session whose
  backend lies below the effective minimum does not start (`SANDBOX_035`,
  ADR-0020, ADR-0025 point 5), so a running session carries `true`, and the
  field exists so that no client has to infer it.
- **`changes.since`** is when the daemon first counted a change in that
  layer, not a file time: the agent sets the times of its files. The daemon
  counts the entries of a running session's layer at most every five seconds
  and stops at 10 000 (`truncated`). In Direct and Read only there is no
  layer and no entry.
- **`problems`** carries what makes the background service work less than
  fully: events that were never written to the audit log, a full event
  buffer (`LIMIT_006`), a recorder that cannot write, an unusable CA. An
  unreachable background service is shown by the client itself with
  `DAEMON_001`, as today.

`SandboxEvent.Status` stays for the sandbox page (mounts, environment, the
exact command). Both are built from the same session state in the daemon, so
they cannot disagree. `Info.session_id` stays.

**Several sessions.** Every message carries its `session_id`; the stream
lists every session of the daemon, and a client that shows one session
filters by it. `DaemonStatus` carries the sums for the tray and for the
hint "+N waiting in other sessions" in a session's status line. #85 adds
starting several sessions; this RPC needs no change for it.

### 2. The status line in the terminal and the chip row in the app

Both show the same fields, in the same order, with the same words. The order
is fixed; a setting removes fields, it does not reorder them.

| # | Field (`ui.status_fields` value) | Long form (en) | Short form | Shown when |
|---|---|---|---|---|
| 1 | `holds` | "2 wait for a decision · 0:42" (the nearest deadline), "+3 in other sessions" | "2 · 0:42" | count > 0 |
| 2 | `service` | "Background service: 1 problem" | "service !" | a problem, or unreachable |
| 3 | `last_block` | "Blocked: registry.npmjs.org · deadline passed" | "blocked: deadline" | a block in this session |
| 4 | `isolation` | "Namespaces (shared kernel) · bubblewrap · minimum met" | "namespaces" | always |
| 5 | `folder` | "Working copy" | "copy" | always |
| 6 | `network` | "Ask" / "Block" / "Decision proxy", "paused" when paused | "ask" | always |
| 7 | `model` | "llama-lan · This project · LAN" | "llama-lan LAN" | a model server |
| 8 | `agent` | "claude 2.1.3 · installed in isolation" | "claude" | always |
| 9 | `changes` | "14 changes · 2 h" | "14 changes" | a working copy with changes |

German texts come from the ARB files like every other string; the words are
the glossary's (isolation levels, folder modes, network modes, sources of an
agent, setting levels).

**Narrow terminals.** First every field switches to its short form. If the
row is still too wide, fields drop in this order: `agent`, `changes`,
`model`, `folder`, `network`, `isolation`, `last_block`. `holds` and
`service` never drop. A terminal narrower than 20 columns or lower than 5
rows gets no reserved row; the client says so once when it attaches.

**Never colour alone.** Every state has a word, and where a symbol stands,
it stands next to the word. Colour only repeats what the word says (a cloud
model server, a problem of the background service, a deadline under 30
seconds). The field `model` always carries the word `CLOUD`, `LAN`, `local`
or "not checked yet". Without a UTF-8 locale the client uses ASCII only.

**Terminal modes**, new key `terminal.status_line`:

- `"row"` (default): the client reserves the bottom row of the terminal. It
  gives the session one row less than the terminal has (the writer's resize
  through the `Terminal` RPC, ADR-0024 point 9), keeps a scrolling region
  over the agent's rows, and redraws the row in place between whole filtered
  chunks of the agent's output, never inside one. Nothing of it enters the
  agent's output or the recording. `NO_COLOR` or `TERM=dumb` turn colour off;
  the row is still redrawn.
- `"plain"`: no colour, no cursor movement, no redraw, no reserved row. The
  client writes one status line at the moments it owns the screen: before it
  hands the screen to the agent on attach, and after detach. The same lines
  come from `humanitl sessions watch --plain` (point 6) in any terminal, one
  line per change, for screen readers and logs.
- `"off"`: nothing besides `BEL`.

A `--read-only` attach may not resize the session (ADR-0024 point 9) and
therefore behaves as `"plain"`. The prefix view `s` ("status") shows every
field in its long form, one per line, and the why chain of the last
failure below it; it is a view like `i` and `,`, so the agent's screen is
redrawn after it.

**Fields**, new key `ui.status_fields`: an ordered subset of the nine names,
default all nine. It applies to the status line and to the chip row. `holds`
and `service` cannot be removed; a value without them is accepted and they
are shown all the same, and the catalog row says so.

**The chip row in the app** stands above the session: one chip per field, in
the order above. What does not fit goes behind a chip "+N", except `holds`
and `service`. Each chip opens the place where its value is acted on:
`holds` the Decisions view, `last_block` the "Why?" sheet (point 3),
`isolation` the sandbox page, `folder`, `network` and `model` the session
settings ("restart this session with it", ADR-0025 point 2), `agent` the
agents page, `changes` the review, `service` the machine check
(`humanitl doctor`). The tray shows the sums of `DaemonStatus`. A change of
`holds`, a passed deadline and a problem of the background service are
announced to assistive technology; the countdown is not.

Both keys are presentation: section 8 of ADR-0025 ("Appearance and
operation"), tier `advanced` for `terminal.status_line` and `expert` for
`ui.status_fields`, effect `now`, denied in the repository file.

### 3. The why chain

```proto
// The ordered chain of decisions behind one request.
rpc Why(WhyRequest) returns (WhyChain);

message WhyRequest {
  oneof subject {
    // One flow, of any session; the daemon finds the session.
    string flow_id = 1;
    // The newest flow of a session that Humanitl did not deliver.
    LastFailure last = 2;
  }
  message LastFailure {
    // Empty: the only running session; with several, a Diagnostic names them.
    string session_id = 1;
    // Empty: any host. Set: the newest flow to this host, delivered or not.
    string host = 2;
  }
}

message WhyChain {
  string session_id = 1;
  FlowSummary flow = 2;
  repeated WhyStep steps = 3;   // in the order the request went through them
  WhyOutcome outcome = 4;
  Diagnostic diagnostic = 5;    // why and fix of the outcome
  SuggestedRule suggestion = 6; // absent where no rule would help
  // The agent's view of the same flow: status, reason and detail line
  // exactly as point 5 renders them.
  AgentView agent_view = 7;
  // true when some steps could not be rebuilt (a flow recorded before the
  // chain was stored); those steps carry WHY_VERDICT_UNKNOWN.
  bool partial = 8;
}

message WhyStep {
  WhyLink link = 1;
  WhyVerdict verdict = 2;
  google.protobuf.Timestamp at = 3;
  // The setting level and mark of the value this step applied, if any.
  SettingLevel level = 4;
  string mark = 5;
  oneof detail {
    PolicyStep policy = 10;
    ConnectionStep connection = 11;
    LimitsStep limits = 12;
    FindingsStep findings = 13;
    RuleStep rule = 14;
    HoldStep hold = 15;
    SecretBlockStep secret_block = 16;
    DnsStep dns = 17;
    UpstreamStep upstream = 18;
    TlsStep tls = 19;
    ResultStep result = 20;
  }
}

message PolicyStep      { NetworkMode network_mode = 1; bool paused = 2;
                          google.protobuf.Duration decision_deadline = 3;
                          string company_policy = 4; }
message ConnectionStep  { string tls_error = 1;      // `tls_handshake_failed:<class>` or empty
                          bool authority_mismatch = 2; string sni = 3; string host_header = 4; }
message LimitsStep      { uint64 body_size = 1; uint64 body_cap = 2;
                          uint32 queue_count = 3; uint64 queue_bytes = 4; string limit_key = 5; }
message FindingsStep    { repeated FindingCount counts = 1; // kind, location, count; no value
                          bool truncated = 2; }
message RuleStep        { bool matched = 1; Rule rule = 2;  // as it was when it decided
                          bool session_group = 3; bool still_exists = 4; NetworkMode fallback = 5; }
message HoldStep        { google.protobuf.Timestamp held_at = 1; google.protobuf.Timestamp deadline = 2;
                          HoldEnd ended = 3; DecisionSource source = 4; string note = 5; }
message SecretBlockStep { repeated string kinds = 1; bool replaced_allow = 2; }
message DnsStep         { repeated string resolved = 1; string pinned = 2; bool private_refused = 3; }
message UpstreamStep    { EgressRoute route = 1; UpstreamError error = 2; }
message TlsStep         { UpstreamError error = 1; string detail = 2; }
message ResultStep      { uint32 destination_status = 1; bool through_decision_proxy = 2; }

message AgentView       { uint32 status = 1; string reason = 2; string detail = 3; string note = 4; }

message FindingCount    { string kind = 1;                 // as `Finding.kind`
                          FindingLocation location = 2;    // existing enum
                          string header_name = 3; FindingTier tier = 4; uint32 count = 5; }

message SuggestedRule {
  Rule rule = 1;
  string reason = 2;
  uint32 would_match = 3;
  uint32 of_recent = 4;
}

enum WhyLink {
  WHY_LINK_UNSPECIFIED = 0;
  WHY_LINK_POLICY = 1;
  WHY_LINK_CONNECTION = 2;
  WHY_LINK_LIMITS = 3;
  WHY_LINK_FINDINGS = 4;
  WHY_LINK_RULE = 5;
  WHY_LINK_HOLD = 6;
  WHY_LINK_SECRET_BLOCK = 7;
  WHY_LINK_DNS = 8;
  WHY_LINK_UPSTREAM = 9;
  WHY_LINK_TLS = 10;
  WHY_LINK_RESULT = 11;
}

enum WhyVerdict {
  WHY_VERDICT_UNSPECIFIED = 0;
  WHY_VERDICT_PASSED = 1;
  WHY_VERDICT_ENDED_HERE = 2;
  WHY_VERDICT_FAILED = 3;
  WHY_VERDICT_NOT_REACHED = 4;
  WHY_VERDICT_UNKNOWN = 5;
}

enum HoldEnd {
  HOLD_END_UNSPECIFIED = 0;
  HOLD_END_ALLOWED = 1;
  HOLD_END_ALLOWED_EDITED = 2;
  HOLD_END_BLOCKED = 3;
  HOLD_END_DEADLINE_PASSED = 4;
  HOLD_END_CLIENT_GAVE_UP = 5;
  HOLD_END_NOT_HELD_PAUSED = 6;
  HOLD_END_FORWARDED_TO_DECISION_PROXY = 7;
  HOLD_END_NOT_HELD = 8;            // a rule decided before any hold
}

enum WhyOutcome {
  WHY_OUTCOME_UNSPECIFIED = 0;
  WHY_OUTCOME_DELIVERED = 1;
  WHY_OUTCOME_DESTINATION_ANSWERED_ERROR = 2;
  WHY_OUTCOME_BLOCKED_BY_PERSON = 3;
  WHY_OUTCOME_BLOCKED_BY_RULE = 4;
  WHY_OUTCOME_BLOCKED_BY_NETWORK_MODE = 5;
  WHY_OUTCOME_DEADLINE_PASSED = 6;
  WHY_OUTCOME_SECRET_BLOCKED = 7;
  WHY_OUTCOME_LIMIT = 8;
  WHY_OUTCOME_AUTHORITY_MISMATCH = 9;
  WHY_OUTCOME_CLIENT_TLS_REFUSED = 10;
  WHY_OUTCOME_CLIENT_GAVE_UP = 11;
  WHY_OUTCOME_PRIVATE_ADDRESS = 12;
  WHY_OUTCOME_DNS_FAILED = 13;
  WHY_OUTCOME_CONNECT_FAILED = 14;
  WHY_OUTCOME_UPSTREAM_TIMEOUT = 15;
  WHY_OUTCOME_UPSTREAM_TLS_FAILED = 16;
  WHY_OUTCOME_DECISION_PROXY_CLOSED = 17;
  WHY_OUTCOME_NO_ROUTE = 18;
}
```

`NetworkMode` and `SettingLevel` are the enums of point 1. Existing types
are reused as they are: `Rule`, `DecisionSource`, `UpstreamError`,
`EgressRoute`, `FlowSummary`, `Finding.kind`, `FindingLocation`,
`FindingTier`, `Diagnostic`.

**The links**, in the order a request passes them:

| # | `WhyLink` | What the step states | Ends the request with |
|---|---|---|---|
| 1 | `POLICY` | network mode and its level, whether the pause is on, the decision deadline and its level, a company lock if one applies | – (context; never ends anything) |
| 2 | `CONNECTION` | the agent's TLS handshake with the local proxy (`tls_handshake_failed:<class>`, `TLS_001`, `TLS_002`, `TLS_003`), the check of `Host` against the TLS target (`PROXY_002`) | `CLIENT_TLS_REFUSED`, `AUTHORITY_MISMATCH` |
| 3 | `LIMITS` | body size against `limits.hold_body_cap_bytes` (`LIMIT_001`, `PROXY_001`), the hold budget (`LIMIT_002`, `limits.hold_max_bytes`, `limits.hold_max_flows`) | `LIMIT` |
| 4 | `FINDINGS` | number of findings per kind and location (header name, query, body), whether the scan was complete (`FINDINGS_002`); never a value or its prefix | – |
| 5 | `RULE` | the first matching rule: `rule_id`, group (this session or saved) and `position`, action, target, duration, level and mark; whether it was bundled or the model server's passthrough; or "no rule matched" and the network mode's default | `BLOCKED_BY_RULE`, `BLOCKED_BY_NETWORK_MODE` |
| 6 | `HOLD` | waited from/until (`deadline`), how it ended: allowed, allowed edited, blocked by a person with the note, deadline passed, the agent's client gave up, not held because of the pause, forwarded to the decision proxy | `BLOCKED_BY_PERSON`, `DEADLINE_PASSED`, `CLIENT_GAVE_UP` |
| 7 | `SECRET_BLOCK` | the hard block (`HOLD_004`): kinds of the checksum-confirmed findings, and whether it replaced an allow | `SECRET_BLOCKED` |
| 8 | `DNS` | resolution after allow (ADR-0006): resolved addresses, the pinned one, the refusal of a private address (`PROXY_008`), a failed resolution | `DNS_FAILED`, `PRIVATE_ADDRESS` |
| 9 | `UPSTREAM` | the egress route (direct or decision proxy, ADR-0017, ADR-0022) and the connection: refused, timed out, closed by the decision proxy (`PROXY_012` to `PROXY_019`) | `NO_ROUTE`, `CONNECT_FAILED`, `UPSTREAM_TIMEOUT`, `DECISION_PROXY_CLOSED` |
| 10 | `TLS` | the TLS handshake with the destination (or through the decision proxy, `PROXY_018`) | `UPSTREAM_TLS_FAILED` |
| 11 | `RESULT` | what the agent got (`agent_view`) and, for a delivered request, the destination's status | `DELIVERED`, `DESTINATION_ANSWERED_ERROR` |

The step that ends the request has `ENDED_HERE` or `FAILED`; every later step
is `NOT_REACHED`. `DESTINATION_ANSWERED_ERROR` says the opposite of the other
outcomes: Humanitl delivered, and the destination answered with a status of
400 or above. Under `hold.ask_mode = "upstream"` the step `RESULT` says that
Humanitl cannot tell whether the decision proxy or the destination produced
the status (`backlog/CONVENTIONS.md` 4.13).

**The `Diagnostic`** of the chain is the flow's own where one exists
(`HOLD_004`, `LIMIT_001`, `LIMIT_002`, `PROXY_001`, `PROXY_002`, `PROXY_008`,
`PROXY_012` to `PROXY_019`, `TLS_001` to `TLS_003`). The outcomes without one
get new codes, registered by the implementing issue with title and why in
`en` and `de` (ARB and `diagnostic_l10n.dart`): the decision deadline passed,
blocked by a person, blocked by a rule, blocked in network mode Block, the
name did not resolve, the TLS handshake with the destination failed, and the
destination answered with an error (severity `info`). Every `why` goes
through the recorder's masks (`Recorder::masked_why`, HUM-256), every fix
through `withhold_secret_fix` (HUM-271).

**The suggested rule** (`SuggestedRule`) is offered only for two outcomes,
`DEADLINE_PASSED` and `BLOCKED_BY_NETWORK_MODE`, and only for a flow without
findings. It is narrow: `allow`, the exact host and port of the request (no
glob), its method, the path up to its last `/` without query, duration "this
session", never `allow_private`, never `stream`. `would_match` of
`of_recent` comes from the same evaluation as `rules dry-run`. No other
outcome gets a suggestion: a person or a rule blocked on purpose, a secret
cannot be allowed by any rule, and DNS, TLS or a closed connection are not
matters of a rule. Where the flow's own `Diagnostic` already carries a rule
as its fix (`LIMIT_001`, `PROXY_008`), that stays its fix. `Why` never
writes a rule; the client opens the existing rule sheet, prefilled, and only
the human's confirmation sends `Rules{add}`.

**What counts as the last failure.** A flow of the session that Humanitl did
not deliver: a block of any reason, a passed deadline, a failed upstream.
`client_aborted` counts only when `error` names a refused or broken
handshake of the agent (`alert_*`, `eof_before_finished`,
`reset_before_finished`), not a tunnel closed empty after it
(`closed_after_handshake`). A
destination's error status is not a failure of Humanitl; `why HOST` still
explains it, `why --last` skips it.

**The chain is stored.** The daemon builds the chain when the flow is
recorded, masks it, and writes it into a new column `flows.why` of the
recording, with the rule's `position` and level as they were at that moment.
`Why` reads it from there, so it answers the same after a restart and after
the rule was moved or removed (the step says so). A flow recorded before the
column exists gets the steps its row allows and `partial = true`.

### 4. A passed deadline is never silent

When the deadline of a request passes, the request is blocked, never allowed
(as today), and five things happen, all of them on every surface:

1. **The status.** The next snapshot has `holds.count` one lower,
   `last_block` with `DEADLINE_PASSED`, `expiries.count` and, for a session
   without a terminal, `expiries.since_detach.expired` one higher.
2. **The terminal.** The field `last_block` shows "Blocked: HOST · deadline
   passed" in its long form for ten seconds even where it would otherwise
   be dropped, and the client rings `BEL` under the rules of
   `terminal.bell_on_hold`. In `"plain"` the line is printed at the next
   moment the client owns the screen and by `sessions watch --plain` at
   once.
3. **The notification**, by `ui.notifications` (ADR-0025 point 8): under
   `count` (default) "1 request blocked: decision deadline passed" without
   host; under `content` and `actions` with host, method and path. `off`
   sends none; the status and the history still show it.
4. **The history.** The flow carries `DecisionKind::TimedOut` in the
   recording and the audit log, as today, and its chain (point 3). The app
   shows it in the session's history and in the Decisions view with "Why?"
   and "Save as rule"; reopening a session that ran in the background prints
   the `since_detach` counts once (item 4.2).
5. **The agent** gets `504` with `reason: timeout` and the `detail:` line of
   point 5, which says that no permission was granted before the deadline
   and the request was not sent.

### 5. What the agent may learn from a block, and what stays with the human

Every response the proxy gives the sandbox uses one closed vocabulary. It
tells the agent what happened to its request in terms it can act on (stop,
wait, try later, tell the user), never which rule, detector or person
decided, and never anything about the request's content. The rule from
HUM-364 stays: nothing the agent reads says that a person decides.

**The block body** keeps its first four lines and gains a fifth, `detail:`,
a fixed sentence per reason; `note:` stays the last line and carries only
what a person wrote to the agent (HUM-072). No sentence of the machine goes
into `note:` any more: the hard block's `HARD_BLOCK_NOTE` and the note of
network mode Block (ADR-0025 point 3) are replaced by the `detail:` line.

```text
Blocked by Humanitl.
reason: not_permitted
flow: 0193d7a8-…
host: example.org
detail: not permitted here; do not repeat this request
note: use the mirror at pypi.org instead
```

| Internal cause | Status | `reason:` today | `reason:` from now | `detail:` (fixed English text) |
|---|---|---|---|---|
| `BlockReason::User` | 403 | `user` | `not_permitted` | "not permitted here; do not repeat this request" |
| `BlockReason::Rule` | 403 | `rule` | `not_permitted` | same |
| network mode Block, no rule | 403 | – (ADR-0025) | `not_permitted` | same |
| `BlockReason::Secret` | 403 | `secret` and a machine note | `not_permitted` | same |
| `BlockReason::AuthorityMismatch` | 403 | `authority_mismatch` | `not_permitted` | same |
| `BlockReason::PrivateAddress` (currently unused; the handler answers with `UpstreamError::PrivateAddress`) | 403 | `private_address` | `not_permitted` | same |
| `BlockReason::Timeout` | 504 | `timeout` | `timeout` | "no permission was granted before the deadline; the request was not sent; this is not an outage" |
| `BlockReason::BodyCap` | 413 | `body_cap` | `body_cap` | "the request body is too large" |
| `BlockReason::HoldMemory`, `HoldMaxFlows` | 503 | `hold_memory`, `hold_max_flows` | `busy` | "try again later" |
| `BlockReason::NoRoute` | 502 | `no_route` | `no_route` | "there is no route to this destination" |
| `UpstreamError::Dns` | 502 | `upstream_dns` | unchanged | "the destination could not be reached: name not resolved" |
| `UpstreamError::Connect` | 502 | `upstream_connect` | unchanged | "the destination could not be reached: connection refused" |
| `UpstreamError::PrivateAddress` | 502 | `upstream_private_address` | `upstream_connect` | the same as `UpstreamError::Connect` |
| `UpstreamError::Tls` | 502 | `upstream_tls` | unchanged | "the destination could not be reached: TLS failed" |
| `UpstreamError::Timeout` | 502 | `upstream_timeout` | unchanged | "the destination could not be reached: no answer in time" |
| `UpstreamError::ProxyClosed` | 502 | `upstream_proxy_closed` | unchanged | as today, with its own first line (HUM-555) |
| `BlockReason::ClientTimeout`, `ClientAborted` | 408, none | | unchanged | the client is gone; nothing is read |

The status codes stay as ADR-0005 and `backlog/CONVENTIONS.md` 3.2 fix
them. `reason: not_permitted` is final; the briefing and the skill say
so. A name that resolves to a private address after an allow
(`UpstreamError::PrivateAddress`, `PROXY_008` for the human) reads to the
agent exactly like a refused connection, in status, reason and detail:
`502` with `not_permitted` would occur only in that case and would answer
the probe "does this name resolve inside the network". `busy` and
`body_cap` name no number and no queue. The `detail:` lines are English and fixed like the
banner; they are protocol, not interface.

**`GET /why/<flow-id>`** answers in the same vocabulary: `decision=` one of
`pending`, `sent`, `blocked`, `timeout`; `reason=` the agent's reason from
the table, for a sent request `sent`, and for a flow nobody has decided yet
`pending` in every state (`received`, `analyzed` and `held` are no longer
named: `held` says that the request waits for someone); `note=` as today.
Who or what decided (`user`, `rule`, `passthrough`, `upstream`, the pause)
is no longer named. The test `why_says_pending_while_nobody_has_decided` in
`daemon/crates/proxy/src/meta.rs` changes to expect
`decision=pending reason=pending`, and a new test
`why_names_no_state_of_a_pending_flow` drives one flow through `received`,
`analyzed` and `held` and expects that same line in each.

**`GET /`** keeps its two parts, changed as follows:

- The session line drops `ask=` and `timeout=` and reads exactly
  `humanitl session=<session-id> llm=<host:port|none>`. `ask=ui` or
  `ask=terminal` tells the agent that a person decides, and `timeout=` gives
  it the window it has. The briefing names the deadline in its own words
  (HUM-364), which is all the agent needs to choose its tool timeouts.
- The rule listing (`rule_rows`) stays, as a deliberate exception to the
  paragraph below. The agent learns where it may go, so it does not send
  requests that only wait or end in `403`, each of which would cost the
  human a decision or a block. The trade-off: a listed pattern can hint at a
  variant next to it (another path, another method), and an allow rule
  names an exit that needs no decision. It opens no way past a human: every
  request outside an allow rule still waits for a decision or is blocked,
  under every network mode. The listing keeps what ADR-0014 and
  `backlog/CONVENTIONS.md` 4.24 fix (no id, no position, no note, and no origin beyond the tags `bundled` and `llm passthrough`)
  and gets no part of the why chain.

**What stays with the human**, on the daemon's socket only, which no sandbox
reaches (guarantee 2), apart from the rule listing of `GET /` above: the why
chain and every step in it; which rule decided a request, and every rule's
id, position, level and origin; whether a person, a
rule, the pause or the decision proxy decided; that the content was scanned,
the kinds, locations and number of findings, and the hard block with
`HOLD_004`; the resolved addresses and whether a name resolved to a private
address; the address of the decision proxy and its diagnostics; every
`Diagnostic` code, `why` and `fix`; the suggested rule and its preview; the
session status of point 1. `humanitl.internal` gets no new path.

### 6. Thin clients

| Surface | Calls | Shows |
|---|---|---|
| `humanitl sessions [--json]` | `Sessions{follow: false}` | one row per session with every field of point 2 in its long form |
| `humanitl sessions watch [SESSION] [--plain] [--json]` | `Sessions{follow: true}` | the status line, or with `--plain` one line per change without colour or cursor movement |
| status line, prefix view `s` | `Sessions{follow: true}` for its session | point 2 |
| `humanitl why [--last \| HOST \| FLOW_ID] [--session ID] [--json]` | `Why` | the steps, one line each with their verdict, then the `Diagnostic` block of the CLI, then the suggested rule as the `humanitl rules add …` command to copy with "would match N of the last M requests"; no argument means `--last` |
| app: chip row, tray | `Sessions{follow: true}` | point 2 |
| app: "Why?" on the chip `last_block`, on the banner of a passed deadline, on a row of the History and on a card in the Decisions view | `Why{flow_id}` | the steps as a list, the `Diagnostic` with its fix button, "Create rule…" opening the rule sheet prefilled |

`humanitl why` without a session in reach and with several running refuses
with the sessions listed; without any flow to explain it says so with a
`Diagnostic`, exit code 1. Both RPCs get their rows in the parity table
(ADR-0018), and `Info.capabilities` names `sessions.status` and `why`; a
client talking to an older daemon (`UNIMPLEMENTED`) keeps today's displays.

### What stays out

Several sessions per daemon (#85; this ADR only keeps the shape ready);
extending a deadline (item 3.1); the content of notifications beyond the
three levels of ADR-0025 point 8; a support bundle; any new path on
`humanitl.internal`; changing the rows of the rule listing of `GET /`.

## Rationale

- **A new RPC instead of a larger `SandboxEvent.Status`.** That message
  rides on the stream of a start and stop, carries the mounts, the
  environment and the command line, and describes one sandbox. A status for
  a line at the bottom of a terminal must be small, available to any client
  at any time, list several sessions and say the level of each value. Both
  are built from the same state, so the split costs no consistency.
- **Complete snapshots** make the stream trivially correct for a slow or
  reconnecting client: the newest snapshot is the truth, and there is no
  ordering of deltas to get wrong. At four per second and under 4 KiB, the
  cost is negligible.
- **The chain in the daemon.** Only the daemon knows the rule's position and
  level at the moment it decided, the hold's end, the resolved address and
  the egress route. A client that pieced the chain together from
  `ListFlows`, `GetFlow` and `Rules` would show today's rule set for
  yesterday's decision and would decide what counts as the cause, which
  ADR-0018 forbids. Storing the chain with the flow makes the answer the
  same before and after a restart.
- **A coarse vocabulary for the agent.** An agent under prompt injection is
  the attacker this tool assumes (`docs/THREAT-MODEL.md`). If a block says
  that a secret was found, the next attempt is the same secret encoded; if it
  names the rule or its pattern, the next attempt is the path or method next
  to it; if it says "authority mismatch" or "private address", it confirms
  what a probe was looking for. `not_permitted` says the one thing the agent
  needs: stop and tell the user. The human loses nothing, because the full
  chain is one command or one button away.
- **The deadline line names a missing permission, not a missing person.**
  HUM-364 decided that the agent does not learn that a person decides; the
  briefing already reads "`504`: no permission granted in time". The
  `detail:` line keeps that wording and adds what #81 asks for: the
  request was not sent, and this is not an outage. A residual channel stays
  and is accepted: `504 timeout` against `403 not_permitted` tells the agent
  that a deadline passed without a permission, so that something could have
  granted one. The briefing says as much already ("no permission granted in
  time"), it names no person, and hiding it would turn an explainable wait
  into an unexplained failure.
- **A suggested rule only where a rule is the answer.** After a passed
  deadline or under network mode Block, an allow rule is what the human
  would write next; after a deliberate block, a secret, or a network error,
  an allow rule would either undo a decision or do nothing. The narrow shape
  (exact host, method, path prefix, this session) is the smallest rule that
  still lets the same request through.
- **A reserved row, redrawn by the client.** The terminal belongs to the
  agent (GitHub #12); the daemon writes nothing into it, and the client
  writes only between whole filtered chunks, as it does with `BEL`. A row at
  the bottom is where tmux and screen keep theirs, so users know where to
  look.
- **`holds` and `service` cannot be hidden** because they are the two fields
  that ask for an action and would otherwise be silent; everything else is
  presentation and follows the user's choice.

## Rejected alternatives

- **Extend `SandboxEvent.Status` with holds, blocks and changes.** Singular,
  bound to the start stream, heavy, and its sources (`run`, `project`,
  `profile`, `default`) are not the levels of ADR-0025. It would have to be
  reshaped for #85 anyway.
- **Deltas on the event stream `Subscribe`.** Every client would have to
  fold flow events into a status and recover after `Lagged`, and each would
  fold them a little differently.
- **Let the daemon write the status line into the session's terminal
  output.** GitHub #12 removed exactly that: it lands in the recording,
  corrupts a full-screen agent's screen and passes through the filter as
  agent output.
- **The terminal window title as the status.** The client could set it, but
  many terminals and multiplexers hide titles, screen readers do not read
  them, and they cannot carry nine fields.
- **Tell the agent the exact cause** (`secret`, the rule, its pattern). It
  helps an honest agent explain the failure, and it helps an injected agent
  get around the block; the human has the exact cause anyway. HUM-364 rules
  out naming a person.
- **A new path `humanitl.internal/status` or `/why-chain`.** Every new path
  is new information inside the sandbox; the agent's view of a block is
  complete with the body and `/why/<id>`.
- **Suggest an allow rule for every failure.** It would invite undoing
  deliberate blocks with one click, the one thing a decision tool must not
  make easy.
- **Show host, method and path in the notification of a passed deadline by
  default.** E6 decided that notifications show a count by default; the
  host is one click away in the status and the history.
- **Let `ui.status_fields` hide everything.** A user who hides the waiting
  requests or a broken background service would not learn of either, which
  is a silent failure in all but name.

## Consequences

- New RPCs `Sessions` and `Why`, new messages of points 1 and 3, a proto
  minor version and the capabilities `sessions.status` and `why`. New rows
  in the parity table (ADR-0018). New CLI commands `humanitl sessions watch`
  and `humanitl why`; `humanitl sessions` renders the snapshot.
- New keys `terminal.status_line` and `ui.status_fields`; the glossary gains
  "status line" / „Statuszeile" and the button "Why?" / „Warum?" (item 1.1
  carries them). "Chip row" names a layout in this file and appears in no
  interface text.
- New diagnostic codes for the outcomes of point 3 without one, each with
  ARB title and why in `en` and `de` and an entry in `diagnostic_l10n.dart`.
- The recording gains the column `flows.why` (a masked JSON of the steps);
  old rows give partial chains.
- **The block body changes for the agent.** `detail:` is a new line, and the
  reasons `user`, `rule`, `secret`, `authority_mismatch` and
  `private_address` become `not_permitted`, `hold_memory` and
  `hold_max_flows` become `busy`, and `upstream_private_address` becomes
  `upstream_connect`. The session line of `GET /` loses `ask=` and
  `timeout=`, and `/why/<id>` says `pending` for every undecided state. Agents and scripts that parsed the old names
  see the new ones. The briefing (`agents/opencode/briefing.*.md`, HUM-071)
  and the skill (`agents/skill/SKILL.*.md`, HUM-364) change in the same
  commit, with their token count (`tools/briefing-tokens.py`). A test runs
  over every response the proxy gives the sandbox and finds none of the
  internal names and none of the words of the HUM-364 list. This decision
  carries the block-body and `/why` part of HUM-401.
- Amendments that the implementing issues make in the same commits:
  ADR-0005 and `backlog/CONVENTIONS.md` 3.5 (the fifth line and the
  vocabulary), ADR-0014 and `backlog/CONVENTIONS.md` 4.24 (the session line
  of `GET /` and the answer of `/why/<id>`), ADR-0025 point 3 (the
  note of network mode Block becomes the `detail:` line).
- `docs/SECURITY.md` and `docs/THREAT-MODEL.md` change together in the
  implementing issue of point 5: they quote what the agent reads from `/why`
  (section 6, "Pausing intercept"). The change narrows what the agent
  learns; no guarantee and no statement of `README.md` changes. This ADR
  changes neither file.
- Risk: the reserved row depends on the scrolling region surviving the
  agent's output; an agent that resets the region (`CSI r`) or the whole
  terminal exposes the row to its scrolling until the client restores it.
  Mitigation: the client restores the region and redraws the row after every
  chunk that contained such a sequence, and the check per shipped agent goes
  on the owner's walkthrough list, beside the redraw of ADR-0024 point 9.
- Risk: the `REACH_UNKNOWN` of a model server given by name lasts until its
  first connection. Mitigation: it reads "not checked yet", never "local".
- Doors kept open: #85 lists several sessions without a change of shape;
  item 3.1 can add "extend the deadline" to the status's `holds`; a later
  support bundle can take `WhyChain` as it is, since it is already masked.

## Affected issues

GitHub #81 (this decision), building on `HUM-072` (note to the agent),
`HUM-073` (meta endpoint), `HUM-159` (hard block), `HUM-364` (what the agent
reads), `HUM-401` (neutral answers, whose block-body and `/why` part this
decision carries), `HUM-407` (pause), `HUM-555` (closed decision proxy),
ADR-0024 and ADR-0025. Implemented in phase 1 of sprint 9: item 1.2 the
status RPC, item 1.3 the status line, item 1.4 the chip row and the tray,
item 1.5 the why RPC, `humanitl why`, the never-silent expiry and the block
body, item 1.6 "Why?" in the app and the terminal views. Item 3.5 builds the
notification of point 4 on ADR-0025 point 8, item 4.2 the line "since
detach" on `expiries.since_detach`, item 4.7 the plain mode on point 2.
#85 (several sessions per daemon) builds on point 1.
