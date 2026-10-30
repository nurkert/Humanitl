---
source: adr/0015-ports-and-adapters.md
source-sha256: cfd4483aa0fac82b63d42219cfbf3efa04ca8b45e99c2ffba428cb2e17ef491c
lang: de
---
# ADR-0015 · Ports und Adapter mit maschinell erzwungener Abhängigkeitsrichtung

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl ist ein Hobbyprojekt mit langer Lebensdauer. Solche Projekte scheitern
selten an einer falschen Technologieentscheidung und fast immer am Wildwuchs:
Nach zwei Jahren kennt jedes Modul jedes andere, ein Test braucht einen
laufenden Daemon, und eine Änderung an der Regelauswertung zieht eine Änderung
an der Protobuf-Definition nach sich.

Zugleich soll der Code so schlank wie möglich bleiben. Die naheliegende Antwort
auf Wildwuchs, mehr Abstraktionsschichten, bewirkt oft das Gegenteil: eine
Schnittstelle über jeder Fremdbibliothek, eine Fabrik über jeder Schnittstelle,
und niemand findet mehr, wo etwas tatsächlich passiert.

Gebraucht wird also eine Regel, die beides zugleich sichert: klare Grenzen und
wenig Code. Und sie muss maschinell prüfbar sein, denn Konventionen fallen unter
Zeitdruck als Erstes.

## Entscheidung

**Drei Schichten mit einer Abhängigkeitsrichtung.**

```
Adapters (outer)  ipc/tonic · sandbox/bwrap · proxy/hudsucker · recorder/sqlite
                  catalog/files · cli/clap                       IO, async, third-party libraries
        │ may point inwards
Application       session orchestration, hold queue,
                  policy decision (rule → verdict → hold)        async, but only via ports
        │
Core (inner)      core-types · rules · findings · audit · config no IO, no async, no proto
```

Abhängigkeiten zeigen ausschließlich nach innen. Der Kern importiert nichts aus
der Anwendung oder den Adaptern. Die Anwendung kennt Adapter nur als Traits.

**Die Port-Liste ist im MVP geschlossen.** Jeder Port hat genau eine
Implementierung:

| Port | Definiert in | MVP-Adapter | Spätere Adapter |
|---|---|---|---|
| `SandboxBackend` | sandbox | bwrap | docker, seatbelt, microvm |
| `AgentAdapter` | core | opencode | aider, codex, claude-code |
| `Detector` | findings | regex/checksum | presidio-grpc, wasm |
| `FlowStore` | recorder | sqlite | postgres (Team) |
| `AuditSink` | audit | jsonl-hashchain | remote-anchor |
| `CatalogSource` | catalog | bundled-yaml | user-yaml, team-url |
| `Resolver` | proxy | hickory (nach dem Erlauben) | doh |
| `Egress` | proxy | direct | http-proxy, socks5h (Tor) |
| `Bridge` (Shim) | sandbox | proxy-socket | cdp-socket (Browser) |
| `AskChannel` | application | grpc-ui, terminal, none | plugin |

**Ein neuer Port braucht ein ADR und einen konkreten zweiten Adapter.** Kein
Trait ohne einen zweiten Nutzer in Sicht.

**Von der CI erzwungen**, nicht vom Review:

- `tools/check-deps.sh` liest `cargo metadata` und prüft jede
  Workspace-Abhängigkeit gegen die Richtungstabelle in `backlog/CONVENTIONS.md` 3.1.
- Dasselbe Skript prüft `#![deny(missing_docs)]` in jedem Bibliotheks-Crate.
- Es prüft, dass `TcpStream::connect` nur unterhalb von
  `daemon/crates/proxy/src/egress/` vorkommt (ADR-0017).
- Es prüft, dass kein Flutter-Feature ein anderes Feature importiert.
- Erzeugter Code (Protobuf, riverpod, freezed) steht in gitignore und wird in der
  CI erzeugt; Drift bricht den Build.

**Sparsamkeit im Code, konkret:**

- Keine Abstraktion über Fremdbibliotheken. hudsucker, tonic und rusqlite werden
  direkt benutzt, aber jede in **einem** Crate. Ändert sich die Bibliothek,
  ändert sich dieses eine Crate.
- Werttypen statt Strings. `HostName`, `FlowId`, `DiagnosticCode` sind Typen;
  ein `String` in einer Signatur ist verdächtig, außer es ist wirklich freier
  Text.
- Fehler sind Werte mit Bedeutung: `thiserror` je Crate, kein `anyhow` in
  Bibliotheks-Crates, `anyhow` nur in `main` (ADR-0012).
- Dünne Binaries. `humanitld` und `humanitl` sind Verdrahtung. Ein `if` in
  `main.rs`, das eine fachliche Entscheidung trifft, steht am falschen Ort.
- Löschen ist ein Feature. Jeder Sprint endet mit einem Blick auf
  `experimental`-Flags und ungenutzte Pfade; was zwei Sprints lang niemand
  nutzt, fliegt raus.

