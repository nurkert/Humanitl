---
source: guides/agent-depot.md
source-sha256: 673ae800ef629ce6cb46856ee83a1e46d6ca55ad6d073d8d3063a01e8b0822b2
lang: de
---
# Das Agent-Depot

Applies to: Humanitl 0.1

Standardmäßig nimmt Humanitl den Agenten von diesem Rechner: Der Adapter findet sein Programm im
`PATH` des Daemons oder in deinen bin-Ordnern und bindet genau diese eine Datei nur lesbar in die
Sandbox. Das **Agent-Depot** ist der andere Weg: Humanitl installiert den Agenten einmal unter
seinem eigenen Datenordner, in einer Sandbox, und jede Session nimmt ihn von dort. Der Agent muss
dann auf diesem Rechner gar nicht vorhanden sein.

Beim Stand von HUM-522 läuft Claude Code aus dem Depot nur mit API-Schlüssel oder Gateway; eine
Abo-Anmeldung im Depot geht noch nicht (siehe [Heutige Grenzen](#current-limits)).

Diese Anleitung sagt, wann sich das Depot lohnt, wie man darauf umschaltet, was es schützt und was
nicht, und was heute geht. Die Entscheidung dahinter ist [ADR-0023](../adr/0023-agent-depot.md); die
Sicherheitsaussage steht in [SECURITY, das Agent-Depot](../SECURITY.md#the-agent-depot-hum-518-adr-0023)
und [Abschnitt 5.7](../SECURITY.md#57-the-credential-file-of-the-depot), die Bedrohung in
[K-36](../THREAT-MODEL.md#k-36-the-agent-depot). Jeder Befehl steht in
[`docs/cli.md`](../cli.md#humanitl-agents) und in der [CLI-Referenz](../reference/cli.md).

## Wann sich das Depot lohnt

Nimm es, wenn:

- **du den Agenten nicht auf diesem Rechner haben willst.** Ein auf dem Host installierter Agent
  kann außerhalb jeder Sandbox laufen, sich selbst aktualisieren und dein ganzes Home-Verzeichnis
  lesen. Aus dem Depot läuft er nur in einer Sandbox von Humanitl; nichts aus dem Depot wird auf
  dem Host gestartet, nicht einmal `--version` (Status und `doctor` lesen die Version aus dem
  `manifest.json` des Depots).
- **du selbst bestimmen willst, wann sich der Agent ändert.** Die Version ist angeheftet. Nur
  `humanitl agents update` ändert sie, eine laufende Session behält die Version, mit der sie
  startete, und in einer Session ist der Programmordner nur lesbar, sodass sich der Agent nicht
  selbst aktualisieren kann.
- **deine Firma es verlangt.** Eine Firmenvorgabe kann die Quelle auf das Depot festlegen (siehe
  [Firmenvorgabe](#company-policy)).

Bleib beim Host, wenn du den Agenten ohnehin auch außerhalb von Humanitl nutzt, oder wo das Depot
noch nicht läuft: Mit der Kernel-Stufe wird eine Depot-Session abgelehnt, und unter macOS
(Seatbelt) wird der Installationslauf abgelehnt, sodass sich dort kein Depot füllen lässt
(`AGENT_015`, siehe [Heutige Grenzen](#current-limits)).

## Was heute geht

| Agent | Installation ins Depot | Anmeldung im Depot | Session aus dem Depot |
|---|---|---|---|
| Claude Code (`claude`) | ja, von `downloads.claude.ai`, das signierte `manifest.json` des Herstellers geprüft mit dem `gpg` des Hosts | noch nicht: `humanitl agents login claude` antwortet mit `AGENT_014` | ja, mit dem Zugangsschlüssel, den Humanitl verwahrt (unten) |
| Codex (`codex`) | noch nicht: `AGENT_015` | erklärt (Geräte-Anmeldung), aber ohne Installation gibt es nichts anzumelden | nein |
| OpenCode, pi | kein Rezept: `AGENT_015` | nein | nein |
| Antigravity CLI (`agy`) | noch nicht: `AGENT_015` | keine, nur API-Schlüssel: `humanitl agents login agy` antwortet mit `AGENT_014` | nein, es gibt keine Installation |

Warum, und was daraus folgt, steht unter [Heutige Grenzen](#current-limits).

## Claude Code aus dem Depot, Schritt für Schritt

Voraussetzungen neben einem funktionierenden Humanitl unter Linux ([Installation](install.md)):
`gpg` auf dem Host (`/usr/bin/gpg` oder `/usr/local/bin/gpg`), weil der Daemon die Signatur des
Herstellers selbst prüft, und `curl` unter `/usr`, weil der Installationslauf mit dem nur lesbar
eingebundenen `/usr` des Hosts lädt. Claude Code selbst braucht der Host nicht, und es stört dort
auch nicht: Mit dem Depot sind die Installationsorte des Hosts, die das Rezept nennt
(`/usr/bin/claude`, `/usr/local/bin/claude`, `/usr/lib/node_modules/@anthropic-ai/claude-code`,
`/usr/local/lib/node_modules/@anthropic-ai/claude-code`), in jeder Session verdeckt.

### 1. Quelle umschalten

```sh
humanitl agents source claude depot
```

Das schreibt `agents.claude.source = "depot"` in deine `config.toml` (dasselbe wie `humanitl config
set agents.claude.source depot`). `humanitl agents source claude host` schaltet zurück und behält
das Depot bis `humanitl agents remove claude`. `agents.source` setzt die Vorgabe für jeden Agenten.
Keinen dieser Schlüssel kann ein Projekt-Profil setzen.

### 2. Installieren

```sh
humanitl agents install claude                  # die aktuelle Version des Herstellers
humanitl agents install claude --version 2.1.90 # oder eine feste
```

Der Befehl fragt vorher (mit `--yes` nicht) und hängt dann dieses Terminal an den
**Wartungslauf**: eine Sandbox mit den drei Garantien und dem Proxy, ohne Projekt. Ohne Halt geht
nur `downloads.claude.ai` hinaus, als Regel dieses Laufs, die mit seinem Ende geht; jede andere
Anfrage wird gehalten wie in jeder Session. Der Lauf lädt nur herunter: Manifest, Signatur, den
Schlüssel des Herstellers und das Programm. Der Installer des Herstellers wird nicht ausgeführt,
und in deinem `~/.local/bin` landet nichts. Nach dem Lauf prüft der Daemon den geladenen Baum,
prüft die Signatur von `manifest.json` gegen den in Humanitl festen Fingerabdruck, vergleicht den
SHA-256 des Programms mit dem Manifest und schaltet erst dann das Depot auf die neue Version. Ein
gescheiterter Lauf ändert nichts.

```sh
humanitl agents status claude
```

zeigt Quelle, Version und `verified`.

### 3. Claude Code seinen Zugang geben

Du brauchst einen API-Schlüssel von Anthropic, angelegt in der Anthropic Console, oder Adresse und
Schlüssel eines Gateways. Claude Code kann im Depot noch keine Abo-Anmeldung behalten (siehe
[Heutige Grenzen](#current-limits)). Seinen Zugang bekommt es stattdessen von Humanitl, auf
demselben Weg wie vom Host:

```sh
humanitl config set agent.adapter claude
humanitl llm key set          # liest den Anthropic-API-Schlüssel von stdin
```

Der Schlüssel liegt im Schlüsselbund des Systems (oder, wo es keinen gibt, in einer Datei `0600`),
nie in der Konfigurationsdatei. Jede Session bekommt ihn als Datei in der Sandbox; ein kleiner
Starter vor `claude` legt ihn in `ANTHROPIC_API_KEY`, sodass er in keiner Kommandozeile steht. Ohne
`llm.endpoint` spricht Claude Code mit `api.anthropic.com`. Für ein Gateway speicherst du es als
Server (`humanitl llm server add NAME URL --model MODELL --use`, danach `humanitl llm key set
--server NAME`): Claude Code bekommt dann `ANTHROPIC_BASE_URL`, den Schlüssel als
`ANTHROPIC_AUTH_TOKEN` und das erste Modell als `ANTHROPIC_MODEL`. So oder so geht nur der
Inferenz-Endpunkt ohne Halt hinaus ([SECURITY 5.1](../SECURITY.md#51-the-llm-passthrough)).

### 4. Starten, zweimal

```sh
cd ~/projects/example
humanitl go
```

Der erste Start von Claude Code mit einem Anbieter außerhalb dieses Rechners fragt einmal, ob das
gewollt ist (Cloud-Inferenz); die Antwort gilt für diesen Agenten und Anbieter. Dann startet Claude
Code aus dem Depot in einer Wegwerf-Session. Starte es noch einmal: Humanitl fragt nicht erneut
nach dem Hinweis und gibt jeder Session den Schlüssel aus seinem Schlüsselbund, sodass für den
Zugang keine Anmeldung nötig ist. Belegt ist das in der CI mit einem Stellvertreter-Programm für
den Weg mit API-Schlüssel und Gateway (siehe [Was die CI belegt](#what-ci-proves)); was ein echtes
Claude Code bei einem Start mit leerem Konfigurationsordner von sich aus fragt, ist noch nicht
gemessen (siehe unten).

Jede Session arbeitet auf ihrer eigenen Kopie der Konfiguration des Depots; was der Agent dort
schreibt, geht mit der Session. Eine Einstellung, die bleiben soll, geht über `humanitl agents
config claude`: Der Agent läuft auf einer Kopie, du siehst jede geänderte Datei mit den
hervorgehobenen Schlüsseln, die Befehle ausführen (`hooks`, `apiKeyHelper`, `statusLine`,
`mcpServers`, `env` und weitere), und erst deine Bestätigung übernimmt sie.

Was ein echtes Claude Code beim allerersten Start mit API-Schlüssel und leerem
Konfigurationsordner zeigt, und ob sich das in jeder frischen Kopie wiederholt, ist noch nicht
gemessen; das macht der gemeinsame Probedurchgang (HUM-553). Die CI belegt die Kette mit einem
Stellvertreter-Programm an Stelle von Claude Code (unten).

### 5. Aktualisieren, zurück, entfernen

```sh
humanitl agents update claude                   # die aktuelle Version des Herstellers
humanitl agents update claude --version 2.1.90  # zurück auf eine Version, die das Depot noch hält, ohne Lauf
humanitl agents remove claude                   # das ganze Depot: Programm, Konfiguration, Anmeldung
```

Ein Wartungslauf zur Zeit, und nur, solange keine Session im Daemon läuft (`CLI_005`). `remove`
fragt vorher und wird abgelehnt, solange eine Session oder ein Lauf das Depot nutzt
(`AGENT_018`); die Einstellung der Quelle bleibt, wie sie ist.

## Was das Depot schützt

- **Kein Agent auf dem Host.** Das Programm liegt unter `$XDG_DATA_HOME/humanitl/agents/<id>/`, im
  Besitz des Nutzers des Daemons, Ordner `0700`, Dateien `0600` oder `0700`, geprüft bei jedem
  Öffnen (`AGENT_013`). Es läuft nur in einer Sandbox.
- **Eine geprüfte, angeheftete Version.** Für Claude Code prüft der Daemon die Signatur des
  Herstellers mit dem `gpg` des Hosts und den SHA-256 des Programms; die Session bindet den
  aufgelösten Versionsordner nur lesbar ein, nie den Verweis `current`, sodass ein Update nie eine
  laufende Session ändert.
- **Aus einer Session bleibt nichts im Depot.** Eine Session bekommt eine schreibbare Arbeitskopie
  der Konfiguration des Depots, die mit ihrem Ende gelöscht wird. Zurück ins Depot darf nur die
  eine Zugangsdatei, die der Adapter erklärt, und nur über den Wächter der Rückschrift (nächster
  Punkt). Claude Code erklärt noch keine, also geht gar nichts ins Depot zurück.
- **Der Wächter der Rückschrift.** Wo ein Adapter das Format seiner Zugangsdatei erklärt, liest
  der Daemon genau diese Datei mit `O_NOFOLLOW`, höchstens 64 KiB, prüft jeden Schlüssel gegen das
  Format, lehnt ein Token ab, das älter ist als das gespeicherte, lehnt **ein anderes Konto** ab
  (nur ein Anmeldelauf, den du startest, kann das Konto ändern), schreibt seine eigene
  Serialisierung atomar und protokolliert jedes Ergebnis nur mit SHA-256
  (`agent.credential_writeback`). `agents.<id>.credential_writeback = false` schaltet die
  Rückschrift ab.
- **Ein Weg zum Agenten.** Mit `depot` wird eine Session abgelehnt, die eine Host-Kopie des
  Agenten nennt, über einen Pfad, einen Wrapper oder eine Einbindung (`AGENT_017`), und die
  bekannten Installationsorte des Hosts sind in der Sandbox verdeckt.

## Was das Depot nicht schützt

- **Dein Projekt.** Das Depot umfasst Programm und Konfiguration des Agenten, nicht `/work`. Was
  der Agent ins Projekt schreibt, bleibt dort wie in jeder Session; eine Wegwerf-Session lässt dich
  es prüfen, bevor es übernommen wird
  ([SECURITY 5.2](../SECURITY.md#52-the-project-directory-work)).
- **Dein eigener Nutzer auf dem Host.** Das Depot sind Dateien deines Nutzers. Jeder andere
  Prozess, der als du läuft, kann sie ändern wie jede Datei in deinem Home
  ([SECURITY 5.6](../SECURITY.md#56-other-known-limits),
  [außerhalb des Umfangs](../SECURITY.md#out-of-scope)). Das Depot hält den Agenten vom Host fern;
  es hält den Host nicht vom Depot fern.
- **Ein Seitenkanal über die Zugangsdatei.** Innerhalb ihres Formats gehört die Zugangsdatei der
  Session: Eine kompromittierte Session kann ein Token desselben Kontos schreiben, das nicht
  funktioniert, oder eines, das ein paar Bytes für die nächste Session trägt. Das ist ein
  verdeckter Kanal zwischen Sessions desselben Agenten, begrenzt durch das Format und 64 KiB,
  sichtbar im Audit-Log und geschlossen mit `credential_writeback = false`, um den Preis einer neuen
  Anmeldung, wenn das Token abläuft
  ([SECURITY 5.7](../SECURITY.md#57-the-credential-file-of-the-depot)).
- **Das Konto, das ein Anmeldelauf speichert.** Der Wächter vergleicht Konten nur mit der
  gespeicherten Datei: Eine Session, die ein anderes Konto schreibt, wird abgelehnt (`AGENT_016`),
  aber ein Anmeldelauf, den du startest, darf jedes Konto speichern, weil eine Anmeldung genau dafür
  da ist.
- **Parallele Sessions mit wechselndem Token.** Zwei Sessions, die dieselbe Anmeldung bei einem
  Anbieter erneuern, der sein Refresh-Token bei jeder Nutzung tauscht, können einander das Token
  ungültig machen; dann meldest du dich neu an.
- **Die Zugangsdatei auf der Platte.** Sie liegt unverschlüsselt im Depot, `0600`, wie die eigene
  Datei des Herstellers auf dem Host. Verschlüsselung mit einem Schlüssel aus dem Schlüsselbund ist
  geplant (HUM-523).
- **Eine absichtliche Kopie.** Die Firmenvorgabe hält Humanitl davon ab, einen Host-Agenten zu
  starten. Gegen jemanden, der ein Agenten-Programm an einen Pfad kopiert, den kein Rezept nennt,
  schützt sie nicht.
- **Inferenz.** Das Depot ändert, woher das Programm kommt, nicht, wohin seine Anfragen gehen.
  Alles, was der Agent an sein Modell schickt, geht wie bisher über die Durchreiche hinaus.

## Firmenvorgabe

ADR-0023 lässt eine Firmenvorgabe die Quelle auf `depot` festlegen, je Agent oder für alle; eine
Sperre darf nur `depot` nennen, nie `host` (`AGENT_010`). Solange gesperrt ist, lehnen `humanitl
agents source`, `humanitl config set`, die Einstellungen in der App und jeder andere Weg, die Quelle
zu schreiben, `host` gleichermaßen ab, App und CLI zeigen die Einstellung als von der Firma
festgelegt, und die Installationsorte des Hosts sind in jeder Session verdeckt. Der Auflöser und
jeder Schreibweg nehmen die Sperre schon als Eingabe. **Die Vorgabedatei selbst gibt es noch nicht**
(BACKLOG Abschnitt 9, Punkt 21): Bis dahin ist keine Sperre aktiv, und das Depot ist nur, was du
wählst.

## Heutige Grenzen

- **Anmeldung von Claude Code.** Der Hersteller dokumentiert weder die Schlüssel von
  `.credentials.json` noch, welche Felder von `.claude.json` eine Session braucht, um angemeldet zu
  starten. Humanitl rät nie ein Format; bis sie im gemeinsamen Probedurchgang gemessen sind
  (HUM-553, Punkt 3), antwortet `humanitl agents login claude` mit `AGENT_014`, und Claude Code aus
  dem Depot läuft nur mit API-Schlüssel oder Gateway.
- **Codex.** Das Paket des Herstellers trägt das Programm neben Hilfsprogrammen und startet es über
  einen Verweis; das Depot startet eine reguläre Datei. Bis das gemessen ist (HUM-553, Punkt 4) und
  das Depot ein solches Paket tragen kann (HUM-566), antwortet `humanitl agents install codex` mit
  `AGENT_015`.
- **Antigravity CLI.** Der Adapter ist da (HUM-517). `agy` legt seine Anmeldung im Schlüsselbund
  des Systems ab, und der Schlüsselbund des Hosts kommt nie in eine Sandbox; im Depot läuft es
  deshalb nur mit einem Gemini-API-Schlüssel (`humanitl llm key set`), und `humanitl agents login
  agy` antwortet mit `AGENT_014`. Der Installer des Herstellers nimmt den Host des Programms erst
  zur Laufzeit aus seinem Manifest und prüft nur einen SHA-512 aus demselben Manifest, keine
  Signatur; bis das gemessen ist (HUM-553, Punkt 5) und es ein Rezept gibt (HUM-566), antwortet
  `humanitl agents install agy` mit `AGENT_015`. Ein Konfigurationslauf antwortet ebenfalls mit
  `AGENT_015`: Keine Variable verlegt `~/.gemini` von `agy`, also trägt das Depot nur das Programm,
  und jede Session schreibt ihre Einstellungen neu.
- **OpenCode und pi** haben kein Rezept fürs Depot; mit `depot` antworten sie mit `AGENT_015`.
- **Backends.** Die Kernel-Stufe lehnt eine Depot-Session ab; Seatbelt (macOS) und die
  Kernel-Stufe lehnen Installations-, Update-, Anmelde- und Konfigurationsläufe ab (`AGENT_015`).
- **Ein Lauf zur Zeit**, und nur, solange keine Session im Daemon läuft (`CLI_005`).

## Was die CI belegt

`daemon/crates/ipc/tests/depot_maintenance.rs` fährt die Kette in einer echten Sandbox hinter dem
echten Proxy, ohne dass etwas den Rechner verlässt: die Quelle über den RPC umgeschaltet,
`install claude --version 2.1.90` mit dem eigenen Rezept von Humanitl für Claude Code gegen einen
Stellvertreter von `downloads.claude.ai` (ein Stellvertreter-Programm und ein Manifest, signiert
mit einem Schlüssel des Tests, geprüft vom `gpg` des Hosts; nur der Fingerabdruck unterscheidet
sich vom echten Rezept), `login claude` mit `AGENT_014` abgelehnt, zwei Sessions nacheinander, die
über `ANTHROPIC_BASE_URL` mit dem gespeicherten Schlüssel einen Stellvertreter-Endpunkt erreichen
und nie bei der Anmeldeabfrage des Stellvertreters landen, jede auf einer frischen Kopie der
Konfiguration, danach `remove claude`, nach dem keine Datei namens `claude` mehr unter dem Home des
Tests liegt und die Ordner seines `PATH` halten, was sie vorher hielten. Ein echtes Release, seine echte Signatur und eine echte
Abo-Anmeldung gehören nicht zur CI.
