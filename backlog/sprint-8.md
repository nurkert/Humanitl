# Sprint 8 · Startfenster, Terminal-Client und Sitzungen, die ihr Terminal überleben

Ziel des Sprints: Humanitl startet so, wie der Eigentümer es am 2026-11-17 beschrieben hat. Die App öffnet nie eine Sitzung ohne Projekt und zeigt sonst ein Startfenster (laufend, zuletzt, neu, aufräumen, Einstellungen als Reiter); `humanitl app [.]` öffnet es. `humanitl go claude` startet den Agenten im aktuellen Ordner, beim ersten Mal mit einem einzigen Bildschirm voller Vorgaben, danach ohne. Alle Einstellungen gibt es auch als TUI auf ratatui, aus demselben Katalog des Daemons. In der Sitzung wechselt eine Präfixtaste die Ansicht (Einstellungen, Interceptor) oder hängt ab, und Schließen hängt ab statt zu beenden. Dazu, unabhängig, ein Rückfall für den macOS-Runner des Releases. Entscheidung in [ADR-0024](../docs/adr/0024-terminal-client-and-start-window.md) (Proposed), Eintrag in `BACKLOG.md` Abschnitt 9, Punkt 24.

Die offenen Fragen des ADR (dort nummeriert, hier als „Frage N" zitiert) hat der Eigentümer am 2026-11-17 beantwortet; die Fragen 3 und 4 hat der Hauptstrang nach dem Vorschlag des ADR entschieden, der Eigentümer hat nicht widersprochen. Die Antworten stehen im Abschnitt „Decision" des ADR mit Datum und sind in die Issues eingearbeitet. Offen ist nur Frage 8 (Umfang des Aufräumens): HUM-533 baut die Vorgabe des ADR, die der Eigentümer noch bestätigt. Kein Issue entscheidet eine offene Frage still.

Drei Grenzen gelten für jedes Issue dieses Sprints und sind Akzeptanzkriterium, auch wo das Issue sie nicht eigens nennt:

1. Die drei Garantien (`README.md`, `docs/SECURITY.md`, `tests/escape/`) und der Filter für Terminal-Ausgabe (`humanitl_core::TerminalFilter`, Politik `FullScreen`) bleiben, wie sie sind. „Sieht aus wie ohne Isolationsschicht" heißt: so weit, wie dieser Filter es zulässt. Ein Projekt senkt die Isolationsstufe nie (CONVENTIONS 4.11).
2. Jede Fähigkeit ist zuerst ein RPC (ADR-018). App, CLI und TUI entscheiden nichts selbst und halten keine eigene Liste von Einstellungen, Optionen oder Kandidaten zum Löschen. Ein Wert, den die Firmenvorgabe sperrt, steht in jedem Client nur lesbar da, mit der Vorgabe als Herkunft.
3. Gelöscht wird nur, was der Daemon vorher einzeln aufgelistet und der Mensch bestätigt hat. Der Projektordner und die Audit-Kette sind nie Teil davon.

Leitlinie: ratatui und crossterm sind Abhängigkeiten nur von `daemon/bin/humanitl`, eingetragen zentral in `daemon/Cargo.toml` (nicht von einem Subagenten, siehe `CLAUDE.md`). Daemon und Shim linken sie nicht. Kein neuer Port. Jeder Commit, der eine Aussage aus `docs/SECURITY.md` berührt, ändert `docs/SECURITY.md` und `docs/THREAT-MODEL.md` im selben Commit.

Diagnose-Codes, hier nur geplant (registriert vom genannten Issue in `codes.rs` und `docs/DIAGNOSTICS.md`). Am 2026-11-17 auf `origin/main` (`e075aca6`, nach HUM-518 mit `AGENT_008` bis `AGENT_018`) und im Arbeitsbaum von HUM-529 frei; das registrierende Issue prüft erneut:

| Code | Bedeutung | Issue |
|---|---|---|
| `SANDBOX_053` | Start ohne Projektordner; nennt, wie man einen wählt (Startfenster, `humanitl go` im Ordner, `--work DIR`) | HUM-532 |
| `SANDBOX_054` | Ein Eintrag des Aufräumplans ist in Gebrauch (laufende Sitzung, wartende Änderungen ohne ausdrückliche Wahl) oder der Plan ist veraltet | HUM-533 |
| `CONFIG_024` | Die Projektwahl enthält einen Schlüssel, den sie nicht tragen darf, oder einen Wert unter `sandbox.min_isolation` | HUM-532 |
| `CLI_007` | Die App ist nicht installiert oder startet nicht | HUM-539 |
| `TERM_003` | Terminal zu klein oder ohne nötige Fähigkeit für einen Bildschirm der TUI; nennt Mindestgröße und den Weg ohne TUI | HUM-540 |

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-530 | ADR-0024: Frage 8 bestätigen lassen, Entscheidung annehmen | S | — |
| HUM-531 | Release: Rückfall auf einen anderen macOS-Runner, klare Anleitung für „Re-run failed jobs" | S | — |
| HUM-532 | Daemon: Projektregister mit Projektwahl, RPC `Projects` (`list`, `get`, `save_choice`) | L | HUM-530 |
| HUM-533 | Aufräumen und Vergessen: Plan und Ausführung über `Projects`, CLI `humanitl projects` | M | HUM-532 |
| HUM-534 | Einstellungskatalog aus dem Daemon: Kategorien, Texte, Herkunft, Sperren, Projektbezug, `ConfigChanged` | L | HUM-532 |
| HUM-535 | App: eine Einstellungskomponente für Reiter und Sheet | M | HUM-534 |
| HUM-536 | Startvorschlag für ein neues Projekt als RPC (`Projects.proposal`) | M | HUM-532 |
| HUM-537 | App: Startfenster mit Laufend, Zuletzt, Neu, Aufräumen und Einstellungen | L | HUM-533, HUM-535, HUM-536 |
| HUM-538 | App: Erststart-Bildschirm unter „Neu" und für ein unbekanntes Projekt | M | HUM-537 |
| HUM-539 | `humanitl app [PATH]`: App mit vorgewähltem Projekt starten oder die laufende erreichen | M | HUM-537 |
| HUM-540 | TUI-Grundlage: ratatui im CLI, Terminal auf jedem Weg zurück | M | HUM-530 |
| HUM-541 | Einstellungs-TUI `humanitl settings` | L | HUM-534, HUM-540 |
| HUM-542 | `humanitl go [AGENT]`: Agent als Wort, `--dir`, `--disposable`, `--rw`, `--ro`, `--kernel`, Projektwahl | M | HUM-532 |
| HUM-543 | Erststart-Bildschirm in `humanitl go` | M | HUM-536, HUM-540, HUM-542 |
| HUM-544 | Präfixtaste `ctrl-b` mit fester Tabelle; `Ctrl+,` direkt nur mit kitty-Tastaturprotokoll | M | HUM-530 |
| HUM-545 | Ansichtswechsel in der Sitzung: Einstellungs-TUI über dem Agenten und zurück | M | HUM-541, HUM-544 |
| HUM-546 | Interceptor-Ansicht im Terminal, Glocke bei neuem Halt | M | HUM-545 |
| HUM-547 | `humanitl go`: Schließen hängt ab statt zu beenden, ein zweites `go` hängt sich an | M | HUM-542 |
| HUM-548 | Doku: `cli.md`, `CONFIG.md`, `UX.md`, Anleitung „Starten"; Nachtrag zu ADR-0024 | S | HUM-531 bis HUM-547 |
| HUM-549 | Kandidat, nicht in diesem Sprint: mehrere Sitzungen je Daemon | L | HUM-537, HUM-547 |

---

## HUM-530 · ADR-0024: Frage 8 bestätigen lassen, Entscheidung annehmen

### Kontext
ADR-0024 hielt neun offene Fragen fest. Am 2026-11-17 hat der Eigentümer
die Fragen 1, 2, 5, 6, 7 und 9 beantwortet; die Fragen 3 und 4 hat der
Hauptstrang nach dem Vorschlag des ADR entschieden, ohne Widerspruch des
Eigentümers. Die Antworten stehen im Abschnitt „Decision" mit Datum. Offen
ist Frage 8, der Umfang des Aufräumens; dafür gilt eine Vorgabe, die der
Eigentümer noch bestätigen soll. Das ADR bleibt `Proposed`, bis dieses
Issue es schließt.

### Spezifikation
1. Frage 8 mit der Vorgabe des ADR vorlegen: Flüsse und Bodies des
   Projekts, wartende Änderungen einer Wegwerf-Sitzung (nur auf
   ausdrückliche Wahl, mit der Warnung, dass nicht übernommene Änderungen
   endgültig verloren sind), gespeicherte Projektwahl samt Eintrag im
   Register; Depot-Versionen nicht (Sache des Depots, `agents update` und
   `agents remove`); Audit-Kette nie.
2. Antwort in ADR-0024 eintragen (in „Decision" mit Datum, Abschnitt
   „Open questions for the owner" danach leer bis auf einen Satz) und, wo
   sie HUM-533 ändert, das Issue in diesem Sprint-File nachziehen, im selben
   Commit.
3. Status des ADR auf `Accepted`, Indexzeile in `docs/adr/README.md`
   nachziehen.
4. Bleibt der Eigentümer ohne Antwort, wird das ADR mit der Vorgabe
   angenommen und Frage 8 im ADR als „Vorgabe, vom Eigentümer noch zu
   bestätigen" stehen gelassen; HUM-533 baut die Vorgabe.

### Tests
- `docs/adr/check.sh` und `scripts/ci/lint-docs.sh` grün.

### Fallstricke
- Keine Antwort erfinden; eine Vorgabe bleibt als solche gekennzeichnet.

### Akzeptanzkriterien
1. ADR-0024 ist `Accepted`; jede Frage ist beantwortet oder als Vorgabe
   gekennzeichnet.
2. Die Issues dieses Sprints widersprechen dem ADR an keiner Stelle.

---

## HUM-531 · Release: Rückfall auf einen anderen macOS-Runner, klare Anleitung für „Re-run failed jobs"

### Kontext
Am 2026-11-17 brauchte das Release v0.4.0 vier neue Anläufe, weil GitHub
keinen macOS-arm64-Runner zuteilte: `build-macos` und `check-macos` endeten
als abgebrochen mit „The job was not acquired by Runner of type hosted even
after multiple attempts". Beide Jobs laufen auf `macos-15`
(`.github/workflows/release.yml`, `runs-on` von `build-macos` und
`check-macos`). Ein Abbruch durch fehlende Runner ist kein Fehler des
Codes, hält aber das Release auf, und `tools/release.sh` sagt nur
„Re-run failed jobs".

Stand der Runner laut GitHub (abgerufen 2026-11-17):
- docs.github.com/en/actions/reference/runners/github-hosted-runners:
  Standard-Runner für öffentliche Repositories, arm64 mit 3 CPU (M1) und
  7 GB unter `macos-latest`, `macos-15`, `macos-26`; Intel x64 mit 4 CPU und
  14 GB unter `macos-15-intel`, `macos-26-intel`. Bei hoher Nachfrage
  starten macOS-Jobs später.
- github.com/actions/runner-images: Alle Abbilder macOS 14 sind seit
  2026-11-02 aus GitHub Actions entfernt; `macos-14` fällt als Rückfall aus.
  `macos-latest` zeigt dort auf macOS 26 arm64.

`macos-26` teilt vermutlich den arm64-Pool mit `macos-15`; ein Mangel dort
trifft dann beide. `macos-15-intel` liegt auf anderer Hardware. Die
Bauskripte bauen ohnehin für beide Architekturen
(`packaging/macos/build-binaries.sh`: `aarch64-apple-darwin` und
`x86_64-apple-darwin`, danach `lipo`), und `check-macos` prüft mit `lipo
-archs` beide Scheiben.

### Spezifikation
1. **Rückfall-Jobs** in `release.yml`: `build-macos-fallback` auf
   `macos-15-intel`, gleiche Schritte wie `build-macos` (gemeinsam über eine
   lokale Composite-Action oder ein Skript unter `packaging/macos/`, keine
   kopierten Schritte). Er läuft nur, wenn der Hauptpfad **abgebrochen
   wurde, bevor ihm ein Runner zugeteilt war**, nie bei einem gescheiterten
   Schritt. `needs.build-macos.result` allein unterscheidet das nicht
   (`failure` und `cancelled` sagen nichts über den Runner). Deshalb
   entscheidet ein Job `classify-macos` auf `ubuntu-24.04` mit
   `if: always() && needs.guard.result == 'success'` und `needs:
   [guard, build-macos]` über die Actions-API (`GET
   /repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt}/jobs`, wie
   `tools/ci-gate.sh` die API liest): Ausgabe `fallback=true` genau dann,
   wenn der Job `build-macos` `conclusion = cancelled` hat, `runner_name`
   leer ist, kein Schritt begonnen hat (`steps` leer oder alle ohne
   `started_at`) **und** der Check-Run des Jobs die Annotation „The job was
   not acquired by Runner of type hosted even after multiple attempts"
   trägt (`GET /repos/{owner}/{repo}/check-runs/{check_run_id}/annotations`;
   genau dieser Text stand am 2026-11-17 am abgebrochenen `build-macos` von
   v0.4.0). Ohne die Annotation ist es ein Abbruch von Hand in der
   Warteschlange, und der bleibt ohne Rückfall. `build-macos-fallback` hat
   `if: always() && needs.classify-macos.outputs.fallback == 'true'`. Die
   Einordnung steht in einem Skript (`tools/ci/classify-runner-loss.sh`),
   das eine Job-Liste als JSON liest, damit sie ohne GitHub testbar ist.
   Ebenso `check-macos-fallback` für einen `check-macos` ohne Runner
   (gleiche Einordnung), der das Artefakt des Jobs prüft, der gebaut hat. `release` verlangt: genau einer der beiden Pfade (Bau und
   Prüfung) grün, alle anderen Jobs grün; die Bedingung steht ausdrücklich
   im `if` von `release` mit `always()`, damit ein abgebrochener Hauptpfad
   den Job nicht überspringt.
2. **Ein rot gewordener Hauptpfad löst den Rückfall nicht aus**, wenn er an
   einem Schritt scheiterte (Codefehler, `conclusion = failure` mit Runner
   und begonnenen Schritten), und auch nicht, wenn er nach dem Start von
   Hand abgebrochen wurde (`cancelled` mit Runner): Beides bleibt rot.
3. **Prüfen, dass der Rückfall wirklich baut:** ein Probelauf
   (`release-dry/0.Y.Z`, HUM-363) mit einem Schalter, der den Hauptpfad
   überspringt, bis `check-macos-fallback` grün ist: universelle Programme,
   Flutter-App universell, `.dmg`, Signatur ad hoc, Seatbelt-Escape-Tests
   auf x86_64. Ergebnis und Lauf-Link im Commit-Body.
4. **Gleiche Artefaktnamen** für beide Pfade; `release` lädt das eine, das
   existiert. Ein Hauptpfad, der vor dem Abbruch schon hochgeladen hätte,
   kann es nicht, weil das Hochladen der letzte Schritt ist; der Rückfall
   lädt mit `overwrite: true`.
5. **`tools/release.sh`:** Bei rotem Tag-Lauf nennt das Skript die roten
   Jobs mit ihrem Ergebnis. Ist ein macOS-Job `cancelled` ohne Runner,
   sagt es das ausdrücklich, und die Anleitung lautet in nummerierten
   Schritten: 1. Lauf öffnen (Link), 2. oben rechts „Re-run jobs", dann
   „Re-run failed jobs" (startet nur rote und abgebrochene Jobs, die grünen
   bleiben), 3. warten, bis `release` grün ist, 4. den Tag nie verschieben.
   Dieselbe Anleitung im Kopfkommentar des Skripts und in
   `docs/guides/` der Release-Anleitung, falls es eine gibt.
6. Kein paralleler Start des Rückfalls (Entscheidung des Eigentümers vom
   2026-11-17): Er läuft nur, wenn der Hauptpfad keinen Runner bekam.

### Tests
- `tools/tests/release_test.sh`: ein Gate, das einen roten Lauf mit
  `build-macos` `cancelled` ohne Runner meldet, ergibt die neue Ausgabe mit
  den vier Schritten; ein roter Lauf mit gescheitertem Schritt die bisherige.
  Mutationsbeweis: Erkennung „ohne Runner" immer falsch färbt den ersten
  Test rot.
- `tools/ci/classify-runner-loss.sh` mit fünf Fixtures (Job-Liste und
  Annotationen als JSON): `cancelled` ohne Runner, ohne Schritte und mit der
  Annotation ergibt `fallback=true`; dasselbe ohne Annotation (Abbruch von
  Hand in der Warteschlange) `false`;
  `failure` mit Runner und gescheitertem Schritt `false`; `cancelled` mit
  Runner und begonnenen Schritten `false`; `success` `false`.
  Mutationsbeweis: Bedingung auf `conclusion != success` vereinfacht färbt
  den Fall „gescheiterter Schritt" rot; Prüfung der Annotation entfernt
  färbt den Fall „Abbruch von Hand" rot.
- `actionlint` über `release.yml` grün.
- Probelauf wie Spezifikation 3, Link im Commit-Body.

### Fallstricke
- `runs-on` mit einer Liste von Labels heißt „alle Labels zugleich", nicht
  „eines davon"; ein Rückfall geht nur über einen zweiten Job.
- `timeout-minutes` zählt die Laufzeit, nicht die Wartezeit in der
  Schlange; ein „Rückfall nach Wartezeit" geht nicht über `timeout-minutes`.
- `needs` auf einen übersprungenen oder abgebrochenen Job überspringt den
  abhängigen Job, wenn sein `if` kein `always()` enthält.
- Für Intel endet die Unterstützung bei GitHub absehbar (Changelog von
  GitHub vom September 2025 nennt `macos-15-intel` als letztes Intel-Abbild
  bis August 2027); vor dem Bau an der Quelle prüfen und das Datum als
  Kommentar am Job eintragen.
- Flutter und Xcode der Intel-Abbilder können älter sein als die des
  arm64-Abbilds; die Versionen im Probelauf vergleichen.

### Akzeptanzkriterien
1. Ein Release, dessen macOS-Hauptpfad keinen Runner bekommt, wird ohne
   Eingriff über `macos-15-intel` fertig, mit denselben Prüfungen.
2. Ein Codefehler auf macOS bleibt rot und startet keinen Rückfall.
3. `tools/release.sh` nennt bei einem Abbruch ohne Runner die Schritte zum
   neuen Anlauf in nummerierter Form.

---

## HUM-532 · Daemon: Projektregister mit Projektwahl, RPC `Projects` (`list`, `get`, `save_choice`)

### Kontext
ADR-0024 Punkte 1 und 2. Heute kennt der Daemon Projekte nur als `work_dir`
in der Tabelle `sessions` des Recorders und als globales
`sandbox.work_dir`. Startfenster, Erststart-Bildschirm und `go` brauchen:
welche Projekte bekannt sind, wann und mit welchem Agenten zuletzt, ob eine
Sitzung läuft, und was der Mensch für das Projekt gewählt hat. Diese Wahl
darf nicht im Projektordner liegen: Das Projekt-Profil
`<projekt>/.humanitl/profile.toml` kommt mit jedem Klon und ist in einer
Sitzung mit schreibbarem Ordner für den Agenten schreibbar.

### Spezifikation
1. **Tabelle `projects`** (neue Migration im SQLite des Daemons, ADR-0008):
   `id` (UUID), `path` (kanonisch, `realpath`), `dev`, `ino` (beim ersten
   Start), `display_name` (Ordnername), `first_seen`, `last_started`,
   `last_agent`, `runs`, `choice` (JSON, siehe 3), `choice_saved_at`,
   `replaced_at` (leer für den aktuellen Eintrag). Die `id` ist die stabile
   Kennung des Projekts, auf die sich Projektwahl, Aufräumen und Clients
   beziehen; der Pfad ist es nicht. Mehrere Zeilen mit demselben `path`
   sind erlaubt (Verlauf ersetzter Ordner), aber genau eine aktuelle je
   Pfad: Teilindex `UNIQUE (path) WHERE replaced_at IS NULL`. Jede Zeile
   trägt einen Versionszähler `version`, der bei jeder Änderung von Wahl
   oder Eintrag steigt (für den Digest des Aufräumens, HUM-533). Ein Start (auch über `run` und `go`) legt den
   Eintrag an oder aktualisiert ihn, im selben Schritt, der heute die Zeile
   in `sessions` schreibt.
1a. **Sitzung gehört zu einer Projekt-`id`:** Die Tabelle `sessions` des
   Recorders bekommt die Spalte `project_id`, gesetzt beim Start der
   Sitzung im selben Schritt wie der Registereintrag. Alles, was je Projekt
   ausgewählt wird (Zuletzt, Aufräumen, Vergessen), wählt über
   `project_id`, nie über `work_dir`. Migration älterer Sitzungen ohne
   `project_id`: zugeordnet dem Projekt, das zu ihrem `started_at` am Pfad
   aktuell war, wenn das eindeutig feststellbar ist (nur ein Eintrag je
   Pfad, kein Ersatz seit dem Start); sonst bleibt `project_id` leer und die
   Sitzung gilt als „nicht zugeordnet".
2. **Wiedererkennen:** Liegt am Pfad ein Ordner mit anderem `dev`/`ino`,
   ist es ein anderes Projekt am alten Ort: Der alte Eintrag bekommt
   `replaced_at` und bleibt als Verlauf, in derselben Transaktion entsteht
   ein neuer aktueller Eintrag mit neuer `id`, und der Start fragt wie bei
   einem neuen Projekt. Ein Ordner, der fehlt, bleibt im Register
   mit `missing = true`.
3. **Projektwahl** (`ProjectChoice` in `humanitl_config`): genau die Schlüssel
   des Erststart-Bildschirms, als feste Erlaubnisliste im Code
   (`agent.adapter`, `sandbox.mode`, `sandbox.work_mode`,
   `sandbox.backend`, `sandbox.min_isolation` nur steigend,
   `hold.ask_mode`, `llm.server` bzw. der Schlüssel des Servers aus HUM-405,
   `sandbox.profile`). Neue Ebene der Auflösung mit Herkunft
   `project-choice`, Rang über dem Projekt-Profil und unter Umgebung und
   Kommandozeile. Ein Schlüssel außerhalb der Liste oder ein Wert, den eine
   Firmensperre oder `sandbox.min_isolation` nicht zulässt: `CONFIG_024`
   beim Speichern und beim Laden, nie still verworfen.
4. **RPC `Projects`** mit `oneof op`: `list` (Seite, neueste zuerst, je
   Eintrag Pfad, Name, letzter Start, letzter Agent, Läufe, ob eine Sitzung
   läuft und deren Kennung, ob Änderungen warten, `missing`), `get` (ein
   Projekt mit seiner Wahl und den Herkünften), `save_choice` (Projekt,
   Wahl; Antwort die gespeicherte Wahl oder `CONFIG_024`). Kein eigenes
   `forget`: Jedes Löschen, auch das Vergessen eines Projekts, geht über
   Plan, Bestätigung und Digest des Aufräumens (HUM-533, Grenze 3 im Kopf
   dieses Sprints).
   Ein Daemon älterer Minor-Version antwortet mit `IPC_005` bzw. dem
   üblichen Weg für unbekannte Operationen.
5. **Start ohne Projekt:** Ein `Sandbox(Start)` ohne Projektordner (leeres
   `work_dir` und kein `sandbox.work_dir` aus einer bewussten Wahl) wird mit
   `SANDBOX_053` abgelehnt, statt ein Verzeichnis des aufrufenden Prozesses
   oder `$HOME` zu nehmen. Die CLI schickt weiter ihr Arbeitsverzeichnis.
   Den Fall des Eigentümers (`SANDBOX_049` aus einem Starter) im Test
   nachstellen und den Weg dorthin im Commit-Body beschreiben.
6. **CLI:** `humanitl projects ls [--json]`, `humanitl projects show PATH
   [--json]` (Zeilen der Paritätstabelle, ADR-018). `projects forget` kommt
   mit HUM-533.

### Tests
- Migration auf einer Datenbank mit Sitzungen von gestern: `projects` wird
  aus `sessions` nachgefüllt (Pfad, letzter Start, Agent, Läufe), und jede
  Sitzung bekommt die `project_id` ihres Projekts; eine Sitzung, deren
  Zuordnung nicht eindeutig ist, bleibt ohne `project_id`.
- Zwei Projekte an einem Pfad (Ordner ersetzt): Sitzungen vor dem Ersatz
  gehören zur alten `id`, danach zur neuen; `list` und Läufe zählen
  getrennt. Mutationsbeweis: Auswahl über `work_dir` statt `project_id`
  färbt den Test rot.
- Ein Start legt an, ein zweiter zählt hoch; `list` sortiert nach letztem
  Start.
- Ordner ersetzt (neuer Inode am alten Pfad): alter Eintrag mit
  `replaced_at` und seiner Wahl bleibt, neuer aktueller Eintrag mit neuer
  `id` und ohne Wahl. Mutationsbeweis: Vergleich von `dev`/`ino` entfernt
  färbt den Test rot.
- Zwei aktuelle Einträge für denselben Pfad sind unmöglich (Teilindex);
  ein zweiter Ersatz desselben Pfads ergibt zwei Verlaufszeilen und eine
  aktuelle. Mutationsbeweis: Teilindex durch `UNIQUE (path)` ersetzt färbt
  den Test mit zwei Ersetzungen rot.
- Auflösung: Wert aus der Projektwahl schlägt das Projekt-Profil, verliert
  gegen `HUMANITL_*` und Schalter; Herkunft `project-choice`.
- `save_choice` mit `sandbox.min_isolation` unter dem globalen Wert,
  mit einem Schlüssel außerhalb der Liste, mit einem durch die
  Firmenvorgabe gesperrten Wert: je `CONFIG_024`, nichts gespeichert.
  Mutationsbeweis: Erlaubnisliste durch „alles außer denied" ersetzt färbt
  den Test mit dem fremden Schlüssel rot.
- Start ohne Projekt: `SANDBOX_053`, keine Sandbox, kein Eintrag.
  Mutationsbeweis: Rückfall auf das Arbeitsverzeichnis des Daemons
  wiederhergestellt färbt den Test rot.
- CLI-Tests für `projects ls|show` mit `--json`.

### Fallstricke
- Die Projektwahl ist kein Profil-File; `x-project-scope = "denied"` gilt
  für das Projekt-Profil aus dem Repository, nicht für sie. Ihre Grenze ist
  die Erlaubnisliste, nicht das Schema-Attribut.
- Pfade sind Anzeige und Schlüssel zugleich: kanonisieren beim Schreiben,
  nie aus einem Anzeigetext zurückrechnen; Namen mit Steuerzeichen werden
  vor der Ausgabe wie in `sessions summary` gesäubert.
- `sandbox.work_dir` bleibt als Einstellung bestehen, wird aber von der App
  nicht mehr als „aktuelles Projekt" missbraucht (HUM-537).
- macOS: Inode-Vergleich gilt auf APFS ebenso; Groß- und Kleinschreibung
  des Pfads wie in HUM-491 behandeln.

### Akzeptanzkriterien
1. Jeder Start hinterlässt einen Eintrag im Register; `projects ls` zeigt
   Projekt, Agent und Zeit.
2. Die Projektwahl wirkt beim nächsten Start, steht mit Herkunft
   `project-choice` in `GetConfig`, und der Agent kann sie nicht ändern.
3. Kein Start ohne Projekt; `SANDBOX_053` mit `why` und `fix`.
4. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` nennen die Projektwahl als
   Ebene auf dem Host, die weder Klon noch Agent schreiben.

---

## HUM-533 · Aufräumen und Vergessen: Plan und Ausführung über `Projects`, CLI `humanitl projects`

### Kontext
ADR-0024 Punkt 2, Vorgabe 1 des Eigentümers: alte Sitzungen und Projekte
aufräumen, mit Bestätigung, und klar, was genau gelöscht wird. Den Umfang
(Frage 8) hat der Eigentümer nicht beantwortet; gebaut wird die Vorgabe des
ADR, vom Eigentümer noch zu bestätigen (HUM-530).

### Spezifikation
1. **`Projects.cleanup_plan`** (Auswahl: Projekte, Alter, „alles außer
   laufend"): Antwort ist eine Liste von Einträgen, je Art, Projekt-`id`,
   Beschreibung in einem Satz, Größe in Bytes, Anzahl, und ein Digest. Der
   Digest bindet nicht die Anzeige (Zahlen und Größen könnten bei einer
   anderen, gleich großen Fassung gleich bleiben), sondern die genaue
   Identität und Fassung jedes Eintrags: die Zeilen-`id`s der Sitzungen und
   Flüsse, die Hashes der Blobs, die Kennung der wartenden Schicht, Projekt-
   `id` mit `version` und den SHA-256 des Inhalts der Projektwahl. Ausgewählt
   wird nur über `project_id` (HUM-532 Punkt 1a); Sitzungen ohne
   `project_id` („nicht zugeordnet") kommen in keinen Projektplan, nur in
   einen eigenen Plan mit der Auswahl `unassigned`, den der Mensch
   ausdrücklich wählt. Arten (Vorgabe, vom Eigentümer noch zu bestätigen):
   `recorded_flows` (Flüsse und Bodies der Sitzungen des Projekts im
   Recorder und Blob-Speicher), `waiting_changes` (wartende Schicht einer
   Wegwerf-Sitzung, nur mit ausdrücklicher Wahl und mit der Warnung in
   jedem Client, dass nicht übernommene Änderungen endgültig verloren
   sind), `project_choice` und `register_entry`. Depot-Versionen, die keine
   Sitzung nutzt, räumt das Depot (`humanitl agents update`,
   `humanitl agents remove`, ADR-0023), nicht dieses Aufräumen.
2. **Nie im Plan:** der Projektordner, die Audit-Kette (sie ist eine
   Hash-Kette; ein gelöschter Eintrag bräche sie), Regeln, die globale
   Konfiguration, das Agent-Depot (eigener Weg über `agents remove`). Der
   Plan sagt das in einer festen Zeile („Not deleted: the project folder,
   the audit log, …"), die jeder Client zeigt.
3. **`Projects.cleanup`** nimmt den Digest eines Plans und führt genau
   diesen aus: In einer Transaktion berechnet der Daemon den Plan mit
   derselben Auswahl neu, vergleicht den Digest und löscht nur bei
   Gleichheit, noch in derselben Transaktion. Hat sich etwas geändert (neue
   Sitzung, geänderte Wahl, auch gleich groß, Änderungen warten inzwischen,
   Sitzung läuft): `SANDBOX_054`, nichts gelöscht. Die Antwort
   nennt je Eintrag: gelöscht, mit freigegebenen Bytes, oder den Grund.
4. **Audit:** ein Eintrag je Ausführung mit den Arten und Mengen, ohne
   Inhalte.
5. **Vergessen ist ein Aufräumen mit engem Umfang:** `cleanup_plan` mit
   der Auswahl `forget` (Projekt-`id`) plant genau `project_choice` und
   `register_entry` dieses Projekts; ausgeführt wird es wie jeder Plan über
   `cleanup` mit dem Digest. Kein RPC löscht am Plan vorbei. CLI `humanitl
   projects forget PATH [--yes]` zeigt den Plan und fragt; das Startfenster
   bietet „Vergessen" am Projekt über denselben Weg.
6. **CLI:** `humanitl projects clean [PATH...] [--older-than DAYS]
   [--include-waiting-changes] [--yes] [--json]` zeigt den Plan als
   Tabelle und fragt; ohne Terminal und ohne `--yes` löscht es nichts und
   endet mit Exit 1 und dem Plan.

### Tests
- Plan über ein Projekt mit zwei Sitzungen: Flüsse, Bodies und Größe
  stimmen mit der Datenbank überein.
- Ausführung entfernt genau die Flüsse dieses Projekts; die eines anderen
  Projekts und jede Zeile der Audit-Kette bleiben. `audit verify` danach
  grün. Mutationsbeweis: Filter auf das Projekt entfernt färbt den Test rot.
- Plan, dann neue Sitzung im Projekt, dann `cleanup` mit dem alten Digest:
  `SANDBOX_054`, nichts gelöscht. Mutationsbeweis: Digest-Prüfung entfernt
  färbt den Test rot.
- Plan, dann die Projektwahl durch eine gleich große andere ersetzt
  (`sandbox.mode = "rw"` statt `"ro"`, gleiche Bytezahl), dann `cleanup`:
  `SANDBOX_054`, neue Wahl bleibt. Mutationsbeweis: Digest nur über Art,
  Anzahl und Größe färbt den Test rot.
- Zwei Projekte an einem Pfad (Ordner ersetzt): Aufräumen des neuen löscht
  nur dessen Flüsse, die des alten bleiben. Mutationsbeweis: Auswahl über
  `work_dir` färbt den Test rot.
- Nicht zugeordnete Sitzungen erscheinen in keinem Projektplan, nur im Plan
  `unassigned`.
- Laufende Sitzung: nie im Plan, auch nicht mit „alles".
- Wartende Änderungen ohne `--include-waiting-changes`: nicht im Plan; mit
  dem Schalter trägt der Eintrag die Warnung, und die CLI zeigt sie vor der
  Frage. Mutationsbeweis: Warnung entfernt färbt den Test rot.
- Ein Blob, auf den auch ein Fluss eines anderen Projekts zeigt
  (inhaltsadressiert), wird nicht gelöscht.
- CLI: ohne Terminal und ohne `--yes` Exit 1, Datenbank unverändert.
- Vergessen: Plan nennt genau Projektwahl und Registereintrag des einen
  Projekts; Flüsse bleiben. `cleanup` mit dem Digest eines Vergessen-Plans,
  nachdem die Wahl inzwischen geändert wurde: `SANDBOX_054`, nichts
  gelöscht. Mutationsbeweis: ein Vergessen, das ohne Plan direkt löscht
  (Abkürzung im Handler), färbt den Test rot, weil die geänderte Wahl
  verschwindet.

### Fallstricke
- Der Blob-Speicher ist inhaltsadressiert; gelöscht wird nur, was keine
  verbleibende Zeile mehr nennt, in derselben Transaktion gezählt.
- Löschen in SQLite gibt keinen Platz frei; die Antwort nennt die Bytes der
  Blobs und der Zeilen, nicht die Dateigröße der Datenbank. `VACUUM` nur,
  wenn keine Sitzung läuft, und nicht im Pfad der Antwort.
- Eine Art, die der Daemon nicht kennt, darf ein Client nie anbieten; die
  Liste der Arten kommt aus der Antwort.

### Akzeptanzkriterien
1. Jeder Client zeigt vor dem Löschen dieselbe Liste aus dem Daemon,
   mit Größe und der Zeile, was nicht gelöscht wird.
2. Ausgeführt wird nur ein unveränderter Plan.
3. Projektordner und Audit-Kette bleiben in jedem Fall unberührt;
   `docs/SECURITY.md` sagt das.

---

## HUM-534 · Einstellungskatalog aus dem Daemon: Kategorien, Texte, Herkunft, Sperren

### Kontext
ADR-0024 Punkt 3. Die Einstellungen der App kommen aus einem Register in
Dart (`app/lib/features/settings/registry/settings_registry.dart`, HUM-404),
Titel und Hilfe aus den ARB-Dateien, Kategorien aus einem Dart-Enum. Die
TUI (HUM-541) kann das nicht lesen. Der Daemon liefert heute Werte, Schema
(`x-tier`, `x-project-scope`) und Herkunft je Schlüssel (`FieldOrigin`);
eine Sperre der Firmenvorgabe kennt bisher nur `agents.source`
(`humanitl_config::agents`, HUM-518).

### Spezifikation
1. **Katalog im Crate `humanitl-config`**: je Eintrag `id` (Schlüssel oder
   Kennung einer besonderen Zeile wie `llm.servers`), `category`, `order`,
   `title`, `help`, `details`, `keywords`, `control` (`switch`, `choice`,
   `text`, `number`, `secret_ref`, `path`, `special`), `choices` mit Titeln,
   `next_session`, `tier`. Texte in `en` und `de` in Dateien neben dem
   Schema im Crate, beim Bau eingebettet; die ARB-Dateien behalten nur die
   Texte der App selbst (Frage 3, vom Hauptstrang am 2026-11-17 entschieden,
   ohne Widerspruch des Eigentümers).
   Kategorien mit Titel und Reihenfolge wie heute in `SettingsCategory`.
2. **`GetConfig.include_catalog`**: Antwort `SettingsCatalog` mit Einträgen,
   je Eintrag zusätzlich `origin` und `lock` (`locked_by`, nur lesbar,
   Grund) und `in_project_choice` (ob der Schlüssel auf der Erlaubnisliste
   der Projektwahl steht, HUM-532). Sprache aus `ui.language` oder einem
   Feld der Anfrage.
2a. **Projektbezug beim Lesen:** `GetConfigRequest` bekommt ein Feld
   `project` (Projekt-`id` aus dem Register oder kanonischer Pfad). Mit ihm
   löst der Daemon so auf, wie ein Start in diesem Projekt es täte, samt
   Projektwahl und Projekt-Profil, und die Herkunft `project-choice`
   erscheint; ohne es wie heute global. Unbekanntes Projekt: Auflösung ohne
   Projektwahl, kein Fehler.
2b. **Schreiben in die Projektwahl nur über `Projects.save_choice`.**
   `SetConfig` schreibt nie in die Projektwahl, nur in die globale
   `config.toml`. Ein Client, der für ein Projekt einen Schlüssel mit
   `in_project_choice` ändert, schickt `save_choice` mit der geänderten
   Wahl; für alle anderen Schlüssel gibt es in der Projektebene keinen
   Schreibweg, die Zeile ist dort nur lesbar mit dem Hinweis auf die
   globale Ebene.
2c. **Ereignis `ConfigChanged`** (neu im Proto, gibt es heute nicht): Der
   Daemon sendet es nach jedem erfolgreichen Schreiben, das die Auflösung
   ändern kann (`SetConfig`, `Projects.save_choice`, Löschen einer Wahl
   durch `cleanup`, Neuladen einer geänderten `config.toml`, Wechsel des
   Servers über `LlmServers`), mit den betroffenen Schlüsseln und, falls
   eine Projektwahl betroffen ist, der Projekt-`id`. Zustellung über den
   bestehenden Ereignisstrom (`Subscribe`, eigener Fall im `oneof`) oder
   einen eigenen Strom; die Wahl steht im Commit-Body. App und TUI laden
   danach neu.
3. **Sperren allgemein:** Ein Eingang in der Auflösung, über den eine
   Firmenvorgabe (BACKLOG 9, Punkt 21) jeden Schlüssel sperren kann; bis die
   Vorgabedatei existiert, kommt er aus den Tests und aus der Sperre von
   HUM-518. `SetConfig` auf einen gesperrten Schlüssel wird mit dem Code der
   Sperre aus HUM-518 abgelehnt.
4. **Vollständigkeit:** Ein Test läuft über alle Blätter des Schemas; jedes
   hat einen Katalog-Eintrag oder steht in einer Liste `hidden` mit Grund.
   Jeder Eintrag hat Titel in `en` und `de`.
5. **App auf den Katalog umstellen:** Das Dart-Register behält nur die
   Bauer der besonderen Zeilen, über die `id` zugeordnet; Titel, Hilfe,
   Stichwörter, Kategorie und Reihenfolge kommen aus dem Katalog. Die Suche
   (`settings_search.dart`) sucht in den Katalogtexten.
6. **CLI:** `humanitl config catalog [--json]` gibt den Katalog aus
   (Paritätszeile; Grundlage der TUI).

### Tests
- Vollständigkeitstest wie Spezifikation 4. Mutationsbeweis: ein neues
  Schema-Feld ohne Eintrag färbt ihn rot.
- `GetConfig` mit Katalog: Herkunft und Sperre je Eintrag; ein gesperrter
  Schlüssel hat `lock` gesetzt, `SetConfig` darauf wird abgelehnt.
- App: Widget-Test, dass die Kategorienliste und die Zeilen einer Kategorie
  aus einem Fake-Katalog kommen (Reihenfolge, Titel); ein gesperrter Wert
  ist nicht bedienbar und nennt die Vorgabe. Mutationsbeweis: Zeile ohne
  Prüfung von `lock` färbt den Test rot.
- Suche findet einen Eintrag über ein Stichwort aus dem Katalog.
- Zwei Projekte A und B mit verschiedener Projektwahl (`sandbox.mode`
  `disposable` und `rw`): `GetConfig(project = A)` und `(project = B)`
  liefern je ihren Wert mit Herkunft `project-choice`, ohne `project` den
  globalen. Mutationsbeweis: Projektfeld der Anfrage ignoriert färbt den
  Test rot.
- `SetConfig` auf `sandbox.mode` ändert die globale Datei, nie die Wahl von
  A oder B; `save_choice` für A ändert nur A.
- `ConfigChanged` mit zwei Abonnenten (Fake-App und Fake-TUI am selben
  Daemon): ein `SetConfig` des einen erreicht beide mit dem Schlüssel; ein
  `save_choice` für A nennt A. Mutationsbeweis: Senden nur bei `SetConfig`
  färbt den `save_choice`-Fall rot.

### Fallstricke
- Gemeinsam genutzte Dateien: `app/l10n/*.arb` nur im eigenen Abschnitt
  ändern (Regel in `CLAUDE.md`); Schlüssel, die in den Katalog wandern,
  erst entfernen, wenn kein Widget sie mehr liest.
- Texte im Katalog sind Anzeige; der Client säubert nichts zusätzlich, aber
  der Katalog enthält keine Steuerzeichen (Test).
- Die Herkunft `project-choice` aus HUM-532 muss im Katalog ankommen.

### Akzeptanzkriterien
1. App und CLI zeigen dieselben Kategorien, Titel und Reihenfolge aus einer
   Quelle im Daemon.
2. Ein neues Feld im Schema ohne Katalog-Eintrag bricht den Bau.
3. Gesperrte Werte sind in jedem Client nur lesbar und nennen die Vorgabe.

---

## HUM-535 · App: eine Einstellungskomponente für Reiter und Sheet

### Kontext
Vorgabe 1: In der Sitzung bleiben die Einstellungen das Sheet von unten
(heute `settings_overlay.dart`, drei Viertel der Höhe), im Startfenster sind
sie ein Reiter. Beide aus einer Komponente, damit eine Änderung überall
erscheint. Die Gestaltung des Sheets überarbeitet der Eigentümer später;
dieses Issue ändert nur die Struktur.

### Spezifikation
1. `SettingsPanel` (Kopf mit Suche, linke Spalte Kategorien, rechte Seite
   Zeilen oder Treffer) aus `settings_overlay.dart` herauslösen, ohne
   Verhalten zu ändern: Tastenwege, Fokus, Speichern je Zeile, Ablehnungen
   an der Zeile.
2. Zwei Wirte: `SettingsSheet` (das heutige Overlay, öffnet mit `Ctrl+,`
   in der Sitzung) und `SettingsTab` (Reiter im Startfenster, HUM-537),
   beide nur Rahmen um `SettingsPanel`.
3. Ein Wirt bestimmt nur Rahmen, Schließen und `Esc`; Inhalt, Suche,
   Sprung eines `ChangeSetting`-Fixes und Zustand der Kategorie gehören dem
   Panel.

### Tests
- Bestehende Tests des Overlays laufen unverändert gegen `SettingsSheet`.
- Ein Test rendert beide Wirte mit demselben Fake-Katalog und vergleicht
  die Zeilen einer Kategorie (gleiche Titel, gleiche Reihenfolge).
  Mutationsbeweis: im Reiter eine eigene Zeilenliste färbt ihn rot.
- `Esc` im Reiter schließt nichts; im Sheet wie bisher.
- `TargetPlatformVariant` für Linux und macOS (siehe Erinnerung
  „flutter test hides Linux").

### Fallstricke
- `HLayerScope` hält die Tastatur nur im Sheet; im Reiter darf das Panel
  die Tastatur nicht an sich ziehen.
- Vor dem Landen Screenshot beider Wirte (Erinnerung „UI quality
  complaint").

### Akzeptanzkriterien
1. Eine Zeile, die im Panel geändert wird, erscheint in Reiter und Sheet
   gleich, ohne Code in einem der Wirte.
2. Das Sheet verhält sich wie vorher.

---

## HUM-536 · Startvorschlag für ein neues Projekt als RPC (`Projects.proposal`)

### Kontext
Vorgabe 3: ein Bildschirm, alles vorbelegt, nichts muss, alles kann. Damit
CLI und App denselben Bildschirm zeigen, kommen Optionen, Vorgaben und
Gründe für Nichtverfügbares aus dem Daemon.

### Spezifikation
1. **`Projects.proposal(path)`** liefert `StartProposal`: je Feld (Agent mit
   Quelle Host oder Depot, Ordnermodus `disposable|rw|ro`, Isolation
   `shared_kernel|kernel`, Nachfragemodus, Modellserver, Profil) die
   Optionen, die Vorgabe, ob jede Option verfügbar ist und sonst den Grund
   als `Diagnostic` (etwa `SANDBOX_045` für die Kernel-Stufe ohne
   `/dev/kvm`, `SANDBOX_030` ohne Wegwerf-Schicht, Agent weder auf dem Host
   noch im Depot), Sperren der Firmenvorgabe, den Hinweis auf
   Cloud-Inferenz (HUM-516) für Agent und Server, und eine Zeile
   „Defaults: …" für den Weg ohne Bildschirm.
2. **Vorgaben:** aus der Auflösung, die ein Start jetzt bekäme, nie aus
   einer zweiten Tabelle. Ein bekanntes Projekt bekommt seine Projektwahl
   als Vorgabe.
3. **Prüfen ohne Speichern:** `Projects.proposal` mit einer Teilwahl gibt
   den Vorschlag neu berechnet zurück (eine gewählte Kernel-Stufe ändert
   etwa die verfügbaren Ordnermodi), ohne zu speichern. Gespeichert wird
   mit `save_choice` beim Start.
4. **CLI:** `humanitl projects proposal [PATH] --json` (Paritätszeile).

### Tests
- Ohne `/dev/kvm` (Fake-Erkennung): Kernel-Stufe nicht verfügbar mit
  `SANDBOX_045`, Vorgabe die gemeinsame Stufe.
- `sandbox.min_isolation = "kernel"`: gemeinsame Stufe nicht wählbar, Grund
  genannt. Mutationsbeweis: Untergrenze nicht beachtet färbt den Test rot.
- Agent nur im Depot: Quelle `depot` vorbelegt, `host` mit Grund.
- Cloud-Agent: Hinweis gesetzt; lokaler Server: nicht gesetzt.
- Vorgabe gleich dem, was `Sandbox(Plan)` ohne Wahl ergäbe (Vergleichstest
  über dieselbe Auflösung).

### Fallstricke
- Die Prüfung der Kernel-Stufe ist teuer (HUM-502); der Vorschlag nutzt
  das gespeicherte Ergebnis von `doctor`, nicht einen Probestart.
- Keine Option darf im Client auftauchen, die der Vorschlag nicht nennt.

### Akzeptanzkriterien
1. Ein Aufruf liefert alles, was der Erststart-Bildschirm zeigt, mit
   Gründen für jede nicht wählbare Option.
2. Die Vorgabe ist genau der Start, der ohne Bildschirm geschähe.

---

## HUM-537 · App: Startfenster mit Laufend, Zuletzt, Neu, Aufräumen und Einstellungen

### Kontext
Vorgabe 1. Die App startet heute direkt in die Oberfläche einer Sitzung, mit
dem globalen `sandbox.work_dir` als Projekt. Aus einem Starter ohne
Arbeitsverzeichnis endete das beim Eigentümer in `SANDBOX_049`.

### Spezifikation
1. **Startfenster** ist der erste Bildschirm, solange die App an keine
   Sitzung gehängt ist. Bereiche (Struktur, nicht Gestaltung):
   - **Laufend:** Sitzungen aus `Projects.list` mit laufender Sitzung;
     „Anhängen" öffnet die Sitzungsansicht dieser Sitzung.
   - **Zuletzt:** Projekte mit Name, Pfad, Agent, relativer Zeit; „Starten"
     startet mit der Projektwahl; fehlende Ordner ausgegraut mit Grund.
   - **Neu:** Ordner wählen (bestehender Dialog `work_dir_picker.dart`),
     dann der Erststart-Bildschirm (HUM-538).
   - **Aufräumen:** Auswahl, Plan aus `Projects.cleanup_plan` als Liste mit
     Größen und der Zeile, was nicht gelöscht wird; Bestätigung mit Nennung
     der Anzahl („3 Einträge, 412 MB löschen"), dann `cleanup`.
   - **Einstellungen:** Reiter mit `SettingsTab` (HUM-535).
2. **Nie ohne Projekt:** Kein Weg der App startet eine Sitzung ohne
   gewähltes Projekt; `sandbox.work_dir` wird nicht mehr als stilles
   „aktuelles Projekt" benutzt, sondern das Projekt geht mit dem Start
   (`Sandbox(Start)` mit `work_dir`).
3. **Rückweg:** Aus einer Sitzung führt ein Befehl (Palette, Menü) zurück
   ins Startfenster; die Sitzung läuft weiter und steht unter „Laufend".
4. Texte nur über ARB (`en` Quelle, `de` Übersetzung), Einträge ans Ende
   der Dateien.

### Tests
- Start der App ohne Projekt und ohne laufende Sitzung: Startfenster, kein
  `Sandbox(Start)` im Fake-Daemon. Mutationsbeweis: Rückfall auf
  `sandbox.work_dir` färbt den Test rot.
- Laufende Sitzung: App startet ins Startfenster mit ihr unter „Laufend";
  ein Start aus „Zuletzt" in einem anderen Projekt nennt `CLI_005`, solange
  der Daemon eine Sitzung führt (Frage 9: mehrere Sitzungen später,
  HUM-549).
- Zuletzt: Reihenfolge und Inhalt aus dem Fake; fehlender Ordner nicht
  startbar.
- Aufräumen: Bestätigungsdialog zeigt Plan und Ausschlusszeile; „Abbrechen"
  schickt kein `cleanup`; „Löschen" schickt den Digest des gezeigten Plans.
  Mutationsbeweis: `cleanup` ohne Digest färbt den Test rot.
- Screenshot des Fensters vor dem Landen.

### Fallstricke
- `app/lib/core/ipc/fake_daemon_client.dart` ist gemeinsam genutzt: nur
  anhängen.
- Kein Feature importiert ein anderes (`tools/check-deps.sh`); das
  Startfenster nutzt `SettingsTab` über `core/ui` oder eine erlaubte
  Schicht, wie heute der Ordnerdialog.
- Ein gelöschtes Projekt kann noch in einer offenen Liste stehen; nach
  `cleanup` neu laden.

### Akzeptanzkriterien
1. Die App öffnet nie eine Sitzung ohne Projekt; ohne Projekt zeigt sie das
   Startfenster.
2. Laufende Sitzungen lassen sich anhängen, zuletzt genutzte Projekte
   neu starten.
3. Vor jedem Löschen steht die Liste des Daemons mit Größe und Ausschlüssen.

---

## HUM-538 · App: Erststart-Bildschirm unter „Neu" und für ein unbekanntes Projekt

### Kontext
Vorgabe 3 in der App: derselbe eine Bildschirm wie in `humanitl go`, aus
`Projects.proposal` (HUM-536).

### Spezifikation
1. Ein Bildschirm: Projektname, eine Zeile mit den Vorgaben („Claude Code
   from the host · disposable folder · shared kernel · ask in the app ·
   local model server"), Knopf „Start" mit dem Fokus, Knopf „Anpassen…".
2. „Anpassen…" klappt die Felder des Vorschlags auf: Agent und Quelle,
   Ordnermodus, Isolation (nicht verfügbare Optionen mit Grund, nicht
   versteckt), Nachfragemodus, Modellserver mit Hinweis auf Cloud-Inferenz,
   Profil. Jede Änderung fragt `proposal` neu.
3. „Start" speichert die Wahl (`save_choice`) und startet. Ein Hinweis auf
   Cloud-Inferenz verlangt die Bestätigung wie heute
   (`cloud_confirm_dialog.dart`).
4. Ein bekanntes Projekt zeigt den Bildschirm nicht; „Zuletzt" startet
   direkt. Die Wahl ändern geht über einen Eintrag „Startwahl ändern" am
   Projekt.

### Tests
- `Enter` ohne weitere Eingabe startet mit genau den Vorgaben des Fakes.
- Nicht verfügbare Isolation ist sichtbar, nicht wählbar, mit Grund.
  Mutationsbeweis: Option ohne Prüfung der Verfügbarkeit färbt den Test
  rot.
- Gesperrter Wert nur lesbar mit Vorgabe als Grund.
- Bekanntes Projekt: kein Bildschirm.

### Fallstricke
- Der Fokus liegt auf „Start", aber `Enter` in einem aufgeklappten Feld
  darf nicht starten, bevor das Feld übernommen ist.
- Screenshot vor dem Landen.

### Akzeptanzkriterien
1. Nichts muss gewählt werden; `Enter` startet.
2. Alles des Vorschlags lässt sich wählen; was nicht geht, sagt warum.
3. Der nächste Start desselben Projekts zeigt keinen Bildschirm.

---

## HUM-539 · `humanitl app [PATH]`: App mit vorgewähltem Projekt starten oder die laufende erreichen

### Kontext
Vorgabe 1: `humanitl app [.]` öffnet das Startfenster, `.` wählt den
aktuellen Ordner vor. Die App heißt auf Linux `humanitl-app` (Desktop-Datei
`packaging/deb/dev.humanitl.humanitl.desktop`, `Exec=humanitl-app %U`), auf
macOS `Humanitl.app`. Frage 4 des ADR ist entschieden (Hauptstrang,
2026-11-17, ohne Widerspruch des Eigentümers): Bei laufender App geht die
Bitte über den Daemon.

### Spezifikation
1. **App:** nimmt `--project PATH` an; das Startfenster wählt das Projekt
   vor (bekannt: unter „Zuletzt" markiert; unbekannt: Erststart-Bildschirm).
   Ein Pfad, der kein Ordner ist, steht als Befund im Startfenster.
2. **CLI `humanitl app [PATH]`:** kanonisiert `PATH` (`.` ist das
   Arbeitsverzeichnis). Ist eine App mit dem Daemon verbunden, schickt das
   CLI die Bitte über den Daemon (Operation `Projects.open_in_app`, die der
   Daemon als Ereignis an die verbundene App gibt); sonst startet es die App (`humanitl-app --project PATH` aus dem
   Ordner des eigenen Programms oder dem `PATH`; macOS
   `open -a Humanitl --args --project PATH`). App nicht gefunden: `CLI_007`
   mit Fix, wie man sie installiert.
3. Ohne `PATH`: Startfenster ohne Vorwahl.
4. Die App bringt ihr Fenster nach vorne, wenn eine Bitte ankommt.

### Tests
- CLI: App nicht auffindbar: `CLI_007`, Exit 1. Ein Fake-Programm statt der
  App bekommt genau `--project <kanonischer Pfad>`.
- Verbundene App (Fake-Daemon): Ereignis kommt an, kein zweiter Prozess.
  Mutationsbeweis: Weiterleitung entfernt färbt den Test rot (es startet ein
  zweiter Prozess).
- App: `--project` auf einen bekannten und auf einen unbekannten Ordner;
  auf eine Datei: Befund.

### Fallstricke
- Der Pfad aus der Bitte ist für die App nur eine Vorwahl; gestartet wird
  erst nach „Start", nie durch die Bitte allein.
- Auf Wayland darf eine App ihr Fenster nicht selbst nach vorne holen; dann
  eine Benachrichtigung statt stillem Nichts.
- Desktop-Datei unverändert lassen; ein Start aus dem Starter hat kein
  Projekt und zeigt das Startfenster.

### Akzeptanzkriterien
1. `humanitl app .` zeigt das Startfenster mit diesem Ordner vorgewählt,
   ob die App lief oder nicht.
2. Kein Weg startet über `humanitl app` eine Sitzung ohne Bestätigung im
   Fenster.

---

## HUM-540 · TUI-Grundlage: ratatui im CLI, Terminal auf jedem Weg zurück

### Kontext
ADR-0024 Punkt 6. Der Einstellungsbildschirm, der Erststart in `go`, die
Interceptor-Ansicht und die Hilfe der Präfixtaste teilen eine Grundlage.
Heute setzt `daemon/bin/humanitl/src/tty.rs` den Rohmodus über `rustix`.

### Spezifikation
1. Abhängigkeiten `ratatui` (Backend `crossterm`) und `crossterm` in der
   aktuellen stabilen Fassung, zentral in `daemon/Cargo.toml` vom
   Hauptstrang eingetragen, `dep.workspace = true` in
   `daemon/bin/humanitl/Cargo.toml`. `cargo deny check` grün; die Größe von
   `humanitl` vorher und nachher im Commit-Body.
2. Modul `daemon/bin/humanitl/src/tui/` mit: `TerminalGuard` (Rohmodus,
   alternativer Schirm, Cursor, Maus, Bracketed Paste; `Drop` stellt alles
   wieder her), Panic-Hook, der vor der Meldung zurücksetzt, Behandlung von
   `SIGTERM`, `SIGHUP`, `SIGINT` und `SIGTSTP`/`SIGCONT`, Prüfung der
   Mindestgröße (`TERM_003`), ein gemeinsamer Rahmen für Kopf, Fuß mit
   Tasten und Suche.
3. Zusammenspiel mit `tty.rs`: ein Weg in den Rohmodus je Prozess. Entweder
   übernimmt der Guard die Einstellung aus `tty::RawMode`, oder `RawMode`
   nutzt den Guard; zwei Besitzer der `termios` gibt es nicht.
4. `NO_COLOR` gilt; ohne Terminal kein TUI-Bildschirm, sondern der
   dokumentierte Weg ohne (je Bildschirm im Issue genannt).

### Tests
- Bildschirme über `ratatui::backend::TestBackend` als Schnappschuss.
- Pseudoterminal-Test (wie `daemon/bin/humanitl/tests/tty.rs`): nach
  normalem Ende, nach Panic, nach `SIGTERM` und `SIGHUP` sind `termios`
  und alternativer Schirm wie vorher. Mutationsbeweis: Panic-Hook ohne
  Zurücksetzen färbt den Panic-Test rot.
- Zu kleines Terminal: `TERM_003`, Terminal unverändert.
- macOS-Querprüfung (`aarch64-apple-darwin`, Erinnerung „macOS
  cross-clippy").

### Fallstricke
- `SIGTSTP` (`Ctrl+Z`): vor dem Anhalten zurücksetzen, nach `SIGCONT` neu
  setzen und neu zeichnen.
- Ein Terminal, das den alternativen Schirm nicht kennt, darf keinen Müll
  zurücklassen; dann ohne alternativen Schirm und am Ende löschen.
- crossterm liest von `/dev/tty`, wenn `stdin` kein Terminal ist; das darf
  den Weg ohne TUI nicht umgehen.

### Akzeptanzkriterien
1. Kein Ausgang eines TUI-Bildschirms hinterlässt ein verstelltes Terminal.
2. Daemon und Shim linken weder ratatui noch crossterm (`cargo tree`-Prüfung
   im Test oder in `tools/check-deps.sh`).

---

## HUM-541 · Einstellungs-TUI `humanitl settings`

### Kontext
Vorgabe 2. Der Befehl heißt `humanitl settings` (Eigentümer, 2026-11-17,
Frage 1).

### Spezifikation
1. Bildschirm aus `GetConfig(include_catalog)` (HUM-534): Kategorien links,
   Zeilen rechts, Suche mit `/`, Herkunft je Zeile (`default`, `global`,
   `project-choice`, …), gesperrte Werte nur lesbar mit Vorgabe.
2. Bearbeiten nach `control`: Schalter, Auswahl, Text, Zahl, Pfad; Geheimnisse
   nur als Verweis wie in der App (`secret_ref`), nie als Klartext im
   Terminal.
3. **Besondere Zeilen bekommen echte Bedienelemente** über dieselben RPC
   wie in der App, je `id`:
   - Modellserver (`llm.servers`, `llm.server_usage`): Liste, anlegen,
     umbenennen, aktiv setzen, entfernen über `LlmServers`; Schlüssel über
     `LlmKey` mit verdeckter Eingabe ohne Echo, einmal gesendet, nie
     angezeigt; Prüfen über `ProbeLlm`.
   - Agent, Ordner, Sandbox-Modus, Backend, Frist, Nachfragemodus: Auswahl
     mit den Optionen und Gründen aus `SandboxBackends` bzw. dem Katalog,
     geschrieben über `SetConfig` bzw. `save_choice`.
   - Egress-Proxy samt CA und Auflösung: wie in der App über `SetConfig`;
     Zugangsdaten des Proxys nur über `Egress.SetAuth` (verdeckte Eingabe
     ohne Echo, einmal gesendet) und `Egress.RemoveAuth`, nie über
     `SetConfig`; „Prüfen" über `Egress`.
   - Daemon: Version aus `GetInfo`, Neustart über denselben RPC wie die
     App, Updateprüfung über `Update`, Pfad der `config.toml` aus
     `ConfigSnapshot.config_path` mit „im Editor öffnen" (`$EDITOR`, wie
     `humanitl config edit`).
   Ausgenommen sind nur Einstellungen des App-Fensters selbst (heute die
   Skalierung, `uiScale`, die die App lokal hält); der Katalog markiert sie
   `client = app`, die TUI zeigt sie nicht.
4. Schreiben je Änderung: globale Ebene über `SetConfig`; Projektebene
   (Schalter `--project PATH`, in der Sitzung das Projekt der Sitzung) liest
   mit `GetConfig(project)` und schreibt Schlüssel mit `in_project_choice`
   über `Projects.save_choice`, nie über `SetConfig`; andere Schlüssel sind
   dort nur lesbar. Ablehnung an der Zeile mit `why` und `fix`.
   Live-Aktualisierung bei `ConfigChanged` (HUM-534).
5. Ohne Terminal oder mit `--json`: kein Bildschirm, Hinweis auf
   `humanitl config get|set`, Exit 1.

### Tests
- Schnappschüsse über `TestBackend` mit einem Fake-Katalog: Kategorien,
  Reihenfolge, Suche, gesperrte Zeile.
- Tastenfolge „Kategorie wählen, Schalter umlegen" schickt genau ein
  `SetConfig` mit Schlüssel und Wert. Mutationsbeweis: gesperrte Zeile
  bearbeitbar färbt den Sperrtest rot.
- Paritätstest: jede Kategorie und jeder Eintrag des Katalogs erscheint in
  der TUI (gleiche Menge wie in der App, HUM-535).
- Ablehnung des Daemons erscheint an der Zeile.
- Projektebene mit zwei Projekten A und B (Fake-Daemon, verschiedene
  Wahl): `--project A` zeigt den Wert von A mit Herkunft `project-choice`;
  Umschalten von `sandbox.mode` schickt `save_choice` für A und kein
  `SetConfig`. Mutationsbeweis: Schreiben über `SetConfig` färbt den Test
  rot.
- Modellserver: Anlegen in der TUI schickt `LlmServers.Add`; der Schlüssel
  erscheint weder im Schnappschuss noch in der Ausgabe. Mutationsbeweis:
  Eingabe mit Echo färbt den Test rot.
- `ConfigChanged` von einem anderen Client lädt die gezeigte Zeile neu.
- Zugangsdaten des Egress-Proxys: Eingabe in der TUI schickt genau ein
  `Egress.SetAuth` und kein `SetConfig`, „Entfernen" ein
  `Egress.RemoveAuth`; das Passwort steht weder im Schnappschuss noch in
  der Ausgabe. Mutationsbeweis: Schreiben über `SetConfig` färbt den Test
  rot.

### Fallstricke
- Keine eigene Liste, keine eigene Prüfung der Werte; Gültigkeit entscheidet
  der Daemon.
- Lange Hilfetexte umbrechen nach Anzeigebreite (Unicode), nicht nach Bytes.
- Ein `secret_ref` wird nie aufgelöst angezeigt.

### Akzeptanzkriterien
1. Jede Einstellung der App ist in der TUI zu sehen und zu ändern, mit
   gleicher Herkunft und Sperre, über dieselben RPC; ausgenommen nur die
   als `client = app` markierten Einstellungen des App-Fensters.
2. Ein neuer Katalog-Eintrag erscheint in der TUI ohne Codeänderung dort.

---

## HUM-542 · `humanitl go [AGENT]`: Agent als Wort, `--dir`, `--disposable`, `--rw`, `--ro`, `--kernel`, Projektwahl

### Kontext
Vorgabe 4: `humanitl go claude`, höchstens etwa vier Wörter für den
gewöhnlichen Start, mehr über Schalter. Heute nimmt `go` ein Kommando nur
nach `--` (`daemon/bin/humanitl/src/cli.rs`, `GoArgs`) und ist immer
`run --disposable`. Grammatik entschieden vom Eigentümer am 2026-11-17
(Frage 5): Der gewöhnliche Start bleibt `humanitl go [AGENT]`, alles
Weitere sind die Schalter `--ro`, `--rw`, `--kernel`, `--dir PATH`,
`--disposable`.

### Spezifikation
1. `humanitl go [AGENT] [--dir PATH] [--disposable|--rw|--ro] [--kernel]
   [--yes] [-- CMD...]`. `AGENT` ist eine Kennung aus der Liste der Adapter
   (`opencode`, `pi`, `claude`, `codex`, `agy`); unbekannt: Befund mit der
   Liste. Das Programm kommt nach ADR-0023 aus Host oder Depot.
2. `--dir PATH`: Projektordner, Vorgabe das Arbeitsverzeichnis. Der
   allgemeine Schalter `--work` (aus `sandbox.work_dir` erzeugt) gilt weiter;
   beide zugleich sind ein Fehler.
3. Ordnermodus, drei Schalter, höchstens einer: `--disposable` (Wegwerf),
   `--rw` (direkt schreibend), `--ro` (nur lesbar). Ohne Schalter gilt die
   Projektwahl, für ein neues Projekt die Vorgabe des Vorschlags (HUM-536).
4. `--kernel`: Kernel-Stufe für diesen Start (hebt `sandbox.min_isolation`
   für die Sitzung, wie `--sandbox-min-isolation` in HUM-489); nicht
   verfügbar: der Befund der Stufe (`SANDBOX_045` u. a.), kein stiller
   Rückfall.
5. Alle Schalter gelten für diesen Aufruf und ändern die gespeicherte
   Projektwahl nicht.
6. Ein bekanntes Projekt startet mit seiner Projektwahl (HUM-532); `AGENT`
   und Schalter überschreiben sie für diesen Aufruf. Ein unbekanntes Projekt
   bekommt den Erststart-Bildschirm (HUM-543); bis der da ist, Vorgaben und
   eine Zeile.
7. Splash (HUM-496) und Frage nach wartenden Änderungen bleiben.
8. `backlog/CONVENTIONS.md` 3.8 (Zeile `humanitl go`) im selben Commit
   nachziehen.

### Tests
- `go claude` setzt den Adapter `claude` für den Start (Fake-Daemon prüft
  die Anfrage).
- `go` ohne Wort im bekannten Projekt nimmt den Agenten der Projektwahl.
  Mutationsbeweis: Projektwahl ignoriert färbt den Test rot.
- `--rw`, `--ro`, `--disposable` setzen Modus und `work_mode`; zwei davon
  zugleich: Fehler von clap.
- `--dir` und `--work` zugleich: Fehler. `--dir` auf eine Datei: Befund.
- `--kernel` ohne verfügbare Kernel-Stufe: Befund, kein Start mit der
  gemeinsamen Stufe. Mutationsbeweis: stiller Rückfall färbt den Test rot.
- Nach einem Aufruf mit Schaltern ist die gespeicherte Projektwahl
  unverändert.
- `go foo`: Befund mit der Liste der Agenten, Exit 1.
- `go claude -- --resume` reicht `--resume` an den Agenten.

### Fallstricke
- Ein Ordner namens `claude` im Arbeitsverzeichnis ist kein Agent;
  Positionsargument ist immer der Agent, der Ordner nur über `--dir`.
- `--ro` ist mit einer Wegwerf-Schicht sinnlos; deshalb schließen die drei
  Schalter einander aus.
- `--ask terminal` bleibt mit `CLI_002` abgelehnt.

### Akzeptanzkriterien
1. `humanitl go claude` startet Claude Code im aktuellen Ordner mit den
   Vorgaben oder der Projektwahl, ohne weitere Angaben.
2. Alles Weitere geht über die fünf Schalter; keiner ändert still die
   gespeicherte Wahl.

---

## HUM-543 · Erststart-Bildschirm in `humanitl go`

### Kontext
Vorgabe 3 im Terminal: derselbe Bildschirm wie in der App (HUM-538), aus
`Projects.proposal`.

### Spezifikation
1. Beim ersten `go` in einem unbekannten Ordner, an einem Terminal und ohne
   `--json` und `--yes`: ein TUI-Bildschirm (HUM-540) mit der Zeile der
   Vorgaben, „Start" mit Fokus (`Enter`), „Anpassen…" (`a`), Abbrechen
   (`Esc`, `q`, nichts gestartet, Exit 130 wie bei `Ctrl+C`).
2. „Anpassen…" zeigt die Felder des Vorschlags wie HUM-538, nicht
   verfügbare Optionen mit Grund.
3. „Start" speichert die Wahl und startet; danach wie HUM-542.
4. `--yes`, kein Terminal oder `--json`: kein Bildschirm, eine Zeile auf
   `stderr` „First start in <name>: <Vorgaben>. Change with humanitl
   projects show <path>" (Wortlaut festlegen), Wahl gespeichert, Start.
5. Der Bildschirm endet vor dem Splash; Splash und Agent bekommen ein
   zurückgesetztes Terminal.

### Tests
- Pseudoterminal: `Enter` startet mit den Vorgaben, die Wahl ist gespeichert,
  das zweite `go` zeigt keinen Bildschirm. Mutationsbeweis: Wahl nicht
  gespeichert färbt den Test des zweiten Starts rot.
- `--yes`: keine Ausgabe auf `stdout`, genau eine Zeile auf `stderr`.
- `Esc`: kein Start, keine Wahl gespeichert.
- Terminalzustand nach dem Bildschirm unverändert (Test aus HUM-540).

### Fallstricke
- Zwischen Bildschirm und Splash darf kein Tastendruck verloren gehen oder
  an den Agenten rutschen; Eingabepuffer nach dem Bildschirm leeren.
- Der Hinweis auf Cloud-Inferenz folgt den Regeln von HUM-516, auch mit
  `--yes`.

### Akzeptanzkriterien
1. Erster Start: ein Bildschirm, `Enter` genügt.
2. Zweiter Start: kein Bildschirm.
3. Ohne Terminal oder mit `--yes`/`--json`: kein Bildschirm, eine Zeile.

---

## HUM-544 · Präfixtaste `ctrl-b` mit fester Tabelle; `Ctrl+,` direkt nur mit kitty-Tastaturprotokoll

### Kontext
Vorgabe 4 und ADR-0024 Punkt 8. Heute gibt es eine Präfixtaste nur zum
Abhängen (`terminal.detach_prefix`, Vorgabe `ctrl-a`, HUM-279, Logik in
`daemon/bin/humanitl/src/cmd/attach.rs`, Lesart `DetachKeys` in
`daemon/crates/config/src/model.rs`). `Ctrl+,` hat in den meisten
Terminals keinen Steuercode. Entscheidung des Eigentümers vom 2026-11-17
(Frage 6): Vorgabe wird `ctrl-b` wie in tmux; `Ctrl+,` direkt nur mit dem
kitty-Tastaturprotokoll.

### Spezifikation
1. Neuer Schlüssel `terminal.prefix` mit denselben erlaubten Werten wie
   heute, **Vorgabe `ctrl-b`**; `terminal.detach_prefix` bleibt als Alias
   (`alias.rs`) mit Hinweis beim Laden. `--detach-keys` bleibt, neu auch
   `--prefix`. Ein ausdrücklich gesetztes `ctrl-a` (unter einem der beiden
   Namen) bleibt, wie es ist.
2. **Umstieg:** Wer bisher die Vorgabe `ctrl-a` hatte (kein Wert gesetzt),
   bekommt beim ersten Anhängen nach dem Wechsel eine Zeile auf `stderr`:
   „The prefix key is now Ctrl+B (was Ctrl+A). Keep the old one: humanitl
   config set terminal.prefix ctrl-a". Gezeigt einmal je Nutzer, gemerkt im
   Datenordner des Daemons, nie im Projekt.
3. Feste Tabelle nach dem Präfix: `d` abhängen, `,` Einstellungen, `i`
   Interceptor, `?` Hilfe (kleines Overlay der TUI mit der Tabelle), Präfix
   zweimal oder sein Buchstabe schickt es einmal an den Agenten, jede andere
   Taste geht mit dem Präfix unverändert weiter (wie heute).
4. Die Tabelle steht an einer Stelle im Code und wird in Hilfe,
   `cli.md` und Settings-Text aus ihr erzeugt.
5. **`Ctrl+,` direkt:** nur, wenn die Ausgabe des Agenten das kitty-
   Tastaturprotokoll eingeschaltet hat (`CSI > flags u`, Zustand im Client
   aus dem Strom gelesen, Abschalten `CSI < u` beachtet) und der Schlüssel
   `terminal.direct_keys` (Vorgabe `true`) es erlaubt. Dann wird
   `CSI 44 ; 5 u` abgefangen. Der Client schaltet das Protokoll nie selbst
   ein.
6. Die Tabelle wirkt in `sandbox attach`, `run -- CMD` am Terminal und `go`.
   Die Ansichten selbst baut HUM-545 und HUM-546; bis dahin sagen `,` und
   `i` in einer Zeile, dass sie kommen, oder fehlen in der Tabelle.

### Tests
- Zustandsautomat der Tasten als Einheitstest: jede Taste nach dem Präfix,
  Präfix am Ende eines Lesevorgangs (wartet ohne Frist, wie heute), Präfix
  zweimal. Mutationsbeweis: Präfix zweimal schickt zwei Bytes färbt den
  Test rot.
- kitty-Zustand: `CSI > 1 u` im Strom, dann `CSI 44;5u` von der Tastatur
  öffnet Einstellungen; ohne vorheriges Einschalten geht dieselbe Folge an
  den Agenten. Mutationsbeweis: Abfangen ohne Prüfung des Zustands färbt den
  zweiten Fall rot.
- Alias: `terminal.detach_prefix = "ctrl-e"` wirkt als `terminal.prefix`.
- Vorgabe ohne Konfiguration ist `ctrl-b`; ein gesetztes `ctrl-a` bleibt.
  Mutationsbeweis: Vorgabe zurück auf `ctrl-a` färbt den Test rot.
- Umstiegszeile: erscheint beim ersten Anhängen ohne gesetzten Wert, nicht
  beim zweiten und nicht mit gesetztem Wert.
- Prüfung der Konfiguration: dieselben Ausschlüsse wie heute (`ctrl-[`,
  `ctrl-h`, `ctrl-i`, `ctrl-j`, `ctrl-m`).

### Fallstricke
- Kollision mit tmux: Ein tmux in der Sandbox hat dasselbe Präfix. Zweimal
  `Ctrl+B` schickt es einmal hindurch; das steht in der Beschreibung des
  Schlüssels, in der Hilfe (`?`) und in `docs/UX.md`. `ctrl-b` ist in
  Shells und readline „ein Zeichen zurück"; auch das kommt mit zweimal
  Drücken beim Agenten an.
- Das Einschalten des kitty-Protokolls durch den Agenten muss den Filter
  `FullScreen` passieren; prüfen, ob `CSI > … u` heute durchgeht, und den
  Filter nicht in diesem Issue erweitern (wenn nicht: `Ctrl+,` direkt ist
  dann nie aktiv, im Commit-Body nennen und eigenes Issue anlegen).
- Die Beschreibung von `terminal.detach_prefix` in `model.rs`,
  `DetachKeys::DEFAULT_SPEC` und die Texte von `attach.rs` und `run.rs`
  nennen heute `ctrl-a`; alle mitziehen.
- Die Oberfläche der App liest die Präfixtaste nicht (heute so); bleibt so.

### Akzeptanzkriterien
1. Eine Präfixtaste, konfigurierbar, mit fester Tabelle in allen drei
   Wegen ins Terminal einer Sitzung.
2. Keine Taste geht dem Agenten verloren außer der gewählten Präfixtaste,
   und auch die kommt mit zweimal Drücken an.

---

## HUM-545 · Ansichtswechsel in der Sitzung: Einstellungs-TUI über dem Agenten und zurück

### Kontext
ADR-0024 Punkt 9. Präfix und `,` öffnen die Einstellungs-TUI (HUM-541) in
der laufenden Sitzung; danach soll der Agent aussehen wie vorher. Der Daemon
hält keinen Bildschirmzustand, nur einen Rückstand roher Bytes.

### Spezifikation
1. Beim Wechsel: Eingabe an den Agenten anhalten, Ausgabe des Agenten im
   Client puffern (Obergrenze, etwa 4 MiB; darüber verwerfen und merken),
   eigenen alternativen Schirm zeigen.
2. Beim Zurück: eigenen Schirm verlassen, gepufferte Ausgabe schreiben
   (oder, wenn verworfen, nicht), dann den Agenten zum Neuzeichnen bringen:
   über die `Terminal`-RPC eine Größenänderung auf eine Zeile weniger und
   zurück auf die echte Größe.
3. Größenänderungen während einer Ansicht gehen beim Zurück an den Agenten.
4. Je ausgelieferten Agenten (OpenCode, pi, Claude Code, Codex, `agy`)
   prüfen und im Commit-Body festhalten, ob das Neuzeichnen wirkt; wo
   nicht, ein Hinweis in der Hilfe (`Ctrl+L` im Agenten).
5. Der Agent läuft während der Ansicht weiter; ein Halt einer Anfrage zeigt
   sich in der Fußzeile der Ansicht.

### Tests
- Mit einem Fake-Agenten (Programm, das bei `SIGWINCH` seinen Schirm neu
  schreibt): nach Wechsel und Zurück steht dessen Schirm. Mutationsbeweis:
  ohne Größenänderung beim Zurück färbt den Test rot.
- Ausgabe während der Ansicht erscheint nach dem Zurück, in Reihenfolge.
- Puffer über der Grenze: verworfen, Neuzeichnen trotzdem.
- Keine Taste aus der Ansicht erreicht den Agenten.

### Fallstricke
- Der Agent kann selbst im alternativen Schirm sein; der Client verlässt
  beim Zurück nur seinen eigenen.
- Die Größenänderung kurz auf eine Zeile weniger ist für den Agenten
  sichtbar; nur einmal je Rückkehr.
- Kein zweiter Filter im Client: gepufferte Bytes sind schon gefiltert.

### Akzeptanzkriterien
1. Präfix und `,` öffnen die Einstellungen in der laufenden Sitzung, `Esc`
   kehrt zurück, und der Agent ist wieder sichtbar und bedienbar.
2. Der Agent verliert keine Ausgabe unterhalb der Puffergrenze.

---

## HUM-546 · Interceptor-Ansicht im Terminal, Glocke bei neuem Halt

### Kontext
Präfix und `i` öffnen eine Ansicht der gehaltenen Anfragen. Im Terminal
von `go` ist `--ask terminal` abgelehnt (`CLI_002`), weil die Fragen das
Terminal des Agenten bräuchten; mit dem Ansichtswechsel (HUM-545) geht das
jetzt ohne Kollision.

### Spezifikation
1. Ansicht aus `Subscribe` und den gehaltenen Flüssen: Liste mit Host,
   Methode, Größe, Befunden, Wartezeit; Auswahl zeigt Kopf und den Anfang
   des Bodys wie `ask_terminal.rs`.
2. Entscheiden über `Decide`: erlauben, blockieren, Regel (die Wege der
   App, nicht mehr).
3. Ein neuer Halt, während der Agent sichtbar ist, schreibt keinen Text in
   dessen Terminal, denn es gehört dem Agenten. Der Client gibt die Glocke
   (ein Byte `BEL`, zwischen zwei Stücken der Ausgabe des Agenten, nie
   mitten in einer Steuerfolge), einstellbar über `terminal.bell_on_hold`,
   Vorgabe `true` (Eigentümer, 2026-11-17). Höchstens eine Glocke je
   Sekunde. Daneben wie heute App und Benachrichtigung des Systems.
4. Mit `--ask terminal` ist `go` weiterhin abgelehnt; die Ansicht ist ein
   Weg zusätzlich zu `--ask ui`.

### Tests
- Fake-Daemon mit zwei gehaltenen Flüssen: Liste, Auswahl, `a` schickt
  `Decide(allow)` für genau den gewählten. Mutationsbeweis: Entscheidung an
  den ersten statt den gewählten färbt den Test rot.
- Ein Fluss, der während der Ansicht von anderswo entschieden wird,
  verschwindet aus der Liste.
- Anzeige säubert Steuerzeichen in Host und Body wie `ask_terminal.rs`.
- Glocke: ein neuer Halt ergibt genau ein `BEL` auf dem Terminal, nie
  innerhalb einer Folge `ESC [ …` der Agentenausgabe; mit
  `terminal.bell_on_hold = false` keines; zehn Halte in einer Sekunde ein
  `BEL`. Mutationsbeweis: `BEL` mitten in einen gepufferten Block
  geschrieben färbt den Test rot.

### Fallstricke
- Hosts und Bodies sind Text des Angreifers; dieselbe Säuberung wie im
  bestehenden Terminal-Kasten, kein zweiter Weg.
- Ein Halt läuft ab, während die Ansicht offen ist; Frist sichtbar.

### Akzeptanzkriterien
1. Gehaltene Anfragen lassen sich im Terminal der Sitzung entscheiden, ohne
   das Terminal des Agenten zu stören.
2. Keine Entscheidung ohne ausdrückliche Taste.

---

## HUM-547 · `humanitl go`: Schließen hängt ab statt zu beenden, ein zweites `go` hängt sich an

### Kontext
Vorgabe 4 und ADR-0024 Punkt 10. Heute beenden `run` und `go` die Sitzung
bei `SIGHUP` und `SIGTERM` (`daemon/bin/humanitl/src/cmd/run.rs`), damit
keine Sandbox ohne Besitzer stehen bleibt. Der Daemon hält das
Pseudoterminal; die Sitzung braucht den Client nicht. Ein zweiter Start
wird mit `CLI_005` abgelehnt. Die App verhält sich schon anders: Schließt
man ihr Fenster oder den Terminalbereich, hängt sie nur ihren Terminalstrom
ab, die Sitzung läuft weiter, und Beenden ist dort eine eigene, bestätigte
Aktion. Entscheidung des Eigentümers vom 2026-11-17 (Frage 7): Abhängen
beim Schließen eines Terminals nur für `humanitl go`; `humanitl run` endet
beim Schließen seines Terminals wie heute, und Sitzungen aus der App
behalten ihr heutiges Verhalten (Schließen lässt sie laufen, sichtbar im
Tray und unter „Laufend" im Startfenster, Beenden bleibt ausdrücklich).

### Spezifikation
1. Schlüssel `terminal.on_close`: `"detach"` (Vorgabe) oder `"stop"`, nur
   für `humanitl go`. Mit `"detach"` hängt `SIGHUP` von `go` ab und die
   Sitzung läuft weiter; `SIGTERM` beendet wie heute. `humanitl run` (auch
   `run -- CMD` am Terminal) beendet bei `SIGHUP` weiter, unabhängig vom
   Schlüssel.
2. Ein `go` in einem Ordner, dessen Sitzung läuft, hängt sich an sie an
   (Splash entfällt, eine Zeile „Attached to the running session in
   <name>"), statt `CLI_005`. Ein `go` in einem anderen Ordner, während eine
   Sitzung läuft, bekommt weiter `CLI_005`, jetzt mit Fix
   `humanitl go` im Ordner der laufenden Sitzung bzw. `humanitl sandbox
   attach`.
3. Endet eine abgehängte Wegwerf-Sitzung, warten die Änderungen wie heute;
   das nächste `go` im Ordner oder das Startfenster fragt danach
   (`changes::review`).
4. Die Moduszeile von `humanitl sessions` und der Eintrag unter „Laufend"
   zeigen, dass eine Sitzung ohne angehängtes Terminal läuft.

### Tests
- Pseudoterminal: `SIGHUP` an `go` mit `"detach"`: Prozess endet, Sitzung
  läuft (Status über den Fake-Daemon), kein `Sandbox(Stop)`. Mit `"stop"`:
  `Sandbox(Stop)`. Mutationsbeweis: Bedingung umgedreht färbt beide rot.
- `SIGHUP` an `run -- bash` mit `terminal.on_close = "detach"`:
  `Sandbox(Stop)` wie heute. Mutationsbeweis: Schlüssel auch für `run`
  ausgewertet färbt den Test rot.
- Zweites `go` im selben Ordner hängt an, kein Start.
- App (Widget-Test mit Fake-Daemon): Schließen des Terminalbereichs und
  des Fensters schickt kein `Sandbox(Stop)`, die Sitzung steht danach unter
  „Laufend"; Beenden nur nach Bestätigung. Mutationsbeweis: `Stop` beim
  Schließen färbt den Test rot.
- `go` in anderem Ordner: `CLI_005` mit dem neuen Fix.
- Terminalzustand nach `SIGHUP`: nicht prüfbar (Terminal weg); stattdessen
  kein hängender Prozess nach 2 s.

### Fallstricke
- Nach `SIGHUP` gibt es kein Terminal mehr; nichts mehr darauf schreiben,
  sonst `EIO` und ein Prozess, der nicht endet.
- Eine abgehängte Sitzung mit gehaltenen Anfragen wartet, bis jemand
  entscheidet oder die Frist abläuft; das steht in der Beschreibung des
  Schlüssels und in `docs/SECURITY.md` nicht als Schwächung, weil die
  Vorgabe der Frist gilt.
- Die Erinnerung „Flock fd inherited by waiters": Kein Kindprozess darf den
  Client nach dem Abhängen am Leben halten.

### Akzeptanzkriterien
1. Ein geschlossenes Terminalfenster beendet einen mit `go` gestarteten
   Agenten nicht (Vorgabe), einstellbar; `run` beendet wie vorher; eine
   Sitzung aus der App läuft nach dem Schließen weiter wie heute und endet
   nur über die ausdrückliche Aktion.
2. App oder ein anderes Terminal hängen sich an die weiterlaufende Sitzung.
3. Wartende Änderungen gehen nicht verloren.

---

## HUM-548 · Doku: `cli.md`, `CONFIG.md`, `UX.md`, Anleitung „Starten"; Nachtrag zu ADR-0024

### Kontext
Abschluss des Sprints. Neue Befehle (`app`, `settings`, `projects`, `go
[AGENT]`), neue Schlüssel (`terminal.prefix` mit Vorgabe `ctrl-b`,
`terminal.direct_keys`, `terminal.on_close`, `terminal.bell_on_hold`),
Startfenster und Erststart.

### Spezifikation
1. `docs/cli.md` aus `cli_docs.rs` neu erzeugt; `docs/CONFIG.md` mit den
   neuen Schlüsseln; `docs/UX.md` mit Startfenster, Erststart, Präfixtabelle
   und Ansichten.
2. Anleitung `docs/guides/getting-started` (Name nach Bestand) mit dem Weg
   „`humanitl go claude`, abhängen, in der App anhängen, aufräumen".
3. ADR-0024 (angenommen mit HUM-530) bekommt einen Nachtrag, wo das
   Gebaute abweicht; Indexzeile in `docs/adr/README.md` nachziehen; `BACKLOG.md` Abschnitt 9 Punkt 24 auf
   den Stand bringen.
4. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` prüfen, dass die Aussagen
   aus HUM-532, HUM-533 und HUM-547 dort stehen.

### Tests
- `scripts/ci/lint-docs.sh` und `docs/adr/check.sh` grün; Test der
  CLI-Doku (`cli_docs.rs`) grün.

### Fallstricke
- Website-Übersetzungen unter `docs/translations/` nicht vergessen, wenn
  geänderte Seiten dort eine Fassung haben (HUM-346).

### Akzeptanzkriterien
1. Jeder neue Befehl und Schlüssel ist dokumentiert.
2. ADR-0024 beschreibt das Gebaute.

---

## HUM-549 · Kandidat, nicht in diesem Sprint: mehrere Sitzungen je Daemon

### Kontext
Frage 9 des ADR-0024: Der Eigentümer will mehrere Sitzungen zugleich
(Vision vom 2026-10-20, „wie screen"), aber später, in einem eigenen Issue
(Entscheidung vom 2026-11-17). Heute führt der Daemon genau eine Sitzung;
ein zweiter Start ist `CLI_005` (`daemon/crates/ipc/src/sandbox.rs`).
Register, Startfenster, RPC `Projects` und `go` sind in diesem Sprint schon
für mehrere ausgelegt. Dieses Issue ist ein Kandidat und wird erst
ausgearbeitet, wenn der Eigentümer es einplant.

### Spezifikation
Vor der Einplanung auszuarbeiten, mindestens:
1. Sitzungen je Projekt im Daemon (Zustand, Halte, Terminal, Wegwerf-Schicht
   je Sitzung statt einer), `CLI_005` nur noch für denselben Ordner.
2. Proxy und Regeln: Zuordnung jeder Anfrage zu ihrer Sitzung (Tür je
   Sitzung), Sitzungsregeln je Sitzung.
3. Grenzen für Speicher, Prozesse und Anzahl der Sitzungen.
4. Benannte Sitzungen in der CLI (`humanitl sessions ls`, `attach NAME`).

### Tests
Werden mit der Spezifikation festgelegt; mindestens: zwei Sitzungen in zwei
Ordnern, jede Anfrage landet in der Aufzeichnung der richtigen, die
Escape-Suite je Sitzung grün.

### Fallstricke
- Eine Anfrage, die der falschen Sitzung zugeordnet wird, bekäme deren
  Regeln; die Zuordnung darf nur aus der Tür kommen, nie aus Angaben der
  Sandbox.

### Akzeptanzkriterien
Werden mit der Spezifikation festgelegt.
