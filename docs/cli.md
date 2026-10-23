# The command line

Applies to: Humanitl 0.1

`humanitl` is a thin client of the daemon (ADR-018). Every capability is a RPC
first; the command line calls it and formats the answer. It contains no
domain logic, and it makes nothing up.

This document describes `humanitl run`, `humanitl doctor`, `humanitl config`,
`humanitl audit` and the three `humanitl daemon` commands in full, and the
remaining subcommands only as far as they concern `run`. The canonical list of
all subcommands is in `backlog/CONVENTIONS.md` 3.8, the schema of all
configuration flags in `docs/CONFIG.md`.

## `humanitl run`

```
humanitl run [--profile NAME] [--work DIR] [--work-mode ro|rw]
             [--ask ui|terminal|none] [--llm URL] [-- CMD...]
```

Starts a session in the project directory: the daemon builds the sandbox, the
agent runs inside it, and `humanitl run` ends with the agent's exit code.

### What the command does, in order

1. **Resolve the profile of the session.** This is the first step, before the
   connection to the daemon and before anything else. A `.humanitl/profile.toml`
   in the project comes from a cloned repository and is therefore foreign text;
   whoever sets a locked key or wants to mount host paths gets `CONFIG_003`
   here, and the session does not start
   (`backlog/CONVENTIONS.md` 4.23).
2. **Connect to the daemon** and check the contract version. Without a daemon
   there is no proxy, no recording and no sandbox.
3. **Send `Sandbox(Start)`** — with the profile of this session, the
   project directory, the work mode, the ask mode and the configuration values
   from the command line. The daemon then resolves again for exactly this
   session and rebuilds the rule store, the hold deadline and the passthrough
   to the language model from them.
4. **Show the three guarantees** as soon as the daemon has measured them on the
   running sandbox — one line each, `[ok  ]` or `[FAIL]`. A red guarantee ends
   the run with exit 3; the sandbox is shut down in the process.
5. **Pass the agent's output through** and end with its exit code.

### The flags

| Flag | Acts on | Meaning |
|---|---|---|
| `--profile NAME` | — | The profile of the session (`profiles/*.toml`), for example `llm-only`. A name that does not exist is `CONFIG_001`. Under `humanitl sandbox` the same flag names something else, namely the bwrap profile; which meaning applies is decided by the subcommand. |
| `--work DIR` | `sandbox.work_dir` | The project directory that sits in the sandbox as `/work`. The default is the current directory. It must be absolute, without `..`, a directory, and lie below the home directory or be exactly what `config.toml` says. Anything else is `SANDBOX_006`. |
| `--work-mode ro\|rw` | `sandbox.work_mode` | Whether the agent may write in the project. |
| `--ask ui\|terminal\|none` | `hold.ask_mode` | Where a held request is decided. See below. |
| `--llm URL` | `llm.endpoint` | The language model of this session. From it comes the declared passthrough: a rule in rank 1 that is not held and overtakes your own block rules, for the inference paths of this one host. It stands as a rule of its own in the list, under `http://humanitl.internal/` and in the recording. If the address is not in your own network according to its name, the start reports `LLM_006`; the session starts anyway. Nothing is resolved for this. |
| `-- CMD...` | `agent.command` | The command in the sandbox, instead of the agent from the configuration. `-- bash` is the way to look at the sandbox from the inside. |

Every other configuration flag (`--hold-timeout-secs`, `--findings-enabled`
and so on) belongs to the configuration and not to a session. The daemon
accepts only two paths from the client — `llm.endpoint` and `hold.timeout_secs` —
and answers every other one with `CONFIG_003`. The reason is in
`backlog/CONVENTIONS.md` 4.26: a client that could set any key would thereby
decide the mount surface of the sandbox and the process inside it. Whoever
wants to change another value writes it into `config.toml` or into a profile of
their own, where a human wrote it and the daemon reads it.

### The three ask modes

- **`ui`** — requests without a rule stay in the queue, and the human decides
  in the application. `humanitl run` says so in one line before the start and
  writes nothing more about it afterwards; the application shows the card.
  If no application is running, the deadline expires and the request is
  blocked. The line per held request (`[humanitl] request held: …`) comes with
  HUM-042: it needs the event stream of the flows and the sanitising of the
  values that come from the agent's request.
- **`none`** — nobody is asked. The deadline is zero, every request without a
  rule runs straight into the timeout, and the agent gets `504` with
  `reason: timeout`. This is the mode of the profile `llm-only`: there a rule
  (`block host "**"`) decides beforehand anyway, and the agent gets `403`.
- **`terminal`** — the human decides in the same terminal. As soon as a
  request is held, a box appears on `stderr` while the agent's output keeps
  running on `stdout`:

  ```
  ┌─ humanitl · request held (1 of 2) ─────────────────────── 04:52 left ─┐
  │ POST https://api.github.com/graphql                                   │
  │ from: opencode · webfetch                                             │
  │ size: 2.1 KB                                                          │
  │ findings: 1 · api_key.github in header Authorization                  │
  │ catalog: github.api · rank #37                                        │
  │                                                                       │
  │ [a] allow once   [s] allow this session   [b] block   [r] rule…       │
  │ [v] view body    [n] next                                             │
  └───────────────────────────────────────────────────────────────────────┘
  ```

  The keys:

  | Key | What it does |
  |---|---|
  | `a` | Allows this one request. |
  | `s` | Allows it and creates a rule for this host, valid for this session. |
  | `b` | Blocks it. The agent gets `403`. |
  | `r` | Asks for the target (URL, host, apex, host and method) and duration (once, session, permanent), shows the rule and creates it after `Enter`. Two targets can fail and say so: an apex the service does not know (an address, an unknown suffix), and a method the contract does not name — a rule that would then be broader than what was on the screen is not created. |
  | `e` | Writes the request including its full body to `$XDG_RUNTIME_DIR/humanitl/edit-<id>.http` (0600, newly created, without following a symlink), opens `$VISUAL` or `$EDITOR` (otherwise `vi`) and sends the result as `allow_edited`. The file is deleted after reading. Without `$XDG_RUNTIME_DIR` this path refuses service instead of writing into a directory anyone can write to. A body that is not UTF-8 is not edited: what an editor made of it would go out as something other than what the agent sent. |
  | `v` | Shows the first 4 KiB of the body; whatever is not printable is shown as hex. |
  | `n` | Goes to the next held request without deciding. |
  | `Esc`, `Ctrl+C` | Closes the box. The request stays held. |

  The box is redrawn every second so that the clock at the top right is
  correct, and it disappears as soon as the request is decided — even when
  someone else decided it in the window. If the deadline expires,
  `[humanitl] timed out -> blocked` appears, and the next held flow
  comes up.

  It needs a terminal on the input and at least 40 columns. Without a
  terminal — in a pipe, in a script — the command refuses service with
  `CLI_002`: nobody meant bytes from a pipe as an answer. The box is also
  ASCII; a width in characters is not the same as a width in columns, and a
  double-width character would make every line wrap.
  A path in another script is therefore not readable in the box; whoever wants
  to read it uses `humanitl flows show`. While the box is up, the agent's
  output is held back (at most 256 KiB); afterwards it goes out and the box is
  redrawn above it.

  Every field runs through `sanitize_note` and is clamped to the window width:
  what the agent sends is text and not a control sequence, and no address
  pushes the key line off the screen.

  For **full-screen TUI agents such as OpenCode** `CLI_002` remains the answer
  (`backlog/CONVENTIONS.md` 4.10): a TUI redraws the whole screen, and the box
  would be gone after the first frame. The decision is made on the effective
  command — `humanitl run --ask terminal -- bash` starts no TUI and gets the
  prompt.

