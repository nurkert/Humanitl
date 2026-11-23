//! The source of the agent as part of the project choice (GitHub #15,
//! addendum to ADR-0023).

use humanitl_config::PolicyLocks;
use humanitl_config::agents::{AgentSource, LockScope, SourceLock};

use super::{
    AGENT, AGENT_SOURCE, Fixture, ask, codes, field, live_service, nothing_missing, option, propose,
};
use crate::projects::answer;
use crate::v1;

/// The company policy that locks a source in these tests.
const POLICY: &str = "/etc/humanitl/policy.toml";

/// Claude Code in the depot under the fixture's data directory, filled by
/// hand as the install of HUM-519 leaves it, without a login.
fn fill_claude_depot(fixture: &Fixture) {
    use std::os::unix::fs::PermissionsExt as _;

    let agents = fixture.paths().agents_dir();
    let depot = agents.join("claude");
    let version = depot.join("program/2.1.0");
    for dir in [
        agents.clone(),
        depot.clone(),
        depot.join("program"),
        version.clone(),
        depot.join("config"),
        depot.join("credentials"),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::fs::write(version.join("claude"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(
        version.join("claude"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
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

/// The value of `key` in these pairs.
fn pair<'a>(pairs: &'a [v1::ChoiceValue], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|pair| pair.key == key)
        .map(|pair| pair.value.as_str())
}

/// The status of a `Plan` in `dir` and the findings that came with it.
async fn plan_in(
    service: &crate::SandboxService,
    dir: &std::path::Path,
    overrides: &[(&str, &str)],
) -> (v1::sandbox_event::Status, Vec<v1::Diagnostic>) {
    use tokio_stream::StreamExt as _;

    let events: Vec<v1::SandboxEvent> = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Plan(v1::sandbox_request::Plan {
                    work_dir: dir.display().to_string(),
                    cli_overrides: overrides
                        .iter()
                        .map(|(path, value)| v1::sandbox_request::CliOverride {
                            path: (*path).to_owned(),
                            value: (*value).to_owned(),
                        })
                        .collect(),
                    ..v1::sandbox_request::Plan::default()
                })),
            })
            .collect(),
    )
    .await
    .expect("a Plan answers within a minute");
    let mut status = None;
    let mut found = Vec::new();
    for event in events {
        match event.event {
            Some(v1::sandbox_event::Event::Status(answered)) => status = Some(answered),
            Some(v1::sandbox_event::Event::Diagnostic(diagnostic)) => found.push(diagnostic),
            _ => {}
        }
    }
    (status.expect("a Plan answers with a status"), found)
}

