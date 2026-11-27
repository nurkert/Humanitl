//! Tests of the sealed credential files (HUM-523). The keyring is a stand-in
//! of `secret-tool` that keeps its entries as files in the test's own
//! directory; the person's keyring is never asked.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::AGENT_020;
use humanitl_core::ids::SessionId;

use std::sync::{Arc, Mutex};

use super::*;
use crate::agent::depot::credential::{
    CredentialField, CredentialFormat, Expiry, Origin, TakeBack, Taken, TimeUnit, ValueForm,
    take_back,
};
use crate::agent::depot::store::tests::filled;

const FILE: &str = ".credentials.json";
const AGENT: &str = "claude";

const FORMAT: CredentialFormat = CredentialFormat {
    fields: &[
        CredentialField {
            pointer: "/token",
            form: ValueForm::Token,
            required: true,
        },
        CredentialField {
            pointer: "/expires",
            form: ValueForm::Count,
            required: true,
        },
    ],
    expiry: Some(Expiry {
        pointer: "/expires",
        unit: TimeUnit::Seconds,
    }),
    account: None,
    one_of: &[],
    requires: &[],
    fixed: &[],
    entries: None,
    evidence: "stand-in of the tests",
};

/// A stand-in of `secret-tool`: `store` writes stdin to `dir/store/<name>`,
/// `lookup` prints it or ends with 1 and nothing on stderr (no entry),
/// `clear` removes it.
pub(crate) fn fake_keyring(dir: &Path) -> PathBuf {
    let store = dir.join("keyring");
    std::fs::create_dir_all(&store).unwrap();
    let script = dir.join("secret-tool");
    crate::test_support::write_program(
        &script,
        &format!(
            "#!/bin/sh\nstore='{}'\nfor last; do :; done\n\
             case \"$1\" in\n  store) cat > \"$store/$last\" ;;\n  \
             lookup) [ -f \"$store/$last\" ] || exit 1; cat \"$store/$last\" ;;\n  \
             clear) rm -f \"$store/$last\" ;;\nesac\n",
            store.display()
        ),
    );
    script
}

struct Fixture {
    _base: tempfile::TempDir,
    base: PathBuf,
    paths: Paths,
    depot: Depot,
}