### Terminal, input and signals

The agent gets **no** PTY. Its output arrives as bytes over the daemon's event
stream and goes to `stdout` and `stderr` of this process; it is filtered in the
daemon, not here. The filter is an allow list: **of all control sequences
exactly one goes out, `ESC [ … m` for colour and attributes.** This discards
access to the clipboard (OSC 52), links under visible text (OSC 8), setting
the window title, every cursor movement, every erase and scroll, and the
terminal reset — each in all three notations: introduced with `ESC`, as a
single C1 byte and as its UTF-8 encoding.

In practice this means: the agent may write and colour and rewrite the line it
is currently on with `\r` and `\b`. It cannot overwrite a line that is already
there — in particular none of the three lines with which this command reports
the isolation checks. The price: a progress bar that erases to the end of the
line with `\x1b[K` leaves remnants behind. Why the rule is so strict is
explained in `docs/SECURITY.md` 3.3.

For this version that means:

- There is **no input** to the agent. A program that asks a question gets no
  answer. For line-oriented runs that is no problem, for a full-screen TUI it
  is. Whoever wants to type attaches to the same session with
  `humanitl sandbox attach`; there every key goes to the agent.
- A **raw mode** exists only with `--ask terminal`, and only for the keys of
  the box: the prompt needs single keys without a line ending. The terminal
  is restored on every exit — on the ordinary end, on every error path, on a
  panic and on `SIGTERM`, `SIGHUP` and `SIGINT` (then with exit `128 + n`).
  Without `--ask terminal` it is not touched at all.
- **`Ctrl+C`** ends the session (`Sandbox(Stop)`); it does not go to the agent
  as a byte. If the box is up, the first `Ctrl+C` closes it, and the request
  stays held. **Pressed twice, the command itself ends** with `130`: the first
  asks the session to end, and if the agent does not listen, the second should
  not go nowhere. The same applies to `SIGTERM` and `SIGHUP` from outside —
  they end the session and the command with `128 + n`, and the terminal is
  back in normal mode afterwards.
- There is no **`Ctrl+]` menu**. The way to a held request is the box of
  `--ask terminal`, the application or `humanitl flows`.

### Exit codes

| Code | Meaning |
|---|---|
| The agent's | It has ended; its number is passed on. A signal becomes `128 + n`. |
| `1` | An error of the caller: a profile that does not exist, a path that is not one, `--ask terminal` with a full-screen TUI agent. |
| `2` | The daemon is not reachable, or it speaks a different major version of the contract. |
| `3` | One of the three isolation guarantees does not hold. The sandbox was shut down. |
| `4` | A security violation, for example an authority mismatch. |

**Known collision:** an agent that itself ends with 2 or 3 cannot be told apart
from a daemon or isolation error. Whoever must separate the two cleanly reads
`--json`: there the agent's exit code is a field of its own, and a failure is a
Diagnostic with its code.

### `--json`

With `--json`, `humanitl run` writes exactly one JSON value to `stdout` at the
end: project directory, profile, ask mode, command and `exit_code`. The agent's
output keeps going through the same `stdout`; whoever needs the JSON value
alone reads the last line. A failure is instead a line with the Diagnostic.

### One session per daemon

The daemon runs exactly one sandbox. A second `humanitl run` while one is
running gets `CLI_005` with the identifier of the running session. Whoever
wants to see it attaches: `humanitl sandbox attach` connects this terminal to
the running session, `--read-only` only watches. Exactly one client may
write; a second one gets `TERM_001` and the hint at `--read-only`.
`Ctrl+C` reaches the agent as the byte `0x03` and not as a signal, because
the sandbox has no controlling terminal. Whoever detaches ends only their own
stream; the session keeps running, and a later `attach` shows the
backlog.

## `humanitl doctor`

```
humanitl doctor [--json] [--probe-llm]
```

Checks the machine and prints one line per precondition, with `ok`, `warn`
or `fail`. This is the command for the question "why doesn't this run here?".

### The eleven lines

| Identifier | What it checks | `fail` if |
|---|---|---|
| `bwrap` | `bwrap` is in the `PATH` and is at least 0.8 | it is missing or older |
| `userns` | `bwrap --unshare-user` opens a namespace | it does not work or hangs longer than two seconds |
| `seccomp` | `/proc/self/status` has a field `Seccomp`, the kernel is at least 5.4 | the field is missing (kernel without `CONFIG_SECCOMP`) |
| `runtime_dir` | `$XDG_RUNTIME_DIR` is set, is a directory, belongs to us and has exactly 0700 | one of these does not hold — including a mode that is too narrow, such as 0500 or 0600, in which the daemon can neither change into it nor create the socket |
| `systemd_user` | `systemctl --user is-system-running` ends with 0 and says `running` | never; without systemd you start `humanitld` by hand |
| `daemon` | the daemon answers and speaks the same major version of the contract | the major version differs |
| `agent` | the agent's command is in the `PATH` and names its version | never; the path in the sandbox can be a different one |
| `llm` | the endpoint from `llm.endpoint` answers | never |
| `tray` | `libayatana-appindicator3` or `libappindicator3` lies in a directory of the loader; under GNOME also the AppIndicator extension | never |
| `renderer` | Impeller gets along with the driver: a loaded NVIDIA module under Wayland is the known black screen | never |
| `disk_space` | at least 1 GiB is free in the data directory | never |

The first four are the preconditions of the sandbox; without them nothing
starts. The line `daemon` also becomes `fail` when the daemon speaks a
different major version of the contract — then neither side understands the
other's messages. Everything else is a warning: it runs, just not as
comfortably.

### What a line means

Every line that is not `ok` carries a `Diagnostic` with code, reason and
suggestion; the blocks stand below the table, one per line and not just for
the first. The codes are `DOCTOR_001` to `DOCTOR_013`
(`docs/DIAGNOSTICS.md`).

**`ok` means looked at and in order, never "I could not look".**
A check that could not be carried out — a file that does not exist on this
kernel, a program that does not answer — is `warn` with `DOCTOR_012`; its
evidence starts with `not measured:`, and its suggestion is the command the
doctor tried, so that a human can look for themselves.

### The language model's endpoint is contacted only on request

`humanitl doctor` does not open a single connection on its own. The line
`llm` therefore carries `DOCTOR_013` without further options — "not
contacted" — and as its suggestion the command that would do it.

