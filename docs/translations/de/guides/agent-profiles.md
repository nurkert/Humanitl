---
source: guides/agent-profiles.md
source-sha256: a618e8ee7dece7bf9d1e4168b6d9f36c8825b01b09a1b6bedcfded8f09512a2b
lang: de
---
# Agentenprofile und Adapter

Applies to: Humanitl 0.1

Drei Dinge entscheiden, was geschieht, wenn Humanitl einen Agenten startet:

| Teil | Datei | Entscheidet |
|---|---|---|
| Session-Profil | `profiles/<name>.toml` | Konfigurationswerte und Regeln einer Session (`humanitl run --profile <name>`) |
| Sandbox-Profil | `profiles/sandbox/<name>.toml` | Mounts, den einen Proxy-Socket, seccomp und die Umgebung der Sandbox |
| Agenten-Adapter | in den Daemon einkompiliert (`agent.adapter`) | den Befehl des Agenten, seine Konfigurationsdateien, seine Umgebung und seine mitgelieferten Regeln |

Diese Anleitung erklärt das Format der Session-Profile, den OpenCode-Adapter Zeile für Zeile,
den pi-Adapter, wie du einen Adapter schreibst oder ein schlichtes Skript als Agenten
laufen lässt, und welche Hosts die bekannten Agenten von sich aus kontaktieren. Jeder
Konfigurationsschlüssel steht in [CONFIG.md](../CONFIG.md), die Regelsprache in der
[Regel-Referenz](../reference/rules.md) und die Kommandozeile in der
[CLI-Referenz](../reference/cli.md).

## Session-Profile

Ein Session-Profil bündelt, was eine Session ausmacht: die Konfigurationswerte, die für sie
gelten, und die Regeln, die zu ihr gehören. Es ist eine TOML-Datei und existiert an drei
Orten: mitgeliefert mit dem Programm, in deinem Konfigurationsverzeichnis und in einem
Projekt. `humanitl run --profile llm-only` und die App wählen über denselben Lader unter
denselben Profilen.

### Format

```toml
name = "llm-only"
description = "Pure inference. Only the configured LLM endpoint is reachable."

[config.hold]                 # configuration values, by group
ask_mode = "none"
timeout_secs = 1

[config.sandbox]
profile = "default"           # the file profiles/sandbox/default.toml
work_mode = "rw"

[rules]
files = ["team.yaml"]         # relative to this profile file
inline = [                    # rules in the schema of rules.yaml
  { action = "block", match = { host = "**" }, expires = "never", note = "..." },
]
```

| Schlüssel | Typ | Bedeutung |
|---|---|---|
| `name` | Zeichenkette | Der Name des Profils. Er muss zum Dateinamen und zu `^[a-z0-9-]{1,32}$` passen, sonst `CONFIG_003`. Ohne ihn ist der Dateiname ohne Endung der Name. |
| `description` | Zeichenkette | Ein Satz dazu, wofür das Profil gedacht ist. Angezeigt in `humanitl config schema --profiles`. |
| `[config.<group>]` | Tabellen | Jeder Konfigurationsschlüssel aus [CONFIG.md](../CONFIG.md), gruppiert wie in `config.toml`: `[config.hold]`, `[config.sandbox]`, `[config.agent]` und so weiter. |
| `[rules].files` | Liste von Pfaden | Regeldateien im Format von `rules.yaml`, aufgelöst relativ zum Verzeichnis der Profildatei, nie zum Arbeitsverzeichnis. |
| `[rules].inline` | Liste von Tabellen | Regeln, die direkt im Profil stehen, im Schema von `rules.yaml`. |

Es gibt genau vier Schlüssel auf oberster Ebene. Jeder andere Block auf oberster Ebene ist
`CONFIG_002`. Der häufigste Fehler ist eine Gruppe eine Ebene zu hoch: `[hold]` statt
`[config.hold]`. Die Diagnose sagt das.

Jedes Feld ist optional. Was ein Profil nicht nennt, erbt es Feld für Feld von der Schicht
darunter: `[config.hold]` mit nur `timeout_secs` lässt `ask_mode` unberührt. Listen ersetzen
und hängen nicht an: `rules.files` und `llm.passthrough_paths` aus einer höheren Schicht
bedeuten genau die Liste, die dort steht. Ein Name ist nie ein Pfad:
`--profile ../../etc/passwd` ist `CONFIG_003`.

### Mitgelieferte Profile

| Profil | Zweck |
|---|---|
| `default` | Alles, worüber keine Regel entscheidet, wird angehalten und einem Menschen gezeigt. |
| `llm-only` | Reine Inferenz: Nur der konfigurierte LLM-Endpunkt ist erreichbar, alles andere wird ohne Nachfrage blockiert. Keine App nötig. |

Beide gibt es zweimal: als Dateien unter `profiles/` und eingebettet im Daemon. Fehlen die
Dateien, startet der Daemon trotzdem. Eine gleichnamige Datei unter
`~/.config/humanitl/profiles/` gewinnt gegen die mitgelieferte, und das Laden meldet sie
(`CONFIG_008`), sobald ihr Text vom mitgelieferten abweicht.

**`default` setzt absichtlich keinen Wert.** Es liegt in der Reihenfolge der Schichten über
`config.toml`, also würde jeder Wert darin, selbst einer, der die Vorgabe wiederholt, deine
`config.toml` für diesen Schlüssel unwirksam machen, ohne dass du es siehst. Die üblichen
Einstellungen stehen als Kommentare in `profiles/default.toml`: Kopiere die Datei nach
`~/.config/humanitl/profiles/default.toml` und entferne das `#`, um sie dir zu eigen zu
machen.

**`llm-only` setzt sehr wohl Werte**, weil seine Wahl eine ausdrückliche Entscheidung ist, die
`config.toml` überschreiben soll. `ask_mode = "none"` bedeutet, dass eine Anfrage, über die
keine Regel entscheidet, nicht angehalten wird: Ihre Frist ist null, sie läuft sofort ab, und
der Agent bekommt `504` mit `reason: timeout`. `timeout_secs = 1` verhindert, dass eine
Fehlkonfiguration fünf Minuten lang hängt. Die Durchleitungsregel des Adapters zum Modell wird
vor der Blockade-Regel des Profils ausgewertet, egal wo sie in der Liste steht; jeder andere
Host fällt auf `block` mit dem Hinweis „llm-only profile".

### Reihenfolge der Schichten

Von unten nach oben; die höhere Schicht gewinnt. Jedes Feld merkt sich, aus welcher Schicht
sein Wert stammt (`Origin`), und sowohl die App als auch `humanitl config get
--origin` zeigen es an.

| Schicht | Quelle | `Origin` |
|---|---|---|
| 1 | eingebaute Vorgaben | `default` |
| 2 | `~/.config/humanitl/config.toml` | `config.toml` |
| 3 | Profil `default`: die Datei, sonst die eingebettete Fassung | `profile default` oder `profile builtin default` |
| 4 | das gewählte Profil, falls es nicht `default` ist | `profile <name>` oder `profile builtin <name>` |
| 5 | `<project>/.humanitl/profile.toml` | `project profile <path>` |
| 6 | Umgebungsvariablen `HUMANITL_*` | `env <name>` |
| 7 | Argumente auf der Kommandozeile | `command line` |

