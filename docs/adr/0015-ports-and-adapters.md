# ADR-0015 · Ports and adapters with a machine-enforced dependency direction

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl is a hobby project with a long lifetime. Such projects rarely fail
because of a wrong technology decision and almost always because of sprawl:
after two years every module knows every other one, a test needs a running
daemon, and a change to rule evaluation drags a change to the Protobuf
definition along with it.

At the same time the code should stay as lean as possible. The obvious answer
to sprawl — more abstraction layers — often produces the opposite: an
interface over every third-party library, a factory over every interface, and
nobody can find any more where something actually happens.

So a rule is needed that secures both at once: clear boundaries and little
code. And it must be machine-checkable, because conventions are the first
thing to fall under time pressure.

## Decision

**Three layers with one dependency direction.**

```
Adapters (outer)  ipc/tonic · sandbox/bwrap · proxy/hudsucker · recorder/sqlite
                  catalog/files · cli/clap                       IO, async, third-party libraries
        │ may point inwards
Application       session orchestration, hold queue,
                  policy decision (rule → verdict → hold)        async, but only via ports
        │
Core (inner)      core-types · rules · findings · audit · config no IO, no async, no proto
```

Dependencies point exclusively inwards. The core imports nothing from the
application or the adapters. The application knows adapters only as traits.

**The port list is closed in the MVP.** Every port has exactly one
implementation:

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

**A new port needs an ADR and a concrete second adapter.** No trait without a
second user in sight.

**Enforced by CI**, not by review:

- `tools/check-deps.sh` reads `cargo metadata` and checks every workspace
  dependency against the direction table in `backlog/CONVENTIONS.md` 3.1.
- The same script checks `#![deny(missing_docs)]` in every library crate.
- It checks that `TcpStream::connect` occurs only below
  `daemon/crates/proxy/src/egress/` (ADR-0017).
- It checks that no Flutter feature imports another feature.
- Generated code (Protobuf, riverpod, freezed) is gitignored and generated in
  CI; drift breaks the build.

**Code frugality, concretely:**

- No abstraction over third-party libraries. hudsucker, tonic and rusqlite are
  used directly, but each in **one** crate. If the library changes, that one
  crate changes.
- Value types instead of strings. `HostName`, `FlowId`, `DiagnosticCode` are
  types; a `String` in a signature is a suspect, unless it really is free
  text.
- Errors are values with meaning: `thiserror` per crate, no `anyhow` in
  library crates, `anyhow` only in `main` (ADR-0012).
- Thin binaries. `humanitld` and `humanitl` are wiring. An `if` in `main.rs`
  that makes a domain decision is in the wrong place.
- Deleting is a feature. Every sprint ends with a look at `experimental` flags
  and unused paths; what nobody uses for two sprints goes.

The same rules apply on the Flutter side: `core/domain` are freezed types
without a Flutter import, `core/ipc` is an interface with two implementations
(gRPC and fake), a feature imports no other feature, and the widget vocabulary
lives in `packages/ui` on top of `shadcn_flutter`. A feature imports
`lib/core/ui/ui.dart`, never a third-party widget package; `tools/check-deps.sh`
objects to every import of the library outside `app/packages/ui`
(ADR-0009, section "Revised on 2026-09-04 by the project owner").

## Rationale

The inward direction is the only rule one has to remember, and it carries
everything else. It makes the core testable without a network, without a
database and without a runtime: rule evaluation, state transitions, hash chain
and configuration precedence are pure functions over value types and can be
checked table-driven. This is not an end in itself — it is the reason why the
security-relevant logic is fully testable at all.

The closed port list is the counterweight to abstraction sprawl. Without it
the typical movement sets in: every module gets a trait as a precaution, and
after a year there are thirty interfaces with one implementation each. Ten
ports with a named second adapter in the future are a manageable set, and the
hurdle "ADR plus concrete second adapter" keeps it small.

That third-party libraries are **not** abstracted but each occurs in only one
crate is the compromise between the two failures. An interface over hudsucker
would be an invention that maps only the lowest common denominator of all
conceivable proxy libraries — more code, fewer capabilities. Using hudsucker
directly, but only in `humanitl-proxy`, keeps the cost of switching local
without inventing anything beforehand.

The machine check is the heart of this decision. An architecture rule that
exists only in people's heads lasts until the first deadline. `cargo metadata`
knows the actual dependency graph; a script that checks it against a table is
twenty lines and lasts for years. The same applies to the grep for
`TcpStream::connect`: it is coarse, but it catches exactly the error that
would silently undermine the egress guarantee.

## Rejected alternatives

- **Layering only as a convention in review.** The usual route and the reason
  why most projects lose their architecture. A review overlooks a line in a
  `Cargo.toml`; `cargo metadata` does not.
- **A single crate with modules.** Less ceremony, but Rust does not enforce
  module boundaries within a crate in terms of dependency direction: `mod
  rules` could use `mod proxy`, and nobody would notice.
- **Full Clean Architecture with use-case classes and DTOs at every
  boundary.** Enforces the same direction and costs many times the code as
  well as a translation layer per boundary. Disproportionate for a program of
  this size.
- **Ports for everything, as a precaution.** Rejected with the rule "no trait
  without a second user in sight". A trait with exactly one implementation
  that never gets a second one is cost without benefit.
- **Microservices instead of crates.** Separation by process boundaries
  instead of module boundaries. Absurd for a desktop program — with one
  exception that remains: the shim in the sandbox, because it has to be
  without tokio and without dependencies.
- **Check in generated code.** Convenient when building without a toolchain,
  but drift between source and product then becomes invisible. Instead:
  gitignored and generated in CI.

## Consequences

- The workspace has more crates than one would at first glance consider
  necessary. That is the price for the direction being checked by the
  compiler and by `cargo metadata` instead of by humans.
- New dependencies between crates need an entry in `tools/deps-allow.toml` and
  thus a conscious decision.
- `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` stand in every library
  crate; the second line is checked by the script.
- A second adapter (Docker, macOS, Postgres, Tor) touches neither core nor
  application. That is the promise ADR-0002, ADR-0008 and ADR-0017 work with.
- The test pyramid follows the layering: table-driven unit tests in the core,
  in-process integration tests against a fake upstream for the proxy,
  privileged escape tests for the guarantees, widget and golden tests against
  the fake daemon. A bug first gets a test on the lowest level at which it can
  be reproduced.
- The price is a little more ceremony when creating a new capability. The gain
  is that a change to rule evaluation never forces a change to the Protobuf
  definition.

## Affected issues

`HUM-074` (dependency lint `tools/check-deps.sh`), `HUM-001` (monorepo layout
with the crate split), `HUM-004` (core-types as the IO-free core),
`HUM-002` (CI job `deps-lint`), `HUM-062` (config in the core, precedence as a
pure function).
