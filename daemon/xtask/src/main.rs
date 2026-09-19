//! Repository automation: protobuf descriptor refresh and reference docs.
//!
//! Run via `cargo xtask <task>` from the `daemon` directory.
#![deny(missing_docs)]

// Derselbe Codepfad wie `daemon/crates/ipc/build.rs`, damit der eingecheckte
// Descriptor und der Rust-Code aus derselben Uebersetzung stammen.
include!("../../crates/ipc/proto_gen.rs");

mod parity;

/// Wurzel des Repositories, abgeleitet aus dem Ort dieser Crate.
fn repo_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Erneuert den eingecheckten Descriptor-Set `proto/descriptor.binpb`.
///
/// Der Rust-Code selbst entsteht bei jedem `cargo build` in `build.rs` und
/// landet in `OUT_DIR`. Hier wird nur der Vertrag ohne Quellpositionen
/// geschrieben, den `tests/proto_contract.rs` und der Drift-Check in CI lesen.
/// Die Datei wird nur angefasst, wenn sich ihr Inhalt aendert.
fn task_proto() -> Result<(), Box<dyn Error>> {
    use protox::prost::Message as _;

    let root = repo_root();
    let proto_dir = root.join("proto");
    let descriptor = proto_dir.join("descriptor.binpb");

    let bytes = compile_protos(&proto_dir, false)?.encode_to_vec();
    if std::fs::read(&descriptor).is_ok_and(|old| old == bytes) {
        println!("descriptor: proto/descriptor.binpb (unchanged)");
    } else {
        std::fs::write(&descriptor, bytes)?;
        println!("descriptor: proto/descriptor.binpb (written)");
    }
    Ok(())
}

/// Erzeugt `docs/reference/parity.md` (HUM-078).
///
/// Befunde, die die Tabelle verhindern, stehen einzeln auf `stderr`, in der CI
/// als Annotation; die Datei bleibt dann unberührt. Warnungen (RPCs ohne Ort
/// in der Oberfläche) verhindern nichts. Wie `task_proto` schreibt der Lauf
/// nur, wenn sich der Inhalt ändert.
///
/// Mit `check` schreibt der Lauf nichts, sondern vergleicht die erzeugte
/// Tabelle mit der Datei auf der Platte und scheitert, wenn sie fehlt oder
/// abweicht. In der CI ist die Datei auf der Platte die eingecheckte; so kommt
/// `scripts/ci/parity-check.sh` ohne Versionsverwaltung aus und läuft auch
/// lokal in `make check`, wo der Arbeitsbaum Änderungen trägt, die noch
/// niemand eingecheckt hat.
fn task_docs(check: bool) -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let github = std::env::var_os("GITHUB_ACTIONS").is_some();
    let fds = compile_protos(&root.join("proto"), false)?;
    let table = match parity::generate(&root, fds) {
        Ok(table) => table,
        Err(error) => {
            for line in error.to_string().lines() {
                if github {
                    eprintln!("::error::parity: {line}");
                } else {
                    eprintln!("error: parity: {line}");
                }
            }
            return Err("the parity table cannot be generated".into());
        }
    };
    for warning in &table.warnings {
        if github {
            eprintln!("::warning::parity: {warning}");
        } else {
            eprintln!("warn: parity: {warning}");
        }
    }

    let output = root.join(parity::OUTPUT);
    if std::fs::read_to_string(&output).is_ok_and(|old| old == table.markdown) {
        println!("docs: {} (unchanged)", parity::OUTPUT);
    } else if check {
        let message = format!(
            "{} is missing or stale; run `cargo xtask docs` and commit the result",
            parity::OUTPUT
        );
        if github {
            eprintln!("::error::parity: {message}");
        }
        return Err(message.into());
    } else {
        if let Some(dir) = output.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&output, table.markdown)?;
        println!("docs: {} (written)", parity::OUTPUT);
    }
    Ok(())
}

/// `cargo xtask docs [--check]`: ein unbekannter Schalter ist ein Fehler, kein
/// Schreiblauf.
fn docs_with(option: Option<&str>) -> Result<(), Box<dyn Error>> {
    match option {
        None => task_docs(false),
        Some("--check") => task_docs(true),
        Some(other) => Err(format!("unknown option: {other}").into()),
    }
}

fn main() -> std::process::ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "" | "help" => {
            println!("usage: cargo xtask <proto|docs [--check]>");
            std::process::ExitCode::SUCCESS
        }
        "proto" => match task_proto() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("xtask proto failed: {error}");
                std::process::ExitCode::FAILURE
            }
        },
        "docs" => match docs_with(std::env::args().nth(2).as_deref()) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("xtask docs failed: {error}");
                std::process::ExitCode::FAILURE
            }
        },
        other => {
            eprintln!("unknown task: {other}");
            std::process::ExitCode::FAILURE
        }
    }
}
