# ADR-0023 · Agent depot: an agent installed and logged in once, inside Humanitl only

Applies to: Humanitl 0.1

Status: Proposed
Date: 2026-11-16

## Context

Today the agent program comes from the host. The adapter finds it on the
daemon's `PATH` or in the user's bin directories, and the daemon binds that
one native file read-only at `/run/humanitl/agent/<name>` (HUM-278,
`daemon/crates/sandbox/src/agent/locate.rs`); scripts and npm entry points are
not bound. The agent's home in the sandbox is a tmpfs, the adapter's own files
come in read-only (`/etc/humanitl`, `$XDG_CONFIG_HOME/<agent>`), and the
host's login of the agent (`~/.claude`, `~/.codex`, the keyring) never reaches
the sandbox (HUM-514, HUM-515, HUM-517).

That has two consequences the owner does not want (2026-11-16):

- To run Claude Code, Codex or `agy` in Humanitl, the user first installs it
  on the host, where it can run outside any sandbox, update itself and read
  the whole home directory. The owner does not want such agents on the host
  at all.
- A login made inside a sandbox is lost with the sandbox. Every disposable
  session asks for the login again, and every project for its settings.

Hard constraints:

- The three sandbox guarantees and `tests/escape/` stay as they are; the
  depot adds no door and no mount the agent can write through to the host.
- The default stays today's behaviour. Nothing changes for a user who does
  not choose the depot.
- Every capability is an RPC first; app and CLI are thin clients (ADR-0018).
- No own cryptography; no new port (ADR-0015).
- The main new threat is persistence: whatever one session writes into the
  depot runs in every later session of every project.

## Decision

We add an optional **agent depot**: a store per agent, owned by the daemon,
under its data directory (`$XDG_DATA_HOME/humanitl/agents/<id>/`, mode
`0700`), never under a path the host's shell or the agent's host install
uses. The agent is installed and logged in there once; every Humanitl sandbox
of that agent then takes program, configuration and login from the depot.

**Nothing of this is on by default.** `agents.source = "host"` stays the
default for every agent, and the adapter finds the program on the host as
today.

