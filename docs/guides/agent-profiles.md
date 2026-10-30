# Agent profiles and adapters

Applies to: Humanitl 0.1

Three things decide what happens when Humanitl starts an agent:

| Piece | File | Decides |
|---|---|---|
| Session profile | `profiles/<name>.toml` | configuration values and rules of a session (`humanitl run --profile <name>`) |
| Sandbox profile | `profiles/sandbox/<name>.toml` | mounts, the one proxy socket, seccomp and the environment of the sandbox |
| Agent adapter | compiled into the daemon (`agent.adapter`) | the agent's command, its configuration files, its environment and its bundled rules |

This guide explains the session profile format, the OpenCode adapter line by
line, the pi adapter, how to write an adapter or run a plain script as the agent, and which
hosts the known agents contact on their own. Every configuration key is listed
in [CONFIG.md](../CONFIG.md), the rule language in the
[rules reference](../reference/rules.md), and the command line in the
[CLI reference](../reference/cli.md).

## Session profiles

A session profile bundles what makes up a session: the configuration values
that apply to it and the rules that belong to it. It is a TOML file, and it
exists in three places: bundled with the program, in your configuration
directory, and in a project. `humanitl run --profile llm-only` and the app
choose among the same profiles through the same loader.

### Format

```toml
name = "llm-only"
description = "Pure inference. Only the configured LLM endpoint is reachable."

[config.hold]                 # configuration values, by group
ask_mode = "none"
timeout_secs = 1

[config.sandbox]
profile = "default"           # the file profiles/sandbox/default.toml
work_mode = "rw"

[rules]
files = ["team.yaml"]         # relative to this profile file
inline = [                    # rules in the schema of rules.yaml
  { action = "block", match = { host = "**" }, expires = "never", note = "..." },
]
```

| Key | Type | Meaning |
|---|---|---|
| `name` | string | The profile's name. It must match the file name and `^[a-z0-9-]{1,32}$`; otherwise `CONFIG_003`. Without it, the file stem is the name. |
| `description` | string | One sentence on what the profile is for. Shown in `humanitl config schema --profiles`. |
| `[config.<group>]` | tables | Any configuration key from [CONFIG.md](../CONFIG.md), grouped as in `config.toml`: `[config.hold]`, `[config.sandbox]`, `[config.agent]` and so on. |
| `[rules].files` | list of paths | Rule files in the format of `rules.yaml`, resolved relative to the directory of the profile file, never to the working directory. |
| `[rules].inline` | list of tables | Rules written directly in the profile, in the schema of `rules.yaml`. |

There are exactly four top-level keys. Any other top-level block is
`CONFIG_002`. The most common mistake is a group one level too high: `[hold]`
instead of `[config.hold]`. The finding says so.

Every field is optional. What a profile does not name, it inherits from the
layer below, field by field: `[config.hold]` with only `timeout_secs` leaves
`ask_mode` alone. Lists replace and do not append: `rules.files` and
`llm.passthrough_paths` from a higher layer mean exactly the list that stands
there. A name is never a path: `--profile ../../etc/passwd` is `CONFIG_003`.

### Bundled profiles

| Profile | Purpose |
|---|---|
| `default` | Everything no rule decides is held and shown to a person. |
| `llm-only` | Pure inference: only the configured LLM endpoint is reachable, everything else is blocked without asking. No app needed. |

Both exist twice: as files under `profiles/` and embedded in the daemon. If the
files are missing, the daemon starts anyway. A file of the same name under
`~/.config/humanitl/profiles/` wins over the bundled one, and loading reports
it (`CONFIG_008`) as soon as its text differs from the bundled text.

**`default` sets no value on purpose.** It sits above `config.toml` in the
order of layers, so any value in it, even one that repeats the default, would
make your `config.toml` ineffective for that key without you seeing it. The
common settings stand as comments in `profiles/default.toml`: copy the file to
`~/.config/humanitl/profiles/default.toml` and remove the `#` to make it yours.

**`llm-only` does set values**, because choosing it is an explicit decision
that should override `config.toml`. `ask_mode = "none"` means a request that no
rule decides is not held: its deadline is zero, it times out at once, and the
agent receives `504` with `reason: timeout`. `timeout_secs = 1` keeps a
misconfiguration from hanging for five minutes. The adapter's passthrough rule
to the model is evaluated before the profile's block rule, wherever it stands
in the list; every other host falls on `block` with the note "llm-only
profile".

### Order of layers

From bottom to top; the higher layer wins. Every field remembers which layer
its value came from (`Origin`), and both the app and `humanitl config get
--origin` show it.

| Layer | Source | `Origin` |
|---|---|---|
| 1 | built-in defaults | `default` |
| 2 | `~/.config/humanitl/config.toml` | `config.toml` |
| 3 | profile `default`: the file, else the embedded version | `profile default` or `profile builtin default` |
| 4 | the chosen profile, if it is not `default` | `profile <name>` or `profile builtin <name>` |
| 5 | `<project>/.humanitl/profile.toml` | `project profile <path>` |
| 6 | environment variables `HUMANITL_*` | `env <name>` |
| 7 | command-line arguments | `command line` |

