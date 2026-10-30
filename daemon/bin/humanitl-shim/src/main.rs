//! Launcher inside the sandbox (HUM-012, ADR-002).
//!
//! The process model with bridges, seccomp filter and exec lives in
//! `linux.rs`, whose module documentation holds the launcher contract. It
//! needs seccomp and `prctl` and therefore builds only on Linux; on other
//! systems the binary exists so that the workspace builds, and refuses to run
//! (HUM-312).
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod linux;

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("humanitl-shim: this build has no launcher for this operating system");
    std::process::exit(125);
}
