# ADR-0001 · Daemon in Rust on hudsucker, not mitmproxy

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl places an intercepting HTTPS proxy between a sandboxed LLM agent and
the internet. This proxy holds every request until a human has decided. In
doing so it holds the most sensitive things of the installation in its hands:
the private key of the MITM CA, the complete recordings of all requests and
responses including bodies, and the control of the sandbox in which the agent
runs. For this component, security is not one quality attribute among others;
it is the product itself.

There were two realistic paths. First: take a finished, mature MITM proxy
(mitmproxy, Python) and write the domain logic as an addon. Second: build our
own daemon in a memory-safe systems language and use a library for the
protocol work. The difference between the paths is not the amount of domain
logic — that is the same in both cases — but the size of the Trusted Computing
Base and the question of whether the result can be shipped as one package.

The target platform is the Linux desktop, shipped as `.deb` and AppImage, run
as a systemd user service. The audience is professionals without a security
background who want to install a package and configure nothing afterwards.

## Decision

The daemon `humanitld` is a Rust binary. The MITM layer is
[hudsucker](https://github.com/omjadas/hudsucker) 0.25 with the features
`rcgen-ca`, `rustls-client`, `http2`. Everything above it (hold queue, rule
engine, findings, recorder, audit chain, gRPC server, sandbox control) is our
own code in the crates under `daemon/crates/`.

Binding protocol target per milestone, so that no sprint goal carries the
status "experimental":

| Milestone | Client side | Upstream | Other |
|---|---|---|---|
| M1 | HTTP/1.1 (ALPN offers the client only `http/1.1`) | HTTP/1.1 enforced | CONNECT, WebSocket passthrough, SSE |
| M1 | gRPC | — | documented failure in the conformance matrix (`PROXY_007 h2 not available`) |
| M6 | HTTP/2 | HTTP/2 | lifts gRPC out of `[experimental]` |

The listener is exactly one Unix socket, no loopback TCP port on the host.
In its standard form hudsucker listens on TCP; HUM-015 step 0 settles in a
spike whether hudsucker 0.25 accepts a generic `Accept` stream. If yes, the
proxy listens directly on the Unix socket. If no, hudsucker's accept loop
(about 100 lines) is kept as a fork in the repository. In both cases the door
into the sandbox stays what it is meant to be: a single socket file that no
other process on the host can reach.

## Rationale

A statically linked, memory-safe binary is a smaller Trusted Computing Base,
and easier to argue about, than a Python bundle with around forty transitive
packages. For a tool whose entire claim is "nothing gets out here", the size
of the attack surface is the central argument. The security argument must be
explainable in three sentences and verifiable with a click (principle 2 in
`BACKLOG.md` 1.3); every line of third-party code in the trusted area makes
those three sentences longer.

hudsucker provides exactly the protocol work nobody wants to write
themselves: CONNECT handling, certificate cache via `rcgen`, HTTP/1 and HTTP/2
on hyper 1, WebSocket handler, streaming bodies with trailers. The shape of the
hook is decisive: `handle_request` is `async`. An `await` on a `oneshot`
channel is therefore the natural holding point for the hold queue, and a held
request costs no thread.

The actual work — hold queue, rules, findings, recorder, audit, gRPC,
sandbox — would have to be written anew in any language. mitmproxy would have
taken none of it off our hands. It would have taken protocol edge cases off
our hands, but bought a packaging problem in exchange.

The price of the decision is the MITM edge cases, which are now ours.
Mitigation: a conformance matrix from sprint 1 (HUM-017) that runs `curl`,
`websocat` and `grpcurl` against an in-process axum fake upstream —
chunked, SSE, CONNECT, large bodies, WebSocket upgrade, trailers. mitmproxy's
test corpus and rule semantics remain usable as a reference, even without
mitmproxy in the product.

## Rejected alternatives

- **mitmproxy as addon host (Python).** Packaging is the killer: Python runtime
  plus C extensions come to 80 to 150 MB per artefact, and a `.deb` that brings
  its own Python environment is a maintenance burden in the long run. Added to
  that are the size of the TCB and the missing memory safety of the C
  extensions. The advantage (mature protocol implementation) does not outweigh
  this, because the domain logic is written anew anyway.
- **Go with goproxy.** goproxy is outdated, and HTTP/2 MITM in `net/http` is
  tedious to incomplete. A second ecosystem in the repository (next to Dart for
  the UI) without a clear gain.
- **Our own MITM layer directly on hyper, without hudsucker.** Would be
  possible, but certificate cache, CONNECT tunnel and WebSocket upgrade are
  exactly the places where subtle bugs arise. hudsucker is thin enough to fork
  if necessary (see the listener question), so the dependency is reversible.
- **Node.js.** Memory-safe in the sense of "no buffer overflows", but the npm
  dependency tree and the runtime size reproduce the Python problem.
- **No proxy of our own, firewall rules only.** Does not solve the problem: a
  firewall sees host names, but no request bodies. The body is exactly where
  exfiltration happens (see ADR-0005).

## Consequences

- Rust becomes the language of the entire daemon, including shim and CLI. A
  contributor needs Rust knowledge; in return the compiler enforces the module
  boundaries from ADR-0015.
- MITM conformance is our responsibility. It is not claimed but measured in
  HUM-017; the matrix is part of the documentation and also lists the
  deliberate gaps (gRPC until M6).
- The binding to hudsucker 0.25 is a version binding with a risk of breakage.
  It is confined to the crate `humanitl-proxy` (ADR-0015: no abstraction over
  third-party libraries, but a third-party library only in one crate), so that
  a switch stays local.
- The listener question is the only open point of this decision. Its outcome
  does not change the decision, only the amount of forked code.
- `experimental.h2_upstream` remains the only switch behind which h2 sits
  until M6. Unused `experimental` flags are dropped after two sprints
  (ADR-0015).

## Affected issues

`HUM-015` (MITM proxy core, step 0 listener spike), `HUM-013`
(proxy socket bind), `HUM-017` (conformance matrix), `HUM-021` (demo script
M1), `HUM-056` (fuzzing of the parsers and decoders), `HUM-061` (buffer for
MITM edge cases).
