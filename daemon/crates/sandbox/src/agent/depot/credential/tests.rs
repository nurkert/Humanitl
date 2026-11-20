//! Tests of the write-back guard with a stand-in adapter whose credential
//! file is JSON (HUM-520). No real agent's file is read: the format below is
//! made up for the tests and names no vendor.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{AGENT_012, AGENT_016};
use humanitl_core::ids::SessionId;

use super::*;
use crate::agent::depot::store::tests::filled;

const FILE: &str = ".credentials.json";

const fn field(pointer: &'static str, form: ValueForm, required: bool) -> CredentialField {
    CredentialField {
        pointer,
        form,
        required,
    }
}

const FIELDS: &[CredentialField] = &[
    field("/oauth", ValueForm::Object, true),
    field("/oauth/accessToken", ValueForm::Token, true),
    field("/oauth/refreshToken", ValueForm::Token, true),
    field("/oauth/expiresAt", ValueForm::Count, true),
    field("/oauth/account", ValueForm::Text, true),
    field("/oauth/scopes", ValueForm::TextList, false),
    field("/oauth/subscriptionType", ValueForm::Text, false),
];

/// The stand-in format: an expiry in milliseconds and an account.
const FORMAT: CredentialFormat = CredentialFormat {
    fields: FIELDS,
    expiry: Some(Expiry {
        pointer: "/oauth/expiresAt",
        unit: TimeUnit::Milliseconds,
    }),
    account: Some("/oauth/account"),
    evidence: "stand-in of the tests",
};

/// The same without an expiry.
const NO_EXPIRY: CredentialFormat = CredentialFormat {
    expiry: None,
    ..FORMAT
};

/// The named fields of the stand-in's account file.
const NAMED: NamedFields = NamedFields {
    file: ".claude.json",
    fields: &[
        field("/oauthAccount", ValueForm::Object, false),
        field("/oauthAccount/emailAddress", ValueForm::Text, true),
        field("/oauthAccount/accountUuid", ValueForm::Text, true),
    ],
    evidence: "stand-in of the tests",
};

/// A credential file of the stand-in.
fn doc(token: &str, expires: u64, account: &str) -> String {
    format!(
        r#"{{"oauth":{{"accessToken":"{token}","refreshToken":"r-{token}","expiresAt":{expires},"account":"{account}","scopes":["a","b"]}}}}"#
    )
}

struct Setup {
    _base: tempfile::TempDir,
    depot: Depot,
    root: PathBuf,
}

impl Setup {
    fn new() -> Self {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "standin", "standin");
        let root = base.path().to_path_buf();
        Self {
            _base: base,
            depot,
            root,
        }
    }

    /// A fresh configuration copy of a session, `0700`.
    fn session_copy(&self, name: &str) -> PathBuf {
        let dir = self.root.join("scratch").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    fn stored(&self) -> Option<Vec<u8>> {
        std::fs::read(self.depot.credentials_dir().join(FILE)).ok()
    }

    /// Stores `text` as a login would, and returns its SHA-256 as the base of
    /// a session that starts now.
    fn login(&self, text: &str) -> String {
        let copy = self.session_copy("login");
        std::fs::write(copy.join(FILE), text).unwrap();
        let taken = take_back(&copy, &self.request(Origin::Login, None, &FORMAT)).unwrap();
        let Taken::Stored { new, .. } = taken else {
            panic!("the login was not stored: {taken:?}");
        };
        std::fs::remove_dir_all(copy).unwrap();
        new
    }

    fn request<'a>(
        &'a self,
        origin: Origin,
        base: Option<&'a str>,
        format: &'a CredentialFormat,
    ) -> TakeBack<'a> {
        TakeBack {
            depot: &self.depot,
            file: FILE,
            format,
            session: SessionId::new(),
            base,
            origin,
        }
    }

    /// A session that started on the stored file and leaves `text`.
    fn session_writes(&self, name: &str, base: &str, text: &str) -> Result<Taken, Refused> {
        let copy = self.session_copy(name);
        std::fs::write(copy.join(FILE), text).unwrap();
        take_back(&copy, &self.request(Origin::Session, Some(base), &FORMAT))
    }
}

