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

Since sprint 9 (#79) every concept a user meets has exactly one word in each
language, the same in the CLI, the terminal views, the app, the documentation
and the diagnostics. The sections "Concepts", "Isolation levels" and "Setting
levels" fix these words. Where today's texts still use an older word, the
[replacement table](#replacement-table) names it; it is the work list of the
terminology sweep that follows as its own issue. Until that sweep lands, the
table "UI terms" describes the ARB files as they are, and the sweep changes a
row there together with its key.

## Principles

- English is the source language; German is first-class, not an afterthought.
- German addresses the human with "du", never with "Sie".
- Protocol terms stay English: GET, POST, Header, Body, Query, Status,
  Content-Type. Keyboard shortcut hints are language-independent.
- Only what is displayed is translated. What ends up in audit records,
  `rules.yaml` or `config.toml` (`allow`, `block`, `ask`, `redact`, the kinds
  of findings) stays the English identifier.
- "Session" stays "Session", in German too, wherever the running session of
  the agent is meant. „Sitzung" is not used.
- One concept, one word. There is no second, simpler vocabulary for
  beginners: the words below are meant to be plain and exact at the same
  time. A text that needs a word for a concept listed here uses that word and
  no synonym.
- User-facing words are not identifiers. Configuration keys, their values,
  protocol fields, RPC names, command names and diagnostic codes keep their
  names (`held`, `disposable`, `depot`, `daemon`, `humanitl sandbox attach`).
  A text may quote an identifier in backticks next to the user-facing word,
  for example "background service (`humanitld`)".
- No word promises more than the tool does. Humanitl is never called "safe",
  "secure", „sicher" or "tamper-proof"/„manipulationssicher", and nothing is
  "guaranteed" except the three sandbox guarantees in `README.md`. The audit
  records show changes; they do not prevent them.
- ADRs and the sprint files under `backlog/` are history and keep the words of
  their time. Everything else follows this glossary.
- Prose in the repository uses British spelling: pseudonymise, behaviour,
  colour, licence (noun). Identifiers and UI strings keep the spelling they
  have in code and in `app_en.arb` ("Pseudonymize").
- German text that must stay German in a document (a quotation, fixture
  content) stands between `<!-- lang: de -->` and `<!-- /lang -->`.

## Concepts

One word per concept, decided for sprint 9 (#79, owner decisions of
2026-11-23). The column "Identifiers" names what stays as it is in code,
configuration and protocol.

<!-- lang: de -->
| Concept | en | de | Meaning | Identifiers |
|---|---|---|---|---|
| A request that waits for a human | waits for a decision | wartet auf Entscheidung | Humanitl has stopped the request before it left the machine, and it goes out only when a human allows it; when the deadline passes it is blocked, never allowed. | `held`, `FlowState::Held`, `hold.*` |
| The time a request waits | decision deadline | Entscheidungsfrist | How long a request waits for a decision before Humanitl blocks it. | `hold.timeout_secs` |
| The view with the waiting requests | Decisions | Entscheidungen | The view in which the requests that wait for a decision are listed and decided. | `shellNavIntercept`, `features/intercept` |
| Changes go to a copy first | working copy; as a choice "Working copy – review changes first" | Arbeitskopie; als Wahl „Arbeitskopie – Änderungen erst prüfen" | The agent writes into a copy of the project folder, and nothing reaches the project folder until you review the changes and apply them. The default `auto` gives a working copy where the system allows it and otherwise writes directly, with a notice (`SANDBOX_033`). | `sandbox.mode = "disposable"` (default `auto`), `--disposable` |
| Changes go to the folder | direct; as a choice "Direct into the project folder" | direkt; als Wahl „Direkt in den Projektordner" | The agent writes into the project folder itself, so there is nothing to review afterwards. | `sandbox.mode = "direct"`, `humanitl run --direct`, `humanitl go --rw` |
| Agent programs kept apart from the computer | Agents (installed in isolation) | Agenten (isoliert installiert) | Humanitl installs, updates and logs in these agent programs inside a sandbox, apart from the programs on this computer; the command is `humanitl agents`. | `depot`, `agents.<id>.source = "depot"` |
| Where an agent program comes from | installed in isolation; on this computer | isoliert installiert; auf diesem Rechner | The two sources of an agent program: the copy Humanitl installed in isolation, or the program already installed on this computer. | `source = "depot"`, `source = "host"` |
| Leaving a session without ending it | keep running in the background | im Hintergrund weiterlaufen lassen | The window or terminal closes and the agent keeps working; its requests still wait for a decision and are blocked when their deadline passes. The app and `humanitl go` with `terminal.on_close = "detach"` (the default) do this; closing the terminal of `humanitl run` ends its session. | `detach`, `terminal.on_close = "detach"` |
| Coming back to such a session | open again | wieder öffnen | Shows a session that runs in the background in a terminal or window again. | `attach`, `humanitl sandbox attach` |
| The process that does the work | background service | Hintergrunddienst | The program `humanitld` that runs the local proxy, the sandboxes, the rules and the recording for your user, also when no window is open. | `daemon`, `humanitld`, `humanitl daemon …` |
| The audit records | tamper-evident log | manipulationserkennbares Protokoll | Decisions and flow events are recorded, chained and keyed, so that a check shows whether records up to the last anchor were changed, removed or reordered; it does not show a cut at the end after the last anchor, events that were never written (`audit.events_dropped`), or what a dishonest background service writes, it prevents nothing, and it does not hold against an attacker running as your user (`docs/SECURITY.md` section 8, ADR-0008). | `audit`, `audit.*`, `humanitl audit verify` |
| Proxy 1 | local proxy | lokaler Proxy | The proxy inside the background service; it is the only way out of the sandbox, and every request of the agent passes it. | `proxy.sock` |
| Proxy 2 | upstream proxy | Upstream-Proxy | A proxy of your network, such as a company proxy, through which allowed requests have to leave; it decides nothing for Humanitl and is not built yet (ADR-0017). The identifier `upstream` in `hold.ask_mode` and in the sources of a recording means the decision proxy, not this one. | none yet |
| Proxy 3 | decision proxy | Entscheidungs-Proxy | Your own intercepting proxy behind Humanitl, such as Burp or mitmproxy, in which allowed requests pass and, with the ask mode `upstream`, undecided requests are decided. | `egress.proxy`, `hold.ask_mode = "upstream"` |
| One agent at work | session | Session | One agent running in one sandbox, from its start until it ends; it can keep running in the background. A session is never called a "run"; `humanitl run` stays the name of a command. | `session`, `humanitl sessions` |
| Where a session runs | sandbox | Sandbox | The isolated environment one session runs in, built by a sandbox backend at an isolation level. | `sandbox.*` |
| What builds the sandbox | sandbox backend | Sandbox-Backend | The program that builds the sandbox (Seatbelt, bubblewrap, Podman or QEMU); it is shown as the second detail next to the isolation level and is never the level's name. | `sandbox.backend` |
| How far the sandbox is separated | isolation level | Isolationsstufe | How the sandbox is separated from your computer; the three levels are listed under "Isolation levels". | `policy`, `namespaces`, `kernel` |
| The weakest level allowed | minimum isolation level | Mindest-Isolationsstufe | The weakest isolation level a session may start with; every setting level can only raise it, never lower it. | `sandbox.min_isolation` |
| The folder and its settings | project | Projekt | A project folder together with the settings and rules chosen for it. | `project`, `humanitl projects` |
| The folder | project folder | Projektordner | The folder on your computer the agent works in; inside a sandbox with namespaces or its own kernel it appears as `/work`, under Seatbelt on macOS at its real path. | `sandbox.work_dir`, `/work` |
| A named file of settings | profile | Profil | A file of settings and rules (`profiles/<name>.toml`) that a session starts with when you name it, for example `--profile llm-only`. | `--profile`, `profiles/` |
| A shipped bundle of settings | preset | Preset | A named bundle of setting values that ships with Humanitl and is chosen at the first start; each value it sets shows that it comes from the preset. Planned with the ADR on the settings model (#80); its names, too, must not promise more than the tool does. | none yet |
<!-- /lang -->

"Sandbox profile" (`profiles/sandbox/<name>.toml`) and "agent adapter" stay
terms of the developer documentation; a user-facing text does not need them.

## Isolation levels

The real levels of the code, with their technical names (owner decision of
2026-11-23). The sandbox backend stands next to the level as a second detail,
for example "Namespaces (shared kernel) · bubblewrap". Each level has one
sentence: what it protects against, what it does not protect against, what
it costs.

<!-- lang: de -->
| Level | en | de | Backends | Protects against / does not protect against / costs |
|---|---|---|---|---|
| `policy` | Kernel policy (macOS, Seatbelt) | Kernel-Richtlinie (macOS, Seatbelt) | Seatbelt | The macOS kernel allows the agent network only to Humanitl (its socket and one loopback port) and to a second loopback port for the agent's own local server; it may read the system trees, but in the private trees `/Users`, `/Volumes`, `/private/tmp`, `/private/var/tmp` and `/private/var/folders` only the project folder, the session folder, the extra folders of the profile and single listed files; it may write only in the project folder (when writable), the session folder and the planned paths, and masked paths are refused; but the agent runs on your kernel and over your real paths, so a flaw in the kernel or in Seatbelt is not contained; it needs no extra install on macOS. |
| `namespaces` | Namespaces (shared kernel; bubblewrap, Podman) | Namensräume (geteilter Kernel; bubblewrap, Podman) | bubblewrap, Podman | The agent gets no network interface except loopback, its own view of the files with only the project folder and the planned mounts, and a seccomp filter that limits which sockets it may open (under bubblewrap: only `SOCK_STREAM` sockets over IPv4 and IPv6, every other family and type refused, the protocol not checked; Podman applies its own profile), but it shares the host's kernel, so a flaw in the kernel is not contained; it needs no virtual machine and starts quickly. |
| `kernel` | Own kernel (micro-VM, QEMU) | Eigener Kernel (Micro-VM, QEMU) | QEMU | The agent runs on its own guest kernel in a micro-VM, so a flaw in the guest kernel does not reach the host directly, but a flaw in QEMU or KVM is not contained, and the level exists only on Linux on x86-64 with `/dev/kvm`; it costs extra packages, more memory and a longer start. |
<!-- /lang -->

The older names "kernel policy over the real paths", "shared kernel, own
namespaces", "own kernel in a micro VM", "plain level" and "kernel level" are
replaced by these three.

## Setting levels

The levels a setting can come from, as a user sees them. The order is from
weakest to strongest; a locked company value is above all of them.

<!-- lang: de -->
| Level | en | de | Meaning | Origin in code |
|---|---|---|---|---|
| default | Default | Standard | The value built into Humanitl. | `Origin::Default` |
| mine | Mine (all projects) | Meins (alle Projekte) | Your own value for every project, kept in your `config.toml`. | `Origin::Global` |
| this project | This project | Dieses Projekt | Your value for one project; a value that comes from the file `.humanitl/profile.toml` in the project's repository carries the mark "from the repository" / „aus dem Repository". | `Origin::ProjectChoice`, `Origin::ProfileProject` |
| this session | Only this session | Nur diese Session | A value for one start that ends with the session. | `Origin::Cli` (`Start.cli_overrides`, the fields of `Start`) |
| company | Company (locked) | Firma (gesperrt) | A value your company fixes; it cannot be changed on any other level. | `Origin::Policy` |
<!-- /lang -->

The environment (`HUMANITL_*`) and command-line flags are no level of their
own. Where they win, the value shows "overridden by environment variable
`HUMANITL_…`" / „überschrieben durch Umgebungsvariable `HUMANITL_…`" or
"overridden by flag `--…`" / „überschrieben durch Flag `--…`". A value from a
profile or a preset carries the mark "from profile NAME" / „aus Profil NAME"
or "from preset NAME" / „aus Preset NAME"; where exactly profiles and presets
sit between the levels is decided by the ADR on the settings model (#80).

## Project terms

The terms of the documentation and the code comments, with the German word
they replace (HUM-086). Sandbox, Diagnostic and Finding stay unchanged.

| en | de | Meaning |
|---|---|---|
| waits for a decision | wartet auf Entscheidung | State of a flow that waits for a human decision; never "intercepted". The identifier stays `held` (see "Concepts") |
| hold | anhalten | Stop a request before it leaves the machine so that it waits for a decision; the verb stays, the state is "waits for a decision". Not to be confused with the gesture "press and hold" (`interceptHoldToBlock`) |
| allow | erlauben, senden | Let a request that waits for a decision go out; also the rule action `allow` |
| block | blockieren | Refuse a request; never "reject" |
| ask | nachfragen | Rule action `ask`: hold the request and ask the human |
| redact | pseudonymisieren (Regelaktion) | Rule action `redact`: pseudonymise the findings before sending |
| pseudonymise | pseudonymisieren | Replace a value with a consistent stand-in; never "anonymise" |
| pseudonym | Pseudonym | The stand-in value that replaces a finding |
| mapping | Zuordnung | The table from pseudonym back to the original value |
| rule | Regel | One entry of `rules.yaml`: match, action, target, duration |
| target | Ziel, Geltungsbereich, Scope | What a remembered decision applies to: exact URL, host, domain, host and method |
| duration | Gültigkeit | How long a rule applies: once, this session, always |
| decision | Entscheidung | The human's answer to a request that waits for a decision |
| request | Anfrage | One outgoing HTTP request of the agent |
| response | Antwort | The upstream answer to a request |
| flow | Fluss, Flow | One request with its response and its state, as the daemon tracks it |
| queue | Warteschlange | The list of flows that wait for a decision |
| history | Verlauf | The recorded flows of the past |
| recording | Aufzeichnung | What the recorder stores about each flow |
| tamper-evident log | manipulationserkennbares Protokoll | The chained, keyed records of decisions and flow events; a check shows changes up to the last anchor, not a cut after it, not events that were never written and not what a dishonest daemon writes. It prevents nothing, and an attacker running as the same user is not covered (`docs/SECURITY.md` section 8, ADR-0008). Replaces "audit chain" and "hash chain" |
| Finding | Fund, Befund | A detected secret or piece of personal data in a request; in reviews, a reported defect |
| secret | Secret, Geheimnis | A credential such as an API key or token |
| personal data | personenbezogene Daten | PII in the sense of the GDPR |
| Diagnostic | Diagnostic, Diagnose | Structured error with code, title, why and fix; never a bare string |
| why | Grund | The field of a Diagnostic that explains the cause |
| fix | Abhilfe | The field of a Diagnostic that tells the human what to do |
| Sandbox | Sandbox | The isolated environment the agent runs in |
| sandbox backend | Sandbox-Backend | The adapter that builds the sandbox: Seatbelt, bubblewrap, Podman or QEMU |
| guarantee | Garantie | One of the three sandbox guarantees in `README.md` |
| escape test | Escape-Test, Ausbruchstest | A test under `tests/escape/` that proves a guarantee holds |
| background service | Hintergrunddienst | The background process `humanitld` that owns proxy, sandbox, rules and recording; "daemon" stays only in identifiers and command names |
| session | Session | One agent running in one sandbox, from its start until it ends |
| passthrough | Durchleitung, Durchreiche | Forwarding of LLM traffic without holding it |
| egress | Egress, Ausgang | The proxy's path to the internet (upstream) |
| airlock | Schleuse | The design direction of the interface (`BACKLOG.md` section 5) |
| timed out | Zeit abgelaufen, Zeitüberschreitung | A flow whose decision deadline passed before a decision; it is blocked |
| project folder | Projektordner | The host directory the agent works in; mounted as `/work` under bubblewrap, Podman and QEMU, at its real path under Seatbelt; never "work directory" or "project directory" |
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
| abgefangen | wartet auf Entscheidung; „anhalten" where the act is meant | Humanitl holds and decides together with the human; "abgefangen" (intercepted) sounds like surveillance |
| Treffer | Fund | A finding is a statement about the request, not a search hit |
| anonymisier | pseudonymisieren | Anonymising is something different in law (BACKLOG.md 5) |
| ablehn | blockieren | The button blocks a request; "ablehnen" (reject) sounds like objecting to a person |
<!-- /lang -->

## Replacement table

The work list of the terminology sweep (#79 decides, a later issue sweeps).
Each row names the old words, the new word from "Concepts", "Isolation
levels" or "Setting levels", what stays, and how often the old words occur
today.

How the numbers were counted, on `main` at `cc6dc233` (2026-11-23): whole
words, case-insensitive, with the English pattern in the English areas and
the German pattern in the German ones. "CLI" counts string literals in
`daemon/bin/humanitl/src` and `daemon/bin/humanitld/src` outside test code,
"Diagnostics" the string literals in
`daemon/crates/core-types/src/diagnostics/codes.rs`, "ARB" the message values
of `app/l10n/app_en.arb` and `app/l10n/app_de.arb` (the `diag…` keys among
them), "Docs en" the English pages under `docs/` with `README.md`, "Docs de"
`docs/translations/de/` with `README.de.md` and the German originals
`docs/CONFIG.md`, `docs/DIAGNOSTICS.md` and `docs/reference/parity.md`. ADRs,
this glossary and `backlog/` are not counted, because they are not swept. The
numbers include identifiers and quoted values that stay (`disposable`,
`humanitl daemon`, `--detach`); they size the work, they are not a list of
places to replace blindly.

<!-- lang: de -->
| # | Old (en) | Old (de) | New (en) | New (de) | Stays | CLI | ARB en | ARB de | Diagnostics | Docs en | Docs de |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | held | angehalten | waits for a decision | wartet auf Entscheidung | `held`, `stateHeld`, `historyChipHeld`, `trayTooltipHeld` as keys; the verb "hold"/„anhalten"; "Hold to block", "Hold to send" (a gesture); "Held to the Seatbelt profile" (`sandboxGuaranteeProfileMacos`, another sense) | 5 | 42 | 37 | 0 | 269 | 138 |
| 2 | hold timeout, hold budget, hold limit | Haltefrist, Haltegrenze | decision deadline; for the size limit "size limit for a decision" | Entscheidungsfrist; für die Größe „Größengrenze für Entscheidungen" | `hold.timeout_secs`, `limits.hold_body_cap_bytes`, `LIMIT_001` | 1 | 3 | 2 | 0 | 6 | 1 |
| 3 | Intercept (name of the view) | Anhalten (rail label) | Decisions | Entscheidungen | `shellNavIntercept`, `features/intercept`, `humanitl intercept`; "Intercept" as the name of a function in Burp | 6 | 7 | 1 | 2 | 67 | – |
| 4 | disposable, disposable sandbox, disposable layer | Wegwerf, Wegwerf-Sandbox, Wegwerf-Schicht | working copy | Arbeitskopie | `sandbox.mode = "disposable"`, `--disposable`, `SANDBOX_030` to `SANDBOX_033` | 6 | 19 | 19 | 13 | 186 | 61 |
| 5 | run (as a noun for a session: "disposable run", "the last run") | Lauf, Wegwerf-Lauf | session; "session with a working copy" | Session; „Session mit Arbeitskopie" | the command `humanitl run`; the verb "run"; "configuration run" of an agent | 6 | 16 | 19 | 9 | 145 | 139 |
| 6 | depot, agent depot | Depot, Agent-Depot | Agents (installed in isolation); as a source "installed in isolation" | Agenten (isoliert installiert); als Quelle „isoliert installiert" | `depot`, `agents.<id>.source`, `humanitl agents`, `AGENT_008` and the other codes | 20 | 40 | 37 | 30 | 243 | 168 |
| 7 | detach | abhängen | keep running in the background | im Hintergrund weiterlaufen lassen | `terminal.on_close = "detach"`, `Ctrl+B d` | 6 | 0 | 0 | 0 | 63 | 7 |
| 8 | attach | anhängen | open again | wieder öffnen | `humanitl sandbox attach`, `startAttach` as a key; "attach" for mounts and files | 11 | 5 | 2 | 2 | 94 | 10 |
| 9 | daemon | Daemon | background service | Hintergrunddienst | `humanitld`, `humanitl daemon …`, the codes `DAEMON_*`, `commonDaemon` as a key | 98 | 82 | 78 | 90 | 1346 | 774 |
| 10 | audit chain, hash chain | Audit-Kette, Kette | tamper-evident log | manipulationserkennbares Protokoll | `audit.*`, `humanitl audit verify`, `AUDIT_*`, `auditChainTitle` as a key | 2 | 5 | 14 | 1 | 48 | 87 |
| 11 | kernel policy over the real paths, policy level | Richtlinie des Kerns | Kernel policy (macOS, Seatbelt) | Kernel-Richtlinie (macOS, Seatbelt) | `policy`, `seatbelt` | 0 | 1 | 1 | 0 | 0 | 1 |
| 12 | shared kernel, own namespaces, plain level | geteilter Kern, eigene Namensräume | Namespaces (shared kernel; bubblewrap, Podman) | Namensräume (geteilter Kernel; bubblewrap, Podman) | `namespaces`, `bwrap`, `podman` | 0 | 2 | 1 | 0 | 7 | 5 |
| 13 | kernel level, own kernel in a micro VM | Kernel-Stufe | Own kernel (micro-VM, QEMU) | Eigener Kernel (Micro-VM, QEMU) | `kernel`, `qemu`, `--kernel` | 0 | 11 | 10 | 16 | 64 | 67 |
| 14 | global (a setting level) | global | Mine (all projects) | Meins (alle Projekte) | `Origin::Global`; "global" in other senses | 1 | 0 | 0 | 0 | 28 | 27 |
| 15 | project profile, project choice, start choice | Projekt-Profil, Projektwahl | This project; for values from the file "from the repository" | Dieses Projekt; für Werte aus der Datei „aus dem Repository" | `.humanitl/profile.toml`, `Origin::ProjectChoice`, `Origin::ProfileProject`, `--project` | 3 | 7 | 3 | 5 | 107 | 105 |
| 16 | company policy, policy lock | Firmenvorgabe, Vorgabe deiner Firma | Company (locked) | Firma (gesperrt) | `Origin::Policy`, `AGENT_010` | 1 | 3 | 3 | 7 | 21 | 29 |
| 17 | – | Sitzung | – | Session | – | – | – | 64 | – | – | 309 |
| 18 | second proxy | zweiter Proxy, Egress-Proxy | decision proxy | Entscheidungs-Proxy | `egress.proxy`, `hold.ask_mode = "upstream"` | 0 | 0 | 0 | 0 | 0 | 5 |
| 19 | work directory, project directory | Arbeitsverzeichnis, Projektverzeichnis | project folder | Projektordner | `sandbox.work_dir`, `/work`, `sandboxWorkDirectory` as a key | 3 | 1 | 1 | 1 | 47 | 40 |
| 20 | session profile | Session-Profil, Sitzungsprofil | profile | Profil | `profiles/`, `--profile`; "sandbox profile" | 1 | 0 | 0 | 0 | 128 | 8 |
| | **Total** | | | | | **170** | **244** | **292** | **176** | **2869** | **1981** |
<!-- /lang -->

Twenty terms, 5732 occurrences. Rows 1, 3, 9, 10 and 19 also change rows of
the table "UI terms" (keys `stateHeld`, `shellNavIntercept`, `commonDaemon`,
`auditChainTitle` and `sandboxWorkDirectory`); the sweep changes such a row in
the same commit as the ARB text, or `make l10n-lint` fails. The terms "local proxy" and
"upstream proxy" have no old word in today's texts; "preset" has no text yet.

## Deviations from the specification

- **"Allow" on the English button.** HUM-052 named "Send". `docs/UX.md`
  4.6 fixes the three places (button "Allow"/"Senden", rule
  "allow"/"Erlauben", strip "Sent to …"/"Gesendet an …") and gives the reason
  for the split; the glossary follows it.
- **"Forbidden words" are word stems.** "ablehn" matches "Ablehnen",
  "anonymisier" matches "Anonymisieren" and "anonymisiert". "abgelehnt" stays
  allowed: there a service turns something away, such as the background
  service a token (`setupTokenInvalidTitle`), and no button is offered as a
  choice.
- **The headings "UI terms" and "Forbidden words"** replace "Begriffe" and
  "Verbotene Wörter" since HUM-086; `app/tool/l10n_lint.dart` looks the tables
  up by these headings.
- **Isolation levels follow the code, not "process sandbox, container,
  micro-VM" (#79).** The specification of #79 named those three. Process
  sandbox and container are one level in the code (`namespaces`, bubblewrap
  and Podman), and macOS has a level of its own (`policy`). The owner decided
  on 2026-11-23 to show the three real levels with their technical names.
- **One project level instead of two (#79).** The project choice held by the
  background service and the file `.humanitl/profile.toml` in the repository
  stay two places in the code; a user sees one level, "This project", and a
  value from the file carries the mark "from the repository" (owner decision
  of 2026-11-23). Which command writes where is decided by the ADR on the
  settings model (#80).
- **The verb "hold" stays (#79).** The specification replaces "held" in
  user-facing text. The state becomes "waits for a decision"; the act of
  stopping a request keeps the verb "hold"/„anhalten", because "stop for a
  decision" names the same act with more words.
- **"Decisions" as the name of the view (#79).** Not named in the
  specification. "Intercept" and „Anhalten" named the view after the act that
  the glossary forbids as a description ("intercepted"); "Decisions" names
  what happens in it.
- **„manipulationserkennbares Protokoll" (#79).** German has no short word for
  "tamper-evident". The compound says what the log does, a change can be
  detected, and nothing more; „manipulationssicher" would promise prevention.
