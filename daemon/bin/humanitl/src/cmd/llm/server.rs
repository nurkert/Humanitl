//! `humanitl llm server ls|add|use|rename|rm`: die gespeicherten
//! Modellserver (HUM-289).
//!
//! Ein gRPC-Aufruf je Befehl (`LlmServers`, ADR-018); hier wird nur
//! geschrieben, was zurückkam. Keine Antwort trägt einen Schlüssel, also auch
//! keine Ausgabe dieses Befehls: In der Spalte `KEY` steht der Verweis, nie
//! der Wert. Ein Wechsel gilt ab der nächsten Sitzung, und die Ausgabe sagt
//! das, damit niemand eine laufende Sitzung für umgestellt hält.

use humanitl_ipc::v1;
use humanitl_ipc::v1::llm_servers_request::{Activate, Add, List, Op, Remove, Rename};
use serde_json::{Value, json};

use crate::cli::ServerCmd;
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
    let (op, changes) = operation(cmd);
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
fn operation(cmd: &ServerCmd) -> (Op, bool) {
    match cmd {
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
    }
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
