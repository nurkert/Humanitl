---
source: guides/getting-started.md
source-sha256: fb2f4e55718b5ca05f78e920216717284dda1c131aad560225cb3e3451f38d26
lang: de
---
# Loslegen im Terminal

Applies to: Humanitl 0.1

Diese Anleitung geht den kürzesten Weg durch eine Session vom Terminal aus: Claude Code mit
`humanitl go claude` in einem Projektordner starten, es laufen lassen, in der Desktop-App wieder
aufnehmen und danach aufräumen. Jeder Befehl und jede Taste unten gibt es in dieser Version; alle
Flags stehen in der [CLI-Referenz](../reference/cli.md), die Einzelheiten in
[`docs/cli.md`](../cli.md). Den Weg nur durch die App beschreibt
[Deine erste Session](first-session.md).

## Bevor du anfängst

- Eine fertige [Installation](install.md): `humanitl daemon status` antwortet, und `humanitl
  sandbox check` zeigt drei `✓`.
- Claude Code dort, wo Humanitl es findet: auf diesem Rechner installiert oder im
  [Agent-Depot](agent-depot.md) von Humanitl (`humanitl agents install claude`).
- Sein Zugang: ein API-Schlüssel von Anthropic, gespeichert mit `humanitl llm key set` (von
  `stdin` gelesen, im Schlüsselbund des Systems gehalten, nie in einer Datei des Projekts). Die
  Anleitung zum Agent-Depot erklärt Schlüssel und Gateway in
  [Schritt 3](agent-depot.md#3-give-claude-code-its-access).

Das Beispielprojekt ist `~/shop`.

## 1. Mit `humanitl go claude` starten

```sh
cd ~/shop
humanitl go claude
```

`claude` ist der Agent; er gilt nur für diesen Start. Der Projektordner ist das aktuelle
Verzeichnis (oder `--dir PATH`).

**Beim ersten Mal in einem Ordner**, den Humanitl nicht kennt, kommt ein Bildschirm, bevor
irgendetwas startet: der Name des Ordners, eine Zeile mit den Vorgaben (Agent, Ordnermodus,
Isolation, wo Anfragen gefragt werden, Modellserver) und der Fokus auf „Start". `Enter` (oder
`Leertaste`) startet mit ihnen; gewählt werden muss nichts. `a` öffnet „Customize…" mit jedem Feld: Die Pfeiltasten
wechseln zwischen Feldern und Optionen, `Enter` auf einem Feld wählt nur. Das Feld des Agenten
bietet auch an, woher sein Programm kommt, von diesem Rechner oder aus dem Agent-Depot. Eine
Option, die hier nicht verfügbar ist, steht mit ihrem Grund da. `Esc` oder `q` bricht ab, dann
ist nichts gespeichert und nichts gestartet.

„Start" speichert die Wahl für diesen Ordner im Daemon, nicht im Ordner. Der nächste `humanitl go`
hier zeigt keinen Bildschirm. Ändern lässt sich die Wahl später mit `humanitl settings --project .`,
ansehen mit `humanitl projects show .`.

Weil Claude Code seine Inferenz an Anthropic schickt, fragt der erste Start dieses Agenten mit
diesem Anbieter einmal, ob er mit Inferenz bei Anthropic starten soll (`… It asks once per agent
and provider. [y/N]`). Danach sagt bei jedem Start eine kurze orange Zeile, wohin die Inferenz
geht.

Dann dreht sich die Faust des Logos, während die Sandbox startet, die drei Isolationsprüfungen
werden gemeldet, und Claude Code füllt das Terminal, als wäre es direkt gestartet. Anfragen ins
Internet, die keine Regel entscheidet, warten auf dich; in der Vorgabe entscheidest du sie in der
App (`--ask ui`).

## 2. In der Session

Jede Taste geht an Claude Code außer der Präfixtaste `Ctrl+B` (wie in tmux), gefolgt von einer
Taste:

| Tasten | Wirkung |
|---|---|
| `Ctrl+B ?` | Diese Tabelle zeigen. |
| `Ctrl+B ,` | Die Einstellungen über dem Agenten; `Esc` kehrt zurück. |
| `Ctrl+B i` | Der Interceptor: die gehaltenen Anfragen. Mit `↑`/`↓` eine wählen, dann erlaubt `a` sie einmal, `b` sperrt sie, `r` baut eine Regel (erst nach `Enter` geschickt); `Esc` kehrt zurück. |
| `Ctrl+B d` | Abhängen; die Session läuft weiter. |
| `Ctrl+B Ctrl+B` | Ein `Ctrl+B` an den Agenten schicken. |

