//! Tests of the depot's planning and of the one guard (HUM-518).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use humanitl_config::agents::{AgentSource, LockScope, SourceLock};
use humanitl_config::{Config, Env, Paths, WorkMode};
use humanitl_core::diagnostics::codes::{AGENT_008, AGENT_015, AGENT_017};
use humanitl_core::ids::SessionId;

use super::store::tests::filled;
use super::*;
use crate::launcher::{LaunchOptions, PlanRequest, SandboxBackend};
use crate::profile::{SandboxProfile, SessionContext};

struct Fixture {
    base: tempfile::TempDir,
    paths: Paths,
}

impl Fixture {
    fn new() -> Self {
        let base = tempfile::tempdir().unwrap();
        let paths = Paths::new(Env::from_pairs([
            ("HOME", base.path().join("home").to_str().unwrap()),
            ("XDG_DATA_HOME", base.path().join("data").to_str().unwrap()),
            ("XDG_RUNTIME_DIR", base.path().join("run").to_str().unwrap()),
        ]));
        Self { base, paths }
    }

    /// Claude Code installed in the depot by hand, version 1.0.0.
    fn with_claude(self) -> Self {
        filled(&self.paths.data_dir(), "claude", "claude");
        self
    }

    fn plan(
        &self,
        config: &Config,
        lock: &SourceLock,
        command: &[&str],
        from_caller: bool,
    ) -> Result<DepotPlan, humanitl_core::Diagnostic> {
        let command: Vec<OsString> = command.iter().map(OsString::from).collect();
        plan_session(&DepotInput {
            config,
            lock,
            paths: &self.paths,
            session: SessionId::nil(),
            adapter: "opencode",
            command: &command,
            from_caller,
            purpose: Purpose::Preview,
        })
    }
}

fn depot_for(agent: &str) -> Config {
    let mut config = Config::default();
    config.agents.source = AgentSource::Host;
    match agent {
        "all" => config.agents.source = AgentSource::Depot,
        "claude" => config.agents.claude.source = Some(AgentSource::Depot),
        "opencode" => config.agents.opencode.source = Some(AgentSource::Depot),
        "codex" => config.agents.codex.source = Some(AgentSource::Depot),
        other => panic!("{other}"),
    }
    config
}

fn locked() -> SourceLock {
    SourceLock::none()
        .lock(
            LockScope::All,
            AgentSource::Depot,
            "/etc/humanitl/policy.toml",
        )
        .unwrap()
}

fn strings(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect()
}

/// AC1: without `agents.*` nothing changes, the command included.
#[test]
fn without_a_depot_source_nothing_changes() {
    let fx = Fixture::new();
    let plan = fx
        .plan(
            &Config::default(),
            &SourceLock::none(),
            &["claude", "-p"],
            false,
        )
        .unwrap();
    assert!(plan.session.is_empty());
    assert!(plan.host_bind);
    assert_eq!(strings(&plan.command), ["claude", "-p"]);
}

#[test]
fn a_bare_name_starts_the_depot_program_with_its_arguments() {
    let fx = Fixture::new().with_claude();
    let mut config = depot_for("claude");
    config.agent.command = Some(vec!["claude".into(), "--model".into(), "x".into()]);
    let plan = fx
        .plan(
            &config,
            &SourceLock::none(),
            &["claude", "--model", "x"],
            false,
        )
        .unwrap();
    assert_eq!(
        strings(&plan.command),
        ["/run/humanitl/agent/claude", "--model", "x"]
    );
    assert!(!plan.host_bind, "the configured agent is the depot's");
    let mount = plan.session.mount.expect("mounted");
    // The resolved version, never `current`.
    assert!(mount.program_dir.ends_with("program/1.0.0"), "{mount:?}");
    assert_eq!(mount.version, "1.0.0");
    assert!(
        mount
            .env
            .contains(&("CLAUDE_CONFIG_DIR".to_owned(), DEPOT_CONFIG_DST.to_owned()))
    );
    assert!(
        mount
            .env
            .contains(&("DISABLE_UPDATES".to_owned(), "1".to_owned()))
    );
}

