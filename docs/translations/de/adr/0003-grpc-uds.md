---
source: adr/0003-grpc-uds.md
source-sha256: 9af71288b0a0fa8885b771153489e1da3d0ac70e5579ab7289aa2142d9bd1597
lang: de
---
# ADR-0003 · gRPC über einen Unix-Domain-Socket als einzige Schnittstelle zum Daemon

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl besteht aus drei Programmen: dem Daemon `humanitld` (Rust), der
Oberfläche `humanitl` (Flutter/GTK) und dem Kommandozeilenwerkzeug `humanitl`
(Rust). Die Oberfläche muss einen laufenden Strom von Ereignissen empfangen (jede
angehaltene Anfrage erscheint sofort), Entscheidungen zurückschicken, Regeln und
Aufzeichnungen abfragen und ein Terminal in beide Richtungen durchreichen. Das
Kommandozeilenwerkzeug braucht dieselben Fähigkeiten.

Die Verbindung darf nur lokal sein. Für andere Benutzer des Rechners darf sie
nicht erreichbar sein, und aus der Sandbox heraus unter keinen Umständen: Wer
den Daemon steuern kann, kann sich selbst freigeben. Zugleich soll die
Schnittstelle langlebig sein; an ihr docken später Plugins an (`BACKLOG.md` 6).

## Entscheidung

Die einzige Schnittstelle zum Daemon ist gRPC über einen Unix-Domain-Socket.
Der Vertrag besteht aus drei Dateien unter `proto/humanitl/v1/`, alle im
Paket `humanitl.v1`: `common.proto` mit den Enums `Method`, `Scheme` und
`Upgrade`, `rules.proto` mit dem Regelmodell und `humanitl.proto`, das beide
importiert und den Dienst `Humanitl` deklariert. Auf der Rust-Seite ist das ein
einziges Modul `humanitl_ipc::v1`. Der Server ist `tonic` im Crate
`humanitl-ipc`, dem einzigen Crate im Workspace, das Protobuf kennt. Es
exportiert auch die Konstanten `PROTO_MAJOR`, `PROTO_MINOR` und
`TOKEN_METADATA_KEY` (`x-humanitl-token`), damit weder Daemon noch CLI diese
Werte ein zweites Mal ausschreiben. Clients sind die Flutter-App (Dart `grpc`
5.x über `InternetAddress.unix`), die CLI und die Tests.

Der Socket liegt unter `$XDG_RUNTIME_DIR/humanitl/daemon.sock` mit Modus `0600`
in einem Verzeichnis mit Modus `0700`. Zusätzlich trägt jeder Aufruf den
Metadaten-Header `x-humanitl-token` mit dem Session-Token aus
`$XDG_RUNTIME_DIR/humanitl/token` (`0600`). Der Socket wird nie in die Sandbox
eingehängt; die Sandbox sieht nur den Proxy-Socket (ADR-0002, Garantie 2).

Regeln für die Proto:

- Langweilig halten: kein `google.protobuf.Any`, wenige verschachtelte `oneof`s.
- IDs als `string` (UUID-Text), Zeitstempel als `google.protobuf.Timestamp`,
  Enums immer mit `_UNSPECIFIED = 0`.
- Bodies stehen nie direkt in Ereignissen, nur als
  `BodyRef { bytes sha256; uint64 size; bool truncated }`; der Inhalt kommt über
  einen eigenen Stream `GetBody`.
- `GetInfo` liefert Daemon-Version, Proto-Version und Fähigkeiten. Die Oberfläche
  verweigert die Verbindung zu einer höheren Hauptversion, statt zu raten.
- Die Rust-Codegenerierung läuft im Build-Skript über `protox` und
  `tonic-prost-build` nach `OUT_DIR`; ein installiertes `protoc` braucht sie
  nicht. Der Deskriptor `proto/descriptor.binpb` ist eingecheckt und
  deterministisch; `cargo xtask proto` schreibt ihn neu, und nur dieses.
- Die Dart-Codegenerierung läuft nur, wenn `protoc` und `protoc-gen-dart`
  vorhanden sind, in der CI mit festgelegtem Plugin; die erzeugten Dateien sind
  in gitignore, Drift bricht den Build.

Erweiterungen gegenüber der Kurzfassung in `BACKLOG.md` 3.3
(`backlog/CONVENTIONS.md` 4.3 und 4.11):

- RPCs `GetConfig` und `SetConfig`; deshalb darf `humanitl-ipc` von
  `humanitl-config` abhängen.
- `FlowEvent` hat zusätzlich die Varianten `Failed { flow_id, error,
  resolved_ip }`, `Diagnostic`, `RulesChanged` und `AgentAsk`.
- `FlowEvent.Received` ist `{ summary, domain }`; die Domain-Information reist
  mit dem ersten Ereignis.
- `DecideRequest.block` ist `Block { note }`: die Notiz des Nutzers beim
  Blockieren (ADR-0014).
- `DecideResponse.created_rule` trägt die vollständige Regel; `created_rule_id`
  (Feld 2) wird bei der nächsten Proto-Änderung `reserved`.