With `--probe-llm` it is measured. Beforehand a line on `stderr` says where it
goes and what is sent, **before** the connection is opened; it can be switched
off neither with `--json` nor with `-q`, because it belongs to the action and
not to the output. The measurement goes through the RPC `ProbeLlm` in the
daemon (`GET` on the endpoint's model list, for an Ollama without a path
prefix first on `/api/tags`; no credentials, no redirects); without a running
daemon there is no probe, and the line stays honestly unmeasured. A server
that wants an access key answers this keyless probe with `401` or `403`; the
line then says so, says that the doctor sends no key, and says whether one is
stored (asked through `LlmKey`, never the key itself). The fix is
`humanitl llm key set` only when none is stored; with one stored it is
`humanitl llm test <endpoint> --with-key` (HUM-283). An `llm.endpoint` in
`config.toml` that carries a quote, an angle bracket, a backslash or white
space (an older version accepted it) does not stop the daemon: it counts as
not set, and the line `llm` carries the `CONFIG_003` warning with the cleaned
address as its fix. Without a model list the line names the last answer
(`no model list, last answer HTTP 404, text/html`) instead of `0 models`. The switch
is not called `--llm`: that is the alias of `--llm-endpoint` and **sets** the
address instead of checking it.

### Where the report comes from

The doctor is a capability of the daemon (RPC `Doctor`, ADR-018); the setup
screen shows the same lines through the same call. If no daemon is running,
the command line runs the same checks from the same crate in its own process
— a doctor that needed a running daemon would be useless in exactly the case
it exists for. Which of the two sources it was is stated in the output
(`source`), because the daemon's `PATH` and `$XDG_RUNTIME_DIR` are those of
its unit and not those of the terminal.

Two lines always come from the client: `daemon`, because only a client knows
whether it reaches the daemon, and `llm`, because a connection would stand
behind it.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | every line is `ok` or `warn` |
| `3` | at least one line is `fail`; the sandbox does not start like this |
| `1` | the configuration could not be read |

A missing daemon is **not** exit 2: it is a line of the report.

### `--json`

One value on `stdout`, the keys in alphabetical order (that is how
`serde_json` writes a map), the lines in display order:

```json
{
  "checks": [
    { "evidence": "bubblewrap 0.12.0 at /usr/bin/bwrap", "id": "bwrap", "status": "ok" },
    {
      "diagnostic": {
        "code": "DOCTOR_013",
        "docs": "…/docs/DIAGNOSTICS.md#doctor_013",
        "fix": { "command": "humanitl doctor --probe-llm", "kind": "copy_command" },
        "severity": "warning",
        "title": "Sprachmodell nicht angesprochen",
        "why": "http://192.168.1.50:11434 was not contacted; …"
      },
      "evidence": "http://192.168.1.50:11434 was not contacted",
      "id": "llm",
      "status": "warn"
    }
  ],
  "source": "local",
  "status": "warn"
}
```

`source` is `daemon` or `local`, `status` the worst state in the report,
`checks` the eleven lines in display order. `diagnostic` is present exactly
when the line is not `ok`. Evidence that starts with `not measured:` is a line
without a measurement.

## `humanitl daemon install`

Writes the systemd user unit that starts the daemon at every login.
It is the most invasive command of this product outside the sandbox, and
that is why what it does is described here in full.

```
humanitl daemon install [--print] [--no-start] [--bin-dir DIR]
humanitl daemon install --refresh
```

**At most one file, in a named place.**
`$XDG_CONFIG_HOME/systemd/user/humanitld.service`, otherwise
`~/.config/systemd/user/humanitld.service`, with the permissions `0644`. No
system unit, no `sudo`, no second file, no `humanitld.socket`. Its
`ExecStart` names the `humanitld` **next to the running command line** —
never one from `PATH` and never one from a configuration value, so that at
login the same version starts as the one that wrote the unit.
`--bin-dir DIR` names a different directory instead, such as the bundle of a
package that the command line was reached through via a link. From an
AppImage (`$APPIMAGE` set), `share/humanitl/catalog/domains.yaml` and
`profiles/sandbox/default.toml` must also lie under `DIR/..`
as in the image, otherwise the command ends with `DAEMON_011`.

**With the package, nothing is written.** If
`/usr/lib/systemd/user/humanitld.service` is there, the package has placed the
units, and the command writes no file (HUM-053). It only runs what the
package cannot do as root: `systemctl --user daemon-reload` and
`systemctl --user enable --now humanitld.socket`. Only the socket (HUM-164):
if a client finds no token, it opens the socket once anyway, systemd then
starts the service, and the client waits at most ten seconds for its token.
Packages without a socket unit get `humanitld.service`. The
output names the package's unit as `unit` and the word `packaged` as `action`; under `--json` the
names are in `units`. After activation, `unit` and `exec_start` contain what
systemd really loaded (`systemctl --user show -p FragmentPath,ExecStart`),
not what is in the package's file; `unit_text` is then the text of this
loaded file and `null` if it cannot be read. If under
`~/.config/systemd/user/`
the own unit of an earlier installation is still there (first line is the
marker), it would hide the package's one: the command announces it, sets it
aside together with the activation link as `humanitld.service.bak` (or
`.bak.N` if the name is taken; `set_aside` in the output), restarts the service
and puts both back if that fails (HUM-211). It is set aside only when
activation also happens: under `--print`, `--no-start` or without `systemctl`
it stays where it is and appears as `shadowed_by` in the output. A file there
without the marker stays where it is, and the command aborts with
`DAEMON_005`, even under `--print`.
From an AppImage and with `--bin-dir` this path does not apply:
both explicitly name a different daemon than the package's. Without a package
the command never writes a socket unit; the daemon then binds its socket
itself.

**From an AppImage, files are copied.** If the command line runs from an
AppImage (`$APPIMAGE` is set), `humanitld` and `humanitl-shim` lie under
`/tmp/.mount_*`, and this path disappears with the process. An
`ExecStart` pointing there would point into nothing at the next login. Both
binaries are therefore copied to `~/.local/lib/humanitl/<version>.<stamp>/bin/`,
and with them the domain catalogue (`share/humanitl/catalog/`) and the
sandbox profile (`profiles/sandbox/default.toml`), in the same layout as in the
image: the daemon looks for both relative to its own path and would run
without them with an empty catalogue and without a profile (HUM-165). The link
`~/.local/lib/humanitl/current` points to this copy, and `ExecStart` names
`current/bin/humanitld`: an update places a new copy next to it and repoints
the link without touching the unit. If one of the two binaries is missing,
the command ends with `DAEMON_007`; if catalogue or profile are missing in the
image, with `DAEMON_011`; in both cases nothing has been copied. Next to
`bin/`, every copy holds `humanitld` as a relative link to `bin/humanitld`: a
unit from before HUM-165 names `current/humanitld`, and if a run aborts after
`current` was repointed but before the new unit is written, the unit still
starts the daemon of the new copy this way. Copying happens only after the
announcement and after every check that can reject the run; a foreign unit
(`DAEMON_005`) or a missing user session (`DAEMON_010`) therefore leave
`~/.local/lib/humanitl` untouched.

