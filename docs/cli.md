# Die Kommandozeile

`humanitl` ist ein dünner Client des Daemons (ADR-018). Jede Fähigkeit ist
zuerst ein RPC; die Kommandozeile ruft ihn auf und formatiert die Antwort. Sie
enthält keine Fachlogik, und sie erfindet nichts dazu.

Dieses Dokument beschreibt `humanitl run`, `humanitl doctor`, `humanitl config`,
`humanitl audit` und die drei `humanitl daemon`-Kommandos vollständig und die
übrigen Unterkommandos nur so weit, wie sie `run` betreffen. Die kanonische
Liste aller Unterkommandos steht in `backlog/CONVENTIONS.md` 3.8, das Schema
aller Konfigurations-Flags in `docs/CONFIG.md`.

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
`backlog/CONVENTIONS.md` 4.26: Ein Client, der jeden Schlüssel setzen dürfte,
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
- **`terminal`** — der Mensch entscheidet im selben Terminal. Sobald eine
  Anfrage gehalten wird, steht ein Kasten auf `stderr`, während die Ausgabe des
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
  | `s` | Erlaubt sie und legt eine Regel für diesen Host an, gültig für diese Sitzung. |
  | `b` | Blockt sie. Der Agent bekommt `403`. |
  | `r` | Fragt Ziel (URL, Host, Apex, Host und Methode) und Dauer (einmal, Sitzung, dauerhaft), zeigt die Regel und legt sie nach `Enter` an. Zwei Ziele können scheitern und sagen es: ein Apex, den der Dienst nicht kennt (eine Adresse, ein unbekanntes Suffix), und eine Methode, die der Vertrag nicht benennt — eine Regel, die dann breiter wäre als das, was auf dem Schirm stand, entsteht nicht. |
  | `e` | Schreibt die Anfrage samt vollständigem Rumpf nach `$XDG_RUNTIME_DIR/humanitl/edit-<id>.http` (0600, neu angelegt, ohne einem Symlink zu folgen), öffnet `$VISUAL` oder `$EDITOR` (sonst `vi`) und schickt das Ergebnis als `allow_edited`. Die Datei wird nach dem Lesen gelöscht. Ohne `$XDG_RUNTIME_DIR` verweigert der Weg den Dienst, statt in ein Verzeichnis zu schreiben, in das jeder schreiben kann. Ein Rumpf, der kein UTF-8 ist, wird nicht bearbeitet: Was ein Editor daraus machte, ginge als etwas anderes hinaus, als der Agent geschickt hat. |
  | `v` | Zeigt die ersten 4 KiB des Rumpfs; was nicht druckbar ist, steht als Hex. |
  | `n` | Geht zur nächsten gehaltenen Anfrage, ohne zu entscheiden. |
  | `Esc`, `Ctrl+C` | Schließt den Kasten. Die Anfrage bleibt gehalten. |

  Der Kasten wird jede Sekunde neu gezeichnet, damit die Uhr oben rechts
  stimmt, und er verschwindet, sobald über die Anfrage entschieden ist — auch
  dann, wenn jemand anders sie im Fenster entschieden hat. Läuft die Frist ab,
  steht `[humanitl] timed out -> blocked` da, und der nächste gehaltene Fluss
  kommt.

  Er braucht ein Terminal auf der Eingabe und mindestens 40 Spalten. Ohne
  Terminal — in einer Pipe, in einem Skript — verweigert der Befehl den Dienst
  mit `CLI_002`: Bytes aus einer Pipe hat niemand als Antwort gemeint. Der
  Kasten ist außerdem ASCII; eine Breite in Zeichen ist nicht dieselbe wie eine
  Breite in Spalten, und ein doppelt breites Zeichen ließe jede Zeile umbrechen.
  Ein Pfad in einer anderen Schrift ist im Kasten deshalb nicht zu lesen; wer
  ihn lesen will, nimmt `humanitl flows show`. Solange er steht,
  wird die Ausgabe des Agenten angehalten (höchstens 256 KiB); danach geht sie
  hinaus und der Kasten wird darüber neu gezeichnet.

  Jedes Feld läuft durch `sanitize_note` und wird auf die Fensterbreite
  geklemmt: Was der Agent schickt, ist Text und keine Steuerfolge, und keine
  Adresse schiebt die Tastenzeile vom Schirm.

  Für **Vollbild-TUI-Agenten wie OpenCode** bleibt `CLI_002` die Antwort
  (`backlog/CONVENTIONS.md` 4.10): Ein TUI zeichnet den ganzen Schirm neu, und
  der Kasten wäre nach dem ersten Bild weg. Entschieden wird am wirksamen
  Kommando — `humanitl run --ask terminal -- bash` startet kein TUI und
  bekommt den Prompt.

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
  Problem, für ein Vollbild-TUI schon. Wer tippen will, hängt sich mit
  `humanitl sandbox attach` an dieselbe Sitzung; dort geht jede Taste an den
  Agenten.
- Einen **Raw-Modus** gibt es nur mit `--ask terminal`, und nur für die Tasten
  des Kastens: Der Prompt braucht einzelne Tasten ohne Zeilenende. Das
  Terminal wird auf jedem Ausgang zurückgegeben — beim gewöhnlichen Ende, auf
  jedem Fehlerpfad, bei einer Panik und bei `SIGTERM`, `SIGHUP` und `SIGINT`
  (dann mit Exit `128 + n`). Ohne `--ask terminal` wird es gar nicht erst
  angefasst.
- **`Ctrl+C`** beendet die Sitzung (`Sandbox(Stop)`), es geht nicht als Byte an
  den Agenten. Steht der Kasten, schließt das erste `Ctrl+C` ihn, und die
  Anfrage bleibt gehalten. **Zweimal gedrückt endet der Befehl selbst** mit
  `130`: Das erste bittet die Sitzung zu enden, und wenn der Agent darauf nicht
  hört, soll das zweite nicht ins Leere gehen. Dasselbe gilt für `SIGTERM` und
  `SIGHUP` von außen — sie beenden die Sitzung und den Befehl mit `128 + n`,
  und das Terminal ist danach wieder im Normalmodus.
- Ein **`Ctrl+]`-Menü** gibt es nicht. Der Weg zu einer gehaltenen Anfrage ist
  der Kasten von `--ask terminal`, die Anwendung oder `humanitl flows`.

### Exit-Codes

| Code | Bedeutung |
|---|---|
| Der des Agenten | Er hat sich beendet; seine Zahl wird weitergegeben. Ein Signal wird zu `128 + n`. |
| `1` | Ein Fehler des Aufrufers: ein Profil, das es nicht gibt, ein Pfad, der keiner ist, `--ask terminal` mit einem Vollbild-TUI-Agenten. |
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
läuft, bekommt `CLI_005` mit der Kennung der laufenden Sitzung. Wer sie sehen
will, hängt sich an: `humanitl sandbox attach` verbindet dieses Terminal mit
der laufenden Sitzung, `--read-only` sieht nur zu. Genau ein Client darf
schreiben; ein zweiter bekommt `TERM_001` und den Hinweis auf `--read-only`.
`Ctrl+C` erreicht den Agenten dabei als Byte `0x03` und nicht als Signal, denn
die Sandbox hat kein steuerndes Terminal. Wer sich abhängt, beendet nur den
eigenen Strom; die Sitzung läuft weiter, und ein späteres `attach` zeigt den
Rückstand.

## `humanitl doctor`

```
humanitl doctor [--json] [--probe-llm]
```

