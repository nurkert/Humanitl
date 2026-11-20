//! The check of a vendor's list of checksums by the daemon (HUM-566).
//!
//! Where a vendor signs nothing but publishes the SHA-256 of its release
//! archives in a list, and the SHA-256 of that list in its release metadata
//! ([`ChecksumList`], Codex), the maintenance run downloads metadata, list
//! and archive into `checked/` and unpacks the archive into the program tree.
//! The run's own comparison only keeps an archive that does not match away
//! from `tar`; it is no verdict. After the run the daemon reads its own copy
//! of the three files and checks the chain itself:
//!
//! 1. the metadata names the release of the version the run installed
//!    ([`ChecksumList::version_pointer`] equals the prefix and the version);
//! 2. the list has the SHA-256 the metadata names for it
//!    (`assets[].digest`, `sha256:<hex>`, exactly one asset of that name);
//! 3. the list names exactly one SHA-256 for the platform's archive, and the
//!    archive has it.
//!
//! `sha2` hashes; nothing here is cryptography of the daemon's own. What
//! binds the chain is TLS to the vendor's host, as for the vendor's own
//! installer: no key of the vendor's stands behind the metadata.

use sha2::{Digest as _, Sha256};

use super::adopt::hex;
use super::recipe::ChecksumList;

/// The directory of the run's result that holds the files of the check.
pub(crate) const DIR: &str = "checked";
/// The vendor's release metadata, JSON.
pub(crate) const METADATA: &str = "metadata";
/// The vendor's list of checksums.
pub(crate) const LIST: &str = "list";
/// The archive the program tree came from.
pub(crate) const ARCHIVE: &str = "archive";

/// Which part of the chain failed, for the step of `AGENT_011`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failed {
    /// The metadata names another release than the run installed.
    Version(String),
    /// Metadata, list or archive do not match.
    Checksum(String),
}

/// Checks the chain metadata, list, archive for `version` on the vendor's
/// `platform`; `archive_sha256` is the archive's SHA-256 the daemon computed,
/// lower-case hex.
///
/// # Errors
///
/// [`Failed`] with what does not match.
pub(crate) fn check(
    spec: &ChecksumList,
    platform: &str,
    version: &str,
    metadata: &[u8],
    list: &[u8],
    archive_sha256: &str,
) -> Result<(), Failed> {
    let value: serde_json::Value = serde_json::from_slice(metadata).map_err(|err| {
        Failed::Checksum(format!("the vendor's release metadata is no JSON ({err})"))
    })?;
    let release = value
        .pointer(spec.version_pointer)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            Failed::Version(format!(
                "the vendor's release metadata names no release at {}",
                spec.version_pointer
            ))
        })?;
    let wanted = format!("{}{version}", spec.version_prefix);
    if release != wanted {
        return Err(Failed::Version(format!(
            "the run named version {version}, and the vendor's release metadata names {release:?}"
        )));
    }
    let list_digest = asset_digest(&value, spec.list)?;
    let list_sum = hex(&Sha256::digest(list));
    if list_sum != list_digest {
        return Err(Failed::Checksum(format!(
            "the SHA-256 of {} is {list_sum}, and the vendor's release metadata names \
             {list_digest}",
            spec.list
        )));
    }
    let archive = spec.archive.replace("{platform}", platform);
    let expected = listed_sum(list, &archive)
        .map_err(|what| Failed::Checksum(format!("{}: {what}", spec.list)))?;
    if archive_sha256 != expected {
        return Err(Failed::Checksum(format!(
            "the SHA-256 of {archive} is {archive_sha256}, and the vendor's {} names {expected}",
            spec.list
        )));
    }
    Ok(())
}

/// The SHA-256 the metadata names for the asset `name`: the `digest` of the
/// one entry of `/assets` with that `name`, `sha256:` and 64 hex digits.
fn asset_digest(metadata: &serde_json::Value, name: &str) -> Result<String, Failed> {
    let assets = metadata
        .pointer("/assets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            Failed::Checksum("the vendor's release metadata lists no assets".to_owned())
        })?;
    let mut found = assets
        .iter()
        .filter(|asset| asset.get("name").and_then(serde_json::Value::as_str) == Some(name));
    let (Some(asset), None) = (found.next(), found.next()) else {
        return Err(Failed::Checksum(format!(
            "the vendor's release metadata does not name the asset {name} exactly once"
        )));
    };
    let digest = asset
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|sum| is_sha256(sum))
        .ok_or_else(|| {
            Failed::Checksum(format!(
                "the vendor's release metadata names no SHA-256 for {name}"
            ))
        })?;
    Ok(digest.to_ascii_lowercase())
}

/// The one SHA-256 the list names for `name`, from lines `<hex>  <name>`
/// (`sha256sum`, with `*` before a name in binary mode). Two lines for the
/// same name are refused, even when they agree: the list is the vendor's and
/// has one line per asset.
fn listed_sum(list: &[u8], name: &str) -> Result<String, String> {
    let text = std::str::from_utf8(list).map_err(|_| "the list is not UTF-8".to_owned())?;
    let mut sums = text.lines().filter_map(|line| {
        let (sum, rest) = line.split_once(char::is_whitespace)?;
        let listed = rest
            .trim_start()
            .strip_prefix('*')
            .unwrap_or(rest.trim_start());
        (listed == name).then_some(sum)
    });
    match (sums.next(), sums.next()) {
        (Some(sum), None) if is_sha256(sum) => Ok(sum.to_ascii_lowercase()),
        (Some(_), None) => Err(format!("the line of {name} holds no SHA-256")),
        (None, _) => Err(format!("it names no checksum for {name}")),
        (Some(_), Some(_)) => Err(format!("it names {name} more than once")),
    }
}

