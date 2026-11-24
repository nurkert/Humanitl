# Design: tools on request

Applies to: Humanitl 0.1

Status: accepted with [ADR-0029](../adr/0029-tools-on-request-off-by-default.md) on 2026-11-24
(GitHub #84). The ADR holds the decisions and wins where the two disagree; this document holds
the detail the five implementing items of sprint 9 phase 5 build from: the catalog format, the
messages at field level, the sealing checks, the card and the threat table.

Nothing described here exists yet. Until item 5.5 lands, [`SECURITY.md`](../SECURITY.md) and
[`THREAT-MODEL.md`](../THREAT-MODEL.md) describe Humanitl without tools on request.

## 1. The problem in one paragraph

An agent in a Humanitl sandbox often needs a program the sandbox does not have. Letting it
install one is dangerous in two ways that cutting its network does not touch: the request itself
carries data (a package name, a version, an index address, free text), and an installer that has
network while it runs foreign code can send anything it reads. The design separates three things:
the agent only asks, from a closed list; network and foreign code never meet; the result is an
immutable, read-only layer that runs with exactly the agent's rights.

## 2. Parts and where they live

| Part | What it does | Where |
|---|---|---|
| Switch | `tools.enabled`, per project, default `false` | `humanitl-config`, project choice in the register |
| Request path | `GET /tools`, `GET` and `POST /tools/<name>` on `humanitl.internal` | `humanitl-proxy` (`meta.rs`) |
| `humanitl-need` | the shim binary under a second name, a thin client of the request path | `daemon/bin/humanitl-shim` |
| `ToolDesk` | the seam from the proxy to the broker: a concrete struct holding one closure, no trait | `humanitl-core` (`tools` module) |
| Broker | quotas, plans, cards, approvals, answers | `humanitl-tools` (new), wired in `humanitld` |
| Catalog | signed TOML file, rollback state | `humanitl-tools`; source `catalog/tools.toml` |
| Fetcher | downloads catalog and artifacts from fixed hosts | `humanitl-proxy`, next to `self_update.rs` |
| Extractor | `humanitl-unpack`, a static binary in a sandbox without a door | `daemon/bin/humanitl-unpack` (new) |
| Sealer | checks the extractor's output and builds the layer | `humanitl-tools`, sharing the tree checks with the depot |
| Store | blobs and layers under `$XDG_DATA_HOME/humanitl/tools/` | `humanitl-tools` |
| Launcher | binds layers, the link directory and `PATH`; collision check | `humanitl-sandbox` |
| Card, Tools page | the person's decision and own choice | app, TUI, `humanitl tools` |

Layout on the host, all under `$XDG_DATA_HOME/humanitl/tools/` (mode `0700`):

```text
catalog/tools.toml          the accepted catalog, exact bytes
catalog/tools.toml.sig      its signature, base64
catalog/state.json          serial, key_id, sha256, revoked (union), newest signed_at (section 6.3)
blobs/<sha256>              downloaded artifacts, 0444, kept only after the hash matched
staging/<random>/           the extractor's /out, then the sealer's new tree
store/<layer-digest>/       sealed layers, directories 0555, files 0444 or 0555
store/<layer-digest>.json   the seal manifest (section 7.3)
sessions/<id>/bin/          the link directory of one session (section 7.4), removed at its end
```

Approvals live in the register database (table `tool_approvals`, section 8.4).

## 3. The ideal path

**App.** The person opens the project's settings, section "Isolation and reach", and turns on
"Tools on request" (origin "This project"). A page "Tools" appears for the project with the
catalog: one row per entry with version, size and whether it is downloaded. The person may pick
entries there; each pick shows the card of section 8 and saves an approval for the project. When
the agent asks for an entry nobody picked, the card appears in the Decisions view next to the
requests that wait for a decision, with "Don't install" focused. After an approval the session's
status says "1 tool from the next start", and the session sheet offers "Restart with tools".

**Terminal.**

```console
$ humanitl config set tools.enabled true --level project
tools.enabled = true  (This project: /home/me/src/app)
$ humanitl tools
catalog 7, signed 2026-12-01, expires 2027-05-30
ENTRY      VERSION  SIZE     HERE  APPROVED
jq         1.7.1    2.2 MB   yes   project
ripgrep    14.1.1   2.1 MB   no    -
…
$ humanitl tools requests
1 request  session 0193…  agent claude  "The agent asks for a tool: ripgrep 14.1.1"
$ humanitl tools decide 0193d7a8-… --for project
<the card of section 8 in text>
Install ripgrep 14.1.1 for this project? [y/N] y
downloading 2.1 MB from github.com … sealed sha256:4be1…  available from the next session start
```

In the TUI the card sits in the Decisions view with the same fields; its keys are fixed by item
5.4. Every surface reads the same RPC (ADR-0018) and decides nothing itself.

**In the sandbox.**

```console
$ humanitl-need ripgrep
tool: ripgrep
status: pending
detail: the request was passed on; ask again later with GET, do not repeat it
$ humanitl-need --status ripgrep
tool: ripgrep
status: ready
path: /opt/humanitl/tools/bin/rg
detail: available from the next session start
```

## 4. Step by step

1. **Ask.** The agent sends `POST /tools/ripgrep` over the proxy port. The proxy recognises the
   meta host, checks method and path, and calls `ToolDesk::ask` with the session it already knows
   for the connection.
2. **Check from memory.** The broker checks the switch, the platform, the name pattern, the
   catalog (accepted, not expired, entry present, not revoked), the session's quotas and existing
   approvals. Nothing here touches disk or network. A failure answers `unavailable` (or `denied`
   after the third refusal) at once.
3. **Plan.** The broker builds the plan: the closure of `requires`, every artifact for the
   platform, layer digests, exports, sizes, whether each blob is in `blobs/`, the collisions it
   can already see, and the plan digest. It records `tool.requested`, emits
   `FlowEvent.ToolRequested` and answers `pending`.
4. **Decide.** A person answers the card (section 8). The decision carries the plan digest.
5. **Fetch, only if needed and only now.** Artifacts that are not in `blobs/` are downloaded by
   the fetcher (section 7.1). `tool.fetched` per artifact. This is a deliberate narrowing of "a
   request causes no network traffic": the request causes none, the person's allow may cause
   this one download of exactly what the card listed.
6. **Unpack.** One throwaway sandbox per artifact (section 7.2). `tool.unpacked`.
7. **Seal.** The sealer checks and copies the output into `store/<layer-digest>/` and compares
   the digest with the catalog (section 7.3). `tool.sealed`.
8. **Store the approval** in `tool_approvals` with scope, `tool.decided`, and the answer for the
   agent becomes `ready` with `when` next session.
9. **Mount at the next start.** The launcher re-hashes each approved layer, checks collisions and
   binds it (section 7.4). `tool.mounted`. The agent then reads `ready`, "available in this
   session".
10. **Remove.** `humanitl tools remove ENTRY --project DIR` deletes the approval; `humanitl tools
    gc` deletes blobs and layers no approval references. A revocation in a newer catalog voids
    approvals at once (`tool.revoked`).

A failure in steps 5 to 7 leaves no approval, gives the person a `TOOL_00x` diagnostic on the
card, and the agent `unavailable`.

## 5. The request path at field level

### 5.1 HTTP on `humanitl.internal`

The three paths join the table of ADR-0014 point 3. Other methods on them answer `405`, other
paths under `/tools/` `404`, as today.

| Request | Body | Limits |
|---|---|---|
| `GET /tools` | none | a counter of its own: 30 `GET` per minute per session, not shared with `/ask` |
| `GET /tools/<name>` | none | same counter |
| `POST /tools/<name>` | optional, `text/plain`, UTF-8, at most 1 KiB before cleaning | the quotas of section 5.3 |

`<name>` is the last path segment, taken before any percent decoding; a `%`, a second segment or
a query makes it fail the pattern `^[a-z0-9][a-z0-9-]{0,31}$`.

The answer to `POST` and `GET /tools/<name>` is always `200 OK`, `Content-Type: text/plain;
charset=us-ascii`, `Cache-Control: no-store`, and this body, each line ending in `\n`:

```text
tool: <name>                      only when <name> matches the pattern
status: pending|ready|denied|unavailable
path: /opt/humanitl/tools/bin/<export>   only with ready; the first export
detail: <fixed sentence>
```

| `status:` | `detail:` |
|---|---|
| `pending` | `the request was passed on; ask again later with GET, do not repeat it` |
| `ready`, mounted | `available in this session` |
| `ready`, not mounted | `available from the next session start` |
| `denied` | `not permitted here; do not ask for this tool again in this session` |
| `unavailable` | `not available here; do not ask for this tool again in this session` |

A `GET /tools/<name>` for an entry the session never asked for answers `unavailable` unless an
approval exists (then `ready`). Over the `GET` limit the answer stays in the vocabulary: `200`,
`status: unavailable` with its fixed `detail:`, and `GET /tools` answers the single line
`tools: unavailable`, which does not claim that tools are off. The
proxy never answers `429` on these paths; `take_ask_slot` and its `429` stay for `/ask` alone.

`denied` reads the same whether a person, an expired card or the stop after three refusals
caused it; no answer names a person (HUM-364).

`GET /tools` answers `200 OK` with one header line and one line per entry of the catalog for the
platform, sorted by id:

```text
tools: on
jq 1.7.1 ready "Command-line JSON processor"
ripgrep 14.1.1 pending "Recursive search in files"
uv 0.5.11 - "Python package and project manager"
```

The state column is `ready`, `pending`, `denied` or `-`. Summaries are ASCII, at most 60
characters, without `"`, checked when the catalog is accepted. When tools are off, when the
platform is not Linux, or when no catalog is accepted, the body is the single line `tools: off`.

**Accepted leaks.** `GET /tools` tells the agent which entries the person approved for the
project, and `unavailable` against `pending` for a known entry tells it one bit: whether tools
are usable here at all. Both describe the agent's own environment and are what it needs to stop
asking; neither says anything about a person or about the content of a request.

### 5.2 Inside the background service

```rust
// humanitl_core::tools
pub struct ToolName(String);                   // validated: ^[a-z0-9][a-z0-9-]{0,31}$
pub struct ToolRequestId(Uuid);                // UUIDv7

pub struct ToolAsk {
    pub id: ToolRequestId,                     // minted by the proxy
    pub session: SessionId,                    // from the connection, never from the request
    pub flow: FlowId,                          // the meta flow, for the history
    pub name: Result<ToolName, RawNameDigest>, // an invalid name only as SHA-256 and length
    pub reason: Option<String>,                // already cleaned and cut, findings masked
    pub at: Timestamp,
}

pub struct RawNameDigest { pub sha256: [u8; 32], pub len: u16 }

pub enum ToolState { Pending, Ready { mounted: bool, export: String }, Denied, Unavailable }

pub struct ToolAnswer { pub name: Option<ToolName>, pub state: ToolState }

pub struct ToolListing { pub enabled: bool, pub rows: Vec<ToolRow> }
pub struct ToolRow { pub id: ToolName, pub version: String, pub summary: String, pub state: Option<ToolState> }

pub enum ToolCall { Ask(ToolAsk), State(SessionId, ToolName), List(SessionId) }
pub enum ToolReply { Answer(ToolAnswer), Listing(ToolListing) }

/// The seam from the proxy to the broker. A concrete struct like `SandboxNoticeSink` and
/// `InterceptPause`, not a trait: nothing is meant to be swapped.
#[derive(Clone)]
pub struct ToolDesk {
    serve: Arc<dyn Fn(ToolCall) -> ToolReply + Send + Sync>,
}

impl ToolDesk {
    /// What `humanitld` builds, with the broker behind the closure.
    pub fn serving(serve: impl Fn(ToolCall) -> ToolReply + Send + Sync + 'static) -> Self;
    /// Tests and the fake mode: every path answers `tools: off` or `unavailable`.
    pub fn off() -> Self;
    pub fn ask(&self, ask: ToolAsk) -> ToolAnswer;
    pub fn state(&self, session: SessionId, name: &ToolName) -> ToolAnswer;
    pub fn list(&self, session: SessionId) -> ToolListing;
}
```

The proxy holds a `ToolDesk` given by `humanitld`, as it holds the pause and the sandbox notice
sink; without one it holds `ToolDesk::off()`. The closure takes no lock that IO holds: the
broker keeps the accepted catalog, the quotas, the approvals of the project and the set of
digests present in `blobs/` in memory, and refreshes them when a sync, a decision, `gc` or a
session start changes them. Building a plan therefore touches no disk, and a known and an unknown
name take the same code path; no stronger timing claim is made.

**What is stored as text.** The meta flow of a tool request is recorded like `/ask` (HUM-103)
with one exception: for a path under `/tools` the recorder keeps the path and the body only as
SHA-256 and length. `tool.requested` in the audit log keeps the same hashes. The cleaned, masked
reason exists as text only in the broker's memory and on the card until it is answered.

### 5.3 Quotas

| Constant | Value | Effect when reached |
|---|---|---|
| `MAX_OPEN` | 3 | a fourth `POST` while three are `pending` answers `unavailable` |
| `MAX_PER_SESSION` | 10 | every further `POST` answers `unavailable`; unknown names count |
| `DENIALS_BEFORE_STOP` | 3 | after three "Don't install" in a session every further `POST` answers `denied` without a card |
| `REASON_MAX_CHARS` | 200 | the rest is cut, the card says "cut" |

A repeated `POST` for an entry that is already `pending`, `ready` or `denied` in the session
answers its state and counts against `MAX_PER_SESSION` but creates no second card.

## 6. The catalog

### 6.1 Schema, version 1

```toml
format = 1
serial = 7                          # strictly increasing, never reused
signed_at = 2026-12-01              # TOML local date, read as a UTC calendar date
expires = 2027-05-30                # UTC calendar date, at most 180 days after signed_at
key_id = "tools-2026"               # one of CATALOG_KEYS
revoked = [                         # artifact sha256 or layer digests, "sha256:<hex>"
  "sha256:9f0c…",
]

[[entry]]
id = "ripgrep"                      # ^[a-z0-9][a-z0-9-]{0,31}$, unique
version = "14.1.1"                  # display; the artifact pins the bytes
summary = "Recursive search in files"   # ASCII, <= 60 characters, no '"'
license = "MIT OR Unlicense"        # SPDX expression, display only
homepage = "https://github.com/BurntSushi/ripgrep"   # display only, never fetched
released = 2024-09-08               # at least 14 days before signed_at
exports = { rg = "bin/rg" }         # PATH name -> path inside the layer, at most 16
requires = []                       # other entry ids; the closure is acyclic
runtime_network = "none"            # "none" or "proxy"
runtime_hosts = []                  # with "proxy": the hosts it is known to contact
needs_glibc = ""                    # "" for static builds, else the minimum, e.g. "2.28"
smoke = "--version"                 # arguments for the admission smoke run of every export

[[entry.artifact]]
platform = "linux-x86_64"           # or "linux-aarch64"
url = "https://github.com/BurntSushi/ripgrep/releases/download/14.1.1/ripgrep-14.1.1-x86_64-unknown-linux-musl.tar.gz"
sha256 = "<64 hex>"
size = 2566310                      # bytes of the download, exact
format = "tar.gz"                   # "raw", "tar.gz" or "tar.xz"
strip_components = 1                # leading directories removed by the extractor
raw_path = ""                       # with "raw": where the file goes, e.g. "bin/jq"
unpacked_size = 6815744             # upper bound of the layer in bytes
layer_digest = "sha256:<64 hex>"    # what the sealer must produce
```

Checks when a catalog is accepted, after the signature: every field present with its type;
`format = 1`; ids unique and valid; every `requires` names an entry and the graph has no cycle;
every export name matches the id pattern plus `.` and `_`, and no two entries of one closure share
an export; every export path is relative, without `..`, and names a file the manifest of the
layer will hold (checked at seal time); every URL is `https`, on the host list of section 7.1,
without port, user information, query or fragment; sizes are positive and `unpacked_size` at
most 2 GiB; `expires` after `signed_at` and at most 180 days later. One failure refuses the whole
catalog with `TOOL_001`, whose `why` names the step (signature, or the field that failed), and
the accepted catalog stays as it was.

### 6.2 Signature and keys

- **Scheme.** Ed25519 (RFC 8032) over the exact bytes of `tools.toml`, the scheme the
  self-update uses. Signing: `openssl pkeyutl -sign -rawin -inkey <key> -in tools.toml` in
  `packaging/release/sign-tools-catalog.sh`, which first derives the public key from the secret
  and refuses to sign when it is not one of `CATALOG_KEYS`. Verifying: as the self-update does
  (`daemon/crates/proxy/src/self_update.rs`), through `rustls::crypto::ring`'s
  `verify_signature` for `SignatureScheme::ED25519`, before the file is parsed. `tools.toml.sig` holds the 64 bytes in standard base64 and a final
  newline, nothing else.
- **Which key.** The verifier tries each of the at most two compiled-in keys on the unparsed
  bytes. Only after one of them verified is the file parsed, and its `key_id` must name that key;
  otherwise the catalog is refused with `TOOL_001`.
- **Custody.** The private key is the repository secret `HUMANITL_TOOLS_CATALOG_SIGNING_KEY`,
  used only by the release workflow on tag builds, like `HUMANITL_UPDATE_SIGNING_KEY`. Whoever
  can use the update key can already replace the daemon; the catalog key therefore adds no new
  root of trust, and a separate key keeps the two rotatable apart.
- **Rotation.** Release N adds the next key to `CATALOG_KEYS`; a catalog after N switches
  `key_id`; release N+1 removes the old key. A machine that skipped release N still verifies the
  old catalog and refuses the new one with `TOOL_001`, whose `fix` is to update Humanitl.
- **Compromise.** A release drops the key at once, and the first catalog under the new key revokes
  every digest the old one signed since the suspected time. Until a machine updates, it trusts the
  old key; a stolen key still needs a person's approval on a card for anything to run.

### 6.3 Rollback, expiry, revocation, distribution

- **The release constants.** Every release compiles in `humanitl_tools::MIN_SERIAL`, the serial
  of the catalog it bundles, and `humanitl_tools::REVOKED`, the union of every revoked digest up
  to that catalog.
- **The serial window.** A catalog is accepted only with `MIN_SERIAL <= serial <= MIN_SERIAL +
  50` and not below the serial in the state, bundled or downloaded, before and after a state
  reset. 50 leaves room for fifty catalogs published while a machine runs an older Humanitl; above
  the window `TOOL_002` says to update Humanitl. The catalog workflow allows only the previous
  serial plus one. A stolen key can sign at most `MIN_SERIAL + 50` and so block legitimate
  catalogs below that serial on machines that accepted it, until the next release drops the key
  and sets `MIN_SERIAL` above it; it cannot set a serial near 2^63 or outlast a release.
- **The state.** `state.json` holds the highest accepted `serial`, the `key_id` that signed it,
  the SHA-256 of that file, the union of every `revoked` digest ever accepted, and the newest
  `signed_at` of an accepted catalog. A lower serial is refused with `TOOL_002`; the same serial
  with other bytes is refused with `TOOL_002` and recorded in `tool.catalog` as `conflict`, a
  sign of key misuse. A missing or unreadable state is rebuilt only from a catalog in the window
  and at or above the bundled serial, recorded as `state_reset`; its revocation union starts from
  `REVOKED` plus that catalog's list. A change of `key_id` is accepted only to another key of
  `CATALOG_KEYS` and recorded.
- **Revocation is cumulative.** The check uses `REVOKED` plus the union in the state. A later
  catalog that omits a digest does not revive it; a catalog whose entries or artifacts name a
  digest of that set is refused as a whole (`TOOL_011`). Release policy keeps every revocation in
  every later catalog. An accepted revocation voids each approval whose plan names a listed
  digest: `tool.revoked`, `TOOL_011` on the Tools page and at the next start, the layer is not
  mounted, and `humanitl tools gc` may delete it.
- **Expiry and the clock.** `signed_at` and `expires` are calendar dates in UTC; the clock is the
  system's wall clock read as a UTC date. The date in force is the later of the clock and the
  newest `signed_at` in the state. The state takes a date only from that validated source, never
  from the wall clock, so a wrong clock never stores a wrong date: a forward jump expires the
  catalog early only while the clock is wrong. Turning the clock back cannot move the date in
  force below the newest signed date, but it can keep an old catalog usable up to its `expires`;
  the person controls the clock, and this is a residual risk (section 10). After `expires` the
  broker answers `unavailable` to every request and offers no card and no approval; layers with
  an approval keep mounting and every start carries `TOOL_003`.
- Every release carries `tools.toml` and `tools.toml.sig` as release files and in the packages
  under `/usr/share/humanitl/`. At start the background service checks the bundled pair like a
  downloaded one and keeps the newer serial. `humanitl tools sync` asks the latest-release
  endpoint the update check uses, fetches the two files from
  `https://github.com/nurkert/Humanitl/releases/download/v<version>/`, then the artifacts of
  every approved entry that is not in `blobs/`.
- `tools.auto_sync = true` (level "Mine", default `false`) runs the same sync once a day at a
  random minute, independent of any request.

### 6.4 The first catalog

Twenty-five entries, Linux on x86-64 and AArch64. Each comes as a self-contained archive or file
from the project's own release host; a static or musl build where the project publishes one,
otherwise a glibc build with `needs_glibc`. Versions, hashes and digests are pinned by item 5.1.

| Entry | Exports | Source host | Format | Network at run time |
|---|---|---|---|---|
| `jq` | `jq` | github.com | raw | none |
| `yq` | `yq` | github.com | raw | none |
| `ripgrep` | `rg` | github.com | tar.gz | none |
| `fd` | `fd` | github.com | tar.gz | none |
| `bat` | `bat` | github.com | tar.gz | none |
| `delta` | `delta` | github.com | tar.gz | none |
| `difftastic` | `difft` | github.com | tar.gz | none |
| `hyperfine` | `hyperfine` | github.com | tar.gz | none |
| `sd` | `sd` | github.com | tar.gz | none |
| `just` | `just` | github.com | tar.gz | none |
| `shellcheck` | `shellcheck` | github.com | tar.xz | none |
| `shfmt` | `shfmt` | github.com | raw | none |
| `hadolint` | `hadolint` | github.com | raw | none |
| `actionlint` | `actionlint` | github.com | tar.gz | none |
| `ruff` | `ruff` | github.com | tar.gz | none |
| `uv` | `uv`, `uvx` | github.com | tar.gz | proxy: `pypi.org`, `files.pythonhosted.org`, `github.com` |
| `python` | `python3`, `pip3` | github.com (relocatable standalone build) | tar.gz | proxy: `pypi.org`, `files.pythonhosted.org` |
| `node` | `node`, `npm`, `npx` | nodejs.org | tar.xz | proxy: `registry.npmjs.org` |
| `go` | `go`, `gofmt` | dl.google.com | tar.gz | proxy: `proxy.golang.org`, `sum.golang.org` |
| `zig` | `zig` (also `zig cc`, `zig c++`) | ziglang.org | tar.xz | none |
| `cmake` | `cmake`, `ctest` | github.com | tar.gz | none |
| `pandoc` | `pandoc` | github.com | tar.gz | none |
| `gh` | `gh` | github.com | tar.gz | proxy: `api.github.com`, `github.com` |
| `xh` | `xh` | github.com | tar.gz | proxy: the hosts the agent names |
| `golangci-lint` | `golangci-lint` | github.com | tar.gz | none |

GitHub release downloads redirect to `objects.githubusercontent.com` or
`release-assets.githubusercontent.com`; both are on the fetcher's list. "Network at run time"
is shown on the card; every such request is still held or ruled by the proxy like any request of
the agent. Distribution packages such as `make` or `gcc`, and tools that ship only as `zip`
(`ninja`, `deno`, `protoc`), are not in the first version (section 12).

### 6.5 Admission to the catalog

A pull request that changes `catalog/tools.toml` runs `.github/workflows/tools-catalog.yml`:

1. download every artifact from its URL, check size and SHA-256;
2. run `humanitl-unpack` and the sealer exactly as the background service does, and compare the
   digest with `layer_digest`;
3. check that every export path exists in the sealed layer and is executable, and run every
   export once with the entry's `smoke` arguments on the sealed layer, with the hooks of
   section 7.3 rule 7 dropped, in a sandbox without network;
4. check `released` against the cooldown of 14 days, that no digest of the cumulative revocation
   list is still used, and that `serial` is the previous one plus one;
5. check exports against a fixed list of common system commands (`ls`, `git`, `ssh`, `curl`,
   `python`, `sh` and the rest of the list in `catalog/tools-reserved.txt`).

The offline part (schema, ids, hosts, cycles, exports, dates) runs in `make check`. The file in
the repository is unsigned; only the release pipeline signs, on a tag build, the exact bytes that
passed review.

## 7. Fetch, unpack, seal, mount

### 7.1 The fetcher

- Code in `humanitl-proxy`, beside `self_update.rs`, using the same client: rustls with the
  bundled roots, egress `Direct` (ADR-0017), never the proxy the agent uses and never the
  decision proxy.
- Hosts, compiled in: `github.com`, `objects.githubusercontent.com`,
  `release-assets.githubusercontent.com`, `nodejs.org`, `dl.google.com`, `ziglang.org`. Port 443
  only. Redirects: at most three, each target `https` on this list, otherwise `TOOL_005`.
- Request: `GET`, headers `Host`, `Accept: */*`, `User-Agent: Humanitl/<version>`; no cookies,
  no credentials, no query, no `Range`.
- Response: read at most the catalog's `size` bytes plus one; more, fewer or another SHA-256 is
  `TOOL_004`. The bytes go to `blobs/.part-<random>`, are renamed to `blobs/<sha256>` only after
  the match, and set `0444`.
- Timeouts: 30 seconds to connect and to the first byte, 10 minutes per artifact. One download at
  a time.

### 7.2 The unpack sandbox

| Property | Value |
|---|---|
| Built by | the configured sandbox backend at the project's isolation level, through `plan` and `launch` |
| Profile | `profiles/sandbox/tool-unpack.toml`: no bridges, no proxy socket, no `[env]` proxy kit |
| Network | none: no interface but loopback, no socket into the sandbox, no resolver configuration |
| Inputs | `/in/artifact` read-only (the blob, bound by descriptor where the backend can); `/usr/local/bin/humanitl-unpack` read-only |
| Output | `/out`, an empty directory under `staging/`, writable |
| Not present | project folder, home with data, depot, CA, daemon socket, session environment, other layers |
| Limits | `/tmp` 64 MiB, wall time 120 s, `RLIMIT_FSIZE` at 512 MiB, `RLIMIT_NPROC` small, seccomp filter of the default profile |
| Command | `humanitl-unpack --format <f> --strip <n> [--raw-path <p>] --max-bytes <unpacked_size> --max-entries 100000 /in/artifact /out` |
| Result | exit code; stderr kept for the person and hashed into `tool.unpacked`, never shown to the agent |

`humanitl-unpack` is a static binary built with the shim's release profile, depending on `tar`,
`flate2` and `lzma-rs` (pure Rust). It never executes, `chown`s or `mknod`s anything. Per entry
of the archive it refuses: absolute paths; `..` components; names with bytes outside printable
ASCII; hard links; character and block devices; FIFOs; symbolic links whose target is absolute or
leaves `/out` lexically; set-uid, set-gid and sticky bits; and it stops when the bytes written or
the entries exceed the limits, or when the decompressed size exceeds 200 times the compressed
size. A refusal is exit code 2 with one line naming the entry. Writes use `openat` relative to the
`/out` descriptor with `O_NOFOLLOW`; directories are created, never followed through a link.

### 7.3 Sealing rules

The sealer runs in the background service on the host, after the unpack sandbox is gone, and
treats `staging/<random>/out` as hostile. It walks it with `openat2(RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)` and `O_NOFOLLOW`, reads every entry's type with
`fstatat(AT_SYMLINK_NOFOLLOW)`, and writes a new tree beside it. It reuses the tree checks of
`depot/adopt.rs`, moved into one module both use.

| # | Rule | On violation |
|---|---|---|
| 1 | Only regular files, directories, symbolic links | fail, `TOOL_007` |
| 2 | A link target is relative and, resolved lexically from the link's directory, stays inside the layer | fail |
| 3 | No file has more than one link in `/out` (`st_nlink == 1`) | fail |
| 4 | No set-uid, set-gid or sticky bit | fail |
| 5 | Names: printable ASCII without `/`, not `.` or `..`, at most 255 bytes; depth at most 32 | fail |
| 6 | At most 100,000 entries; one file at most 512 MiB; the total at most `unpacked_size` and 2 GiB | fail |
| 7 | Startup hooks are dropped and listed under `dropped` in the manifest: any `*.pth`, `sitecustomize.py`, `usercustomize.py`; any entry below a directory named `etc`, `profile.d`, `autostart`, `systemd` or `cron.*`; files named `ld.so.preload`, `ld.so.conf` or `ld.so.conf.d`; `.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.npmrc`, `npmrc`, `pip.conf`, `pyvenv.cfg`, `.gitconfig`. This covers configuration a runtime reads relative to its own prefix, such as `lib/node_modules/npm/npmrc` or `pyvenv.cfg`; the smoke run of section 6.5 shows before admission whether an entry needs one | drop |
| 8 | Content is copied byte for byte into new files; mode `0555` when the source had any execute bit, else `0444`; directories `0555`; no owner, extended attribute, ACL or file capability is copied | by construction |
| 9 | Every export path names a regular file with mode `0555` in the new tree | fail |
| 10 | The manifest digest equals the catalog's `layer_digest` | fail, `TOOL_007` with "digest" |

The **manifest** is a JSON file with `entry`, `version`, `platform`, `artifact_sha256`,
`catalog_serial`, `dropped` (paths), and `files`, one object per entry sorted by the bytes of its
path: `{"path", "type": "f"|"d"|"l", "mode", "size", "sha256"}` for files, `{"path", "type",
"mode"}` for directories, `{"path", "type", "target"}` for links. The **layer digest** is the
SHA-256 of the canonical text of `files` only: one line per entry, `type SP mode-octal SP size SP
sha256-or-target SP path LF`, sorted as above. The same function runs in the catalog workflow.

On success the new tree is renamed to `store/<layer-digest>/`, the manifest written beside it,
`staging/<random>/` deleted. A layer that exists already is not written again.

### 7.4 Mounting at session start

For each approval of the project whose plan is not revoked (and, for the scope "next session",
not yet used):

1. Open `store/<layer-digest>/` once and hash every file through that descriptor (`openat2`
   beneath it, no link followed) against the manifest; a mismatch leaves the layer out with
   `TOOL_009`. The same descriptor is what step 5 binds. The cost is about one second per 500 MB on a current SSD; the
   largest entry of the first catalog (`go`) is about 250 MB unpacked.
2. Collect the exports of all approved layers; two layers exporting one name are both left out
   with `TOOL_008`.
3. Compare every export with: the regular files with an execute bit in every `PATH` directory of
   the planned sandbox, read from the host sources of the plan's read-only mounts; the agent
   program's name; `humanitl-shim` and `humanitl-need`; the shell builtins and keywords of POSIX
   `sh` and `bash`. A collision leaves that tool out with `TOOL_008`. Collisions depend on the host
   (a host `python3`, `node`, `go`, `gh` or `cmake` in a bound directory): the broker runs the
   same check when it builds the plan, so the card already shows them and offers no approval for
   a colliding entry; this step catches a host that changed since.
4. Create `$XDG_DATA_HOME/humanitl/tools/sessions/<id>/bin/` with one relative link per export
   (`rg -> ../ripgrep/bin/rg`), mode `0555`. It lies under the data directory, never under the
   runtime directory, which no sandbox may see (`docs/SECURITY.md` section 3); it is removed when
   the session ends.
5. Add to `LaunchOptions::session_ro`: the descriptor of step 1 as `/opt/humanitl/tools/<entry>`
   per layer, and the link directory of step 4 as `/opt/humanitl/tools/bin`. Append `:/opt/humanitl/tools/bin` to `PATH`. Bind
   `humanitl-need` at `/usr/local/bin/humanitl-need`.
6. The shim's start report lists every mount below `/opt/humanitl/tools` from
   `/proc/self/mountinfo`; one without `ro`, `nosuid` or `nodev` fails the start with `TOOL_009`.
7. `tool.mounted` per layer; the approval's `last_mount` is set; a "next session" approval is
   marked used.

A session with no approved tool binds nothing of this except `humanitl-need`, and only when tools
are on.

## 8. The card

### 8.1 Fields, in order

| # | Field | Example | Source |
|---|---|---|---|
| 1 | Head | "The agent asks for a tool" | fixed |
| 2 | Context | project `app`, agent `claude`, session started 10:42, "request 2 of 10 in this session" | session, quotas |
| 3 | Entry | `ripgrep` 14.1.1, "Recursive search in files", MIT OR Unlicense | catalog |
| 4 | Origin | catalog 7, signed 2026-12-01 with `tools-2026`; downloads from `github.com` | catalog, A-labels |
| 5 | Hashes | artifact `sha256:4be1…` (full on demand, mono), layer `sha256:9a07…` | catalog |
| 6 | Size | 2.1 MB download, 6.5 MB unpacked | catalog |
| 7 | Closure | "no further entries", or one row per required entry with its own fields 3 to 6 | plan |
| 8 | Here or not | "already downloaded: 0 bytes over the network", or "allowing downloads 2.1 MB from github.com" | `blobs/` |
| 9 | Commands | `rg`, added last on `PATH`; a collision in orange with the program it would meet, and then allow is disabled | plan |
| 10 | Released | 2024-09-08 | catalog |
| 11 | Network at run time | "none expected", or the hosts, with "each request still waits for a decision or a rule" | catalog |
| 12 | Fixed sentence | "The tool runs with the agent's rights and sees what the agent sees. A hash proves where it came from, not that it is harmless." | fixed |
| 13 | When | "available from the next session start" | fixed in this version |
| 14 | Reason | small, grey: "Reason from the agent, not checked:" and the text, plain, never a link | request |

No field is a trust signal: no colour or word says "verified", "safe" or "trusted"; the
forbidden-word lint of ADR-0028 applies to the card's ARB keys.

### 8.2 Actions

| Action | en | de | Effect |
|---|---|---|---|
| default, `Esc`, session end, 24 h | Don't install | Nicht installieren | `denied`, counts towards `DENIALS_BEFORE_STOP` |
| | For the next session | Für die nächste Session | approval with scope `next_session` |
| | For this project | Für dieses Projekt | approval with scope `project` |
| | Don't install, and stop asking | Nicht installieren und nicht mehr fragen | `denied`, and every further request of the session is `denied` |

"Don't install" and the two approvals are never adjacent. A card whose entry has a collision
offers only the two refusals.

### 8.3 Binding

The plan digest is the SHA-256 of this text, lines ending in `\n`:

```text
humanitl-tool-plan 1
catalog <serial>
platform <platform>
entry <id> <version> <artifact-sha256> <size> <layer-digest> <export>=<path>,…
entry …                     one line per entry of the closure, sorted by id
```

`Tools.Decide` carries it. A mismatch with the broker's current plan (a sync accepted a new catalog
meanwhile) refuses the decision with `TOOL_012`, and the client shows the new card.

