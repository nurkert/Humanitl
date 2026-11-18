//! `Projects.cleanup_plan` and `Projects.cleanup` (HUM-533, ADR-0024 point
//! 14).
//!
//! The daemon lists every item it would delete, the person confirms, and the
//! client sends the digest of that plan back. The recording computes the
//! plan again inside the deleting transaction
//! ([`humanitl_recorder::Recorder::cleanup_blocking`]) and deletes only when
//! the digest is the same. Anything else is `SANDBOX_054`, and nothing is
//! deleted. The waiting layer of a disposable run is locked for the whole
//! run, so no `apply` or `discard` decides it in between.
//!
//! Clients never hold their own list of kinds or candidates (limit 2 of
//! sprint 8): kinds, sentences, warnings and the line of what is never
//! deleted all come from here.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use humanitl_audit::RecordKind;
use humanitl_audit::kinds::{CleanedKind, ProjectsCleaned};
use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes::{AUDIT_006, SANDBOX_054};
use humanitl_core::{Diagnostic, FixAction, ProjectId, Severity};
use humanitl_recorder::{
    CleanupItem, CleanupKind, CleanupOutcome, CleanupPlan, CleanupScope, CleanupTarget, Executed,
    Kept, KeptReason, LiveState, Recorder, RecorderError,
};

use super::{Context, blocking, find, lookup, parse_id, refused};
use crate::{convert, v1};

/// The fixed line every client shows with a plan.
pub const NOT_DELETED: &str = "Not deleted: the project folder, the audit log, rules, the global \
                               configuration and the agent depot (humanitl agents remove).";

/// The warning every client shows before it asks about waiting changes.
pub const WAITING_WARNING: &str = "Changes of the disposable run that were not applied are lost \
                                   for good.";

/// The longest age a selection takes, in days (100 years).
const MAX_DAYS: u32 = 36_500;

/// `cleanup_plan`: the plan of a selection. Deletes nothing.
///
/// # Errors
///
/// `CONFIG_003` for a selection that names not exactly one target or an
/// unknown project, `SANDBOX_054` for forgetting a project that is in use,
/// and the findings of the recording.
pub(super) async fn plan(
    ctx: &Context,
    recorder: &Recorder,
    asked: v1::projects_request::CleanupPlan,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let selection = asked.selection.unwrap_or_default();
    let scope = resolve(recorder, &selection, Some(SystemTime::now())).await?;
    let live = live_now(ctx).await?;
    refuse_incomplete(&scope, &live)?;
    let planned = recorder
        .cleanup_plan(scope.clone(), live)
        .await
        .map_err(RecorderError::into_diagnostic)?;
    if let CleanupTarget::Forget(id) = scope.target
        && planned.items.is_empty()
        && let Some(kept) = planned.kept.first()
    {
        return Err(in_use(id, kept));
    }
    Ok(v1::ProjectsResponse {
        plan: Some(view(&planned, &scope)),
        ..v1::ProjectsResponse::default()
    })
}

/// `cleanup`: runs a plan if its digest still holds.
///
/// # Errors
///
/// `CONFIG_003` for a selection or digest that is no plan's, `SANDBOX_054`
/// for a plan that changed or holds an item in use, and the findings of the
/// recording; in each case nothing is deleted.
pub(super) async fn run(
    ctx: &Context,
    recorder: &Recorder,
    asked: v1::projects_request::Cleanup,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let selection = asked.selection.unwrap_or_default();
    let scope = resolve(recorder, &selection, None).await?;
    let digest = asked.digest.trim().to_owned();
    if digest.is_empty() {
        return Err(refused(
            "cleanup needs the digest of a plan; ask for the plan first (cleanup_plan)".to_owned(),
        ));
    }
    let executed = execute(ctx, recorder, scope.clone(), digest).await?;
    let (done, layer) = match executed {
        (Executed::Stale(now), _) => return Err(stale(&now)),
        (Executed::Done { plan, outcomes }, layer) => ((plan, outcomes), layer),
    };
    let (plan, mut outcomes) = done;
    for (item, outcome) in plan.items.iter().zip(outcomes.iter_mut()) {
        if item.kind != CleanupKind::WaitingChanges {
            continue;
        }
        match &layer {
            Some(Ok(())) => {
                outcome.deleted = true;
                outcome.count = item.count;
                outcome.bytes = item.bytes;
            }
            Some(Err(found)) => outcome.reason = Some(found.why.clone()),
            None => {
                outcome.reason = Some("the waiting layer was no longer there".to_owned());
            }
        }
    }
    let unrecorded = record(ctx, &scope, &outcomes).await?;
    let paths = paths_of(&plan);
    let mut freed = 0_u64;
    let outcomes = outcomes
        .iter()
        .map(|outcome| {
            freed = freed.saturating_add(outcome.bytes);
            outcome_view(outcome, &paths)
        })
        .collect();
    Ok(v1::ProjectsResponse {
        cleaned: Some(v1::CleanupResult {
            outcomes,
            bytes_freed: freed,
            not_deleted: NOT_DELETED.to_owned(),
        }),
        diagnostics: unrecorded
            .iter()
            .map(convert::diagnostic_to_proto)
            .collect(),
        ..v1::ProjectsResponse::default()
    })
}

