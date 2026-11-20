//! Tests of the login and configuration runs with a stand-in agent whose
//! credential file is JSON (HUM-520). The run itself is played by writing
//! into its configuration copy, as the agent in the sandbox would.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use humanitl_core::diagnostics::codes::{AGENT_008, AGENT_012, AGENT_014, AGENT_019};

use super::*;
use crate::agent::depot::InstallHost;
use crate::agent::depot::credential::{
    CredentialField, CredentialFormat, Expiry as Expires, NamedFields, TimeUnit, ValueForm,
};
use crate::agent::depot::recipe::LoginCallback;
use crate::agent::depot::store::tests::filled;

const fn field(pointer: &'static str, form: ValueForm, required: bool) -> CredentialField {
    CredentialField {
        pointer,
        form,
        required,
    }
}

const FORMAT: CredentialFormat = CredentialFormat {
    fields: &[
        field("/token", ValueForm::Token, true),
        field("/expires", ValueForm::Count, true),
        field("/account", ValueForm::Text, true),
    ],
    expiry: Some(Expires {
        pointer: "/expires",
        unit: TimeUnit::Seconds,
    }),
    account: Some("/account"),
    one_of: &[],
    requires: &[],
    fixed: &[],
    evidence: "stand-in of the tests",
};

const LOGIN: LoginRecipe = LoginRecipe {
    args: &["auth", "login"],
    hosts: &[InstallHost {
        host: "login.example.invalid",
        evidence: "https://example.invalid/ (stand-in)",
    }],
    callback: Some(LoginCallback {
        port: 1455,
        path: "/auth/callback",
    }),
    named: Some(NamedFields {
        file: ".claude.json",
        fields: &[
            field("/oauthAccount", ValueForm::Object, false),
            field("/oauthAccount/emailAddress", ValueForm::Text, true),
        ],
        evidence: "stand-in of the tests",
    }),
    device_flow_setting: Some("Enable device code login"),
};

const RECIPE: DepotRecipe = DepotRecipe {
    program: "standin",
    config_env: Some("STANDIN_CONFIG_DIR"),
    update_off: &[],
    credential_file: Some(".credentials.json"),
    install: None,
    login: LoginFlow::DeviceCode,
    credential_format: Some(FORMAT),
    login_run: Some(LOGIN),
    command_keys: &["hooks", "mcpServers"],
};

static STANDIN: KnownAgent = KnownAgent {
    id: "standin",
    display_name: "Stand-in",
    program: "standin",
    host_installs: &["/usr/local/bin/standin"],
    recipe: Some(RECIPE),
};

static NO_FORMAT: KnownAgent = KnownAgent {
    recipe: Some(DepotRecipe {
        credential_format: None,
        ..RECIPE
    }),
    ..STANDIN
};

static KEY_ONLY: KnownAgent = KnownAgent {
    recipe: Some(DepotRecipe {
        login: LoginFlow::ApiKeyOnly,
        ..RECIPE
    }),
    ..STANDIN
};

struct Fixture {
    _base: tempfile::TempDir,
    paths: Paths,
    depot: Depot,
}

impl Fixture {
    fn new() -> Self {
        let base = tempfile::tempdir().unwrap();
        // Aufgelöst: Unter macOS liegt das Temp-Verzeichnis hinter dem Link
        // `/var` -> `/private/var`, und das Depot nennt seine Wurzel aufgelöst.
        let dir = &base.path().canonicalize().unwrap();
        let paths = Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.join("home").to_str().unwrap()),
            ("XDG_RUNTIME_DIR", dir.join("run").to_str().unwrap()),
            ("XDG_DATA_HOME", dir.join("data").to_str().unwrap()),
        ]));
        let filled = filled(&paths.data_dir(), "standin", "standin");
        let depot = Depot::of(&paths, "standin");
        assert_eq!(filled.root(), depot.root());
        Self {
            _base: base,
            paths,
            depot,
        }
    }

    fn run(&self, agent: &'static KnownAgent, task: MaintenanceTask) -> AgentRun {
        prepare(&self.paths, agent, task, SessionId::new(), None).unwrap()
    }

    /// The depots sealed (HUM-523), with keys behind a stand-in keyring in
    /// `dir`; the stored login, if any, sealed with them.
    fn sealed(&self, dir: &Path) -> crate::agent::depot::seal::CredentialKeys {
        use crate::agent::depot::seal;
        let tool = seal::tests::fake_keyring(dir);
        let keys = seal::CredentialKeys::new(
            humanitl_config::secret::KeyStore::for_paths(&self.paths).with_secret_tool(Some(tool)),
        );
        seal::switch(&self.paths, &keys, true).unwrap();
        let stored = self.depot.credentials_dir().join(".credentials.json");
        if let Ok(plain) = std::fs::read(&stored) {
            let sealed =
                seal::seal(&keys.key().unwrap(), "standin", ".credentials.json", &plain).unwrap();
            std::fs::write(&stored, sealed).unwrap();
        }
        keys
    }
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