#[test]
fn a_depot_agent_that_is_not_installed_is_agent_008() {
    let fx = Fixture::new();
    let found = fx
        .plan(
            &depot_for("claude"),
            &SourceLock::none(),
            &["claude"],
            false,
        )
        .unwrap_err();
    assert_eq!(found.code, AGENT_008);
}

/// `agents.opencode.source = "depot"` loads, the start says `AGENT_015`.
#[test]
fn an_agent_without_a_recipe_is_agent_015_at_the_start() {
    let fx = Fixture::new();
    let found = fx
        .plan(
            &depot_for("opencode"),
            &SourceLock::none(),
            &["opencode"],
            false,
        )
        .unwrap_err();
    assert_eq!(found.code, AGENT_015);
}

/// A shell of the caller starts beside an unavailable depot agent; the
/// configured agent is then not bound from the host either.
#[test]
fn a_callers_shell_starts_without_the_host_agent() {
    let fx = Fixture::new();
    let plan = fx
        .plan(&depot_for("opencode"), &SourceLock::none(), &["bash"], true)
        .unwrap();
    assert!(!plan.host_bind);
    assert!(plan.session.mount.is_none());
    assert_eq!(plan.warnings.len(), 1, "{:?}", plan.warnings);
}

/// The agent's own command names a host path of the depot agent.
#[test]
fn an_agent_command_with_a_host_path_is_agent_017() {
    let fx = Fixture::new().with_claude();
    let mut config = depot_for("claude");
    config.agent.command = Some(vec!["/home/u/.local/bin/claude".into()]);
    let found = fx
        .plan(
            &config,
            &SourceLock::none(),
            &["/home/u/.local/bin/claude"],
            false,
        )
        .unwrap_err();
    assert_eq!(found.code, AGENT_017);
}

// ---- the guard ----

fn profile() -> SandboxProfile {
    SandboxProfile::parse(
        include_str!("../../../../../../profiles/sandbox/default.toml"),
        Path::new("default.toml"),
    )
    .unwrap()
}

fn session(command: &[&str], depot: DepotSession) -> SessionContext {
    SessionContext {
        session: SessionId::nil(),
        work_src: PathBuf::from("/nonexistent-hum518/work"),
        work_mode: WorkMode::Rw,
        proxy_socket_src: PathBuf::from("/nonexistent-hum518/proxy.sock"),
        ca_cert_src: PathBuf::from("/nonexistent-hum518/ca.crt"),
        ca_bundle_src: PathBuf::from("/nonexistent-hum518/ca-bundle.crt"),
        shim_src: PathBuf::from("/nonexistent-hum518/shim"),
        session_env: Vec::new(),
        command: command.iter().map(OsString::from).collect(),
        files: Vec::new(),
        agent_program: None,
        agent_depot: depot,
    }
}

/// The depot session of a start path for this command.
fn depot_session(
    fx: &Fixture,
    config: &Config,
    lock: &SourceLock,
    command: &[&str],
    from_caller: bool,
) -> (Vec<String>, DepotSession) {
    let plan = fx.plan(config, lock, command, from_caller).unwrap();
    (strings(&plan.command), plan.session)
}

fn guard_of(command: &[String], depot: DepotSession) -> Result<(), humanitl_core::Diagnostic> {
    let refs: Vec<&str> = command.iter().map(String::as_str).collect();
    let session = session(&refs, depot);
    let options = LaunchOptions::default();
    let profile = profile();
    guard::check(&PlanRequest::new(&profile, &session, &options))
}

/// The start paths with `depot`, and again under the lock with
/// `agents.claude.source = "host"` in a layer below it.
#[test]
fn every_named_host_program_is_agent_017() {
    let fx = Fixture::new().with_claude();
    let home_bin = fx.base.path().join("home/bin");
    std::fs::create_dir_all(&home_bin).unwrap();
    let host_claude = fx.base.path().join("host/claude");
    std::fs::create_dir_all(host_claude.parent().unwrap()).unwrap();
    std::fs::write(&host_claude, b"#!/bin/sh\n").unwrap();
    std::os::unix::fs::symlink(&host_claude, home_bin.join("c")).unwrap();
    let link = home_bin.join("c").display().to_string();

    let mut host_below_lock = depot_for("all");
    host_below_lock.agents.claude.source = Some(AgentSource::Host);
    for (config, lock) in [
        (depot_for("all"), SourceLock::none()),
        (host_below_lock, locked()),
    ] {
        for command in [
            vec!["/usr/local/bin/claude"],
            vec![link.as_str()],
            // An agent without a depot recipe (`codex` has one since HUM-515).
            vec!["agy"],
            vec!["/home/u/.local/bin/claude", "-p"],
        ] {
            let (command, depot) = depot_session(&fx, &config, &lock, &command, true);
            let found = guard_of(&command, depot).unwrap_err();
            assert_eq!(found.code, AGENT_017, "{command:?}");
        }
        // The bare name starts the depot program with its arguments.
        let (command, depot) =
            depot_session(&fx, &config, &lock, &["claude", "--model", "x"], true);
        assert_eq!(command[0], "/run/humanitl/agent/claude");
        guard_of(&command, depot).unwrap();
    }
}

