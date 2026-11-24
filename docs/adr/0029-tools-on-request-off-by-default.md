# ADR-0029 · The agent asks for a tool from a signed catalog, a person decides, and the tool arrives as a sealed read-only layer; off by default

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-24

## Context

An agent often needs a program the sandbox lacks: a JSON processor, a
search tool, a compiler, a runtime. Today it has two ways to get one, and
both are bad. It can download the program through the proxy, which costs a
decision per request and puts a binary of unknown origin into the project
folder; or the person installs it on the host, which reaches the sandbox
only through the read-only host directories and is outside Humanitl's view.
The owner wants the agent to be able to ask for a tool and get it installed
safely, without data leaving, and only when the person has switched this on
(owner decision E9 of 2026-11-23: opt-in per project, catalog fixed in
advance, mounted at session start; sprint 9 tracking issue #86, phase 5).

Cutting the agent's network while something installs does not help. Data can
leave through the request itself (a package name, a version, an index
address, free text carry bits) and through an installer that has network
while it runs foreign code (`postinstall`, `setup.py`, `build.rs`, a DNS
lookup). The expert interviews behind this decision (25 simulated
specialists in supply chain, sandboxing, package managers, cryptography,
consent interfaces and incident response) agreed on three separations: the
agent only asks and never installs; network and foreign code never meet; the
result is an immutable artefact that runs with exactly the agent's rights.

What exists today, on `main` at `a5941c0f`:

- **The one door.** The sandbox reaches the background service only through
  the proxy socket. The meta host `humanitl.internal`
  (`daemon/crates/proxy/src/meta.rs`, ADR-0014) is answered by the proxy
  itself: `GET /`, `GET /why/<flow-id>`, `POST /ask` (2 KiB, ten per minute,
  cleaned by `sanitize_note`). ADR-0026 point 5 closed what the agent reads
  there and said that `humanitl.internal` gets no new path; this decision
  adds one path family and amends that sentence.
- **The control stream** of the shim (`daemon/bin/humanitl-shim/src/frame.rs`,
  HUM-443) carries terminal bytes between the background service and the
  shim; the agent cannot reach it (its filter refuses `AF_UNIX` and
  `AF_VSOCK`). It stays as it is.
- **Read-only binds per session.** `LaunchOptions::session_ro` carries
  read-only binds through every sandbox backend; on the namespaces level the
  source is an opened descriptor, so a path swapped after the check is not
  what is bound.
- **The agent depot** (ADR-0023, HUM-518 to HUM-566) installs an agent in a
  maintenance run: a sandbox session with network through the proxy and
  allow rules for the recipe's hosts, after which the background service
  checks the tree (`depot/adopt.rs`: regular files, relative links inside the
  tree, no set-uid, no hard links out), the vendor's signature or checksum
  list (`depot/verify.rs`, `depot/checksums.rs`) and commits
  `program/<version>/` with a `manifest.json`.
- **A signed download already exists.** The self-update (HUM-382,
  `daemon/crates/proxy/src/self_update.rs`) fetches with its own connection
  stack over the egress `Direct`, never through the agent's proxy, and
  verifies an Ed25519 signature over the exact bytes of a manifest through
  `rustls::crypto::ring`'s `verify_signature` for the scheme `ED25519`
  (`ring` underneath). `Updater::newer` there compares a release only with
  the running version; the floor against a replayed older manifest lives in
  `daemon/crates/ipc/src/update.rs` (`installed_floor`): the highest of the
  running version, the installed one and the highest version this daemon
  ever installed at that target, kept in `update-installed.json`.
- **Settings.** The project choice in the register (`PROJECT_CHOICE_KEYS`,
  `backlog/CONVENTIONS.md` 4.51) holds per-project values on the host, never
  in the project folder; ADR-0025 point 6 lets the repository file only
  tighten.

"Item 5.N" below is the numbering of phase 5 of the sprint's tracking issue
(#86): 5.1 catalog, store and sync; 5.2 the request path and `humanitl-need`;
5.3 unpacking, sealing and the layer; 5.4 the card; 5.5 the shims, the escape
tests and the security documents.

Hard constraints: the three sandbox guarantees, `tests/escape/`, the hard
block and the terminal output filter stay as they are. Every capability is an
RPC first (ADR-0018). No new port without a second adapter (ADR-0015), no
sandbox backend named outside the sandbox crate and the shim, and the counts
of `tools/coupling-baseline.toml` do not rise. No cryptography of our own.
The agent's vocabulary of ADR-0026 point 5 stays closed and says nothing
about a person. `docs/SECURITY.md` and `docs/THREAT-MODEL.md` are not changed
by this decision; point 9 lists what item 5.5 writes into them.

The full design, with the catalog format, the messages at field level, the
sealing checks, the card and the threat table, stands in
[`docs/design/tools-on-request.md`](../design/tools-on-request.md). This
record fixes the decisions; where the two disagree, this record wins and the
design document has a bug.

## Decision

We let the agent ask, through the one door it already has, for an entry of a
closed, signed catalog that the background service keeps locally. A person
decides on a card, deny is the default, and nothing of this exists until the
person switches it on for a project. Downloading is code of the background
service against fixed hosts with hashes known in advance; unpacking runs in a
throwaway sandbox without network, project or secrets; the result is a
content-addressed, read-only layer sealed by the background service and
mounted at the next session start, at the end of `PATH`. At run time the tool
is a program of the agent like any other, behind the same proxy.

### 1. Off by default, switched on per project

- **The switch** is `tools.enabled` (boolean, default `false`, `x-tier =
  "basic"`, section 4 "Isolation and reach" of ADR-0025 point 7, takes effect
  `next_session`). It can be set to `true` only on the level "This project",
  that is in the project choice of the register; it joins
  `PROJECT_CHOICE_KEYS`. On "Mine" and in a profile it may only be `false`.
  The repository file `.humanitl/profile.toml` may set it, and only to
  `false` (`x-repo-direction = "stricter"` with the order `false` < `true`,
  ADR-0025 point 6). The environment (`HUMANITL_TOOLS__ENABLED`) and a
  command-line override rank above the project choice and may likewise only
  set `false`: a `true` from either is ignored, the resolved origin stays the
  project choice, and the start carries a warning `TOOL_010` naming the
  source. A test drives `HUMANITL_TOOLS__ENABLED=true` against a project
  choice of `false` and expects `false` and the warning, and the reverse
  (`false` from the environment over `true` in the choice) expects `false`.
  A company lock may fix it to `false`; a lock to `true` is refused. No
  preset sets it.
- **Platform.** Linux only in this version. On macOS `SetConfig` refuses
  `true` with `TOOL_010`, whose `fix` says that the feature is Linux-only.
- **Audit.** The switch writes `config.changed` with level and origin
  (ADR-0025); every request, decision, download, unpack, seal, mount and
  revocation writes one record of point 8.
- **What switching off does.** The next session mounts no tool, `humanitl-need`
  answers `unavailable`, and approvals stay stored until the person removes
  them; switching on again brings them back without new cards.

### 2. The agent only asks

- **The channel** is the meta host `humanitl.internal` over the proxy socket,
  the only door of the sandbox (ADR-0014). No new socket, no file, no
  environment variable; the shim's control stream gains nothing. Three
  paths join the table of ADR-0014 point 3, answered by the proxy itself
  without DNS, upstream or rules:

  | Path | Method | Effect |
  |---|---|---|
  | `/tools` | `GET` | one line per catalog entry for this platform: id, version, a summary of at most 60 ASCII characters, and the state for this session |
  | `/tools/<name>` | `GET` | the state of one entry for this session; creates nothing |
  | `/tools/<name>` | `POST` | a request; the body is an optional reason of at most 200 characters |

- **The name** must match `^[a-z0-9][a-z0-9-]{0,31}$` and be the id of an
  entry of the accepted catalog for the platform. The agent names no
  version, no URL, no index, no flag, no script and no path: the catalog has
  exactly one version per entry and serial, and the request has no other
  field. A name that fails the pattern, or matches no entry, is answered
  `unavailable` from memory, with no disk access and no network, and only
  its SHA-256 and length are recorded.
- **The reason** is display text for the person: cleaned like `/ask`
  (`sanitize_note`: control, invisible and direction characters removed),
  cut to 200 characters, scanned by the findings detectors with every
  finding masked, and never used in a command, a file name, a URL or a
  network request.
- **Limits per session**, fixed constants, not settings: at most 3 open
  requests, at most 10 requests in total (unknown names count), and after
  3 entries the person did not install in that session every further request
  is answered `denied` without a card (the person sees the count). `GET`
  requests on `/tools` have a counter of their own per session, 30 per
  minute, separate from the window of `/ask` (`take_ask_slot` in
  `meta.rs`, which answers `429` with free text). Over that limit a `GET`
  answers inside the vocabulary: `200`, `status: unavailable` with its fixed
  `detail:`, and `GET /tools` the single line `tools: unavailable`, which
  does not claim that tools are off; never `429`.
- **Fixed answers.** Every answer is `200 OK`, `text/plain`, ASCII, three or
  four lines, with exactly one of four states:

  ```text
  tool: jq
  status: ready
  path: /opt/humanitl/tools/bin/jq
  detail: available from the next session start
  ```

  | `status:` | When | `detail:` (fixed English text) |
  |---|---|---|
  | `pending` | the request was taken and nobody has answered it yet | "the request was passed on; ask again later with GET, do not repeat it" |
  | `ready` | an approval exists and the layer is sealed | "available in this session" when the layer is mounted, else "available from the next session start" |
  | `denied` | a person chose not to install, the card expired, or the session reached the limit of denials | "not permitted here; do not ask for this tool again in this session" |
  | `unavailable` | tools are off, the name is unknown, the quota is used up, the catalog expired, the entry was revoked, a name collides, or sealing failed | "not available here; do not ask for this tool again in this session" |

  `tool:` is printed only for a name that matches the pattern; `path:` only
  with `ready`. Nothing else reaches the agent: no build or unpack log, no
  hash, no size, no host, no `Diagnostic`, no reason why something is
  `unavailable`, and no word saying that a person decides (HUM-364): `denied`
  reads the same whether a person, an expired card or the stop after three
  refusals caused it. Answers are computed from memory only: the broker
  keeps the accepted catalog, the quotas, the approvals of the project and
  the set of blobs present in `tools/blobs/` in memory (refreshed by sync,
  decision, `gc` and session start), and builds a plan without touching the
  disk. A known and an unknown name therefore take the same path; no
  stronger timing claim is made.
- **Accepted leaks to the agent**, named so nobody mistakes them for
  oversights: `GET /tools` shows the state of every entry for this session,
  so the agent learns which tools the person approved for the project; and
  `unavailable` versus `pending` for a known entry tells the agent one bit,
  whether tools are usable here at all (on, catalog valid, quota left). Both
  describe the agent's own environment, are what it needs to stop asking,
  and say nothing about a person or about the request's content.
- **`humanitl-need`** is a second name of the shim binary (`argv[0]`), bound
  read-only at `/usr/local/bin/humanitl-need` in sessions of a project with
  tools switched on. It sends the request over the proxy port the sandbox
  already uses, prints the answer unchanged and exits with 0 (`ready`),
  2 (`pending`), 3 (`denied`) or 4 (`unavailable`). `humanitl-need --list`
  is `GET /tools`, `humanitl-need --status NAME` is `GET /tools/NAME`.
  `curl -X POST http://humanitl.internal/tools/jq` does the same. When tools
  are on, the briefing gains one fixed line naming the command; when they are
  off, the briefing says nothing about tools.
- **Inside the background service** the proxy hands a request to the broker
  through `humanitl_core::tools::ToolDesk`, a concrete struct and no trait,
  built the way `SandboxNoticeSink` and `InterceptPause` are: `Clone`, cheap
  to copy, holding one `Arc<dyn Fn(ToolCall) -> ToolReply + Send + Sync>`
  that `humanitld` sets with `ToolDesk::serving(...)` to the broker, and a
  constructor `ToolDesk::off()` for tests and the fake mode that answers
  `tools: off` and `unavailable`. Its methods are `ask(ToolAsk) ->
  ToolAnswer`, `state(SessionId, &ToolName) -> ToolAnswer` and
  `list(SessionId) -> ToolListing`. The call is synchronous and in memory;
  audit records go through the writer's queue. The fields are in the design
  document, section 5.

### 3. Network and foreign code never meet

- **The catalog is synchronised ahead of time**, by `humanitl tools sync` or,
  when the person switched it on, once a day (`tools.auto_sync`, default
  `false`, level "Mine"). A request never causes a download.
- **Fetching** is code of the background service in `humanitl-proxy`, next
  to the self-update and built the same way: its own connection stack over
  the egress `Direct` (ADR-0017), never the proxy the agent talks to; HTTPS
  only; `GET` only; no query string, cookie, credential or header beyond
  `Accept` and `User-Agent: Humanitl/<version>`; at most three redirects,
  each to a host of the fixed list; the size the catalog names as a hard
  cap; SHA-256 computed while reading, and the file is kept under
  `tools/blobs/<sha256>` only when it matches. The host list is compiled in:
  `github.com`, `objects.githubusercontent.com`,
  `release-assets.githubusercontent.com`, `nodejs.org`, `dl.google.com`,
  `ziglang.org`. A catalog entry whose URL names another host, a port, user
  information, a query or a fragment makes the whole catalog invalid.
- **What is fetched when.** A sync fetches the catalog and the artifacts of
  every entry approved for a project. An entry that is not yet downloaded is
  fetched only after a person allowed it on its card, and the card says so
  with hosts and bytes before the click. This is a deliberate narrowing of
  the specification's "a request causes no network traffic" (owner, 2026-11-24):
  the request itself causes none; a person's allow on the card may cause one
  download of exactly the artifacts the card lists. The agent's choice among
  the entries of the catalog is therefore the only information a request can
  move towards the network, and only through a person.
- **Unpacking** runs in a throwaway sandbox built by the configured sandbox
  backend at the project's isolation level, through `SandboxBackend::plan`
  and `launch` with a profile of its own (`profiles/sandbox/tool-unpack.toml`):
  no bridge and no proxy socket, so no door at all; no project folder, no
  home with data, no depot, no CA, no daemon socket, no session
  environment; the artifact read-only at `/in/artifact`, an empty writable
  `/out`, a 64 MiB `/tmp`; a wall time of 120 seconds; the seccomp filter of
  the default profile. The program it runs is `humanitl-unpack`, a new
  static binary built like the shim (`daemon/bin/humanitl-unpack`), which
  reads `raw`, `tar.gz` and `tar.xz` with pure-Rust decoders and runs
  nothing it unpacks. It refuses absolute paths, `..`, links leaving `/out`,
  hard links, device nodes, FIFOs, sockets and set-uid or set-gid bits, and
  stops at the sizes the catalog names. There are no maintainer scripts in
  this version, because there are no distribution packages (point 10).
- **The sandbox is the boundary, the extractor is not.** The background
  service treats `/out` as hostile and checks it again while sealing
  (point 4); a flaw in the extractor yields at most a failed seal.

### 4. The result: a sealed, read-only layer at a fixed place

- **Sealing** is host code of the background service that walks `/out`
  without following any link (`openat2` with `RESOLVE_BENEATH` and
  `RESOLVE_NO_SYMLINKS`, `O_NOFOLLOW`) and copies what it accepts into a new
  tree. The rules, in full in the design document section 7:
  1. only regular files, directories and symbolic links whose target is
     relative and stays inside the layer; everything else fails the seal;
  2. modes become `0444` or `0555` for files (`0555` when any execute bit was
     set) and `0555` for directories; set-uid, set-gid and sticky bits fail
     the seal; owner, extended attributes, ACLs and file capabilities are
     not copied;
  3. names are printable ASCII without `/`, at most 255 bytes, depth at most
     32; at most 100,000 entries, one file at most 512 MiB, the layer at
     most the catalog's `unpacked_size` and never above 2 GiB;
  4. startup hooks are dropped and listed in the seal manifest: `*.pth`,
     `sitecustomize.py`, `usercustomize.py`, anything under a directory named
     `etc`, `profile.d`, `autostart`, `systemd` or `cron.*`, files named
     `ld.so.preload` or `ld.so.conf*`, shell start files (`.bashrc`,
     `.profile`, `.zshrc`), and package manager configuration (`.npmrc`,
     `npmrc`, `pip.conf`, `.gitconfig`). Configuration a runtime reads
     relative to its own prefix (`lib/node_modules/npm/npmrc`,
     `pyvenv.cfg`, `pip.conf` inside a Python prefix) falls under this rule
     too; the catalog workflow runs each entry's exports once with
     `--version` (or the entry's declared smoke command) on the sealed layer
     with these files dropped, so an entry that needs one fails admission
     instead of a session;
  5. the digest is the SHA-256 of a canonical manifest (one line per entry,
     sorted by path bytes: type, path, mode, size and SHA-256 or link
     target); it must equal the catalog's `layer_digest`, which the catalog
     pipeline computed with the same sealer, or the seal fails.
- **The store** is `$XDG_DATA_HOME/humanitl/tools/store/<layer-digest>/`,
  written as `staging/` and renamed into place, never changed afterwards,
  with its manifest next to it. Only the background service writes there.
- **Mounting** happens at session start in this version, never into a running
  session. For every approval of the project (point 6) the launcher opens
  the layer's directory once, hashes every file through that descriptor
  (`openat2` beneath it, no link followed) against the manifest, and adds
  that same descriptor as `(layer, /opt/humanitl/tools/<id>)` to
  `LaunchOptions::session_ro`, so what was hashed is what is bound. A
  per-session directory of relative links (`jq -> ../jq/bin/jq`), built
  under `$XDG_DATA_HOME/humanitl/tools/sessions/<id>/bin/` and never under
  the runtime directory, is bound
  read-only at `/opt/humanitl/tools/bin`, and the launcher appends
  `:/opt/humanitl/tools/bin` to the end of `PATH`. No other variable changes:
  no `LD_LIBRARY_PATH`, `PYTHONPATH`, `NODE_PATH` or `MANPATH`. Every mount is
  read-only, `nosuid` and `nodev`; the shim's start report reads
  `/proc/self/mountinfo` and the start fails with `TOOL_009` when any mount
  under `/opt/humanitl/tools` lacks one of the three.
- **Name collisions** are refused, not ordered. Before the start the launcher
  compares every exported name with the executables in each `PATH`
  directory of the planned sandbox (the sources of the plan's read-only
  mounts, the agent program, the shim's names), with the shell's builtins
  and keywords, and with the exports of the other approved tools. A
  collision leaves that tool out of the session, gives the person `TOOL_008`
  and the agent `unavailable` for it. Collisions depend on the host: a host
  that ships `python3`, `node`, `go`, `gh` or `cmake` in a directory the
  sandbox binds makes the same entry collide there and not elsewhere. The
  broker therefore checks collisions already when it builds the plan, the
  card shows each one with the program it meets, and approving a colliding
  entry is not offered. A layer whose digest does not match at mount time is
  left out with `TOOL_009`. Either way the session starts.

### 5. At run time the tool is the agent

The tool runs as a child of the agent, in the same sandbox, under the same
seccomp filter, in the same network namespace, with the same environment
and the same proxy. Every request it makes is held, ruled and recorded like
any request of the agent; nothing marks it as trusted. The catalog cannot set
an environment variable, a network rule or a mount. A tool that needs the
network at run time says so in its entry (`runtime_network = "proxy"` with
the hosts), and the card shows it; that is information for the person, not a
rule.

### 6. The person decides

- **The card** appears in the Decisions view of the app, the TUI and the
  terminal (`humanitl tools requests`), from the same RPC, never in the
  agent's terminal. Its fields, in order (design document section 8): the
  head "The agent asks for a tool" with project, agent and the quota
  ("request 2 of 10 in this session"); name, version, summary, license;
  origin (catalog serial, signing date and key id, the A-label of every
  download host); every artifact's SHA-256 in full and the layer digest;
  download and unpacked size; the full dependency closure, each entry with
  version, hashes and sizes, or "no further entries"; whether the artifacts
  are already here ("0 bytes over the network") or how many bytes from which
  hosts allowing will download; the commands it adds and that they come last
  on `PATH`; any collision; the release date; the network the tool needs at
  run time; the fixed sentence "The tool runs with the agent's rights and
  sees what the agent sees. A hash proves where it came from, not that it is
  harmless."; and last, small and grey, "Reason from the agent, not checked"
  with the reason.
- **Actions.** "Don't install" has the focus, answers `Esc` and is what a
  card becomes when the session ends or 24 hours pass. "For the next session"
  approves one start of a session of this project. "For this project"
  approves every later start until the person removes it or 90 days pass
  without a start that mounted it. "Don't install, and stop asking" also
  answers `denied` to every further request of that session. There is no
  "always for every project", no name pattern and no automatic approval by
  policy in this version.
- **Binding.** The decision carries the plan digest of the card: SHA-256 over
  catalog serial, platform, and for every entry of the closure its id,
  version, artifact hashes and sizes, layer digest and exports. A decision
  whose digest differs from the broker's current plan is refused with
  `TOOL_012`; a changed catalog therefore needs a new card.
- **Where the approval lives.** In the register of the background service
  (table `tool_approvals`: project id, entry id, plan digest, scope, time,
  who, last mount), never in the project folder. No file in the repository
  approves a tool, and in this sprint no file in the repository proposes one
  (#86, "Not in this sprint").
- **The person's own path.** The person may approve without a request: the
  Tools page of the project lists the catalog, and choosing an entry shows
  the same card. `humanitl tools allow ENTRY --project DIR` does the same in
  the terminal after printing the card and asking for confirmation.

### 7. The catalog

- **Format.** One TOML file, `catalog/tools.toml` in the repository,
  reviewed by pull request, with `format = 1`, a strictly increasing
  `serial`, `signed_at`, `expires` (at most 180 days after `signed_at`),
  `key_id`, a list `revoked` of artifact or layer digests, and one
  `[[entry]]` per tool with `id`, `version`, `summary`, `license`,
  `homepage` (shown, never fetched), `released`, `exports`, `requires`
  (other entry ids, acyclic), `runtime_network`, `runtime_hosts`, and one
  `[[entry.artifact]]` per platform (`linux-x86_64`, `linux-aarch64`) with
  `url`, `sha256`, `size`, `format`, `strip_components`, `unpacked_size` and
  `layer_digest`. The schema is in the design document, section 6.
- **Signature.** Ed25519 over the exact bytes of the file, the same scheme as
  the self-update: the release pipeline signs with `openssl pkeyutl -sign
  -rawin` using a repository secret `HUMANITL_TOOLS_CATALOG_SIGNING_KEY`, in
  a step that first checks the secret against the public key built into the
  daemon; the background service verifies the same way the self-update
  does, through `rustls::crypto::ring`'s `verify_signature` for
  `SignatureScheme::ED25519`, before it parses a single field. The signature travels as
  `tools.toml.sig` (64 bytes, base64). `ring`, `toml` and `base64` are already
  workspace dependencies.
- **Keys.** A key of its own, separate from the update key, so that one can
  be rotated or revoked without the other. The daemon carries at most two
  public keys (`humanitl_tools::CATALOG_KEYS`, raw 32 bytes, base64, each
  with its `key_id`): the current one and, during a rotation, the next. A
  rotation ships the next key in a release, switches `key_id` in a later
  catalog and drops the old key in the release after. After a suspected
  compromise a release drops the key and a catalog signed with the new one
  revokes what the old one could have signed.
- **Rollback and the serial window.** Every release compiles in two
  constants beside `CATALOG_KEYS`: `humanitl_tools::MIN_SERIAL`, the serial
  of the catalog that release bundles, and `humanitl_tools::REVOKED`, the
  union of every revoked digest up to that catalog. A catalog is accepted
  only when its serial lies in the window `MIN_SERIAL <= serial <=
  MIN_SERIAL + 50` and is not below the serial in the state; bundled and
  downloaded copies alike, before and after a state reset. 50 is room for
  fifty catalogs published while a machine runs an older Humanitl; a catalog
  above the window is refused with `TOOL_002`, whose `fix` is to update
  Humanitl.
  - `tools/catalog/state.json` holds the highest accepted serial, the
    `key_id` that signed it, the SHA-256 of that file, the union of every
    `revoked` digest ever accepted, and the newest `signed_at` of an accepted
    catalog. A lower serial is refused (`TOOL_002`); the same serial with
    other bytes is refused (`TOOL_002`) and recorded as a possible key
    misuse.
  - When the state file is missing or does not parse, the background service
    accepts only a catalog in the window and at or above the bundled serial,
    writes a new state from it, and records `tool.catalog` with
    `state_reset`. The revocation union then starts from `REVOKED` and that
    catalog's own list, so a reset loses no revocation the release knew.
  - **What a stolen key can do to the serial.** It can sign a catalog up to
    `MIN_SERIAL + 50` of the releases it targets and so make legitimate
    catalogs below that serial fail with `TOOL_002` on machines that accepted
    it. The lockout ends with the next release, which drops the key and sets
    `MIN_SERIAL` above the poisoned serial; it cannot reach a serial near
    2^63 and cannot outlast a release.
  - A serial signed by a `key_id` other than the stored one is accepted only
    when that key is one of `CATALOG_KEYS`, and the switch is recorded.
- **Revocation is cumulative.** The check uses the compiled-in `REVOKED`
  plus the union in the state; a later catalog that omits a digest does not
  revive it. A catalog whose entries or artifacts name a digest of that set
  is refused as a whole (`TOOL_011`). Release policy keeps every revocation
  in every later catalog anyway. An accepted revocation voids every approval
  whose plan names a listed digest (audit `tool.revoked`), and the layer is
  no longer mounted.
- **Expiry and the clock.** `signed_at` and `expires` are calendar dates in
  UTC, and the clock is the system's wall clock read as a UTC date. The date
  in force is the later of the clock and the newest `signed_at` in the state.
  The state stores a date only from that validated source, never from the
  wall clock, so a wrong clock never writes a wrong date; a forward jump
  expires the catalog early only while the clock is wrong and ends when it is
  corrected. Turning the clock back cannot move the date in force below the
  newest signed date, but it can keep an old catalog in use up to its
  `expires`: the person controls their own clock, and this is named as a
  residual risk. An expired catalog accepts no new request (`unavailable`)
  and no approval; layers already approved keep mounting, and every start
  shows `TOOL_003` until a newer catalog arrives.
- **Distribution.** Every release carries `tools.toml` and `tools.toml.sig`
  as release files and in the packages (`/usr/share/humanitl/`). The bundled
  copy goes through the same check as a downloaded one; the newer serial
  wins. `humanitl tools sync` reads the latest release the update check
  already knows and fetches its two files from
  `https://github.com/nurkert/Humanitl/releases/download/v<version>/`.
  Catalog updates therefore come with releases.
- **Admission to the catalog**, checked in a pull request workflow that
  touches the network (`.github/workflows/tools-catalog.yml`, not part of
  `make check`): every artifact downloads from an allowed host and has the
  stated hash and size; the release is at least 14 days old at signing; the
  extractor and the sealer reproduce `layer_digest`; each export runs its
  smoke command on the sealed layer with the hooks of point 4 dropped; no
  export collides with another entry or with a list of common system
  commands; no digest of the cumulative revocation list is used; the serial
  is the previous one plus one. The offline
  part (schema, hosts, ids, exports, cycles) runs in `make check`.
- **The first version** has 25 entries, Linux on x86-64 and AArch64, each a
  self-contained archive from the project's own release host: `jq`, `yq`,
  `ripgrep`, `fd`, `bat`, `delta`, `difftastic`, `hyperfine`, `sd`, `just`,
  `shellcheck`, `shfmt`, `hadolint`, `actionlint`, `ruff`, `uv`, `python`
  (relocatable standalone build), `node` (with `npm` and `npx`), `go`, `zig`
  (also the C and C++ compiler through `zig cc`), `cmake`, `pandoc`, `gh`,
  `xh` and `golangci-lint`. Versions and hashes are pinned by item 5.1; the
  design document, section 6.4, lists source host, format and run-time
  network per entry.

### 8. Relation to what exists, and no new port

- **The depot** is the model for the mechanics and not for the trust. Its
  maintenance runs execute a vendor's installer with network and keep a
  login; the tool path executes nothing it downloads, has no network where it
  unpacks and keeps no login. They share code, not state: the tree checks of
  `depot/adopt.rs` move into one sealing module both use, the manifest
  follows the same pattern, and both reach the sandbox through
  `LaunchOptions`. They share no directory (`tools/` and `agents/` are
  separate under the data directory), no tool layer is ever mounted into a
  maintenance run or into the unpack sandbox, and a catalog sync is not a
  maintenance run: it runs no foreign code, so it needs no sandbox.
- **`SandboxBackend`** gains nothing. The unpack sandbox is a plan with a
  profile of its own, the layers are `session_ro` binds, and the mount
  options are checked in the shim's report. No code outside the sandbox crate
  and the shim names a backend; `tools/check_coupling.py` keeps its counts.
- **The broker is not a port.** It is application code in the background
  service with one implementation. Its outside world uses ports that exist:
  the download goes through `Egress`, the unpacking through `SandboxBackend`,
  the records through `AuditSink`, the approvals through the register's
  store. `ToolDesk` is a concrete struct in the core holding one closure,
  as `SandboxNoticeSink` and `InterceptPause` are; it lets the proxy hand a
  request to the background service without depending on it, defines no
  trait, and has nothing to swap. A
  second source kind (distribution packages with a resolver, a company
  mirror, Nix) would be the second adapter of a `ToolSource` port, and the
  decision that adds it creates the port.
- **Crates.** New: `humanitl-tools` (`daemon/crates/tools`, depends on core
  and config): catalog, signature, rollback state, plan and digest, quotas,
  approvals, store and sealer. New binary: `humanitl-unpack`. Grown:
  `humanitl-core` (`tools` module with `ToolName`, `ToolAsk`, `ToolAnswer`,
  `ToolDesk`), `humanitl-proxy` (the three meta paths, the fetcher next to
  the self-update), `humanitl-sandbox` (unpack plan, layer binds, `PATH`,
  collision check), the shim (`humanitl-need`, the mount check),
  `humanitl-ipc` (RPCs) and `humanitld` (wiring, audit). New workspace
  dependencies, added centrally by the implementing issue: `tar` and
  `lzma-rs` (pure Rust, no C library) for the extractor.
- **Contract.** One RPC per capability (ADR-0018), `Tools` with `Status`,
  `Allow`, `Decide`, `Remove`, `Verify` and `Gc`, `ToolsSync` as a stream,
  and `FlowEvent` gains `ToolRequested` and `ToolAnswered`; the messages at
  field level are in the design document, section 9. The parity table gains
  `humanitl tools`, `tools sync`, `tools allow`, `tools requests`,
  `tools decide`, `tools remove`, `tools verify` and `tools gc`.
- **Audit kinds**: `tool.catalog` (serial, key id, accepted or the reason it
  was refused), `tool.requested` (session, request id, entry id or the
  SHA-256 and length of an unknown name, SHA-256 of the reason, answer),
  `tool.decided` (request id or none, entry, plan digest, scope, who),
  `tool.fetched` (host, URL, bytes, SHA-256, result), `tool.unpacked`
  (backend level, limits, exit, SHA-256 of the log), `tool.sealed` (layer
  digest, dropped paths, result), `tool.mounted` (session, entry, layer
  digest) and `tool.revoked`. In the audit log a reason, a log and an
  unknown name are stored only as SHA-256 (and length). The recording keeps
  the meta flow of a tool request like `/ask` (HUM-103), with one
  difference: for a path under `/tools` the recorder stores the path and the
  body only as SHA-256 and length, never as text, so an unknown name or a
  reason carrying a secret is in neither store. The person reads the
  cleaned, masked reason only on the card, from the broker's memory, until
  the card is answered.
- **Diagnostics**, the next free numbers of a new prefix `TOOL`, each with
  title and why in `en` and `de`: `TOOL_001` catalog refused (signature or
  content; `why` names the step),
  `TOOL_002` catalog serial out of range (below the release floor or the
  accepted serial, the same serial with other bytes, or above the window of
  point 7), `TOOL_003` catalog expired,
  `TOOL_004` artifact hash or size differs, `TOOL_005` download host or
  redirect refused, `TOOL_006` unpacking failed or hit a limit, `TOOL_007`
  sealing refused an entry, `TOOL_008` name collision, `TOOL_009` layer
  digest or mount options wrong at start, `TOOL_010` tools cannot be
  switched on here, `TOOL_011` revoked digest (an approved entry was revoked,
  or a catalog was refused because it names a revoked digest), `TOOL_012`
  the card changed
  before the decision.

### 9. What `docs/SECURITY.md` and `docs/THREAT-MODEL.md` will need

Item 5.5 changes both files in the same commit, with these statements; no
earlier item switches the feature on outside tests. No guarantee changes
and no statement of `README.md` changes: a layer is a read-only input like
the host directories the profile already binds, it adds no socket and no
interface, and every byte a tool sends still passes the proxy.

`docs/SECURITY.md`:

1. Section 1, "What the claim does not say", a new item: an approved tool is
   code from outside that runs with the agent's rights; its hash proves where
   it came from, not that it is harmless, and sandbox and proxy stay the
   boundary for it as for the agent.
2. Section 3, guarantee 2: the tool paths on `humanitl.internal` are requests
   over the existing door; `humanitl-need` is the shim under a second name
   and opens nothing.
3. A new section "Tools on request" in section 6: off by default and only per
   project; the closed request and its four answers; the limits; downloading
   by the background service from the fixed hosts; the unpack sandbox
   without a door; the sealing rules; the layer at the end of `PATH`,
   read-only, `nosuid`, `nodev`; collisions refused; the card and its
   binding; approvals only in the register.
4. The same section: the catalog's signature, the key handling, rollback and
   expiry, revocation, and what a stolen catalog key allows (an entry with
   a matching hash that the person still has to approve).
5. Section 5, a new declared side channel: the choice among the entries of
   the catalog, at most about five bits per request, at most ten requests per
   session, each seen by a person; and the time of an approved download.
   Beside it the accepted leaks to the agent of point 2: the per-entry states
   of `GET /tools` and the one bit of `unavailable`.
6. Section 8: the audit kinds of point 8, and that reasons, logs and unknown
   names are stored only as hashes.
7. Section 9: the escape tests of item 5.5 (a request with a URL, Unicode or
   an unknown name causes no download and no lookup; the unpack sandbox has
   no interface besides loopback, no socket and resolves no name; an archive
   with `..`, an absolute link, a hard link, a device node, a set-uid file or
   a size bomb is refused; the agent cannot write the layer; one changed byte
   in a layer keeps it out of the session; a second session sees nothing of
   the first's layers), each with its mutation proof.
8. Section 11, the changelog entry.

`docs/THREAT-MODEL.md`:

1. Attack surface, a new row K-41 (the next free number when item 5.5 lands)
   "Tools on request", with the table of the design document, section 10:
   request content as a carrier, the reason, URL and index injection,
   install scripts, DNS, timing, oracles, typosquatting and compromised
   releases, persistence and shadowing, the tool at run time, self-approval
   through the repository, archive and parser attacks, the catalog key,
   rollback and freeze, approval fatigue, and the store on the host.
2. K-11 "Queue and rule metadata", extended: the agent learns the catalog and
   the state of its own requests, nothing about the person.
3. K-36 "The agent depot", extended: tool layers and the depot share no
   directory and no mount.
4. Residual risks, four entries: a tool that is what its hash says and still
   malicious (the main risk; sandbox and proxy contain it, nothing detects
   it); a person who approves without reading; other processes of the same
   user on the host, which can change the store as they can change the
   depot; and an outdated catalog on a machine that stays offline, which
   receives no revocation.

### 10. Out of scope for the first version

Distribution packages and their dependency resolution (Debian snapshots,
`.deb`, maintainer scripts); language registries as sources (PyPI, npm,
crates.io, Go modules); building from source and any micro-VM builder;
project dependencies (`npm ci`, `uv sync` into the project folder); mounting
into a running session; macOS and Windows; `zip` archives; approval for every
project, by name pattern or by company policy; a file in the repository that
lists or proposes tools; automatic updates of tools; vulnerability feeds
(OSV) and malware scanners; a transparency log with an outside witness; a
per-tool network profile; attributing a request of a tool to that tool in
the history; a Rust toolchain (its installer is a script). Each of these
needs its own decision.

## Rationale

- **A closed catalog makes the request nearly empty.** A name from a list of
  25 carries under five bits; a free package name carries as much as the
  agent wants and is read by the registry's logs. Refusing unknown names from
  memory leaves no lookup to observe.
- **The one door is enough.** `humanitl.internal` already exists, is already
  recorded and needs no new channel into the sandbox; a socket or a file for
  this would weaken guarantee 2 for a feature that sends 20 bytes.
- **Fetching ahead of time, by our own code.** A download caused by a request
  is a signal outside; a sync on the person's schedule is not. The fetcher
  runs no code it fetches, so it needs no sandbox, and the self-update has
  already shown how to do it with the TLS stack and `ring` we ship.
- **No network where foreign bytes are parsed.** The extractor reads hostile
  archives; if it is ever exploited, the exploit finds no door, no project
  and no secret. Sealing on the host again means the extractor is not trusted
  either.
- **The catalog pipeline reproduces the layer.** Comparing the sealed digest
  with the catalog's catches a broken extractor, a changed sealer and a
  manipulated archive with a colliding size in one comparison.
- **End of `PATH`, collisions refused.** A tool can then never replace
  `git`, `ssh` or the agent; refusing a collision instead of ordering it keeps
  "what runs is what you expect" true without the person reading the order.
- **Start-only mounting.** Mounting into a running sandbox would need mount
  propagation into a namespace the agent lives in, the place where escapes
  hide; one restart is the honest price of the first version (E9).
- **Deny by default, decision bound to a digest.** Fatigue is the largest
  residual risk named in the interviews. A focused "Don't install", a quota,
  a stop after three refusals and a card that cannot change under the
  person's eyes keep each approval a decision.
- **A separate key with the trust of the update key.** Whoever holds the
  update key can replace the whole daemon, so a catalog key held the same way
  adds no new root of trust; keeping it separate limits what a leak of one of
  them signs.

## Rejected alternatives

- **Cut the agent's network during an installation.** Protects nothing: the
  agent is not part of the installation, the installer is the channel, and a
  window of silence is itself a signal and breaks running requests.
- **Let the agent run `pip`, `npm` or `apt` against an allowed registry.**
  Names and paths become a channel, install scripts run next to the project
  and its secrets, and the registry is an open exfiltration target.
- **An installer sandbox with network to a mirror.** Install scripts still
  run with a network; the path of a request is still a channel.
- **Build from source in a micro-VM.** The safest way to run foreign build
  code, and far beyond one sprint; kept for after a measurement of which
  tools the catalog misses.
- **An image (erofs or squashfs) built by the extractor and mounted by the
  host kernel.** Puts a parser of attacker-shaped bytes into the host kernel.
  A tree checked and copied by the background service and bound read-only
  needs no image parser.
- **A new socket or file in the sandbox for tool requests.** Breaks
  guarantee 2; the meta host carries the same request.
- **The approval as a lock file in the repository.** The agent writes the
  project folder and would approve itself for the next session.
- **A per-tool trust signal (provenance, scanner, popularity) as the reason
  to approve.** Such signals prove origin or nothing; the card shows facts and
  states that a hash is not a verdict.
- **minisign, signify or Sigstore for the catalog.** Established, and each
  needs a format parser or a client the workspace does not have. Ed25519
  over the exact bytes with `ring` is the scheme the self-update already
  uses and verifies with code the project ships.

## Consequences

- New crate `humanitl-tools`, new binary `humanitl-unpack`, new profile
  `profiles/sandbox/tool-unpack.toml`, the catalog `catalog/tools.toml` with
  its pull request workflow, a signing step `packaging/release/sign-tools-catalog.sh`
  beside `sign-update.sh`, the repository secret, and the workspace entries
  for `tar` and `lzma-rs`. `backlog/CONVENTIONS.md` 3.1 gains the crate and
  the binary, 3.4 the sandbox paths `/opt/humanitl/tools` and
  `/usr/local/bin/humanitl-need`.
- The configuration gains `tools.enabled` and `tools.auto_sync` with their
  catalog rows; `PROJECT_CHOICE_KEYS` gains `tools.enabled`; the schema gains
  the direction `stricter` for it.
- The contract gains `Tools`, `ToolsSync`, the two `FlowEvent` variants, a
  minor version and the capability `tools`; the session status of ADR-0026
  point 1 gains the mounted tools (entry, version, layer digest).
- New diagnostics `TOOL_001` to `TOOL_012` with ARB titles and whys in `en`
  and `de` and their entries in `diagnostic_l10n.dart`; new audit kinds of
  point 8, shown by the export and `humanitl audit`.
- The glossary needs "tool" / „Werkzeug", "tool catalog" /
  „Werkzeugkatalog", "Tools on request" / „Werkzeuge auf Anfrage" and the
  action "Don't install" / „Nicht installieren" before item 5.4 puts them
  into the interface; "ablehnen" stays forbidden.
- ADR-0026 is amended in the commit of this decision: its two sentences
  "no new path on `humanitl.internal`" point to this record and name the four
  tool states, so the two accepted records do not contradict each other.
- Amendments the implementing issues make in the same commits: ADR-0014
  (the three paths in the table of point 3), ADR-0023 (the shared sealing
  module), ADR-0018 (parity rows), and ADR-0025: the two keys in their
  sections, a row in the table of point 6 for `tools.enabled` with
  `x-repo-direction = "stricter"` (only `false` applies, order `false` <
  `true`), and `PROJECT_CHOICE_KEYS` growing by one, which the count test in
  `daemon/crates/ipc/src/projects/tests.rs` (around line 188, comparing the
  saved choice with `PROJECT_CHOICE_KEYS.len()`) follows.
- Items: 5.1 catalog, signature, rollback state, store, fetcher,
  `humanitl tools sync` and the catalog workflow; 5.2 `ToolDesk`, the meta
  paths, quotas, `humanitl-need`, the briefing line, `Tools` and the
  `FlowEvent` variants; 5.3 `humanitl-unpack`, the unpack sandbox, the
  sealer, the layer binds, `PATH`, the collision and mount checks; 5.4 the
  card and the Tools page in the app, the TUI and the CLI, with screenshot
  and design check; 5.5 the shims, the escape tests and the statements of
  point 9.
- **Shims.** Item 5.5 places message-only programs named `apt`, `apt-get`,
  `dpkg` and `snap` first on `PATH` in sessions with tools switched on; they
  can never work in a sandbox and print a `Diagnostic` whose `fix` names
  `humanitl-need`. `pip` and `npm` are not shadowed, because installing into
  the project through the proxy is legitimate and every registry request is
  already held; the briefing line points the agent to `humanitl-need` for
  programs.
- Risk: a person approves a tool that is malicious and correctly hashed.
  Mitigation: the catalog's review and cooldown, the card's facts, and above
  all the sandbox and the proxy, which hold the tool to what the agent may
  do. Nothing in this decision detects it.
- Risk: the catalog misses what agents ask for, and people fall back to
  downloads through the proxy. The audit's unknown-name counts (hashes only)
  and the owner walkthrough decide what joins the catalog next.
- Doors kept open: mounting into a running session, distribution packages
  through a `ToolSource` port, a micro-VM builder, macOS through the Linux VM
  of the kernel level, a company list of pre-approved entries, an outside
  witness for the catalog, attribution of tool traffic in the history.

## Affected issues

GitHub #84 (this decision), building on `HUM-073` (the meta host),
`HUM-364` (nothing the agent reads names a person), `HUM-382` (the signed
self-update and its rollback guard), `HUM-443` (the control stream, which
stays unchanged), `HUM-477` (sandbox notices into the audit log), `HUM-518`
to `HUM-523` and `HUM-566` (the depot, its tree checks, signature and
checksum checks), `HUM-532` (the project register), ADR-0014, ADR-0015,
ADR-0017, ADR-0018, ADR-0023, ADR-0025 and ADR-0026. Implemented in phase 5
of sprint 9 as items 5.1 to 5.5 (see Consequences), Linux first.