#[test]
fn a_login_runs_the_depot_program_with_the_login_hosts_alone() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let command: Vec<String> = run
        .command()
        .iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    assert_eq!(command, ["/run/humanitl/agent/standin", "auth", "login"]);
    let mount = run.mount();
    assert_eq!(mount.program_dir, fixture.depot.program_dir().join("1.0.0"));
    assert!(
        !mount
            .scratch
            .as_ref()
            .unwrap()
            .starts_with(fixture.depot.root())
    );
    assert!(
        mount
            .env
            .contains(&("STANDIN_CONFIG_DIR".to_owned(), DEPOT_CONFIG_DST.to_owned()))
    );
    let rules = run.session_rules();
    assert_eq!(rules.len(), 1);
    assert!(matches!(rules[0].expires, Expiry::Session(_)));
    // A configuration run carries no rule of its own.
    drop(run);
    let config = fixture.run(&STANDIN, MaintenanceTask::Config);
    assert!(config.session_rules().is_empty());
    assert_eq!(config.command().len(), 1);
}

/// The login run writes, beside the credential file, a `.claude.json` with
/// `mcpServers` and `oauthAccount`, a `settings.json` with `hooks` and a file
/// under `config/`: the depot holds afterwards the credential file and the
/// named field in its copy of `.claude.json`, nothing else.
#[test]
fn a_login_takes_over_the_credential_file_and_the_named_field_only() {
    let fixture = Fixture::new();
    let before = listing(&fixture.depot.config_dir());
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let copy = run.mount().scratch.unwrap();
    std::fs::write(
        copy.join(".credentials.json"),
        r#"{"token":"t1","expires":100,"account":"acc"}"#,
    )
    .unwrap();
    std::fs::write(
        copy.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"a@example.invalid"},"mcpServers":{"x":{"command":"/bin/sh"}}}"#,
    )
    .unwrap();
    std::fs::write(
        copy.join("settings.json"),
        r#"{"hooks":{"Stop":"curl evil"}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(copy.join("config")).unwrap();
    std::fs::write(copy.join("config/x"), "x").unwrap();

    let done = run.commit_login(true, true).unwrap();
    assert!(
        matches!(done.taken, Taken::Stored { .. }),
        "{:?}",
        done.taken
    );
    assert_eq!(done.named, vec!["oauthAccount".to_owned()]);
    assert!(!copy.exists(), "the copy of the run goes with it");

    assert_eq!(
        listing(&fixture.depot.credentials_dir()),
        [".credentials.json"]
    );
    let mut expected = before;
    expected.push(".claude.json".to_owned());
    expected.sort();
    assert_eq!(listing(&fixture.depot.config_dir()), expected);
    let account = std::fs::read_to_string(fixture.depot.config_dir().join(".claude.json")).unwrap();
    assert!(account.contains("oauthAccount"), "{account}");
    assert!(!account.contains("mcpServers"), "{account}");
    let settings =
        std::fs::read_to_string(fixture.depot.config_dir().join("settings.json")).unwrap();
    assert!(!settings.contains("hooks"), "{settings}");
    assert!(fixture.depot.logged_in());
}

#[test]
fn a_login_without_a_credential_file_or_with_a_refused_one_fails() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let failed = run.commit_login(true, true).unwrap_err();
    assert_eq!(failed.found.code, AGENT_014);
    assert!(
        failed.found.why.contains("no credential file"),
        "{}",
        failed.found.why
    );

    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    std::fs::write(
        run.mount().scratch.unwrap().join(".credentials.json"),
        r#"{"token":"t1","expires":100,"account":"acc","hooks":{}}"#,
    )
    .unwrap();
    let failed = run.commit_login(true, true).unwrap_err();
    assert_eq!(failed.found.code, AGENT_014);
    assert_eq!(failed.refused.unwrap().found.code, AGENT_012);
    assert!(!fixture.depot.logged_in());
}

