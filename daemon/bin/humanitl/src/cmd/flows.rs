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
//!
//! `ID` ist das Ende der Flow-Id, nicht ihr Anfang (HUM-316): Eine `UUIDv7`
//! beginnt mit der Uhrzeit, und alle Flows einer Minute teilen dieselben
//! ersten acht Zeichen. Das Ende ist Zufall. Die Spalte zeigt so viele
//! Zeichen, wie es braucht, um die Zeilen der Seite zu unterscheiden, und
//! mindestens acht; `flows show` nimmt genau diese Form.
//!
//! `DECISION` schreibt `aborted`, wo der Dienst `BLOCK_REASON_CLIENT_ABORTED`
//! meldet, und `passthrough` für die Durchreiche zum Modell. Beides liest die
//! Spalte aus dem, was der Dienst sagt; die Kommandozeile deutet nichts aus
//! `error` (ADR-018).

use std::io::Write as _;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, FlowId, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::convert::state_name;
use humanitl_ipc::v1;
use serde_json::{Value, json};
use tonic::Code;

use crate::cli::{FlowsCmd, RememberArgs};
use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::render::table;

/// Wie viele Zeilen `flows show` höchstens durchsucht, wenn `GetFlow` fehlt
/// oder die Id nur ihr Ende nennt.
const SEARCH_LIMIT: u32 = 1000;

/// So viele Zeichen zeigt die Spalte `ID` mindestens, und so viele braucht
/// `flows show` mindestens, um ein Ende nachzuschlagen (HUM-316).
const MIN_ID_TAIL: usize = 8;

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
            passthrough,
        } => {
            let order = order_by(sort, *asc);
            list(ctx, client, filter, *limit, &order, *passthrough).await
        }
        FlowsCmd::Show { id, body, raw } => show(ctx, client, id, body.as_deref(), *raw).await,
        FlowsCmd::Decide {
            id,
            verdict,
            note,
            remember,
        } => decide(ctx, client, id, verdict, note.as_deref(), remember).await,
    }
}

/// `flows list [FILTER...]`.
///
/// Der Filter wird Wort für Wort so weitergereicht, wie er auf der
/// Kommandozeile stand; gefiltert und sortiert wird im Dienst. Ob die
/// Durchreiche zum Modell mitkommt, entscheidet `--passthrough` über
/// `ListFlowsRequest.include_passthrough`, wie der Schalter der History.
async fn list(
    ctx: &Context,
    mut client: Client,
    filter: &[String],
    limit: u32,
    order_by: &str,
    passthrough: bool,
) -> Result<u8, Failure> {
    let filter = filter.join(" ");
    let page = fetch(&mut client, &filter, limit, order_by, passthrough).await?;

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

    // Die Breite der Spalte richtet sich nach der Menge, in der `flows show`
    // nachschlägt, nicht nach der Seite allein: Sonst druckte eine kurze
    // Seite ein Ende, das `show` als mehrdeutig abwiese.
    let searched = search_set(&mut client).await?;
    print_flows(&page.flows, &searched);
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

    // Die Spalte `ID` zeigt nur das Ende; der Dienst kennt nur ganze Ids.
    let id = resolve_id(&mut client, id).await?;
    let id = id.as_str();

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

/// `flows decide ID allow|block [--note TEXT] [--remember PATTERN ...]`.
///
/// Genau eine Id je Aufruf, auch mit `--remember`. Der Vertrag erlaubt einen
/// Stapel, aber auf der Kommandozeile wäre er die bequeme Art, versehentlich
/// mehr freizugeben als gemeint; wer einen Stapel will, ruft das Kommando in
/// einer Schleife auf und sieht dabei jede Entscheidung. Wie in der Oberfläche
/// trägt dann nur der erste Durchlauf `--remember`: Zwölf Aufrufe mit dem Flag
/// legen zwölf Regeln an.
///
/// `--remember` schickt die Regel in derselben Anfrage, in der die
/// Entscheidung steht. Der Dienst legt sie vor der Entscheidung an und nimmt
/// sie zurück, wenn kein Flow entschieden wurde
/// (`humanitl_ipc::server`, `humanitl_ipc::rules::RulesService::remember`).
/// Deshalb nie `rules add` und danach `decide`: Dieser Rücknahmeweg gilt nur
/// für die Regel aus `remember`, und sonst bliebe eine Regel stehen, obwohl
/// kein Flow mehr wartete.
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
    remember: &RememberArgs,
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
            remember: remembered_rule(id, verdict, remember)?,
            acknowledged_findings: Vec::new(),
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
        ctx.render
            .value(&decide_json(verdict, note, &result.flow_id, &response));
        return Ok(EXIT_OK);
    }
    ctx.render.line(&format!(
        "{} {}",
        verdict,
        id_tail(&result.flow_id, MIN_ID_TAIL)
    ));
    if let Some(rule) = response.created_rule.as_ref() {
        ctx.render.line(&rule_line(rule));
    }
    Ok(EXIT_OK)
}

