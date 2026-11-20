# mitmproxy behind Humanitl

Applies to: Humanitl 0.1

This guide puts [mitmproxy](https://mitmproxy.org/) behind Humanitl, so that you
decide the agent's requests in mitmproxy instead of in the Humanitl app. Humanitl
keeps doing what only it can do: it isolates the agent, applies your rules and
blocks what a `block` rule matches. Every request no rule decides goes on to
mitmproxy, and you resume or refuse it there. The design is
[ADR-0022](../adr/0022-decision-proxy-behind-humanitl.md); what it changes about
the security statement is in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512).

Nothing of this is on by default. Without `egress.proxy` Humanitl is the only proxy
and every request without a rule waits for you in Humanitl.

The mitmproxy steps were tried with mitmproxy 12.3.1 on Linux; where this guide
says "observed", that is what that version did. Every other statement about
mitmproxy links to its documentation.

## How the pieces fit

```text
agent ──▶ Humanitl ──▶ mitmproxy ──▶ target
     TLS with the       TLS with the       TLS checked by
     Humanitl CA        mitmproxy CA       mitmproxy
```

There are two CAs, and each side trusts exactly one of them:

| Connection | Who presents a certificate | Signed by | Who trusts it |
|---|---|---|---|
| Agent to Humanitl | Humanitl, for the name the agent asked for | the Humanitl CA | the agent, inside the sandbox |
| Humanitl to mitmproxy | mitmproxy, for the same name | the mitmproxy CA | Humanitl, through `egress.proxy_ca`, only on this route |
| mitmproxy to the target | the real server | a public CA | mitmproxy, which verifies it unless its option `ssl_insecure` is set |

Humanitl opens the agent's TLS, applies its rules and secret detection to the plain
request, then opens a new TLS connection through mitmproxy, with the host name in
SNI. mitmproxy opens that one again, shows you the request and, if you let it go,
sends it to the target. The agent never trusts the mitmproxy CA and never reaches
mitmproxy itself; it still has exactly one way out, Humanitl. Humanitl trusts the
mitmproxy CA only on connections through `egress.proxy`, never on a direct one.

## 1. Install and start mitmproxy