### 8.4 Where an approval lives

Table `tool_approvals` in the register database (migration of item 5.2):

| Column | Type | Meaning |
|---|---|---|
| `project_id` | text | `ProjectId` of the register |
| `entry` | text | catalog id of the requested entry |
| `plan_digest` | blob | the plan the person saw |
| `scope` | text | `next_session` or `project` |
| `approved_at` | text | RFC 3339 |
| `approved_by` | text | the client that decided (`app`, `tui`, `cli`), as `flow.decided` names it |
| `request_id` | text | the request, or empty for the person's own pick |
| `last_mount` | text | RFC 3339 or empty |
| `used` | integer | 1 when a `next_session` approval was mounted once |

A `project` approval without a mount for 90 days expires at the next start, with `tool.revoked`
(reason `expired`).

## 9. The contract at field level

Field numbers inside new messages are fixed here; numbers in existing messages (`FlowEvent`,
`Info.capabilities`, the service) are the next free ones when item 5.2 lands.

```protobuf
service Humanitl {
  // ...
  rpc Tools(ToolsRequest) returns (ToolsResponse);
  rpc ToolsSync(ToolsSyncRequest) returns (stream ToolsSyncEvent);
}

message ToolsRequest {
  message Status { string project = 1; }            // project path; empty: no project column
  message Allow {                                   // the person's own pick
    string project = 1;
    string entry = 2;
    bytes plan_digest = 3;
    ToolScope scope = 4;
  }
  message Decide {                                  // the answer to a card
    string request_id = 1;
    bytes plan_digest = 2;
    ToolDecision decision = 3;
  }
  message Remove { string project = 1; string entry = 2; }
  message Verify {}                                 // re-hash every layer against its manifest
  message Gc { bool dry_run = 1; }
  oneof op {
    Status status = 1;
    Allow allow = 2;
    Decide decide = 3;
    Remove remove = 4;
    Verify verify = 5;
    Gc gc = 6;
  }
}

enum ToolScope {
  TOOL_SCOPE_UNSPECIFIED = 0;
  TOOL_SCOPE_NEXT_SESSION = 1;
  TOOL_SCOPE_PROJECT = 2;
}

enum ToolDecision {
  TOOL_DECISION_UNSPECIFIED = 0;
  TOOL_DECISION_DONT_INSTALL = 1;
  TOOL_DECISION_DONT_INSTALL_STOP_ASKING = 2;
  TOOL_DECISION_NEXT_SESSION = 3;
  TOOL_DECISION_PROJECT = 4;
}

message ToolsResponse {
  ToolCatalogInfo catalog = 1;
  repeated ToolEntryView entries = 2;               // for the platform, sorted by id
  repeated ToolRequestView open_requests = 3;       // cards that wait
  repeated Diagnostic diagnostics = 4;              // TOOL_00x of this operation
  bool enabled = 5;                                 // tools.enabled for the project
  string enabled_origin = 6;                        // the level, as GetConfig names it
  uint64 freed_bytes = 7;                           // Gc
}

message ToolCatalogInfo {
  uint64 serial = 1;
  string signed_at = 2;                             // YYYY-MM-DD
  string expires = 3;
  string key_id = 4;
  bool expired = 5;
  string source = 6;                                // "bundled" or "synced"
}

message ToolEntryView {
  string id = 1;
  string version = 2;
  string summary = 3;
  string license = 4;
  string homepage = 5;
  string released = 6;
  repeated ToolExport exports = 7;
  repeated string requires = 8;
  string runtime_network = 9;                       // "none" or "proxy"
  repeated string runtime_hosts = 10;
  repeated ToolArtifactView artifacts = 11;         // this platform only
  bool downloaded = 12;
  bool sealed = 13;
  repeated ToolApprovalView approvals = 14;         // for the project of Status
  bool revoked = 15;
}

message ToolExport { string name = 1; string path = 2; }

message ToolArtifactView {
  string url_host = 1;                              // A-label
  bytes sha256 = 2;
  uint64 size = 3;
  string format = 4;
  uint64 unpacked_size = 5;
  bytes layer_digest = 6;
}

message ToolApprovalView {
  ToolScope scope = 1;
  bytes plan_digest = 2;
  google.protobuf.Timestamp approved_at = 3;
  string approved_by = 4;
  google.protobuf.Timestamp last_mount = 5;
  google.protobuf.Timestamp expires_at = 6;
}

message ToolRequestView {
  string request_id = 1;
  string session_id = 2;
  string project = 3;
  string agent = 4;
  string entry = 5;
  bytes plan_digest = 6;
  repeated ToolEntryView closure = 7;               // the entry first, then what it requires
  uint64 download_bytes = 8;                        // 0 when everything is here
  repeated string download_hosts = 9;
  repeated ToolCollision collisions = 10;
  string reason = 11;                               // cleaned, masked, at most 200 characters
  bool reason_cut = 12;
  uint32 request_number = 13;                       // 2 in "request 2 of 10"
  uint32 request_limit = 14;
  google.protobuf.Timestamp at = 15;
  google.protobuf.Timestamp expires_at = 16;        // the card becomes "Don't install"
}

message ToolCollision { string export = 1; string meets = 2; }   // meets: a path in the sandbox, or "builtin"

message ToolsSyncRequest {
  bool catalog_only = 1;
  repeated string entries = 2;                      // empty: every approved entry
}

message ToolsSyncEvent {
  oneof event {
    ToolCatalogInfo catalog_accepted = 1;
    ToolsSyncProgress progress = 2;
    ToolsSyncDone done = 3;
    Diagnostic diagnostic = 4;
  }
}

message ToolsSyncProgress { string entry = 1; string host = 2; uint64 bytes = 3; uint64 total = 4; string step = 5; }
message ToolsSyncDone { uint32 fetched = 1; uint32 sealed = 2; uint32 failed = 3; }

// In FlowEvent's oneof, at the next free numbers:
//   ToolRequested tool_requested = N;
//   ToolAnswered tool_answered = N + 1;
message ToolRequested { ToolRequestView request = 1; }
message ToolAnswered {
  string request_id = 1;
  string entry = 2;
  string state = 3;                                 // "pending", "ready", "denied", "unavailable"
  repeated Diagnostic diagnostics = 4;              // for the person only
}
```

