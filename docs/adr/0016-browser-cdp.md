# ADR-0016 · Browser for the agent over CDP, watching and intervening in the UI

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Agents increasingly need a browser: reading documentation that loads content
with JavaScript, filling in forms, going through sign-in flows, operating a
web interface. An agent without a browser falls back on `curl` and fails at
every page that renders on the client side.

A browser in the sandbox raises three questions. First: does it run at all?
Chromium brings its own sandbox, which occupies user namespaces — the same
resource that bwrap already uses. Second: does the guarantee still hold? A
browser opens connections, speaks QUIC, has its own DNS cache and uses Unix
socketpairs for its internal communication. Third: what does the human see?
An agent clicking around a web interface unobserved is exactly the
loss-of-control scenario Humanitl is built against.

This decision is **post-MVP (M7)**. It is made now because two pieces of
groundwork arise in the MVP and would otherwise be dimensioned wrongly: the
bridge management in the shim and the configurability of the allowed seccomp
socket families per profile.

## Decision

**Sandbox profile `browser`.** It uses the host's Chromium via the read-only
mount of `/usr` and brings `nodriver` along in the Python package. Start
flags:

```
--proxy-server=http://127.0.0.1:3128
--disable-quic
--remote-debugging-port=9222
--headless=new
--no-sandbox
--ignore-certificate-errors-spki-list=<SPKI hash of the Humanitl CA>
```

A bundled `humanitl_browser.py` provides the ready-made nodriver
configuration. All page loads go through the proxy like any other request and
are subject to the same rules and the same recording.

**Watching.** Before the seccomp filter, the shim starts a second,
host-initiated bridge: a Unix socket in the sandbox that forwards to
`127.0.0.1:9222` (direction `out`). Over it the daemon speaks CDP
(`chromiumoxide`); `Page.startScreencast` delivers single frames, and a gRPC
stream `Browser` brings them into the UI.

**Intervening.** Mouse and keyboard input goes back as CDP input. The takeover
mode is visible in the UI (frame). Agent and human can be connected at the
same time, because Chromium allows several CDP clients.

**Security consequences, stated.** The shim supports a **list** of bridges
from the profile, each with direction `in` or `out`. The seccomp filter always
allows `AF_INET`/`AF_INET6` (only `lo` exists in the namespace); the profile
`browser` **additionally allows `AF_UNIX`** (and `SOCK_DGRAM`), because
Chromium creates its own Unix sockets for its process IPC; `socketpair()`
stays unrestricted in all profiles independently of this (CONVENTIONS 4.11).
In this profile the guarantee is carried by the network namespace plus the
mount allowlist; seccomp remains a second floor but is no longer the
narrowest point. Chromium runs without its own sandbox (`--no-sandbox`),
because bwrap occupies the user namespaces — bwrap **is** the outer sandbox.

**In the MVP only the groundwork:** the bridge list in the shim (HUM-012) and
the seccomp families per profile (HUM-010).

## Rationale

The host's Chromium instead of a bundled browser, because a shipped Chromium
inflates the package by more than a hundred megabytes and creates its own
obligation to ship security updates. The read-only mount of `/usr` is there
anyway.

`--disable-quic` is not optional. QUIC runs over UDP and would bypass the HTTP
proxy; in the empty network namespace it fails anyway, but a silent failure
with long timeouts is worse than a clear switch-off.

The SPKI hash of the Humanitl CA instead of `--ignore-certificate-errors`
altogether: the browser trusts exactly one key, not any certificate whatever.
The difference is the question of whether a MITM attack within the sandbox
would be possible.

CDP instead of an embedded WebView in the UI: the browser runs where it
belongs — in the sandbox — and the UI sees only single frames. This keeps
ADR-0009 ("no WebView on Linux") intact, and the rendering process of the page
under review never runs in the same process as the interface.

That agent and human can be connected at the same time is the actual product
idea behind it: watching is the normal case, intervening the exception, and
the transition needs no mode switch — only a visible frame.

The price is named instead of hidden: in the profile `browser` the seccomp
filter is weaker than in the default profile. That is why it is a **profile of
its own**. Whoever needs no browser gets the narrower setting; whoever needs
one sees the exception in the profile and in the isolation panel.

## Rejected alternatives

- **Browser on the host, controlled from the daemon.** Bypasses the sandbox
  completely: the browser would have the user's network, file system and
  cookies. The attack path "agent visits page, page exfiltrates via the host
  browser" would be open.
- **Playwright or Selenium instead of CDP directly.** Brings a large Node or
  Python dependency and its own browser management that downloads browsers —
  exactly what does not work in a sandbox without a network. `chromiumoxide`
  speaks CDP directly.
- **Firefox via the remote protocol.** Marionette and the Remote Agent protocol
  are less complete for screencast and input than CDP.
- **An embedded WebView in the UI.** Contradicts ADR-0009 and would render the
  visited page in the interface's process.
- **Screencast via video encoding instead of single frames.** Better frame
  rate, but an encoder dependency and more latency when intervening. JPEG
  single frames, throttled to ten frames per second, are enough for watching.
- **Allow `AF_UNIX` globally so as not to need a second profile.** Explicitly
  rejected: that would have weakened guarantee 3 for all users, including
  those without a browser. The exception stays in the profile, where it is
  visible.
- **Chromium with its own sandbox inside bwrap.** Technically not possible,
  because both claim the same user namespaces. `--no-sandbox` is not a
  slackening here but the statement of who the outer sandbox is.

## Consequences

- The shim (`humanitl-shim`) manages a **list** of bridges with direction, not
  a single one. This generalisation already arises in the MVP (HUM-012),
  although the second entry is only used in M7.
- The sandbox profile format has `[network].bridges` as a list and
  `[seccomp].allow_families` and `[seccomp].allow_types` as profile-dependent
  values (HUM-010).
- In the profile `browser` the isolation panel must show an additional,
  honest row: the seccomp filter is wider here. No green tick for something
  that does not apply.
- `chromiumoxide`, a `Browser` RPC and a browser tab in the UI are added in M7;
  none of these parts touches core or application (ADR-0015).
- Known pitfalls for M7, so that they do not have to be rediscovered: throttle
  screencast frames (at most 10 fps, JPEG quality 60); keyboard layouts in
  `dispatchKeyEvent` are error-prone; pin nodriver versions.
- The domain preview in the domain panel can later use the same browser
  (HUM-085) instead of a fetch path of its own — and thereby keeps the rule
  from ADR-0006 that nothing is fetched automatically.

## Affected issues

In the MVP: `HUM-012` (bridge list in the shim), `HUM-010` (seccomp families
per profile). In M7: `HUM-080` (sandbox profile `browser`), `HUM-081`
(reverse bridge and `cdp.sock`), `HUM-082` (CDP client in the daemon),
`HUM-083` (gRPC `Browser` bidi stream), `HUM-084` (browser tab in the UI),
`HUM-085` (screenshot preview for the domain panel).
