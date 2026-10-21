# Rules reference

Applies to: Humanitl 0.1

A rule decides a request without asking a human. Without a matching rule,
every request is held and a human decides (`ask`). This page describes the
file format of `rules.yaml`, how a rule matches, in which order rules are
evaluated, and the rules Humanitl ships with.

- [Where rules live](#where-rules-live)
- [How the examples on this page are checked](#how-the-examples-on-this-page-are-checked)
- [File layout](#file-layout)
- [Rule fields](#rule-fields)
- [Matching semantics](#matching-semantics)
- [Evaluation order](#evaluation-order)
- [Trying a rule](#trying-a-rule)
- [Recipes](#recipes)
- [Bundled rules](#bundled-rules)
- [Diagnostics](#diagnostics)

## Where rules live

| Source | Where | Written by | Can be removed |
|---|---|---|---|
| Your persistent rules | `$XDG_CONFIG_HOME/humanitl/rules.yaml`, mode `0600` | the daemon, on behalf of the UI and `humanitl rules add/update/remove` | yes |
| Your session rules | in memory only | the daemon, when a rule is created with `expires: session` | yes, and they end with the session |
| Profile rules | `[rules].inline` and `[rules].files` of a profile | you, in the profile | no, they belong to the profile |
| Bundled rules | [`rules/default.yaml`](../../rules/default.yaml) and the agent adapter | the build | no; switch them off with `disabled_bundled` |

The daemon reads `rules.yaml` when it starts. After editing the file by hand,
run `humanitl rules reload`; the daemon does not watch the file.

Parsing is strict. An unknown key, an unknown method, a pattern that does not
compile or a duplicate id is an error, and **one error rejects the whole
file**. The daemon then keeps running with the last valid rule set and reports
the diagnostics. A half-loaded rule set would be worse, because nobody could
tell which half is missing. Warnings do not reject the file; they are
reported next to the loaded rules.

## How the examples on this page are checked

Every fenced `yaml` block on this page is a complete `rules.yaml` and is
parsed with the daemon's own parser by the test `rules_reference_examples`
(`daemon/crates/rules/tests/reference_examples.rs`). A block that does not
parse fails the build. Snippets that are meant to be rejected are shown as
`text`, never as `yaml`.

Inside a block, a comment pair like this one is a probe:

```text
# humanitl rules test https://api.github.com/repos --method POST
# verdict: block
```

The test evaluates the probe against that block alone, with the real rule
evaluator, and compares the verdict. The command is the one you can run
yourself (see [Trying a rule](#trying-a-rule)); its first output line is the
verdict. Against a running daemon the answer can differ when a bundled rule or
a rule of yours matches first, so the probes on this page describe the block
on its own. The bundled block in [Bundled rules](#bundled-rules) is
additionally compared rule by rule with `rules/default.yaml`.

## File layout

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "*.github.com"
      method: [GET, HEAD]
    note: "read the GitHub API without asking"
disabled_bundled: []
# humanitl rules test https://api.github.com/repos/demo-project
# verdict: allow
# humanitl rules test https://api.github.com/repos/demo-project --method POST
# verdict: ask
```

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `version` | integer | yes | none | Format version. Only `1` exists; anything else, or a missing key, is `RULES_006`. |
| `rules` | list of rules | no | `[]` | The rules, in order. Order matters only inside one rank, see [Evaluation order](#evaluation-order). |
| `disabled_bundled` | list of rule ids | no | `[]` | Ids of bundled rules you switched off. See [`disabled_bundled`](#disabled_bundled). |

Any other top-level key is an error (`RULES_001`). An empty file with only
`version: 1` is valid and means "no rules of my own".

## Rule fields

A rule has an `action`, a `match` block, and a few flags. All keys are lower
case. Unknown keys inside a rule or inside `match` are an error
(`RULES_001`).

| Field | Type | Required | Default |
|---|---|---|---|
| [`id`](#id) | UUID string | no | a new id is generated |
| [`action`](#action) | `allow`, `block`, `ask`, `redact` | yes | none |
| [`match.host`](#matchhost) | host pattern string | yes | none |
| [`match.method`](#matchmethod) | list of method names | no | every known method |
| [`match.path`](#matchpath) | glob, or `~` followed by a regular expression | no | every path |
| [`match.path_prefixes`](#matchpath_prefixes) | list of strings | no | `[]`, every path |
| [`match.scheme`](#matchscheme) | `http`, `https`, `ws`, `wss` | no | every scheme |
| [`match.port`](#matchport) | integer 1 to 65535 | no | every port |
| [`match.upgrade`](#matchupgrade) | `websocket` | no | none, the rule matches only requests without an upgrade |
| [`expires`](#expires) | `never`, `session`, RFC 3339 timestamp | no | `never` |
| [`stream`](#stream) | boolean | no | `false` |
| [`allow_private`](#allow_private) | boolean | no | `false` |
| [`created_from`](#created_from) | flow id (UUID string) | no | none |
| [`bundled`](#bundled) | boolean | no | `false`, and ignored when set |
| [`passthrough_llm`](#passthrough_llm) | boolean | no | `false` |
| [`note`](#note) | string | no | none |

Every condition in `match` narrows the rule. A request matches when all
conditions that are present hold.

### `id`

A UUID in the usual hyphenated form, for example
`01920000-0000-7000-8000-000000000001`. The daemon generates UUIDv7 ids;
the parser accepts any UUID version.

- Leave it out when you write a rule by hand. A new id is generated on every
  read, so a rule without an id gets a different id after each reload. The
  daemon writes every rule back with its id the next time it saves the file.
- Two rules with the same id reject the file (`RULES_007`).
- Text that is not a UUID rejects the file (`RULES_001`).

### `action`

What happens to a matching request.

| Value | Effect in 0.1 | Exit code of `humanitl rules test` |
|---|---|---|
| `allow` | Forwarded without asking. Secret findings can still stop it (hard block). | `0` |
| `block` | Refused with `403`, without asking. | `10` |
| `ask` | Held for a human, the same as no rule. Useful to keep a narrow case in front of a broader `allow`. | `11` |
| `redact` | Accepted by the parser, but held like `ask` until the pseudonymizer exists. | `11` |

Values are lower case; `Allow` is an error.

### `match.host`

Required. One of four forms:

| Form | Example | Matches |
|---|---|---|
| Exact name | `github.com` | exactly this name, no subdomain |
| Label glob | `*.github.com`, `**.github.com` | names, label by label |
| `ip:` | `ip:192.168.1.50`, `ip:::1` | exactly this address |
| `cidr:` | `cidr:192.168.0.0/16`, `cidr:2606:4700::/32` | every address in this network |

The full matching rules, with a truth table, are in
[Matching semantics](#matching-semantics). Edge cases:

- A wildcard must be a whole label. `*foo.com` and `foo*.com` are errors
  (`RULES_003`), so are an empty label (`foo..com`) and an empty pattern.
- A port does not belong in the host. Use `match.port`.
- A bare IP address written as a name (`host: "140.82.112.3"`) is read as
  `ip:140.82.112.3` and reported as a warning (`RULES_002`). Write `ip:` to
  make the intent explicit.
- A punycode label you wrote yourself (`xn--mnchen-3ya.de`) is valid but a
  warning (`RULES_002`), because nobody can read which name it is. Write the
  Unicode name (`münchen.de`) instead; it is normalized to the same label.
- An IPv4-mapped network (`cidr:::ffff:192.168.0.0/112`) is converted to its
  IPv4 form. A prefix below 96 on such a network is an error (`RULES_003`).

### `match.method`

A list of HTTP methods, case-insensitive, stored in upper case. Known methods
are `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS`, `CONNECT` and
`TRACE`. Anything else in the list is an error (`RULES_001`).

- Without `method`, the rule matches every known method.
- A request with an unknown method (for example `PROPFIND`) never matches any
  rule and is always held.
- An empty list (`method: []`) is valid YAML and matches nothing. The rule
  then never decides anything.

### `match.path`

A glob over the path, or a regular expression when the value starts with `~`.
The query string is never compared: `/search` matches `/search?q=x`, and
appending `?a=b` can neither escape nor widen a rule.

- Glob: `*` matches within one segment and never crosses `/`; `**` crosses
  segments; `?`, `[abc]` and `{a,b}` work as usual. `/repos/*` matches
  `/repos/demo-project` but not `/repos/demo-project/issues`.
- Regular expression: unanchored, so it matches anywhere in the path. Write
  `^` and `$` when you mean the start and the end, for example
  `~^/v[0-9]+/models$`. The engine is the Rust `regex` crate (no
  backtracking), with a size limit of 1 MiB per compiled expression.
- An invalid glob or regular expression rejects the file (`RULES_005`).
- A rule with a path condition that lets a request through (`allow`,
  `redact`) never matches a path containing a `..` segment, in any spelling
  (`%2e%2e`, `..;x`, backslashes). A rule that blocks or asks matches the path
  as written **or** the resolved one, so a `block` for `/admin/**` also
  catches `/x/../admin/users`. The request is still forwarded with the path
  as written.

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "api.example.com"
      path: "/repos/*"
  - action: allow
    match:
      host: "api.example.com"
      path: "~^/v[0-9]+/models$"
  - action: allow
    match:
      host: "code.example.com"
      path: "/repos/me/**"
  - action: block
    match:
      host: "admin.example.com"
      path: "/admin/**"
# humanitl rules test https://api.example.com/repos/demo-project
# verdict: allow
# humanitl rules test https://api.example.com/repos/demo-project?page=2
# verdict: allow
# humanitl rules test https://api.example.com/repos/demo-project/issues
# verdict: ask
# humanitl rules test https://code.example.com/repos/me/demo-project
# verdict: allow
# humanitl rules test https://code.example.com/repos/me/../../user/keys
# verdict: ask
# humanitl rules test https://api.example.com/v1/models
# verdict: allow
# humanitl rules test https://api.example.com/v1/models/delete
# verdict: ask
# humanitl rules test https://admin.example.com/x/../admin/users
# verdict: block
```

### `match.path_prefixes`

A list of path prefixes. The condition holds when the path (without query)
starts with one of them. An entry that ends in `?` means exactly that path
instead of a prefix: `/v1/models?` matches `/v1/models` and
`/v1/models?limit=5`, but not `/v1/models/delete`.

- Every entry must start with `/` and be at least two characters long. `""`
  and `/` would match every path and reject the file (`RULES_005`).
- An entry ending in `?` is a warning (`RULES_005`), so that nobody writes an
  exact path while meaning a prefix.
- A prefix compares characters, not segments: `/api` also matches `/apifoo`.
  Write `/api/` when you mean the directory.
- When both `path` and `path_prefixes` are present, both must hold.
- The same `..` handling as for `path` applies.

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "llm.example.com"
      method: [POST, GET]
      path_prefixes: ["/v1/chat/completions?", "/v1/models?"]
# humanitl rules test https://llm.example.com/v1/chat/completions --method POST
# verdict: allow
# humanitl rules test https://llm.example.com/v1/models?limit=5
# verdict: allow
# humanitl rules test https://llm.example.com/v1/models/delete --method POST
# verdict: ask
```

### `match.scheme`

`http`, `https`, `ws` or `wss`, case-insensitive. Anything else is an error
(`RULES_001`).

The proxy records a request under the scheme of the connection that carries
it: `https` inside a `CONNECT` tunnel, `http` otherwise. A WebSocket handshake
is such a request with an upgrade, so `ws` and `wss` parse but match nothing
the proxy sees in 0.1. Use [`match.upgrade`](#matchupgrade) for WebSockets.

### `match.port`

The destination port, 1 to 65535. `0` is an error (`RULES_001`), and so is a
number above 65535. A request always carries a port; when the URL names none,
the default of the scheme is used (443 for `https`, 80 for `http`).

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "api.example.com"
      port: 8443
# humanitl rules test https://api.example.com:8443/status
# verdict: allow
# humanitl rules test https://api.example.com/status
# verdict: ask
```

### `match.upgrade`

The only value is `websocket` (case-insensitive). The upgrade dimension works
both ways:

- A rule **without** `upgrade` never matches a request that asks for an
  upgrade. Allowing a host does not allow WebSockets to it.
- A rule **with** `upgrade: websocket` never matches an ordinary request.

Any other value is an error (`RULES_001`). See the
[WebSocket recipe](#allowing-a-websocket).

### `expires`

How long the rule applies.

| Value | Meaning |
|---|---|
| `never` | Until you remove it. The default when the key is missing. |
| `session` | Only for the running session; kept in memory, never written to `rules.yaml`. |
| RFC 3339 timestamp, for example `"2099-01-01T00:00:00Z"` | Until that moment. Any offset is accepted and converted to UTC. From that moment on, the rule is skipped. |

- `never` and `session` are lower case. Anything else that is not an RFC 3339
  timestamp is an error (`RULES_001`).
- Quote timestamps, so YAML keeps them as text.
- Expired rules are skipped during evaluation and removed when the daemon
  prunes the rule set.
- Session rules are what the UI creates with the duration "this session" and
  what `humanitl rules add --expires session` creates. When a rule with
  `expires: session` is read from a file, it is bound to the session the
  daemon is running when it reads the file.

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "old.example.com"
    expires: "2025-01-01T00:00:00Z"
  - action: allow
    match:
      host: "new.example.com"
    expires: "2099-01-01T00:00:00+02:00"
  - action: allow
    match:
      host: "now.example.com"
    expires: session
# humanitl rules test https://old.example.com/
# verdict: ask
# humanitl rules test https://new.example.com/
# verdict: allow
# humanitl rules test https://now.example.com/
# verdict: allow
```

### `stream`

Only relevant for request bodies above `limits.hold_body_cap_bytes`
(32 MiB by default). Such a request does not fit into the buffer that a human
reviews, and it is blocked with `413`, even if a rule allows the host. A rule
with `action: allow` or `action: ask` and `stream: true` lets it out as a
stream, unchecked and without preview. With `ask`, the head is held and the
body flows only after a human approves. While
`hold.hard_block_checksum_secrets` is on, `stream: true` never applies.

On `block` the flag has no effect. Details are in [limits.md](limits.md); see
also the [streaming upload recipe](#allowing-a-streaming-upload).

### `allow_private`

Without this flag, the proxy refuses a destination that resolves to a private
address: RFC 1918, loopback, link-local or CGNAT. With `allow_private: true`
the destination may be opened.

- The flag opens the destination; it decides nothing. It applies to every
  action, so `action: ask` with `allow_private: true` holds every request and
  can still reach a private host after a human approves it. This is the rule
  the daemon suggests when it refuses a private address.
- The bundled LLM passthrough sets it, because a local model usually lives in
  your own network.
- It has no effect on `block`.

### `created_from`

The id of the flow from which the rule was created, for example when you
choose "remember" in the UI. It is informational; it does not change
matching. Text that is not a UUID is an error (`RULES_001`).

### `bundled`

Says where a rule comes from. A file cannot decide that: the daemon sets the
mark only for rules it loads from `rules/default.yaml` and the agent adapter.
In `rules.yaml` or a profile, `bundled: true` is ignored and reported as a
warning (`RULES_010`); the rule keeps working at its place in the list.

The reason is that a rank depends on the mark (see
[Evaluation order](#evaluation-order)). A file that could set it could promote
itself.

### `passthrough_llm`

Marks the declared passthrough to the language model: a matching request is
forwarded without being held, recorded in full, and findings in it produce a
warning (`LLM_005`) instead of stopping it. This is the one declared side
channel (see [SECURITY.md](../SECURITY.md)).

- Only allowed with `action: allow`; with any other action the file is
  rejected (`RULES_001`).
- The passthrough rule of 0.1 is built by the agent adapter from
  `llm.endpoint` and `llm.passthrough_paths`. You do not need to write one.
- Only that bundled rule is evaluated first. A passthrough in your own
  `rules.yaml` stays at its place in the list. In a profile, the flag is
  removed and reported.
- A passthrough that does not name a single host, a port, a scheme and a path
  condition is a warning (`RULES_008`): everything it covers goes out unheld.

### `note`

Free text: why the rule exists. It is shown in the Rules screen and kept in
`rules.yaml`. Write it; a rule without a reason is hard to review later.

### `disabled_bundled`

A top-level list of ids of bundled rules you switched off. A switched-off rule
stays visible in the Rules screen and decides nothing.
`humanitl rules disable <ID>` and `humanitl rules enable <ID>` edit this list.

- An id that names no bundled rule is kept, so that switching off survives a
  version in which the rule does not exist yet or any more.
- An id that names one of your own rules switches it off as well, with a
  warning (`RULES_010`): your own rules are removed, not disabled.
- Text that is not a UUID is an error (`RULES_001`).

```yaml
version: 1
rules: []
disabled_bundled:
  - 01920000-0000-7000-8000-000000000008
```

## Matching semantics

### Host names are normalized first

Before comparing, both the request host and the fixed labels of a pattern are
normalized: IDNA to ASCII (`münchen.de` becomes `xn--mnchen-3ya.de`), lower
case, trailing dot removed. A host that is not a valid name or address, such
as `0x8c527003` or `0177.0.0.1`, is refused before any rule sees it.

### Label globs

Hosts are compared label by label, never as strings. `*.github.com` cannot
match `evil-github.com` or `github.com.evil.io`, which a suffix or substring
check would.

- `*` matches exactly one label.
- `**` matches one or more labels.
- A pattern that starts with `**.` additionally matches the name without it:
  `**.example.com` matches `example.com` itself (apex exception). A `**` in
  the middle still needs at least one label.
- A pattern without `*` is exact.
- An IP address never matches a glob or an exact name, not even `**`. Only
  `ip:` and `cidr:` match addresses, and they never match names. A rule never
  resolves a name to check it against an address.

### Truth table

The table is test 4 of the escape tests (BACKLOG.md 4.5), extended by the
host matching table of the rules engine. Every row is a test in
`daemon/crates/rules/tests/host_table.rs` (named `host_NN`) or
`daemon/crates/rules/tests/escape_table.rs`.

| Pattern | Request host | Match | Why |
|---|---|---|---|
| `*.github.com` | `api.github.com` | yes | one label |
| `*.github.com` | `github.com` | no | `*` needs exactly one label |
| `*.github.com` | `evil-github.com` | no | labels, not a suffix |
| `*.github.com` | `github.com.evil.io` | no | labels, not a substring |
| `*.github.com` | `a.b.github.com` | no | two labels |
| `*.github.com` | `API.GITHUB.COM.` | yes | case and trailing dot are normalized |
| `**.github.com` | `github.com` | yes | apex exception |
| `**.github.com` | `api.github.com` | yes | one label |
| `**.github.com` | `a.b.c.github.com` | yes | several labels |
| `**.github.com` | `github.com.evil.io` | no | labels, not a substring |
| `**.github.com` | `notgithub.com` | no | labels, not a suffix |
| `github.com` | `github.com` | yes | exact |
| `github.com` | `www.github.com` | no | exact means exact |
| `github.com` | `GitHub.Com` | yes | case |
| `*.*.example.com` | `a.b.example.com` | yes | two single labels |
| `*.*.example.com` | `a.example.com` | no | one label short |
| `api.*.com` | `api.github.com` | yes | wildcard in the middle |
| `api.*.com` | `api.co.uk` | no | the last label decides |
| `**` | `anything.example` | yes | every name |
| `**` | `140.82.112.3` | no | an address never matches a glob |
| `*.github.com` | `140.82.112.3` | no | an address never matches a glob |
| `münchen.de` | `xn--mnchen-3ya.de` | yes | IDNA normalization |
| `ip:140.82.112.3` | `140.82.112.3` | yes | exact address |
| `ip:140.82.112.3` | `140.82.112.4` | no | another address |
| `ip:140.82.112.3` | `[::ffff:140.82.112.3]` | yes | IPv4-mapped IPv6 is the same address |
| `cidr:192.168.0.0/16` | `192.168.1.50` | yes | inside the network |
| `cidr:192.168.0.0/16` | `10.0.0.1` | no | outside the network |
| `cidr:192.168.0.0/16` | `192.168.1.50.nip.io` | no | a name, not an address |
| `ip:127.0.0.1` | `localhost` | no | no rule resolves a name |
| `*.github.com`, plain rule | `api.github.com` with WebSocket upgrade | no, so `ask` | an upgrade needs `upgrade: websocket` |

The last case of test 4, a 40 MB `POST`, is not decided by a rule: the body
cap applies first, and the request is buffered and held or blocked above the
cap, never streamed, unless a rule says `stream: true` (see [`stream`](#stream)).

The same table as probes:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "*.github.com"
# humanitl rules test https://api.github.com/
# verdict: allow
# humanitl rules test https://github.com/
# verdict: ask
# humanitl rules test https://evil-github.com/
# verdict: ask
# humanitl rules test https://github.com.evil.io/
# verdict: ask
# humanitl rules test https://a.b.github.com/
# verdict: ask
# humanitl rules test https://API.GITHUB.COM./
# verdict: allow
# humanitl rules test https://140.82.112.3/
# verdict: ask
# humanitl rules test https://api.github.com/ --upgrade websocket
# verdict: ask
```

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "**.github.com"
  - action: allow
    match:
      host: "a.**.example.com"
# humanitl rules test https://github.com/
# verdict: allow
# humanitl rules test https://a.b.c.github.com/
# verdict: allow
# humanitl rules test https://notgithub.com/
# verdict: ask
# humanitl rules test https://a.example.com/
# verdict: ask
# humanitl rules test https://a.x.y.example.com/
# verdict: allow
```

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "**"
  - action: block
    match:
      host: "ip:140.82.112.3"
  - action: allow
    match:
      host: "cidr:192.168.0.0/16"
    allow_private: true
# humanitl rules test https://anything.example/
# verdict: allow
# humanitl rules test https://140.82.112.3/
# verdict: block
# humanitl rules test http://192.168.1.50:8080/
# verdict: allow
# humanitl rules test http://10.0.0.1/
# verdict: ask
```

The first rule of the last block allows every name without any condition;
the parser accepts it with a warning (`RULES_008`).

### Path, method, scheme, port and upgrade

These are described with their fields: [`match.path`](#matchpath),
[`match.path_prefixes`](#matchpath_prefixes), [`match.method`](#matchmethod),
[`match.scheme`](#matchscheme), [`match.port`](#matchport),
[`match.upgrade`](#matchupgrade). A rule compares only the destination:
host, method, path, scheme, port and upgrade. It never looks at headers or the
body; those are for the secret detectors and for the human.

## Evaluation order

`RuleSet::evaluate` in `daemon/crates/rules/src/eval.rs` does exactly this:

1. A request with an unknown method gets `ask`, without looking at any rule.
2. The rules are checked in four ranks, in this order:
   1. the **bundled** LLM passthrough (`passthrough_llm` and `bundled`),
   2. your **session** rules (`expires: session`, not bundled),
   3. your **persistent** rules (`rules.yaml`),
   4. everything else that is **bundled**: profile rules, the agent adapter,
      `rules/default.yaml`.
3. Inside a rank, the list order applies and the **first matching rule wins**.
4. Expired rules and switched-off rules are skipped as if they did not exist.
5. If no rule matches, the verdict is `ask`: the request is held for a human.

Consequences:

- The rank belongs to the rule, not to its place in the list. Moving a rule
  only changes its place inside its own rank, which is why
  `humanitl rules reorder` moves a rule inside its group.
- What you decided a moment ago (a session rule) applies at once, even when an
  older, broader persistent rule would match.
- Your own rules override bundled ones. To overrule a bundled rule, write a
  rule with the same pattern, or switch it off with `disabled_bundled`.
- The passthrough to your model stays recognizable as such: no rule of yours,
  not even `block` for `**`, can shadow it. That is how the profile
  `llm-only` blocks everything except the model.
- A bundled rule with `expires: session` stays in rank 4.
- A session rule is bound to its session. A rule of another session counts as
  expired.
- There is no path on which an error in the rule set leads to a silent allow:
  a file that does not parse leaves the last valid set in place, and a rule
  that matches nothing leads to `ask`.

First match inside a rank:

```yaml
version: 1
rules:
  - action: block
    match:
      host: "**.github.com"
      method: [POST]
    note: "first matching rule wins"
  - action: allow
    match:
      host: "*.github.com"
# humanitl rules test https://api.github.com/repos --method POST
# verdict: block
# humanitl rules test https://api.github.com/repos
# verdict: allow
# humanitl rules test https://api.github.com/repos --method PATCH
# verdict: allow
```

## Trying a rule

`humanitl rules test <URL>` asks the running daemon how it would decide a
request to this URL, with the full rule set including bundled rules:

```text
humanitl rules test https://api.github.com/repos/demo-project
humanitl rules test https://registry.npmjs.org/left-pad --method HEAD
humanitl rules test https://realtime.example.com/socket --upgrade websocket
humanitl rules test https://api.github.com/repos --json
```

| Flag | Meaning |
|---|---|
| `URL` | A full URL with scheme, without `#fragment`. Normalization (punycode, trailing dot, case) is done by the daemon. |
| `--method M` | The method of the imagined request, one of the nine known methods, case-insensitive. Default `GET`. |
| `--upgrade websocket` | The imagined request asks for a WebSocket upgrade. |
| `--json` | One JSON object with `verdict`, `matched`, `rule_id`, `origin`, `position` and `passthrough`. |

The first output line is `verdict: allow`, `verdict: block` or
`verdict: ask`, the second names the rule that decided, or that none did. The
exit code is `0` for allow, `10` for block and `11` for ask or redact, so the
command works in scripts. The generated reference is in
[cli.md](cli.md#humanitl-rules-test).

To see which of the recently recorded requests a new rule would have matched,
before you add it, use `humanitl rules dry-run` with the same flags as
`humanitl rules add` (`--action`, `--host`, `--method`, `--path`, `--scheme`,
`--port`, `--upgrade`); see [cli.md](cli.md#humanitl-rules-dry-run).

## Recipes

### Only the language model

The profile `llm-only` runs a session without a human: only the configured
LLM endpoint is reachable, everything else is blocked without asking. Start
it with `humanitl run --profile llm-only`. The profile carries one rule:

```yaml
version: 1
rules:
  - action: block
    match:
      host: "**"
    expires: never
    note: "llm-only profile: block everything that is not the LLM passthrough"
# humanitl rules test https://example.com/
# verdict: block
# humanitl rules test https://registry.npmjs.org/left-pad
# verdict: block
```

The bundled passthrough to the model is evaluated in rank 1, before this
rule, so the model stays reachable. The profile also sets
`hold.ask_mode = "none"`: a request that no rule decides is not held for a
human but times out at once, and the agent gets `504`. The passthrough itself
is built from `llm.endpoint` and `llm.passthrough_paths` (see
[The LLM passthrough](#the-llm-passthrough)); to pass more model paths through,
add them to `llm.passthrough_paths` instead of writing a rule.

### npm, PyPI and GitHub, read-only

Package downloads and clones without asking; publishing and pushing are still
held:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "registry.npmjs.org"
      method: [GET, HEAD]
    note: "npm package metadata and tarballs"
  - action: allow
    match:
      host: "pypi.org"
      method: [GET, HEAD]
      path_prefixes: ["/simple/", "/pypi/"]
    note: "PyPI index and JSON API"
  - action: allow
    match:
      host: "files.pythonhosted.org"
      method: [GET, HEAD]
    note: "PyPI package files"
  - action: allow
    match:
      host: "github.com"
      method: [GET, HEAD]
    note: "GitHub pages and git ref discovery"
  - action: allow
    match:
      host: "github.com"
      method: [POST]
      path: "~^/[^/]+/[^/]+/git-upload-pack$"
    note: "git fetch and clone over HTTPS; git-receive-pack (push) stays held"
  - action: allow
    match:
      host: "codeload.github.com"
      method: [GET, HEAD]
    note: "release and branch archives"
# humanitl rules test https://registry.npmjs.org/left-pad
# verdict: allow
# humanitl rules test https://registry.npmjs.org/-/user/org.couchdb.user:demo --method PUT
# verdict: ask
# humanitl rules test https://pypi.org/simple/requests/
# verdict: allow
# humanitl rules test https://pypi.org/account/login/
# verdict: ask
# humanitl rules test https://github.com/demo-org/demo-project.git/info/refs?service=git-upload-pack
# verdict: allow
# humanitl rules test https://github.com/demo-org/demo-project.git/git-upload-pack --method POST
# verdict: allow
# humanitl rules test https://github.com/demo-org/demo-project.git/git-receive-pack --method POST
# verdict: ask
```

These rules are in your rank 3, so they also override the bundled `ask` rule
for `registry.npmjs.org`.

### A company registry

A registry in your own network resolves to a private address, which the proxy
refuses unless the rule says `allow_private: true`. Reading is allowed;
publishing stays with a human, and the second rule opens the private address
for the request once it is approved:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "npm.corp.example"
      method: [GET, HEAD]
      scheme: https
      port: 443
    allow_private: true
    note: "company npm registry, read-only"
  - action: ask
    match:
      host: "npm.corp.example"
      scheme: https
      port: 443
    allow_private: true
    note: "publishing to the company registry needs a human"
# humanitl rules test https://npm.corp.example/@corp%2fui
# verdict: allow
# humanitl rules test https://npm.corp.example/@corp%2fui --method PUT
# verdict: ask
# humanitl rules test http://npm.corp.example/@corp%2fui
# verdict: ask
```

Point the package manager inside the sandbox at the registry as usual (for
npm, `registry=https://npm.corp.example/` in the project's `.npmrc`).

### Allowing a WebSocket

A WebSocket is a separate decision. A rule for the host alone does not cover
it; the rule has to say `upgrade: websocket`:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "realtime.example.com"
      path: "/socket"
      upgrade: websocket
    note: "live updates for the demo-project dashboard"
  - action: allow
    match:
      host: "api.example.com"
# humanitl rules test https://realtime.example.com/socket --upgrade websocket
# verdict: allow
# humanitl rules test https://realtime.example.com/socket
# verdict: ask
# humanitl rules test https://api.example.com/stream --upgrade websocket
# verdict: ask
```

Do not set `scheme: wss`; the handshake travels as an `https` request (see
[`match.scheme`](#matchscheme)).

### Allowing a streaming upload

A request body above `limits.hold_body_cap_bytes` is blocked with `413`
(`LIMIT_001`), even when a rule allows the host. To let such an upload out, a
rule needs `stream: true`. With `ask`, the head is held and the body flows
only after you approve; with `allow` it goes at once. Either way the body is
not searched for secrets and has no preview.

```yaml
version: 1
rules:
  - action: ask
    match:
      host: "uploads.example.com"
      method: [POST]
      path_prefixes: ["/v1/files"]
    stream: true
    note: "large dataset uploads for demo-project; held, then streamed"
# humanitl rules test https://uploads.example.com/v1/files --method POST
# verdict: ask
# humanitl rules test https://uploads.example.com/v1/files --method PUT
# verdict: ask
```

The probes look the same with and without `stream: true`; the flag only
changes what happens to a body above the cap. Keep the rule narrow (host,
method, path), and prefer `ask` over `allow`. While
`hold.hard_block_checksum_secrets` is on, `stream: true` never applies and the
upload is blocked. See [limits.md](limits.md) for the limits involved.

## Bundled rules

Humanitl ships two kinds of rules with the OpenCode adapter. Both are in rank 4
except the passthrough, cannot be deleted, and show the badge "Bundled" in the
Rules screen.

### The LLM passthrough

Built by the agent adapter from `llm.endpoint`, with the fixed id
`01920000-0000-7000-8000-0000000000ff`. It has no file of its own. For an
endpoint like `http://192.168.1.20:11434/v1` it is equivalent to:

```yaml
version: 1
rules:
  - id: 01920000-0000-7000-8000-0000000000ff
    action: allow
    match:
      host: "192.168.1.20"
      method: [POST, GET]
      path_prefixes:
        - "/v1/chat/completions?"
        - "/v1/completions?"
        - "/v1/responses?"
        - "/v1/embeddings?"
        - "/v1/models?"
        - "/api/chat?"
        - "/api/generate?"
        - "/api/embed?"
        - "/api/embeddings?"
        - "/api/tags?"
        - "/api/show?"
        - "/api/ps?"
        - "/api/version?"
      scheme: http
      port: 11434
    allow_private: true
    passthrough_llm: true
    note: "LLM passthrough for http://192.168.1.20:11434/v1. Logged, never held."
# humanitl rules test http://192.168.1.20:11434/v1/chat/completions --method POST
# verdict: allow
# humanitl rules test http://192.168.1.20:11434/api/pull --method POST
# verdict: ask
# humanitl rules test http://192.168.1.20:11434/v1/files --method POST
# verdict: ask
```

Reasons: exactly one host, scheme and port, because it is the one request
type that is not held; only the inference endpoints as exact paths, so that
endpoints which change the server (file uploads, fine-tuning, model pulls and
deletes) still go through the queue; `allow_private`, because a local model
lives in your network. The paths come from `llm.passthrough_paths`, default
`["/v1/", "/api/"]`: `/v1/` expands to the OpenAI-compatible inference
endpoints under the base path of the endpoint, `/api/` to the native Ollama
inference endpoints (only for an endpoint at the root of the server), and any
other entry is kept as a prefix. In a
hand-written `rules.yaml` the parser reads `192.168.1.20` as
`ip:192.168.1.20` with a warning (`RULES_002`); the adapter builds the rule
directly.

### `rules/default.yaml`

The purpose of these rules: on first start, nobody should see a flood of
requests they did not cause. The first request a human sees should be one they
understand. The ids are fixed so that `disabled_bundled` can point at them.
`bundled: true` is deliberately not in the file; the loader sets it.

<!-- bundled-rules: rules/default.yaml -->
```yaml
version: 1
rules:
  - id: 01920000-0000-7000-8000-000000000001
    action: block
    match: { host: "models.dev" }
    expires: never
    note: "models.dev is the model catalog that the OpenCode documentation names. The installed 1.18.25 uses models.opencode.ai instead (rule ...0009); this rule covers the documented host. Humanitl ships the catalog as a file, so neither call is needed."

  - id: 01920000-0000-7000-8000-000000000002
    action: block
    match: { host: "api.github.com", path: "/repos/anomalyco/opencode/releases/**" }
    expires: never
    note: "Update check. Disabled via OPENCODE_DISABLE_AUTOUPDATE; this rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000003
    action: block
    match: { host: "**.posthog.com" }
    expires: never
    note: "Product analytics endpoint used by several agent CLIs. The string posthog does not appear in the OpenCode 1.18.25 binary at all; this rule is a lock for other agents and for a later version, not a claim about what OpenCode does today."

  - id: 01920000-0000-7000-8000-000000000004
    action: block
    match: { host: "**.sentry.io" }
    expires: never
    note: "Crash reporting would carry stack traces including file paths. OpenCode 1.18.25 bundles a Sentry shim but calls no sentry.io address; this rule is a lock for other agents and for a later version, not a claim about what OpenCode does today."

  - id: 01920000-0000-7000-8000-000000000005
    action: block
    match: { host: "opncd.ai" }
    expires: never
    note: "Session sharing uploads the whole conversation. OpenCode 1.18.25 posts it to opncd.ai (POST /api/share, then /api/share/<id>/sync), and that is the only place the host appears in the binary, so the rule carries no path. What else the host may serve is unknown. Sharing is off by config as well; this rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000006
    action: block
    match: { host: "**.exa.ai" }
    expires: never
    note: "Hosted web search backend for OpenCode's websearch tool."

  - id: 01920000-0000-7000-8000-000000000007
    action: block
    match: { host: "**.parallel.ai" }
    expires: never
    note: "Hosted web search backend (Parallel)."

  - id: 01920000-0000-7000-8000-000000000008
    action: ask
    match: { host: "registry.npmjs.org", method: [GET, HEAD] }
    expires: never
    note: "OpenCode installs provider packages on first use. Shown as a normal held request so the user learns what it is."

  - id: 01920000-0000-7000-8000-000000000009
    action: block
    match: { host: "models.opencode.ai" }
    expires: never
    note: "OpenCode 1.18.25 fetches its model catalog from models.opencode.ai; the documentation and the rule above name models.dev. Both are blocked because Humanitl ships the catalog as a file."

  - id: 01920000-0000-7000-8000-00000000000a
    action: block
    match: { host: "opencode.ai", path: "/share/**" }
    expires: never
    note: "The share links of OpenCode are read under this path. The upload itself goes to opncd.ai in 1.18.25; this rule covers the case that it moves back to the main host."
# humanitl rules test https://models.dev/api.json
# verdict: block
# humanitl rules test https://eu.posthog.com/capture
# verdict: block
# humanitl rules test https://posthog.com/
# verdict: block
# humanitl rules test https://opncd.ai/api/share --method POST
# verdict: block
# humanitl rules test https://registry.npmjs.org/@ai-sdk%2fopenai
# verdict: ask
# humanitl rules test https://opencode.ai/docs/
# verdict: ask
```

| Id (suffix) | Host | Action | Reason |
|---|---|---|---|
| `…0001` | `models.dev` | block | Model catalog named by the OpenCode documentation. Humanitl ships the catalog as a file. |
| `…0002` | `api.github.com`, path `/repos/anomalyco/opencode/releases/**` | block | Update check of OpenCode. Disabled by `OPENCODE_DISABLE_AUTOUPDATE`; this is the second lock. |
| `…0003` | `**.posthog.com` | block | Product analytics used by several agent CLIs; a lock for other agents and later versions. |
| `…0004` | `**.sentry.io` | block | Crash reports would carry stack traces with file paths; a lock for other agents and later versions. |
| `…0005` | `opncd.ai` | block | Session sharing uploads the whole conversation. Sharing is also off by config; this is the second lock. |
| `…0006` | `**.exa.ai` | block | Hosted web search backend of the OpenCode websearch tool. |
| `…0007` | `**.parallel.ai` | block | Hosted web search backend (Parallel). |
| `…0008` | `registry.npmjs.org`, `GET`/`HEAD` | ask | OpenCode installs provider packages on first use; held on purpose, so you learn what it is. |
| `…0009` | `models.opencode.ai` | block | The model catalog host the installed OpenCode actually uses. |
| `…000a` | `opencode.ai`, path `/share/**` | block | Share links, in case the upload moves back to the main host. |

The `ask` rule `…0008` behaves like no rule at all; it exists so that the
Rules screen can explain the request. Any rule of yours for
`registry.npmjs.org` overrides it.

## Diagnostics

| Code | Severity | When |
|---|---|---|
| `RULES_001` | error | The file is not valid YAML, has an unknown key, a wrong type, an invalid id, method, scheme, port, upgrade, expiry or flow id, or `passthrough_llm` without `allow`. |
| `RULES_002` | warning | A host pattern contains a punycode label, or is a bare IP address. |
| `RULES_003` | error | A host pattern is invalid. |
| `RULES_005` | error or warning | A path glob, regular expression or prefix is invalid (error); a prefix ends in `?` (warning). |
| `RULES_006` | error | `version` is missing or not `1`. |
| `RULES_007` | error | Two rules share an id. |
| `RULES_008` | warning | A rule allows every host without any condition, or a passthrough is not bounded. |
| `RULES_010` | warning | `bundled` in a file, or `disabled_bundled` names one of your own rules. |

Every error rejects the whole file; the message names the rule index, the
field and, where known, the line, for example
`rules[3].match.host (line 12): host pattern "*foo.com" is invalid: a wildcard must be a whole label`.