/// Runs the plan in the recording while the waiting layer is locked, and
/// discards the layer afterwards when the executed plan named it.
async fn execute(
    ctx: &Context,
    recorder: &Recorder,
    scope: CleanupScope,
    digest: String,
) -> Result<(Executed, Option<Result<(), Diagnostic>>), Diagnostic> {
    let recorder = recorder.clone();
    let Some(sandbox) = ctx.sandbox.clone() else {
        let live = live_of_activity(ctx);
        return blocking("Projects", move || {
            recorder
                .cleanup_blocking(scope, live, digest)
                .map(|executed| (executed, None))
        })
        .await?
        .map_err(RecorderError::into_diagnostic);
    };
    blocking("Projects", move || {
        let (executed, layer) = sandbox.with_cleanup_lock(|live| {
            // Unter der Sperre der Schicht noch einmal: Eine Liste, die
            // inzwischen unvollständig ist, verwirft nichts.
            if let Err(refused) = refuse_incomplete(&scope, live) {
                return (Err(refused), None);
            }
            let executed = recorder
                .cleanup_blocking(scope, live.clone(), digest)
                .map_err(RecorderError::into_diagnostic);
            let discard = match &executed {
                Ok(Executed::Done { plan, .. })
                    if plan
                        .items
                        .iter()
                        .any(|item| item.kind == CleanupKind::WaitingChanges) =>
                {
                    live.waiting.as_ref().map(|waiting| waiting.layer.clone())
                }
                _ => None,
            };
            (executed, discard)
        });
        executed.map(|executed| (executed, layer))
    })
    .await?
}

/// `SANDBOX_054`, wenn die Auswahl wartende Änderungen ausdrücklich
/// einschließt, deren Liste aber unvollständig ist: gekürzt, mit Stellen, die
/// sich nicht lesen ließen, oder gar nicht lesbar. Ein Aufräumen verwürfe
/// sonst die ganze Schicht samt Änderungen, die niemand gesehen hat.
pub(crate) fn refuse_incomplete(scope: &CleanupScope, live: &LiveState) -> Result<(), Diagnostic> {
    let Some(waiting) = live.waiting.as_ref() else {
        return Ok(());
    };
    let covered = match &scope.target {
        CleanupTarget::Projects(ids) => ids.contains(&waiting.project),
        CleanupTarget::All => true,
        CleanupTarget::Unassigned | CleanupTarget::Forget(_) => false,
    };
    if !scope.include_waiting || !covered || waiting.complete {
        return Ok(());
    }
    Err(Diagnostic::builder(SANDBOX_054, Severity::Error)
        .why(format!(
            "the waiting changes of the disposable run {} could not be listed completely (the \
             list is cut off or part of the layer could not be read), so a cleanup would discard \
             changes nobody saw; nothing was deleted. Look at them with humanitl sandbox changes \
             and decide them with apply or discard, or clean up without \
             --include-waiting-changes",
            waiting.layer
        ))
        .fix(FixAction::CopyCommand(
            "humanitl sandbox changes".to_owned(),
        ))
        .build())
}

