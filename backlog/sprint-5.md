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
| HUM-152 | Der M3-Lauf tippt nicht ins TUI des echten Agenten | S | HUM-141 |
| HUM-155 | Der Audit-Schreiber lebt im Kern | M | HUM-050 |
| HUM-220 | Das Terminal der Oberfläche zeigt OpenCode nicht sauber an | L | HUM-042, HUM-067, HUM-136 |
| HUM-059 | Dokumentation | M | alle vorherigen |
| HUM-086 | Repository auf Englisch | M | HUM-059 |
| HUM-256 | Befunde eines Flusses überleben den Neustart des Daemons | S | HUM-026, HUM-057, HUM-250 |
| HUM-060 | Release 0.1.0 | S | HUM-053, HUM-059 |
| HUM-061 | Puffer | L | keine |
| HUM-247 | Der Findings-Scan liest nur das erste gzip-Member | S | HUM-025 |
| HUM-248 | Nächtliche Fuzz-Funde selbst minimieren und den Corpus behalten | S | HUM-056 |
| HUM-250 | Ein Ausfall der Aufzeichnung bleibt in der Oberfläche unsichtbar (vor HUM-060) | M | HUM-026, HUM-058 |
| HUM-251 | Ein unerreichbares Modell färbt die Zeile „LLM“ nicht | S | HUM-039, HUM-058 |
| HUM-252 | Eine neue Regel, die nie an die Reihe kommt, wird nicht gemeldet | M | HUM-033, HUM-058 |
| HUM-253 | Folgen aus HUM-058: Protokollwechsel im Vertrag, Drosselung | S | HUM-058 |
| HUM-249 | Ein Tastenkürzel speichert eine andere Regel, als die Leiste zeigt (vor HUM-060) | S | HUM-028, HUM-058 |
| HUM-258 | Tröpfelnde Uploads hungern das Byte-Budget aus | S | HUM-057 |
| HUM-254 | Die Seitenleiste klappt auf und zu | S | HUM-019, HUM-054 |
| HUM-259 | Neues App-Icon: die Faust | S | HUM-034, HUM-053 |
| HUM-257 | Der Daemon vertraut der Zertifikatsablage des Systems (vor HUM-060) | S | HUM-015, HUM-039, HUM-087 |
| HUM-255 | Das Fenster trägt keine doppelte Titelleiste | S | HUM-019, HUM-034, HUM-054 |
| HUM-260 | Gelb wird die Akzentfarbe der Oberfläche | M | HUM-008, HUM-254 |
| HUM-261 | Die Probe versteht Endpunkte mit Pfad-Präfix (vor HUM-060) | S | HUM-037, HUM-039, HUM-076 |
| HUM-262 | Ein Projektordner, der die eigene Konfiguration enthält, fällt erst beim Start auf (vor HUM-060) | S | HUM-040, HUM-044 |
| HUM-263 | Zwischen Prüfung und Mount kann der Projektordner getauscht werden (vor HUM-060) | M | HUM-011, HUM-262 |
| HUM-265 | Shim, CA und Proxy-Socket werden weiter per Pfad gebunden (nach HUM-060) | S | HUM-263 |
| HUM-266 | Ein deflate-Body, der als zlib und als nackter Strom Verschiedenes sagt | S | HUM-247 |

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
- [x] `cargo +nightly fuzz list` im Verzeichnis `daemon/` zeigt genau: `audit_verify`, `body_decoder`, `chunked_decoder`, `flow_filter`, `http_request_path`, `rules_yaml`.
- [x] `cargo build --workspace` mit stabilem Toolchain läuft ohne Nightly.
- [x] Jedes Target hat mindestens fünf Seed-Dateien unter `daemon/fuzz/corpus/<target>/`.
- [ ] Der Workflow `fuzz-nightly` wurde einmal manuell ausgelöst und alle sechs Jobs sind grün (Link im PR).
- [x] `daemon/fuzz/README.md` enthält den sechsstufigen Triage-Prozess.
- [x] Mindestens ein Regressionstest pro Target existiert und ist grün.
- [ ] Alle in Schritt 3 bis 5 gefundenen Abstürze sind behoben und als Regressionstest abgelegt (Liste im PR).
- [x] `cargo clippy --workspace -- -D warnings` sauber, auch für die freigelegten Funktionen.

> **Abgleich bei der Umsetzung (2026-10-19).** Die Spezifikation stammt aus der Zeit vor HUM-015 bis HUM-050 und nennt Namen, die der Code nicht hat. Gebaut ist, was der Code hergibt:
>
> - **Kein eigener Parser für Anfragen und `chunked`.** Der Proxy liest Kopf und Body mit hyper; ein `request_parse.rs` mit `parse_request_head` über `httparse` und ein `ChunkedDecoder` wären genau der zweite Parser, vor dem der letzte Fallstrick warnt. `http_request_path` und `chunked_decoder` füttern deshalb hyper, gebaut wie in `handler::serve_connection`, über `tokio::io::duplex` und rufen danach die Produktionsfunktionen `connect::check_authority`, `connect::requested_upgrade`, `RuleSet::evaluate` und `body::buffer`. Einzige Abweichung vom Handler ist `half_close(true)`, weil der Test-Client nach der Eingabe seine Richtung schließt. Ohne IO im Sinn von Netz, aber mit tokio: Der Pfad ist asynchron.
> - **`decode_body` liegt in `humanitl-findings::decode`**, nicht im Proxy, war schon öffentlich und entpackt schon streamend durch ein Budget (HUM-025). Die Ratio-Grenze meldet keinen `Err(DecodeError::RatioExceeded)`, sondern einen gekürzten Body mit `FINDINGS_002`; daran prüft das Target. Ein Umbau nach Schritt 4 war nicht nötig.
> - **`parse_filter`** ist `humanitl_recorder::filter::parse`, jetzt als `humanitl_recorder::{parse_filter, Filter, FilterParam}` freigegeben, ohne Verhaltensänderung. Ein `to_sql()` gibt es nicht; das SQL steht in `Filter::sql`. Der Seed `host:*.npmjs.org size:>1mb` trifft den Fehlerpfad: einen Schlüssel `size` kennt die Filtersprache nicht.
> - **`AuditChain::verify_bytes` und `Key::test()`** sind `AuditVerifier::verify_reader` über `&[u8]` und der feste Testschlüssel `[7; 32]` aus `tests/common`. Ohne Feature `fuzzing`: Der Prüfer nimmt den Schlüssel als `&[u8; 32]`, und kein Produktions-Build trägt einen Testschlüssel.
> - **`RuleSet::parse_yaml`** ist `parse_rules_for_session` mit `SessionId::nil()`, `to_yaml` ist `serialize_rules`.
> - **Eingabeform.** Alle sechs Targets nehmen `&[u8]`; `body_decoder` liest die Kodierung aus dem ersten Byte statt über `arbitrary`. So spielt der Regressionstest jede Datei ohne Übersetzung ab, und `rules_yaml` und `flow_filter` treffen auch ungültiges UTF-8.
> - **Eine Prüfung, zwei Aufrufer.** Jede Invariante steht einmal, in `daemon/crates/<crate>/tests/regressions/check.rs`; das Target bindet die Datei über `#[path]` ein, `tests/regressions/main.rs` der Crate ebenso. `regressions_replay_<target>` spielt jede `<target>-*.bin` dort ab. Damit bleibt `hyper` und `humanitl_proxy` auch im Fuzz-Code im Proxy-Crate, und die Kopplungs-Grundlinie (`tools/check_coupling.py`) steigt nicht.
> - **Workspace.** `daemon/fuzz/Cargo.toml` trägt eine eigene leere `[workspace]`-Tabelle und ein eigenes `Cargo.lock`; ein `exclude` in `daemon/Cargo.toml` ist dafür nicht nötig.
> - **Tests.** `decode_ratio_bomb` prüft gegen 2 s statt 200 ms, weil `cargo test` im Dev-Profil ohne Optimierung läuft; der Abbruch am Budget ist trotzdem um eine Größenordnung schneller als das Entpacken der 200 MiB. Der Speichertest heißt `decode_ratio_bomb_peak_rss`, ist `#[ignore]` und misst `VmHWM` nach `clear_refs`. `chunked_huge_size_no_alloc` erwartet `BufferError::Read` oder `Cap`, `filter_never_interpolates` prüft zwanzig Nutzlasten in vier Stellungen, `audit_flip_one_byte` kippt jedes Bit jedes Bytes.
> - **Offen.** Die Kästchen „Workflow einmal manuell ausgelöst" und „gefundene Abstürze behoben" schließt der erste Lauf von `fuzz-nightly`. Lokal gab es keinen Nightly-Toolchain; die Targets liefen nur als Stable-Build ohne Coverage je 60 s, ohne Fund.

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

Übernimmt aus HUM-058: Tabellenzeilen 9 und 16

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
- [x] `humanitl config schema | jq '.properties.limits.properties | keys'` liefert genau die elf Feldnamen. *Die Namen aus `backlog/CONVENTIONS.md` 4.4, nicht die der Spezifikation oben (siehe Umsetzung); am 2026-10-19 mit dem gebauten `humanitl` in einem abgeschotteten Zuhause nachgezählt: 11.*
- [x] `humanitl config get limits.hold_body_cap_bytes` liefert `33554432` bei Default.
- [x] Alte Schlüssel `hold.body_cap_bytes`, `preview.cap_bytes`, `ipc.event_buffer` in einer `config.toml` werden weiterhin gelesen (Test `legacy_alias_keys`). *Belegt durch den vorhandenen Test `the_old_key_names_still_work` (`daemon/crates/config/tests/precedence.rs`), der alle sechs Aliasse liest; kein zweiter Test unter dem Namen der Spezifikation.*
- [ ] Alle acht Tests oben grün, `backpressure_1000_flows` in CI mit ausgegebenen Messwerten im Job-Log. *Alle acht lokal grün (Namen in der Umsetzung). Der Job-Log-Nachweis steht aus, bis die CI den neuen Schritt „Backpressure load test with measurements (HUM-057)" auf dem Issue-Branch gefahren hat; lokal gemessen: p95 151 µs, max RSS 30 MiB, 400 `Lagged` beim langsamen Zuhörer.*
- [ ] Fuzz-Target `body_decoder` läuft 5 min ohne Fund (Nachweis im PR). *Offen: Nachweis über `.github/workflows/fuzz-nightly.yml` (workflow_dispatch) nach dem Merge. Lokal gibt es keine Nightly-Toolchain; die Prüfung des Targets (`daemon/crates/findings/tests/regressions/check.rs`) kennt `LIMIT_003` und läuft als Regressionstest mit.*
- [x] `docs/reference/limits.md` existiert mit Tabelle aller elf Limits.
- [x] Sechs neue Diagnostic-Codes `LIMIT_001` bis `LIMIT_006` im Code-Register (HUM-063) mit `why`-Text in `en` und `de`.
- [x] Der Header der UI zeigt nach HUM-058 die Queue-Auslastung; hier reicht, dass `Held`-Events die Felder `queue_bytes`, `queue_count` tragen (Proto-Test). *Proto-Test seit HUM-016 (`daemon/crates/ipc/tests/proto_roundtrip.rs`); die Anzeige selbst (HUM-058 Zeile 16) hat dieses Issue übernommen.*
- [x] Übernimmt aus HUM-058: Tabellenzeilen 9 und 16 samt Tests `history_detail_test: body_cap_row_has_fix` und `status_bar_test: queue_pressure_colors` (Umplanung vom 2026-10-19).

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

### Umsetzung (HUM-057)

Vieles aus der Spezifikation stand schon: die Gruppe `limits` mit elf Feldern und Aliassen
(HUM-016, HUM-120), die Zähler der Warteschlange samt `queue_bytes`/`queue_count` im `Held`-Ereignis
(HUM-016), die Uhren über Kopf und Rumpf (HUM-120), `Lagged` im Strom (HUM-018), der Entpacker mit
Budget (HUM-025, HUM-056). Dieses Issue hat die Lücken geschlossen: die sechs Codes und wo sie
entstehen, `stream: true`, den Lasttest, die Referenzseite und die zwei Zeilen aus HUM-058.

**Was gebaut wurde.**
- `LIMIT_001` bis `LIMIT_006` im Register (`codes.rs`), `docs/DIAGNOSTICS.md` neu erzeugt, Titel und
  Grund in `app_en.arb`/`app_de.arb`, Eintrag in `diagnostic_l10n.dart`.
- `LIMIT_001` am Block über `limits.hold_body_cap_bytes` (angekündigt und `chunked`), mit dem
  Vorschlag einer Regel `ask` + `stream: true`, zugeschnitten wie der Vorschlag von `PROXY_008`
  (`handler::request_matcher`, aus `private_address_rule` herausgelöst); ohne wirksame Regel
  `ChangeSetting`. Neues Modul `daemon/crates/proxy/src/limits.rs`.
- `stream: true` im Proxy: `FlowPipeline::stream_rule` (Vorgabe `None`, `RulesPipeline` nur für
  `allow` und `ask`), `FlowHandler::over_cap` und `stream_out`, `body::buffer_request` gibt einen
  Rumpf über dem Cap als Präfix plus Rest zurück, `body::streamed_request` streamt ihn mit Uhr und
  Mitschnitt, `upstream::Outgoing` trägt gepuffert oder gestreamt, `Recorder::begin_request`
  schneidet den gestreamten Anfrage-Rumpf mit (derselbe Mitschnitt wie für Antworten, jetzt mit
  Richtung).
- `LIMIT_002` vor der Ablehnung in `HoldQueue::admit`, mit beiden Zählern und Grenzen im `why`.
- `LIMIT_003` im Entpacker, wenn das Verhältnis die engere Grenze war (`Budget::limited_by_ratio`),
  ohne Fix; zog das Vorschau-Cap, bleibt es `FINDINGS_002`, dessen Fix jetzt auf
  `limits.preview_cap_bytes` zeigt statt auf das Verhältnis.
- `LIMIT_004` am Fluss, wenn ein gestreamter Antwort-Rumpf verstummt (`TeeBody`).
- `LIMIT_005` als Protokollzeile einer abgelaufenen Kopf-Frist (`info` ohne Anfrage, `debug` für
  eine ruhende Keep-Alive-Verbindung), `LIMIT_006` als Protokollzeile bei `Lagged` im IPC-Strom.
- Befunde am Fluss bleiben: `FlowRecord::diagnostics` (höchstens `MAX_FLOW_DIAGNOSTICS` = 8), und
  `GetFlow` liefert sie in `FlowDetail.diagnostics`, auch zu einem Detail aus der Aufzeichnung.
- CI-Schritt „Backpressure load test with measurements (HUM-057)" in `.github/workflows/ci.yml`.
- `docs/reference/limits.md`; `docs/SECURITY.md` (Tabelle, Absatz `stream: true`) und
  `docs/THREAT-MODEL.md` (Schutzziel 1, Änderungshistorie) im selben Stand.
- App: `features/history/widgets/history_diagnostics.dart` (kompakte Zeile je Befund unter den
  Fakten des Details: Code, Titel aus ARB, `FixControl`), `features/shell/queue_pressure.dart` und
  `QueuePressureLabel` in der Statusleiste (ab 80 % amber, ab 100 % rot mit `LIMIT_002`),
  `DiagnosticCodes.queueFull`, ARB `shellStatusQueue`.

**Tests (Namen der Spezifikation fett).**
- `daemon/crates/proxy/tests/hold.rs`: **`hold_rejects_over_memory`**, **`hold_rejects_over_count`**;
  angepasst `budget_refuses_by_flows_and_never_evicts`, `budget_refuses_by_bytes` (der Befund steht
  jetzt im Strom).
- `daemon/crates/proxy/tests/limits.rs` (neu): **`body_cap_blocks_with_diagnostic`**,
  **`body_cap_stream_rule_passes`** (angekündigt und `chunked`), **`upstream_idle_timeout`**,
  **`client_header_timeout`**, `a_chunked_body_over_the_cap_is_blocked_with_limit_001`,
  `a_stream_rule_with_ask_holds_the_head_and_streams_only_after_allow`,
  `the_hard_block_overrides_a_stream_rule`.
- `daemon/crates/proxy/tests/backpressure.rs` (neu): **`backpressure_1000_flows`** (`#[ignore]`,
  in CI mit `--ignored --nocapture`).
- `daemon/crates/ipc/src/server.rs`: **`subscriber_lag_gets_lagged_event`**,
  `the_detail_carries_the_diagnostics_of_its_flow`; `daemon/crates/ipc/tests/ipc_server.rs`:
  `get_flow_carries_the_diagnostics_of_the_flow` (mit und ohne Aufzeichnung).
- `daemon/crates/proxy/src/limits.rs`: Vorschlag passt durch `parse_rules` und trifft die
  Anfrage, Rückfall auf `ChangeSetting`, `Info` beim Streamen, `LIMIT_004` mit doppeltem Wert.
- `daemon/crates/proxy/src/registry.rs`: `a_record_keeps_the_diagnostics_of_its_flow_up_to_the_cap`.
- `daemon/crates/findings`: `decode_ratio_bomb` (HUM-056, unverändert grün),
  `decode_ratio_bomb_reports_limit_003`, `a_bomb_stopped_by_the_cap_keeps_findings_002_with_the_cap_as_fix`,
  `the_budget_knows_which_limit_is_the_narrower_one`; angepasst `gzip_bomb_truncated` und
  `a_gzip_bomb_is_scanned_only_as_far_as_the_budget_reaches` (Verhältnis-Bombe jetzt `LIMIT_003`).
- `daemon/crates/proxy/tests/proxy.rs`: angepasst `body_over_the_cap_returns_413_without_holding`,
  `exhausted_hold_budget_returns_503` (Befund im Strom).
- `app/test/features/history/history_detail_test.dart`: **`body_cap_row_has_fix`**,
  `a detail without diagnostics draws no diagnostic row`.
- `app/test/features/shell/status_bar_test.dart` (neu): **`queue_pressure_colors`**,
  `QueueLoad.pressure` (zwei Fälle), `the status bar itself shows a full queue`.

**Mutationen (jede rot, danach zurückgespielt und per `cmp` geprüft).**
`LIMIT_002` nicht veröffentlicht (beide `hold_rejects_*`); Zähler im `why` um eins verschoben
(beide); `LIMIT_001` nicht an den Block gehängt (`body_cap_blocks_with_diagnostic`); `stream_rule`
ignoriert `stream` (`body_cap_stream_rule_passes`, `a_stream_rule_with_ask_…`); Präfix eines
`chunked` Rumpfs verworfen und gestreamter Mitschnitt nie vollständig geschlossen (je
`body_cap_stream_rule_passes`); `LIMIT_004` nicht veröffentlicht (`upstream_idle_timeout`);
`LIMIT_005` nur noch auf `debug` (`client_header_timeout`); `Lagged{n}` halbiert
(`subscriber_lag_gets_lagged_event`); Registry vergisst Befunde (Registry- und IPC-Test); Detail aus
der Aufzeichnung ohne Befunde der Registry (`get_flow_carries_the_diagnostics_of_the_flow`);
Vorschlag mit `stream: false` (Unit-Test in `limits.rs`); 60 ms Verzögerung in
`RulesPipeline::decide` (`backpressure_1000_flows` an p95); `limited_by_ratio` nie wahr (drei
Tests), Gleichstand als Verhältnis (`the_budget_knows_…`); App: Befunde im Detail verworfen
(`body_cap_row_has_fix`), Schwelle 90 % statt 80 % und Rot als Amber (`queue_pressure_colors`),
Etikett aus der Leiste entfernt (`the status bar itself shows a full queue`).

**Abweichungen von der Spezifikation, mit Grund.**
- Namen der Felder nach `backlog/CONVENTIONS.md` 4.4: `hold_max_bytes` statt
  `hold_memory_cap_bytes`, `connect_timeout_secs`, `header_timeout_secs`, `body_timeout_secs` (ein
  Schlüssel für beide Rumpf-Richtungen, HUM-120), dazu `recorder_max_body_bytes`. Elf bleiben es.
- Block am Cap ist `413`, nicht `403` (`BlockReason::BodyCap`, CONVENTIONS 3.2).
- Ein verstummter Antwort-Rumpf wird abgeschnitten und als gekürzt aufgezeichnet, kein
  `Responded{504}`: Die Kopfzeilen mit dem Status des Ziels sind dann schon beim Client
  (HUM-120). `LIMIT_004` gilt nur dieser Spanne; ein Connect-Timeout bleibt `Failed` mit
  `PROXY_003` (CONVENTIONS 4.10), ein stehender Anfrage-Rumpf `PROXY_011` (HUM-120).
- Der Entpacker bleibt beim Budget `min(cap, gepackt × Verhältnis)` aus HUM-025 statt der
  Prüfung je Block gegen die bisher gelesene Eingabe: Er bricht ebenso während des Entpackens ab
  und hält den Speicher am Cap; geändert hat sich nur, welcher Befund entsteht.
- „Flow trägt `streamed = true`": kein neues Feld am Fluss. Gestreamt erkennt man am Rumpf ohne
  Inhalt und mit `truncated`, an `findings_truncated` und am `LIMIT_001` als `Info` daneben.
- `stream: true` gilt für `allow` und `ask` (Halten mit Kopf, Rumpf nach Freigabe); der Vorschlag
  ist `ask`, aus demselben Grund wie bei `PROXY_008`. Unter `hold.hard_block_checksum_secrets`
  greift `stream: true` nie, und `LIMIT_001` schlägt dann das Cap vor statt einer Regel: „Keine
  Regel hebt die harte Sperre auf" (`docs/SECURITY.md`) gilt unverändert
  (`the_hard_block_overrides_a_stream_rule`, `under_the_hard_block_the_fix_is_the_cap_never_a_stream_rule`;
  Mutation: Prüfung der Sperre in `over_cap` entfernt, beziehungsweise in `body_over_cap`, je rot).