impl Fixture {
    fn new() -> Self {
        let base = tempfile::tempdir().unwrap();
        // Resolved: on macOS the temp dir lies behind the link /var -> /private/var.
        let root = base.path().canonicalize().unwrap();
        let paths = Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", root.join("home").to_str().unwrap()),
            ("XDG_RUNTIME_DIR", root.join("run").to_str().unwrap()),
            ("XDG_DATA_HOME", root.join("data").to_str().unwrap()),
        ]));
        std::fs::create_dir_all(paths.data_dir()).unwrap();
        let depot = filled(&paths.data_dir(), AGENT, AGENT);
        assert_eq!(depot.root(), Depot::of(&paths, AGENT).root());
        Self {
            _base: base,
            base: root,
            paths,
            depot,
        }
    }

    /// Keys behind the stand-in keyring.
    fn keys(&self) -> CredentialKeys {
        let store =
            KeyStore::for_paths(&self.paths).with_secret_tool(Some(fake_keyring(&self.base)));
        CredentialKeys::new(store)
    }

    /// Keys without any keyring.
    fn no_keyring(&self) -> CredentialKeys {
        CredentialKeys::new(KeyStore::for_paths(&self.paths).with_secret_tool(None))
    }

    fn stored_path(&self) -> PathBuf {
        self.depot.credentials_dir().join(FILE)
    }

    fn store_plain(&self, text: &str) {
        std::fs::write(self.stored_path(), text).unwrap();
        std::fs::set_permissions(self.stored_path(), std::fs::Permissions::from_mode(0o600))
            .unwrap();
    }

    fn stored(&self) -> Vec<u8> {
        std::fs::read(self.stored_path()).unwrap()
    }

    /// Every file under the data directory, with its bytes.
    fn data_files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                let meta = std::fs::symlink_metadata(&path).unwrap();
                if meta.is_dir() {
                    walk(&path, out);
                } else if meta.is_file() {
                    out.push((path.clone(), std::fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.paths.data_dir(), &mut out);
        out.sort();
        out
    }

    fn request<'a>(
        &'a self,
        base: Option<&'a str>,
        keys: Option<&'a CredentialKeys>,
    ) -> TakeBack<'a> {
        TakeBack {
            depot: &self.depot,
            file: FILE,
            format: &FORMAT,
            session: SessionId::new(),
            base,
            origin: Origin::Session,
            keys,
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn a_sealed_file_opens_only_with_its_key_agent_and_name() {
    let key = SealKey::generate().unwrap();
    let plain = br#"{"token":"secret-token-123","expires":5}"#;
    let sealed = seal(&key, AGENT, FILE, plain).unwrap();
    assert!(is_sealed(&sealed));
    assert!(!is_sealed(plain));
    assert!(!contains(&sealed, b"secret-token-123"));
    assert_eq!(&sealed[..MAGIC.len()], MAGIC);
    assert_eq!(sealed[MAGIC.len()], VERSION);
    assert_eq!(sealed.len(), plain.len() + HEADER_LEN + 16);
    assert_eq!(open(&key, AGENT, FILE, &sealed).unwrap().as_slice(), plain);

    // A fresh nonce on every write: the same file seals differently.
    let again = seal(&key, AGENT, FILE, plain).unwrap();
    assert_ne!(
        sealed[MAGIC.len() + 1..HEADER_LEN],
        again[MAGIC.len() + 1..HEADER_LEN]
    );

    let other = SealKey::generate().unwrap();
    assert!(open(&other, AGENT, FILE, &sealed).is_err(), "another key");
    assert!(open(&key, "codex", FILE, &sealed).is_err(), "another agent");
    assert!(
        open(&key, AGENT, "auth.json", &sealed).is_err(),
        "another file"
    );
    let mut flipped = sealed.clone();
    *flipped.last_mut().unwrap() ^= 1;
    assert!(open(&key, AGENT, FILE, &flipped).is_err(), "a damaged tag");
    let mut version = sealed.clone();
    version[MAGIC.len()] = 2;
    let why = open(&key, AGENT, FILE, &version).unwrap_err();
    assert!(why.contains("version 2"), "{why}");
    assert!(open(&key, AGENT, FILE, &sealed[..HEADER_LEN + 3]).is_err());

    let hex = key.to_hex();
    assert_eq!(SealKey::from_hex(&hex).unwrap(), key);
    assert!(SealKey::from_hex("zz").is_none());
    assert_eq!(format!("{key:?}"), "SealKey(••••)");
}

/// Acceptance 1: with the switch on no credential file lies in plain text in
/// the depot, the key lies only in the keyring, and the switch off brings
/// back the very file.
#[test]
fn switching_on_seals_every_file_and_off_brings_it_back() {
    let fx = Fixture::new();
    let plain = r#"{"token":"secret-token-123","expires":5}"#;
    fx.store_plain(plain);
    let keys = fx.keys();

    let switched = switch(&fx.paths, &keys, true).unwrap();
    assert!(switched.enabled);
    assert_eq!(switched.converted, vec![format!("{AGENT}/{FILE}")]);
    assert!(enabled(&fx.paths));
    assert!(files_sealed(&fx.depot));
    let stored = fx.stored();
    assert!(is_sealed(&stored));
    let mode = std::fs::metadata(fx.stored_path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let key = keys.key().unwrap();
    let hex = key.to_hex();
    for (path, bytes) in fx.data_files() {
        assert!(
            !contains(&bytes, b"secret-token-123"),
            "{} holds the token in plain text",
            path.display()
        );
        assert!(
            !contains(&bytes, hex.as_bytes()) && !contains(&bytes, key.0.as_ref()),
            "{} holds the key",
            path.display()
        );
    }
    assert_eq!(
        std::fs::read_to_string(fx.base.join("keyring").join(KEY_NAME)).unwrap(),
        hex.as_str()
    );
    // Twice is the same: nothing to convert.
    assert!(switch(&fx.paths, &keys, true).unwrap().converted.is_empty());

    let off = switch(&fx.paths, &keys, false).unwrap();
    assert_eq!(off.converted.len(), 1);
    assert!(!enabled(&fx.paths));
    assert_eq!(fx.stored(), plain.as_bytes());
    assert!(!files_sealed(&fx.depot));
}

/// Without a keyring the setting is refused, and nothing changes: no state,
/// no sealed file, no key file anywhere.
#[test]
fn without_a_keyring_the_switch_is_refused() {
    let fx = Fixture::new();
    let plain = r#"{"token":"t","expires":5}"#;
    fx.store_plain(plain);
    let before = fx.data_files();
    let found = switch(&fx.paths, &fx.no_keyring(), true).unwrap_err();
    assert_eq!(found.code, AGENT_020);
    assert!(found.why.contains("keyring"), "{}", found.why);
    assert!(!enabled(&fx.paths));
    assert_eq!(fx.stored(), plain.as_bytes());
    let after: Vec<_> = fx
        .data_files()
        .into_iter()
        .filter(|(path, _)| path.extension().is_none_or(|ext| ext != "lock"))
        .collect();
    let before: Vec<_> = before
        .into_iter()
        .filter(|(path, _)| path.extension().is_none_or(|ext| ext != "lock"))
        .collect();
    assert_eq!(after, before);
    assert!(!fx.paths.key_dir().exists(), "no key file as a fallback");
}

/// A session gets plain text in its copy; its renewal goes back sealed, and
/// the stored file opens to the renewal. A plain write-back turns this red.
#[test]
fn a_session_gets_plain_text_and_its_renewal_is_stored_sealed() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"old-token","expires":1}"#);
    let keys = fx.keys();
    switch(&fx.paths, &keys, true).unwrap();

    let scratch = crate::agent::depot::scratch::create(
        &fx.paths,
        SessionId::new(),
        &fx.depot,
        None,
        Some(&keys),
    )
    .unwrap();
    let copy = scratch.path().join(FILE);
    assert_eq!(
        std::fs::read_to_string(&copy).unwrap(),
        r#"{"token":"old-token","expires":1}"#
    );
    let mode = std::fs::metadata(&copy).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let base = scratch.base(FILE).unwrap().to_owned();

    std::fs::write(&copy, r#"{"token":"new-token","expires":2}"#).unwrap();
    let taken = take_back(scratch.path(), &fx.request(Some(&base), Some(&keys))).unwrap();
    let Taken::Stored { old, new, .. } = taken else {
        panic!("not stored: {taken:?}");
    };
    // The hashes are those of the plain files, as before the encryption.
    assert_eq!(old.as_deref(), Some(base.as_str()));
    let stored = fx.stored();
    assert!(
        is_sealed(&stored),
        "the renewal lies in plain text in the depot"
    );
    assert!(!contains(&stored, b"new-token"));
    let opened = open(&keys.key().unwrap(), AGENT, FILE, &stored).unwrap();
    assert_eq!(credential::sha256_hex(&opened), new);
    let value: serde_json::Value = serde_json::from_slice(&opened).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"token": "new-token", "expires": 2})
    );
}

