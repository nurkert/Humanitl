# Der Vertrag `humanitl.v1`

> Die gRPC-Proto unter `proto/humanitl/v1/` ist die einzige Art, mit dem Daemon zu reden. UI, CLI, Tests und spätere Plugins sind gleichberechtigte Clients (ADR-003, `docs/ARCHITECTURE.md` 1.3 und 3b). Wer eine Fähigkeit hinzufügt, fügt sie hier hinzu, nicht daneben.

## 1. Die Dateien

| Datei | Inhalt | Importiert |
|---|---|---|
| `proto/humanitl/v1/common.proto` | `Method`, `Scheme`, `Upgrade` | nichts |
| `proto/humanitl/v1/rules.proto` | `Rule`, `RuleMatcher`, `RuleExpiry`, `RuleAction` | `common.proto` |
| `proto/humanitl/v1/humanitl.proto` | Service `Humanitl`, Flows, Findings, Diagnostics, Sandbox, Terminal, Audit, Config, Doctor, LLM-Suche | `common.proto`, `rules.proto` |

`common.proto` existiert, weil `RuleMatcher` dieselben Methoden und Schemata braucht wie `HttpRequest`. Ohne die dritte Datei entstünde ein Import-Zyklus zwischen `rules.proto` und `humanitl.proto`.

Alle drei Dateien liegen im Paket `humanitl.v1`. Für die Codegenerierung ist das ein einziges Modul; in Rust landet alles in `humanitl_ipc::v1`.

## 2. Codegenerierung

**Rust** braucht weder Skript noch Fremdwerkzeug. `daemon/crates/ipc/build.rs` übersetzt die `.proto`-Dateien bei jedem `cargo build` mit `protox`, einem Protobuf-Compiler in reinem Rust, und erzeugt mit `tonic-prost-build` Client und Server nach `OUT_DIR`; `src/lib.rs` zieht das Ergebnis mit `include!(concat!(env!("OUT_DIR"), "/humanitl.v1.rs"))` ein. In den Quellbaum wird nichts geschrieben. Ein frischer Clone baut ohne `protoc` und ohne `buf`.

```
make proto          # oder: ./scripts/gen-proto.sh
```

macht zwei Dinge: `cargo xtask proto` erneuert `proto/descriptor.binpb`, und `protoc` mit `protoc-gen-dart` erzeugt `app/lib/core/ipc/generated/` samt `proto/generated.sha256`. Die Übersetzung selbst steht einmal in `daemon/crates/ipc/proto_gen.rs` und wird von `build.rs` und `xtask` per `include!` geteilt, damit Rust-Code und Descriptor nie aus verschiedenen Quellen stammen.

**Dart** braucht `protoc` und `protoc-gen-dart`, das Plugin in genau der Version, die `scripts/gen-proto.sh` als `PLUGIN_VERSION` nennt (25.0.0): der Hash in `proto/generated.sha256` hängt an der Ausgabe des Plugins, und CI vergleicht ihn mit dem eingecheckten Stand.

```
sudo apt install protobuf-compiler
dart pub global activate protoc_plugin 25.0.0
export PATH="$PATH:$HOME/.pub-cache/bin"
```

Fehlt ein Werkzeug oder stimmt die Plugin-Version nicht, sagt das Skript, was zu tun ist, und endet mit 0. Mit `STRICT=1` oder `CI=true` (GitHub Actions setzt es) endet es stattdessen mit 1 und verlangt zusätzlich, dass `proto/generated.sha256` eingecheckt ist und dass sich die Plugin-Version über `dart pub global list` belegen lässt: ein `protoc-gen-dart` unbekannter Herkunft ist im harten Modus ein Fehler, am Arbeitsplatz nur eine Warnung. So ist CI hart und der Arbeitsplatz weich. Ein Arbeitsplatz ohne `protoc` baut und testet die Rust-Seite vollständig, kann das Flutter-Gate (`make flutter-analyze`, `make flutter-test`) aber erst ausführen, wenn `scripts/gen-proto.sh` den Dart-Code einmal erzeugt hat; `make proto` sagt, was dafür fehlt.

