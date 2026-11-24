# ADR-0025 · Settings show five levels, a narrow session level and three presets, in nine sections

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-23

## Context

A setting in Humanitl can come from eight places, and a user who wants to
know what applies, or to change one value for one project or one session,
has to know which. The baseline of sprint 9 ([`UX-BASELINE.md`](../UX-BASELINE.md),
GitHub #78) counted them on `main` at `cc6dc233`:

- **Nine layers in the code** (`daemon/crates/config/src/load.rs`,
  `Origin::rank` in `origin.rs`): `Default` 0, `Global` 1 (`config.toml`),
  `ProfileBuiltin` and `ProfileGlobal` 2 (`profiles/*.toml`),
  `ProfileProject` 3 (`<project>/.humanitl/profile.toml`, HUM-066),
  `ProjectChoice` 4 (the daemon's project register, HUM-532), `Env` 5
  (`HUMANITL_*`), `Cli` 6 (flags, `Start.cli_overrides` and the fields of
  `Start`), `Policy` 7 (company locks, HUM-534).
- **Two project levels under one name.** The project profile lives in the
  repository and counts as influenced by an attacker; `x-project-scope`
  decides which 20 keys it may set, and `humanitl config set --project` and
  `humanitl llm server assign --project` write it. The project choice lives
  in the daemon, carries the 13 keys of `PROJECT_CHOICE_KEYS`, and the
  first-start screen, "Start choice…" and `humanitl settings --project`
  write it. Only `llm.server` and `sandbox.min_isolation` exist on both. A
  non-default `llm.server` from the repository needs a confirmation per
  folder (`CONFIG_022`, `humanitl llm server confirm`, stored in
  `project-servers.toml` in the daemon's data directory).
- **A session level that only the command line reaches.** Twelve keys
  travel with a start (`SESSION_OVERRIDE_KEYS` in
  `daemon/crates/ipc/src/session.rs` and six fields of `Start`); the app
  sends only folder mode and sandbox backend. Every other flag of
  `humanitl run` or `go` changes only what the command resolves locally.
- **A company level without a file.** `PolicyLocks::of_policy` and
  `SourceLock::of_policy` always return "no lock"; resolution, the origin
  `policy`, the read-only row and `AGENT_010` are built and tested.
- **Tiers that order nothing.** `x-tier` is `basic` for 10 keys, `advanced`
  for 50 and `expert` for 35 (ADR-0011); no client reads it. 61 of the 95
  keys have no control anywhere; `catalog/hidden.toml` gives each a reason.
  The catalog has ten categories (`CATEGORIES` in `catalog.rs`) and a
  boolean `next_session` per row.
- **Defaults that are not the most sealed value.** The baseline (section 5)
  lists fourteen keys and two behaviours, among them `sandbox.mode = "auto"`
  (falls back to writing directly with `SANDBOX_033`),
  `sandbox.min_isolation = "policy"` (the weakest of three levels),
  `sandbox.backend = "auto"` (never the kernel level), `ui.notifications =
  true` (host, method and path with Allow and Block),
  `hold.hard_block_checksum_secrets = false` and
  `tls.trust_system_store = true`.

The owner decided on 2026-11-23 (sprint 9, decisions E1, E6 and E11, and
the brief of #80):

1. Secure by default. The factory preset is the strictest. Anything that
   lets the agent do or reach more is opt-in, has a visible switch, shows
   its origin and leaves an audit record.
2. Presets are named Strict, Balanced and Open (Streng, Ausgewogen, Offen).
   (#80 called the third one "loose"; the owner chose "Open".)
   Strict is the factory default: working copy, only the chosen model server
   allowed, everything else waits for a decision, the highest isolation
   level available on the machine. No preset name promises more than the
   tool does.
3. One vocabulary ([`GLOSSARY.md`](../GLOSSARY.md), #79). The visible levels
   are Default, Mine (all projects), This project, Only this session and
   Company (locked). Environment and flags appear only as "overridden by …".
4. One visible project level. The repository file and the project choice
   both stay in the code and show as "This project"; a value from the file
   carries the mark "from the repository". No new project file.
5. Notifications show a count only by default; content and action buttons
   are opt-in.

"Item N.M" below is the numbering of the sprint's tracking issue on GitHub
(item 2.5 is the session level, for example); "#N" is a GitHub issue.

Hard constraints: the three sandbox guarantees, `tests/escape/` and the
terminal output filter stay as they are. An expired decision deadline
blocks, always. The isolation level is never lowered silently, and
`sandbox.min_isolation` is merged as the maximum over every level
(CONVENTIONS 4.11, ADR-0020). Every capability is an RPC first (ADR-0018);
the app, the settings TUI and the CLI render the daemon's catalog and decide
nothing themselves (ADR-0024 point 3). No new port (ADR-0015).

## Decision

We keep every layer of the code, add one for presets, and show users five
levels with marks and override hints; the session level carries four
settings, presets bundle the values that decide the agent's reach, and the
catalog orders all 95 keys into nine sections with a simple and an expert
view.

### 1. Visible levels

The daemon maps every `Origin` to one visible level and at most one mark.
Clients show the level and the mark; they never compute either.

| Visible level (en / de) | Origins in the code | Mark |
|---|---|---|
| Default / Standard | `Default`, new `Preset(name)` | "from preset NAME" / „aus Preset NAME" for a value the preset sets |
| Mine (all projects) / Meins (alle Projekte) | `Global`, `ProfileBuiltin(name)`, `ProfileGlobal(name)` | "from profile NAME" / „aus Profil NAME" for a profile's value |
| This project / Dieses Projekt | `ProjectChoice`, `ProfileProject(path)` | "from the repository" / „aus dem Repository" for a value from `.humanitl/profile.toml` |
| Only this session / Nur diese Session | new `Session` (the four keys of point 2, from a client's choice or their flag) | none |
| Company (locked) / Firma (gesperrt) | `Policy(by)` | the name of the policy |

- **New origin `Preset(name)`**, rank 1, between `Default` and `Global`;
  every rank above moves up by one. Its kind on the wire is `preset`.
- **`Cli` splits in two.** `Session` (kind `session`) is a value of one of
  the four session keys that a client chose for a start, by a control or by
  the flag that names it (`--llm NAME`, `--sandbox-min-isolation`,
  `--kernel`, `--backend`, `--disposable`, `--direct`, `--rw`, `--ro`,
  `--work-mode`, `--ask`). `Cli` stays for every other flag and for
  `--llm URL`. With `Preset` inserted the ranks become `Default` 0,
  `Preset` 1, `Global` 2, profiles 3, `ProfileProject` 4, `ProjectChoice` 5,
  `Env` 6, `Session` and `Cli` 7 (they share the rank of today's `Cli`;
  a key is either a session key or not, so they never compete), `Policy` 8.
  `Session` is shown as a level, `Cli` as a hint.
- **Environment and flags are hints, not levels.** Where `Env(name)` or
  `Cli` wins, the row shows the level the value would have without them and
  the line "overridden by environment variable `HUMANITL_…`" or "overridden
  by flag `--…`". A variable that reaches a start as a session wish
  (baseline 2.1, point 3) is shown the same way.
- **Profiles belong to Mine.** A session profile (`profiles/<name>.toml`,
  `--profile`) is a file in the user's configuration directory or shipped
  with Humanitl; its values show on the level Mine with the mark "from
  profile NAME", wherever the profile was chosen. Their rank is 3 (2 before
  `Preset` is inserted), still above `Global`. The
  expert chain says who chose the profile.
- **The full chain** is the expert view of a row: every layer that has a
  value for the key, from `Default` to `Policy`, with value, origin kind,
  file or flag, and which one won. `GetConfig` returns it per key
  (point 6).
- **Reset** removes the value of the shown level only, never of another
  one: on Mine the key leaves `config.toml`, on This project it leaves the
  project choice. A value from the repository cannot be reset from a client
  (the file belongs to the repository); the row names the file and line.

### 2. Session level

Exactly four settings can be chosen for one session:

| Setting | Keys | Rule |
|---|---|---|
| Model server | `llm.server` | one of the saved servers; an unknown name is `CONFIG_021` as today |
| Isolation level | `sandbox.min_isolation`, `sandbox.backend` | equal or stricter only: the floor only rises, and a backend below the effective floor is `SANDBOX_035` |
| Folder mode | `sandbox.mode`, `sandbox.work_mode` | Working copy, Direct or Read only (point 4) |
| Network mode | `hold.ask_mode` | Ask, Block or Decision proxy (point 3) |

- **Relaxations are allowed, never silent.** A session may choose Direct
  over the preset's working copy, or a model server other than the
  project's. Such a value shows as "Only this session" in the status of the
  session and in its header, and the start writes one `config.changed`
  record per session value with origin `session`. The isolation level can
  never be relaxed for a session.
- **Never stored.** A session value lives in the `Start` request and in
  the running session, nowhere else. The one exception is the action
  "Remember for this project" ("Für dieses Projekt merken"), which calls
  `Projects.save_choice` with exactly these values; the usual checks of the
  project choice apply (`CONFIG_024`).
- **Chosen before the start, no pending values.** A session's values are
  fixed when it starts: the passthrough rule, the key file and the mounts
  are built from them. There is no value that waits in the daemon for a
  later start. Changing a session value while a session runs (the settings
  sheet, the terminal view) means "restart this session with it": the
  client shows what the restart ends (the running agent; with a working
  copy, the changes that wait for review stay reviewable as today) and asks
  for a confirmation; only then does the daemon stop the session and start
  a new one with the new value, through the same `Start` and its checks.
  Cancelling changes nothing. A test holds that a start without session
  values carries none, whatever an earlier client chose.
- **The other start parameters are not settings.** The agent (`go AGENT`,
  `agent.adapter`), the project folder (`sandbox.work_dir`), `-- CMD`
  (`agent.command`), `--llm URL` (`llm.endpoint`), `--hold-timeout-secs`
  and `--podman-runtime` keep travelling with `Start` as today
  (`SESSION_OVERRIDE_KEYS` is not narrowed), but they are not offered as a
  session level in any view. Where they win, the row shows
  "overridden by flag".
- **The `Start` allow-list grows.** Today folder mode, network mode and
  model server travel in their own fields of `Start`. They stay there, and
  the daemon now reads `sandbox.mode`, `sandbox.work_mode`,
  `hold.ask_mode` and `llm.server` from those fields with origin
  `session`. That widens what a process on the socket may decide for one
  session: Direct is a relaxation any client can request. It stays behind
  the same socket boundary as every other start (CONVENTIONS 4.17), is shown
  in the status and header, and is audited as above.

### 3. Network mode

The visible setting "network mode" ("Netzmodus") is `hold.ask_mode`, with
one new value:

| Value | Shown as | Behaviour |
|---|---|---|
| `ui`, `terminal` | Ask: waits for a decision | as today; where you decide is a second detail, not a mode |
| `block` (new) | Block: no request waits | a request without a rule is blocked at once, without asking; the passthrough to the chosen model server and `allow` rules still apply |
| `upstream` | Decision proxy | as today (ADR-0022); only with `egress.proxy` |

Under `block`, a rule with the action `ask` is treated like no rule: its
request is blocked at once, because nobody is asked. The agent gets the
block response of ADR-0005 (`403`) with the note "network mode Block, no
rule allowed this request" (ADR-0014), not the `504` of an expired
deadline; the agent briefing (HUM-071) is changed to say so.

`none` (nobody is asked, every request runs into its deadline) becomes an
alias of `block`. It blocks the same requests, but at once and with `403`
instead of `504` after the deadline; a user who set `none` sees this in the
upgrade notice (point 11). The bundled profile `profiles/llm-only.toml`
moves from `ask_mode = "none"` with `timeout_secs = 1` to `ask_mode =
"block"`. There is no mode that allows a request without a rule, and none
will be added.

### 4. Folder mode

The visible setting "folder mode" ("Ordnermodus") replaces the two keys in
every view:

| Shown as | `sandbox.mode` | `sandbox.work_mode` |
|---|---|---|
| Working copy – review changes first | `disposable` | `rw` |
| Direct into the project folder | `direct` | `rw` |
| Read only | (not used) | `ro` |

`auto` is no longer offered. A file that sets `sandbox.mode = "auto"`
keeps working as today and shows "Working copy where possible, otherwise
direct" with the hint that Working copy refuses the start instead
(`SANDBOX_030`). `sandbox.work_mode` stays `rw` in every preset: the
working copy already keeps the project folder untouched until the changes
are reviewed, and an agent that cannot write cannot do its work.

### 5. Presets

A preset is a named bundle of values that decide the agent's reach. It is
the layer `Preset(name)` between Default and Mine, so every value the user
sets on any level beats it. The chosen preset is the new top-level key
`preset` in `config.toml` (`"strict"`, `"balanced"`, `"open"`; tier
`basic`; denied in the repository file; not in the project choice). Without
the key, Strict applies.

| Key | Strict (Streng), factory | Balanced (Ausgewogen) | Open (Offen) |
|---|---|---|---|
| `sandbox.mode` | `disposable` | `disposable` | `direct` |
| `sandbox.work_mode` | `rw` | `rw` | `rw` |
| `sandbox.min_isolation` | `highest` (new) | `policy` | `policy` |
| `sandbox.backend` | `auto` | `auto` | `auto` |
| `hold.ask_mode` | `ui` | `ui` | `ui` |
| `hold.hard_block_checksum_secrets` | `true` | `true` | `false` |
| `llm.unsaved_endpoint` (new) | `local_only` | `local_only` | `warn` |
| `rules.templates` (new) | `[]` | `["toolchain"]` | `["toolchain"]` |
| `agent.pi.web_access` | `off` | `auto` | `auto` |

What each preset means, in one sentence each (the catalog text):

- **Strict:** the agent writes into a working copy, runs at the highest
  isolation level this machine has, reaches only the chosen saved model
  server without asking, and every other request waits for a decision.
- **Balanced:** as Strict, but the isolation level is the platform's own
  without a virtual machine (Namespaces on Linux, Kernel policy on macOS),
  read requests to the package registries of the project's toolchain go
  out without asking, and pi may load its web extension.
- **Open:** as Balanced, but the agent writes directly into the project
  folder, and an allowed request may send an IBAN or card number unchanged.

The terms behind the new values:

- **`sandbox.min_isolation = "highest"`** resolves to the highest isolation
  level available on this machine. The daemon keeps the highest level it
  has started a session at, or that `humanitl sandbox check` proved, in its
  data directory (the machine ceiling); it only ever rises by itself.
  `highest` is the higher of the ceiling and the level the backends'
  availability check reports at the start. When the check reports less than
  the ceiling (for example `/dev/kvm` gone), the start is refused with
  `SANDBOX_035`, and the fix offers three explicit acts: repair the level,
  switch to Balanced, or reset the ceiling with `humanitl sandbox check
  --reset-ceiling` (also offered by `humanitl doctor`). The reset runs the
  check again, sets the ceiling to the level it proves now, says old and new
  level, and writes an audit record; it never runs as part of a start. A
  start never runs lower without one of these acts.
- **`sandbox.backend = "auto"`** picks the cheapest backend whose level
  meets the effective floor: QEMU under Strict where the kernel level is
  available, bubblewrap or Seatbelt otherwise. Today `auto` never picks the
  kernel level; under Balanced and Open it behaves as today.
- **An agent from the depot at the kernel level.** QEMU refuses an agent
  whose source is the depot (`AGENT_015`, `sandbox/src/qemu/mod.rs`). Under
  Strict on a machine with the kernel level, such a start is refused with
  `AGENT_015`, never moved to another backend or source. Its fix offers two
  explicit acts, each written with `config.changed`: take the agent from
  this computer for this project (`agents.<id>.source = "host"` in the
  project choice), or choose Balanced. The conflict is temporary: depot
  agents at the kernel level are their own issue in sprint 9 (see
  "Affected issues"), after which Strict needs neither act.
- **`llm.unsaved_endpoint`** decides a model server given by address for
  one session (`--llm URL`): `local_only` accepts loopback and private
  addresses and refuses every other one with `LLM_006` and the fix "save it
  with `humanitl llm server add` and choose it"; `warn` starts with the
  warning `LLM_006` as today.
- **`rules.templates`** switches on the shipped rule templates of item 3.6;
  `toolchain` means the templates of the toolchains detected in the project
  folder. Templates allow only reading methods; a writing request to the
  same hosts waits for a decision.
  *Trust boundary of the detection:* the project folder belongs to the
  repository and, in a session with a writable folder, to the agent, so
  which toolchain is detected is attacker-influenced. The detection may
  therefore only choose among the shipped templates; it never makes a rule,
  a host or a path. A template matches only fixed registry hosts and the
  path shapes of package downloads and index reads (a GET can carry data in
  its path and query), and a finding in the URL still makes the request
  wait. Detection runs once at the start of a session: a manifest the agent
  creates during the session widens nothing until the next start, and that
  start shows the detected templates in its status and writes them to the
  audit. `rules.templates` is denied in the repository file, so only the
  user's preset or own value switches templates on.

Rules for every preset:

- **The floor is still a maximum.** `sandbox.min_isolation` merges as the
  maximum over every level, the preset level included. A preset can set the
  floor that applies when no other level raises it; Mine, This project, a
  session and the company can only raise it. Leaving Strict lowers the floor
  only by the explicit choice of Balanced or Open (point 5, "Switching").
- **Not in any preset, ever:** a network mode that allows without a rule,
  allowing on an expired deadline, a floor below a project's, a session's
  or the company's, switching off the detection of findings
  (`findings.enabled`), switching off TLS inspection, sending anything
  home.
- **Not in the presets, because they are no matter of the agent's reach:**
  presentation (`ui.*`, `terminal.*`), notifications (point 8), storage
  and proof (`audit.*`, `recorder.*`), the decision proxy (`egress.*`,
  itself an opt-in), the source and credentials of agents (`agents.*`), and
  the cloud lock of agents and model servers, which item 2.7 decides. No
  preset confirms cloud inference on the user's behalf.
- **Kept as they are, outside the presets (owner, 2026-11-23):**
  `tls.trust_system_store` stays `true` in every preset, as today: the
  system's CA store counts at the target. `update.check` stays `true`: it
  is no reach of the agent, sends nothing about the user, and an outdated
  Humanitl is the larger risk; the first-start screen names it with its
  switch. `agents.source` stays `"host"`: the host program is mounted
  read-only and its configuration is never read (a fresh home), so the
  agent's reach equals the depot's; `agents.encrypt_credentials` and
  `agents.<id>.credential_writeback` stay as they are for the same reason.
- **Built-in defaults equal Strict.** `impl Default` of every key in the
  table carries Strict's value, so a reader that skips the preset layer
  still gets Strict. The one exception is `sandbox.min_isolation`, whose
  built-in default stays `policy`: it is the bottom of the maximum, and the
  preset raises it. A test holds the table and `impl Default` equal.
- **Switching.** Choosing a preset shows the delta first: every key whose
  effective value changes, old and new, and which of them let the agent do
  or reach more. The change is written only after a confirmation of that
  delta (by its digest, as `Projects.cleanup` does), lands in `config.toml`
  as `preset = "…"` with origin Mine, and writes `config.changed`. The
  first-start screen shows the preset as one line with "Change for all
  projects…".
- **Every value shows its preset.** A row whose value comes from the
  preset shows the level Default and the mark "from preset Strict".

### 6. Where every command writes

Every command and every view that sets a value for a project writes the
**project choice** in the daemon. Nothing in Humanitl writes
`.humanitl/profile.toml` any more; the file stays readable and is edited by
hand or by the repository.

| Command or view | Today | From now on |
|---|---|---|
| `humanitl config set KEY VALUE --level project` (`--project` stays as its alias) | the repository file | the project choice, through `SetConfig` with `level` and `project` |
| `humanitl llm server assign NAME --project` | the repository file, counts as confirmation | the project choice (`llm.server`); no confirmation needed, the choice is the human's |
| `humanitl llm server confirm NAME --project DIR`, the app's question before the start (`CONFIG_022`) | a confirmation in `project-servers.toml` | adopts the repository's value: writes `llm.server = NAME` into the project choice. A later change of the repository's value is shadowed by the choice and shown in the chain |
| `humanitl settings --project`, first-start screen, "Start choice…" | the project choice | unchanged |
| App settings, level switch "This project" (item 2.2) | – | the project choice |

To carry this, the allow-list of the project choice (`PROJECT_CHOICE_KEYS`)
grows by every key the repository file may set (`x-project-scope =
"allowed"`, 20 keys, two already on the list), to 31 keys. None of them
touches a guarantee: each one could already come from the
attacker-influenced repository file, and a value the human sets on the host
is strictly less exposed. `x-project-scope` keeps governing only the
repository file. The isolation floor in the project choice still only
rises, and a locked value is still `CONFIG_024`.

Migration of `project-servers.toml`, which holds several confirmed server
names per folder (`assign.rs`): on its first start after the upgrade the
daemon moves, per folder, **only the confirmation of the server the
repository file selects at that moment**, and only when that server is
confirmed and the project choice has no `llm.server` yet. It writes that
name into the project choice (registering the folder if needed, as
`save_choice` by folder does today) with one `config.changed`, and removes
that one entry. Nothing else moves:

- A folder whose repository now selects a server that is not confirmed
  (confirmed A, file now names B) gets nothing; its next start is refused
  with `CONFIG_022` as today, never started with A.
- The other confirmations of a folder stay in the file, unchanged, until
  their disposition is explicit. The project view lists them with "Adopt"
  (writes the project choice) and "Remove"; `humanitl llm server confirm
  --list` shows them. While a folder has such entries, a repository that
  later selects one of them asks with `CONFIG_022` like any unadopted
  value (point 6, "The repository file only tightens"), naming the earlier
  confirmation; the answer is the explicit disposition.
- The file is retired when it is empty.

A value the earlier `config set --project` wrote into a repository file
stays there and applies as "from the repository" within the rule below.

#### The repository file only tightens (owner, 2026-11-23)

From now on a value from `.humanitl/profile.toml` applies only when it is
equal to or stricter than the value of the levels below it (Default with
the preset, Mine). Each of the 20 keys the file may set, and its profile
`name`, gets a direction in
the schema, `x-repo-direction`:

| Direction | Keys | A value from the file applies when |
|---|---|---|
| `raise` | `sandbox.min_isolation` | it is at or above the floor below (as today) |
| `add` | `findings.user_terms` | always; the terms are added to the ones below |
| `lower` | `hold.timeout_secs`, `limits.hold_max_flows`, `limits.hold_max_bytes`, `limits.connect_timeout_secs`, `limits.header_timeout_secs`, `limits.body_timeout_secs`, `limits.max_client_connections` | it is at or below the value below |
| `stricter` | `ui.notifications` | it shows less (`off` < `count` < `content` < `actions`) |
| `adopt` | `llm.server` | only once a human copied it into the project choice (`CONFIG_022`, point 6 table) |
| `ignored` | `limits.hold_body_cap_bytes`, `limits.preview_cap_bytes`, `limits.max_decompress_ratio`, `limits.event_buffer` | never; the value is reported and not applied |
| `free` | `ui.language`, `ui.theme`, `ui.terminal_notices`, `agent.briefing.enabled`, `recorder.inline_max_bytes` | always; presentation and storage, no reach (the inline cap only moves a body between the record and a blob file, the stored total stays the same) |
| `embedded` | `name` (the profile the file selects) | it names a bundled profile; the embedded text is used, never a user file of that name (below) |

Why each direction is safe, key by key:

- `lower` holds only for pure resource and time limits: a shorter deadline
  or timeout ends a request earlier, fewer held flows or bytes make the
  proxy answer `503` or `413` sooner. None of them lets a request leave
  that would otherwise wait or be scanned; the worst a repository achieves
  is that its own requests fail.
- The four `ignored` keys are scan and delivery budgets, and either
  direction weakens something. A lower `limits.hold_body_cap_bytes` moves a
  body onto the unscanned `stream: true` path when the hard block is off; a
  lower `limits.preview_cap_bytes` or `limits.max_decompress_ratio` shows
  and scans less of a body (the attack `limits.recorder_max_body_bytes`
  documents, HUM-209); a lower `limits.event_buffer` makes clients miss
  events (`Lagged`); a higher value of any of them costs the daemon memory
  for every session. They stay settable on Mine and in the project choice.
  From the repository they are ignored with the warning below instead of
  `CONFIG_003`, because the file could set them until now and a repository
  that did must keep starting.
- `findings.user_terms` merges as a union across every level (Default,
  preset, Mine, a profile, the repository, the project choice), not top-wins like
  every other list (`load.rs`); it is the one list with this rule, so a
  term on This project adds to Mine's and never drops one. A term is
  removed only on the level that holds it. A term adds a finding and never
  takes one away: when findings at the same place are tidied, a
  checksum-confirmed finding (the kinds `hold.hard_block_checksum_secrets`
  blocks) is never removed because a user term overlaps it, from whatever
  level the term comes. Both stay, and the hard block sees the checksum
  finding. Test: a term that overlaps an IBAN in the body, with the hard
  block on and a rule that allows the request, and the request stays
  blocked with `HOLD_004`.
- `free` keys change what a person sees or how much of a body is stored
  inline, never what leaves or how the sandbox is built.

With these directions the file can make a session safer or leave it as it
is, never less safe; a schema test checks that every key the file may set
names a direction and that `lower` holds only keys of the register of
resource limits.

*The profile a repository selects.* The file's `name` may name only a
bundled profile (`resolve.rs`, `project_wish`). A profile carries keys and
rules of its own, so the rule for it is fixed here: a profile selected by
the repository file is always loaded from its **embedded** text; a user
file of the same name in `profiles/` applies only when a human selects it
(`--profile` for a session, or a project choice, written with
`config.changed`). Loading the embedded text is not enough on its own: a
profile the repository selects counts as part of the repository file. Every
key it sets passes the same check as a key of the file itself: a key with
`x-project-scope = "denied"` is ignored, a key with a direction applies only
when it is equal or stricter than the levels below, and a looser value is
ignored with the warning below. Its rules may only `block` or `ask`; an
`allow` rule in it is ignored with the same warning. Example: the bundled
`llm-only` sets `sandbox.work_mode = "rw"`; with Mine at `sandbox.mode =
"direct"` and `sandbox.work_mode = "ro"`, a repository that names
`llm-only` gets its block rule, while `sandbox.work_mode`,
`hold.ask_mode` and `agent.adapter`, which the file may not set, are
ignored, and the folder stays read-only. Tests: that case keeps `ro`; and every embedded profile, loaded
as if a repository had selected it, contributes no looser value and no
`allow` rule.

A looser value is not applied. The row shows it as "from the repository,
ignored: looser than your value" ("aus dem Repository, ignoriert: lockerer
als dein Wert"), and the load reports one warning with a new `CONFIG_*` code
(ARB title and why in `en` and `de`). It never refuses a start: a
repository must not be able to block one. `x-project-scope = "denied"`
stays as it is for every other key (`CONFIG_003`). A new key that the file
may set needs a direction, or the schema test fails. The cost: repositories
that raised a limit or set a longer decision deadline lose that effect after
the upgrade; the upgrade notice (point 11) lists each such value with
"Set for this project", which writes it into the project choice.

### 7. Information architecture

The catalog groups every key into nine sections, in this order. The table
names the groups; the exceptions are listed under it.

| # | Section (en / de) | Keys |
|---|---|---|
| 1 | Start and session / Start und Session | `preset`, `agent.adapter`, `agent.command`, `sandbox.work_dir`, `sandbox.mode`, `sandbox.work_mode`, `terminal.on_close` |
| 2 | Agents / Agenten | `agent.briefing.enabled`, `agent.pi.*`, `agents.*` |
| 3 | Model servers / Modellserver | `llm.*` |
| 4 | Isolation and reach / Isolation und Reichweite | `sandbox.min_isolation`, `sandbox.backend`, `sandbox.profile`, `sandbox.env`, `sandbox.podman.*`, `sandbox.qemu.*` |
| 5 | Network and rules / Netz und Regeln | `hold.ask_mode`, `hold.timeout_secs`, `rules.templates`, `egress.*`, `tls.*`, `resolver.*`, `limits.connect_timeout_secs`, `limits.header_timeout_secs`, `limits.body_timeout_secs`, `limits.max_client_connections`, `limits.hold_body_cap_bytes`, `limits.hold_max_bytes`, `limits.hold_max_flows`, and a row that opens the rules |
| 6 | Content and secrets / Inhalt und Secrets | `findings.*`, `hold.hard_block_checksum_secrets`, `pseudonyms.*`, `limits.preview_cap_bytes`, `limits.max_decompress_ratio` |
| 7 | Recording, storage and proof / Aufzeichnung, Speicher und Nachweis | `audit.*`, `recorder.*`, `limits.recorder_max_body_bytes`, `limits.registry_max_finished_flows` |
| 8 | Appearance and operation / Darstellung und Bedienung | `ui.*`, `terminal.prefix`, `terminal.direct_keys`, `terminal.bell_on_hold`, `terminal.splash`, `limits.event_buffer`, `update.check`, the rows "Scale" and "Keyboard shortcuts" |
| 9 | Company and diagnostics / Firma und Diagnose | `experimental.*`, the company level, the rows "Version", "Restart" and "config.toml", and a row that opens `humanitl doctor` |

- **Every key has a row.** A key without a control of its own shows its
  value, level, mark and the command `humanitl config set KEY … --level …`
  to copy. `catalog/hidden.toml` keeps only keys drawn by a special row
  (the agent sources inside the agents page, the server table).
- **Simple and expert.** The view "Simple" shows the keys with `x-tier =
  "basic"`; "All settings" adds `advanced` and `expert`. The three tiers
  stay in the schema, the clients show two marks. Search covers both views:
  a hit outside the simple view is shown there with the mark "expert".
- **Tier changes:** to `basic`: `preset`, `agent.adapter`,
  `sandbox.min_isolation`, `hold.ask_mode`, `llm.server`, `ui.theme`,
  `ui.notifications`. From `basic` to `advanced`: `sandbox.backend` (the
  level is the simple choice, the backend its second detail),
  `llm.endpoint` and `llm.models` (servers are managed in the server list).
  The simple view then has 14 keys.
- **Search text** per row: title, help, keywords (as today), the key path,
  the display names of its values, and the section. The catalog provides it
  in English and German; clients filter, they do not index.
- **When a change takes effect** replaces the boolean `next_session` with
  three values: `now` (presentation and notifications), `next_session`
  (every key a session is built from: section 1 to 4, `hold.ask_mode`,
  `hold.timeout_secs`, which a running session keeps from its start
  (`ipc/src/sandbox.rs`), `rules.templates`,
  `hold.hard_block_checksum_secrets`) and `restart` (keys
  the daemon reads once: `limits.*`, `audit.*`, `recorder.*`, `resolver.*`).
  The implementing issue derives the value per key from the register of
  readers (`tests/config_readers.rs`). The row says "from the next session"
  or "after a restart of the background service" next to a changed value.

### 8. Notifications

`ui.notifications` becomes a choice: `off`, `count` (the number of waiting
requests, no content), `content` (host, method and path) and `actions`
(content with Allow and Block; Allow stays absent for a request with a
finding). Default `count`. A file with `true` reads as `actions`, `false`
as `off`: the user who wrote it chose what it did then. The chain of item 3.5
builds on this type.

### 9. Company level

While no policy file exists, the company level is not shown: no column, no
empty row, no badge. The expert chain ends with one line "No company
policy" ("Keine Firmenvorgabe"), and `humanitl config explain KEY` says the
same. Once a lock exists, the row shows the level Company (locked), the
policy's name, the control read-only, and `SetConfig` and `config set`
refuse with `AGENT_010` as today. Where the policy file lives, how it is
signed and distributed, is a later issue (BACKLOG section 9, item 21).

### 10. The CLI rule

`humanitl config set KEY VALUE` always names the level it wrote to:

```text
sandbox.mode = direct (This project: /home/me/src/app)
  overridden by environment variable HUMANITL_SANDBOX__MODE in this shell
```

- `--level mine` writes `config.toml`, `--level project` writes the project
  choice of `--project DIR` (default: the current folder). There is no
  `--level session`: a session value is given at the start.
- Without `--level`, at a terminal, the command asks which level and
  preselects Mine; without a terminal, with `--json` or with
  `--non-interactive` it refuses with a new `CONFIG_*` code whose fix names
  the command with `--level mine`. No default level is guessed in a script.
- When a higher level or the environment beats the written value, a second
  line says so. A write to a locked key is `AGENT_010`.
- `config set` writes through `SetConfig` when the daemon answers; without
  the daemon, `--level mine` writes `config.toml` locally as today, and
  `--level project` is refused with the diagnostic that the daemon is not
  running.

### 11. Consequences for the contract and the clients

- **`GetConfig` catalog entry** gains: `section`, `simple` (bool),
  `effect` (`now`, `next_session`, `restart`), `level` (one of the five),
  `mark` (preset, profile or repository with its name), `overridden_by`
  (env name or flag), `chain` (the layers of point 1), `session_capable`,
  `project_capable`, `can_reset`. `GetConfig` with a `preset` preview returns
  the values as that preset would resolve them, with the delta and its
  digest.
- **`SetConfig`** gains `level` (`mine`, `project`), `project` and, for
  `preset`, the delta digest. Every successful write emits
  `config.changed` (audit) with the level and `FlowEvent.config_changed`
  (clients reload).
- **`Start`** gains the origin `session` for the four session settings;
  the status of a session (#81) carries each with its level.
- **App** (item 2.2): the settings show a level switch "Mine (all
  projects) | This project", the nine sections, "Simple | All settings", search, per row
  value, level, mark, override hint, reset and lock. The start screen and
  the session sheet show the four session settings and "Remember for this
  project".
- **Settings TUI** (item 2.3): the same rows from the same catalog; the project
  view writes every key of the project choice, not five.
- **Config file:** new keys `preset`, `rules.templates`,
  `llm.unsaved_endpoint`; new values `sandbox.min_isolation = "highest"`,
  `hold.ask_mode = "block"`; `ui.notifications` becomes a choice with the
  boolean aliases of point 8.
- **Upgrade for users who never chose.** On the first start after the
  upgrade, the daemon compares the old built-in defaults with Strict for
  every key that has no value on any level the user wrote (only `Default`
  in the chain). Strict applies at once. The next start in the app and in
  the terminal shows one notice listing exactly those keys, old and new
  value, and the conflicts below, with two actions:
  - "Keep Strict" writes `preset = "strict"`; the notice does not return.
  - "Keep the previous values" is **one preset transition**, not a set of
    single writes, because the floor is a maximum and a value in Mine
    cannot undo Strict's `highest`. The daemon picks the strictest preset
    whose floor admits the effective level the user had before (Balanced
    when Strict's `highest` lies above it, otherwise Strict). It then
    compares the **full resolution** of that preset with the **full
    pre-upgrade resolution** the daemon kept on its first start: every key,
    including the implicit values of keys that did not exist before (a new
    key resolves to what makes the old behaviour, for example
    `rules.templates = []`, `llm.unsaved_endpoint = "warn"`). For every key
    that differs it adds the old value in Mine (for example `sandbox.mode =
    "auto"`, `rules.templates = []`, `ui.notifications = "actions"`).
    It resolves the result once more and shows the whole remaining delta.
    The action is offered for confirmation only when that delta is empty,
    or when every remaining difference is named in the dialogue with the
    reason it cannot be restored. For the floor the delta is empty: the
    old built-in floor was `policy`, which is Balanced's. The user confirms that delta by its digest (point 5,
    "Switching"), and every write leaves `config.changed`.

  The notice also names behaviour that changes without a key of its own:
  `hold.ask_mode = "none"` now blocks at once with `403` instead of `504`
  after the deadline (point 3), and repository values that are now ignored
  (point 6).

  The notice returns at every start until one of the two actions is taken.
  Without a terminal (no TTY, `--json`, `--non-interactive`, CI) a start
  prints no dialogue: it prints one Diagnostic line, a warning with its own
  `CONFIG_*` code, whose fix is `humanitl config diff --since-upgrade`, and
  it prints that line at every start until an action is taken, through
  `humanitl config set preset strict --level mine` or the "keep" command
  that `config diff --since-upgrade` names.

  A user who set a value keeps it. Conflicts are named in the notice and
  refuse the start until resolved, each with explicit fixes and never with
  a fallback:
  - an explicit `sandbox.backend` (Mine or a project choice) below the new
    floor: `SANDBOX_035`; fixes: choose Balanced, or remove the backend;
  - an agent from the depot on a machine with the kernel level:
    `AGENT_015`; fixes: this agent from this computer for the project, or
    choose Balanced (point 5, "An agent from the depot at the kernel
    level");
  - a repository value now ignored as looser (point 6): no refusal, a line
    with "Set for this project".

  `humanitl config diff --since-upgrade` lists the same. Nothing is migrated
  silently.

### What stays out

A new project file in the repository; the company policy file, its
signature and its distribution; presets per project (a project tightens
single keys); presets of the user's own (session profiles cover them);
several sessions per daemon (#85).

## Rationale

- **Five levels without moving a layer.** The visible levels are a mapping
  of the existing `Origin` values, so the resolution, its tests and the
  security argument for each layer stay as they are. Only two changes touch
  the chain: `Preset` is a new layer at the bottom, and `Cli` is split by
  meaning, not by rank. A regression is caught by the existing load tests
  plus one test per new origin.
- **The project choice as the one write target** closes the confusion the
  baseline found (W2, W9: two places named "project") and removes the only
  path on which Humanitl writes into a folder the agent can write. The
  confirmation of `CONFIG_022` becomes an ordinary project value, which is
  stricter than today: a repository that changes its server after the
  confirmation no longer reaches the session.
- **The repository file only tightens** because it is the one layer an
  attacker can write (a cloned repository, an agent with a writable
  folder). Today the per-key `x-project-scope` keeps the worst keys out,
  but a repository can still move every limit it may set, in either
  direction. With a
  direction checked per key (point 6: `lower` only for resource and time
  limits, the scan budgets ignored, profiles only embedded) the file can
  make a session safer or leave it as it is, never less safe, and ignoring
  instead of refusing keeps it from blocking a start.
- **A narrow session level** keeps `Start` an allow-list (ADR-0024,
  `session.rs`): the four settings are those a person changes between two
  sessions of one project (baseline W2 and W7). A value for one session that
  were stored would silently become a project value.
- **The preset as the bottom layer** gives a user's own value the last
  word without exceptions, and it lets the floor stay a pure maximum: the
  preset sets the floor that applies when nobody raises it. A preset above
  Mine would overwrite the user's own choices with a bundle; a preset as
  built-in defaults only could not show "from preset Strict" or be switched
  as one act.
- **`highest` with a machine ceiling** turns "the highest level available"
  into something checkable that never sinks: a probe that fails once would
  otherwise lower the level for that session, which is exactly a silent
  lowering. Whether the ceiling rule is right is checked by a test that
  removes the kernel level's availability after a recorded kernel start and
  expects `SANDBOX_035`.
- **Balanced lowers the floor to the platform's base level** because the
  kernel level costs extra packages, memory and a longer start (GLOSSARY,
  isolation levels); a user who wants faster starts has one explicit,
  audited act for it instead of a per-key exception to the maximum rule.
- **`block` as a network mode** answers the survey's "never ask me" without
  a mode that allows: requests without a rule fail fast instead of waiting
  for a deadline nobody watches.
- **Tiers stay three, marks become two**, because the schema's tiers are
  data that 95 keys already carry; the simple view needs only one cut. The
  seven promotions are the settings of the first-start screen, the preset,
  notifications and theme, which the baseline's workflows W1, W2, W4 and W7
  touch first.

## Rejected alternatives

- **Show every layer as its own level** (nine rows: default, global,
  profiles, project profile, project choice, env, flags, policy). It is what
  the code does, and it is what the baseline found unusable: two levels
  named "project", and environment and flags that are not set in any view.
  The full chain stays available in the expert view.
- **Write the repository file from every project command** (keep
  `config set --project` as it is, move the first-start screen there). It
  would give teams a shared file, but it makes Humanitl write into the
  folder the agent writes, keeps `CONFIG_022`, and contradicts E11. The
  owner ruled out a project file in this sprint.
- **Presets as shipped session profiles** (`profiles/strict.toml`).
  Profiles rank above Mine, so a preset would beat the user's own values,
  and they carry rules and are chosen per session, not per user.
- **Presets per project.** A project choosing Open would lower the floor
  through the project level, which CONVENTIONS 4.11 forbids for any project
  layer. A project tightens single keys; the preset is a choice of the user.
- **Let Mine lower the floor below the preset.** It would break the rule
  "every level only raises the floor" that the glossary, ADR-0020 and the
  project choice rely on, for a case Balanced already covers.
- **`sandbox.min_isolation = "kernel"` as the factory floor.** It refuses
  every start on macOS and on Linux without KVM; "highest" is the owner's
  wording and checkable per machine.
- **`sandbox.work_mode = "ro"` in Strict.** The baseline lists `ro` as the
  most sealed value, but a read-only agent cannot do its work, and the
  working copy already keeps the project folder unchanged until review.
- **A default level for `config set` in scripts.** Writing Mine when no
  level is given is today's behaviour and the reason a value lands on a
  level nobody meant; the owner asked for the level to be required.
- **Show the company level as an empty row.** Single users do not want to
  see a "company" they do not have (survey, "Policy unumgehbar vs.
  Einzelnutzer"); the expert chain says that there is none.

## Consequences

- The factory default changes for existing users: working copy refuses
  instead of falling back, the kernel level where it is installed, the hard
  block for checksum-safe secrets on, notifications as a count, looser
  repository values ignored. The upgrade notice (point 11) makes the
  change visible once and offers the previous values in one confirmed
  preset transition.
- Until depot agents run at the kernel level, a depot user on a machine
  with KVM has to make one explicit choice under Strict (`AGENT_015`).
- Humanitl no longer writes `.humanitl/profile.toml`. The write path with
  `O_NOFOLLOW` (`assign.rs`) is retired; `project-servers.toml` is retired
  once its last entry is adopted or removed; `CONFIG_022` stays as the
  question "adopt the repository's server for this project?".
- **`docs/SECURITY.md` and `docs/THREAT-MODEL.md` change together** in the
  implementing issues: the project profile section (no write by the daemon,
  the confirmation now an adoption into the project choice, the file only
  tightens with `x-repo-direction`), the project choice section (31 keys),
  `sandbox.min_isolation = "highest"` and the machine ceiling,
  `hold.ask_mode = "block"`, `llm.unsaved_endpoint`, `rules.templates`
  under Balanced and Open (read requests to package registries leave
  without waiting for a decision; the toolchain detection reads the
  attacker-influenced project folder, with the trust boundary of point 5),
  the growth of what a `Start` may decide (`sandbox.mode`,
  `sandbox.work_mode`, `hold.ask_mode` and `llm.server` read as session
  values; Direct is a relaxation a client on the socket can request), the
  profile a repository selects (embedded text only, a user file of the same
  name only by a human's selection), the scan budgets ignored from the
  repository, the new defaults, and the row of HUM-405 in the table of
  protections. This ADR changes neither file.
- Amendments that the implementing issues make in the same commits:
  ADR-0024 point 1 (the project choice grows to 31 keys and becomes the
  only project write target; the confirmation of `CONFIG_022` becomes an
  adoption), `backlog/CONVENTIONS.md` 4.4 and 4.23 (the order of layers with
  `Preset` and `Session`, ranks 0 to 8 as in point 1), and the statement of
  the defaults in `README.md` (working copy, `highest`, hard block on,
  notifications as a count).
- The glossary gains "folder mode" / „Ordnermodus", "network mode" /
  „Netzmodus" and the three preset names with their sentence; the
  terminology sweep (item 1.1) carries them.
- `docs/CONFIG.md` (generated) gains the column "Section" and the new
  keys; ADR-0011's statement that the tiers drive the settings screen
  becomes true.
- The catalog RPC grows by the fields of point 11; ADR-0024 point 3 stays
  the frame. A client that ignores the new fields still renders a correct,
  if poorer, row.
- Risk: the machine ceiling can lock a user out after hardware changes
  (KVM switched off in firmware). Mitigation: the fix of `SANDBOX_035` names
  the three acts (repair, Balanced, the audited `humanitl sandbox check
  --reset-ceiling`), and `humanitl doctor` explains the ceiling and offers
  the reset.
- Doors kept open: a policy file can lock `preset` like any key; a later
  project file can reuse the mark "from the repository"; several sessions
  per daemon only multiply the session level.

## Affected issues

GitHub #80 (this decision), building on `HUM-066` (project profile),
`HUM-405` (confirmation per folder), `HUM-532` (project choice), `HUM-534`
(catalog and company locks), `HUM-357` (`sandbox.mode = "auto"`) and
ADR-0024. Implemented in phase 2 of sprint 9; "item N.M" in this file is the
numbering of the sprint's tracking issue: item 2.1 catalog in nine sections,
item 2.2 app settings, item 2.3 settings TUI, item 2.4 `config set` with its
level, item 2.5 session level, item 2.10 presets, item 2.11 first start.
Two new issues of sprint 9 follow from this decision: the repository file
only tightens (`x-repo-direction`, point 6), and depot agents at the kernel
level (point 5). Item 3.5 builds on point 8, item 3.6 on `rules.templates`,
#81 on the levels in the status of a session.
Three rules of point 6 are already in place on `main`: the profile a
repository selects loads its embedded text (#90), follows the project scope
of each key (#95), and an overlapping finding never removes a
checksum-confirmed finding (#96).