fn canonical(text: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::from_str::<Value>(text).unwrap()).unwrap()
}

#[test]
fn a_valid_renewal_is_taken_back_private() {
    let setup = Setup::new();
    let base = setup.login(&doc("t1", 1_000, "acc-1"));
    let renewed = doc("t2", 2_000, "acc-1");
    let taken = setup.session_writes("s", &base, &renewed).unwrap();
    let Taken::Stored {
        old, new, account, ..
    } = taken
    else {
        panic!("not stored: {taken:?}");
    };
    assert_eq!(old.as_deref(), Some(base.as_str()));
    assert_eq!(account.as_deref(), Some("acc-1"));
    assert_eq!(setup.stored().unwrap(), canonical(&renewed));
    assert_eq!(new, sha256_hex(&canonical(&renewed)));
    let mode = std::fs::metadata(setup.depot.credentials_dir().join(FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    // The depot stays usable for the next session.
    setup.depot.check().unwrap();
    assert!(setup.depot.logged_in());
    let state = setup.depot.last_writeback().unwrap();
    assert_eq!(state.sha256, new);
}

#[test]
fn a_first_login_takes_a_file_of_the_format() {
    let setup = Setup::new();
    assert!(setup.stored().is_none());
    setup.login(&doc("t1", 1_000, "acc-1"));
    assert_eq!(
        setup.stored().unwrap(),
        canonical(&doc("t1", 1_000, "acc-1"))
    );
}

/// Each of these is refused at the first login, and the depot keeps no
/// credential file.
#[test]
fn a_first_login_refuses_what_is_out_of_the_format() {
    let big = format!(
        r#"{{"oauth":{{"accessToken":"{}","refreshToken":"r","expiresAt":1,"account":"a"}}}}"#,
        "x".repeat(65 * 1024)
    );
    let cases = [
        (
            "a missing required key",
            r#"{"oauth":{"accessToken":"t","expiresAt":1,"account":"a"}}"#.to_owned(),
            "/oauth/refreshToken",
        ),
        (
            "a wrong type",
            r#"{"oauth":{"accessToken":"t","refreshToken":"r","expiresAt":"soon","account":"a"}}"#
                .to_owned(),
            "/oauth/expiresAt",
        ),
        (
            "an additional key",
            r#"{"oauth":{"accessToken":"t","refreshToken":"r","expiresAt":1,"account":"a"},"hooks":{}}"#
                .to_owned(),
            "/hooks",
        ),
        ("a file above the cap", big, "64 KiB"),
    ];
    for (what, text, named) in cases {
        let setup = Setup::new();
        let copy = setup.session_copy("login");
        std::fs::write(copy.join(FILE), &text).unwrap();
        let refused = take_back(&copy, &setup.request(Origin::Login, None, &FORMAT)).unwrap_err();
        assert_eq!(refused.found.code, AGENT_012, "{what}");
        assert!(
            refused.found.why.contains(named),
            "{what}: {}",
            refused.found.why
        );
        assert!(setup.stored().is_none(), "{what}: the depot holds a file");
        assert!(!setup.depot.logged_in(), "{what}");
    }
}

/// A later write-back must have the stored file's keys, no more and no
/// fewer, even where the format allows the key.
#[test]
fn a_later_writeback_that_adds_or_drops_an_optional_key_is_refused() {
    let without_scopes =
        r#"{"oauth":{"accessToken":"t2","refreshToken":"r2","expiresAt":2000,"account":"acc-1"}}"#;
    let with_subscription = r#"{"oauth":{"accessToken":"t2","refreshToken":"r2","expiresAt":2000,"account":"acc-1","scopes":["a"],"subscriptionType":"pro"}}"#;
    for (what, text) in [
        ("drops scopes", without_scopes),
        ("adds subscriptionType", with_subscription),
    ] {
        // The format admits both files on their own.
        check_format(&serde_json::from_str(text).unwrap(), FIELDS).unwrap();
        let setup = Setup::new();
        let first = doc("t1", 1_000, "acc-1");
        let base = setup.login(&first);
        let refused = setup.session_writes("s", &base, text).unwrap_err();
        assert_eq!(refused.found.code, AGENT_012, "{what}");
        assert!(
            refused.found.why.contains("other keys"),
            "{what}: {}",
            refused.found.why
        );
        assert_eq!(refused.old.as_deref(), Some(base.as_str()));
        assert_eq!(setup.stored().unwrap(), canonical(&first), "{what}");
    }
}

#[test]
fn a_session_writeback_refuses_what_is_out_of_the_format() {
    let cases = [
        (
            "an unknown key",
            r#"{"oauth":{"accessToken":"t2","refreshToken":"r2","expiresAt":2000,"account":"acc-1","scopes":["a"]},"hooks":{"x":"y"}}"#,
            "/hooks",
        ),
        (
            "a wrong type",
            r#"{"oauth":{"accessToken":"t2","refreshToken":"r2","expiresAt":2000,"account":"acc-1","scopes":"a"}}"#,
            "/oauth/scopes",
        ),
        ("no JSON", "not json", "no JSON"),
        ("no object", "[1]", "no JSON object"),
    ];
    for (what, text, named) in cases {
        let setup = Setup::new();
        let first = doc("t1", 1_000, "acc-1");
        let base = setup.login(&first);
        let refused = setup.session_writes("s", &base, text).unwrap_err();
        assert_eq!(refused.found.code, AGENT_012, "{what}");
        assert!(
            refused.found.why.contains(named),
            "{what}: {}",
            refused.found.why
        );
        assert_eq!(setup.stored().unwrap(), canonical(&first), "{what}");
    }
}

#[test]
fn a_file_of_65_kib_is_refused() {
    let setup = Setup::new();
    let first = doc("t1", 1_000, "acc-1");
    let base = setup.login(&first);
    let copy = setup.session_copy("s");
    std::fs::write(copy.join(FILE), vec![b' '; 65 * 1024]).unwrap();
    let refused =
        take_back(&copy, &setup.request(Origin::Session, Some(&base), &FORMAT)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_012);
    assert!(
        refused.found.why.contains("64 KiB"),
        "{}",
        refused.found.why
    );
    assert_eq!(setup.stored().unwrap(), canonical(&first));
}

/// A link to a file of the host is never followed, even when that file is a
/// valid credential file: only `O_NOFOLLOW` stands in the way.
#[test]
fn a_symbolic_link_to_a_host_file_is_refused() {
    let setup = Setup::new();
    let first = doc("t1", 1_000, "acc-1");
    let base = setup.login(&first);
    let host_file = setup.root.join("host-secret.json");
    std::fs::write(&host_file, doc("host", 9_000, "acc-1")).unwrap();
    let copy = setup.session_copy("s");
    std::os::unix::fs::symlink(&host_file, copy.join(FILE)).unwrap();
    let refused =
        take_back(&copy, &setup.request(Origin::Session, Some(&base), &FORMAT)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_012);
    assert!(
        refused.found.why.contains("symbolic link"),
        "{}",
        refused.found.why
    );
    assert_eq!(setup.stored().unwrap(), canonical(&first));
}

#[test]
fn a_fifo_is_refused_without_blocking() {
    let setup = Setup::new();
    let first = doc("t1", 1_000, "acc-1");
    let base = setup.login(&first);
    let copy = setup.session_copy("s");
    let made = std::process::Command::new("mkfifo")
        .arg(copy.join(FILE))
        .status()
        .unwrap();
    assert!(made.success());
    let refused =
        take_back(&copy, &setup.request(Origin::Session, Some(&base), &FORMAT)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_012);
    assert!(
        refused.found.why.contains("no regular file"),
        "{}",
        refused.found.why
    );
    assert_eq!(setup.stored().unwrap(), canonical(&first));
}

#[test]
fn an_older_token_is_refused() {
    let setup = Setup::new();
    let first = doc("t1", 5_000, "acc-1");
    let base = setup.login(&first);
    let refused = setup
        .session_writes("s", &base, &doc("t0", 4_999, "acc-1"))
        .unwrap_err();
    assert_eq!(refused.found.code, AGENT_016);
    assert!(
        refused.found.why.contains("expires earlier"),
        "{}",
        refused.found.why
    );
    assert_eq!(setup.stored().unwrap(), canonical(&first));
}

#[test]
fn another_account_is_refused() {
    let setup = Setup::new();
    let first = doc("t1", 1_000, "acc-1");
    let base = setup.login(&first);
    let refused = setup
        .session_writes("s", &base, &doc("t2", 2_000, "acc-2"))
        .unwrap_err();
    assert_eq!(refused.found.code, AGENT_016);
    assert!(
        refused.found.why.contains("another account"),
        "{}",
        refused.found.why
    );
    assert_eq!(setup.stored().unwrap(), canonical(&first));
}

/// A login run may change the account: the person logged in on purpose.
#[test]
fn a_login_run_may_change_the_account() {
    let setup = Setup::new();
    setup.login(&doc("t1", 1_000, "acc-1"));
    setup.login(&doc("t2", 500, "acc-2"));
    assert_eq!(setup.stored().unwrap(), canonical(&doc("t2", 500, "acc-2")));
}

/// The guard reads the one file and nothing else of the session's copy:
/// settings with hooks, a second file beside the credential file, a
/// directory and a file in the program's place stay out of the depot.
#[test]
fn nothing_but_the_credential_file_reaches_the_depot() {
    let setup = Setup::new();
    let base = setup.login(&doc("t1", 1_000, "acc-1"));
    let before_config = listing(&setup.depot.config_dir());
    let before_program = listing(&setup.depot.program_dir());
    let copy = setup.session_copy("s");
    std::fs::write(copy.join("settings.json"), r#"{"hooks":{"x":"rm -rf ~"}}"#).unwrap();
    std::fs::write(copy.join("second.json"), doc("t9", 9_000, "acc-1")).unwrap();
    std::fs::create_dir_all(copy.join("program/1.0.0")).unwrap();
    std::fs::write(copy.join("program/1.0.0/standin"), "#!/bin/sh\nevil\n").unwrap();
    std::fs::write(copy.join(FILE), doc("t2", 2_000, "acc-1")).unwrap();
    let taken = take_back(&copy, &setup.request(Origin::Session, Some(&base), &FORMAT)).unwrap();
    assert!(matches!(taken, Taken::Stored { .. }), "{taken:?}");
    assert_eq!(listing(&setup.depot.config_dir()), before_config);
    assert_eq!(listing(&setup.depot.program_dir()), before_program);
    assert_eq!(
        std::fs::read(setup.depot.config_dir().join("settings.json")).unwrap(),
        b"{}\n"
    );
    assert_eq!(
        listing(&setup.depot.credentials_dir()),
        vec![FILE.to_owned()]
    );
    let root: Vec<String> = listing(setup.depot.root())
        .into_iter()
        .filter(|name| {
            ![
                "program",
                "config",
                "credentials",
                "current",
                WRITEBACK_STATE,
            ]
            .contains(&name.as_str())
        })
        .collect();
    assert!(root.is_empty(), "the depot's root holds {root:?}");
}

/// Two sessions started on the same file renew one after the other: the
/// newer token stays, the older never overwrites it.
#[test]
fn of_two_sessions_the_newer_token_wins() {
    let setup = Setup::new();
    let base = setup.login(&doc("t1", 1_000, "acc-1"));
    let newer = doc("t3", 3_000, "acc-1");
    setup.session_writes("b", &base, &newer).unwrap();
    let refused = setup
        .session_writes("a", &base, &doc("t2", 2_000, "acc-1"))
        .unwrap_err();
    assert_eq!(refused.found.code, AGENT_016);
    assert_eq!(setup.stored().unwrap(), canonical(&newer));
}

/// Without an expiry in the format, only a session that started on the
/// stored file writes back.
#[test]
fn without_an_expiry_a_stale_session_does_not_write_back() {
    let setup = Setup::new();
    let copy = setup.session_copy("login");
    std::fs::write(copy.join(FILE), doc("t1", 1, "acc-1")).unwrap();
    let Taken::Stored { new: base, .. } =
        take_back(&copy, &setup.request(Origin::Login, None, &NO_EXPIRY)).unwrap()
    else {
        panic!("no login");
    };
    let write = |name: &str, text: &str| {
        let copy = setup.session_copy(name);
        std::fs::write(copy.join(FILE), text).unwrap();
        take_back(
            &copy,
            &setup.request(Origin::Session, Some(&base), &NO_EXPIRY),
        )
    };
    let newer = doc("t3", 1, "acc-1");
    write("b", &newer).unwrap();
    let refused = write("a", &doc("t2", 1, "acc-1")).unwrap_err();
    assert_eq!(refused.found.code, AGENT_016);
    assert_eq!(setup.stored().unwrap(), canonical(&newer));
}

/// A write cut off in the middle leaves the stored file whole: the new file
/// goes to a temporary place first and only a `rename` puts it in.
#[test]
fn an_interrupted_write_leaves_the_stored_file() {
    let setup = Setup::new();
    let first = doc("t1", 1_000, "acc-1");
    let base = setup.login(&first);
    let copy = setup.session_copy("s");
    std::fs::write(copy.join(FILE), doc("t2", 2_000, "acc-1")).unwrap();
    let request = setup.request(Origin::Session, Some(&base), &FORMAT);
    let refused = take_back_at(&copy, &request, "2026-11-20T00:00:00Z", Some(10)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_012);
    assert_eq!(setup.stored().unwrap(), canonical(&first));
    assert!(
        listing(setup.depot.root())
            .iter()
            .all(|name| !name.ends_with(".writeback")),
        "a temporary file stayed behind"
    );
}

#[test]
fn nothing_changed_is_nothing_to_take() {
    let setup = Setup::new();
    let first = doc("t1", 1_000, "acc-1");
    let base = setup.login(&first);
    let before = setup.depot.last_writeback();
    // The bytes the session got.
    let copy = setup.session_copy("same");
    std::fs::write(copy.join(FILE), setup.stored().unwrap()).unwrap();
    assert_eq!(
        take_back(&copy, &setup.request(Origin::Session, Some(&base), &FORMAT)).unwrap(),
        Taken::Nothing
    );
    // No file at all: a logout in the session keeps the stored login.
    let empty = setup.session_copy("empty");
    assert_eq!(
        take_back(
            &empty,
            &setup.request(Origin::Session, Some(&base), &FORMAT)
        )
        .unwrap(),
        Taken::Nothing
    );
    // Other whitespace, the same value.
    let spaced = setup.session_copy("spaced");
    std::fs::write(spaced.join(FILE), format!("  {first}\n")).unwrap();
    assert_eq!(
        take_back(
            &spaced,
            &setup.request(Origin::Session, Some(&base), &FORMAT)
        )
        .unwrap(),
        Taken::Nothing
    );
    assert_eq!(setup.depot.last_writeback(), before);
    assert_eq!(setup.stored().unwrap(), canonical(&first));
}

#[test]
fn the_fingerprint_is_the_base_of_a_session() {
    let setup = Setup::new();
    let base = setup.login(&doc("t1", 1_000, "acc-1"));
    let copy = setup.session_copy("s");
    std::fs::write(copy.join(FILE), setup.stored().unwrap()).unwrap();
    assert_eq!(fingerprint(&copy, FILE).as_deref(), Some(base.as_str()));
    assert_eq!(fingerprint(&copy, "missing.json"), None);
}

/// The login run writes `.claude.json` with `mcpServers` and `oauthAccount`,
/// a `settings.json` with `hooks` and a file in a subdirectory beside the
/// credential file: the depot gets the credential file and the named field
/// `oauthAccount` in its copy of `.claude.json`, nothing else.
#[test]
fn a_login_takes_the_credential_file_and_the_named_fields_only() {
    let setup = Setup::new();
    let before_config = listing(&setup.depot.config_dir());
    let copy = setup.session_copy("login");
    std::fs::write(copy.join(FILE), doc("t1", 1_000, "acc-1")).unwrap();
    std::fs::write(
        copy.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"a@example.invalid","accountUuid":"u-1"},"mcpServers":{"x":{"command":"/bin/sh"}},"numStartups":3}"#,
    )
    .unwrap();
    std::fs::write(copy.join("settings.json"), r#"{"hooks":{"x":"y"}}"#).unwrap();
    std::fs::create_dir_all(copy.join("config")).unwrap();
    std::fs::write(copy.join("config/extra"), "x").unwrap();

    take_back(&copy, &setup.request(Origin::Login, None, &FORMAT)).unwrap();
    let keys = take_named_fields(&copy, &setup.depot, &NAMED).unwrap();
    assert_eq!(keys, vec!["oauthAccount".to_owned()]);

    assert_eq!(
        listing(&setup.depot.credentials_dir()),
        vec![FILE.to_owned()]
    );
    let mut expected = before_config;
    expected.push(".claude.json".to_owned());
    expected.sort();
    assert_eq!(listing(&setup.depot.config_dir()), expected);
    let account: Value = serde_json::from_slice(
        &std::fs::read(setup.depot.config_dir().join(".claude.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        account,
        serde_json::json!({"oauthAccount":{"emailAddress":"a@example.invalid","accountUuid":"u-1"}})
    );
    let settings = std::fs::read_to_string(setup.depot.config_dir().join("settings.json")).unwrap();
    assert!(!settings.contains("hooks"));
    setup.depot.check().unwrap();
}

/// A named field out of its form is refused, and the depot's file stays.
#[test]
fn a_named_field_out_of_its_form_is_refused() {
    let setup = Setup::new();
    let copy = setup.session_copy("login");
    std::fs::write(
        copy.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"a","accountUuid":"u","hooks":{}}}"#,
    )
    .unwrap();
    let found = take_named_fields(&copy, &setup.depot, &NAMED).unwrap_err();
    assert_eq!(found.code, AGENT_012);
    assert!(found.why.contains("/oauthAccount/hooks"), "{}", found.why);
    assert!(!setup.depot.config_dir().join(".claude.json").exists());
}

/// The named fields replace only their own keys in the depot's file.
#[test]
fn named_fields_keep_the_other_keys_of_the_depot_file() {
    let setup = Setup::new();
    let target = setup.depot.config_dir().join(".claude.json");
    std::fs::write(
        &target,
        r#"{"theme":"dark","oauthAccount":{"emailAddress":"old","accountUuid":"o"}}"#,
    )
    .unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    let copy = setup.session_copy("login");
    std::fs::write(
        copy.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"new","accountUuid":"n"},"theme":"light"}"#,
    )
    .unwrap();
    take_named_fields(&copy, &setup.depot, &NAMED).unwrap();
    let value: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"theme":"dark","oauthAccount":{"emailAddress":"new","accountUuid":"n"}})
    );
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Two sessions started on the same file renew to the same expiry with a
/// rotated token: the one that comes second is older and is refused. A
/// session that renews again on its own stored file still writes back.
#[test]
fn of_two_sessions_with_the_same_expiry_the_first_stays() {
    let setup = Setup::new();
    let base = setup.login(&doc("t1", 1_000, "acc-1"));
    let first = doc("t2", 2_000, "acc-1");
    let Taken::Stored { new: after_b, .. } = setup.session_writes("b", &base, &first).unwrap()
    else {
        panic!("b was not stored");
    };
    let refused = setup
        .session_writes("a", &base, &doc("t3", 2_000, "acc-1"))
        .unwrap_err();
    assert_eq!(refused.found.code, AGENT_016);
    assert!(
        refused.found.why.contains("same time"),
        "{}",
        refused.found.why
    );
    assert_eq!(setup.stored().unwrap(), canonical(&first));
    // Session b renews once more on its own file, same expiry: taken.
    let again = doc("t4", 2_000, "acc-1");
    setup.session_writes("b2", &after_b, &again).unwrap();
    assert_eq!(setup.stored().unwrap(), canonical(&again));
}

