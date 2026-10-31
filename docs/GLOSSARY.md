# Glossary

Applies to: Humanitl 0.1

Binding for every text in this repository and for every translation in
`app/l10n/app_en.arb` (source) and `app/l10n/app_de.arb` (HUM-052, HUM-086).
Anyone writing a new text takes the terms from here; anyone who wants to
change a term changes it here first and gives the reason.

`dart run tool/l10n_lint.dart` (from `app/`, part of `make l10n-lint`) reads
the tables "UI terms" and "Forbidden words" in this file. For every row of the
first, the named key must exist in both ARB files, and its text must contain
the term of the respective language as a whole word (case does not matter).
No German text may contain a word of the second table.

## Principles

- English is the source language; German is first-class, not an afterthought.
- German addresses the human with "du", never with "Sie".
- Protocol terms stay English: GET, POST, Header, Body, Query, Status,
  Content-Type. Keyboard shortcut hints are language-independent.
- Only what is displayed is translated. What ends up in audit records,
  `rules.yaml` or `config.toml` (`allow`, `block`, `ask`, `redact`, the kinds
  of findings) stays the English identifier.
- "Session" stays "Session", in German too, wherever the running session of
  the agent is meant.
- Prose in the repository uses British spelling: pseudonymise, behaviour,
  colour, licence (noun). Identifiers and UI strings keep the spelling they
  have in code and in `app_en.arb` ("Pseudonymize").
- German text that must stay German in a document (a quotation, fixture
  content) stands between `<!-- lang: de -->` and `<!-- /lang -->`.

## Project terms

The terms of the documentation and the code comments, with the German word
they replace (HUM-086). Sandbox, Diagnostic and Finding stay unchanged.

| en | de | Meaning |
|---|---|---|
| held | angehalten | State of a flow that waits for a human decision; never "intercepted" (the screen name "Intercept" is only the name of the view) |
| hold | anhalten | Stop a request before it leaves the machine |
| allow | erlauben, senden | Let a held request go out; also the rule action `allow` |
| block | blockieren | Refuse a request; never "reject" |
| ask | nachfragen | Rule action `ask`: hold the request and ask the human |
| redact | pseudonymisieren (Regelaktion) | Rule action `redact`: pseudonymise the findings before sending |
| pseudonymise | pseudonymisieren | Replace a value with a consistent stand-in; never "anonymise" |
| pseudonym | Pseudonym | The stand-in value that replaces a finding |
| mapping | Zuordnung | The table from pseudonym back to the original value |
| rule | Regel | One entry of `rules.yaml`: match, action, target, duration |
| target | Ziel, Geltungsbereich, Scope | What a remembered decision applies to: exact URL, host, domain, host and method |
| duration | Gültigkeit | How long a rule applies: once, this session, always |
| decision | Entscheidung | The human's answer to a held request |
| request | Anfrage | One outgoing HTTP request of the agent |
| response | Antwort | The upstream answer to a request |
| flow | Fluss, Flow | One request with its response and its state, as the daemon tracks it |
| queue | Warteschlange | The list of held flows waiting for a decision |
| history | Verlauf | The recorded flows of the past |
| recording | Aufzeichnung | What the recorder stores about each flow |
| audit chain | Audit-Kette | The hash-chained, keyed audit records that make tampering evident; they do not protect against an attacker running as the same user who holds both the key and the file (ADR-0008, `docs/SECURITY.md`) |
| Finding | Fund, Befund | A detected secret or piece of personal data in a request; in reviews, a reported defect |
| secret | Secret, Geheimnis | A credential such as an API key or token |
| personal data | personenbezogene Daten | PII in the sense of the GDPR |
| Diagnostic | Diagnostic, Diagnose | Structured error with code, title, why and fix; never a bare string |
| why | Grund | The field of a Diagnostic that explains the cause |
| fix | Abhilfe | The field of a Diagnostic that tells the human what to do |
| Sandbox | Sandbox | The isolated environment the agent runs in |
| sandbox backend | Sandbox-Backend | The adapter that builds the sandbox (today bubblewrap) |
| guarantee | Garantie | One of the three sandbox guarantees in `README.md` |
| escape test | Escape-Test, Ausbruchstest | A test under `tests/escape/` that proves a guarantee holds |
| daemon | Daemon | The background process that owns proxy, sandbox, rules and recording |
| session | Session, Sitzung | One run of an agent inside a sandbox |
| passthrough | Durchleitung, Durchreiche | Forwarding of LLM traffic without holding it |
| egress | Egress, Ausgang | The proxy's path to the internet (upstream) |
| airlock | Schleuse | The design direction of the interface (`BACKLOG.md` section 5) |
| timed out | Zeit abgelaufen, Zeitüberschreitung | A held flow whose decision deadline passed |
| project folder | Projektordner | The host directory mounted into the sandbox as `/work` |
| acceptance criterion | Akzeptanzkriterium | A checkable condition an issue must meet |
| pitfall | Fallstrick | A known trap named in an issue specification |
| mutation proof | Mutationsbeweis | Evidence that a test fails when the code it guards is broken |

