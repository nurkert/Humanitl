# ADR-0003 · gRPC over a Unix domain socket as the only interface to the daemon

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl consists of three programs: the daemon `humanitld` (Rust), the
interface `humanitl` (Flutter/GTK) and the command-line tool `humanitl`
(Rust). The interface must receive a live stream of events (every held
request appears immediately), send decisions back, query rules and
recordings and pass a terminal through bidirectionally. The command-line tool
needs the same capabilities.

The connection may only be local. It must not be reachable for other users of
the machine, and under no circumstances may it be reachable from inside the
sandbox — whoever can control the daemon can release themselves. At the same
time the interface is meant to be long-lived: it is the place where plugins
will dock later (`BACKLOG.md` 6).

## Decision

The only interface to the daemon is gRPC over a Unix domain socket.
The contract consists of three files under `proto/humanitl/v1/`, all in the
package `humanitl.v1`: `common.proto` with the enums `Method`, `Scheme` and
`Upgrade`, `rules.proto` with the rule model, and `humanitl.proto`, which
imports both and declares the service `Humanitl`. On the Rust side this is a
single module `humanitl_ipc::v1`. The server is `tonic` in the crate
`humanitl-ipc` — the only crate in the workspace that knows Protobuf. It also
exports the constants `PROTO_MAJOR`, `PROTO_MINOR` and
`TOKEN_METADATA_KEY` (`x-humanitl-token`), so that neither daemon nor CLI
spell these values a second time. Clients are the Flutter app (Dart `grpc`
5.x via `InternetAddress.unix`), the CLI and the tests.

The socket lives at `$XDG_RUNTIME_DIR/humanitl/daemon.sock` with mode `0600`
in a directory with mode `0700`. In addition, every call carries the
metadata header `x-humanitl-token` with the session token from
`$XDG_RUNTIME_DIR/humanitl/token` (`0600`). The socket is never mounted into
the sandbox; the sandbox sees only the proxy socket
(ADR-0002, guarantee 2).

Rules for the proto:

- Keep it boring: no `google.protobuf.Any`, few nested `oneof`s.
- IDs as `string` (UUID text), timestamps as `google.protobuf.Timestamp`,
  enums always with `_UNSPECIFIED = 0`.
- Bodies never stand inline in events, only as
  `BodyRef { bytes sha256; uint64 size; bool truncated }`; the content comes via
  its own stream `GetBody`.
- `GetInfo` returns daemon version, proto version and capabilities. The UI
  refuses the connection to a higher major version instead of guessing.
- Rust codegen runs in the build script via `protox` and `tonic-prost-build`
  into `OUT_DIR`; an installed `protoc` is not needed for it. The
  descriptor `proto/descriptor.binpb` is committed and deterministic;
  `cargo xtask proto` rewrites it, and only it.
- The Dart codegen runs only if `protoc` and `protoc-gen-dart` are present,
  in CI with a pinned plugin; the generated files are gitignored, drift
  breaks the build.

Extensions compared with the short version in `BACKLOG.md` 3.3
(`backlog/CONVENTIONS.md` 4.3 and 4.11):

- RPCs `GetConfig` and `SetConfig`; therefore `humanitl-ipc` may depend on
  `humanitl-config`.
- `FlowEvent` additionally has the variants `Failed { flow_id, error,
  resolved_ip }`, `Diagnostic`, `RulesChanged` and `AgentAsk`.
- `FlowEvent.Received` is `{ summary, domain }`; the domain information travels
  with the first event.
- `DecideRequest.block` is `Block { note }`: the user's note when blocking
  (ADR-0014).
- `DecideResponse.created_rule` carries the full rule; `created_rule_id`
  (field 2) becomes `reserved` at the next proto change.

The daemon remains a separate process, although Rust could be embedded into
the Flutter app via FFI.

## Rationale

Server streaming is exactly the form the event stream needs:
`Subscribe` returns `stream FlowEvent`, and HTTP/2 flow control provides
backpressure without us building it ourselves. A slow UI slows down the
stream instead of letting memory in the daemon fill up. When the broadcast
buffer overflows (capacity 1024), the daemon sends a `Lagged{n}` event,
and the UI resynchronises via `ListFlows(since)` — a visible, handled state
instead of silent data loss.

A Unix domain socket with `0600` excludes other users of the machine via file
permissions, without network configuration and without port collisions. The
session token is the second bolt in case the file permissions do not take
effect because of a misconfiguration.

The `.proto` is three things at once: executable documentation of the
capabilities, version contract between UI and daemon, and the later
plugin interface. A plugin is thus simply another gRPC client; it
needs no new mechanism.

The daemon remains a separate process for four reasons. First, the proxy must
outlive the UI: whoever closes the UI while the agent works must not leave the
agent running into the void. Second, a Flatpak cannot start `bwrap`, so the
later Flatpak delivery of the UI (ADR-0010) needs the daemon outside anyway.
Third, headless operation (ADR-0013) uses the same contract. Fourth, the
process boundary is a security boundary: the CA key does not live in the
address space of a UI toolkit.

## Rejected alternatives

- **REST/JSON over HTTP on loopback.** Would need a TCP port on the host that
  every local process can reach — exactly what ADR-0001 rules out for the
  proxy. Streaming would be SSE or WebSocket, i.e. a second mechanism next to
  the unary calls, and the schema contract would have to be maintained
  separately (OpenAPI) instead of coming from the same file.
- **Embed Rust into the Flutter app via FFI.** Saves a process, but loses all
  four properties named above: the proxy dies with the UI, Flatpak is out,
  headless would need a second path, and the CA key would live in the UI
  process.
- **D-Bus.** Obvious on the Linux desktop, but streaming large volumes of
  events is awkward, the typing is weaker, there is no usable binding for the
  later plugin use, and the coupling to the session bus makes tests more
  laborious.
- **Cap'n Proto or our own length-prefix protocol.** Faster, but without a
  mature Dart binding. The bottleneck is not serialisation.
- **Named pipes plus JSON lines.** Simple to build, but without schema,
  without backpressure, without versioning. Exactly the three things this
  interface needs in the long run.

## Consequences

- Every capability of the system must exist as an RPC first. This consequence
  is so load-bearing that it got its own ADR: ADR-0018 (parity).
- The Rust side needs no `protoc`; the toolchain grows by `protoc` and
  `protoc-gen-dart` only for Dart. Both run in CI with pinned versions,
  locally they are optional; `make check` must not depend on them.
- A version break of the proto is a visible event: `GetInfo` reports the
  major version, the UI shows a setup screen instead of stale data.
- The fake daemon (HUM-005) implements the same proto in Rust and replays a
  JSONL session. UI development is thereby fully decoupled from the proxy,
  without inventing a second contract.

## Affected issues

`HUM-003` (define proto v1), `HUM-018` (gRPC server skeleton),
`HUM-005` (fake daemon), `HUM-013` (proxy socket bind, delimitation from the
gRPC socket), `HUM-019` (Flutter shell with `GetInfo` version check),
`HUM-078` (parity table).
