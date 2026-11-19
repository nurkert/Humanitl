//! The check of a vendor's signature by the daemon (ADR-0023 point 5,
//! HUM-519).
//!
//! The maintenance run only downloads: the vendor's manifest, its detached
//! signature and the vendor's public key. It reports no verdict, and the
//! daemon reads none from it: a sandbox could write any status line it
//! likes. The daemon copies the three files out of the run's result into a
//! directory of its own, and runs the host's `gpg` itself on that copy, in
//! a fresh `GNUPGHOME` that holds nothing but the imported key, with
//! `--no-autostart` (no agent) and `--disable-dirmngr` (no network). It reads
//! the status lines from its own child process (`--status-fd 1`):
//!
//! - the import must name exactly one key, and its fingerprint must be the
//!   one the adapter fixes (`check_import`); a key file with another key,
//!   or with a second one beside it, is refused;
//! - the verification must end with status 0, report `GOODSIG`, report no
//!   bad, expired or revoked signature, and its `VALIDSIG` must name the
//!   fixed fingerprint (`check_verify`).
//!
//! The SHA-256 of the program is then compared with the checksum in that
//! verified copy of the manifest. No cryptography of the daemon's own:
//! `gpg` checks the signature, `sha2` hashes.
//!
//! Where the adapter declares a signature, a host without `gpg` fails the
//! install closed (`AGENT_011` at the step `signature`); nothing unsigned is
//! ever taken over.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where the daemon looks for the host's `gpg`, in this order.
pub const GPG_PLACES: &[&str] = &[
    "/usr/bin/gpg",
    "/usr/local/bin/gpg",
    "/opt/homebrew/bin/gpg",
];

/// The host's `gpg`, the first executable regular file of [`GPG_PLACES`].
#[must_use]
pub fn host_gpg() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    GPG_PLACES.iter().map(PathBuf::from).find(|path| {
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// The files of a signed manifest, in a directory of the daemon.
pub(crate) const MANIFEST: &str = "manifest";
/// The detached signature.
pub(crate) const SIGNATURE: &str = "signature";
/// The vendor's public key.
pub(crate) const KEY: &str = "key.asc";

/// Verifies `dir/manifest` against `dir/signature` with the key in
/// `dir/key.asc`, which must be the key with `fingerprint`. `dir` is the
/// daemon's own and private; `gpg` gets a fresh home `dir/gnupg`.
///
/// # Errors
///
/// What failed, for the `why` of `AGENT_011`: no `gpg`, the import, the
/// verification.
pub(crate) fn verify(gpg: Option<&Path>, dir: &Path, fingerprint: &str) -> Result<(), String> {
    let Some(gpg) = gpg else {
        return Err(
            "gpg is not installed on the host, so the vendor's signature cannot be checked; an \
             unsigned program is never taken over (install gnupg on the host)"
                .to_owned(),
        );
    };
    let home = dir.join("gnupg");
    super::adopt::make_dir(&home)
        .map_err(|err| format!("the home of gpg cannot be created ({err})"))?;
    // An empty `common.conf` keeps a `GnuPG` from 2.4.1 on from writing one
    // with `use-keyboxd` into the fresh home: the key box daemon would have to
    // start, and `--no-autostart` forbids it.
    std::fs::write(home.join("common.conf"), b"")
        .map_err(|err| format!("the home of gpg cannot be prepared ({err})"))?;
    let (ok, status, said) = run(
        gpg,
        &home,
        &["--import".as_ref(), dir.join(KEY).as_os_str()],
    )?;
    if !ok {
        return Err(format!("gpg could not import the vendor's key ({said})"));
    }
    check_import(&status, fingerprint)?;
    let (ok, status, _) = run(
        gpg,
        &home,
        &[
            "--verify".as_ref(),
            dir.join(SIGNATURE).as_os_str(),
            dir.join(MANIFEST).as_os_str(),
        ],
    )?;
    check_verify(ok, &status, fingerprint)
}

/// Runs `gpg` with its status lines on standard output; answers whether it
/// ended with status 0, the status lines, and the start of what it said on
/// standard error, for a finding.
fn run(
    gpg: &Path,
    home: &Path,
    args: &[&std::ffi::OsStr],
) -> Result<(bool, String, String), String> {
    let output = Command::new(gpg)
        .env_clear()
        .env("GNUPGHOME", home)
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin:/bin")
        .args([
            "--batch",
            "--no-tty",
            "--no-autostart",
            "--disable-dirmngr",
            "--no-auto-key-retrieve",
            "--status-fd",
            "1",
        ])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("gpg cannot be run ({err})"))?;
    let said: String = String::from_utf8_lossy(&output.stderr)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect();
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        said,
    ))
}

/// The fields of the status lines (`[GNUPG:] KEYWORD ...`).
fn status_lines(status: &str) -> impl Iterator<Item = Vec<&str>> {
    status.lines().filter_map(|line| {
        line.strip_prefix("[GNUPG:] ")
            .map(|rest| rest.split_ascii_whitespace().collect())
    })
}

