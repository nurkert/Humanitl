//! Tests of `Projects.cleanup_plan` and `Projects.cleanup` (HUM-533).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::SystemTime;

use humanitl_audit_log::VerifyFile as _;
use humanitl_config::{Env, Paths, PolicyLocks};
use humanitl_core::{
    Authority, FlowEvent, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
};
use humanitl_recorder::{
    CleanupKind, CleanupScope, CleanupTarget, LiveState, ProjectRow, ProjectStart, Recorder,
    RecorderSettings, SessionMeta, WaitingLayer,
};

use super::{NOT_DELETED, WAITING_WARNING, item_view, refuse_incomplete};
use crate::projects::{Context, ProjectActivity, ProjectIdentity, answer};
use crate::v1;

struct Fixture {
    home: tempfile::TempDir,
    recorder: Recorder,
}

impl Fixture {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("config/humanitl")).unwrap();
        let recorder = Recorder::open(
            &home.path().join("data/humanitl/humanitl.db"),
            &home.path().join("data/humanitl/blobs"),
            RecorderSettings::default(),
        )
        .unwrap();
        Self { home, recorder }
    }

    fn context(&self, activity: Option<ProjectActivity>) -> Context {
        let home = self.home.path();
        Context {
            recorder: Some(self.recorder.clone()),
            paths: Paths::new(Env::from_pairs([
                ("HOME", home.display().to_string()),
                ("XDG_CONFIG_HOME", home.join("config").display().to_string()),
                ("XDG_DATA_HOME", home.join("data").display().to_string()),
            ])),
            locks: PolicyLocks::none(),
            activity,
            sandbox: None,
            audit: None,
        }
    }

    fn folder(&self, name: &str) -> String {
        let dir = self.home.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        ProjectIdentity::of(&dir).unwrap().path_text()
    }

    /// A session of the daemon with one run in `name` and one request.
    async fn run_in(&self, name: &str) -> (SessionId, ProjectRow, FlowId) {
        let dir = self.home.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let identity = ProjectIdentity::of(&dir).unwrap();
        let session = SessionId::new();
        self.recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: String::new(),
            agent: "opencode".to_owned(),
        });
        let project = self
            .recorder
            .register_project_start(ProjectStart {
                session,
                path: identity.path_text(),
                dev: identity.dev,
                ino: identity.ino,
                agent: "claude".to_owned(),
                at: SystemTime::now(),
            })
            .await
            .unwrap()
            .project;
        let flow = FlowId::new();
        self.recorder.apply(&FlowEvent::Received {
            flow_id: flow,
            at: SystemTime::now(),
            request: Box::new(HttpRequest::new(
                Method::GET,
                Scheme::Https,
                Authority::with_scheme(HostName::parse("api.example").unwrap(), Scheme::Https),
                "/secret-path",
            )),
        });
        self.recorder.end_session(session);
        self.recorder.flush().await;
        (session, project, flow)
    }

    async fn has_flow(&self, flow: FlowId) -> bool {
        self.recorder.get_flow(flow).await.unwrap().is_some()
    }
}

fn plan_request(selection: v1::CleanupSelection) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::CleanupPlan(
            v1::projects_request::CleanupPlan {
                selection: Some(selection),
            },
        )),
    }
}

fn run_request(plan: &v1::CleanupPlanView) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::Cleanup(
            v1::projects_request::Cleanup {
                selection: plan.selection.clone(),
                digest: plan.digest.clone(),
            },
        )),
    }
}

fn save(project: &ProjectRow, mode: &str) -> v1::ProjectsRequest {
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::SaveChoice(
            v1::projects_request::SaveChoice {
                project_id: project.id.to_string(),
                values: vec![v1::ChoiceValue {
                    key: "sandbox.work_mode".to_owned(),
                    value: mode.to_owned(),
                }],
            },
        )),
    }
}

fn kinds(plan: &v1::CleanupPlanView) -> Vec<(String, String)> {
    plan.items
        .iter()
        .map(|item| (item.kind.clone(), item.project_id.clone()))
        .collect()
}

