# ADR-0014 · Agent awareness and feedback over the one existing channel

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

An LLM agent in the Humanitl sandbox experiences an environment it was not
trained for. Its `webfetch` hangs for five minutes and then returns `403`. Its
update check fails. A `curl` runs into a timeout. Without an explanation it
reacts the way agents react to errors: it retries, varies the URL, tries a
different route and burns time and tokens in the process — and the human sees
a queue full of retries of the same blocked request.

Conversely, the human often has a reason when blocking that the agent could
use: "not GitHub, take PyPI", "not this domain, that is telemetry". Today this
reason goes to waste.

The temptation is a second channel: a control socket, a file the agent reads,
a sidecar. Each of these is a new door into the sandbox and thus a crack in
the guarantee "exactly one door" (ADR-0002).

## Decision

Everything runs over the one existing channel, the proxy socket. No new
channel, no new capability for the agent, no new gap in the guarantee.

**1. Briefing.** At start the `AgentAdapter` places a global instruction file
**in the sandbox** — for OpenCode `~/.config/opencode/AGENTS.md`, for Claude
Code `~/.claude/CLAUDE.md` — and **never in the project directory**. Its
content is a bundled template of about 150 tokens in the user's language
(`ui.language`). Key statements: no direct internet access; every HTTP request
goes through a proxy, a human decides; waiting is normal and no reason to
abort; a response `403` with `Blocked by Humanitl` is final and is not retried
— instead inform the user and suggest an alternative; status query at
`http://humanitl.internal/`.

**2. Feedback on blocking.** The action bar has an optional, expandable note
field. The text ends up in the `403` body under `note:` and in the header
`X-Humanitl-Note`. The agent sees it in the result of its tool call and can
react to it. The note is cleaned before both (`sanitize_note` in
`humanitl-core`, `backlog/CONVENTIONS.md` 4.11): `CR` and `LF` become spaces,
other control characters are dropped, as are invisible characters (zero-width
characters, bidi overrides, BOM, soft hyphen, tag characters) that could fake
a different text in the agent's terminal than the one the user wrote. At most
500 characters remain. The header value is additionally restricted to visible
ASCII characters plus `SP` and `HTAB` (RFC 9110 §5.5); non-ASCII stands only
in the body (no header injection).

**3. Meta endpoint.** The proxy serves the virtual host `humanitl.internal`
itself, without an upstream and without DNS, with small `text/plain`
responses:

| Path | Method | Response |
|---|---|---|
| `/` | GET | Short status: session, ask mode, timeout, valid rules (one line per rule) |
| `/why/<flow-id>` | GET | Reason for the decision including the note |
| `/ask` | POST | Free text up to 2 KB; creates an `AgentAsk` event and in the UI a card "The agent asks for …" with a rule suggestion |

Other methods are refused. `/ask` creates **only** a card in the UI, **never**
a rule.

**4. Visibility in the terminal.** When a request is held, the daemon writes a
status line into the agent's terminal — over the PTY stream, not into
`stdin`. The human looking at the terminal thereby sees what is currently
pending.

**What this explicitly is not:** the agent gets no way to create rules,
influence decisions or bypass the user. `/ask` is a request, not an action.

## Rationale

The proxy socket already exists, is already the only door, is already fully
recorded and is already subject to rule evaluation. Using it as well for
status information and feedback costs no additional attack surface: a request
to `humanitl.internal` is a request like any other, except that the proxy
answers it itself.

The briefing costs about 150 tokens and saves many times that. An agent that
knows that `403 Blocked by Humanitl` is final does not retry. An agent that
knows that waiting is normal does not abort after thirty seconds. This is the
cheapest improvement in the whole design.

That the briefing goes into a **global** configuration directory of the
sandbox and not into the project directory matters: `/work` is the user's
project folder, often a Git repository. A file that Humanitl places there ends
up in a diff, perhaps in a commit, and eventually in someone else's
repository. The global directory lives in the sandbox's tmpfs and disappears
with it.

The note in the `403` is the shortest path from the human's decision to the
agent's behaviour. It uses a response path the agent reads anyway and needs no
protocol. Header **and** body, because some tools pass only one of the two on
to the model.

`/ask` is deliberately a dead end. The agent can phrase a request, it appears
in the UI as a card with a rule suggestion, and there its effect ends. An
agent that could create rules would have the control this is all about; an
agent that cannot say anything at all leaves the human guessing. The card is
the compromise that leaves the balance of power unchanged.

The status line in the terminal solves a perception problem: the human sits in
front of the agent's terminal and sees a standstill there. The line tells them
that it is not the agent that hangs but that a decision is pending. It goes
over the output stream, never over `stdin`, so that it cannot be mistaken for
the agent's input.

## Rejected alternatives

- **A second socket or a control file for agent communication.** The obvious
  route, and the only one that breaks the guarantee "exactly one door". Ruled
  out.
- **Environment variables for the status.** Static at start; rules and state
  change during the session. Too little room for a briefing.
- **Briefing in the project directory (`/work/AGENTS.md`).** The simplest and
  the default location in many agents. Rejected because it pollutes the
  user's repository.
- **Inject the briefing as a system prompt via the LLM passthrough.** Would
  have meant that Humanitl writes into the user's prompts. A massive
  intervention, hard to debug, and hardly implementable cleanly with
  streaming.
- **`/ask` automatically creates a rule or an approval request with an
  auto-timeout.** Rejected: with it the agent could steer the decision flow
  instead of only informing it.
- **No feedback on blocking.** The current state without this ADR. The agent
  keeps trying, the human keeps blocking, and nobody learns anything.
- **Write the status line into the agent via `stdin`.** Would be convenient
  for line-based agents and is an injection into the input stream of a program
  we do not trust. Ruled out.

## Consequences

- `humanitl.internal` is a reserved hostname. It is never resolved and never
  passed on to an upstream; rule evaluation sees it as a special case.
- The meta endpoint is part of the proxy's attack surface and is treated
  accordingly: only `GET` and `POST`, fixed paths, `/ask` with a 2 KB upper
  limit, tests for all three paths and for the refusal of other methods.
- The note extends `Decision::Block { reason, note }` and with it the core
  type, the proto (`DecideRequest.block.note`), the audit log and the history
  view.
- Header injection via the note is a test case, not a residual risk: length
  limited, `CR`/`LF` to spaces, control and invisible characters removed,
  header value restricted to visible ASCII plus `SP`/`HTAB`.
- The `AgentAdapter` gets `files(&self, ctx) -> Vec<(PathBuf, Vec<u8>)>` for
  files that are created in the sandbox. The briefing is the first use of it;
  `opencode.json` is the second.
- The briefing template lives under `agents/opencode/briefing.{en,de}.md` and
  is thus readable, customisable and translatable instead of standing in the
  code.
- A test checks that after the start the file exists in the sandbox **and**
  the project directory is unchanged.

## Affected issues

`HUM-071` (agent briefing), `HUM-072` (block with note), `HUM-073`
(meta endpoint `humanitl.internal`), `HUM-037` (AgentAdapter and
OpenCode profile), `HUM-042` (terminal with status line).