Every copy gets a new directory (`<stamp>` from time and process number),
even when the same version is installed a second time: the directory that
`current` points to at the moment is never touched before `current` points
to a complete new copy. An abort halfway through therefore never leaves a link
pointing into nothing. `current` is repointed via a second link and `rename`,
never deleted and created anew. If `current` pointed to another copy before,
the service may still be running from it; the command therefore restarts it
after `enable --now` with `systemctl --user try-restart
humanitld.service`, and only after that do the previous copy and every older
one that an earlier run had to leave behind go away (HUM-077). The restart
happens for every copy, also on the first run and when `current` was missing:
if a service of this name was already running, for instance from a unit under
`/etc` that hides the new one, `enable --now` would leave it running. A copy
goes away only if afterwards a daemon of exactly this version answers and no
process can have its program from it (`/proc/<pid>/exe`, HUM-210): a daemon
still running from it would otherwise no longer find its shim, and every
sandbox would fail with `SANDBOX_011`.
When in doubt it stays: if `/proc` or one of its entries cannot be read; if
`systemctl --user show -p MainPID` fails, yields no line `MainPID=` or no
number (only `MainPID=0` means "no main process"); if the main process it
names is not visible; if a process does not let its program be read and its
`status` does not say whom it belongs to and what it is called; or if a
process `humanitld` of the same account does not let its program be read. A
copy that stays behind is cleaned up by a later run. The new copy of a failed
run also stays by the same rules, as long as a process can be running from
it. Without a restart (`--no-start`, no `systemctl`) the previous copy stays
behind. If the restart fails, `current` points to the previous copy again, the
unit gets its old text back, the old daemon is started once more, and the
Diagnostic is `DAEMON_008`. If `current` or the unit does not demonstrably go
back, the command starts nothing more, because it would otherwise start the
copy that just failed; the Diagnostic then names what did not go back. The
same applies to a replaced unit on the path without AppImage: whoever calls
`daemon install` from a new archive gets the new archive's daemon right away
and not only at the next login. If `~/.local/lib/humanitl` is a link or
belongs to a different account than the home directory, nothing is copied
(`DAEMON_011`). If something else fails after the copy — the unit,
`systemctl` —, `current` points back to where it pointed before, and the new
copy goes away again. `--print` names the same path `…/current/bin/humanitld`
the unit would get, and copies nothing.

**`--refresh`: a newer AppImage renews the service.** `AppRun` calls
`humanitl -q daemon install --refresh` at every start of the application
(HUM-077). The switch does something only if the run comes from an AppImage,
the unit under `~/.config/systemd/user/` carries the marker and names
`ExecStart=~/.local/lib/humanitl/current/bin/humanitld` (or
`current/humanitld`, as before HUM-165; the unit gets the new path in the
process),
and `current` points to
the copy of a different version; then everything runs as described above,
including the restart. Otherwise it ends at once with 0 and writes why not:
`not_appimage`, `not_installed` or `up_to_date` (under `--json` as `action`,
plus `installed` and `version`). It never does the first setup: a unit that
starts a service at every login is not created unasked by a program start;
that remains the button in the setup or
`--cli daemon install`. And whoever removed the service with
`daemon uninstall` does not silently get it back from the next start of the
AppImage, because the unit is missing. `--refresh` does not go together with
`--print`, `--no-start` and `--bin-dir`.

**Without a user session nothing is written.** If `XDG_RUNTIME_DIR` is missing
and the unit is to be started (no `--print`, no `--no-start`), the command
ends before the first write with `DAEMON_010` and the suggestion
`loginctl enable-linger $USER`. The same applies if `systemctl --user` does
not find the session bus ("Failed to connect to bus"); then the unit has
already been taken back when the Diagnostic appears.

**At the end, it waits.** Once systemd has taken the unit, the command asks
`GetInfo` for up to five seconds until the daemon answers, and writes the
version it names as the last line. If it does not answer, the line reads
`no answer within 5000 ms`; that is an observation and no reason to take the
installation back — the unit is in place, and `humanitl daemon logs` says
what it is stuck on.

**Visible before it happens.** The whole text of the unit and both
`systemctl` calls go to `stderr` before the first write, bypassing the
output control: `-q` does not switch this off. Under `--json` a program is
reading, and for it the contract of this command line applies — one object on
`stdout`, `stderr` empty —; text and commands then stand in the object as
`unit_text` and `commands`. `--print` shows the same file and writes
nothing.

**Repeatable.** A second call with the same result does not write again; the
output then says `unchanged` instead of `created`.

**Never over someone else's property.** The first line of the unit is the
marker `# humanitl daemon install: written by Humanitl`. If it is missing in an
existing file, the file belongs to someone else, and the command refuses with
`DAEMON_005` instead of overwriting it. There is deliberately no `--force` for
this: whoever runs a unit of their own sets it aside.

**Afterwards `systemctl --user`, never `sudo`.** Without `--no-start`,
`systemctl --user daemon-reload` and `enable --now` run, depending on the
path:

- *Archive and AppImage:* `systemctl --user enable --now humanitld.service`,
  on the unit just written. The daemon binds its socket itself.
- *Units of the package:* `systemctl --user enable --now humanitld.socket
  humanitld.service`, on the units under `/usr/lib/systemd/user/`. Nothing is
  written in the process. Both units, because a client reads the token before
  it opens the socket, and only the running daemon writes it.

If there is no `systemctl` in `PATH`, nothing starts, and the output names the
two commands.

**A failure leaves nothing behind.** If systemd does not accept the unit, the
previous state is restored — a created file disappears, a replaced one gets
its old content back, the package's units stay as they are — and
`daemon-reload` runs once more so that systemd's picture of it is correct too.
If `enable --now` fails, all named units are stopped first (`stop`,
`reset-failed`). The Diagnostic is `DAEMON_008`.

The activation is taken back as well. `systemctl --user enable --now` is a
call with two steps: it creates the links and then starts. The links lie in
the user's unit directory, also for the package's units:
`default.target.wants/humanitld.service` and for the package instead
`sockets.target.wants/humanitld.socket`. If the start fails, they would remain
in place without this rollback, and the service would start at the next
login, even though the command aborted with `DAEMON_008`. Only the links this
call created are removed: whoever had activated the service before keeps the
activation.

**What counts is the directory of the systemd manager** (HUM-217). The
command line writes the unit under its own `XDG_CONFIG_HOME`; if the variable
is only set in the shell rc, the manager looks elsewhere. That is why, after
`daemon-reload`, the command asks with `systemctl --user show -p FragmentPath
humanitld.service` which file systemd loads the unit from. If it is not the
one written, the unit goes back before `enable` creates a link, and
`DAEMON_008` names both directories; the suggestion `systemctl --user show
-p UnitPath` shows where the manager looks. If systemd says nothing about
`FragmentPath`, these search paths decide (`systemctl --user show -p UnitPath`):
if the unit's directory is not among them, that is the same Diagnostic. Under
`--no-start` the same check over the search paths applies, without systemd
rereading anything. If systemd gives no answer to either, the unit counts as
unchecked and goes back as well; only if `systemctl` does not reach the
session bus at all does it stay under `--no-start`, because this path needs
no session. Before `enable`, the command notes with `systemctl --user
is-enabled` which units were already permanently enabled (an activation
only under `/run` does not count), and after a failure it disables the others
with `systemctl --user disable`, also after a failed restart: systemd removes
the links where the manager created them. The search in the own unit
directory remains as a fallback. If `is-enabled` says nothing about a unit,
the command does not activate and takes the unit back.