/// A host bind of a depot agent's program, the way `bind_agent_program` or
/// `plan_configured_agent` would add it, is refused.
#[test]
fn a_host_bind_of_a_depot_agent_is_agent_017() {
    let fx = Fixture::new().with_claude();
    let (command, depot) = depot_session(
        &fx,
        &depot_for("claude"),
        &SourceLock::none(),
        &["bash"],
        true,
    );
    let refs: Vec<&str> = command.iter().map(String::as_str).collect();
    let mut session = session(&refs, depot);
    session.agent_program = Some(crate::agent::AgentProgram::new(
        PathBuf::from("/home/u/.local/bin/claude"),
        std::ffi::OsStr::new("claude"),
    ));
    let options = LaunchOptions::default();
    let profile = profile();
    let found = guard::check(&PlanRequest::new(&profile, &session, &options)).unwrap_err();
    assert_eq!(found.code, AGENT_017);
}

/// Mutation proof: the guard called in only one backend turns these red.
/// Every backend this build plans with refuses before any other check.
#[test]
fn every_backend_calls_the_guard_first() {
    let fx = Fixture::new();
    let (command, depot) = depot_session(
        &fx,
        &depot_for("claude"),
        &SourceLock::none(),
        &["/usr/local/bin/claude"],
        true,
    );
    let refs: Vec<&str> = command.iter().map(String::as_str).collect();
    let session = session(&refs, depot);
    let options = LaunchOptions::default();
    let profile = profile();
    let request = PlanRequest::new(&profile, &session, &options);
    // Every backend this build lists, by its neutral name, plus Seatbelt,
    // whose plan compiles on every system.
    let mut backends: Vec<(String, Result<crate::LaunchPlan, humanitl_core::Diagnostic>)> =
        crate::backend_list::LISTED_BACKENDS
            .iter()
            .filter_map(|name| {
                let backend = crate::Backend::fallback(
                    name,
                    fx.paths.clone(),
                    &humanitl_config::SandboxRef::default(),
                )
                .ok()?;
                Some(((*name).to_owned(), backend.plan(&request)))
            })
            .collect();
    backends.push((
        "seatbelt".to_owned(),
        crate::seatbelt::SeatbeltBackend::unchecked("/usr/bin/sandbox-exec", fx.paths.clone())
            .plan(&request),
    ));
    assert!(backends.len() >= 2, "{}", backends.len());
    for (name, planned) in backends {
        let found = planned.expect_err(&name);
        assert_eq!(found.code, AGENT_017, "{name}: {}", found.why);
    }
}

// ---- masks and the line ----

fn reserved_at(places: &[&Path]) -> DepotSession {
    DepotSession {
        reserved: vec![ReservedAgent {
            agent: "claude".to_owned(),
            program: "claude".to_owned(),
            host_installs: places.iter().map(|place| place.to_path_buf()).collect(),
            locked_by: None,
        }],
        ..DepotSession::default()
    }
}

