# Sprint 5 · MVP 0.1 (M5)

Ziel des Sprints: Das System aus Sprint 0 bis 4 wird gehärtet, dokumentiert und als Version 0.1.0 veröffentlicht. Es kommt keine neue Funktion hinzu. Jedes Issue macht Bestehendes robuster, sichtbarer oder verteilbar. Am Ende steht eine manuelle Abnahme, die ein Mensch in 60 Minuten durchgeht.

Voraussetzung: Demo-Skripte M1 bis M4 (HUM-021, HUM-036, HUM-046, HUM-055) sind in CI grün. Wenn eines rot ist, wird es zuerst repariert, bevor ein Issue aus diesem Sprint begonnen wird.

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-056 | Fuzzing | M | HUM-015, HUM-022, HUM-032, HUM-050 |
| HUM-057 | Ressourcen-Limits und Backpressure | S | HUM-015, HUM-016, HUM-018, HUM-026, HUM-062 |
| HUM-058 | Fehlerpfade im UI | M | HUM-063, HUM-019, HUM-040, HUM-041, HUM-042, HUM-045, HUM-068 |
| HUM-146 | Private Ziele hinter NAT64, 6to4 und Sonderbereichen | S | HUM-004 |
| HUM-147 | verify-commit baut inkrementell, die CI nicht | S | — |
| HUM-059 | Dokumentation | M | alle vorherigen |
| HUM-086 | Repository auf Englisch | M | HUM-059 |
| HUM-060 | Release 0.1.0 | S | HUM-053, HUM-059 |
| HUM-061 | Puffer | L | keine |

---

> **Abgleich 2026-09-02**: Diagnostic-Codes, die HUM-058 voraussetzt, sind im Register (HUM-063) reserviert, siehe CONVENTIONS 4.6. `daemon/xtask` ist in CONVENTIONS 3.1 registriert. Fake-Szenarien über `--dart-define=HUMANITL_FAKE=<scenario>`.

## HUM-056 · Fuzzing
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-015, HUM-022, HUM-032, HUM-050 · Blockiert: HUM-060

### Kontext
Der Daemon verarbeitet Bytes, die ein Angreifer kontrolliert: Antworten von beliebigen Servern, Request-Bodies eines möglicherweise unterwanderten Agenten, Regel-Dateien, die Nutzer von Dritten übernehmen, und das Audit-Log, das jemand manipuliert haben könnte. BACKLOG.md 4.4 verlangt Fuzzing des Parsers und der Decoder in CI. ADR-001 begründet die Rust-Wahl unter anderem mit der Bedingung, dass die Parser gefuzzt werden. Dieses Issue löst diese Bedingung ein.

### Ziel
Unter `daemon/fuzz/` existiert ein cargo-fuzz-Workspace mit sechs Targets. Jedes Target läuft lokal mit `cargo +nightly fuzz run <target>` und in einem Nightly-CI-Job zehn Minuten. Ein Seed-Corpus pro Target liegt im Repo. Gefundene Abstürze werden reproduzierbar als Regressionstest übernommen. Kein Target darf nach zehn Minuten einen Absturz, einen Hänger über zehn Sekunden oder einen Speicherverbrauch über 512 MiB zeigen.

### Nicht-Ziel
Fuzzing des Flutter-Codes (Dart hat kein libFuzzer-Äquivalent, Widget-Tests decken das ab, HUM-054). Fuzzing der TLS-Schicht selbst (rustls wird von seinen Maintainern gefuzzt). Fuzzing des gRPC-Servers gegen bösartige UI-Clients (der gRPC-Socket ist 0600 und token-geschützt, Bedrohung ist gering, kommt nach dem MVP). Property-based Tests mit `proptest` sind ergänzend erlaubt, ersetzen aber kein Target hier.

### Betroffene Pfade
- `daemon/fuzz/Cargo.toml` (neu)
- `daemon/fuzz/fuzz_targets/http_request_path.rs` (neu)
- `daemon/fuzz/fuzz_targets/body_decoder.rs` (neu)
- `daemon/fuzz/fuzz_targets/chunked_decoder.rs` (neu)
- `daemon/fuzz/fuzz_targets/rules_yaml.rs` (neu)
- `daemon/fuzz/fuzz_targets/flow_filter.rs` (neu)
- `daemon/fuzz/fuzz_targets/audit_verify.rs` (neu)
- `daemon/fuzz/corpus/<target>/` (neu, Seeds eingecheckt)
- `daemon/fuzz/README.md` (neu, Triage-Prozess)
- `daemon/crates/proxy/src/decode.rs` (geändert: `pub fn decode_body(encoding, input, limits) -> Result<Bytes, DecodeError>` muss ohne Netzwerk aufrufbar sein)
- `daemon/crates/proxy/src/request_parse.rs` (geändert: reiner Parser-Einstieg `pub fn parse_request_head(bytes: &[u8]) -> Result<RequestHead, ParseError>` für das Target)
- `daemon/crates/recorder/src/filter.rs` (geändert: `pub fn parse_filter(&str) -> Result<Filter, FilterError>` öffentlich)
- `.github/workflows/fuzz-nightly.yml` (neu)
- `daemon/crates/*/tests/regressions/` (neu je nach Fund)

### Spezifikation

Workspace-Definition:

```toml
# daemon/fuzz/Cargo.toml
[package]
name = "humanitl-fuzz"
version = "0.0.0"
publish = false
edition = "2024"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
arbitrary = { version = "1", features = ["derive"] }
humanitl-core = { path = "../crates/core-types" }
humanitl-proxy = { path = "../crates/proxy" }
humanitl-rules = { path = "../crates/rules" }
humanitl-recorder = { path = "../crates/recorder" }
humanitl-audit = { path = "../crates/audit" }

[[bin]]
name = "http_request_path"
path = "fuzz_targets/http_request_path.rs"
test = false
doc = false
# ... ein [[bin]] pro Target, gleiche Form
```

Targets, Eingabeform und Invariante:

| Target | Eingabe | Aufruf | Invariante (Panic = Fund) |
|---|---|---|---|
| `http_request_path` | `&[u8]` beliebig | `parse_request_head(bytes)` dann bei `Ok` `RequestKey::from(&head)` und `HostName::parse(head.authority)` | Kein Panic. Bei `Ok` ist `host` normalisiert (lowercase, kein trailing dot, gültiges A-Label oder IP). Parsen von `head.to_bytes()` liefert dasselbe `head` (Roundtrip). |
| `body_decoder` | `struct In { encoding: u8, data: Vec<u8> }` via `arbitrary` | `decode_body(Encoding::from_u8(encoding % 4), &data, &Limits { max_out: 8 MiB, max_ratio: 100 })` | Kein Panic. Ausgabe nie größer als `max_out`. Bei `data.len() > 0` und Ausgabe `> data.len() * max_ratio` muss `Err(DecodeError::RatioExceeded)` kommen. Laufzeit pro Eingabe unter 1 s (libFuzzer `-timeout=10`). |
| `chunked_decoder` | `&[u8]` | `ChunkedDecoder::new(Limits::default()).feed_all(bytes)` | Kein Panic. Summe der ausgegebenen Chunks ≤ `max_out`. Ungültige Hex-Längen, negative Längen, fehlendes CRLF, Trailer über 8 KiB liefern `Err`, nie Hänger. |
| `rules_yaml` | `&[u8]` als UTF-8-Versuch | `RuleSet::parse_yaml(str)` dann bei `Ok` `evaluate` mit drei fixen `RequestKey` (Host `api.github.com`, IP `192.168.1.50`, WebSocket-Upgrade) | Kein Panic. Roundtrip `parse(to_yaml(parsed)) == parsed`. `evaluate` terminiert. Regex-Muster (`~`-Präfix) mit Backtracking-Bombe werden von `regex` (linear) toleriert, Compile-Fehler sind `Err`. |
| `flow_filter` | `&str` (arbitrary String) | `parse_filter(s)` dann bei `Ok` `filter.to_sql()` | Kein Panic. Erzeugtes SQL enthält nur Platzhalter, nie Literale aus der Eingabe (Prüfung: Eingabe-Substrings mit `'` tauchen im SQL nicht auf). |
| `audit_verify` | `&[u8]` als JSONL-Versuch | `AuditChain::verify_bytes(bytes, &Key::test())` | Kein Panic. Ergebnis ist `Ok(Verified{..})` oder `Err(AuditError::{Broken{seq}, Truncated, Malformed{line}})`. Eine gültige Kette, an der ein Byte geflippt wird, ergibt immer `Err`. |

Corpus-Seeds (mindestens je fünf Dateien, eingecheckt unter `daemon/fuzz/corpus/<target>/`):
- `http_request_path`: eine minimale GET-Zeile, ein POST mit Content-Length, ein CONNECT, ein Request mit IDN-Host, ein Request mit IPv6-Literal.
- `body_decoder`: ein 1-KiB-gzip einer Textdatei, ein brotli davon, ein „gzip-Bomb"-Anfang (10 MiB Nullen komprimiert, abgeschnitten auf 4 KiB), leere Eingabe, unkomprimierter Text.
- `chunked_decoder`: gültige Zwei-Chunk-Nachricht, Chunk mit Extension, Trailer-Header, Größe `FFFFFFFFFFFFFFFF`, abgeschnittene Nachricht.
- `rules_yaml`: `rules/default.yaml`, ein Regelsatz mit allen Feldern, ein leerer Regelsatz, eine Regel mit Regex-Pfad, eine Regel mit `expires` als Zeitstempel.
- `flow_filter`: `host:github.com`, `state:blocked method:POST`, `host:*.npmjs.org size:>1mb`, leerer String, ein String mit `' OR 1=1`.
- `audit_verify`: eine gültige Kette mit fünf Einträgen (aus HUM-050-Test erzeugt), dieselbe mit gelöschter Zeile 3, dieselbe ohne letzte Zeile, eine leere Datei, eine Zeile Nicht-JSON.

Der Seed für `audit_verify` wird mit einem Test-HMAC-Schlüssel erzeugt, der als `Key::test()` in `humanitl-audit` unter `#[cfg(any(test, feature = "fuzzing"))]` existiert und nie in Produktions-Builds vorhanden ist.

CI-Job:

```yaml
# .github/workflows/fuzz-nightly.yml
name: fuzz-nightly
on:
  schedule: [{ cron: "0 3 * * *" }]
  workflow_dispatch:
jobs:
  fuzz:
    runs-on: ubuntu-latest
    timeout-minutes: 90
    strategy:
      fail-fast: false
      matrix:
        target: [http_request_path, body_decoder, chunked_decoder, rules_yaml, flow_filter, audit_verify]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz --locked
      - run: cargo +nightly fuzz run ${{ matrix.target }} -- -max_total_time=600 -timeout=10 -rss_limit_mb=512
        working-directory: daemon
      - if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: fuzz-artifacts-${{ matrix.target }}
          path: daemon/fuzz/artifacts/${{ matrix.target }}/
```

Triage-Prozess (`daemon/fuzz/README.md`):
1. Artefakt herunterladen, lokal reproduzieren: `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<crash-file>`.
2. Minimieren: `cargo +nightly fuzz tmin <target> <crash-file>`.
3. Ursache beheben in der betroffenen Crate.
4. Minimierte Eingabe als Regressionstest ablegen: `daemon/crates/<crate>/tests/regressions/<target>-<kurzbeschreibung>.bin` plus ein `#[test]`, das die Datei lädt und den Aufruf ohne Panic erwartet.
5. Eingabe zusätzlich in den Corpus kopieren.
6. Bei Sicherheitsrelevanz (Speicherüberschreitung, Hänger im Proxy-Pfad) Eintrag in `SECURITY.md`-Changelog und, falls bereits released, Patch-Release nach HUM-060.

### Schritte
1. `cargo install cargo-fuzz` lokal, `daemon/fuzz/` mit `cargo fuzz init` anlegen, `Cargo.toml` wie oben, `daemon/Cargo.toml` Workspace um `exclude = ["fuzz"]` ergänzen (Fuzz-Crate braucht Nightly, darf den stabilen Workspace-Build nicht beeinflussen). Prüfen: `cargo build` im Workspace weiterhin stabil, `cargo +nightly fuzz list` zeigt sechs Targets.
2. Reine Einstiegsfunktionen freilegen: `parse_request_head`, `decode_body`, `ChunkedDecoder`, `parse_filter`, `AuditChain::verify_bytes`. Jede ohne IO, ohne tokio. Prüfen: bestehende Tests grün, Funktionen `pub` und dokumentiert.
3. Target `http_request_path` schreiben, 60 s laufen lassen. Prüfen: keine Funde oder Funde nach Triage behoben.
4. Targets `body_decoder` und `chunked_decoder` schreiben, mit `-timeout=10` laufen lassen. Erwartung: `body_decoder` findet in den ersten Minuten typischerweise die Ratio-Prüfung, wenn sie erst nach vollständiger Dekompression greift. Dann Decoder auf streaming umbauen (siehe HUM-057). Prüfen: 5 min ohne Fund.
5. Targets `rules_yaml`, `flow_filter`, `audit_verify` schreiben. Prüfen: 5 min ohne Fund.
6. Corpus-Seeds anlegen, `cargo +nightly fuzz cmin <target>` zum Minimieren, Ergebnis einchecken.
7. `fuzz-nightly.yml` anlegen, einmal per `workflow_dispatch` starten. Prüfen: alle sechs Matrix-Jobs grün.
8. `daemon/fuzz/README.md` mit Triage-Prozess schreiben.

### Tests
- `fuzz_targets_compile`: `cargo +nightly fuzz build` baut alle sechs Targets (läuft im Nightly-Job als erster Schritt implizit).
- `regressions_replay` je Crate: Jeder Regressionstest unter `tests/regressions/` lädt seine Datei und ruft die Zielfunktion auf, erwartet kein Panic. Beim Anlegen des Issues existiert je Target mindestens ein Regressionstest mit einem der Corpus-Seeds, damit der Mechanismus nachweislich funktioniert.
- `decode_ratio_bomb`: Eingabe ist ein gzip-Stream von 200 MiB Nullen (im Test erzeugt, nicht eingecheckt), `Limits { max_out: 8 MiB, max_ratio: 100 }`. Erwartung: `Err(DecodeError::RatioExceeded)` innerhalb von 200 ms, Speicherzuwachs unter 16 MiB (gemessen über `peak_rss` in einem `#[ignore]`-Test, der nur in CI unter Linux läuft).
- `chunked_huge_size_no_alloc`: Chunk-Größe `FFFFFFFFFFFFFFFF` liefert `Err(ChunkedError::SizeExceeded)` ohne Allokation dieser Größe.
- `filter_never_interpolates`: 20 Eingaben mit SQL-Metazeichen, keine erscheint im erzeugten SQL.
- `audit_flip_one_byte`: Für jede Byteposition einer 5-Zeilen-Kette einmal flippen, immer `Err`.

### Akzeptanzkriterien
- [ ] `cargo +nightly fuzz list` im Verzeichnis `daemon/` zeigt genau: `audit_verify`, `body_decoder`, `chunked_decoder`, `flow_filter`, `http_request_path`, `rules_yaml`.
- [ ] `cargo build --workspace` mit stabilem Toolchain läuft ohne Nightly.
- [ ] Jedes Target hat mindestens fünf Seed-Dateien unter `daemon/fuzz/corpus/<target>/`.
- [ ] Der Workflow `fuzz-nightly` wurde einmal manuell ausgelöst und alle sechs Jobs sind grün (Link im PR).
- [ ] `daemon/fuzz/README.md` enthält den sechsstufigen Triage-Prozess.
- [ ] Mindestens ein Regressionstest pro Target existiert und ist grün.
- [ ] Alle in Schritt 3 bis 5 gefundenen Abstürze sind behoben und als Regressionstest abgelegt (Liste im PR).
- [ ] `cargo clippy --workspace -- -D warnings` sauber, auch für die freigelegten Funktionen.

### Fallstricke
- Ein Target, das nur auf Panics prüft, findet keine Hänger. Immer `-timeout=10` mitgeben, sonst blockiert libFuzzer ewig auf einer quadratischen Eingabe. Der Chunked-Decoder und der Regex-Compiler sind die typischen Kandidaten.
- `-rss_limit_mb=512` ist Pflicht. Ohne Limit findet man Speicherbomben nicht, der Runner stirbt stattdessen mit OOM ohne Artefakt.
- Das Ratio-Limit kann nicht vor dem Dekomprimieren geprüft werden, weil die unkomprimierte Größe erst beim Dekomprimieren bekannt wird. Der Decoder muss streamend arbeiten und nach jedem Block prüfen, ob `out_len > in_consumed * max_ratio` oder `out_len > max_out`, und dann sofort abbrechen. Ein Decoder, der `decompress_to_vec` aufruft und danach die Länge prüft, ist falsch, auch wenn der Test grün wird, weil der Test-Input klein ist.
- `arbitrary` für Strings erzeugt gültiges UTF-8. Für `rules_yaml` und `flow_filter` zusätzlich das rohe `&[u8]`-Target mit `std::str::from_utf8(...).ok()` verwenden, damit auch ungültiges UTF-8 die Fehlerpfade trifft.
- Fuzz-Targets müssen deterministisch sein. Kein `SystemTime::now()`, kein Zufall, keine Umgebungsvariablen. `RuleSet::evaluate` bekommt eine feste `DateTime`, `SessionId::nil()`.
- Das Fuzz-Crate darf nicht in `[workspace.members]` stehen, sonst bricht `cargo test --workspace` auf Stable.
- Corpus-Dateien sind binär. `.gitattributes` mit `daemon/fuzz/corpus/** binary` setzen, sonst normalisiert Git Zeilenenden und die Seeds ändern sich.
- Die Funktion `parse_request_head` darf keine Kopie des hudsucker-Parsers sein. Sie ist der Einstieg, den auch der Produktionspfad benutzt (hyper liefert `Request<Body>`, daraus wird `RequestHead`; das Target füttert den Konstruktor aus rohen Bytes über `httparse`). Wenn Produktion und Fuzz-Target verschiedene Parser nutzen, ist das Fuzzing wertlos.

### Referenzen
- BACKLOG.md 4.4 (Proxy-Härtung), ADR-001, ADR-005
- cargo-fuzz Buch: https://rust-fuzz.github.io/book/cargo-fuzz.html
- libFuzzer-Optionen: https://llvm.org/docs/LibFuzzer.html#options
- h2 Rapid Reset (CVE-2023-44487) und CONTINUATION-Flood (CVE-2024-27316) als Motivation für gepinnte Versionen
- HUM-015 (Proxy-Kern), HUM-022 (Regel-Engine), HUM-032 (Filter-Syntax), HUM-050 (Audit-Kette)

---

## HUM-057 · Ressourcen-Limits und Backpressure
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-015, HUM-016, HUM-018, HUM-026, HUM-062 · Blockiert: HUM-060

### Kontext
Der Daemon hält Requests im Speicher, dekomprimiert fremde Bodies für die Vorschau und sendet Events an eine UI, die langsamer sein kann als der Proxy. Ohne harte Grenzen kann ein einziger Agent-Lauf (etwa ein `npm install` mit hunderten Requests oder ein 2-GB-Download) den Daemon zum Absturz bringen. BACKLOG.md 4.4 nennt Body-Caps, Dekompressions-Ratio und Timeouts. CONVENTIONS.md 3.5 legt die Defaults fest. Dieses Issue setzt alle Grenzen an einer Stelle um, macht sie konfigurierbar und beweist mit einem Lasttest, dass Proxy und UI sich nicht gegenseitig blockieren.

### Ziel
Alle Ressourcengrenzen sind Felder von `Config`, haben Tier `advanced` oder `expert`, gelten im Proxy, im Recorder und im IPC-Server, und jede Überschreitung endet in einem definierten Zustand mit `Diagnostic`. Ein Integrationstest feuert 1000 Flows in 10 Sekunden gegen den Daemon mit einem absichtlich langsamen gRPC-Subscriber und zeigt: kein Flow verloren, `Lagged` wird gesendet, p95-Latenz vom Proxy-Eingang bis zum `Held`-Event unter 50 ms, RSS des Daemons unter 300 MiB.

### Nicht-Ziel
Rate-Limiting pro Host (kommt mit Credential-Injection nach dem MVP). Disk-Quota für die Datenbank (Retention in HUM-051 reicht). Limits für die Terminal-Stream-Rate (HUM-042 hat chunked stream, das genügt).

### Betroffene Pfade
- `daemon/crates/config/src/limits.rs` (neu): `LimitsConfig` mit allen Feldern
- `daemon/crates/config/src/lib.rs` (geändert): `Config.limits: LimitsConfig`
- `daemon/crates/proxy/src/decode.rs` (geändert): streamender Decoder mit Ratio-Abbruch
- `daemon/crates/proxy/src/hold.rs` (geändert): Memory-Bound der HoldQueue
- `daemon/crates/proxy/src/timeouts.rs` (neu): Timeout-Anwendung auf hudsucker/hyper
- `daemon/crates/ipc/src/events.rs` (geändert): `broadcast` mit Kapazität aus Config, `Lagged`-Handling
- `daemon/crates/proxy/tests/backpressure.rs` (neu)
- `daemon/crates/proxy/tests/limits.rs` (neu)
- `docs/reference/limits.md` (neu, wird von HUM-059 eingebunden)

### Spezifikation

```rust
/// Alle harten Grenzen des Daemons. Jede Grenze hat einen Default,
/// der für einen Coding-Agenten auf einem Laptop passt.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct LimitsConfig {
    /// Maximale Größe eines Request-Bodys, der für die Moderation gepuffert wird. Größere Requests werden geblockt (BlockReason::BodyCap), außer eine Regel setzt `stream: true`.
    #[humanitl(tier = "advanced")] #[serde(default = "d_body_cap")] pub hold_body_cap_bytes: u64,          // 32 MiB
    /// Maximale dekomprimierte Größe für Vorschau und Findings-Scan.
    #[humanitl(tier = "advanced")] #[serde(default = "d_preview_cap")] pub preview_cap_bytes: u64,         // 8 MiB
    /// Maximales Verhältnis dekomprimiert zu komprimiert. Darüber bricht die Dekompression ab.
    #[humanitl(tier = "expert")] #[serde(default = "d_ratio")] pub max_decompress_ratio: u32,               // 100
    /// Gesamtspeicher für gleichzeitig gehaltene Request-Bodies. Wird er erreicht, werden neue Requests geblockt (BlockReason::HoldMemory).
    #[humanitl(tier = "expert")] #[serde(default = "d_hold_mem")] pub hold_memory_cap_bytes: u64,          // 256 MiB
    /// Maximale Anzahl gleichzeitig gehaltener Requests.
    #[humanitl(tier = "expert")] #[serde(default = "d_hold_max")] pub hold_max_flows: u32,                 // 200
    /// Timeout für den TCP/TLS-Verbindungsaufbau zum Upstream.
    #[humanitl(tier = "expert")] #[serde(default = "d_connect")] pub upstream_connect_timeout_secs: u32,   // 10
    /// Timeout, bis der Client alle Request-Header gesendet hat.
    #[humanitl(tier = "expert")] #[serde(default = "d_header")] pub client_header_timeout_secs: u32,       // 30
    /// Timeout, bis der Client den vollständigen Request-Body gesendet hat (gilt bis zum Cap).
    #[humanitl(tier = "expert")] #[serde(default = "d_body")] pub client_body_timeout_secs: u32,           // 120
    /// Leerlauf-Timeout einer Upstream-Antwort (Zeit zwischen zwei Chunks). Gilt auch für LLM-Streaming.
    #[humanitl(tier = "advanced")] #[serde(default = "d_idle")] pub response_idle_timeout_secs: u32,       // 300
    /// Kapazität des Event-Puffers pro gRPC-Subscriber. Bei Überlauf erhält der Subscriber `Lagged{n}`.
    #[humanitl(tier = "expert")] #[serde(default = "d_events")] pub event_buffer: u32,                     // 1024
    /// Maximale Anzahl gleichzeitiger Client-Verbindungen aus der Sandbox.
    #[humanitl(tier = "expert")] #[serde(default = "d_conns")] pub max_client_connections: u32,            // 256
}
```

Config-Schlüssel (überschreiben die in CONVENTIONS.md 3.5 genannten Einzelschlüssel, die dort genannten Namen `hold.body_cap_bytes`, `preview.cap_bytes`, `ipc.event_buffer` werden zu Aliassen über `#[serde(alias)]` beibehalten): `limits.hold_body_cap_bytes`, `limits.preview_cap_bytes`, `limits.max_decompress_ratio`, `limits.hold_memory_cap_bytes`, `limits.hold_max_flows`, `limits.upstream_connect_timeout_secs`, `limits.client_header_timeout_secs`, `limits.client_body_timeout_secs`, `limits.response_idle_timeout_secs`, `limits.event_buffer`, `limits.max_client_connections`.

