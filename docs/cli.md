# Die Kommandozeile

`humanitl` ist ein dünner Client des Daemons (ADR-018). Jede Fähigkeit ist
zuerst ein RPC; die Kommandozeile ruft ihn auf und formatiert die Antwort. Sie
enthält keine Fachlogik, und sie erfindet nichts dazu.

Dieses Dokument beschreibt `humanitl run` vollständig und die übrigen
Unterkommandos nur so weit, wie sie `run` betreffen. Die kanonische Liste aller
Unterkommandos steht in `backlog/CONVENTIONS.md` 3.8, das Schema aller
Konfigurations-Flags in `docs/CONFIG.md`.

## `humanitl run`

```
humanitl run [--profile NAME] [--work DIR] [--work-mode ro|rw]
             [--ask ui|terminal|none] [--llm URL] [-- CMD...]
```

Startet im Projektverzeichnis eine Sitzung: Der Daemon baut die Sandbox, der
Agent läuft darin, und `humanitl run` endet mit dessen Exit-Code.

### Was der Befehl der Reihe nach tut

1. **Das Profil der Sitzung auflösen.** Das ist der erste Schritt, vor der
   Verbindung zum Daemon und vor allem anderen. Ein `.humanitl/profile.toml`
   im Projekt kommt aus einem geklonten Repository und ist damit fremder Text;
   wer einen gesperrten Schlüssel setzt oder Host-Pfade einhängen will,
   bekommt hier `CONFIG_003` und die Sitzung startet nicht
   (`backlog/CONVENTIONS.md` 4.23).
2. **Den Daemon verbinden** und die Vertragsversion prüfen. Ohne Daemon gibt
   es keinen Proxy, keine Aufzeichnung und keine Sandbox.
3. **`Sandbox(Start)` senden** — mit dem Profil dieser Sitzung, dem
   Projektverzeichnis, dem Arbeitsmodus, dem Frage-Modus und den
   Konfigurationswerten der Kommandozeile. Der Daemon löst daraufhin für genau
   diese Sitzung erneut auf und baut Regelspeicher, Haltefrist und die
   Durchreiche zum Sprachmodell daraus neu.
4. **Die drei Garantien zeigen**, sobald der Daemon sie an der laufenden
   Sandbox gemessen hat — je eine Zeile, `[ok  ]` oder `[FAIL]`. Eine rote
   Garantie beendet den Lauf mit Exit 3; die Sandbox wird dabei beendet.
5. **Die Ausgabe des Agenten durchreichen** und mit seinem Exit-Code enden.

### Die Flags

| Flag | Wirkt auf | Bedeutung |
|---|---|---|
| `--profile NAME` | — | Das Profil der Sitzung (`profiles/*.toml`), zum Beispiel `llm-only`. Ein Name, den es nicht gibt, ist `CONFIG_001`. Unter `humanitl sandbox` benennt dasselbe Flag etwas anderes, nämlich das bwrap-Profil; welche Bedeutung gilt, entscheidet das Unterkommando. |
| `--work DIR` | `sandbox.work_dir` | Das Projektverzeichnis, das in der Sandbox als `/work` liegt. Vorgabe ist das aktuelle Verzeichnis. Es muss absolut sein, ohne `..`, ein Verzeichnis, und unter dem Heimatverzeichnis liegen oder genau das sein, was in `config.toml` steht. Alles andere ist `SANDBOX_006`. |
| `--work-mode ro\|rw` | `sandbox.work_mode` | Ob der Agent im Projekt schreiben darf. |
| `--ask ui\|terminal\|none` | `hold.ask_mode` | Wo über eine gehaltene Anfrage entschieden wird. Siehe unten. |
| `--llm URL` | `llm.endpoint` | Das Sprachmodell dieser Sitzung. Daraus entsteht die erklärte Durchreiche: eine Regel in Rang 1, die nicht gehalten wird und die eigenen Block-Regeln überholt, für die Inferenzpfade dieses einen Hosts. Sie steht als eigene Regel in der Liste, unter `http://humanitl.internal/` und in der Aufzeichnung. Ist die Adresse nach ihrem Namen nicht im eigenen Netz, meldet der Start `LLM_006`; die Sitzung startet trotzdem. Aufgelöst wird dafür nichts. |
| `-- CMD...` | `agent.command` | Der Befehl in der Sandbox, statt des Agenten aus der Konfiguration. `-- bash` ist der Weg, sich die Sandbox von innen anzusehen. |

Jedes andere Konfigurations-Flag (`--hold-timeout-secs`, `--findings-enabled`
und so weiter) gehört zur Konfiguration und nicht zu einer Sitzung. Der Daemon
nimmt vom Client nur zwei Pfade an — `llm.endpoint` und `hold.timeout_secs` —
und antwortet auf jeden anderen mit `CONFIG_003`. Der Grund steht in
`backlog/CONVENTIONS.md` 4.25: Ein Client, der jeden Schlüssel setzen dürfte,
bestimmte damit die Einhängefläche der Sandbox und den Prozess darin. Wer einen
anderen Wert ändern will, schreibt ihn in `config.toml` oder in ein eigenes
Profil, wo ein Mensch ihn geschrieben hat und der Daemon ihn liest.

### Die drei Frage-Modi

- **`ui`** — Anfragen ohne Regel bleiben in der Warteschlange, und der Mensch
  entscheidet in der Anwendung. `humanitl run` sagt das vor dem Start in einer
  Zeile und schreibt danach nichts mehr dazu; die Anwendung zeigt die Karte.
  Läuft keine Anwendung, läuft die Frist ab und die Anfrage wird geblockt. Die
  Zeile je gehaltener Anfrage (`[humanitl] request held: …`) kommt mit
  HUM-042: Sie braucht den Ereignisstrom der Flüsse und die Säuberung der
  Werte, die aus der Anfrage des Agenten stammen.
