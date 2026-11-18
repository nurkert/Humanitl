//! `humanitl projects ls|show|clean|forget` (HUM-532, HUM-533, ADR-0024).
//!
//! Ein gRPC-Aufruf je Schritt und sonst nichts (ADR-018): welche Projekte es
//! gibt, wann und mit welchem Agenten sie zuletzt liefen und was für eines
//! gewählt ist, weiß der Daemon in seinem Register. `clean` und `forget`
//! löschen nur, was der Daemon vorher als Plan aufgelistet hat: Die CLI zeigt
//! den Plan, wie er kommt (Arten, Sätze, Warnungen, die Zeile, was nie
//! gelöscht wird), fragt, und schickt den Digest dieses Plans zurück. Eine
//! eigene Liste von Arten oder Kandidaten hält sie nicht.

use std::io::{BufRead as _, IsTerminal as _};

use chrono::{DateTime, Local};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cli::ProjectsCmd;
use humanitl_ipc::client::Client;

use crate::cmd::{Context, EXIT_OK, EXIT_USER, Failure, from_proto, status_diagnostic};

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
            let shown = get(&mut client, &absolute(ctx, path)).await?;
            print_detail(ctx, &shown);
        }
        ProjectsCmd::Clean {
            paths,
            older_than,
            include_waiting_changes,
            unassigned,
            yes,
        } => {
            let selection = v1::CleanupSelection {
                paths: paths.iter().map(|path| absolute(ctx, path)).collect(),
                all: paths.is_empty() && !unassigned,
                unassigned: *unassigned,
                older_than_days: older_than.unwrap_or(0),
                include_waiting_changes: *include_waiting_changes,
                ..v1::CleanupSelection::default()
            };
            return clean(ctx, &mut client, selection, *yes).await;
        }
        ProjectsCmd::Forget { path, yes } => {
            let shown = get(&mut client, &absolute(ctx, path)).await?;
            let selection = v1::CleanupSelection {
                forget: shown
                    .project
                    .map(|project| project.project_id)
                    .unwrap_or_default(),
                ..v1::CleanupSelection::default()
            };
            return clean(ctx, &mut client, selection, *yes).await;
        }
    }
    Ok(EXIT_OK)
}

/// Ein Ordner als absoluter Pfad, relativ zum Arbeitsverzeichnis.
fn absolute(ctx: &Context, path: &std::path::Path) -> String {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        ctx.cwd.join(path)
    };
    path.display().to_string()
}

async fn get(client: &mut Client, path: &str) -> Result<v1::ProjectsResponse, Failure> {
    projects(
        client,
        v1::projects_request::Op::Get(v1::projects_request::Get {
            project_id: String::new(),
            path: path.to_owned(),
        }),
    )
    .await
}

async fn projects(
    client: &mut Client,
    op: v1::projects_request::Op,
) -> Result<v1::ProjectsResponse, Failure> {
    client
        .projects(v1::ProjectsRequest { op: Some(op) })
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "Projects")))
}

/// Plan holen und zeigen, fragen, und genau diesen Plan ausführen.
///
/// Ohne Terminal und ohne `--yes` löscht nichts: Der Plan steht auf
/// `stdout`, und der Lauf endet mit Exit 1.
async fn clean(
    ctx: &Context,
    client: &mut Client,
    selection: v1::CleanupSelection,
    yes: bool,
) -> Result<u8, Failure> {
    let plan = projects(
        client,
        v1::projects_request::Op::CleanupPlan(v1::projects_request::CleanupPlan {
            selection: Some(selection),
        }),
    )
    .await?
    .plan
    .unwrap_or_default();
    print_plan(ctx, &plan);
    if plan.items.is_empty() {
        ctx.render
            .value(&json!({ "plan": plan_json(&plan), "deleted": false }));
        ctx.render.line("Nothing to clean up.");
        return Ok(EXIT_OK);
    }
    if !yes {
        if ctx.render.is_json() || !std::io::stdin().is_terminal() {
            let why = "nothing was deleted: there is no terminal to ask on; run the same command \
                       with --yes to delete what the daemon then lists";
            ctx.render.value(&json!({
                "plan": plan_json(&plan),
                "deleted": false,
                "reason": why,
            }));
            ctx.render.note(why);
            return Ok(EXIT_USER);
        }
        confirm(&plan)?;
    }
    let cleaned = projects(
        client,
        v1::projects_request::Op::Cleanup(v1::projects_request::Cleanup {
            selection: plan.selection.clone(),
            digest: plan.digest.clone(),
        }),
    )
    .await?
    .cleaned
    .unwrap_or_default();
    print_result(ctx, &plan, &cleaned);
    Ok(EXIT_OK)
}

