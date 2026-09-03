//! Erzeugt bei jedem `cargo build` den Rust-Code aus `proto/humanitl/v1/`.
//!
//! Die Ausgabe landet in `OUT_DIR`, wie Cargo es fuer Build-Skripte vorsieht;
//! `src/lib.rs` zieht sie mit `include!(concat!(env!("OUT_DIR"), ...))` ein.
//! In den Quellbaum wird nichts geschrieben, zwei gleichzeitige Builds
//! (rust-analyzer neben `cargo test`) kommen sich deshalb nicht in die Quere.
//!
//! Die Uebersetzung der `.proto`-Dateien steht in `proto_gen.rs` und ist mit
//! `cargo xtask proto` geteilt, damit Rust-Code und `proto/descriptor.binpb`
//! aus demselben Aufruf stammen.

include!("proto_gen.rs");

/// Verzeichnis `proto/` relativ zu dieser Crate.
const PROTO_DIR_FROM_CRATE: &str = "../../../proto";

fn main() -> Result<(), Box<dyn Error>> {
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let proto_dir = crate_dir.join(PROTO_DIR_FROM_CRATE);

    println!("cargo:rerun-if-changed={}", proto_dir.display());
    for file in proto_paths(&proto_dir) {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    println!("cargo:rerun-if-changed=proto_gen.rs");

    let fds = compile_protos(&proto_dir, true)?;
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(&out_dir)
        .compile_fds(fds)?;
    Ok(())
}
