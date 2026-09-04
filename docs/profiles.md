# Profile

Ein Profil bündelt, was eine Sitzung ausmacht: die Konfigurationswerte, die für
sie gelten, und die Regeln, die zu ihr gehören. Es ist eine Datei, und es gibt
sie an drei Orten — mitgeliefert im Programm, global beim Nutzer und im Projekt.
`humanitl run --profile llm-only` und die Oberfläche wählen dieselben Profile
über denselben Lader (ADR-011, `BACKLOG.md` Abschnitt 6).

Diese Seite beschreibt das Format, die Reihenfolge der Ebenen und die Grenzen,
die für ein Profil aus einem geklonten Repository gelten. Die einzelnen
Konfigurationsschlüssel stehen in `docs/CONFIG.md`, die Regelsprache in
`backlog/CONVENTIONS.md` 3.3.

## Format

```toml
name = "llm-only"
description = "Pure inference. Only the configured LLM endpoint is reachable."

[config.hold]                 # Konfigurationswerte, gruppenweise
ask_mode = "none"
timeout_secs = 1

[config.sandbox]
profile = "default"           # die Datei profiles/sandbox/default.toml
work_mode = "rw"

[rules]
files = ["team.yaml"]         # relativ zu dieser Profildatei
inline = [                    # Regeln im Schema von rules.yaml
  { action = "block", match = { host = "**" }, expires = "never", note = "..." },
]
```

Auf der obersten Ebene gibt es genau vier Schlüssel: `name`, `description`,
`[config]` und `[rules]`. Jeder andere Block dort ist `CONFIG_002`. Der häufigste
Fall ist eine Gruppe, die eine Ebene zu hoch steht: `[hold]` statt
`[config.hold]`. Die Meldung sagt das, denn ein Profil, das ohne Wirkung und
ohne Meldung bliebe, wäre schlimmer als eines, das nicht lädt.

Alle Felder sind wahlfrei. Was ein Profil nicht nennt, erbt es aus der Ebene
darunter, und zwar feldweise: `[config.hold]` mit nur `timeout_secs` lässt
`ask_mode` in Ruhe. Listen dagegen ersetzen, sie hängen nicht an — `rules.files`
und `llm.passthrough_paths` aus einer höheren Ebene meinen genau die Liste, die
dort steht.

`name` muss zum Dateinamen passen; eine Profildatei `work.toml`, die sich
`other` nennt, ist `CONFIG_003`. Geprüft wird der Name, der am Ende gilt, und
zwar auf `^[a-z0-9-]{1,32}$` — ob er im `name`-Schlüssel steht oder aus dem
Dateistamm kommt, macht keinen Unterschied: `Work.Profile.toml` ohne `name` ist
genauso `CONFIG_003` wie ein getipptes `Work.Profile`. Ein Name ist nie ein
Pfad: `--profile ../../etc/passwd` ist keine Frage der Formulierung, sondern
`CONFIG_003`.

## Die mitgelieferten Profile

| Profil | Wofür |
|---|---|
| `default` | Alles, was keine Regel entscheidet, wird angehalten und einem Menschen gezeigt. |
| `llm-only` | Reine Inferenz: nur der eingerichtete LLM-Endpunkt ist erreichbar, alles andere wird geblockt, ohne zu fragen. Ohne Oberfläche. |

Beide liegen zweimal vor: als Datei unter `profiles/` im Auslieferungsumfang und
eingebettet im Binary. Fehlen die Dateien, startet der Daemon trotzdem. Legt
jemand unter `$XDG_CONFIG_HOME/humanitl/profiles/` eine eigene Fassung mit
demselben Namen, gewinnt diese Datei — und das Laden legt einen Hinweis dazu
(`CONFIG_008`), sobald sie sich vom mitgelieferten Text unterscheidet. Eine
wortgleiche Kopie ändert nichts und wird nicht gemeldet.

**`default` setzt mit Absicht keinen Wert.** Es liegt in der Reihenfolge über
`config.toml`; ein Wert darin — auch einer, der nur den Vorgabewert wiederholt —
machte die Datei des Nutzers für diesen Schlüssel wirkungslos, ohne dass er es
sähe. Der gängige Fall steht deshalb als Kommentar in `profiles/default.toml`:
Wer ihn ändern will, kopiert die Datei nach
`$XDG_CONFIG_HOME/humanitl/profiles/default.toml` und nimmt die Rauten weg.

`llm-only` setzt sehr wohl Werte. Es zu wählen ist eine ausdrückliche
Entscheidung, und sie soll `config.toml` überstimmen: `ask_mode = "none"` heißt,
dass eine Anfrage, die keine Regel entscheidet, sofort geblockt statt angehalten
wird. `timeout_secs` ist dann bedeutungslos und steht auf 1, damit eine
Fehlkonfiguration nicht fünf Minuten lang hängt. Die Durchreichregel des
Agent-Adapters wird vor der Blockregel des Profils ausgewertet und trifft
zuerst, wo immer sie in der Liste steht (`backlog/CONVENTIONS.md` 4.5); alles
andere fällt auf `block` mit dem Grund „llm-only profile".

