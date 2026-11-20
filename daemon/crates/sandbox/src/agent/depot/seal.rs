//! The credential files of the depot, encrypted (ADR-0023 point 6, HUM-523).
//!
//! With `agents.encrypt_credentials = true` no credential file lies in plain
//! text under `agents/<id>/credentials/`. The parts:
//!
//! - **The cipher** is ChaCha20-Poly1305 from `ring::aead`, an established
//!   crate already in the tree through rustls; nothing here is cryptography
//!   of our own. Every write draws a fresh 96-bit nonce from
//!   `ring::rand::SystemRandom`.
//! - **The format** of a sealed file: [`MAGIC`] (8 bytes), the format
//!   [`VERSION`] (1 byte), the nonce (12 bytes), then the ciphertext with its
//!   16-byte tag. A JSON file never starts with [`MAGIC`], so a plain and a
//!   sealed file are told apart by their first bytes, and a reader takes
//!   both. The header, the agent and the file name are the associated data:
//!   a sealed file moved to another agent or name, or with another version
//!   byte, does not open.
//! - **The key** is 32 random bytes in the system keyring (Secret Service on
//!   Linux, the login Keychain on macOS) under `humanitl/agents-depot`, as
//!   hex text, reached through [`humanitl_config::secret::KeyStore`] like
//!   `llm.api_key_ref`, but only the keyring: never the file of the
//!   fallback, never a key file next to the data. Without a keyring that
//!   answers the setting is refused (`AGENT_020`). The daemon keeps the key in
//!   memory once read ([`CredentialKeys`]); a rotation of the key is not
//!   part of this.
//! - **The state on disk** is [`SEAL_STATE`] under `agents/`: present, every
//!   credential file is written sealed, and a plain file there is refused
//!   unless a switch in progress lists it. Every writer reads it under the
//!   agent's credential lock ([`super::credential::lock`]), so a session that
//!   started before a switch writes in the state after it.
//! - **The switch** is a transaction with a journal (`switch.rs`): the
//!   journal names the target and the files to convert before anything
//!   changes, and it goes only after the files, the state and
//!   `config.toml` agree. An interrupted switch is finished forward before a
//!   status answer, a session start or a login run, under the same lock that
//!   holds the whole switch together with the write of `config.toml`.
//! - **Where it is opened:** only into a session's scratch copy
//!   ([`super::scratch::create`]) and inside the guard of the write-back,
//!   which compares against the stored login. A wrong key leaves every file
//!   as it is and says `AGENT_020`.
//!
//! The limit, named in `docs/SECURITY.md` 5.7: the daemon holds the key and
//! decrypts for every session, so a process that runs as the daemon's user
//! and can ask the keyring (or read the daemon's memory) reads the login as
//! before. The encryption protects the files at rest: a copy of the data
//! directory, a backup, a disk image without the keyring.

use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

mod switch;

pub use self::switch::{Switched, reconcile, recover, switch};

use humanitl_config::Paths;
use humanitl_config::secret::{ApiKey, KeyStore};
use humanitl_core::diagnostics::codes::AGENT_020;
use humanitl_core::{Diagnostic, FixAction, Severity};
use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::rand::{SecureRandom as _, SystemRandom};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::credential::{self, CREDENTIAL_CAP};
use super::store::Depot;

/// The first bytes of a sealed credential file.
pub const MAGIC: &[u8; 8] = b"HTLSEAL\0";

/// The version of the format this daemon writes and reads.
pub const VERSION: u8 = 1;

/// The bytes before the ciphertext: magic, version, nonce.
pub const HEADER_LEN: usize = MAGIC.len() + 1 + NONCE_LEN;

/// What sealing adds to a file: the header and the tag.
pub const SEAL_OVERHEAD: u64 = (HEADER_LEN + 16) as u64;

/// The name of the key in the system keyring (`humanitl/agents-depot`).
pub const KEY_NAME: &str = "agents-depot";

/// The file under `agents/` that says the credential files are sealed.
pub const SEAL_STATE: &str = "credential-seal.json";

/// The configuration key of the switch.
pub const SETTING: &str = "agents.encrypt_credentials";

/// The bytes of the key.
const KEY_LEN: usize = 32;

/// The name of the switch's own lock, `agents/credential-seal.switch.lock`.
const LOCK_ID: &str = "credential-seal";

/// The key of the depot's credential files, in memory. Overwritten when
/// dropped; its `Debug` form shows nothing.
#[derive(Clone, PartialEq, Eq)]
pub struct SealKey(Zeroizing<[u8; KEY_LEN]>);

