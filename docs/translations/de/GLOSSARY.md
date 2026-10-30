---
source: GLOSSARY.md
source-sha256: d20688ed2902bff8c071c5207dcf766bf5a5cd2114ab47e675a8434e73012cd9
lang: de
---
# Glossar

Applies to: Humanitl 0.1

Verbindlich für jeden Text in diesem Repository und für jede Übersetzung in
`app/l10n/app_en.arb` (Quelle) und `app/l10n/app_de.arb` (HUM-052, HUM-086).
Wer einen neuen Text schreibt, nimmt die Begriffe von hier; wer einen Begriff
ändern will, ändert ihn zuerst hier und nennt den Grund.

`dart run tool/l10n_lint.dart` (aus `app/`, Teil von `make l10n-lint`) liest
die Tabellen „UI terms" und „Forbidden words" in dieser Datei. Für jede Zeile
der ersten muss der genannte Schlüssel in beiden ARB-Dateien existieren, und
sein Text muss den Begriff der jeweiligen Sprache als ganzes Wort enthalten
(Groß- und Kleinschreibung spielt keine Rolle). Kein deutscher Text darf ein
Wort der zweiten Tabelle enthalten.

## Grundsätze

- Englisch ist die Quellsprache; Deutsch ist gleichwertig, kein Nachgedanke.
- Deutsch spricht den Menschen mit „du" an, nie mit „Sie".
- Protokollbegriffe bleiben englisch: GET, POST, Header, Body, Query, Status,
  Content-Type. Hinweise auf Tastenkürzel sind sprachunabhängig.
- Übersetzt wird nur, was angezeigt wird. Was in Audit-Einträgen, in
  `rules.yaml` oder in `config.toml` landet (`allow`, `block`, `ask`, `redact`,
  die Arten von Funden), bleibt der englische Bezeichner.
- „Session" bleibt „Session", auch im Deutschen, überall dort, wo die laufende
  Session des Agenten gemeint ist.
- Prosa im Repository nutzt britische Schreibung: pseudonymise, behaviour,
  colour, licence (Substantiv). Bezeichner und UI-Texte behalten die Schreibung,
  die sie im Code und in `app_en.arb` haben („Pseudonymize").
- Deutscher Text, der in einem Dokument deutsch bleiben muss (ein Zitat, der
  Inhalt einer Fixture), steht zwischen `<!-- lang: de -->` und `<!-- /lang -->`.

## Projektbegriffe

Die Begriffe der Dokumentation und der Code-Kommentare, mit dem deutschen Wort,
das sie ersetzen (HUM-086). Sandbox, Diagnostic und Finding bleiben unverändert.

| en | de | Bedeutung |
|---|---|---|
| held | angehalten | Zustand eines Flows, der auf eine menschliche Entscheidung wartet; nie „intercepted" (der Bildschirmname „Intercept" ist nur der Name der Ansicht) |
| hold | anhalten | Eine Anfrage stoppen, bevor sie den Rechner verlässt |
| allow | erlauben, senden | Eine angehaltene Anfrage hinauslassen; auch die Regelaktion `allow` |
| block | blockieren | Eine Anfrage verweigern; nie „reject" |
| ask | nachfragen | Regelaktion `ask`: die Anfrage anhalten und den Menschen fragen |
| redact | pseudonymisieren (Regelaktion) | Regelaktion `redact`: die Funde vor dem Senden pseudonymisieren |
| pseudonymise | pseudonymisieren | Einen Wert durch einen gleichbleibenden Platzhalter ersetzen; nie „anonymise" |
| pseudonym | Pseudonym | Der Platzhalterwert, der einen Fund ersetzt |
| mapping | Zuordnung | Die Tabelle vom Pseudonym zurück zum Originalwert |
| rule | Regel | Ein Eintrag in `rules.yaml`: Muster, Aktion, Ziel, Gültigkeit |
| target | Ziel, Geltungsbereich, Scope | Wofür eine gemerkte Entscheidung gilt: genaue URL, Host, Domain, Host und Methode |
| duration | Gültigkeit | Wie lange eine Regel gilt: einmal, diese Session, immer |
| decision | Entscheidung | Die Antwort des Menschen auf eine angehaltene Anfrage |
| request | Anfrage | Eine ausgehende HTTP-Anfrage des Agenten |
| response | Antwort | Die Antwort des Ziels auf eine Anfrage |
| flow | Fluss, Flow | Eine Anfrage mit ihrer Antwort und ihrem Zustand, wie der Daemon sie verfolgt |
| queue | Warteschlange | Die Liste der angehaltenen Flows, die auf eine Entscheidung warten |
| history | Verlauf | Die aufgezeichneten Flows der Vergangenheit |
| recording | Aufzeichnung | Was der Recorder über jeden Flow speichert |
| audit chain | Audit-Kette | Die per Hash verketteten, mit Schlüssel versehenen Audit-Einträge, die Manipulation sichtbar machen; sie schützen nicht vor einem Angreifer, der als derselbe Benutzer läuft, der sowohl den Schlüssel als auch die Datei hält (ADR-0008, `docs/SECURITY.md`) |
| Finding | Fund, Befund | Ein erkanntes Secret oder ein Stück personenbezogener Daten in einer Anfrage; in Reviews ein gemeldeter Defekt |
| secret | Secret, Geheimnis | Ein Zugangsnachweis wie ein API-Schlüssel oder ein Token |
| personal data | personenbezogene Daten | PII im Sinne der DSGVO |
| Diagnostic | Diagnostic, Diagnose | Strukturierter Fehler mit Code, Titel, why und fix; nie ein nackter String |
| why | Grund | Das Feld einer Diagnose, das die Ursache erklärt |
| fix | Abhilfe | Das Feld einer Diagnose, das dem Menschen sagt, was zu tun ist |
| Sandbox | Sandbox | Die isolierte Umgebung, in der der Agent läuft |
| sandbox backend | Sandbox-Backend | Der Adapter, der die Sandbox baut (heute bubblewrap) |
| guarantee | Garantie | Eine der drei Sandbox-Garantien in `README.md` |
| escape test | Escape-Test, Ausbruchstest | Ein Test unter `tests/escape/`, der nachweist, dass eine Garantie hält |
| daemon | Daemon | Der Hintergrundprozess, dem Proxy, Sandbox, Regeln und Aufzeichnung gehören |
| session | Session, Sitzung | Ein Lauf eines Agenten in einer Sandbox |
| passthrough | Durchleitung, Durchreiche | Weiterleiten von LLM-Verkehr, ohne ihn anzuhalten |
| egress | Egress, Ausgang | Der Weg des Proxys ins Internet (Upstream) |
| airlock | Schleuse | Die Designrichtung der Oberfläche (`BACKLOG.md` Abschnitt 5) |
| timed out | Zeit abgelaufen, Zeitüberschreitung | Ein angehaltener Flow, dessen Entscheidungsfrist verstrichen ist |
| project folder | Projektordner | Das Verzeichnis auf dem Rechner, das als `/work` in die Sandbox eingehängt wird |
| acceptance criterion | Akzeptanzkriterium | Eine prüfbare Bedingung, die ein Issue erfüllen muss |
| pitfall | Fallstrick | Eine bekannte Falle, die in einer Issue-Spezifikation genannt ist |
| mutation proof | Mutationsbeweis | Der Nachweis, dass ein Test fehlschlägt, wenn der Code, den er schützt, kaputtgeht |

