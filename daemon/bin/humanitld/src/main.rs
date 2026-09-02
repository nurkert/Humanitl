//! Hintergrunddienst: Proxy, Sandbox-Verwaltung, Aufzeichnung, gRPC-Server. Nur Verdrahtung, keine Fachlogik.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

fn main() {
    println!("humanitld {}", env!("CARGO_PKG_VERSION"));
}