/// Mutation proof: without the check for existence the missing place gets a
/// mask, and bwrap would fail with `EROFS` under the read-only `/usr`.
#[test]
fn only_existing_install_locations_are_masked() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("claude");
    std::fs::write(&file, b"#!/bin/sh\n").unwrap();
    let tree = dir.path().join("claude-code");
    std::fs::create_dir(&tree).unwrap();
    let missing = dir.path().join("missing/claude");
    let depot = reserved_at(&[&missing, &tree, &file]);
    let masks = depot.masks();
    let targets: Vec<&Path> = masks.iter().map(|mask| mask.target.as_path()).collect();
    let file = file.canonicalize().unwrap();
    let tree = tree.canonicalize().unwrap();
    assert_eq!(
        targets,
        [file.as_path(), tree.as_path()],
        "files before directories"
    );

    let line: Vec<String> = profile()
        .to_bwrap_args(&session(&["bash"], depot), &crate::LaunchInputs::preview())
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let tree = tree.display().to_string();
    let file = file.display().to_string();
    assert!(
        line.windows(4)
            .any(|w| w == ["--tmpfs", tree.as_str(), "--remount-ro", tree.as_str()]),
        "{line:?}"
    );
    assert!(
        line.windows(3)
            .any(|w| w[0] == "--ro-bind-data" && w[2] == file),
        "{line:?}"
    );
    assert!(
        !line.iter().any(|arg| arg.contains("missing")),
        "a missing place gets no mount: {line:?}"
    );
}

/// The depot's mounts in the line: the resolved version read-only at the
/// agent's place, the scratch writable at the configuration's.
#[test]
fn the_plan_binds_the_resolved_version_read_only_and_the_scratch_writable() {
    let fx = Fixture::new().with_claude();
    let (command, depot) = depot_session(
        &fx,
        &depot_for("claude"),
        &SourceLock::none(),
        &["claude"],
        false,
    );
    let mount = depot.mount.clone().unwrap();
    let refs: Vec<&str> = command.iter().map(String::as_str).collect();
    let line: Vec<String> = profile()
        .to_bwrap_args(
            &session(&refs, depot.clone()),
            &crate::LaunchInputs::preview(),
        )
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let program = mount.program_dir.display().to_string();
    let scratch = mount.scratch.display().to_string();
    assert!(
        line.windows(3)
            .any(|w| w == ["--ro-bind", program.as_str(), "/run/humanitl/agent"]),
        "{line:?}"
    );
    assert!(
        line.windows(3)
            .any(|w| w == ["--bind", scratch.as_str(), DEPOT_CONFIG_DST]),
        "{line:?}"
    );
    assert!(
        !line.iter().any(|arg| arg.ends_with("/current")),
        "{line:?}"
    );
    assert!(!scratch.starts_with(&fx.paths.agents_dir().display().to_string()));
    let env = profile().effective_env(&session(&refs, depot), None);
    let path = env.iter().find(|(key, _)| key == "PATH").unwrap();
    assert!(path.1.starts_with("/run/humanitl/agent:"), "{path:?}");
}

/// Nothing from the depot runs on the host: status, the doctor's reading and
/// the planning read the manifest and never start the program, which would
/// write its marker.
#[test]
fn no_process_starts_from_the_depot_on_the_host() {
    let fx = Fixture::new().with_claude();
    let depot = Depot::of(&fx.paths, "claude");
    let marker = fx.paths.data_dir().join("marker");
    assert!(depot.installed().is_ok());
    let _ = (depot.logged_in(), depot.size());
    fx.plan(
        &depot_for("claude"),
        &SourceLock::none(),
        &["claude"],
        false,
    )
    .unwrap();
    assert!(!marker.exists(), "a depot program ran on the host");
}

/// Rule 5 of the guard: a bind that moves a host install of a depot agent,
/// a place inside one or a place that contains one to another path is
/// `AGENT_017` (`--tests-dir`, a session bind, the project at `/work`); a
/// bind at its own path stays, the mask covers it there. Mutation proof:
/// without `check_binds` this turns red.
#[test]
fn a_bind_that_moves_a_host_install_is_agent_017() {
    let dir = tempfile::tempdir().unwrap();
    let modules = dir.path().join("node_modules");
    let install = modules.join("@anthropic-ai/claude-code");
    std::fs::create_dir_all(install.join("lib")).unwrap();
    let depot = reserved_at(&[&install]);
    let profile = profile();
    let refused = |session: &SessionContext, options: &LaunchOptions| {
        guard::check(&PlanRequest::new(&profile, session, options))
    };
    for src in [install.clone(), modules.clone(), install.join("lib")] {
        let options = LaunchOptions {
            session_ro: vec![(src.clone(), PathBuf::from("/tests/escape"))],
            ..LaunchOptions::default()
        };
        let found = refused(
            &session(&["node", "/tests/escape/cli.js"], depot.clone()),
            &options,
        )
        .unwrap_err();
        assert_eq!(found.code, AGENT_017, "{}", src.display());
    }
    let same_path = LaunchOptions {
        session_ro: vec![(modules.clone(), modules.canonicalize().unwrap())],
        ..LaunchOptions::default()
    };
    refused(&session(&["bash"], depot.clone()), &same_path).unwrap();

    let mut inside_work = session(&["bash"], depot);
    inside_work.work_src = install.join("lib");
    let found = refused(&inside_work, &LaunchOptions::default()).unwrap_err();
    assert_eq!(found.code, AGENT_017);
}

