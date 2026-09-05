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
//! Änderung die vollständigen Angaben. Nur `--body` kommt ohne `GetFlow` nicht
//! aus; dort steht der Verweis auf den Body, und ohne ihn wird kein Byte
//! erfunden.
//!
//! Was die Tabelle nicht führt, steht in `--json`: die Zeile ist die Übersicht,
//! der JSON-Wert die Auskunft. `SIZE` zählt Anfrage und Antwort zusammen in
//! Bytes, `MS` ist die Dauer in Millisekunden, und `PATH` wird in der Mitte auf
//! 40 Zeichen gekürzt, damit die Spalten stehen bleiben.

use std::io::Write as _;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
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

/// Auf wie viele Zeichen `PATH` in der Tabelle gekürzt wird.
const PATH_WIDTH: usize = 40;

/// Die Spalten von `flows list` und des Probelaufs aus `rules dry-run`.
pub const FLOW_HEADERS: [&str; 12] = [
    "ID", "TIME", "STATE", "DECISION", "METHOD", "HOST", "PATH", "STATUS", "SIZE", "MS",
    "FINDINGS", "RULE",
];

/// Führt `humanitl flows <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `IPC_003`, wenn `show` oder
/// `decide` eine Id nennt, die der Daemon nicht (mehr) kennt.
pub async fn run(ctx: &Context, cmd: &FlowsCmd) -> Result<u8, Failure> {
    let client = ctx.connect().await?;
    match cmd {
        FlowsCmd::List {
            filter,
            limit,
            sort,
            asc,
        } => list(ctx, client, filter, *limit, sort, *asc).await,
        FlowsCmd::Show { id, body, raw } => show(ctx, client, id, body.as_deref(), *raw).await,
        FlowsCmd::Decide { id, verdict, note } => {
            decide(ctx, client, id, verdict, note.as_deref()).await
        }
    }
}

/// `flows list [FILTER...]`.
///
/// Der Filter wird Wort für Wort so weitergereicht, wie er auf der
/// Kommandozeile stand; gefiltert und sortiert wird im Dienst.
async fn list(
    ctx: &Context,
    mut client: Client,
    filter: &[String],
    limit: u32,
    sort: &str,
    asc: bool,
) -> Result<u8, Failure> {
    let filter = filter.join(" ");
    let page = fetch(&mut client, &filter, limit, &order_by(sort, asc)).await?;

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

    print_flows(&page.flows);
    if !page.next_cursor.is_empty() {
        ctx.render.note(&format!(
            "{} of {} flows; --limit 0 asks the daemon for its default page",
            page.flows.len(),
            page.total
        ));
    }
    Ok(EXIT_OK)
}

/// `flows show ID [--body request|response] [--raw]`.
async fn show(
    ctx: &Context,
    mut client: Client,
    id: &str,
    body: Option<&str>,
    raw: bool,
) -> Result<u8, Failure> {
    if raw && ctx.render.is_json() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(
                    "--raw writes the body byte for byte and --json writes one line of JSON;                      one call does one of the two"
                        .to_owned(),
                )
                .fix(FixAction::CopyCommand(format!(
                    "humanitl flows show {id} --body response --raw"
                )))
                .build(),
        ));
    }

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
            if let Some(which) = body {
                // Ohne `GetFlow` gibt es keinen Verweis auf einen Body. Ein
                // leerer Ausdruck sähe aus wie ein leerer Body.
                return Err(Failure::new(status_diagnostic(
                    &status,
                    &format!("GetFlow, which --body {which} needs,"),
                )));
            }
            ctx.render.detail(&format!(
                "GetFlow is not implemented yet ({}); showing the summary from ListFlows",
                status.message()
            ));
            None
        }
        Err(status) => return Err(Failure::new(status_diagnostic(&status, "GetFlow"))),
    };

    if let Some(which) = body {
        let detail = detail.ok_or_else(|| {
            Failure::new(
                Diagnostic::builder(codes::IPC_003, Severity::Error)
                    .why(format!(
                        "the daemon answered GetFlow for {id} without a flow"
                    ))
                    .build(),
            )
        })?;
        return show_body(ctx, &mut client, &detail, which, raw).await;
    }

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

