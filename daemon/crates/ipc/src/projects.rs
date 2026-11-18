//! `Projects`: the project register (HUM-532, ADR-0024 points 1 and 2).
//!
//! Three operations:
//!
//! - `list`: a page of the current entries, the latest start first, each
//!   with path, name, last start, last agent, runs, whether the session of
//!   this daemon runs in it (and its id), whether changes of a disposable
//!   sandbox wait, and whether the folder is missing.
//! - `get`: one project by id (also a replaced row) or by folder, with every
//!   key of the project choice: what was chosen, what applies, and from which
//!   layer.
//! - `save_choice`: replaces the choice of a current entry. A key outside the
//!   allow-list, a locked value or a lowered `sandbox.min_isolation` is
//!   `CONFIG_024`, and nothing is saved.
//!
//! Two more delete, and only through a plan (HUM-533, limit 3 of sprint 8,
//! `cleanup.rs`):
//!
//! - `cleanup_plan`: the items a selection would delete, each with kind,
//!   project, one sentence, size and count, what stays because it is in use,
//!   the fixed line of what is never deleted, and a digest over the identity
//!   and version of every item.
//! - `cleanup`: runs exactly that plan. The recording computes the plan again
//!   in the deleting transaction and deletes only when the digest is the same;
//!   otherwise `SANDBOX_054` and nothing is deleted. Forgetting a project is
//!   a plan with the selection `forget` and runs the same way.
//!
//! Paths and names come from the file system and may hold control
//! characters; they leave the daemon through [`sanitize_note`], as in
//! `sessions summary`. The stored path is never computed back from a shown
//! one: lookups by folder canonicalize the folder itself.

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::{
    Paths, PolicyLocks, ProfileSelection, ProjectChoice, Resolved, check_choice,
};
use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes::{CONFIG_003, CONFIG_024, IPC_006, SANDBOX_035};
use humanitl_core::{Diagnostic, ProjectId, SessionId, Severity};
use humanitl_recorder::{ProjectRow, Recorder};

use crate::{convert, v1};

/// So viele Einträge hat eine Seite ohne Angabe.
pub const DEFAULT_LIMIT: u32 = 50;

/// Mehr hat eine Seite nie.
pub const MAX_LIMIT: u32 = 500;

/// A folder as the register recognises it: canonical path, device and
/// inode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIdentity {
    /// The canonical path (`realpath`).
    pub path: PathBuf,
    /// The device of the folder.
    pub dev: u64,
    /// The inode of the folder.
    pub ino: u64,
}

impl ProjectIdentity {
    /// Reads the identity of an existing folder.
    ///
    /// # Errors
    ///
    /// The error of `canonicalize` or `metadata`, or `NotADirectory`.
    pub fn of(dir: &Path) -> std::io::Result<Self> {
        let path = dir.canonicalize()?;
        let meta = std::fs::metadata(&path)?;
        if !meta.is_dir() {
            return Err(std::io::Error::from(std::io::ErrorKind::NotADirectory));
        }
        Ok(Self {
            path,
            dev: meta.dev(),
            ino: meta.ino(),
        })
    }

    /// The path as the register stores it.
    #[must_use]
    pub fn path_text(&self) -> String {
        self.path.display().to_string()
    }
}

/// What the sandbox service knows about the project of its latest start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectActivity {
    /// The project of the latest start.
    pub project: ProjectId,
    /// Whether its sandbox runs.
    pub running: bool,
    /// Whether changes of its disposable sandbox wait for a decision.
    pub changes_waiting: bool,
    /// The session of this daemon.
    pub session: SessionId,
}

