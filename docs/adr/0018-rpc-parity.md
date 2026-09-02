# ADR-0018 · Parität: Jede Fähigkeit ist zuerst ein RPC, UI und CLI sind dünne Clients
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl hat zwei Bedienoberflächen: eine grafische und eine auf der
Kommandozeile (ADR-0013). Beide sind gRPC-Clients desselben Daemons (ADR-0003).
Diese Anordnung zerfällt vorhersehbar, wenn man sie nicht aktiv hält.

Der übliche Verlauf: Eine Funktion wird im UI gebraucht und dort gebaut, weil es
schneller geht — ein Filter, eine Sortierung, eine Vorschau, eine kleine
Umrechnung. Sechs Monate später enthält das UI Fachlogik, die CLI kann die Hälfte
nicht, und ein Automatisierungsversuch scheitert an einer Funktion, die es nur
mit Maus gibt. Ab da ist das UI der Gatekeeper, den Prinzip 10 ausschließt.

Der zweite Verlauf ist die stille Divergenz: Dieselbe Fähigkeit existiert in
beiden Oberflächen, aber mit unterschiedlichen Namen, unterschiedlichen Defaults
und leicht unterschiedlichem Verhalten. Beides ist mit Disziplin allein nicht zu
verhindern.

## Entscheidung

**Es gibt genau eine Schnittstelle zum Kern: die gRPC-Proto `humanitl.v1`.**
Jede Fähigkeit — Entscheiden, Regeln, Sandbox, Terminal, Doctor, Discovery,
Konfiguration, Audit, Export — wird zuerst als RPC entworfen und im Daemon
implementiert. UI und CLI rufen ausschließlich RPCs auf und enthalten **keine
Fachlogik**.

**Reihenfolge und Mitlieferpflicht.** In der Umsetzung wird GUI-first gearbeitet,
aber: Jedes Issue, das einen neuen RPC einführt, liefert im selben Issue das
CLI-Subkommando mit — und sei es nur eine `--json`-Ausgabe. Umgekehrt bekommt
jedes CLI-Subkommando spätestens im Folgesprint eine Entsprechung in der
Oberfläche.

**Paritäts-Tabelle, generiert und geprüft.** `docs/reference/parity.md` wird von
`cargo xtask docs` aus drei Quellen erzeugt: den Service-Methoden der Proto, der
clap-Struktur der CLI (Subkommandos mit dem Attribut
`#[humanitl(rpc = "…")]`) und einer UI-Registry (`app/lib/core/parity.dart`,
Liste RPC → Bildschirm). Sie hat drei Spalten: RPC, CLI-Subkommando, UI-Ort.
Der CI-Job `parity-check` **schlägt fehl**, wenn ein RPC ohne CLI-Zeile
existiert; fehlende UI-Entsprechungen werden als `warn` gelistet.

**Gute Defaults sind Teil der Parität.** Das mitgelieferte Profil `default`
beschreibt den gängigen Anwendungsfall vollständig: OpenCode, LLM im LAN,
Ask-Modus `ask`, fünf Minuten Timeout, gebündelte Block-Regeln,
Session-Regeln bevorzugt. Wer nichts ändert, bekommt diesen Weg; wer etwas ändern
will, findet jeden Wert unter demselben Schlüssel im Profil, im
Einstellungsbildschirm und in der CLI (ADR-0011).

## Begründung

Die asymmetrische Härte der Prüfung ist Absicht. Ein RPC ohne CLI-Subkommando
bricht den Build; eine fehlende UI-Entsprechung ist nur eine Warnung. Der Grund:
Der teure Fehler ist Fachlogik, die nur über die Oberfläche erreichbar ist —
denn sie ist dann nicht skriptbar, nicht testbar ohne Bildschirm und nicht
automatisierbar. Eine fehlende UI-Entsprechung ist dagegen bloß eine
Unbequemlichkeit, die der nächste Sprint behebt.

Dass die Tabelle **generiert** wird und nicht gepflegt, ist der Unterschied
zwischen einer Regel und einer Absichtserklärung. Eine handgepflegte
Paritäts-Tabelle wäre nach dem dritten Sprint unvollständig, und niemand würde es
merken. Die Erzeugung aus Proto, clap-Struktur und UI-Registry zieht die Wahrheit
aus dem Code; die Registry auf der Dart-Seite ist der einzige handgepflegte Teil
und klein genug, um vollständig zu bleiben.