## UI terms

<!-- lang: de -->
| Term | en | de | ARB key | Note |
|---|---|---|---|---|
| held (state) | Held | Angehalten | `stateHeld` | never "abgefangen" |
| hold (verb) | Hold | Anhalten | `commonHold` | |
| allow (button on request) | Allow | Senden | `interceptAllowButton` | The button names what happens. English "Allow" instead of "Send" per `docs/UX.md` 4.6 |
| allow (rule action) | Allow | Erlauben | `rulesActionAllow` | in rules only |
| allow edited | Send edited version | Editierte Version senden | `editorSend` | |
| block (button) | Block | Blockieren | `interceptBlockButton` | not "Ablehnen" |
| block (rule action) | Block | Blockieren | `rulesActionBlock` | |
| ask (rule action) | Ask | Nachfragen | `rulesActionAsk` | |
| redact (rule action) | Redact | Pseudonymisieren | `rulesActionRedact` | The action `redact` stays English in the YAML |
| timed out | Timed out | Zeit abgelaufen | `stateTimedOut` | |
| auto-allowed by rule | Allowed by rule | Durch Regel erlaubt | `stateAllowedByRule` | |
| LLM passthrough | LLM passthrough | LLM-Durchleitung | `statePassthroughLlm` | |
| edited | Edited | Editiert | `historyChipEdited` | Chip |
| finding | Finding | Fund | `commonFinding` | not "Treffer" |
| secret | Secret | Secret | `commonSecret` | stays |
| PII | Personal data | Personenbezogene Daten | `commonPersonalData` | |
| pseudonymize | Pseudonymize | Pseudonymisieren | `commonPseudonymize` | never "Anonymisieren"; prose spells it "pseudonymise" |
| pseudonym | Pseudonym | Pseudonym | `editorMappingPseudonym` | |
| mapping | Mapping | Zuordnung | `editorMapping` | Panel title "Zuordnung (3)" |
| replace all | Replace all with pseudonyms | Alle durch Pseudonyme ersetzen | `editorReplaceAll` | |
| ignore (once) | Ignore | Ignorieren | `editorFindingIgnore` | |
| ignore always | Always ignore | Immer ignorieren | `editorFindingIgnoreAlways` | |
| send anyway | Send anyway | Trotzdem senden | `commonSendAnyway` | |
| rule | Rule | Regel | `rulesEditorTitle` | |
| remember | Remember | Merken | `interceptRemember` | |
| scope (target) | Target | Ziel | `interceptScopeTarget` | |
| scope (duration) | Duration | Gültigkeit | `interceptScopeDuration` | |
| once | Once | Einmal | `interceptDurationOnce` | |
| this session | This session | Diese Session | `rulesExpirySession` | "Session" stays |
| forever | Always | Immer | `rulesExpiryAlways` | |
| exact URL | Exact URL | Genaue URL | `rulesTargetExactUrl` | |
| host | Host | Host | `interceptTargetHost` | |
| domain (apex + subs) | Domain and subdomains | Domain und Subdomains | `rulesTargetDomain` | |
| host + method | Host and method | Host und Methode | `rulesTargetHostMethod` | |
| queue | Queue | Warteschlange | `interceptQueueTitle` | |
| history | History | Verlauf | `shellNavHistory` | |
| intercept (screen name) | Intercept | Anhalten | `shellNavIntercept` | Rail label |
| sandbox | Sandbox | Sandbox | `shellNavSandbox` | |
| isolation check | Isolation check | Isolationsprüfung | `sandboxIsolationCheck` | |
| no network interface | No network interface | Kein Netzwerk-Interface | `sandboxGuaranteeNoInterface` | |
| one socket | Exactly one socket, to Humanitl | Genau ein Socket, zu Humanitl | `sandboxGuaranteeOneSocket` | |
| seccomp active | New sockets forbidden (seccomp) | Neue Sockets verboten (seccomp) | `sandboxGuaranteeSeccomp` | |
| no way out | No way out | Kein Weg nach außen | `sandboxGuaranteeNoWayOutMacos` | guarantee 1 on macOS |
| one door | Exactly one door, to Humanitl | Genau eine Tür, zu Humanitl | `sandboxGuaranteeOneDoorMacos` | guarantee 2 on macOS |
| held to the profile | Held to the Seatbelt profile | An das Seatbelt-Profil gehalten | `sandboxGuaranteeProfileMacos` | guarantee 3 on macOS |
| project folder | Project folder | Projektordner | `setupCheckProject` | |
| work dir | Work directory | Arbeitsverzeichnis | `sandboxWorkDirectory` | `/work` stays |
| agent is waiting | The agent is waiting for you | Der Agent wartet auf dich | `commonAgentWaiting` | "du" form |
| daemon | Daemon | Daemon | `commonDaemon` | |
| audit chain | Audit chain | Audit-Kette | `auditChainTitle` | |
| verified | Verified | Verifiziert | `auditStatusOk` | |
| broken at | Broken at sequence {seq} | Gebrochen ab Sequenz {seq} | `auditBrokenAtSequence` | |
| retention | Retention | Aufbewahrung | `auditRetentionTitle` | |
| settings tier basic | Basic | Grundlegend | `settingsTierBasic` | |
| settings tier advanced | Advanced | Erweitert | `settingsTierAdvanced` | |
| settings tier expert | Expert | Experte | `settingsTierExpert` | |
| origin (of a setting) | Source | Herkunft | `settingsOrigin` | |
| reset | Reset to default | Auf Standard zurücksetzen | `settingsResetToDefault` | |
<!-- /lang -->