/// What an answer needs from the daemon.
#[derive(Debug, Clone)]
pub struct Context {
    /// The recording that holds the register; `None` answers `IPC_006`.
    pub recorder: Option<Recorder>,
    /// The paths the configuration is resolved against.
    pub paths: Paths,
    /// The locks of a company policy over keys of the choice.
    pub locks: PolicyLocks,
    /// The project of the latest start, if any.
    pub activity: Option<ProjectActivity>,
    /// The sandbox service: what runs and which disposable layer waits, for
    /// a cleanup plan; `None` in a daemon without sandbox and in tests, then
    /// `activity` says what runs and no layer waits.
    pub sandbox: Option<crate::sandbox::SandboxService>,
    /// Where an executed cleanup is recorded (`projects.cleaned`).
    pub audit: Option<humanitl_audit_log::AuditHandle>,
}

/// Answers one request.
///
/// # Errors
///
/// `IPC_006` without a recording, `CONFIG_003` without an operation, for an
/// unreadable id or cursor and an unknown or replaced project, `CONFIG_024`
/// for a choice that may not be saved, and the findings of the recording.
pub async fn answer(
    ctx: Context,
    request: v1::ProjectsRequest,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let Some(recorder) = ctx.recorder.clone() else {
        return Err(Diagnostic::builder(IPC_006, Severity::Error)
            .why("this daemon runs without a recording, so it keeps no project register".to_owned())
            .build());
    };
    match request.op {
        Some(v1::projects_request::Op::List(list)) => page(&ctx, &recorder, &list).await,
        Some(v1::projects_request::Op::Get(get)) => {
            let row = find(&recorder, &get).await?;
            detail(ctx, row).await
        }
        Some(v1::projects_request::Op::SaveChoice(save)) => save_choice(ctx, &recorder, save).await,
        Some(v1::projects_request::Op::CleanupPlan(asked)) => {
            cleanup::plan(&ctx, &recorder, asked).await
        }
        Some(v1::projects_request::Op::Cleanup(asked)) => {
            cleanup::run(&ctx, &recorder, asked).await
        }
        None => Err(refused("the request names no operation".to_owned())),
    }
}

/// The current row of a project id, for `GetConfig.project_id`.
///
/// # Errors
///
/// As [`answer`] for an unknown id.
pub async fn lookup(recorder: &Recorder, project_id: &str) -> Result<ProjectRow, Diagnostic> {
    let id = parse_id(project_id)?;
    recorder
        .get_project(id)
        .await
        .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        .ok_or_else(|| unknown(project_id))
}

/// The configuration of a project: its folder chooses the project profile,
/// and its stored choice is the layer `project-choice`. A choice that does not
/// load leaves the resolution without it and comes back as the finding.
///
/// Blocking: reads files.
pub fn resolve_for(
    paths: &Paths,
    locks: &PolicyLocks,
    row: &ProjectRow,
) -> (Result<Resolved, Diagnostic>, Option<Diagnostic>) {
    let without = resolve_at(paths, locks, &row.path, None);
    let choice = match row.choice.as_deref().map(ProjectChoice::from_json) {
        None => return (without, None),
        Some(Err(found)) => return (without, Some(found)),
        Some(Ok(choice)) => choice,
    };
    let with = without
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|_below| check_for(paths, locks, &row.path, &choice));
    match with {
        Ok(resolved) => (Ok(resolved), None),
        Err(found) => (without, Some(found)),
    }
}

