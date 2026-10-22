# ADR-0006 · DNS resolution only after allow

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

A proxy that accepts a request normally resolves the target host immediately
— if only because most HTTP client libraries handle resolution and connection
in one step. In Humanitl, however, a human sits between acceptance and
connection, and this gap is a channel.

A DNS query leaves the machine. It goes to a resolver that is not under our
control, and the attacker may well control the authoritative zone. A label may
carry 63 bytes, a name 253. An agent that gets
`aGVsbG8td29ybGQtc2VjcmV0.attacker.com` resolved has exfiltrated data —
even if the human blocks the request afterwards and no TCP connection ever
comes about. In this case the block would be theatre.

The second problem is DNS rebinding. If the proxy resolves at check time and
connects again later, the same name can point to a different address the
second time — for instance to the router, to `169.254.169.254` (cloud
metadata) or to the LLM server in the LAN. The human would then have allowed
`example.com` and the agent would be talking to the router.

## Decision

**Nothing is resolved before the request is allowed.** The hold queue and
the rule evaluation work exclusively on the host name as a normalised string.
Only after `allow` is it resolved, exactly once, via the port `Resolver`
(adapter: `hickory`) in the crate `humanitl-proxy` — never via the system
resolver of an HTTP connector, never via `GaiResolver`, never via the DNS of an
`HttpConnector`. The IP address obtained this way is passed to
`Egress::connect(authority, Some(ip))` and thereby pinned for this connection
(ADR-0017).

**Private destination addresses are refused after resolution.** If a name
resolves to an address in `10/8`, `172.16/12`, `192.168/16` (RFC 1918),
`127/8`, `169.254/16`, `100.64/10` (CGNAT), `fc00::/7` or `::1`, the
connection is refused: `FlowState::Failed { UpstreamError::PrivateAddress(ip) }`
and `502` to the client, plus the Diagnostic `PROXY_008` with a rule
suggestion. Exception: the matching rule carries `allow_private: true`. The
LLM passthrough rule sets this flag automatically, so that `localhost` or
`192.168.x.y` work as an LLM host.

The rule suggestion is `action: ask` with `allow_private: true`, tailored to
host, port, scheme, method, protocol upgrade and path prefix of the failed
request: the right opens a destination, it decides nothing, and therefore the
request is still put before a human every time. A suggested `allow` would be
more opening than the allow that has just failed. For this suggestion to
take effect, `allow_private` of a matching rule applies to every action and
not only to `allow` (HUM-102).

A suggestion must pass `parse_rules` and must then also match the request;
where a request permits neither, there is no suggestion, and the Diagnostic says
why. This concerns port `0`, which no rule can carry, and a method the rule
engine does not know, against which no rule matches at all. If the path yields
no prefix that matches the request, the field is dropped; the rule then
applies to every path of this host, and the Diagnostic spells out this breadth.
The details are in `backlog/CONVENTIONS.md` 4.10.

It remains open where the accepted rule ends up. A `FixAction::AddRule` carries
no position, and the path over the wire appends it to the end of the user
rules; if an older rule already matches the same host, that rule continues to
decide. As long as that is the case, the Diagnostic names the condition in its
`why` and in the note of the suggested rule, instead of offering a button that
silently does nothing. The remedy touches the wire form and belongs in an
issue of its own.

The address itself goes into the Diagnostic and into `resolved_ip` of the
`Failed` event, that is, to the interface. It does not stand in the body of
the `502` response or in any header: the sandbox has no resolver, so the
agent knows only the name, and the mapping of this name to a private address
would be new information about the local network for it.

**The domain preview in the UI never fetches anything from the network
automatically.** The card in the domain panel comes from a bundled catalogue
(`catalog/domains.yaml`). A live fetch (favicon, `og:title`) happens only on
an explicit click, on the host side, and only for the eTLD+1.

## Rationale

The host name is the only piece of information an attacker can control and
carry outside without an allow. Whoever resolves it before the decision has
left open an exfiltration channel with 63 bytes per label — quietly, without
any action by the user, and in a protocol that most people do not perceive as
a data channel. Moving resolution to the back costs almost nothing and closes
the channel completely.

The check for private addresses must take place *after* resolution, not
beforehand on the name, because that is exactly the rebinding trick: a
completely public-looking name points to `169.254.169.254`. And because the
checked IP is then pinned for the connection, no second resolution attempt can
come in between check and connection.

`allow_private` as a rule flag instead of a global setting keeps the exception
visible and local. The LLM server is the only case in which a private address
is normal, and it has its own narrowly cut rule anyway (host + port + path
prefix + method). A user who needs a further private address sees their
exception as one line in `rules.yaml`.

The automatic domain preview would be the same mistake in a different colour:
the UI would open a connection to exactly the domain the human is deciding
about, and thereby confirm to the attacker that someone is looking — with the
IP address of the host, not that of the sandbox.

## Rejected alternatives

- **Resolve and show the IP in the UI.** Would be informative for the user
  ("this domain points to a server in …"), but opens exactly the channel this
  ADR closes. The information gain does not outweigh that; the catalogue
  delivers the useful half of it offline.
- **Resolve via DNS over HTTPS to a fixed resolver.** Encrypts the channel,
  but does not close it: the attacker controls the authoritative zone and sees
  the query there, no matter how it reached them.
- **Resolve only the eTLD+1.** Shrinks the channel instead of closing it.
  A half-measure that is hard to explain.
- **Check private addresses on the name (`localhost`, `*.local`, IP literals).**
  Catches the naive case and misses the interesting one: a public name with a
  private answer. Remains sensible as an additional check, not as the only one.
- **Allow private addresses globally, with a warning.** Rejected: cloud
  metadata under `169.254.169.254` is a standard target of exfiltration
  payloads. Allowing by default with a warning banner is not fail-closed
  behaviour.
- **Leave DNS resolution to the upstream proxy.** That is exactly what happens
  in the later `Socks5h` mode (ADR-0017), where local resolution is even
  forbidden. For the MVP without an upstream proxy, local resolution after the
  allow is the simpler path.

## Consequences

- The hold queue and the rule engine work only on strings. Both thereby stay
  IO-free and testable in a table-driven way (ADR-0015).
- A resolution failure occurs only *after* allow and is therefore a
  state of its own: `FlowState::Failed{ UpstreamError::Dns }`, `502` to the
  client (ADR-0004). The user sees "you allowed it, but the name does not
  resolve" instead of a silent block.
- The test for this decision measures rather than claims: escape test 3
  checks that the host resolver shows **zero** lookups as long as the request
  stands in the queue (HUM-024, resolver statistics).
- `Egress::connect` receives the IP as a parameter. The point at which
  resolution and connection come apart is thereby visible in the type system.
- Homographs and Punycode are handled during normalisation, not during
  resolution: the displayed host and the checked host are the same A-label
  string (ADR-0007).
- The catalogue card must provide enough without the network for the user to
  be able to decide. Hence around 200 bundled dev services with icon,
  category and description, as well as a Tranco rank from a bundled list
  (HUM-031).

## Affected issues

`HUM-024` (DNS only after allow, with resolver statistics in the test),
`HUM-015` (proxy core, connection setup via `Egress`), `HUM-031` (domain panel
without automatic fetch), `HUM-039` (LLM passthrough sets `allow_private`),
`HUM-022` (rule flag `allow_private`), `HUM-102` (the Diagnostic `PROXY_008` with
its rule suggestion; `allow_private` has applied to every action since then).