Layer 3 always applies, also when another profile is chosen; the chosen one
lies above it. Which profile takes layer 4 is decided in this order: the name
on the command line, else the `name` in the project profile, else `default`. A
name with neither a file nor a bundled profile is `CONFIG_001`, with the list
of known profiles. A file that exists but does not load counts as present: a
broken profile stops the start instead of vanishing silently.

The project directory is `sandbox.work_dir`, else the current directory. The
folder chosen in Setup or Settings is stored there, in `config.toml`, so the
app, the desktop launcher, `humanitl run` and `humanitl sandbox run` all start
in the same folder; `--work DIR` overrides it for one call and is not stored
(HUM-293). The key is locked at the project layer, so a project profile cannot
decide where it is looked for.

### What a project profile may not do

`<project>/.humanitl/profile.toml` lies in a cloned repository and is therefore
under the influence of whoever wrote that repository. It is held to four
limits:

1. **No other profile.** Its `name` chooses only among the bundled profiles.
   Any other wish is skipped and reported as `CONFIG_009`; whoever means a
   profile of their own names it with `--profile`.
2. **Only allowed keys.** Every key in the schema carries
   `x-project-scope: allowed | denied` (column "Project" in
   [CONFIG.md](../CONFIG.md)). A denied key at this layer is `CONFIG_003`.
   Denied are among others `llm.*`, `sandbox.*` (including `sandbox.profile`
   and `sandbox.env`), `agent.adapter`, `agent.command`, `hold.ask_mode`,
   `findings.enabled`, `pseudonyms.*`, `resolver.*`, `experimental.*` and
   `recorder.retention_days`.
3. **No mounts.** Mounts live in sandbox profiles, not in the configuration. A
   project profile naming `sandbox.mounts.extra_ro` or `sandbox.mounts.extra_rw`
   is refused with `CONFIG_003` and the sentence that only global profiles may
   bring host paths into the sandbox.
4. **No rules.** A `[rules]` block in a project profile is `CONFIG_003`. A
   repository does not decide what may leave the sandbox.

A project profile owned by another account is a warning (`CONFIG_007`), not a
refusal: the limits hold the file either way.

### Rules from a profile

Rules from `[rules].files` and `[rules].inline` are bundled rules. The order in
which a rule set is built is not the order in which it is evaluated. Evaluation
runs in four ranks: first the adapter's passthrough rule to the model, then the
session rules, then your saved rules from `rules.yaml`, last the bundled rules,
which are those of the adapter (`rules/default.yaml`) and those of the
profiles. Within a rank, the first matching rule wins. A request that no rule
matches is held (default `ask`).

So a profile cannot cover the passthrough, not even with `block host "**"`, and
you can override a bundled rule without deleting it. Only the loader marks a
rule as bundled: an inline rule with `bundled = true` loses the mark and gets a
warning (`RULES_010`). Details on ranks are in the
[rules reference](../reference/rules.md).

### On the command line

```sh
humanitl config schema --profiles            # which profiles exist
humanitl config get hold.ask_mode --profile llm-only --origin
humanitl run --profile llm-only              # start a session with it
```

`--profile NAME` names two different things, and the subcommand decides which:

| Command | `--profile` means |
|---|---|
| `humanitl sandbox run`, `sandbox argv`, `sandbox check` | the sandbox profile under `profiles/sandbox/` |
| everything else (`run`, `config`, `rules`, `flows`, `daemon`) | the session profile |

Under `run`, the sandbox profile is chosen with `--sandbox-profile` or
`sandbox.profile`.

### Writing your own profile

```sh
mkdir -p ~/.config/humanitl/profiles
cat > ~/.config/humanitl/profiles/work.toml <<'EOF'
name = "work"
description = "Shorter hold timeout, German interface."

[config.hold]
timeout_secs = 120

[config.ui]
language = "de"

[rules]
files = ["work-rules.yaml"]
EOF

humanitl config schema --profiles
humanitl run --profile work
```

`work-rules.yaml` is looked for next to `work.toml`, under
`~/.config/humanitl/profiles/`.

## Sandbox profiles

A sandbox profile is the complete policy of the sandbox. From it the daemon
builds one bubblewrap command line, which `humanitl sandbox argv` and the
Sandbox screen of the app show word for word. The bundled
`profiles/sandbox/default.toml` comments every line; in short:

| Table | What it holds |
|---|---|
| `[sandbox]` | backend, host name, the namespaces to unshare (all of them; a profile that leaves one out is refused), `min_bwrap_version` |
| `[mounts]` | `/work` and its mode, read-only system paths, tmpfs over directories that tools would run code from, masked files such as `/work/.env`, and your own `extra_ro` and `extra_rw` paths |
| `[network]` | the proxy socket in the sandbox, the proxy port, the CA path and exactly one bridge |
| `[seccomp]` | allowed socket families and types (a floor in both directions) and the syscalls that are always refused |
| `[env]` | the environment: proxy variables and the CA path for every common tool |

Your own sandbox profiles go to `~/.config/humanitl/profiles/sandbox/<name>.toml`.
Every host path in them is checked against a fixed deny list (`SANDBOX_006`),
so a profile cannot mount `~/.ssh` or the runtime directory. A second bridge or
another socket family is refused with `CONFIG_003`. Why these limits exist is
in [SECURITY.md](../SECURITY.md).

## The OpenCode adapter

