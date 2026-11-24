# ADR-0028 · A request that waits for a decision gets a fixed head, and two kinds get a card that explains and never decides

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-24

## Context

A request that waits for a decision is shown as it arrived, and reading it
takes time. The owner's idea for sprint 9 (GitHub #83): recognise typical
requests and show them at a glance, for example a web search as the search
text it carries. The bytes of every such request are written by an agent
that may be compromised (`BACKLOG.md` 4.3, attackers a to c), so whatever
turns those bytes into a sentence a person believes is a new surface for
deception. Owner decision E8 of 2026-11-23: a card may leave a visible
remainder below a fixed threshold, above it there is only the raw view, and
requests that write get no card.

What exists today, on `main` at `f79774de`:

- **The app.** The middle pane of the Decisions view is `RequestCard`
  (`app/lib/features/intercept/widgets/request_card.dart`). Its header
  `_CardHeader` shows the method badge, `flow.url` in `mono14` and the
  countdown. `flow.url` is built from `Authority`
  (`app/lib/core/domain/http.dart`). Below the header come
  notices (recording outage, timeout, WebSocket, unreadable request), then
  three sections: `SectionQuery` splits `path_and_query` in Dart at `?` and
  `&` and decodes each pair with `Uri.decodeQueryComponent`, falling back to
  the raw text; `SectionHeaders` masks the values of `authorization`,
  `proxy-authorization`, `cookie`, `set-cookie` and `x-api-key` until the eye
  is clicked; `BodyView` (`app/lib/core/body/`) shows the body as raw text,
  JSON tree, form or hex, replaces direction and invisible characters before
  drawing (`sanitizeBodyText`, length-preserving) and marks findings. The
  right pane shows the catalogue card and the popularity rank from
  `DomainInfo`. Nothing in the app says how many bytes of the request it
  explained. `SectionQuery` is the only place that splits the query into
  pairs; the findings scan does not split it.
- **The terminal.** `humanitl run --ask terminal` draws an ASCII box
  (`daemon/bin/humanitl/src/ask_terminal.rs`, `from_detail`): the request
  line, the origin tool, the body size, the first finding and their count,
  the catalogue id and rank. Every field goes through `render::plain`, and
  every scalar outside ASCII becomes `?`, so that the frame stays where it
  is. Keys: `a` allow once, `s` allow for the session, `b` block, `r` rule,
  `e` edit, `v` view, `n` next, `Esc` close. `v` (`Step::ViewBody`,
  `cmd/moderate.rs`) streams the body only; the headers and the query are not
  shown anywhere in the terminal before the decision.
- **The contract.** `GetFlow` returns `FlowDetail` with `request`
  (`HttpRequest`: `method`, `method_raw`, `scheme`, `authority`,
  `path_and_query`, `headers`, `body` as `BodyRef`, `version`), `findings`,
  `body_preview` (at most 4,096 characters), `findings_truncated`,
  `send_refusal` (in the summary, `HOLD_004`) and `domain`.
  `FlowEvent.Held` carries `flow_id`, `deadline`, `queue_bytes` and
  `queue_count`. `FlowSummary.request_size` is the size of the body only.
- **The decision.** `DecideRequest` names flows by id (`flow_ids`, field 1)
  with `allow` (2), `block` (4), `remember` (5), `allow_edited` (7) and
  `acknowledged_findings` (8); fields 3 and 6 are reserved. A decision names a
  flow, not the bytes the person looked at. The held request cannot change
  inside the daemon while it waits, but nothing ties what a client showed to
  what the daemon forwards. The body has a digest (`BodyRef.sha256`,
  `daemon/crates/core-types/src/http.rs`); the request as a whole has none.
- **The audit record.** `flow.received` (`FlowReceived`,
  `daemon/crates/audit/src/kinds.rs`) carries method, scheme, host, port,
  `path_hash`, size and finding kinds; `flow.decided` (`FlowDecided`) carries
  the decision, `rule`, `edited`, `replacements`, `unresolved_findings`,
  `acknowledged`, `allowlisted_added` and `decided_by`. Nothing says what the
  person saw.
- **Findings.** `humanitl-findings` scans headers, the whole query as one
  entry, once raw and once after one `percent_decode` pass without splitting
  at `&` or `=` and without reading `+` as a space (`input.rs`, lines 183 to
  200), and the body decoded once by `decode_body` within
  `limits.preview_cap_bytes`; a finding leaves the crate only as `kind`,
  `location`, span, `value_hash` and a masked `display_prefix`.
- **Before the hold.** A `Host` header or `:authority` that does not match the
  `CONNECT` target and the SNI is blocked without asking
  (`BlockReason::AuthorityMismatch`); a request that waits for a decision has
  therefore one host. A request over `limits.hold_body_cap_bytes` waits only
  through a `stream: true` rule, with its body unread.
- **What the agent reads.** ADR-0026 point 5 closed the vocabulary of the
  block body and of `humanitl.internal`. ADR-0027 added content conditions
  and their findings; `RuleStep` of the why chain names them.

The expert interviews behind this decision (ten simulated specialists in
interface deception, exfiltration, automation bias, accessibility, parsing
and privacy) agreed on one condition: the card is a reading aid built only
from the bytes, it never produces trust, and every byte it does not explain
stays visible.

