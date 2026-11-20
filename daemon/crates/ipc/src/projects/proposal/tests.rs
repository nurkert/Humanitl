//! Tests of `Projects.proposal` (HUM-536).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::SystemTime;

use humanitl_config::agents::AgentSource;
use humanitl_config::{Env, Paths, PolicyLocks};
use humanitl_core::diagnostics::codes::SANDBOX_045;
use humanitl_core::{Diagnostic, SessionId, Severity};
use humanitl_recorder::{ProjectRow, ProjectStart, Recorder, RecorderSettings, SessionMeta};

use super::{
    AGENT, ASK_MODE, BackendFacts, FOLDER_MODE, ISOLATION, Inputs, KERNEL, LLM_SERVER, PROFILE,
    Ports, SHARED_KERNEL, ServerFacts, build,
};
use crate::projects::{Context, ProjectIdentity, answer};
use crate::v1;

struct Fixture {
    home: tempfile::TempDir,
    recorder: Recorder,
    session: SessionId,
}

impl Fixture {
    fn new(config_toml: &str) -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config/humanitl");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("config.toml"), config_toml).unwrap();
        std::fs::create_dir_all(home.path().join("run")).unwrap();
        let recorder = Recorder::open(
            &home.path().join("data/humanitl/humanitl.db"),
            &home.path().join("data/humanitl/blobs"),
            RecorderSettings::default(),
        )
        .unwrap();
        let session = SessionId::new();
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: String::new(),
            agent: "opencode".to_owned(),
        });
        Self {
            home,
            recorder,
            session,
        }
    }

    fn paths(&self) -> Paths {
        let home = self.home.path();
        Paths::new(Env::from_pairs([
            ("HOME", home.display().to_string()),
            ("XDG_CONFIG_HOME", home.join("config").display().to_string()),
            ("XDG_DATA_HOME", home.join("data").display().to_string()),
            ("XDG_RUNTIME_DIR", home.join("run").display().to_string()),
        ]))
    }

    /// A daemon without sandbox service on a system where nothing is missing
    /// but what `missing` names.
    fn context(
        &self,
        locks: PolicyLocks,
        missing: impl Fn(&'static str) -> Option<Diagnostic> + Send + Sync + 'static,
    ) -> Context {
        Context {
            recorder: Some(self.recorder.clone()),
            paths: self.paths(),
            locks,
            activity: None,
            sandbox: None,
            audit: None,
            proposal: Ports {
                backends: BackendFacts::Given(Arc::new(missing)),
                source_lock: humanitl_config::agents::SourceLock::none(),
            },
        }
    }

    fn folder(&self, name: &str) -> std::path::PathBuf {
        let dir = self.home.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn known(&self, name: &str, choice: Option<&str>) -> ProjectRow {
        let dir = self.folder(name);
        let identity = ProjectIdentity::of(&dir).unwrap();
        let row = self
            .recorder
            .register_project_start(ProjectStart {
                session: self.session,
                path: identity.path_text(),
                dev: identity.dev,
                ino: identity.ino,
                agent: "opencode".to_owned(),
                at: SystemTime::now(),
            })
            .await
            .unwrap()
            .project;
        match choice {
            Some(text) => self
                .recorder
                .save_project_choice(row.id, Some(text.to_owned()))
                .await
                .unwrap()
                .unwrap(),
            None => row,
        }
    }
}

fn default_profile() -> super::ProfileFacts {
    super::ProfileFacts {
        name: "default".to_owned(),
        state: Ok(()),
    }
}

/// A sandbox service whose resolver reads the fixture's files, as the
/// daemon's does.
fn live_service(fixture: &Fixture, paths: Paths) -> crate::SandboxService {
    let base = humanitl_config::resolve(
        &humanitl_config::ProfileSelection::default(),
        None,
        paths.env(),
        &[],
    )
    .unwrap();
    crate::SandboxService::new(
        crate::session::SessionResolver::new(paths, base),
        fixture.session,
        crate::sandbox::SandboxPorts::none().with_recorder(fixture.recorder.clone()),
    )
}

fn nothing_missing(_: &'static str) -> Option<Diagnostic> {
    None
}

/// The kernel level without `/dev/kvm`, as the backend search reports it.
fn no_kvm(name: &'static str) -> Option<Diagnostic> {
    (name == humanitl_sandbox::VM_BACKEND).then(|| {
        Diagnostic::builder(SANDBOX_045, Severity::Blocking)
            .why("/dev/kvm is missing".to_owned())
            .build()
    })
}

fn ask(path: &std::path::Path, values: &[(&str, &str)]) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::Proposal(
            v1::projects_request::Proposal {
                path: path.display().to_string(),
                project_id: String::new(),
                values: values
                    .iter()
                    .map(|(key, value)| v1::ChoiceValue {
                        key: (*key).to_owned(),
                        value: (*value).to_owned(),
                    })
                    .collect(),
                cli_overrides: Vec::new(),
            },
        )),
    }
}