Die Version des Dart-Plugins (25.x) und die des Pakets `protobuf` (6.x) gehören zusammen. BACKLOG.md und HUM-003 nennen 22.x/4.x; seitdem verlangt `grpc` 5.x das Paket `protobuf` 6.x, und dazu gehört `protoc_plugin` 25.x. Gepinnt ist das Plugin an genau einer Stelle, als `PLUGIN_VERSION` in `scripts/gen-proto.sh`; die CI-Aktion `setup-flutter` liest den Wert von dort, statt ihn zu wiederholen (CONVENTIONS.md 4.11: eine Pin-Datei je Werkzeug). Das Paket ist in `app/pubspec.yaml` gepinnt.

## 3. Was eingecheckt ist und was nicht

| Pfad | Im Git | Warum |
|---|---|---|
| `proto/humanitl/v1/*.proto` | ja | Der Vertrag. |
| `proto/descriptor.binpb` | ja | Übersetzter Vertrag ohne Quellpositionen. Eingabe von `daemon/crates/ipc/tests/proto_contract.rs`, Grundlage für `grpcurl`. Frisch halten ihn genau zwei Prüfungen: der Test `checked_in_descriptor_matches_the_proto_sources` in derselben Datei übersetzt die `.proto`-Dateien bei jedem `cargo test -p humanitl-ipc` über `proto_gen.rs` (derselbe Codepfad wie `cargo xtask proto`) und vergleicht Byte für Byte, mit dem Hinweis `run cargo xtask proto`; und der Schritt „Fail on generated drift" im CI-Job `proto-lint-and-gen` (`.github/workflows/ci.yml`) lässt `scripts/gen-proto.sh` laufen und bricht ab, wenn `git diff` danach etwas unter `proto/`, `daemon/crates/ipc/` oder `app/lib/core/ipc/` zeigt. `buf breaking` läuft nicht dagegen, sondern gegen die `.proto`-Dateien auf `main`. Nach jeder Proto-Änderung mit `make proto` (oder `cargo xtask proto` in `daemon/`) neu erzeugen und mitcommitten. |
| `proto/generated.sha256` | ja | Hash über die erzeugten Dart-Dateien. Frisch hält ihn allein der CI-Schritt „Fail on generated drift"; lokal prüft ihn kein Test, weil `generated/` gitignored ist und ohne `protoc` nicht entsteht. Nach jeder Proto-Änderung mitcommitten. |
| `daemon/crates/ipc/src/` | nur Handgeschriebenes | Der erzeugte Rust-Code liegt in `OUT_DIR` unter `daemon/target/` und entsteht bei jedem `cargo build`. |
| `app/lib/core/ipc/generated/` | nein | Entsteht in `scripts/gen-proto.sh`. Jeder Job, der die App analysiert, testet oder baut, erzeugt ihn vorher. |

Generierter Code ist kein Quellcode (`docs/ARCHITECTURE.md` 4). Wer ihn von Hand ändert, verliert die Änderung beim nächsten Build.

## 4. Wie man den Vertrag ändert