- Die Statusleiste nimmt die Zahl der Flüsse aus den gehaltenen Flüssen der Oberfläche und die
  Bytes aus dem letzten `Held.queue_bytes` des Daemons (Belegung des Byte-Budgets, siehe „Nach
  Review 2"); ohne gehaltene Flüsse und nach einem Neuverbinden gilt 0. Die Grenzen kommen aus
  `GetConfig`, ohne Wert die Vorgaben des Daemons. Eine laufende Belegung gehört zu HUM-258.
- Die kompakte Zeile im History-Detail ist ein eigenes Widget, nicht `HDiagnosticCard(compact)`:
  Die kompakte Karte und der `FixActionButton` gehören HUM-058; `FixControl` zeigt `AddRule` heute
  als Abzeichen „Add rule".

**Nach dem Review (Codex, Opus als Ersatz für Antigravity).**
- Erlaubnis zum Streamen und Entscheidung aus **einem** Schnappschuss der Regeln:
  `FlowPipeline::stream_grant` liefert eine `StreamGrant` (Regel, `ask`/`allow`, `allow_private`,
  Durchreiche) aus einem Lesezugriff, `decide_streamed` entscheidet nur daraus. Vorher werteten
  `stream_rule` und `decide` getrennt aus, und ein Neuladen dazwischen konnte einen ungeprüften
  Rumpf unter einer Regel ohne `stream` hinauslassen
  (`the_stream_grant_and_the_decision_come_from_one_snapshot`; Mutation: `decide_streamed` fragt
  wieder `decide`, rot).
- Byte-Budget für Rümpfe in Arbeit: `HoldQueue::in_flight` gibt einen Anteil an
  `limits.hold_max_bytes`, der beim Puffern Stück für Stück wächst (`InFlight::grow`) und beim
  Fallenlassen frei wird; ein daran gebundener Fluss (`InFlight::cover`) reserviert beim Halten
  keine Bytes ein zweites Mal. Voll heißt `503`, `Block{HoldMemory}`, `LIMIT_002`
  (`the_byte_budget_counts_bodies_while_they_are_buffered`; Mutationen: keine Freigabe, kein
  Zählen beim Puffern, je rot). Kein zwölftes Feld: derselbe Topf wie `limits.hold_max_bytes`.
- Ein gehaltener Kopf unter `ask` + `stream: true` reserviert nur sein Präfix
  (`a_held_head_reserves_only_what_is_in_memory`, 1 MiB angekündigt gegen 512 KiB Budget; Mutation:
  Bindung ignoriert, rot).
- Gestreamter Upload mit Fristen: `upstream::StreamWatch` prüft alle 100 ms, ob das Ziel länger als
  `limits.body_timeout_secs` kein Stück angenommen hat oder nach dem ganzen Rumpf länger als
  `limits.header_timeout_secs` ohne Kopfzeilen bleibt; dann bricht die Verbindung zum Ziel ab,
  der Fluss wird `Failed` mit `LIMIT_004` (`a_stalled_streamed_upload_ends_with_limit_004`;
  Mutation: keine Wache, rot).
- Kopf, Query und Kopfzeilen eines gestreamten Flusses werden gescannt
  (`the_head_of_a_streamed_flow_is_scanned`; Mutation: Funde verworfen, rot).
- Schema-Text von `limits.hold_body_cap_bytes` nennt die Ausnahme `stream: true`; Schnappschuss
  und `docs/CONFIG.md` neu erzeugt.

**Nach Review 2.**
- Antwortet das Ziel vor dem Ende eines gestreamten Uploads, bleibt der Upload gezählt und bewacht:
  Der `InFlight`-Anteil wandert in den Rumpf (`body::streamed_request`) und fällt erst mit dessen
  Ende oder Drop; `upstream::watch_after_head` bricht die Verbindung ab, wenn das Ziel danach
  länger als `limits.body_timeout_secs` kein Stück annimmt
  (`an_early_answer_keeps_the_upload_counted_and_watched`; Mutationen: Anteil im Handler
  fallen gelassen, keine Wache nach der Antwort, je rot).
- Das Präfix geht in Scheiben zu 64 KiB hinaus (`PREFIX_SLICE`, `split_to` ohne Kopie)
  (`a_large_prefix_over_a_slow_line_is_not_a_stall`: 16 MiB Präfix, Ziel liest gut 2 MiB/s,
  Frist 1 s; Mutation: keine Scheiben, rot).
- Die Fortschrittsuhr beginnt mit dem ersten Stück, nicht vor Verbindungsaufbau und Handschlag
  (`the_progress_clock_starts_with_the_first_chunk`; Mutation: Uhr ab Erzeugung, rot).
- Ein schweigender Client in einem gestreamten Upload bekommt `408` und `PROXY_011` am Fluss statt
  `502` (`a_silent_client_in_a_streamed_upload_gets_408`; Mutation: Ursache ignoriert, rot).
- `Held.queue_bytes` ist als Belegung des Byte-Budgets dokumentiert (gehaltene plus gepufferte
  und gestreamte Rümpfe, Stream-Kopf nur mit Präfix); die Statusleiste zeigt diese Zahl des
  Daemons in ihrem letzten Stand (`daemonQueueBytesProvider`) statt die angekündigten Größen zu
  summieren (`queue_pressure_colors` mit Stream-Kopf-Fall, `the occupancy follows the last Held
  event of the daemon`; Mutationen: wieder `requestSize` summiert, Ereignis ignoriert, je rot).
  Kein neues Proto-Feld.
- Gesamtfrist für das Puffern (Review 2, Punkt 5): nicht in diesem Issue. CONVENTIONS 4.4 und die
  elf Felder bleiben; ein tröpfelnder Client kann den Topf belegen, das Ergebnis ist fail-closed
  (`503`, kein Egress), und der Angreifer ist der eigene Agent in der Sandbox. Benannt in
  `docs/SECURITY.md`, weiter in HUM-258.

**Nach Review 3.**
- Bricht die Wache einen Upload nach einer frühen Antwort ab, steht `LIMIT_004` am Fluss
  (`StreamWatch::report`; Test `an_early_answer_keeps_the_upload_counted_and_watched`; Mutation:
  kein Veröffentlichen, rot).
- Client-Stille und „Ziel liest nicht" hängen nicht mehr an derselben Uhr: Jede Frage von hyper
  zählt als Fortschritt, und solange der Strom auf den Client wartet, gibt es keinen Stillstand des
  Ziels (`waiting_for_the_client_is_never_a_stall_of_the_target`; Mutation: Wartezustand
  ignoriert, rot).
- Ein schweigender Client in einem gestreamten Upload bekommt einen eigenen Satz
  (`idle_streamed_request_body`: ein Teil war schon beim Ziel), und der Fluss wird mit der `408`
  verbucht, die der Client bekam, nicht als Fehler des Ziels (`a_silent_client_in_a_streamed_upload_gets_408`;
  Mutationen: alter Satz, `Fail` statt `Respond{408}`, je rot).
- Statusleiste: Beim Neuverbinden gilt 0 (`the occupancy follows the last Held event of the
  daemon`; Mutation: Verbindung nicht beobachtet, rot). Laufende Belegung und gepufferter Body nach
  früher Antwort stehen in HUM-258.

**Kandidaten für neue Issues (außerhalb des Umfangs).**
- Befunde eines Flusses in der Aufzeichnung ablegen: aufgenommen als HUM-256 (vor HUM-060).
- HTTP/2-Streams: Ein Client kann über eine Verbindung viele Streams öffnen, und
  `limits.max_client_connections` deckelt dann nicht mehr, wie viele Präfixe gleichzeitig gelesen
  werden (das Byte-Budget deckelt die Summe, aber nicht die Zahl der Aufgaben). War schon vor
  HUM-057 so; in M1 spricht der Proxy nach unten nur HTTP/1.1.

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
- [x] Jede der 20 Tabellenzeilen hat einen grünen Test (Liste im PR mit Zeilennummer und Testname). *Neu gefasst am 2026-10-19: 15 von 20 hier, grün, Liste in der Umsetzung unten; 9 und 16 an HUM-057, 15, 17 und 20 an HUM-251, HUM-252 und HUM-250.*
- [ ] `grep -r "showDialog\|AlertDialog" app/lib/` findet nur die drei destruktiven Stellen (Block-all, Forever-Regel löschen, Sandbox stoppen) und die Regel-Sheet-Implementierung nutzt `Sheet`, nicht `Dialog`. *Das Regel-Blatt ist ein `HSheet`. Der grep findet außerdem `showDialog` für „Über Humanitl“ (HUM-031, `app/lib/features/about/about_dialog.dart`), und `HModal` tragen außer den drei destruktiven auch Palette, Modell-Suche und Export-Menü. Keins davon ist ein Fehlerzustand; dass keiner als Modal aufgeht, belegt `no_modal_for_diagnostics`.*
- [x] `flutter run -d linux --dart-define=HUMANITL_FAKE=daemon_missing` zeigt den Setup-Screen mit Diagnostic-Karte und laufendem Verbindungszähler. *Belegt ohne sichtbares Fenster: `start_without_daemon_shows_setup` löst dieselben Startoptionen auf (`LaunchOptions.resolve(fakeDefine: 'daemon_missing')`) und sieht Karte und Zähler von 1 auf 3 steigen.*
- [ ] Alle 22 Goldens (11 Zustände × dark/light) eingecheckt und in CI grün. *Angepasst an den Standard von HUM-054: fünf neue Namen `states_*` × vier Varianten = 20 Bilder, vier Zustände über bestehende Goldens, zwei fehlen mit ihren Zeilen. Lokal grün mit `scripts/ci/goldens-placeholder.sh`; der Haken für CI folgt dem Lauf auf dem Issue-Branch.*
- [x] ~~Bei `Disconnected` ist `heldFlowsProvider` leer (Test `providers_reset_on_disconnect`).~~ *Umformuliert, Entscheidung Projekteigentümer 2026-10-19 (`docs/UX.md` 4.2, Fall 4):* Bei `Disconnected` bleibt die Queue als eingefrorener Schnappschuss stehen, keine Entscheidung geht hinaus, und nach dem Wiederanschluss gleicht `ListFlows` ab. Tests: `frozen_shell_test` `a_break_keeps_the_shell`, `nothing_is_decided_while_the_connection_is_down`, `reconnect_from_the_banner_thaws_the_queue`; `flows_test` `resync_on_lagged`, `reconnect_resyncs_after_the_stream_broke`; `stream_done_test` `a_stream_that_ends_cleanly_reconnects_and_resyncs`.
- [x] Jeder Diagnostic-Code aus der Tabelle hat `why` in `app_en.arb` und `app_de.arb` (Schlüssel `diag_<code>_why`), Lint-Skript `tool/check_diag_l10n.dart` prüft Vollständigkeit gegen `daemon/crates/core-types/src/diagnostics/codes.rs` und läuft in CI. *Seit HUM-052: Schlüssel `diag<CODE>Why`, geprüft von der Prüfung `diagnostic` in `app/tool/l10n_lint.dart` (`make l10n-lint`, CI) für jeden Code des Registers.*
- [x] `flutter analyze` sauber.

### Fallstricke
- Reconnect-Backoff ohne Obergrenze führt nach Stunden zu Minuten-Wartezeiten. Max 10 s.
- *Überholt durch die Entscheidung des Projekteigentümers vom 2026-10-19 (einfrieren statt leeren, `docs/UX.md` 4.2, Fall 4); der Klick ins Leere ist dort verhindert, weil keine Entscheidung hinausgeht und nach dem Wiederanschluss abgeglichen wird:* Bei `Disconnected` die Provider nicht leeren heißt, dass eine Karte „held" bleibt, obwohl der Daemon sie längst per Timeout geblockt hat. Der Nutzer klickt Allow ins Leere. Deshalb ist Leeren Pflicht, auch wenn es kurz flackert.
- Rückkehr-Banner darf nicht bei jedem Fokuswechsel erscheinen, nur wenn zwischen Fokusverlust und Rückkehr mindestens ein Flow angekommen ist oder älter als 60 s wurde.
- Hex-Ansicht für 50 MiB Body darf nicht den ganzen Body rendern. Nur den Vorschau-Ausschnitt, virtualisiert.
- `find.byType(Dialog)` findet shadcn-Dialoge nicht zwingend, weil shadcn eigene Overlay-Typen nutzt. Den Wrapper in `packages/ui` so bauen, dass jeder Dialog über `HModal` geht, und im Test `find.byType(HModal)` prüfen.
- Streaming-Zähler in Nr. 12 nicht bei jedem Chunk `setState` auslösen, sondern maximal 10 Hz throttlen, sonst friert die History bei LLM-Streaming ein.
- Die Regel-Konflikt-Prüfung (Nr. 17) ist eine Heuristik über die Testfälle im Dry-Run, kein formaler Beweis. Text sagt „wird überdeckt" nur, wenn alle generierten Testfälle der neuen Regel von einer früheren gefangen werden; sonst schweigen.

### Referenzen
- BACKLOG.md Prinzip 7, ADR-012, Abschnitt 5 (Modal-Regel), Usability-Review 6 (Fehler und Randfälle)
- CONVENTIONS.md 3.2 (`Diagnostic`, `FixAction`), 3.9 (Provider-Namen)
- HUM-034 (Notification, Tray), HUM-045, HUM-047, HUM-049, HUM-063, HUM-068

> **Umsetzung (HUM-058, 2026-10-19).** Als dieses Issue begann, stand der größte Teil der Tabelle schon: HUM-019 und HUM-044 hatten Verbindungszustand, Setup-Bildschirm und eingefrorene Shell gebaut, HUM-030 die Rumpf-Ansichten, HUM-034 das Rückkehr-Banner, HUM-047 den Editor, HUM-136 das Ende des Terminals. Gebaut wurde, was fehlte; was es schon gab, ist unten mit seinen Tests genannt. Pfade und Namen folgen dem Code, nicht der Liste „Betroffene Pfade": `ConnectionStatus` und `connectionStateProvider` stehen in `app/lib/core/ipc/connection.dart`, die Karte heißt `HDiagnosticCard` (`app/lib/core/ui/h_diagnostic_card.dart`), der Knopf der Abhilfe `FixControl` (`app/lib/core/ui/fix_control.dart`).
>
> **Codes.** Das Register belegt die Nummern anders als die Tabelle: Verbindung verloren ist `DAEMON_001` (die Tabelle sagt `DAEMON_002`), der unverträgliche Vertrag `DAEMON_002` (Tabelle `DAEMON_003`, im Register „Socket bereits belegt"), das zurückgewiesene Token `IPC_001` (Tabelle `DAEMON_004`, im Register das Laufzeitverzeichnis), der zweite Schreiber im Terminal `TERM_001`. Die Bereiche `LIMIT` und die Codes `LIMIT_001` bis `LIMIT_006` gibt es noch nicht; sie gehören HUM-057.
>
> | Nr | Stand | Test |
> |---|---|---|
> | 1 | Gebaut: Die Zeile des Dienstes zählt die gescheiterten Versuche („3 attempts failed · trying again", `ConnectionStatus.failed.attempts`), Szenario `daemon_missing`. Karte und Abhilfe (`InstallService`, bei `--fake`/`--socket` der Startbefehl) gab es seit HUM-044. | `connection_state_test`: `start_without_daemon_shows_setup` (über `LaunchOptions.resolve(fakeDefine: 'daemon_missing')`, also derselbe Weg wie `--dart-define`), `the attempts line goes once the daemon answers`; Golden `states_daemon_missing` |
> | 2 | Bestand, **Entscheidung Projekteigentümer 2026-10-19**: Die Shell bleibt stehen und friert ein (`docs/UX.md` 4.2, Fall 4, HUM-044), statt nach 5 s zum Setup zu wechseln und die Warteschlange zu leeren. Keine Entscheidung geht hinaus, solange die Verbindung fehlt, und nach dem Wiederanschluss gleicht ein `ListFlows` die gehaltenen Zeilen ab, so dass eine längst abgelaufene verschwindet. Neu: Der Rückfall des Ereignisstroms ist auf 10 s gedeckelt statt auf 30 s, und ein Strom, der sauber endet, wird wie ein abgerissener neu aufgebaut (vorher blieb die Warteschlange danach taub; Review). | `frozen_shell_test`: `a_break_keeps_the_shell`, `nothing_is_decided_while_the_connection_is_down`, `reconnect_from_the_banner_thaws_the_queue`; `flows_test`: `resync_on_lagged`, `reconnect_resyncs_after_the_stream_broke`; `stream_done_test`: `a_stream_that_ends_cleanly_reconnects_and_resyncs` |
> | 3 | Gebaut: Kein `Subscribe`, bevor `GetInfo` angenommen ist (`flowEventsProvider` wartet auf `linkLiveProvider`, wo es die Verbindung gibt). Karte und Abhilfe `OpenUrl` gab es. | `connection_state_test`: `major_mismatch_incompatible`; Golden `states_incompatible` |
> | 4 | Gebaut: `IPC_001` trägt `CopyCommand("systemctl --user restart humanitld")`, auch der Befund, den der Daemon selbst schickt (`ClientDiagnostics.withTokenFix`); bei `--fake` oder `--socket` keinen, weil dann nicht die Unit gemeint ist. Szenario `unauthenticated` (als Daemon von Hand, also ohne Vorschlag). | `connection_state_test`: Gruppe `unauthenticated` |
> | 5 | Bestand (HUM-068). | Golden `states_sandbox_failed` (vier Varianten; das alte `sandbox_header_failed` hat nur zwei) |
> | 6 | Bestand (HUM-041, HUM-054). | Golden `isolation_panel_failed` |
> | 7 | Gebaut: Das Band nennt die Wartezeit und die Folge („Blocked (timed out after 300 s). The agent may retry.“). Darunter steht der Satz der Regel aus dem erhaltenen Entwurf des Rasters und „Als Regel übernehmen“; der Knopf legt sie über `Rules(add)` an (offenes Raster: seine Dauer, sonst die Session). Die Zeile bleibt 3 s statt 5 s stehen (`docs/UX.md` 4.8, HUM-029). | `flow_card_states_test`: `timeout_banner_names_the_wait`, `timeout_banner_keeps_draft`; Golden `request_card_timed_out` (neu erzeugt) |
> | 8 | Bestand (HUM-030, HUM-047) mit Abweichung: „Bearbeiten" bleibt an, weil Methode, Pfad und Kopfzeilen auch bei einem zu großen Rumpf bearbeitbar sind; der Editor sagt, dass der Rumpf es nicht ist. | `detect_too_large`, `hex_first_64k_only`, `binary_body_disables_editor`; Golden `request_card_large` |
> | 9 | **An HUM-057** (Umplanung 2026-10-19): `LIMIT_001` steht nicht im Register. | dort: `history_detail_test: body_cap_row_has_fix` |
> | 10 | Bestand (HUM-030, HUM-047) plus gebaut: über einem binären Rumpf die Vermutung aus den ersten Bytes („The first bytes look like image/png …“) und, wo der Detektor nur druckbare Läufe durchsucht, genau dieser Satz (`app/lib/core/body/binary_facts.dart`, Spiegel von `is_textual` in `daemon/crates/findings/src/input.rs`). | `binary_facts_test`: `binary_body_hex_view`, `magic bytes`, `the strings line mirrors the detector`; `detect_binary_ratio`, `the hex view`, `binary_body_disables_editor`; Goldens `request_card_binary`, `history_detail_body_hex_*` (neu erzeugt) |
> | 11 | Gebaut: Abzeichen „WebSocket“, Satz über die dauerhafte Verbindung, Knopf „Allow connection“ / „Verbindung öffnen“ (bestätigt vom Projekteigentümer 2026-10-19). Bis das Detail geladen ist, ist die Freigabe aus, auch über `Enter`, `A` und jede Gültigkeit (Review): Erst das Detail sagt, ob gewarnt werden muss. Die Freigabe aus der Benachrichtigung fragt beim Druck nach dem Detail und gibt einen WebSocket-Aufbau nie frei; das Fenster kommt dann auf der Anfrage nach vorn. Eine Freigabe über mehrere Anfragen (Gruppe, Auswahl, „Allow all“) fragt in `_many` nach jedem Detail; ein WebSocket-Aufbau oder eine Anfrage ohne Detail bleibt gehalten, der Cursor springt auf sie, und die Leiste sagt warum. Durch den Sammelpfad darf nur die eine Anfrage, deren Karte mit geladenem Detail entschieden wurde (`BatchRequest.cardSeen`, etwa hinter dem Modal für eine Regel „für immer“). Ein Detail ohne Kopfzeilen gilt auf allen Wegen als nicht verfügbar (`requestIsReadable`, `detailSufficesForAllow`), und die Karte sagt es. Ein Stapel überspringt eine Anfrage, die die Warteschlange inzwischen verlassen hat, statt abzubrechen, und wartet auf jedes Detail höchstens 5 s. Trägt ein Stapel eine Regel, ist sie das Objekt, das beim Öffnen des Modals beziehungsweise für den Satz der Leiste einmal gebaut wurde (`ShownRule` im `BatchRequest`, samt Quelle); `_many` baut keine eigene, und die Regel reist nur an der Entscheidung ihrer Quelle, die zuerst geht. Ist die Quelle zurückgehalten, verlassen, gesperrt oder mit `FLOW_NOT_HELD` beantwortet, geht nichts hinaus und keine Regel entsteht. Die Leiste und die Benachrichtigung nennen den wirklichen Grund (WebSocket, unlesbar, Detail kam nicht, Warteschlange verlassen). Der Editor zeigt über dem Senden dieselbe Zeile wie die Karte. Die Erkennung spiegelt `requested_upgrade` in `daemon/crates/proxy/src/connect.rs` (nur die Kopfzeile `Upgrade`, das Schema zählt dort nicht; nach dem Review angeglichen). `FlowSummary.upgrade` im Vertrag ist HUM-253. | `flow_card_states_test`: `websocket_card_copy`, `requestOpensWebSocket mirrors the daemon`; `allow_waits_for_detail_test`: `allow_waits_for_the_detail`; `tray_websocket_test`: `a_notification_allows_no_websocket_unseen`; `batch_websocket_test`: `a_group_allow_leaves_the_websocket_held`, `a request whose detail does not arrive stays held too`; `allow_paths_test` (12 Tests); `editor_websocket_test`; Golden `states_websocket_card` |
> | 12 | Bestand (HUM-032): laufende Summe mit „still arriving", danach die Gesamtgröße. Nicht gebaut: Drosselung auf 10 Hz. | `history_streaming_test`: `streaming_counter_updates` |
> | 13 | Bestand (HUM-034). | `attention_test`: `banner_after_60s_on_focus`, `banner_stays_away_below_the_threshold`, `banner_goes_when_the_queue_empties`; `return_banner_test`: `the_banner_says_one_line_and_offers_two_ways_out` |
> | 14 | Bestand (HUM-045). | Golden `states_tls_rejected` (vier Varianten; `intercept_diagnostic_tls` hat zwei) |
> | 15 | **An HUM-251**: Der Daemon meldet ein unerreichbares Modell im Lauf nicht als `LLM_001`, und ein gescheiterter Verbindungsaufbau ist nach CONVENTIONS 4.10 `Failed`, nie `Responded{502}`. | — |
> | 16 | **An HUM-057** (Umplanung 2026-10-19): `LIMIT_002` fehlt, und die Oberfläche kennt `limits.hold_max_flows` nicht. | dort: `status_bar_test: queue_pressure_colors` |
> | 17 | **An HUM-252**: Der Dry-Run zählt nur die Treffer einer Regel; welche frühere Regel jeden Testfall fängt, kann nur der Daemon sagen (ADR-018). | — |
> | 18 | Bestand (HUM-049). | `findings_pause_test`, Golden `findings_pause` |
> | 19 | Gebaut: Endet der Terminal-Strom ohne Exit-Code oder bricht die Leitung (`DAEMON_001`), schreibt das Fenster „[Humanitl] Verbindung zum Terminal verloren, verbinde neu …“ als letzte Zeile ins Terminal (gedämpft, SGR 90) und hängt sich nach 1 s, 2 s, 4 s bis höchstens 10 s wieder an; das `Open` trägt die Geometrie von jetzt, Emulator und Rücklauf bleiben. Eine Absage und ein Ende des Agenten planen nichts. `TERM_001` heißt im Register „zweiter Schreiber“; kommt er beim Wiederanschluss, hängt sich das Fenster wie seit HUM-067 lesend an. Der Streifen von HUM-136 bleibt. | `terminal_reconnect_test`: `reconnect_keeps_buffer`, `an agent that ended is not attached again`, `a refusal of the daemon is not tried again`; `terminal_end_test` (HUM-136) |
> | 20 | **An HUM-250** (vor HUM-060): Der Daemon schreibt einen Fehler der Aufzeichnung nur ins Log (`daemon/crates/recorder/src/writer.rs`, `report`). | — |
>
> **Goldens.** Seit HUM-054 hat jedes Golden vier Varianten (dunkel und hell, Englisch und Deutsch) und steht in `app/test/goldens/support/golden_names.dart`. Neu sind fünf Namen mit dem Präfix `states_` statt eines Ordners `states/` (`app/test/goldens/states_golden_test.dart`), 20 Bilder; vier weitere Zustände hält HUM-054 schon in derselben Form fest, zwei fehlen mit ihren Zeilen.
>
> **Weitere Tests.** `diagnostic_card_all_fix_actions` steht in `app/test/core/ui/fix_action_verbs_test.dart`; die ARB-Schlüssel der Verben heißen seit HUM-044 `setupFix…`, nicht `fix_set_env`. `no_modal_for_diagnostics` steht in `app/test/features/shell/no_modal_test.dart`, mit einer Gegenprobe, dass der Finder ein `HModal` überhaupt sieht.

---

## HUM-220 · Das Terminal der Oberfläche zeigt OpenCode nicht sauber an
Sprint: 5 · Größe: L · Abhängigkeiten: HUM-042, HUM-067, HUM-136 · Blockiert: HUM-060

### Kontext
Am 2026-09-18 meldete der Nutzer, dass OpenCode im Terminal der Oberfläche „nicht sauber in der gui angezeigt“ wird. HUM-042 hat das Bild nur für einen Fall abgenommen: Die Sitzung startet über den Knopf, und die Oberfläche ist der einzige Schreiber. Das vierte Kriterium von HUM-042 hält selbst fest: „Die dritte Aussage des Kriteriums, das Neuzeichnen ohne Zeilensalat, ist nicht gemessen“ (`backlog/sprint-3.md:1130`).

Die Codelektüre vom 2026-09-23 (Stand `main` 2a8df37, keine Messung) findet sechs mögliche Ursachen, geordnet nach Wahrscheinlichkeit. Welche den Bericht erklären, entscheidet Schritt 1. Das Issue behebt jede, die sich bestätigt.

1. **Leser rendern nicht letterboxed.** CONVENTIONS 4.10 (`backlog/CONVENTIONS.md:407`) und das Nicht-Ziel von HUM-042 versprechen: „Geometrie ist die des Schreibers, Leser erhalten `Resize`-Events und rendern letterboxed.“ `TerminalSession._onFrame` (`app/lib/features/sandbox/providers/terminal_provider.dart:240-270`) legt die Geometrie aber nur in den Zustand, und `TerminalPane` (`terminal_pane.dart:276-296`) liest sie nie. `TerminalView` folgt mit `autoResize` dem Fenster. Die Oberfläche wird zum Leser, sobald `humanitl run` schreibt (`TERM_001`, dann `_watchInstead`, `terminal_provider.dart:282`). Laut HUM-067 Kriterium 4 ist das der Normalfall.
2. **Der Rückstand stellt dem Agenten alte Fragen, und der Emulator beantwortet sie als Tastendrücke.** In der Politik `FullScreen` lässt der Filter jede CSI außer `t` hinaus (`daemon/crates/core-types/src/terminal.rs:724-757`). Der Ring hält die Fragen (`daemon/crates/ipc/src/terminal.rs:364-368`), und jeder Anschluss bekommt ihn, der Schreiber eingeschlossen (`:333`, `:599-603`). `xterm2` 5.2.0 beantwortet DA1/DA2, DSR 6, XTVERSION und DECRQM über `onOutput` (`xterm2/lib/src/terminal.dart:1525-1553`, `:1686`, `:2835`). `onOutput` ist die Tastatur (`terminal_provider.dart:155`).
3. **Der Ring beginnt irgendwo, und ein langsamer Client verliert Bytes ohne Ausgleich.** `emit` schneidet an einer beliebigen Byte-Grenze ab (`ipc/src/terminal.rs:366-367`). Nach mehr als 64 KiB fehlen dann die Moduswahl vom Start (`?1049h`, `?1000`/`?1006`, `?2004`) und der Anfang der ersten Folge. `forward` verwirft `RecvError::Lagged` stillschweigend (`:686`). Ein Differenz-Renderer zeichnet die verlorenen Zellen nicht von selbst neu.
4. **Das erste Bild entsteht für 80x24, bevor jemand zusieht.** Der Start läuft mit `DEFAULT_TERMINAL_SIZE` (`ipc/src/sandbox.rs:244`), und die Oberfläche hängt sich erst nach ihrem ersten Frame an (`terminal_pane.dart:82-89`). Bis dahin beantwortet niemand die Startfragen des TUI. Der Rückstand läuft danach im alten Raster in einen größeren Emulator.
5. **Farbfragen bleiben unbeantwortet.** Der Provider baut `Terminal` ohne `onColorQuery` und ohne `onColorSchemeQuery` (`terminal_provider.dart:152`). Deshalb schweigt `xterm2` auf `OSC 10/11 ; ?`, `OSC 4 ; n ; ?` und `CSI ? 996 n` (`xterm2/lib/src/terminal.dart:3607`, `:3733-3735`, `:1675-1677`), obwohl der Filter diese Fragen hinauslässt (`OSC_ALLOWED`). Eine Hell/Dunkel-Erkennung im TUI läuft damit ins Leere.
6. **Die Schrift ist nicht die entworfene.** `JetBrains Mono` (`app/packages/ui/lib/src/tokens/typography.dart:27`) ist weder gebündelt noch auf der Maschine des Nutzers installiert, also greift `Fira Code`. `TerminalStyle` läuft mit der Vorgabe `height: 1.2`.

Geprüft und bisher keine Ursache: `TERM=xterm-256color`, `COLORTERM=truecolor` und `LANG=C.UTF-8` setzt der Adapter (`daemon/crates/sandbox/src/agent/opencode.rs:670-672`). terminfo liegt unter dem ro gebundenen `/usr` (`profiles/sandbox/default.toml:43`). `SIGWINCH` geht an die Prozessgruppe des Sandbox-Init (`daemon/crates/sandbox/src/handle.rs:880-899`). Gemessen ist das nur mit `/bin/sh` als Agent (`daemon/crates/sandbox/tests/pty.rs:206-244`), und dieser Test endet grün, wenn `stty` fehlt. Keine dieser drei Aussagen hat heute einen Test, der am echten Agenten rot werden kann.

### Ziel
OpenCode steht im Terminal der Oberfläche so da wie in einem gewöhnlichen Terminalfenster derselben Größe, und zwar in allen drei Lagen: Die Oberfläche schreibt. Die Oberfläche sieht nur zu, während `humanitl run` schreibt. Die Oberfläche hängt sich spät oder erneut an. Kein Anschluss schickt dem Agenten Bytes, die kein Mensch getippt hat.

### Nicht-Ziel
- Kein Feld für die Geometrie in `SandboxRequest.Start` und kein Aufschub des Agentenstarts bis zum ersten Schreiber. Beides wäre eine Vertragsänderung mit eigener Entscheidung. Zeigt Schritt 1, dass es nötig ist, bekommt es ein eigenes Issue.
- Kein zweiter Filter in der Oberfläche (CONVENTIONS 4.28). Was aus dem Rückstand fällt, nimmt der Daemon heraus.
- Keine Änderung an `OSC_ALLOWED` und an dem, was live hinausgeht. Die Sicherheitsaussage in `README.md` und `docs/SECURITY.md` 3.3 bleibt wörtlich.
- Kein Wechsel weg von `xterm2`.

### Betroffene Pfade
- `daemon/crates/core-types/src/terminal.rs`: Der Filter ordnet jede ausgegebene Folge ein (Frage, Modus setzen, sonst).
- `daemon/crates/ipc/src/terminal.rs`: Ring ohne Fragen, Ringanfang an einer Grenze, Modus-Schnappschuss vor dem Rückstand, Neuzeichnen nach `Lagged`.
- `daemon/crates/sandbox/src/handle.rs`: `redraw()`, falls `resize` mit gleicher Größe nicht genügt.
- `daemon/crates/ipc/tests/terminal.rs`, `daemon/crates/sandbox/tests/pty.rs`: neue Tests.
- `app/lib/features/sandbox/providers/terminal_provider.dart`: Leser übernehmen die Geometrie; `onColorQuery` und `onColorSchemeQuery` antworten aus der Palette.
- `app/lib/features/sandbox/widgets/terminal_pane.dart`: Letterbox für Leser, `TerminalStyle.height` ausdrücklich gesetzt.
- `app/packages/ui` (`pubspec.yaml`, `typography.dart`, OFL-Lizenztext): `JetBrains Mono` Regular gebündelt, falls Schritt 1 die Schrift als Anteil bestätigt.
- `app/test/features/sandbox/terminal_pane_test.dart`, `app/test/features/sandbox/harness.dart`, `app/test/goldens/`, `fixtures/terminal/`.
- `backlog/CONVENTIONS.md`: Abschnitt „Aus der Umsetzung“.
- `app/lib/core/ipc/fake_daemon_client.dart` nur bei Bedarf. Die Datei wird geteilt: vor jedem Schreiben neu lesen, nur anhängen.

### Spezifikation
**Schritt 1 misst, bevor gebaut wird.** Gegen den echten Daemon und OpenCode werden drei Lagen aufgenommen, jeweils mit dem Rohstrom von `humanitl sandbox attach --read-only` und einem Bildschirmfoto der Oberfläche: (a) Start über den Knopf, Fenster unverändert. (b) `humanitl run` in einem Terminal von 200x50, die Oberfläche danach geöffnet. (c) wie (a), dann mehr als 64 KiB Ausgabe, die Oberfläche schließen und neu öffnen. Gezählt werden die Fragen an das Terminal (`CSI c`, `CSI > c`, `CSI 6 n`, `CSI ? … $ p`, `CSI ? u`, `CSI > q`, `OSC 4/10/11 ; ?`, `CSI ? 996 n`), die ersten 64 Bytes des Rückstands und das, was OpenCode auf `SIGWINCH` schreibt (`CSI 2 J` oder ein vollständiges Bild). Die Rohströme von (a) und (b) werden, gekürzt auf ein stabiles Bild, als `fixtures/terminal/opencode-a.bin` und `opencode-b.bin` eingecheckt. Sie enthalten keine Geheimnisse, denn der Agent spricht nur mit dem Mock. Das Ergebnis steht mit Datum in diesem Issue, bevor Code geändert wird.

**Leser rendern letterboxed.** Kommt `TerminalGeometry` bei einer lesenden Sitzung an, ruft der Provider `terminal.resize(cols, rows)`. `TerminalPane` setzt `autoResize: false` und zeichnet eine Fläche von genau `cols` mal `rows` Zellen, zentriert auf `tokens.terminal.background`. Fehlt der Platz, wird die Fläche verkleinert, nie beschnitten. Ein Leser schickt kein `TerminalResize` hinauf. Ein Schreiber behält `autoResize`.

**Der Ring ist ein Bild, keine Fragenliste.** Der Filter kennt die Grenzen jeder Folge und ordnet jede hinausgehende Folge ein. `TerminalHub::emit` legt keine Frage in den Ring. Live geht weiter alles hinaus, was die Politik erlaubt, denn ein Schreiber, der beim Start dabei ist, muss antworten können. Die Liste der Fragen steht als Konstante neben `OSC_ALLOWED`, mit einer Zeile Begründung je Eintrag.

**Der Ring beginnt an einer Grenze und bringt die Modi mit.** Beim Abschneiden läuft `emit` bis zur nächsten Folgen- oder Zeichengrenze vor. Der Hub merkt sich den letzten Zustand einer geschlossenen Liste von DEC-Privatmodi (`25`, `47`, `1047`, `1049`, `1000`, `1002`, `1003`, `1004`, `1006`, `2004`, `2026`) und das zuletzt gesetzte kitty-Tastaturprotokoll. `attach` schickt diesen Zustand vor dem Rückstand.

**Nach einem Verlust und bei jedem Schreiber-Anschluss zeichnet der Agent neu.** `Lagged` löst gleiche Größe plus `SIGWINCH` aus, gedrosselt mit `RESIZE_INTERVAL`. Zeichnet OpenCode laut Schritt 1 bei unveränderter Größe nicht neu, geht der Hub kurz über `rows - 1` und zurück. Der Kommentar an `ipc/src/terminal.rs:681-686` wird berichtigt.

**Farbfragen bekommen eine Antwort.** `onColorQuery` antwortet für 4, 10, 11 und 12 aus `HTerminalPalette`, `onColorSchemeQuery` aus der Helligkeit des Hintergrunds. Das ist keine OSC-Behandlung im Sinn von `docs/SECURITY.md` 3.3: Die Antwort geht nur an den Agenten und nie in Zwischenablage oder Titel.

**Schrift.** Nur wenn Schritt 1 sie bestätigt: `JetBrains Mono` Regular wird in `app/packages/ui` gebündelt, samt OFL-Text, und die Größe der Datei steht im Commit-Body. `TerminalStyle.height` wird auf den Wert gesetzt, den das Bildschirmfoto begründet.

### Tests
- `a_tui_frame_renders_at_the_writers_geometry` (`terminal_pane_test.dart`, Widget-Test unter `TargetPlatformVariant.only(TargetPlatform.linux)`): Der Fake spielt `fixtures/terminal/opencode-b.bin` einer lesenden Sitzung mit `TerminalGeometry(200, 50)` in ein Fenster, das 120x30 Zellen böte. Zusicherungen: `terminal.viewWidth == 200` und `viewHeight == 50`; die Pufferzeile mit dem oberen Rand des Eingabefelds trägt ihre Rahmenzeichen in Spalte 0 und Spalte 199; keine Zeile enthält `[?`, `;1R`, `rgb:` oder `$y`; die Fläche ist kleiner als das Fenster und nicht beschnitten. Mutationsprobe: `terminal.resize` im Leserzweig entfernen, rot.
- `opencode_frame` (`app/test/goldens/`, Golden unter `TargetPlatformVariant linux`): Dasselbe Bild aus `opencode-a.bin` als Schreiber in 120x30, mit gebündelter oder Test-Schrift. Mutationsprobe: `TerminalStyle.height` ändern, Golden rot.
- `a_writer_keeps_its_own_geometry` und `a_reader_sends_no_resize` (`terminal_pane_test.dart`, linux): Ein Schreiber folgt dem Fenster und nicht der Nachricht. Ändert sich das Fenster eines Lesers, sieht `SandboxTestClient` kein `TerminalResize`. Mutationsprobe je Test.
- `a_colour_query_is_answered_from_the_palette` (Provider-Test): Auf `OSC 11 ; ? BEL` im Strom folgt ein `TerminalKeys` mit dem Hintergrund der Palette. Mutationsprobe: `onColorQuery` entfernen, rot.
- `a_replay_asks_nothing` (`ipc/tests/terminal.rs`, echte Sandbox): Der Agent schreibt `CSI c`, `CSI 6 n`, `OSC 11 ; ? BEL`, `CSI ? 2026 $ p` und danach `READY`. Ein früher Client bekommt alle vier Fragen live. Ein später Client bekommt `READY` und keine der vier. Mutationsprobe: den Ausschluss in `emit` entfernen, rot.
- `a_late_client_gets_the_modes` (`ipc/tests/terminal.rs`): `?1049h`, `?1006h` und `?2004h`, danach 80 KiB Text. Der Rückstand beginnt mit genau diesen Modi, sein Text beginnt nicht mit einem Folgebyte `0x80..=0xbf`, und nichts im Rückstand ist eine Folge ohne Einleiter. Zwei Mutationsproben: Schnappschuss weglassen, Vorlauf zur Grenze weglassen.
- `a_lagging_client_triggers_a_redraw` (`ipc/tests/terminal.rs`): Ein Agent, der auf `SIGWINCH` `stty size` meldet, und ein Client, der nicht liest, bis `Lagged` eintritt. Danach steht eine neue `SIZE`-Zeile im Strom. Mutationsprobe: der `Lagged`-Zweig wieder `{}`, rot.
- `a_resize_reaches_a_grandchild_of_the_shim` (`sandbox/tests/pty.rs`): Wie `a_resize_reaches_the_agent`, aber der Agent meldet seine Größe aus einem Enkelprozess (`sh -c 'sh -c "…"'`), wie bei OpenCode mit seinem Starter. `stty` und `bwrap` fehlen: Der Test schlägt fehl, statt grün zu überspringen. Mutationsprobe: `kill_process_group` in `resize` entfernen, rot.
- `the_agent_sees_a_usable_terminal` (`sandbox/tests/pty.rs`, Profile `default` und `llm-only`, Umgebung aus dem OpenCode-Adapter): In der Sandbox gibt `tput -T "$TERM" colors` `256` aus, `test -t 0` gilt, `$COLORTERM` ist `truecolor` und `$LANG` ist `C.UTF-8`. Mutationsprobe: `TERM` im Adapter auf `dumb`, rot; `/usr` aus `ro`, rot.

### Akzeptanzkriterien
- [ ] Schritt 1 ist gemessen: drei Transkripte und drei Bildschirmfotos liegen vor, zwei Fixtures sind eingecheckt, und für jede der sechs Ursachen steht mit Datum und Fundstelle „bestätigt“ oder „nicht beobachtet“.
- [ ] Ein Leser zeigt das Bild in der Geometrie des Schreibers, zentriert und nie beschnitten. Belegt durch den Widget-Test über `opencode-b.bin` unter `TargetPlatformVariant linux`.
- [ ] Ein Bild von OpenCode ist als Golden unter `TargetPlatformVariant linux` festgehalten und wird bei geänderter Zellhöhe rot.
- [ ] Ein Resize erreicht den Agenten auch als Enkel des Shims. `a_resize_reaches_the_agent` und der neue Test werden nie still übersprungen.
- [ ] TERM, COLORTERM, LANG und terminfo sind in der Sandbox für beide Profile per Test belegt, nicht per Lektüre.
- [ ] Ein später oder erneuter Anschluss schickt dem Agenten kein einziges Byte, das kein Mensch getippt hat. In Lage (c) steht im Eingabefeld von OpenCode nach dem Wiederöffnen derselbe Text wie davor, gemessen am Transkript und nicht am Bild.
- [ ] Jeder Test aus „Tests“ ist grün, jede genannte Mutationsprobe ist einmal rot gelaufen, und der Commit-Body nennt sie.
- [ ] Menschenauge: Der Nutzer prüft gegen den echten Daemon (1) Start über den Knopf, (2) `humanitl run` im eigenen Terminal bei offener Oberfläche, (3) Fenster ziehen, (4) Schließen und Wiederöffnen, während OpenCode antwortet. In allen vier Fällen: kein Zeilensalat, keine Reste alter Bilder, Rahmen durchgezogen, Farben wie im Terminal des Nutzers. Sein Wortlaut steht im Kriterium.
- [ ] Die Sicherheitsaussage ist unverändert: `OSC_ALLOWED` unverändert, `tests/escape/` grün, live geht keine Folge hinaus, die vorher nicht hinausging.
- [ ] `make check` und `make flutter-analyze` grün, `tools/verify-commit.sh` auf dem Commit grün, `python3 tools/check_coupling.py` ohne steigende Zahl.

### Fallstricke
- Fragen fallen nur aus dem Ring, nie aus dem Live-Strom. Sonst läuft die Erkennung von OpenCode auch dann ins Leere, wenn jemand zusieht.
- `autoResize: false` nur beim Leser. Ein Schreiber ohne `autoResize` meldet nie eine neue Größe, und `a_resize_of_the_window_goes_up_as_a_command` muss grün bleiben.
- `flutter test` läuft als Android. Ohne `TargetPlatformVariant linux` prüft der Widget-Test eine Tastatur- und Schriftwelt, die der Nutzer nie sieht.
- Ein Golden ohne feste Schrift ist ein Golden der Test-Maschine. Die Schrift wird im Test geladen, sonst misst der Golden nichts.
- Der Modus-Schnappschuss ist eine geschlossene Liste. Jeder weitere Eintrag ist eine Behauptung des Daemons über den Agenten.
- `TerminalSessionState.copyWith` kann kein Feld leeren. Wer beim Wechsel zwischen Leser und Schreiber etwas zurücksetzt, baut den Zustand neu, wie `_onFrame` es für `DAEMON_001` tut.
- Der Ring hält weiter **gefilterte** Bytes. Das Herausnehmen der Fragen ist ein Schritt nach dem Filter, kein zweiter Filter davor.
- Tests mit echter Sandbox laufen einzeln, mit `nice -n 19`, im eigenen `CARGO_TARGET_DIR` und nie in einem privilegierten Container.

### Referenzen
Bericht des Nutzers vom 2026-09-18; HUM-042 (`backlog/sprint-3.md:1024`, `:1130`); HUM-067 Kriterium 4; HUM-136 (`backlog/sprint-4.md`); HUM-152; CONVENTIONS 4.10 und 4.28; `daemon/crates/ipc/src/terminal.rs`; `daemon/crates/core-types/src/terminal.rs`; `daemon/crates/sandbox/src/handle.rs:880-899`; `daemon/crates/sandbox/tests/pty.rs`; `app/lib/features/sandbox/providers/terminal_provider.dart`; `app/lib/features/sandbox/widgets/terminal_pane.dart`; `xterm2` 5.2.0 (`lib/src/terminal.dart`, `lib/src/ui/terminal_text_style.dart`). Zeilennummern aus der Lektüre vom 2026-09-23, vor dem Bau gegenlesen.

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

## HUM-256 · Befunde eines Flusses überleben den Neustart des Daemons
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-026, HUM-057, HUM-250 · Blockiert: —

### Kontext
Seit HUM-057 hängt jeder Befund, den der Proxy einem Fluss zuordnet (`LIMIT_001` mit dem
Regelvorschlag `stream: true`, `FINDINGS_002`, `LIMIT_003`, `LIMIT_004`, `PROXY_008` …), am
Datensatz der Registry (`FlowRecord::diagnostics`), und `GetFlow` liefert ihn in
`FlowDetail.diagnostics`, auch zu einem Detail aus der Aufzeichnung. Die Registry lebt aber nur im
Prozess: Nach einem Neustart des Daemons steht ein am Cap geblockter Fluss in der Historie ohne
`LIMIT_001`, ohne Grund und ohne den Weg hinaus. Die Aufzeichnung führt keine Befunde; dafür
braucht sie eine Schema-Migration. Herausgelöst aus dem Review zu HUM-057; die Anzeige im
History-Detail stammt aus HUM-058 (Zeile 9) und HUM-250.

### Ziel
Die Aufzeichnung legt die Befunde eines Flusses ab, und `GetFlow` liefert sie auch nach einem
Neustart des Daemons in derselben Reihenfolge und mit demselben `fix`.

### Nicht-Ziel
Befunde ohne Fluss (sitzungsweite, `flow_id: None`). Neue Codes. Eine Suche über Befunde in der
Filtersprache.

### Betroffene Pfade
- `daemon/crates/recorder/src/schema.rs` (Migration: Tabelle `diagnostics` je Fluss)
- `daemon/crates/recorder/src/writer.rs` (`FlowEvent::Diagnostic` mit `flow_id` schreiben)
- `daemon/crates/recorder/src/lib.rs`, `types.rs` (`FlowDetail.diagnostics`)
- `daemon/crates/ipc/src/convert.rs`, `server.rs` (Detail aus der Aufzeichnung trägt sie; die
  Registry bleibt der Rückfall für die laufende Sitzung)
- `daemon/crates/recorder/src/retention.rs` (Befunde fallen mit ihrem Fluss)

### Akzeptanzkriterien
- [ ] Ein am Cap geblockter Fluss zeigt nach einem Neustart des Daemons `LIMIT_001` mit dem
  Regelvorschlag im History-Detail (Integrationstest über `restart_of`).
- [ ] Höchstens `MAX_FLOW_DIAGNOSTICS` Befunde je Fluss, wie in der Registry.
- [ ] Die Retention entfernt sie mit dem Fluss; kein verwaister Eintrag.
- [ ] Geheimnisse im `why` sind maskiert wie Kopfzeilen und Pfade (`SecretMasks`).

### Fallstricke
- Ein `fix` mit `AddRule` trägt eine ganze Regel; abgelegt wird die Wire-Form, nicht ein eigenes
  Format, sonst laufen zwei Serialisierungen auseinander.
- Migration vorwärts, alte Datenbanken bleiben lesbar.

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
- `recorder.retention_days` (Vorgabe 180 seit HUM-051, vorher 90) räumt nach Alter auf, einmal beim
  Start und danach täglich (`humanitld/src/main.rs`, `purge_daily`).

Ein Alter begrenzt keine Menge. Ein Agent, der einen Tag lang große Antworten
zieht, füllt bis zu 180 Tage lang, bevor überhaupt etwas gelöscht wird; ein
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
- [x] `make check` grün. **Gemessen am 2026-09-11** mit `tools/verify-commit.sh` über `24af9af` (enthält `02b4fe7`), dieselben Schritte wie die CI; CI-Lauf 34585656949 grün.

### Fallstricke
- `Ipv6Addr::to_ipv4` entpackt NAT64 nicht; das Entpacken ist eigener Code und gehört getestet.
- Ein öffentliches Ziel darf nicht versehentlich privat werden: `64:ff9b::808:808` (`8.8.8.8`) bleibt erreichbar.

### Referenzen
Proxy-Analyse 2026-09-11; RFC 6052, RFC 8215, RFC 3056, RFC 4380; IANA IPv4 und IPv6 Special-Purpose Address Registry; `daemon/crates/core-types/src/host.rs:192-225`; `docs/SECURITY.md` Abschnitt 6 (Regeln und ihre Fallen).

---

## HUM-147 · verify-commit baut inkrementell, die CI nicht
Sprint: 5 · Größe: S · Abhängigkeiten: — · Blockiert: nichts; es hält die Platte des Entwicklungsrechners klein

> **Aufgegangen in HUM-237** (`backlog/sprint-4.md`, 2026-10-16). Die Kriterien unten waren schon am 2026-09-11 erfüllt und gemessen; HUM-237 hat sie gegen den heutigen Stand von `tools/verify-commit.sh` nachgeprüft (`CARGO_INCREMENTAL=0` in `step()`, Löschen von `*/incremental` unter der Sperre des Zielverzeichnisses) und den Satz zum Zielverzeichnis in `CONTRIBUTING.md` ergänzt. Mit HUM-237 ist `tools/verify-commit.sh` freiwillig; das Tor nach `main` ist die CI auf dem Issue-Zweig.

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
- [x] `grep -n "CARGO_INCREMENTAL=0" tools/verify-commit.sh` trifft an der Stelle, an der die Schritte laufen. **Gemessen am 2026-09-11**: Zeile 242, in `step()`.
- [x] Nach einem Lauf gibt es unter `~/.cache/humanitl/verify-target` kein `incremental`-Verzeichnis mehr; die Größe vorher und nachher steht im Commit-Body. **Gemessen am 2026-09-11, mit zwei Einschränkungen, die hier stehen statt im Commit-Body:** Die Zahl nachher entsteht erst durch den Lauf über den Commit selbst und steht deshalb hier: vorher 62 GB, davon 30 GB in `debug/incremental` mit 832 Einträgen (Commit-Body `18dcb14`), nachher 36 GB. Und Cargo legt `debug/incremental` und `shim/incremental` auch mit `CARGO_INCREMENTAL=0` wieder an, leer: je 4 KB, 0 Einträge.
- [x] `tools/verify-commit.sh HEAD` ist grün. **Gemessen am 2026-09-11** über `24af9af`.

### Fallstricke
- Das Skript darf nicht geändert werden, während ein verify läuft: bash liest ein Skript beim Ausführen nach.
- Gelöscht wird nur unter dem gehaltenen Lock, sonst nimmt man einem parallelen Lauf den Cache unter den Füßen weg.

### Referenzen
Messung am 2026-09-11; `.github/workflows/ci.yml:43`; `tools/verify-commit.sh:59`, `:220`; Nutzervorgabe zur Plattensparsamkeit.

---

## HUM-152 · Der M3-Lauf tippt nicht ins TUI des echten Agenten
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-141 · Blockiert: nichts; der Werkzeugaufruf selbst ist seit HUM-141 gemessen

### Kontext
HUM-141 misst den Werkzeugaufruf des echten Agenten kopflos: Der M3-Lauf gibt OpenCode die Frage als Argument von `opencode run`, der Agent ruft `webfetch`, die Regel des Profils antwortet mit `403`, und das steht im Transkript des Agenten. Die Spezifikation von HUM-141 sah dafür das Tippen in das laufende TUI vor, über `humanitl sandbox attach`, weil `humanitl run` ausdrücklich keine Taste weiterreicht (`daemon/bin/humanitl/src/cmd/run.rs`, Kopf). Das ist nicht gebaut. Gemessen ist nur der Weg: `sandbox attach` reicht die Standardeingabe an das Pseudoterminal weiter (am 2026-09-07 mit `/bin/sh`), und OpenCode nimmt Zeichen in den ersten Sekunden nach seinem ersten Bild nicht an. Wie lange es braucht, ist nicht gemessen.

### Ziel
Der TUI-Lauf in Schritt 13 bekommt dieselbe Frage getippt (`humanitl-webfetch https://humanitl-probe.example/page`), und im Transkript des TUI steht der gescheiterte Aufruf mit dem `403` der Regel.

### Nicht-Ziel
Eine Änderung an `humanitl run`. Ein Mock, der mehr kann als in HUM-141.

### Betroffene Pfade
- `tests/e2e/m3_agent_inside/run.sh` (TUI-Lauf in Schritt 13)

### Spezifikation
Nach dem ersten Bild wartet der Lauf eine Frist, die gemessen und als Zahl ins Skript geschrieben ist, nicht geraten. Dann tippt er über `humanitl sandbox attach` die Frage samt Eingabetaste, wartet, bis `/_debug/tool` des Mocks die Antwort des Werkzeugs zeigt, und sucht im TUI-Transkript die Zeile des gescheiterten Aufrufs, ohne die Zeile, mit der der Mock die Antwort wiederholt.

### Akzeptanzkriterien
- [ ] Die Frist, bis OpenCode Zeichen annimmt, ist gemessen und steht als Zahl mit ihrer Messung im Lauf.
- [ ] Im Transkript des TUI-Laufs steht der gescheiterte `webfetch` mit dem `403` der Profilregel, und die Zahl der Zusicherungen des OpenCode-Zweigs steigt entsprechend.

### Referenzen
HUM-141, HUM-067 (`backlog/sprint-3.md`); `tests/e2e/m3_agent_inside/run.sh` Schritt 13; Messungen vom 2026-09-07 und 2026-09-11.

## HUM-155 · Der Audit-Schreiber lebt im Kern
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-050 · Blockiert: nichts

### Kontext
`docs/ARCHITECTURE.md` 2 zählt `audit` zum Kern: kein IO, kein async, kein Proto. HUM-050 hat den Schreiber der Kette dorthin gelegt, wie seine Spezifikation es verlangt (`daemon/crates/audit/src/lib.rs`: `AuditWriter`), und damit Datei, `flock`, `fsync`, die Schlüsseldatei, einen Schreib-Thread und einen `tokio::sync::broadcast`-Kanal in eine Kern-Crate. Das Review von Codex hat es als Befund gemeldet; es ist zutreffend, ging aber nicht mehr in den Commit von HUM-050, weil der Umbau die Crate-Grenzen und `tools/check-deps.sh` berührt. `docs/ARCHITECTURE.md` nennt die Abweichung seither ausdrücklich.

### Ziel
`humanitl-audit` enthält nur noch Kanonisierung, Record, Arten und die Prüfung über einen Leser. Der Schreiber (`writer.rs`, die Datei-Seite von `key.rs`, `AuditVerifier::verify` über einen Pfad) lebt in einem Adapter außen, und `tools/check-deps.sh` verbietet `std::fs` und `tokio` im Kern.

### Nicht-Ziel
Ein neuer Port. Der Schreiber bleibt der einzige Adapter, und die Aufrufer im Daemon ändern nur ihren Import.

### Betroffene Pfade
- `daemon/crates/audit/src/` (Kern)
- ein Adapter-Crate oder `daemon/crates/recorder/`, je nachdem, was `backlog/CONVENTIONS.md` 3.1 zulässt
- `tools/check-deps.sh`, `docs/ARCHITECTURE.md`

### Akzeptanzkriterien
- [ ] `humanitl-audit` hängt weder von `tokio` noch von `rustix` oder `libc` ab, und kein Modul darin öffnet eine Datei.
- [ ] `tools/check-deps.sh` meldet es, wenn eine Kern-Crate wieder eine dieser Abhängigkeiten bekommt; nachgewiesen durch eine Mutation.
- [ ] Die Tests aus HUM-050 laufen unverändert grün, und der Satz über die Abweichung in `docs/ARCHITECTURE.md` ist gestrichen.

### Referenzen
HUM-050 (`backlog/sprint-4.md`), Review von Codex vom 2026-09-11; `docs/ARCHITECTURE.md` 1 und 2.

## HUM-247 · Der Findings-Scan liest nur das erste gzip-Member
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-025 · Blockiert: nichts

### Kontext
`inflate` in `daemon/crates/findings/src/decode.rs` entpackt `Content-Encoding: gzip` mit `flate2::read::GzDecoder`. Dieser Leser hört nach dem ersten gzip-Member auf und meldet das Ende des Stroms; RFC 1952 erlaubt aber beliebig viele Member hintereinander, und verbreitete Entpacker auf der Gegenseite (Go `compress/gzip`, `zlib` mit `inflateReset`, `gzip -d`) lesen sie alle. Ein unterwanderter Agent kann so zwei verkettete gzip-Ströme schicken, das Geheimnis im zweiten. Der Scan sieht nur das erste Member, hält den Body für vollständig durchsucht (`truncated: false`, kein `FINDINGS_002`), und das Ziel entpackt den ganzen Body.

Das ist genau die stille Lücke, die `backlog/CONVENTIONS.md` 4.13 und `docs/SECURITY.md` („der Scan meldet eine Lücke, `FINDINGS_002`") ausschließen: Die Funde sind keine Garantie, aber wo der Scan nicht hinsieht, muss er es sagen. Die drei Garantien aus `README.md` berührt der Fall nicht; er betrifft die Aussage, dass ein nicht durchsuchter Teil des Bodys sichtbar gemacht wird. Gefunden beim Lesen des Decoders in HUM-056, nicht vom Fuzzer.

### Ziel
Ein gzip-Body wird über alle Member entpackt, durch dasselbe Budget aus `limits.preview_cap_bytes` und `limits.max_decompress_ratio`. Daten hinter dem Ende eines Stroms, die kein weiteres Member sind, bleiben nicht still liegen, sondern führen zu `FINDINGS_002`.

### Nicht-Ziel
Ein Entpacker für `zstd` (heute `ContentEncoding::Other`, also nicht durchsucht und mit `FINDINGS_002` gemeldet; dort gibt es keine stille Lücke).

### Betroffene Pfade
- `daemon/crates/findings/src/decode.rs` (`inflate`, `read_limited`)
- `daemon/crates/findings/tests/regressions/` (Regressionseingabe für `body_decoder`)

### Spezifikation
- `ContentEncoding::Gzip` liest mit `flate2::read::MultiGzDecoder`. Cap und Ratio-Budget gelten über alle Member zusammen, nicht je Member; `read_limited` bleibt der eine Weg.
- Die anderen Kodierungen werden mitgeprüft, weil sie dieselbe Frage haben:
  - **deflate** (`zlib`-Rahmen oder nackter Strom) kennt keine Member; der Strom endet mit dem letzten Block. Bytes dahinter ignoriert `ZlibDecoder`/`DeflateDecoder` heute still. Sie werden als Lücke gemeldet (`FINDINGS_002`), weil ein nachsichtiger Entpacker auf der Gegenseite sie anders deuten kann.
  - **br** kennt ebenfalls nur einen Strom. Was `brotli::Decompressor` mit Bytes hinter dem Ende macht, wird mit einem Test festgestellt; ignoriert er sie, gilt dasselbe wie bei deflate.
  - **zstd** erlaubt nach RFC 8878 mehrere verkettete Frames und überspringbare Frames, und ein Entpacker muss alle lesen. Solange `zstd` nicht entpackt wird, ist das kein Fall; das Issue, das einen `zstd`-Entpacker einführt, muss alle Frames lesen und bekommt den Test aus diesem Issue mit.

### Tests
- `gzip_second_member_is_scanned`: zwei verkettete gzip-Member, ein IBAN im zweiten. `decode_body` liefert beide Klartexte, `truncated: false`; der Scan meldet den Fund. Mutation: zurück auf `GzDecoder` macht den Test rot.
- `gzip_members_share_the_budget`: zwei Member, zusammen über dem Ratio-Budget, jedes für sich darunter: `truncated: true`, `FINDINGS_002`.
- `trailing_bytes_after_deflate_are_a_gap` und dasselbe für `br`: ein gültiger Strom plus angehängte Bytes ergibt `FINDINGS_002`.
- Die Eingabe aus dem ersten Test liegt als `body_decoder-gzip-two-members.bin` unter `tests/regressions/` und im Corpus von `body_decoder`.

### Akzeptanzkriterien
- [ ] Ein Geheimnis im zweiten gzip-Member wird gefunden; Mutationsbeweis mit `GzDecoder`.
- [ ] Cap und Ratio gelten über alle Member zusammen.
- [ ] Bytes hinter dem Ende eines deflate- oder br-Stroms führen zu `FINDINGS_002`, nie zu einem still als vollständig gemeldeten Body.
- [ ] `docs/SECURITY.md` nennt im Abschnitt zum Scan, dass mehrere gzip-Member gelesen werden.

### Referenzen
HUM-025 (`backlog/sprint-2.md`), HUM-056 (Abgleich oben), RFC 1952 §2.2, RFC 8878 §3.1, `backlog/CONVENTIONS.md` 4.13.

### Umsetzung (HUM-247)

**Was gebaut wurde.**
- `decode.rs` liest alle drei Kodierungen über die `bufread`-Entpacker von flate2 bzw.
  `brotli::Decompressor` auf dem Slice des Bodys. gzip läuft statt über `read::GzDecoder` über
  den neuen Leser `GzipMembers`: Er reiht `bufread::GzDecoder` Member für Member aneinander, wie
  es `MultiGzDecoder` intern tut, weiß aber zusätzlich, ob schon ein Member vollständig gelesen
  war, als ein Fehler kam (Abweichung von der Spezifikation, die `MultiGzDecoder` nennt; der
  Grund steht im gzip-Punkt unten). Alle Member laufen durch das eine `Budget` in
  `read_limited`; einen zweiten Weg gibt es nicht.
- Neuer privater Trait `StreamEnd` mit `has_trailing_input`, aufgerufen in `read_limited`, sobald
  der Strom mit `0` endet (auch am genau vollen Budget). Für flate2 ist das der Rest im Slice:
  Die `bufread`-Leser verbrauchen genau die Bytes des Stroms. Die Prüfung entpackt nichts und
  kann das Budget deshalb nicht umgehen; ein Strom, der genau endet, meldet nichts.
- Neue Variante `InflateError::TrailingBytes { decoded }`. `decode_body` durchsucht den
  entpackten Teil vor dem Ende, setzt `truncated` und meldet `FINDINGS_002` ohne Fix
  (`trailing_gap`). `GetBody` (`ipc/src/body.rs`) liefert in diesem Fall den entpackten Teil mit
  leerem `encoding_left`, also genau die Bytes, auf die die Fundstellen des Scans zeigen.
- deflate: Nur ein kaputter zlib-Rahmen (`BrokenStream`) führt zum zweiten Versuch mit dem
  nackten Strom. Ein sauberer Rahmen mit Bytes dahinter bleibt eine Lücke und wird nicht anders
  gedeutet.
- gzip: Bytes hinter dem letzten vollständigen Member, die kein Member sind (oder ein kaputtes
  späteres Member), führen ebenfalls zu `TrailingBytes`: Das Member davor wird durchsucht, der
  Rest ist die Lücke. `MultiGzDecoder` hätte hier nur den Fehler gemeldet, und der Scan hätte
  gar nichts gesehen; der alte `GzDecoder` hatte den Rest still ignoriert. Ein Fehler im ersten
  Member bleibt `BrokenStream`.

**Was der Test zu br festgestellt hat.** `brotli::Decompressor` meldet das Ende mit `Ok(0)`, auch
wenn dahinter Bytes liegen (`brotli_reader_ignores_trailing_bytes`); bis hierher war ein
angehängter Rest also eine stille Lücke. Erst ein zweites `read` meldet einen Rest im eigenen
Puffer als `InvalidData`; was der Leser noch nicht aus dem Slice geholt hat, steht in `get_ref`.
`has_trailing_input` prüft beides.

**Tests.** `gzip_second_member_is_scanned` (in `tests/scan_request.rs`, liest die
Regressionsdatei `body_decoder-gzip-two-members.bin`, die auch als `gzip-two-members` im Corpus
von `body_decoder` liegt), `gzip_members_share_the_budget`, `gzip_garbage_after_the_last_member_is_a_gap`,
`trailing_bytes_after_deflate_are_a_gap`, `trailing_bytes_after_br_are_a_gap`,
`trailing_bytes_behind_the_brotli_buffer_are_a_gap`, `trailing_bytes_are_seen_at_an_exactly_full_budget`;
`a_gzip_member_with_junk_behind_it_is_scanned_and_reported` (Scan findet die IBAN im Member,
meldet `FINDINGS_002`), in `ipc/src/body.rs` `bytes_behind_the_end_of_the_stream_leave_the_unpacked_part`;
`deflate_is_read_with_and_without_the_zlib_frame` und `a_brotli_body_is_decoded_whole` prüfen
jetzt auch, dass ein genau endender Strom keinen Befund trägt.

**Abweichung von der Spezifikation.** `gzip_members_share_the_budget` erwartet bei zwei Membern
über dem Ratio-Budget `LIMIT_003`, nicht `FINDINGS_002`: Seit HUM-057 meldet das Entpacken so
den Abbruch am Verhältnis. `FINDINGS_002` prüft derselbe Test für den Fall, dass das Cap die
engere Grenze ist.

**Mutationsbeweise.**
- `GzipMembers` liest nur das erste Member, wie `GzDecoder` (nach dem ersten Member immer
  Ende): `gzip_second_member_is_scanned` rot (der zweite Klartext fehlt), ebenso
  `gzip_members_share_the_budget`.
- Fehler nach einem vollständigen Member wieder als Fehler durchgereicht, wie bei
  `MultiGzDecoder`: `gzip_garbage_after_the_last_member_is_a_gap`,
  `a_gzip_member_with_junk_behind_it_is_scanned_and_reported` und
  `bytes_behind_the_end_of_the_stream_leave_the_unpacked_part` rot.
- Prüfung auf den Rest entfernt (`finish` ohne `has_trailing_input`):
  `trailing_bytes_after_deflate_are_a_gap`, `trailing_bytes_after_br_are_a_gap`,
  `trailing_bytes_behind_the_brotli_buffer_are_a_gap` und
  `trailing_bytes_are_seen_at_an_exactly_full_budget` rot.
- br nur mit `get_ref`, ohne zweites `read`: `trailing_bytes_after_br_are_a_gap` und
  `trailing_bytes_are_seen_at_an_exactly_full_budget` rot. br nur mit zweitem `read`, ohne
  `get_ref`: `trailing_bytes_behind_the_brotli_buffer_are_a_gap` rot.
- Ein Budget je Member (jedes Member mit einer frischen Kopie des Budgets entpackt):
  `gzip_members_share_the_budget` rot, `gzip_second_member_is_scanned` bleibt grün.

## HUM-248 · Nächtliche Fuzz-Funde selbst minimieren und den Corpus behalten
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-056 · Blockiert: nichts · Priorität: niedrig (Kandidat)

### Kontext
Zwei Vorschläge aus dem Review von Antigravity zu HUM-056, außerhalb seines Umfangs. Erstens endet ein roter Job von `fuzz-nightly` heute mit einem Artefakt, und der Mensch fängt mit Schritt 1 und 2 der Triage (`daemon/fuzz/README.md`) von Hand an. Zweitens beginnt jeder nächtliche Lauf wieder bei den eingecheckten Seeds: Was libFuzzer in zehn Minuten an neuer Abdeckung findet, ist am nächsten Morgen weg.

### Ziel
- Ein roter Matrix-Job ruft `cargo fuzz tmin` auf die auslösende Eingabe auf, legt die minimierte Datei mit ins Artefakt und eröffnet ein GitHub-Issue mit Target, Fehlerausgabe und Link zum Lauf (einmal je Target und Ursache, nicht je Nacht).
- Der gewachsene Corpus jedes Targets wird zwischen den Läufen über `actions/cache` behalten, mit Obergrenze und Ablauf (`cargo fuzz cmin` vor dem Speichern), damit er nicht unbegrenzt wächst. Eingecheckt bleiben nur die Seeds.

### Akzeptanzkriterien
- [ ] Ein absichtlich eingebauter Absturz erzeugt ein Issue mit minimierter Eingabe; ein zweiter Lauf mit demselben Absturz erzeugt kein zweites.
- [ ] Ein Lauf beginnt mit dem Corpus des vorigen; der Cache bleibt unter einer im Workflow genannten Größe.

### Referenzen
HUM-056, `.github/workflows/fuzz-nightly.yml`, Review von Antigravity zu HUM-056.

---

## HUM-250 · Ein Ausfall der Aufzeichnung bleibt in der Oberfläche unsichtbar
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-026, HUM-058 · Blockiert: HUM-060 (vor HUM-060 zu erledigen)

### Kontext
HUM-058, Zeile 20: Ist die Platte voll oder die Datenbank gesperrt, schreibt der Recorder den Befund nur ins Log (`daemon/crates/recorder/src/writer.rs`, `report`, `tracing::warn!`). Kein Client erfährt davon. Die Oberfläche sagt weiter „alles wird aufgezeichnet“, während nichts mehr ankommt; die Sicherheitsaussage in `README.md` hängt an dieser Aufzeichnung (CONVENTIONS 4.10, letzter Punkt).

### Ziel
Ein Fehler der Aufzeichnung erreicht jeden Client als `FlowEvent.Diagnostic` mit `RECORDER_001` (öffnen) beziehungsweise `RECORDER_003` (schreiben), höchstens einmal je Ursache, und die Oberfläche zeigt ihn: Statusleiste rot „Aufzeichnung gestört“, jede neue Karte trägt den Hinweis „wird nicht aufgezeichnet“, und die Freigabe bekommt einmal je Sitzung eine Bestätigungs-Pause (kein Modal, `docs/UX.md` 5.4). Kehrt die Aufzeichnung zurück, sagt ein zweiter Befund das, und die Hinweise verschwinden.

### Nicht-Ziel
Ein Puffer, der verlorene Flows nachträglich schreibt.

### Dazu (aus dem Review von HUM-058, Runde 3)
`GetFlow` liefert für einen gehaltenen Flow die Anfrage aus der Live-Registry des Proxys, bevor es auf eine unvollständige Aufzeichnung zurückfällt. Eine aus der Aufzeichnung nachgebaute Anfrage ohne Kopfzeilen verschweigt sonst ein `Upgrade: websocket` der gehaltenen. Die Oberfläche hält eine solche Anfrage seit HUM-058 auf allen drei Wegen gehalten (`requestIsReadable` in `app/lib/core/domain/websocket.dart`); mit diesem Punkt wird sie wieder entscheidbar.

### Akzeptanzkriterien
- [ ] Daemon: ein Test, der den Schreiber scheitern lässt und genau ein `FlowEvent.Diagnostic` mit `RECORDER_003` auf dem Strom sieht.
- [ ] Oberfläche: `status_bar_test: recorder_failure_marks_cards` (Statusleiste, Hinweis auf neuen Karten, einmalige Pause) und ein Golden `states_recorder_failure` in vier Varianten.
- [ ] Mutationsbeweise für beide Hälften.

### Referenzen
HUM-058 (Zeile 20 und Umsetzung), HUM-026, `docs/SECURITY.md`.

## HUM-251 · Ein unerreichbares Modell färbt die Zeile „LLM“ nicht
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-039, HUM-058 · Blockiert: nichts

### Kontext
HUM-058, Zeile 15: Scheitert die Durchreiche zum Modell an der Verbindung, entsteht ein `Failed`-Flow (CONVENTIONS 4.10), aber kein Befund `LLM_001`. Die Zeile „LLM“ im Isolation-Panel bleibt eine neutrale Ausnahme-Zeile. Sie aus den Flows abzuleiten wäre Fachlogik in der Oberfläche (ADR-018).

### Ziel
Der Daemon meldet ein im Lauf unerreichbares Modell als `LLM_001` mit Fix `ChangeSetting{llm.endpoint}` am Sandbox-Status, und die Zeile „LLM“ wird rot, bis eine Durchreiche wieder gelingt.

### Akzeptanzkriterien
- [ ] Daemon-Test: Connect-Fehler der Durchreiche setzt `LLM_001` am Status, ein Erfolg nimmt ihn weg.
- [ ] `sandbox_screen_test: llm_unreachable_row` mit Mutationsbeweis.

### Referenzen
HUM-058 (Zeile 15), HUM-039, HUM-041.

## HUM-252 · Eine neue Regel, die nie an die Reihe kommt, wird nicht gemeldet
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-033, HUM-058 · Blockiert: nichts

### Kontext
HUM-058, Zeile 17: Das Regel-Blatt warnt nicht, wenn eine frühere Regel jeden Fall der neuen schon fängt. Der Dry-Run zählt nur die Treffer der einen Regel. Die Prüfung „überdeckt“ gehört in den Daemon, der die Reihenfolge der Ränge kennt (CONVENTIONS 4.5).

### Ziel
`Rules(dry_run)` meldet für die neue Regel an ihrer Position, ob eine frühere Regel jeden erzeugten Testfall fängt, und nennt sie; das Blatt zeigt vor dem Speichern amber „Wird von Regel #3 (`block **.example.com`) überdeckt“, Speichern bleibt erlaubt. Der Befund bekommt einen eigenen Code im Register (`RULES_001` heißt dort „rules.yaml unlesbar“). Heuristik, kein Beweis: Bei einem einzigen nicht gefangenen Testfall schweigt die Warnung.

### Akzeptanzkriterien
- [ ] Daemon-Test über die Ränge, `rule_sheet_test: shadowed_rule_warning`, Mutationsbeweise.

### Referenzen
HUM-058 (Zeile 17, Fallstrick zur Heuristik), HUM-033, HUM-104.

## HUM-253 · Folgen aus HUM-058: Protokollwechsel im Vertrag, Drosselung
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-058 · Blockiert: nichts

### Kontext
Kleine Reste aus HUM-058, gebündelt (die letzten beiden Kriterien kamen aus Runde 3 des Reviews):

1. **`FlowSummary.upgrade` im Vertrag.** Die Oberfläche spiegelt heute `requested_upgrade` aus `daemon/crates/proxy/src/connect.rs` über die Kopfzeilen des Details (`app/lib/features/intercept/websocket.dart`), und die Freigabe wartet deshalb auf `GetFlow`. Mit dem Feld in der Zeile entfällt der Spiegel, und die Karte warnt ab der ersten Zeile.
2. **Drosselung des Streaming-Zählers auf 10 Hz** (Fallstrick zu Zeile 12): Heute faltet `HistoryPageNotifier` jedes `ResponseChunk` sofort in die Zeile.
3. ~~**Die Freigabe aus der Benachrichtigung** (HUM-034) entscheidet ohne die Karte, also auch über einen WebSocket-Aufbau, dessen Warnung niemand gesehen hat.~~ *Erledigt in HUM-058 (2026-10-19): Der Knopf fragt beim Druck nach dem Detail und gibt nur frei, was nachweislich keinen Wechsel verlangt; sonst kommt das Fenster auf der Anfrage nach vorn (`tray_websocket_test`). Mit Punkt 1 kann die Meldung den Knopf später gleich weglassen.*

### Akzeptanzkriterien
- [ ] Proto-Feld mit Minor-Erhöhung, Spiegel in der Oberfläche entfernt, Test der Karte ohne Detail.
- [ ] Drosselung mit Test über viele Chunks in kurzer Zeit.
- [ ] Eine Regel aus einer WebSocket-Anfrage trägt kein `upgrade` und trifft sie deshalb nie (`daemon/crates/rules/src/eval.rs`, Zeile 378 zum Stand vom 2026-10-19): Der Regelvorschlag aus Raster, Gruppenkopf und „Als Regel übernehmen“ übernimmt den Protokollwechsel der Anfrage, wie es `PROXY_008` schon tut (CONVENTIONS 4.10). Kandidat aus dem Review von HUM-058.
- [ ] Randfall leerer `Content-Type`: Der Daemon durchsucht einen Rumpf mit vorhandener, aber leerer Kopfzeile im Modus `printable_only`, die Oberfläche (`scannedAsStrings`) prüft dann wie ohne Kopfzeile auf UTF-8. Angleichen, damit der Satz über den Scan stimmt.
- [ ] „Allow all“ fragt das Detail jeder Anfrage zugleich ab (`_holds` in `decision.dart`); bei einer langen Warteschlange fehlt eine Obergrenze für die gleichzeitigen `GetFlow`. Kandidat aus Runde 4 des Reviews von HUM-058.
- [ ] `TimeoutRuleState.failure` hält nur einen Befund; scheitern zwei Regeln nacheinander, verliert die erste Anfrage ihren Grund. Kandidat aus Runde 4 des Reviews von HUM-058.
- [ ] Bricht ein Stapel in `_abortAfter` ab, fehlt in der Leiste der Satz zu den Anfragen, die er zurückgehalten hatte. Kandidat aus Runde 5 des Reviews von HUM-058.
- [ ] `PROXY_003` mit unbekannter Stelle (HUM-257) erscheint je Fluss und ohne Drosselung; ein
  Agent, der dasselbe Ziel mit fremder CA in einer Schleife fragt, füllt den Strom der Befunde.
  Zusammenfassen wie `PressureWatch` bei `PROXY_011`. Kandidat aus dem Review von HUM-257.
- [x] ~~Benachrichtigung ohne Freigabe für einen WebSocket-Aufbau, Test in `attention_test`.~~ *Erledigt in HUM-058, Test `tray_websocket_test: a_notification_allows_no_websocket_unseen`.*

### Referenzen
HUM-058 (Zeilen 11 und 12, Review vom 2026-10-19), HUM-034, HUM-091.

## HUM-249 · Ein Tastenkürzel speichert eine andere Regel, als die Leiste zeigt
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-028, HUM-058 · Blockiert: HUM-060 (vor HUM-060 zu erledigen)

### Kontext
Aus Runde 6 des Reviews von HUM-058 (Codex). Die Aktionsleiste zeigt den Satz der Regel für die Aktion, die gerade hervorgehoben ist (`app/lib/features/intercept/widgets/action_bar.dart`, `blockHighlighted`). Ein Tastenkürzel kann aber die andere Aktion auslösen: Steht die Maus über „Blockieren“, heißt der Satz „block …“, und ein Druck auf `A` speichert eine Allow-Regel, die niemand gelesen hat. Vor „Merken“ soll die Regel als Satz stehen, damit sie geprüft werden kann (`docs/UX.md` 4.6). Das Verhalten war schon vor HUM-058 so; HUM-058 hat nur den Weg über mehrere Anfragen geschlossen (`ShownRule`).

### Ziel
Keine Entscheidung speichert eine Regel, deren Satz nicht auf dem Schirm stand. Mögliche Wege, im Issue zu entscheiden:
- beide Sätze zeigen, den für Senden und den für Blockieren;
- ein Kürzel, das eine Regel anlegen würde, zeigt zuerst ihren Satz und verlangt einen zweiten Druck;
- ein neutraler Satz ohne Aktion, und die Aktion steht am Knopf.

### Akzeptanzkriterien
- [ ] Widget-Test: Raster offen mit Gültigkeit Session, Maus über „Blockieren“ (der Satz sagt „block …“), Druck auf `A`. Erwartet: Es wird keine Allow-Regel gespeichert, deren Satz nicht stand, oder der Satz der Allow-Regel stand vor dem Speichern sichtbar da. Mit Mutationsbeweis.
- [ ] Dasselbe umgekehrt: Fokus auf dem Ventil, Satz „allow …“, Kürzel für Blockieren.
- [ ] `docs/UX.md` 4.6 nennt den gewählten Weg.

### Referenzen
HUM-058 (Review Runde 6), HUM-028, `docs/UX.md` 4.6 und 5.4.

---

## HUM-258 · Tröpfelnde Uploads hungern das Byte-Budget aus
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-057 · Blockiert: — · nach 0.1.0

### Kontext
Seit HUM-057 reserviert der Handler jeden Anfrage-Body schon beim Puffern im Byte-Budget
`limits.hold_max_bytes` (`HoldQueue::in_flight`, `InFlight::grow`). Gegen den Speicher ist das dicht,
aber `limits.body_timeout_secs` misst nur die Stille zwischen zwei Stücken, nie die Gesamtdauer
(`backlog/CONVENTIONS.md` 4.4). Ein Client, der knapp unter der Frist ein Byte nachschiebt, hält
seinen Anteil beliebig lange. Acht Verbindungen mit je knapp 32 MiB füllen die Vorgabe von 256 MiB,
und jede weitere Anfrage mit Body bekommt `503` mit `LIMIT_002`. Das Ergebnis ist fail-closed (kein
Egress), und der Angreifer ist der eigene Agent in der Sandbox; trotzdem legt er so den Proxy für
alle anderen Anfragen mit Body lahm. Benannt in `docs/SECURITY.md` (Grenzen und Zeitlimits);
herausgelöst aus Review 2 zu HUM-057.

### Ziel
Ein einzelner langsamer Client kann das Byte-Budget nicht dauerhaft belegen, ohne dass ein langer,
stetiger Upload über eine langsame Leitung dabei abbricht.

### Zwei Wege (einer wird gewählt, die Entscheidung steht im Commit)
1. **Gesamtfrist für das Puffern bis zum Cap.** Ein Schlüssel wie
   `limits.client_body_timeout_secs` (Vorgabe etwa 120), der die Zeit vom ersten Byte bis zum
   vollständigen Body begrenzt, solange er gepuffert wird; gestreamte Bodies (`stream: true`)
   bleiben bei der Stille-Frist. Verlangt den Abgleich in `backlog/CONVENTIONS.md` 4.4 (dort ist
   der Name mit HUM-120 entfallen) und ein Kriterium „zwölf Felder" statt „elf".
2. **Mindestrate oder Anteil je Verbindung, ohne neues Feld.** Entweder muss das Puffern nach einer
   Anlaufzeit im Mittel eine feste Mindestrate halten (etwa `hold_body_cap_bytes / body_timeout`),
   oder eine Verbindung darf höchstens einen festen Bruchteil von `limits.hold_max_bytes` belegen
   (etwa ein Viertel), sodass auch bei `max_client_connections` gleichzeitigen Tröpflern Platz für
   andere bleibt. Kein Abgleich in CONVENTIONS, aber eine Konstante, die im Schema nicht sichtbar
   ist und deshalb in `docs/reference/limits.md` steht.

### Tests
- `trickling_uploads_cannot_starve_the_budget`: Acht Clients schicken je knapp den Cap an
  Ankündigung und danach ein Byte je `body_timeout - 1 s` (im Test kleine Werte, `start_paused` wo
  möglich). Erwartung: Nach der Gesamtfrist bzw. bei Unterschreiten der Rate ist ihr Anteil frei
  (`queue_bytes() == 0`), und eine neunte Anfrage mit Body wird gehalten statt mit `503` geblockt.
- `a_steady_slow_upload_still_completes`: Ein Upload mit konstanter, niedriger Rate über mehrere
  Fristen läuft durch (Gegenprobe gegen eine zu scharfe Grenze).
- Mutation: Grenze entfernt, der erste Test wird rot; Grenze verschärft, der zweite wird rot.

### Akzeptanzkriterien
- [ ] Beide Tests grün, Mutationsbeweis im Commit.
- [ ] `docs/SECURITY.md` beschreibt die neue Grenze statt des Tauschs „begrenzter Speicher gegen
  Aushungern".
- [ ] Bei Weg 1: CONVENTIONS 4.4, Schema-Schnappschuss, `docs/CONFIG.md` und das Kriterium der
  Feldzahl in HUM-057 im selben Commit angepasst.

### Weitere Punkte (aus Review 3 zu HUM-057)
- **Laufende Belegung für die Oberfläche.** Die Statusleiste kennt die Belegung des Byte-Budgets
  nur aus dem letzten `Held.queue_bytes`; nach einer Entscheidung, nach dem Ende eines Puffervorgangs
  und bei reinem Puffern ohne gehaltene Flüsse veraltet sie. Lösung: die Belegung laufend melden
  (eigenes Ereignis oder eine RPC) und zusätzlich in `Decided` mitgeben; die Statusleiste nimmt dann
  diesen Wert statt des letzten `Held`.
- **Gepufferter Body nach einer frühen Antwort.** Auf dem gepufferten Pfad fällt `in_flight`, wenn
  der Handler mit den Antwort-Kopfzeilen zurückkehrt (`FlowHandler::handle_request`). Antwortet ein
  Ziel früh und liest den gepufferten Body nie, hält die Verbindung zum Ziel bis zu
  `limits.hold_body_cap_bytes` je Verbindung außerhalb des Budgets und ohne Wache. Lösung wie im
  Stream-Fall aus HUM-057: Der Anteil wandert in den ausgehenden Body und fällt mit ihm, und eine
  Wache bricht die Verbindung ab, wenn das Ziel länger als `limits.body_timeout_secs` nicht liest.

### Referenzen
HUM-057 (Umsetzung, Review 2 und 3), HUM-120, `backlog/CONVENTIONS.md` 4.4, `docs/SECURITY.md`.

---

## HUM-254 · Die Seitenleiste klappt auf und zu
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-019, HUM-054 · Blockiert: nichts

### Kontext
Wunsch des Projekteigentümers vom 2026-10-20. Die Icon-Rail links (`app/lib/features/shell/widgets/icon_rail.dart`, HUM-019) ist eine feste Leiste von 48 px (`railWidth`): nur Glyphen, der Name eines Bereichs erscheint erst, wenn der Zeiger `HMotion.hoverLabel` lang auf dem Eintrag ruht (`HoverLabel`), und das Tastenkürzel steht nur in diesem Hover-Text. Wer die Anwendung neu kennenlernt, muss sechs Glyphen einzeln abfahren. Die Leiste soll sich deshalb aufklappen lassen und dann Name und Kürzel jedes Bereichs dauerhaft zeigen, animiert auf und zu.

Ausgangslage, die die Spezifikation bindet:
- Bewegung läuft über `HMotion` und `HReducedMotion` in `app/packages/ui/lib/src/tokens/motion.dart`; kein Widget entscheidet selbst, ob es sich bewegen darf (`docs/UX.md` 2.2 und 2.10). Die nächste Zeile der Tabelle in 2.2 ist „Abschnitt oder Gruppe klappt auf": 180 ms, `enter`, `HMotion.arrive`, Höhe.
- Für Oberflächenzustand, der einen Neustart überleben soll, gibt es seit HUM-044 genau einen Ort: `UiStateFile` in `app/lib/features/intercept/providers/coach_mark.dart`, eine JSON-Datei unter `$XDG_STATE_HOME/humanitl/ui-state.json`. `shared_preferences` ist keine Abhängigkeit (siehe `pane_layout.dart`) und kommt auch nicht dazu.
- `Ctrl+B` ist nicht frei. `backlog/CONVENTIONS.md` 3.9 reserviert es für `AddRuleIntent` (offen in HUM-243), und `B` allein blockiert in der Warteschlange: Ein `Ctrl+B`, bei dem die Strg-Taste einen Hauch zu spät kommt, blockierte eine gehaltene Anfrage, statt die Leiste umzuschalten. Neben einer Entscheidungstaste liegt keine Layout-Taste.

### Ziel
- Die Leiste hat zwei Zustände: **eingeklappt** (Vorgabe, wie heute 48 px, nur Glyphen, Name per `HoverLabel`) und **ausgeklappt** (200 px, `railExpandedWidth`): Glyph, Name des Bereichs und dezent rechtsbündig sein Kürzel (`Ctrl+1` bzw. `Strg+1`). Der aktive Bereich ist in beiden Zuständen gleich markiert (Füllung `bg3`, Akzent-Rail `HSize.selectionRail` am linken Rand).
- Umgeschaltet wird über einen Knopf am unteren Ende der Leiste (Chevron, zeigt nach rechts zum Aufklappen und nach links zum Zuklappen, mit Hover-Label und Semantik-Label samt Kürzel), über `Ctrl+0` und über einen Befehl der Command Palette („Seitenleiste ausklappen" bzw. „einklappen"; die Palette ist das Handbuch, `docs/UX.md` 5.1).
- Der Zustand überlebt einen Neustart, im selben `UiStateFile` unter dem Schlüssel `shellRailExpanded`.

### Nicht-Ziel
- Kein Ziehen der Breite mit der Maus, keine dritte Breite, kein automatisches Ein- oder Ausklappen nach Fensterbreite.
- Kein Umzug von `UiStateFile` nach `core/`. Der Speicher liegt heute in einer Feature-Datei der Warteschlange, und die Shell ist die eine Stelle, die Features zusammensetzen darf (`docs/ARCHITECTURE.md` 5). Der Umzug ist ein eigenes Issue, sobald ein zweites Feature außer der Shell ihn braucht.
- `Ctrl+B` und `AddRuleIntent` bleiben bei HUM-243.

### Betroffene Pfade
- `app/lib/features/shell/widgets/icon_rail.dart` (Breite, Beschriftung, Umschaltknopf)
- `app/lib/features/shell/providers/rail.dart` (neu: `railExpandedProvider`)
- `app/lib/features/shell/shell_screen.dart` (Provider lesen, `ToggleRailIntent` binden, Palettenbefehl)
- `app/lib/core/shortcuts/intents.dart` (`ToggleRailIntent`, `Ctrl+0` in `shellShortcuts`)
- `app/lib/features/intercept/providers/coach_mark.dart` (`UiStateFile.setBool`, `setFlag` delegiert dorthin)
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb` (nur angehängt)
- `docs/UX.md` 2.2 (eine Zeile der Bewegungstabelle), `backlog/CONVENTIONS.md` 3.9 (`ToggleRailIntent` Ctrl+0)
- `app/test/features/shell/icon_rail_test.dart` (neu), `app/test/core/shortcuts/shortcuts_map_to_intents_test.dart` (eine Zeile)
- `app/test/goldens/icon_rail_golden_test.dart` (neu), `app/test/goldens/support/golden_names.dart`, die neuen Bilder unter `app/test/goldens/goldens/ci/` und die bestehenden Bilder der ganzen App, die die Leiste zeigen (der Knopf kommt hinzu)

### Spezifikation
- **Breite.** `railWidth` (48) bleibt der eingeklappte Wert, `railExpandedWidth` (200) kommt daneben. Die Breite animiert über `HMotion.arrive` (180 ms), mit `HMotion.enter` beim Aufklappen und `HMotion.exit` beim Zuklappen. Das entspricht der Zeile „Abschnitt oder Gruppe klappt auf" in `docs/UX.md` 2.2 mit der Breite als Achse; die Tabelle bekommt dafür eine eigene Zeile „Seitenleiste klappt auf oder zu".
- **Beschriftung ohne Umbruch und ohne Überlauf.** Die Beschriftung (Name und Kürzel) wird in der Breite des ausgeklappten Zustands gesetzt und von der wachsenden Leiste nur aufgedeckt (`ClipRect` um eine feste innere Breite), nie in der Zwischenbreite neu umbrochen; sie ist einzeilig und kürzt mit Ellipse, falls ein Name länger ist als der Platz. Sie blendet über `HMotion.arrive` ein, sobald die Leiste die halbe Breite überschritten hat (`railLabelRevealWidth`), und beim Zuklappen über dieselbe Dauer aus, während die Leiste schrumpft. Das Kürzel nimmt höchstens die halbe Breite des Streifens ein und kürzt bei großer Textskalierung, statt den Namen hinauszuschieben. In keinem Frame der Animation entsteht ein `RenderFlex`-Überlauf.
- **Reduzierte Bewegung** (`docs/UX.md` 2.10: der Weg fällt weg, nicht die Rückmeldung). Unter `HReducedMotion.of(context)` springt die Breite im nächsten Frame auf den Endwert (`HReducedMotion.displace`), ohne Zwischenbreite. Das Einblenden der Beschriftung behält seine Dauer (`AnimationBehavior.preserve`), denn Einblenden ist Rückmeldung und kein Weg; dieselbe Regel wie beim Abgang einer Zeile, deren Kollaps entfällt und deren Ausblenden bleibt. Beim Zuklappen verschwindet die Beschriftung unter reduzierter Bewegung mit der Breite im selben Frame; die gesprungene Leiste ist dort die Rückmeldung.
- **Umschaltknopf.** Unten in der Leiste, 48 px hoch wie ein Eintrag, in der Glyph-Spalte, damit er dem Zeiger nach dem Klick nicht davonläuft; `HIconButton` mit `HGlyph.chevronRight`, ausgeklappt gespiegelt. Er ist mit `Tab` erreichbar und mit `Enter` und `Leertaste` auslösbar (`docs/UX.md` 5.1), trägt `Semantics(button: true, expanded: …)` mit dem Label „Seitenleiste ausklappen (Strg+0)" bzw. „einklappen" und dasselbe als `HoverLabel`.
- **Hover-Label der Einträge.** Eingeklappt wie heute. Ausgeklappt steht der Name schon in der Zeile; das `HoverLabel` entfällt dann, damit derselbe Text nicht zweimal nebeneinander steht. Das Semantik-Label bleibt in beiden Zuständen gleich.
- **Kürzel `Ctrl+0`.** `ToggleRailIntent` in `app/lib/core/shortcuts/intents.dart`, in `shellShortcuts` gebunden, Action in `ShellScreen`. Gründe: Es gehört zur Tastenfamilie der Leiste (`Ctrl+1` bis `Ctrl+6`), ist in keiner Tabelle gebunden, liegt auf deutscher und englischer Tastatur ohne Umschalttaste, und ohne Strg ist `0` in der Warteschlange ungebunden (`1` bis `4` sind `RememberDurationIntent`); ein verrutschter Griff entscheidet also nichts. `navigationKeys` endet damit spätestens bei `digit9`; die `0` ist kein künftiger zehnter Bereich.
- **Zustand.** `railExpandedProvider` (Riverpod, `keepAlive`) liest beim Start `UiStateFile.read()['shellRailExpanded'] == true` und schreibt bei jedem Umschalten mit `UiStateFile.setBool`. Ein Schreibfehler lässt den Zustand nur im Speicher, wie beim Hinweis aus HUM-044: Es ist eine Bequemlichkeit, und sie scheitert wie eine.
- **Texte.** Alles über ARB, `en` Quelle, `de` Übersetzung, nur angehängt: Label des Knopfs für beide Richtungen und die beiden Palettenbefehle. Das Kürzel kommt aus `shellNavShortcutKey(0)`, damit es im Deutschen `Strg` heißt.

### Tests
- `app/test/features/shell/icon_rail_test.dart`:
  - `toggle button expands and collapses the rail`: Breite 48, Tipp auf den Knopf, nach dem Ende der Animation 200, erneuter Tipp, wieder 48.
  - `Ctrl+0 and the palette toggle the rail in the shell`: in der ganzen App über `pumpApp`, Breite vorher und nachher, dazu der Palettenbefehl `toggle-rail`.
  - `labels show only when expanded`: eingeklappt steht kein Name als `Text` in der Leiste, ausgeklappt stehen alle sechs mit ihrem Kürzel.
  - `reduced motion: the width jumps, the names still fade`: unter reduzierter Bewegung der Plattform hat die Leiste nach einem einzigen `pump()` schon 200 px, kein Frame danach zeigt eine andere Breite, und die Beschriftung steht nach `HMotion.arrive / 2` zwischen 0 und 1.
  - `no overflow mid-animation`: in Schritten von 8 ms durch beide Richtungen pumpen, in Englisch und Deutsch und bei `TextScaler` 2.0; `tester.takeException()` bleibt `null`, und die Zeile der Beschriftung hat in jedem Frame die volle Breite (ein Überlauf unter durchsichtiger Beschriftung zeichnet sonst keinen Fehler).
  - `the state survives a restart`: Umschalten schreibt `shellRailExpanded` in eine Testdatei (`uiStateOverride`), ein neuer `ProviderScope` über derselben Datei startet ausgeklappt.
- `shortcuts_map_to_intents_test.dart`: Zeile `ctrl(digit0, 'ToggleRailIntent')`.
- Goldens nach HUM-054: `icon_rail_collapsed` und `icon_rail_expanded` in `golden_names.dart`, je vier Varianten (dunkel/hell, en/de) über `goldenVariants`; `scripts/ci/goldens-placeholder.sh` fährt sie mit. Die bestehenden Bilder der ganzen App werden neu erzeugt, weil der Knopf hinzukommt, und jedes geänderte Bild wird angesehen.
- Mutationsbeweis je neuem Test: Knopf ohne `onToggle`; Intent ohne Bindung; Palettenbefehl fehlt; Beschriftung unabhängig vom Zustand; Dauer ohne `HReducedMotion.displace`; Einblenden ohne `AnimationBehavior.preserve`; Beschriftung ohne feste innere Breite; Provider ohne Schreiben.

### Akzeptanzkriterien
- [ ] Eingeklappt ist die Leiste 48 px breit und sieht bis auf den Knopf aus wie vor diesem Issue; ausgeklappt 200 px mit Glyph, Name und Kürzel je Bereich.
- [ ] Knopf, `Ctrl+0` und Palettenbefehl schalten um; der Knopf ist per Tastatur bedienbar und hat Semantik- und Hover-Label.
- [ ] Die Breite animiert über `HMotion.arrive` mit `enter`/`exit`; unter reduzierter Bewegung springt sie ohne Zwischenframe.
- [ ] Kein Overflow und kein Umbruch in einem Frame der Animation, in beiden Sprachen.
- [ ] Der Zustand überlebt einen Neustart über `UiStateFile`, ohne neue Abhängigkeit.
- [ ] Alle neuen Texte stehen in beiden ARB-Dateien; `make l10n-lint` grün.
- [ ] Goldens `icon_rail_collapsed` und `icon_rail_expanded` in vier Varianten; die übrigen Goldens sind nachgezogen.
- [ ] `docs/UX.md` 2.2 und `backlog/CONVENTIONS.md` 3.9 nennen die neue Bewegung und das neue Kürzel.

### Fallstricke
- **Überlauf in der Zwischenbreite.** Eine `Row` aus Glyph, `Expanded(Text)` und Kürzel läuft über, sobald die Restbreite kleiner ist als das Kürzel, also in fast jedem Frame der Animation. Die Beschriftung wird deshalb in fester Breite gesetzt und abgeschnitten, nicht in die Zwischenbreite gequetscht.
- **Bestehende Goldens.** Der Knopf ändert jedes Bild, das die ganze App zeigt. Die neuen Bilder entstehen mit `make goldens-update` und werden einzeln angesehen, nicht blind übernommen.
- **Tests schreiben nicht ins Heimatverzeichnis.** Der Provider schreibt in `UiStateFile`; jeder Test läuft mit `uiStateOverride()` (`app/test/harness/ui_state.dart`).
- **Fokus.** Der Knopf ist fokussierbar. Er darf `Ctrl+1` und die übrigen Kürzel der Shell nicht verbrauchen, wenn er den Fokus hat; sie hängen am `Shortcuts` über dem Baum und erreichen ihn entlang der Fokuskette.

### Referenzen
HUM-019 (`backlog/sprint-1.md`, Icon-Rail), HUM-044 (`backlog/sprint-3.md`, `UiStateFile`), HUM-054 (`backlog/sprint-4.md`, Goldens), HUM-243 (`Ctrl+B`), `docs/UX.md` 2.2, 2.10 und 5.1.

## HUM-259 · Neues App-Icon: die Faust
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-034, HUM-053 · Blockiert: nichts

### Kontext
Der Projekteigentümer hat am 2026-10-20 ein neues App-Icon freigegeben: eine schwarze Faust (`#14161C`) auf einer gelben, abgerundeten Kachel mit senkrechtem Verlauf (`#EBC05A` oben, `#D49E34` unten). Die Faust folgt „hand-fist“ aus Font Awesome Free 6.6.0 (CC BY 4.0, <https://fontawesome.com/license/free>), Zeige- und Mittelfinger sind umgeformt. Bisher trug die Anwendung den Amber-Chip aus HUM-034 (`packaging/deb/humanitl.svg`) und im Kopf der Oberfläche ein blaues Akzent-Quadrat vor dem Namen (`Wordmark` in `header_bar.dart`).

Ausgangslage, die die Spezifikation bindet:
- Das Paket installiert das SVG unter `scalable` und drei eingecheckte PNGs (64, 128, 256) unter `hicolor` (`build-deb.sh`); das AppImage nimmt dasselbe SVG als `.DirIcon`. Beide Wege lesen die Dateien aus `packaging/deb/`.
- Der Linux-Runner setzt kein eigenes Fenster-Icon; das Fenster bekommt sein Symbol über die Desktop-Datei (`Icon=humanitl`). Flutter-Assets enthalten kein Icon.
- Die Zuordnung von Fenster und Desktop-Datei war gebrochen. Der Runner setzt `APPLICATION_ID` (`dev.humanitl.humanitl`) als `GApplication`-ID und mit `g_set_prgname`; GTK 3 bildet daraus unter Wayland die `app_id` des Fensters und unter X11 den Instanznamen in `WM_CLASS` (die Klasse ist derselbe Name mit großem Anfangsbuchstaben). Die Desktop-Datei hieß aber `humanitl.desktop` mit `StartupWMClass=humanitl`. Ein Wayland-Compositor sucht `<app_id>.desktop` und fand nichts, die Fensterleiste zeigte ein Ersatzsymbol.
- Die App hat kein SVG-Paket, und es kommt keines dazu.
- CC BY 4.0 verlangt Namensnennung mit Herkunft, Lizenz und Hinweis auf Änderungen, auch im ausgelieferten Paket.

### Ziel
- `packaging/deb/humanitl.svg` ist die freigegebene Datei, nur aufgeräumt: gültiges XML, `<title>`, Herkunftskommentar. Die PNGs sind daraus neu gerechnet, mit einem Skript, das jeder wieder fahren kann.
- Der Kopf der Oberfläche zeigt dieselbe Faust auf der Kachel statt des Akzent-Quadrats, als eigenes Widget `AppMark`, das die Geometrie des SVG als Pfad zeichnet.
- Herkunft und Lizenz stehen in `THIRD_PARTY.md` und im Paket in `/usr/share/doc/humanitl/copyright`.
- Die Fensterleiste des Fenstermanagers zeigt das Icon: Die Desktop-Datei heißt wie die Anwendungs-ID.

### Nicht-Ziel
- Kein Neuentwurf des Icons, keine weiteren Größen, kein Icon für macOS oder Windows.
- Kein Fenster-Icon im Runner (`gtk_window_set_icon`); das Symbol kommt weiter aus der Desktop-Datei.
- Der Tray bleibt in diesem Issue unverändert, siehe „Folge“.

### Betroffene Pfade
- `packaging/deb/humanitl.svg`, `packaging/deb/icons/humanitl-{64,128,256}.png`, `packaging/deb/icons/render.sh` (neu)
- `packaging/deb/copyright` (Abschnitt für die Symbole), `packaging/deb/build-deb.sh` (Kommentar, Name der Desktop-Datei)
- `packaging/deb/humanitl.desktop` umbenannt in `packaging/deb/dev.humanitl.humanitl.desktop`, `packaging/appimage/build-appimage.sh`, `packaging/deb/check-install.sh`
- `THIRD_PARTY.md` (neu), `docs/INSTALL.md` (Abschnitt „Paket bauen“)
- `app/lib/core/ui/app_mark.dart` (neu), `app/lib/features/shell/widgets/header_bar.dart` (nur `Wordmark`)
- `app/test/core/ui/app_mark_test.dart` (neu)

### Spezifikation
- **SVG.** Geometrie und Farben unverändert aus der freigegebenen Datei. Kopf: XML-Deklaration, Kommentar mit Herkunft, Lizenz, Änderung und Verweis auf `THIRD_PARTY.md`, `<title>Humanitl</title>`. Der Kommentar reist mit dem SVG auch ins AppImage, das keine Copyright-Datei trägt.
- **PNGs.** `packaging/deb/icons/render.sh` rechnet die drei Größen mit `inkscape FILE -w N -h N -o OUT`. Die Bilder bleiben eingecheckt; der Paketbau braucht kein Grafikprogramm. Inkscape schreibt keinen Zeitstempel in die PNGs.
- **`AppMark`.** `StatelessWidget` mit `size` (Vorgabe 16), darunter `AppMarkPainter`: Kachel als `RRect` mit `LinearGradient`, Faust als vier `RRect` und ein `Path`, dessen Befehle die des SVG sind, relativ aufgelöst. Farben als Konstanten im Widget, nicht als Theme-Token: Ein App-Icon sieht auf dunkler und heller Fläche gleich aus. Keine Semantik, der Name steht daneben.
- **`Wordmark`.** Das Akzent-Quadrat wird `AppMark(size: markSize)`. Sonst bleibt der Kopf unberührt; HUM-255 arbeitet parallel an derselben Leiste.
- **Desktop-Datei.** Ausgeliefert als `dev.humanitl.humanitl.desktop` (Reverse-DNS wie die Freedesktop-Spezifikation empfiehlt, gleich `APPLICATION_ID`), `StartupWMClass=dev.humanitl.humanitl`. Verworfen: die ID im Runner auf `humanitl` zurückzunehmen. Das bräche die Reverse-DNS-Form der `GApplication`-ID, die GLib für die Einmal-Instanz über D-Bus verlangt, und kein Ort außer der Desktop-Datei hängt am alten Namen. `Icon=humanitl` bleibt; das Symbol wird über die gefundene Desktop-Datei aufgelöst. Paket und AppImage nehmen dieselbe Datei, `check-install.sh` prüft sie unter dem neuen Namen mit `desktop-file-validate`.
- **Lizenz.** `THIRD_PARTY.md` nennt Datei, Vorlage mit Urheber (Fonticons, Inc.), Lizenz mit Link, Änderungen und die Lizenz des Icons als Ganzes (CC BY 4.0). `packaging/deb/copyright` bekommt einen `Files`-Abschnitt für `usr/share/icons/hicolor/*/apps/humanitl.png` und `usr/share/icons/hicolor/scalable/apps/humanitl.svg` mit `License: CC-BY-4.0` und derselben Angabe.

### Tests
- `app/test/core/ui/app_mark_test.dart`:
  - `the_fist_is_black_on_the_yellow_tile`: Der Painter in 256 px (eine Einheit der View-Box je Pixel); die Ecke außerhalb der Rundung ist durchsichtig, ein Punkt im Handballen ist `#14161C`, die Kachel ist oben heller als unten und liegt nahe an den beiden Verlaufsfarben.
  - `the_wordmark_carries_the_app_mark`: `Wordmark(markSize: 24)` enthält genau ein `AppMark` in 24 px und den Namen.
- `packaging/deb/copyright` parst als DEP-5 (`python3-debian`, strikt) und ordnet beide Symbolpfade `CC-BY-4.0` zu; lintian läuft in der Release-Pipeline (`packaging/deb/lintian.sh`).
- Mutationsbeweis: Faust in Akzentfarbe statt `appMarkFist` lässt den ersten Test fallen; Verlauf umgedreht ebenso; `Wordmark` mit dem alten Quadrat lässt den zweiten fallen.
- Goldens werden beim Landen neu erzeugt: Jedes Bild, das den Kopf oder den Splash zeigt (`shell_*`, `intercept_*`, `queue_grouped_*`, `queue_new_pill_*`, `action_bar_note_*`, `setup_*`, `states_*`, soweit sie die ganze App zeigen), ändert sich in den 16 px des Zeichens.

### Akzeptanzkriterien
- [ ] `packaging/deb/humanitl.svg` ist die freigegebene Faust, gültiges SVG mit `<title>` und Herkunftskommentar.
- [ ] Die drei PNGs sind aus dem SVG gerechnet und haben ihre Größe (`check-install.sh`); `render.sh` erzeugt sie neu.
- [ ] Der Kopf der Oberfläche und der Splash zeigen die Faust auf der Kachel statt des Akzent-Quadrats.
- [ ] `THIRD_PARTY.md` und `/usr/share/doc/humanitl/copyright` nennen Herkunft, Lizenz und Änderung des Icons.
- [ ] Die Desktop-Datei heißt `dev.humanitl.humanitl.desktop`, `StartupWMClass` ist dieselbe ID, `desktop-file-validate` grün; unter Wayland und X11 zeigt die Fensterleiste die Faust.
- [ ] lintian ohne neue Warnung.

### Fallstricke
- **Namensnennung im AppImage.** Das AppImage liefert keine Copyright-Datei. Die Angabe steht deshalb im SVG selbst, das als einziges Symbol mitreist.
- **Zwei Zeichnungen derselben Faust.** SVG und `AppMark` müssen gleich bleiben; der Kommentar beider Dateien verlangt, sie im selben Commit zu ändern.
- **Ein Name für Fenster und Desktop-Datei.** Wer `APPLICATION_ID` in `app/linux/CMakeLists.txt` ändert, benennt die Desktop-Datei im selben Commit um; sonst verliert die Fensterleiste das Icon ohne jede Fehlermeldung.
- **`s` im Pfad.** Der glatte Bogen übernimmt den gespiegelten Kontrollpunkt des vorigen; wer den Pfad von Hand überträgt und ihn als einfachen Bogen liest, verbiegt Daumen und Handballen.

### Folge: Tray-Icon als Faust
Der Tray (`app/lib/features/tray/tray_icon.dart`) zeichnet heute einen Chip in der Zustandsfarbe: gefüllt mit Zahl bei `held` und `alert` (Kreuz bei `alert` ohne Zahl), als Umriss bei `idle` und `offline` (`?`). Ziel ist dieselbe Faust als Silhouette ohne Kachel, weil sie in 22 px klarer ist als Faust auf Kachel; den Zustand trägt die Farbe, die Zahl ein Abzeichen. Das ist größer als S und kommt in ein eigenes Issue, weil es neue Entscheidungen verlangt:
- Wie „Umriss heißt: nichts wartet“ (`docs/UX.md` 3.3) mit einer Faust aussieht; eine umrissene Faust in 22 px ist kaum lesbar, eine gefüllte in `fg2` sagt dasselbe wie eine gefüllte in Amber.
- Wo das Abzeichen mit Zahl, `9+` und `?` sitzt und wie groß es sein darf, ohne die Faust zu verdecken; „A tray icon carries no text of its own“ (`sni_tray.dart`) bleibt, die Zahl ist Teil des Bildes.
- Das Kreuz bei `alert` ohne Zahl.
- `tray_icon_test.dart` prüft heute die Mitte des Bildes (gefüllt in Amber bei `held`, leer bei `idle`); beide Tests werden neu formuliert. Den Pfad liefert `AppMarkPainter` schon, ohne Kachel.

### Referenzen
HUM-034 (`backlog/sprint-2.md`, Tray-Icon und Amber-Chip), HUM-053 (`backlog/sprint-4.md`, Paket), HUM-255 (Kopfleiste, parallel), Font Awesome Free License <https://fontawesome.com/license/free>, CC BY 4.0 <https://creativecommons.org/licenses/by/4.0/>.

---

## HUM-257 · Der Daemon vertraut der Zertifikatsablage des Systems
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-015, HUM-039, HUM-087 · Blockiert: HUM-060 (vor HUM-060 zu erledigen)

### Kontext
Am 2026-10-20 trägt der Projekteigentümer in der Oberfläche den Modellserver
`https://llm.hzo.schutzwerk.net/api/v1/` ein (10.100.50.177, privat). Die Probe scheitert mit
`LLM_001` und `upstream_tls`. Das Zertifikat stammt von der Firmen-CA „SCHUTZWERK Service CA 3",
die unter `/usr/local/share/ca-certificates/` liegt und mit `update-ca-certificates` in
`/etc/ssl/certs` installiert ist; `curl` vertraut ihr. Der Daemon vertraute dagegen nur
`webpki-roots` (`daemon/crates/proxy/src/upstream.rs`, `ClientTls::new`). Damit scheiterte jede
Firmen-CA: bei der Probe (`llm_probe.rs`), beim Verkehr des Agenten zum Modell und zu internen
Zielen. Das `why` sagte dazu nur „could not reach … (… did not answer: upstream_tls)", als wäre der
Server nicht da.

### Ziel
Der Daemon prüft Zertifikate von Zielen gegen dieselben Stellen wie `curl` auf demselben Rechner,
und wenn eine Stelle fehlt, sagt er das und wie sie dazukommt. Wer das nicht will, schaltet es ab.

### Entscheidungen
- **`rustls-native-certs`, nicht `rustls-platform-verifier`.** `rustls-native-certs` liest die
  Ablage einmal als Liste von Zertifikaten, die als `RootCertStore` in das vorhandene
  `ClientConfig::builder_with_provider(ring)` geht; Test-CA und Rückfall auf `webpki-roots` sind
  damit gewöhnliche Wurzeln derselben Ablage. `rustls-platform-verifier` bringt einen eigenen
  Verifier mit eigenem Provider-Pfad, unter Linux ohnehin auf `rustls-native-certs` gebaut, und
  ein Rückfall auf `webpki-roots` bei leerer Ablage ließe sich nur daneben bauen.
- **Einstellung `tls.trust_system_store`**, Vorgabe `true`, Stufe `advanced`,
  `x-project-scope = denied`: Ein geklontes Projekt kann das Vertrauen weder erweitern noch
  verengen. Aus gilt nur `webpki-roots` wie vor diesem Issue. Setzbar mit
  `humanitl config set tls.trust_system_store false` und im Einstellungsformular (HUM-069),
  wirkt nach dem Neustart des Daemons. Eine eigene Gruppe `tls` statt eines Schlüssels unter
  `resolver`, weil es nicht um Namensauflösung geht; `resolver.test_ca` bleibt, wo es ist.
- **Im Formular** als neunter Schlüssel von `config_rpc::FORM_KEYS` und `FormKeys.all`: eine
  Gruppe „Verbindungen" mit einem Schalter „Zertifikaten dieses Systems vertrauen, z. B. einer
  Firmen-CA" und einer Hilfe, was aus bedeutet. Das Formular hat keinen Bereich „Erweitert"
  (Stufen sind mit HUM-069 verschoben); die Stufe ist deshalb `advanced` wie bei
  `ui.notifications`, nicht `expert`, denn `expert` heißt „nur in `config.toml`".
- **Rückfall:** Liefert die Ablage keine Wurzel, die rustls annimmt (leer, unlesbar), gelten die
  Wurzeln aus `webpki-roots`. Ein Eintrag, den rustls oder der PEM-Leser ablehnt, kostet eine
  zusammengefasste Warnung, nie den Start.
- **Einmal je Prozess:** `TrustAnchors::configured` liest beim ersten Aufruf und hält das Ergebnis
  in einem `OnceLock`. Proxy-Stapel, Endpunkt-Probe in `humanitl-ipc` und die Probe mit Testwurzel
  teilen sich die Lesung; die Startzeile `upstream trust anchors loaded` (`anchors`, `source`,
  `skipped`) steht genau einmal im Protokoll. `humanitl doctor` hat keinen TLS-Abschnitt, also
  keine Zeile dort.
- **Diagnose:** `Upstream::forward`/`forward_to` liefern `ForwardError` (`UpstreamError` plus
  `untrusted_issuer: Option<TrustSource>`). Nur `CertificateError::UnknownIssuer` zählt; ein
  abgelaufenes Zertifikat oder ein falscher Name bekäme sonst den falschen Rat. Die Probe macht
  daraus `LLM_001` mit „the certificate of X is signed by an authority this machine does not
  trust …" und `CopyCommand("sudo update-ca-certificates")`, bei ausgeschaltetem Schlüssel
  `ChangeSetting tls.trust_system_store = true`. Der Proxy-Pfad meldet dasselbe als `PROXY_003` am
  Fluss, nur an die Oberfläche; der Agent bekommt weiter `502` mit `reason: upstream_tls`.
  Vorher stand im `why` nur `upstream_tls`.
- **Die Test-CA** (`resolver.test_ca`, `--allow-test-ca`) kommt unverändert zusätzlich hinzu.
- **Umlenkung über die Umgebung** (Review, Codex): `rustls-native-certs` liest statt der Ablage
  `SSL_CERT_FILE` und `SSL_CERT_DIR`. Kein Umbau, denn wer die Umgebung des Daemons setzt, richtet
  ihn ein (`docs/SECURITY.md` 11). Aber die Startzeile nennt die Umlenkung (`env`), und der
  Daemon warnt, wenn ein solcher Pfad oder bei einer Datei ihr Verzeichnis dem Besitzer von
  `$HOME` gehört oder für andere schreibbar ist; „gehört nicht root" wäre unter
  `PrivateUsers=yes` immer wahr, root erscheint dort als `nobody`.
- **systemd-Unit:** keine Änderung. `ProtectSystem=strict` macht `/etc` und `/usr` nur
  schreibgeschützt, nicht unlesbar; `ProtectHome`, `TemporaryFileSystem` und
  `InaccessiblePaths` stehen nicht in `packaging/systemd/humanitld.service`. `/etc/ssl/certs` und
  die Ziele seiner Symlinks unter `/usr/share/ca-certificates` und
  `/usr/local/share/ca-certificates` sind `0644` und unter `PrivateUsers=yes` weiter lesbar.

### Betroffene Pfade
- `daemon/crates/proxy/src/trust.rs` (neu: `TrustAnchors`, `TrustSource`, `untrusted_issuer`)
- `daemon/crates/proxy/src/upstream.rs` (`ClientTls::new(anchors, …)`, `ClientTls::configured`, `ForwardError`)
- `daemon/crates/proxy/src/handler.rs` (`untrusted_upstream_certificate`, `PROXY_003` am Fluss)
- `daemon/crates/proxy/src/llm_probe.rs` (`Failure::Untrusted`)
- `daemon/crates/proxy/src/lib.rs`, `daemon/crates/proxy/Cargo.toml`, `daemon/Cargo.lock`
- `daemon/crates/config/src/model.rs` (`TlsConfig`), Schema-Schnappschuss, `docs/CONFIG.md`
- `daemon/bin/humanitld/src/main.rs`, `daemon/crates/ipc/src/server.rs`, `daemon/crates/ipc/src/config_rpc.rs` (`FORM_KEYS`)
- `app/lib/core/domain/settings.dart`, `app/lib/features/settings/settings_sheet.dart`, `app/lib/core/ipc/fake_daemon_client.dart` (Wahrheitswert im Fake), `app/l10n/app_{en,de}.arb` (angehängt), `app/test/features/settings/settings_sheet_test.dart`
- `daemon/crates/core-types/src/diagnostics/codes.rs` (Texte `PROXY_003`, `LLM_001`), `docs/DIAGNOSTICS.md`
- `docs/SECURITY.md` (Abschnitt 4 und 5), `docs/THREAT-MODEL.md` (2.2, Änderungshistorie)

### Akzeptanzkriterien
- [x] Eine eigene CA, per `SSL_CERT_FILE` als Ablage hineingereicht, macht ein lokales TLS-Ziel
  erreichbar; mit einer Ablage ohne sie wird es abgelehnt, ohne Treffer beim Ziel
  (`tests/system_trust.rs`, der Testprozess startet sich dafür mit der Umgebung neu).
- [x] Mit `tls.trust_system_store = false` wird dieselbe CA trotz Ablage abgelehnt.
- [x] Ein kaputter Eintrag in der Ablage verhindert den Stapel nicht; die gültige CA wirkt weiter.
- [x] Leere oder unlesbare Ablage: Rückfall auf `webpki-roots`.
- [x] `LLM_001` und `PROXY_003` nennen die unbekannte Stelle im `why` und den nächsten Schritt.
- [x] Die Startzeile steht genau einmal im Protokoll des echten Binary.
- [x] Beide Stapel des Daemons und der Stapel in `humanitl-ipc` nehmen ihre Anker aus
  `tls.trust_system_store`.
- [x] `docs/SECURITY.md` und `docs/THREAT-MODEL.md` beschreiben, was die Ablage bedeutet, das
  Opt-out, warum root nichts Neues gewinnt und dass die Humanitl-CA der Sandbox unberührt bleibt.
- [x] Der Schalter im Formular zeigt den Wert (ohne Wert an) und schreibt ihn über `SetConfig`;
  `SetConfig` nimmt den Schlüssel an und lehnt einen Text ab, der kein Wahrheitswert ist.
- [x] Mutationsbeweis für jeden neuen Test.

### Fallstricke
- Jede `CaStore` auf demselben Rechner trägt denselben Namen (der `CN` kommt aus der
  `machine-id`), auch die Humanitl-CA des Proxys. Eine zweite `UpstreamCa` als „fremde" CA lässt
  webpki die Unterschrift gegen die gleichnamige falsche CA prüfen und `BadSignature` statt
  `UnknownIssuer` melden. Tests mit unbekannter Stelle nehmen deshalb `ForeignCa`
  (`tests/support/upstream.rs`).
- `SSL_CERT_FILE` im laufenden Testprozess zu setzen ist in Rust 2024 `unsafe` und mit
  parallelen Tests ein Wettlauf; die Tests starten sich deshalb als Kindprozess neu.
- Das Feld der Startzeile heißt `anchors`, nicht `roots`: `roots` ist schon das Feld der
  Warnzeile zu `--allow-test-ca`, nach dem `a_test_ca_is_only_trusted_with_the_flag` sucht.
- Der Zähler in `tools/check_coupling.py` zählt jeden Namen aus `humanitl_proxy` außerhalb des
  Crates. Der Daemon ruft deshalb `ClientTls::configured(bool, …)` und nennt `TrustAnchors` nicht.

### Referenzen
HUM-015, HUM-039, HUM-087, `docs/SECURITY.md` 5, `docs/THREAT-MODEL.md` 2.2.

---

## HUM-255 · Das Fenster trägt keine doppelte Titelleiste
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-019, HUM-034, HUM-054 · Blockiert: nichts

### Kontext
Befund des Projekteigentümers vom 2026-10-20 unter sway: Über der App liegt eine GTK-Kopfleiste mit „humanitl" und ×, und darüber zeichnet sway seine eigene Titelzeile. Zwei Titelleisten übereinander, mit der Kopfleiste der App drei Zeilen, bevor der erste Inhalt kommt.

Ausgangslage, die die Spezifikation bindet:
- Die GTK-Leiste stammt aus dem Standard-Runner von Flutter. `app/linux/runner/my_application.cc` baut auf jeder Wayland-Sitzung und unter X11 bei GNOME Shell eine sichtbare `GtkHeaderBar` als `titlebar`; unter X11 mit einem anderen Fenstermanager setzt er nur den Titel und überlässt die Dekoration dem Fenstermanager.
- `window_manager` 0.5.2 ist schon Abhängigkeit (`app/lib/main.dart` setzt Titel und Mindestgröße, `app/lib/features/tray/platform/window_port.dart` ist der Port aus HUM-034). `WindowPort` in `app/lib/features/tray/desktop_ports.dart` ist die eine Naht zum Fenster; Tests ersetzen ihn über `desktopPortsProvider` durch `FakeWindow` (`app/test/features/tray/fixtures.dart`).
- Schließen über den Fenstermanager löst heute `delete-event` aus. `window_manager` hängt sich dort ein, meldet das Ereignis `close` und verhindert das Schließen nur, wenn `setPreventClose(true)` gesetzt ist. Das ist es nirgends: Das Fenster geht, die `GtkApplication` verliert ihr letztes Fenster, das Programm endet. Ein „ins Tray verstecken" gibt es nicht. Der Menüpunkt „Beenden" des Trays geht einen anderen Weg (`DesktopPorts.dispose`, dann `WindowPort.quit`).

### Ziel
- **Keine Kopfleiste des Toolkits mehr, auf Wayland wie auf X11.** Der Runner baut die `GtkHeaderBar` weiter dort, wo er sie heute baut, zeigt sie aber nie (`gtk_widget_show` entfällt), und setzt den Fenstertitel in beiden Zweigen mit `gtk_window_set_title`, damit der Compositor schon vor dem ersten Dart-Frame einen Namen hat.
  Begründung der Wahl:
  - Eine gesetzte, aber unsichtbare `titlebar` hält GTK 3 in clientseitiger Dekoration: Unter GNOME (Wayland und X11 mit Compositor) bleiben Schatten und die unsichtbare Randzone, an der der Zeiger die Größe ändert; ohne Compositor bleibt der schmale Rahmen von `.solid-csd`. Es fehlt nur die Leiste. Das ist derselbe Zustand, den `window_manager` mit `TitleBarStyle.hidden` herstellt (`gtk_widget_set_visible(header_bar, false)`).
  - `gtk_window_set_decorated(FALSE)` wäre unter GNOME auf Wayland das Ende jedes Rahmens: kein Schatten, keine Randzone, kein Ändern der Größe ohne einen selbst gebauten Ziehrand.
  - Gar keine `titlebar` ist auf Wayland kein Ausweg. GTK 3 fragt dann den Compositor nach serverseitiger Dekoration; sway und KDE geben sie, Mutter (GNOME) nicht, und dort zeichnet GTK ersatzweise seine eigene Standard-Titelleiste, also wieder eine GTK-Leiste.
  - `windowManager.setTitleBarStyle(TitleBarStyle.hidden)` aus Dart erreicht auf Wayland und unter GNOME dasselbe, ruft aber im X11-Zweig ohne Kopfleiste `gtk_window_set_decorated(false)` und nimmt einem schwebenden X11-Fenstermanager seinen Rahmen samt Randzone. Außerdem liefe es erst, wenn Dart läuft; der Runner entscheidet, bevor das Fenster je sichtbar ist.
  - Der X11-Zweig ohne GNOME Shell bleibt, wie er ist: Dort zeichnet der Fenstermanager seine Dekoration selbst, wie sway unter Wayland, und GTK zeichnet keine.
- **Die Kopfleiste der App wird zur Griff-Fläche.** `HeaderBar` (`app/lib/features/shell/widgets/header_bar.dart`, „Humanitl · <Bereich> … Ctrl+K") liegt über einer Fläche, die auf Ziehen `WindowPort.startDragging` und auf Doppelklick `WindowPort.toggleMaximize` ruft. Die anzeigenden Teile (Wortmarke, Bereichstitel, die beiden Badges) lassen den Zeiger durch, die Bedienelemente (Isolationsring, Palettenknopf, Fensterknöpfe) fangen ihn selbst: Ein Klick auf sie löst nur sie aus, und der Doppelklick-Erkenner der Fläche verzögert ihren Klick nicht.
- **Eigene Fensterknöpfe rechts in der Kopfleiste**, nach dem Palettenknopf: Minimieren (`HGlyph.minus`, neu), Maximieren oder Wiederherstellen (`HGlyph.square`, neu), Schließen (`HGlyph.close`). Je ein `HIconButton` mit `HoverLabel` und Semantik-Label aus ARB, per `Tab` erreichbar und mit `Enter` und `Leertaste` auslösbar (`docs/UX.md` 5.1). Farbe wie jeder Ghost-Knopf; kein rotes Schließen, weil Farbe in diesem Programm Zustand bedeutet (`docs/UX.md` 3.3).
- **Schließen ist der Weg des Fenstermanagers.** Der Knopf ruft `WindowPort.close`, und `WindowManagerPort.close` ruft `windowManager.close()`, also `gtk_window_close`: dasselbe `delete-event`, das der Fenstermanager schickt, mit derselben Behandlung durch `window_manager` und GTK. Nicht `quit`: Das ist der Weg des Tray-Menüs. Kommt eines Tages „Schließen versteckt ins Tray", setzt es `setPreventClose` an einer Stelle und gilt dann für Knopf und Fenstermanager zugleich.
- **Der Port wächst um vier Methoden:** `close`, `minimize`, `toggleMaximize` (fragt `isMaximized` und ruft `maximize` oder `unmaximize`) und `startDragging`. Die träge Fassung tut nichts, `FakeWindow` zählt mit, `WindowManagerPort` schluckt wie bisher `MissingPluginException`.
- **Kein Raten nach Kachel-Fenstermanagern.** Unter sway und anderen Kachel-Managern bleiben Minimieren und Maximieren oft wirkungslos. Sauber erkennen lässt sich das nicht: GTK 3, auf dem der Linux-Embedder von Flutter steht, gibt die Fähigkeiten des Compositors nicht heraus (`xdg_toplevel.wm_capabilities` aus xdg-shell v5 erreicht erst GTK 4), und Umgebungsvariablen wie `SWAYSOCK`, `I3SOCK` oder `XDG_CURRENT_DESKTOP` sind eine Namensliste, die jeden weiteren Kachel-Manager verfehlt und in verschachtelte Sitzungen und dort gestartete Kindprozesse durchsickert. Die Knöpfe bleiben deshalb überall stehen; ein wirkungsloser Knopf schadet nicht, ein fälschlich versteckter schon.
- **Jeder Bildschirm, der das ganze Fenster füllt, ist greifbar und schließbar.** Splash und der eigenständige Setup-Bildschirm eines Kaltstarts ohne Daemon stehen ohne Shell und damit ohne deren Kopfleiste. `ConnectionGate` stellt sie deshalb unter eine schmale `WindowBar` (Griff-Fläche, rechts die drei Knöpfe); die Shell bekommt keine, weil ihre Kopfleiste beides schon trägt. So gibt es nirgends eine doppelte Leiste und nirgends ein Fenster, das sich mit der Maus weder verschieben noch schließen lässt.
- **Texte** über ARB, `en` Quelle, `de` Übersetzung, nur angehängt: `shellWindowMinimize`, `shellWindowMaximize`, `shellWindowClose`.
- **`docs/UX.md` 4.9** bekommt einen Absatz zum Fenster: keine Toolkit-Leiste, Kopfleiste als Griff, eigene Knöpfe, Schließen wie beim Fenstermanager.

### Nicht-Ziel
- Kein „Schließen versteckt ins Tray", kein `setPreventClose`. Heute beendet Schließen das Programm, und dieses Issue ändert kein Verhalten, nur den Ort des Knopfs.
- Kein eigener Ziehrand zum Ändern der Größe (`DragToResizeArea`); die Randzone liefert die clientseitige Dekoration von GTK oder der Fenstermanager.
- Kein Umschalten des Maximieren-Glyphs nach Zustand. Dafür müsste der Port Maximieren-Ereignisse melden, die unter Kachel-Managern nie kommen; das Label nennt beides.
- Keine Erkennung von Kachel-Managern (siehe Ziel), kein Wechsel auf GTK 4.

### Betroffene Pfade
- `app/linux/runner/my_application.cc` (Kopfleiste unsichtbar, Titel in beiden Zweigen)
- `app/lib/features/tray/desktop_ports.dart` (`WindowPort` um vier Methoden, träge Fassung)
- `app/lib/features/tray/platform/window_port.dart` (`WindowManagerPort`: die vier Aufrufe)
- `app/lib/features/shell/widgets/window_frame.dart` (neu: `WindowDragArea`, `WindowButtons`, `WindowBar`, `WindowFramed`)
- `app/lib/features/shell/connection_gate.dart` (Splash und Setup unter `WindowFramed`)
- `app/lib/features/shell/widgets/header_bar.dart` (Griff-Fläche, Knöpfe, anzeigende Teile ohne Zeiger)
- `app/packages/ui/lib/src/tokens/flow_state.dart`, `app/packages/ui/lib/src/widgets/h_glyph.dart` (`HGlyph.minus`, `HGlyph.square`, angehängt)
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb` (nur angehängt)
- `docs/UX.md` 4.9
- `app/test/features/shell/window_frame_test.dart` (neu), `app/test/features/tray/fixtures.dart` (`FakeWindow` zählt die vier neuen Aufrufe), `app/packages/ui/test/gaps_test.dart` (die zwei neuen Glyphen)

### Tests
- `app/test/features/shell/window_frame_test.dart`, in der ganzen App über `pumpApp` mit `FakeDesktop`:
  - `the header carries minimize, maximize and close`: drei Knöpfe, je ein Semantik-Knoten mit dem ARB-Label, in Englisch und Deutsch.
  - `close takes the window manager's way, not the tray's`: Tipp auf Schließen, `FakeWindow.closes == 1`, `quits == 0`, kein `window.dispose` im Protokoll.
  - `minimize and maximize reach the port`: je ein Tipp, je ein Zähler auf 1.
  - `the window buttons answer the keyboard`: Fokus auf Schließen, `Enter` ruft `close`, `Leertaste` auf Minimieren ruft `minimize`.
  - `dragging the header moves the window`: Ziehen auf dem Bereichstitel und auf der freien Fläche zwischen Titel und Badges ruft `startDragging`.
  - `a control in the header does not drag`: Tipp auf den Palettenknopf öffnet die Palette, `startDragging` bleibt 0; ein Ziehen, das auf dem Palettenknopf beginnt, ebenfalls 0.
  - `a double click on the header toggles maximize`: zwei Tipps im Abstand unter `kDoubleTapTimeout` auf freier Fläche, `toggleMaximize == 1`.
  - `the setup screen of a cold start can be moved and closed`: ohne Daemon steht der Setup-Bildschirm, die drei Knöpfe sind da, Schließen ruft `close`, Ziehen auf der Leiste ruft `startDragging`.
  - `the shell shows one set of window buttons`: in der Shell genau ein Schließen-Knopf und keine `WindowBar`.
  - `a control's click is not held back by the double click`: Tipp auf den Palettenknopf und ein einziges `pump()` ohne Wartezeit; die Palette steht schon.
- `app/packages/ui/test/gaps_test.dart`: die zwei neuen Glyphen in der Liste von `every glyph paints`.
- Mutationsbeweis je Test: Knopf entfernt; Schließen ruft `quit` statt `close`; `onPressed` von Minimieren und Maximieren vertauscht; Knopf in `ExcludeFocus`; Griff-Fläche ohne `onPanStart`; Bereichstitel ohne `IgnorePointer`; Griff-Fläche als Elternteil der Zeile statt als Geschwister darunter; `onDoubleTap` entfernt; `WindowFramed` um das Setup entfernt; die Shell zusätzlich gerahmt.
- `make runner-test` bleibt grün; `my_application.cc` wird zusätzlich mit `g++ -fsyntax-only` gegen `gtk+-3.0` und die Header von `flutter_linux` übersetzt, soweit sie vorliegen.
- Goldens: Jedes Bild, das die Kopfleiste zeigt, ändert sich durch die drei Knöpfe und wird beim Landen mit `make goldens-update` neu erzeugt und einzeln angesehen.

### Akzeptanzkriterien
- [ ] Unter Wayland (sway, GNOME) und X11 zeigt GTK keine Kopfleiste mehr; unter GNOME lässt sich die Größe an den Rändern weiter ändern.
- [ ] Ziehen auf freier Fläche oder auf anzeigenden Teilen der Kopfleiste verschiebt das Fenster, Doppelklick maximiert oder stellt wieder her; Bedienelemente darin reagieren auf Klick wie vorher und ohne Verzögerung.
- [ ] Minimieren, Maximieren und Schließen stehen rechts in der Kopfleiste, mit Hover-Label, Semantik-Label und Tastaturbedienung.
- [ ] Schließen ruft `windowManager.close()` und verhält sich damit genau wie das Schließen über den Fenstermanager.
- [ ] Alle `window_manager`-Aufrufe der Shell laufen über `WindowPort`; kein Widget importiert `window_manager`.
- [ ] Neue Texte in beiden ARB-Dateien, nur angehängt; `make l10n-lint` grün.
- [ ] `docs/UX.md` 4.9 beschreibt das Fensterverhalten.

### Fallstricke
- **Doppelklick-Erkenner verzögern Klicks.** Ein `GestureDetector(onDoubleTap: …)` als Elternteil der ganzen Leiste hält jeden Klick auf einen Knopf darin bis `kDoubleTapTimeout` in der Arena fest. Die Griff-Fläche liegt deshalb als Geschwister unter der Zeile (`Stack`), nicht über ihr, und bekommt nur, was die Zeile durchlässt.
- **Text fängt den Zeiger.** `RenderParagraph` meldet sich selbst als getroffen; ohne `IgnorePointer` ist der Bereichstitel ein toter Fleck in der Griff-Fläche.
- **Schließen ist nicht Beenden.** `quit` räumt die Ports ab und zerstört das Fenster; das ist der Weg des Tray-Menüs. Der Knopf nimmt den Weg des Fenstermanagers, sonst ändert ein späteres `setPreventClose` nur die halbe App.
- **Unsichtbar heißt nicht fehlend.** Ohne `titlebar` zeichnet GTK unter GNOME auf Wayland eine Ersatzleiste; die Kopfleiste muss gesetzt bleiben und nur unsichtbar sein.

### Referenzen
HUM-019 (`backlog/sprint-1.md`, Shell und Kopfleiste), HUM-034 (`backlog/sprint-2.md`, `WindowPort`), HUM-054 (`backlog/sprint-4.md`, Goldens), `docs/UX.md` 3.3, 4.9 und 5.1, `window_manager` 0.5.2 `linux/window_manager_plugin.cc` (`set_title_bar_style`, `close`, `start_dragging`).

---

## HUM-260 · Gelb wird die Akzentfarbe der Oberfläche
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-008, HUM-254 · Blockiert: nichts

### Kontext
Entscheidung des Projekteigentümers vom 2026-10-20. Das neue App-Icon ist eine schwarze Faust auf einer gelben Kachel (#EBC05A bis #D49E34, HUM-259). Die Oberfläche übernimmt diese Corporate Identity: gelbe Akzente, dunkler Vordergrund darauf. Heute ist der Akzent blau (`HColors.accent` #7C9CF5, `HColors.lAccent` #5B7FE6 in `app/packages/ui/lib/src/tokens/colors.dart`), und der Zustand `held` ist Amber (#E0B24A). Ein gelber Akzent neben einer amberfarbenen `held`-Rail wäre genau die Verwechslung, die `docs/UX.md` 3.3 (Regel 8 und 9: Fläche zum Anfassen ist Akzent, Fläche zum Ablesen ist Zustand) ausschließt. `held` bekommt deshalb auf Wunsch des Projekteigentümers eine eigene, klar andere Farbe, ein kühles Cyan.

Ergänzung vom selben Tag: Zur Marke gehört ein gelb-schwarzes Absperrband, diagonale Streifen, die endlos durchlaufen. Humanitl ist die Absperrung, an der der Verkehr hält. Es kommt als wiederverwendbares Widget in `packages/ui` und wird sparsam eingesetzt.

Ausgangslage, die die Spezifikation bindet:
- Die Token-Schicht trennt Fläche und Wort: `accent` (Fläche, 3:1), `accentText` (Wort, 4,5:1, abgeleitet), `accentFill` (Füllung des Primärbuttons, weicht zurück, bis `onAccent` darauf 4,5:1 erreicht). Die hellen Zustandsfarben werden abgeleitet (`HColorDerivation.lightState`), nie von Hand geschrieben; `tokens_test.dart` friert die Ableitungen ein.
- Die Methoden-Badges borgen ihre Farbe: `GET` den Akzent, `PUT`/`PATCH` `held` (`HColors.methodGet`, `HColors.methodPutPatch`).
- Das Terminal-Farbschema (`terminal_palette.dart`) legt die ANSI-Grundfarben auf die Zustandsfarben: bisher Gelb auf `held`, Blau auf den Akzent.
- `docs/UX.md` 2.9 verbietet Dauerbewegung außer dem begrenzten Atem, 2.10 verlangt für jede Schleife einen ruhenden Ersatz unter reduzierter Bewegung, 2.8 verbietet Verschieben unter dem lesenden Auge.

### Ziel
- **Akzent dunkel:** #E8B84B, aus der Marke abgeleitet. Auf `bg0` bis `bg3` mindestens 7,95:1.
- **Akzent hell:** Ocker #A57A1C, Ton der Marke (41°), so dunkel, dass er als Fläche auf der dunkelsten hellen Fläche `lBg3` 3,29:1 erreicht und `onAccent` auf ihm 4,65:1. Das Fenster für beide Bedingungen ist schmal (relative Leuchtdichte rund 0,211 bis 0,243); der Wert sitzt in seiner Mitte. Damit ist die Füllung des Primärbuttons in beiden Themes der Akzent selbst.
- **Vordergrund auf dem Akzent:** neues Token `HColors.onAccent` #14161C, in beiden Themes; `HSurfaceColors.onAccent` zeigt darauf. Weiß misst auf dem Akzent 1,85:1 und auf dem Ocker 3,89:1 und ist deshalb ausgeschlossen. Auf dem Gelb misst `onAccent` 9,81:1.
- **`held`:** Himmelscyan #79C9EC (hell abgeleitet #1C91C5, als Wort hell #136386). Abstand zu jeder anderen Zustandsfarbe und zum Akzent in OKLab mindestens 0,08, normal und unter simulierter Protanopie und Deuteranopie (Machado 2009), in beiden Themes. Unter Deuteranopie fallen Cyan und das Violett von `passthrough` auf dasselbe Blau; die Helligkeit trennt sie, deshalb ist `held` heller als `passthrough`. Zum Vergleich: `allowed` und `allowedEdited` liegen 0,037 auseinander, das alte Amber läge neben dem neuen Akzent bei 0,02.
- **`allowedEdited`** bleibt #57B99F, jetzt als fester Wert statt als Mischung mit dem Akzent; zum Gelb gezogen rückte es ans Grün heran.
- **Terminal:** ANSI-Gelb ist der Akzent, ANSI-Cyan ist `held`, ANSI-Blau eine eigene Konstante, weil keine Farbe des Programms mehr blau ist.
- **Warnung und Information** (Nachtrag vom selben Tag, Koordination): neues Token `warning` im Gelb der Marke (Wert gleich dem Akzent, Wort `warningText`) und `info` im bisherigen Blau (#7C9CF5 dunkel, #5B7FE6 hell, Wort `infoText`). Alle Stellen, die `held` als Warnfarbe benutzten, nehmen `warning`: `Severity.warning`, Setup `warn` und `checking`, Sandbox `starting`/`stopping`, laufende Isolationsprüfung, beschreibbarer Mount, Ausnahme der Isolation, Hinweise im Isolations- und Mount-Tab, abgeschaltete Verweigerungsmeldung, Warnungen der Audit-Kette, Hinweis „ganzer Host" der Agentenbitte, offene Funde im Editor und in der Fundleiste, Hinweise im Export, Füllstand der Warteschlange in der Statusleiste, Release Valve mit Funden, `FindingsPause`, PII-Markierung in Body und Editor. `Severity.info` nimmt `info`. `held` bleibt ausschließlich der Zustand „gehalten" (Queue, Kopfzähler, Tray, Rückkehr-Banner, Terminal-Streifen „wartet"). Eine Warnung steht nie als Farbe allein, sondern mit Glyph oder Text (`docs/UX.md` 3.3, Regel 12).
- **Tray-Zähler:** Ziffer auf dem Cyan-Chip bleibt `bg0`, gemessen über 9:1; die Farbe steht als `trayIconInk` und ist getestet.
- **Absperrband:** `HCautionTape` in `packages/ui`, diagonale Streifen in `HColors.accent` und `HColors.onAccent`, Höhe frei wählbar (Vorgabe `HSize.cautionTape`, 4 px), läuft linear und nahtlos mit `HMotion.cautionTape` (1200 ms je Periode von `HMotion.cautionTapePeriod`, 16 px), steht unter `HReducedMotion` still. Eingesetzt an zwei Stellen: über der Intercept-Warteschlange, solange mindestens eine Anfrage gehalten ist, und als unbestimmte Fortschrittsanzeige der Netzsuche im Setup an Stelle der laufenden Haarlinie.

### Nicht-Ziel
- Kein neues App-Icon, kein Tray-Icon, kein Logo im Shell-Kopf (HUM-259). `packaging/deb/humanitl.svg` bleibt unberührt.
- Keine Änderung an der Titelleiste (HUM-255).
- Goldens werden nicht in diesem Arbeitsbaum erzeugt; das geschieht beim Landen auf dem aktuellen `main`.

### Betroffene Pfade
- `app/packages/ui/lib/src/tokens/colors.dart` (Akzent, `onAccent`, `held`, `allowedEdited`, Doku von `readableFill`)
- `app/packages/ui/lib/src/tokens/tokens.dart` (`onAccent` in beiden Leitern, `accentFill` gegen `onAccent`)
- `app/packages/ui/lib/src/tokens/terminal_palette.dart` (ANSI-Zuordnung)
- `app/packages/ui/lib/src/tokens/motion.dart`, `spacing.dart` (Tokens des Bands)
- `app/packages/ui/lib/src/theme/shadcn_theme.dart` (Hover und Druck des Primärbuttons rücken vom Wort weg)
- `app/packages/ui/lib/src/widgets/h_caution_tape.dart` (neu), `app/packages/ui/lib/humanitl_ui.dart` (Export)
- `app/packages/ui/lib/src/h_editor_decorations.dart` (Kommentar mit altem Hex)
- `app/lib/features/intercept/widgets/queue_pane.dart` (Band unter dem Queue-Kopf)
- `app/lib/features/intercept/widgets/new_arrivals_pill.dart` (Wort in `accentText` statt `accent`)
- `app/lib/features/setup/widgets/llm_discover_sheet.dart` (Band als Fortschritt der Suche)
- `app/test/flutter_test_config.dart` (`HCautionTape.debugFreeze`)
- `app/packages/ui/test/tokens_test.dart`, `app/packages/ui/test/caution_tape_test.dart` (neu), `app/test/features/intercept/caution_tape_queue_test.dart` (neu), `app/test/features/setup/llm_discover_test.dart` (ein Test angehängt)
- Warnung und Information: `app/lib/core/ui/{diagnostic_severity,findings_pause}.dart`, `app/lib/core/body/body_marks.dart`, `app/lib/features/setup/setup_text.dart`, `app/lib/features/sandbox/sandbox_text.dart`, `app/lib/features/sandbox/widgets/{isolation_panel,mounts_tab,refusals_section}.dart`, `app/lib/features/audit/widgets/chain_status_card.dart`, `app/lib/features/intercept/widgets/{agent_ask_card,action_bar}.dart`, `app/lib/features/editor/editor_screen.dart`, `app/lib/features/editor/widgets/findings_rail.dart`, `app/lib/features/history/{history_export_menu,history_filter_bar}.dart`, `app/lib/features/shell/widgets/status_bar.dart` (eine Zeile), `app/lib/features/tray/tray_icon.dart` (`trayIconInk`)
- Tests dazu: `app/test/core/ui/warning_info_test.dart` (neu), `app/test/features/tray/tray_icon_test.dart`, angepasst `audit_screen_test`, `action_bar_contrast_test`, `diagnostic_card_test`, `decorations_test`, `diagnostics_place_test`, `attention_notice_test`
- `docs/UX.md` 2.2, 2.3, 2.9, 3.3 (Regel 3, 9, 10, neue Regeln 11 und 12, Tabelle für `warning` und `info`), 3.6 (neu), 4.7, 4.9, 8, 9; `BACKLOG.md` 5 (Farbzeile)

### Spezifikation
- **Kontrast.** Akzent dunkel als Fläche auf allen vier dunklen Flächen mindestens 3:1 (gemessen mindestens 7,95:1), Ocker auf allen vier hellen mindestens 3:1 (gemessen mindestens 3,29:1). `onAccent` auf `accent` und `accentFill` beider Themes mindestens 4,5:1. Ein Wort in Akzentfarbe steht in `accentText`; der Ocker misst als Wort auf Weiß nur 3,89:1.
- **Primärbutton.** Hover und Druck verschieben die Füllung nie zum dunklen Wort hin, wenn es dort unter 4,5:1 fiele. Hover hellt auf; der Druck dunkelt ab, solange das Wort lesbar bleibt (dunkles Theme), sonst hellt er eine Stufe weiter auf als der Hover (helles Theme), damit Hover und Druck nie dieselbe Farbe tragen.
- **Absperrband.** Ein `AnimationController` treibt einen `CustomPainter` über `repaint`; das Widget baut beim Laufen nicht neu. `RepaintBoundary` um das Band. Ticker aus `SingleTickerProviderStateMixin`, also still unter `TickerMode(enabled: false)`. Ohne Anlass ist das Band nicht im Baum. Sein Platz ist immer reserviert (sonst Haarlinie), damit nichts springt. `ExcludeSemantics`: das Band ist weder Control noch Zustand. Unter reduzierter Bewegung Phase null, kein Ticker. `HCautionTape.debugFreeze` hält es in den Tests der Anwendung und in den Goldens an; sonst liefe jedes `pumpAndSettle` über einer gehaltenen Anfrage in seine Zeitgrenze.
- **Regel in `docs/UX.md` 3.6:** wo das Band steht (die zwei Stellen), wo nie (in Zeilen, Karten, über Text, als Zustand, als Control, in Zustandsfarbe), dass es nie der einzige Kanal einer Aussage ist und nie mit einem Zustand verwechselt werden darf; Abweichung von 2.9 in Abschnitt 8.

### Tests
- `tokens_test.dart`: eingefrorene Tabellen und Literale nachgezogen; neue Gruppe „HUM-260": Akzent auf `bg0`/`bg1` und `lBg0`/`lBg1` mindestens 3:1 mit eingefrorenen Messwerten; `onAccent` auf Akzent und Füllung beider Themes mindestens 4,5:1, Weiß dort darunter; Ton beider Akzente im Bereich der Marke; Abstand von `held` zu allen Zustandsfarben und zum Akzent in OKLab mindestens 0,08 unter normalem Sehen, Protanopie und Deuteranopie, beide Themes; Gegenbeweis, dass das alte Amber die Prüfung nicht bestünde und die Simulation `allowed`/`blocked` zusammenrückt; Methoden folgen; Terminal-Töne an ANSI 3, 4 und 6.
- `caution_tape_test.dart`: läuft gleichmäßig und läuft über das Ende vorwärts weiter; eine volle Periode zeichnet dieselben Streifen wie keine; steht unter reduzierter Bewegung ohne geplanten Frame; plant unter `TickerMode(enabled: false)` keinen Frame; baut beim Laufen nicht neu und hat eine eigene `RepaintBoundary`; `debugFreeze` hält es an; trägt Marke, nicht Zustand.
- `caution_tape_queue_test.dart`: kein Band ohne gehaltene Anfrage, Band mit der ersten, weg nach dem Timeout der letzten; die Liste darunter verschiebt sich dabei nicht.
- `llm_discover_test.dart`: `the_caution_tape_runs_only_while_searching`.
- Mutationsbeweise: `held` zurück auf #E0B24A; `onAccent` auf Weiß; `lAccent` auf ein helleres Gelb; Hover des Primärbuttons wieder abdunkelnd; `repeat()` entfernt bzw. `repeat(reverse: true)`; Periode der Verschiebung verfälscht; Prüfung auf `HReducedMotion` entfernt; Maler über `AnimatedBuilder` statt `repaint`; Bedingung `held > 0` entfernt; reservierter Platz entfernt.

### Akzeptanzkriterien
- [ ] Akzent dunkel #E8B84B, hell #A57A1C, `onAccent` #14161C in beiden Themes; alle Kontrastzahlen oben in `tokens_test.dart` belegt.
- [ ] Kein weißer oder heller Text auf einer Akzentfläche; Wörter in Akzentfarbe stehen in `accentText`.
- [ ] `warning` (Gelb der Marke) und `info` (#7C9CF5 / #5B7FE6) sind eigene Tokens mit Textvariante; keine Warnung und kein Info trägt mehr `held` oder den Akzent als Zuordnung; Kontraste getestet.
- [ ] `held` #79C9EC, Abstand zu allen Zustandsfarben und zum Akzent auch unter Rot-Grün-Schwäche nachgerechnet; `PUT`/`PATCH` folgen.
- [ ] Kein hart kodiertes #E0B24A, #7C9CF5 oder #5B7FE6 mehr außerhalb historischer Backlog-Texte und des App-Icons.
- [ ] `HCautionTape` läuft, steht unter reduzierter Bewegung, zeichnet unsichtbar nichts; erscheint und verschwindet mit gehaltenen Anfragen; je Test ein Mutationsbeweis.
- [ ] `docs/UX.md` beschreibt neue Farben und die Regel des Bands; `BACKLOG.md` 5 nennt die neuen Werte.
- [ ] Goldens beim Landen neu erzeugt und angesehen; das Band steht darin als Standbild.

### Fallstricke
- **Gelb auf Weiß.** Ein heller Akzent, der als Fläche 3:1 hält, ist zu dunkel für weißen Text und zu hell für schwarzen, wenn man ihn nicht in das schmale Fenster legt. Der Ocker sitzt mittig; wer ihn verschiebt, prüft beide Grenzen.
- **Richtung der Hover-Füllung.** Die alte Regel „hell abdunkeln, dunkel aufhellen" setzte weißen Text im hellen Theme voraus. Mit dunklem Text bricht sie das Wort (gemessen 3,84:1).
- **`held` als „Warnung".** Viele Stellen benutzten `held` als Warnfarbe. Mit Cyan hätte sich eine Warnung neben einer gelben Information verkehrt herum gelesen; deshalb `warning` und `info` als eigene Tokens. Weil `warning` dem Akzent gleicht, unterscheidet sich die Release Valve mit Funden nur noch über Beschriftung und Glyph von der ohne, und die PII-Welle von der Ersetzung über die Form.
- **Endlose Bewegung in Tests.** Ein laufendes Band plant endlos Frames; ohne `debugFreeze` in `flutter_test_config.dart` hängt jedes `pumpAndSettle` über gehaltenen Anfragen und jedes Golden der Warteschlange.
- **Terminal.** Die ANSI-Zuordnung hing an den alten Tönen; ohne Umstellung hätte `ESC [ 33 m` Cyan und `ESC [ 34 m` Gelb gezeigt.

### Referenzen
HUM-008 (Tokens), HUM-034 (Tray), HUM-042 (Terminal), HUM-076 (Netzsuche), HUM-254, HUM-255, HUM-259, `docs/UX.md` 2.2, 2.9, 2.10, 3.3, 3.6, 6.

### Umsetzung, Nachträge aus dem Review
- Die History schreibt `Severity.info` in `infoText`, nicht mehr in `accentText` (`historySeverityColor`).
- Die Shell setzt um jeden Abschnitt ihres `IndexedStack` einen `TickerMode`, an nur für den sichtbaren. `IndexedStack` hält die Ticker verdeckter Abschnitte nicht an; ohne das plante das Absperrband nach einem Abschnittswechsel weiter Frames, und `TickerMode.valuesOf` in der History antwortete nicht, wie sie annimmt.
- Die Release Valve trägt bei offenem Fund das Warnglyph vor der Beschriftung (`docs/UX.md` 4.7).
- Kommentare und Testnamen sprechen von der Warnfarbe statt von Amber.

### Kandidaten für ein neues Issue
- Terminal: Cursor und ANSI-Gelb sind beide der Akzent; ein gelber Cursor auf gelber Ausgabe verschwindet. Eigener Cursorton oder Invertierung prüfen.
- `info` liegt unter Deuteranopie nah an `passthrough` (OKLab 0,019) und im hellen Theme nah an `held` (0,063). Heute trägt `info` immer sein Wort; ein eigener Ton wäre robuster.

---

## HUM-261 · Die Probe versteht Endpunkte mit Pfad-Präfix
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-037, HUM-039, HUM-076 · Blockiert: HUM-060 (vor HUM-060 zu erledigen)

### Kontext
Am 2026-10-20 trägt der Projekteigentümer mit v0.0.4 den Endpunkt
`https://llm.hzo.schutzwerk.net/api/v1/` ein, einen OpenAI-kompatiblen Server hinter dem
Pfad-Präfix `/api`. `OpenCode` benutzt genau diese `baseURL` mit `@ai-sdk/openai-compatible`
erfolgreich. Die Diagnose der Probe schlägt dagegen zum Kopieren
`curl -sS https://llm.hzo.schutzwerk.net:443/api/api/tags` vor, und der Server antwortet darauf
`{"detail":"Not Found"}`. Der Mensch bekommt damit den Hinweis auf einen Pfad, den es nicht gibt.

Ursache in `daemon/crates/proxy/src/llm_probe.rs`: `Target::parse` nahm als Wurzel den Pfad ohne
abschließendes `/v1`, hier `/api`. Die Ollama-Frage hängte `/api/tags` daran (`/api/api/tags`),
die OpenAI-Frage `/v1/models` (`/api/v1/models`, richtig). Der Ablauf der Probe am alten Code:

- Ein `404` von `/api/tags` bricht nicht ab; `get` gibt `None`, und die Probe fragt danach
  `/api/v1/models`.
- `models` mit `401` (ohne Schlüssel): `LLM_002`, `why` „the LLM server at host:443 answered 401
  …", Vorschlag `humanitl llm key set`. Kein `curl`, kein falscher Pfad.
- `models` mit `200` (mit Schlüssel): Erfolg als `openai_compatible`, kein Befund außer
  gegebenenfalls `LLM_006`.
- Den `curl` gibt es nur bei `LLM_001` aus „nicht erreichbar" oder „nicht fertig geworden" (Frist,
  Verbindungsfehler, der keine unbekannte Zertifikatsstelle ist). Dort stand **fest**
  `target.url(OLLAMA_TAGS_PATH)`, gleich welche der beiden Anfragen scheiterte. Bei einem
  Endpunkt mit Präfix war das immer `/api/api/tags`. Welche Anfrage im Befund vom 2026-10-20
  scheiterte, nennt das `why` des damaligen Befunds (`… did not answer: …` oder
  `no answer within … ms`); der Vorschlag war in jedem Fall falsch.

Dieselbe Lücke hatte die Durchreichregel (`OpenCodeAdapter::llm_passthrough`): Sie zählt die
Inferenz-Endpunkte als `/v1/chat/completions` und so fort ab der Wurzel des Hosts auf. Hinter
einem Präfix schickt der Agent an `/api/v1/chat/completions`; keine Durchreiche trifft das, und
jede Inferenz ginge in die Warteschlange. Die `baseURL` in `opencode.json`
(`OpenCodeAdapter::base_url`) war dagegen schon richtig, nur eine zweite, eigene Lesart.

### Ziel
`llm.endpoint` ist die Basis-Adresse der OpenAI-kompatiblen API, wie `OpenCode` eine `baseURL`
liest. Probe, Agent-Konfiguration, Durchreichregel und jeder Vorschlag zum Abschreiben lesen ihn
über dieselbe Funktion, und kein Befund nennt eine Adresse, die es nicht gibt.

### Entscheidungen
- **Eine Lesart, in `humanitl-config`.** Neues Modul `humanitl_config::llm_endpoint` neben
  `LlmConfig`: `openai_base_path`, `openai_base_url`, `openai_path`, `path_prefix`,
  `serves_native_ollama`. Endet der Pfad auf `/v1` (mit oder ohne Schrägstrich), ist er die
  Basis; sonst kommt `/v1` dazu. `humanitl-config` ist die eine Crate, die Proxy und Sandbox
  beide schon kennen; eine eigene Crate oder ein Port wären Abstraktion ohne zweiten Nutzer.
  `OpenCodeAdapter::base_url` entfällt, die Aufrufer nehmen `llm_endpoint::openai_base_url`.
- **Die Ollama-Frage nur an der Wurzel des Servers, mit Präfix gar nicht.** Die native
  Ollama-API (`/api/tags`) gibt es nur an der Wurzel eines Ollama-Servers. Mit dem Präfix
  verkettet ergibt sie Pfade wie `/api/api/tags`, die kein Server kennt. An die Wurzel des Hosts
  (`<scheme://host:port>/api/tags`) geht sie auch nicht: Ein Präfix heißt Reverse-Proxy mit
  Verteilung nach Pfaden, die Wurzel gehört womöglich einem anderen Dienst, und ein Ollama dort
  wäre nicht der Server, mit dem der Agent später redet (`OpenCode` fragt `<Basis>/…`). Die
  Probe fragte damit eine Adresse, die der Mensch nicht eingetragen hat. Ein Ollama hinter einem
  Präfix wird trotzdem erkannt, als `openai_compatible` über seine `/v1/models`, und das ist die
  Oberfläche, die der Agent benutzt. `…/v1` ohne weiteres Präfix ist kein Präfix: Dort fragt die
  Probe wie bisher erst `/api/tags`, dann `/v1/models`.
- **Der `curl` nennt die Modellliste unter der Basis.** `LLM_001` (nicht erreichbar, nicht fertig)
  schlägt `curl -sS <scheme://host:port><Basis>/models` vor: die Adresse, an der die Probe
  entscheidet und die der Agent benutzt. Ollama beantwortet sie auch, der Vorschlag passt also
  für beide Arten. Das `why` nennt bei einem Verbindungsfehler die volle Adresse, die nicht
  antwortete.
- **`LLM_002` nennt die Adresse, die `401`/`403` sagte**, mit Vorschlag `humanitl llm key set`
  wie bisher. Ein `401` ist nie „nicht erreichbar".
- **`LLM_003` nennt genau die gefragten Pfade**, bei einem Endpunkt mit Präfix also nur
  `<Basis>/models`, und rät, die Basis-Adresse der OpenAI-kompatiblen API einzutragen.
- **Durchreiche nach derselben Regel.** `passthrough_prefixes(endpoint, prefixes)`: Das nackte
  `/v1/` wird zu den fünf Inferenz-Endpunkten **unter der Basis** (`/api/v1/chat/completions`
  …), das nackte `/api/` zu den acht Ollama-Endpunkten nur bei einem Endpunkt an der Wurzel;
  hinter einem Präfix fällt es weg. Der Rückfall bei leerer Liste folgt derselben Regel. Ein
  ausdrücklich geschriebener Pfad (`/v1/files`) bleibt wörtlich. Für einen Endpunkt ohne Präfix
  ändert sich nichts. Die Regel wird nicht breiter, sie zählt dieselben Endpunkte dort auf, wo
  der Server sie anbietet; `docs/SECURITY.md` 3.1 sagt das.
- **Erzeugte Inferenz-Endpunkte gelten genau** (Review Codex). Bis hierher waren sie Präfixe,
  `POST /v1/models/delete` (und hinter dem Präfix `/api/v1/models/delete`) ging ungehalten
  durch. Ein Eintrag in `Matcher::path_prefixes`, der auf `?` endet
  (`humanitl_core::EXACT_PATH_MARK`), trifft nur genau seinen Pfad, mit beliebiger Query; kein
  Unterpfad, kein abschließender `/`, keine kodierte Schreibweise (`%2F`), nichts wird dekodiert.
  Das Zeichen taugt, weil der verglichene Pfad (vor dem ersten `?`) es nie enthält: Ein solcher
  Eintrag traf vorher nichts; einzig ein solcher handgeschriebener, bisher toter Eintrag trifft
  jetzt genau seinen Pfad, und der Parser von `rules.yaml` warnt davor (`RULES_005`, Warnung).
  Proto, YAML und
  Oberfläche tragen ihn ohne neues Feld. Ein neues Feld neben `path_prefixes` hätte Proto,
  Dart-Bindungen, Parser und Konvertierung berührt, und ein vergessener Rückweg hätte die Grenze
  still verloren. `passthrough_prefixes` erzeugt alle dreizehn Endpunkte als genaue Einträge;
  was der Mensch selbst schreibt, bleibt Präfix, ein Eintrag mit `?` fällt dort weg.
  `GET /v1/models/<id>` geht damit durch die Warteschlange; `OpenCode` fragt nur die Liste.
- **Der `curl` nennt den Pfad, an dem die Probe scheiterte** (Review Codex). Fällt an der Wurzel
  schon `/api/tags` aus, wurde `/v1/models` nie gefragt; der Befund nennt dann `/api/tags`.
  Scheitert die Probe vor jeder Anfrage (Auflösung), nennt er keinen Pfad und schlägt
  `getent hosts <host>` vor.
- **Kein Teil des eingetragenen Textes, der nicht hinausging, steht im Befund** (Review Codex).
  `LLM_003` baut seine Adressen aus dem Ziel und den gefragten Pfaden; `LLM_006` und `LLM_007`
  zeigen den Endpunkt über `llm_endpoint::display`, ohne Benutzer, Passwort, Query und Fragment.
- **Kopier-Befehle als Wörter der Shell** (Review Codex, Runde 2). Der `curl` von `LLM_001` und
  `LLM_004` und `getent hosts` gehen durch `humanitl_core::shell::shell_word`; ein Pfad wie
  `/api;id;/v1` oder `/$(id)/v1` führt beim Einfügen keinen zweiten Befehl aus.
- **Keine Zugangsdaten in Notiz und `LLM_004`** (Review Opus, Runde 2): Die Notiz der
  Durchreichregel nennt `llm_endpoint::display`, der `curl` von `LLM_004` die Basis aus
  `llm_endpoint::without_secrets`. In `opencode.json` bleibt die Basis, wie sie eingetragen ist.
- **Oberfläche** (Review Codex, Runde 2): `ruleMatchSummary` und der Regel-Editor zeigen einen
  genauen Eintrag als „exact `/v1/models`" beziehungsweise in der Zeile „Exact paths", ohne `?`
  und nicht als Präfix; die Daten bleiben unverändert. Drei neue ARB-Texte, angehängt.
- **Discover** (`llm_discover.rs`) fragt nur `http://<adresse>:<port>` ohne Pfad und geht über
  dieselbe Probe; dort ändert sich nichts.

### Betroffene Pfade
- `daemon/crates/config/src/llm_endpoint.rs` (neu), `daemon/crates/config/src/lib.rs`
- `daemon/crates/core-types/src/rule.rs`, `daemon/crates/core-types/src/lib.rs`
  (`EXACT_PATH_MARK`, `exact_path_entry`, `exact_path_of`)
- `daemon/crates/rules/src/path.rs` (`prefix_matches` mit genauen Einträgen)
- `daemon/crates/proxy/src/meta.rs` (genauer Eintrag als Pfad, nicht als `**`)
- `daemon/crates/core-types/src/diagnostics/codes.rs` (nur `LLM_003`), `docs/DIAGNOSTICS.md`
  (erzeugt), `app/l10n/app_{en,de}.arb` (nur `diagLLM003Why`), `docs/cli.md`,
  `docs/THREAT-MODEL.md` (K-02, Änderungshistorie)
- `daemon/crates/rules/src/parse.rs`, `daemon/crates/rules/tests/parse.rs` (Warnung `RULES_005`)
- `app/lib/core/text/rule_sentence.dart`, `app/lib/features/rules/widgets/rule_editor.dart`,
  `app/test/features/rules/rule_text_test.dart`, `app/test/features/rules/rule_editor_test.dart`
- `daemon/crates/proxy/src/llm_probe.rs` (`Target`, `Failure::NeedsAuth`, `unknown_api`,
  `OPENAI_MODELS` statt `OPENAI_MODELS_PATH`)
- `daemon/crates/proxy/tests/llm_probe.rs`
- `daemon/crates/sandbox/src/agent/opencode.rs` (`base_url` entfällt, `passthrough_prefixes`)
- `daemon/crates/sandbox/tests/opencode_adapter.rs`
- `daemon/bin/humanitl/src/cli.rs`, `daemon/bin/humanitl/src/cmd/llm.rs`,
  `daemon/bin/humanitl/src/cmd/doctor.rs` (Ankündigung und Hilfetexte der Probe)
- `docs/SECURITY.md` 3.1

### Tests
- `config/src/llm_endpoint.rs`: Basis und Basis-Adresse für Wurzel, `/`, `/v1`, `/v1/`,
  `/openai`, `/api/v1`, `/api/v1/`; Query und Fragment gehören nicht zur Basis; Präfix und
  `serves_native_ollama` je Form.
- `proxy/src/llm_probe.rs`: `the_models_path_lies_under_the_base_and_tags_only_at_the_root`
  (kein gefragter Pfad enthält `/api/api/`), `the_probe_only_ever_gets_two_fixed_paths`.
- `proxy/tests/llm_probe.rs` mit einem Fake hinter `/api` (alles außer `/api/v1/models` ist `404`
  mit `{"detail":"Not Found"}`):
  - `a_prefixed_endpoint_is_asked_under_its_base`: `/api/v1/` und `/api/v1` mit Schlüssel,
    `openai_compatible`, genau eine Anfrage `/api/v1/models`.
  - `a_prefixed_endpoint_without_a_key_asks_for_the_key`: `LLM_002`, `why` mit der models-Adresse
    und „none was sent", kein „could not reach", kein `/api/api/`, Vorschlag
    `humanitl llm key set`.
  - `a_silent_prefixed_endpoint_gets_a_curl_on_its_model_list`: `LLM_001`, `curl` genau auf
    `/api/v1/models`.
  - `an_unknown_prefixed_endpoint_names_only_the_path_it_asked`: `LLM_003` nennt
    `/other/v1/models`, nicht `/api/tags`.
  - `a_trailing_v1_without_a_slash_is_no_prefix`: OpenAI-Fake unter `…/v1` wird erkannt.
  - `probe_timeout_llm_001` erwartet jetzt `curl … /v1/models`; die Tests für Ollama, OpenAI,
    `401` und `…/v1` mit Ollama ohne Präfix bleiben unverändert grün.
- `sandbox/tests/opencode_adapter.rs`: `opencode_keeps_the_path_prefix_of_the_endpoint`
  (`baseURL` und `curl` von `LLM_004` je Form), `opencode_passthrough_follows_the_path_prefix`
  (Inferenz unter `/api/v1/` frei, `/api/v1/files`, `/v1/chat/completions`, `/api/chat`,
  `/api/api/tags` gehalten); in `opencode.rs`
  `behind_a_path_prefix_inference_lies_under_the_base`.
- `rules/src/path.rs`: `an_exact_entry_matches_only_its_own_path` (Query ja; Unterpfad, `/` am
  Ende, `%2F`, `%2f`, `//`, Großschreibung nein; Präfix daneben bleibt Präfix).
- Durchreiche: `/v1/models/delete`, `/v1/models%2Fdelete`, `/v1/chat/completions/` und unter
  dem Präfix `/api/v1/models/delete`, `/api/v1/models/m`, `/api/v1/models/./delete` gehalten,
  `/api/v1/chat/completions?x=1` und `/v1/chat/completions?x=1` frei;
  `a_written_entry_with_a_question_mark_is_dropped`.
- Probe: `the_curl_names_the_path_that_went_unanswered` (tags `404`, models schweigt: `curl` auf
  `/v1/models`), `probe_timeout_llm_001` (tags schweigt: `curl` auf `/api/tags`),
  `a_name_that_does_not_resolve_names_no_path` (`getent hosts`),
  `a_token_in_the_endpoint_appears_in_no_diagnostic` und
  `no_diagnostic_repeats_a_secret_from_the_endpoint` (`?token=geheim`, `user:pw@`, Fragment).
- Mutationsbeweis je Test, siehe Akzeptanzkriterien.

### Akzeptanzkriterien
- [ ] Endet der Pfad des Endpunkts auf `/v1` (mit oder ohne `/`), ist `<Endpunkt>/models` die
  Probe, sonst `<Endpunkt>/v1/models`; Probe und `OpenCode`-Konfiguration benutzen dieselbe
  Funktion.
- [ ] Bei einem Endpunkt mit Präfix fragt die Probe `/api/tags` weder mit dem Präfix noch an der
  Wurzel des Hosts.
- [ ] Kopier-Befehl und `why` nennen die Adresse, die entscheidet; nie `/api/api/`.
- [ ] Die Ankündigung der Kommandozeile vor der Probe nennt genau die Pfade, die die Probe
  fragt; die Hilfetexte stimmen für Endpunkte mit und ohne Präfix.
- [ ] `401`/`403` ist `LLM_002` mit Hinweis auf `humanitl llm key set`, nicht „nicht erreichbar".
- [ ] Die Durchreiche deckt hinter einem Präfix die Inferenz unter der Basis, und nichts sonst
  zusätzlich; ohne Präfix bleibt sie unverändert.
- [ ] Endpunkte ohne Präfix (Ollama, OpenAI-kompatibel, `…/v1`) verhalten sich wie vorher.
- [ ] Die erzeugten Inferenz-Endpunkte der Durchreiche gelten genau; `/api/v1/models/delete` und
  `/v1/models/delete` werden gehalten, `/api/v1/chat/completions?x=1` geht durch. Vom Menschen
  eingetragene Pfade bleiben Präfixe.
- [ ] Der `curl` von `LLM_001` fragt den Pfad, an dem die Probe scheiterte; vor jeder Anfrage
  keinen.
- [ ] Kein Befund der Probe enthält Benutzer, Passwort, Query oder Fragment des Endpunkts.
- [ ] Mutationsbeweis je neuem Test.

### Fallstricke
- Ollama beantwortet auch `/v1/models`. Die Reihenfolge „erst `/api/tags`" bleibt deshalb an der
  Wurzel erhalten, sonst erschiene ein Ollama als `openai_compatible`.
- `Url::path()` liefert bei `https://host` den Pfad `/`; erst nach dem Abschneiden der
  Schrägstriche ist das die Wurzel. `https://host:443` verliert beim Serialisieren den
  Vorgabeport, `openai_base_url` ebenso wie vorher `base_url`.
- Die Ankündigung von `humanitl llm test` und `humanitl doctor --probe-llm` sagte fest „two GET
  requests, /api/tags then /v1/models"; bei einem Endpunkt mit Präfix war das falsch. Sie kommt
  jetzt aus `llm_endpoint::probe_plan`, also aus denselben Pfaden wie die Probe
  (`llm_endpoint::probe_paths`; ein Unit-Test der Probe hält beide gleich): „one GET request,
  /api/v1/models" hinter einem Präfix. Die Hilfetexte in `cli.rs` sind statisch und deshalb
  neutral gefasst (Modellliste des Endpunkts, davor `/api/tags` nur bei einem Ollama ohne
  Präfix).

### Kandidaten für neue Issues
- Ein Pfad, den der Mensch wörtlich gleich der Basis in `llm.passthrough_paths` schreibt
  (`/api/v1/` bei einem Endpunkt hinter `/api`), wird nicht wie das nackte `/v1/` auf die
  Inferenz-Endpunkte verengt, sondern bleibt ein Präfix über die ganze Fläche darunter, mit
  `POST /api/v1/files` und Co. Sinnvoll: jedes Präfix gleich `llm_endpoint::openai_base_path`
  (mit oder ohne `/`) wie `/v1/` behandeln.

### Referenzen
HUM-037 (`OpenCodeAdapter::base_url`), HUM-039 (Probe), HUM-076 (Discover), HUM-257 (Befund
vom 2026-10-20, Zertifikat), `docs/SECURITY.md` 3.1.

---

## HUM-262 · Ein Projektordner, der die eigene Konfiguration enthält, fällt erst beim Start auf
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-040, HUM-044 · Blockiert: HUM-060 (vor HUM-060 zu erledigen)

### Kontext
Am 2026-10-20 zeigt das Setup von v0.0.4 dem Projekteigentümer „Project folder ok
/home/nburkert" samt Auswahl „read only / read and write". Der Start im Sandbox-Bereich scheitert
danach mit `SANDBOX_006` „Mount not allowed": „sandbox.work_dir names /home/nburkert:
/home/nburkert contains /home/nburkert/.config/humanitl, Humanitl's own configuration directory:
config.toml, rules.yaml and the profiles". Die Weigerung ist richtig und bleibt; der Agent darf die
eigene Konfiguration nie lesen oder schreiben (`README.md`, Sicherheitsaussage). Falsch ist, dass
das Setup den Ordner grün meldete und dass die Karte keinen Ausweg anbot.

Ursache: Ohne `sandbox.work_dir` nahm der Daemon sein eigenes Arbeitsverzeichnis als Vorgabe
(`daemon/crates/ipc/src/sandbox.rs`, `work_dir`). Als Nutzerdienst startet systemd ihn im
Heimatverzeichnis, eine Anwendung aus dem Menü des Desktops ebenso. `Sandbox(Status)` und
`Sandbox(Plan)` prüften nur einen Ordner, der über die RPC hereinkam (`check_work_dir`); die
Vorgabe und ein Ordner aus `config.toml` liefen ungeprüft bis zum Start, wo
`MountPolicy::check_work_dir` im Sandbox-Backend sie ablehnte. Der Befund trug außerdem keine
Abhilfe (`deny` in `daemon/crates/sandbox/src/profile.rs`).

### Ziel
Das Setup meldet einen Projektordner, den der Start ablehnt, schon vor dem Druck auf Start, mit
demselben Befund und mit einer Abhilfe, die direkt einen anderen Ordner wählen lässt. Die Vorgabe
ist nie ein Ordner, den der Start ablehnt.

### Entscheidungen
- **Eine Regel, eine Quelle.** `MountPolicy::check_work_dir` ist die Regel des Starts und wird
  jetzt auch im Plan angewandt (`prepare_keyed`), für jeden nicht leeren Ordner und bei einem Start
  immer. Damit melden `Sandbox(Status)` und `Sandbox(Plan)` denselben `SANDBOX_006` wie der Start,
  und der Client urteilt weiter über nichts (ADR-018).
- **Die Vorgabe geht durch dieselbe Regel.** Ohne `sandbox.work_dir` bleibt das Arbeitsverzeichnis
  des Daemons die Vorgabe, aber nur, wenn `MountPolicy::check_work_dir` es annimmt und es ein
  Verzeichnis ist; sonst gibt es keine Vorgabe. Der leere Ordner ist die bekannte offene Wahl: Die
  Momentaufnahme nennt keinen, und das Setup zeigt `CONFIG_013` und verlangt eine Auswahl. Bewusst
  nicht die drei Stufen von `check_work_dir` im IPC-Crate (die dritte verlangt einen Ordner unter
  dem Heimatverzeichnis): Eine neue Regel ist kein Ziel, nur dieselbe früher.
- **Ein Ordner aus `config.toml`** bleibt sichtbar, wenn er abgelehnt wird: Der Befund steht über
  der Momentaufnahme `failed`, die ihn weiter nennt, damit die Karte einen Bezug hat.
- **Satz und Abhilfe am Befund.** `MountPolicy::check_work_dir` hängt an das `why` „choose the
  folder of a single project instead, such as ~/projects/xyz" und setzt
  `ChangeSetting { key: "sandbox.work_dir", value: "" }`, wie es `work_dir_refused` für die übrigen
  Stufen schon tat. Eine Quelle aus dem Profil (`MountPolicy::check`) bekommt weder Satz noch
  Abhilfe; sie ändert sich im Profil.
- **Die Abhilfe im Client.** `FixControl` bekommt `onChooseFolder`. Ein `ChangeSetting` auf
  `sandbox.work_dir` wird dort, wo ein Bildschirm den Picker hat, ein Knopf „Choose another
  folder", der denselben Dialog öffnet wie der Picker (`chooseWorkDir` in
  `app/lib/core/ui/work_dir_picker.dart`, aus `WorkDirPicker._choose` herausgezogen). Das gilt
  im Setup und im Sandbox-Bereich, der im Kopf denselben Picker hat; dort plant der Knopf den
  gewählten Ordner über `sandboxStatusProvider.notifier.plan`. Ohne Picker bleibt das Abzeichen.
  Solange eine Sandbox läuft oder ein Start oder Stopp unterwegs ist, gibt es den Knopf nicht, wie
  beim Picker.
- **Andere sensible Elternordner** deckt die Regel schon ab: Ein Ordner, der `~/.ssh`, `~/.gnupg`,
  `~/.gitconfig`, `~/.netrc`, `$XDG_CONFIG_HOME/humanitl`, `$XDG_DATA_HOME/humanitl` oder das
  Laufzeitverzeichnis enthält, ist abgelehnt, ebenso das Heimatverzeichnis selbst und alles
  darüber (`FORBIDDEN_IN_HOME`, `FORBIDDEN_MOUNTS`, `with_protected_dir`). `~/.config` enthält
  `~/.config/humanitl` und fällt damit darunter. Mit diesem Issue meldet das Setup jeden dieser
  Fälle genauso früh, weil Plan und Vorgabe durch dieselbe Prüfung gehen.

### Tests
- `daemon/crates/sandbox/src/profile.rs`: `the_work_dir_may_live_under_home_but_not_be_it` prüft
  zusätzlich Satz und Abhilfe für jeden abgelehnten Ordner;
  `a_home_that_holds_the_config_dir_is_refused_for_that_reason` (Fall vom Bildschirmfoto);
  `a_profile_source_carries_no_folder_fix`.
- `daemon/crates/ipc/tests/sandbox_rpc.rs`:
  `a_configured_folder_that_holds_the_config_is_refused_by_the_plan`: `Sandbox(Status)` und
  `Sandbox(Plan)` melden für ein `sandbox.work_dir`, das die Konfiguration enthält, `SANDBOX_006`
  mit Satz und `ChangeSetting` auf `sandbox.work_dir`, und der Zustand `failed` nennt den Ordner.
- `daemon/crates/ipc/src/sandbox.rs`: `the_default_folder_is_never_one_the_start_refuses`: das
  Heimatverzeichnis, `/`, ein Ordner, der das Konfigurationsverzeichnis ist, kein und ein fehlendes
  Arbeitsverzeichnis ergeben keine Vorgabe; ein Projektordner bleibt die Vorgabe. Das
  Arbeitsverzeichnis ist über `work_dir_with_cwd` eingespeist, damit der Test nicht vom
  Arbeitsverzeichnis von `cargo test` abhängt.
- `app/test/features/setup/project_refused_test.dart` mit einem Fake, der wie der Daemon antwortet:
  Setup mit Heimatverzeichnis zeigt `SANDBOX_006`, sperrt den Start und trägt den Knopf; der Knopf
  öffnet die Ordnerauswahl und plant den gewählten Ordner, die Zeile wird grün; ein abgebrochener
  Dialog ändert nichts; das Heimatverzeichnis über den Picker gewählt ergibt dieselbe Karte; bei
  laufender Sandbox gibt es keinen Knopf, nur das Abzeichen.
- `app/test/features/sandbox/workdir_fix_test.dart`: Im Sandbox-Bereich öffnet der Knopf die
  Ordnerauswahl und plant den gewählten Ordner; bei laufender Sandbox fehlt er.
- Mutationsbeweise: Prüfung in `prepare_keyed` entfernt; Vorgabe wieder ungeprüft; Abhilfe oder
  Satz in `MountPolicy::check_work_dir` entfernt; `FixControl` zeigt immer das Abzeichen;
  `SetupCheckRow` reicht `onChooseFolder` nicht weiter; der Knopf ruft `onPick` nicht; der
  Sandbox-Bereich übergibt keinen Callback; Sperre im Sandbox-Bereich oder im Setup entfernt;
  `work_dir_with_cwd` nimmt das Arbeitsverzeichnis ungeprüft. Jede Mutation macht mindestens
  einen der Tests rot.

### Akzeptanzkriterien
- [ ] Ein Projektordner, den der Start mit `SANDBOX_006` ablehnt, steht im Setup schon vor dem
  Start als `SANDBOX_006` in der Projektzeile, und der Start-Knopf ist aus.
- [ ] Die Karte trägt ein `why`, das in einem Satz sagt, warum der Ordner nicht geht und was
  stattdessen, und einen Knopf, der die Ordnerauswahl öffnet.
- [ ] Ohne `sandbox.work_dir` ist die Vorgabe nie ein Ordner, den der Start ablehnt; ohne gültige
  Vorgabe zeigt das Setup `CONFIG_013` statt „ok".
- [ ] Die Prüfung liegt nur im Daemon; der Client urteilt nicht über den Ordner.
- [ ] Neue Texte in beiden ARB-Dateien, nur angehängt; `make l10n-lint` grün.

### Fallstricke
- **Ein leerer Ordner ist im Plan kein Befund.** Wer ihn im Plan als `SANDBOX_006` meldet, ersetzt
  die Karte „Kein Projektordner gewählt" durch eine Weigerung über einen Ordner, den niemand
  gewählt hat. Nur ein Start ohne Ordner ist ein Befund.
- **Nicht die dritte Stufe für die Vorgabe.** `check_work_dir` im IPC-Crate verlangt für einen
  Wunsch über die RPC einen Ordner unter dem Heimatverzeichnis. Für die Vorgabe wäre das eine neue
  Regel, die Abläufe mit einem Arbeitsverzeichnis außerhalb von `$HOME` brechen ließe.
- **`ChangeSetting` auf `sandbox.work_dir` schreibt nichts.** `SetConfig` lehnt den Schlüssel bis
  HUM-069 mit `CONFIG_014` ab; die Abhilfe wird deshalb über die Ordnerauswahl und
  `Sandbox(Plan)` ausgeführt, nie über `SetConfig`.
- **Gemeinsame Dateien:** ARB nur angehängt. In `codes.rs` ändert sich nur die Spalte „Fix" von
  `SANDBOX_006`, die seit `work_dir_refused` „Kein Fix" behauptete; `docs/DIAGNOSTICS.md` ist mit
  `UPDATE_DIAG_DOCS=1` neu erzeugt.

### Nicht-Ziele
- Neue Regeln für Projektordner.
- Die Prüfsumme des Arbeitsverzeichnisses in `session.started` (`humanitld`, `work_dir_of`), die
  ohne `sandbox.work_dir` weiter das Arbeitsverzeichnis des Daemons nimmt; Kandidat für ein
  eigenes Issue.

### Referenzen
HUM-040 (`Sandbox(Plan)`, Picker), HUM-044 (Setup, `CONFIG_013`), HUM-011 (`MountPolicy`),
`docs/UX.md` 4.4, ADR-018, `daemon/crates/sandbox/src/profile.rs`
(`MountPolicy::check_work_dir`), `daemon/crates/ipc/src/sandbox.rs` (`prepare_keyed`,
`default_work_dir`).

## HUM-263 · Zwischen Prüfung und Mount kann der Projektordner getauscht werden
Sprint: 5 · Größe: M · Abhängigkeiten: HUM-011, HUM-262 · Blockiert: HUM-060 (vor HUM-060 zu erledigen)

### Kontext
Gefunden im Review von HUM-262 (Codex, Opus). Das Sandbox-Backend prüft den Projektordner per Pfad
(`daemon/crates/sandbox/src/bwrap.rs`, `policy.check_work_dir(&session.work_src)` vor dem Start)
und bindet ihn danach erneut per Pfad (`daemon/crates/sandbox/src/bwrap_args.rs`,
`args.bind(work_flag, &ctx.work_src, work_dst)`); `bwrap` löst den Pfad erst beim Einhängen auf.
Wer den Elternordner kontrolliert, kann ihn zwischen Prüfung und Mount gegen einen Symlink auf
`~/.config/humanitl` oder `~/.ssh` tauschen: ein anderer lokaler Nutzer mit Schreibrecht auf den
Elternordner (Angreifer (d) im Threat Model), oder der Agent selbst, wenn der neue Projektordner in
einem Ordner liegt, den eine frühere Sitzung beschreibbar eingehängt hatte. Die Sandbox sähe dann
genau das, was die Prüfung ausschließen soll. Das berührt die Sicherheitsaussage in `README.md`.

### Ziel
Eingehängt wird genau das Verzeichnis, das geprüft wurde, nicht ein Pfad, der zur Zeit der Prüfung
auf es zeigte.

### Fix-Idee
- Den Ordner nach der Auflösung einmal öffnen (`O_PATH | O_DIRECTORY | O_NOFOLLOW` auf jeder
  Komponente, etwa über `openat2` mit `RESOLVE_NO_SYMLINKS` oder schrittweise) und den Deskriptor
  halten.
- Die Prüfung auf dem geöffneten Deskriptor fahren: den echten Pfad aus `/proc/self/fd/N` lesen
  und gegen `MountPolicy::check_work_dir` prüfen.
- Genau diesen Deskriptor binden: `bwrap --bind-fd`/`--ro-bind-fd`, falls die Mindestversion es
  trägt, sonst `/proc/self/fd/N` als Quelle mit vererbtem Deskriptor.
- `docs/SECURITY.md` und `docs/THREAT-MODEL.md` nennen die Lücke und ihren Abschluss im selben
  Commit.

### Tests
- Ein Test-Hook zwischen Prüfung und Start tauscht den geprüften Ordner gegen einen Symlink auf
  ein geschütztes Verzeichnis; die Sandbox sieht danach das alte Verzeichnis oder startet nicht,
  nie das geschützte.
- Mutationsbeweis: Bindung wieder per Pfad, der Test wird rot.

### Akzeptanzkriterien
- [x] Der Projektordner wird per Deskriptor geprüft und gebunden; ein Tausch zwischen Prüfung und
  Mount erreicht kein geschütztes Verzeichnis.
- [x] `docs/SECURITY.md` und `docs/THREAT-MODEL.md` beschreiben den Fall.

> **Umsetzung (HUM-263, 2026-10-21).** Neues Modul `daemon/crates/sandbox/src/pinned.rs`:
> `PinnedSource::open` löst den Pfad auf, öffnet das Ergebnis mit `O_PATH | O_NOFOLLOW |
> O_CLOEXEC` (Projektordner zusätzlich `O_DIRECTORY`), prüft per `fstat` die Art (ein Symlink an
> der Stelle ist ein Tausch) und vergleicht den Ort aus `/proc/self/fd/N` mit dem aufgelösten
> Pfad; weicht er ab, bricht der Plan mit `SANDBOX_006` ab. `BwrapBackend::plan` prüft wie bisher
> den geschriebenen Pfad, öffnet dann den Ordner, prüft `MountPolicy::check_work_dir` auf dem Ort
> des Deskriptors und rechnet auch `present_under_work` über `/proc/self/fd/N`, damit die Masken
> zum eingehängten Verzeichnis passen. Der Deskriptor kommt in die vererbte Liste;
> `LaunchInputs::work_dir` rendert `--bind-fd`/`--ro-bind-fd` ab bubblewrap 0.10
> (`BIND_FD_VERSION`, eingeführt gegen CVE-2024-42472; lokal 0.13.1, Mindestversion bleibt 0.8),
> darunter `--bind /proc/self/fd/N` (`SourceFd::ProcSelf`). Nach dem Start schließt
> `supervise` die eigene Kopie wie jeden geerbten Deskriptor. Vorschau und Schnappschuss
> (`LaunchInputs::preview`) zeigen weiter die Pfade.
>
> **Der Deskriptor darf den Agenten nie erreichen.** Gemessen: Ein geerbter
> Verzeichnis-Deskriptor des Hosts öffnet in der Sandbox mit `cat /proc/self/fd/9/../x` Dateien
> neben dem Projektordner. `--bind-fd` schließt ihn in `bwrap`; beim Rückfall über
> `/proc/self/fd/N` lässt `bwrap` ihn offen, und der Shim schließt ihn vor dem Start des Agenten
> (`close_inherited`, Test `the_parent_drops_what_the_launcher_leaked` im Shim-Crate).
>
> **Weitere Mounts, die der Nutzer wählt.** `mounts.extra_ro` und `mounts.extra_rw` hatten
> dieselbe Lücke (Quelle außerhalb von `$HOME`, Elternordner etwa unter `/srv` oder im
> beschreibbaren `extra_rw` einer anderen Sitzung) und gehen denselben Weg, als Verzeichnis oder
> gewöhnliche Datei, geprüft mit `MountPolicy::check` auf dem Ort des Deskriptors. Eine solche
> Quelle, die beim Planen fehlt, lehnt der Plan jetzt mit `SANDBOX_006` ab; vorher scheiterte
> `bwrap` erst beim Start. Deshalb nehmen `humanitl sandbox run --tests-dir` und `escape-launch
> --tests-dir` den Platzhalter `/tests/escape` vor dem Planen aus dem Profil und geben das
> Verzeichnis dem Backend als nur lesbaren Bind der Sitzung (`BwrapBackend::with_read_only_bind`):
> geöffnet, mit `MountPolicy::check_session_dir` (Regeln des Projektordners, ohne dessen Abhilfe)
> geprüft und per Deskriptor gebunden, hinter den Erweiterungen, vor dem Projektordner.
> `mounts.ro` (Systemverzeichnisse, Elternordner gehört root) bleibt per Pfad.
>
> **Nachtrag aus dem Review (Codex, Opus).** `escape-launch` verweigert den Start ohne brauchbaren
> Shim, wenn `bwrap` älter als 0.10 ist (`refuse_unclosed_descriptors`): Dann schlösse niemand den
> Deskriptor. `supervise` nimmt `FD_CLOEXEC` nicht mehr im eigenen Prozess unter einem Riegel weg,
> sondern nur im Kind, zwischen `fork` und `exec` (`daemon/crates/sandbox/src/inherit.rs`,
> `CommandExt::pre_exec`); das gilt für alle vererbten Deskriptoren, Pipes und memfds eingeschlossen.
> Andere Starts im Daemon (Doctor-Proben, Secret-Helper, Werkzeuge der Zusammenfassung) erben sie
> damit nicht mehr. Dafür ist `#![forbid(unsafe_code)]` der Crate `#![deny(unsafe_code)]` geworden,
> mit `#![allow(unsafe_code)]` nur in `inherit.rs`, nach dem Vorbild von `humanitld/systemd.rs`
> (CONVENTIONS 4.34); `command-fds` täte an derselben Stelle dasselbe. Jeder vererbte Deskriptor
> liegt über den Standardströmen (`above_stdio`, `F_DUPFD_CLOEXEC` ab 3), weil die stdio des Kindes
> 0 bis 2 vor dem Hook per `dup2` ersetzt; `inherit_in_child` nimmt `BorrowedFd` und lehnt
> Nummern unter 3 ab. Scheitert der Hook (`EBADF`, das `execve` nie liefert), meldet der Start
> `SANDBOX_012` statt „cannot execute“ mit Installationshinweis. Test
> `a_closed_standard_stream_is_never_handed_on` (im Kindprozess mit geschlossenem Deskriptor 0);
> Mutation ohne `above_stdio` in `PinnedSource::open` macht ihn rot („pinned on 0“).
>
> Tests: `pinned::tests` (Tausch nach dem Öffnen, Symlink-Ordner, fehlend, Datei, Socket),
> `bwrap_args::tests::opened_sources_are_bound_by_descriptor_and_the_preview_by_path`,
> in `tests/launcher.rs` `a_folder_swapped_after_the_check_stays_the_checked_folder` (echtes
> `bwrap`, `--bind-fd`), `…_without_bind_fd` (dasselbe über `/proc/self/fd/N`),
> `the_folder_descriptor_does_not_reach_the_command` und
> `extra_mounts_are_bound_by_descriptor_and_must_exist`,
> `a_read_only_bind_of_the_caller_is_pinned_and_checked`, `inherit::tests` (das Kind erbt, der
> Elternprozess behält `FD_CLOEXEC`) und `without_a_shim_an_old_bwrap_is_refused` in
> `escape-launch.rs`; die Swap-Tests planen, ersetzen den
> Ordner durch einen Symlink auf das geschützte Laufzeitverzeichnis und starten dann. Dazu die
> Harness-Tests in `escape-launch.rs` und `cmd/sandbox.rs`. Mutationsbeweise: Bind wieder per
> Pfad (`bind_source` ignoriert den Deskriptor) macht beide Swap-Tests rot, die Sandbox liest
> `protected` statt `checked`; immer `/proc/self/fd/N` statt `--bind-fd` macht
> `the_folder_descriptor_does_not_reach_the_command` rot, der Befehl erbt den Deskriptor;
> `FD_CLOEXEC` wieder im Elternprozess löschen macht
> `inherit::tests::the_child_inherits_and_the_parent_keeps_cloexec` rot.
>
> Die beiden Kandidaten oben fallen nicht billig mit und bleiben stehen. Shim, CA und
> Proxy-Socket per Deskriptor: HUM-265. Neue Kandidaten:
> **Schnappschuss per Pfad.** `SummaryWatch::start` liest `work_src` zwischen Plan und Start per
> Pfad; ein Tausch in diesem Fenster ließe die Zusammenfassung einen anderen Baum vergleichen
> (Anzeige, kein Zugriff der Sandbox). Der Plan könnte den Deskriptor dafür hergeben.
> **`sandbox run --profile test` ohne `--tests-dir`** scheitert jetzt beim Planen statt beim Start,
> weil `/tests/escape` auf dem Host fehlt; der Befund könnte `--tests-dir` als Abhilfe nennen.

### Kandidaten aus demselben Review
- **Status meldet `failed` bei laufender Sitzung.** Seit HUM-262 prüft `prepare_keyed` auch den
  Ordner einer laufenden Sandbox. Wird er währenddessen zum Symlink auf ein geschütztes
  Verzeichnis, meldet `Sandbox(Status)` `SANDBOX_006` und `failed`, obwohl die Sitzung weiter
  läuft. Zu klären: Zustand aus dem Handle, Befund nur als Warnung an der laufenden Sitzung.
- **Plan-Vorschau mit leerem `work_src`.** Ohne Ordner baut der Plan trotzdem eine Kommandozeile
  mit leerer Quelle für `/work`; `Path::starts_with("")` ist für jeden Pfad wahr, und Prüfungen,
  die einen leeren Ordner als Präfix lesen, sagen dann Falsches. Die Vorschau sollte ohne Ordner
  keine Einhängung für `/work` zeigen.

### Referenzen
HUM-011 (`MountPolicy`), HUM-262, `daemon/crates/sandbox/src/bwrap.rs`,
`daemon/crates/sandbox/src/bwrap_args.rs`, `docs/THREAT-MODEL.md` (Angreifer (d)).

## HUM-265 · Shim, CA und Proxy-Socket werden weiter per Pfad gebunden
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-263 · Sicherheit · nach HUM-060

### Kontext
Gefunden im Review von HUM-263. Seit HUM-263 bindet der Launcher den Projektordner, die
Erweiterungen des Profils und `--tests-dir` per Deskriptor. Per Pfad bleiben die Quellen, die die
Sitzung selbst mitbringt: `shim_src`, `ca_cert_src`, `ca_bundle_src` und `proxy_socket_src`
(`daemon/crates/sandbox/src/bwrap_args.rs`, die vier `args.bind("--ro-bind", …)` nach dem
Projektordner). Die Dateien des Agent-Adapters sind nicht betroffen: Sie kommen als versiegelte
memfds (`--ro-bind-data`), nie als Pfad.

Geprüft am 2026-10-21:
- **CA, Bundle, Proxy-Socket** liegen unter `$XDG_DATA_HOME/humanitl` und dem
  Laufzeitverzeichnis, beide von der `MountPolicy` geschützt und vom Daemon mit `0700` angelegt.
  Ein anderes Konto kann dort nichts tauschen, der Agent erreicht beide Bäume nie (ein
  Projektordner darin wird abgelehnt). Ein Tausch setzt ein Konto voraus, das ohnehin die
  Einrichtung des Daemons ändert (`docs/THREAT-MODEL.md` 2.2).
- **Der Shim** wird zuerst neben dem eigenen Binary gesucht (`shim_path` in
  `daemon/crates/ipc/src/sandbox.rs` und `daemon/bin/humanitl/src/cmd/sandbox.rs`), dann unter
  `/usr/lib/humanitl` und Verwandten. Im Paket liegt er root-eigen unter `/usr/lib/humanitl`;
  dort tauscht ihn niemand ohne root. Im Entwicklungsfall liegt er in `daemon/target/debug/` eines
  Repos. Hatte eine frühere Sitzung dieses Repo beschreibbar als `/work` eingehängt, kann ihr Agent
  `target/debug/humanitl-shim` ersetzen, und die nächste Sitzung startet seinen Shim als PID 1.
  Der setzt dann weder den seccomp-Filter noch berichtet er ehrlich an den Isolations-Check
  (er kann `CHECK … ok` fälschen). Die Namensräume von `bwrap` bleiben: kein Netz-Interface, kein
  Mount außer den gebundenen, nur der Proxy-Socket als Tür.

### Ziel
Die Quellen der Sitzung kommen genauso per Deskriptor in die Sandbox wie der Projektordner, und
ein Shim, den die Sandbox schreiben konnte, startet nicht.

### Fix-Idee
- `shim_src`, `ca_cert_src`, `ca_bundle_src`, `proxy_socket_src` über `PinnedSource` öffnen
  (der Socket als `O_PATH` ohne `O_DIRECTORY`, Art per `fstat`) und per `--ro-bind-fd` binden.
- Den Shim zusätzlich prüfen: Eigentümer root oder das eigene Konto, nicht für Gruppe oder Andere
  schreibbar, und kein Vorfahr, den die `MountPolicy` als beschreibbaren Projektordner zuließe,
  oder einfacher: Das Shim-Binary wird vor dem Start in ein versiegeltes memfd kopiert und von dort
  gebunden (`--ro-bind-data` geht für ausführbare Dateien nicht, also `--file` plus `--chmod` oder
  der Bind des memfd über `/proc/self/fd/N`; zu messen).

### Warum nach HUM-060
Das Release liefert den Shim root-eigen unter `/usr/lib/humanitl`, und das Archiv legt ihn
außerhalb jedes Projektordners ab; der Angriff braucht einen Entwicklungsbau in einem Repo, das
eine frühere Sitzung beschreibbar eingehängt hatte. Selbst dann bleiben Netz- und
Mount-Namensraum der Sandbox stehen; verloren geht der seccomp-Filter und die Ehrlichkeit des
Isolations-Berichts. CA und Socket sind schon heute nur für das eigene Konto erreichbar. Das
rechtfertigt keinen Aufschub des Releases, wohl aber ein eigenes Issue direkt danach.

### Tests
- Plan-Test: Shim, CA, Bundle und Socket erscheinen als `--ro-bind-fd FD DST`, nie als Pfad.
- Swap-Test wie in HUM-263: nach dem Plan wird `shim_src` durch ein anderes Skript ersetzt; die
  Sandbox startet den geprüften Shim.
- Mutationsbeweis: Bind wieder per Pfad, der Swap-Test wird rot.

### Akzeptanzkriterien
- [ ] Shim, CA, Bundle und Proxy-Socket werden per Deskriptor gebunden; ein Tausch nach dem Plan
  erreicht die Sandbox nicht.
- [ ] Ein Shim in einem Verzeichnis, das eine Sandbox beschreiben konnte, wird erkannt oder kann
  nicht mehr getauscht werden; `docs/SECURITY.md` und `docs/THREAT-MODEL.md` nennen den Fall.

### Referenzen
HUM-263, HUM-012 (Shim), HUM-013 (Proxy-Socket), `daemon/crates/sandbox/src/pinned.rs`.

---

## HUM-266 · Ein deflate-Body, der als zlib und als nackter Strom Verschiedenes sagt
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-247 · Blockiert: — · nach 0.1.0

### Kontext
Gefunden im Review von HUM-247 (Opus). `Content-Encoding: deflate` ist im Netz zweideutig, und
`inflate` in `daemon/crates/findings/src/decode.rs` liest deshalb erst den zlib-Rahmen aus
RFC 1950 und nur, wenn der scheitert, den nackten Strom aus RFC 1951. Ein Body lässt sich so
bauen, dass beide Lesarten gültig sind und Verschiedenes ergeben: Der Kopf `08 1d` ist ein
gültiger zlib-Kopf (Prüfsumme über CMF und FLG stimmt) und zugleich der Anfang eines nackten
Stroms mit einem Stored-Block. Nackt gelesen folgt ein Stored-Block mit `LEN` `0x011d` und danach
ein letzter Block mit festen Huffman-Codes, der ein Geheimnis trägt. Als zlib gelesen ist
dasselbe ein Stored-Block von 65250 Bytes mit gültiger Adler-32-Summe dahinter.

`decode_body(Deflate)` nimmt die zlib-Lesart, meldet `truncated: false` und findet nichts. Ein
Empfänger, der nur den nackten Strom kennt (Python `zlib.decompressobj(-15)`, Node
`zlib.inflateRaw`), bekommt das Geheimnis. Das ist dieselbe Art stiller Lücke, die HUM-247 für
gzip-Member und angehängte Bytes schließt, und `backlog/CONVENTIONS.md` 4.13 schließt sie aus:
Wo der Scan nicht hinsieht, muss er es sagen. Die Lücke bestand schon vor HUM-247; sie liegt an
der Mehrdeutigkeit des Rahmens, nicht an einer Änderung dort. Die drei Garantien aus `README.md`
berührt sie nicht, die Funde sind keine Garantie.

### Ziel
Ein deflate-Body, den zlib-Rahmen und nackter Strom unterschiedlich lesen, wird nicht still als
vollständig durchsucht gemeldet.

### Fix-Idee
- Gelingt die zlib-Lesart, zusätzlich den nackten Strom durch dasselbe Budget lesen (eine Kopie,
  wie heute für den ersten Versuch). Gelingt auch er und ist das Ergebnis ein anderes, meldet der
  Scan `FINDINGS_002` und durchsucht beide Ergebnisse oder das längere; scheitert der nackte
  Strom, bleibt es bei der zlib-Lesart.
- Das Budget gilt für beide Lesarten zusammen oder je Lesart mit derselben Grenze; zu
  entscheiden ist, ob eine Bombe in zwei Lesarten das doppelte Entpacken wert ist. Ein Vorschlag:
  die zweite Lesart nur bis zum Ende der ersten Ausgabe vergleichen und beim ersten
  abweichenden Byte abbrechen.

### Tests
- Ein Polyglott wie oben (Kopf `08 1d`, Stored-Block, Geheimnis im festen Huffman-Block):
  `decode_body(Deflate)` meldet `truncated: true` und `FINDINGS_002`. Mutation: ohne die zweite
  Lesart wird der Test rot.
- Ein gewöhnlicher zlib-Body und ein gewöhnlicher nackter Strom bleiben vollständig und ohne
  Befund.
- Der Polyglott liegt als `body_decoder-deflate-polyglot.bin` unter
  `daemon/crates/findings/tests/regressions/` und im Corpus von `body_decoder`.

### Akzeptanzkriterien
- [ ] Ein deflate-Body mit zwei gültigen, verschiedenen Lesarten führt zu `FINDINGS_002`.
- [ ] Das Budget aus `limits.preview_cap_bytes` und `limits.max_decompress_ratio` bleibt
  eingehalten, auch über beide Lesarten.

### Kandidaten aus demselben Review
- **Den Anfang eines kaputten Stroms durchsuchen.** Bei `BrokenStream` wirft der Scan heute alles
  weg, was bis zum Abbruch entpackt wurde (`partial`), und meldet nur `FINDINGS_002`. Ein
  Empfänger, der nachsichtig liest, verarbeitet diesen Anfang trotzdem. Den Anfang zu durchsuchen
  und die Lücke weiter zu melden wäre strenger, ohne einen Fund zu erfinden.

### Referenzen
HUM-025, HUM-247, RFC 1950 §2.2, RFC 1951 §3.2.4, `backlog/CONVENTIONS.md` 4.13.