`Info.capabilities` gains `tools`. `Subscribe` carries the two events; a client that reconnects
after `Lagged` reads `Tools{status}`. The terminal client draws `ToolRequested` as a card in the
Decisions view and prints one status line per open card, like a request that waits for a
decision.

CLI, one command per operation, each a thin client:

| Command | RPC |
|---|---|
| `humanitl tools [--project DIR] [--json]` | `Tools{status}` |
| `humanitl tools sync [--catalog-only] [ENTRY…]` | `ToolsSync` |
| `humanitl tools allow ENTRY --project DIR [--next-session]` | `Tools{status}`, card, confirmation, `Tools{allow}` |
| `humanitl tools requests [--json]` | `Tools{status}` (`open_requests`) |
| `humanitl tools decide REQUEST-ID --for project\|next-session\|--dont-install [--stop-asking]` | `Tools{decide}` after printing the card |
| `humanitl tools remove ENTRY --project DIR` | `Tools{remove}` |
| `humanitl tools verify` | `Tools{verify}` |
| `humanitl tools gc [--dry-run]` | `Tools{gc}` |

`tools.enabled` and `tools.auto_sync` go through `SetConfig` like every key.

## 10. Threat model

The attacker is the agent with subverted intent (`BACKLOG.md` 4.3: prompt injection, a malicious
dependency, a compromised model), plus an upstream project or its release host that ships a
malicious version, plus whoever obtains the catalog key.

