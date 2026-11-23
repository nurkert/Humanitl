---
source: GLOSSARY.md
source-sha256: 61b734244a61c081b7474bc750b2af89a07ca00c9f10218f2fa9f0f2b1e5ac43
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

Seit Sprint 9 (#79) hat jeder Begriff, dem ein Nutzer begegnet, genau ein Wort
je Sprache, dasselbe in der Kommandozeile, den Terminal-Ansichten, der App, der
Dokumentation und den Diagnosen. Die Abschnitte „Begriffe",
„Isolationsstufen" und „Einstellungsebenen" legen diese Wörter fest. Wo die
heutigen Texte noch ein älteres Wort benutzen, nennt es die
[Ersetzungstabelle](#ersetzungstabelle); sie ist die Arbeitsliste der
Begriffsbereinigung, die als eigenes Issue folgt. Bis diese Bereinigung landet,
beschreibt die Tabelle „UI terms" die ARB-Dateien, wie sie sind, und die
Bereinigung ändert dort eine Zeile zusammen mit ihrem Schlüssel.

## Grundsätze

- Englisch ist die Quellsprache; Deutsch ist gleichwertig, kein Nachgedanke.
- Deutsch spricht den Menschen mit „du" an, nie mit „Sie".
- Protokollbegriffe bleiben englisch: GET, POST, Header, Body, Query, Status,
  Content-Type. Hinweise auf Tastenkürzel sind sprachunabhängig.
- Übersetzt wird nur, was angezeigt wird. Was in Audit-Einträgen, in
  `rules.yaml` oder in `config.toml` landet (`allow`, `block`, `ask`, `redact`,
  die Arten von Funden), bleibt der englische Bezeichner.
- „Session" bleibt „Session", auch im Deutschen, überall dort, wo die laufende
  Session des Agenten gemeint ist. „Sitzung" wird nicht benutzt.
- Ein Begriff, ein Wort. Es gibt keinen zweiten, einfacheren Wortschatz für
  Einsteiger: Die Wörter unten sollen zugleich schlicht und genau sein. Ein
  Text, der ein Wort für einen hier genannten Begriff braucht, nimmt dieses
  Wort und kein Synonym.
- Wörter für Nutzer sind keine Bezeichner. Konfigurationsschlüssel, ihre Werte,
  Protokollfelder, RPC-Namen, Befehlsnamen und Diagnose-Codes behalten ihre
  Namen (`held`, `disposable`, `depot`, `daemon`, `humanitl sandbox attach`).
  Ein Text darf einen Bezeichner in Backticks neben das Wort für Nutzer
  stellen, etwa „Hintergrunddienst (`humanitld`)".
- Kein Wort verspricht mehr, als das Werkzeug tut. Humanitl heißt nie „safe",
  „secure", „sicher" oder „tamper-proof"/„manipulationssicher", und nichts ist
  „garantiert" außer den drei Sandbox-Garantien in `README.md`. Die
  Audit-Einträge zeigen Änderungen; sie verhindern sie nicht.
- ADRs und die Sprint-Dateien unter `backlog/` sind Geschichte und behalten die
  Wörter ihrer Zeit. Alles andere folgt diesem Glossar.
- Prosa im Repository nutzt britische Schreibung: pseudonymise, behaviour,
  colour, licence (Substantiv). Bezeichner und UI-Texte behalten die Schreibung,
  die sie im Code und in `app_en.arb` haben („Pseudonymize").
- Deutscher Text, der in einem Dokument deutsch bleiben muss (ein Zitat, der
  Inhalt einer Fixture), steht zwischen `<!-- lang: de -->` und `<!-- /lang -->`.

## Begriffe

Ein Wort je Begriff, entschieden für Sprint 9 (#79, Entscheidungen des Owners
vom 2026-11-23). Die Spalte „Bezeichner" nennt, was in Code, Konfiguration und
Protokoll bleibt, wie es ist.

| Begriff | en | de | Bedeutung | Bezeichner |
|---|---|---|---|---|
| Eine Anfrage, die auf einen Menschen wartet | waits for a decision | wartet auf Entscheidung | Humanitl hat die Anfrage gestoppt, bevor sie den Rechner verließ, und sie geht erst hinaus, wenn ein Mensch sie erlaubt; läuft die Frist ab, wird sie blockiert, nie erlaubt. | `held`, `FlowState::Held`, `hold.*` |
| Die Zeit, die eine Anfrage wartet | decision deadline | Entscheidungsfrist | Wie lange eine Anfrage auf eine Entscheidung wartet, bevor Humanitl sie blockiert. | `hold.timeout_secs` |
| Die Ansicht mit den wartenden Anfragen | Decisions | Entscheidungen | Die Ansicht, in der die Anfragen stehen, die auf eine Entscheidung warten, und in der sie entschieden werden. | `shellNavIntercept`, `features/intercept` |
| Änderungen gehen zuerst in eine Kopie | working copy; als Wahl "Working copy – review changes first" | Arbeitskopie; als Wahl „Arbeitskopie – Änderungen erst prüfen" | Der Agent schreibt in eine Kopie des Projektordners, und nichts erreicht den Projektordner, bevor du die Änderungen geprüft und übernommen hast. Die Vorgabe `auto` gibt eine Arbeitskopie, wo das System es kann, und schreibt sonst direkt, mit einem Hinweis (`SANDBOX_033`). | `sandbox.mode = "disposable"` (Vorgabe `auto`), `--disposable` |
| Änderungen gehen in den Ordner | direct; als Wahl "Direct into the project folder" | direkt; als Wahl „Direkt in den Projektordner" | Der Agent schreibt in den Projektordner selbst, danach gibt es also nichts zu prüfen. | `sandbox.mode = "direct"`, `humanitl run --direct`, `humanitl go --rw` |
| Agentenprogramme getrennt vom Rechner | Agents (installed in isolation) | Agenten (isoliert installiert) | Humanitl installiert, aktualisiert und meldet diese Agentenprogramme in einer Sandbox an, getrennt von den Programmen dieses Rechners; der Befehl ist `humanitl agents`. | `depot`, `agents.<id>.source = "depot"` |
| Woher ein Agentenprogramm kommt | installed in isolation; on this computer | isoliert installiert; auf diesem Rechner | Die zwei Quellen eines Agentenprogramms: die Kopie, die Humanitl isoliert installiert hat, oder das Programm, das auf diesem Rechner schon installiert ist. | `source = "depot"`, `source = "host"` |
| Eine Session verlassen, ohne sie zu beenden | keep running in the background | im Hintergrund weiterlaufen lassen | Fenster oder Terminal schließen sich, und der Agent arbeitet weiter; seine Anfragen warten weiter auf eine Entscheidung und werden blockiert, wenn ihre Frist abläuft. So verhalten sich die App und `humanitl go` mit `terminal.on_close = "detach"` (der Vorgabe); schließt sich das Terminal von `humanitl run`, endet seine Session. | `detach`, `terminal.on_close = "detach"` |
| Zu so einer Session zurückkehren | open again | wieder öffnen | Zeigt eine Session, die im Hintergrund läuft, wieder in einem Terminal oder Fenster. | `attach`, `humanitl sandbox attach` |
| Der Prozess, der die Arbeit macht | background service | Hintergrunddienst | Das Programm `humanitld`, das für deinen Nutzer den lokalen Proxy, die Sandboxen, die Regeln und die Aufzeichnung betreibt, auch wenn kein Fenster offen ist. | `daemon`, `humanitld`, `humanitl daemon …` |
| Die Audit-Einträge | tamper-evident log | manipulationserkennbares Protokoll | Entscheidungen und Ereignisse der Flows werden aufgezeichnet, verkettet und mit Schlüssel versehen, sodass eine Prüfung zeigt, ob Einträge bis zum letzten Anker geändert, entfernt oder umgestellt wurden; sie zeigt keinen Schnitt am Ende hinter dem letzten Anker, keine Ereignisse, die nie geschrieben wurden (`audit.events_dropped`), und nicht, was ein unehrlicher Hintergrunddienst schreibt, sie verhindert nichts, und sie hält nicht gegen einen Angreifer, der als dein Nutzer läuft (`docs/SECURITY.md` Abschnitt 8, ADR-0008). | `audit`, `audit.*`, `humanitl audit verify` |
| Proxy 1 | local proxy | lokaler Proxy | Der Proxy im Hintergrunddienst; er ist der einzige Weg aus der Sandbox, und jede Anfrage des Agenten geht durch ihn. | `proxy.sock` |
| Proxy 2 | upstream proxy | Upstream-Proxy | Ein Proxy deines Netzes, etwa ein Firmen-Proxy, über den erlaubte Anfragen hinausgehen müssen; er entscheidet nichts für Humanitl und ist noch nicht gebaut (ADR-0017). Der Bezeichner `upstream` in `hold.ask_mode` und in den Quellen einer Aufzeichnung meint den Entscheidungs-Proxy, nicht diesen. | noch keine |
| Proxy 3 | decision proxy | Entscheidungs-Proxy | Dein eigener abfangender Proxy hinter Humanitl, etwa Burp oder mitmproxy, durch den erlaubte Anfragen gehen und in dem, mit dem Fragemodus `upstream`, unentschiedene Anfragen entschieden werden. | `egress.proxy`, `hold.ask_mode = "upstream"` |
| Ein Agent bei der Arbeit | session | Session | Ein Agent, der in einer Sandbox läuft, vom Start bis zum Ende; er kann im Hintergrund weiterlaufen. Eine Session heißt nie „Lauf"; `humanitl run` bleibt der Name eines Befehls. | `session`, `humanitl sessions` |
| Wo eine Session läuft | sandbox | Sandbox | Die isolierte Umgebung, in der eine Session läuft, gebaut von einem Sandbox-Backend auf einer Isolationsstufe. | `sandbox.*` |
| Was die Sandbox baut | sandbox backend | Sandbox-Backend | Das Programm, das die Sandbox baut (Seatbelt, bubblewrap, Podman oder QEMU); es steht als zweite Angabe neben der Isolationsstufe und ist nie ihr Name. | `sandbox.backend` |
| Wie weit die Sandbox getrennt ist | isolation level | Isolationsstufe | Wie die Sandbox von deinem Rechner getrennt ist; die drei Stufen stehen unter „Isolationsstufen". | `policy`, `namespaces`, `kernel` |
| Die schwächste erlaubte Stufe | minimum isolation level | Mindest-Isolationsstufe | Die schwächste Isolationsstufe, mit der eine Session starten darf; jede Einstellungsebene kann sie nur heben, nie senken. | `sandbox.min_isolation` |
| Der Ordner und seine Einstellungen | project | Projekt | Ein Projektordner zusammen mit den Einstellungen und Regeln, die für ihn gewählt sind. | `project`, `humanitl projects` |
| Der Ordner | project folder | Projektordner | Der Ordner auf deinem Rechner, in dem der Agent arbeitet; in einer Sandbox mit Namensräumen oder eigenem Kernel erscheint er als `/work`, unter Seatbelt auf macOS unter seinem echten Pfad. | `sandbox.work_dir`, `/work` |
| Eine benannte Datei mit Einstellungen | profile | Profil | Eine Datei mit Einstellungen und Regeln (`profiles/<name>.toml`), mit der eine Session startet, wenn du sie nennst, etwa `--profile llm-only`. | `--profile`, `profiles/` |
| Ein mitgeliefertes Bündel von Einstellungen | preset | Preset | Ein benanntes Bündel von Einstellungswerten, das mit Humanitl kommt und beim ersten Start gewählt wird; jeder Wert, den es setzt, zeigt, dass er aus dem Preset kommt. Geplant mit dem ADR zum Einstellungsmodell (#80); auch seine Namen dürfen nicht mehr versprechen, als das Werkzeug tut. | noch keine |

„Sandbox-Profil" (`profiles/sandbox/<name>.toml`) und „Agent-Adapter" bleiben
Begriffe der Entwicklerdokumentation; ein Text für Nutzer braucht sie nicht.

## Isolationsstufen

Die wirklichen Stufen des Codes, mit ihren technischen Namen (Entscheidung des
Owners vom 2026-11-23). Das Sandbox-Backend steht als zweite Angabe neben der
Stufe, etwa „Namensräume (geteilter Kernel) · bubblewrap". Jede Stufe hat einen
Satz: wovor sie schützt, wovor sie nicht schützt, was sie kostet.

| Stufe | en | de | Backends | Schützt vor / schützt nicht vor / kostet |
|---|---|---|---|---|
| `policy` | Kernel policy (macOS, Seatbelt) | Kernel-Richtlinie (macOS, Seatbelt) | Seatbelt | Der macOS-Kernel erlaubt dem Agenten Netz nur zu Humanitl (dessen Socket und ein Loopback-Port) und zu einem zweiten Loopback-Port für den eigenen lokalen Server des Agenten; er darf die Systembäume lesen, in den privaten Bäumen `/Users`, `/Volumes`, `/private/tmp`, `/private/var/tmp` und `/private/var/folders` aber nur den Projektordner, das Sitzungsverzeichnis, die zusätzlichen Ordner des Profils und einzeln genannte Dateien; schreiben darf er nur im Projektordner (wenn er schreibbar ist), im Sitzungsverzeichnis und auf den geplanten Pfaden, und maskierte Pfade sind verweigert; aber der Agent läuft auf deinem Kernel und über deinen echten Pfaden, sodass ein Fehler im Kernel oder in Seatbelt nicht eingedämmt ist; unter macOS braucht sie keine zusätzliche Installation. |
| `namespaces` | Namespaces (shared kernel; bubblewrap, Podman) | Namensräume (geteilter Kernel; bubblewrap, Podman) | bubblewrap, Podman | Der Agent bekommt keine Netzschnittstelle außer Loopback, eine eigene Sicht auf die Dateien mit nur dem Projektordner und den geplanten Einhängungen und einen seccomp-Filter, der begrenzt, welche Sockets er öffnen darf (unter bubblewrap: nur `SOCK_STREAM`-Sockets über IPv4 und IPv6, jede andere Familie und Art verweigert, das Protokoll ungeprüft; Podman setzt sein eigenes Profil), aber er teilt den Kernel des Hosts, sodass ein Fehler im Kernel nicht eingedämmt ist; sie braucht keine virtuelle Maschine und startet schnell. |
| `kernel` | Own kernel (micro-VM, QEMU) | Eigener Kernel (Micro-VM, QEMU) | QEMU | Der Agent läuft auf einem eigenen Gastkernel in einer Micro-VM, sodass ein Fehler im Gastkernel den Host nicht direkt erreicht, aber ein Fehler in QEMU oder KVM ist nicht eingedämmt, und die Stufe gibt es nur unter Linux auf x86-64 mit `/dev/kvm`; sie kostet zusätzliche Pakete, mehr Arbeitsspeicher und einen längeren Start. |

Die älteren Namen „kernel policy over the real paths", „shared kernel, own
namespaces", „own kernel in a micro VM", „plain level" und „kernel level" gehen
in diesen drei auf.

## Einstellungsebenen

Die Ebenen, aus denen eine Einstellung kommen kann, so wie ein Nutzer sie
sieht. Die Reihenfolge geht von der schwächsten zur stärksten; ein gesperrter
Wert der Firma steht über allen.

| Ebene | en | de | Bedeutung | Herkunft im Code |
|---|---|---|---|---|
| Standard | Default | Standard | Der Wert, der in Humanitl eingebaut ist. | `Origin::Default` |
| Meins | Mine (all projects) | Meins (alle Projekte) | Dein eigener Wert für jedes Projekt, in deiner `config.toml`. | `Origin::Global` |
| Dieses Projekt | This project | Dieses Projekt | Dein Wert für ein Projekt; ein Wert aus der Datei `.humanitl/profile.toml` im Repository des Projekts trägt die Marke "from the repository" / „aus dem Repository". | `Origin::ProjectChoice`, `Origin::ProfileProject` |
| Diese Session | Only this session | Nur diese Session | Ein Wert für einen Start, der mit der Session endet. | `Origin::Cli` (`Start.cli_overrides`, die Felder von `Start`) |
| Firma | Company (locked) | Firma (gesperrt) | Ein Wert, den deine Firma festlegt; auf keiner anderen Ebene lässt er sich ändern. | `Origin::Policy` |

Umgebung (`HUMANITL_*`) und Flags der Kommandozeile sind keine eigene Ebene. Wo
sie gewinnen, zeigt der Wert "overridden by environment variable
`HUMANITL_…`" / „überschrieben durch Umgebungsvariable `HUMANITL_…`" oder
"overridden by flag `--…`" / „überschrieben durch Flag `--…`". Ein Wert aus
einem Profil oder Preset trägt die Marke "from profile NAME" / „aus Profil
NAME" oder "from preset NAME" / „aus Preset NAME"; wo genau Profile und Presets
zwischen den Ebenen liegen, entscheidet das ADR zum Einstellungsmodell (#80).

## Projektbegriffe

Die Begriffe der Dokumentation und der Code-Kommentare, mit dem deutschen Wort,
das sie ersetzen (HUM-086). Sandbox, Diagnostic und Finding bleiben unverändert.

| en | de | Bedeutung |
|---|---|---|
| waits for a decision | wartet auf Entscheidung | Zustand eines Flows, der auf eine menschliche Entscheidung wartet; nie „intercepted". Der Bezeichner bleibt `held` (siehe „Begriffe") |
| hold | anhalten | Eine Anfrage stoppen, bevor sie den Rechner verlässt, sodass sie auf eine Entscheidung wartet; das Verb bleibt, der Zustand heißt „wartet auf Entscheidung". Nicht zu verwechseln mit der Geste „gedrückt halten" (`interceptHoldToBlock`) |
| allow | erlauben, senden | Eine Anfrage, die auf eine Entscheidung wartet, hinauslassen; auch die Regelaktion `allow` |
| block | blockieren | Eine Anfrage verweigern; nie „reject" |
| ask | nachfragen | Regelaktion `ask`: die Anfrage anhalten und den Menschen fragen |
| redact | pseudonymisieren (Regelaktion) | Regelaktion `redact`: die Funde vor dem Senden pseudonymisieren |
| pseudonymise | pseudonymisieren | Einen Wert durch einen gleichbleibenden Platzhalter ersetzen; nie „anonymise" |
| pseudonym | Pseudonym | Der Platzhalterwert, der einen Fund ersetzt |
| mapping | Zuordnung | Die Tabelle vom Pseudonym zurück zum Originalwert |
| rule | Regel | Ein Eintrag in `rules.yaml`: Muster, Aktion, Ziel, Gültigkeit |
| target | Ziel, Geltungsbereich, Scope | Wofür eine gemerkte Entscheidung gilt: genaue URL, Host, Domain, Host und Methode |
| duration | Gültigkeit | Wie lange eine Regel gilt: einmal, diese Session, immer |
| decision | Entscheidung | Die Antwort des Menschen auf eine Anfrage, die auf eine Entscheidung wartet |
| request | Anfrage | Eine ausgehende HTTP-Anfrage des Agenten |
| response | Antwort | Die Antwort des Ziels auf eine Anfrage |
| flow | Fluss, Flow | Eine Anfrage mit ihrer Antwort und ihrem Zustand, wie der Daemon sie verfolgt |
| queue | Warteschlange | Die Liste der Flows, die auf eine Entscheidung warten |
| history | Verlauf | Die aufgezeichneten Flows der Vergangenheit |
| recording | Aufzeichnung | Was der Recorder über jeden Flow speichert |
| tamper-evident log | manipulationserkennbares Protokoll | Die verketteten, mit Schlüssel versehenen Einträge zu Entscheidungen und Ereignissen der Flows; eine Prüfung zeigt Änderungen bis zum letzten Anker, keinen Schnitt dahinter, keine Ereignisse, die nie geschrieben wurden, und nicht, was ein unehrlicher Daemon schreibt. Sie verhindert nichts, und ein Angreifer, der als derselbe Benutzer läuft, ist nicht erfasst (`docs/SECURITY.md` Abschnitt 8, ADR-0008). Ersetzt „audit chain" und „hash chain" |
| Finding | Fund, Befund | Ein erkanntes Secret oder ein Stück personenbezogener Daten in einer Anfrage; in Reviews ein gemeldeter Defekt |
| secret | Secret, Geheimnis | Ein Zugangsnachweis wie ein API-Schlüssel oder ein Token |
| personal data | personenbezogene Daten | PII im Sinne der DSGVO |
| Diagnostic | Diagnostic, Diagnose | Strukturierter Fehler mit Code, Titel, why und fix; nie ein nackter String |
| why | Grund | Das Feld einer Diagnose, das die Ursache erklärt |
| fix | Abhilfe | Das Feld einer Diagnose, das dem Menschen sagt, was zu tun ist |
| Sandbox | Sandbox | Die isolierte Umgebung, in der der Agent läuft |
| sandbox backend | Sandbox-Backend | Der Adapter, der die Sandbox baut: Seatbelt, bubblewrap, Podman oder QEMU |
| guarantee | Garantie | Eine der drei Sandbox-Garantien in `README.md` |
| escape test | Escape-Test, Ausbruchstest | Ein Test unter `tests/escape/`, der nachweist, dass eine Garantie hält |
| background service | Hintergrunddienst | Der Hintergrundprozess `humanitld`, dem Proxy, Sandbox, Regeln und Aufzeichnung gehören; „daemon" bleibt nur in Bezeichnern und Befehlsnamen |
| session | Session | Ein Agent, der in einer Sandbox läuft, vom Start bis zum Ende |
| passthrough | Durchleitung, Durchreiche | Weiterleiten von LLM-Verkehr, ohne ihn anzuhalten |
| egress | Egress, Ausgang | Der Weg des Proxys ins Internet (Upstream) |
| airlock | Schleuse | Die Designrichtung der Oberfläche (`BACKLOG.md` Abschnitt 5) |
| timed out | Zeit abgelaufen, Zeitüberschreitung | Ein Flow, dessen Entscheidungsfrist vor einer Entscheidung verstrichen ist; er wird blockiert |
| project folder | Projektordner | Das Verzeichnis auf dem Rechner, in dem der Agent arbeitet; unter bubblewrap, Podman und QEMU als `/work` eingehängt, unter Seatbelt unter seinem echten Pfad; nie „Arbeitsverzeichnis" oder „Projektverzeichnis" |
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
| kein Weg nach außen | No way out | Kein Weg nach außen | `sandboxGuaranteeNoWayOutMacos` | Garantie 1 unter macOS |
| eine Tür | Exactly one door, to Humanitl | Genau eine Tür, zu Humanitl | `sandboxGuaranteeOneDoorMacos` | Garantie 2 unter macOS |
| an das Profil gehalten | Held to the Seatbelt profile | An das Seatbelt-Profil gehalten | `sandboxGuaranteeProfileMacos` | Garantie 3 unter macOS |
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
| abgefangen | wartet auf Entscheidung; „anhalten", wo der Vorgang gemeint ist | Humanitl hält an und entscheidet gemeinsam mit dem Menschen; „abgefangen" (intercepted) klingt nach Überwachung |
| Treffer | Fund | Ein Fund ist eine Aussage über die Anfrage, kein Ergebnis einer Suche |
| anonymisier | pseudonymisieren | Anonymisieren ist rechtlich etwas anderes (BACKLOG.md 5) |
| ablehn | blockieren | Der Knopf blockiert eine Anfrage; „ablehnen" (reject) klingt nach einem Einwand gegen eine Person |

## Ersetzungstabelle

Die Arbeitsliste der Begriffsbereinigung (#79 entscheidet, ein späteres Issue
bereinigt). Jede Zeile nennt die alten Wörter, das neue Wort aus „Begriffe",
„Isolationsstufen" oder „Einstellungsebenen", was bleibt, und wie oft die alten
Wörter heute vorkommen. Die Spalte „Bleibt" ist Englisch wie im Original, weil
sie Bezeichner und Schlüssel aufzählt.

Wie gezählt wurde, auf `main` bei `cc6dc233` (2026-11-23): ganze Wörter, ohne
Rücksicht auf Groß- und Kleinschreibung, mit dem englischen Muster in den
englischen Bereichen und dem deutschen in den deutschen. „CLI" zählt
String-Literale in `daemon/bin/humanitl/src` und `daemon/bin/humanitld/src`
außerhalb von Testcode, „Diagnosen" die String-Literale in
`daemon/crates/core-types/src/diagnostics/codes.rs`, „ARB" die Texte von
`app/l10n/app_en.arb` und `app/l10n/app_de.arb` (die Schlüssel `diag…`
eingeschlossen), „Docs en" die englischen Seiten unter `docs/` mit `README.md`,
„Docs de" `docs/translations/de/` mit `README.de.md` und den deutschen
Originalen `docs/CONFIG.md`, `docs/DIAGNOSTICS.md` und
`docs/reference/parity.md`. ADRs, dieses Glossar und `backlog/` sind nicht
gezählt, weil sie nicht bereinigt werden. Die Zahlen schließen Bezeichner und
zitierte Werte ein, die bleiben (`disposable`, `humanitl daemon`, `--detach`);
sie bemessen die Arbeit und sind keine Liste von Stellen, die man blind
ersetzt.

| # | Alt (en) | Alt (de) | Neu (en) | Neu (de) | Bleibt | CLI | ARB en | ARB de | Diagnosen | Docs en | Docs de |
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
| | **Summe** | | | | | **170** | **244** | **292** | **176** | **2869** | **1981** |

Zwanzig Begriffe, 5732 Vorkommen. Die Zeilen 1, 3, 9, 10 und 19 ändern auch
Zeilen der Tabelle „UI terms" (Schlüssel `stateHeld`, `shellNavIntercept`,
`commonDaemon`, `auditChainTitle` und `sandboxWorkDirectory`); die Bereinigung
ändert so eine Zeile im selben Commit wie den ARB-Text, sonst schlägt
`make l10n-lint` fehl. Die Begriffe „lokaler Proxy" und „Upstream-Proxy" haben
in den heutigen Texten kein altes Wort; „Preset" hat noch keinen Text.

## Abweichungen von der Spezifikation

- **„Allow" auf dem englischen Knopf.** HUM-052 nannte „Send". `docs/UX.md`
  4.6 legt die drei Stellen fest (Knopf „Allow"/„Senden", Regel
  „allow"/„Erlauben", Streifen „Sent to …"/„Gesendet an …") und begründet die
  Trennung; das Glossar folgt ihm.
- **„Forbidden words" sind Wortstämme.** „ablehn" trifft „Ablehnen",
  „anonymisier" trifft „Anonymisieren" und „anonymisiert". „abgelehnt" bleibt
  erlaubt: Dort weist ein Dienst etwas zurück, etwa der Hintergrunddienst ein
  Token (`setupTokenInvalidTitle`), und es wird kein Knopf zur Wahl angeboten.
- **Die Überschriften „UI terms" und „Forbidden words"** ersetzen seit HUM-086
  „Begriffe" und „Verbotene Wörter"; `app/tool/l10n_lint.dart` sucht die
  Tabellen über diese Überschriften.
- **Isolationsstufen folgen dem Code, nicht „Prozess-Sandbox, Container,
  Micro-VM" (#79).** Die Spezifikation von #79 nannte diese drei.
  Prozess-Sandbox und Container sind im Code eine Stufe (`namespaces`,
  bubblewrap und Podman), und macOS hat eine eigene Stufe (`policy`). Der Owner
  hat am 2026-11-23 entschieden, die drei wirklichen Stufen mit ihren
  technischen Namen zu zeigen.
- **Eine Projekt-Ebene statt zwei (#79).** Die Projektwahl, die der
  Hintergrunddienst hält, und die Datei `.humanitl/profile.toml` im Repository
  bleiben im Code zwei Orte; ein Nutzer sieht eine Ebene, „Dieses Projekt", und
  ein Wert aus der Datei trägt die Marke „aus dem Repository" (Entscheidung des
  Owners vom 2026-11-23). Welcher Befehl wohin schreibt, entscheidet das ADR
  zum Einstellungsmodell (#80).
- **Das Verb „hold" bleibt (#79).** Die Spezifikation ersetzt „held" in Texten
  für Nutzer. Der Zustand wird „wartet auf Entscheidung"; der Vorgang, eine
  Anfrage zu stoppen, behält das Verb „hold"/„anhalten", weil „zur Entscheidung
  stoppen" denselben Vorgang mit mehr Wörtern benennt.
- **„Entscheidungen" als Name der Ansicht (#79).** In der Spezifikation nicht
  genannt. „Intercept" und „Anhalten" benannten die Ansicht nach dem Vorgang,
  den das Glossar als Beschreibung verbietet („intercepted"); „Entscheidungen"
  benennt, was in ihr geschieht.
- **„manipulationserkennbares Protokoll" (#79).** Das Deutsche hat kein kurzes
  Wort für „tamper-evident". Das Kompositum sagt, was das Protokoll tut, eine
  Änderung lässt sich erkennen, und nicht mehr; „manipulationssicher" würde
  Verhinderung versprechen.