"Item N.M" below is the numbering of the sprint's tracking issue on GitHub
(#86), as in ADR-0025 to ADR-0027; item 3.8 is the renderer, in five issues.

Hard constraints: the three sandbox guarantees, `tests/escape/`, the hard
block and the terminal output filter stay as they are. Every capability is an
RPC first (ADR-0018); app and terminal draw what the background service sends
and decide nothing. No new port (ADR-0015), and the coupling counts of
`tools/coupling-baseline.toml` do not rise. The agent's vocabulary of
ADR-0026 point 5 stays closed. `docs/SECURITY.md` and `docs/THREAT-MODEL.md`
are not changed by this decision; point 10 lists what the implementing issues
write into them.

## Decision

We give every request that waits for a decision a fixed head computed by the
background service, and we add a renderer: a pure, deterministic function in
the background service that turns a held request into labelled fields and an
explicit remainder, served by a new RPC `Interpret`. Two renderers exist,
`web-search` and `package-download`, each an allowlist that fails closed to
the raw view. The renderer explains and never decides: it changes no rule, no
decision, no default focus and no key. The decision is bound to a digest of
the bytes that were shown, and the audit record names the renderer, its
version and the view that was open.

### 1. Principle: the renderer explains and never decides

- **Pure.** `humanitl_render::interpret(&HeldInput) -> Interpretation` in a
  new crate `humanitl-render` (`daemon/crates/render`). It depends on
  `humanitl-core` only, has no IO, no async, no clock, no randomness, no
  environment and no configuration. It is listed in
  `tools/offline-crates.toml` and in `tools/deps-allow.toml` (used by
  `humanitl-proxy` and `humanitl-ipc`). Same input, same output, on every
  machine; a test calls it twice on every fixture and compares.
- **One object, one tokenizer.** `HeldInput` carries the core `HttpRequest`
  as held (method, scheme, `Authority`, `path_and_query`, the `HeaderMap`
  with its lowercase names and its duplicates grouped by name, `BodyRef`),
  whether the body was buffered, and nothing of `hyper`, `rustls` or
  `rcgen`. The query tokenizer is new, in `humanitl_core::query`: it splits at
  `&` and `=` and decodes once (`%XX`, `+` as a space) by the rules of point
  3. The findings scan keeps scanning the whole query as one entry, raw and
  decoded, as today. A test feeds the same queries (`%XX`, `%2B`, `+`, `%20`)
  through both and asserts that, with `+` read as a space on the scan's side,
  every decoded value of the card is a substring of the scan's decoded entry,
  so a finding in a decoded value can never be missing from what the card
  shows.
- **No outside knowledge.** No network, no DNS, no file, no catalogue entry,
  no rank, no favicon, no title, no model. The only knowledge is the
  renderer's own fixed tables (hosts, paths, parameters, header grammars),
  versioned in code. A renderer is added only by a code change with a section
  on its abuse cases in `docs/THREAT-MODEL.md`; there are no plugins and no
  user-defined renderers.
- **When it runs.** Once per flow, in `humanitl-proxy`, after the scan and
  before `FlowState::Held`, on the bytes that wait. The result is stored with
  the hold entry next to the digest of point 8; `Interpret` returns the stored
  result, so the app, the terminal and the audit record see the same one.
  Every input is bounded (point 3), so the call is linear and small and runs
  inline; there is no time budget to hit and no partial result.
- **What it may not touch.** The renderer's output never reaches
  `RulesPipeline`, the hold queue's decision, `Decide`, a rule suggestion, a
  notification or the sandbox. A rule made from a request with a card is made
  from the request's method, host and path as today (`remember` in
  `DecideRequest`), never from the renderer's kind; there is no "allow every
  web search".

### 2. Coverage invariant and the threshold

**The canonical input** of a held request is the sequence of its segments in
this order: `METHOD` (the method token as held), `SCHEME` (`http` or
`https`), `AUTHORITY` (`host:port`, host as A-label), `TARGET`
(`path_and_query` as held), then for every header in the order of
`HeaderMap::iter` (names lowercase, the values of one name next to each
other) `HEADER_NAME` and `HEADER_VALUE`, then `BODY`. "As held" is the core
`HttpRequest` the flow carries, not the wire form: `build_outgoing`
(`daemon/crates/proxy/src/upstream.rs`) drops hop-by-hop headers, `host` and
`content-length`, sets its own `host` and `content-length`, and over a
decision proxy writes the absolute form and adds the daemon's
`Proxy-Authorization`. Hop-by-hop headers of the agent (`connection`,
`keep-alive`, `te`, `upgrade` and the others of `HOP_BY_HOP`) are counted and
shown like any header, although they are not forwarded; what the daemon adds
is not the agent's and is not counted. The HTTP version is chosen by the
proxy's parser from a closed set and carries no bytes of the agent; it is not
a segment. **Total** is the sum of the segment lengths: the counted bytes the
head shows (point 5), not the size on the wire; today's `request_size` is
only the body.

**The invariant.** Every byte of the canonical input belongs to exactly one
span, and every span is one of three:

- **head**: `METHOD`, `SCHEME`, `AUTHORITY` and a `Host` header equal to the
  authority (point 3), always shown by the head of point 5;
- **explained**: assigned to a field of the card, shown on the card;
- **remainder**: not explained, listed under the card with its place, its
  length and its bytes.

`head + explained + remainder = total`, the spans are disjoint and their
union covers every segment. An explained field shows its bytes: decoded once
where the field says so, with every byte visible (point 4), never shortened,
never collapsed behind a click.

**Where the path and the separators count.** `TARGET` is split by the
renderer's anchored path pattern and the tokenizer of point 1, and every
byte of it lands in a field or in the remainder:

- the literal bytes of the matched pattern (`/search`, `/`, `/html/`,
  `/-/`, `.tgz`, `/packages/`, `/simple/`, the `/` between captures) and the
  `?` that starts the query belong to the field `PATH`, which every card has
  and which shows the path as held, with the bytes that belong to other
  fields highlighted; its spans are only the literal bytes;
- a capture of the pattern (`{name}`, `{version}`, `{file}`) belongs to its
  own field (point 6);
- a capture that repeats or derives from another belongs to that field's
  span and must match it exactly, else the renderer stops with
  `PATH_AMBIGUOUS`: the npm tarball's `{base}` and the second `{name}` of
  `/crates/{name}/{name}-{version}.crate` belong to `PACKAGE_NAME`, the
  crates index prefix `{c}`, `{ab}` or `{cd}` belongs to `PACKAGE_NAME` and
  must be the prefix Cargo derives from `{name}`;
- `FILE_HASH_PATH` covers the whole `{h2}/{h2}/{h60}` of
  `files.pythonhosted.org`, the two `/` between them included;
- a query pair belongs to one span together with its name, its `=` and the
  `&` before it (the first pair has none): `SEARCH_TEXT` for `q=…`, a
  `SEARCH_OPTION` for a known parameter, and the remainder for any other.

A minimal search, `GET /search?q=rust` with only explained headers, is
therefore `EXPLAINED` with remainder 0, and P1 of point 9 holds for every
request. The `Coverage` message of point 6 carries the four
numbers and every remainder span, and clients draw the numbers as they are.

**The threshold.** `REMAINDER_MAX_BYTES = 64`, a constant of
`humanitl-render`, not a setting (behaviour is fixed, sprint 9). With a
remainder of 0 and no mark the state is `EXPLAINED`; with 1 to 64 bytes it
is `PARTIAL` (its remainder marks are `UNKNOWN_PARAMETER` and
`UNKNOWN_HEADER`, point 4), unless a field carries another mark, which makes
it `WARNING`; both open the raw view first (point 7), and on the card the remainder stands
directly under the fields, uncollapsed, in the same
type size as the fields, each span as "query parameter `ei`, 22 bytes:
`…`" or "header `x-trace`, 19 bytes: `…`"; with 65 bytes or more there is no
card (`RENDER_STOP_REMAINDER_OVER_THRESHOLD`), and the raw view says how many
bytes were not explained. The numbers are absolute bytes; no percentage and
no confidence value is shown, because a deterministic renderer matches or it
does not, and a percentage would read as a score.

### 3. Allowlist, and failing closed to the raw view

A renderer matches only when all of the following hold; any other request,
any parse error, any ambiguity and any limit gives `RENDER_STATE_NONE` with a
`RenderStop` reason and the raw view.