1. Nur additiv. Neue Felder bekommen die nächste freie Nummer, neue Enum-Werte die nächste freie Zahl, neue RPCs kommen ans Ende des Service.
2. Feldnummern werden nie recycelt. Ein entferntes Feld wird `reserved`, nie überschrieben. Dasselbe gilt für Enum-Werte.
3. Jedes Enum hat `_UNSPECIFIED = 0` als ersten Wert, und jeder Wert trägt den Namen des Enums als Präfix in `SCREAMING_SNAKE_CASE`.
4. IDs sind `string` mit UUID-Text, Zeitstempel `google.protobuf.Timestamp`, Zeitspannen `google.protobuf.Duration`.
5. Bodies reisen nie in Ereignissen. Ein Ereignis trägt höchstens einen `BodyRef`; den Inhalt holt der Client über `GetBody`. `proto_contract.rs` prüft das transitiv über alles, was von `FlowEvent` aus erreichbar ist; ein neues `bytes`-Feld dort muss in die Erlaubnisliste des Tests, mit Begründung. Ein Body als Inhalt reist an genau zwei Stellen, beide außerhalb des Ereignisstroms: hinein über `DecideRequest.allow_edited` (`EditedRequest.body`, weil der Daemon den bearbeiteten Inhalt sonst nirgends hat) und hinaus über `GetBody`. Dazu kommt `FlowDetail.body_preview`, der Anfang des Request-Bodys als verlustbehaftetes UTF-8 mit höchstens 4096 Zeichen, nur im Detail. Der Test `edited_request_and_body_preview_stay_out_of_the_event_stream` hält beide von `FlowEvent` fern.
6. Header-Werte sind `bytes`, nicht `string`. HTTP-Header sind nicht garantiert UTF-8.
7. Jede Variante von `humanitl_core::FlowEvent` hat ihr Gegenstück im `oneof event` von `FlowEvent`, damit die Abbildung total ist. Ein Upstream-Fehler ist `FlowEvent.Failed` mit `UpstreamError`, nie `ResponseHeaders` mit Status 502 (CONVENTIONS.md 3.2, 4.10).
8. Nach der Änderung: `make proto`, `proto/descriptor.binpb` und `proto/generated.sha256` mitcommitten, `cargo test -p humanitl-ipc` und `flutter test`. Ein vergessener Descriptor fällt sofort auf: `checked_in_descriptor_matches_the_proto_sources` schlägt an, solange `proto/descriptor.binpb` nicht zu den `.proto`-Dateien passt; ein vergessener Dart-Hash erst in CI („Fail on generated drift"). Die Tabellen in `daemon/crates/ipc/tests/proto_contract.rs` sind Teil des Vertrags und werden mitgepflegt.
9. Ein neuer RPC bringt sein CLI-Subkommando im selben Issue mit (`docs/ARCHITECTURE.md` 3b).

## 5. Versionsregel

`Info.proto_major` und `Info.proto_minor` sagen, was der Daemon spricht; die Konstanten dazu stehen in `humanitl_ipc::PROTO_MAJOR` und `PROTO_MINOR`.

- Additive Änderung ⇒ Minor steigt. Ältere Clients funktionieren weiter, sie sehen die neuen Felder nicht.
- Bruch ⇒ Major steigt und das Paket heißt `humanitl.v2` in einem neuen Verzeichnis. `humanitl.v1` bleibt bestehen, bis niemand mehr danach fragt.
- Ein Client mit kleinerer Major verweigert die Verbindung und zeigt einen `Diagnostic` statt einer halb funktionierenden Oberfläche.

## 6. Lint und Kompatibilität in CI

`proto/buf.yaml` schaltet `STANDARD` ein und nimmt fünf Regeln aus, jede mit Begründung in der Datei: `PACKAGE_VERSION_SUFFIX`, `SERVICE_SUFFIX` (der Service heißt `Humanitl`, nicht `HumanitlService`) und die drei `RPC_*`-Namensregeln (die Request- und Response-Namen folgen BACKLOG.md 3.3, und mehrere RPCs teilen sich `google.protobuf.Empty`).

`buf breaking` läuft gegen die `.proto`-Dateien auf `main` und braucht `fetch-depth: 0` im Checkout.

Was `buf` prüft, prüfen die Tabellen in `daemon/crates/ipc/tests/proto_contract.rs` zusätzlich lokal am eingecheckten Descriptor: `_UNSPECIFIED = 0`, Enum-Präfixe, `snake_case`-Felder, Vollständigkeit von Nachrichten und RPCs, keine Bodies in Ereignissen. Dass dieser Descriptor die aktuellen `.proto`-Dateien wiedergibt und nicht einen alten Stand, sichert `checked_in_descriptor_matches_the_proto_sources` in derselben Datei (Abschnitt 3). Damit fällt eine Vertragsverletzung schon in `cargo test` auf, nicht erst in CI.

## 7. Authentisierung

Jeder Aufruf trägt das Session-Token aus `$XDG_RUNTIME_DIR/humanitl/token` (Modus 0600) im Metadata-Header `x-humanitl-token`; die Konstante dafür ist `humanitl_ipc::TOKEN_METADATA_KEY`. Transport ist ein Unix-Domain-Socket unter `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (Modus 0600), kein TCP.