/// Fragt auf dem Terminal; die Warnungen standen schon über der Frage.
fn confirm(plan: &v1::CleanupPlanView) -> Result<(), Failure> {
    use std::io::Write as _;

    let mut stderr = std::io::stderr();
    let _ = write!(
        stderr,
        "Delete these {} item{}? [y/N] ",
        plan.items.len(),
        if plan.items.len() == 1 { "" } else { "s" }
    );
    let _ = stderr.flush();
    let mut answer = String::new();
    let _ = std::io::stdin().lock().read_line(&mut answer);
    if is_yes(&answer) {
        return Ok(());
    }
    Err(declined())
}

/// Nur `y` und `yes` gelten, gleich in welcher Schreibweise.
fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Die Antwort war nicht ja: nichts gelöscht, Exit 1.
fn declined() -> Failure {
    Failure::with_exit(
        Diagnostic::builder(codes::CLI_004, Severity::Info)
            .why("nothing was deleted; the answer was not yes".to_owned())
            .build(),
        EXIT_USER,
    )
}

/// Der Plan als Tabelle, darunter jede Warnung, was bleibt und was nie
/// gelöscht wird; alles aus der Antwort des Daemons.
fn print_plan(ctx: &Context, plan: &v1::CleanupPlanView) {
    if ctx.render.is_json() {
        return;
    }
    if !plan.items.is_empty() {
        let rows: Vec<Vec<String>> = plan
            .items
            .iter()
            .map(|item| {
                vec![
                    item.kind.clone(),
                    item.count.to_string(),
                    size(item.bytes),
                    item.description.clone(),
                ]
            })
            .collect();
        ctx.render
            .line(crate::render::table(&["KIND", "COUNT", "SIZE", "WHAT"], &rows).trim_end());
        let total = plan
            .items
            .iter()
            .map(|item| item.bytes)
            .fold(0, u64::saturating_add);
        ctx.render.line(&format!("About {} in total.", size(total)));
    }
    for item in plan.items.iter().filter(|item| !item.warning.is_empty()) {
        ctx.render
            .line(&format!("WARNING: {} ({})", item.warning, item.description));
    }
    for kept in &plan.kept {
        ctx.render.line(&format!("Stays: {kept}"));
    }
    ctx.render.line(&plan.not_deleted);
}

fn print_result(ctx: &Context, plan: &v1::CleanupPlanView, cleaned: &v1::CleanupResult) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "plan": plan_json(plan),
            "deleted": true,
            "outcomes": cleaned.outcomes.iter().map(|outcome| json!({
                "kind": outcome.kind,
                "project_id": outcome.project_id,
                "path": outcome.path,
                "deleted": outcome.deleted,
                "count": outcome.count,
                "bytes_freed": outcome.bytes_freed,
                "reason": outcome.reason,
            })).collect::<Vec<_>>(),
            "bytes_freed": cleaned.bytes_freed,
            "not_deleted": cleaned.not_deleted,
        }));
        return;
    }
    for outcome in &cleaned.outcomes {
        let place = if outcome.path.is_empty() {
            "sessions without a project".to_owned()
        } else {
            outcome.path.clone()
        };
        let mut line = if outcome.deleted {
            format!(
                "deleted {} {} of {place}, {} freed",
                outcome.count,
                outcome.kind,
                size(outcome.bytes_freed)
            )
        } else {
            format!("not deleted: {} of {place}", outcome.kind)
        };
        if !outcome.reason.is_empty() {
            line = format!("{line} ({})", outcome.reason);
        }
        ctx.render.line(&line);
    }
    ctx.render
        .line(&format!("{} freed in total.", size(cleaned.bytes_freed)));
    ctx.render.line(&cleaned.not_deleted);
}

fn plan_json(plan: &v1::CleanupPlanView) -> Value {
    json!({
        "items": plan.items.iter().map(|item| json!({
            "kind": item.kind,
            "project_id": item.project_id,
            "path": item.path,
            "description": item.description,
            "bytes": item.bytes,
            "count": item.count,
            "warning": item.warning,
        })).collect::<Vec<_>>(),
        "digest": plan.digest,
        "kept": plan.kept,
        "not_deleted": plan.not_deleted,
        "kinds": plan.kinds,
    })
}

/// Bytes, wie ein Mensch sie liest: `512 B`, `1.5 KiB`, `3.2 MiB`.
fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64 / 1024.0;
    let mut unit = UNITS[0];
    for next in &UNITS[1..] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    format!("{value:.1} {unit}")
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

    /// Review HUM-533: Ein `n` am Terminal löscht nichts und endet mit Exit 1.
    #[test]
    fn answering_no_ends_with_exit_1() {
        assert!(is_yes("y\n"));
        assert!(is_yes(" YES "));
        for answer in ["n\n", "", "no", "yes please"] {
            assert!(!is_yes(answer), "{answer:?}");
        }
        let failure = declined();
        assert_eq!(failure.exit, EXIT_USER);
        assert_eq!(crate::cmd::exit_code(&failure.diagnostic), EXIT_USER);
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_004");
    }

    #[test]
    fn sizes_read_like_a_human_writes_them() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(1536), "1.5 KiB");
        assert_eq!(size(3 * 1024 * 1024), "3.0 MiB");
    }

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
