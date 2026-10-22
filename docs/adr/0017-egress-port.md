# ADR-0017 · One egress port for direct connection, upstream proxy and Tor

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Once the human has approved a request, the proxy has to take it outside. In
the simplest case that is a TCP connection to the resolved IP address. There
are, however, two foreseeable variants: in many company networks the only way
out leads through an HTTP proxy with `CONNECT`, and some users want to route
agent traffic over Tor.

The real danger lies elsewhere. In an HTTP client, setting up a connection is
an inconspicuous line, and it can arise in many places: in the upstream
client, in a health check, in a retry, in a library that connects internally
by itself. Each of these places possibly bypasses the rules from ADR-0006
(resolution only after approval, IP pinning, check for private addresses). A
single forgotten `TcpStream::connect` undermines a core promise without any
test tripping over it.

## Decision

**Every upstream connection goes through exactly one port.**

```rust
pub trait Egress: Send + Sync {
    async fn connect(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic>;
}

pub struct Direct;   // MVP
```

In the MVP exactly one implementation exists: `Direct`. It connects to the IP
that was determined via the `Resolver` port after the approval, and pins it
(ADR-0006).

Later, without a change to the core:

- `HttpProxy(Url)` — `CONNECT` to a company proxy.
- `Socks5h(Url)` — Tor at `socks5h://127.0.0.1:9050`.

It is configured globally via `egress.via` or per rule via `via:`.

**With `Socks5h` the hostname is passed unresolved to the SOCKS proxy, and
local DNS resolution is forbidden in this mode.** Otherwise there would be a
DNS leak that defeats the whole purpose of using Tor. In this case the
isolation check gets a fourth, optional row "Egress: Tor" with a check
against `check.torproject.org`.

**Machine-enforced:** `tools/check-deps.sh` fails as soon as
`TcpStream::connect` appears in production code outside
`daemon/crates/proxy/src/egress/` (ADR-0015).

In the MVP this ADR is **only** the port definition plus the enforcement. The
adapters `HttpProxy` and `Socks5h` are post-MVP.

## Rationale

The value of this port lies not in extensibility but in control. A single
place where an outbound connection is made is a single place where the check
for private addresses takes place, where the IP is pinned, where a timeout
applies and where an error becomes `FlowState::Failed` with an
`UpstreamError`. If there were five places, each of them would have to get the
same five things right — and the sixth, added later, would not.

That is why the grep in CI is part of the decision and not an accompanying
measure. It is coarse, knows no aliases and no library that connects
internally, but it reliably catches the most common case: someone quickly
writes a direct connection setup because it happens to be convenient at that
spot.

That `Egress::connect` takes both the `Authority` and, optionally, the
resolved IP makes the split visible in the type system. `Direct` gets the IP
and connects there. `Socks5h` does **not** get it and may not determine it
either — the caller passes `None`, and the adapter hands the name on to the
SOCKS proxy. The DNS leak thereby becomes a case the signature rules out,
instead of a rule one has to remember.

The return value `Result<_, Diagnostic>` instead of an `io::Error` ensures
that a failed connection setup reaches the user as "why" and "what now"
(ADR-0012): no route to the company proxy, Tor is not running, the target is a
private address.

`via:` per rule instead of only globally, because the realistic case is
mixed: the LLM server in the LAN is reached directly, the rest via the company
proxy. A purely global setting would not cover this case.

## Rejected alternatives

- **No port, direct connections in the proxy code.** The least code and the
  cause of exactly the scattering this ADR prevents. Not tenable even with
  "we will be careful".
- **Configuration via the daemon's environment variables
  `HTTP_PROXY`/`ALL_PROXY`.** Would have made the way out depend on an
  environment that is a systemd unit detail — invisible in the UI, not
  controllable per rule and, in a security component, an unpleasant action at
  a distance.
- **Build `HttpProxy` and `Socks5h` already in the MVP.** Violates "no trait
  without a second user in sight" in its practical reading: the port comes
  into being now (it has a concrete second adapter in prospect), the adapters
  come into being when someone needs them.
- **Tor as an embedded `arti` client instead of via a local SOCKS proxy.** A
  considerably larger dependency and an update obligation of its own for a
  nice-to-have feature. The local Tor service is the established route.
- **Local DNS resolution in SOCKS mode too.** Would be simpler and makes using
  Tor worthless, because the DNS traffic gives away the targets.
- **Checking the egress rule with a Clippy lint instead of a grep.** Would be
  cleaner, but requires a lint tool of its own. The grep is available today
  and costs nothing.

## Consequences

- In the MVP there is exactly one adapter, but the port exists and is
  enforced. The later Tor or company-proxy mode touches neither core nor
  application (ADR-0015).
- Timeouts for the connection setup belong to the port and stand in the group
  `limits` (`limits.connect_timeout_secs`, alias
  `upstream.connect_timeout_secs`).
- A failed connection setup becomes
  `FlowState::Failed { UpstreamError::Connect | Tls | PrivateAddress(_) |
  Timeout }` and thus `502` at the client (ADR-0004, ADR-0005).
- The grep in `tools/check-deps.sh` excludes test code; tests may connect
  directly, production code may not.
- When `HttpProxy` or `Socks5h` are added, the isolation check needs an
  additional row so that the user sees which route their traffic takes.
  Without this display the egress mode would be an invisible setting with a
  large effect.
- The port has an `async` signature and therefore lives in the adapter layer,
  not in the IO-free core.

## Affected issues

`HUM-015` (MITM proxy core: all upstream connections via `Egress`),
`HUM-024` (DNS only after allow, IP pinned at `Egress::connect`), `HUM-074`
(grep check for `TcpStream::connect` in `tools/check-deps.sh`), `HUM-057`
(timeouts in the group `limits`).