Install mitmproxy as the [installation guide](https://docs.mitmproxy.org/stable/overview/installation/)
describes; on Linux the standalone binaries from mitmproxy.org or
`uv tool install mitmproxy`. Both bring three tools: `mitmproxy` (console),
`mitmweb` (browser interface) and `mitmdump` (no interface, so it cannot hold a
request for you).

Start it as your own user, in the regular proxy mode, the default, which
[listens on port 8080](https://docs.mitmproxy.org/stable/concepts/modes/):

```sh
mitmproxy --listen-host 127.0.0.1 --listen-port 8080 \
  --set connection_strategy=lazy --set upstream_cert=false
```

`--listen-host` and `--listen-port` set the [options](https://docs.mitmproxy.org/stable/concepts/options/)
`listen_host` and `listen_port`. Observed: without `--listen-host` mitmproxy
listens on every interface (`*:8080`), so anyone in your network could use it;
`127.0.0.1` keeps it on this machine, where Humanitl reaches it. The same works
with `mitmweb`, whose browser interface is on port 8081 by default (option
`web_port`).

The two `--set` options keep mitmproxy from contacting the target before you
decide. By default mitmproxy does: its [option](https://docs.mitmproxy.org/stable/concepts/options/)
`connection_strategy` is `eager`, which establishes the server connection as early
as possible, and `upstream_cert` is on, which connects to the target to look up its
certificate details while the TLS handshake with Humanitl is still running
([certificate sniffing](https://docs.mitmproxy.org/stable/concepts/how-mitmproxy-works/)).
Both happen before the request inside the tunnel exists, so before an intercept
filter can hold it: the name is resolved and the target is contacted even for a
request you then kill. `connection_strategy=lazy` defers the server connection
"as long as possible".

Observed with an intercept filter `~q` and a local TLS target that counted its
accepted connections, for a `CONNECT` to the name and one to the address:

| mitmproxy options | Connections at the target before the decision | After resuming |
|---|---|---|
| defaults | 1 | 1 |
| `upstream_cert=false` only | 1 | 1 |
| `connection_strategy=lazy` only | 0 | 1 |
| both | 0 | 1 |

`connection_strategy=lazy` alone was enough in 12.3.1; this guide sets
`upstream_cert=false` as well, because that option's whole purpose is a connection
to the target. What it costs: mitmproxy builds its certificate for Humanitl from
the name in SNI alone instead of copying the names of the target's certificate.
Humanitl always sends the host name in SNI and checks the certificate against
exactly that name, so the connection still verifies; what is lost is only the
mirroring of further names and, with `lazy`, of the target's TLS ALPN choice (both
in the option texts linked above). mitmproxy still verifies the target's own
certificate when it connects after your decision.

## 2. Find the mitmproxy CA

On its [first start](https://docs.mitmproxy.org/stable/concepts/certificates/)
mitmproxy creates its CA in its configuration directory, `~/.mitmproxy` by default
(option `confdir`). The file Humanitl needs is the certificate in PEM format:

```sh
ls ~/.mitmproxy/mitmproxy-ca-cert.pem
```

Never give Humanitl `mitmproxy-ca.pem`: that file holds the private key as well.
Humanitl reads only certificates from the file and needs nothing else.

## 3. Point Humanitl at mitmproxy

On the command line, each key with `humanitl config set`. `egress.proxy_ca` must be
an absolute path, so let the shell expand `$HOME`:

```sh
humanitl config set egress.proxy http://127.0.0.1:8080
humanitl config set egress.proxy_ca "$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
humanitl config set egress.proxy_resolve proxy     # the default; see "Who resolves names"
```

Or in `config.toml`:

```toml
[egress]
proxy = "http://127.0.0.1:8080"
proxy_ca = "/home/you/.mitmproxy/mitmproxy-ca-cert.pem"
proxy_resolve = "proxy"
proxy_response_timeout_secs = 300
```

In the app: Settings (`Ctrl+,`), section "Decision proxy". "Address" takes
`http://127.0.0.1:8080`; "CA certificate" has "Choose file" for
`~/.mitmproxy/mitmproxy-ca-cert.pem`; "Who resolves names" is "Proxy" or
"Humanitl". "Credentials" is only for a proxy that asks for a login (mitmproxy
does that only with its option `proxyauth`).

Every key is described in [CONFIG.md](../CONFIG.md#egress). The keys are
`x-project-scope = denied`: a project profile in a cloned repository can neither
set nor change them.

## 4. Let mitmproxy decide requests without a rule

Up to here every request a rule or you allowed leaves through mitmproxy, but
requests without a rule still wait in Humanitl. To hand them to mitmproxy, set the
ask mode `upstream`:

```sh
humanitl config set hold.ask_mode upstream
```

In the app: Settings, section "Requests", "Ask", the fourth choice "Proxy behind".
It can be chosen only once an address stands under "Decision proxy". Humanitl
refuses `upstream` without `egress.proxy` (`CONFIG_003`).

What the rules still do in this mode:

| Rule | What happens |
|---|---|
| `block` | Humanitl answers `403`; mitmproxy never sees the request. A blocked name is not even resolved. |
| `allow` | Leaves through mitmproxy at once, without holding. |
| `ask` or no rule | Forwarded to mitmproxy without holding in Humanitl. You decide there. |
| `redact` | Still held in Humanitl, because only there can you edit the request. |

With `hold.hard_block_checksum_secrets`, a request carrying a checksum-confirmed
secret is blocked by the system (`403`); mitmproxy sees no byte of it.

## 5. Restart the daemon and check

The `egress.*` keys take effect when the daemon starts. Restart it while no session
is running:

```sh
humanitl daemon restart
humanitl egress status
humanitl egress check
```

`humanitl egress status` shows what `config.toml` says and the way out the running
daemon takes. `humanitl egress check` opens a tunnel through mitmproxy to
`example.com:443` and the TLS handshake in it; it sends no request, so nothing
waits for you in mitmproxy. A failure names its cause with a fix, for example an
unreachable proxy or a certificate that does not match `egress.proxy_ca`. The app
has the same check under "Decision proxy", "Check connection".

The mode line of every session then says who decides: `humanitl sessions` and
`humanitl sandbox plan` print `without a rule: decided by 127.0.0.1:8080`, and the
isolation panel of the app reads "Without a rule: decided by 127.0.0.1:8080" with
who resolves names and what follows from that.

## 6. Intercept and decide in mitmproxy

mitmproxy forwards everything until you set an intercept filter. In the console
press `i` and enter a [filter expression](https://docs.mitmproxy.org/stable/concepts/filters/)
between the quotes of `set intercept ''`, as the
[intercept tutorial](https://docs.mitmproxy.org/stable/mitmproxytutorial-interceptrequests/)
shows. Or set it at the start (option `intercept`):

```sh
mitmproxy --listen-host 127.0.0.1 --listen-port 8080 \
  --set connection_strategy=lazy --set upstream_cert=false \
  --set intercept='~q'
```

`~q` matches every request that has no response yet, so every request Humanitl
forwards waits for you. `~q & ~d example\.com` holds only requests to that domain
(`~d` is the domain, `&` is and). Observed: `~d` also matches the name when
Humanitl connects to the address (`egress.proxy_resolve = "humanitl"`), because the
request inside the tunnel carries the name in `Host`.

mitmproxy does not know which requests a Humanitl rule allowed: its filter applies
to everything that reaches it, the LLM passthrough and `allow` rules included. If
the model server's traffic should not wait for you, leave it out of the filter, for
example `~q & !~d api\.example-llm\.com` (`!` is not).

On a held request, as the tutorial describes:

- `a` resumes it unchanged; mitmproxy sends it to the target, and the agent gets
  the target's answer.
- `X` kills it, so it is not forwarded.

`mitmweb` takes the same option `intercept` and resumes or kills a held request
from the browser.

To refuse with a clear answer instead of killing, use the option
[`block_list`](https://docs.mitmproxy.org/stable/overview/features/) with a filter
and a status code. mitmproxy then answers itself, and the target never sees the
request:

```sh
mitmproxy --listen-host 127.0.0.1 --listen-port 8080 \
  --set connection_strategy=lazy --set upstream_cert=false \
  --set intercept='~q' \
  --set 'block_list=:~d tracker\.example$:403'
```

The status code `444` makes mitmproxy hang up without an answer.

## 7. What the agent sees

| In mitmproxy | The agent gets |
|---|---|
| resumed | the target's answer, unchanged |
| refused with `block_list` and `403` | mitmproxy's `403`, unchanged, at once |
| `block_list` with `444` | `502`, at once, closed behind Humanitl (`upstream_proxy_closed`; derived from the code since HUM-555, not tried again with mitmproxy) |
| killed while held (`X`) | `502` from Humanitl, after `egress.proxy_response_timeout_secs` (not tried again since HUM-555) |
| nobody decides | `502` from Humanitl, after `egress.proxy_response_timeout_secs` |

Observed: a request killed while it was held was not forwarded, but its connection
stayed open, so Humanitl waits until the deadline. In mitmproxy 12.3.1
[`Flow.kill()`](https://github.com/mitmproxy/mitmproxy/blob/v12.3.1/mitmproxy/flow.py)
marks a held flow as no longer intercepted without resuming it, and a comment there
calls the way killing is signalled "not ideal"
([mitmproxy#4711](https://github.com/mitmproxy/mitmproxy/issues/4711)). If the agent
should learn of a refusal at once, refuse with `block_list`. Should a later
mitmproxy close the connection on a kill, the agent would get `502` with
`upstream_proxy_closed` at once instead.

`egress.proxy_response_timeout_secs` (default 300 seconds) is how long Humanitl
waits for the head of mitmproxy's answer, because a person may be deciding there.
The briefing of the agent tells it the same: wait, then `502`; a `403` is final.

Humanitl records such a request with the decision source `upstream` (in the audit
`decision: forward`, `decided_by: upstream`) and the status of mitmproxy's answer.
It does not claim an allow or a block it did not see; the history in the app reads
"Forwarded, proxy answered 403". A request whose connection mitmproxy closes without
an answer (`444`) has no status; history and recording keep it as failed with
`upstream_proxy_closed`, and the HMAC audit log ends it with `flow.failed` and the
same reason (HUM-565).

## Who resolves names

`egress.proxy_resolve` decides who turns the target's name into an address.

| | `proxy` (default) | `humanitl` |
|---|---|---|
| What Humanitl sends | `CONNECT example.com:443` | `CONNECT 93.184.215.14:443`, the name in SNI |
| DNS query from Humanitl | none; mitmproxy resolves after your decision only with the options from step 1 | one, after Humanitl's own rules, before your decision in mitmproxy |
| Private targets | Humanitl refuses private IP literals and local names (`localhost`, `*.local`, `*.internal`, names without a dot); a public name that points into your network is mitmproxy's to refuse | Humanitl checks the resolved address too, as without mitmproxy |
| What mitmproxy shows as host | the name | the address (observed); the name is in `Host` |

mitmproxy handles both forms: a `CONNECT` to an address is
[legitimate](https://docs.mitmproxy.org/stable/concepts/how-mitmproxy-works/)
for it, and it takes the name from SNI to connect to the target. Observed: with a
`CONNECT` to `127.0.0.1` and SNI `localhost`, mitmproxy checked the target's
certificate against `localhost`.

Choose `proxy` if no DNS query should leave before you decided. That holds only
with mitmproxy started as in step 1, with `connection_strategy=lazy` and
`upstream_cert=false`: with its defaults mitmproxy resolves the name and contacts
the target during the TLS handshake, before the decision. With `humanitl`
Humanitl has resolved the name already, but with its defaults mitmproxy still
contacts the target before the decision. mitmproxy has no
guard against private targets of its own (its [option](https://docs.mitmproxy.org/stable/concepts/options/)
`block_private` is about clients connecting to it, not about targets), so a public name that resolves to
`127.0.0.1` or your router reaches it if you resume the request. Choose `humanitl`
if your network matters more to you than the query: then nothing private is
reachable, and the name leaves as a DNS query before you see the request.

## Limits

These are in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512) and the
[threat model](../THREAT-MODEL.md); in short:

- mitmproxy sees the decrypted traffic, including secrets Humanitl did not block
  and the LLM passthrough. Whoever can read mitmproxy's flows can read them.
- The isolation does not move: the agent reaches only Humanitl, the connection to
  mitmproxy starts in the daemon.
- The LLM passthrough declared for a session is an `allow` that ranks ahead of your
  own block rules; it leaves through mitmproxy, recorded as `passthrough`.
- Requests the daemon makes on its own (search for model servers, update check)
  stay direct.
- Humanitl speaks only `http://` to mitmproxy; TLS to the proxy itself and SOCKS
  are not supported.
- If mitmproxy is not running, requests fail with a finding; they never leave
  directly instead.
- Pausing intercept in Humanitl lets a `redact` request go to mitmproxy as well, as
  it lets everything waiting go out.

## Going back

Remove the keys and restart the daemon; Humanitl is the only proxy again:

```sh
humanitl config set hold.ask_mode ui
humanitl config set egress.proxy_ca -
humanitl config set egress.proxy -
humanitl daemon restart
```

`-` (or `null`) removes a key from `config.toml`.