Prüft die Maschine und gibt eine Zeile je Vorbedingung aus, mit `ok`, `warn`
oder `fail`. Das ist der Befehl für die Frage „warum läuft das hier nicht?".

### Die elf Zeilen

| Kennung | Was sie prüft | `fail`, wenn |
|---|---|---|
| `bwrap` | `bwrap` liegt im `PATH` und ist mindestens 0.8 | es fehlt oder ist älter |
| `userns` | `bwrap --unshare-user` macht einen Namensraum auf | es geht nicht oder hängt länger als zwei Sekunden |
| `seccomp` | `/proc/self/status` führt ein Feld `Seccomp`, der Kernel ist mindestens 5.4 | das Feld fehlt (Kernel ohne `CONFIG_SECCOMP`) |
| `runtime_dir` | `$XDG_RUNTIME_DIR` ist gesetzt, ist ein Verzeichnis, gehört uns und hat genau 0700 | eines davon nicht — auch ein zu enger Modus wie 0500 oder 0600, in dem der Daemon weder hineinwechseln noch den Socket anlegen kann |
| `systemd_user` | `systemctl --user is-system-running` endet mit 0 und sagt `running` | nie; ohne systemd startet man `humanitld` von Hand |
| `daemon` | der Daemon antwortet und spricht dieselbe Major-Version des Vertrags | die Major-Version abweicht |
| `agent` | das Kommando des Agenten liegt im `PATH` und nennt seine Fassung | nie; der Pfad in der Sandbox kann ein anderer sein |
| `llm` | der Endpunkt aus `llm.endpoint` antwortet | nie |
| `tray` | `libayatana-appindicator3` oder `libappindicator3` liegt in einem Verzeichnis des Laders; unter GNOME zusätzlich die AppIndicator-Erweiterung | nie |
| `renderer` | Impeller verträgt sich mit dem Treiber: ein geladenes NVIDIA-Modul unter Wayland ist der bekannte schwarze Bildschirm | nie |
| `disk_space` | im Datenverzeichnis ist mindestens 1 GiB frei | nie |

Die vier ersten sind die Vorbedingungen der Sandbox; ohne sie startet nichts.
`fail` wird außerdem die Zeile `daemon`, wenn der Daemon eine andere
Major-Version des Vertrags spricht — dann versteht keine Seite die Nachrichten
der anderen. Alles Übrige ist eine Warnung: Es läuft, nur nicht so bequem.

### Was eine Zeile bedeutet

Jede Zeile, die nicht `ok` ist, trägt einen `Diagnostic` mit Code, Grund und
Vorschlag; die Blöcke stehen unter der Tabelle, einer je Zeile und nicht nur
für die erste. Die Codes sind `DOCTOR_001` bis `DOCTOR_013`
(`docs/DIAGNOSTICS.md`).

**`ok` heißt nachgesehen und in Ordnung, nie „ich konnte nicht nachsehen".**
Eine Prüfung, die nicht durchgeführt werden konnte — eine Datei, die es auf
diesem Kernel nicht gibt, ein Programm, das nicht antwortet —, ist `warn` mit
`DOCTOR_012`; ihr Beleg beginnt mit `not measured:`, und ihr Vorschlag ist der
Befehl, den der Doctor versucht hat, damit ein Mensch selbst nachsehen kann.

### Der Endpunkt des Sprachmodells wird nur auf Verlangen angesprochen

`humanitl doctor` baut von sich aus keine einzige Verbindung auf. Die Zeile
`llm` trägt deshalb ohne weitere Angabe `DOCTOR_013` — „nicht angesprochen" —
und als Vorschlag den Befehl, der es täte.

Mit `--probe-llm` wird gemessen. Vorher steht auf `stderr` eine Zeile, die
sagt, wohin es geht und was geschickt wird, **bevor** die Verbindung aufgebaut
wird; sie lässt sich weder mit `--json` noch mit `-q` abstellen, weil sie zur
Handlung gehört und nicht zur Ausgabe. Gemessen wird über die RPC `ProbeLlm`
im Daemon (zwei `GET` auf
`/api/tags` und `/v1/models`, keine Zugangsdaten, keine Weiterleitungen); ohne
laufenden Daemon gibt es die Probe nicht, und die Zeile bleibt ehrlich
ungemessen. Der Schalter heißt nicht `--llm`: Das ist der Zweitname von
`--llm-endpoint` und **setzt** die Adresse, statt sie zu prüfen.

### Woher der Bericht kommt

Der Doctor ist eine Fähigkeit des Daemons (RPC `Doctor`, ADR-018); der
Setup-Bildschirm zeigt dieselben Zeilen über denselben Aufruf. Läuft kein
Daemon, führt die Kommandozeile dieselben Prüfungen aus derselben Crate im
eigenen Prozess aus — ein Doctor, der einen laufenden Daemon bräuchte, wäre in
genau dem Fall nutzlos, für den es ihn gibt. Welche der beiden Quellen es war,
steht in der Ausgabe (`source`), denn `PATH` und `$XDG_RUNTIME_DIR` des Daemons
sind die seiner Unit und nicht die des Terminals.

Zwei Zeilen kommen immer vom Client: `daemon`, weil nur ein Client weiß, ob er
den Daemon erreicht, und `llm`, weil dahinter eine Verbindung stünde.

### Exit-Codes

| Code | Bedeutung |
|---|---|
| `0` | jede Zeile ist `ok` oder `warn` |
| `3` | mindestens eine Zeile ist `fail`; die Sandbox startet so nicht |
| `1` | die Konfiguration ließ sich nicht lesen |

Ein fehlender Daemon ist **kein** Exit 2: Er ist eine Zeile des Berichts.

### `--json`

Ein Wert auf `stdout`, die Schlüssel alphabetisch (so schreibt `serde_json`
eine Map), die Zeilen in der Reihenfolge der Anzeige:

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

`source` ist `daemon` oder `local`, `status` der schlimmste Zustand im Bericht,
`checks` die elf Zeilen in der Reihenfolge der Anzeige. `diagnostic` steht
genau dann, wenn die Zeile nicht `ok` ist. Ein Beleg, der mit `not measured:`
beginnt, ist eine Zeile ohne Messung.

## `humanitl daemon install`

Schreibt die systemd-Nutzer-Unit, die den Daemon bei jeder Anmeldung startet.
Es ist der eingriffsreichste Befehl dieses Produkts außerhalb der Sandbox, und
deshalb steht hier vollständig, was er tut.

```
humanitl daemon install [--print] [--no-start] [--bin-dir DIR]
humanitl daemon install --refresh
```

**Höchstens eine Datei, an einem genannten Ort.**
`$XDG_CONFIG_HOME/systemd/user/humanitld.service`, sonst
`~/.config/systemd/user/humanitld.service`, mit den Rechten `0644`. Keine
System-Unit, kein `sudo`, keine zweite Datei, keine `humanitld.socket`. Ihr
`ExecStart` nennt das `humanitld` **neben der laufenden Kommandozeile** — nie
eines aus `PATH` und nie eines aus einem Konfigurationswert, damit beim
Anmelden dieselbe Fassung startet wie die, die die Unit geschrieben hat.
`--bin-dir DIR` nennt statt dessen ein anderes Verzeichnis, etwa das Bundle
eines Pakets, das die Kommandozeile über einen Verweis erreicht hat.

