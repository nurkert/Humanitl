//! The agent's environment through an inherited descriptor (HUM-430).
//!
//! Under bwrap the launcher puts the agent's environment on the command line
//! (`--setenv`), and nothing of it lands on a disk. A container engine may write every
//! value into the stored configuration of the container (HUM-292, measurement
//! P11), so a backend like that hands the environment over as a sealed memfd
//! instead: one flat JSON object, `String` to `String`, named with
//! `--env-fd <n>`. The shim reads it before anything else of the start, makes
//! it the environment of the agent, and closes it. No value of it stands in an
//! argument vector, in the process list or in a file of the container engine.
//!
//! The same rules hold as for `--setenv`:
//!
//! - the five variables of the launcher contract (`SHIM_VARS` in `linux.rs`,
//!   `humanitl_sandbox::RESERVED_ENV`) are refused: they reach the shim
//!   through its own environment and never from the agent's side;
//! - the three loader variables (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`,
//!   `humanitl_config::LOADER_ENV_KEYS`) are refused: they would load code
//!   before the agent's filter is in place.
//!
//! A descriptor that is not sealed against writing, shrinking and growing
//! is refused as well: only the launcher may have written it, and a
//! descriptor someone can still change
//! is not the one it handed over. Every refusal ends the start with exit 126
//! and a failed check line (`linux.rs`).

use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::FileExt;

use crate::json;

/// The most bytes the descriptor may hold. An environment is a few
/// kilobytes; a megabyte is a launcher bug, not an environment.
pub const MAX_BYTES: usize = 1 << 20;

/// The variables the dynamic linker reads before `main`; the same three as
/// `humanitl_config::LOADER_ENV_KEYS`, written out because the shim knows no
/// workspace crate.
pub const LOADER_KEYS: [&str; 3] = ["LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH"];

/// Why the environment behind `--env-fd` was refused.
#[derive(Debug)]
pub enum Error {
    /// The number names no open descriptor.
    NotOpen(RawFd),
    /// The descriptor is not sealed against writing, shrinking and growing
    /// (or no memfd at all).
    NotSealed,
    /// More than [`MAX_BYTES`].
    TooLarge,
    /// `pread(2)` failed.
    Read(io::Error),
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The text is not a flat JSON object of strings.
    Json(json::Error),
    /// A key that cannot be a variable name: empty, with `=` or with NUL.
    BadKey(String),
    /// A value with a NUL byte, which no environment can carry.
    BadValue(String),
    /// A key given twice.
    Duplicate(String),
    /// A key of the launcher contract.
    Reserved(String),
    /// A key the dynamic linker reads.
    Loader(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOpen(fd) => write!(f, "--env-fd {fd} is not an open descriptor"),
            Self::NotSealed => write!(
                f,
                "--env-fd is not a memfd sealed against writing, shrinking and growing"
            ),
            Self::TooLarge => write!(f, "--env-fd holds more than {MAX_BYTES} bytes"),
            Self::Read(err) => write!(f, "--env-fd cannot be read: {err}"),
            Self::NotUtf8 => write!(f, "--env-fd is not UTF-8"),
            Self::Json(err) => write!(f, "--env-fd: {err}"),
            Self::BadKey(key) => write!(f, "--env-fd: {key:?} is not a variable name"),
            Self::BadValue(key) => write!(f, "--env-fd: the value of {key} holds a NUL byte"),
            Self::Duplicate(key) => write!(f, "--env-fd: {key} is given twice"),
            Self::Reserved(key) => {
                write!(f, "--env-fd: {key} belongs to the launcher contract")
            }
            Self::Loader(key) => write!(
                f,
                "--env-fd: {key} would load code before the agent's filter"
            ),
        }
    }
}

/// Takes the descriptor, reads it from offset 0, closes it, and checks the
/// pairs against `reserved` and [`LOADER_KEYS`].
///
/// The descriptor is closed on every path, also when it is refused.
///
/// # Errors
///
/// Every variant of [`Error`].
pub fn read(fd: RawFd, reserved: &[&str]) -> Result<Vec<(String, String)>, Error> {
    let bytes = read_sealed(fd)?;
    let text = String::from_utf8(bytes).map_err(|_| Error::NotUtf8)?;
    parse(&text, reserved)
}

/// Takes the descriptor, reads the sealed memfd behind it from offset 0 and
/// closes it, on every path. Shared with `--pins-fd` (HUM-437), which hands
/// over its own JSON the same way.
///
/// # Errors
///
/// [`Error::NotOpen`], [`Error::NotSealed`], [`Error::TooLarge`],
/// [`Error::Read`].
pub fn read_sealed(fd: RawFd) -> Result<Vec<u8>, Error> {
    // SAFETY: F_GETFD only asks whether the number is an open descriptor.
    if fd < 3 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(Error::NotOpen(fd));
    }
    // SAFETY: the launcher handed this descriptor to the shim alone, it is
    // open (above), and nothing else in this process owns it.
    let file = File::from(unsafe { OwnedFd::from_raw_fd(fd) });
    sealed_content(&file)
}