/// Prüft eine Projektwahl für den Ordner `path`, beim Speichern und beim
/// Laden für einen Start dieselben Schritte, und liefert die Auflösung mit
/// ihr:
///
/// 1. gegen die Auflösung dieses Ordners ohne Wahl und gegen die Sperren
///    einer Firmenvorgabe ([`check_choice`]),
/// 2. die Auflösung mit der Wahl (Typen, Untergrenze beim Mischen),
/// 3. das Backend, das danach gälte, gegen die Untergrenze der Isolation:
///    Eine Wahl, die ein Backend unter der Untergrenze nennt oder die
///    Untergrenze über das gewählte Backend hebt, ist `CONFIG_024`, nicht
///    erst beim Start `SANDBOX_035`. Die Stufe eines Backends kennt nur die
///    Sandbox-Crate ([`humanitl_sandbox::choose_backend`]); deshalb steht
///    dieser Schritt hier und nicht in `humanitl_config`.
///
/// Blockierend: liest Dateien.
///
/// # Errors
///
/// `CONFIG_024` für eine Wahl, die nicht gilt, und die Befunde des Ladens.
pub fn check_for(
    paths: &Paths,
    locks: &PolicyLocks,
    path: &str,
    choice: &ProjectChoice,
) -> Result<Resolved, Diagnostic> {
    let below = resolve_at(paths, locks, path, None)?;
    check_choice(choice, &below.config, locks)?;
    let with = resolve_at(paths, locks, path, Some(choice))?;
    let touches_level =
        choice.get("sandbox.backend").is_some() || choice.get("sandbox.min_isolation").is_some();
    if touches_level
        && let Err(found) =
            humanitl_sandbox::choose_backend(&with.config.sandbox, humanitl_sandbox::AUTO_BACKEND)
        && found.code == SANDBOX_035
    {
        return Err(Diagnostic::builder(CONFIG_024, Severity::Error)
            .why(format!(
                "the project choice leaves a backend below the isolation floor: {}",
                found.why
            ))
            .build());
    }
    Ok(with)
}

async fn page(
    ctx: &Context,
    recorder: &Recorder,
    list: &v1::projects_request::List,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let limit = match list.limit {
        0 => DEFAULT_LIMIT,
        asked => asked.min(MAX_LIMIT),
    };
    let offset = match list.cursor.trim() {
        "" => 0,
        cursor => cursor
            .parse::<u32>()
            .map_err(|_err| refused(format!("{cursor:?} is no cursor of a project list")))?,
    };
    let mut rows = recorder
        .list_projects(limit.saturating_add(1), offset)
        .await
        .map_err(humanitl_recorder::RecorderError::into_diagnostic)?;
    let more = rows.len() > usize::try_from(limit).unwrap_or(usize::MAX);
    rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    let activity = ctx.activity.clone();
    let projects = blocking("Projects", move || {
        rows.iter()
            .map(|row| entry(row, activity.as_ref()))
            .collect::<Vec<_>>()
    })
    .await?;
    Ok(v1::ProjectsResponse {
        projects,
        next_cursor: if more {
            offset.saturating_add(limit).to_string()
        } else {
            String::new()
        },
        ..v1::ProjectsResponse::default()
    })
}

pub(crate) async fn find(
    recorder: &Recorder,
    get: &v1::projects_request::Get,
) -> Result<ProjectRow, Diagnostic> {
    if !get.project_id.trim().is_empty() {
        return lookup(recorder, &get.project_id).await;
    }
    let asked = get.path.trim();
    if asked.is_empty() || !Path::new(asked).is_absolute() {
        return Err(refused(
            "a project is named by its id or by an absolute folder".to_owned(),
        ));
    }
    let dir = PathBuf::from(asked);
    // Ein fehlender Ordner bleibt im Register; gesucht wird er dann so, wie
    // er geschrieben ist.
    let (path, identity) = match ProjectIdentity::of(&dir) {
        Ok(found) => (found.path_text(), Some((found.dev, found.ino))),
        Err(_missing) => (asked.to_owned(), None),
    };
    recorder
        .current_project_at(path, identity)
        .await
        .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        .ok_or_else(|| unknown(asked))
}

async fn save_choice(
    ctx: Context,
    recorder: &Recorder,
    save: v1::projects_request::SaveChoice,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let row = lookup(recorder, &save.project_id).await?;
    if row.replaced_at.is_some() {
        return Err(refused(format!(
            "the project {} was replaced by another folder at the same path; its choice stays as \
             history and is not changed",
            row.id
        )));
    }
    let choice = ProjectChoice::from_pairs(
        save.values
            .into_iter()
            .map(|value| (value.key.trim().to_owned(), value.value)),
    )?;
    let (paths, locks, path) = (ctx.paths.clone(), ctx.locks.clone(), row.path.clone());
    let checked = choice.clone();
    blocking("Projects", move || {
        check_for(&paths, &locks, &path, &checked).map(|_resolved| ())
    })
    .await??;
    let stored = (!choice.is_empty()).then(|| choice.to_json());
    let saved = recorder
        .save_project_choice(row.id, stored)
        .await
        .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        .ok_or_else(|| unknown(&row.id.to_string()))?;
    detail(ctx, saved).await
}