**Mit dem Paket wird nichts geschrieben.** Liegt
`/usr/lib/systemd/user/humanitld.service` da, hat das Paket die Units
abgelegt, und der Befehl schreibt keine Datei (HUM-053). Er ruft nur, was das
Paket als root nicht kann: `systemctl --user daemon-reload` und
`systemctl --user enable --now humanitld.socket humanitld.service`. Beide
Units, weil ein Client das Token liest, bevor er den Socket öffnet, und das
Token erst der laufende Daemon schreibt. Die Ausgabe nennt als `unit` die Unit
des Pakets und als `action` das Wort `packaged`; unter `--json` stehen die
Namen in `units`. Nach der Aktivierung stehen in `unit` und `exec_start`, was
systemd wirklich geladen hat (`systemctl --user show -p FragmentPath,ExecStart`),
nicht, was in der Datei des Pakets steht; `unit_text` ist dann der Text dieser
geladenen Datei und `null`, wenn sie sich nicht lesen lässt. Liegt unter
`~/.config/systemd/user/`
noch die eigene Unit einer früheren Installation (erste Zeile ist die Marke),
verdeckte sie die des Pakets: Der Befehl kündigt es an, legt sie samt Verweis
der Aktivierung als `humanitld.service.bak` beiseite (oder `.bak.N`, wenn der
Name vergeben ist; `set_aside` in der Ausgabe), startet den Dienst neu und
legt beides zurück, wenn das scheitert (HUM-211). Beiseitegelegt wird nur,
wenn auch aktiviert wird: Unter `--print`, `--no-start` oder ohne `systemctl`
bleibt sie liegen und steht als `shadowed_by` in der Ausgabe. Eine Datei dort
ohne die Marke bleibt liegen, und der Befehl bricht mit `DAEMON_005` ab, auch
unter `--print`.
Aus einem AppImage und mit `--bin-dir` gilt dieser Weg nicht:
Beide nennen ausdrücklich einen anderen Daemon als den des Pakets. Ohne Paket
schreibt der Befehl nie eine Socket-Unit; der Daemon bindet seinen Socket dann
selbst.

**Aus einem AppImage wird kopiert.** Läuft die Kommandozeile aus einem
AppImage (`$APPIMAGE` ist gesetzt), liegen `humanitld` und `humanitl-shim`
unter `/tmp/.mount_*`, und dieser Pfad verschwindet mit dem Prozess. Ein
`ExecStart` darauf zeigte beim nächsten Anmelden ins Leere. Beide Binaries
werden deshalb nach `~/.local/lib/humanitl/<version>.<stempel>/` kopiert, der
Verweis `~/.local/lib/humanitl/current` zeigt auf diese Kopie, und `ExecStart`
nennt den Verweis: Ein Update legt eine neue Kopie daneben und hängt den
Verweis um, ohne die Unit anzufassen. Fehlt eines der beiden Binaries, endet
der Befehl mit `DAEMON_007`. Kopiert wird erst nach der Ankündigung und nach
jeder Prüfung, die den Lauf ablehnen kann; eine fremde Unit (`DAEMON_005`) oder
eine fehlende Nutzersitzung (`DAEMON_010`) lassen `~/.local/lib/humanitl` also
unberührt.

Jede Kopie bekommt ein neues Verzeichnis (`<stempel>` aus Zeit und
Prozessnummer), auch wenn dieselbe Fassung ein zweites Mal installiert wird:
Das Verzeichnis, auf das `current` gerade zeigt, wird nie angefasst, bevor
`current` auf eine vollständige neue Kopie zeigt. Ein Abbruch mittendrin lässt
also nie einen Verweis ins Leere. `current` wird über einen zweiten Verweis und
`rename` umgehängt, nie gelöscht und neu angelegt. Zeigte `current` vorher auf
eine andere Kopie, läuft der Dienst womöglich noch aus ihr; der Befehl startet
ihn deshalb nach `enable --now` mit `systemctl --user restart
humanitld.service` neu, und erst danach gehen die vorige Kopie und jede ältere,
die ein früherer Lauf liegen lassen musste (HUM-077). Ohne Neustart
(`--no-start`, kein `systemctl`) bleibt die vorige Kopie liegen. Scheitert der
Neustart, zeigt `current` wieder auf die vorige Kopie, die Unit bekommt ihren
alten Text, der alte Daemon wird noch einmal gestartet, und der Befund ist
`DAEMON_008`. Geht `current` oder die Unit nicht nachweislich zurück, startet
der Befehl nichts mehr, denn er startete sonst die eben gescheiterte Kopie;
der Befund nennt dann, was nicht zurückging. Dasselbe gilt für eine ersetzte Unit auf dem Weg ohne AppImage:
Wer `daemon install` aus einem neuen Archiv ruft, bekommt den Daemon des neuen
Archivs auch sofort und nicht erst bei der nächsten Anmeldung. Ist
`~/.local/lib/humanitl` ein Verweis oder gehört
es einem anderen Konto als das Heimatverzeichnis, wird nichts kopiert
(`DAEMON_011`). Scheitert nach der Kopie noch etwas — die Unit, `systemctl` —,
zeigt `current` wieder dorthin, wohin es vorher zeigte, und die neue Kopie
geht wieder. `--print` nennt denselben Pfad `…/current/humanitld`, den die
Unit bekäme, und kopiert nichts.

**`--refresh`: ein neueres AppImage erneuert den Dienst.** `AppRun` ruft bei
jedem Start der Anwendung `humanitl -q daemon install --refresh` (HUM-077).
Der Schalter tut nur dann etwas, wenn der Lauf aus einem AppImage kommt, die
Unit unter `~/.config/systemd/user/` die Marke trägt und
`ExecStart=~/.local/lib/humanitl/current/humanitld` nennt, und `current` auf
die Kopie einer anderen Fassung zeigt; dann läuft alles wie oben beschrieben,
samt Neustart. Sonst endet er sofort mit 0 und schreibt, warum nicht:
`not_appimage`, `not_installed` oder `up_to_date` (unter `--json` als `action`,
dazu `installed` und `version`). Die erste Einrichtung macht er nie: Eine Unit,
die bei jeder Anmeldung einen Dienst startet, legt ein Programmstart nicht
ungefragt an; das bleibt der Knopf in der Einrichtung oder
`--cli daemon install`. Und wer den Dienst mit `daemon uninstall` entfernt hat,
bekommt ihn vom nächsten Start des AppImage nicht still zurück, weil die Unit
fehlt. `--refresh` verträgt sich nicht mit `--print`, `--no-start` und
`--bin-dir`.