/// Die Regel, die die Entscheidung hinterlässt, oder `None` ohne `--remember`.
///
/// Die Herkunft setzt die Kommandozeile selbst: Der Dienst leitet sie nicht
/// aus `flow_ids` ab, er legt die Regel des Clients ab, wie sie kommt. Die
/// Ableitung im Server wäre dieselbe Rechnung ein zweites Mal, einmal im
/// echten Dienst und einmal im Fake.
///
/// Ist die Id keine Flow-Id, geht die Anfrage ohne Regel hinaus. Der Daemon
/// antwortet dann wie ohne das Flag mit `IPC_004` auf die Id statt mit
/// `IPC_005` auf `created_from_flow_id`, und es entsteht keine Regel für einen
/// Flow, den es nicht gibt.
fn remembered_rule(
    id: &str,
    verdict: &str,
    remember: &RememberArgs,
) -> Result<Option<v1::Rule>, Failure> {
    let Some(args) = remember.rule_args(verdict) else {
        return Ok(None);
    };
    if FlowId::parse(id).is_err() {
        return Ok(None);
    }
    let mut rule = crate::cmd::rules::rule_from_args(&args, None)?;
    // Die volle Id, nicht das Ende aus `id_tail`: `rule_from_proto` weist
    // ein Präfix mit `IPC_005` ab.
    id.clone_into(&mut rule.created_from_flow_id);
    Ok(Some(rule))
}

/// Die Antwort auf `decide` als JSON.
///
/// `created_rule_id` und `created_rule` stehen nur darin, wenn der Dienst eine
/// Regel angelegt hat. Ohne `--remember` sieht ein Aufrufer damit genau die
/// vier Schlüssel, die er vorher sah.
fn decide_json(
    verdict: &str,
    note: Option<&str>,
    flow_id: &str,
    response: &v1::DecideResponse,
) -> Value {
    let mut value = json!({
        "flow_id": flow_id,
        "decision": verdict,
        "note": note.unwrap_or_default(),
        "applied": true,
    });
    if let (Some(object), Some(rule)) = (value.as_object_mut(), response.created_rule.as_ref()) {
        object.insert(
            "created_rule_id".to_owned(),
            Value::String(response.created_rule_id.clone()),
        );
        object.insert(
            "created_rule".to_owned(),
            crate::cmd::rules::rule_json(rule),
        );
    }
    value
}

/// Die zweite Klartextzeile: die Regel, die die Entscheidung hinterlassen hat.
///
/// Die Felder kommen aus derselben Abbildung wie `rules list --json`, damit
/// Aktion und Ablauf hier nicht ein zweites Mal benannt werden.
fn rule_line(rule: &v1::Rule) -> String {
    let json = crate::cmd::rules::rule_json(rule);
    let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
    format!(
        "rule {} {} {} {}",
        short_id(&rule.rule_id),
        text(&json["action"]),
        text(&json["host"]),
        text(&json["expires"]["kind"])
    )
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
    include_passthrough: bool,
) -> Result<v1::FlowPage, Failure> {
    client
        .list_flows(v1::ListFlowsRequest {
            filter: filter.to_owned(),
            since_flow_id: String::new(),
            cursor: String::new(),
            limit,
            order_by: order_by.to_owned(),
            include_passthrough,
        })
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "ListFlows")))
}

