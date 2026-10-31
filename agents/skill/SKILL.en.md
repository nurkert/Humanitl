<!-- Die englische Fassung des Skills `humanitl` (HUM-364). Die Adapter legen
     ihn nur lesbar dorthin, wo der Agent seine Skills sucht: OpenCode unter
     <config>/opencode/skills/humanitl/SKILL.md, pi unter
     <agent-dir>/skills/humanitl/SKILL.md. Das Briefing (AGENTS.md) bleibt
     kurz und steht in jedem Kontext; dieser Text wird nur geladen, wenn die
     Beschreibung im Kopf passt, und darf deshalb ausführlicher sein. Jede
     Aussage ist am Verhalten geprüft, die Belege stehen in
     daemon/crates/sandbox/src/agent/skill.rs. Wie im Briefing erfährt der
     Agent nicht, dass ein Mensch entscheidet (Entscheidung des Owners vom
     2026-10-31): lange Wartezeit ist eine Eigenschaft dieser Umgebung, kein
     Wort über eine Person, eine Freigabe oder ein Prüfen durch jemanden. Der
     Kopf mit name und description muss nach dem Entfernen der Kommentare als
     Erstes stehen, sonst lesen ihn weder OpenCode noch pi. Im Text darf keine
     geschweifte Klammer stehen außer in den Platzhaltern, und im Text eines
     Kommentars kein Kommentar-Ende. Die Blöcke am Ende gehören zu den
     Ask-Modi und treten an die Stelle des Platzhalters ask_mode. -->
---
name: humanitl
description: "How network access works in this Humanitl sandbox. Load it whenever a network request is slow, hangs, fails, is refused or blocked (403, 502, 503, 504, Blocked by Humanitl.), and before network-heavy work such as downloads, package installs, API calls or fetching web pages."
---

# Humanitl: network access in this sandbox

Every HTTP(S) request leaves this sandbox through the Humanitl proxy in the environment (`HTTPS_PROXY`, `HTTP_PROXY`); there is no other way out. The proxy applies the rules of this environment to each request:

- Permitted: it goes out at once.
- Not permitted: `403` at once.
- Neither: see the next section.

Calls to your model at {llm_host} are permitted by one bundled rule, on the paths it is configured for, and never wait here; when the model is slow, the model server is slow, not Humanitl.

{ask_mode}

## Reading an answer from Humanitl

A body that starts with `Blocked by Humanitl.` comes from the proxy, not from the server. Its lines are `reason:`, `flow:`, `host:` and sometimes `note:`, which explains the answer.

- `403`: the destination or the request is not permitted in this environment, or the request carried a secret or went to a private address. Do not send it again.
- `504` with `reason: timeout`: no permission was granted in time; see the section above.
- `502` with `reason: upstream_dns`, `upstream_connect`, `upstream_tls` or `upstream_timeout`: Humanitl let the request through and the target failed. This is a real error of that host.
- `502` with `reason: upstream_private_address:` and an address: the name resolves to a private address, which is not permitted here. Do not send it again.
- `503` with `reason: hold_max_flows` or `hold_memory`: too many requests or too much data are waiting at the same time. Wait for the open ones and send fewer at once.
- Any other status or reason in such a body (for example `408`, `413` or `502` with `reason: no_route`): do not send the request again unchanged; read the `note:` line if there is one.

## Asking Humanitl

The proxy answers `http://humanitl.internal/` itself:

- `curl -s http://humanitl.internal/` shows this session (`timeout=`, `llm=`) and what is permitted, as rules; the first match wins.
- `curl -s http://humanitl.internal/why/FLOW`, with the `flow:` value of an answer for FLOW, gives `decision=`, `reason=` and `note=` of that request, if the request is known.
- `curl -s --data 'one line: which destination you need and why' http://humanitl.internal/ask` requests access to a destination and answers `202` at once; access may follow later. It grants nothing by itself. At most ten per minute; more get `429` with a `retry-after` header.

## When something is refused

Tell the user in one or two sentences which host was not reachable, the `note:` if there is one, and what else could do the job: a host that is already permitted, a file the user can provide, or the access that would be needed. A request through another tool, host or mirror is a new request, and the same rules apply to it.

<!-- ask_mode: ui -->
## High latency is normal here

Network requests here have high and variable latency. A request that no rule permits or refuses yet can wait up to {timeout}s; if no permission was granted by then, it ends with `504`. A permitted request takes as long as the server needs; a refused one ends at once with `403`. While a request waits, no response arrives. That is normal for this environment, not an outage.

- Keep waiting. Use generous timeouts above {timeout}s: `curl --max-time {max_time}`, and the same for the timeout of the tool call that runs it. `curl` without `--max-time` waits on its own.
- A tool that gives up sooner, such as a fetch tool with a fixed short limit, closes the connection and ends the request; a new request starts the wait again. For a request that may take long, use `curl`.
- Send each request once. Do not retry in a loop, do not run the same request twice in parallel, and do not switch to another tool or mirror to get around the latency: each of these is one more request that waits as long.
- Do not conclude that the network is down because a request takes long. If the task still needs a destination that ended in `504`, request access once with `POST /ask` and send the request once more later.
<!-- ask_mode: none -->
## Requests do not wait here

Only what the rules of this environment permit goes through, at once; nothing waits: a refused request gets `403` at once, any other `504` (`reason: timeout`). A `504` means not permitted, not an outage and not a slow network; sending the same request again at once gives the same answer. If the task still needs a destination that ended in `504`, request access once with `POST /ask` and send the request once more later. Tell the user which host and path were not reachable. A slow answer to a permitted request comes from the server, not from Humanitl.