/// [`ask`] with these session values (HUM-567).
fn ask_with(path: &std::path::Path, overrides: &[(&str, &str)]) -> v1::ProjectsRequest {
    let mut request = ask(path, &[]);
    if let Some(v1::projects_request::Op::Proposal(proposal)) = request.op.as_mut() {
        proposal.cli_overrides = overrides
            .iter()
            .map(|(path, value)| v1::sandbox_request::CliOverride {
                path: (*path).to_owned(),
                value: (*value).to_owned(),
            })
            .collect();
    }
    request
}

async fn propose(ctx: Context, request: v1::ProjectsRequest) -> v1::StartProposal {
    answer(ctx, request)
        .await
        .unwrap()
        .proposal
        .expect("a proposal")
}

fn field<'a>(proposal: &'a v1::StartProposal, id: &str) -> &'a v1::ProposalField {
    proposal
        .fields
        .iter()
        .find(|field| field.id == id)
        .unwrap_or_else(|| panic!("no field {id}: {proposal:?}"))
}

fn option<'a>(field: &'a v1::ProposalField, value: &str) -> &'a v1::ProposalOption {
    field
        .options
        .iter()
        .find(|option| option.value == value)
        .unwrap_or_else(|| panic!("no option {value} in {field:?}"))
}

fn codes(reasons: &[v1::Diagnostic]) -> Vec<&str> {
    reasons.iter().map(|reason| reason.code.as_str()).collect()
}

/// Without `/dev/kvm` the kernel level is not available, with
/// `SANDBOX_045`, and the default is the shared kernel.
#[tokio::test]
async fn without_kvm_the_kernel_level_is_sandbox_045_and_the_default_is_shared() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let proposal = propose(fixture.context(PolicyLocks::none(), no_kvm), ask(&dir, &[])).await;

    let isolation = field(&proposal, ISOLATION);
    assert_eq!(isolation.value, SHARED_KERNEL, "{isolation:?}");
    let kernel = option(isolation, KERNEL);
    assert!(!kernel.available, "{kernel:?}");
    assert_eq!(codes(&kernel.reasons), ["SANDBOX_045"], "{kernel:?}");
    let shared = option(isolation, SHARED_KERNEL);
    assert!(shared.available, "{shared:?}");
    assert!(proposal.defaults_line.starts_with("Defaults: "));
    assert!(
        proposal.defaults_line.contains("shared kernel"),
        "{}",
        proposal.defaults_line
    );
}

/// `sandbox.min_isolation = "kernel"`: the shared kernel is not
/// choosable, and the reason says why (`SANDBOX_035`).
///
/// Mutation proof: measuring the backend list in `BackendFacts::list`
/// against a `sandbox` group whose `min_isolation` is reset to the default
/// (floor ignored) makes the shared kernel available and this test red.
#[tokio::test]
async fn a_kernel_floor_makes_the_shared_kernel_unavailable_with_a_reason() {
    let fixture = Fixture::new("[sandbox]\nmin_isolation = \"kernel\"\n");
    let dir = fixture.folder("repo");
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[]),
    )
    .await;

    let isolation = field(&proposal, ISOLATION);
    let shared = option(isolation, SHARED_KERNEL);
    assert!(!shared.available, "{shared:?}");
    assert!(
        codes(&shared.reasons).contains(&"SANDBOX_035"),
        "{shared:?}"
    );
    // The start without a screen would stop here; the proposal says so.
    assert!(
        codes(&proposal.diagnostics).contains(&"SANDBOX_035"),
        "{:?}",
        proposal.diagnostics
    );
}