/// What runs and waits now.
async fn live_now(ctx: &Context) -> Result<LiveState, Diagnostic> {
    match ctx.sandbox.clone() {
        Some(sandbox) => blocking("Projects", move || sandbox.cleanup_live()).await,
        None => Ok(live_of_activity(ctx)),
    }
}

/// Without a sandbox service: the activity names the running session; no
/// layer waits.
fn live_of_activity(ctx: &Context) -> LiveState {
    ctx.activity
        .as_ref()
        .filter(|activity| activity.running)
        .map_or_else(LiveState::default, |activity| LiveState {
            running_session: Some(activity.session),
            running_project: Some(activity.project),
            waiting: None,
        })
}

/// Reads a selection. With `now` (a plan) folders and an age in days are
/// resolved, and every named project must exist; without (a run) the
/// selection must be the one a plan returned, so that a project gone in
/// between makes the plan stale instead of unknown.
async fn resolve(
    recorder: &Recorder,
    selection: &v1::CleanupSelection,
    now: Option<SystemTime>,
) -> Result<CleanupScope, Diagnostic> {
    let named = !selection.project_ids.is_empty() || !selection.paths.is_empty();
    let forget = selection.forget.trim();
    let targets = [
        named,
        selection.all,
        selection.unassigned,
        !forget.is_empty(),
    ]
    .iter()
    .filter(|chosen| **chosen)
    .count();
    if targets != 1 {
        return Err(refused(
            "a cleanup selection names exactly one of: projects (ids or folders), all, \
             unassigned, forget"
                .to_owned(),
        ));
    }
    let target = if !forget.is_empty() {
        let id = parse_id(forget)?;
        if now.is_some() {
            lookup(recorder, forget).await?;
        }
        CleanupTarget::Forget(id)
    } else if selection.all {
        CleanupTarget::All
    } else if selection.unassigned {
        CleanupTarget::Unassigned
    } else {
        CleanupTarget::Projects(named_projects(recorder, selection, now.is_some()).await?)
    };
    let before = match (&selection.before, selection.older_than_days, now) {
        (Some(at), _, _) => Some(from_timestamp(at)?),
        (None, 0, _) => None,
        (None, days, Some(now)) => Some(days_before(now, days)),
        (None, _, None) => {
            return Err(refused(
                "cleanup takes the selection its plan returned, with the cutoff as a time, not \
                 an age in days"
                    .to_owned(),
            ));
        }
    };
    let forgetting = matches!(target, CleanupTarget::Forget(_));
    Ok(CleanupScope {
        target,
        before: before.filter(|_| !forgetting),
        include_waiting: selection.include_waiting_changes && !forgetting,
    })
}

async fn named_projects(
    recorder: &Recorder,
    selection: &v1::CleanupSelection,
    planning: bool,
) -> Result<Vec<ProjectId>, Diagnostic> {
    if !planning && !selection.paths.is_empty() {
        return Err(refused(
            "cleanup takes the selection its plan returned, with project ids instead of folders"
                .to_owned(),
        ));
    }
    let mut ids = Vec::new();
    for text in &selection.project_ids {
        let id = parse_id(text)?;
        if planning {
            lookup(recorder, text).await?;
        }
        ids.push(id);
    }
    for path in &selection.paths {
        let row = find(
            recorder,
            &v1::projects_request::Get {
                project_id: String::new(),
                path: path.clone(),
            },
        )
        .await?;
        ids.push(row.id);
    }
    let mut seen = std::collections::BTreeSet::new();
    ids.retain(|id| seen.insert(*id));
    Ok(ids)
}

