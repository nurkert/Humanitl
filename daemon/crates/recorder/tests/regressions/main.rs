//! Regressionstests des Fuzz-Targets `flow_filter` (HUM-056).
//!
//! Jede Datei `flow_filter-<kurzbeschreibung>.bin` in diesem Verzeichnis läuft
//! durch dieselbe Prüfung wie im Fuzzer (`check.rs`). Wie eine Datei hierher
//! kommt, steht in `daemon/fuzz/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;

use std::path::{Path, PathBuf};

use humanitl_recorder::{FilterParam, parse_filter};

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
fn regressions_replay_flow_filter() {
    for (path, bytes) in inputs("flow_filter") {
        eprintln!("replaying {}", path.display());
        check::flow_filter(&bytes);
    }
}

/// Zwanzig Nutzlasten mit SQL-Metazeichen, jede in vier Stellungen: Keine
/// erscheint im `SQL`, das `SQL` bleibt aus den Bausteinen der Übersetzung, und
/// der Wert geht als Platzhalter-Wert hinein.
#[test]
fn filter_never_interpolates() {
    let payloads = [
        "' OR 1=1 --",
        "'; DROP TABLE flows; --",
        "\" OR \"\"=\"",
        "x' UNION SELECT * FROM rules --",
        "1); DELETE FROM flows; --",
        "%' OR '%'='",
        "_",
        "\\' OR 1=1",
        "'||(SELECT sqlite_version())||'",
        "a'/**/OR/**/'1'='1",
        "';ATTACH DATABASE '/tmp/x' AS y;--",
        "0x27",
        "char(39)",
        "' AND randomblob(1000000000) --",
        "`flows`",
        "[flows]",
        "$1",
        "?",
        ":name",
        "@p",
    ];
    for payload in payloads {
        for expression in [
            format!("path:\"{payload}\""),
            format!("host:{payload}"),
            format!("method:{payload}"),
            payload.to_owned(),
        ] {
            let Ok(filter) = parse_filter(&expression, check::NOW_MS) else {
                continue;
            };
            check::assert_sql_is_closed(&filter.sql);
            // `_` und `?` stehen als Teil von Spaltennamen und Platzhaltern
            // ohnehin im SQL; der Vergleich gilt den Nutzlasten, die etwas
            // anderes als ein Zeichen der Übersetzung tragen.
            let foreign = payload.contains(['\'', '"', ';', '-', '`', '[', '$', '@', '%']);
            assert!(
                !foreign || !filter.sql.contains(payload),
                "{payload:?} appears in {:?}",
                filter.sql
            );
        }
        let filter = parse_filter(&format!("method:{}", payload.replace(' ', "")), 0).unwrap();
        assert!(
            matches!(filter.params.as_slice(), [FilterParam::Text(_)]),
            "{:?}",
            filter.params
        );
    }
}

/// Die Prüfung des Targets erkennt einen Wert, der doch im `SQL` steht.
#[test]
#[should_panic(expected = "outside the translation")]
fn the_closed_sql_check_catches_a_literal() {
    check::assert_sql_is_closed("path LIKE '%x%' ESCAPE '\\'");
}
