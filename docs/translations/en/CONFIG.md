---
source: CONFIG.md
source-sha256: a21936d2f82403b17cd85bdbbf47d9653d09b9ad27b7fbf1c0db1d801f2ae09e
lang: en
---
# Configuration

Applies to: Humanitl 0.1

<!-- Erzeugt aus daemon/crates/config/src/model.rs.
     Nicht von Hand ändern: `UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs` schreibt die Datei neu. -->

Humanitl has one configuration source: the Rust types in `humanitl-config`. From them come
the JSON schema, the validation on load, the settings screen and this page. Anyone who
wants to change a field changes the type.

## Visibility tiers

| Tier | Where it appears |
|---|---|
| `basic` | Always visible, on the first view of the settings. |
| `advanced` | Behind "Show more". |
| `expert` | Only in `config.toml` and here. |

## Project profile

`<projekt>/.humanitl/profile.toml` lives in the cloned repository and is therefore
attacker-controlled: whoever clones a repository runs its profile. The column
"Project" states for every field whether the project profile may set it (in the JSON schema
`x-project-scope`).

| Project | Meaning |
|---|---|
| `allowed` | The project profile may set the value. |
| `denied` | Only defaults, `config.toml`, the global profile, the environment or the command line may set the value. In the project profile the key is `CONFIG_003`, under an old name too. |

Two values are checked on load beyond their type: `llm.endpoint` accepts only
`http` or `https`; `sandbox.work_dir` must be absolute, without `..`, and an
existing directory that can be canonicalised. Otherwise both are `CONFIG_003`.

## Effect

This page is generated from the schema and therefore cannot forget a key. Whether a
key does anything is not the schema's business: a value that is described and validated
but that nobody reads would look here like any other. The column "Effect" therefore
states it explicitly (in the JSON schema `x-pending-issue`, HUM-101).

| Effect | Meaning |
|---|---|
| `yes` | The key is read and takes effect. |
| `pending (HUM-xxx)` | The key has no reader today. The named issue decides it, by wiring it in or by removing it; until then its value changes nothing. The register `daemon/crates/config/tests/config_readers.rs` records for every key which of the two cases applies. |

## Order of sources

From bottom to top; the upper layer wins. Every field remembers which layer its value
comes from, and the interface shows it.

| Layer | Source |
|---|---|
| 1 | built-in defaults |
| 2 | `$XDG_CONFIG_HOME/humanitl/config.toml` |
| 3 | profile `default`: `$XDG_CONFIG_HOME/humanitl/profiles/default.toml`, otherwise the embedded version |
| 4 | the selected profile, if it is not `default`; file, otherwise embedded |
| 5 | `<projekt>/.humanitl/profile.toml`, block `[config]`; only fields with Project `allowed` |
| 6 | environment variables `HUMANITL_*` |
| 7 | command-line arguments |

Besides `[config]`, a profile has only `name`, `description` and `[rules]` (HUM-066,
`docs/profiles.md`). Every other block at the top level is `CONFIG_002`; a
group such as `[hold]` belongs under `[config.hold]` in a profile. The bundled profile
`default` deliberately sets no value: it sits above `config.toml` and would otherwise make
it ineffective for every key it names.

`<projekt>` is `sandbox.work_dir`, otherwise the current directory, not the other way round:
whoever works with `--work` from some other directory gets the profile of the
project they are working on. The key is locked at the project layer, so the project
profile cannot decide where it is looked for. Its `name` only chooses
among the bundled profiles; any other wish is skipped and reported with
`CONFIG_009` (`docs/profiles.md`).

An environment variable is named like its path in upper case, with `__` between the
levels: `hold.timeout_secs` becomes `HUMANITL_HOLD__TIMEOUT_SECS`. The value is read according
to the type of the field: for a text field it stays text, otherwise it is read as a boolean,
then as a number, otherwise as a string; a list stands in square brackets.
An unknown key in a file or on the command line is an error
(`CONFIG_002`); in the environment it is a warning that does not abort loading. Variables
without `__` in the name (`HUMANITL_GALLERY`, `HUMANITL_ESCAPE_MARKER`) belong to other
tools and are skipped.

## Fields

### `agent`