Wird eine Anfrage gehalten, klingelt die Glocke des Terminals einmal (`terminal.bell_on_hold =
false` schaltet das ab). Lässt Claude Code einen Rest einer Ansicht auf dem Bildschirm stehen,
zeichnet `Ctrl+L` in Claude Code neu.

## 3. Abhängen

Die Session lässt sich auf zwei Wegen verlassen:

- `Ctrl+B d`. Der Befehl schreibt `Detached; the session keeps running.` und gibt das Terminal
  zurück.
- Das Terminalfenster schließen oder die SSH-Verbindung verlieren. Auch dann hängt `humanitl go`
  ab (`terminal.on_close = "detach"`, die Vorgabe; `"stop"` beendet die Session stattdessen).

Die Session läuft im Daemon weiter. Prüfen von jedem Terminal aus:

```sh
humanitl sessions
```

Die erste Zeile sagt `no terminal attached`, solange niemand angehängt ist. Anfragen, die der
Agent derweil schickt, warten auf eine Entscheidung in der App oder im Interceptor eines neuen
Anschlusses; läuft ihre Frist ab (`hold.timeout_secs`), werden sie gesperrt, nie erlaubt.

## 4. Wieder anhängen, in der App

```sh
humanitl app .
```

öffnet das Startfenster der Desktop-App mit diesem Ordner vorgewählt oder holt eine laufende App
nach vorn. Im Reiter „Projects" steht die Session unter „Running", markiert mit „No terminal
attached"; „Attach" öffnet sie in der App mit ihrem Terminal und ihrer Warteschlange gehaltener
Anfragen.

Das Terminal geht auch: `humanitl go` im selben Ordner hängt sich mit der Zeile `Attached to the
running session in shop` an, und `humanitl sandbox attach` geht aus jedem Ordner. Ein `go`, das
sich angehängt hat, stoppt die Session nie, was auch mit seinem Terminal geschieht.

Beenden lässt sich die Session, indem man Claude Code wie gewohnt verlässt, oder in der App auf
der Sandbox-Seite mit „Stop" und der Bestätigung „Stop the agent".

## 5. Aufräumen

**Änderungen.** Was mit dem Ordner geschah, hängt vom Ordnermodus des Erststart-Bildschirms ab.
Sagte seine Zeile der Vorgaben `disposable folder`, hat der Agent in eine Wegwerf-Schicht
geschrieben, nicht nach `~/shop`. Sagte sie `folder read-write`, kann dieses System hier keine
Wegwerf-Schicht halten (die Vorgabe `auto` fällt dann auf direktes Schreiben zurück, und der Start
nennt den Grund mit `SANDBOX_033`): Der Agent hat direkt nach `~/shop` geschrieben, und es gibt
nichts zu entscheiden. Wer Schreibzugriffe aus dem Ordner heraushalten muss, startet mit
`humanitl go claude --disposable` oder wählt unter „Customize…" `disposable folder`; wo das System
das nicht kann, lehnt der Start mit `SANDBOX_030` ab, statt in den Ordner zu schreiben.

Endet ein Wegwerf-Lauf an einem Terminal, listet der Befehl die Änderungen und fragt: `[a]pply all`, `[d]iscard all`, `[v]iew the diffs` oder für später behalten. Behaltene
Änderungen warten; der nächste `humanitl go` im Ordner fragt erneut, die App markiert das Projekt
mit „Changes waiting", und

```sh
humanitl sandbox changes --stat
humanitl sandbox apply      # oder: humanitl sandbox discard
```

entscheidet sie jederzeit.

**Aufgezeichnete Daten.** Jede Anfrage der Session ist mit ihren Bodies aufgezeichnet. Um das
Projekt aufzuräumen:

```sh
humanitl projects clean .
```

Der Plan umfasst die aufgezeichneten Anfragen mit ihren Bodies und, sofern dort keine Session
läuft und keine Änderungen warten, die dieses Aufräumen nicht verwirft, die gespeicherte Projektwahl und den Registereintrag des
Ordners. Der Daemon listet jeden Eintrag mit seiner Größe und was bleibt, und fragt `[y/N]`; nur
ein Ja löscht, und nur genau den gezeigten Plan. Noch wartende Änderungen gehen nur mit
`--include-waiting-changes` und sind dann endgültig verloren. In der App tut der Reiter „Clean up"
des Startfensters dasselbe. `humanitl projects forget .` entfernt die gespeicherte Wahl und den
Registereintrag des Ordners (nicht, solange dort eine Session läuft oder Änderungen
warten) und behält die aufgezeichneten Anfragen; der nächste `go` dort zeigt
wieder den Erststart-Bildschirm.

Den Projektordner und die Audit-Kette löscht nichts davon
([SECURITY, Aufräumen und Vergessen](../SECURITY.md#cleanup-and-forgetting-hum-533)).