/// An agent installed only in the depot, with the default configuration
/// (source `host`), through the real per-agent path: the default source
/// stays `host` as the start would take it (ADR-0023, no fallback), not
/// available, with `AGENT_001`; the depot is named available beside it.
#[test]
fn an_agent_only_in_the_depot_keeps_the_host_source_unavailable_and_names_the_depot() {
    use std::os::unix::fs::PermissionsExt as _;

    let fixture = Fixture::new("");
    let paths = fixture.paths();
    // An installed depot of claude, the one agent with a recipe today.
    let depot = paths.agents_dir().join("claude");
    let version = depot.join("program/2.1.0");
    for dir in [
        paths.agents_dir(),
        depot.clone(),
        depot.join("program"),
        version.clone(),
        depot.join("credentials"),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::fs::write(
        version.join("manifest.json"),
        r#"{"version":"2.1.0","signature":"verified"}"#,
    )
    .unwrap();
    std::fs::set_permissions(
        version.join("manifest.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::os::unix::fs::symlink("program/2.1.0", depot.join("current")).unwrap();

    let resolved = humanitl_config::load(&humanitl_config::Sources::empty()).unwrap();
    let facts = super::agent_facts(
        &paths,
        &resolved.config,
        &humanitl_config::agents::SourceLock::none(),
        "claude",
        "claude",
    );
    assert_eq!(facts.source, AgentSource::Host);
    assert!(facts.depot.is_ok(), "{facts:?}");
    let built = build(&Inputs {
        resolved: &resolved,
        locks: &PolicyLocks::none(),
        plan: super::assumed_plan(&resolved.config),
        backends: &[],
        agents: vec![facts],
        servers: Vec::new(),
        profiles: vec![default_profile()],
        cloud: None,
    });
    let agent = built.fields.iter().find(|field| field.id == AGENT).unwrap();
    let claude = option(agent, "claude");
    assert_eq!(claude.source, "host");
    assert!(!claude.available, "{claude:?}");
    assert_eq!(codes(&claude.reasons), ["AGENT_001"]);
    let host = claude
        .sources
        .iter()
        .find(|source| source.source == "host")
        .unwrap();
    assert!(!host.available && host.selected, "{host:?}");
    let depot = claude
        .sources
        .iter()
        .find(|source| source.source == "depot")
        .unwrap();
    assert!(depot.available && !depot.selected, "{depot:?}");
}

/// The host search finds a program in the user's program folders and names
/// a missing one with `AGENT_001`.
#[test]
fn the_host_search_finds_a_program_or_names_agent_001() {
    let fixture = Fixture::new("");
    let paths = fixture.paths();
    let config = humanitl_config::Config::default();
    let lock = humanitl_config::agents::SourceLock::none();
    let before = super::agents(&paths, &config, &lock);
    let pi = before.iter().find(|agent| agent.id == "pi").unwrap();
    assert_eq!(
        pi.host.as_ref().unwrap_err().code.as_str(),
        "AGENT_001",
        "{pi:?}"
    );
    assert_eq!(pi.source, AgentSource::Host);
    // Without a depot recipe, the depot is no source for pi.
    assert_eq!(pi.depot.as_ref().unwrap_err().code.as_str(), "AGENT_015");

    let bin = fixture.folder(".local/bin");
    crate::test_support::write_executable(&bin.join("pi"), "#!/bin/sh\n");
    let after = super::agents(&paths, &config, &lock);
    let pi = after.iter().find(|agent| agent.id == "pi").unwrap();
    assert!(pi.host.is_ok(), "{pi:?}");
}

/// Two saved servers, one in the cloud and one on this computer.
const TWO_SERVERS: &str = r#"
[llm]
active = "cloud"

[llm.servers.cloud]
endpoint = "https://api.example.com/v1"
models = ["big"]

[llm.servers.local]
endpoint = "http://127.0.0.1:11434/v1"
models = ["small"]
"#;

/// A cloud agent carries the notice of HUM-516; a local server does not.
#[tokio::test]
async fn a_cloud_agent_carries_the_notice_and_a_local_server_does_not() {
    let fixture = Fixture::new(TWO_SERVERS);
    let dir = fixture.folder("repo");
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[]),
    )
    .await;

    let agent = field(&proposal, AGENT);
    let opencode = option(agent, "opencode");
    let notice = opencode
        .cloud_inference
        .as_ref()
        .expect("opencode sends the inference to api.example.com");
    assert_eq!(notice.provider, "api.example.com");
    assert!(proposal.cloud_inference.is_some());

    let servers = field(&proposal, LLM_SERVER);
    assert_eq!(servers.value, "cloud");
    assert!(option(servers, "cloud").cloud_inference.is_some());
    assert!(
        option(servers, "local").cloud_inference.is_none(),
        "{servers:?}"
    );

    // Choosing the local server: no notice for the start any more.
    let local = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[("llm.server", "local")]),
    )
    .await;
    assert_eq!(field(&local, LLM_SERVER).value, "local");
    assert!(local.cloud_inference.is_none(), "{local:?}");
}

