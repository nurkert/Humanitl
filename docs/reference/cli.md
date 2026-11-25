# CLI reference

Applies to: Humanitl 0.1

<!-- Generated from daemon/bin/humanitl/src/cli.rs by `cargo xtask docs`.
     Do not edit by hand: change the clap definitions and rerun the task. -->

`humanitl` is a thin client of the daemon (ADR-018): every subcommand calls an RPC of
`humanitld` or reads a local file, and formats the answer. This page is the `--help` text
of every command, generated from the code. What the commands do in detail, step by
step, is described in [`docs/cli.md`](../cli.md); which RPC each subcommand serves is in
[`parity.md`](parity.md).

## Commands

- [`humanitl run`](#humanitl-run) Start a session in the current directory, or show the session a profile resolves to
- [`humanitl go`](#humanitl-go) Start an agent in a project folder and land in its terminal; afterwards choose what to keep
- [`humanitl sandbox`](#humanitl-sandbox) Start, plan and check the sandbox
- [`humanitl sandbox run`](#humanitl-sandbox-run) Start a sandbox with CMD as the agent and exit with its exit code
- [`humanitl sandbox argv`](#humanitl-sandbox-argv) Print the bwrap command line for the current profile without starting it
- [`humanitl sandbox check`](#humanitl-sandbox-check) Start a short-lived sandbox and show the three isolation guarantees
- [`humanitl sandbox backends`](#humanitl-sandbox-backends) List the sandbox backends of this system: the isolation level each offers, whether a start with it would pass, which one sandbox.backend chooses, and what is missing
- [`humanitl sandbox plan`](#humanitl-sandbox-plan) Show what a start in this project would get, without starting: profile, project folder, mode, and the saved model server with the place its choice comes from (this run, the project, a profile or llm.active)
- [`humanitl sandbox attach`](#humanitl-sandbox-attach) Attach this terminal to the running session
- [`humanitl sandbox exec`](#humanitl-sandbox-exec) Open a further shell in the running sandbox, next to the agent
- [`humanitl sandbox changes`](#humanitl-sandbox-changes) Show what a disposable run changed: files changed, created and deleted, with a diff for text files and size and hash for others
- [`humanitl sandbox apply`](#humanitl-sandbox-apply) Write the changes of a disposable run to the project folder and discard the rest
- [`humanitl sandbox discard`](#humanitl-sandbox-discard) Drop all changes of a disposable run. Asks first
- [`humanitl sandbox image`](#humanitl-sandbox-image) The image a container sandbox starts from, when its root is an image
- [`humanitl sandbox image pull`](#humanitl-sandbox-image-pull) Pull an image into the local storage of the container engine, pinned by its digest
- [`humanitl rules`](#humanitl-rules) List, add, change, reorder, dry-run and reload rules
- [`humanitl rules list`](#humanitl-rules-list) List the rules in the order in which they are evaluated
- [`humanitl rules add`](#humanitl-rules-add) Add a rule. The daemon checks it and answers with the whole set
- [`humanitl rules update`](#humanitl-rules-update) Change one rule. What is not given stays as it is
- [`humanitl rules remove`](#humanitl-rules-remove) Remove one rule
- [`humanitl rules reorder`](#humanitl-rules-reorder) Move one rule to another place inside its own group
- [`humanitl rules dry-run`](#humanitl-rules-dry-run) Show which of the last recorded requests a rule would have matched
- [`humanitl rules disable`](#humanitl-rules-disable) Switch a bundled rule off. It stays in the list and decides nothing
- [`humanitl rules enable`](#humanitl-rules-enable) Switch a bundled rule back on
- [`humanitl rules reload`](#humanitl-rules-reload) Read rules.yaml again and report what changed
- [`humanitl rules test`](#humanitl-rules-test) Evaluate one URL against the rule set; exit 10 block, 11 ask
- [`humanitl flows`](#humanitl-flows) The flows of this and earlier sessions
- [`humanitl flows list`](#humanitl-flows-list) List flows, newest first; --asc turns the order around
- [`humanitl flows show`](#humanitl-flows-show) Show one flow, or one of its bodies
- [`humanitl flows decide`](#humanitl-flows-decide) Decide one waiting flow: allow it or block it
- [`humanitl sessions`](#humanitl-sessions) The session and who decides a request without a rule; with a subcommand, what the agent left in the project during a sandbox run
- [`humanitl sessions summary`](#humanitl-sessions-summary) Show what a sandbox run changed in the project
- [`humanitl audit`](#humanitl-audit) Verify and export the audit chain
- [`humanitl audit verify`](#humanitl-audit-verify) Check the hash chain: every record, its predecessor and the anchors
- [`humanitl audit export`](#humanitl-audit-export) Write the records to a file, as JSON lines or as CSV
- [`humanitl config`](#humanitl-config) The resolved configuration and its schema
- [`humanitl config get`](#humanitl-config-get) Print the resolved value of one key, or of every key without one
- [`humanitl config set`](#humanitl-config-set) Write one key into config.toml, after checking it against the schema
- [`humanitl config schema`](#humanitl-config-schema) Print the JSON schema of the configuration, or the list of profiles
- [`humanitl config edit`](#humanitl-config-edit) Open config.toml in $VISUAL or $EDITOR and check it afterwards
- [`humanitl config catalog`](#humanitl-config-catalog) Print the settings catalog of the running daemon: categories, titles, controls, origin and lock of every setting, the source the app and the settings TUI draw from
- [`humanitl config paths`](#humanitl-config-paths) Print where Humanitl keeps your data: configuration, recordings, the project register, the agent depot and the audit chain
- [`humanitl daemon`](#humanitl-daemon) The background service
- [`humanitl daemon status`](#humanitl-daemon-status) Ask the daemon who it is
- [`humanitl daemon install`](#humanitl-daemon-install) Write the systemd user unit that starts the daemon at login
- [`humanitl daemon logs`](#humanitl-daemon-logs) Show the journal of the daemon: journalctl --user -u humanitld
- [`humanitl daemon restart`](#humanitl-daemon-restart) Restart the background service: launchctl kickstart on macOS, systemctl --user restart on Linux
- [`humanitl daemon uninstall`](#humanitl-daemon-uninstall) Stop and disable the service and remove the unit daemon install wrote
- [`humanitl daemon retire-if-idle`](#humanitl-daemon-retire-if-idle) Ask the daemon to end now if no agent session holds it (after an upgrade)
- [`humanitl doctor`](#humanitl-doctor) Check this machine: one line per precondition, with a fix
- [`humanitl update`](#humanitl-update) Check for a newer Humanitl and install it
- [`humanitl update check`](#humanitl-update-check) Ask GitHub now whether a newer release exists
- [`humanitl update apply`](#humanitl-update-apply) Install the newer release this installation can take
- [`humanitl llm`](#humanitl-llm) Find LLM servers in the local network
- [`humanitl llm discover`](#humanitl-llm-discover) Search the local /24 for OpenAI-compatible servers and Ollama
- [`humanitl llm test`](#humanitl-llm-test) Ask one endpoint what it is: which API answered, its models, how long
- [`humanitl llm key`](#humanitl-llm-key) The access key of the LLM endpoint, kept in the system keyring
- [`humanitl llm key status`](#humanitl-llm-key-status) Whether a key is stored, and where: system keyring or 0600 file
- [`humanitl llm key set`](#humanitl-llm-key-set) Store the key. It is read from stdin, never from the command line; at a terminal the input is hidden
- [`humanitl llm key remove`](#humanitl-llm-key-remove) Remove the key and its reference from the configuration file
- [`humanitl llm server`](#humanitl-llm-server) Saved model servers: endpoint, models and access key per entry
- [`humanitl llm server ls`](#humanitl-llm-server-ls) List the saved servers; the active one is marked
- [`humanitl llm server add`](#humanitl-llm-server-add) Save a server. Its key follows with `humanitl llm key set --server NAME`
- [`humanitl llm server use`](#humanitl-llm-server-use) Make a server the active one, from the next session on
- [`humanitl llm server rename`](#humanitl-llm-server-rename) Rename a server. Its key stays where it is
- [`humanitl llm server rm`](#humanitl-llm-server-rm) Remove a server and, unless --keep-key, its key from the keyring
- [`humanitl llm server edit`](#humanitl-llm-server-edit) Change the address or the models of a saved server without making it the default. Its key stays and goes only to its own address
- [`humanitl llm server assign`](#humanitl-llm-server-assign) Let a session profile or a project use a saved server. A project's choice made here counts as confirmed for that folder
- [`humanitl llm server unassign`](#humanitl-llm-server-unassign) Remove the server a session profile or a project uses; it then uses the default
- [`humanitl llm server confirm`](#humanitl-llm-server-confirm) Confirm the saved server the project in a folder chooses, so that a session there starts with it
- [`humanitl intercept`](#humanitl-intercept) Pause intercepting, resume it, or show whether it is paused
- [`humanitl intercept status`](#humanitl-intercept-status) Show whether intercepting is on or paused, and since when
- [`humanitl intercept pause`](#humanitl-intercept-pause) Pause intercepting until you resume it, the next session starts or the daemon stops
- [`humanitl intercept resume`](#humanitl-intercept-resume) Resume intercepting: every request no rule decides waits again
- [`humanitl egress`](#humanitl-egress) The decision proxy behind Humanitl (`egress.proxy`): show it, check the connection, store its credentials
- [`humanitl egress status`](#humanitl-egress-status) Show the decision proxy as config.toml names it, the way out the running daemon takes, and whether credentials are stored
- [`humanitl egress check`](#humanitl-egress-check) Open a tunnel through `egress.proxy` to a fixed target (example.com:443) and the TLS handshake in it, as config.toml says now. Sends no request, so nobody has to decide anything at the proxy
- [`humanitl egress auth`](#humanitl-egress-auth) Store or remove the credentials of the decision proxy
- [`humanitl egress auth set`](#humanitl-egress-auth-set) Store USER:PASSWORD in the system keyring and set `egress.proxy_auth_ref`. Read from stdin, never from the command line; at a terminal the input is hidden
- [`humanitl egress auth remove`](#humanitl-egress-auth-remove) Remove the credentials and `egress.proxy_auth_ref`
- [`humanitl agents`](#humanitl-agents) The agent depot: where each agent comes from (host or depot), its installed version and login; set the source, install or update an agent in a maintenance sandbox, remove a depot
- [`humanitl agents status`](#humanitl-agents-status) Show every agent, or one: its source with origin or company lock, the installed version and signature, whether it is logged in, its size. Nothing from the depot is started for this
- [`humanitl agents source`](#humanitl-agents-source) Set where an agent comes from: `host` or `depot` (`agents.<id>.source`). Switching back to `host` keeps the depot
- [`humanitl agents remove`](#humanitl-agents-remove) Remove the depot of an agent with its program, configuration and login; refused while a session uses it. The source stays as it is
- [`humanitl agents install`](#humanitl-agents-install) Install an agent into the depot in a maintenance sandbox: no project, only the vendor's hosts without a hold, nothing downloaded runs on the host. Attaches to the run's terminal. The version is pinned; only `update` changes it
- [`humanitl agents update`](#humanitl-agents-update) Update an agent in the depot in a maintenance sandbox, or switch back to a version the depot still holds (`--version`). Running sessions keep their version; the next one starts the new
- [`humanitl agents login`](#humanitl-agents-login) Log an agent of the depot in, in a maintenance sandbox with the depot program and no project: only its login hosts pass without a hold, and only the credential file, checked by the guard, goes back into the depot. Attaches to the run's terminal; every session after it starts logged in
- [`humanitl agents callback`](#humanitl-agents-callback) Deliver the callback URL from the browser's address bar into the running login, where the agent waits for it on its own loopback. Only exactly the agent's callback (`http://localhost:<port><path>?...`) is delivered, inside the sandbox; no port is forwarded. Not under Seatbelt (macOS): there only the device flow works
- [`humanitl agents config`](#humanitl-agents-config) Change an agent's configuration: the agent runs on a writable copy of its configuration without a project; afterwards the changed files are listed, keys that run commands highlighted, and taken over only after confirmation
- [`humanitl projects`](#humanitl-projects) The project register: the projects the daemon knows with their last start, agent and runs, and what was chosen for one of them
- [`humanitl projects ls`](#humanitl-projects-ls) List the known projects, the latest start first: name, folder, agent, last start, runs, and whether a session runs or changes wait
- [`humanitl projects show`](#humanitl-projects-show) Show one project by its folder: its entry and every key of its project choice with the chosen value, the value that applies and its origin
- [`humanitl projects clean`](#humanitl-projects-clean) Clean up: the daemon lists every item it would delete (recorded requests with their bodies, waiting changes of a disposable run, project choices, register entries) with size, and asks. Without a folder every project except what is in use. The project folder and the audit log are never deleted. Without a terminal it deletes only with --yes and otherwise ends with exit 1 and the plan
- [`humanitl projects forget`](#humanitl-projects-forget) Forget a project: its stored project choice and its register entry, shown as a plan and asked first. Recorded requests stay
- [`humanitl projects proposal`](#humanitl-projects-proposal) Show the start proposal for a folder: every field of the first-start screen (agent and its source, folder mode, isolation, ask mode, model server, profile) with its options, the default, and why an option is not available. The defaults are the start that would happen without a screen. Nothing is saved
- [`humanitl app`](#humanitl-app) Open the start window of the desktop app, with a folder preselected
- [`humanitl settings`](#humanitl-settings) Every setting as a full-screen view in the terminal: categories, search with `/`, the origin of each value, values locked by a company policy read only

## Configuration flags

Every leaf key of the configuration schema is also a global flag: `hold.timeout_secs`
becomes `--hold-timeout-secs`, and the environment variable `HUMANITL_HOLD__TIMEOUT_SECS`.
A flag wins over the environment, which wins over profiles and `config.toml`. Types,
defaults and the meaning of each key are in [`docs/CONFIG.md`](../CONFIG.md); the table
below lists the flag names and their short aliases.

| Flag | Alias | Value | Key |
|---|---|---|---|
| `--agent-adapter` | - | `STRING` | `agent.adapter` |
| `--agent-briefing-enabled` | - | `true`, `false` | `agent.briefing.enabled` |
| `--agent-command` | - | `LIST_OF_STRING,_OPTIONAL` | `agent.command` |
| `--agent-pi-web-access` | - | `auto`, `on`, `off` | `agent.pi.web_access` |
| `--agents-agy-credential-writeback` | - | `true`, `false` | `agents.agy.credential_writeback` |
| `--agents-agy-inference` | - | `model-server`, `own` | `agents.agy.inference` |
| `--agents-agy-source` | - | `host`, `depot`, `null` | `agents.agy.source` |
| `--agents-claude-credential-writeback` | - | `true`, `false` | `agents.claude.credential_writeback` |
| `--agents-claude-inference` | - | `model-server`, `own` | `agents.claude.inference` |
| `--agents-claude-source` | - | `host`, `depot`, `null` | `agents.claude.source` |
| `--agents-codex-credential-writeback` | - | `true`, `false` | `agents.codex.credential_writeback` |
| `--agents-codex-inference` | - | `model-server`, `own` | `agents.codex.inference` |
| `--agents-codex-source` | - | `host`, `depot`, `null` | `agents.codex.source` |
| `--agents-encrypt-credentials` | - | `true`, `false` | `agents.encrypt_credentials` |
| `--agents-opencode-credential-writeback` | - | `true`, `false` | `agents.opencode.credential_writeback` |
| `--agents-opencode-inference` | - | `model-server`, `own` | `agents.opencode.inference` |
| `--agents-opencode-source` | - | `host`, `depot`, `null` | `agents.opencode.source` |
| `--agents-pi-credential-writeback` | - | `true`, `false` | `agents.pi.credential_writeback` |
| `--agents-pi-inference` | - | `model-server`, `own` | `agents.pi.inference` |
| `--agents-pi-source` | - | `host`, `depot`, `null` | `agents.pi.source` |
| `--agents-source` | - | `host`, `depot` | `agents.source` |
| `--audit-anchor-every` | - | `INTEGER` | `audit.anchor_every` |
| `--audit-fsync-every` | - | `INTEGER` | `audit.fsync_every` |
| `--audit-max-bytes` | - | `INTEGER` | `audit.max_bytes` |
| `--audit-retention-days` | - | `INTEGER` | `audit.retention_days` |
| `--egress-proxy` | - | `STRING,_OPTIONAL` | `egress.proxy` |
| `--egress-proxy-auth-ref` | - | `STRING,_OPTIONAL` | `egress.proxy_auth_ref` |
| `--egress-proxy-ca` | - | `STRING,_OPTIONAL` | `egress.proxy_ca` |
| `--egress-proxy-resolve` | - | `proxy`, `humanitl` | `egress.proxy_resolve` |
| `--egress-proxy-response-timeout-secs` | - | `INTEGER` | `egress.proxy_response_timeout_secs` |
| `--experimental-h2-upstream` | - | `true`, `false` | `experimental.h2_upstream` |
| `--findings-email-allow-domains` | - | `LIST_OF_STRING` | `findings.email_allow_domains` |
| `--findings-enabled` | - | `true`, `false` | `findings.enabled` |
| `--findings-ignored-hashes` | - | `LIST_OF_STRING` | `findings.ignored_hashes` |
| `--findings-user-terms` | - | `LIST_OF_STRING` | `findings.user_terms` |
| `--hold-ask-mode` | `--ask` | `ui`, `terminal`, `none`, `upstream` | `hold.ask_mode` |
| `--hold-hard-block-checksum-secrets` | - | `true`, `false` | `hold.hard_block_checksum_secrets` |
| `--hold-timeout-secs` | - | `INTEGER` | `hold.timeout_secs` |
| `--limits-body-timeout-secs` | - | `INTEGER` | `limits.body_timeout_secs` |
| `--limits-connect-timeout-secs` | - | `INTEGER` | `limits.connect_timeout_secs` |
| `--limits-event-buffer` | - | `INTEGER` | `limits.event_buffer` |
| `--limits-header-timeout-secs` | - | `INTEGER` | `limits.header_timeout_secs` |
| `--limits-hold-body-cap-bytes` | - | `INTEGER` | `limits.hold_body_cap_bytes` |
| `--limits-hold-max-bytes` | - | `INTEGER` | `limits.hold_max_bytes` |
| `--limits-hold-max-flows` | - | `INTEGER` | `limits.hold_max_flows` |
| `--limits-max-client-connections` | - | `INTEGER` | `limits.max_client_connections` |
| `--limits-max-decompress-ratio` | - | `INTEGER` | `limits.max_decompress_ratio` |
| `--limits-preview-cap-bytes` | - | `INTEGER` | `limits.preview_cap_bytes` |
| `--limits-recorder-max-body-bytes` | - | `INTEGER` | `limits.recorder_max_body_bytes` |
| `--limits-registry-max-finished-flows` | - | `INTEGER` | `limits.registry_max_finished_flows` |
| `--llm-active` | - | `STRING,_OPTIONAL` | `llm.active` |
| `--llm-api-key-ref` | - | `STRING,_OPTIONAL` | `llm.api_key_ref` |
| `--llm-cloud` | - | `allow`, `block` | `llm.cloud` |
| `--llm-endpoint` | `--llm` | `STRING,_OPTIONAL` | `llm.endpoint` |
| `--llm-models` | - | `LIST_OF_STRING` | `llm.models` |
| `--llm-passthrough-paths` | - | `LIST_OF_STRING` | `llm.passthrough_paths` |
| `--llm-server` | - | `STRING,_OPTIONAL` | `llm.server` |
| `--pseudonyms-max-response-bytes` | - | `INTEGER` | `pseudonyms.max_response_bytes` |
| `--pseudonyms-translate-responses` | - | `true`, `false` | `pseudonyms.translate_responses` |
| `--recorder-inline-max-bytes` | - | `INTEGER` | `recorder.inline_max_bytes` |
| `--recorder-retention-days` | - | `INTEGER` | `recorder.retention_days` |
| `--resolver-cache-ttl-secs` | - | `INTEGER` | `resolver.cache_ttl_secs` |
| `--resolver-nameserver` | - | `STRING,_OPTIONAL` | `resolver.nameserver` |
| `--resolver-prefer` | - | `ipv4`, `ipv6` | `resolver.prefer` |
| `--resolver-test-ca` | - | `STRING,_OPTIONAL` | `resolver.test_ca` |
| `--sandbox-backend` | `--backend` | `auto`, `bwrap`, `podman`, `seatbelt`, `qemu` | `sandbox.backend` |
| `--sandbox-min-isolation` | - | `policy`, `namespaces`, `kernel` | `sandbox.min_isolation` |
| `--sandbox-mode` | - | `auto`, `direct`, `disposable` | `sandbox.mode` |
| `--sandbox-podman-cpus` | - | `INTEGER,_OPTIONAL` | `sandbox.podman.cpus` |
| `--sandbox-podman-image` | - | `STRING,_OPTIONAL` | `sandbox.podman.image` |
| `--sandbox-podman-memory-mib` | - | `INTEGER,_OPTIONAL` | `sandbox.podman.memory_mib` |
| `--sandbox-podman-pids-limit` | - | `INTEGER` | `sandbox.podman.pids_limit` |
| `--sandbox-podman-program` | - | `STRING,_OPTIONAL` | `sandbox.podman.program` |
| `--sandbox-podman-rootfs` | - | `host`, `image` | `sandbox.podman.rootfs` |
| `--sandbox-podman-runtime` | `--podman-runtime` | `crun` | `sandbox.podman.runtime` |
| `--sandbox-podman-start-timeout-secs` | - | `INTEGER` | `sandbox.podman.start_timeout_secs` |
| `--sandbox-profile` | - | `STRING` | `sandbox.profile` |
| `--sandbox-qemu-cpus` | - | `INTEGER,_OPTIONAL` | `sandbox.qemu.cpus` |
| `--sandbox-qemu-kernel` | - | `STRING,_OPTIONAL` | `sandbox.qemu.kernel` |
| `--sandbox-qemu-memory-mib` | - | `INTEGER,_OPTIONAL` | `sandbox.qemu.memory_mib` |
| `--sandbox-qemu-modules` | - | `STRING,_OPTIONAL` | `sandbox.qemu.modules` |
| `--sandbox-qemu-qemu` | - | `STRING,_OPTIONAL` | `sandbox.qemu.qemu` |
| `--sandbox-qemu-start-timeout-secs` | - | `INTEGER` | `sandbox.qemu.start_timeout_secs` |
| `--sandbox-qemu-vhost-device-vsock` | - | `STRING,_OPTIONAL` | `sandbox.qemu.vhost_device_vsock` |
| `--sandbox-qemu-virtiofsd` | - | `STRING,_OPTIONAL` | `sandbox.qemu.virtiofsd` |
| `--sandbox-work-dir` | `--work` | `STRING,_OPTIONAL` | `sandbox.work_dir` |
| `--sandbox-work-mode` | `--work-mode` | `ro`, `rw` | `sandbox.work_mode` |
| `--terminal-bell-on-hold` | - | `true`, `false` | `terminal.bell_on_hold` |
| `--terminal-direct-keys` | - | `true`, `false` | `terminal.direct_keys` |
| `--terminal-on-close` | - | `detach`, `stop` | `terminal.on_close` |
| `--terminal-prefix` | - | `STRING` | `terminal.prefix` |
| `--terminal-splash` | - | `true`, `false` | `terminal.splash` |
| `--tls-trust-system-store` | - | `true`, `false` | `tls.trust_system_store` |
| `--ui-language` | - | `en`, `de` | `ui.language` |
| `--ui-notifications` | - | `true`, `false` | `ui.notifications` |
| `--ui-terminal-notices` | - | `true`, `false` | `ui.terminal_notices` |
| `--ui-theme` | - | `dark`, `light`, `system` | `ui.theme` |
| `--update-check` | - | `true`, `false` | `update.check` |

## humanitl

```text
Human-in-the-loop network moderation for AI agents

Usage: humanitl [OPTIONS] <COMMAND>

Commands:
  run        Start a session in the current directory, or show the session a profile resolves to
  go         Start an agent in a project folder and land in its terminal; afterwards choose what to keep
  sandbox    Start, plan and check the sandbox
  rules      List, add, change, reorder, dry-run and reload rules
  flows      The flows of this and earlier sessions
  sessions   The session and who decides a request without a rule; with a subcommand, what the agent left in the project during a sandbox run
  audit      Verify and export the audit chain
  config     The resolved configuration and its schema
  daemon     The background service
  doctor     Check this machine: one line per precondition, with a fix
  update     Check for a newer Humanitl and install it
  llm        Find LLM servers in the local network
  intercept  Pause intercepting, resume it, or show whether it is paused
  egress     The decision proxy behind Humanitl (`egress.proxy`): show it, check the connection, store its credentials
  agents     The agent depot: where each agent comes from (host or depot), its installed version and login; set the source, install or update an agent in a maintenance sandbox, remove a depot
  projects   The project register: the projects the daemon knows with their last start, agent and runs, and what was chosen for one of them
  app        Open the start window of the desktop app, with a folder preselected
  settings   Every setting as a full-screen view in the terminal: categories, search with `/`, the origin of each value, values locked by a company policy read only
  help       Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Exit codes:
  0   the command did what it says
  1   user error; the diagnostic on stderr says what and why
  2   the daemon is not reachable
  3   a sandbox isolation check failed
  4   a security violation, for example an authority mismatch
  10  rules test: the request would be blocked
  11  rules test: the request would be held for a decision

A failing command writes a diagnostic block to stderr, or with --json one line
of JSON to stdout.
```

## humanitl run

```text
Start a session in the current directory, or show the session a profile resolves to

Usage: humanitl run [OPTIONS] [-- <CMD>...]

Arguments:
  [CMD]...
          The command to run in the sandbox, after `--`; without it the agent from the configuration

Options:
      --disposable
          Keep the project folder untouched: it lies read-only under a layer (in RAM on Linux, an APFS clone on macOS), and after the run `humanitl sandbox changes` shows what the agent changed, `apply` writes it and `discard` drops it. The same as sandbox.mode = "disposable" for this run: where the system cannot do it, the start refuses instead of writing to the folder. Without this flag and without --direct the run follows sandbox.mode, by default "auto": disposable where the system allows it, otherwise direct with a notice

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --direct
          Let the agent write to the project folder directly, as without the disposable sandbox. The same as sandbox.mode = "direct" for this run

  -v, --verbose...
          Explain what is happening; repeat for more

      --yes
          Do not ask before the first start of an agent whose model provider lies outside this machine (cloud inference); the notice still goes to stderr, and this counts as confirmed for that agent and provider. Without a terminal or with --json nobody is asked either: the notice is one line on stderr and the next start shows it again

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl go

```text
Start an agent in a project folder and land in its terminal; afterwards choose what to keep.

`humanitl go claude` starts Claude Code in the current folder. A known project starts with its project choice. The first start in a new project shows one screen with the defaults of the start proposal: Enter starts with them, `a` customizes, Esc or q cancels (nothing is saved or started, exit 130); the choice is kept for the project, and the next start shows no screen. With --yes, --json or without a terminal there is no screen: one line on stderr names the defaults, they are saved, and the agent starts. The agent word and the switches hold for this start only and never change the stored project choice. This terminal is handed to the agent as soon as the session runs. While the session starts, a rotating fist is shown for at least one second (off with `terminal.splash = false`, without colour with `NO_COLOR`, never with --json or without a terminal). When the agent ends and changes wait in the disposable layer, the command lists them and asks whether to apply, discard or keep them for later.

Usage: humanitl go [OPTIONS] [AGENT] [-- <CMD>...]

Arguments:
  [AGENT]
          The agent to start, by the id of its adapter as `humanitl agents status` lists them (opencode, pi, claude, codex, agy); its program comes from the host or the agent depot as `agents.source` says. Without it, the agent of the project choice, for a new project the default. Always the agent: a folder of the same name is chosen with --dir

  [CMD]...
          The command to run in the sandbox, after `--`. With an AGENT these are arguments for the agent (`humanitl go claude -- --resume`); without one, the command replaces the agent

Options:
      --dir <PATH>
          The project folder; relative to the current directory. Without it, the current directory. Not together with --work

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --disposable
          Keep the project folder untouched for this start: the agent writes into a disposable layer, and afterwards you choose what to keep

      --rw
          Let the agent write to the project folder directly for this start

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --ro
          Let the agent only read the project folder for this start

      --kernel
          Start at the kernel level (an own guest kernel) for this start: raises `sandbox.min_isolation` to "kernel" like --sandbox-min-isolation kernel. Where the kernel level is not available the start is refused with its finding; it never falls back to a weaker level

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

      --yes
          Start a new project without the first-start screen: one line on stderr names the defaults of the start proposal, they are saved as the project choice, and the agent starts. Do not ask before the first start of an agent whose model provider lies outside this machine (cloud inference) either; the notice still goes to stderr, and this counts as confirmed for that agent and provider. Without a terminal or with --json nobody is asked: the notice is one line on stderr and the next start shows it again

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox

```text
Start, plan and check the sandbox

Usage: humanitl sandbox [OPTIONS] <COMMAND>

Commands:
  run       Start a sandbox with CMD as the agent and exit with its exit code
  argv      Print the bwrap command line for the current profile without starting it
  check     Start a short-lived sandbox and show the three isolation guarantees
  backends  List the sandbox backends of this system: the isolation level each offers, whether a start with it would pass, which one sandbox.backend chooses, and what is missing
  plan      Show what a start in this project would get, without starting: profile, project folder, mode, and the saved model server with the place its choice comes from (this run, the project, a profile or llm.active)
  attach    Attach this terminal to the running session
  exec      Open a further shell in the running sandbox, next to the agent
  changes   Show what a disposable run changed: files changed, created and deleted, with a diff for text files and size and hash for others
  apply     Write the changes of a disposable run to the project folder and discard the rest
  discard   Drop all changes of a disposable run. Asks first
  image     The image a container sandbox starts from, when its root is an image
  help      Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox run

```text
Start a sandbox with CMD as the agent and exit with its exit code

Usage: humanitl sandbox run [OPTIONS] -- <CMD>...

Arguments:
  <CMD>...
          The command to run in the sandbox, after `--`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --tests-dir <DIR>
          Host directory mounted at /tests/escape of the `test` profile

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --disposable
          Run with the project folder read-only under a layer (in RAM on Linux, an APFS clone on macOS); at the end the changes are listed and discarded (this command keeps no layer; `humanitl run --disposable` keeps them for apply). Without this flag the run is direct unless sandbox.mode is "disposable": the default "auto" means direct here, because nothing could apply the changes afterwards

      --direct
          Run direct even if sandbox.mode is "disposable"

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --yes
          Do not ask before the first start of an agent whose model provider lies outside this machine (cloud inference); the notice still goes to stderr, and this counts as confirmed for that agent and provider. Without a terminal or with --json the notice is one line on stderr and nobody is asked

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox argv

```text
Print the bwrap command line for the current profile without starting it.

Environment values the sandbox screen withholds (anything written by hand in sandbox.env or a profile of your own, and any name not on the list of visible variables) are printed as `<withheld>`.

Usage: humanitl sandbox argv [OPTIONS] [-- <CMD>...]

Arguments:
  [CMD]...
          The command the line ends with; without it the agent from the configuration, otherwise `/bin/sh`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --show-values
          Print withheld environment values in clear text. The line may then carry credentials; do not paste it into a ticket

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox check

```text
Start a short-lived sandbox and show the three isolation guarantees

Usage: humanitl sandbox check [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox backends

```text
List the sandbox backends of this system: the isolation level each offers, whether a start with it would pass, which one sandbox.backend chooses, and what is missing.

Asks the daemon (`SandboxBackends`) and starts nothing; `humanitl doctor` makes the probe start. A backend that is not available is never replaced by another one: choose it with `--backend NAME` on `humanitl run` or `humanitl sandbox run`, and the start fails with the reason shown here.

Usage: humanitl sandbox backends [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox plan

```text
Show what a start in this project would get, without starting: profile, project folder, mode, and the saved model server with the place its choice comes from (this run, the project, a profile or llm.active).

Takes the flags of `humanitl run`: `--profile NAME` is the session profile here (not the profile of the sandbox backend; that is `--sandbox-profile`), and `--llm NAME` shows the server a `humanitl run --llm NAME` would get. Asks the daemon (Sandbox Plan); while a session runs, it shows that session's server.

Usage: humanitl sandbox plan [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox attach

```text
Attach this terminal to the running session.

One client writes, any number watch. Ctrl+C goes to the session's terminal as byte 0x03, which interrupts its foreground program, as in any terminal; this terminal itself never reaches the sandbox. The session keeps running after a detach, and a later attach shows it.

The prefix key (terminal.prefix, default Ctrl+B as in tmux, --prefix for one call) is followed by a fixed table. The same table works in humanitl run -- CMD at a terminal and in humanitl go:

  Ctrl+B d  detach; the session keeps running
  Ctrl+B ,  settings over the agent; Esc returns to it
  Ctrl+B i  interceptor over the agent: decide held requests; Esc returns
  Ctrl+B ?  this help
  Ctrl+B Ctrl+B or Ctrl+B b  send Ctrl+B once to the agent
            (a tmux in the sandbox, readline's one character back)
  Ctrl+B and any other key  both go to the agent
  Ctrl+,    settings directly while the agent uses the kitty keyboard
            protocol (terminal.direct_keys)
  Ctrl+L    in the agent redraws it if a view left a remnant on the
            screen (most agents redraw by themselves after a view)

Usage: humanitl sandbox attach [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --read-only
          Watch without sending anything. Ctrl+B d detaches here too, and so does Ctrl+C

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --prefix <SPEC>
          The prefix key: ctrl-a to ctrl-z (not ctrl-h, ctrl-i, ctrl-j, ctrl-m: Backspace, Tab and Enter send them), ctrl-\, ctrl-], ctrl-^, ctrl-_, or none for no prefix key, so that every byte goes to the agent. Default from `terminal.prefix`, which is ctrl-b. The old name --detach-keys still works

          [alias: --detach-keys]

      --shell <ID>
          Attach to a further shell from `humanitl sandbox exec` instead of the agent's terminal

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox exec

```text
Open a further shell in the running sandbox, next to the agent.

Like docker exec: the shell gets a terminal of its own, the agent keeps running and keeps its terminal. The shell carries the same limits as the agent (filter, mounts, network through the proxy, rules) and sees the same project folder. It ends when its process exits or when the sandbox stops; at most four live at a time. Detach with Ctrl+B d, attach again with `humanitl sandbox attach --shell ID`. Without a terminal on stdin the command only watches the output.

Usage: humanitl sandbox exec [OPTIONS] [-- <CMD>...]

Arguments:
  [CMD]...
          The command to run, after `--`; without it an interactive bash, or sh when the sandbox has no bash

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --prefix <SPEC>
          The prefix key, as for `humanitl sandbox attach`; the old name --detach-keys still works

          [alias: --detach-keys]

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox changes

```text
Show what a disposable run changed: files changed, created and deleted, with a diff for text files and size and hash for others.

Each row starts with the id that `humanitl sandbox apply` takes. A conflict means the file changed in the project since the start; a change marked "never applied" (a symbolic link out of the project, a device, FIFO or socket) is not written to the host.

Usage: humanitl sandbox changes [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --stat
          Only the list, without the diffs

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox apply

```text
Write the changes of a disposable run to the project folder and discard the rest.

Without IDs all changes; with IDs only those. Asks before it writes. Nothing is written when one of the chosen changes cannot be applied as it is; the changes keep waiting then.

Usage: humanitl sandbox apply [OPTIONS] [ID]...

Arguments:
  [ID]...
          The ids from `humanitl sandbox changes`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --overwrite
          Also write files that changed in the project since the start

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --yes
          Do not ask; for scripts

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sandbox discard

```text
Drop all changes of a disposable run. Asks first

Usage: humanitl sandbox discard [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --yes
          Do not ask; for scripts

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox image

```text
The image a container sandbox starts from, when its root is an image

Usage: humanitl sandbox image [OPTIONS] <COMMAND>

Commands:
  pull  Pull an image into the local storage of the container engine, pinned by its digest
  help  Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox image pull

```text
Pull an image into the local storage of the container engine, pinned by its digest.

Pulling uses the network directly: the container engine downloads the image from the registry its name names, not through the Humanitl proxy, without its rules and without the proxy variables of this shell. No start ever pulls; a start whose image is missing or holds another digest stops with `SANDBOX_043` and names this command. Says so and asks first.

The pull goes to the backend a start would take: `--backend NAME` or `HUMANITL_SANDBOX__BACKEND` as with `humanitl run`, otherwise sandbox.backend of the daemon.

Usage: humanitl sandbox image pull [OPTIONS] [REFERENCE]

Arguments:
  [REFERENCE]
          NAME@sha256:<64 hex digits>, with the registry in the name (docker.io/library/debian@sha256:...). Without it the image from the configuration

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --yes
          Do not ask; for scripts. The note on the network still goes to stderr

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl rules

```text
List, add, change, reorder, dry-run and reload rules

Usage: humanitl rules [OPTIONS] <COMMAND>

Commands:
  list     List the rules in the order in which they are evaluated
  add      Add a rule. The daemon checks it and answers with the whole set
  update   Change one rule. What is not given stays as it is
  remove   Remove one rule
  reorder  Move one rule to another place inside its own group
  dry-run  Show which of the last recorded requests a rule would have matched
  disable  Switch a bundled rule off. It stays in the list and decides nothing
  enable   Switch a bundled rule back on
  reload   Read rules.yaml again and report what changed
  test     Evaluate one URL against the rule set; exit 10 block, 11 ask
  help     Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules list

```text
List the rules in the order in which they are evaluated

Usage: humanitl rules list [OPTIONS]

Options:
      --all
          Show bundled and expired rules as well

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules add

```text
Add a rule. The daemon checks it and answers with the whole set

Usage: humanitl rules add [OPTIONS]

Options:
      --action <ACTION>
          What happens to a matching request

          [possible values: allow, block, ask, redact]

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --host <PATTERN>
          Host pattern: a label glob, `ip:ADDRESS` or `cidr:ADDRESS/LEN`

      --method <M>
          One HTTP method; repeat the flag for more. Without it, every method matches

          [possible values: GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS, CONNECT, TRACE]

  -v, --verbose...
          Explain what is happening; repeat for more

      --path <P>
          Path glob, or a regular expression when it starts with `~`

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

      --scheme <S>
          Only requests with this scheme

          [possible values: http, https, ws, wss]

      --port <N>
          Only requests to this port

      --upgrade <UPGRADE>
          Only requests that carry this upgrade

          [possible values: none, websocket]

      --expires <WHEN>
          never, session, or a point in time in RFC 3339

      --note <TEXT>
          Why the rule exists. It ends up in rules.yaml and in the window

      --position <N>
          Place inside the group of the rule, 1-based; without it at the end

      --allow-private[=<BOOL>]
          Allow private destinations (RFC 1918, loopback, link-local, CGNAT)

          [possible values: true, false]

  -h, --help
          Print help
```

## humanitl rules update

```text
Change one rule. What is not given stays as it is

Usage: humanitl rules update [OPTIONS] <ID>

Arguments:
  <ID>
          The id of the rule

Options:
      --action <ACTION>
          What happens to a matching request

          [possible values: allow, block, ask, redact]

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --host <PATTERN>
          Host pattern: a label glob, `ip:ADDRESS` or `cidr:ADDRESS/LEN`

      --method <M>
          One HTTP method; repeat the flag for more. Without it, every method matches

          [possible values: GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS, CONNECT, TRACE]

  -v, --verbose...
          Explain what is happening; repeat for more

      --path <P>
          Path glob, or a regular expression when it starts with `~`

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

      --scheme <S>
          Only requests with this scheme

          [possible values: http, https, ws, wss]

      --port <N>
          Only requests to this port

      --upgrade <UPGRADE>
          Only requests that carry this upgrade

          [possible values: none, websocket]

      --expires <WHEN>
          never, session, or a point in time in RFC 3339

      --note <TEXT>
          Why the rule exists. It ends up in rules.yaml and in the window

      --position <N>
          Place inside the group of the rule, 1-based; without it at the end

      --allow-private[=<BOOL>]
          Allow private destinations (RFC 1918, loopback, link-local, CGNAT)

          [possible values: true, false]

  -h, --help
          Print help
```

## humanitl rules remove

```text
Remove one rule

Usage: humanitl rules remove [OPTIONS] <ID>

Arguments:
  <ID>
          The id of the rule

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules reorder

```text
Move one rule to another place inside its own group

Usage: humanitl rules reorder [OPTIONS] <ID> <POSITION>

Arguments:
  <ID>
          The id of the rule

  <POSITION>
          The new place, 1-based, inside the group of the rule

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules dry-run

```text
Show which of the last recorded requests a rule would have matched

Usage: humanitl rules dry-run [OPTIONS]

Options:
      --action <ACTION>
          What happens to a matching request

          [possible values: allow, block, ask, redact]

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --host <PATTERN>
          Host pattern: a label glob, `ip:ADDRESS` or `cidr:ADDRESS/LEN`

      --method <M>
          One HTTP method; repeat the flag for more. Without it, every method matches

          [possible values: GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS, CONNECT, TRACE]

  -v, --verbose...
          Explain what is happening; repeat for more

      --path <P>
          Path glob, or a regular expression when it starts with `~`

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

      --scheme <S>
          Only requests with this scheme

          [possible values: http, https, ws, wss]

      --port <N>
          Only requests to this port

      --upgrade <UPGRADE>
          Only requests that carry this upgrade

          [possible values: none, websocket]

      --expires <WHEN>
          never, session, or a point in time in RFC 3339

      --note <TEXT>
          Why the rule exists. It ends up in rules.yaml and in the window

      --position <N>
          Place inside the group of the rule, 1-based; without it at the end

      --allow-private[=<BOOL>]
          Allow private destinations (RFC 1918, loopback, link-local, CGNAT)

          [possible values: true, false]

      --scan <N>
          How many of the most recently recorded requests to check; 0 means the daemon's default

          [default: 0]

  -h, --help
          Print help
```

## humanitl rules disable

```text
Switch a bundled rule off. It stays in the list and decides nothing

Usage: humanitl rules disable [OPTIONS] <ID>

Arguments:
  <ID>
          The id of the bundled rule

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules enable

```text
Switch a bundled rule back on

Usage: humanitl rules enable [OPTIONS] <ID>

Arguments:
  <ID>
          The id of the bundled rule

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules reload

```text
Read rules.yaml again and report what changed

Usage: humanitl rules reload [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl rules test

```text
Evaluate one URL against the rule set; exit 10 block, 11 ask

Usage: humanitl rules test [OPTIONS] <URL>

Arguments:
  <URL>
          The full URL, for example `https://api.github.com/repos/x`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --method <M>
          The method of the imagined request

          [possible values: GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS, CONNECT, TRACE]

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --upgrade <UPGRADE>
          Whether the imagined request carries an upgrade

          [possible values: websocket]

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl flows

```text
The flows of this and earlier sessions

Usage: humanitl flows [OPTIONS] <COMMAND>

Commands:
  list    List flows, newest first; --asc turns the order around
  show    Show one flow, or one of its bodies
  decide  Decide one waiting flow: allow it or block it
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl flows list

```text
List flows, newest first; --asc turns the order around.

SIZE is request plus response in bytes, MS the duration in milliseconds, PATH shortened in the middle to 40 characters. What the daemon does not know stays a dash.

ID is the end of the flow id, as many characters as it takes to tell the rows of this page apart and at least eight; `flows show` takes it as it stands. DECISION reads `aborted` where the client gave up its TLS connection to the proxy before there was anything to decide (nobody blocked it; `flows show` names the cause), `passthrough` for the requests to the model endpoint, which only --passthrough lists, and `forward` for a request the decision proxy decided (`hold.ask_mode = "upstream"`); STATUS is its answer.

Usage: humanitl flows list [OPTIONS] [FILTER]...

Arguments:
  [FILTER]...
          The filter, for example `host:github.com state:blocked`. Several words are joined with one space and passed to the daemon unchanged

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --limit <N>
          At most this many rows; 0 means the daemon's default

          [default: 0]

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --sort <KEY>
          The column to sort by

          [default: ts]
          [possible values: ts, host, duration, size]

      --asc
          Ascending instead of descending: the oldest row first

  -v, --verbose...
          Explain what is happening; repeat for more

      --passthrough
          Include the requests the passthrough let through to the model endpoint; DECISION reads `passthrough` for them

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl flows show

```text
Show one flow, or one of its bodies

Usage: humanitl flows show [OPTIONS] <ID>

Arguments:
  <ID>
          The id of the flow: the whole id, or its end as `flows list` prints it (at least eight characters), looked up among the last 1000 flows

Options:
      --body <WHICH>
          Print one of the two bodies instead of the fields

          [possible values: request, response]

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --raw
          Write the body byte for byte instead of as text

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl flows decide

```text
Decide one waiting flow: allow it or block it.

The human decision the window sends, from the terminal. It came before `--ask terminal` (HUM-021) and stays, because a script or a remote session has no window.

Usage: humanitl flows decide [OPTIONS] <ID> <VERDICT>

Arguments:
  <ID>
          The id of the flow

  <VERDICT>
          What happens to it

          [possible values: allow, block]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --note <TEXT>
          A reason for the agent; it stands in the block body under `note:`

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --remember <PATTERN>
          Host pattern of the rule to remember. Without it nothing is remembered

      --remember-method <M>
          One HTTP method for the rule; repeat the flag for more

          [possible values: GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS, CONNECT, TRACE]

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --remember-path <P>
          Path glob of the rule, or a regular expression when it starts with `~`

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

      --remember-expires <WHEN>
          never, session, or a point in time in RFC 3339. Without it: session

      --remember-note <TEXT>
          Why the rule exists. It ends up in rules.yaml and in the window

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sessions

```text
The session and who decides a request without a rule; with a subcommand, what the agent left in the project during a sandbox run.

Without a subcommand: the running session, or the one a start here would get, its state and the mode line: who decides a request without a rule, with `hold.ask_mode = "upstream"` the decision proxy as `HOST:PORT` and who resolves names.

Usage: humanitl sessions [OPTIONS] [COMMAND]

Commands:
  summary  Show what a sandbox run changed in the project
  help     Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl sessions summary

```text
Show what a sandbox run changed in the project.

Changed files, possible secrets and symlinks that leave the project. ID is the sandbox id from the log line of the run or from `humanitl sandbox` in the window; it is the only value this command needs.

Every path comes from the agent and is shown, never reopened: two different names can look the same, so each row carries the first 16 hex characters of the sha256 of the real name.

Usage: humanitl sessions summary [OPTIONS] <ID>

Arguments:
  <ID>
          The id of the sandbox run

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl audit

```text
Verify and export the audit chain

Usage: humanitl audit [OPTIONS] <COMMAND>

Commands:
  verify  Check the hash chain: every record, its predecessor and the anchors
  export  Write the records to a file, as JSON lines or as CSV
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl audit verify

```text
Check the hash chain: every record, its predecessor and the anchors

Usage: humanitl audit verify [OPTIONS]

Options:
      --file <PATH>
          Verify this file instead of asking the daemon; without key and anchors, so the answer is weaker and says so

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl audit export

```text
Write the records to a file, as JSON lines or as CSV

Usage: humanitl audit export [OPTIONS] --format <FORMAT> --out <FILE>

Options:
      --format <FORMAT>
          `jsonl` keeps every record as it stands, `csv` is one row per record

          [possible values: jsonl, csv]

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --out <FILE>
          Where the export is written. An existing file is not overwritten

      --since <TS>
          Only records at or after this point in time (RFC 3339)

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --until <TS>
          Only records before this point in time (RFC 3339)

      --file <PATH>
          Export this file instead of asking the daemon

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config

```text
The resolved configuration and its schema

Usage: humanitl config [OPTIONS] <COMMAND>

Commands:
  get      Print the resolved value of one key, or of every key without one
  set      Write one key into config.toml, after checking it against the schema
  schema   Print the JSON schema of the configuration, or the list of profiles
  edit     Open config.toml in $VISUAL or $EDITOR and check it afterwards
  catalog  Print the settings catalog of the running daemon: categories, titles, controls, origin and lock of every setting, the source the app and the settings TUI draw from
  paths    Print where Humanitl keeps your data: configuration, recordings, the project register, the agent depot and the audit chain
  help     Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config get

```text
Print the resolved value of one key, or of every key without one

Usage: humanitl config get [OPTIONS] [KEY]

Arguments:
  [KEY]
          The path of the key, separated by dots; without it every leaf value

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --origin
          Add the layer that set each value

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config set

```text
Write one key into config.toml, after checking it against the schema

Usage: humanitl config set [OPTIONS] <KEY> <VALUE>

Arguments:
  <KEY>
          The path of the key, separated by dots

  <VALUE>
          The value, read by the type of the key: `5m` for a duration, `32MiB` for a size, `true`/`false`, JSON for a list, `null` or `-` to remove the key. A value that starts with `-` is a value, not a flag

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --project
          Write into the profile of this project instead of the global file

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config schema

```text
Print the JSON schema of the configuration, or the list of profiles

Usage: humanitl config schema [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --profiles
          List the profiles --profile can choose instead of the schema

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config edit

```text
Open config.toml in $VISUAL or $EDITOR and check it afterwards

Usage: humanitl config edit [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config catalog

```text
Print the settings catalog of the running daemon: categories, titles, controls, origin and lock of every setting, the source the app and the settings TUI draw from

Usage: humanitl config catalog [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --project <PATH>
          Resolve as a start in this project folder would, with its project choice

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --language <LANG>
          The language of the texts, `en` or `de`; without it `ui.language`

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl config paths

```text
Print where Humanitl keeps your data: configuration, recordings, the project register, the agent depot and the audit chain.

One line per directory, NAME and PATH: `config` and `data`, on macOS also `support` (Application Support) and `logs`. --json gives the same as a list. A project folder is never among them. This is the list `uninstall.sh --purge` deletes from; the command itself opens and creates no file.

Usage: humanitl config paths [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl daemon

```text
The background service

Usage: humanitl daemon [OPTIONS] <COMMAND>

Commands:
  status          Ask the daemon who it is
  install         Write the systemd user unit that starts the daemon at login
  logs            Show the journal of the daemon: journalctl --user -u humanitld
  restart         Restart the background service: launchctl kickstart on macOS, systemctl --user restart on Linux
  uninstall       Stop and disable the service and remove the unit daemon install wrote
  retire-if-idle  Ask the daemon to end now if no agent session holds it (after an upgrade)
  help            Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl daemon status

```text
Ask the daemon who it is

Usage: humanitl daemon status [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl daemon install

```text
Write the systemd user unit that starts the daemon at login

Usage: humanitl daemon install [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --print
          Show the unit and the commands, write nothing and start nothing

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --no-start
          Write the unit, but do not tell systemd about it.

          Without this, the command runs `systemctl --user daemon-reload` and `systemctl --user enable --now humanitld.service` after writing. Never with sudo: the daemon is a user service.

      --bin-dir <DIR>
          Take humanitld and humanitl-shim from this directory instead of from the directory of the running humanitl.

          From an `AppImage` (`$APPIMAGE` set), DIR/.. must also hold share/humanitl/catalog/domains.yaml and profiles/sandbox/default.toml, as in the image; they are copied with the binaries.

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --refresh
          Only from an `AppImage`, and only when daemon install ran before: if the installed copy is another version, copy this one and restart the service. Otherwise do nothing.

          The `AppImage` runs this at every start (`AppRun`, HUM-077); the first install stays a click in the setup or `--cli daemon install`.

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl daemon logs

```text
Show the journal of the daemon: journalctl --user -u humanitld

Usage: humanitl daemon logs [OPTIONS]

Options:
  -f, --follow
          Keep the journal open and show new lines as they arrive

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -n, --lines <N>
          How many lines to show; the default of journalctl without it

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl daemon restart

```text
Restart the background service: launchctl kickstart on macOS, systemctl --user restart on Linux

Usage: humanitl daemon restart [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl daemon uninstall

```text
Stop and disable the service and remove the unit daemon install wrote

Usage: humanitl daemon uninstall [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --purge-binaries
          Also remove the copies an `AppImage` placed under `~/.local/lib/humanitl`

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl daemon retire-if-idle

```text
Ask the daemon to end now if no agent session holds it (after an upgrade)

Usage: humanitl daemon retire-if-idle [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl doctor

```text
Check this machine: one line per precondition, with a fix

Usage: humanitl doctor [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --probe-llm
          Contact llm.endpoint as part of the report; without this, doctor opens no connection at all.

          The endpoint is named on stderr before it is contacted. The probe runs in the daemon (ProbeLlm): GET requests for the model list of the endpoint (and /api/tags first on a plain Ollama server without a path prefix), no credentials, no redirects.

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --probe
          Start an empty sandbox with the shim and `true` under every admitted, available sandbox backend and check its five CHECK lines; without this, doctor starts nothing.

          A passed probe is remembered until the version of the backend's program or of Humanitl changes. A backend that is not admitted yet is never started; its probe line says `SANDBOX_048`.

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl update

```text
Check for a newer Humanitl and install it

Usage: humanitl update [OPTIONS] <COMMAND>

Commands:
  check  Ask GitHub now whether a newer release exists
  apply  Install the newer release this installation can take
  help   Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl update check

```text
Ask GitHub now whether a newer release exists.

The daemon fetches the release's update manifest and checks its Ed25519 signature against the key built into Humanitl before it believes the version. It asks even when update.check is off.

Usage: humanitl update check [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl update apply

```text
Install the newer release this installation can take.

macOS app, unpacked archive or `AppImage`: the daemon downloads the file the signed manifest names, checks size and SHA-256, unpacks, checks and swaps it, keeping the previous version until the swap succeeded. A package from the apt or dnf repository: runs `pkexec apt-get install --only-upgrade -y humanitl` or `pkexec dnf upgrade -y humanitl`. Anything else (AUR, a package without repository) prints the command to run.

Usage: humanitl update apply [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl llm

```text
Find LLM servers in the local network

Usage: humanitl llm [OPTIONS] <COMMAND>

Commands:
  discover  Search the local /24 for OpenAI-compatible servers and Ollama
  test      Ask one endpoint what it is: which API answered, its models, how long
  key       The access key of the LLM endpoint, kept in the system keyring
  server    Saved model servers: endpoint, models and access key per entry
  help      Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm discover

```text
Search the local /24 for OpenAI-compatible servers and Ollama.

Connection attempts only, to four ports, and only when you ask for it. Nothing is written and no credentials are sent.

Usage: humanitl llm discover [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --subnet <CIDR>
          The network to search, as CIDR; without it, the local /24 of the default route. Never wider than a /24

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --port <PORT>
          A port to ask, repeatable; without it 11434, 1234, 8000 and 8080

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl llm test

```text
Ask one endpoint what it is: which API answered, its models, how long.

GET requests in the daemon for the model list of the endpoint (and /api/tags first on a plain Ollama server without a path prefix), no credentials and no redirects. Nothing goes through the sandbox.

Usage: humanitl llm test [OPTIONS] <URL>

Arguments:
  <URL>
          The address as it would stand in `llm.endpoint`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --timeout-ms <N>
          Deadline for the whole probe; without it the daemon's default

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --with-key
          Send the stored access key along, if URL has the origin of llm.endpoint; the daemon never gives it to another address

      --server <NAME>
          With --with-key: the key of this saved server instead of the active one, sent only if URL has the origin of that server

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl llm key

```text
The access key of the LLM endpoint, kept in the system keyring.

The key never stands in config.toml, on a command line, in a log or in the recording; the configuration file carries only a reference to it.

Usage: humanitl llm key [OPTIONS] <COMMAND>

Commands:
  status  Whether a key is stored, and where: system keyring or 0600 file
  set     Store the key. It is read from stdin, never from the command line; at a terminal the input is hidden
  remove  Remove the key and its reference from the configuration file
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --server <NAME>
          The saved server whose key is meant; without it, the active one

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl llm key status

```text
Whether a key is stored, and where: system keyring or 0600 file

Usage: humanitl llm key status [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --server <NAME>
          The saved server whose key is meant; without it, the active one

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm key set

```text
Store the key. It is read from stdin, never from the command line; at a terminal the input is hidden

Usage: humanitl llm key set [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --server <NAME>
          The saved server whose key is meant; without it, the active one

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm key remove

```text
Remove the key and its reference from the configuration file

Usage: humanitl llm key remove [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --server <NAME>
          The saved server whose key is meant; without it, the active one

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server

```text
Saved model servers: endpoint, models and access key per entry.

The active server is what `llm.endpoint`, `llm.models` and `llm.api_key_ref` show; only its address gets the stored key. A switch applies to the next session, a running session keeps its server.

Usage: humanitl llm server [OPTIONS] <COMMAND>

Commands:
  ls        List the saved servers; the active one is marked
  add       Save a server. Its key follows with `humanitl llm key set --server NAME`
  use       Make a server the active one, from the next session on
  rename    Rename a server. Its key stays where it is
  rm        Remove a server and, unless --keep-key, its key from the keyring
  edit      Change the address or the models of a saved server without making it the default. Its key stays and goes only to its own address
  assign    Let a session profile or a project use a saved server. A project's choice made here counts as confirmed for that folder
  unassign  Remove the server a session profile or a project uses; it then uses the default
  confirm   Confirm the saved server the project in a folder chooses, so that a session there starts with it
  help      Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl llm server ls

```text
List the saved servers; the active one is marked

Usage: humanitl llm server ls [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server add

```text
Save a server. Its key follows with `humanitl llm key set --server NAME`

Usage: humanitl llm server add [OPTIONS] <NAME> <URL>

Arguments:
  <NAME>
          1 to 32 characters a-z, 0-9, - and _

  <URL>
          The address as it would stand in `llm.endpoint`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --model <MODEL>
          A model of this server, repeatable; the first is the default

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --use
          Make it the active server right away

      --local[=<BOOL>]
          Mark it as a server in your own network whose name looks public: it counts as local, also under `llm.cloud = block`

          [possible values: true, false]

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server use

```text
Make a server the active one, from the next session on

Usage: humanitl llm server use [OPTIONS] <NAME>

Arguments:
  <NAME>
          The saved server

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server rename

```text
Rename a server. Its key stays where it is

Usage: humanitl llm server rename [OPTIONS] <NAME> <NEW>

Arguments:
  <NAME>
          The saved server

  <NEW>
          The new name

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server rm

```text
Remove a server and, unless --keep-key, its key from the keyring

Usage: humanitl llm server rm [OPTIONS] <NAME>

Arguments:
  <NAME>
          The saved server

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --use <NEXT>
          The server to use afterwards; required for the active server while there are others

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --keep-key
          Leave the key of the server in the keyring

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server edit

```text
Change the address or the models of a saved server without making it the default. Its key stays and goes only to its own address

Usage: humanitl llm server edit [OPTIONS] <NAME>

Arguments:
  <NAME>
          The saved server

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --url <URL>
          The new address, as it would stand in `llm.endpoint`

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --model <MODEL>
          The models, repeatable; the first is the default. Replaces the list; without it the models stay

      --no-models
          Remove every model of the server

  -v, --verbose...
          Explain what is happening; repeat for more

      --local[=<BOOL>]
          Mark it as a server in your own network (`--local`) or remove the mark (`--local=false`); without it the mark stays while the address keeps its origin

          [possible values: true, false]

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl llm server assign

```text
Let a session profile or a project use a saved server. A project's choice made here counts as confirmed for that folder

Usage: humanitl llm server assign [OPTIONS] <--profile <PROFILE>|--project [<DIR>]> <NAME>

Arguments:
  <NAME>
          The saved server

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --profile <PROFILE>
          A session profile (`profiles/<name>.toml`)

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --project [<DIR>]
          A project folder; the current directory when given without a value

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

  -h, --help
          Print help
```

## humanitl llm server unassign

```text
Remove the server a session profile or a project uses; it then uses the default

Usage: humanitl llm server unassign [OPTIONS] <--profile <PROFILE>|--project [<DIR>]>

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --profile <PROFILE>
          A session profile (`profiles/<name>.toml`)

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --project [<DIR>]
          A project folder; the current directory when given without a value

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

  -h, --help
          Print help
```

## humanitl llm server confirm

```text
Confirm the saved server the project in a folder chooses, so that a session there starts with it

Usage: humanitl llm server confirm [OPTIONS] <NAME>

Arguments:
  <NAME>
          The saved server the project chooses

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --project <DIR>
          The project folder; the current directory without it

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl intercept

```text
Pause intercepting, resume it, or show whether it is paused

Usage: humanitl intercept [OPTIONS] <COMMAND>

Commands:
  status  Show whether intercepting is on or paused, and since when
  pause   Pause intercepting until you resume it, the next session starts or the daemon stops
  resume  Resume intercepting: every request no rule decides waits again
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl intercept status

```text
Show whether intercepting is on or paused, and since when

Usage: humanitl intercept status [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl intercept pause

```text
Pause intercepting until you resume it, the next session starts or the daemon stops.

While paused, a request that would wait for your decision goes out at once instead of being held, and is recorded with the reason "paused". Without a hold deadline (--ask none) nothing waits, and the pause changes nothing. Block rules, the hard block on verified secrets, the sandbox and the recording stay. The agent in the sandbox is not told. No setting makes the pause permanent.

Usage: humanitl intercept pause [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl intercept resume

```text
Resume intercepting: every request no rule decides waits again

Usage: humanitl intercept resume [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl egress

```text
The decision proxy behind Humanitl (`egress.proxy`): show it, check the connection, store its credentials

Usage: humanitl egress [OPTIONS] <COMMAND>

Commands:
  status  Show the decision proxy as config.toml names it, the way out the running daemon takes, and whether credentials are stored
  check   Open a tunnel through `egress.proxy` to a fixed target (example.com:443) and the TLS handshake in it, as config.toml says now. Sends no request, so nobody has to decide anything at the proxy
  auth    Store or remove the credentials of the decision proxy
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl egress status

```text
Show the decision proxy as config.toml names it, the way out the running daemon takes, and whether credentials are stored

Usage: humanitl egress status [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl egress check

```text
Open a tunnel through `egress.proxy` to a fixed target (example.com:443) and the TLS handshake in it, as config.toml says now. Sends no request, so nobody has to decide anything at the proxy

Usage: humanitl egress check [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl egress auth

```text
Store or remove the credentials of the decision proxy

Usage: humanitl egress auth [OPTIONS] <COMMAND>

Commands:
  set     Store USER:PASSWORD in the system keyring and set `egress.proxy_auth_ref`. Read from stdin, never from the command line; at a terminal the input is hidden
  remove  Remove the credentials and `egress.proxy_auth_ref`
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl egress auth set

```text
Store USER:PASSWORD in the system keyring and set `egress.proxy_auth_ref`. Read from stdin, never from the command line; at a terminal the input is hidden

Usage: humanitl egress auth set [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl egress auth remove

```text
Remove the credentials and `egress.proxy_auth_ref`

Usage: humanitl egress auth remove [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents

```text
The agent depot: where each agent comes from (host or depot), its installed version and login; set the source, install or update an agent in a maintenance sandbox, remove a depot

Usage: humanitl agents [OPTIONS] <COMMAND>

Commands:
  status    Show every agent, or one: its source with origin or company lock, the installed version and signature, whether it is logged in, its size. Nothing from the depot is started for this
  source    Set where an agent comes from: `host` or `depot` (`agents.<id>.source`). Switching back to `host` keeps the depot
  remove    Remove the depot of an agent with its program, configuration and login; refused while a session uses it. The source stays as it is
  install   Install an agent into the depot in a maintenance sandbox: no project, only the vendor's hosts without a hold, nothing downloaded runs on the host. Attaches to the run's terminal. The version is pinned; only `update` changes it
  update    Update an agent in the depot in a maintenance sandbox, or switch back to a version the depot still holds (`--version`). Running sessions keep their version; the next one starts the new
  login     Log an agent of the depot in, in a maintenance sandbox with the depot program and no project: only its login hosts pass without a hold, and only the credential file, checked by the guard, goes back into the depot. Attaches to the run's terminal; every session after it starts logged in
  callback  Deliver the callback URL from the browser's address bar into the running login, where the agent waits for it on its own loopback. Only exactly the agent's callback (`http://localhost:<port><path>?...`) is delivered, inside the sandbox; no port is forwarded. Not under Seatbelt (macOS): there only the device flow works
  config    Change an agent's configuration: the agent runs on a writable copy of its configuration without a project; afterwards the changed files are listed, keys that run commands highlighted, and taken over only after confirmation
  help      Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents status

```text
Show every agent, or one: its source with origin or company lock, the installed version and signature, whether it is logged in, its size. Nothing from the depot is started for this

Usage: humanitl agents status [OPTIONS] [AGENT]

Arguments:
  [AGENT]
          Only this agent, such as `claude`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents source

```text
Set where an agent comes from: `host` or `depot` (`agents.<id>.source`). Switching back to `host` keeps the depot

Usage: humanitl agents source [OPTIONS] <AGENT> <SOURCE>

Arguments:
  <AGENT>
          The agent, such as `claude`

  <SOURCE>
          `host` or `depot`

          [possible values: host, depot]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents remove

```text
Remove the depot of an agent with its program, configuration and login; refused while a session uses it. The source stays as it is

Usage: humanitl agents remove [OPTIONS] <AGENT>

Arguments:
  <AGENT>
          The agent, such as `claude`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --yes
          Do not ask

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents install

```text
Install an agent into the depot in a maintenance sandbox: no project, only the vendor's hosts without a hold, nothing downloaded runs on the host. Attaches to the run's terminal. The version is pinned; only `update` changes it

Usage: humanitl agents install [OPTIONS] <AGENT>

Arguments:
  <AGENT>
          The agent, such as `claude`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --version <VERSION>
          The version; without it the vendor's current one

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --yes
          Do not ask

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents update

```text
Update an agent in the depot in a maintenance sandbox, or switch back to a version the depot still holds (`--version`). Running sessions keep their version; the next one starts the new

Usage: humanitl agents update [OPTIONS] <AGENT>

Arguments:
  <AGENT>
          The agent, such as `claude`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --version <VERSION>
          The version; without it the vendor's current one

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --yes
          Do not ask

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents login

```text
Log an agent of the depot in, in a maintenance sandbox with the depot program and no project: only its login hosts pass without a hold, and only the credential file, checked by the guard, goes back into the depot. Attaches to the run's terminal; every session after it starts logged in

Usage: humanitl agents login [OPTIONS] <AGENT>

Arguments:
  <AGENT>
          The agent, such as `claude`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --open
          Open the login URL in the browser of this machine once the agent prints it. Only a URL to one of the adapter's login hosts is ever opened; any other is shown, never opened

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --yes
          Do not ask

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents callback

```text
Deliver the callback URL from the browser's address bar into the running login, where the agent waits for it on its own loopback. Only exactly the agent's callback (`http://localhost:<port><path>?...`) is delivered, inside the sandbox; no port is forwarded. Not under Seatbelt (macOS): there only the device flow works

Usage: humanitl agents callback [OPTIONS] <AGENT> <URL>

Arguments:
  <AGENT>
          The agent, such as `codex`

  <URL>
          The URL from the browser's address bar

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl agents config

```text
Change an agent's configuration: the agent runs on a writable copy of its configuration without a project; afterwards the changed files are listed, keys that run commands highlighted, and taken over only after confirmation

Usage: humanitl agents config [OPTIONS] <AGENT>

Arguments:
  <AGENT>
          The agent, such as `claude`

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --show
          Only list the configuration as it is, with its keys that run commands

      --accept
          Take over the changes of a configuration run that waits for a confirmation

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --discard
          Discard the changes of a configuration run that waits for a confirmation

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --yes
          Do not ask before the run, and take its changes over without asking

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl projects

```text
The project register: the projects the daemon knows with their last start, agent and runs, and what was chosen for one of them

Usage: humanitl projects [OPTIONS] <COMMAND>

Commands:
  ls        List the known projects, the latest start first: name, folder, agent, last start, runs, and whether a session runs or changes wait
  show      Show one project by its folder: its entry and every key of its project choice with the chosen value, the value that applies and its origin
  clean     Clean up: the daemon lists every item it would delete (recorded requests with their bodies, waiting changes of a disposable run, project choices, register entries) with size, and asks. Without a folder every project except what is in use. The project folder and the audit log are never deleted. Without a terminal it deletes only with --yes and otherwise ends with exit 1 and the plan
  forget    Forget a project: its stored project choice and its register entry, shown as a plan and asked first. Recorded requests stay
  proposal  Show the start proposal for a folder: every field of the first-start screen (agent and its source, folder mode, isolation, ask mode, model server, profile) with its options, the default, and why an option is not available. The defaults are the start that would happen without a screen. Nothing is saved
  help      Print this message or the help of the given subcommand(s)

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl projects ls

```text
List the known projects, the latest start first: name, folder, agent, last start, runs, and whether a session runs or changes wait

Usage: humanitl projects ls [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl projects show

```text
Show one project by its folder: its entry and every key of its project choice with the chosen value, the value that applies and its origin

Usage: humanitl projects show [OPTIONS] <PATH>

Arguments:
  <PATH>
          The project folder; relative to the current directory

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl projects clean

```text
Clean up: the daemon lists every item it would delete (recorded requests with their bodies, waiting changes of a disposable run, project choices, register entries) with size, and asks. Without a folder every project except what is in use. The project folder and the audit log are never deleted. Without a terminal it deletes only with --yes and otherwise ends with exit 1 and the plan

Usage: humanitl projects clean [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...
          The project folders; relative to the current directory. None means every project

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --older-than <DAYS>
          Only requests older than this many days, and the choice and entry of projects not started for so long

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --include-waiting-changes
          Also discard changes of a disposable run that wait for apply or discard. Changes not applied are lost for good

      --unassigned
          Only the sessions that belong to no project

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --yes
          Delete without asking

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl projects forget

```text
Forget a project: its stored project choice and its register entry, shown as a plan and asked first. Recorded requests stay

Usage: humanitl projects forget [OPTIONS] <PATH>

Arguments:
  <PATH>
          The project folder; relative to the current directory

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --yes
          Forget without asking

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl projects proposal

```text
Show the start proposal for a folder: every field of the first-start screen (agent and its source, folder mode, isolation, ask mode, model server, profile) with its options, the default, and why an option is not available. The defaults are the start that would happen without a screen. Nothing is saved

Usage: humanitl projects proposal [OPTIONS] [PATH]

Arguments:
  [PATH]
          The project folder; relative to the current directory. Without it, the current directory

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl app

```text
Open the start window of the desktop app, with a folder preselected.

When an app is connected to the daemon, the daemon hands it the folder and the app brings its window forward; otherwise this starts the app (`humanitl-app --project PATH` next to this program or on PATH, on macOS `open -a Humanitl`). A known project is marked under "Recent", a new folder gets the first-start screen. Nothing starts before "Start" in the window.

Limits: on macOS an app that already runs without a daemon connection only comes forward, without the folder preselected; on Linux such an app is not seen, and a second one starts.

Usage: humanitl app [OPTIONS] [PATH]

Arguments:
  [PATH]
          The folder to preselect; `.` is the current directory. Without it, the start window without a preselection

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

## humanitl settings

```text
Every setting as a full-screen view in the terminal: categories, search with `/`, the origin of each value, values locked by a company policy read only.

Rendered from the daemon's settings catalog and written through the same calls as the app. With --project, the values a start in that folder would get; there only the keys of the project choice can be changed. Secrets (access key, proxy credentials) are typed without echo and sent once. Without a terminal, or with --json, there is no view: use `humanitl config get` and `humanitl config set`.

Usage: humanitl settings [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --project <PATH>
          Show and change the project choice of this folder instead of the global settings; relative to the current directory

      --config <PATH>
          Read this config file instead of the one in the user's config directory

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help (see a summary with '-h')
```