- **`none`** — es wird nicht gefragt. Die Frist ist null, jede Anfrage ohne
  Regel läuft sofort in die Zeitüberschreitung, und der Agent bekommt `504`
  mit `reason: timeout`. Das ist der Modus des Profils `llm-only`: Dort
  entscheidet ohnehin eine Regel (`block host "**"`) vorher, und der Agent
  bekommt `403`.
- **`terminal`** — **gibt es noch nicht.** Der Befehl antwortet mit `CLI_002`
  und schlägt `--ask ui` oder `--ask none` vor. Der Prompt im Terminal braucht
  ein PTY, und das kommt mit HUM-042. Für Vollbild-TUI-Agenten wie OpenCode
  bleibt `CLI_002` auch danach die Antwort (`backlog/CONVENTIONS.md` 4.10): In
  einem Vollbild-TUI wäre die Frage nicht zu sehen.

### Terminal, Eingabe und Signale

Der Agent bekommt **kein** PTY. Seine Ausgabe kommt als Bytes über den
Ereignisstrom des Daemons und geht auf `stdout` und `stderr` dieses Prozesses;
gefiltert wird sie im Daemon, nicht hier. Der Filter ist eine Erlaubnisliste:
**von allen Steuerfolgen geht genau eine hinaus, `ESC [ … m` für Farbe und
Attribute.** Verworfen werden damit der Zugriff auf die Zwischenablage
(OSC 52), Verweise unter sichtbarem Text (OSC 8), das Setzen des
Fenstertitels, jede Bewegung des Cursors, jedes Löschen und Scrollen und das
Zurücksetzen des Terminals — jeweils in allen drei Schreibweisen: mit `ESC`
eingeleitet, als einzelnes C1-Byte und als dessen UTF-8-Kodierung.

Praktisch heißt das: Der Agent darf schreiben und färben und mit `\r` und `\b`
die Zeile umschreiben, auf der er gerade steht. Er kann keine Zeile
überschreiben, die schon dasteht — insbesondere keine der drei Zeilen, mit
denen dieser Befehl die Isolationsprüfungen meldet. Der Preis: Ein
Fortschrittsbalken, der mit `\x1b[K` bis zum Zeilenende löscht, lässt Reste
stehen. Warum die Regel so streng ist, steht in `docs/SECURITY.md` 3.3.

Daraus folgt für diese Fassung:

- Es gibt **keine Eingabe** an den Agenten. Ein Programm, das eine Frage
  stellt, bekommt keine Antwort. Für zeilenorientierte Läufe ist das kein
  Problem, für ein Vollbild-TUI schon.
- Es gibt **keinen Raw-Modus** und keine Weiterleitung der Fenstergröße. Das
  Terminal bleibt in jedem Ausgang so, wie es war.
- **`Ctrl+C`** beendet die Sitzung (`Sandbox(Stop)`), es geht nicht als Byte an
  den Agenten. Ohne Eingabekanal wäre die Alternative, das Signal zu
  verschlucken.
- Ein **`Ctrl+]`-Menü** gibt es nicht.

Alles davon kommt mit HUM-042.

### Exit-Codes

| Code | Bedeutung |
|---|---|
| Der des Agenten | Er hat sich beendet; seine Zahl wird weitergegeben. Ein Signal wird zu `128 + n`. |
| `1` | Ein Fehler des Aufrufers: ein Profil, das es nicht gibt, ein Pfad, der keiner ist, `--ask terminal`. |
| `2` | Der Daemon ist nicht erreichbar, oder er spricht eine andere Major-Version des Vertrags. |
| `3` | Eine der drei Isolations-Garantien gilt nicht. Die Sandbox wurde beendet. |
| `4` | Eine Sicherheitsverletzung, zum Beispiel ein Authority-Mismatch. |

**Bekannte Kollision:** Ein Agent, der selbst mit 2 oder 3 endet, ist von einem
Daemon- oder Isolationsfehler nicht zu unterscheiden. Wer beides sauber trennen
muss, liest `--json`: Dort steht der Exit-Code des Agenten als eigenes Feld,
und ein Fehlschlag ist ein Befund mit seinem Code.

### `--json`

Mit `--json` schreibt `humanitl run` am Ende genau einen JSON-Wert auf
`stdout`: Projektverzeichnis, Profil, Frage-Modus, Befehl und `exit_code`. Die
Ausgabe des Agenten geht dabei weiter durch dieselbe `stdout`; wer den JSON-Wert
allein braucht, liest die letzte Zeile. Ein Fehlschlag ist stattdessen eine
Zeile mit dem Befund.

### Eine Sitzung je Daemon

Der Daemon führt genau eine Sandbox. Ein zweites `humanitl run`, während eine
läuft, bekommt `CLI_005` mit der Kennung der laufenden Sitzung. Einen Befehl
zum Anhängen nennt der Text nicht, weil es keinen gibt; die Anwendung sieht die
laufende Sitzung von selbst.

## Was `run` mit den anderen Unterkommandos teilt

- `humanitl sandbox run` startet die Sandbox im Prozess der Kommandozeile und
  ist der Weg für Selbsttests und die Escape-Tests. `humanitl run` startet sie
  im Daemon; nur dort gibt es Proxy, Aufzeichnung und Warteschlange.
- `humanitl rules list` zeigt den Regelsatz, der gerade gilt — nach einem Start
  also den der Sitzung, samt der Regeln ihres Profils und ihrer Durchreiche.
- `humanitl daemon status` sagt, ob überhaupt ein Daemon da ist. Das ist die
  Antwort auf Exit 2.
