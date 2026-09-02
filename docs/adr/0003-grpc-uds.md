# ADR-0003 · gRPC über Unix Domain Socket als einzige Schnittstelle zum Daemon
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl besteht aus drei Programmen: dem Daemon `humanitld` (Rust), der
Oberfläche `humanitl` (Flutter/GTK) und dem Kommandozeilenwerkzeug `humanitl`
(Rust). Die Oberfläche muss einen Live-Strom von Ereignissen bekommen (jeder
angehaltene Request erscheint sofort), Entscheidungen zurückschicken, Regeln und
Aufzeichnungen abfragen und ein Terminal bidirektional durchreichen. Das
Kommandozeilenwerkzeug braucht dieselben Fähigkeiten.

Die Verbindung darf nur lokal sein. Sie darf für andere Nutzer der Maschine
nicht erreichbar sein, und sie darf unter keinen Umständen aus der Sandbox
heraus erreichbar sein — wer den Daemon steuern kann, kann sich selbst
freigeben. Gleichzeitig soll die Schnittstelle langlebig sein: Sie ist der Ort,
an dem später Plugins andocken (`BACKLOG.md` 6).

## Entscheidung

Die einzige Schnittstelle zum Daemon ist gRPC über einen Unix Domain Socket.
Der Vertrag besteht aus drei Dateien unter `proto/humanitl/v1/`, alle im
Package `humanitl.v1`: `common.proto` mit den Enums `Method`, `Scheme` und
`Upgrade`, `rules.proto` mit dem Regel-Modell, und `humanitl.proto`, das beide
importiert und den Service `Humanitl` deklariert. Auf Rust-Seite ist das ein
einziges Modul `humanitl_ipc::v1`. Server ist `tonic` in der Crate
`humanitl-ipc` — der einzigen Crate im Workspace, die Protobuf kennt. Sie
exportiert außerdem die Konstanten `PROTO_MAJOR`, `PROTO_MINOR` und
`TOKEN_METADATA_KEY` (`x-humanitl-token`), damit weder Daemon noch CLI diese
Werte ein zweites Mal buchstabieren. Clients sind die Flutter-App (Dart `grpc`
5.x über `InternetAddress.unix`), das CLI und die Tests.

Der Socket liegt unter `$XDG_RUNTIME_DIR/humanitl/daemon.sock` mit Modus `0600`
in einem Verzeichnis mit Modus `0700`. Zusätzlich trägt jeder Aufruf den
Metadata-Header `x-humanitl-token` mit dem Session-Token aus
`$XDG_RUNTIME_DIR/humanitl/token` (`0600`). Der Socket wird niemals in die
Sandbox gemountet; die Sandbox sieht ausschließlich den Proxy-Socket
(ADR-0002, Garantie 2).

Regeln für die Proto:

- Langweilig halten: kein `google.protobuf.Any`, wenig verschachtelte `oneof`.
- IDs als `string` (UUID-Text), Zeitstempel als `google.protobuf.Timestamp`,
  Enums immer mit `_UNSPECIFIED = 0`.
- Bodies stehen nie inline in Events, nur als
  `BodyRef { bytes sha256; uint64 size; bool truncated }`; der Inhalt kommt über
  den eigenen Stream `GetBody`.
- `GetInfo` liefert Daemon-Version, Proto-Version und Capabilities. Das UI
  verweigert die Verbindung zu einer höheren Major-Version, statt zu raten.
- Rust-Codegen läuft im Build-Skript über `protox` und `tonic-prost-build`
  nach `OUT_DIR`; ein installiertes `protoc` ist dafür nicht nötig. Der
  Deskriptor `proto/descriptor.binpb` ist committet und deterministisch;
  `cargo xtask proto` schreibt ihn neu, und nur ihn.
- Der Dart-Codegen läuft nur, wenn `protoc` und `protoc-gen-dart` vorhanden
  sind, in CI mit gepinntem Plugin; die erzeugten Dateien sind gitignored, Drift
  bricht den Build.

Erweiterungen gegenüber der Kurzfassung in `BACKLOG.md` 3.3
(`backlog/CONVENTIONS.md` 4.3 und 4.11):

- RPCs `GetConfig` und `SetConfig`; deshalb darf `humanitl-ipc` von
  `humanitl-config` abhängen.
- `FlowEvent` hat zusätzlich die Varianten `Failed { flow_id, error,
  resolved_ip }`, `Diagnostic`, `RulesChanged` und `AgentAsk`.
- `FlowEvent.Received` ist `{ summary, domain }`; die Domain-Information reist
  mit dem ersten Ereignis.
- `DecideRequest.block` ist `Block { note }`: die Notiz des Nutzers beim Blocken
  (ADR-0014).
- `DecideResponse.created_rule` trägt die volle Regel; `created_rule_id`
  (Feld 2) wird bei der nächsten Proto-Änderung `reserved`.

Der Daemon bleibt ein eigener Prozess, obwohl Rust per FFI in die Flutter-App
einbettbar wäre.

