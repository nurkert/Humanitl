//! The guard of the credential write-back (ADR-0023 points 4 and 7, HUM-520).
//!
//! Tokens expire, and some vendors swap the refresh token on every use, so
//! exactly one file goes from a session back into the depot: the credential
//! file the adapter names ([`super::DepotRecipe::credential_file`]). This is
//! the depot's new attack surface: a compromised session that writes into the
//! depot acts in every later one. The guard lets only that file through, and
//! only checked:
//!
//! - **One file.** It is opened relative to the session's copy of the
//!   configuration with `openat` and `O_NOFOLLOW`, never through a link; it
//!   must be a regular file of at most [`CREDENTIAL_CAP`] bytes. Nothing else
//!   of the copy is ever read here.
//! - **The adapter's format** ([`CredentialFormat`]): only known keys, every
//!   value of the expected type and form, every required key present. A
//!   login checks against the format alone; every later write-back also
//!   against the stored file: no key more or fewer, the expiry not older,
//!   the account the same (`AGENT_016` otherwise). A format without an
//!   expiry takes a file back only from a session that started on the file
//!   now stored, so an older login never replaces a newer one.
//! - **What is written is the daemon's own serialisation** of the checked
//!   value, not the session's bytes: whitespace, key order and duplicate keys
//!   carry nothing into the depot.
//! - **Atomic, under a lock per agent:** a temporary file `0600` beside the
//!   depot's directories, `fsync`, `rename` into `credentials/`, `fsync` of
//!   both directories. A crash leaves the old file or the new one.
//! - **Every outcome is reported** with agent, session and the SHA-256 of the
//!   old and the new file, never their content; a refusal leaves the stored
//!   file as it was and says `AGENT_012` (or `AGENT_016`).
//! - **Sealed at rest** when the depot is (HUM-523, [`super::seal`]): the
//!   stored file is opened for the comparison and the new one sealed before
//!   it is written, under the same lock that a switch of the encryption
//!   takes. Every SHA-256 here is one of the plain file, so a session's base
//!   and the stored file compare alike in both forms.
//!
//! A format with an account field remembers the account through the stored
//! file itself; [`Depot::last_writeback`] names the session that wrote last.
//! An adapter without a [`CredentialFormat`] gets nothing back: the guard
//! never guesses a format.
//!
//! The login run may also take over single named fields of another file
//! ([`NamedFields`]), where an agent keeps account state beside configuration
//! (Claude Code's `.claude.json` holds `mcpServers` too): never the file,
//! only the named keys, read and checked by the daemon and written into the
//! depot's copy of that file ([`take_named_fields`]).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_core::diagnostics::codes::{AGENT_012, AGENT_016};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{FileType, Mode, OFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::adopt;
use super::recipe::{credential_name, credential_path_ok};
use super::seal::{self, CredentialKeys};
use super::store::Depot;

/// The largest credential file the guard reads, 64 KiB; also the largest
/// subtree of named fields taken over.
pub const CREDENTIAL_CAP: u64 = 64 * 1024;

/// The largest file of named fields the login run reads (`.claude.json` also
/// holds the agent's own state); only the named subtrees count against
/// [`CREDENTIAL_CAP`].
pub const NAMED_FILE_CAP: u64 = 4 * 1024 * 1024;

/// How far in the future an expiry may lie: a file that claims to hold for
/// longer is refused, so a session cannot pin its token above every renewal.
pub const EXPIRY_HORIZON: Duration = Duration::from_secs(400 * 24 * 60 * 60);

/// The longest key or pointer of a session's file a finding quotes.
const SHOWN_CAP: usize = 64;

/// How often the daemon looks at the credential file of a running session.
pub const WRITEBACK_POLL: Duration = Duration::from_secs(10);

/// The file in the depot's root that names the last write-back.
pub const WRITEBACK_STATE: &str = "writeback.json";

/// The longest token the format admits, in characters.
const TOKEN_CAP: usize = 8192;
/// The longest text, in characters.
const TEXT_CAP: usize = 256;
/// The most entries of a text list.
const LIST_CAP: usize = 64;
/// The largest whole number: `2^53`, exact in every JSON reader.
const COUNT_CAP: u64 = 1 << 53;

/// The form one value of a credential file must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueForm {
    /// A JSON object; its keys are the fields declared below its pointer.
    Object,
    /// A secret: a string of 1 to 8192 visible ASCII characters, no space.
    Token,
    /// A short text: at most 256 characters, no control character.
    Text,
    /// A whole number from 0 to `2^53`.
    Count,
    /// `true` or `false`.
    Flag,
    /// An array of at most 64 texts.
    TextList,
    /// A [`ValueForm::Token`] or `null`: a key the vendor always writes,
    /// empty as `null` (Codex's `OPENAI_API_KEY` in `auth.json`, HUM-515).
    TokenOrNull,
    /// A [`ValueForm::Text`] or `null` (Codex's `tokens.account_id`).
    TextOrNull,
    /// One of these strings exactly, as an enumeration of the vendor's
    /// (Codex's `auth_mode`, HUM-515).
    OneOf(&'static [&'static str]),
    /// An object of at most 64 entries, each a non-empty
    /// [`ValueForm::Text`] key with a [`ValueForm::Text`] value (`OpenCode`'s
    /// `metadata` of an API key, #110).
    TextMap,
}

/// A credential file that is a map from a name the vendor does not fix to
/// an entry of one of several kinds (#110: `OpenCode`'s `auth.json`, provider
/// identifier to `oauth`, `api` or `wellknown`).
///
/// Every top-level key must be an entry name ([`ENTRY_NAME_CAP`] visible
/// ASCII characters at most, no space) whose value is an object; its key
/// [`EntryMap::tag`] picks the kind, and the entry is checked against that
/// kind's fields like a whole file against [`CredentialFormat::fields`]. An
/// unknown kind, a value that is no object and a key the kind does not
/// declare are refused. A later write-back must hold exactly the stored
/// entries, each of the same kind with the same keys: a session renews the
/// tokens of the providers a login stored, it never adds or drops one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryMap {
    /// The key of an entry that names its kind: `type`.
    pub tag: &'static str,
    /// The kinds, each with its fields; pointers are relative to the entry
    /// (`/type`, `/key`).
    pub kinds: &'static [(&'static str, &'static [CredentialField])],
    /// The most entries a file holds.
    pub max_entries: usize,
}