/// A known project gets its choice as the default, and a partial choice
/// computes a new proposal without saving anything.
#[tokio::test]
async fn a_known_project_defaults_to_its_choice_and_a_partial_choice_saves_nothing() {
    let fixture = Fixture::new("");
    let row = fixture
        .known("repo", Some(r#"{"hold.ask_mode":"none"}"#))
        .await;
    let dir = std::path::PathBuf::from(&row.path);
    let ctx = || fixture.context(PolicyLocks::none(), nothing_missing);

    let stored = propose(ctx(), ask(&dir, &[])).await;
    assert_eq!(stored.project_id, row.id.to_string());
    assert!(stored.has_choice);
    let asked = field(&stored, ASK_MODE);
    assert_eq!(asked.value, "none");
    assert_eq!(asked.origin, "project-choice");

    let kernel = propose(
        ctx(),
        ask(
            &dir,
            &[
                ("sandbox.backend", humanitl_sandbox::VM_BACKEND),
                ("hold.ask_mode", ""),
            ],
        ),
    )
    .await;
    assert_eq!(field(&kernel, ISOLATION).value, KERNEL);
    assert_eq!(
        field(&kernel, ASK_MODE).value,
        "ui",
        "the empty value drops the key"
    );
    assert!(
        kernel
            .choice
            .iter()
            .any(|pair| pair.key == "sandbox.backend"
                && pair.value == humanitl_sandbox::VM_BACKEND),
        "{:?}",
        kernel.choice
    );

    let after = fixture.recorder.get_project(row.id).await.unwrap().unwrap();
    assert_eq!(after.choice, row.choice, "nothing was saved");
    assert_eq!(after.version, row.version);
}

/// A partial choice that may not be saved is refused like `save_choice`
/// refuses it (`CONFIG_024`).
#[tokio::test]
async fn a_partial_choice_below_the_floor_is_config_024() {
    let fixture = Fixture::new("[sandbox]\nmin_isolation = \"namespaces\"\n");
    let dir = fixture.folder("repo");
    let refused = answer(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[("sandbox.min_isolation", "policy")]),
    )
    .await
    .unwrap_err();
    assert_eq!(refused.code.as_str(), "CONFIG_024", "{}", refused.why);
}

/// A lock of the company policy shows in the field, makes every other
/// option unavailable and stays out of the choice the client saves.
#[tokio::test]
async fn a_policy_lock_shows_and_stays_out_of_the_choice() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let locks = PolicyLocks::none().with("hold.ask_mode", "terminal", "/etc/humanitl/policy.toml");
    let proposal = propose(fixture.context(locks, nothing_missing), ask(&dir, &[])).await;

    let asked = field(&proposal, ASK_MODE);
    assert_eq!(asked.value, "terminal");
    let lock = asked.lock.as_ref().expect("the field names its lock");
    assert!(lock.read_only);
    assert_eq!(lock.locked_by, "/etc/humanitl/policy.toml");
    assert!(option(asked, "terminal").available);
    assert!(option(asked, "terminal").choice.is_empty());
    assert!(!option(asked, "ui").available);
    assert!(
        !proposal
            .choice
            .iter()
            .any(|pair| pair.key == "hold.ask_mode"),
        "{:?}",
        proposal.choice
    );
}