The hardening of the unit is measured, not claimed (HUM-044, hardened in
HUM-053). The escape tests run green under exactly its `[Service]` lines with
124 of 124 cases. The lines that must be there for this:
`SystemCallFilter=@system-service @mount @sandbox sethostname` —
`@mount`, because `bubblewrap` mounts and calls `pivot_root`; `@sandbox`,
because the filter of a unit is inherited by every child and `humanitl-shim`
installs its own seccomp filter with `seccomp(2)` (without the call the shim
dies of `SIGSYS` before the third sandbox guarantee is in place; on systemd 262
`@system-service` would contain it via `@default` anyway, but that is not
written in any line of `systemd.exec(5)`); `sethostname`, because `bubblewrap`
names the UTS namespace of the sandbox `sandbox` and otherwise aborts with
"Can't set hostname to sandbox". `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK` —
without `AF_NETLINK` `bubblewrap` does not bring up `lo` in the namespace
("loopback: Failed to create NETLINK_ROUTE socket"), and without `lo` there is
no bridge from the shim to the proxy. In addition `NoNewPrivileges`,
`ProtectSystem=strict`, `PrivateUsers`, `ProtectProc=invisible`,
`CapabilityBoundingSet=CAP_SYS_ADMIN` and more; every line, also every one
deliberately missing, is listed with its reason in
`docs/INSTALL.md#hardening`, and `systemd-analyze security` rates the unit at
3.7. Every path in `ReadWritePaths` carries a `-`, because systemd does not
even start a unit with a non-existent path in it, and on a fresh installation
none of the three exists. Measured on 2026-09-18 on Debian 14 with bubblewrap
0.12.0 and systemd 262.

**`PrivateTmp=yes` has a consequence you need to know: a project folder under
`/tmp` does not work.** The unit gives the daemon its own private `/tmp`; it
does not see the `/tmp` of the login session. A folder like `/tmp/repo`
therefore does not exist for it, `bwrap` cannot mount it, and the start ends
with `SANDBOX_005` and the sentence
"sandbox.work_dir /tmp/repo does not exist", although the folder is right
there in the terminal. The same applies to everything under `/var/tmp`.
Whoever wants to moderate a project out of `/tmp` puts it in a lasting place —
under `$HOME` or anywhere else outside `/tmp` — or starts the daemon by hand
(`humanitld`) instead of through the unit. The line stays in the unit anyway: a
shared `/tmp` is the way another process of the user slips the daemon a file,
and a project folder in a directory that disappears at the next start is no
place for work in any case.

## `humanitl daemon uninstall`

The counterpart to `daemon install` (HUM-077): disables the service and
removes what `daemon install` created.

```
humanitl daemon uninstall [--purge-binaries]
```

**Disable first, then remove.** First `systemctl --user disable
--now` runs for the units that exist: `humanitld.service` if the unit with the
marker lies under `~/.config/systemd/user/`, for the package
`humanitld.socket humanitld.service`, even though `daemon install` has
activated only the socket since HUM-164: an earlier run activated the service
as well.
If the call fails, nothing has been
removed yet, and the Diagnostic `DAEMON_014` names exactly this call for
copying; if `systemctl` does not find the session bus, it is `DAEMON_010`.
Then these go:

- every activation link under `~/.config/systemd/user/*.wants/` and
  `*.requires/` for these units (without `systemctl` the command removes them
  itself, otherwise the remnants that `disable` left behind);
- the unit `~/.config/systemd/user/humanitld.service`, only if it carries the
  marker;
- the socket `$XDG_RUNTIME_DIR/humanitl/daemon.sock`, only if it is a socket
  and nobody is listening on it any more. If a daemon that someone started by
  hand still answers there, it stays untouched;
- with `--purge-binaries` the copies from an AppImage: the link
  `~/.local/lib/humanitl/current`, leftover intermediate links
  `current.tmp-*` and every own directory of the form
  `<version>.<stamp>`, then `~/.local/lib/humanitl` itself if it is empty.
  Whatever is named differently, is a link or belongs to another account
  stays behind. The copies go only here, because only now does no service run
  from them any more.

If after disabling a daemon still answers on the socket, for instance one that
someone started by hand, `--purge-binaries` removes nothing: it may be running
from exactly these copies. The Diagnostic is `DAEMON_014` with the
suggestion `pkill -x humanitld`.

Then `daemon-reload` and `reset-failed`, whose result does not count.
If something remains, the command ends with `DAEMON_014` and names every path
that remained.

**Never over someone else's property.** Humanitl did not write a unit without
the marker; the command disables nothing, removes nothing and ends with
`DAEMON_005`. The package's units under `/usr/lib/systemd/user/` are
disabled but stay in place until the package goes
(`sudo apt remove humanitl`); a note says so. Configuration,
recording and audit log stay in place as well; they belong to the human.

Without a unit and without a copy, the command ends with 0 and
`activation: nothing`. The output names `unit`, `units`, `activation`
(`disabled`, `no systemctl`, `nothing`), `removed`, `binaries` and under
`--json` `package`, the path of the package's service unit or `null`.

Afterwards `humanitl doctor` shows the Diagnostic `DOCTOR_006` in the line
`daemon`, with the suggestion `InstallService`, that is `humanitl daemon install`:
without a reachable daemon the doctor suggests the path that sets up the
service permanently, and not `humanitld` in the foreground of a terminal.

## `humanitl daemon status`

Asks the daemon who it is.

```
humanitl daemon status [--json]
```

```
FIELD         VALUE
socket        /run/user/1000/humanitl/daemon.sock
unit          active
daemon        0.0.0
proto         1.0
session       7b1c…
capabilities  hold, rules, sandbox
```

`socket` is the path where the client looks; `unit` is the answer of
`systemctl --user is-active humanitld.service` and shows as `-` if there is no
`systemctl` — a daemon that someone started by hand is not an error, and an
invented `inactive` would be false information about a unit nobody
installed. Everything else comes from `GetInfo` and is not
supplemented.

If nobody answers, the command ends with **2** and `DAEMON_001`; if the daemon
speaks a different major version of the contract, likewise with 2 and
`DAEMON_002`. A script waiting for the service therefore checks only this one
number.

## `humanitl daemon logs`

Passes the service's journal through.

```
humanitl daemon logs [-f] [-n N]
```

The command starts `journalctl --user -u humanitld.service` as a child and gives
it its own terminal. `-n N` and `-f` are passed on unchanged. If
`journalctl` ends with 0, the command ends with 0, otherwise with 1: a 2 or 4 from
`journalctl` would mean "daemon not reachable" or
"security violation" here, and both would be wrong. There is no reader of its own for journal entries: it
would be a second source for the same truth, with its own formats and its own
errors.

Because the child writes, `--json` has no effect here; whoever wants JSON lines
gets them from `journalctl` itself (`humanitl daemon logs -- -o json` does not
exist; for that you call `journalctl --user -u humanitld.service -o json` directly).

If `XDG_RUNTIME_DIR` is missing, the command ends with `DAEMON_010` and the only
sensible suggestion: `loginctl enable-linger $USER`. Over SSH without linger
there is no user session in which a user service could live, and therefore no
journal either. If there is no `journalctl` in the `PATH`, on the other hand, a
program is missing and not a session: `DAEMON_012` with `sudo apt-get install systemd`.

## `humanitl config`

Read the resolved configuration, write a value, print the schema,
open the file by hand.

```
humanitl config get [KEY] [--origin] [--json]
humanitl config set KEY VALUE [--project]
humanitl config schema [--profiles] [--json]
humanitl config edit
```