/// The content of a memfd sealed against writing, shrinking and growing, at most [`MAX_BYTES`].
fn sealed_content(file: &File) -> Result<Vec<u8>, Error> {
    use std::os::fd::AsRawFd as _;
    // SAFETY: F_GET_SEALS reads the seals of one descriptor; a descriptor
    // that is not a memfd answers EINVAL.
    let seals = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) };
    // Writing alone is not enough: a memfd sealed only against writing can
    // still be emptied with `ftruncate(2)` or grown, so all three.
    let needed = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW;
    if seals < 0 || seals & needed != needed {
        return Err(Error::NotSealed);
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let offset = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let n = match file.read_at(&mut chunk, offset) {
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(Error::Read(err)),
        };
        if n == 0 {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
    }
}

/// The pairs of `text`, checked: names a variable can have, no NUL byte, no
/// key twice, none of `reserved`, none of [`LOADER_KEYS`].
///
/// # Errors
///
/// [`Error::Json`], [`Error::BadKey`], [`Error::BadValue`],
/// [`Error::Duplicate`], [`Error::Reserved`], [`Error::Loader`].
pub fn parse(text: &str, reserved: &[&str]) -> Result<Vec<(String, String)>, Error> {
    let pairs = json::string_map(text).map_err(Error::Json)?;
    for (index, (key, value)) in pairs.iter().enumerate() {
        if key.is_empty() || key.contains(['=', '\0']) {
            return Err(Error::BadKey(key.clone()));
        }
        if value.contains('\0') {
            return Err(Error::BadValue(key.clone()));
        }
        if pairs[..index].iter().any(|(earlier, _)| earlier == key) {
            return Err(Error::Duplicate(key.clone()));
        }
        if reserved.contains(&key.as_str()) {
            return Err(Error::Reserved(key.clone()));
        }
        if LOADER_KEYS.contains(&key.as_str()) {
            return Err(Error::Loader(key.clone()));
        }
    }
    Ok(pairs)
}

/// Makes `pairs` the environment of this process: every variable but those
/// in `keep` goes, then the pairs are set.
///
/// The environment is emptied with `clearenv(3)` and the kept variables are
/// set again, instead of removing each inherited key: `remove_var` panics on a
/// key that is empty or holds `=`, and an inherited environment may have one.
///
/// What the container engine or bwrap put into the environment on its own
/// does not reach the agent; the plan's environment is the whole of it.
/// `keep` are the shim's own variables, which the child removes before
/// `exec` anyway.
///
/// # Safety
///
/// The process must be single-threaded: nothing may read or write the
/// environment concurrently.
pub unsafe fn apply(pairs: &[(String, String)], keep: &[&str]) {
    let kept: Vec<(&str, std::ffi::OsString)> = keep
        .iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (*key, value)))
        .collect();
    // SAFETY: the caller guarantees a single thread; nothing holds a pointer
    // into the environment across this call.
    unsafe {
        libc::clearenv();
    }
    for (key, value) in kept {
        // SAFETY: the caller guarantees a single thread; `keep` holds the
        // shim's own names, none empty or with `=`, and a value read from the
        // environment holds no NUL.
        unsafe { std::env::set_var(key, value) };
    }
    for (key, value) in pairs {
        // SAFETY: the caller guarantees a single thread; `parse` refused a
        // key with `=` or NUL and a value with NUL, so this cannot panic.
        unsafe { std::env::set_var(key, value) };
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const RESERVED: [&str; 2] = ["HUMANITL_BRIDGES", "HUMANITL_REPORT_FD"];

    #[test]
    fn a_flat_object_becomes_the_pairs_in_order() {
        let pairs = parse(
            r#"{"PATH":"/usr/bin","HOME":"/home/agent","EMPTY":""}"#,
            &RESERVED,
        )
        .unwrap();
        assert_eq!(
            pairs,
            [
                ("PATH".to_owned(), "/usr/bin".to_owned()),
                ("HOME".to_owned(), "/home/agent".to_owned()),
                ("EMPTY".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn broken_json_is_refused() {
        for text in ["", "{", r#"{"A":"x""#, r#"["A"]"#, r#"{"A":1}"#] {
            assert!(
                matches!(parse(text, &RESERVED), Err(Error::Json(_))),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_reserved_key_is_refused() {
        let err = parse(r#"{"A":"x","HUMANITL_BRIDGES":"[]"}"#, &RESERVED).unwrap_err();
        assert!(
            matches!(&err, Error::Reserved(key) if key == "HUMANITL_BRIDGES"),
            "{err}"
        );
        assert!(err.to_string().contains("launcher contract"), "{err}");
    }

    #[test]
    fn loader_keys_are_refused_and_their_relatives_are_not() {
        for key in LOADER_KEYS {
            let err = parse(&format!(r#"{{"{key}":"/tmp/x.so"}}"#), &RESERVED).unwrap_err();
            assert!(
                matches!(&err, Error::Loader(found) if found == key),
                "{err}"
            );
        }
        // The same three as `--setenv` refuses, not the whole prefix.
        assert!(parse(r#"{"LD_DEBUG":"libs","ld_preload":"x"}"#, &RESERVED).is_ok());
    }

    #[test]
    fn names_values_and_duplicates_a_process_cannot_carry_are_refused() {
        for (text, what) in [
            (r#"{"":"x"}"#, "name"),
            (r#"{"A=B":"x"}"#, "name"),
            (r#"{"A\u0000":"x"}"#, "name"),
            (r#"{"A":"x\u0000y"}"#, "NUL"),
            (r#"{"A":"x","A":"y"}"#, "twice"),
        ] {
            let err = parse(text, &RESERVED).unwrap_err().to_string();
            assert!(err.contains(what), "{text}: {err}");
        }
    }
}
