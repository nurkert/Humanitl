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
| HUM-267 | Ein Test scheitert zufällig an „Text file busy" | S | HUM-134 |
| HUM-268 | Zwei Tasten in einem Frame speichern eine nicht gezeichnete Regel (nach 0.1.0) | S | HUM-249 |
| HUM-269 | Das Senden aus dem Editor übergeht die Pause einer gestörten Aufzeichnung | S | HUM-250, HUM-047 |
| HUM-270 | Flüsse aus einer Störung tragen keine eigene Markierung (nach 0.1.0) | S | HUM-250 |
| HUM-271 | Ein Regelvorschlag trägt einen Schlüssel aus Host oder Pfad unmaskiert | S | HUM-140, HUM-256 |
| HUM-272 | Accent-filled controls react too weakly to hover and press | S | HUM-260 |
| HUM-273 | Releases 0.0.x appear as regular releases, not as pre-releases | S | HUM-053, HUM-225 |
| HUM-277 | Saved settings take effect without restarting the daemon (before HUM-060) | M | HUM-069, HUM-140, HUM-151 |
| HUM-278 | The daemon finds agents installed in ~/.local/bin (before HUM-060) | S | HUM-037, HUM-136 |
| HUM-275 | Settings are reachable without a shortcut, and LLM_002 leads to the key field (before HUM-060) | S | HUM-069, HUM-140, HUM-254 |
| HUM-276 | The "no tray" notice can be dismissed for good and sits under the header (before HUM-060) | S | HUM-034, HUM-254 |
| HUM-279 | `humanitl sandbox attach` has a detach key (before HUM-060) | S | HUM-042 |
| HUM-280 | The agent starts from a shell inside the sandbox (before HUM-060) | M | HUM-042, HUM-278, HUM-279 |
| HUM-281 | The concurrent start test counts one sandbox twice | S | HUM-067, HUM-277 |
| HUM-288 | Setup's model probe sends the stored key (before HUM-060) | S | HUM-140, HUM-044 |
| HUM-282 | The project folder has a visible "Choose folder" button (before HUM-060) | S | HUM-262 |
| HUM-283 | Pasted endpoint and key are cleaned, and a stray quote is refused (before HUM-060) | S | HUM-039, HUM-140 |
| HUM-284 | A package update replaces the running daemon without a command (before HUM-060) | M | HUM-053, HUM-277 |
| HUM-285 | The CLI ends quietly when the pipe closes | S | — |
| HUM-289 | Several saved model servers, switchable in the app (before HUM-060) | M | HUM-140, HUM-277, HUM-283 |
| HUM-286 | One layout grid: pane headers, action bar and the rail highlight line up (before HUM-060) | M | HUM-254, HUM-275 |
| HUM-287 | Setup becomes a short onboarding, not a scroll list (before HUM-060) | M | HUM-282, HUM-286, HUM-289 |
| HUM-292 | Spike: Podman as a second sandbox backend, with and without a kernel boundary (after HUM-060, first) | S | HUM-060, HUM-145 |
| HUM-293 | One project folder, chosen once, for app and CLI alike (before HUM-060) | S | HUM-069, HUM-262, HUM-282, HUM-287 |
| HUM-274 | pi becomes a first-class agent next to OpenCode (before HUM-060) | M | HUM-037, HUM-039, HUM-140, HUM-261 |
| HUM-294 | A stored project folder gets the full folder check at every start (after HUM-060) | S | HUM-293 |
| HUM-295 | Settings re-plans the sandbox after the project folder changes (after HUM-060) | S | HUM-293 |
| HUM-296 | `humanitl run --config` with a folder outside HOME is judged against the file it names (after HUM-060) | S | HUM-293 |
| HUM-297 | RUSTSEC-2026-0414 in pulldown-cmark turns the dependency check red | S | — |
| HUM-304 | The agent knows it has web access through the proxy | S | HUM-071, HUM-274 |
| HUM-311 | The Debian changelog line stays within lintian's 80 characters for every version | S | HUM-273 |
| HUM-298 | The terminal goes full window from anywhere, by shortcut and button | S | HUM-042, HUM-286 |
| HUM-308 | Agents do not ask twice and do not give up while a request waits | S | HUM-274, HUM-304 |
| HUM-299 | A second shell in a running sandbox | M | HUM-042, HUM-279, HUM-280 |
| HUM-307 | The Intercept action bar keeps Allow and Block on one line | S | HUM-286 |
| HUM-309 | pi gets web search and fetch through pi-web-access | S | HUM-274, HUM-304, HUM-308 |
| HUM-305 | Pane boundaries can be dragged, and the sandbox terminal can grow | S | HUM-286, HUM-298 |
| HUM-303 | The terminal takes the keyboard on hover and by shortcut | S | HUM-298 |
| HUM-306 | The empty queue says the agent is working when a session runs | S | HUM-298 |
| HUM-301 | The terminal text can be zoomed | S | HUM-298, HUM-303 |
| HUM-302 | Notices float as dismissible overlays and never push the layout | S | HUM-276, HUM-286 |
| HUM-313 | Terminal copy and paste | S | HUM-042, HUM-301, HUM-303 |
| HUM-300 | Terminal as picture-in-picture | M | HUM-298, HUM-302, HUM-303, HUM-305 |
| HUM-314 | The terminal pane opens further shells as tabs | S | HUM-299, HUM-300, HUM-302, HUM-303, HUM-313 |
| HUM-315 | The floating terminal docks on the Sandbox page, floats from the empty queue and keeps a title strip | S | HUM-300, HUM-303, HUM-306, HUM-314 |
| HUM-350 | Disposable sandbox: the project folder as an overlay, with a diff, apply and discard | L | HUM-043, HUM-263, HUM-299 |
| HUM-351 | The app shows the changes of a disposable sandbox and applies or discards them | M | HUM-350 |
| HUM-352 | The floating terminal keeps its resize grip in the corner and grows equally in every corner | M | HUM-300, HUM-314, HUM-315 |

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

### Umsetzung (HUM-059)

Alle Dokumente sind englisch, außer `README.de.md` (eine Seite) und den beiden
erzeugten Registern, deren Quelle deutsch ist (siehe Abweichungen).

- **README.md** mit den zwölf Pflichtabschnitten: Einzeiler und Screenshot vor
  der ersten `##`-Überschrift, danach die zehn Überschriften `Why` bis
  `License` in der Reihenfolge der Spezifikation. `README.de.md` als Kurzfassung.
  `docs/README.md` ist neu, ein Wegweiser über alle Seiten.
- **docs/SECURITY.md** und **docs/THREAT-MODEL.md** final, englisch, in der
  Gliederung der Spezifikation (elf Abschnitte; Assets, Attacker, Trust
  boundaries, Attack surface mit zwölf Zeilen und Status nur
  `mitigated`/`declared`/`open`, Out of scope, Residual risks). Gegen den Code
  geprüft: Shim-Reihenfolge (`main.rs`: Brücke, Schließen und Fork,
  `PR_SET_DUMPABLE` im Elternprozess, `PR_SET_NO_NEW_PRIVS`, Filter mit
  Zuhörer ohne `TSYNC`, Rückfall mit `TSYNC`, `execvp`), Syscall-Liste aus
  `seccomp.rs` (`FLOOR` und `SYSCALLS`), Pfade aus `config/src/paths.rs` und
  dem Sandbox-Profil, HUM-263, HUM-257, HUM-247, HUM-250, systemd-Unit.
  Korrigiert gegenüber dem Entwurf: Shim liegt in der Sandbox unter
  `/run/humanitl/humanitl-shim` (nicht `/usr/local/bin`, auch CONVENTIONS 3.4
  ist hier veraltet), `#![forbid(unsafe_code)]` gilt nicht für alle
  Bibliotheks-Crates (`humanitl-sandbox/src/inherit.rs`), Toolchain 1.95.0.
  Root-`SECURITY.md` mit Meldeweg.
- **docs/DESIGN.md** aus `HTokens` (`app/packages/ui/tool/dump_tokens.dart`
  gibt die Tabellen aus, `flutter test tool/dump_tokens.dart`), alle 19
  Hex-Werte aus BACKLOG.md 5 stehen darin; Abweichungen BACKLOG gegen Code
  stehen im Dokument (Code gewinnt).
- **docs/reference/rules.md** mit Feldern, Wahrheitstabelle, Auswertung,
  Rezepten und den gebündelten Regeln. Test `rules_reference_examples` in
  `daemon/crates/rules/tests/reference_examples.rs`: jeder YAML-Block parst,
  jede Probe `# humanitl rules test …` / `# verdict: …` läuft durch den echten
  Evaluator, und der Block der gebündelten Regeln ist gleich `rules/default.yaml`.
- **docs/reference/cli.md** erzeugt aus `clap` (`bin/humanitl/src/cli_docs.rs`,
  eigener Renderer über `render_long_help`, kein `clap_markdown`). Dabei 27
  deutsche Hilfetexte in `cli.rs` ins Englische gebracht (CONVENTIONS 3.9).
- **`cargo xtask docs`** (`daemon/xtask/src/docs.rs`) erzeugt neben
  `parity.md` die drei Referenzen über ihre Generatoren in den Crates;
  `--check` vergleicht alle. Zweiter Lauf ohne Änderung.
- **docs/guides/**: `install.md`, `first-session.md`, `agent-profiles.md`.
  `docs/INSTALL.md` bleibt als englische Maintainer-Seite (APT-Repository,
  Härtung, Bauen, Testmatrix), weil ausgelieferter Code nach
  `#xdg_runtime_dir-ohne-logind` und `#hardening` verlinkt und `unit.rs` die
  Zeile `Overall exposure level` liest; `docs/profiles.md` ist eine Zeigerseite.
  `docs/reference/limits.md` übersetzt.
- **Prüfungen** in `scripts/ci/lint-docs.sh` (CI-Job `rust-check`): jede Seite
  unter `docs/` mit H1 und `Applies to: Humanitl 0.1` in Zeile 3 (alle Seiten
  einschließlich ADRs nachgezogen), README-Abschnitte, Meldeweg in beiden
  `SECURITY.md`, Links und Anker über `scripts/ci/check-doc-links.py` mit
  Selbsttest. `security_doc_mentions_syscalls` in `humanitl-shim/src/seccomp.rs`.
- **Mutationsbeweise:** `userfaultfd` aus `docs/SECURITY.md` entfernt, Test
  rot; Hilfetext in `cli.rs` geändert, `cargo xtask docs --check` rot; Zeile 3
  einer Seite, ein README-Abschnitt und ein Anker verändert, `lint-docs.sh` rot;
  drei Mutationen an `rules.md` (unbekannter Schlüssel, falsches Verdikt,
  geänderte gebündelte Notiz) je rot.

- **Review (Codex, Opus), übernommen:** Prüfbefehl der zweiten Garantie in
  README, README.de und SECURITY auf die Form von `esc_find_sockets`
  (`-xtype s`, kein `-xdev`; die alte Form fand in der Sandbox nichts), mit
  Lint-Wache in `lint-docs.sh`. Ausnahmen an jeder Halte- und
  Aufzeichnungsaussage: `allow`-Regeln und Durchreiche ohne Halt, Ausfall der
  Aufzeichnung (HUM-250) ohne Aufzeichnung. README nennt alle fünf
  Seitenkanäle, `/work` und Durchreiche als die zwei Datenwege. THREAT-MODEL:
  Integrität der Historie nur gegen einen Angreifer ohne HMAC-Schlüssel,
  K-14 entsprechend eingeschränkt, Ausnahmen beim Verfügbarkeitsziel, neuer
  Kanal K-16 (Ausfall der Aufzeichnung, `declared`) und zwei Restrisiken.
  hudsucker ist keine Abhängigkeit: eigener Accept-Loop auf hyper 1 mit
  tokio-rustls und rcgen. Kleinere: Kernel-Minimum 5.4 wie `doctor`,
  `--cap-drop ALL` und `--disable-userns`, `unsafe` in
  `bin/humanitld/src/systemd.rs`, veraltete Zahl 124/124 gestrichen,
  `/proc/net/unix` aus den Prüfaussagen genommen, direkter Link auf das
  Advisory-Formular. Die zwölf Zeilen der Angriffsflächen-Tabelle bleiben die
  der Review; K-13 bis K-16 stehen in der zweiten Tabelle.

Abweichungen von der Pfadliste:

- `docs/reference/config.md` und `docs/reference/diagnostics.md` gibt es nicht.
  Erzeugt und in CI geprüft werden weiter `docs/CONFIG.md` und
  `docs/DIAGNOSTICS.md` mit ihren Generatoren (`config_docs.rs`,
  `diag_docs.rs`); `cargo xtask docs` ruft sie auf. Grund: Binaries verlinken
  `docs/DIAGNOSTICS.md#<code>` (`render.rs`, `doctor.rs`), ausgelieferte
  Versionen würden sonst ins Leere zeigen. Beide bleiben deutsch, weil Register
  und Schema-Beschreibungen deutsch sind; das gehört zu HUM-086.
- `.github/workflows/docs-check.yml` gibt es nicht. Der Pflicht-Job
  `parity-check` fährt `cargo xtask docs --check` und ist damit der
  `docs-check`; ein neuer Job wäre nicht im Branch-Schutz. Er vergleicht die
  Dateien mit dem Erzeugten statt `git diff --exit-code`, gleiches Ergebnis
  ohne Versionsverwaltung.
- `scripts/ci/check-doc-links.py` ersetzt `lychee` (offline, Dateien und
  Anker), ohne zu pinnendes Fremd-Binary in CI.
- `docs/adr/README.md` hatte den Index schon; ADR-0011 bis 0018 existieren.
- `docs/img/intercept.png` ist eine Kopie des Goldens
  `intercept_three_held_dark.png`: 1280 px statt 1600 px, Text als Blöcke
  (Ahem). Weitere Bilder ebenfalls nur aus Goldens.
- Beispiele in `rules.md` gegen den Evaluator im Test geprüft, nicht gegen
  einen laufenden Daemon.

Offen für den Maintainer:

- Meldeweg: `SECURITY.md` nennt bewusst nur die privaten GitHub Security
  Advisories, keine E-Mail-Adresse und keinen PGP-Fingerprint; das
  Akzeptanzkriterium „mit E-Mail“ ist damit offen und laut Fallstricke ein
  Release-Blocker, bis der Maintainer entscheidet.
- Fresh-VM-Durchlauf (Debian 13, nur `.deb`) von `install.md` und
  `first-session.md` mit Protokoll; Zeile in der Testmatrix von `docs/INSTALL.md`.
- Ein echter Screenshot mit Schrift (1600 px, Demo-Sitzung `demo-project`).
- Spezifikation gegen Code: Debian 13 statt 12 (Build auf Ubuntu 24.04),
  `OPENCODE_MODELS_PATH` statt `OPENCODE_MODELS_URL`, Dienst über
  `humanitl daemon install` bzw. `humanitld.socket` statt
  `systemctl --user enable --now humanitld`.
- Kandidaten für neue Issues: Kommentar zur Rangfolge in
  `profiles/default.toml` veraltet; Gleitweg in `queue_pane.dart`
  (`_LeavingLine`) vermutlich relativ statt 12 px; `R`, `Ctrl+D`, `Ctrl+B` aus
  CONVENTIONS 3.9 nicht gebunden; Übersetzung von Register und
  Schema-Beschreibungen (HUM-086).

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

### Umsetzung (HUM-256)

- **Schema.** `V10__flow_diagnostics.sql` legt die Tabelle `diagnostics` an: eine
  Zeile je Befund, `(flow_id, idx)` als Schlüssel, `code`, `severity`, `title`,
  `why`, `docs` und der Vorschlag in `fix_kind` (`FixAction::as_str`), `fix_key`,
  `fix_value`. Die Migration legt nur an, eine Datenbank von vorher bleibt lesbar,
  ihre Flüsse kommen ohne Befunde. Fremdschlüssel auf `flows(id)` ohne Kaskade,
  wie `messages` und `findings`.
- **Wire-Form der Regel.** Weder Recorder noch Proxy kennen die Proto-Typen
  (`tools/deps-allow.toml`). Eine vorgeschlagene Regel steht deshalb in der Form
  von `rules.yaml` in `fix_value`, geschrieben von `humanitl_rules::serialize_rules`
  und gelesen von `parse_rules_for_session` (mit der Sitzung des Flusses für
  `expires: session`). Das ist die Serialisierung, die schon die Datei des
  Nutzers schreibt, kein eigenes Format. Beim Lesen baut `DiagnosticRecord::fix`
  den `FixAction` zurück, und `recorded_diagnostic_to_proto` schickt ihn durch
  dasselbe `fix_to_proto` wie den Befund aus der Registry. Der Test in
  `humanitl-ipc` vergleicht die Proto-Nachrichten vor und nach dem Neustart als
  Ganzes.
- **Schreiben.** `HoldQueue::publish` reicht jedes `FlowEvent::Diagnostic` mit
  Flusskennung an `Recorder::store_diagnostic` weiter, mit dem Serialisierer als
  Closure. Der Schreib-Thread vergibt `idx` als Zahl der schon stehenden Befunde
  und hält die Grenze in derselben Anweisung ein (`HAVING COUNT(*) < 8`).
  `MAX_FLOW_DIAGNOSTICS` steht jetzt in `humanitl-core` und gilt für Registry
  und Aufzeichnung; `humanitl_proxy::registry::MAX_FLOW_DIAGNOSTICS` ist ein
  Re-Export. Ein Befund zu einem Fluss ohne Zeile fällt still weg, statt als
  Verstoß gegen den Fremdschlüssel eine Störung der Aufzeichnung auszulösen.
- **Masken.** `why` geht vor dem Schreiben durch `Recorder::masked_why`
  (`SecretMasks::apply`, danach `apply_host` für einen Schlüssel als Label in
  Kleinschreibung). Dieselbe Funktion maskiert die Befunde, die `GetFlow` aus der
  Registry nimmt, im Rückfall `registry_detail` wie in `with_live_diagnostics`.
- **Lesen.** `GetFlow` liefert die Befunde aus der Aufzeichnung. Die Registry
  bleibt der Rückfall der laufenden Sitzung: Kennt sie mehr Befunde als die
  Aufzeichnung (Bündelung, gescheiterte Zeile), gelten ihre.
- **Aufbewahrung.** Der Aufräumlauf löscht `diagnostics` ausdrücklich vor dem
  Fluss und zählt sie in `PurgeReport::diagnostics`. Ohne diese Zeile
  verweigerte der Fremdschlüssel das Löschen.
- **Tests.** `a_flow_blocked_at_the_cap_keeps_limit_001_after_a_restart`
  (Proxy, `restart_of`: dieselben Codes in derselben Reihenfolge, derselbe `why`,
  dieselbe Regel nach dem Rundweg durch `rules.yaml`);
  `get_flow_after_a_restart_takes_the_diagnostics_from_the_recording` und
  `the_registry_stays_the_fallback_and_is_masked_like_the_recording` (ipc);
  `tests/flow_diagnostics.rs` im Recorder: Neustart mit Reihenfolge und Fix,
  Grenze, Aufbewahrung ohne Waise, Maskierung bis in die Datei, unbekannter
  Fluss ohne Störung, Migration einer Datenbank auf Stand 9.
- **Mutationsbeweise.** Ohne die Grenze im `INSERT` scheitert der Test zur
  Grenze; ohne das `DELETE` der Aufbewahrung der zur Aufbewahrung; ohne
  `masked_why` oder ohne `apply_host` der zur Maskierung; mit `ORDER BY idx DESC`
  die zu Reihenfolge und Grenze; ohne `EXISTS` der zum unbekannten Fluss; ohne
  den Aufruf in `publish` und mit einer veränderten Regel beim Serialisieren der
  Test über `restart_of`; mit leerer Liste in `recorded_detail_to_proto` und mit
  einer nicht gefundenen Regel der Neustart-Test in ipc; ohne Maske im Rückfall
  und mit der alten Bedingung `is_empty()` der Test zum Rückfall.
- **Review, Runde 1** (Codex 2 Punkte, Opus 2 kleinere, alle übernommen; die
  Ablage der Regel in der Form von `rules.yaml` haben beide angenommen):
  `HoldQueue::publish` hält Registry und Aufzeichnung unter einer Sperre
  (`publish_order`), sonst konnten zwei Threads für denselben Fluss die Befunde
  in beiden in verschiedener Reihenfolge hinterlassen. `GetFlow` entscheidet
  nicht mehr nach der Länge, sondern führt beide Listen nach Gleichheit der
  Befunde zusammen (`merge_diagnostics`, Reihenfolge erhalten, doppelte einmal,
  höchstens `MAX_FLOW_DIAGNOSTICS`). Die Befunde des Scans gehen erst nach dem
  Datensatz der Registry hinaus, vorher fand die Registry keinen Fluss für sie.
  Tests: `concurrent_publishers_leave_the_same_order_in_registry_and_recording`
  (200 Runden, zwei Threads), `equal_length_lists_are_merged_by_identity`,
  `merging_keeps_order_drops_duplicates_and_caps`, Befund am Datensatz in
  `a_partial_scan_is_never_an_all_clear`, je mit Mutationsbeweis (ohne Sperre,
  mit der alten Längenregel, ohne Suche nach dem Kopf, ohne Grenze, mit der alten
  Reihenfolge im Handler). Kommentar der Migration und Verweis im Proxy-Test auf
  den Test der Wire-Form berichtigt.
- **Nachtrag aus der Kurzprüfung:** `GetFlow` legt die Masken von jetzt auch
  über die aufgezeichneten Befunde, bevor es zusammenführt. Ein Schlüssel, der
  erst nach dem Schreiben dazukam, steht so nicht im Klartext da, und derselbe
  Befund aus der Registry erscheint nicht ein zweites Mal
  (`a_mask_added_after_recording_applies_and_keeps_one_copy`, mit
  Mutationsbeweis).
- **Offen.** Der Vorschlag selbst geht nicht durch die Masken: Eine Regel aus
  Host und Pfad der Anfrage trüge einen Schlüssel als Label oder Pfadsegment in
  `fix_value` und im Detail, siehe HUM-271.

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

### Umsetzung (HUM-152)

**Wie getippt wird.** Der TUI-Lauf startet `humanitl -v run` im Hintergrund, wartet auf die
Startzeile der Sitzung und hängt dann `humanitl sandbox attach` an, dessen Standardeingabe eine
Fifo ist, die der Lauf zum Lesen und Schreiben offen hält (`exec 8<>`, Deskriptor 8; ein reines
`8>` hinge für immer, wenn `attach` vor dem Öffnen stirbt). Weil der Lauf damit selbst Leser ist,
endet ein Schreiben ohne `attach` nicht an `SIGPIPE`; vor jedem Schreiben prüft er deshalb mit
`kill -0`, ob `attach` lebt, und nennt sonst dessen Protokoll. An einer Pipe gibt es keinen Rohmodus, und das
Ende der Eingabe hängt das Terminal ab (`cmd/attach.rs`, `spawn_input`); deshalb bleibt die Fifo
offen, bis der Lauf fertig ist. Angehängt wird vor dem ersten Bild, damit das TUI die Größe des
Schreibers (ohne Terminal 80 × 24) schon beim ersten Messen hat. Danach: erstes Bild abwarten
(`Ask anything`, höchstens 30 s), die gemessene Frist, die Frage ohne Eingabetaste tippen, warten,
bis sie vollständig im Eingabefeld steht (höchstens 10 s, sonst ein Fehlschlag, der die Frist
nennt), dann `\r`. Beendet wird über `SIGTERM` an `timeout`, das es an `humanitl run` weiterreicht
(`timeout -k 10`: wer `SIGTERM` überhört, wird zehn Sekunden später hart beendet); die Schranke `M3_OPENCODE_TUI_SECONDS` (jetzt 120 s) ist nur noch die Obergrenze.

**Die Frist: 0 s, gemessen.** Messfassung des Laufs am 2026-10-21 mit OpenCode 1.18.31, in genau
dieser Anordnung: ab dem ersten Bild 52 einzelne Buchstaben, einer alle 0,2 s, daneben alle 20 ms
die Größe des Transkripts. Jeder Buchstabe stand im Eingabefeld, in Reihenfolge, auch der erste
6 ms nach dem ersten Bild; zwischen Schicken und Bild höchstens 31 ms. Ein zweiter Lauf zeigte
dasselbe. Die Beobachtung vom 2026-09-07 („in den ersten Sekunden nichts") gilt für diese Anordnung
nicht; ob sie an einer älteren Fassung oder am Anhängen nach dem ersten Bild lag, ist nicht
gemessen. Zahl und Messung stehen bei `M3_OC_TUI_SETTLE` im Skript.

**Was das TUI vom gescheiterten Aufruf zeigt, und was nicht.** Das TUI malt einen gescheiterten
Werkzeugaufruf als eigene Zeile `% WebFetch <url>` in der Fehlerfarbe des Themas (`#e06c75`);
laufend und erfolgreich steht dieselbe Zeile gedämpft. Den Fehlertext, also das `403`, zeigt es
erst nach einem **Mausklick** auf die Zeile (`errorExpanded`, umgeschaltet nur in `onMouseUp`,
nachgelesen im Binary von 1.18.31). Eine Klickposition kennt der Lauf nicht: Der Daemon filtert
das Transkript mit `ColourOnly`, Cursorfolgen kommen nicht durch. Das einzige `403` im
TUI-Transkript ist darum die Zeile `tool said: …` des Mocks, und die zählt nach der Spezifikation
nicht. Die Aussage des Kriteriums tragen deshalb drei Zusicherungen zusammen: Die Zeile
`WebFetch https://humanitl-probe.example/page` steht in der Fehlerfarbe im TUI-Transkript; der
Zähler der Anfragen mit `last=tool` (`mock-llm-note`) wuchs nach der Eingabetaste, und
`/_debug/tool` trägt das `403`; die Historie hat genau eine Anfrage mehr an diesen Host, und genau
diese neue (erkannt an einer `flow_id`, die vor dem TUI nicht da war) hat eine Regel mit `block`
entschieden. Das `403` selbst steht nicht in der Zeile der Historie: Die Antwort gibt der Proxy,
`status` bleibt `0` (gemessen).

**`/_debug/tool` gehört zur Anfrage des TUI.** Der Zähler allein reicht nicht, weil `/_debug/tool`
seit dem kopflosen Lauf schon ein `403` trägt, und der Mock schreibt die Zeile `mock-llm-note`,
bevor er den Inhalt ablegt (Review von Codex). Der Lauf liest `/_debug/tool` deshalb erst, wenn
es zu jeder Notiz auch die Zugriffszeile `mock-llm: POST /v1/chat/completions` gibt; die schreibt
der Mock erst nach der Antwort, also nach dem Ablegen. Der Mock bleibt unverändert (Nicht-Ziel).

**Warten ohne `| grep -q`.** `m3_wait_screen` holt den Text erst ganz in eine Variable und
vergleicht mit `=~`. Ein `grep -q` am Ende der Pipe ließe `sed` an `SIGPIPE` sterben, und unter
`pipefail` würde ein Treffer in einem langen Transkript als Fehlschlag gezählt (Review von Codex).

**Profil.** Das TUI läuft jetzt unter `llm-only-webfetch` statt `llm-only`: Unter `llm-only`
fragt OpenCode einen Menschen, ob es `webfetch` benutzen darf, und das TUI zeigte einen Dialog
statt des Fehlschlags. Das Profil ist aus `llm-only` abgeleitet und unterscheidet sich nur in
`OPENCODE_PERMISSION`; Prompt und Anbieter (HUM-067) bleiben geprüft, die Profilzusicherung
verlangt jetzt `llm-only-webfetch`, und die Texte der Zusicherungen nennen es so.

**Zahlen.** `M3_OPENCODE_ASSERTIONS` von 22 auf 26: der Werkzeugaufruf aus dem TUI, die
Fehlerzeile im TUI, die eine neue Anfrage in der Historie, die Regel mit `block` an genau dieser. In CI läuft der Zweig weiter nicht
(kein `opencode` im Image, `M3_OPENCODE=auto` meldet ihn übersprungen); lokal grün mit
`M3_OPENCODE=1 M3_OPENCODE_BIN=~/.opencode/bin/opencode`.

**Offen.** Das `403` als Text aus OpenCodes eigener Zeile bräuchte entweder den Klick (also eine
Position, die das gefilterte Transkript nicht hergibt) oder eine Fassung von OpenCode, die Fehler
ausgeklappt zeigt. Kandidat für ein eigenes Issue, falls das Kriterium wörtlich gemeint ist.

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

### Umsetzung (HUM-155)

**Wohin der Schreiber geht.** In eine neue Adapter-Crate `humanitl-audit-log`
(`daemon/crates/audit-log`), nicht nach `humanitl-recorder`: `backlog/CONVENTIONS.md` 3.1
erlaubt der Aufzeichnung nur `core`, der Schreiber braucht aber Record, Kanonisierung und Arten aus
`humanitl-audit`, und in der Aufzeichnung lägen SQLite-`FlowStore` und JSONL-Kette in einer Crate.
Die neue Crate darf `core` und `audit` (Zeile in 3.1, Eintrag in `tools/deps-allow.toml` mit
Begründung, warum es ohne ADR geht: kein Port, kein zweiter Adapter, nur die Datei-Seite, die aus
dem Kern auszog).

**Was im Kern blieb** (`humanitl-audit`, jetzt ohne `tokio`, `rustix`, `libc`, `tempfile`,
`getrandom`, `tracing`): `canonical`, `record` (neu dort `MAX_LINE_BYTES`), `kinds`, `Anchor`,
`AuditKey` ohne Datei (neu `AuditKey::from_file` für den Adapter), die Prüfung über einen Leser
(`verify_reader`, `verify_reader_until`, `VerifyReport::diagnostic`, `set_aside_fix` jetzt
öffentlich), die Seiten über Leser (`query::query_from` mit einem Öffner, der höchstens zweimal
gerufen wird, `query::tail_from`), die Formate des Exports (`ExportFormat`, `CSV_COLUMNS`,
`csv_row`, `csv_field`) und Record und Grenzen der Aufbewahrung (`AuditPruned`, `PruneLimits`,
`PruneOutcome`, `PruneReport`, `MIN_MAX_BYTES`).

**Was auszog** nach `humanitl-audit-log`: `writer.rs` ganz, die Datei-Seite von `key.rs`
(`AuditKeyFile::load_or_create_file`), `AuditVerifier::verify` und `verify_until` über einen Pfad
(`VerifyFile`), `query::query` und `query::tail` über einen Pfad, `export::export` mit Nebendatei,
Sperre und `hard_link`, und Schnitt und Umschreiben der Aufbewahrung (`find_cut`, `find_byte_cut`,
`rewrite`, `sync_dir`). `AuditKeyFile` und `VerifyFile` sind Erweiterungs-Traits mit genau einer
Implementierung, kein Port: Sie halten `AuditKey::load_or_create_file(..)` und
`AuditVerifier::verify(..)` an ihrer Stelle, die Aufrufer in `humanitld`, `humanitl` und `ipc`
ändern nur ihre Importe.

**Tests.** Die Tests aus HUM-050 (`tests/writer.rs`, `tamper.rs`, `retention.rs`, `bench.rs`,
`common/`) zogen mit dem Schreiber nach `daemon/crates/audit-log/tests/`; geändert sind nur die
Importe, keine Zusicherung. Die Unit-Tests der Datei-Seite von `key`, `export` und `retention`
zogen mit ihrem Code. `tests/regressions/` bleibt im Kern, es prüft über einen Leser, und das
Fuzz-Target zeigt weiter darauf. Die Tests von `query/quick.rs` lesen die Kette jetzt aus Bytes
statt aus einer Datei; Fälle und Zusicherungen sind dieselben.

**Die Prüfung.** `tools/deps-allow.toml` hat einen Abschnitt `[core]`: `humanitl-core`,
`humanitl-rules`, `humanitl-findings`, `humanitl-audit`. `tools/check_deps.py` weist eine normale
Abhängigkeit einer dieser Crates auf `tokio`, `rustix`, `libc`, `tempfile`, `nix`, `fs-err`,
`memmap2`, `mio` oder `async-std` ab und jeden Treffer eines Musters aus `[core].source` in ihrem
`src/`, nachdem die Kommentare entfernt sind: `std::fs` auch in gruppierten, umbenannten und
mehrzeiligen Importen, ein nackter Pfad `fs::`, `use std::*`, `use std as`, `File::open`,
`OpenOptions`, `.exists()`/`.metadata()`/`.read_dir()`/`.canonicalize()`, `tokio::`,
`thread::spawn`/`thread::Builder`, `std::process`, Sockets aus `std::net` (Adressen bleiben
erlaubt). Jede Datei wird als Ganzes durchsucht. Absichtlich nicht geprüft: Build-Abhängigkeiten
und Module, die `#[path]` von außerhalb `src/` holt. Neue Fälle in
`tools/tests/check_deps_test.py`, einer je Muster; ohne das Muster für Gruppen scheitern vier
davon. Mutationsbeweis in einer Kopie des Baums: `tokio` zurück in
`daemon/crates/audit/Cargo.toml` lässt `tools/check-deps.sh` mit Exit 1 und „core crate
humanitl-audit depends on tokio" scheitern, `rustix` in `humanitl-findings` ebenso, eine Zeile
`std::fs::metadata` in `audit/src/record.rs` mit „does IO"; zurückgesetzt wieder Exit 0.

`humanitl-config` steht im Bild von `docs/ARCHITECTURE.md` 2 im Kern, liest und schreibt aber
`config.toml` und seine privaten Verzeichnisse mit `rustix`. Es steht deshalb nicht in `[core]`;
das gehört in ein eigenes Issue (Kandidat, siehe unten). Der Satz über die Abweichung von `audit`
in `docs/ARCHITECTURE.md` ist gestrichen, das Adapter-Bild nennt `audit-log/jsonl`.

**Kandidaten für neue Issues.** `humanitl-config` aus dem Kern ziehen oder seine Datei-Seite in
einen Adapter legen, damit auch es in `[core]` stehen kann. Die Quelltextprüfung von `[core]` über
den Syntaxbaum (etwa mit `syn`) statt über Textmuster: Die Muster bleiben umgehbar, wer es darauf
anlegt, und sehen `#[path]`-Module außerhalb von `src/` nicht.

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

### Umsetzung (HUM-250)

Der Weg vom Schreib-Thread in den Ereignisstrom stand schon (HUM-026:
`Recorder::diagnostics`, im Daemon `report_diagnostics`), aber ohne Grenze
und ohne Ende: Jede gescheiterte Zeile war ein Befund, ein toter Thread einer
je Kommando, und nichts sagte, wann es vorbei war. Ein Client, der sich
während der Störung verband, erfuhr gar nichts.

- **Daemon, einmal je Ursache.** `humanitl-recorder` hat einen Melder
  (`src/health.rs`, `Reporter`), über den Schreib-Thread, Handle und jeder
  Mitschnitt melden. Ein Befund mit `RECORDER_001` oder `RECORDER_003` und
  mindestens `Error` beginnt eine Störung und geht je Code einmal hinaus; die
  erste Transaktion danach, in der nichts scheiterte, beendet sie mit dem
  neuen `RECORDER_005` („Aufzeichnung läuft wieder“, `Info`). Scheitert
  schon `BEGIN` oder `COMMIT` an einer gesperrten, nur lesbaren oder fremden
  Datei, heißt der Befund `RECORDER_001` (`Error`, Fix mit dem Pfad), sonst
  `RECORDER_003`. Die Warnung über die Rechte von `-wal` und `-shm` ist jetzt
  `Warning` und beginnt keine Störung.
- **Späte Clients.** `Recorder::outage` nennt die Befunde der laufenden
  Störung; `Subscribe` liefert sie vor allem anderen nach.
- **`GetFlow` über einen noch nicht entschiedenen Flow** nimmt die
  Kopfzeilen aus der Live-Registry des Proxys (`with_live_request`), maskiert
  wie in der Aufzeichnung (`Recorder::masked_headers`, HUM-140); ein
  fehlender Body-Verweis kommt ebenfalls von dort. Eine Anfrage, deren
  Aufzeichnung nur die Zeile trägt, verschweigt damit kein
  `Upgrade: websocket` mehr und ist in der Oberfläche wieder entscheidbar
  (`requestIsReadable` bleibt als Riegel für den Rest).
- **Oberfläche.** `recorderHealthProvider` (`providers/recorder_health.dart`)
  liest Beginn und Ende aus dem Strom und vergisst den Stand beim
  künstlichen `Lagged` jeder neuen Verbindung, nach dem der Daemon eine
  laufende Störung nachliefert. Die Statusleiste zeigt Code und
  „Aufzeichnung gestört“, jede gehaltene und jede seit Beginn angekommene
  Karte „Wird nicht aufgezeichnet“, beides in der Fehlerfarbe und nicht im
  Rot des Blockierens (`docs/UX.md` 3.3). Die erste Freigabe der Sitzung hält
  an `RecorderPause`, an der Stelle der Leiste wie die Pause mit offenen
  Funden; „Unaufgezeichnet senden“ (`S`) nimmt erst nach `HMotion.rearm` an
  und setzt genau die angehaltene Freigabe fort, `Esc` schließt. Gilt für
  Klick, `Enter`, `A`, Gruppe, Auswahl, „Allow all“ und die Benachrichtigung
  (dort kommt das Fenster mit offener Pause nach vorn).
- **Tests.** `recorder_outage_tests` in `humanitld` (Schreiber scheitert
  dreimal, genau ein `RECORDER_003` auf dem Strom, danach genau ein
  `RECORDER_005`), `a_write_failure_reports_once_and_its_end_once` (Trigger
  statt voller Platte), `health::tests`, die Einordnung nach `SQLite`-Code,
  `get_flow_of_a_held_flow_takes_the_request_from_the_registry`,
  `a_late_subscriber_hears_of_a_running_recorder_outage`,
  `status_bar_test: recorder_failure_marks_cards`, Golden
  `states_recorder_failure` in vier Varianten.
- **Mutationsbeweise.** Daemon: ohne die Entstörung in `Reporter::report`
  scheitern `a_write_failure_reports_once_and_its_end_once` und der Test in
  `humanitld`; ohne `recovered()` im Commit beide am fehlenden
  `RECORDER_005`; ohne `with_live_request` und mit ungeleerten Masken
  scheitert der Test zu `GetFlow`, ohne die Nachlieferung der Störung der zu
  späten Abonnenten. Oberfläche: `recorder_failure_marks_cards` scheitert,
  wenn der Beginn der Störung nicht erkannt wird, die Pause nicht aufgeht,
  die Bestätigung nicht für die Sitzung gilt, der Knopf vor `HMotion.rearm`
  annimmt oder der Hinweis auf der Karte fehlt.
- **Offen.** Das Senden aus dem Editor (HUM-047) geht an der Pause vorbei,
  siehe HUM-269. Eine eigene Markierung der Flüsse aus einer Störung:
  HUM-270.
- **Review, Runde 1** (Codex 6 blockierend, Opus 1 major, alle übernommen):
  Der Rückfall von `GetFlow` auf die Registry (`registry_detail`) maskiert
  jetzt Ziel, Pfad, Kopfzeilen und Body-Angaben wie die Aufzeichnung, ebenso
  `content_type` und `content_encoding` in `with_live_request`
  (`masked_request`). Nach einem `Lagged` liefern Daemon-Brücke
  (`report_diagnostics`) und Ereignisstrom (`event_stream`, über `Flatten`)
  die laufende Störung erneut; die Oberfläche löscht ihren Stand nach einem
  echten `Lagged` erst, wenn das nächste Ereignis keine Störung ist.
  `_send` und `_many` prüfen die Pause unmittelbar vor dem Senden, die
  Benachrichtigung nach dem Detail noch einmal. Die Bestätigung gilt erst,
  wenn die fortgesetzte Freigabe hinausging, und nur für die Sitzung des
  Daemons. Kleinere: Ein toter Schreib-Thread ersetzt den früheren Befund
  (Fix „neu starten“ für späte Clients), die Brücke protokolliert auf der
  Stufe der Schwere, Kommentare zur Reihenfolge der Pausen berichtigt.
  Tests: `get_flow_from_the_registry_alone_is_masked_like_the_recording`,
  `a_lag_during_a_recorder_outage_repeats_the_outage`,
  `a_lagging_bridge_repeats_the_running_outage`,
  `a_dead_writer_replaces_the_earlier_write_error`, `recorder_pause_test`
  (Überlauf, Sitzung, abgewiesene Fortsetzung, Modal nach Beginn der
  Störung, Blockieren unberührt), je mit Mutationsbeweis.
- **Review, Runde 2** (Codex, vier Punkte): Der Rückfall maskiert auch
  `summary.apex` und `domain.apex` (Test mit dem Schlüssel als
  registrierbarem Hostnamen). Der Ereignisstrom löst die Listen erst nach dem
  Zusammenführen mit den Regeländerungen auf, sodass die nachgelieferte
  Störung unmittelbar hinter `Lagged` steht; die Oberfläche lässt eine
  Regeländerung nach einem Überlauf ohnehin nicht entscheiden. Die Pause
  steht nur noch an den beiden Stellen, an denen eine Freigabe hinausgeht
  (`_send`, `_many`): Eine Freigabe mit Modal hält einmal, nach dem Modal.
  Nicht übernommen: Die Wiederholung nach `Lagged` kann einen noch
  gepufferten ersten Befund doppelt liefern. Das ist harmlos, weil die
  Oberfläche die Störung als Zustand hält und ein zweites `RECORDER_003` ihn
  nicht ändert (Test in `recorder_pause_test`).
- **Nachtrag aus der Kurzprüfung:** `_many` fragt die Pause vor jedem
  einzelnen `Decide`. Beginnt die Störung mitten im Stapel, ist das schon
  Gesendete verbucht, und die Pause setzt mit genau den übrigen Anfragen
  fort. Die Pause eines Stapels gehört nicht zur Auswahl und bleibt stehen,
  wenn der Cursor nach der ersten Freigabe weiterrückt
  (`RecorderPause.selectionMoved`). Test: Störung nach der ersten von drei
  Freigaben, danach genau die zwei übrigen, mit Mutationsbeweis.

## HUM-251 · Ein unerreichbares Modell färbt die Zeile „LLM“ nicht
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-039, HUM-058 · Blockiert: nichts

### Kontext
HUM-058, Zeile 15: Scheitert die Durchreiche zum Modell an der Verbindung, entsteht ein `Failed`-Flow (CONVENTIONS 4.10), aber kein Befund `LLM_001`. Die Zeile „LLM“ im Isolation-Panel bleibt eine neutrale Ausnahme-Zeile. Sie aus den Flows abzuleiten wäre Fachlogik in der Oberfläche (ADR-018).

### Ziel
Der Daemon meldet ein im Lauf unerreichbares Modell als `LLM_001` mit Fix `ChangeSetting{llm.endpoint}` am Sandbox-Status, und die Zeile „LLM“ wird rot, bis eine Durchreiche wieder gelingt.

### Akzeptanzkriterien
- [x] Daemon-Test: Connect-Fehler der Durchreiche setzt `LLM_001` am Status, ein Erfolg nimmt ihn weg.
- [x] `sandbox_screen_test: llm_unreachable_row` mit Mutationsbeweis.

### Umsetzung (HUM-251)
- **Code und Bedeutung.** `LLM_001` bleibt „LLM-Endpoint nicht erreichbar“; die Durchreiche im Lauf ist derselbe Befund wie die Probe vor dem Start, nur mit anderem Vorschlag. Ein eigener Code hätte zwei Nummern für eine Beobachtung bedeutet. Das Register nennt den Lauf jetzt im Doc-Kommentar und in der Fix-Spalte (`ChangeSetting` auf `llm.endpoint`), `docs/DIAGNOSTICS.md` ist neu erzeugt. Schwere im Lauf `Error`, nicht `Blocking`: Der Lauf ist schon gestartet, und nichts wird damit verhindert.
- **Wo entschieden wird.** `daemon/crates/ipc/src/llm_reach.rs` hört den Ereignisstrom der Warteschlange mit. Gezählt wird nur ein Fluss mit `Decided { source: Passthrough }`. `Failed` mit `Dns`, `Connect`, `Tls` oder `Timeout` setzt den Befund, also genau die Ursachen, die das Register unter `LLM_001` nennt; `PrivateAddress` nicht, das ist `PROXY_008`. Die erste `ResponseHeaders` einer Durchreiche nimmt ihn weg, gleich mit welchem Status: Ein Server, der `500` antwortet, ist erreichbar. Grenze 1024 offene Durchreich-Flüsse; an der Grenze fällt nur der älteste heraus.
- **Lebensdauer des Zuhörers (Review).** `start` meldet ihn **vor** dem Start des Agenten an (`Inner::listen_llm`, Abonnement und leerer Stand); der Empfänger puffert, was ein Agent in den ersten Millisekunden auslöst, während die Garantien geprüft werden, und `attend` lässt ihn danach lesen (`LlmWatch::run`). Am Ende des Agenten bricht `LlmWatch::finish` ab und schickt, wenn dabei ein Befund wegfiel, vor dem Exit-Code eine Momentaufnahme ohne ihn; sonst bliebe die Zeile nach dem Ende rot. Fällt der Zuhörer ohne `finish` weg (früher Ausstieg, gescheiterter Start, abgebrochene Aufgabe), bricht `Drop` die Aufgabe ab und leert den Stand. Ein Abbruch wartet nicht auf eine Aufgabe, die gerade läuft; deshalb schließt `LlmReach::end` den Lauf, `observe` zählt ohne offenen Lauf nichts, und erst `begin` öffnet wieder. Ein gepuffertes `Failed`, das nach dem Ende noch gelesen wird, setzt so kein `LLM_001` mehr. `finish` wartet außerdem auf die abgebrochene Aufgabe, bevor es die letzte Momentaufnahme schickt. Endet ein Agent, bevor der Zuhörer überhaupt gelesen hat, erscheint `LLM_001` im Strom des Starts nie; der Lauf ist dann vorbei, und `end` hätte den Befund ohnehin geleert.
- **Vertrag.** `SandboxEvent.Status.llm` (Feld 15, `Diagnostic`), Minor 22 auf beiden Seiten. Ändert sich der Befund, schickt der Daemon eine neue Momentaufnahme in den Strom des Starts; jeder andere Client liest ihn, wenn er die Momentaufnahme holt (wie `Status.refusals`, HUM-138).
- **Oberfläche.** `SandboxStatus.llmDiagnostic` spiegelt das Feld. Die Ausnahme-Zeile im Isolations-Reiter wird rot (Punkt `state.blocked`, Text `stateText.blocked`), und darunter steht der Befund als `HDiagnosticCard` mit `FixControl`. Die Farbe ist damit nicht der einzige Kanal. Die Oberfläche liest dafür nur das Feld, keinen Fluss.
- **Tests.** `llm_reach::tests` (acht Fälle, darunter: nur Durchreich-Flüsse zählen, eine private Adresse zählt nicht, an der Grenze fällt nur der älteste Fluss heraus, nach dem Ende zählt nichts bis zum nächsten Lauf), in `sandbox::tests` über eine echte `HoldQueue` und denselben Zuhörer, den `start` anmeldet: `a_failed_passthrough_sets_llm_001_on_the_status_and_an_answer_removes_it` (die erste Anfrage scheitert, bevor der Zuhörer liest), `the_end_of_the_agent_sends_a_snapshot_without_llm_001`, `a_dropped_watch_stops_listening_and_clears_the_finding`; dazu `sandbox_screen_test: llm_unreachable_row`. Mutationsbeweise: Antwort nimmt den Befund nicht weg (beide Daemon-Tests rot), Momentaufnahme ohne `llm` (Status-Test rot), Zeile ohne `llmDiagnostic` (Widget-Test rot), Abonnement erst beim Lesen (Status-Test rot), `finish` ohne Momentaufnahme (Ende-Test rot), `Drop` ohne Abbruch (Drop-Test rot), Verdrängung des jüngsten statt des ältesten Flusses (Grenz-Test rot), `observe` ohne Prüfung des offenen Laufs (Ende-Test in `llm_reach` rot).
- **Offen.** Ein Ende-zu-Ende-Test durch den echten Proxy mit einem Ziel, an dem niemand lauscht, fehlt; die Ereignisfolge `Decided(Passthrough)`, `Failed(Connect)` ist aus `FlowHandler::forward` und `record_failure` abgelesen und im Test nachgestellt. Ebenso fehlt ein Test, dass `start` den Zuhörer vor `launch` anmeldet; er bräuchte eine echte Sandbox samt Proxy. Belegt ist nur, dass ein vor dem Lesen angemeldeter Zuhörer die frühen Ereignisse noch sieht.

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

### Umsetzung (HUM-249)

Gewählt ist der erste Weg: Solange das Raster merkt, stehen beide Sätze unter den Controls,
untereinander, der für Senden zuerst, darunter der für Blockieren. Welcher Satz dasteht, hängt nicht
mehr an Zeiger oder Fokus. Der zweite Weg (Kürzel zeigt den Satz, zweiter Druck speichert) bräche den
Rhythmus der Queue und bräuchte einen Zustand, den Zeiger, Fokus und Tasten gemeinsam pflegen; der
dritte (neutraler Satz ohne Aktion) machte die Regel unprüfbar, denn ihre Aktion ist ihre Politik
(`docs/UX.md` 4.6, Tabelle der Verben).

**Was gebaut wurde.**
- `_FirstLine` in `app/lib/features/intercept/widgets/action_bar.dart` zeigt bei `remember.remembers`
  zwei Zeilen mit je `actionBarLineHeight` (Schlüssel `intercept-rule-sentence-allow` und
  `intercept-rule-sentence-block`, die Spalte behält `intercept-rule-sentence`). Der Parameter
  `blockHighlighted` ist weg; `_blockHighlighted` deckt nur noch die Notiz neben „Blockieren" auf.
- Die Sätze entstehen aus denselben Eingaben wie die Regel, die `InterceptDecision` beim Entscheiden
  einmal baut (`ShownRule`, HUM-058): Entwurf, Gültigkeit, `ruleFlowProvider`.
- Die Sätze brechen um, keiner wird mehr mit `maxLines: 1` und Auslassungspunkten gekürzt (Review,
  Codex): bei 480 px und doppelter Schrift stand von einer deutschen Sitzungsregel sonst nur der
  Anfang da. Dasselbe im Modal (`batch_modal.dart`, `intercept-batch-rule`, vorher `maxLines: 2`).
- Das Halten des Ventils bei geschlossenem Raster legt eine Sitzungsregel an (`onAllowRemembered`,
  `_draftDuration`). Ihr Satz steht jetzt vom Druck an, solange der Finger unten ist, also 400 ms vor
  dem Speichern: `HoldToConfirm.onHoldChanged` (neu, optional) meldet Beginn und Ende eines Haltens,
  `ReleaseValve.onHoldChanged` reicht es durch, `_FirstLine.holdRule` zeigt den Allow-Satz mit der
  Dauer Sitzung. Nicht bei offenem Fund, dort ist das Halten nur Senden.
- Das Halten speichert nur die Regel, deren Satz es zeigte (Review Runde 2, Codex): Es schließt im
  Animations-Callback ab, vor dem Build, und `3` im letzten Frame hätte sonst eine Stundenregel unter
  dem Satz für die Sitzung gespeichert. Beim Beginn merkt sich die Leiste die Eingaben der Regel
  (`_HoldInputs`: Entwurf, Quelle aus `ruleFlowProvider`, registrierbare Domain), beim Abschluss liest
  sie sie frisch; weichen sie ab, geht nichts hinaus, und die Leiste sagt warum
  (`RefusalReason.ruleChanged`).
- Die Regelsätze der Leiste nehmen höchstens ein Drittel des Inspektor-Panes (ohne Pane die Hälfte
  des Fensters) und scrollen darüber hinaus (Review Runde 2): Ein Pfad, den der Agent beliebig lang
  machen kann, schob sonst die Entscheidungen aus dem Pane. Eine Entscheidung, die eine Regel direkt
  aus der Leiste speichern würde, fragt `ruleSentenceSeenProvider` (neu); solange die Sätze nicht
  passen und nicht bis zum Ende gescrollt wurden, seit sie sich zuletzt änderten, geht nichts hinaus
  (`RefusalReason.ruleNotInView`). Geprüft in `allow`, `block`, `allowMany`, `blockMany` vor dem
  Senden; der Weg über das Modal bewacht sich selbst. Zum Gesehenen gehört das Layout (Review
  Runde 3): Schrumpft der Pane, nachdem die Sätze ganz zu sehen waren, und ihr Ende rutscht aus dem
  Bild, ist das Tor wieder zu, bis das Ende erneut zu sehen war (Schlüssel aus Eingaben,
  `maxScrollExtent` und Höhe des Ausschnitts; gemessen auch bei `ScrollMetricsNotification`).
- Das Modal (`batch_modal.dart`) ist auf das Fenster begrenzt, sein Rumpf scrollt
  (`HModal.bodyScrollController`, neu und optional; ohne Controller bleibt jedes andere Modal wie es
  war, die Befehlspalette mit eigener Liste eingeschlossen). Bestätigen ist abgeschaltet, bis der
  Rumpf bis zum Ende zu sehen war, und ein Hinweis oben sagt das; solange ist der Rumpf ein
  Fokusstopp, damit Pfeile und Bild-ab ihn ohne Maus scrollen. Gewählt, weil es die einfachste Regel
  ist, die ohne Messung einzelner Zeilen hält: Wer das Ende gesehen hat, hat jeden Teil gesehen.
  Bewusst so (Review Runde 3, verworfen): Auch ein Sprung ans Ende, etwa mit `Ende` oder einem Zug
  am Scrollbalken, zählt als gesehen. Die Regel heißt „bis zum Ende zu sehen", nicht „jede Zeile
  einmal im Bild"; was dazwischen steht, lässt sich zurückscrollen, und das Modal schließt nicht.
  Bestätigen liest die Scrollposition beim Klick und nicht ein Flag, das erst der Frame nach dem
  Öffnen setzt: Ein Rumpf, der passt, gilt sofort als gesehen, und der Knopf ist nur aus, sobald
  feststeht, dass der Rumpf nicht passt. Mit dem Flag verschluckte das Modal einen Klick im ersten
  Frame, und die M2-Demo (`app/integration_test/m2_first_decision_test.dart`) blieb beim Stapel der
  zwölf npm-Anfragen stehen.
- `docs/UX.md` 4.6 nennt den Weg, die verworfenen beiden, das Umbrechen, das Halten, die Grenze der
  Leiste und das Modal.
- Drei ARB-Einträge, am Ende angehängt: `interceptRefusedRuleChanged`,
  `interceptBatchScrollToRead`, `interceptRefusedRuleNotInView`.
- Goldens: `action_bar_remember_open` (vier Varianten) und `action_bar_text_scale_2` neu; der Rahmen
  `withGrid` in `action_bar_golden_test.dart` ist eine Zeile höher (244 statt 220 px), der Rahmen von
  `action_bar_text_scale_2` 400 statt 320 px, weil beide Sätze dort umbrechen.

**Tests.** `app/test/features/intercept/rule_sentence_shortcut_test.dart`. Ein Satz zählt nur als
sichtbar, wenn sein `RenderParagraph` nicht gekürzt ist (`didExceedMaxLines`); gelesen wird jeder
gezeichnete Absatz, auch aus `Text.rich`.
- Raster offen mit Session, Maus über „Blockieren" (Notiz aufgedeckt, Satz „block …" steht), `A`.
  Geprüft wird, dass der Satz der gespeicherten Allow-Regel, vorgelesen vom Generator des
  Regel-Bildschirms, schon vor dem Druck auf dem Schirm stand.
- Raster offen, Fokus auf dem Ventil (geprüft), Satz „allow …" steht, Blockier-Kürzel, gespeichert wird
  eine Block-Regel, deren Satz vorher dastand. Das Kürzel ist `Ctrl+L`: Ein einzelnes `B` gibt ein
  fokussiertes Control an sich selbst weiter (`docs/UX.md` 5.2, `focusedControlHandlesActivate`) und
  entscheidet dann gar nicht.
- Leiste allein bei 480 px, doppelter Schrift, Deutsch, langem Host und Ziel Domain: Beide Sätze
  stehen ganz da, gleich dem Satz der Regel, die gespeichert würde, umgebrochen, nicht gekürzt, ohne
  Überlauf. Am ganzen Bildschirm geht das nicht: Dort laufen bei diesen Maßen schon vorher
  „Bearbeiten + Senden" (`action_bar.dart`, Zeile mit `interceptEditAllow`) und der Kopf der Karte
  (`request_card.dart`) über, beides nicht Teil dieses Issues.
- Raster zu, Finger auf dem Ventil: nach 100 ms steht „allow · ∗ · registry.npmjs.org · this
  session", noch nichts gespeichert; nach dem Halten ist genau die Regel mit diesem Satz gespeichert.
- Raster zu, Finger auf dem Ventil, dann `3` ohne Frame dazwischen: nichts gespeichert, die Leiste
  nennt den Grund.
- Fenster 1400 × 600, URL-Regel „für immer" mit einem Pfad aus 160 Segmenten: Der Satz reicht über
  das Fenster, der Hinweis steht, ein Klick auf Bestätigen tut nichts; nach dem Scrollen bis zum Ende
  speichert er genau die Regel des Satzes.
- Dasselbe Fenster, Sitzung über die URL: `A` geht nicht, solange die Sätze der Leiste nicht bis zum
  Ende gescrollt sind, danach speichert es die Regel des gezeigten Satzes.
- Fenster 900 hoch, Sätze passen; dann 600 hoch, ihr Ende ist außer Sicht: `A` speichert nichts.
- Modal mit passendem Rumpf, Klick auf Bestätigen im ersten Frame: Die Regel wird gespeichert
  (rot mit dem Knopf, der erst nach dem ersten Frame angeht).

**Mutationsbeweise.**
- `action_bar.dart` auf den Stand vor dem Issue (ein Satz nach Hervorhebung): beide Tests rot, der
  erste mit „does not contain 'allow · ∗ · registry.npmjs.org · this session'", der zweite mit
  „does not contain 'block · …'".
- Die zweite Zeile wiederholt `RuleAction.allow` statt `RuleAction.block`: beide Tests rot.
- Die Sätze wieder mit `maxLines: 1` und `TextOverflow.ellipsis`: der Test bei 480 px rot
  (`didExceedMaxLines` true).
- `holdRule` immer falsch: der Halte-Test rot („does not contain 'allow · ∗ · registry.npmjs.org ·
  this session'").
- Vergleich der Eingaben in `_allowHeld` aus: nur der Test mit `3` im letzten Frame rot.
- Bestätigen im Modal immer an: nur der Modal-Test rot.
- `_ruleInView` immer wahr: nur der Test mit der Leiste im kurzen Fenster rot.
- Schlüssel des Gesehenen ohne Layout (nur die Eingaben): der Test mit dem schrumpfenden Fenster rot.

**Nicht übernommen, eigenes Issue.** Zwei Tasten in einem Frame (`3`, dann `A` vor dem Layout)
speichern eine Regel, deren Satz nie gezeichnet wurde. Das Fenster bestand schon vor diesem Issue
und ist kein Szenario seiner Kriterien; HUM-268.

**Offen, als Kandidat für ein eigenes Issue.** Bei 480 px Pane-Breite und doppelter Schrift laufen
auf Deutsch „Bearbeiten + Senden" (92 px nach rechts) und, bei niedrigem Fenster, der Kopf der Karte
über, weil nur ihr Rumpf scrollt. Seit diesem Issue ist die Leiste mit offenem Raster dort höher,
der Kopf der Karte wird also früher eng.

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

### Umsetzung (HUM-266)

- **Zweite Lesart.** `inflate` liest bei `deflate` weiter erst den zlib-Rahmen. Endet
  er vollständig im Budget, vergleicht `raw_reading_differs` den nackten Strom gegen
  dessen Ausgabe. Weicht er ab (anderer Inhalt, kürzer oder länger) und endet er
  sauber, steht in `Inflated::ambiguity` der Wert `Ambiguity::Readings`, und
  `decode_body` meldet `truncated: true` mit `FINDINGS_002` ohne `fix`. Bytes hinter
  dem Ende des nackten Stroms zählen nicht als Bruch, denn Python
  `zlib.decompressobj(-15)` liest den Strom davor trotzdem; genau so endet der
  Polyglott. Dasselbe gilt für einen nackten Strom, der nach der Abweichung mitten
  im Block aufhört (`UnexpectedEof`): Ein Empfänger, der stückweise entpackt, hat
  den abweichenden Teil schon ausgeliefert (Befund aus dem Review). Ist der Inhalt
  des nackten Stroms ungültig, bleibt es bei der zlib-Lesart ohne Befund.
- **Wörterbuch.** Ein gültiger zlib-Kopf mit `FDICT` scheitert am Entpacker dieser
  Crate immer, weil der Scan das Wörterbuch nicht hat; bisher galt dann still der
  nackte Strom. Endet der sauber, meldet der Scan jetzt `Ambiguity::PresetDictionary`
  als `FINDINGS_002` und durchsucht die nackte Lesart (Befund aus dem Review).
- **Was durchsucht wird.** Die zlib-Lesart, nicht beide. `GetBody` liefert über
  `inflate` dieselben Bytes aus, und nur dann zeigen die Fundstellen auf den Wert,
  den die Oberfläche anzeigt (HUM-119). Die nackte Lesart als zweiten Eintrag zu
  durchsuchen hieße, Bereiche zu liefern, die in `GetBody` auf andere Bytes zeigen.
  Die Lücke ist benannt; das verlangt 4.13.
- **Budget.** Je Lesart dieselbe Grenze: Die zweite Lesart bekommt eine Kopie des
  Budgets vor der zlib-Lesart. Ihre Ausgabe wird stückweise verglichen und
  verworfen, es liegen also höchstens 16 KiB zusätzlich im Speicher, und entpackt
  werden höchstens `allowance + 1` Bytes. Läuft sie über das Budget, gilt sie als
  verschieden, weil die zlib-Lesart ganz hineinpasste. Hat schon die zlib-Lesart
  das Budget erreicht, gibt es keine zweite Lesart; der Body ist dann ohnehin als
  gekürzt gemeldet. Der Vorschlag aus der Fix-Idee, beim ersten abweichenden Byte
  abzubrechen, ist nicht übernommen: Erst das Ende zeigt, ob der nackte Strom eine
  gültige Lesart ist, und ein Abbruch dort hätte jeden zufällig durchrutschenden
  Stored-Kopf zum Befund gemacht.
- **Tests.** `the_polyglot_has_two_valid_readings` prüft die Datei selbst,
  `a_deflate_polyglot_is_reported_and_not_called_complete` das Kriterium,
  `ordinary_zlib_and_raw_deflate_have_one_reading` gewöhnliche Bodies aller Stufen
  einschließlich `none`, `the_second_reading_stays_inside_the_same_budget` die
  Grenzfälle der zweiten Lesart,
  `a_naked_reading_that_diverges_and_then_stops_short_is_a_reading` den
  abgeschnittenen nackten Strom, `a_zlib_header_with_a_preset_dictionary_is_a_gap`
  den Kopf mit Wörterbuch. Die drei Bodies liegen als
  `body_decoder-deflate-polyglot.bin`, `body_decoder-deflate-truncated-divergence.bin`
  und `body_decoder-deflate-preset-dictionary.bin` in den Regressionen und unter
  denselben Namen ohne Präfix im Corpus.
- **Kandidat bleibt offen.** Den Anfang eines kaputten Stroms zu durchsuchen gehört
  nicht zu diesem Issue.

---

## HUM-267 · Ein Test scheitert zufällig an „Text file busy"
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-134 · Blockiert: —

### Kontext
Die CI ist auf den Branches von HUM-255 und HUM-247 an einem Test gescheitert, der mit beiden
Issues nichts zu tun hat:
`secret::tests::without_a_keyring_the_key_lands_in_a_0600_file_and_says_so` meldet
„secret-tool-broken could not be started (Text file busy (os error 26))". Die Attrappen von
`secret-tool` in `daemon/crates/config/src/secret.rs` (`fake_tool`, `broken_tool`, das Skript in
`a_tool_that_never_answers_runs_into_the_deadline`) schreiben ihr Skript mit `fs::write` und
starten es gleich danach. Ein anderer Test-Faden desselben Prozesses kann in genau dem Fenster
`fork` aufrufen, in dem das Skript zum Schreiben offen ist. Das Kind erbt den Deskriptor und hält
ihn bis zu seinem eigenen `exec`; das `execve` des Skripts scheitert in dieser Zeit mit
`ETXTBSY`. `O_CLOEXEC` hilft nicht, weil das Fenster zwischen `fork` und `exec` liegt. Dieselbe
Ursache hat HUM-134 in der Sandbox-Crate behoben; `humanitl-config` und `humanitl-ipc` haben
die Lösung nie bekommen. In `humanitl-ipc` haben `fake_tool` in `llm_key.rs` und
`fake_secret_tool` in `sandbox.rs` dasselbe Muster.

### Ziel
Kein Test in `humanitl-config` und `humanitl-ipc` hält beim Start eines selbst geschriebenen
Skripts einen Schreib-Deskriptor darauf in einem Prozess, der parallel `fork` aufruft. Der
Produktionscode bleibt unverändert; ein Wiederholen bei `ETXTBSY` gehört nicht dorthin.

### Akzeptanzkriterien
- [ ] Jede Test-Attrappe in `daemon/crates/config/src/secret.rs`, `daemon/crates/ipc/src/llm_key.rs`
  und `daemon/crates/ipc/src/sandbox.rs`, die ein Skript schreibt und ausführt, legt es über
  `test_support::write_executable` an.
- [ ] `write_executable` öffnet das ausführbare Ziel im Testprozess nie zum Schreiben.
- [ ] Ein Belastungslauf zeigt den Fehler vor dem Fix und nicht danach.

### Umsetzung
> **Umsetzung (HUM-267, 2026-10-21).** Neues Modul `test_support` (nur unter `cfg(test)`) in
> `humanitl-config` und `humanitl-ipc` mit `write_executable(path, body)`, nach dem Vorbild von
> `write_program` in `daemon/crates/sandbox/src/test_support.rs`: Der Inhalt geht in eine
> Quelldatei `<name>.source`, die nie ausgeführt wird; ein vorhandenes Ziel wird entfernt, `cp`
> legt das Skript als neue Inode an, danach `chmod 0755` und die Quelle wird gelöscht. Den einzigen
> Schreib-Deskriptor auf das Skript hat der Prozess `cp`; der ist beendet, bevor ein Test das
> Skript startet. Erbt ein anderer Faden beim `fork` den Deskriptor der Quelldatei, schadet das
> nicht, denn die Quelle wird nie ausgeführt und ist eine andere Inode. Umbenennen oder ein harter
> Link der Quelle hätte nicht geholfen, weil es dieselbe Inode bliebe. Die drei Attrappen in
> `secret.rs`, `fake_tool` in `llm_key.rs` und `fake_secret_tool` in `sandbox.rs` nutzen die
> Funktion. Belastungslauf (`humanitl_config-* secret --test-threads=16`, `taskset -c 0-5`,
> `nice -n 19`, Last um 15): vor dem Fix 1 Fehlschlag mit `ETXTBSY` in 500 Läufen, danach
> keiner in 2000.
>
> Kandidaten mit demselben Muster außerhalb dieses Issues (Skript per `fs::write` oder
> `fs::copy` geschrieben und im selben Testprozess gestartet): `write_script` in
> `daemon/bin/humanitl/tests/daemon_lifecycle.rs` (dort fängt `run` `ETXTBSY` mit Wiederholen
> ab), die Skripte in `daemon/bin/humanitl/tests/cli.rs` (ebenfalls mit Wiederholen über
> `output_when_not_busy`). Nur geschrieben und als ausführbar markiert, nach erstem Blick aber
> nicht gestartet: `daemon/bin/humanitl/src/cmd/unit.rs` um Zeile 1201,
> `program` in `daemon/crates/sandbox/src/agent/mod.rs` um Zeile 1308 und
> `daemon/crates/sandbox/tests/opencode_adapter.rs` um Zeile 366, 441 und 1274. Ob eine davon
> das Skript doch startet, ist je Stelle zu prüfen.

### Referenzen
HUM-134, `daemon/crates/sandbox/src/test_support.rs`, `daemon/crates/sandbox/tests/common/mod.rs`.

---

## HUM-268 · Zwei Tasten in einem Frame speichern eine nicht gezeichnete Regel
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-249 · Blockiert: — · nach 0.1.0

### Kontext
Aus Runde 3 des Reviews von HUM-249 (Codex). Die Leiste zeigt die Regelsätze, und seit HUM-249
geht eine Entscheidung, die eine Regel direkt aus der Leiste speichert, nur hinaus, wenn die Sätze
ganz zu sehen waren (`ruleSentenceSeenProvider`). Gemessen wird nach dem Layout. Kommen zwei Tasten
im selben Frame an, etwa `3` (eine Stunde) und gleich danach `A`, liest die Entscheidung den neuen
Entwurf, während auf dem Schirm noch der Satz des alten steht und das Tor noch vom alten Layout
offen ist. Gespeichert wird dann eine Regel, deren Satz nie gezeichnet wurde. Für das Halten des
Ventils hat HUM-249 dieses Fenster geschlossen (`_HoldInputs`); für Tasten bestand es schon vor
HUM-249.

### Ziel
Keine Taste speichert eine Regel, deren Satz nicht in einem gezeichneten Frame stand, auch wenn
mehrere Tasten zwischen zwei Frames ankommen.

### Fix-Idee
- Das Tor synchron schließen, sobald sich eine Eingabe der Regel ändert (Entwurf, Quelle, Domain),
  nicht erst nach dem nächsten Layout.
- Die zu speichernde Regel mit der zuletzt gezeichneten vergleichen: die Leiste hält nach jedem
  Frame die `ShownRule` fest, deren Satz sie gemalt hat, und die Entscheidung speichert nur genau
  diese, sonst nichts und mit Grund.

### Tests
- Widget-Test: Raster offen mit Sitzung, `3` und `A` ohne Frame dazwischen; gespeichert wird
  nichts oder genau die Regel des gezeichneten Satzes.
- Mutationsbeweis: Vergleich mit der gezeichneten Regel aus, der Test wird rot.

### Akzeptanzkriterien
- [ ] Zwei Tasten in einem Frame speichern keine Regel, deren Satz nicht gezeichnet war.
- [ ] Der Weg über das Modal und das Halten des Ventils bleiben unverändert.

### Referenzen
HUM-249 (Review Runde 3), HUM-058 (`ShownRule`), `docs/UX.md` 4.6.

## HUM-269 · Das Senden aus dem Editor übergeht die Pause einer gestörten Aufzeichnung
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-250, HUM-047 · Blockiert: nichts

### Kontext
HUM-250 hält die erste Freigabe einer Sitzung während einer Störung der Aufzeichnung an einer Pause an: Klick, `Enter`, `A`, Gruppe, Auswahl, „Allow all“ und die Benachrichtigung. Der Editor (`app/lib/features/editor/editor_host.dart`, `_decide`) ruft `Decide` mit `AllowEdited` direkt am Client und geht an `InterceptDecision` vorbei; eine bearbeitete Anfrage geht damit ohne die Pause hinaus. Kandidat aus der Umsetzung von HUM-250.

### Akzeptanzkriterien
- [ ] Während einer Störung hält auch das Senden aus dem Editor einmal je Sitzung an derselben Pause, ohne dass ein Feature das andere importiert (Pause nach `core`, oder der Editor fragt über einen gemeinsamen Provider).
- [ ] Test im Editor mit Mutationsbeweis.

### Referenzen
HUM-250 (Umsetzung), HUM-047, `docs/UX.md` 5.4.

### Umsetzung (HUM-269)

- **Pause nach `core`.** `RecorderHealth`, `RecorderPause` und
  `RecorderOutage` stehen jetzt in `app/lib/core/recorder/recorder_health.dart`,
  die Ansicht `RecorderPauseView` in `app/lib/core/ui/recorder_pause.dart`
  (wie `findings_pause.dart`). `sanitizeDiagnostic` zog mit nach
  `app/lib/core/text/sanitize_diagnostic.dart`, weil der Kern kein Feature
  importieren darf. Warteschlange, Shell (Statusleiste, Benachrichtigung) und
  Editor lesen denselben Provider; damit gilt eine Bestätigung für alle drei,
  einmal je Sitzung des Daemons. Inhaltlich ist an Pause und Störung nichts
  geändert, nur zwei Methoden kamen dazu: `standsFor(resume)` und
  `withdraw(resume)`.
- **Editor.** `EditorHost._decide` fragt `RecorderPause.owed` unmittelbar
  vor dem `Decide`, wie `_send` der Warteschlange; Blockieren hält nie an.
  Die angehaltene Freigabe ist genau dieses `AllowEdited` mit dem
  bearbeiteten Rumpf. Nach einer angenommenen Freigabe ruft der Wirt
  `noteAllowSent`, die Sitzung gilt dann auch in der Warteschlange als
  bestätigt. Der Bildschirm zeigt die Pause an der Stelle der Leiste (vor der
  Pause mit offenen Funden, die beim Aufgehen schon zu ist), ohne die Taste
  `S` (`letterKeys: false`, die Taste schriebe ins Feld); `Esc` schließt sie,
  „Unaufgezeichnet senden“ nimmt erst nach `HMotion.rearm` an. Gezeigt wird
  nur die eigene Pause (`standsFor`): Öffnet die Benachrichtigung ihre Pause,
  während der Editor offen ist, setzte deren Knopf eine andere Freigabe fort.
- **Aufräumen.** Geht der Editor, während seine Pause steht, nimmt er sie
  mit (`withdraw` in einer Mikroaufgabe nach `dispose`); läuft die Frist ab,
  ebenso. Sonst zeigte die Leiste der Warteschlange eine Pause, deren Knopf
  eine Freigabe in einem abgebauten Widget fortsetzte.
- **Tests.** `test/features/editor/editor_recorder_pause_test.dart`: Das
  erste Senden hält an, vor `HMotion.rearm` nimmt der Knopf nicht an, danach
  geht genau das bearbeitete `AllowEdited` hinaus und `owed` ist für die
  Sitzung falsch; eine in der Warteschlange bestätigte Sitzung sendet ohne
  Halt; `Esc` schließt, sendet nichts und bestätigt nichts; ein geschlossener
  Editor nimmt seine Pause mit. Die bestehenden Tests von Warteschlange,
  Shell, Editor und `states_recorder_failure` laufen unverändert, nur mit
  neuen Importpfaden.
- **Mutationsbeweise.** Ohne die Abfrage in `_decide` scheitern der erste,
  dritte und vierte Test (das Senden geht ohne Pause hinaus), ohne `noteAllowSent`
  der erste (die Sitzung bliebe unbestätigt), ohne `withdraw` in `dispose`
  der vierte, und ohne die Anzeige der eigenen Pause im Editor der erste und
  der dritte.
- **Review-Runde 1 (Codex, Sonnet).** Alle Befunde übernommen.
  - Codex (blockierend): `owed` war für jeden Sender falsch, solange eine
    fortgesetzte Freigabe auf die Antwort des Daemons wartete (globales
    `_resuming`). Eine zweite Freigabe aus Benachrichtigung oder
    Warteschlange ging in dieser Zeit ohne Pause hinaus, auch wenn der Daemon
    die erste danach abwies. Jetzt trägt nur die fortgesetzte Freigabe einen
    Freibrief: `confirm` ruft sie in einer eigenen `Zone` mit einem einmaligen
    Objekt auf, `owed` und `noteAllowSent` erkennen sie daran, und nach ihrem
    Ende gilt er nicht mehr. Die Aufrufer müssen dafür kein Pfand durchreichen.
    Test `a second send waits while the resumed one is refused`.
  - Sonnet, statt Antigravity (alle minor): „Unaufgezeichnet senden“ baut
    die Anfrage beim Bestätigen aus dem Entwurf, wie er dann steht
    (`editedRequestOf` in `draft_ops.dart`, auch für den Knopf „Senden“),
    Test `send unrecorded takes the draft as it stands then`. Ist im Entwurf
    hinter der Pause wieder ein Fund offen (etwa ein zurückgetippter Wert),
    geht nichts hinaus: Der Bildschirm schließt die Pause der Aufzeichnung
    unbestätigt und zeigt die Pause mit offenen Funden, kein `noteAllowSent`;
    der Wirt sendet einen solchen Entwurf auch bei einer Fortsetzung nicht
    (`_current`). Test `a finding reopened behind the pause stops the send
    there`. Die Pause des
    Editors öffnet mit `bound: false`, eine weiterziehende Auswahl schließt
    sie nicht mehr still (im selben Test). Nach „Zurück“ in beiden Pausen
    holt der Editor den Fokus zurück. Die Bibliotheksdoku von `editor_host.dart`
    zählt vier Dinge.
  - Mutationsbeweise: Mit dem Freibrief ohne Prüfung der Zone (globaler
    Schalter) scheitert der Test zur abgewiesenen Freigabe; mit der beim
    Aufgehen gebauten Anfrage und mit `bound: true` scheitert der Test zum
    Entwurf. Ohne die Prüfung im Bildschirm scheitert der Test zum wieder
    offenen Fund (keine Pause mit offenen Funden), ohne beide Prüfungen geht
    dort die Anfrage hinaus.
- **Review-Runde 2 (Codex).** Beide Befunde übernommen.
  - Blockierend: Die `Zone` aus Runde 1 erkannte alles, was aus der
    fortgesetzten Freigabe stammt, nicht die Freigabe selbst. Ein Hörer, ein
    Zeitgeber oder eine Mikroaufgabe, die sie plant, erbte den Freibrief und
    ging ohne Pause hinaus, solange ihr `Decide` offen war. Jetzt gibt
    `confirm` der einen fortgesetzten Freigabe einen `RecorderBypass` als
    Argument (`RecorderResume`); sie reicht ihn ausdrücklich bis zum `Decide`
    durch (`_send`, `_many` und `allow` in `decision.dart`, die Benachrichtigung
    in `tray_host.dart`, `_decide` im Editor) und legt ihn bei `owes` und
    `noteAllowSent` vor. Es gibt keinen Zustand drumherum mehr. Eine
    Freigabe, die vor ihrem `Decide` erst das Modal fragt, verliert ihn dort
    und hält nach dem Modal noch einmal an. Test `a release scheduled inside
    the resumed one stops at the pause`.
  - Minor: Überlappende Bestätigungen ersetzten den einen Freibrief. Jetzt
    gilt eine Menge lebender Freibriefe, jeder bis zum Ende seiner eigenen
    Freigabe, und jede angenommene Freigabe mit einem davon bestätigt die
    Sitzung. Dabei fiel auf, dass `noteAllowSent` die Sitzung des Daemons
    nicht nachzog: Kam die Bestätigung vor der ersten Frage nach `owes`,
    löschte diese sie wieder. `noteAllowSent` zieht die Sitzung jetzt zuerst
    nach. Test `overlapping confirmations: one success confirms, a later
    refusal does not undo it` in `recorder_pause_test.dart`.
  - Mutationsbeweise: Mit einer Prüfung, die den Freibrief nicht vergleicht
    (globaler Schalter), scheitern der Test zur abgewiesenen Freigabe und der
    zur geplanten Freigabe; mit nur einem Platz für Freibriefe und ohne das
    Nachziehen der Sitzung in `noteAllowSent` scheitert der Test zu den
    überlappenden Bestätigungen.
- **Kandidat für ein eigenes Issue.** Öffnet die Benachrichtigung ihre Pause,
  während der Editor offen ist, bleibt sie unsichtbar: Die Leiste der
  Warteschlange ist verdeckt, und der Editor zeigt nur die eigene Pause
  (`standsFor`). So war es schon vor HUM-269.

## HUM-270 · Flüsse aus einer Störung tragen keine eigene Markierung
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-250 · Blockiert: nichts · nach 0.1.0

### Kontext
HUM-250 zeigt „Wird nicht aufgezeichnet“ auf jeder Karte, solange die Aufzeichnung gestört ist, und nimmt den Hinweis mit `RECORDER_005` überall zurück. Danach sieht ein Fluss, der während der Störung ankam und dessen Entscheidung in der History fehlt, aus wie jeder andere; die Oberfläche merkt sich nicht, welche Flüsse betroffen waren. Kandidat aus dem Review von HUM-250.

### Ziel
Ein Fluss, der während einer Störung ankam, trägt eine eigene Markierung „nicht aufgezeichnet“, die die Störung überdauert: in der Zeile, auf der Karte und, soweit der Daemon ihn noch kennt, im Detail. Die Zuordnung kommt vom Daemon (ADR-018), nicht aus einem Zeitvergleich in der Oberfläche.

### Akzeptanzkriterien
- [ ] Daemon: Flüsse aus einer Störung sind als solche erkennbar (Feld oder Befund am Fluss), Test.
- [ ] Oberfläche: Markierung bleibt nach `RECORDER_005` stehen, Test mit Mutationsbeweis.

### Referenzen
HUM-250 (Umsetzung, Review), HUM-026.

## HUM-271 · Ein Regelvorschlag trägt einen Schlüssel aus Host oder Pfad unmaskiert
Sprint: 5 · Größe: S · Abhängigkeiten: HUM-140, HUM-256 · Blockiert: nichts

### Kontext
Seit HUM-256 geht das `why` eines Befunds durch die Masken der Aufzeichnung (`Recorder::masked_why`), bevor es in `diagnostics` steht oder `GetFlow` es aus der Registry liefert. Der Vorschlag daneben nicht: `LIMIT_001` und der Vorschlag zu einer privaten Zieladresse bauen eine Regel aus Host, Port und Pfadpräfix der Anfrage (`stream_rule`, `private_address_rule`). Schreibt der Agent den Zugangsschlüssel des Modells als Label eines Hostnamens oder als Pfadsegment in die Anfrage, steht er im Klartext in `diagnostics.fix_value` und im `fix` des Details, während Zeile, Pfad und Kopfzeilen derselben Anfrage maskiert sind. Ein `CopyCommand` mit einem Pfad hat dieselbe Lücke. Kandidat aus der Umsetzung von HUM-256.

### Ziel
Kein Geheimnis der Masken verlässt den Daemon oder landet in der Datei über einen Vorschlag. Eine maskierte Regel ist keine gültige Regel mehr; der Vorschlag entfällt dann mit einem Satz im `why`, statt einen Knopf anzubieten, der etwas anderes tut, als er zeigt.

### Akzeptanzkriterien
- [ ] Ein `LIMIT_001` an einer Anfrage mit dem Schlüssel als Label des Hosts trägt keinen Regelvorschlag mit dem Schlüssel, weder im Strom noch in der Aufzeichnung noch im Detail; Test mit Mutationsbeweis.
- [ ] Ein Vorschlag ohne Geheimnis bleibt unverändert.

### Referenzen
HUM-140, HUM-256 (Umsetzung, Offen), `daemon/crates/proxy/src/limits.rs`.

### Umsetzung (HUM-271)

- **Ort.** Nicht in den Bauern der Befunde (`body_over_cap`,
  `private_address_refused`, `repeated_drops`, die Proben), sondern im
  Trichter des Ereignisstroms des Proxys: `HoldQueue::publish` ruft vor
  Registry, Aufzeichnung und Strom `HoldQueue::withhold_secret_fix`, das
  `suggestion::withhold_secret_fix` mit den Masken der Aufzeichnung
  (`Recorder::masks`) anwendet. So gilt die Prüfung für jeden Befund im
  Ereignisstrom, gleich welcher Code und welche Art von Vorschlag, und keiner
  der drei Wege kann sie verpassen. Die Sperre `Flow::send_refusal`
  (`HOLD_004`) steht schon vor `publish` am Flow und von dort am Datensatz, in
  `ListFlows` und in der Zurückweisung eines `Allow`; der Handler legt sie
  deshalb vorher selbst durch `HoldQueue::withhold_secret_fix` (Review,
  Codex). Ein zweiter Aufruf in `publish` ändert nichts mehr. Befunde
  außerhalb des Ereignisstroms (`SANDBOX_022`, die Zusammenfassung einer
  Sitzung) gehen nicht durch `publish` und sind nicht abgedeckt; das wird ein
  eigenes Issue. `GetFlow` liest den Befund der
  laufenden Sitzung aus der Registry und den nach einem Neustart aus der
  Aufzeichnung; beide haben ihn schon ohne Vorschlag.
- **Prüfung.** `SecretMasks::appears_in` sagt, ob `apply` oder `apply_host`
  etwas ersetzen würde, also roh, in einer URL kodiert und als Label ohne
  Rücksicht auf die Schreibung. Geprüft wird jeder Text, den ein Vorschlag
  hinausträgt: bei einer Regel Host, Pfadmuster, Pfadpräfixe, Notiz und dazu
  die Form von `rules.yaml`, in der sie in `fix_value` und in die Datei des
  Nutzers ginge; bei `CopyCommand` und `OpenUrl` der Text, bei `SetEnv` und
  `ChangeSetting` Schlüssel und Wert, bei `RemountReadOnly` der Pfad.
- **Folge.** Trägt der Vorschlag ein Geheimnis, fällt er weg, und das `why`
  bekommt den Satz `suggestion::WITHHELD_SENTENCE` angehängt. Ein Vorschlag
  ohne Geheimnis und ein Befund ohne Masken (keine Aufzeichnung) bleiben Byte
  für Byte, wie sie waren.
- **Tests.** In `tests/limits.rs` des Proxys
  `a_key_as_a_host_label_never_rides_on_the_limit_001_suggestion` (Schlüssel
  mit Großbuchstaben in den Masken, klein als Label des Hosts),
  `a_key_as_a_path_segment_never_rides_on_the_limit_001_suggestion` und
  `a_suggestion_without_a_secret_stays_as_it_was` (dieselbe Regel in Strom,
  Registry und `fix_value`, `why` ohne den Satz), je über Strom, Registry und
  Aufzeichnung; in `hold.rs`
  `a_copy_command_with_a_masked_key_is_withheld_on_every_way_out` (Befehl mit
  dem Schlüssel im Pfad neben einem ohne, über alle drei Wege), in
  `tests/hard_block.rs`
  `the_refusal_on_the_flow_carries_no_masked_secret_in_its_fix` (Sperre am
  Datensatz und in der Zurückweisung eines `Allow` gleich der im Strom, ohne
  Vorschlag; ein Teil des Einstellungsnamens steht dafür als Geheimnis in den
  Masken, weil der Vorschlag von `HOLD_004` heute nur ihn nennt) und in
  `suggestion.rs` sowie `secret_mask.rs` je ein Einheitstest.
- **Mutationsbeweise.** Ohne den Aufruf in `publish` scheitern die drei Tests
  mit Schlüssel; ohne `apply_host` in `appears_in` der Test zum Label des
  Hosts; ohne die Pfadpräfixe und die Form von `rules.yaml` in `fix_texts` der
  Test zum Pfadsegment; ohne den Aufruf im Handler der Test zur Sperre am
  Flow; mit einem Vorschlag, der immer wegfällt, der Test ohne
  Geheimnis und der zum Befehl (dessen Befehl ohne Schlüssel bleiben muss).
  Jede Datei danach mit `cmp` gegen ihre Sicherung geprüft.
- **Offen.** Das `why` selbst geht im Strom weiterhin unmaskiert hinaus, wie
  schon die Zeile und der Pfad im `Received` des Stroms; maskiert wird es erst
  in Aufzeichnung und Detail (HUM-256). Ein Vorschlag, der vor dem Eintrag
  eines Schlüssels aufgezeichnet wurde, wird von `GetFlow` nicht nachgeprüft;
  der Start einer Sitzung trägt den Schlüssel vor dem ersten Verkehr ein.

---

## HUM-273 · Releases 0.0.x appear as regular releases, not as pre-releases
Sprint: 5 · Size: S · Dependencies: HUM-053, HUM-225 · Blocks: none

### Context
On 2026-10-22 the project owner asked for 0.0.6 as a proper release, not a pre-release ("mach ein gescheiten 0.0.6 nicht pre release"). Until now `.github/workflows/release.yml` created every `v0.0.N` with `--prerelease` and the title "Humanitl 0.0.N (pre-release)", so GitHub never marked any of them "Latest", and the release notes, the Debian changelog entry and the package description called each one a pre-release. In the same conversation the owner decided that releases stay unsigned until 1.0.0; the notes promised signed packages with 0.1.0.

### Goal
A `v0.0.N` tag publishes a regular GitHub release marked latest, titled "Humanitl 0.0.N". The notes, the package description and the Debian changelog speak of an early release on the way to 0.1.0 and say that signatures come with 1.0.0.

### Non-goal
No change to guard, build, check-deb or the draft-then-publish window; no change to the 0.0.N channel (0.1.0 is still HUM-060). No signing.

### Affected paths
`.github/workflows/release.yml`, `packaging/release/release-notes.sh`, `packaging/release/release-notes-test.sh`, `packaging/deb/control.in`, `packaging/deb/build-deb.sh`, `packaging/release/INSTALL.txt` (shipped in the archive; translated to English along the way, like the notes since HUM-225), `README.md`, `CONTRIBUTING.md`.

### Acceptance criteria
- [x] `gh release create` runs without `--prerelease` and with the title `Humanitl $VERSION`; the publish step runs `--draft=false --prerelease=false --latest`.
- [x] `release-notes-test.sh` fails when either `--prerelease` comes back or `--latest` is missing (mutation: restoring the old publish line gives two failures).
- [x] Notes banner, package description and changelog entry no longer say pre-release; the notes say signatures come with 1.0.0.

---

## HUM-272 · Accent-filled controls react too weakly to hover and press in the light theme
Sprint: 5 · Size: S · Depends on: HUM-260 · Blocks: nothing

### Context
Owner decision 2026-10-22: in the light theme the primary button barely changes on hover and press. Since HUM-260 the accent is the brand yellow with a dark word (`HColors.onAccent`). The light ocher `lAccent` sits in a narrow window: at least 4.5:1 for the word and at least 3:1 as an area on `lBg3`, roughly relative luminance 0.210 to 0.245, about 0.03 of OKLab lightness. The derivation of HUM-260 only moved the HSL lightness. Measured on `main`: light rest `#A57A1C`, hover `#A97D1D`, pressed `#AE801D`, OKLab distance 0.011 rest to hover and 0.012 hover to pressed; dark hover `#EABF5D` 0.020 from rest. About 0.02 is the smallest difference visible side by side. The release valve has the same problem in the light theme (hover and press alpha 0.06/0.12 over its tint: 0.022 per step on `bg1`; dark 0.038), and during a pointer hold it stayed at the hover alpha, so its press step was zero.

### Decision
The owner reviewed three variants on 2026-10-22: a pale hover (rejected as khaki), a saturated colour-only step that turns the light press orange (V1), and a saturated hover with a press close to the ocher plus an inset ring (V2). The owner chose V2.

### Goal
Every accent-filled control steps visibly from rest to hover and from hover to pressed, in both themes, without breaking a contrast floor and without moving anything.

### Non-goal
No change of `lAccent` or `accent`. No size change, no shadow. Secondary, ghost and danger buttons stay as they are. The release valve gets no ring: it is a tint pill whose colour step already reaches 0.04.

### Affected paths
`app/packages/ui/lib/src/tokens/colors.dart`, `app/packages/ui/lib/src/tokens/tokens.dart`, `app/packages/ui/lib/src/theme/shadcn_theme.dart`, `app/lib/features/intercept/widgets/release_valve.dart`, their tests, `docs/UX.md` 2.9 and 3.3.

### Specification
- Thresholds (OKLab distance): rest to hover at least `HColorDerivation.interactionStepMin = 0.04`; hover to pressed and rest to pressed at least `interactionJumpMin = 0.025` as colour, because the ring carries the rest of the press step. The derivation keeps a margin of 0.002.
- Hover (`accentFillHover`, `HColorDerivation.fillHover`): OKLCh, lightness wish +0.04 away from the word, chroma times 1.15, hue turned warmer one degree at a time (at most 40) until rest to hover reaches the threshold. Per hue, the lightness nearest the wish that keeps the word at 4.5:1 and the fill at 3:1 on all four surfaces; lightness outside 0..1 is skipped, chroma is clipped to sRGB. Light `#BE7812`, dark `#FBC33B`.
- Pressed (`accentFillPressed`, `fillPressed`): lightness wish 0.04 towards the word, chroma times 1.3, hue 4 degrees past the hover's turn, further only if the colour floor needs it. Light `#B96D00` (0.029 from hover), dark `#E6A600`.
- Ring (`HSurfaceColors.accentPressedRing`): `onAccent` at `HColors.pressedRingAlpha = 0.75` over the pressed fill, painted in the button's 1 px border, which exists in every state and only changes colour (rest and hover: the fill itself). The owner's sketch said 50 percent; that measures 2.23:1 light and 2.82:1 dark, below the 3:1 a state-carrying UI component needs (WCAG 1.4.11). At 75 percent: 3.35:1 light, 5.18:1 dark.
- No fitting step returns the resting fill; a test fails loudly on that.
- Release valve: hover and press alpha per theme, light 0.12/0.24, dark 0.07/0.14; a pointer down on the left half paints the press alpha while the hold runs.

### Tests
- `tokens_test.dart`: "the primary button steps visibly on hover and on press" (OKLab of the test file, saturation order, word and area floors); "only the pressed primary button carries the ring" (border colour per state, width 1 in every state, ring at least 3:1 on the pressed fill); "the OKLab of the derivation matches the reference"; "a closed window never passes silently as an unchanged fill" (including black/white and white/black fills).
- `widgets_test.dart`: "HButton primary press paints the ring without moving": previews and a real pointer press, painted fill and ring, identical size and word position in rest, hover and pressed.
- `release_valve_test.dart`: "a finger on the valve paints the press, not the hover".
- `action_bar_contrast_test.dart`: "the valve steps visibly on hover and on press", own OKLab.
- Mutation proofs: HUM-260 derivation restored, red (dark rest to hover 0.020); ring colour replaced by the fill, red (ring test, both widget tests); pointer-down term removed from the valve, red (press alpha 0.07 instead of 0.14); lightness guard and null handling reverted, red (null check thrown for black/white); valve alphas back to 0.06/0.12, red.

### Acceptance criteria
- [ ] Rest to hover at least 0.04 OKLab, hover to pressed and rest to pressed at least 0.025 as colour, both themes; test with mutation proof.
- [ ] Pressed carries the ring, rest and hover do not; ring at least 3:1 on the pressed fill; test with mutation proof.
- [ ] Size and word position identical in rest, hover and pressed, also under a real press.
- [ ] `onAccent` at least 4.5:1 on every state, fill at least 3:1 as area on `bg0` to `bg3` of its theme in every state.
- [ ] A closed window fails a test; extreme fills do not throw.
- [ ] Release valve: at least 0.04 per step in both themes, and a held finger paints the press; tests with mutation proof.
- [ ] `docs/UX.md` names the thresholds and the ring.

### Pitfalls
- Darkening on hover breaks the dark word in the light theme (HUM-260 measured 3.84:1). Lightness alone cannot reach the threshold in the light window; the step comes from chroma and hue.
- At the same OKLab lightness a warmer hue has a different luminance, so the lightness search goes to both sides of the wish.
- The OKLab-to-sRGB matrix has to be the exact inverse; a wrong blue row shifted every derived hue in the first draft while all contrast tests stayed green.
- A ring that only appears on press must not add width; the border is always there.
- Measure with an OKLab that is independent of the derivation.

### References
HUM-260 (accent, pitfalls), `docs/UX.md` 2.9, 3.3 and 6, WCAG 2.1 SC 1.4.11.

## HUM-277 · Saved settings take effect without restarting the daemon (before HUM-060)

### Context
Owner report from 2026-10-22, real machine, package v0.0.6: endpoint, model and
access key were saved in the settings sheet and land correctly in `config.toml`
(`humanitl config get llm`) and in the keyring (`humanitl llm key status`). The
agent still could not use them, also after restarting the app. `humanitl doctor`
showed two things:

- `[ok] daemon humanitld 0.0.5` while the CLI is 0.0.6: `apt install` of the new
  package left the old, socket-activated user daemon running, and restarting the
  app does not restart it.
- `[warn] llm llm.endpoint is not set`: the running daemon answered from the
  configuration it read at its own start.

Where the start snapshot is cached: `IpcServer::new` copies `llm.endpoint`,
`agent.adapter` and `agent.command` into `DoctorSetup` (the `llm` and `agent`
lines of `Doctor`), and `SandboxService::new` copies the start configuration
into the slot `Inner::config` that `Plan`/`Argv` snapshots (`llm_endpoint` in
`Sandbox(Status)`, which feeds the setup screen) and the `sandbox.work_dir`
check of `remember` read until the first session start replaces it. The
fallback of `SessionResolver::resolve` for the project directory also came from
the start snapshot. A session start itself already resolved afresh (HUM-067):
the LLM passthrough rule (`llm_passthrough_rule`), the key mask
(`Recorder::mask_secret`), hold mode and deadline. The header of
`config_rpc.rs` and the note of the settings form claimed a restart for all of
them.

### Goal
A value saved through `SetConfig` (the form keys) or `LlmKey` applies without a
manual daemon restart: at the latest at the next sandbox start, at the next
`Doctor` and at the next `Sandbox(Plan)`. A daemon whose version differs from
the client's is visible in `humanitl doctor` and in the app.

### Non-goal
- General live reload with a file watcher (BACKLOG.md section 9, item 17).
- Swapping the proxy's upstream TLS stack at runtime: `tls.trust_system_store`
  keeps applying at the next daemon start, for proxy and probe alike, and the
  form says so. The same holds for keys outside the form (limits, recorder,
  resolver, audit), which the form does not write.
- Restarting user daemons from the package's maintainer scripts. A system
  package does not reach into the running user managers of logged-in users
  (`deb-systemd-invoke --user` enables units, it does not restart them per
  user). The clients say it instead.

### Affected paths
`daemon/crates/ipc/src/{session.rs,sandbox.rs,server.rs,config_rpc.rs}`,
`daemon/crates/sandbox/src/doctor.rs`, `daemon/crates/sandbox/src/doctor/checks.rs`,
`daemon/crates/sandbox/tests/doctor.rs`, `daemon/bin/humanitl/src/cmd/doctor.rs`,
`daemon/crates/core-types/src/diagnostics/codes.rs` (comment of `DOCTOR_006`
only), `app/lib/features/shell/widgets/status_bar.dart`,
`app/l10n/app_{en,de}.arb`.

### Specification
1. `SessionResolver::current()` resolves the configuration without a wish
   (default profile, no project directory) afresh from disk. A resolver built
   with `for_config` (fake mode, tests) keeps returning its fixed base. The
   fallback for the project directory in `resolve` comes from `current()`.
2. `Plan` and `Argv` take their configuration once (`Inner::plan_config`) and
   build the whole snapshot from that one value: the slot while a session
   holds claim or handle and has published its configuration
   (`apply_session` sets the mark, the next claim clears it), otherwise
   `current()`. They never write the slot. A start refreshes the slot from
   `current()` right after taking its claim (`refresh_for_start`), before the
   project directory is checked, also when the handle of an agent that
   exited is still held (that session is being replaced). A session
   generation counts claim, release, publication and handle changes; a `Plan`
   built across a change is built again, at most three times; no build writes
   the pending choice: each stages profile, directory and mode and builds its
   snapshot from them, only the answered build commits them, and a
   start checks an inherited project directory again against its refreshed
   configuration. A running
   session keeps the configuration it started with; a failed resolution
   falls back to the slot.
3. `Doctor` resolves `llm.endpoint`, `agent.adapter` and `agent.command` afresh
   from its paths; only when that fails it falls back to the start values.
4. Passthrough rule, key mask, hold mode and deadline stay as they are: they
   come from the resolution of each start (HUM-039, HUM-067, HUM-140).
5. `DaemonFacts::Reachable` carries `expected_version`. Same major, different
   version: the `daemon` line is a warning with `DOCTOR_006` (one code per
   line). An empty version (the CLI against a daemon announcing `fake`) or
   `0.0.0` (unstamped development build) on either side is not compared; `why` naming both versions and the cause (an upgrade leaves the old
   daemon running), `fix` `CopyCommand("systemctl --user restart humanitld")`.
6. The status bar of the app shows a warning next to the daemon version when
   the build version of the app is known and differs from
   `GetInfo.daemon_version`, with the restart command. Not for the fake daemon
   and not when either side is `0.0.0`.
7. The settings note and the help of `tls.trust_system_store` say what applies
   when; the header of `config_rpc.rs` is corrected.

### Tests
- `ipc` unit: after `config_rpc::set` of `llm.endpoint` and `LlmKey` set, the
  next `Plan` snapshot names the new endpoint without a restart, and the next
  session start installs the passthrough rule for the new host, hands the new
  key to the sandbox file and expects its mask at the new authority.
- `ipc` unit: `Doctor` after `SetConfig llm.endpoint` names the new endpoint.
- `sandbox` unit: a daemon of the same major and another version is a warning
  with `DOCTOR_006` and the restart fix; the same version stays green.
- App widget test: the status bar shows the mismatch notice and hides it for an
  equal version, an unknown app version and the fake daemon.
- One mutation proof per test.

### Acceptance criteria
1. `SetConfig llm.endpoint` followed by `Sandbox(Plan)`, `Doctor` and a session
   start uses the new value without restarting the daemon.
2. `LlmKey` set followed by a session start hands the new key to the agent and
   masks it at the new endpoint's authority; the passthrough rule follows the
   new endpoint.
3. A running session keeps its configuration, and the UI text says so.
4. `humanitl doctor` warns about a daemon whose version differs from the CLI's,
   with why and the restart command; the app shows the same.
5. The adapter coupling counts do not grow.

### Pitfalls
- A `Plan` beside a start that holds its claim but has not published must not
  answer from the stale slot, and one snapshot must not mix two reads of the
  slot (endpoint from one configuration, preview from another; review of
  HUM-277). Test the interleaving deterministically.
- An agent that exited still holds the session (state `running`); it keeps its
  configuration until it is stopped.
- Test resolvers built with `for_config` carry hand-made configurations; a
  fresh resolution from their empty XDG directories would silently replace
  them.
- Versions are compared as strings: CLI, daemon and app are stamped from the
  same tag (`packaging/release/stamp-version.sh`).

### References
HUM-039, HUM-067, HUM-069, HUM-075, HUM-140, HUM-151, HUM-257,
BACKLOG.md section 9 item 17. `docs/SECURITY.md` and `docs/THREAT-MODEL.md`
stay unchanged: no guarantee moves, the passthrough rule and the key mask
followed every start already.

## HUM-278 · The daemon finds agents installed in ~/.local/bin (before HUM-060)

### Context
On the owner's machine (v0.0.6 `.deb`, socket-activated user daemon) `humanitl
doctor` reported `no opencode in PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin`
with DOCTOR_007 and the fix `curl -fsSL https://opencode.ai/install | bash`.
OpenCode was installed: its installer puts one native binary into
`~/.opencode/bin`, and `~/.local/bin/opencode` held a wrapper script. The
systemd user manager's `PATH` names neither directory, and the sandbox binds
`/usr` read-only and nothing under `/home`, so even a found binary would not
start inside. Reinstalling does not help.

### Goal
The daemon finds an agent in the usual per-user directories, makes that one
file available in the sandbox read-only, and points the command at it. When
it finds nothing usable, DOCTOR_007 and AGENT_001 name the directories
searched, the candidates passed over, and a fix that helps everywhere:
`humanitl config set agent.command '["/absolute/path/to/opencode"]'`.

### Non-goal
- Scripts and npm entry points (`#!/usr/bin/env node`): they need files
  beside them; they are named, not bound.
- Detecting the npm user prefix (`npm config get prefix`); a prefix of
  `~/.local` is covered by `~/.local/bin`.
- Commands the caller names after `--` that are not the agent.

### Affected paths
`daemon/crates/sandbox/src/agent/locate.rs` (new), `agent/mod.rs`,
`agent/opencode.rs`, `profile.rs` (`SessionContext::agent_program`),
`bwrap_args.rs`, `doctor.rs`, `doctor/probe.rs`, `doctor/checks.rs`,
`daemon/crates/ipc/src/sandbox.rs`, `daemon/bin/humanitl/src/cmd/sandbox.rs`,
`daemon/crates/core-types/src/diagnostics/codes.rs` (texts of DOCTOR_007 and
AGENT_001), `docs/DIAGNOSTICS.md`, `docs/SECURITY.md`, `docs/THREAT-MODEL.md`
(K-17), `profiles/sandbox/default.toml` (comment), the `SessionContext`
literals in tests.

### Specification
1. Search order for a bare command not startable inside the sandbox: the
   daemon's `PATH`, then `$HOME/.opencode/bin`, `$HOME/.local/bin`,
   `$HOME/bin`. An absolute `agent.command` outside the binds is checked the
   same way. A path relative to the project stays as it is.
2. A candidate is accepted only when its canonical file is executable for
   this user, a native ELF executable, owned by the user or root, and neither
   file nor directory is world-writable; group-writable only when the group
   is the user's private group (primary group, named like the user in
   `/etc/group`, no listed members, no other account in `/etc/passwd` with it
   as primary group; unknown to the files means refused). Debian's umask 002
   leaves `~/.local/bin` at `drwxrwxr-x`.
3. The accepted file is opened once (`O_PATH | O_NOFOLLOW`, `PinnedSource`
   as in HUM-263), checked through that descriptor (owner, mode, ELF header:
   class, byte order, length, type `EXEC` or `DYN`) and bound by descriptor
   (`--ro-bind-fd`) at `/run/humanitl/agent/<name>` after the profile's mounts
   (`SessionContext::agent_program`, `LaunchInputs::agent_program`). The name
   is the one asked for, the host path the canonical one. The first word of
   the command becomes that path. One file, never the directory. Only the
   configured agent is bound, never a command after `--`. The bind is checked
   by these rules, not by the SANDBOX_006 denylist.
4. The adapter preflight stays quiet when the bind will happen. Preflight,
   plan and doctor share one search (`locate::search` over `search_dirs`).
   AGENT_001 names the searched directories, the skipped candidates with their
   reason and the install command; its fix is the `config set` line shared
   with DOCTOR_007. AGENT_002 and AGENT_004 name a passed-over candidate with
   its reason.
5. The doctor searches the same directories under `PATH` and `HOME`. With a
   sandbox view, a host candidate counts only when the start would bind it;
   without one (the daemon's own `Doctor` RPC) a host PATH hit stays a
   statement about the host. Found: the agent line is green with the
   canonical path. Missing: evidence `no
   opencode in PATH=… or <dirs>`, `why` with the passed-over candidates and
   the install command, fix `CopyCommand` of the `config set` line above.

### Tests
- `locate::tests::an_agent_in_local_bin_is_found_while_path_lacks_it`
- `locate::tests::a_group_writable_dir_is_skipped_unless_the_group_is_private`
- `locate::tests::a_script_is_passed_over_for_a_native_binary_later_in_the_order`
- `locate::tests::the_private_group_needs_the_users_name_and_no_members`
- `opencode_adapter::an_agent_in_local_bin_is_bound_and_the_preflight_stays_quiet`
- `opencode_adapter::agent_001_names_the_per_user_dirs_it_searched`
- `bwrap_args_snapshot::the_agent_program_is_bound_read_only_after_the_mounts`
- `doctor::probe::tests::an_agent_in_a_per_user_bin_dir_is_found_outside_path`,
  `a_script_in_a_per_user_bin_dir_is_named_as_passed_over`
- `doctor::checks::tests::an_agent_without_a_version_is_not_reported_as_a_version`
  (extended: dirs, skipped candidate, config fix)

Also: `the_pinned_program_survives_a_swap_of_its_path`,
`only_a_complete_elf_executable_header_is_native`,
`a_callers_command_is_never_bound`,
`a_script_in_path_outside_the_sandbox_is_not_reported_green`,
`a_script_in_the_host_path_is_named_as_passed_over`, and the descriptor bind
in the snapshot test.

Mutation proofs: dropping the per-user dirs from `search_dirs`, accepting
group write unconditionally, `--bind` instead of `--ro-bind`, skipping the ELF
check, the install command as DOCTOR_007 fix, and a doctor without the
per-user search each turn at least one of these tests red. All run without
`bwrap`; the real start is CI's.

### Acceptance criteria
1. With OpenCode only in `~/.opencode/bin` or `~/.local/bin` (native binary)
   and the systemd `PATH`, a session starts the agent, and `humanitl sandbox
   argv` shows one `--ro-bind` of that file to `/run/humanitl/agent/opencode`.
2. A group- or world-writable candidate that is not the user's private group
   is not bound.
3. A script candidate is not bound and is named in AGENT_001 and DOCTOR_007.
4. DOCTOR_007 names the searched directories and offers `humanitl config set
   agent.command`, not a reinstall.
5. `tools/check_coupling.py` does not grow.

### Pitfalls
- A wrapper in `~/.local/bin` that execs `~/.opencode/bin/opencode` works on
  the host and fails inside; hence `.opencode/bin` first and scripts skipped.
- `SessionContext` gained a field; every literal names it.
- The doctor runs in the daemon, so its `PATH` and `HOME` are the systemd ones.
- `/run/humanitl` is reserved for adapter files; the launcher's own bind is
  the one entry there that names a host file besides shim and socket.

### References
HUM-037, HUM-135, HUM-136, HUM-139, HUM-263. `docs/SECURITY.md` section on
binds and `docs/THREAT-MODEL.md` K-17 changed in the same commit.

## HUM-275 · Settings are reachable without a shortcut, and LLM_002 leads to the key field (before HUM-060)

### Context
On the owner's machine (v0.0.6) the status line said the endpoint "requires an
access key that is not provided". The app offered no visible way to enter one:
the settings sheet opens only with `Ctrl+,`, and the LLM_002 diagnostic has no
fix that leads there. The owner found the shortcut by chance.

### Goal
Settings have a visible entry, and every diagnostic that a setting resolves
opens the settings sheet at that field.

### Specification
1. A settings button (gear icon, tooltip "Settings (Ctrl+,)") sits in the
   shell where it is visible in every view and with the sidebar collapsed
   (HUM-254). It opens the same sheet as `Ctrl+,`.
2. LLM_002 carries `FixAction::ChangeSetting { key: "llm.api_key_ref", value:
   "" }` from the daemon (probe and status line). The app renders a
   `ChangeSetting` fix as a button "Open settings" that opens the sheet,
   scrolls to the field of `key` and focuses it; for `llm.api_key_ref` that is
   the access-key field. Keys the sheet does not show keep today's rendering.
3. CLI: the same fix prints `humanitl llm key set` as before (no change to the
   CLI text).
4. ARB keys in `en` and `de`; widget test and golden of the button in both
   themes.

### Tests
- `settings_entry_test`: the gear button exists with the sidebar open and
  collapsed and opens the sheet.
- `change_setting_fix_test`: an LLM_002 diagnostic with the fix opens the sheet
  with the key field focused.
- Daemon: `llm_002_carries_the_change_setting_fix`.
Mutation proofs: removing the button, dropping the fix in the daemon, opening
the sheet without focusing the field each turn a test red.

### Acceptance criteria
1. Without knowing `Ctrl+,`, a user reaches the settings in one click from any
   view.
2. LLM_002 in the status line offers a button that lands in the key field.
3. `flutter analyze`, goldens and the daemon tests are green.

## HUM-276 · The "no tray" notice can be dismissed for good and sits under the header (before HUM-060)

### Context
On a desktop without a StatusNotifier host (sway without a tray bar) the app
shows UI_002 "This desktop has no tray" in the left column. The owner could not
dismiss it, it took space in the queue column, and after a restart it was back.

### Goal
The notice is shown once where it does not cover work, can be dismissed with a
reachable button, and stays dismissed across restarts.

### Specification
1. The notice is a banner under the header, full window width, not inside the
   queue or sidebar column.
2. Its "Dismiss" button is reachable by mouse and keyboard (focus order, Esc
   does not dismiss by accident) and works in both widths of the sidebar.
3. Dismissal is stored in the app's local preferences (the store HUM-254 uses
   for the sidebar state), keyed by the diagnostic code. A later UI_002 after
   a restart is not shown again; it stays visible in Diagnostics / the doctor.
4. Nothing else about tray detection changes.

### Tests
- `attention_notice_test`: the banner sits under the header, not in the queue
  column; Dismiss hides it.
- `notice_dismissal_persists_test`: after Dismiss and a rebuilt app with the
  same preferences the banner stays hidden; with fresh preferences it shows.
- Goldens of the banner in both themes.
Mutation proofs: not writing the preference, rendering in the old place, each
turns a test red.

### Acceptance criteria
1. The notice can be dismissed and does not return after a restart.
2. It does not sit in the left column.

### Umsetzung (HUM-276)

- **Ort.** Der Meldungsplatz `ShellNotices` stand schon unter der Kopfzeile,
  die Karte darin war aber eine `HDiagnosticCard` von 560 px am linken Rand
  und lag damit optisch über Leiste und Queue. `AttentionNoticeCard` ist jetzt
  `AttentionNoticeBanner` (`app/lib/features/tray/widgets/attention_notice.dart`):
  ein Banner über die ganze Fensterbreite, getönt im Schweregrad, Linie unten
  (`docs/UX.md` 3.7), mit Code- und Schweregrad-Badge, Titel, Grund, Detail und
  Vorschlag (`FixControl`), „Dismiss" oben rechts. Der Doku-Verweis entfällt im
  Banner; bei `UI_002` ist er derselbe Link wie der Vorschlag.
- **Tastatur.** „Dismiss" liegt in der Lesefolge nach der Kopfzeile und vor dem
  Kopierknopf des Vorschlags; `Space` schließt, `Esc` nicht. Vom
  Intercept-Bildschirm aus erreicht `Shift+Tab` das Banner. Ein `Tab` nach vorn
  bleibt dort in der Gruppe von `HResizablePanes` und erreicht weder Kopfzeile
  noch Leiste noch Banner; das ist älter als dieses Issue (Kandidat für ein
  eigenes Issue).
- **Merken.** `DismissedNotices` (`app/lib/features/shell/providers/notice_dismissal.dart`)
  liest und schreibt `noticeDismissed.<Code>` in `ui-state.json`, denselben
  Speicher wie die Leistenbreite (HUM-254). Er liegt in der Shell, weil `tray`
  `intercept` (Ort von `UiStateFile`) nicht importieren darf
  (`tools/check-deps.sh`). Gemerkt wird nur eine Meldung aus `showOnce`
  (`AttentionNotice.lasting`): eine Tatsache über den Schreibtisch. Eine
  Meldung aus `show` (`IPC_003`, Verbindung weg) betrifft ein Ereignis und
  kommt beim nächsten wieder. `TrayHost` erkennt das fehlende Tray weiter und
  zeigt nur das Banner nicht; die Tray-Prüfung des Doctors und des
  Setup-Bildschirms ist unverändert.
- **Tests.** `attention_notice_test.dart`: `attention_notice_test: under the
  header, rail collapsed|expanded` (Banner beginnt an der Unterkante der
  Kopfzeile, `left == 0`, volle Breite, Leiste und `QueuePane` darunter,
  Dismiss rechts, Klick schließt) und `attention_notice_test: the keyboard
  reaches Dismiss, Esc not`; Kontrast der Bannerfläche in beiden Themes.
  `notice_dismissal_persists_test.dart`: `notice_dismissal_persists_test`
  (Neustart mit demselben Speicher: kein Banner, frischer Speicher: Banner)
  und `a_notice_about_one_event_is_not_remembered`. Golden
  `attention_notice_banner` in vier Varianten; kein bestehendes Golden ändert
  sich. `tray_host_test` legt jetzt `uiStateOverride()` vor, damit Dismiss nie
  in die Datei des Menschen schreibt, der den Test fährt.
- **Mutationsbeweise.** Schreiben in `DismissedNotices.remember` entfernt:
  `notice_dismissal_persists_test` rot. Prüfung in `TrayHost._seed` entfernt:
  `notice_dismissal_persists_test` rot (Banner nach Neustart da).
  `ShellNotices` in die Spalte rechts der Leiste verschoben (alter Ort):
  beide `under the header`-Tests rot (`left` 48 bzw. 200 statt 0).

## HUM-279 · `humanitl sandbox attach` has a detach key (before HUM-060)

### Context
`humanitl sandbox attach` puts the terminal into raw mode and forwards every
byte, `Ctrl+C` included, to the session. The only way out today is closing the
terminal or a signal from outside. The owner wants to work like `screen`:
attach, detach, attach again, with the agent running on.

### Goal
A key sequence detaches the client and leaves the session running.

### Specification
1. Default prefix `Ctrl+A` (0x01), as in `screen`. After the prefix:
   `d` detaches (sends `close`, restores the terminal, exit 0 with the line
   "Detached; the session keeps running. Attach again: humanitl sandbox
   attach"); `Ctrl+A` or `a` sends one literal 0x01; any other byte sends the
   prefix and that byte unchanged.
2. `--detach-keys <spec>` overrides the prefix (`ctrl-a`, `ctrl-b`, ... `ctrl-]`;
   `none` disables detaching). Config key `terminal.detach_prefix` sets the
   default.
3. On attach, one line on stderr before raw mode: "Detach: Ctrl+A d".
4. The prefix may arrive split across reads; a prefix at the end of a read
   waits for the next byte, without a timeout.
5. `--read-only` detaches on the same key (no bytes go up anyway).
6. The app terminal is not affected.

### Tests
- `attach::tests::ctrl_a_d_detaches`, `ctrl_a_ctrl_a_sends_one_prefix`,
  `a_prefix_split_across_reads_still_detaches`,
  `an_unknown_key_after_the_prefix_is_forwarded`, `none_disables_detaching`.
- CLI integration: attach to a fake daemon, send `\x01d`, exit 0 and a `close`
  message, terminal restored.
Mutation proofs: forwarding the prefix unconditionally, dropping the split
handling, each turn a test red.

### Acceptance criteria
1. `Ctrl+A d` detaches, the session keeps running, a second `attach` shows it.
2. A literal `Ctrl+A` still reaches the agent (`Ctrl+A Ctrl+A`).
3. The terminal is restored after detaching.

### Umsetzung (HUM-279)

- **Filter.** `DetachFilter` in `daemon/bin/humanitl/src/cmd/attach.rs` ist eine
  reine Zustandsmaschine mit einem Bit („Präfix gelesen"); die fünf Tests der
  Spezifikation stehen dort als `cmd::attach::tests::*`, dazu
  `another_prefix_has_its_own_letter`. Die Lesart der Angabe (`ctrl-a` bis
  `ctrl-z`, `ctrl-\`, `ctrl-]`, `ctrl-^`, `ctrl-_`, `none`) ist
  `humanitl_config::DetachKeys::parse`, dieselbe für `--detach-keys` (`CLI_004`)
  und `terminal.detach_prefix` (`CONFIG_003` beim Laden). `ctrl-[` ist
  ausgenommen, weil jede Pfeiltaste mit `ESC` beginnt, ebenso `ctrl-h`, `ctrl-i`,
  `ctrl-j` und `ctrl-m` (Rücktaste, Tab, Enter). Die Gruppe `terminal`
  ist `x-project-scope = denied`: Ein Projekt-Profil darf die Taste weder
  ändern noch abschalten (`a_cloned_project_cannot_take_the_detach_key`).
- **Abhängen.** Der Tastenleser meldet die Taste, `close` schickt danach der
  Befehl selbst; so kann das Ende des Stroms nie vor der Meldung ankommen. Der
  Leser wartet nie auf den Kanal zum Daemon: Er füllt eine Warteschlange von
  höchstens 1 MiB, die ein eigener Weiterreicher leert, und sieht die Taste
  deshalb auch, wenn die Sitzung ihre Eingabe nicht liest. Abgeben, `close`
  und das Ende des Stroms stehen zusammen unter einer Frist von zwei
  Sekunden; dann gibt der Befehl das Terminal zurück und endet mit 0 und der
  Zeile der Spezifikation auf `stderr`.
- **`--read-only`** setzt an einem Terminal jetzt den Rohmodus, um die Taste zu
  sehen; `Ctrl+C` ist dort ein Byte und hängt ebenfalls ab. An einer Pipe liest
  er weiterhin nichts, mit `--detach-keys none` bleibt sein Terminal gewöhnlich.
- **Tests am Terminal** (`daemon/bin/humanitl/tests/tty.rs`, Pseudoterminal und
  Fake): `ctrl_a_d_detaches_and_gives_the_terminal_back` (Exit 0, `close` beim
  Fake, kein `data`, Terminal gewöhnlich, zweites `attach` kommt an),
  `ctrl_a_twice_reaches_the_agent`, `read_only_detaches_on_the_same_key`,
  `detach_keys_none_forwards_ctrl_a_d`. Der Fake der Kommandozeilen-Tests
  zeichnet dafür die Art jeder Terminal-Nachricht auf
  (`FakeServer::terminal_inputs`), denn `close` und das Ende des Stroms
  beantwortet er gleich.
- **Mutationsbeweise.** Präfix immer weiterreichen: fünf der sechs Filtertests
  und `ctrl_a_d_detaches_and_gives_the_terminal_back` rot. Präfix am Ende eines
  Stücks sofort weiterreichen: `a_prefix_split_across_reads_still_detaches` und
  `an_unknown_key_after_the_prefix_is_forwarded` rot. Kein `close` beim
  Abhängen: `ctrl_a_d_detaches_and_gives_the_terminal_back` und
  `read_only_detaches_on_the_same_key` rot. Nach dem Review: Leser wartet auf
  das Weiterreichen, oder Abgeben und `close` ohne Frist:
  `a_session_that_reads_nothing_still_lets_go` rot (Fake
  `FakeServer::start_with_stalled_terminal`, 4 MiB eingefügt). `Ctrl+C` erst
  nach dem Filter geprüft: `read_only_ctrl_c_detaches_even_as_the_prefix` rot.

## HUM-280 · The agent starts from a shell inside the sandbox (before HUM-060)

### Context
`humanitl run -- bash` gives a shell in the sandbox with the adapter's config
files and environment (model, key file, CA), because the daemon applies the
adapter contribution to caller commands too. The agent program itself is not
there when it lives outside the mounts: HUM-278 binds only the configured
agent and only as the session's own command. The owner wants to open a shell,
start `opencode` or `pi` there, stop it with `Ctrl+C`, and start it again.

### Goal
In a session started with a caller command, the configured agent is on the
shell's PATH with the same model, key and config as a daemon-started agent,
and `Ctrl+C` in the shell stops it.

### Specification
1. When the session command comes from the caller, the daemon still binds the
   configured agent program by HUM-278's rules (same search, same checks, fd
   bind) at `/run/humanitl/agent/<name>`, and prepends `/run/humanitl/agent`
   to the sandbox PATH. The caller's own command is still never bound.
2. The adapter's files and environment are the same as for a daemon-started
   agent (already so; a test pins it).
3. Job control: the session's first process gets the session's pseudo-terminal
   as controlling terminal, so `Ctrl+C` in an interactive shell interrupts the
   foreground program, not the shell. The pty is the daemon's own, never the
   user's terminal, so TIOCSTI cannot reach the host. `docs/SECURITY.md` and
   `docs/THREAT-MODEL.md` say why `--new-session` stays and how the controlling
   terminal is set.
4. `humanitl run -- bash` attaches interactively like `humanitl sandbox attach`
   when stdin is a terminal (raw mode, resize, HUM-279 detach key); otherwise
   it keeps today's read-only behaviour.
5. The doctor and the preflight do not change.

### Tests
- `ipc::sandbox`: a caller command plan contains the agent bind and the PATH
  entry; the caller's command is not bound.
- Sandbox integration (CI, real bwrap): `bash -ic 'command -v opencode'` prints
  `/run/humanitl/agent/opencode`; `OPENCODE_CONFIG` points at the adapter file.
- PTY integration: an interactive shell, `sleep 100`, then `\x03`, the shell
  prompt comes back and the session still runs.
- CLI: `run -- bash` with a terminal enters raw mode; without one it does not.
Mutation proofs: not binding for caller commands, no PATH entry, no
controlling terminal, each turns a test red.

### Acceptance criteria
1. `humanitl run -- bash`, then `opencode` (or `pi` with HUM-274) starts the
   agent with the configured model and key, without further setup.
2. `Ctrl+C` stops the agent and returns to the shell; `exit` ends the session.
3. Detach and attach work as in HUM-279.
4. The security statement in `README.md` holds; SECURITY.md and THREAT-MODEL.md
   are updated in the same commit.

### Umsetzung (HUM-280)

- **Beitrag für jedes Kommando.** Punkt 2 der Spezifikation stimmte nicht:
  Bis hierher bekam ein Kommando, das nicht der Agent ist (`-- bash`), keinen
  Beitrag des Adapters (HUM-135, so auch der Satz in HUM-274). Der Daemon
  ruft `agent_contribution` jetzt für jedes Kommando
  (`daemon/crates/ipc/src/sandbox.rs`); Dateien, Umgebung und Schlüsseldatei
  sind dieselben wie für einen vom Daemon gestarteten Agenten. Der Test
  `a_named_agent_still_gets_its_environment` sagt das jetzt für `bash` und
  `sh -c opencode`. Der Weg ohne Daemon (`humanitl sandbox run`, die
  Escape-Suiten) bleibt bei der Regel von HUM-135.
- **Bind und `PATH`.** `agent::plan_configured_agent` (`locate.rs`) plant den
  konfigurierten Agenten mit dem Kontext des Aufrufers, aber dem Kommando aus
  `agent.command` oder der Vorgabe des Adapters, über dieselbe
  `plan_agent_program`-Regel von HUM-278. Das Kommando des Aufrufers bleibt
  unverändert und wird nie gebunden. `SandboxProfile::effective_env` setzt
  `/run/humanitl/agent` an den Anfang von `PATH`, sobald ein Bind besteht,
  nie doppelt. Für pi gibt es nichts zu binden (Skript, HUM-274).
- **Steuerndes Terminal.** `--new-session` bleibt. Neuer Shim-Schalter
  `--controlling-terminal` (`SHIM_CONTROLLING_TERMINAL`), gerendert nach
  `--proxy-port`, nur wenn der Start am eigenen Pseudoterminal des Daemons
  steht (`LaunchInputs::controlling_terminal`, gesetzt aus
  `StdioMode::Pty`); die Vorschau des Daemons zeigt ihn mit
  (`with_controlling_terminal(true)`), `humanitl sandbox argv` nicht. Das
  Kind des Shims ruft vor dem Filter `setsid` und `TIOCSCTTY` mit Argument 0
  auf Deskriptor 0; ein Fehlschlag ist eine Warnung, kein Abbruch. Der Daemon
  öffnet die Untertanenseite jetzt mit `O_NOCTTY`. `SandboxHandle::interrupt`
  signalisiert zusätzlich die Vordergrundgruppe (`tcgetpgrp` auf der
  Herrscherseite), weil der Agent die Gruppe des Init verlassen hat.
- **`humanitl run -- CMD` am Terminal.** Mit Terminal auf `stdin`, ohne
  `--ask terminal` und ohne `--json` hängt sich der Lauf bei `running` über
  `cmd::attach::attach` an (Rohmodus, Größe, Taste von HUM-279) und liest die
  Ausgabe nicht mehr aus dem Ereignisstrom. Abhängen endet mit 0 und lässt
  die Sitzung laufen, ein Signal stoppt sie wie bisher, das Ende der Sitzung
  liefert den Exit-Code. Sonst bleibt es beim Mitlesen.
- **Tests.** `ipc::sandbox::tests::a_callers_command_gets_the_configured_agent_on_its_path`,
  `bwrap_args_snapshot::a_bound_agent_puts_its_directory_first_on_the_path`,
  `the_controlling_terminal_is_asked_for_only_at_the_daemons_terminal`, in
  `daemon/crates/ipc/tests/terminal.rs` (echtes `bwrap`, echter Shim)
  `a_shell_finds_the_configured_agent_on_its_path` (`bash -ic`,
  `command -v opencode` ergibt `/run/humanitl/agent/opencode`,
  `OPENCODE_CONFIG` lesbar) und
  `ctrl_c_interrupts_the_foreground_program_not_the_shell`, in
  `daemon/bin/humanitl/tests/tty.rs` `run_with_a_command_at_a_terminal_attaches`
  und `run_with_a_command_without_a_terminal_does_not_attach`, dazu
  `cmd::run::tests::only_a_callers_command_at_a_terminal_attaches` und der
  Schalter in den Tests des Shims.
- **Mutationsbeweise.** Kein Bind für Kommandos des Aufrufers:
  `a_callers_command_gets_the_configured_agent_on_its_path` und
  `a_shell_finds_the_configured_agent_on_its_path` rot. Kein Eintrag in
  `PATH`: dieselben zwei und `a_bound_agent_puts_its_directory_first_on_the_path`
  rot. Kein steuerndes Terminal, im Shim oder im Launcher:
  `ctrl_c_interrupts_the_foreground_program_not_the_shell` rot (`bash: no job
  control in this shell`). Kein Anschluss in `run`:
  `run_with_a_command_at_a_terminal_attaches` und
  `only_a_callers_command_at_a_terminal_attaches` rot.
- **Review Runde 1.** `/run/humanitl/agent` steht genau einmal und vorn in
  `PATH`, auch wenn er schon weiter hinten stand (sonst gewänne ein
  gleichnamiges Programm in `/work/bin`). Ein Kommando des Aufrufers startet
  ohne Schlüssel, wenn der Speicher ihn nicht herausgibt, und meldet
  `LLM_009` als Warnung (`a_shell_starts_without_a_key_the_store_does_not_give`);
  ohne Kommando bleibt `LLM_009` blockierend, `CONFIG_003` und Fehler der
  Adapterdateien ebenso. Bekommt `run` am Terminal `TERM_001`, weil schon
  jemand schreibt, hängt es sich lesend an und sieht den Rückstand.
  Mutationsbeweise: Eintrag nicht verschoben macht
  `a_bound_agent_puts_its_directory_first_on_the_path` rot, Schlüssel blockiert
  die Shell macht den neuen Test rot. `docs/guides/agent-profiles.md`
  nachgezogen.
- **Review Runde 2.** Im lesenden Rückfall nach `TERM_001` beendet ein Signal
  nur den Zuschauer (Exit `128 + n`, Terminal zurück) und stoppt die Sitzung
  nicht, wie `sandbox attach --read-only`; sonst stoppte `Ctrl+C` bei
  `terminal.detach_prefix = "none"` eine Sitzung, die dieser Lauf nur sah
  (`signal_stops_session`, Test
  `a_watching_run_leaves_the_session_on_a_signal`, Mutation rot).
- **Doku.** `docs/SECURITY.md` (Abschnitt 3 und 5.3, Changelog),
  `docs/THREAT-MODEL.md` (K-06, K-17, Änderungsverlauf), `docs/cli.md`
  („A shell for the agent"), `docs/reference/cli.md` neu erzeugt.

## HUM-281 · The concurrent start test counts one sandbox twice

Done (b8e11d0). `only_one_of_two_concurrent_starts_gets_the_session` failed on
main about three times in four with a real bwrap. The refused start reports,
by design, the state of the running session after CLI_005; once the winner
reached `Running`, both streams carried `Running` and the old count gave two.
The test now counts distinct non-empty sandbox ids across `Running` statuses.
The daemon is unchanged; HUM-277's timing made the race visible.

## HUM-282 · The project folder has a visible "Choose folder" button (before HUM-060)

### Context
On v0.0.6 the Setup row "Project folder" shows "No project folder" and the
ro/rw switch. The text is a ghost button that opens the folder dialog, but
nothing says so; the owner looked for a way to choose the folder and found
none. The CONFIG_013 card below offers no action either.

### Specification
1. `WorkDirPicker` shows a visible button "Choose folder…" (folder glyph,
   outline variant) next to the path, in Setup and in the Sandbox header; the
   path itself stays a label (ellipsized, full path on hover). With a folder
   chosen the button reads "Change…".
2. The CONFIG_013 card carries the same action as a button "Choose folder"
   that opens the dialog; locked while a session runs, with the existing
   locked sentence.
3. A cancelled dialog changes nothing; a failing desktop portal shows a
   diagnostic with the CLI alternative (`humanitl run` in the folder) instead
   of doing nothing.
4. ARB en/de, widget tests, goldens both themes.

### Acceptance criteria
1. Without prior knowledge a user finds and uses the folder chooser in Setup.
2. CONFIG_013 offers the button.

## HUM-283 · Pasted endpoint and key are cleaned, and a stray quote is refused (before HUM-060)

### Context
The owner pasted the endpoint from a config file as
`https://llm.hzo.schutzwerk.net/api/v1/"`. App and daemon took it; the probe
asked `/api/v1/%22/v1/models` and reported LLM_003 next to "answered with 0
models". The access key store trims only trailing whitespace, so a pasted key
with quotes or a leading space is stored as is and the server answers 401.

### Specification
1. App: the endpoint field and the key field trim surrounding whitespace and
   one pair of matching or a single trailing/leading `"`/`'` on paste and on
   submit.
2. Daemon (`humanitl-config` validation of `llm.endpoint`, also `SetConfig`
   and `config set`): a URL containing `"`, `'`, `<`, `>`, a backslash or
   whitespace is refused with CONFIG_00x, why naming the character and its
   position, fix `ChangeSetting { llm.endpoint }` with the cleaned value.
3. Key store (`LlmKey` set, CLI `llm key set`): surrounding whitespace and
   quotes are stripped before storing; a key with inner whitespace is refused
   with a diagnostic (never echoing the key).
4. The probe's summary line does not say "answered with 0 models" when it
   reports LLM_003; it says what came back (status, content type).

### Acceptance criteria
1. Pasting the owner's endpoint with the quote results in the clean URL or a
   clear refusal, never a `%22` request.
2. A key pasted with quotes works.

## HUM-284 · A package update replaces the running daemon without a command (before HUM-060)

### Context
After upgrading the .deb from 0.0.5 to 0.0.6 the owner's systemd user manager
still ran the 0.0.5 daemon and warned "unit file … changed on disk. Run
'systemctl --user daemon-reload'". Only `systemctl --user daemon-reload &&
systemctl --user restart humanitld` by hand brought 0.0.6. The owner wants no
command after an update.

### Specification
1. `postinst` (configure, upgrade): for every user with a running user manager
   (`loginctl list-users`, `/run/user/<uid>/systemd`), run as that user
   `systemctl --user daemon-reload`, then restart `humanitld.service` only if
   it is active and no agent session is running (asked over the daemon socket
   with the CLI as that user, short timeout; unknown means "running"). With a
   running session, set a marker so the daemon exits after the session ends
   and socket activation starts the new binary; the app shows the HUM-277
   version notice meanwhile.
2. `prerm`/`postrm` stay as they are except for the same reload on remove.
3. Errors in this step never fail the package installation; they are logged
   to the journal with the manual command.
4. The app's HUM-277 version-mismatch notice gets a "Restart daemon" button
   that runs the reload + restart for this user when no session runs.
5. CI job `e2e-m4-package`: install the previous release, start the daemon,
   install the new .deb, assert `humanitl doctor` reports the new version
   without manual steps.

### Acceptance criteria
1. Upgrading the .deb with an idle daemon leaves the new version running.
2. With a running session the session survives and the new daemon starts
   after it ends.

## HUM-285 · The CLI ends quietly when the pipe closes

### Context
`humanitl doctor | head -5` panicked: `failed printing to stdout: Broken pipe
(os error 32)`.

### Specification
The CLI treats EPIPE on stdout as a normal end (exit 0 for a reader that
stopped early, like coreutils with SIGPIPE default): reset SIGPIPE to default
at startup or map the write error; no panic text. A test pipes `doctor` and
`flows` into a reader that closes after one line.

## HUM-286 · One layout grid: pane headers, action bar and the rail highlight line up (before HUM-060)

### Context
Screenshots of v0.0.6 on the owner's machine (2026-10-23): the Settings sheet
header is taller than the Queue pane header beside it, so their bottom borders
do not meet; the Intercept action bar (Allow / Edit + Allow / Block) sits inset
in its own box with gaps to the pane borders on the left, right and below; the
active rail item's highlight is not on the same row as the right pane's header
and keeps a gap to the border above it. Each surface invented its own
geometry.

### Goal
One layout concept for the whole shell, written down in docs/UX.md and as
tokens, applied everywhere: every horizontal divider that crosses the window
continues across panes, and highlights and bars are flush with the borders
they belong to.

### Specification
1. Tokens (app theme): `paneHeaderHeight` (one value for every pane header:
   Queue, detail, History, Rules, Sandbox, Audit, Setup, Settings sheet, any
   side sheet), `railItemHeight`, and the border width. Headers use the token,
   never their own padding to reach a height.
2. Rail: the first rail item starts directly under the header border with no
   gap; the active highlight fills the item's full height edge to edge and
   touches the border above. The rail's first row lines up with the pane
   header row of the content (same top, same height), so "Intercept" and
   "Queue" share one row.
3. Intercept action bar: a full-width footer of the detail pane, flush with
   the pane's left/right borders and the window's bottom edge (or status bar),
   separated from the content by one border; no inner card, no inset.
4. Settings sheet and other side sheets: header height and border line match
   the pane headers, so the divider continues across the window.
5. Rules view (screenshot 4): under the Saved/Temporary toolbar there are two
   stacked horizontal borders (toolbar bottom and list top, offset by a few
   px); one border only. General rule: two adjacent regions share one divider,
   owned by the upper one; nested containers do not add their own top border.
   The audit checks every view for doubled or offset dividers.
6. docs/UX.md gets a short "Layout grid" section with these rules; the
   impeccable audit/polish pass (layout) is the method, the owner's three
   screenshots are the acceptance reference.
7. Goldens regenerated; a geometry test asserts that the bottom edges of all
   visible pane headers and the sheet header are equal and that the active
   rail item's top equals the header border's bottom.

### Acceptance criteria
1. The four spots in the screenshots line up: sheet header vs Queue header,
   action bar flush, rail highlight flush and on the header row, one divider
   under the Rules toolbar.
2. No pane header computes its height outside the token.
3. Goldens and `make flutter-analyze` green.

## HUM-287 · Setup becomes a short onboarding, not a scroll list (before HUM-060)

### Context
The owner (2026-10-23): the Setup page is a long list of rows with cards
below each; it should be rethought for the current vision and workflow as a
mini onboarding that ideally finds everything already set and otherwise leads
through the few things that are missing, without a blunt scroll list.

### Goal
First start: a guided, step-by-step setup that confirms what is already fine
in one glance and puts the one thing that needs the user in front. Later: the
same page is a compact health overview, not a wall of cards.

### Specification
1. Design pass first (impeccable `shape`, then `onboard`): two to three
   rendered variants (dark and light, en) for the owner to pick from before
   building; the chosen one is recorded in docs/UX.md.
2. Steps in workflow order: background service, model server (endpoint,
   model, access key), project folder, sandbox checks; agent detection
   (HUM-278) is shown with the model step or as its own step, as the design
   decides.
3. A step that is green collapses to one line with its evidence; the first
   step that needs the user is expanded and focused, with its action as the
   primary button (Contact, Choose folder, Store key, Open settings). Steps
   do not need scrolling to reach the current one at 1280×800.
4. All green: a clear "Ready" state with one primary action (start a session
   / go to Intercept). The app opens Setup on first start and whenever a step
   blocks the start; otherwise it opens Intercept.
5. Diagnostics keep their content (code, why, fix) but appear inside the
   step, not as stacked cards below a list.
6. Uses the HUM-286 layout grid; reuses HUM-282's folder button and HUM-275's
   settings opener.
7. ARB en/de, widget tests per step state, goldens of first-run, partial and
   ready states in both themes.

### Acceptance criteria
1. On a machine where everything is set, Setup shows "Ready" with four green
   one-liners and no cards.
2. With the owner's 2026-10-23 state (no folder, key rejected) the first
   blocking step is open with its action; nothing below needs scrolling to
   find it.
3. The owner picked the design from rendered variants.

### Umsetzung (HUM-287)

Der Entwurf kam zuerst: drei Varianten als echte Flutter-Widgets mit echten
Schriften gerendert (V1 senkrechter Stepper, V2 Checkliste mit Detail-Pane,
V3 eine Karte mit Fortschrittsband), je erster Start, Lage des Owners und
Bereit, hell und dunkel. Der Owner hat am 2026-10-24 V1 gewählt, mit der
Überschrift in Worten aus V3 und dem „Next: …" aus V2; der Agent steht als
Zeile im Modellschritt. Festgehalten in `docs/UX.md` 4.10.

Gebaut:

- `SetupCheckRow` (`app/lib/features/setup/widgets/setup_check_row.dart`) ist
  ein Schritt: eine Zeile mit Marke, Titel, Zustandswort, Beleg und
  „Details", oder offen als Fläche mit Überschrift, eigenen Bedienelementen,
  Befund und Aktionen. `SetupFinding` zeigt den Befund im Schritt mit Code,
  Schweregrad, Titel, Grund, Detail, Abhilfe und Doku-Verweis, als
  Zustandsschiene statt als Karte. `SetupStepView` sagt, ob ein Schritt offen
  ist, ob er vorn steht und was danach kommt.
- `SetupScreen` sucht den vorderen Schritt nach der Reihenfolge
  (`setupCurrentStep`, `setupStepAfter`) und urteilt über keinen. Mit vier
  grünen Schritten sagt die Überschrift „Humanitl is ready"; die eine
  Primäraktion ist „Start a session", oder „Go to Intercept", solange eine
  Session läuft (`onOpenIntercept` aus `SetupHost`).
- `FixControl.primary` und `FixControl.offersButton` (HUM-282 Ordnerknopf,
  HUM-275 Weg ins Einstellungsblatt, Installer) und
  `LlmEndpointField.primary`. So hat ein Schritt genau einen Primärknopf,
  und der hat den Fokus. Die Probe schickt weiter den hinterlegten Schlüssel
  (HUM-288); der Modellschritt nennt weiter den gespeicherten Server
  (HUM-289).
- Beim Öffnen spricht das Setup den gespeicherten Modellserver einmal selbst
  an (`SetupHost`, Entscheidung des Owners vom 2026-10-24): dieselbe Probe wie
  „Contact it" mit dem hinterlegten Schlüssel, wieder bei „Check again" und
  wenn sich der gespeicherte Endpunkt ändert, nie nach einer Uhr. Ohne
  gespeicherten Endpunkt geht nichts hinaus. Das ersetzt die Zusage aus
  HUM-075; `opening_the_setup_contacts_nothing` heißt jetzt
  `opening_the_setup_contacts_only_the_saved_model_server`, dazu
  `opening_the_setup_without_a_saved_server_contacts_nothing`.
- Reviews, drei Runden: Jede Probe läuft durch ein Steuerwerk
  (`SetupLlmProbe`) mit Anfrage-Nummer; nur die neueste setzt das Ergebnis.
  Jedes Ergebnis trägt einen Fingerabdruck aus Adresse, aktivem Server,
  Schlüsselreferenz, Schlüsselstand und den Schlüsseländerungen dieser App
  (`llmKeyRevisionProvider`). Der Modellschritt zählt es nur für den
  Fingerabdruck dessen, was das Feld zeigt. Der gespeicherte Endpunkt kommt aus
  der Konfiguration (`setupLlmSavedProvider`), nicht aus dem Schnappschuss
  der Sandbox. `SetupHost` verdrahtet nur noch die Auslöser der Regel. Ein
  über „Details" geöffneter Schritt faltet sich, sobald er grün wird.
- Im offenen Ordnerschritt steht genau ein „Choose folder", der Primärknopf
  des Befunds; der Picker erscheint erst, wenn ein Ordner gilt.
- Wo die App öffnet: unverändert `_offerSetup` in `shell_screen.dart` auf
  `SetupState.needsSetup`. Ein erster Start hat keinen Ordner, und der fehlende
  Ordner sperrt (`CONFIG_013`); einen eigenen Merker für den ersten Start
  braucht der Client deshalb nicht.
- ARB en/de: 20 neue Schlüssel `setupTitleReady` bis
  `setupHeadSandboxUnmeasured`. `setupStart` heißt jetzt „Start a session" /
  „Session starten", und `setupIntro` verliert den ersten Satz, weil der
  Fortschritt („2 of 4 steps ready.") davor steht.

Tests: `app/test/features/setup/setup_onboarding_test.dart` (Lage des Owners
mit offenem ersten Sperrschritt, Primärknopf mit Fokus und „Next", kein
Scrollen bei 1280×800, grüne Schritte als eine Zeile, Übergabe nach vorn,
Bereit ohne Befund mit genau einer Primäraktion, laufende Session, Wahl des
vorderen Schritts, wo die App öffnet). Die bestehenden Tests der
Einrichtung öffnen einen gefalteten Schritt über `openSetupStep`
(`app/test/harness/app_harness.dart`). Neue Goldens `setup_firstrun` und
`setup_partial` in vier Varianten; neu erzeugt wurden die Goldens des
Setup-Bildschirms (`setup_*`) und des kalten Starts (`states_daemon_missing`,
`states_incompatible`).

Mutationsbeweise, je einzeln, je rot: grüner Schritt offen gezeichnet (6
Tests), Warteschlange statt Setup bei sperrendem Schritt (3), Abhilfe nicht
primär (1), vorn der gravste statt der erste Schritt (2), Schritte unter
die Falz geschoben (4).

Damit zeigt ein fertig eingerichteter Rechner „Ready" ohne Klick
(Akzeptanzkriterium 1). Belegt mit einem Doctor, der den Modellserver wie
ein echter Daemon nicht angesprochen hat (`DOCTOR_013`). Mutation: ohne die
Selbst-Probe wird `everything set shows Ready` rot.

## HUM-288 · Setup's model probe sends the stored key (before HUM-060)

### Context
On v0.0.6 the owner's Settings check reported "2 models offered. The access
key was sent", while Setup's "Contact it" on the same endpoint showed LLM_002
"none was sent". Setup called `probeLlm` without `useStoredKey`; Settings
passed it.

### Specification
Setup's probe passes `useStoredKey: true`. The daemon sends the stored key
only when `llm.api_key_ref` is set and the probed endpoint has the origin of
`llm.endpoint` (`server.rs` `probe_key`), so an address typed but not saved
gets no key. Test `the setup probe sends the stored key`; mutation: dropping
the flag turns it red.

## HUM-289 · Several saved model servers, switchable in the app (before HUM-060)

### Context
The owner (2026-10-23) wants to switch quickly in the GUI between several
saved model servers: endpoint, access key and model as one entry, each key in
the system keyring.

### Specification
1. Config: named entries `[llm.servers.<name>]` with `endpoint`, `model`,
   `api_key_ref` (`secret-service:humanitl/<name>`); `llm.active = "<name>"`.
   The flat `llm.endpoint`/`llm.model`/`llm.api_key_ref` keys stay as the
   active entry's view for compatibility (migration writes one entry
   `default`).
2. RPCs: list, add, rename, remove, activate; keys per entry through the
   existing `LlmKey` RPC with the entry name; keys never leave the daemon.
3. App: a switcher in the Settings model section and in the status bar's LLM
   item; adding an entry probes it with its own key. Switching takes effect
   for the next session (HUM-277); a running session keeps its server.
4. CLI: `humanitl llm server ls|add|use|rm`.
5. Bundled passthrough rule follows the active entry's origin; the security
   statement and THREAT-MODEL note that only the active origin gets the key.

## HUM-292 · Spike: Podman as a second sandbox backend, with and without a kernel boundary (after HUM-060, first)

### Context
The owner (2026-10-24) wants a sandbox with its own kernel as the first work
after 0.1.0. BACKLOG.md section 9 point 1 already plans a swappable sandbox
backend and names Podman (wish of 2026-10-16) next to Incus and microsandbox.
A plain rootless Podman container shares the host kernel like bwrap; a kernel
boundary needs another OCI runtime: `crun` with libkrun (`--runtime krun` or
the annotation `run.oci.handler=krun`) or Kata Containers. Rough estimate for
the whole backend: one to two weeks, so a time-boxed spike decides first.

### Goal
A written answer, backed by a throwaway prototype, to: can Humanitl launch a
session through Podman (plain and krun) and keep the three sandbox guarantees,
and what does the neutral `LaunchPlan` need for that.

### Specification
1. Time box one working day. Prototype code lives outside `main` (branch or
   scratch); the result is a document, `docs/adr/` draft plus a section in
   BACKLOG.md 9.1, not production code.
2. Questions to answer, each with evidence (command, output, test run):
   - Project folder and agent binary: fd-pinned binds (HUM-263, HUM-278) vs
     Podman volumes and, under krun, virtiofs; can the TOCTOU guarantee hold?
   - The proxy door: today a Unix socket in the sandbox; under krun the VM
     boundary needs vsock or a passt/TSI path. Does guarantee 2 (no network
     except through the door) hold, and how is it tested?
   - PTY and session lifecycle through conmon: attach, detach (HUM-279),
     resize, exit status, retire.
   - Rootfs: image or host bind, startup time, disk cost.
   - Prerequisites (`podman`, `crun` built with libkrun, `/dev/kvm`, Kata)
     and the Diagnostic with why and fix when one is missing.
   - `tests/escape/` against both levels; can CI run them (KVM on GitHub
     runners)?
3. Output: which `LaunchPlan`/`SandboxBackend` fields are bwrap-specific
   today (from `tools/coupling-baseline.toml` and the sandbox crate), the
   proposed neutral shape, the effort estimate per part, and a go/no-go per
   level (plain Podman, Podman + krun, Kata).

### Acceptance criteria
1. The document answers every question in item 2 with evidence or names it
   as open with the reason.
2. The owner decides from it which level comes next, with the estimate.

## HUM-293 · One project folder, chosen once, for app and CLI alike (before HUM-060)

### Context
The owner (2026-10-24) chose `~/Documents/Masterthesis` in Setup; the app
uses it. `humanitl sandbox run` started from `~` then failed with
SANDBOX_006 "sandbox.work_dir names /home/nburkert", although `config.toml`
holds no `sandbox.work_dir`: Setup's choice travels only in `Sandbox(Plan)`
and `Sandbox(Start)` (comment in `project_check.dart`, written before HUM-069
let `SetConfig` write `sandbox.work_dir`), and the CLI falls back to its
current directory. Two places pick the folder by different rules, and the
diagnostic names a config key that was never set. The owner wants the folder
set once and then used everywhere.

### Specification
1. Choosing a folder in Setup or Settings writes `sandbox.work_dir` through
   `SetConfig` (user `config.toml`, never a project profile; the key stays
   `x-project-scope = denied`). The daemon checks it with the same
   `MountPolicy::check_work_dir` before writing; a refused folder is not
   stored and Setup shows SANDBOX_006 in the step as today.
2. App, desktop launcher start and `humanitl sandbox run`/`humanitl run` all
   use the stored folder. `--work <path>` (`--sandbox-work-dir`) overrides it
   for one call and is not stored.
3. The current directory is used only when nothing is stored and no `--work`
   is given. Every SANDBOX_006 names the source of the path: config file
   (with its path), `--work`, environment variable, or "the current
   directory"; its fix then says "choose a project folder: humanitl config
   set sandbox.work_dir <dir>" first.
4. A session that is running keeps its folder; the new one applies to the
   next start (like HUM-277).
5. The `project_check.dart` comment and docs/UX.md, docs/cli.md, docs/guides
   are updated; the in-memory-only path for the chosen folder is removed
   (no second source of truth).

### Tests
- App: choosing a folder calls SetConfig with `sandbox.work_dir`; a refused
  folder is not stored and shows SANDBOX_006 in the step.
- Daemon: SetConfig with `$HOME` as work_dir is refused with SANDBOX_006 and
  the file stays unchanged.
- CLI: with a stored folder, `sandbox run` from another cwd uses the stored
  one; `--work` overrides without writing; with nothing stored the cwd is
  used and a refused cwd's SANDBOX_006 says "the current directory".
Mutation proofs: dropping the SetConfig call in the picker, ignoring the
stored folder in the CLI, each turns a test red.

### Acceptance criteria
1. After choosing `~/Documents/Masterthesis` in Setup once, `humanitl sandbox
   run` from `~` runs in that folder.
2. The diagnostic never names `sandbox.work_dir` for a path that came from
   somewhere else.

### Umsetzung (HUM-293)

- **Eine Quelle.** Der Ordner aus Setup, Sandbox-Kopf und Einstellungen geht
  über `SetConfig` als `sandbox.work_dir` in die `config.toml` des Nutzers
  (`SandboxStatusNotifier.chooseWorkDir`, danach ein `Plan` ohne Ordner).
  `DaemonClient.planSandbox`/`startSandbox` haben kein `workDir` mehr; der
  Daemon merkt sich keinen Ordner (`Pending.work_dir` entfällt). Ein
  `work_dir` in `Plan` gilt nur für diesen Plan, eines in `Start` nur, solange
  der Start seinen Anspruch hält (`Pending::starting_work_dir`); danach nennt
  die laufende Sitzung ihren Ordner selbst.
- **Prüfung vor dem Schreiben.** `config_rpc::check_chosen_work_dir` prüft mit
  `work_dir::check_requested`, denselben drei Stufen wie ein Ordner über den
  Socket (absolut, ohne `..`, vorhanden; `MountPolicy::check_work_dir_from`;
  unter `$HOME` oder der schon erklärte Ordner). Abgelehnt ist `SANDBOX_006`,
  die Datei bleibt Byte für Byte. Ein relativer Pfad bleibt `CONFIG_003` der
  Gegenprobe. `x-project-scope = "denied"` unverändert.
- **Die Quelle im Satz.** `humanitl_sandbox::WorkDirSource` (Datei mit Pfad,
  Profil, `--work`, Umgebungsvariable, aktuelles Verzeichnis, gerade
  gewählter Ordner, Anfrage ohne Angabe). Die Kommandozeile bestimmt sie aus
  der Herkunft von `sandbox.work_dir` (`Context::work_dir`) und schickt sie
  als `SandboxRequest.Start.work_dir_source` (neue Nachricht
  `WorkDirSource`, Vertrag `1.21`); `humanitl sandbox run` prüft vor dem Plan
  mit `check_project_dir`. Der Daemon sucht die Herkunft eines
  konfigurierten Ordners erst, wenn ein Befund entsteht (`with_source`). Die
  allgemeinen Sätze im Backend nennen „the project directory", nie mehr
  `sandbox.work_dir`. Die Kommandozeile druckt die Abhilfe als
  „choose a project folder: humanitl config set sandbox.work_dir <dir>"
  (`CHOOSE_PROJECT_FOLDER`), die Oberfläche öffnet weiter die Ordnerauswahl.
- **Fakes.** Der Dart-Fake legt einen angenommenen `sandbox.work_dir` in die
  Momentaufnahme, solange keine Sandbox läuft; der Rust-Fake zeigt ihn im
  Plan.
- **Tests.** Daemon: `config_rpc` (`$HOME` und ein Ordner außerhalb abgelehnt,
  Datei unverändert, Projekt geschrieben), `work_dir` (Quelle über die
  Leitung, bereinigt, drei Stufen), `sandbox` (Ordner eines Plans nicht
  gemerkt, Ordner eines Starts lebt mit dem Anspruch, Quelle der Anfrage und
  Datei im Satz), `sandbox_rpc` angepasst. Kommandozeile: gespeicherter Ordner
  aus `~`, `--work` ohne Schreiben, abgelehntes aktuelles Verzeichnis mit
  „the current directory" und der Abhilfe (Text und JSON), gespeicherter
  Ordner nennt seine Datei, `Context::work_dir` für alle Quellen. App:
  `project_refused_test` (Wahl ruft `SetConfig`, abgelehnter Ordner nicht
  gespeichert und `SANDBOX_006` im Schritt), `project_chooser_test`,
  Sandbox-Harness zählt gespeicherte Ordner.
- **Mutationen.** Ohne `SetConfig` im Picker: 7 App-Tests rot. Gespeicherter
  Ordner in der Kommandozeile übergangen: 4 Tests rot. Ohne Prüfung in
  `SetConfig`: 1 rot. Quelle im Daemon übergangen: 2 rot.
- **Review Runde 1.** Stufe 3 vergleicht auch mit dem aufgelösten
  `$HOME` (verlinktes Heimatverzeichnis wie auf Silverblue), die Politik prüft
  geschriebene und aufgelöste Form, letztere auch gegen das aufgelöste
  Heimatverzeichnis. `SetConfig` lehnt einen relativen Ordner selbst mit
  `CONFIG_003` ab, unabhängig von der Gegenprobe, und schreibt den
  aufgelösten Pfad. Quelle des Ordners reist mit `Pending::starting_work_dir`
  und `Running::work_dir_source`; der Ordner des Starts wird geleert, sobald
  `running` steht. `chooseWorkDir`: nur `SetConfig` zählt als Ablehnung, ein
  gescheitertes Neulesen der Konfiguration nicht, und nur die jüngste Wahl
  schreibt die Momentaufnahme.
- **Offen.** `action_bar_de_test` ist auf `main` schon rot (Schriftbreite),
  nicht Teil dieses Issues. Die Einstellungen planen nach dem Schreiben nicht
  neu; der Sandbox-Bildschirm fragt beim Sichtbarwerden ohnehin.

---

## HUM-274 · pi becomes a first-class agent next to OpenCode
Sprint: 5 · Size: M · Dependencies: HUM-037, HUM-039, HUM-140, HUM-261 · Blocks: HUM-060

### Context
Owner decision of 2026-10-22: release 0.1.0 integrates the pi coding agent (<https://pi.dev/>, binary `pi`) as cleanly as OpenCode, not only OpenCode; pi is the long-term focus (BACKLOG.md section 9, item 9). Until now `AdapterRegistry::builtin()` knows one adapter, and every agent-facing surface (doctor, preflight fixes, docs, settings form) names OpenCode.

Facts measured on pi 1.3.0 (`@earendil-works/pi-coding-agent`, published 2026-10-15; the old package `@mariozechner/pi-coding-agent` is deprecated and stops at 0.73.1), installed into a private prefix and run against `tests/e2e/mock_llm`:

- **Launch.** `pi` is a Node script (`#!/usr/bin/env node`, engines `node >= 22.19.0`). Installed with `npm install -g`, it lies under `/usr/local/bin` or `/usr/bin`, both inside the `/usr` mount. A Node from nvm or a prefix under `$HOME` is not visible in the sandbox, and then `env` fails with exit 127 although `pi` itself is reachable.
- **Configuration.** Everything lives in the agent directory, `$PI_CODING_AGENT_DIR`, default `~/.pi/agent`: `models.json` (custom providers), `settings.json`, `AGENTS.md` (global instructions), plus files pi writes itself (`auth.json`, `models-store.json`, `sessions/`, lock directories). The directory must stay writable; the files Humanitl places may be read-only. A read-only `settings.json` is tolerated: pi records the failed write of `lastChangelogVersion` and carries on (measured).
- **Custom provider.** `models.json` → `providers.<id>` with `baseUrl`, `api: "openai-completions"`, `apiKey`, `models: [{id, name, contextWindow, maxTokens}]`. pi posts to `<baseUrl>/chat/completions`, so `baseUrl` is the same value OpenCode gets (`llm_endpoint::openai_base_url`, including a path prefix, HUM-261). Measured requests: only `POST /v1/chat/completions`; no `GET /models`.
- **Key.** `apiKey` takes a literal, `$ENV`/`${ENV}` interpolation, or `!command`, which pi runs through `/bin/sh -c` and whose trimmed stdout is the key. `"!cat /etc/humanitl/pi/llm-api-key"` reads the key from the memfd file of HUM-140; it never stands in the environment, in `argv` of `pi` or in `models.json`. Only the path stands in the `argv` of the short-lived `cat`. A model whose provider has no resolvable key is not offered at all, so without a stored key the adapter writes a fixed placeholder that names itself (`humanitl-no-key`).
- **Phone-home.** Interactive mode, without switches, contacts `pi.dev` (`/api/latest-version`, the model catalogue `/api/models/providers/<id>`) and `github.com` (`/sharkdp/fd/releases/latest`, the same for ripgrep, when `fd` or `rg` is missing). Print mode contacts nothing. `PI_OFFLINE=1` stops all of it (version check, catalogue refresh, package update check, tool download); `PI_SKIP_VERSION_CHECK=1` and `PI_TELEMETRY=0` are the explicit second switches. With `PI_OFFLINE=1` the logging proxy of the probe saw no request at all. User-triggered paths exist and are left to the queue: `/share` (GitHub gist or `radius.pi.dev`), `/bug` upload (`radius.pi.dev`), `pi update` (npm, `pi.dev/api/installer`), packages from trusted project settings (npm).
- **Briefing.** pi reads `<agent-dir>/AGENTS.md` (and `CLAUDE.md`) on every start and the project's `AGENTS.md` from the working directory up, regardless of project trust.
- **Project trust.** `.pi/settings.json`, `.pi/extensions` and the like in `/work` load only after a trust decision; the default `defaultProjectTrust: "ask"` shows a prompt in the TUI. A trusted project can override `defaultProvider`/`defaultModel` and add extensions. All of their traffic still goes through the proxy.
- **Terminal.** Full-screen TUI by default (`tuiMode: "fullscreen"`): alternate screen, any-event mouse tracking (`?1003`), focus events, bracketed paste, synchronized output (`?2026`), kitty keyboard push and query (`CSI >7u`, `CSI ?u`), DA1, OSC 10/11/4 colour queries with a 100 ms timeout, OSC 7501 program status, OSC 8 links, `?2031`. `TERM=xterm-256color` and `COLORTERM=truecolor` work. `--ask terminal` must refuse it like OpenCode (CLI_002).

### Goal
`agent.adapter = "pi"` starts pi in the sandbox with the user's `llm.endpoint`, models and key, without any request pi makes on its own, with the same briefing, passthrough rule and preflight quality as OpenCode, selectable from the CLI and the settings form, documented in a guide section.

### Non-goal
- No new port, no change to the sandbox profile, no second bridge, no new mount.
- No pi extensions, skills or MCP configuration shipped by Humanitl; no `defaultProjectTrust` override (the human decides in pi's own prompt).
- No automatic choice of the adapter from the command name (`humanitl run -- pi` with `agent.adapter = "opencode"` starts pi without its configuration, as `-- bash` does today); candidate issue.
- No per-adapter bundled rule set: the daemon loads one `rules/default.yaml` for every session (HUM-038, HUM-104). pi's blocks are appended there.

### Affected paths
`daemon/crates/sandbox/src/agent/{mod.rs,opencode.rs,pi.rs,pi_config.rs}`, `agents/pi/` (new), `rules/default.yaml`, `daemon/crates/sandbox/src/doctor/{probe.rs,checks.rs}`, `daemon/crates/sandbox/tests/{pi_adapter.rs,default_rules.rs}`, `daemon/crates/core-types/src/diagnostics/codes.rs` (append), `docs/DIAGNOSTICS.md` (generated), `daemon/crates/ipc/src/{sandbox.rs,config_rpc.rs}`, `app/lib/core/domain/settings.dart`, `app/lib/features/settings/settings_sheet.dart`, `app/lib/core/ipc/fake_daemon_client.dart` (own entry only), `app/lib/l10n/diagnostic_l10n.dart`, `app/l10n/app_{en,de}.arb` (append), `app/test/features/settings/`, `docs/guides/{agent-profiles.md,install.md}`, `docs/SECURITY.md`, `README.md`.

### Specification
1. **Adapter `pi`** (`PiAdapter`, `ADAPTER_ID = "pi"`, program `pi`), registered after OpenCode in `AdapterRegistry::builtin()`. Full-screen TUI: yes.
2. **Command:** `["pi"]`, or `agent.command` / the caller's command unchanged.
3. **Environment:** `PI_CODING_AGENT_DIR=<home>/.pi/agent`, `PI_OFFLINE=1`, `PI_SKIP_VERSION_CHECK=1`, `PI_TELEMETRY=0`, `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, `NODE_EXTRA_CA_CERTS=<ca>`, `TERM=xterm-256color`, `COLORTERM=truecolor`, `LANG=C.UTF-8`. No variable carries a secret; all pi variables are added by name to the visible environment list of the sandbox screen.
4. **Files**, all read-only, all under `<home>/.pi/agent` or `/etc/humanitl/pi`, none under `/work`:
   - `<agent-dir>/models.json` from `agents/pi/models.json.tmpl`: provider `humanitl-local`, `baseUrl = llm_endpoint::openai_base_url(endpoint)` (fallback as OpenCode), `api = "openai-completions"`, one model entry per `llm.models` (placeholder model when empty, `LLM_004`), `apiKey = "!cat /etc/humanitl/pi/llm-api-key"` when a key is configured, else `"humanitl-no-key"`. Rendered through `serde_json`, never by text substitution.
   - `<agent-dir>/settings.json` from `agents/pi/settings.json.tmpl`: `defaultProvider = "humanitl-local"`, `defaultModel = <first model>`, `enableInstallTelemetry = false`.
   - `/etc/humanitl/pi/llm-api-key` (`SandboxFile::secret`) only when a key is configured; empty for a plan.
   - `<agent-dir>/AGENTS.md`: the shared briefing (`briefing::render`), unless `agent.briefing.enabled = false`.
5. **Default rules:** `default_rules()` returns the shared `rules/default.yaml`. Appended, with fixed ids `01920000-0000-7000-8000-0000000001xx`: block `pi.dev` path `/api/**` (version check, catalogue, installer), block `radius.pi.dev` (bug upload, Radius share), block `github.com` paths `/sharkdp/fd/releases/**` and `/BurntSushi/ripgrep/releases/**` (tool download). Nothing is allowed in addition.
6. **Passthrough:** the same rule as OpenCode (same id, methods, exact inference paths under the endpoint's base, HUM-261); the shared code is used, not copied.
7. **Preflight:** the command check of OpenCode, parameterised by the adapter (program, install command `sudo npm install -g --ignore-scripts @earendil-works/pi-coding-agent` (lands under `/usr/local`, which the sandbox mounts; the `pi.dev/install.sh` installer puts pi under `$HOME`), docs `https://github.com/earendil-works/pi/tree/main/packages/coding-agent/docs`), so `AGENT_001`/`002`/`004` name pi and its installer. New `AGENT_006` (Blocking): the resolved `pi` is a script whose `#!` line asks for `node` (directly or through `env`), and no startable `node` is found where the sandbox looks for it; `why` names both, `fix` is a `ChangeSetting` of `sandbox.env.PATH` when a `node` is mounted but not on the sandbox path, otherwise a `CopyCommand` that installs Node under `/usr`. Undecidable lookups stay silent (`backlog/CONVENTIONS.md` 4.13). `LLM_004` as for OpenCode.
8. **Selection:** `agent.adapter = "pi"` in `config.toml` or `humanitl config set agent.adapter pi`. The settings form gets a group "Agent" with a segmented control OpenCode / pi writing `agent.adapter` through `SetConfig`; `agent.adapter` joins `FORM_KEYS` and `FormKeys`, and `SetConfig` refuses an id the registry does not know (`CONFIG_003`, naming the known ids).
9. **Doctor:** `default_command`, `install_command` and the docs link of `DOCTOR_007` come from the registry instead of being fixed to OpenCode.
10. **Docs:** `docs/guides/agent-profiles.md` gets "## Using pi" (install, select, what Humanitl writes, key handling, phone-home table, terminal notes) and pi rows in "Known phone-home hosts"; `install.md` names both agents; `docs/SECURITY.md` names pi's key path and the `!cat` reference next to OpenCode's; README mentions pi. `agents/pi/README.md` documents the templates.

### Tests
- `daemon/crates/sandbox/tests/pi_adapter.rs`: environment (offline switches present, no secret, no network pointer); `models.json` valid JSON, base URL with and without path prefix, `/v1` once, key reference with key and placeholder without; key never in environment, `argv` or config text; `settings.json` names the first model; placeholder model and `LLM_004`; briefing in the agent directory, absent when switched off; nothing under `/work`, nothing on reserved targets; preflight `AGENT_001` with pi's installer, quiet when `pi` and `node` are there, `AGENT_006` when `node` is missing, silent when undecidable; passthrough equals OpenCode's; registry lists `opencode`, `pi`.
- Real sandbox (skips without `bwrap`, like the OpenCode integration tests): a fake `pi` script does what pi does at start — reads `models.json`, runs the `apiKey` command through `/bin/sh -c`, prints whether the key arrived, checks `PI_OFFLINE`, reads `AGENTS.md`, writes into the agent directory, fails to write `settings.json` — and the three guarantees hold. No network.
- `default_rules.rs`: each new pi rule blocks its target and leaves a neighbour (`pi.dev/docs`, another GitHub repository) unmatched.
- `config_rpc`: `agent.adapter = "pi"` is written, `"nope"` is refused with `CONFIG_003`.
- Flutter: the settings form writes `agent.adapter` and shows the current value; `diagnostic_l10n_test` covers `AGENT_006`.
- Mutation proof per new test, listed in the commit body.
- Manual: one real pi 1.3.0 run in a Humanitl sandbox against the mock LLM (`PI_OFFLINE` on: no held request before the first prompt; the prompt reaches the model through the passthrough).

### Acceptance criteria
- [ ] `agent.adapter = "pi"` starts pi in the sandbox with endpoint, models and key from the configuration; the key is in no environment variable, no `argv` of `pi` and no config file text.
- [ ] Before the first prompt, a pi session produces no held or blocked flow (offline switches plus bundled rules).
- [ ] Preflight names pi and its installer when `pi` is missing, and reports `AGENT_006` with `why` and `fix` when Node is missing in the sandbox.
- [ ] The adapter is selectable from `config.toml`, `humanitl config set` and the settings form; an unknown id is refused by `SetConfig`.
- [ ] The guide section "Using pi" exists and its phone-home table matches `rules/default.yaml`.
- [ ] Coupling ratchet unchanged or lower; README security statement unchanged (pi's key path is the same mechanism as OpenCode's).

### Pitfalls
1. pi writes into its agent directory at every start (`auth.json`, `models-store.json`, `sessions/`, lock directories): only files go in read-only, never the directory.
2. `!command` runs through the shell; the path is a constant, never a value from the configuration, so nothing from a user or a project reaches that shell line.
3. Without any `apiKey` pi hides the provider and starts with "no model". The placeholder must be non-empty.
4. `sandbox.env.PI_CODING_AGENT_DIR` wins over the adapter's value, and pi then reads none of the files; documented, not prevented (same class as `XDG_CONFIG_HOME` for OpenCode, which `AgentContext::config_home` follows).
5. `timeout` without `--foreground` around an interactive pi in a test harness leaves it in a background process group: raw mode fails, Enter arrives as `\n` and inserts a newline. A probe, not a product issue, but it cost an hour.
6. `fd` is missing in most sandboxes; pi warns once ("Offline mode enabled, skipping download") and works without it.

### References
HUM-037 (adapter contract), HUM-039 (passthrough), HUM-071 (briefing), HUM-135, HUM-139 (preflight), HUM-140 (key file), HUM-261 (path prefix), HUM-220 (terminal rendering), pi docs `docs/{configuration,models,settings,environment-variables,security}.md` in the npm package 1.3.0, <https://github.com/earendil-works/pi>.

### Umsetzung (HUM-274)

- **Adapter.** `PiAdapter` in `daemon/crates/sandbox/src/agent/pi.rs`, die
  Vorlagen in `pi_config.rs` aus `agents/pi/`, registriert nach OpenCode. Der
  Schlüssel geht als Datei `/etc/humanitl/pi/llm-api-key`, `models.json` nennt
  `"!cat /etc/humanitl/pi/llm-api-key"`; ohne Schlüssel steht dort
  `humanitl-no-key`. Endpunkt, Modelle und Schlüssel kommen wie bei OpenCode
  aus `AgentContext` (`ctx.llm`, `ctx.llm_key`), also aus dem aktiven
  Modellserver von HUM-289 samt der Regel, dass der Schlüssel nur mit dem
  Endpunkt desselben Eintrags geht (`agent_key` im IPC-Crate, für beide
  Adapter dieselbe Stelle).
- **Vorprüfung.** `command_preflight` in `opencode.rs` nimmt jetzt den Adapter
  (Programm, Installer, Doku) und behält die Suche und das Binden von HUM-278
  (`plan_agent_program`, `passed_over`). Neu ist
  `AgentAdapter::is_native_program` (Vorgabe `true`, pi `false`): Ein Skript
  bindet der Daemon nie (HUM-278), deshalb bieten `AGENT_001` und
  `DOCTOR_007` für pi den npm-Installer statt `agent.command` an, und der Satz
  sagt, dass pi unter einer Einhängung wie `/usr/local` liegen muss.
  `AGENT_006` (Node.js fehlt in der Sandbox) folgt nur, wenn das Kommando
  selbst in Ordnung ist.
- **Doctor.** `default_command`, `install_command` und der Doku-Link von
  `DOCTOR_007` kommen aus `AdapterRegistry::builtin()`.
- **Auswahl.** `agent.adapter` steht in `FORM_KEYS`; `SetConfig` lehnt eine
  unbekannte Kennung mit `CONFIG_003` ab und nennt die bekannten. Im
  Formular die Gruppe „Agent" mit `HSegmented` OpenCode / pi
  (`settings-agent`), `FormKeys.agentAdapter`, im Fake dieselbe Prüfung. Die
  Goldens der Einstellungen sind neu erzeugt (neue Gruppe).
- **Regeln.** Vier gebündelte Sperren `…0101` bis `…0104` in
  `rules/default.yaml`; `docs/reference/rules.md` nennt sie.
- **Doku.** „Using pi" und die pi-Zeilen der Phone-home-Tabelle in
  `docs/guides/agent-profiles.md`, pi in `install.md`, der Schlüsselpfad in
  `docs/SECURITY.md`, ein Satz in `README.md`, `agents/pi/README.md`.
- **Tests.** `daemon/crates/sandbox/tests/pi_adapter.rs` (15 Tests, davon einer
  in einer echten Sandbox mit einem falschen `pi`), `default_rules.rs`
  (`the_pi_rules_block_exactly_what_pi_does_on_its_own`), `config_rpc`
  (`pi` geschrieben, `aider` und `Pi` abgelehnt), `sandbox.rs`
  (`the_key_reaches_pi_through_a_sandbox_file_only`), `doctor::checks`
  (pi-Fall von `DOCTOR_007`), Flutter `settings_sheet_test.dart` (zwei Tests),
  `change_setting_fix_test.dart` (`agent.adapter` landet auf dem Schalter);
  `AGENT_006` deckt `diagnostic_l10n_test` über das Register ab.
- **Review Runde 1.** `AGENT_004` für pi schlägt den npm-Installer vor statt
  `install` des Einstiegsskripts, das ohne seine Paketdateien mit
  `ERR_MODULE_NOT_FOUND` endete. `AGENT_006` mit absolutem `#!`-Interpreter
  außerhalb der Einhängungen schlägt die Neuinstallation von pi mit einem
  Node.js unter `/usr` vor, nicht das Kopieren von node. Der Hinweis von
  `Skip::Script` („native binary") entfällt für Skript-Adapter. Die
  `$schema`-Zeilen der pi-Vorlagen sind weg (nicht gegen pi 1.3.0 geprüft).
  `node_interpreter` überspringt `env -u NAME`, `-C DIR` und `NAME=wert`.
- **Offen.** Der Handlauf mit einem echten pi 1.3.0 gegen den Mock-LLM steht
  aus.

---
## HUM-294 · A stored project folder gets the full folder check at every start (after HUM-060)

### Context
Found in the HUM-293 review (2026-10-24). `check_requested` in
`daemon/crates/ipc/src/work_dir.rs` skips the third stage (the folder lies
under `$HOME`) for the folder that is already stored in `sandbox.work_dir`,
because a stored folder used to count as declared by a person. Since HUM-293
the app writes that value through `SetConfig` too. SetConfig stores the
resolved path, so a stored link cannot be retargeted; a path component that
is later replaced by a link still is. The mount policy (deny list, `$HOME`
itself and its parents) still runs at every start, so the security statement
holds; only the under-HOME rule is skipped.

### Specification
1. Record who declared a folder outside `$HOME`: a value written by
   `humanitl config set` or by hand keeps today's exemption, a value written
   through `SetConfig` from the app gets all three stages at every start.
   Or, simpler: every start re-resolves the stored folder and compares the
   result with the stored text; a difference is `SANDBOX_006` naming the file.
2. The diagnostic names the config file as the source (HUM-293 `WorkDirSource`).

### Acceptance criteria
1. A stored folder whose parent was replaced by a link to a place outside
   `$HOME` after it was chosen in the app is refused at the next start with
   `SANDBOX_006` naming the config file. Test with a temporary home.
2. A folder outside `$HOME` declared with `humanitl config set` still starts.

### Umsetzung (HUM-294)

- **Variante.** Die einfachere: Jeder Plan und jeder Start schickt den
  gespeicherten Ordner durch `work_dir::check_requested`, alle drei Stufen
  gegen seinen heutigen aufgelösten Pfad (`Inner::is_stored_work_dir` in
  `ipc/src/sandbox.rs`, nur wenn weder Anfrage noch laufender Start noch
  laufende Sitzung einen Ordner nennt). Der Befund nennt über `with_source`
  die Datei (`sandbox.work_dir in <config.toml>`).
- **Unterschied nur, wo er zählt.** Ein Unterschied zwischen Text und
  aufgelöstem Pfad wird nicht pauschal abgelehnt, sondern nur, wenn Stufe 3
  an ihm hängt: Unter dem Heimatverzeichnis (geschrieben oder aufgelöst)
  bleibt er erlaubt. Sonst fiele das verlinkte Heimatverzeichnis aus der
  Review von HUM-293 (Silverblue, `/home/u` auf `/var/home/u`) für jeden von
  Hand oder mit `humanitl config set` geschriebenen Ordner heraus, und ein
  Verweis innerhalb des Heimatverzeichnisses ändert an der Sicherheitsaussage
  nichts; die Politik prüft weiter geschriebene und aufgelöste Form.
- **Erklärt ist der Text, nicht sein Ziel.** Die Ausnahme von Stufe 3 gilt nur,
  wenn der erklärte Text schon sein aufgelöster Pfad ist (`declared`). Damit
  fällt das Schlupfloch auch für `--work` und die Quelle `ConfigFile` der
  Kommandozeile, nicht nur für den Start ohne Ordner. Steht der erklärte Text
  selbst zur Prüfung und führt woandershin, sagt der Satz, wohin, und nennt
  `humanitl config set sandbox.work_dir <aufgelöst>` als Weg.
- **`humanitl config set`.** Die Kommandozeile schreibt `sandbox.work_dir`
  jetzt aufgelöst, wie `SetConfig` (`resolved_work_dir` in
  `bin/humanitl/src/cmd/config.rs`). So bleibt die Ausnahme für einen Ordner
  außerhalb von `$HOME`, auch wenn der Mensch ihn über einen Verweis tippt
  (`/data` als Verweis); der Verweis zählt beim Erklären, nicht danach.
  Relativ, mit `..` oder fehlend bleibt der Text unverändert und scheitert
  an der Gegenprobe wie bisher. Von Hand über einen Verweis geschrieben und
  außerhalb von `$HOME`: abgelehnt, der Satz nennt den echten Pfad.
- **Tests.** `sandbox`:
  `a_stored_folder_moved_out_of_home_by_a_link_is_refused_at_start`
  (temporäres Heimatverzeichnis, Wahl über `config_rpc::set`, Start grün,
  Elternverzeichnis durch Verweis nach außen ersetzt, Start und Plan
  `SANDBOX_006` mit Datei), `a_declared_folder_outside_home_still_starts`
  (erklärter Ordner außerhalb startet, erklärt über einen Verweis abgelehnt
  mit dem echten Pfad). `work_dir`:
  `a_declaration_holds_only_for_the_path_it_really_names`. Kommandozeile:
  `config_set_work_dir_writes_the_resolved_path`.
- **Mutationen.** Prüfung des gespeicherten Ordners beim Start entfernt: 2 rot.
  Alte Ausnahme (Ziel statt Text verglichen): 3 rot. Ausnahme ganz entfernt:
  3 rot (darunter AC 2). `config set` ohne Auflösen: 1 rot. Alle Dateien
  danach byte-gleich zurück (`cmp`).
- **Review Runde 1.** Zwischen der Prüfung im Daemon und dem Öffnen im
  Sandbox-Backend (HUM-263) konnte ein Verweis in den Pfad kommen; das Backend
  prüfte dort nur die Denylist. Jetzt reist der freigegebene aufgelöste Ort
  mit (`with_approved_work_dir`, neutral benannt, keine neue Kopplung), für
  den gespeicherten Ordner und für einen Ordner aus der Anfrage; das Backend
  bindet den Projektordner nur, wenn der Ort seines Deskriptors genau dieser
  ist, sonst `SANDBOX_006`. Die Vorgabe aus dem aktuellen Verzeichnis bleibt
  ohne Freigabe (nur Politik, wie bisher). `work_src` behält den
  geschriebenen Text, die Anzeige ändert sich also nicht (kein
  `/var/home/...` auf Silverblue); verglichen wird der aufgelöste Ort. Test
  `launcher::a_folder_that_no_longer_lies_where_it_was_approved_is_not_mounted`
  (Tausch zwischen Freigabe und Plan; ohne Freigabe ließe die Denylist das
  Ziel durch), im Daemon-Test trägt das Backend den freigegebenen Ort.
  Mutationen: Vergleich im Backend entfernt 1 rot, Freigabe im Daemon nicht
  weitergereicht 1 rot. Ein gespeicherter Ordner, den es nicht mehr gibt, ist
  schon im Plan `SANDBOX_006` (Kommentar und `docs/cli.md`). Satz neutral:
  „a symbolic link in its path leads elsewhere".
- **Review Runde 2.** `humanitl sandbox run` startet ohne Daemon und prüfte
  einen gespeicherten Ordner nur mit `check_project_dir`. Jetzt geht ein
  Ordner aus `sandbox.work_dir` (Datei, Profil, Umgebungsvariable) dort durch
  `humanitl_ipc::work_dir::check_requested`, der freigegebene Ort geht an
  `with_approved_work_dir` (`bin/humanitl/src/cmd/sandbox.rs`). Test
  `sandbox_run_refuses_a_stored_folder_moved_out_of_home_by_a_link`;
  Mutation (Prüfung übergangen) 1 rot. Nicht übernommen, mit Grund:
  `--work` und das aktuelle Verzeichnis auf diesem Weg (keine Erklärung im
  Spiel, die Politik prüft geschriebene und aufgelöste Form am Deskriptor;
  Stufe 3 für sie hier wäre eine neue Regel für die Escape-Suiten, eigenes
  Issue), `sandbox check` (Platzhalter-Ordner im Temp-Verzeichnis),
  `sandbox argv` (Vorschau, startet nichts), `escape-launch` (Testwerkzeug,
  Ordner aus seinen Argumenten, kein gespeicherter Ordner), `humanitl run`
  (startet immer über den Daemon, keinen eigenen Weg).

---
## HUM-295 · Settings re-plans the sandbox after the project folder changes (after HUM-060)

### Context
Found in the HUM-293 review. Settings writes `sandbox.work_dir` through
`SetConfig`, but the sandbox snapshot is planned again only when the sandbox
screen becomes visible or Setup chooses a folder itself, because a feature
does not import another.

### Specification
1. After a successful write of `sandbox.work_dir`, the sandbox snapshot is
   planned again, through a shared provider in `core/` or by watching the
   config revision, without a feature importing another.

### Acceptance criteria
1. Changing the folder in Settings shows the new folder in the sandbox
   header and Setup without switching screens. Widget test.

### Umsetzung (HUM-295)

- **Die Shell reicht den Ordner weiter.** `ShellScreen` hört in `initState`
  mit `listenManual` (sofort einmal) auf `sandbox.work_dir` in
  `configProvider` und gibt jeden lesbaren Wert an
  `SandboxStatusNotifier.storedWorkDirRead`. Die Shell hält ohnehin Sandbox
  und Einstellungen; kein Feature importiert ein anderes, und die
  Einstellungen bleiben unverändert (`configProvider.write` liest nach dem
  Schreiben neu). Der erste Wert ist der Ausgangspunkt, jeder abweichende
  danach löst einen `Plan` ohne Ordner aus. Auch ein Ordner, der nach einer
  Rückkehr der Verbindung anders gelesen wird (etwa nach
  `humanitl config set`), plant so neu.
- **Warum nicht im Notifier selbst.** Ein `ref.listen` auf `configProvider`
  in `SandboxStatusNotifier.build` verschob den Takt des Wiederverbindens:
  `setup_test.dart` „retry reconnects once the daemon is back" sah die Shell
  einen Frame früher. In der Shell besteht das Abonnement erst, wenn es eine
  Verbindung gibt.
- **Gespeichert gegen geplant.** Der Notifier merkt sich den zuletzt
  gelesenen Ordner (`_storedWorkDir`) und den, mit dem der Schnappschuss
  geplant wurde (`_plannedWorkDir`: der erste gelesene Wert, der Ordner einer
  Wahl, der gespeicherte bei `plan`). `_reconcileWorkDir` plant genau dann
  einmal, wenn beide verschieden sind und der Schnappschuss es erlaubt.
  Eine Änderung, die warten muss, geht nicht verloren:
  - Während `chooseWorkDir` läuft (`_choosing`), plant nichts daneben. Nach
    der letzten Wahl folgt der Abgleich; hat die Wahl ihren eigenen Ordner
    zurückgelesen, ist nichts mehr zu tun. Haben die Einstellungen währenddessen
    einen anderen Ordner geschrieben, wird er danach einmal geplant.
  - Vor dem ersten Schnappschuss ist unbekannt, ob eine Sandbox läuft; ist
    die Sandbox oben oder im Starten oder Stoppen, behält sie ihren Ordner
    (wie die gesperrten Ordnerknöpfe). In beiden Fällen plant ein
    `listenSelf` in `build` nach, sobald der Schnappschuss es erlaubt, also
    beim ersten Schnappschuss oder auf dem Weg nach `stopped`/`failed`.
- **Ältere Antworten.** Refresh und jeder Plan zählen gemeinsam in
  `_snapshots`; nur die jüngste Anfrage schreibt den Schnappschuss. Eine
  Statusabfrage, die vor einem Plan begann und nach ihm antwortet, stellt den
  alten Ordner nicht wieder her. Start und Stopp zählen nicht mit; ihre
  Ereignisse sind der Zustand der Sandbox selbst.
- **Tests.** `test/features/settings/settings_replans_sandbox_test.dart`
  (Widget): Ordner in den Einstellungen gewählt, über dem Sandbox-Bereich
  zeigt der Kopf den neuen Ordner, über dem Setup die Projektzeile; über
  einer laufenden Sandbox kein Plan. Die Pläne zählen ab dem Bau des Fakes,
  ein Plan auf den ersten Wert fiele auf. `test/features/sandbox/stored_work_dir_test.dart`
  (Provider, mit einer Weitergabe wie die Shell): erster Wert und derselbe
  Wert erneut planen nicht; ein erster Wert, der erst nach einem
  Lesefehler kommt, plant nicht; ein Ordner aus den Einstellungen plant einmal;
  ein verspäteter Refresh ersetzt den späteren Plan nicht; ein Ordner aus den
  Einstellungen während einer Wahl wird nach ihr geplant; eine Wahl plant
  einmal; eine Änderung vor dem ersten Schnappschuss wartet auf ihn; eine
  Änderung bei laufender Sandbox wird beim Stoppen geplant.
- **Mutationsbeweise.** Weitergabe in `ShellScreen` entfernt: beide Tests
  des Kriteriums rot. Generation aus `superseded` entfernt: der Test des
  verspäteten Refresh rot. Abgleich nach der letzten Wahl entfernt: der Test
  der Einstellungen während einer Wahl rot. `listenSelf` entfernt: die Tests
  „vor dem ersten Schnappschuss" und „laufende Sandbox" rot. Plan schon auf
  den ersten Wert: fünf Provider-Tests und der Widget-Test des Kopfs rot.

---
## HUM-296 · `humanitl run --config` with a folder outside HOME is judged against the file it names (after HUM-060)

### Context
Found in the HUM-293 review. `humanitl run --config other.toml` sends the
folder from `other.toml` with source `ConfigFile(other.toml)`. The daemon
allows a folder outside `$HOME` only when it equals the folder in its own
`config.toml`, so the start is refused although the person declared the
folder in the file they named.

### Specification
1. Decide: either the daemon accepts the declaration from a file the CLI
   names (it can read the file itself, under the same owner checks as its own
   config), or the refusal says plainly that only the daemon's config file
   declares folders outside `$HOME` and names it in the fix.

### Acceptance criteria
1. The case has a test and either starts or ends in a `SANDBOX_006` whose fix
   names the daemon's config file.

### Umsetzung (HUM-296)

- **Entscheidung: Ablehnung (Variante 2).** Der Daemon liest keine Datei,
  die ein Client nennt. Eigentümer- und Rechteprüfung trennen nicht den
  Menschen vom Prozess: Jede Datei, die der Nutzer schreiben kann, kann auch
  jedes Programm unter seiner Kennung schreiben, das den Socket erreicht.
  Läse der Daemon sie als Erklärung, würde jeder solche Client mit einer
  selbst geschriebenen Datei einen beliebigen Ordner außerhalb von `$HOME`
  einhängen lassen, den `SetConfig` aus demselben Grund ablehnt (HUM-293).
  Das erweiterte, was ein Aufrufer ohne Erklärung eines Menschen einhängt.
  Die Ablehnung lässt alles, was eingehängt werden darf, wie es ist; nur Satz
  und Abhilfe ändern sich, die Sicherheitsaussage im README bleibt.
- **Satz und Abhilfe.** `work_dir::check_requested` gibt für einen Ordner,
  der an Stufe 3 scheitert und dessen Quelle `ConfigFile(<datei>)` nicht
  die Datei des Daemons (`Paths::config_path`) ist, `SANDBOX_006` mit dem
  Satz „… counts as declared only in the daemon's config file <datei des
  Daemons>, not in a config file other than the daemon's (<datei>)" und der
  Abhilfe
  `CopyCommand("humanitl --config <datei des Daemons> config set
  sandbox.work_dir <aufgelöst>")` (`named_file_refused`, Pfade über
  `shell_path`). Die Datei des Daemons als Quelle behält den allgemeinen Satz
  und die Ordnerauswahl; der Fall des erklärten Texts, der über einen Verweis
  woandershin führt (HUM-294), geht vor.
- **`humanitl sandbox run --config`** startet ohne Daemon und prüft den
  Ordner gegen die Datei, die es selbst liest; dort ist der Aufruf der
  Mensch, kein Client am Socket. Unverändert.
- **Doku.** `docs/cli.md` (Absatz zu `--config`), `docs/SECURITY.md` 5.2
  (nur die eigene `config.toml` erklärt für den Daemon).
  `docs/THREAT-MODEL.md` nennt die Erklärung nicht; unverändert.
- **Tests.** `ipc::sandbox`:
  `a_folder_outside_home_from_a_named_config_file_names_the_daemons_file`
  (Start mit Quelle `other.toml` abgelehnt, Satz nennt beide Dateien, Abhilfe
  genau der Befehl mit der Datei des Daemons; nach der Erklärung dort startet
  dieselbe Anfrage). `ipc::work_dir`:
  `only_the_daemons_config_file_declares_a_folder_outside_home` (dazu: Datei
  des Daemons als Quelle behält Ordnerauswahl, erklärter Ordner startet
  auch mit fremder Quelle).
- **Mutationen.** `named_file_refused` nicht aufgerufen: 2 rot (beide neuen
  Tests). Vergleich mit der Datei des Daemons entfernt: 1 rot
  (`only_the_daemons_config_file_declares_a_folder_outside_home`). Datei
  danach byte-gleich zurück (`cmp`).
- **Review Runde 1 (Codex, zwei minor).** Der Vergleich der genannten Datei
  mit der des Daemons geht über die aufgelösten Pfade (roh, wenn sich einer
  nicht auflösen lässt); ein Verweis auf die Datei des Daemons heißt nicht
  „eine andere Datei". Nur der Satz hängt daran, erklärt ist weiter allein,
  was der Daemon gelesen hat. Die Kommandozeile schickt die Datei von
  `--config` absolut (`Context::work_dir`, gegen ihr aktuelles Verzeichnis),
  damit der Daemon einen relativen Pfad nicht gegen sein eigenes deutet.
  Tests: Verweis-Fall in `only_the_daemons_config_file_declares_a_folder_outside_home`,
  relativer Fall in `cmd::tests` (`Context::work_dir`). `cargo fmt` sauber.
- **Review Runde 1 (Sonnet, minor).** Der Satz sagt „a config file other than
  the daemon's (<datei>)" statt „a file named with --config": Auch ohne
  `--config` kann die Datei der Kommandozeile eine andere sein (anderes
  `XDG_CONFIG_HOME`). Test
  `the_fix_quotes_the_daemons_file_and_the_folder_for_the_shell` (Datei des
  Daemons und Ordner mit Leerzeichen und `'`); Mutation `.display()` statt
  `shell_path` in der Abhilfe: rot.

---
## HUM-297 · RUSTSEC-2026-0414 in pulldown-cmark turns the dependency check red

### Context
On 2026-10-29 the CI step "Licenses, advisories, duplicate crates"
(`make rust-deny`) failed for the HUM-271 branch although the branch changed
no dependency: the advisory RUSTSEC-2026-0414 (exponential event growth with
`ENABLE_WIKILINKS`) was published for `pulldown-cmark` 0.13.4. The crate is
only a build dependency (`prost-build` through `tonic-prost-build` of
`humanitl-ipc`), so no shipped binary parses Markdown with it; every branch
is red until the lock file moves.

### Specification
1. `cargo update -p pulldown-cmark --precise 0.13.5` in `daemon/`; only
   `daemon/Cargo.lock` changes.

### Acceptance criteria
1. `cargo deny check` reports advisories, bans, licenses and sources ok.

---
## HUM-304 · The agent knows it has web access through the proxy
Sprint: 5 · Size: S · Dependencies: HUM-071, HUM-274

### Context
Owner report of 2026-10-29: asked to look something up, pi answered that it has
no internet access. The owner wants the agent to know that it can fetch and
search the web, with every request controlled in Humanitl's queue.

The briefing (`agents/opencode/briefing.{en,de}.md`, rendered by
`daemon/crates/sandbox/src/agent/briefing.rs`, placed for pi as
`<agent-dir>/AGENTS.md`) opened with "No network interface: HTTP(S) leaves only
through the proxy in the proxy environment." A model reads the first three
words as "no internet". pi ships only the tools `read`, `write`, `edit` and
`bash`, so its way to the web is `curl`, `wget` or a script through `bash`.

Measured on 2026-10-29 (Node 24.22.1, pi 1.4.0 under `/usr/local`), against a
logging proxy on loopback with the proxy variables of the env kit and an empty
`NO_PROXY`:

- Node's built-in `fetch` ignores `HTTP_PROXY`/`HTTPS_PROXY` without
  `NODE_USE_ENV_PROXY=1`: it connects directly and fails (`ECONNREFUSED` for
  `127.0.0.1`, `EAI_AGAIN` for a name). With the switch every request goes to
  the proxy, `http` in absolute form, `https` as `CONNECT`, including a
  request to `127.0.0.1:<port>`, because `NO_PROXY` is empty. In the sandbox a
  `node -e "fetch(…)"` from the agent's shell therefore found no route.
- pi's own HTTP client does not depend on the switch: pi installs an undici
  `EnvHttpProxyAgent` (`proxyTunnel: true`) as global dispatcher and global
  `fetch`. Its model call through the logging proxy was the same `CONNECT` to
  the model server with and without `NODE_USE_ENV_PROXY=1`, and the answer
  arrived both times.
- A request to the proxy's own address (`http://127.0.0.1:3128/…`, the
  fallback base URL of both adapters when no `llm.endpoint` is set) names the
  same target whether it arrives in origin form or through the proxy
  environment, so `NO_PROXY` needs no exception for it.

### Specification
1. **Briefing.** The first sentence says what works before it says the limit,
   in both languages: internet access over HTTP(S) with `curl`, `wget` and
   other clients, through the proxy in the environment, nothing else. The
   ask-mode blocks keep saying that a request may wait for a person and that
   waiting is normal. Every other statement stays (`Blocked by Humanitl.`,
   `403` handling with `note:`, `http://humanitl.internal/` with `GET /` and
   `POST /ask`, only the user adds a rule, the model rule). Each statement keeps
   its evidence in the table of the module comment of `briefing.rs`. Both
   languages stay within `TOKEN_BUDGET` (185, `tools/briefing-tokens.py`,
   worst case); the budget is not raised.
2. **Env kit.** `humanitl_proxy::ca::ENV_KIT` gets `NODE_USE_ENV_PROXY=1`, and
   `profiles/sandbox/{default,test}.toml` carry the same pair. The variable is
   on `VISIBLE_ENV` of the sandbox screen with the other proxy variables. Node
   versions before 22.21 and 24.5 ignore it. A custom profile copied before
   this issue lacks the pair, exactly as it would lack `HTTP_PROXY` if it had
   been written without it: the variables come from the profile's `[env]`
   only, and a missing one fails closed (candidate issue: inject `ENV_KIT` at
   launch instead of relying on the profile).
3. **`NO_PROXY` stays empty.** No exception for loopback or for the proxy's
   port. A request from Node to a loopback address now goes to the proxy, as
   it already does for curl, wget and pi, and the proxy decides it like any
   other target. Whether servers the agent starts inside the sandbox should be
   reachable directly (`NO_PROXY=localhost,127.0.0.1`) is a separate decision
   and a candidate issue.
4. `docs/SECURITY.md` names the switch where it describes the proxy
   environment. Guarantee 2 of the README security statement is unchanged:
   the switch only points Node at the proxy that curl already uses.

### Non-goal
- No web-search extension or tool for pi and no search engine named in the
  briefing (candidate issue: a pi extension or a documented search recipe,
  each request still through the queue).
- No change to the proxy, the rules or the hold flow.

### Tests
- `briefing.rs`: the first sentence after the heading starts with the access
  statement, names `curl` and `wget` and the proxy in the environment, in both
  languages and both ask modes, and the old opening "No network interface" /
  "Keine Netzwerkschnittstelle" is gone.
- `proxy/tests/ca.rs`: `NODE_USE_ENV_PROXY` is in `ENV_KIT` with value `1` and
  in both shipped profiles with the same value.
- `sandbox/tests/profile_parse.rs`: both profiles carry the key.
- `sandbox/tests/bwrap_args_snapshot.rs`: the snapshot shows the
  `--setenv NODE_USE_ENV_PROXY 1`.
- `ipc/tests/sandbox_rpc.rs`: the bundled profile withholds no row (the new
  variable is visible).
- Mutation proof per new assertion.

### Acceptance criteria
- [ ] The rendered briefing in English and German opens with internet access
      over HTTP(S) through the proxy, keeps every earlier statement, and stays
      within 185 tokens in the worst case.
- [ ] Every bundled sandbox profile sets `NODE_USE_ENV_PROXY=1`; Node's built-in `fetch` in the
      sandbox reaches the proxy instead of failing without a route.
- [ ] pi's own model calls work unchanged; `NO_PROXY` stays empty.
- [ ] README security statement unchanged; `docs/SECURITY.md` in sync.

### Umsetzung (HUM-304)

- **Einweisung.** Der erste Satz heißt jetzt „You have internet access:
  HTTP(S) with `curl`, `wget` etc. goes through the proxy in the environment,
  nothing else." bzw. „Du hast Internet: HTTP(S) mit `curl`, `wget` usw.
  läuft über den Proxy der Umgebung, sonst nichts." Um im Budget zu bleiben,
  entfällt „The proxy answers" / „beantwortet der Proxy" vor
  `http://humanitl.internal/`; alle übrigen Aussagen und beide Ask-Blöcke
  bleiben. Das ist ein bewusster Tausch: der Agent erfährt nicht mehr
  ausdrücklich, dass der Proxy den Meta-Endpunkt beantwortet, was aus der
  Adresse und aus „Blocked by Humanitl." folgt; dafür trägt der erste Satz
  den Zugang. Nachgezählt mit `tools/briefing-tokens.py` (`tiktoken`,
  `o200k_base`, ungünstigster Fall): englisch 170 (`ui`) und 168 (`none`),
  deutsch 184 und 179; `TOKEN_BUDGET` bleibt 185. Tabelle und Begründung im
  Modulkommentar von `briefing.rs` nachgezogen.
- **Env-Kit.** `NODE_USE_ENV_PROXY=1` in `humanitl_proxy::ca::ENV_KIT`, in
  `profiles/sandbox/{default,test}.toml`, auf `VISIBLE_ENV` und im
  argv-Schnappschuss; `docs/SECURITY.md` nennt den Schalter. `NO_PROXY` bleibt
  leer.
- **Gemessen** (Node 24.22.1, pi 1.4.0, loggender Proxy auf Loopback): Nodes
  `fetch` ohne Schalter geht direkt und scheitert, mit Schalter landet jede
  Anfrage beim Proxy. pis Modellaufruf ist mit und ohne Schalter derselbe
  `CONNECT` über den Proxy und bekommt die Antwort des Mock-LLM.
- **Tests.** `briefing.rs`
  (`the_briefing_opens_with_internet_access_over_the_proxy`),
  `proxy/tests/ca.rs` (Schlüssel und Wert `1` in `ENV_KIT` und beiden
  Profilen), `profile_parse.rs` (Schlüssel in beiden Profilen),
  Schnappschuss neu erzeugt. Mutationsbeweise: englischer oder deutscher
  erster Satz zurückgedreht, Eintrag aus `ENV_KIT` entfernt, Wert `0`,
  Zeile aus `default.toml` entfernt, Name aus `VISIBLE_ENV` entfernt; jede
  Mutation macht ihren Test rot.
- **Review.** Codex blocking (alte eigene Profile ohne den Schalter) als
  außerhalb des Umfangs bewertet: auch `HTTP_PROXY` kommt nur aus dem
  Profil, ein Fehlen schlägt geschlossen fehl; Kandidat, Wortlaut der Spec
  auf „bundled profile" korrigiert. Opus minor: `docs/SECURITY.md` nennt
  `fetch` und die globalen Agenten von `node:http`/`node:https`;
  `conf_17_node_fetch_https` setzt den Schalter nicht mehr selbst, er kommt
  aus dem Env-Kit, Versionsangabe wie in `ca.rs`.
- **Kandidaten.** `ENV_KIT` beim Start einspeisen statt nur aus dem Profil.
  Eine Websuche für pi (Erweiterung oder dokumentiertes
  Rezept, jede Anfrage weiter durch die Warteschlange);
  `NO_PROXY=localhost,127.0.0.1` für Server, die der Agent in der Sandbox
  selbst startet.

---
## HUM-311 · The Debian changelog line stays within lintian's 80 characters for every version

### Context
The tag v0.0.10 (2026-10-29) did not publish: `check-deb` failed in the
`lintian` step. The .deb of v0.0.9 passes the same lintian in the same
pinned `ubuntu:24.04` container, and nothing in the package changed except
`NODE_USE_ENV_PROXY` in the bundled profile. The cause is the changelog
written by `packaging/deb/build-deb.sh`: "  * Early release $version, built
from commit $commit. Not for production use." is 80 characters with a
five-character version and 81 with `0.0.10`, and lintian warns from 81 on
(`debian-changelog-line-too-long`); warnings fail the job.

### Specification
1. The line becomes "  * Early release $version from commit $commit, not
   for production use." (74 characters for 0.0.10, 78 for 10.100.100).
2. `build-deb.sh` fails at build time with a clear error when any changelog
   line exceeds 80 characters, so a long version cannot reach lintian.
3. The failed tag v0.0.10 stays as it is (no release was published); the next
   early release is v0.0.11.

### Acceptance criteria
1. The changelog lines are at most 80 characters for versions up to
   10.100.100 (checked by hand: 74, 73, 77, 78).
2. `check-deb` passes on the next tag.

## HUM-298 · The terminal goes full window from anywhere, by shortcut and button

### Context
Owner request 2026-10-29. The terminal of the agent lives in the upper part
of the Sandbox section, next to the tabs below it, the rail and the status
bar. Working in a full-screen TUI such as OpenCode or pi in that pane is
cramped, and getting to it from another section takes two steps. The owner
wants the terminal over the whole window from any screen, with one shortcut
(owner decision: `Ctrl+Shift+Enter`) or a button, and the same gesture
back to where he was.

### Specification
1. A core intent `ToggleTerminalFullWindowIntent`, bound in `shellShortcuts()`
   to `Ctrl+Shift+Enter` (and with the `Enter` of the number block, like
   every `Enter` binding of the app), without repeats, toggles a core provider that says where the terminal
   stands (pane or full window). Features do not import each other: the
   shell (composition root) lays the window out from the provider, the
   Sandbox section leaves its own terminal slot empty while the terminal
   stands elsewhere, and the terminal's button writes the provider.
2. Full window: the terminal of the current session fills the content of the
   window, without rail, sections, other panes or status bar. The header bar
   of the window stays (it is the title bar with the window buttons and the
   count of held requests), and so does the slim header strip of the
   terminal with the untrusted-output banner and the toggle button.
3. The shortcut works while the terminal has the keyboard focus: the
   terminal looks the chord up in `shellShortcuts()` like `Ctrl+1..6` and
   `Ctrl+K` (`docs/UX.md` 5.2). Every other key, `Esc` included, keeps going
   to the agent.
4. A button at the right end of the terminal's header strip does the same;
   its tooltip names the shortcut. It stands there in both layouts and also
   without a running session.
5. The same shortcut or the button returns to the previous section and
   layout; the sections stay built underneath, so their state survives.
   Choosing a section (`Ctrl+1..6` or the palette; the rail is hidden) or
   opening the settings also ends the full window.
6. Without a running session the full window shows the terminal's usual
   empty state.
7. The command palette lists the toggle; the shortcut tables of
   `docs/DESIGN.md` and `docs/UX.md` 5.2 name it.
8. One view of the session at a time. The emulator and its scrollback stay
   in `terminalSessionProvider`; a host that shows the terminal attaches to
   the same session. The design leaves room for a third host, a floating
   overlay (HUM-300), without recreating the session.

### Acceptance criteria
1. Widget test: from a section other than Sandbox, `Ctrl+Shift+Enter` shows the
   terminal over the window (no rail, no status bar, one terminal view) and
   the same chord returns to that section.
2. Widget test: with the keyboard focus in the terminal, `Esc` reaches the
   agent as `0x1b`, and `Ctrl+Shift+Enter` does not reach it and leaves the full
   window.
3. Widget test: the button toggles the full window and back.
4. Widget test: without a session the full window shows the empty state and
   the button.
5. Widget test: with the focus in the queue and an armed held request,
   `Ctrl+Shift+Enter` (both `Enter` keys) decides nothing and opens the full
   window; plain `Enter` at the same place still allows (counter-check).
6. Each of these behaviours has a mutation proof; `make flutter-analyze` and
   the tests of the touched packages are green; goldens changed only where
   the terminal header gained the button.

### Fallstricke
- The first proposal, `Ctrl+Shift+F`, is `AllowGroupIntent` in the queue
  (`backlog/CONVENTIONS.md` 3.9, HUM-029). The queue's `Shortcuts` sits nearer
  to the focus than the shell's, so the chord would have allowed a group of
  one held request at the first press. Owner decision 2026-10-29:
  `Ctrl+Shift+Enter`; the group allow keeps `Ctrl+Shift+F`.
- `Enter` decides in the queue. `SingleActivator` demands its modifiers
  exactly, so `Enter` (allow), `Ctrl+Enter` (block, also the note field and
  the editor's send) and `Shift+Enter` (remember grid) never match
  `Ctrl+Shift+Enter`; the palette and the history bind plain `Enter` only.
  The decision-key lock of the queue reads `Enter` too: right after a
  decision by `Enter`, a chord pressed before `Enter` came up is refused
  like any held decision key.

### Umsetzung (HUM-298)

- **Wo das Terminal steht.** `core/ui/terminal_placement.dart`:
  `TerminalPlacement { pane, fullWindow }` und `terminalPlacementProvider`
  (`toggleFullWindow`, `leaveFullWindow`). HUM-300 bekommt hier einen dritten
  Wert und in der Shell einen dritten Wirt.
- **Ein Widget für jeden Wirt.** `features/sandbox/widgets/sandbox_terminal.dart`
  (`SandboxTerminal`) liest die laufende Sitzung aus `sandboxStatusProvider`
  und baut `TerminalPane`. Die Sandbox zeigt es im Pane, solange die
  Platzierung `pane` ist, sonst eine leere Fläche in Terminalfarbe; die Shell
  zeigt es über dem ganzen Fenster (`catchUp` fragt den Status einmal nach,
  `focusOnMount` gibt dem Agenten die Tastatur). Sitzung und Rücklauf
  überstehen den Wechsel, weil sie im `keepAlive`-Provider leben und
  `attach` idempotent ist; verloren geht nur Auswahl und Scrollposition der
  Ansicht.
- **Shell.** `_withTerminalHost`: ein `IndexedStack` zwischen dem gewohnten
  Layout (Leiste, Abschnitte, Blatt) und dem Terminal. Das Layout bleibt
  gebaut, verliert den Fokus, meldet über `Visibility.of` „nicht sichtbar"
  (die Entscheidungstasten der Queue sind damit außer Dienst) und hält seine
  Ticker an. Die Statusleiste ist versteckt, nicht entfernt. Die Kopfleiste
  bleibt: Sie ist die Titelleiste des rahmenlosen Fensters (HUM-255) mit
  Fensterknöpfen und Halte-Zähler. Das ganze Fenster endet mit
  `ToggleTerminalFullWindowIntent`, mit jedem `NavIntent` (auch auf den
  Abschnitt, auf dem man steht), mit jeder Änderung von `navigationProvider`
  (Palette, Übergaben, Setup) und beim Öffnen der Einstellungen. Bei jedem
  Platzwechsel nimmt die Shell den Fokus, das Terminal holt ihn sich ein
  Frame später (`TerminalPane.focusOnMount`); die Sandbox gibt ihn dem Pane
  zurück, wenn es aus dem ganzen Fenster zurückkommt.
- **Taste und Knopf.** `terminalFullWindowActivator` (`Ctrl+Shift+Enter`)
  und `terminalFullWindowNumpadActivator` in `core/shortcuts/intents.dart`,
  ohne Wiederholungen, in `shellShortcuts()`; `TerminalPane._onKey`
  reicht ihn wie die anderen Kürzel der Shell an `Actions` weiter. Der Knopf
  (`FullWindowButton`, `sandbox-terminal-full-window`) steht rechts im
  Hinweisstreifen über dem Terminal, auch im Leerzustand, `HButton` ghost
  `sm` mit `HoverLabel` unten; Glyphen `ShellGlyph.maximize`/`minimize`
  (Lucide `maximize-2`/`minimize-2`). Der Streifen wird dadurch so hoch wie
  der Knopf.
- **Palette und Doku.** Befehl `toggle-terminal-full-window` am Ende der
  Liste (davor verdrängte er `queue-allow-all` aus dem sichtbaren Bereich).
  `docs/DESIGN.md` Kürzeltabelle, `docs/UX.md` 5.2.
- **Tests.** `test/features/shell/terminal_full_window_test.dart` (neun, davon drei aus dem Review; sechs
  Tests: Kürzel aus History hin, mit Ziffernblock-`Enter` zurück; Fokus im
  Terminal, `Esc` als `0x1b`, Kürzel erreicht den Agenten nicht; Knopf hin
  und zurück plus `Ctrl+4` auf dem eigenen Abschnitt; Leerzustand aus
  Intercept mit Knopf; Palette hin, „Go to History" zurück; Queue mit
  gehaltener, scharfer Anfrage im Fokus: beide Varianten entscheiden nichts
  und öffnen das ganze Fenster, `Enter` danach gibt frei), zwei Zeilen in
  `shortcuts_map_to_intents_test.dart`. Goldens neu: `sandbox_header_*`
  (6) und `isolation_panel_*` (10), nur der Knopf im Terminalstreifen.
- **Mutationsbeweise**, je rot und danach Byte für Byte zurück: Bindung aus
  `shellShortcuts()` entfernt; Terminal verschluckt das Kürzel; kein Fokus
  beim Einhängen (`Esc` kommt nicht an); Knopf ohne Wirkung; Leerzustand ohne
  Kopfstreifen; Shell ohne Action; ohne Navigations-Listener; `NavIntent`
  ohne Rückkehr; Pane behält sein Terminal (zwei Ansichten); `IndexedStack`
  bleibt auf dem Layout; die Queue sperrt auf `Ctrl+Shift+Enter`;
  Ziffernblock-Bindung entfernt; `_onKey` überspringt das Kürzel (der
  Emulator übersetzt `Ctrl+Shift+Enter` sonst in Bytes für den Agenten).
- **Entschieden.** Der erste Entwurf band `Ctrl+Shift+F` und kollidierte
  mit der Gruppenfreigabe der Queue; der Eigentümer hat am 2026-10-29
  `Ctrl+Shift+Enter` gewählt (siehe Fallstricke). Geprüft gegen jede
  `SingleActivator` in `app/lib` und CONVENTIONS 3.9: keine Kollision.
- **Restrisiko, bekannt.** Ein halb gegriffenes Kürzel ist eine andere
  Taste: Kommt `Shift` erst nach `Ctrl+Enter`, sperrt die Queue die
  ausgewählte Anfrage (umkehrbar, der Agent darf es erneut versuchen);
  kommen beide Modifikatoren zu spät, ist der Druck ein `Enter` und gibt
  frei, hinter der Scharfschaltung von `docs/UX.md` 5.4. Dieselbe Klasse wie
  jedes `Ctrl`-Kürzel auf einer Entscheidungstaste (`Ctrl+F`, `Ctrl+L`,
  `Ctrl+Shift+F`); kein eigener Schutz in diesem Issue.
- **Review (Codex, Sonnet).** „Go to <Abschnitt>" in der Palette und die
  Übergaben aus History und Befunden gehen über `_go`, das das ganze Fenster
  ausdrücklich beendet: Auf den Abschnitt darunter ändert sich die
  Navigation nicht, und der Listener hört nichts. Die Palette gibt die
  Tasten beim Schließen über dem ganzen Fenster an das Terminal zurück: Der
  Wirt steht in einem eigenen `FocusScope` (`_terminalScope`), der sich den
  fokussierten Knoten merkt. Startet die Sitzung erst, nachdem das ganze
  Fenster offen ist, holt `TerminalPane.didUpdateWidget` den Fokus nach.
  `_terminalCameBack` gilt nur für den einen Build. Tests:
  `going_to_the_section_underneath_ends_the_whole_window`,
  `after_the_palette_the_keys_go_back_to_the_terminal`,
  `a_session_that_starts_later_gets_the_keys`. Mutationen: `_go` ohne
  Rückkehr rot; Palette gibt den Fokus der Shell rot; das Nachholen in
  `didUpdateWidget` allein ist grün, weil zwei Wege es schon tragen (eine
  Fokusanfrage an einen noch nicht eingehängten Knoten wirkt beim Einhängen,
  `autofocus` im neuen Scope); alle drei Wege entfernt rot, nur das
  Nachholen übrig grün.

---
## HUM-308 · Agents do not ask twice and do not give up while a request waits
Sprint: 5 · Size: S · Dependencies: HUM-274, HUM-304

### Context
Owner report of 2026-10-29, two problems:

1. **Asked twice.** OpenCode asks "allow webfetch?" in its own terminal before
   it fetches a page, and then Humanitl holds the same request in Intercept.
   The bundled `agents/opencode/opencode.json.tmpl` sets
   `"webfetch": "ask"`, also in `OPENCODE_PERMISSION`. The owner wants the
   agents to run without their own prompts inside the sandbox; Humanitl is the
   gate.
2. **Gives up while waiting.** An agent tool such as OpenCode's `webfetch`
   times out while the request still waits for the owner in Intercept; the
   agent then reports a failure and stops. When a client gives up, the proxy
   ends the held flow as `ClientTimeout` (`HoldQueue::hold`, ticket dropped),
   and the card leaves the queue before anybody decided.

The hold lasts `hold.timeout_secs` (default 300) with `hold.ask_mode` `ui` or
`terminal`; with `none` a request without a rule fails at once and nothing
waits.

### Specification
1. **OpenCode permissions.** The template's `permission` block sets
   `webfetch` and `bash` to `allow`; `websearch` and `external_directory`
   stay `deny`. `OPENCODE_PERMISSION` carries the same block (it is derived
   from the template). No entry is `ask`. Check the schema against the
   installed OpenCode, not the documentation.
2. **`edit`.** OpenCode already allows edits by default. Write `edit`
   explicitly only if it does not change the behaviour of OpenCode's built-in
   agents; otherwise leave it out and say why.
3. **pi.** Find out from the installed package (1.4.0, read-only, never
   `~/.pi`) whether pi has permission prompts. If it has, the bundled
   `settings.json` switches them off; if not, the template stays and
   `agents/pi/README.md` says so.
4. **Timeouts.** Find each agent's own timeout for the tools that reach the
   network (OpenCode `webfetch` and `bash`, pi `bash`). Where a configuration
   switch exists, the adapter raises the tool's default timeout to at least
   `hold.timeout_secs` in the ask modes `ui` and `terminal`, and leaves the
   agent's default with `none`. Where the cap is fixed below the hold, the
   briefing says in one short sentence what happens and what to do instead;
   the sentence must be true for every adapter that shares the briefing, and
   both languages stay within `TOKEN_BUDGET` (185, worst case).
5. **Documentation.** `docs/SECURITY.md` explains why the agent's own prompts
   protect nothing inside the sandbox, `docs/THREAT-MODEL.md` stays
   consistent, and the guides that describe the permission block and the
   environment follow. The README security statement is unchanged.

### Non-goal
- No change to the proxy, the hold or `hold.timeout_secs`.
- No countdown of the agent's own tool timeout in Intercept (candidate).
- OpenCode's remaining own questions (`read` of `*.env`, `doom_loop` after
  three identical calls) stay at OpenCode's default; neither is a network
  decision.

### Tests
- `sandbox/tests/opencode_adapter.rs`: the rendered `opencode.json` and
  `OPENCODE_PERMISSION` allow `webfetch` and `bash`, carry no `ask`, keep
  `websearch` and `external_directory` at `deny`; the bash timeout variable
  follows `hold.timeout_secs` in `ui` and `terminal`, has a floor and a
  ceiling, and is absent with `none`.
- `pi_config.rs`: the rendered `settings.json` sets nothing beyond provider,
  model and telemetry.
- `briefing.rs`: the sentence about a tool that gives up sooner and `curl`
  stands in the ask block of both languages and not in the `none` block.
- Mutation proof per new assertion, restored byte-identical.

### Acceptance criteria
- [ ] OpenCode in the sandbox fetches and runs commands without asking in its
      own interface; the request is held once, in Intercept.
- [ ] A `curl` in OpenCode's `bash` waits at least `hold.timeout_secs` by
      default; pi's `bash` has no default timeout to raise.
- [ ] The briefing tells the agent that a tool with a shorter limit
      withdraws the question and that `curl` waits, in both languages, within
      185 tokens.
- [ ] README security statement unchanged; `docs/SECURITY.md` and
      `docs/THREAT-MODEL.md` say why agent-side prompts are no boundary.

### Umsetzung (HUM-308)

- **Gemessen, OpenCode.** Installiert ist 1.18.31 (`~/.opencode/bin/opencode`,
  Konstante `"1.18.31"` in den Zeichenketten des Binärprogramms; ausgeführt
  wurde es nicht, damit nichts nach `~/.local/share/opencode` schreibt).
  Schema des Blocks `permission`: `read`, `edit`, `glob`, `grep`, `list`,
  `bash`, `task`, `external_directory`, `todowrite`, `question`, `webfetch`,
  `websearch`, `lsp`, `doom_loop`, `skill`, Werte `allow`/`ask`/`deny`.
  Vorgabe aller Agenten: `"*": "allow"`, `doom_loop: "ask"`,
  `external_directory: "ask"` (außer Arbeits- und Temp-Verzeichnis),
  `read` von `*.env`/`*.env.*`: `ask`. Die Rechte aus der Konfiguration
  werden als Letztes über die eines jeden eingebauten Agenten gelegt
  (`merge(defaults, agent, user)`); der Plan-Agent hat
  `edit: {"*": "deny", <Plan-Dateien>: "allow"}`.
- **Gemessen, Fristen OpenCode.** `webfetch`: Vorgabe 30 s, höchstens 120 s
  (`timeout` des Modells in Sekunden, `Math.min(..., 120000)`), fest im Code,
  keine Einstellung. Vor der Frist steht die eigene Rückfrage
  (`ask({permission: "webfetch"})`), dann der Abruf mit
  `timeoutOrElse(... "Request timed out")`. `bash`: Vorgabe 120 s aus
  `OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS` (positive ganze Zahl, sonst
  verworfen), je Aufruf überschreibbar, keine Obergrenze außer der des
  Zeitgebers.
- **Gemessen, pi 1.4.0** (`/usr/local/lib/node_modules/@earendil-works/pi-coding-agent`).
  Keine Rückfragen vor Werkzeugaufrufen (`docs/security.md`: „it does not ask
  for approval before every tool call"; Werkzeuge `read`, `write`, `edit`,
  `bash`, `grep`, `find`, `ls` ohne Freigabeschritt; Freigaben gibt es nur
  über Erweiterungen). `bash`: `timeout` optional, „no default timeout",
  höchstens 2 147 483 s. Kein Web-Werkzeug. Die Vorlage bleibt; die
  Vertrauensfrage zu Projekt-Erweiterungen (`defaultProjectTrust`) bleibt
  beim Menschen.
- **Gemessen, Humanitl.** `hold.timeout_secs` Vorgabe 300, gilt mit
  `ask_mode` `ui` und `terminal`; `none` setzt die Frist auf null. Gibt der
  Client auf, fällt in `HoldQueue::hold` das Ticket, und der Flow endet als
  `Block { ClientTimeout }` (`408`); die Karte verlässt die Warteschlange.
  Ein abgelaufener `webfetch` lässt also keine Frage zurück, die der Mensch
  noch beantworten könnte; ein Satz „nach der Entscheidung einfach
  wiederholen" wäre falsch gewesen.
- **Vorlage.** `"webfetch": "allow"`, neu `"bash": "allow"`; `websearch` und
  `external_directory` bleiben `deny`. `edit` bewusst nicht: ein
  ausdrückliches `allow` stünde über dem `edit: deny` des Plan-Agenten und
  machte den Plan-Modus schreibend; die Vorgabe `"*": "allow"` erlaubt
  `edit` im Build-Agenten ohnehin ohne Rückfrage.
- **Frist `bash`.** `opencode::bash_default_timeout_ms`: mit Frage
  `(hold.timeout_secs + 60) * 1000`, mindestens 120 000, höchstens
  2 147 483 547 (JavaScript-Zeitgeber minus die 100 ms, die OpenCode
  aufschlägt); mit `none` keine Variable. Vorgabe ergibt `360000`. Die
  Variable steht auf `VISIBLE_ENV`.
- **Einweisung.** Block `ui`: „Waiting is normal: do not abort; a tool that
  gives up sooner withdraws the question, `curl` waits." bzw. „Warten ist
  normal: nicht abbrechen; gibt ein Tool früher auf, ist die Frage weg, `curl`
  wartet." Wahr für beide Adapter: `curl` hat keine eigene Gesamtfrist, pis
  `bash` keine Vorgabefrist, OpenCodes `bash` jetzt mindestens die Halte-Frist.
  „or retry"/„nicht wiederholen" im Block entfällt, weil nach einem Abbruch nur
  ein neuer Abruf neu fragt; das `403` bleibt „nicht wiederholen". Für das
  Budget entfallen „in a body", im Deutschen „im Rumpf", „die" vor „Regeln",
  „läuft", „auf diesem Host", „das meiste" wird „fast alles", und „eine
  etwaige `note:`-Zeile" wird „etwaige `note:`-Zeilen" (eine `403` kann ohne
  `note:` kommen; Review Codex). Nachgezählt mit
  `tools/briefing-tokens.py` (`o200k_base`, ungünstigster Fall): englisch
  181 (`ui`) und 165 (`none`), deutsch 185 und 169. `TOKEN_BUDGET` bleibt 185;
  der deutsche Block `ui` steht damit genau an der Grenze.
- **Doku.** `docs/SECURITY.md` neuer Abschnitt „The agent's own permission
  prompts" und Changelog-Zeile, `docs/THREAT-MODEL.md` B1 und
  Änderungshistorie, `docs/guides/agent-profiles.md` (Block, Tabelle, Fristen,
  Umgebung), `docs/guides/first-session.md`, `agents/opencode/README.md`,
  `agents/pi/README.md`, Kommentar in `tests/e2e/m3_agent_inside/run.sh`.
  Die Sicherheitsaussage im README ist unverändert.
- **Tests.** `opencode_adapter.rs`:
  `opencode_does_not_ask_before_fetching_or_running_a_command`,
  `opencode_bash_waits_at_least_as_long_as_the_hold`, dazu
  `opencode_config_is_valid_json_and_points_to_llm` auf `allow` nachgezogen.
  `briefing.rs`: `a_tool_that_gives_up_sooner_is_named_together_with_curl`.
  `pi_config.rs`:
  `the_settings_switch_nothing_beyond_provider_model_and_telemetry`.
  `ipc`: der Name im Test der Erlaubnisliste.
- **Mutationsbeweise**, je Mutation rot und danach mit `cmp` byte-gleich
  zurückgestellt: `webfetch` wieder `ask`; `bash` entfernt; `edit: allow`
  ergänzt; Reserve 0; ohne Untergrenze; ohne Obergrenze; `none` setzt die
  Variable; Variable nicht in die Umgebung gekettet; englischer oder deutscher
  Satz entfernt; Satz im Block `none`; `defaultProjectTrust` in der
  pi-Vorlage.
- **Kandidaten.** In Intercept anzeigen, wie lange das Werkzeug des Agenten
  noch wartet (die Frist kennt nur der Agent; für `webfetch` 30 oder 120 s).
  OpenCodes `doom_loop`-Frage nach drei gleichen Aufrufen, die ein
  wiederholter Abruf auslösen kann. Mehr Luft im deutschen Briefing.

---
## HUM-299 · A second shell in a running sandbox

Owner request 2026-10-29. Daemon, shim and command line only; the tab
"+ Shell" in the app's terminal pane is a follow-up issue, because the app's
terminal files are being changed by HUM-298, HUM-300 and HUM-303.

### Context
Like `docker exec`, the owner wants to open a shell inside a sandbox in which
the agent (pi, OpenCode) is already running, without stopping the agent and
without sharing its terminal. Today the only way into a running session is
`humanitl sandbox attach`, which is the agent's own terminal, and
`humanitl run -- bash` (HUM-280), which is a new session.

Entering the sandbox from outside (`nsenter`, `setns`) is ruled out: the
process would start without the shim's filter, without `PR_SET_NO_NEW_PRIVS`
and with the daemon's descriptors, and each of these would have to be
rebuilt by hand outside the shim. The shim is PID 1 of the sandbox (HUM-203)
and already carries all of it, so a child it forks inherits every
restriction and sees the same mounts, proxy environment, rules and project
folder.

### Specification
1. **The door.** For a start at the daemon's own pseudo-terminal
   (`StdioMode::Pty`), and only then, the launcher creates a
   `SOCK_SEQPACKET` pair. bwrap inherits one end, which reaches the shim as
   `--control-fd <n>`, rendered after `--proxy-port` and
   `--controlling-terminal`; the daemon keeps the other with `CLOEXEC`. This
   is the least invasive channel: the shim has no other way to hear from the
   daemon (the report descriptor and the status pipe point outward), a named
   socket would be a second socket in the sandbox's filesystem
   (`single_socket`), and a pipe cannot carry the shell's terminal.
2. **Who holds it.** Inside the sandbox only PID 1: the agent's child closes
   it before its filter, it carries `CLOEXEC`, every further shell closes
   all descriptors above 2, and PID 1 stays non-dumpable. On the host only
   the daemon, and only the RPC `Exec` uses it. Nothing inside the sandbox
   can trigger a spawn.
3. **The request.** `S`, a four-byte id chosen by the daemon (never 0) and
   the command, each argument NUL-terminated, at most 16 KiB and 256
   arguments, with exactly one descriptor as `SCM_RIGHTS`: the follower side
   of a pseudo-terminal the daemon opened for this shell. Anything else is
   refused (`F`). Answers are nine bytes: `R` id pid, `F` id code, `E` id
   status. The daemon reads answers without a control buffer, so a
   descriptor sent its way is closed by the kernel.
4. **The shell.** A thread of PID 1 serves the door. Per request it forks a
   child that dies with that thread (`PR_SET_PDEATHSIG`), calls `setsid` and
   `TIOCSCTTY` (argument 0) on the follower, makes it descriptors 0 to 2,
   closes everything above, resets the signal mask, installs the agent's
   filter in the variant that answers `EPERM` itself, proves it
   (`socket(AF_UNIX, SOCK_STREAM)` must answer `EPERM`, otherwise exit 126),
   resets the signal dispositions and `execvpe`s the command with the
   shim's environment minus its own variables. Everything the child needs
   exists before the fork; the child allocates nothing.
5. **Limits and end.** At most four further shells live at a time; the shim
   and the daemon both count, the fifth is `TERM_002`. A shell ends when its
   process exits (its exit code reaches its terminal) and with the sandbox:
   PID 1 kills every shell after the agent's end, the PID namespace ends
   with PID 1, and the daemon ends the shell's terminal with `128 + 9` when
   its end of the pair closes without an `E`.
6. **Protocol.** RPC `Exec(ExecRequest) returns (ExecResponse)`:
   `sandbox_id` (empty: the running session), `command` (empty: an
   interactive `bash`, else `sh`), `cols`, `rows`; the answer carries
   `shell_id`, the command started and `sandbox_id`.
   `TerminalInput.Open.shell_id` attaches to that shell's terminal with the
   rules of the agent's (one writer, any number of readers, `TERM_001`, a
   filtered ring buffer, notice lines); an id under which no shell lives is
   `IPC_006`. `Info.capabilities` names `sandbox.exec`. `PROTO_MINOR` 23,
   `docs/PROTOCOL.md`.
7. **Command line.** `humanitl sandbox exec [--detach-keys SPEC] [-- CMD]`
   asks for the shell, prints `Shell <id> in sandbox <id>` and attaches like
   `humanitl sandbox attach` (raw mode, size, `Ctrl+A d` from HUM-279);
   without a terminal on `stdin` it only watches until the shell ends.
   Detaching says `Attach again: humanitl sandbox attach --shell <id>`; that
   flag is new on `sandbox attach`. The command ends with the shell's exit
   code.
8. **Follow-up.** The app tab "+ Shell" in the terminal pane, reading
   `sandbox.exec`, `Exec` and `Open.shell_id`, is its own issue.

### Security
The pair is a new way into the sandbox, so `docs/SECURITY.md` (section 4,
"Further shells in a running sandbox", changelog) and
`docs/THREAT-MODEL.md` (K-18, change history) describe it in the same
change. The statement in `README.md` holds: the shell has no network
interface (same network namespace), sees exactly one door (the pair is
unnamed and held only by PID 1), and carries a filter that is at least the
agent's (PID 1's plus the agent's, proven by a refused `AF_UNIX`). Refused
`socket()` calls of a further shell are refused but not counted (the filters
refuse a second listener); that is declared, not hidden.

### Acceptance criteria
1. With a running session, `humanitl sandbox exec` opens a shell next to the
   agent; the agent keeps running and keeps its terminal.
2. Inside the shell the escape checks hold as for the agent: same network,
   mount, PID and user namespaces, `Seccomp: 2`, `NoNewPrivs: 1`,
   `socket(AF_UNIX)` refused with `EPERM`, nothing above descriptor 2, PID
   1's descriptors closed.
3. Stopping the sandbox ends every further shell; no process outlives it.
4. A fifth simultaneous shell is refused with `TERM_002`; once one ends, its
   place is free again.
5. Nothing inside the sandbox holds the door or can request a spawn.

### Pitfalls
- The forking thread is not the main thread: `PR_SET_PDEATHSIG` fires when
  that thread ends, which is wanted (the daemon let go of the door).
- The parent has threads: no allocation and no `std::io::stderr` lock in the
  child, and the reaper must find a shell that ended at once, so the live
  list stays locked across `fork` and the `R` answer.
- The reaper is `waitpid(-1)` on the main thread; a shell's status goes to
  the daemon as `E`, an orphan's is dropped as before.
- A background job can keep the shell's terminal open after the shell
  exited: the daemon stops reading a short drain after the exit and closes
  the master.
- Fake shims in tests must accept `--control-fd <n>`.

### Umsetzung (HUM-299)

- **Tür.** `crate::shell::pair()` (`daemon/crates/sandbox/src/shell.rs`) legt
  das `SOCK_SEQPACKET`-Paar nur an, wenn `BwrapBackend` am eigenen
  Pseudoterminal startet (`BwrapBackend::shell_door`). Die Seite des Shims
  liegt über den Standardströmen, wird vererbt und als `--control-fd <n>`
  gerendert (`LaunchInputs::control_fd`, `SHIM_CONTROL_FD`); die Vorschau
  und `humanitl sandbox run` haben keine. Die Seite des Daemons reist in
  `LaunchOnce::shells` zu `launch`, dort öffnet `ShellDoor::start` einen
  Leser der Antworten; eine Tür, die nicht aufgeht, kostet nur die weiteren
  Shells.
- **Shim.** Neue Datei `bin/humanitl-shim/src/exec.rs`. `take_control`
  prüft die Nummer (offener Socket, über 2) und setzt `CLOEXEC`; das Kind des
  Agenten schließt sie ausdrücklich (`drop(control)`) und behält sie in
  `close_inherited` nicht. Der Faden `shells` startet nach dem eigenen
  Filter und der Prüfung auf nicht dumpbar, direkt vor dem Tor. Je Anfrage
  forkt er unter dem Schloss der Liste lebender Shells (der Ernter in
  `reap(child, shells)` meldet `E` erst nach dem `R`), das Kind tut nur
  async-signal-sichere Schritte (`setsid`, `TIOCSCTTY` mit 0, `dup2`,
  `close_range`, Signalmaske, Filter des Agenten in der stillen Fassung,
  Probe `socket(AF_UNIX)` muss `EPERM` sein, Dispositionen, `execvpe` mit
  der Umgebung ohne `SHIM_VARS`). Nach dem Ende des Agenten tötet
  `kill_all` alle Shells. Grenze `MAX_SHELLS = 4` im Shim und im Daemon.
- **Daemon.** `SandboxHandle::open_shell` gibt einen `Shell` (eigenes
  Pseudoterminal, `write_input`, `resize`, `pump`). `pump` liest mit `poll`,
  nach dem `E` noch 300 ms, und liefert den Exit-Code, ohne `E` bei
  geschlossener Tür `ENDED_WITH_SANDBOX` (137). In `humanitl-ipc` trägt
  `TerminalHub` jetzt ein `Target` (Agent oder Shell), `TerminalHub::for_shell`
  ist dasselbe Terminal mit Filter, Ring, einem Schreiber und Hinweiszeilen.
  Neues Modul `ipc/src/exec.rs` (`ShellRegistry`, `open`, `Opened::run`);
  `SandboxService::exec` und `terminal_for` sind dünn, `Running::shells`
  hält die Ablage. `TERM_001` für eine Shell nennt
  `humanitl sandbox attach --shell <id> --read-only`.
- **Vertrag.** `rpc Exec(ExecRequest) returns (ExecResponse)`,
  `TerminalInput.Open.shell_id = 5`, `Info.capabilities` mit `sandbox.exec`,
  `PROTO_MINOR` 23, `docs/PROTOCOL.md`, `proto/descriptor.binpb` und
  `proto/generated.sha256` neu erzeugt (`scripts/gen-proto.sh`, protoc 27.5,
  protoc_plugin 25.0.0). `DaemonApi::exec` hat eine Vorgabe mit `IPC_006`;
  `humanitld --fake` öffnet keine Shell. `app/lib/core/ipc/proto_version.dart`
  bleibt bei 22 (darf nachziehen, `docs/PROTOCOL.md` 5).
- **Befunde.** Kein neuer Code: Grenze, beendete Sandbox, Ablehnung des
  Shims und fehlende Tür sind `TERM_002` mit eigenem Satz, keine Sitzung oder
  eine unbekannte Kennung `IPC_006`. Ein eigener Code (`TERM_003`) bräuchte
  ARB-Einträge in `app/` und gehört deshalb zur Folge-Issue der App.
- **Kommandozeile.** `humanitl sandbox exec [--detach-keys] [-- CMD]`
  (`bin/humanitl/src/cmd/exec.rs`) und `humanitl sandbox attach --shell ID`;
  `attach::attach_to` und `finish_with` tragen die Kennung und die eigene
  Zeile nach dem Abhängen. Ohne Terminal auf `stdin` nur lesend. Ohne
  Kommando `/bin/sh -c 'if command -v bash …; then exec bash -i; fi; exec sh -i'`.
  `parity.rs`, `docs/reference/cli.md` und `docs/reference/parity.md`
  (`cargo xtask docs`), `docs/cli.md` („A second shell next to the agent").
- **Sicherheit.** `docs/SECURITY.md` Abschnitt 4 „Further shells in a running
  sandbox" und Changelog, `docs/THREAT-MODEL.md` K-18, Tabelle und
  Änderungsverlauf. Gezählt werden verweigerte `socket()`-Aufrufe einer
  weiteren Shell nicht (zweiter Listener verboten); das steht dort.
- **Tests.** Echtes `bwrap`, echter Shim (`daemon/crates/ipc/tests/terminal.rs`):
  `a_further_shell_runs_next_to_the_agent_with_the_agents_limits` (Agent und
  Shell drucken denselben Bericht: gleiche Namensräume net, mnt, pid, user,
  ipc, uts, `Seccomp 2`, `NoNewPrivs 1`, `Seccomp_filters` der Shell ≥ dem
  des Agenten, `AF_UNIX` `EPERM` per `python3`, gleicher Proxy, `PATH` und
  `/work`, keine Shim-Variablen, beide nur Deskriptoren 0 bis 2, `/proc/1/fd`
  zu, eigene Größe 101x33, der Agent läuft und liest weiter),
  `a_further_shell_dies_with_the_sandbox` (Stopp, `Exit 137` am Terminal der
  Shell, `sleep 2990299` nicht mehr auf dem Host, Kennung weg),
  `the_fifth_further_shell_is_refused_until_one_ends` (vier offen, fünfte
  `TERM_002`, Exit-Code 5 kommt an, danach geht eine neue auf),
  `exec_without_a_session_says_so`. Einheiten: `shell::tests` (Fake-Shim am
  Paar: Kommando und Exit-Code, Grenze vier auch für verwaiste Shells,
  Ablehnung, fremde Kennungen, Ende mit der Sandbox, schlechte Kommandos),
  `exec::tests` im Shim (Format, jede Abweichung abgelehnt), `parse_cli` mit
  `--control-fd`, `ipc::exec::tests`, `cmd::exec::tests`. Kommandozeile:
  `tty::sandbox_exec_opens_a_shell_and_attaches_to_its_terminal` (Fake: `Exec`
  vor `Open` mit `shell_id 1`, `Ctrl+A d`, Zeile mit `--shell 1`, erneutes
  `attach --shell 1`). `proto_contract` kennt `Exec`; der Ersatz-Shim in
  `sandbox/tests/pty.rs` nimmt `--control-fd`.
- **Mutationsbeweise**, alle Dateien danach mit `cmp` byte-gleich zurück:
  Filter des Agenten und Probe in der Shell entfernt:
  `…with_the_agents_limits` rot (`SHELL-UNIX=ok`, `Seccomp_filters=1`).
  `hub.finish` in `Opened::run` entfernt: `…dies_with_the_sandbox` und
  `the_fifth…` rot. `MAX_SHELLS` 5 in Shim und Daemon: `the_fifth…` rot
  (fünfte Shell geht auf), nur im Daemon zusätzlich
  `the_door_counts_to_four_and_no_further` rot. Tür beim Agenten gelassen
  (kein `drop(control)`, in der Behalteliste, `CLOEXEC` weg; drei Schichten,
  eine allein zeigt nichts, weil die anderen halten):
  `…with_the_agents_limits` rot (`AGENT-FD` mit 26). Prüfung „genau ein
  Deskriptor" im Shim auf „mindestens einer": `anything_else_is_refused…`
  rot.
- **Offen.** App-Reiter „+ Shell" (eigene Issue, liest `sandbox.exec`,
  `Exec`, `Open.shell_id`); Liste offener Shells für `attach` ohne Kennung
  (heute nennt die Zeile nach dem Abhängen die Kennung, `IPC_006` die offenen);
  eigener Befundcode für die Grenze samt ARB.
- **Review Runde 1** (Codex, Opus; keine blocking/major, alle minor übernommen):
  - Ein schnelles Kommando (`sandbox exec -- true`) endete, bevor sich die
    Kommandozeile anhing, und der Exit-Code ging verloren. Jetzt bleibt das
    Terminal einer beendeten Shell samt Rückstand und Exit-Code für genau
    einen ersten Client stehen, höchstens `FINISHED_RETENTION` (30 s)
    (`ShellRegistry::retire`, `get` gibt es einmal heraus). Test
    `a_fast_shell_leaves_output_and_exit_for_the_first_client`; Mutation
    (Frist 0) rot.
  - Die Nummern des Shims beginnen je Sandbox bei 1; ein altes
    `attach --shell 1` hätte die Shell einer neuen Sitzung getroffen. Die
    Kennung nach außen ist jetzt `<sitzung>-<n>` (acht Hex-Zeichen aus der
    zufälligen Hälfte der Sandbox-Kennung, `ShellRegistry::external_id`);
    geprüft wird beim Anhängen gegen die Ablage der laufenden Sitzung. Tests
    `a_shell_id_holds_only_in_its_own_session`,
    `exec::tests::an_external_id_carries_its_sandbox`; Mutation (nur die
    Nummer) beide rot.
  - Restrisiko dokumentiert (`docs/SECURITY.md` Abschnitt 4, K-18): Die
    Shell läuft mit der UID des Agenten im selben PID-Namensraum ohne
    `hidepid`; der Agent kann lesen, was dort getippt wird, und in ihr
    Terminal schreiben. Keine Geheimnisse dort tippen. Die Vorgabe-Shell
    startet ohne Startdateien (`bash --norc --noprofile -i`, `sh -i` ohne
    `ENV`), ein vom Agenten angelegtes `~/.bashrc` läuft nicht.
  - „session token" heißt jetzt „the daemon's token".
  - Shim: Alle empfangenen Deskriptoren eines Kontrollkopfes werden bis zu
    seinem Ende übernommen und bis auf den einen erwarteten geschlossen
    (keine Obergrenze 16 mehr); eine Nachricht über `MAX_REQUEST` gilt als
    fehlerhaft, auch wenn sie in das eine Ersatzbyte passte.
  - Kandidaten für eigene Issues: weitere Shells vom Agenten trennen
    (Unter-Nutzernamensraum mit anderer UID, oder `hidepid`/`subset=pid`);
    das interne Paar in Garantie 2 der `README.md` erwähnen.

---
## HUM-307 · The Intercept action bar keeps Allow and Block on one line

### Context
Owner report of 2026-10-29: in the Intercept tab "Block" sits lower than
"Allow", as if wrapped into a second row. It is a second row: below an
inspector width of `actionBarWrapWidth` (800 px, times the text scale) the bar
switched to a column with Block on its own line. The threshold was measured
with the square glyphs of the test font, which are about twice as wide as
Inter; with real fonts the controls fit long before that, yet the inspector
reaches 800 px only in windows wider than about 1920 px, and with the rail
expanded (HUM-254) never at 1280 px.

At the same time `action_bar_de_test` "the German action bar keeps every
label on one line" failed locally ("Angehalten: keine Regel · Vorgabe:
fragen" cut short) and passed in CI. The test built the app without its own
UI state file and read `~/.local/state/humanitl/ui-state.json` of whoever
ran it; with the rail expanded there, the inspector shrank to its minimum of
480 px.

### Specification
1. Allow and Block share one line at every inspector width from the minimum
   (480 px) up, at text scale 1.0 and 1.3, in English and German. The line
   measures what it carries and gives way in a fixed order: the distance
   between the decisions shrinks from 160 px to a minimum, then
   "Edit + Allow" leaves the line (`E` still opens the editor; the hidden
   control is neither painted, hit, read out nor focusable), then the valve
   shortens its label down to `HSize.hitDecision.width`. Only when even that
   does not fit (twice the text scale in the narrowest pane) Block moves
   under Allow, as `docs/UX.md` 6 allows for scaled text. Block and the note
   slot keep their size; the hold reason below keeps its ellipsis.
2. Allow and Block stand at the same height, vertically centred on one line.
3. No widget test reads the UI state file of the person running it.
4. Layout tests that judge what fits measure with a proportional font that
   is the same in CI and locally.

### Acceptance criteria
1. A widget test asserts the same vertical centre and label baseline for
   Allow and Block in both locales, at 1.0 and 1.3, in the narrowest pane,
   the default window and a wide window, with and without findings.
2. A test of the layout steps pins every boundary to the pixel.
3. A guard test fails when a test file builds `HumanitlApp()` without its
   own UI state file.
4. Only the goldens the layout change alters are regenerated, and they are
   named in the commit.

### Umsetzung (HUM-307)

- **Ursache des lokalen Rots.** Sieben Testdateien (`action_bar_de_test`,
  `action_bar_test`, `intercept_screen_test`, `undo_test`, `rebuild_test`,
  `history_handoff_test`, `smoke_test`) bauten `HumanitlApp()` ohne
  `uiStateOverride()` und lasen damit `~/.local/state/humanitl/ui-state.json`.
  Beim Besitzer steht dort `shellRailExpanded: true`; die Leiste der Shell
  nimmt dann 200 statt 48 px, der Inspector fällt bei 1280 px auf seine
  480 px, die Leiste schaltete auf die Spalte, und der Haltegrund wurde mit
  den Quadraten der Testschrift gekürzt. In der CI gibt es die Datei nicht.
  Schriftmetrik, Plattform und Textskalierung waren es nicht. Alle sieben
  Dateien tragen jetzt `uiStateOverride()`; `test/harness/ui_state_guard_test.dart`
  hält das am Quelltext fest.
- **Ursache der Meldung.** Dieselbe Spalte im echten Fenster: Unter 800 px
  Inspector stand Block unter Allow, also bei 1920 px Fenster
  ((1920 − 48) · 0,44 − 24 = 799 px) gerade noch, mit aufgeklappter Leiste
  erst recht. Mit Inter oder Roboto passen Ventil, Notizplatz und Block schon
  in 456 px.
- **`DecisionRow`** (`widgets/decision_row.dart`), ein eigenes
  Render-Objekt mit vier Plätzen (Ventil, Edit, Notiz, Block), ersetzt
  `LayoutBuilder`, `_wide`, `_narrow`, den `Wrap` und `actionBarWrapWidth`.
  Es misst die Kinder und gibt in der Reihenfolge der Spezifikation nach:
  Abstand von [decisionGap] (160) bis [decisionGapMin] (96, neu), dann
  verlässt Edit die Zeile, dann schrumpft das Ventil bis 120 px (oder bis
  zu seiner eigenen kleinsten Breite, wenn die größer ist; die liefert
  `getMinIntrinsicWidth`, seit die Beschriftung des Ventils in
  `ShrinkableLabel` steht, denn ein einzeiliger Text mit Ellipse meldet
  sonst seine volle Breite als Minimum), zuletzt
  Block unter Allow. Der Abstand zählt vom rechten Rand der Allow-Seite bis
  Block, der leere Notizplatz steht darin. Alle Kinder stehen vertikal
  zentriert. Das ausgelassene Edit wird nicht gemalt, nicht getroffen,
  nicht vorgelesen (`visitChildrenForSemantics`) und verliert im selben
  Layout Fokus und Tabstopp: Ein eigener `FocusNode` vor dem Edit
  (`_editGate`) bekommt `descendantsAreFocusable` und
  `descendantsAreTraversable` direkt gesetzt, ohne Neubau. Die erste
  Fassung nahm es per `ExcludeFocus` erst einen Frame später heraus; ein
  `Enter` in diesem Frame hätte den Editor aus einem unsichtbaren Knopf
  geöffnet (Review Codex). Das offene Raster steht jetzt unter der Zeile
  statt zwischen Allow und Block.
- **Review.** Codex nannte die privaten benannten Konstruktorparameter
  (`required this._gap`) blockierend; verworfen: Dart 3.13 erlaubt private
  benannte Parameter, Analyse und Tests laufen damit. Übernommen: Fokus im
  selben Frame (siehe oben), 12 px statt 0 über einer eigenen Edit-Zeile
  in der Spalte, ein Test der echten Spalte bei doppelter Schrift im
  480-px-Pane (Block rechtsbündig an der Innenkante), Grundlinien statt
  Unterkanten der Beschriftungen im Matrixtest, der Wächter prüft jeden Bau
  von `HumanitlApp()` einzeln, und der Kommentar zu `decisionGap` sagt,
  dass 160 nur noch in die intrinsische Breite eingeht.
- **Offene Entscheidung des Besitzers.** Im Vorgabefenster 1280 px fällt
  „Edit + Allow" auf Deutsch aus der Zeile und bleibt über `E` erreichbar.
  So bleibt es, bis der Besitzer anders entscheidet.
- **Hover der Notiz.** Statt einer `MouseRegion` um den `Wrap` hat Block
  eine und der Notizplatz eine, deren Fläche bis an Block reicht; beim
  Übergang fallen Verlassen und Betreten in dasselbe Ereignis, der Platz
  flackert nicht.
- **Deterministische Schrift.** `test/harness/layout_fonts.dart` lädt Roboto
  aus `bin/cache/artifacts/material_fonts` des Flutter-SDK unter dem Namen
  `Inter`, gleich in CI und lokal; Inter selbst liefert die App nicht mit.
  Die Monospace-Familie bleibt bei den Quadraten (breiter als JetBrains Mono,
  also auf der sicheren Seite). Benutzt von `action_bar_de_test` und
  `action_bar_one_line_test`.
- **Tests.** `action_bar_one_line_test.dart`: 24 Fälle (en/de, 1,0/1,3,
  schmalster Pane mit aufgeklappter Leiste, Vorgabefenster, 1920 px, ohne
  und mit Funden) prüfen gleiche Mitte und Grundlinie von Allow und Block,
  mindestens 96 px Abstand, keine umbrochene Beschriftung eines Controls und
  bei 1,0 ungekürzte Beschriftungen beider Entscheidungen (Ausnahme:
  „Senden mit 2 Funden" im schmalsten Pane kürzt sich, bevor Block
  umbricht); dazu Edit sichtbar und 160 px breit, Edit im schmalsten Pane
  ausgelassen und fokusgesperrt. `decision_row_test.dart` legt jede Stufe auf
  den Pixel (478/477, 366/336/335). `action_bar_test`: „the bar wraps and
  puts block on its own line" (prüfte genau den Fehler) wird „a narrow pane
  keeps block on the line of allow", neu „at twice the text scale block
  moves under allow". `uiStateOverride` hat ein optionales `railExpanded`.
- **Goldens neu (33, nur die von der Leiste veränderten):**
  `action_bar_findings.{dark,light}-de`, `action_bar_narrow_{dark,light}`,
  `action_bar_note_{dark,light}`, `action_bar_remember_open.{dark,light}-{de,en}`,
  `action_bar_text_scale_2`, `intercept_diagnostic_tls_{dark,light}`,
  `intercept_empty_{dark,light}`, `intercept_three_held_{dark,light}`,
  `queue_grouped_collapsed_{dark,light}`, `queue_grouped_expanded_{dark,light}`,
  `queue_grouped_scale2_{dark,light}`, `queue_new_pill_{dark,light}`,
  `shell_{dark,light}`, `shell_settings_{dark,light}`,
  `states_tls_rejected.{dark,light}-{de,en}`. Auf dem alten Stand sind alle
  grün; neu erzeugt wurden genau die, die mit der Änderung rot wurden.
- **Mutationsbeweise.** Alte `action_bar.dart` (Spalte unter 800 px): 24 von
  24 Matrixfällen rot, dazu der schmale Fall in `action_bar_test` und
  „drops the edit control". `uiStateOverride()` aus `action_bar_de_test`
  entfernt: Wächter rot, und lokal beim Besitzer auch „with findings as
  well". Edit in Stufe 3 nicht ausgelassen: drei rot („drops the edit control",
  zwei Stufentests). Fokustor nie geschaltet: drei rot (Stufentest an
  477 px, „loses focus in the frame it leaves", „drops the edit control").
  Der alte Weg über `ExcludeFocus` nach einem Frame: „loses focus in the
  frame it leaves" rot (`Enter` öffnet das Edit noch). Kein Abstand über
  einer eigenen Edit-Zeile: „gets its own" rot. Block oben statt
  zentriert: 12 Fälle bei 1,3 rot. Kleinster Abstand ignoriert
  (`free(0)`): Stufentest an 477 px rot. Kleinste Breite des Ventils
  ignoriert (nur 120 px): Stufentest „never shrinks below" rot, und
  `queue_grouped_scale2_*` läuft im Ventil über. Ohne `ShrinkableLabel`
  um die Beschriftung des Ventils: sechs Matrixfälle mit Funden rot (Block
  rutscht unter Allow, weil das Ventil seine volle Beschriftung als
  Minimum meldet).

---
## HUM-309 · pi gets web search and fetch through pi-web-access
Sprint: 5 · Size: S · Dependencies: HUM-274, HUM-304, HUM-308

### Context
Owner request of 2026-10-29: pi in the sandbox should search and fetch the
web, and every request must still wait in the proxy queue (Intercept). pi has
no web tool of its own (HUM-308). The npm package `pi-web-access` (0.46.0,
MIT, `github.com/nicobailon/pi-web-access`) adds the tools `web_search`,
`fetch_content`, `source_check` and `get_search_content`. URL fetch works
without a key; search uses providers such as Exa, Brave, Tavily or Jina, most
of which need an API key.

`pi install npm:pi-web-access` on the host does not help: it writes to
`~/.pi/agent`, and pi's agent directory in the sandbox is a fresh tmpfs
(`PI_CODING_AGENT_DIR=/home/agent/.pi/agent`). The owner installs the package
globally instead, `sudo npm install -g --ignore-scripts pi-web-access`; it
lands under `/usr/local`, which the sandbox mounts read-only, next to pi
itself.

This is the first building block of the planned agent setups (HUM-310:
layered defaults < user < enforced). It stays a small, explicit feature of
the pi adapter; no general extension manager.

### Specification
1. **Detection.** The pi adapter looks for the package where a global npm
   install puts it: the `node_modules` directory that holds pi itself (from
   the resolved pi command), then `/usr/local/lib/node_modules` and
   `/usr/lib/node_modules`. A candidate counts only when the sandbox reaches
   `<dir>/pi-web-access/package.json` through its mounts and the file names
   the package `pi-web-access`. Undecidable lookups count as not found and
   produce no finding (`backlog/CONVENTIONS.md` 4.13).
2. **Loading.** When found and switched on, the rendered `settings.json`
   lists the package directory, as a sandbox path, under `packages`. pi loads
   a local package from that path without copying or installing anything.
3. **DNS.** The extension checks every target with a local DNS lookup before
   it fetches (SSRF guard). The sandbox has no resolver, so every fetch would
   fail. With the package loaded the adapter also writes
   `<agent-dir>/web-search.json` with `ssrf.trustEnvProxy: true`: the proxy
   from the environment resolves names, as for every other client.
4. **Configuration.** `agent.pi.web_access`, values `auto` (default: load
   when installed), `on` (load, and say so when missing), `off` (never
   load). Tier advanced, denied in the project profile: loading third-party
   code into the agent is the user's decision. Validated by the schema like
   every enum, so `SetConfig` and `humanitl config set` refuse other values.
   No form field in the app in this issue.
5. **Finding.** `on` with the package missing gives `AGENT_007` (warning, the
   start goes on without web tools) with `why` and the npm command as
   `CopyCommand`. `auto` with the package missing says nothing.
6. **Proxy.** Measure that the extension's requests go through
   `HTTPS_PROXY` (pi's own `EnvHttpProxyAgent` dispatcher, `NODE_USE_ENV_PROXY`
   from HUM-304) and therefore appear in Intercept.
7. **Documentation.** `agents/pi/README.md`, `docs/guides/agent-profiles.md`
   ("Using pi"), `docs/SECURITY.md` (third-party extension code runs inside
   the sandbox with all its restrictions), `docs/CONFIG.md`,
   `backlog/CONVENTIONS.md` 4.4.

### Non-goal
- No keyring handling for search-provider API keys. Keyless fetch works; keys
  (keyring, a file into the sandbox, masks in the recorder) are a candidate
  for a new issue.
- No general extension or package manager, no other pi packages.
- No install of the package by Humanitl, no download at start.
- No Settings form field (follow-up in the app).

### Tests
- `pi_config.rs`: with a package path the rendered `settings.json` lists it
  under `packages` and keeps provider, model and telemetry; without one it
  sets nothing more than before; the rendered `web-search.json` sets
  `ssrf.trustEnvProxy` to `true`.
- `sandbox/tests/pi_adapter.rs`: a fake global install under a mount lands in
  `settings.json` and brings `web-search.json`; `off` loads nothing; `auto`
  without the package loads nothing and finds nothing; `on` without the
  package gives `AGENT_007` with a `CopyCommand`; a directory without a
  matching `package.json` does not count.
- Config: the key is registered, defaults to `auto`, is denied in the
  project profile and refuses unknown values.
- Mutation proof per new assertion, restored byte-identical.

### Acceptance criteria
- [ ] With `pi-web-access` installed globally, pi in the sandbox offers
      `web_search` and `fetch_content`, and a fetch shows up as a held
      request in Intercept.
- [ ] `agent.pi.web_access = off` keeps the package out; `auto` without the
      package changes nothing; `on` without it gives `AGENT_007` with the
      npm command.
- [ ] No API key and no host file of the user reaches the sandbox.
- [ ] README security statement unchanged; `docs/SECURITY.md` names the
      third-party code and its limits.

### Umsetzung (HUM-309)

- **Gemessen, wie pi das Paket lädt.** pi 1.4.0 (`docs/packages.md`,
  `docs/settings.md` des installierten Pakets, `~/.pi` nicht gelesen): ein
  lokaler Pfad unter `packages` in `settings.json` wird an Ort und Stelle
  geladen, nichts kopiert, nichts installiert. Probe mit `npm install -g
  --prefix <scratch> --ignore-scripts pi-web-access@0.46.0` (212 Pakete, die
  optionalen Peers von pi nicht mit, `typebox` schon), `PI_OFFLINE=1`,
  leerem Agentenverzeichnis und `pi -p` gegen einen nachgebauten
  OpenAI-Server: pi bot `read, bash, edit, write, web_search, source_check,
  fetch_content, get_search_content` an; kein `web_enable`-Umweg.
- **Gemessen, wohin die Anfragen gehen.** Logging-Proxy in `HTTP(S)_PROXY`,
  `ALL_PROXY`, `NO_PROXY` leer, `NODE_USE_ENV_PROXY=1`, wie in der Sandbox.
  `fetch_content` auf `http://example.invalid/hum309`: `CONNECT
  example.invalid:80`, darin `GET /hum309`, die Seite kam als Markdown beim
  Modell an. Keyless `web_search`: `CONNECT mcp.exa.ai:443`. Der Weg ist pis
  eigener Dispatcher (`core/http-dispatcher.js`: `EnvHttpProxyAgent` mit
  `proxyTunnel: true`, `undici.install()` ersetzt das globale `fetch`); die
  Erweiterung ruft `fetch` und baut selbst keinen Agenten (nur
  `gemini-web.ts` nimmt `EnvHttpProxyAgent`, ebenfalls aus der Umgebung).
  Ein `proxy` in `web-search.json` schaltete sie auf `curl` um; den setzt die
  Vorlage nicht.
- **Gemessen, warum `web-search.json` nötig ist.** Ohne `ssrf.trustEnvProxy`
  löst die Erweiterung jeden Namen selbst auf, bevor sie abruft:
  `Error: Failed to resolve example.invalid: getaddrinfo EAI_AGAIN`. In der
  Sandbox gibt es keinen Resolver, also schlüge jeder Abruf fehl. Mit dem
  Schalter ging die Anfrage an den Proxy. Literale private Adressen und
  `localhost` lehnt die Erweiterung weiter ab.
- **Befund zur Suche.** Keyless-Suche geht an `mcp.exa.ai`, und die
  mitgelieferte Regel `…0006` (`**.exa.ai`, für OpenCode geschrieben) sperrt
  das in jeder Sitzung. Ohne eigene Regel des Nutzers bleibt `web_search`
  also erfolglos; `fetch_content` geht. Dokumentiert in
  `docs/guides/agent-profiles.md`; die Regel bleibt, wie sie ist (Kandidat).
- **Bau.** `agent.pi.web_access` (`PiAgentConfig`, `PiWebAccess`
  `auto|on|off`, Stufe advanced, im Projekt gesperrt, CONVENTIONS 4.4 und
  die Sperrliste). `AgentContext::pi_web_access`, gesetzt in
  `ipc::sandbox` und `humanitl sandbox`. `pi.rs`: `find_web_access` sucht im
  `node_modules` neben pi (aus dem aufgelösten Skript), dann
  `/usr/local/lib/node_modules`, `/usr/lib/node_modules`, über
  `search_view` und nur mit `package.json` `name = "pi-web-access"`;
  Unentscheidbares schweigt. `web_access_package` entscheidet nach Schalter,
  `files` nimmt `packages` und `web-search.json` (nur lesbar) auf,
  `preflight` meldet `AGENT_007` (Warnung, `CopyCommand` mit der
  npm-Zeile) nur bei `on` und fehlendem Paket. Vorlage
  `agents/pi/web-search.json.tmpl`, `render_web_search` lehnt eine Vorlage
  ohne `trustEnvProxy: true` mit `AGENT_003` ab. `AGENT_007` mit ARB
  (en, de) und `diagnostic_l10n.dart`. Kein Feld im Einstellungsformular.
- **Tests.** `pi_config`:
  `the_settings_list_the_web_access_package_when_given`,
  `the_web_search_file_lets_the_proxy_resolve_names`. `pi_adapter`:
  `a_global_install_under_usr_local_lands_in_the_settings`,
  `the_extension_is_found_next_to_pi_itself`,
  `web_access_off_loads_nothing_even_when_installed`,
  `web_access_auto_without_the_package_changes_nothing`,
  `web_access_on_without_the_package_warns_with_the_npm_command`,
  `only_a_package_json_that_names_the_extension_counts`. Config:
  `pi_web_access_defaults_to_auto_and_refuses_other_values`, Register,
  Sperrliste, Schema-Fixture, `docs/CONFIG.md`, `docs/DIAGNOSTICS.md`,
  `docs/reference/cli.md` erzeugt.
- **Grenzen der Erkennung.** `node_modules_of_pi` nimmt den kanonischen
  Host-Pfad des pi-Skripts als Pfad in der Sandbox. Gefunden wird das Paket
  neben pi deshalb nur, wo die Sandbox denselben Pfad sieht (gleichnamige
  Einhängung wie `/usr`); liegt pi unter einer anders benannten Einhängung,
  bleibt es bei den festen Orten. Die Lücke fällt in die sichere Richtung:
  nichts wird geladen, was die Sandbox nicht unter genau diesem Pfad hat,
  denn `package.json` wird über `search_view` nachgeschlagen.
- **Was nicht Ende zu Ende gezeigt ist.** Kriterium 1 (die Anfrage erscheint
  in Intercept) folgt aus der Architektur: Die Anfrage kam am Proxy aus
  `HTTPS_PROXY` an, und in der Sandbox ist das die Brücke zum Proxy, der jede
  Anfrage ohne entscheidende Regel hält. Ein Lauf mit echtem Daemon, echter
  Sandbox und installiertem Paket fehlt; der Besitzer prüft es von Hand.
- **Review Runde 1** (Opus, Codex; keine blocking/major, alle minor
  übernommen): `names_web_access` liest nur eine reguläre Datei (Typ vor dem
  `open`, `O_NONBLOCK`, Typ des Deskriptors danach), Test
  `a_fifo_as_package_json_does_not_hang_the_lookup`; `docs/SECURITY.md`
  „in a read-only mount (normally `/usr`)“; Leitfaden und THREAT-MODEL
  sagen, dass eine Anfrage nur ohne entscheidende Regel gehalten wird.
- **Mutationsbeweise**, alle Dateien danach mit `cmp` byte-gleich zurück
  (und mit frischem mtime, sonst baut cargo nicht neu): `packages` leer:
  `the_settings_list…` und `a_global_install…` rot; Vorlage
  `trustEnvProxy: false`: `the_web_search_file…` und `a_global_install…`
  rot; `off` wie `auto`: `web_access_off…` rot; Befund auch bei `auto`:
  `web_access_auto…` rot; jede `package.json` zählt:
  `only_a_package_json…` rot; nichts neben pi: `the_extension_is_found…`
  rot; kein `web-search.json`: `a_global_install…` rot; Befund blockierend:
  `web_access_on…` rot; Vorgabe `on`: `pi_web_access_defaults…` rot;
  Projekt `allowed`: `the_denied_keys…` rot.
- **Offen.** Schlüssel der Suchanbieter (Schlüsselbund, Datei in die
  Sandbox, Masken im Recorder) als eigene Issue; Regel `…0006` für pi
  lockern oder pro Adapter schneiden; Feld im Einstellungsformular; die
  Erweiterung als Baustein der Agent-Setups (HUM-310).

---
## HUM-305 · Pane boundaries can be dragged, and the sandbox terminal can grow

### Context
Owner request of 2026-10-29: "die Mounts usw müssen nicht so viel Platz
einnehmen, ich kann die Kante vom Terminal nicht runter- und das Terminal
größer ziehen". The Sandbox section splits its body at a fixed 60/40 between
the terminal (HUM-042, HUM-298) and the tabs below it (mounts, environment,
isolation, log). The boundary is a plain hairline; nothing can be dragged, and
on a laptop screen the terminal keeps fewer rows than a TUI agent wants.

The two-column screens already have `HResizablePanes` (HUM-020): the three
Intercept panes can be dragged, but the ratios are lost on restart, and the
editor (HUM-031) passes fixed ratios with a callback that drops every drag, so
its splitter moves the cursor but never the boundary.

### Specification
1. `HResizablePanes` gets a vertical axis. One widget for both directions:
   the same minimums, the same whole-pixel seams (HUM-286), the same grip
   geometry (`HSize.splitter`, hairline at rest, `HSize.splitterActive` while
   dragged), a row-resize cursor on the vertical axis.
2. The Sandbox body is two panes on the vertical axis: the terminal above,
   tab bar plus tab content below. Dragging the boundary down grows the
   terminal; the lower pane shrinks to a slim minimum (the tab bar and about
   one row) and its content scrolls. The terminal keeps a minimum of a few
   rows. The default split stays 60/40.
3. Every boundary between two panes is draggable: the Intercept panes, the
   editor halves and the Sandbox split.
4. Each screen remembers its split across restarts in the UI state file of
   HUM-254 (`$XDG_STATE_HOME/humanitl/ui-state.json`), under a key of its
   own. The file is written when a drag ends, not on every pixel. A missing,
   unreadable or malformed entry means the default split.
5. A double click on a boundary restores the default split of that screen and
   stores it.
6. Every boundary is a focus stop; the arrow keys of its axis move it by
   `HSize.splitterStep`, and the move is stored like a drag. Keys reach the
   boundary only while it has the focus: the terminal keeps every arrow key
   while it has the focus.
7. While the terminal grows or shrinks, its new columns and rows reach the
   PTY so the TUI redraws, but not once per pixel: resizes go out at most
   once per short interval, the last size of a drag always goes out, and a
   size equal to the last one sent is not sent again.
8. The UI state store is a core service, not a part of the Intercept feature:
   the Sandbox section and the editor may not import another feature
   (`tools/check-deps.sh`).

### Acceptance criteria
1. Widget tests on the Sandbox screen: dragging the boundary down grows the
   terminal; the lower pane never goes below its minimum; the split is stored
   and a new screen over the same file starts with it; a double click
   restores the default; the arrow keys move a focused boundary and the
   terminal keeps its arrow keys.
2. A widget test drags the boundary in many small steps and counts the
   `TerminalResize` commands at the daemon: fewer than the number of row
   changes, and the last one carries the final size.
3. The same persistence test for the Intercept panes and the editor halves.
4. Every new test has a mutation proof; only goldens the change alters are
   regenerated.

### Umsetzung (HUM-305)

- **Lage der Naht.** Im Sandbox-Abschnitt steht das Terminal oben und die
  Reiter darunter, also wird die Naht nach unten gezogen, um das Terminal zu
  vergrößern (so auch der Wortlaut des Besitzers, „runterziehen"). Die
  Reiter schrumpfen bis `sandboxDetailsMinHeight` (72 px: Reiterleiste und
  etwa eine Zeile); unter `sandboxDetailsContentFloor` (120 px) rollt der
  Inhalt des Reiters als Ganzes, statt gequetscht zu werden. Das Terminal
  behält mindestens `sandboxTerminalMinHeight` (160 px). Die Vorgabe bleibt
  60/40, jetzt von der ganzen Höhe unter dem Kopf; die Reiterleiste gehört
  zum unteren Pane, und die Haarlinie zwischen Terminal und Reitern zeichnet
  der Griff.
- **Ein Widget, zwei Achsen.** `HResizablePanes` hat `axis`, `onDragEnd` und
  `onReset`. Vertikal: `Flex` statt `Row`, liegender Griff,
  `resizeRow`-Zeiger, Pfeil hoch/runter. Die Pfeile der Naht gelten nur,
  solange sie den Fokus hat; das Terminal behält seine Pfeile. Zwei
  Ereignisse in einem Frame (zwei Tastendrücke, zwei Zeigerbewegungen)
  rechneten bisher beide von den alten Verhältnissen aus und der zweite
  überschrieb den ersten; das Widget rechnet jetzt bis zum nächsten Frame
  von dem weiter, was es zuletzt gemeldet hat. Der Doppelklick ist von Hand
  gezählt (zwei Klicks ohne Zug innerhalb `kDoubleTapTimeout`), weil
  `onDoubleTap` nach jedem Zug einen Zeitgeber hinterlässt, der jeden
  Ziehtest mit „Timer still pending" rot machte.
- **Gemerkt je Bildschirm.** `PaneSplit` (`core/ui/pane_split.dart`), eine
  Familie über `PaneSplitSpec(key, defaults)`: `set` nur im Speicher
  während des Zugs, `commit` am Ende des Zugs und nach jedem Tastendruck,
  `reset` beim Doppelklick. Schlüssel `paneSplit.sandbox`,
  `paneSplit.intercept`, `paneSplit.editor`. Ein fehlender oder kaputter
  Eintrag (falsche Länge, keine positive Zahl) ist die Vorgabe.
- **`UiStateFile` in den Kern.** Sandbox und Editor dürfen kein anderes
  Feature importieren (`tools/check-deps.sh`); die Datei und
  `uiStateFileProvider` stehen jetzt in `core/ui_state/ui_state_file.dart`
  (handgeschriebener `Provider`), `coach_mark.dart` exportiert sie weiter,
  damit bestehende Importe gelten. Neu: `setValue` für Listen.
- **Editor.** Der Griff verwarf bisher jeden Zug (`onRatiosChanged: (_) {}`);
  jetzt zieht er und merkt sich die Aufteilung.
- **Größe zum PTY.** `TerminalView` misst sich bei jeder Höhe neu und
  meldet über `terminal.onResize`; `TerminalSession._resize` schickt die
  erste Größe sofort, sammelt danach für `terminalResizeInterval` (100 ms)
  und schickt am Ende der Frist nur die letzte, und eine Größe gleich der
  zuletzt gesendeten (auch der aus `TerminalOpen`) gar nicht.
  `terminal_pane.dart` ist unberührt (HUM-303 arbeitet dort).
- **Testgerüste.** `sandboxUnderTest` legt `uiStateOverride()` vor, Editor-
  Test und Editor-Golden ebenso; sonst läsen sie die Datei des Menschen.
- **Mutationsbeweise** (je angewandt, Test rot, Datei byte-gleich zurück,
  per SHA-256 geprüft): Drossel aus (`_resizeQuiet`-Prüfung weg) und
  nachlaufende Größe weg (beide `the terminal resize reaches the daemon`);
  `onDragEnd` im Sandbox-Abschnitt weg und `commit` in `set` (beide
  `the split is written when the drag ends`); `onReset` weg (`double click`);
  vertikale Achse mit links/rechts und Rechnen von den alten Verhältnissen
  (beide `arrow keys`); Minimum der Reiter 0 und Rollboden aus (beide
  `minimum`); vertikaler Zug liest `dx` (`grows the terminal`); Länge des
  gespeicherten Eintrags ungeprüft (`core/ui/pane_split_test.dart`);
  `onDragEnd` im Intercept weg (`the splitter drags`); Editor verwirft Züge
  (`the halves can be dragged`).
- **Goldens.** Neu erzeugt nur die 20, die die Änderung verschiebt:
  `sandbox_header_{stopped,running,failed}_{dark,light}`,
  `states_sandbox_failed.*` (4), `isolation_panel_{ok,failed}.*` (8),
  `isolation_panel_no_report_{dark,light}`. Das Terminal nimmt jetzt 60
  Prozent der ganzen Höhe unter dem Kopf; das hohe Fenster der
  Isolations-Goldens wächst deshalb von 1100 auf 1160 px, damit der Reiter
  wieder so viel zeigt wie vorher.
- **Nach dem Review.** Ein Zug zählt nicht als Klick: Der Griff zählt den
  gemeldeten Zugweg seit dem Aufsetzen, und über `kTouchSlop` ist das
  Loslassen kein Klick (Codex: Zug um 80 px plus ein Klick setzte zurück; der
  Start des Erkenners taugt nicht als Merkmal, weil er allein in der Arena
  schon beim Aufsetzen gewinnt). `PaneSplit` verwirft eine Summe, die
  überläuft, und jede Aufteilung, in der ein Pane nach dem Normieren null
  hätte (`[1e308, 1e308]`, `[1e308, 1e-308]`). Die Tests für Intercept und
  Editor bauen jetzt einen zweiten Baum über derselben Datei und prüfen die
  Breiten; der Drosseltest zieht mit 16 ms je Frame und prüft höchstens eine
  Meldung je Intervall plus zwei. Neue Mutationen: Zug als Klick gezählt,
  Null-Anteil angenommen, gespeicherte Aufteilung beim Start ignoriert
  (Intercept und Editor) -- alle rot, Dateien byte-gleich zurück.

---
## HUM-303 · The terminal takes the keyboard on hover and by shortcut

### Context
Owner request 2026-10-29. Typing to the agent takes a click into the
terminal first, in its pane as well as over the whole window (HUM-298). The
owner wants the terminal to take the keyboard as soon as the pointer is over
it, and a shortcut (owner proposal accepted: ``Ctrl+` ``) that gives it the
keyboard wherever it is shown, without changing the layout.

### Specification
1. The pointer over the emulator hands it the keyboard, without a click, in
   every host: the pane, the whole window and later the floating terminal
   (HUM-300). The rule lives with the terminal widget, so every host gets it.
2. Exceptions that keep the focus where it is: an open dialog, sheet, menu
   or the command palette (input there must never slip into the terminal),
   and a text field that has the focus while the pointer only crosses the
   terminal. The rule chosen and why is documented in `docs/UX.md` 5.2.
3. A core intent `FocusTerminalIntent`, bound in `shellShortcuts()` to
   ``Ctrl+` ``, gives the keyboard to the terminal wherever it is shown and
   changes nothing in the layout. With no terminal on the screen it does
   nothing for now (HUM-300 will open the floating terminal). On layouts
   where the backquote is a dead key the chord is matched by the physical
   key. No repeats.
4. The chord is not swallowed by the terminal: it takes the way of the
   other application keys through `TerminalPane._onKey` and never reaches
   the agent. It collides with no binding (`app/lib/core/shortcuts/intents.dart`,
   intercept decision keys, editor, CONVENTIONS 3.9).
5. The command palette lists it (while a terminal is shown); `docs/DESIGN.md`
   shortcut table, `docs/UX.md` 5.2 and CONVENTIONS 3.9 name it; the palette
   label is in ARB (`en` source, `de` translation, appended).

### Acceptance criteria
1. Widget test: hovering the terminal gives it the keyboard, and `Enter`
   then reaches the agent as `0x0d`, not as an Allow.
2. Widget test: with a held request selected and the focus in the queue,
   `Enter` allows (counter-check) and the agent hears nothing; after the
   pointer moved over the terminal and the focus actually moved, `Enter`
   goes to the agent and the queue decides nothing. Never both.
3. Widget tests: a text field, the terminal's context menu, an open sheet
   and the palette keep the keyboard while the pointer is over the terminal.
4. Widget tests: ``Ctrl+` `` gives the keyboard to the terminal in its pane
   and over the whole window, leaves the layout as it was, does not reach the
   agent, and does nothing with no terminal on the screen; the activator
   matches the physical key, exactly `Ctrl`, no repeats.
5. Each behaviour has a mutation proof; `make flutter-analyze` and the tests
   of the touched packages are green; no golden changes.

### Fallstricke
- A delay before hover focus (say 150 ms) looks gentler and is the less safe
  choice: while it runs, the pointer is on the terminal and an `Enter` meant
  for the agent still allows the selected request.
- `HModal`, `HSheet` and the menu of `HContextMenu` hold the keyboard in a
  `FocusScope(autofocus: true)` of their own. The terminal's context menu
  opens under the pointer, over the terminal, and lets hover through; a
  hover rule without the scope check would take the keys from it at the next
  movement.
- The palette closes before it runs a command, and its field holds the focus
  until the frame ends; a request for the keyboard from the palette has to
  wait for that frame, or the scope rule refuses it.
- `` ` `` is a dead key on German (`^`) and other layouts; a
  `SingleActivator` on the logical backquote would match nothing there.

### Umsetzung (HUM-303)

- **Regel beim Terminal.** `TerminalPane._mayTakeKeyboard` gilt für jeden
  Wirt (Pane, ganzes Fenster, später das schwebende Terminal). Ein
  `MouseRegion` um den Emulator ruft sie bei `onEnter` und `onHover`; auch
  bei jeder Bewegung, damit ein Textfeld, das den Fokus inzwischen abgegeben
  hat, nicht verhindert, dass der Zeiger ihn holt.
- **Entschieden: Fokus sofort, keine Verzögerung.** Zwei Ausnahmen halten
  ihn. (1) Eine Ebene über der Arbeit: Das Terminal nimmt den Fokus nur von
  einem Knoten, dessen Bereich sein eigener ist oder einer der Bereiche um
  ihn. `HModal`, `HSheet` und das Menü von `HContextMenu` haben je einen
  eigenen `FocusScope`, also deckt das Dialog, Blatt, Menü und Palette ohne
  Liste. Gilt auch für das Kürzel. (2) Ein Textfeld mit Fokus, nur für den
  Zeiger (`isTextInputFocused`); das Kürzel fragt ausdrücklich und darf es.
  Eine Verzögerung von 150 ms wurde verworfen: In dieser Zeit stünde der
  Zeiger auf dem Terminal, und ein `Enter` für den Agenten gäbe in der
  Warteschlange frei.
- **Kürzel.** `FocusTerminalIntent` und `TerminalFocusActivator` in
  `core/shortcuts/intents.dart`: physische Taste links der 1 oder logischer
  Backquote, genau `Ctrl`, nur `KeyDownEvent` (keine Wiederholung). In
  `shellShortcuts()`, also nimmt das Terminal über `_onKey` denselben Weg wie
  `Ctrl+K`. Die Shell zählt `terminalFocusRequestProvider`
  (`core/ui/terminal_placement.dart`) hoch; jeder Wirt hört zu, nur der
  sichtbare mit Sitzung handelt. Ohne sichtbares Terminal passiert nichts.
- **Palette.** Befehl `focus-terminal` am Ende, nur solange ein Terminal mit
  Sitzung zu sehen ist (ganzes Fenster oder Abschnitt Sandbox). Die Anfrage
  wartet auf den Frame nach dem Schließen, weil bis dahin das Feld der
  Palette den Fokus hält und die Bereichsregel ablehnen würde.
- **Doku.** `docs/UX.md` 5.2, `docs/DESIGN.md` Kürzeltabelle,
  CONVENTIONS 3.9, ARB `shellPaletteFocusTerminal` (en, de, angehängt).
- **Tests.** `test/features/shell/terminal_keyboard_test.dart` (zwölf):
  Zeiger gibt die Tastatur, `Enter` kommt als `0x0d` an; Warteschlange
  neben dem Terminal (nimmt HUM-300 vorweg): `Enter` gibt zuerst frei,
  Agent hört nichts; nach dem Darüberfahren steht der Fokus nachweislich im
  Terminal, `Enter` geht an den Agenten, keine zweite Entscheidung;
  Notizfeld behält den Fokus beim Darüberfahren; Kontextmenü des Terminals
  und Einstellungsblatt behalten ihn unter dem Zeiger, `Esc` schließt das
  Menü und erreicht den Agenten nicht; Palette behält ihn gegen Zeiger und
  Kürzel, listet den Befehl nur mit sichtbarem Terminal, der Befehl gibt die
  Tastatur; Kürzel im Pane und über dem ganzen Fenster, Layout bleibt;
  Kürzel erreicht den Agenten nicht; ohne Terminal tut es nichts;
  Aktivator: andere logische Taste auf derselben physischen Taste, kein
  `Shift`, kein Wiederholen. Eine Zeile in
  `shortcuts_map_to_intents_test.dart`. Keine Goldens geändert.
- **Mutationsbeweise**, je rot und danach mit `cmp` byte-gleich zurück:
  Zeiger-Handler entfernt; Bereichsregel aus; Textfeldregel aus; Bindung
  entfernt; nur logischer Backquote; Wiederholung erlaubt; `Shift`
  ignoriert; Wirt hört nicht auf die Anfrage; Shell ohne Action; Befehl
  immer in der Palette; Palette-Anfrage ohne Warten auf den Frame (zuerst
  grün, weil der ruhende Zeiger die Tastatur beim Verschwinden der Palette
  brachte; der Test führt den Zeiger jetzt weg und nimmt `Enter`, dann rot).
  Grün und so belassen: ohne `Visibility.of`-Prüfung, weil `ExcludeFocus`
  des `IndexedStack` die Anfrage eines verdeckten Wirts schon abweist; die
  Prüfung bleibt als zweite Wand. Bindung entfernt lässt
  `the_chord_never_reaches_the_agent` grün: Der Emulator schickt für
  `Ctrl+Backquote` im Test ohnehin kein Byte; der Test hält das Ergebnis
  fest, nicht den Weg.
- **Review (Codex, Sonnet), nachgebessert.** Kontextmenü, Blatt und Modal
  nahmen den Fokus nicht, solange das Terminal ihn hielt:
  `FocusScope(autofocus: true)` gibt nach, sobald im Bereich darum etwas
  fokussiert ist. `Esc` im Terminalmenü ging an den Agenten, Tippen in einem
  über dem Terminal geöffneten Blatt ebenso (Akzeptanzkriterium 3). Neu
  `HLayerScope` (`packages/ui`, `h_layer_scope.dart`), von `HModal`,
  `HSheet` und dem Menü von `HContextMenu` benutzt: Der Bereich fordert den
  Fokus einen Frame nach dem Erscheinen selbst an, außer etwas darin hat ihn
  schon. Zurück geht er beim Schließen an den, der ihn vorher hatte
  (ausdrücklich; siehe zweite Runde).
  `HLayerScope.holdsFocus()` sagt, ob gerade eine Ebene die Tastatur hat
  (auch im Release-Build, anders als `debugLabel`). Palette über einem
  Blatt oder Modal (Befund Codex): Die Shell merkt sich das beim Öffnen,
  parkt den Fokus beim Schließen nicht auf sich oder dem Terminal, sondern
  überlässt ihn der Ebene darunter, und bietet „Focus the terminal" dann
  nicht an; `_requestTerminalFocus` fragt nicht, solange eine Ebene die
  Tastatur hat. Tests: Menü und Blatt ohne Hilfsgriff, also auf dem echten
  Weg (Menü über dem fokussierten Terminal nimmt die Tastatur, `Esc`
  schließt es, der Agent hört nichts, das Terminal bekommt sie zurück;
  Blatt mit `Ctrl+,` aus dem Terminal, `A` und `Enter` erreichen den Agenten
  nicht); `the_palette_over_a_sheet_gives_the_keyboard_back_to_the_sheet`;
  `hover_takes_the_keyboard_when_nobody_holds_it` (Befund Sonnet; ein
  `primaryFocus` von `null` gibt es nach dem ersten Frame nicht, gemessen
  wird mit dem Bereich als Halter, im Aufbau ohne Shell, weil die Shell den
  Fokus sonst zurückholt); `packages/ui/test/layer_scope_test.dart` (drei, mit der zweiten Runde vier).
  Mutationen rot: Ebene fordert den Fokus nicht an; Palette bietet den
  Befehl über einer Ebene an; Schließen der Palette parkt auf der Shell;
  ein Bereich als Halter wird abgewiesen. Grün und belassen: die Shell fragt
  auch unter einer Ebene nach dem Terminal (die Bereichsregel des Terminals
  weist ab; zweite Wand).
- **Review zweite Runde (Codex), nachgebessert.** (1) Zwischen dem Öffnen
  einer Ebene und ihrem ersten Bild lag ein Frame, in dem eine Taste noch
  beim alten Halter ankam: `Esc` beim Agenten nach dem Rechtsklick, `Enter`
  in der Warteschlange nach „Regel anlegen". Neu
  `HLayerScope.claimKeyboard()`: Der Öffner ruft es im selben Callback, es
  merkt sich den Halter und parkt die Tastatur auf dem Bereich um ihn, wo
  keine Bindung der Arbeit lauscht. Öffner mit Griff: Kontextmenü
  (`HContextMenu._open`), Palette und Einstellungen in der Shell, „Regel
  anlegen" auf der Karte des Agenten. Die Ebene übernimmt den gemerkten
  Halter und gibt ihm die Tastatur beim Schließen ausdrücklich zurück, denn
  das Parken löscht das Gedächtnis des Bereichs. Kein Rückgeben, wenn eine
  neuere Ebene offen ist; schließt eine Ebene, aus der heraus eine neuere
  aufging, erbt die neuere deren früheren Halter (Palette, „Allow all",
  Modal). Zwei Stellen nahmen die geparkte Tastatur sofort wieder an sich
  und brauchten deshalb `HLayerScope.keyboardClaimed` (beansprucht oder
  gebaut, aber noch ohne Tastatur): `InterceptScreen._syncFocus`, das
  sich jeden Fokus auf einem Bereich darüber zurückholt, und
  `TerminalPane._takeFocus`, das beim Zurückkehren ins Pane zugriff, sobald
  die Sitzung wieder bekannt war -- also nachdem das Blatt die Tastatur
  hatte; Tippen im Blatt ging dann an den Agenten. `_takeFocus` folgt jetzt
  derselben Regel wie Zeiger und Kürzel. (2) Die Shell forderte beim
  Schließen von Palette und Einstellungen den Fokus für sich an und
  überstimmte damit die Rückgabe: Einstellungen, aus dem Terminal geöffnet,
  gaben die Tastatur der Shell statt dem Terminal. Jetzt
  `_keysBackAfterLayer`: nach der Rückgabe prüfen, und nur wenn der frühere
  Halter weg ist (das Terminal des ganzen Fensters, das die Einstellungen
  beenden), nimmt die Shell sie. Tests:
  `esc_in_the_frame_the_menu_opens_does_not_reach_the_agent`,
  `settings_opened_from_the_terminal_give_the_keys_back_to_it`,
  `when_the_holder_is_gone_the_shell_takes_the_keys`,
  `test/features/intercept/layer_opening_frame_test.dart` (`Enter` vor dem
  ersten Bild des Regelblatts entscheidet nichts, mit Gegenprobe),
  `a_layer_opened_from_a_closing_layer_returns_to_the_first_holder` in
  `packages/ui`. Nicht mit Griff, weil außerhalb dieses Issues und ohne
  Nachbarschaft zu Terminal oder Entscheidung im selben Bild: Stopp-Dialog,
  Befehlsblatt der Sandbox, Blatt und Exportmenü der History, Regel-Editor,
  Audit-Blatt, Suchblatt im Setup, Sammelmodal, About; für sie bleibt das
  eine Bild bis zur Übernahme.
  Mutationen, je rot und mit `cmp` byte-gleich zurück: Menü ohne Griff;
  Regelblatt ohne Griff; Warteschlange nimmt die geparkte Tastatur;
  gebaute, noch wartende Ebene zählt nicht als beansprucht; Terminal im
  Pane nimmt die Tastatur beim Einhängen ohne Ebenenregel; Shell fordert
  beim Schließen der Einstellungen den Fokus für sich; ohne ausdrückliche
  Rückgabe; ohne Übergabe zwischen Ebenen (Test in `packages/ui` mit
  Griff, denn ohne Griff stellt Flutters eigenes Gedächtnis den Halter
  wieder her). Grün und belassen: die Ausweichstelle der Shell, wenn der
  frühere Halter weg ist. In jedem erreichbaren Fall gibt Flutter die
  Tastatur beim Abbau des Terminalbereichs schon an den zuletzt
  fokussierten Knoten des Elternbereichs, die Shell; die Ausweichstelle
  bleibt als zweite Wand für einen Halter, der ohne einen solchen Knoten
  verschwindet. Ebenso grün und entfernt: die sofortige Anwendung der
  Fokusänderung im Griff (`applyFocusChangesIfNeeded`). Flutter wendet
  sie in einer Microtask an, und die läuft vor dem nächsten Ereignis der
  Plattform; der Fehler im ersten Versuch lag am Zurückholen durch die
  Warteschlange, nicht an der Microtask. Gefunden im vollen Lauf: „About"
  aus der Palette öffnete den Dialog im selben Bild, in dem die Palette
  ging; weil das Parken das Gedächtnis des Bereichs gelöscht hatte, gab
  Flutter die Tastatur dem ersten Bedienelement der Seite unter dem
  Dialog, der Warteschlange, und `Esc` schloss den Dialog nicht mehr
  (`Escape closes About and the shell keys work again`). Der Befehl
  öffnet den Dialog jetzt nach diesem Bild, wie „Focus the terminal";
  Mutation (Dialog wieder im selben Bild) rot.
- **Für HUM-300.** Das schwebende Terminal hält dieselbe Ebenenregel: Es
  baut `TerminalPane` und erbt damit `_mayTakeKeyboard`; ein eigener
  `FocusScope` für den schwebenden Wirt muss wie `_terminalScope` unter dem
  Bereich der Shell hängen und darf kein `HLayerScope` sein, sonst hielte
  das Terminal sich selbst für eine Ebene über der Arbeit.

---
## HUM-306 · The empty queue says the agent is working when a session runs

### Context
Owner report of 2026-10-29: Intercept showed "Start session" while a pi
agent was running in the terminal. The empty state of the queue
(`QueueEmptyState` in `features/intercept/widgets/queue_pane.dart`) offered
its primary "Start session" whenever nothing waited, whether or not a session
ran. With a running agent the queue is empty because the agent works without
asking; a button that sends somebody to start what already runs is wrong, and
it hides the one thing worth doing from there, looking at the agent.

### Specification
1. While the agent of a sandbox session runs, the empty queue says so: the
   title says that the agent is working, the hint says that every request
   that needs the person appears here. The words "nothing", "no" and "yet"
   stay out, as for every empty state (`docs/UX.md` 4.1).
2. Its one action is then "Go to terminal", filled like the start: it opens
   the Sandbox section with the agent's terminal, rail and tabs, from where
   `Ctrl+1` returns as from any section. The terminal over the whole window
   (HUM-298) stays one chord away; a floating terminal (HUM-300) may later
   take this button's place.
3. Without a running agent (no session, or a sandbox whose agent has exited)
   the empty state stays as it was: "The queue is open", the hint, and
   "Start session" into Setup.
4. The line with the three keys stands in both cases.
5. Features do not import each other (ARCHITECTURE 5,
   `tools/check-deps.sh`). The queue reads one bit from a provider in
   `app/lib/core`; the shell, the composition root, feeds it from the
   sandbox snapshot. "Running" means what the terminal asks: the agent runs,
   not merely the sandbox.
6. The empty state follows the session live: a session that starts or stops
   while the queue is showing changes it without a reload. That includes
   the end of the agent, which the daemon reports as `SandboxEvent.Exit` on
   the stream of the start and follows with no `Status`, and a session
   started or stopped on the command line, which sends this window nothing:
   the daemon has no stream that announces sessions, so while the empty
   queue is in view (Intercept shown, link live, terminal not over the
   window, nothing held) the shell asks for the snapshot when the queue
   comes into view, when the window comes back to the front, and every 5 s.
   The question is the snapshot alone, without the isolation check; it
   keeps the findings that stand and does not turn the sandbox screen red
   when it goes unanswered.
7. Without a live link the queue does not claim that the agent works.
8. Strings come from ARB, English source and German translation, appended.

### Acceptance criteria
1. Widget test: with a running agent the empty queue shows the working title
   and hint, "Go to terminal" and the three keys, and no "Start session";
   the button opens the Sandbox section.
2. Widget test: without a session the empty queue shows "Start session" as
   before and no terminal button; the button opens Setup.
3. Widget test: a sandbox whose agent has exited shows "Start session".
4. Widget test: starting and stopping a session while the queue shows
   switches the empty state both ways.
5. Widget tests: the end of the agent on the start stream switches back at
   once; a session started and stopped on the command line appears and
   goes within one interval; no question while another section shows, one
   at once on return; one at once when the window comes back to the front;
   without a link no "working"; an unanswered question leaves the snapshot.
6. A test holds the core's index of the Sandbox section to
   `Section.sandbox.index`.
7. Each behaviour has a mutation proof; `make flutter-analyze` and the tests
   of intercept, shell and sandbox are green; existing goldens are
   unchanged, a new golden shows the working state.

### Umsetzung (HUM-306)

- **Das eine Bit im Kern.** `core/ui/agent_session.dart`:
  `agentSessionProvider` (`bool`, am Anfang `false`) mit `report(running:)`.
  Gefüttert von `features/shell/agent_session_feed.dart` (`AgentSessionFeed`,
  angelegt in `initState` der Shell, entsorgt mit ihr): Bit = Verbindung lebt
  und `sandboxStatusProvider` sagt `agentRunning` (dieselbe Frage wie
  `SandboxTerminal`). Der erste Stand geht nach dem ersten Frame hinüber, nicht
  mit `fireImmediately`: Riverpod verbietet Schreiben während des Baus, und
  `initState` gehört dazu.
- **Ende des Agenten.** `GrpcDaemonClient` warf `SandboxEvent.Exit` weg, und
  der Daemon schickt danach kein `Status`; der Schnappschuss blieb bei „läuft"
  (Codex-Befund). Neu `SandboxUpdate.exit(code)`, im Notifier gefaltet zu
  `agentRunning: false`. Der Fake schließt den Strom des Starts schon bei
  `running` und schickt kein `Exit`; die Tests spielen den echten Ablauf mit
  einem eigenen Client nach (`SessionClient.agentEnd`), der Fake bleibt, wie
  er ist (gemeinsame Datei, und ein offener Start-Strom hielte jeden
  `start()`-Aufruf der übrigen Tests an).
- **Sitzungen von der Kommandozeile.** Der Daemon hat keinen Strom, der
  Sitzungen ankündigt. Solange die leere Warteschlange zu sehen ist
  (Intercept, Verbindung lebt, Terminal nicht im ganzen Fenster, nichts
  gehalten), fragt die Shell nach dem Schnappschuss: sofort beim Eintreten,
  wenn das Fenster nach vorn kommt (`AppLifecycleListener.onResume`), und alle
  `agentSessionPollInterval` (5 s). Dafür `pollStatus()` im
  Sandbox-Notifier: nur `Sandbox(Status)`, keine Isolationsprüfung. Sie nimmt
  nur den Schnappschuss, keine Befunde und keine Logzeilen (der Daemon
  wiederholt die Befunde seiner Vorabprüfung mit jedem Schnappschuss; sonst
  stünde alle 5 s eine Kopie mehr da, und `sandboxCallProvider` spräche sie
  der Frage zu). Sie erhöht `_snapshots` nicht (ein laufender Plan oder
  Refresh behält seine Antwort, eine spätere Antwort der Frage fällt weg).
  Jedes Ereignis eines anderen Aufrufs (Start, Stopp, `Exit`, Plan,
  Refresh) zählt `_sessionEvents` hoch, und eine Antwort, die vor dem
  letzten davon losging, fällt weg: Ein `Exit` ist neuer als ein
  Schnappschuss von davor. Sie faltet auf den Stand bei Ankunft statt auf
  den beim Fragen (sonst gingen Prüfergebnisse eines Starts dazwischen
  verloren), und ein Fehlschlag lässt den Schnappschuss stehen. Immer nur
  eine Frage zugleich, weitere Aufrufe schließen sich ihr an; nach
  `sandboxPollDeadline` (10 s) gilt sie als verloren, ihre Antwort fällt
  weg, und die nächste fragt neu. Ein `Stream.timeout` oder ein eigener
  `StreamController` als Frist ließ in den Widget-Tests das Ende des Stroms
  nie ankommen; deshalb ein Zähler (`_pollToken`) und ein Timer. Die
  abgelaufene Anfrage selbst bleibt offen, bis der Daemon sie beendet.
  Ein Zubringer je App: Eine zweite Shell im selben Prozess bleibt untätig
  (`AgentSessionFeed.idle`).
- **Ohne Verbindung** sagt die Warteschlange nicht „arbeitet" (Sonnet-Befund).
- **Leerzustand.** `QueueEmptyState` ist ein `ConsumerWidget`. Mit laufendem
  Agenten: Titel „The agent is working" / „Der Agent arbeitet", Hinweis
  „Every request that needs you appears here." / „Jede Anfrage, die dich
  braucht, erscheint hier.", Knopf `queue-go-to-terminal` „Go to terminal" /
  „Zum Terminal". Ohne: alles wie vorher. Die Tastenzeile steht in beiden.
  „Nothing waiting" ist nicht übernommen, die verbotenen Wörter aus
  `docs/UX.md` 4.1 kommen nicht vor.
- **Wohin der Knopf führt.** In den Sandbox-Abschnitt (`NavIntent`), nicht
  ins ganze Fenster: Leiste und Tabs bleiben, `Ctrl+1` führt zurück, und
  `Ctrl+Shift+Enter` führt von überall ins ganze Fenster.
  `sandboxSectionIndex = 3` steht in `core/shortcuts/intents.dart` neben
  `navigationKeys`, ein Test hält ihn an `Section.sandbox.index`.
- **Tests.** `test/features/intercept/empty_queue_session_test.dart`, zwölf
  Fälle über die ganze App: laufender Agent (Text, Knopf, Tastenzeile, kein
  Start, Knopf öffnet Sandbox), ohne Session (Start, öffnet Setup), Sandbox
  mit beendetem Agenten, Start und Stopp aus der App, ein Schnappschuss, der
  vor der Shell steht, `Exit` auf dem Start-Strom, Sitzung der
  Kommandozeile kommt und geht, keine Frage auf einem anderen Abschnitt und
  sofort eine beim Zurückkehren, Fenster kommt nach vorn, ohne Verbindung
  kein „arbeitet", unbeantwortete Frage lässt den Schnappschuss stehen,
  eine alte Antwort nach dem `Exit` weckt den Agenten nicht, drei Fragen mit
  wiederholtem Befund lassen einen Befund und seine Herkunft stehen, eine
  hängende Frage wird nicht verdoppelt und gibt nach der Frist auf, eine
  zweite Shell füttert nichts, Index. Dazu `test/core/ipc/grpc_client_test.dart` „the end of the agent on
  the start stream reaches the app" gegen einen echten gRPC-Server auf einem
  Unix-Socket.
- **Goldens.** Bestehende unverändert. Neu
  `intercept_empty_working_{dark,light}` in `intercept_golden_test.dart`, mit
  einem festen `WorkingAgent`.
- **Mutationsbeweise** (jede Datei danach byte-gleich zurück, mit `cmp`
  geprüft), alle 20 rot: Warteschlange ignoriert das Bit; Feed ohne
  Zuhörer am Schnappschuss; Feed ohne ersten Stand; `isUp` statt
  `agentRunning`; Knopf nach Setup; Index 2; Tastenzeile fehlt; `Exit` nicht
  gefaltet; keine wiederholte Frage; Frage auf jedem Abschnitt; keine Frage
  beim Eintreten; kein `onResume`; Verbindung ignoriert; Frage nicht leise
  (Sandbox-Ansicht wird rot); gRPC-Client wirft `Exit` weg; Frage achtet
  nicht auf `_sessionEvents`; Frage faltet Befunde; keine Bündelung; keine
  Frist; kein Schutz vor einer zweiten Shell.
- **Offen.** Der Zustand eines anderen Clients kommt höchstens 5 s später
  an. Ein Strom des Daemons, der Sitzungsänderungen meldet, ersetzte die
  Frage; Kandidat für ein eigenes Issue.

## HUM-301 · The terminal text can be zoomed

### Context
Owner report 2026-10-29: "ich kann den Text im Terminal von pi nicht mit Strg
größer machen". Every terminal emulator on the desktop zooms with `Ctrl` and
`+`/`-`; ours keeps the 13 px of `mono13` and hands those chords to the agent.
On a large screen or from some distance pi is hard to read.

### Specification
1. `Ctrl` + `+` (also `Ctrl+=` and numpad `+`) makes the terminal text one
   step larger, `Ctrl` + `-` (also numpad `-`) one step smaller, within 10 to
   28 px, starting at the size the terminal has today (13). A reset chord
   brings back the default. `Ctrl` + mouse wheel over the emulator zooms too,
   and the scrollback does not move with it.
2. The chords are bound in `shellShortcuts()` and take the way of the other
   application keys through `TerminalPane._onKey` (HUM-298, HUM-303): with
   the keyboard in the terminal they never reach the agent. They act only
   while a terminal with a session is shown; otherwise the key goes on to
   whoever else wants it. All other keys still go to the agent.
3. The reset chord collides with no binding
   (`app/lib/core/shortcuts/intents.dart`, intercept decision keys, editor,
   CONVENTIONS 3.9). `Ctrl+0` is taken by `ToggleRailIntent` (HUM-254)
   everywhere, also inside the terminal; when it collides, a free chord is
   chosen and the reason is written down.
4. The size survives a restart in the UI state file of HUM-254
   (`UiStateFile`, `app/lib/core/ui_state/ui_state_file.dart`). An unreadable
   or out-of-range value falls back to the default or the nearest bound.
5. After a zoom the emulator recomputes columns and rows, and the new
   geometry goes to the daemon through the existing throttled resize path
   (`terminal_provider.dart`, HUM-305), so the TUI redraws.
6. The command palette lists zoom in, zoom out and reset while a terminal is
   shown; `docs/DESIGN.md` shortcut table, `docs/UX.md` 5.2 and CONVENTIONS
   3.9 name the chords; labels in ARB (`en` source, `de` translation,
   appended).

### Acceptance criteria
1. Widget tests: with the terminal focused, the zoom-in, zoom-out and reset
   chords change the font size of the emulator, and none of them reaches the
   agent as a byte; another key still does.
2. Widget test: the three palette commands do the same, and they are listed
   only while a terminal is shown.
3. Widget test: `Ctrl` + wheel over the terminal zooms, the wheel alone does
   not.
4. Widget test: the size stops at 10 and 28.
5. Widget test: a zoomed size is there again after a restart over the same
   state file.
6. Widget test: after a zoom the daemon receives a resize with the new
   columns and rows.
7. Each behaviour has a mutation proof; `make flutter-analyze` and the tests
   of the touched packages are green; no golden changes.

### Fallstricke
- `Ctrl+0`, the reset of every browser and terminal, is the rail toggle here
  (HUM-254) and reaches the shell from inside the terminal as well.
- On a US layout `+` is `Shift+=`, on a German one `+` has its own key and
  `=` is `Shift+0`. The activators have to accept `Shift` for zoom in and
  must not mistake a German `Ctrl+Shift+0` for anything but the digit.
- `Ctrl+Shift+-` is `Ctrl+_`, undo in readline; zoom out takes `-` without
  `Shift` only.
- The emulator's `Scrollable` registers for every wheel event; a `Listener`
  around it is asked later in the hit-test path and loses the
  `PointerSignalResolver` race, so `Ctrl` + wheel would scroll the
  scrollback instead of zooming.

### Umsetzung (HUM-301)

- **Die Größe als Notiz im Kern.** `app/lib/core/ui/terminal_zoom.dart`
  hält `terminalFontSizeProvider` (Vorgabe 13 = `mono13`, Grenzen 10 und 28,
  Schritt 1) und schreibt sie unter `terminalFontSize` in die Zustandsdatei
  (`UiStateFile`); die Vorgabe wird als `null` geschrieben, damit eine
  spätere Vorgabe wieder greift. Gelesen wird einmal synchron beim ersten
  Bau, wie bei der Leiste: asynchron meldete das Terminal erst 13 px an den
  Daemon und sprünge einen Frame später. Ein Wert, den niemand so
  geschrieben haben kann (kein Zahlwert, außerhalb der Grenzen), fällt auf
  die Vorgabe oder die nächste Grenze zurück.
- **Die Griffe.** `TerminalZoomIntent(TerminalZoom)` und
  `TerminalZoomActivator` in `intents.dart`, in `shellShortcuts()`
  gebunden. Größer: `Ctrl` mit `=`, `+` (deutsche Taste) oder Ziffernblock-
  `+`, jeweils mit oder ohne `Shift`, denn das `+` der US-Belegung ist
  `Shift+=`. Kleiner: `Ctrl` mit `-` oder Ziffernblock-`-`, nur ohne `Shift`,
  weil `Ctrl+_` Rückgängig in readline ist. Größer und kleiner wiederholen
  sich beim Halten, die Vorgabe nicht. Genau `Ctrl`: mit `Alt` oder `Meta`
  nicht.
- **Vorgabe auf `Ctrl+Shift+0`, nicht auf `Ctrl+0`.** `Ctrl+0` ist seit
  HUM-254 die Leiste, in jedem Bereich und über `_onKey` auch aus dem
  Terminal. Dieselbe Taste je nach Fokus mit zwei Bedeutungen hätte dem,
  der die Leiste will, einen kleineren Text beschert und umgekehrt. Mit
  `Shift` bleibt die Vorgabe auf der Taste, nach der man greift; Flutter
  benennt die Taste nach ihrem Zeichen ohne Modifikatoren, also ist es auch
  auf deutscher Belegung (`Shift+0` = `=`) die Null und kein Zoom.
  Zusätzlich `Ctrl` mit der Null des Ziffernblocks, ohne `Shift`, weil
  `Shift` diese Taste bei Num Lock zu `Insert` macht.
- **Nur mit sichtbarem Terminal.** Die Shell bindet `TerminalZoomAction`,
  die nur aktiv ist, solange `_terminalShown()` gilt; eine inaktive Aktion
  lässt `Shortcuts` `ignored` antworten, die Taste geht unverändert weiter.
  Im Terminal nehmen die Kürzel den Weg aller Shell-Kürzel durch
  `TerminalPane._onKey` und erreichen den Agenten nie. Die Palette führt
  die drei Befehle, solange ein Terminal zu sehen ist, auch über einer
  Ebene, denn die Größe braucht keinen Fokus.
- **Das Rad.** `_WheelZoom` in `terminal_pane.dart`: ein `RenderProxyBox`,
  der sich im Trefferpfad **vor** seine Kinder setzt und so beim
  `PointerSignalResolver` als erster meldet, nur für ein Rad mit `Ctrl`.
  Ein `Listener` hätte verloren: Das `Scrollable` des Rücklaufs (und für
  ein TUI das, das Mausberichte erzeugt) steht tiefer im Pfad und meldet
  sich zuerst.
- **Neue Geometrie.** Kein eigener Weg: Die neue Schrift ändert die Zellen,
  `RenderTerminal` zählt Spalten und Zeilen neu und ruft `terminal.resize`,
  und die Sitzung schickt das über den gedrosselten Pfad von HUM-305.
- Tests: `test/features/shell/terminal_zoom_test.dart` (Kürzel im
  fokussierten Terminal ohne Byte beim Agenten, `Enter` als Gegenprobe;
  `Ctrl+0` bleibt die Leiste, `Ctrl+Shift+-` zoomt nicht; ohne Terminal tun
  die Kürzel nichts; Palette nur mit Terminal; `Ctrl` + Rad mit Rücklauf im
  Emulator, Rad allein nicht; Grenzen 10 und 28; Neustart über dieselbe
  Datei; Resize mit weniger Spalten und Zeilen beim Daemon),
  `test/core/ui/terminal_zoom_test.dart` (Vorgabe = `mono13` in allen
  Themen, Lesen mit Rückfall, Schreiben mit `null` für die Vorgabe,
  Aktivatoren: genau `Ctrl`, `Ctrl+0` nicht, Wiederholung nur beim Zoom),
  `shortcuts_map_to_intents_test.dart` um die zehn Zoom-Zeilen ergänzt. Der
  Tastensimulator kennt für `add`, Ziffernblock-`-` und Ziffernblock-`0`
  keinen Linux-Code; diese Tasten kommen im Test als Android-Ereignis, die
  Shortcuts sehen nur die logische Taste.
- Mutationen, je rot und mit `cmp` byte-gleich zurück: Zoom nicht in
  `shellShortcuts()`; `_onKey` überspringt den Zoom; Aktion immer aktiv;
  Rad-Box nach den Kindern im Trefferpfad; ohne Begrenzung; ohne
  Schreiben; Palette listet den Zoom immer; Schrift wieder aus `mono13`;
  Vorgabe auf `Ctrl+0`; kleiner auch mit `Shift`; Vorgabe wiederholt sich;
  `Alt` erlaubt; Bau liest einen anderen Schlüssel. Eine Prüfung in `_onKey`
  auf eine inaktive Aktion wurde erwogen und weggelassen: Ein Emulator mit
  der Tastatur ist immer zu sehen, der Fall ist unerreichbar und ließe sich
  durch keinen Test belegen.
- **Nach dem Review: gehaltene Griffe.** Codex und Sonnet fanden, dass
  das Loslassen jedes Zoom-Griffs und die Wiederholungen der Vorgabe nicht
  vom Aktivator angenommen werden und deshalb zum Emulator durchfielen; ein
  Agent mit dem Tastaturprotokoll von kitty hätte so die zweite Hälfte eines
  Griffs bekommen. `TerminalPane` merkt sich jetzt die physische Taste
  eines genommenen Zoom-Griffs (`_heldZoomKey`) und behält jedes weitere
  Ereignis dieser Taste bis zum Loslassen; die Vorgabe gilt dabei nur
  einmal. Ein neues Drücken irgendeiner Taste löscht die Merkung, falls das
  Loslassen nach einem Fokuswechsel woanders ankam. Test
  `a_held_zoom_chord_never_falls_through_to_the_agent` (`Ctrl+=` und
  `Ctrl+Shift+0` mit Drücken, drei Wiederholungen, Loslassen: jedes
  Ereignis behandelt, kein Byte beim Agenten, Vorgabe einmal). Mutationen,
  je rot und byte-gleich zurück: ohne Behalten; Merkung schon nach der
  ersten Wiederholung vergessen; Vorgabe wiederholt sich. Nicht übernommen
  (optional, minor): das Schreiben in die Zustandsdatei während eines
  gehaltenen Zooms zu entprellen; es ist ein kleines JSON je Schritt, und
  die Wiederholrate der Tastatur begrenzt es.
---
## HUM-302 · Notices float as dismissible overlays and never push the layout

### Context
Owner report of 2026-10-29, two complaints about the same thing.

1. The finding "LLM endpoint is not on a private network" (`LLM_006`) has a
   band of its own at the top of the Sandbox section, is oddly tall, and
   makes the terminal re-wrap ("macht mir mein Terminal so witzig"). The
   daemon puts `LLM_006` into every sandbox status while the endpoint is
   public; the band stood between the sandbox header and the terminal, so
   each status that brought it or took it away resized the PTY and the TUI
   of the agent redrew. It could not be closed.
2. In Intercept, the cards above the queue (findings of the daemon,
   HUM-106, and the requests of the agent, HUM-073) are not clean overlays:
   the close glyph of a finding sits beside the card instead of on it, which
   leaves an empty column on the right, and every card pushes the queue list
   down.

The desktop notice of the shell (`UI_002`, HUM-276) and the return banner
(HUM-034) are bands of the same kind, directly under the header, and push
every pane.

### Specification
1. One shared overlay pattern in `app/lib/core/ui` (`HNoticeLayer`): notices
   float above the content in a `Stack`, never in its `Column`, right-aligned
   at the top of their region: the window below the header row of the panes
   for global notices (`UI_002`, the return banner), the area under the
   sandbox header for the findings of the Sandbox, the list area of the queue
   column for the queue cards. The width is the region minus its inset on
   both sides, at most 420 px; cards stack with the 8 px gap of the grid;
   beyond a bounded height they scroll.
2. The content keeps its full size whether a notice stands or not; a
   running terminal is not resized when a notice appears or disappears. The
   content keeps its element across arrivals and dismissals.
3. Every card has its close affordance inside the card, top right, a 28 px
   hit target with its label on hover (`HNoticeDismiss`). Details, fixes and
   "Open request" stay on the card.
4. A finding that stands, as opposed to an event, stays closed while it is
   unchanged: the dismissal is keyed by the code plus a fingerprint of its
   subject (code and cause; the cause names the endpoint of `LLM_006`), kept
   in `ui-state.json`, and the card comes back when the subject changes. The
   `showOnce` memory of HUM-276 for `UI_002` keeps working unchanged.
5. Notices are passive: no autofocus, no `HLayerScope`, no shortcut, and
   `Esc` closes none of them. They never cover the Intercept action bar: the
   global notices start below the header row of the panes, at the top right,
   and the queue cards stay in the queue column.
6. Surfaces follow `docs/DESIGN.md` (HUM-286): `bg2`, strong hairline, card
   radius, no shadow, `H*` widgets only. ARB keys are appended only.

### Acceptance criteria
1. A widget test shows a global notice without moving the queue pane, and a
   Sandbox finding without changing the size of the terminal; the terminal
   sends no resize when the finding comes or goes.
2. A widget test closes a Sandbox finding with its close glyph; it stays
   closed across a new status with the same finding and over a new screen
   on the same state file, and comes back when its subject changes.
3. A widget test shows the queue cards right-aligned over the queue column,
   the first row of the list at the same place with and without them, and
   the close glyph inside the card.
4. A widget test shows that the arrival of a notice leaves the keyboard focus
   where it was.
5. Every new test has a mutation proof; only goldens the change alters are
   regenerated.

### Umsetzung (HUM-302)

- **Fundorte.** Vier Bänder: `ShellNotices` (`UI_002` und Rückkehr-Banner)
  unter der Kopfzeile der Shell; `_Diagnostics` im Sandbox-Bildschirm
  zwischen Kopf und Terminal (dort stand `LLM_006`, das der Daemon in jeden
  Sandbox-Status legt, solange der Endpunkt öffentlich ist); `AgentAskStrip`
  und `DiagnosticStrip` über der Liste der Queue. Das eingefrorene Banner
  (`FrozenBanner`) bleibt ein Band: Es ist ein Zustand der Verbindung, kein
  wegklickbarer Hinweis.
- **Gemeinsames Muster** in `app/lib/core/ui/h_notice_layer.dart`:
  `HNoticeLayer` legt die Hinweise in einem `Stack` über die Region, oben
  rechts, 8 px Abstand, Breite Region minus Abstand, höchstens
  `noticeMaxWidth` (420 px); Inhalt immer erstes, Hinweise immer zweites
  Kind, auch ohne Hinweis, damit das Element des Inhalts (Terminal) jede
  Ankunft übersteht. `HNoticeCard` (bg2, `lineStrong`, Kartenradius,
  optionale Zustandsschiene, kein Schatten), `HNoticeDismiss`
  (`HIconButton` 28 px mit `HoverLabel`, 4 px aus der Ecke, in der Karte),
  `HNoticeColumn` (8 px Abstand). Diagnosen bleiben `HDiagnosticCard`, mit
  zwei neuen optionalen Parametern: `dense` (12 px Innenabstand, Titel 13/600,
  Grund 12) und `dismiss` (Slot oben rechts in der Karte, Text hält Abstand),
  dazu `detailMaxLines`. So bleiben alle bestehenden Tests auf
  `HDiagnosticCard` gültig. Beschrieben in `docs/DESIGN.md`.
- **Orte.** Shell: `HNoticeLayer` um den Bereich zwischen Kopfzeile und
  Statusleiste, `top` = `HSize.paneHeader` + 8, damit kein Hinweis die
  Kopfzeilen der Panes (Start/Stopp, Schließen des Blatts) und nie die
  Aktionsleiste von Intercept verdeckt. Sandbox: über Terminal und Reitern,
  höchstens ein Drittel der Höhe, dann rollt es. Queue: über der Liste,
  höchstens `interceptStripsMaxHeight`, der Befund-Streifen rollt wie bisher
  selbst (`scroll: false`). Die Karten der Agent-Bitten tragen jetzt
  `HNoticeCard` (Rahmen ringsum, Schließen in der Ecke, `ClipRect` bleibt).
- **Schließen mit Gedächtnis.** `app/lib/core/ui_state/dismissed_findings.dart`:
  Schlüssel `findingDismissed.<code>` in `ui-state.json`, Liste von
  Fingerabdrücken (FNV-1a, 32 Bit, über Code und Grund; der Grund nennt den
  Gegenstand, bei `LLM_006` den Endpunkt), höchstens 8 je Code. Gilt für die
  stehenden Befunde des Sandbox-Bildschirms (siehe Review-Runde). Der Satz
  selbst landet nicht in der Datei.
  `UI_002` behält `showOnce` und `noticeDismissed.UI_002` unverändert; die
  Befunde der Queue sind Ereignisse und gehen weiter je Eintrag weg.
- **Tastatur.** Kein Autofokus, kein eigener Fokusbereich, keine Kürzel,
  `Esc` schließt nichts. Die Karten sind gewöhnliche Fokushalte; der globale
  Hinweis ist jetzt mit `Tab` nach den Splittern erreichbar statt mit
  `Shift+Tab` (Test angepasst).
- **Tests.** Neu: `test/core/ui/notice_layer_test.dart` (Region behält
  Größe, Zustand und Fokus; rechtsbündig, 420 px, 8 px Abstand; Schließen in
  der Karte, 28 px, Beschriftung beim Überfahren; ohne Hinweis gehört die Ecke
  der Region), `test/features/sandbox/notice_overlay_test.dart` (Terminal
  behält Rechteck, kein `TerminalResize` bei Kommen und Gehen; Schließen
  bleibt über neuen Status und neuen Bildschirm auf derselben Datei, nur der
  eine von zwei Befunden geht, anderer Endpunkt bringt ihn zurück, Datei
  trägt Fingerabdruck, nicht den Satz; Fokus bleibt im Terminal),
  `test/features/intercept/queue_notices_test.dart` (erste Zeile bleibt
  stehen, Karte rechtsbündig unter dem Queue-Kopf, Schließen in der Karte,
  Fokus bleibt). Angepasst: `attention_notice_test` (schwebt oben rechts,
  Leiste und Queue beginnen an der Kopfzeile und bleiben nach dem Schließen
  stehen; Kontrast auf `bg2`; Tastatur), `return_banner_test` (Schließen ist
  `HIconButton`).
- **Mutationsbeweise** (je Datei byte-gleich zurückgestellt, mit `cmp`
  geprüft): Sandbox-Befunde wieder als Band über dem Terminal: zwei rot
  (Terminalgröße, Schließen). Shell-Hinweise wieder in der Spalte: drei rot.
  Queue-Streifen wieder über der Liste: rot. Fingerabdruck nur aus dem Code:
  rot (anderer Endpunkt kommt nicht wieder). Filter der geschlossenen Befunde
  entfernt: rot. Ein Hinweis, der bei Ankunft den Fokus weiterreicht: drei
  rot (Layer, Sandbox, Queue). Schließen ohne Schreiben in die Datei: rot.
- **Goldens neu (16, nur die veränderten):**
  `attention_notice_banner.{dark,light}-{de,en}` (Rahmen jetzt die Karte in
  436 × 260), `intercept_diagnostic_tls_{dark,light}`,
  `sandbox_header_failed_{dark,light}`,
  `states_sandbox_failed.{dark,light}-{de,en}`,
  `states_tls_rejected.{dark,light}-{de,en}`.
- **Review-Runde (Sonnet).** Übernommen: (1, major) Dauerhaft geschlossen
  wird nur, was steht. Befunde der Schwere Fehler oder blockierend im Status
  sind Fehlschläge der letzten Operation und werden nur im Speicher
  geschlossen (`features/sandbox/providers/closed_failures.dart`); das
  Gedächtnis endet, sobald die Sandbox eine neue Operation beginnt (Übergang
  nach `starting`/`stopping`) oder der Befund nicht mehr im Status steht. Ein
  fehlgeschlagener Ordnerdialog (`UI_003`) gilt nur als genau dieses Objekt
  geschlossen; der nächste Fehlschlag ist ein neues. Nichts davon landet in
  `ui-state.json`. (2) Queue-Test auch mit einer Bitte des Agenten. (3) Die
  Fokustests stellen den Fokus vorher auf eine Zeile bzw. das Terminal und
  prüfen, dass er steht. (4) In `HNoticeCard` und `HDiagnosticCard` weicht nur
  noch die erste Zeile (Abzeichen bzw. `header`) dem Schließen aus
  (`noticeDismissClearance` = 4 + 28 + 8 px: Knopf mit Fokusring 32 px plus
  4 px Abstand); Titel und Text darunter laufen bis an die Innenkante.
  `HNoticeCard` hat dafür `header` und `child`. (5) Solange das
  Einstellungsblatt offen ist, zeigt die Shell keine Hinweise; sie bleiben im
  Provider und erscheinen nach dem Schließen wieder. Abgefragt wird
  `_settingsOpen` der Shell und nicht `HLayerScope.holdsFocus()`, weil diese
  statische Abfrage keinen Neubau auslöst.
- **Weitere Mutationsbeweise** (byte-gleich zurückgestellt): Klassifizierung
  „alles steht" (`isOperationFailure` immer false): rot (zweiter Fehlstart
  bleibt unsichtbar, Datei bekommt einen Eintrag). Kein Zurücksetzen bei
  neuer Operation: rot. Hinweise auch bei offenem Blatt: rot. Erste Zeile
  ohne Abstand zum Schließen: rot.
- **Review-Runde (Codex).** Übernommen: (1) Die Karten der Queue stehen
  unten rechts in der Spalte (`HNoticeLayer.bottom`); die obersten Zeilen
  mit ihrem Blockieren bleiben greifbar, ein Zeiger auf der ersten Zeile
  trifft die Zeile. Damit verdecken sie auch die Pille „neue Anfragen" oben
  nicht mehr. Den Abstand zwischen Bitten und Befunden setzt der `QueuePane`
  (`_StripGap`), nur wenn beide stehen. (2) Geschlossene Fehlschläge
  vergisst die Sandbox, sobald eine neue Operation gerufen wird (Plan,
  Start, Stopp, Ordnerwahl: `closedFailuresProvider.newOperation()` in
  `_apply`), nicht erst bei `starting`; eine Abfrage des Status behält sie.
  Ein Start, der vor `starting` abgelehnt wird, zeigt seinen Fehlschlag beim
  nächsten Versuch wieder. (3) Der Abstand der Shell-Hinweise nach oben ist
  `HPaneHeader.rowHeight(context)` plus 8 px, also 96 + 8 bei doppelter
  Schrift. (4) Über dem Terminal werden die Befunde höchstens so hoch, dass
  unten zwei Zeilen des Emulators und sein Innenabstand frei bleiben
  (`sandboxTerminalFreeRows`, aus Aufteilung und Mindesthöhen errechnet wie
  in `HResizablePanes`), auch wenn das Terminal auf 160 px gezogen ist.
  (5) Die Shell-Hinweise warten, solange irgendeine Ebene offen ist:
  `HLayerScope.openLayers` (neu, `ValueListenable<int>`, nach dem Frame
  aktualisiert) zählt Blätter, Modals und Menüs, darunter das Regelblatt
  einer Bitte.
- **Mutationsbeweise dieser Runde** (byte-gleich zurückgestellt): Karten der
  Queue wieder oben: rot. Kein `newOperation()`: rot. Fester Abstand 56 px:
  rot (doppelte Schrift). Keine Grenze über dem Terminal: rot. Nur
  `_settingsOpen` statt offener Ebenen: rot (Regelblatt).
- **Hinweis zur Prüfung.** Ein Zwischenstand ließ `DiagnosticStrip`
  `agentAsksProvider` beobachten; der Streifentest ohne Fake-Client öffnete
  darauf einen Abonnement-Versuch gegen den echten Daemon-Socket, der
  scheiterte. Zurückgenommen, der Abstand steht jetzt im `QueuePane`.
- **Offen.** Keins aus diesem Issue.

## HUM-313 · Terminal copy and paste

### Ziel
Wunsch des Owners 2026-10-30: Kopieren und Einfügen im Terminal der Sandbox
wie in jedem Terminal unter Linux. Bisher kopierte `Ctrl+Shift+C` (Vorgabe
des Emulators), aber eingefügt wurde mit dem nackten `Ctrl+V`, das in
readline das wörtliche Einfügen des nächsten Zeichens ist (`0x16`) und dem
Agenten gehört; `Ctrl+Shift+V` kam als `0x16` beim Agenten an. Die mittlere
Maustaste tat nichts, das Kontextmenü kannte kein „Alles auswählen".

Beim Lesen fiel eine Lücke auf, die die Sicherheitsaussage betrifft:
`TerminalView` aus `xterm2` setzt eigene Handler für OSC 52, sobald das
Terminal keine hat (`??=`), und schreibt und **liest** damit die
Zwischenablage des Menschen, solange das Terminal den Fokus hat. Der Filter
des Daemons lässt OSC 52 nicht durch, also war die Lücke nicht erreichbar;
`docs/SECURITY.md` 3.3 sagt aber, das Widget registriere keinen OSC-Handler,
und das stimmte nicht. Dieses Issue schließt die zweite Wand.

### Akzeptanzkriterien
1. Mit der Tastatur im Terminal kopiert `Ctrl+Shift+C` die Auswahl in die
   Zwischenablage, ohne ein Byte an den Agenten. Ohne Auswahl tut der Griff
   nichts, auch dann kein `0x03`.
2. `Ctrl+Shift+V` fügt die Zwischenablage ein. Das nackte `Ctrl+C` bleibt
   `0x03` (SIGINT), auch mit Auswahl, und kopiert nie; das nackte `Ctrl+V`
   bleibt `0x16`. Gehaltene Griffe fügen einmal ein, Wiederholungen und
   Loslassen erreichen den Agenten nicht.
3. Die Griffe wirken nur, solange das Terminal die Tastatur hat. Keine
   Bindung der Anwendung (`shellShortcuts`, Intercept, Befundpause, Editor,
   Verlauf, Regelliste) feuert auf `Ctrl+Shift+C` oder `Ctrl+Shift+V`. Auf
   Apple-Plattformen sind es `Cmd+C` und `Cmd+V`.
4. Einfügen klammert (`ESC [200~` … `ESC [201~`), wenn das Programm
   `DECSET 2004` gesetzt hat; ein mehrzeiliger Text läuft dann nicht Zeile
   für Zeile. Ein `ESC [201~` im eingefügten Text (und jede andere
   Escape-Folge) fällt weg und beendet die Klammer nie früh.
5. Ein Leser (`readOnly`) und eine nicht verbundene Sitzung fügen nicht ein,
   weder per Griff noch per Menü noch per Mittelklick.
6. Die mittlere Maustaste fügt ein, solange das Programm die Maus nicht
   selbst nimmt (Mausberichte; mit `Shift` gilt die Regel des Emulators).
7. Rechtsklick öffnet das Menü mit Kopieren (deaktiviert ohne Auswahl),
   Einfügen und Alles auswählen; Kopieren und Einfügen nennen ihren Griff.
   Das Menü ist das vorhandene `HContextMenu`, eine Ebene mit
   `HLayerScope.claimKeyboard` (HUM-303).
8. OSC 52 aus der Ausgabe des Agenten schreibt die Zwischenablage nicht und
   liest sie nicht, auch am Filter des Daemons vorbei und mit Fokus im
   Terminal; ebenso `OSC 1337 CopyToClipboard`. Kopiert wird nur auf eine
   ausdrückliche Handlung des Menschen.
9. UI-Texte über ARB (`en` Quelle, `de` Übersetzung, angehängt). Keine neue
   Abhängigkeit in `app/pubspec.yaml`.

### Tests
`test/features/sandbox/terminal_clipboard_test.dart`, als Linux:
`ctrl_shift_c_copies_the_selection_and_sends_nothing_to_the_agent`,
`ctrl_c_with_a_selection_still_interrupts_the_agent_and_copies_nothing`,
`ctrl_shift_v_pastes_and_plain_ctrl_v_reaches_the_agent`,
`a_held_clipboard_chord_never_falls_through_to_the_agent`,
`a_paste_is_bracketed_when_the_program_asked_and_201_is_stripped`,
`a_reader_cannot_paste_by_chord`,
`the_chords_do_nothing_while_the_terminal_lacks_the_keyboard`,
`the_agent_cannot_write_or_read_the_clipboard_through_osc_52`,
`the_middle_button_pastes_unless_the_program_takes_the_mouse`,
`the_menu_offers_copy_paste_and_select_all`.
`test/core/shortcuts/terminal_clipboard_activator_test.dart`: genau
`Ctrl+Shift` unter Linux, genau `Cmd` auf macOS, und keine Tabelle der
Anwendung feuert auf die Griffe. Jeder Test mit Mutationsbeweis.

### Fallstricke
- **Primäre Auswahl gibt es in Flutter nicht.** `Clipboard` kennt nur
  `text/plain` der Zwischenablage; die GTK-Einbettung unter Linux spricht
  `GDK_SELECTION_CLIPBOARD` an, nie `GDK_SELECTION_PRIMARY`, und unter
  Wayland fehlt das Protokoll ganz. Ein Plugin dafür wäre eigener Code über
  X11 und `zwp_primary_selection`, für eine Bequemlichkeit; das lohnt keine
  neue Abhängigkeit. Deshalb kopiert das Auswählen mit der Maus **nichts**
  (in die Zwischenablage gehört nicht jedes Ziehen der Maus, und „kopiert
  wird nur auf ausdrückliche Handlung" bleibt wahr), und der Mittelklick
  fügt die Zwischenablage ein. Kandidat für ein eigenes Issue, falls Flutter
  die primäre Auswahl je anbietet.
- `TerminalView` prüft seine eigenen Kürzel vor jeder Übersetzung in Bytes,
  und seine Vorgabe bindet Einfügen auf `Ctrl+V` und fragt dabei nicht nach
  `readOnly`. Die Tabelle des Emulators ist deshalb leer (`null` hieße
  „Vorgabe"); die Griffe fragt `TerminalPane._onKey` selbst.
- Ein Griff, den die Anwendung nicht nimmt, meldet der Emulator einem
  Agenten mit dem Tastaturprotokoll von kitty als eigene Folge (`CSI 99;6 u`).
  Der Griff gehört der Anwendung deshalb auch dann, wenn er nichts tun kann.
  (Ohne dieses Protokoll schickt der Emulator für `Ctrl+Shift+C` nichts;
  die erste Fassung dieses Abschnitts behauptete `0x03`, das hat die
  Mutation in Review-Runde 1 widerlegt.)
- Die Bereinigung beim Einfügen macht `Terminal.paste` von `xterm2`
  (Escape-Folgen weg, Steuerbytes wie `0x03` und `0x1a` zu Leerzeichen,
  Klammer bei `DECSET 2004`). `Terminal.textInput` täte nichts davon; der
  Test misst das Ergebnis, nicht die Bibliothek.
- Die OSC-52-Handler von `TerminalView` greifen nur, wo das Terminal keine
  hat. Die Ablehnung muss deshalb in der Sitzung gesetzt sein, bevor die
  Ansicht gebaut wird, also in `TerminalSession.build`.
- Die Plattform: `flutter test` spielt Android, und dort wären die Griffe
  dieselben wie unter Linux. Die Tests laufen trotzdem als Linux, damit der
  Zweig für Apple nicht zufällig mitläuft.

### Umsetzung (HUM-313)

- **Griffe.** `TerminalClipboard` und `TerminalClipboardActivator` in
  `app/lib/core/shortcuts/intents.dart`: `Ctrl+Shift+C`/`Ctrl+Shift+V`, genau
  diese Modifikatoren, auf Apple `Cmd`. Nicht in `shellShortcuts()`, sondern
  von `TerminalPane._onKey` nach den Shell-Kürzeln gefragt, also nur mit der
  Tastatur im Terminal. Die physische Taste eines genommenen Griffs merkt
  sich die Ansicht wie beim Zoom (`_heldChordKey`, vorher `_heldZoomKey`)
  und behält Wiederholungen und Loslassen.
- **Emulator ohne eigene Kürzel.** `_emulatorShortcuts` ist leer; damit gehen
  `Ctrl+A` (wie seit HUM-042) und jetzt auch `Ctrl+V` an den Agenten.
  **Verhaltensänderung:** Wer bisher mit `Ctrl+V` eingefügt hat, nimmt jetzt
  `Ctrl+Shift+V`, das Menü oder die mittlere Taste.
- **Einfügen** über `Terminal.paste`, zweimal gegen `readOnly` und die Phase
  geprüft: vor dem Lesen der Zwischenablage und nach ihrer Antwort.
  **Kopieren** über `selectionFor` und `getText(…, true)`, also ohne die
  Leerzeichen am Zeilenende, mit denen der Emulator auffüllt.
- **Mittelklick.** Ein `Listener` um den Emulator; er fügt ein, wenn
  `mouseMode` `none` ist oder `Shift` gehalten wird und das Programm `Shift`
  nicht ebenfalls beansprucht (`mouseShiftCaptureMode`), dieselbe Regel, mit
  der der Emulator seine eigenen Klicks verteilt.
- **Menü.** „Alles auswählen" (`SelectionMode.line` über den ganzen Puffer)
  dazu, Kopieren und Einfügen mit Griff (`sandboxTerminalCopyKey`,
  `sandboxTerminalPasteKey`, auf Apple ohne Anzeige).
- **OSC 52.** `TerminalSession.build` setzt `onClipboardStore` auf eine
  Funktion, die nichts tut, und `onClipboardQuery` auf eine, die `null`
  liefert; `TerminalView` setzt dann keine eigenen. `docs/SECURITY.md` 3.3
  und `docs/THREAT-MODEL.md` K-09 nennen das jetzt.
- **Review-Runde 1 (Codex, Opus).** Alle Befunde übernommen.
  - Codex: Jeder neue Tastendruck löschte die Merkung des gehaltenen
    Griffs. `Ctrl+Shift+V` halten, eine andere Taste drücken, `V` loslassen:
    das Loslassen ging an den Emulator und damit an einen Agenten mit dem
    Loslass-Bericht von kitty. Jetzt eine Menge `_heldChordKeys`, je Taste bis
    zu ihrem eigenen Loslassen; geleert, wenn das Terminal die Tastatur
    verliert, und ein neues Drücken einer Taste darin gilt als neues Drücken.
    Test `a_chord_key_released_after_another_press_stays_with_the_application`.
    Das Leeren beim Fokusverlust hat keinen eigenen Test: Was es abdeckt,
    deckt im erreichbaren Fall schon das Entfernen beim nächsten Drücken ab.
  - Opus: `_paste` hält die Sitzung fest, in die eingefügt wurde, und fügt
    nach der Antwort der Zwischenablage nur ein, wenn noch dieselbe Sitzung
    gezeigt wird (der Zustand überlebt einen Wechsel der Sandbox). Ohne
    eigenen Test: Ein Wechsel der Sitzung im Test braucht einen zweiten
    Sandbox-Zustand im Fake, den es nicht gibt.
  - Opus: „Kopieren" im Menü fragt wie `_copy` nach `selectionFor` des
    aktuellen Puffers; eine Auswahl des anderen Bildschirms zählt nicht
    (`copy_is_disabled_for_a_selection_of_the_other_screen`).
  - Opus: `docs/SECURITY.md` 3.3 und K-09 genau: kein Handler für die
    Zwischenablage; Farbabfragen (OSC 4, 10, 11, `CSI ? 996 n`) beantwortet
    `TerminalView` mit den Farben des Themas, nur hinter dem Filter.
  - Opus, Tests: `ctrl_shift_c_without_a_selection_sends_nothing`,
    `a_reader_or_a_session_that_is_not_attached_cannot_paste_by_menu_or_click`,
    `a_reader_by_the_time_the_clipboard_answers_does_not_paste`,
    `shift_middle_click_pastes_past_mouse_reports_unless_the_program_claims_shift`.
  - Mutationen, je rot und byte-gleich zurück: eine Merkung statt Menge;
    zweite Prüfung in `_paste` weg; Menü fragt `selection` statt
    `selectionFor`; Kopier-Griff ohne Auswahl fällt durch; Phase nicht
    geprüft; `mouseShiftCaptureMode` übergangen; `Shift` übergangen. Der
    Kopier-Griff ohne Auswahl wird mit eingeschaltetem kitty-Protokoll
    geprüft, sonst schickt der Emulator ohnehin nichts. Gleichwertig und
    deshalb ohne roten Test: „Kopieren" im Menü mit `selection` statt
    `selectionFor`, denn der Emulator löscht die Auswahl selbst, sobald er
    den Puffer wechselt (`RenderTerminal._onTerminalChange`); der Test
    `copy_is_disabled_for_a_selection_of_the_other_screen` hält das Verhalten
    trotzdem fest.
- **Review-Runde 2 (Codex).** Übernommen. Das Leeren von `_heldChordKeys`
  beim Fokusverlust (aus Runde 1) ließ das Loslassen wieder durch:
  `Ctrl+Shift+V` halten, Fokus geht weg, der Zeiger über dem Terminal holt
  ihn zurück (HUM-303), dann `V` loslassen. Das Leeren ist weg; jede Taste
  bleibt bis zu ihrem eigenen Loslassen, ein verpasstes Loslassen räumt das
  nächste Drücken derselben Taste. Test
  `a_chord_key_released_after_the_focus_came_back_stays_with_the_application`;
  Mutation (Leeren beim Fokusverlust wieder eingebaut) rot.

### Kandidaten (HUM-313)
- (A) Farbabfragen in `TerminalSession.build` beantworten oder ablehnen,
  damit „kein OSC-Handler" wörtlich gilt.
- (B) Rückfrage vor einem mehrzeiligen Einfügen über `Terminal.isPasteSafe`,
  solange `DECSET 2004` aus ist.
- (C) `Shift+Insert` fügt ein.

---
## HUM-300 · Terminal as picture-in-picture

### Ziel
Wunsch des Owners 2026-10-30: Während ein Agent läuft, schwebt das Terminal
als kleines Fenster über den anderen Abschnitten (Intercept, History, Rules,
Audit, Setup, Einstellungen), damit der Owner dem Agenten zusieht, während er
dessen Anfragen entscheidet. HUM-298 hat dafür den Platz gelassen: „eine
schwebende Ebene (HUM-300) wird ein dritter Wert in
`terminalPlacementProvider` und ein dritter Wirt in der Shell", HUM-303 hat
die Tastaturregel für diesen Wirt vorgedacht („eigener `FocusScope` nur unter
dem Bereich der Shell, kein `HLayerScope`") und ``Ctrl+` `` ohne sichtbares
Terminal ausdrücklich für das schwebende Terminal freigehalten.

### Spezifikation
1. **Dritte Platzierung.** `TerminalPlacement.floating` neben `pane` und
   `fullWindow`. Genau ein Wirt zeigt die Sitzung: schwebt das Terminal, bleibt
   das Pane der Sandbox leer (mit „Terminal hierher zurückholen"), auch wenn
   der Abschnitt Sandbox gezeigt wird. Emulator und Rücklauf leben weiter in
   `terminalSessionProvider`; der Wechsel öffnet die Sitzung nicht neu.
2. **Umschalten.** Kürzel `Ctrl+Shift+P` (`ToggleTerminalFloatIntent` in
   `shellShortcuts()`, ohne Wiederholung), ein Knopf links vom Vollfenster-Knopf
   im Streifen über dem Terminal (Tooltip nennt das Kürzel), ein Befehl in der
   Palette. Ohne laufenden Agenten tut das Kürzel nichts, der Knopf ist
   deaktiviert und die Palette listet den Befehl nicht; ein schwebendes
   Terminal lässt sich immer zurücklegen. ``Ctrl+` `` mit laufendem Agenten
   und ohne sichtbares Terminal lässt es schweben und gibt ihm die Tastatur.
   Das ganze Fenster (`Ctrl+Shift+Enter`) kehrt zum schwebenden Fenster
   zurück, wenn es von dort kam.
3. **Begründung des Kürzels.** `P` wie Picture-in-Picture, in der Familie
   der `Ctrl+Shift`-Griffe, die das Terminal bewegen. Geprüft gegen jede
   Bindung in `app/lib`, `app/packages` und CONVENTIONS 3.9: `Ctrl+Shift+Enter`
   ganzes Fenster, ``Ctrl+` `` Fokus, `Ctrl` mit `+`/`-`/`=` und
   `Ctrl+Shift+0` Zoom, `Ctrl+0` Leiste, `Ctrl+Shift+C`/`V` Kopieren und
   Einfügen, `Ctrl+Shift+F`/`L` Gruppenentscheidungen, `Ctrl+F`/`L`/`Enter`
   Einzelentscheidungen, `Ctrl+K` Palette, `Ctrl+,` Einstellungen,
   `Ctrl+1..6` Abschnitte, `Ctrl+R` und `Ctrl+Enter` im Editor. Das nackte `P`
   ist „Pseudonymisieren" in der Befundpause; `SingleActivator` verlangt seine
   Modifikatoren genau, also trifft es `Ctrl+Shift+P` nicht. Der Agent verliert
   nichts: Ein Terminal unter Linux schickt `Ctrl+Shift+P` als `0x10` wie
   `Ctrl+P`, und `Ctrl+P` bleibt beim Agenten.
4. **Das Fenster.** Es liegt nur über den Abschnitten, nie über Leiste,
   Kopf- oder Statusleiste; Einstellungsblatt und Palette liegen darüber.
   Titelleiste zum Ziehen mit „Zurück zur Sandbox-Seite" (legt zurück und
   zeigt den Abschnitt) und „Schließen" (legt zurück, bleibt auf dem
   Abschnitt; dasselbe wie das Kürzel). Losgelassen rastet es in die Ecke des
   Quadranten ein. Ein Griff in der inneren Ecke ändert die Größe zwischen
   320 x 200 und 60 % der Abschnitte (höchstens 960 x 640). Ecke und Größe
   überleben einen Neustart in der Zustandsdatei (`UiStateFile`,
   `terminalFloatCorner`, `terminalFloatSize`).
5. **Nie über einer Entscheidung.** Vorgabe ist die Ecke unten rechts: Auf
   Intercept ist das der untere Rand des Kontext-Panes, die einzige Ecke
   ohne Entscheidung (links die Warteschlange mit Zeilen und Hinweisen unten
   rechts in ihrer Spalte, in der Mitte unten die Leiste mit Freigeben und
   Blockieren, oben rechts die Hinweise der Shell). Ausweichregel: Was frei
   bleiben muss, markiert sich selbst mit `HKeepClear` (Warteschlange,
   Aktionsleiste, jede Hinweisspalte eines `HNoticeLayer`, jedes `HSheet`,
   jedes `HModal` in der Arbeit). Das Fenster nimmt die gewählte Ecke in der
   gewählten Größe, sonst in der kleinsten, sonst die Ecke daneben, darüber
   oder gegenüber; ist keine frei (ein Modal über der Arbeit), tritt es
   zurück, bis wieder Platz ist.
6. **Tastatur.** Kein eigener `FocusScope`, kein `HLayerScope`: Das
   Terminal im Fenster nimmt die Tastatur unter derselben Regel wie im Pane
   (Zeiger, ``Ctrl+` ``, Klick, HUM-303), sonst bleibt sie, wo sie ist. Aus der
   Warteschlange geöffnet, bleibt sie in der Warteschlange, und `Enter` gibt
   frei. Ein Terminal, das die Tastatur hatte, nimmt sie in den neuen Wirt
   mit. Mit dem Fokus im schwebenden Terminal erreichen alle Kürzel der Shell
   die Shell (`TerminalPane._onKey`), keines den Agenten.
7. **Ende der Sitzung.** Endet der Agent, zeigt das Fenster das Ende vier
   Sekunden lang so, wie das Pane es zeigen würde, und legt das Terminal dann
   selbst zurück ins Pane. Ein neuer Agent in dieser Zeit hält es offen.
   Solange es schwebt, fragt die Shell den Schnappschuss der Sandbox alle
   fünf Sekunden nach (`AgentSessionFeed`), sonst hörte es vom Ende nichts.
8. UI-Texte nur über ARB (`en` Quelle, `de` Übersetzung, angehängt);
   `docs/UX.md` 5.2, `docs/DESIGN.md` (Kürzeltabelle, `HKeepClear`) und
   CONVENTIONS 3.9 beschreiben die neue Platzierung.

### Akzeptanzkriterien
1. Widget-Test: `Ctrl+Shift+P` aus Intercept zeigt das schwebende Fenster,
   der Abschnitt bleibt, genau eine `TerminalView` im ganzen Baum, das Pane
   der Sandbox ist leer; dasselbe Kürzel legt zurück.
2. Widget-Test: Derselbe Emulator (identisches `Terminal`) im Fenster, kein
   zweites `Open` an den Daemon; der Knopf im Streifen schaltet, der Knopf im
   leeren Pane legt zurück.
3. Widget-Test: Ohne Agenten schwebt nichts, weder per Kürzel noch per Knopf.
4. Widget-Test: Auf Intercept mit gehaltener Anfrage überdeckt das Fenster
   weder Warteschlange noch Aktionsleiste noch einen Hinweis; Freigeben,
   Blockieren und jede Zeile bleiben treffbar. Eine gespeicherte Ecke über der
   Warteschlange weicht auf die nächste freie aus. Ein offenes Blatt schiebt
   das Fenster zur Seite. Reine Tests der Regel: kleinere Größe vor anderer
   Ecke, Reihenfolge der Ecken, `null` ohne freie Ecke, Berühren ist kein
   Überdecken.
5. Widget-Test: Aus der Warteschlange geöffnet, gibt `Enter` frei und der
   Agent hört nichts; nach dem Zeiger über dem Fenster geht `Enter` an den
   Agenten und entscheidet nichts. Mit dem Fokus im Fenster wirken
   `Ctrl+1..6` und `Ctrl+Shift+P`, ohne den Agenten zu erreichen; danach hält
   kein bloßer Bereich die Tastatur. ``Ctrl+` `` ohne sichtbares Terminal lässt
   es schweben und gibt ihm die Tastatur; ohne Agenten tut es nichts.
6. Widget-Test: Ziehen rastet in die Ecke ein und die Ecke übersteht einen
   Neustart; der Griff ändert die Größe, gespeichert, nie unter dem Minimum.
   Zurück zur Sandbox und Schließen tun, was sie sagen.
7. Widget-Test: Endet der Agent, bleibt das Fenster `terminalFloatLinger`
   lang und schließt dann; der Abschnitt bleibt.
8. Jedes Verhalten hat einen Mutationsbeweis; `make flutter-analyze` und die
   Tests der berührten Pakete sind grün; Goldens nur dort geändert, wo der
   Streifen über dem Terminal den neuen Knopf bekommt.

### Tests
`test/features/shell/terminal_float_test.dart` (Widget-Tests der Kriterien
1 bis 7), `test/core/ui/terminal_float_test.dart` (Regel, Gedächtnis,
Platzierung), `app/packages/ui/test/keep_clear_test.dart` (nur gezeigte,
nicht leere Bereiche zählen, Meldung nach dem Frame), eine Zeile in
`shortcuts_map_to_intents_test.dart`, der Test von HUM-303 „ohne Terminal
tut das Kürzel nichts" wird zu „ohne Agenten tut es nichts".

### Fallstricke
- Zwei Ansichten derselben Sitzung melden beide ihre Größe und nehmen beide
  Tasten. Deshalb entscheidet allein die Platzierung, wer zeigt, auch auf dem
  Abschnitt Sandbox.
- Die Größe des Fensters ist die Größe der Sitzung: Ein schmales Fenster
  lässt die Oberfläche des Agenten neu umbrechen, wie jedes Ziehen der Naht
  (HUM-305).
- Ein eigener `FocusScope` oder gar ein `HLayerScope` um das Fenster hielte
  das Terminal für eine Ebene über der Arbeit; dann nähme es die Tastatur nie
  (`_mayTakeKeyboard`), oder die Shell gäbe sie beim Schließen von Ebenen an
  den falschen Knoten zurück.
- Der Wechsel zwischen Pane und Fenster darf die Tastatur nicht pauschal der
  Shell geben, wie es beim ganzen Fenster nötig ist: Dann verlöre die
  Warteschlange sie, und `Enter` entschiede nach dem Öffnen nichts mehr.
- Ein Bereich ohne Höhe überlappt in `Rect.overlaps` jedes Rechteck, das ihn
  kreuzt; leere Bereiche zählen deshalb nicht.
- Die Palette ist ein `HModal`, liegt aber über allem. Wiche das Fenster ihr
  aus, würde es bei jedem `Ctrl+K` abgebaut und neu gebaut; sie verzichtet
  deshalb auf `HKeepClear`.
- Die Rechtecke der Bereiche stammen aus dem letzten Layout. Ein Blatt oder
  Modal in einem Abschnitt (Regelblatt, Sammelentscheidung), das gerade erst
  erscheint, kann einen Frame lang unter dem Fenster liegen, bevor dieses
  ausweicht. Das Einstellungsblatt und die Palette liegen in der
  Zeichenreihenfolge ohnehin darüber.

### Umsetzung (HUM-300)

- **Platzierung.** `core/ui/terminal_placement.dart`:
  `TerminalPlacement.floating`, `toggleFloating`, `float`, `dock`; das ganze
  Fenster merkt sich, ob es aus Pane oder Fenster kam.
- **Regel und Gedächtnis.** `core/ui/terminal_float.dart`: `FloatCorner`,
  Grenzen (`terminalFloatMinSize` 320 x 200, Vorgabe 480 x 300, höchstens
  960 x 640 und 60 % der Abschnitte), die reine Funktion
  `placeFloatingTerminal` und `terminalFloatProvider`, synchron aus der
  Zustandsdatei gelesen wie der Zoom. `terminalFloatLinger` (4 s) steht beim
  Wirt in der Shell: In `core/ui` kommt jede Dauer aus `HMotion`
  (`design_system_test`), und das hier ist Lesezeit, keine Bewegung.
- **Freihalten.** `HKeepClear` in `packages/ui` (Registrierung im
  Render-Objekt, `Visibility.of` für verdeckte Abschnitte, leere Bereiche
  zählen nicht, `changes` meldet nach dem Frame). Markiert: `HSheet`, `HModal`
  (außer der Palette, `keepClear: false`), die Hinweisspalte von
  `HNoticeLayer`, die Warteschlange und die Aktionsleiste in
  `intercept_screen.dart`.
- **Wirt.** `features/shell/widgets/floating_terminal.dart`
  (`FloatingTerminalLayer`): `Stack` aus Abschnitten und Fenster, eingehängt
  im `Expanded` neben der Leiste; Titelleiste, Griff (`DragStartBehavior.down`),
  Ausweichen, Nachlauf am Ende. `SandboxTerminal(floating: true)`, im Fenster
  ohne eigenen Schwebe-Knopf im Streifen. Das Einrasten rechnet aus dem Zug
  selbst und nicht aus dem letzten Bild: Bewegungen und Loslassen können vor
  dem nächsten Frame ankommen, und dann rastete das Fenster in der alten Ecke
  ein (im ersten Testlauf gefunden).
- **Shell.** `ToggleTerminalFloatIntent`, ``Ctrl+` `` lässt schweben,
  `_terminalMoved` gibt die Tastatur nur beim ganzen Fenster pauschal der
  Shell, sonst nimmt nur ein Terminal mit Tastatur sie mit
  (`terminalHoldsKeyboard`), und landet sie auf einem Bereich, holt sie die
  Shell. Palette `toggle-terminal-float`. `AgentSessionFeed` fragt nach,
  solange das Terminal schwebt.
- **Sandbox.** Knopf `FloatButton` (`sandbox-terminal-float`, Glyphe
  `ShellGlyph.pictureInPicture`), leeres Pane mit
  `sandbox-terminal-dock`.
- **Doku.** `docs/UX.md` 5.2, `docs/DESIGN.md` (Kürzeltabelle, `HKeepClear`
  und das schwebende Terminal), CONVENTIONS 3.9, ARB (9 Schlüssel, en und de,
  angehängt).
- **Tests.** `test/features/shell/terminal_float_test.dart` (15, nach
  Review-Runde 1 22, davon
  einer bei 1280 x 800, der Breite des Laptops des Owners: Freigeben,
  Blockieren und jede Zeile bleiben mit offenem Fenster treffbar, und einer mit
  schmalem Kontext-Pane, in dem das Fenster der Aktionsleiste nach oben
  ausweicht), `test/core/ui/terminal_float_test.dart` (16),
  `packages/ui/test/keep_clear_test.dart` (4), eine Zeile in
  `shortcuts_map_to_intents_test.dart`, der umbenannte Test von HUM-303. Grün:
  `test/features/shell`, `test/features/sandbox`, `test/features/intercept`,
  `test/core` (außer drei Tests, die in der Sandbox der Agentensitzung keinen
  Unix-Socket anlegen dürfen oder Eigentümer prüfen: `grpc_client_test`,
  `rule_prefixes_round_trip_test`, `app_log_test`; sie berühren nichts aus
  diesem Issue), `packages/ui`, alle Goldens über
  `scripts/ci/goldens-placeholder.sh`; `flutter analyze` beider Pakete und
  `dart format` ohne Befund. Gelaufen über einen Schatten des SDK mit
  beschreibbarem `bin/cache` und `--no-pub`.
- **Goldens neu:** `sandbox_header_*` (4) und `isolation_panel_*` (10), nur
  der Knopf im Terminalstreifen.
- **Mutationsbeweise**, je rot und danach mit `cmp` byte-gleich zurück:
  Bindung aus `shellShortcuts()` entfernt; Warteschlange nicht freigehalten;
  Aktionsleiste nicht freigehalten; kein Rückfall auf die kleinste Größe;
  jeder Platzwechsel gibt die Tastatur der Shell; kein Schließen nach dem
  Ende; verdeckte Abschnitte zählen; leere Bereiche zählen; Ecke und Größe
  nicht geschrieben; Umschalten ohne laufenden Agenten. „Leere Bereiche
  zählen" blieb zuerst grün, weil eine zweite Prüfung nach der Umrechnung
  dasselbe abfing; die doppelte Prüfung ist entfernt.
- **Gefunden im ersten Lauf:** das Einrasten aus dem letzten Bild (oben) und
  `terminalFloatLinger` als Literal in `core/ui` (`design_system_test`).

### Review-Runde 1 (Codex, Sonnet)

Antigravity fiel aus; ein frischer Sonnet-Subagent ersetzte es mit demselben
Auftrag. Alle Befunde übernommen.

- **Gehaltene Kürzel (Codex, blockierend).** Ein Kürzel der Shell nahm im
  Terminal nur das Drücken; Wiederholungen und Loslassen fielen zum Emulator
  durch, und ein Agent mit dem Tastaturprotokoll von kitty sah das `P` von
  `Ctrl+Shift+P`. `_heldChordKeys` (HUM-313) nimmt jetzt das Drücken jedes
  Kürzels der Shell, und die Menge ist statisch, also allen Wirten gemeinsam:
  Das Loslassen kommt nach dem Schweben im anderen Wirt an. Das nächste
  Drücken derselben Taste räumt sie wie bisher. Test mit `CSI > 11 u`.
- **Ende im Nachlauf (Codex).** Das Fenster zeigte nach dem Ende vier
  Sekunden den Leerzustand. `SandboxTerminal` behält im schwebenden Wirt die
  letzte Sitzung, bis der Wirt geht oder eine neue Sitzung sie ersetzt.
- **Ende über dem ganzen Fenster (Codex).** Schweben, ganzes Fenster, Ende,
  zurück: Das Fenster blieb stehen. Der Nachlauf zählt jetzt unabhängig von
  der Platzierung; ist er vorbei, legt ein Fenster, das wieder schwebt,
  sofort zurück.
- **Halten mit der Hand (Codex, minor).** Entschieden: Der Griff lässt das
  Fenster in keiner Richtung in einen freizuhaltenden Bereich wachsen; beim
  Ziehen darf es kurz darüberstehen, weil die Hand am Fenster ist und der
  Platz beim Loslassen entschieden wird. Getestet: Mitten im Zug am Griff
  überdeckt es nichts, losgelassen über der Warteschlange weicht es aus.
- **Klick auf die Titelleiste (Sonnet).** Ein Klick endete als Zug ohne Weg
  (allein in der Arena gewinnt der Zug) oder als abgebrochener Zug und
  speicherte die Ausweichecke als gewählte. Unter `kTouchSlop` Weg rastet
  nichts ein.
- **Griff auf dem Schließen-Knopf (Sonnet).** Unten verankert liegt der Griff
  jetzt unter der Titelleiste.
- **Dateigröße (Sonnet).** Fenster, Titelleiste, Griff und Maler stehen in
  `floating_terminal_window.dart`, der Wirt bleibt in `floating_terminal.dart`.
- **Doku (Sonnet).** `HKeepClear.changes` sagt, dass ein Bereich, der sich bei
  gleicher Größe nur verschiebt, nichts meldet.
- **Mutationsbeweise**, je rot und mit `cmp` zurück: nur Zoom-Kürzel gehalten;
  eine Menge je Pane; Nachlauf mit leerer Sitzung; Rückkehr ins Fenster
  ignoriert das Ende; Griff ohne Freihalten; Fenster bleibt, wo es
  losgelassen wurde; Klick rastet ein; Griff in der Titelleiste.

### Review-Runde 2 (Codex)

Beide Befunde übernommen.

- **Ende über dem ganzen Fenster, rechtzeitig zurück.** Schweben, ganzes
  Fenster, Ende, zurück innerhalb der vier Sekunden: Das schwebende Fenster
  wird neu gebaut, und die Sitzung stand nur im alten Widget; es zeigte den
  Leerzustand. Die Sitzung steht jetzt in `lastFloatingSessionProvider`
  (Sandbox-Feature): Ein laufender Agent schreibt sie, ein Wechsel ins
  Schweben schreibt sie, das Zurücklegen ins Pane löscht sie, das ganze
  Fenster lässt sie stehen.
- **Ein Loslassen außerhalb jedes Terminals.** Kam das Loslassen einer
  Kürzeltaste woanders an (etwa in der Palette, die `Ctrl+K` geöffnet hat),
  blieb die Taste in der Menge, und die Wiederholungen eines späteren
  Drückens, das woanders begann und per Zeiger ins Terminal kam, gingen
  verloren. Ein Handler an `HardwareKeyboard` nimmt die Taste bei jedem
  Loslassen heraus, nach einer Microtask, weil diese Handler vor der
  Fokusverteilung laufen und das Terminal sein eigenes Loslassen noch in der
  Menge finden muss. Jedes Pane meldet ihn an und ab; ein einmaliger Handler
  für das ganze Programm überlebte das Zurücksetzen der Tastatur zwischen
  zwei Widget-Tests nicht (im ersten Lauf gefunden).
- **Mutationsbeweise**, je rot und mit `cmp` zurück: Sitzung beim ganzen
  Fenster gelöscht; Loslassen woanders lässt die Taste in der Menge. Die
  Beweise der Runde 1 zur Menge (nur Zoom, eine Menge je Pane, jetzt als
  „neues Pane leert die Menge") und zum Nachlauf liefen nach dem Umbau erneut
  rot.

### Review-Runde 3 (Codex)

Beide Befunde übernommen; danach keine weitere Runde.

- **Eine neue Sandbox im Nachlauf.** Endete A und nannte der Status im
  Nachlauf schon eine Sandbox B, deren Agent noch nicht lief, baute das
  Fenster weiter den Emulator von A, und ganzes Fenster hin und zurück
  konnte A neu anschließen. `lastFloatingSessionProvider` löscht die Sitzung,
  sobald der Status eine andere Sandbox oder keine nennt, und
  `SandboxTerminal` nimmt sie nur, solange der Status noch die beendete
  Sandbox nennt. Die beiden Stellen sichern sich gegenseitig; der
  Mutationsbeweis nimmt beide zugleich heraus.
- **Kein Pane eingehängt.** Hing der Handler fürs Loslassen an den Panes,
  fehlte er, solange keines stand (schwebendes Fenster mangels Platz
  ausgeblendet, altes Pane fort), und die Taste blieb gehalten. Menge und
  Handler gehören jetzt `HeldChordKeys` in `heldChordKeysProvider`: einmal
  angemeldet für die Lebenszeit des Provider-Bereichs, beim Ende abgemeldet;
  die Panes lesen und schreiben nur die Menge. Test ohne jedes Terminal in
  `test/features/sandbox/held_chord_keys_test.dart`.
- **Mutationsbeweise**, je rot und mit `cmp` zurück: Sandbox-Prüfung an
  beiden Stellen entfernt; Handler nicht von der gemeinsamen Menge
  angemeldet; eine Menge je Pane; Handler überlebt den Bereich; Loslassen
  woanders lässt die Taste in der Menge.

### Kandidaten (HUM-300)
- (A) Der Knopf „Go to terminal" der leeren Warteschlange (HUM-306) könnte
  das Terminal schweben lassen statt in den Abschnitt Sandbox zu wechseln.
- (B) Auf dem Abschnitt Sandbox könnte das schwebende Terminal pausieren und
  ins Pane zurückkehren, beim Verlassen wieder schweben.
- (C) Das Fenster ohne Platz könnte statt zu verschwinden zu einer schmalen
  Leiste mit Titel schrumpfen.
- (D) Das schwebende Fenster mit der Tastatur verschieben und in der Größe
  ändern (Review-Runde 1).
- (E) `HKeepClear` meldet auch reine Verschiebungen bei gleicher Größe
  (Review-Runde 1).

---
## HUM-314 · The terminal pane opens further shells as tabs

### Kontext
Folge-Issue von HUM-299, Punkt 8 und „Offen": Daemon, Shim, Vertrag und
Kommandozeile öffnen weitere Shells in der laufenden Sandbox (`Exec`,
`TerminalInput.Open.shell_id`, Fähigkeit `sandbox.exec`, höchstens vier,
die fünfte `TERM_002`). Die Oberfläche kannte sie nicht; `parity.md` führte
`Humanitl.Exec` als UI-Lücke. Die Terminal-Dateien der App sind seit HUM-298,
HUM-300, HUM-301, HUM-303 und HUM-313 fertig, dieses Issue setzt darauf auf.

Was der Daemon bietet und was nicht (ADR-018: jede Fähigkeit zuerst eine
RPC): `Exec` startet eine Shell und nennt ihre Kennung, `Terminal` mit
`Open.shell_id` hängt sich an ihr Terminal (ein Schreiber, beliebig viele
Leser, `TERM_001`, Ring, Exit-Code), `Close` am Strom hängt ab. **Es gibt
keine RPC, die eine Shell beendet, und keine, die die offenen Shells
auflistet.** Ein Reiter kann deshalb nur seine Ansicht schließen; die Shell
endet mit ihrem Prozess oder mit der Sitzung. Beides steht unten als
Kandidat und wird hier nicht erfunden.

### Spezifikation
1. **Reiterleiste.** Unter dem Banner des Terminals steht eine Zeile mit
   Reitern: zuerst „Agent", immer da und nicht schließbar, dann ein Reiter je
   weiterer Shell („Shell 1", „Shell 2", je Sitzung ab 1 gezählt), am Ende
   „+ Shell". Die Zeile steht, solange es etwas zu wählen gibt:
   `Info.capabilities` nennt `sandbox.exec` und eine Sitzung mit laufendem
   Agenten ist da, oder ein Shell-Reiter ist offen. Ohne die Fähigkeit fehlt
   sie, und das Terminal sieht aus wie vor diesem Issue (kein Golden ändert
   sich).
2. **Öffnen.** „+ Shell" (Tooltip nennt `Ctrl+Shift+T`) ruft `Exec` mit der
   laufenden Sitzung, leerem Kommando (interaktive Shell) und Spalten und
   Zeilen der Ansicht, die gerade zu sehen ist. Der neue Reiter wird aktiv
   und hängt sich mit `TerminalOpen.shellId` an genau die Kennung, die `Exec`
   nannte. Während ein `Exec` unterwegs ist, tut der Knopf nichts.
3. **Ein Terminal, viele Sitzungen.** Jeder Reiter zeigt dasselbe
   `TerminalPane`, nur mit `shellId`; `terminalSessionProvider(sandboxId,
   shellId: …)` hält je Shell eigenen Emulator, Rücklauf und Strom. Zoom,
   Kopieren und Einfügen, Tastatur unter dem Zeiger, die Ablehnung von
   OSC 52, gehaltene Griffe, `TERM_001` als Zuschauer und der Wiederanschluss
   nach `DAEMON_001` gelten unverändert, ohne zweite Kopie der Logik. Ein
   Reiter, der nicht zu sehen ist, bleibt angeschlossen. Das schwebende
   Fenster (HUM-300) und das ganze Fenster (HUM-298) zeigen den aktiven
   Reiter samt Leiste.
4. **Schließen.** Jeder Shell-Reiter hat ein Schließen-Glyph. Es hängt diese
   Ansicht ab (`TerminalDetach`) und verwirft ihren Emulator; die Shell läuft
   weiter, bis sie endet oder die Sitzung endet, und belegt bis dahin einen
   der vier Plätze im Daemon. Der Tooltip sagt das. Aktiv wird der Reiter
   links davon.
5. **Ende einer Shell.** Kommt der Exit-Code, trägt der Reiter ihn
   („Shell 1 · exit 3"), unter dem Terminal steht „The shell ended with exit
   code 3.", und ein Klick auf den Reiter schließt ihn.
6. **Grenze.** Mit vier offenen Reitern ist „+ Shell" gesperrt (Tooltip sagt
   warum). Lehnt der Daemon ab (`TERM_002`, auch für eine Shell, die dieses
   Fenster nicht zeigt, etwa aus `humanitl sandbox exec`; `IPC_006` ohne
   Sitzung), steht der Befund als schließbarer Hinweis über dem Terminal,
   in der Hinweisebene von HUM-302 (`HNoticeLayer`, `HDiagnosticCard` mit
   `dense`), nie über der unteren Hälfte des Terminals. Der nächste Versuch
   räumt ihn.
7. **Ende der Sitzung.** Endet der Agent oder nennt der Status eine andere
   Sandbox, enden alle Shell-Reiter: Ansicht abgehängt, Emulator verworfen,
   der Agent-Reiter wird aktiv. Im Daemon beendet PID 1 die Shells ohnehin
   (HUM-299).
8. **Tastatur.** `Ctrl+Shift+T` öffnet eine Shell, `Ctrl+PageUp` und
   `Ctrl+PageDown` wechseln zum vorigen und nächsten Reiter, rundum. Ein
   Wechsel gibt dem neuen Reiter die Tastatur, unter der Regel von HUM-303.
   Geprüft gegen jede Bindung in `app/lib`, `app/packages` und CONVENTIONS
   3.9: Keine nutzt `T`, `PageUp` oder `PageDown`; die `Ctrl+Shift`-Familie
   des Terminals ist `Enter`, `P`, `0`, `C`, `V`, die der Warteschlange `F`
   und `L`. Die Griffe stehen in `shellShortcuts()`, ihre Aktionen aber nur
   im Wirt des Terminals (`TerminalTabActions`) und nur, solange sie etwas
   tun können: `Ctrl+Shift+T` mit `sandbox.exec` und laufender Sitzung, der
   Wechsel ab zwei Reitern. Sonst gehen die Tasten wie bisher an den
   Emulator; `Ctrl+PageUp`/`Ctrl+PageDown` erreichen dann den Agenten als
   `CSI 5;5~`/`CSI 6;5~`. Der Emulator macht aus `Ctrl` mit Buchstabe nur
   ohne `Shift` ein Steuerbyte, `Ctrl+T` (`0x14`) bleibt also beim Agenten.
9. **Vertrag.** `DaemonClient.exec` mit `GrpcDaemonClient` und
   `FakeDaemonClient` (Grenze vier, `TERM_002`, `IPC_006` ohne Sitzung und
   für eine unbekannte Kennung am Terminal, `endShell` spielt das Ende);
   `TerminalOpen.shellId`; `ShellStarted`; `DaemonInfo.offersShells`;
   `DiagnosticCodes.terminalUnavailable`. `ProtoVersion.minor` 23,
   `docs/PROTOCOL.md`. `parity.dart` nennt `Humanitl.Exec` bei
   `features/sandbox/widgets/terminal_tabs`, `docs/reference/parity.md`
   ohne UI-Lücke.
10. **Aufbau.** Reitermodell und Notifier in
    `features/sandbox/providers/terminal_tabs_provider.dart`
    (`terminalTabsProvider`, `shellsOfferedProvider`, `shellHintSeenProvider`),
    Leiste, Hinweise und Aktionen in
    `features/sandbox/widgets/terminal_tabs.dart`; `SandboxTerminal` setzt
    sie um das `TerminalPane` zusammen. UI-Texte nur über ARB (`en`, `de`,
    angehängt). `docs/UX.md` 5.2, `docs/DESIGN.md` (Kürzeltabelle) und
    CONVENTIONS 3.9 nennen die Reiter und Griffe.

### Sicherheit
Eine weitere Shell läuft mit der UID des Agenten in dessen Sandbox und
PID-Namensraum; der Agent kann lesen, was dort getippt wird, und in ihr
Terminal schreiben (`docs/SECURITY.md` 4, K-18). Die Oberfläche sagt es: Mit
dem ersten Shell-Reiter erscheint einmalig der Hinweis „Do not type secrets
here; the agent can read this shell.", bis er geschlossen wird; danach nie
wieder (`terminalShellHintSeen` in der Zustandsdatei der Oberfläche). Die
Aussage in `README.md` bleibt unberührt: Die App öffnet keine neue Tür, sie
ruft nur die RPC, die der Daemon schon hat, und filtert nichts selbst.

### Akzeptanzkriterien
1. Ohne `sandbox.exec` keine Leiste und kein „+ Shell"; `Ctrl+Shift+T` ruft
   kein `Exec`, `Ctrl+PageDown` erreicht den Agenten.
2. Ohne laufende Sitzung kein „+ Shell".
3. „+ Shell" ruft `Exec` mit leerem Kommando und der Größe der Ansicht; der
   neue Reiter ist aktiv, sein `Open` trägt genau die Kennung aus `Exec`,
   er hat einen eigenen Emulator, und der Wechsel zurück zum Agenten zeigt
   dessen Emulator ohne neues `Open` der Shell.
4. Tasten in einem Shell-Reiter erreichen nur diese Shell.
5. `TERM_002` steht als Hinweis über dem Terminal und lässt sich schließen;
   mit vier Reitern ist der Knopf gesperrt, und `Ctrl+Shift+T` ruft kein
   `Exec`.
6. Eine beendete Shell zeigt ihren Exit-Code am Reiter und unter dem
   Terminal; ein Klick schließt den Reiter und verwirft seinen Emulator.
7. Schließen eines laufenden Shell-Reiters beendet nur dessen Strom.
8. Das Ende der Sitzung beendet alle Shell-Reiter.
9. `Ctrl+Shift+T` öffnet, `Ctrl+PageUp`/`Ctrl+PageDown` wechseln rundum, der
   neue Reiter hat die Tastatur, kein Byte eines Griffs erreicht eine Shell.
10. Der Hinweis zur Mitlesbarkeit kommt einmal mit der ersten Shell und wird
    beim Schließen in die Zustandsdatei geschrieben.
11. Das schwebende Fenster zeigt den aktiven Reiter samt Leiste.

### Tests
`app/test/features/sandbox/terminal_tabs_test.dart`, je ein Test pro
Kriterium, gegen `FakeDaemonClient` (nie gegen den Socket des Daemons),
Plattform Linux; dazu die Grenze und die unbekannte Kennung im Fake.
`shortcuts_map_to_intents_test` kennt die drei neuen Griffe.

### Umsetzung (HUM-314)
- **Vertrag.** `DaemonClient.exec` (`ExecRequest`/`ExecResponse` in
  `grpc_daemon_client.dart`, `ExecResponseToDomain` in `convert.dart`),
  `TerminalOpen.shellId` bis auf den Draht, `ShellStarted` in
  `core/domain/terminal.dart`. Der Fake führt Grenze und Kennungen wie der
  Daemon (`<acht Hex-Zeichen>-<n>`), sein `terminal()` nimmt für eine Shell
  einen eigenen Weg, der den Schreibplatz des Agenten nicht berührt, und
  `endTerminals` räumt die Shells mit. Ein zweiter Schreiber derselben Shell
  wird im Fake nicht abgelehnt (steht dort).
- **Sitzung.** `TerminalSession.build(sandboxId, {shellId = ''})`; sonst
  unverändert. `TerminalPane` bekommt `shellId`, `tabs` und `notices`; alle
  Lesezugriffe gehen über einen Getter `_session`, die Exit-Zeile nennt bei
  einer Shell die Shell. Die Hinweisebene legt sich nur um den Emulator,
  solange die Leiste steht, damit der Baum ohne Fähigkeit derselbe bleibt.
- **Reiter.** `TerminalTabs` hört auf die laufende Sitzung und räumt bei
  jedem Wechsel; `close` hängt ab und invalidiert die Sitzung der Shell.
  `SandboxTerminal` schlüsselt den Pane je Reiter (der Agent behält
  `Key('sandbox-terminal')`) und gibt beim Wechsel die Tastatur.

### Kandidaten (HUM-314)
- (A) RPC zum Beenden einer weiteren Shell (etwa `Exec` mit `close`, oder
  `SIGHUP` an ihre Sitzung über die Tür), damit das Schließen eines Reiters
  die Shell auch beenden kann und ihren Platz freigibt.
- (B) RPC, die die offenen Shells einer Sitzung auflistet, damit die App
  nach einem Neustart oder für Shells aus `humanitl sandbox exec` Reiter
  wieder anbieten kann (auch für `attach` ohne Kennung, HUM-299 „Offen").
- (C) Eigener Befundcode für die Grenze (`TERM_003`) samt ARB, statt
  `TERM_002` mit eigenem Satz.
- (D) Ein Befehl „New shell" in der Befehlspalette.
- (E) Weitere Shells vom Agenten trennen (andere UID oder `hidepid`), dann
  kann der Hinweis zur Mitlesbarkeit entfallen (HUM-299, Kandidat).

### Review Runde 1 (HUM-314)
- **Status ohne Antwort beendet keine Reiter** (Codex, major). `TerminalTabs`
  räumt nur auf eine `AsyncData` des Sandbox-Status hin; Laden und Fehler
  zählen nicht. Gemessen: Riverpod 3 trägt in `AsyncError` den letzten Wert
  weiter, der beschriebene Verlust trat mit dem heutigen Status also nicht
  ein; die Wache macht die Regel unabhängig davon. Test
  `a_failed_status_question_keeps_the_shell_tabs`; Mutation (Wache weg und
  ein Fehler ohne Wert) rot.
- **Eine Shell, die der Daemon nicht mehr kennt** (Codex, major). Endete sie
  vor dem ersten Anhängen und kam das `Open` nach `FINISHED_RETENTION`,
  antwortete es `IPC_006`, und der Reiter hieß für immer „Shell 1". Jetzt
  „Shell 1 · ended" (ARB `sandboxTerminalTabShellGone`, en und de), ein Klick
  schließt. Test `a_shell_the_daemon_no_longer_knows_shows_ended_and_closes`;
  Mutation (`gone` immer falsch) rot.
- **Tastatur nur auf Wunsch** (Sonnet, minor). Ein Wechsel, den der Mensch
  nicht auslöste (späte Antwort von `Exec`, Ende der Sitzung), nimmt einem
  Textfeld die Tastatur nicht mehr (`TerminalTabsState.byUser`,
  `isTextInputFocused`).
- **Fake** (Codex, minor): prüft beim `Open` einer Shell erst die Sandbox
  (`IPC_006`), schließt den Strom einer Shell nach ihrem `Exit`, verwirft
  Tasten danach und nimmt abgehängte Ströme aus seiner Liste. Tests
  `the_fake_refuses_a_live_shell_of_another_sandbox`,
  `the_fake_closes_a_shell_stream_after_its_exit`.

---
## HUM-315 · The floating terminal docks on the Sandbox page, floats from the empty queue and keeps a title strip

### Kontext
Folge-Issue von HUM-300, Kandidaten (A), (B) und (C). Der Owner hat am
2026-10-30 alle drei Punkte entschieden und dazu das Kürzel bestätigt:
`Ctrl+Shift+P` bleibt, nichts daran ändert sich. HUM-314 hat seither Reiter
ins Terminal gebracht; der Streifen unten nennt den aktiven.

### Spezifikation
1. **Auf der Sandbox-Seite angedockt.** Solange der Abschnitt Sandbox zu
   sehen ist, steht ein schwebendes Terminal in seinem Pane dort, an seinem
   gewohnten Platz; ein Fenster über der eigenen Seite verdeckte nur deren
   Pane. Wer die Seite verlässt, sieht es wieder schweben, solange die
   Sitzung läuft und der Mensch es vorher schweben ließ. Das Andocken wegen
   der Seite ist ein Zustand der Ansicht und keine Wahl:
   `terminalPlacementProvider` bleibt `floating`, Knopf und Palette sagen
   weiter „zurücklegen", und ein Druck darauf legt das Terminal wirklich
   zurück, sodass es beim Verlassen nicht mehr schwebt.
2. **Eine Regel für den Wirt.** Welcher Wirt zeigt, entscheidet allein
   `terminalHostFor` in `core/ui/terminal_placement.dart` über die Wahl und
   `sandboxPageShownProvider` (die Shell schreibt ihn bei jedem Wechsel des
   Abschnitts), gelesen über `terminalHostProvider`. Wer zeigt (Pane,
   schwebende Ebene), liest den Wirt; wer die Wahl ändert oder nennt
   (Knöpfe, Kürzel, Palette, Zustandsabfrage), liest die Wahl.
3. **Ende auf der Seite.** Endet der Agent, während die Seite zu sehen ist,
   legt die Ebene das Terminal sofort zurück: Das Pane zeigt das Ende
   selbst, und beim Verlassen schwebte sonst eine Sitzung, die vorbei ist.
   Endet er anderswo, gilt der Nachlauf von HUM-300.
4. **Tastatur beim Wechsel der Seite.** Hatte das schwebende Terminal die
   Tastatur, behält das Pane sie beim Betreten der Seite; es wird
   weitergetippt. Beim Verlassen nimmt sie das Terminal nicht mit: Wer
   einen anderen Abschnitt verlangt, bekommt dessen Regel, die
   Warteschlange nimmt die Tastatur, und `Enter` entscheidet wie bisher
   (HUM-300, Spezifikation 6).
5. **„Go to terminal".** Der Knopf der leeren Warteschlange (HUM-306)
   wechselt nicht mehr den Abschnitt, sondern tut, was ``Ctrl+` `` tut
   (`FocusTerminalIntent`): Ohne sichtbares Terminal schwebt es über der
   Warteschlange und bekommt die Tastatur, nach der Regel von HUM-303
   (`_mayTakeKeyboard`, nicht über einer Ebene). Schwebt es schon, bekommt
   es nur die Tastatur. Auf der Sandbox-Seite bekommt das angedockte
   Terminal die Tastatur. Zeigt die schwebende Ebene nur den Streifen (oder
   gar nichts), legt die Anfrage das Terminal zurück, zeigt die
   Sandbox-Seite und gibt dem Pane die Tastatur, wie ein Klick auf den
   Streifen ohne Platz (Review-Runde 1). Der ausdrückliche Wunsch nach dem Terminal ist
   der Unterschied zu `Ctrl+Shift+P` aus der Warteschlange, das die
   Tastatur dort lässt.
6. **Titelstreifen statt Verschwinden.** Findet das Fenster keine freie
   Ecke, schrumpft es zu einem schmalen Streifen: Profil der Sandbox (oder
   „Terminal"), der aktive Reiter aus HUM-314 („Agent", „Shell 1") und ein
   Knopf. Größe als Token `HSize.terminalStrip` (240 x 28, so hoch wie
   `hitMin` und die Titelleiste des Fensters). Er nimmt die erste Ecke in
   derselben Reihenfolge wie das Fenster, die ihm Platz lässt
   (`placeTerminalStrip`), und überdeckt nie, was `HKeepClear` markiert:
   Warteschlange, Freigeben und Blockieren, Hinweise, Blätter, Modale. Nur
   ohne Platz auch für ihn tritt er zurück. Er hält kein Terminal, ändert
   die Größe der Sitzung also nicht und nimmt keine Taste.
7. **Klick auf den Streifen.** Öffnet das Fenster in der nächsten freien
   Ecke, gemessen an den Bereichen, wie sie jetzt stehen; ist keine frei,
   legt er das Terminal zurück und zeigt die Sandbox-Seite. Weil das
   Fenster von selbst zurückkehrt, sobald eine Ecke frei wird, ist der
   zweite Fall der übliche; nur zurücklegen ohne die Seite zu zeigen hieße
   das Terminal ganz zu verstecken, obwohl der Mensch es sehen wollte.
8. UI-Texte nur über ARB (`en` Quelle, `de` Übersetzung, angehängt);
   `docs/UX.md` 5.2 und `docs/DESIGN.md` (Token-Tabelle, `HKeepClear` und
   das schwebende Terminal) beschreiben Andocken, Streifen und den Knopf.

### Akzeptanzkriterien
1. Widget-Test: Schwebend auf History, `Ctrl+4` zeigt das Terminal im Pane
   der Sandbox-Seite mit demselben Emulator, kein Fenster, genau eine
   `TerminalView`, die Wahl bleibt `floating`; `Ctrl+3` lässt es wieder
   schweben. Ein nie geschwebtes Terminal schwebt beim Verlassen nicht.
2. Widget-Test: Endet der Agent auf der Sandbox-Seite, ist die Wahl danach
   `pane`, und beim Verlassen schwebt nichts.
3. Widget-Test: Hatte das Fenster die Tastatur, hat sie das Pane nach
   `Ctrl+4`; nach `Ctrl+1` hat sie das Terminal nicht, und kein bloßer
   Bereich hält sie.
4. Widget-Test: „Go to terminal" lässt das Terminal über Intercept
   schweben, der Abschnitt bleibt, das Terminal hat die Tastatur; schwebt
   es schon, bekommt es nur die Tastatur.
5. Widget-Test: Ohne freie Ecke (History, offenes Einstellungsblatt, links
   zu schmal für 320 x 200) steht der Streifen in 240 x 28, nicht über dem
   Blatt, mit Profil und Reiter; geht das Blatt, wird er wieder Fenster;
   ein Klick ohne Platz legt zurück und zeigt die Sandbox-Seite.
6. Reine Tests: `placeTerminalStrip` (Ecke zu klein fürs Fenster, Reihenfolge,
   `null` ohne Platz) und `terminalHostFor` samt Provider (die Seite ändert
   die Wahl nie).
7. Goldens `terminal_strip` in vier Varianten (dunkel und hell, en und de);
   sonst ändert sich kein Golden.
8. Mutationsbeweise für Andocken und erneutes Schweben, „Go to terminal"
   und den Streifen statt Verschwinden.

### Tests
`app/test/features/shell/terminal_dock_test.dart` (Kriterien 1 bis 5),
`app/test/core/ui/terminal_float_test.dart` (Kriterium 6),
`app/test/goldens/terminal_strip_golden_test.dart`, das Token in
`app/packages/ui/test/tokens_test.dart`. Angepasst: in
`terminal_float_test.dart` das Schweben vom Pane aus (jetzt erst nach dem
Verlassen der Seite, zurück über `Ctrl+4` statt über den entfallenen Knopf
im leeren Pane) und der gehaltene Griff (der Wechsel des Wirts kommt jetzt
vom schwebenden Fenster über History); in `empty_queue_session_test.dart`
bleibt der Abschnitt nach „Go to terminal"; in `terminal_tabs_test.dart`
schwebt das Terminal mit seinen Reitern über History statt über der
Sandbox-Seite.

### Fallstricke
- Verdeckte Abschnitte bauen in Riverpod 3 bei geändertem Provider nicht
  neu (ihre Abos ruhen unter `TickerMode`). Das Pane der Sandbox-Seite
  baut deshalb bei jeder Rückkehr des Terminals ausdrücklich neu
  (`setState`), wie es das vor diesem Issue schon tat; die Tastatur nimmt es
  dabei nur aus dem ganzen Fenster.
- Der Knopf „Terminal hierher zurückholen" im leeren Pane hat keinen Platz
  mehr: Auf der eigenen Seite dockt das Terminal ohnehin an. Der
  ARB-Schlüssel `sandboxTerminalDockHere` ist in en und de entfernt.
- Der Streifen nimmt nur Ecken. Eine Lage, in der alle vier Ecken auch für
  240 x 28 belegt sind (ein Modal über der Arbeit), lässt ihn zurücktreten
  wie früher das Fenster.

### Umsetzung (HUM-315)
- **Wirt.** `core/ui/terminal_placement.dart`: `sandboxPageShownProvider`,
  `terminalHostFor`, `terminalHostProvider`. Die Shell schreibt die Seite in
  `_sectionAsked` (und einmal nach dem ersten Frame) und reicht dort die
  Tastatur auf die Seite weiter.
- **Ebene.** `FloatingTerminalLayer` zeigt nach dem Wirt, dockt bei einem
  Ende auf der Seite sofort und baut ohne freie Ecke den Streifen
  (`floating_terminal_strip.dart`: `FloatingTerminalStrip` ohne Provider,
  `LiveFloatingTerminalStrip` fragt Profil und Reiter).
- **Regel.** `core/ui/terminal_float.dart`: `placeTerminalStrip` teilt mit
  `placeFloatingTerminal` die Ecken und die Prüfung (`_firstFree`).
- **Token.** `HSize.terminalStrip` und `HSizeTokens.terminalStrip` in
  `packages/ui`, `dump_tokens.dart`, Token-Tabelle in `docs/DESIGN.md`.
- **Sandbox.** Pane nach dem Wirt, leeres Pane ohne Knopf;
  `terminalActiveTabLabel` in `terminal_tabs.dart`.
- **Warteschlange.** „Go to terminal" ruft `FocusTerminalIntent`.
- **ARB.** 4 Schlüssel, en und de, angehängt; `sandboxTerminalDockHere`
  entfernt, die Beschreibung von `interceptEmptyGoToTerminal` nachgezogen.
- **Goldens neu:** `terminal_strip` (4: dunkel und hell, en und de); kein
  anderes Golden geändert.
- **Grün:** `test/features/shell`, `test/features/sandbox`,
  `test/features/intercept`, `test/core/ui`, `test/core/shortcuts`,
  `packages/ui`, alle Goldens; `flutter analyze` beider Pakete, `dart format`
  und `tools/check_coupling.py` ohne Befund. Gelaufen über einen Schatten des
  SDK mit `--no-pub`.
- **Mutationsbeweise**, je rot und danach mit `cmp` byte-gleich zurück:
  `terminalHostFor` ohne die Seite (Andocken und erneutes Schweben); „Go to
  terminal" wechselt wieder den Abschnitt; ohne freie Ecke kein Streifen.

### Kandidaten (HUM-315)
- (A) Der Streifen nimmt nur die vier Ecken; er könnte entlang der Kanten
  einen freien Platz suchen.
- (B) `sandboxSectionIndex` in `core/shortcuts/intents.dart` hat keinen
  Nutzer mehr außer seinem Test; streichen oder für einen künftigen Weg
  behalten.

### Review-Runde 1 (Codex, Sonnet)
Kein blockierender oder schwerer Befund; die minor-Befunde übernommen.
- **Tastatur ohne Fenster.** ``Ctrl+` `` und „Go to terminal" taten nichts,
  solange nur der Streifen stand. Die Ebene hört jetzt auf
  `terminalFocusRequestProvider` und legt dann zurück, zeigt die
  Sandbox-Seite und fragt einen Frame später erneut, sodass das Pane die
  Tastatur nimmt (ehemals Kandidat B). Test
  `asking_for_the_terminal_with_only_the_strip_docks_it` (Rules, 900 x 280:
  zu niedrig für das kleinste Fenster); Mutation (Anfrage ignoriert) rot.
- **Verlassen mit der Tastatur im Pane.** Test
  `the_pane_keeps_no_keyboard_after_leaving_and_ctrl_1_works`: nach
  `Ctrl+2` hält das Terminal die Tastatur nicht, und `Ctrl+1` erreicht die
  Shell.
- **ARB.** `sandboxTerminalDockHere` entfernt, Beschreibung von
  `interceptEmptyGoToTerminal` nachgezogen.

---
## HUM-316 · Aborted TLS tunnels, model traffic and flow IDs are shown truthfully

### Kontext
Befund des Eigentümers am 2026-10-30. `humanitl flows list` zeigte 16 Zeilen
`CONNECT llm.hzo.schutzwerk.net`, Entscheidung `block`, Zustand `recorded`,
0 bis 1 ms, Fehler `tls_handshake_failed`, null Funde. Der Host ist der
eingestellte Modellserver. Drei Dinge stimmten nicht:

- `record_connect_failure` in `daemon/crates/proxy/src/handler.rs` verbuchte
  jeden abgebrochenen oder abgelehnten Handschlag des Clients und jeden
  Tunnel, der nach dem Handschlag leer blieb, als `Decision::Block { reason:
  NoRoute }` durch das System. Geblockt hatte niemand. Die Klasse
  (`tls_observe::TlsFailure`: `eof_before_finished`, `reset_before_finished`,
  `closed_after_handshake`, `alert_unknown_ca` und andere) stand nur in einer
  Protokollzeile auf `debug`; am Flow stand `tls_handshake_failed` ohne
  Ursache. In der App hieß dieselbe Zeile in der Regelspalte sogar „manuell",
  weil die Aufzeichnung keine Quelle führt.
- Die gelungenen Anfragen an das Modell sind Durchreich-Flows.
  `humanitl flows list` schickte `include_passthrough: false` fest
  eingetragen; die History hat dafür einen Schalter, die Kommandozeile hatte
  keinen.
- Die Spalte `ID` zeigte die ersten acht Zeichen einer UUIDv7. Die sind die
  Uhrzeit und für alle Flows einer Minute gleich (fünfmal `01a189bd`). Eine
  Zeile ließ sich so nicht ansprechen, und `flows show` nahm ohnehin nur die
  ganze Id.

### Spezifikation
1. **Eigener Grund statt `NoRoute`.** Neue Variante
   `BlockReason::ClientAborted` (`client_aborted`), auf der Leitung
   `BLOCK_REASON_CLIENT_ABORTED = 12`, `PROTO_MINOR` 24, `docs/PROTOCOL.md`,
   `proto/descriptor.binpb`, `proto/generated.sha256`,
   `ProtoVersion.minor` 24. Der Flow endet weiter als `Decided(Block)` durch
   das System und dann `Recorded`.
   **Warum so und nicht anders.** Der Automat schließt einen Flow nur über
   eine Entscheidung ab, und nichts ging zum Ziel; ein neuer
   Entscheidungstyp hätte jeden Zweig über `Decision` in Proxy, Hold,
   Pipeline, Audit und App berührt. Eine neue `DecisionSource` wäre nach dem
   Neuladen verloren, denn die Tabelle `flows` führt keine Quelle. Der Grund
   dagegen steht schon heute als Text in `flows.block_reason` und reist über
   `FlowEvent::Decided` wie über `ListFlows`: keine Migration, eine additive
   Enum-Nummer, und ältere Clients lesen ihn als unbekannt. `decision` bleibt
   `block`, damit Zählungen und der Filter `decision:block` wie bisher
   rechnen; was die Oberflächen zeigen, entscheidet der Grund.
   `http_status` ist 408 wie `ClientTimeout`, geht aber nie hinaus.
2. **Die Klasse steht am Flow.** `flows.error` trägt
   `tls_handshake_failed:<klasse>` (`tls_observe::flow_error`), dieselbe Form
   wie `upstream_private_address:<ip>`. `AlertOther` schreibt nur
   `alert_other`; die Beschreibung von rustls steht im Befund. Keine
   Migration, die Spalte gibt es seit HUM-045. Ältere Zeilen behalten
   `tls_handshake_failed` ohne Klasse; wer den Fall erkennt, vergleicht den
   Teil vor dem ersten `:` (Kommentar an `FlowSummary.error`).
   **Protokoll.** Kein `info` je Flow: Ein Client, der im Takt neu verbindet,
   flutete sonst das Journal. Eine Zeile auf `info` mit Host, Klasse und Flow
   steht genau dann, wenn auch eine Karte fällig ist, also unter demselben
   Zählfenster (`HandshakeWatch`, `MAX_REPORTS_PER_WINDOW`). Die Zeile auf
   `debug` je Versuch bleibt.
3. **`flows list --passthrough`.** Der Schalter setzt
   `ListFlowsRequest.include_passthrough`; der Name folgt dem Feld und dem
   Chip der History. Die Spalte DECISION schreibt `passthrough` für einen
   Durchreich-Flow, der freigegeben wurde (`allow`, `allow_edited`); ein
   Block oder Ablauf an ihm bleibt, was er ist, und ohne Entscheidung steht
   wie überall ein Strich. `flows show` sucht seine Zeile ebenfalls mit
   Durchreiche.
4. **Eindeutige IDs.** Die Spalte `ID` zeigt das Ende der Id in Hexziffern
   ohne Bindestriche, so viele Zeichen, wie es braucht, damit das Ende jeder
   Zeile auf genau einen Flow passt, mindestens acht, eine Breite für die
   ganze Seite. Gezählt wird über die Seite und über dieselbe Menge, in der
   `flows show` nachschlägt (`search_set`: die jüngsten 1000 Flows samt
   Durchreiche, eine Stelle im Code für Liste, Probelauf und `show`); sonst
   druckte `flows list --limit 1` ein Ende, das `show` als mehrdeutig
   abwiese. `ListFlows` kennt keine Abfrage nur nach Ids, die Liste holt
   deshalb eine zweite Seite Zusammenfassungen.
   Das Ende statt des Anfangs, weil der Anfang einer UUIDv7 die Uhrzeit ist
   und das Ende Zufall. `flows show` nimmt die ganze Id (mit oder ohne
   Bindestriche) wie bisher, oder ein Ende ab acht Hexziffern
   (Groß-/Kleinschreibung und Bindestriche egal), nachgeschlagen unter den
   letzten 1000 Flows samt Durchreiche. Keiner passt: `IPC_003` wie beim
   bisherigen Nachschlagen, mit dem Satz, dass nur die jüngsten 1000 Flows
   durchsucht werden und ein älterer die ganze Id aus
   `humanitl flows list --json` braucht. Mehrere passen: `CLI_004` mit allen Kandidaten
   im Text, geraten wird nie. Zu kurz oder kein Hex: `CLI_004` mit
   `CopyCommand` auf `humanitl flows list`. Auch die Bestätigungszeile von
   `flows decide` nennt das Ende.
5. **Wahrheit in Tabelle, `show` und JSON.** DECISION und die Zeile
   `decision` von `flows show` schreiben `aborted`, wo der Grund
   `client_aborted` ist. `flows show` zeigt unter `status` eine Zeile
   `error`, wenn der Flow einen Fehler trägt. `--json` behält `decision`
   (`block`) und nennt daneben `outcome` (dasselbe Wort wie die Spalte) und
   `block_reason` (leer ohne Block). Die Kommandozeile liest dafür nur Enums
   des Dienstes, sie deutet nichts aus `error` (ADR-018).
6. **App, so knapp wie möglich.** `BlockReason.clientAborted` (Position 12),
   in der History Fehlerfarbe wie `no_route`, Regelspalte „client"
   („Client"), Feld Entscheidung im Detail „aborted" („abgebrochen"); ARB
   `historyDeciderClient`, `historyDecisionAborted` (en und de, angehängt).
7. **Dokumentation.** Hilfe von `flows list` und `flows show`,
   `docs/reference/cli.md` (erzeugt), Tabelle in
   `docs/guides/first-session.md`, CONVENTIONS 3.1 (Enum) und 4 (Grund und
   Status).

### Akzeptanzkriterien
1. Ein abgelehnter Handschlag, ein abgebrochener Handschlag und ein leer
   geschlossener Tunnel ergeben je einen Flow mit `block_reason =
   client_aborted`, nie `no_route`; Tabelle und `flows show` schreiben
   `aborted`, `--json` `outcome: "aborted"`.
2. `flows.error` trägt die Klasse: `tls_handshake_failed:alert_unknown_ca`,
   `…:closed_after_handshake`, `…:eof_before_finished` oder
   `…:reset_before_finished`; `flows show` und `--json` zeigen sie.
3. Der Grund überlebt das Neuladen aus der Aufzeichnung.
4. `flows list --passthrough` schickt `include_passthrough: true`, ohne den
   Schalter `false`; ein Durchreich-Flow heißt in DECISION `passthrough`.
5. Die Spalte `ID` ist auf jeder Seite eindeutig, mindestens acht Zeichen;
   `flows show` findet mit genau diesem Ende genau den Flow der Zeile,
   nimmt weiter die ganze Id und rät bei mehreren Treffern nicht.
6. Die History zeigt einen abgebrochenen Tunnel nicht als Blockierung und
   nicht als „manuell".
7. Kein Schema der Aufzeichnung ändert sich; die Proto ändert sich nur
   additiv.

### Tests
- `daemon/crates/proxy/tests/tls_observe.rs`: die vorhandenen Fälle prüfen
  Klasse und `client_aborted` (Ablehnung `alert_unknown_ca`, leerer Tunnel
  `closed_after_handshake`, Wiederholung), der Fall `TLS_002` zeichnet jetzt
  auf und prüft für jeden der drei Abbrüche `eof_before_finished` oder
  `reset_before_finished` samt `client_aborted`.
- `tls_observe::tests::the_flow_error_carries_the_class`.
- `humanitl_ipc::convert::tests::an_aborted_tunnel_survives_the_recording`.
- `humanitl` (`cmd::flows`): `an_aborted_tunnel_reads_aborted_and_names_its_cause`,
  `a_passthrough_flow_reads_passthrough`,
  `the_id_column_tells_every_row_of_a_page_apart`,
  `the_id_column_grows_until_the_ends_differ`,
  `flows_show_takes_the_end_the_table_prints`,
  `an_end_that_fits_several_or_none_is_refused`; `cli`:
  `passthrough_is_a_flag_of_flows_list`.
- `app/test/features/history/history_view_test.dart`: `an aborted tunnel was
  decided by nobody (HUM-316)`, aus einer `FlowSummary` von der Leitung.

### Kandidaten (HUM-316)
- (A) `flows decide` nimmt das Ende der Id ebenso wie `flows show`. Der
  Leitfaden führt `flows show <id>` und `flows decide <id>` nebeneinander;
  heute braucht `decide` die ganze Id aus `--json`.
- (B) Die Aufzeichnung führt `decision_source` als Spalte, damit „manuell"
  in der History nicht mehr aus dem Fehlen einer Regel geschlossen wird.
- (C) Den Grund der 16 Abbrüche beim Modellserver klären, sobald die Klasse
  am Flow steht: ein Client mit eigenem Verbindungspool, der Verbindungen
  vorab öffnet und ungenutzt schließt, sähe genau so aus
  (`closed_after_handshake`). Dann wäre der Flow kein Fehler, sondern
  Rauschen, und die Durchreiche könnte ihn unterdrücken oder zusammenfassen.
- (D) Ein Filterterm `reason:` im Recorder (`reason:client_aborted`), damit
  die History und `flows list` die Abbrüche ein- oder ausblenden können.

### Review Runde 1 (HUM-316)
- **Breite der Spalte gegen die Suchmenge** (Codex, major, AK 5). Die Breite
  kam aus der Seite allein, `flows show` sucht aber in den jüngsten 1000
  Flows; `flows list --limit 1` konnte so ein Ende drucken, das `show` mit
  `CLI_004` abwies. Jetzt misst `id_tail_len` gegen Seite und `search_set`.
  Test `the_id_column_is_unique_among_what_show_searches`; Mutation (Suchmenge
  ignoriert) rot. Auch `rules dry-run` misst so.
- **Text von `IPC_003`** (Sonnet, minor): nennt die Grenze von 1000 und den
  Weg über `flows list --json`.
- **`passthrough` nur nach Freigabe** (Sonnet, minor): ohne Entscheidung
  wieder ein Strich.

---
## HUM-350 · Disposable sandbox: the project folder as an overlay, with a diff, apply and discard

Owner request 2026-10-29, put into 0.1.0 on 2026-10-30. Daemon, RPC and
command line; the changes panel of the app is HUM-351, because it brings its
own screen, ARB strings and fake scenarios and the issue would otherwise be
too large to review in one diff.

### Context
Today the agent writes into the project folder directly (`work_mode = rw`)
or not at all (`ro`). The owner wants a third way that lets the agent work
freely and keeps the folder untouched until a human decides: the folder as
the read-only lower layer of an overlay, every change in an upper layer in
RAM, and afterwards a diff with "apply" (all or single files) or "discard".

### Decision: the upper layer on the host, read after the end (option a)
`--tmp-overlay` keeps the upper layer in a tmpfs that exists only in the
sandbox's mount namespace; after the end nobody can read it. Two ways out:

- **(a)** `--overlay-src LOWER --overlay UPPER WORK /work`, with `UPPER` and
  `WORK` in a directory the daemon creates on the tmpfs of its runtime
  directory (`$XDG_RUNTIME_DIR/humanitl/disposable/<id>/`, mode `0700`). The
  daemon reads the layer after the sandbox has ended.
- **(b)** The shim (PID 1) builds a diff or archive before it exits and sends
  it out.

Chosen is **(a)**, because its attack surface is smaller. (b) puts a diff
engine into PID 1, the process that holds the sandbox together and runs with
the agent's UID next to it, and opens a second channel out of the sandbox
whose payload is agent-controlled bytes the daemon has to parse. (a) adds no
code inside the sandbox and no channel: the daemon reads a directory nobody
writes to any more, with the same `openat2(RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS)` discipline as the session summary (HUM-043). The path
of the upper layer is not visible inside: `/run` never comes from the host,
and bubblewrap opens the overlay directories by descriptor before it mounts
(`upperdir=/proc/self/fd/N` in the sandbox's mount table, measured with
bubblewrap 0.13.1). The lower layer is the opened project folder as
`/proc/self/fd/N` (HUM-263); the shim closes that descriptor before the agent
starts, as for `SourceFd::ProcSelf`.

The price of (a) is the size cap: an unprivileged daemon cannot mount an
own tmpfs with `size=`. The hard limit is therefore the tmpfs of the runtime
directory (logind, by default 10 % of RAM); below it the daemon measures that
file system with `statvfs(2)` and stops the sandbox above its own cap. That is
a cap with a measuring interval, not a kernel limit, and the spec says so.

### Specification
1. **Mode.** `sandbox.mode = "direct" | "disposable"` in `config.toml`
   (tier `basic`, closed to project profiles: a project that switches back to
   `direct` would give the agent the write path the human deselected). Per
   start: `SandboxRequest.Start.sandbox_mode` (empty: the configuration),
   `humanitl run --disposable`, `humanitl sandbox run --disposable`.
2. **Mounts.** The project folder becomes `--overlay-src /proc/self/fd/N
   --overlay UPPER WORK /work`; masks and `tmpfs` under `/work` follow as
   before and lie on the overlay. Per mount type:
   - project folder: overlay, upper layer reviewed and applied on request;
   - `mounts.extra_rw` (the user's writable extensions, also the only way to
     share a package cache): **read-only** (`--ro-bind-fd`). A second overlay
     per entry would be a second upper layer nobody reviews, and a cache that
     is only written inside a disposable run gains nothing; read-only keeps
     the cache usable for reading and leaves no writable host path;
   - `mounts.extra_ro`, adapter files, CA, shim, proxy socket: unchanged
     (already read-only);
   - `tmpfs` (`/tmp`, `/dev/shm`, the profile's list): unchanged, they never
     came from the host.
3. **Preconditions** (`SANDBOX_030`, blocking, each with a fix): bubblewrap
   0.11 or newer (`--overlay`, fix: install command); the runtime directory
   on tmpfs (fix: `XDG_RUNTIME_DIR=/run/user/<uid>`); the project mounted
   writable for the agent's view (`work_mode = rw`); no earlier disposable run
   with changes still waiting (fix: `humanitl sandbox changes`).
4. **During the run.** A `SizeGuard` measures the growth of the file system
   under the layer every 250 ms: more than 2 GiB or 200 000 entries (or half
   of what was free at the start) stops the sandbox with `SANDBOX_032`. The
   project snapshot of the summary is not taken; a disposable run has no
   project summary, its changes are the summary.
5. **After the run.** The layer and the baseline (the project's tree at the
   start: type, size, `mtime`, mode, no hashes, no skipped names) wait in the
   daemon. The start stream gets one `SandboxEvent.changes` without anyone
   asking. Reading requires that the sandbox has ended: bubblewrap is reaped
   and the sandbox's first process is gone from `/proc` (`SANDBOX_031`
   otherwise).
6. **The diff.** Walking the upper layer: a whiteout (character device `0:0`)
   is a deletion of what the project has there; a directory with
   `user.overlay.opaque = "y"` deletes every lower entry it does not have; a
   regular file is added, modified (content hash differs) or mode-changed; a
   file copied up but unchanged is no change; a symlink is added or modified
   with its target. Each change carries a conflict note when the project's
   entry differs from the baseline (for deletions and type changes of a
   directory, the whole subtree). Never applicable (`refused`): a symlink
   whose target leaves the project (lexically, never resolved), a device,
   FIFO or socket, a path whose parent in the project is a symlink, an entry
   with `user.overlay.redirect` or `user.overlay.metacopy`. Setuid, setgid
   and sticky bits are reported as dropped, hard links as split. Text files up
   to 1 MiB get a `diff -u` (Myers, at most 1000 changed lines, at most 64 KiB,
   control and bidi characters escaped); everything else shows size and
   SHA-256. Paths are sanitised and identified by the 16-hex path hash.
   Budgets: 200 000 entries, 64 levels; a cut list refuses "apply all".
7. **Apply.** `SandboxRequest.apply_changes` with the chosen ids (empty: all)
   and `overwrite`. Every chosen change is checked first (refused, conflict
   without `overwrite`); one failure writes nothing and the layer waits.
   Writing: the parent opened beneath the project without symlinks, missing
   directories created step by step with `O_NOFOLLOW` and the upper layer's
   mode; a file under its own name (`O_CREAT | O_EXCL | O_NOFOLLOW`), mode
   `& 0o777`, `fsync`, `renameat` over the target (replaces a symlink, never
   follows it); a symlink via `symlinkat` and `renameat`; a deletion without
   following a symlink. The rest is discarded; the layer goes. A write that
   fails midway answers `partial` with what was written, and the layer waits.
8. **Discard.** `SandboxRequest.discard_changes` removes the layer: never
   following a symlink, giving directories without permissions `0700`,
   flattening trees deeper than 64 levels instead of recursing. The daemon's
   end discards a waiting layer, its start sweeps layers a crashed daemon left
   behind. A waiting layer holds the daemon against `retire_if_idle`.
9. **Recording.** After apply or discard the run's summary row
   (`session_summaries`) carries `disposable: {outcome, applied, discarded}`.
10. **Protocol.** `PROTO_MINOR` 25, `Info.capabilities` names
    `sandbox.disposable`, `docs/PROTOCOL.md`; a daemon before 25 reads the new
    operations as `status`.
11. **Command line.** `humanitl sandbox changes [--stat]` (list with ids, then
    the diffs), `humanitl sandbox apply [ID ...] [--overwrite] [--yes]` and
    `humanitl sandbox discard [--yes]`, both asking on the terminal and
    refusing without one unless `--yes`. `humanitl run` says after the end how
    many changes wait and names the three commands.

### Security
A new mount mode and a new write path back to the host: `docs/SECURITY.md`
(section 5.2, "The disposable sandbox", the CI table and the changelog) and
`docs/THREAT-MODEL.md` (K-24, table and change history) describe it in the
same change. The statement in `README.md` is untouched: the network, the one
door and the filter are the same in both modes; the disposable mode only
removes the write path to the project for the duration of the run. Declared
limits: the size cap acts with a 250 ms interval below the hard limit of the
runtime tmpfs; the conflict check compares type, size, `mtime` and mode, not
content; requests the agent sent are out regardless of the layer; a daemon
that ends drops a waiting layer.

### Acceptance criteria
1. A disposable run changes nothing in the project folder while it runs;
   `extra_rw` is read-only.
2. After the end, `humanitl sandbox changes` lists files changed, created and
   deleted, with a text diff or size and hash.
3. `apply` writes all or the chosen changes, never a symlink out of the
   project, a device, a setuid bit or through a symlink in the project, and
   never over a file changed since the start without `--overwrite`; nothing
   is written when one check fails.
4. `discard` and every apply leave no layer behind; the recording names the
   decision and the files.
5. The layer cannot grow beyond its cap without the sandbox being stopped.
6. bubblewrap below 0.11, a runtime directory off tmpfs and a read-only
   project are refused with `SANDBOX_030` and a fix.

### Umsetzung (HUM-350)
- **Sandbox-Crate.** Neues Modul `daemon/crates/sandbox/src/disposable/`:
  `mod.rs` (`Layer` mit `create`, `discard`, `sweep`, `Budget`, `SizeGuard`
  über `statvfs`, `ensure_ended`, `baseline`, `DisposableRecord`,
  `remove_tree_at` mit flachem Umhängen ab Tiefe 64), `scan.rs` (Whiteouts,
  opake Verzeichnisse, Kopie nach oben, Konflikte gegen die Grundlage,
  Ablehnungen), `apply.rs` (`select` über den Pfad-Hash, `apply` mit
  Vorprüfung, `ensure_dir`, `write_file` über `O_EXCL` und `renameat`),
  `text_diff.rs` (Myers, `diff -u`, Maskierung). `LaunchInputs::overlay`,
  `SandboxProfile::mount_work`, `BwrapBackend::with_overlay` und
  `check_overlay` (`MIN_OVERLAY_VERSION` 0.11.0). `sha2` als
  Workspace-Abhängigkeit der Crate (gestreamter Hash großer Dateien).
- **Konfiguration.** `SandboxRef::mode` mit `SandboxMode` (`direct`,
  `disposable`), Projekt-Profil gesperrt (CONVENTIONS 4.11), Schema-Fixture
  und `docs/CONFIG.md` neu erzeugt.
- **Daemon.** Neues Modul `daemon/crates/ipc/src/disposable.rs` (`Waiting`,
  Wire-Form, Aufzeichnung); in `sandbox.rs` `launch` → `launch_with` und
  `watch_layer`, `settle_disposable`, `disposable_changes`, `apply_changes`,
  `discard_changes`, `drop_layer`, `finish_layer`; `retire_if_idle` und
  `shutdown` kennen die wartende Schicht; die Zeile der Vorschau zeigt das
  Overlay mit `LAYER`, die Mount-Tabelle den Ordner als `ro`. `humanitld`
  räumt beim Start liegengebliebene Schichten weg. Fake: die drei
  Operationen antworten `SANDBOX_031`.
- **Befunde.** `SANDBOX_030` (Wegwerf-Sandbox nicht möglich), `SANDBOX_031`
  (Änderung nicht übernommen, nichts wartet, Sandbox läuft noch),
  `SANDBOX_032` (Schicht voll); Bereich `SANDBOX` bis 32 erweitert,
  `docs/DIAGNOSTICS.md`, ARB en/de und `diagnostic_l10n.dart`.
- **Vertrag.** `SandboxRequest.changes|apply_changes|discard_changes`,
  `Start.sandbox_mode`, `SandboxEvent.changes|changes_outcome`,
  `DisposableChange`, `DisposableChanges`, `DisposableOutcome`, `PROTO_MINOR`
  25, `proto/descriptor.binpb` und `proto/generated.sha256` neu
  (protoc 27.5, protoc_plugin 25.0.0); die App überspringt die beiden neuen
  Ereignisse bis HUM-351 (`grpc_daemon_client.dart`).
- **Kommandozeile.** `bin/humanitl/src/cmd/changes.rs`, `--disposable` an
  `run` und `sandbox run`, `parity.rs`, `docs/reference/cli.md` und
  `parity.md` (`cargo xtask docs`), `docs/cli.md` („A disposable session").
- **Tests.** Einheiten in `disposable/tests.rs` (Kopie nach oben ohne
  Änderung, hinzugefügt/geändert/Rechte, Whiteout für Datei und Baum, opakes
  Verzeichnis, FIFO abgelehnt, setuid und harte Verweise, Verzeichnis ohne
  Rechte, Symlink nach draußen abgelehnt ohne Schreiben, Symlink im Projekt
  nach dem Start, Datei ersetzt Symlink ohne ihm zu folgen, Konflikte für
  Datei, Teilbaum und neu angelegte Datei, Auswahl, neue Verzeichnisse,
  Verwerfen samt `chmod 000`, `work/work` und 192 Ebenen, 20 000 Ebenen mit
  1 MiB Stapel, Aufräumen, privates Verzeichnis und tmpfs, Grenze Bytes und
  Einträge, Zombie), `text_diff` (Kontext, neue und gelöschte Datei, binär,
  zwei Abschnitte, Maskierung, Obergrenze). Mit echtem bwrap:
  `launcher.rs::a_disposable_sandbox_never_writes_to_the_project` (auch als
  ESC-5-Fall), Plan-Tests für Overlay, Version und `ro`;
  `ipc/tests/sandbox_start.rs::a_disposable_run_keeps_the_project_and_applies_only_what_was_chosen`
  über die RPC samt zweitem Start (`SANDBOX_030`) und Aufzeichnung.
  `proto_contract` kennt die neuen Felder; `bwrap_args` rendert das Overlay.
- **Mutationsbeweise**, alle Dateien danach mit `cmp` byte-gleich zurück:
  Overlay-Zweig in `to_bwrap_args` übergangen: `a_disposable_sandbox_never_writes_to_the_project`
  rot („the project is untouched“) und der Plan-Test rot; Prüfung `escapes`
  im Diff aus: `apply_refuses_a_symlink_out_of_the_project_and_writes_nothing`
  rot; `chmod` in `open_dir_forcing` aus: `discard_leaves_no_upper_layer_behind`
  rot; flaches Umhängen aus: `discard_flattens_a_tree_deeper_than_the_stack`
  bricht mit Stapelüberlauf ab; Grenze greift nie: `the_size_cap_holds` rot;
  Konfliktprüfung aus: `a_file_the_user_changed_since_the_start_is_not_overwritten_unasked`
  rot; Sonderbits behalten: `setuid_and_setgid_bits_are_stripped_and_hardlinks_split`
  rot.

### Kandidaten (HUM-350)
- (A) Fundscan (`SANDBOX_023`, `SANDBOX_026`) über die obere Schicht, bevor
  übernommen wird: Heute bekommt nur ein direkter Lauf die Funde zu
  Geheimnissen und zu Dateien, die der Rechner selbst ausführt.
- (B) Die Konfliktprüfung über einen Hash statt über `mtime` und Größe für
  Dateien unter einer Grenze; heute fällt eine Änderung, die beides erhält,
  nicht auf.
- (C) `sandbox.disposable_max_mb` als Einstellung statt der festen 2 GiB.
- (D) Eine wartende Schicht über einen Neustart des Daemons hinweg behalten
  (Grundlage mit ablegen), statt sie beim Abschied zu verwerfen.
- (E) `humanitl sessions summary` zeigt die Entscheidung über eine
  Wegwerf-Sandbox (heute nur im JSON der Aufzeichnung).
- (F) Die Mount-Tabelle bekommt eine eigene Art `overlay` statt `ro`.
- (G) Die Kosten des Text-Diffs über eine ganze Liste deckeln (Zeit oder Bytes
  insgesamt): Myers bleibt je Datei unter 1000 Änderungen, aber viele Dateien
  knapp darunter kosten zusammen viel.

### Review Runde 1 (HUM-350)
Codex (C) und Opus (O), alle Befunde übernommen; je ein Test, der ohne die
Behebung rot ist (Mutationsbeweis, Dateien danach mit `cmp` zurück).
- **O1 Symlink-Paar nach draußen.** `escapes` war lexikalisch; `sub/d -> ..`
  plus `e -> sub/d/..` landete über dem Projekt. Jetzt `target_stays_inside`:
  `..` nur am Anfang, höchstens so viele wie die eigene Tiefe, nichts
  Absolutes; in Diff und Übernahme. Test `a_symlink_pair_that_leads_out_is_refused`.
- **O2 Löschungen unter opaken Verzeichnissen.** `under_opaque` reist durch den
  Lauf; darunter verdeckt jedes Verzeichnis die untere Schicht. Test
  `deletions_below_a_plain_directory_inside_an_opaque_one_are_seen`.
- **C1 Projekt per Deskriptor.** `Waiting::lower` hält den vor dem Start
  geöffneten Ordner; das Backend bekommt seinen aufgelösten Ort als
  `approved_work_dir`. Test `ipc/tests/sandbox_start.rs::apply_writes_into_the_folder_that_was_started_even_if_its_path_moved`
  (echtes bwrap).
- **C3/O5 Grundlage vor dem Start** (`baseline_at` auf demselben
  Deskriptor, vor `launch_with`). Kein deterministischer Test: Der Fehler
  war ein Zeitfenster.
- **C4/O4 Grenze ab Anlegen der Schicht.** `Layer` misst beim Anlegen,
  `SizeGuard::start` vergleicht damit. Test
  `the_cap_counts_what_was_written_before_the_guard_started`.
- **C2 Änderung während der Übernahme.** Vor jedem `renameat`, `unlinkat`,
  `mkdirat` wird gegen den Stand der Vorprüfung verglichen; Abweichung heißt
  `partial`. Restfenster in SECURITY.md 5.2 und K-24. Test
  `a_file_changed_while_applying_stops_with_partial`.
- **C5 Grenze in `sandbox run --disposable`.** `Throwaway` startet denselben
  `SizeGuard`, öffnet den Ordner einmal und nimmt die Grundlage davor. Kein
  Test ohne Daemon.
- **C6 Nur Rechte.** Auch eine reine Rechteänderung schreibt einen neuen
  Inode. Test `a_mode_change_writes_a_new_inode_and_leaves_a_hard_link_outside_alone`.
- **C7 Verzeichnis `000`.** Anlegen mit Besitzer-`rwx`, endgültige Rechte am
  Ende, tiefste zuerst. Test `a_directory_without_permissions_is_filled_before_its_mode_is_set`.
- **C8** Rechte von Verzeichnissen zählen für Konflikte. Test
  `a_directory_whose_mode_the_user_changed_is_a_conflict`.
- **C9/O3** `redirect`/`metacopy` auch an Dateien. Test
  `a_file_with_a_foreign_overlay_attribute_is_refused`.
- **O6** Der Satz nennt die abgeschnittene Grundlage und ihre Zahl. Test
  `a_cut_baseline_says_why_every_addition_is_a_conflict`.
- **O7** Ein durch ein Verzeichnis ersetzter Symlink steht beim Namen in der
  Ablehnung. Test `a_symlink_replaced_by_a_directory_is_named_in_the_refusal`.
- **O8** `chmod_nofollow` über `O_PATH | O_NOFOLLOW` und `/proc/self/fd`
  statt `chmodat`. Test `changing_a_mode_never_follows_a_symlink`.
- **O9** `partial` nach einer Löschung nennt `0700`-Verzeichnisse und
  `.humanitl-flat-*`. Test `a_removal_that_fails_names_what_may_be_left`.
- **O10** U+061C, U+2028, U+2029, U+2060 bis U+2064 maskiert. Test
  `line_separators_and_invisible_marks_are_shown_masked`.
- **O11** `process_lives` vergleicht die Startzeit (Feld 22). Test
  `a_reused_process_number_is_not_the_sandbox`.

### Review Runde 2 (HUM-350)
Codex, Delta der Runde 1; alle fünf übernommen, dazu die Härtung der Tests
über `statvfs`. Mutationsbeweise wie oben, Dateien danach mit `cmp` zurück.
- **Inode statt Ort.** `BwrapBackend::with_expected_work_inode`: Das Backend
  hängt den Projektordner nur ein, wenn sein Deskriptor Gerät und Inode des
  Ordners trägt, den Daemon und `sandbox run --disposable` vor dem Start
  offen halten; sonst `SANDBOX_006`. Test
  `launcher.rs::plan_of_a_disposable_sandbox_mounts_only_the_folder_the_caller_holds`.
- **Löschen Eintrag für Eintrag.** `Expected` hält zum Eintrag den Baum
  darunter aus der Vorprüfung; `remove_checked` vergleicht jeden Eintrag
  unmittelbar vor dem Löschen, ein neuer oder geänderter hält an
  (`partial`). Test `removing_a_directory_stops_at_what_the_user_changed_during_apply`.
- **Aufgeschobene Rechte geprüft.** `DirModes` merkt Gerät, Inode und Rechte
  beim Vormerken und setzt nur, wenn alles noch stimmt, über den geprüften
  Deskriptor. Test `a_deferred_mode_is_not_set_on_a_directory_that_changed`.
- **`RENAME_NOREPLACE` beim flachen Umhängen**, bei `EEXIST` der nächste
  Name. Test `flattening_never_replaces_an_existing_directory`.
- **Verdeckte Löschungen zählen** gegen das Budget von 200 000 Einträgen.
  Test `hidden_deletions_count_against_the_budget`.
- **Tests über `statvfs`** laufen nacheinander (ein `Mutex`), schreiben bis
  zur Auslösung weiter (Größe und Einträge) beziehungsweise 64 MiB gegen
  1 MiB vor dem Start des Wächters; die Mutationsbeweise bleiben rot.

### Review Runde 3 (HUM-350)
- **Unlesbarer Teilbaum.** `Expected::observe` lieferte bei einem
  Verzeichnis, das sich nicht öffnen oder nicht vollständig lesen ließ, still
  keinen Baum, und das Löschen lief dann ohne Vergleich je Eintrag. Jetzt ist
  das ein Fehler der Vorprüfung: nichts wird geschrieben. Test
  `an_unreadable_directory_is_never_removed_unchecked`; Mutation (wieder
  `None`) rot.

### Review Runde 4 (HUM-350)
- **Datei durch Verzeichnis ersetzt.** Ohne Baum der Vorprüfung löschte
  `remove` rekursiv; ein Verzeichnis, das der Mensch nach der Prüfung an die
  Stelle einer Datei legte, verschwand samt Inhalt. Jetzt nur ein `unlinkat`
  ohne `AT_REMOVEDIR` (`EISDIR`, `partial`). `remove_tree_at` läuft nur noch
  über der oberen Schicht (Verwerfen, Aufräumen), im Projekt nie. Test
  `removing_a_file_never_deletes_a_directory_that_took_its_place`; Mutation
  (wieder rekursiv) rot.

---
## HUM-351 · The app shows the changes of a disposable sandbox and applies or discards them

Follow-up of HUM-350, which built daemon, RPC and command line. This issue is
the app half.

### Specification
1. **Switch.** The start flow (Sandbox page and the start in the empty
   queue) gets a switch "Disposable" next to the project folder, shown only
   when `Info.capabilities` names `sandbox.disposable`. It sends
   `Start.sandbox_mode`; its default is `sandbox.mode` from `GetConfig`.
   The settings form offers `sandbox.mode` as a basic setting.
2. **Changes panel.** When a disposable run ends, the app receives
   `SandboxEvent.changes` on the start stream, or asks with
   `SandboxRequest.changes` after a reconnect. A panel on the Sandbox page
   lists the changes: path (sanitised, with the 16-hex id as tooltip), kind
   (added, modified, deleted, symlink, mode), size, and the badges
   "conflict" (with the daemon's sentence), "never applied" (with its
   reason), "mode bits dropped". A row expands to its `diff` in a monospace
   view, or size and SHA-256 for binary entries.
3. **Selection.** Every applicable row has a checkbox, checked by default;
   refused rows cannot be checked; a conflicting row is unchecked by default
   and checking it turns on "overwrite" with a confirmation. Buttons:
   "Apply selected" (confirmation dialog naming the count and the folder,
   sends `apply_changes` with the chosen ids) and "Discard all"
   (confirmation, `discard_changes`). The outcome replaces the panel with a
   one-line result; `SANDBOX_031` shows as a notice with its fix.
4. **Start guard.** While changes wait, the start button explains that the
   last disposable run waits for a decision (`SANDBOX_030`) and offers to
   open the panel.
5. **Fake.** `HUMANITL_FAKE=disposable` plays a run with a modified text
   file, a new file, a deleted file, a symlink out of the project and a
   conflict; `fake_daemon_client.dart` answers the three operations.
6. **Strings.** All in ARB (en source, de translation); `proto_version.dart`
   follows to 25.

### Acceptance criteria
1. A disposable start from the app writes nothing to the project folder; the
   panel appears after the run without a click.
2. Applying a subset writes exactly that subset; a refused row cannot be
   selected; a conflict needs an explicit confirmation.
3. Discard leaves nothing waiting; the start works again.
4. Widget tests for the panel (rows, badges, diff, selection, both
   confirmations) and a golden in both themes.

---
## HUM-352 · The floating terminal keeps its resize grip in the corner and grows equally in every corner

### Kontext
Befund des Owners am 2026-10-30, mit Bildschirmfotos, zum schwebenden
Terminal aus HUM-300 und HUM-315. Unten rechts angedockt saß der Griff zum
Ändern der Größe oben links unter der Titelleiste, und Titel „Agent
terminal" wie Hinweiszeile („Agent output is untrusted…") begannen unter
seinen schrägen Strichen. Unten rechts ließ sich das Fenster außerdem nicht
so weit nach links ziehen wie oben rechts. Am selben Tag hat der Owner den
Umfang erweitert: Das Fenster soll sich wie ein Teil der Anwendung anfühlen,
nicht wie ein Nachtrag, mit neun Ankern und einem Einrasten, das den Schwung
der Hand ernst nimmt. Dazu kam ein dringender Fehler aus demselben Code: Über
History, schwebend, „+ Shell" und ein Wechsel des Reiters ließen das Fenster
mehrmals je Sekunde zwischen einem großen Fenster links oben und einem
kleinen rechts oben springen; auch nach Schließen und erneutem Öffnen kam es
mit dem Reiter der Shell sofort zurück.

**Die Ursachen.** (1) Der Griff begann unten verankert bei 28 px, also unter
der Titelleiste und auf dem Banner (Review-Runde 1 von HUM-300 hatte ihn
dorthin gelegt, weg von den Knöpfen). (2) Die Rechnung der Größe ist an
allen Ecken dieselbe (`terminalFloatMaxIn`); auf Intercept hielt aber die
Leiste mit Freigeben und Blockieren unten in der Mitte auch dann frei, wenn
nichts wartete, sodass das Fenster unten rechts nie über das Kontext-Pane
hinauswuchs. Außerdem verwarf `_resized` einen Schritt, der in einen
freizuhaltenden Bereich reichte, ganz und hielt damit um die Länge des
letzten Zeigerschritts vor der Kante. (3) Der Hinweis zur ersten Shell
(`TerminalTabNotices`, HUM-314) steht in einer `HNoticeLayer` im
schwebenden Fenster selbst und meldete sich als `HKeepClear`: Das Fenster
wich sich selbst aus, nahm den Hinweis mit, wurde dabei kleiner, und im
nächsten Bild war die gewählte Ecke wieder frei. Dazu baute die verborgene
Sandbox-Seite nur neu, wenn das Terminal zu ihr zurückkam, nicht wenn es
ging; nach „Schließen" und erneutem Schweben blieb sie ein zweiter Wirt
derselben Sitzung.

### Spezifikation
1. **Neun Anker.** `FloatCorner` wird `FloatAnchor` mit neun Werten auf
   einem Raster von drei mal drei (`x`, `y` je -1, 0, 1): die vier Ecken,
   die Mitten der vier Kanten (`topCenter`, `bottomCenter`, `centerLeft`,
   `centerRight`) und `center`. Die Ecken behalten ihre Namen, ein in der
   Zustandsdatei gespeicherter Anker (`terminalFloatCorner`) überlebt das
   Update. Ein Anker hält `x3` Abstand zu den Kanten, an denen er hängt,
   und liegt auf einer mittleren Achse mittig (`terminalFloatRectAt`, eine
   Regel für alle). Ausweichen geht wie bisher in die Ecken, ausgehend von
   der Ecke des Ankers (`homeCorner`: auf einer mittleren Achse rechts
   beziehungsweise unten); eine Mitte nimmt das Fenster nur, wenn sie
   gewählt ist.
2. **Der Griff.** Er sitzt bündig in der Ecke des Fensters gegenüber dem
   Anker (`gripX`, `gripY`); auf einer mittleren Achse rechts beziehungsweise
   unten, also in der Ecke zur Mitte des Fensters hin, auf der Seite der
   Vorgabe. Seine Striche liegen in dieser Ecke. Steht er oben, rücken
   Titelleiste und Banner auf seiner Seite um seine Breite ein
   (`floatingTerminalHeaderInset`, `TerminalPane.headerInset`); unten
   rückt nichts ein.
3. **Größe überall gleich.** Am Griff wächst das Fenster weg von den Kanten,
   an denen es hängt, auf einer mittleren Achse nach beiden Seiten um den
   doppelten Weg der Hand, an jedem Anker bis zu `terminalFloatMaxIn`. Die
   Rechnung steht rein in `growTerminalFloat`: Ein Schritt, der in einen
   freizuhaltenden Bereich reichte, wird bis an dessen Kante genommen statt
   verworfen. Die Leiste mit Freigeben und Blockieren hält nur frei, solange
   etwas wartet (`HKeepClear.enabled`); kommt eine Anfrage, weicht das
   Fenster aus wie vor jedem anderen Bereich.
4. **Einrasten mit Schwung.** Beim Loslassen zählt die Geschwindigkeit aus
   `DragEndDetails` (`FloatAnchor.release`): unter
   `terminalFloatFlingSpeed` (400 px/s) der nächste Anker, darüber vom
   nächsten Anker ein Schritt auf dem Raster in Richtung des Wurfs, in acht
   Richtungen geschnitten und nie über das Raster hinaus. Ein kurzer Wurf
   aus einer Ecke landet beim Nachbarn, nicht zurück am Start. Das Fenster
   gleitet in `HMotion.arrive` mit `enter` an seinen Platz, nur der Ort,
   nie die Größe; unter reduzierter Bewegung springt es
   (`HReducedMotion.displace`).
   Ein abgebrochener Zug (`onPanCancel`, etwa wenn ein anderer Erkenner
   gewinnt) zählt als ruhiges Loslassen ohne Geschwindigkeit: Nach echtem
   Weg rastet das Fenster am nächsten Anker ein und speichert ihn, ohne Weg
   bleibt alles, wie es war. Der Weg des Gleitens wird geprüft, nicht nur
   das Ziel: Berührt das überstrichene Rechteck einen freizuhaltenden
   Bereich, springt das Fenster. Ein Zug während des Gleitens beginnt dort,
   wo das Fenster gerade steht.
5. **Kein Ausweichen vor sich selbst.** `HKeepClearExempt` in `packages/ui`
   nimmt jeden `HKeepClear` darunter aus; die schwebende Ebene legt ihn um
   das Fenster. Die Sandbox-Seite baut bei jedem Wechsel des Wirts neu, auch
   verborgen und auch beim Gehen.
6. Streifen (HUM-315), Andocken und Schweben bleiben, wie sie sind; der
   Anker wird gespeichert wie bisher die Ecke. `docs/UX.md` (5.2 und die
   Tabelle der Bewegungen) und `docs/DESIGN.md` beschreiben Anker, Griff,
   Einrasten und die Ausnahmen von `HKeepClear`.

### Akzeptanzkriterien
1. Widget-Test: An jedem der neun Anker liegt der Griff bündig in der Ecke
   gegenüber und schneidet weder Titel noch Hinweis noch einen Knopf.
2. Widget-Test: An jedem Anker erreicht der Griff dieselbe größte Größe;
   auf Intercept mit leerer Warteschlange wächst das Fenster unten rechts so
   groß wie oben rechts.
3. Widget-Test: Jeder Anker ist mit einem ruhigen Zug erreichbar und wird
   gespeichert.
4. Widget-Test: Ein Wurf aus jeder Ecke zu jedem ihrer beiden Nachbarn
   landet dort; derselbe Weg langsam landet wieder am Start.
5. Widget-Test: History, schwebend, groß links oben, „+ Shell", zurück zum
   Agenten: Kein Bereich im Fenster hält frei, 60 Bilder lang steht das
   Fenster still, genau eine `TerminalView`. Dann schließen,
   `Ctrl+Shift+P`, Reiter der Shell: dasselbe.
6. Reine Tests: `terminalFloatRectAt` für alle Anker, `FloatAnchor.release`
   (ruhig und geworfen), `growTerminalFloat` (gleiche Grenze überall, Halt an
   der Kante), Ausweichen aus einer Mitte; `HKeepClear.enabled` und
   `HKeepClearExempt`.
7. Mutationsbeweise: ohne Einrücken, mit der Leiste, die immer freihält, ohne
   Geschwindigkeit, ohne einen Anker, mit dem Fenster, das sich selbst
   ausweicht, und mit der Sandbox-Seite, die nur beim Kommen neu baut.

### Tests
`app/test/features/shell/terminal_float_test.dart` (Kriterien 1 bis 4; der
Test zum Griff ersetzt `the_grip_keeps_off_the_title_buttons`),
`app/test/features/sandbox/terminal_tabs_test.dart` (Kriterium 5),
`app/test/core/ui/terminal_float_test.dart` (Kriterium 6),
`app/packages/ui/test/keep_clear_test.dart`.

### Fallstricke
- `flutter test` spielt Android; alle Fenster-Tests laufen mit
  `TargetPlatformVariant.only(TargetPlatform.linux)`.
- Ein Zug aus `tester.drag` hat keine Geschwindigkeit und gilt als ruhiges
  Loslassen; nach dem Loslassen läuft das Gleiten `HMotion.arrive` lang, die
  Tests pumpen es aus (`glideHome`).
- Eine `HKeepClear`, die sich nur verschiebt, meldet nichts, und im Test
  baute die Ebene ohne äußeren Anlass nicht neu; das Springen trat dort
  nicht von selbst auf. Der Regressionstest beginnt deshalb wie beim Owner
  groß (900 x 500) links oben und prüft die Ursache direkt: Kein Bereich,
  den `HKeepClear.rectsIn` liefert, überlappt das Fenster.

### Review-Runde 1 (Codex, Sonnet für Antigravity)
- **Editor ohne freizuhaltenden Bereich** (Codex, major): Der Editor einer
  gehaltenen Anfrage ersetzt die Aktionsleiste und hielt nichts frei; unten
  in der Mitte lag das Fenster auf Senden und Verwerfen. Jetzt halten die
  Bedienelemente unten im Editor (Leiste, beide Pausen) und die Rückfrage
  zum Pseudonymisieren frei. Test
  `the_window_keeps_off_the_controls_of_the_editor` (unten Mitte und unten
  rechts); Mutation (ohne `HKeepClear` um `_bottom`) rot.
- **Gleiten über die Leiste** (Codex major, Sonnet minor): Geprüft war nur
  das Ziel. Berührt der Weg einen freizuhaltenden Bereich, springt das
  Fenster jetzt. Test `a_window_let_go_over_the_bar_never_glides_across_it`;
  Mutation (Weg ungeprüft) rot.
- **Zug mitten im Gleiten** (Codex, Sonnet): `_shown` hielt das Ziel, ein Zug
  begann dort. Jetzt hält es den gezeichneten Ort. Test
  `a_drag_during_the_glide_starts_where_the_window_is`; Mutation rot.
- **Abgebrochener Zug** (Sonnet, minor): rastet nach echtem Weg am nächsten
  Anker ein und speichert ihn; so gewollt und in Spezifikation 4 sowie
  `docs/UX.md` beschrieben.

### Kandidaten (HUM-352)
- (A) Freie Lage: Mit gehaltener Umschalttaste beim Loslassen bliebe das
  Fenster, wo es ist, statt einzurasten. Vom Owner als erlaubt genannt,
  hier nicht gebaut: Es braucht eine Speicherung der freien Lage und eine
  Regel, wie eine freie Lage ausweicht.
- (B) Die Leiste mit Freigeben und Blockieren könnte nur die Knöpfe
  freihalten statt ihrer ganzen Breite.
