// Protobuf-Uebersetzung, wortgleich eingebunden von drei Aufrufern:
// `daemon/crates/ipc/build.rs` (jeder `cargo build`, erzeugt Rust-Code nach
// `OUT_DIR`), `daemon/xtask/src/main.rs` (`cargo xtask proto`, erneuert den
// eingecheckten `proto/descriptor.binpb`) und
// `daemon/crates/ipc/tests/proto_contract.rs` (uebersetzt noch einmal und
// vergleicht mit dem eingecheckten Descriptor; deshalb `pub(crate)`).
//
// Der Weg kommt ohne `protoc` und ohne `buf` aus: `protox` uebersetzt die
// `.proto`-Dateien in einen `FileDescriptorSet`. Alle Aufrufer teilen sich
// Dateiliste und Aufruf, damit Rust-Code und Descriptor nie aus verschiedenen
// Quellen entstehen. Ein frischer Clone baut damit ohne Fremdwerkzeuge.
//
// Die Datei wird per `include!` eingezogen, nicht als Modul kompiliert; sie
// darf deshalb nur `protox` und die Standardbibliothek benutzen.
use std::error::Error;
use std::path::{Path, PathBuf};

use protox::prost_reflect::prost_types::FileDescriptorSet;

/// Die uebersetzten Dateien, relativ zum Verzeichnis `proto/`.
/// Reihenfolge egal, `protox` loest Importe selbst auf.
const PROTO_FILES: [&str; 3] = [
    "humanitl/v1/common.proto",
    "humanitl/v1/rules.proto",
    "humanitl/v1/humanitl.proto",
];

/// Vollstaendige Pfade der `.proto`-Dateien unter `proto_dir`.
pub(crate) fn proto_paths(proto_dir: &Path) -> Vec<PathBuf> {
    PROTO_FILES.iter().map(|f| proto_dir.join(f)).collect()
}

/// Uebersetzt den Vertrag samt Importen in einen `FileDescriptorSet`.
///
/// Mit `source_info` tragen die Descriptoren Kommentare und Positionen; das
/// braucht die Rust-Erzeugung fuer die Doc-Kommentare. Ohne `source_info`
/// haengt die Ausgabe nur vom Inhalt der `.proto`-Dateien ab, nicht von deren
/// Formatierung; so entsteht der eingecheckte Descriptor.
pub(crate) fn compile_protos(
    proto_dir: &Path,
    source_info: bool,
) -> Result<FileDescriptorSet, Box<dyn Error>> {
    Ok(protox::Compiler::new([proto_dir])?
        .include_imports(true)
        .include_source_info(source_info)
        .open_files(proto_paths(proto_dir))?
        .file_descriptor_set())
}