### `config get`

Without `KEY`, every leaf field is shown as a line:

```
$ humanitl config get --origin
KEY                    VALUE    ORIGIN
hold.ask_mode          ui       default
hold.timeout_secs      300      default
llm.endpoint           -        default
sandbox.work_mode      rw       config.toml
```

Without `--origin` the third column is left out so that the table fits into a
terminal; with `--json` it is always there, since a column costs no width there.

With `KEY`, only the value is on `stdout` — `$(humanitl config get
hold.timeout_secs)` is thus the number and not a sentence about it. Which layer
set it goes to `stderr`; it does not end up in a pipe.

Resolution happens locally, over the same seven layers the daemon runs through
at start (ADR-011). A running daemon can hold values only it knows —
`hold.timeout_secs` from a session, for instance —; what is shown here is the
resolution of this call.

### `config set`

The value is read according to the schema's type:

| Field | Input | stands in the file as |
|---|---|---|
| `hold.timeout_secs` | `5m` | `300` |
| `limits.hold_body_cap_bytes` | `32MiB` | `33554432` |
| `findings.enabled` | `true` | `true` |
| `ui.theme` | `dark` | `"dark"` |
| `llm.passthrough_paths` | `'["/v1/","/api/"]'` | the list |
| `sandbox.env` | `'{"FOO":"bar"}'` | the table |
| `llm.endpoint` | `null` or `-` | nothing: the key goes, the default applies |

A value that starts with `-` is a value and not a flag
(`humanitl config set hold.timeout_secs -5m` is read and rejected, not
treated as an unknown option).

A duration takes `s`, `m`, `h` and `d`, a size `KiB`, `MiB`, `GiB` and the
forms without `i`. Both apply only to fields whose name ends accordingly
(`_secs`, `_ms`, `_bytes`); the schema itself knows no units.

```
$ humanitl config set hold.timeout_secs 5m
hold.timeout_secs = 300 (global)
```

What is written is `$XDG_CONFIG_HOME/humanitl/config.toml` (with `--config` the
file named there), through the same writer the daemon uses for
`SetConfig` (`humanitl_config::edit::set_value`). Four promises apply:

- **Only the one value changes.** The document is changed with `toml_edit`
  and not serialised anew; comments, order and notation stay, including a
  comment behind the value, a byte order mark and CRLF line endings. A linked
  file stays linked; its target is changed. Afterwards the result is read
  once more and compared with the old file plus the new value; if it differs,
  nothing is written (`CONFIG_015`).
- **Never half a file.** Writing goes to a side file in the same directory,
  then `rename`.
- **No writer loses a value.** While writing, the file's directory is locked
  with `flock`; a second `config set` or a `SetConfig` of the daemon waits
  instead of overwriting a value it never read.
- **Checked before it is in the file.** First against the schema — type,
  enumeration, bounds —, then the finished side file is loaded with all other
  layers, the way the next start loads it: profiles, environment and the
  other values of the file. A value that is correct on its own but does not
  fit together with another (`limits.hold_body_cap_bytes` above
  `limits.hold_max_bytes`) thus never ends up in the file. The sources are
  determined anew with the new file for this: `config set sandbox.work_dir` is
  checked with the profile of the project it names, not with that of the old
  one. If the configuration already does not load without the change —
  another value of the file, an environment variable such as
  `HUMANITL_HOLD__TIMEOUT_SECS=0` —, `config set` counts all Diagnostics before
  and after: loading reports only its first one, so the check takes the key of
  each Diagnostic out of file, project profile and environment and loads again.
  The value is written if the new value brings no Diagnostic that did not
  exist before and none names the key being set; `stderr` then says that the
  configuration still does not load. The same Diagnostic means same code, same
  key and same reason: if the new value moves the bound of an old Diagnostic
  (`limits.hold_max_bytes` newly set while `limits.hold_body_cap_bytes` was
  already above it), that is a new Diagnostic. This way a file with two wrong
  values can be repaired key by key, and an older error does not hide any
  new one. A Diagnostic that names no key (an unknown key in the file, a
  profile that does not load) cannot be counted this way; then nothing is
  written, and the suggestion is `humanitl config edit`, never a value from
  another layer. A value that is already there exactly like this is
  checked as well; if it was wrong, `config set` ends with the Diagnostic. A value that does not fit is `CONFIG_003` and exit 1, and the file
  stays untouched:

```
$ humanitl config set hold.ask_mode banana
error[CONFIG_003]: Wert außerhalb des Bereichs
  why: banana is not a value of hold.ask_mode; it takes one of ui, terminal, none
  fix: humanitl config set hold.ask_mode ui
  docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#config_003
```

Every suggestion is a command that succeeds: for an enumeration its first
value, otherwise the field's default (for `-5m` on `hold.timeout_secs` that is
`300`, not the bound `0`, which the check would reject as well), and lists or
tables in it stand in single quotes.

If the value is already there like this, nothing is written; `--json` then says
`"written": "unchanged"`.

Afterwards `stderr` says when the new value takes effect: if a daemon is
running, it uses it from the next sandbox start on (settings of the proxy, the
recording and the limits only after `systemctl --user restart humanitld`); if
none is running, it reads it as soon as it starts. Both are correct, and that is why the output says which
case applies.

`--project` writes instead into `<project>/.humanitl/profile.toml` under
`[config]`. That works only for keys the project profile may set: this file
lies in the cloned repository and is foreign text, and a locked key there is
`CONFIG_003` — the same boundary at which the start also rejects such a file
(`backlog/CONVENTIONS.md` 4.11). The boundary is checked before anything else:
whoever wants to write a locked key into the project learns about the boundary
and not that their value is spelled wrong.

### `config schema`

Prints the JSON schema, always as JSON; without `--json` indented. Every leaf
field carries `description`, `x-tier` (the visibility tier of the settings
screen) and `x-project-scope` (the trust boundary above).
`--profiles` instead prints the profiles that `--profile` can choose.

### `config edit`

Opens `config.toml` in `$VISUAL`, otherwise `$EDITOR`, otherwise `nano`,
otherwise `vi`, and checks the file after it is closed: first whether it is
TOML, then whether it can be resolved. If that fails, the Diagnostic is shown,
and at a terminal the only interactive question of this command line follows —
"open it again? [y/N]". Without a terminal and under `--json` there is no
question; then the command ends with the Diagnostic and exit 1. The Diagnostic
is shown exactly once in every case, under `--json` as one object. If no
editor is found, that is `CONFIG_017` with the suggestion `export EDITOR=nano`.

## `humanitl audit`

Check the hash chain and export the records.

```
humanitl audit verify [--file PATH] [--json]
humanitl audit export --format jsonl|csv --out FILE [--since TS] [--until TS] [--file PATH]
```

### What `verify` proves and what it does not

