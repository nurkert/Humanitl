# ADR-0024 · A start window, a terminal client for settings and views, and sessions that outlive their terminal

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-18

## Context

On 2026-11-17 the owner described how starting and using Humanitl should
feel. Five requirements are binding; their details are still to be agreed
with the owner; the answers of 2026-11-17 stand in the Decision section
with their date, and what is still open under "Open questions for the
owner" below.

1. **A start window in the app**, like the welcome screen of IntelliJ:
   running sessions to attach to, recent projects (project, agent, time) to
   start again, a new project (folder, agent, profile), and cleaning up old
   sessions and projects with a confirmation that says exactly what is
   deleted. The global settings are a tab of this window, not a popup;
   inside a running session they stay the sheet from below (about 75 % of
   the height). Tab and sheet render from one component, so a change shows
   everywhere. The app never opens a session without a project: started
   without one, for example from a launcher such as vicinae, it shows the
   start window. On 2026-11-17 the owner hit `SANDBOX_049` exactly that way.
   `humanitl app [.]` opens the start window, `.` preselects the current
   folder.
2. **A settings TUI** built on ratatui: every setting editable as in the
   app, rendered from the same catalog (categories, search, origins, locks
   of the company policy) and written through the same RPCs, without a list
   or logic of its own. Reached from the CLI and from inside a running
   terminal session.
3. **A first-start screen** for a project Humanitl does not know yet, in
   `humanitl go` and in the start window: one screen, everything defaulted,
   the focus on "Start", details only behind "Customise…". Nothing must be
   set, everything can be. The choice is kept for the project, and the next
   start shows no screen. `--yes`, no terminal or `--json`: no screen, the
   defaults and one line saying which.
4. **One command to start**, `humanitl go claude`: the agent from the host
   or the depot (ADR-0023), the current folder mounted, the splash of
   HUM-496, then the agent full screen as if no isolation layer were there.
   Keys inside the session switch views (interceptor, settings); closing the
   terminal detaches instead of ending, and the GUI or another terminal can
   attach.
5. A fallback for the macOS release runner (independent; HUM-531).

What exists today and constrains the design:

- `humanitl go` (HUM-496, `daemon/bin/humanitl/src/cmd/go.rs`) is
  `humanitl run --disposable` with the terminal handed to the agent, the
  splash (`daemon/bin/humanitl/src/splash/`) and the question about waiting
  changes at the end. The agent comes from `agent.adapter`; a different one
  needs `-- CMD`.
- The daemon owns the pseudoterminal of the session (HUM-042, opened in
  `daemon/crates/sandbox/src/bwrap.rs`). The `Terminal` RPC has one writer
  and many readers and replays a backlog of raw bytes on attach; it keeps
  no screen state. `humanitl sandbox attach` (HUM-279) and `humanitl run --
  CMD` at a terminal (HUM-280) detach with a prefix key, today
  `terminal.detach_prefix = "ctrl-a"` and `d`.
- Agent output to a terminal passes the `FullScreen` policy of
  `humanitl_core::TerminalFilter` (`daemon/crates/core-types/src/terminal.rs`),
  an allow-list: OSC 52, OSC 8, window titles, DCS, APC and others never
  leave the daemon. "Looks like no isolation layer" therefore means "as far
  as that filter allows", never a raw byte copy.
- `humanitl run` and `go` end the session on `SIGHUP` and `SIGTERM`
  (`daemon/bin/humanitl/src/cmd/run.rs`), so closing the terminal window
  ends the agent today.
- The daemon runs exactly one session; a second start is refused with
  `CLI_005` (`daemon/crates/ipc/src/sandbox.rs`). "Running sessions" are at
  most one today.
- The project folder of the app is the global `sandbox.work_dir`, set
  through `SetConfig` by the folder picker (`app/lib/core/ui/work_dir_picker.dart`);
  the CLI uses its working directory. The recorder keeps `work_dir`, `agent`
  and `started_at` per session (`daemon/crates/recorder`, table `sessions`).