Schicht 3 gilt immer, auch wenn ein anderes Profil gewählt ist; das gewählte liegt darüber.
Welches Profil Schicht 4 einnimmt, wird in dieser Reihenfolge entschieden: der Name auf der
Kommandozeile, sonst der `name` im Projekt-Profil, sonst `default`. Ein Name, zu dem es weder
eine Datei noch ein mitgeliefertes Profil gibt, ist `CONFIG_001`, mit der Liste der bekannten
Profile. Eine Datei, die existiert, aber nicht lädt, gilt als vorhanden: Ein kaputtes Profil
hält den Start an, statt still zu verschwinden.

Das Projektverzeichnis ist `sandbox.work_dir`, sonst das aktuelle Verzeichnis. Der Ordner, der
in Einrichten oder Einstellungen gewählt wurde, wird dort gespeichert, in `config.toml`, sodass
die App, der Desktop-Starter, `humanitl run` und `humanitl sandbox run` alle im selben Ordner
starten; `--work DIR` überschreibt ihn für einen Aufruf und wird nicht gespeichert (HUM-293).
Der Schlüssel ist auf der Projektschicht gesperrt, damit ein Projekt-Profil nicht entscheiden
kann, wo nach ihm gesucht wird.

### Was ein Projekt-Profil nicht darf

`<project>/.humanitl/profile.toml` liegt in einem geklonten Repository und steht deshalb unter
dem Einfluss dessen, der dieses Repository geschrieben hat. Es unterliegt vier Grenzen:

1. **Kein anderes Profil.** Sein `name` wählt nur unter den mitgelieferten Profilen. Jeder
   andere Wunsch wird übersprungen und als `CONFIG_009` gemeldet; wer ein eigenes Profil meint,
   nennt es mit `--profile`.
2. **Nur erlaubte Schlüssel.** Jeder Schlüssel im Schema trägt
   `x-project-scope: allowed | denied` (Spalte „Projekt" in
   [CONFIG.md](../CONFIG.md)). Ein gesperrter Schlüssel auf dieser Schicht ist `CONFIG_003`.
   Gesperrt sind unter anderem `llm.*`, `sandbox.*` (einschließlich `sandbox.profile` und
   `sandbox.env`), `agent.adapter`, `agent.command`, `hold.ask_mode`, `findings.enabled`,
   `pseudonyms.*`, `resolver.*`, `experimental.*` und `recorder.retention_days`.
3. **Keine Mounts.** Mounts leben in Sandbox-Profilen, nicht in der Konfiguration. Ein
   Projekt-Profil, das `sandbox.mounts.extra_ro` oder `sandbox.mounts.extra_rw` nennt, wird mit
   `CONFIG_003` verweigert, mit dem Satz, dass nur globale Profile Pfade des Rechners in die
   Sandbox bringen dürfen.
4. **Keine Regeln.** Ein `[rules]`-Block in einem Projekt-Profil ist `CONFIG_003`. Ein
   Repository entscheidet nicht, was die Sandbox verlassen darf.

Ein Projekt-Profil, das einem anderen Konto gehört, ist eine Warnung (`CONFIG_007`), keine
Verweigerung: Die Grenzen halten die Datei so oder so.

### Regeln aus einem Profil

Regeln aus `[rules].files` und `[rules].inline` sind mitgelieferte Regeln. Die Reihenfolge, in
der ein Regelsatz aufgebaut wird, ist nicht die Reihenfolge, in der er ausgewertet wird. Die
Auswertung läuft in vier Rängen: zuerst die Durchleitungsregel des Adapters zum Modell, dann die
Session-Regeln, dann deine gespeicherten Regeln aus `rules.yaml`, zuletzt die mitgelieferten
Regeln, also die des Adapters (`rules/default.yaml`) und die der Profile. Innerhalb eines Rangs
gewinnt die erste passende Regel. Eine Anfrage, auf die keine Regel passt, wird angehalten
(Vorgabe `ask`).

Ein Profil kann die Durchleitung also nicht verdecken, nicht einmal mit `block host "**"`, und
du kannst eine mitgelieferte Regel überstimmen, ohne sie zu löschen. Nur der Lader markiert eine
Regel als mitgeliefert: Eine Inline-Regel mit `bundled = true` verliert die Markierung und
bekommt eine Warnung (`RULES_010`). Einzelheiten zu den Rängen stehen in der
[Regel-Referenz](../reference/rules.md).

### Auf der Kommandozeile

```sh
humanitl config schema --profiles            # which profiles exist
humanitl config get hold.ask_mode --profile llm-only --origin
humanitl run --profile llm-only              # start a session with it
```

`--profile NAME` bezeichnet zwei verschiedene Dinge, und das Unterkommando entscheidet, welches:

| Befehl | `--profile` bedeutet |
|---|---|
| `humanitl sandbox run`, `sandbox argv`, `sandbox check` | das Sandbox-Profil unter `profiles/sandbox/` |
| alles andere (`run`, `config`, `rules`, `flows`, `daemon`) | das Session-Profil |

Unter `run` wird das Sandbox-Profil mit `--sandbox-profile` oder `sandbox.profile` gewählt.

### Ein eigenes Profil schreiben

```sh
mkdir -p ~/.config/humanitl/profiles
cat > ~/.config/humanitl/profiles/work.toml <<'EOF'
name = "work"
description = "Shorter hold timeout, German interface."

[config.hold]
timeout_secs = 120

[config.ui]
language = "de"

[rules]
files = ["work-rules.yaml"]
EOF

humanitl config schema --profiles
humanitl run --profile work
```

`work-rules.yaml` wird neben `work.toml` gesucht, unter `~/.config/humanitl/profiles/`.

## Sandbox-Profile

Ein Sandbox-Profil ist die vollständige Richtlinie der Sandbox. Daraus baut der Daemon eine
bubblewrap-Kommandozeile, die `humanitl sandbox argv` und der Sandbox-Bildschirm der App Wort
für Wort zeigen. Das mitgelieferte `profiles/sandbox/default.toml` kommentiert jede Zeile; kurz
gefasst:

| Tabelle | Was sie enthält |
|---|---|
| `[sandbox]` | Backend, Hostname, die abzutrennenden Namespaces (alle; ein Profil, das einen auslässt, wird verweigert), `min_bwrap_version` |
| `[mounts]` | `/work` und seinen Modus, nur lesbare Systempfade, tmpfs über Verzeichnissen, aus denen Werkzeuge Code ausführen würden, verdeckte Dateien wie `/work/.env` und deine eigenen Pfade `extra_ro` und `extra_rw` |
| `[network]` | den Proxy-Socket in der Sandbox, den Proxy-Port, den CA-Pfad und genau eine Bridge |
| `[seccomp]` | erlaubte Socket-Familien und -Typen (eine Untergrenze in beide Richtungen) und die Syscalls, die immer verweigert werden |
| `[env]` | die Umgebung: Proxy-Variablen und der CA-Pfad für jedes gängige Werkzeug |

