//! Repository automation: protobuf code generation and reference docs.
//!
//! Run via `cargo xtask <task>` from the `daemon` directory.
#![deny(missing_docs)]

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "" | "help" => println!("usage: cargo xtask <proto|docs>"),
        other => {
            eprintln!("unknown task: {other}");
            std::process::exit(1);
        }
    }
}