/// A session never stores the first credential file: only a login run
/// chooses the account.
#[test]
fn a_session_never_stores_the_first_file() {
    let setup = Setup::new();
    let copy = setup.session_copy("s");
    std::fs::write(copy.join(FILE), doc("t1", 1_000, "attacker")).unwrap();
    let refused = take_back(&copy, &setup.request(Origin::Session, None, &FORMAT)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_012);
    assert!(
        refused.found.why.contains("login run"),
        "{}",
        refused.found.why
    );
    assert!(setup.stored().is_none());
    assert!(!setup.depot.logged_in());
}

/// An expiry more than 400 days ahead is refused, at login and later.
#[test]
fn an_expiry_beyond_the_horizon_is_refused() {
    let setup = Setup::new();
    let far = u64::try_from(
        (std::time::SystemTime::now() + EXPIRY_HORIZON + std::time::Duration::from_secs(86_400))
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let copy = setup.session_copy("login");
    std::fs::write(copy.join(FILE), doc("t1", far, "acc-1")).unwrap();
    let refused = take_back(&copy, &setup.request(Origin::Login, None, &FORMAT)).unwrap_err();
    assert_eq!(refused.found.code, AGENT_012);
    assert!(
        refused.found.why.contains("400 days"),
        "{}",
        refused.found.why
    );
    assert!(setup.stored().is_none());
    // Within the horizon: taken.
    let near = far - 2 * 86_400_000;
    std::fs::write(copy.join(FILE), doc("t1", near, "acc-1")).unwrap();
    take_back(&copy, &setup.request(Origin::Login, None, &FORMAT)).unwrap();
}

/// A key of a session's file is quoted cut and with control and bidi
/// characters escaped.
#[test]
fn a_key_of_the_session_is_quoted_safely() {
    let setup = Setup::new();
    let copy = setup.session_copy("login");
    let key = format!("x\u{202E}y\u{1b}[31m{}", "k".repeat(200));
    let text = format!(
        r#"{{"oauth":{{"accessToken":"t","refreshToken":"r","expiresAt":1,"account":"a"}},"{}":1}}"#,
        key.replace('\u{1b}', "\\u001b")
    );
    std::fs::write(copy.join(FILE), text).unwrap();
    let refused = take_back(&copy, &setup.request(Origin::Login, None, &FORMAT)).unwrap_err();
    let why = &refused.found.why;
    assert!(
        !why.contains('\u{202E}') && !why.contains('\u{1b}'),
        "{why:?}"
    );
    assert!(why.contains("\\u{202e}"), "{why}");
    assert!(!why.contains(&"k".repeat(100)), "{why}");
}

/// A directory that cannot be synced after the `rename` does not turn a
/// replaced file into a refusal: the answer is stored, with a warning.
#[test]
fn a_sync_failure_after_the_rename_is_a_warning() {
    let setup = Setup::new();
    let base = setup.login(&doc("t1", 1_000, "acc-1"));
    let renewed = doc("t2", 2_000, "acc-1");
    crate::agent::depot::adopt::sync_probe::fail(&setup.depot.credentials_dir(), &[0]);
    let taken = setup.session_writes("s", &base, &renewed).unwrap();
    let Taken::Stored { warning, .. } = taken else {
        panic!("not stored: {taken:?}");
    };
    assert!(warning.unwrap().contains("cannot be synced"));
    assert_eq!(setup.stored().unwrap(), canonical(&renewed));
    assert_eq!(
        setup.depot.last_writeback().unwrap().sha256,
        sha256_hex(&canonical(&renewed))
    );
}
