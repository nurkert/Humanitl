# Fuzzing des Daemons

Der Daemon liest Bytes, die ein Angreifer kontrolliert: Anfragen eines
möglicherweise unterwanderten Agenten, gepackte Bodies, Regel-Dateien aus
fremder Hand, Filterausdrücke und ein Audit-Log, das jemand verändert haben
könnte. Dieses Verzeichnis enthält je Eingang ein Fuzz-Target (HUM-056,
BACKLOG.md 4.4, ADR-001).

## Die Targets

| Target | Eingabe | Was aufgerufen wird | Prüfung |
|---|---|---|---|
| `http_request_path` | rohe Bytes einer HTTP/1.1-Verbindung | hyper wie in `handler::serve_connection`, dann `check_authority`, `requested_upgrade`, `RuleSet::evaluate` | `crates/proxy/tests/regressions/check.rs` |
| `chunked_decoder` | Bytes hinter einem festen `Transfer-Encoding: chunked`-Kopf | hyper, dann `body::buffer` mit Cap 512 Bytes | `crates/proxy/tests/regressions/check.rs` |
| `body_decoder` | erstes Byte wählt die Kodierung (0 identity, 1 gzip, 2 deflate, 3 br, sonst unbekannt), der Rest ist der Body | `decode::decode_body` mit 8 MiB Cap und Ratio 100 | `crates/findings/tests/regressions/check.rs` |
| `rules_yaml` | Bytes, als UTF-8 versucht | `parse_rules_for_session`, `serialize_rules`, `RuleSet::evaluate` | `crates/rules/tests/regressions/check.rs` |
| `flow_filter` | Bytes, als UTF-8 versucht | `parse_filter` | `crates/recorder/tests/regressions/check.rs` |
| `audit_verify` | Bytes als `audit.jsonl`; die ersten drei Bytes kippen zusätzlich ein Byte einer gültigen Kette | `AuditVerifier::verify_reader` | `crates/audit/tests/regressions/check.rs` |

Jedes Target ist eine Zeile: Es bindet die Prüfung aus `check.rs` der Crate
über `#[path]` ein. Die Regressionstests derselben Crate rufen dieselbe
Funktion auf. Was der Fuzzer prüft, prüft `cargo test` also mit genau
demselben Code, und eine Invariante, die man in `check.rs` ergänzt, gilt an
beiden Stellen. Die Invarianten selbst stehen im Doc-Kommentar der jeweiligen
Funktion.

Die Prüfungen des Proxys laufen durch hyper, nicht durch einen eigenen
Parser: Ein Parser nur für das Fuzzing fände Fehler, die der Proxy nicht hat,
und übersähe die, die er hat. Eine Abweichung vom Handler gibt es: `half_close`
ist an, weil der Test-Client nach seiner Eingabe die Schreibrichtung schließt
und hyper sonst die laufende Anfrage verwürfe.

## Lokal laufen lassen

Die Crate ist ein eigener Workspace und steht nicht in `daemon/Cargo.toml`;
`cargo build --workspace` bleibt dadurch auf Stable. libFuzzer und
AddressSanitizer brauchen Nightly:

```sh
cargo install cargo-fuzz --locked
cd daemon
cargo +nightly fuzz list
cargo +nightly fuzz run http_request_path -- -max_total_time=60 -timeout=10 -rss_limit_mb=512
```

`-timeout=10` und `-rss_limit_mb=512` gehören immer dazu. Ohne Zeitgrenze
findet ein Target keine Hänger, sondern bleibt auf einer quadratischen Eingabe
stehen; ohne Speichergrenze stirbt der Lauf an einer Bombe, ohne ein Artefakt
zu hinterlassen.

Ein Lauf schreibt neue Eingaben in `corpus/<target>/`. Eingecheckt sind dort
nur die Seeds; vor einem Commit gehören neue Dateien wieder weg
(`git clean -n daemon/fuzz/corpus` zeigt sie), oder man lässt den Lauf auf
einer Kopie arbeiten: `cargo +nightly fuzz run <target> /tmp/corpus-kopie`.

In CI läuft jedes Target jede Nacht zehn Minuten
(`.github/workflows/fuzz-nightly.yml`, auch per Hand über `workflow_dispatch`).

## Triage eines Funds

1. **Reproduzieren.** Das Artefakt `fuzz-artifacts-<target>` des roten Jobs
   herunterladen und lokal abspielen:
   `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<crash-file>`.
2. **Minimieren.** `cargo +nightly fuzz tmin <target> <crash-file>`.
3. **Ursache beheben**, in der Crate, in der sie liegt, nicht im Target. Eine
   Invariante zu lockern ist nur richtig, wenn sie falsch war; dann steht die
   Begründung im Doc-Kommentar von `check.rs`.
4. **Regressionstest ablegen.** Die minimierte Eingabe kommt nach
   `daemon/crates/<crate>/tests/regressions/<target>-<kurzbeschreibung>.bin`.
   Der Test `regressions_replay_<target>` in `tests/regressions/main.rs` spielt
   jede Datei mit diesem Präfix durch `check.rs` ab; ein eigener `#[test]` ist
   nur nötig, wenn der Fund eine schärfere Aussage verdient als „kein Panic".
5. **In den Corpus kopieren**, unter `daemon/fuzz/corpus/<target>/`, damit der
   nächste Lauf von dort weitersucht.
6. **Sicherheitsrelevanz prüfen.** Ein Speicherverbrauch über die Grenze oder
   ein Hänger im Proxy-Pfad bekommt einen Eintrag im Changelog von
   `docs/SECURITY.md`, und ist die betroffene Fassung schon veröffentlicht,
   folgt ein Patch-Release nach dem Ablauf aus HUM-060.