## Reihenfolge der Ebenen

Von unten nach oben; die obere gewinnt. Jedes Feld merkt sich, aus welcher Ebene
sein Wert stammt (`Origin`), und Oberfläche wie `humanitl config get` zeigen es
an. Ohne diese Auskunft ließe sich eine überraschende Sandbox nicht erklären.

| Ebene | Quelle | `Origin` |
|---|---|---|
| 1 | eingebaute Vorgabewerte | `default` |
| 2 | `$XDG_CONFIG_HOME/humanitl/config.toml` | `config.toml` |
| 3 | Profil `default`: die Datei, sonst die eingebettete Fassung | `profile default` bzw. `profile builtin default` |
| 4 | das gewählte Profil, falls es nicht `default` ist | `profile <name>` bzw. `profile builtin <name>` |
| 5 | `<projekt>/.humanitl/profile.toml` | `project profile <pfad>` |
| 6 | Umgebungsvariablen `HUMANITL_*` | `env <name>` |
| 7 | Argumente der Kommandozeile | `command line` |

Ebene 3 gilt immer, auch wenn ein anderes Profil gewählt wurde; das gewählte
liegt darüber. Welche Profile mitgeredet haben, steht als Kette in
`Resolved::profile_chain()` und unter `humanitl run -v`.

Welches Profil Ebene 4 besetzt, entscheiden in dieser Reihenfolge: der Name auf
der Kommandozeile, sonst der `name` im Projekt-Profil, sonst `default`. Ein Name,
zu dem es weder eine Datei noch ein mitgeliefertes Profil gibt, ist `CONFIG_001`
mit der Liste der bekannten Profile. Eine Datei, die es gibt, sich aber nicht
lesen lässt, gilt dabei als vorhanden: Ein kaputtes Profil verschwindet nicht
stillschweigend, es hält den Start an.

**Welches Verzeichnis das Projekt ist, entscheidet `sandbox.work_dir`, sonst das
aktuelle Verzeichnis.** Wer mit `--work` aus einem anderen Verzeichnis heraus
arbeitet, bekommt das Profil des Projekts, an dem er arbeitet, und nicht das des
Verzeichnisses, in dem seine Shell steht. Der Schlüssel ist auf der
Projekt-Ebene gesperrt; das Projekt-Profil kann also nicht bestimmen, wo nach
ihm gesucht wird, und die Auflösung bleibt zirkelfrei.

**Das Projekt darf mit `name` nur ein mitgeliefertes Profil wählen.** Dürfte es
ein beliebiges Profil des Nutzers als Ebene 4 einsetzen, käme ein geklontes
Repository über diesen Umweg an jeden Schlüssel, den ihm die Projekt-Ebene
verwehrt — `sandbox.profile` und `agent.command` eingeschlossen, also die
Einhängefläche der Sandbox und den Prozess darin. Ein anderer Wunsch wird
übergangen und mit `CONFIG_009` gemeldet; wer sein eigenes Profil meint, nennt
es mit `--profile`. Wer auf der Kommandozeile ein Profil nennt, überstimmt den
Wunsch des Projekts ohnehin.

## Was ein Projekt-Profil nicht darf

`<projekt>/.humanitl/profile.toml` liegt im geklonten Repository. Wer ein
Repository klont, führt dessen Profil aus; die Datei ist damit
Angreifer-beeinflusst und steht unter vier Grenzen.

1. **Kein anderes Profil.** `name` wählt nur unter den mitgelieferten Profilen;
   siehe „Reihenfolge der Ebenen". Ohne diese Grenze wären die drei folgenden
   umsonst, denn ein Repository hätte sich das passende Profil des Nutzers
   ausgesucht und dessen Werte bekommen.
