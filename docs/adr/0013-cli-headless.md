# ADR-0013 · CLI as an equal client and headless operation

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

The obvious design for Humanitl is an application with an interface that
starts a proxy on the side. This design has three problems.

First: an agent run takes hours. Whoever closes the interface, or whose
session crashes, must not drag the agent down with it. Second: there are legitimate
uses without a screen — a script that runs an agent in a directory,
or a pure inference instance that only lets the LLM traffic
through. Third: if the interface is the only way, it becomes the
gatekeeper, and every capability has to be built there before it exists
(principle 10: "CLI is first-class").

The difficult question is the held request without an interface. Holding only makes
sense if someone decides. Without an interface there must either be another
human channel or an honest default.

## Decision

There are two binaries: `humanitld` (daemon) and `humanitl` (CLI). Both are
of equal rank; the interface and the CLI are equal gRPC clients
of the same daemon (ADR-0003).

Subcommands:

```
humanitl run [--profile NAME] [--work DIR] [--ask ui|terminal|none] [--llm URL] [-- CMD...]
humanitl sandbox run [--profile NAME] -- CMD...
humanitl sandbox argv [--profile NAME]
humanitl sandbox check
humanitl rules list|add|remove|test URL [--json]
humanitl flows list [FILTER] | show ID [--json]
humanitl audit verify|export [--format jsonl|csv] [--out FILE]
humanitl config get KEY | set KEY VALUE | schema | edit
humanitl daemon install|status|logs
```

Exit codes, binding: `0` ok, `1` user error (with Diagnostic), `2` daemon
not reachable, `3` sandbox check failed, `4` security violation
(for example an authority mismatch in the test), `10` `rules test` yields `block`, `11`
`rules test` yields `ask`.

**Two modes for held requests without a connected interface:**

- `--ask terminal` — a prompt in the terminal following the pattern of
  `pipelock`: host, method, size, findings, then `[a]llow [b]lock [r]ule`.
- `--ask none` — everything for which no rule applies is blocked.

`--ask terminal` only works for line-oriented commands
(`humanitl sandbox run -- curl …`, scripts, Aider in basic mode). With a
full-screen TUI agent such as OpenCode, `humanitl run --ask terminal` refuses to
start with Diagnostic `CLI_002` and suggests `--ask ui` (the interface is started
or attached) or `--ask none`. Detection runs via
`AgentAdapter::is_fullscreen_tui()`.

The bundled profile `llm-only` sets `--ask none` plus the
LLM passthrough rule. That is the "inference only" instance.

**Takeover by the interface.** If an interface connects later,
it takes over the hold queue seamlessly, because both are only gRPC clients of the daemon.
The state lives in the daemon, not in the client.

## Rationale

That the hold queue lives in the daemon and not in the client is the decision from
which everything else follows. The daemon holds the request, the client displays it. A
client can come and go; the request stays held until it is decided
or the deadline expires. That is why "close the interface" is harmless and why
an interface can take over a running terminal session without a
handover mechanism being necessary for it.

The two ask modes are the two honest answers to "nobody is watching".
`terminal` is the human at another place. `none` is the clear statement:
without a rule nothing goes through. A third mode "let everything through" would be the
abolition of the product and does not exist.

Refusing `--ask terminal` for full-screen TUI agents is a
decision against a broken state. OpenCode draws on the same
screen on which the prompt would have to appear; the result would be
a wrecked terminal and a question that the user cannot read. Better
a clean failure with two named ways out than an unusable
state.

The fixed exit codes make the CLI scriptable. `10` and `11` for
`rules test` are deliberately separate: a script can distinguish `block` and `ask`
without parsing the output. `4` for a security violation is
intentionally a code of its own, so that a CI pipeline can stop on it.

`llm-only` as a bundled profile shows that this mode of operation is not a special case
but a configuration: an agent that talks exclusively to the local
model, fully recorded, without internet and without anyone who
has to watch.

## Rejected alternatives

- **Only an application with an interface.** Loses all three properties from the
  context: no survival beyond the interface, no
  headless operation, interface as gatekeeper.
- **CLI as a thin wrapper around `humanitld` subcommands.** Would have created two ways into
  the core, one via gRPC and one direct. That is exactly what
  ADR-0018 rules out.
- **Ask in the terminal via a separate TTY or a second window.** Solves the
  full-screen problem technically and creates a channel that the user does not
  expect. `CLI_002` with two named ways out is more understandable.
- **`--ask none` as the default for all headless runs.** Would be safe, but makes
  scripts with `curl` unusable in which a human sits alongside.
- **A mode "allow everything" for automated runs.** Expressly
  rejected: it abolishes the product. Whoever wants to allow everything does not need Humanitl.
- **Interactive decisions via a file or a named pipe.** Would be
  scriptable, but nobody wants to release a held request via `echo`.
  For automation, rules are the right mechanism.

## Consequences

- The state of a session lives in the daemon. Clients are stateless enough to
  disappear and come back at any time.
- Every capability needs a CLI subcommand, even if it only prints `--json`.
  This rule is so important that it has its own ADR: ADR-0018.
- The exit codes are public behaviour and regression-tested. The
  escape tests and the demo scripts use the CLI instead of ad hoc scripts and
  are thus its test at the same time.
- `AgentAdapter` needs `is_fullscreen_tui()` so that `--ask terminal` can check its
  precondition.
- `humanitl run` is the standard path without flags: current directory as
  `/work`, discovered LLM server, OpenCode from the `PATH`.
- The CLI flags are generated from the config schema (ADR-0011), so that
  flag name and configuration key never drift apart.

## Affected issues

`HUM-064` (CLI skeleton with clap and sandbox subcommands), `HUM-067`
(`humanitl run` with `--ask terminal`/`--ask none` and takeover by the
interface), `HUM-065` (`humanitl rules|flows`), `HUM-070` (`humanitl
config|audit|daemon`), `HUM-066` (profiles `default` and `llm-only`),
`HUM-075` (`humanitl doctor --json`).