The chain shows a change, deletion or reordering before the last anchor, as
long as the attacker does not have the HMAC key (`docs/SECURITY.md`, "What
the chain proves"). This statement depends on three things: the chain
itself, the MACs and the anchors. Only the daemon has all three — the key
from the key store, the anchors from the table `audit_anchors`.

That is why the command asks the daemon first (HUM-156). It checks every record
with its key and against the anchors and says so:

```
$ humanitl audit verify
audit chain: OK
records:     4213
head:        a3f9…c2e1 (seq 4213, 2026-09-02T10:42:01.000000Z)
hmac key:    checked by the daemon
anchors:     42 (last at 2026-09-02T10:40:00.000000Z), checked by the daemon
warnings:    unanchored tail: 13 records
checked by:  daemon
```

The head is the same hash that the audit screen of the interface shows: both
ask the same daemon, and before every answer it flushes to disk everything it
has written up to then.

If **no** daemon answers — none reachable, none that accepts the token, or one
that does not know `Audit` yet —, the command line checks the file itself, and
that is the **weaker** check: chain and canonical form, but no MACs and no
anchors. The output says so, in every format:

```
$ humanitl audit verify
audit chain: OK
records:     4213
head:        a3f9…c2e1 (seq 4213, 2026-09-02T10:42:01.000000Z)
hmac key:    not checked
anchors:     not checked
warnings:    the daemon is not reachable (cannot stat the session token /run/user/1000/humanitl/token: No such file or directory (os error 2)), so this is the file-mode check; no HMAC key (file mode); unanchored tail: 4213 records; no anchors (file mode)
checked by:  file:/home/nik/.local/share/humanitl/audit/audit.jsonl
```

The first warning says why the file was checked: "is not reachable" if no
daemon is there, "does not answer Audit" if an older daemon does not know the
call. The rest of the file check knows no anchors, so every record counts as
unanchored.

A daemon that answers and **refuses** — its log or its anchors are not
readable, or it runs without an audit log (`IPC_006`) — does not get a file
check as a substitute. Its Diagnostic is on `stderr`, and the command ends
with its exit code: a weaker answer that covers up a refusal would be the
wrong information.

`--file PATH` explicitly checks this file, likewise without key and
anchors — the way for a log that comes from elsewhere.

If the chain breaks, the command ends with **4** (security violation according
to `backlog/CONVENTIONS.md` 3.8), names the position and writes `AUDIT_001` to
`stderr`:

```
$ humanitl audit verify --file audit.jsonl
audit chain: BROKEN at seq 4012 (hash_mismatch)
records:     4011
hmac key:    not checked
anchors:     not checked
warnings:    no HMAC key (file mode); no anchors (file mode)
checked by:  file:audit.jsonl
error[AUDIT_001]: Hash-Kette gebrochen
  why: audit.jsonl: hash_mismatch at seq 4012; 4011 records before it hold
  fix: mv audit.jsonl audit.jsonl.broken-$(date -u +%Y%m%dT%H%M%SZ)
  docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#audit_001
```

The first seven lines are on `stdout`, the Diagnostic on `stderr`.

With `--json` all of it is in one object on `stdout`, the Diagnostic as the
field `diagnostic`; `stderr` stays empty. `mode` says which check ran:
`full` at the daemon with key and anchors, `file` without both; `hmac` and
`anchors` say the same per part (`checked` or `not_checked`). At the daemon
`anchor_count` and `last_anchor_at` are added. `head.hash` is the hash the
audit screen shows; `head.ts` is the timestamp of this record's line,
character for character as in the log (HUM-162). Only a daemon that does not
know `AuditResponse.head_ts` yet leaves it out; then it reads
`null`.

### `audit export`

```
$ humanitl audit export --format csv --out audit.csv
exported 4213 records to audit.csv
```

The daemon writes the export if one answers, otherwise the command line
itself; both use the same code (`humanitl_audit_log::export`) and write the
same file. `jsonl` writes every line of the log verbatim — the export thus
stays verifiable against the file. `csv` writes a header line and one line
per record, with the twelve columns from HUM-050:

```
seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash
```

The eight columns between `kind` and `hash` come from `data` and stay empty
where the record does not carry the field; `flow.decided`, for instance, names
neither host nor method, those are in `flow.received`. `data` as a whole is
only in the JSONL. Until HUM-156 the command line wrote eight columns
(`seq,ts,session,kind,data,prev,hash,mac`); the overview of the interface and
that of the command line have been the same since then.

Fields with a comma or quotation marks are quoted per RFC 4180, double
quotation marks doubled, and every line ends with CRLF. JSON lines end with
LF, as in the log.

A file that already exists is never overwritten (`AUDIT_008`): an export is
evidence, and there is no `--force` for the same reason as with
`daemon install`. Writing goes to a side file, and only the finished export
gets its name; a log with a line that is not a record (`AUDIT_001`) leaves no
half file behind that would turn away the next attempt. A broken chain is
exported unchanged; the break is reported by `humanitl audit verify`
(HUM-214). The side file of an aborted export is cleaned up by the next export
into the same directory, but only if the directory belongs to the own account
and neither group nor others may write into it; in a shared directory it stays
behind. A relative `--out` is resolved against the own directory before the
call to the daemon: the daemon runs elsewhere. If it runs under systemd with
`PrivateTmp=yes`, it has its own `/tmp`; an export to `/tmp` or `/var/tmp` is
then rejected with `AUDIT_008` instead of landing in a directory only the
daemon sees. The daemon creates no directory and does not write through a link
on the way to the target; the command line creates the directory itself
beforehand.

`--since` and `--until` take RFC 3339 timestamps and cut half-open
(`since` is included, `until` is not), so that a record never appears in two
consecutive exports. They are passed on to the daemon: the contract
(`AuditRequest.Export.from` and `.to`) includes both bounds, and because
the log writes microseconds, the command line sends as the upper bound the
last whole microsecond before `--until`. The result is the same as in the
version that works on the file.

What is in no record and therefore in no export either: bodies,
plain-text values, the originals of pseudonyms — and the note of a
decision. It is in the recording (`flows.decision_note`,
`humanitl flows show`), not in the audit log (HUM-117).

## `humanitl llm discover`

Searches the local network for model servers. After `daemon install` it is the
second command that reaches beyond your own machine, and that is why what it
does is described here in full.

```
humanitl llm discover [--subnet CIDR] [--port PORT]... [--json]
```

**Only when called, and announced beforehand.** Without this command not a
single connection is made; the interface has the same path behind a button
with the same text. Before the first packet a line goes out on `stderr` that
names the network and the ports — bypassing the output control, so that
`--json` does not swallow it either.

**In your own `/24` and nowhere else.** Without `--subnet` the daemon derives
the network from the default route: the routing table names interface and
gateway, a connected UDP socket (which sends nothing) names your own address,
and from that comes the `/24` around it. A `--subnet` wider than a `/24` is
rejected with `LLM_008` before anything connects: a `/16` would be 65 534
connection attempts into a network that may not belong to the caller.

**Four ports, two steps.** The ports asked are `11434` (Ollama), `1234`
(LM Studio), `8000` (vLLM and most Python servers) and `8080`
(llama.cpp); `--port` replaces this list. First one connection attempt per
address and port with a 200 ms deadline, 64 at a time; for every open port
then the same probe as with the test button: `GET /api/tags`, otherwise `GET
/v1/models`, without credentials and without redirects. A `/24` without a
single hit is thus done in under four seconds.

**What is in the list, the server said.** Product, models and latency
come from its answer; the model names are capped and sanitised like those of
the test button. A server that says `401` or `403` does not disappear but
carries `(auth required)`. A server that listens on one of the ports but
answers neither of the two APIs is shown as `unknown` — it is named, not
concealed and not passed off as a model server.

**No flow and no queue.** The search runs in the daemon in the host network,
never in the sandbox. It appears in no queue and in no history; it changes
nothing and writes nothing.

The run ends with `0` even if nothing answered: "nothing found" is a result
and not an error. With `--json` a value with `servers` and `count` stands at
the end, otherwise a table, and during the search one line per find on
`stderr`.

## `humanitl rules test`

Asks the rule set in force what would happen to a request. Evaluation happens
in the daemon, with the same engine as in the proxy path (ADR-018): a second
evaluation in the command line could answer differently than the one that
really decides.

```
humanitl rules test <URL> [--method M] [--upgrade websocket] [--json]

verdict: block
rule: 018f0000-0000-7000-8000-00000000000a (bundled, position 3)
```

Without `--method`, `GET` applies. If no rule matches, it reads
`rule: none (default ask)`. The origin — `session`, `user` or `bundled` —
comes from the same answer as the verdict and not from a second call.

**The exit code carries the verdict**, so that a script can read it without
parsing the line: `allow` ends with `0`, `block` with `10`, `ask` with `11`.
`redact` also ends with `11`, because such a request is held today. A failure
is not affected by this: a URL without a scheme or with a fragment is
`CLI_004` and exit `1` before anyone is asked, and without a daemon this
command too ends with `2`.

With `--json` there is one line on `stdout`, with `verdict`, `matched`,
`rule_id`, `origin`, `position` and `passthrough`; the exit code stays the
same. `origin` and `passthrough` are `null` where the answer does not carry
the rule — the same "unknown" twice, never a `false` that claims more than is
known.

Normalisation happens in the daemon: upper and lower case, a trailing dot and
an internationalised name go out raw and are treated there as in a real
request. `tests/escape/esc-4-rules.sh` runs fifteen lines of the host table
from HUM-022 along exactly this path.

## `humanitl llm test`

Asks a single endpoint what it is. The same probe as behind the test button in
the setup and as with `doctor --probe-llm`: GET requests in the daemon on the
endpoint's model list (`<base>/models`, for an Ollama without a path prefix
first `/api/tags`), without redirects, none of it through the sandbox.
Credentials are sent only with `--with-key`: then the daemon sends the stored
access key as `Authorization: Bearer`, and only if `URL` has the same origin
(scheme, host, port) as `llm.endpoint`; `key_sent` in `--json` says whether
one was sent (HUM-140).