impl std::fmt::Debug for SealKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SealKey(••••)")
    }
}

impl SealKey {
    /// A fresh random key.
    ///
    /// # Errors
    ///
    /// The system's random source failed.
    pub(crate) fn generate() -> Result<Self, String> {
        let mut bytes = Zeroizing::new([0_u8; KEY_LEN]);
        SystemRandom::new()
            .fill(bytes.as_mut())
            .map_err(|_| "the system's random source failed".to_owned())?;
        Ok(Self(bytes))
    }

    /// The key from its hex text in the keyring; `None` for any other text.
    fn from_hex(text: &str) -> Option<Self> {
        let text = text.as_bytes();
        if text.len() != KEY_LEN * 2 {
            return None;
        }
        let mut bytes = Zeroizing::new([0_u8; KEY_LEN]);
        for (slot, pair) in bytes.iter_mut().zip(text.chunks_exact(2)) {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            *slot = u8::try_from(high * 16 + low).ok()?;
        }
        Some(Self(bytes))
    }

    /// The id of the key the state records: SHA-256 of a fixed label and
    /// the key, in hex; it names the key without revealing it.
    #[must_use]
    pub(crate) fn id(&self) -> String {
        let mut input = Zeroizing::new(b"humanitl-agents-depot-key\0".to_vec());
        input.extend_from_slice(self.0.as_ref());
        credential::sha256_hex(&input)
    }

    /// The key as hex text for the keyring.
    fn to_hex(&self) -> Zeroizing<String> {
        use std::fmt::Write as _;
        let mut out = Zeroizing::new(String::with_capacity(KEY_LEN * 2));
        for byte in self.0.iter() {
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    fn aead(&self) -> Result<LessSafeKey, String> {
        UnboundKey::new(&CHACHA20_POLY1305, self.0.as_ref())
            .map(LessSafeKey::new)
            .map_err(|_| "the key does not fit the cipher".to_owned())
    }
}

/// Whether `bytes` are a sealed file (by their first bytes).
#[must_use]
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// The associated data: the header, the agent and the file name.
fn associated(header: &[u8], agent: &str, file: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(header.len() + agent.len() + file.len() + 1);
    aad.extend_from_slice(header);
    aad.extend_from_slice(agent.as_bytes());
    aad.push(0);
    aad.extend_from_slice(file.as_bytes());
    aad
}

/// Seals `plain` as the credential file `file` of `agent`, with a fresh
/// nonce.
///
/// # Errors
///
/// The random source or the cipher failed; nothing is written.
pub(crate) fn seal(
    key: &SealKey,
    agent: &str,
    file: &str,
    plain: &[u8],
) -> Result<Vec<u8>, String> {
    let mut nonce = [0_u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| "the system's random source failed".to_owned())?;
    let mut out = Vec::with_capacity(HEADER_LEN + plain.len() + 16);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    let aad = associated(&out, agent, file);
    // The plain text is copied once, into memory that is overwritten when
    // it goes.
    let mut body = Zeroizing::new(plain.to_vec());
    key.aead()?
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad.as_slice()),
            &mut *body,
        )
        .map_err(|_| "the file could not be sealed".to_owned())?;
    out.extend_from_slice(&body);
    Ok(out)
}

/// Opens a sealed credential file `file` of `agent`.
///
/// # Errors
///
/// What is wrong, never a byte of the file: no sealed file, a version this
/// daemon does not know, or a file that does not open with this key (another
/// key, a damaged or moved file).
pub(crate) fn open(
    key: &SealKey,
    agent: &str,
    file: &str,
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, String> {
    if !is_sealed(sealed) || sealed.len() < HEADER_LEN + 16 {
        return Err("it is no sealed credential file".to_owned());
    }
    let version = sealed[MAGIC.len()];
    if version != VERSION {
        return Err(format!(
            "it is sealed in format version {version}, and this daemon reads version {VERSION}"
        ));
    }
    let (header, body) = sealed.split_at(HEADER_LEN);
    let mut nonce = [0_u8; NONCE_LEN];
    nonce.copy_from_slice(&header[MAGIC.len() + 1..]);
    let aad = associated(header, agent, file);
    let mut body = Zeroizing::new(body.to_vec());
    let plain_len = key
        .aead()?
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad.as_slice()),
            body.as_mut_slice(),
        )
        .map_err(|_| {
            "it does not open with the key from the keyring (another key, or a damaged file)"
                .to_owned()
        })?
        .len();
    body.truncate(plain_len);
    Ok(body)
}