/// Every agent `agents.<id>` knows is reserved under the lock with the host
/// install locations of its entry, recipe or not: a shell cannot reach any
/// of them past the masks.
#[test]
fn under_the_lock_every_agent_is_reserved_with_its_install_locations() {
    let fx = Fixture::new();
    let plan = fx
        .plan(&Config::default(), &locked(), &["bash"], true)
        .unwrap();
    let reserved: Vec<&str> = plan
        .session
        .reserved
        .iter()
        .map(|agent| agent.agent.as_str())
        .collect();
    assert_eq!(reserved, humanitl_config::agents::KNOWN_AGENTS);
    for agent in &plan.session.reserved {
        assert!(!agent.host_installs.is_empty(), "{}", agent.agent);
        assert_eq!(
            agent.locked_by.as_deref(),
            Some("/etc/humanitl/policy.toml")
        );
    }
}

/// A start reserves the depot until its scratch, and with it the sandbox,
/// goes: `remove` meanwhile is `AGENT_018`, and a removal in progress keeps
/// a start out with the same code.
#[test]
fn a_start_and_a_removal_never_overlap() {
    let fx = Fixture::new().with_claude();
    let config = depot_for("claude");
    let command = [OsString::from("claude")];
    let start = || {
        plan_session(&DepotInput {
            config: &config,
            lock: &SourceLock::none(),
            paths: &fx.paths,
            session: SessionId::new(),
            adapter: "opencode",
            command: &command,
            from_caller: false,
            purpose: Purpose::Start,
        })
    };
    let depot = Depot::of(&fx.paths, "claude");
    let started = start().unwrap();
    assert!(depot.in_use());
    let refused = depot.remove().unwrap_err();
    assert_eq!(refused.code.as_str(), "AGENT_018");
    assert!(depot.exists());

    let second = start().unwrap();
    drop(started);
    assert!(depot.remove().is_err(), "the second start still holds it");
    drop(second);
    assert!(!depot.in_use());

    // A removal holds the depot exclusively while it deletes: a start in
    // that moment is refused, never planned against a vanishing tree.
    let removing = depot.reserve(true).unwrap();
    assert_eq!(start().unwrap_err().code.as_str(), "AGENT_018");
    drop(removing);
    depot.remove().unwrap();
    assert!(!depot.exists());
}

/// The check after pinning (HUM-518, review): a source that is a link to a
/// host install of a depot agent at the moment the backend opens it, after
/// the guard saw something else, is `AGENT_017` before anything is mounted.
/// The guard is bypassed here on purpose; the opened descriptor alone
/// decides. Mutation proof: without `check_depot` this turns red.
#[test]
fn a_link_swapped_after_the_guard_is_refused_at_the_pin() {
    let dir = tempfile::tempdir().unwrap();
    let install = dir.path().join("node_modules/@anthropic-ai/claude-code");
    std::fs::create_dir_all(&install).unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let tests_dir = dir.path().join("tests-dir");
    std::os::unix::fs::symlink(&install, &tests_dir).unwrap();
    let mut session = session(&["bash"], reserved_at(&[&install]));
    session.work_src = work;
    let options = LaunchOptions {
        session_ro: vec![(tests_dir, PathBuf::from("/tests/escape"))],
        ..LaunchOptions::default()
    };
    // A profile without system binds: the bundled one binds places that exist
    // only on Linux (`/etc/alternatives`), and the pin would stop there.
    let profile = SandboxProfile::parse("version = 1\nname = \"x\"\n", Path::new("<t>")).unwrap();
    let policy = crate::MountPolicy::new("/nonexistent-hum518-home");
    let Err(found) =
        crate::bwrap::PinnedMounts::open(&policy, &PlanRequest::new(&profile, &session, &options))
    else {
        panic!("the swapped link was pinned");
    };
    assert_eq!(found.code, AGENT_017, "{}", found.why);
}