/// A session that started before the switch writes in the state after it:
/// the guard reads the state under the lock.
#[test]
fn a_write_back_follows_a_switch_made_during_the_session() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let keys = fx.keys();
    let scratch = crate::agent::depot::scratch::create(
        &fx.paths,
        SessionId::new(),
        &fx.depot,
        None,
        Some(&keys),
    )
    .unwrap();
    let base = scratch.base(FILE).unwrap().to_owned();
    switch(&fx.paths, &keys, true).unwrap();
    std::fs::write(scratch.path().join(FILE), r#"{"token":"b","expires":2}"#).unwrap();
    take_back(scratch.path(), &fx.request(Some(&base), Some(&keys))).unwrap();
    assert!(is_sealed(&fx.stored()));

    // And back: after the switch off, the next renewal is plain.
    switch(&fx.paths, &keys, false).unwrap();
    let base = credential::sha256_hex(&fx.stored());
    std::fs::write(scratch.path().join(FILE), r#"{"token":"c","expires":3}"#).unwrap();
    take_back(scratch.path(), &fx.request(Some(&base), Some(&keys))).unwrap();
    let stored = fx.stored();
    assert!(!is_sealed(&stored));
    let value: serde_json::Value = serde_json::from_slice(&stored).unwrap();
    assert_eq!(value, serde_json::json!({"token": "c", "expires": 3}));
}

/// Another key in the keyring: the start, the write-back and the switch off
/// each say `AGENT_020`, and the stored file stays as it was.
#[test]
fn a_wrong_key_is_a_finding_and_leaves_the_file() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    switch(&fx.paths, &fx.keys(), true).unwrap();
    let sealed = fx.stored();
    // The keyring now holds another key; fresh keys read it.
    let other = SealKey::generate().unwrap();
    std::fs::write(
        fx.base.join("keyring").join(KEY_NAME),
        other.to_hex().as_bytes(),
    )
    .unwrap();
    let keys = fx.keys();

    let found = crate::agent::depot::scratch::create(
        &fx.paths,
        SessionId::new(),
        &fx.depot,
        None,
        Some(&keys),
    )
    .unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
    assert!(found.why.contains("does not open"), "{}", found.why);

    let copy = fx.base.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(copy.join(FILE), r#"{"token":"b","expires":2}"#).unwrap();
    let refused = take_back(&copy, &fx.request(None, Some(&keys))).unwrap_err();
    assert_eq!(refused.found.code, AGENT_020, "{}", refused.found.why);
    assert_eq!(fx.stored(), sealed);

    let found = switch(&fx.paths, &keys, false).unwrap_err();
    assert_eq!(found.code, AGENT_020);
    assert_eq!(fx.stored(), sealed);
    assert!(enabled(&fx.paths));

    // Without any access to the keyring a sealed file does not open either.
    let refused = take_back(&copy, &fx.request(None, None)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_020);
    assert_eq!(fx.stored(), sealed);
}

/// A keyring that does not answer a lookup is never taken for an empty
/// one: no new key replaces the entry, also when no file is sealed right now
/// (switched off earlier, the key kept) and the keyring would take a store.
#[test]
fn a_silent_keyring_never_gets_a_new_key() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    switch(&fx.paths, &fx.keys(), true).unwrap();
    switch(&fx.paths, &fx.keys(), false).unwrap();
    let entry = std::fs::read(fx.base.join("keyring").join(KEY_NAME)).unwrap();
    // A lookup that fails with a message (a bus that does not answer), and a
    // store that would succeed.
    let silent_tool = fx.base.join("silent-tool");
    crate::test_support::write_program(
        &silent_tool,
        &format!(
            "#!/bin/sh\nstore='{}'\nfor last; do :; done\n\
             case \"$1\" in\n  store) cat > \"$store/$last\" ;;\n  \
             lookup) echo 'Cannot autolaunch D-Bus without X11 $DISPLAY' >&2; exit 1 ;;\nesac\n",
            fx.base.join("keyring").display()
        ),
    );
    let silent =
        CredentialKeys::new(KeyStore::for_paths(&fx.paths).with_secret_tool(Some(silent_tool)));
    let found = switch(&fx.paths, &silent, true).unwrap_err();
    assert_eq!(found.code, AGENT_020);
    assert_eq!(
        std::fs::read(fx.base.join("keyring").join(KEY_NAME)).unwrap(),
        entry,
        "the entry the files were sealed with stays"
    );
    assert!(!enabled(&fx.paths));
    assert!(!is_sealed(&fx.stored()));
}