/// The plan names its items with sentences, the line of what is never
/// deleted and the kinds of the daemon; the run deletes exactly that, leaves
/// one `projects.cleaned` without contents in the audit chain, and the chain
/// verifies afterwards.
#[tokio::test]
async fn a_confirmed_plan_runs_and_leaves_one_audit_record() {
    let fixture = Fixture::new();
    let (_a, alpha, alpha_flow) = fixture.run_in("alpha").await;
    let (_b, _beta, beta_flow) = fixture.run_in("beta").await;

    let planned = answer(
        fixture.context(None),
        plan_request(v1::CleanupSelection {
            paths: vec![fixture.folder("alpha")],
            ..v1::CleanupSelection::default()
        }),
    )
    .await
    .unwrap()
    .plan
    .unwrap();
    assert_eq!(planned.not_deleted, NOT_DELETED);
    assert!(
        planned
            .not_deleted
            .contains("the project folder, the audit log")
    );
    assert_eq!(
        planned.kinds,
        [
            "recorded_flows",
            "waiting_changes",
            "project_choice",
            "register_entry"
        ]
    );
    let id = alpha.id.to_string();
    assert_eq!(
        kinds(&planned),
        [
            ("recorded_flows".to_owned(), id.clone()),
            ("register_entry".to_owned(), id.clone()),
        ]
    );
    assert!(
        planned.items[0]
            .description
            .starts_with("1 recorded request ")
    );
    assert!(planned.items[0].bytes > 0);
    assert_eq!(
        planned.selection.as_ref().unwrap().project_ids,
        std::slice::from_ref(&id),
        "the folder comes back as the project id"
    );

    let dir = tempfile::tempdir().unwrap();
    let key = humanitl_audit::AuditKey::from_bytes([3; 32], humanitl_audit::KeyOrigin::File);
    let (writer, _) = humanitl_audit_log::AuditWriter::open(
        &dir.path().join("audit.jsonl"),
        &key,
        humanitl_audit_log::WriterOptions::default(),
        &[],
        None,
    )
    .unwrap();
    let mut ctx = fixture.context(None);
    ctx.audit = Some(writer.handle());
    let cleaned = answer(ctx, run_request(&planned))
        .await
        .unwrap()
        .cleaned
        .unwrap();
    assert!(cleaned.outcomes.iter().all(|outcome| outcome.deleted));
    assert_eq!(cleaned.not_deleted, NOT_DELETED);
    assert!(!fixture.has_flow(alpha_flow).await);
    assert!(fixture.has_flow(beta_flow).await);

    writer.handle().sync().unwrap();
    let path = writer.path().to_path_buf();
    drop(writer);
    let text = std::fs::read_to_string(&path).unwrap();
    let records: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("\"kind\":\"projects.cleaned\""))
        .collect();
    assert_eq!(records.len(), 1, "{text}");
    assert!(records[0].contains("\"recorded_flows\""), "{}", records[0]);
    assert!(!records[0].contains("alpha"), "no path: {}", records[0]);
    assert!(!records[0].contains(&id), "no project id: {}", records[0]);
    assert!(
        !records[0].contains("secret-path"),
        "no content: {}",
        records[0]
    );
    let report = humanitl_audit::AuditVerifier::verify(&path, Some(&[3; 32]), &[]).unwrap();
    assert!(report.is_ok(), "{report:?}");
    assert_eq!(report.records, 1, "{report:?}");
}