2. **Nur erlaubte Schlüssel.** Jedes Feld im Schema trägt
   `x-project-scope: allowed | denied` (`docs/CONFIG.md`, Spalte „Projekt").
   Ein gesperrter Schlüssel aus dieser Ebene ist `CONFIG_003`, auch unter einem
   alten Namen. Gesperrt sind unter anderem `llm.*`, `sandbox.*` (samt
   `sandbox.profile` und `sandbox.env`), `agent.adapter`, `agent.command`,
   `hold.ask_mode`, `findings.enabled`, `pseudonyms.*`, `resolver.*`,
   `experimental.*` und `recorder.retention_days`.
3. **Keine Einhängungen.** Einhängungen stehen im Sandbox-Profil unter
   `profiles/sandbox/`, nicht in der Konfiguration. Ein Projekt-Profil, das
   `sandbox.mounts.extra_ro` oder `sandbox.mounts.extra_rw` nennt, wird mit
   `CONFIG_003` und dem Satz abgelehnt, dass nur globale Profile Host-Pfade in
   die Sandbox holen dürfen — nicht mit „unbekannter Schlüssel", denn die
   Absicht ist erkennbar und verdient eine klare Antwort.
4. **Keine Regeln.** Ein `[rules]`-Block im Projekt-Profil ist `CONFIG_003`. Ein
   Repository entscheidet nicht, was die Sandbox verlassen darf. Regeln kommen
   aus `rules.yaml`, aus einem globalen Profil oder vom Agent-Adapter.

Gehört die Datei einem anderen Konto als dem, das Humanitl startet, ist das eine
Warnung (`CONFIG_007`) und keine Ablehnung: Die Grenzen halten die Datei
ohnehin, und ein Start, der daran scheiterte, wäre auf einem geteilten Rechner
nicht zu gebrauchen.

## Regeln aus einem Profil

`[rules].files` nennt Regeldateien im Format von `rules.yaml`. Relative Pfade
werden gegen das Verzeichnis der Profildatei aufgelöst, nie gegen das
Arbeitsverzeichnis: Ein Profil soll dieselben Dateien meinen, egal von wo aus
Humanitl startet. Ein eingebettetes Profil hat kein Verzeichnis und kann deshalb
nur Regeln mitbringen, die in ihm selbst stehen.

`[rules].inline` nennt Regeln unmittelbar, im selben Schema wie `rules.yaml`.
`Profile::rules_document()` gibt sie als Dokument zurück, das
`humanitl_rules::parse_rules` liest.

Die Reihenfolge, in der ein Regelsatz **entsteht**, ist nicht die Reihenfolge,
in der er **ausgewertet** wird. Ausgewertet wird in vier Rängen
(`backlog/CONVENTIONS.md` 4.5): zuerst die Durchreichregel des Agent-Adapters,
dann die Sitzungsregeln, dann die dauerhaften Regeln des Nutzers aus
`rules.yaml`, zuletzt die mitgelieferten — die Regeln aus `rules/default.yaml`
des Adapters ebenso wie die Dateien und Regeln der Profile. Innerhalb eines
Rangs gewinnt die erste passende Regel.

Ein Profil kann die Durchreiche also nicht überdecken, auch nicht mit
`block host "**"`, und der Nutzer kann eine mitgelieferte Regel überstimmen,
ohne sie zu löschen.

Den ersten Rang trägt allein die Durchreiche, die der Agent-Adapter baut. Ein
Profil kann ihn sich nicht selbst geben: Eine Inline-Regel mit `bundled = true`
verliert den Vermerk beim Lesen und bekommt eine Warnung (`RULES_010`). Der
Vermerk sagt, woher eine Regel kommt, und das entscheidet der Lader, nicht die
Datei — sonst schriebe sich ein Profil eine ungehaltene Durchreiche an jeder
Block-Regel seines Nutzers vorbei (`backlog/CONVENTIONS.md` 4.5).

## Auf der Kommandozeile

```sh
humanitl config schema --profiles            # welche Profile es gibt
humanitl config get hold.ask_mode --profile llm-only
humanitl run --profile llm-only              # startet die Sitzung (HUM-067)
```

`--profile NAME` benennt nach `backlog/CONVENTIONS.md` 3.8 zwei Dinge, und das
**Unterkommando** unterscheidet sie:

| Aufruf | `--profile` meint |
|---|---|
| `humanitl sandbox run\|argv\|check` | das bwrap-Profil unter `profiles/sandbox/` |
| alles andere (`run`, `config`, `rules`, `flows`, `daemon`) | das Profil der Sitzung |

Nicht die Dateien auf der Platte entscheiden das. Eine Bedeutung, die daran
hinge, kippte lautlos, sobald jemand ein gleichnamiges Sitzungsprofil anlegt —
und eine Einhängung, die dabei verschwindet, sähe niemand. Wer unter `sandbox`
das Sitzungsprofil meint, setzt seine Werte über die Konfigurations-Flags; wer
unter `run` das bwrap-Profil meint, schreibt `--sandbox-profile`.

Als Sitzungsprofil ist der Name streng: Gibt es keines, ist das `CONFIG_001` und
kein stiller Start mit dem Vorgabeprofil. Eine Datei, die sich nicht lesen lässt,
zählt dabei als vorhanden; `humanitl config schema --profiles` schreibt
`(does not load)` hinter ihre Herkunft, damit die Liste nicht zu einem Aufruf
einlädt, der scheitern muss.

## Ein eigenes Profil anlegen

```sh
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/humanitl/profiles"
cat > "${XDG_CONFIG_HOME:-$HOME/.config}/humanitl/profiles/work.toml" <<'EOF'
name = "work"
description = "Der Arbeitsrechner: kürzere Frist, deutsche Oberfläche."

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

Die Datei `work-rules.yaml` wird neben `work.toml` gesucht, also unter
`$XDG_CONFIG_HOME/humanitl/profiles/`.
