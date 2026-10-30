---
source: cli.md
source-sha256: d1f1847ada8e819c363dcd624182d39ba0601a857b84bb4caf642d2f05de81bf
lang: de
---
# Die Kommandozeile

Applies to: Humanitl 0.1

`humanitl` ist ein dünner Client des Daemons (ADR-018). Jede Fähigkeit ist zuerst
ein RPC; die Kommandozeile ruft ihn auf und formatiert die Antwort. Sie enthält
keine Fachlogik, und sie erfindet nichts.

Dieses Dokument beschreibt `humanitl run`, `humanitl doctor`, `humanitl config`,
`humanitl audit` und die drei `humanitl daemon`-Befehle vollständig, die übrigen
Unterkommandos nur, soweit sie `run` betreffen. Die kanonische Liste aller
Unterkommandos steht in `backlog/CONVENTIONS.md` 3.8, das Schema aller
Konfigurations-Flags in `docs/CONFIG.md`.

## `humanitl run`

```
humanitl run [--profile NAME] [--work DIR] [--work-mode ro|rw]
             [--ask ui|terminal|none] [--llm URL] [-- CMD...]
```

Startet eine Session im Projektverzeichnis: Der Daemon baut die Sandbox, der
Agent läuft darin, und `humanitl run` endet mit dem Exit-Code des Agenten.

### Was der Befehl tut, der Reihe nach

1. **Das Profil der Session auflösen.** Das ist der erste Schritt, vor der
   Verbindung zum Daemon und vor allem anderen. Eine `.humanitl/profile.toml`
   im Projekt stammt aus einem geklonten Repository und ist deshalb fremder
   Text; wer darin einen gesperrten Schlüssel setzt oder Pfade des Rechners
   einhängen will, bekommt hier `CONFIG_003`, und die Session startet nicht
   (`backlog/CONVENTIONS.md` 4.23).
2. **Mit dem Daemon verbinden** und die Vertragsversion prüfen. Ohne Daemon
   gibt es keinen Proxy, keine Aufzeichnung und keine Sandbox.
3. **`Sandbox(Start)` senden**, mit dem Profil dieser Session, dem
   Projektverzeichnis, dem Arbeitsmodus, dem Frage-Modus und den
   Konfigurationswerten von der Kommandozeile. Der Daemon löst dann für genau
   diese Session erneut auf und baut daraus den Regelspeicher, die Frist für
   angehaltene Anfragen und die Durchleitung zum Sprachmodell neu.
4. **Die drei Garantien zeigen**, sobald der Daemon sie an der laufenden
   Sandbox gemessen hat, je eine Zeile, `[ok  ]` oder `[FAIL]`. Eine rote
   Garantie beendet den Lauf mit Exit 3; die Sandbox wird dabei heruntergefahren.
5. **Die Ausgabe des Agenten durchreichen** und mit seinem Exit-Code enden.

### Die Flags

