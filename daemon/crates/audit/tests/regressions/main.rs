//! Regressionstests des Fuzz-Targets `audit_verify` (HUM-056).
//!
//! Jede Datei `audit_verify-<kurzbeschreibung>.bin` in diesem Verzeichnis
//! läuft durch dieselbe Prüfung wie im Fuzzer (`check.rs`). Wie eine Datei
//! hierher kommt, steht in `daemon/fuzz/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;

use std::path::{Path, PathBuf};

use humanitl_audit::{AuditVerifier, BreakReason, VerifyStatus};

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
fn regressions_replay_audit_verify() {
    for (path, bytes) in inputs("audit_verify") {
        eprintln!("replaying {}", path.display());
        check::audit_verify(&bytes);
    }
}

/// Die Seeds sagen, was ihr Name verspricht: Die gültige Kette hält mit fünf
/// Records, die mit gelöschter dritter Zeile bricht an Nummer 4, und die ohne
/// letzte Zeile hält mit vier, weil ohne Anker niemand das Ende kennt
/// (`docs/SECURITY.md`, „Was die Audit-Kette beweist").
#[test]
fn the_seeds_are_what_they_claim() {
    let verify =
        |bytes: &[u8]| AuditVerifier::verify_reader(bytes, Some(&check::KEY), &[]).unwrap();

    let valid = verify(check::VALID_CHAIN);
    assert_eq!(valid.status, VerifyStatus::Ok);
    assert_eq!(valid.records, 5);

    let deleted = verify(include_bytes!("audit_verify-line-three-deleted.bin"));
    assert_eq!(
        deleted.status,
        VerifyStatus::Broken {
            first_bad_seq: 4,
            reason: BreakReason::SeqGap
        }
    );

    let short = verify(include_bytes!("audit_verify-last-line-missing.bin"));
    assert_eq!(short.status, VerifyStatus::Ok);
    assert_eq!(short.records, 4);
}

/// Jedes Bit jedes Bytes einer gültigen Kette aus fünf Zeilen einmal kippen:
/// Die Kette hält danach nie.
#[test]
fn audit_flip_one_byte() {
    for position in 0..check::VALID_CHAIN.len() {
        for bit in 0..8 {
            check::assert_flip_breaks(position, 1 << bit);
        }
    }
}
