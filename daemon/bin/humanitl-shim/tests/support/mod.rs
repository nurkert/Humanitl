//! Gemeinsames der PID-1-Integrationstests (`tests/shim.rs`,
//! `tests/door_backpressure.rs`): ob `bwrap` den Shim als PID 1 eines eigenen
//! Benutzer- und PID-Namensraums starten kann.
//!
//! Wo das nicht geht, überspringt ein Test mit dem Grund auf stderr, damit
//! `cargo test` auf einem Rechner ohne Namensräume grün bleibt. Unter
//! `STRICT=1`, wie in der CI, ist das Überspringen ein Fehler mit demselben
//! Grund: Sperrt ein Runner die Namensräume (etwa per `AppArmor`), wäre der
//! Test sonst grün, ohne etwas geprüft zu haben (HUM-458).

use std::process::{Command, Stdio};

/// Ob `bwrap --as-pid-1` mit eigenem Benutzer- und PID-Namensraum startet.
///
/// `false` mit dem Grund auf stderr, wo nicht; unter `STRICT=1` statt dessen
/// ein Abbruch des Tests mit dem Grund.
pub fn bwrap_as_pid_1_usable() -> bool {
    let probe = Command::new("bwrap")
        .args([
            "--unshare-user",
            "--unshare-pid",
            "--as-pid-1",
            "--dev-bind",
            "/",
            "/",
            "--proc",
            "/proc",
            "--",
            "true",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    let reason = match probe {
        Ok(output) if output.status.success() => return true,
        Ok(output) => format!(
            "bwrap exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(err) => format!("bwrap could not start: {err}"),
    };
    assert!(
        std::env::var("STRICT").as_deref() != Ok("1"),
        "bwrap --as-pid-1 is not usable, and STRICT=1 forbids skipping a PID-1 test: {reason}"
    );
    eprintln!("skipping: bwrap --as-pid-1 is not usable on this machine: {reason}");
    false
}