- The settings overlay of the app (HUM-404) draws from a register written in
  Dart (`app/lib/features/settings/registry/settings_registry.dart`): titles,
  help and keywords come from the ARB files, categories from a Dart enum.
  The daemon delivers values, the JSON schema (`x-tier`,
  `x-project-scope`), origins per key (`FieldOrigin`) and, so far only for
  `agents.source`, a lock of the company policy (`humanitl_config::agents`,
  HUM-518). A terminal client cannot read that register.
- The project profile `<project>/.humanitl/profile.toml` is the one
  configuration layer that arrives with a cloned repository. It may not set
  a key marked `x-project-scope = "denied"` nor bring rules (ADR-0011,
  `daemon/crates/config/src/profile.rs`), and in a session with a writable
  folder the agent can rewrite it.

Hard constraints:

- The three sandbox guarantees and `tests/escape/` stay as they are. No new
  door, no new mount, no weakening of the terminal filter.
- Every capability is an RPC first; app, CLI and TUI are thin clients
  (ADR-0018). A setting, an option of the first-start screen or a cleanup
  candidate that only one client knows is a defect.
- No new port (ADR-0015); ratatui is a library of the CLI binary, not an
  abstraction.
- A project never lowers the isolation level (CONVENTIONS 4.11), and a
  locked value of the company policy is shown read-only everywhere.
- Deleting data is irreversible: whatever cleanup removes is listed before,
  by the daemon, and the project folder and the audit chain are never part
  of it.

## Decision

We add a project register and a settings catalog to the daemon, three
clients render them (the app's start window, the app's session sheet and a
ratatui TUI in the `humanitl` binary), and the terminal client of a session
gets a prefix key table, switchable views and detach on close.

1. **Project register in the daemon.** The daemon keeps known projects in
   its SQLite store (ADR-0008, new migration). Every project has a stable
   id that choice, cleanup and clients refer to; the canonical path and the
   device and inode seen at the first start recognise it. A folder replaced
   at the same path keeps its old row as history and gets a new current row
   with a new id; there is exactly one current row per path. Every recorded
   session carries the project id it was started in, and everything per
   project selects by that id, never by path. Per project: last start, last agent, number of runs, and the
   **project choice** (`ProjectChoice`): the values of the first-start
   screen (agent, folder mode, isolation, ask mode, model server, profile),
   nothing else. The project choice is a host-side layer, written only
   through the RPC, never read from the project folder. It ranks above the
   repository's project profile and below environment and command line;
   its origin is `project-choice`. It obeys the same rules as every layer:
   `sandbox.min_isolation` only rises, company locks win, a value it may not
   carry is `CONFIG_024`. (Owner, 2026-11-17, question 2: host-side in
   the daemon, not the project file.)
2. **RPC `Projects`** with one operation per capability, like `Agents` and
   `Egress`: `list` (recent projects, newest first, with running session),
   `get`, `proposal` (the first-start screen as data: every option, its
   default, whether it is available and, if not, why), `save_choice`,
   `cleanup_plan` and `cleanup` (the plan names every item and its size;
   `cleanup` deletes exactly a plan the client confirmed, by its digest;
   the digest binds the identity and version of every item, not the shown
   counts and sizes, and is rechecked in the deleting transaction).
   There is no separate `forget`: forgetting a project is a cleanup plan
   limited to its choice and register entry, confirmed like any other. The
   scope of cleanup is point 14 (owner, 2026-11-18, question 8). A start without a project folder is
   refused with `SANDBOX_053` instead of falling back to a directory of
   the calling process.
