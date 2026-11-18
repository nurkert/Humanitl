//! `humanitl projects ls|show` (HUM-532, ADR-0024).
//!
//! Ein gRPC-Aufruf je Befehl und sonst nichts (ADR-018): welche Projekte es
//! gibt, wann und mit welchem Agenten sie zuletzt liefen und was für eines
//! gewählt ist, weiß der Daemon in seinem Register. `projects forget` kommt
//! mit dem Aufräumen (HUM-533) und löscht nur, was der Daemon vorher
//! aufgelistet hat.

use chrono::{DateTime, Local};
use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cli::ProjectsCmd;
use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};

/// So viele Seiten liest `ls` höchstens; mehr als 50 000 Projekte kennt
/// niemand.
const MAX_PAGES: usize = 100;

/// Führt `humanitl projects <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; der Befund des Daemons, wenn er
/// ablehnt (`CONFIG_003` für einen Ordner, den das Register nicht kennt,
/// `IPC_006` für einen Daemon ohne Register).
pub async fn run(ctx: &Context, cmd: &ProjectsCmd) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    match cmd {
        ProjectsCmd::Ls => {
            let mut projects = Vec::new();
            let mut cursor = String::new();
            for _ in 0..MAX_PAGES {
                let page = client
                    .projects(v1::ProjectsRequest {
                        op: Some(v1::projects_request::Op::List(v1::projects_request::List {
                            limit: 0,
                            cursor: cursor.clone(),
                        })),
                    })
                    .await
                    .map_err(|status| Failure::new(status_diagnostic(&status, "Projects")))?
                    .into_inner();
                projects.extend(page.projects);
                if page.next_cursor.is_empty() {
                    break;
                }
                cursor = page.next_cursor;
            }
            print_list(ctx, &projects);
        }
        ProjectsCmd::Show { path } => {
            let absolute = if path.is_absolute() {
                path.clone()
            } else {
                ctx.cwd.join(path)
            };
            let shown = client
                .projects(v1::ProjectsRequest {
                    op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                        project_id: String::new(),
                        path: absolute.display().to_string(),
                    })),
                })
                .await
                .map_err(|status| Failure::new(status_diagnostic(&status, "Projects")))?
                .into_inner();
            print_detail(ctx, &shown);
        }
    }
    Ok(EXIT_OK)
}

fn print_list(ctx: &Context, projects: &[v1::ProjectEntry]) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "projects": projects.iter().map(entry_json).collect::<Vec<_>>(),
        }));
        return;
    }
    if projects.is_empty() {
        ctx.render
            .line("no projects yet; a start in a folder registers it");
        return;
    }
    for project in projects {
        ctx.render.line(&line(project));
    }
}

fn print_detail(ctx: &Context, shown: &v1::ProjectsResponse) {
    let diagnostics: Vec<_> = shown.diagnostics.iter().filter_map(from_proto).collect();
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "project": shown.project.as_ref().map(entry_json),
            "choice": shown.choice.iter().map(|field| json!({
                "key": field.key,
                "chosen": field.chosen,
                "effective": field.effective,
                "origin": field.origin,
            })).collect::<Vec<_>>(),
            "diagnostics": diagnostics
                .iter()
                .map(crate::render::diagnostic_json)
                .collect::<Vec<_>>(),
        }));
        return;
    }
    if let Some(project) = shown.project.as_ref() {
        ctx.render.line(&line(project));
        ctx.render.line(&format!("  id {}", project.project_id));
    }
    for field in &shown.choice {
        let chosen = if field.chosen.is_empty() {
            String::new()
        } else {
            format!(" (chosen {})", field.chosen)
        };
        ctx.render.line(&format!(
            "  {} = {}{chosen} [{}]",
            field.key, field.effective, field.origin
        ));
    }
    for diagnostic in &diagnostics {
        ctx.render.diagnostic(diagnostic);
    }
}

/// Eine Zeile je Projekt: Name, Ordner, Agent, letzter Start, Läufe und
/// was gerade gilt.
fn line(project: &v1::ProjectEntry) -> String {
    let mut marks = Vec::new();
    if project.running {
        marks.push("running".to_owned());
    }
    if project.changes_waiting {
        marks.push("changes waiting".to_owned());
    }
    if project.missing {
        marks.push("folder missing".to_owned());
    }
    if project.replaced {
        marks.push("replaced".to_owned());
    }
    let marks = if marks.is_empty() {
        String::new()
    } else {
        format!(", {}", marks.join(", "))
    };
    let agent = if project.last_agent.is_empty() {
        "-"
    } else {
        project.last_agent.as_str()
    };
    format!(
        "{} ({}): {agent}, last start {}, {} run{}{marks}",
        project.display_name,
        project.path,
        project
            .last_started
            .as_ref()
            .map_or_else(|| "-".to_owned(), |at| local_time(at.seconds)),
        project.runs,
        if project.runs == 1 { "" } else { "s" },
    )
}

fn entry_json(project: &v1::ProjectEntry) -> Value {
    json!({
        "id": project.project_id,
        "path": project.path,
        "name": project.display_name,
        "first_seen": project.first_seen.as_ref().map(|at| at.seconds),
        "last_started": project.last_started.as_ref().map(|at| at.seconds),
        "last_agent": project.last_agent,
        "runs": project.runs,
        "running": project.running,
        "session_id": project.session_id,
        "changes_waiting": project.changes_waiting,
        "missing": project.missing,
        "replaced": project.replaced,
        "has_choice": project.has_choice,
        "version": project.version,
    })
}

/// Ein Zeitpunkt als lokale Zeit, oder `-`.
fn local_time(seconds: i64) -> String {
    DateTime::from_timestamp(seconds, 0).map_or_else(
        || "-".to_owned(),
        |time| {
            DateTime::<Local>::from(time)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_line_names_agent_runs_and_state() {
        let project = v1::ProjectEntry {
            display_name: "humanitl".to_owned(),
            path: "/home/u/humanitl".to_owned(),
            last_agent: "claude".to_owned(),
            runs: 3,
            running: true,
            missing: false,
            ..v1::ProjectEntry::default()
        };
        assert_eq!(
            line(&project),
            "humanitl (/home/u/humanitl): claude, last start -, 3 runs, running"
        );
    }
}
