//! Repository automation: protobuf descriptor refresh and reference docs.
//!
//! Run via `cargo xtask <task>` from the `daemon` directory.
#![deny(missing_docs)]

// Derselbe Codepfad wie `daemon/crates/ipc/build.rs`, damit der eingecheckte
// Descriptor und der Rust-Code aus derselben Uebersetzung stammen.
include!("../../crates/ipc/proto_gen.rs");

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

fn main() -> std::process::ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "" | "help" => {
            println!("usage: cargo xtask <proto|docs>");
            std::process::ExitCode::SUCCESS
        }
        "proto" => match task_proto() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("xtask proto failed: {error}");
                std::process::ExitCode::FAILURE
            }
        },
        other => {
            eprintln!("unknown task: {other}");
            std::process::ExitCode::FAILURE
        }
    }
}