3. **Settings catalog in the daemon.** Categories, order, titles, help
   texts, search keywords, kind of control, "takes effect next session" and
   the lock state per key move from the Dart register into a catalog the
   config crate owns (texts in English and German beside the schema; the
   ARB files keep only the chrome of the app). Decided by the lead on
   2026-11-17 (question 3), the owner did not object: the config crate is
   the one place both a Rust client and the app can read without a build
   step that generates one from the other.
   `GetConfig` with `include_catalog` returns it together with values and
   origins; with a `project` it resolves as a start in that project would,
   project choice included. Writes to a project choice go only through
   `Projects.save_choice`, never through `SetConfig`, which writes only the
   global file. A new event `ConfigChanged`, sent after every write that can
   change the resolution, keeps app and TUI current. The app keeps widgets only for rows that need a special control
   (servers, backends, update), keyed by catalog id; everything else is
   drawn generically. A test fails when a schema leaf has neither a catalog
   entry nor an explicit `hidden`.
4. **One settings component in the app** (`SettingsPanel`) with two hosts:
   the tab of the start window and the sheet in a session. Visual design is
   out of scope of this decision; the owner redesigns the sheet later.
5. **Start window** as the app's first screen whenever no session is
   attached: running, recent, new, cleanup, settings tab. `humanitl-app`
   accepts `--project PATH`; `humanitl app [PATH]` starts it, or, if an app
   is already connected to the daemon, asks the daemon to forward the
   request to it as an event on the connection the app already holds.
   Decided by the lead on 2026-11-17 (question 4), the owner did not
   object: no new socket and no desktop-specific single-instance mechanism
   per platform.
6. **Terminal client on ratatui.** The `humanitl` binary gets a module
   `tui/` built on ratatui with the crossterm backend: the settings TUI
   (`humanitl settings`, owner 2026-11-17, question 1), the first-start
   screen of `go`, the
   interceptor view and the help of the prefix key. Every screen reads RPC
   answers and writes through RPCs; none holds a list of its own. One guard
   restores the terminal on every exit path, panics included. ratatui and
   crossterm are dependencies of `daemon/bin/humanitl` only, pinned in the
   workspace `daemon/Cargo.toml`; the daemon and the shim do not link them.
7. **`humanitl go [AGENT]`.** The normal start stays `humanitl go
   [AGENT]`: the agent is an optional word (`humanitl go claude`), resolved
   by the adapter list and taking its program from host or depot as
   ADR-0023 says. Everything else is a flag (owner, 2026-11-17, question
   5): `--dir PATH` (folder, default the working directory), `--disposable`,
   `--rw` (direct writes), `--ro` (read-only), `--kernel` (kernel level).
   Flags hold for one call and never change the stored project choice. An
   unknown project gets the first-start screen, a known one starts with its
   project choice.
8. **Prefix key table.** `terminal.detach_prefix` becomes
   `terminal.prefix` (the old key stays as an alias) with a fixed table
   after the prefix: `d` detach, `,` settings, `i` interceptor, `?` help,
   the prefix twice or its letter sends it once to the agent. Where the
   agent has switched the terminal to the kitty keyboard protocol
   (CSI-u), `Ctrl+,` opens the settings directly; the client never enables
   that protocol on its own. **Default prefix `ctrl-b`, as in tmux** (owner,
   2026-11-17, question 6; today `ctrl-a`). Two consequences are part of
   the decision: a tmux running inside the sandbox has the same prefix, and
   the user sends it through by pressing `Ctrl+B` twice; and a user who had
   the old default gets a one-line migration note at the first attach after
   the change (an explicitly configured `ctrl-a`, under either key name,
   stays as it is).
9. **Views.** The client switches between the agent and its own screens.
   While a client screen is shown, agent output keeps arriving and is
   buffered by the client up to a bound; on return the client restores its
   screen and asks the agent to redraw (a resize to the same size through
   the `Terminal` RPC). Input goes to the agent only in the agent view.
