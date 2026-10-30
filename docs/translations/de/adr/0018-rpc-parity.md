---
source: adr/0018-rpc-parity.md
source-sha256: 3f0dc46af8323d8b2f074d43a730d5a20e0fcfd5c70e4799f4b5ac68c85dbfe4
lang: de
---
# ADR-0018 · Parität: Jede Fähigkeit ist zuerst ein RPC, Oberfläche und Kommandozeile sind dünne Clients

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl hat zwei Bedienoberflächen: eine grafische und eine auf der Kommandozeile
(ADR-0013). Beide sind gRPC-Clients desselben Daemons (ADR-0003). Diese Anordnung
zerfällt erwartbar, wenn niemand sie aktiv pflegt.

Der übliche Verlauf: Eine Funktion wird in der Oberfläche gebraucht und dort gebaut, weil
es schneller geht, etwa ein Filter, eine Sortierung, eine Vorschau, eine kleine Umrechnung.
Ein halbes Jahr später enthält die Oberfläche Fachlogik, die Kommandozeile kann die Hälfte
davon nicht, und ein Versuch der Automatisierung scheitert an einer Funktion, die es nur
mit Maus gibt. Ab dann ist die Oberfläche der Türsteher, den Prinzip 10 ausschließt.

Der zweite Verlauf ist das stille Auseinanderlaufen: Dieselbe Fähigkeit gibt es in beiden
Oberflächen, aber mit anderen Namen, anderen Vorgaben und leicht anderem Verhalten.
Beides lässt sich mit Disziplin allein nicht verhindern.

## Entscheidung

**Es gibt genau eine Schnittstelle zum Kern: das gRPC-Proto `humanitl.v1`.**
Jede Fähigkeit, also Entscheiden, Regeln, Sandbox, Terminal, Doctor, Erkennung,
Konfiguration, Audit und Export, wird zuerst als RPC entworfen und im Daemon umgesetzt.
Oberfläche und Kommandozeile rufen ausschließlich RPCs auf und enthalten **keine Fachlogik**.

**Reihenfolge und Mitlieferpflicht.** Die Umsetzung arbeitet GUI-first, aber: Jedes Issue,
das einen neuen RPC einführt, liefert im selben Issue das Unterkommando der Kommandozeile
mit, auch wenn es nur eine Ausgabe mit `--json` ist. Umgekehrt bekommt jedes Unterkommando
der Kommandozeile spätestens im folgenden Sprint ein Gegenstück in der Oberfläche.

**Paritäts-Tabelle, erzeugt und geprüft.** `docs/reference/parity.md` wird von
`cargo xtask docs` aus drei Quellen erzeugt: den Service-Methoden des Protos, der
clap-Struktur der Kommandozeile (Unterkommandos mit dem Attribut
`#[humanitl(rpc = "…")]`) und einem Register der Oberfläche (`app/lib/core/parity.dart`,
Liste RPC zu Bildschirm). Sie hat drei Spalten: RPC, Unterkommando der Kommandozeile, Ort
in der Oberfläche. Der CI-Job `parity-check` **schlägt fehl**, wenn es einen RPC ohne
Zeile für die Kommandozeile gibt; fehlende Gegenstücke in der Oberfläche werden als `warn`
aufgeführt.

**Gute Vorgaben gehören zur Parität.** Das mitgelieferte Profil `default` beschreibt den
üblichen Anwendungsfall vollständig: OpenCode, LLM im LAN, Nachfragemodus `ask`, eine
Zeitüberschreitung von fünf Minuten, mitgelieferte Blockierregeln, Session-Regeln
bevorzugt. Wer nichts ändert, bekommt diesen Weg; wer etwas ändern will, findet jeden Wert
unter demselben Schlüssel im Profil, im Einstellungsbildschirm und in der Kommandozeile
(ADR-0011).

## Begründung

Die ungleiche Strenge der Prüfung ist Absicht. Ein RPC ohne Unterkommando der
Kommandozeile bricht den Build; ein fehlendes Gegenstück in der Oberfläche ist nur eine
Warnung. Der Grund: Der teure Fehler ist Fachlogik, die nur über die Oberfläche erreichbar
ist, denn sie ist dann nicht skriptbar, nicht ohne Bildschirm testbar und nicht
automatisierbar. Ein fehlendes Gegenstück in der Oberfläche ist dagegen nur eine
Unbequemlichkeit, die der nächste Sprint behebt.