/// The macOS shape on any system (HUM-518, CI): on the shared loopback the
/// `OpenCode` adapter wraps the agent's own command in `/bin/sh -c <script>
/// opencode <command>`. The guard judges the agent's words at the end, so the
/// depot program behind the launcher starts, while a host program behind it,
/// or a wrapper around other words, stays `AGENT_017`. Mutation proof:
/// judging the whole command again turns the first case red.
#[test]
fn the_launcher_of_an_adapter_does_not_hide_or_block_the_agent_command() {
    use crate::agent::AgentAdapter as _;

    let fx = Fixture::new().with_claude();
    let mut config = depot_for("claude");
    config.agent.command = Some(vec!["claude".into(), "--model".into(), "x".into()]);
    let plan = fx
        .plan(
            &config,
            &SourceLock::none(),
            &["claude", "--model", "x"],
            false,
        )
        .unwrap();
    let wrapped: Vec<String> =
        strings(&crate::agent::OpenCodeAdapter::new().on_shared_loopback(plan.command.clone()));
    assert_eq!(wrapped[0], "/bin/sh", "the macOS shape");
    guard_of(&wrapped, plan.session.clone()).unwrap();

    let other: Vec<String> = strings(
        &crate::agent::OpenCodeAdapter::new()
            .on_shared_loopback(vec![OsString::from("/usr/local/bin/claude")]),
    );
    let found = guard_of(&other, plan.session).unwrap_err();
    assert_eq!(found.code, AGENT_017);
}

/// Claude Code's key launcher (HUM-514) in front of the depot program passes
/// the guard; in front of a host program it does not.
#[test]
fn the_key_launcher_of_claude_code_keeps_the_guard() {
    use crate::agent::AgentAdapter as _;

    let fx = Fixture::new().with_claude();
    let mut config = depot_for("claude");
    "claude".clone_into(&mut config.agent.adapter);
    config.agent.command = Some(vec!["claude".into()]);
    let plan = fx
        .plan(&config, &SourceLock::none(), &["claude"], false)
        .unwrap();
    assert!(plan.session.agent_command, "the agent's own command");
    let ctx = crate::agent::AgentContext::new(
        SessionId::nil(),
        PathBuf::from("/work"),
        humanitl_config::LlmConfig::default(),
    )
    .with_llm_key(crate::agent::AgentKey::Withheld);
    let launcher = crate::agent::ClaudeCodeAdapter::new().launcher(&ctx);
    assert!(!launcher.is_empty());
    let wrapped: Vec<String> = strings(&[launcher.clone(), plan.command.clone()].concat());
    assert_eq!(wrapped[0], "/bin/sh");
    guard_of(&wrapped, plan.session.clone()).unwrap();

    let other: Vec<String> =
        strings(&[launcher, vec![OsString::from("/usr/local/bin/claude")]].concat());
    let found = guard_of(&other, plan.session).unwrap_err();
    assert_eq!(found.code, AGENT_017);
}

// ---- the maintenance run (HUM-563) ----

/// A maintenance session whose work place is `home`.
fn maintenance_session(home: &Path) -> SessionContext {
    let mut session = session(&["/bin/sh", "-c", "true"], DepotSession::default());
    session.work_src = home.to_path_buf();
    session.agent_depot.maintenance_home = Some(home.to_path_buf());
    session
}

fn maintenance_profile() -> SandboxProfile {
    let mut profile = profile();
    profile.mounts.work.dst = PathBuf::from(maintenance::MAINTENANCE_HOME_DST);
    profile
}