/// On a backend that shares the host's loopback (Seatbelt) a failed login of
/// an agent with a callback names the vendor's device flow setting.
#[test]
fn a_failed_login_without_an_own_loopback_names_the_device_flow_setting() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let failed = run.commit_login(false, false).unwrap_err();
    assert_eq!(failed.found.code, AGENT_014);
    assert!(
        failed.found.why.contains("Enable device code login"),
        "{}",
        failed.found.why
    );
    assert!(failed.found.fix.is_some());
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let failed = run.commit_login(false, true).unwrap_err();
    assert!(
        !failed.found.why.contains("device code"),
        "{}",
        failed.found.why
    );
}

#[test]
fn a_login_the_adapter_cannot_store_is_refused_before_the_run() {
    let fixture = Fixture::new();
    for agent in [&NO_FORMAT, &KEY_ONLY] {
        let found = prepare(
            &fixture.paths,
            agent,
            MaintenanceTask::Login,
            SessionId::new(),
            None,
        )
        .unwrap_err();
        assert_eq!(found.code, AGENT_014, "{}", found.why);
    }
    // The real recipes declare no format yet: nothing is guessed.
    for agent in crate::agent::depot::KNOWN {
        if agent
            .recipe
            .is_some_and(|recipe| recipe.credential_format.is_none())
        {
            let found = prepare(
                &fixture.paths,
                agent,
                MaintenanceTask::Login,
                SessionId::new(),
                None,
            )
            .unwrap_err();
            assert!(
                matches!(found.code, c if c == AGENT_014 || c == AGENT_008),
                "{}",
                found.why
            );
        }
    }
}

#[test]
fn a_login_needs_an_installed_agent() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.depot.root().join("current")).unwrap();
    let found = prepare(
        &fixture.paths,
        &STANDIN,
        MaintenanceTask::Login,
        SessionId::new(),
        None,
    )
    .unwrap_err();
    assert_eq!(found.code, AGENT_008);
}

/// Without a confirmation nothing changes; the list highlights a new
/// `hooks` entry; the confirmation takes it over, a discard drops it.
#[test]
fn a_configuration_run_changes_nothing_until_it_is_confirmed() {
    let fixture = Fixture::new();
    let settings = fixture.depot.config_dir().join("settings.json");
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    let copy = run.mount().scratch.unwrap();
    std::fs::write(
        copy.join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"command":"curl evil"}]},"theme":"dark"}"#,
    )
    .unwrap();
    std::fs::write(copy.join("CLAUDE.md"), "be nice").unwrap();
    let proposal = run.finish_config(true).unwrap();
    assert_eq!(
        std::fs::read(&settings).unwrap(),
        b"{}\n",
        "nothing changed yet"
    );
    let changed: Vec<(&str, Change)> = proposal
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change.change))
        .collect();
    assert_eq!(
        changed,
        [
            ("CLAUDE.md", Change::Added),
            ("settings.json", Change::Changed)
        ]
    );
    let settings_change = &proposal.changes[1];
    assert_eq!(settings_change.keys, ["hooks", "theme"]);
    assert_eq!(settings_change.omitted, 0);
    assert_eq!(settings_change.command_keys, ["hooks"]);
    assert_eq!(pending(&fixture.paths, &STANDIN).unwrap(), proposal.changes);

    discard_config(&fixture.paths, "standin");
    assert!(pending(&fixture.paths, &STANDIN).is_none());
    assert_eq!(std::fs::read(&settings).unwrap(), b"{}\n");
    let found = accept_config(&fixture.paths, "standin").unwrap_err();
    assert_eq!(found.code, AGENT_019);

    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    std::fs::write(
        run.mount().scratch.unwrap().join("settings.json"),
        r#"{"hooks":{}}"#,
    )
    .unwrap();
    run.finish_config(true).unwrap();
    let accepted = accept_config(&fixture.paths, "standin").unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(std::fs::read(&settings).unwrap(), br#"{"hooks":{}}"#);
    fixture.depot.check().unwrap();
    assert!(pending(&fixture.paths, &STANDIN).is_none());
}

