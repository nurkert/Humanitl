# Humanitl — Architecture principles

Applies to: Humanitl 0.1

> Purpose: From the start, the software should grow into a shape that does not sprawl historically even after years of hobby growth. At the same time the code should stay as lean as possible. This document fixes how both go together. It complements `BACKLOG.md` (decisions, plan) and `backlog/CONVENTIONS.md` (canonical names).

## 1. The three sentences

1. **A core that knows nothing about the world.** Everything Humanitl *means* (flow lifecycle, rules, findings, Diagnostics, audit chain) lives in crates without IO, without async, without Protobuf. These crates are pure functions over value types and can be tested table-driven.
2. **An event stream as the backbone.** Every state change of a flow becomes a `FlowEvent`. UI, CLI, recorder, audit and later plugins are consumers of this stream. Nobody asks the proxy for its state; everyone listens.
3. **One public interface.** The gRPC proto `humanitl.v1` is the only way to talk to the daemon. UI, CLI, tests and plugins are clients with equal rights. There is no back door for "just quickly".

## 2. Layers and dependency direction

```
                    ┌────────────────────────────────────────────┐
  Adapters (outer)  │ ipc (tonic)  sandbox/bwrap  proxy/hudsucker │  IO, async, third-party libraries
                    │ recorder/sqlite  catalog/files  cli/clap    │
                    │ audit-log/jsonl                             │
                    └───────────────────┬────────────────────────┘
                                        │ may point inwards
                    ┌───────────────────▼────────────────────────┐
  Application       │ session orchestration, hold queue,          │  async, but only via ports
                    │ policy decision (rule → verdict → hold)     │
                    └───────────────────┬────────────────────────┘
                                        │
                    ┌───────────────────▼────────────────────────┐
  Core (inner)      │ core-types  rules  findings  audit  config  │  no IO, no async, no proto
                    └────────────────────────────────────────────┘
```

Rules:

- Dependencies point inwards only. The core imports nothing from the application or the adapters. The application knows adapters only as traits (ports).
- A port is a trait in the core or in the application; an adapter is its implementation outside. In the MVP every port has exactly one implementation. A second adapter (Docker, macOS, external plugin) touches neither core nor application.
- Enforced by CI: `cargo deny` for licences and duplicates, a script `tools/check-deps.sh` that checks the Cargo graph against the allowed direction (from `backlog/CONVENTIONS.md` 3.1), `#![deny(missing_docs)]` in all library crates and, for the core crates in `[core]` of `tools/deps-allow.toml`, the ban on the dependencies in `[core].forbidden` together with a source check against the patterns in `[core].source` (files, runtime, own threads and processes, sockets; HUM-155).

Ports (binding list, complete for the MVP):

| Port | Defined in | MVP adapter | Later adapters |
|---|---|---|---|
| `SandboxBackend` | sandbox | bwrap | docker, seatbelt, microvm |
| `AgentAdapter` | core | opencode | aider, codex, claude-code |
| `Detector` | findings | regex/checksum | presidio-grpc, wasm |
| `FlowStore` | recorder | sqlite | postgres (team) |
| `AuditSink` | audit | jsonl-hashchain | remote-anchor |
| `CatalogSource` | catalog | bundled-yaml | user-yaml, team-url |
| `Resolver` | proxy | hickory (after allow) | doh |
| `Egress` | proxy | direct | http-proxy, socks5h (Tor) |
| `Bridge` (shim) | sandbox | proxy-socket | cdp-socket (browser) |
| `AskChannel` | application | grpc-ui, terminal, none | plugin |

## 3. The flow lifecycle as a state machine

```
Received ──analyze──▶ Analyzed ──evaluate──▶ Held ──decide──▶ Decided ──▶ Forwarded ──▶ Responded ──▶ Recorded
                          │                    │                 ▲
                          │ (rule: allow)      │ (timeout)       │
                          └────────────────────┴─────────────────┘
                          (rule: block) ──────────────────────────▶ Decided(Block) ──▶ Recorded
```

- The state machine is an `enum` with a method `on(self, &FlowEvent) -> Result<Self, InvalidTransition>`. There is no other way to change the state. Invalid transitions are bugs and are enumerated completely in the test.
- The proxy handler *drives* the state machine; it does not *own* it. It calls `on`, publishes the event and, where needed, waits for the decision. This keeps the handler thin and the logic testable without a network.
- Timeouts are events (`TimedOut`), not special paths.

## 3b. Parity of UI and CLI

Every capability exists exactly once, as an RPC in the daemon. UI and CLI are interchangeable, thin clients of the same proto. Rules: a new RPC brings its CLI subcommand along in the same issue; the UI counterpart follows in the next sprint at the latest; `docs/reference/parity.md` is generated and checked in CI (ADR-018). Domain logic in `app/` or `bin/humanitl` is an architecture violation, even when it is small. The common use case is fully described as the profile `default`; customisation changes values, never code.

## 4. Code frugality, concretely

Growth is allowed, sprawl is not. The distinction:

- **No trait without a second user in sight.** The port list above is closed. Anyone who wants a new port first writes an ADR with the concrete second adapter.
- **No abstraction over third-party libraries.** hudsucker, tonic, rusqlite are used directly, in *one* crate. If the library changes, that one crate changes.
- **Value types instead of strings.** `HostName`, `FlowId`, `DiagnosticCode` are types. A `String` in a signature is a code smell, unless it really is free text.
- **Errors are values with meaning.** `Diagnostic` everywhere a human could see the cause. `thiserror` per crate, no `anyhow` in library crates, `anyhow` only in `main`.
- **One configuration source.** Settings are Rust types with a schema. UI, CLI and docs are generated from them, never maintained in parallel.
- **Thin binaries.** `humanitld` and `humanitl` are wiring, not logic. If `main.rs` contains an `if` that makes a domain decision, it is in the wrong place.
- **Generated code is not source code.** Proto output, riverpod and freezed output are gitignored and generated in CI. Drift breaks the build.
- **Deleting is a feature.** Every sprint ends with a look at `experimental` flags and unused paths. What nobody has used for two sprints goes.