**Shared preconditions** (both renderers):

| Check | Stop reason |
|---|---|
| method `GET` or `HEAD`; every other method writes and gets no card, `OPTIONS`, `CONNECT`, `TRACE` and unknown methods included | `METHOD_WRITES` |
| scheme `https`, port 443 | `NOT_HTTPS` |
| host equal byte for byte to an entry of the renderer's host table after the normalisation of `docs/SECURITY.md` section 7 (A-label, lowercase, no trailing dot); no suffix, glob or fuzzy match; an IP literal never matches | `NO_RENDERER` |
| a `Host` header is absent (HTTP/2) or equal to `host` or `host:443`, and present at most once | `HOST_HEADER` |
| no body: `BodyRef.size == 0`, no `Content-Length` other than `0`, no `Transfer-Encoding`, body buffered | `UNEXPECTED_BODY`, `BODY_NOT_BUFFERED` |
| no `Upgrade` header | `UPGRADE` |
| no `Cookie`, `Authorization`, `Proxy-Authorization`, `X-Api-Key`; a credential does not fit a card | `CREDENTIAL_PRESENT` |
| no `X-HTTP-Method-Override`, `X-HTTP-Method`, `X-Method-Override` | `METHOD_OVERRIDE` |
| the path matches one of the renderer's anchored patterns exactly; no `..`, `.` or empty segment, no `//`, no `;`, no percent escape in the path except where a pattern names one | `NO_RENDERER`, `PATH_AMBIGUOUS` |
| every query parameter name appears at most once; names are compared byte for byte and case-sensitively (`Q` is not `q`, it is an unknown parameter); a name contains no `%` and no `+`; the separator is `&` only (`;` stops); every `%` in a value is followed by two hex digits; a value decodes once (`+` as space, `%XX`) to valid UTF-8; the decoded value contains no further `%XX` (double encoding) | `DUPLICATE_PARAMETER`, `AMBIGUOUS_ENCODING`, `INVALID_UTF8` |
| every pair has a non-empty name and an `=`: no empty pair (`a&&b`), no trailing `&`, no `?` without a query, no name without `=`, because servers read these differently | `QUERY_MALFORMED` |
| limits: `TARGET` at most 2,048 bytes, at most 32 headers, each header value at most 1,024 bytes, all header bytes at most 8 KiB | `LIMIT` |

**Headers.** A header is explained when its name stands in the renderer's
header table and its value matches the grammar there within its length;
otherwise its name and value are remainder. Shared table:

| Header | Grammar and limit | Shown as |
|---|---|---|
| `user-agent` | printable ASCII (0x20 to 0x7E), at most 512 bytes | verbatim, field "sent along" |
| `accept` | `[A-Za-z0-9*/.+;=, -]`, at most 256 bytes | verbatim |
| `accept-encoding` | comma list of `gzip`, `deflate`, `br`, `zstd`, `identity`, `*`, each with optional `;q=` and a number | verbatim |
| `accept-language` | `[A-Za-z0-9*,;=. -]`, at most 64 bytes | verbatim |
| `connection` | `keep-alive` or `close` | verbatim |
| `cache-control`, `pragma` | `no-cache` or `max-age=0` | verbatim |
| `if-none-match` | quoted or weak entity tag, printable ASCII, at most 128 bytes | verbatim |
| `if-modified-since` | IMF-fixdate (RFC 9110 5.6.7) | verbatim |

Each renderer adds rows (point 6). Every other header, `referer` and
`origin` included, is remainder.

### 4. No trust signal; a card can only raise suspicion

- **Never a sign of trust.** A card has no green (`allowed` `#4FBF8C` is
  never used on it), no check mark, no shield, lock or badge, no brand name,
  logo or favicon. Its words never include "safe", "secure", "harmless",
  "only", "normal", "verified", "trusted", nor „sicher", „harmlos", „nur",
  „normal", „verifiziert", „vertrauenswürdig". `make l10n-lint` fails on
  these words in every ARB key with the prefix `renderCard`. The card is drawn
  on the neutral `fg` ladder, like the body view.
- **The sentence.** Every card opens with a fixed line from ARB: "Looks like a
  web search at `www.google.com` (`web-search` v1). A reading aid, not a
  check." and ends with a fixed line: "Not checked: what the server does with
  this, redirects, the response, and what several requests reveal together."
  The host in that line is the head's host, never a provider name.
- **Content of the agent stays content.** Field values stand in a framed box
  labelled "Sent by the agent, not checked", in monospace, one line per field.
  In the app each value is a text-direction isolate, so no character of it
  can reorder the text around it; the terminal shows `display_ascii`, which
  holds no direction character at all. No Markdown, no link, no system font,
  no status colour inside.
- **Every byte visible.** The background service builds two display forms
  per field: `display` with every C0, C1 and `DEL` control, every direction
  mark and isolate (`U+061C`, `U+200E`, `U+200F`, `U+202A` to `U+202E`,
  `U+2066` to `U+2069`), every zero-width and format character (`U+00AD`,
  `U+200B` to `U+200D`, `U+2060` to `U+2064`, `U+FEFF`), every tag character
  (`U+E0000` to `U+E007F`) and every variation selector replaced by
  `⟨U+XXXX⟩`; and `display_ascii`, the same with every other non-ASCII scalar
  as `\u{XXXX}`, for the terminal. Clients draw these strings, never the raw
  value.
- **Marks.** A card may mark a field or its remainder. The remainder marks
  `UNKNOWN_PARAMETER` and `UNKNOWN_HEADER` give `PARTIAL`; every other mark
  gives `WARNING`, which takes precedence; both open the raw view first
  (point 7). Marks are
  facts phrased as comparisons, at the field: "search text 1,812 characters,
  usual under 128", "40 characters look like base64". Their kinds and
  thresholds are constants of `humanitl-render`:

| Mark | When |
|---|---|
| `LONG_VALUE` | the search text is longer than 128 characters; a package field is longer than its threshold in point 6 |
| `HIGH_ENTROPY` | a value of at least 24 characters with at least 4.2 bits of Shannon entropy per character |
| `BASE64_RUN` | a run of at least 32 characters of `[A-Za-z0-9+/_-]` with `=` padding allowed |
| `HEX_RUN` | a run of at least 32 characters of `[0-9A-Fa-f]`, outside fields that are hex by format (point 6) |
| `INVISIBLE_CHARACTERS` | the value contains any character `display` replaces; the mark names their number |
| `CONTAINS_URL` | the value contains `://` or starts with `www.` |
| `UNKNOWN_PARAMETER` | the remainder contains a query parameter |
| `UNKNOWN_HEADER` | the remainder contains a header |

  A mark never removes a field and never softens a sentence. A finding is no
  mark: findings stand in the head (point 5). **Every remainder carries a
  mark.** After the preconditions of point 3 a remainder can stand only in a
  query parameter (`UNKNOWN_PARAMETER`) or a header (`UNKNOWN_HEADER`); a
  path that does not match, a body and anything else stop the renderer. A
  `PARTIAL` card therefore always has a remainder mark, and neither a
  `PARTIAL` nor a `WARNING` card opens by default (point 7).

