//! Die Umgehung der Zulassung bleibt im Testcode (HUM-431).
//!
//! `Backend::detect_unadmitted` startet eine Stufe, die noch nicht in
//! `ADMITTED_BACKENDS` steht. Sie steht hinter dem Feature
//! `unadmitted-backends`, und nur `escape-launch` und die Tests dürfen es
//! einschalten. Der Compiler prüft das nicht: Ein Eintrag
//! `features = ["unadmitted-backends"]` in `daemon/bin/humanitld/Cargo.toml`
//! baut und besteht jeden anderen Test. Dieser Test liest deshalb den Baum
//! und nennt jede Stelle außerhalb der erlaubten.
//!
//! Erlaubt sind: die Definition in `src/backend.rs` hinter ihrem `cfg`, das
//! Manifest dieser Crate, `escape-launch`, jede Datei unter einem
//! Verzeichnis `tests/` und Code nach dem ersten `#[cfg(test)]` einer Datei
//! (die Testmodule stehen hier am Ende). Dazu im Manifest von `humanitl-ipc`
//! genau die Zeile [`IPC_FORWARD`] und Kommentare: Mit ihr prüft ESC-5 das
//! Terminal jeder Stufe (HUM-435). Was `cargo tree` dazu sagt, prüft
//! `tools/check-deps.sh`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

/// Die beiden Namen, die außerhalb der erlaubten Stellen nicht vorkommen.
const NEEDLES: [&str; 2] = ["detect_unadmitted", "unadmitted-backends"];

/// Die eine Zeile im Manifest von `humanitl-ipc`, die das Feature an diese
/// Crate weiterreicht, damit `tests/terminal.rs` unter der Escape-Suite jede
/// Stufe starten kann (HUM-435). Eingeschaltet wird es dort wie hier nur von
/// einem Testaufruf.
const IPC_FORWARD: &str = "unadmitted-backends = [\"humanitl-sandbox/unadmitted-backends\"]";

fn daemon_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Alle `.rs`- und `Cargo.toml`-Dateien unter `dir`, ohne `target/`.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != ".git" {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") || name == "Cargo.toml" {
            out.push(path);
        }
    }
}

/// Die Stellen einer Datei, an denen ein Name außerhalb des Erlaubten steht,
/// als `Pfad:Zeile`.
fn offending(relative: &str, text: &str) -> Vec<String> {
    let whole_file_allowed = relative == "crates/sandbox/Cargo.toml"
        || relative == "crates/sandbox/src/bin/escape-launch.rs"
        || relative.split('/').any(|part| part == "tests");
    if whole_file_allowed {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut in_tests = false;
    let mut gated = false;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            in_tests = true;
        }
        // Die Definition: der Name einer Funktion direkt unter ihrem `cfg`.
        let definition = relative == "crates/sandbox/src/backend.rs"
            && (trimmed.starts_with("#[cfg(any(test, feature = \"unadmitted-backends\"))]")
                || (gated && trimmed.starts_with("pub fn detect_unadmitted(")));
        gated = relative == "crates/sandbox/src/backend.rs"
            && trimmed.starts_with("#[cfg(any(test, feature = \"unadmitted-backends\"))]");
        // Ein Doku-Kommentar nennt die Namen, ohne sie zu benutzen.
        let comment = trimmed.starts_with("//");
        // Im Manifest von `humanitl-ipc`: Kommentare und die Weiterleitung.
        let forward = relative == "crates/ipc/Cargo.toml"
            && (trimmed.starts_with('#') || trimmed == IPC_FORWARD);
        if in_tests || definition || comment || forward {
            continue;
        }
        if NEEDLES.iter().any(|needle| line.contains(needle)) {
            out.push(format!("{relative}:{}", number + 1));
        }
    }
    out
}

#[test]
fn the_bypass_appears_only_in_escape_launch_and_tests() {
    let root = daemon_dir();
    let mut files = Vec::new();
    for top in ["crates", "bin", "xtask"] {
        sources(&root.join(top), &mut files);
    }
    sources_of_manifest(&root, &mut files);
    assert!(
        files.len() > 50,
        "the walk found only {} files",
        files.len()
    );
    let mut found = Vec::new();
    for file in &files {
        let relative = file
            .strip_prefix(&root)
            .expect("under daemon/")
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(file).expect("read a source file");
        found.extend(offending(&relative, &text));
    }
    assert!(
        found.is_empty(),
        "detect_unadmitted or the feature unadmitted-backends outside escape-launch and tests: \
         {found:?}"
    );
}

/// Das Manifest des Arbeitsbereichs selbst, `daemon/Cargo.toml`.
fn sources_of_manifest(root: &Path, out: &mut Vec<PathBuf>) {
    out.push(root.join("Cargo.toml"));
}

/// Die Probe des Tests: Ein Eintrag im Manifest des Daemons und ein Aufruf
/// in seinem Code werden gefunden, die Definition und ein Test nicht.
#[test]
fn the_check_names_a_daemon_that_switches_the_bypass_on() {
    let manifest = "[dependencies]\nhumanitl-sandbox = { workspace = true, features = \
                    [\"unadmitted-backends\"] }\n";
    assert_eq!(
        offending("bin/humanitld/Cargo.toml", manifest),
        ["bin/humanitld/Cargo.toml:2"]
    );
    let code = "fn start() {\n    let b = Backend::detect_unadmitted(\"podman\", paths);\n}\n";
    assert_eq!(
        offending("crates/ipc/src/sandbox.rs", code),
        ["crates/ipc/src/sandbox.rs:2"]
    );
    let gated = "    #[cfg(any(test, feature = \"unadmitted-backends\"))]\n    pub fn \
                 detect_unadmitted(name: &str) {}\n";
    assert!(offending("crates/sandbox/src/backend.rs", gated).is_empty());
    // Ohne `cfg` darüber ist auch die Definition ein Fund.
    let ungated = "    pub fn detect_unadmitted(name: &str) {}\n";
    assert_eq!(
        offending("crates/sandbox/src/backend.rs", ungated),
        ["crates/sandbox/src/backend.rs:1"]
    );
    let test_module = "fn x() {}\n#[cfg(test)]\nmod tests {\n    fn t() { \
                       Backend::detect_unadmitted(\"podman\", p); }\n}\n";
    assert!(offending("crates/ipc/src/sandbox.rs", test_module).is_empty());
    assert!(offending("crates/ipc/tests/podman.rs", code).is_empty());
    // Die Weiterleitung in `humanitl-ipc` ist erlaubt, ein Einschalten dort
    // nicht, und dieselbe Zeile in einem anderen Manifest auch nicht.
    let forward = format!("[features]\n# unadmitted-backends\n{IPC_FORWARD}\n");
    assert!(offending("crates/ipc/Cargo.toml", &forward).is_empty());
    let default_on = "[features]\ndefault = [\"unadmitted-backends\"]\n";
    assert_eq!(
        offending("crates/ipc/Cargo.toml", default_on),
        ["crates/ipc/Cargo.toml:2"]
    );
    assert_eq!(
        offending("bin/humanitld/Cargo.toml", &forward),
        ["bin/humanitld/Cargo.toml:2", "bin/humanitld/Cargo.toml:3"]
    );
}