/// `flows show ID --body request|response`.
///
/// Der Body kommt über `GetBody` und nicht aus der Vorschau: die Vorschau ist
/// auf 4096 Zeichen gekürzt, und ein gekürzter Body, der wie der ganze
/// aussieht, ist eine Unwahrheit (`backlog/CONVENTIONS.md` 4.13).
async fn show_body(
    ctx: &Context,
    client: &mut Client,
    detail: &v1::FlowDetail,
    which: &str,
    raw: bool,
) -> Result<u8, Failure> {
    let reference = if which == "request" {
        detail
            .request
            .as_ref()
            .and_then(|request| request.body.clone())
    } else {
        detail.response_body.clone()
    };
    let flow_id = detail
        .summary
        .as_ref()
        .map_or_else(String::new, |summary| summary.flow_id.clone());

    let Some(reference) = reference.filter(|body| body.size > 0) else {
        if ctx.render.is_json() {
            ctx.render.value(&json!({
                "flow_id": flow_id,
                "body": which,
                "present": false,
            }));
        } else {
            ctx.render
                .note(&format!("the flow has no {which} body of its own"));
        }
        return Ok(EXIT_OK);
    };

    let bytes = body_bytes(client, &reference).await?;
    let complete = u64::try_from(bytes.len()).unwrap_or(u64::MAX) == reference.size;
    if !complete {
        ctx.render.note(&format!(
            "the daemon sent {} of {} bytes",
            bytes.len(),
            reference.size
        ));
    }
    if reference.truncated {
        ctx.render
            .note("the recorder kept only the beginning of this body");
    }

    if raw {
        let mut out = std::io::stdout().lock();
        if let Err(error) = out.write_all(&bytes).and_then(|()| out.flush()) {
            return Err(Failure::new(
                Diagnostic::builder(codes::CLI_001, Severity::Error)
                    .why(format!("the body could not be written: {error}"))
                    .build(),
            ));
        }
        return Ok(EXIT_OK);
    }

    let text = String::from_utf8_lossy(&bytes);
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "flow_id": flow_id,
            "body": which,
            "present": true,
            "bytes": bytes.len(),
            "size": reference.size,
            "truncated": reference.truncated,
            "content_type": reference.content_type,
            // Ob `text` Byte für Byte dem Body entspricht: ungültiges UTF-8
            // wird zu U+FFFD, und wer vergleicht, muss das wissen.
            "utf8": std::str::from_utf8(&bytes).is_ok(),
            "text": text,
        }));
        return Ok(EXIT_OK);
    }
    print!("{text}");
    Ok(EXIT_OK)
}

/// Holt einen Body über `GetBody`, Stück für Stück.
async fn body_bytes(client: &mut Client, reference: &v1::BodyRef) -> Result<Vec<u8>, Failure> {
    let mut stream = client
        .get_body(reference.clone())
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetBody")))?;

    let mut out = Vec::new();
    while let Some(chunk) = stream
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetBody")))?
    {
        out.extend_from_slice(&chunk.data);
    }
    Ok(out)
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

/// Die Sortierung, wie `ListFlows.order_by` sie erwartet.
///
/// Der Schlüssel ist einer der vier aus `humanitl_recorder::SortKey`; ohne
/// `--asc` steht die jüngste Zeile oben.
fn order_by(sort: &str, asc: bool) -> String {
    format!("{sort} {}", if asc { "asc" } else { "desc" })
}

/// Eine Seite der Historie.
async fn fetch(
    client: &mut Client,
    filter: &str,
    limit: u32,
    order_by: &str,
) -> Result<v1::FlowPage, Failure> {
    client
        .list_flows(v1::ListFlowsRequest {
            filter: filter.to_owned(),
            since_flow_id: String::new(),
            cursor: String::new(),
            limit,
            order_by: order_by.to_owned(),
            include_passthrough: false,
        })
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "ListFlows")))
}

