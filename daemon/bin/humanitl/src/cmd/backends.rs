//! `humanitl sandbox backends` (HUM-432).
//!
//! Ein gRPC-Aufruf und sonst nichts (ADR-018): Welche Backends es gibt,
//! welche Stufe sie bieten, ob ein Start damit ginge und was fehlt, sagt der
//! Daemon (`SandboxBackends`). Die Kommandozeile prüft nichts selbst; sie
//! zeigt die Liste als Tabelle mit den Befunden darunter, beides auf stdout
//! und damit auch mit `-q`, oder mit `--json` als einen Wert.

use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cmd::rules::{wire_block, wire_json};
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic};
use crate::render::table;

/// Die Spalten der Tabelle.
const HEADERS: [&str; 4] = ["BACKEND", "ISOLATION", "AVAILABLE", "SELECTED"];

/// Führt `humanitl sandbox backends` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; der Befund des Daemons, wenn er
/// die Liste nicht bilden kann.
pub async fn run(ctx: &Context) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let list = client
        .sandbox_backends(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "SandboxBackends")))?
        .into_inner();
    if ctx.render.is_json() {
        ctx.render.value(&list_json(&list));
        return Ok(EXIT_OK);
    }
    ctx.render.line(table(&HEADERS, &rows(&list)).trim_end());
    // Warum ein Backend fehlt und was hilft, gehört zum Ergebnis und steht
    // deshalb auf stdout, auch mit `-q`; nur die Untergrenze ist ein Hinweis.
    ctx.render.note(&format!(
        "sandbox.min_isolation: {}",
        level_name(list.min_isolation)
    ));
    for (name, diagnostics) in findings(&list) {
        for diagnostic in diagnostics {
            ctx.render
                .line(&format!("{name}: {}", wire_block(diagnostic).trim_end()));
        }
    }
    Ok(EXIT_OK)
}

/// Eine Zeile je Backend, darunter eingerückt seine Laufzeiten.
fn rows(list: &v1::SandboxBackendList) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for backend in &list.backends {
        rows.push(row(
            &backend.name,
            backend.isolation_level,
            backend.available,
            backend.selected,
        ));
        for runtime in &backend.runtimes {
            rows.push(row(
                &format!("  {}", runtime.name),
                runtime.isolation_level,
                runtime.available,
                runtime.selected,
            ));
        }
    }
    rows
}

fn row(name: &str, level: i32, available: bool, selected: bool) -> Vec<String> {
    vec![
        name.to_owned(),
        level_name(level).to_owned(),
        if available { "yes" } else { "no" }.to_owned(),
        if selected { "*" } else { "" }.to_owned(),
    ]
}

/// Die Befunde unter der Tabelle: die eines Backends unter seinem Namen,
/// die einer Laufzeit, die nicht gewählt ist, unter `backend/laufzeit`. Die
/// Befunde der gewählten Laufzeit sind die des Backends und stehen nur einmal
/// da.
fn findings(list: &v1::SandboxBackendList) -> Vec<(String, &[v1::Diagnostic])> {
    let mut out = Vec::new();
    for backend in &list.backends {
        out.push((backend.name.clone(), backend.diagnostics.as_slice()));
        for runtime in backend.runtimes.iter().filter(|runtime| !runtime.selected) {
            out.push((
                format!("{}/{}", backend.name, runtime.name),
                runtime.diagnostics.as_slice(),
            ));
        }
    }
    out
}

/// Der Name einer Stufe, wie ihn `sandbox.min_isolation` schreibt.
fn level_name(level: i32) -> &'static str {
    match v1::IsolationLevel::try_from(level) {
        Ok(v1::IsolationLevel::Policy) => "policy",
        Ok(v1::IsolationLevel::Namespaces) => "namespaces",
        Ok(v1::IsolationLevel::Kernel) => "kernel",
        Ok(v1::IsolationLevel::Unspecified) | Err(_) => "unknown",
    }
}

/// Die Liste als ein JSON-Wert.
fn list_json(list: &v1::SandboxBackendList) -> Value {
    json!({
        "min_isolation": level_name(list.min_isolation),
        "backends": list.backends.iter().map(|backend| json!({
            "name": backend.name,
            "isolation_level": level_name(backend.isolation_level),
            "available": backend.available,
            "selected": backend.selected,
            "diagnostics": backend.diagnostics.iter().map(wire_json).collect::<Vec<_>>(),
            "runtimes": backend.runtimes.iter().map(|runtime| json!({
                "name": runtime.name,
                "isolation_level": level_name(runtime.isolation_level),
                "available": runtime.available,
                "selected": runtime.selected,
                "diagnostics": runtime.diagnostics.iter().map(wire_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::{findings, level_name, list_json, rows};

    fn diagnostic(code: &str) -> v1::Diagnostic {
        v1::Diagnostic {
            code: code.to_owned(),
            severity: v1::Severity::Blocking as i32,
            why: "because".to_owned(),
            ..v1::Diagnostic::default()
        }
    }

    fn list() -> v1::SandboxBackendList {
        v1::SandboxBackendList {
            min_isolation: v1::IsolationLevel::Policy as i32,
            backends: vec![
                v1::SandboxBackend {
                    name: "native".to_owned(),
                    isolation_level: v1::IsolationLevel::Namespaces as i32,
                    available: true,
                    selected: true,
                    ..v1::SandboxBackend::default()
                },
                v1::SandboxBackend {
                    name: "podman".to_owned(),
                    isolation_level: v1::IsolationLevel::Namespaces as i32,
                    diagnostics: vec![diagnostic("SANDBOX_048")],
                    runtimes: vec![
                        v1::SandboxRuntime {
                            name: "crun".to_owned(),
                            isolation_level: v1::IsolationLevel::Namespaces as i32,
                            selected: true,
                            diagnostics: vec![diagnostic("SANDBOX_048")],
                            ..v1::SandboxRuntime::default()
                        },
                        v1::SandboxRuntime {
                            name: "krun".to_owned(),
                            isolation_level: v1::IsolationLevel::Kernel as i32,
                            diagnostics: vec![diagnostic("SANDBOX_048")],
                            ..v1::SandboxRuntime::default()
                        },
                    ],
                    ..v1::SandboxBackend::default()
                },
            ],
        }
    }

    #[test]
    fn the_table_shows_each_backend_with_its_runtimes_below() {
        let rows = rows(&list());
        assert_eq!(
            rows,
            [
                ["native", "namespaces", "yes", "*"],
                ["podman", "namespaces", "no", ""],
                ["  crun", "namespaces", "no", "*"],
                ["  krun", "kernel", "no", ""],
            ]
        );
    }

    /// Die Befunde der gewählten Laufzeit sind die des Backends und stehen
    /// nur einmal unter der Tabelle.
    #[test]
    fn the_findings_of_the_chosen_runtime_are_not_repeated() {
        let list = list();
        let names: Vec<String> = findings(&list)
            .into_iter()
            .filter(|(_, diagnostics)| !diagnostics.is_empty())
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["podman", "podman/krun"]);
    }

    #[test]
    fn the_json_carries_levels_by_name_and_every_diagnostic() {
        let value = list_json(&list());
        assert_eq!(value["min_isolation"], "policy");
        assert_eq!(value["backends"][0]["name"], "native");
        assert_eq!(value["backends"][0]["available"], true);
        assert_eq!(
            value["backends"][1]["diagnostics"][0]["code"],
            "SANDBOX_048"
        );
        assert_eq!(
            value["backends"][1]["runtimes"][1]["isolation_level"],
            "kernel"
        );
        assert_eq!(level_name(0), "unknown");
    }
}
