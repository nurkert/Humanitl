//! `humanitl flows list|show|decide`: die Flows dieser und früherer Sitzungen,
//! und die Entscheidung über einen, der wartet.
//!
//! Alle drei sind gRPC-Aufrufe und sonst nichts (ADR-018). Gefiltert, sortiert
//! und geschnitten wird im Daemon; hier wird gezählt, ausgerichtet und
//! geschrieben. Auch `decide` entscheidet nichts selbst: Es schickt `Decide`
//! mit genau einer Flow-Id und gibt die Antwort des Dienstes wieder.
//! Die Notiz wandert ungeprüft mit; gesäubert (Steuerzeichen, Länge) wird sie
//! dort, wo sie in den Body und in den Header geht, und nicht an jeder Stelle,
//! die sie schickt (`humanitl_core::block::sanitize_note`, HUM-072).
//!
//! `GetFlow` ist in M1 noch `UNIMPLEMENTED` (HUM-026 bringt den Recorder).
//! `flows show` fragt trotzdem zuerst danach und fällt erst dann auf die Zeile
//! aus `ListFlows` zurück: sobald die RPC da ist, zeigt dasselbe Kommando ohne
//! Änderung die vollständigen Angaben.

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::convert::state_name;
use humanitl_ipc::v1;
use serde_json::{Value, json};
use tonic::Code;

use crate::cli::FlowsCmd;
use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::render::table;

/// Wie viele Zeilen `flows show` höchstens durchsucht, wenn `GetFlow` fehlt.
const SEARCH_LIMIT: u32 = 1000;

/// Führt `humanitl flows <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `IPC_003`, wenn `show` oder
/// `decide` eine Id nennt, die der Daemon nicht (mehr) kennt.
pub async fn run(ctx: &Context, cmd: &FlowsCmd) -> Result<u8, Failure> {
    let client = ctx.connect().await?;
    match cmd {
        FlowsCmd::List { filter, limit } => list(ctx, client, filter.as_deref(), *limit).await,
        FlowsCmd::Show { id } => show(ctx, client, id).await,
        FlowsCmd::Decide { id, verdict, note } => {
            decide(ctx, client, id, verdict, note.as_deref()).await
        }
    }
}

/// `flows list [FILTER]`.
async fn list(
    ctx: &Context,
    mut client: Client,
    filter: Option<&str>,
    limit: u32,
) -> Result<u8, Failure> {
    let page = fetch(&mut client, filter.unwrap_or_default(), limit).await?;

    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "flows": page.flows.iter().map(summary_json).collect::<Vec<Value>>(),
            "next_cursor": page.next_cursor,
            "total": page.total,
        }));
        return Ok(EXIT_OK);
    }

    if page.flows.is_empty() {
        ctx.render.note("no flows");
        return Ok(EXIT_OK);
    }

    let rows: Vec<Vec<String>> = page.flows.iter().map(summary_row).collect();
    print!(
        "{}",
        table(
            &[
                "ID", "TIME", "METHOD", "HOST", "PATH", "STATE", "DECISION", "STATUS"
            ],
            &rows
        )
    );
    if !page.next_cursor.is_empty() {
        ctx.render.note(&format!(
            "{} of {} flows; --limit 0 asks the daemon for its default page",
            page.flows.len(),
            page.total
        ));
    }
    Ok(EXIT_OK)
}