fn is_sha256(sum: &str) -> bool {
    sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const SPEC: ChecksumList = ChecksumList {
        version_pointer: "/tag_name",
        version_prefix: "rust-v",
        list: "pkg_SHA256SUMS",
        archive: "pkg-{platform}.tar.gz",
        evidence: "https://standin.test/docs",
    };

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    /// A release as the vendor publishes it: an archive, a list with its
    /// line among others, metadata with the list's digest.
    struct Release {
        archive: Vec<u8>,
        list: Vec<u8>,
        metadata: Vec<u8>,
    }

    fn release(archive: &[u8]) -> Release {
        let list = format!(
            "{}  pkg-other.tar.gz\n{}  pkg-x86_64.tar.gz\n",
            sha(b"other"),
            sha(archive)
        )
        .into_bytes();
        let metadata = serde_json::json!({
            "tag_name": "rust-v1.2.3",
            "assets": [
                {"name": "pkg-x86_64.tar.gz", "digest": format!("sha256:{}", sha(archive))},
                {"name": "pkg_SHA256SUMS", "digest": format!("sha256:{}", sha(&list))},
            ],
        })
        .to_string()
        .into_bytes();
        Release {
            archive: archive.to_vec(),
            list,
            metadata,
        }
    }

    fn checked(release: &Release, version: &str) -> Result<(), Failed> {
        check(
            &SPEC,
            "x86_64",
            version,
            &release.metadata,
            &release.list,
            &sha(&release.archive),
        )
    }

    #[test]
    fn a_release_whose_chain_holds_is_taken() {
        checked(&release(b"the archive"), "1.2.3").unwrap();
    }

    /// Mutation proof: without the comparison of the archive's SHA-256 with
    /// its line this turns red.
    #[test]
    fn an_archive_other_than_the_listed_one_is_refused() {
        let mut changed = release(b"the archive");
        changed.archive = b"another archive".to_vec();
        let Err(Failed::Checksum(why)) = checked(&changed, "1.2.3") else {
            panic!("taken");
        };
        assert!(why.contains("pkg-x86_64.tar.gz"), "{why}");
    }

    /// Mutation proof: without the comparison of the list's SHA-256 with the
    /// metadata this turns red: a list with a line for another archive would
    /// pass.
    #[test]
    fn a_list_other_than_the_one_the_metadata_names_is_refused() {
        let mut changed = release(b"the archive");
        changed.archive = b"planted".to_vec();
        changed.list = format!("{}  pkg-x86_64.tar.gz\n", sha(b"planted")).into_bytes();
        let Err(Failed::Checksum(why)) = checked(&changed, "1.2.3") else {
            panic!("taken");
        };
        assert!(why.contains("pkg_SHA256SUMS"), "{why}");
    }

    /// Mutation proof: without the comparison of the release's name the
    /// first assert turns red: an older authentic release under a newer
    /// version would pass.
    #[test]
    fn metadata_of_another_release_is_refused() {
        let found = checked(&release(b"the archive"), "1.2.4");
        assert!(matches!(found, Err(Failed::Version(_))), "{found:?}");
        let mut no_name = release(b"the archive");
        no_name.metadata = br#"{"assets":[]}"#.to_vec();
        assert!(matches!(
            checked(&no_name, "1.2.3"),
            Err(Failed::Version(_))
        ));
    }

    #[test]
    fn a_list_without_one_clear_line_or_a_digest_is_refused() {
        let base = release(b"the archive");
        // No line for the platform's archive.
        let mut missing = release(b"the archive");
        missing.list = format!("{}  pkg-other.tar.gz\n", sha(b"other")).into_bytes();
        missing.metadata = String::from_utf8(base.metadata.clone())
            .unwrap()
            .replace(&sha(&base.list), &sha(&missing.list))
            .into_bytes();
        assert!(checked(&missing, "1.2.3").is_err());
        // Two lines for it.
        let mut twice = release(b"the archive");
        twice.list = format!(
            "{0}  pkg-x86_64.tar.gz\n{0} *pkg-x86_64.tar.gz\n",
            sha(b"the archive")
        )
        .into_bytes();
        twice.metadata = String::from_utf8(base.metadata.clone())
            .unwrap()
            .replace(&sha(&base.list), &sha(&twice.list))
            .into_bytes();
        let Err(Failed::Checksum(why)) = checked(&twice, "1.2.3") else {
            panic!("taken");
        };
        assert!(why.contains("more than once"), "{why}");
        // A digest that is no SHA-256.
        let mut short = release(b"the archive");
        short.metadata = String::from_utf8(base.metadata.clone())
            .unwrap()
            .replace(&format!("sha256:{}", sha(&base.list)), "sha256:abc")
            .into_bytes();
        assert!(checked(&short, "1.2.3").is_err());
        // No JSON at all.
        let mut broken = release(b"the archive");
        broken.metadata = b"<html>".to_vec();
        assert!(checked(&broken, "1.2.3").is_err());
    }

    /// `sha256sum -b` writes `*` before the name; upper-case hex is the same
    /// sum.
    #[test]
    fn a_binary_mode_line_and_upper_case_hex_are_read() {
        let base = release(b"the archive");
        let mut binary = release(b"the archive");
        binary.list = format!(
            "{} *pkg-x86_64.tar.gz\n",
            sha(b"the archive").to_ascii_uppercase()
        )
        .into_bytes();
        binary.metadata = String::from_utf8(base.metadata)
            .unwrap()
            .replace(&sha(&base.list), &sha(&binary.list))
            .into_bytes();
        checked(&binary, "1.2.3").unwrap();
    }
}
