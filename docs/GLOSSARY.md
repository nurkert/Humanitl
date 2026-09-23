# Glossar der Oberfläche

Verbindlich für jede Übersetzung in `app/l10n/app_en.arb` (Quelle) und
`app/l10n/app_de.arb` (HUM-052). Wer einen neuen Text schreibt, nimmt die
Begriffe von hier; wer einen Begriff ändern will, ändert ihn zuerst hier und
begründet es.

`dart run tool/l10n_lint.dart` (aus `app/`, Teil von `make l10n-lint`) liest
beide Tabellen dieser Datei. Für jede Zeile der ersten muss der genannte
Schlüssel in beiden ARB-Dateien stehen, und sein Text muss den Begriff der
jeweiligen Sprache als ganzes Wort enthalten (Groß- und Kleinschreibung
zählen nicht). Kein deutscher Text darf ein Wort der zweiten Tabelle
enthalten.

## Grundsätze

- Englisch ist die Quellsprache, Deutsch ist erstklassig und nicht nachgereicht.
- Deutsch spricht den Menschen mit „du“ an, nie mit „Sie“.
- Protokollbegriffe bleiben Englisch: GET, POST, Header, Body, Query, Status,
  Content-Type. Tastenkürzel-Hinweise sind sprachunabhängig.
- Nur Anzeige wird übersetzt. Was in Audit-Records, `rules.yaml` oder
  `config.toml` landet (`allow`, `block`, `ask`, `redact`, die Arten der Funde),
  bleibt der englische Bezeichner.
- „Session“ bleibt „Session“, auch im Deutschen, überall dort, wo die laufende
  Sitzung des Agenten gemeint ist.

## Begriffe

| Begriff | en | de | ARB-Schlüssel | Anmerkung |
|---|---|---|---|---|
| held (state) | Held | Angehalten | `stateHeld` | nie „abgefangen“ |
| hold (verb) | Hold | Anhalten | `commonHold` | |
| allow (button on request) | Allow | Senden | `interceptAllowButton` | Der Knopf nennt, was passiert. Englisch „Allow“ statt „Send“ nach `docs/UX.md` 4.6 |
| allow (rule action) | Allow | Erlauben | `rulesActionAllow` | nur in Regeln |
| allow edited | Send edited version | Editierte Version senden | `editorSend` | |
| block (button) | Block | Blockieren | `interceptBlockButton` | nicht „Ablehnen“ |
| block (rule action) | Block | Blockieren | `rulesActionBlock` | |
| ask (rule action) | Ask | Nachfragen | `rulesActionAsk` | |
| redact (rule action) | Redact | Pseudonymisieren | `rulesActionRedact` | Die Aktion `redact` bleibt im YAML englisch |
| timed out | Timed out | Zeit abgelaufen | `stateTimedOut` | |
| auto-allowed by rule | Allowed by rule | Durch Regel erlaubt | `stateAllowedByRule` | |
| LLM passthrough | LLM passthrough | LLM-Durchleitung | `statePassthroughLlm` | |
| edited | Edited | Editiert | `historyChipEdited` | Chip |
| finding | Finding | Fund | `commonFinding` | nicht „Treffer“ |
| secret | Secret | Secret | `commonSecret` | bleibt |
| PII | Personal data | Personenbezogene Daten | `commonPersonalData` | |
| pseudonymize | Pseudonymize | Pseudonymisieren | `commonPseudonymize` | nie „Anonymisieren“ |
| pseudonym | Pseudonym | Pseudonym | `editorMappingPseudonym` | |
| mapping | Mapping | Zuordnung | `editorMapping` | Panel-Titel „Zuordnung (3)“ |
| replace all | Replace all with pseudonyms | Alle durch Pseudonyme ersetzen | `editorReplaceAll` | |
| ignore (once) | Ignore | Ignorieren | `editorFindingIgnore` | |
| ignore always | Always ignore | Immer ignorieren | `editorFindingIgnoreAlways` | |
| send anyway | Send anyway | Trotzdem senden | `commonSendAnyway` | |
| rule | Rule | Regel | `rulesEditorTitle` | |
| remember | Remember | Merken | `interceptRemember` | |
| scope (target) | Target | Ziel | `interceptScopeTarget` | |
| scope (duration) | Duration | Gültigkeit | `interceptScopeDuration` | |
| once | Once | Einmal | `interceptDurationOnce` | |
| this session | This session | Diese Session | `rulesExpirySession` | „Session“ bleibt |
| forever | Always | Immer | `rulesExpiryAlways` | |
| exact URL | Exact URL | Genaue URL | `rulesTargetExactUrl` | |
| host | Host | Host | `interceptTargetHost` | |
| domain (apex + subs) | Domain and subdomains | Domain und Subdomains | `rulesTargetDomain` | |
| host + method | Host and method | Host und Methode | `rulesTargetHostMethod` | |
| queue | Queue | Warteschlange | `interceptQueueTitle` | |
| history | History | Verlauf | `shellNavHistory` | |
| intercept (screen name) | Intercept | Anhalten | `shellNavIntercept` | Rail-Label |
| sandbox | Sandbox | Sandbox | `shellNavSandbox` | |
| isolation check | Isolation check | Isolationsprüfung | `sandboxIsolationCheck` | |
| no network interface | No network interface | Kein Netzwerk-Interface | `sandboxGuaranteeNoInterface` | |
| one socket | Exactly one socket, to Humanitl | Genau ein Socket, zu Humanitl | `sandboxGuaranteeOneSocket` | |
| seccomp active | New sockets forbidden (seccomp) | Neue Sockets verboten (seccomp) | `sandboxGuaranteeSeccomp` | |
| project folder | Project folder | Projektordner | `setupCheckProject` | |
| work dir | Work directory | Arbeitsverzeichnis | `sandboxWorkDirectory` | `/work` bleibt |
| agent is waiting | The agent is waiting for you | Der Agent wartet auf dich | `commonAgentWaiting` | Du-Form |
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

## Verbotene Wörter

| Wort | statt | Grund |
|---|---|---|
| abgefangen | angehalten | Humanitl hält an und entscheidet mit dem Menschen; „abgefangen“ klingt nach Überwachung |
| Treffer | Fund | Ein Fund ist ein Befund über die Anfrage, kein Suchergebnis |
| anonymisier | pseudonymisieren | Anonymisieren ist rechtlich etwas anderes (BACKLOG.md 5) |
| ablehn | blockieren | Der Knopf blockiert eine Anfrage; „ablehnen“ klingt nach Widerspruch gegen eine Person |

## Abweichungen von der Spezifikation

- **„Allow“ auf dem englischen Knopf.** HUM-052 nannte „Send“. `docs/UX.md`
  4.6 legt die drei Orte fest (Knopf „Allow“/„Senden“, Regel
  „allow“/„Erlauben“, Streifen „Sent to …“/„Gesendet an …“) und begründet den
  Split; das Glossar folgt dem.
- **„Verbotene Wörter“ sind Wortanfänge.** „ablehn“ trifft „Ablehnen“,
  „anonymisier“ trifft „Anonymisieren“ und „anonymisiert“. „abgelehnt“ bleibt
  erlaubt: Dort weist ein Dienst etwas zurück, etwa der Daemon ein Token
  (`setupTokenInvalidTitle`), und kein Knopf steht zur Wahl.
