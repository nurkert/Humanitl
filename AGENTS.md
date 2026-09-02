# AGENTS.md — briefing for external reviewers and helper agents

You are reading this because a tool (Codex, Antigravity, Gemini or similar)
was pointed at this repository. Most of the time your job here is a
**read-only critical review of a diff**. The request you receive names the
exact task; this file gives you the context to do it well.

## What Humanitl is

A Linux desktop tool that lets an AI coding agent use the internet under human
control. The agent runs in a sandbox with **no network interface**. Its only
way out is a loopback TCP connection over `lo` to a bridge held by the
in-sandbox shim, which forwards across a single bind-mounted Unix socket to the
moderating proxy on the host. Every request that no rule decides is held,
shown to a person with headers, parameters and body, and leaves only after
they allow it. Rules cover traffic the person already trusts: allow, block, or
ask. One rule is special: the declared LLM passthrough to the language model on
the LAN is streamed and logged, not held. Everything is recorded (persisted
from M2 on, once the SQLite recorder of HUM-026 exists).

Stack: Rust daemon (`daemon/`, hudsucker-based MITM proxy, bubblewrap sandbox,
seccomp shim, SQLite recorder, tonic gRPC over a Unix socket) and a Flutter
desktop client (`app/`). The CLI and the desktop app are thin clients of the
same gRPC contract; all domain logic lives in the daemon.

## The three guarantees you must protect

1. **No network interface.** `bwrap --unshare-all --cap-drop ALL`: only `lo`,
   empty routing table, no DNS, no ICMP, no QUIC, no capabilities. The
   `--cap-drop ALL` is not implied by `--unshare-all`; its absence is a defect.
2. **One door.** Exactly one Unix socket, bind-mounted as a file, leads to the
   proxy on the host. The proxy listens on that Unix socket only, never on a
   host loopback TCP port. Nothing else from `$XDG_RUNTIME_DIR`, `/run`,
   `/tmp`, X11, Wayland, D-Bus or Docker is mounted.
3. **No new doors.** seccomp allows `socket()` only for `AF_INET`/`AF_INET6`
   with `SOCK_STREAM` (`arg1` masked with `0xff` so that `SOCK_NONBLOCK` and
   `SOCK_CLOEXEC` pass) and refuses every other family and type with `EPERM`,
   plus ptrace, io_uring, process_vm and keyctl. x32 syscalls
   (`nr & 0x40000000`) get `EPERM` through a hand-written BPF prelude; an
   architecture mismatch kills the process. The filter lives in
   `daemon/bin/humanitl-shim/src/seccomp.rs`.

Five side channels are declared in `BACKLOG.md` section 4.2 and must stay
visible or constrained: the project directory `/work` (read-write, masked
paths), the LLM passthrough (logged, not held), terminal output (OSC 52 and
OSC 8 disabled, untrusted banner), hostnames in the log, and package caches
(per project). The first two are the open data paths. Anything that weakens
the three guarantees or hides one of these channels is a blocking finding,
even if the diff looks small.

## Where the truth lives

| Question | File |
|---|---|
| What are we building, and why these decisions | `BACKLOG.md` sections 0 to 5, ADR-001 to ADR-018 |
| Layering, ports, what code may depend on what | `docs/ARCHITECTURE.md`, `tools/deps-allow.toml` |
| Binding names, types, paths, defaults | `backlog/CONVENTIONS.md` — **section 4 overrides section 3** |
| The specification of the issue under review | `backlog/sprint-N.md`, heading `## HUM-xxx` |
| Threat model and what the audit chain does and does not prove | `docs/SECURITY.md`, `docs/THREAT-MODEL.md` |
| Definition of done | `CONTRIBUTING.md`, `backlog/CONVENTIONS.md` section 3.12 |
| Diagnostic code register | `daemon/crates/core-types/src/diagnostics/codes.rs`, `docs/DIAGNOSTICS.md` |
| Protobuf contract and generation | `proto/`, `scripts/gen-proto.sh`, `docs/PROTOCOL.md` |
| Process rules for AI sessions | `CLAUDE.md`, `CONTRIBUTING.md` |

Documentation prose is German; identifiers, code and commit messages are
English. Read the German; do not ask for a translation.

## How to review

- Start from the issue specification, not from the diff. Check the acceptance
  criteria one by one against the code that is actually there.
- Inspect the whole working tree, not only tracked modifications: run
  `git status --short`, then `git diff HEAD` for tracked files and read every
  new file from `git ls-files --others --exclude-standard`. A diff review that
  skips untracked files misses most of a new issue.
- Distrust the implementer's summary. Run what you can: `make check` is the
  complete local gate and skips missing rustfmt/clippy on its own. The pieces:
  `cd daemon && cargo build --workspace --all-targets && cargo test --workspace`,
  `./tools/check-deps.sh`, `scripts/ci/lint-docs.sh`,
  `scripts/ci/lint-no-string-errors.sh`, and for Flutter
  `cd app && flutter analyze && flutter test`. On this machine there is no
  rustfmt, clippy, rustup, protoc or buf; do not report their absence as a
  defect.
- Reviewers may run concurrently. To avoid fighting over the cargo lock, build
  into your own target directory: `export CARGO_TARGET_DIR=$PWD/daemon/target/review-<yourname>`
  (for example `review-codex`, `review-agy`). Do not delete or modify the
  shared `daemon/target/`.
- Look hardest at: the sandbox argument vector and seccomp rules; the proxy's
  hold, block-response and note sanitisation; DNS resolution timing (only after
  allow, never before); private-address rejection; the state machine
  transitions; anything returning `Err(String)` instead of a `Diagnostic`;
  `unwrap`/`expect`/`panic` outside tests and `main`; tests that assert nothing or only
  the happy path; code placed in the wrong crate for the dependency rules;
  public items without a doc comment; new settings without tier, description
  and default; new UI strings missing from `app/l10n/app_en.arb` or `app_de.arb`.
- Report only what you confirmed by reading or running. Give file and line, the
  concrete problem, and the concrete fix. An empty list is a valid answer;
  invented style opinions are not. Rank by severity:
  - **blocking**: weakens one of the three guarantees or hides a declared side
    channel; breaks build or tests; contradicts `docs/SECURITY.md`; leaves an
    acceptance criterion of the issue unmet.
  - **major**: violates the dependency direction or the crate placement rules;
    an error path without `Diagnostic` (`Err(String)`, `unwrap`/`expect` outside
    tests and `main`); a public item or setting missing what the definition of
    done requires (docs, tier, default, tests).
  - **minor**: inconsistencies, missing l10n entries, low-risk edge cases.
- Stay read-only unless the request explicitly asks you to change files. Never
  run git commands that change state.

## What a good finding looks like

```
[blocking] daemon/crates/proxy/src/handler.rs:142
Resolves the hostname before the hold decision; leaks the name via DNS
even when the user later blocks. Move the resolver call after `Decided(Allow)`
and pin the returned IP into `Egress::connect` (ADR-006).
```
