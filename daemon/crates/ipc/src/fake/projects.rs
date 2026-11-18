//! `Projects` im Fake (HUM-532).
//!
//! Der Fake hat kein Register. Er kennt genau ein Projekt: den Ordner seiner
//! aufgezeichneten Sitzung, mit der Kennung der Sitzung als Projekt-`id`,
//! einem Lauf und laufender Sitzung. `get` nennt die Schlüssel der
//! Projektwahl mit ihren Vorgabewerten; `save_choice` schreibt nirgends hin
//! und antwortet `IPC_006`, wie der Fake bei jeder anderen Änderung, die er
//! nicht halten kann.

use humanitl_config::{ProjectChoice, Sources};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};

use super::state::SessionMeta;
use crate::v1;

/// Beantwortet eine Anfrage an das Register des Fakes.
///
/// # Errors
///
/// `CONFIG_003` ohne Operation und für ein Projekt, das nicht das der
/// Sitzung ist, `IPC_006` für `save_choice`.
pub(super) fn answer(
    session: &SessionMeta,
    request: v1::ProjectsRequest,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let project = entry(session);
    match request.op {
        Some(v1::projects_request::Op::List(_)) => Ok(v1::ProjectsResponse {
            projects: project.into_iter().collect(),
            ..v1::ProjectsResponse::default()
        }),
        Some(v1::projects_request::Op::Get(get)) => {
            let Some(project) = project.filter(|project| {
                get.project_id == project.project_id
                    || (get.project_id.is_empty() && get.path == project.path)
            }) else {
                return Err(refused(
                    "the fake daemon knows only the project of its session",
                ));
            };
            let defaults = humanitl_config::load(&Sources::empty()).ok();
            Ok(v1::ProjectsResponse {
                project: Some(project),
                choice: crate::projects::choice_fields(&ProjectChoice::new(), defaults.as_ref()),
                ..v1::ProjectsResponse::default()
            })
        }
        Some(v1::projects_request::Op::SaveChoice(_)) => {
            Err(Diagnostic::builder(codes::IPC_006, Severity::Error)
                .why("the fake daemon keeps no project register to write to".to_owned())
                .build())
        }
        None => Err(refused("the request names no operation")),
    }
}

/// Das eine Projekt des Fakes; keines, wenn die Sitzung keinen Ordner nennt.
fn entry(session: &SessionMeta) -> Option<v1::ProjectEntry> {
    let path = session.work_dir.trim();
    if path.is_empty() {
        return None;
    }
    let name = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned();
    Some(v1::ProjectEntry {
        project_id: session.id.to_string(),
        path: path.to_owned(),
        display_name: name,
        last_agent: "opencode".to_owned(),
        runs: 1,
        running: true,
        session_id: session.id.to_string(),
        version: 1,
        ..v1::ProjectEntry::default()
    })
}

fn refused(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::CONFIG_003, Severity::Error)
        .why(why.to_owned())
        .build()
}