```
humanitl llm test <URL> [--timeout-ms N] [--with-key] [--json]

flavor: ollama
latency: 12 ms
models: 2
  qwen2.5-coder:14b
  llama3.1:8b
```

Before the first packet a line goes out on `stderr` that names the endpoint
and the two requests — bypassing the output control, so that `--json` and
`-q` do not swallow it either. At most six names of at most 40 characters each
are shown, the rest as `+N more`; the same limits as in the interface, because
a name from the network is text from a machine about which nobody has decided
yet. The full list is in `--json`.

An endpoint that does not answer ends with `1` and the daemon's Diagnostic
(`LLM_001`; `LLM_007` if the address could not even be read). Diagnostics
that are not a failure stand below the output and the run ends with `0`:
`LLM_006` for an address outside your own network, and `LLM_003` for a server
that answers, but as no known API — then it reads `flavor: unknown`, there
is no model list, and `answer:` (in `--json` the key `answer`) says what came
back instead, status and content type, for instance `HTTP 404, text/html`
(HUM-283). Whoever answered, answered. An address with a quote, an angle
bracket, a backslash or white space in it is refused before any request with
`CONFIG_003`, naming the character and its position and offering the cleaned
address: such a character is almost always left over from copying the address
out of a configuration file.

## `humanitl llm key`

The endpoint's access key, for a model that answers only with a key
(HUM-140). A call of the RPC `LlmKey`; the same one that serves the field
"Access key" of the settings.

```
humanitl llm key status [--json]
humanitl llm key set [--json]        # reads the key from stdin
humanitl llm key remove [--json]

access key: set (secret-service:humanitl/llm, secret-service)
```

`set` reads the key from `stdin`, never from the command line: `argv` is
visible in `/proc/<pid>/cmdline` to every process of the same user and in the
shell's history. At a terminal the input is hidden; from a pipe everything is
read up to the end, without the trailing line break
(`printf '%s' "$KEY" | humanitl llm key set`). The daemon stores it in the
system keyring (`secret-tool`, Secret Service), otherwise in
`$XDG_DATA_HOME/humanitl/keys/llm.key` with `0600` and then reports `CONFIG_019`.
Afterwards `config.toml` holds only the reference `llm.api_key_ref`. No output
carries the key, not even `--json`: it names `set`, `store`
(`secret-service`, `file` or `none`), `key_ref` and the Diagnostics.

`status` reports `LLM_009` if `llm.api_key_ref` names a key that the store
does not hand out. `remove` removes key and reference. White space around
the key and one quote at each end are removed before it is stored, so a key
copied with its quotes out of a configuration file works (HUM-283). A key
that is then empty, contains white space inside or is longer than 4096 bytes
is rejected with `CONFIG_020`, without repeating the key, and nothing is
written. As with every setting, a new key applies
to the next session; a running one keeps what it was started with.

## The global switches

Five switches apply to every subcommand, because `clap` carries them as
`global` (`daemon/bin/humanitl/src/cli.rs`, `GlobalOpts`). They stand before or
after the subcommand; both work.

| Switch | Effect |
|---|---|
| `--json` | Machine-readable output: exactly one JSON value on `stdout`, Diagnostics included. Hints on `stderr` are dropped; what a subcommand writes in JSON mode is described with it. |
| `--config PATH` | Reads this configuration file instead of the one in the user's configuration directory. The path applies to this call and changes nothing about the user's file. |
| `-v`, `--verbose` | Explains what is happening: hints that otherwise stay silent (`Renderer::detail`). Giving it several times works (`-vv`), today without a further level. |
| `-q`, `--quiet` | Only the result, no hints: `Renderer::note` no longer writes anything to `stderr`. Excludes `-v`; Diagnostics and errors remain. |
| `--profile NAME` | The profile of the session. Under `humanitl sandbox` the same flag names the bwrap profile; which meaning applies is decided by the subcommand. |

The result text of a subcommand is on `stdout` and stays there with `-q` too;
`-q` removes only the accompanying hints. A Diagnostic (`Diagnostic`) goes to
`stderr` and cannot be switched off with any of the switches, because it
explains the exit code.

## What `run` shares with the other subcommands

- `humanitl sandbox run` starts the sandbox in the process of the command line
  and is the way for self-tests and the escape tests. `humanitl run` starts it
  in the daemon; only there are proxy, recording and queue.
- `humanitl rules list` shows the rule set currently in force — after a start,
  therefore, that of the session, including the rules of its profile and its
  passthrough.
- `humanitl daemon status` says whether there is a daemon at all. That is the
  answer to exit 2.
- `humanitl doctor` says whether `run` can get a sandbox on this machine at
  all. That is the answer to exit 3, before it happens.
- `humanitl daemon install` creates the unit that starts the daemon at login.
  That is the answer to the doctor's line `daemon` and to
  `DAEMON_001` in the setup screen.