/// `flows show ID`.
async fn show(ctx: &Context, mut client: Client, id: &str) -> Result<u8, Failure> {
    let detail = match client
        .get_flow(v1::FlowRef {
            flow_id: id.to_owned(),
        })
        .await
    {
        Ok(response) => Some(response.into_inner()),
        // Der Vertrag kennt die RPC, dieser Daemon noch nicht: die Zeile aus
        // `ListFlows` trägt alles, was M1 über einen Flow weiß.
        Err(status) if status.code() == Code::Unimplemented => {
            ctx.render.detail(&format!(
                "GetFlow is not implemented yet ({}); showing the summary from ListFlows",
                status.message()
            ));
            None
        }
        Err(status) => return Err(Failure::new(status_diagnostic(&status, "GetFlow"))),
    };

    let summary = match detail.as_ref().and_then(|detail| detail.summary.clone()) {
        Some(summary) => summary,
        None => find(&mut client, id).await?,
    };

    if ctx.render.is_json() {
        let mut value = summary_json(&summary);
        if let Some(object) = value.as_object_mut()
            && let Some(preview) = detail.as_ref().map(|detail| detail.body_preview.clone())
        {
            object.insert("body_preview".to_owned(), Value::String(preview));
        }
        ctx.render.value(&value);
        return Ok(EXIT_OK);
    }

    let rows = detail_rows(&summary);
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    if let Some(detail) = detail.as_ref()
        && !detail.body_preview.is_empty()
    {
        ctx.render.line("");
        ctx.render.line(&detail.body_preview);
    }
    Ok(EXIT_OK)
}

/// `flows decide ID allow|block [--note TEXT]`.
///
/// Genau eine Id je Aufruf. Der Vertrag erlaubt einen Stapel, aber auf der
/// Kommandozeile wäre er die bequeme Art, versehentlich mehr freizugeben als
/// gemeint; wer einen Stapel will, ruft das Kommando in einer Schleife auf und
/// sieht dabei jede Entscheidung.
///
/// Ein Flow, der nicht mehr wartet, ist kein Erfolg mit leerem Inhalt: Der
/// Dienst antwortet dann mit `FailedPrecondition` und `IPC_003`, und das wird
/// hier zum Exit-Code 1 mit dem Befund des Dienstes.
async fn decide(
    ctx: &Context,
    mut client: Client,
    id: &str,
    verdict: &str,
    note: Option<&str>,
) -> Result<u8, Failure> {
    // `clap` lässt nur `allow` und `block` durch (`value_parser` in
    // `cli::FlowsCmd::Decide`); alles andere kommt hier nie an. Ein Zweig, der
    // im Zweifel blockt, ist trotzdem der richtige: fail closed.
    let decision = if verdict == "allow" {
        v1::decide_request::Decision::Allow(())
    } else {
        v1::decide_request::Decision::Block(v1::decide_request::Block {
            note: note.unwrap_or_default().to_owned(),
        })
    };

    let response = client
        .decide(v1::DecideRequest {
            flow_ids: vec![id.to_owned()],
            decision: Some(decision),
            remember: None,
            acknowledge_findings: false,
        })
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "Decide")))?;

    let result = response.results.first().ok_or_else(|| {
        Failure::new(
            Diagnostic::builder(codes::IPC_003, Severity::Error)
                .why(format!(
                    "the daemon answered Decide for {id} without a result"
                ))
                .build(),
        )
    })?;

    if !result.applied {
        // Der Befund des Dienstes, wenn er einen mitgab; sonst einer, der
        // wenigstens die Id nennt.
        let diagnostic = result
            .diagnostic
            .as_ref()
            .and_then(from_proto)
            .unwrap_or_else(|| {
                Diagnostic::builder(codes::IPC_003, Severity::Error)
                    .why(format!("the daemon did not decide {id}"))
                    .build()
            });
        return Err(Failure::new(diagnostic));
    }

    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "flow_id": result.flow_id,
            "decision": verdict,
            "note": note.unwrap_or_default(),
            "applied": true,
        }));
        return Ok(EXIT_OK);
    }
    ctx.render
        .line(&format!("{} {}", verdict, short_id(&result.flow_id)));
    Ok(EXIT_OK)
}

/// Eine Seite der Historie.
async fn fetch(client: &mut Client, filter: &str, limit: u32) -> Result<v1::FlowPage, Failure> {
    client
        .list_flows(v1::ListFlowsRequest {
            filter: filter.to_owned(),
            since_flow_id: String::new(),
            cursor: String::new(),
            limit,
            order_by: String::new(),
            include_passthrough: false,
        })
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "ListFlows")))
}