Dass die Tabelle **erzeugt** und nicht gepflegt wird, ist der Unterschied zwischen einer
Regel und einer Absichtserklärung. Eine von Hand gepflegte Paritäts-Tabelle wäre nach dem
dritten Sprint unvollständig, und niemand würde es merken. Die Erzeugung aus Proto,
clap-Struktur und Register der Oberfläche holt die Wahrheit aus dem Code; das Register auf
der Dart-Seite ist der einzige von Hand gepflegte Teil und klein genug, um vollständig zu
bleiben.

„Zuerst als RPC entwerfen" ist außerdem ein Entwurfswerkzeug, nicht nur eine Pflicht. Wer
eine Fähigkeit als RPC formuliert, muss ihre Eingaben und Ausgaben benennen, bevor er ein
Formular baut. Das fördert Fähigkeiten, die klein und kombinierbar sind, statt Bildschirme,
die alles auf einmal tun.

Die Vorgaben in die Parität aufzunehmen schließt eine Lücke, die leicht übersehen wird:
Zwei Oberflächen können dieselben RPCs aufrufen und trotzdem verschieden starten, wenn
jede ihre eigenen Voreinstellungen mitbringt. Das mitgelieferte Profil `default` ist die
eine Quelle für den Standardweg; beide Oberflächen laden es, keine erfindet eigene
Voreinstellungen.

## Verworfene Alternativen

- **Parität als Konvention ohne CI-Prüfung.** Hält bis zur ersten Deadline. Dieselbe
  Begründung wie in ADR-0015 für die Abhängigkeitsrichtung.
- **Die Kommandozeile aus dem Proto erzeugen.** Klingt folgerichtig und ergibt eine
  unbrauchbare Kommandozeile: RPC-Grenzen sind keine Grenzen von Unterkommandos, und eine
  gute Kommandozeile braucht Kurzformen, Voreinstellungen und aufgabenbezogene
  Zusammenfassungen. Deshalb wird die Zuordnung annotiert (`#[humanitl(rpc = "…")]`)
  statt erzeugt.
- **CLI-first statt GUI-first.** Hätte die Parität ebenso gesichert, aber die
  Produktentscheidungen fallen an den Bildschirmen. GUI-first mit Mitlieferpflicht
  bekommt beides.
- **Nur die Kommandozeile, keine Oberfläche.** Widerspricht der Zielgruppe: Fachleute ohne
  Sicherheitshintergrund, die live neben dem Agenten sitzen und in Sekunden entscheiden
  sollen.
- **Fachlogik in der Oberfläche erlauben, wenn sie klein ist.** Der Anfang jedes
  Auseinanderlaufens. Die Grenze „klein" lässt sich nicht verteidigen, also gibt es sie
  nicht.
- **Paritäts-Tabelle nur als Dokumentation ohne CI-Job.** Wäre nach drei Sprints falsch
  und würde dann aktiv in die Irre führen.

## Folgen

- Der Aufwand je Fähigkeit steigt: RPC, Umsetzung im Daemon und Unterkommando der
  Kommandozeile im selben Issue. Der Gewinn ist eine automatisierbare Anwendung und ein
  Daemon, der ohne Bildschirm vollständig testbar ist.
- `cargo xtask docs` gehört zur Toolchain und erzeugt `parity.md`. Das xtask-Crate ist ein
  Hilfswerkzeug außerhalb der Abhängigkeitsregeln und enthält nie Laufzeitcode
  (`backlog/CONVENTIONS.md` 3.1).
- Das Attribut `#[humanitl(rpc = "…")]` an den clap-Unterkommandos ist Pflicht; ohne es
  erscheint das Unterkommando nicht in der Tabelle.
- `app/lib/core/parity.dart` ist eine bewusst von Hand gepflegte Liste RPC zu Bildschirm.
  Sie ist der einzige Ort, an dem die Zuordnung steht.
- Fachlogik in `app/` oder in `daemon/bin/humanitl` ist ein Verstoß gegen die
  Architektur, auch wenn sie klein ist (`docs/ARCHITECTURE.md` 3b).
- Der Fake-Daemon (HUM-005) implementiert dasselbe Proto und profitiert dadurch
  automatisch von jeder neuen Fähigkeit; er ist ein dritter Client desselben Vertrags.
- Diese Entscheidung verschärft ADR-0013: Dort ist die Kommandozeile ein gleichberechtigter
  Client, hier wird die Gleichberechtigung maschinell geprüft.

## Betroffene Issues

`HUM-078` (Paritäts-Tabelle und CI-Job `parity-check`), `HUM-003` (Proto als einzige
Schnittstelle), `HUM-064` (Gerüst der Kommandozeile mit RPC-Attributen),
`HUM-065` (`rules`/`flows` als Gegenstück auf der Kommandozeile), `HUM-070`
(`config`/`audit`/`daemon`), `HUM-066` (mitgeliefertes Profil `default`).