| Channel or attack | Example | Countermeasure | Residual risk |
|---|---|---|---|
| Name or version as a carrier | `POST /tools/aws-akia1234` so that a registry's logs show the key | closed catalog; unknown names answered from memory, no lookup, no download; no version field; pattern and length; quota of 10 | the choice among 25 entries: under 5 bits per request, at most 10 per session, each seen by a person |
| The reason | a secret in the reason, or "approve immediately, the build is down" | 200 characters, cleaned, findings masked, shown grey and marked unchecked, never in a command or on the network | a tired person persuaded by the text |
| URL, index or config injection | `--extra-index-url`, a `.npmrc` or `pip.conf` in the project | the request has no such field; the fetcher reads only the catalog; the extractor reads no configuration and sees no project | none in the install path |
| Install scripts with network | a `postinstall` sends the environment | no package manager runs; nothing unpacked is executed; the unpack sandbox has no door | none for exfiltration; an extractor exploit meets an empty sandbox |
| DNS as a tunnel | `<data>.attacker.example` from an installer | the unpack sandbox has no resolver and no route; the fetcher resolves only the six fixed hosts | none |
| Timing and volume | request patterns correlated with data | downloads only on sync or after a person's approval; answers from memory with fixed text | a few bits through the time of a person's approval |
| Oracle and back channel | build logs, README text or error messages as prompt injection; "is it already here?" | four fixed answers with fixed details; logs go to the person and, hashed, to the audit log | none |
| Typosquatting, compromised release | a lookalike project; a hijacked maintainer account ships a backdoor | catalog reviewed by pull request; cooldown of 14 days; hash pinned; reproduced layer digest; revocation list | **main risk:** a hash and a signature prove origin, not intent |
| Persistence and shadowing | a tool named `git` or `ssh`; a `.pth` file; `ld.so.preload`; a shell start file | layer read-only, `nosuid`, `nodev`; end of `PATH`; collisions refused; hooks dropped; no environment variable changed | persistence through the writable project folder, the same risk as with the agent itself |
| The tool at run time | telemetry, slow exfiltration of the project | the tool has the agent's rights only: same seccomp, same namespace, same proxy, rules, holds and findings | exfiltration through allowed hosts stays the proxy's matter |
| Self-approval through the repository | the agent writes `tools.lock` and expects the next session to mount it | approvals only in the register on the host; the repository file can only switch tools off | none |
| Archive and parser attacks | `../`, absolute or escaping links, hard links, devices, set-uid, a decompression bomb, a crafted header | pure-Rust extractor in a sandbox without a door; refusals and limits; the sealer re-checks without following links; no image is mounted by the host kernel | bugs in the extractor (contained) and in the sealer's walk (narrow, host code) |
| Catalog key compromise | a forged entry pointing to a malicious artifact | key only in the release workflow; separate from the update key; rotation and revocation; a person still approves each entry | until a machine updates, it trusts the stolen key |
| Rollback and freeze | an old catalog with a revoked entry; a deleted state file; a downgraded package; withholding new catalogs; a clock turned back | `MIN_SERIAL` and `REVOKED` per release; serial window `MIN_SERIAL + 50`; monotonic serial with a conflict check; cumulative revocation; expiry after 180 days against the later of the clock and the newest signed date | a machine that stays offline keeps mounting what it approved and receives no revocation; a clock turned back keeps an old catalog usable up to its `expires`; a stolen key can block newer catalogs until the next release |
| Approval fatigue | many requests until the person clicks through | "Don't install" focused and default; quotas; stop after three refusals; no approval for every project or by pattern | a person who approves without reading |
| Tampering with the store | another process of the same user changes a layer | the full re-hash at every mount; `humanitl tools verify` | that process could change the daemon's data anyway, as with the depot |
| Swapping a layer between check and bind | a link replaced after the hash | the hash runs through the opened directory descriptor that is bound, where the backend binds by descriptor | backends that bind by path have a short window, local to the user |