Which agent runs in the sandbox.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `agent.adapter` | string | `"opencode"` | advanced | denied | yes | Identifier of the adapter, for example `opencode`. |
| `agent.briefing.enabled` | boolean | `true` | advanced | allowed | yes | Creates the agent's instruction file in the sandbox. |
| `agent.command` | list of string, optional | `-` | expert | denied | yes | Replaces the adapter's command line entirely. Empty means: the adapter's own. |
| `agent.pi.web_access` | auto \| on \| off | `"auto"` | advanced | denied | yes | Loads the pi extension `pi-web-access` (web search and fetching pages) if it is installed globally (`sudo npm install -g --ignore-scripts pi-web-access`). `auto` loads it if the sandbox finds it, `on` does the same and reports `AGENT_007` if it is missing, `off` never loads it. Every request of the extension goes through the proxy like any other. Locked in the project profile: loading third-party code into the agent is the user's decision. |

### `audit`

The audit chain: anchors, synchronisation, retention.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `audit.anchor_every` | integer | `100` | advanced | denied | yes | How often a record is an anchor (every n-th), in the file and in the database at the same time. Someone with access to the file can cut off records after the last anchor at the end without the verification noticing; a smaller value makes this window smaller. The daemon always anchors when it shuts down. |
| `audit.fsync_every` | integer | `50` | expert | denied | yes | After how many records the daemon forces the audit log to disk (fsync). It happens anyway after one second at the latest and before every anchor. |
| `audit.max_bytes` | integer | `0` | expert | denied | yes | Upper limit for the size of the audit log in bytes; 0 means no limit, otherwise 2097152 (2 MiB) to 1099511627776 (1 TiB). The same run as for `audit.retention_days`, at start-up and daily after that, deletes whole records at the beginning of the chain, oldest first, until the file, including the record `audit.pruned` and its anchor, is below the limit; between two runs it can grow beyond it. The newest record always stays; if it is longer than 1 MiB, the log can stay above the limit with it, and the daemon reports this in its log. A chain that does not hold is not shortened. The run briefly needs space for a copy of the remainder next to the file; if that is missing, the log stays as it was and the next run tries again. |
| `audit.retention_days` | integer | `0` | expert | denied | yes | Days a record is kept in the audit log; 0 means forever. A daily run deletes the records at the beginning of the chain that are older and documents the cut with a record `audit.pruned` including an anchor; the verification accepts a beginning documented this way and keeps reporting a beginning without it as a break. A chain that does not hold is not shortened. What the deleted records contained, the chain no longer proves afterwards; the anchors in `audit_anchors` stay. |

### `experimental`

Switches for unfinished paths. Everything here may disappear without notice.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `experimental.h2_upstream` | boolean | `false` | expert | denied | yes | Offers HTTP/2 to the upstream. In M1 the proxy speaks only HTTP/1.1 upstream. |

### `findings`

Detection of secrets and personal data.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `findings.email_allow_domains` | list of string | `[]` | advanced | denied | yes | Domains whose email addresses are not a finding, for example your own company. |
| `findings.enabled` | boolean | `true` | advanced | denied | yes | Switches detection off entirely. Off means: no highlights, no pseudonyms. |
| `findings.ignored_hashes` | list of string | `[]` | expert | denied | yes | Checksums (SHA-256, hex) of individual values that never appear as a finding again. |
| `findings.user_terms` | list of string | `[]` | basic | allowed | yes | Your own terms that count as a finding, for example a project name or a client name. |

### `hold`

How long and by which route the human is asked before a request continues.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `hold.ask_mode` | ui \| terminal \| none | `"ui"` | advanced | denied | yes | Where the question is asked: in the interface, in the terminal or not at all. |
| `hold.hard_block_checksum_secrets` | boolean | `false` | advanced | denied | yes | Lets requests with checksum-verified secrets out only when edited; no rule and no approval sends them unchanged. |
| `hold.timeout_secs` | integer | `300` | basic | allowed | yes | Seconds a held request waits for a decision before it ends as timed out. |

### `limits`