/// The import named exactly the fixed key and no other.
///
/// # Errors
///
/// What the import named instead.
pub(crate) fn check_import(status: &str, fingerprint: &str) -> Result<(), String> {
    let imported: Vec<&str> = status_lines(status)
        .filter(|fields| fields.first() == Some(&"IMPORT_OK"))
        .filter_map(|fields| fields.get(2).copied())
        .collect();
    match imported.as_slice() {
        [one] if one.eq_ignore_ascii_case(fingerprint) => Ok(()),
        [] => Err("the vendor's key file holds no key gpg could import".to_owned()),
        others => Err(format!(
            "the vendor's key file holds {}, and the adapter fixes only the key {fingerprint}",
            others.join(", ")
        )),
    }
}

/// The verification ended well, by the fixed key.
///
/// The fingerprint counts as the signing key's (second field of
/// `VALIDSIG`) or as the primary key's (its last field, for a signature by a
/// subkey); see `doc/DETAILS` of `GnuPG`.
///
/// # Errors
///
/// What failed.
pub(crate) fn check_verify(ok: bool, status: &str, fingerprint: &str) -> Result<(), String> {
    let mut good = false;
    let mut valid = false;
    for fields in status_lines(status) {
        match fields.first().copied() {
            Some(
                word @ ("BADSIG" | "ERRSIG" | "EXPSIG" | "EXPKEYSIG" | "REVKEYSIG" | "NO_PUBKEY"
                | "FAILURE"),
            ) => {
                return Err(format!("gpg reported {word} for the vendor's manifest"));
            }
            Some("GOODSIG") => good = true,
            Some("VALIDSIG") => {
                let signing = fields.get(1).copied().unwrap_or_default();
                let primary = if fields.len() >= 11 {
                    fields.last().copied().unwrap_or_default()
                } else {
                    ""
                };
                if signing.eq_ignore_ascii_case(fingerprint)
                    || primary.eq_ignore_ascii_case(fingerprint)
                {
                    valid = true;
                }
            }
            _ => {}
        }
    }
    if !ok {
        return Err("gpg --verify ended with an error".to_owned());
    }
    if !good || !valid {
        return Err(format!(
            "gpg reported no good signature by the key with the fingerprint {fingerprint}, \
             which the adapter fixes"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const FPR: &str = "31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE";

    fn good(fpr: &str) -> String {
        format!(
            "[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG BAA929FF1A7ECACE Anthropic\n\
             [GNUPG:] VALIDSIG {fpr} 2026-11-01 1761955200 0 4 0 1 10 00 {fpr}\n"
        )
    }

    #[test]
    fn a_good_signature_by_the_fixed_key_passes() {
        check_verify(true, &good(FPR), FPR).unwrap();
    }

    #[test]
    fn a_signature_by_a_subkey_counts_through_the_primary_fingerprint() {
        let status = format!(
            "[GNUPG:] GOODSIG 1111 x\n[GNUPG:] VALIDSIG {} 2026-11-01 0 0 4 0 1 10 00 {FPR}\n",
            "A".repeat(40)
        );
        check_verify(true, &status, FPR).unwrap();
    }

    #[test]
    fn an_error_another_key_or_a_bad_line_fails() {
        assert!(check_verify(false, &good(FPR), FPR).is_err());
        assert!(check_verify(true, &good(&"0".repeat(40)), FPR).is_err());
        let bad = format!("{}[GNUPG:] BADSIG BAA929FF1A7ECACE x\n", good(FPR));
        assert!(
            check_verify(true, &bad, FPR)
                .unwrap_err()
                .contains("BADSIG")
        );
        let text = format!("[GNUPG:] GOODSIG x {FPR}\n[GNUPG:] VALIDSIG 00 {FPR}x\n");
        assert!(check_verify(true, &text, FPR).is_err());
        // Status text on another channel than a status line counts nothing.
        let quoted = format!("gpg: [GNUPG:] VALIDSIG {FPR}\n[GNUPG:] GOODSIG 1 x\n");
        assert!(check_verify(true, &quoted, FPR).is_err());
    }

    #[test]
    fn the_import_names_exactly_the_fixed_key() {
        check_import(&format!("[GNUPG:] IMPORT_OK 1 {FPR}\n"), FPR).unwrap();
        assert!(check_import("", FPR).is_err());
        let other = "0".repeat(40);
        assert!(check_import(&format!("[GNUPG:] IMPORT_OK 1 {other}\n"), FPR).is_err());
        let two = format!("[GNUPG:] IMPORT_OK 1 {FPR}\n[GNUPG:] IMPORT_OK 1 {other}\n");
        assert!(check_import(&two, FPR).is_err());
    }

    /// Mutation proof (HUM-519): a missing `gpg` judged as "not checked"
    /// turns this red.
    #[test]
    fn without_gpg_nothing_is_verified() {
        let dir = tempfile::tempdir().unwrap();
        let why = verify(None, dir.path(), FPR).unwrap_err();
        assert!(why.contains("never taken over"), "{why}");
    }
}