#[test]
fn a_maintenance_run_may_work_in_its_own_directory_and_nowhere_else() {
    let base = tempfile::tempdir().unwrap();
    let paths = Paths::new(Env::from_pairs([
        ("HOME", base.path().join("home").to_str().unwrap()),
        ("XDG_DATA_HOME", base.path().join("data").to_str().unwrap()),
        ("XDG_RUNTIME_DIR", base.path().join("run").to_str().unwrap()),
    ]));
    let policy = crate::profile::MountPolicy::from_paths(&paths);
    let home = paths.data_dir().join("agents/.maintenance/claude-run");
    let elsewhere = paths.data_dir().join("ca");
    for dir in [&home, &elsewhere] {
        std::fs::create_dir_all(dir).unwrap();
    }
    // Its own work directory, marked by the daemon: admitted.
    guard::work_place(&policy, &maintenance_session(&home), &home).unwrap();
    // The same directory without the mark is a project under the data
    // directory: the policy refuses it.
    let unmarked = session(&["/bin/sh"], DepotSession::default());
    assert!(guard::work_place(&policy, &unmarked, &home).is_err());
    // Marked, but another place of the data directory: refused.
    let found =
        guard::work_place(&policy, &maintenance_session(&elsewhere), &elsewhere).unwrap_err();
    assert_eq!(found.code, humanitl_core::diagnostics::codes::SANDBOX_006);
    // Marked, and the opened place is not the marked one: refused.
    assert!(guard::work_place(&policy, &maintenance_session(&home), &elsewhere).is_err());
}

#[test]
fn a_maintenance_run_carries_nothing_else_of_the_host() {
    let home = Path::new("/nonexistent-hum563/agents/.maintenance/claude-run");
    let options = LaunchOptions::default();
    let check = |profile: &SandboxProfile, session: &SessionContext, options: &LaunchOptions| {
        guard::check(&PlanRequest::new(profile, session, options))
    };
    let session = maintenance_session(home);
    check(&maintenance_profile(), &session, &options).unwrap();
    // At /work instead of its own place.
    assert!(check(&profile(), &session, &options).is_err());
    // An extension of the profile.
    let mut extended = maintenance_profile();
    extended.mounts.extra_ro.push(PathBuf::from("/srv"));
    assert!(check(&extended, &session, &options).is_err());
    // A session bind.
    let mut bound = LaunchOptions::default();
    bound
        .session_ro
        .push((PathBuf::from("/srv"), PathBuf::from("/tests")));
    assert!(check(&maintenance_profile(), &session, &bound).is_err());
    // Another work source than the marked home.
    let mut moved = maintenance_session(home);
    moved.work_src = PathBuf::from("/nonexistent-hum563/elsewhere");
    assert!(check(&maintenance_profile(), &moved, &options).is_err());
}

/// A login or configuration run (HUM-520) carries the depot mount of its own
/// agent and starts exactly that program; an install run carries none.
#[test]
fn a_login_run_carries_only_its_own_depot_program() {
    let home = Path::new("/nonexistent-hum520/agents/.maintenance/claude-run");
    let options = LaunchOptions::default();
    let check = |session: &SessionContext| {
        guard::check(&PlanRequest::new(&maintenance_profile(), session, &options))
    };
    let mount = DepotMount {
        agent: "claude".to_owned(),
        version: "1.0.0".to_owned(),
        program_dir: PathBuf::from("/nonexistent-hum520/agents/claude/program/1.0.0"),
        program: "claude".to_owned(),
        scratch: PathBuf::from("/nonexistent-hum520/scratch"),
        env: Vec::new(),
    };
    let run = |agent: Option<&str>, mount: Option<DepotMount>, command: &[&str]| {
        let mut session = maintenance_session(home);
        session.command = command.iter().map(OsString::from).collect();
        session.agent_depot.maintenance_program = agent.map(ToOwned::to_owned);
        session.agent_depot.mount = mount;
        session
    };
    let program = "/run/humanitl/agent/claude";
    check(&run(
        Some("claude"),
        Some(mount.clone()),
        &[program, "auth", "login"],
    ))
    .unwrap();
    // An install run with a mount.
    assert!(check(&run(None, Some(mount.clone()), &[program])).is_err());
    // The mount of another agent.
    assert!(check(&run(Some("codex"), Some(mount.clone()), &[program])).is_err());
    // Another command beside the mount.
    assert!(
        check(&run(
            Some("claude"),
            Some(mount),
            &["/bin/sh", "-c", program]
        ))
        .is_err()
    );
    // A named agent without a mount.
    assert!(check(&run(Some("claude"), None, &[program])).is_err());
}
