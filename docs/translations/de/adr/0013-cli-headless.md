---
source: adr/0013-cli-headless.md
source-sha256: dd088dd54bb7282cf3df4b50fbad8a6629f56315e1141230650130966fd80892
lang: de
---
# ADR-0013 · Kommandozeile als gleichrangiger Client und Betrieb ohne Oberfläche

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Der naheliegende Entwurf für Humanitl ist eine Anwendung mit Oberfläche, die nebenbei
einen Proxy startet. Dieser Entwurf hat drei Probleme.

Erstens: Ein Lauf eines Agenten dauert Stunden. Wer die Oberfläche schließt oder wessen
Sitzung abstürzt, darf den Agenten nicht mit sich reißen. Zweitens: Es gibt legitime
Einsätze ohne Bildschirm, etwa ein Skript, das einen Agenten in einem Verzeichnis
laufen lässt, oder eine reine Inferenz-Instanz, die nur den LLM-Verkehr durchlässt.
Drittens: Ist die Oberfläche der einzige Weg, wird sie zum Türsteher, und jede Fähigkeit
muss dort gebaut werden, bevor es sie gibt (Prinzip 10: „CLI is first-class“).

Die schwierige Frage ist die angehaltene Anfrage ohne Oberfläche. Anhalten ergibt nur
Sinn, wenn jemand entscheidet. Ohne Oberfläche muss es entweder einen anderen
menschlichen Kanal geben oder eine ehrliche Vorgabe.

## Entscheidung

Es gibt zwei Binaries: `humanitld` (Daemon) und `humanitl` (Kommandozeile). Beide sind
gleichrangig; die Oberfläche und die Kommandozeile sind gleichberechtigte gRPC-Clients
desselben Daemons (ADR-0003).

Unterkommandos:

```
humanitl run [--profile NAME] [--work DIR] [--ask ui|terminal|none] [--llm URL] [-- CMD...]
humanitl sandbox run [--profile NAME] -- CMD...
humanitl sandbox argv [--profile NAME]
humanitl sandbox check
humanitl rules list|add|remove|test URL [--json]
humanitl flows list [FILTER] | show ID [--json]
humanitl audit verify|export [--format jsonl|csv] [--out FILE]
humanitl config get KEY | set KEY VALUE | schema | edit
humanitl daemon install|status|logs
```

Exit-Codes, verbindlich: `0` in Ordnung, `1` Fehler des Nutzers (mit Diagnostic), `2`
Daemon nicht erreichbar, `3` Sandbox-Prüfung fehlgeschlagen, `4` Sicherheitsverletzung
(etwa ein Authority-Mismatch im Test), `10` `rules test` ergibt `block`, `11`
`rules test` ergibt `ask`.

**Zwei Modi für angehaltene Anfragen ohne verbundene Oberfläche:**

- `--ask terminal`: eine Abfrage im Terminal nach dem Muster von `pipelock`: Host,
  Methode, Größe, Funde, dann `[a]llow [b]lock [r]ule`.
- `--ask none`: Alles, für das keine Regel gilt, wird blockiert.

`--ask terminal` funktioniert nur für zeilenorientierte Befehle
(`humanitl sandbox run -- curl …`, Skripte, Aider im Grundmodus). Bei einem
Vollbild-TUI-Agenten wie OpenCode verweigert `humanitl run --ask terminal` den Start mit
dem Diagnostic `CLI_002` und schlägt `--ask ui` (die Oberfläche wird gestartet oder
angebunden) oder `--ask none` vor. Die Erkennung läuft über
`AgentAdapter::is_fullscreen_tui()`.

Das mitgelieferte Profil `llm-only` setzt `--ask none` und dazu die Regel für die
LLM-Durchleitung. Das ist die Instanz „nur Inferenz“.

**Übernahme durch die Oberfläche.** Verbindet sich später eine Oberfläche, übernimmt sie
die Warteschlange der angehaltenen Anfragen nahtlos, weil beide nur gRPC-Clients des
Daemons sind. Der Zustand lebt im Daemon, nicht im Client.

## Begründung

Dass die Warteschlange der angehaltenen Anfragen im Daemon lebt und nicht im Client, ist
die Entscheidung, aus der alles andere folgt. Der Daemon hält die Anfrage an, der Client
zeigt sie an. Ein Client kann kommen und gehen; die Anfrage bleibt angehalten, bis über
sie entschieden ist oder die Frist abläuft. Deshalb ist „die Oberfläche schließen“
harmlos, und deshalb kann eine Oberfläche eine laufende Terminal-Session übernehmen,
ohne dass dafür ein Übergabemechanismus nötig wäre.