/// The longest entry name of an [`EntryMap`], in characters.
pub const ENTRY_NAME_CAP: usize = 256;

/// Material a file must carry when one of its keys holds a given value
/// ([`CredentialFormat::requires`]): Codex's `auth_mode = "chatgpt"` needs
/// `tokens`, `"apikey"` needs `OPENAI_API_KEY` (HUM-515).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requires {
    /// The pointer of the deciding key.
    pub pointer: &'static str,
    /// The value that brings the requirement.
    pub equals: &'static str,
    /// The pointers that must then hold a value other than `null`.
    pub present: &'static [&'static str],
}

/// One key of a credential file, by its JSON pointer (RFC 6901).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredentialField {
    /// The pointer, as `/claudeAiOauth/accessToken`.
    pub pointer: &'static str,
    /// The form of its value.
    pub form: ValueForm,
    /// Whether it must be present whenever the object holding it is.
    pub required: bool,
}

/// The unit of an expiry in a credential file; each adapter shows its own
/// with an example from the vendor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    /// Seconds since 1970.
    Seconds,
    /// Milliseconds since 1970.
    Milliseconds,
}

/// Where a credential file carries its expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expiry {
    /// The pointer of a [`ValueForm::Count`] field.
    pub pointer: &'static str,
    /// Its unit.
    pub unit: TimeUnit,
}

/// The format of an adapter's credential file: every key it may hold. A key
/// that is not listed is refused, so is a value of another form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredentialFormat {
    /// Every key, objects included; the root is an object.
    pub fields: &'static [CredentialField],
    /// The expiry, where the format carries one: a later write-back must not
    /// be older than the stored file.
    pub expiry: Option<Expiry>,
    /// The pointer of a [`ValueForm::Text`] or [`ValueForm::Token`] field
    /// (or one of their forms with `null`) that names the account, where the
    /// format names one. Wherever the object holding it is present, the
    /// account must be a non-empty string, never `null` or missing; a later
    /// write-back must name the same account, and a file without one never
    /// counts as the same (HUM-515, review: `null` equal to `null` would let
    /// a session swap in another account's tokens).
    pub account: Option<&'static str>,
    /// Pointers of which at least one must hold a value other than `null`
    /// (Codex: `OPENAI_API_KEY` or `tokens`): a file that holds none of them
    /// is no login. Empty for no such rule.
    pub one_of: &'static [&'static str],
    /// What a file must carry for the value of one of its keys
    /// ([`Requires`]). Empty for no such rule.
    pub requires: &'static [Requires],
    /// Pointers whose value a later write-back must keep as the stored file
    /// has it, present or absent (Codex's `auth_mode`: its refresh keeps the
    /// mode). Empty for none.
    pub fixed: &'static [&'static str],
    /// The file is a map of entries ([`EntryMap`]) instead of fixed keys;
    /// [`CredentialFormat::fields`] is then empty (#110).
    pub entries: Option<EntryMap>,
    /// Where the vendor documents the format, or the measurement it comes
    /// from.
    pub evidence: &'static str,
}

/// Single named keys of another file of the agent that the login run takes
/// over (point 3 of HUM-520): never the file itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedFields {
    /// The file, relative to the configuration directory: `.claude.json`.
    pub file: &'static str,
    /// The keys taken over, top-level pointers with their subtrees declared
    /// below them; every other key of the file is left as the depot has it.
    pub fields: &'static [CredentialField],
    /// Where the vendor documents them, or the measurement they come from.
    pub evidence: &'static str,
}

/// Whether a write-back comes from the login run or from an ordinary
/// session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The login run: the person logged in on purpose, perhaps with another
    /// account; the file is checked against the format alone.
    Login,
    /// A session: the format, and against the stored file.
    Session,
}

/// What [`take_back`] needs.
#[derive(Debug, Clone, Copy)]
pub struct TakeBack<'a> {
    /// The depot.
    pub depot: &'a Depot,
    /// The credential file's name, relative to the configuration directory.
    pub file: &'a str,
    /// The adapter's format.
    pub format: &'a CredentialFormat,
    /// The session the file comes from.
    pub session: SessionId,
    /// The SHA-256 of the copy the session got at its start; `None` when it
    /// got none.
    pub base: Option<&'a str>,
    /// Login run or session.
    pub origin: Origin,
    /// The key of a sealed depot (HUM-523); `None` where the caller has no
    /// keyring access, and then a sealed depot refuses (`AGENT_020`).
    pub keys: Option<&'a CredentialKeys>,
}

/// What a write-back did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken {
    /// Nothing to take: no file, or the one the session got, or the one
    /// stored already.
    Nothing,
    /// The file is stored now.
    Stored {
        /// SHA-256 of the file stored before; `None` at the first login.
        old: Option<String>,
        /// SHA-256 of the file stored now.
        new: String,
        /// The account the file names, where the format names one.
        account: Option<String>,
        /// A directory that could not be synced after the `rename`: the file
        /// is in place, but a crash right now could lose it.
        warning: Option<String>,
    },
}

/// A refused write-back: the stored file is as it was.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    /// `AGENT_012`, or `AGENT_016` for an older token or another account.
    pub found: Diagnostic,
    /// SHA-256 of the stored file, if there is one.
    pub old: Option<String>,
    /// SHA-256 of the file the session left, if it could be read.
    pub new: Option<String>,
}

/// The last write-back, as [`WRITEBACK_STATE`] holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WritebackState {
    /// The session that wrote it.
    pub session: String,
    /// When, RFC 3339 in UTC.
    pub at: String,
    /// The SHA-256 of the file written.
    pub sha256: String,
    /// The account it names, where the format names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

/// The SHA-256 of `bytes`, in hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    humanitl_core::http::sha256(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// The SHA-256 of the credential file in a session's copy, read like
/// [`take_back`] reads it; `None` without a regular file there. A session's
/// base, right after the copy was made.
#[must_use]
pub fn fingerprint(config_copy: &Path, file: &str) -> Option<String> {
    let dir = adopt::open_dir(config_copy).ok()?;
    match read_credential(dir.as_fd(), file) {
        Ok(Some(bytes)) => Some(sha256_hex(&bytes)),
        _ => None,
    }
}