/// `now` minus so many days, on a whole second, so that it travels through
/// a `Timestamp` and back without change.
fn days_before(now: SystemTime, days: u32) -> SystemTime {
    let back = Duration::from_secs(u64::from(days.min(MAX_DAYS)) * 24 * 60 * 60);
    let at = now.checked_sub(back).unwrap_or(UNIX_EPOCH).max(UNIX_EPOCH);
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn from_timestamp(at: &prost_types::Timestamp) -> Result<SystemTime, Diagnostic> {
    let seconds = u64::try_from(at.seconds)
        .map_err(|_negative| refused("the cutoff of a cleanup lies before 1970".to_owned()))?;
    let nanos = u32::try_from(at.nanos).unwrap_or(0);
    Ok(UNIX_EPOCH + Duration::new(seconds, nanos))
}

/// The selection as `cleanup` takes it back.
fn echo(scope: &CleanupScope) -> v1::CleanupSelection {
    let mut selection = v1::CleanupSelection {
        before: scope.before.map(convert::timestamp),
        include_waiting_changes: scope.include_waiting,
        ..v1::CleanupSelection::default()
    };
    match &scope.target {
        CleanupTarget::Projects(ids) => {
            selection.project_ids = ids.iter().map(ToString::to_string).collect();
        }
        CleanupTarget::All => selection.all = true,
        CleanupTarget::Unassigned => selection.unassigned = true,
        CleanupTarget::Forget(id) => selection.forget = id.to_string(),
    }
    selection
}

/// The path of each project a plan names, for the outcomes.
fn paths_of(plan: &CleanupPlan) -> BTreeMap<ProjectId, String> {
    plan.items
        .iter()
        .filter_map(|item| item.project.map(|id| (id, item.path.clone())))
        .collect()
}

fn view(plan: &CleanupPlan, scope: &CleanupScope) -> v1::CleanupPlanView {
    let mut kept: Vec<String> = Vec::new();
    for entry in &plan.kept {
        let line = kept_line(entry);
        if !kept.contains(&line) {
            kept.push(line);
        }
    }
    v1::CleanupPlanView {
        items: plan.items.iter().map(item_view).collect(),
        digest: plan.digest.clone(),
        not_deleted: NOT_DELETED.to_owned(),
        selection: Some(echo(scope)),
        kept,
        kinds: CleanupKind::ALL
            .iter()
            .map(|kind| kind.as_str().to_owned())
            .collect(),
    }
}

/// One item as it leaves the daemon, with its sentence and, for waiting
/// changes, the warning.
#[must_use]
pub(crate) fn item_view(item: &CleanupItem) -> v1::CleanupItem {
    let place = place(item);
    let description = match item.kind {
        CleanupKind::RecordedFlows => format!(
            "{} recorded request{} with responses and bodies from {} session{} of {place}",
            item.count,
            plural(item.count),
            item.sessions,
            plural(item.sessions),
        ),
        CleanupKind::WaitingChanges => format!(
            "{} change{} of the disposable run in {place} that wait for apply or discard",
            item.count,
            plural(item.count),
        ),
        CleanupKind::ProjectChoice => format!("The stored project choice of {place}"),
        CleanupKind::RegisterEntry => format!(
            "The register entry of {place}; its {} session{} stay without a project",
            item.sessions,
            plural(item.sessions),
        ),
    };
    v1::CleanupItem {
        kind: item.kind.as_str().to_owned(),
        project_id: item.project.map(|id| id.to_string()).unwrap_or_default(),
        path: sanitize_note(&item.path),
        description,
        bytes: item.bytes,
        count: item.count,
        warning: if item.kind == CleanupKind::WaitingChanges {
            WAITING_WARNING.to_owned()
        } else {
            String::new()
        },
    }
}

fn place(item: &CleanupItem) -> String {
    if item.project.is_none() {
        return "sessions without a project".to_owned();
    }
    format!(
        "{} ({})",
        sanitize_note(&item.name),
        sanitize_note(&item.path)
    )
}

const fn plural(count: u64) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn kept_line(kept: &Kept) -> String {
    let path = if kept.path.is_empty() {
        "sessions without a project".to_owned()
    } else {
        sanitize_note(&kept.path)
    };
    match (kept.kind, kept.reason) {
        (CleanupKind::RecordedFlows, _) => {
            format!("The running session in {path} and its requests stay.")
        }
        (CleanupKind::WaitingChanges, _) => {
            format!("The waiting changes in {path} stay; include them explicitly to discard them.")
        }
        (_, KeptReason::Running) => {
            format!("The project choice and register entry of {path} stay while its session runs.")
        }
        (_, KeptReason::ChangesWaiting) => format!(
            "The project choice and register entry of {path} stay while changes of a disposable \
             run wait."
        ),
    }
}

fn outcome_view(
    outcome: &CleanupOutcome,
    paths: &BTreeMap<ProjectId, String>,
) -> v1::CleanupOutcome {
    v1::CleanupOutcome {
        kind: outcome.kind.as_str().to_owned(),
        project_id: outcome.project.map(|id| id.to_string()).unwrap_or_default(),
        path: outcome
            .project
            .and_then(|id| paths.get(&id))
            .map(|path| sanitize_note(path))
            .unwrap_or_default(),
        deleted: outcome.deleted,
        count: outcome.count,
        bytes_freed: outcome.bytes,
        reason: outcome.reason.clone().unwrap_or_default(),
    }
}

/// One audit record per run, with kinds and amounts, without contents.
///
/// The deletion is committed already; the record follows it and the writer
/// is asked to confirm it (`AuditHandle::sync`). A writer that has ended
/// cannot take it: the answer then carries `AUDIT_006` as a warning, so the
/// person learns that this run is missing from the chain. Without an audit
/// log (a daemon or a test without one) nothing is recorded.
async fn record(
    ctx: &Context,
    scope: &CleanupScope,
    outcomes: &[CleanupOutcome],
) -> Result<Option<Diagnostic>, Diagnostic> {
    let Some(audit) = ctx.audit.clone() else {
        return Ok(None);
    };
    let mut kinds: BTreeMap<CleanupKind, (u64, u64)> = BTreeMap::new();
    for outcome in outcomes.iter().filter(|outcome| outcome.deleted) {
        let entry = kinds.entry(outcome.kind).or_default();
        entry.0 = entry.0.saturating_add(outcome.count);
        entry.1 = entry.1.saturating_add(outcome.bytes);
    }
    let selection = match scope.target {
        CleanupTarget::Projects(_) => "projects",
        CleanupTarget::All => "all",
        CleanupTarget::Unassigned => "unassigned",
        CleanupTarget::Forget(_) => "forget",
    };
    let record = RecordKind::ProjectsCleaned(ProjectsCleaned {
        selection: selection.to_owned(),
        kinds: kinds
            .into_iter()
            .map(|(kind, (count, bytes))| CleanedKind {
                kind: kind.as_str().to_owned(),
                count,
                bytes,
            })
            .collect(),
    });
    let confirmed = blocking("Projects", move || {
        audit.record(None, record);
        audit.sync().is_some()
    })
    .await?;
    Ok((!confirmed).then(|| {
        Diagnostic::builder(AUDIT_006, Severity::Warning)
            .why(
                "the cleanup ran, but the audit log did not take its record projects.cleaned: \
                 the audit writer has ended; the daemon log has the reason"
                    .to_owned(),
            )
            .fix(FixAction::CopyCommand("humanitl daemon restart".to_owned()))
            .build()
    }))
}

/// `SANDBOX_054` for forgetting a project that is in use.
fn in_use(id: ProjectId, kept: &Kept) -> Diagnostic {
    let why = match kept.reason {
        KeptReason::Running => "its session runs",
        KeptReason::ChangesWaiting => "changes of its disposable run wait for apply or discard",
    };
    Diagnostic::builder(SANDBOX_054, Severity::Error)
        .why(format!(
            "the project {id} cannot be forgotten while {why}; nothing was deleted"
        ))
        .fix(FixAction::CopyCommand("humanitl sandbox stop".to_owned()))
        .build()
}

/// `SANDBOX_054` for a plan that changed since it was shown.
fn stale(now: &CleanupPlan) -> Diagnostic {
    let in_use = if now.kept.is_empty() {
        String::new()
    } else {
        format!(
            "; in use now: {}",
            now.kept.iter().map(kept_line).collect::<Vec<_>>().join(" ")
        )
    };
    Diagnostic::builder(SANDBOX_054, Severity::Error)
        .why(format!(
            "the cleanup plan changed since it was shown (a new session or request, a changed \
             project choice, a session that started, or changes that wait now){in_use}; nothing \
             was deleted. Show the plan again and confirm the new one"
        ))
        .fix(FixAction::CopyCommand("humanitl projects clean".to_owned()))
        .build()
}

#[cfg(test)]
mod tests;