/// Every field of the screen is there, in order, and the choice of the
/// defaults is one `save_choice` accepts.
#[tokio::test]
async fn the_fields_come_in_screen_order_and_their_choice_saves() {
    let fixture = Fixture::new("");
    let row = fixture.known("repo", None).await;
    let dir = std::path::PathBuf::from(&row.path);
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[]),
    )
    .await;
    let ids: Vec<&str> = proposal
        .fields
        .iter()
        .map(|field| field.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [AGENT, FOLDER_MODE, ISOLATION, ASK_MODE, LLM_SERVER, PROFILE]
    );
    for field in &proposal.fields {
        assert!(
            field.value.is_empty()
                || field
                    .options
                    .iter()
                    .any(|option| option.value == field.value),
            "the default is one of the options: {field:?}"
        );
    }
    let saved = answer(
        fixture.context(PolicyLocks::none(), nothing_missing),
        v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    project_id: row.id.to_string(),
                    values: proposal.choice.clone(),
                },
            )),
        },
    )
    .await;
    assert!(saved.is_ok(), "{saved:?}");
}

/// Servers of a fixed set, for the builder alone.
#[test]
fn a_server_name_that_is_not_saved_stays_the_default_and_says_why() {
    let mut resolved = humanitl_config::load(&humanitl_config::Sources::empty()).unwrap();
    resolved.config.llm.choice = Some(humanitl_config::servers::ServerChoice {
        name: "gone".to_owned(),
        source: humanitl_config::servers::ServerSource::Default,
        known: false,
        saved: vec!["home".to_owned()],
    });
    let built = build(&Inputs {
        resolved: &resolved,
        locks: &PolicyLocks::none(),
        plan: super::assumed_plan(&resolved.config),
        backends: &[],
        agents: Vec::new(),
        servers: vec![ServerFacts {
            name: "home".to_owned(),
            cloud: None,
            refusal: None,
        }],
        profiles: vec![default_profile()],
        cloud: None,
    });
    let servers = built
        .fields
        .iter()
        .find(|field| field.id == LLM_SERVER)
        .unwrap();
    assert_eq!(servers.value, "gone");
    let gone = option(servers, "gone");
    assert!(!gone.available, "{gone:?}");
    assert_eq!(codes(&gone.reasons), ["CONFIG_021"]);
}

/// The defaults are exactly the start that would happen without a screen:
/// compared with the snapshot a `Sandbox(Plan)` for the same folder yields
/// without a project choice, through the same resolution (config file,
/// project folder) and the same sandbox service.
#[tokio::test]
async fn the_defaults_are_what_a_plan_without_a_choice_yields() {
    use tokio_stream::StreamExt as _;

    let fixture = Fixture::new(
        r#"
[hold]
ask_mode = "none"

[sandbox]
mode = "direct"
work_mode = "ro"

[llm]
active = "local"

[llm.servers.local]
endpoint = "http://127.0.0.1:11434/v1"
models = ["small"]
"#,
    );
    let dir = fixture.folder("repo");
    let paths = fixture.paths();
    let base = humanitl_config::resolve(
        &humanitl_config::ProfileSelection::default(),
        None,
        paths.env(),
        &[],
    )
    .unwrap();
    let service = crate::SandboxService::new(
        crate::session::SessionResolver::new(paths, base),
        fixture.session,
        crate::sandbox::SandboxPorts::none().with_recorder(fixture.recorder.clone()),
    );
    let events: Vec<v1::SandboxEvent> = service
        .stream(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Plan(v1::sandbox_request::Plan {
                work_dir: dir.display().to_string(),
                ..v1::sandbox_request::Plan::default()
            })),
        })
        .collect()
        .await;
    let status = events
        .into_iter()
        .find_map(|event| match event.event {
            Some(v1::sandbox_event::Event::Status(status)) => Some(status),
            _ => None,
        })
        .expect("a Plan answers with a status");

    let ctx = Context {
        sandbox: Some(service),
        ..fixture.context(PolicyLocks::none(), nothing_missing)
    };
    let proposal = propose(ctx, ask(&dir, &[])).await;

    let folder = match (status.sandbox_mode.as_str(), status.work_mode.as_str()) {
        ("disposable", _) => super::DISPOSABLE,
        (_, "ro") => super::READ_ONLY,
        _ => super::READ_WRITE,
    };
    assert_eq!(field(&proposal, FOLDER_MODE).value, folder, "{status:?}");
    assert_eq!(folder, super::READ_ONLY, "the configured mode: {status:?}");
    let level = if status.isolation_level == v1::IsolationLevel::Kernel as i32 {
        KERNEL
    } else {
        SHARED_KERNEL
    };
    assert_eq!(field(&proposal, ISOLATION).value, level);
    assert_eq!(field(&proposal, PROFILE).value, status.profile);
    assert_eq!(field(&proposal, ASK_MODE).value, status.ask_mode);
    assert_eq!(status.ask_mode, "none");
    assert_eq!(
        field(&proposal, LLM_SERVER).value,
        status
            .llm_server
            .map(|server| server.name)
            .unwrap_or_default()
    );
    assert_eq!(field(&proposal, LLM_SERVER).value, "local");
}