/// Forgetting plans exactly the choice and the register entry of the one
/// project; the flows stay. A choice changed after the plan makes it
/// `SANDBOX_054`, and the new choice stays.
///
/// Mutation proof: a forget that deletes in the handler without the plan
/// (`cleanup` deleting choice and entry of `forget` directly) loses the
/// changed choice, and the test turns red.
#[tokio::test]
async fn forgetting_runs_only_an_unchanged_plan() {
    let fixture = Fixture::new();
    let (_a, alpha, alpha_flow) = fixture.run_in("alpha").await;
    answer(fixture.context(None), save(&alpha, "ro"))
        .await
        .unwrap();
    let forget = || {
        plan_request(v1::CleanupSelection {
            forget: alpha.id.to_string(),
            ..v1::CleanupSelection::default()
        })
    };
    let planned = answer(fixture.context(None), forget())
        .await
        .unwrap()
        .plan
        .unwrap();
    let id = alpha.id.to_string();
    assert_eq!(
        kinds(&planned),
        [
            ("project_choice".to_owned(), id.clone()),
            ("register_entry".to_owned(), id.clone()),
        ]
    );

    answer(fixture.context(None), save(&alpha, "rw"))
        .await
        .unwrap();
    let refused = answer(fixture.context(None), run_request(&planned))
        .await
        .expect_err("stale");
    assert_eq!(refused.code.as_str(), "SANDBOX_054", "{}", refused.why);
    let row = fixture
        .recorder
        .get_project(alpha.id)
        .await
        .unwrap()
        .unwrap();
    assert!(row.choice.as_deref().unwrap().contains("rw"));

    let planned = answer(fixture.context(None), forget())
        .await
        .unwrap()
        .plan
        .unwrap();
    answer(fixture.context(None), run_request(&planned))
        .await
        .unwrap();
    assert!(
        fixture
            .recorder
            .get_project(alpha.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(fixture.has_flow(alpha_flow).await, "forgetting keeps flows");
}

/// A project whose session runs cannot be forgotten: `SANDBOX_054` at the
/// plan, nothing deleted.
#[tokio::test]
async fn forgetting_a_running_project_is_sandbox_054() {
    let fixture = Fixture::new();
    let (session, alpha, _flow) = fixture.run_in("alpha").await;
    let activity = ProjectActivity {
        project: alpha.id,
        running: true,
        changes_waiting: false,
        session,
    };
    let refused = answer(
        fixture.context(Some(activity)),
        plan_request(v1::CleanupSelection {
            forget: alpha.id.to_string(),
            ..v1::CleanupSelection::default()
        }),
    )
    .await
    .expect_err("in use");
    assert_eq!(refused.code.as_str(), "SANDBOX_054");
    assert!(
        fixture
            .recorder
            .get_project(alpha.id)
            .await
            .unwrap()
            .is_some()
    );
}

/// A selection names exactly one target; `cleanup` takes only the
/// selection a plan returned and a digest.
#[tokio::test]
async fn a_selection_is_read_strictly() {
    let fixture = Fixture::new();
    let (_a, alpha, _flow) = fixture.run_in("alpha").await;
    let both = v1::CleanupSelection {
        all: true,
        unassigned: true,
        ..v1::CleanupSelection::default()
    };
    let none = v1::CleanupSelection::default();
    for selection in [both, none] {
        let refused = answer(fixture.context(None), plan_request(selection))
            .await
            .expect_err("refused");
        assert_eq!(refused.code.as_str(), "CONFIG_003");
    }
    let by_folder = v1::CleanupPlanView {
        selection: Some(v1::CleanupSelection {
            paths: vec![fixture.folder("alpha")],
            ..v1::CleanupSelection::default()
        }),
        digest: "00".repeat(32),
        ..v1::CleanupPlanView::default()
    };
    let by_age = v1::CleanupPlanView {
        selection: Some(v1::CleanupSelection {
            project_ids: vec![alpha.id.to_string()],
            older_than_days: 30,
            ..v1::CleanupSelection::default()
        }),
        digest: "00".repeat(32),
        ..v1::CleanupPlanView::default()
    };
    let without_digest = v1::CleanupPlanView {
        selection: Some(v1::CleanupSelection {
            project_ids: vec![alpha.id.to_string()],
            ..v1::CleanupSelection::default()
        }),
        ..v1::CleanupPlanView::default()
    };
    for plan in [by_folder, by_age, without_digest] {
        let refused = answer(fixture.context(None), run_request(&plan))
            .await
            .expect_err("refused");
        assert_eq!(refused.code.as_str(), "CONFIG_003", "{}", refused.why);
    }
    // Ein Digest, der zu keinem Plan gehört, ist ein veralteter Plan.
    let wrong = v1::CleanupPlanView {
        selection: Some(v1::CleanupSelection {
            project_ids: vec![alpha.id.to_string()],
            ..v1::CleanupSelection::default()
        }),
        digest: "00".repeat(32),
        ..v1::CleanupPlanView::default()
    };
    let refused = answer(fixture.context(None), run_request(&wrong))
        .await
        .expect_err("stale");
    assert_eq!(refused.code.as_str(), "SANDBOX_054");
    assert!(
        fixture
            .recorder
            .get_project(alpha.id)
            .await
            .unwrap()
            .is_some()
    );
}

/// Waiting changes carry the warning every client shows before it asks.
///
/// Mutation proof: without the warning in `item_view` the test turns red.
#[tokio::test]
async fn waiting_changes_carry_the_warning() {
    let fixture = Fixture::new();
    let (_a, alpha, _flow) = fixture.run_in("alpha").await;
    let live = LiveState {
        waiting: Some(WaitingLayer {
            project: alpha.id,
            layer: "layer".to_owned(),
            count: 2,
            bytes: 10,
            complete: true,
        }),
        ..LiveState::default()
    };
    let scope = CleanupScope {
        target: CleanupTarget::Projects(vec![alpha.id]),
        before: None,
        include_waiting: true,
    };
    let plan = fixture.recorder.cleanup_plan(scope, live).await.unwrap();
    let waiting = plan
        .items
        .iter()
        .find(|item| item.kind == CleanupKind::WaitingChanges)
        .unwrap();
    let shown = item_view(waiting);
    assert_eq!(shown.warning, WAITING_WARNING);
    assert!(shown.warning.contains("lost for good"));
    for other in plan
        .items
        .iter()
        .filter(|item| item.kind != CleanupKind::WaitingChanges)
    {
        assert!(item_view(other).warning.is_empty());
    }
}

/// Review HUM-533: Eine Liste wartender Änderungen, die gekürzt ist oder sich
/// nicht ganz lesen ließ, verwirft kein Aufräumen, auch mit ausdrücklicher
/// Wahl; ohne die Wahl oder für ein anderes Projekt stört sie nicht.
///
/// Mutationsbeweis: Ohne die Prüfung von `complete` in `refuse_incomplete`
/// wird der Test rot.
#[test]
fn an_incomplete_layer_is_never_discarded() {
    let project = humanitl_core::ProjectId::new();
    let live = |complete| LiveState {
        waiting: Some(WaitingLayer {
            project,
            layer: "layer".to_owned(),
            count: 200_000,
            bytes: 1,
            complete,
        }),
        ..LiveState::default()
    };
    let scope = |include_waiting, target| CleanupScope {
        target,
        before: None,
        include_waiting,
    };
    for target in [CleanupTarget::Projects(vec![project]), CleanupTarget::All] {
        let refused =
            refuse_incomplete(&scope(true, target.clone()), &live(false)).expect_err("incomplete");
        assert_eq!(refused.code.as_str(), "SANDBOX_054");
        assert!(refuse_incomplete(&scope(true, target.clone()), &live(true)).is_ok());
        assert!(refuse_incomplete(&scope(false, target), &live(false)).is_ok());
    }
    let other = CleanupTarget::Projects(vec![humanitl_core::ProjectId::new()]);
    assert!(refuse_incomplete(&scope(true, other), &live(false)).is_ok());
}

/// Review HUM-533: Nimmt das Audit-Log den Record nicht mehr an (Schreiber
/// beendet), ist die Löschung trotzdem geschehen, und die Antwort sagt es mit
/// `AUDIT_006` als Warnung.
#[tokio::test]
async fn a_lost_audit_record_is_named_in_the_answer() {
    let fixture = Fixture::new();
    let (_a, alpha, alpha_flow) = fixture.run_in("alpha").await;
    let planned = answer(
        fixture.context(None),
        plan_request(v1::CleanupSelection {
            project_ids: vec![alpha.id.to_string()],
            ..v1::CleanupSelection::default()
        }),
    )
    .await
    .unwrap()
    .plan
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let key = humanitl_audit::AuditKey::from_bytes([3; 32], humanitl_audit::KeyOrigin::File);
    let (writer, _) = humanitl_audit_log::AuditWriter::open(
        &dir.path().join("audit.jsonl"),
        &key,
        humanitl_audit_log::WriterOptions::default(),
        &[],
        None,
    )
    .unwrap();
    let handle = writer.handle();
    let _head = writer.stop("test");
    let mut ctx = fixture.context(None);
    ctx.audit = Some(handle);
    let answered = answer(ctx, run_request(&planned)).await.unwrap();
    assert!(!fixture.has_flow(alpha_flow).await);
    assert_eq!(answered.diagnostics.len(), 1, "{answered:?}");
    assert_eq!(answered.diagnostics[0].code, "AUDIT_006");
}