## 5. Flutter side, same rules

- `core/domain` are freezed value types, mirrors of the Rust core types, without a Flutter import.
- `core/ipc` is an interface `DaemonClient` with two implementations: gRPC and fake. Every screen is built against the interface, every widget test runs against the fake.
- Since HUM-044 the **state of the connection** also lives there (`core/ipc/connection.dart`): `ConnectionStatus` and the predicate `linkLiveProvider`, which says whether the daemon is answering right now. It used to live under `features/shell`, but the queue and the event stream need the same question answered and may not import `features/shell` (section 5, enforced by `tools/check-deps.sh`). A question that more than one feature asks belongs in `core` — otherwise it is answered anew, and a little differently, in every place, and exactly that had led to five findings.
- For the same reason the **UI clock** has also lived in `core` since HUM-044 (`core/time/now.dart`): `nowProvider` is the one ticker from which every label, every countdown and every running time reads its second (`docs/UX.md` 7, "Two clocks, not one"). It lived under `features/intercept` as long as only the queue needed it; the sandbox's running time now reads the same one, so that both stop at the same moment when the connection breaks. The old path still exports the file.
- Since HUM-116 the **body view** also lives in `core` (`core/body/`): `BodyView` with tree, form, raw and hex, including unpacking and parsing. With it the queue shows the body of the request, the history request and response, and both fetch the bytes through `flowBodyProvider` (`core/body/flow_body_provider.dart`), the one body provider (`backlog/CONVENTIONS.md` 3.9). It lives with the view and not in `core/ipc`, because it delivers the view's types (`RawBody`, `BodyKind`); `core/ipc` stays the transport and imports nothing from `core/body`. What distinguishes the screens they pass in: the headers of the side the body belongs to, and the findings. They keep reading their detail from a provider of their own.
- Features are directories with screen, providers, widgets. A feature imports no other feature, only `core`. Cross-cutting widgets live in `packages/ui`.
- State flows in one direction: event stream → provider → widget. Widgets call `DaemonClient` only through provider methods (mutations), never directly.
- The widget vocabulary lives in `packages/ui` and builds on `shadcn_flutter` (ADR-0009, section "Revised on 2026-09-04 by the project owner"). Only this package imports the library; a feature imports `lib/core/ui/ui.dart`, never a third-party widget package. `tools/check-deps.sh` checks both.

## 6. Tests as architecture guards

| Level | What it protects | Where |
|---|---|---|
| Unit, table-driven | Core semantics (globs, transitions, hash chain, config precedence) | `crates/*/src`, `#[cfg(test)]` |
| Integration in-process | Proxy handler against an axum fake upstream, no subprocess | `crates/proxy/tests` |
| Escape (privileged) | The three guarantees | `tests/escape`, own CI job |
| Widget + golden | UI states against the fake daemon | `app/test` |
| e2e | One user path per milestone | `tests/e2e`, xvfb |
| Fuzz (nightly) | Parsers and decoders | `daemon/fuzz` |
| Dependency lint | Dependency direction | `tools/check-deps.sh` |

Rule: a bug first gets a test on the lowest level at which it can be reproduced.

## 7. What we deliberately do not do

- No event sourcing of the in-memory state. Events are output, not truth. Truth is the state machine plus SQLite.
- No plugin system in the MVP. Only the ports. Plugins come as external gRPC clients once three real needs are documented.
- No microservices. One daemon, one UI, one CLI. Separation by crates, not by processes, with one exception: the shim in the sandbox, because it has to be without tokio and without dependencies.
- No home-grown cryptography. rustls, rcgen, sha2, hmac, aes-gcm from RustCrypto, keyring.

## 8. The agent in the sandbox (awareness and feedback)

The agent should know where it is with as few tokens as possible and be able to react to feedback. Everything runs over the one existing channel, the proxy socket. No new channel, no new gap in the guarantees.

1. **Briefing.** The `AgentAdapter` places a global instruction file in the sandbox (OpenCode: `~/.config/opencode/AGENTS.md`, Claude Code: `~/.claude/CLAUDE.md`), never in the project directory. Its content is a bundled template of about 150 tokens, in the user's language. Key statements: no direct internet access; every HTTP request goes through a proxy, a human decides; waiting is normal, do not abort; a response `403` with `Blocked by Humanitl` is final, do not retry, inform the user instead and suggest an alternative; status query at `http://humanitl.internal/`.
2. **Feedback on blocking.** The action bar has an optional note field. The text ends up in the 403 body under `note:` and in the header `X-Humanitl-Note`. The agent sees it in the tool result and can react ("Use PyPI instead of GitHub").
3. **Meta endpoint.** The proxy serves the virtual host `humanitl.internal` itself, without an upstream, only `GET` and `POST` with small text responses: `/` short status and the currently valid rules, one line per rule; `/why/<flow-id>` the reason for a decision; `/ask` with a free-text body, appears in the UI as a card "The agent asks for …" with a rule suggestion. The endpoint is read-mostly; `/ask` only creates a UI card, never a rule.
4. **Visibility in the terminal.** When a request is held, the daemon writes a line into the agent's terminal (over the PTY stream, as a status line, not into stdin). This way the human at the agent sees what is currently pending.

What this is not: the agent gets no way to create rules, influence decisions or bypass the user. `/ask` is a request, not an action.