Die beiden Modi des Nachfragens sind die beiden ehrlichen Antworten auf „niemand sieht
hin“. `terminal` ist der Mensch an einem anderen Ort. `none` ist die klare Aussage: Ohne
Regel geht nichts durch. Ein dritter Modus „alles durchlassen“ wäre die Abschaffung des
Produkts und existiert nicht.

`--ask terminal` für Vollbild-TUI-Agenten zu verweigern, ist eine Entscheidung gegen
einen kaputten Zustand. OpenCode zeichnet auf denselben Bildschirm, auf dem die Abfrage
erscheinen müsste; das Ergebnis wäre ein zerschossenes Terminal und eine Frage, die der
Nutzer nicht lesen kann. Besser ein sauberes Scheitern mit zwei genannten Auswegen als
ein unbrauchbarer Zustand.

Die festen Exit-Codes machen die Kommandozeile skriptbar. `10` und `11` für
`rules test` sind bewusst getrennt: Ein Skript kann `block` und `ask` unterscheiden, ohne
die Ausgabe zu parsen. `4` für eine Sicherheitsverletzung ist absichtlich ein eigener
Code, damit eine CI-Pipeline darauf anhalten kann.

`llm-only` als mitgeliefertes Profil zeigt, dass diese Betriebsart kein Sonderfall ist,
sondern eine Konfiguration: ein Agent, der ausschließlich mit dem lokalen Modell spricht,
vollständig aufgezeichnet, ohne Internet und ohne jemanden, der zusehen muss.

## Verworfene Alternativen

- **Nur eine Anwendung mit Oberfläche.** Verliert alle drei Eigenschaften aus dem
  Kontext: kein Weiterleben über die Oberfläche hinaus, kein Betrieb ohne Oberfläche,
  Oberfläche als Türsteher.
- **Kommandozeile als dünne Hülle um Unterkommandos von `humanitld`.** Hätte zwei Wege
  in den Kern geschaffen, einen über gRPC und einen direkten. Genau das schließt
  ADR-0018 aus.
- **Im Terminal über ein eigenes TTY oder ein zweites Fenster fragen.** Löst das
  Vollbildproblem technisch und schafft einen Kanal, den der Nutzer nicht erwartet.
  `CLI_002` mit zwei genannten Auswegen ist verständlicher.
- **`--ask none` als Vorgabe für alle Läufe ohne Oberfläche.** Wäre sicher, macht aber
  Skripte mit `curl` unbrauchbar, neben denen ein Mensch sitzt.
- **Ein Modus „alles erlauben“ für automatisierte Läufe.** Ausdrücklich verworfen: Er
  schafft das Produkt ab. Wer alles erlauben will, braucht Humanitl nicht.
- **Interaktive Entscheidungen über eine Datei oder eine Named Pipe.** Wäre skriptbar,
  aber niemand will eine angehaltene Anfrage per `echo` freigeben. Für Automatisierung
  sind Regeln der richtige Mechanismus.

## Konsequenzen

- Der Zustand einer Session lebt im Daemon. Clients sind zustandslos genug, um jederzeit
  zu verschwinden und wiederzukommen.
- Jede Fähigkeit braucht ein Unterkommando der Kommandozeile, auch wenn es nur `--json`
  ausgibt. Diese Regel ist so wichtig, dass sie ein eigenes ADR hat: ADR-0018.
- Die Exit-Codes sind öffentliches Verhalten und mit Regressionstests abgesichert. Die
  Escape-Tests und die Demo-Skripte nutzen die Kommandozeile statt eigens geschriebener
  Skripte und sind damit zugleich ihr Test.
- `AgentAdapter` braucht `is_fullscreen_tui()`, damit `--ask terminal` seine
  Vorbedingung prüfen kann.
- `humanitl run` ist ohne Flags der Standardweg: aktuelles Verzeichnis als `/work`,
  gefundener LLM-Server, OpenCode aus dem `PATH`.
- Die Flags der Kommandozeile werden aus dem Konfigurationsschema erzeugt (ADR-0011),
  damit Flag-Name und Konfigurationsschlüssel nie auseinanderlaufen.

## Betroffene Issues

`HUM-064` (Gerüst der Kommandozeile mit clap und den Sandbox-Unterkommandos), `HUM-067`
(`humanitl run` mit `--ask terminal`/`--ask none` und Übernahme durch die Oberfläche),
`HUM-065` (`humanitl rules|flows`), `HUM-070` (`humanitl
config|audit|daemon`), `HUM-066` (Profile `default` und `llm-only`),
`HUM-075` (`humanitl doctor --json`).