## UI-Begriffe

| Begriff | en | de | ARB-Schlüssel | Hinweis |
|---|---|---|---|---|
| angehalten (Zustand) | Held | Angehalten | `stateHeld` | nie „abgefangen" |
| anhalten (Verb) | Hold | Anhalten | `commonHold` | |
| senden (Knopf an der Anfrage) | Allow | Senden | `interceptAllowButton` | Der Knopf nennt, was passiert. Englisch „Allow" statt „Send" nach `docs/UX.md` 4.6 |
| erlauben (Regelaktion) | Allow | Erlauben | `rulesActionAllow` | nur in Regeln |
| editiert senden | Send edited version | Editierte Version senden | `editorSend` | |
| blockieren (Knopf) | Block | Blockieren | `interceptBlockButton` | nicht „Ablehnen" |
| blockieren (Regelaktion) | Block | Blockieren | `rulesActionBlock` | |
| nachfragen (Regelaktion) | Ask | Nachfragen | `rulesActionAsk` | |
| pseudonymisieren (Regelaktion) | Redact | Pseudonymisieren | `rulesActionRedact` | Die Aktion `redact` bleibt im YAML englisch |
| Zeit abgelaufen | Timed out | Zeit abgelaufen | `stateTimedOut` | |
| durch Regel automatisch erlaubt | Allowed by rule | Durch Regel erlaubt | `stateAllowedByRule` | |
| LLM-Durchleitung | LLM passthrough | LLM-Durchleitung | `statePassthroughLlm` | |
| editiert | Edited | Editiert | `historyChipEdited` | Chip |
| Fund | Finding | Fund | `commonFinding` | nicht „Treffer" |
| Secret | Secret | Secret | `commonSecret` | bleibt |
| PII | Personal data | Personenbezogene Daten | `commonPersonalData` | |
| pseudonymisieren | Pseudonymize | Pseudonymisieren | `commonPseudonymize` | nie „Anonymisieren"; Prosa schreibt „pseudonymise" |
| Pseudonym | Pseudonym | Pseudonym | `editorMappingPseudonym` | |
| Zuordnung | Mapping | Zuordnung | `editorMapping` | Bereichstitel „Zuordnung (3)" |
| alle ersetzen | Replace all with pseudonyms | Alle durch Pseudonyme ersetzen | `editorReplaceAll` | |
| ignorieren (einmal) | Ignore | Ignorieren | `editorFindingIgnore` | |
| immer ignorieren | Always ignore | Immer ignorieren | `editorFindingIgnoreAlways` | |
| trotzdem senden | Send anyway | Trotzdem senden | `commonSendAnyway` | |
| Regel | Rule | Regel | `rulesEditorTitle` | |
| merken | Remember | Merken | `interceptRemember` | |
| Geltungsbereich (Ziel) | Target | Ziel | `interceptScopeTarget` | |
| Geltungsbereich (Gültigkeit) | Duration | Gültigkeit | `interceptScopeDuration` | |
| einmal | Once | Einmal | `interceptDurationOnce` | |
| diese Session | This session | Diese Session | `rulesExpirySession` | „Session" bleibt |
| für immer | Always | Immer | `rulesExpiryAlways` | |
| genaue URL | Exact URL | Genaue URL | `rulesTargetExactUrl` | |
| Host | Host | Host | `interceptTargetHost` | |
| Domain (Apex und Subdomains) | Domain and subdomains | Domain und Subdomains | `rulesTargetDomain` | |
| Host und Methode | Host and method | Host und Methode | `rulesTargetHostMethod` | |
| Warteschlange | Queue | Warteschlange | `interceptQueueTitle` | |
| Verlauf | History | Verlauf | `shellNavHistory` | |
| Anhalten (Bildschirmname) | Intercept | Anhalten | `shellNavIntercept` | Beschriftung in der Leiste |
| Sandbox | Sandbox | Sandbox | `shellNavSandbox` | |
| Isolationsprüfung | Isolation check | Isolationsprüfung | `sandboxIsolationCheck` | |
| kein Netzwerk-Interface | No network interface | Kein Netzwerk-Interface | `sandboxGuaranteeNoInterface` | |
| ein Socket | Exactly one socket, to Humanitl | Genau ein Socket, zu Humanitl | `sandboxGuaranteeOneSocket` | |
| seccomp aktiv | New sockets forbidden (seccomp) | Neue Sockets verboten (seccomp) | `sandboxGuaranteeSeccomp` | |
| Projektordner | Project folder | Projektordner | `setupCheckProject` | |
| Arbeitsverzeichnis | Work directory | Arbeitsverzeichnis | `sandboxWorkDirectory` | `/work` bleibt |
| Agent wartet | The agent is waiting for you | Der Agent wartet auf dich | `commonAgentWaiting` | „du"-Form |
| Daemon | Daemon | Daemon | `commonDaemon` | |
| Audit-Kette | Audit chain | Audit-Kette | `auditChainTitle` | |
| verifiziert | Verified | Verifiziert | `auditStatusOk` | |
| gebrochen ab | Broken at sequence {seq} | Gebrochen ab Sequenz {seq} | `auditBrokenAtSequence` | |
| Aufbewahrung | Retention | Aufbewahrung | `auditRetentionTitle` | |
| Einstellungsstufe grundlegend | Basic | Grundlegend | `settingsTierBasic` | |
| Einstellungsstufe erweitert | Advanced | Erweitert | `settingsTierAdvanced` | |
| Einstellungsstufe Experte | Expert | Experte | `settingsTierExpert` | |
| Herkunft (einer Einstellung) | Source | Herkunft | `settingsOrigin` | |
| zurücksetzen | Reset to default | Auf Standard zurücksetzen | `settingsResetToDefault` | |

