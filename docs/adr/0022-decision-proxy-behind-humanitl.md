# ADR-0022 · A decision proxy of the user's choice can stand behind Humanitl

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-16

## Context

Today Humanitl is the only place where a request of the agent is decided:
a rule allows or blocks it, and a request without a rule is held until a
person decides in the app or in the terminal (`hold.ask_mode`). Some users
already work with an intercepting proxy of their own (mitmproxy, Burp, a
company tool) and want to decide there, case by case, with the tooling they
know. Humanitl should then do what only it can do — isolate the agent and
filter a first layer with its rules — and hand everything it does not block to
that proxy (BACKLOG section 9, item 22).

Hard constraints:

- The three sandbox guarantees stay as they are: the agent has exactly one
  way out, the proxy of Humanitl; it never reaches the second proxy itself.
- Resolution only after the decision and the check for private addresses
  (ADR-0006) must not be lost through the second hop: the second proxy would
  otherwise resolve `localhost` or an address in the LAN for the agent.
- Every way out goes through the egress port (ADR-0017). That ADR already
  names `HttpProxy(Url)` as the second adapter; no new port is needed.
- A rule `block` in Humanitl stays final; the second proxy never sees such a
  request.

## Decision

We add the egress adapter `HttpProxy` from ADR-0017 and a fourth ask mode
`hold.ask_mode = "upstream"`, in which a request without a rule is not held
but goes to that proxy, which decides.

**Nothing of this is on by default.** Without `egress.proxy` Humanitl is the
only proxy, resolves only after its own decision and checks every target for
private addresses (ADR-0006), as before. The second proxy, and with it every
weakening below, exists only after an explicit opt-in by the user.

1. **Configuration** (all `x-project-scope = denied`; a project profile can
   neither set nor change them):
   - `egress.proxy`: `http://HOST:PORT` of the second proxy. Set, every
     allowed request leaves through it; empty (default), nothing changes.
   - `egress.proxy_ca`: absolute path of the PEM with the CA of that proxy.
     The daemon trusts it **only** on connections through `egress.proxy`, in
     addition to `tls.trust_system_store`; never on direct connections.
   - `egress.proxy_auth_ref`: optional reference into the keyring like
     `llm.api_key_ref`, for `Proxy-Authorization: Basic`.
   - `egress.proxy_resolve`: `"proxy"` (default once `egress.proxy` is set)
     or `"humanitl"`. Who resolves the target's name; see point 2.
   - `egress.proxy_response_timeout_secs`: how long the daemon waits for the
     response head through the proxy, default 300, because a person may be
     deciding there. It applies only to the response head (also after a
     streamed upload); `CONNECT` and the TLS handshake through the proxy keep
     `limits.connect_timeout_secs` and the existing handshake limit.

   `egress.proxy` covers the agent's traffic, everything that passes the
   handler (including the LLM alias and passthrough). Requests the daemon makes
   on its own (LLM discovery, the update check) stay direct; the doctor row
   says so.
2. **TLS: Humanitl opens, the second proxy opens again.** Humanitl speaks TLS
   with `SNI = host` through the tunnel and checks the certificate against
   the CA of the second proxy. Who resolves depends on
   `egress.proxy_resolve`:
   - `"proxy"` (default): Humanitl resolves nothing. It sends
     `CONNECT host:port`, and plain `http` as an absolute-form request with
     the name; the second proxy resolves after its own decision, so no DNS
     query leaves before the person decided there. Humanitl still refuses IP
     literals in private ranges and names that are local by definition
     (`localhost`, `*.localhost`, `*.local`, `*.internal`, the metadata
     names) before forwarding. A public name whose DNS answer points to a
     private address is the second proxy's to refuse.
   - `"humanitl"`: Humanitl resolves after its own decision, checks the
     address (ADR-0006) and sends `CONNECT IP:PORT` and the absolute form
     with the pinned address and the name in `Host`. The private check is
     whole only if the second proxy connects to the address in `CONNECT`;
     Burp Suite 2026.11.2 resolves the name from `Host` again (observed
     2026-11-18, HUM-524), so with Burp it does not bind. The query leaves
     before the second proxy decides.

   In both cases the
   egress port therefore tells the sender which request form to write (origin
   form direct, absolute form through the proxy), a change to the contract of
   `Egress` and `Upstream::send`, not only a new adapter. The CA of
   `egress.proxy_ca` lives in a `ClientConfig` of its own for this route; the
   shared one used for direct connections never sees it. The
   address of the proxy itself may be loopback or private; the check for
   private addresses applies to the target, never to `egress.proxy`.
3. **`hold.ask_mode = "upstream"`** is valid only with `egress.proxy` set
   (otherwise `CONFIG_*`, the start refuses). Rules keep working: `block`
   blocks in Humanitl, `allow` allows (and still leaves through the proxy),
   and a request without a rule or with `ask` is forwarded instead of held.
   `redact` keeps holding in Humanitl, because it asks a person to edit the
   request there. The last guard before sending covers the new decision:
   with `hold.hard_block_checksum_secrets`, a request carrying a
   checksum-confirmed secret is blocked by the system and never reaches the
   second proxy.
   The flow goes `Analyzed ──▶ Decided(Forward upstream) ──▶ Forwarded`, a
   new decision source next to rule, person and timeout; the state machine
   gets this transition, nothing else.