/// Sucht einen Flow in der Historie, solange `GetFlow` fehlt.
///
/// Mit der Durchreiche: Auch eine Anfrage an das Modell ist ein Flow, den
/// `flows list --passthrough` zeigt und `flows show` finden muss.
async fn find(client: &mut Client, id: &str) -> Result<v1::FlowSummary, Failure> {
    let page = fetch(client, "", SEARCH_LIMIT, &order_by("ts", false), true).await?;
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

/// Die Flows, unter denen `flows show` ein Ende nachschlägt: die jüngsten
/// [`SEARCH_LIMIT`] samt Durchreiche.
///
/// Die eine Stelle für diese Menge. `flows list` und `rules dry-run` messen
/// die Breite der Spalte `ID` an ihr ([`print_flows`]), `flows show` sucht
/// in ihr ([`resolve_id`]); laufen beide auseinander, druckt die Tabelle ein
/// Ende, das `show` nicht eindeutig findet. `ListFlows` kennt keine Abfrage
/// nur nach Ids, also kommen die Zusammenfassungen.
pub async fn search_set(client: &mut Client) -> Result<Vec<v1::FlowSummary>, Failure> {
    fetch(client, "", SEARCH_LIMIT, &order_by("ts", false), true)
        .await
        .map(|page| page.flows)
}

/// Macht aus dem, was in `flows show ID` stand, eine ganze Flow-Id.
///
/// Eine ganze Id (mit oder ohne Bindestriche) geht unverändert weiter. Alles
/// andere gilt als Ende einer Id, wie die Spalte `ID` es zeigt, und wird in
/// [`search_set`] nachgeschlagen ([`match_tail`]).
async fn resolve_id(client: &mut Client, id: &str) -> Result<String, Failure> {
    if let Some(full) = full_id(id) {
        return Ok(full);
    }
    let wanted = tail_query(id)?;
    let searched = search_set(client).await?;
    match_tail(&searched, id, &wanted)
}

/// Eine ganze Id in der Form, in der der Daemon sie führt: mit Bindestrichen
/// und klein geschrieben. Die Tabelle druckt eine Zeile außerhalb von
/// [`search_set`] als 32 Hexziffern ohne Bindestriche; verglichen wird in
/// [`find`] aber mit der Form des Daemons.
fn full_id(id: &str) -> Option<String> {
    FlowId::parse(id).ok().map(|flow| flow.to_string())
}

/// Prüft, ob `id` als Ende einer Flow-Id taugt, und gibt die Vergleichsform.
///
/// Mindestens [`MIN_ID_TAIL`] Hexziffern: So viele zeigt die Tabelle immer,
/// und ein kürzeres Ende passte unter tausend Flows schnell auf mehrere.
fn tail_query(id: &str) -> Result<String, Failure> {
    let wanted = hex_id(id);
    if wanted.len() < MIN_ID_TAIL || !wanted.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "{id:?} is neither a flow id nor the end of one as humanitl flows list prints it (at least {MIN_ID_TAIL} hex characters)"
                ))
                .fix(FixAction::CopyCommand("humanitl flows list".to_owned()))
                .build(),
        ));
    }
    Ok(wanted)
}

/// Der eine Flow, dessen Id auf `wanted` endet.
///
/// Keiner ist `IPC_003` wie bei [`find`]. Mehrere sind `CLI_004` mit allen
/// Kandidaten im Text: Raten wäre hier die falsche Hilfe, denn `flows show`
/// zeigte dann still einen anderen Flow als gemeint.
fn match_tail(flows: &[v1::FlowSummary], given: &str, wanted: &str) -> Result<String, Failure> {
    let candidates: Vec<&str> = flows
        .iter()
        .map(|summary| summary.flow_id.as_str())
        .filter(|flow_id| hex_id(flow_id).ends_with(wanted))
        .collect();
    match candidates.as_slice() {
        [] => Err(Failure::new(
            Diagnostic::builder(codes::IPC_003, Severity::Error)
                .why(format!(
                    "no flow among the newest {SEARCH_LIMIT} ends in {given}; flows show looks up the end of an id only there, so an older flow needs its whole id from humanitl flows list --json"
                ))
                .fix(FixAction::CopyCommand(
                    "humanitl flows list --json".to_owned(),
                ))
                .build(),
        )),
        [one] => Ok((*one).to_owned()),
        many => Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "{given} is the end of {} flows ({}); give more characters or the whole id",
                    many.len(),
                    many.join(", ")
                ))
                .fix(FixAction::CopyCommand("humanitl flows list --json".to_owned()))
                .build(),
        )),
    }
}

/// Die Tabelle einer Liste von Flows.
///
/// Auch der Probelauf aus `humanitl rules dry-run` zeigt seine Treffer so:
/// dieselben Spalten, damit man beide Ausgaben nebeneinander lesen kann. Die
/// Breite der Spalte `ID` gilt für die ganze Seite und wird gegen
/// `searched` gemessen, die Menge aus [`search_set`] ([`id_tail_len`]).
pub fn print_flows(flows: &[v1::FlowSummary], searched: &[v1::FlowSummary]) {
    print!("{}", table(&FLOW_HEADERS, &flow_rows(flows, searched)));
}