Deine eigenen Sandbox-Profile kommen nach `~/.config/humanitl/profiles/sandbox/<name>.toml`.
Jeder Pfad des Rechners darin wird gegen eine feste Sperrliste geprüft (`SANDBOX_006`), sodass
ein Profil weder `~/.ssh` noch das Laufzeitverzeichnis einhängen kann. Eine zweite Bridge oder
eine andere Socket-Familie wird mit `CONFIG_003` verweigert. Warum es diese Grenzen gibt, steht
in [SECURITY.md](../SECURITY.md).

## Der OpenCode-Adapter

OpenCode ist der Standardadapter in Humanitl 0.1 (`agent.adapter = "opencode"`); der zweite ist
pi, siehe [pi verwenden](#using-pi).
Sein Code ist `daemon/crates/sandbox/src/agent/opencode.rs`; alles, was er in die Sandbox
schreibt, liegt unter `agents/opencode/` und ist einkompiliert. Zur Laufzeit wird von dort
nichts gelesen und nichts heruntergeladen.

OpenCode braucht einen Anbieter für einen OpenAI-kompatiblen Endpunkt und tut beim Start von sich
aus mehrere Dinge im Netzwerk: Es holt einen Modellkatalog, prüft auf neue Versionen, installiert
Anbieterpakete mit npm und bietet eine gehostete Websuche an. Nichts davon hat jemand verlangt,
und die erste angehaltene Anfrage, die ein Mensch sieht, sollte eine sein, die er versteht. Der
Adapter schaltet diese Dinge deshalb auf drei Wegen ab: über die Umgebung, über seine
Konfigurationsdatei und über seine mitgelieferten Regeln als zweites Schloss. Die Werte wurden
gegen OpenCode 1.18.25 geprüft, die Berechtigungen und Zeitgrenzen aus HUM-308 gegen 1.18.31;
andere Versionen wurden nicht geprüft.

### Die Konfigurationsvorlage

`agents/opencode/opencode.json.tmpl`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "humanitl-local": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Humanitl local LLM",
      "options": {
        "baseURL": "{{LLM_BASE_URL}}"
      },
      "models": {}
    }
  },
  "model": "humanitl-local/{{DEFAULT_MODEL}}",
  "autoupdate": false,
  "share": "disabled",
  "permission": {
    "webfetch": "allow",
    "bash": "allow",
    "websearch": "deny",
    "external_directory": "deny"
  }
}
```

Zeile für Zeile:

| Zeile | Bedeutung |
|---|---|
| `"$schema"` | Das JSON-Schema der OpenCode-Konfiguration, für Editoren und Validierung. |
| `"provider": { "humanitl-local": … }` | Ein Anbieter, der Modellserver, den du in `llm.endpoint` einstellst. |
| `"npm": "@ai-sdk/openai-compatible"` | Das Anbieterpaket, das OpenCode für eine OpenAI-kompatible API lädt. OpenCode installiert es bei der ersten Verwendung von `registry.npmjs.org`; die mitgelieferten Regeln machen daraus eine angehaltene Anfrage (siehe [Bekannte Hosts, die nach Hause telefonieren](#known-phone-home-hosts)). |
| `"baseURL": "{{LLM_BASE_URL}}"` | Ersetzt durch `llm.endpoint` plus `/v1`, außer der Endpunkt endet schon auf `/v1`. Ohne Endpunkt schreibt der Adapter einen Platzhalter auf dem Loopback der Sandbox. |
| `"models": {}` | Gefüllt mit einem Eintrag je Modell in `llm.models`. |
| `"model": "humanitl-local/{{DEFAULT_MODEL}}"` | Das erste Modell aus `llm.models`. Bei leerer Liste das Platzhaltermodell `default` und die Warnung `LLM_004`. |
| `"autoupdate": false` | Keine Selbstaktualisierung. |
| `"share": "disabled"` | Kein Teilen von Sessions, das das ganze Gespräch hochladen würde. |
| `"permission"` | Die eigenen Berechtigungen von OpenCode; siehe unten. |

Der Adapter parst die Vorlage als JSON und setzt Werte; er ersetzt nie Text. Ein Modellname mit
einem Anführungszeichen kann keine Struktur in die Datei einschleusen. Ist ein Zugangsschlüssel
gespeichert (`humanitl llm key set`), bekommt der Anbieter zusätzlich
`"apiKey": "{file:/etc/humanitl/opencode/llm-api-key}"`; der Schlüssel selbst ist eine eigene,
nur lesbare Datei und steht nie in der Konfiguration.

### Berechtigungen

| Berechtigung | Wert | Warum |
|---|---|---|
| `webfetch` | `allow` | OpenCode ruft ab, ohne in seiner eigenen Oberfläche zu fragen. Der Abruf geht durch Humanitl und wird angehalten wie jede andere Anfrage, also entscheidest du einmal, in Anhalten, nicht zweimal (HUM-308). |
| `bash` | `allow` | Befehle laufen ohne Abfrage des Agenten. Sie laufen in der Sandbox; ein `curl` darunter wird am Proxy angehalten wie jede andere Anfrage. |
| `websearch` | `deny` | Das Suchwerkzeug spricht mit einem gehosteten Suchdienst, nicht mit der Seite. |
| `external_directory` | `deny` | Kein Zugriff auf Verzeichnisse außerhalb des Projekts. In der Sandbox gibt es wenig zu erreichen, aber der Agent soll es nicht versuchen. |

`edit` steht absichtlich nicht im Block. OpenCode erlaubt es standardmäßig, und weil dieser Block
über die eigenen Berechtigungen jedes Agenten gelegt wird, würde ein ausdrückliches
`"edit": "allow"` auch den Plan-Agenten von OpenCode schreiben lassen, der nur lesen soll. Warum
die eigenen Abfragen des Agenten in der Sandbox nichts schützen, steht in
[SECURITY.md](../SECURITY.md#the-agents-own-permission-prompts-hum-308).

**Wie lange ein Werkzeug wartet.** Eine angehaltene Anfrage wartet bis zu `hold.timeout_secs`
(standardmäßig 300 Sekunden). `webfetch` von OpenCode gibt nach 30 Sekunden auf, oder nach
höchstens 120, wenn das Modell um mehr bittet; in OpenCode 1.18.31 ist diese Grenze fest im Code.
Gibt ein Werkzeug auf, schließt es die Verbindung, der Proxy beendet den Flow als
`client_timeout`, und die Karte verlässt die Warteschlange in Anhalten, bevor du entschieden
hast. Das Briefing sagt dem Agenten das und dass `curl` wartet, sofern das Modell keine eigene
Zeitgrenze mitgibt. Für `bash`
hebt der Adapter die Standard-Zeitgrenze mit
`OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS` an (siehe die nächste Tabelle).

Derselbe `permission`-Block geht auch in die Umgebungsvariable `OPENCODE_PERMISSION`, die
OpenCode zuletzt über jede Konfigurationsquelle legt. Das zählt wegen der Art, wie OpenCode seine
Konfiguration zusammenführt: Spätere Quellen gewinnen, in dieser Reihenfolge: das
Konfigurationsverzeichnis, `OPENCODE_CONFIG`, **dann** die `opencode.json` des Projektbaums,
`.opencode`-Verzeichnisse, `OPENCODE_CONFIG_CONTENT`, die Konfiguration einer Organisation und
zuletzt das verwaltete Verzeichnis (`/etc/opencode` auf Linux). Ein geklontes Repository läge
über `OPENCODE_CONFIG`. Nur das verwaltete Verzeichnis und `OPENCODE_PERMISSION` liegen darüber,
also schreibt der Adapter dieselbe Datei an drei Orte und setzt die Variable.

Der `provider`-Block lässt sich auf diese Weise nicht erzwingen: Das Zusammenführen ist additiv,
und ein Projekt kann einen eigenen Anbieter mit eigener Adresse hinzufügen. Damit fällt keine
Garantie, weil aller Verkehr dorthin durch den Proxy geht und angehalten wird; welches Modell
voreingestellt ist, entscheidet weiterhin Humanitl.

### Strg+C beendet OpenCode nicht

OpenCode 1.18.31 beendet sich standardmäßig bei `ctrl+c,ctrl+d,<leader>q`, und im Terminal von
Humanitl erreicht `Ctrl+C` den Agenten als `0x03`, sodass ein reflexhaftes `Ctrl+C` zum
Kopieren die Session beendet hat. Seit HUM-353 kopiert das Terminal der App bei `Ctrl+C`, während
sichtbarer Text markiert ist, und sendet dann nichts; ohne Markierung und in `humanitl attach`
erreicht `Ctrl+C` weiterhin den Agenten. Der Adapter schreibt
`agents/opencode/tui.json.tmpl` als `~/.config/opencode/tui.json` mit
`"keybinds": { "app_exit": "ctrl+d,<leader>q" }` (HUM-353). Beende mit `Ctrl+D` bei leerer
Eingabezeile oder mit `Ctrl+X q`; `Ctrl+C` leert weiterhin die Eingabe (`input_clear`), und `Esc`
unterbricht weiterhin (`session_interrupt`). Die Datei liegt nicht im verwalteten Verzeichnis,
also gewinnt eine `tui.json` in deinem Projekt und kann `app_exit` anders belegen. Nur gegen die
Zeichenketten im Binary von OpenCode 1.18.31 geprüft.

### Dateien in der Sandbox

| Pfad in der Sandbox | Inhalt |
|---|---|
| `/etc/humanitl/opencode/opencode.json` | die gerenderte Konfiguration (`OPENCODE_CONFIG` zeigt hierher) |
| `~/.config/opencode/opencode.json` | dieselbe Datei, im Konfigurationsverzeichnis von OpenCode |
| `/etc/opencode/opencode.json` | dieselbe Datei, im verwalteten Verzeichnis, das gegen das Projekt gewinnt |
| `/etc/humanitl/opencode/models.json` | der Modellkatalog, gerendert aus `agents/opencode/models.json` |
| `/etc/humanitl/opencode/llm-api-key` | der Zugangsschlüssel, nur wenn einer gespeichert ist |
| `~/.config/opencode/tui.json` | die Tastenbelegung der Oberfläche: `app_exit` ohne `ctrl+c` (HUM-353) |
| `~/.config/opencode/AGENTS.md` | das Briefing des Agenten in der Sprache von `ui.language`, außer bei `agent.briefing.enabled = false` |

`~` ist das Home des Agenten, `/home/agent`, ein leeres tmpfs. Jede Datei stammt aus einem
versiegelten memfd und ist nur lesbar eingehängt. Keine Adapter-Datei darf unter `/work` liegen:
Humanitl schreibt nie in dein Repository, und der Agent darf seine eigene Konfiguration nicht mit
einem Commit umschreiben können.

`models.json` enthält keine Daten aus einem öffentlichen Katalog. Die Datei beschreibt genau einen
Anbieter, `humanitl-local`, mit Platzhalterwerten für `release_date` (`1970-01-01`) und die
Grenzen (`context` 32768, `output` 4096). Passe sie in `agents/opencode/models.json` an, wenn du
dein Modell kennst.

### Umgebung

Der Adapter fügt der Umgebung des Sandbox-Profils diese Variablen hinzu:

| Variable | Wert | Wirkung |
|---|---|---|
| `OPENCODE_DISABLE_AUTOUPDATE` | `true` | Keine Versionsprüfung gegen GitHub. Die Regel `…0002` blockiert die Release-API als zweites Schloss. |
| `OPENCODE_MODELS_PATH` | `/etc/humanitl/opencode/models.json` | OpenCode liest seinen Modellkatalog aus dieser Datei statt aus dem Netzwerk. |
| `OPENCODE_DISABLE_MODELS_FETCH` | `true` | Die stündliche Aktualisierung des Katalogs im Hintergrund läuft nicht. |
| `OPENCODE_CONFIG` | `/etc/humanitl/opencode/opencode.json` | Die Konfiguration außerhalb von `/work`. |
| `OPENCODE_AUTO_SHARE` | `false` | Kein automatisches Teilen einer Session. |
| `OPENCODE_DISABLE_SHARE` | `true` | Überhaupt kein Teilen. |
| `OPENCODE_ENABLE_EXA` | `false` | Keine gehostete Websuche (Exa). |
| `OPENCODE_ENABLE_PARALLEL` | `false` | Keine gehostete Websuche (Parallel). |
| `OPENCODE_DISABLE_LSP_DOWNLOAD` | `true` | Keine Sprachserver aus dem Netzwerk herunterladen. |
| `OPENCODE_PERMISSION` | der `permission`-Block als JSON | Zuletzt über jede Konfigurationsquelle gelegt. |
| `OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS` | `hold.timeout_secs` plus 60 Sekunden, in Millisekunden, mindestens `120000` (standardmäßig `360000`) | Ein Befehl in `bash` wartet auf eine angehaltene Anfrage, statt nach den 120 Sekunden von OpenCode abgeschnitten zu werden. Nicht gesetzt bei `hold.ask_mode = "none"`, wo nichts wartet. Das Modell kann für einen Aufruf weiterhin eine eigene Zeitgrenze mitgeben. |
| `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` | unter `/home/agent` | Der Zustand von OpenCode bleibt im tmpfs, nicht auf dem Rechner. |
| `NODE_EXTRA_CA_CERTS` | `/etc/humanitl/ca.crt` | Node vertraut der lokalen CA des Proxys. |
| `TERM`, `COLORTERM`, `LANG` | `xterm-256color`, `truecolor`, `C.UTF-8` | Das Terminal des Agenten. |

**`OPENCODE_MODELS_URL` ist absichtlich nicht gesetzt.** Ihr Wert ist eine Basisadresse (Vorgabe
`https://models.opencode.ai`), an die OpenCode `/api.json` anhängt und die es über HTTP abruft;
eine `file://`-URL funktioniert dort nicht. Sie auf eine lokale Datei zeigen zu lassen, hätte eine
zweite Bridge in die Sandbox gebraucht. Mit `OPENCODE_MODELS_PATH` und
`OPENCODE_DISABLE_MODELS_FETCH` behält die Sandbox ihre eine Tür.