4. **Audit and recording**: such a flow is recorded with the source
   `upstream` and the status of the response from the second proxy. What
   that proxy decided, Humanitl knows only through its response; the
   recording says so instead of naming a decision it did not see.
5. **Visibility**: `humanitl doctor` and the isolation panel get the row
   "Egress: through `HOST:PORT`" (ADR-0017 asked for it), and the app shows
   in the mode line that requests without a rule are decided outside.

## Rationale

- Opening and re-encrypting keeps everything that makes Humanitl useful in
  front of the second proxy: path and body rules, secret detection,
  `hold.hard_block_checksum_secrets` and the recording see the request in
  plain text. The second proxy needs the user to trust nothing new from
  Humanitl; Humanitl trusts its CA, which the user configured on purpose.
- ADR-0006 has two halves, no DNS before the decision and no private
  target, and with a second proxy the two cannot both stay with Humanitl:
  the decision is made there, so the name is either resolved before it
  (Humanitl) or by the deciding proxy. The owner's use is Humanitl as
  isolation plus block filter in front of their own interception tool, so
  the default keeps the first half (`"proxy"`); `"humanitl"` keeps the
  second for users who value their network more than the query. mitmproxy
  documents `CONNECT` to an address followed by a certificate for the name
  from SNI (docs.mitmproxy.org, "How mitmproxy works"), so `"humanitl"`
  works with it. Burp Suite Community 2026.11.2, tried by the AI on
  2026-11-18 (docs/guides/decision-proxy-burp.md), takes a `CONNECT` to an
  address as well and issues its certificate for the name from SNI, but
  for the request in the tunnel it resolves the name from `Host` again and
  ignores the address, so with Burp `"humanitl"` keeps neither half and
  the guide recommends `"proxy"`; Burp also needs "Verify upstream TLS"
  and "Suppress Burp error messages in browser" switched on, which are off
  by default. The trial by a person is open (HUM-553).
- The connection to the second proxy starts on the host from the daemon,
  through the egress port; the sandbox has no route to it. The guarantees
  therefore stay untouched, and `tests/escape/` needs no change.
- A fourth ask mode instead of a new switch keeps one answer to the question
  "who decides a request without a rule".

## Rejected alternatives

- **Pass non-blocked connections through unopened (`CONNECT` tunnel).** The
  second proxy would see everything and Humanitl only the host: no path rules,
  no secret detection, no recording of content. Humanitl would shrink to a
  host filter, and secrets would leave unseen. Kept out; a per-rule `via:`
  that tunnels a single host can come later as its own decision.
- **Let the second proxy resolve the name without any check in Humanitl.**
  First rejected outright, because `localhost`, the LAN and the cloud
  metadata address would be reachable for the agent through the second proxy
  and DNS could answer differently for the two hops (ADR-0006). On the
  owner's objection it came back in a narrower form and is now the default
  `proxy_resolve = "proxy"`: Humanitl still refuses local names and private
  literals before the proxy, and the remaining limit (the second proxy may
  resolve a public name to a private address) is stated in Consequences and
  in `docs/SECURITY.md`. Only the unchecked form stays rejected.
- **Configure through `HTTPS_PROXY` in the environment of the daemon.** Same
  reasons as in ADR-0017: invisible, not controllable, a systemd detail.
- **Keep holding in Humanitl and also forward.** Two people would decide the
  same request in two places; the timeouts of both would race each other.
- **An API instead of a proxy (Humanitl asks a webhook "allow?").** A second
  protocol of our own that no existing tool speaks; the owner wants the tools
  they already have. Can come later next to this mode (see the LLM triage
  idea).

## Consequences

- One new egress adapter, four configuration keys, one ask mode and one
  decision source; the core changes only by the new transition.
- The second proxy sees decrypted traffic, including secrets that Humanitl
  did not block. `docs/SECURITY.md` and `docs/THREAT-MODEL.md` (en and de)
  say so in the same commit as the mode: the trust boundary for content moves
  to that proxy, the isolation does not.
- **With `egress.proxy_resolve = "proxy"` (default of the opt-in)** a public
  name that resolves to `127.0.0.1`, the LAN or the metadata address reaches
  that address through the second proxy if the person there allows it;
  mitmproxy has no such guard of its own. Humanitl's check covers only IP
  literals and local names. The security statement, the doctor row and the
  mode line say so in the same commit as the mode (HUM-512).
- **With `"humanitl"`** the DNS query happens before the second proxy
  decides and names the host; the private check is whole only for a second
  proxy that connects to the address in `CONNECT`, which Burp does not
  (HUM-524).
- In both cases a rule `block` stops a request before any resolution, and
  the sandbox guarantees are untouched: the agent reaches only Humanitl.
- A slow decision in the second proxy holds the agent's request up to
  `egress.proxy_response_timeout_secs`; then `502` with `UpstreamError::Timeout`.
- Only `http://` to the second proxy in this step; TLS to the proxy itself,
  SOCKS and a per-rule `via:` stay open (ADR-0017).

## Affected issues

`HUM-511` (egress adapter `HttpProxy`, configuration, doctor row),
`HUM-512` (`hold.ask_mode = "upstream"`, decision source, audit, security
statement and threat model), `HUM-513` (guide for mitmproxy, end-to-end test
with a stand-in proxy); see `backlog/sprint-7.md`.
