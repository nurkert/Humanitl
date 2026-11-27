# The agent depot

Applies to: Humanitl 0.1

By default Humanitl takes the agent from this machine: the adapter finds its program on the
daemon's `PATH` or in your bin directories and binds that one file read-only into the sandbox.
The **agent depot** is the other way: Humanitl installs the agent once under its own data
directory, inside a sandbox, and every session takes it from there. The agent then never has to
exist on this machine at all.

At the time of HUM-522, Claude Code from the depot runs with an API key or a gateway only; a
subscription login inside the depot is not possible yet (see [Current limits](#current-limits)).

This guide says when the depot is worth it, how to switch, what it protects and what it does not,
and what works today. The decision behind it is [ADR-0023](../adr/0023-agent-depot.md); the
security statement is [SECURITY, the agent depot](../SECURITY.md#the-agent-depot-hum-518-adr-0023)
and [section 5.7](../SECURITY.md#57-the-credential-file-of-the-depot); the threat is
[K-36](../THREAT-MODEL.md#k-36-the-agent-depot). Every command is in
[`docs/cli.md`](../cli.md#humanitl-agents) and the [CLI reference](../reference/cli.md).

## When to use the depot

Use it when:

- **you do not want the agent on this machine.** An agent installed on the host can run outside
  any sandbox, update itself and read your whole home directory. From the depot it only ever runs
  inside a Humanitl sandbox; nothing from the depot is started on the host, not even
  `--version` (status and `doctor` read the version from the depot's `manifest.json`).
- **you want to choose when the agent changes.** The version is pinned. Only `humanitl agents
  update` changes it, a running session keeps the version it started with, and inside a session
  the program directory is read-only, so the agent cannot update itself.
- **your company requires it.** A company policy can lock the source to the depot (see
  [Company policy](#company-policy)).

Stay with the host when you run the agent outside Humanitl anyway, or where the depot cannot run
yet: under the kernel level a depot session is refused, and on macOS (Seatbelt) the install run is
refused, so a depot cannot be filled there (`AGENT_015`, see [Current limits](#current-limits)).

## What works today

| Agent | Install into the depot | Login inside the depot | Session from the depot |
|---|---|---|---|
| Claude Code (`claude`) | yes, from `downloads.claude.ai`, the vendor's signed `manifest.json` verified with the host's `gpg` | not yet: `humanitl agents login claude` answers `AGENT_014` | yes, with the access key Humanitl keeps (below) |
| Codex (`codex`) | yes, the vendor's whole package from `releases.openai.com`, checked against its list of checksums (no signature) | declared (device login), not tried with the real `codex` | from the depot, but see [Current limits](#current-limits) |
| OpenCode (`opencode`) | yes, the release archive from GitHub, checked against GitHub's SHA-256 of an immutable release (no signature, no vendor list) | yes: `humanitl agents login opencode` runs `opencode auth login` in its terminal; pick a provider and paste its key | yes, with the logins stored in the depot |
| pi | no recipe: `AGENT_015` | no | no |
| Antigravity CLI (`agy`) | no: `AGENT_015` names why (no chosen version can be fetched, no signed check) | none, API key only: `humanitl agents login agy` answers `AGENT_014` | no, there is no install |

Why, and what follows, is in [Current limits](#current-limits).

## Claude Code from the depot, step by step

Prerequisites, besides a working Humanitl on Linux ([install](install.md)): `gpg` on the host
(`/usr/bin/gpg` or `/usr/local/bin/gpg`), because the daemon verifies the vendor's signature
itself, and `curl` under `/usr`, because the install run downloads with the host's `/usr`
mounted read-only. Claude Code itself is not needed on the host, and it does not hurt there
either: with the depot, the host install locations the recipe lists (`/usr/bin/claude`,
`/usr/local/bin/claude`, `/usr/lib/node_modules/@anthropic-ai/claude-code`,
`/usr/local/lib/node_modules/@anthropic-ai/claude-code`) are covered in every session.

### 1. Switch the source

```sh
humanitl agents source claude depot
```

This writes `agents.claude.source = "depot"` to your `config.toml` (the same as `humanitl config
set agents.claude.source depot`). `humanitl agents source claude host` switches back and keeps
the depot until `humanitl agents remove claude`. `agents.source` sets the default for every
agent. None of these keys can be set by a project profile.

For a single project, the first-start screen (`humanitl go`, or "New" in the start window of the
app) has a field "Source" beside the agent: it stores the source in the project choice, above
`config.toml`. When you have not set a source, it preselects the depot for an agent installed only
there; a source you set stays as it is. A start of that
project then takes the stored source, and never the other one. Without a project choice the
setting above applies.

### 2. Install

```sh
humanitl agents install claude                  # the vendor's current version
humanitl agents install claude --version 2.1.90 # or a fixed one
```

The command asks first (`--yes` does not), then attaches this terminal to the **maintenance
run**: a sandbox with the three guarantees and the proxy, without a project. Only
`downloads.claude.ai` passes without a hold, as a rule of this run that goes when it ends; any
other request is held as in any session. The run only downloads: manifest, signature, the
vendor's key and the program. The vendor's installer is not executed, and nothing lands in your
`~/.local/bin`. After the run the daemon checks the downloaded tree, verifies the signature of
`manifest.json` against the fingerprint fixed in Humanitl, compares the program's SHA-256 with the
manifest and only then switches the depot to the new version. A run that fails changes nothing.

```sh
humanitl agents status claude
```

shows the source, the version and `verified`.

### 3. Give Claude Code its access

You need an Anthropic API key, created in the Anthropic Console, or the address and key of a
gateway. Claude Code in the depot cannot keep a subscription login yet (see
[Current limits](#current-limits)). It gets its access from Humanitl instead, the same way as from
the host:

```sh
humanitl config set agent.adapter claude
humanitl llm key set          # reads the Anthropic API key from stdin
```

The key lives in the system keyring (or a `0600` file where there is none), never in the
configuration file. Each session gets it as a file inside the sandbox; a small launcher in front
of `claude` moves it into `ANTHROPIC_API_KEY`, so it stands in no command line. Without
`llm.endpoint` Claude Code talks to `api.anthropic.com`. To use a gateway instead, save it as a
server (`humanitl llm server add NAME URL --model MODEL --use`, then `humanitl llm key set
--server NAME`): Claude Code then gets `ANTHROPIC_BASE_URL`, the key as `ANTHROPIC_AUTH_TOKEN` and
the first model as `ANTHROPIC_MODEL`. Either way only the inference endpoint passes without a
hold ([SECURITY 5.1](../SECURITY.md#51-the-llm-passthrough)).

### 4. Start, twice

```sh
cd ~/projects/example
humanitl go
```

The first start of Claude Code with a provider outside this machine asks once whether that is
intended (cloud inference); the answer is kept for this agent and provider. Claude Code then
starts from the depot in a disposable session. Start it again: Humanitl does not ask for the cloud
notice again, and it hands the key from its keyring to every session, so no login is needed for
the access. That is shown in CI with a stand-in program for the API-key and gateway path (see
[What CI proves](#what-ci-proves)); what a real Claude Code asks on its own at a start with an
empty configuration directory is not measured yet (see below).

Each session works on its own copy of the depot's configuration; what the agent writes there goes
with the session. A setting meant to stay goes through `humanitl agents config claude`: the agent
runs on a copy, you see every changed file with the keys that run commands highlighted (`hooks`,
`apiKeyHelper`, `statusLine`, `mcpServers`, `env` and more), and only your confirmation takes it
over.

What a real Claude Code shows on its very first start with an API key and an empty configuration
directory, and whether that repeats in every fresh copy, has not been measured yet; the owner
walkthrough does that (HUM-553). CI proves the chain with a stand-in program in place of Claude
Code (below).

### 5. Update, go back, remove

```sh
humanitl agents update claude                   # the vendor's current version
humanitl agents update claude --version 2.1.90  # back to a version the depot still holds, without a run
humanitl agents remove claude                   # the whole depot: program, configuration, login
```

One maintenance run at a time, and only while no session runs in the daemon (`CLI_005`).
`remove` asks first and is refused while a session or a run uses the depot (`AGENT_018`); the
source setting stays as it is.

## What the depot protects

- **No agent on the host.** The program lives under `$XDG_DATA_HOME/humanitl/agents/<id>/`, owned
  by the daemon's user, directories `0700`, files `0600` or `0700`, checked on every open
  (`AGENT_013`). It runs only inside a sandbox.
- **A verified, pinned version.** For Claude Code the daemon checks the vendor's signature with
  the host's `gpg` and the program's SHA-256. Codex signs nothing; there the daemon checks its own
  copy of the vendor's chain: the release metadata names the SHA-256 of
  `codex-package_SHA256SUMS`, that list names the SHA-256 of the package archive. What vouches for
  the chain is TLS to `releases.openai.com`, the same trust the vendor's own installer has.
  OpenCode publishes neither; there the daemon checks GitHub's release metadata: the release must
  be immutable, and the archive must have the SHA-256 GitHub names for it. That anchor is weaker:
  the SHA-256 is GitHub's, computed at upload, not the vendor's (ADR-0023, amendment 2026-11-27). The
  session mounts the resolved version directory read-only, never the `current` link, so an update
  never changes a running session.
- **Nothing from a session persists in the depot.** A session gets a writable scratch copy of the
  depot's configuration, which is deleted when it ends. The only thing that may go back into the
  depot is the one credential file the adapter declares, and only through the guard of the
  write-back (next point). Claude Code declares none yet, so nothing at all goes back into the
  depot.
- **The write-back guard.** Where an adapter declares the format of its credential file, the
  daemon reads exactly that file with `O_NOFOLLOW`, at most 64 KiB, checks every key against the
  format, refuses a token older than the stored one, refuses **another account** (only a login run
  you start can change the account), writes its own serialisation atomically and logs every
  outcome with SHA-256 only (`agent.credential_writeback`). `agents.<id>.credential_writeback =
  false` turns the write-back off.
- **One way to the agent.** With `depot`, a session that names a host copy of the agent, by path,
  through a wrapper or a bind, is refused (`AGENT_017`), and the known host install locations are
  covered inside the sandbox.

## What the depot does not protect

- **Your project.** The depot covers the agent's program and configuration, not `/work`. What the
  agent writes into the project stays there like in any session; a disposable session lets you
  review it before it is applied ([SECURITY 5.2](../SECURITY.md#52-the-project-directory-work)).
- **Your own user on the host.** The depot is files of your user. Any other process running as
  you can change them, like any file in your home ([SECURITY 5.6](../SECURITY.md#56-other-known-limits),
  [out of scope](../SECURITY.md#out-of-scope)). The depot keeps the agent away from the host; it
  does not keep the host away from the depot.
- **A side channel through the credential file.** Within its format the credential file is the
  session's: a compromised session can write a token of the same account that does not work, or
  one that carries a few bytes for the next session. That is a covert channel between sessions of
  the same agent, bounded by the format and 64 KiB, visible in the audit log, and closed by
  `credential_writeback = false` at the price of a new login when the token expires
  ([SECURITY 5.7](../SECURITY.md#57-the-credential-file-of-the-depot)).
- **The account a login run stores.** The guard compares accounts only against the stored file: a
  session that writes another account is refused (`AGENT_016`), but a login run you start may
  store any account, because changing the account is what a login is for.
- **Parallel sessions with a rotating token.** Two sessions renewing the same login with a vendor
  that rotates its refresh token can invalidate each other's token; then log in again.
- **The credential file at rest.** It lies unencrypted in the depot, `0600`, like the vendor's own
  file on the host. Encryption with a key from the keyring is planned (HUM-523).
- **A deliberate copy.** The company lock keeps Humanitl from starting a host agent. It is no
  defence against someone who copies an agent binary to a path no recipe lists.
- **Inference.** The depot changes where the program comes from, not where its requests go.
  Everything the agent sends to its model leaves through the passthrough as before.

## Company policy

ADR-0023 lets a company policy lock the source to `depot`, per agent or for all; a lock may only
name `depot`, never `host` (`AGENT_010`). While locked, `humanitl agents source`, `humanitl config
set`, the settings in the app and every other way of writing the source refuse `host` alike, app
and CLI show the setting as fixed by the company, and the host install locations are covered in
every session. The resolver and every write path already take the lock as an input. **The policy
file itself does not exist yet** (BACKLOG section 9, item 21): until it does, no lock is active,
and the depot is only what you choose.

## Current limits

- **Claude Code login.** The vendor does not document the keys of `.credentials.json` or which
  fields of `.claude.json` a session needs to start logged in. Humanitl never guesses a format, so
  until they are measured in the owner walkthrough (HUM-553, item 3) `humanitl agents login claude`
  answers `AGENT_014`, and Claude Code from the depot runs with an API key or a gateway only.
- **Codex.** `humanitl agents install codex [--version V]` takes the vendor's package
  (`codex-package-<target>.tar.gz`) whole: `bin/codex` beside its helpers, started through the
  link `codex -> bin/codex`, as the vendor's installer does (HUM-566). The run checks the archive
  against the list before `tar` unpacks it, and the daemon checks the chain again on its own copy;
  the binding between the archive and the unpacked tree rests on the run, which executes nothing
  it downloaded. Whether the real `codex` runs from there, and the device login, are measured in
  the owner walkthrough (HUM-553, item 4). A session from the depot reads `config.toml` from the
  depot's copy, which does not yet carry `sandbox_mode = "danger-full-access"`; Codex's own
  sandbox cannot start inside Humanitl's, so set the key with `humanitl agents config codex`
  until the adapter puts its defaults there itself.
- **Antigravity CLI.** The adapter exists (HUM-517). `agy` keeps its login in the system keyring,
  and the host's keyring never reaches a sandbox, so in the depot it runs with a Gemini API key only
  (`humanitl llm key set`); `humanitl agents login agy` answers `AGENT_014`. `humanitl agents
  install agy` answers `AGENT_015` with the reason (decided in HUM-566): the vendor's manifest names
  the current version and nothing fetches a chosen one, so neither `--version` nor an update to a
  chosen version is possible; it points at the program on `storage.googleapis.com`, a host of every
  Cloud Storage bucket; and the only check is a SHA-512 in that same unsigned manifest. A
  configuration run answers `AGENT_015` as well: no variable moves `agy`'s `~/.gemini`, so the depot
  carries only the program, and each session writes its settings afresh.
- **OpenCode.** `humanitl agents install opencode [--version V]` takes the release archive
  (`opencode-<target>.tar.gz`, on macOS `.zip`) from `github.com/anomalyco/opencode/releases`
  and keeps the one program `opencode` from its top (#110). Only OpenCode 1 is installed, as the
  vendor's own installer does; without `--version` the run takes the newest `v1.x.y` of the release
  list. On x86_64 it takes the `baseline` build, which runs on every x86_64 CPU. The check is
  GitHub's digest of an immutable release, a weaker anchor than a signature or a vendor's list
  ([What the depot protects](#what-the-depot-protects)). OpenCode keeps its logins in `auth.json`
  under its data directory, so the depot's copy is its `XDG_DATA_HOME` and the credential file is
  `opencode/auth.json`; the adapter's own configuration (`XDG_CONFIG_HOME/opencode`,
  `OPENCODE_CONFIG`) is written as for a host install. **Logins:** `humanitl agents login
  opencode` runs `opencode auth login` in the run's terminal; choose a provider and paste its key
  (a provider's own browser login reaches hosts the recipe does not name, and they are held like any
  request). Only this run stores `auth.json` and adds a provider. A session renews what is stored
  (a refreshed OAuth token goes back through the guard), but it never adds or drops a provider: a
  `/connect` inside a session works for that session only, and when it ends the guard refuses the
  file (`AGENT_012`, "the file holds other entries than the stored one … a new provider comes only
  through a login run") and the stored logins stay as they were. Add the provider with `humanitl
  agents login opencode` instead. The guard checks `auth.json` against OpenCode's schema: each
  entry a provider with `type` `oauth`, `api` or `wellknown` and exactly that kind's keys; any
  other kind or key is refused. **No configuration run:** `humanitl agents config opencode`
  answers `AGENT_015`, because the copy is OpenCode's data directory (sessions, logs, the model
  catalog's cache), not its configuration. Sources, read 2026-11-27: the vendor's
  `install` and `packages/core/src/global.ts`, `packages/opencode/src/auth/index.ts`,
  `packages/core/src/flag/flag.ts`, `packages/opencode/src/cli/cmd/providers.ts` at tag `v1.19.6`
  (`github.com/anomalyco/opencode`, commit `f4e363c`), and GitHub's REST documentation of
  releases (`immutable`, `assets[].digest`).
- **pi** has no depot recipe; with `depot` it answers `AGENT_015`.
- **Backends.** The kernel level refuses a depot session; Seatbelt (macOS) and the kernel level
  refuse install, update, login and configuration runs (`AGENT_015`).
- **One run at a time**, and only while no session runs in the daemon (`CLI_005`).

## What CI proves

`daemon/crates/ipc/tests/depot_maintenance.rs` runs the chain in a real sandbox behind the real
proxy, with nothing leaving the machine: the source switched through the RPC,
`install claude --version 2.1.90` with Humanitl's own Claude Code recipe against a stand-in of
`downloads.claude.ai` (a stand-in program and a manifest signed with a test key, verified by the
host's `gpg`; only the fingerprint differs from the real recipe), `login claude` refused with
`AGENT_014`, two sessions in a row that reach a stand-in endpoint through `ANTHROPIC_BASE_URL`
with the stored key and never reach the stand-in's login prompt, each on a fresh copy of the
configuration, then `remove claude`, after which no file named `claude` is left under the test's
home, and the directories of its `PATH` hold what they held before. A real release, its real
signature and a real subscription login are not part of CI.