/// Claude Code lies only in the depot, and nothing chooses its source: the
/// proposal preselects `depot`, and the pairs of the agent carry the source,
/// so the saved defaults start from the depot. Before the choice is saved, a
/// start without a screen takes `host` as ADR-0023 says and does not fall
/// back to the depot.
///
/// Mutation proof: without the pair `agents.<id>.source` in the options of
/// the agent field (`agent_field`), the first assertion on the pairs is red.
/// Without the preselection (`preselected` returning the effective source),
/// the source field defaults to `host` and the test is red as well.
#[tokio::test]
async fn an_agent_only_in_the_depot_preselects_the_depot_and_the_saved_choice_plans_from_it() {
    let fixture = Fixture::new("[agent]\nadapter = \"claude\"\n");
    fill_claude_depot(&fixture);
    let dir = fixture.folder("repo");
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(&dir, &[]),
    )
    .await;

    let agent = field(&proposal, AGENT);
    assert_eq!(agent.value, "claude");
    assert!(
        agent.keys.iter().any(|key| key == "agents.claude.source"),
        "choosing an agent also writes the source keys: {agent:?}"
    );
    let claude = option(agent, "claude");
    assert_eq!(
        pair(&claude.choice, "agents.claude.source"),
        Some("depot"),
        "{claude:?}"
    );
    assert_eq!(pair(&claude.choice, "agent.adapter"), Some("claude"));
    assert_eq!(claude.source, "depot");
    assert!(claude.available, "{claude:?}");
    let depot = claude
        .sources
        .iter()
        .find(|source| source.source == "depot")
        .unwrap();
    assert!(depot.available && depot.selected, "{depot:?}");
    let host = claude
        .sources
        .iter()
        .find(|source| source.source == "host")
        .unwrap();
    assert!(!host.available && !host.selected, "{host:?}");
    assert_eq!(codes(&host.reasons), ["AGENT_001"]);

    depot_preselected_in_the_source_field(&proposal);
    // One pair per key, although agent and source both carry it.
    assert_eq!(
        proposal
            .choice
            .iter()
            .filter(|pair| pair.key == "agents.claude.source")
            .count(),
        1,
        "{:?}",
        proposal.choice
    );
    assert_eq!(
        pair(&proposal.choice, "agents.claude.source"),
        Some("depot")
    );
    assert!(
        proposal.defaults_line.contains("from the depot"),
        "{}",
        proposal.defaults_line
    );
    assert!(
        !codes(&proposal.diagnostics).contains(&"AGENT_001"),
        "{:?}",
        proposal.diagnostics
    );

    // Without a project choice the start keeps ADR-0023: host, no fallback.
    let service = live_service(&fixture, fixture.paths());
    let (status, found) = plan_in(&service, &dir, &[]).await;
    assert_eq!(
        depot_mount(&status),
        None,
        "a start without a choice takes the host: {found:?}"
    );

    // The screen saves the defaults; the next start takes the depot.
    answer(
        fixture.context(PolicyLocks::none(), nothing_missing),
        v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    path: dir.display().to_string(),
                    values: proposal.choice.clone(),
                    ..Default::default()
                },
            )),
        },
    )
    .await
    .expect("the defaults save");
    let service = live_service(&fixture, fixture.paths());
    let (status, found) = plan_in(&service, &dir, &[]).await;
    // Der Depot-Mount steht nur im Plan des Linux-Backends; auf macOS
    // bindet das Backend das Programm anders ein.
    if cfg!(target_os = "linux") {
        let program = depot_mount(&status).unwrap_or_else(|| {
            panic!(
                "the saved choice starts from the depot: {found:?} {:?}",
                status.mounts
            )
        });
        assert!(program.ends_with("claude/program/2.1.0"), "{program}");
    }
}

/// The source field of claude in the depot only: `depot` preselected, `host`
/// not available with `AGENT_001` and still carrying its pair.
fn depot_preselected_in_the_source_field(proposal: &v1::StartProposal) {
    let source = field(proposal, AGENT_SOURCE);
    assert_eq!(source.value, "depot", "{source:?}");
    assert_eq!(source.keys, ["agents.claude.source"]);
    assert!(source.lock.is_none());
    assert!(option(source, "depot").available);
    let host = option(source, "host");
    assert!(!host.available, "{host:?}");
    assert_eq!(codes(&host.reasons), ["AGENT_001"]);
    assert_eq!(
        pair(&host.choice, "agents.claude.source"),
        Some("host"),
        "the other source is choosable through its pairs once it is ready"
    );
}

/// The host folder a `Plan` mounts as the agent's program directory from
/// the depot (`/run/humanitl/agent`), if it mounts one.
fn depot_mount(status: &v1::sandbox_event::Status) -> Option<String> {
    status
        .mounts
        .iter()
        .find(|mount| mount.dst == "/run/humanitl/agent")
        .map(|mount| mount.src.clone())
}

/// A stored source is the default, also where its program is missing: the
/// default is the start that would happen with the stored choice, and that
/// start stops with `AGENT_001` instead of falling back to the depot.
///
/// Mutation proof: a `preselected` that ignores `AgentFacts::fixed` picks
/// the depot here, and the test is red.
#[tokio::test]
async fn a_stored_source_stays_the_default_even_where_the_program_is_missing() {
    let fixture = Fixture::new("");
    fill_claude_depot(&fixture);
    let row = fixture
        .known(
            "repo",
            Some(r#"{"agent.adapter":"claude","agents.claude.source":"host"}"#),
        )
        .await;
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(std::path::Path::new(&row.path), &[]),
    )
    .await;

    let source = field(&proposal, AGENT_SOURCE);
    assert_eq!(source.value, "host", "{source:?}");
    assert_eq!(source.origin, "project-choice");
    let claude = option(field(&proposal, AGENT), "claude");
    assert_eq!(claude.source, "host");
    assert!(!claude.available, "{claude:?}");
    assert_eq!(pair(&claude.choice, "agents.claude.source"), Some("host"));
    assert!(
        codes(&proposal.diagnostics).contains(&"AGENT_001"),
        "{:?}",
        proposal.diagnostics
    );
}