Auf der Flutter-Seite gelten dieselben Regeln: `core/domain` sind freezed-Typen
ohne Flutter-Import, `core/ipc` ist eine Schnittstelle mit zwei Implementierungen
(gRPC und Fake), ein Feature importiert kein anderes Feature, und das
Widget-Vokabular lebt in `packages/ui` auf Basis von `shadcn_flutter`. Ein
Feature importiert `lib/core/ui/ui.dart`, nie ein fremdes Widget-Paket;
`tools/check-deps.sh` beanstandet jeden Import der Bibliothek außerhalb von
`app/packages/ui` (ADR-0009, Abschnitt „Am 2026-09-04 vom Projektinhaber
überarbeitet").

## Begründung

Die Richtung nach innen ist die einzige Regel, die man sich merken muss, und sie
trägt alles andere. Sie macht den Kern testbar ohne Netzwerk, ohne Datenbank und
ohne Laufzeitumgebung: Regelauswertung, Zustandsübergänge, Hash-Kette und
Vorrang der Konfiguration sind reine Funktionen über Werttypen und lassen sich
tabellengetrieben prüfen. Das ist kein Selbstzweck, sondern der Grund, warum die
sicherheitsrelevante Logik überhaupt vollständig testbar ist.

Die geschlossene Port-Liste ist das Gegengewicht zum Wildwuchs an Abstraktionen.
Ohne sie setzt die typische Bewegung ein: Jedes Modul bekommt vorsorglich einen
Trait, und nach einem Jahr gibt es dreißig Schnittstellen mit je einer
Implementierung. Zehn Ports mit einem benannten zweiten Adapter in der Zukunft
sind eine überschaubare Menge, und die Hürde „ADR plus konkreter zweiter
Adapter" hält sie klein.

Dass Fremdbibliotheken **nicht** abstrahiert werden, aber jede nur in einem
Crate vorkommt, ist der Kompromiss zwischen den beiden Fehlern. Eine
Schnittstelle über hudsucker wäre eine Erfindung, die nur den kleinsten
gemeinsamen Nenner aller denkbaren Proxy-Bibliotheken abbildet: mehr Code,
weniger Fähigkeiten. hudsucker direkt zu benutzen, aber nur in `humanitl-proxy`,
hält die Kosten eines Wechsels lokal, ohne vorab etwas zu erfinden.

Die maschinelle Prüfung ist das Herz dieser Entscheidung. Eine
Architekturregel, die nur in den Köpfen existiert, hält bis zur ersten Deadline.
`cargo metadata` kennt den tatsächlichen Abhängigkeitsgraphen; ein Skript, das
ihn gegen eine Tabelle prüft, hat zwanzig Zeilen und hält Jahre. Dasselbe gilt
für das grep nach `TcpStream::connect`: Es ist grob, fängt aber genau den
Fehler, der die Egress-Garantie still untergraben würde.

## Verworfene Alternativen

- **Schichtung nur als Konvention im Review.** Der übliche Weg und der Grund,
  warum die meisten Projekte ihre Architektur verlieren. Ein Review übersieht
  eine Zeile in einer `Cargo.toml`; `cargo metadata` nicht.
- **Ein einziges Crate mit Modulen.** Weniger Zeremonie, aber Rust erzwingt
  innerhalb eines Crates keine Modulgrenzen in Sachen Abhängigkeitsrichtung: `mod
  rules` könnte `mod proxy` benutzen, und niemand würde es merken.
- **Vollständige Clean Architecture mit Use-Case-Klassen und DTOs an jeder
  Grenze.** Erzwingt dieselbe Richtung und kostet ein Vielfaches an Code sowie
  eine Übersetzungsschicht je Grenze. Unverhältnismäßig für ein Programm dieser
  Größe.
- **Ports für alles, vorsorglich.** Verworfen mit der Regel „kein Trait ohne
  einen zweiten Nutzer in Sicht". Ein Trait mit genau einer Implementierung, der
  nie eine zweite bekommt, ist Aufwand ohne Nutzen.
- **Microservices statt Crates.** Trennung über Prozessgrenzen statt
  Modulgrenzen. Absurd für ein Desktop-Programm, mit einer Ausnahme, die bleibt:
  der Shim in der Sandbox, weil er ohne tokio und ohne Abhängigkeiten auskommen
  muss.
- **Erzeugten Code einchecken.** Bequem beim Bauen ohne Toolchain, aber Drift
  zwischen Quelle und Erzeugnis wird dann unsichtbar. Stattdessen: in gitignore
  und in der CI erzeugt.

## Folgen

- Der Workspace hat mehr Crates, als man auf den ersten Blick für nötig hielte.
  Das ist der Preis dafür, dass die Richtung vom Compiler und von
  `cargo metadata` geprüft wird statt von Menschen.
- Neue Abhängigkeiten zwischen Crates brauchen einen Eintrag in
  `tools/deps-allow.toml` und damit eine bewusste Entscheidung.
- `#![forbid(unsafe_code)]` und `#![deny(missing_docs)]` stehen in jedem
  Bibliotheks-Crate; die zweite Zeile prüft das Skript.
- Ein zweiter Adapter (Docker, macOS, Postgres, Tor) berührt weder Kern noch
  Anwendung. Das ist das Versprechen, mit dem ADR-0002, ADR-0008 und ADR-0017
  arbeiten.
- Die Testpyramide folgt der Schichtung: tabellengetriebene Unit-Tests im Kern,
  In-Process-Integrationstests gegen ein Fake-Ziel für den Proxy, privilegierte
  Escape-Tests für die Garantien, Widget- und Golden-Tests gegen den
  Fake-Daemon. Ein Fehler bekommt zuerst einen Test auf der niedrigsten Ebene,
  auf der er sich nachstellen lässt.
- Der Preis ist etwas mehr Zeremonie beim Anlegen einer neuen Fähigkeit. Der
  Gewinn: Eine Änderung an der Regelauswertung erzwingt nie eine Änderung an der
  Protobuf-Definition.

## Betroffene Issues

`HUM-074` (Abhängigkeits-Lint `tools/check-deps.sh`), `HUM-001`
(Monorepo-Aufbau mit der Aufteilung in Crates), `HUM-004` (core-types als Kern
ohne IO), `HUM-002` (CI-Job `deps-lint`), `HUM-062` (Konfiguration im Kern,
Vorrang als reine Funktion).