/// Reads `agents.encrypt_credentials` as `config.toml` says it now.
pub type ReadSetting = Arc<dyn Fn() -> Result<bool, Diagnostic> + Send + Sync>;

/// Writes `agents.encrypt_credentials` into `config.toml`.
pub type WriteSetting = Arc<dyn Fn(bool) -> Result<(), Diagnostic> + Send + Sync>;

/// The setting `agents.encrypt_credentials`, read and written by the daemon
/// (`humanitl_ipc::agents::setting`). A switch writes it inside its
/// transaction, and every writer of a credential file reads it.
#[derive(Clone)]
pub struct Setting {
    /// Reads the value.
    pub read: ReadSetting,
    /// Writes the value.
    pub write: WriteSetting,
}

/// The key of the depot's credential files: read from the system keyring
/// once, then kept in memory; with the [`Setting`] where the caller has one.
/// Cheap to clone; the clones share the memory.
#[derive(Clone)]
pub struct CredentialKeys {
    store: KeyStore,
    cached: Arc<Mutex<Option<SealKey>>>,
    setting: Option<Setting>,
}

impl std::fmt::Debug for CredentialKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialKeys")
            .field("store", &self.store)
            .field("setting", &self.setting.is_some())
            .finish_non_exhaustive()
    }
}

impl CredentialKeys {
    /// The keys behind this keyring access. Tests hand a store with a stand-in
    /// of `secret-tool` ([`KeyStore::with_secret_tool`]) and never touch the
    /// person's keyring.
    #[must_use]
    pub fn new(store: KeyStore) -> Self {
        Self {
            store,
            cached: Arc::new(Mutex::new(None)),
            setting: None,
        }
    }

    /// The same keys with the setting `agents.encrypt_credentials`: a switch
    /// writes it in its transaction, and a write of a credential file refuses
    /// plain text while it says `true`.
    #[must_use]
    pub fn with_setting(mut self, setting: Setting) -> Self {
        self.setting = Some(setting);
        self
    }

    /// The setting as `config.toml` says it now; `None` without one.
    ///
    /// # Errors
    ///
    /// The finding of reading `config.toml`.
    pub fn configured(&self) -> Result<Option<bool>, Diagnostic> {
        self.setting
            .as_ref()
            .map(|setting| (setting.read)())
            .transpose()
    }

    /// Writes the setting; nothing without one.
    fn write_setting(&self, value: bool) -> Result<(), Diagnostic> {
        self.setting
            .as_ref()
            .map_or(Ok(()), |setting| (setting.write)(value))
    }

    /// Whether this system has a keyring to ask at all. Whether it answers
    /// shows only a lookup.
    #[must_use]
    pub fn keyring_available(&self) -> bool {
        self.store.keyring_ref(KEY_NAME).is_some()
    }

    /// The key, from memory or the keyring.
    ///
    /// # Errors
    ///
    /// `AGENT_020` when the keyring cannot be asked, does not answer, or
    /// holds no key.
    pub fn key(&self) -> Result<SealKey, Diagnostic> {
        if let Some(key) = self.memory().clone() {
            return Ok(key);
        }
        let text = self
            .store
            .lookup_in_keyring(KEY_NAME)
            .map_err(|found| self.no_keyring(&found.why))?
            .ok_or_else(no_entry)?;
        let key = SealKey::from_hex(text.expose()).ok_or_else(not_a_key)?;
        *self.memory() = Some(key.clone());
        Ok(key)
    }

    /// The key for switching on, always asked from the keyring, never only
    /// from memory: a removed or locked entry refuses, and an entry that
    /// holds another key than the one read before refuses too. A new key is
    /// created only when the keyring answers that it holds none and
    /// `may_create` (no file is sealed); a keyring that does not answer is
    /// never taken for an empty one.
    fn key_for_enabling(&self, may_create: bool) -> Result<SealKey, Diagnostic> {
        let entry = self
            .store
            .lookup_in_keyring(KEY_NAME)
            .map_err(|found| self.no_keyring(&found.why))?;
        match entry {
            Some(text) => {
                let key = SealKey::from_hex(text.expose()).ok_or_else(not_a_key)?;
                if self.memory().as_ref().is_some_and(|known| *known != key) {
                    return Err(unavailable(
                        "the keyring entry humanitl/agents-depot holds another key than the one \
                         this daemon read before; the files may be sealed with the earlier one",
                        None,
                    ));
                }
                *self.memory() = Some(key.clone());
                Ok(key)
            }
            // A key this daemon read before and that is gone now is never
            // replaced silently: the person may have removed it on purpose,
            // or a backup still holds files sealed with it.
            None if may_create && self.memory().is_none() => {
                let key = SealKey::generate().map_err(|why| unavailable(&why, None))?;
                let text =
                    ApiKey::new(&key.to_hex()).map_err(|found| unavailable(&found.why, None))?;
                self.store
                    .store_in_keyring(KEY_NAME, &text)
                    .map_err(|found| self.no_keyring(&found.why))?;
                *self.memory() = Some(key.clone());
                Ok(key)
            }
            None => Err(no_entry()),
        }
    }