/// `agents.encrypt_credentials` held in memory, as the tests' `config.toml`.
fn memory_setting(initial: bool) -> (Setting, Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let value = Arc::new(AtomicBool::new(initial));
    let read = Arc::clone(&value);
    let write = Arc::clone(&value);
    let setting = Setting {
        read: Arc::new(move || Ok(read.load(Ordering::SeqCst))),
        write: Arc::new(move |on| {
            write.store(on, Ordering::SeqCst);
            Ok(())
        }),
    };
    (setting, value)
}

fn setting_of(value: &std::sync::atomic::AtomicBool) -> bool {
    value.load(std::sync::atomic::Ordering::SeqCst)
}

/// A hand edit of `config.toml` takes effect at the next start: the daemon
/// reconciles to the value read under the lock, and a settled depot is left
/// alone. A plain file under the state that no journal lists is no file to
/// seal but a finding (a damaged header), and stays as it is.
#[test]
fn reconcile_brings_the_depot_to_the_setting() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let (setting, value) = memory_setting(false);
    let keys = fx.keys().with_setting(setting);
    assert!(reconcile(&fx.paths, &keys).unwrap().is_none());
    value.store(true, std::sync::atomic::Ordering::SeqCst);
    let done = reconcile(&fx.paths, &keys).unwrap().unwrap();
    assert!(done.enabled);
    assert!(is_sealed(&fx.stored()));
    assert!(reconcile(&fx.paths, &keys).unwrap().is_none());
    let plain = open(&keys.key().unwrap(), AGENT, FILE, &fx.stored()).unwrap();
    std::fs::write(fx.stored_path(), plain.as_slice()).unwrap();
    let found = reconcile(&fx.paths, &keys).unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
    assert_eq!(fx.stored(), plain.as_slice());
}

