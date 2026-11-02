//! `humanitl llm server ls|add|use|rename|rm|edit|assign|unassign|confirm`:
//! die gespeicherten Modellserver (HUM-289, HUM-405).
//!
//! Ein gRPC-Aufruf je Befehl (`LlmServers`, ADR-018); hier wird nur
//! geschrieben, was zurückkam. Keine Antwort trägt einen Schlüssel, also auch
//! keine Ausgabe dieses Befehls: In der Spalte `KEY` steht der Verweis, nie
//! der Wert. Ein Wechsel gilt ab der nächsten Sitzung, und die Ausgabe sagt
//! das, damit niemand eine laufende Sitzung für umgestellt hält.

use humanitl_ipc::v1;
use humanitl_ipc::v1::llm_servers_request::{
    Activate, Add, Assign, ConfirmProject, Edit, List, Op, Remove, Rename, assign, edit,
};
use serde_json::{Value, json};

use crate::cli::{AssignTarget, ServerCmd};
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic};
use crate::render::table;

/// Die Spalten der Tabelle.
const HEADERS: [&str; 5] = ["", "NAME", "ENDPOINT", "MODEL", "KEY"];

/// Führt `humanitl llm server <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; `CONFIG_021` für einen Namen,
/// der keiner ist, den es schon gibt oder nicht gibt, und für den aktiven
/// Server ohne `--use`; `CONFIG_003` für einen Endpunkt, der keiner ist.
pub async fn run(ctx: &Context, cmd: &ServerCmd) -> Result<u8, Failure> {
    let (op, changes) = operation(cmd)?;
    let mut client = ctx.connect().await?;
    let list = client
        .llm_servers(v1::LlmServersRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "LlmServers")))?
        .into_inner();
    render(ctx, &list, changes);
    Ok(EXIT_OK)
}

/// Die Operation zu einem Unterkommando, und ob sie etwas ändert.
///
/// # Errors
///
/// `CLI_004`, wenn sich ein relativer Projektordner nicht gegen das aktuelle
/// Verzeichnis auflösen lässt.
fn operation(cmd: &ServerCmd) -> Result<(Op, bool), Failure> {
    Ok(match cmd {
        ServerCmd::Ls => (Op::List(List {}), false),
        ServerCmd::Add {
            name,
            url,
            model,
            activate,
        } => (
            Op::Add(Add {
                name: name.clone(),
                endpoint: url.clone(),
                models: model.clone(),
                activate: *activate,
            }),
            true,
        ),
        ServerCmd::Use { name } => (Op::Activate(Activate { name: name.clone() }), true),
        ServerCmd::Rename { name, new_name } => (
            Op::Rename(Rename {
                name: name.clone(),
                new_name: new_name.clone(),
            }),
            true,
        ),
        ServerCmd::Rm {
            name,
            replacement,
            keep_key,
        } => (
            Op::Remove(Remove {
                name: name.clone(),
                delete_key: !keep_key,
                replacement: replacement.clone().unwrap_or_default(),
            }),
            true,
        ),
        ServerCmd::Edit {
            name,
            url,
            model,
            no_models,
        } => (
            Op::Edit(Edit {
                name: name.clone(),
                endpoint: url.clone(),
                models: (*no_models || !model.is_empty()).then(|| edit::Models {
                    names: model.clone(),
                }),
            }),
            true,
        ),
        ServerCmd::Assign { name, target } => (
            Op::Assign(Assign {
                server: name.clone(),
                target: Some(assign_target(target)?),
            }),
            true,
        ),
        ServerCmd::Unassign { target } => (
            Op::Assign(Assign {
                server: String::new(),
                target: Some(assign_target(target)?),
            }),
            true,
        ),
        ServerCmd::Confirm { name, project } => (
            Op::ConfirmProject(ConfirmProject {
                project_dir: absolute(project.as_deref().unwrap_or(std::path::Path::new(".")))?,
                server: name.clone(),
            }),
            false,
        ),
    })
}

/// Das Ziel einer Zuordnung auf der Leitung; ein Projektordner absolut.
fn assign_target(target: &AssignTarget) -> Result<assign::Target, Failure> {
    match (&target.profile, &target.project) {
        (Some(profile), _) => Ok(assign::Target::Profile(profile.clone())),
        (None, Some(dir)) => Ok(assign::Target::ProjectDir(absolute(dir)?)),
        (None, None) => Err(Failure::new(
            humanitl_core::Diagnostic::builder(
                humanitl_core::diagnostics::codes::CLI_004,
                humanitl_core::Severity::Error,
            )
            .why(
                "name a session profile with --profile or a project folder with --project"
                    .to_owned(),
            )
            .build(),
        )),
    }
}