/// Die Zeilen der Tabelle, mit einer Breite der Spalte `ID` für alle.
fn flow_rows(flows: &[v1::FlowSummary], searched: &[v1::FlowSummary]) -> Vec<Vec<String>> {
    let ids: Vec<&str> = flows
        .iter()
        .map(|summary| summary.flow_id.as_str())
        .collect();
    let others: Vec<&str> = searched
        .iter()
        .map(|summary| summary.flow_id.as_str())
        .collect();
    let width = id_tail_len(&ids, &others);
    flows
        .iter()
        .map(|summary| summary_row(summary, width))
        .collect()
}

/// Die Id ohne Bindestriche und in Kleinbuchstaben, die Form, in der Enden
/// verglichen werden.
fn hex_id(id: &str) -> String {
    id.chars()
        .filter(|c| *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Die kürzeste Länge ab [`MIN_ID_TAIL`], bei der das Ende jeder Id der
/// Seite (`page`) auf genau einen Flow passt, gezählt über die Seite und
/// über `searched`, die Menge, in der `flows show` nachschlägt.
///
/// Eine Breite für die ganze Seite statt einer je Zeile: Die Spalte steht
/// dann bündig, und wer zwei Zeilen vergleicht, vergleicht gleich lange
/// Enden. Dieselbe Id in beiden Mengen ist ein Flow, kein Zusammenstoß; zwei
/// Flows außerhalb der Seite, die sich ein Ende teilen, machen die Spalte
/// nicht breiter, denn ihre Enden druckt sie nicht.
///
/// Steht eine Zeile der Seite nicht in `searched` (eine alte Seite mit
/// `--asc` oder ein Filter, der hinter die neuesten Flows reicht), fände
/// `flows show` ihr Ende nicht. Dann druckt die Spalte die ganzen Ids, die
/// `flows show` ohne Suche annimmt.
fn id_tail_len(page: &[&str], searched: &[&str]) -> usize {
    let page: Vec<String> = page.iter().map(|id| hex_id(id)).collect();
    let searched: std::collections::HashSet<String> =
        searched.iter().map(|id| hex_id(id)).collect();
    let all: std::collections::HashSet<String> = page
        .iter()
        .cloned()
        .chain(searched.iter().cloned())
        .collect();
    let longest = all.iter().map(String::len).max().unwrap_or(0);
    if page.iter().any(|id| !searched.contains(id)) {
        return longest.max(MIN_ID_TAIL);
    }
    (MIN_ID_TAIL..longest)
        .find(|&len| {
            let mut counts = std::collections::HashMap::new();
            for id in &all {
                *counts.entry(tail(id, len)).or_insert(0_usize) += 1;
            }
            page.iter()
                .all(|id| counts.get(tail(id, len)).copied() == Some(1))
        })
        .unwrap_or_else(|| longest.max(MIN_ID_TAIL))
}

/// Die letzten `len` Zeichen einer Id in der Vergleichsform.
fn id_tail(id: &str, len: usize) -> String {
    tail(&hex_id(id), len).to_owned()
}

/// Die letzten `len` Zeichen von `text`, oder `text` selbst, wenn er kürzer
/// ist. Eine Id besteht aus ASCII; geschnitten wird trotzdem an einer
/// Zeichengrenze.
fn tail(text: &str, len: usize) -> &str {
    let skip = text.chars().count().saturating_sub(len);
    text.char_indices()
        .nth(skip)
        .map_or("", |(index, _)| &text[index..])
}

/// Eine Zeile der Liste.
///
/// Ein Strich steht für „der Daemon weiß es nicht", nie für eine Null
/// (`backlog/CONVENTIONS.md` 4.13). `SIZE` ist die Summe aus Anfrage und
/// Antwort in Bytes, `MS` die Dauer in Millisekunden.
fn summary_row(summary: &v1::FlowSummary, id_width: usize) -> Vec<String> {
    vec![
        id_tail(&summary.flow_id, id_width),
        clock(summary.received_at.as_ref().map(|at| at.seconds)),
        state_name(summary.state).to_ascii_lowercase(),
        outcome_name(summary).to_owned(),
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
        vec!["decision".to_owned(), outcome_name(summary).to_owned()],
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
    // Die Notiz steht unmittelbar unter der Entscheidung, zu der sie gehört,
    // und nur dort: Die Liste (`summary_row`) führt sie nicht, weil eine Zeile
    // mit 500 Zeichen Freitext keine Tabelle mehr wäre (HUM-117).
    if !summary.decision_note.is_empty() {
        let after_decision = rows
            .iter()
            .position(|row| row.first().is_some_and(|field| field == "decision"))
            .map_or(rows.len(), |index| index.saturating_add(1));
        rows.insert(
            after_decision,
            vec!["note".to_owned(), summary.decision_note.clone()],
        );
    }
    if !summary.rule_id.is_empty() {
        rows.push(vec!["rule".to_owned(), summary.rule_id.clone()]);
    }
    if !summary.origin_tool.is_empty() {
        rows.push(vec!["tool".to_owned(), summary.origin_tool.clone()]);
    }
    // Woran der Flow scheiterte, mit der Klasse, etwa
    // `tls_handshake_failed:eof_before_finished` (HUM-316). Leer heißt
    // „nichts gescheitert", dann keine Zeile.
    if !summary.error.is_empty() {
        let after_status = rows
            .iter()
            .position(|row| row.first().is_some_and(|field| field == "status"))
            .map_or(rows.len(), |index| index.saturating_add(1));
        rows.insert(
            after_status,
            vec!["error".to_owned(), summary.error.clone()],
        );
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
        // Die registrierbare Domain, wie der Daemon sie kennt, und derselbe
        // Wert, den die Suchzeile `apex:<domain>` vergleicht. Unbekannt ist der
        // leere String, nie `null`, wie bei `origin_tool` und `error`: Ein
        // Skript, das ihn liest, soll nicht zwei Formen von „nichts"
        // unterscheiden muessen (HUM-091).
        "apex": summary.apex,
        // Der Dienst, den der Domain-Katalog des Daemons erkannt hat, als
        // Kennung und nicht als Name: Der Name steht in `catalog/domains.yaml`
        // und kann sich ändern, die Kennung ist der Schlüssel. Unbekannt ist
        // auch hier der leere String (HUM-094). Ohne diese Zeile zeigte der
        // Bildschirm einen Dienst, den `--json` nicht nennt, und die beiden
        // Clients derselben Proto sagten Verschiedenes (ADR-018).
        "catalog_id": summary.catalog_id,
        "duration_ms": summary.duration.as_ref().map(|duration| {
            duration.seconds * 1_000 + i64::from(duration.nanos) / 1_000_000
        }),
        "path": summary.path,
        "state": state_name(summary.state).to_ascii_lowercase(),
        // `decision` bleibt, was der Automat verbucht hat, damit kein Skript
        // bricht, das `block` zählt; `outcome` ist das Wort der Spalte
        // DECISION, mit `aborted` und `passthrough` (HUM-316), und
        // `block_reason` der Grund dahinter, leer ohne Block.
        "decision": decision_name(summary.decision),
        "outcome": outcome_name(summary),
        "block_reason": block_reason_name(summary.block_reason),
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
        // Der Satz, den der Mensch beim Blocken an den Agenten gerichtet hat,
        // so wie der Agent ihn in der 403-Antwort gelesen hat. Der leere
        // String heißt „es gibt keine", nie `null`, wie bei `origin_tool` und
        // `error` (HUM-117).
        "decision_note": summary.decision_note,
        "origin_tool": summary.origin_tool,
        "error": summary.error,
    })
}

/// Die ersten acht Zeichen einer Regel-Id, wie `rules list` sie zeigt.
///
/// Für Flow-Ids nicht mehr: Deren Anfang ist die Uhrzeit (HUM-316), siehe
/// [`id_tail`].
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

/// Das Wort der Spalte DECISION: die Entscheidung, oder was an ihrer Stelle
/// wahr ist (HUM-316).
///
/// `aborted`, wo der Dienst `BLOCK_REASON_CLIENT_ABORTED` meldet: Der Client
/// hat die TLS-Verbindung zum Proxy aufgegeben, bevor es etwas zu entscheiden
/// gab, und `block` behauptete eine Blockierung, die niemand vorgenommen hat.
/// `passthrough` für eine Anfrage, die die Durchreiche zum Modell freigegeben
/// hat; ein Block oder Ablauf an ihr bleibt, was er ist, und ohne
/// Entscheidung steht wie überall ein Strich.
fn outcome_name(summary: &v1::FlowSummary) -> &'static str {
    if summary.block_reason == v1::BlockReason::ClientAborted as i32 {
        return "aborted";
    }
    if summary.passthrough
        && matches!(
            v1::DecisionKind::try_from(summary.decision),
            Ok(v1::DecisionKind::Allow | v1::DecisionKind::AllowEdited)
        )
    {
        return "passthrough";
    }
    decision_name(summary.decision)
}

/// Der Block-Grund, wie ihn der Dienst nennt, klein geschrieben; leer ohne.
fn block_reason_name(reason: i32) -> String {
    match v1::BlockReason::try_from(reason) {
        Ok(v1::BlockReason::Unspecified) | Err(_) => String::new(),
        Ok(reason) => reason
            .as_str_name()
            .trim_start_matches("BLOCK_REASON_")
            .to_ascii_lowercase(),
    }
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
        FLOW_HEADERS, MIN_ID_TAIL, PATH_WIDTH, authority, decision_name, detail_rows, flow_rows,
        full_id, id_tail, id_tail_len, match_tail, method_name, order_by, outcome_name, short_id,
        summary_json, summary_row, tail_query, truncate_middle,
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
            apex: "github.com".to_owned(),
            decision_note: String::new(),
            catalog_id: String::new(),
            unresolved_findings: None,
            send_refusal: None,
        }
    }

    /// `humanitl flows show` druckt die Notiz unter der Entscheidung.
    ///
    /// Die Liste zeigt sie nicht: Eine Spalte mit bis zu 500 Zeichen Freitext
    /// wäre keine Tabelle mehr (HUM-117).
    #[test]
    fn flows_show_prints_the_note() {
        let blocked = v1::FlowSummary {
            state: v1::FlowState::Recorded as i32,
            decision: v1::DecisionKind::Block as i32,
            block_reason: v1::BlockReason::User as i32,
            decision_note: "use PyPI".to_owned(),
            ..summary()
        };
        let rows = detail_rows(&blocked);
        let decision = rows
            .iter()
            .position(|row| row[0] == "decision")
            .unwrap_or_else(|| panic!("no decision row"));
        assert_eq!(rows[decision][1], "block");
        assert_eq!(
            rows[decision + 1],
            vec!["note".to_owned(), "use PyPI".to_owned()],
            "the note belongs under the decision it explains"
        );
        assert_eq!(summary_json(&blocked)["decision_note"], "use PyPI");

        assert!(
            !summary_row(&blocked, MIN_ID_TAIL)
                .iter()
                .any(|cell| cell == "use PyPI"),
            "the list is a table and carries no free text"
        );

        let without = detail_rows(&summary());
        assert!(
            !without.iter().any(|row| row[0] == "note"),
            "no note, no row"
        );
        assert_eq!(summary_json(&summary())["decision_note"], "");
    }

    /// `humanitl flows list --json` zeigt die registrierbare Domain.
    ///
    /// Sie steht neben `authority`, weil sie zum Ziel gehört und weil
    /// die Suchzeile `apex:<domain>` genau gegen diesen String vergleicht
    /// (HUM-091).
    #[test]
    fn summary_json_carries_the_apex() {
        let row = v1::FlowSummary {
            apex: "b.github.io".to_owned(),
            ..summary()
        };
        assert_eq!(summary_json(&row)["apex"], "b.github.io");
    }

    /// Unbekannt ist der leere String, nie `null`.
    #[test]
    fn summary_json_apex_is_empty_string_when_unknown() {
        let row = v1::FlowSummary {
            apex: String::new(),
            ..summary()
        };
        let value = summary_json(&row);
        assert!(
            value["apex"].is_string(),
            "ein Skript soll nicht zwei Formen von nichts unterscheiden muessen"
        );
        assert_eq!(value["apex"], "");
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
        let row = summary_row(&summary(), MIN_ID_TAIL);

        assert_eq!(row.len(), FLOW_HEADERS.len());
        assert_eq!(row[0], "deadbeef", "ID is the random end, not the clock");
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
        let row = summary_row(&summary, MIN_ID_TAIL);

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

    /// Fünf Flows derselben Minute, wie sie der Eigentümer am 2026-10-30 sah:
    /// gleicher Anfang, verschiedenes Ende.
    fn same_minute() -> Vec<v1::FlowSummary> {
        [
            "01a189bd-1111-7000-8000-00000000a001",
            "01a189bd-2222-7000-8000-00000000b002",
            "01a189bd-3333-7000-8000-00000000c003",
            "01a189bd-4444-7000-8000-00000000d004",
            "01a189bd-5555-7000-8000-00000000e005",
        ]
        .iter()
        .map(|id| v1::FlowSummary {
            flow_id: (*id).to_owned(),
            ..summary()
        })
        .collect()
    }

    /// HUM-316: Die Spalte `ID` trennt jede Zeile der Seite von jeder anderen.
    ///
    /// Vorher zeigte sie die ersten acht Zeichen, und die sind bei einer
    /// `UUIDv7` die Uhrzeit: fünfmal `01a189bd`.
    #[test]
    fn the_id_column_tells_every_row_of_a_page_apart() {
        let flows = same_minute();
        let rows = flow_rows(&flows, &flows);
        let ids: std::collections::HashSet<&str> = rows.iter().map(|row| row[0].as_str()).collect();
        assert_eq!(ids.len(), flows.len(), "{rows:?}");
        for (row, flow) in rows.iter().zip(&flows) {
            assert_eq!(row[0].len(), MIN_ID_TAIL, "eight suffice here");
            assert!(
                flow.flow_id.replace('-', "").ends_with(&row[0]),
                "{} is the end of {}",
                row[0],
                flow.flow_id
            );
        }
    }

    /// Teilen zwei Ids die letzten acht Zeichen, wird die ganze Spalte
    /// breiter, bis sie sich unterscheiden, und bleibt bündig.
    #[test]
    fn the_id_column_grows_until_the_ends_differ() {
        let ids = [
            "01a189bd-1111-7000-8000-0000deadbeef",
            "01a189bd-2222-7000-8000-1000deadbeef",
            "01a189bd-3333-7000-8000-00000000c003",
        ];
        assert_eq!(id_tail_len(&ids, &ids), 12);
        assert_eq!(id_tail(ids[0], 12), "0000deadbeef");
        assert_eq!(id_tail(ids[1], 12), "1000deadbeef");
        assert_eq!(id_tail_len(&[], &[]), MIN_ID_TAIL);
        assert_eq!(
            id_tail_len(&[ids[0]], &[ids[0]]),
            MIN_ID_TAIL,
            "the same flow on the page and in the search set is one flow"
        );
    }

    /// Die ganze Id, wie die Tabelle sie druckt (32 Hexziffern), findet
    /// `flows show` in der Form des Daemons.
    #[test]
    fn a_printed_whole_id_finds_the_daemons_form() {
        assert_eq!(
            full_id("01A189BD1111700080000000DEADBEEF").as_deref(),
            Some("01a189bd-1111-7000-8000-0000deadbeef")
        );
        assert_eq!(full_id("0000deadbeef"), None);
    }

    /// Delta-Runde (HUM-316): Eine Zeile, die `flows show` nicht durchsucht,
    /// etwa die älteste mit `--asc`, steht mit ihrer ganzen Id da, denn ein
    /// kurzes Ende fände `flows show` nicht.
    #[test]
    fn a_row_outside_the_search_set_prints_its_whole_id() {
        let old = "01a189bd-1111-7000-8000-0000deadbeef";
        let newer = ["01a189bd-2222-7000-8000-00000000c003"];
        assert_eq!(id_tail_len(&[old], &newer), 32);
        assert_eq!(id_tail(old, 32), "01a189bd1111700080000000deadbeef");
    }

    /// Review Runde 1 (HUM-316): Eine Seite mit einer Zeile, deren letzte
    /// acht Zeichen ein Flow außerhalb der Seite teilt. Die Tabelle druckt
    /// ein längeres Ende, und `flows show` findet mit genau diesem Ende den
    /// Flow der Zeile.
    #[test]
    fn the_id_column_is_unique_among_what_show_searches() {
        let searched = same_minute();
        let page = vec![v1::FlowSummary {
            flow_id: "01a189bd-6666-7000-8000-10000000a001".to_owned(),
            ..summary()
        }];
        let mut all = searched.clone();
        all.extend(page.iter().cloned());

        let rows = flow_rows(&page, &all);
        let printed = &rows[0][0];
        assert_eq!(printed, "10000000a001", "eight would also fit 0000a001");
        let wanted = tail_query(printed).expect("a printed end is a valid query");
        assert_eq!(
            match_tail(&all, printed, &wanted).ok().as_deref(),
            Some("01a189bd-6666-7000-8000-10000000a001")
        );
        // Gegen die Seite allein gemessen wären es acht, und die passten
        // auf zwei Flows.
        let short = tail_query(&rows[0][0][4..]).expect("eight hex digits");
        assert!(match_tail(&all, &rows[0][0][4..], &short).is_err());
    }

    /// `flows show` nimmt das Ende, wie die Tabelle es zeigt, und findet
    /// genau den einen Flow.
    #[test]
    fn flows_show_takes_the_end_the_table_prints() {
        let flows = same_minute();
        let rows = flow_rows(&flows, &flows);
        for (row, flow) in rows.iter().zip(&flows) {
            let wanted = tail_query(&row[0]).expect("a printed end is a valid query");
            let found = match_tail(&flows, &row[0], &wanted)
                .unwrap_or_else(|failure| panic!("{:?}", failure.diagnostic));
            assert_eq!(found, flow.flow_id);
        }
        // Groß geschrieben und mit Bindestrich ist es dasselbe Ende.
        let wanted = tail_query("0000-0000A001").expect("hex with a dash");
        assert_eq!(
            match_tail(&flows, "0000-0000A001", &wanted).ok().as_deref(),
            Some("01a189bd-1111-7000-8000-00000000a001")
        );
    }

    /// Ein Ende, das auf mehrere passt, wird nicht geraten, und eines, das
    /// auf keinen passt, auch nicht.
    #[test]
    fn an_end_that_fits_several_or_none_is_refused() {
        let flows = same_minute();
        let none = tail_query("0000f006").expect("eight hex digits");
        let failure = match_tail(&flows, "0000f006", &none).expect_err("no flow ends so");
        assert_eq!(failure.diagnostic.code.as_str(), "IPC_003");

        let mut twins = same_minute();
        twins[1].flow_id = "01a189bd-9999-7000-8000-00000000a001".to_owned();
        let wanted = tail_query("0000a001").expect("eight hex digits");
        let failure =
            match_tail(&twins, "0000a001", &wanted).expect_err("two flows end in 0000a001");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_004");
        assert!(
            failure
                .diagnostic
                .why
                .contains("01a189bd-9999-7000-8000-00000000a001")
                && failure
                    .diagnostic
                    .why
                    .contains("01a189bd-1111-7000-8000-00000000a001"),
            "{}",
            failure.diagnostic.why
        );

        assert!(
            tail_query("a001").is_err(),
            "shorter than the table ever prints"
        );
        assert!(tail_query("zzzzzzzz").is_err(), "not hex");
    }

    /// HUM-316: Ein abgebrochener Tunnel heißt `aborted`, nicht `block`, in
    /// der Tabelle, in `flows show` und im JSON, und die Ursache steht dabei.
    #[test]
    fn an_aborted_tunnel_reads_aborted_and_names_its_cause() {
        let aborted = v1::FlowSummary {
            method: v1::Method::Connect as i32,
            state: v1::FlowState::Recorded as i32,
            decision: v1::DecisionKind::Block as i32,
            decision_source: v1::DecisionSource::System as i32,
            block_reason: v1::BlockReason::ClientAborted as i32,
            error: "tls_handshake_failed:eof_before_finished".to_owned(),
            ..summary()
        };
        assert_eq!(outcome_name(&aborted), "aborted");
        assert_eq!(summary_row(&aborted, MIN_ID_TAIL)[3], "aborted");

        let rows = detail_rows(&aborted);
        let field = |name: &str| {
            rows.iter()
                .find(|row| row[0] == name)
                .map_or_else(|| panic!("no {name} row in {rows:?}"), |row| row[1].clone())
        };
        assert_eq!(field("decision"), "aborted");
        assert_eq!(field("error"), "tls_handshake_failed:eof_before_finished");

        let json = summary_json(&aborted);
        assert_eq!(json["outcome"], "aborted");
        assert_eq!(json["block_reason"], "client_aborted");
        assert_eq!(json["decision"], "block", "the recorded decision stays");
        assert_eq!(json["error"], "tls_handshake_failed:eof_before_finished");

        // Ein Block ohne diesen Grund bleibt ein Block.
        let blocked = v1::FlowSummary {
            block_reason: v1::BlockReason::NoRoute as i32,
            ..aborted
        };
        assert_eq!(outcome_name(&blocked), "block");
        assert_eq!(summary_json(&blocked)["block_reason"], "no_route");
        assert!(
            !detail_rows(&summary()).iter().any(|row| row[0] == "error"),
            "nothing failed, no error row"
        );
    }

    /// HUM-316: Die Durchreiche zum Modell steht als `passthrough` da; ein
    /// Block an ihr bleibt ein Block.
    #[test]
    fn a_passthrough_flow_reads_passthrough() {
        let model = v1::FlowSummary {
            passthrough: true,
            decision: v1::DecisionKind::Allow as i32,
            decision_source: v1::DecisionSource::Passthrough as i32,
            ..summary()
        };
        assert_eq!(summary_row(&model, MIN_ID_TAIL)[3], "passthrough");
        assert_eq!(summary_json(&model)["outcome"], "passthrough");
        assert_eq!(summary_json(&model)["passthrough"], true);

        let undecided = v1::FlowSummary {
            decision: v1::DecisionKind::Unspecified as i32,
            ..model.clone()
        };
        assert_eq!(outcome_name(&undecided), "-", "no decision yet, a dash");

        let refused = v1::FlowSummary {
            decision: v1::DecisionKind::Block as i32,
            block_reason: v1::BlockReason::Secret as i32,
            ..model
        };
        assert_eq!(outcome_name(&refused), "block");
    }
}