/// A sealed file whose header is damaged is never taken for plain text: the
/// start, the switch off and the switch on again say `AGENT_020`, and the
/// file stays byte for byte.
#[test]
fn a_damaged_header_is_a_finding_not_plain_text() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let keys = fx.keys();
    switch(&fx.paths, &keys, true).unwrap();
    let mut damaged = fx.stored();
    damaged[0] ^= 0x20;
    std::fs::write(fx.stored_path(), &damaged).unwrap();
    let found = crate::agent::depot::scratch::create(
        &fx.paths,
        SessionId::new(),
        &fx.depot,
        None,
        Some(&keys),
    )
    .unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
    for on in [true, false] {
        let found = switch(&fx.paths, &keys, on).unwrap_err();
        assert_eq!(found.code, AGENT_020, "{}", found.why);
        assert_eq!(fx.stored(), damaged);
    }
    assert!(enabled(&fx.paths));
}

/// Switching on asks the keyring every time: an entry that is gone, or that
/// holds another key than the one read before, refuses, also while the
/// daemon still has a key in memory.
#[test]
fn switching_on_checks_the_keyring_every_time() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let keys = fx.keys();
    switch(&fx.paths, &keys, true).unwrap();
    switch(&fx.paths, &keys, false).unwrap();
    let entry = fx.base.join("keyring").join(KEY_NAME);
    // Another key in the keyring while the daemon remembers the first.
    std::fs::write(&entry, SealKey::generate().unwrap().to_hex().as_bytes()).unwrap();
    let found = switch(&fx.paths, &keys, true).unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
    assert!(found.why.contains("another key"), "{}", found.why);
    assert!(!enabled(&fx.paths));
    // A keyring that holds nothing while the daemon remembers a key: refused
    // when a file is sealed.
    let fresh = fx.keys();
    switch(&fx.paths, &fresh, true).unwrap();
    std::fs::remove_file(&entry).unwrap();
    let found = switch(&fx.paths, &fresh, true).unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
}