„Zuerst als RPC entwerfen" ist außerdem ein Entwurfswerkzeug, nicht nur eine
Auflage. Wer eine Fähigkeit als RPC formuliert, muss ihre Ein- und Ausgaben
benennen, bevor er ein Formular baut. Das fördert Fähigkeiten, die klein und
zusammensetzbar sind, statt Bildschirme, die alles gleichzeitig tun.

Die Aufnahme der Defaults in die Parität schließt eine Lücke, die man leicht
übersieht: Zwei Oberflächen können dieselben RPCs aufrufen und trotzdem
unterschiedlich starten, wenn jede ihre eigenen Vorgaben mitbringt. Das
mitgelieferte Profil `default` ist die eine Quelle für den Standardweg; beide
Oberflächen laden es, keine erfindet eigene Vorgaben.

## Verworfene Alternativen

- **Parität als Konvention ohne CI-Prüfung.** Hält bis zum ersten Termindruck.
  Dieselbe Überlegung wie in ADR-0015 für die Abhängigkeitsrichtung.
- **CLI aus der Proto generieren.** Klingt konsequent und ergibt eine
  unbenutzbare Kommandozeile: RPC-Grenzen sind nicht Subkommando-Grenzen, und
  eine gute CLI braucht Kurzformen, Vorgaben und aufgabenorientierte
  Zusammenfassungen. Deshalb wird die Zuordnung annotiert (`#[humanitl(rpc =
  "…")]`) statt erzeugt.
- **CLI-first statt GUI-first.** Hätte die Parität ebenso gesichert, aber die
  Produktentscheidungen fallen an den Bildschirmen. GUI-first mit
  Mitlieferpflicht bekommt beides.
- **Nur die CLI, kein UI.** Widerspricht der Zielgruppe: Professionelle ohne
  Security-Hintergrund, die live neben dem Agenten sitzen und Entscheidungen in
  Sekunden treffen sollen.
- **Fachlogik im UI erlauben, wenn sie klein ist.** Der Anfang jeder Divergenz.
  Die Grenze „klein" ist nicht verteidigbar, deshalb gibt es sie nicht.
- **Paritäts-Tabelle nur als Dokumentation ohne CI-Job.** Wäre nach drei Sprints
  falsch und würde dann aktiv in die Irre führen.

## Konsequenzen

- Der Aufwand pro Fähigkeit steigt: RPC, Daemon-Implementierung und
  CLI-Subkommando im selben Issue. Der Gewinn ist eine automatisierbare Anwendung
  und ein Daemon, der ohne Bildschirm vollständig testbar ist.
- `cargo xtask docs` gehört zur Werkzeugkette und erzeugt `parity.md`. Die
  xtask-Crate ist ein Hilfswerkzeug außerhalb der Abhängigkeitsregeln und
  enthält nie Laufzeitcode (`backlog/CONVENTIONS.md` 3.1).
- Das CLI-Attribut `#[humanitl(rpc = "…")]` an den clap-Subkommandos ist Pflicht;
  ohne es taucht das Subkommando nicht in der Tabelle auf.
- `app/lib/core/parity.dart` ist eine bewusst handgepflegte Liste RPC →
  Bildschirm. Sie ist der einzige Ort, an dem die Zuordnung steht.
- Fachlogik in `app/` oder in `daemon/bin/humanitl` ist ein Architekturverstoß,
  auch wenn sie klein ist (`docs/ARCHITECTURE.md` 3b).
- Der Fake-Daemon (HUM-005) implementiert dieselbe Proto und profitiert damit
  automatisch von jeder neuen Fähigkeit — er ist ein dritter Client desselben
  Vertrags.
- Diese Entscheidung verschärft ADR-0013: Dort ist die CLI ein gleichwertiger
  Client, hier wird die Gleichwertigkeit maschinell geprüft.

## Betroffene Issues

`HUM-078` (Paritäts-Tabelle und CI-Job `parity-check`), `HUM-003` (Proto als
einzige Schnittstelle), `HUM-064` (CLI-Grundgerüst mit RPC-Attributen),
`HUM-065` (`rules`/`flows` als CLI-Entsprechung), `HUM-070`
(`config`/`audit`/`daemon`), `HUM-066` (mitgeliefertes Profil `default`).