/// A project profile that picks another saved server than the default,
/// not confirmed for this folder: the start refuses with `CONFIG_022`, so
/// the default server option is not available and says so.
///
/// Mutation proof: without the `assign::unconfirmed` check in `servers`
/// the option is available and this test is red.
#[tokio::test]
async fn an_unconfirmed_server_of_the_project_profile_is_config_022() {
    let fixture = Fixture::new(TWO_SERVERS);
    let dir = fixture.folder("repo");
    let profile = fixture.paths().project_profile_path(&dir);
    std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
    std::fs::write(&profile, "[config.llm]\nserver = \"local\"\n").unwrap();
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[]),
    )
    .await;
    let servers = field(&proposal, LLM_SERVER);
    assert_eq!(servers.value, "local", "{servers:?}");
    let local = option(servers, "local");
    assert!(!local.available, "{local:?}");
    assert_eq!(codes(&local.reasons), ["CONFIG_022"]);
    assert!(codes(&proposal.diagnostics).contains(&"CONFIG_022"));
}

/// A policy lock `sandbox.mode = "auto"`: the folder option the start
/// resolves auto to stays available, and the locked key is in no pair.
///
/// Mutation proof: comparing the options against the locked text `auto`
/// (no `folder_locks`) makes every folder option unavailable and this test
/// red.
#[tokio::test]
async fn a_lock_on_auto_keeps_the_resolved_folder_mode_available() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let locks = PolicyLocks::none().with("sandbox.mode", "auto", "/etc/humanitl/policy.toml");
    let proposal = propose(fixture.context(locks, nothing_missing), ask(&dir, &[])).await;
    let folder = field(&proposal, FOLDER_MODE);
    let chosen = option(folder, &folder.value);
    assert!(chosen.available, "{folder:?}");
    assert!(
        !chosen.choice.iter().any(|pair| pair.key == "sandbox.mode"),
        "{chosen:?}"
    );
    assert!(
        !proposal
            .choice
            .iter()
            .any(|pair| pair.key == "sandbox.mode"),
        "{:?}",
        proposal.choice
    );
}

/// A sandbox profile that does not load is offered with its finding and is
/// not available.
///
/// Mutation proof: `sandbox_profiles` without the load (every state
/// `Ok`) offers the broken profile as available and this test is red.
#[tokio::test]
async fn a_broken_sandbox_profile_is_not_available() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let paths = fixture.paths();
    let own = paths.profiles_dir().join("sandbox");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::write(own.join("broken.toml"), "this is = not [ toml").unwrap();
    let ctx = Context {
        sandbox: Some(live_service(&fixture, paths)),
        ..fixture.context(PolicyLocks::none(), nothing_missing)
    };
    let proposal = propose(ctx, ask(&dir, &[])).await;
    let profiles = field(&proposal, PROFILE);
    let broken = option(profiles, "broken");
    assert!(!broken.available, "{broken:?}");
    assert!(!broken.reasons.is_empty(), "{broken:?}");
}

/// A stored key no field of the screen writes (`sandbox.min_isolation`)
/// stays in the choice of the defaults, so saving them unchanged keeps it.
///
/// Mutation proof: without `keep_unedited` the floor drops out of the
/// choice and this test is red.
///
/// Linux only: the floor `namespaces` is above the macOS backend, so there
/// the stored choice is refused (`CONFIG_024`) before the proposal is built.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_stored_floor_stays_in_the_choice_of_the_defaults() {
    let fixture = Fixture::new("");
    let row = fixture
        .known("repo", Some(r#"{"sandbox.min_isolation":"namespaces"}"#))
        .await;
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(std::path::Path::new(&row.path), &[]),
    )
    .await;
    assert!(
        proposal
            .choice
            .iter()
            .any(|pair| pair.key == "sandbox.min_isolation" && pair.value == "namespaces"),
        "{:?}",
        proposal.choice
    );
}