/// Sucht einen Flow in der Historie, solange `GetFlow` fehlt.
async fn find(client: &mut Client, id: &str) -> Result<v1::FlowSummary, Failure> {
    let page = fetch(client, "", SEARCH_LIMIT).await?;
    page.flows
        .into_iter()
        .find(|summary| summary.flow_id == id)
        .ok_or_else(|| {
            Failure::new(
                Diagnostic::builder(codes::IPC_003, Severity::Error)
                    .why(format!(
                        "no flow {id} in the last {SEARCH_LIMIT} of this session; humanitl flows list shows what there is"
                    ))
                    .build(),
            )
        })
}

/// Eine Zeile der Liste.
fn summary_row(summary: &v1::FlowSummary) -> Vec<String> {
    vec![
        short_id(&summary.flow_id),
        clock(summary.received_at.as_ref().map(|at| at.seconds)),
        method_name(summary),
        authority(summary.authority.as_ref()),
        summary.path.clone(),
        state_name(summary.state).to_ascii_lowercase(),
        decision_name(summary.decision).to_owned(),
        if summary.status == 0 {
            "-".to_owned()
        } else {
            summary.status.to_string()
        },
    ]
}

/// Die Felder eines einzelnen Flows.
fn detail_rows(summary: &v1::FlowSummary) -> Vec<Vec<String>> {
    let mut rows = vec![
        vec!["flow".to_owned(), summary.flow_id.clone()],
        vec!["session".to_owned(), summary.session_id.clone()],
        vec![
            "received".to_owned(),
            clock(summary.received_at.as_ref().map(|at| at.seconds)),
        ],
        vec!["method".to_owned(), method_name(summary)],
        vec![
            "url".to_owned(),
            format!("{}{}", authority(summary.authority.as_ref()), summary.path),
        ],
        vec![
            "state".to_owned(),
            state_name(summary.state).to_ascii_lowercase(),
        ],
        vec![
            "decision".to_owned(),
            decision_name(summary.decision).to_owned(),
        ],
        vec![
            "status".to_owned(),
            if summary.status == 0 {
                "-".to_owned()
            } else {
                summary.status.to_string()
            },
        ],
        vec![
            "bytes".to_owned(),
            format!(
                "{} up, {} down",
                summary.request_size, summary.response_size
            ),
        ],
        vec!["findings".to_owned(), summary.finding_count.to_string()],
    ];
    if !summary.rule_id.is_empty() {
        rows.push(vec!["rule".to_owned(), summary.rule_id.clone()]);
    }
    if !summary.origin_tool.is_empty() {
        rows.push(vec!["tool".to_owned(), summary.origin_tool.clone()]);
    }
    rows
}

/// Ein Flow als JSON, mit denselben Feldern wie die Tabelle plus den Zahlen.
fn summary_json(summary: &v1::FlowSummary) -> Value {
    json!({
        "flow_id": summary.flow_id,
        "session_id": summary.session_id,
        "received_at": summary.received_at.as_ref().map(|at| at.seconds),
        "method": method_name(summary),
        "host": authority(summary.authority.as_ref()),
        "path": summary.path,
        "state": state_name(summary.state).to_ascii_lowercase(),
        "decision": decision_name(summary.decision),
        "status": summary.status,
        "request_size": summary.request_size,
        "response_size": summary.response_size,
        "finding_count": summary.finding_count,
        "edited": summary.edited,
        "passthrough": summary.passthrough,
        "rule_id": summary.rule_id,
        "origin_tool": summary.origin_tool,
    })
}

/// Die ersten acht Zeichen einer Flow-Id: genug, um sie in einer Liste
/// wiederzuerkennen. `flows show` will die ganze.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Ein Zeitstempel als lokale Uhrzeit, oder `-`.
///
/// Die Sekunden kommen aus `google.protobuf.Timestamp`; der Typ selbst wird
/// hier nicht genannt, damit diese Crate `prost-types` nicht braucht.
fn clock(seconds: Option<i64>) -> String {
    seconds
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map_or_else(
            || "-".to_owned(),
            |time| {
                chrono::DateTime::<chrono::Local>::from(time)
                    .format("%H:%M:%S")
                    .to_string()
            },
        )
}