/// Takes the credential file of a session's configuration copy back into the
/// depot, if it passes the guard.
///
/// # Errors
///
/// [`Refused`] with `AGENT_012` for a file that is no regular file, a link, a
/// FIFO, larger than [`CREDENTIAL_CAP`], no JSON, or not in the adapter's
/// format, or a depot that cannot take it; `AGENT_016` for an older token, a
/// file the session did not start from (formats without an expiry), or
/// another account. The stored file is then unchanged.
pub fn take_back(config_copy: &Path, request: &TakeBack<'_>) -> Result<Taken, Refused> {
    take_back_at(config_copy, request, &now(), None)
}

/// [`take_back`] at a given time; `interrupt` stops writing the new file
/// after that many bytes, as a crash would, for the tests.
pub(crate) fn take_back_at(
    config_copy: &Path,
    request: &TakeBack<'_>,
    at: &str,
    interrupt: Option<usize>,
) -> Result<Taken, Refused> {
    take_back_with(config_copy, request, at, interrupt, None)
}

/// [`take_back`] under a lock the caller holds already
/// ([`CredentialLock`]), so that a later step can still undo it
/// ([`restore`]).
///
/// # Errors
///
/// As [`take_back`].
pub fn take_back_held(
    config_copy: &Path,
    request: &TakeBack<'_>,
    held: &CredentialLock,
) -> Result<Taken, Refused> {
    take_back_with(config_copy, request, &now(), None, Some(held))
}

fn take_back_with(
    config_copy: &Path,
    request: &TakeBack<'_>,
    at: &str,
    interrupt: Option<usize>,
    held: Option<&CredentialLock>,
) -> Result<Taken, Refused> {
    let agent = request.depot.agent();
    let refuse = |what: String, new: Option<String>| Refused {
        found: refused(agent, &what),
        old: None,
        new,
    };
    let dir = adopt::open_dir(config_copy).map_err(|err| {
        refuse(
            format!("the session's configuration copy cannot be opened ({err})"),
            None,
        )
    })?;
    let raw = match read_credential(dir.as_fd(), request.file) {
        Ok(Some(raw)) => raw,
        // The session holds no credential file (a logout): the depot keeps
        // its own.
        Ok(None) => return Ok(Taken::Nothing),
        Err(what) => return Err(refuse(format!("{} {what}", request.file), None)),
    };
    let raw_sum = sha256_hex(&raw);
    if request.base == Some(raw_sum.as_str()) {
        return Ok(Taken::Nothing);
    }
    let value = parse(&raw).map_err(|what| refuse(what, Some(raw_sum.clone())))?;
    check_document(&value, request.format).map_err(|what| refuse(what, Some(raw_sum.clone())))?;
    check_horizon(&value, request.format, std::time::SystemTime::now())
        .map_err(|what| refuse(what, Some(raw_sum.clone())))?;
    let bytes = serialise(&value).map_err(|what| refuse(what, Some(raw_sum.clone())))?;
    let new = sha256_hex(&bytes);
    let account = request
        .format
        .account
        .and_then(|pointer| value.pointer(pointer))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    let depot = request.depot;
    let _lock = match held {
        Some(_) => None,
        None => Some(lock(depot).map_err(|found| Refused {
            found,
            old: None,
            new: Some(new.clone()),
        })?),
    };
    depot.check().map_err(|found| Refused {
        found: refused(agent, &found.why),
        old: None,
        new: Some(new.clone()),
    })?;
    // The depot keeps the file under its name; the session may keep it one
    // directory deeper (OpenCode's `opencode/auth.json`, #110).
    let name = credential_name(request.file);
    let stored = read_stored(depot, name, request.keys).map_err(|found| Refused {
        found,
        old: None,
        new: Some(new.clone()),
    })?;
    let old = stored.as_ref().map(|stored| sha256_hex(&stored.plain));
    let refuse_against = |found: Diagnostic| Refused {
        found,
        old: old.clone(),
        new: Some(new.clone()),
    };
    if old.as_deref() == Some(new.as_str()) {
        return Ok(Taken::Nothing);
    }
    match (request.origin, &stored) {
        (Origin::Session, Some(stored)) => {
            compare(&stored.value, &value, request, old.as_deref()).map_err(&refuse_against)?;
        }
        // Only a login run, which the person starts, stores the first file:
        // a session never chooses the account every later one works for.
        (Origin::Session, None) => {
            return Err(refuse_against(refused(
                agent,
                "no login is stored, and only a login run stores the first credential file \
                 (humanitl agents login)",
            )));
        }
        (Origin::Login, _) => {}
    }
    // Sealed when the depot is; read under the lock, so a switch of the
    // encryption that ran since this session started is followed.
    let on_disk = seal::for_disk(depot, name, &bytes, request.keys).map_err(&refuse_against)?;
    let warning = store(depot, name, &on_disk, interrupt)
        .map_err(|what| refuse_against(refused(agent, &what)))?;
    let state = WritebackState {
        session: request.session.to_string(),
        at: at.to_owned(),
        sha256: new.clone(),
        account: account.clone(),
    };
    // The credential file is stored; a state that cannot be written only
    // costs the status its line.
    let _ = write_state(depot, &state);
    Ok(Taken::Stored {
        old,
        new,
        account,
        warning,
    })
}