/// No plain login while the setting says `true` and the depot is not
/// sealed yet: the write-back refuses, the stored file stays.
#[test]
fn a_write_back_refuses_plain_text_while_the_setting_says_true() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let (setting, _value) = memory_setting(true);
    let keys = fx.keys().with_setting(setting);
    let copy = fx.base.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(copy.join(FILE), r#"{"token":"b","expires":2}"#).unwrap();
    let base = credential::sha256_hex(&fx.stored());
    let refused = take_back(&copy, &fx.request(Some(&base), Some(&keys))).unwrap_err();
    assert_eq!(refused.found.code, AGENT_020, "{}", refused.found.why);
    assert_eq!(fx.stored(), br#"{"token":"a","expires":1}"#);
}

/// Every crash point of a switch, both ways: until it is finished, every
/// file reads as the login it was and nothing is written; after `recover`
/// the state, every file and the setting agree with the target, and the
/// journal is gone.
#[test]
fn an_interrupted_switch_is_finished_forward_at_every_crash_point() {
    use super::switch::{Fault, JOURNAL, switch_with};
    const SECOND: &str = "second.json";
    let first = r#"{"token":"a","expires":1}"#;
    let second = r#"{"token":"s","expires":9}"#;
    for on in [true, false] {
        for fault in [
            Fault::Journal,
            Fault::State,
            Fault::FirstFile,
            Fault::Files,
            Fault::Config,
        ] {
            let fx = Fixture::new();
            fx.store_plain(first);
            let second_path = fx.depot.credentials_dir().join(SECOND);
            std::fs::write(&second_path, second).unwrap();
            std::fs::set_permissions(&second_path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let (setting, value) = memory_setting(false);
            let keys = fx.keys().with_setting(setting);
            if !on {
                switch(&fx.paths, &keys, true).unwrap();
            }
            let case = format!("on {on}, {fault:?}");
            switch_with(&fx.paths, &keys, on, Some(fault)).unwrap_err();
            assert!(
                fx.paths.data_dir().join("agents").join(JOURNAL).is_file(),
                "{case}"
            );
            // Readable as before, and nothing written meanwhile.
            for (name, text) in [(FILE, first), (SECOND, second)] {
                let bytes = std::fs::read(fx.depot.credentials_dir().join(name)).unwrap();
                let plain = from_disk(&fx.depot, name, &bytes, Some(&keys)).unwrap();
                assert_eq!(plain.as_slice(), text.as_bytes(), "{case} {name}");
            }
            let refused = for_disk(&fx.depot, FILE, b"{}", Some(&keys)).unwrap_err();
            assert_eq!(refused.code, AGENT_020, "{case}");
            // While `config.toml` says true, no file lies in plain text:
            // switching on says it last, switching off says false first.
            if setting_of(&value) {
                for name in [FILE, SECOND] {
                    let bytes = std::fs::read(fx.depot.credentials_dir().join(name)).unwrap();
                    assert!(is_sealed(&bytes), "{case}: true with {name} in plain text");
                }
            }
            if !on && fault != Fault::Journal {
                assert!(
                    !setting_of(&value),
                    "{case}: switching off says false first"
                );
            }

            assert!(recover(&fx.paths, &keys).unwrap(), "{case}");
            assert!(
                !fx.paths.data_dir().join("agents").join(JOURNAL).exists(),
                "{case}"
            );
            assert_eq!(enabled(&fx.paths), on, "{case}");
            assert_eq!(setting_of(&value), on, "{case}");
            for (name, text) in [(FILE, first), (SECOND, second)] {
                let bytes = std::fs::read(fx.depot.credentials_dir().join(name)).unwrap();
                assert_eq!(is_sealed(&bytes), on, "{case} {name}");
                let plain = from_disk(&fx.depot, name, &bytes, Some(&keys)).unwrap();
                assert_eq!(plain.as_slice(), text.as_bytes(), "{case} {name}");
            }
            assert!(!recover(&fx.paths, &keys).unwrap(), "{case}");
        }
    }
}

/// A step that fails without a crash (here `config.toml`) turns the switch
/// around: files, state and setting are as before, and no journal stays.
#[test]
fn a_failed_setting_write_turns_the_switch_back() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let setting = Setting {
        read: Arc::new(|| Ok(false)),
        write: Arc::new(|on| {
            if on {
                Err(unavailable("config.toml cannot be written", None))
            } else {
                Ok(())
            }
        }),
    };
    let keys = fx.keys().with_setting(setting);
    let found = switch(&fx.paths, &keys, true).unwrap_err();
    assert!(found.why.contains("turned back"), "{}", found.why);
    assert!(!enabled(&fx.paths));
    assert_eq!(fx.stored(), br#"{"token":"a","expires":1}"#);
    assert!(
        !fx.paths
            .data_dir()
            .join("agents")
            .join(super::switch::JOURNAL)
            .exists()
    );
}

/// What the start racing a switch answered.
type Raced = Arc<Mutex<Option<Result<Option<Switched>, Diagnostic>>>>;

/// A start that reconciles while a switch writes `config.toml` cannot take
/// the lock and switch back to the value before: one lock holds the files
/// and the setting together.
#[test]
fn a_start_during_the_setting_write_does_not_switch_back() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let value = Arc::new(AtomicBool::new(false));
    let paths = fx.paths.clone();
    let inner_keys: Arc<Mutex<Option<CredentialKeys>>> = Arc::new(Mutex::new(None));
    let raced: Raced = Arc::new(Mutex::new(None));
    let (read_value, write_value) = (Arc::clone(&value), Arc::clone(&value));
    let (keys_for_race, raced_out) = (Arc::clone(&inner_keys), Arc::clone(&raced));
    let setting = Setting {
        read: Arc::new(move || Ok(read_value.load(Ordering::SeqCst))),
        write: Arc::new(move |on| {
            // A session start right now, before the value lands.
            if let Some(keys) = keys_for_race.lock().unwrap().clone() {
                *raced_out.lock().unwrap() = Some(reconcile(&paths, &keys));
            }
            write_value.store(on, Ordering::SeqCst);
            Ok(())
        }),
    };
    let keys = fx.keys().with_setting(setting);
    *inner_keys.lock().unwrap() = Some(keys.clone());
    switch(&fx.paths, &keys, true).unwrap();
    *inner_keys.lock().unwrap() = None;
    let raced = raced.lock().unwrap().take().unwrap();
    assert!(raced.is_err(), "the start took the lock: {raced:?}");
    assert!(enabled(&fx.paths));
    assert!(is_sealed(&fx.stored()));
    assert!(value.load(Ordering::SeqCst));
}

/// A write-back temporary file a crash left at the depot's root holds a
/// plain login; switching on removes it before the setting can say true,
/// and so does the recovery of an interrupted switch.
#[test]
fn a_left_over_write_back_file_goes_before_a_switch_on() {
    use super::switch::{Fault, switch_with};
    for crashed in [false, true] {
        let fx = Fixture::new();
        fx.store_plain(r#"{"token":"a","expires":1}"#);
        let keys = fx.keys();
        // A login whose first write-back a crash cut off: no stored file of
        // that name, only the temporary file.
        let temp = fx.depot.root().join(".first-login.json.writeback");
        if crashed {
            switch_with(&fx.paths, &keys, true, Some(Fault::Journal)).unwrap_err();
            std::fs::write(&temp, br#"{"token":"secret-token-123"}"#).unwrap();
            recover(&fx.paths, &keys).unwrap();
        } else {
            std::fs::write(&temp, br#"{"token":"secret-token-123"}"#).unwrap();
            switch(&fx.paths, &keys, true).unwrap();
        }
        assert!(!temp.exists(), "crashed {crashed}");
        for (path, bytes) in fx.data_files() {
            assert!(
                !contains(&bytes, b"secret-token-123"),
                "{} holds the login in plain text, crashed {crashed}",
                path.display()
            );
        }
    }
}

/// A journal is followed only when it names this daemon's own format, a
/// registered agent and a credential file of it; a forged one that names a
/// path outside the depot is refused and nothing is touched.
#[test]
fn a_forged_journal_is_refused_and_followed_nowhere() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let keys = fx.keys();
    switch(&fx.paths, &keys, true).unwrap();
    switch(&fx.paths, &keys, false).unwrap();
    let outside = fx.base.join("outside.json");
    std::fs::write(&outside, br#"{"token":"o"}"#).unwrap();
    let journal = fx
        .paths
        .data_dir()
        .join("agents")
        .join(super::switch::JOURNAL);
    for forged in [
        r#"{"version":1,"target":true,"files":[["claude","../../../outside.json"]]}"#.to_owned(),
        r#"{"version":1,"target":true,"files":[["../..","outside.json"]]}"#.to_owned(),
        r#"{"version":1,"target":true,"files":[["claude","missing.json"]]}"#.to_owned(),
        r#"{"version":9,"target":true,"files":[]}"#.to_owned(),
    ] {
        std::fs::write(&journal, &forged).unwrap();
        let found = recover(&fx.paths, &keys).unwrap_err();
        assert_eq!(found.code, AGENT_020, "{forged}");
        assert!(found.why.contains("refused"), "{}", found.why);
        assert_eq!(std::fs::read(&outside).unwrap(), br#"{"token":"o"}"#);
        assert!(!is_sealed(&fx.stored()), "{forged}");
        assert!(!enabled(&fx.paths));
    }
}

/// Enable, disable, the keyring entry removed, enable again: the daemon read
/// a key before, so no new one is created silently. A depot whose state
/// records another key id is refused as well, even without a file.
#[test]
fn a_key_once_read_is_never_replaced_silently() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let keys = fx.keys();
    switch(&fx.paths, &keys, true).unwrap();
    switch(&fx.paths, &keys, false).unwrap();
    let entry = fx.base.join("keyring").join(KEY_NAME);
    std::fs::remove_file(&entry).unwrap();
    let found = switch(&fx.paths, &keys, true).unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
    assert!(!entry.exists());
    assert!(!enabled(&fx.paths));

    let empty = Fixture::new();
    std::fs::remove_file(empty.stored_path()).ok();
    switch(&empty.paths, &empty.keys(), true).unwrap();
    std::fs::write(
        empty.base.join("keyring").join(KEY_NAME),
        SealKey::generate().unwrap().to_hex().as_bytes(),
    )
    .unwrap();
    let found = switch(&empty.paths, &empty.keys(), true).unwrap_err();
    assert_eq!(found.code, AGENT_020, "{}", found.why);
    assert!(found.why.contains("encrypted with"), "{}", found.why);
}

/// The session's copy decides sealed or plain under the agent's credential
/// lock: while a switch holds it, the copy waits its turn and is refused
/// instead of reading a file between two forms.
#[test]
fn the_copy_of_a_session_reads_under_the_credential_lock() {
    let fx = Fixture::new();
    fx.store_plain(r#"{"token":"a","expires":1}"#);
    let held = crate::agent::depot::credential::lock(&fx.depot).unwrap();
    let found = crate::agent::depot::scratch::create(
        &fx.paths,
        SessionId::new(),
        &fx.depot,
        None,
        Some(&fx.keys()),
    )
    .unwrap_err();
    assert_eq!(found.code.as_str(), "AGENT_012", "{}", found.why);
    drop(held);
    crate::agent::depot::scratch::create(
        &fx.paths,
        SessionId::new(),
        &fx.depot,
        None,
        Some(&fx.keys()),
    )
    .unwrap();
}
