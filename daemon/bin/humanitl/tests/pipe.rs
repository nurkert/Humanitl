//! A reader that closes the pipe early ends the CLI quietly (HUM-285).
//!
//! `humanitl doctor | head -5` used to panic with "failed printing to
//! stdout: Broken pipe". The test closes the read end before the child
//! writes, so its first write to `stdout` meets `EPIPE`.
#![allow(clippy::expect_used)]

use std::process::{Command, Stdio};

/// Runs `args` with a `stdout` pipe whose read end is already closed, in an
/// empty environment of its own: no daemon, no configuration of this machine.
fn run_with_closed_stdout(args: &[&str]) -> (Option<i32>, String) {
    let home = tempfile::tempdir().expect("a temporary home");
    let (reader, writer) = std::io::pipe().expect("a pipe");
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_humanitl"))
        .args(args)
        .env_clear()
        .env("PATH", "")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("XDG_DATA_HOME", home.path().join("data"))
        .env("XDG_RUNTIME_DIR", home.path().join("run"))
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .expect("humanitl runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_closed_pipe_ends_the_cli_without_a_panic() {
    // `doctor` prints its table with `println!`, so the first line meets the
    // closed pipe. 141 is the hook's code; without it the panic gives 101.
    let (code, stderr) = run_with_closed_stdout(&["doctor"]);
    assert!(
        !stderr.contains("panicked"),
        "doctor panicked on a closed pipe: {stderr}"
    );
    assert_eq!(code, Some(141), "doctor ends like SIGPIPE: {stderr}");
}