/// The checks against the stored file of a session's write-back.
fn compare(
    stored: &Value,
    value: &Value,
    request: &TakeBack<'_>,
    old: Option<&str>,
) -> Result<(), Diagnostic> {
    let agent = request.depot.agent();
    if let Some(map) = request.format.entries {
        compare_entries(stored, value, &map).map_err(|what| refused(agent, &what))?;
    }
    let fields = request.format.fields;
    let before = present(stored, fields);
    let after = present(value, fields);
    if before != after {
        let gone: Vec<String> = before
            .difference(&after)
            .map(|key| shown_key(key))
            .collect();
        let added: Vec<String> = after
            .difference(&before)
            .map(|key| shown_key(key))
            .collect();
        return Err(refused(
            agent,
            &format!(
                "the file has other keys than the stored one (missing: {gone:?}, added: \
                 {added:?})"
            ),
        ));
    }
    for pointer in &after {
        if kind(stored.pointer(pointer)) != kind(value.pointer(pointer)) {
            return Err(refused(
                agent,
                &format!("{pointer} has another form than in the stored file"),
            ));
        }
    }
    for pointer in request.format.fixed {
        if stored.pointer(pointer) != value.pointer(pointer) {
            return Err(refused(
                agent,
                &format!("{pointer} differs from the stored file; only a login run changes it"),
            ));
        }
    }
    if let Some(expiry) = request.format.expiry {
        let at = |doc: &Value| doc.pointer(expiry.pointer).and_then(Value::as_u64);
        if let (Some(then), Some(now)) = (at(stored), at(value))
            && now == then
            && request.base != old
        {
            // The same expiry, and another session stored its file after
            // this one started: with a rotating refresh token the stored file
            // is the newer one.
            return Err(other(
                agent,
                "the token expires at the same time as the stored one, which another session \
                 stored after this session started; an older token never replaces a newer one",
            ));
        }
        if let (Some(then), Some(now)) = (at(stored), at(value))
            && now < then
        {
            return Err(other(
                agent,
                &format!(
                    "the token expires earlier ({}) than the stored one ({}); an older token \
                     never replaces a newer one",
                    millis(now, expiry.unit),
                    millis(then, expiry.unit)
                ),
            ));
        }
    } else if request.base != old {
        return Err(other(
            agent,
            "another session stored a newer file after this session started; the format \
             carries no expiry, so only a session that started on the stored file writes back",
        ));
    }
    if let Some(pointer) = request.format.account {
        let name = |doc: &Value| {
            doc.pointer(pointer)
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        };
        // Never the same account on `null` or a missing one: only two named
        // and equal accounts are (HUM-515, review).
        let same = matches!((name(stored), name(value)), (Some(old), Some(new)) if old == new);
        if !same {
            return Err(other(
                agent,
                "the file names another account than the login; only a login run changes the \
                 account",
            ));
        }
    }
    Ok(())
}

/// The expiry in milliseconds.
fn millis(value: u64, unit: TimeUnit) -> u64 {
    match unit {
        TimeUnit::Seconds => value.saturating_mul(1000),
        TimeUnit::Milliseconds => value,
    }
}

/// Opens `file` under the directory `dir` for reading: never through a link
/// (`O_NOFOLLOW`), never blocking on a FIFO (`O_NONBLOCK`), and only a
/// regular file of at most [`CREDENTIAL_CAP`] bytes. `Ok(None)` when nothing
/// lies there.
///
/// `file` may lie one directory deeper ([`credential_path_ok`], `OpenCode`'s
/// `opencode/auth.json`, #110): every directory on the way is opened relative
/// to the one before with `O_DIRECTORY` and `O_NOFOLLOW`, so a link in the
/// session's copy never leads the read out of it.
fn read_credential(
    dir: std::os::fd::BorrowedFd<'_>,
    file: &str,
) -> Result<Option<Vec<u8>>, String> {
    if !credential_path_ok(file) {
        return Err("is no path inside the configuration copy".to_owned());
    }
    let mut parts: Vec<&str> = file.split('/').collect();
    let leaf = parts.pop().unwrap_or(file);
    let mut held: Option<OwnedFd> = None;
    for part in parts {
        let at = held.as_ref().map_or(dir, AsFd::as_fd);
        match adopt::open_dir_at(at, OsStr::new(part)) {
            Ok(fd) => held = Some(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
                return Err(format!(
                    "lies under {part}, which is a symbolic link or no directory"
                ));
            }
            Err(err) => return Err(format!("cannot be opened ({err})")),
        }
    }
    read_capped(held.as_ref().map_or(dir, AsFd::as_fd), leaf, CREDENTIAL_CAP)
}

/// [`read_credential`] with another cap.
pub(crate) fn read_capped(
    dir: std::os::fd::BorrowedFd<'_>,
    file: &str,
    cap: u64,
) -> Result<Option<Vec<u8>>, String> {
    if !adopt::relative_inside(file) || Path::new(file).components().count() != 1 {
        return Err("is no plain file name".to_owned());
    }
    let fd: OwnedFd = match rustix::fs::openat(
        dir,
        OsStr::new(file),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(rustix::io::Errno::LOOP) => return Err("is a symbolic link".to_owned()),
        Err(err) => return Err(format!("cannot be opened ({err})")),
    };
    let stat = rustix::fs::fstat(&fd).map_err(|err| format!("cannot be read ({err})"))?;
    let kind = FileType::from_raw_mode(stat.st_mode);
    if kind != FileType::RegularFile {
        return Err(format!("is no regular file ({kind:?})"));
    }
    if u64::try_from(stat.st_size).unwrap_or(u64::MAX) > cap {
        return Err(format!("is larger than {} KiB", cap / 1024));
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| format!("cannot be read ({err})"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return Err(format!("is larger than {} KiB", cap / 1024));
    }
    Ok(Some(bytes))
}

/// The stored credential file: its bytes on disk, sealed or not, its plain
/// bytes and its value.
struct Stored {
    raw: zeroize::Zeroizing<Vec<u8>>,
    plain: zeroize::Zeroizing<Vec<u8>>,
    value: Value,
}

/// The stored credential file; `None` before the first login. A sealed file
/// is opened with the key (HUM-523).
///
/// # Errors
///
/// `AGENT_012` for a file that cannot be read or is no JSON object,
/// `AGENT_020` for a sealed file without a key that opens it.
fn read_stored(
    depot: &Depot,
    file: &str,
    keys: Option<&CredentialKeys>,
) -> Result<Option<Stored>, Diagnostic> {
    let file = credential_name(file);
    let agent = depot.agent();
    let dir = match adopt::open_dir(&depot.credentials_dir()) {
        Ok(dir) => dir,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(err) => {
            return Err(refused(
                agent,
                &format!("credentials/ of the depot cannot be opened ({err})"),
            ));
        }
    };
    let Some(raw) = read_capped(dir.as_fd(), file, CREDENTIAL_CAP + seal::SEAL_OVERHEAD)
        .map_err(|what| refused(agent, &format!("the stored {file} {what}")))?
        .map(zeroize::Zeroizing::new)
    else {
        return Ok(None);
    };
    let plain = seal::from_disk(depot, file, &raw, keys)?;
    if u64::try_from(plain.len()).unwrap_or(u64::MAX) > CREDENTIAL_CAP {
        return Err(refused(
            agent,
            &format!(
                "the stored {file} is larger than {} KiB",
                CREDENTIAL_CAP / 1024
            ),
        ));
    }
    let value =
        parse(&plain).map_err(|what| refused(agent, &format!("the stored file: {what}")))?;
    Ok(Some(Stored { raw, plain, value }))
}

/// The JSON value of a credential file; the root must be an object.
fn parse(bytes: &[u8]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|err| format!("the file is no JSON ({err})"))?;
    if !value.is_object() {
        return Err("the file is no JSON object".to_owned());
    }
    Ok(value)
}