OpenCode is the default adapter in Humanitl 0.1 (`agent.adapter = "opencode"`);
the second one is pi, see [Using pi](#using-pi).
Its code is `daemon/crates/sandbox/src/agent/opencode.rs`; everything it writes
into the sandbox lives under `agents/opencode/` and is compiled in. Nothing is
read from there at run time and nothing is downloaded.

OpenCode needs a provider for an OpenAI-compatible endpoint and, when it
starts, does several things on the network by itself: it fetches a model
catalog, checks for new versions, installs provider packages with npm and
offers a hosted web search. Nobody asked for any of that, and the first held
request a person sees should be one they understand. The adapter therefore
turns these off in three ways: through the environment, through its
configuration file, and through its bundled rules as a second lock. The values
were checked against OpenCode 1.18.25, the permissions and timeouts of HUM-308
against 1.18.31; other versions were not checked.

### The configuration template

`agents/opencode/opencode.json.tmpl`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "humanitl-local": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Humanitl local LLM",
      "options": {
        "baseURL": "{{LLM_BASE_URL}}"
      },
      "models": {}
    }
  },
  "model": "humanitl-local/{{DEFAULT_MODEL}}",
  "autoupdate": false,
  "share": "disabled",
  "permission": {
    "webfetch": "allow",
    "bash": "allow",
    "websearch": "deny",
    "external_directory": "deny"
  }
}
```

Line by line:

| Line | Meaning |
|---|---|
| `"$schema"` | The JSON schema of the OpenCode configuration, for editors and validation. |
| `"provider": { "humanitl-local": … }` | One provider, the model server you set in `llm.endpoint`. |
| `"npm": "@ai-sdk/openai-compatible"` | The provider package OpenCode loads for an OpenAI-compatible API. OpenCode installs it from `registry.npmjs.org` on first use; the bundled rules make that a held request (see [Known phone-home hosts](#known-phone-home-hosts)). |
| `"baseURL": "{{LLM_BASE_URL}}"` | Replaced with `llm.endpoint` plus `/v1`, unless the endpoint already ends in `/v1`. Without an endpoint, the adapter writes a placeholder on the sandbox's loopback. |
| `"models": {}` | Filled with one entry per model in `llm.models`. |
| `"model": "humanitl-local/{{DEFAULT_MODEL}}"` | The first model of `llm.models`. With an empty list, the placeholder model `default` and the warning `LLM_004`. |
| `"autoupdate": false` | No self-update. |
| `"share": "disabled"` | No session sharing, which would upload the whole conversation. |
| `"permission"` | OpenCode's own permissions; see below. |

The adapter parses the template as JSON and sets values; it never replaces
text. A model name with a quotation mark cannot inject structure into the
file. When an access key is stored (`humanitl llm key set`), the provider also
gets `"apiKey": "{file:/etc/humanitl/opencode/llm-api-key}"`; the key itself is
a separate read-only file and never stands in the configuration.

### Permissions

| Permission | Value | Why |
|---|---|---|
| `webfetch` | `allow` | OpenCode fetches without asking in its own interface. The fetch goes through Humanitl and is held like any other request, so you decide once, in Intercept, not twice (HUM-308). |
| `bash` | `allow` | Commands run without a prompt of the agent. They run inside the sandbox; a `curl` among them is held at the proxy like any other request. |
| `websearch` | `deny` | The search tool talks to a hosted search service, not to the page. |
| `external_directory` | `deny` | No access to directories outside the project. Inside the sandbox there is little to reach, but the agent should not try. |

`edit` is deliberately not in the block. OpenCode allows it by default, and
because this block is merged over every agent's own permissions, an explicit
`"edit": "allow"` would also let OpenCode's plan agent write, which is meant to
be read-only. Why the agent's own prompts protect nothing inside the sandbox is
in [SECURITY.md](../SECURITY.md#the-agents-own-permission-prompts-hum-308).

**How long a tool waits.** A held request waits up to `hold.timeout_secs` (300
seconds by default). OpenCode's `webfetch` gives up after 30 seconds, or after
at most 120 when the model asks for longer; in OpenCode 1.18.31 that limit is
fixed in its code. When a tool gives up, it closes the connection, the proxy
ends the flow as `client_timeout` and the card leaves the queue in Intercept before
you decided. The briefing tells the agent so and that `curl` waits, unless the model
passes its own timeout. For `bash`
the adapter raises the default timeout with
`OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS` (see the next table).

The same `permission` block also goes into the environment variable
`OPENCODE_PERMISSION`, which OpenCode merges over every configuration source
last. That matters because of how OpenCode merges its configuration: later
sources win, in this order: the configuration directory, `OPENCODE_CONFIG`,
**then** the `opencode.json` of the project tree, `.opencode` directories,
`OPENCODE_CONFIG_CONTENT`, an organisation's configuration, and last the
managed directory (`/etc/opencode` on Linux). A cloned repository would sit
above `OPENCODE_CONFIG`. Only the managed directory and `OPENCODE_PERMISSION`
sit above it, so the adapter writes the same file to three places and sets the
variable.

The `provider` block cannot be enforced this way: the merge is additive, and a
project can add its own provider with its own address. No guarantee falls with
that, because all traffic there goes through the proxy and is held; which model
is preset is still decided by Humanitl.

### Ctrl+C does not quit OpenCode

OpenCode 1.18.31 quits on `ctrl+c,ctrl+d,<leader>q` by default, and in
Humanitl's terminal `Ctrl+C` reaches the agent as `0x03`, so a reflexive
`Ctrl+C` to copy ended the session. Since HUM-353 the app's terminal copies
on `Ctrl+C` while visible text is selected and sends nothing then; without
a selection, and in `humanitl attach`, `Ctrl+C` still reaches the agent. The adapter writes
`agents/opencode/tui.json.tmpl` as `~/.config/opencode/tui.json` with
`"keybinds": { "app_exit": "ctrl+d,<leader>q" }` (HUM-353). Quit with
`Ctrl+D` on an empty prompt or `Ctrl+X q`; `Ctrl+C` still clears the input
(`input_clear`) and `Esc` still interrupts (`session_interrupt`). The file
is not in the managed directory, so a `tui.json` in your project wins and
can bind `app_exit` differently. Checked against the strings of the
OpenCode 1.18.31 binary only.

### Files in the sandbox

| Path in the sandbox | Content |
|---|---|
| `/etc/humanitl/opencode/opencode.json` | the rendered configuration (`OPENCODE_CONFIG` points here) |
| `~/.config/opencode/opencode.json` | the same file, in OpenCode's configuration directory |
| `/etc/opencode/opencode.json` | the same file, in the managed directory that wins over the project |
| `/etc/humanitl/opencode/models.json` | the model catalog, rendered from `agents/opencode/models.json` |
| `/etc/humanitl/opencode/llm-api-key` | the access key, only if one is stored |
| `~/.config/opencode/tui.json` | the key bindings of the interface: `app_exit` without `ctrl+c` (HUM-353) |
| `~/.config/opencode/AGENTS.md` | the briefing of the agent in the language of `ui.language`, unless `agent.briefing.enabled = false` |

`~` is the agent's home `/home/agent`, an empty tmpfs. Every file comes from a
sealed memfd and is mounted read-only. No adapter file may lie under `/work`:
Humanitl never writes into your repository, and the agent must not be able to
rewrite its own configuration with a commit.

`models.json` contains no data from any public catalog. It describes exactly
one provider, `humanitl-local`, with placeholder values for `release_date`
(`1970-01-01`) and the limits (`context` 32768, `output` 4096). Adjust them in
`agents/opencode/models.json` if you know your model.

### Environment

The adapter adds these variables to the environment of the sandbox profile:

| Variable | Value | Effect |
|---|---|---|
| `OPENCODE_DISABLE_AUTOUPDATE` | `true` | No version check against GitHub. The rule `…0002` blocks the release API as a second lock. |
| `OPENCODE_MODELS_PATH` | `/etc/humanitl/opencode/models.json` | OpenCode reads its model catalog from this file instead of the network. |
| `OPENCODE_DISABLE_MODELS_FETCH` | `true` | The hourly background refresh of the catalog does not run. |
| `OPENCODE_CONFIG` | `/etc/humanitl/opencode/opencode.json` | The configuration outside `/work`. |
| `OPENCODE_AUTO_SHARE` | `false` | No automatic sharing of a session. |
| `OPENCODE_DISABLE_SHARE` | `true` | No sharing at all. |
| `OPENCODE_ENABLE_EXA` | `false` | No hosted web search (Exa). |
| `OPENCODE_ENABLE_PARALLEL` | `false` | No hosted web search (Parallel). |
| `OPENCODE_DISABLE_LSP_DOWNLOAD` | `true` | No language servers downloaded from the network. |
| `OPENCODE_PERMISSION` | the `permission` block as JSON | Merged over every configuration source last. |
| `OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS` | `hold.timeout_secs` plus 60 seconds, in milliseconds, at least `120000` (`360000` by default) | A command in `bash` waits for a held request instead of being cut after OpenCode's 120 seconds. Not set with `hold.ask_mode = "none"`, where nothing waits. The model can still pass its own timeout for one call. |
| `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` | under `/home/agent` | OpenCode's state stays in the tmpfs, not on the host. |
| `NODE_EXTRA_CA_CERTS` | `/etc/humanitl/ca.crt` | Node trusts the local CA of the proxy. |
| `TERM`, `COLORTERM`, `LANG` | `xterm-256color`, `truecolor`, `C.UTF-8` | The terminal of the agent. |

**`OPENCODE_MODELS_URL` is deliberately not set.** Its value is a base address
(default `https://models.opencode.ai`) to which OpenCode appends `/api.json`
and fetches it over HTTP; a `file://` URL does not work there. Pointing it at a
local file would have needed a second bridge into the sandbox. With
`OPENCODE_MODELS_PATH` and `OPENCODE_DISABLE_MODELS_FETCH` the sandbox keeps its
one door.