/// The credential file never travels with a configuration: its renewal
/// goes through the guard, and an altered one is refused there.
#[test]
fn a_configuration_run_never_carries_the_credential_file() {
    let fixture = Fixture::new();
    let stored = fixture.depot.credentials_dir().join(".credentials.json");
    std::fs::write(&stored, r#"{"token":"t1","expires":100,"account":"acc"}"#).unwrap();
    std::fs::set_permissions(&stored, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    std::fs::write(
        run.mount().scratch.unwrap().join(".credentials.json"),
        r#"{"token":"t1","expires":100,"account":"acc","hooks":{}}"#,
    )
    .unwrap();
    let proposal = run.finish_config(true).unwrap();
    assert!(proposal.changes.is_empty(), "{:?}", proposal.changes);
    assert!(matches!(proposal.credential, Some(Err(_))));
    assert!(pending(&fixture.paths, &STANDIN).is_none());
    assert_eq!(
        std::fs::read_to_string(&stored).unwrap(),
        r#"{"token":"t1","expires":100,"account":"acc"}"#
    );
}

#[test]
fn a_failed_configuration_run_keeps_nothing() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    std::fs::write(
        run.mount().scratch.unwrap().join("settings.json"),
        r#"{"hooks":{}}"#,
    )
    .unwrap();
    let found = run.finish_config(false).unwrap_err();
    assert_eq!(found.code, AGENT_019);
    assert!(pending(&fixture.paths, &STANDIN).is_none());
}

#[test]
fn the_shown_configuration_lists_its_command_keys() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.depot.config_dir().join("settings.json"),
        r#"{"mcpServers":{"a":{}},"theme":"x"}"#,
    )
    .unwrap();
    let shown = show_config(&fixture.paths, &STANDIN);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].command_keys, ["mcpServers"]);
}

/// A link in the run's copy is never followed into the host: the whole
/// configuration run is refused, and nothing is laid aside.
#[test]
fn a_link_in_a_configuration_run_is_refused() {
    let fixture = Fixture::new();
    let host_file = fixture.paths.data_dir().join("host-secret");
    std::fs::write(&host_file, "secret").unwrap();
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    std::os::unix::fs::symlink(&host_file, run.mount().scratch.unwrap().join("notes.md")).unwrap();
    let found = run.finish_config(true).unwrap_err();
    assert_eq!(found.code, AGENT_019);
    assert!(found.why.contains("notes.md"), "{}", found.why);
    assert!(pending(&fixture.paths, &STANDIN).is_none());
    assert!(!pending_dir(&fixture.paths, "standin").exists());
}

/// A valid credential file beside a named field out of its form: nothing is
/// stored, neither the credential file nor the field. Everything is checked
/// before anything is written.
#[test]
fn a_bad_named_field_stores_no_credential_either() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let copy = run.mount().scratch.unwrap();
    std::fs::write(
        copy.join(".credentials.json"),
        r#"{"token":"t1","expires":100,"account":"acc"}"#,
    )
    .unwrap();
    std::fs::write(
        copy.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"a@example.invalid","hooks":{}}}"#,
    )
    .unwrap();
    let failed = run.commit_login(true, true).unwrap_err();
    assert_eq!(failed.found.code, AGENT_014);
    assert!(failed.stored.is_none());
    assert!(!fixture.depot.logged_in());
    assert!(!fixture.depot.config_dir().join(".claude.json").exists());
}

/// The cap of 64 KiB holds for the named subtree, not for the file: a large
/// `.claude.json` with a small `oauthAccount` is taken, a large
/// `oauthAccount` or a file above 4 MiB is refused, and then nothing is
/// stored at all.
#[test]
fn the_cap_of_the_named_fields_holds_for_their_subtree() {
    let credential = r#"{"token":"t1","expires":100,"account":"acc"}"#;
    let cases = [
        (
            "a large file, a small field",
            format!(
                r#"{{"oauthAccount":{{"emailAddress":"a@example.invalid"}},"history":"{}"}}"#,
                "x".repeat(1024 * 1024)
            ),
            true,
        ),
        (
            "a large field",
            format!(
                r#"{{"oauthAccount":{{"emailAddress":"a@example.invalid","x":"{}"}}}}"#,
                "y".repeat(70 * 1024)
            ),
            false,
        ),
        (
            "a file above 4 MiB",
            format!(
                r#"{{"oauthAccount":{{"emailAddress":"a@example.invalid"}},"history":"{}"}}"#,
                "x".repeat(5 * 1024 * 1024)
            ),
            false,
        ),
    ];
    for (what, file, taken) in cases {
        let fixture = Fixture::new();
        let run = fixture.run(&STANDIN, MaintenanceTask::Login);
        let copy = run.mount().scratch.unwrap();
        std::fs::write(copy.join(".credentials.json"), credential).unwrap();
        std::fs::write(copy.join(".claude.json"), file).unwrap();
        let result = run.commit_login(true, true);
        assert_eq!(result.is_ok(), taken, "{what}: {result:?}");
        assert_eq!(fixture.depot.logged_in(), taken, "{what}");
        let account = fixture.depot.config_dir().join(".claude.json");
        assert_eq!(account.exists(), taken, "{what}");
        if taken {
            let text = std::fs::read_to_string(account).unwrap();
            assert!(!text.contains("history"), "{what}");
        }
    }
}