## Begründung

Server-Streaming ist genau die Form, die der Ereignisstrom braucht:
`Subscribe` liefert `stream FlowEvent`, und die Flusskontrolle von HTTP/2 sorgt
für Backpressure, ohne dass wir sie selbst bauen. Ein langsames UI bremst den
Strom, statt Speicher im Daemon volllaufen zu lassen. Bei Überlauf des
Broadcast-Puffers (Kapazität 1024) schickt der Daemon ein `Lagged{n}`-Ereignis,
und das UI synchronisiert über `ListFlows(since)` nach — ein sichtbarer,
behandelter Zustand statt stillen Datenverlusts.

Ein Unix Domain Socket mit `0600` schließt andere Nutzer der Maschine über
Dateirechte aus, ohne Netzwerkkonfiguration und ohne Portkollisionen. Das
Session-Token ist der zweite Riegel für den Fall, dass die Dateirechte durch
eine Fehlkonfiguration nicht greifen.

Die `.proto` ist gleichzeitig drei Dinge: ausführbare Dokumentation der
Fähigkeiten, Versionsvertrag zwischen UI und Daemon, und die spätere
Plugin-Schnittstelle. Ein Plugin ist damit einfach ein weiterer gRPC-Client, es
braucht keinen neuen Mechanismus.

Der Daemon bleibt aus vier Gründen ein eigener Prozess. Erstens muss der Proxy
die UI überleben: Wer die UI schließt, während der Agent arbeitet, darf den
Agenten nicht ins Leere laufen lassen. Zweitens kann ein Flatpak kein `bwrap`
starten, die spätere Flatpak-Auslieferung der UI (ADR-0010) braucht den Daemon
also ohnehin außerhalb. Drittens nutzt der Headless-Betrieb (ADR-0013)
denselben Vertrag. Viertens ist die Prozessgrenze eine Sicherheitsgrenze: Der
CA-Schlüssel liegt nicht im Adressraum eines UI-Toolkits.

## Verworfene Alternativen

- **REST/JSON über HTTP auf Loopback.** Bräuchte einen TCP-Port auf dem Host,
  den jeder lokale Prozess erreicht — genau das, was ADR-0001 für den Proxy
  ausschließt. Streaming wäre SSE oder WebSocket, also ein zweiter Mechanismus
  neben den unären Aufrufen, und der Schema-Vertrag müsste separat gepflegt
  werden (OpenAPI), statt aus derselben Datei zu kommen.
- **Rust per FFI in die Flutter-App einbetten.** Spart einen Prozess, verliert
  aber alle vier oben genannten Eigenschaften: Der Proxy stirbt mit der UI,
  Flatpak scheidet aus, Headless bräuchte einen zweiten Pfad, und der
  CA-Schlüssel läge im UI-Prozess.
- **D-Bus.** Auf dem Linux-Desktop naheliegend, aber Streaming großer
  Ereignismengen ist unbequem, die Typisierung ist schwächer, es gibt keine
  brauchbare Bindung für die spätere Plugin-Nutzung, und die Kopplung an den
  Session-Bus macht Tests aufwändiger.
- **Cap'n Proto oder ein eigenes Längenpräfix-Protokoll.** Schneller, aber ohne
  ausgereifte Dart-Bindung. Der Engpass ist nicht die Serialisierung.
- **Named Pipes plus JSON-Lines.** Einfach zu bauen, aber ohne Schema, ohne
  Backpressure, ohne Versionierung. Genau die drei Dinge, die diese
  Schnittstelle langfristig braucht.

## Konsequenzen

- Jede Fähigkeit des Systems muss zuerst als RPC existieren. Diese Konsequenz ist
  so tragend, dass sie einen eigenen ADR bekommen hat: ADR-0018 (Parität).
- Die Rust-Seite braucht kein `protoc`; die Toolchain wächst nur für Dart um
  `protoc` und `protoc-gen-dart`. Beides läuft in CI mit gepinnten Versionen,
  lokal ist es optional; `make check` darf nicht davon abhängen.
- Ein Versionsbruch der Proto ist ein sichtbares Ereignis: `GetInfo` meldet die
  Major-Version, das UI zeigt einen Setup-Screen statt veralteter Daten.
- Der Fake-Daemon (HUM-005) implementiert dieselbe Proto in Rust und spielt eine
  JSONL-Session ab. Damit ist die UI-Entwicklung vollständig vom Proxy
  entkoppelt, ohne einen zweiten Vertrag zu erfinden.

## Betroffene Issues

`HUM-003` (Proto v1 definieren), `HUM-018` (gRPC-Server-Grundgerüst),
`HUM-005` (Fake-Daemon), `HUM-013` (Proxy-Socket-Bind, Abgrenzung zum
gRPC-Socket), `HUM-019` (Flutter-Shell mit `GetInfo`-Versionscheck),
`HUM-078` (Paritäts-Tabelle).