All caps and time limits in one place.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `limits.body_timeout_secs` | integer | `300` | expert | allowed | yes | Seconds of silence between two chunks of a body, not its total duration. Applies in both directions: to the client's request body (after that 408) and to the upstream's streamed response body (after that the stream is aborted and the recording is marked as truncated). A large upload and a long model stream may therefore take as long as they like, as long as they do not fall silent. |
| `limits.connect_timeout_secs` | integer | `10` | advanced | allowed | yes | Seconds until the connection to the upstream is established. |
| `limits.event_buffer` | integer | `1024` | expert | allowed | yes | Length of the event queue per client. If it overflows, the daemon reports `Lagged`. |
| `limits.header_timeout_secs` | integer | `30` | expert | allowed | yes | Seconds within which the client must have sent its request headers. On a keep-alive connection this is also the deadline until the next request, and so the only idle limit of the connection to the agent; while a request is held, it does not run. |
| `limits.hold_body_cap_bytes` | integer | `33554432` | advanced | allowed | yes | Largest request whose body is kept in memory for the decision. Above that the proxy answers with 413, unless a rule `allow` or `ask` with `stream: true` matches the request and `hold.hard_block_checksum_secrets` is off: then the body goes out streamed and unchecked. |
| `limits.hold_max_bytes` | integer | `268435456` | advanced | allowed | yes | Largest sum of the bodies of all held flows. Above that the proxy answers with 503. |
| `limits.hold_max_flows` | integer | `200` | advanced | allowed | yes | Largest number of flows held at the same time. Above that the proxy answers with 503. |
| `limits.max_client_connections` | integer | `256` | expert | allowed | yes | Largest number of simultaneous connections from the sandbox per session. Above that the proxy answers with 503 and closes; a clock per span alone does not stop a process from tying up the same resources over many connections. |
| `limits.max_decompress_ratio` | integer | `100` | expert | allowed | yes | Highest permitted ratio of decompressed to compressed bytes of a preview. |
| `limits.preview_cap_bytes` | integer | `8388608` | expert | allowed | yes | Largest amount of body that the interface receives as a preview. |
| `limits.recorder_max_body_bytes` | integer | `33554432` | expert | denied | yes | Largest body that the recording stores as a blob. Everything above that is only noted with a checksum. |

### `llm`

The local LLM endpoint and what counts as passthrough.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `llm.active` | string, optional | `-` | advanced | denied | yes | The name of the active entry from `llm.servers`. Only its endpoint gets the stored access key and counts as LLM passthrough. Empty as long as there are no entries. |
| `llm.api_key_ref` | string, optional | `-` | advanced | denied | yes | Reference to the endpoint's access key, never the key itself: `secret-service:humanitl/llm` for the system keyring, on macOS `keychain:humanitl/llm`, or `file:llm` for a file with `0600` under `$XDG_DATA_HOME/humanitl/keys`. It is set with `humanitl llm key set` or in the settings; the agent gets the key as a file in the sandbox, never via the command line. Empty means: no key. |
| `llm.endpoint` | string, optional | `-` | basic | denied | yes | OpenAI-compatible endpoint in the LAN. Traffic there is not held, but it is logged. |
| `llm.models` | list of string | `[]` | basic | denied | yes | Models the endpoint offers. Empty means: the agent gets a placeholder model and a warning. |
| `llm.passthrough_paths` | list of string | `["/v1/","/api/"]` | advanced | denied | yes | Path prefixes that count as LLM passthrough. A prefix should name one endpoint, not a whole API surface: the agent adapter therefore replaces `/v1/` and `/api/` with the endpoints that do inference, so that `POST /api/pull` and `POST /v1/files` do not go out without asking. A path that names more stays as it stands here. |
| `llm.server` | string, optional | `-` | advanced | allowed | yes | The name of an entry from `llm.servers` that applies to the session instead of `llm.active`: set in the project profile, in a profile or for one run (`humanitl run --llm NAME`). It only chooses among the saved servers and never defines an endpoint, a model or a key itself. Order: run before project before profile before `llm.active`. A name that does not exist keeps the session from starting. |
| `llm.servers` | table of table | `{}` | advanced | denied | yes | Saved model servers, one entry per name with `endpoint`, `models` and `api_key_ref` (`secret-service:humanitl/NAME`, on macOS `keychain:humanitl/NAME`). The entry that `llm.active` names supplies `llm.endpoint`, `llm.models` and `llm.api_key_ref`; the flat keys are its view. A name has 1 to 32 characters `a-z`, `0-9`, `-` and `_`. The table is maintained with `humanitl llm server` or in the settings; a change applies from the next session on. |

### `pseudonyms`