    fn memory(&self) -> std::sync::MutexGuard<'_, Option<SealKey>> {
        self.cached.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `AGENT_020` for a keyring that cannot be used, with what helps.
    fn no_keyring(&self, why: &str) -> Diagnostic {
        let fix = (!self.keyring_available() && !cfg!(target_os = "macos"))
            .then(|| FixAction::CopyCommand("sudo apt-get install libsecret-tools".to_owned()));
        unavailable(
            &format!(
                "the system keyring cannot be used ({why}); the key of the depot's credential \
                 files lies only there, never in a file next to the data"
            ),
            fix,
        )
    }
}

fn no_entry() -> Diagnostic {
    unavailable(
        "the system keyring holds no key humanitl/agents-depot, and the credential files of the \
         depot are sealed with one",
        None,
    )
}

fn not_a_key() -> Diagnostic {
    unavailable(
        "the keyring entry humanitl/agents-depot is no key of the depot",
        None,
    )
}

/// `AGENT_020` with a reason.
#[must_use]
pub fn unavailable(why: &str, fix: Option<FixAction>) -> Diagnostic {
    Diagnostic::builder(AGENT_020, Severity::Error)
        .why(format!(
            "the credential files of the depot cannot be encrypted or opened: {why}; nothing was \
             changed"
        ))
        .maybe_fix(fix)
        .build()
}

/// `AGENT_020` for a stored file that does not open.
pub(crate) fn unopened(agent: &str, file: &str, why: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_020, Severity::Error)
        .why(format!(
            "the stored credential file {file} of {agent} {why}; it is left as it is, and \
             nothing was written in plain text instead"
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents login {agent}"
        )))
        .build()
}

/// What [`SEAL_STATE`] holds: no secret, only which key seals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SealState {
    version: u8,
    key: String,
    /// [`SealKey::id`] of the key the depot is sealed with.
    #[serde(default)]
    key_id: Option<String>,
}

/// The key id the state records, if any.
fn state_key_id(agents: &Path) -> Option<String> {
    let bytes = std::fs::read(agents.join(SEAL_STATE)).ok()?;
    serde_json::from_slice::<SealState>(&bytes).ok()?.key_id
}

/// `agents/` of the depots, as [`Depot::of`] resolves it.
fn agents_dir(paths: &Paths) -> PathBuf {
    Depot::of(paths, LOCK_ID)
        .root()
        .parent()
        .map_or_else(|| paths.agents_dir(), Path::to_path_buf)
}

/// Whether the credential files under this depot's `agents/` are written
/// sealed: [`SEAL_STATE`] is a regular file there.
#[must_use]
pub fn sealed_for(depot: &Depot) -> bool {
    depot.root().parent().is_some_and(state_present)
}

fn state_present(agents: &Path) -> bool {
    std::fs::symlink_metadata(agents.join(SEAL_STATE)).is_ok_and(|meta| meta.is_file())
}

/// Whether the depots are sealed now.
#[must_use]
pub fn enabled(paths: &Paths) -> bool {
    state_present(&agents_dir(paths))
}

/// The bytes to store for `plain` in `depot`: sealed when the depot is.
///
/// Called under the agent's credential lock. Plain text is refused while a
/// switch is unfinished, and while `agents.encrypt_credentials` says `true`
/// and the depot is not sealed yet (a hand edit that no start has followed):
/// never a plain login into a depot that should be sealed.
///
/// # Errors
///
/// `AGENT_020` when the depot is sealed and the key cannot be had or the
/// file cannot be sealed, or plain text is refused; nothing is written then.
pub fn for_disk(
    depot: &Depot,
    file: &str,
    plain: &[u8],
    keys: Option<&CredentialKeys>,
) -> Result<Vec<u8>, Diagnostic> {
    if depot.root().parent().is_some_and(switch::journal_present) {
        return Err(unavailable(
            "a switch of the encryption was interrupted; it is finished at the next status or \
             start, and nothing is written before",
            None,
        ));
    }
    if sealed_for(depot) {
        let keys = keys.ok_or_else(no_keys)?;
        return seal(&keys.key()?, depot.agent(), file, plain)
            .map_err(|why| unavailable(&why, None));
    }
    if let Some(keys) = keys
        && keys.configured()? == Some(true)
    {
        return Err(unavailable(
            "agents.encrypt_credentials is true and the depot is not encrypted yet; the next \
             start of a session from the depot encrypts it, and no plain login is written before",
            Some(FixAction::CopyCommand(format!(
                "humanitl config set {SETTING} true"
            ))),
        ));
    }
    Ok(plain.to_vec())
}

