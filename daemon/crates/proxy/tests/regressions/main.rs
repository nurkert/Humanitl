//! Regressionstests der Fuzz-Targets `http_request_path` und
//! `chunked_decoder` (HUM-056).
//!
//! Jede Datei `<target>-<kurzbeschreibung>.bin` in diesem Verzeichnis ist eine
//! Eingabe, die einmal einen Fund ausgelöst hat oder einen Randfall festhält.
//! `regressions_replay` spielt sie alle durch dieselbe Prüfung, die auch der
//! Fuzzer aufruft (`check.rs`). Der Ablauf, wie eine neue Datei hierher kommt,
//! steht in `daemon/fuzz/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use humanitl_core::HostName;
use humanitl_proxy::body::BufferError;

/// Die Eingaben eines Targets, nach Namen sortiert.
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
fn regressions_replay_http_request_path() {
    for (path, bytes) in inputs("http_request_path") {
        eprintln!("replaying {}", path.display());
        check::http_request_path(&bytes);
    }
}

#[test]
fn regressions_replay_chunked_decoder() {
    for (path, bytes) in inputs("chunked_decoder") {
        eprintln!("replaying {}", path.display());
        check::chunked_decoder(&bytes);
    }
}

/// Die Prüfung sieht die Anfragen wirklich, auch zwei hintereinander auf
/// einer Verbindung, und normalisiert ihr Ziel.
#[test]
fn the_request_check_sees_every_request() {
    let targets = check::plain_targets(
        b"GET / HTTP/1.1\r\nHost: Example.COM.\r\n\r\nGET / HTTP/1.1\r\nHost: [::1]:8080\r\n\r\n",
    );
    assert_eq!(
        targets,
        vec![
            Some("example.com:80".to_owned()),
            Some("[::1]:8080".to_owned())
        ]
    );
}

/// Der IDN-Seed erreicht die Zielprüfung: Ein A-Label mit Großbuchstaben und
/// Punkt am Ende besteht `check_authority` und kommt normalisiert heraus. Der
/// Seed mit rohem UTF-8 im `Host` wird dagegen schon vorher abgelehnt; er
/// bleibt als eigener Fall für diesen Ablehnungspfad.
#[test]
fn the_idn_seeds_reach_the_target_check_or_are_refused() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressions");
    let a_label = std::fs::read(dir.join("http_request_path-idn-host.bin")).unwrap();
    assert_eq!(
        check::plain_targets(&a_label),
        vec![Some("xn--mnchen-3ya.example:80".to_owned())]
    );
    let raw = std::fs::read(dir.join("http_request_path-idn-host-raw-utf8.bin")).unwrap();
    assert_eq!(check::plain_targets(&raw), vec![None]);
}

/// Eine Chunk-Größe von `FFFFFFFFFFFFFFFF` endet mit einem Fehler und legt
/// keinen Puffer dieser Größe an: Die Antwort kommt sofort, und gepuffert wird
/// höchstens bis zum Cap.
#[test]
fn chunked_huge_size_no_alloc() {
    let started = Instant::now();
    let result = check::first_buffered(b"FFFFFFFFFFFFFFFF\r\nhallo\r\n0\r\n\r\n")
        .expect("hyper read no request at all");
    assert!(
        matches!(result, Err(BufferError::Read | BufferError::Cap)),
        "{result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

/// Ein gültiger Body aus zwei Chunks kommt vollständig an; damit ist belegt,
/// dass die Prüfung den Body wirklich liest und nicht nur Fehler sieht.
#[test]
fn two_chunks_arrive_whole() {
    let result = check::first_buffered(b"5\r\nhallo\r\n6\r\n welt!\r\n0\r\n\r\n")
        .expect("hyper read no request at all");
    assert_eq!(result.unwrap().as_ref(), b"hallo welt!");
}

/// Ein Body über dem Cap endet mit `Cap`, nie als vollständiger Body.
#[test]
fn a_chunked_body_over_the_cap_is_refused() {
    let size = usize::try_from(check::CHUNKED_CAP).unwrap() + 1;
    let mut input = format!("{size:x}\r\n").into_bytes();
    input.extend(std::iter::repeat_n(b'a', size));
    input.extend_from_slice(b"\r\n0\r\n\r\n");
    let result = check::first_buffered(&input).expect("hyper read no request at all");
    assert!(matches!(result, Err(BufferError::Cap)), "{result:?}");
}

/// Die Normalisierungs-Prüfung des Targets erkennt einen Host, der nicht
/// normalisiert ist. Ohne diesen Test könnte `assert_normalized` nichts prüfen
/// und `regressions_replay` bliebe trotzdem grün.
#[test]
#[should_panic(expected = "trailing dot")]
fn the_normalisation_check_catches_a_trailing_dot() {
    check::assert_normalized(&HostName::Dns("example.com.".to_owned()));
}

#[test]
#[should_panic(expected = "not lower case")]
fn the_normalisation_check_catches_upper_case() {
    check::assert_normalized(&HostName::Dns("Example.com".to_owned()));
}
