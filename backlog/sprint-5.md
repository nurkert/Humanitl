# Sprint 5 · MVP 0.1 (M5)

Ziel des Sprints: Das System aus Sprint 0 bis 4 wird gehärtet, dokumentiert und als Version 0.1.0 veröffentlicht. Es kommt keine neue Funktion hinzu. Jedes Issue macht Bestehendes robuster, sichtbarer oder verteilbar. Am Ende steht eine manuelle Abnahme, die ein Mensch in 60 Minuten durchgeht.

Voraussetzung: Demo-Skripte M1 bis M4 (HUM-021, HUM-036, HUM-046, HUM-055) sind in CI grün. Wenn eines rot ist, wird es zuerst repariert, bevor ein Issue aus diesem Sprint begonnen wird.

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-056 | Fuzzing | M | HUM-015, HUM-022, HUM-032, HUM-050 |
| HUM-057 | Ressourcen-Limits und Backpressure | S | HUM-015, HUM-016, HUM-018, HUM-026, HUM-062 |
| HUM-058 | Fehlerpfade im UI | M | HUM-063, HUM-019, HUM-040, HUM-041, HUM-042, HUM-045, HUM-068 |
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
