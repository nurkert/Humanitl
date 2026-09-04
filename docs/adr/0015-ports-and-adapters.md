# ADR-0015 · Ports-and-Adapters mit maschinell erzwungener Abhängigkeitsrichtung
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl ist ein Hobbyprojekt mit langer Laufzeit. Solche Projekte scheitern
selten an einer falschen Technologieentscheidung und fast immer an der
Wucherung: Nach zwei Jahren kennt jedes Modul jedes andere, ein Test braucht
einen laufenden Daemon, und eine Änderung an der Regelauswertung zieht eine
Änderung an der Protobuf-Definition nach sich.

Zugleich soll der Code so schmal wie möglich bleiben. Die naheliegende Antwort
auf Wucherung — mehr Abstraktionsschichten — erzeugt oft das Gegenteil: eine
Schnittstelle über jeder Fremdbibliothek, eine Fabrik über jeder Schnittstelle,
und niemand findet mehr, wo tatsächlich etwas passiert.

Es braucht also eine Regel, die beides gleichzeitig sichert: klare Grenzen und
wenig Code. Und sie muss maschinell prüfbar sein, weil Konventionen unter
Zeitdruck als Erstes fallen.

## Entscheidung

**Drei Schichten mit einer Abhängigkeitsrichtung.**

```
Adapter (außen)   ipc/tonic · sandbox/bwrap · proxy/hudsucker · recorder/sqlite
                  catalog/files · cli/clap                       IO, async, Fremdbibliotheken
        │ darf nach innen
Anwendung         Session-Orchestrierung, Hold-Queue,
                  Policy-Entscheidung (Regel → Verdict → Hold)   async, aber nur über Ports
        │
Kern (innen)      core-types · rules · findings · audit · config kein IO, kein async, kein Proto
```

Abhängigkeiten zeigen ausschließlich nach innen. Der Kern importiert nichts aus
Anwendung oder Adaptern. Die Anwendung kennt Adapter nur als Traits.

**Die Port-Liste ist im MVP abgeschlossen.** Jeder Port hat genau eine
Implementierung:

| Port | Definiert in | MVP-Adapter | Spätere Adapter |
|---|---|---|---|
| `SandboxBackend` | sandbox | bwrap | docker, seatbelt, microvm |
| `AgentAdapter` | core | opencode | aider, codex, claude-code |
| `Detector` | findings | regex/checksum | presidio-grpc, wasm |
| `FlowStore` | recorder | sqlite | postgres (Team) |
| `AuditSink` | audit | jsonl-hashchain | remote-anchor |
| `CatalogSource` | catalog | bundled-yaml | user-yaml, team-url |
| `Resolver` | proxy | hickory (nach Allow) | doh |
| `Egress` | proxy | direct | http-proxy, socks5h (Tor) |
| `Bridge` (Shim) | sandbox | proxy-socket | cdp-socket (Browser) |
| `AskChannel` | Anwendung | grpc-ui, terminal, none | plugin |

**Ein neuer Port braucht einen ADR und einen konkreten zweiten Adapter.** Kein
Trait ohne zweiten Nutzer in Sicht.

**Erzwungen durch CI**, nicht durch Review:

- `tools/check-deps.sh` liest `cargo metadata` und prüft jede
  Workspace-Abhängigkeit gegen die Richtungstabelle in `backlog/CONVENTIONS.md`
  3.1.
- Dasselbe Skript prüft `#![deny(missing_docs)]` in jeder Bibliotheks-Crate.
- Es prüft, dass `TcpStream::connect` nur unterhalb von
  `daemon/crates/proxy/src/egress/` vorkommt (ADR-0017).
- Es prüft, dass kein Flutter-Feature ein anderes Feature importiert.
- Generierter Code (Protobuf, riverpod, freezed) ist gitignored und wird in CI
  erzeugt; Drift bricht den Build.

**Code-Sparsamkeit, konkret:**

- Keine Abstraktion über Fremdbibliotheken. hudsucker, tonic und rusqlite werden
  direkt benutzt, aber jeweils in **einer** Crate. Wechselt die Bibliothek,
  wechselt diese eine Crate.
- Werttypen statt Strings. `HostName`, `FlowId`, `DiagnosticCode` sind Typen;
  ein `String` an einer Signatur ist ein Verdachtsfall, außer er ist wirklich
  Freitext.
- Fehler sind Werte mit Bedeutung: `thiserror` pro Crate, kein `anyhow` in
  Bibliotheks-Crates, `anyhow` nur in `main` (ADR-0012).
- Dünne Binaries. `humanitld` und `humanitl` sind Verdrahtung. Ein `if` in
  `main.rs`, das eine Fachentscheidung trifft, steht am falschen Ort.
- Löschen ist ein Feature. Jeder Sprint endet mit einem Blick auf
  `experimental`-Flags und ungenutzte Pfade; was zwei Sprints lang niemand
  benutzt, fliegt.

