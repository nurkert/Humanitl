//! Launcher inside the sandbox (HUM-012, ADR-002).
//!
//! On Linux the process model with bridges, seccomp filter and exec lives in
//! `linux.rs`, whose module documentation holds the launcher contract. On
//! macOS the shim runs under the Seatbelt profile of `sandbox-exec` and lives
//! in `macos.rs` (HUM-323). Both share the bridge (`bridge.rs`) and the report
//! (`report.rs`). Elsewhere the binary exists so that the workspace builds,
//! and refuses to run (HUM-312).
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

// Shared with Linux; the macOS half uses a part of them (no socket walk, no
// interface list), so the rest is dead code there.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod bridge;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod report;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn main() {
    eprintln!("humanitl-shim: this build has no launcher for this operating system");
    std::process::exit(125);
}