## Verbotene Wörter

| Wort | stattdessen | Grund |
|---|---|---|
| abgefangen | angehalten | Humanitl hält an und entscheidet gemeinsam mit dem Menschen; „abgefangen" (intercepted) klingt nach Überwachung |
| Treffer | Fund | Ein Fund ist eine Aussage über die Anfrage, kein Ergebnis einer Suche |
| anonymisier | pseudonymisieren | Anonymisieren ist rechtlich etwas anderes (BACKLOG.md 5) |
| ablehn | blockieren | Der Knopf blockiert eine Anfrage; „ablehnen" (reject) klingt nach einem Einwand gegen eine Person |

## Abweichungen von der Spezifikation

- **„Allow" auf dem englischen Knopf.** HUM-052 nannte „Send". `docs/UX.md`
  4.6 legt die drei Stellen fest (Knopf „Allow"/„Senden", Regel
  „allow"/„Erlauben", Streifen „Sent to …"/„Gesendet an …") und begründet die
  Trennung; das Glossar folgt ihm.
- **„Forbidden words" sind Wortstämme.** „ablehn" trifft „Ablehnen",
  „anonymisier" trifft „Anonymisieren" und „anonymisiert". „abgelehnt" bleibt
  erlaubt: Dort weist ein Dienst etwas zurück, etwa der Daemon ein Token
  (`setupTokenInvalidTitle`), und es wird kein Knopf zur Wahl angeboten.
- **Die Überschriften „UI terms" und „Forbidden words"** ersetzen seit HUM-086
  „Begriffe" und „Verbotene Wörter"; `app/tool/l10n_lint.dart` sucht die
  Tabellen über diese Überschriften.
