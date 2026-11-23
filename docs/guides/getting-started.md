# Getting started from the terminal

Applies to: Humanitl 0.1

This guide walks the shortest path through a session from the terminal: start Claude Code in a
project folder with `humanitl go claude`, leave it running, pick it up again in the desktop app,
and clean up afterwards. Every command and key below exists in this version; all flags are in the
[CLI reference](../reference/cli.md), the details in [`docs/cli.md`](../cli.md). The path through
the app alone is [Your first session](first-session.md).

## Before you start

- A finished [installation](install.md): `humanitl daemon status` answers, and `humanitl sandbox
  check` shows three `✓`.
- Claude Code where Humanitl finds it: installed on this machine, or in Humanitl's
  [agent depot](agent-depot.md) (`humanitl agents install claude`).
- Its access: an Anthropic API key, stored with `humanitl llm key set` (read from `stdin`, kept in
  the system keyring, never in a file of the project). The agent depot guide explains the key and
  a gateway in [step 3](agent-depot.md#3-give-claude-code-its-access).

The example project is `~/shop`.

## 1. Start with `humanitl go claude`

```sh
cd ~/shop
humanitl go claude
```

`claude` is the agent; it holds for this start only. The project folder is the current directory
(or `--dir PATH`).

**The first time in a folder** Humanitl does not know, one screen comes before anything starts:
the folder's name, one line with the defaults (agent, folder mode, isolation, where requests are
asked, model server) and the focus on "Start". Press `Enter` (or `Space`) to start with them; nothing
has to be chosen. `a` opens "Customize…" with every field: the arrow keys move between fields and options,
`Enter` on a field only chooses. The agent's field also offers where its program comes from, this
machine or the agent depot. An option that is not available here is shown with its reason.
`Esc` or `q` cancels, and then nothing is saved and nothing starts.

"Start" saves the choice for this folder in the daemon, not in the folder. The next `humanitl go`
here shows no screen. To change the choice later, use `humanitl settings --project .`; to see it,
`humanitl projects show .`.

Because Claude Code sends its inference to Anthropic, the first start of this agent with this
provider asks once whether to start with inference at Anthropic (`… It asks once per agent and
provider. [y/N]`). Afterwards a short orange line on every start says where the inference goes.

Then the fist of the logo turns while the sandbox starts, the three isolation checks are reported,
and Claude Code fills the terminal as if it had been started directly. Requests to the internet
that no rule decides wait for you; by default you decide them in the app (`--ask ui`).

## 2. Inside the session

Every key goes to Claude Code except the prefix key `Ctrl+B` (as in tmux), followed by one key:

| Keys | Effect |
|---|---|
| `Ctrl+B ?` | Show this table. |
| `Ctrl+B ,` | The settings over the agent; `Esc` returns. |
| `Ctrl+B i` | The interceptor: the held requests. Choose one with `↑`/`↓`, then `a` allows it once, `b` blocks it, `r` builds a rule (sent only after `Enter`); `Esc` returns. |
| `Ctrl+B d` | Detach; the session keeps running. |
| `Ctrl+B Ctrl+B` | Send one `Ctrl+B` to the agent. |

When a request is held, the terminal bell rings once (`terminal.bell_on_hold = false` turns that
off). If Claude Code leaves a remnant of a view on the screen, `Ctrl+L` in Claude Code redraws it.

## 3. Detach

Leave the session in either of two ways:

- `Ctrl+B d`. The command prints `Detached; the session keeps running.` and gives the terminal
  back.
- Close the terminal window, or lose the SSH connection. `humanitl go` detaches on that as well
  (`terminal.on_close = "detach"`, the default; `"stop"` ends the session instead).

The session keeps running in the daemon. Check from any terminal:

```sh
humanitl sessions
```

Its first line says `no terminal attached` while nobody is attached. Requests the agent sends
meanwhile wait for a decision in the app or in the interceptor of a new attach; when their
deadline runs out (`hold.timeout_secs`) they are blocked, never allowed.

## 4. Attach again, in the app

```sh
humanitl app .
```

opens the start window of the desktop app with this folder preselected, or brings a running app
forward. On the tab "Projects", the session stands under "Running", marked "No terminal attached";
"Attach" opens it in the app with its terminal and its queue of held requests.

The terminal works too: `humanitl go` in the same folder attaches with the line `Attached to the
running session in shop`, and `humanitl sandbox attach` works from any folder. A `go` that
attached never stops the session, whatever happens to its terminal.

To end the session, quit Claude Code as usual, or stop it in the app on the Sandbox page with
"Stop" and the confirmation "Stop the agent".

## 5. Clean up

**Changes.** What happened to the folder depends on the folder mode of the first-start screen.
When its line of defaults said `disposable folder`, the agent wrote into a disposable layer, not
into `~/shop`. When it said `folder read-write`, this system cannot keep a disposable layer here
(the default `auto` then falls back to direct writes, and the start names the reason with
`SANDBOX_033`): the agent wrote straight into `~/shop`, and there is nothing to decide. Whoever
needs writes kept out of the folder starts with `humanitl go claude --disposable`, or chooses
`disposable folder` under "Customize…"; where the system cannot do that, the start refuses with
`SANDBOX_030` instead of writing into the folder.

After a disposable run that ends at a terminal, the command lists the changes and asks: `[a]pply
all`, `[d]iscard all`, `[v]iew the diffs`, or keep them for later. Changes kept for later wait;
the next `humanitl go` in the folder asks again, the app marks the project "Changes waiting", and

```sh
humanitl sandbox changes --stat
humanitl sandbox apply      # or: humanitl sandbox discard
```

decides them at any time.

**Recorded data.** Every request the session made is recorded with its bodies. To clean up the
project:

```sh
humanitl projects clean .
```

The plan covers the recorded requests with their bodies and, unless a session runs there or changes
wait that this cleanup does not discard, the stored project choice and the register entry of the folder. The daemon lists
every item with its size and what stays, and asks `[y/N]`; only a yes deletes, and only exactly the
plan shown. Changes that still wait go only with `--include-waiting-changes`, and are then lost for
good. In the app, the tab "Clean up" of the start window does the same.
`humanitl projects forget .` removes the stored choice and the register entry of the folder
(not while a session runs there or changes wait) and keeps the recorded requests; the next `go` there shows the first-start screen again.

The project folder and the audit chain are never deleted by any of this
([SECURITY, cleanup and forgetting](../SECURITY.md#cleanup-and-forgetting-hum-533)).
