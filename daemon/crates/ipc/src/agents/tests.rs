//! Tests of the `Agents` RPC and of the lock on every way of writing the
//! source (HUM-518).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use humanitl_config::agents::{AgentSource, LockScope, SourceLock};

use super::*;

fn paths(dir: &Path) -> Paths {
    Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", dir.display().to_string()),
        ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
        ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ("XDG_RUNTIME_DIR", dir.join("run").display().to_string()),
    ]))
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

fn fill(paths: &Paths, agent: &str) {
    let depot = paths.agents_dir().join(agent);
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
}

#[allow(clippy::unnecessary_wraps)]
fn status_op() -> Option<v1::agents_request::Op> {
    Some(v1::agents_request::Op::Status(
        v1::agents_request::Status {},
    ))
}

#[allow(clippy::unnecessary_wraps)]
fn set(agent: &str, source: &str) -> Option<v1::agents_request::Op> {
    Some(v1::agents_request::Op::SetSource(
        v1::agents_request::SetSource {
            agent: agent.to_owned(),
            source: source.to_owned(),
        },
    ))
}

fn of<'a>(status: &'a v1::AgentsStatus, agent: &str) -> &'a v1::AgentDepot {
    status
        .agents
        .iter()
        .find(|entry| entry.agent == agent)
        .unwrap()
}

#[test]
fn the_status_lists_every_registered_agent_from_host_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let status = answer(&paths(dir.path()), &SourceLock::none(), None, status_op()).unwrap();
    let ids: Vec<&str> = status
        .agents
        .iter()
        .map(|entry| entry.agent.as_str())
        .collect();
    assert_eq!(ids, humanitl_config::agents::KNOWN_AGENTS);
    for entry in &status.agents {
        assert_eq!(entry.source, "host", "{entry:?}");
        assert_eq!(entry.origin, "default");
        assert!(entry.locked_by.is_empty());
    }
    assert!(of(&status, "claude").depot_supported);
}

#[test]
fn the_status_reads_version_and_signature_from_the_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    let status = answer(
        &paths,
        &SourceLock::none(),
        Some("claude"),
        set("claude", "depot"),
    )
    .unwrap();
    let claude = of(&status, "claude");
    assert_eq!(claude.source, "depot");
    assert_eq!(claude.origin, "agents.<id>.source");
    assert_eq!(claude.version, "2.1.0");
    assert_eq!(claude.signature, "verified");
    assert!(!claude.logged_in);
    assert!(claude.in_use);
    assert!(claude.size_bytes > 0);
    assert!(
        claude
            .diagnostics
            .iter()
            .any(|found| found.code == "AGENT_009"),
        "{claude:?}"
    );
    let text = std::fs::read_to_string(paths.config_path()).unwrap();
    assert!(text.contains("source = \"depot\""), "{text}");
}

/// Under a lock every way of writing `host` is refused, and the file stays
/// as it was: `agents source`, `ChangeSetting` and the generic settings row
/// through `SetConfig` for one agent and for all. `humanitl config set` has
/// its own test in the CLI.
#[test]
fn under_the_lock_no_way_writes_host_and_the_file_stays() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    std::fs::create_dir_all(paths.config_dir()).unwrap();
    std::fs::write(paths.config_path(), "# mine\n").unwrap();
    let lock = locked();

    let refused = answer(&paths, &lock, None, set("claude", "host")).unwrap_err();
    assert_eq!(refused.code.as_str(), "AGENT_010");
    assert!(
        refused.why.contains("/etc/humanitl/policy.toml"),
        "{}",
        refused.why
    );
    assert!(refused.fix.is_none());
    for key in ["agents.claude.source", "agents.source"] {
        let refused = crate::config_rpc::set_locked(
            &paths,
            key,
            "host",
            "",
            &lock,
            &humanitl_config::PolicyLocks::none(),
        )
        .unwrap_err();
        assert_eq!(refused.code.as_str(), "AGENT_010", "{key}");
    }
    assert_eq!(
        std::fs::read_to_string(paths.config_path()).unwrap(),
        "# mine\n"
    );

    let status = answer(&paths, &lock, None, status_op()).unwrap();
    let claude = of(&status, "claude");
    assert_eq!(claude.source, "depot");
    assert_eq!(claude.origin, "policy");
    assert_eq!(claude.locked_by, "/etc/humanitl/policy.toml");
}

#[test]
fn set_config_writes_the_source_without_a_lock() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    crate::config_rpc::set_locked(
        &paths,
        "agents.source",
        "depot",
        "",
        &SourceLock::none(),
        &humanitl_config::PolicyLocks::none(),
    )
    .unwrap();
    assert!(crate::config_rpc::is_form_key(
        "agents.codex.credential_writeback"
    ));
    assert!(!crate::config_rpc::is_form_key("agents.unknown.source"));
    let status = answer(&paths, &SourceLock::none(), None, status_op()).unwrap();
    assert_eq!(of(&status, "codex").source, "depot");
    assert_eq!(of(&status, "codex").origin, "agents.source");
}