`sandbox.env` wird nach dem Adapter angewendet und gewinnt. Setzt du dort `HOME` oder
`XDG_CONFIG_HOME`, folgen die Dateien des Adapters an diesen Ort.

### Die Durchleitungsregel

Aus `llm.endpoint` baut der Adapter eine Regel mit fester ID (`…00ff`): `POST` und `GET` zu genau
diesem Host, Schema und Port erlauben, nur auf den Inferenz-Pfaden (`/v1/chat/completions`,
`/v1/completions`, `/v1/responses`, `/v1/embeddings`, `/v1/models` und für Ollama `/api/chat`,
`/api/generate`, `/api/embed`, `/api/embeddings`, `/api/tags`, `/api/show`, `/api/ps`,
`/api/version`), mit `allow_private`, weil das Modell meist in deinem eigenen Netzwerk steht. Der
Proxy hält diese Anfragen nie an; er zeichnet sie vollständig auf und warnt bei Funden, statt sie
anzuhalten (`LLM_005`). Pfade, die den Server verändern, etwa `/api/pull` oder `/v1/files`, gehen
durch die Warteschlange. Um einen davon durchzulassen, füge seinen Pfad zu
`llm.passthrough_paths` hinzu.

### Prüfungen vor dem Start

Bevor irgendetwas startet, prüft der Adapter auf dem Rechner, ohne das Netzwerk zu berühren:

