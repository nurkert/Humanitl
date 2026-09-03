//! Der Lint aus HUM-063 prüft sich selbst, und `cargo test` prüft den Lint.
//!
//! Ohne diesen Test liefe der Selbsttest des Skripts nur dort, wo jemand ihn
//! ausdrücklich aufruft. So läuft er in jedem `cargo test --workspace`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/ci/lint-no-string-errors.sh")
}

#[test]
fn the_lint_script_detects_its_fixtures() {
    let script = script_path();
    assert!(script.exists(), "{} is missing (HUM-063)", script.display());

    let output = Command::new("sh")
        .arg(&script)
        .arg("--self-test")
        .output()
        .unwrap_or_else(|err| panic!("cannot run {}: {err}", script.display()));

    assert!(
        output.status.success(),
        "self-test failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
