//! Regressionstests des Fuzz-Targets `rules_yaml` (HUM-056).
//!
//! Jede Datei `rules_yaml-<kurzbeschreibung>.bin` in diesem Verzeichnis läuft
//! durch dieselbe Prüfung wie im Fuzzer (`check.rs`). Wie eine Datei hierher
//! kommt, steht in `daemon/fuzz/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;

use std::path::{Path, PathBuf};

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
fn regressions_replay_rules_yaml() {
    for (path, bytes) in inputs("rules_yaml") {
        eprintln!("replaying {}", path.display());
        check::rules_yaml(&bytes);
    }
}

/// Der mitgelieferte Regelsatz besteht die Prüfung, und zwar über den
/// Rundweg: Er wird gelesen, nicht übersprungen.
#[test]
fn the_bundled_rules_pass_the_round_trip() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../rules/default.yaml");
    let text = std::fs::read_to_string(path).unwrap();
    assert!(humanitl_rules::parse_rules(&text).is_ok());
    check::rules_yaml(text.as_bytes());
}

/// Ungültiges UTF-8 und ein Muster mit Backtracking-Bombe sind kein Absturz.
#[test]
fn invalid_utf8_and_a_regex_bomb_are_tolerated() {
    check::rules_yaml(b"version: 1\nrules: \xff\xfe\n");
    check::rules_yaml(
        b"version: 1\nrules:\n  - action: block\n    match:\n      host: \"*.example.com\"\n      path: \"~^(a|a?)+$\"\n",
    );
    check::rules_yaml(
        b"version: 1\nrules:\n  - action: block\n    match:\n      host: \"*.example.com\"\n      path: \"~(\"\n",
    );
}