/// A company policy locks the source of claude: the source field is read
/// only with the policy named as its origin, `host` is not choosable and
/// says why, no pair carries the source, and a choice that names it is
/// refused with the policy named.
///
/// Mutation proof: without the lock on the source field (`source_field`),
/// or without `check_source_lock` in `check_for`, this test is red.
#[tokio::test]
async fn a_source_lock_is_read_only_named_and_stays_out_of_the_choice() {
    let fixture = Fixture::new("[agent]\nadapter = \"claude\"\n");
    fill_claude_depot(&fixture);
    let dir = fixture.folder("repo");
    let lock = SourceLock::none()
        .lock(
            LockScope::Agent("claude".to_owned()),
            AgentSource::Depot,
            POLICY,
        )
        .unwrap();
    let context = || {
        let mut ctx = fixture.context(PolicyLocks::none(), nothing_missing);
        ctx.proposal.source_lock = lock.clone();
        ctx
    };
    let proposal = propose(context(), ask(&dir, &[])).await;

    let source = field(&proposal, AGENT_SOURCE);
    assert_eq!(source.value, "depot");
    let locked = source.lock.as_ref().expect("the source names its lock");
    assert!(locked.read_only);
    assert_eq!(locked.locked_by, POLICY);
    assert_eq!(source.origin, "policy");
    let host = option(source, "host");
    assert!(!host.available, "{host:?}");
    assert!(codes(&host.reasons).contains(&"AGENT_010"), "{host:?}");
    assert!(
        host.reasons
            .iter()
            .any(|reason| reason.why.contains(POLICY)),
        "{host:?}"
    );
    assert!(option(source, "depot").choice.is_empty());
    let claude = option(field(&proposal, AGENT), "claude");
    assert_eq!(claude.source, "depot");
    assert_eq!(pair(&claude.choice, "agents.claude.source"), None);
    assert_eq!(pair(&proposal.choice, "agents.claude.source"), None);
    // Another agent's source is not locked and stays in its pairs.
    let codex = option(field(&proposal, AGENT), "codex");
    assert!(
        pair(&codex.choice, "agents.codex.source").is_some(),
        "{codex:?}"
    );

    // The defaults save; a choice that names the locked source does not.
    answer(
        context(),
        v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    path: dir.display().to_string(),
                    values: proposal.choice.clone(),
                    ..Default::default()
                },
            )),
        },
    )
    .await
    .expect("the defaults save");
    for value in ["host", "depot"] {
        let refused = answer(
            context(),
            v1::ProjectsRequest {
                op: Some(v1::projects_request::Op::SaveChoice(
                    v1::projects_request::SaveChoice {
                        path: dir.display().to_string(),
                        values: vec![v1::ChoiceValue {
                            key: "agents.claude.source".to_owned(),
                            value: value.to_owned(),
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .await
        .expect_err("a locked source stays out of the choice");
        assert_eq!(refused.code.as_str(), "CONFIG_024", "{value}");
        assert!(refused.why.contains(POLICY), "{}", refused.why);
    }
}

/// Saves `choice` for the folder `dir` as the first-start screen does.
async fn save(ctx: crate::projects::Context, dir: &std::path::Path, choice: &[v1::ChoiceValue]) {
    answer(
        ctx,
        v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    path: dir.display().to_string(),
                    values: choice.to_vec(),
                    ..Default::default()
                },
            )),
        },
    )
    .await
    .expect("the defaults save");
}

/// A source set in `config.toml`, per agent or for all, stays the default
/// although the depot is missing and the host has the program: no silent
/// switch (ADR-0023); the source names why it does not start.
///
/// Mutation proof: counting only project choice, command line and policy as
/// fixed (the first version of `AgentFacts::fixed`) preselects `host` here,
/// and the test is red.
#[tokio::test]
async fn a_source_set_in_the_configuration_is_never_switched() {
    for config in [
        "[agent]\nadapter = \"claude\"\n[agents.claude]\nsource = \"depot\"\n",
        "[agent]\nadapter = \"claude\"\n[agents]\nsource = \"depot\"\n",
    ] {
        let fixture = Fixture::new(config);
        let bin = fixture.folder(".local/bin");
        crate::test_support::write_executable(&bin.join("claude"), "#!/bin/sh\n");
        let dir = fixture.folder("repo");
        let proposal = propose(
            fixture.context(PolicyLocks::none(), nothing_missing),
            ask(&dir, &[]),
        )
        .await;

        let source = field(&proposal, AGENT_SOURCE);
        assert_eq!(source.value, "depot", "{config}: {source:?}");
        let depot = option(source, "depot");
        assert!(!depot.available, "{depot:?}");
        assert!(!depot.reasons.is_empty(), "the depot says why: {depot:?}");
        assert!(option(source, "host").available);
        let claude = option(field(&proposal, AGENT), "claude");
        assert_eq!(claude.source, "depot");
        assert!(!claude.available, "{claude:?}");
        assert_eq!(
            pair(&proposal.choice, "agents.claude.source"),
            Some("depot"),
            "{config}"
        );
    }
}

/// `HUMANITL_AGENTS__CLAUDE__SOURCE=host` outranks a project choice: the
/// proposal keeps `host` although only the depot holds claude, saves `host`,
/// and the plan after it takes the host as well. Proposal, saved choice and
/// start agree.
///
/// Mutation proof: a `fixed` that ignores the environment preselects and
/// saves `depot` here, and the test is red.
#[tokio::test]
async fn a_source_from_the_environment_is_what_proposal_save_and_plan_take() {
    let fixture = Fixture::new("[agent]\nadapter = \"claude\"\n");
    fill_claude_depot(&fixture);
    let dir = fixture.folder("repo");
    let paths = humanitl_config::Paths::new(
        fixture
            .paths()
            .env()
            .clone()
            .with("HUMANITL_AGENTS__CLAUDE__SOURCE", "host"),
    );
    let context = || {
        let mut ctx = fixture.context(PolicyLocks::none(), nothing_missing);
        ctx.paths = paths.clone();
        ctx
    };
    let proposal = propose(context(), ask(&dir, &[])).await;
    let source = field(&proposal, AGENT_SOURCE);
    assert_eq!(source.value, "host", "{source:?}");
    assert_eq!(source.origin, "env");
    assert_eq!(pair(&proposal.choice, "agents.claude.source"), Some("host"));
    assert!(
        codes(&proposal.diagnostics).contains(&"AGENT_001"),
        "the start would stop on the host: {:?}",
        proposal.diagnostics
    );

    save(context(), &dir, &proposal.choice).await;
    let service = live_service(&fixture, paths.clone());
    let (status, found) = plan_in(&service, &dir, &[]).await;
    assert_eq!(
        depot_mount(&status),
        None,
        "the plan takes the host: {found:?}"
    );
}

/// `humanitl go claude --yes` with `OpenCode` configured and claude only in
/// the depot: the line shows claude from the depot, and the saved choice
/// carries that source although the agent itself is a value of this start
/// only; the next plan with the same word takes the depot.
///
/// Mutation proof: without keeping the shown agent's source pair when the
/// choice is rebuilt without the session values (`propose`), the saved
/// choice lacks `agents.claude.source` and the plan takes the host: red.
#[tokio::test]
async fn the_agent_of_a_start_keeps_its_shown_source_in_the_saved_choice() {
    let fixture = Fixture::new("[agent]\nadapter = \"opencode\"\n");
    fill_claude_depot(&fixture);
    let dir = fixture.folder("repo");
    let word = [("agent.adapter", "claude")];
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        super::ask_with(&dir, &word),
    )
    .await;
    assert_eq!(field(&proposal, AGENT).value, "claude");
    assert_eq!(field(&proposal, AGENT_SOURCE).value, "depot");
    assert!(
        proposal.defaults_line.contains("from the depot"),
        "{}",
        proposal.defaults_line
    );
    assert_eq!(
        pair(&proposal.choice, "agent.adapter"),
        Some("opencode"),
        "the agent of the start is not saved"
    );
    assert_eq!(
        pair(&proposal.choice, "agents.claude.source"),
        Some("depot"),
        "{:?}",
        proposal.choice
    );

    save(
        fixture.context(PolicyLocks::none(), nothing_missing),
        &dir,
        &proposal.choice,
    )
    .await;
    let service = live_service(&fixture, fixture.paths());
    let (status, found) = plan_in(&service, &dir, &word).await;
    // Der Depot-Mount steht nur im Plan des Linux-Backends.
    if cfg!(target_os = "linux") {
        let program = depot_mount(&status)
            .unwrap_or_else(|| panic!("go claude starts from the depot: {found:?}"));
        assert!(program.ends_with("claude/program/2.1.0"), "{program}");
    }
}

/// A choice stored before #15 names the agent and no source: a start
/// without a screen takes the effective source, `host`, so the proposal of
/// that project preselects `host` too and names why it would stop, instead
/// of showing a depot start that does not happen.
///
/// Mutation proof: without `fixed |= stored` in `propose` the proposal
/// shows `depot`, and the test is red.
#[tokio::test]
async fn a_stored_choice_without_a_source_shows_the_start_it_makes() {
    let fixture = Fixture::new("");
    fill_claude_depot(&fixture);
    let row = fixture
        .known("repo", Some(r#"{"agent.adapter":"claude"}"#))
        .await;
    let proposal = propose(
        fixture.context(PolicyLocks::none(), nothing_missing),
        ask(std::path::Path::new(&row.path), &[]),
    )
    .await;
    assert_eq!(field(&proposal, AGENT_SOURCE).value, "host");
    let claude = option(field(&proposal, AGENT), "claude");
    assert_eq!(claude.source, "host");
    assert!(!claude.available, "{claude:?}");
    assert!(
        codes(&proposal.diagnostics).contains(&"AGENT_001"),
        "{:?}",
        proposal.diagnostics
    );
    // The depot stays choosable on the screen.
    assert!(option(field(&proposal, AGENT_SOURCE), "depot").available);
}

/// `agents.source = "host"` set explicitly, in `config.toml` or in the
/// environment, is set although it equals the default: with claude only in
/// the depot the proposal keeps `host` with `AGENT_001`, saves `host`, and
/// the plan after it takes the host too.
///
/// Mutation proof: deciding "set" from the value of `agents.source` again
/// (only the per-agent origin, `effective_source`) preselects and saves
/// `depot` here, and the test is red.
#[tokio::test]
async fn an_explicit_global_host_is_kept_from_config_and_environment() {
    for (config, env) in [
        (
            "[agent]\nadapter = \"claude\"\n[agents]\nsource = \"host\"\n",
            None,
        ),
        (
            "[agent]\nadapter = \"claude\"\n",
            Some(("HUMANITL_AGENTS__SOURCE", "host")),
        ),
    ] {
        let fixture = Fixture::new(config);
        fill_claude_depot(&fixture);
        let dir = fixture.folder("repo");
        let mut env_pairs = fixture.paths().env().clone();
        if let Some((key, value)) = env {
            env_pairs = env_pairs.with(key, value);
        }
        let paths = humanitl_config::Paths::new(env_pairs);
        let context = || {
            let mut ctx = fixture.context(PolicyLocks::none(), nothing_missing);
            ctx.paths = paths.clone();
            ctx
        };
        let proposal = propose(context(), ask(&dir, &[])).await;
        let source = field(&proposal, AGENT_SOURCE);
        assert_eq!(source.value, "host", "{config} {env:?}: {source:?}");
        assert!(
            codes(&proposal.diagnostics).contains(&"AGENT_001"),
            "{:?}",
            proposal.diagnostics
        );
        assert_eq!(pair(&proposal.choice, "agents.claude.source"), Some("host"));

        save(context(), &dir, &proposal.choice).await;
        let service = live_service(&fixture, paths.clone());
        let (status, found) = plan_in(&service, &dir, &[]).await;
        assert_eq!(
            depot_mount(&status),
            None,
            "the plan takes the host: {found:?}"
        );
    }
}