/// Sixty changed keys before `hooks`: `hooks` is still found, listed first
/// and marked; only the other keys are cut, and the list says how many.
#[test]
fn a_command_key_behind_many_keys_is_never_cut() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    let mut settings = serde_json::Map::new();
    for index in 0..60 {
        settings.insert(format!("a{index:02}"), serde_json::json!(index));
    }
    settings.insert("hooks".to_owned(), serde_json::json!({"Stop": "curl evil"}));
    settings.insert(
        "projects".to_owned(),
        serde_json::json!({"p": {"mcpServers": {"x": {"command": "sh"}}}}),
    );
    std::fs::write(
        run.mount().scratch.unwrap().join("settings.json"),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    let proposal = run.finish_config(true).unwrap();
    let change = &proposal.changes[0];
    assert_eq!(change.command_keys, ["hooks", "projects.p.mcpServers"]);
    assert_eq!(&change.keys[..2], ["hooks", "projects.p.mcpServers"]);
    assert_eq!(change.keys.len(), 2 + 50);
    assert_eq!(change.omitted, 10);
}

/// A file of the depot, `0600`.
fn private_file(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
}

/// The named fields cannot be written after the credential file was stored:
/// the login is all or nothing, the earlier login (or none) is put back, and
/// both steps are reported for the audit log.
#[test]
fn a_failed_named_write_puts_the_earlier_login_back() {
    let earlier = br#"{"token":"t0","expires":50,"account":"acc"}"#;
    for had in [true, false] {
        let fixture = Fixture::new();
        let stored = fixture.depot.credentials_dir().join(".credentials.json");
        if had {
            private_file(&stored, earlier);
        }
        let run = fixture.run(&STANDIN, MaintenanceTask::Login);
        let copy = run.mount().scratch.unwrap();
        std::fs::write(
            copy.join(".credentials.json"),
            r#"{"token":"t1","expires":100,"account":"acc"}"#,
        )
        .unwrap();
        std::fs::write(
            copy.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"a@example.invalid"}}"#,
        )
        .unwrap();
        // The temporary file of the named fields cannot be created.
        std::fs::create_dir(fixture.depot.root().join("..claude.json.named")).unwrap();
        let failed = run.commit_login(true, true).unwrap_err();
        assert_eq!(failed.found.code, AGENT_014, "had {had}");
        assert!(
            failed.found.why.contains("put back"),
            "{}",
            failed.found.why
        );
        assert!(
            matches!(failed.stored, Some(Taken::Stored { .. })),
            "had {had}"
        );
        let Some(Taken::Stored { new, .. }) = failed.restored else {
            panic!("no restore reported, had {had}");
        };
        if had {
            assert_eq!(std::fs::read(&stored).unwrap(), earlier);
            assert_eq!(new, credential::sha256_hex(earlier));
        } else {
            assert!(!stored.exists());
            assert!(new.is_empty());
        }
        assert!(!fixture.depot.config_dir().join(".claude.json").exists());
    }
}

/// An object that replaces `null`, and objects inside an array: a key that
/// runs commands below them is found and marked.
#[test]
fn a_command_key_below_a_replaced_scalar_or_in_an_array_is_found() {
    let fixture = Fixture::new();
    private_file(
        &fixture.depot.config_dir().join("settings.json"),
        br#"{"projects":null,"list":[1]}"#,
    );
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    std::fs::write(
        run.mount().scratch.unwrap().join("settings.json"),
        r#"{"projects":{"p":{"mcpServers":{"x":{"command":"sh"}}}},"list":[{"hooks":{}}]}"#,
    )
    .unwrap();
    let proposal = run.finish_config(true).unwrap();
    let change = &proposal.changes[0];
    assert_eq!(
        change.command_keys,
        ["list[0].hooks", "projects.p.mcpServers"]
    );
}