/// A backend search that takes 10 s does not hold up the proposal: it
/// answers within the caller's deadline from the memory of the search, and
/// the level still being checked is not available (`DOCTOR_012`).
///
/// Mutation proof: `proposal_plan` searching afresh (`BackendPick::Detect`)
/// waits for the slow program and this test is red.
///
/// Linux only: the macOS backend is found by an absolute path, not through
/// `PATH`, so the slow stand-ins never take its place there.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn a_slow_backend_search_does_not_hold_up_the_proposal() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let bin = fixture.folder("slow-bin");
    // Every listed backend's program name sleeps; the search of the
    // platform backend is the one the default start waits for.
    for name in humanitl_sandbox::LISTED_BACKENDS {
        crate::test_support::write_executable(&bin.join(name), "#!/bin/sh\nsleep 10\n");
    }
    let home = fixture.home.path();
    let paths = Paths::new(Env::from_pairs([
        ("HOME", home.display().to_string()),
        ("XDG_CONFIG_HOME", home.join("config").display().to_string()),
        ("XDG_DATA_HOME", home.join("data").display().to_string()),
        ("XDG_RUNTIME_DIR", home.join("run").display().to_string()),
        ("PATH", bin.display().to_string()),
    ]));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let ctx = Context {
        paths: paths.clone(),
        sandbox: Some(live_service(&fixture, paths)),
        proposal: Ports {
            backends: BackendFacts::Searches {
                searches: humanitl_sandbox::BackendSearches::new(),
                deadline: Some(deadline),
            },
            source_lock: humanitl_config::agents::SourceLock::none(),
        },
        ..fixture.context(PolicyLocks::none(), nothing_missing)
    };
    let started = std::time::Instant::now();
    let proposal = propose(ctx, ask(&dir, &[])).await;
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    let shared = option(field(&proposal, ISOLATION), SHARED_KERNEL);
    assert!(!shared.available, "{shared:?}");
    assert!(codes(&shared.reasons).contains(&"DOCTOR_012"), "{shared:?}");
}

/// A policy lock `sandbox.mode = "auto"` on a writable folder with a
/// disposable layer: each folder option is measured by what auto resolves to
/// with that option's write mode. `ro` makes auto direct read-only, so it
/// stays available; `rw` would resolve to disposable, so it is not.
///
/// Mutation proof: measuring every option against the mode of the default
/// (one `folder_locks` for all) makes `ro` unavailable and this test red.
#[tokio::test]
async fn a_lock_on_auto_measures_each_folder_option_with_its_own_write_mode() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let locks = PolicyLocks::none().with("sandbox.mode", "auto", "/etc/humanitl/policy.toml");
    let proposal = propose(fixture.context(locks, nothing_missing), ask(&dir, &[])).await;
    let folder = field(&proposal, FOLDER_MODE);
    assert_eq!(folder.value, super::DISPOSABLE, "{folder:?}");
    assert!(option(folder, super::DISPOSABLE).available, "{folder:?}");
    assert!(option(folder, super::READ_ONLY).available, "{folder:?}");
    assert!(!option(folder, super::READ_WRITE).available, "{folder:?}");
    assert!(
        !option(folder, super::READ_ONLY)
            .choice
            .iter()
            .any(|pair| pair.key == "sandbox.mode"),
        "{folder:?}"
    );
}

