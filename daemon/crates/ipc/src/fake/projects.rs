//! `Projects` im Fake (HUM-532).
//!
//! Der Fake hat kein Register. Er kennt genau ein Projekt: den Ordner seiner
//! aufgezeichneten Sitzung, mit der Kennung der Sitzung als Projekt-`id`,
//! einem Lauf und laufender Sitzung. `get` nennt die Schlüssel der
//! Projektwahl mit ihren Vorgabewerten; `save_choice` schreibt nirgends hin
//! und antwortet `IPC_006`, wie der Fake bei jeder anderen Änderung, die er
//! nicht halten kann. `proposal` (HUM-536) baut die Felder mit derselben
//! Funktion wie der Daemon, aus den Vorgaben der Konfiguration, mit jedem
//! Backend und jedem Agenten vom Host verfügbar und ohne Depot.

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
        // HUM-533: Das eine Projekt des Fakes läuft; ein Plan nennt deshalb
        // nichts zum Löschen, nur was bleibt, und ausführen kann der Fake
        // nichts.
        Some(v1::projects_request::Op::CleanupPlan(_)) => Ok(v1::ProjectsResponse {
            plan: Some(v1::CleanupPlanView {
                not_deleted: crate::projects::NOT_DELETED.to_owned(),
                kept: project
                    .iter()
                    .map(|project| {
                        format!(
                            "The running session in {} and its requests stay.",
                            project.path
                        )
                    })
                    .collect(),
                kinds: humanitl_recorder::CleanupKind::ALL
                    .iter()
                    .map(|kind| kind.as_str().to_owned())
                    .collect(),
                ..v1::CleanupPlanView::default()
            }),
            ..v1::ProjectsResponse::default()
        }),
        Some(v1::projects_request::Op::Cleanup(_)) => {
            Err(Diagnostic::builder(codes::IPC_006, Severity::Error)
                .why("the fake daemon keeps no recording to clean up".to_owned())
                .build())
        }
        Some(v1::projects_request::Op::Proposal(asked)) => proposal(project, &asked),
        // Der Fake beantwortet die Bitte an die App vor dem Register, wie der
        // Daemon (HUM-539).
        Some(v1::projects_request::Op::OpenInApp(_)) => Err(refused(
            "open_in_app goes to the connected app, not to the register",
        )),
        None => Err(refused("the request names no operation")),
    }
}

/// Der Startvorschlag des Fakes für einen absoluten Ordner oder das Projekt
/// der Sitzung, mit der Teilwahl der Anfrage.
fn proposal(
    project: Option<v1::ProjectEntry>,
    asked: &v1::projects_request::Proposal,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    use crate::projects::proposal::{AgentFacts, Inputs, ServerFacts};

    let known = project.filter(|project| {
        asked.project_id == project.project_id
            || (asked.project_id.is_empty() && asked.path == project.path)
    });
    let path = match &known {
        Some(project) => project.path.clone(),
        None if asked.project_id.is_empty() && asked.path.starts_with('/') => asked.path.clone(),
        None => {
            return Err(refused(
                "the fake daemon proposes for an absolute folder or the project of its session",
            ));
        }
    };
    let choice = crate::projects::proposal::merged(None, &asked.values)?;
    let resolved = humanitl_config::load(&Sources::empty().with_project_choice(Some(choice)))?;
    let config = &resolved.config;
    let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
        "XDG_DATA_HOME",
        "/nonexistent-humanitl-fake",
    )]));
    let agents = humanitl_sandbox::AdapterRegistry::builtin()
        .ids()
        .into_iter()
        .map(|id| {
            let mut with_agent = config.clone();
            id.clone_into(&mut with_agent.agent.adapter);
            AgentFacts {
                id: id.to_owned(),
                display_name: humanitl_sandbox::agent::depot::recipe::known(id)
                    .map_or(id, |agent| agent.display_name)
                    .to_owned(),
                source: humanitl_config::agents::AgentSource::Host,
                host: Ok(()),
                depot: Err(Diagnostic::builder(codes::AGENT_008, Severity::Error)
                    .why("the fake daemon has no agent depot".to_owned())
                    .build()),
                cloud: crate::cloud::status(&paths, &with_agent),
            }
        })
        .collect();
    let backends = humanitl_sandbox::list_backends_with(
        &config.sandbox,
        humanitl_sandbox::AUTO_BACKEND,
        |_| None,
    );
    let built = crate::projects::proposal::build(&Inputs {
        resolved: &resolved,
        locks: &humanitl_config::PolicyLocks::none(),
        plan: crate::projects::proposal::assumed_plan(config),
        backends: &backends,
        agents,
        servers: config
            .llm
            .servers
            .keys()
            .map(|name| ServerFacts {
                name: name.clone(),
                cloud: None,
                refusal: None,
            })
            .collect(),
        profiles: vec![crate::projects::proposal::ProfileFacts {
            name: config.sandbox.profile.clone(),
            state: Ok(()),
        }],
        cloud: crate::cloud::status(&paths, config),
    });
    let mut proposal = crate::projects::proposal::proposal(None, &path, built);
    if let Some(project) = known {
        proposal.project_id = project.project_id;
        proposal.display_name = project.display_name;
    }
    Ok(v1::ProjectsResponse {
        proposal: Some(proposal),
        ..v1::ProjectsResponse::default()
    })
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