async fn detail(ctx: Context, row: ProjectRow) -> Result<v1::ProjectsResponse, Diagnostic> {
    blocking("Projects", move || {
        let (resolved, found) = resolve_for(&ctx.paths, &ctx.locks, &row);
        let chosen = row
            .choice
            .as_deref()
            .and_then(|text| ProjectChoice::from_json(text).ok())
            .unwrap_or_default();
        let mut diagnostics: Vec<v1::Diagnostic> =
            found.iter().map(convert::diagnostic_to_proto).collect();
        let config = match &resolved {
            Ok(resolved) => Some(resolved),
            Err(failed) => {
                diagnostics.push(convert::diagnostic_to_proto(failed));
                None
            }
        };
        v1::ProjectsResponse {
            project: Some(entry(&row, ctx.activity.as_ref())),
            choice: choice_fields(&chosen, config),
            diagnostics,
            ..v1::ProjectsResponse::default()
        }
    })
    .await
}

/// Every key of the allow-list with what was chosen, what applies and from
/// which layer.
pub(crate) fn choice_fields(
    chosen: &ProjectChoice,
    resolved: Option<&Resolved>,
) -> Vec<v1::ProjectChoiceField> {
    let values = resolved.and_then(|resolved| serde_json::to_value(&resolved.config).ok());
    humanitl_config::PROJECT_CHOICE_KEYS
        .iter()
        .map(|key| v1::ProjectChoiceField {
            key: (*key).to_owned(),
            chosen: chosen.get(key).map(sanitize_note).unwrap_or_default(),
            effective: values
                .as_ref()
                .map(|values| sanitize_note(&leaf_text(values, key)))
                .unwrap_or_default(),
            origin: resolved
                .and_then(|resolved| resolved.origin(key))
                .map(|origin| origin.kind().to_owned())
                .unwrap_or_default(),
        })
        .collect()
}

