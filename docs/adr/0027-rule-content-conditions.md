# ADR-0027 · Rules gain content conditions that can narrow a rule and never widen one (rule format 2)

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-24

## Context

A rule decides about the target of a request and nothing else. ADR-0007
fixed the matcher keys `host`, `method`, `path`, `scheme`, `port` and
`upgrade` (plus `path_prefixes` since HUM-039) and an expiry. The code says
so in as many words: `RequestKey` in `daemon/crates/rules/src/eval.rs`
"carries no headers and no body: a rule decides about the target, never about
the content". The content is the business of the detectors
(`humanitl-findings`) and of the human.

That leaves a gap the owner named for sprint 9 (GitHub #82): a rule cannot say
"requests to this host must not carry a body" and block the ones that do. The
human can only allow or block the target as a whole, or have every request
to it wait for a decision. Users who know that an API is read-only for their
agent, or that an upload endpoint should only ever receive JSON under a size
limit, have no way to write that down.

What exists today, on `main` at `97883190`:

- **Evaluation.** `RuleSet::evaluate(&RequestKey, now, session)` walks four
  tiers (`backlog/CONVENTIONS.md` 4.5): the bundled passthrough to the model
  server, the user's session rules, the user's persistent rules, then every
  other bundled rule (`rules/default.yaml`, the agent adapter, profiles).
  Within a tier the first match wins; without a match the result is
  `Verdict::Default`, which means `ask`. `RulesPipeline`
  (`daemon/crates/proxy/src/pipeline.rs`) turns `allow` and `block` into a
  decision at once (`DecisionSource::Rule`), passes `ask` to the inner
  pipeline and holds `redact` in Humanitl (`decide_here`) until HUM-079.
  `decide_here` honours the pause of intercepting (HUM-407): while it is on,
  a request with a deadline is allowed with `DecisionSource::Paused`, a
  `redact` request included; only the hard block stays. A poisoned rule set
  gives `Verdict::Default`, never an allow. Two call sites read the verdict
  as `let Verdict::Matched { .. } = … else { inner.decide }`
  (`RulesPipeline::decide` and `stream_grant`), and `Verdict::action()`
  maps everything that is not `Matched` to `Action::Ask`.
- **Order of a request through the proxy** (`handler.rs`): the head is read;
  a declared `Content-Length` above `limits.hold_body_cap_bytes` (32 MiB) goes
  to `over_cap` without reading the body; otherwise the body is buffered
  completely (ADR-0005), the flow is `Received`, the request is recorded,
  the scan runs (`analyze`, `FlowState::Analyzed` with the findings), and only
  then does the pipeline decide. The rules therefore already run after the
  body is in memory and after the scan.
- **Over the cap** (`over_cap`, HUM-057): `stream_grant` evaluates the rules
  on the head alone. Only a first matching `allow` or `ask` rule with
  `stream: true`, and only while `hold.hard_block_checksum_secrets` is off,
  lets the request through the pipeline with its body unread; otherwise it is
  blocked with `413` (`BlockReason::BodyCap`, `LIMIT_001`).
- **What the scan sees** (`humanitl-findings`, `decode.rs`): the body is
  decoded once through `decode_body` with the `Content-Encoding` values
  `gzip`, `x-gzip`, `deflate` and `br`, under one `Budget` that keeps both
  `limits.preview_cap_bytes` (8 MiB) and `limits.max_decompress_ratio` (100).
  An unknown coding, a chain of codings, a broken or truncated stream, bytes
  after the end of the stream, a `deflate` body with two readings, a body over
  the preview cap and a stop at the ratio each mark the scan incomplete
  (`FINDINGS_002` or `LIMIT_003`; `ScanReport.truncated`). The patterns of the
  detectors run on `regex::bytes`, finite automata without backtracking,
  with a set `size_limit`.
- **Findings** (`humanitl_core::Finding`): `kind` is one of `api_key`, `jwt`,
  `email`, `iban`, `credit_card`, `phone`, `ipv4`, `user_term` and `custom`;
  `tier` is `checksum`, `regex` or `user_term`; `location` is a header, the
  query or the body. A value never leaves the crate, only its hash and a
  masked prefix.
- **The hard block** (`findings.rs`, HUM-049, HUM-159): with
  `hold.hard_block_checksum_secrets` on, a checksum-confirmed finding sets
  `Flow::send_refusal`; `RulesPipeline::allow_by_rule` turns a rule's allow
  into a block by the system (`BlockReason::Secret`), and no rule lifts it.
- **A rule that cannot be read** (`unreadable.rs`, HUM-561): it costs only
  itself. An `allow` is left out; any other action with a readable action and
  match stands in its place unchanged; with an unreadable condition or action,
  `ask` for its host (or every host) stands in its place. Nothing is blocked
  that the user did not block (`docs/SECURITY.md` section 7).
- **The file and the wire.** `rules.yaml` carries `version: 1`
  (`RULES_VERSION`); every rule and every `match` is read with
  `deny_unknown_fields`. A file with another version is refused as a whole,
  and then none of the user's persistent rules apply. The wire type is
  `Rule` in `proto/humanitl/v1/rules.proto` (fields 1 to 14) with
  `RuleMatcher` (fields 1 to 7). The Rust client (`prost`) drops unknown
  fields when it decodes.
- **Trying rules.** `humanitl rules test URL [--method M] [--upgrade
  websocket]` sends `RulesRequest.Test` and prints the matching rule; its exit
  code is 0 for allow, 10 for block, 11 for ask. `humanitl rules dry-run`
  sends `RulesRequest.DryRun` with a candidate rule and gets the recorded
  flows it would match (`dry_run_matches`, `dry_run_scanned`, at most the last
  500).
- **The audit record** `flow.decided` (`daemon/crates/audit/src/kinds.rs`,
  `FlowDecided`) names the deciding `rule`, `decided_by` and counts of
  findings; `rule.added` and `rule.updated` (`RuleChange`) name action, host,
  methods and expiry.
- **What the agent reads** (ADR-0026 point 5): a block reads
  `reason: not_permitted` with a fixed `detail:` line, whoever or whatever
  decided; the rule listing of `GET /` on `humanitl.internal` (`rule_rows`)
  shows action, methods, host, path and duration of each rule.

The owner decided on 2026-11-23 (sprint 9, E10) which conditions the first
version has: body present or absent, body size, Content-Type, header,
query parameter, kind of finding, "body contains text", and patterns only
with a linear-time engine. The brief of #82 adds: secure by default, so
anything that can only fail must fail closed (block or hold, never allow),
and a content condition can narrow an allow, never widen one.

"Item N.M" below is the numbering of the sprint's tracking issue on GitHub,
as in ADR-0025 and ADR-0026.

Hard constraints: the three sandbox guarantees, `tests/escape/` and the hard
block stay as they are. Every capability is an RPC first (ADR-0018); app,
terminal views and CLI render what the daemon says and decide nothing. No new
port (ADR-0015). The rule engine stays a crate without IO and without async.
The agent's vocabulary of ADR-0026 point 5 stays closed. `docs/SECURITY.md`
and `docs/THREAT-MODEL.md` are not changed by this decision; point 7 lists what
the implementing issues write into them.

## Decision

We add an optional block `when:` to a rule: a list of content conditions that
must all hold for the rule to match. Each condition evaluates to one of three
answers, holds, fails or undetermined. A rule whose target matches and whose
conditions all hold decides as today; a rule with a failing condition is
skipped and the walk goes on; a rule with an undetermined condition and no
failing one ends the walk and blocks (action `block`) or holds (any other
action), never allows. First match wins stays the one order. The agent learns
nothing about conditions beyond the word `conditional` in the rule listing.

### 1. The conditions of the first version

All keys are optional; a `when:` needs at least one. Every key is one
condition, and all conditions of a rule must hold (and). A list is one
condition that holds when any of its items holds; a list whose key ends in
`_absent` holds when none of its items occurs.

| Key | Value | Holds when |
|---|---|---|
| `body` | `present` or `absent` | the request carries a body of at least one byte, or none (`Content-Length: 0` and an empty chunked body count as none) |
| `body_min_bytes` | integer | the body as sent on the wire (after transfer decoding, before content decoding) has at least this many bytes |
| `body_max_bytes` | integer | the body as sent on the wire has at most this many bytes |
| `content_type` | list of media types, `type/subtype` or `type/*` | the media type of the single `Content-Type` header, without parameters and compared without case, is one of them |
| `header_present` | list of header names | any of the named headers is present (names compared without case) |
| `header_absent` | list of header names | none of the named headers is present |
| `query_present` | list of parameter names | any of the named parameters occurs in the query |
| `query_absent` | list of parameter names | none of the named parameters occurs in the query |
| `query_equals` | map from parameter name to value | for every entry, the parameter occurs exactly once and its value equals the given value; a named parameter that occurs more than once, with the same or different values, makes it undetermined |
| `findings_present` | list of finding classes or kinds | the request carries at least one finding of a listed class or kind, in a header, the query or the body |
| `findings_absent` | list of finding classes or kinds | the request carries no finding of any listed class or kind |
| `body_contains` | list of texts | the decoded body contains any of the texts, byte for byte |
| `body_matches` | list of patterns | any of the patterns matches somewhere in the decoded body |

`query_equals` is the one map; it holds when every entry holds, because "this
parameter has this value and that one that value" is the only reading a map
of equalities has.

**Finding classes.** `secret` stands for the kinds `api_key`, `jwt` and
`custom`; `personal_data` for `email`, `iban`, `credit_card`, `phone` and
`ipv4`; `user_term` for a term from `findings.user_terms`. A single kind is
written as `FindingKind::as_str` writes it (`iban`, `api_key`). A finding that
`findings.ignored_hashes` names, or the user's own model access key in its
header to its own endpoint, is no finding here either, exactly as for the
hold. `custom` counts as `secret` so that a finding of a detector without a
class of its own makes a `findings_absent: [secret]` allow fail rather than
hold.

**What "the decoded body" is.** The bytes `humanitl_findings::decode_body`
produces for the scan, from the same `Content-Encoding` and under the same
`Budget` (`limits.preview_cap_bytes`, `limits.max_decompress_ratio`), decoded
once and shared by the scan and the conditions so that the two never see
different bodies. Nothing else is decoded: no base64, no percent-decoding of a
form body, no charset conversion, no multipart parts. The text of
`body_contains` is UTF-8 and compared as bytes, with case. A query is split at
`&` and the first `=`, and names and values are compared after
percent-decoding; `+` stays `+` and `;` is no separator. If any pair of the
query cannot be decoded (invalid percent-encoding, not UTF-8), every query
condition of the rule is undetermined, not only the one naming that pair:
a server may read the broken pair as the very name a condition is about.

**The pattern engine is `regex::bytes`** of the crate `regex` (workspace
dependency `regex = "1.13"`, already used by `PathMatcher` in
`humanitl-rules` and by every detector). It builds finite automata without
backtracking: a search costs at most time linear in the length of the body
times the size of the compiled pattern, it has no back-references and no
look-around, and `RegexBuilder::size_limit` and `dfa_size_limit` bound the
compiled size. `body_contains` runs through `aho-corasick` (workspace
dependency `aho-corasick = "1.1"`), linear in the length of the body for all
texts of a rule at once. `humanitl-rules` takes `aho-corasick` with
`workspace = true`; `daemon/Cargo.toml` does not change. Patterns and texts
are compiled once when the rule is read, like the `~` path regex today.

### 2. Evaluation, and the fail-closed answers

Conditions are evaluated where rules are evaluated today: after the body is
buffered and after the scan, in `RulesPipeline`, on the flow in
`FlowState::Analyzed`. Only a rule whose target matches gets its conditions
evaluated, so a request to a host without conditional rules costs nothing
new.

**Three answers.** Each condition answers `holds`, `fails` or
`undetermined`. A rule's target matches and then:

- every condition `holds`: the rule matches, as today;
- at least one condition `fails`: the rule is skipped, whatever the others
  answer;
- no condition `fails` and at least one is `undetermined`: the rule is
  **undetermined**, and the walk ends there.

Conditions are evaluated in the order of the table in point 1 (cheap before
expensive) and the walk over one rule's conditions stops at the first
`fails`; the body is searched only when every cheaper condition held or was
undetermined. A condition not evaluated because an earlier one failed is
reported as `not_checked`.

**What an undetermined rule does.**

| Action of the rule | Result |
|---|---|
| `block` | blocked at once, as by the rule (`BlockReason::Rule`, `403`) |
| `allow`, `ask`, `redact` | held in Humanitl through a new entry point of the ask pipeline, `FlowPipeline::hold_undetermined`: the request waits for a decision in the queue, also during a pause of intercepting (HUM-407) and under network mode decision proxy; under network mode Block (ADR-0025 point 3), and wherever no deadline exists (`hold.timeout_secs = 0`), it is blocked at once |

It is never allowed. A held request carries the flow diagnostic `RULES_015`
(point 5) that names the rule, the condition and the gap. The hard block
stays above all of it: a flow with `send_refusal` is blocked by the system
whatever a rule's conditions answer.

The pause does not apply to an undetermined rule for the same reason it does
not apply to the hard block: the pause trades sight for flow on requests a
human would have looked at, and an undetermined rule is the system saying that
it could not look. Letting it through under the pause would make "could not
check" an allow. That is why `hold_undetermined` is a new entry point and not
`decide_here`: `decide_here` lets a request through during the pause (it does
so for `redact` today), `hold_undetermined` does not read the pause at all.
Under `hold.ask_mode = "upstream"` it holds in Humanitl as well, because the
decision proxy cannot know that a condition went unchecked. The trait
default of `hold_undetermined` blocks, so a pipeline that does not
implement it (the passthrough and test pipelines) can never allow through
it; a test pins that default.

**The verdict cannot fall through to `ask`.** `Verdict::Undetermined` is
handled explicitly at both call sites that today read
`let Verdict::Matched { .. } = … else { inner.decide }`
(`RulesPipeline::decide` and `RulesPipeline::stream_grant`); falling into
`inner.decide` would make it a plain `ask`, which the pause allows.
`Verdict::action()` returns `Action::Block` for an undetermined `block` and
`Action::Ask` for every other undetermined rule, never `Action::Allow`.
Required tests in item 3.7a, each with a mutation proof (the test fails when
the explicit arm is replaced by the fall-through, or when `action()` returns
the rule's own action): `undetermined_allow_is_held_during_the_pause`,
`undetermined_allow_is_held_under_ask_mode_upstream`,
`undetermined_block_blocks_over_the_cap`, and
`verdict_action_never_allows_an_undetermined_rule` over every `Action`.

**When a condition is undetermined**, each with its gap (`ConditionGap`,
point 4):

| Situation | Conditions that become undetermined | Conditions that still answer | Gap |
|---|---|---|---|
| body over `limits.hold_body_cap_bytes` (only reachable on the way to a `stream: true` rule) | the ones that read the body: `body_contains`, `body_matches`, and `findings_present` and `findings_absent` as far as the body goes | `body` (present); `body_min_bytes` and `body_max_bytes` against a declared `Content-Length` exactly; without one (chunked, `OverCap::AtLeast`) the bytes read so far are only a lower bound, so `body_max_bytes` fails once they exceed it and `body_min_bytes` holds once they reach it, and each is otherwise undetermined (`over_cap`); `content_type`, headers, query, findings in head and query | `over_cap` |
| `Content-Encoding` unknown or a chain of codings | `body_contains`, `body_matches`, `findings_absent` | everything else; a text found is still found | `encoding_unknown` |
| decoding failed (broken or truncated stream, bytes after its end, two readings of `deflate`) | as above | as above | `decode_failed` |
| decoding stopped at `limits.max_decompress_ratio` | as above | as above | `ratio_limit` |
| decoded body longer than `limits.preview_cap_bytes` (for `findings_absent`), or longer than the 1 MiB a body search reads (for `body_contains`, `body_matches`; point 6) | as above | as above | `preview_cap` |
| the scan reported itself incomplete for another reason (`ScanReport.truncated`) | `findings_absent` | `findings_present` with a finding found | `scan_incomplete` |
| more than one `Content-Type` header, or one that does not parse | `content_type` | | `ambiguous` |
| a parameter named in `query_equals` occurs more than once, with the same or different values | `query_equals` | `query_present`, `query_absent` | `ambiguous` |
| a pair of the query cannot be decoded (invalid percent-encoding, not UTF-8) | every query condition: `query_present`, `query_absent`, `query_equals` | everything else | `ambiguous` |
| the time budget of the request is spent (point 6) | every body condition not yet evaluated | the ones already evaluated | `time_budget` |

"A text found is still found": `body_contains`, `body_matches` and
`findings_present` hold when they found something in the part that was
searched, even if the rest could not be; they can only fail when the whole
body was searched. On a body that was cut, a match of `body_matches` that ends
at the cut counts as undetermined, because a `$` or a longer repetition might
have read differently with the rest. `findings_absent` can only hold when the
scan was complete.

**Over the cap.** `stream_grant` walks the same rules with the same
evaluator on the head alone, with the gaps above. The first rule whose target
matches and whose conditions do not fail ends the walk:

- a rule that holds and carries `stream: true` streams as today (a rule with
  `when` never carries `stream: true`, point 4, so this is always a rule
  without conditions);
- a `block` rule whose conditions all hold blocks with `403`
  (`BlockReason::Rule`), as a `block` rule without conditions does;
- any other rule that holds and has no `stream: true` ends with `413`
  (`BlockReason::BodyCap`, `LIMIT_001`), as today;
- an undetermined `block` rule blocks with `403` (`BlockReason::Rule`);
- any other undetermined rule ends with `413` (`BlockReason::BodyCap`,
  `LIMIT_001` without a suggestion of `stream: true`), because Humanitl can
  neither check nor show the body it would have to hold.

**Edited requests.** An allow with edits (`AllowEdited`) is a human decision;
the rules are not walked again. The second scan of the edited version for the
hard block stays as it is.

**WebSocket.** An upgrade request is evaluated like any other; it has no body,
so `body: absent` holds. Frames after the upgrade are not evaluated (ADR-0007).

### 3. Order and precedence

**First match wins, read with conditions:** within each tier, in list order,
the first rule whose target matches and none of whose conditions fails
decides. Its conditions all hold, or it is undetermined and blocks or holds
(point 2). A rule with a failing condition is not a match; the walk continues
with the next rule, then the next tier, and without a match the result is
`ask` as today. The four tiers of `backlog/CONVENTIONS.md` 4.5 are unchanged.

**A content block and a broader allow.** Order decides, as for every rule. A
block with conditions protects a target only from the rules below it:

```yaml
version: 1
rules:
  - action: block                 # 1: uploads with a body are blocked
    match: { host: "uploads.example.com", method: [POST, PUT] }
    when: { body: present }
  - action: allow                 # 2: everything else to the host is allowed
    match: { host: "uploads.example.com" }
```

The same two rules the other way round never reach the block. Three things
make that visible, none of them changes the order:

- **Where a new rule lands.** The rule sheet and `humanitl rules add` put a
  rule with `when` and action `block` or `ask` at position 1 of its group,
  unless a position is given; every other rule still goes to the end. A
  conditional block that lands below the allow it is meant to narrow would be
  dead on arrival.
- **A shadowed conditional rule is reported.** On `add`, `update`, `reorder`
  and `reload`, a rule with `when` and action `block`, `ask` or `redact` that
  stands behind a rule of the same or an earlier tier which allows every
  request it could match gets the warning `RULES_014` naming both rules. "Allows every
  request it could match" is checked conservatively: the earlier rule is
  `allow`, has no `when`, has not expired, has the same host pattern or `**`,
  no `path`, no `path_prefixes`, no `upgrade` unless the later one has the
  same, and `method`, `scheme` and `port` empty or equal. A shadow the check
  cannot prove is not reported; `rules test` shows it for any one request.
  The tier order makes this case real: a session `allow` for a host stands in
  front of every persistent conditional block for it, and the warning names
  that too. The check runs wherever a rule enters the set, not only through
  the rules RPC: a rule remembered from a decision (`DecideRequest.remember`,
  `DecideResponse.created_rule`), a `FixAction::AddRule` and
  `make_permanent` get it as well. On the decision path the warning stands in
  the `DecideResponse` and as a `FlowEvent::Diagnostic` on the flow, so the
  person who just allowed "this session" sees which conditional block they
  just put out of reach.
- **`rules test` and `Why` name the skipped rules** (point 5).

**No deny-overrides.** A conditional block does not outrank an allow above it.
ADR-0007 chose first match wins so that a list can be read from top to bottom,
and so that what the user has just decided applies at once; a rule kind that
wins from below would break both.

**The passthrough is untouched.** Tier 1 is the bundled passthrough rule,
which no file can set; conditions in another tier cannot reach a request it
matches, and the passthrough rule itself carries no `when`. Findings on the
passthrough keep warning with `LLM_005`.

**A profile the repository selects** may still contribute only `block` and
`ask` rules (ADR-0025 point 6); conditions on them are allowed, because a
conditional `block` or `ask` from the repository decides no more than the same
rule without conditions would.

### 4. Format and compatibility

**YAML.** `when` stands next to `match`; `match` stays the target, `when` the
content.

```yaml
version: 1
rules:
  - action: block
    match: { host: "api.example.com", method: [POST, PUT, PATCH] }
    when:
      body: present
    note: "this API is read-only for the agent"

  - action: allow
    match: { host: "telemetry.example.com", path: "/v1/events" }
    when:
      content_type: [application/json]
      body_max_bytes: 65536
      findings_absent: [secret, personal_data, user_term]

  - action: block
    match: { host: "**.paste.example" }
    when:
      body_matches: ['-----BEGIN [A-Z ]*PRIVATE KEY-----']
```

The parser reads `when` with `deny_unknown_fields`, like every other part of a
rule. It refuses a rule (`RULES_013`, an error that costs only this rule,
HUM-561) when its `when`:

- is empty, or has an unknown key or a value of the wrong type;
- contradicts itself (`body: absent` with `body_min_bytes` above 0, a minimum
  above the maximum, a name in both `header_present` and `header_absent` or in
  both `query_present` and `query_absent`);
- names an invalid header name, media type, finding class or kind;
- has a pattern that does not compile or exceeds a limit of point 6;
- stands on a rule with `stream: true` (the body of a streamed request is
  never looked at, so a condition on it could only ever be undetermined) or
  `passthrough_llm: true`.

A `body_matches` that matches the empty string, or a `when` on an `allow`
rule that has only `findings_present`, reads but gets a warning, like
`too_broad` today.

**What stands in place of a rule whose `when` cannot be read.** The `when` is
part of the rule's condition: `unreadable.rs` treats it like an unreadable
match, so a `block`, `ask` or `redact` rule gets `ask` for its host (or every
host) in its place, and an `allow` is left out. The rule without its `when`
never stands in its place: as a block it would block requests the user did
not block, as an allow it would widen what was allowed.

**The file version stays `1`.** "Rule format 2" is the format of a rule, not of
the file: `RULES_VERSION` stays 1, and a file without `when` is byte for byte
what it was. An old `rules.yaml` therefore loads unchanged. The reason is the
older daemon. One that refused `version: 2` would refuse the whole file, and
then none of the user's persistent rules would apply, their blocks included.
An older daemon since HUM-561 reads a rule with the unknown key `when` as
unreadable instead, and `unreadable.rs` of that daemon leaves an `allow` out
and keeps a `block`, `ask` or `redact` with its readable action and match,
that is: without its condition, which blocks or holds more, never less. The
older daemon writes the entry back unchanged. This per-rule degradation holds
for releases from 0.5.0 on, which carry HUM-561. Version 0.4 and earlier
refuse the whole file at an unknown key, exactly as for a `version: 2`: none
of the user's persistent rules apply, bundled and session rules still do, and
without a match every request is asked; the refusal stands as `RULES_001` in
`humanitl rules list` and in `humanitl doctor`, and that daemon does not
write the file until it can be read. The release notes of the version that
ships item 3.7a say so: whoever goes back to 0.4 or earlier with rules that
carry `when` removes those rules first.

**Core types.** `humanitl_core::rule::Rule` gains `conditions:
Option<Conditions>`; `Conditions` is the typed `when`, compiled in
`humanitl-rules` (`ConditionMatcher`, beside `PathMatcher`).
`humanitl_rules::Verdict` gains a third variant:

```rust
pub enum Verdict {
    Matched { rule: RuleId, action: Action },
    Undetermined { rule: RuleId, action: Action },
    Default,
}
```

`RuleSet::evaluate` takes the request's content view next to the
`RequestKey` (headers, query, body size, the decoded body with its gap, the
findings with the scan's completeness) and a budget from the caller, and
returns the verdict together with an `Explanation`: the condition results of
the deciding rule and of every skipped rule. The crate still does no IO; the
budget is a deadline the caller hands in.

**Protocol** (`proto/humanitl/v1/rules.proto`):

```proto
message Rule {
  // fields 1 to 14 unchanged
  // Content conditions; absent means none (rule format 1). YAML key `when`.
  RuleConditions conditions = 15;
}

// Field names of repeated fields are plural (`content_types`,
// `headers_present`); the YAML keys and `ConditionResult.condition` use the
// singular YAML key (`content_type`, `header_present`). The mapping is one
// to one and lives in `humanitl-ipc` beside the other rule conversions.
message RuleConditions {
  BodyPresence body = 1;
  optional uint64 body_min_bytes = 2;
  optional uint64 body_max_bytes = 3;
  repeated string content_types = 4;
  repeated string headers_present = 5;
  repeated string headers_absent = 6;
  repeated string query_present = 7;
  repeated string query_absent = 8;
  map<string, string> query_equals = 9;
  repeated string findings_present = 10;   // classes or kinds
  repeated string findings_absent = 11;
  repeated string body_contains = 12;
  repeated string body_matches = 13;
}

enum BodyPresence {
  BODY_PRESENCE_UNSPECIFIED = 0;           // no condition on presence
  BODY_PRESENCE_PRESENT = 1;
  BODY_PRESENCE_ABSENT = 2;
}

message ConditionResult {
  string condition = 1;                    // the YAML key, e.g. `body_contains`
  uint32 item = 2;                         // 1-based list item that decided; 0 for none
  ConditionVerdict verdict = 3;
  ConditionGap gap = 4;                    // set for UNDETERMINED
}

enum ConditionVerdict {
  CONDITION_VERDICT_UNSPECIFIED = 0;
  CONDITION_VERDICT_HOLDS = 1;
  CONDITION_VERDICT_FAILS = 2;
  CONDITION_VERDICT_UNDETERMINED = 3;
  CONDITION_VERDICT_NOT_CHECKED = 4;
}

enum ConditionGap {
  CONDITION_GAP_UNSPECIFIED = 0;
  CONDITION_GAP_OVER_CAP = 1;
  CONDITION_GAP_ENCODING_UNKNOWN = 2;
  CONDITION_GAP_DECODE_FAILED = 3;
  CONDITION_GAP_RATIO_LIMIT = 4;
  CONDITION_GAP_PREVIEW_CAP = 5;
  CONDITION_GAP_SCAN_INCOMPLETE = 6;
  CONDITION_GAP_AMBIGUOUS = 7;
  CONDITION_GAP_TIME_BUDGET = 8;
  CONDITION_GAP_NOT_RECORDED = 9;          // dry-run: the recording holds less
}

message SkippedRule {
  string rule_id = 1;
  uint32 position = 2;                     // as Rule.position
  RuleAction action = 3;
  repeated ConditionResult conditions = 4;
}
```

The proto field is `conditions`, the YAML key `when`, because `when` is a
keyword in Dart's pattern syntax and the generated name would differ per
language.

In `proto/humanitl/v1/humanitl.proto`:

- `RulesRequest` gains `uint32 rule_format = 11`: the newest rule format the
  client understands. 0 and 1 mean format 1; a client of this decision sends 2.
- `RulesRequest.Test` gains `repeated Header headers = 4` and `bytes body =
  5`; an empty `body` is a request without a body, as a `GET` sends it.
- `RuleTest` gains `bool undetermined = 5`, `repeated ConditionResult
  conditions = 6` (of the deciding rule) and `repeated SkippedRule skipped =
  7`. For an undetermined rule, `action` is `RULE_ACTION_BLOCK` or
  `RULE_ACTION_ASK` as point 2 decides, and `matched` is true.
- `RulesResponse` gains `repeated DryRunExplanation dry_run_explanations = 7`,
  one per flow in `dry_run_matches` and per flow the candidate left
  undetermined, with `flow_id`, `undetermined` and the `ConditionResult`s.
- `Info.capabilities` gains `rules.conditions`.
- The why chain's `RuleStep` (ADR-0026 point 3) gains `bool undetermined = 6`,
  `repeated ConditionResult conditions = 7` and `repeated SkippedRule skipped
  = 8`.

**An older client.** It does not know field 15; `prost` drops it on decode,
and `Rule` would look to it like the same rule without conditions. Two rules
protect the conditions from such a client:

- **It cannot change them.** The daemon refuses `RulesRequest.update` of a rule
  that has conditions when `rule_format` is below 2, with `RULES_016`
  (error): the client's copy lacks the conditions, and storing it would drop
  them; for an `allow` that widens what is allowed. It also refuses `remove`
  of a rule with conditions from such a client, with the same `RULES_016`.
  The app offers "undo" after a removal by adding the removed rule again
  (`RulesController.remove` keeps the client's copy for the undo strip,
  `app/lib/features/rules/providers/rules.dart`); an older app's copy lacks
  the conditions, so the undo would bring back a conditional `allow` without
  them. `reorder`, `set_disabled` and `make_permanent` carry only ids and
  stay allowed. An `add` from such a client is stored as it is sent, and it
  cannot carry conditions: the daemon never derives a new rule from a removed
  one, so nothing an older client adds is wider than what it wrote. Tests of
  item 3.7b: `old_client_cannot_update_a_conditional_rule` and
  `old_client_cannot_remove_a_conditional_rule` (both `rule_format = 0`,
  both expect `RULES_016` and an unchanged rule set), and
  `old_client_undo_after_remove_cannot_widen` (remove refused, so the undo
  strip never holds the conditional rule).
- **It is told.** Every `RulesResponse` to a client with `rule_format` below
  2 carries one `RULES_016` warning per rule with conditions in
  `diagnostics`, the list every client already shows next to the rules:
  "rule N has conditions on the content of a request that this version of
  the app or CLI cannot show; it applies only when they hold. Update to see
  them." Older CLIs print it under `rules list`, older apps in the rules
  screen's diagnostics.

**A newer client and an older daemon.** An older daemon drops field 15 of an
`add` or `update` and would store a conditional `allow` as a plain one. A
client therefore sends `conditions` only when `Info.capabilities` names
`rules.conditions`; otherwise the CLI refuses `--when`-style options with a
`Diagnostic` and the app does not offer the conditions. This is a protocol
guard of the client, not a decision about a request.

**CLI.** `humanitl rules add` and `rules dry-run` gain the options
`--body present|absent`, `--body-min-bytes N`, `--body-max-bytes N`,
`--content-type T` (repeatable), `--header-present NAME`,
`--header-absent NAME`, `--query-present NAME`, `--query-absent NAME`,
`--query-equals NAME=VALUE`, `--findings-present CLASS`,
`--findings-absent CLASS`, `--body-contains TEXT` and `--body-matches
PATTERN` (each repeatable where the key is a list). `humanitl rules test`
gains `--header 'Name: value'` (repeatable) and `--body-file PATH`.
`rules list` shows a column `when` with the keys of the conditions
("body, content_type") and the full text in `--json`.

**App and terminal views** (item 3.7c). The rule sheet gets a section for
the conditions, closed by default, in sentences built from the table of
point 1 ("only when the request has no body"); the rules list shows the
sentence below the target. The same sentences come from the daemon's
explanation, so app, terminal views and CLI say the same.

### 5. Explainability

**`humanitl rules test`** prints the deciding rule, the result of each of its
conditions, and each rule that was skipped on the way with the condition
that failed:

```text
$ humanitl rules test https://uploads.example.com/v1/files --method POST --body-file report.pdf
block  rule 1 of your saved rules  uploads.example.com  POST
  when body: present   holds (412 KiB)
skipped: none
```

```text
$ humanitl rules test https://telemetry.example.com/v1/events --method POST \
    --header 'Content-Type: application/json' --header 'Content-Encoding: zstd' \
    --body-file event.json.zst
ask  rule 2 of your saved rules  telemetry.example.com  /v1/events
  could not check a condition, so the request waits for a decision instead of being allowed
  when content_type: application/json   holds
  when body_max_bytes: 65536            holds (1.2 KiB)
  when findings_absent: secret, …       could not be checked: Content-Encoding zstd is unknown
skipped: rule 1 (block)  when body_matches[1]   fails
```

Every verdict is a word (`holds`, `fails`, `could not be checked`, `not
checked`), never a symbol or a colour alone (ADR-0026 point 2). The exit codes
stay: 0 allow, 10 block, 11 ask; an undetermined rule exits 10 when it blocks
and 11 when it holds. `--json` carries `RuleTest` as it is. `rules test`
evaluates with the daemon's own evaluator and scanner over the given headers
and body (ADR-0018); it creates no flow and records nothing.

**`humanitl rules dry-run`** runs the candidate rule over the recorded flows
with the same evaluator. The recording gives headers, the query, the request
body up to `limits.recorder_max_body_bytes` and the stored findings with
`findings_truncated`. A body condition on a flow whose body was recorded only
in part, or not at all, is undetermined with `not_recorded`. The summary line
reads "would match N, could not check M, of the last K requests", and each
row names the condition that decided; a row that could not be checked says
whether the candidate would have blocked or held it.

**The why chain** (ADR-0026 point 3): the step `RULE` carries the deciding
rule's `ConditionResult`s, `undetermined`, and the skipped rules. An
undetermined `block` ends the request as `BLOCKED_BY_RULE`; an undetermined
hold continues to the step `HOLD`. The `Diagnostic` of an undetermined rule is
`RULES_015`.

**The audit record.** `flow.decided` gains, for a decision by rule:

- `conditions`: one entry per condition of the deciding rule with `condition`
  (the key), `item`, `verdict` and `gap`;
- `undetermined`: true when the rule decided undetermined;
- `skipped`: for each skipped rule its id and the key and item of the
  condition that failed, at most 16 entries, then `skipped_truncated`.

None of them carries a byte of the body, a matched span, a header or query
value, or the text of a `body_contains` or `body_matches`: the key and the
item number point into the rule, and the rule is in `rules.yaml`. `rule.added`
and `rule.updated` (`RuleChange`) name the condition keys with the number of
items, not their texts, because a `body_contains` text can be the very string
the user wants to keep in.

**New diagnostic codes**, each with title and why in `en` and `de` (ARB and
`diagnostic_l10n.dart`); the numbers are the next free ones when the
implementing issue lands:

| Code | Severity | When |
|---|---|---|
| `RULES_013` | error | a `when` cannot be read (point 4) |
| `RULES_014` | warning | a conditional `block`, `ask` or `redact` stands behind a rule that allows every request it could match (point 3) |
| `RULES_015` | warning, on the flow | a condition could not be checked; names rule, key, item and gap, and says that the request was blocked or waits for a decision instead of being allowed; its fix is `limits.preview_cap_bytes` for `preview_cap` of `findings_present`/`findings_absent`; for `body_contains` and `body_matches` the 1 MiB search cap is fixed, so none, and none otherwise |
| `RULES_016` | error on `update`, warning on a listing | the client does not know rule format 2 (point 4) |

**The agent.** Nothing of this reaches the sandbox. A block by a conditional
rule, determined or not, is `403` with `reason: not_permitted` and the
`detail:` line of ADR-0026 point 5; a hold looks like every other hold. The
rule listing of `GET /` (`rule_rows`) shows a rule with conditions with its
action, methods, host, path and duration, and the word `conditional` in place
of the conditions; it names no key and no value. Without the word the agent
would read a conditional `allow` as an exit and a conditional `block` as a
wall; with the conditions themselves it would learn how to shape a request
past a block.

### 6. Limits, and what is out of scope

| Limit | Value | When exceeded |
|---|---|---|
| texts in `body_contains` | at most 16 per rule, each 1 to 256 bytes | `RULES_013` |
| patterns in `body_matches` | at most 4 per rule, each at most 256 bytes of source | `RULES_013` |
| compiled size of a pattern | `size_limit` 256 KiB, `dfa_size_limit` 1 MiB | `RULES_013` |
| rules with `body_contains` or `body_matches` | at most 64 among the user's session and persistent rules; separately at most 16 among the rules of profiles, the agent adapter and `rules/default.yaml` | the rule that would exceed its quota is refused with `RULES_013`; a profile's rules are counted against their own quota, so a profile the repository selects cannot use up the user's |
| items in any other list, entries in `query_equals` | at most 32 | `RULES_013` |
| body searched by `body_contains` and `body_matches` | the first 1 MiB of the decoded body (and never more than `limits.preview_cap_bytes`) | the rest is the gap `preview_cap` |
| body searched by the scan for `findings_*` | the decoded body up to `limits.preview_cap_bytes` (default 8 MiB), as today | the rest is the gap `preview_cap` |
| `RulesRequest.Test.body` | at most 3 MiB, below the 4 MiB default message limit of gRPC | the CLI refuses a larger `--body-file` with a `Diagnostic` before sending; the daemon refuses a larger body with `RULES_013` |
| time budget | 100 ms of wall time per request for all body conditions of all rules the walk reaches | the remaining body conditions are undetermined (`time_budget`) |

The limits are constants of `humanitl-rules`, not settings: they decide when a
rule fails closed, and behaviour is fixed (sprint 9). The time budget is
checked before each search; one search is not interrupted, and the size and
count limits keep its cost bounded: at most 80 rules with at most 16 texts in
one automaton and 4 patterns each, every search over at most 1 MiB, each
search linear in that 1 MiB. Body searches run off the async executor
(`spawn_blocking`) once the decoded body exceeds 64 KiB. The evaluation works
on an `Arc` snapshot of the rule set taken under `rules.read()` and released
at once; no lock is held across `spawn_blocking` or across a search, so a
slow search never stalls `Rules{add}`, a reload or another request's
evaluation, and a reload during a search does not change the set that search
decides with. A rule added or removed during that window (at most the time
budget plus one search) applies from the next request on; this is the same
bounded window as today's evaluation and widens nothing.
A project profile may lower `limits.preview_cap_bytes`; that can only make
more conditions undetermined, and so block or hold more.

**Out of scope of this version:** conditions on fields of a JSON, form or
multipart body (a later format with its own parser and limits); any condition
on the response; header values other than `Content-Type`; `or` and `not`
across conditions, beyond the `_absent` lists; conditions on WebSocket frames;
conditions on the passthrough rule; decoding beyond `Content-Encoding`;
suggested rules with conditions (ADR-0026's `SuggestedRule` stays without
them); conditions in a company policy (BACKLOG.md section 9).

### 7. What `docs/SECURITY.md` and `docs/THREAT-MODEL.md` will need

The implementing issue of item 3.7a changes both files in the same commit,
with these statements. No guarantee and no statement of `README.md` changes.

`docs/SECURITY.md`:

1. Section 7, a new item "Content conditions narrow a rule and never widen
   one": a rule's `when` must hold in full for the rule to match; a failing
   condition skips the rule; a condition that cannot be checked (the gaps of
   point 2) ends the walk and blocks for a `block` rule and holds otherwise,
   also during a pause and under network mode decision proxy, and blocks under
   network mode Block. It never allows.
2. Section 7, the item "No rule lifts the hard block" extended: no condition
   lifts it either; conditions run after the scan, and the hard block stays
   above every rule and every condition.
3. Section 7, a new item "A content condition is not a detector": `body`,
   `body_min_bytes` and `body_max_bytes` read the request as sent; every other
   condition reads what the agent chose to write. `Content-Type`, headers and
   query are set by the agent, and a block on a text or pattern is passed by
   any encoding the condition does not undo (base64, a different spelling, a
   split across two requests). Header and query conditions read the request
   as HTTP/1.1 states it, and the destination may read it differently: a
   CGI or WSGI server maps both `X-Foo` and `X_Foo` to `HTTP_X_FOO`, so
   `header_absent: [x-foo]` passes `X_Foo`; some servers split the query at
   `;` as well as `&`; values in trailers after a chunked body are not
   headers here; on HTTP/2 to the destination (not built yet, `PROXY_007`)
   pseudo-headers are no headers for these conditions; a header sent twice
   counts as present once, and its values are not compared. `findings_absent`
   covers the headers, the query and the body, not the path: a secret in a
   path segment is not seen by it. A conditional block narrows what the
   human has to look at; it does not prove that a request is harmless. The
   reliable form is an `allow` narrowed by conditions with `ask` as the
   fall-through.
4. Section 7, the item on unreadable rules extended: a rule whose `when`
   cannot be read stands as `ask` for its host, never as the rule without its
   `when`; an older background service from 0.5.0 on reading such a rule
   keeps a `block`, `ask` or `redact` without its condition and leaves an
   `allow` out, and 0.4 or earlier refuses the whole file, so that none of the
   user's persistent rules apply and unmatched requests are asked.
5. Section 6, the paragraph on `stream: true`: a rule with `when` never
   streams; over the cap, an undetermined `block` blocks with `403` and any
   other undetermined rule ends with `413`.
6. Section 6, the limits table: the rows of point 6, and the sentence that
   lowering `limits.preview_cap_bytes` can only make more conditions
   undetermined.
7. Section 6 or 7, the compatibility guard: an older client can neither change
   nor remove a rule with conditions (`RULES_016`), and a newer client sends conditions
   only to a background service with the capability `rules.conditions`.
8. Section 8: `flow.decided` names the conditions by key, item, verdict and
   gap, and the skipped rules by id and key; never a body byte, a matched
   span, a header or query value, or the text of a condition. `rule.added`
   and `rule.updated` name keys and counts, not texts.
9. Section 7, the check: `humanitl rules test` with `--header` and
   `--body-file` prints each condition's verdict and the skipped rules, exit
   codes as before; ESC-4 gains rows for a conditional block above an allow,
   an undetermined allow under a compressed body that is held, and an
   undetermined allow during a pause that is still held.
10. Section 11, the changelog entry.

`docs/THREAT-MODEL.md`:

1. Attack surface, a new entry for content conditions: a hostile pattern
   (catastrophic backtracking) is excluded by the engine (`regex::bytes`,
   linear time) and bounded by the limits and the time budget of point 6; a
   body built to exhaust the budget makes conditions undetermined and so
   blocks or holds it; a repository profile can only add `block` and `ask`
   rules with conditions.
2. K-11 "Queue and rule metadata", extended: the agent can probe a condition
   by varying a request (with and without a body, other sizes, other types)
   and reading which variant passes. The block body stays `not_permitted`,
   and the rule listing says `conditional` without naming the condition, so
   the probe costs one request per guess and each refused guess is a
   recorded block. Accepted as part of the oracle K-11 already accepts.
3. Residual risks, two entries: content blocks on text and patterns are
   evaded by encoding (statement 3 above); and the time budget can make the
   same request decide differently under load, only ever towards block or
   hold.

## Rationale

- **Three answers instead of two.** A condition that could not be checked is
  neither true nor false. Read as false, it would skip a `block` and let a
  broader `allow` below decide: the classic way a filter fails open. Read as
  true, it would let a narrowed `allow` allow exactly the request it could not
  look at. A third answer that ends the walk and blocks or holds is the only
  reading under which "could not check" never becomes "allowed", and the owner
  asked for exactly that.
- **Hold, not block, for a non-block rule.** An undetermined `allow` is a rule
  the user wrote to let some requests through; blocking would break the
  agent's work every time a server sends `zstd`. Holding hands the one request
  to the human with the reason, which is what happens without any rule. Under
  network mode Block nobody is asked, so the hold becomes a block there.
- **The order stays first match wins.** It is the one sentence a user can
  hold in mind (ADR-0007). A deny-overrides rule kind would be safer for the
  one case of a forgotten shadow, but it would add a second order that the
  user has to know, and it would undo "a decision just made applies at once"
  for every session allow. The shadow warning and the insertion at position 1
  address the case where it occurs.
- **`when` beside `match`.** The target can be judged from the head, before
  the body is read; the content cannot. Keeping them apart in the file shows
  the user which part of a rule depends on the content, and it lets the
  unreadable-rule logic and an older daemon treat the content part as one
  unit.
- **The file version stays 1** because the fail-closed degradation of an
  older daemon is per rule (HUM-561), while a refused file costs every rule.
  The proof is in `unreadable.rs` of today's daemon: an unknown key makes the
  rule unreadable, and `stand_of` keeps a readable non-`allow` rule and leaves
  an `allow` out. A test of item 3.7a reads a file with `when` through the
  parser of the last release and asserts exactly that.
- **`regex::bytes` and `aho-corasick`** are already in the dependency tree,
  already used for the same purpose on the same bodies, and guarantee time
  linear in the input. No new dependency, no second regex dialect for users:
  `body_matches` and the `~` path pattern are the same syntax.
- **The decoded body of the scan.** One decoder, one budget, one view: the
  human, the detectors and the conditions see the same bytes. A second decoder
  for the rules would be a second place where a zip bomb or an ambiguous
  `deflate` has to be handled, and the two could disagree about what the body
  is.
- **Fixed limits** keep a hostile `rules.yaml` from a profile, and a careless
  one, from turning every request into a long search. 64 user rules with body
  conditions is above anything the sprint-9 personas described, and the error
  names the limit.
- **The agent learns `conditional` and nothing more.** ADR-0026 point 5
  closed the vocabulary because an injected agent turns every detail of a
  block into its next attempt. The word keeps the listing honest without
  handing over the condition.

## Rejected alternatives

- **An expression language (CEL, Rego, Cedar conditions) now.** Expressive and
  established, but the user writes a program instead of a list, the
  fail-closed reading of every operator would have to be specified, and the
  rule sheet could no longer show a rule as sentences. ADR-0007 keeps the
  schema translatable to Cedar; the keys of point 1 map to Cedar `when`
  clauses with context attributes, so the door stays open.
- **A backtracking engine (`fancy-regex`, PCRE).** Look-around and
  back-references would be welcome to some users, and each brings a pattern
  whose runtime is exponential in the body. Excluded by E10.
- **Read an undetermined condition as false.** Simple, and it lets a broader
  `allow` decide every request whose body is compressed with an unknown coding:
  a block that the agent can switch off with one header.
- **Read an undetermined condition as true for every action.** Safe for
  `block`, and for `allow` it allows the request whose content was not checked.
- **Content blocks win over every allow (deny-overrides).** See the rationale;
  it also could not be explained in the rule list as "the first matching rule
  decides".
- **A separate list of content rules evaluated before the rules.** Two lists
  with two orders, and a user who reads the rules list no longer sees what
  decides.
- **Bump the file to `version: 2`.** Honest about the format, and an older
  daemon would refuse the whole file and drop the user's blocks with it.
- **Evaluate conditions on the first N bytes and decide.** The payload would
  move to byte N+1, the same reason ADR-0005 buffers the whole body.
- **Conditions on JSON fields in this version.** Wanted, and needs its own
  parser, its own limits for depth and size, and a decision about duplicate
  keys, where parsers disagree exactly as `deflate` readings do. Later, as an
  addition of its own.
- **Tell the agent which condition blocked.** Helps an honest agent, and hands
  an injected one the shape of the next attempt (ADR-0026 point 5).
- **Leave conditional rules out of the rule listing of `GET /`.** The agent
  would read the host as one that needs a decision when an allow exists, or
  try a request a block refuses; the word `conditional` costs nothing the
  probe of K-11 does not already reveal.

## Consequences

- `humanitl-core::rule` gains `Conditions`; `humanitl-rules` gains the
  condition matcher, `Verdict::Undetermined` and the explanation;
  `humanitl-rules` adds `aho-corasick` with `workspace = true`. The proxy
  passes the decoded body and the scan's completeness to the evaluator and
  handles the undetermined verdict in `decide` and `stream_grant`. The
  doc comment of `RequestKey` ("never about the content") changes with it.
- The protocol gains the messages and fields of point 4, a minor version, and
  the capability `rules.conditions`; the parity table of ADR-0018 gets rows
  for the new options of `rules add`, `rules test` and `rules dry-run`.
- New diagnostic codes `RULES_013` to `RULES_016` (or the next free ones),
  each with ARB title and why in `en` and `de` and an entry in
  `diagnostic_l10n.dart`.
- The audit kinds `flow.decided`, `rule.added` and `rule.updated` gain the
  fields of point 5; the export and `humanitl audit` show them.
- The recording gains nothing: dry-run reads what is recorded today and
  reports `not_recorded` for the rest.
- `docs/reference/rules.md` gains the key `when` with the table of point 1,
  the three answers and the examples of point 4 as checked examples;
  `backlog/CONVENTIONS.md` 3.3 gains `when` in the schema and 4.5 the sentence
  on first match wins with conditions.
- The glossary needs a word for a content condition ("condition" /
  „Bedingung") and for `user_term` as a finding class before item 3.7c puts
  either into the interface (item 1.1 carries the glossary).
- Amendments the implementing issues make in the same commits: ADR-0007
  (matcher keys, the reading of first match wins, the rejected "regular
  expressions" now naming `body_matches`), ADR-0005 (`stream: true` never on a
  rule with `when`), ADR-0014 and `backlog/CONVENTIONS.md` 4.24 (the word
  `conditional` in the rule listing), ADR-0026 (the fields of `RuleStep`).
- Risk: users read a conditional block on a text as protection against
  exfiltration. Mitigation: the rule sheet says under `body_contains` and
  `body_matches` that encoding passes them; `docs/SECURITY.md` statement 3;
  the shipped templates use only `body`, sizes and `findings_absent`.
- Risk: undetermined holds annoy, for example a server that compresses with
  `zstd`. Mitigation: `RULES_015` names the gap; supporting another coding
  in `decode_body` shrinks the gap for the scan and the conditions at once.
- Risk: the time budget makes a decision depend on load. It moves only towards
  block or hold, and `RULES_015` names it.
- Doors kept open: JSON field conditions as a further key in `when`; response
  conditions as a separate block; `or`/`not` if users ask for them; a Cedar
  translation of the whole rule.

## Affected issues

GitHub #82 (this decision), building on `HUM-022` (rule engine), `HUM-027`
(rules RPCs and `dry_run`), `HUM-039` (`path_prefixes`, passthrough rule),
`HUM-049` and `HUM-159` (hard block), `HUM-057` (`stream: true`, limits),
`HUM-065` (`humanitl rules test`), `HUM-104` (tiers), `HUM-247` and `HUM-266`
(decoding gaps), `HUM-407` (pause), `HUM-561` (unreadable rules), ADR-0005,
ADR-0007, ADR-0025 and ADR-0026. Implemented in phase 3 of sprint 9: item
3.7a the format, the evaluation and the fail-closed answers in the daemon,
with the changes to `docs/SECURITY.md` and `docs/THREAT-MODEL.md` of point 7;
item 3.7b the RPC fields, `rules add`, `rules test`, `rules dry-run`, the why
chain fields and the audit fields; item 3.7c the conditions in the app and
the terminal views.