/// `agent.command` replaces the command line of every adapter at the
/// start, so another adapter starts that command too: it is checked
/// against it, not against its default program.
///
/// Mutation proof: applying `agent.command` only to the configured adapter
/// marks pi available and this test red.
#[test]
fn a_configured_agent_command_applies_to_every_adapter() {
    let fixture = Fixture::new("");
    let paths = fixture.paths();
    let bin = fixture.folder(".local/bin");
    crate::test_support::write_executable(&bin.join("pi"), "#!/bin/sh\n");
    let mut config = humanitl_config::Config::default();
    config.agent.adapter = "opencode".to_owned();
    config.agent.command = Some(vec!["/missing".to_owned()]);
    let facts = super::agents(
        &paths,
        &config,
        &humanitl_config::agents::SourceLock::none(),
    );
    let pi = facts.iter().find(|agent| agent.id == "pi").unwrap();
    let missing = pi.host.as_ref().unwrap_err();
    assert_eq!(missing.code.as_str(), "AGENT_001");
    assert!(missing.why.contains("/missing"), "{missing:?}");
}

/// The session floor of a start reaches the proposal (HUM-567): with
/// `sandbox.min_isolation = kernel` among the session values the shared
/// kernel is not choosable and the proposal names `SANDBOX_035` as the start
/// would; without them the same folder proposes the shared kernel.
///
/// Mutation proof: `resolve_with_session` returning the resolution without
/// the values (or `answer` dropping `cli_overrides`) makes the shared kernel
/// available and this test red.
#[tokio::test]
async fn the_session_floor_reaches_the_proposal_like_the_start() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let ctx = || fixture.context(PolicyLocks::none(), nothing_missing);

    let without = propose(ctx(), ask(&dir, &[])).await;
    assert!(option(field(&without, ISOLATION), SHARED_KERNEL).available);

    let with = propose(
        ctx(),
        ask_with(&dir, &[("sandbox.min_isolation", "kernel")]),
    )
    .await;
    let shared = option(field(&with, ISOLATION), SHARED_KERNEL);
    assert!(!shared.available, "{shared:?}");
    assert!(
        codes(&with.diagnostics).contains(&"SANDBOX_035"),
        "{:?}",
        with.diagnostics
    );
}

/// The backend among the session values lies above the saved project
/// choice, as at the start (HUM-567): a choice of the kernel level with a
/// session backend `auto` proposes the shared kernel, the choice alone the
/// kernel level. Nothing is saved.
///
/// Mutation proof: the session values mixed below the choice (or not at
/// all) keep the kernel level and turn the second assertion red.
#[tokio::test]
async fn the_session_backend_lies_above_the_saved_choice() {
    let fixture = Fixture::new("");
    let choice = format!(
        r#"{{"sandbox.backend":"{}"}}"#,
        humanitl_sandbox::VM_BACKEND
    );
    let row = fixture.known("repo", Some(&choice)).await;
    let dir = std::path::PathBuf::from(&row.path);
    let ctx = || fixture.context(PolicyLocks::none(), nothing_missing);

    let saved = propose(ctx(), ask(&dir, &[])).await;
    assert_eq!(field(&saved, ISOLATION).value, KERNEL);
    let session = propose(
        ctx(),
        ask_with(&dir, &[("sandbox.backend", humanitl_sandbox::AUTO_BACKEND)]),
    )
    .await;
    assert_eq!(field(&session, ISOLATION).value, SHARED_KERNEL);
    // The values to save stay those without the session (HUM-567): the
    // backend of the saved choice, not the one of this call.
    //
    // Mutation proof: `propose` without the second pass for the choice saves
    // the shared backend, and this assertion turns red.
    assert_eq!(session.choice, saved.choice);
    assert!(
        session
            .choice
            .iter()
            .any(|value| value.key == "sandbox.backend"
                && value.value == humanitl_sandbox::VM_BACKEND),
        "{:?}",
        session.choice
    );
    let after = fixture.recorder.get_project(row.id).await.unwrap().unwrap();
    assert_eq!(after.choice, row.choice, "nothing was saved");
}

/// A path a session may not set is `CONFIG_003`, as the start refuses it
/// (HUM-567, the same allow list).
///
/// Mutation proof: `session_values` without `check_override_key` mixes
/// `sandbox.profile` in and answers with a proposal; this test turns red.
#[tokio::test]
async fn a_session_value_the_start_refuses_is_config_003() {
    let fixture = Fixture::new("");
    let dir = fixture.folder("repo");
    let refused = answer(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask_with(&dir, &[("sandbox.profile", "default")]),
    )
    .await
    .unwrap_err();
    assert_eq!(refused.code.as_str(), "CONFIG_003", "{}", refused.why);
}