Dieselben Regeln gelten auf der Flutter-Seite: `core/domain` sind freezed-Typen
ohne Flutter-Import, `core/ipc` ist ein Interface mit zwei Implementierungen
(gRPC und Fake), ein Feature importiert kein anderes Feature, und das
Widget-Vokabular steht in `packages/ui` auf `shadcn_flutter`. Ein Feature
importiert `lib/core/ui/ui.dart`, nie ein fremdes Widget-Paket; `tools/check-deps.sh`
beanstandet jeden Import der Bibliothek ausserhalb von `app/packages/ui`
(ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer").

## Begründung

Die Richtung nach innen ist die einzige Regel, die man sich merken muss, und sie
trägt alles Weitere. Sie macht den Kern testbar ohne Netzwerk, ohne Datenbank und
ohne Laufzeitumgebung: Regelauswertung, Zustandsübergänge, Hash-Kette und
Konfigurationspräzedenz sind pure Funktionen über Werttypen und tabellengetrieben
prüfbar. Das ist kein Selbstzweck — es ist der Grund, warum die
sicherheitsrelevante Logik überhaupt vollständig testbar ist.

Die abgeschlossene Port-Liste ist der Gegenpol zur Abstraktionswucherung. Ohne
sie entsteht die typische Bewegung: Jedes Modul bekommt vorsorglich ein Trait,
und nach einem Jahr gibt es dreißig Schnittstellen mit je einer Implementierung.
Zehn Ports mit einem benannten zweiten Adapter in der Zukunft sind eine
überschaubare Menge, und die Hürde „ADR plus konkreter zweiter Adapter" hält sie
klein.

Dass Fremdbibliotheken **nicht** abstrahiert werden, aber jeweils nur in einer
Crate vorkommen, ist der Kompromiss zwischen den beiden Fehlern. Eine
Schnittstelle über hudsucker wäre eine Erfindung, die nur den kleinsten
gemeinsamen Nenner aller denkbaren Proxy-Bibliotheken abbildet — mehr Code,
weniger Fähigkeiten. hudsucker direkt zu benutzen, aber nur in
`humanitl-proxy`, hält den Wechselaufwand lokal, ohne vorher etwas zu erfinden.

Die maschinelle Prüfung ist der Kern dieser Entscheidung. Eine Architekturregel,
die nur im Kopf existiert, hält bis zum ersten Termindruck. `cargo metadata`
kennt den tatsächlichen Abhängigkeitsgraphen; ein Skript, das ihn gegen eine
Tabelle prüft, ist zwanzig Zeilen und hält jahrelang. Dasselbe gilt für den
Grep nach `TcpStream::connect`: Er ist grob, aber er fängt genau den Fehler, der
die Egress-Garantie stillschweigend aushebeln würde.

## Verworfene Alternativen

- **Schichtung nur als Konvention im Review.** Der übliche Weg und der Grund,
  warum die meisten Projekte ihre Architektur verlieren. Ein Review übersieht
  eine Zeile in einer `Cargo.toml`; `cargo metadata` nicht.
- **Ein einziger Crate mit Modulen.** Weniger Zeremonie, aber Rust erzwingt
  Modulgrenzen innerhalb einer Crate nicht in der Abhängigkeitsrichtung: `mod
  rules` könnte `mod proxy` benutzen, und niemand merkt es.
- **Vollständige Clean Architecture mit Use-Case-Klassen und DTOs an jeder
  Grenze.** Erzwingt dieselbe Richtung und kostet ein Vielfaches an Code sowie
  eine Übersetzungsschicht pro Grenze. Für ein Programm dieser Größe
  unverhältnismäßig.
- **Ports für alles, vorsorglich.** Verworfen mit der Regel „kein Trait ohne
  zweiten Nutzer in Sicht". Ein Trait mit genau einer Implementierung, die nie
  eine zweite bekommt, ist Kosten ohne Nutzen.
- **Mikroservices statt Crates.** Trennung durch Prozessgrenzen statt durch
  Modulgrenzen. Für ein Desktop-Programm absurd — mit einer Ausnahme, die
  bestehen bleibt: der Shim in der Sandbox, weil er ohne tokio und ohne
  Abhängigkeiten sein muss.
- **Generierten Code einchecken.** Bequem beim Bauen ohne Toolchain, aber Drift
  zwischen Quelle und Erzeugnis wird dann unsichtbar. Stattdessen: gitignored und
  in CI erzeugt.

## Konsequenzen

- Der Workspace hat mehr Crates, als man auf den ersten Blick für nötig hielte.
  Das ist der Preis dafür, dass die Richtung vom Compiler und von `cargo
  metadata` geprüft wird statt von Menschen.
- Neue Abhängigkeiten zwischen Crates brauchen einen Eintrag in
  `tools/deps-allow.toml` und damit eine bewusste Entscheidung.
- `#![forbid(unsafe_code)]` und `#![deny(missing_docs)]` stehen in jeder
  Bibliotheks-Crate; die zweite Zeile wird vom Skript geprüft.
- Ein zweiter Adapter (Docker, macOS, Postgres, Tor) berührt Kern und Anwendung
  nicht. Das ist die Zusage, mit der ADR-0002, ADR-0008 und ADR-0017 arbeiten.
- Die Testpyramide folgt der Schichtung: tabellengetriebene Unit-Tests im Kern,
  In-Process-Integrationstests gegen einen Fake-Upstream für den Proxy,
  privilegierte Escape-Tests für die Garantien, Widget- und Golden-Tests gegen
  den Fake-Daemon. Ein Bug bekommt zuerst einen Test auf der niedrigsten Ebene,
  auf der er reproduzierbar ist.
- Der Preis ist etwas mehr Zeremonie beim Anlegen einer neuen Fähigkeit. Der
  Gewinn ist, dass eine Änderung an der Regelauswertung nie eine Änderung an der
  Protobuf-Definition erzwingt.

## Betroffene Issues

`HUM-074` (Abhängigkeits-Lint `tools/check-deps.sh`), `HUM-001` (Monorepo-Layout
mit der Crate-Aufteilung), `HUM-004` (core-types als IO-freier Kern),
`HUM-002` (CI-Job `deps-lint`), `HUM-062` (config im Kern, Präzedenz als reine
Funktion).