/// The daemon's own bytes of a checked value.
fn serialise(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|err| format!("the file cannot be written ({err})"))
}

/// Checks a document against the whole format: the declared fields
/// ([`check_format`]), at least one of [`CredentialFormat::one_of`] not
/// `null`, and a named account wherever the object holding it is present
/// ([`CredentialFormat::account`]).
///
/// # Errors
///
/// What is wrong, naming the pointer, never a value.
pub(crate) fn check_document(value: &Value, format: &CredentialFormat) -> Result<(), String> {
    if let Some(map) = format.entries {
        check_entries(value, &map)?;
    } else {
        check_format(value, format.fields)?;
    }
    if !format.one_of.is_empty()
        && !format
            .one_of
            .iter()
            .any(|pointer| value.pointer(pointer).is_some_and(|found| !found.is_null()))
    {
        return Err(format!(
            "none of {:?} holds a value; the file is no login",
            format.one_of
        ));
    }
    for rule in format.requires {
        if value.pointer(rule.pointer).and_then(Value::as_str) == Some(rule.equals) {
            for needed in rule.present {
                if value.pointer(needed).is_none_or(Value::is_null) {
                    return Err(format!(
                        "{} is {:?}, and {needed} holds no value",
                        rule.pointer, rule.equals
                    ));
                }
            }
        }
    }
    if let Some(pointer) = format.account {
        let parent = parent_of(pointer);
        let parent_there = parent.is_empty() || value.pointer(parent).is_some();
        let named = value
            .pointer(pointer)
            .and_then(Value::as_str)
            .is_some_and(|name| !name.is_empty());
        if parent_there && !named {
            return Err(format!(
                "{pointer} names no account; a file with tokens must name the account they \
                 belong to"
            ));
        }
    }
    Ok(())
}

/// Checks a map of entries ([`EntryMap`]): every top-level key an entry name,
/// every value an object of a declared kind, checked against its fields.
fn check_entries(value: &Value, map: &EntryMap) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err("the file is no object".to_owned());
    };
    if object.len() > map.max_entries {
        return Err(format!(
            "the file holds {} entries, more than {}",
            object.len(),
            map.max_entries
        ));
    }
    for (name, entry) in object {
        let at = shown_key(&format!("/{}", escape(name)));
        if name.is_empty()
            || name.chars().count() > ENTRY_NAME_CAP
            || !name.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(format!("{at} is no entry name"));
        }
        let fields = entry_fields(entry, map).map_err(|what| format!("{at}: {what}"))?;
        check_format(entry, fields).map_err(|what| format!("{at}: {what}"))?;
    }
    Ok(())
}

/// The fields of the kind an entry names; an error for a value that is no
/// object or names no declared kind.
fn entry_fields(entry: &Value, map: &EntryMap) -> Result<&'static [CredentialField], String> {
    if !entry.is_object() {
        return Err("is no object".to_owned());
    }
    let tag = entry
        .get(map.tag)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("names no kind in {}", map.tag))?;
    map.kinds
        .iter()
        .find(|(kind, _)| *kind == tag)
        .map(|(_, fields)| *fields)
        .ok_or_else(|| {
            format!(
                "names a kind the adapter's format does not know in {}",
                map.tag
            )
        })
}

/// [`compare`] for a map of entries: the same entries, each of the same
/// kind with the same keys of the same forms.
fn compare_entries(stored: &Value, value: &Value, map: &EntryMap) -> Result<(), String> {
    let names = |doc: &Value| -> BTreeSet<String> {
        doc.as_object()
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default()
    };
    let (before, after) = (names(stored), names(value));
    if before != after {
        let gone: Vec<String> = before
            .difference(&after)
            .map(|key| shown_key(key))
            .collect();
        let added: Vec<String> = after
            .difference(&before)
            .map(|key| shown_key(key))
            .collect();
        return Err(format!(
            "the file holds other entries than the stored one (missing: {gone:?}, added: \
             {added:?}); a new provider comes only through a login run"
        ));
    }
    for name in &after {
        let (Some(old), Some(new)) = (stored.get(name), value.get(name)) else {
            continue;
        };
        if old.get(map.tag) != new.get(map.tag) {
            return Err(format!(
                "{} has another kind than in the stored file; only a login run changes it",
                shown_key(name)
            ));
        }
        let fields = entry_fields(new, map)?;
        let (was, is) = (present(old, fields), present(new, fields));
        if was != is {
            return Err(format!(
                "{} has other keys than in the stored file",
                shown_key(name)
            ));
        }
        for pointer in &is {
            if kind(old.pointer(pointer)) != kind(new.pointer(pointer)) {
                return Err(format!(
                    "{}{pointer} has another form than in the stored file",
                    shown_key(name)
                ));
            }
        }
    }
    Ok(())
}

/// Checks a document against the declared fields: every key declared, every
/// value of its form, every required key present where its object is.
///
/// # Errors
///
/// What is wrong, naming the pointer, never a value.
pub(crate) fn check_format(value: &Value, fields: &[CredentialField]) -> Result<(), String> {
    check_object(value, "", fields)?;
    for field in fields.iter().filter(|field| field.required) {
        let parent = parent_of(field.pointer);
        let parent_there = parent.is_empty() || value.pointer(parent).is_some();
        if parent_there && value.pointer(field.pointer).is_none() {
            return Err(format!("the required key {} is missing", field.pointer));
        }
    }
    Ok(())
}