Swapping pseudonyms back in responses.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `pseudonyms.max_response_bytes` | integer | `8388608` | expert | denied | pending (HUM-079) | Largest response that is buffered for swapping back. Everything above that passes through unchanged. |
| `pseudonyms.translate_responses` | boolean | `true` | advanced | denied | pending (HUM-079) | Replaces pseudonyms in text responses with the original value again. |

### `recorder`

Recording of the flows.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `recorder.inline_max_bytes` | integer | `262144` | expert | allowed | yes | Bodies up to this size are stored in the database, larger ones as a file in the blob store. |
| `recorder.retention_days` | integer | `180` | advanced | denied | yes | Days a recording (requests, responses, bodies) is kept; 0 means never delete. A daily run deletes what is older and notes every deletion in the audit log (`recorder.retention_applied`). The period implements the storage limitation of GDPR Art. 5(1)(e): personal data in requests does not stay around longer than necessary. This run never deletes the audit chain. |

### `resolver`

Name resolution after the decision.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `resolver.cache_ttl_secs` | integer | `300` | expert | denied | yes | Seconds an answer stays in the cache. |
| `resolver.nameserver` | string, optional | `-` | expert | denied | yes | Name server as `IP:Port`; without a port, 53 applies. When set, the daemon asks only this server after an approval, over UDP with TCP fallback, and neither `/etc/resolv.conf` nor `/etc/hosts`. Empty means: the system's name service. |
| `resolver.overrides` | table of string | `{}` | expert | denied | yes | Fixed mappings from host name to address, before any query. When not empty, start-up reports it as `CONFIG_016`, so that the test lever does not sit unnoticed in an everyday run. |
| `resolver.prefer` | ipv4 \| ipv6 | `"ipv4"` | expert | denied | yes | Which address family is preferred when both are available. |
| `resolver.test_ca` | string, optional | `-` | expert | denied | yes | Additional CA for tests, as PEM and with an absolute path. Applies only if the daemon was started with `--allow-test-ca`; without the flag the key is not accepted, and a path that is not absolute ends start-up instead of being resolved against the working directory. Set it only in test runs, never in everyday use. |

### `sandbox`