/// `remove` is refused while the running session uses the depot and keeps
/// the source; otherwise the depot is gone.
#[test]
fn remove_waits_for_the_session_and_keeps_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    answer(&paths, &SourceLock::none(), None, set("claude", "depot")).unwrap();
    let remove = || {
        Some(v1::agents_request::Op::Remove(v1::agents_request::Remove {
            agent: "claude".to_owned(),
        }))
    };
    let refused = answer(&paths, &SourceLock::none(), Some("claude"), remove()).unwrap_err();
    assert_eq!(refused.code.as_str(), "AGENT_018");
    assert!(paths.agents_dir().join("claude").exists());

    let status = answer(&paths, &SourceLock::none(), None, remove()).unwrap();
    assert!(!paths.agents_dir().join("claude").exists());
    let claude = of(&status, "claude");
    assert_eq!(claude.source, "depot", "remove does not change the source");
    assert!(
        claude
            .diagnostics
            .iter()
            .any(|found| found.code == "AGENT_008"),
        "a session with depot says not installed: {claude:?}"
    );
}

#[test]
fn an_unknown_agent_or_source_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    for op in [set("aider", "depot"), set("claude", "cloud"), None] {
        let refused = answer(&paths, &SourceLock::none(), None, op).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_003");
    }
}

/// One doctor line per agent with the source `depot`, none for `host`.
#[test]
fn the_doctor_has_a_line_per_depot_agent() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    let mut config = Config::default();
    config.agents.claude.source = Some(AgentSource::Depot);
    config.agents.codex.source = Some(AgentSource::Depot);
    let mut report = v1::DoctorReport::default();
    insert_doctor_lines(&mut report, &paths, &config, &SourceLock::none());
    let ids: Vec<&str> = report
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect();
    assert_eq!(ids, ["agent-depot-claude", "agent-depot-codex"]);
    assert!(
        report.checks[0]
            .evidence
            .contains("2.1.0 from the depot, not logged in"),
        "{:?}",
        report.checks[0]
    );
    assert_eq!(report.checks[0].status, v1::CheckStatus::Warn as i32);
    assert_eq!(report.checks[1].status, v1::CheckStatus::Fail as i32);
}

#[test]
fn install_and_update_go_to_the_sandbox_service() {
    let wish = v1::agents_request::Maintain {
        agent: "claude".to_owned(),
        version: " 2.1.89 ".to_owned(),
    };
    let (task, got) = maintenance_of(Some(&v1::agents_request::Op::Install(wish.clone()))).unwrap();
    assert_eq!(task, MaintenanceTask::Install);
    assert_eq!(wanted_version(&got.version).as_deref(), Some("2.1.89"));
    let (task, _) = maintenance_of(Some(&v1::agents_request::Op::Update(wish.clone()))).unwrap();
    assert_eq!(task, MaintenanceTask::Update);
    assert_eq!(wanted_version("  "), None);
    assert!(
        maintenance_of(Some(&v1::agents_request::Op::Status(
            v1::agents_request::Status {}
        )))
        .is_none()
    );
    // The answer of this module never runs one itself.
    let dir = tempfile::tempdir().unwrap();
    let found = answer(
        &paths(dir.path()),
        &SourceLock::none(),
        None,
        Some(v1::agents_request::Op::Install(wish)),
    )
    .unwrap_err();
    assert_eq!(found.code, CONFIG_003);
}

#[test]
fn every_answer_names_the_last_run_of_its_agent() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    let mut status = status(&paths, &Config::default(), &SourceLock::none(), None);
    let run = v1::AgentMaintenance {
        sandbox_id: "s1".to_owned(),
        task: "install".to_owned(),
        state: "running".to_owned(),
        ..v1::AgentMaintenance::default()
    };
    let runs = std::collections::BTreeMap::from([("claude".to_owned(), run.clone())]);
    with_maintenance(&mut status, &runs);
    for entry in &status.agents {
        let expected = (entry.agent == "claude").then(|| run.clone());
        assert_eq!(entry.maintenance, expected, "{}", entry.agent);
    }
}