/// A merge that would grow the depot's file beyond 4 MiB is refused before
/// anything is written: the next login could not read its own file.
#[test]
fn a_merge_beyond_the_file_cap_writes_nothing() {
    let fixture = Fixture::new();
    let target = fixture.depot.config_dir().join(".claude.json");
    let cap = usize::try_from(credential::NAMED_FILE_CAP).unwrap();
    let filler = "x".repeat(cap - 100 - r#"{"history":""}"#.len());
    let depot_file = format!(r#"{{"history":"{filler}"}}"#);
    private_file(&target, depot_file.as_bytes());
    let run = fixture.run(&STANDIN, MaintenanceTask::Login);
    let copy = run.mount().scratch.unwrap();
    std::fs::write(
        copy.join(".credentials.json"),
        r#"{"token":"t1","expires":100,"account":"acc"}"#,
    )
    .unwrap();
    std::fs::write(
        copy.join(".claude.json"),
        format!(
            r#"{{"oauthAccount":{{"emailAddress":"{}"}}}}"#,
            "a".repeat(200)
        ),
    )
    .unwrap();
    let failed = run.commit_login(true, true).unwrap_err();
    assert!(failed.found.why.contains("4 MiB"), "{}", failed.found.why);
    assert!(!fixture.depot.logged_in());
    assert_eq!(std::fs::read(&target).unwrap(), depot_file.as_bytes());
}

/// HUM-521 (review): a restart in the middle of the copy leaves only the
/// staging place, never a partial proposal; the next run clears it.
#[test]
fn an_interrupted_copy_leaves_no_pending_proposal() {
    let fixture = Fixture::new();
    let run = fixture.run(&STANDIN, MaintenanceTask::Config);
    std::fs::write(
        run.mount().scratch.unwrap().join("settings.json"),
        r#"{"hooks":{}}"#,
    )
    .unwrap();
    // Der Daemon endet nach der Kopie und vor dem Umbenennen.
    let staged = run.stage_config().unwrap();
    assert!(staged.join("settings.json").exists());
    assert!(pending(&fixture.paths, &STANDIN).is_none());
    assert!(!pending_dir(&fixture.paths, "standin").exists());
    drop(run);
    assert!(staging_dir(&fixture.paths, "standin").exists());
    // Übernehmen findet nichts, die Konfiguration bleibt.
    let found = accept_config(&fixture.paths, "standin").unwrap_err();
    assert_eq!(found.code, AGENT_019);
    assert_eq!(
        std::fs::read(fixture.depot.config_dir().join("settings.json")).unwrap(),
        b"{}\n"
    );
    let next = fixture.run(&STANDIN, MaintenanceTask::Config);
    assert!(
        !staging_dir(&fixture.paths, "standin").exists(),
        "stale staging is cleared"
    );
    drop(next);
}

/// HUM-523: a login in a sealed depot stores its credential file sealed; a
/// named write that fails afterwards puts the earlier sealed file back byte
/// for byte, never its plain text.
#[test]
fn a_login_in_a_sealed_depot_stores_sealed_and_restores_byte_for_byte() {
    use crate::agent::depot::seal;
    let earlier = br#"{"token":"t0","expires":50,"account":"acc"}"#;
    for fail in [false, true] {
        let fixture = Fixture::new();
        let stored = fixture.depot.credentials_dir().join(".credentials.json");
        private_file(&stored, earlier);
        let keyring = tempfile::tempdir().unwrap();
        let keys = fixture.sealed(keyring.path());
        let sealed_before = std::fs::read(&stored).unwrap();
        assert!(seal::is_sealed(&sealed_before));
        let run = prepare(
            &fixture.paths,
            &STANDIN,
            MaintenanceTask::Login,
            SessionId::new(),
            Some(keys.clone()),
        )
        .unwrap();
        let copy = run
            .mount()
            .scratch
            .expect("a login run has a configuration copy");
        assert_eq!(
            std::fs::read(copy.join(".credentials.json")).unwrap(),
            earlier
        );
        std::fs::write(
            copy.join(".credentials.json"),
            r#"{"token":"t1","expires":100,"account":"acc"}"#,
        )
        .unwrap();
        std::fs::write(
            copy.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"a@example.invalid"}}"#,
        )
        .unwrap();
        if fail {
            std::fs::create_dir(fixture.depot.root().join("..claude.json.named")).unwrap();
            run.commit_login(true, true).unwrap_err();
            assert_eq!(std::fs::read(&stored).unwrap(), sealed_before);
        } else {
            run.commit_login(true, true).unwrap();
            let now = std::fs::read(&stored).unwrap();
            assert!(seal::is_sealed(&now), "the login lies in plain text");
            let opened =
                seal::from_disk(&fixture.depot, ".credentials.json", &now, Some(&keys)).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&opened).unwrap();
            assert_eq!(value["token"], "t1");
        }
    }
}