1. **Configuration** (all `x-project-scope = denied`):
   - `agents.source`: `"host"` (default) or `"depot"`, the default for every
     agent.
   - `agents.<id>.source`: the same per agent (`opencode`, `pi`, `claude`,
     `codex`, `agy`); unset, `agents.source` applies. Switching back to
     `"host"` keeps the depot data until `humanitl agents remove <id>`.
   - `agents.<id>.credential_writeback`: `true` (default) or `false`; see
     point 4.
   - The company policy file (BACKLOG section 9, item 21) may lock the source
     to `"depot"`, per agent or for all. A lock may only name `"depot"`.
     While locked, a host agent is refused with a `Diagnostic` that names the
     policy as the source, and app and CLI show the setting as fixed by the
     company. Until item 21 exists, the resolver takes the lock as an input
     and the tests supply it. The lock binds every way of writing the
     setting: `humanitl agents source`, `humanitl config set
     agents.<id>.source`, the generic settings row and `ChangeSetting` are
     refused alike.
   - **Every launch path follows the source.** With `"depot"`, and always
     under the lock, the program comes from the depot, whichever way the
     session was started: `humanitl go`, `humanitl run`, the app, or a
     command given in `agent.command` or after `humanitl run --`. Such a
     command may add arguments and may name the depot program by its name;
     a host path, or a program on the host that the adapter recognises as
     its agent (`claude`, `codex`, `agy`, also through a wrapper path), is
     refused with `AGENT_017`. Under the lock this holds for every adapter
     and without exception; without it, `"host"` keeps today's behaviour.
     There is one guard, on the final command the sandbox receives
     (`PlanRequest`), in the sandbox crate before any backend plans, so the
     configured agent, `agent.command`, a caller's `run --` command and the
     app all pass it; no start path resolves or binds a program after it.
   - **Known host installs are masked.** A launch check cannot stop a shell:
     the default profile binds the host's `/usr` read-only, so `humanitl run
     -- /bin/sh -c /usr/bin/claude` would reach a host install there. With
     `"depot"`, and always under the lock, the launcher therefore covers the
     host install locations the adapter's recipe lists (for Claude Code
     `/usr/bin/claude`, `/usr/local/bin/claude`, the npm global directory
     `/usr/lib/node_modules/@anthropic-ai/claude-code`; the host's home is
     not mounted) with an empty read-only mount, so a recognised host agent
     is not reachable inside the sandbox. Only locations that exist on the
     host are masked; an absent one is skipped, because there is nothing to
     reach and bwrap cannot create a mount point under the read-only
     `/usr` (`EROFS`). Seatbelt on macOS has no mounts: there the masks are
     explicit deny rules (`file-read*` and `process-exec`) for the same
     paths, placed after the rule that allows reads outside the private
     trees, because in SBPL the last matching rule wins.
   - **Limit.** The lock keeps Humanitl from starting or exposing a host
     agent. It is no defence against a user who deliberately copies an
     agent binary to a path no recipe lists: on their own machine the user
     controls that, and the company policy file does not claim otherwise.

2. **Layout of a depot**, decided by the daemon, never by the agent:
   - `program/<version>/`: the agent's files as the install left them, plus
     `manifest.json` written by the daemon (version, source URL, SHA-256 of
     every file, signature result where the vendor signs, time). Only
     regular files, directories and relative symlinks that stay inside the
     tree are accepted; sockets, FIFOs, devices, hard links out of the tree
     and set-uid or set-gid bits make the install fail.
   - `config/`: the agent's configuration (`CLAUDE_CONFIG_DIR`,
     `CODEX_HOME`, `~/.gemini/antigravity-cli` and the like), changed only
     through a deliberate maintenance run (point 5).
   - `credentials/`: the one credential file the adapter declares, `0600`,
     optionally encrypted at rest with a key from the keyring (point 6).
   The daemon never executes anything from the depot on the host, not even
   `--version`; versions come from `manifest.json`.

3. **In a session** the depot is mounted through the backend-neutral
   `LaunchPlan`, so bwrap, Podman, the kernel level and Seatbelt carry it the
   same way:
   - `program/<version>/` read-only at `/run/humanitl/agent/`. The plan binds
     the resolved version directory, never the `current` link, so an update
     never changes a running session. This is a new mount kind of the
     daemon, next to today's `AgentProgram` (one regular file, HUM-278): a
     directory, checked before every bind with the trust rules of
     `locate.rs` (owner, no write for others, no symlink out of the tree)
     applied to every entry of the version directory. Adapters cannot add
     it.
   - `config/` and the credential file are **copied** into a per-session
     scratch directory on the host, which is bound writable as the agent's
     configuration directory. Agents write their configuration directory
     all the time (Claude Code its `.claude.json`), so a read-only mount
     would break them. The agent writes its state into the copy as it does
     today in its tmpfs home; at the end of the session the copy is
     deleted. The guarantee is: **nothing a session writes persists into
     the depot, except the guarded credential file (point 4).** A
     compromised agent can change its own configuration for the rest of its
     own session, and only there. That is no new risk: the same agent can
     already run any command in that session, and with a host source today
     its tmpfs home behaves the same; what the depot must prevent, and does,
     is that such a change reaches a later session.
   - Self-update in the session is off through the adapter's documented
     switch (Claude Code `DISABLE_UPDATES`, Codex
     `check_for_update_on_startup = false`, `agy` to be verified); the
     read-only program directory makes it fail even if the switch is missed.

4. **Credential write-back guard.** Access tokens expire and some providers
   rotate the refresh token on every use, so a credential discarded with the
   scratch would force a new login. The daemon therefore takes back exactly
   one file, the adapter's declared credential file, and only after checking
   it:
   - opened with `O_NOFOLLOW` relative to the scratch, a regular file, at
     most 64 KiB;
   - parsed with the adapter's format: known keys only, values of the
     expected type and shape. On the **first login** no stored file exists;
     then the file is checked against the adapter's schema alone (required
     keys present, types, value shapes, size cap). Every later write-back
     is checked against the schema **and** against the stored file: no key
     added or dropped compared with it, same shape per value;
   - not older than the stored one where the format carries an expiry, and,
     where the format names the account (a `chatgpt_account_id` claim, an
     email), the same account as at login; a different account is refused,
     not swapped in;
   - written atomically (`0600`, temporary file and `rename`) under a lock
     per agent; an audit entry records agent, session and the SHA-256 of old
     and new file, never their content.
   A refused file leaves the stored credential unchanged and yields an
   `AGENT_*` finding on the session. With `credential_writeback = false`
   nothing is taken back; the login then lasts as long as the stored token.
   The daemon takes the file back when it changes (polled) and at the end of
   the session, so a token refreshed in a long session is not lost to a
   crash.

5. **Install, update, login and configuration are maintenance runs** inside
   a normal Humanitl sandbox, never on the host:
   - no project directory, no host data; `HOME` is a scratch seeded from the
     depot;
   - network only through Humanitl, with session rules from the adapter that
     allow its package or download hosts (install, update) or its login
     hosts (login); everything else is held as in any session;
   - the version is pinned: `install` takes the version the user names or
     the vendor's current one, records it, and `update` is the only way to
     change it; where the vendor publishes a signature (Claude Code: a
     manifest signed with the key
     `31DD DE24 DDFA B679 F42D 7BD2 BAA9 29FF 1A7E CACE`), the run verifies it
     with the host's `gpg` inside the sandbox and the result goes into
     `manifest.json`. Where the adapter declares a signature, a missing or
     failing `gpg` fails the run closed with `AGENT_011`; nothing is
     committed unsigned;
   - after a successful run the daemon commits: the install moves the
     program tree into `program/<version>/` and switches `current`; the login
     takes **only the authentication data** through the guard of point 4:
     the adapter's credential file, and where an agent keeps account data
     in a file that also holds configuration (Claude Code's `.claude.json`
     mixes account state with `mcpServers` commands), only named fields the
     daemon extracts and writes into the depot's copy itself, never the
     file as written by the agent. Everything else the login run wrote is
     discarded; a change of configuration goes only through the confirmed
     configuration run. A configuration run shows the
     changed files as a list, with keys that execute commands (Claude Code
     `hooks`, `apiKeyHelper`, `statusLine`, `mcpServers`, `env`; the
     equivalents of the other agents) marked, and commits only after the
     person confirms in app or CLI.

6. **Credentials at rest** are `0600` files in a `0700` directory. With
   `agents.encrypt_credentials = true` they are encrypted with an AEAD from
   an established crate, the key in the system keyring like `llm.api_key_ref`
   (`config/src/secret.rs`); without a keyring the setting is refused, never
   silently stored next to the data.

7. **Login.** The browser opens on the host with the login URL; the agent in
   the sandbox cannot reach the host and the host browser cannot reach the
   sandbox's loopback. Per agent, from the vendor's documentation
   (2026-11-16):
   - **Claude Code** prints the URL and offers `Paste code here if
     prompted`: the browser shows a code that the person pastes into the
     terminal (code.claude.com/docs/en/authentication). Credentials live in
     `$CLAUDE_CONFIG_DIR/.credentials.json`, mode `0600`.
   - **Codex** returns the token through a callback on `localhost:1455`,
     which a sandbox cannot receive. Its device flow `codex login
     --device-auth` needs no callback but must be enabled in the ChatGPT
     security settings (learn.chatgpt.com/docs/auth). Codex keeps the login
     in `$CODEX_HOME/auth.json` with `cli_auth_credentials_store = "file"`.
     Fallback when the device flow is off: the person pastes the callback
     URL from the browser's address bar into Humanitl, and the daemon
     replays it to `127.0.0.1:1455` inside the login sandbox, after checking
     that it is exactly that callback. The replay exists only on backends
     where the sandbox has a loopback of its own that the agent may bind
     (bwrap, Podman, the kernel level). Under Seatbelt on macOS the profile
     lets the agent bind only the session's two ports and shares the host's
     loopback, so there the device flow is the only supported way; with
     the device flow switched off, the login ends with `AGENT_014`, whose
     `fix` names the ChatGPT setting.
   - **`agy`** offers a headless flow with a pasted code
     (antigravity.google/docs/install, tab CLI), but stores its tokens in the
     operating system's keyring (Secret Service over D-Bus). The host's
     session bus is never passed into a sandbox. Whether `agy` falls back to
     a file is open; until verified, the depot supports `agy` with an API
     key (`modelProvider = "gemini"`, `GEMINI_API_KEY` from Humanitl's
     keyring) and not with a stored login.
   App and CLI open the URL on the host only on the person's click, and only
   for the hosts the adapter declares for login; a URL to any other host from
   the agent's output is shown, not opened.

8. **Control.** One RPC per capability (ADR-0018): status of all agents
   (effective source with origin and lock, installed version, signature,
   logged in, last write-back, size), set source, install, update, login,
   configure, remove. Maintenance runs are sessions with a terminal, so app
   and CLI attach to them as to any session. CLI `humanitl agents
   status|source|install|update|login|config|remove`; app section "Agents"
   with one card per agent.

## Rationale

- Storing is not running. The depot is files on the host; every byte of it
  that executes, executes inside a Humanitl sandbox. No kernel isolation is
  needed for the depot itself, and the sandbox around install and login is
  the same one that already holds the agent in a session.
- A read-only program and a configuration copy that is discarded with the
  session turn "a compromised session writes a hook into the depot" from a
  persistence into a change that ends with that session. The one file that must flow back is narrow enough to check:
  a token file has a known shape, a settings file does not.
- Copying `config/` into a scratch instead of an overlay works on every
  backend through the existing plan (bind mounts on bwrap and Podman,
  virtiofs on the kernel level, path rules on Seatbelt); configuration
  directories are small.
- Maintenance runs as sessions reuse what exists: the terminal stream, the
  hold, the rules and the recording. The person sees every request of an
  install or a login like any other.
- Pinning the version and turning off self-update makes an update a
  decision with a record, and keeps the program the same for every session.

## Rejected alternatives

- **Write the whole configuration back after each session.** Comfortable,
  and exactly the persistence this decision is about: a prompt-injected
  session would add a `hooks` entry or an MCP server that runs in every
  later session of every project.
- **Mount the host's login (`~/.claude`, `~/.codex`) into the sandbox.**
  Needs the agent on the host, gives the sandbox the host's live
  credentials and lets a session change them for the host too. HUM-514
  keeps this as a separate switch with a warning, outside this decision.
- **Run install and login on the host and only store the result.** Runs an
  untrusted installer and program on the host, the thing the owner wants
  to avoid.
- **A container image per agent.** Ties the depot to the Podman backend and
  leaves bwrap, the kernel level and Seatbelt out; an image also mixes
  program and login.
- **Forward the host's `localhost:1455` into the login sandbox.** An inbound
  connection into the sandbox is a second door; the paste of the callback
  URL replayed inside the sandbox needs none.
- **Pass the host's D-Bus session bus for `agy`'s keyring.** Opens the whole
  host keyring and every service on that bus to the agent.

## Consequences

- One new configuration section `agents` (next to `agent`, which picks the
  adapter), a depot directory, one RPC group, a CLI command group and an app
  section; the core does not change. New mount kind: a depot version
  directory, daemon-only, next to the single-file `AgentProgram`. The adapter trait grows by optional
  declarations (install recipe, update switch, credential file and format,
  login hosts and flow, keys that execute commands); an adapter without them
  supports only `"host"`.
- A new declared side channel: the credential file carries a few kilobytes
  from one session to every later one, across projects. The guard limits it
  to the format and to the same account; it cannot tell a token the
  provider issued from one a session made up in the same shape.
  `docs/SECURITY.md` (section 5), `docs/THREAT-MODEL.md` (new entry K-36) and
  the side-channel table in `README.md` say so in the same commit as the
  write-back.
- Every session of that agent holds the same login. A compromised session
  can use or send the token as today with a login inside the sandbox; it can
  no longer make the login outlive a `remove`, and no longer plant code for
  later sessions.
- Two parallel sessions of an agent with rotating refresh tokens can
  invalidate each other's token; the guard never writes an older token over
  a newer one, and the person may have to log in again. The doctor and the
  status say which session wrote last.
- The depot protects against the agent and against accidental execution,
  not against other processes of the same user on the host, which can read
  and change files under the user's data directory as they can today.
- `agy` starts with API-key support only, until its storage without a
  keyring is verified.

## Adding an agent to the depot

Every part of the depot (the core, the guard `AGENT_017`, the masks, the `Agents` RPC, the CLI
and later the app section) iterates over the registered agents and names none itself (HUM-518).
An agent joins in two places:

1. one `KnownAgent` in `daemon/crates/sandbox/src/agent/depot/recipe.rs` (`KNOWN`): identifier,
   display name, program name, its host install locations to mask (always, recipe or not), and,
   once every value is sourced from the vendor's documentation, its `DepotRecipe` (program inside
   the version directory, configuration variable, self-update switch, credential file, install
   hosts for HUM-519, login flow for HUM-520); without a recipe `depot` answers `AGENT_015`;
2. one line in the macro `known_agents!` in `daemon/crates/config/src/agents.rs`, which gives the
   identifier, the field `agents.<id>` and its branch of `AgentsConfig::settings`.

The schema snapshot, `docs/CONFIG.md` and the reader register then name the new keys, and their
tests say so. A test walks every registered entry and holds the invariants (install locations
named and absolute, a plain program name, a credential file inside the configuration copy, the
same identifiers in both places), so a new entry is checked without a new test. What cannot be
data stays in the agent's adapter (`AgentAdapter::depot_recipe` may override the table).

## Addendum 2026-11-23: The source as part of the project choice (#15)

Point 1 makes the source a setting per agent, and a start never falls back
to the other source. On the first-start screen (ADR-0024 point 3) that left
an agent that lies only in the depot with the default `host`, not
available, and the screen could not change it; the person had to know
`humanitl agents source`. That contradicts "everything preset, nothing
required". The source therefore also becomes a key of the project choice:

1. **Allow-list.** `PROJECT_CHOICE_KEYS` carries `agents.<id>.source` for
   every registered agent (`SOURCE_KEYS`, from the same `known_agents!`
   line, so a new agent needs no third place). The project choice
   overrides `agents.<id>.source` and `agents.source` per project; the
   project profile from the repository stays closed to both
   (`x-project-scope = denied`). The source picks only where the program
   comes from; both sources run in the same sandbox, so the choice touches
   none of the three guarantees.
2. **Proposal.** `Projects.proposal` (HUM-536) decides, never a client
   (ADR-0018). The option of every agent carries `agents.<id>.source` in
   its pairs, and a field `agent_source` after `agent` offers `host` and
   `depot` for the chosen agent, each with its pairs, its availability and
   its reason. The preselected source is, in this order: `depot` under a
   lock; the effective source when it is set anywhere (`agents.<id>.source`
   or `agents.source` in `config.toml`, the environment, the project choice,
   the company policy) or when the project already has a stored choice,
   also one from before this addendum without a source, because that is
   the source its start takes; only an unset source of a project without a
   stored choice is preselected by readiness, `host` where the agent is
   ready there, else the depot where it is ready there. So the default is
   always the start that the saved choice would make, and nothing the
   person set is switched silently. The defaults line names it ("Claude
   Code from the depot"). With `Proposal.cli_overrides` (`humanitl go
   claude`) the choice to save comes without the session values but keeps
   the shown agent's `agents.<id>.source`. The contract stays at 1.62: the field and its pairs
   are values of the existing generic messages, and a client that does not
   know `agent_source` shows it under its id.
3. **Start.** A start without a screen and with a stored project choice
   takes its source. Without a project choice point 1 applies unchanged:
   the effective source, and never a fallback to the other one. The
   preselection never reaches a start that nobody saved; the line of
   `humanitl go --yes` saves the defaults before it starts.
4. **Lock.** A company lock of the source wins: the field `agent_source` is
   read only with the policy as its origin (`SettingLock`), `host` is not
   available and names the policy (`AGENT_010`), no pair carries the
   source, and a choice that names it is `CONFIG_024` when it is saved and
   when a start loads it, whatever its value, like any locked key of the
   choice (HUM-534).

## Affected issues

`HUM-518` (depot core, configuration, policy lock, RPC, CLI status, source
and remove), `HUM-519` (install and update in a maintenance run, pinned
version, signature), `HUM-520` (login, credential write-back guard, security
statement and threat model), `HUM-521` (app section "Agents"), `HUM-522`
(end-to-end run and guide), `HUM-523` (credentials encrypted at rest); the
adapters `HUM-514`, `HUM-515` and `HUM-517` use the depot when it is chosen.
See `backlog/sprint-7.md`.