Der Daemon bleibt ein eigener Prozess, obwohl sich Rust über FFI in die
Flutter-App einbetten ließe.

## Begründung

Server-Streaming ist genau die Form, die der Ereignisstrom braucht:
`Subscribe` liefert `stream FlowEvent`, und die Flusskontrolle von HTTP/2 sorgt
für Gegendruck, ohne dass wir ihn selbst bauen. Eine langsame Oberfläche bremst
den Strom, statt den Speicher im Daemon volllaufen zu lassen. Läuft der
Broadcast-Puffer über (Kapazität 1024), schickt der Daemon ein Ereignis
`Lagged{n}`, und die Oberfläche synchronisiert sich über `ListFlows(since)` neu:
ein sichtbarer, behandelter Zustand statt stillem Datenverlust.

Ein Unix-Domain-Socket mit `0600` schließt andere Benutzer des Rechners über
Dateirechte aus, ohne Netzwerkkonfiguration und ohne Portkollisionen. Das
Session-Token ist der zweite Riegel für den Fall, dass die Dateirechte wegen
einer Fehlkonfiguration nicht greifen.

Die `.proto` ist drei Dinge zugleich: ausführbare Dokumentation der
Fähigkeiten, Versionsvertrag zwischen Oberfläche und Daemon und die spätere
Plugin-Schnittstelle. Ein Plugin ist damit einfach ein weiterer gRPC-Client; es
braucht keinen neuen Mechanismus.

Der Daemon bleibt aus vier Gründen ein eigener Prozess. Erstens muss der Proxy
die Oberfläche überleben: Wer die Oberfläche schließt, während der Agent
arbeitet, darf den Agenten nicht ins Leere laufen lassen. Zweitens kann ein
Flatpak `bwrap` nicht starten, die spätere Flatpak-Auslieferung der Oberfläche
(ADR-0010) braucht den Daemon also ohnehin außerhalb. Drittens nutzt der
Betrieb ohne Oberfläche (ADR-0013) denselben Vertrag. Viertens ist die
Prozessgrenze eine Sicherheitsgrenze: Der CA-Schlüssel lebt nicht im
Adressraum eines UI-Toolkits.

## Verworfene Alternativen

- **REST/JSON über HTTP auf Loopback.** Bräuchte einen TCP-Port auf dem Rechner,
  den jeder lokale Prozess erreicht, genau das, was ADR-0001 für den Proxy
  ausschließt. Streaming wäre SSE oder WebSocket, also ein zweiter Mechanismus
  neben den unären Aufrufen, und der Schemavertrag müsste getrennt gepflegt
  werden (OpenAPI), statt aus derselben Datei zu kommen.
- **Rust über FFI in die Flutter-App einbetten.** Spart einen Prozess, verliert
  aber alle vier oben genannten Eigenschaften: Der Proxy stirbt mit der
  Oberfläche, Flatpak fällt weg, der Betrieb ohne Oberfläche bräuchte einen
  zweiten Weg, und der CA-Schlüssel lebte im Prozess der Oberfläche.
- **D-Bus.** Naheliegend auf dem Linux-Desktop, aber das Streamen großer
  Ereignismengen ist umständlich, die Typisierung schwächer, für die spätere
  Plugin-Nutzung gibt es keine brauchbare Anbindung, und die Kopplung an den
  Session-Bus macht Tests mühsamer.
- **Cap'n Proto oder ein eigenes Längenpräfix-Protokoll.** Schneller, aber ohne
  ausgereifte Dart-Anbindung. Der Engpass ist nicht die Serialisierung.
- **Named Pipes plus JSON-Zeilen.** Einfach zu bauen, aber ohne Schema, ohne
  Gegendruck, ohne Versionierung. Genau die drei Dinge, die diese Schnittstelle
  auf Dauer braucht.

## Folgen

- Jede Fähigkeit des Systems muss zuerst als RPC existieren. Diese Folge trägt
  so viel, dass sie ein eigenes ADR bekam: ADR-0018 (Parität).
- Die Rust-Seite braucht kein `protoc`; die Toolchain wächst nur für Dart um
  `protoc` und `protoc-gen-dart`. Beide laufen in der CI mit festgelegten
  Versionen, lokal sind sie optional; `make check` darf nicht von ihnen abhängen.
- Ein Versionsbruch der Proto ist ein sichtbares Ereignis: `GetInfo` meldet die
  Hauptversion, die Oberfläche zeigt einen Einrichtungsbildschirm statt
  veralteter Daten.
- Der Fake-Daemon (HUM-005) implementiert dieselbe Proto in Rust und spielt eine
  JSONL-Session ab. Die Entwicklung der Oberfläche ist damit vollständig vom
  Proxy entkoppelt, ohne einen zweiten Vertrag zu erfinden.

## Betroffene Issues

`HUM-003` (Proto v1 festlegen), `HUM-018` (Gerüst des gRPC-Servers),
`HUM-005` (Fake-Daemon), `HUM-013` (Bind des Proxy-Sockets, Abgrenzung vom
gRPC-Socket), `HUM-019` (Flutter-Shell mit Versionsprüfung über `GetInfo`),
`HUM-078` (Paritäts-Tabelle).