fn check_object(value: &Value, at: &str, fields: &[CredentialField]) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err(format!("{} is no object", shown(at)));
    };
    for (key, child) in object {
        let pointer = format!("{at}/{}", escape(key));
        let Some(field) = fields.iter().find(|field| field.pointer == pointer) else {
            return Err(format!(
                "{} is no key of the adapter's format",
                shown_key(&pointer)
            ));
        };
        check_value(child, &pointer, field.form)?;
        if field.form == ValueForm::Object {
            check_object(child, &pointer, fields)?;
        }
    }
    Ok(())
}

fn check_value(value: &Value, pointer: &str, form: ValueForm) -> Result<(), String> {
    let fits = match form {
        ValueForm::Object => value.is_object(),
        ValueForm::Token => value.as_str().is_some_and(token_ok),
        ValueForm::Text => value.as_str().is_some_and(text_ok),
        ValueForm::Count => value.as_u64().is_some_and(|n| n <= COUNT_CAP),
        ValueForm::Flag => value.is_boolean(),
        ValueForm::TokenOrNull => value.is_null() || value.as_str().is_some_and(token_ok),
        ValueForm::TextOrNull => value.is_null() || value.as_str().is_some_and(text_ok),
        ValueForm::OneOf(allowed) => value.as_str().is_some_and(|text| allowed.contains(&text)),
        ValueForm::TextList => value.as_array().is_some_and(|items| {
            items.len() <= LIST_CAP && items.iter().all(|item| item.as_str().is_some_and(text_ok))
        }),
        ValueForm::TextMap => value.as_object().is_some_and(|object| {
            object.len() <= LIST_CAP
                && object.iter().all(|(key, item)| {
                    !key.is_empty() && text_ok(key) && item.as_str().is_some_and(text_ok)
                })
        }),
    };
    if fits {
        Ok(())
    } else {
        Err(format!("{pointer} is not of the form {form:?}"))
    }
}

fn token_ok(text: &str) -> bool {
    !text.is_empty() && text.len() <= TOKEN_CAP && text.bytes().all(|b| b.is_ascii_graphic())
}

fn text_ok(text: &str) -> bool {
    text.chars().count() <= TEXT_CAP && !text.chars().any(char::is_control)
}

/// The declared pointers present in a document.
fn present<'f>(value: &Value, fields: &'f [CredentialField]) -> BTreeSet<&'f str> {
    fields
        .iter()
        .filter(|field| value.pointer(field.pointer).is_some())
        .map(|field| field.pointer)
        .collect()
}

/// The JSON type of a value, for the comparison of forms.
fn kind(value: Option<&Value>) -> Option<u8> {
    value.map(|value| match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    })
}

fn parent_of(pointer: &str) -> &str {
    pointer.rfind('/').map_or("", |at| &pointer[..at])
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn shown(pointer: &str) -> &str {
    if pointer.is_empty() {
        "the file"
    } else {
        pointer
    }
}

/// Puts `bytes` in place as the stored `file`, as they are: atomic, synced.
/// For the switch of the encryption, which holds the lock ([`super::seal`]).
pub(crate) fn put(depot: &Depot, file: &str, bytes: &[u8]) -> Result<(), String> {
    store(depot, file, bytes, None).map(|_warning| ())
}

/// Writes the new credential file: a temporary file `0600` in the depot's
/// root, `fsync`, `rename` into `credentials/`, `fsync` of both directories.
///
/// Once the `rename` has happened the file is replaced: a directory that
/// cannot be synced afterwards is a warning in the answer, never a refusal.
fn store(
    depot: &Depot,
    file: &str,
    bytes: &[u8],
    interrupt: Option<usize>,
) -> Result<Option<String>, String> {
    let file = credential_name(file);
    let dir = depot.credentials_dir();
    adopt::make_dir(&dir).map_err(|err| format!("credentials/ cannot be created ({err})"))?;
    let temp = depot.root().join(format!(".{file}.writeback"));
    let _ = std::fs::remove_file(&temp);
    let written = write_new(&temp, bytes, interrupt).and_then(|()| {
        std::fs::rename(&temp, dir.join(file))
            .map_err(|err| format!("the new file cannot be put in place ({err})"))
    });
    if let Err(what) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(what);
    }
    let unsynced: Vec<String> = [dir.as_path(), depot.root()]
        .into_iter()
        .filter_map(|synced| {
            adopt::sync_dir(synced)
                .err()
                .map(|err| format!("{} cannot be synced ({err})", synced.display()))
        })
        .collect();
    Ok((!unsynced.is_empty()).then(|| unsynced.join("; ")))
}

/// Refuses an expiry further away than [`EXPIRY_HORIZON`].
fn check_horizon(
    value: &Value,
    format: &CredentialFormat,
    now: std::time::SystemTime,
) -> Result<(), String> {
    let Some(expiry) = format.expiry else {
        return Ok(());
    };
    let Some(at) = value.pointer(expiry.pointer).and_then(Value::as_u64) else {
        return Ok(());
    };
    let horizon = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_add(EXPIRY_HORIZON)
        .as_millis();
    if u128::from(millis(at, expiry.unit)) > horizon {
        return Err(format!(
            "{} lies more than {} days in the future",
            expiry.pointer,
            EXPIRY_HORIZON.as_secs() / 86_400
        ));
    }
    Ok(())
}