What tools on request protect: the host, the depot and other sessions from the installation, and
the supply chain from being invisible. What they do not protect: the session from a tool that is
what its hash says and malicious. For that, the sandbox and the proxy remain the boundary, as
they are for the agent.

## 11. Statements for the security documents

[ADR-0029 point 9](../adr/0029-tools-on-request-off-by-default.md#9-what-docssecuritymd-and-docsthreat-modelmd-will-need)
lists them; item 5.5 writes them into `docs/SECURITY.md` and `docs/THREAT-MODEL.md` in one
commit, with the escape tests and their mutation proofs.

## 12. Not in the first version

- distribution packages and dependency resolution (Debian snapshots, `.deb`, maintainer scripts);
- language registries as sources (PyPI, npm, crates.io, Go modules);
- building from source, and any builder in a micro-VM;
- project dependencies (`npm ci`, `uv sync` into the project folder);
- mounting into a running session;
- macOS and Windows;
- `zip` archives;
- approvals for every project, by name pattern, or by company policy;
- a file in the repository that lists or proposes tools;
- automatic updates of approved tools;
- vulnerability feeds and malware scanners;
- a transparency log with an outside witness;
- a network profile per tool;
- attributing a tool's requests to the tool in the history;
- a Rust toolchain, whose installer is a script.