## Forbidden words

<!-- lang: de -->
| Word | instead | Reason |
|---|---|---|
| abgefangen | angehalten | Humanitl holds and decides together with the human; "abgefangen" (intercepted) sounds like surveillance |
| Treffer | Fund | A finding is a statement about the request, not a search hit |
| anonymisier | pseudonymisieren | Anonymising is something different in law (BACKLOG.md 5) |
| ablehn | blockieren | The button blocks a request; "ablehnen" (reject) sounds like objecting to a person |
<!-- /lang -->

## Deviations from the specification

- **"Allow" on the English button.** HUM-052 named "Send". `docs/UX.md`
  4.6 fixes the three places (button "Allow"/"Senden", rule
  "allow"/"Erlauben", strip "Sent to …"/"Gesendet an …") and gives the reason
  for the split; the glossary follows it.
- **"Forbidden words" are word stems.** "ablehn" matches "Ablehnen",
  "anonymisier" matches "Anonymisieren" and "anonymisiert". "abgelehnt" stays
  allowed: there a service turns something away, such as the daemon a token
  (`setupTokenInvalidTitle`), and no button is offered as a choice.
- **The headings "UI terms" and "Forbidden words"** replace "Begriffe" and
  "Verbotene Wörter" since HUM-086; `app/tool/l10n_lint.dart` looks the tables
  up by these headings.
