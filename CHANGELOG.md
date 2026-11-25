# Changelog

All notable changes to Humanitl are listed here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Before
1.0.0, a minor version may change the configuration, the protocol or the rule
format; a patch version never does.

Releases before 1.0.0 are not signed. Each release carries `SHA256SUMS`, which
detects a damaged download but not a tampered release; see
[`docs/guides/verify-download.md`](docs/guides/verify-download.md). Signed
releases come with 1.0.0.

The release notes on GitHub for a version start with its section from this
file (`packaging/release/changelog-section.sh`); the pre-releases 0.0.x have no
section here and list their commits instead.

## [Unreleased]

## [0.7.0] - 2026-11-25

Agents that use a model server no longer start without one, and an agent
can use its own providers when you ask for it: OpenCode with OpenCode Zen,
including its free models. A new cloud lock keeps inference on this machine
or network for people who want only local models. A saved model server in
your own network can be marked as local. The protocol (1.63) and the
configuration gain fields, so this is a minor version; read the upgrade
notes.

### Added

- `agents.<id>.inference` for every agent: `model-server` (default) sends
  inference only to the model server Humanitl chose, `own` lets the agent
  use its own providers where Humanitl knows them. Today that is OpenCode
  with OpenCode Zen: only its inference paths are passed through, `POST`
  only, behind the cloud notice and its confirmation. The setting is
  user-level; a project cannot switch it (#112).
- `llm.cloud = "block"` keeps inference on this machine or network. The
  strictest level wins: a project or a session can switch the lock on, never
  off. Under the lock a start whose inference would leave this machine is
  refused with `LLM_014`, whatever was confirmed before; no passthrough to a
  non-local host is created; a non-local saved server cannot be added or
  chosen; `humanitl doctor` shows the lock and the level that set it.
  `docs/SECURITY.md` lists every connection Humanitl itself makes next to
  the lock (#113).
- `humanitl llm server add|edit NAME --local[=false]` and the checkbox "Own
  network" on a server card mark a saved server in your own network whose
  name looks public. The mark counts for the cloud notice and the cloud
  lock, drops when the address moves to another origin, and a project can
  never set it (#115).

### Fixed

- OpenCode and pi without a model server were configured against the
  proxy's own port, so the agent started and every prompt failed with a
  connection error. Such a start is now refused with `LLM_012`, which names
  the setting to fix; an agent set to `own` without own providers in
  Humanitl is refused with `LLM_013` (#112).
- A saved server with `local = true` made the configuration fail to load
  with `CONFIG_002`; the loader now accepts the mark (#115).

### Security

- A profile that a project's profile selects by the name of a bundled
  profile always loads the bundled text, never a file of the same name in
  the user's configuration directory (#90).
- A profile that a project's profile names follows the project scope of
  each key, like the project's own profile (#95).
- An overlapping finding, such as a user term, never removes a finding that
  a checksum confirmed, so `hold.hard_block_checksum_secrets` blocks it as
  intended (#96).
- The decision surfaces in the app, the CLI and the terminal show the
  A-label of a host (`xn--...`), with the Unicode form only beside it, as
  `docs/SECURITY.md` states for look-alike names (#103). In
  `humanitl flows --json` the field `host` now carries the A-label;
  `authority.host` and `authority.display_host` are unchanged.

### Upgrade notes

- Restart the daemon after upgrading:
  `systemctl --user restart humanitld.service`, `humanitl daemon install`
  for archive and AppImage installs, or `humanitl daemon restart` after
  replacing the macOS app. A CLI that needs a newer daemon names the
  restart (`CLI_004`).
- OpenCode and pi now need a model server to start
  (`humanitl llm server add`); without one the start is refused with
  `LLM_012`. To use OpenCode with OpenCode Zen instead, run
  `humanitl config set agents.opencode.inference own` and confirm the cloud
  notice at the next start; under `llm.cloud = "block"` that start is
  refused with `LLM_014`. pi has no own providers in Humanitl yet
  (`own` is refused with `LLM_013`), so it needs a model server.
- Scripts that read `host` from `humanitl flows --json` now get the
  A-label of an internationalized name (#103).

## [0.6.0] - 2026-11-23

A start window in the app and `humanitl go` as the one command to start an
agent in a folder: a first-start screen proposes agent, source, folder mode
and network, a closed terminal leaves the session running, and a second
`go` attaches to it. In the session's terminal a prefix key (`Ctrl+B`) opens
the settings or the held requests over the running agent and brings the
agent back afterwards. A fix makes the isolation verdict independent of the
client that started a sandbox. The protocol (1.62) and the configuration
gain fields, and the prefix key changes its default, so this is a minor
version; read the upgrade notes.

### Added

- A start window in the app with the running sessions, recent projects, a
  new project, cleanup and settings, and a first-start screen fed by the
  daemon's proposal for a folder without a stored choice.
- `humanitl go [AGENT]` starts an agent in the current folder. In a new
  folder it shows a first-start screen (Enter or Space starts with the
  defaults, `a` customizes, Esc cancels); the choice is stored per project.
- The agent's source (host or agent depot) is part of the project choice.
  Only a source that is set nowhere is preselected, to the one place where
  the agent is ready (the host when it is ready in neither); a source from
  the configuration, the environment or a stored choice is kept.
- `humanitl app [PATH]` opens the app with that project preselected.
- `humanitl settings`, a settings TUI in the terminal with the same catalog,
  origins and locks as the app. API keys and proxy credentials are entered
  hidden and go to the daemon through their own requests, never through
  the configuration file or the command line.
- A prefix key in the session's terminal, `ctrl-b` by default
  (`terminal.prefix`), with a fixed table: `d` detaches, `,` opens the
  settings, `i` the held requests, `?` the table. Under the kitty keyboard
  protocol `Ctrl+,` opens the settings directly (`terminal.direct_keys`).
  Pasted text is never read as a key.
- Settings and interceptor views over the running agent. The agent keeps
  running; its output is held as whole filtered chunks up to 4 MiB and the
  agent redraws on return. The interceptor decides only the selected request
  after an explicit key, never selects by itself and refuses decisions while
  the view does not fit the terminal.
- A bell on a new held request (`terminal.bell_on_hold`, default on), at
  most once per second and never inside the agent's output.
- `terminal.on_close`: closing the terminal of `go` detaches (`detach`,
  default) or stops (`stop`) the session this `go` started. `humanitl
  sessions` and the start window show a session without an attached
  terminal.
- A getting-started guide (`docs/guides/getting-started.md`).

### Changed

- The prefix key's default is `ctrl-b` instead of `ctrl-a`, like tmux; the
  old name `terminal.detach_prefix` still works. Whoever never set the key
  sees a one-time note.
- `humanitl run` keeps stopping its session when its terminal closes;
  `humanitl go` detaches. A `go` that only attached never stops a session.
- A second start in another folder (`CLI_005`) names `humanitl go --dir` or
  `humanitl sandbox attach` as the way to the running session.
- Notices of the daemon are no longer written into the agent's terminal
  when it is a terminal; recordings to a file or pipe keep them.
- The rule targets of `humanitl moderate` and the terminal views state
  their real scope (any query, any method, http/https/ws/wss, any port, no
  WebSocket upgrade).

### Fixed

- After a hangup nothing is written to the closed terminal any more; a
  `run` whose terminal closed could panic on `EIO` before it stopped its
  session.

### Security

- The isolation verdict of a starting sandbox no longer depends on the
  client that started it. Before, a client that closed its start stream
  after the start and before the three guarantees were reported could leave
  a sandbox with a red or missing isolation result running unproven. The
  daemon now evaluates the verdict and stops the sandbox on red before it
  sends anything; the finding is kept in the session summary. A second
  kill attempt never signals a process that was already reaped, so a
  reused process id on the host is never hit. The kill
  budgets per backend are in `docs/SECURITY.md` and the threat model
  (K-15).
- `docs/SECURITY.md` and the threat model (K-39) state that the held
  requests of a detached session wait for a decision or their deadline;
  an expired request is blocked, never allowed.
- `url` 2.5.9 (RUSTSEC-2026-0592).

### Upgrade notes

- Restart the daemon after upgrading:
  `systemctl --user restart humanitld.service`, `humanitl daemon install`
  for archive and AppImage installs, or `humanitl daemon restart` after
  replacing the macOS app. A CLI that needs a newer daemon names
  the restart (`CLI_004`).
- The prefix key is now `Ctrl+B`. To keep `Ctrl+A`, run
  `humanitl config set terminal.prefix ctrl-a`.
- Closing the terminal of `humanitl go` no longer stops the agent. To get
  the old behavior, run `humanitl config set terminal.on_close stop`.

## [0.5.0] - 2026-11-21

Claude Code, Codex and the Antigravity CLI as agents, an agent depot that
installs and logs in agents inside a sandbox instead of on the host, and a
project register that keeps each folder's sessions and a stored project
choice. The passthrough for the model API now pairs each method with its
paths, and the audit log records failed requests and the events it lost
when it fell behind; a failed request lost in such a gap is counted, not
recorded. The protocol (1.57) and the
configuration gain fields, and a few values the daemon used to accept are
now refused, so this is a minor version; read the upgrade notes.

### Added

- Claude Code (`agent.adapter = "claude"`), Codex (`agent.adapter = "codex"`)
  and the Antigravity CLI (`agent.adapter = "agy"`) as built-in agents next
  to OpenCode and pi, each with its own home on a tmpfs, updates and
  telemetry turned off where the vendor documents a switch, and the sandbox
  CA. An API key from the keyring reaches the agent through a file and a
  small launcher, never through its command line or a variable of the plan.
  Codex and agy run with their own sandbox off, because it cannot start
  inside Humanitl's; the briefing tells the agent so. Codex from the depot
  reads the depot's copy of its `config.toml`, which does not yet carry
  `sandbox_mode = "danger-full-access"`; until it does, commands of the
  agent can fail, and `humanitl agents config codex` sets the key. agy keeps
  its own login in the system keyring, which never enters a sandbox, so it
  runs with a Gemini API key only (`humanitl llm key set`).
- Agent depot: `humanitl agents install|update <agent> [--version V]`
  downloads an agent in a maintenance run inside a sandbox, with no project,
  where only the recipe's hosts pass without a hold and every other
  request is held for your decision; the daemon checks the result itself
  and switches to the new version atomically, and a failed run leaves the
  depot as it was. Claude Code comes from `downloads.claude.ai`, its signed
  manifest verified with the host's `gpg`; Codex as the vendor's whole
  package from `releases.openai.com`, checked against the vendor's checksum
  list (not signed); see the note on Codex's `sandbox_mode` above. agy,
  OpenCode and pi have no install recipe (`AGENT_015` names why).
  `humanitl agents login`, `agents callback` and `agents config` run a
  login or a configuration run of the depot program; only the credential
  file comes back, through a guard that checks its format, size, account
  and expiry. `humanitl agents login claude` answers
  `AGENT_014` until the vendor's credential format is measured, so Claude
  Code from the depot runs with an API key or a gateway. Install, update,
  login and configuration runs are refused under the kernel level and on
  macOS (`AGENT_015`). See
  [`docs/guides/agent-depot.md`](docs/guides/agent-depot.md).
- `agents.<id>.credential_writeback` (default on) lets a running session
  hand a refreshed credential file back to the depot, through the same
  guard; `agents.encrypt_credentials` (default off, closed to project
  profiles) encrypts the depot's credential files with a key kept only in
  the system keyring (`AGENT_020` without one). The daemon decrypts them for
  every session.
- The app's settings gain the category "Agents": host or depot per agent,
  installed version and signature, install, update, log in, configure and
  remove, the write-back and encryption switches.
- Project register: the daemon keeps each project folder with its sessions
  and a stored project choice, shown with
  `humanitl projects ls|show|clean|forget|proposal`. The daemon can store a
  choice (`Projects.save_choice`), but neither the CLI nor the app saves one
  yet. `clean` and `forget` show a plan of everything they would delete and
  ask first, never touching the project folder or the audit log; `proposal`
  shows the options and defaults a first start of a folder would offer. A
  project choice never lowers `sandbox.min_isolation` (`CONFIG_024`).
- Settings catalog: the daemon describes every setting the app shows, with
  its category, texts, origin and lock; settings kept in the file only, such
  as `limits.registry_max_finished_flows`, are left out. The app's settings
  are built from it, and
  `humanitl config catalog [--project PATH] [--language en|de]` prints it.
- One install script for every system:
  `curl -fsSL https://humanitl.de/install | sh` (Debian and Ubuntu through
  the APT repository, Fedora and RHEL through the `.rpm`, Arch through an
  AUR helper, other Linux through the tarball into `~/.local`, macOS
  through the `.dmg`). It checks every download before the first `sudo`,
  shows its plan and asks once before it runs the steps (`--yes`,
  `--dry-run`), and on macOS never removes
  the quarantine: you allow the app with "Open Anyway". aarch64 Linux and
  Windows are not covered.
- `curl -fsSL https://humanitl.de/uninstall | sh` removes Humanitl the way
  it came: it finds the package, the archive of the install script or the
  macOS app, stops while an agent session runs, removes the background
  service with `humanitl daemon uninstall`, then the program and, on its own
  question, the APT source. Your data stays unless `--purge` is given, which
  lists every location with its size and asks first.
- `humanitl config paths` prints the directories that hold your data, the
  list `--purge` deletes from.
- New audit record kinds: `flow.failed` (a request whose way to the target
  failed, with its `upstream_*` reason and nothing of the request),
  `audit.events_dropped` (the audit sink fell behind and lost events; counts
  only; a failed request lost in such a gap leaves no `flow.failed`, only
  this count), `sandbox.door_capped` (the kernel level's connection cap was
  hit), `agent.credential_writeback` (with the SHA-256 of both files, never
  their content) and `projects.cleaned`. The kernel level's host check now also
  lands in the audit log as `isolation.check`.
- `limits.registry_max_finished_flows` (default 10,000, closed to project
  profiles) bounds the finished requests the daemon keeps in memory; held
  and running requests never count.
- `humanitl doctor` gains the line `rules`, which names every rule of
  `rules.yaml` that could not be read.
- `SANDBOX_055`: the kernel level's vsock device stopped answering, named
  instead of the generic `SANDBOX_013`.

### Changed

- The passthrough for the agent's model API pairs each method with its
  paths instead of allowing every listed method on every listed path:
  Claude Code POST on `/v1/messages` and `/v1/messages/count_tokens`; Codex
  POST on `/responses` and GET on `/models`; agy POST on
  `models/{id}:generateContent` and `:streamGenerateContent` and GET on the
  model list; OpenCode and pi POST on the inference endpoints and GET on
  `/v1/models` and Ollama's `tags`, `ps` and `version`, and both methods
  only on paths you add with `llm.passthrough_paths`. The security
  statement only narrows.
- One unreadable rule in `rules.yaml` no longer discards every rule: the
  others apply, and the bad entry is reported with field, line and reason
  (start log, Rules screen, `humanitl rules list`, `humanitl doctor`) and
  kept in place on every write. It fails closed, in two cases. If the
  error lies elsewhere (`expires`, `id`, an unknown key) and the rule's
  action and match can be read, the rule keeps its action with exactly its
  match, so a `block` still blocks. If the action or the match cannot be
  read, a `block`, `ask` or `redact` rule becomes `ask` for its host, or for
  every host and address when the host cannot be read. Only an unreadable
  `allow` rule is left out. A file that cannot be read as a whole (YAML,
  top-level key, `version`, `disabled_bundled`) is still rejected entirely.
- When the decision proxy or the target behind it closes the connection
  without an answer, the agent's 502 says so (reason
  `upstream_proxy_closed`) instead of reporting a Humanitl block with
  `upstream_connect`. A proxy's own answer, such as Burp's 200 page for a
  dropped request, still cannot be told apart from a relayed one.
- `resolver.test_ca` must be an absolute path whenever it is set, with or
  without `--allow-test-ca`; a relative path now stops the daemon from
  starting (`CONFIG_012`) instead of being ignored with a warning.
  `humanitl config set` and the settings in the app now refuse such a value
  before writing, and so a `resolver.nameserver` that is no address or
  `address:port` (`CONFIG_003`), which already stopped the start. They
  refuse any other key while the file holds a value the daemon cannot start
  with, except the repair of that very key. A URL with a reserved example
  host (such as `https://api.example.com/v1`) is written with a warning
  that it will not answer without `resolver.overrides`.
- A start without a project folder is refused with `SANDBOX_053`; the
  daemon no longer falls back to its own working directory.
- `humanitl run` and `humanitl go` send work, ask and sandbox mode only when
  given on that call, so the folder's project choice applies otherwise.
  `--sandbox-min-isolation` (also from the environment or a `--config`
  file) reaches the daemon and can only raise the floor.
- `humanitl sandbox plan` and `humanitl projects proposal` apply the same
  session values and project choice as the start, so the preview shows the
  plan the start would take or refuses as the start would. The project
  choice also applies to `humanitl sandbox image pull`.
- In an interface language other than English, the app shows the
  translated cause of a finding; the daemon's English sentence stays
  available under "Original message (untranslated)".
- A suggested fix that changes a setting always carries a value
  `humanitl config set` accepts; where only you can supply the value, the
  app opens the field instead.
- The kernel level's helpers run only their own programs: its unit
  executes nothing outside a list of files, and each helper's jail refuses
  new user namespaces, `execveat` and executable memory files (the
  emulator keeps its guest memory). The
  bubblewrap prerequisite of the kernel level now asks for
  `--disable-userns`, `--remount-ro` and `--seccomp`.
- A disposable session on the kernel level lays the profile's masks over
  the project inside the guest as the namespace level does, and the layer's
  file service accepts only `user.overlay.*` extended attributes.
- Dates and times are handled by `jiff` instead of `chrono`. Stored data
  keeps its format; error texts for a time the CLI cannot parse change, and
  instants beyond the year 9999 show as `-`.

### Fixed

- On macOS, a name that the case-insensitive volume folds to a masked path
  (a Kelvin sign for `k`, for example) no longer escapes the mask of a
  disposable clone.
- The kernel level removes its `humanitl-vm-*` directories under
  `$XDG_RUNTIME_DIR` when a session ends and sweeps those of ended daemons
  at start.
- The flow registry and the audit queue no longer grow without bound over
  a long session; a full audit queue makes the sender wait, it never drops
  a record.
- The history no longer loses a request that arrived while a page was
  loading.
- `humanitl doctor` shows `virtio_fs` as not measured when the kernel image
  is missing, instead of repeating the kernel line's failure.
- The settings' backend row shows a stale list as pending after a project
  change, gives reasons and commands the full row width, and keyboard focus
  and Tab keep working while the backend choice is locked.

### Security

- The README's side-channel table and `docs/SECURITY.md` name the depot's
  credential file as a declared side channel; the threat model (K-36)
  covers the depot's maintenance runs, the write-back guard and the
  encryption, whose limit is that the daemon decrypts for every session.
- A Codex package is bound to its checked archive by the maintenance run,
  not by the daemon, which has no tar reader of its own yet; documented in
  `docs/SECURITY.md` and K-36.
- The kernel level still lets code that a taken-over helper already runs in
  its own memory do what that helper may do; documented in K-33.
- `proxy_resolve = "humanitl"` binds only a decision proxy that connects to
  the address in `CONNECT`; Burp Suite resolves the name again, so the Burp
  guide recommends `"proxy"`. The guide now rests on a trial with Burp Suite
  Community Edition 2026.11.2.
- `doctor-probe.json` stores a hash of the stored probe start instead of
  the program's path, which could name a private folder.
- `chrono` (RUSTSEC-2026-0543, unmaintained) is no longer a dependency, and
  its exception is gone.

### Upgrade notes

- Restart the daemon after upgrading:
  `systemctl --user restart humanitld.service`, `humanitl daemon install`
  for archive and AppImage installs, or `humanitl daemon restart` after
  replacing the macOS app. A CLI that needs a newer daemon names
  the restart (`CLI_004`).
- A relative `resolver.test_ca` now stops the daemon from starting. Fix it
  with `humanitl config set resolver.test_ca /absolute/path` or remove the
  key.
- A `rules.yaml` with an unreadable entry now loads its readable rules,
  where 0.4 discarded the whole file. Check `humanitl doctor` (line
  `rules`): until the entry is repaired, an unreadable rule whose action
  and match can be read keeps its action, one whose action or match cannot
  be read holds its host (or every host) for a decision, and a change that
  would put a rule above it is refused with `RULES_009`.
- The narrower passthrough holds a request that 0.4 passed only because
  its method was allowed on another listed path, such as POST on
  `/v1/models` with OpenCode or pi. If an agent needs such a request, add
  its path to `llm.passthrough_paths` or decide it with a rule.
- A start without a project folder (`SANDBOX_053`), such as the app opened
  from a launcher, now needs a folder: run `humanitl go` or `humanitl run`
  in the folder, or pass `--work DIR`.
- The kernel level needs a bubblewrap with `--disable-userns`,
  `--remount-ro` and `--seccomp`; `humanitl doctor` names a missing one.
  Empty `humanitl-vm-*` directories left by earlier versions carry no lock
  and are never swept; remove them once with
  `rmdir /run/user/$(id -u)/humanitl-vm-*`.
- The recorder database gains the project register and is migrated on the
  first start.

## [0.4.1] - 2026-11-17

Sessions start again under the packaged user unit. A patch release: no
change to the configuration, the protocol or the rule format.

### Fixed

- Under the packaged user unit of 0.4.0, every session the daemon started
  failed with `SANDBOX_049` (mount table differs from the plan), and the
  kernel level reported its modules missing with `SANDBOX_045`. The unit's
  `ProtectKernelModules=yes` hid `/usr/lib/modules` behind a mount that
  bubblewrap carried into the sandbox. The unit no longer sets it; the
  module system calls stay denied (`SystemCallFilter=~@module`).
  `humanitl sandbox check` from a terminal was not affected (HUM-529).
- `SANDBOX_049` names the mounts outside the plan and no longer suggests
  `sandbox.backend = auto`, which could not help; `SANDBOX_045` suggests a
  reboot when an upgrade removed the running kernel, instead of installing a
  package that is already there.
- `humanitl doctor`, `humanitl run` and `humanitl go` print a finding once,
  not twice.

Upgrading from 0.4.0: update the package, then run
`systemctl --user daemon-reload && systemctl --user restart humanitld.service`.
Archive and AppImage installs run `humanitl daemon install` instead. An
override `ProtectKernelModules=no` made by hand can be removed with
`systemctl --user revert humanitld.service`.

## [0.4.0] - 2026-11-17

A decision proxy of your own behind Humanitl, a one-command start, and a
clear notice when a cloud model receives what the agent reads. The protocol
and the configuration gain fields (protocol 1.43), so this is a minor
version.

### Added

- `humanitl go`: one command starts a disposable session and lands in the
  agent's terminal, with a short spinning-fist splash while the sandbox
  starts (`terminal.splash`).
- Decision proxy (opt-in): allowed requests can leave through an
  intercepting proxy of your choice, such as mitmproxy, instead of going
  direct (`egress.proxy`, `egress.proxy_ca`, `egress.proxy_auth_ref`,
  `egress.proxy_resolve`, `egress.proxy_response_timeout_secs`). By default
  the proxy resolves names and Humanitl sends `CONNECT host:port`, still
  refusing local names and private addresses before the proxy;
  `proxy_resolve = "humanitl"` resolves and checks in Humanitl first.
  Configure it in the app (Settings › Decision proxy) or with
  `humanitl egress status|check|auth`. Without `egress.proxy` nothing
  changes.
- Ask mode `upstream` (`hold.ask_mode = "upstream"`, needs `egress.proxy`):
  a request no rule decides goes to that proxy unheld, and the proxy
  decides. Block rules stay final, `redact` rules still hold in Humanitl,
  and a request with a recognised secret never reaches the proxy while
  `hold.hard_block_checksum_secrets` is on. History shows such requests as
  "Forwarded" with the proxy's answer; `humanitl sessions` and
  `humanitl sandbox plan` show who decides.
- Guide: mitmproxy as the decision proxy behind Humanitl, tried with
  mitmproxy 12.3.1
  ([`docs/guides/decision-proxy-mitmproxy.md`](docs/guides/decision-proxy-mitmproxy.md)).
- Cloud inference notice: when the agent's model runs at a provider outside
  this machine, Humanitl says once per agent and provider that what the
  agent reads goes to that provider, asks before the first start, and keeps
  an orange tag at the top of the session, an orange line in the CLI and a
  `humanitl doctor` row. Local endpoints get none; mark a model server as
  local with `llm.servers.NAME.local = true`.

### Changed

- The security statement in the README now says that every request is
  approved by a rule, by you, or by your decision proxy, and names the
  limits of the proxy mode in `docs/SECURITY.md` and the threat model.
- The recorder database gains a column for proxy decisions; it is migrated
  on the first start.
- The app refuses to start a session against a daemon older than this
  version and names the restart.

### Security

- Website dependencies: `astro` 7.4.12 fixes GHSA-r3h5-4585-7x3w and
  GHSA-mh32-jxcr-mh9f. humanitl.de is a static site, so neither applied to
  it.
- `chrono` was reported as unmaintained (RUSTSEC-2026-0543), with no known
  vulnerability; it is accepted until 2027-01-17 while the move to `jiff`
  is prepared.

## [0.3.0] - 2026-11-15

Fixes from a review of the whole tool, most of them in the kernel level. The
protocol gains one field (`ImagePull.plan`), so this is a minor version.

### Fixed

- Kernel level: input over the terminal stream travels under credit and is
  never dropped when the guest reads slowly; a shell whose open failed is
  closed and its process group killed in the guest after a short grace;
  window size changes and closes no longer wait on a busy stream.
- `humanitl sandbox image pull` and the app pull the image for the backend
  of the session (`--backend`, the project profile), not always the
  configured one; the fix the CLI proposes repeats the reference and the
  backend.
- The host limits of a backend (CPUs, memory) are checked when that backend
  starts, not when the configuration loads, so a setting meant for another
  machine no longer breaks every start.
- Setting fixes propose values without TOML quotes, in the form
  `humanitl config set` takes.
- `humanitl doctor` and the backend list answer within the app's deadline;
  a backend still being checked says so (`DOCTOR_012`) instead of timing out.
- A cached doctor probe no longer outlives a change of its inputs: another
  kernel image, kernel config, module list, program or runtime directory
  probes again.
- The app keeps the backend section in place while the list for a newly
  chosen backend loads, instead of collapsing it.

## [0.2.0] - 2026-11-15

Two more sandbox backends on Linux, each chosen by a person and never by
`auto`, which stays bubblewrap.

### Added

- The kernel level: `sandbox.backend = "qemu"` or `--backend qemu` runs the
  agent in a QEMU micro VM with a guest kernel of its own, from Debian
  packages only (`qemu-system-x86`, `virtiofsd`, `vhost-device-vsock`, which
  the packages suggest). The three guarantees are held on the host and stay
  intact with root in the guest: one vsock door to the proxy, no network
  device, the checked project folder pinned by descriptor. A host check
  inspects every process, descriptor, socket and share of the session's unit
  before the agent starts; the unit has fixed limits for descriptors, memory
  and tasks. Disposable sessions work in the guest. It is the only backend
  that meets `sandbox.min_isolation = "kernel"`. Admitted after the escape
  suite with root in the guest was green on `main` in ten consecutive CI runs
  with KVM; see `docs/SECURITY.md`, section 14.
- Podman as a second backend at the plain level (rootless, `crun`), started
  through the user's service manager, optionally from an image pinned by
  digest (`humanitl sandbox image pull`, cancellable, credentials only from a
  file of the adapter).
- `sandbox.backend` and `sandbox.min_isolation` (`policy`, `namespaces`,
  `kernel`): the strictest floor over all layers wins, a chosen backend never
  falls back to another one.
- `humanitl sandbox backends` lists each backend with its isolation level and
  availability; the app chooses the backend in the settings and per start,
  with its level and the reason when it cannot run.
- `humanitl doctor` has lines per backend, one line per missing prerequisite,
  and an opt-in probe start (`--probe`).

### Changed

- The shim compares the mount table and the checked folders with the plan
  before the agent starts, under every backend.
- `sandbox argv` and `-v` withhold hidden values unless asked.
- A disposable sandbox is refused, and `auto` runs direct, when the project
  folder folds names by case (casefold, vfat, exFAT, HFS, cifs), so a masked
  path cannot be reached under another spelling.
- The earlier Podman runtime `krun` is gone; a configuration that names it is
  refused when it loads (`CONFIG_023`).

### Fixed

- Podman hides the host's `/sys` and ignores the user's `mounts.conf`, like
  bubblewrap; a container whose client ends without a stop is removed at once.

## [0.1.0] - 2026-11-11

The first release. Compared with the 0.0.x pre-releases, from the first public one, 0.0.1, to the
last, 0.0.21:

### Added

- macOS (Apple Silicon and Intel): a Seatbelt sandbox, the daemon as a launchd
  agent, a menu bar item and notifications for held requests, and a disk image
  and a zip in every release.
- The disposable sandbox: the agent works on an overlay of the project folder;
  the app and the CLI show the diff, apply selected changes or discard them. It
  is the default where the system supports it (`sandbox.mode = "auto"`).
- pi as a second agent next to OpenCode, with web search and fetch through the
  proxy; both agents learn that a held request is slow on purpose and do not
  give up while it waits.
- Several saved model servers, shown as a list with the chosen server's
  settings beside it: one default plus assignments per project or session
  profile, `humanitl run --llm NAME`, and a one-time confirmation for a
  project that picks a server other than the default.
- One project folder, chosen once, shared by the app and the CLI.
- Updates from inside the app on Linux and macOS, from a daily check of the
  GitHub releases (`update.check`, on by default), with an Ed25519-signed
  manifest; APT and dnf installations update through their package manager.
- Packages: an AppImage, an RPM for Fedora and openSUSE, the AUR package
  `humanitl-bin`, and a `.deb` with a socket-activated, hardened user unit
  that replaces a running daemon on update.
- Intercept: pause before sending while findings are open, pause the whole
  intercept from the header, and an "Edited" marker on changed requests in the
  queue and the history.
- Audit: a screen with chain status, record table, export and retention,
  `audit.max_bytes`, and a trail of findings that were sent on purpose.
- Sandbox terminal: several shells as tabs, a second shell in a running
  sandbox, `humanitl sandbox attach` that detaches with Ctrl+A d, copy and
  paste, zoom, full window, and a floating or docked terminal.
- The daemon can trust the system certificate store towards destinations.
- Resource limits and backpressure in the proxy.
- Settings as a searchable overlay, with a "Keyboard shortcuts" category that
  lists every shortcut; German and English; the whole app scales with
  Ctrl+Plus, Ctrl+Minus and Ctrl+0.
- `humanitl config`, `humanitl audit` and `humanitl daemon` commands;
  `humanitl daemon install` sets up, refreshes and removes the service in one
  step.

### Changed

- New visual design: neutral greys, a yellow accent, orchid for held requests,
  one layout grid, a single title bar, and a short onboarding instead of the
  setup screen.
- Saved settings take effect without restarting the daemon.
- With the hard block on, a request carrying a checksum-confirmed secret is
  held and leaves only edited, after a second scan finds no such secret in the
  edited version; `Allow` on the unchanged request is refused with `HOLD_004`.
- Release notes are in English and releases are regular GitHub releases, no
  longer marked as pre-releases.

### Fixed

- OpenCode reaches a model on loopback through the reserved name
  `llm.humanitl.internal`, and pi 1.x reaches a local model through a CONNECT
  tunnel that carries cleartext HTTP.
- Model endpoints behind a path prefix work; pasted endpoints and keys are
  cleaned up.
- Ctrl+C no longer quits OpenCode in the terminal; a lone modifier key no longer
  types a stray character.
- `humanitl daemon install` checks the systemd unit directory and sets aside a
  unit of its own that would hide the package's unit.
- The app shows "reconnecting" during a short daemon restart instead of an
  error, and a flow held before the first resync still reaches the queue.
- A few bytes of non-standard large-window brotli in a body no longer make the
  proxy reserve up to 1 GiB of memory; such a body is reported as not searched.

### Security

- PID 1 in the sandbox is the filtered, non-dumpable shim, and the shim closes
  descriptors the launcher leaked before it forks.
- The project folder is checked at every start and bound by descriptor, so it
  cannot be swapped between check and bind.
- Dot segments in a path can neither widen an `allow` rule nor slip past a
  `block` rule.
- Findings scan every gzip member and flag deflate bodies whose zlib and raw
  readings differ.
- A project profile can neither switch off the hard block nor shrink the
  recorded body; a rule suggestion that carries a masked secret is withheld.
- On macOS, the agent cannot replace its adapter files or reconfigure OpenCode
  from the project, and the session directory with its access key ends with
  the sandbox.
- The `/tmp` fallback for the runtime directory is checked for owner and mode.
- Dependency updates for security advisories in pulldown-cmark, tokio, rustls,
  rustls-webpki, webpki-roots, ctutils, rusqlite (bundled SQLite 3.54.1) and
  rustix.
