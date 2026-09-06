# Die Kommandozeile

`humanitl` ist ein dünner Client des Daemons (ADR-018). Jede Fähigkeit ist
zuerst ein RPC; die Kommandozeile ruft ihn auf und formatiert die Antwort. Sie
enthält keine Fachlogik, und sie erfindet nichts dazu.

Dieses Dokument beschreibt `humanitl run` und `humanitl doctor` vollständig und
die übrigen Unterkommandos nur so weit, wie sie `run` betreffen. Die kanonische
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
humanitl daemon install [--print] [--no-start]
```

**Genau eine Datei, an einem genannten Ort.**
`$XDG_CONFIG_HOME/systemd/user/humanitld.service`, sonst
`~/.config/systemd/user/humanitld.service`, mit den Rechten `0644`. Keine
System-Unit, kein `sudo`, keine zweite Datei, keine `humanitld.socket`. Ihr
`ExecStart` nennt das `humanitld` **neben der laufenden Kommandozeile** — nie
eines aus `PATH` und nie eines aus einem Konfigurationswert, damit beim
Anmelden dieselbe Fassung startet wie die, die die Unit geschrieben hat.

**Sichtbar, bevor es geschieht.** Der ganze Text der Unit und beide
`systemctl`-Aufrufe gehen vor dem ersten Schreibzugriff auf `stderr`, an der
Ausgabesteuerung vorbei: Weder `--json` noch `-q` können das abschalten.
`--print` zeigt dieselbe Datei und schreibt nichts.

**Wiederholbar.** Ein zweiter Aufruf mit demselben Ergebnis schreibt nicht
noch einmal; die Ausgabe sagt dann `unchanged` statt `created`.

**Nie über fremdes Eigentum.** Die erste Zeile der Unit ist die Marke
`# humanitl daemon install: written by Humanitl`. Fehlt sie in einer
vorhandenen Datei, gehört die Datei jemand anderem, und der Befehl weigert
sich mit `DAEMON_005`, statt sie zu überschreiben. Es gibt dafür mit Absicht
kein `--force`: Wer eine eigene Unit führt, legt sie beiseite.

**Danach `systemctl --user`, nie `sudo`.** Ohne `--no-start` laufen
`systemctl --user daemon-reload` und `systemctl --user enable --now
humanitld.service`. Gibt es kein `systemctl` in `PATH`, bleibt die Unit
liegen, es startet nichts, und die Ausgabe nennt die beiden Befehle.

**Ein Fehlschlag lässt nichts liegen.** Nimmt systemd die Unit nicht an, wird
der Zustand von vorher wiederhergestellt — eine angelegte Datei verschwindet,
eine ersetzte bekommt ihren alten Inhalt zurück — und `daemon-reload` läuft
noch einmal, damit auch systemds Bild davon stimmt. Der Befund ist
`DAEMON_008`.

Zurückgenommen wird dabei auch die Aktivierung. `systemctl --user enable --now`
ist ein Aufruf mit zwei Schritten: Er legt die Verweise unter
`<ziel>.wants/humanitld.service` an und startet dann den Dienst. Misslingt der
Start, stünden die Verweise ohne diese Rücknahme weiter da, und der Dienst
startete beim nächsten Anmelden, obwohl der Befehl mit `DAEMON_008` abgebrochen
ist. Entfernt werden nur die Verweise, die dieser Aufruf angelegt hat: Wer den
Dienst schon vorher aktiviert hatte, behält die Aktivierung.

Die Härtung der Unit ist gemessen und nicht behauptet: `bubblewrap` läuft
unter `SystemCallFilter=@system-service @mount @sandbox` und
`NoNewPrivileges=yes` durch, braucht aber `AF_NETLINK` in
`RestrictAddressFamilies`, um die Loopback-Schnittstelle im Namensraum
hochzubringen — ohne sie bricht es mit „loopback: Failed to create
NETLINK_ROUTE socket" ab, und ohne `lo` gibt es keine Brücke vom Shim zum
Proxy. `@sandbox` steht in der Zeile, weil der Filter einer Unit an jedes Kind
vererbt wird und `humanitl-shim` seinen eigenen seccomp-Filter mit `seccomp(2)`
installiert; erlaubt der Filter den Aufruf nicht, stirbt der Shim an `SIGSYS`,
bevor die dritte Sandbox-Garantie steht. Auf systemd 262 käme `seccomp` auch
ohne das Wort durch — `@system-service` enthält `@default`, und `@default`
enthält `@sandbox` —, aber diese Verschachtelung steht in keiner Zeile von
`systemd.exec(5)`, und eine Sandbox-Garantie hängt nicht an einer
undokumentierten Untergruppe. Jeder Pfad in `ReadWritePaths` trägt ein `-`, weil
systemd eine Unit mit einem nicht vorhandenen Pfad darin gar nicht erst startet
und auf einer frischen Installation keiner der drei existiert. Alles gemessen
am 2026-09-06 auf Debian 14 mit bubblewrap 0.12.0 und systemd 262 (HUM-044).

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