/// A key or pointer of a session's file as a finding may quote it: at most
/// [`SHOWN_CAP`] characters, control and bidirectional characters escaped.
fn shown_key(text: &str) -> String {
    let mut out = String::new();
    for (count, c) in text.chars().enumerate() {
        if count == SHOWN_CAP {
            out.push_str("...");
            break;
        }
        let bidi = matches!(
            c,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        );
        if c.is_control() || bidi {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// A new private file with `bytes`, synced. `create_new` is `O_CREAT |
/// O_EXCL`, which never follows a link at the last component.
pub(crate) fn write_new(path: &Path, bytes: &[u8], interrupt: Option<usize>) -> Result<(), String> {
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| format!("{} cannot be created ({err})", path.display()))?;
    let cut = interrupt.map_or(bytes.len(), |at| at.min(bytes.len()));
    out.write_all(&bytes[..cut])
        .map_err(|err| format!("{} cannot be written ({err})", path.display()))?;
    if cut < bytes.len() {
        return Err(format!(
            "{} was interrupted while being written",
            path.display()
        ));
    }
    out.sync_all()
        .map_err(|err| format!("{} cannot be synced ({err})", path.display()))
}

/// Writes [`WRITEBACK_STATE`] the same way.
fn write_state(depot: &Depot, state: &WritebackState) -> Result<(), String> {
    let bytes = serde_json::to_vec(state).map_err(|err| err.to_string())?;
    let temp = depot.root().join(format!(".{WRITEBACK_STATE}.tmp"));
    let _ = std::fs::remove_file(&temp);
    write_new(&temp, &bytes, None)
        .and_then(|()| {
            std::fs::rename(&temp, depot.root().join(WRITEBACK_STATE))
                .map_err(|err| err.to_string())
        })
        .and_then(|()| adopt::sync_dir(depot.root()).map_err(|err| err.to_string()))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&temp);
        })
}

impl Depot {
    /// The last write-back of the credential file, if the depot names one.
    #[must_use]
    pub fn last_writeback(&self) -> Option<WritebackState> {
        let dir = adopt::open_dir(self.root()).ok()?;
        let bytes = adopt::read_small(dir.as_fd(), Path::new(WRITEBACK_STATE), 4096)
            .ok()
            .flatten()?;
        serde_json::from_slice(&bytes).ok()
    }
}

/// The named keys of a login run, read and checked, ready to be written
/// ([`prepare_named_fields`], [`write_named_fields`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedUpdate {
    /// The depot's file with the named keys replaced, the daemon's bytes.
    bytes: Vec<u8>,
    /// The keys taken over.
    keys: Vec<String>,
}

/// Takes the named keys of [`NamedFields::file`] in the login run's copy into
/// the depot's copy of that file in `config/`: [`prepare_named_fields`], then
/// [`write_named_fields`]. Returns the keys taken over.
///
/// # Errors
///
/// As the two steps; the depot's file is then unchanged.
pub fn take_named_fields(
    config_copy: &Path,
    depot: &Depot,
    named: &NamedFields,
) -> Result<Vec<String>, Diagnostic> {
    match prepare_named_fields(config_copy, depot, named)? {
        Some(update) => write_named_fields(depot, named, &update, &lock(depot)?),
        None => Ok(Vec::new()),
    }
}

/// Reads and checks the named keys of [`NamedFields::file`] in the login
/// run's copy and builds the depot's new copy of that file, without writing
/// anything: each named key present in the session's file, checked against
/// its declared subtree, replaces the same key; every other key of the
/// depot's file stays as it is, and nothing else of the session's file is
/// used. `None` when the session's file names none of them.
///
/// # Errors
///
/// `AGENT_012` for a file that is no regular JSON object within
/// [`NAMED_FILE_CAP`], named keys above [`CREDENTIAL_CAP`] or out of their
/// form, or a depot file that is no JSON object.
pub fn prepare_named_fields(
    config_copy: &Path,
    depot: &Depot,
    named: &NamedFields,
) -> Result<Option<NamedUpdate>, Diagnostic> {
    let agent = depot.agent();
    let dir = adopt::open_dir(config_copy).map_err(|err| {
        refused(
            agent,
            &format!("the session's configuration copy cannot be opened ({err})"),
        )
    })?;
    let Some(raw) = read_capped(dir.as_fd(), named.file, NAMED_FILE_CAP)
        .map_err(|what| refused(agent, &format!("{} {what}", named.file)))?
    else {
        return Ok(None);
    };
    let session = parse(&raw).map_err(|what| refused(agent, &format!("{}: {what}", named.file)))?;
    drop(raw);
    // Only the named top-level keys, each with its declared subtree; the
    // rest of the session's file is never looked at.
    let mut taken = serde_json::Map::new();
    for top in named
        .fields
        .iter()
        .filter(|field| parent_of(field.pointer).is_empty())
    {
        if let Some(value) = session.pointer(top.pointer) {
            let key = top.pointer[1..].replace("~1", "/").replace("~0", "~");
            taken.insert(key, value.clone());
        }
    }
    drop(session);
    if taken.is_empty() {
        return Ok(None);
    }
    let taken_value = Value::Object(taken);
    let size = serialise(&taken_value)
        .map_err(|what| refused(agent, &what))?
        .len();
    if u64::try_from(size).unwrap_or(u64::MAX) > CREDENTIAL_CAP {
        return Err(refused(
            agent,
            &format!(
                "{}: the named keys are larger than {} KiB",
                named.file,
                CREDENTIAL_CAP / 1024
            ),
        ));
    }
    check_format(&taken_value, named.fields)
        .map_err(|what| refused(agent, &format!("{}: {what}", named.file)))?;
    let Value::Object(taken) = taken_value else {
        return Ok(None);
    };
    let config_fd = match adopt::open_dir(&depot.config_dir()) {
        Ok(fd) => Some(fd),
        Err(rustix::io::Errno::NOENT) => None,
        Err(err) => {
            return Err(refused(agent, &format!("config/ cannot be opened ({err})")));
        }
    };
    let existing = match &config_fd {
        Some(fd) => read_capped(fd.as_fd(), named.file, NAMED_FILE_CAP)
            .map_err(|what| refused(agent, &format!("the depot's {} {what}", named.file)))?,
        None => None,
    };
    let mut target = match existing {
        Some(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(map)) => map,
            _ => {
                return Err(refused(
                    agent,
                    &format!("the depot's {} is no JSON object", named.file),
                ));
            }
        },
        None => serde_json::Map::new(),
    };
    let keys: Vec<String> = taken.keys().cloned().collect();
    target.extend(taken);
    let bytes = serialise(&Value::Object(target)).map_err(|what| refused(agent, &what))?;
    // The merged file must stay readable for the next login.
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > NAMED_FILE_CAP {
        return Err(refused(
            agent,
            &format!(
                "the depot's {} would grow beyond {} MiB",
                named.file,
                NAMED_FILE_CAP / (1024 * 1024)
            ),
        ));
    }
    Ok(Some(NamedUpdate { bytes, keys }))
}