### 5. The fixed head of every request that waits for a decision

The head is computed by the background service for every held flow, with or
without a card, and stands above both views in the app and in the terminal.
It is `HeldHead` in `Interpretation` (point 6); clients compute none of it.

| Field | Content |
|---|---|
| host | `Authority.host`, the A-label, in monospace, always; when `display_host` differs, the U-label next to it with every non-ASCII character marked (`non_ascii_at`), never the U-label alone; in the terminal the A-label and the words "non-ASCII name" |
| port | shown only when it is not the scheme's default |
| method | the token as held (`method_raw` for an unknown method) |
| scheme | `http` or `https` |
| counted bytes | the total of point 2, labelled "counted bytes" and written out ("612 counted bytes"), not the size on the wire; the body size next to it when there is a body |
| credentials | "Cookie: yes/no", "Authorization: yes/no" (yes for `Authorization` or `Proxy-Authorization`); values never in the head |
| headers | the number of headers |
| findings | the number, the kinds and whether the scan was incomplete (`findings_truncated`), and "Sending not possible" with `HOLD_004`; the findings bar stands in the head, above the card and above the raw view, at a fixed size the card cannot move or shrink |
| marks | the card's marks, one line each, when the state is `WARNING`, so they stay visible in the raw view |
| view | the switch "Card / Raw", disabled with the stop reason as its tooltip when there is no card |

The decision buttons stand at the same place in both views and look the same;
the card adds no key but the view switch and changes no binding (`A`, `B`,
the release valve, the duration and target grid stay as they are). Opening a request
puts the focus on the head's host line, never on Allow. Screen readers get
the order host line, findings, marks, card sentence, fields as label and value
pairs, remainder, decision; every state is a word, never a colour alone.

### 6. The first two renderers and the RPC

**`web-search`, version 1.** Four engines, five hosts, one fixed path each,
read-only requests only:

| Host | Path | Search parameter | Further known parameters (each at most once) |
|---|---|---|---|
| `www.google.com` | `/search` | `q` | `hl` (`[A-Za-z-]{2,8}`), `num` (1 to 3 digits), `start` (1 to 4 digits), `safe` (`active`, `off`, `images`) |
| `www.bing.com` | `/search` | `q` | `setlang` (`[A-Za-z-]{2,8}`), `count` (1 to 2 digits), `first` (1 to 4 digits) |
| `duckduckgo.com` | `/` | `q` | `kl` (`[a-z-]{2,8}`), `ia` (`web`) |
| `html.duckduckgo.com` | `/html/` | `q` | `kl` |
| `search.brave.com` | `/search` | `q` | `source` (`web`), `offset` (1 to 2 digits) |

`q` must be present exactly once and not empty. Fields: `SEARCH_TEXT` (the
decoded `q`, with its length in characters and in bytes), one
`SEARCH_OPTION` per known parameter, one `SENT_ALONG` per explained header.
An unknown parameter is remainder (and marks `UNKNOWN_PARAMETER`). Redirector
and tracking paths (`/url`, `/imgres`, `/amp/…`, `/complete/search`,
`/ck/a`, `/l/`) never match, because only the paths above do. Other country
domains of an engine (`www.google.de`), bare `google.com`, `kagi.com` (needs a
credential) and search APIs with a key header (`api.search.brave.com`) get no
card in version 1. A search sent by `POST` gets none either.

**`package-download`, version 1.** Three registries, `GET` and `HEAD`
only, no query at all (`QUERY_NOT_ALLOWED`):

| Registry | Host | Paths (anchored) |
|---|---|---|
| npm | `registry.npmjs.org` | metadata `/{name}`, `/@{scope}%2f{name}` (lowercase `%2f` only, as the npm client writes it; `%2F` stops with `PATH_AMBIGUOUS`), `/@{scope}/{name}`; tarball `/{name}/-/{base}-{version}.tgz`, `/@{scope}/{name}/-/{base}-{version}.tgz`, where `{base}` must equal `{name}` |
| PyPI | `pypi.org` | `/simple/{name}/`, `/pypi/{name}/json`, `/pypi/{name}/{version}/json` |
| PyPI | `files.pythonhosted.org` | `/packages/{h2}/{h2}/{h60}/{file}`, `{h2}` two and `{h60}` sixty lowercase hex digits; `{file}` an sdist `{name}-{version}.tar.gz` or `.zip`, or a wheel `{name}-{version}(-{build})?-{py}-{abi}-{platform}.whl` |
| crates.io | `index.crates.io` | `/config.json`; `/1/{name}`, `/2/{name}`, `/3/{c}/{name}`, `/{ab}/{cd}/{name}`, where the prefix must be the one Cargo derives from `{name}` |
| crates.io | `static.crates.io` | `/crates/{name}/{name}-{version}.crate`, `/crates/{name}/{version}/download` |

