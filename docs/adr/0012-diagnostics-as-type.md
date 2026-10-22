# ADR-0012 · Guided states as a type: `Diagnostic` instead of an error string

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl stands on a stack of kernel capabilities, system services and
third-party programs: unprivileged user namespaces, seccomp, bubblewrap in
a particular version, a systemd user session, a `$XDG_RUNTIME_DIR`,
an LLM server somewhere in the LAN, OpenCode in the `PATH`, a project folder with the
right permissions, a certificate that the agent has to accept. Each of these
parts can be missing or set up wrongly.

The target audience cannot diagnose these errors. A "Permission denied"
in the log is the end for them, not the beginning. Principle 7 states the
requirement: every state that is not green carries a reason in plain language and
an action that fixes it. No error without "why" and "what now".

This cannot be upheld as a convention. If an error path *may* return a
`String`, it will — usually under time pressure, usually at the
place where it hurts most. UI and CLI would then each have to
guess how to present an error, and the fix would remain prose.

## Decision

Error paths return a value, not text:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,      // Info | Warning | Error | Blocking
    pub title: String,
    pub why: String,
    pub fix: Option<FixAction>,
    pub docs: Option<String>,
}

pub enum FixAction {
    SetEnv { key: String, value: String },
    AddRule(Box<Rule>),
    InstallService,
    ChangeSetting { key: String, value: String },
    CopyCommand(String),
    OpenUrl(String),
    RemountReadOnly(PathBuf),
}

pub struct DiagnosticCode(pub &'static str);   // Schema: BEREICH_NNN
```

The type lives in `humanitl-core`. `FixAction` is an **enumerable action**,
not a text instruction: UI and CLI each render the same action in their own way (button
or copyable command, respectively), and a click executes it.

**Codes are registered, not invented.** All codes are constants with a
documentation comment in
`daemon/crates/core-types/src/diagnostics/codes.rs`; the table `AREAS` in
the same file is the binding list of reserved areas. As of
today: `DAEMON_001..019`, `IPC_001..009`, `CONFIG_001..009`,
`SANDBOX_001..029`, `PROXY_001..009`, `TLS_001..009`, `LLM_001..009`,
`RULES_001..009`, `TERM_001..009`, `RECORDER_001..009`, `LIMIT_001..009`,
`AUDIT_001..009`, `DOCTOR_001..019`, `CLI_001..009`
(`backlog/CONVENTIONS.md` 4.6 names the original subset). A code is
**never reused**; removed codes stay in place as `#[deprecated]`.
A CI test checks that every Diagnostic code used in the code is in the register
and lies within its area.

**Presentation.** The UI always shows Diagnostics at the place of the problem — on the
card, on the row, on the checklist entry —, **never as a modal**. The CLI shows
them as a block with `why:` and `fix:`.

**Definition of Done.** Every issue that introduces a new error path delivers
a `Diagnostic` for it with `why` and, where possible, `fix`. No `Err(String)` in
public daemon paths; the CI lint checks this.

## Rationale

A type enforces completeness. Whoever constructs a `Diagnostic` must give `code`,
`severity`, `title` and `why`; they cannot accidentally leave only the
technical cause behind. With a `String` the opposite is the
normal case.

`FixAction` as an enum instead of text is the point at which an
error message becomes help. "Set `SSL_CERT_FILE` to `/etc/humanitl/ca.crt`" is
an instruction that the user has to carry out. `FixAction::SetEnv { … }` is
something that the UI shows as a button and the CLI prints as a copyable command —
and both demonstrably mean the same thing, because it is the same value. Principle 9
demands exactly that: what is missing is offered, never merely reported.

The stable code makes errors addressable. A user can type `SANDBOX_007` into
a search, a bug report becomes unambiguous, and the documentation can
refer to it directly. That codes are never reused keeps old
bug reports and old documentation valid. The register with a CI check prevents
the obvious decay: two places that use the same code for
different things.

Diagnostics inline instead of modal is not a matter of taste. A modal tears the user out of
the context, has to be clicked away and is gone afterwards — exactly when the
user needs the information. At the place of the problem it stays as long as
the problem exists. That fits the basic stance of the interface (ADR-0009):
decisions inline, never modal.

`severity` separates "this is a hint" from "nothing goes on here".
`Blocking` is the state in which the sandbox screen disables the start and
never offers "start anyway" — a security check that can be clicked away
is not one.

## Rejected alternatives

- **`anyhow` with context chains.** Sensible in `main` and allowed there. Rejected for
  library crates: the chain is written for developers, not
  for users, and it carries no fix. `thiserror` per crate plus
  `Diagnostic` at the surface is the split.
- **`Err(String)` with the convention "please phrase it kindly".** Exactly the
  model whose failure can be seen in many programs.
- **Error codes as numbers (`E1042`).** Compact and unreadable. `SANDBOX_007`
  already says when read where to look.
- **Fix as free text.** Halfway: the UI could show it, but not
  execute it. The difference between "here it says what you should do" and "here
  is the button" is the whole difference for the target audience.
- **Translation of the error text in the daemon.** Rejected: the daemon delivers
  `code` plus structured fields, translation happens in the interface
  via ARB keys. Otherwise the daemon would have one language and the CLI a
  second.
- **Diagnostics as a modal dialogue.** See above; contradicts the basic
  interaction stance.
- **No register, codes assigned ad hoc.** Would be faster and would produce
  duplicates and gaps within a year.

## Consequences

- `Diagnostic.docs` and `FixAction::OpenUrl` carry `String`, not `url::Url`, so that the `url` crate stays out of `humanitl-core` (CONVENTIONS 4.11).

- `Diagnostic`, `Severity`, `FixAction` and `DiagnosticCode` are core types and
  have a proto mapping so that they can be transported over the event stream and in
  RPC responses.
- `FlowEvent` has a variant `Diagnostic`; a problem can therefore be attached to a flow,
  not only to an RPC result.
- There is a renderer contract for UI and CLI: same fields, same
  order, same meaning. Snapshot tests cover all codes in use
  (HUM-068).
- `humanitl doctor` is essentially a list of checks, each of which delivers a
  `Diagnostic`. That is why it can print `--json` and show the same information in the setup screen
  as in the terminal.
- New error paths cost somewhat more effort than a `bail!`. That is
  intended: the effort is spent once when writing and spares every user
  a dead end.
- `FixAction::AddRule` takes a `Box<Rule>` (`backlog/CONVENTIONS.md` 4.1),
  so that `Diagnostic` stays small and no separate rule stub type is needed.

## Affected issues

`HUM-063` (Diagnostic type, proto mapping, lint against `Err(String)`),
`HUM-068` (guided Diagnostics in the sandbox screen, snapshot tests for all
codes), `HUM-045` (TLS error detection with `FixAction::SetEnv`), `HUM-075`
(`humanitl doctor`), `HUM-044` (setup flow with checklist), `HUM-077`
(`FixAction::InstallService`).