`sandbox.env` is applied after the adapter and wins. If you set `HOME` or
`XDG_CONFIG_HOME` there, the adapter's files follow to that place.

### The passthrough rule

From `llm.endpoint` the adapter builds one rule with a fixed id (`…00ff`):
allow `POST` and `GET` to exactly that host, scheme and port, on the inference
paths only (`/v1/chat/completions`, `/v1/completions`, `/v1/responses`,
`/v1/embeddings`, `/v1/models`, and for Ollama `/api/chat`, `/api/generate`,
`/api/embed`, `/api/embeddings`, `/api/tags`, `/api/show`, `/api/ps`,
`/api/version`), with `allow_private`, because the model usually sits in your
own network. The proxy never holds these requests; it records them in full and
warns about findings instead of stopping them (`LLM_005`). Paths that change
the server, such as `/api/pull` or `/v1/files`, go through the queue. To let
one of them pass, add its path to `llm.passthrough_paths`.

### Checks before the start

Before anything starts, the adapter checks on the host, without touching the
network:

- `AGENT_001`: `opencode` is not in `PATH` and `agent.command` is not set.
- `AGENT_002`: the file is there but not executable.
- `AGENT_004`: the binary is on the host but not visible in the sandbox,
  for example under `~/.opencode/bin`. The fix copies it to `/usr/local/bin`.