Grammars, each with a length cap (longer stops with `LIMIT`) and a
`LONG_VALUE` threshold (longer is shown and marked): npm `{name}`
`[a-z0-9._~-]`, not starting with `.` or `_`, cap 214 bytes, mark above 64;
npm `{scope}` `[a-z0-9._~-]`, cap 214 bytes, mark above 64; PyPI `{name}`
`[A-Za-z0-9._-]`, cap 128 bytes, mark above 64 (wheel and sdist file names
with `_`); wheel `{build}` `[0-9][A-Za-z0-9_]*`, cap 32 bytes, mark above
16; wheel `{py}` and `{abi}` `[A-Za-z0-9_.]`, cap 64 bytes, mark above 16;
wheel `{platform}` `[A-Za-z0-9_.]`, cap 256 bytes, mark above 96 (a
`manylinux` tag set runs to about 50); crates.io `{name}`
`[A-Za-z][A-Za-z0-9_-]{0,63}`, the index path lowercase, mark above 48;
`{version}` semantic versioning for npm and crates.io and PEP 440 for PyPI,
cap 64 bytes, mark above 32.
Fields: `REGISTRY` (npm, PyPI or crates.io, from the host table, beside the
host, never instead of it), `REQUEST_KIND` (metadata, index entry or file),
`PACKAGE_SCOPE`, `PACKAGE_NAME` (each character outside `[a-z0-9._-]`
highlighted), `PACKAGE_VERSION`, `PACKAGE_FILE`, `FILE_HASH_PATH` (the
pythonhosted hex path, hex by format, with the fixed note "names the file by
its hash; not checked"), `SENT_ALONG`. Further headers of this renderer:
`npm-command` (`[a-z-]{1,16}`), `npm-session` (16 hex digits),
`npm-auth-type` (`web` or `legacy`), `npm-scope` (empty or an npm scope),
`npm-in-ci` (`true` or `false`), `pacote-version` (semantic version),
`pacote-req-type` (`packument` or `tarball`), `pacote-pkg-id` (at most 256
bytes of printable ASCII), `cargo-protocol` (`version=1`). A publish (`PUT`
to `registry.npmjs.org`, an upload to `upload.pypi.org`, `PUT` to
`crates.io/api/v1/crates/new`) writes and gets no card. "Not in the lockfile"
and "looks like package X" are not shown: both need knowledge from outside
the request.

A renderer's version rises with every change to its tables, fields or marks;
golden fixtures pin each version.

**The RPC.** One new unary call, a minor version of the protocol, and the
capability `render` in `Info.capabilities`:

```proto
// Explains one request that waits for a decision. Decides nothing.
rpc Interpret(FlowRef) returns (Interpretation);

message Interpretation {
  string flow_id = 1;
  bytes request_sha256 = 2;          // the digest of point 8
  HeldHead head = 3;                 // point 5, always set
  RenderState state = 4;
  string renderer_id = 5;            // `web-search`, `package-download`; empty when none matched
  uint32 renderer_version = 6;
  repeated RenderField fields = 7;   // empty unless EXPLAINED, PARTIAL or WARNING
  Coverage coverage = 8;             // always set, also without a card
  repeated RenderMark marks = 9;     // every mark of every field, for the head
  RenderStop stop = 10;              // set exactly when state is NONE
  RenderView default_view = 11;      // point 7
  repeated RawReason raw_because = 12; // why default_view is RAW; empty for CARD
}

message HeldHead {
  string host = 1;                   // A-label, as Authority.host
  string host_unicode = 2;           // U-label; empty when equal to host
  repeated uint32 non_ascii_at = 3;  // character indexes in host_unicode
  uint32 port = 4;
  Scheme scheme = 5;
  Method method = 6;
  string method_raw = 7;
  uint64 total_bytes = 8;            // counted bytes of point 2, not the wire size
  uint64 body_bytes = 9;
  bool cookie_present = 10;
  bool authorization_present = 11;   // Authorization or Proxy-Authorization
  uint32 header_count = 12;
  uint32 finding_count = 13;
  repeated string finding_kinds = 14;
  bool findings_truncated = 15;
  bool send_refused = 16;            // HOLD_004 is set
}

message RenderField {
  FieldLabel label = 1;              // closed; clients take the words from ARB
  string name = 2;                   // the parameter or header name, display form
  string display = 3;
  string display_ascii = 4;
  repeated RenderSpan spans = 5;     // where the bytes of this field stand
  uint32 char_count = 6;
  uint64 byte_count = 7;
  repeated RenderMark marks = 8;
}

message RenderSpan {
  SpanPart part = 1;                 // METHOD, SCHEME, AUTHORITY, TARGET, HEADER_NAME, HEADER_VALUE, BODY
  uint32 header_index = 2;           // for HEADER_NAME and HEADER_VALUE
  uint64 start = 3;                  // byte offset within the part
  uint64 length = 4;
}

message Coverage {
  uint64 total_bytes = 1;
  uint64 head_bytes = 2;
  uint64 explained_bytes = 3;
  uint64 remainder_bytes = 4;
  repeated RemainderSpan remainder = 5;
}

message RemainderSpan {
  RemainderPlace place = 1;          // PATH, QUERY_PARAMETER, HEADER, BODY
  string name = 2;                   // display form of the parameter or header name
  RenderSpan span = 3;
  string display = 4;                // masked for the headers SectionHeaders masks
  string display_ascii = 5;
}

message RenderMark {
  MarkKind kind = 1;                 // the kinds of point 4
  FieldLabel field = 2;
  uint64 measured = 3;               // characters, bits per character times 100, or a count
  uint64 usual = 4;                  // the threshold it was compared with
}

enum RenderState { RENDER_STATE_UNSPECIFIED = 0; RENDER_STATE_NONE = 1;
                   RENDER_STATE_EXPLAINED = 2; RENDER_STATE_PARTIAL = 3; RENDER_STATE_WARNING = 4; }
enum RenderView  { RENDER_VIEW_UNSPECIFIED = 0; RENDER_VIEW_CARD = 1; RENDER_VIEW_RAW = 2;
                   RENDER_VIEW_NONE = 3; } // NONE: decided without opening, for example from a group
enum RenderStop  { RENDER_STOP_UNSPECIFIED = 0; NO_RENDERER = 1; METHOD_WRITES = 2; NOT_HTTPS = 3;
                   HOST_HEADER = 4; UNEXPECTED_BODY = 5; BODY_NOT_BUFFERED = 6; UPGRADE = 7;
                   CREDENTIAL_PRESENT = 8; METHOD_OVERRIDE = 9; PATH_AMBIGUOUS = 10;
                   DUPLICATE_PARAMETER = 11; AMBIGUOUS_ENCODING = 12; INVALID_UTF8 = 13;
                   QUERY_NOT_ALLOWED = 14; LIMIT = 15; REMAINDER_OVER_THRESHOLD = 16;
                   CARDS_OFF = 17; INTERNAL = 18; QUERY_MALFORMED = 19; }
enum RawReason   { RAW_REASON_UNSPECIFIED = 0; NO_CARD = 1; WARNING = 2; FINDINGS = 3;
                   FINDINGS_TRUNCATED = 4; SEND_REFUSED = 5; FIRST_TO_HOST = 6; CARDS_OFF = 7;
                   REMAINDER = 8; }
```

(Enum values carry their type's prefix in the file, as `docs/PROTOCOL.md`
requires; they are shortened above.) `FieldLabel` is a closed enum with
`PATH`, `SEARCH_TEXT`, `SEARCH_OPTION`, `REGISTRY`, `REQUEST_KIND`, `PACKAGE_SCOPE`,
`PACKAGE_NAME`, `PACKAGE_VERSION`, `PACKAGE_FILE`, `FILE_HASH_PATH`,
`SENT_ALONG`. `MarkKind` and `RemainderPlace` hold the values named in points
4 and 2. `Interpret` on a flow that is not held answers with the same
`Diagnostic` as `Decide` on it; with `hold.cards = false` it answers with
`state = NONE`, `stop = CARDS_OFF` and the head. A renderer that panics is
caught in the proxy (`catch_unwind` around the call), and the flow waits with
`stop = INTERNAL` and the warning `RENDER_001` on the flow; the flow is never
held up or allowed by it.

Two existing messages grow: `FlowEvent.Held` gains `bytes request_sha256 =
5`, so a queue row knows the digest without a call, and `DecideRequest`
gains `seen = 9` and `binds_seen = 10` (point 8). The parity table of ADR-0018
gets rows for `Interpret` (`humanitl flows explain FLOW_ID [--json]`, which
prints the head, the card or the stop reason, and the coverage) and for
`seen`.

### 7. The raw view

The raw view is the truth and the card a layer above it. It shows the target
exactly as held on one line, with visible escapes, then today's parameter
table as a reading of it, every header (masked as `SectionHeaders` masks
today, with the length visible), and the body in today's `BodyView`, with
the digest of point 8. When a card exists, the raw view marks the spans it
explained, and pointing at a field of the card highlights its spans in the
raw view.

- **Always one key away.** In the app the switch "Card / Raw" sits in the
  head; `V` toggles it (free in the Decisions view; `R` is the scope popover of `BACKLOG.md` 5). In the
  terminal `v` opens the raw view of the whole request (today it shows only
  the body) and returns to the box. It is never behind a menu and cannot be
  hidden.
- **Raw by default** (`default_view = RAW`, computed by the background service
  with the reasons in `raw_because`): when there is no card; when the state is
  `PARTIAL` (every remainder is marked, point 4) or `WARNING`; when the flow has any finding; when the scan was incomplete
  (`findings_truncated`); when `HOLD_004` stands; when this is the first
  request to this host in the session (`DomainInfo.seen_count == 1`); and
  always with `hold.cards = false`. Otherwise the card is shown first. The
  rule sheet opened from a request always shows the rule in its own form, as
  today, never the card.
- **The setting.** `hold.cards` (boolean, default `true`, tier advanced, in
  the section of the Decisions view of ADR-0025): `false` turns cards off
  entirely, and every request shows the head and the raw view. A project
  profile may set it to `false` and never to `true` (it only tightens,
  ADR-0025 point 6); a company lock may force `false`. Changing it writes
  `config.changed` as every setting does.
- **The view is per request.** Each request opens in its `default_view`; a
  switch by the person holds for that request only, so a warning is never
  hidden by a choice made for the previous one.

### 8. Binding the decision to the bytes

**The digest.** `request_sha256 = SHA-256` over

```text
"humanitl.held.v1" 0x00
lp(method) lp(scheme) lp(host A-label) u16be(port) lp(path_and_query)
u32be(header count) { lp(name) lp(value) } in the order of HeaderMap::iter
u8(body buffered: 1, not buffered: 0) body.sha256 (32 zero bytes when not buffered) u64be(body.size)
```

where `lp(x)` is `u32be(len(x))` followed by `x`. It is computed over the
held `HttpRequest` as point 2 defines it (hop-by-hop headers included, the
daemon's own `host`, `content-length` and `Proxy-Authorization` not), once in
`humanitl_core::http` (`HttpRequest::held_sha256`), before `Held`, from the
`BodyRef.sha256` the proxy already holds; a body that was not buffered binds
only the head.

**The binding.**

```proto
message SeenRequest {
  string flow_id = 1;
  bytes request_sha256 = 2;   // as Interpretation or FlowEvent.Held carried it
  string renderer_id = 3;     // empty for no card
  uint32 renderer_version = 4;
  RenderView view = 5;        // the view that was open when the person decided
}
```

`DecideRequest` gains `repeated SeenRequest seen = 9` and `bool binds_seen =
10`. A client that knows the capability `render` sets `binds_seen` and sends
exactly one `SeenRequest` for each entry of `flow_ids`; a decision on a group
(the batch "Allow 14 → host") sends the digests of its rows with `view =
NONE`. The background service refuses the whole request with `IPC_002`, and
decides nothing, when `binds_seen` is set and `seen` is missing, when any
`SeenRequest` is present and `seen` does not hold exactly one entry per flow
id, or when an entry names a flow not in `flow_ids`. For `allow` and
`allow_edited` it then compares each digest with the stored one before it
applies the decision; a mismatch refuses that flow with the new warning
`IPC_008` ("the request you saw is not the request that waits; open it
again") and leaves it waiting. A `block` is never refused for a digest
mismatch, because blocking cannot let bytes out that the person did not see;
it is applied, and the audit record carries the digest the client sent next
to the stored one. Only a client
without the capability (no `binds_seen`, no `seen`) decides as today, and the
audit record then says `view: unknown`. The binding protects against a stale
view and a client bug; a client on the daemon's socket is the person's own,
and guarantee 2 keeps the agent off that socket.

Before forwarding an allowed request (not `allow_edited`, which sends what
the person wrote), the proxy computes the digest again over the held
`HttpRequest` that `send_via` (`upstream.rs`) hands to `build_outgoing`,
right before that call, never over the wire form; if it differs from the stored one, the flow ends as a block by
the system with the new `BlockReason::RequestChanged` and `PROXY_020`. No code
path changes a held request today, so this check guards a future one; the
agent reads `403` with `reason: not_permitted` and the `detail:` line of
ADR-0026 point 5 (one new row in its table).

**The audit record.** `flow.decided` gains `seen`: `request_sha256` as hex,
`renderer` as `web-search@1` or `null`, `render_state` (`explained`,
`partial`, `warning`, `none`), `stop` (the stop reason or `null`) and `view`
(`card`, `raw`, `none`, `unknown`). It never carries a field, a display
string, a remainder byte or a card sentence; what the person saw can be
rebuilt from the recorded request, the digest and the renderer version. The
digest covers headers and body but is not reversible for any value with
enough entropy; it stands next to the `path_hash` that `flow.received`
already carries.

**The why chain.** `HoldStep` of ADR-0026 gains `request_sha256 = 6`,
`renderer_id = 7`, `renderer_version = 8`, `render_state = 9` and
`view = 10`.

**The agent.** Nothing of this reaches the sandbox: not whether a card was
shown, not its state, not the stop reason. A request with a card waits and is
answered exactly like one without; `humanitl.internal` gets no new path and
no new word.

### 9. Test strategy

In `daemon/crates/render`:

- **Adversarial fixtures**, one file per case under `tests/fixtures/` with the
  request and the expected state, stop reason, fields and coverage, per
  renderer at least:
  hosts `www.googIe.com`, `www.google.com.evil.tld`, `xn--ggle-0nda.com`,
  `google.com`, `www.google.de`, `WWW.GOOGLE.COM.` (matches after
  normalisation), an IP literal, port 8443, `http`; a `Host` header with
  another host, with `user@` and twice; `q=a&q=b`, `q=a;x=b`, `%2526`, `%zz`,
  `%C3%28`, `%C0%AF`, `+` against `%2B`; paths `/search/`, `//search`,
  `/search/../url`, `/search;x`, `/%73earch`, `/SEARCH`, `/url?q=https://…`,
  `/imgres`; `q` with `%E2%80%AE` (`U+202E`), `%E2%80%8B`, `%F3%A0%80%81`
  (a tag character), `%1B%5B31m`, `%0A`, 40 characters of base64, 64 hex
  digits, 129 characters, `https://`; `GET` with a body, with
  `Content-Length: 5`, with `Transfer-Encoding: chunked`; `POST /search`;
  `X-HTTP-Method-Override: POST`; `Cookie`, `Authorization`; `Upgrade:
  websocket`; an unknown parameter of 64 and of 65 bytes (the boundary); an
  unknown header of 20 bytes (`PARTIAL`, `UNKNOWN_HEADER`, raw first);
  `Connection: x-foo` (hop-by-hop, counted, remainder); `a&&q=x`, `q=x&`,
  `/search?`, `q`, `q=x&flag`; `%71=x` (a `%` in a name) and `Q=x`; a
  minimal `GET /search?q=rust` (`EXPLAINED`, remainder 0);
  npm `/lodash/-/evil-1.0.0.tgz`, `/Lodash`, `/%2e%2e/lodash`,
  `/@scope%2Fname`, a scope of 65 bytes, a tarball with `?x=1`, `PUT /lodash`; PyPI
  `/simple/../x/`, a hash path with 59 hex digits, wheels with a `{build}` of
  33 bytes and a `{platform}` of 97 bytes, a wheel whose name differs
  from nothing it can be compared with (shown, not stopped); crates.io
  `/sr/de/serde` (wrong prefix), `/3/s/serde`, `/crates/serde/evil-1.0.0.crate`;
  P1 fixtures with remainder 0 for the npm tarball
  `/lodash/-/lodash-4.17.21.tgz`, the crates index path `/se/rd/serde` and a
  pythonhosted wheel, each asserting that every `TARGET` byte lies in
  `PATH`, `PACKAGE_NAME`, `PACKAGE_VERSION`, `PACKAGE_FILE` or
  `FILE_HASH_PATH`.
- **Property tests** (`proptest`, added once as a workspace dev-dependency by
  item 3.8b): for arbitrary requests, (P1) `head + explained + remainder ==
  total`, spans disjoint and covering every part; (P2) `interpret` gives
  equal results on repeated calls; (P3) changing any one byte of the
  canonical input changes `fields` or `coverage.remainder`, or the state
  becomes `NONE`: no byte is invisible; (P4) any method other than `GET` and
  `HEAD`, any duplicate parameter and any `Cookie` give `NONE`; (P5)
  `held_sha256` changes with any one byte.
- **Fuzzing**: target `render_interpret` in `daemon/fuzz`, asserting P1 and
  no panic.
- **Mutation proofs** each implementing issue names: treating an unknown
  parameter as explained, dropping the mark `UNKNOWN_HEADER`, leaving the
  `PATH` literals out of the coverage, `>` instead of `>=` at the threshold, dropping the
  duplicate check, skipping header coverage, and accepting a host suffix each
  turn a test red.
- **Binding and audit**: an IPC test allows with a wrong digest and expects
  `IPC_008` and a flow that still waits, and blocks with a wrong digest and
  expects the block applied and both digests in `flow.decided`; one with `binds_seen` and no `seen`,
  one with two flow ids and one `SeenRequest`, and one with an entry for a
  third flow each expect `IPC_002` and no decision; a proxy test changes the held request
  before forwarding through a test hook and expects `PROXY_020` and a `403`;
  an audit test reads `seen` back with every `view`.
- **Clients**: the app's golden tests for the head, a card in each state and
  the raw view in light and dark, on Linux (`TargetPlatformVariant`); a
  terminal test that the box stays ASCII and keeps its frame with every
  fixture; screenshot and design check before landing (sprint 9 boundary 4).

### 10. What `docs/SECURITY.md` and `docs/THREAT-MODEL.md` will need

Item 3.8b changes both files in the same commit, with these statements; items
3.8c and 3.8d add the row of their renderer. No guarantee and no statement of
`README.md` changes: the claim "every byte on that socket is an HTTP request
you approved" stays true, because a card changes nothing in what is
forwarded.

`docs/SECURITY.md`:

1. Section 1, "What the claim does not say", a new item: a card is a reading
   aid built only from the request's bytes; it checks nothing, a field it
   explains still leaves the machine when the person allows, and the search
   text or a package name can itself be the payload.
2. Section 6, a new subsection "The head and the card of a request that waits
   for a decision": the head of point 5 for every held request; the
   renderer as a pure function in the background service without network,
   files or outside knowledge; the coverage invariant and the threshold of 64
   bytes; the allowlist and the stop reasons of point 3; the two renderers
   with their hosts.
3. The same subsection: no trust signal (point 4) and the forbidden words; a
   card can only raise suspicion; the raw view is one key away and the default
   in the cases of point 7; `hold.cards` and who may set it.
4. Section 6, the binding: a decision carries the digest of the request it
   was made on and is refused on a mismatch (`IPC_008`); an allowed request
   whose bytes differ from the digest is blocked (`PROXY_020`).
5. Section 7, the item "Internationalised names look like others": the head
   shows the A-label always and the U-label only beside it; a non-ASCII host
   never gets a card.
6. Section 8: `flow.decided` names digest, renderer and version, state, stop
   reason and view, never a field or a card sentence.
7. Section 9: the fixture corpus, the property tests and the fuzz target of
   point 9.
8. Section 11, the changelog entry.

`docs/THREAT-MODEL.md`:

1. Attack surface, a new row K-40 "the card for a request that waits for a
   decision": attacker-controlled bytes turned into statements a person reads
   (interface deception, homographs, invisible characters, parser
   differentials, habituation); mitigations points 2 to 8; status declared;
   tests point 9.
2. K-11 "Queue and rule metadata", extended: the agent learns nothing about
   cards; it can still shape requests so that they get a card, which buys it
   at most a faster decision by the person, never a decision.
3. Residual risks, three entries: a card with a remainder of up to 64 bytes
   carries up to 64 bytes the person may not read; explained fields are
   channels shown in full (the search text, a package name, the 60 hex digits
   of a pythonhosted path, a user agent of up to 512 bytes); and a person who
   has seen many cards reads less carefully, which the raw-view defaults and
   the marks reduce and do not remove.

## Rationale

- **A deterministic function, not a model.** The payload of the request is the
  input; a model asked to describe it can be told by that payload how to
  describe it. A table-driven matcher can be tested byte for byte, a model
  cannot.
- **Coverage as an invariant.** The worst failure of a card is a byte it does
  not show. Counting every byte into head, field or remainder turns "the card
  hides nothing" into a property a test checks on every input, and it makes
  the threshold a number instead of a feeling.
- **64 bytes.** Owner decision E8 asked for a card with a remainder below a
  fixed threshold. 64 bytes keeps one or two stray tracking parameters
  (`ei`, `ved`) on a card and stops a key or a token, which is 20 to 100
  bytes of base64, from hiding in a list the person skims; and a remainder is
  printed, so even within the threshold it is not hidden.
- **Allowlist and fail closed.** A missing card costs a second of reading; a
  wrong card can cost a secret. Every ambiguity (duplicate parameter, double
  encoding, a second `Host`) is exactly where the card and the destination
  could read the request differently, so it gets no card.
- **Read-only requests only.** The decision about a write is about its body,
  and the body is where a renderer would have to parse most and could be
  fooled most; the owner excluded writes (E8).
- **Web search and package download first.** They are the two most frequent
  held requests in the sprint-9 personas, both are `GET` without a body, and
  both have a payload that fits a field: the search text, the package name
  and version.
- **Computed once, at hold time.** The app, the terminal and the audit record
  see the same result, and the head is the same for every client (ADR-0018).
- **The digest.** Without it a decision names a flow id, and a client bug or a
  stale view could let a person allow bytes they did not see. The digest makes
  "what was rendered is what was allowed" checkable in one comparison, and the
  forwarding check keeps it true if a later feature ever rewrites held
  requests.
- **Raw by default where it matters.** Findings, marks, a first contact with a
  host and an incomplete scan are the cases in which a person should read
  every byte; a card first there would be a reassurance.

## Rejected alternatives

- **No card, only a better raw view.** Safer by construction, and it leaves
  the problem the owner named: people allow without reading after many
  decisions. The fixed head of point 5 is this alternative's useful part, and
  it ships first (item 3.8a).
- **A card for every partially understood request.** More cards, and the
  remainder grows until nobody reads it; E8 decided for a threshold.
- **A percentage of explained bytes or a confidence score.** A number that
  reads as a grade and that an attacker would aim for; absolute bytes say the
  same without the grade.
- **A model that summarises the request.** Prompt injection by the payload
  itself; excluded by point 1.
- **Rendering in the clients.** The app already splits queries in Dart; a
  second splitter in the terminal would be a third parser, and the two
  clients could disagree (ADR-0018). The background service renders once.
- **Fuzzy host matching or a provider name instead of the host.** It rewards
  look-alike names (`googIe.com`, `google.com.evil.tld`), the classic
  deception.
- **Rendering writes as warning cards (issue comments, `git push`).** Useful,
  and the body parsing it needs is the riskiest part; excluded by E8 and left
  for a later decision after a measurement.
- **Lockfile and remote comparison ("not in the lockfile").** Useful against
  typosquatting, and it needs the background service to read project files
  for a display; left out of version 1.
- **A random forced raw view against habituation.** Proposed by the experts;
  it makes the view depend on chance, and the defaults of point 7 already
  force the raw view in the risky cases. Kept for after a measurement.
- **Remember the chosen view per session.** Convenient, and a choice made for
  a plain search would hide the marks of the next request.

## Consequences

- A new crate `humanitl-render` (`daemon/crates/render`) with the two
  renderers, `HeldInput`, `Interpretation` and the property tests; a workspace
  member entry, `proptest` as a workspace dev-dependency and a fuzz target,
  all added centrally. `humanitl_core::query` takes the tokenizer out of
  `humanitl-findings`; `HttpRequest::held_sha256` joins `humanitl_core::http`.
  `tools/check_coupling.py` counts stay: the crate sees no engine type.
- `humanitl-proxy` runs the renderer before `Held`, stores digest and result
  with the hold entry, checks the digest before forwarding and gains
  `BlockReason::RequestChanged`.
- The protocol gains `Interpret`, the messages of point 6, `FlowEvent.Held`
  field 5 and `DecideRequest` field 9, a minor version and the capability
  `render`; the parity table gains `humanitl flows explain` and `seen`.
- New diagnostic codes, the next free numbers when the issues land, each with
  title and why in `en` and `de` (ARB and `diagnostic_l10n.dart`): `IPC_008`
  (warning, the seen digest differs), `PROXY_020` (error, the forwarded bytes
  differ), `RENDER_001` (warning on the flow, the renderer failed; raw view).
- The configuration gains `hold.cards` with its catalogue row; the audit kind
  `flow.decided` gains `seen`, and the export and `humanitl audit` show it.
- The app: the head above `RequestCard`'s sections, the A-label first, the
  switch "Card / Raw" with `V`, the card with its states and the remainder;
  the terminal: the head and card lines in the box, `v` as the raw view of the
  whole request, `humanitl flows explain`. ARB keys under `renderCard…` with
  the forbidden-word lint.
- The glossary needs "card" / „Karte", "raw view" / „Rohansicht" and
  "reading aid" / „Lesehilfe" before item 3.8e puts them into the interface
  (item 1.1 carries the glossary).
- Amendments the implementing issues make in the same commits: ADR-0026
  (the row `BlockReason::RequestChanged` in the table of point 5, the fields
  of `HoldStep`), ADR-0018 (parity rows), `backlog/CONVENTIONS.md` 3.2 (the
  status of the new block reason).
- Items: 3.8a the hardened head (`HeldHead`, `Interpret` with `NONE` for
  every flow, the A-label fix, the terminal's raw view with `v`); 3.8b the
  scaffold (crate, coverage, digest, `seen`, forwarding check, audit, why
  chain, `hold.cards`, the codes, the statements of point 10); 3.8c
  `web-search`; 3.8d `package-download`; 3.8e the card, the switch and the
  defaults of point 7 in the app and in the terminal.
- Risk: cards make people allow faster, including the request that matters.
  Mitigation: the raw-view defaults, the marks in the head, findings above
  everything, and the owner walkthrough of item 3.8e with injected cases
  (base64 in a search text, an unknown parameter of 60 bytes, a homograph)
  before release 0.8.0.
- Risk: the allowlists drift from what engines and registries send, and cards
  become rare. That fails towards the raw view; a renderer's next version
  widens its table with new fixtures.
- Doors kept open: Git over HTTPS (read and push as separate cards), a
  structural view of model API calls, documentation fetches, session counters
  ("the 14th search to this host"), a typosquatting list shipped offline, a
  company lock on `hold.cards`, all after a measurement of wrong allows with
  injected cases.

## Affected issues

GitHub #83 (this decision), building on `HUM-025` (decoding and
`findings_truncated`), `HUM-030` and `HUM-154` (the body view), `HUM-031` and
`HUM-094` (domain context and catalogue id), `HUM-049`, `HUM-159` and
`HUM-160` (findings pause, hard block, acknowledged findings), `HUM-057`
(hold limits, `stream: true`), `HUM-067` (`--ask terminal`), `HUM-286` (the
pane header), ADR-0005, ADR-0007, ADR-0014, ADR-0018, ADR-0025, ADR-0026 and
ADR-0027. Implemented in phase 3 of sprint 9 as item 3.8 in five issues
(3.8a to 3.8e, see Consequences), after items 3.1 and 3.2, whose card for
waiting requests in the app and the terminal the head and the card extend.