Neue `BlockReason`-Varianten in `humanitl-core`: `HoldMemory`, `HoldMaxFlows`, `ClientTimeout`. Neue `DiagnosticCode`s: `LIMIT_001` (Body-Cap überschritten, fix: `ChangeSetting{limits.hold_body_cap_bytes}` oder `AddRule{stream: true}`), `LIMIT_002` (Hold-Speicher voll, why nennt Anzahl gehaltener Flows, fix: keiner, Hinweis „Queue abarbeiten"), `LIMIT_003` (Dekompressions-Ratio, why nennt Ratio, fix: keiner), `LIMIT_004` (Upstream-Timeout, fix: `ChangeSetting`), `LIMIT_005` (Client-Timeout), `LIMIT_006` (Event-Puffer übergelaufen, nur `Info`, UI synchronisiert per `ListFlows`).

Streamender Decoder:

```rust
pub struct Limits { pub max_out: u64, pub max_ratio: u32 }
pub enum DecodeError { RatioExceeded { consumed: u64, produced: u64 }, OutputCap { max: u64 }, Corrupt(String), Unsupported(String) }
/// Dekomprimiert blockweise (64 KiB Eingabe pro Schritt) und prüft nach jedem Block:
/// produced > max_out  => OutputCap
/// produced > consumed * max_ratio && consumed >= 4096  => RatioExceeded
/// Die Schwelle 4096 verhindert Fehlalarme bei winzigen Eingaben, deren Header allein schon eine hohe Ratio ergeben.
pub fn decode_body(enc: Encoding, input: &[u8], limits: &Limits) -> Result<Bytes, DecodeError>;
```

HoldQueue-Memory-Bound: `HoldQueue` führt `AtomicU64 held_bytes` und `AtomicU32 held_count`. `hold()` prüft vor dem Einfügen `held_bytes + body.size <= hold_memory_cap_bytes && held_count < hold_max_flows`, sonst `Err(HoldRejected::{Memory, Count})`, was der Proxy in `Decision::Block{HoldMemory|HoldMaxFlows}` übersetzt. Beim Entscheiden werden beide Zähler dekrementiert. Der `Held`-Event trägt `queue_bytes` und `queue_count` mit, damit die UI eine Auslastungsanzeige im Header rendern kann (Statusleiste, HUM-058).

Timeouts werden über `tokio::time::timeout` um die jeweilige hyper-Phase gelegt, nicht über hudsucker-Optionen, weil hudsucker 0.25 nur den Connect-Timeout des Clients konfiguriert. `response_idle_timeout_secs` wird über einen `StreamExt::timeout` auf den Response-Body-Stream angewendet; bei Ablauf wird der Stream mit einem letzten Chunk `ResponseChunk{error: LIMIT_004}` beendet und der Flow als `Responded{status: 504}` aufgezeichnet. Dem Client wird die Verbindung geschlossen.

Backpressure: Der IPC-Server erzeugt pro `Subscribe` einen `broadcast::Receiver` mit `event_buffer`. Bei `RecvError::Lagged(n)` sendet der Server `FlowEvent::Lagged{n}` und setzt fort. Der Proxy wartet nie auf einen Subscriber. Die Hold-Entscheidung kommt aus der `HoldQueue`, unabhängig vom Event-Stream, also kann eine langsame UI nie einen Request verzögern, nur die Anzeige.

### Schritte
1. `LimitsConfig` anlegen, in `Config` einhängen, Aliasse setzen, `humanitl config schema` zeigt alle elf Felder mit Tier und Beschreibung. Prüfen: Schema-Snapshot-Test in `humanitl-config` aktualisiert.
2. Decoder auf blockweises Streaming umbauen (gzip via `flate2::read::MultiGzDecoder` mit `Read::take`-Schleife, brotli via `brotli::Decompressor`, deflate analog). Prüfen: `decode_ratio_bomb`-Test aus HUM-056 grün, Fuzz-Target `body_decoder` 5 min ohne Fund.
3. HoldQueue-Zähler und Ablehnung einbauen, `Held`-Event um `queue_bytes`, `queue_count` erweitern (Proto-Änderung, Feld-Nummern anhängen, nie umnummerieren). Prüfen: Unit-Test `hold_rejects_over_memory`.
4. Timeouts einbauen. Prüfen: Integrationstest `upstream_idle_timeout` mit axum-Upstream, der nach dem ersten Chunk 400 s schweigt (Test setzt `response_idle_timeout_secs = 1`).
5. Event-Puffer aus Config ziehen, `Lagged`-Pfad testen. Prüfen: `subscriber_lag_gets_lagged_event`.
6. Backpressure-Lasttest schreiben (siehe Tests). Prüfen: Messwerte im Test-Output, Schwellen als Assertion.
7. `docs/reference/limits.md` mit Tabelle aller Limits, Defaults, Wirkung, Diagnostic-Code schreiben.

### Tests
- `hold_rejects_over_memory` (Unit, proxy): `hold_memory_cap_bytes = 1 MiB`, drei Requests à 400 KiB. Erwartung: erste zwei gehalten, dritter `Block{HoldMemory}`, nach Entscheidung des ersten passt der vierte wieder.
- `hold_rejects_over_count` (Unit): `hold_max_flows = 2`, dritter Request `Block{HoldMaxFlows}`.
- `body_cap_blocks_with_diagnostic` (Integration): 40-MiB-POST an erlaubten Host. Erwartung: Client erhält 403 mit `reason: body_cap`, Flow in DB mit `decision = block`, `Diagnostic LIMIT_001` im Event.
- `body_cap_stream_rule_passes` (Integration): gleiche Anfrage, Regel `allow` mit `stream: true`. Erwartung: Request wird nach Header-Freigabe gestreamt, Flow trägt `streamed = true`.
- `upstream_idle_timeout` (Integration): siehe Schritt 4. Erwartung: Client-Verbindung nach 1 s geschlossen, Flow `Responded{504}`, Event enthält `LIMIT_004`.
- `client_header_timeout` (Integration): Client sendet `GET / HTTP/1.1\r\n` und schweigt. Erwartung: Verbindung nach `client_header_timeout_secs` (Test: 1) geschlossen, kein Flow angelegt (kein Head, also kein `Received`), Log-Zeile mit `LIMIT_005`.
- `subscriber_lag_gets_lagged_event` (Integration, ipc): `event_buffer = 8`, Subscriber liest nicht, 50 Flows erzeugen, dann lesen. Erwartung: erstes gelesenes Event ist `Lagged{n}` mit `n >= 42`, danach aktuelle Events.
- `backpressure_1000_flows` (Integration, `#[ignore]` lokal, in CI aktiv): Fake-Agent (in-process, `reqwest` mit Proxy) sendet 1000 GET an axum-Upstream über den Proxy in 10 s (100/s), Regel `allow` für den Host, ein gRPC-Subscriber verarbeitet jedes Event mit `sleep(20 ms)`. Messung: pro Flow Zeit von Proxy-Eingang bis `Held` bzw. `Decided`-Event im Daemon (nicht beim Subscriber), RSS über `/proc/self/status` alle Sekunde. Erwartung: alle 1000 Flows in DB mit `state = recorded`, p95 unter 50 ms, max RSS unter 300 MiB, Subscriber hat mindestens ein `Lagged` erhalten und danach per `ListFlows(since)` alle 1000 gesehen.

### Akzeptanzkriterien
- [ ] `humanitl config schema | jq '.properties.limits.properties | keys'` liefert genau die elf Feldnamen.
- [ ] `humanitl config get limits.hold_body_cap_bytes` liefert `33554432` bei Default.
- [ ] Alte Schlüssel `hold.body_cap_bytes`, `preview.cap_bytes`, `ipc.event_buffer` in einer `config.toml` werden weiterhin gelesen (Test `legacy_alias_keys`).
- [ ] Alle acht Tests oben grün, `backpressure_1000_flows` in CI mit ausgegebenen Messwerten im Job-Log.
- [ ] Fuzz-Target `body_decoder` läuft 5 min ohne Fund (Nachweis im PR).
- [ ] `docs/reference/limits.md` existiert mit Tabelle aller elf Limits.
- [ ] Sechs neue Diagnostic-Codes `LIMIT_001` bis `LIMIT_006` im Code-Register (HUM-063) mit `why`-Text in `en` und `de`.
- [ ] Der Header der UI zeigt nach HUM-058 die Queue-Auslastung; hier reicht, dass `Held`-Events die Felder `queue_bytes`, `queue_count` tragen (Proto-Test).

### Fallstricke
- Ratio-Prüfung erst nach vollständigem Dekomprimieren ist falsch, siehe HUM-056. Auch eine Prüfung über den `Content-Length`-Header ist falsch, weil der bei chunked fehlt und bei Lügen des Servers nichts hilft.
- Timeout auf den ganzen Response statt auf den Leerlauf zwischen Chunks würde LLM-Streaming über fünf Minuten abbrechen. Es ist ein Idle-Timeout, kein Gesamt-Timeout.
- `broadcast::Sender::send` liefert `Err`, wenn kein Receiver existiert. Das ist kein Fehler; der Proxy ignoriert ihn. Ein `unwrap()` an dieser Stelle lässt den Daemon abstürzen, sobald die UI sich trennt.
- `held_bytes` muss beim Timeout ebenso dekrementiert werden wie bei einer Nutzerentscheidung, sonst läuft der Zähler nach Stunden voll.
- Die Proto-Felder `queue_bytes`, `queue_count` bekommen neue Feldnummern am Ende der Message. Bestehende Nummern nie ändern.
- Lasttest nicht mit `tokio::test(flavor = "current_thread")` laufen lassen, sonst misst man das Test-Runtime, nicht den Daemon. `flavor = "multi_thread", worker_threads = 4`.
- RSS-Messung über `/proc/self/status` ist Linux-spezifisch; Test unter `#[cfg(target_os = "linux")]`.
- `max_client_connections` wird über ein `Semaphore` vor `accept` durchgesetzt, nicht nach dem Accept, sonst hält man Verbindungen offen, die man gleich wieder schließt.

### Referenzen
- BACKLOG.md 4.4, ADR-005, ADR-004 (HoldQueue), CONVENTIONS.md 3.5
- hyper Timeouts: https://docs.rs/hyper-util/latest/hyper_util/rt/tokio/index.html und `hyper::server::conn::http1::Builder::header_read_timeout`
- tokio broadcast Lagged: https://docs.rs/tokio/latest/tokio/sync/broadcast/error/enum.RecvError.html
- flate2 MultiGzDecoder: https://docs.rs/flate2/latest/flate2/read/struct.MultiGzDecoder.html

---

## HUM-058 · Fehlerpfade im UI
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-063, HUM-019, HUM-040, HUM-041, HUM-042, HUM-045, HUM-068 · Blockiert: HUM-060

### Kontext
Prinzip 7 in BACKLOG.md verlangt, dass jeder nicht-grüne Zustand einen Grund und eine Aktion trägt. ADR-012 gibt dafür den Typ `Diagnostic` vor, HUM-063 hat ihn eingeführt, HUM-068 hat ihn für den Sandbox-Screen umgesetzt. Was fehlt, ist eine vollständige Abdeckung aller Zustände, in die die App geraten kann, insbesondere solche, die nicht in einem Screen wohnen (Daemon weg, Verbindung verloren, Versionskonflikt) und solche im Intercept-Screen, die Usability-Review 6 aufgezählt hat (Timeout, große Bodies, Binär, WebSocket, Streaming, Nutzer war weg).

### Ziel
Eine Tabelle definiert jeden Fehler- und Randzustand der App mit Auslöser, UI-Reaktion, Ort der Darstellung, Diagnostic-Code und Test. Jeder Eintrag ist umgesetzt. Ein Golden-Test pro sichtbarem Zustand existiert. Ein Nutzer kann in keinem dieser Zustände falsch verstehen, was passiert ist und was er tun kann. Kein Zustand wird als Modal dargestellt, außer den drei in BACKLOG.md Abschnitt 5 genannten destruktiven.

### Nicht-Ziel
Neue Diagnostics im Daemon (die kommen aus den jeweiligen Issues). Crash-Reporting an einen Server (gibt es nicht, das Tool telefoniert nicht nach Hause). Wiederherstellung von Editor-Entwürfen über einen Neustart der App hinweg (Entwürfe leben im Daemon-Speicher pro Flow, HUM-047; App-Neustart bei laufendem Daemon behält sie, Daemon-Neustart nicht).

### Betroffene Pfade
- `app/lib/core/ipc/connection_state.dart` (neu): `ConnectionState` sealed class und `connectionStateProvider`
- `app/lib/core/ui/diagnostic_card.dart` (neu): `HDiagnosticCard` als einheitlicher Renderer
- `app/lib/core/ui/fix_action_button.dart` (neu): rendert `FixAction` als Button mit passendem Verb
- `app/lib/features/setup/setup_screen.dart` (geändert): Zustand „Daemon weg" und „Version inkompatibel"
- `app/lib/features/intercept/widgets/flow_card_states.dart` (neu): Timeout-Banner, Large-Body, Binary, WebSocket, Streaming
- `app/lib/features/intercept/widgets/waiting_banner.dart` (neu): „Agent wartet seit …"
- `app/lib/app.dart` (geändert): Statusleiste mit Verbindungs- und Queue-Auslastung, Lifecycle-Hook für Rückkehr-Banner
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb` (geändert)
- `app/test/goldens/states/*.png` (neu)
- `app/test/features/intercept/flow_card_states_test.dart` (neu)
- `app/test/core/connection_state_test.dart` (neu)
- `app/lib/core/ipc/fake_daemon_client.dart` (geändert): Szenarien für jeden Zustand

### Spezifikation

`ConnectionState`:

```dart
sealed class ConnectionState {
  const ConnectionState();
}
class Connecting extends ConnectionState { final int attempt; }
class Connected extends ConnectionState { final DaemonInfo info; }
class Disconnected extends ConnectionState { final Diagnostic diagnostic; final DateTime since; }
class Incompatible extends ConnectionState { final String daemonProto; final String appProto; }
```

`connectionStateProvider` ist ein `Notifier`, der `GetInfo` beim Start ruft, bei Erfolg `Subscribe` öffnet, bei Stream-Ende oder Fehler nach Backoff (1 s, 2 s, 4 s, max 10 s) erneut verbindet und in `Disconnected` bleibt, solange kein Erfolg. Alle datenhaltenden Provider (`flowsProvider`, `rulesProvider`, `sandboxStatusProvider`) leeren sich bei Übergang nach `Disconnected`, damit nie veraltete Daten als live erscheinen.

`HDiagnosticCard(diagnostic, {compact: bool})`: Icon nach Severity (Info: `info`, Warning: `triangle-alert` in amber, Error: `triangle-alert` in orange #F0784F, Blocking: `shield-x` in rot), Titel 13/500, `why` 12 fg-1, darunter `FixActionButton` falls `fix != null`, rechts „Diagnose kopieren" (kopiert `code`, `title`, `why`, Zeitstempel, Daemon-Version als Text). `compact` reduziert auf eine Zeile mit Icon, Titel und Fix-Button.

`FixActionButton` Verben pro `FixAction`: `SetEnv` „Fix kopieren" (kopiert `export KEY=VALUE`), `AddRule` „Regel anlegen" (öffnet Regel-Sheet vorausgefüllt), `InstallService` „Dienst installieren", `ChangeSetting` „Einstellung öffnen" (springt in Settings mit Feld fokussiert), `CopyCommand` „Befehl kopieren", `OpenUrl` „Doku öffnen", `RemountReadOnly` „Nur lesend mounten".

Zustandstabelle (verbindlich, jede Zeile ist ein Akzeptanzkriterium):

| Nr | Zustand | Auslöser | UI-Reaktion | Ort | Code | Test |
|---|---|---|---|---|---|---|
| 1 | Daemon nicht erreichbar beim Start | `GetInfo` schlägt fehl (Socket fehlt oder Connection refused) | Ganze App zeigt Setup-Screen mit Checkliste, Punkt „Daemon" rot, `HDiagnosticCard` mit Fix `InstallService` oder `CopyCommand("systemctl --user start humanitld")`, Verbindungsversuche laufen sichtbar weiter („Versuch 3 …") | Setup | `DAEMON_001` | `connection_state_test: start_without_daemon_shows_setup` + Golden `states/daemon_missing` |
| 2 | Verbindung verloren im Betrieb | Stream endet, gRPC `UNAVAILABLE` | Statusleiste rot „Verbindung verloren, verbinde neu …", alle Screens zeigen ihren Inhalt ausgegraut mit Overlay-Zeile, nach 5 s ohne Erfolg Wechsel zu Setup wie Nr. 1. Queue wird geleert, kein Flow bleibt als „held" sichtbar | Statusleiste, dann Setup | `DAEMON_002` | `connection_state_test: stream_end_reconnects_then_setup` |
| 3 | Proto-Version inkompatibel | `GetInfo.proto_version` Major größer als App | Setup-Screen, `Incompatible`-Karte mit beiden Versionen, Fix `OpenUrl(docs/upgrade)`; App verweigert `Subscribe` | Setup | `DAEMON_003` | `connection_state_test: major_mismatch_incompatible` + Golden |
| 4 | Token ungültig | gRPC `UNAUTHENTICATED` | Wie Nr. 1 mit `why` „Token-Datei stimmt nicht mit Daemon überein", Fix `CopyCommand("systemctl --user restart humanitld")` | Setup | `DAEMON_004` | `connection_state_test: unauthenticated` |
| 5 | Sandbox-Start fehlgeschlagen | `SandboxEvent.Failed{diagnostic}` | Inline im Sandbox-Screen (HUM-068), Start-Button bleibt deaktiviert, Isolation-Ring grau | Sandbox | aus Daemon (`SANDBOX_*`) | HUM-068 deckt ab; hier nur Golden `states/sandbox_failed` |
| 6 | Isolation-Check fehlgeschlagen | `CheckResult.passed == false` | Ring-Segment rot, Zeile rot mit `evidence` und Diagnostic, Start deaktiviert, nie „trotzdem starten" | Sandbox, Header-Ring | `SANDBOX_010..012` | Golden `states/isolation_failed` |
| 7 | Request-Timeout | `FlowEvent.TimedOut` | Karte bekommt Banner grau „Blockiert (Timeout nach 300 s). Der Agent kann es erneut versuchen.", Entwurf im Editor bleibt erhalten mit Button „Als Regel übernehmen", Karte verlässt Queue nach 5 s oder sofort bei Klick | Intercept-Karte, dann History | keiner (kein Fehler, ein Ergebnis) | `flow_card_states_test: timeout_banner_keeps_draft` + Golden |
| 8 | Großer Body (über `preview_cap_bytes`) | `BodyRef.size > cap` | Body-Sektion zeigt Größe, ersten 64 KiB als Raw, Zeile „Vorschau auf 64 KiB begrenzt · Findings-Scan: 8 MiB geprüft, 0 Funde" oder „Scan übersprungen (über Cap)", kein Editor, Allow/Block aktiv, Edit deaktiviert mit Tooltip | Intercept-Karte | `LIMIT_003` bei Ratio, sonst keiner | `flow_card_states_test: large_body_no_editor` + Golden |
| 9 | Body über `hold_body_cap_bytes` | Daemon hat bereits geblockt | Erscheint nur in History mit Decision `Block{BodyCap}` und `HDiagnosticCard(LIMIT_001)` compact mit Fix `AddRule{stream: true}` | History-Detail | `LIMIT_001` | `history_detail_test: body_cap_row_has_fix` |
| 10 | Binärer Body | Content-Type nicht textuell oder Body enthält NUL in ersten 8 KiB | Hex-Ansicht (16 Bytes pro Zeile, Offset, ASCII-Spalte), MIME-Vermutung aus Magic Bytes, Zeile „Findings-Scan nur auf druckbaren Strings", Edit deaktiviert | Intercept-Karte | keiner | `flow_card_states_test: binary_body_hex_view` + Golden |
| 11 | WebSocket-Upgrade | `RequestKey.upgrade == WebSocket` | Karte trägt Chip „WebSocket" violett-umrandet, Hinweiszeile „Öffnet eine dauerhafte Verbindung zu {host}. Nachrichten danach werden aufgezeichnet, nicht angehalten.", Allow-Button heißt „Verbindung erlauben" | Intercept-Karte | keiner | `flow_card_states_test: websocket_card_copy` + Golden |
| 12 | Streaming-Response | `ResponseHeaders` mit `text/event-stream` oder chunked ohne Length | History-Detail zeigt „streaming … 1,2 MB" live mit Zähler, nach Ende Gesamtgröße; kein Halt | History-Detail | keiner | `history_detail_test: streaming_counter_updates` |
| 13 | Nutzer war weg | App-Fenster verliert Fokus während Queue > 0, oder Queue geht 0 → 1 bei unfokussiertem Fenster | Tray-Badge mit Zähler, Desktop-Notification (HUM-034), bei Rückkehr Banner oben „Der Agent wartet seit 4 min · 3 Anfragen" mit Klick auf älteste Karte; Banner verschwindet, wenn Queue leer | Intercept, Tray | keiner | `waiting_banner_test: shows_on_focus_return` |
| 14 | TLS vom Tool abgelehnt | Daemon-Diagnostic `TLS_001` (HUM-045) | Karte im Intercept-Feed (nicht in der Queue, sie hält nichts) mit Fix `SetEnv` | Intercept-Feed | `TLS_001` | HUM-045 deckt ab; Golden `states/tls_rejected` |
| 15 | LLM-Server unerreichbar | Passthrough-Flow scheitert mit Connect-Fehler | Isolation-Panel-Zeile „LLM" wird rot mit `LLM_001`, Fix `ChangeSetting{llm.endpoint}`, Intercept-Feed zeigt Flow mit `Responded{502}` | Sandbox, Feed | `LLM_001` | `sandbox_screen_test: llm_unreachable_row` |
| 16 | Queue-Auslastung hoch | `Held.queue_count > 0.8 * hold_max_flows` oder `queue_bytes > 0.8 * cap` | Statusleiste zeigt Auslastung amber „Queue 412/200 · 210 MB", ab 100 % rot mit `LIMIT_002` compact | Statusleiste | `LIMIT_002` | `status_bar_test: queue_pressure_colors` |
| 17 | Regel-Konflikt beim Anlegen | Nutzer legt Regel an, die durch frühere Regel nie erreicht wird | Regel-Sheet zeigt vor dem Speichern amber „Wird von Regel #3 (`block **.example.com`) überdeckt", Speichern erlaubt | Regel-Sheet | `RULES_001` | `rule_sheet_test: shadowed_rule_warning` |
| 18 | Findings ungelöst beim Senden | Nutzer drückt Allow bei Findings > 0 | Inline-Pause (HUM-049), kein Modal | Aktionsleiste | keiner | HUM-049 deckt ab |
| 19 | Terminal-Stream abgerissen | `Terminal`-Stream endet ohne Sandbox-Stop | Terminal zeigt letzte Zeile „[Humanitl] Verbindung zum Terminal verloren, verbinde neu …" in fg-2, Reconnect mit Resize, Puffer bleibt | Terminal | `TERM_001` | `terminal_test: reconnect_keeps_buffer` |
| 20 | Datenbank-Fehler | Daemon meldet `RECORDER_001` (Disk voll, DB gesperrt) | Statusleiste rot „Aufzeichnung gestört", Intercept bleibt bedienbar, jede neue Karte trägt Hinweis „wird nicht aufgezeichnet", Allow-Button bekommt Bestätigungs-Pause (einmalig pro Session) | Statusleiste, Karte | `RECORDER_001` | `status_bar_test: recorder_failure_marks_cards` |

Der `FakeDaemonClient` bekommt für jede Zeile ein Szenario `FakeScenario.<name>`, das per `--dart-define=HUMANITL_FAKE=<name>` beim App-Start gewählt wird, damit Golden-Tests und manuelle Prüfung ohne echten Daemon möglich sind.

### Schritte
1. `ConnectionState` und Provider mit Backoff bauen, Provider-Reset bei `Disconnected`. Prüfen: `connection_state_test` Nr. 1 bis 4 grün.
2. `HDiagnosticCard` und `FixActionButton` in `packages/ui` bzw. `core/ui` bauen, alle sieben `FixAction`-Verben mit ARB-Schlüsseln `fix_set_env`, `fix_add_rule`, `fix_install_service`, `fix_change_setting`, `fix_copy_command`, `fix_open_url`, `fix_remount_ro`. Prüfen: Widget-Test rendert jede Variante mit korrektem Verb in `en` und `de`.
3. Setup-Screen-Zustände Nr. 1, 3, 4 anbinden. Prüfen: Goldens.
4. Statusleiste mit Verbindung und Queue-Auslastung (Nr. 2, 16, 20). Prüfen: `status_bar_test`.
5. Karten-Zustände Nr. 7, 8, 10, 11 in `flow_card_states.dart`. Prüfen: vier Goldens, Tests.
6. History-Detail Nr. 9, 12. Prüfen: Tests.
7. Rückkehr-Banner Nr. 13 mit `WidgetsBindingObserver.didChangeAppLifecycleState` plus `window_manager` Fokus-Events. Prüfen: `waiting_banner_test`.
8. Regel-Sheet Nr. 17 über `Rules(dry_run)` mit der neuen Regel an Position und Auswertung, ob eine frühere Regel jeden Testfall der neuen bereits fängt. Prüfen: Test.
9. Terminal Nr. 19. Prüfen: Test.
10. `FakeScenario` für alle 20 Zeilen, Golden-Lauf `flutter test --update-goldens` einmal, danach Goldens eingecheckt.

### Tests
Alle in der Tabelle genannten Tests. Zusätzlich:
- `diagnostic_card_all_fix_actions` (Widget): sieben `FixAction`-Varianten, je Verb-Text in `en` und `de` per `find.text`.
- `no_modal_for_diagnostics` (Widget): Für jedes `FakeScenario` App starten, `find.byType(Dialog)` und shadcn `AlertDialog` ist leer, außer bei Szenarien `block_all_confirm`, `delete_forever_rule`, `stop_running_sandbox`.
- Goldens unter `app/test/goldens/states/`: `daemon_missing`, `incompatible`, `sandbox_failed`, `isolation_failed`, `timeout_banner`, `large_body`, `binary_body`, `websocket_card`, `tls_rejected`, `queue_pressure`, `recorder_failure`, je dark und light.

### Akzeptanzkriterien
- [ ] Jede der 20 Tabellenzeilen hat einen grünen Test (Liste im PR mit Zeilennummer und Testname).
- [ ] `grep -r "showDialog\|AlertDialog" app/lib/` findet nur die drei destruktiven Stellen (Block-all, Forever-Regel löschen, Sandbox stoppen) und die Regel-Sheet-Implementierung nutzt `Sheet`, nicht `Dialog`.
- [ ] `flutter run -d linux --dart-define=HUMANITL_FAKE=daemon_missing` zeigt den Setup-Screen mit Diagnostic-Karte und laufendem Verbindungszähler.
- [ ] Alle 22 Goldens (11 Zustände × dark/light) eingecheckt und in CI grün.
- [ ] Bei `Disconnected` ist `heldFlowsProvider` leer (Test `providers_reset_on_disconnect`).
- [ ] Jeder Diagnostic-Code aus der Tabelle hat `why` in `app_en.arb` und `app_de.arb` (Schlüssel `diag_<code>_why`), Lint-Skript `tool/check_diag_l10n.dart` prüft Vollständigkeit gegen `daemon/crates/core-types/src/diagnostics/codes.rs` und läuft in CI.
- [ ] `flutter analyze` sauber.

### Fallstricke
- Reconnect-Backoff ohne Obergrenze führt nach Stunden zu Minuten-Wartezeiten. Max 10 s.
- Bei `Disconnected` die Provider nicht leeren heißt, dass eine Karte „held" bleibt, obwohl der Daemon sie längst per Timeout geblockt hat. Der Nutzer klickt Allow ins Leere. Deshalb ist Leeren Pflicht, auch wenn es kurz flackert.
- Rückkehr-Banner darf nicht bei jedem Fokuswechsel erscheinen, nur wenn zwischen Fokusverlust und Rückkehr mindestens ein Flow angekommen ist oder älter als 60 s wurde.
- Hex-Ansicht für 50 MiB Body darf nicht den ganzen Body rendern. Nur den Vorschau-Ausschnitt, virtualisiert.
- `find.byType(Dialog)` findet shadcn-Dialoge nicht zwingend, weil shadcn eigene Overlay-Typen nutzt. Den Wrapper in `packages/ui` so bauen, dass jeder Dialog über `HModal` geht, und im Test `find.byType(HModal)` prüfen.
- Streaming-Zähler in Nr. 12 nicht bei jedem Chunk `setState` auslösen, sondern maximal 10 Hz throttlen, sonst friert die History bei LLM-Streaming ein.
- Die Regel-Konflikt-Prüfung (Nr. 17) ist eine Heuristik über die Testfälle im Dry-Run, kein formaler Beweis. Text sagt „wird überdeckt" nur, wenn alle generierten Testfälle der neuen Regel von einer früheren gefangen werden; sonst schweigen.

### Referenzen
- BACKLOG.md Prinzip 7, ADR-012, Abschnitt 5 (Modal-Regel), Usability-Review 6 (Fehler und Randfälle)
- CONVENTIONS.md 3.2 (`Diagnostic`, `FixAction`), 3.9 (Provider-Namen)
- HUM-034 (Notification, Tray), HUM-045, HUM-047, HUM-049, HUM-063, HUM-068

---

## HUM-059 · Dokumentation
Sprint: 5 · Größe: M · Abhängigkeiten: alle vorherigen · Blockiert: HUM-060

### Kontext
Ein Sicherheitstool, dessen Argument nicht in fünf Minuten lesbar ist, wird nicht vertraut. BACKLOG.md verlangt `SECURITY.md`, `THREAT-MODEL.md`, `DESIGN.md`, Regel-Referenz, Agent-Profil-Anleitung. HUM-007 hat Entwürfe geliefert, seitdem haben sich Details geändert (Shim, DNS nach Allow, Limits, CLI). Dieses Issue bringt alle Dokumente auf den Stand des Codes und macht sie zur Voraussetzung des Releases.

### Ziel
Ein neuer Nutzer versteht in fünf Minuten aus dem README, was Humanitl tut und warum es sicher ist, installiert es in zehn Minuten, und findet für jede Regel-, Config- und CLI-Frage eine Referenzseite. Ein Security-Reviewer findet in `SECURITY.md` und `THREAT-MODEL.md` die vollständige Argumentation inklusive der ehrlichen Grenzen. Ein Contributor findet in `DESIGN.md`, wie das UI aussieht und warum.

### Nicht-Ziel
Eine Website oder ein Doku-Generator (mdBook, Docusaurus). Markdown im Repo reicht für 0.1. Video oder animierte GIFs. Übersetzung der Doku ins Deutsche (UI ist zweisprachig, Doku bleibt Englisch, weil die Community englisch liest; Ausnahme: `README.de.md` als Kurzfassung, eine Seite).

### Betroffene Pfade
- `README.md` (neu)
- `README.de.md` (neu, eine Seite)
- `docs/SECURITY.md` (geändert, final)
- `docs/THREAT-MODEL.md` (geändert, final)
- `docs/DESIGN.md` (neu)
- `docs/reference/rules.md` (neu)
- `docs/reference/config.md` (generiert aus Schema, neu)
- `docs/reference/cli.md` (generiert aus clap, neu)
- `docs/reference/limits.md` (aus HUM-057, eingebunden)
- `docs/reference/diagnostics.md` (generiert aus Code-Register, neu)
- `docs/guides/agent-profiles.md` (neu)
- `docs/guides/install.md` (neu)
- `docs/guides/first-session.md` (neu)
- `docs/adr/` (bestehend, Index `docs/adr/README.md` neu)
- `daemon/xtask/src/docs.rs` (neu): Generator für config/cli/diagnostics
- `.github/workflows/docs-check.yml` (neu)
- `SECURITY.md` im Repo-Root (neu, GitHub-Konvention, verweist auf `docs/SECURITY.md` und enthält den Meldeweg)

### Spezifikation

**README.md** Pflichtabschnitte in dieser Reihenfolge:
1. Einzeiler: „Humanitl lets a sandboxed AI coding agent use the internet, one approved request at a time."
2. Screenshot des Intercept-Screens (dark, `docs/img/intercept.png`, 1600 px breit, aus HUM-054-Golden-Setup oder echtem Lauf).
3. „Why" (drei Sätze: lokale LLMs, sensible Daten, kein Internet bisher).
4. „The three guarantees" wörtlich aus BACKLOG.md 4.1 mit den drei Prüfbefehlen.
5. „What it does not protect against" (Link auf THREAT-MODEL, die deklarierten Seitenkanäle in drei Zeilen).
6. „Install" (deb, AppImage, `systemctl --user enable --now humanitld`, Link auf `docs/guides/install.md`).
7. „First session" (fünf Schritte, Link auf `docs/guides/first-session.md`).
8. „CLI" (drei Beispiele: `humanitl run --profile llm-only`, `humanitl rules test https://…`, `humanitl audit verify`).
9. „How it works" (ASCII-Diagramm aus BACKLOG.md 3.1).
10. „Status" (0.1.0, was im MVP ist, was nicht, Link auf BACKLOG.md Abschnitt 9).
11. „Contributing" (Link `CONTRIBUTING.md`, Hinweis auf `backlog/`).
12. „License" (GPL-3.0).

**docs/SECURITY.md** Gliederung:
1. The claim (der Sicherheitssatz aus BACKLOG.md 0).
2. Guarantee 1: No network interface (Mechanismus `--unshare-all`, was das kernelseitig bedeutet, Prüfbefehl, Escape-Test ESC-1/ESC-2).
3. Guarantee 2: Exactly one door (Datei-Bind des Sockets, warum das Verzeichnis nie gebunden wird, Prüfbefehl, ESC-2).
4. Guarantee 3: No new doors (Shim-Ablauf: Bridge starten, `PR_SET_NO_NEW_PRIVS`, seccomp mit TSYNC, exec; Liste der verbotenen Syscalls; Prüfbefehl; ESC-1).
5. Declared side channels (Tabelle aus BACKLOG.md 4.2 mit aktuellem Stand).
6. The proxy (CA-Handhabung, Speicherorte, Host-Trust-Store nie, systemd-Härtung, Limits, Fuzzing).
7. The rules engine (Normalisierung, Label-Globs, Authority-Konsistenz, DNS nach Allow, IP-Literale).
8. The audit log (was die Kette beweist, was nicht, Head-Anchoring).
9. What we verify in CI (fünf Escape-Tests mit Link auf `tests/escape/`).
10. Reporting a vulnerability (E-Mail-Adresse des Maintainers, PGP-Key-Fingerprint, 90-Tage-Disclosure, keine Bug-Bounty).
11. Changelog of security-relevant changes (Liste, zunächst leer bis auf „0.1.0 initial").

**docs/THREAT-MODEL.md** Gliederung: Assets (Projektdaten, Credentials, LLM-Prompts, Audit-Log, CA-Key), Attacker (a, b, c aus BACKLOG.md 4.3 mit Fähigkeiten), Trust boundaries (Diagramm mit Sandbox, Daemon, UI, LLM-Host, Internet), Attack surface (Tabelle aus Security-Review Abschnitt 1 mit zwölf Kanälen, Severity, Mitigation, Status im MVP: mitigated / declared / open), Out of scope (Host-Kompromittierung, physischer Zugriff, bösartiger Nutzer), Residual risks (LLM-Host geteilt, `/work`-Exfiltration, menschlicher Klassifikator).

**docs/DESIGN.md** Gliederung: Direction „Airlock" (Mood, Referenzen, Anti-Referenzen), Tokens (vollständige Tabellen für Farbe dark/light, Typo, Spacing, Radius, exakt aus BACKLOG.md 5 und `HTokens`), Components (jede `H*`-Komponente aus `packages/ui` mit Zustandsbild), Layout (drei Panes, Maße), Interaction (Shortcuts-Tabelle aus CONVENTIONS.md 3.9), Motion (fünf Micro-Interactions mit Dauer und Easing), Signature elements (Release Valve, Isolation Ring, Diff-Glow mit Screenshot), Language (DE/EN-Begriffe), Anti-patterns (acht).

**docs/reference/rules.md**: Jedes Feld aus CONVENTIONS.md 3.3 mit Typ, Pflicht/optional, Default, Beispiel, Randfällen. Abschnitt „Matching semantics" mit der Label-Glob-Tabelle aus BACKLOG.md 4.5 Test 4 als Wahrheitstabelle. Abschnitt „Evaluation order" (first match, Default ask, Expiry, Session-Bindung). Abschnitt „Recipes": nur LLM, npm+pypi+github, Firmen-Registry, WebSocket erlauben, Streaming-Upload erlauben. Abschnitt „Bundled rules" mit Inhalt von `rules/default.yaml` und Begründung pro Regel.

**docs/reference/config.md**, **cli.md**, **diagnostics.md**: generiert durch `cargo xtask docs`. `config.md` aus `humanitl config schema`: Tabelle pro Gruppe mit Schlüssel, Typ, Default, Tier, Beschreibung. `cli.md` aus `clap_markdown` oder `clap::Command::render_long_help` pro Subkommando. `diagnostics.md` aus dem Code-Register: Code, Severity, Titel, `why` (en), Fix-Typ, wo er auftritt. Ein CI-Job `docs-check` führt `cargo xtask docs` aus und schlägt fehl, wenn `git diff --exit-code docs/reference/` Änderungen zeigt.

**docs/guides/agent-profiles.md**: Aufbau eines Profils (`profiles/*.toml` Felder), OpenCode-Profil erklärt Zeile für Zeile (`opencode.json`-Template, `OPENCODE_MODELS_URL`, `OPENCODE_DISABLE_AUTOUPDATE`, Permissions), Abschnitt „Writing an adapter" mit dem `AgentAdapter`-Trait und einem Minimalbeispiel für ein Shell-Skript als Agent, Abschnitt „Known phone-home hosts" pro Agent.

**docs/guides/install.md**: Voraussetzungen (Debian 12+/Ubuntu 24.04+, bwrap ≥ 0.8, User Namespaces aktiv: `sysctl kernel.unprivileged_userns_clone` bzw. `user.max_user_namespaces`, GTK3, libayatana-appindicator für Tray), deb-Installation, AppImage, systemd-Unit aktivieren, Prüfen mit `humanitl daemon status` und `humanitl sandbox check`, Deinstallation inklusive Datenpfade.

**docs/guides/first-session.md**: die vier Setup-Punkte, LLM-Endpoint testen, Projekt wählen, Start, Isolation-Check lesen, erste gehaltene Anfrage, Regel anlegen, Editor, History, Audit prüfen. Mit Screenshots (fünf).

### Schritte
1. `cargo xtask docs` bauen (Generator für drei Referenzen), CI-Job `docs-check`. Prüfen: Job grün, generierte Dateien eingecheckt.
2. `docs/reference/rules.md` schreiben, alle Beispiele mit `humanitl rules test` gegen einen laufenden Daemon verifizieren (Ausgabe als Kommentar im Dokument).
3. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` von Entwurf (HUM-007) auf final bringen; jede technische Aussage gegen den Code prüfen (Shim-Reihenfolge, Syscall-Liste aus `humanitl-shim/src/seccomp.rs`, Pfade aus CONVENTIONS.md 3.4). Root-`SECURITY.md` mit Meldeweg.
4. `docs/DESIGN.md` schreiben, Token-Tabellen aus `HTokens` generieren (kleines Dart-Skript `tool/dump_tokens.dart`), Screenshots aus dem Golden-Setup.
5. Guides schreiben, jeden Schritt auf einer frischen VM (Debian 13, nur `.deb`) durchspielen und Abweichungen korrigieren.
6. README und README.de.md, Screenshot erzeugen.
7. `docs/adr/README.md` Index, ADR-011 bis 013 als Dateien anlegen falls HUM-009 sie noch nicht hat.
8. Link-Check: `lychee --offline docs/ README.md` (nur interne Links), in `docs-check` aufnehmen.

### Tests
- `docs-check` CI: `cargo xtask docs && git diff --exit-code docs/reference/`.
- `docs-links` CI: `lychee --offline --include-fragments README.md docs/` ohne Fehler.
- `rules_reference_examples` (Rust-Test in `humanitl-rules`): Parst jeden YAML-Block aus `docs/reference/rules.md` (Blöcke mit ` ```yaml `), erwartet `Ok`. Dazu ein kleiner Markdown-Extraktor im Test.
- `security_doc_mentions_syscalls` (Rust-Test in `humanitl-shim`): Liest `docs/SECURITY.md`, prüft, dass jeder in `seccomp.rs` verbotene Syscall-Name im Dokument vorkommt. Verhindert, dass Doku und Filter auseinanderlaufen.
- Manuell: Fresh-VM-Durchlauf von `install.md` und `first-session.md` mit Protokoll im PR.

### Akzeptanzkriterien
- [ ] `README.md` hat die zwölf Pflichtabschnitte in der Reihenfolge (Prüfung: `grep '^## '`).
- [ ] `docs/SECURITY.md` hat die elf Abschnitte, Root-`SECURITY.md` existiert mit E-Mail und Disclosure-Frist.
- [ ] `docs/THREAT-MODEL.md` enthält die Angriffsflächen-Tabelle mit zwölf Zeilen und einer Status-Spalte, deren Werte nur `mitigated`, `declared`, `open` sind.
- [ ] `docs/reference/{config,cli,diagnostics}.md` sind generiert, `docs-check` grün.
- [ ] `rules_reference_examples` und `security_doc_mentions_syscalls` grün.
- [ ] `lychee --offline` ohne Fehler.
- [ ] Fresh-VM-Protokoll im PR: Installation bis erste gehaltene Anfrage in unter 15 Minuten ohne Abweichung vom Guide.
- [ ] `docs/DESIGN.md` enthält alle Hex-Werte aus BACKLOG.md 5 und sie stimmen mit `HTokens` überein (Skript-Ausgabe im PR).
- [ ] Jede Datei unter `docs/` beginnt mit einem H1 und einer Zeile „Applies to: Humanitl 0.1".

### Fallstricke
- Doku, die von Hand Werte aus dem Code wiederholt, veraltet beim nächsten PR. Alles, was aus Code ableitbar ist (Config, CLI, Diagnostics, Tokens, Syscall-Liste), wird generiert oder per Test gegen den Code geprüft.
- Der Sicherheitssatz darf nicht weicher formuliert werden als in BACKLOG.md 0, aber auch nicht härter. Insbesondere nicht „the agent cannot exfiltrate data" (falsch wegen `/work` und LLM), sondern „cannot exfiltrate over the network except through approved requests".
- Screenshots mit echten Hostnamen oder Projektnamen des Maintainers vermeiden; Demo-Session mit `example.com`, `registry.npmjs.org`, Projekt `demo-project`.
- `README.de.md` ist keine Übersetzung des ganzen README, sondern eine Seite: Einzeiler, drei Garantien, Install-Befehle, Link auf das englische README.
- Die Meldeadresse in `SECURITY.md` muss eine existierende Adresse sein, die der Maintainer liest. Platzhalter sind ein Release-Blocker.
- `lychee` ohne `--offline` würde externe Links prüfen und in CI flaky sein; extern nur manuell vor dem Release.

### Referenzen
- BACKLOG.md 0, 3.1, 4, 5, 9; ADR-001 bis 013
- CONVENTIONS.md 3.3, 3.4, 3.7, 3.8, 3.9
- GitHub Security Policy: https://docs.github.com/en/code-security/getting-started/adding-a-security-policy-to-your-repository
- clap Markdown-Rendering: https://docs.rs/clap-markdown
- lychee: https://github.com/lycheeverse/lychee

---

## HUM-060 · Release 0.1.0
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-053, HUM-059, HUM-056, HUM-057, HUM-058 · Blockiert: keine

### Kontext
HUM-053 hat das Packaging (deb, AppImage, systemd-Unit) gebaut. Es fehlt der reproduzierbare Weg vom Tag zum veröffentlichten, signierten Artefakt mit Changelog, sowie die Regeln, nach denen Versionen vergeben werden. Ohne das kann niemand nachprüfen, ob das heruntergeladene Binary dem Quellcode entspricht, und Sicherheits-Patches (HUM-056 Triage Schritt 6) hätten keinen Kanal.

### Ziel
`git tag v0.1.0 && git push --tags` löst einen CI-Job aus, der beide Artefakte baut, Prüfsummen und minisign-Signaturen erzeugt, ein GitHub-Release mit dem Changelog-Abschnitt anlegt und die Artefakte anhängt. Ein Nutzer kann Signatur und Prüfsumme mit zwei dokumentierten Befehlen prüfen. Alle Versionsstellen im Repo stammen aus einer Quelle.

### Nicht-Ziel
Automatisches Versions-Bumping. Veröffentlichung auf Flathub, Snap, AUR, crates.io (nach MVP). Reproducible Builds im strengen Sinn (bitgenaue Reproduktion; Ziel für 0.2, jetzt nur Prüfsumme und Signatur). Auto-Update im Tool (das Tool telefoniert nicht nach Hause; Update-Hinweis kommt frühestens 0.2 und nur opt-in).

### Betroffene Pfade
- `VERSION` (neu, einzige Quelle, Inhalt `0.1.0`)
- `daemon/Cargo.toml` (geändert: `[workspace.package] version` aus `VERSION` via `build.rs` oder cargo-Feature, siehe Spezifikation)
- `app/pubspec.yaml` (geändert: `version: 0.1.0+1`, im Release-Job aus `VERSION` gesetzt)
- `CHANGELOG.md` (neu)
- `.github/workflows/release.yml` (neu)
- `packaging/release/checksums.sh` (neu)
- `packaging/release/verify.sh` (neu, für Nutzer)
- `docs/guides/verify-download.md` (neu)
- `docs/guides/release-process.md` (neu, für Maintainer)
- `daemon/xtask/src/version.rs` (neu: `cargo xtask version check|set X.Y.Z`)

### Spezifikation

Versionierung: SemVer. `0.x`: Minor bricht Config/Proto/Regelformat möglicherweise, Patch nie. Proto-Version (`humanitl.v1`) bleibt bei `v1`, solange keine inkompatible Feldänderung; Major-Bump der Proto ist ein Minor-Bump des Tools vor 1.0. `cargo xtask version check` prüft, dass `VERSION`, `daemon/Cargo.toml` Workspace-Version, `app/pubspec.yaml`, `packaging/deb/control` und der letzte `CHANGELOG.md`-Abschnitt übereinstimmen; läuft in `rust-check`. `cargo xtask version set 0.1.1` schreibt alle Stellen.

`CHANGELOG.md` nach Keep a Changelog 1.1.0: Abschnitte `Unreleased`, dann `[0.1.0] - 2026-MM-DD`; Kategorien `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`. Jeder PR fügt eine Zeile unter `Unreleased` hinzu (CI-Check `changelog-touched`: PR ohne Label `no-changelog` muss `CHANGELOG.md` ändern). Der Release-Job extrahiert den Abschnitt der getaggten Version als Release-Notes.

Release-Job:

```yaml
# .github/workflows/release.yml
name: release
on:
  push:
    tags: ["v[0-9]+.[0-9]+.[0-9]+"]
permissions:
  contents: write
jobs:
  build:
    if: github.repository == 'OWNER/humanitl'      # nie in Forks, Secrets wären leer und der Job würde mit kryptischen Fehlern scheitern
    runs-on: ubuntu-22.04                           # älteste unterstützte glibc für das AppImage
    steps:
      - uses: actions/checkout@v4
      - run: test "v$(cat VERSION)" = "${GITHUB_REF_NAME}"   # Tag muss VERSION entsprechen
      - uses: dtolnay/rust-toolchain@stable
      - uses: subosito/flutter-action@v2
        with: { flutter-version: "3.47.x", cache: true }
      - run: sudo apt-get install -y libgtk-3-dev ninja-build clang libayatana-appindicator3-dev
      - run: cargo build --release --workspace --locked
        working-directory: daemon
      - run: flutter build linux --release
        working-directory: app
      - run: ./packaging/build-deb.sh "$(cat VERSION)"        # aus HUM-053
      - run: ./packaging/build-appimage.sh "$(cat VERSION)"   # aus HUM-053
      - run: ./packaging/release/checksums.sh dist/
      - run: |
          echo "$MINISIGN_KEY" > /tmp/minisign.key
          for f in dist/*.deb dist/*.AppImage dist/SHA256SUMS; do
            minisign -S -s /tmp/minisign.key -m "$f" -t "humanitl $(cat VERSION)"
          done
          rm /tmp/minisign.key
        env: { MINISIGN_KEY: "${{ secrets.MINISIGN_KEY }}", MINISIGN_PASSWORD: "${{ secrets.MINISIGN_PASSWORD }}" }
      - run: ./packaging/release/extract-changelog.sh "$(cat VERSION)" > dist/RELEASE_NOTES.md
      - uses: softprops/action-gh-release@v2
        with:
          body_path: dist/RELEASE_NOTES.md
          files: |
            dist/*.deb
            dist/*.AppImage
            dist/SHA256SUMS
            dist/*.minisig
```

Artefakt-Namen: `humanitl_0.1.0_amd64.deb`, `Humanitl-0.1.0-x86_64.AppImage`, `SHA256SUMS`, je `.minisig`. Der öffentliche minisign-Schlüssel liegt im Repo unter `packaging/release/minisign.pub` und wird im README und in `verify-download.md` mit seinem Fingerprint abgedruckt.

`verify.sh` für Nutzer:

```sh
#!/bin/sh
set -eu
PUB="RWQ..."   # Inhalt von minisign.pub, hart eingebettet, damit das Skript allein reicht
minisign -V -P "$PUB" -m SHA256SUMS
sha256sum -c SHA256SUMS --ignore-missing
```

`docs/guides/release-process.md` Checkliste für den Maintainer:
1. `Unreleased` in `CHANGELOG.md` vollständig, Datum eintragen.
2. `cargo xtask version set X.Y.Z`, Commit `chore(release): X.Y.Z`.
3. Alle CI-Jobs auf `main` grün, inklusive `escape-tests`, `e2e-xvfb`, letzter `fuzz-nightly`.
4. Manuelle MVP-Abnahme (Abschnitt unten) durchgeführt, Protokoll unter `docs/releases/X.Y.Z-acceptance.md`.
5. Externe Links einmal mit `lychee` (online) prüfen.
6. `git tag -s vX.Y.Z -m "Humanitl X.Y.Z"` (signierter Git-Tag mit dem Maintainer-GPG-Key), `git push origin vX.Y.Z`.
7. Release-Job beobachten, Artefakte herunterladen, `verify.sh` auf einer anderen Maschine ausführen.
8. `.deb` auf frischer VM installieren, `humanitl sandbox check` grün, eine gehaltene Anfrage durchspielen.
9. Release von „Draft" auf veröffentlicht setzen (der Job legt es als Draft an: `draft: true` in `action-gh-release`).
10. Neuen `Unreleased`-Abschnitt anlegen.

### Schritte
1. `VERSION` und `cargo xtask version` bauen, alle Versionsstellen anbinden, `rust-check` erweitert. Prüfen: `cargo xtask version check` grün, absichtliche Abweichung schlägt fehl.
2. `CHANGELOG.md` anlegen, `0.1.0`-Abschnitt aus den gemergten PR-Titeln der Sprints 0 bis 5 befüllen (nach Kategorie), `changelog-touched`-Check in CI. Prüfen: PR ohne Changelog-Zeile wird rot.
3. minisign-Schlüsselpaar erzeugen (`minisign -G`), privaten Schlüssel und Passwort als Repo-Secrets `MINISIGN_KEY`, `MINISIGN_PASSWORD`, öffentlichen unter `packaging/release/minisign.pub` einchecken. Prüfen: Fingerprint im README.
4. `checksums.sh`, `extract-changelog.sh`, `verify.sh` schreiben. Prüfen: lokal gegen ein Dummy-`dist/`.
5. `release.yml` schreiben, mit Tag `v0.1.0-rc.1` auf einem Test-Branch probelaufen (Pattern im Workflow temporär auf `-rc` erweitern, danach zurück). Prüfen: Draft-Release mit acht Dateien.
6. `verify-download.md` und `release-process.md` schreiben.
7. Release-Checkliste durchführen, `v0.1.0` taggen.

### Tests
- `xtask_version_check_detects_drift` (Rust, xtask): temporäres Repo mit abweichender `pubspec.yaml`, Erwartung Exit 1 mit Nennung der Datei.
- `extract_changelog_section` (Shell-Test via `bats` oder Rust): Für `0.1.0` wird genau der Abschnitt bis zur nächsten `## [`-Zeile geliefert, ohne Header-Zeile.
- `verify_script_rejects_tampered` (Shell-Test): Nach `verify.sh` ein Byte im `.deb` ändern, erneut `sha256sum -c` schlägt fehl.
- Probelauf `v0.1.0-rc.1` (manuell, Link im PR).

### Akzeptanzkriterien
- [ ] `cat VERSION` liefert `0.1.0`, `cargo xtask version check` grün.
- [ ] `CHANGELOG.md` hat `[0.1.0]`-Abschnitt mit mindestens `Added` und `Security`.
- [ ] `release.yml` hat `if: github.repository == ...`, Tag-VERSION-Prüfung, `draft: true`, alle acht Artefakte.
- [ ] Probelauf-Release `v0.1.0-rc.1` existiert als Draft mit acht Dateien und wurde danach gelöscht.
- [ ] `verify.sh` auf einer zweiten Maschine ausgeführt: beide Prüfungen OK (Protokoll im PR).
- [ ] `packaging/release/minisign.pub` eingecheckt, Fingerprint im README-Abschnitt „Install".
- [ ] Release-Checkliste vollständig abgehakt in `docs/releases/0.1.0-acceptance.md`.
- [ ] Git-Tag `v0.1.0` ist signiert (`git tag -v v0.1.0` OK).

### Fallstricke
- Release-Jobs in Forks: Secrets sind dort leer, `minisign` schlägt mit einer Meldung fehl, die wie ein Bug aussieht. Deshalb `if: github.repository == ...` am Job.
- AppImage auf `ubuntu-latest` bauen bindet eine zu neue glibc ein und läuft dann auf Debian 12 nicht. Ältester unterstützter Runner (`ubuntu-22.04`) oder Container-Build.
- `--locked` bei `cargo build` ist Pflicht, sonst kann der Release andere Dependency-Versionen enthalten als die getestete `Cargo.lock`.
- Der private minisign-Schlüssel darf nie in Logs erscheinen. `set -x` in Release-Skripten verboten; GitHub maskiert Secrets in Logs, aber nicht abgeleitete Werte.
- `softprops/action-gh-release` erzeugt bei erneutem Lauf mit gleichem Tag ein zweites Release oder überschreibt; Tag nie neu setzen, bei Fehler Patch-Version erhöhen.
- Flutter `version:` in `pubspec.yaml` braucht `+build`-Nummer; die Build-Nummer wird auf die Anzahl der Commits (`git rev-list --count HEAD`) gesetzt, damit sie monoton steigt.
- Der Changelog-Extraktor muss mit `##` innerhalb von Codeblöcken umgehen; einfacher: Abschnitte nur an Zeilen erkennen, die mit `## [` beginnen.

### Referenzen
- Keep a Changelog 1.1.0: https://keepachangelog.com/en/1.1.0/
- SemVer 2.0.0: https://semver.org/
- minisign: https://jedisct1.github.io/minisign/
- softprops/action-gh-release: https://github.com/softprops/action-gh-release
- HUM-053 (Packaging), BACKLOG.md ADR-010

---

## HUM-061 · Puffer
Sprint: 5 · Größe: L · Abhängigkeiten: keine · Blockiert: keine

### Kontext
BACKLOG.md 10 nennt Risiken, die im Sprint-Verlauf mit hoher Wahrscheinlichkeit Zeit kosten, ohne dass man sie vorher einem Issue zuordnen kann. Ein geplanter Puffer verhindert, dass der Release-Sprint stillschweigend ein siebter wird. Dieses Issue ist kein Arbeitspaket, sondern ein Budget von fünf Tagen mit Regeln, wofür es ausgegeben werden darf.

### Ziel
Am Ende von Sprint 5 ist entweder das Budget verbraucht und dokumentiert, wofür, oder es ist übrig und wird explizit nicht in neue Funktionen gesteckt. Jede Entnahme ist ein Eintrag in einer Tabelle mit Ursache, Dauer und Ergebnis.

### Nicht-Ziel
Neue Funktionen, auch kleine. Refactorings ohne auslösenden Fehler. Alles aus BACKLOG.md Abschnitt 9.

### Betroffene Pfade
- `docs/releases/0.1.0-buffer-log.md` (neu)

### Spezifikation

Bekannte Risiken mit vorab zugeordnetem Budget (Summe 5 Tage):

| Risiko | Wahrscheinlichkeit | Budget | Auslöser für Entnahme | Erste Maßnahme |
|---|---|---|---|---|
| shadcn_flutter-Breakage beim Pinnen auf die Release-Version | hoch | 1 Tag | `flutter pub upgrade` oder Flutter-Patch bricht `packages/ui` | Version einfrieren, Wrapper anpassen, nie den Screen-Code |
| MITM-Randfall bei OpenCode (Bun-fetch, h1 keep-alive, `Expect`) | mittel | 1 Tag | Demo M3 rot nach Dependency-Update oder OpenCode-Update im Profil | Konformitäts-Matrix (HUM-017) um den Fall erweitern, dann fixen |
| Wayland/NVIDIA/Impeller-Rendering | mittel | 0,5 Tag | Artefakte, schwarzes Fenster, Tray fehlt | `--no-enable-impeller` dokumentieren, Bug upstream melden, Workaround in `install.md` |
| Fuzz-Funde aus HUM-056 mit tieferer Ursache | mittel | 1 Tag | Fund, der einen Decoder-Umbau statt einer Zeile braucht | Triage-Prozess, Regressionstest, notfalls Encoding vorübergehend als `Unsupported` |
| Escape-Test-Flakiness auf CI-Runnern (User-Namespaces, seccomp-Version) | mittel | 0,5 Tag | `escape-tests` rot ohne Codeänderung | Runner-Kernel prüfen, Test auf Container mit `--privileged` nicht ausweichen (das würde die Aussage entwerten), stattdessen Runner-Image pinnen |
| Frischer VM-Durchlauf (HUM-059) findet Install-Lücken | hoch | 0,5 Tag | Guide-Schritt scheitert | Guide korrigieren, fehlende Abhängigkeit in `deb/control` |
| Unvorhergesehenes | – | 0,5 Tag | alles andere | Eintrag im Log mit Begründung |

Triage-Regel für jede Entnahme:
1. Ist es ein Blocker für ein Demo-Skript (M1 bis M4) oder für die MVP-Abnahme? Wenn nein: nicht aus dem Puffer, sondern als Issue nach 0.1 (BACKLOG.md Abschnitt 9 ergänzen).
2. Wenn ja: Eintrag im Buffer-Log mit Datum, Risiko-Zeile, geschätzter Dauer, dann erst arbeiten.
3. Nach Abschluss: tatsächliche Dauer und Ergebnis nachtragen. Wenn ein Risiko sein Budget überschreitet, wird das nächste unwahrscheinlichste Risiko gekürzt, nicht der Release verschoben; erst wenn das gesamte Budget überschritten ist, wird der Release-Termin verschoben und das im Log begründet.
4. Übriges Budget am Sprint-Ende verfällt. Es wird nicht in Features gesteckt.

Log-Format:

```markdown
| Datum | Risiko | Geschätzt | Tatsächlich | Ergebnis (Commit/PR) |
|---|---|---|---|---|
| 2026-11-03 | shadcn-Breakage | 0,5 d | 0,75 d | #142 Wrapper HResizable angepasst |
```

### Schritte
1. `docs/releases/0.1.0-buffer-log.md` mit der Risikotabelle und leerem Log anlegen (erster Tag des Sprints).
2. Bei jeder Entnahme: Triage-Regel anwenden, Log pflegen.
3. Am letzten Sprint-Tag: Summenzeile, Restbudget, kurze Bewertung, welche Risiken für 0.2 bleiben.

### Tests
keine

### Akzeptanzkriterien
- [ ] Buffer-Log existiert ab Sprint-Beginn mit den sieben Risikozeilen.
- [ ] Jede Entnahme hat einen Log-Eintrag vor Beginn der Arbeit (Commit-Zeitstempel des Log-Eintrags liegt vor dem ersten Fix-Commit).
- [ ] Summenzeile am Sprint-Ende, Restbudget ausgewiesen.
- [ ] Kein Log-Eintrag verweist auf ein Feature aus BACKLOG.md Abschnitt 9.

### Fallstricke
- Der häufigste Fehler ist, Puffer für „nur noch schnell" Features zu verwenden. Der Log macht das sichtbar und die Triage-Regel 1 verbietet es.
- Escape-Test-Flakiness mit `--privileged` oder `sudo` zu „lösen" macht die Sicherheitsaussage wertlos, weil die Tests dann nicht mehr die rootless-Konfiguration prüfen.
- Ein Risiko, das sich früh materialisiert (z. B. shadcn in Sprint 5 Tag 1), verbraucht Budget dieses Sprints, nicht des vorherigen; die Zuordnung ist nach Zeitpunkt der Entnahme.

### Referenzen
- BACKLOG.md 10 (Risiken), ADR-009 (shadcn-Pinning), HUM-017, HUM-056

---

## MVP-Abnahme

Manuelle Abnahme für 0.1.0. Ein Mensch geht die Liste in etwa 60 Minuten auf einer frischen Debian-13-VM (oder Ubuntu 24.04) mit installiertem `.deb` durch. Voraussetzung: ein erreichbarer Ollama-Server im LAN mit einem kleinen Modell, ein Testprojekt `demo-project` mit einer Datei `notes.md`, die eine E-Mail-Adresse und den String `Acme Corp` enthält. Jeder Punkt wird mit OK, FAIL oder SKIP (Begründung) markiert. Das Protokoll wird als `docs/releases/0.1.0-acceptance.md` eingecheckt. Ein FAIL in den Abschnitten A oder B blockiert den Release.

### A. Installation und Daemon (5 min)
1. `.deb` installiert ohne Fehlermeldung; `humanitl --version` und `humanitld --version` zeigen `0.1.0`.
2. `systemctl --user enable --now humanitld` läuft; `humanitl daemon status` meldet „connected", Proto `v1`.
3. `humanitl sandbox check` zeigt drei grüne Prüfungen mit Evidence-Text.
4. `ls -la $XDG_RUNTIME_DIR/humanitl/` zeigt `daemon.sock` mit `0600`, Verzeichnis `proxy/` mit `0700`.
5. `ls -la ~/.local/share/humanitl/ca/` zeigt `ca.key` mit `0600`; `ca.crt` ist nicht in `/etc/ssl/certs` und nicht im Browser-Trust-Store.

### B. Sicherheit (15 min)
6. `humanitl sandbox run -- ip link` zeigt nur `lo`.
7. `humanitl sandbox run -- sh -c 'find / -type s 2>/dev/null'` zeigt genau `/run/humanitl/proxy.sock`.
8. `humanitl sandbox run -- python3 -c "import socket; socket.socket()"` schlägt mit `PermissionError` fehl.
9. `humanitl sandbox run -- cat /proc/1/environ` ist leer oder verweigert; `hostname` innen ist `sandbox`.
10. `humanitl sandbox run -- ls /tmp/.X11-unix /run/user 2>&1` findet nichts.
11. `humanitl sandbox run -- curl -s http://example.com` ohne Proxy-Env scheitert mit „Could not resolve host" oder „Network unreachable".
12. `humanitl sandbox run -- curl -sI https://example.com` (mit Profil-Env) bleibt hängen, im UI erscheint eine gehaltene Anfrage mit Host `example.com`; nach Block erhält curl `403` mit `reason: user`.
13. Während Punkt 12 hängt: `resolvectl statistics` (oder `tcpdump -i any port 53`) auf dem Host zeigt keinen Lookup für `example.com` vor der Entscheidung.
14. `humanitl sandbox run -- curl -s --proxy http://127.0.0.1:3128 -H 'Host: evil.io' https://github.com/` wird geblockt mit `reason: authority_mismatch`, ohne Nachfrage im UI.
15. `humanitl sandbox run -- curl -s --proxy http://127.0.0.1:3128 http://169.254.169.254/` erscheint als gehaltene Anfrage mit Host `169.254.169.254`, keine Regel matcht automatisch.
16. `humanitl rules test https://api.github.com/repos` mit Regel `*.github.com` ergibt `match`; `https://evil-github.com/` und `https://github.com.evil.io/` ergeben `default (ask)`.
17. Im Sandbox-Screen wird die exakte bwrap-Kommandozeile angezeigt und enthält `--unshare-all`, `--new-session`, `--die-with-parent`, genau ein `--bind` für den Proxy-Socket als Datei.
18. In der Sandbox `ln -s /home /work/escape` anlegen, Session beenden: Zusammenfassung markiert den Symlink als „zeigt außerhalb /work".
19. In der Sandbox `printf '\033]52;c;SGVsbG8=\a'` ausgeben: Host-Clipboard bleibt unverändert.
20. `humanitl audit verify` meldet „chain OK, N entries, head <hash>"; eine Zeile aus `audit.jsonl` löschen, erneut prüfen: „broken at seq K".

### C. Setup und erste Session (10 min)
21. Erster App-Start zeigt die Vier-Punkte-Checkliste, nicht eine leere Queue; Punkt „Daemon" ist grün.
22. LLM-Endpoint eintragen, „Test" listet Modelle; darunter steht der Satz, dass Traffic dorthin die Queue umgeht.
23. Projektordner wählen, `rw`; der Satz „Der Agent sieht nur /work = …" zeigt den richtigen Pfad.
24. Start: Isolation-Check animiert drei grüne Zeilen, vierte Zeile amber mit LLM-Adresse; Header-Ring vollständig grün.
25. Terminal zeigt OpenCode; erster Prompt „Was steht in notes.md?" wird beantwortet; im History-Screen erscheinen Passthrough-Flows in violett, keine gehaltene Anfrage vor dem ersten Prompt (Default-Regeln greifen, Bundled-Badge im Rules-Screen).
26. Prompt „Lade https://example.com und fasse zusammen": Anfrage wird gehalten, Karte zeigt „Angehalten, weil keine Regel passt", Coach-Mark am Scope-Selektor beim ersten Mal.

### D. Intercept und Entscheidung (10 min)
27. Karte: Sektionen Query/Headers/Body auf- und zuklappbar mit Space; Method-Badge, Host, Pfad mittig gekürzt, Countdown-Ring sichtbar.
28. Domain-Panel: `example.com` als Unbekannt-Karte gestrichelt; für `registry.npmjs.org` (Prompt „installiere lodash") Katalog-Karte mit Icon, Kategorie, Tranco-Rang.
29. Enter sendet einmalig; Toast/Inline zeigt „Gesendet an example.com · N KB", kein Undo-Button.
30. `R` öffnet Scope-Popover; Regelsatz-Vorschau ändert sich live beim Umschalten von Host auf Apex; Default-Dauer ist „Session".
31. Regel anlegen: Inline „Regel gespeichert · Rückgängig", Rückgängig entfernt sie; Rules-Screen zeigt „erstellt vor … aus Request #…".
32. `npm install`-Prompt: Queue gruppiert nach Host, Summary-Zeile „registry.npmjs.org · N GET · 0 Findings"; Button heißt „Allow N → registry.npmjs.org"; kein „Allow all" sichtbar.
33. Neue Anfrage während Karte geöffnet: Fokus bleibt, Liste sortiert nicht um, „+1 seit du liest" erscheint.
34. `B` blockt; Allow und Block liegen nicht nebeneinander; Block-all bei mehr als fünf fragt per Modal, sonst nie ein Modal.
35. Timeout auf 10 s stellen (Settings), Anfrage auslösen, warten: Karte wird grau „Blockiert (Timeout)", Agent-Terminal zeigt 403; Entwurf im Editor bleibt.
36. Fenster minimieren, Anfrage auslösen: Desktop-Notification mit Allow/Block, Tray-Badge „1"; zurückkehren: Banner „Der Agent wartet seit …".

### E. Editor und Findings (8 min)
37. Prompt, der `notes.md` an eine Web-API schickt (z. B. „poste den Inhalt von notes.md an https://httpbin.org/post"): Karte zeigt Findings-Chip mit E-Mail und `Acme Corp` (Nutzer-Term aus Settings) unterstrichen.
38. `E` öffnet Editor: Split-View, Findings-Rail gruppiert; „Alle durch Pseudonyme ersetzen" ersetzt zu `<EMAIL_1>` und `Client-A`; Diff-Glow sichtbar.
39. Senden: Button heißt „Editierte Version senden" mit Stift; Karte in History trägt Chip „Edited"; Response-Body in History zeigt die Pseudonyme, nicht die Originale.
40. Mapping-Panel zeigt beide Einträge mit maskiertem Original; dieselbe E-Mail in einer zweiten Anfrage erhält wieder `<EMAIL_1>`.
41. Allow mit ungelösten Findings: Button amber „Senden mit 2 Findings", Inline-Pause mit drei Optionen, kein Modal.
42. `~/.local/share/humanitl/` enthält keine Klartext-E-Mail (`grep -r` über DB und Blobs findet sie nur in den aufgezeichneten Original-Requests, nicht im Pseudonym-Store).

### F. History, Rules, Audit, Settings (7 min)
43. History: Filter `host:example.com state:blocked` liefert nur passende Zeilen; Sortierung nach Zeit und Größe funktioniert; 500 Zeilen scrollen flüssig.
44. Export HAR und JSONL erzeugen Dateien; JSONL enthält die Decision und Rule-ID.
45. Rules-Screen: Tabs „Gespeichert" und „Temporär"; Session-Regel aus Punkt 30 steht unter Temporär mit Restlaufzeit; „dauerhaft machen" verschiebt sie.
46. Dry-Run einer neuen Regel zeigt, welche vergangenen Flows gematcht hätten.
47. Audit-Screen: „Kette prüfen" grün, Head-Hash sichtbar, Export CSV erzeugt Datei.
48. Settings: Suche nach „timeout" findet `limits.response_idle_timeout_secs` und `hold.timeout_secs`; Feld zeigt Beschreibung, Default, Herkunft; Expert-Gruppe ist eingeklappt mit Warnhinweis.
49. `humanitl config set hold.timeout_secs abc` liefert Diagnostic mit Exit 1; `humanitl config set hold.timeout_secs 120` wird im UI ohne Neustart sichtbar.

### G. CLI und Profile (5 min)
50. In `demo-project`: `humanitl run --profile llm-only` startet OpenCode im Terminal des Nutzers; Prompt gegen LLM funktioniert; `webfetch`-Versuch scheitert mit 403 `reason: rule`, im Terminal sichtbar; keine UI nötig.
51. `humanitl run --ask terminal` mit Fetch-Prompt: Terminal-Prompt zeigt Host, Methode, Größe, Findings und `[a]llow [b]lock [r]ule`; `a` sendet.
52. Während `humanitl run` läuft die UI starten: die laufende Session erscheint, Queue wird von der UI übernommen, Terminal-Prompt verschwindet.
53. `.humanitl/profile.toml` im Projekt mit anderem Timeout: `humanitl config get hold.timeout_secs` zeigt Projektwert mit Herkunft „project".
54. `humanitl flows list --json | jq length` entspricht der Zeilenzahl im History-Screen.

### H. i18n, Theme, Fehlerpfade (5 min)
55. Sprache auf Deutsch: Aktionsbutton heißt „Senden", Regel-Aktion „Erlauben", Zustand „angehalten", Editor „Pseudonymisieren"; kein englischer String in Intercept, Setup, Sandbox sichtbar.
56. Light-Theme: alle Zustandsfarben unterscheidbar, kein Text unter Kontrast 4,5:1 (Stichprobe mit Farbpipette an Queue-Zeile und Statusleiste).
57. `systemctl --user stop humanitld` bei laufender UI: Statusleiste rot, nach 5 s Setup-Screen mit Diagnostic und Fix „Dienst starten"; `start` wieder: UI verbindet sich ohne Neustart, Queue ist leer, keine veraltete Karte.
58. TLS-Ablehnung provozieren (`humanitl sandbox run -- env -u SSL_CERT_FILE curl https://example.com`): Feed zeigt Karte „curl hat das Zertifikat abgelehnt" mit Fix „Fix kopieren", Clipboard enthält `export SSL_CERT_FILE=…`.
59. Falschen LLM-Endpoint eintragen: Isolation-Panel-Zeile LLM rot mit `LLM_001` und Fix „Einstellung öffnen", der ins richtige Feld springt.
60. `.AppImage` auf derselben VM starten: verbindet sich mit dem laufenden Daemon, Intercept-Screen identisch zum `.deb`-Build.

### Abschluss
- Summe OK / FAIL / SKIP eintragen.
- Jeder FAIL bekommt ein Issue oder einen Buffer-Log-Eintrag (HUM-061).
- Protokoll signiert mit Datum, Maschine, Kernel-Version, bwrap-Version, Flutter-Version.


## HUM-086 · Repository auf Englisch
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-059 · Blockiert: HUM-060

### Kontext
Die Planung entstand auf Deutsch, weil der Gründer so denkt. Ein Open-Source-Projekt mit Anspruch braucht eine englische Codebasis und Dokumentation, damit Beiträge von außen möglich sind. Die Übersetzung passiert einmal, am Ende, wenn die Texte stabil sind.

### Ziel
Jede Datei im Repository außer `app/l10n/app_de.arb` ist Englisch: `BACKLOG.md`, `backlog/*.md`, `docs/**`, `CLAUDE.md`, `CONTRIBUTING.md`, `AGENTS.md`, `README.md`, Code-Kommentare, Doc-Kommentare, Diagnostic-Texte (`title`, `why`) im Register, Fixture-Kommentare, Skript-Header. Deutsch existiert nur noch als Übersetzung in der ARB-Datei. Ein Lint verhindert Rückfall.

### Nicht-Ziel
Keine inhaltlichen Änderungen beim Übersetzen. Keine Umbenennung von Bezeichnern (die sind schon Englisch). Keine Übersetzung der git-Historie.

### Betroffene Pfade
- alle `*.md` außerhalb `app/l10n/`
- alle `*.rs`, `*.dart`, `*.sh`, `*.py`, `*.toml`, `*.yaml` (Kommentare)
- `daemon/crates/core-types/src/diagnostics/codes.rs` (Texte)
- `scripts/ci/lint-docs.sh` (erweitern)

### Spezifikation
- Reihenfolge: erst Dokumente, dann Code-Kommentare, dann Diagnostics; pro Bereich ein Commit, damit Reviews lesbar bleiben.
- Terminologie fest: held, allow, block, target (für Scope), rule, pseudonymise (britische Schreibung) durchgängig; Sandbox, Diagnostic, Finding unverändert.
- `scripts/ci/lint-docs.sh` bekommt eine Stoppwortliste (`und`, `oder`, `nicht`, `wird`, `werden`, `ist`, `sind`, `mit`, `für`, `über`, `durch`, `Datei`, `Regel`, `Anfrage`) und prüft Markdown, Doc-Kommentare (`///`, `//!`) und Shell-/Python-Kommentare; Treffer in `app_de.arb` und in als `<!-- lang: de -->` markierten Blöcken sind erlaubt.
- Abschnitt Sprache in `CLAUDE.md` wird zu: English only; German exists solely in `app_de.arb`.

### Schritte
1. Glossar in `docs/GLOSSARY.md` anlegen (en, de, Bedeutung), 30 bis 40 Einträge.
2. Dokumente übersetzen, Lint schreiben, Lint grün.
3. Code-Kommentare übersetzen, `cargo doc --no-deps` und `dart doc` bauen ohne Warnung.
4. Diagnostics-Texte übersetzen, Snapshot-Tests aktualisieren.
5. `CLAUDE.md`, `CONTRIBUTING.md`, `AGENTS.md` umstellen.

### Tests
- `scripts/ci/lint-docs.sh` grün, mit Negativtest (eine deutsche Zeile in einer Fixture-Datei bricht den Lint).
- Diagnostics-Snapshot-Tests grün.
- `cargo doc --no-deps --document-private-items` ohne Warnung.

### Akzeptanzkriterien
- [ ] `git grep -lE ' (und|oder|nicht|wird|werden) ' -- ':!app/l10n/app_de.arb' ':!*.lock'` liefert nichts.
- [ ] Lint in CI aktiv.
- [ ] Glossar existiert und wird von README verlinkt.
- [ ] Kein inhaltlicher Unterschied: Stichprobe von zehn ADR-Absätzen gegen die deutsche Fassung im git-Verlauf.

### Fallstricke
- Maschinelle Übersetzung verwischt Fachbegriffe; das Glossar ist verbindlich, nicht optional.
- Deutsche Umlaute in Bezeichnern gibt es nicht, aber in Fixture-Bodies (Kundennamen) absichtlich; die bleiben, der Lint ignoriert `fixtures/`.
- Snapshot-Tests der Diagnostics brechen erwartet; nicht blind aktualisieren, sondern jede Änderung lesen.

### Referenzen
BACKLOG.md 1.3 Prinzip 2; CLAUDE.md Abschnitt Sprache.

---

## HUM-093 · M1-Demolauf raeumt sein Verzeichnis nicht weg
Sprint: 5 · Größe: S · Schweregrad: minor · Abhängigkeiten: HUM-021, HUM-036 · Blockiert: keine

### Kontext
Der Kopf von `tests/e2e/lib.sh` (Zeilen 10 bis 13) sagt über den Wegwerf-Baum, in dem jeder Demolauf steht: „Ein laufender Daemon des Entwicklers wird dadurch nie berührt, und der Lauf hinterlässt nichts, was der nächste erbt." Der erste Halbsatz stimmt, der zweite ist für M1 unwahr. `e2e_short_workdir` legt den Baum mit `mktemp -d /tmp/hum-e2e-XXXXXX` an (`lib.sh:237`), und weder `lib.sh` noch `m1_sealed_box.sh` entfernen ihn je wieder: `m1_sealed_box.sh` hat einen EXIT-Trap (Zeile 81), sein `collect()` kopiert `out/*` und `daemon.log` nach `target/e2e` (Zeilen 78 und 79) und hört dort auf. Jeder Durchgang lässt also einen weiteren `/tmp/hum-e2e-*` stehen.

Das ist keine vergessene Bequemlichkeit, sondern eine dokumentierte Zusage, die der Code nicht einlöst — und zwar ausgerechnet im Skript, dessen letzter Schritt `the daemon leaves nothing behind` heißt und Socket, Token und Proxy-Socket genau daraufhin prüft. Das Demo belegt eine Aufräum-Aussage über den Daemon und bricht dieselbe Aussage über sich selbst.

Was stehen bleibt, ist der vollständige XDG-Baum der Sitzung: `data/humanitl/ca/ca.key`, der private Schlüssel der Sitzungs-CA (Rechte `0600`, `daemon/crates/proxy/src/ca.rs` `KEY_MODE`, Pfad aus `daemon/crates/config/src/paths.rs` `ca_key_path`), dazu `ca.crt`, die Aufzeichnung, `state/runtime`, `work` und die Protokolle. Das Verzeichnis selbst hat `0700` aus `mktemp -d`, es liegt also nichts offen; es sammelt sich aber mit jedem Lauf Testschlüsselmaterial an, das laut Kommentar längst weg sein sollte. Der Lauf sieht das Host-`/tmp`, denn `e2e_enter_namespace` nimmt `unshare -rn`, also nur Nutzer- und Netz-Namensraum, keinen Mount-Namensraum (`lib.sh:206-227`). CI-Runner sind kurzlebig, der Schaden liegt auf Entwicklerrechnern.

M2 macht es seit HUM-036 richtig und schreibt den Grund dazu („auf dem Rechner bleibt sonst nichts", `m2_first_decision/run.sh:125-128`): doppelt abgesichert über eine Existenzprüfung und einen `case`-Präfix `/tmp/hum-e2e-*` (Zeilen 137 bis 143), im EXIT-Trap, der wegen `set -euo pipefail` und `e2e_die` mit Exit 1 auch nach einem Fehlschlag greift. M1 ist der Ausreißer, und die Absicherung existiert bereits — sie steht nur an der falschen Stelle, nämlich einmal in M2 statt einmal für beide.

### Ziel
`m1_sealed_box.sh` entfernt seinen Arbeitsbaum im vorhandenen `collect()`, mit derselben doppelten Absicherung wie M2. Die Absicherung steht als Helfer `e2e_drop_workdir` in `lib.sh` neben `e2e_short_workdir`, und M2 nutzt ihn statt der eigenen `case`-Zeile. `E2E_KEEP_WORKDIR=1` behält den Baum für die Fehlersuche und nennt seinen Pfad im Protokoll.

### Nicht-Ziel
Kein neuer Trap: M1 hat einen, es fehlt ihm nur eine Zeile. Keine Änderung daran, was gesammelt wird oder wohin — `out/*` und `daemon.log` liegen vor dem Löschen bereits in `target/e2e`, und genau diesen Pfad lädt die CI hoch (`.github/workflows/ci.yml`, `path: target/e2e`). Kein Aufräumen fremder Bäume: Was ältere Läufe hinterlassen haben, entfernt der Nutzer; es gibt keinen Reaper über `/tmp/hum-e2e-*`. Kein Rust, keine CI-Datei, kein Makefile, kein `.gitignore`.

### Betroffene Pfade
- `tests/e2e/lib.sh`: neuer Helfer `e2e_drop_workdir` direkt unter `e2e_short_workdir` (231-243)
- `tests/e2e/m1_sealed_box.sh`: `collect()` (75-80), Aufrufblock im Kopf (33-38)
- `tests/e2e/m2_first_decision/run.sh`: `collect()` (137-143) auf den Helfer umstellen, Kommentar (125-128) und Aufrufblock (37-42) nachziehen
- `tests/e2e/run.sh`: Aufrufblock im Kopf (4-8)

### Spezifikation
Der Helfer in `lib.sh`, POSIX sh wie der Rest der Datei:

```sh
# e2e_drop_workdir — den Wegwerf-Baum dieses Laufs entfernen.
#
# Zweifach abgesichert, weil `rm -rf` auf einem Variablenwert die Stelle ist,
# an der ein leerer Wert teuer wird: Der Baum muss existieren und unter dem
# Präfix liegen, den `e2e_short_workdir` vergibt. Alles andere wird gemeldet
# und stehen gelassen. `E2E_KEEP_WORKDIR=1` behält ihn für die Fehlersuche.
e2e_drop_workdir() {
    [ -n "${E2E_WORKDIR:-}" ] || return 0
    [ -d "$E2E_WORKDIR" ] || return 0
    if [ "${E2E_KEEP_WORKDIR:-0}" = 1 ]; then
        e2e_say "keeping the workdir $E2E_WORKDIR (E2E_KEEP_WORKDIR=1)"
        return 0
    fi
    case "$E2E_WORKDIR" in
    /tmp/hum-e2e-*) rm -rf "$E2E_WORKDIR" ;;
    *) e2e_say "not removing $E2E_WORKDIR: it is not under /tmp/hum-e2e-" ;;
    esac
}
```

`collect()` in `m1_sealed_box.sh` bekommt die Reihenfolge, die M2 schon hat: erst die Prozesse dieses Laufs beenden, dann die Protokolle nach `$E2E_OUT` kopieren, dann `e2e_drop_workdir` als letzte Zeile. Die beiden `cp`-Zeilen werden dabei wie in M2 gegen einen ungesetzten oder fehlenden Baum abgesichert.

```sh
collect() {
    stop_daemon
    stop_fake_upstream
    if [ -n "${E2E_WORKDIR:-}" ] && [ -d "$E2E_WORKDIR" ]; then
        cp -f "$E2E_WORKDIR"/out/* "$E2E_OUT/" 2> /dev/null || true
        cp -f "$E2E_WORKDIR"/daemon.log "$E2E_OUT/" 2> /dev/null || true
    fi
    e2e_drop_workdir
}
```

`collect()` in `m2_first_decision/run.sh` verliert seine `case`-Zeilen und ruft stattdessen `e2e_drop_workdir`; Verhalten und Reihenfolge bleiben, was sie sind. Der Präfix-Vergleich lautet in beiden Fällen `/tmp/hum-e2e-`, nicht `$TMPDIR/…`: Der Baum liegt bewusst unter `/tmp`, weil ein Unix-Socket-Pfad in 108 Bytes passen muss (`lib.sh`, Kopf).

`E2E_KEEP_WORKDIR` wird in den drei Aufrufblöcken dokumentiert, in der Form, die dort schon steht:

```
#   E2E_KEEP_WORKDIR=1 ./tests/e2e/m1_sealed_box.sh
#                                         den Wegwerf-Baum unter /tmp behalten
```

### Schritte
1. `e2e_drop_workdir` in `lib.sh` schreiben, direkt unter `e2e_short_workdir`.
2. `collect()` in `m1_sealed_box.sh` um die Absicherung der `cp`-Zeilen und den Aufruf ergänzen.
3. `collect()` in `m2_first_decision/run.sh` auf den Helfer umstellen, Kommentar 125 bis 128 auf die neue Stelle beziehen.
4. `E2E_KEEP_WORKDIR` in die drei Aufrufblöcke aufnehmen.
5. `./tests/e2e/run.sh` einmal grün fahren, einmal absichtlich rot (siehe Akzeptanzkriterien), `/tmp` jeweils davor und danach zählen.

### Tests
Die Demoskripte sind der Test; geprüft wird von außen, mit einem Blick nach `/tmp` und nach `target/e2e` vor und nach dem Lauf. Kein neuer Testrunner, keine neue Datei.

### Akzeptanzkriterien
- [ ] Grüner Lauf räumt auf: `ls -d /tmp/hum-e2e-* 2>/dev/null | wc -l` liefert nach `./tests/e2e/run.sh` (Exit 0) dieselbe Zahl wie unmittelbar davor, auf einer sauberen Maschine 0.
- [ ] Roter Lauf räumt auch auf: `E2E_SKIP_BUILD=1 CARGO_TARGET_DIR=/tmp/no-such-target ./tests/e2e/m1_sealed_box.sh` endet mit Exit 1 und `no humanitld binary at …`, und danach ist die Zahl der `/tmp/hum-e2e-*` unverändert.
- [ ] Artefakte bleiben vollständig: `ls target/e2e` nennt nach dem Lauf dieselben Dateien wie vor der Änderung (`daemon.log`, `upstream.log`, die Ausgaben aus `out/`), und `target/e2e/m2` ist nach `E2E_ONLY=m2 ./tests/e2e/run.sh` unverändert gefüllt.
- [ ] `E2E_KEEP_WORKDIR=1 ./tests/e2e/m1_sealed_box.sh` endet mit Exit 0, das Verzeichnis existiert danach noch, und das Protokoll enthält genau eine Zeile `keeping the workdir /tmp/hum-e2e-…`.
- [ ] M2 löscht weiter: `E2E_ONLY=m2 ./tests/e2e/run.sh` Exit 0, danach kein neues `/tmp/hum-e2e-*`.
- [ ] Die Absicherung steht genau einmal: `git grep -n 'hum-e2e-\*' tests/e2e` zeigt nur die eine Stelle in `lib.sh`.

### Fallstricke
- Reihenfolge in `collect()`: erst die Prozesse beenden, dann kopieren, dann löschen. Ein `rm -rf` vor dem `cp` nimmt genau die Protokolle mit, die den Fehlschlag erklären, und macht aus einem roten Lauf einen stummen.
- Der Trap läuft auch, wenn `e2e_short_workdir` selbst gescheitert ist. Unter `set -euo pipefail` ist `E2E_WORKDIR` dann ungesetzt, und ein nacktes `"$E2E_WORKDIR"` beendet `collect()` mit `unbound variable`, bevor `stop_daemon` etwas tut. Deshalb `${E2E_WORKDIR:-}` in den `cp`-Zeilen und im Helfer.
- Beide Absicherungen bleiben. Die Existenzprüfung allein genügt nicht, der Präfix allein auch nicht; zusammen decken sie den leeren Wert und den fremden Pfad ab.
- `lib.sh` ist POSIX sh (`# shellcheck shell=sh`, Zeile 1): kein `local`, keine Arrays, kein `[[`. Hilfsvariablen brauchen ein Präfix nach dem Vorbild von `wait_socket_path`, sonst überschreibt der Helfer dem Aufrufer etwas.
- `m1_sealed_box.sh` setzt `E2E_OUT="$E2E_ROOT/target/e2e"` und löscht das Verzeichnis zu Beginn als Ganzes, also auch das Unterverzeichnis `m2` von M2. Das geht heute nur gut, weil `run.sh` M1 zuerst fährt; dieses Issue fasst es nicht an, aber wer die `rm -rf "$E2E_OUT"`-Zeilen anrührt, verliert Artefakte.
- Das Löschen darf nur den eigenen Baum treffen: kein Muster über `/tmp/hum-e2e-*` als Ganzes, sonst räumt ein Lauf den Baum eines parallel laufenden zweiten weg.

### Referenzen
`tests/e2e/lib.sh` 10-13 (die Zusage), 206-227 (`unshare -rn`, kein Mount-Namensraum) und 231-243; `tests/e2e/m1_sealed_box.sh` 69-81 und der Schlussschritt `the daemon leaves nothing behind`; `tests/e2e/m2_first_decision/run.sh` 125-145; `.github/workflows/ci.yml`, Artefakt-Pfad `target/e2e`; CONVENTIONS.md 3.11; HUM-021, HUM-036.


## HUM-098 · Der Ereignis-Reduzierer steht zweimal in der Anwendung
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-020, HUM-031 · Blockiert: —

### Kontext
`FlowEvent` ist der einzige Weg, auf dem der Zustand eines Flusses in der Anwendung fortschreibt. Zwei Stellen wenden dieselben Ereignisse auf dieselbe Domäne an: `app/lib/features/intercept/providers/flows.dart:54` (`_apply`) und `app/lib/features/history/providers/history_page.dart:460` (`_apply`). Beide führen dieselbe `switch`-Kette über dieselben Varianten und dieselben `copyWith`-Ketten. `app/lib/features/shell/widgets/tray_host.dart:111` hört auf denselben Strom, reagiert aber nur auf `FlowEventTimedOut` und `FlowEventLagged`; er ist ein Beobachter, kein dritter Reduzierer.

Die beiden Reduzierer sind bereits auseinandergelaufen. Der Review vom 2026-09-04 hat sechs Unterschiede gezählt. Drei davon gehören dem Aufrufer und sollen ihm bleiben: das `_ready`-Tor der Historie vor ihrem `_apply`, `_arrived(flow)` gegen das direkte Einfügen in die Karte bei `FlowEventReceived`, und `_resync()` gegen `reload(keepSelection: true)` bei `FlowEventLagged`. Drei sind Divergenz im Übergang selbst, und jede ist ein eigener Fehler:

1. `FlowEventDecided` setzt in der Historie zusätzlich `deadline: null`, in der Warteschlange nicht.
2. `FlowEventTimedOut` ebenso: die Historie löscht die Frist, die Warteschlange behält sie.
3. `FlowEventResponseChunk` schreibt in der Historie `responseSize` fort; in der Warteschlange steht die Variante in der Gruppe, die nichts tut. Die Warteschlange zeigt die anwachsende Antwortgröße also nie, obwohl das Ereignis dafür ankommt.

Die ersten beiden sind sichtbar: `visibleQueueFlows` (`flows.dart:226`) zeigt entschiedene und abgelaufene Flüsse noch für die Dauer von `queueExitWindow` und sortiert die Liste mit `compareByDeadline` (`flows.dart:254`). Eine solche Zeile sortiert deshalb weiter nach ihrer Frist, mitten unter die wartenden, statt hinter sie. Der Countdown-Ring läuft dabei nicht weiter — `CountdownRing` (`countdown_ring.dart:50`) setzt den Fortschritt nur für `flow.isHeld` —, wohl aber der Text daneben: `request_card.dart:132` ruft `flow.remainingAt(now)` ohne diese Bedingung und zählt damit auf einer bereits entschiedenen Zeile weiter herunter.

Das Muster ist das gleiche wie bei HUM-092: Fachlogik, die zweimal existiert, driftet, und die Tests der einen Seite belegen nichts über die andere.

### Ziel
Ein Reduzierer, der aus einem `Flow` und einem `FlowEvent` den nächsten `Flow` berechnet, als reine Funktion ohne Riverpod und ohne Bildschirmbezug. Beide Provider rufen ihn und behalten nur, was wirklich ihres ist: die Warteschlange ihr Sichtbarkeitsfenster, ihre Sortierung und ihr `_resync`, die Historie ihr Seitenmodell, ihr `_ready`-Tor und ihr `reload`.

### Nicht-Ziel
Keine Änderung an `FlowEvent`, an der Proto oder am Daemon. Keine Zusammenlegung der beiden Provider — sie haben verschiedene Lebensdauern und verschiedene Quellen. Kein neues Zustandsverwaltungs-Paket.

### Betroffene Pfade
- `app/lib/core/domain/flow_reducer.dart` (neu): `Flow applyFlowEvent(Flow flow, FlowEvent event)`
- `app/lib/features/intercept/providers/flows.dart`: `_apply` ruft den Reduzierer
- `app/lib/features/history/providers/history_page.dart`: dito
- `app/lib/features/intercept/widgets/request_card.dart`: der Countdown-Text bekommt dieselbe Bedingung wie der Ring
- `app/test/core/domain/flow_reducer_test.dart` (neu)

### Spezifikation
Der Reduzierer ist total über die Varianten von `FlowEvent`: jede Variante hat einen Fall, geprüft über eine erschöpfende `switch` auf der versiegelten Klasse, damit eine neue Variante den Übersetzer rot macht und nicht stillschweigend nichts tut.

Die drei Divergenzen werden zugunsten der Historie aufgelöst. `FlowEventDecided` und `FlowEventTimedOut` löschen die Frist: ein entschiedener oder abgelaufener Fluss hat keine mehr, und was die Warteschlange während des Ausstiegsfensters zeigen will, holt sie sich aus `decidedAt`. `compareByDeadline` sortiert Flüsse ohne Frist ans Ende, also fällt die ausscheidende Zeile genau dorthin, wo sie hingehört. `FlowEventResponseChunk` schreibt `responseSize` fort, auch für die Warteschlange; der Zähler ist kumulativ, wird also gesetzt und nicht addiert.

`FlowEventReceived` liefert einen ganzen `Flow` und keinen Übergang. Er bleibt beim Aufrufer, weil beide Seiten dort verschiedene Dinge tun, die nichts mit dem Zustand eines vorhandenen Flusses zu tun haben.

Ereignisse zu einem unbekannten Fluss ändern nichts und werfen nicht; das entscheidet weiterhin der Aufrufer über sein `_update`.

### Tests
- `flow_reducer_test.dart`: je ein Fall pro `FlowEvent`-Variante, mit dem erwarteten `Flow` als Ganzem verglichen, nicht feldweise.
- Je ein Test für die drei aufgelösten Divergenzen: nach `FlowEventDecided` und nach `FlowEventTimedOut` ist `deadline` null, `remainingAt` null Sekunden, `holdBudget` null; nach `FlowEventResponseChunk` trägt der Fluss die Größe aus dem Ereignis.
- Ein Test in `flows_test.dart`: eine entschiedene Zeile im Ausstiegsfenster steht in `visibleQueueFlows` hinter jeder wartenden.
- Ein Widget-Test: die Karte einer entschiedenen Zeile zeigt keinen laufenden Countdown-Text mehr.
- Mutationsprobe: den Reduzierer in einem der beiden Provider zurück auf die eigene Kette drehen, dann muss mindestens ein Test rot werden.

### Akzeptanzkriterien
- [ ] `grep -c 'case FlowEventDecided' app/lib` findet außerhalb der Tests genau einen Treffer.
- [ ] Beide Provider rufen `applyFlowEvent`; keiner hat noch eine eigene `switch` über den Übergang.
- [ ] Eine neue `FlowEvent`-Variante macht die Übersetzung rot, ohne dass jemand daran gedacht hat.
- [ ] Die drei genannten Divergenzen sind weg und je durch einen Test festgehalten.
- [ ] `make flutter-analyze` und `make flutter-test` grün.

### Fallstricke
- Das `_ready`-Tor der Historie bleibt beim Aufrufer, sonst verschluckt der Reduzierer Ereignisse aus einer Phase, die er nicht kennt.
- `FlowEventResponseChunk` trägt einen kumulativen Zähler. Setzen, nie addieren — sonst zählt eine Wiederaufnahme des Stroms doppelt.

## HUM-099 · Die Filtersprache wird an drei Stellen verschieden verstanden
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-031 · Blockiert: —

### Kontext
Die Filtersprache der Historie (`host:`, `decision:`, `findings:>0`, `since:10m`, Anführungszeichen, Vergleiche) wird an drei Stellen ausgelegt:

1. `daemon/crates/recorder/src/filter.rs`, Einstieg `parse(input, now_ms)`: die vollständige Sprache, übersetzt nach SQL. Das ist die Wahrheit.
2. `daemon/crates/ipc/src/convert.rs:1134`, `matches_filter`: eine bewusst kleinere Lesart für den Serverpfad ohne Recorder und für den Rust-Fake. Ihr Doc-Kommentar sagt selbst, sie kenne nur `host:`, `state:` und `session:` und behandle alles Übrige als Teilzeichenkette. Sie ist keine versehentliche Kopie, aber sie beantwortet dieselbe Eingabe anders, und der Mensch, der sie tippt, sieht dem Ergebnis nicht an, welcher Pfad geantwortet hat.
3. `FakeFlowFilter` in `app/lib/core/ipc/fake_daemon_client.dart:1561`, mit eigenem `_tokenize`, eigenem `_translate` und eigener Entscheidung, wann `RECORDER_002` fliegt.

Kein Test vergleicht die drei. `daemon/crates/ipc/tests/fake_parity.rs` stellt den Rust-Fake gegen den echten Rust-Dienst; über den Dart-Fake sagt er nichts. Jeder Widget- und Provider-Test der Historie beweist damit nur, dass die Oberfläche zur Lesart des Dart-Fakes passt.

Eine Divergenz ist belegt, und sie zeigt, dass hier nicht nur Doppelung liegt, sondern ein Fehler auf jeder der beiden Seiten: die Eingabe `upgrade:none`.

- Der Recorder baut daraus `upgrade = 'none'` (`filter.rs:222`). Die Spalte hält aber `NULL` oder `'websocket'` (`migrations/V1__init.sql:22`, `writer.rs:953`). Der Ausdruck trifft deshalb nie eine Zeile, obwohl er genau die Zeilen meinen soll, die keine Aufwertung tragen.
- Der Dart-Fake gibt `lower == 'none'` zurück und sieht den Fluss gar nicht an (`fake_daemon_client.dart:1667`). `upgrade:none` lässt dort also **alles** durch und `upgrade:websocket` **nichts**. Der Kommentar daneben erklärt es als Absicht für M1.

Dieselbe Eingabe bedeutet an einer Stelle „nichts" und an der anderen „alles". Beide Antworten sind falsch.

Dieselbe Datei trägt außerdem sechs weitere Rollen auf 2253 Zeilen: den Client selbst (`FakeDaemonClient`, Zeilen 76 bis 1223), die Abspielung eines Skripts (`ScriptedEvent`, `_ScriptedFlow`), eine abbrechbare Verzögerung (`_CancellableDelay`), die Sortierschlüssel (`FakeSortKey`), den Filter und die Startdaten (`_SeededFlow`). Dreißig Dateien importieren sie.

### Ziel
Die Sprache hat einen Datensatz, der sagt, was sie bedeutet, und alle drei Stellen werden gegen ihn geprüft. Wo eine Stelle die Sprache nur teilweise kennt, steht das im Datensatz und nicht nur in einem Kommentar. `upgrade:none` ist auf beiden Seiten repariert.

### Nicht-Ziel
Kein Filtern im Daemon für den Fake (er läuft ohne Daemon, das ist sein Zweck). Keine Übersetzung der Rust-Implementierung nach Dart durch einen Codegenerator. Keine Erweiterung der Sprache. Kein Ausbau von `matches_filter` zur vollen Sprache — es bleibt die kleine Lesart, aber der Datensatz sagt, welche Terme sie kennt, und ein Test hält sie darauf fest.

### Betroffene Pfade
- `tests/fixtures/filter-language.json` (neu): Flüsse, Eingaben, erwartetes Ergebnis je Stelle, erwarteter Diagnostic-Code
- `daemon/crates/recorder/tests/filter_language.rs` (neu)
- `daemon/crates/ipc/tests/filter_language.rs` (neu): prüft `matches_filter` gegen die Spalte „kennt diesen Term"
- `app/test/core/ipc/filter_language_test.dart` (neu)
- `daemon/crates/recorder/src/filter.rs`: `upgrade:none` wird zu `upgrade IS NULL`
- `app/lib/core/ipc/fake_daemon_client.dart`: `upgrade` sieht den Fluss an
- `app/lib/core/ipc/fake/` (neu): Aufteilung auf eine Rolle je Datei; `fake_daemon_client.dart` bleibt als Sammel-Export, damit kein Importeur angefasst wird

### Spezifikation
Die Tabelle ist die Spezifikation der Sprache. Für jede Eingabe steht darin entweder die Menge der Flüsse, die durchkommen, oder der Diagnostic-Code, mit dem die Eingabe abgelehnt wird — und zwar je Stelle, denn `matches_filter` darf weniger können. Der Satz der Flüsse steht in derselben Datei, in einer Form, die alle drei Seiten aufbauen können.

Neue Terme kommen erst in die Tabelle, dann in die Implementierungen. Eine Zeile, die eine Stelle nicht erfüllt, ist ein Fehler dieser Stelle und keine erlaubte Abweichung.

Die Aufteilung der Fake-Datei ist mechanisch: eine Datei je Rolle, kein Umbenennen öffentlicher Namen, keine neue Abstraktion.

### Tests
- Alle drei Tabellenläufe grün, mit derselben Datei als Quelle.
- Mutationsprobe: eine Zeile der Tabelle ändern, dann müssen die betroffenen Läufe rot werden — sonst liest einer die Tabelle nicht wirklich.
- `upgrade:none` und `upgrade:websocket` stehen als eigene Zeilen mit dem Verhalten, das sie haben sollen.

### Akzeptanzkriterien
- [ ] `tests/fixtures/filter-language.json` wird von allen drei Stellen gelesen; keine hat ihre eigene Erwartungstabelle.
- [ ] `upgrade:none` trifft im Recorder genau die Flüsse ohne Aufwertung und im Dart-Fake dieselben.
- [ ] Keine Datei unter `app/lib/core/ipc/fake/` ist länger als 500 Zeilen.
- [ ] Kein Importeur von `fake_daemon_client.dart` wurde angefasst.
- [ ] `make flutter-analyze`, `make flutter-test`, `cargo test -p humanitl-recorder` und `cargo test -p humanitl-ipc` grün.

### Fallstricke
- `since:10m` hängt an einer Uhr; die Tabelle nennt den Bezugspunkt ausdrücklich, und keine Seite liest die Wanduhr.
- Die Tabelle arbeitet in Millisekunden seit der Epoche, nicht in lokalen Zeitstempeln.
- `upgrade IS NULL` lässt sich nicht mit einem Platzhalter parametrisieren wie `= ?`; der Fall braucht einen eigenen Zweig im Bau des SQL.

## HUM-100 · `sandbox/profile.rs` trägt Modell und sicherheitskritische Pfadauflösung zugleich
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-011 · Blockiert: —

### Kontext
`daemon/crates/sandbox/src/profile.rs` hat 2190 Zeilen; die Tests beginnen bei Zeile 1797, davor liegen rund 1795 Zeilen Produktionscode. Sie tragen drei Verantwortungen, die nichts voneinander wissen müssen:

1. Das Profilmodell mit seiner Serde-Abbildung und den Vorgaben, Zeilen 230 bis 714: `SandboxProfile`, `SandboxSection`, `Namespace`, `MountSection`, `WorkMount`, `Symlink`, `NetworkSection`, `Bridge`, `BridgeDirection`, `SeccompSection`, `SocketFamily`, `SocketType`, `SocketFloor` und `SessionContext` (das bei 678 beginnt und bis 714 reicht).
2. Die Einhänge-Politik samt Pfadauflösung, Zeilen 718 bis 1234: `MountRule`, `MountPolicy`, `Scope` und die freien Funktionen `is_meaningful_base`, `is_socket`, `find_socket_below`, `deny`, `spellings`, `resolve_candidates`, `resolve_existing_prefix`.
3. Das Laden, Zusammenführen und Prüfen eines Profils sowie den Bau der bwrap-Argumente: `impl SandboxProfile`, Zeilen 1236 bis 1764, dahinter die freien Helfer `names`, `range` und `union_deny_syscalls` bis 1795.

Der zweite Block ist die Stelle, an der entschieden wird, welcher Pfad des Wirts in der Sandbox sichtbar wird. Er entscheidet über Symlinks, über Schreibweisen desselben Pfades und darüber, ob ein Socket unterhalb eines Verzeichnisses gefunden wird. Er trägt damit die erste der drei Sandbox-Garantien mit. Dass er im selben Modul steht wie die Vorgabewerte der Serde-Abbildung, macht ihn schwerer zu lesen, schwerer zu prüfen und schwerer allein zu testen. `backlog/CONVENTIONS.md` 3.4 beschreibt das Profil, sagt aber nichts darüber, wo die Auflösung wohnt.

### Ziel
Drei Module statt eines, mit derselben öffentlichen Schnittstelle nach außen: `profile/model.rs` (Modell, Serde, `SessionContext`), `profile/mounts.rs` (Politik und Auflösung), `profile/build.rs` (Laden, Zusammenführen, Prüfen, Argumentbau, samt der drei freien Helfer). `profile.rs` wird zu `profile/mod.rs` und exportiert weiter, was heute exportiert wird.

### Nicht-Ziel
Keine Verhaltensänderung, keine neue Prüfung, keine geänderten Diagnostics. Kein Umbenennen öffentlicher Typen. Keine Aufteilung von `daemon/bin/humanitl-shim/src/seccomp.rs` — dort liegt der Filter selbst, und er wird in einem eigenen Issue betrachtet, wenn er wieder wächst.

### Betroffene Pfade
- `daemon/crates/sandbox/src/profile/mod.rs`, `model.rs`, `mounts.rs`, `build.rs`
- Die Tests wandern mit ihrem Gegenstand; `#[cfg(test)] mod tests` wird auf die drei Module aufgeteilt

### Spezifikation
Die Aufteilung ist mechanisch. Was heute privat ist, bleibt privat, soweit es innerhalb eines Moduls bleibt; was über eine Modulgrenze hinweg gebraucht wird, wird `pub(crate)` und nicht `pub`. Ein Test, der heute grün ist, ist danach grün, und kein Test wird umgeschrieben, um die Aufteilung zu ermöglichen.

Am Kopf von `mounts.rs` steht ein Doc-Kommentar, der sagt, was dieses Modul trägt und warum es allein steht: es entscheidet, welcher Pfad des Wirts in der Sandbox sichtbar wird, und ist Teil der ersten Garantie aus `README.md`.

### Tests
- `cargo test -p humanitl-sandbox` grün, mit derselben Anzahl Testfälle wie vorher; die Zahl steht im Commit-Text.
- `tests/escape/` bleibt grün.
- Mutationsprobe an einer Stelle in `mounts.rs` (etwa `spellings` um eine Schreibweise kürzen), die einen Test rot macht — der Beleg, dass die Tests mitgewandert und nicht verwaist sind.

### Akzeptanzkriterien
- [ ] Keine der vier Dateien ist länger als 800 Zeilen.
- [ ] `git diff --stat` zeigt außerhalb von `daemon/crates/sandbox/src/profile*` keine Änderung an Rust-Dateien.
- [ ] Ein `grep` über die `pub`-Zeilen belegt: die öffentliche Schnittstelle der Crate ist unverändert.
- [ ] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- `include_bytes!` und andere pfadbezogene Makros hängen an der Datei, in der sie stehen; nach dem Verschieben stimmen relative Pfade nicht mehr.
- Die Testmodule teilen heute Hilfsfunktionen; die wandern in ein gemeinsames `#[cfg(test)] mod testing` unter `profile/`, nicht in drei Kopien.

## HUM-122 · Die Regel für einen Befehl aus fremdem Wert steht fünfmal und gilt an sieben von siebzehn Stellen

Sprint: 5 · Größe: L · Abhängigkeiten: HUM-043, HUM-075 · Blockiert: —

### Kontext
Ein Wert von außen, der zu einem Befehl wird, den ein Mensch in seine Shell
einfügt, ist der gefährlichste Weg in diesem Baum. Sprint 3 hat diese Gestalt
achtmal gefunden: ein `rm` aus einem gesäuberten Anzeigepfad (HUM-043), ein
`export KEY=VALUE` mit unzitiertem Wert (HUM-106), ein
`chmod 700 /tmp/h; touch /tmp/humanitl-pwn` aus `XDG_RUNTIME_DIR` (HUM-075) und,
als Umkehrung, ein bereits bewiesener Befehl, der ein zweites Mal gesäubert und
dabei hinter dem schließenden Anführungszeichen abgeschnitten wurde (HUM-043b).
Daraus ist eine Regel geworden: Ein Wert wird genau einmal geprüft, an der
Stelle, die ihn erzeugt; danach wird er weder erneut geprüft noch erneut
gesäubert.

Am 2026-09-06 gegen den Code gemessen — nicht geschätzt, jede Zeile
nachgeschlagen. Das Bild ist schlechter als die acht Einzelfälle vermuten
ließen.

**Die Regel steht fünfmal, in drei Lesarten.**

| Ort | Lesart | `sanitize_note`? | Verlust |
|---|---|---|---|
| `sandbox/src/summary.rs:933` `copy_command` | beweisend: lehnt `'a'\''b'` ab | ja | `to_str()`, sonst `None` |
| `sandbox/src/doctor.rs:134` `shell_command` | beweisend, zeichengleiche Kopie | ja | entfällt (nimmt `&str`) |
| `proxy/src/ca.rs:977` `shell_quote` | erzeugend: baut `'a'\''b'` | nein | `to_string_lossy` beim Aufrufer |
| `sandbox/src/bwrap_args.rs:523` `shell_quote` | erzeugend, bis auf das ausgelagerte Prädikat zeichengleich mit `ca.rs` | nein | `to_string_lossy` |
| `config/src/validate.rs:174` `shell_word` | erzeugend, einzeilig | nein | `to_string_lossy` |

Die ersten beiden lehnen genau die Form ab, die die letzten drei **herstellen**.
Der Doc-Kommentar von `copy_command` begründet das: Zwischen einfachen
Anführungszeichen hat kein Zeichen eine Sonderbedeutung, aber `'a'\''b'`
verlässt die Anführungszeichen viermal, und ob jede Shell das gleich liest, ist
genau die Frage, die dort niemand beantworten will. Drei Stellen im selben
Repository beantworten sie mit „ja", ohne es zu sagen.

Dazu kommt: Die drei erzeugenden Fassungen lassen `sanitize_note` aus. Ein Pfad
mit `\n`, mit einer Bidi-Umkehrung oder von 100 kB Länge geht durch sie
unverändert in einen Befehl, den ein Mensch liest, bevor er ihn einfügt — und
`copy_command`s zweite Bedingung existiert gerade dafür, dass das, was im Befund
steht, auch das ist, was der Mensch einfügt. Und alle drei gehen über
`to_string_lossy`: ein Pfad ohne gültiges `UTF-8` wird ein **anderer** Pfad, mit
Ersatzzeichen darin. Das ist wörtlich der Fund von HUM-043, drei Zeilen weiter
unten wieder eingebaut.

**Siebzehn Stellen bauen einen Befehl mit `format!`. Zehn davon setzen einen
rohen Wert ein, ohne jede Zitierung:**

1. `sandbox/src/agent/opencode.rs:586` — `sudo install -m 0755 {binary} /usr/local/bin/opencode`. Der schärfste Fall: `sudo` davor, Pfad roh. Ein ganz gewöhnlicher Pfad mit Leerzeichen (`/home/u/My Tools/opencode`) macht daraus drei Argumente, und `install` tut etwas anderes, als die Zeile zeigt.
2. `recorder/src/error.rs:80` — `ls -ld {shown} && df -h {shown}`, derselbe rohe Pfad zweimal.
3. `humanitld/src/main.rs:1110` — `chmod 700 {dir}`. Dieselbe Zeile, die HUM-075 an einer anderen Datei behoben hat; hier steht sie noch.
4. `humanitld/src/main.rs:1097` — `humanitld --fake <session.jsonl> --socket {own}`.
5. `humanitld/src/main.rs:714` — `openssl x509 -in {path} -noout -subject`.
6. `proxy/src/llm_probe.rs:360` und 7. `:374` — `curl -sS {target.url(...)}`. Die Adresse kommt aus `llm.endpoint`.
8. `sandbox/src/agent/opencode.rs:605` — `curl -sS {base_url(endpoint)}/models`.
9. `humanitl/src/cmd/rules.rs:1011` — `humanitl rules add --action ask --host {matcher} --position 1`.
10. `config/src/load.rs:639` — `humanitl run --profile {wish}`.

Vier Stellen zitieren (`ca.rs:945`, `ca.rs:971`, `validate.rs:98`, `summary.rs:1202`),
drei setzen einen Wert aus geschlossener Menge ein und sind darum heute in
Ordnung, sagen es aber nirgends (`cmd/config.rs:188` ein Schema-Schlüssel,
`cmd/flows.rs:127` und `rules/src/parse.rs:275` je eine Kennung aus Hex).

Keiner der zehn ist ein Angriffsweg mit einem fremden Angreifer: `agent.command`
ist `x-project-scope = "denied"`, die Adresse und der Socket-Pfad kommen aus der
eigenen Konfiguration oder der eigenen Kommandozeile. Der Schaden ist trotzdem
echt und trifft gewöhnliche Nutzer: ein Leerzeichen im Pfad, ein `&` in einer
Adresse, ein Byte ohne `UTF-8` — und der Befehl im Befund tut etwas anderes, als
er zeigt. Bei einer Zeile, die mit `sudo` anfängt, ist das genug.

### Ziel
Eine Stelle, an der aus Wörtern eine Befehlszeile wird oder eben keine. Jede
Stelle, die einen Befehl zum Einfügen anbietet, geht durch sie. Wo ein Wert
nachweislich aus einer geschlossenen Menge kommt, steht das als Zusicherung im
Code und nicht als stille Annahme.

### Nicht-Ziel
Keine Änderung an der beweisenden Regel selbst. Die vier Bedingungen bleiben,
ihre Reihenfolge bleibt, und was heute `None` ergibt, ergibt danach `None`. Kein
neuer Krate, kein Port. Keine zweite Prüfung an einer Stelle, die schon geprüft
hat — das war der Fehler von HUM-043b.

**`bwrap_args::shell_quote` wird nicht zusammengelegt.** Es rendert die
Argumentzeile von `bwrap` zur *Anzeige* (`argv_line`, das UI zeigt sie), nicht
einen Befehl zum Einfügen. Anzeigen und Anbieten sind zwei Zusagen, und die
Zusammenlegung würde beide beschädigen. Es bekommt in diesem Issue nur einen
Namen, der sagt, was es ist, und einen Doc-Kommentar, der die Grenze zieht.

### Betroffene Pfade
- `daemon/crates/sandbox/src/shell.rs` (neu): `quote_word`, `command_line`
- `daemon/crates/sandbox/src/lib.rs`, `summary.rs`, `doctor.rs`
- `daemon/crates/sandbox/src/bwrap_args.rs`: Umbenennung und Doc-Kommentar
- `daemon/crates/sandbox/src/agent/opencode.rs` (zwei Stellen)
- `daemon/crates/proxy/src/ca.rs`, `daemon/crates/proxy/src/llm_probe.rs`
- `daemon/crates/config/src/validate.rs`, `daemon/crates/config/src/load.rs`
- `daemon/crates/recorder/src/error.rs`
- `daemon/bin/humanitld/src/main.rs` (drei Stellen)
- `daemon/bin/humanitl/src/cmd/rules.rs`
- `daemon/crates/sandbox/tests/shell_quoting.rs` (neu)

Die Abhängigkeitsrichtung muss halten (`tools/check-deps.sh`): `humanitl-proxy`,
`humanitl-config` und `humanitl-recorder` dürfen nicht auf `humanitl-sandbox`
zeigen. Die Funktion gehört deshalb wahrscheinlich nach `humanitl-core` neben
`sanitize_note` (`core-types/src/block.rs:79`) und nicht in die Sandbox. Wer das
Issue umsetzt, entscheidet das zuerst und begründet es im Commit; alles andere
hängt daran.

### Spezifikation
`command_line(words: &[&str]) -> Option<String>` trägt die beweisende Regel:
`None` für eine leere Wortliste, ein leeres Wort, ein Wort, das `sanitize_note`
verändert, eine Zitierung, die nicht wörtlich ist, und wenn `shlex::split` der
fertigen Zeile nicht wieder genau diese Wörter in dieser Reihenfolge ergibt.
`quote_word` ist der Einzelfall und teilt sich die Prüfung mit ihr.

Jede der zehn rohen Stellen wird eine von zwei Formen:

- **Der Wert kann beliebig sein** (Pfade, Adressen, Regelmuster): Der Befund
  bekommt seinen Befehl über `command_line`. Ergibt sie `None`, gibt es keinen
  Befehl, sondern einen Satz, der sagt, was zu tun ist — dieselbe Behandlung, die
  `doctor::command_fix` schon hat.
- **Der Wert kommt aus einer geschlossenen Menge** (Schema-Schlüssel, Hex-Kennung):
  Der Typ sagt das. Eine Kennung ist kein `String`, sondern der Typ, der nur
  gültige Kennungen zulässt; ein Schema-Schlüssel kommt aus `schema::field`. Wo
  der Typ es heute nicht sagt, geht der Wert durch `command_line` wie alle
  anderen, statt sich auf eine Annahme zu verlassen, die niemand aufgeschrieben
  hat.

`ca::shell_quote` und `validate::shell_word` entfallen. `bwrap_args::shell_quote`
heißt danach nach seiner Aufgabe (etwa `display_word`) und trägt einen
Doc-Kommentar, der sagt: Das Ergebnis ist zum Lesen, nicht zum Einfügen, und wer
daraus einen Befehl baut, nimmt `command_line`.

`humanitld/src/main.rs:1110` ist derselbe Fall wie HUM-075. Die dortige Lösung
ist der Bezugspunkt; wenn sie hier nicht passt, gehört der Grund in den Commit.

### Tests
- `shell_quoting.rs`: je ein Fall pro abgelehnter Bedingung — leeres Wort,
  Steuerzeichen, Zeilenumbruch, Bidi-Umkehrung, Überlänge, ein Wort mit `'`, ein
  Wort mit `\`, ein Wort mit `$(`.
- Ein Gleichheitstest: für dreißig Pfade liefert `copy_command` denselben
  `Option<String>` wie vor der Zusammenlegung. Die Erwartungen stehen als Tabelle
  im Test, nicht als zweite Implementierung.
- Je ein Test pro behobener Stelle mit einem Pfad, der ein Leerzeichen enthält,
  und einem, der `;` enthält: Entweder ist der Befehl beweisbar zitiert, oder es
  gibt keinen.
- Ein Test über den Baum: Kein `FixAction::CopyCommand` entsteht aus einem
  `format!`, dessen Argumente nicht durch `command_line` gegangen sind. Ein
  `grep`-Test ist dafür zulässig und ehrlicher als keiner — er steht als Skript
  neben `scripts/ci/lint-no-string-errors.sh`, das dieselbe Art Regel schon so
  durchsetzt.
- Mutationsprobe: die wörtliche Prüfung in `command_line` auf `true` festnageln.
  Danach muss auf **jeder** der behobenen Seiten mindestens ein Test rot werden.
  Wird nur eine Seite rot, prüfen die anderen die Regel nicht.

### Akzeptanzkriterien
- [ ] `grep -rn 'fn shell_quote\|fn shell_word\|fn is_literal_word' daemon/crates daemon/bin` findet außerhalb der Anzeigefunktion und der einen gemeinsamen Stelle keinen Treffer.
- [ ] Alle zehn oben genannten Stellen gehen durch `command_line` oder haben einen Typ, der die geschlossene Menge zusichert; im Issue-Commit steht je Stelle, welches von beidem.
- [ ] `summary::copy_command` und `doctor::shell_command` bestehen aus höchstens drei Zeilen.
- [ ] Die Anzeigefunktion heißt nach ihrer Aufgabe und sagt im Doc-Kommentar, dass ihr Ergebnis nicht zum Einfügen ist.
- [ ] Kein Weg von einem Wert in einen `CopyCommand` geht mehr über `to_string_lossy`.
- [ ] Der Gleichheitstest über dreißig Pfade ist grün.
- [ ] Die Mutationsprobe macht auf jeder behobenen Seite mindestens einen Test rot.
- [ ] `tools/check-deps.sh` grün; die Entscheidung über den Ort der Funktion steht im Commit.
- [ ] `make check` grün.

### Fallstricke
- `copy_command` nimmt einen `&Path`, `shell_command` nimmt Wörter. Die
  Umwandlung `to_str()` gehört zum Aufrufer und bleibt dort: Ein Pfad ohne
  gültiges `UTF-8` ist kein Wort, das man zitieren könnte, sondern ein anderer
  Pfad.
- `sanitize_note` gilt für **jedes** Wort, auch für `rm` und `--`. Sie überstehen
  es unverändert, aber die Prüfung darf nicht auf das letzte Wort verkürzt
  werden; sonst hängt die Zusage an der Zusammensetzung des Aufrufers.
- Der Satz „Schritt 4 ist heute unerreichbar" aus `doctor.rs` gehört mit an die
  neue Stelle. Ohne ihn liest der nächste den überlebenden Mutanten als Lücke.
- Zwei der zehn Stellen liegen in `humanitld`, das keine Fehlerpfade verstecken
  darf: Wenn dort kein Befehl entsteht, muss der Satz trotzdem sagen, was zu tun
  ist. Ein Befund ohne `fix` ist erlaubt, ein Befund ohne `why` nicht.

## HUM-123 · `IPC_005` heißt nach dem Regel-RPC und wird längst überall benutzt

Sprint: 5 · Größe: S · Abhängigkeiten: HUM-026 · Blockiert: —

### Kontext
Das Register in `daemon/crates/core-types/src/diagnostics/codes.rs:228` führt
`IPC_005` mit dem Titel „Rules-Anfrage ungültig". Der Titel ist nicht Beiwerk:
`Diagnostic::builder` holt ihn aus dem Register (`diagnostics/mod.rs:149`), und
die `Display`-Form eines Befunds ist `{code}: {title}: {why}`. Er steht damit in
der Kopfzeile jeder Befundkarte, in jeder Zeile auf `stderr` und in
`docs/DIAGNOSTICS.md:72`.

Der Code wird längst weit außerhalb des `Rules`-RPC erzeugt:

- `ipc/src/validate.rs:141` — die Sandbox-Kennung ist keine Kennung.
- `ipc/src/validate.rs:156` und `ipc/src/server.rs:832` — eine Prüfsumme hat keine 32 Bytes.
- `ipc/src/validate.rs:174` — die Anfrage trägt keinen `BodyRef` (seit HUM-026).
- `ipc/src/validate.rs:269` und `ipc/src/server.rs:758` — ein unbekannter Schlüssel in `order_by`.
- `ipc/src/server.rs:796` — ein Cursor, der nicht von `encode_cursor` stammt.

Der Test `session_summary_rpc.rs:221` hält genau diesen Fall fest: Wer eine
Sitzungszusammenfassung mit einer unlesbaren Kennung anfordert, bekommt
`IPC_005` — und liest in der Kopfzeile seiner Karte „Rules-Anfrage ungültig",
einen Satz über einen RPC, den er nicht aufgerufen hat. Das ist die Sorte
Meldung, die einen Menschen an der falschen Stelle suchen lässt, und sie
widerspricht CONVENTIONS 4.13: Der Titel sagt, was nicht ging.

In der Anwendung heißt die Konstante `rulesRequestInvalid`
(`app/lib/core/domain/diagnostic_codes.dart:25`); der Name trägt denselben
Irrtum weiter.

### Ziel
Ein Titel, der alle Fälle des Codes deckt, im Register, in `docs/DIAGNOSTICS.md`
und im Dart-Bezeichner. Die Nummer bleibt: Ein zurückgezogener Code bliebe als
`#[deprecated]` stehen, und hier ist nichts zurückzuziehen, sondern nur richtig
zu benennen.

### Nicht-Ziel
Keine Aufteilung auf mehrere Codes. `IPC_005` ist „die Anfrage lässt sich so
nicht ausführen"; das ist eine Aussage, und der Bereich `ipc` hat nur die
Nummern 1 bis 9. Kein neuer Code für den Sandbox-Fall. Keine Änderung an
`why`-Texten — die sind bereits fallgenau.

### Betroffene Pfade
- `daemon/crates/core-types/src/diagnostics/codes.rs`: Titel und Doc-Kommentar von `IPC_005`
- `docs/DIAGNOSTICS.md`: erzeugt mit `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs`
- `app/lib/core/domain/diagnostic_codes.dart`: `rulesRequestInvalid` wird `requestInvalid`
- die Aufrufer des Dart-Bezeichners
- `daemon/crates/ipc/src/validate.rs`: der Modul-Kommentar in Zeile 22, der `IPC_005` heute den `Rules`- und Decide-Pfaden zuordnet

### Spezifikation
Der Titel wird „Anfrage ungültig". Er ist wahr für jeden der genannten Fälle und
sagt weiterhin, was nicht ging: Die Anfrage, nicht der Zustand und nicht der
Daemon. Der Doc-Kommentar über dem Eintrag zählt die Fälle auf — Operation
fehlt, Regel fehlt oder ist unlesbar, Regel-Id unbekannt, kein Regelspeicher,
Kennung unlesbar, Prüfsumme nicht 32 Bytes, `BodyRef` fehlt, `order_by`
unbekannt, Cursor nicht lesbar — und nennt zu jedem die Stelle. Er sagt
außerdem, was `IPC_005` **nicht** ist: `IPC_004` gehört `Decide`, `IPC_006`
heißt „der RPC existiert, dieser Daemon kann ihn nicht".

`docs/DIAGNOSTICS.md` wird nicht von Hand angefasst, sondern erzeugt; der Anker
`#ipc_005` bleibt, damit ein alter Verweis nicht ins Leere zeigt.

In der Anwendung wird `rulesRequestInvalid` zu `requestInvalid`. Der Wert
`'IPC_005'` bleibt.

### Tests
- Ein Test in `core-types`, der für jeden Code prüft, dass sein Titel keinen
  RPC-Namen trägt, der nicht in seinem Doc-Kommentar vorkommt. Das ist die
  Verallgemeinerung des Fundes und verhindert den nächsten.
- `session_summary_rpc.rs`: der bestehende Fall prüft zusätzlich den Titel, nicht
  nur den Code. Ein Test, der nur `"IPC_005"` liest, hätte diesen Fund nie
  gemacht.
- `cargo test -p humanitl-core --test diag_docs` ohne `UPDATE_DIAG_DOCS` ist
  grün, das heißt: die Datei im Baum ist die erzeugte.

### Akzeptanzkriterien
- [ ] `IPC_005` trägt im Register den Titel „Anfrage ungültig"; sein Doc-Kommentar nennt alle neun Fälle mit Datei und Zeile.
- [ ] `docs/DIAGNOSTICS.md` ist erzeugt, nicht editiert, und der Anker `#ipc_005` steht unverändert.
- [ ] `grep -rn 'rulesRequestInvalid' app/` findet keinen Treffer mehr.
- [ ] Der Test über Titel und Doc-Kommentar aller Codes ist grün und wird rot, wenn man `IPC_005` den alten Titel zurückgibt.
- [ ] `make check` und `make flutter-analyze` grün.

### Fallstricke
- Der Titel steht in `Display`; Tests, die auf `"Rules-Anfrage ungültig"`
  vergleichen, müssen mitgeführt und nicht gelöscht werden.
- `docs/DIAGNOSTICS.md` ist erzeugt. Wer sie von Hand ändert, macht den
  `diag_docs`-Test rot und merkt es erst in CI.

## HUM-124 · Die Terminal-Tests warten ohne Frist auf den Start

Sprint: 5 · Größe: S · Abhängigkeiten: HUM-042 · Blockiert: —

### Kontext
`daemon/crates/ipc/tests/terminal.rs` startet echte Sandboxen. Zwei seiner
Warteschritte haben keine Frist:

- `running_with` (`terminal.rs:242-266`) liest den Ereignisstrom des Starts, bis
  ein `Status` `Running` oder `Failed` meldet — `while let Some(event) = stream.next().await`,
  ohne `timeout`.
- `with_session` (`terminal.rs:275-284`) wartet am Ende auf die erste Antwort auf
  `stop()`, ebenfalls ohne Frist.

Die Schritte **danach** sind gedeckt: `Client::wait_for` und `Client::next`
(`terminal.rs:218-233`) laufen in ein `tokio::time::timeout(WAIT, …)` mit
`WAIT = 20s`. Die Frist steht also überall dort, wo der Test schon läuft, und
fehlt genau dort, wo er noch nicht angefangen hat.

Am 2026-09-05 haben drei Agenten unabhängig voneinander gemeldet, dass
`osc52_does_not_reach_host` hängt, wenn parallel ein zweiter Testlauf des
Arbeitsbereichs Sandboxen startet. Allein läuft derselbe Test in 1,3 Sekunden.
Ein hängender Test sagt niemandem etwas: Die CI bricht nach ihrer Gesamtfrist
ab, und im Bericht steht eine Zeitüberschreitung ohne Ort. Genau dieselbe Regel
steht schon ausgeschrieben im Kommentar von `m3_wait_ready`
(`tests/e2e/m3_agent_inside/run.sh:333-341`), dort für Shell-Skripte. Sie gilt
für die Rust-Tests genauso.

**In der CI ist es messbar.** Der Schritt `rust-test` (`make rust-build rust-test`,
also `cargo test --workspace`) läuft grün in 184 bis 194 Sekunden. Die
öffentliche Actions-API zeigt daneben zwei Läufe, die abgebrochen wurden,
nachdem sie 2 144 beziehungsweise **13 784 Sekunden** (3 Stunden 49 Minuten) in
demselben Schritt standen — beide am 2026-09-05. Ein Lauf, der das
Zwanzigfache seiner grünen Dauer braucht, ist nicht langsam, sondern steht.
Vier weitere Läufe sind in demselben Schritt nach 96 bis 133 Sekunden rot
geworden, also **vor** der grünen Dauer; das ist ein zweites, anderes Phänomen
und gehört nicht hierher, solange niemand die Protokolle gelesen hat (sie
brauchen Admin-Rechte am Repository).

**Und er ist in der Laufliste unsichtbar.** `.github/workflows/ci.yml:28-31` setzt
`concurrency: group: ci-${{ github.ref }}` mit `cancel-in-progress: true`. Ein
hängender Lauf endet deshalb nicht als `failure`, sondern als `cancelled`,
sobald der nächste Push kommt — und `cancelled` liest jeder als „überholt", nicht
als „stand still". Genau deshalb sind die beiden langen Läufe niemandem
aufgefallen: Man sieht sie nur, wenn man die Dauer neben die grüne Dauer legt.
Am 2026-09-06 stand ein Lauf auf einem Commit, der ausschließlich zwei
Backlog-Dateien ändert, über eine Stunde in `rust-test`, während alle zehn
anderen Jobs grün waren (der längste, `flutter-analyze-test`, in 366 Sekunden).
Der Hänger hängt an keiner Codeänderung.

**Lokal reproduziert er nicht, und das ist ein Befund und kein Freispruch.**
Am selben Tag auf dieser Maschine gemessen: `cargo test --workspace` läuft
vollständig durch, 90 Suiten, null Fehler, langsamste Suite 13,68 Sekunden
(`daemon_end_to_end.rs`). `terminal.rs` läuft dabei wirklich — sechs Tests in
1,35 Sekunden, kein `ESC5-SKIP` in der Ausgabe, `bwrap` 0.12.0 vorhanden und der
Shim gebaut. Der Unterschied zur CI ist die Enge: vier virtuelle Kerne statt
acht, ein anderer Kernel, `kernel.apparmor_restrict_unprivileged_userns=0` per
`sysctl` gesetzt.

Das ändert nichts an der Aufgabe, sondern schärft sie. Eine Frist behebt den
Hänger nicht — sie macht ihn **sichtbar**: Aus einem Lauf, der drei Stunden steht
und dann als „cancelled" verschwindet, wird ein roter Test, der sagt, worauf er
gewartet hat. Das ist der ganze Zweck.

Der Skript-Text von `osc52_does_not_reach_host` endet mit
`while :; do sleep 0.05; done`. Der Agent läuft also weiter, bis ihn jemand
beendet. Das ist gewollt und Teil der Aussage; es macht das fehlende Zeitlimit
davor aber teurer, weil nichts von selbst endet.

### Die Ursache, am 2026-09-06 lokal gefunden

**Der Hänger ist kein Warten in `async`. Es ist ein `waitpid` auf eine Sandbox,
die niemand umgebracht hat.**

Auf dieser Maschine standen drei Testprozesse aus früheren Läufen: 45, 40 und 29
Minuten, alle bei null Prozent CPU, jeder mit zwei Threads in `do_wait` und
**zwei lebenden `bwrap`-Kindern**. Die Kinder gehörten zu
`osc52_does_not_reach_host` und `osc8_and_title_are_inert`, deren Agentenskript
mit `while :; do sleep 0.05; done` endet und von selbst nie aufhört.

Warum sie überlebten, steht in `ipc/src/sandbox.rs`, `Inner::stop`: Der RPC
meldet **zuerst** `status_event(Stopping)` und tötet die Sandbox **danach**
(`handle.terminate(KILL_GRACE)`). `with_session` nahm vom Stop-Strom nur das
erste Ereignis und ließ ihn dann fallen. Damit scheitert das nächste
`tx.send(...)`, und `stop` kehrt an genau dieser Stelle zurück — vor dem Töten.
Der Test war fertig, die Sandbox lief weiter, und die Testbinärdatei blockierte
beim Beenden im `wait` auf ein Kind, das niemand mehr beendete.

Das ist die Signatur, die die CI viermal gezeigt hat: **jeder Test grün, danach
ein Prozess, der nicht endet.** Kein roter Test, keine Meldung, nur Stille bis
zum Abbruch.

Gemessen vor und nach der Behebung, dreimal wiederholt: vorher ließ jeder Lauf
zwei `bwrap` zurück, danach null, und der Lauf endet in zwei Sekunden statt zu
stehen.

**Was das für die Fristen bedeutet.** Sie bleiben richtig und sie bleiben in
diesem Issue — aber sie hätten diesen Hänger nicht gefangen, und der Issue soll
das nicht behaupten. Eine Frist um ein `await` sieht ein blockierendes `wait`
auf einem anderen Thread nicht. Beides wird gebraucht: das Leeren des
Stop-Stroms behebt die bekannte Ursache, die Fristen machen die nächste
sichtbar.

**Und um den Stopp herum darf keine Frist stehen.** Eine Frist bricht ab, und
Abbrechen heißt hier den Empfänger fallen lassen — genau die Ursache. Ein
Review schlug vor, das Leeren mit `tokio::spawn` abzusetzen, damit es die Frist
überlebt. Das trägt nicht, und es ist gemessen: Läuft die Frist ab, kehrt
`with_session` zurück, der Test endet, `#[tokio::test]` fährt die Laufzeit
herunter, und die abgesetzte Aufgabe wird mitten im Lauf verworfen. Mit einer
Frist von einer Millisekunde standen danach drei Tests über sechzig Sekunden.

Der Stopp wird deshalb **gemessen statt abgeschnitten**: bis zum Ende lesen, die
Dauer nehmen, hinterher beurteilen. Ein Stopp, der wirklich nie zurückkommt,
hängt weiter — das wäre ein Fehler des Daemons, nicht des Gerüsts, und er hat
sein eigenes Issue (HUM-128).

### Ziel
Der Stop-Strom wird bis zum Ende gelesen, damit `stop` bis zum Töten kommt und
keine Sandbox einen Test überlebt.

Und: kein Wartepunkt in diesem Testmodul ohne Frist, und jede abgelaufene Frist
sagt, worauf sie gewartet hat und was zuletzt zu sehen war.

**Und danach dasselbe für den ganzen Arbeitsbereich.** Fristen in einer Datei
binden eine Datei. Hängt der Lauf woanders — und welche Datei es ist, weiß
niemand, solange die Protokolle Admin-Rechte am Repository brauchen —, ändert
dieses Issue nichts. Der allgemeine Weg ist `cargo-nextest`: Es fährt jeden Test
in einem eigenen Prozess und kennt `slow-timeout` mit `terminate-after`, also
wird ein hängender Test nach n Sekunden zu einem **benannten** Fehlschlag, statt
den Job stehen zu lassen. `cargo test` kann das nicht; es hat keine Frist je
Test.

Das ist eine Änderung an der Werkzeugkette der CI und nicht an diesem Testmodul,
deshalb steht es hier als Empfehlung und nicht in den Akzeptanzkriterien. Die
Reihenfolge spricht dafür, es zuerst zu tun: Mit `terminate-after` benennt der
nächste hängende Lauf seinen Test von selbst, und die Fristen hier wären dann
eine zweite Sicherung statt die einzige.

**Vier Folgen, die ein Review beim Nachrechnen fand und die zur Empfehlung
gehören, damit niemand sie später entdeckt:**

1. **Doctests laufen nicht mit.** `cargo nextest run` fährt keine. Der
   Arbeitsbereich hat welche, die etwas behaupten — `crates/rules/src/lib.rs`
   und `crates/findings/src/lib.rs` prüfen darin —, und der heutige Lauf zeigt
   `Doc-tests` für zehn Kisten. Ein Wechsel ohne zusätzliches
   `cargo test --doc` lässt sie still fallen.
2. **Die Escape-Tests lesen die Ausgabe wörtlich.** `esc-5-filesystem.sh`
   entscheidet an der Zeichenkette `1 passed; 0 failed` und am Marker
   `ESC5-SKIP` auf stdout. `nextest` schreibt weder das eine noch das andere und
   verbirgt die Ausgabe bestandener Tests. Beide Terminal-Fälle von ESC-5
   kippten auf `fail`.
3. **Die Nebenläufigkeit steigt, sie sinkt nicht.** `cargo test` fährt eine
   Testbinärdatei nach der anderen; `nextest` fährt sie gleichzeitig. Auf einem
   Läufer mit vier Kernen erhöht das die Zahl gleichzeitiger echter Sandboxen —
   genau die Bedingung, unter der der Hänger auftritt. `test-threads` gehört
   dann in eine `.config/nextest.toml`.
4. **Die Installation.** Übersetzen kostet Minuten, und der bequeme Weg ist ein
   ungepinntes `curl | tar` von `get.nexte.st`. `ci.yml` sagt oben: „Every
   action is pinned to a commit SHA."

Dazu die Einordnung, die daraus folgt: Der Hänger dieses Issues liegt **in**
dieser Datei, und dort benennt ihn die Frist schon. `nextest` ist das Netz für
den nächsten, den niemand kennt — nicht die Diagnose für diesen.

### Nicht-Ziel
Keine Änderung an dem, was die Tests prüfen. Kein Ausschalten von Tests unter
Last, kein `#[ignore]`. Keine Serialisierung des Arbeitsbereichs über eine
globale Sperre — das verdeckte die Frage, statt sie zu beantworten.

### Betroffene Pfade
- `daemon/crates/ipc/tests/terminal.rs`: `running_with`, `with_session`, ein gemeinsamer Helfer
- `daemon/crates/ipc/tests/sandbox_start.rs`, falls dort dieselbe Gestalt steht

### Spezifikation
Ein Helfer `within(label: &str, deadline: Duration, fut)` legt eine Frist um ein
Warten und bricht bei Ablauf mit einer Meldung ab, die `label` nennt. Er wird von
`running_with` und `with_session` benutzt.

`running_with` bekommt eine Frist von `START_WAIT` (60 Sekunden; ein Start kostet
hier zwei Sekunden, unter Last mehr, und die Zahl darf großzügig sein, solange
sie existiert). Läuft sie ab, endet der Test rot mit: worauf gewartet wurde
(`Status Running`), welcher Zustand zuletzt kam, und wie viele Ereignisse
insgesamt eintrafen. Ein Start, der `Failed` meldet, bleibt wie heute ein
übersprungener Test mit `SKIP_MARKER` — das ist eine Umgebung ohne Sandbox und
kein Fehler des Codes.

`with_session` bekommt eine Frist von `WAIT`. Läuft sie ab, endet der Test rot,
**nachdem** ein etwaiger Panik-Ausgang des Rumpfes weitergereicht wurde: Die
Aussage des Tests ist wichtiger als die Aufräumfrist, und ein `panic` im Rumpf
ist der interessantere Befund.

### Tests
- Ein Test des Helfers selbst: `within` über ein Future, das nie fertig wird,
  endet innerhalb der Frist mit einer Panik, deren Text das Label enthält.
- Mutationsprobe: die Frist in `running_with` auf `Duration::MAX` setzen. Der
  Helfer-Test bleibt grün, der Test aus dem vorigen Punkt wird rot — das zeigt,
  dass die Frist und nicht nur ihr Helfer geprüft wird.

### Akzeptanzkriterien
- [x] **Ein Lauf hinterlässt keine Sandbox.** Vor und nach `cargo test -p humanitl-ipc --test terminal` dieselbe Zahl `bwrap`-Prozesse, dreimal wiederholt; der Prozess endet, statt im `wait` zu stehen.
- [x] Jeder Stop-Strom wird bis zum Ende gelesen, nicht bis zum ersten Ereignis; kein Aufruf von `stream(stop())` endet mehr auf `.next()`.
- [x] Die drei Stop-Ströme haben **keine** Frist, und daneben steht, warum: Abbrechen heißt den Empfänger fallen lassen, und das ist die Ursache selbst. Ihre Dauer wird stattdessen gemessen und hinterher beurteilt.
- [x] Jeder **andere** Wartepunkt auf einen Strom steht in `within`, `within_at` oder `timeout`.
- [x] Eine abgelaufene Frist nennt Label, letzten Zustand und Ereigniszahl.
- [x] Ein Start, der `Failed` meldet, überspringt weiterhin mit `SKIP_MARKER`, statt rot zu werden.
- [x] `cargo test -p humanitl-ipc --test terminal` ist grün, allein und parallel zu einem zweiten Lauf des Arbeitsbereichs.
- [x] `make check` grün.

### Fallstricke
- `with_session` reicht die Panik des Rumpfes mit `resume_unwind` weiter. Eine
  Frist, die davor abbricht, verschluckt die eigentliche Fehlermeldung; die
  Reihenfolge in der Spezifikation ist deshalb bindend.
- Die Frist gehört nicht in `Client::attach`: Dort ist das Warten schon gedeckt,
  und eine zweite Frist an derselben Stelle verdoppelt nur die Zahl, die man
  später anpassen muss.
- Eine großzügige Frist ist kein Verzicht auf die Frist. 60 Sekunden, die etwas
  sagen, sind besser als kein Limit, das nichts sagt.

## HUM-125 · Die Hinweiszeile erreicht nur ein Terminal, und erst nach der Prüfung

Sprint: 5 · Größe: M · Abhängigkeiten: HUM-042, HUM-067 · Blockiert: —

### Kontext
Wenn eine Anfrage des Agenten auf eine Entscheidung wartet, schreibt der Daemon
eine Zeile: `[humanitl] request held: GET example.com/pfad · waiting for you`.
Sie entsteht in `HeldNotices` (`daemon/crates/ipc/src/terminal.rs:430-505`) aus
dem Ereignisstrom der Warteschlange. Zwei Dinge stimmen daran nicht.

**Sie kommt zu spät.** `HeldNotices::run` (`terminal.rs:443`) ruft
`self.queue.subscribe()` als erste Zeile — und die Aufgabe wird erst in
`SandboxService::attend` (`sandbox.rs:987`) gestartet, also **nach** der
Isolationsprüfung (`sandbox.rs:964`), nach der Momentaufnahme und nach dem
Öffnen des Terminals. Der Prozess des Agenten läuft zu diesem Zeitpunkt schon:
`launch` (`sandbox.rs:1534-1605`) hat ihn gestartet, bevor `attend` überhaupt
anfängt. `HoldQueue::subscribe` ist ein Rundfunk; wer später abonniert, bekommt
nichts von vorher. Eine Anfrage, die der Agent in dieser Spanne stellt und die
gehalten wird, erzeugt also nie eine Zeile. Die Spanne ist keine theoretische:
Die Isolationsprüfung liest `/proc` und macht echte Syscalls, und ein Agent, der
sofort loslegt, ist der Normalfall und nicht der Ausnahmefall.

**Sie erreicht nur einen Strom.** `TerminalHub::notice` (`terminal.rs:222`)
schreibt in den Ring und den Rundfunk des Terminals. Der `Sandbox`-Strom bekommt
sie nicht: `SandboxEvent` (`proto/humanitl/v1/humanitl.proto:913-930`) trägt
`LogLine log = 5`, aber `notice` schreibt nie dorthin. Wer `humanitl run` tippt,
liest den `Sandbox`-Strom (`daemon/bin/humanitl/src/cmd/run.rs:225-276`) und
sieht die Zeile nie. Für ihn steht der Agent still, ohne dass irgendwo stünde,
warum. `docs/CONFIG.md:203` beschreibt `ui.terminal_notices` als „Zeile im
Bytestrom, die ein Vollbild-TUI überschreibt"; das ist für die Anwendung richtig,
lässt die Kommandozeile aber ohne Antwort auf die einzige Frage, die sie in dem
Moment hat.

### Ziel
Die Hinweiszeile entsteht für jeden gehaltenen Fluss dieser Sitzung, auch für
einen, der vor der Isolationsprüfung gehalten wurde, und sie erreicht beide
Ströme: das Terminal als Bytes an einer Folgengrenze, den `Sandbox`-Strom als
`LogLine`.

### Nicht-Ziel
Keine neue Nachricht in der Proto — `LogLine` gibt es. Keine Richtung Proxy zu
Terminal (ARCHITECTURE 1.2: „Niemand fragt den Proxy nach seinem Zustand, alle
hören zu"); es bleibt beim Abonnement. Kein Vorziehen des Abonnements auf den
blockierenden Faden, der `bwrap` startet — der Grund dafür steht als Kommentar in
`sandbox.rs:975-977` und gilt weiter.

### Betroffene Pfade
- `daemon/crates/ipc/src/terminal.rs`: `HeldNotices` bekommt `subscribe` und `drain` statt eines `run`
- `daemon/crates/ipc/src/sandbox.rs`: `attend` abonniert früh, leert später
- `daemon/bin/humanitl/src/cmd/run.rs`: nichts zu ändern, `Event::Log` steht schon
- `app/lib/features/sandbox/…`: der Log-Reiter, dessen leerer Zustand heute „ein Start und ein Stopp schreiben je eine Zeile" verspricht
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb`
- `daemon/crates/ipc/tests/terminal.rs`
- `docs/CONFIG.md` (erzeugt), `backlog/CONVENTIONS.md` 4.4

### Spezifikation
`HeldNotices` wird in zwei Schritte geteilt. `subscribe(&self) -> HeldNotices Receiver`
gibt das Abonnement zurück und ist billig, nicht blockierend und ohne
`TerminalHub`. `drain(receiver, hub, tx)` liest daraus, bildet die Zeile wie
heute (`line_for`, `line`, `sanitize_note` in `notice_line`) und gibt sie an
beide Senken.

`attend` ruft `subscribe` als erste Anweisung, noch vor `started_line`, und hält
den `Receiver`. Erst nachdem das Terminal offen ist, startet es die Aufgabe mit
`drain`. Der Rundfunk puffert in der Zwischenzeit; ein `Lagged` wird wie heute
mit `continue` übergangen, denn die Warteschlange selbst steht in der Oberfläche
und ein verlorener Hinweis ist folgenlos.

Die zweite Senke ist `tx.clone()`, derselbe `mpsc::Sender<v1::SandboxEvent>`, den
`attend` schon hat. Die Zeile geht als `log_event(line)` hinaus, also mit
demselben Text, den das Terminal bekommt, aber ohne die `\r\n`-Umrahmung aus
`notice_line` — die gehört dem Bytestrom und nicht einer Nachricht mit Feldern.
`ui.terminal_notices` schaltet weiterhin **nur** den Bytestrom; der `Sandbox`-Strom
bekommt die Zeile immer. Der Schlüssel heißt, was er tut.

Der leere Zustand des Log-Reiters verspricht heute zwei Zeilen je Sitzung. Er
bekommt einen Satz dazu, der die Hinweiszeilen nennt. Der ARB-Schlüssel wird
angepasst, `en` ist die Quelle.

### Tests
- Ein Test, der einen Fluss **vor** dem Öffnen des Terminals in die Warteschlange
  legt und danach die Zeile im Terminal sieht. Er wird rot, wenn man das
  Abonnement zurück hinter die Isolationsprüfung schiebt.
- Ein Test, der dieselbe Zeile im `Sandbox`-Strom als `LogLine` sieht.
- Ein Test mit `ui.terminal_notices = false`: keine Bytes im Terminal, die
  `LogLine` trotzdem da.
- Ein Test der Reihenfolge: Die `LogLine` und die Bytes tragen denselben Text.
- Mutationsprobe: `drain` nur noch an `hub` geben. Der zweite Test wird rot.

### Akzeptanzkriterien
- [ ] `attend` abonniert die Warteschlange vor `check_isolation_or_kill`; der Test mit dem früh gehaltenen Fluss ist grün und wird rot, wenn man die Zeile zurückschiebt.
- [ ] Eine gehaltene Anfrage erzeugt in `humanitl run` die Zeile `[humanitl] request held: …`, gemessen an einem Lauf und nicht an einer Absicht.
- [ ] `ui.terminal_notices = false` unterdrückt die Bytes und nicht die `LogLine`; `docs/CONFIG.md` sagt das.
- [ ] Der leere Zustand des Log-Reiters nennt die Hinweiszeilen; `en` und `de` sind beide gepflegt.
- [ ] Die Aufgabe wird am Sitzungsende weiterhin abgebrochen; `two_sessions_leave_nothing_behind` bleibt grün.
- [ ] `make check` und `make flutter-test` grün.

### Fallstricke
- Der `Receiver` muss zwischen `subscribe` und `drain` **gehalten** werden. Ein
  `subscribe()`, dessen Ergebnis fallen gelassen und später neu geholt wird, ist
  genau der heutige Zustand mit mehr Zeilen.
- Der Rundfunkpuffer ist endlich. Zwischen Abonnement und Leeren liegt hier eine
  kurze Spanne; ein `Lagged` bleibt trotzdem möglich und bleibt folgenlos
  behandelt.
- `tx` ist der Sender des Startstroms. Ist der Client weg, schlägt `send` fehl;
  das darf die Aufgabe nicht beenden, denn das Terminal hängt noch daran.
- Die Aufgabe hält einen `TerminalHub` und damit den `SandboxHandle`. Der
  `abort()` am Ende von `attend` bleibt, und der Grund dafür bleibt als
  Kommentar stehen.

## HUM-126 · M1 und M2 lesen ihre Bereitschaft aus einer Fifo ohne Frist

Sprint: 5 · Größe: S · Abhängigkeiten: HUM-046 · Blockiert: —

### Kontext
`m3_agent_inside/run.sh` trägt seit HUM-046 einen Helfer `m3_wait_ready`
(Zeilen 342-357) und dazu einen Kommentar, der die Regel ausspricht:

> Die ersten Fassungen von M2 und M3 lasen die Zeile mit `read < fifo`. Das
> wartet unbegrenzt, und zwar schon beim Öffnen: Kommt der Server gar nicht hoch
> — ein Syntaxfehler, ein belegter Port, ein fehlendes `python3` —, steht der
> ganze Lauf, bis die CI ihn nach dreissig Minuten abbricht, und im Bericht steht
> eine Zeitüberschreitung, die nichts sagt.

M3 wurde repariert, M2 nicht, und M1 auch nicht. `m2_start_upstream`
(`tests/e2e/m2_first_decision/run.sh:271-293`) macht ein `mkfifo` und liest mit
`read -r m2_ready < "$m2_fifo"`. `start_fake_upstream` (`tests/e2e/lib.sh:287-304`)
tut dasselbe für M1. In beiden Fällen blockiert schon das Öffnen der Fifo, und
zwar auch dann, wenn der Serverprozess bereits gestorben ist: Ein toter Schreiber
öffnet die Fifo nie, und der Leser wartet auf ein Ereignis, das nicht mehr
kommen kann. Der Kommentar in `lib.sh:295-296` behauptet das Gegenteil
(„bricht er vorher ab, bleibt FAKE_HTTP leer und der Aufrufer merkt es sofort")
— das gilt für einen Schreiber, der die Fifo geöffnet und dann geschlossen hat,
nicht für einen, der nie so weit kam.

Das ist derselbe Mangel, den HUM-046 in seinem eigenen Gerüst behoben hat, an
zwei Stellen, die davon nichts wissen. Der Helfer steht in einem
Meilenstein-Skript statt in `lib.sh`, also konnte ihn niemand erben.

### Ziel
Ein Helfer in `lib.sh`, den alle drei Meilensteine benutzen. Ein Server, der
nicht hochkommt, beendet den Lauf innerhalb seiner Frist mit seinem Protokoll im
Text, und ein Server, der vorher stirbt, sofort.

### Nicht-Ziel
Keine Änderung an dem, was die Skripte prüfen, und keine an den Bereitschafts-
zeilen der Python-Server. Keine neue Abhängigkeit; `grep`, `kill -0` und `sleep`
reichen.

### Betroffene Pfade
- `tests/e2e/lib.sh`: `e2e_wait_ready` und `e2e_ready_failed` (neu), `start_fake_upstream` benutzt sie
- `tests/e2e/m2_first_decision/run.sh`: `m2_start_upstream`
- `tests/e2e/m3_agent_inside/run.sh`: `m3_wait_ready` und `m3_ready_failed` entfallen zugunsten der Helfer aus `lib.sh`
- `tests/e2e/README.md`: die Regel steht dort einmal

### Spezifikation
`e2e_wait_ready FILE PID SECONDS PATTERN` wartet, bis `FILE` eine Zeile enthält,
die auf `PATTERN` passt (Vorgabe `^READY `), und gibt sie auf `stdout` aus.
Rückgabe 0 mit der Zeile, 1 nach Ablauf der Frist, 2 wenn `kill -0 PID`
fehlschlägt, bevor die Zeile da ist. Gewartet wird in Schritten von 0,1
Sekunden; das ist ein Poll und kein Blockieren, und genau deshalb kann daneben
die zweite Frage gestellt werden, ob der Prozess überhaupt noch lebt.

`e2e_ready_failed WHAT PID CODE LOG` schreibt die Meldung: bei Code 2, dass der
Prozess starb, bevor er Bereitschaft meldete, sonst, dass die Frist ablief;
beide mit dem Protokoll des Servers im Text und, im zweiten Fall, einem `kill`
davor.

Beide Server schreiben ihre Bereitschaftszeile weiterhin nach `stdout`; die
Umleitung geht statt in eine Fifo in eine leere Datei, die vorher angelegt wird.
Die Prüfung der **ganzen** Zeile bleibt, wo sie heute steht: `m2_start_upstream`
vergleicht weiter gegen `READY http=$M2_HTTP_PORT https=$M2_HTTPS_PORT`, denn
ohne Zertifikat meldete der Server `https=-` und der Lauf bewiese in Schritt 7
nur, dass auf 443 niemand antwortet.

Die Frist ist 20 Sekunden, wie in M3.

### Tests
- Ein Selbsttest in `lib.sh` hinter `--self-test`, wie ihn
  `scripts/ci/lint-no-string-errors.sh` schon hat: `e2e_wait_ready` gegen eine
  Datei, die nie beschrieben wird, endet mit 1 innerhalb der Frist; gegen einen
  Prozess, den der Test vorher beendet, mit 2; gegen eine Datei mit der Zeile
  mit 0 und der Zeile auf `stdout`.
- Ein Lauf von M1 und M2 mit einem Server, der absichtlich nicht startet
  (`--http-port 1`, ein Port, den ein unprivilegierter Prozess nicht bekommt):
  beide enden rot innerhalb von 25 Sekunden mit dem Protokoll im Text, nicht
  nach der Gesamtfrist der CI.
- Kein `mkfifo` mehr in `tests/e2e/`.

### Akzeptanzkriterien
- [ ] `grep -rn 'mkfifo' tests/e2e/` findet keinen Treffer.
- [ ] `grep -rn 'read -r .* < ' tests/e2e/` findet keinen Treffer, der auf eine Fifo liest.
- [ ] `e2e_wait_ready` und `e2e_ready_failed` stehen in `lib.sh`; M1, M2 und M3 rufen sie, und `m3_wait_ready` gibt es nicht mehr.
- [ ] Der Selbsttest deckt alle drei Rückgabewerte und läuft in `make check`.
- [ ] Ein Server, der nicht hochkommt, beendet M1 und M2 innerhalb von 25 Sekunden mit seinem Protokoll im Text — gemessen, nicht behauptet.
- [ ] `E2E_ONLY=m1 make e2e` und `E2E_ONLY=m2 make e2e` sind grün.

### Fallstricke
- `grep -m 1` beendet sich beim ersten Treffer; ohne `|| true` reißt es unter
  `set -e` den Lauf ab, wenn noch nichts da ist.
- Die Datei muss vor dem Start des Servers existieren und leer sein (`: > "$file"`),
  sonst liest `grep` im ersten Durchlauf einen Rest des vorigen Laufs.
- `kill -0` auf einen Zombie gelingt. Der Aufrufer muss den Prozess deshalb am
  Ende weiterhin über `wait` einsammeln; das tut `stop_fake_upstream` schon.
- Der Kommentar in `lib.sh:295-296` ist falsch und wird ersetzt, nicht verschoben.

## HUM-127 · Der Tray-Befehl gilt nur für apt, und dort ändert sich der Paketname

Sprint: 5 · Größe: S · Abhängigkeiten: HUM-044 · Blockiert: —

### Kontext
`DOCTOR_009` meldet, dass die Tray-Bibliothek fehlt, und bietet dazu
`TRAY_INSTALL_COMMAND` an (`daemon/crates/sandbox/src/doctor/checks.rs:45`):

```rust
/// Der Befehl, der die Tray-Bibliothek auf Debian und Ubuntu nachinstalliert.
const TRAY_INSTALL_COMMAND: &str = "sudo apt install libayatana-appindicator3-1";
```

Der Doc-Kommentar sagt selbst, für welche zwei Distributionen die Zeile gilt,
und der Befund bietet sie trotzdem jedem an. Auf Fedora, Arch und openSUSE
bekommt der Mensch einen Befehl, der mit „command not found" endet — in einem
Befund, dessen einzige Aufgabe es ist, ihm den nächsten Schritt zu nennen.

HUM-044 hat denselben Fehler für bubblewrap behoben: `install_command()` in
`daemon/crates/sandbox/src/os_release.rs` wählt über `ID` und `ID_LIKE` aus vier
festen Literalen. Hier reicht diese Abbildung aber nicht. Bei bubblewrap heißt
das Paket überall `bubblewrap`, es wechselt nur der Verwalter. Bei der
Tray-Bibliothek wechselt **auch der Paketname**, und ein falscher Paketname ist
schlechter als kein Befehl: Er schickt die Fehlersuche in eine Richtung, in der
nichts ist, und der Mensch glaubt danach, es liege nicht am Paket.

### Ziel
Der Befund nennt für die Distribution, auf der er entsteht, den Befehl, der dort
wirklich das Paket installiert — oder er nennt keinen und sagt stattdessen, was
zu suchen ist.

### Nicht-Ziel
Keine Installation durch die Anwendung. Keine Erweiterung von `os_release.rs` um
eine allgemeine Paketdatenbank; diese eine Tabelle reicht, und die nächste
Bibliothek bekommt ihre eigene. Keine Änderung an `DOCTOR_009` selbst — der
Befund ist richtig, nur sein Vorschlag ist es nicht.

### Betroffene Pfade
- `daemon/crates/sandbox/src/os_release.rs`: die Abbildung bekommt neben dem Verwalter einen Paketnamen als Parameter
- `daemon/crates/sandbox/src/doctor/checks.rs`: `TRAY_INSTALL_COMMAND` entfällt
- `daemon/crates/sandbox/tests/os_release.rs`

### Spezifikation
Aus `install_bubblewrap()` wird eine Funktion, die Verwalter und Paketnamen
trennt: der Verwalter kommt wie heute aus `ID`/`ID_LIKE`, der Paketname aus einer
Tabelle je Bibliothek. Die vier Literale bleiben Literale — es wird weiterhin
**kein** Text aus `/etc/os-release` in einen Befehl gesetzt (HUM-122, und der
Test `a_hostile_id_never_reaches_the_command` bleibt grün).

Jeder Paketname wird gegen den Paketindex seiner Distribution geprüft, und die
Quelle steht als Kommentar über dem Eintrag: welche Distribution, welche
Version, wo nachgesehen. Ein Name ohne Beleg kommt nicht in die Tabelle.

Für eine Distribution ohne belegten Namen entsteht kein `CopyCommand`. Der
Befund trägt dann nur seinen Satz und `docs` — dieselbe Behandlung, die
`doctor::command_fix` schon für einen nicht beweisbar zitierbaren Befehl hat.
Ein Befund ohne `fix` ist erlaubt; ein Befund, dessen `fix` nicht funktioniert,
ist es nicht.

### Tests
- Je ein Fall pro Distributionsfamilie: der erwartete Befehl, wörtlich, oder das
  begründete Fehlen.
- Der bestehende `a_hostile_id_never_reaches_the_command` deckt die neue
  Funktion mit ab; ohne ihn wäre die Tabelle ein neuer Weg für Dateitext in
  einen Befehl.
- Mutationsprobe: den Paketnamen einer Familie durch den einer anderen ersetzen.
  Genau ein Test wird rot. Wird keiner rot, prüft die Tabelle nichts.

### Akzeptanzkriterien
- [ ] `grep -rn 'apt install' daemon/crates/sandbox/src` findet keinen fest verdrahteten Verwalter mehr.
- [ ] Für jede der vier Familien steht der Paketname mit seiner Quelle als Kommentar im Code.
- [ ] Eine Familie ohne belegten Namen erzeugt keinen `CopyCommand`, und ein Test hält das fest.
- [ ] Die Mutationsprobe macht genau einen Test rot.
- [ ] `make check` grün.

### Fallstricke
- Der Paketname der Tray-Bibliothek ist auf Debian und Ubuntu versionsbehaftet
  (`libayatana-appindicator3-1`). Ein Name, der eine Versionsnummer trägt, altert;
  der Kommentar nennt deshalb die Distributionsversion, gegen die er belegt ist.
- Die Prüfung selbst sucht nach der Bibliothek in den lesbaren
  Bibliotheksverzeichnissen, nicht nach dem Paket. Die beiden können
  auseinanderlaufen — ein Nutzer, der die Bibliothek von Hand gelegt hat, bekommt
  den Befund nicht, und das ist richtig so.

## HUM-128 · Ein Stopp, den der Client nicht zu Ende hört, tötet die Sandbox nicht

Sprint: 5 · Größe: M · Abhängigkeiten: HUM-124 · Blockiert: —

### Kontext
`Sandbox(Stop)` ist ein Ereignisstrom, und sein Vollzug hängt daran, dass jemand
zuhört. `daemon/crates/ipc/src/sandbox.rs`, `Inner::stop`:

```rust
if let Ok(Ok(status)) = spawn_blocking(|| snapshot_with(&plan, Some(Stopping))).await
    && tx.send(status_event(status)).await.is_err()
{
    return;
}
let _ = spawn_blocking(move || handle.terminate(KILL_GRACE)).await;
```

Erst die Meldung `Stopping`, dann das Töten. Wer den Strom nach dem ersten
Ereignis fallen lässt, lässt `tx.send` scheitern — und `stop` kehrt **an dieser
Stelle** zurück, vor `handle.terminate`. Der Agent läuft weiter.

Am 2026-09-06 in HUM-124 gemessen, nicht hergeleitet: Drei Testprozesse standen
45, 40 und 29 Minuten mit je zwei lebenden `bwrap`-Kindern, deren
Kommandozeilen die Skripte von `osc52_does_not_reach_host` und
`osc8_and_title_are_inert` trugen. HUM-124 hat das Testgerüst geheilt, indem es
den Strom bis zum Ende liest. Der Weg im Daemon ist unverändert.

**Wie weit das trägt.** `--die-with-parent` steht immer im Argumentbau
(`bwrap_args.rs:273`), also endet die Sandbox spätestens mit dem Daemon. Der
Daemon ist aber ein langlebiger Nutzerdienst; zwischen dem abgebrochenen Stopp
und seinem nächsten Ende arbeitet der Agent weiter. Die heutigen Clients leeren
den Strom (`sandbox_status_provider.dart`, `_apply`, liest bis zum Ende), also
braucht es einen Client, der **während** des RPC verschwindet: ein `Ctrl-C` auf
`humanitl sandbox stop`, ein geschlossenes Fenster, ein Absturz, eine
abreißende Verbindung.

Der Mensch hat dann auf Stopp gedrückt, und es ist nicht gestoppt. Nichts auf
dem Bildschirm sagt es ihm, denn der Bildschirm ist weg.

### Ziel
Ein Stopp ist ein Befehl und kein Abonnement. Wer ihn erteilt hat, hat ihn
erteilt: Der Daemon führt ihn zu Ende, gleich ob noch jemand zuhört.

### Nicht-Ziel
Keine Änderung an der Reihenfolge der Meldungen — `Stopping` vor dem Töten ist
richtig, der Client soll den Übergang sehen. Keine zweite Stopp-Fähigkeit neben
dem RPC. Kein Aufräumen fremder Sandboxen beim Start des Daemons; das ist ein
eigenes Thema.

### Betroffene Pfade
- `daemon/crates/ipc/src/sandbox.rs`: `Inner::stop` und die Stellen, die auf `tx.send(...).is_err()` mit `return` antworten
- `daemon/crates/ipc/tests/sandbox_stop.rs` (neu) oder eine Ergänzung in `sandbox_start.rs`

### Spezifikation
Die Arbeit des Stopps wird von der Meldung getrennt. Das Töten läuft in einer
Aufgabe, die der Daemon besitzt, nicht der RPC-Strom; die Meldungen gehen
weiterhin über `tx`, und ein fehlgeschlagenes `send` beendet **die Meldungen**,
nicht die Arbeit.

Jede Stelle in `Inner::stop`, die heute mit `return` auf einen verlorenen
Empfänger antwortet, wird darauf geprüft: Was danach käme, ist entweder eine
weitere Meldung (dann ist das `return` richtig) oder eine Zustandsänderung
(dann nicht). Dasselbe gilt für `stop_after_failed_check`, das eine Sandbox
tötet, deren Isolationsprüfung fehlschlug — dort wäre ein verlorener Empfänger
noch schwerer zu ertragen.

Die Sitzung gilt als beendet, wenn `terminate` zurückkam, nicht wenn die letzte
Meldung raus ist. `clear_running` bleibt hinter dem Töten.

### Tests
- Ein Test, der den Stopp-Strom **nach dem ersten Ereignis fallen lässt** und
  danach belegt, dass die Sandbox weg ist. Er ist heute rot; das ist der Punkt.
- Ein Test, der zeigt, dass die Meldungen weiterhin in der Reihenfolge
  `Stopping`, Log-Zeile, Endzustand kommen, wenn jemand zuhört.
- Eine Zählung: vor und nach dem Test dieselbe Zahl `bwrap`-Prozesse. Ohne sie
  prüft der erste Test nur, dass ein Schnappschuss „stopped" sagt, und das ist
  eine Aussage über eine Struktur, nicht über einen Prozess.
- Mutationsprobe: das `return` beim fehlgeschlagenen `send` wieder vor das
  Töten ziehen. Der erste Test muss rot werden.

### Akzeptanzkriterien
- [ ] Ein Stopp, dessen Strom nach dem ersten Ereignis fallen gelassen wird, beendet die Sandbox trotzdem; ein Test hält das fest und zählt dabei die `bwrap`-Prozesse.
- [ ] Kein `return` in `Inner::stop` oder `stop_after_failed_check` steht mehr zwischen einer Zusage und ihrer Ausführung; wo eines bleibt, sagt ein Kommentar, dass danach nur noch gemeldet wird.
- [ ] Die Reihenfolge der Meldungen für einen zuhörenden Client ist unverändert.
- [ ] Die Mutationsprobe macht den ersten Test rot.
- [ ] `make check` grün.

### Fallstricke
- `handle.terminate` blockiert; es gehört auf `spawn_blocking` und nicht in die
  Ereignisschleife. Eine Aufgabe, die den Daemon überlebt, gibt es nicht — der
  Daemon wartet beim Herunterfahren auf sie, sonst nimmt `--die-with-parent`
  ihr die Sandbox unter den Händen weg und der Endzustand ist erfunden.
- Zwei Stopps zur selben Zeit dürfen nicht zweimal töten und nicht zweimal
  `clear_running` rufen. Der `running_handle` ist die Stelle, an der das
  entschieden wird.
- `stop_after_failed_check` benutzt `terminate(Duration::ZERO)` mit Absicht:
  Eine Sandbox, deren Isolation nicht hält, bekommt keine Gnadenfrist. Das
  bleibt so.

## HUM-129 · Der Daemon fragt den Agent-Adapter nie, ob der Start gelingen kann

Sprint: 5 · Größe: M · Abhängigkeiten: HUM-037 · Blockiert: —

### Kontext
`AgentAdapter::preflight` liefert die Befunde, die ein Start vorher kennen kann:
`AGENT_001` (der Agent liegt nicht im Pfad), `AGENT_004` (er liegt auf diesem
Rechner, aber nicht unter einem Pfad, den die Sandbox einhängt). Der Weg über
die Kommandozeile fragt danach (`daemon/bin/humanitl/src/cmd/sandbox.rs:663`,
`for diagnostic in adapter.preflight(&agent_ctx)`). Der Weg über den Daemon
fragt nicht: `daemon/crates/ipc/src/sandbox.rs:1944-1946` ruft `adapter.command`,
`adapter.env` und `adapter.files`, und `preflight` steht dort nicht. Auch
`SandboxService::preflight` (`sandbox.rs:765`) prüft nur `bwrap`.

Das trifft den Normalfall, nicht einen Randfall. Der Befund `AGENT_001` schlägt
als Abhilfe den eigenen Installationsbefehl von OpenCode vor
(`daemon/crates/sandbox/src/agent/opencode.rs:150`,
`curl -fsSL https://opencode.ai/install | bash`), und der legt das Programm nach
`$HOME/.opencode/bin`. Das mitgelieferte Profil hängt `$HOME` nicht ein
(`profiles/sandbox/default.toml`, `mounts.ro = ["/usr", "/etc/ssl", …]`), und
`profile.rs:1111-1116` weist jeden `mounts.extra_ro` unter `$HOME` ausdrücklich
ab. Wer also tut, was das Programm ihm sagt, bekommt danach:

```
$ humanitl run
exit 127
humanitl-shim: exec failed: opencode: No such file or directory (os error 2)
```

Am 2026-09-06 mit echtem Daemon gemessen: `grep -c AGENT_004` über die Ausgabe
findet null. Derselbe Zustand über die Kommandozeile gefragt zeigt den Befund
sehr wohl:

```
$ humanitl sandbox argv
blocking[AGENT_004] … fix: sudo install -m 0755 /home/…/.local/bin/opencode /usr/local/bin/opencode
```

Zwei Wege, dieselbe Frage, zwei Antworten. Der Weg, den fast jeder nimmt, gibt
die schlechtere.

HUM-037 hat den Aufruf im eigenen Text vorgesehen — „Integration: `let diags =
adapter.preflight(&ctx); if diags.iter().any(|d| d.severity == Severity::Blocking)
{ return Err(diags); }`" — und die Kästchen dieses Teils sind abgehakt, weil der
CLI-Pfad ihn erfüllt.

### Ziel
Beide Wege geben dieselbe Auskunft. Ein Start, der an etwas scheitern wird, das
vorher zu sehen war, scheitert mit dem Befund, der sagt, was zu tun ist — nicht
mit `exit 127`.

### Nicht-Ziel
Keine Änderung an den Befunden selbst und keine an der Einhänge-Politik. Ob ein
Profil `$HOME` einhängen darf, ist eine eigene Frage (`profile.rs:1111`); dieses
Issue sorgt nur dafür, dass der Mensch erfährt, woran es liegt. Kein zweiter
Preflight-RPC — `Sandbox(Plan)` ist die Stelle, die das schon beantwortet.

### Betroffene Pfade
- `daemon/crates/ipc/src/sandbox.rs`: der Zusammenbau des Starts und `SandboxService::preflight`
- `daemon/crates/ipc/tests/sandbox_start.rs`
- gegebenenfalls `daemon/bin/humanitl/src/cmd/sandbox.rs`, wenn der gemeinsame Aufruf dorthin wandert

### Spezifikation
`preflight` wird auf dem Daemon-Pfad gerufen, bevor `bwrap` startet, mit
demselben `AgentContext`, den `command`, `env` und `files` bekommen — also
inklusive `sandbox_ro_paths`, denn ohne die kann `AGENT_004` nicht entstehen.

Ein blockierender Befund beendet den Start, bevor er beginnt, und geht als
`Diagnostic`-Ereignis hinaus wie jeder andere. Nicht blockierende Befunde gehen
mit und halten nichts auf.

Der Aufruf steht **einmal**. Heute steht die Logik in der CLI und fehlt im
Daemon; nach diesem Issue darf sie nicht an zwei Stellen stehen, sonst laufen
sie wieder auseinander — dieselbe Lehre wie in HUM-122.

`Sandbox(Plan)` beantwortet dieselbe Frage ohne zu starten und muss dieselben
Befunde liefern; heute tut es das bereits über die CLI, und der Test dazu gehört
in denselben Commit.

### Tests
- Ein Test in `sandbox_start.rs`, der `agent.command` auf einen Pfad außerhalb
  der eingehängten Bäume setzt und belegt, dass der Start mit `AGENT_004` endet
  und **nicht** mit einem Exit-Code des Shims. Er ist heute rot.
- Ein Test, dass ein nicht blockierender Befund den Start nicht aufhält.
- Ein Test, dass `Sandbox(Plan)` und `Sandbox(Start)` für denselben Zustand
  dieselbe Menge Befunde liefern. Ohne ihn laufen die beiden Wege wieder
  auseinander.
- Mutationsprobe: den Aufruf wieder entfernen. Der erste Test muss rot werden.

### Akzeptanzkriterien
- [ ] `humanitl run` mit einem Agenten außerhalb der eingehängten Pfade endet mit `AGENT_004` und seinem Vorschlag, gemessen an der Ausgabe, nicht am Code.
- [ ] `grep -rn 'adapter.preflight' daemon/` findet den Aufruf an genau einer Stelle, die beide Wege erreichen.
- [ ] `Sandbox(Plan)` und `Sandbox(Start)` liefern für denselben Zustand dieselben Befunde; ein Test hält das fest.
- [ ] Die Mutationsprobe macht den ersten Test rot.
- [ ] `make check` grün.

### Fallstricke
- `preflight` braucht `sandbox_ro_paths`. Wird es zu früh gerufen — bevor das
  Profil aufgelöst ist —, kann es `AGENT_004` nicht erheben und meldet nur
  `AGENT_001` oder gar nichts. Die Reihenfolge ist Teil der Zusage.
- Der Befund darf den Start nicht doppelt beenden: `kill_and_fail` und der neue
  Weg müssen dieselbe Sitzung nicht zweimal abräumen.
- Ein Agent, der über `agent.command` ausdrücklich benannt wurde, ist etwas
  anderes als einer, der über `PATH` gefunden wurde. Beide Fälle brauchen den
  Befund, aber nur der zweite darf ihn mit `AGENT_001` beantworten.

## HUM-130 · Ein ignoriertes `SIGINT` des Starters erreicht den Agenten und macht seinen Handler wirkungslos
Sprint: 5 · Größe: M · Abhängigkeiten: — · Blockiert: —

### Kontext
`humanitl sandbox run` gibt ein `SIGINT` an die Sandbox weiter, statt sie zu
erschlagen: Der Agent soll selbst aufhören und seinen eigenen Code liefern
(`daemon/bin/humanitl/src/cmd/sandbox.rs`, `wait_or_interrupt`). Die Bitte ist
auf `INTERRUPT_GRACE` befristet, fünf Sekunden; danach folgt der Schlag und der
Lauf endet mit 130.

`execve` setzt einen Signal-**Handler** auf die Vorgabe zurück, behält aber
`SIG_IGN`. Ein ignoriertes `SIGINT` wandert deshalb von dem, der Humanitl
gestartet hat, bis in die Sandbox: aus einem Hintergrundjob einer Shell ohne
Job-Control, aus `nohup`, aus einem Dienst, der mit ignoriertem `SIGINT`
startet. Der Agent kann dafür keinen Handler mehr setzen — POSIX verbietet
einer nicht-interaktiven Shell, ein beim Start ignoriertes Signal zu trappen —,
und die höfliche Bitte trifft einen Prozess, der die Frage nicht hören kann.

Gemessen am 2026-09-06: `sh -c 'trap "exit 42" INT; : > /work/ready; sleep 60'`
in der Sandbox überlebte das `SIGINT` vollständig; 300 ms danach standen
`bwrap`, Shim, `sh` und `sleep` unverändert in `ps`, nach fünf Sekunden folgte
`SIGTERM`, und der Lauf endete mit 130 statt mit 42. Ohne unseren Code
nachgestellt: dasselbe `bwrap`-Kommando aus einem Hintergrundjob heraus, und
`kill -INT -<pgid>` bewegt nichts. In `tools/verify-commit.sh` und in
`make check` — beide aus einem Kontext gestartet, der `SIGINT` ignoriert — war
`sigint_reaches_the_agent_and_keeps_its_exit_code` deshalb acht von acht Läufen
rot, während er allein im Terminal in 50 ms grün lief.

Ein zweites, kleineres Loch derselben Stelle: Die Quelle für `SIGINT` wurde
erst im `select!` angemeldet, also **nach** dem Start der Sandbox. Zwischen
Start und erstem Warten verschwand ein `SIGINT` spurlos; der Mensch drückt
Strg+C, nichts passiert, und der Agent läuft weiter.

### Ziel
Der Agent startet mit den Vorgaben für alle Signale, unabhängig davon, wie
Humanitl gestartet wurde. Die Quelle für `SIGINT` steht, bevor die Sandbox
läuft.

### Nicht-Ziel
Keine längere Frist. Fünf Sekunden reichen einem Handler; die Frist war nie das
Problem. Keine Weitergabe von `SIGINT` durch den Shim — sie bleibt aus dem
Grund aus, der in `forward_signals` steht (der Agent bekäme es zweimal).

### Betroffene Pfade
- `daemon/bin/humanitl-shim/src/main.rs` (`child`, `reset_signal_dispositions`)
- `daemon/bin/humanitl/src/cmd/sandbox.rs` (`start`, `wait_or_interrupt`)
- `daemon/bin/humanitl/tests/cli.rs`

### Spezifikation
Das Zurücksetzen gehört in das Init der Sandbox, nicht in den Starter: Der Shim
setzt im Kind nach `reset_signal_mask` jede Disposition auf `SIG_DFL`, außer
für `SIGKILL` und `SIGSTOP`, die sich nicht setzen lassen. Das deckt auch den
Weg über den Daemon ab, der Sandboxen ohne die Kommandozeile startet.

Die Kommandozeile meldet ihre `SIGINT`-Quelle vor dem Start der Sandbox an
(`tokio::signal::unix::signal(SignalKind::interrupt())` statt
`tokio::signal::ctrl_c()` im `select!`). Das schließt das Fenster und hebt
nebenbei ein geerbtes `SIG_IGN` im eigenen Prozess auf.

Beide Schichten bleiben, obwohl jede für sich genügt (gemessen, siehe unten).
Der Shim ist die Stelle, die die Zusage für **jeden** Weg in die Sandbox hält;
die frühe Anmeldung ist die Stelle, die das Fenster schließt. Wer eine davon
entfernt, nimmt eine Zusage weg, die die andere nicht gibt.

### Tests
- `an_ignored_sigint_of_the_launcher_does_not_reach_the_agent`: Der Lauf startet
  über `sh -c "trap '' INT; exec …"`, also mit ignoriertem `SIGINT`, und der
  Agent endet trotzdem mit 42 — in weniger als vier Sekunden, also vor der
  Eskalation. Zusätzlich liest der Test die Ignoriermaske des Agenten aus
  `/proc/self/status` und verlangt, dass `SIGINT` nicht darin steht.
- Mutationsprobe: Mit beiden Schichten zurückgebaut ist der Test rot
  (5,15 s, `left: Some(130)`), mit je einer von beiden grün.
- Der vorhandene `sigint_reaches_the_agent_and_keeps_its_exit_code` bleibt und
  läuft unter Last aus demselben Kontext.

### Akzeptanzkriterien
- [x] Ein ignoriertes `SIGINT` des Starters erreicht den Agenten nicht; seine
      Ignoriermaske ist frei davon (`an_ignored_sigint_of_the_launcher_does_not_reach_the_agent`
      liest `SigIgn` aus `/proc/self/status` des Agenten und verlangt Bit 1 frei).
- [x] Der Agent beantwortet das `SIGINT` und behält seinen Exit-Code, auch wenn
      Humanitl aus einem Hintergrundjob gestartet wurde: derselbe Test verlangt
      42 in unter vier Sekunden, und zehn Läufe der Suite unter Last aus genau
      diesem Kontext sind grün, wo vorher acht von acht rot waren.
- [x] Die `SIGINT`-Quelle der Kommandozeile steht vor dem Start der Sandbox
      (`cmd/sandbox.rs`, `signal(SignalKind::interrupt())` vor `backend.launch`).
- [x] Mutationsprobe für beide Schichten dokumentiert, siehe Stand unten.
- [x] `make check` mit `STRICT=1` grün, also einschließlich clippy mit
      `-D warnings` und `cargo fmt --all -- --check` (2026-09-06).

### Fallstricke
- Die beiden Schichten decken einander zu: Eine Mutation in einer allein bleibt
  grün. Die Probe muss beide zugleich zurückbauen, sonst misst sie nichts.
- `reset_signal_dispositions` läuft im Kind **nach** dem Fork und
  **unmittelbar vor** `execvp`, nicht früher. `SIGPIPE` gehört zu den
  Dispositionen, die es zurücksetzt, und genau dessen geerbtes `SIG_IGN` (Rust
  setzt es beim Start) hält die Berichtsschreibungen des Kindes am Leben, wenn
  der Starter nicht mehr liest. Eine frühere Stelle tötet das Kind beim ersten
  Schreiben in eine geschlossene Pipe; im Review von Codex ist genau das
  aufgefallen.
- Die Echtzeit-Signale gehören dazu. Ein ignoriertes `SIGRTMIN+n` fiele sonst
  genauso still unter den Tisch wie `SIGINT`.


### Stand (2026-09-06)

**Die Mutationsprobe braucht beide Schichten zugleich.** Gemessen in vier
Läufen desselben Tests:

| Shim setzt zurück | Kommandozeile meldet früh an | `an_ignored_sigint_…` |
|---|---|---|
| nein | nein | **rot**, 5,15 s, `left: Some(130)` |
| nein | ja | grün, 0,14 s |
| ja | nein | grün, 0,14 s |
| ja | ja | grün, 0,14 s |

Jede Schicht allein genügt also, und darin liegt die Grenze dieses Tests: Er
deckt die Zusage ab, nicht ihre beiden Quellen. Wer nur eine Schicht entfernt,
bleibt grün. Beide bleiben trotzdem, weil sie verschiedene Zusagen halten — der
Shim für **jeden** Weg in die Sandbox, auch den über den Daemon, der die
Kommandozeile nicht durchläuft; die frühe Anmeldung für das Fenster zwischen
Start und erstem Warten, das der Shim nicht schließen kann. Ein Test, der den
Shim allein misst, müsste ihn ohne `bwrap` und ohne Brücken starten; das ist
ein eigener Zuschnitt und steht hier als bewusst offene Stelle, nicht als
Versehen.

**Die Reihenfolge im Kind ist die eigentliche Feinheit, und sie ist im Review
aufgefallen.** Der erste Entwurf setzte die Dispositionen direkt nach
`reset_signal_mask` zurück, also **vor** den Berichtszeilen des Kindes. Damit
war `SIGPIPE` wieder auf der Vorgabe, und ein Starter, der die Berichts-Pipe
nicht mehr liest, hätte das Kind beim nächsten `report.check` getötet — vor
`execvp`, also mit einer Sandbox, die nie startet. Der vorhandene Kommentar an
der alten `SIGPIPE`-Zeile sagte das seit HUM-012 („Last, so the report writes
above cannot kill the child"); der Entwurf hat ihn übersehen. Codex und
Antigravity haben unabhängig voneinander dieselbe Stelle gemeldet. Der Aufruf
steht jetzt genau dort, wo vorher das einzelne `SIGPIPE` zurückgesetzt wurde:
als letzte Zeile vor `exec`.

**Was der Befund über die Ursache hinaus zeigt.** Das Signal kam an: Die
Prozessgruppe stimmte, `kill(-pgid, SIGINT)` lieferte keinen Fehler, und
trotzdem lief `sleep 60` weiter. Die Erklärung steckt nicht im Signalweg,
sondern in der Disposition — und die ist von außen nicht sichtbar, solange man
nur auf `ps` schaut. Der Weg dahin führte über `/proc/<pid>/status`, und der
gehört bei jedem „das Signal kommt nicht an" als zweiter Blick dazu.

## HUM-131 · Die Aufzeichnung kennt ein Alter, aber keine Menge
Sprint: 5 · Größe: M · Abhängigkeiten: — · Blockiert: —

### Kontext
Humanitl schreibt in das Zuhause des Menschen: die Datenbank
(`$XDG_DATA_HOME/humanitl/humanitl.db`), den Blob-Speicher daneben und das
Audit-Protokoll. Begrenzt ist heute jede einzelne Zeile, aber nicht die Summe:

- `limits.recorder_max_body_bytes` (Vorgabe 33 554 432) begrenzt **einen**
  Body; alles darüber wird nur mit Prüfsumme vermerkt.
- `recorder.inline_max_bytes` (Vorgabe 262 144) entscheidet nur, ob ein Body in
  der Datenbank oder als Datei liegt — nicht, ob er überhaupt liegt.
- `recorder.retention_days` (Vorgabe 90) räumt nach Alter auf, einmal beim
  Start und danach täglich (`humanitld/src/main.rs`, `purge_daily`).

Ein Alter begrenzt keine Menge. Ein Agent, der einen Tag lang große Antworten
zieht, füllt bis zu 90 Tage lang, bevor überhaupt etwas gelöscht wird; ein
zweiter Agent daneben verdoppelt das. Es gibt keine Obergrenze in Bytes, keine
Verdrängung des Ältesten bei Überschreitung und keine Untergrenze für den
freien Platz, bei der Humanitl aufhört, Rümpfe zu schreiben, statt die Platte
des Menschen zu füllen. `journald` (`SystemMaxUse`, `SystemKeepFree`) und
`docker` (`max-size`, `max-file`) führen beides seit Jahren; ein Werkzeug, das
im Hintergrund mitschreibt, braucht es.

### Ziel
Die Aufzeichnung hat eine Mengengrenze und eine Untergrenze für den freien
Platz. Beide sind Einstellungen mit Vorgaben, beide sind gemessen, und beim
Erreichen der Grenze passiert etwas Benanntes statt etwas Stillem.

### Nicht-Ziel
Keine Kompression in diesem Issue (eigener Schnitt, eigene Messung). Keine
Änderung an dem, was aufgezeichnet wird — die Auswahl ist ADR-008.

### Betroffene Pfade
- `daemon/crates/recorder/src/` (Grenze, Verdrängung, Bericht)
- `daemon/crates/config/src/model.rs` (`recorder.max_total_bytes`,
  `recorder.keep_free_bytes`), `docs/CONFIG.md` (erzeugt)
- `daemon/bin/humanitld/src/main.rs` (die tägliche Aufgabe)
- `daemon/crates/core-types/src/diagnostics/codes.rs` (ein Befund für den Fall)

### Spezifikation
Zwei Zahlen, beide in `recorder`:

- `max_total_bytes`: die Summe aus Datenbank und Blob-Speicher. Über der Grenze
  wird das Älteste verdrängt, in derselben Reihenfolge wie die Aufbewahrung es
  tut, bis wieder Platz ist. Vorgabe: 2 GiB.
- `keep_free_bytes`: der freie Platz des Dateisystems, unter den Humanitl nicht
  drückt. Darunter werden **Rümpfe** nicht mehr abgelegt (Kopfzeilen, Regel,
  Entscheidung und Prüfsumme bleiben, das ist die Zusage aus ADR-008), und ein
  Befund sagt es einmal je Sitzung statt in jeder Zeile. Vorgabe: 1 GiB.

Gemessen wird die Summe, nicht geschätzt: `page_count * page_size` der
Datenbank plus die Größe des Blob-Verzeichnisses, gecacht und bei jedem
Purge-Lauf neu erhoben.

### Tests
- Eine Aufzeichnung über `max_total_bytes` verliert die ältesten Flows, bis sie
  darunter liegt; die jüngsten und alle gehaltenen bleiben.
- Unter `keep_free_bytes` wird ein Body nicht mehr abgelegt, der Flow aber
  weiterhin vollständig verzeichnet, und der Befund steht genau einmal.
- Mutationsprobe: Wer die Grenze auf `u64::MAX` setzt, macht den ersten Test
  rot.
- Eine Messung im Commit-Body: wie viele Bytes eine Sitzung mit einem
  Standard-Prompt tatsächlich schreibt.

### Akzeptanzkriterien
- [ ] `recorder.max_total_bytes` und `recorder.keep_free_bytes` stehen im
      Schema, in `docs/CONFIG.md` und im Leser-Register als `effective`.
- [ ] Die Summe wird gemessen, die Verdrängung nimmt das Älteste zuerst.
- [ ] Unter der Untergrenze bleibt die Aufzeichnung vollständig bis auf die
      Rümpfe, und der Mensch erfährt es einmal.
- [ ] Die gemessene Größe einer Sitzung steht im Commit-Body.
- [ ] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- `VACUUM` gibt den Platz einer SQLite-Datenbank erst zurück, wenn er läuft;
  ohne ihn schrumpft die Datei nach dem Löschen nicht, und die Messung sähe
  eine Grenze, die längst unterschritten ist.
- Die Verdrängung darf keinen gehaltenen Fluss löschen, über den gerade
  entschieden wird.
- Ein Blob hängt an mehreren Zeilen (Deduplizierung über sha256). Er fällt erst,
  wenn die letzte Zeile fällt — sonst zeigt eine Aufzeichnung auf eine Datei,
  die es nicht mehr gibt.

---

## HUM-132 · Der Entwicklungsbaum schreibt die Platte voll
Sprint: 5 · Größe: S · Abhängigkeiten: — · Blockiert: —

### Kontext
Am 2026-09-06 lag `daemon/target` bei 103 GiB: 89 GiB `debug/deps`, 12 GiB
`debug/incremental`, 2 GiB `debug/build`. Ein `cargo clean` gab 107,9 GiB frei
(83 159 Dateien). Das ist kein Ausreißer, sondern der Normalfall dieses
Repositories: Jede Änderung an einer Abhängigkeit oder an einem Feature-Satz
legt einen neuen Satz Artefakte an, und Cargo räumt alte nie von selbst weg.

Zwei Zahlen dazu, am selben Tag gemessen, beide nach `cargo clean` und mit
`cargo build --workspace --all-targets` auf demselben Stand:

- mit `debug = 2`, der Vorgabe des Dev-Profils: **13 GiB**
- mit `debug = "line-tables-only"`: **6,1 GiB**

Also 53 Prozent weniger für einen sauberen Baum, und derselbe Anteil für jeden
Satz, der später danebenliegt. Ein Backtrace behält dabei Datei und Zeile; was
fehlt, sind Variablenwerte im Debugger.

### Ziel
Ein Entwicklungsbaum dieses Repositories wächst langsamer, und wer ihn
aufräumen will, findet den Weg dokumentiert statt in einer Sitzung.

### Nicht-Ziel
Keine Änderung am Release-Profil und keine am `shim`-Profil: Der Shim wird mit
`strip = "symbols"` gebaut und ist davon nicht berührt. Kein automatisches
Löschen im Hintergrund — was ein Werkzeug ungefragt löscht, fehlt genau dann,
wenn jemand es gerade braucht.

### Betroffene Pfade
- `daemon/Cargo.toml` (`[profile.dev]`)
- `CONTRIBUTING.md` (der Absatz über den Platz, den ein Baum braucht)

### Spezifikation
`[profile.dev] debug = "line-tables-only"` im Workspace. Die Begründung samt
den beiden gemessenen Zahlen steht als Kommentar darüber, damit niemand sie
später als Schätzung liest.

`CONTRIBUTING.md` nennt die Größenordnung eines Baums (rund 6 GiB frisch, mehr
mit jedem Wechsel von Abhängigkeiten) und den Befehl, der ihn zurücksetzt.

### Tests
Eine Messung statt eines Tests: `cargo clean` gefolgt von
`cargo build --workspace --all-targets`, danach `du -sh daemon/target`, mit und
ohne den Schlüssel. Beide Zahlen in den Commit-Body.

### Akzeptanzkriterien
- [x] `[profile.dev] debug = "line-tables-only"` steht im Workspace, mit den
      beiden gemessenen Zahlen als Begründung darüber (`daemon/Cargo.toml`).
- [x] `make check` mit `STRICT=1` grün (2026-09-06), und ein Backtrace aus einem
      fehlschlagenden Test zeigt weiterhin Datei und Zeile. Gemessen mit einer
      Probe: eine Zusicherung in `a_resize_reaches_the_agent` auf eine Größe
      gestellt, die nie kommt, ergibt
      `panicked at crates/ipc/tests/terminal.rs:704:9` und mit
      `RUST_BACKTRACE=1` den Rahmen `at ./tests/terminal.rs:704:9`.
- [x] `CONTRIBUTING.md` nennt die Größenordnung und den Aufräumbefehl
      (Abschnitt „Disk").
- [x] Der Commit-Body trägt beide Messungen.

### Fallstricke
- `line-tables-only` ist kein `debug = false`: Wer im Debugger Variablen
  ansehen will, baut mit `RUSTFLAGS="-C debuginfo=2"` oder setzt den Schlüssel
  lokal zurück. Das gehört in den Kommentar.
- Der Wert gilt für `profile.dev` und damit auch für `cargo test`; die
  Zeilennummern in Panics kommen daher, nicht aus voller Debug-Information.

---

## HUM-133 · Ein roter Testlauf in CI nennt seinen Test nicht
Sprint: 5 · Größe: S · Abhängigkeiten: — · Blockiert: —

### Kontext
Am 2026-09-06 sind drei Läufe des Jobs `rust-test` auf `main` rot geworden
(`3f293f2`, `5ee92ba`, `9bc3717`), und in allen drei Fällen steht in der
Annotation genau ein Satz: `Process completed with exit code 2`. Mehr ist von
außen nicht zu sehen: Die Job-Logs verlangen Schreibrechte am Repository
(`GET /actions/jobs/<id>/logs` antwortet 403), und die Annotationen tragen nur
die letzte Zeile des Schritts. Der Schritt ist `make rust-build rust-test`,
also `cargo test --workspace` über rund 900 Tests; welcher davon fiel, steht
nirgends.

Der Lauf davor und der danach waren grün, und lokal ist der Fehlschlag nicht zu
erzeugen: dreimal `cargo test --workspace` auf zwei Kernen (`taskset -c 0,1`,
`CI=1`), einmal davon mit den installierten Clients der Konformitäts-Matrix
(`websocat`, `grpcurl`, `python3-requests`) — alles grün. Ein Flake also, und
einer, den niemand einkreisen kann, solange die Ausgabe nicht aus dem Lauf
herauskommt.

Das ist mehr als eine Unbequemlichkeit: `BACKLOG.md` 8 macht die grüne
Pipeline zur Sprint-Bedingung, und eine rote Pipeline, deren Grund niemand
lesen kann, ist eine Bedingung, die niemand erfüllen kann.

### Ziel
Ein roter `rust-test`-Lauf nennt in der Annotation die Namen der Tests, die
fielen, und legt seine vollständige Ausgabe als Artefakt ab.

### Nicht-Ziel
Keine Jagd auf den Flake selbst in diesem Issue — erst muss der Lauf sagen
können, wer fiel. Kein Wechsel des Testläufers (`cargo-nextest` wäre eine
eigene Entscheidung mit eigener Abwägung). Keine Wiederholung fehlgeschlagener
Tests: Ein Test, der beim zweiten Mal grün ist, hat trotzdem etwas gefunden.

### Betroffene Pfade
- `.github/workflows/ci.yml` (Schritt „Build and test the workspace")
- `scripts/ci/test-report.sh` (neu): liest die Ausgabe von `cargo test` und
  schreibt je gefallenem Test eine `::error::`-Zeile
- `CONTRIBUTING.md` (wo die Ausgabe eines roten Laufs zu finden ist)

### Spezifikation
Der Schritt leitet die Ausgabe durch `tee` in eine Datei und wertet sie bei
einem Fehlschlag aus. Für jeden Namen unter `failures:` entsteht eine Zeile
`::error::rust-test: <name>`; die Datei wird als Artefakt `rust-test-log`
hochgeladen (`actions/upload-artifact`, `if: failure()`), damit auch die
Panik-Meldung selbst erreichbar bleibt.

Die Namen stehen in der Ausgabe zweimal: einmal je Testbinärdatei unter
`failures:` mit Einrückung, einmal als `---- <name> stdout ----` mit der
Meldung darüber. Ausgewertet wird der erste Block; der zweite wandert
ungekürzt ins Artefakt.

### Tests
- `scripts/ci/test-report.sh --self-test` mit einer eingebetteten Beispiel-
  Ausgabe: zwei gefallene Tests ergeben zwei `::error::`-Zeilen mit genau
  diesen Namen, eine grüne Ausgabe ergibt keine.
- Ein absichtlich roter Testlauf in einem Zweig belegt die Annotation. Der
  Nachweis steht im Commit-Body, der Zweig bleibt nicht.

### Akzeptanzkriterien
- [x] `scripts/ci/test-report.sh --self-test` ist grün, Teil von `make check`
      (Ziel `typed-errors-lint`) und Teil des CI-Schritts `rust-check`:
      einundzwanzig Fälle — zwei gefallene Tests (der Name steht genau einmal, obwohl das
      Protokoll ihn zweimal trägt); eine Zeile eines Kindprozesses mitten in
      der Namensliste (`bwrap: setting up uid map: Permission denied`), nach
      der alle drei Namen weiterhin genannt werden; Lärm, der aussieht wie eine
      Marke des Testläufers (`error:`, `warning:`), der ebenfalls keinen Namen
      kostet; eine Panik-Meldung, die selbst `failures:` in Spalte 0 trägt und
      trotzdem nur den einen echten Namen ergibt; drei abgeschnittene Läufe —
      mitten im Namen, mitten in der Zeile eines Kindprozesses und nach einem
      ganzen Namen mit der Einrückung des nächsten —, bei denen jeder ganze
      Name bleibt und kein Bruchstück einer wird; zwölf gefallene Tests (zehn
      Zeilen, die zehnte sammelt die drei übrigen, weil GitHub die elfte nicht
      mehr zeigt); ein Panik-Bericht, den `libtest` nicht eingefangen hat, mit
      eingerückten Rahmen mitten in der Namensliste (zwei Namen, keine drei
      erfundenen dazu); ein verirrtes `failures:` vor einer grünen Binärdatei,
      dessen eingerückte Statuszeilen von Cargo keine Namen werden; drei
      gefallene Doku-Tests, deren Namen Leerzeichen tragen — mit Pfad, mit
      Generics und ohne Pfad (`src/lib.rs - (line 3)`, wie ein Doku-Test in den
      `//!`-Zeilen einer Datei heißt; diese Crates haben solche); ein grüner Lauf; und
      eine fehlende Datei. **Dazu seit dem 2026-09-07 der Grund**, und die
      sieben Fälle, die ihn halten: eine Panik mit Rahmen und Hinweis (genommen
      wird die Zeile der Zusicherung, nicht der `note:`-Hinweis und kein
      Rahmen); ein Test, der vor seiner Panik selbst Striche ausgibt und
      trotzdem seinen Grund behält; ein Test, dessen eigene Ausgabe wie eine
      Panik aussieht (der Grund ist seine Panik, nicht sein Zitat); ein
      `assert_eq!` mit seinen Werten darunter, die zum Grund gehören; zwei
      Paniken in einem Block, von denen die erste gilt; eine Panik ohne
      Meldung vor dem Kopf des nächsten Tests und eine vor einem Backtrace, die
      beide nur ihren Ort behalten; und eine sehr lange Meldung, die gekürzt
      wird und das sagt.
- [x] Der Auswerter nennt zu jedem gefallenen Test auch den Grund: den Ort aus
      der `panicked at`-Zeile und die Meldung der Zusicherung darunter, mit den
      Werten eines `assert_eq!`. **Gemessen am 2026-09-07** an einem Protokoll
      der Form, die zwei rote CI-Läufe desselben Tages hinterlassen haben
      (`rust-test failed: one_writer_many_readers` und sonst nichts): Die Zeile
      lautet jetzt `rust-test failed: one_writer_many_readers --
      crates/ipc/tests/terminal.rs:795:5: the reader gets the scrollback: ""`.
      Sieben Mutationsproben, je eine Regel: unverankerte Suche, fehlende
      Grenze an `stack backtrace`, an `----`, an einer zweiten Panik, „die
      letzte Panik gewinnt", keine Kürzung und keine Werte — jede rot.
- [x] Der Auswerter nennt die Tests eines **echten** roten Laufs. Gemessen am
      2026-09-06: Eine Zusicherung in `every_target_builds_what_its_line_promises`
      auf einen Wert gestellt, den niemand baut, ergab
      `::error::rust-test failed: cmd::moderate::tests::every_target_builds_what_its_line_promises`
      — genau einmal, obwohl `cargo test` den Namen zweimal schreibt. **Was
      hier nicht gemessen ist:** die Annotation an einem Lauf von GitHub
      selbst. Dieser Baum kann keinen roten Lauf erzeugen, ohne `main` rot zu
      machen (die CI läuft auf `push` nach `main` und auf `pull_request`, und
      es gibt kein `gh` in dieser Umgebung, um einen PR zu öffnen); der
      Nachweis fällt beim nächsten roten Lauf an, und das Kästchen darunter
      bleibt bis dahin offen.
- [ ] Die Annotation ist an einem Lauf von GitHub gesehen (Lauf-Id im
      Nachtrag), und die vollständige Ausgabe liegt dort als Artefakt
      `rust-test-log`.
- [x] `CONTRIBUTING.md` sagt, wo die Ausgabe eines roten Laufs zu finden ist
      (Abschnitt „When CI is red").

### Fallstricke
- `set -euo pipefail` und `tee`: Ohne `pipefail` verschluckt die Pipe den
  Exit-Code von `cargo test`, und der Schritt wäre grün.
- `cargo test` schreibt die Namen der gefallenen Tests nach `stdout`, die
  Panik-Meldungen nach `stderr`; beide gehören in dieselbe Datei.
- Eine Annotation je Test, nicht eine je Zeile: GitHub zeigt höchstens zehn
  Annotationen je Schritt an.

---

## HUM-146 · Private Ziele hinter NAT64, 6to4 und Sonderbereichen
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-004 · Blockiert: nichts; es macht einen Satz in `docs/SECURITY.md` in jedem Netz wahr

### Kontext
`docs/SECURITY.md` sagt: Löst ein Name auf RFC-1918, `127/8`, `169.254/16` (einschließlich `169.254.169.254`, der Cloud-Metadaten-Adresse), `100.64/10`, `fc00::/7` oder `::1` auf, wird die Verbindung verweigert. `ip_is_private` in `daemon/crates/core-types/src/host.rs:192-225` prüft genau diese Bereiche, entpackt IPv4-mapped und IPv4-compatible Adressen und nimmt `0/8` und `fe80::/10` dazu.

Nicht erfasst sind Adressen, die eine IPv4-Adresse in sich tragen, und einige Sonderbereiche (gefunden in der Proxy-Analyse am 2026-09-11, gegen den Code nachgelesen):

- `64:ff9b::/96` (NAT64, RFC 6052) und `64:ff9b:1::/48` (RFC 8215). In einem Netz mit DNS64 und NAT64 erreicht `64:ff9b::a9fe:a9fe` die Adresse `169.254.169.254`. Dort ist der Satz in `SECURITY.md` falsch.
- `2002::/16` (6to4, RFC 3056): Die Bits 16 bis 48 sind eine IPv4-Adresse.
- `2001::/32` (Teredo, RFC 4380): Die letzten 32 Bit sind die invertierte IPv4-Adresse des Clients.
- IPv4-Sonderbereiche aus der IANA-Special-Purpose-Tabelle: `192.0.0.0/24`, `198.18.0.0/15`, `240.0.0.0/4`; IPv6 `fec0::/10` (veraltetes Site-Local).
- Im Review ergänzt: IPv4-translated `::ffff:0:0/96` (RFC 2765) und ISATAP-Kennungen `…:0:5efe:a.b.c.d` und `…:200:5efe:a.b.c.d` unter beliebigem Präfix (RFC 5214) tragen die IPv4-Adresse ebenfalls in den letzten 32 Bit. 6rd (RFC 5969) hat kein festes Präfix und bleibt unerkannt; `SECURITY.md` sagt das.

Ob einer dieser Wege in einem echten Netz des Nutzers erreichbar ist, ist nicht gemessen. Die Aussage in `SECURITY.md` muss aber ohne diese Einschränkung stimmen.

### Ziel
`ip_is_private` liefert für jede Adresse `true`, die in einem der genannten Bereiche liegt oder eine private IPv4-Adresse über NAT64, 6to4 oder Teredo in sich trägt. `docs/SECURITY.md` nennt die Bereiche vollständig.

### Nicht-Ziel
Eine eigene Tabelle aller IANA-Bereiche mit Aktualisierungsdienst. Ein neues Crate. Die Regel `allow_private: true` ändert sich nicht.

### Betroffene Pfade
- `daemon/crates/core-types/src/host.rs` (`ipv4_is_private`, `ipv6_is_private`)
- Tests daneben
- `docs/SECURITY.md` (Abschnitt „Private Bereiche sind gesperrt")

### Spezifikation
NAT64 (`64:ff9b::/96`) und 6to4 (`2002::/16`) werden entpackt und die IPv4-Adresse mit `ipv4_is_private` geprüft. Teredo und `64:ff9b:1::/48` gelten als privat, weil sich die eingebettete Adresse dort nicht eindeutig lesen lässt. Die IPv4- und IPv6-Sonderbereiche kommen in die beiden Funktionen. Jeder Bereich trägt einen Kommentar mit seiner RFC.

### Tests
Je Bereich eine Adresse innen und eine knapp außerhalb. NAT64 und 6to4 je mit einer privaten (`10.0.0.1`, `169.254.169.254`) und einer öffentlichen eingebetteten Adresse. Mutationsprobe: jede neue Zeile einzeln entfernen, ein Test wird rot.

### Akzeptanzkriterien
- [x] `ip_is_private("64:ff9b::a9fe:a9fe")` und `ip_is_private("2002:a9fe:a9fe::1")` sind `true`, `64:ff9b::808:808` ist `false`. **Gemessen am 2026-09-11** (`is_private_covers_special_ranges_and_embedded_ipv4`).
- [x] `198.18.0.1`, `192.0.0.1`, `240.0.0.1`, `fec0::1` und eine Teredo-Adresse sind `true`. **Gemessen am 2026-09-11**; zehn Mutationen der neuen Zeilen (jede Zeile entfernt, zwei Masken zu weit) machen den Test jeweils rot.
- [x] `docs/SECURITY.md` nennt die Bereiche. **Gemessen am 2026-09-11.**
- [ ] `make check` grün.

### Fallstricke
- `Ipv6Addr::to_ipv4` entpackt NAT64 nicht; das Entpacken ist eigener Code und gehört getestet.
- Ein öffentliches Ziel darf nicht versehentlich privat werden: `64:ff9b::808:808` (`8.8.8.8`) bleibt erreichbar.

### Referenzen
Proxy-Analyse 2026-09-11; RFC 6052, RFC 8215, RFC 3056, RFC 4380; IANA IPv4 und IPv6 Special-Purpose Address Registry; `daemon/crates/core-types/src/host.rs:192-225`; `docs/SECURITY.md` Abschnitt 6 (Regeln und ihre Fallen).

---

## HUM-147 · verify-commit baut inkrementell, die CI nicht
Sprint: 5 · Größe: S · Abhängigkeiten: — · Blockiert: nichts; es hält die Platte des Entwicklungsrechners klein

### Kontext
Die CI setzt `CARGO_INCREMENTAL: "0"` (`.github/workflows/ci.yml:43`). `tools/verify-commit.sh` fährt dieselben Schritte in einem frischen Worktree, gegen ein gemeinsames Zielverzeichnis (`~/.cache/humanitl/verify-target`, Zeile 59), aber inkrementell. Gemessen am 2026-09-11: Das Zielverzeichnis belegt 62 GB, davon 30 GB in `debug/incremental` mit 830 Einträgen und 32 GB in `debug/deps`. Die Inkrement-Artefakte wachsen mit jedem geprüften Commit und helfen kaum, weil jeder Lauf in einem neuen Worktree beginnt. Zugleich weicht verify dadurch von der CI ab, deren Stand es nachstellen soll.

### Ziel
verify-commit baut wie die CI nicht inkrementell, und ein vorhandenes `incremental`-Verzeichnis im Zielverzeichnis verschwindet beim nächsten Lauf.

### Nicht-Ziel
Das gemeinsame Zielverzeichnis abschaffen: `deps` spart jedem Lauf das Übersetzen aller Abhängigkeiten. Eine Obergrenze für `deps` (eigenes Issue, falls es wächst).

### Betroffene Pfade
- `tools/verify-commit.sh`
- `CONTRIBUTING.md` (ein Satz zum Zielverzeichnis, falls dort beschrieben)

### Spezifikation
Die Schritte laufen mit `CARGO_INCREMENTAL=0`, gesetzt an derselben Stelle wie `CARGO_TARGET_DIR` (Zeile 220). Vor dem ersten Schritt löscht das Skript `"$target"/*/incremental`, solange es die Sperre des Zielverzeichnisses hält; das ist Cache und wird mit `CARGO_INCREMENTAL=0` nie wieder gelesen.

### Akzeptanzkriterien
- [ ] `grep -n "CARGO_INCREMENTAL=0" tools/verify-commit.sh` trifft an der Stelle, an der die Schritte laufen.
- [ ] Nach einem Lauf gibt es unter `~/.cache/humanitl/verify-target` kein `incremental`-Verzeichnis mehr; die Größe vorher und nachher steht im Commit-Body.
- [ ] `tools/verify-commit.sh HEAD` ist grün.

### Fallstricke
- Das Skript darf nicht geändert werden, während ein verify läuft: bash liest ein Skript beim Ausführen nach.
- Gelöscht wird nur unter dem gehaltenen Lock, sonst nimmt man einem parallelen Lauf den Cache unter den Füßen weg.

### Referenzen
Messung am 2026-09-11; `.github/workflows/ci.yml:43`; `tools/verify-commit.sh:59`, `:220`; Nutzervorgabe zur Plattensparsamkeit.