**Ohne Nutzersitzung wird nichts geschrieben.** Fehlt `XDG_RUNTIME_DIR` und
soll die Unit gestartet werden (kein `--print`, kein `--no-start`), endet der
Befehl vor dem ersten Schreibzugriff mit `DAEMON_010` und dem Vorschlag
`loginctl enable-linger $USER`. Dasselbe gilt, wenn `systemctl --user` den Bus
der Sitzung nicht findet („Failed to connect to bus"); dann ist die Unit schon
zurückgenommen, wenn der Befund erscheint.

**Am Ende wird gewartet.** Hat systemd die Unit genommen, fragt der Befehl bis
zu fünf Sekunden lang `GetInfo`, bis der Daemon antwortet, und schreibt die
Fassung, die er nennt, als letzte Zeile. Antwortet er nicht, steht dort
`no answer within 5000 ms`; das ist eine Beobachtung und kein Grund, die
Installation zurückzunehmen — die Unit liegt, und `humanitl daemon logs` sagt,
woran es hängt.

**Sichtbar, bevor es geschieht.** Der ganze Text der Unit und beide
`systemctl`-Aufrufe gehen vor dem ersten Schreibzugriff auf `stderr`, an der
Ausgabesteuerung vorbei: `-q` schaltet das nicht ab. Unter `--json` liest ein
Programm, und für das gilt der Vertrag dieser Kommandozeile — ein Objekt auf
`stdout`, `stderr` leer —; Text und Befehle stehen dann im Objekt als
`unit_text` und `commands`. `--print` zeigt dieselbe Datei und schreibt
nichts.

**Wiederholbar.** Ein zweiter Aufruf mit demselben Ergebnis schreibt nicht
noch einmal; die Ausgabe sagt dann `unchanged` statt `created`.

**Nie über fremdes Eigentum.** Die erste Zeile der Unit ist die Marke
`# humanitl daemon install: written by Humanitl`. Fehlt sie in einer
vorhandenen Datei, gehört die Datei jemand anderem, und der Befehl weigert
sich mit `DAEMON_005`, statt sie zu überschreiben. Es gibt dafür mit Absicht
kein `--force`: Wer eine eigene Unit führt, legt sie beiseite.

**Danach `systemctl --user`, nie `sudo`.** Ohne `--no-start` laufen
`systemctl --user daemon-reload` und `enable --now`, und zwar je nach Weg:

- *Archiv und AppImage:* `systemctl --user enable --now humanitld.service`,
  auf die eben geschriebene Unit. Der Daemon bindet seinen Socket selbst.
- *Units des Pakets:* `systemctl --user enable --now humanitld.socket
  humanitld.service`, auf die Units unter `/usr/lib/systemd/user/`. Geschrieben
  wird dabei nichts. Beide Units, weil ein Client das Token liest, bevor er
  den Socket öffnet, und erst der laufende Daemon es schreibt.

Gibt es kein `systemctl` in `PATH`, startet nichts, und die Ausgabe nennt die
beiden Befehle.

**Ein Fehlschlag lässt nichts liegen.** Nimmt systemd die Unit nicht an, wird
der Zustand von vorher wiederhergestellt — eine angelegte Datei verschwindet,
eine ersetzte bekommt ihren alten Inhalt zurück, die Units des Pakets bleiben,
wie sie sind — und `daemon-reload` läuft noch einmal, damit auch systemds Bild
davon stimmt. Scheitert `enable --now`, werden vorher alle genannten Units
angehalten (`stop`, `reset-failed`). Der Befund ist `DAEMON_008`.

Zurückgenommen wird dabei auch die Aktivierung. `systemctl --user enable --now`
ist ein Aufruf mit zwei Schritten: Er legt die Verweise an und startet dann.
Die Verweise liegen immer im Unit-Verzeichnis des Nutzers, auch für die Units
des Pakets: `default.target.wants/humanitld.service` und beim Paket zusätzlich
`sockets.target.wants/humanitld.socket`. Misslingt der Start, stünden sie ohne
diese Rücknahme weiter da, und der Dienst startete beim nächsten Anmelden,
obwohl der Befehl mit `DAEMON_008` abgebrochen ist. Entfernt werden nur die
Verweise, die dieser Aufruf angelegt hat, für beide Units: Wer den Dienst schon
vorher aktiviert hatte, behält die Aktivierung.

Die Härtung der Unit ist gemessen und nicht behauptet (HUM-044, gehärtet in
HUM-053). Die Escape-Tests laufen unter genau ihren `[Service]`-Zeilen mit 124
von 124 Fällen grün. Die Zeilen, die dafür stehen müssen:
`SystemCallFilter=@system-service @mount @sandbox sethostname` —
`@mount`, weil `bubblewrap` mountet und `pivot_root`t; `@sandbox`, weil der
Filter einer Unit an jedes Kind vererbt wird und `humanitl-shim` seinen eigenen
seccomp-Filter mit `seccomp(2)` installiert (ohne den Aufruf stirbt der Shim an
`SIGSYS`, bevor die dritte Sandbox-Garantie steht; auf systemd 262 enthielte
`@system-service` ihn über `@default` auch so, aber das steht in keiner Zeile
von `systemd.exec(5)`); `sethostname`, weil `bubblewrap` den UTS-Namensraum
der Sandbox `sandbox` nennt und sonst mit „Can't set hostname to sandbox"
abbricht. `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK` —
ohne `AF_NETLINK` bringt `bubblewrap` `lo` im Namensraum nicht hoch
(„loopback: Failed to create NETLINK_ROUTE socket"), und ohne `lo` gibt es
keine Brücke vom Shim zum Proxy. Dazu `NoNewPrivileges`, `ProtectSystem=strict`,
`PrivateUsers`, `ProtectProc=invisible`, `CapabilityBoundingSet=CAP_SYS_ADMIN`
und weitere; jede Zeile, auch jede mit Absicht fehlende, steht mit ihrem
Grund in `docs/INSTALL.md#hardening`, und `systemd-analyze security` bewertet
die Unit mit 3,7. Jeder Pfad in `ReadWritePaths` trägt ein `-`, weil systemd
eine Unit mit einem nicht vorhandenen Pfad darin gar nicht erst startet und auf
einer frischen Installation keiner der drei existiert. Gemessen am 2026-09-18
auf Debian 14 mit bubblewrap 0.12.0 und systemd 262.

**`PrivateTmp=yes` hat eine Folge, die man kennen muss: Ein Projektordner
unter `/tmp` funktioniert nicht.** Die Unit gibt dem Daemon ein eigenes,
privates `/tmp`; das `/tmp` der Anmeldesitzung sieht er nicht. Ein Ordner wie
`/tmp/repo` ist für ihn deshalb nicht vorhanden, `bwrap` kann ihn nicht
einhängen, und der Start endet mit `SANDBOX_005` und dem Satz
„sandbox.work_dir /tmp/repo does not exist", obwohl der Ordner im Terminal
danebensteht. Dasselbe gilt für alles unter `/var/tmp`. Wer ein Projekt aus
`/tmp` heraus moderieren will, legt es an eine bleibende Stelle — unter `$HOME`
oder irgendwo sonst außerhalb von `/tmp` — oder startet den Daemon von Hand
(`humanitld`) statt über die Unit. Die Zeile bleibt trotzdem in der Unit: Ein
gemeinsames `/tmp` ist der Weg, auf dem ein anderer Prozess des Nutzers dem
Daemon eine Datei unterschiebt, und ein Projektordner in einem Verzeichnis, das
beim nächsten Start verschwindet, ist ohnehin kein Ort für Arbeit.

## `humanitl daemon uninstall`

Das Gegenstück zu `daemon install` (HUM-077): meldet den Dienst ab und nimmt
weg, was `daemon install` angelegt hat.

```
humanitl daemon uninstall [--purge-binaries]
```

**Erst abmelden, dann entfernen.** Zuerst läuft `systemctl --user disable
--now` für die Units, die es gibt: `humanitld.service`, wenn die Unit mit der
Marke unter `~/.config/systemd/user/` liegt, beim Paket
`humanitld.socket humanitld.service`. Scheitert der Aufruf, ist noch nichts
entfernt, und der Befund `DAEMON_014` nennt genau diesen Aufruf zum Kopieren;
findet `systemctl` den Bus der Sitzung nicht, ist es `DAEMON_010`. Danach
gehen:

- jeder Verweis der Aktivierung unter `~/.config/systemd/user/*.wants/` und
  `*.requires/` für diese Units (ohne `systemctl` entfernt der Befehl sie
  selbst, sonst die Reste, die `disable` übrig ließ);
- die Unit `~/.config/systemd/user/humanitld.service`, nur wenn sie die Marke
  trägt;
- der Socket `$XDG_RUNTIME_DIR/humanitl/daemon.sock`, nur wenn es ein Socket
  ist und niemand mehr an ihm lauscht. Antwortet dort noch ein Daemon, den
  jemand von Hand gestartet hat, bleibt er unberührt;
- mit `--purge-binaries` die Kopien aus einem AppImage: der Verweis
  `~/.local/lib/humanitl/current`, liegengebliebene Zwischenverweise
  `current.tmp-*` und jedes eigene Verzeichnis der Form
  `<version>.<stempel>`, danach `~/.local/lib/humanitl` selbst, wenn es leer
  ist. Was anders heißt, ein Verweis ist oder einem anderen Konto gehört,
  bleibt liegen. Die Kopien gehen erst hier, weil erst jetzt kein Dienst mehr
  aus ihnen läuft.

Antwortet nach dem Abmelden noch ein Daemon am Socket, etwa einer, den
jemand von Hand gestartet hat, entfernt `--purge-binaries` nichts: Er läuft
womöglich aus genau diesen Kopien. Der Befund ist `DAEMON_014` mit dem
Vorschlag `pkill -x humanitld`.

Danach `daemon-reload` und `reset-failed`, deren Ergebnis nicht zählt.
Bleibt etwas stehen, endet der Befehl mit `DAEMON_014` und nennt jeden Pfad,
der blieb.

**Nie über fremdes Eigentum.** Eine Unit ohne die Marke hat Humanitl nicht
geschrieben; der Befehl meldet nichts ab, entfernt nichts und endet mit
`DAEMON_005`. Die Units des Pakets unter `/usr/lib/systemd/user/` werden
abgemeldet, bleiben aber liegen, bis das Paket geht
(`sudo apt remove humanitl`); eine Notiz sagt das. Konfiguration,
Aufzeichnung und Audit-Log bleiben ebenfalls liegen, sie gehören dem Menschen.

Ohne Unit und ohne Kopie endet der Befehl mit 0 und `activation: nothing`.
Die Ausgabe nennt `unit`, `units`, `activation` (`disabled`, `no systemctl`,
`nothing`), `removed`, `binaries` und unter `--json` `package`, den Pfad der
Dienst-Unit des Pakets oder `null`.

Danach zeigt `humanitl doctor` in der Zeile `daemon` den Befund `DOCTOR_006`
mit dem Vorschlag `InstallService`, also `humanitl daemon install`: Ohne
erreichbaren Daemon schlägt der Doctor den Weg vor, der den Dienst auf Dauer
einrichtet, und nicht `humanitld` im Vordergrund eines Terminals.

## `humanitl daemon status`

Fragt den Daemon, wer er ist.

```
humanitl daemon status [--json]
```

```
FIELD         VALUE
socket        /run/user/1000/humanitl/daemon.sock
unit          active
daemon        0.0.0
proto         1.0
session       7b1c…
capabilities  hold, rules, sandbox
```

`socket` ist der Pfad, an dem der Client sucht; `unit` ist die Antwort von
`systemctl --user is-active humanitld.service` und steht als `-`, wenn es kein
`systemctl` gibt — ein Daemon, den jemand von Hand gestartet hat, ist kein
Fehler, und ein erfundenes `inactive` wäre eine falsche Auskunft über eine
Unit, die niemand installiert hat. Alles Übrige kommt aus `GetInfo` und wird
nicht ergänzt.

Antwortet niemand, endet der Befehl mit **2** und `DAEMON_001`; spricht der
Daemon eine andere Hauptversion des Vertrags, ebenfalls mit 2 und
`DAEMON_002`. Ein Skript, das auf den Dienst wartet, fragt also nur diese eine
Zahl ab.

## `humanitl daemon logs`

Reicht das Journal des Dienstes durch.

```
humanitl daemon logs [-f] [-n N]
```

Der Befehl startet `journalctl --user -u humanitld.service` als Kind und gibt
ihm das eigene Terminal. `-n N` und `-f` gehen unverändert weiter. Endet
`journalctl` mit 0, endet der Befehl mit 0, sonst mit 1: Eine 2 oder 4 von
`journalctl` hieße hier „Daemon nicht erreichbar" oder
„Sicherheitsverletzung", und beides wäre falsch. Einen eigenen Leser für Journal-Einträge gibt es nicht: Er
wäre eine zweite Quelle für dieselbe Wahrheit, mit eigenen Formaten und eigenen
Fehlern.

Weil das Kind schreibt, hat `--json` hier keine Wirkung; wer JSON-Zeilen will,
bekommt sie von `journalctl` selbst (`humanitl daemon logs -- -o json` gibt es
nicht, dafür ruft man `journalctl --user -u humanitld.service -o json` direkt).

Fehlt `XDG_RUNTIME_DIR`, endet der Befehl mit `DAEMON_010` und dem einzig
sinnvollen Vorschlag: `loginctl enable-linger $USER`. Über SSH ohne Linger gibt
es keine Nutzersitzung, in der ein Nutzerdienst leben könnte, und damit auch
kein Journal. Liegt kein `journalctl` im `PATH`, fehlt dagegen ein Programm und
keine Sitzung: `DAEMON_012` mit `sudo apt-get install systemd`.

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

Ohne `KEY` steht jedes Blattfeld als Zeile da:

```
$ humanitl config get --origin
KEY                    VALUE    ORIGIN
hold.ask_mode          ui       default
hold.timeout_secs      300      default
llm.endpoint           -        default
sandbox.work_mode      rw       config.toml
```

Ohne `--origin` bleibt die dritte Spalte weg, damit die Tabelle in ein Terminal
passt; mit `--json` steht sie immer, dort kostet eine Spalte keine Breite.

Mit `KEY` steht auf `stdout` nur der Wert — `$(humanitl config get
hold.timeout_secs)` ist damit die Zahl und nicht ein Satz darüber. Welche Ebene
ihn gesetzt hat, geht auf `stderr`; in eine Pipe gerät sie nicht.

Aufgelöst wird lokal, über dieselben sieben Ebenen, die der Daemon beim Start
fährt (ADR-011). Ein laufender Daemon kann Werte tragen, die nur er kennt —
`hold.timeout_secs` aus einer Sitzung etwa —; was hier steht, ist die
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
| `llm.endpoint` | `null` oder `-` | nichts: der Schlüssel geht, der Vorgabewert gilt |

Ein Wert, der mit `-` beginnt, ist ein Wert und kein Flag
(`humanitl config set hold.timeout_secs -5m` wird gelesen und abgelehnt, nicht
als unbekannte Option).

Eine Dauer nimmt `s`, `m`, `h` und `d`, eine Größe `KiB`, `MiB`, `GiB` und die
Formen ohne `i`. Beides gilt nur für Felder, deren Name darauf endet (`_secs`,
`_ms`, `_bytes`); das Schema selbst kennt keine Einheiten.

```
$ humanitl config set hold.timeout_secs 5m
hold.timeout_secs = 300 (global)
```

Geschrieben wird `$XDG_CONFIG_HOME/humanitl/config.toml` (mit `--config` die
dort genannte Datei), und zwar über denselben Schreiber, den der Daemon für
`SetConfig` benutzt (`humanitl_config::edit::set_value`). Dabei gelten vier
Zusagen:

- **Nur der eine Wert ändert sich.** Das Dokument wird mit `toml_edit`
  geändert und nicht neu serialisiert; Kommentare, Reihenfolge und
  Schreibweise bleiben, auch ein Kommentar hinter dem Wert, eine Markierung
  der Byte-Reihenfolge und Zeilenenden mit CRLF. Eine verlinkte Datei bleibt
  verlinkt; geändert wird ihr Ziel. Danach wird das Ergebnis noch einmal
  gelesen und mit der alten Datei samt dem neuen Wert verglichen; weicht es ab,
  wird nichts geschrieben (`CONFIG_015`).
- **Nie eine halbe Datei.** Geschrieben wird in eine Nebendatei im selben
  Verzeichnis, dann `rename`.
- **Kein Schreiber verliert einen Wert.** Während des Schreibens ist das
  Verzeichnis der Datei mit `flock` gesperrt; ein zweites `config set` oder
  ein `SetConfig` des Daemons wartet, statt einen Wert zu überschreiben, den es
  nie gelesen hat.
- **Geprüft, bevor es in der Datei steht.** Erst gegen das Schema — Typ,
  Aufzählung, Grenzen —, dann wird die fertige Nebendatei mit allen übrigen
  Ebenen geladen, wie der nächste Start sie lädt: Profile, Umgebung und die
  anderen Werte der Datei. Ein Wert, der für sich richtig ist, aber mit einem
  anderen nicht zusammenpasst (`limits.hold_body_cap_bytes` über
  `limits.hold_max_bytes`), landet so nie in der Datei. Die Quellen werden
  dafür mit der neuen Datei neu bestimmt: `config set sandbox.work_dir` wird
  mit dem Profil des Projekts geprüft, das es nennt, nicht mit dem des alten.
  Lädt die Konfiguration schon ohne die Änderung nicht — ein anderer Wert der
  Datei, eine Umgebungsvariable wie `HUMANITL_HOLD__TIMEOUT_SECS=0` —, zählt
  `config set` alle Befunde vorher und nachher: Die Ladung meldet nur ihren
  ersten, also nimmt die Prüfung den Schlüssel jedes Befunds aus Datei,
  Projekt-Profil und Umgebung und lädt neu. Geschrieben wird, wenn der neue
  Wert keinen Befund mitbringt, den es vorher nicht gab, und keiner den
  gesetzten Schlüssel nennt; auf `stderr` steht dann, dass die Konfiguration
  weiter nicht lädt. Derselbe Befund heißt gleicher Code, gleicher Schlüssel
  und gleiche Begründung: Verschiebt der neue Wert die Schranke eines alten
  Befunds (`limits.hold_max_bytes` neu gesetzt, während
  `limits.hold_body_cap_bytes` schon darüber lag), ist das ein neuer Befund. So lässt sich eine Datei mit zwei falschen Werten
  Schlüssel für Schlüssel reparieren, und ein älterer Fehler verdeckt keinen
  neuen. Ein Befund, der keinen Schlüssel nennt (ein unbekannter Schlüssel in
  der Datei, ein Profil, das nicht lädt), lässt sich so nicht zählen; dann
  wird nicht geschrieben, und der Vorschlag ist `humanitl config edit`, nie ein
  Wert aus einer anderen Ebene. Ein Wert, der schon genau so dasteht, wird
  ebenso geprüft; stand er falsch da, endet `config set` mit dem Befund. Ein Wert, der nicht passt, ist `CONFIG_003` und Exit 1, und die Datei
  bleibt unberührt:

```
$ humanitl config set hold.ask_mode banana
error[CONFIG_003]: Wert außerhalb des Bereichs
  why: banana is not a value of hold.ask_mode; it takes one of ui, terminal, none
  fix: humanitl config set hold.ask_mode ui
  docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#config_003
```

Jeder Vorschlag ist ein Befehl, der gelingt: bei einer Aufzählung ihr erster
Wert, sonst der Vorgabewert des Feldes (bei `-5m` für `hold.timeout_secs` also
`300`, nicht die Grenze `0`, die die Prüfung ebenso ablehnte), und Listen oder
Tabellen darin stehen in einfachen Anführungszeichen.

Steht der Wert schon so da, wird nicht geschrieben; `--json` sagt dann
`"written": "unchanged"`.

Auf `stderr` steht danach, wann der neue Wert wirkt: Läuft ein Daemon,
übernimmt er ihn beim nächsten Start; läuft keiner, liest er ihn, sobald er
startet. Beides ist richtig, und deshalb sagt die Ausgabe, welcher Fall
vorliegt.

`--project` schreibt statt dessen in `<projekt>/.humanitl/profile.toml` unter
`[config]`. Das geht nur für Schlüssel, die das Projekt-Profil setzen darf:
Diese Datei liegt im geklonten Repository und ist fremder Text, und ein
gesperrter Schlüssel ist dort `CONFIG_003` — dieselbe Grenze, an der auch der
Start eine solche Datei zurückweist (`backlog/CONVENTIONS.md` 4.11). Die
Grenze wird vor allem anderen geprüft: Wer einen gesperrten Schlüssel ins
Projekt schreiben will, erfährt die Grenze und nicht, dass sein Wert falsch
geschrieben ist.

### `config schema`

Gibt das JSON-Schema aus, immer als JSON; ohne `--json` eingerückt. Jedes
Blattfeld trägt `description`, `x-tier` (die Sichtbarkeitsstufe des
Einstellungs-Bildschirms) und `x-project-scope` (die Vertrauensgrenze oben).
`--profiles` gibt statt dessen die Profile aus, die `--profile` wählen kann.

### `config edit`

Öffnet `config.toml` in `$VISUAL`, sonst `$EDITOR`, sonst `nano`, sonst `vi`,
und prüft die Datei nach dem Schließen: erst, ob sie TOML ist, dann, ob sie
sich auflösen lässt. Geht das nicht, steht der Befund da, und an einem Terminal
folgt die einzige interaktive Frage dieser Kommandozeile — „open it again?
[y/N]". Ohne Terminal und unter `--json` gibt es keine Frage; dann endet der
Befehl mit dem Befund und Exit 1. Der Befund steht in jedem Fall genau einmal
da, unter `--json` als ein Objekt. Findet sich kein Editor, ist das
`CONFIG_017` mit dem Vorschlag `export EDITOR=nano`.

## `humanitl audit`

Die Hash-Kette prüfen und die Records exportieren.

```
humanitl audit verify [--file PATH] [--json]
humanitl audit export --format jsonl|csv --out FILE [--since TS] [--until TS] [--file PATH]
```

### Was `verify` beweist und was nicht

Die Kette zeigt eine Änderung, Löschung oder Umordnung vor dem letzten Anker,
solange der Angreifer den HMAC-Schlüssel nicht hat (`docs/SECURITY.md`, „Was
die Audit-Kette beweist"). Diese Aussage hängt an drei Dingen: der Kette
selbst, den MACs und den Ankern. Alle drei hat nur der Daemon — den Schlüssel
aus dem Schlüsselspeicher, die Anker aus der Tabelle `audit_anchors`.

Deshalb fragt der Befehl zuerst den Daemon (HUM-156). Der prüft jeden Record
mit seinem Schlüssel und gegen die Anker und sagt das:

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

Der Kopf ist derselbe Hash, den der Audit-Screen der Oberfläche zeigt: Beide
fragen denselben Daemon, und der bringt vor jeder Antwort alles auf die
Platte, was er bis dahin geschrieben hat.

Antwortet **kein** Daemon — keiner erreichbar, keiner, der das Token annimmt,
oder einer, der `Audit` noch nicht kennt —, prüft die Kommandozeile die Datei
selbst, und das ist die **schwächere** Prüfung: Kette und kanonische Form,
aber keine MACs und keine Anker. Die Ausgabe sagt das, in jeder Fassung:

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

Die erste Warnung sagt, warum die Datei geprüft wurde: „is not reachable",
wenn kein Daemon da ist, „does not answer Audit", wenn ein älterer Daemon den
Aufruf nicht kennt. Der Rest der Datei-Prüfung kennt keine Anker, also zählt
jeder Record als unverankert.

Ein Daemon, der antwortet und **ablehnt** — sein Log oder seine Anker sind
nicht lesbar, oder er läuft ohne Audit-Log (`IPC_006`) —, bekommt keine
Datei-Prüfung als Ersatz. Sein Befund steht auf `stderr`, und der Befehl
endet mit dessen Exit-Code: Eine schwächere Antwort, die eine Ablehnung
überdeckt, wäre die falsche Auskunft.

`--file PATH` prüft ausdrücklich diese Datei, ebenfalls ohne Schlüssel und
Anker — der Weg für ein Log, das von woanders kommt.

Bricht die Kette, endet der Befehl mit **4** (Sicherheitsverletzung nach
`backlog/CONVENTIONS.md` 3.8), nennt die Stelle und schreibt `AUDIT_001` auf
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

Die ersten sieben Zeilen stehen auf `stdout`, der Befund auf `stderr`.

Mit `--json` steht alles davon in einem Objekt auf `stdout`, der Befund als
Feld `diagnostic`; `stderr` bleibt leer. `mode` sagt, welche Prüfung lief:
`full` beim Daemon mit Schlüssel und Ankern, `file` ohne beides; `hmac` und
`anchors` sagen dasselbe je Teil (`checked` oder `not_checked`). Beim Daemon
kommen `anchor_count` und `last_anchor_at` dazu. `head.hash` ist der Hash, den
der Audit-Screen zeigt; `head.ts` ist der Zeitstempel der Zeile dieses
Records, Zeichen für Zeichen wie im Log (HUM-162). Nur ein Daemon, der
`AuditResponse.head_ts` noch nicht kennt, lässt ihn aus; dann steht dort
`null`.

### `audit export`

```
$ humanitl audit export --format csv --out audit.csv
exported 4213 records to audit.csv
```

Den Export schreibt der Daemon, wenn einer antwortet, sonst die
Kommandozeile selbst; beide nehmen denselben Code (`humanitl_audit::export`)
und schreiben dieselbe Datei. `jsonl` schreibt jede Zeile des Logs wörtlich —
der Export bleibt damit gegen die Datei nachrechenbar. `csv` schreibt eine
Kopfzeile und eine Zeile je Record, mit den zwölf Spalten aus HUM-050:

```
seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash
```

Die acht Spalten zwischen `kind` und `hash` kommen aus `data` und bleiben leer,
wo der Record das Feld nicht trägt; `flow.decided` etwa nennt weder Host noch
Methode, die stehen in `flow.received`. `data` als Ganzes steht nur im JSONL.
Bis HUM-156 schrieb die Kommandozeile acht Spalten (`seq,ts,session,kind,data,
prev,hash,mac`); die Übersicht der Oberfläche und die der Kommandozeile sind
seitdem dieselbe.

Felder mit Komma oder Anführungszeichen stehen nach RFC 4180 in
Anführungszeichen, doppelte Anführungszeichen verdoppelt, und jede Zeile endet
mit CRLF. JSON-Zeilen enden mit LF, wie im Log.

Eine Datei, die schon da ist, wird nie überschrieben (`AUDIT_008`): Ein Export
ist ein Beleg, und ein `--force` gibt es aus demselben Grund nicht wie bei
`daemon install`. Geschrieben wird in eine Nebendatei, und erst der fertige
Export bekommt seinen Namen; ein Log mit einer Zeile, die kein Record ist
(`AUDIT_001`), hinterlässt keine halbe Datei, die den nächsten Versuch
abwiese. Eine gebrochene Kette exportiert er unverändert, den Bruch meldet
`humanitl audit verify` (HUM-214). Die
Nebendatei eines abgebrochenen Exports räumt der nächste Export in dasselbe
Verzeichnis weg, aber nur, wenn das Verzeichnis dem eigenen Konto gehört und
weder Gruppe noch andere hineinschreiben dürfen; in einem geteilten
Verzeichnis bleibt sie liegen. Ein relativer
`--out` wird vor dem Aufruf beim Daemon gegen das eigene Verzeichnis
aufgelöst: Der Daemon läuft woanders. Läuft er unter systemd mit
`PrivateTmp=yes`, hat er ein eigenes `/tmp`; ein Export nach `/tmp` oder
`/var/tmp` wird dann mit `AUDIT_008` abgelehnt, statt in einem Verzeichnis zu
landen, das nur der Daemon sieht. Der Daemon legt kein Verzeichnis an und
schreibt nicht durch einen Verweis im Weg zum Ziel; das Verzeichnis legt die
Kommandozeile vorher selbst an.

`--since` und `--until` nehmen RFC-3339-Zeitpunkte und schneiden halboffen
(`since` gehört dazu, `until` nicht), damit ein Record nie in zwei
aufeinanderfolgenden Exporten steht. Sie gehen mit zum Daemon: Der Vertrag
(`AuditRequest.Export.from` und `.to`) schließt beide Grenzen ein, und weil
das Log Mikrosekunden schreibt, schickt die Kommandozeile als obere Grenze die
letzte ganze Mikrosekunde vor `--until`. Das Ergebnis ist dasselbe wie in der
Fassung über die Datei.

Was in keinem Record steht und damit auch in keinem Export: Bodies,
Klartext-Werte, die Originale von Pseudonymen — und die Notiz einer
Entscheidung. Sie steht in der Aufzeichnung (`flows.decision_note`,
`humanitl flows show`), nicht im Audit-Log (HUM-117).

## `humanitl llm discover`

Sucht im eigenen Netz nach Modellservern. Es ist nach `daemon install` der
zweite Befehl, der über den eigenen Rechner hinausgreift, und deshalb steht
hier vollständig, was er tut.

```
humanitl llm discover [--subnet CIDR] [--port PORT]... [--json]
```

**Nur auf Zuruf, und vorher angekündigt.** Ohne diesen Befehl entsteht keine
einzige Verbindung; die Oberfläche hat denselben Weg hinter einem Knopf mit
demselben Text. Vor dem ersten Paket geht auf `stderr` eine Zeile heraus, die
das Netz und die Ports nennt — an der Ausgabesteuerung vorbei, damit auch
`--json` sie nicht verschluckt.

**Im eigenen `/24` und nirgends sonst.** Ohne `--subnet` leitet der Daemon das
Netz aus der Vorgaberoute ab: Die Routing-Tabelle nennt Schnittstelle und
Gateway, ein verbundener UDP-Socket (der nichts schickt) nennt die eigene
Adresse, und daraus wird das `/24` um sie herum. Ein `--subnet`, das weiter ist
als ein `/24`, wird mit `LLM_008` abgelehnt, bevor irgendetwas verbindet: Ein
`/16` wären 65 534 Verbindungsversuche in ein Netz, das dem Aufrufer
vielleicht nicht gehört.

**Vier Ports, zwei Schritte.** Gefragt werden `11434` (Ollama), `1234`
(LM Studio), `8000` (vLLM und die meisten Python-Server) und `8080`
(llama.cpp); `--port` ersetzt diese Liste. Zuerst ein Verbindungsversuch je
Adresse und Port mit 200 ms Frist, 64 gleichzeitig; für jeden offenen Port
danach dieselbe Probe wie beim Testknopf: `GET /api/tags`, sonst `GET
/v1/models`, ohne Zugangsdaten und ohne Weiterleitung. Ein `/24` ohne einen
einzigen Treffer ist damit in unter vier Sekunden durch.

**Was in der Liste steht, hat der Server gesagt.** Produkt, Modelle und Latenz
kommen aus seiner Antwort; die Modellnamen sind gedeckelt und gesäubert wie
die des Testknopfs. Ein Server, der `401` oder `403` sagt, verschwindet nicht,
sondern trägt `(auth required)`. Ein Server, der auf einem der Ports horcht,
aber keine der beiden APIs beantwortet, steht als `unknown` da — er wird
genannt, nicht verschwiegen und nicht als Modellserver ausgegeben.

**Kein Fluss und keine Warteschlange.** Die Suche läuft im Daemon im Host-Netz,
nie in der Sandbox. Sie erscheint in keiner Warteschlange und in keiner
Historie; sie ändert nichts und schreibt nichts.

Der Lauf endet mit `0`, auch wenn nichts geantwortet hat: „nichts gefunden" ist
ein Ergebnis und kein Fehler. Mit `--json` steht am Ende ein Wert mit
`servers` und `count`, sonst eine Tabelle, während der Suche je Fund eine
Zeile auf `stderr`.

## `humanitl rules test`

Fragt den geltenden Regelsatz, was mit einer Anfrage geschähe. Ausgewertet wird
im Daemon, mit derselben Engine wie im Proxy-Pfad (ADR-018): Eine zweite
Auswertung in der Kommandozeile könnte anders antworten als die, die wirklich
entscheidet.

```
humanitl rules test <URL> [--method M] [--upgrade websocket] [--json]

verdict: block
rule: 018f0000-0000-7000-8000-00000000000a (bundled, position 3)
```

Ohne `--method` gilt `GET`. Trifft keine Regel, steht dort
`rule: none (default ask)`. Die Herkunft — `session`, `user` oder `bundled` —
kommt aus derselben Antwort wie das Verdikt und nicht aus einem zweiten Aufruf.

**Der Exit-Code trägt das Verdikt**, damit ein Skript es lesen kann, ohne die
Zeile zu zerlegen: `allow` endet mit `0`, `block` mit `10`, `ask` mit `11`.
`redact` endet ebenfalls mit `11`, weil eine solche Anfrage heute gehalten
wird. Ein Fehlschlag bleibt davon unberührt: eine URL ohne Schema oder mit
Fragment ist `CLI_004` und Exit `1`, bevor irgendjemand gefragt wird, und ohne
Daemon endet auch dieses Kommando mit `2`.

Mit `--json` steht eine Zeile auf `stdout`, mit `verdict`, `matched`,
`rule_id`, `origin`, `position` und `passthrough`; der Exit-Code bleibt
derselbe. `origin` und `passthrough` sind `null`, wo die Antwort die Regel
nicht führt — dasselbe „unbekannt" zweimal, nie ein `false`, das mehr behauptet
als bekannt ist.

Normalisiert wird im Daemon: Groß- und Kleinschreibung, ein Punkt am Ende und
ein internationalisierter Name gehen roh hinaus und werden dort behandelt wie
in einer echten Anfrage. `tests/escape/esc-4-rules.sh` fährt fünfzehn Zeilen der
Host-Tabelle aus HUM-022 auf genau diesem Weg.

## `humanitl llm test`

Fragt einen einzelnen Endpunkt, was er ist. Dieselbe Probe wie hinter dem
Testknopf im Setup und wie bei `doctor --probe-llm`: zwei GET-Anfragen im
Daemon, `/api/tags` und dann `/v1/models`, ohne Zugangsdaten und ohne
Weiterleitung, nichts davon durch die Sandbox.

```
humanitl llm test <URL> [--timeout-ms N] [--json]

flavor: ollama
latency: 12 ms
models: 2
  qwen2.5-coder:14b
  llama3.1:8b
```

Vor dem ersten Paket geht auf `stderr` eine Zeile heraus, die den Endpunkt und
die zwei Anfragen nennt — an der Ausgabesteuerung vorbei, damit auch `--json`
und `-q` sie nicht verschlucken. Gezeigt werden höchstens sechs Namen mit je
höchstens 40 Zeichen, der Rest als `+N more`; dieselben Grenzen wie in der
Oberfläche, denn ein Name aus dem Netz ist Text von einer Maschine, über die
noch niemand entschieden hat. Die vollständige Liste steht in `--json`.

Ein Endpunkt, der nicht antwortet, endet mit `1` und dem Befund des Daemons
(`LLM_001`; `LLM_007`, wenn die Adresse gar nicht lesbar war). Befunde, die
kein Fehlschlag sind, stehen unter der Ausgabe und der Lauf endet mit `0`:
`LLM_006` für eine Adresse außerhalb des eigenen Netzes, und `LLM_003` für
einen Server, der antwortet, aber als keine bekannte API — dann steht dort
`flavor: unknown` und keine Modellliste. Wer geantwortet hat, hat geantwortet.

## Die globalen Schalter

Fünf Schalter gelten für jedes Unterkommando, weil `clap` sie als `global`
führt (`daemon/bin/humanitl/src/cli.rs`, `GlobalOpts`). Sie stehen vor oder
hinter dem Unterkommando, beides geht.

| Schalter | Wirkung |
|---|---|
| `--json` | Maschinenlesbare Ausgabe: genau ein JSON-Wert auf `stdout`, Befunde eingeschlossen. Hinweise auf `stderr` entfallen; was ein Unterkommando im JSON-Modus schreibt, steht bei ihm. |
| `--config PATH` | Liest diese Konfigurationsdatei statt der im Konfigurationsverzeichnis des Nutzers. Der Pfad gilt für diesen Aufruf und ändert nichts an der Datei des Nutzers. |
| `-v`, `--verbose` | Erklärt, was gerade geschieht: Hinweise, die sonst stumm bleiben (`Renderer::detail`). Mehrfach angeben geht (`-vv`), heute ohne weitere Stufe. |
| `-q`, `--quiet` | Nur das Ergebnis, keine Hinweise: `Renderer::note` schreibt nichts mehr auf `stderr`. Schließt `-v` aus; Befunde und Fehler bleiben. |
| `--profile NAME` | Das Profil der Sitzung. Unter `humanitl sandbox` benennt dasselbe Flag das bwrap-Profil; welche Bedeutung gilt, entscheidet das Unterkommando. |

Der Ergebnistext eines Unterkommandos steht auf `stdout` und bleibt auch mit
`-q` stehen; `-q` nimmt nur die begleitenden Hinweise weg. Ein Befund
(`Diagnostic`) geht auf `stderr` und lässt sich mit keinem der Schalter
abstellen, weil er den Exit-Code erklärt.

## Was `run` mit den anderen Unterkommandos teilt

- `humanitl sandbox run` startet die Sandbox im Prozess der Kommandozeile und
  ist der Weg für Selbsttests und die Escape-Tests. `humanitl run` startet sie
  im Daemon; nur dort gibt es Proxy, Aufzeichnung und Warteschlange.
- `humanitl rules list` zeigt den Regelsatz, der gerade gilt — nach einem Start
  also den der Sitzung, samt der Regeln ihres Profils und ihrer Durchreiche.
- `humanitl daemon status` sagt, ob überhaupt ein Daemon da ist. Das ist die
  Antwort auf Exit 2.
- `humanitl doctor` sagt, ob `run` auf dieser Maschine überhaupt eine Sandbox
  bekommen kann. Das ist die Antwort auf Exit 3, bevor er eintritt.
- `humanitl daemon install` legt die Unit an, die den Daemon beim Anmelden
  startet. Das ist die Antwort auf die Zeile `daemon` des Doctors und auf
  `DAEMON_001` im Setup-Bildschirm.