- `AGENT_001`: `opencode` ist nicht im `PATH`, und `agent.command` ist nicht gesetzt.
- `AGENT_002`: Die Datei ist da, aber nicht ausführbar.
- `AGENT_004`: Das Binary liegt auf dem Rechner, ist aber in der Sandbox nicht sichtbar, zum
  Beispiel unter `~/.opencode/bin`. Die Abhilfe kopiert es nach `/usr/local/bin`.
- `LLM_004`: `llm.models` ist leer, also bekommt der Agent ein Platzhaltermodell.

OpenCode ist eine Vollbild-TUI. `humanitl run --ask terminal` verweigert es deshalb mit
`CLI_002`; entscheide in der App (`--ask ui`) oder lass Anfragen ohne Regel blockieren
(`--ask none`). Alle Codes sind in [DIAGNOSTICS.md](../DIAGNOSTICS.md) erklärt.

## pi verwenden

pi (<https://pi.dev/>, npm-Paket `@earendil-works/pi-coding-agent`) ist der zweite Adapter
(`agent.adapter = "pi"`). Sein Code ist `daemon/crates/sandbox/src/agent/pi.rs` und
`pi_config.rs`; seine Vorlagen liegen unter `agents/pi/` und sind einkompiliert. Die Werte unten
wurden an pi 1.3.0 gemessen; andere Versionen wurden nicht geprüft.

### Installieren

pi ist ein Node.js-Skript (`#!/usr/bin/env node`, Node.js 22.19 oder neuer). Die Sandbox hängt
`/usr` nur lesbar ein und nichts unter deinem Home, und der Daemon bindet nur native Programme
aus einem Verzeichnis des Nutzers ein, nie ein Skript. Installiere pi und Node.js beide unter
`/usr`:

```sh
sudo npm install -g --ignore-scripts @earendil-works/pi-coding-agent
```

Eine globale npm-Installation landet unter `/usr/local` (oder `/usr` mit dem npm einer
Distribution). Das Installationsskript von `pi.dev` legt pi unter `$HOME` ab, das die Sandbox
nicht sieht; ein Node.js aus nvm unter `$HOME` ist ebenfalls nicht sichtbar.

### Auswählen

```sh
humanitl config set agent.adapter pi
```

oder `agent.adapter = "pi"` in `config.toml`, oder die Gruppe „Agent" im Einstellungsformular.
Der Daemon verweigert eine ID, die er nicht kennt, mit `CONFIG_003` und nennt die bekannten. Die
Wahl gilt für die nächste Session.

### Was Humanitl schreibt

Alles, was pi liest, liegt in seinem Agentenverzeichnis, `PI_CODING_AGENT_DIR`, hier
`/home/agent/.pi/agent`. pi schreibt dort auch bei jedem Start (`auth.json`,
`models-store.json`, `sessions/`, Sperrverzeichnisse), also bleibt das Verzeichnis im
tmpfs-Home beschreibbar; nur die Dateien, die Humanitl ablegt, sind nur lesbar.

| Pfad in der Sandbox | Inhalt |
|---|---|
| `~/.pi/agent/models.json` | ein Anbieter `humanitl-local` mit `baseUrl` aus `llm.endpoint` (dieselbe Basis-URL, die OpenCode bekommt, samt Pfadpräfix), `api: "openai-completions"`, ein Modell je Eintrag in `llm.models` |
| `~/.pi/agent/settings.json` | `defaultProvider = "humanitl-local"`, `defaultModel` = das erste Modell, `enableInstallTelemetry = false`; mit Webzugriff zusätzlich `packages` = das Verzeichnis von `pi-web-access` |
| `~/.pi/agent/web-search.json` | nur mit Webzugriff: `ssrf.trustEnvProxy = true` (siehe „Websuche und Abruf") |
| `/etc/humanitl/pi/llm-api-key` | der Zugangsschlüssel, nur wenn einer gespeichert ist |
| `~/.pi/agent/AGENTS.md` | das Briefing in der Sprache von `ui.language`, außer bei `agent.briefing.enabled = false` |

Beide JSON-Dateien werden mit einer JSON-Bibliothek geparst und neu aufgebaut, nie als Text
ausgefüllt: Ein Modellname mit einem Anführungszeichen kann keine Struktur einschleusen. Bei
leerem `llm.models` bekommt pi das Platzhaltermodell `default` und die Warnung `LLM_004`. Die
Grenzen jedes Modells (`contextWindow` 32768, `maxTokens` 4096) sind vorsichtige Vorgaben; passe
sie in `agents/pi/models.json.tmpl` an, wenn du dein Modell kennst.

Endpunkt, Modelle und Schlüssel sind die des aktiven gespeicherten Modellservers, genau wie bei
OpenCode: Der Schlüssel geht nur mit dem Endpunkt desselben Eintrags, und ein Wechsel gilt für
die nächste Session.

### Umgang mit dem Schlüssel

`apiKey` in `models.json` ist `"!cat /etc/humanitl/pi/llm-api-key"`. pi führt einen Wert, der mit
`!` beginnt, über `/bin/sh -c` aus und nimmt die getrimmte Ausgabe als Schlüssel. Die
Schlüsseldatei stammt wie jede Adapter-Datei aus einem versiegelten memfd; der Schlüssel steht in
keiner Umgebungsvariable, nicht im `argv` von `pi` und nicht in `models.json`. Nur der Pfad steht
im `argv` des kurzlebigen `cat`. Der Pfad ist eine Konstante des Adapters, nie ein Wert aus der
Konfiguration, also erreicht nichts, was ein Nutzer oder ein Projekt schreibt, diese Shell-Zeile.

Ohne gespeicherten Schlüssel ist `apiKey` der Platzhalter `humanitl-no-key`: pi blendet einen
Anbieter ohne Schlüssel aus und würde ganz ohne Modell starten. Ein Server ohne Authentifizierung
ignoriert den Wert.

### Umgebung

| Variable | Wert | Wirkung |
|---|---|---|
| `PI_CODING_AGENT_DIR` | `/home/agent/.pi/agent` | Wo pi seine Dateien liest und schreibt. |
| `PI_OFFLINE` | `1` | Keine automatische Netzwerkaktivität: Versionsprüfung, Aktualisierung des Katalogs, Prüfung auf Paket-Updates, Herunterladen von Werkzeugen. Modellaufrufe sind nicht betroffen. |
| `PI_SKIP_VERSION_CHECK` | `1` | Keine Anfrage an `pi.dev/api/latest-version`. |
| `PI_TELEMETRY` | `0` | Keine Installationstelemetrie. |
| `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` | unter `/home/agent` | Der Zustand von pi bleibt im tmpfs. |
| `NODE_EXTRA_CA_CERTS` | `/etc/humanitl/ca.crt` | Node vertraut der lokalen CA des Proxys. |
| `TERM`, `COLORTERM`, `LANG` | `xterm-256color`, `truecolor`, `C.UTF-8` | Das Terminal des Agenten. |

Keine Variable trägt ein Secret, und alle vier `PI_*`-Variablen werden mit Namen in der
Umgebungsliste des Sandbox-Bildschirms angezeigt. `sandbox.env` gewinnt gegen den Adapter: Setzt
du dort `PI_CODING_AGENT_DIR`, liest pi keine der Dateien, die Humanitl schreibt.

### Was pi von sich aus tut

| Host | Was | Regel | Auch abgeschaltet durch |
|---|---|---|---|
| `pi.dev`, Pfad `/api/**` | Versionsprüfung, Modellkatalog, Installer für `pi update` | block (`…0101`) | `PI_OFFLINE`, `PI_SKIP_VERSION_CHECK` |
| `radius.pi.dev` | Hochladen von Fehlerberichten (`/bug`), Teilen von Sessions über Radius | block (`…0102`) | nichts; nur bei einem solchen Befehl |
| `github.com`, Pfad `/sharkdp/fd/releases/**` | Herunterladen von `fd`, wenn es fehlt | block (`…0103`) | `PI_OFFLINE` |
| `github.com`, Pfad `/BurntSushi/ripgrep/releases/**` | Herunterladen von `rg`, wenn es fehlt | block (`…0104`) | `PI_OFFLINE` |

Mit `PI_OFFLINE=1` sah ein protokollierender Proxy vor dem ersten Prompt überhaupt keine Anfrage.
Andere vom Nutzer ausgelöste Pfade (`/share` als GitHub-Gist, `pi update` über npm, Pakete aus
einem vertrauenswürdigen Projekt) werden nicht blockiert und gehen wie jede andere Anfrage durch
die Warteschlange. Ohne `fd` gibt pi einmal „Offline mode enabled, skipping download" aus und
arbeitet ohne.

### Vertrauen in Projekte

Die `.pi/settings.json`, Erweiterungen und Skills eines Projekts laden erst nach der eigenen
Vertrauensabfrage von pi, die du in der TUI beantwortest; Humanitl überschreibt
`defaultProjectTrust` nicht. Ein vertrauenswürdiges Projekt kann ein anderes Standardmodell wählen
oder einen Anbieter hinzufügen; jede Anfrage geht trotzdem durch den Proxy und wird angehalten.
`models.json` wird nur aus dem Agentenverzeichnis gelesen, also kann ein Projekt die Basis-URL des
mitgelieferten Anbieters nicht umschreiben.

### Websuche und Abruf

pi hat kein eigenes Web-Werkzeug. Das npm-Paket `pi-web-access` (MIT,
<https://github.com/nicobailon/pi-web-access>, gemessen mit 0.46.0) fügt `web_search`,
`fetch_content`, `source_check` und `get_search_content` hinzu.
`pi install npm:pi-web-access` hilft hier nicht: Es schreibt nach `~/.pi/agent` auf deinem
Rechner, und das Agentenverzeichnis von pi in der Sandbox ist ein frisches tmpfs. Installiere das
Paket global, neben pi:

```sh
sudo npm install -g --ignore-scripts pi-web-access
```

Humanitl sucht es im `node_modules`-Verzeichnis, das pi enthält, dann unter
`/usr/local/lib/node_modules` und `/usr/lib/node_modules`, und zählt ein Verzeichnis nur, wenn die
Sandbox seine `package.json` erreicht und diese Datei `pi-web-access` nennt. Wird es gefunden,
führt `settings.json` das Verzeichnis unter `packages` auf, und pi lädt es von dort nur lesbar,
ohne etwas zu installieren. Der Schalter ist `agent.pi.web_access`:

| Wert | Wirkung |
|---|---|
| `auto` (Vorgabe) | das Paket laden, wenn es installiert ist, sonst ohne starten und nichts sagen |
| `on` | dasselbe, aber ein fehlendes Paket ergibt die Warnung `AGENT_007` mit dem npm-Befehl |
| `off` | nie laden |

```sh
humanitl config set agent.pi.web_access off
```

Der Schlüssel ist für Projekt-Profile gesperrt: Code von Dritten in den Agenten zu laden, ist
deine Entscheidung, nicht die des geklonten Repositorys. Das Einstellungsformular hat noch kein
Feld dafür.

**Jede Anfrage geht weiterhin durch den Proxy.** Sie wird in Anhalten angehalten, wenn keine
Regel über sie entscheidet; eine Regel entscheidet ohne Anhalten (die mitgelieferte Regel
`**.exa.ai` blockiert die Suche ohne Schlüssel, eine `allow`-Regel leitet sofort weiter). Die
Erweiterung nutzt `fetch` von Node, das pi über seinen eigenen `EnvHttpProxyAgent` auf
`HTTPS_PROXY` leitet. Gemessen mit einem protokollierenden Proxy: `fetch_content` auf einer
`http://`-Seite kam als `CONNECT host:80` an, die Suche ohne Schlüssel als
`CONNECT mcp.exa.ai:443`. Die Erweiterung prüft jedes Ziel mit einer eigenen DNS-Abfrage, bevor
sie abruft; die Sandbox hat keinen Resolver, also scheiterte jeder Abruf mit `Failed to resolve`,
bis `web-search.json` `ssrf.trustEnvProxy` setzte. Diese Datei ist nur lesbar: Die eigenen Befehle
der Erweiterung, die eine Anbieterwahl speichern, scheitern in der Sandbox.

**Schlüssel und Suche.** Eine URL abzurufen braucht keinen Schlüssel. Ohne Schlüssel geht die
Suche an den schlüssellosen MCP-Endpunkt `mcp.exa.ai` von Exa, und die mitgelieferte Regel
`…0006` blockiert `**.exa.ai` für jede Session (sie wurde für die gehostete Suche von OpenCode
geschrieben). Ohne weiteres Zutun funktioniert also `fetch_content`, und `web_search` meldet, dass
die Exa-Suche gescheitert ist. Eine Regel von dir gewinnt gegen die mitgelieferte; um die Suche
durchzulassen, füge eine für den Host hinzu, zum Beispiel `ask` mit `host: "mcp.exa.ai"` in
`rules.yaml`. Suchanbieter mit API-Schlüssel (Brave, Tavily, Jina und andere) sind nicht
angebunden: Kein Schlüssel aus dem Schlüsselbund erreicht die Sandbox, und die Erweiterung findet
in der geleerten Umgebung keinen.

### Terminal und Prüfungen

pi ist eine Vollbild-TUI. `humanitl run --ask terminal` verweigert es mit `CLI_002`, wie bei
OpenCode. Ein einzelnes `Ctrl+C` beendet pi nicht: Es leert den Editor (`app.clear`), und erst ein
zweites innerhalb von 500 ms beendet, ebenso wie `Ctrl+D` bei leerem Editor (`app.exit`). Der
Adapter lässt die Tastenbelegung von pi deshalb in Ruhe (HUM-353, geprüft gegen pi 1.4.0). Vor
dem Start prüft der Adapter:

- `AGENT_001`: `pi` wird nicht gefunden; die Abhilfe ist der npm-Befehl oben.
- `AGENT_002`: wie bei OpenCode, mit pi als Namen.
- `AGENT_004`: pi liegt auf dem Rechner dort, wo die Sandbox nicht hinschaut. Die Abhilfe ist der
  npm-Befehl oben; das eine Skript zu kopieren, ließe seine Paketdateien zurück.
- `AGENT_006`: `pi` ist erreichbar, aber seine `#!`-Zeile verlangt `node`, und die Sandbox findet
  kein startbares `node`. Die Abhilfe fügt ein eingehängtes `node` zu `sandbox.env.PATH` hinzu,
  kopiert das `node` des Rechners nach `/usr/local/bin` oder verweist auf die Downloadseite von
  Node.js. Nennt die `#!`-Zeile ein absolutes `node` außerhalb der Mounts, ist die Abhilfe, pi mit
  einem Node.js unter `/usr` oder `/usr/local` neu zu installieren.
- `AGENT_007` (Warnung): `agent.pi.web_access = "on"`, und `pi-web-access` ist nicht dort
  installiert, wo die Sandbox es sieht; pi startet ohne Web-Werkzeuge.
- `LLM_004`: `llm.models` ist leer.

## Einen Adapter schreiben

Ein Adapter ist Rust-Code im Crate `humanitl-sandbox`, registriert in
`AdapterRegistry::builtin()` (`daemon/crates/sandbox/src/agent/mod.rs`). Es gibt kein Laden von
Plugins: Adapter sind in den Daemon einkompiliert. Der Trait, genau wie im Code:

```rust
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn command(&self, ctx: &AgentContext) -> Vec<OsString>;
    fn env(&self, ctx: &AgentContext) -> Vec<(String, String)>;
    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic>;
    fn default_rules(&self) -> &'static str;
    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule>;
    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic>;
    fn program(&self) -> &'static str;
    fn is_agent_command(&self, command: &[std::ffi::OsString]) -> bool { /* provided */ }
    fn is_fullscreen_tui(&self) -> bool;
}
```

| Methode | Vertrag |
|---|---|
| `id` | Der Wert von `agent.adapter`, der diesen Adapter auswählt. |
| `command` | Der Befehl, den der Shim nach seccomp startet. Ist `ctx.agent_command_override` gesetzt (`agent.command`), genau dieser. |
| `env` | Variablen zusätzlich zur Umgebung des Sandbox-Profils. |
| `files` | Dateien, die vor dem `exec` in der Sandbox existieren müssen, als `SandboxFile::read_only` oder `SandboxFile::secret`. Nie unter `/work`, nie auf einem Pfad, den die Sandbox selbst setzt (Proxy-Socket, CA, Shim, `/proc`, `/sys`, `/dev`, `/run/humanitl`). Eine kaputte mitgelieferte Vorlage ist `AGENT_003`. |
| `default_rules` | Die mitgelieferten Regeln des Adapters als `rules.yaml`-Text. Der Daemon parst ihn; das Sandbox-Crate hängt nicht vom Regel-Crate ab. |
| `llm_passthrough` | Die Regel für `llm.endpoint`, oder `None` ohne brauchbaren Endpunkt. Kopiere `OpenCodeAdapter::llm_passthrough`: enge Pfade, genauer Host, `allow_private`, `passthrough_llm`. |
| `preflight` | Prüfungen auf dem Rechner vor dem Start. Nur das Dateisystem, nie das Netzwerk. Eine Diagnose mit der Schwere `Blocking` verhindert den Start. |
| `program` | Der Programmname ohne Pfad. `is_agent_command` vergleicht den Dateinamen eines Befehls damit, sodass `opencode`, `/usr/local/bin/opencode` und `./opencode` derselbe Agent sind. |
| `is_fullscreen_tui` | `true` lässt `--ask terminal` mit `CLI_002` verweigern. |

`AgentContext` trägt die Session, den Projektpfad auf dem Rechner und in der Sandbox (`/work`),
`llm` (Endpunkt und Modelle), `agent_command_override`, den Proxy-Port, den CA-Pfad in der
Sandbox, die Sprache, `hold`, den Schalter für das Briefing, den `PATH` des Rechners für den
Preflight und das Home des Agenten.

Drei Versprechen prüft kein Compiler, und jeder Adapter hält sie:

1. **Keine neue Tür.** Ein Adapter fügt keine Bridge und keine seccomp-Familie hinzu. Beides
   kommt aus dem Sandbox-Profil, und das kennt genau eine Bridge.
2. **Nichts unter `/work`.** `SandboxFile::is_outside_work` und `files_inside_work` prüfen das.
3. **Kein Netzwerk im Preflight.** Ob das Modell antwortet, ist die Aufgabe der Probe des
   Endpunkts (`humanitl llm test`), nicht des Adapters.

Ein minimaler Adapter für einen zeilenorientierten Agenten `echo-agent`, der eine
Konfigurationsdatei liest:

```rust
use std::ffi::OsString;

use humanitl_config::LlmConfig;
use humanitl_core::Diagnostic;
use humanitl_core::rule::Rule;

use crate::agent::{AgentAdapter, AgentContext, SandboxFile};

/// An adapter for `echo-agent`.
pub struct EchoAdapter;

impl AgentAdapter for EchoAdapter {
    fn id(&self) -> &'static str {
        "echo"
    }

    fn program(&self) -> &'static str {
        "echo-agent"
    }

    fn command(&self, ctx: &AgentContext) -> Vec<OsString> {
        ctx.agent_command_override
            .clone()
            .filter(|command| !command.is_empty())
            .unwrap_or_else(|| vec![OsString::from("echo-agent")])
    }

    fn env(&self, _ctx: &AgentContext) -> Vec<(String, String)> {
        vec![("ECHO_AGENT_CONFIG".to_owned(), "/etc/humanitl/echo/config.toml".to_owned())]
    }

    fn files(&self, _ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic> {
        Ok(vec![SandboxFile::read_only(
            "/etc/humanitl/echo/config.toml",
            b"telemetry = false\n".to_vec(),
        )])
    }

    fn default_rules(&self) -> &'static str {
        "version: 1\nrules: []\n"
    }

    fn llm_passthrough(&self, _llm: &LlmConfig) -> Option<Rule> {
        // Without a passthrough every request to the model is held.
        None
    }

    fn preflight(&self, _ctx: &AgentContext) -> Vec<Diagnostic> {
        Vec::new()
    }

    fn is_fullscreen_tui(&self) -> bool {
        false
    }
}
```

Registriere ihn in `AdapterRegistry::builtin()`, wähle ihn mit `agent.adapter = "echo"` und teste
ihn so, wie `daemon/crates/sandbox/tests/opencode_adapter.rs`
OpenCode testet: Jede Methode ist eine reine Funktion von `AgentContext`, also braucht ein Test
keine Sandbox.

### Ein Shell-Skript als Agent

Um ein eigenes Programm laufen zu lassen, brauchst du keinen Adapter. Alles nach `--` läuft in der
Sandbox anstelle des konfigurierten Agenten:

```sh
cat > agent.sh <<'EOF'
#!/bin/sh
# The smallest possible agent: one request, and print what came back.
# HTTPS_PROXY and the CA variables are set by the sandbox profile.
set -u
curl -sS https://example.com/ | head -n 5
EOF

humanitl run --ask terminal -- /bin/sh /work/agent.sh
```

Das Projektverzeichnis ist in der Sandbox `/work`, also ist das Skript im aktuellen Verzeichnis
`/work/agent.sh`. Die Anfrage an `example.com` wird angehalten; mit `--ask terminal` erscheint die
Entscheidungsbox im selben Terminal, weil das Skript kein Vollbildprogramm ist. Sende sie, und das
Skript gibt den Anfang der Seite aus; blockiere sie, und es gibt den Block-Body aus:

```
Blocked by Humanitl.
reason: user
flow: <flow id>
host: example.com
```

Unter `humanitl run` bekommt ein solcher Befehl denselben Beitrag des Adapters wie der Agent
selbst (HUM-280): die Dateien des konfigurierten Adapters, seine Variablen und, wenn ein Schlüssel
gespeichert ist, die Schlüsseldatei. Der konfigurierte Agent wird daneben eingebunden und steht an
erster Stelle im `PATH`, sodass `humanitl run -- bash` eine Shell ist, in der `opencode` (oder
`pi`) mit dem konfigurierten Modell und Schlüssel startet. Der Befehl, den du nennst, wird selbst
nie eingebunden; er muss aus den Mounts der Sandbox startbar sein. Gibt der Schlüsselspeicher den
Schlüssel nicht heraus, startet ein eigener Befehl trotzdem, ohne den Schlüssel, und der Start
sagt das mit `LLM_009` als Warnung. Nur das Daemon-lose `humanitl sandbox run` behält die ältere
Regel: Dort bekommt ein Befehl, der nicht der Agent ist, nichts vom Adapter. Um dein Programm zur
Vorgabe zu machen, setze `agent.command` in `config.toml`, zum Beispiel
`agent.command = ["/usr/local/bin/my-agent"]`.

## Bekannte Hosts, die nach Hause telefonieren

Was ein Agent von sich aus kontaktiert, ohne dass der Mensch darum gebeten hat. Die Liste stammt
aus den mitgelieferten Regeln in `rules/default.yaml`, die jede Session lädt, egal mit welchem
Adapter, und aus den Variablen des Adapters. Jede Regel trägt einen Hinweis
mit der Begründung; `humanitl rules list --all` und die Gruppe „Mitgelieferte Regeln" im
Bildschirm Regeln zeigen sie.

### OpenCode

| Host | Was | Regel | Auch abgeschaltet durch |
|---|---|---|---|
| `models.opencode.ai` | Modellkatalog, von OpenCode 1.18.25 benutzt | block (`…0009`) | `OPENCODE_MODELS_PATH`, `OPENCODE_DISABLE_MODELS_FETCH` |
| `models.dev` | Modellkatalog, den die Dokumentation von OpenCode nennt | block (`…0001`) | dieselben |
| `api.github.com`, Pfad `/repos/anomalyco/opencode/releases/**` | Prüfung auf Updates | block (`…0002`) | `OPENCODE_DISABLE_AUTOUPDATE`, `"autoupdate": false` |
| `opncd.ai` | Hochladen geteilter Sessions (`POST /api/share`) | block (`…0005`) | `OPENCODE_DISABLE_SHARE`, `"share": "disabled"` |
| `opencode.ai`, Pfad `/share/**` | Lesen von Teilen-Links | block (`…000a`) | dieselben |
| `**.exa.ai` | gehostete Websuche | block (`…0006`) | `OPENCODE_ENABLE_EXA=false`, `"websearch": "deny"` |
| `**.parallel.ai` | gehostete Websuche | block (`…0007`) | `OPENCODE_ENABLE_PARALLEL=false` |
| `registry.npmjs.org`, `GET` und `HEAD` | Anbieterpakete wie `@ai-sdk/openai-compatible`, bei der ersten Verwendung installiert | ask (`…0008`): angehalten und gezeigt, damit du erfährst, was es ist | nichts; das Paket wird gebraucht |

Das Herunterladen von Sprachservern ist mit `OPENCODE_DISABLE_LSP_DOWNLOAD` abgeschaltet; ihre
Hosts sind nicht erfasst, also wird einer, falls er je auftaucht, angehalten wie jede andere
Anfrage.

### pi

| Host | Was | Regel | Auch abgeschaltet durch |
|---|---|---|---|
| `pi.dev`, Pfad `/api/**` | Versionsprüfung, Modellkatalog, Installer | block (`…0101`) | `PI_OFFLINE`, `PI_SKIP_VERSION_CHECK` |
| `radius.pi.dev` | Hochladen von Fehlerberichten, Teilen von Sessions über Radius | block (`…0102`) | nichts; nur bei einem solchen Befehl |
| `github.com`, Pfad `/sharkdp/fd/releases/**` | Herunterladen von `fd` | block (`…0103`) | `PI_OFFLINE` |
| `github.com`, Pfad `/BurntSushi/ripgrep/releases/**` | Herunterladen von `rg` | block (`…0104`) | `PI_OFFLINE` |

### Andere Agenten

Humanitl 0.1 hat keinen Adapter für Aider, Codex oder Claude Code und erfasst keine Hosts für sie.
Zwei mitgelieferte Regeln sind eher Schlösser für solche Agenten und für spätere Versionen von
OpenCode als Beobachtungen:

| Host | Warum | Regel |
|---|---|---|
| `**.posthog.com` | Produktanalyse, die mehrere Agenten-CLIs nutzen; kommt im Binary von OpenCode 1.18.25 nicht vor | block (`…0003`) |
| `**.sentry.io` | Absturzberichte mit Stacktraces und Dateipfaden; OpenCode 1.18.25 ruft keine solche Adresse auf | block (`…0004`) |

Jeder andere Host, den ein Agent kontaktiert, wird angehalten, weil eine Anfrage, auf die keine
Regel passt, standardmäßig auf `ask` fällt. So zeigt sich ein neuer Host, der nach Hause
telefoniert: in der Warteschlange, mit Namen, bevor irgendetwas den Rechner verlässt.

Eine mitgelieferte Regel lässt sich weder bearbeiten noch löschen, aber du kannst sie mit
`humanitl rules disable <id>` abschalten oder mit einer eigenen Regel überstimmen, die zuerst
ausgewertet wird.