/// The plain bytes of a stored credential file: opened when sealed.
///
/// A file without the header in a sealed depot is refused, unless the
/// journal of a switch in progress lists it: a damaged header is never taken
/// for plain text.
///
/// # Errors
///
/// `AGENT_020` when the file is sealed and no key opens it, or is not sealed
/// in a sealed depot.
pub fn from_disk(
    depot: &Depot,
    file: &str,
    stored: &[u8],
    keys: Option<&CredentialKeys>,
) -> Result<Zeroizing<Vec<u8>>, Diagnostic> {
    if !is_sealed(stored) {
        if sealed_for(depot) && !switch::pending(depot, file) {
            return Err(unopened(
                depot.agent(),
                file,
                "is not sealed, and the depot is (a damaged header or a format this daemon does \
                 not know)",
            ));
        }
        return Ok(Zeroizing::new(stored.to_vec()));
    }
    let keys = keys.ok_or_else(no_keys)?;
    open(&keys.key()?, depot.agent(), file, stored)
        .map_err(|why| unopened(depot.agent(), file, &why))
}

fn no_keys() -> Diagnostic {
    unavailable(
        "the files are sealed, and this process was given no access to the keyring",
        None,
    )
}

/// Whether `depot` holds credential files and every one of them is sealed:
/// what the status shows as "encrypted".
#[must_use]
pub fn files_sealed(depot: &Depot) -> bool {
    let (sealed, plain) = counted(depot);
    sealed > 0 && plain == 0
}

/// How many credential files of `depot` are sealed and how many plain; a
/// depot that cannot be read counts as none.
fn counted(depot: &Depot) -> (usize, usize) {
    credential_files(depot).map_or((0, 0), |files| {
        let sealed = files.iter().filter(|(_, bytes)| is_sealed(bytes)).count();
        (sealed, files.len() - sealed)
    })
}

/// The regular files of `credentials/` with their bytes as stored, in
/// memory that is overwritten when it goes.
/// One credential file: its name and its bytes as stored.
type StoredFile = (String, Zeroizing<Vec<u8>>);

fn credential_files(depot: &Depot) -> Result<Vec<StoredFile>, Diagnostic> {
    let dir = match super::adopt::open_dir(&depot.credentials_dir()) {
        Ok(dir) => dir,
        Err(rustix::io::Errno::NOENT) => return Ok(Vec::new()),
        Err(err) => {
            return Err(credential::refused(
                depot.agent(),
                &format!("credentials/ cannot be opened ({err})"),
            ));
        }
    };
    let entries = std::fs::read_dir(depot.credentials_dir()).map_err(|err| {
        credential::refused(
            depot.agent(),
            &format!("credentials/ cannot be read ({err})"),
        )
    })?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let bytes = credential::read_capped(dir.as_fd(), &name, CREDENTIAL_CAP + SEAL_OVERHEAD)
            .map_err(|what| credential::refused(depot.agent(), &format!("{name} {what}")))?;
        if let Some(bytes) = bytes {
            files.push((name, Zeroizing::new(bytes)));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn write_state(agents: &Path, key: &SealKey) -> Result<(), String> {
    let state = SealState {
        version: VERSION,
        key: format!("{}:humanitl/{KEY_NAME}", keyring_scheme()),
        key_id: Some(key.id()),
    };
    let bytes = serde_json::to_vec(&state).map_err(|err| err.to_string())?;
    switch::write_small(agents, SEAL_STATE, &bytes)
}

fn remove_state(agents: &Path) -> Result<(), String> {
    switch::remove_small(agents, SEAL_STATE)
}

/// The scheme the key's place carries in [`SEAL_STATE`].
const fn keyring_scheme() -> &'static str {
    if cfg!(target_os = "macos") {
        humanitl_config::secret::KEYCHAIN_SCHEME
    } else {
        humanitl_config::secret::SECRET_SERVICE_SCHEME
    }
}

#[cfg(test)]
pub(crate) mod tests;
