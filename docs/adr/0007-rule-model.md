# ADR-0007 · Rule model: ordered list, first match wins, default `ask`

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

The human is meant to decide, but not to make the same decision three hundred
times. After the third `allow` for `registry.npmjs.org` there has to be a
rule, otherwise nobody uses the tool for longer than an hour. At the same
time every rule is a permanently opened door: it must be possible to phrase it
so precisely that it opens no more than intended, and it must stay readable
enough that a user understands weeks later what they allowed there.

The traps are known and have all occurred in real products before:
substring matching on host names (`github.com` matches `evil-github.com`),
upper and lower case, trailing dot, internationalised domains and their
Punycode form, IP literals that undercut a host rule, and domain fronting
— the same TLS tunnel, but a different `Host` header inside than outside.

## Decision

Rules are an **ordered list**, **first match wins** applies, and if nothing
matches, the action is **`ask`**. The format is YAML in `rules.yaml`, the
schema deliberately kept such that it remains translatable to
[Cedar](https://www.cedarpolicy.com/) later.

**Actions:** `allow`, `block`, `ask`, `redact` (run the pseudonymisers,
then `allow` or `ask`).

**Matcher keys:** `host`, `method`, `path`, `scheme`, `port`, `upgrade`.

- `host` is a glob **on labels, never on substrings**: `*` stands for exactly
  one label, `**` for one or more labels, a host without a star matches
  exactly. `**.example.com` additionally matches `example.com` itself.
- `path` is a glob or, with a leading `~`, a regular expression
  (`~^/v[0-9]+/`).
- `upgrade: websocket` matches only a WebSocket upgrade request; such a
  request matches no rule without this key and otherwise runs into `ask`.

**Normalisation before every comparison:** host names are brought to A-label
form (Punycode, `idna::domain_to_ascii`), lower-cased and stripped of a
trailing dot. `API.GITHUB.COM.` and `api.github.com` are the same host.

**IP literals never match a host rule.** They need a notation of their own:
`host: "ip:192.168.1.50"` or `host: "cidr:192.168.0.0/16"`.
This rules out that `**` accidentally includes `169.254.169.254`.

**`expires`:** `never` | `session` | timestamp — exactly the three variants
`Expiry::Never | Session | At` from `backlog/CONVENTIONS.md` 3.3. `session` is
bound to the sandbox instance, not to a time of day — whoever restarts the
agent starts with a clean slate. "Allow once" is not a rule and therefore not
an `expires` value: it decides only this one flow and creates nothing.

**Session rules are evaluated before persistent rules.** Within each of the
two groups the list order applies. Reason: what the user has just decided
should apply immediately, even if an older, broader persistent rule would
match as well.

**`allow_private: true`** allows private destination addresses after
resolution; without the flag they are blocked (ADR-0006).

**Authority consistency:** after TLS termination it is checked per request
that `Host` or `:authority` respectively matches the CONNECT target and the
SNI. A mismatch is blocked without asking
(`BlockReason::AuthorityMismatch`, `403`). That is domain fronting, and there
is no legitimate reason to ask a human about it.

**WebSocket:** the upgrade is a decision of its own, preset to `ask`.
After that, frames are recorded but not held individually. The UI states
this limitation explicitly instead of concealing it.

## Rationale

First match wins with default `ask` is the only order that is fail-closed and
can still be explained in one sentence: "The first matching line wins; if none
matches, you are asked." A user can read such a list from top to bottom and
predict what happens. With priority- or specificity-based resolution they
cannot.

Label globs instead of substrings are not convenience but the defence against
an entire class of attacks. `github.com` as a substring matches
`evil-github.com`, `github.com.evil.io` and `mygithub.community`. As a label
expression, `*.github.com` matches exactly `api.github.com` and
`raw.github.com`, but neither `github.com` itself nor `a.b.github.com` nor
anything with a foreign apex. The rule table in escape test 4 enumerates these
cases one by one and is thereby the executable version of this paragraph.

Normalisation to A-label form closes the homograph gap: `рaypal.com` with a
Cyrillic `р` becomes `xn--aypal-uye.com` and is thereby visibly not
`paypal.com` — both for the rule and for the eye in the UI.

That IP literals never match a host rule is deliberate friction. Whoever
wants to reach `169.254.169.254` writes that down. A user who allows
`**.example.com` has thereby certainly not allowed any address.

Binding `session` to the sandbox instance instead of to a time span makes the
lifetime understandable: the rule applies to "this agent run". A time span
would be wrong the moment the user restarts the agent and the old rule lives
on.

The precedence of session rules solves a concrete annoyance: the user clicks
"allow this session", and nothing happens, because a three-week-old `block`
rule stands further up. The order makes the decision just taken effective.

YAML instead of a language of our own, because users are meant to edit the
file by hand and because the schema can be read in typed with `serde_yaml`.
The closeness to Cedar is a door kept open, not a dependency.

## Rejected alternatives

- **OPA/Rego as the policy engine.** Powerful and established, but: a language
  of its own that the audience cannot read, an evaluation that is hard to
  predict, an additional runtime artefact in the package and a considerably
  larger TCB. For an ordered list with six keys that is disproportionate.
- **Cedar right away.** Good semantics, but the Rust integration would have
  made the rule engine more complicated in the MVP without anyone needing the
  expressiveness. Therefore: cut the schema so that a later translation
  remains possible, but do not integrate today.
- **Most specific rule wins.** Sounds helpful, but is not predictable: the
  user would have to keep a specificity metric in mind to understand their own
  list. First match wins needs only reading from top to bottom.
- **Default `allow` with blocklists.** The standard path of almost all
  firewalls and fundamentally wrong here: Humanitl exists because the agent is
  not trusted. A blocklist protects only against known targets.
- **Regular expressions for host names.** Too easy to write wrongly
  (`.` unescaped, missing anchors), too easy to make too broad. Regex remains
  allowed for `path`, where the consequences of a mistake are smaller, and is
  marked there as the exception by the `~` prefix.
- **Ask about domain fronting instead of blocking.** Rejected: there is no
  legitimate case in which the inner `Host` deviates from the CONNECT target,
  and asking would train the user to click away an attack signature.
- **Hold WebSocket frames individually.** Would be consistent, but makes every
  interactive connection unusable. The point of attachment is the upgrade; the
  rest is recorded and the gap is named in the UI. A switch
  `experimental.ws_hold` was meant to keep the door open; it was dropped with
  HUM-121 because it never got a reader and the upgrade path itself is still
  pending (HUM-110). The door stays open, it just no longer hangs on a key
  without effect: whoever wants to hold the frames brings the switch back
  together with its reader in the same commit (`backlog/CONVENTIONS.md` 4.25).

## Consequences

- The rule engine (`humanitl-rules`) is pure: parsing, normalising, matching,
  without IO and without async. The value types (`Rule`, `Matcher`, `Action`,
  `Expiry`, `HostPattern`, `Upgrade`) live in `humanitl-core::rule`
  (`backlog/CONVENTIONS.md` 4.1).
- Rules have two storage places: persistent in `rules.yaml` and temporary in
  the daemon's memory (`expires: session`). The UI shows both in separate
  tabs, temporary ones with remaining lifetime and "make permanent" (ADR-0011).
- `RuleSet::evaluate` returns `Verdict::Matched { rule, action }` or
  `Verdict::Default`, which means `ask`. Unknown HTTP methods lead to
  `ask`, not to an error.
- Every rule can name its origin (`created_from: <FlowId>`), so that the
  rules screen can show "created 2 min ago from request #41".
- Escape test 4 is the binding table: `*.github.com` against
  `api.github.com` ✓, `github.com` ✗, `evil-github.com` ✗, `github.com.evil.io` ✗,
  `a.b.github.com` ✗, `API.GITHUB.COM.` ✓, plus the `**` variants, IP literals
  and the WebSocket case.
- Bundled rules (`rules/default.yaml`) carry a `bundled: true` and are marked
  as such in the UI, so that a user recognises what they created themselves.

## Affected issues

`HUM-022` (rule engine, escape test 4), `HUM-023` (host/SNI/authority
consistency), `HUM-027` (rules RPCs including `dry_run`), `HUM-033` (rules
screen with tabs "Saved" and "Temporary"), `HUM-038` (default rules),
`HUM-039` (LLM passthrough rule), `HUM-065` (`humanitl rules test`).
