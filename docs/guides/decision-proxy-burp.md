# Burp Suite behind Humanitl

Applies to: Humanitl 0.1

> **Tried by the AI under a virtual display, not yet by a person.** On 2026-11-18
> Burp Suite Community Edition 2026.11.2 (the JAR from portswigger.net, checksum
> checked against the one PortSwigger publishes) ran on Linux under Xvfb, and
> Humanitl's egress code was pointed at its listener. Where this guide says
> "observed in the trial", that is what this build did; every other statement about
> Burp links to the PortSwigger documentation. What the trial could not show is
> marked "not observed in the AI trial" and is open for the walkthrough with the
> owner (HUM-553). How the trial was run is described under
> [How the trial was run](#how-the-trial-was-run).

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

Burp needs three settings changed from its defaults before it is a safe decision
proxy, and Burp Community forgets them at every start. Step 1 loads them from a
file; the reasons are in steps 1, 6 and 7:

- "Verify upstream TLS" is off by default, so Burp passes a target certificate
  nobody trusts.
- A dropped request reaches the agent as `200 OK` unless "Suppress Burp error
  messages in browser" is on.
- The default interception rule lets requests for images, stylesheets, scripts and
  fonts through without asking.

## How the pieces fit

```text
agent ──▶ Humanitl ──▶ Burp ──▶ target
     TLS with the       TLS with the     TLS to the target, checked only
     Humanitl CA        Burp CA          with "Verify upstream TLS" on
```

There are two CAs, and each side trusts exactly one of them:

| Connection | Who presents a certificate | Signed by | Who trusts it |
|---|---|---|---|
| Agent to Humanitl | Humanitl, for the name the agent asked for | the Humanitl CA | the agent, inside the sandbox |
| Humanitl to Burp | Burp, a certificate it generates per host | the Burp CA of this installation | Humanitl, through `egress.proxy_ca`, only on this route |
| Burp to the target | the real server | a public CA | Burp, only with "Verify upstream TLS" on; off by default |

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

Burp checks the target's certificate only if you tell it to. Its TLS settings say:
["To enable upstream TLS verification, click Verify upstream TLS"](https://portswigger.net/burp/documentation/desktop/settings/network/tls).
Observed in the trial: the option is off in a fresh Burp, and a target whose
self-signed certificate nobody trusted reached the agent with its answer. With the
option on, Burp refused that target (the target saw the TLS alert "certificate
unknown") and the agent got `502` at once. Without it, the connection from Burp to
the target is not checked by anyone, and Humanitl cannot make up for that: it sees
only Burp's certificate, never the target's. Burp trusts further CAs for this check
from its list of
["additional certificate authorities that Burp trusts when validating upstream TLS connections"](https://portswigger.net/burp/documentation/desktop/settings/network/tls).
That a target with a certificate from a public CA passes with the option on was not
observed in the AI trial (open for the owner walkthrough, HUM-553).

## 1. Start Burp with the settings for Humanitl

Burp Community has only a temporary project in memory; the start dialog says
"Project files are Burp Suite Professional only" (observed in the trial).
[Project settings](https://portswigger.net/burp/documentation/desktop/settings)
"only apply to the current project", so whatever you change in them is gone at the
next start. Keep them in a file instead. Save this as `~/burp/humanitl.json`:

```json
{
  "project_options": {
    "ssl": { "negotiation": { "enforce_upstream_trust": true } }
  },
  "proxy": {
    "intercept_client_requests": {
      "do_intercept": true,
      "rules": [
        {
          "boolean_operator": "and",
          "enabled": false,
          "match_condition": "(^gif$|^jpg$|^png$|^css$|^js$|^ico$|^svg$|^eot$|^woff$|^woff2$|^ttf$)",
          "match_relationship": "does_not_match",
          "match_type": "file_extension"
        }
      ]
    },
    "miscellaneous": { "suppress_burp_error_messages_in_browser": true }
  }
}
```

It switches on "Verify upstream TLS", switches off the default interception rule
that skips static files (step 6), and switches on "Suppress Burp error messages in
browser" (step 7). The key names come from a file Burp 2026.11.2 saved itself with
"Save project settings" (observed in the trial); the
[settings page](https://portswigger.net/burp/documentation/desktop/settings)
describes saving and loading them as JSON.

Start Burp with the file. The JAR takes it as `--config-file`, which
["Load[s] the specified project configuration file"](https://portswigger.net/burp/documentation/desktop/troubleshooting/launch-from-command-line):

```sh
java -jar burpsuite.jar --config-file="$HOME/burp/humanitl.json"
```

Choose "Temporary project in memory" and "Start Burp". Observed in the trial: all
three settings were in effect after such a start. The JAR needs your own Java;
PortSwigger names
["Java 21"](https://portswigger.net/burp/documentation/desktop/troubleshooting/launch-from-command-line)
as the minimum, and the native installers bring their own. If you start Burp from
its installer instead, load the file after the start: Settings, "Manage global
settings", "Project settings", "Load project settings" (these menu names observed in
the trial). Whether the installed launcher takes `--config-file` was not observed in
the AI trial (open for the owner walkthrough, HUM-553).

Then check the listener under Proxy settings, "Proxy listeners". By default Burp
["creates a single listener on port 8080 of the loopback interface"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy),
which is where Humanitl reaches it; observed in the trial: `127.0.0.1:8080`,
certificate "Per-host". Binding it to all interfaces lets other computers in your
network use it.

Leave the listener's "Request handling" settings ("Redirect to host", "Redirect to
port", "Force use of TLS", invisible proxying) off. Humanitl is a proxy-aware
client: it sends `CONNECT` before TLS, which is the case Burp handles without
[invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible).

Whether Burp contacts the target before you decide: observed in the trial, Burp
did not send a held request to the target before "Forward", with either value of
`egress.proxy_resolve`. Of 16 held requests for which the target's connection log
was read while the request waited (6 to 12 seconds), 15 left no connection at the
target before the decision. Once, the target saw a TLS handshake without a request
at the moment a held request reached Burp; the 10 held requests after it did not
repeat it. Take it as possible: the target may learn that
someone connects (your address, the name in SNI) before you decide. mitmproxy with
its defaults does that every time, see
[step 1 of the mitmproxy guide](decision-proxy-mitmproxy.md#1-install-and-start-mitmproxy).

## 2. Export the Burp CA

Humanitl needs the certificate of Burp's CA in PEM format. Burp exports it from
the listener settings, as
[Managing CA certificates](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates)
describes:

1. In the Proxy tab, open Proxy settings.
2. Under "Proxy listeners", click "Import / export CA certificate".
3. Under "Export", choose "Certificate in DER format", click "Next", enter a file
   name such as `~/burp/burp-ca.der` and click "Next" again. Do not choose "Private
   key in DER format" or "Certificate and private key in PKCS#12 keystore". (The
   names of the choices observed in the trial.)

Convert it to PEM with OpenSSL:

```sh
openssl x509 -inform der -in ~/burp/burp-ca.der -out ~/burp/burp-ca.pem
```

Observed in the trial: the certificate is called "PortSwigger CA". Never give
Humanitl a file that holds the private key. Humanitl reads only certificates from
the file and needs nothing else.

## 3. Point Humanitl at Burp

On the command line, each key with `humanitl config set`. `egress.proxy_ca` must be
an absolute path, so let the shell expand `$HOME`:

```sh
humanitl config set egress.proxy http://127.0.0.1:8080
humanitl config set egress.proxy_ca "$HOME/burp/burp-ca.pem"
humanitl config set egress.proxy_resolve proxy     # the default; keep it with Burp
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
default Burp listener has no login. Keep "Proxy": with Burp, "Humanitl" does not
protect your network, see [Who resolves names](#who-resolves-names).

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
proxy", "Check connection".

Observed in the trial: the check passed against Burp with `egress.proxy_resolve`
set to `proxy` and to `humanitl`, with `proxy` also with "Verify upstream TLS" on. The trial ran
the check's code from Humanitl's proxy crate, not the `humanitl` command against a
running daemon; the command and the button in the app were not observed in the AI
trial (open for the owner walkthrough, HUM-553). The check shows that Humanitl
reaches Burp and trusts its CA. It does not show that Burp reaches the target:
Burp completes the handshake with Humanitl without contacting the target (observed
in the trial with held requests).

## 6. Intercept and decide in Burp

In Proxy, Intercept, the button "Intercept on/off" switches interception:
["If the button shows Intercept on, messages are intercepted"](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages);
with "Intercept off", Burp forwards all messages automatically. Observed in the
trial: a fresh Burp started with "Intercept off", also with the file from step 1,
so switch it on after every start. Whether interception is on when Burp starts is a
user setting,
["Default Proxy interception state"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy).

On a held request, as
[Intercepting messages](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages)
describes:

- "Forward" sends it to the target, and the agent gets the target's answer.
- "Drop" cancels it, "so that they never reach the target server".

Observed in the trial, with both values of `egress.proxy_resolve`: a forwarded
request reached the target only after "Forward", and the agent got the target's
answer; a dropped request never reached the target. The trial clicked the buttons
with a program on a virtual display; a person at a real screen was not observed in
the AI trial (open for the owner walkthrough, HUM-553).

Interception rules in Proxy settings
["control which messages are stalled"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)
for you. Observed in the trial: a fresh Burp has one rule switched on, "File
extension Does not match `(^gif$|^jpg$|^png$|^css$|^js$|^ico$|^svg$|^eot$|^woff$|^woff2$|^ttf$)`".
With it, a request of the agent for `/lib/app.js` was not held although
interception was on: Burp contacted the target within 76 milliseconds (the target
logged the connection; it refused it, because "Verify upstream TLS" was on in that
run). The file from step 1 switches the rule off; then the same request stood in
Burp's Intercept list and the target saw no connection for 6 seconds until "Drop"
(observed in the trial). If you start Burp without the
file, switch the rule off by hand.

Burp does not know which requests a Humanitl rule allowed: its interception applies
to everything that reaches it, the LLM passthrough and `allow` rules included. If
the model server's traffic should not wait for you, add an interception rule that
leaves its host out. How the LLM passthrough of a real session behaves through Burp
was not observed in the AI trial (open for the owner walkthrough, HUM-553).

## 7. What the agent sees

| In Burp | The agent gets |
|---|---|
| forwarded | the target's answer, unchanged |
| dropped, with "Suppress Burp error messages in browser" on | `502` from Humanitl, at once |
| dropped, with Burp's default | `200 OK` from Burp with an HTML page "Request was dropped by user.", at once |
| Burp cannot resolve the target's name, Burp's default | `200 OK` from Burp with an HTML page "Unknown host: …", at once |
| Burp does not trust the target ("Verify upstream TLS" on), with "Suppress Burp error messages in browser" on | `502` from Humanitl, at once |
| nobody decides | `502` from Humanitl, after `egress.proxy_response_timeout_secs` |

All rows observed in the trial. Not observed: an unknown name with the setting on,
and an untrusted target with Burp's default; going by the two "dropped" rows they
are likely the same `502` and `200 OK`, but the trial did not show it. The default
answer to "Drop" is the dangerous one:
the agent sees `200 OK` and may take Burp's error page for the target's answer, and
Humanitl records the request with status `200`. The same happened when Burp could
not resolve the target's name ("Unknown host"). With
["Suppress Burp error messages in browser"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)
on, which "suppress[es] the messages that are usually sent to the browser when
errors occur", Burp closes the connection instead, and Humanitl answers `502` at
once. The file from step 1 switches it on.

The last row is Humanitl's own behaviour and holds for any proxy behind it; in the
trial it came after the deadline, with the reason `upstream_timeout` in the body.
`egress.proxy_response_timeout_secs` (default 300 seconds) is how long Humanitl
waits for the head of Burp's answer, because a person may be deciding there. The
briefing of the agent tells it the same: wait, then `502`; a `403` is final.

Humanitl records such a request with the decision source `upstream` (in the audit
`decision: forward`, `decided_by: upstream`) and the status of Burp's answer. It
does not claim an allow or a block it did not see.

## Who resolves names

`egress.proxy_resolve` decides who turns the target's name into an address. With
Burp, use `proxy`.

| | `proxy` (default) | `humanitl` |
|---|---|---|
| What Humanitl sends | `CONNECT example.com:443` | `CONNECT 93.184.215.14:443`, the name in SNI |
| DNS query from Humanitl | none | one, after Humanitl's own rules, before your decision in Burp |
| Where Burp connects | to the name, resolved by Burp; the request only after your decision, though a TLS handshake without a request was seen once before it (step 1) | to the name again, resolved by Burp; the address in `CONNECT` is not used |
| Private targets | Humanitl refuses private IP literals and local names; a public name that points into your network is Burp's to refuse | the same as with `proxy`: Humanitl's check of the resolved address does not bind Burp |
| Works with Burp | yes, observed in the trial | the request goes through, observed in the trial, but see the row above |

The `CONNECT` with the name is the
[usual way](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible)
a client talks to Burp: "clients send a CONNECT request that identifies the
destination host and then perform TLS negotiation". Observed in the trial, with
`humanitl`: Burp accepted `CONNECT 127.0.0.1:18443` and gave Humanitl a certificate
for the name in SNI, which Humanitl accepted. But for the request in the tunnel
Burp resolved the name in `Host` itself and ignored the address: with a name Burp
could not resolve, the request failed with "Unknown host", although the `CONNECT`
named an address that was reachable.

So with Burp, `humanitl` costs the DNS query before your decision and buys nothing:
Humanitl checks an address that Burp does not use, and Burp's own lookup decides
where the request goes. Burp connected to a target on `127.0.0.1` without objection
(observed in the trial), so a public name that resolves to your router or to a
service on your computer reaches it if you forward the request. Look at the host
before you click "Forward".

## Limits

These are in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512) and the
[threat model](../THREAT-MODEL.md); in short:

- Burp sees the decrypted traffic, including secrets Humanitl did not block and
  the LLM passthrough. Its HTTP history holds them while Burp runs; whoever can
  read Burp's memory can read them.
- Burp verifies the target's certificate only with "Verify upstream TLS" on (step
  1). Without it, the connection from Burp to the target is not checked, and
  Humanitl cannot make up for that.
- `egress.proxy_resolve = "humanitl"` does not keep private targets out with Burp,
  because Burp resolves the name again (see
  [Who resolves names](#who-resolves-names)).
- Burp's default answer to "Drop" and to its own errors is `200 OK` with an error
  page (step 7).
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

## How the trial was run

On 2026-11-18 the AI ran Burp Suite Community Edition 2026.11.2 from the JAR
(SHA-256
`ddd11e506bbbee7491c120901722a8bbd79d64d1298afceee1d334d27981516b`, as published
by PortSwigger) with OpenJDK 25 from
Debian, which Burp flags as "not been fully tested", on a virtual X display (Xvfb),
with its own home directory. It accepted the licence, chose Community Edition and a
temporary project, exported the CA as above and clicked Intercept, Forward and Drop
with `java.awt.Robot`, reading the screen from screenshots.

In place of a running daemon, the trial used Humanitl's proxy from the test support
of the proxy crate: the same egress code with `hold.ask_mode = "upstream"`,
`egress.proxy = http://127.0.0.1:8080`, the exported CA as `egress.proxy_ca`, and
both values of `egress.proxy_resolve`. The agent was a client speaking `CONNECT`
and TLS to that proxy. The target was a small TLS server on `127.0.0.1` with a
self-signed certificate for `upstream.test` that nothing trusted; it logged every
connection with a time stamp, and the trial read that log while a request was held
and again after the decision, which is how it counted contacts before the decision.
The agent's side logged status, body and time of each answer. These logs and the
screenshots stay with the HUM-524 trial notes outside the repository. Burp
found it through its "Hostname resolution overrides"
([DNS settings](https://portswigger.net/burp/documentation/desktop/settings/network/dns)).
The check from step 5 ran against the real `example.com`.

Not observed in the AI trial, open for the walkthrough with the owner (HUM-553):

- `humanitl egress check` and "Check connection" in the app against Burp, with a
  real daemon after `humanitl daemon restart`.
- A real agent session through Humanitl and Burp, the LLM passthrough included.
- A target with a certificate from a public CA, with "Verify upstream TLS" on.
- Burp from the native installer, and whether its launcher takes `--config-file`.
- Intercept, Forward and Drop clicked by a person at a real screen, and the
  interception state at the start of the owner's own Burp.
- Whether the single early TLS handshake to the target (step 1) shows up again.

## Going back

Remove the keys and restart the daemon; Humanitl is the only proxy again:

```sh
humanitl config set hold.ask_mode ui
humanitl config set egress.proxy_ca -
humanitl config set egress.proxy -
humanitl daemon restart
```

`-` (or `null`) removes a key from `config.toml`.
