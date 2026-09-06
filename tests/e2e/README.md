# Die Demoskripte der Meilensteine

Jeder Meilenstein endet mit einem Demoskript, das in CI grün läuft, und jedes
bleibt stehen, wenn das nächste dazukommt: Ein Sprint gilt erst als fertig,
wenn alle bisherigen Demos grün sind (`BACKLOG.md` 8, `CONTRIBUTING.md`
„Sprint gate"). Hier steht, wie man sie ausführt, was sie belegen und was
nicht.

```
./tests/e2e/run.sh                      # alle drei, mit Bauen
E2E_SKIP_BUILD=1 ./tests/e2e/run.sh     # die Binaries nehmen, wie sie sind
E2E_ONLY=m1 ./tests/e2e/run.sh          # nur die versiegelte Kiste
E2E_ONLY=m2 ./tests/e2e/run.sh          # nur die erste Entscheidung
E2E_ONLY=m3 ./tests/e2e/run.sh          # nur den Agenten in der Sandbox
E2E_TRACE=1  ./tests/e2e/run.sh         # zusätzlich `set -x` (die CI setzt es)
```

Ein Demo lässt sich auch direkt aufrufen (`./tests/e2e/m3_agent_inside/run.sh`)
und verhält sich dann genau wie in der CI. Der Einstieg tut selbst nichts
weiter, als sie der Reihe nach zu starten und nach dem ersten Lauf das Bauen
abzuschalten.

Voraussetzungen auf der Maschine: `bubblewrap`, `curl`, `jq`, `python3`,
`iproute2`, `util-linux`, `openssl`, dazu unprivilegierte Nutzer-Namensräume
(auf Ubuntu 24.04 `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`).
Fehlt etwas, sagt der Lauf welches Werkzeug und bricht ab; er hängt nicht.

## Die drei Demos

| Demo | Skript | Was es belegt |
|---|---|---|
| M1 | `m1_sealed_box.sh` | Die Kiste ist dicht, eine einzelne Entscheidung wirkt, ein Neustart verliert nichts |
| M2 | `m2_first_decision/run.sh` | Ein voller Arbeitsabschnitt eines Menschen: Gruppierung, Stapel-Freigabe mit Sitzungsregel, Block, Zeitüberschreitung, Historie, der MITM-Pfad über TLS |
| M3 | `m3_agent_inside/run.sh` | Der Agent selbst: `humanitl run`, die Durchreiche zum Sprachmodell, die mitgelieferten Regeln, eine Entscheidung über gRPC, das Terminal mit seinen Hinweiszeilen, die Zusammenfassung des Laufs |

Jedes Demo schreibt seine Artefakte in ein eigenes Verzeichnis unter
`target/e2e/` (`m1`, `m2`, `m3`) und räumt vor dem Lauf nur sein eigenes leer.
Die CI lädt `target/e2e` als Ganzes hoch.

Jedes Demo läuft in einem eigenen Nutzer- und Netz-Namensraum. Der Grund ist
in beiden Richtungen derselbe: Das Ziel braucht eine Adresse, die der Proxy
erreichen darf — `198.51.100.7` aus TEST-NET-2, also keine private, denn eine
private wiese der Proxy ab —, und der Namensraum hat keine Route nach draußen,
der Lauf also kein Netz. Was der Lauf anlegt, liegt vollständig unter einem
Wegwerf-Baum in `/tmp` und verschwindet mit ihm, auch nach einem Abbruch.

## Die Zahl der Zusicherungen

M2 und M3 tragen eine feste Zahl (`M2_EXPECTED_ASSERTIONS`,
`M3_EXPECTED_ASSERTIONS`) und scheitern, wenn weniger Behauptungen liefen als
erwartet: Ein Demoskript, das grün ist, weil ein Zweig übersprungen wurde, ist
schlimmer als keines. Wer eine Zusicherung hinzufügt, zieht die Zahl mit; der
Lauf sagt am Ende, wenn sie zu klein geworden ist.

Ein übersprungener Zweig meldet sich in M3 zusätzlich in der letzten Zeile
(`M3 demo: OK with gaps — …`). Ein Lauf ohne echtes OpenCode ist damit von
einem vollständigen unterscheidbar, ohne dass jemand das Protokoll liest.

## Was in diesem Verzeichnis liegt

| Pfad | Wofür |
|---|---|
| `run.sh` | Der Einstieg; fährt die Demos der Reihe nach |
| `lib.sh` | Die gemeinsamen Helfer: Namensraum, Wegwerf-Baum, Daemon, Kommandozeile, Sandbox, Zusicherungszähler |
| `m1_sealed_box.sh` | Das Demo von M1 |
| `m2_first_decision/` | Das Demo von M2, mit seiner `config.toml` und dem Drehbuch `script.json` |
| `m3_agent_inside/` | Das Demo von M3, mit seiner `config.toml` und den beiden Agentenskripten |
| `fake_upstream.py` | Das Ziel von M1 |
| `fake-upstream/` | Das Ziel von M2 und M3, im Klartext und über TLS, mit `gen-test-ca.sh` |
| `fake-agent/` | Der Agent von M2: arbeitet ein Drehbuch aus Zeitpunkten ab |
| `mock_llm/` | Das Sprachmodell von M3, mit seinem eigenen Test |

Die Ziele und die Agenten sind Python und keine Rust-Binaries. Der Grund steht
in `backlog/CONVENTIONS.md` 4.22: Für einen Server, der zurückmeldet, wonach
gefragt wurde, wäre eine eigene Crate mehr Bauzeit als Nutzen, und `python3`
liegt auf Debian wie auf `ubuntu-latest` bereit.

## Das Sprachmodell von M3

`mock_llm/mock_llm.py` bedient die Endpunkte, über die ein Coding-Agent mit
einem lokalen Sprachmodell spricht, und **streamt** dabei wirklich: zehn
SSE-Rahmen im Abstand von 30 ms, jeder mit eigenem `flush`. Der M3-Lauf misst
den Abstand zwischen dem ersten und dem letzten Byte beim Agenten; ein Proxy,
der den Strom sammelte, fiele damit auf.

Ein Modell, das er nicht bedient, bekommt `404` und keine Antwort; `--model`
sagt, welches er hat (Vorgabe `mock`). Ein Testdoppel, das jede Anfrage
freundlich beantwortet, verdeckt genau den Fehler, für den man es hat.

Er trägt das ganze Milestone, also hat er einen eigenen Test:

```
tests/e2e/mock_llm/self_test.sh
```

Er braucht weder Daemon noch Sandbox noch Namensraum und läuft in einer
Sekunde. `tests/e2e/run.sh` fährt ihn als eigenen Schritt vor M3, damit ein
roter M3-Lauf von einem kaputten Testdouble unterscheidbar ist.

## Was die Demos nicht belegen

Ein Gate ist nur so viel wert, wie ein späterer Leser über seine Reichweite
weiß. Der Kopf jedes Skripts zählt das im Einzelnen auf; in kurz:

- **Der Bildschirm fehlt.** M2 hat seine Oberflächen-Hälfte noch nicht
  (HUM-097), M3 ebenso wenig. Ein grünes `e2e-xvfb` heißt „die Daemon-Hälfte
  von M2 hält", nicht „M2 hält".
- **Die Audit-Kette fehlt** (HUM-070). M3 hält das mit einem Stolperdraht
  fest, der rot wird, sobald es sie gibt.
- **Der Durchreich-Fluss ist in der Liste unsichtbar.** `humanitl flows list`
  hat keinen Schalter für `include_passthrough`; M3 misst die Durchreiche
  deshalb an dem, was der Agent bekam, an dem, was der Mock empfing, und an
  ihrer Abwesenheit in der Liste. Auch das trägt einen Stolperdraht.
- **Die OpenCode-Variante läuft nur dort, wo die Sandbox das Binary sieht**,
  also unter `/usr/local/bin`, `/usr/bin` oder `/bin` — das ist der ganze
  `PATH` der Sandbox. Ein `opencode` unter `~/.local/bin` zählt nicht, und der
  Lauf sagt das, statt drinnen an etwas anderem zu scheitern. `M3_OPENCODE=1`
  macht aus dem Überspringen einen Fehlschlag.

## Die CI-Jobs

| Job | Fährt |
|---|---|
| `e2e` | `E2E_ONLY=m1` |
| `e2e-xvfb` | `E2E_ONLY=m2` (Flutter und xvfb für die spätere Oberflächen-Hälfte) |
| `e2e-agent` | `E2E_ONLY=m3` |

Alle drei laden `target/e2e` als Artefakt hoch: Daemon-Protokoll, Transkript
des Agenten, Zugriffsprotokolle der Ziele und die Flow-Liste des Laufs.
