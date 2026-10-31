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
- [`humanitl sandbox`](#humanitl-sandbox) Start, plan and check the sandbox
- [`humanitl sandbox run`](#humanitl-sandbox-run) Start a sandbox with CMD as the agent and exit with its exit code
- [`humanitl sandbox argv`](#humanitl-sandbox-argv) Print the bwrap command line for the current profile without starting it
- [`humanitl sandbox check`](#humanitl-sandbox-check) Start a short-lived sandbox and show the three isolation guarantees
- [`humanitl sandbox plan`](#humanitl-sandbox-plan) Show what a start in this project would get, without starting: profile, project folder, mode, and the saved model server with the place its choice comes from (this run, the project, a profile or llm.active)
- [`humanitl sandbox attach`](#humanitl-sandbox-attach) Attach this terminal to the running session
- [`humanitl sandbox exec`](#humanitl-sandbox-exec) Open a further shell in the running sandbox, next to the agent
- [`humanitl sandbox changes`](#humanitl-sandbox-changes) Show what a disposable run changed: files changed, created and deleted, with a diff for text files and size and hash for others
- [`humanitl sandbox apply`](#humanitl-sandbox-apply) Write the changes of a disposable run to the project folder and discard the rest
- [`humanitl sandbox discard`](#humanitl-sandbox-discard) Drop all changes of a disposable run. Asks first
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
- [`humanitl sessions`](#humanitl-sessions) What the agent left in the project during a sandbox run
- [`humanitl sessions summary`](#humanitl-sessions-summary) Show what a sandbox run changed in the project
- [`humanitl audit`](#humanitl-audit) Verify and export the audit chain
- [`humanitl audit verify`](#humanitl-audit-verify) Check the hash chain: every record, its predecessor and the anchors
- [`humanitl audit export`](#humanitl-audit-export) Write the records to a file, as JSON lines or as CSV
- [`humanitl config`](#humanitl-config) The resolved configuration and its schema
- [`humanitl config get`](#humanitl-config-get) Print the resolved value of one key, or of every key without one
- [`humanitl config set`](#humanitl-config-set) Write one key into config.toml, after checking it against the schema
- [`humanitl config schema`](#humanitl-config-schema) Print the JSON schema of the configuration, or the list of profiles
- [`humanitl config edit`](#humanitl-config-edit) Open config.toml in $VISUAL or $EDITOR and check it afterwards
- [`humanitl daemon`](#humanitl-daemon) The background service
- [`humanitl daemon status`](#humanitl-daemon-status) Ask the daemon who it is
- [`humanitl daemon install`](#humanitl-daemon-install) Write the systemd user unit that starts the daemon at login
- [`humanitl daemon logs`](#humanitl-daemon-logs) Show the journal of the daemon: journalctl --user -u humanitld
- [`humanitl daemon restart`](#humanitl-daemon-restart) Restart the background service: launchctl kickstart on macOS, systemctl --user restart on Linux
- [`humanitl daemon uninstall`](#humanitl-daemon-uninstall) Stop and disable the service and remove the unit daemon install wrote
- [`humanitl daemon retire-if-idle`](#humanitl-daemon-retire-if-idle) Ask the daemon to end now if no agent session holds it (after an upgrade)
- [`humanitl doctor`](#humanitl-doctor) Check this machine: one line per precondition, with a fix
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
| `--audit-anchor-every` | - | `INTEGER` | `audit.anchor_every` |
| `--audit-fsync-every` | - | `INTEGER` | `audit.fsync_every` |
| `--audit-max-bytes` | - | `INTEGER` | `audit.max_bytes` |
| `--audit-retention-days` | - | `INTEGER` | `audit.retention_days` |
| `--experimental-h2-upstream` | - | `true`, `false` | `experimental.h2_upstream` |
| `--findings-email-allow-domains` | - | `LIST_OF_STRING` | `findings.email_allow_domains` |
| `--findings-enabled` | - | `true`, `false` | `findings.enabled` |
| `--findings-ignored-hashes` | - | `LIST_OF_STRING` | `findings.ignored_hashes` |
| `--findings-user-terms` | - | `LIST_OF_STRING` | `findings.user_terms` |
| `--hold-ask-mode` | `--ask` | `ui`, `terminal`, `none` | `hold.ask_mode` |
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
| `--llm-active` | - | `STRING,_OPTIONAL` | `llm.active` |
| `--llm-api-key-ref` | - | `STRING,_OPTIONAL` | `llm.api_key_ref` |
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
| `--sandbox-mode` | - | `auto`, `direct`, `disposable` | `sandbox.mode` |
| `--sandbox-profile` | - | `STRING` | `sandbox.profile` |
| `--sandbox-work-dir` | `--work` | `STRING,_OPTIONAL` | `sandbox.work_dir` |
| `--sandbox-work-mode` | `--work-mode` | `ro`, `rw` | `sandbox.work_mode` |
| `--terminal-detach-prefix` | - | `STRING` | `terminal.detach_prefix` |
| `--tls-trust-system-store` | - | `true`, `false` | `tls.trust_system_store` |
| `--ui-language` | - | `en`, `de` | `ui.language` |
| `--ui-notifications` | - | `true`, `false` | `ui.notifications` |
| `--ui-terminal-notices` | - | `true`, `false` | `ui.terminal_notices` |
| `--ui-theme` | - | `dark`, `light`, `system` | `ui.theme` |

## humanitl

```text
Human-in-the-loop network moderation for AI agents

Usage: humanitl [OPTIONS] <COMMAND>

Commands:
  run       Start a session in the current directory, or show the session a profile resolves to
  sandbox   Start, plan and check the sandbox
  rules     List, add, change, reorder, dry-run and reload rules
  flows     The flows of this and earlier sessions
  sessions  What the agent left in the project during a sandbox run
  audit     Verify and export the audit chain
  config    The resolved configuration and its schema
  daemon    The background service
  doctor    Check this machine: one line per precondition, with a fix
  llm       Find LLM servers in the local network
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
          Keep the project folder untouched: it lies read-only under a layer in RAM, and after the run `humanitl sandbox changes` shows what the agent changed, `apply` writes it and `discard` drops it. The same as sandbox.mode = "disposable" for this run: where the system cannot do it, the start refuses instead of writing to the folder. Without this flag and without --direct the run follows sandbox.mode, by default "auto": disposable where the system allows it, otherwise direct with a notice

      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --direct
          Let the agent write to the project folder directly, as without the disposable sandbox. The same as sandbox.mode = "direct" for this run

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox

```text
Start, plan and check the sandbox

Usage: humanitl sandbox [OPTIONS] <COMMAND>

Commands:
  run      Start a sandbox with CMD as the agent and exit with its exit code
  argv     Print the bwrap command line for the current profile without starting it
  check    Start a short-lived sandbox and show the three isolation guarantees
  plan     Show what a start in this project would get, without starting: profile, project folder, mode, and the saved model server with the place its choice comes from (this run, the project, a profile or llm.active)
  attach   Attach this terminal to the running session
  exec     Open a further shell in the running sandbox, next to the agent
  changes  Show what a disposable run changed: files changed, created and deleted, with a diff for text files and size and hash for others
  apply    Write the changes of a disposable run to the project folder and discard the rest
  discard  Drop all changes of a disposable run. Asks first
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
          Run with the project folder read-only under a layer in RAM; at the end the changes are listed and discarded (this command keeps no layer; `humanitl run --disposable` keeps them for apply). Without this flag the run is direct unless sandbox.mode is "disposable": the default "auto" means direct here, because nothing could apply the changes afterwards

      --direct
          Run direct even if sandbox.mode is "disposable"

  -v, --verbose...
          Explain what is happening; repeat for more

  -q, --quiet
          Only the result, no notes

      --profile <NAME>
          Use this profile for the session; falls back to the sandbox profile of that name when no session profile exists

  -h, --help
          Print help
```

## humanitl sandbox argv

```text
Print the bwrap command line for the current profile without starting it

Usage: humanitl sandbox argv [OPTIONS] [-- <CMD>...]

Arguments:
  [CMD]...
          The command the line ends with; without it the agent from the configuration, otherwise `/bin/sh`

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

One client writes, any number watch. Ctrl+C goes to the session's terminal as byte 0x03, which interrupts its foreground program, as in any terminal; this terminal itself never reaches the sandbox. Detach with Ctrl+A d, as in screen; the session keeps running, and a later attach shows it. Ctrl+A Ctrl+A (or Ctrl+A a) sends one Ctrl+A to the agent.

Usage: humanitl sandbox attach [OPTIONS]

Options:
      --json
          Machine-readable output: one JSON value on stdout, diagnostics included

      --read-only
          Watch without sending anything. Ctrl+A d detaches here too, and so does Ctrl+C

      --config <PATH>
          Read this config file instead of the one in the user's config directory

      --detach-keys <SPEC>
          The prefix of the detach key: ctrl-a to ctrl-z (not ctrl-h, ctrl-i, ctrl-j, ctrl-m: Backspace, Tab and Enter send them), ctrl-\, ctrl-], ctrl-^, ctrl-_, or none for no detach key. Default from `terminal.detach_prefix`, which is ctrl-a

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

Like docker exec: the shell gets a terminal of its own, the agent keeps running and keeps its terminal. The shell carries the same limits as the agent (filter, mounts, network through the proxy, rules) and sees the same project folder. It ends when its process exits or when the sandbox stops; at most four live at a time. Detach with Ctrl+A d, attach again with `humanitl sandbox attach --shell ID`. Without a terminal on stdin the command only watches the output.

Usage: humanitl sandbox exec [OPTIONS] [-- <CMD>...]

Arguments:
  [CMD]...
          The command to run, after `--`; without it an interactive bash, or sh when the sandbox has no bash

Options:
      --detach-keys <SPEC>
          The prefix of the detach key, as for `humanitl sandbox attach`

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

ID is the end of the flow id, as many characters as it takes to tell the rows of this page apart and at least eight; `flows show` takes it as it stands. DECISION reads `aborted` where the client gave up its TLS connection to the proxy before there was anything to decide (nobody blocked it; `flows show` names the cause), and `passthrough` for the requests to the model endpoint, which only --passthrough lists.

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
What the agent left in the project during a sandbox run

Usage: humanitl sessions [OPTIONS] <COMMAND>

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
          Print help
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
  get     Print the resolved value of one key, or of every key without one
  set     Write one key into config.toml, after checking it against the schema
  schema  Print the JSON schema of the configuration, or the list of profiles
  edit    Open config.toml in $VISUAL or $EDITOR and check it afterwards
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
  ls      List the saved servers; the active one is marked
  add     Save a server. Its key follows with `humanitl llm key set --server NAME`
  use     Make a server the active one, from the next session on
  rename  Rename a server. Its key stays where it is
  rm      Remove a server and, unless --keep-key, its key from the keyring
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