/// Sucht einen Flow in der Historie, solange `GetFlow` fehlt.
async fn find(client: &mut Client, id: &str) -> Result<v1::FlowSummary, Failure> {
    let page = fetch(client, "", SEARCH_LIMIT, &order_by("ts", false)).await?;
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

/// Die Tabelle einer Liste von Flows.
///
/// Auch der Probelauf aus `humanitl rules dry-run` zeigt seine Treffer so:
/// dieselben Spalten, damit man beide Ausgaben nebeneinander lesen kann.
pub fn print_flows(flows: &[v1::FlowSummary]) {
    let rows: Vec<Vec<String>> = flows.iter().map(summary_row).collect();
    print!("{}", table(&FLOW_HEADERS, &rows));
}

/// Eine Zeile der Liste.
///
/// Ein Strich steht für „der Daemon weiß es nicht", nie für eine Null
/// (`backlog/CONVENTIONS.md` 4.13). `SIZE` ist die Summe aus Anfrage und
/// Antwort in Bytes, `MS` die Dauer in Millisekunden.
fn summary_row(summary: &v1::FlowSummary) -> Vec<String> {
    vec![
        short_id(&summary.flow_id),
        clock(summary.received_at.as_ref().map(|at| at.seconds)),
        state_name(summary.state).to_ascii_lowercase(),
        decision_name(summary.decision).to_owned(),
        method_name(summary),
        authority(summary.authority.as_ref()),
        truncate_middle(&summary.path, PATH_WIDTH),
        if summary.status == 0 {
            "-".to_owned()
        } else {
            summary.status.to_string()
        },
        (summary.request_size + summary.response_size).to_string(),
        summary.duration.as_ref().map_or_else(
            || "-".to_owned(),
            |duration| {
                let millis = duration.seconds * 1_000 + i64::from(duration.nanos) / 1_000_000;
                millis.to_string()
            },
        ),
        summary.finding_count.to_string(),
        if summary.rule_id.is_empty() {
            "-".to_owned()
        } else {
            short_id(&summary.rule_id)
        },
    ]
}

/// Kürzt einen Text in der Mitte, damit Anfang und Ende lesbar bleiben.
///
/// Ein Pfad ist vorne und hinten aussagekräftig und in der Mitte selten; wer
/// den ganzen sehen will, nimmt `flows show` oder `--json`.
fn truncate_middle(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_owned();
    }
    // Ein Zeichen geht an das Auslassungszeichen.
    let keep = width.saturating_sub(1);
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let front: String = text.chars().take(head).collect();
    let back: String = text.chars().skip(count - tail).collect();
    format!("{front}…{back}")
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
///
/// `host` bleibt die Anzeigeform mit Port, wie sie in der Tabelle steht;
/// darunter steht `authority` mit dem Namen, über den entschieden wurde: dem
/// A-Label, dem Port und der Anzeigeform daneben. Bei einem
/// internationalisierten Namen sind die beiden nicht dasselbe, und ein Skript
/// muss den vergleichen können, den der Daemon kennt.
pub fn summary_json(summary: &v1::FlowSummary) -> Value {
    json!({
        "flow_id": summary.flow_id,
        "session_id": summary.session_id,
        "received_at": summary.received_at.as_ref().map(|at| at.seconds),
        "method": method_name(summary),
        "host": authority(summary.authority.as_ref()),
        "authority": summary.authority.as_ref().map(|authority| json!({
            "host": authority.host,
            "display_host": authority.display_host,
            "port": authority.port,
            "is_ip_literal": authority.is_ip_literal,
        })),
        "duration_ms": summary.duration.as_ref().map(|duration| {
            duration.seconds * 1_000 + i64::from(duration.nanos) / 1_000_000
        }),
        "path": summary.path,
        "state": state_name(summary.state).to_ascii_lowercase(),
        "decision": decision_name(summary.decision),
        "status": summary.status,
        "request_size": summary.request_size,
        "response_size": summary.response_size,
        "finding_count": summary.finding_count,
        "edited": summary.edited,
        "passthrough": summary.passthrough,
        // Neben der Entscheidung, nicht darin: Über eine Anfrage an
        // `humanitl.internal` entscheidet niemand. Ohne dieses Feld sähe sie
        // in `--json` aus wie ein aufgezeichneter Fluss, über den noch nicht
        // entschieden wurde — genau die Unterscheidung, um die es geht
        // (HUM-103).
        "meta": summary.meta,
        "rule_id": summary.rule_id,
        "origin_tool": summary.origin_tool,
        "error": summary.error,
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

    use super::{
        FLOW_HEADERS, PATH_WIDTH, authority, decision_name, method_name, order_by, short_id,
        summary_json, summary_row, truncate_middle,
    };

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
            error: String::new(),
            meta: false,
        }
    }

    #[test]
    fn the_json_carries_the_reason_a_flow_failed() {
        // HUM-045, Akzeptanzkriterium: `humanitl flows list --json` zeigt das
        // Feld `error`. Leer heißt „nichts gescheitert", nicht „unbekannt".
        let mut summary = summary();
        assert_eq!(summary_json(&summary)["error"], "");
        summary.error = "tls_handshake_failed".to_owned();
        assert_eq!(summary_json(&summary)["error"], "tls_handshake_failed");
    }

    /// `--json` unterscheidet einen Meta-Fluss von einem unentschiedenen.
    ///
    /// HUM-103: An beiden ist `decision` leer. Wer sie in einem Skript
    /// auseinanderhalten will — und der Demolauf tut genau das —, braucht das
    /// Feld `meta`; ohne es sähe eine Auskunft, die der Agent sich geholt hat,
    /// aus wie eine Anfrage, über die noch niemand entschieden hat.
    #[test]
    fn the_json_tells_a_meta_flow_from_an_undecided_one() {
        let mut summary = summary();
        assert_eq!(summary_json(&summary)["meta"], false);
        assert_eq!(
            summary_json(&summary)["decision"],
            decision_name(v1::DecisionKind::Unspecified as i32)
        );

        summary.meta = true;
        summary.path = "/why/0199c0ff-ee00-7000-8000-8000deadbeef".to_owned();
        summary.status = 404;
        let json = summary_json(&summary);
        assert_eq!(json["meta"], true);
        assert_eq!(
            json["decision"],
            decision_name(v1::DecisionKind::Unspecified as i32),
            "nobody decided about a meta request"
        );
        assert_eq!(json["status"], 404);
        assert_eq!(json["path"], "/why/0199c0ff-ee00-7000-8000-8000deadbeef");
    }

    #[test]
    fn a_row_carries_every_column_of_the_table() {
        let row = summary_row(&summary());

        assert_eq!(row.len(), FLOW_HEADERS.len());
        assert_eq!(row[0], "0199c0ff", "ID");
        assert_eq!(row[2], "held", "STATE");
        assert_eq!(row[3], "-", "DECISION");
        assert_eq!(row[4], "GET", "METHOD");
        assert_eq!(row[5], "github.com", "HOST");
        assert_eq!(row[6], "/api/v3", "PATH");
        assert_eq!(row[7], "-", "STATUS");
        assert_eq!(row[8], "12", "SIZE is request plus response in bytes");
        assert_eq!(row[9], "-", "MS stays unknown without a duration");
        assert_eq!(row[10], "0", "FINDINGS");
        assert_eq!(row[11], "-", "RULE");
    }

    #[test]
    fn a_known_duration_is_counted_in_milliseconds() {
        let mut summary = summary();
        summary.duration = Some(prost_types::Duration {
            seconds: 1,
            nanos: 250_000_000,
        });
        summary.response_size = 30;
        let row = summary_row(&summary);

        assert_eq!(row[8], "42", "12 up plus 30 down");
        assert_eq!(row[9], "1250");
    }

    #[test]
    fn a_long_path_is_shortened_in_the_middle() {
        let long = "/repos/humanitl/humanitl/commits/0123456789abcdef/status/checks";
        let short = truncate_middle(long, PATH_WIDTH);

        assert_eq!(short.chars().count(), PATH_WIDTH);
        assert!(short.starts_with("/repos/humanitl"), "{short}");
        assert!(short.ends_with("status/checks"), "{short}");
        assert!(short.contains('…'), "{short}");
        // Was hineinpasst, bleibt unangetastet.
        assert_eq!(truncate_middle("/api/v3", PATH_WIDTH), "/api/v3");
    }

    #[test]
    fn the_order_is_the_key_and_the_direction() {
        assert_eq!(order_by("ts", false), "ts desc");
        assert_eq!(order_by("host", true), "host asc");
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
