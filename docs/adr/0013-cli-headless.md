# ADR-0013 · CLI als gleichwertiger Client und Headless-Betrieb
Status: Accepted
Datum: 2026-09-02

## Kontext

Der naheliegende Entwurf für Humanitl ist eine Anwendung mit Oberfläche, die
nebenbei einen Proxy startet. Dieser Entwurf hat drei Probleme.

Erstens: Ein Agentenlauf dauert Stunden. Wer die Oberfläche schließt oder wessen
Sitzung abstürzt, darf den Agenten nicht mitreißen. Zweitens: Es gibt legitime
Einsätze ohne Bildschirm — ein Skript, das einen Agenten in einem Verzeichnis
laufen lässt, oder eine reine Inferenz-Instanz, die nur den LLM-Verkehr
durchlässt. Drittens: Wenn die Oberfläche der einzige Weg ist, wird sie zum
Gatekeeper, und jede Fähigkeit muss dort gebaut werden, bevor sie existiert
(Prinzip 10: „CLI ist erstklassig").

Die schwierige Frage ist der angehaltene Request ohne Oberfläche. Halten ergibt
nur Sinn, wenn jemand entscheidet. Ohne Oberfläche muss es entweder einen
anderen Menschen-Kanal geben oder eine ehrliche Vorgabe.

## Entscheidung

Es gibt zwei Binaries: `humanitld` (Daemon) und `humanitl` (CLI). Beide sind
gleichrangig; die Oberfläche und die CLI sind gleichwertige gRPC-Clients
desselben Daemons (ADR-0003).

Subkommandos:

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

Exit-Codes, verbindlich: `0` ok, `1` Nutzerfehler (mit Diagnostic), `2` Daemon
nicht erreichbar, `3` Sandbox-Check fehlgeschlagen, `4` Sicherheitsverletzung
(etwa Authority-Mismatch im Test), `10` `rules test` ergibt `block`, `11`
`rules test` ergibt `ask`.

**Zwei Modi für gehaltene Requests ohne verbundene Oberfläche:**

- `--ask terminal` — Eingabeaufforderung im Terminal nach dem Muster von
  `pipelock`: Host, Methode, Größe, Findings, dann `[a]llow [b]lock [r]ule`.
- `--ask none` — alles, wofür keine Regel gilt, wird geblockt.

`--ask terminal` funktioniert nur für zeilenorientierte Kommandos
(`humanitl sandbox run -- curl …`, Skripte, Aider im Basic-Mode). Bei einem
Vollbild-TUI-Agenten wie OpenCode verweigert `humanitl run --ask terminal` den
Start mit Diagnostic `CLI_002` und schlägt `--ask ui` (Oberfläche wird gestartet
oder angehängt) oder `--ask none` vor. Die Erkennung läuft über
`AgentAdapter::is_fullscreen_tui()`.

Das mitgelieferte Profil `llm-only` setzt `--ask none` plus die
LLM-Passthrough-Regel. Das ist die „nur Inferenz"-Instanz.

**Übernahme durch die Oberfläche.** Verbindet sich später eine Oberfläche,
übernimmt sie die Hold-Queue nahtlos, weil beide nur gRPC-Clients des Daemons
sind. Der Zustand liegt im Daemon, nicht im Client.

## Begründung

Dass die Hold-Queue im Daemon lebt und nicht im Client, ist die Entscheidung, aus
der alles andere folgt. Der Daemon hält den Request, der Client zeigt ihn an. Ein
Client kann kommen und gehen; der Request bleibt gehalten, bis er entschieden ist
oder die Frist abläuft. Deshalb ist „Oberfläche schließen" harmlos und deshalb
kann eine Oberfläche eine laufende Terminal-Sitzung übernehmen, ohne dass dafür
ein Übergabemechanismus nötig wäre.

Die zwei Ask-Modi sind die zwei ehrlichen Antworten auf „niemand schaut hin".
`terminal` ist der Mensch an einer anderen Stelle. `none` ist die klare Ansage:
Ohne Regel geht nichts durch. Ein dritter Modus „alles durchlassen" wäre die
Aufhebung des Produkts und existiert nicht.

Die Verweigerung von `--ask terminal` bei Vollbild-TUI-Agenten ist eine
Entscheidung gegen einen kaputten Zustand. OpenCode zeichnet auf denselben
Bildschirm, auf dem die Eingabeaufforderung erscheinen müsste; das Ergebnis wäre
ein zerstörtes Terminal und eine Frage, die der Nutzer nicht lesen kann. Lieber
ein sauberer Fehlschlag mit zwei benannten Auswegen als ein unbrauchbarer
Zustand.

Die festgelegten Exit-Codes machen die CLI skriptfähig. `10` und `11` für
`rules test` sind bewusst getrennt: Ein Skript kann `block` und `ask`
unterscheiden, ohne die Ausgabe zu parsen. `4` für eine Sicherheitsverletzung ist
absichtlich ein eigener Code, damit eine CI-Pipeline daran anhalten kann.

`llm-only` als mitgeliefertes Profil zeigt, dass diese Betriebsart kein Sonderfall
ist, sondern eine Konfiguration: ein Agent, der ausschließlich mit dem lokalen
Modell spricht, vollständig aufgezeichnet, ohne Internet und ohne jemanden, der
zuschauen muss.

## Verworfene Alternativen

- **Nur eine Anwendung mit Oberfläche.** Verliert alle drei Eigenschaften aus dem
  Kontext: keine Überlebensfähigkeit über die Oberfläche hinaus, kein
  Headless-Betrieb, Oberfläche als Gatekeeper.
- **CLI als dünner Wrapper um `humanitld`-Unterkommandos.** Hätte zwei Wege in
  den Kern erzeugt, einen über gRPC und einen direkt. Genau das schließt
  ADR-0018 aus.
- **Ask im Terminal über eine separate TTY oder ein zweites Fenster.** Löst das
  Vollbild-Problem technisch und erzeugt einen Kanal, den der Nutzer nicht
  erwartet. `CLI_002` mit zwei benannten Auswegen ist verständlicher.
- **`--ask none` als Vorgabe für alle Headless-Läufe.** Wäre sicher, macht aber
  Skripte mit `curl` unbrauchbar, in denen ein Mensch danebensitzt.
- **Ein Modus „alles erlauben" für automatisierte Läufe.** Ausdrücklich
  verworfen: Er hebt das Produkt auf. Wer alles erlauben will, braucht Humanitl
  nicht.
- **Interaktive Entscheidungen über eine Datei oder einen Named Pipe.** Wäre
  skriptbar, aber niemand will einen angehaltenen Request per `echo` freigeben.
  Für Automatisierung sind Regeln der richtige Mechanismus.

## Konsequenzen

- Der Zustand einer Sitzung lebt im Daemon. Clients sind zustandslos genug, um
  jederzeit zu verschwinden und wiederzukommen.
- Jede Fähigkeit braucht ein CLI-Subkommando, auch wenn es nur `--json` ausgibt.
  Diese Regel ist so wichtig, dass sie einen eigenen ADR hat: ADR-0018.
- Die Exit-Codes sind öffentliches Verhalten und regressionsgetestet. Die
  Escape-Tests und die Demo-Skripte benutzen die CLI statt Ad-hoc-Skripten und
  sind damit gleichzeitig ihr Test.
- `AgentAdapter` braucht `is_fullscreen_tui()`, damit `--ask terminal` seine
  Vorbedingung prüfen kann.
- `humanitl run` ist der Standardweg ohne Flags: aktuelles Verzeichnis als
  `/work`, gefundener LLM-Server, OpenCode aus dem `PATH`.
- Die CLI-Flags werden aus dem Config-Schema erzeugt (ADR-0011), damit
  Flag-Name und Konfigurationsschlüssel nie auseinanderlaufen.

## Betroffene Issues

`HUM-064` (CLI-Grundgerüst mit clap und Sandbox-Unterkommandos), `HUM-067`
(`humanitl run` mit `--ask terminal`/`--ask none` und Übernahme durch die
Oberfläche), `HUM-065` (`humanitl rules|flows`), `HUM-070` (`humanitl
config|audit|daemon`), `HUM-066` (Profile `default` und `llm-only`),
`HUM-075` (`humanitl doctor --json`).