10. **Detach on close, for `humanitl go` only** (owner, 2026-11-17,
    question 7). `terminal.on_close = "detach"` (default) or `"stop"`
    governs `go`; with `"detach"`, `SIGHUP` of `go` detaches and the
    session keeps running in the daemon. `humanitl run` ends the session
    when its terminal closes, as today. Sessions started from the app keep
    today's behaviour: closing the window or the terminal pane detaches the
    stream, the session keeps running (tray, "running" in the start window),
    and stopping stays an explicit, confirmed action. A `go` in a folder whose
    session runs attaches to it instead of `CLI_005`. Waiting changes of a
    disposable run are asked for on the next `go` in that folder, or in the
    app.
11. **A bell for a new held request** in the terminal of a session
    (owner, 2026-11-17): `terminal.bell_on_hold`, default `true`. The bell
    is one `BEL` byte written by the client between frames of the agent's
    output, never text in the agent's screen.
12. **One session per daemon stays for this sprint** (owner, 2026-11-17,
    question 9). Several sessions at once come later in an issue of their
    own; register, start window, RPC and `go` are designed for several so
    that they need no reshaping then.
13. **The macOS release runner falls back** to `macos-15-intel` only when
    the primary job was cancelled before a runner was assigned (classified
    through the Actions API: conclusion `cancelled`, no runner name, no step
    started, and the check-run annotation "The job was not acquired by
    Runner of type hosted even after multiple attempts" seen on 2026-11-17),
    not in parallel, not after a failed step and not after a manual cancel,
    queued or running (owner, 2026-11-17).
14. **Cleanup scope** (owner, 2026-11-18, question 8; the default of this
    ADR, confirmed). Cleanup offers, each item listed by the daemon and
    confirmed by the human: the recorded flows and bodies of the project;
    its waiting changes of a disposable run, only on explicit choice and
    with the warning that changes not applied are lost for good; and the
    stored project choice with its register entry. Depot versions no
    session uses are not part of project cleanup; the depot handles them
    (`humanitl agents update`, `humanitl agents remove`, ADR-0023). The
    project folder and the audit chain are never deleted.

Diagnostic codes, planned here and registered by their issues:
`SANDBOX_053` (start without project), `SANDBOX_054` (cleanup item in use),
`CONFIG_024` (value not allowed in a project choice), `CLI_007` (app not
found or not startable), `TERM_003` (terminal too small or without a
needed capability for a TUI screen). All five were free in `codes.rs` on
`origin/main` (`e075aca6`) on 2026-11-17; the issue that registers one
checks again.

## Rationale

- **One catalog in the daemon** is the only place a Rust TUI and a Flutter
  app can both read. Two lists drift: the register of HUM-404 is
  written by hand next to the schema, and nothing checks that both name the
  same keys. A test over schema and
  catalog makes the drift a red build instead of a review finding.
- **The project choice lives on the host** because the alternative, the
  repository's `.humanitl/profile.toml`, is written by whoever cloned the
  repository and, in a writable session, by the agent itself. A value the
  agent can write cannot decide the agent's isolation next time.
- **ratatui** is the maintained standard for terminal UIs in Rust (MIT,
  used by gitui, bottom, yazi), renders into a test backend without a
  terminal, so screens are unit-testable, and with crossterm brings key
  parsing including the kitty keyboard protocol. Writing the same on top of
  `rustix` termios, as `tty.rs` does for raw mode, would mean our own
  layout, widget and input code.
- **A prefix key** costs the agent one key, chosen by the user; a direct
  hotkey costs a key the user did not choose. `ctrl-b` is the key most
  terminal users already know as a prefix (tmux), and unlike `ctrl-a` it is
  not the start of line in shells and readline. `Ctrl+,` has no control code
  in the legacy encoding, so it can only be direct where CSI-u is active.
- **Detach on close** is what `screen` and `tmux` users expect, and the
  daemon-owned PTY already makes the session independent of the client.
  Only `go` gets it: `go` is the entry meant to feel like a multiplexer,
  while `run` is also used in scripts, where a session outliving its caller
  would surprise.
- **Buffer and redraw** instead of a screen model in the daemon: a VT
  emulator in the daemon would be a second parser beside the filter, the
  very thing CONVENTIONS 4.26 forbids.

Unverified assumptions and where they are checked: that a resize to the same
size makes the agents we ship (OpenCode, pi, Claude Code, Codex, `agy`)
redraw (HUM-545, measured per agent); that the crossterm backend coexists
with the raw mode of `tty.rs` on the same terminal (HUM-540); that the
binary grows by less than 1.5 MB (HUM-540, measured).

## Rejected alternatives

- **The TUI with its own list of settings.** Faster to build, and the first
  new setting would exist in one client only. ADR-0018 forbids it.
- **Storing the choice in `<project>/.humanitl/profile.toml`.** It cannot
  carry denied keys, comes with clones, and the agent can rewrite it.
- **cursive or tui-realm instead of ratatui.** cursive owns the event loop
  and the terminal in a way that fights the attach loop; tui-realm adds a
  framework layer over ratatui we do not need.
- **Running the session under tmux or screen.** A third-party multiplexer
  between filter and terminal, its own escape handling and key table, and a
  dependency on the host.
- **`ctrl-a` (today's default, like screen) or `ctrl-]` as the prefix.**
  `ctrl-a` is the start of line in every shell and readline prompt an agent
  shows; `ctrl-]` is the escape key of `telnet` and unfamiliar as a prefix.
  The owner chose `ctrl-b`.
- **Detach on close for every terminal client.** Rejected by the owner:
  `humanitl run` keeps ending the session when its terminal closes. App
  starts are unaffected; the app already only detaches its stream on close,
  and stopping stays an explicit, confirmed action there.
- **Starting the macOS fallback runner in parallel.** It would always use
  a second runner and race two uploads of the same artefact; the owner
  chose the fallback only after a missing runner.
- **`Ctrl+,` only.** No control code in most terminals; it would work for a
  minority and fail silently for the rest.
- **Enabling the kitty keyboard protocol in the client for every session.**
  The outer terminal would then send CSI-u for every key, and the client
  would have to translate every key back for an agent that did not ask for
  it; one translation error is a key the agent never gets.
- **A screen model in the daemon** for redraw after a view switch; see
  Rationale.
- **Do nothing.** The owner's launcher case (`SANDBOX_049`) stays, and a
  closed terminal keeps ending the agent.

## Consequences

- New dependencies `ratatui` and `crossterm` in `daemon/bin/humanitl`, with
  `cargo deny` and the binary size checked in HUM-540. They are added
  centrally to `daemon/Cargo.toml`; an implementing subagent does not edit
  that file.
- New RPC `Projects`, new rows in the parity table (ADR-0018), new SQLite
  migration, new origin `project-choice` in `FieldOrigin`, `GetConfig`
  grows a catalog and a project selector, new event `ConfigChanged`. The settings register in Dart shrinks to the special
  widgets.
- `docs/SECURITY.md` and `docs/THREAT-MODEL.md` gain: the project choice as
  a host-side layer the agent cannot write; cleanup never deletes the
  project folder or the audit chain; the terminal filter unchanged for every
  view.
- Changing the default of the prefix key from `ctrl-a` to `ctrl-b` changes
  muscle memory of existing users of `sandbox attach`; they get a one-line
  migration note, and a tmux inside the sandbox needs `Ctrl+B` pressed
  twice.
- A session that survives its terminal can run unattended; holds that wait
  for a human wait longer. The status line of `go` and the start window show
  that a detached session runs.
- Several sessions at once (the screen-like vision of 2026-10-20) come
  later in an issue of their own (HUM-549, candidate); register, start
  window and RPC are plural so that they need no reshaping when that comes.

## Open questions for the owner

None. Questions 1 to 9 were answered on 2026-11-17 and 2026-11-18 and stand
in the Decision section with their date (3 and 4 decided by the lead, the
owner did not object).

## Affected issues

`HUM-530` to `HUM-548`, candidate `HUM-549`, in [`backlog/sprint-8.md`](../../backlog/sprint-8.md).