/// Ein Ordner als absoluter Pfad, gegen das aktuelle Verzeichnis; der Daemon
/// löst Verweise selbst auf und prüft, dass es ihn gibt.
fn absolute(dir: &std::path::Path) -> Result<String, Failure> {
    std::path::absolute(dir)
        .map(|dir| dir.display().to_string())
        .map_err(|err| {
            Failure::new(
                humanitl_core::Diagnostic::builder(
                    humanitl_core::diagnostics::codes::CLI_004,
                    humanitl_core::Severity::Error,
                )
                .why(format!(
                    "the folder {} could not be resolved against the current directory: {err}",
                    dir.display()
                ))
                .build(),
            )
        })
}

/// Schreibt die Liste: als Tabelle oder als ein JSON-Wert.
fn render(ctx: &Context, list: &v1::LlmServerList, changes: bool) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "active": list.active,
            "implicit": list.implicit,
            "servers": list.servers.iter().map(|server| json!({
                "name": server.name,
                "endpoint": server.endpoint,
                "models": server.models,
                "key_ref": server.key_ref,
                "active": server.active,
                "profiles": server.profiles,
            })).collect::<Vec<Value>>(),
            "profiles": list.profiles.iter().map(|profile| json!({
                "name": profile.name,
                "server": profile.server,
                "builtin": profile.builtin,
            })).collect::<Vec<Value>>(),
            "diagnostics": list
                .diagnostics
                .iter()
                .filter_map(crate::cmd::from_proto)
                .map(|diagnostic| crate::render::diagnostic_json(&diagnostic))
                .collect::<Vec<Value>>(),
        }));
        return;
    }

    if list.servers.is_empty() {
        ctx.render
            .line("no model server saved; add one with `humanitl llm server add NAME URL`");
    } else {
        let rows: Vec<Vec<String>> = list
            .servers
            .iter()
            .map(|server| {
                vec![
                    if server.active { "*" } else { "" }.to_owned(),
                    server.name.clone(),
                    // Benutzer, Passwort und Query bleiben aus der Zeile.
                    humanitl_config::llm_endpoint::display_text(&server.endpoint),
                    server
                        .models
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "-".to_owned()),
                    if server.key_ref.is_empty() {
                        "-".to_owned()
                    } else {
                        server.key_ref.clone()
                    },
                ]
            })
            .collect();
        ctx.render.line(table(&HEADERS, &rows).trim_end());
    }
    if changes {
        ctx.render
            .note("a running session keeps its server; the change applies to the next session");
    }
    for wire in &list.diagnostics {
        if let Some(diagnostic) = crate::cmd::from_proto(wire) {
            ctx.render
                .note(crate::render::diagnostic_block(&diagnostic).trim_end());
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::cli::{Cmd, LlmCmd};

    /// Die Operation einer Kommandozeile `humanitl llm server …`.
    fn op_of(args: &[&str]) -> Op {
        let line = ["humanitl", "llm", "server"].iter().chain(args);
        let invocation = crate::cli::parse(line).expect("the command line parses");
        let Cmd::Llm {
            cmd: LlmCmd::Server { cmd },
        } = invocation.cli.cmd
        else {
            panic!("not llm server");
        };
        operation(&cmd).expect("an operation").0
    }

    /// `edit` ändert nur, was genannt ist; `--no-models` leert die Liste
    /// (HUM-405).
    #[test]
    fn edit_names_only_what_changes() {
        let Op::Edit(edit) = op_of(&["edit", "cloud", "--url", "https://x.example/v1"]) else {
            panic!("not an edit");
        };
        assert_eq!(edit.endpoint.as_deref(), Some("https://x.example/v1"));
        assert!(edit.models.is_none(), "the models stay");
        let Op::Edit(cleared) = op_of(&["edit", "cloud", "--no-models"]) else {
            panic!("not an edit");
        };
        assert_eq!(cleared.endpoint, None);
        assert_eq!(cleared.models.map(|models| models.names), Some(Vec::new()));
    }

    /// `assign` und `unassign` nennen genau ein Ziel; ein Projektordner geht
    /// absolut hinaus, `unassign` mit leerem Server (HUM-405).
    #[test]
    fn assign_and_unassign_name_one_target() {
        let Op::Assign(assign) = op_of(&["assign", "work", "--profile", "llm-only"]) else {
            panic!("not an assignment");
        };
        assert_eq!(assign.server, "work");
        assert_eq!(
            assign.target,
            Some(assign::Target::Profile("llm-only".to_owned()))
        );
        let Op::Assign(project) = op_of(&["unassign", "--project", "some/dir"]) else {
            panic!("not an assignment");
        };
        assert!(project.server.is_empty());
        let Some(assign::Target::ProjectDir(dir)) = project.target else {
            panic!("not a project");
        };
        assert!(std::path::Path::new(&dir).is_absolute(), "{dir}");
        assert!(
            crate::cli::parse(["humanitl", "llm", "server", "assign", "work"]).is_err(),
            "a target is required"
        );
        let Op::ConfirmProject(confirm) = op_of(&["confirm", "work", "--project", "/p"]) else {
            panic!("not a confirmation");
        };
        assert_eq!(confirm.project_dir, "/p");
        assert_eq!(confirm.server, "work");
    }
}