/// Der Name der Methode, mit dem Rohwert bei `METHOD_OTHER`.
fn method_name(summary: &v1::FlowSummary) -> String {
    if !summary.method_raw.is_empty() {
        return summary.method_raw.clone();
    }
    v1::Method::try_from(summary.method)
        .unwrap_or(v1::Method::Unspecified)
        .as_str_name()
        .trim_start_matches("METHOD_")
        .to_owned()
}

/// Der Name der Entscheidung, klein geschrieben.
fn decision_name(decision: i32) -> &'static str {
    match v1::DecisionKind::try_from(decision).unwrap_or(v1::DecisionKind::Unspecified) {
        v1::DecisionKind::Allow => "allow",
        v1::DecisionKind::AllowEdited => "allow_edited",
        v1::DecisionKind::Block => "block",
        v1::DecisionKind::TimedOut => "timed_out",
        v1::DecisionKind::Unspecified => "-",
    }
}

/// Host und Port, wie sie in einer URL stünden.
fn authority(authority: Option<&v1::Authority>) -> String {
    let Some(authority) = authority else {
        return "-".to_owned();
    };
    let host = if authority.display_host.is_empty() {
        authority.host.clone()
    } else {
        authority.display_host.clone()
    };
    match authority.port {
        0 | 80 | 443 => host,
        port => format!("{host}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::{authority, decision_name, method_name, short_id, summary_row};

    fn summary() -> v1::FlowSummary {
        v1::FlowSummary {
            flow_id: "0199c0ffee00700080008000deadbeef".to_owned(),
            session_id: "session".to_owned(),
            received_at: None,
            method: v1::Method::Get as i32,
            method_raw: String::new(),
            scheme: v1::Scheme::Https as i32,
            authority: Some(v1::Authority {
                host: "github.com".to_owned(),
                port: 443,
                is_ip_literal: false,
                display_host: String::new(),
            }),
            path: "/api/v3".to_owned(),
            state: v1::FlowState::Held as i32,
            decision: v1::DecisionKind::Unspecified as i32,
            decision_source: 0,
            block_reason: 0,
            rule_id: String::new(),
            status: 0,
            request_size: 12,
            response_size: 0,
            duration: None,
            finding_count: 0,
            edited: false,
            passthrough: false,
            deadline: None,
            origin_tool: String::new(),
            upstream_error: 0,
        }
    }

    #[test]
    fn a_row_names_the_flow_the_host_and_the_state() {
        let row = summary_row(&summary());

        assert_eq!(row[0], "0199c0ff");
        assert_eq!(row[2], "GET");
        assert_eq!(row[3], "github.com");
        assert_eq!(row[4], "/api/v3");
        assert_eq!(row[5], "held");
        assert_eq!(row[6], "-");
        assert_eq!(row[7], "-");
    }

    #[test]
    fn a_raw_method_wins_over_the_enum() {
        let mut summary = summary();
        summary.method = v1::Method::Other as i32;
        summary.method_raw = "PROPFIND".to_owned();
        assert_eq!(method_name(&summary), "PROPFIND");
    }

    #[test]
    fn a_port_that_is_not_the_default_stays_visible() {
        let with_port = v1::Authority {
            host: "localhost".to_owned(),
            port: 1234,
            is_ip_literal: false,
            display_host: String::new(),
        };
        assert_eq!(authority(Some(&with_port)), "localhost:1234");
        assert_eq!(authority(None), "-");
    }

    #[test]
    fn every_decision_has_a_name() {
        assert_eq!(decision_name(v1::DecisionKind::Allow as i32), "allow");
        assert_eq!(decision_name(v1::DecisionKind::Block as i32), "block");
        assert_eq!(
            decision_name(v1::DecisionKind::TimedOut as i32),
            "timed_out"
        );
        assert_eq!(short_id("abc"), "abc");
    }
}