/// The value of a dotted key as text: a string as it is, nothing as empty,
/// anything else in its JSON form.
fn leaf_text(values: &serde_json::Value, key: &str) -> String {
    let pointer = format!("/{}", key.replace('.', "/"));
    match values.pointer(&pointer) {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

/// One entry of the register as it leaves the daemon.
///
/// Blocking: looks whether the folder is there.
#[must_use]
pub fn entry(row: &ProjectRow, activity: Option<&ProjectActivity>) -> v1::ProjectEntry {
    let active = activity.filter(|activity| activity.project == row.id);
    let running = active.is_some_and(|activity| activity.running);
    v1::ProjectEntry {
        project_id: row.id.to_string(),
        path: sanitize_note(&row.path),
        display_name: sanitize_note(&row.display_name),
        first_seen: Some(convert::timestamp(row.first_seen)),
        last_started: row.last_started.map(convert::timestamp),
        last_agent: sanitize_note(row.last_agent.as_deref().unwrap_or_default()),
        runs: row.runs,
        running,
        session_id: active
            .filter(|_| running)
            .map(|activity| activity.session.to_string())
            .unwrap_or_default(),
        changes_waiting: active.is_some_and(|activity| activity.changes_waiting),
        missing: !Path::new(&row.path).is_dir(),
        replaced: row.replaced_at.is_some(),
        has_choice: row.choice.is_some(),
        choice_saved_at: row.choice_saved_at.map(convert::timestamp),
        version: row.version,
    }
}

/// Resolves the configuration of a project folder without a wish: the folder
/// as `sandbox.work_dir` on the command line level (only while it exists),
/// so that its project profile applies even when `config.toml` names
/// another folder. The locks of a company policy are the top layer
/// (HUM-534).
fn resolve_at(
    paths: &Paths,
    locks: &PolicyLocks,
    path: &str,
    choice: Option<&ProjectChoice>,
) -> Result<Resolved, Diagnostic> {
    let dir = Path::new(path);
    let present = dir.is_dir();
    let pairs: Vec<(String, String)> = if present {
        vec![("sandbox.work_dir".to_owned(), path.to_owned())]
    } else {
        Vec::new()
    };
    let sources = humanitl_config::sources_for(
        &ProfileSelection::default(),
        present.then_some(dir),
        paths.env(),
        &pairs,
    )?
    .with_project_choice(choice.cloned())
    .with_policy(locks.clone());
    humanitl_config::load(&sources)
}

/// What `GetConfigRequest.project_id` names (HUM-534, specification 2a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigTarget {
    /// No project, or one the register does not know: the global resolution.
    Global,
    /// A project of the register, with its choice.
    Project(ProjectRow),
    /// A folder the register does not know: resolved as a start there would,
    /// with its project profile and without a choice. The canonical path.
    Folder(String),
}

/// Reads `GetConfigRequest.project_id`: a project id of the register or an
/// absolute folder. Neither an unknown id nor an unknown folder is an error;
/// they resolve without a project choice.
///
/// # Errors
///
/// The findings of the recording.
pub async fn config_target(
    recorder: Option<&Recorder>,
    asked: &str,
) -> Result<ConfigTarget, Diagnostic> {
    let asked = asked.trim();
    if asked.is_empty() {
        return Ok(ConfigTarget::Global);
    }
    if Path::new(asked).is_absolute() {
        let Ok(identity) = ProjectIdentity::of(Path::new(asked)) else {
            return Ok(ConfigTarget::Global);
        };
        if let Some(recorder) = recorder
            && let Some(row) = recorder
                .current_project_at(identity.path_text(), Some((identity.dev, identity.ino)))
                .await
                .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        {
            return Ok(ConfigTarget::Project(row));
        }
        return Ok(ConfigTarget::Folder(identity.path_text()));
    }
    let (Some(recorder), Ok(id)) = (recorder, parse_id(asked)) else {
        return Ok(ConfigTarget::Global);
    };
    Ok(recorder
        .get_project(id)
        .await
        .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        .map_or(ConfigTarget::Global, ConfigTarget::Project))
}

/// The configuration of a folder the register does not know, without a
/// choice (HUM-534). Blocking: reads files.
///
/// # Errors
///
/// The findings of loading the configuration.
pub fn resolve_folder(
    paths: &Paths,
    locks: &PolicyLocks,
    path: &str,
) -> Result<Resolved, Diagnostic> {
    resolve_at(paths, locks, path, None)
}

/// Runs blocking work off the runtime threads.
pub(crate) async fn blocking<T: Send + 'static>(
    rpc: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Diagnostic> {
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        Diagnostic::builder(IPC_006, Severity::Error)
            .why(format!(
                "{rpc} did not finish: its worker thread ended early ({error}); read the state \
                 again or repeat the call"
            ))
            .build()
    })
}

pub(crate) fn parse_id(text: &str) -> Result<ProjectId, Diagnostic> {
    ProjectId::parse(text.trim())
        .map_err(|_err| refused(format!("{:?} is no project id", text.trim())))
}

pub(crate) fn unknown(what: &str) -> Diagnostic {
    refused(format!(
        "no project {what:?} is known to this daemon; humanitl projects ls lists them"
    ))
}

pub(crate) fn refused(why: String) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(why)
        .build()
}

mod cleanup;

pub use cleanup::{NOT_DELETED, WAITING_WARNING};

#[cfg(test)]
mod tests;