/// HUM-521: the client switches its buttons by `actions` and decides
/// nothing itself; the daemon derives them from recipe, depot and lock.
#[test]
fn the_actions_follow_recipe_depot_and_lock() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    let fresh = status(&paths, &Config::default(), &SourceLock::none(), None);
    let claude = of(&fresh, "claude").actions.unwrap();
    assert!(claude.install && !claude.update && !claude.config && !claude.remove);
    assert!(claude.source_host && claude.source_depot);
    // Claude Code declares no login run yet (HUM-520), so no login.
    assert!(!claude.login);
    // Ohne Rezept: kein Lauf, aber `set_source depot` nimmt der Daemon an.
    let opencode = of(&fresh, "opencode").actions.unwrap();
    assert!(!opencode.install && opencode.source_depot && opencode.source_host);
    assert!(!opencode.remove, "no depot, nothing to remove");
    assert!(of(&fresh, "opencode").credential_writeback);

    fill(&paths, "claude");
    let filled = status(&paths, &Config::default(), &locked(), None);
    let entry = of(&filled, "claude");
    assert_eq!(entry.versions, vec!["2.1.0"]);
    let claude = entry.actions.unwrap();
    assert!(!claude.install && claude.update && claude.config && claude.config_show);
    assert!(claude.remove);
    assert!(!claude.source_host, "the lock takes host away");

    let used = status(
        &paths,
        &Config::default(),
        &SourceLock::none(),
        Some("claude"),
    );
    assert!(!of(&used, "claude").actions.unwrap().remove);
}

#[test]
fn a_running_run_takes_back_every_maintenance_action() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    let mut status = status(&paths, &Config::default(), &SourceLock::none(), None);
    let run = v1::AgentMaintenance {
        sandbox_id: "s1".to_owned(),
        task: "update".to_owned(),
        state: "running".to_owned(),
        ..v1::AgentMaintenance::default()
    };
    let runs = std::collections::BTreeMap::from([("claude".to_owned(), run)]);
    with_maintenance(&mut status, &runs);
    let actions = of(&status, "claude").actions.unwrap();
    assert!(!actions.update && !actions.config && !actions.remove && !actions.install);
    assert!(actions.config_show && actions.source_host);
}

/// HUM-521 (review): an agent without a recipe whose depot exists can still
/// lose it, as `agents remove` does.
#[test]
fn a_depot_without_recipe_can_be_removed() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "opencode");
    let state = status(&paths, &Config::default(), &SourceLock::none(), None);
    let actions = of(&state, "opencode").actions.unwrap();
    assert!(actions.remove && actions.config_show);
    assert!(!actions.install && !actions.update && !actions.login && !actions.config);
}

/// HUM-521 (review): a depot that fails its checks takes no install; the
/// run would stop with `AGENT_013`.
#[test]
fn an_unusable_depot_offers_no_install() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    let current = paths.agents_dir().join("claude/current");
    std::fs::remove_file(&current).unwrap();
    std::fs::write(&current, b"not a link").unwrap();
    let state = status(&paths, &Config::default(), &SourceLock::none(), None);
    let entry = of(&state, "claude");
    assert!(entry.version.is_empty());
    let actions = entry.actions.unwrap();
    assert!(!actions.install && !actions.update, "{entry:?}");
    assert!(actions.remove);
}

/// HUM-521 (review): no run while the daemon could not start a session.
#[test]
fn without_a_start_no_run_is_offered() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    let mut state = status(&paths, &Config::default(), &SourceLock::none(), None);
    with_startable(&mut state, true);
    assert!(of(&state, "claude").actions.unwrap().update);
    with_startable(&mut state, false);
    let actions = of(&state, "claude").actions.unwrap();
    assert!(!actions.install && !actions.update && !actions.login && !actions.config);
    assert!(actions.remove && actions.config_show);
}

/// HUM-521 (review): after a restart of the daemon a configuration that
/// waits on disk still stands in the status, ahead of an ended run.
#[test]
fn a_configuration_waiting_on_disk_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(dir.path());
    fill(&paths, "claude");
    let pending = account::pending_dir(&paths, "claude");
    std::fs::create_dir_all(&pending).unwrap();
    std::fs::write(pending.join("settings.json"), br#"{"hooks":{}}"#).unwrap();
    let mut state = status(&paths, &Config::default(), &SourceLock::none(), None);
    let run = of(&state, "claude").maintenance.clone().unwrap();
    assert_eq!(run.state, "confirm");
    assert_eq!(run.task, "config");
    assert_eq!(run.config_changes[0].path, "settings.json");
    let ended = v1::AgentMaintenance {
        task: "install".to_owned(),
        state: "done".to_owned(),
        ..v1::AgentMaintenance::default()
    };
    let runs = std::collections::BTreeMap::from([("claude".to_owned(), ended)]);
    with_maintenance(&mut state, &runs);
    assert_eq!(
        of(&state, "claude").maintenance.clone().unwrap().state,
        "confirm"
    );
}

#[test]
fn accept_and_discard_are_told_apart() {
    let wish = || v1::agents_request::Agent {
        agent: "claude".to_owned(),
    };
    assert_eq!(
        config_decision_of(Some(&v1::agents_request::Op::ConfigAccept(wish()))),
        Some(("claude".to_owned(), true))
    );
    assert_eq!(
        config_decision_of(Some(&v1::agents_request::Op::ConfigDiscard(wish()))),
        Some(("claude".to_owned(), false))
    );
}