- `LLM_004`: `llm.models` is empty, so the agent gets a placeholder model.

OpenCode is a full-screen TUI. `humanitl run --ask terminal` therefore refuses
it with `CLI_002`; decide in the app (`--ask ui`) or let unruled requests be
blocked (`--ask none`). All codes are explained in
[DIAGNOSTICS.md](../DIAGNOSTICS.md).

## Using pi

pi (<https://pi.dev/>, npm package `@earendil-works/pi-coding-agent`) is the
second adapter (`agent.adapter = "pi"`). Its code is
`daemon/crates/sandbox/src/agent/pi.rs` and `pi_config.rs`; its templates live
under `agents/pi/` and are compiled in. The values below were measured on pi
1.3.0; other versions were not checked.

### Install

pi is a Node.js script (`#!/usr/bin/env node`, Node.js 22.19 or newer). The
sandbox mounts `/usr` read-only and nothing under your home, and the daemon
binds only native programs from a per-user directory, never a script. Install
both pi and Node.js under `/usr`:

```sh
sudo npm install -g --ignore-scripts @earendil-works/pi-coding-agent
```

A global npm install lands under `/usr/local` (or `/usr` with a distribution's
npm). The installer script from `pi.dev` puts pi under `$HOME`, which the
sandbox does not see; a Node.js from nvm under `$HOME` is not visible either.

### Select

```sh
humanitl config set agent.adapter pi
```

or `agent.adapter = "pi"` in `config.toml`, or the "Agent" group of the
settings form. The daemon refuses an id it does not know with `CONFIG_003` and
names the known ones. The choice applies to the next session.

### What Humanitl writes

Everything pi reads lives in its agent directory, `PI_CODING_AGENT_DIR`, here
`/home/agent/.pi/agent`. pi also writes there at every start (`auth.json`,
`models-store.json`, `sessions/`, lock directories), so the directory stays
writable in the tmpfs home; only the files Humanitl places are read-only.

| Path in the sandbox | Content |
|---|---|
| `~/.pi/agent/models.json` | one provider `humanitl-local` with `baseUrl` from `llm.endpoint` (the same base URL OpenCode gets, path prefix included), `api: "openai-completions"`, one model per entry of `llm.models` |
| `~/.pi/agent/settings.json` | `defaultProvider = "humanitl-local"`, `defaultModel` = the first model, `enableInstallTelemetry = false`; with web access also `packages` = the directory of `pi-web-access` |
| `~/.pi/agent/web-search.json` | only with web access: `ssrf.trustEnvProxy = true` (see "Web search and fetch") |
| `/etc/humanitl/pi/llm-api-key` | the access key, only if one is stored |
| `~/.pi/agent/AGENTS.md` | the briefing in the language of `ui.language`, unless `agent.briefing.enabled = false` |

Both JSON files are parsed and rebuilt with a JSON library, never filled in as
text: a model name with a quotation mark cannot inject structure. With an empty
`llm.models` pi gets the placeholder model `default` and the warning
`LLM_004`. The limits of every model (`contextWindow` 32768, `maxTokens` 4096)
are cautious defaults; adjust them in `agents/pi/models.json.tmpl` if you know
your model.

The endpoint, the models and the key are those of the active saved model
server, exactly as for OpenCode: the key only goes with the endpoint of the
same entry, and a switch applies to the next session.

### Key handling

`apiKey` in `models.json` is `"!cat \"$HUMANITL_PI_KEY_FILE\""`. pi runs a
value that starts with `!` through `/bin/sh -c` and takes the trimmed output as
the key. The key file comes from a sealed memfd like every adapter file; the
key stands in no environment variable, not in the `argv` of `pi` and not in
`models.json`. Only the path stands in the `argv` of the short-lived `cat`. The
path is a constant of the adapter, never a value from the configuration, so
nothing a user or a project writes reaches that shell line.

Without a stored key, `apiKey` is the placeholder `humanitl-no-key`: pi hides
a provider without a key and would start with no model at all. A server
without authentication ignores the value.

### Environment

| Variable | Value | Effect |
|---|---|---|
| `PI_CODING_AGENT_DIR` | `/home/agent/.pi/agent` | Where pi reads and writes its files. |
| `PI_OFFLINE` | `1` | No automatic network activity: version check, catalogue refresh, package update check, tool download. Model calls are not affected. |
| `PI_SKIP_VERSION_CHECK` | `1` | No request to `pi.dev/api/latest-version`. |
| `PI_TELEMETRY` | `0` | No install telemetry. |
| `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` | under `/home/agent` | pi's state stays in the tmpfs. |
| `NODE_EXTRA_CA_CERTS` | `/etc/humanitl/ca.crt` | Node trusts the local CA of the proxy. |
| `TERM`, `COLORTERM`, `LANG` | `xterm-256color`, `truecolor`, `C.UTF-8` | The terminal of the agent. |

No variable carries a secret, and all four `PI_*` variables are shown by name
in the environment list of the sandbox screen. `sandbox.env` wins over the
adapter: if you set `PI_CODING_AGENT_DIR` there, pi reads none of the files
Humanitl writes.

### What pi does on its own

| Host | What | Rule | Also switched off by |
|---|---|---|---|
| `pi.dev`, path `/api/**` | version check, model catalogue, `pi update` installer | block (`…0101`) | `PI_OFFLINE`, `PI_SKIP_VERSION_CHECK` |
| `radius.pi.dev` | bug report upload (`/bug`), Radius session sharing | block (`…0102`) | nothing; only on such a command |
| `github.com`, path `/sharkdp/fd/releases/**` | download of `fd` when it is missing | block (`…0103`) | `PI_OFFLINE` |
| `github.com`, path `/BurntSushi/ripgrep/releases/**` | download of `rg` when it is missing | block (`…0104`) | `PI_OFFLINE` |

With `PI_OFFLINE=1` a logging proxy saw no request at all before the first
prompt. Other user-triggered paths (`/share` as a GitHub gist, `pi update`
through npm, packages from a trusted project) are not blocked and go through
the queue like any other request. Without `fd`, pi prints once "Offline mode
enabled, skipping download" and works without it.

### Project trust

A project's `.pi/settings.json`, extensions and skills load only after pi's own
trust prompt, which you answer in the TUI; Humanitl does not override
`defaultProjectTrust`. A trusted project can choose another default model or
add a provider; every request still goes through the proxy and is held.
`models.json` is read from the agent directory only, so a project cannot
rewrite the base URL of the bundled provider.

### Web search and fetch

pi has no web tool of its own. The npm package `pi-web-access` (MIT,
<https://github.com/nicobailon/pi-web-access>, measured with 0.46.0) adds
`web_search`, `fetch_content`, `source_check` and `get_search_content`.
`pi install npm:pi-web-access` does not help here: it writes to `~/.pi/agent`
on your machine, and pi's agent directory in the sandbox is a fresh tmpfs.
Install the package globally, next to pi:

```sh
sudo npm install -g --ignore-scripts pi-web-access
```

Humanitl looks for it in the `node_modules` directory that holds pi, then
under `/usr/local/lib/node_modules` and `/usr/lib/node_modules`, and counts a
directory only if the sandbox reaches its `package.json` and that file names
`pi-web-access`. When found, `settings.json` lists the directory under
`packages`, and pi loads it from there read-only, without installing
anything. The switch is `agent.pi.web_access`:

| Value | Effect |
|---|---|
| `auto` (default) | load the package when it is installed, otherwise start without it and say nothing |
| `on` | the same, but a missing package gives the warning `AGENT_007` with the npm command |
| `off` | never load it |

```sh
humanitl config set agent.pi.web_access off
```

The key is closed to project profiles: loading third-party code into the
agent is your decision, not the cloned repository's. The settings form has no
field for it yet.

**Every request still passes the proxy.** It is held in Intercept when no
rule decides it; a rule decides without a hold (the bundled `**.exa.ai` rule
blocks keyless search, an `allow` rule forwards at once). The extension uses Node's
`fetch`, which pi routes through its own `EnvHttpProxyAgent` on `HTTPS_PROXY`.
Measured with a logging proxy: `fetch_content` on an `http://` page arrived as
`CONNECT host:80`, keyless search as `CONNECT mcp.exa.ai:443`. The extension
checks each target with its own DNS lookup before it fetches; the sandbox has
no resolver, so every fetch failed with `Failed to resolve` until
`web-search.json` set `ssrf.trustEnvProxy`. That file is read-only: the
extension's own commands that save a provider choice fail in the sandbox.

**Keys and search.** Fetching a URL needs no key. Without a key, search
goes to Exa's keyless MCP endpoint `mcp.exa.ai`, and the bundled rule
`…0006` blocks `**.exa.ai` for every session (it was written for OpenCode's
hosted search). So out of the box `fetch_content` works and `web_search`
reports that the Exa search failed. A rule of yours wins over the bundled one; to let
search through, add one for the host, for example `ask` with
`host: "mcp.exa.ai"` in `rules.yaml`. Search providers with an API key (Brave,
Tavily, Jina and others) are not wired up: no key from the keyring reaches
the sandbox, and the extension finds none in the cleared environment.

### Terminal and checks

pi is a full-screen TUI. `humanitl run --ask terminal` refuses it with
`CLI_002`, as for OpenCode. A single `Ctrl+C` does not quit pi: it clears
the editor (`app.clear`), and only a second one within 500 ms exits, as
does `Ctrl+D` on an empty editor (`app.exit`). The adapter therefore
leaves pi's key bindings alone (HUM-353, checked against pi 1.4.0). Before
the start the adapter checks:

- `AGENT_001`: `pi` is not found; the fix is the npm command above.
- `AGENT_002`: as for OpenCode, naming pi.
- `AGENT_004`: pi lies on the host where the sandbox does not look. The fix is
  the npm command above; copying the one script would leave its package
  files behind.
- `AGENT_006`: `pi` is reachable, but its `#!` line asks for `node`, and the
  sandbox finds no startable `node`. The fix adds a mounted `node` to
  `sandbox.env.PATH`, copies the host's `node` under `/usr/local/bin`, or
  points at the Node.js download page. When the `#!` line names an absolute
  `node` outside the mounts, the fix is reinstalling pi with a Node.js under
  `/usr` or `/usr/local`.
- `AGENT_007` (warning): `agent.pi.web_access = "on"`, and `pi-web-access`
  is not installed where the sandbox sees it; pi starts without web tools.
- `LLM_004`: `llm.models` is empty.

## Writing an adapter

An adapter is Rust code in the crate `humanitl-sandbox`, registered in
`AdapterRegistry::builtin()` (`daemon/crates/sandbox/src/agent/mod.rs`). There
is no plugin loading: adapters are compiled into the daemon. The trait, exactly
as in the code:

```rust
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn command(&self, ctx: &AgentContext) -> Vec<OsString>;
    fn env(&self, ctx: &AgentContext) -> Vec<(String, String)>;
    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic>;
    fn default_rules(&self) -> &'static str;
    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule>;
    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic>;
    fn program(&self) -> &'static str;
    fn is_agent_command(&self, command: &[std::ffi::OsString]) -> bool { /* provided */ }
    fn is_fullscreen_tui(&self) -> bool;
}
```

| Method | Contract |
|---|---|
| `id` | The value of `agent.adapter` that selects this adapter. |
| `command` | The command the shim starts after seccomp. If `ctx.agent_command_override` is set (`agent.command`), exactly that. |
| `env` | Variables in addition to the environment of the sandbox profile. |
| `files` | Files that must exist in the sandbox before `exec`, as `SandboxFile::read_only` or `SandboxFile::secret`. Never under `/work`, never on a path the sandbox sets itself (proxy socket, CA, shim, `/proc`, `/sys`, `/dev`, `/run/humanitl`). A broken bundled template is `AGENT_003`. |
| `default_rules` | The adapter's bundled rules as `rules.yaml` text. The daemon parses it; the sandbox crate does not depend on the rules crate. |
| `llm_passthrough` | The rule for `llm.endpoint`, or `None` without a usable endpoint. Copy `OpenCodeAdapter::llm_passthrough`: narrow paths, exact host, `allow_private`, `passthrough_llm`. |
| `preflight` | Checks on the host before the start. File system only, never the network. A finding with severity `Blocking` prevents the start. |
| `program` | The program name without a path. `is_agent_command` compares the file name of a command against it, so `opencode`, `/usr/local/bin/opencode` and `./opencode` are the same agent. |
| `is_fullscreen_tui` | `true` makes `--ask terminal` refuse with `CLI_002`. |

`AgentContext` carries the session, the project path on the host and in the
sandbox (`/work`), `llm` (endpoint and models), `agent_command_override`, the
proxy port, the CA path in the sandbox, the language, `hold`, the briefing
switch, the host `PATH` for the preflight and the agent's home.

Three promises no compiler checks, and every adapter keeps them:

1. **No new door.** An adapter adds no bridge and no seccomp family. Both come
   from the sandbox profile, and it knows exactly one bridge.
2. **Nothing under `/work`.** `SandboxFile::is_outside_work` and
   `files_inside_work` check it.
3. **No network in the preflight.** Whether the model answers is the job of
   the endpoint probe (`humanitl llm test`), not of the adapter.

A minimal adapter for a line-oriented agent `echo-agent` that reads a
configuration file:

```rust
use std::ffi::OsString;

use humanitl_config::LlmConfig;
use humanitl_core::Diagnostic;
use humanitl_core::rule::Rule;

use crate::agent::{AgentAdapter, AgentContext, SandboxFile};

/// An adapter for `echo-agent`.
pub struct EchoAdapter;

impl AgentAdapter for EchoAdapter {
    fn id(&self) -> &'static str {
        "echo"
    }

    fn program(&self) -> &'static str {
        "echo-agent"
    }

    fn command(&self, ctx: &AgentContext) -> Vec<OsString> {
        ctx.agent_command_override
            .clone()
            .filter(|command| !command.is_empty())
            .unwrap_or_else(|| vec![OsString::from("echo-agent")])
    }

    fn env(&self, _ctx: &AgentContext) -> Vec<(String, String)> {
        vec![("ECHO_AGENT_CONFIG".to_owned(), "/etc/humanitl/echo/config.toml".to_owned())]
    }

    fn files(&self, _ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        Ok(vec![SandboxFile::read_only(
            "/etc/humanitl/echo/config.toml",
            b"telemetry = false\n".to_vec(),
        )])
    }

    fn default_rules(&self) -> &'static str {
        "version: 1\nrules: []\n"
    }

    fn llm_passthrough(&self, _llm: &LlmConfig) -> Option<Rule> {
        // Without a passthrough every request to the model is held.
        None
    }

    fn preflight(&self, _ctx: &AgentContext) -> Vec<Diagnostic> {
        Vec::new()
    }

    fn is_fullscreen_tui(&self) -> bool {
        false
    }
}
```

Register it in `AdapterRegistry::builtin()`, select it with
`agent.adapter = "echo"` and test it the way `daemon/crates/sandbox/tests/opencode_adapter.rs`
tests OpenCode: every method is a pure function of `AgentContext`, so a test
needs no sandbox.

### A shell script as the agent

You do not need an adapter to run your own program. Anything after `--` runs
in the sandbox instead of the configured agent:

```sh
cat > agent.sh <<'EOF'
#!/bin/sh
# The smallest possible agent: one request, and print what came back.
# HTTPS_PROXY and the CA variables are set by the sandbox profile.
set -u
curl -sS https://example.com/ | head -n 5
EOF

humanitl run --ask terminal -- /bin/sh /work/agent.sh
```

The project directory is `/work` in the sandbox, so the script in the current
directory is `/work/agent.sh`. The request to `example.com` is held; with
`--ask terminal` the decision box appears in the same terminal, because the
script is not a full-screen program. Allow it and the script prints the start
of the page; block it and it prints the block body:

```
Blocked by Humanitl.
reason: user
flow: <flow id>
host: example.com
```

Under `humanitl run`, such a command gets the same adapter contribution as
the agent itself (HUM-280): the configured adapter's files, its variables and,
when a key is stored, the key file. The configured agent is bound beside it and
stands first on `PATH`, so `humanitl run -- bash` is a shell in which
`opencode` (or `pi`) starts with the configured model and key. The command you
name is never bound itself; it must be startable from the sandbox's mounts. If
the key store does not give out the key, a command of your own still starts,
without the key, and the start says so with `LLM_009` as a warning. Only the
daemon-less `humanitl sandbox run` keeps the older rule: there a command that
is not the agent gets nothing from the adapter. To make your program the
default, set `agent.command` in `config.toml`, for example
`agent.command = ["/usr/local/bin/my-agent"]`.

## Known phone-home hosts

What an agent contacts by itself, without the person asking for it. The list
comes from the bundled rules in `rules/default.yaml`, which every session
loads, whatever the adapter, and from the variables of the adapter. Each rule carries a note
with the reasoning; `humanitl rules list --all` and the "Bundled rules" group
in the Rules screen show them.

### OpenCode

| Host | What | Rule | Also switched off by |
|---|---|---|---|
| `models.opencode.ai` | model catalog, used by OpenCode 1.18.25 | block (`…0009`) | `OPENCODE_MODELS_PATH`, `OPENCODE_DISABLE_MODELS_FETCH` |
| `models.dev` | model catalog named by the OpenCode documentation | block (`…0001`) | the same |
| `api.github.com`, path `/repos/anomalyco/opencode/releases/**` | update check | block (`…0002`) | `OPENCODE_DISABLE_AUTOUPDATE`, `"autoupdate": false` |
| `opncd.ai` | session sharing upload (`POST /api/share`) | block (`…0005`) | `OPENCODE_DISABLE_SHARE`, `"share": "disabled"` |
| `opencode.ai`, path `/share/**` | reading share links | block (`…000a`) | the same |
| `**.exa.ai` | hosted web search | block (`…0006`) | `OPENCODE_ENABLE_EXA=false`, `"websearch": "deny"` |
| `**.parallel.ai` | hosted web search | block (`…0007`) | `OPENCODE_ENABLE_PARALLEL=false` |
| `registry.npmjs.org`, `GET` and `HEAD` | provider packages such as `@ai-sdk/openai-compatible`, installed on first use | ask (`…0008`): held and shown, so you learn what it is | nothing; the package is needed |

Language-server downloads are switched off with
`OPENCODE_DISABLE_LSP_DOWNLOAD`; their hosts are not catalogued, so if one ever
appears, it is held like any other request.

### pi

| Host | What | Rule | Also switched off by |
|---|---|---|---|
| `pi.dev`, path `/api/**` | version check, model catalogue, installer | block (`…0101`) | `PI_OFFLINE`, `PI_SKIP_VERSION_CHECK` |
| `radius.pi.dev` | bug report upload, Radius session sharing | block (`…0102`) | nothing; only on such a command |
| `github.com`, path `/sharkdp/fd/releases/**` | `fd` download | block (`…0103`) | `PI_OFFLINE` |
| `github.com`, path `/BurntSushi/ripgrep/releases/**` | `rg` download | block (`…0104`) | `PI_OFFLINE` |

### Other agents

Humanitl 0.1 has no adapter for Aider, Codex or Claude Code, and catalogues no
hosts for them. Two bundled rules are locks for such agents and for later
OpenCode versions rather than observations:

| Host | Why | Rule |
|---|---|---|
| `**.posthog.com` | product analytics used by several agent CLIs; does not appear in the OpenCode 1.18.25 binary | block (`…0003`) |
| `**.sentry.io` | crash reports with stack traces and file paths; OpenCode 1.18.25 calls no such address | block (`…0004`) |

Every other host an agent contacts is held, since a request that no rule
matches defaults to `ask`. That is how a new phone-home host shows up: in the
queue, with a name, before anything leaves the machine.

A bundled rule cannot be edited or deleted, but you can switch it off with
`humanitl rules disable <id>` or override it with a rule of your own, which is
evaluated first.
