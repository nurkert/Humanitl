# Burp Suite behind Humanitl

Applies to: Humanitl 0.1

> **Not yet tried with a current build.** This guide is written from the
> PortSwigger documentation alone. Nobody has run these steps with a current Burp
> Suite yet, so Burp is not named as supported
> ([ADR-0022](../adr/0022-decision-proxy-behind-humanitl.md)). Every statement
> about Burp links to the PortSwigger documentation or is marked "to be confirmed
> in the trial"; the open points are collected under
> [Open until the trial](#open-until-the-trial). The guide gets the date, the Burp
> version and what was observed once the trial has run.

This guide puts [Burp Suite](https://portswigger.net/burp/documentation/desktop/tools/proxy)
behind Humanitl, so that you decide the agent's requests in Burp instead of in the
Humanitl app. Humanitl keeps doing what only it can do: it isolates the agent,
applies your rules and blocks what a `block` rule matches. Every request no rule
decides goes on to Burp, and you forward or drop it there. The design is
[ADR-0022](../adr/0022-decision-proxy-behind-humanitl.md); what it changes about
the security statement is in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512).

The Humanitl side is the same as with mitmproxy and is described in more detail in
[mitmproxy behind Humanitl](decision-proxy-mitmproxy.md); this guide repeats only
what you need for Burp. Nothing of this is on by default. Without `egress.proxy`
Humanitl is the only proxy and every request without a rule waits for you in
Humanitl.

## How the pieces fit

```text
agent ──▶ Humanitl ──▶ Burp ──▶ target
     TLS with the       TLS with the     TLS to the target;
     Humanitl CA        Burp CA          checked? see below
```

There are two CAs, and each side trusts exactly one of them:

| Connection | Who presents a certificate | Signed by | Who trusts it |
|---|---|---|---|
| Agent to Humanitl | Humanitl, for the name the agent asked for | the Humanitl CA | the agent, inside the sandbox |
| Humanitl to Burp | Burp, a certificate it generates per host | the Burp CA of this installation | Humanitl, through `egress.proxy_ca`, only on this route |
| Burp to the target | the real server | a public CA | Burp, only if it verifies upstream TLS (to be confirmed in the trial) |

Burp's proxy listener by default generates
["CA-signed per-host certificates"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy),
signed by the CA that
["each installation of Burp generates"](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates).
Humanitl opens the agent's TLS, applies its rules and secret detection to the plain
request, then opens a new TLS connection through Burp, with the host name in SNI.
Burp opens that one again, shows you the request and, if you forward it, sends it
to the target. The agent never trusts the Burp CA and never reaches Burp itself; it
still has exactly one way out, Humanitl. Humanitl trusts the Burp CA only on
connections through `egress.proxy`, never on a direct one.

Whether Burp checks the target's certificate is open. Its TLS settings have an
option "Verify upstream TLS" and a list of
["additional certificate authorities that Burp trusts when validating upstream TLS connections"](https://portswigger.net/burp/documentation/desktop/settings/network/tls);
the documentation does not say whether verification is on by default (to be
confirmed in the trial). If Burp does not verify, the connection from Burp to the
target is not checked by anyone, and Humanitl cannot make up for that: it sees only
Burp's certificate, never the target's.

## 1. Start Burp and check the listener

Start Burp Suite as your own user. By default Burp
["creates a single listener on port 8080 of the loopback interface"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy),
which is where Humanitl reaches it. Check it under Proxy settings, "Proxy
listeners": the listener should be bound to the loopback interface only. Binding it
to all interfaces lets other computers in your network use it.

Leave the listener's "Request handling" settings ("Redirect to host", "Redirect to
port", "Force use of TLS", invisible proxying) off. Humanitl is a proxy-aware
client: it sends `CONNECT` before TLS, which is the case Burp handles without
[invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible).

Whether Burp contacts the target before you decide is open: the documentation
does not say when Burp opens the server connection for a held request (to be
confirmed in the trial). mitmproxy does with its defaults, see
[step 1 of the mitmproxy guide](decision-proxy-mitmproxy.md#1-install-and-start-mitmproxy).

## 2. Export the Burp CA

Humanitl needs the certificate of Burp's CA in PEM format. Burp exports it from
the listener settings, as
[Managing CA certificates](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates)
describes:

1. In the Proxy tab, open Proxy settings.
2. Under "Proxy listeners", click "Import / export CA certificate".
3. Under "Export", choose the certificate in DER format, not the one with the
   private key, and save it, for example as `~/burp/burp-ca.der`. (The names of
   the export choices are not in that page; to be confirmed in the trial.)

Convert it to PEM with OpenSSL:

```sh
openssl x509 -inform der -in ~/burp/burp-ca.der -out ~/burp/burp-ca.pem
```

Never give Humanitl a file that holds the private key. Humanitl reads only
certificates from the file and needs nothing else.

## 3. Point Humanitl at Burp

On the command line, each key with `humanitl config set`. `egress.proxy_ca` must be
an absolute path, so let the shell expand `$HOME`:

```sh
humanitl config set egress.proxy http://127.0.0.1:8080
humanitl config set egress.proxy_ca "$HOME/burp/burp-ca.pem"
humanitl config set egress.proxy_resolve proxy     # the default; see "Who resolves names"
```

Or in `config.toml`:

```toml
[egress]
proxy = "http://127.0.0.1:8080"
proxy_ca = "/home/you/burp/burp-ca.pem"
proxy_resolve = "proxy"
proxy_response_timeout_secs = 300
```

In the app: Settings (`Ctrl+,`), section "Decision proxy". "Address" takes
`http://127.0.0.1:8080`; "CA certificate" has "Choose file" for the PEM file;
"Who resolves names" is "Proxy" or "Humanitl". Leave "Credentials" empty: the
default Burp listener has no login.

Every key is described in [CONFIG.md](../CONFIG.md#egress). The keys are
`x-project-scope = denied`: a project profile in a cloned repository can neither
set nor change them.

## 4. Let Burp decide requests without a rule

Set the ask mode `upstream`, so that requests without a rule go to Burp instead of
waiting in Humanitl:

```sh
humanitl config set hold.ask_mode upstream
```

In the app: Settings, section "Requests", "Ask", the fourth choice "Proxy behind".
Humanitl refuses `upstream` without `egress.proxy` (`CONFIG_003`). What the rules
still do in this mode is the same as with mitmproxy, see
[step 4 of the mitmproxy guide](decision-proxy-mitmproxy.md#4-let-mitmproxy-decide-requests-without-a-rule):
`block` answers `403` in Humanitl and Burp never sees the request, `allow` leaves
through Burp at once, `ask` or no rule goes to Burp without holding, `redact` is
still held in Humanitl.

## 5. Restart the daemon and check

The `egress.*` keys take effect when the daemon starts. Restart it while no session
is running:

```sh
humanitl daemon restart
humanitl egress status
humanitl egress check
```

`humanitl egress check` opens a tunnel through Burp to `example.com:443` and the TLS
handshake in it; it sends no request, so nothing waits for you in Burp. A failure
names its cause with a fix, for example an unreachable proxy or a certificate that
does not match `egress.proxy_ca`. The app has the same check under "Decision
proxy", "Check connection". That the check passes against Burp is to be confirmed
in the trial.

## 6. Intercept and decide in Burp

In Proxy, Intercept, the button "Intercept on/off" switches interception:
["If the button shows Intercept on, messages are intercepted"](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages);
with "Intercept off", Burp forwards all messages automatically. Whether
interception is on when Burp starts is a setting,
["Default Proxy interception state"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy).

On a held request, as
[Intercepting messages](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages)
describes:

- "Forward" sends it to the target, and the agent gets the target's answer.
- "Drop" cancels it, "so that they never reach the target server".

Interception rules in Proxy settings
["control which messages are stalled"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)
for you. Which rules a fresh Burp has, and so which requests it lets through
without asking, is to be confirmed in the trial.

Burp does not know which requests a Humanitl rule allowed: its interception applies
to everything that reaches it, the LLM passthrough and `allow` rules included. If
the model server's traffic should not wait for you, add an interception rule that
leaves its host out.

## 7. What the agent sees

| In Burp | The agent gets |
|---|---|
| forwarded | the target's answer, unchanged |
| dropped | to be confirmed in the trial: an answer from Burp, a closed connection (`502` from Humanitl at once) or `502` from Humanitl after `egress.proxy_response_timeout_secs` |
| nobody decides | `502` from Humanitl, after `egress.proxy_response_timeout_secs` |

The last row is Humanitl's own behaviour and holds for any proxy behind it.
`egress.proxy_response_timeout_secs` (default 300 seconds) is how long Humanitl
waits for the head of Burp's answer, because a person may be deciding there. The
briefing of the agent tells it the same: wait, then `502`; a `403` is final.

Humanitl records such a request with the decision source `upstream` (in the audit
`decision: forward`, `decided_by: upstream`) and the status of Burp's answer. It
does not claim an allow or a block it did not see.

## Who resolves names

`egress.proxy_resolve` decides who turns the target's name into an address.

| | `proxy` (default) | `humanitl` |
|---|---|---|
| What Humanitl sends | `CONNECT example.com:443` | `CONNECT 93.184.215.14:443`, the name in SNI |
| DNS query from Humanitl | none; when Burp resolves is to be confirmed in the trial | one, after Humanitl's own rules, before your decision in Burp |
| Private targets | Humanitl refuses private IP literals and local names; a public name that points into your network is Burp's to refuse | Humanitl checks the resolved address too, as without Burp |
| Works with Burp | to be confirmed in the trial | to be confirmed in the trial |

The `CONNECT` with the name is the
[usual way](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible)
a client talks to Burp: "clients send a CONNECT request that identifies the
destination host and then perform TLS negotiation". With `humanitl` the `CONNECT`
names only an address, and Burp's certificate must still carry the name, because
Humanitl checks it against the name. For clients without `CONNECT`, Burp uses the
name in SNI "to generate a certificate for that host"
([Invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible));
whether it does the same after a `CONNECT` to an address is to be confirmed in the
trial. If it does not, `humanitl egress check` fails with a certificate that does
not match.

Choose `proxy` if no DNS query should leave Humanitl before you decided; whether
Burp then waits for your decision before it resolves is open, see step 1. Choose
`humanitl` if your network matters more to you than the query: then nothing private
is reachable, and the name leaves as a DNS query before you see the request.

## Limits

These are in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512) and the
[threat model](../THREAT-MODEL.md); in short:

- Burp sees the decrypted traffic, including secrets Humanitl did not block and
  the LLM passthrough. Its project file and its HTTP history hold them; whoever can
  read those can read them.
- Whether Burp verifies the target's certificate is open (see
  [How the pieces fit](#how-the-pieces-fit)). If it does not, the connection from
  Burp to the target is not checked, and Humanitl cannot make up for that.
- The isolation does not move: the agent reaches only Humanitl, the connection to
  Burp starts in the daemon.
- The LLM passthrough declared for a session is an `allow` that ranks ahead of your
  own block rules; it leaves through Burp, recorded as `passthrough`.
- Requests the daemon makes on its own (search for model servers, update check)
  stay direct.
- Humanitl speaks only `http://` to Burp; TLS to the proxy itself and SOCKS are not
  supported.
- If Burp is not running, requests fail with a finding; they never leave directly
  instead.
- Pausing intercept in Humanitl lets a `redact` request go to Burp as well, as it
  lets everything waiting go out.

## Open until the trial

The trial on a current Burp build answers these; the answers replace the marks
above:

- Does `humanitl egress check` pass, with `egress.proxy_resolve` set to `proxy` and
  to `humanitl`?
- Does a forwarded request reach the agent, and is a dropped one kept from the
  target, with both values?
- Does Burp verify the target's certificate by default ("Verify upstream TLS")?
- Does Burp contact the target before you forward a held request?
- What does the agent get when you drop a request, and after how long?
- Which interception rules and which interception state does a fresh Burp start
  with, and what are the names of the CA export choices?

## Going back

Remove the keys and restart the daemon; Humanitl is the only proxy again:

```sh
humanitl config set hold.ask_mode ui
humanitl config set egress.proxy_ca -
humanitl config set egress.proxy -
humanitl daemon restart
```

`-` (or `null`) removes a key from `config.toml`.
