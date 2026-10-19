//! Regressionstests des Fuzz-Targets `body_decoder` (HUM-056).
//!
//! Jede Datei `body_decoder-<kurzbeschreibung>.bin` in diesem Verzeichnis
//! läuft durch dieselbe Prüfung wie im Fuzzer (`check.rs`). Das erste Byte
//! wählt die Kodierung, der Rest ist der Body. Wie eine Datei hierher kommt,
//! steht in `daemon/fuzz/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use flate2::Compression;
use flate2::write::GzEncoder;
use humanitl_findings::decode::{ContentEncoding, decode_body};

/// Die Eingaben des Targets, nach Namen sortiert.
fn inputs(target: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressions");
    let prefix = format!("{target}-");
    let mut found: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "bin")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix))
        })
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "no regression input for {target} in {}",
        dir.display()
    );
    found
}

#[test]
fn regressions_replay_body_decoder() {
    for (path, bytes) in inputs("body_decoder") {
        eprintln!("replaying {}", path.display());
        check::body_decoder(&bytes);
    }
}

/// Ein gzip-Strom aus `mib` MiB Nullbytes, im Test erzeugt, nie eingecheckt.
fn zero_bomb(mib: usize) -> Vec<u8> {
    let block = vec![0u8; 1024 * 1024];
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    for _ in 0..mib {
        encoder.write_all(&block).unwrap();
    }
    encoder.finish().unwrap()
}

/// 200 MiB Nullbytes, gepackt: Das Entpacken hört am Budget auf, statt erst
/// alles zu entpacken und dann zu messen.
///
/// Die Spezifikation nennt `Err(DecodeError::RatioExceeded)` und 200 ms. Die
/// Crate meldet die Grenze nicht als Fehler, sondern als gekürzten Body mit
/// `FINDINGS_002` (HUM-025); das Budget ist hier `min(8 MiB, gepackt * 100)`.
/// Die Zeitgrenze ist großzügiger als 200 ms, weil der Test im Dev-Profil ohne
/// Optimierung läuft; sie trennt trotzdem „bricht beim Budget ab" (ein
/// Bruchteil einer Sekunde) von „entpackt 200 MiB" (viele Sekunden).
#[test]
fn decode_ratio_bomb() {
    let bomb = zero_bomb(200);
    let started = Instant::now();
    let decoded = decode_body(&bomb, &ContentEncoding::Gzip, check::CAP, check::RATIO);
    let elapsed = started.elapsed();
    assert!(decoded.truncated);
    let limit = check::CAP.min(bomb.len() * 100);
    assert_eq!(decoded.bytes.len(), limit);
    assert_eq!(
        decoded.diagnostic.as_ref().map(|found| found.code.as_str()),
        Some("FINDINGS_002")
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "decoding took {elapsed:?}"
    );
}

/// Der Speicherzuwachs beim Entpacken der Bombe bleibt unter 16 MiB.
///
/// Gemessen über den Höchststand des residenten Speichers (`VmHWM`), der
/// vorher über `/proc/self/clear_refs` zurückgesetzt wird. Der Wert gilt für
/// den ganzen Prozess, deshalb ist der Test `#[ignore]` und läuft allein:
/// `cargo test -p humanitl-findings --test regressions -- --ignored --test-threads=1`.
#[test]
#[ignore = "measures the whole process; run alone, Linux only"]
fn decode_ratio_bomb_peak_rss() {
    let bomb = zero_bomb(200);
    std::fs::write("/proc/self/clear_refs", "5").unwrap();
    let before = status_kib("VmRSS");
    let decoded = decode_body(&bomb, &ContentEncoding::Gzip, check::CAP, check::RATIO);
    let peak = status_kib("VmHWM");
    assert!(decoded.truncated);
    let growth = peak.saturating_sub(before);
    assert!(growth < 16 * 1024, "peak grew by {growth} KiB");
}

/// Ein Feld aus `/proc/self/status` in KiB.
fn status_kib(field: &str) -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find_map(|line| line.strip_prefix(field)?.strip_prefix(':'))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap_or_else(|| panic!("no {field} in /proc/self/status"))
}

/// Das erste Byte wählt die Kodierung, auch über die fünf Werte hinaus.
#[test]
fn the_check_reads_the_selector() {
    assert_eq!(check::encoding(1), ContentEncoding::Gzip);
    assert_eq!(check::encoding(3), ContentEncoding::Brotli);
    assert_eq!(
        check::encoding(9),
        ContentEncoding::Other("zstd".to_owned())
    );
}