/// Writes a prepared update of the named keys: a temporary file `0600`,
/// `rename` into `config/`, under the agent's lock. Returns the keys.
///
/// # Errors
///
/// `AGENT_012` when the file cannot be written; it is then as it was.
pub fn write_named_fields(
    depot: &Depot,
    named: &NamedFields,
    update: &NamedUpdate,
    _held: &CredentialLock,
) -> Result<Vec<String>, Diagnostic> {
    let agent = depot.agent();
    let config = depot.config_dir();
    adopt::make_dir(&config)
        .map_err(|err| refused(agent, &format!("config/ cannot be created ({err})")))?;
    let temp = depot.root().join(format!(".{}.named", named.file));
    let _ = std::fs::remove_file(&temp);
    write_new(&temp, &update.bytes, None)
        .and_then(|()| {
            std::fs::rename(&temp, config.join(named.file)).map_err(|err| err.to_string())
        })
        .map_err(|what| {
            let _ = std::fs::remove_file(&temp);
            refused(
                agent,
                &format!("the depot's {} cannot be written ({what})", named.file),
            )
        })?;
    // Replaced: a directory that cannot be synced now is no refusal.
    let _ = adopt::sync_dir(&config);
    Ok(update.keys.clone())
}

/// The lock of an agent's credential file and named fields: one writer at a
/// time, held across every step a login must be able to undo.
#[derive(Debug)]
pub struct CredentialLock(#[allow(dead_code)] super::store::Reservation);

/// Takes the lock of `depot`'s credential file.
///
/// # Errors
///
/// `AGENT_012` while another write-back of the agent holds it.
pub fn lock(depot: &Depot) -> Result<CredentialLock, Diagnostic> {
    depot
        .lock_named("credentials")
        .map(CredentialLock)
        .map_err(|found| {
            refused(
                depot.agent(),
                &format!(
                    "another write-back of the agent holds the depot ({})",
                    found.why
                ),
            )
        })
}

/// The stored credential file and the state of the last write-back as they
/// are, so a login can put them back ([`restore`]).
///
/// The credential file is kept as it lies on disk, sealed or not, and is put
/// back byte for byte; the lock held between both keeps a switch of the
/// encryption out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    credential: Option<zeroize::Zeroizing<Vec<u8>>>,
    /// The SHA-256 of the plain file.
    sha256: Option<String>,
    state: Option<Vec<u8>>,
}

impl Snapshot {
    /// The SHA-256 of the stored credential file, plain; `None` without one.
    #[must_use]
    pub fn sha256(&self) -> Option<String> {
        self.sha256.clone()
    }
}

/// Reads what [`restore`] puts back.
///
/// # Errors
///
/// `AGENT_012` when the stored file cannot be read, `AGENT_020` when it is
/// sealed and does not open.
pub fn snapshot(
    depot: &Depot,
    file: &str,
    keys: Option<&CredentialKeys>,
    _held: &CredentialLock,
) -> Result<Snapshot, Diagnostic> {
    let stored = read_stored(depot, file, keys)?;
    let sha256 = stored.as_ref().map(|stored| sha256_hex(&stored.plain));
    let credential = stored.map(|stored| stored.raw);
    let state = adopt::open_dir(depot.root())
        .ok()
        .and_then(|dir| adopt::read_small(dir.as_fd(), Path::new(WRITEBACK_STATE), 4096).ok())
        .flatten();
    Ok(Snapshot {
        credential,
        sha256,
        state,
    })
}

/// Puts the credential file and the state back as `snapshot` saw them: the
/// earlier file in place with `rename`, or no file when there was none.
///
/// # Errors
///
/// `AGENT_012` naming what failed; the stored file may then still be the
/// newer one.
pub fn restore(
    depot: &Depot,
    file: &str,
    snapshot: &Snapshot,
    _held: &CredentialLock,
) -> Result<(), Diagnostic> {
    restore_files(depot, file, snapshot).map_err(|what| refused(depot.agent(), &what))
}

fn restore_files(depot: &Depot, file: &str, snapshot: &Snapshot) -> Result<(), String> {
    let file = credential_name(file);
    match &snapshot.credential {
        Some(bytes) => {
            store(depot, file, bytes, None)?;
        }
        None => match std::fs::remove_file(depot.credentials_dir().join(file)) {
            Ok(()) => {
                let _ = adopt::sync_dir(&depot.credentials_dir());
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(format!("the new {file} cannot be removed ({err})")),
        },
    }
    let state = depot.root().join(WRITEBACK_STATE);
    match &snapshot.state {
        Some(bytes) => {
            let temp = depot.root().join(format!(".{WRITEBACK_STATE}.tmp"));
            let _ = std::fs::remove_file(&temp);
            write_new(&temp, bytes, None)
                .and_then(|()| std::fs::rename(&temp, &state).map_err(|err| err.to_string()))
                .inspect_err(|_| {
                    let _ = std::fs::remove_file(&temp);
                })?;
        }
        None => {
            let _ = std::fs::remove_file(&state);
        }
    }
    let _ = adopt::sync_dir(depot.root());
    Ok(())
}

/// `AGENT_012`: the write-back was refused.
pub(crate) fn refused(agent: &str, what: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_012, Severity::Warning)
        .why(format!(
            "the credential file of {agent} was not taken back into the depot: {what}; the \
             stored login stays as it was"
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents login {agent}"
        )))
        .build()
}

/// `AGENT_016`: an older token or another account.
fn other(agent: &str, what: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_016, Severity::Warning)
        .why(format!(
            "the credential file of {agent} was not taken back into the depot: {what}; the \
             stored login stays as it was"
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents login {agent}"
        )))
        .build()
}

/// Now, RFC 3339 in UTC, whole seconds.
fn now() -> String {
    humanitl_core::time::rfc3339_secs(humanitl_core::time::from_system_time(
        std::time::SystemTime::now(),
    ))
}

/// Where the credential file of a depot lies; for the status and tests.
#[must_use]
pub fn stored_path(depot: &Depot, file: &str) -> PathBuf {
    depot.credentials_dir().join(credential_name(file))
}

#[cfg(test)]
mod tests;