| Flag | Wirkt auf | Bedeutung |
|---|---|---|
| `--profile NAME` | — | Das Profil der Session (`profiles/*.toml`), zum Beispiel `llm-only`. Ein Name, den es nicht gibt, ist `CONFIG_001`. Unter `humanitl sandbox` benennt dasselbe Flag etwas anderes, nämlich das bwrap-Profil; welche Bedeutung gilt, entscheidet das Unterkommando. |
| `--work DIR` | `sandbox.work_dir` | Das Projektverzeichnis, das als `/work` in der Sandbox liegt, für diesen einen Aufruf; es wird nicht gespeichert. Ohne das Flag gilt der in `sandbox.work_dir` gespeicherte Ordner (der in „Einrichten" oder in den Einstellungen gewählte oder mit `humanitl config set sandbox.work_dir DIR` gesetzte), und nur wenn nichts gespeichert ist, das aktuelle Verzeichnis. Es muss absolut sein, ohne `..`, ein Verzeichnis, und unterhalb des Home-Verzeichnisses liegen oder genau das sein, was `config.toml` sagt, unter dem Pfad, den es wirklich hat (kein symbolischer Link darin). Alles andere ist `SANDBOX_006`. |
| `--work-mode ro\|rw` | `sandbox.work_mode` | Ob der Agent im Projekt schreiben darf. |
| `--ask ui\|terminal\|none` | `hold.ask_mode` | Wo über eine angehaltene Anfrage entschieden wird. Siehe unten. |
| `--llm URL` | `llm.endpoint` | Das Sprachmodell dieser Session. Daraus entsteht die erklärte Durchleitung: eine Regel auf Rang 1, die nicht angehalten wird und deine eigenen Blockierregeln überholt, für die Inferenzpfade dieses einen Hosts. Sie steht als eigene Regel in der Liste, unter `http://humanitl.internal/` und in der Aufzeichnung. Liegt die Adresse ihrem Namen nach nicht in deinem eigenen Netz, meldet der Start `LLM_006`; die Session startet trotzdem. Aufgelöst wird dafür nichts. |
| `-- CMD...` | `agent.command` | Der Befehl in der Sandbox, statt des Agenten aus der Konfiguration. `-- bash` ist der Weg, sich die Sandbox von innen anzusehen und den Agenten dort von Hand zu starten (siehe „Eine Shell für den Agenten"). |

**Woher das Projektverzeichnis kommt (HUM-293).** `humanitl run`,
`humanitl sandbox run` und die App nehmen denselben Ordner: `--work` für einen
Aufruf, sonst `sandbox.work_dir` aus `HUMANITL_SANDBOX__WORK_DIR`, einem Profil
oder `config.toml`, sonst das aktuelle Verzeichnis. Ein abgewiesener Ordner sagt,
aus welcher dieser Quellen er kam, damit der Satz dich nie zu einer
`config.toml` schickt, in der nichts steht:

```text
$ cd ~ && humanitl sandbox run
error[SANDBOX_006]: Mount verboten
  why: the project directory from the current directory names /home/u: /home/u contains /home/u/.config/humanitl, Humanitl's own configuration directory …; choose the folder of a single project instead, such as ~/projects/xyz
  fix: choose a project folder: humanitl config set sandbox.work_dir <dir>
  docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#sandbox_006
```

Bei einem gespeicherten Ordner lautet die Quelle `sandbox.work_dir in
~/.config/humanitl/config.toml`, beim Flag `--work`, bei der Variablen
`the environment variable HUMANITL_SANDBOX__WORK_DIR`. Eine laufende Session
behält ihren Ordner; ein neu gewählter gilt ab dem nächsten Start.

**Der gespeicherte Ordner wird bei jedem Start erneut geprüft (HUM-294).**
„Einrichten", die Einstellungen und `humanitl config set sandbox.work_dir`
speichern den aufgelösten Pfad, mit jedem symbolischen Link verfolgt. Bei jedem
Plan und jedem Start löst der Daemon den gespeicherten Ordner noch einmal auf und
wendet dieselben drei Prüfungen an wie auf `--work`: ein Verzeichnis, von der
Mount-Richtlinie erlaubt, und unterhalb des Home-Verzeichnisses oder genau der
erklärte Ordner. Ein Ordner außerhalb des Home-Verzeichnisses gilt nur so lange
als erklärt, wie sein gespeicherter Text noch der Pfad ist, zu dem er sich
auflöst. Wird ein übergeordnetes Verzeichnis später durch einen Link ersetzt, der
aus dem Home-Verzeichnis hinausführt, endet der Start mit `SANDBOX_006` aus `sandbox.work_dir in
~/.config/humanitl/config.toml`, und der Satz nennt, wohin der Pfad jetzt führt. Ein Ordner außerhalb des Home-Verzeichnisses, der von Hand
über einen Link eingetragen wurde, wird genauso abgewiesen;
`humanitl config set sandbox.work_dir <dir>` schreibt seinen echten Pfad. Ein
gespeicherter Ordner, der nicht mehr existiert, ist schon im Plan `SANDBOX_006`,
sodass „Einrichten" ihn vor jedem Start rot zeigt. Der Start hängt den Ordner
dann nur ein, wenn das Verzeichnis, das er öffnet, noch an dem Pfad liegt, den er
geprüft hat.

**Nur die Konfigurationsdatei des Daemons erklärt einen Ordner außerhalb des
Home-Verzeichnisses (HUM-296).** `humanitl run --config other.toml` schickt den
Ordner aus `other.toml` an den Daemon, und der Daemon liest diese Datei nicht.
Unterhalb des Home-Verzeichnisses startet der Ordner wie gewohnt. Außerhalb davon
endet der Start mit `SANDBOX_006` aus `sandbox.work_dir in other.toml`; der Satz
nennt die Datei des Daemons, und die Abhilfe ist der Befehl, der den Ordner dort
erklärt: `humanitl --config ~/.config/humanitl/config.toml config set sandbox.work_dir
<dir>`. `humanitl sandbox run --config other.toml` startet ohne
den Daemon und liest die Erklärung weiter aus der Datei, die es nennt.

Jedes andere Konfigurations-Flag (`--hold-timeout-secs`, `--findings-enabled`
und so weiter) gehört zur Konfiguration und nicht zu einer Session. Der Daemon
nimmt vom Client nur zwei Pfade an, `llm.endpoint` und `hold.timeout_secs`, und
beantwortet jeden anderen mit `CONFIG_003`. Der Grund steht in
`backlog/CONVENTIONS.md` 4.26: Ein Client, der jeden Schlüssel setzen könnte,
würde damit über die Mount-Fläche der Sandbox und den Prozess darin entscheiden.
Wer einen anderen Wert ändern will, schreibt ihn in `config.toml` oder in ein
eigenes Profil, wo ein Mensch ihn geschrieben hat und der Daemon ihn liest.

### Die drei Frage-Modi

- **`ui`**: Anfragen ohne Regel bleiben in der Warteschlange, und der Mensch
  entscheidet in der Anwendung. `humanitl run` sagt das vor dem Start in einer
  Zeile und schreibt danach nichts mehr dazu; die Anwendung zeigt die Karte.
  Läuft keine Anwendung, läuft die Frist ab, und die Anfrage wird blockiert. Die
  Zeile pro angehaltener Anfrage (`[humanitl] request held: …`) kommt mit
  HUM-042: Sie braucht den Ereignisstrom der Flows und das Bereinigen der Werte,
  die aus der Anfrage des Agenten stammen.
- **`none`**: Niemand wird gefragt. Die Frist ist null, jede Anfrage ohne Regel
  läuft direkt in die Zeitüberschreitung, und der Agent bekommt `504` mit
  `reason: timeout`. Das ist der Modus des Profils `llm-only`: Dort entscheidet
  ohnehin vorher eine Regel (`block host "**"`), und der Agent bekommt `403`.
- **`terminal`**: Der Mensch entscheidet im selben Terminal. Sobald eine Anfrage
  angehalten wird, erscheint auf `stderr` ein Kasten, während die Ausgabe des
  Agenten auf `stdout` weiterläuft:

  ```
  ┌─ humanitl · request held (1 of 2) ─────────────────────── 04:52 left ─┐
  │ POST https://api.github.com/graphql                                   │
  │ from: opencode · webfetch                                             │
  │ size: 2.1 KB                                                          │
  │ findings: 1 · api_key.github in header Authorization                  │
  │ catalog: github.api · rank #37                                        │
  │                                                                       │
  │ [a] allow once   [s] allow this session   [b] block   [r] rule…       │
  │ [v] view body    [n] next                                             │
  └───────────────────────────────────────────────────────────────────────┘
  ```

  Die Tasten:

  | Taste | Was sie tut |
  |---|---|
  | `a` | Erlaubt diese eine Anfrage. |
  | `s` | Erlaubt sie und legt eine Regel für diesen Host an, gültig für diese Session. |
  | `b` | Blockiert sie. Der Agent bekommt `403`. |
  | `r` | Fragt nach dem Ziel (URL, Host, Apex, Host und Methode) und der Gültigkeit (einmal, Session, dauerhaft), zeigt die Regel und legt sie nach `Enter` an. Zwei Ziele können scheitern und sagen es: ein Apex, den der Dienst nicht kennt (eine Adresse, eine unbekannte Endung), und eine Methode, die der Vertrag nicht nennt; eine Regel, die dann breiter wäre als das, was auf dem Bildschirm stand, wird nicht angelegt. |
  | `e` | Schreibt die Anfrage einschließlich ihres vollständigen Bodys nach `$XDG_RUNTIME_DIR/humanitl/edit-<id>.http` (0600, neu angelegt, ohne einem Symlink zu folgen), öffnet `$VISUAL` oder `$EDITOR` (sonst `vi`) und sendet das Ergebnis als `allow_edited`. Die Datei wird nach dem Lesen gelöscht. Ohne `$XDG_RUNTIME_DIR` verweigert dieser Weg den Dienst, statt in ein Verzeichnis zu schreiben, in das jeder schreiben kann. Ein Body, der kein UTF-8 ist, wird nicht bearbeitet: Was ein Editor daraus machte, ginge als etwas anderes hinaus, als der Agent gesendet hat. |
  | `v` | Zeigt die ersten 4 KiB des Bodys; was nicht druckbar ist, erscheint als Hex. |
  | `n` | Geht zur nächsten angehaltenen Anfrage, ohne zu entscheiden. |
  | `Esc`, `Ctrl+C` | Schließt den Kasten. Die Anfrage bleibt angehalten. |

  Der Kasten wird jede Sekunde neu gezeichnet, damit die Uhr oben rechts stimmt,
  und er verschwindet, sobald über die Anfrage entschieden ist, auch wenn jemand
  anderes im Fenster entschieden hat. Läuft die Frist ab, erscheint
  `[humanitl] timed out -> blocked`, und der nächste angehaltene Flow kommt
  dran.

  Er braucht ein Terminal an der Eingabe und mindestens 40 Spalten. Ohne
  Terminal, in einer Pipe, in einem Skript, verweigert der Befehl den Dienst mit
  `CLI_002`: Bytes aus einer Pipe hat niemand als Antwort gemeint. Der Kasten ist
  außerdem ASCII; eine Breite in Zeichen ist nicht dasselbe wie eine Breite in
  Spalten, und ein Zeichen doppelter Breite würde jede Zeile umbrechen lassen.
  Ein Pfad in einer anderen Schrift ist im Kasten deshalb nicht lesbar; wer ihn
  lesen will, nimmt `humanitl flows show`. Solange der Kasten offen ist, wird die
  Ausgabe des Agenten zurückgehalten (höchstens 256 KiB); danach geht sie hinaus,
  und der Kasten wird darüber neu gezeichnet.

  Jedes Feld läuft durch `sanitize_note` und wird auf die Fensterbreite
  gekürzt: Was der Agent sendet, ist Text und keine Steuersequenz, und keine
  Adresse schiebt die Tastenzeile aus dem Bildschirm.

  Für **Vollbild-TUI-Agenten wie OpenCode** bleibt `CLI_002` die Antwort
  (`backlog/CONVENTIONS.md` 4.10): Eine TUI zeichnet den ganzen Bildschirm neu,
  und der Kasten wäre nach dem ersten Frame weg. Entschieden wird am wirksamen
  Befehl: `humanitl run --ask terminal -- bash` startet keine TUI und bekommt
  die Eingabeaufforderung.

### Eine Shell für den Agenten

`humanitl run -- bash` mit einem Terminal an `stdin` hängt dieses Terminal an
die Session, wie `humanitl sandbox attach`: Raw-Modus, sobald der Daemon
`running` meldet, die Fenstergröße, jede Taste an die Shell und die Trenntaste
`Ctrl+A d` (`terminal.detach_prefix`). Das Terminal der Session ist das eigene
Pseudo-Terminal des Daemons, nie dieses hier; die Bytes kommen gefiltert zurück
wie bei `attach` (HUM-280).

In dieser Shell liegt der konfigurierte Agent im `PATH`: Liegt er außerhalb der
Mounts der Sandbox (`~/.opencode/bin`, `~/.local/bin`, `~/bin`), bindet der Daemon
diese eine Datei nach den Regeln von HUM-278 schreibgeschützt unter
`/run/humanitl/agent/<name>` ein und stellt `/run/humanitl/agent` an den Anfang
von `PATH`. Dateien und Umgebung des Adapters sind dieselben wie bei einem
Agenten, den der Daemon selbst startet: Modell, Schlüsseldatei, CA und
Konfiguration. So startet `opencode` (oder `pi`) mit dem konfigurierten Modell
und Schlüssel, `Ctrl+C` beendet ihn und kehrt zur Shell zurück, `exit` beendet
die Session, und `Ctrl+A d` lässt sie für ein späteres
`humanitl sandbox attach` weiterlaufen. Der Befehl nach `--` selbst wird nie
eingebunden; er muss in der Sandbox startbar sein. pi ist ein Skript und wird nie
eingebunden; es muss unter einem Mount wie `/usr/local` liegen. Gibt der
Schlüsselspeicher den Schlüssel des Modells nicht heraus (`LLM_009`, etwa bei
gesperrtem Schlüsselbund), startet ein eigener Befehl trotzdem, ohne den
Schlüssel, und der Start meldet `LLM_009` als Warnung; der Agent in dieser Shell
hat dann keinen Schlüssel. Eine Session ohne Befehl bricht wie bisher mit
`LLM_009` ab.

Ohne Terminal an `stdin`, mit `--ask terminal` (dessen Kasten dieselbe Tastatur
braucht) oder mit `--json` liest der Befehl weiter mit, wie unten beschrieben.

### Eine zweite Shell neben dem Agenten

`humanitl sandbox exec` öffnet eine weitere Shell in der laufenden Sandbox, wie
`docker exec`: Der Agent läuft weiter und behält sein eigenes Terminal, und die
Shell bekommt ein eigenes Terminal. Ohne Befehl nach `--` ist es eine
interaktive `bash`, oder `sh`, wenn die Sandbox keine `bash` hat, beide ohne
Startdateien (`--norc --noprofile`, `ENV` nicht gesetzt);
`humanitl sandbox exec -- htop` führt stattdessen das aus. Die Shell trägt
dieselben Grenzen wie der Agent: den seccomp-Filter, die Mounts, den
Projektordner unter `/work`, das Netz nur über den Proxy und seine Regeln,
dieselbe Umgebung mit dem konfigurierten Agenten im `PATH`. Gestartet wird sie
von innen aus der Sandbox durch den Shim, nie durch Betreten ihrer Namespaces von
außen (HUM-299, `docs/SECURITY.md` Abschnitt 4).

```text
$ humanitl sandbox exec
Shell 4f2a91c3-1 in sandbox 0193…
Detach: Ctrl+A d
agent@sandbox:/work$
```

`Ctrl+A d` trennt und lässt die Shell weiterlaufen; `humanitl sandbox attach
--shell 4f2a91c3-1` hängt sich wieder an, mit `--read-only` zum Zuschauen; die
ID gilt nur für diese Session. Ein Befehl, der endet, bevor das Terminal
angehängt ist, hinterlässt Ausgabe und Exit-Code für den ersten Client, 30
Sekunden lang. Die Shell ist nicht privat gegenüber dem Agenten, der als
derselbe Nutzer daneben läuft: Tippe dort keine Secrets ein (`docs/SECURITY.md`
Abschnitt 4). Ein Client schreibt, beliebig viele schauen zu, wie beim Terminal
des Agenten. Die Shell endet, wenn ihr Prozess endet (der Befehl endet mit dessen
Exit-Code) oder wenn die Sandbox stoppt; sie überlebt die Session nie. Höchstens
vier weitere Shells leben gleichzeitig; eine fünfte wird mit `TERM_002`
abgewiesen. Ohne Terminal an `stdin` schaut der Befehl nur der Ausgabe zu, bis
die Shell endet.

### Eine Wegwerf-Session

`humanitl run --disposable` (oder `sandbox.mode = "disposable"` in
`config.toml`) lässt den Projektordner unberührt, während der Agent arbeitet.
Der Ordner liegt schreibgeschützt unter einer Schicht im RAM; der Agent sieht und
ändert `/work` wie gewohnt, und jede Änderung geht in diese Schicht (HUM-350,
`docs/SECURITY.md` Abschnitt 5.2). Wenn der Agent endet, sagt `run`, wie viele
Änderungen warten:

```text
$ humanitl sandbox changes
Changes of sandbox 0193… in /home/me/project (3):
  5c1d0e…  modified src/main.rs
  a03b77…  added    notes.txt
  9e41f2…  symlink  out -> /etc/shadow  [never applied: it is a symbolic link to /etc/shadow, which leaves the project]
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
…
$ humanitl sandbox apply 5c1d0e…
Apply 1 change(s) to /home/me/project and discard 2 other(s)? [y/N] y
applied: sandbox 0193…
  applied    src/main.rs
  discarded  notes.txt
  discarded  out
```

`apply` ohne IDs nimmt alle Änderungen, `discard` verwirft sie alle; beide fragen
vorher (`--yes` für Skripte). Geschrieben wird nichts, wenn eine gewählte
Änderung sich nicht so anwenden lässt, wie sie ist: Eine Datei, die du seit dem
Start im Projekt geändert hast, ist ein Konflikt und braucht `--overwrite`, und
ein symbolischer Link aus dem Projekt hinaus, ein Gerät, ein FIFO oder ein Socket
wird nie geschrieben. setuid- und setgid-Bits fallen weg. Ein neuer Wegwerf-Lauf
wartet, bis die Änderungen des letzten angewendet oder verworfen sind
(`SANDBOX_030`); ein Lauf, dessen Schicht über ihre Obergrenze im RAM wächst,
wird gestoppt (`SANDBOX_032`). `humanitl sandbox run --disposable` behält nach
dem Ende keine Schicht: Es listet die Änderungen auf und verwirft sie.

### Terminal, Eingabe und Signale

Ohne „Eine Shell für den Agenten" bekommt der Agent **kein** PTY dieses Terminals. Seine Ausgabe kommt als Bytes über den
Ereignisstrom des Daemons an und geht nach `stdout` und `stderr` dieses
Prozesses; gefiltert wird sie im Daemon, nicht hier. Der Filter ist eine
Erlaubnisliste: **Von allen Steuersequenzen geht genau eine hinaus, `ESC [ … m`
für Farbe und Attribute.** Damit fallen weg: der Zugriff auf die
Zwischenablage (OSC 52), Links unter sichtbarem Text (OSC 8), das Setzen des
Fenstertitels, jede Cursorbewegung, jedes Löschen und Scrollen und das
Zurücksetzen des Terminals, jeweils in allen drei Schreibweisen: eingeleitet mit
`ESC`, als einzelnes C1-Byte und als dessen UTF-8-Kodierung.

Praktisch heißt das: Der Agent darf schreiben, färben und die Zeile, auf der er
gerade steht, mit `\r` und `\b` neu schreiben. Eine Zeile, die schon dasteht,
kann er nicht überschreiben, insbesondere keine der drei Zeilen, mit denen dieser
Befehl die Isolationsprüfungen meldet. Der Preis: Ein Fortschrittsbalken, der mit
`\x1b[K` bis zum Zeilenende löscht, hinterlässt Reste. Warum die Regel so streng
ist, erklärt `docs/SECURITY.md` 3.3.

Für diese Version bedeutet das:

- Es gibt **keine Eingabe** an den Agenten. Ein Programm, das eine Frage stellt,
  bekommt keine Antwort. Für zeilenorientierte Läufe ist das kein Problem, für
  eine Vollbild-TUI schon. Wer tippen will, hängt sich mit
  `humanitl sandbox attach` an dieselbe Session; dort geht jede Taste an den
  Agenten, außer der Trenntaste `Ctrl+A d`.
- Einen **Raw-Modus** gibt es nur mit `--ask terminal`, und nur für die Tasten
  des Kastens: Die Abfrage braucht einzelne Tasten ohne Zeilenende. Das Terminal
  wird bei jedem Ende wiederhergestellt: beim gewöhnlichen Ende, auf jedem
  Fehlerpfad, bei einer Panic und bei `SIGTERM`, `SIGHUP` und `SIGINT` (dann mit
  Exit `128 + n`). Ohne `--ask terminal` wird es gar nicht angefasst.
- **`Ctrl+C`** beendet die Session (`Sandbox(Stop)`); es geht nicht als Byte an
  den Agenten. Ist der Kasten offen, schließt das erste `Ctrl+C` ihn, und die
  Anfrage bleibt angehalten. **Zweimal gedrückt, endet der Befehl selbst** mit
  `130`: Das erste bittet die Session zu enden, und wenn der Agent nicht hört,
  soll das zweite nicht ins Leere gehen. Dasselbe gilt für `SIGTERM` und `SIGHUP`
  von außen: Sie beenden Session und Befehl mit `128 + n`, und das Terminal ist
  danach wieder im normalen Modus.
- Es gibt kein **`Ctrl+]`-Menü**. Der Weg zu einer angehaltenen Anfrage ist der
  Kasten von `--ask terminal`, die Anwendung oder `humanitl flows`.

### Exit-Codes

| Code | Bedeutung |
|---|---|
| Der des Agenten | Er hat geendet; seine Zahl wird weitergegeben. Ein Signal wird zu `128 + n`. |
| `1` | Ein Fehler des Aufrufers: ein Profil, das es nicht gibt, ein Pfad, der keiner ist, `--ask terminal` mit einem Vollbild-TUI-Agenten. |
| `2` | Der Daemon ist nicht erreichbar, oder er spricht eine andere Hauptversion des Vertrags. |
| `3` | Eine der drei Isolationsgarantien hält nicht. Die Sandbox wurde heruntergefahren. |
| `4` | Eine Sicherheitsverletzung, zum Beispiel eine nicht passende Authority. |

**Bekannte Überschneidung:** Ein Agent, der selbst mit 2 oder 3 endet, lässt sich
nicht von einem Daemon- oder Isolationsfehler unterscheiden. Wer beides sauber
trennen muss, liest `--json`: Dort ist der Exit-Code des Agenten ein eigenes
Feld, und ein Fehlschlag ist eine Diagnostic mit ihrem Code.

### `--json`

Mit `--json` schreibt `humanitl run` am Ende genau einen JSON-Wert nach
`stdout`: Projektverzeichnis, Profil, Frage-Modus, Befehl und `exit_code`. Die
Ausgabe des Agenten läuft weiter über dasselbe `stdout`; wer den JSON-Wert allein
braucht, liest die letzte Zeile. Ein Fehlschlag ist stattdessen eine Zeile mit
der Diagnostic.

### Eine Session pro Daemon

Der Daemon betreibt genau eine Sandbox. Ein zweites `humanitl run`, während eine
läuft, bekommt `CLI_005` mit der Kennung der laufenden Session. Wer sie sehen
will, hängt sich an: `humanitl sandbox attach` verbindet dieses Terminal mit der
laufenden Session, `--read-only` schaut nur zu. Genau ein Client darf schreiben;
ein zweiter bekommt `TERM_001` und den Hinweis auf `--read-only`.
`Ctrl+C` geht als Byte `0x03` an das Pseudo-Terminal der Session; der erste
Prozess der Session hält dieses Terminal als sein steuerndes Terminal, also
unterbricht es dort das Vordergrundprogramm, und ein Programm im Raw-Modus liest
das Byte. Wer sich trennt, beendet nur seinen eigenen Strom; die Session läuft
weiter, und ein späteres `attach` zeigt, was sich inzwischen angesammelt hat.

Der Weg hinaus ist eine Taste, wie in `screen`: **`Ctrl+A d`** trennt. Der Befehl
sendet `close`, gibt das Terminal zurück und endet mit `0` und der Zeile
`Detached; the session keeps running. Attach again: humanitl sandbox attach`.
Bevor das Terminal in den Raw-Modus geht, nennt eine Zeile auf `stderr` die Taste
(`Detach: Ctrl+A d`). Nach dem Präfix:

| Taste | Wirkung |
|---|---|
| `d` | Trennen; die Session läuft weiter. |
| `Ctrl+A` oder `a` | Ein `Ctrl+A` an den Agenten senden. |
| alles andere | `Ctrl+A` und diese Taste unverändert senden. |

Das Präfix darf in einem Lesevorgang ankommen und die Taste im nächsten; es
wartet ohne Zeitgrenze auf die nächste Taste. `--detach-keys SPEC` wählt für
einen Aufruf ein anderes Präfix (`ctrl-a` bis `ctrl-z`, `ctrl-\`, `ctrl-]`,
`ctrl-^`, `ctrl-_`), und `--detach-keys none` schaltet das Trennen ab, sodass
jedes Byte an den Agenten geht. `terminal.detach_prefix` in `config.toml` setzt
die Vorgabe; das Profil eines Projektordners darf es nicht setzen. `ctrl-[` wird
abgewiesen, weil jede Pfeiltaste mit diesem Byte beginnt, und `ctrl-h`,
`ctrl-i`, `ctrl-j` und `ctrl-m`, weil Rücktaste, Tab und Enter dieselben Bytes
senden. `ctrl-c` und `ctrl-d` sind erlaubt, aber dann erreicht diese Taste den
Agenten nur als zweimal gedrücktes Präfix. Bei einem anderen Präfix tritt dessen
eigener Buchstabe an die Stelle von `a` (`Ctrl+] ]`).

Liest die Session ihre Eingabe nicht, warten höchstens 1 MiB getippter oder
eingefügter Text im Befehl; mehr wird verworfen, und die Trenntaste funktioniert
weiterhin. Nach der Taste wartet der Befehl höchstens zwei Sekunden auf den
Daemon, bevor er das Terminal zurückgibt.

`--read-only` trennt auf dieselbe Taste. Es liest die Tastatur nur dafür und
sendet nichts hinauf; an einem Terminal nutzt es dafür den Raw-Modus, und
`Ctrl+C` trennt ebenfalls. Mit `--detach-keys none` lässt es das Terminal in
Ruhe. Das Terminal der Anwendung ist nicht betroffen: Dort trennt das Schließen
des Bereichs.

## `humanitl doctor`

```
humanitl doctor [--json] [--probe-llm]
```

Prüft den Rechner und gibt eine Zeile pro Voraussetzung aus, mit `ok`, `warn`
oder `fail`. Das ist der Befehl für die Frage „warum läuft das hier nicht?".

### Die elf Zeilen

| Kennung | Was sie prüft | `fail`, wenn |
|---|---|---|
| `bwrap` | `bwrap` liegt im `PATH` und ist mindestens 0.8 | es fehlt oder älter ist |
| `userns` | `bwrap --unshare-user` öffnet einen Namespace | es nicht funktioniert oder länger als zwei Sekunden hängt |
| `seccomp` | `/proc/self/status` hat ein Feld `Seccomp`, der Kernel ist mindestens 5.4 | das Feld fehlt (Kernel ohne `CONFIG_SECCOMP`) |
| `runtime_dir` | `$XDG_RUNTIME_DIR` ist gesetzt, ist ein Verzeichnis, gehört uns und hat genau 0700 | eines davon nicht zutrifft, auch bei einem zu engen Modus wie 0500 oder 0600, in dem der Daemon weder hineinwechseln noch den Socket anlegen kann |
| `systemd_user` | `systemctl --user is-system-running` endet mit 0 und sagt `running` | nie; ohne systemd startest du `humanitld` von Hand |
| `daemon` | der Daemon antwortet und spricht dieselbe Hauptversion des Vertrags | die Hauptversion abweicht |
| `agent` | der Befehl des Agenten liegt im `PATH` und nennt seine Version | nie; der Pfad in der Sandbox kann ein anderer sein |
| `llm` | der Endpunkt aus `llm.endpoint` antwortet | nie |
| `tray` | `libayatana-appindicator3` oder `libappindicator3` liegt in einem Verzeichnis des Loaders; unter GNOME auch die AppIndicator-Erweiterung | nie |
| `renderer` | Impeller kommt mit dem Treiber zurecht: ein geladenes NVIDIA-Modul unter Wayland ist der bekannte schwarze Bildschirm | nie |
| `disk_space` | im Datenverzeichnis ist mindestens 1 GiB frei | nie |

Die ersten vier sind die Voraussetzungen der Sandbox; ohne sie startet nichts.
Die Zeile `daemon` wird auch dann `fail`, wenn der Daemon eine andere
Hauptversion des Vertrags spricht; dann versteht keine Seite die Nachrichten der
anderen. Alles andere ist eine Warnung: Es läuft, nur nicht so bequem.

### Was eine Zeile bedeutet

Jede Zeile, die nicht `ok` ist, trägt eine `Diagnostic` mit Code, Grund und
Vorschlag; die Blöcke stehen unter der Tabelle, einer pro Zeile und nicht nur für
die erste. Die Codes sind `DOCTOR_001` bis `DOCTOR_013`
(`docs/DIAGNOSTICS.md`).

**`ok` heißt nachgesehen und in Ordnung, nie „ich konnte nicht nachsehen".**
Eine Prüfung, die sich nicht durchführen ließ, etwa eine Datei, die es auf
diesem Kernel nicht gibt, oder ein Programm, das nicht antwortet, ist `warn` mit
`DOCTOR_012`; ihr Beleg beginnt mit `not measured:`, und ihr Vorschlag ist der
Befehl, den der Doctor versucht hat, damit ein Mensch selbst nachsehen kann.

### Der Endpunkt des Sprachmodells wird nur auf Wunsch angesprochen

`humanitl doctor` öffnet von sich aus keine einzige Verbindung. Die Zeile `llm`
trägt deshalb ohne weitere Optionen `DOCTOR_013`, „nicht angesprochen", und als
Vorschlag den Befehl, der es täte.

Mit `--probe-llm` wird gemessen. Vorher sagt eine Zeile auf `stderr`, wohin es
geht und was gesendet wird, **bevor** die Verbindung geöffnet wird; sie lässt
sich weder mit `--json` noch mit `-q` abschalten, weil sie zur Aktion gehört und
nicht zur Ausgabe. Die Messung läuft über den RPC `ProbeLlm` im Daemon (`GET`
auf die Modellliste des Endpunkts, bei einem Ollama ohne Pfad-Präfix zuerst auf
`/api/tags`; keine Zugangsdaten, keine Weiterleitungen); ohne laufenden Daemon
gibt es keine Probe, und die Zeile bleibt ehrlich ungemessen. Ein Server, der
einen Zugangsschlüssel will, beantwortet diese Probe ohne Schlüssel mit `401`
oder `403`; die Zeile sagt das dann, sagt, dass der Doctor keinen Schlüssel
sendet, und sagt, ob einer hinterlegt ist (erfragt über `LlmKey`, nie der
Schlüssel selbst). Die Abhilfe ist `humanitl llm key set` nur, wenn keiner
hinterlegt ist; ist einer hinterlegt, ist es
`humanitl llm test <endpoint> --with-key` (HUM-283). Ein `llm.endpoint` in
`config.toml`, das ein Anführungszeichen, eine spitze Klammer, einen Backslash
oder Leerraum enthält (eine ältere Version hat das angenommen), hält den Daemon
nicht auf: Es gilt als nicht gesetzt, und die Zeile `llm` trägt die Warnung
`CONFIG_003` mit der bereinigten Adresse als Abhilfe. Ohne Modellliste nennt die
Zeile die letzte Antwort (`no model list, last answer HTTP 404, text/html`) statt
`0 models`. Der Schalter heißt nicht `--llm`: Das ist der Alias von
`--llm-endpoint` und **setzt** die Adresse, statt sie zu prüfen.

### Woher der Bericht kommt

Der Doctor ist eine Fähigkeit des Daemons (RPC `Doctor`, ADR-018); der
Bildschirm „Einrichten" zeigt dieselben Zeilen über denselben Aufruf. Läuft kein
Daemon, führt die Kommandozeile dieselben Prüfungen aus demselben Crate im
eigenen Prozess aus; ein Doctor, der einen laufenden Daemon bräuchte, wäre genau
in dem Fall nutzlos, für den es ihn gibt. Welche der beiden Quellen es war, steht
in der Ausgabe (`source`), weil `PATH` und `$XDG_RUNTIME_DIR` des Daemons die
seiner Unit sind und nicht die des Terminals.

Zwei Zeilen kommen immer vom Client: `daemon`, weil nur ein Client weiß, ob er
den Daemon erreicht, und `llm`, weil dahinter eine Verbindung stünde.

### Exit-Codes

| Code | Bedeutung |
|---|---|
| `0` | jede Zeile ist `ok` oder `warn` |
| `3` | mindestens eine Zeile ist `fail`; so startet die Sandbox nicht |
| `1` | die Konfiguration ließ sich nicht lesen |

Ein fehlender Daemon ist **nicht** Exit 2: Er ist eine Zeile des Berichts.

### `--json`

Ein Wert auf `stdout`, die Schlüssel in alphabetischer Reihenfolge (so schreibt
`serde_json` eine Map), die Zeilen in Anzeigereihenfolge:

```json
{
  "checks": [
    { "evidence": "bubblewrap 0.12.0 at /usr/bin/bwrap", "id": "bwrap", "status": "ok" },
    {
      "diagnostic": {
        "code": "DOCTOR_013",
        "docs": "…/docs/DIAGNOSTICS.md#doctor_013",
        "fix": { "command": "humanitl doctor --probe-llm", "kind": "copy_command" },
        "severity": "warning",
        "title": "Sprachmodell nicht angesprochen",
        "why": "http://192.168.1.50:11434 was not contacted; …"
      },
      "evidence": "http://192.168.1.50:11434 was not contacted",
      "id": "llm",
      "status": "warn"
    }
  ],
  "source": "local",
  "status": "warn"
}
```

`source` ist `daemon` oder `local`, `status` der schlechteste Zustand im Bericht,
`checks` die elf Zeilen in Anzeigereihenfolge. `diagnostic` ist genau dann
vorhanden, wenn die Zeile nicht `ok` ist. Ein Beleg, der mit `not measured:`
beginnt, ist eine Zeile ohne Messung.

## `humanitl daemon install`

Schreibt die systemd-User-Unit, die den Daemon bei jeder Anmeldung startet. Es ist
der eingreifendste Befehl dieses Produkts außerhalb der Sandbox, und deshalb ist
hier vollständig beschrieben, was er tut.

```
humanitl daemon install [--print] [--no-start] [--bin-dir DIR]
humanitl daemon install --refresh
```

**Höchstens eine Datei, an einem benannten Ort.**
`$XDG_CONFIG_HOME/systemd/user/humanitld.service`, sonst
`~/.config/systemd/user/humanitld.service`, mit den Rechten `0644`. Keine
System-Unit, kein `sudo`, keine zweite Datei, kein `humanitld.socket`. Ihr
`ExecStart` nennt das `humanitld` **neben der laufenden Kommandozeile**, nie eines
aus `PATH` und nie eines aus einem Konfigurationswert, damit bei der Anmeldung
dieselbe Version startet wie die, die die Unit geschrieben hat.
`--bin-dir DIR` nennt stattdessen ein anderes Verzeichnis, etwa das Bundle eines
Pakets, zu dem die Kommandozeile über einen Link gelangt ist. Aus einem AppImage
(`$APPIMAGE` gesetzt) müssen außerdem `share/humanitl/catalog/domains.yaml` und
`profiles/sandbox/default.toml` unter `DIR/..` liegen wie im Image, sonst endet
der Befehl mit `DAEMON_011`.

**Mit dem Paket wird nichts geschrieben.** Liegt
`/usr/lib/systemd/user/humanitld.service` vor, hat das Paket die Units abgelegt,
und der Befehl schreibt keine Datei (HUM-053). Er führt nur aus, was das Paket
nicht für dich tut: `systemctl --user daemon-reload` und
`systemctl --user enable --now humanitld.socket` (das Paket lädt neu und startet
neu, was schon läuft, HUM-284, aktiviert aber nie einen Dienst). Nur den Socket
(HUM-164): Findet ein Client kein Token, öffnet er den Socket trotzdem einmal,
systemd startet dann den Dienst, und der Client wartet höchstens zehn Sekunden auf
sein Token. Pakete ohne Socket-Unit bekommen `humanitld.service`. Die Ausgabe
nennt die Unit des Pakets als `unit` und das Wort `packaged` als `action`; unter
`--json` stehen die Namen in `units`. Nach der Aktivierung enthalten `unit` und
`exec_start`, was systemd wirklich geladen hat
(`systemctl --user show -p FragmentPath,ExecStart`), nicht was in der Datei des
Pakets steht; `unit_text` ist dann der Text dieser geladenen Datei und `null`,
wenn er sich nicht lesen lässt. Liegt unter
`~/.config/systemd/user/`
noch die eigene Unit einer früheren Installation (die erste Zeile ist die
Markierung), würde sie die des Pakets verdecken: Der Befehl kündigt das an, legt
sie zusammen mit dem Aktivierungslink als `humanitld.service.bak` beiseite (oder
`.bak.N`, wenn der Name vergeben ist; `set_aside` in der Ausgabe), startet den
Dienst neu und legt beide zurück, wenn das scheitert (HUM-211). Beiseitegelegt
wird nur, wenn auch aktiviert wird: unter `--print`, `--no-start` oder ohne
`systemctl` bleibt sie, wo sie ist, und erscheint als `shadowed_by` in der
Ausgabe. Eine Datei dort ohne die Markierung bleibt, wo sie ist, und der Befehl
bricht mit `DAEMON_005` ab, auch unter `--print`.
Aus einem AppImage und mit `--bin-dir` gilt dieser Weg nicht: Beide nennen
ausdrücklich einen anderen Daemon als den des Pakets. Ohne Paket schreibt der
Befehl nie eine Socket-Unit; der Daemon bindet seinen Socket dann selbst.

**Aus einem AppImage werden Dateien kopiert.** Läuft die Kommandozeile aus einem
AppImage (`$APPIMAGE` ist gesetzt), liegen `humanitld` und `humanitl-shim` unter
`/tmp/.mount_*`, und dieser Pfad verschwindet mit dem Prozess. Ein `ExecStart`,
das dorthin zeigt, zeigte bei der nächsten Anmeldung ins Nichts. Beide Binaries
werden deshalb nach `~/.local/lib/humanitl/<version>.<stamp>/bin/` kopiert, und
mit ihnen der Domain-Katalog (`share/humanitl/catalog/`) und das Sandbox-Profil
(`profiles/sandbox/default.toml`), im selben Aufbau wie im Image: Der Daemon sucht
beides relativ zu seinem eigenen Pfad und liefe ohne sie mit leerem Katalog und
ohne Profil (HUM-165). Der Link `~/.local/lib/humanitl/current` zeigt auf diese
Kopie, und `ExecStart` nennt `current/bin/humanitld`: Ein Update legt eine neue
Kopie daneben und biegt den Link um, ohne die Unit anzufassen. Fehlt eines der
beiden Binaries, endet der Befehl mit `DAEMON_007`; fehlen Katalog oder Profil im
Image, mit `DAEMON_011`; in beiden Fällen ist nichts kopiert worden. Neben `bin/`
enthält jede Kopie `humanitld` als relativen Link auf `bin/humanitld`: Eine Unit
von vor HUM-165 nennt `current/humanitld`, und bricht ein Lauf ab, nachdem
`current` umgebogen wurde, aber bevor die neue Unit geschrieben ist, startet die
Unit so trotzdem den Daemon der neuen Kopie. Kopiert wird erst nach der
Ankündigung und nach jeder Prüfung, die den Lauf abweisen kann; eine fremde Unit
(`DAEMON_005`) oder eine fehlende User-Session (`DAEMON_010`) lassen
`~/.local/lib/humanitl` deshalb unberührt.

Jede Kopie bekommt ein neues Verzeichnis (`<stamp>` aus Uhrzeit und
Prozessnummer), auch wenn dieselbe Version ein zweites Mal installiert wird: Das
Verzeichnis, auf das `current` gerade zeigt, wird nie angefasst, bevor `current`
auf eine vollständige neue Kopie zeigt. Ein Abbruch auf halbem Weg hinterlässt
deshalb nie einen Link, der ins Nichts zeigt. `current` wird über einen zweiten
Link und `rename` umgebogen, nie gelöscht und neu angelegt. Zeigte `current`
vorher auf eine andere Kopie, läuft der Dienst womöglich noch aus ihr; der Befehl
startet ihn deshalb nach `enable --now` mit `systemctl --user try-restart
humanitld.service` neu, und erst danach verschwinden die vorige Kopie und jede
ältere, die ein früherer Lauf zurücklassen musste (HUM-077). Der Neustart
geschieht bei jeder Kopie, auch beim ersten Lauf und wenn `current` fehlte: Lief
schon ein Dienst dieses Namens, etwa aus einer Unit unter `/etc`, die die neue
verdeckt, ließe `enable --now` ihn weiterlaufen. Eine Kopie verschwindet nur, wenn
danach ein Daemon genau dieser Version antwortet und kein Prozess sein Programm
aus ihr haben kann (`/proc/<pid>/exe`, HUM-210): Ein Daemon, der noch aus ihr
läuft, fände sonst seinen Shim nicht mehr, und jede Sandbox scheiterte mit
`SANDBOX_011`.
Im Zweifel bleibt sie: wenn `/proc` oder einer seiner Einträge sich nicht lesen
lässt; wenn `systemctl --user show -p MainPID` scheitert, keine Zeile `MainPID=`
oder keine Zahl liefert (nur `MainPID=0` bedeutet „kein Hauptprozess"); wenn der
Hauptprozess, den es nennt, nicht sichtbar ist; wenn ein Prozess sein Programm
nicht lesen lässt und sein `status` nicht sagt, wem er gehört und wie er heißt;
oder wenn ein Prozess `humanitld` desselben Kontos sein Programm nicht lesen
lässt. Eine Kopie, die zurückbleibt, räumt ein späterer Lauf auf. Auch die neue
Kopie eines gescheiterten Laufs bleibt nach denselben Regeln, solange ein Prozess
aus ihr laufen kann. Ohne Neustart (`--no-start`, kein `systemctl`) bleibt die
vorige Kopie zurück. Scheitert der Neustart, zeigt `current` wieder auf die
vorige Kopie, die Unit bekommt ihren alten Text zurück, der alte Daemon wird noch
einmal gestartet, und die Diagnostic ist `DAEMON_008`. Gehen `current` oder die
Unit nicht nachweislich zurück, startet der Befehl nichts mehr, weil er sonst die
gerade gescheiterte Kopie starten würde; die Diagnostic nennt dann, was nicht
zurückging. Dasselbe gilt für eine ersetzte Unit auf dem Weg ohne AppImage: Wer
`daemon install` aus einem neuen Archiv aufruft, bekommt den Daemon des neuen
Archivs sofort und nicht erst bei der nächsten Anmeldung. Ist
`~/.local/lib/humanitl` ein Link oder gehört es einem anderen Konto als das
Home-Verzeichnis, wird nichts kopiert (`DAEMON_011`). Scheitert nach dem Kopieren
etwas anderes, die Unit oder `systemctl`, zeigt `current` wieder dorthin, wohin es
vorher zeigte, und die neue Kopie verschwindet wieder. `--print` nennt denselben
Pfad `…/current/bin/humanitld`, den die Unit bekäme, und kopiert nichts.

**`--refresh`: Ein neueres AppImage erneuert den Dienst.** `AppRun` ruft bei
jedem Start der Anwendung `humanitl -q daemon install --refresh` auf (HUM-077).
Der Schalter tut nur etwas, wenn der Lauf aus einem AppImage kommt, die Unit unter
`~/.config/systemd/user/` die Markierung trägt und
`ExecStart=~/.local/lib/humanitl/current/bin/humanitld` nennt (oder
`current/humanitld`, wie vor HUM-165; die Unit bekommt dabei den neuen Pfad),
und `current` auf
die Kopie einer anderen Version zeigt; dann läuft alles wie oben beschrieben,
einschließlich des Neustarts. Sonst endet er sofort mit 0 und schreibt, warum
nicht: `not_appimage`, `not_installed` oder `up_to_date` (unter `--json` als
`action`, dazu `installed` und `version`). Die Ersteinrichtung macht er nie: Eine
Unit, die bei jeder Anmeldung einen Dienst startet, legt kein Programmstart
ungefragt an; das bleibt der Knopf in „Einrichten" oder
`--cli daemon install`. Und wer den Dienst mit `daemon uninstall` entfernt hat,
bekommt ihn nicht beim nächsten Start des AppImage stillschweigend zurück, weil
die Unit fehlt. `--refresh` verträgt sich nicht mit `--print`, `--no-start` und
`--bin-dir`.

**Ohne User-Session wird nichts geschrieben.** Fehlt `XDG_RUNTIME_DIR` und soll
die Unit gestartet werden (kein `--print`, kein `--no-start`), endet der Befehl
vor dem ersten Schreiben mit `DAEMON_010` und dem Vorschlag
`loginctl enable-linger $USER`. Dasselbe gilt, wenn `systemctl --user` den
Session-Bus nicht findet („Failed to connect to bus"); dann ist die Unit schon
zurückgenommen, wenn die Diagnostic erscheint.

**Am Ende wartet er.** Sobald systemd die Unit übernommen hat, fragt der Befehl
bis zu fünf Sekunden lang `GetInfo`, bis der Daemon antwortet, und schreibt die
Version, die dieser nennt, als letzte Zeile. Antwortet er nicht, lautet die Zeile
`no answer within 5000 ms`; das ist eine Beobachtung und kein Grund, die
Installation zurückzunehmen: Die Unit ist an ihrem Platz, und
`humanitl daemon logs` sagt, woran es hängt.

**Sichtbar, bevor es geschieht.** Der ganze Text der Unit und beide
`systemctl`-Aufrufe gehen vor dem ersten Schreiben nach `stderr`, an der
Ausgabesteuerung vorbei: `-q` schaltet das nicht ab. Unter `--json` liest ein
Programm, und für es gilt der Vertrag dieser Kommandozeile, ein Objekt auf
`stdout`, `stderr` leer; Text und Befehle stehen dann im Objekt als `unit_text`
und `commands`. `--print` zeigt dieselbe Datei und schreibt nichts.

**Wiederholbar.** Ein zweiter Aufruf mit demselben Ergebnis schreibt nicht noch
einmal; die Ausgabe sagt dann `unchanged` statt `created`.

**Nie über fremdes Eigentum.** Die erste Zeile der Unit ist die Markierung
`# humanitl daemon install: written by Humanitl`. Fehlt sie in einer vorhandenen
Datei, gehört die Datei jemand anderem, und der Befehl verweigert mit
`DAEMON_005`, statt sie zu überschreiben. Ein `--force` dafür gibt es bewusst
nicht: Wer eine eigene Unit betreibt, legt sie beiseite.

**Danach `systemctl --user`, nie `sudo`.** Ohne `--no-start` laufen
`systemctl --user daemon-reload` und `enable --now`, je nach Weg:

- *Archiv und AppImage:* `systemctl --user enable --now humanitld.service`, auf
  der gerade geschriebenen Unit. Der Daemon bindet seinen Socket selbst.
- *Units des Pakets:* `systemctl --user enable --now humanitld.socket
  humanitld.service`, auf den Units unter `/usr/lib/systemd/user/`. Dabei wird
  nichts geschrieben. Beide Units, weil ein Client das Token liest, bevor er den
  Socket öffnet, und nur der laufende Daemon es schreibt.

Gibt es kein `systemctl` im `PATH`, startet nichts, und die Ausgabe nennt die
beiden Befehle.

**Ein Fehlschlag hinterlässt nichts.** Nimmt systemd die Unit nicht an, wird der
vorige Zustand wiederhergestellt: Eine angelegte Datei verschwindet, eine
ersetzte bekommt ihren alten Inhalt zurück, die Units des Pakets bleiben, wie sie
sind, und `daemon-reload` läuft noch einmal, damit auch das Bild stimmt, das
systemd davon hat. Scheitert `enable --now`, werden zuerst alle genannten Units
gestoppt (`stop`, `reset-failed`). Die Diagnostic ist `DAEMON_008`.

Auch die Aktivierung wird zurückgenommen. `systemctl --user enable --now` ist ein
Aufruf mit zwei Schritten: Er legt die Links an und startet dann. Die Links
liegen im Unit-Verzeichnis des Nutzers, auch für die Units des Pakets:
`default.target.wants/humanitld.service` und beim Paket stattdessen
`sockets.target.wants/humanitld.socket`. Scheitert der Start, blieben sie ohne
dieses Zurücknehmen liegen, und der Dienst startete bei der nächsten Anmeldung,
obwohl der Befehl mit `DAEMON_008` abgebrochen hat. Entfernt werden nur die
Links, die dieser Aufruf angelegt hat: Wer den Dienst vorher aktiviert hatte,
behält die Aktivierung.

**Es zählt das Verzeichnis des systemd-Managers** (HUM-217). Die Kommandozeile
schreibt die Unit unter ihr eigenes `XDG_CONFIG_HOME`; ist die Variable nur in der
rc-Datei der Shell gesetzt, sucht der Manager woanders. Deshalb fragt der Befehl
nach `daemon-reload` mit `systemctl --user show -p FragmentPath
humanitld.service`, aus welcher Datei systemd die Unit lädt. Ist es nicht die
geschriebene, geht die Unit zurück, bevor `enable` einen Link anlegt, und
`DAEMON_008` nennt beide Verzeichnisse; der Vorschlag `systemctl --user show
-p UnitPath` zeigt, wo der Manager sucht. Sagt systemd nichts zu
`FragmentPath`, entscheiden diese Suchpfade (`systemctl --user show -p UnitPath`):
Ist das Verzeichnis der Unit nicht darunter, ist das dieselbe Diagnostic. Unter
`--no-start` gilt dieselbe Prüfung über die Suchpfade, ohne dass systemd etwas neu
einliest. Gibt systemd auf keins von beiden eine Antwort, gilt die Unit als
ungeprüft und geht ebenfalls zurück; nur wenn `systemctl` den Session-Bus gar
nicht erreicht, bleibt sie unter `--no-start`, weil dieser Weg keine Session
braucht. Vor `enable` notiert der Befehl mit `systemctl --user
is-enabled`, welche Units schon dauerhaft aktiviert waren (eine Aktivierung nur
unter `/run` zählt nicht), und nach einem Fehlschlag deaktiviert er die anderen
mit `systemctl --user disable`, auch nach einem gescheiterten Neustart: systemd
entfernt die Links dort, wo der Manager sie angelegt hat. Die Suche im eigenen
Unit-Verzeichnis bleibt als Rückfall. Sagt `is-enabled` zu einer Unit nichts,
aktiviert der Befehl nicht und nimmt die Unit zurück.

Die Härtung der Unit ist gemessen, nicht behauptet (HUM-044, gehärtet in
HUM-053). Die Escape-Tests laufen unter genau ihren `[Service]`-Zeilen grün, mit
124 von 124 Fällen. Die Zeilen, die dafür da sein müssen:
`SystemCallFilter=@system-service @mount @sandbox sethostname`:
`@mount`, weil `bubblewrap` einhängt und `pivot_root` aufruft; `@sandbox`, weil
der Filter einer Unit an jedes Kind vererbt wird und `humanitl-shim` mit
`seccomp(2)` seinen eigenen seccomp-Filter installiert (ohne den Aufruf stirbt der
Shim an `SIGSYS`, bevor die dritte Sandbox-Garantie steht; unter systemd 262
enthielte `@system-service` ihn über `@default` ohnehin, aber das steht in keiner
Zeile von `systemd.exec(5)`); `sethostname`, weil `bubblewrap` den UTS-Namespace
der Sandbox `sandbox` nennt und sonst mit „Can't set hostname to sandbox"
abbricht. `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK`:
Ohne `AF_NETLINK` bringt `bubblewrap` `lo` im Namespace nicht hoch
(„loopback: Failed to create NETLINK_ROUTE socket"), und ohne `lo` gibt es keine
Bridge vom Shim zum Proxy. Dazu `NoNewPrivileges`,
`ProtectSystem=strict`, `PrivateUsers`, `ProtectProc=invisible`,
`CapabilityBoundingSet=CAP_SYS_ADMIN` und mehr; jede Zeile, auch jede bewusst
fehlende, steht mit ihrem Grund in `docs/INSTALL.md#hardening`, und
`systemd-analyze security` bewertet die Unit mit 3,7. Jeder Pfad in
`ReadWritePaths` trägt ein `-`, weil systemd eine Unit mit einem nicht
existierenden Pfad darin gar nicht erst startet, und auf einer frischen
Installation existiert keiner der drei. Gemessen am 2026-09-18 auf Debian 14 mit
bubblewrap 0.12.0 und systemd 262.

**`PrivateTmp=yes` hat eine Folge, die du kennen musst: Ein Projektordner unter
`/tmp` funktioniert nicht.** Die Unit gibt dem Daemon ein eigenes privates
`/tmp`; er sieht das `/tmp` der Anmeldesitzung nicht. Ein Ordner wie `/tmp/repo`
existiert für ihn deshalb nicht, `bwrap` kann ihn nicht einhängen, und der Start
endet mit `SANDBOX_006` und dem Satz „the project directory from … is
/tmp/repo: it cannot be read as a directory", obwohl der Ordner im Terminal
gleich daliegt. Dasselbe gilt für alles unter `/var/tmp`. Wer ein Projekt aus
`/tmp` heraus moderieren will, legt es an einen dauerhaften Ort, unter `$HOME`
oder irgendwo sonst außerhalb von `/tmp`, oder startet den Daemon von Hand
(`humanitld`) statt über die Unit. Die Zeile bleibt trotzdem in der Unit: Ein
geteiltes `/tmp` ist der Weg, auf dem ein anderer Prozess des Nutzers dem Daemon
eine Datei unterschiebt, und ein Projektordner in einem Verzeichnis, das beim
nächsten Start verschwindet, ist ohnehin kein Ort für Arbeit.

## `humanitl daemon uninstall`

Das Gegenstück zu `daemon install` (HUM-077): deaktiviert den Dienst und entfernt,
was `daemon install` angelegt hat.

```
humanitl daemon uninstall [--purge-binaries]
```

**Erst deaktivieren, dann entfernen.** Zuerst läuft `systemctl --user disable
--now` für die Units, die es gibt: `humanitld.service`, wenn die Unit mit der
Markierung unter `~/.config/systemd/user/` liegt, beim Paket
`humanitld.socket humanitld.service`, obwohl `daemon install` seit HUM-164 nur den
Socket aktiviert: Ein früherer Lauf hat auch den Dienst aktiviert.
Scheitert der Aufruf, ist noch nichts
entfernt, und die Diagnostic `DAEMON_014` nennt genau diesen Aufruf zum Kopieren;
findet `systemctl` den Session-Bus nicht, ist es `DAEMON_010`.
Dann verschwinden:

- jeder Aktivierungslink unter `~/.config/systemd/user/*.wants/` und
  `*.requires/` für diese Units (ohne `systemctl` entfernt der Befehl sie selbst,
  sonst die Reste, die `disable` zurückgelassen hat);
- die Unit `~/.config/systemd/user/humanitld.service`, nur wenn sie die
  Markierung trägt;
- der Socket `$XDG_RUNTIME_DIR/humanitl/daemon.sock`, nur wenn er ein Socket ist
  und niemand mehr darauf lauscht. Antwortet dort noch ein Daemon, den jemand von
  Hand gestartet hat, bleibt er unberührt;
- mit `--purge-binaries` die Kopien aus einem AppImage: der Link
  `~/.local/lib/humanitl/current`, übrig gebliebene Zwischenlinks
  `current.tmp-*` und jedes eigene Verzeichnis der Form
  `<version>.<stamp>`, danach `~/.local/lib/humanitl` selbst, wenn es leer ist.
  Was anders heißt, ein Link ist oder einem anderen Konto gehört, bleibt zurück.
  Die Kopien gehen erst hier, weil erst jetzt kein Dienst mehr aus ihnen läuft.

Antwortet nach dem Deaktivieren noch ein Daemon auf dem Socket, etwa einer, den
jemand von Hand gestartet hat, entfernt `--purge-binaries` nichts: Er läuft
womöglich genau aus diesen Kopien. Die Diagnostic ist `DAEMON_014` mit dem
Vorschlag `pkill -x humanitld`.

Danach `daemon-reload` und `reset-failed`, deren Ergebnis nicht zählt.
Bleibt etwas übrig, endet der Befehl mit `DAEMON_014` und nennt jeden Pfad, der
übrig blieb.

**Nie über fremdes Eigentum.** Eine Unit ohne die Markierung hat Humanitl nicht
geschrieben; der Befehl deaktiviert nichts, entfernt nichts und endet mit
`DAEMON_005`. Die Units des Pakets unter `/usr/lib/systemd/user/` werden
deaktiviert, bleiben aber liegen, bis das Paket geht
(`sudo apt remove humanitl`); ein Hinweis sagt das. Konfiguration, Aufzeichnung
und Audit-Log bleiben ebenfalls liegen; sie gehören dem Menschen.

Ohne Unit und ohne Kopie endet der Befehl mit 0 und
`activation: nothing`. Die Ausgabe nennt `unit`, `units`, `activation`
(`disabled`, `no systemctl`, `nothing`), `removed`, `binaries` und unter
`--json` `package`, den Pfad der Dienst-Unit des Pakets oder `null`.

Danach zeigt `humanitl doctor` in der Zeile `daemon` die Diagnostic `DOCTOR_006`,
mit dem Vorschlag `InstallService`, also `humanitl daemon install`: Ohne
erreichbaren Daemon schlägt der Doctor den Weg vor, der den Dienst dauerhaft
einrichtet, und nicht `humanitld` im Vordergrund eines Terminals.

## `humanitl daemon status`

Fragt den Daemon, wer er ist.

```
humanitl daemon status [--json]
```

```
FIELD          VALUE
socket         /run/user/1000/humanitl/daemon.sock
unit           active
daemon         0.0.0
proto          1.0
session        7b1c…
agent session  none
capabilities   hold, rules, sandbox
```

`socket` ist der Pfad, an dem der Client sucht; `unit` ist die Antwort von
`systemctl --user is-active humanitld.service` und erscheint als `-`, wenn es kein
`systemctl` gibt: Ein Daemon, den jemand von Hand gestartet hat, ist kein Fehler,
und ein erfundenes `inactive` wäre eine falsche Auskunft über eine Unit, die
niemand installiert hat. `agent session` (unter `--json` `agent_session`) kommt
aus `Sandbox(Status)`: `running`, solange eine Sandbox gehalten wird (startend,
laufend, stoppend, oder ihr Agent hat geendet und niemand hat sie gestoppt),
`none`, wenn nichts gehalten wird, und `unknown`, wenn der Daemon nicht innerhalb
von fünf Sekunden geantwortet hat. Das `postinst` des Pakets startet einen Daemon
nach einem Upgrade nur bei `none` neu (HUM-284); `session` ist die Proxy-Session
des Daemons und ist gesetzt, wann immer ein Daemon läuft. Alles andere kommt aus
`GetInfo` und wird nicht ergänzt.

Antwortet niemand, endet der Befehl mit **2** und `DAEMON_001`; spricht der Daemon
eine andere Hauptversion des Vertrags, ebenfalls mit 2 und `DAEMON_002`. Ein
Skript, das auf den Dienst wartet, prüft deshalb nur diese eine Zahl.

## `humanitl daemon retire-if-idle`

Bittet den laufenden Daemon, jetzt zu enden, wenn keine Agenten-Session ihn hält
(HUM-284).

```
humanitl daemon retire-if-idle [--json]
```

Der Daemon entscheidet unter derselben Sperre, unter der ein Start seinen
Anspruch anmeldet (`SandboxRequest.retire_if_idle`): Kein Start hält einen
Anspruch und keine Sandbox wird gehalten, und von diesem Moment an bekommt kein
Start mehr einen. Dann endet er mit seinem üblichen Abschied;
`systemctl --user restart humanitld.service` oder die nächste Verbindung über den
Socket startet das Programm, das jetzt installiert ist. Das `postinst` des
Pakets und der Knopf „Daemon neu starten" in der Statusleiste nutzen es nach einem
Upgrade.

Die Antwort ist ein Wort, unter `--json` das Feld `outcome`: `retired` (er endet
jetzt), `busy` (eine Session hält ihn, er läuft weiter), `unsupported` (ein Daemon
älter als Vertrag 1.19 kennt die Anfrage nicht), `none` (kein Daemon läuft: sein
Token oder sein Socket fehlt; der Befehl weckt keinen hinter dem Socket; jeder
andere Verbindungsfehler ist `unknown`) oder `unknown` (keine lesbare Antwort
innerhalb von fünf Sekunden; behandle es wie `busy`). Jede Antwort endet mit
**0**.

## `humanitl daemon logs`

Reicht das Journal des Dienstes durch.

```
humanitl daemon logs [-f] [-n N]
```

Der Befehl startet `journalctl --user -u humanitld.service` als Kind und gibt ihm
sein eigenes Terminal. `-n N` und `-f` werden unverändert weitergegeben. Endet
`journalctl` mit 0, endet der Befehl mit 0, sonst mit 1: Eine 2 oder 4 von
`journalctl` hieße hier „Daemon nicht erreichbar" oder „Sicherheitsverletzung",
und beides wäre falsch. Einen eigenen Leser für Journal-Einträge gibt es nicht: Er
wäre eine zweite Quelle für dieselbe Wahrheit, mit eigenen Formaten und eigenen
Fehlern.

Weil das Kind schreibt, wirkt `--json` hier nicht; wer JSON-Zeilen will, bekommt
sie von `journalctl` selbst (`humanitl daemon logs -- -o json` gibt es nicht;
dafür rufst du `journalctl --user -u humanitld.service -o json` direkt auf).

Fehlt `XDG_RUNTIME_DIR`, endet der Befehl mit `DAEMON_010` und dem einzigen
sinnvollen Vorschlag: `loginctl enable-linger $USER`. Über SSH ohne Linger gibt es
keine User-Session, in der ein User-Dienst leben könnte, und deshalb auch kein
Journal. Gibt es dagegen kein `journalctl` im `PATH`, fehlt ein Programm und
keine Session: `DAEMON_012` mit `sudo apt-get install systemd`.

## `humanitl config`

Die aufgelöste Konfiguration lesen, einen Wert schreiben, das Schema ausgeben,
die Datei von Hand öffnen.

```
humanitl config get [KEY] [--origin] [--json]
humanitl config set KEY VALUE [--project]
humanitl config schema [--profiles] [--json]
humanitl config edit
```

### `config get`

Ohne `KEY` wird jedes Blattfeld als Zeile gezeigt:

```
$ humanitl config get --origin
KEY                    VALUE    ORIGIN
hold.ask_mode          ui       default
hold.timeout_secs      300      default
llm.endpoint           -        default
sandbox.work_mode      rw       config.toml
```

Ohne `--origin` entfällt die dritte Spalte, damit die Tabelle in ein Terminal
passt; mit `--json` ist sie immer da, weil eine Spalte dort keine Breite kostet.

Mit `KEY` steht nur der Wert auf `stdout`: `$(humanitl config get
hold.timeout_secs)` ist also die Zahl und kein Satz darüber. Welche Schicht ihn
gesetzt hat, geht nach `stderr`; es landet nicht in einer Pipe.

Aufgelöst wird lokal, über dieselben sieben Schichten, die der Daemon beim Start
durchläuft (ADR-011). Ein laufender Daemon kann Werte halten, die nur er kennt,
etwa `hold.timeout_secs` aus einer Session; was hier gezeigt wird, ist die
Auflösung dieses Aufrufs.

### `config set`

Der Wert wird nach dem Typ des Schemas gelesen:

| Feld | Eingabe | steht in der Datei als |
|---|---|---|
| `hold.timeout_secs` | `5m` | `300` |
| `limits.hold_body_cap_bytes` | `32MiB` | `33554432` |
| `findings.enabled` | `true` | `true` |
| `ui.theme` | `dark` | `"dark"` |
| `llm.passthrough_paths` | `'["/v1/","/api/"]'` | die Liste |
| `sandbox.env` | `'{"FOO":"bar"}'` | die Tabelle |
| `llm.endpoint` | `null` oder `-` | nichts: Der Schlüssel verschwindet, die Vorgabe gilt |

Ein Wert, der mit `-` beginnt, ist ein Wert und kein Flag
(`humanitl config set hold.timeout_secs -5m` wird gelesen und abgewiesen, nicht
als unbekannte Option behandelt).

Eine Dauer nimmt `s`, `m`, `h` und `d`, eine Größe `KiB`, `MiB`, `GiB` und die
Formen ohne `i`. Beides gilt nur für Felder, deren Name entsprechend endet
(`_secs`, `_ms`, `_bytes`); das Schema selbst kennt keine Einheiten.

```
$ humanitl config set hold.timeout_secs 5m
hold.timeout_secs = 300 (global)
```

Geschrieben wird `$XDG_CONFIG_HOME/humanitl/config.toml` (mit `--config` die dort
genannte Datei), über denselben Schreiber, den der Daemon für `SetConfig` nutzt
(`humanitl_config::edit::set_value`). Vier Versprechen gelten:

- **Nur der eine Wert ändert sich.** Das Dokument wird mit `toml_edit` geändert
  und nicht neu serialisiert; Kommentare, Reihenfolge und Schreibweise bleiben,
  auch ein Kommentar hinter dem Wert, eine Byte-Order-Mark und CRLF-Zeilenenden.
  Eine verlinkte Datei bleibt verlinkt; geändert wird ihr Ziel. Danach wird das
  Ergebnis noch einmal gelesen und mit der alten Datei plus dem neuen Wert
  verglichen; weicht es ab, wird nichts geschrieben (`CONFIG_015`).
- **Nie eine halbe Datei.** Geschrieben wird in eine Nebendatei im selben
  Verzeichnis, dann `rename`.
- **Kein Schreiber verliert einen Wert.** Während des Schreibens ist das
  Verzeichnis der Datei mit `flock` gesperrt; ein zweites `config set` oder ein
  `SetConfig` des Daemons wartet, statt einen Wert zu überschreiben, den es nie
  gelesen hat.
- **Geprüft, bevor er in der Datei steht.** Zuerst gegen das Schema, Typ,
  Aufzählung, Grenzen, dann wird die fertige Nebendatei mit allen anderen
  Schichten geladen, so wie der nächste Start sie lädt: Profile, Umgebung und die
  anderen Werte der Datei. Ein Wert, der für sich richtig ist, aber nicht zu einem
  anderen passt (`limits.hold_body_cap_bytes` über
  `limits.hold_max_bytes`), landet so nie in der Datei. Die Quellen werden dafür
  mit der neuen Datei neu bestimmt: `config set sandbox.work_dir` wird mit dem
  Profil des Projekts geprüft, das es nennt, nicht mit dem des alten. Lädt die
  Konfiguration schon ohne die Änderung nicht, wegen eines anderen Werts der Datei
  oder einer Umgebungsvariablen wie `HUMANITL_HOLD__TIMEOUT_SECS=0`, zählt
  `config set` alle Diagnostics davor und danach: Das Laden meldet nur seine
  erste, also nimmt die Prüfung den Schlüssel jeder Diagnostic aus Datei,
  Projekt-Profil und Umgebung heraus und lädt erneut.
  Der Wert wird geschrieben, wenn der neue Wert keine Diagnostic bringt, die es
  vorher nicht gab, und keine den gesetzten Schlüssel nennt; `stderr` sagt dann,
  dass die Konfiguration immer noch nicht lädt. Dieselbe Diagnostic heißt
  derselbe Code, derselbe Schlüssel und derselbe Grund: Verschiebt der neue Wert
  die Grenze einer alten Diagnostic (`limits.hold_max_bytes` neu gesetzt, während
  `limits.hold_body_cap_bytes` schon darüber lag), ist das eine neue Diagnostic.
  So lässt sich eine Datei mit zwei falschen Werten Schlüssel für Schlüssel
  reparieren, und ein älterer Fehler verdeckt keinen neuen. Eine Diagnostic, die
  keinen Schlüssel nennt (ein unbekannter Schlüssel in der Datei, ein Profil, das
  nicht lädt), lässt sich so nicht zählen; dann wird nichts geschrieben, und der
  Vorschlag ist `humanitl config edit`, nie ein Wert aus einer anderen Schicht.
  Ein Wert, der schon genau so dasteht, wird ebenfalls geprüft; war er falsch,
  endet `config set` mit der Diagnostic. Ein Wert, der nicht passt, ist `CONFIG_003` und Exit 1, und die Datei
  bleibt unberührt:

```
$ humanitl config set hold.ask_mode banana
error[CONFIG_003]: Wert außerhalb des Bereichs
  why: banana is not a value of hold.ask_mode; it takes one of ui, terminal, none
  fix: humanitl config set hold.ask_mode ui
  docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#config_003
```

Jeder Vorschlag ist ein Befehl, der gelingt: bei einer Aufzählung ihr erster
Wert, sonst die Vorgabe des Felds (für `-5m` auf `hold.timeout_secs` ist das
`300`, nicht die Grenze `0`, die die Prüfung ebenfalls abweisen würde), und Listen
oder Tabellen darin stehen in einfachen Anführungszeichen.

Steht der Wert schon so da, wird nichts geschrieben; `--json` sagt dann
`"written": "unchanged"`.

Danach sagt `stderr`, wann der neue Wert wirkt: Läuft ein Daemon, nutzt er ihn ab
dem nächsten Sandbox-Start (Einstellungen des Proxys, der Aufzeichnung und der
Grenzen erst nach `systemctl --user restart humanitld`); läuft keiner, liest er
ihn, sobald er startet. Beides ist richtig, und deshalb sagt die Ausgabe, welcher
Fall vorliegt.

`--project` schreibt stattdessen in `<project>/.humanitl/profile.toml` unter
`[config]`. Das geht nur für Schlüssel, die das Projekt-Profil setzen darf: Diese
Datei liegt im geklonten Repository und ist fremder Text, und ein gesperrter
Schlüssel dort ist `CONFIG_003`, dieselbe Grenze, an der auch der Start eine
solche Datei abweist (`backlog/CONVENTIONS.md` 4.11). Die Grenze wird vor allem
anderen geprüft: Wer einen gesperrten Schlüssel ins Projekt schreiben will,
erfährt von der Grenze und nicht, dass sein Wert falsch geschrieben ist.

### `config schema`

Gibt das JSON-Schema aus, immer als JSON; ohne `--json` eingerückt. Jedes
Blattfeld trägt `description`, `x-tier` (die Sichtbarkeitsstufe des
Einstellungsbildschirms) und `x-project-scope` (die Vertrauensgrenze von oben).
`--profiles` gibt stattdessen die Profile aus, die `--profile` wählen kann.

### `config edit`

Öffnet `config.toml` in `$VISUAL`, sonst `$EDITOR`, sonst `nano`, sonst `vi`, und
prüft die Datei, nachdem sie geschlossen ist: zuerst, ob sie TOML ist, dann, ob
sie sich auflösen lässt. Scheitert das, wird die Diagnostic gezeigt, und an einem
Terminal folgt die einzige interaktive Frage dieser Kommandozeile:
„open it again? [y/N]". Ohne Terminal und unter `--json` gibt es keine Frage;
dann endet der Befehl mit der Diagnostic und Exit 1. Die Diagnostic wird in jedem
Fall genau einmal gezeigt, unter `--json` als ein Objekt. Findet sich kein Editor,
ist das `CONFIG_017` mit dem Vorschlag `export EDITOR=nano`.

## `humanitl audit`

Die Hash-Kette prüfen und die Einträge exportieren.

```
humanitl audit verify [--file PATH] [--json]
humanitl audit export --format jsonl|csv --out FILE [--since TS] [--until TS] [--file PATH]
```

### Was `verify` beweist und was nicht

Die Kette zeigt eine Änderung, Löschung oder Umstellung vor dem letzten Anker,
solange der Angreifer den HMAC-Schlüssel nicht hat (`docs/SECURITY.md`, „What the
chain proves"). Diese Aussage hängt an drei Dingen: der Kette selbst, den MACs
und den Ankern. Alle drei hat nur der Daemon, den Schlüssel aus dem
Schlüsselspeicher, die Anker aus der Tabelle `audit_anchors`.

Deshalb fragt der Befehl zuerst den Daemon (HUM-156). Er prüft jeden Eintrag mit
seinem Schlüssel und gegen die Anker und sagt das auch:

```
$ humanitl audit verify
audit chain: OK
records:     4213
head:        a3f9…c2e1 (seq 4213, 2026-09-02T10:42:01.000000Z)
hmac key:    checked by the daemon
anchors:     42 (last at 2026-09-02T10:40:00.000000Z), checked by the daemon
warnings:    unanchored tail: 13 records
checked by:  daemon
```

Der Kopf ist derselbe Hash, den der Audit-Bildschirm der Oberfläche zeigt: Beide
fragen denselben Daemon, und vor jeder Antwort schreibt er alles, was er bis
dahin geschrieben hat, auf die Platte.

Antwortet **kein** Daemon, keiner erreichbar, keiner, der das Token annimmt,
oder einer, der `Audit` noch nicht kennt, prüft die Kommandozeile die Datei
selbst, und das ist die **schwächere** Prüfung: Kette und kanonische Form, aber
keine MACs und keine Anker. Die Ausgabe sagt das, in jedem Format:

```
$ humanitl audit verify
audit chain: OK
records:     4213
head:        a3f9…c2e1 (seq 4213, 2026-09-02T10:42:01.000000Z)
hmac key:    not checked
anchors:     not checked
warnings:    the daemon is not reachable (cannot stat the session token /run/user/1000/humanitl/token: No such file or directory (os error 2)), so this is the file-mode check; no HMAC key (file mode); unanchored tail: 4213 records; no anchors (file mode)
checked by:  file:/home/nik/.local/share/humanitl/audit/audit.jsonl
```

Die erste Warnung sagt, warum die Datei geprüft wurde: „is not reachable", wenn
kein Daemon da ist, „does not answer Audit", wenn ein älterer Daemon den Aufruf
nicht kennt. Der Rest der Dateiprüfung kennt keine Anker, also gilt jeder Eintrag
als nicht verankert.

Ein Daemon, der antwortet und **verweigert**, weil sein Log oder seine Anker nicht
lesbar sind oder er ohne Audit-Log läuft (`IPC_006`), bekommt keine Dateiprüfung
als Ersatz. Seine Diagnostic steht auf `stderr`, und der Befehl endet mit ihrem
Exit-Code: Eine schwächere Antwort, die eine Verweigerung überdeckt, wäre die
falsche Auskunft.

`--file PATH` prüft ausdrücklich diese Datei, ebenfalls ohne Schlüssel und
Anker; das ist der Weg für ein Log, das von anderswo kommt.

Bricht die Kette, endet der Befehl mit **4** (Sicherheitsverletzung nach
`backlog/CONVENTIONS.md` 3.8), nennt die Stelle und schreibt `AUDIT_001` nach
`stderr`:

```
$ humanitl audit verify --file audit.jsonl
audit chain: BROKEN at seq 4012 (hash_mismatch)
records:     4011
hmac key:    not checked
anchors:     not checked
warnings:    no HMAC key (file mode); no anchors (file mode)
checked by:  file:audit.jsonl
error[AUDIT_001]: Hash-Kette gebrochen
  why: audit.jsonl: hash_mismatch at seq 4012; 4011 records before it hold
  fix: mv audit.jsonl audit.jsonl.broken-$(date -u +%Y%m%dT%H%M%SZ)
  docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#audit_001
```

Die ersten sieben Zeilen stehen auf `stdout`, die Diagnostic auf `stderr`.

Mit `--json` steht alles in einem Objekt auf `stdout`, die Diagnostic als Feld
`diagnostic`; `stderr` bleibt leer. `mode` sagt, welche Prüfung lief: `full`
beim Daemon mit Schlüssel und Ankern, `file` ohne beides; `hmac` und `anchors`
sagen dasselbe je Teil (`checked` oder `not_checked`). Beim Daemon kommen
`anchor_count` und `last_anchor_at` hinzu. `head.hash` ist der Hash, den der
Audit-Bildschirm zeigt; `head.ts` ist der Zeitstempel der Zeile dieses Eintrags,
Zeichen für Zeichen wie im Log (HUM-162). Nur ein Daemon, der
`AuditResponse.head_ts` noch nicht kennt, lässt ihn weg; dann steht dort
`null`.

### `audit export`

```
$ humanitl audit export --format csv --out audit.csv
exported 4213 records to audit.csv
```

Den Export schreibt der Daemon, wenn einer antwortet, sonst die Kommandozeile
selbst; beide nutzen denselben Code (`humanitl_audit_log::export`) und schreiben
dieselbe Datei. `jsonl` schreibt jede Zeile des Logs wörtlich; der Export bleibt so
gegen die Datei prüfbar. `csv` schreibt eine Kopfzeile und eine Zeile pro
Eintrag, mit den zwölf Spalten aus HUM-050:

```
seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash
```

Die acht Spalten zwischen `kind` und `hash` stammen aus `data` und bleiben leer,
wo der Eintrag das Feld nicht trägt; `flow.decided` etwa nennt weder Host noch
Methode, die stehen in `flow.received`. `data` als Ganzes steht nur im JSONL. Bis
HUM-156 schrieb die Kommandozeile acht Spalten
(`seq,ts,session,kind,data,prev,hash,mac`); die Übersicht der Oberfläche und die
der Kommandozeile sind seitdem dieselbe.

Felder mit einem Komma oder Anführungszeichen werden nach RFC 4180 in
Anführungszeichen gesetzt, doppelte Anführungszeichen verdoppelt, und jede Zeile
endet mit CRLF. JSON-Zeilen enden mit LF, wie im Log.

Eine schon vorhandene Datei wird nie überschrieben (`AUDIT_008`): Ein Export ist
ein Beweismittel, und ein `--force` gibt es aus demselben Grund nicht wie bei
`daemon install`. Geschrieben wird in eine Nebendatei, und erst der fertige Export
bekommt seinen Namen; ein Log mit einer Zeile, die kein Eintrag ist
(`AUDIT_001`), hinterlässt keine halbe Datei, die den nächsten Versuch abweisen
würde. Eine gebrochene Kette wird unverändert exportiert; den Bruch meldet
`humanitl audit verify` (HUM-214). Die Nebendatei eines abgebrochenen Exports
räumt der nächste Export in dasselbe Verzeichnis auf, aber nur, wenn das
Verzeichnis dem eigenen Konto gehört und weder Gruppe noch andere hineinschreiben
dürfen; in einem geteilten Verzeichnis bleibt sie liegen. Ein relatives `--out`
wird vor dem Aufruf des Daemons gegen das eigene Verzeichnis aufgelöst: Der Daemon
läuft woanders. Läuft er unter systemd mit `PrivateTmp=yes`, hat er ein eigenes
`/tmp`; ein Export nach `/tmp` oder `/var/tmp` wird dann mit `AUDIT_008`
abgewiesen, statt in einem Verzeichnis zu landen, das nur der Daemon sieht. Der
Daemon legt kein Verzeichnis an und schreibt auf dem Weg zum Ziel nicht durch
einen Link; die Kommandozeile legt das Verzeichnis vorher selbst an.

`--since` und `--until` nehmen Zeitstempel nach RFC 3339 und schneiden halboffen
(`since` ist enthalten, `until` nicht), sodass ein Eintrag nie in zwei
aufeinanderfolgenden Exporten erscheint. Sie werden an den Daemon weitergegeben:
Der Vertrag (`AuditRequest.Export.from` und `.to`) enthält beide Grenzen, und weil
das Log Mikrosekunden schreibt, schickt die Kommandozeile als obere Grenze die
letzte volle Mikrosekunde vor `--until`. Das Ergebnis ist dasselbe wie in der
Fassung, die auf der Datei arbeitet.

Was in keinem Eintrag und deshalb auch in keinem Export steht: Bodys,
Klartextwerte, die Originale von Pseudonymen und die Notiz einer Entscheidung.
Die steht in der Aufzeichnung (`flows.decision_note`, `humanitl flows show`),
nicht im Audit-Log (HUM-117).

## `humanitl llm discover`

Sucht im lokalen Netz nach Modellservern. Nach `daemon install` ist es der zweite
Befehl, der über deinen eigenen Rechner hinausreicht, und deshalb ist hier
vollständig beschrieben, was er tut.

```
humanitl llm discover [--subnet CIDR] [--port PORT]... [--json]
```

**Nur auf Aufruf, und vorher angekündigt.** Ohne diesen Befehl wird keine
einzige Verbindung aufgebaut; die Oberfläche hat denselben Weg hinter einem
Knopf mit demselben Text. Vor dem ersten Paket geht auf `stderr` eine Zeile
hinaus, die das Netz und die Ports nennt, an der Ausgabesteuerung vorbei, damit
auch `--json` sie nicht verschluckt.

**In deinem eigenen `/24` und nirgends sonst.** Ohne `--subnet` leitet der Daemon
das Netz aus der Standardroute ab: Die Routing-Tabelle nennt Interface und
Gateway, ein verbundener UDP-Socket (der nichts sendet) nennt deine eigene
Adresse, und daraus ergibt sich das `/24` darum herum. Ein `--subnet`, das weiter
als ein `/24` ist, wird mit `LLM_008` abgewiesen, bevor irgendetwas verbindet: Ein
`/16` wären 65.534 Verbindungsversuche in ein Netz, das dem Aufrufer womöglich
nicht gehört.

**Vier Ports, zwei Schritte.** Gefragt werden die Ports `11434` (Ollama), `1234`
(LM Studio), `8000` (vLLM und die meisten Python-Server) und `8080`
(llama.cpp); `--port` ersetzt diese Liste. Zuerst ein Verbindungsversuch pro
Adresse und Port mit einer Frist von 200 ms, 64 gleichzeitig; für jeden offenen
Port dann dieselbe Probe wie beim Testknopf: `GET /api/tags`, sonst `GET
/v1/models`, ohne Zugangsdaten und ohne Weiterleitungen. Ein `/24` ohne einen
einzigen Fund ist so in unter vier Sekunden erledigt.

**Was in der Liste steht, hat der Server gesagt.** Produkt, Modelle und Latenz
stammen aus seiner Antwort; die Modellnamen werden gekürzt und bereinigt wie die
des Testknopfs. Ein Server, der `401` oder `403` sagt, verschwindet nicht,
sondern trägt `(auth required)`. Ein Server, der auf einem der Ports lauscht, aber
keine der beiden APIs beantwortet, erscheint als `unknown`: Er wird genannt,
nicht verschwiegen und nicht als Modellserver ausgegeben.

**Kein Flow und keine Warteschlange.** Die Suche läuft im Daemon im Netz des
Rechners, nie in der Sandbox. Sie erscheint in keiner Warteschlange und in keinem
Verlauf; sie ändert nichts und schreibt nichts.

Der Lauf endet mit `0`, auch wenn nichts geantwortet hat: „nichts gefunden" ist
ein Ergebnis und kein Fehler. Mit `--json` steht am Ende ein Wert mit `servers`
und `count`, sonst eine Tabelle, und während der Suche eine Zeile pro Fund auf
`stderr`.

## `humanitl rules test`

Fragt den geltenden Regelsatz, was mit einer Anfrage geschehen würde. Ausgewertet
wird im Daemon, mit derselben Engine wie im Proxy-Pfad (ADR-018): Eine zweite
Auswertung in der Kommandozeile könnte anders antworten als die, die wirklich
entscheidet.

```
humanitl rules test <URL> [--method M] [--upgrade websocket] [--json]

verdict: block
rule: 018f0000-0000-7000-8000-00000000000a (bundled, position 3)
```

Ohne `--method` gilt `GET`. Passt keine Regel, steht dort
`rule: none (default ask)`. Die Herkunft, `session`, `user` oder `bundled`, kommt
aus derselben Antwort wie das Urteil und nicht aus einem zweiten Aufruf.

**Der Exit-Code trägt das Urteil**, damit ein Skript es lesen kann, ohne die
Zeile zu parsen: `allow` endet mit `0`, `block` mit `10`, `ask` mit `11`.
Auch `redact` endet mit `11`, weil eine solche Anfrage heute angehalten wird. Ein
Fehlschlag ist davon nicht betroffen: Eine URL ohne Schema oder mit Fragment ist
`CLI_004` und Exit `1`, bevor jemand gefragt wird, und ohne Daemon endet auch
dieser Befehl mit `2`.

Mit `--json` steht eine Zeile auf `stdout`, mit `verdict`, `matched`,
`rule_id`, `origin`, `position` und `passthrough`; der Exit-Code bleibt derselbe.
`origin` und `passthrough` sind `null`, wo die Antwort die Regel nicht trägt:
zweimal dasselbe „unbekannt", nie ein `false`, das mehr behauptet, als bekannt
ist.

Normalisiert wird im Daemon: Groß- und Kleinschreibung, ein Punkt am Ende und ein
internationalisierter Name gehen roh hinaus und werden dort behandelt wie in einer
echten Anfrage. `tests/escape/esc-4-rules.sh` schickt fünfzehn Zeilen der
Host-Tabelle aus HUM-022 genau diesen Weg entlang.

## `humanitl llm test`

Fragt einen einzelnen Endpunkt, was er ist. Dieselbe Probe wie hinter dem
Testknopf in „Einrichten" und wie bei `doctor --probe-llm`: GET-Anfragen im
Daemon auf die Modellliste des Endpunkts (`<base>/models`, bei einem Ollama ohne
Pfad-Präfix zuerst `/api/tags`), ohne Weiterleitungen, nichts davon durch die
Sandbox. Zugangsdaten werden nur mit `--with-key` gesendet: Dann sendet der Daemon
den hinterlegten Zugangsschlüssel als `Authorization: Bearer`, und nur wenn `URL`
denselben Ursprung (Schema, Host, Port) hat wie `llm.endpoint`; `key_sent` in
`--json` sagt, ob einer gesendet wurde (HUM-140). Mit `--server NAME` wird
stattdessen der Schlüssel dieses gespeicherten Servers gesendet, und nur wenn
`URL` den Ursprung des Endpunkts dieses Servers hat; ein Schlüssel geht nie an den
Ursprung eines anderen Servers (HUM-289).

```
humanitl llm test <URL> [--timeout-ms N] [--with-key [--server NAME]] [--json]

flavor: ollama
latency: 12 ms
models: 2
  qwen2.5-coder:14b
  llama3.1:8b
```

Vor dem ersten Paket geht auf `stderr` eine Zeile hinaus, die den Endpunkt und die
beiden Anfragen nennt, an der Ausgabesteuerung vorbei, damit auch `--json` und
`-q` sie nicht verschlucken. Gezeigt werden höchstens sechs Namen mit je
höchstens 40 Zeichen, der Rest als `+N more`; dieselben Grenzen wie in der
Oberfläche, weil ein Name aus dem Netz Text von einer Maschine ist, über die noch
niemand entschieden hat. Die volle Liste steht in `--json`.

Ein Endpunkt, der nicht antwortet, endet mit `1` und der Diagnostic des Daemons
(`LLM_001`; `LLM_007`, wenn sich die Adresse nicht einmal lesen ließ).
Diagnostics, die kein Fehlschlag sind, stehen unter der Ausgabe, und der Lauf
endet mit `0`: `LLM_006` für eine Adresse außerhalb deines eigenen Netzes und
`LLM_003` für einen Server, der antwortet, aber als keine bekannte API; dann steht
dort `flavor: unknown`, es gibt keine Modellliste, und `answer:` (in `--json` der
Schlüssel `answer`) sagt, was stattdessen zurückkam, Status und Content-Type, etwa
`HTTP 404, text/html` (HUM-283). Wer geantwortet hat, hat geantwortet. Eine
Adresse mit einem Anführungszeichen, einer spitzen Klammer, einem Backslash oder
Leerraum darin wird vor jeder Anfrage mit `CONFIG_003` abgewiesen, mit Nennung des
Zeichens und seiner Position und mit der bereinigten Adresse als Angebot: Ein
solches Zeichen ist fast immer vom Kopieren der Adresse aus einer
Konfigurationsdatei übrig geblieben.

## `humanitl llm key`

Der Zugangsschlüssel des Endpunkts, für ein Modell, das nur mit Schlüssel
antwortet (HUM-140). Ein Aufruf des RPC `LlmKey`; derselbe, der das Feld
„Zugangsschlüssel" der Einstellungen bedient.

```
humanitl llm key status [--server NAME] [--json]
humanitl llm key set [--server NAME] [--json]    # reads the key from stdin
humanitl llm key remove [--server NAME] [--json]

access key: set (secret-service:humanitl/llm, secret-service)
```

`set` liest den Schlüssel von `stdin`, nie von der Kommandozeile: `argv` ist in
`/proc/<pid>/cmdline` für jeden Prozess desselben Nutzers sichtbar und in der
History der Shell. An einem Terminal ist die Eingabe verborgen; aus einer Pipe
wird alles bis zum Ende gelesen, ohne den abschließenden Zeilenumbruch
(`printf '%s' "$KEY" | humanitl llm key set`). Der Daemon legt ihn im
Schlüsselbund des Systems ab (`secret-tool`, Secret Service), sonst in
`$XDG_DATA_HOME/humanitl/keys/llm.key` mit `0600` und meldet dann `CONFIG_019`.
Danach enthält `config.toml` nur den Verweis `llm.api_key_ref`. Keine Ausgabe
trägt den Schlüssel, auch `--json` nicht: Sie nennt `set`, `store`
(`secret-service`, `file` oder `none`), `key_ref` und die Diagnostics.

`status` meldet `LLM_009`, wenn `llm.api_key_ref` einen Schlüssel nennt, den der
Speicher nicht herausgibt. `remove` entfernt Schlüssel und Verweis. Leerraum um
den Schlüssel und je ein Anführungszeichen an jedem Ende werden vor dem Ablegen
entfernt, sodass ein Schlüssel, der samt Anführungszeichen aus einer
Konfigurationsdatei kopiert wurde, funktioniert (HUM-283). Ein Schlüssel, der
danach leer ist, innen Leerraum enthält oder länger als 4096 Bytes ist, wird mit
`CONFIG_020` abgewiesen, ohne den Schlüssel zu wiederholen, und nichts wird
geschrieben. Wie bei jeder Einstellung gilt ein neuer Schlüssel ab der nächsten
Session; eine laufende behält, womit sie gestartet wurde.

`--server NAME` meint den Schlüssel dieses gespeicherten Servers (siehe
`humanitl llm server` unten); ohne das Flag den des aktiven Servers. Ein Server
ohne eigenen Schlüssel bekommt den Schlüsselbund-Eintrag
`secret-service:humanitl/<NAME>` (oder im Rückfall `file:<NAME>`); einer, der
schon einen Verweis hat, behält ihn, sodass der aus einer älteren Konfiguration
übernommene Server `default` `secret-service:humanitl/llm` behält. Den Schlüssel
eines Servers zu setzen, der nicht aktiv ist, berührt den Schlüssel des aktiven
nicht.

## `humanitl llm server`

Gespeicherte Modellserver, jeder mit Endpunkt, Modellen und Zugangsschlüssel,
umschaltbar mit einem Befehl (HUM-289). Ein Aufruf des RPC `LlmServers`;
derselbe, der den Serverwechsler in den Einstellungen und in der Statusleiste
bedient.

```
humanitl llm server ls [--json]
humanitl llm server add <NAME> <URL> [--model MODEL]... [--use] [--json]
humanitl llm server use <NAME> [--json]
humanitl llm server rename <NAME> <NEW> [--json]
humanitl llm server rm <NAME> [--use NEXT] [--keep-key] [--json]

   NAME   ENDPOINT                    MODEL  KEY
*  home   http://192.168.1.20:11434/  qwen3  secret-service:humanitl/llm
   cloud  https://api.example.com/v1  -      secret-service:humanitl/cloud
```

Die Einträge stehen in `config.toml` als `[llm.servers.<NAME>]` mit `endpoint`,
`models` und `api_key_ref`; `llm.active` nennt den aktiven. Die flachen Schlüssel
`llm.endpoint`, `llm.models` und `llm.api_key_ref` sind die Sicht des aktiven
Servers: `use` schreibt sie zusammen mit `llm.active` in einem Schritt, und wer
einen davon schreibt (das Einstellungsformular, `config set`), schreibt den
aktiven Eintrag mit. Nur der Ursprung des aktiven Servers bekommt den
hinterlegten Schlüssel und ist die LLM-Durchleitung der nächsten Session; eine
laufende Session behält den Server, mit dem sie gestartet wurde, und jeder
ändernde Befehl sagt das auf `stderr`.

Eine Konfiguration von vor HUM-289 mit nur den flachen Schlüsseln wird als Server
`default` aufgeführt; der erste ändernde Befehl schreibt sie als Eintrag, mit
demselben Schlüsselverweis. `add` prüft den Endpunkt wie `llm.endpoint`
(`CONFIG_003` bei einem Anführungszeichen oder Leerraum, bei einem anderen Schema
als http oder https); der erste Server wird aktiv, spätere nur mit `--use`. Sein
Schlüssel folgt mit `humanitl llm key set --server NAME`. `rename` lässt den
Schlüssel, wo er ist. `rm` löscht den Schlüsselbund-Eintrag des Servers, außer bei
`--keep-key`, und nur, wenn kein anderer Server ihn teilt; der aktive Server wird
nur mit `--use NEXT` entfernt, solange es andere gibt (sonst `CONFIG_021`), und
der letzte nimmt die flachen Schlüssel mit. Namen haben 1 bis 32 Zeichen aus
`a-z`, `0-9`, `-` und `_`. Die Tabelle zeigt nie einen Schlüssel, nur seinen
Verweis, und den Endpunkt ohne Nutzer, Passwort und Query.

## Die globalen Schalter

Fünf Schalter gelten für jedes Unterkommando, weil `clap` sie als `global` führt
(`daemon/bin/humanitl/src/cli.rs`, `GlobalOpts`). Sie stehen vor oder nach dem
Unterkommando; beides funktioniert.

| Schalter | Wirkung |
|---|---|
| `--json` | Maschinenlesbare Ausgabe: genau ein JSON-Wert auf `stdout`, Diagnostics eingeschlossen. Hinweise auf `stderr` entfallen; was ein Unterkommando im JSON-Modus schreibt, steht bei ihm. |
| `--config PATH` | Liest diese Konfigurationsdatei statt der im Konfigurationsverzeichnis des Nutzers. Der Pfad gilt für diesen Aufruf und ändert nichts an der Datei des Nutzers. |
| `-v`, `--verbose` | Erklärt, was geschieht: Hinweise, die sonst still bleiben (`Renderer::detail`). Mehrfach angeben geht (`-vv`), heute ohne weitere Stufe. |
| `-q`, `--quiet` | Nur das Ergebnis, keine Hinweise: `Renderer::note` schreibt nichts mehr nach `stderr`. Schließt `-v` aus; Diagnostics und Fehler bleiben. |
| `--profile NAME` | Das Profil der Session. Unter `humanitl sandbox` benennt dasselbe Flag das bwrap-Profil; welche Bedeutung gilt, entscheidet das Unterkommando. |

Der Ergebnistext eines Unterkommandos steht auf `stdout` und bleibt auch mit `-q`
dort; `-q` entfernt nur die begleitenden Hinweise. Eine Diagnose (`Diagnostic`)
geht nach `stderr` und lässt sich mit keinem der Schalter abschalten, weil sie den
Exit-Code erklärt.

## Was `run` mit den anderen Unterkommandos teilt

- `humanitl sandbox run` startet die Sandbox im Prozess der Kommandozeile und ist
  der Weg für Selbsttests und die Escape-Tests. `humanitl run` startet sie im
  Daemon; nur dort gibt es Proxy, Aufzeichnung und Warteschlange.
- `humanitl rules list` zeigt den gerade geltenden Regelsatz, nach einem Start
  also den der Session, einschließlich der Regeln ihres Profils und ihrer
  Durchleitung.
- `humanitl daemon status` sagt, ob es überhaupt einen Daemon gibt. Das ist die
  Antwort auf Exit 2.
- `humanitl doctor` sagt, ob `run` auf diesem Rechner überhaupt eine Sandbox
  bekommen kann. Das ist die Antwort auf Exit 3, bevor er eintritt.
- `humanitl daemon install` legt die Unit an, die den Daemon bei der Anmeldung
  startet. Das ist die Antwort auf die Zeile `daemon` des Doctors und auf
  `DAEMON_001` im Bildschirm „Einrichten".