Which sandbox profile starts with which work directory.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `sandbox.env` | table of string | `{}` | advanced | denied | yes | Additional environment variables for the sandbox; they override entries of the same name from the profile's `[env]`. The key can be set from the process environment and is therefore only as trustworthy as the shell Humanitl starts from; the dynamic linker's variables (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`) are therefore refused, as they would run before the shim's seccomp filter. |
| `sandbox.mode` | direct \| disposable | `"direct"` | basic | denied | yes | Where the agent's changes go. `direct` writes into the project directory; `disposable` puts it read-only beneath a layer in memory, and after the run the human decides which changes to keep (`humanitl sandbox changes`, `apply`, `discard`). `disposable` exists only on Linux for now (bubblewrap overlay); on macOS start-up refuses with `SANDBOX_030`. |
| `sandbox.profile` | string | `"default"` | advanced | denied | yes | Name of the profile under `profiles/sandbox/`, without extension. |
| `sandbox.work_dir` | string, optional | `-` | basic | denied | yes | Project directory that is mounted as `/work`. Empty means: the current directory. |
| `sandbox.work_mode` | ro \| rw | `"rw"` | basic | denied | yes | Whether the agent may write in the project directory. |

### `terminal`

The command line's terminal when it is attached to a session.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `terminal.detach_prefix` | string | `"ctrl-a"` | advanced | denied | yes | Prefix of the key with which `humanitl sandbox attach` detaches: `ctrl-a` to `ctrl-z`, `ctrl-\`, `ctrl-]`, `ctrl-^`, `ctrl-_`, or `none`, in which case there is none. After the prefix, `d` detaches and leaves the session running; the prefix twice or its letter sends it once to the agent, any other key passes on unchanged together with the prefix. `ctrl-[` is excluded because every arrow key begins with this byte, and `ctrl-h`, `ctrl-i`, `ctrl-j` and `ctrl-m` because backspace, tab and enter send the same bytes. `--detach-keys` overrides the value for one invocation. |

### `tls`

Whom the daemon trusts on the way to the upstream.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `tls.trust_system_store` | boolean | `true` | advanced | denied | yes | Trusts, for the upstream, the roots from the system's certificate store (`/etc/ssl/certs`), like `curl` and the browser: a company CA or a CA installed with `update-ca-certificates` is accepted. When off, only the public roots from `webpki-roots` apply. Takes effect after the daemon restarts; the project profile cannot set the key. |

### `ui`

Language, appearance and notifications of the interface.

| Key | Type | Default | Tier | Project | Effect | Description |
|---|---|---|---|---|---|---|
| `ui.language` | en \| de | `"en"` | basic | allowed | yes | Language of the interface. |
| `ui.notifications` | boolean | `true` | advanced | allowed | yes | System notification when a request is waiting and the window is not in front. |
| `ui.terminal_notices` | boolean | `true` | advanced | allowed | yes | The daemon's notice line when one of the agent's requests is waiting for a decision or has been decided. It goes to every client as a frame of its own, never into the agent's byte stream: whoever sits at the terminal gets it written between the bytes, and the interface shows it in the strip above the terminal. The strip stays in any case, because it comes from the event stream; this switch only affects the frame. |
| `ui.theme` | dark \| light \| system | `"dark"` | advanced | allowed | yes | Appearance of the interface. |

## Old names

These keys keep working. If the old name stands next to the current one, the current
one wins, and loading adds a diagnostic about it.

| Old | Current | Since |
|---|---|---|
| `hold.body_cap_bytes` | `limits.hold_body_cap_bytes` | HUM-057 |
| `ipc.event_buffer` | `limits.event_buffer` | HUM-057 |
| `preview.cap_bytes` | `limits.preview_cap_bytes` | HUM-057 |
| `preview.max_decompress_ratio` | `limits.max_decompress_ratio` | HUM-057 |
| `recorder.max_body_bytes` | `limits.recorder_max_body_bytes` | HUM-057 |
| `upstream.connect_timeout_secs` | `limits.connect_timeout_secs` | HUM-057 |

## Removed keys

These keys existed once and no longer exist. They have no successor. Whoever
still has them in a file gets a warning on load (`CONFIG_005`) with
the issue and the reason; the value is skipped, and the daemon starts anyway. A
hard error would here be the penalty for a decision the user did not
make (`backlog/CONVENTIONS.md` 4.25).

The column "Form" states what happens to whatever stands **beneath** the key. For
a value there never was a level there: `[schlüssel.irgendwas]` is not an old
entry but an unknown one, and still fails with `CONFIG_002`. For a
table, its keys belonged to the user; it disappears as a whole, in a
single warning.

| Key | Removed with | Form | Reason (text of the diagnostic) |
|---|---|---|---|
| `experimental.upstream_port_map` | HUM-088 | table | the proxy never redirected a port, and no shipped test needs one: they bind the real port inside their own network namespace or address the ephemeral port directly |
| `experimental.ws_hold` | HUM-121 | value | no upgrade was ever held by it: a WebSocket upgrade is decided by the rule alone, and the proxy never read the switch |
| `limits.idle_timeout_secs` | HUM-101 | value | it described the same span as limits.header_timeout_secs, the one idle clock of the connection to the agent |
| `ui.sound` | HUM-121 | value | there is no sound the switch could have turned off: nothing in the user interface ever played one |

## Paths

| What | Where |
|---|---|
| Configuration | `$XDG_CONFIG_HOME/humanitl/config.toml` |
| Rules | `$XDG_CONFIG_HOME/humanitl/rules.yaml` |
| Profiles | `$XDG_CONFIG_HOME/humanitl/profiles/<name>.toml` |
| Project profile | `<projekt>/.humanitl/profile.toml` |
| Database | `$XDG_DATA_HOME/humanitl/humanitl.db` |
| Blobs | `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>` |
| Audit | `$XDG_DATA_HOME/humanitl/audit/audit.jsonl` |
| CA | `$XDG_DATA_HOME/humanitl/ca/ca.crt`, `ca.key` (0600) |
| Daemon socket | `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (0600, directory 0700) |
| Proxy socket | `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` |
| Token | `$XDG_RUNTIME_DIR/humanitl/token` (0600) |

If `XDG_RUNTIME_DIR` is missing, `/run/user/<uid>` is used; if that is missing too, Humanitl
falls back to `$TMPDIR/humanitl-<uid>` and reports `CONFIG_004` as a notice.
