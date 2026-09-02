//! Launcher inside the sandbox.
//!
//! Starts the bridges declared by the sandbox profile, applies the seccomp
//! filter and then `execvp`s the agent. Deliberately dependency-free (no tokio,
//! no workspace crates) so the security-critical step stays auditable in one
//! file. See `BACKLOG.md` section 4.1 and issue HUM-012.
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

fn main() {
    println!("humanitl-shim {}", env!("CARGO_PKG_VERSION"));
}
