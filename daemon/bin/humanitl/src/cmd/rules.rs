//! `humanitl rules list|add|update|remove|reorder|dry-run|reload`: der Regelsatz
//! des Daemons, über den `Rules`-RPC.
//!
//! Jedes Unterkommando ist genau ein Aufruf (ADR-018). Was eine Regel ist und
//! ob sie gültig ist, entscheidet der Dienst; hier wird die Wire-Form gebaut,
//! die Antwort ausgerichtet und der Befund des Dienstes unverändert
//! weitergegeben — mit Zeile und Feld, so wie er kam. Eine zweite Regel-Engine
//! in der Kommandozeile gäbe es sonst zweimal, und sie liefen auseinander.
//!
//! # Was die Ausgabe behauptet
//!
//! Nur das, was in der Antwort steht (`backlog/CONVENTIONS.md` 4.13). `add`
//! zeigt die Zeile, die der Daemon danach führt, nicht die, die geschickt
//! wurde; `remove` meldet erst dann eine gelöschte Regel, wenn sie in der
//! Antwort wirklich fehlt; `reload` schreibt den Befund `RULES_011` des
//! Dienstes hin und nicht eine eigene Zusammenfassung; und wo der Daemon
//! nichts weiß, steht ein Strich.
//!
//! # Die Id einer neuen Regel
//!
//! `RulesResponse` trägt den ganzen Regelsatz, sagt aber nicht, welche Zeile
//! neu ist. Damit `add` die angelegte Regel benennen kann, ohne zu raten,
//! vergibt die Kommandozeile die Id vor dem Aufruf und sucht sie danach in der
//! Antwort. Der Vertrag lässt das zu (`rule_id` leer heißt „der Daemon
//! vergibt sie", nicht „nur der Daemon darf"), und eine doppelte Id lehnt der
//! Dienst mit `RULES_007` ab.

use chrono::{DateTime, Local, Utc};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, RuleId, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cli::{RuleArgs, RulesCmd};
use crate::cmd::{Context, EXIT_OK, EXIT_USER, Failure, from_proto, status_diagnostic};
use crate::render::{diagnostic_block, diagnostic_json, table};

/// Die Spalten von `rules list` (`backlog/sprint-2.md`, HUM-065).
const HEADERS: [&str; 8] = [
    "POS", "ACTION", "HOST", "METHODS", "PATH", "EXPIRES", "ORIGIN", "ID",
];

/// Führt `humanitl rules <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `CLI_004` bei einer Regel, die
/// die Kommandozeile nicht in die Wire-Form bringt, `IPC_005` bei einer Id,
/// die der Daemon nicht kennt, und sonst der Befund des Dienstes, zum Beispiel
/// `RULES_003` bei einem Host-Muster oder `RULES_010` bei einer
/// mitgelieferten Regel.
pub async fn run(ctx: &Context, cmd: &RulesCmd) -> Result<u8, Failure> {
    match cmd {
        // Braucht keinen Daemon: es gibt die Operation im Vertrag nicht, und
        // das ist unabhängig davon, ob gerade einer läuft.
        RulesCmd::Test {
            url,
            method,
            upgrade,
        } => Err(test_not_yet(url, method.as_deref(), upgrade.as_deref())),
        RulesCmd::List { all } => list(ctx, *all).await,
        RulesCmd::Add { rule } => add(ctx, rule).await,
        RulesCmd::Update { id, rule } => update(ctx, id, rule).await,
        RulesCmd::Remove { id } => remove(ctx, id).await,
        RulesCmd::Reorder { id, position } => reorder(ctx, id, *position).await,
        RulesCmd::DryRun { rule, scan } => dry_run(ctx, rule, *scan).await,
        RulesCmd::Reload => reload(ctx).await,
    }
}

/// `rules list [--all]`.
async fn list(ctx: &Context, all: bool) -> Result<u8, Failure> {
    let response = call(ctx, v1::rules_request::Op::List(())).await?;
    warnings(ctx, &response);
    if ctx.render.is_json() {
        emit(ctx, &response, Vec::new());
        return Ok(EXIT_OK);
    }
    print_rules(ctx, &response.rules, all);
    Ok(EXIT_OK)
}

/// `rules add`.
async fn add(ctx: &Context, args: &RuleArgs) -> Result<u8, Failure> {
    let mut rule = rule_from_args(args, None)?;
    let id = RuleId::new().to_string();
    rule.rule_id.clone_from(&id);

    let response = call(ctx, v1::rules_request::Op::Add(rule)).await?;
    let added = find(&response, &id).ok_or_else(|| not_reported(&id, "adding"))?;
    warnings(ctx, &response);

    if ctx.render.is_json() {
        emit(ctx, &response, vec![("added", rule_json(added))]);
        return Ok(EXIT_OK);
    }
    print!("{}", table(&HEADERS, &[rule_row(added, Utc::now())]));
    Ok(EXIT_OK)
}

/// `rules update ID`.
async fn update(ctx: &Context, id: &str, args: &RuleArgs) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let before = call_on(&mut client, v1::rules_request::Op::List(())).await?;
    let base = find(&before, id).ok_or_else(|| unknown_rule(id))?.clone();

    let rule = rule_from_args(args, Some(&base))?;
    let response = call_on(&mut client, v1::rules_request::Op::Update(rule)).await?;
    let changed = find(&response, id).ok_or_else(|| not_reported(id, "changing"))?;
    warnings(ctx, &response);

    if ctx.render.is_json() {
        emit(ctx, &response, vec![("updated", rule_json(changed))]);
        return Ok(EXIT_OK);
    }
    print!("{}", table(&HEADERS, &[rule_row(changed, Utc::now())]));
    Ok(EXIT_OK)
}

/// `rules remove ID`.
///
/// Erst nachsehen, dann löschen: ein Daemon, der eine unbekannte Id
/// stillschweigend hinnimmt, dürfte sonst als „gelöscht" gemeldet werden,
/// obwohl nie etwas da war.
async fn remove(ctx: &Context, id: &str) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let before = call_on(&mut client, v1::rules_request::Op::List(())).await?;
    let target = find(&before, id).ok_or_else(|| unknown_rule(id))?.clone();

    let response = call_on(&mut client, v1::rules_request::Op::Remove(id.to_owned())).await?;
    if let Some(still) = find(&response, id) {
        return Err(still_there(still));
    }
    warnings(ctx, &response);

    if ctx.render.is_json() {
        emit(ctx, &response, vec![("removed", rule_json(&target))]);
        return Ok(EXIT_OK);
    }
    ctx.render.line(&format!("removed {id}"));
    Ok(EXIT_OK)
}

/// `rules reorder ID POSITION`.
///
/// Der Vertrag kennt nur die vollständige Reihenfolge (`Reorder
/// .rule_ids_in_order`); die eine verschobene Regel wird deshalb hier in die
/// Liste eingesetzt, die der Daemon gerade gemeldet hat.
async fn reorder(ctx: &Context, id: &str, position: u32) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let before = call_on(&mut client, v1::rules_request::Op::List(())).await?;
    let target = find(&before, id).ok_or_else(|| unknown_rule(id))?.clone();
    if target.bundled {
        // Der Dienst sortiert mitgelieferte Regeln nicht mit und meldet dabei
        // auch nichts. Ohne diese Prüfung stünde hinterher eine Zeile da, die
        // eine Bewegung behauptet, die es nicht gab.
        return Err(bundled_immutable(&target, "moved"));
    }

    let order = order_with(&before.rules, &target, position);
    let response = call_on(
        &mut client,
        v1::rules_request::Op::Reorder(v1::rules_request::Reorder {
            rule_ids_in_order: order,
        }),
    )
    .await?;
    let moved = find(&response, id).ok_or_else(|| not_reported(id, "moving"))?;
    warnings(ctx, &response);

    if ctx.render.is_json() {
        emit(ctx, &response, vec![("moved", rule_json(moved))]);
        return Ok(EXIT_OK);
    }
    if moved.position != position {
        ctx.render.note(&format!(
            "the daemon keeps {id} at position {}, not {position}",
            moved.position
        ));
    }
    print!("{}", table(&HEADERS, &[rule_row(moved, Utc::now())]));
    Ok(EXIT_OK)
}

/// `rules dry-run`.
async fn dry_run(ctx: &Context, args: &RuleArgs, scan: u32) -> Result<u8, Failure> {
    let rule = rule_from_args(args, None)?;
    let response = call(
        ctx,
        v1::rules_request::Op::DryRun(v1::rules_request::DryRun {
            rule: Some(rule),
            limit: scan,
        }),
    )
    .await?;
    warnings(ctx, &response);

    let scanned = response.dry_run_scanned;
    let hits = response.dry_run_matches.len();
    if ctx.render.is_json() {
        emit(
            ctx,
            &response,
            vec![
                ("scanned", json!(scanned)),
                (
                    "matches",
                    Value::Array(
                        response
                            .dry_run_matches
                            .iter()
                            .map(crate::cmd::flows::summary_json)
                            .collect(),
                    ),
                ),
            ],
        );
        return Ok(EXIT_OK);
    }

    ctx.render.line(&format!(
        "{hits} of {scanned} recorded requests would have matched this rule"
    ));
    if scanned == 0 {
        ctx.render
            .note("the daemon has no recorded requests to scan");
    }
    if !response.dry_run_matches.is_empty() {
        crate::cmd::flows::print_flows(&response.dry_run_matches);
    }
    ctx.render
        .note("a match is not a decision: another rule may match first");
    Ok(EXIT_OK)
}

/// `rules reload`.
///
/// Was sich geändert hat, sagt der Daemon mit `RULES_011`; die Kommandozeile
/// schreibt seinen Befund hin und zählt nicht selbst nach.
async fn reload(ctx: &Context) -> Result<u8, Failure> {
    let response = call(ctx, v1::rules_request::Op::Reload(())).await?;
    if ctx.render.is_json() {
        emit(ctx, &response, Vec::new());
        return Ok(EXIT_OK);
    }
    if response.diagnostics.is_empty() {
        ctx.render.line(&format!(
            "the daemon answered without a report of what changed; the rule set has {} rules",
            response.rules.len()
        ));
        return Ok(EXIT_OK);
    }
    for wire in &response.diagnostics {
        print!("{}", wire_block(wire));
    }
    Ok(EXIT_OK)
}

/// Verbindet sich und schickt eine Operation.
async fn call(ctx: &Context, op: v1::rules_request::Op) -> Result<v1::RulesResponse, Failure> {
    let mut client = ctx.connect().await?;
    call_on(&mut client, op).await
}

/// Schickt eine Operation über eine bestehende Verbindung.
///
/// Ein Fehlerbefund in der Antwort wird zum [`Failure`]: der Aufruf hat dann
/// nichts bewirkt, und der Exit-Code folgt dem Code des Befunds.
async fn call_on(
    client: &mut Client,
    op: v1::rules_request::Op,
) -> Result<v1::RulesResponse, Failure> {
    let response = client
        .rules(v1::RulesRequest { op: Some(op) })
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| Failure::new(status_diagnostic(&status, "Rules")))?;
    match first_error(&response) {
        Some(diagnostic) => Err(Failure::new(diagnostic)),
        None => Ok(response),
    }
}

/// Der erste Befund der Antwort, der den Aufruf hat scheitern lassen.
fn first_error(response: &v1::RulesResponse) -> Option<Diagnostic> {
    let wire = response
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity >= v1::Severity::Error as i32)?;
    Some(from_proto(wire).unwrap_or_else(|| unknown_diagnostic(wire)))
}

/// Die Befunde der Antwort, die kein Fehler sind, als Hinweis.
fn warnings(ctx: &Context, response: &v1::RulesResponse) {
    for wire in &response.diagnostics {
        if wire.severity < v1::Severity::Error as i32 {
            ctx.render.note(wire_block(wire).trim_end());
        }
    }
}

/// Ein Befund der Leitung als Block.
fn wire_block(wire: &v1::Diagnostic) -> String {
    from_proto(wire).map_or_else(
        || {
            format!(
                "{}[{}]: {}\n  why: {}\n",
                severity_word(wire.severity),
                wire.code,
                wire.title,
                crate::render::one_line(&wire.why)
            )
        },
        |diagnostic| diagnostic_block(&diagnostic),
    )
}

/// Der JSON-Wert einer Antwort, plus die Felder des Unterkommandos.
fn emit(ctx: &Context, response: &v1::RulesResponse, extra: Vec<(&str, Value)>) {
    let mut value = json!({
        "rules": response.rules.iter().map(rule_json).collect::<Vec<Value>>(),
        "diagnostics": response
            .diagnostics
            .iter()
            .map(wire_json)
            .collect::<Vec<Value>>(),
    });
    if let Some(object) = value.as_object_mut() {
        for (key, item) in extra {
            object.insert((*key).to_owned(), item);
        }
    }
    ctx.render.value(&value);
}

/// Ein Befund der Leitung als JSON.
fn wire_json(wire: &v1::Diagnostic) -> Value {
    from_proto(wire).map_or_else(
        || {
            json!({
                "code": wire.code,
                "severity": severity_word(wire.severity),
                "title": wire.title,
                "why": wire.why,
            })
        },
        |diagnostic| diagnostic_json(&diagnostic),
    )
}

/// Das Wort zu einer Stufe der Leitung.
fn severity_word(severity: i32) -> &'static str {
    match severity {
        1 => "info",
        2 => "warning",
        4 => "blocking",
        _ => "error",
    }
}

/// Die Regel mit dieser Id in der Antwort.
fn find<'a>(response: &'a v1::RulesResponse, id: &str) -> Option<&'a v1::Rule> {
    response.rules.iter().find(|rule| rule.rule_id == id)
}

/// Die Tabelle der Regeln, gefiltert wie `--all` es sagt.
fn print_rules(ctx: &Context, rules: &[v1::Rule], all: bool) {
    let now = Utc::now();
    let rows: Vec<Vec<String>> = rules
        .iter()
        .filter(|rule| all || (!rule.bundled && !is_expired(rule, now)))
        .map(|rule| rule_row(rule, now))
        .collect();
    if rows.is_empty() {
        ctx.render.note(if all {
            "no rules"
        } else {
            "no rules of your own; --all also shows the bundled ones"
        });
        return;
    }
    print!("{}", table(&HEADERS, &rows));
}

/// Eine Zeile der Regel-Tabelle.
///
/// Schema und Port stehen im Feld `HOST`, wie sie in einer URL stünden, und
/// ein `websocket`-Upgrade als `ws` hinter den Methoden: die acht Spalten der
/// Spezifikation dürfen nicht dazu führen, dass zwei verschiedene Regeln
/// gleich aussehen.
fn rule_row(rule: &v1::Rule, now: DateTime<Utc>) -> Vec<String> {
    let matcher = rule.matcher.clone().unwrap_or_default();
    vec![
        if rule.position == 0 {
            "-".to_owned()
        } else {
            rule.position.to_string()
        },
        action_name(rule.action).to_owned(),
        host_cell(&matcher),
        methods_cell(&matcher),
        if matcher.path.is_empty() {
            "*".to_owned()
        } else {
            matcher.path.clone()
        },
        expires_cell(rule, now),
        origin(rule).to_owned(),
        rule.rule_id.clone(),
    ]
}

/// Das Ziel einer Regel, wie es in einer URL stünde.
fn host_cell(matcher: &v1::RuleMatcher) -> String {
    let scheme = scheme_name(matcher.scheme);
    let head = if scheme.is_empty() {
        String::new()
    } else {
        format!("{scheme}://")
    };
    let tail = if matcher.port == 0 {
        String::new()
    } else {
        format!(":{}", matcher.port)
    };
    format!("{head}{}{tail}", matcher.host)
}

/// Die Methoden einer Regel, mit dem Upgrade dahinter.
fn methods_cell(matcher: &v1::RuleMatcher) -> String {
    let mut cell = if matcher.methods.is_empty() {
        "*".to_owned()
    } else {
        method_names(&matcher.methods).join(",")
    };
    if matcher.upgrade == v1::Upgrade::Websocket as i32 {
        cell.push_str(" ws");
    }
    cell
}

/// Die Gültigkeit einer Regel, mit dem Vermerk, wenn sie vorbei ist.
fn expires_cell(rule: &v1::Rule, now: DateTime<Utc>) -> String {
    match expiry_of(rule) {
        None | Some(v1::rule_expiry::Expiry::Never(())) => "never".to_owned(),
        Some(v1::rule_expiry::Expiry::Session(())) => "session".to_owned(),
        Some(v1::rule_expiry::Expiry::At(at)) => {
            let text = local_time(at.seconds);
            if is_expired(rule, now) {
                format!("{text} (expired)")
            } else {
                text
            }
        }
    }
}

/// Ob die Regel abgelaufen ist. Ohne Zeitpunkt läuft sie nie ab.
fn is_expired(rule: &v1::Rule, now: DateTime<Utc>) -> bool {
    match expiry_of(rule) {
        Some(v1::rule_expiry::Expiry::At(at)) => at.seconds <= now.timestamp(),
        _ => false,
    }
}

/// Ein Zeitpunkt als lokale Zeit, oder `-`.
fn local_time(seconds: i64) -> String {
    DateTime::from_timestamp(seconds, 0).map_or_else(
        || "-".to_owned(),
        |time| {
            DateTime::<Local>::from(time)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

/// Die Gültigkeit einer Regel, soweit sie eine trägt.
fn expiry_of(rule: &v1::Rule) -> Option<&v1::rule_expiry::Expiry> {
    rule.expires
        .as_ref()
        .and_then(|wrapper| wrapper.expiry.as_ref())
}

/// Aus welcher der drei Gruppen die Regel stammt.
///
/// Der Vertrag führt die Herkunft nicht als eigenes Feld: `bundled` und
/// `expires` sagen sie schon (`humanitl_ipc::convert::stored_rule_to_proto`).
fn origin(rule: &v1::Rule) -> &'static str {
    if rule.bundled {
        return "bundled";
    }
    match expiry_of(rule) {
        Some(v1::rule_expiry::Expiry::Session(())) => "session",
        _ => "user",
    }
}

/// Eine Regel als JSON, mit jedem Feld, das über die Leitung kam.
///
/// Der Host steht so da, wie die Regel ihn führt (A-Label bei einem
/// internationalisierten Namen); für Augen ist die Tabelle da.
fn rule_json(rule: &v1::Rule) -> Value {
    let matcher = rule.matcher.clone().unwrap_or_default();
    json!({
        "rule_id": rule.rule_id,
        "position": rule.position,
        "action": action_name(rule.action),
        "origin": origin(rule),
        "bundled": rule.bundled,
        "host": matcher.host,
        "methods": method_names(&matcher.methods),
        "path": matcher.path,
        "scheme": scheme_name(matcher.scheme),
        "port": matcher.port,
        "upgrade": upgrade_name(matcher.upgrade),
        "expires": expires_json(rule),
        "expired": is_expired(rule, Utc::now()),
        "note": rule.note,
        "stream": rule.stream,
        "allow_private": rule.allow_private,
        "hit_count": rule.hit_count,
        "created_at": rule.created_at.as_ref().map(|at| at.seconds),
        "created_from_flow_id": rule.created_from_flow_id,
    })
}

/// Die Gültigkeit einer Regel als JSON.
fn expires_json(rule: &v1::Rule) -> Value {
    match expiry_of(rule) {
        None | Some(v1::rule_expiry::Expiry::Never(())) => json!({ "kind": "never" }),
        Some(v1::rule_expiry::Expiry::Session(())) => json!({ "kind": "session" }),
        Some(v1::rule_expiry::Expiry::At(at)) => json!({
            "kind": "at",
            "epoch_seconds": at.seconds,
        }),
    }
}

/// Der Name einer Aktion, klein geschrieben.
fn action_name(action: i32) -> &'static str {
    match v1::RuleAction::try_from(action).unwrap_or(v1::RuleAction::Unspecified) {
        v1::RuleAction::Allow => "allow",
        v1::RuleAction::Block => "block",
        v1::RuleAction::Ask => "ask",
        v1::RuleAction::Redact => "redact",
        v1::RuleAction::Unspecified => "-",
    }
}

/// Die Namen der Methoden einer Bedingung.
fn method_names(methods: &[i32]) -> Vec<String> {
    methods
        .iter()
        .map(|raw| {
            v1::Method::try_from(*raw)
                .unwrap_or(v1::Method::Unspecified)
                .as_str_name()
                .trim_start_matches("METHOD_")
                .to_owned()
        })
        .collect()
}

/// Der Name eines Schemas; leer, wenn die Regel keines verlangt.
fn scheme_name(scheme: i32) -> &'static str {
    match v1::Scheme::try_from(scheme).unwrap_or(v1::Scheme::Unspecified) {
        v1::Scheme::Http => "http",
        v1::Scheme::Https => "https",
        v1::Scheme::Ws => "ws",
        v1::Scheme::Wss => "wss",
        v1::Scheme::Unspecified => "",
    }
}

/// Der Name eines Upgrades.
fn upgrade_name(upgrade: i32) -> &'static str {
    match v1::Upgrade::try_from(upgrade).unwrap_or(v1::Upgrade::Unspecified) {
        v1::Upgrade::Websocket => "websocket",
        v1::Upgrade::None | v1::Upgrade::Unspecified => "none",
    }
}

/// Baut die Wire-Form einer Regel aus den Flags.
///
/// Mit `base` werden nur die genannten Felder ersetzt (`update`); ohne `base`
/// entsteht eine neue Regel, und `--action` und `--host` müssen dabei sein.
/// Geprüft wird hier nur, was die Wire-Form überhaupt tragen kann; ob das
/// Host-Muster gültig ist und ob der Pfad übersetzbar ist, sagt der Daemon.
fn rule_from_args(args: &RuleArgs, base: Option<&v1::Rule>) -> Result<v1::Rule, Failure> {
    let mut rule = base.cloned().unwrap_or_default();
    let mut matcher = rule.matcher.clone().unwrap_or_default();

    match args.action.as_deref() {
        Some(action) => rule.action = action_from_name(action)? as i32,
        None if base.is_none() => {
            return Err(missing("--action", "allow, block, ask or redact"));
        }
        None => {}
    }
    match args.host.as_deref() {
        Some(host) => host.clone_into(&mut matcher.host),
        None if base.is_none() => {
            return Err(missing(
                "--host",
                "a label glob such as **.github.com, or ip:ADDRESS or cidr:ADDRESS/LEN",
            ));
        }
        None => {}
    }

    if !args.method.is_empty() {
        let mut methods = Vec::with_capacity(args.method.len());
        for name in &args.method {
            methods.push(method_from_name(name)? as i32);
        }
        matcher.methods = methods;
    }
    if let Some(path) = args.path.as_deref() {
        path.clone_into(&mut matcher.path);
    }
    if let Some(scheme) = args.scheme.as_deref() {
        matcher.scheme = scheme_from_name(scheme)? as i32;
    }
    if let Some(port) = args.port {
        matcher.port = u32::from(port);
    }
    if let Some(upgrade) = args.upgrade.as_deref() {
        matcher.upgrade = upgrade_from_name(upgrade)? as i32;
    }
    if let Some(expires) = args.expires.as_deref() {
        rule.expires = Some(expiry_from_text(expires)?);
    }
    if let Some(note) = args.note.as_deref() {
        note.clone_into(&mut rule.note);
    }
    if let Some(allow_private) = args.allow_private {
        rule.allow_private = allow_private;
    }
    if let Some(position) = args.position {
        rule.position = position;
    }

    rule.matcher = Some(matcher);
    Ok(rule)
}

/// Die Aktion zu ihrem Namen.
fn action_from_name(name: &str) -> Result<v1::RuleAction, Failure> {
    match name {
        "allow" => Ok(v1::RuleAction::Allow),
        "block" => Ok(v1::RuleAction::Block),
        "ask" => Ok(v1::RuleAction::Ask),
        "redact" => Ok(v1::RuleAction::Redact),
        other => Err(bad_value("--action", other, "allow, block, ask or redact")),
    }
}

/// Die Methode zu ihrem Namen.
///
/// Groß- und Kleinschreibung ist gleich; `clap` lässt nur die neun Methoden
/// aus [`crate::cli::RULE_METHODS`] durch, und `METHOD_OTHER` ist keine davon.
fn method_from_name(name: &str) -> Result<v1::Method, Failure> {
    match name.to_ascii_uppercase().as_str() {
        "GET" => Ok(v1::Method::Get),
        "HEAD" => Ok(v1::Method::Head),
        "POST" => Ok(v1::Method::Post),
        "PUT" => Ok(v1::Method::Put),
        "PATCH" => Ok(v1::Method::Patch),
        "DELETE" => Ok(v1::Method::Delete),
        "OPTIONS" => Ok(v1::Method::Options),
        "CONNECT" => Ok(v1::Method::Connect),
        "TRACE" => Ok(v1::Method::Trace),
        other => Err(bad_value(
            "--method",
            other,
            &crate::cli::RULE_METHODS.join(", "),
        )),
    }
}

/// Das Schema zu seinem Namen.
fn scheme_from_name(name: &str) -> Result<v1::Scheme, Failure> {
    match name {
        "http" => Ok(v1::Scheme::Http),
        "https" => Ok(v1::Scheme::Https),
        "ws" => Ok(v1::Scheme::Ws),
        "wss" => Ok(v1::Scheme::Wss),
        other => Err(bad_value("--scheme", other, "http, https, ws or wss")),
    }
}

/// Das Upgrade zu seinem Namen.
fn upgrade_from_name(name: &str) -> Result<v1::Upgrade, Failure> {
    match name {
        "none" => Ok(v1::Upgrade::None),
        "websocket" => Ok(v1::Upgrade::Websocket),
        other => Err(bad_value("--upgrade", other, "none or websocket")),
    }
}

/// Die Gültigkeit zu ihrem Text: `never`, `session` oder ein Zeitpunkt.
fn expiry_from_text(text: &str) -> Result<v1::RuleExpiry, Failure> {
    let expiry = match text {
        "never" => v1::rule_expiry::Expiry::Never(()),
        "session" => v1::rule_expiry::Expiry::Session(()),
        other => {
            let at = DateTime::parse_from_rfc3339(other).map_err(|error| {
                Failure::new(
                    Diagnostic::builder(codes::CLI_004, Severity::Error)
                        .why(format!(
                            "--expires {other:?} is neither never nor session nor a point in \
                             time in RFC 3339: {error}"
                        ))
                        .fix(FixAction::CopyCommand(
                            "humanitl rules add --expires 2026-12-31T23:59:59Z".to_owned(),
                        ))
                        .build(),
                )
            })?;
            v1::rule_expiry::Expiry::At(prost_types::Timestamp {
                seconds: at.timestamp(),
                nanos: i32::try_from(at.timestamp_subsec_nanos().min(999_999_999)).unwrap_or(0),
            })
        }
    };
    Ok(v1::RuleExpiry {
        expiry: Some(expiry),
    })
}

/// Die vollständige Reihenfolge, in der die Regel an ihrem neuen Platz steht.
///
/// Eine Position zählt nur innerhalb der Gruppe der Regel (Sitzung, dauerhaft,
/// mitgeliefert; `proto/humanitl/v1/rules.proto`). Mitgelieferte Regeln stehen
/// nicht in der Liste: sie werden nicht sortiert.
fn order_with(rules: &[v1::Rule], target: &v1::Rule, position: u32) -> Vec<String> {
    let group = origin(target);
    let mut out = Vec::with_capacity(rules.len());
    for current in ["session", "user"] {
        let mut ids: Vec<String> = rules
            .iter()
            .filter(|rule| !rule.bundled && origin(rule) == current)
            .map(|rule| rule.rule_id.clone())
            .collect();
        if current == group {
            ids.retain(|id| id != &target.rule_id);
            let at = usize::try_from(position.saturating_sub(1))
                .unwrap_or(usize::MAX)
                .min(ids.len());
            ids.insert(at, target.rule_id.clone());
        }
        out.append(&mut ids);
    }
    out
}

/// Ein Flag, das für diesen Aufruf gebraucht wird und fehlt.
fn missing(flag: &str, expected: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!("{flag} is missing; it takes {expected}"))
            .fix(FixAction::CopyCommand(
                "humanitl rules add --help".to_owned(),
            ))
            .build(),
    )
}

/// Ein Wert, den die Wire-Form nicht kennt.
fn bad_value(flag: &str, value: &str, expected: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!("{flag} {value:?} is not one of {expected}"))
            .fix(FixAction::CopyCommand(
                "humanitl rules add --help".to_owned(),
            ))
            .build(),
    )
}

/// Eine Id, die der Regelsatz des Daemons nicht führt.
fn unknown_rule(id: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!(
                "the daemon has no rule {id}; humanitl rules list --all shows the ids it has"
            ))
            .fix(FixAction::CopyCommand(
                "humanitl rules list --all".to_owned(),
            ))
            .build(),
    )
}

/// Eine mitgelieferte Regel, die sich nicht ändern lässt.
fn bundled_immutable(rule: &v1::Rule, what: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::RULES_010, Severity::Error)
            .why(format!(
                "the rule {} is bundled and cannot be {what}; add your own rule with the same \
                 matcher in front of it instead",
                rule.rule_id
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl rules add --action ask --host {} --position 1",
                rule.matcher
                    .as_ref()
                    .map_or("HOST", |matcher| matcher.host.as_str())
            )))
            .build(),
    )
}

/// Eine Regel, die nach dem Löschen noch im Regelsatz steht.
fn still_there(rule: &v1::Rule) -> Failure {
    if rule.bundled {
        return bundled_immutable(rule, "removed");
    }
    Failure::new(
        Diagnostic::builder(codes::CLI_001, Severity::Error)
            .why(format!(
                "the daemon still lists the rule {} after removing it",
                rule.rule_id
            ))
            .build(),
    )
}

/// Eine Regel, die der Daemon nach der Änderung nicht mehr nennt.
fn not_reported(id: &str, what: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_001, Severity::Error)
            .why(format!(
                "the daemon answered {what} the rule {id} without listing it; nothing is claimed \
                 about it, humanitl rules list --all shows what there is"
            ))
            .fix(FixAction::CopyCommand(
                "humanitl rules list --all".to_owned(),
            ))
            .build(),
    )
}

/// Ein Befund der Leitung, dessen Code dieses Binary nicht kennt.
fn unknown_diagnostic(wire: &v1::Diagnostic) -> Diagnostic {
    Diagnostic::builder(codes::CLI_001, Severity::Error)
        .why(format!(
            "the daemon reported {}: {}",
            wire.code,
            crate::render::one_line(&wire.why)
        ))
        .build()
}

/// Der Befund für `humanitl rules test`.
///
/// Das Kommando steht in `backlog/CONVENTIONS.md` 3.8 und in der
/// Spezifikation, aber der Vertrag hat keine Operation, die eine URL gegen den
/// Regelsatz auswertet. Sie hier auszuwerten hieße, die Engine ein zweites Mal
/// zu bauen (ADR-018); solange die RPC fehlt, sagt das Kommando genau das.
fn test_not_yet(url: &str, method: Option<&str>, upgrade: Option<&str>) -> Failure {
    use core::fmt::Write as _;

    // Ein `String` nimmt jedes `write!` an; der `Result` kann nicht scheitern.
    let mut what = format!("humanitl rules test {url}");
    if let Some(method) = method {
        let _ = write!(what, " --method {method}");
    }
    if let Some(upgrade) = upgrade {
        let _ = write!(what, " --upgrade {upgrade}");
    }
    Failure::with_exit(
        Diagnostic::builder(codes::CLI_003, Severity::Error)
            .why(format!(
                "{what} needs an operation of the Rules RPC that evaluates one URL against the \
                 rule set; the contract has list, add, update, remove, reorder, make_permanent, \
                 dry_run and reload, and deciding it in the command line would be a second rule \
                 engine next to the daemon"
            ))
            .fix(FixAction::OpenUrl(format!(
                "{}/issues?q=HUM-065",
                env!("CARGO_PKG_REPOSITORY")
            )))
            .build(),
        EXIT_USER,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::{
        expires_cell, order_with, origin, rule_from_args, rule_json, rule_row, test_not_yet,
    };
    use crate::cli::RuleArgs;
    use crate::cmd::EXIT_USER;

    /// Die Flags eines Aufrufs, alle leer.
    fn args() -> RuleArgs {
        RuleArgs {
            action: None,
            host: None,
            method: Vec::new(),
            path: None,
            scheme: None,
            port: None,
            upgrade: None,
            expires: None,
            note: None,
            position: None,
            allow_private: None,
        }
    }

    /// Eine Regel, wie der Daemon sie meldet.
    fn rule(id: &str, expiry: Option<v1::rule_expiry::Expiry>, bundled: bool) -> v1::Rule {
        v1::Rule {
            rule_id: id.to_owned(),
            action: v1::RuleAction::Allow as i32,
            matcher: Some(v1::RuleMatcher {
                host: "api.github.com".to_owned(),
                ..v1::RuleMatcher::default()
            }),
            expires: expiry.map(|expiry| v1::RuleExpiry {
                expiry: Some(expiry),
            }),
            bundled,
            position: 1,
            ..v1::Rule::default()
        }
    }

    #[test]
    fn a_new_rule_needs_an_action_and_a_host() {
        let failure = rule_from_args(&args(), None).expect_err("without --action");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_004");
        assert!(failure.diagnostic.why.contains("--action"));

        let mut only_action = args();
        only_action.action = Some("allow".to_owned());
        let failure = rule_from_args(&only_action, None).expect_err("without --host");
        assert!(failure.diagnostic.why.contains("--host"));
    }

    #[test]
    fn an_update_keeps_what_the_flags_do_not_name() {
        let base = rule(
            "018f0001-0000-7000-8000-000000000001",
            Some(v1::rule_expiry::Expiry::Session(())),
            false,
        );
        let mut change = args();
        change.action = Some("block".to_owned());

        let updated = rule_from_args(&change, Some(&base)).expect("the change is readable");
        assert_eq!(updated.action, v1::RuleAction::Block as i32);
        assert_eq!(updated.rule_id, base.rule_id);
        assert_eq!(
            updated
                .matcher
                .as_ref()
                .map(|matcher| matcher.host.as_str()),
            Some("api.github.com")
        );
        assert_eq!(origin(&updated), "session");
    }

    #[test]
    fn every_field_of_the_wire_form_comes_from_a_flag() {
        let mut full = args();
        full.action = Some("redact".to_owned());
        full.host = Some("**.github.com".to_owned());
        full.method = vec!["get".to_owned(), "POST".to_owned()];
        full.path = Some("/repos/**".to_owned());
        full.scheme = Some("https".to_owned());
        full.port = Some(8443);
        full.upgrade = Some("websocket".to_owned());
        full.expires = Some("2026-12-31T23:59:59Z".to_owned());
        full.note = Some("only for the release".to_owned());
        full.position = Some(2);
        full.allow_private = Some(true);

        let built = rule_from_args(&full, None).expect("the rule is readable");
        let matcher = built.matcher.clone().expect("a matcher");
        assert_eq!(built.action, v1::RuleAction::Redact as i32);
        assert_eq!(matcher.host, "**.github.com");
        assert_eq!(
            matcher.methods,
            vec![v1::Method::Get as i32, v1::Method::Post as i32]
        );
        assert_eq!(matcher.path, "/repos/**");
        assert_eq!(matcher.scheme, v1::Scheme::Https as i32);
        assert_eq!(matcher.port, 8443);
        assert_eq!(matcher.upgrade, v1::Upgrade::Websocket as i32);
        assert_eq!(built.position, 2);
        assert!(built.allow_private);
        let value = rule_json(&built);
        assert_eq!(value["expires"]["kind"], "at");
        assert_eq!(value["expires"]["epoch_seconds"], 1_798_761_599_i64);
    }

    #[test]
    fn a_time_that_is_no_time_is_cli_004() {
        let mut broken = args();
        broken.action = Some("allow".to_owned());
        broken.host = Some("api.github.com".to_owned());
        broken.expires = Some("tomorrow".to_owned());

        let failure = rule_from_args(&broken, None).expect_err("tomorrow is not RFC 3339");
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_004");
        assert_eq!(failure.exit, EXIT_USER);
    }

    #[test]
    fn the_origin_comes_from_bundled_and_the_expiry() {
        assert_eq!(
            origin(&rule("a", Some(v1::rule_expiry::Expiry::Never(())), true)),
            "bundled"
        );
        assert_eq!(
            origin(&rule(
                "b",
                Some(v1::rule_expiry::Expiry::Session(())),
                false
            )),
            "session"
        );
        assert_eq!(
            origin(&rule("c", Some(v1::rule_expiry::Expiry::Never(())), false)),
            "user"
        );
        // Ohne Angabe gilt `never`, wie im Vertrag.
        assert_eq!(origin(&rule("d", None, false)), "user");
    }

    #[test]
    fn an_expired_rule_says_so_and_a_row_shows_every_column() {
        let now = chrono::Utc::now();
        let mut expired = rule("e", None, false);
        expired.expires = Some(v1::RuleExpiry {
            expiry: Some(v1::rule_expiry::Expiry::At(prost_types::Timestamp {
                seconds: now.timestamp() - 60,
                nanos: 0,
            })),
        });

        assert!(expires_cell(&expired, now).ends_with("(expired)"));

        let mut matcher = expired.matcher.clone().expect("a matcher");
        matcher.scheme = v1::Scheme::Https as i32;
        matcher.port = 8443;
        matcher.upgrade = v1::Upgrade::Websocket as i32;
        matcher.methods = vec![v1::Method::Get as i32];
        expired.matcher = Some(matcher);

        let row = rule_row(&expired, now);
        assert_eq!(row.len(), super::HEADERS.len());
        assert_eq!(row[0], "1", "POS");
        assert_eq!(row[1], "allow", "ACTION");
        assert_eq!(
            row[2], "https://api.github.com:8443",
            "scheme and port belong to the target"
        );
        assert_eq!(row[3], "GET ws", "the upgrade belongs to the methods");
        assert_eq!(row[4], "*", "a rule without a path matches every path");
        assert_eq!(row[6], "user", "ORIGIN");
        assert_eq!(row[7], "e", "the whole id, because remove needs it");
    }

    #[test]
    fn a_position_counts_inside_the_group_of_the_rule() {
        let session_first = rule("s1", Some(v1::rule_expiry::Expiry::Session(())), false);
        let session_second = rule("s2", Some(v1::rule_expiry::Expiry::Session(())), false);
        let user = rule("u1", Some(v1::rule_expiry::Expiry::Never(())), false);
        let bundled = rule("b1", Some(v1::rule_expiry::Expiry::Never(())), true);
        let rules = vec![session_first, session_second.clone(), user.clone(), bundled];

        // Die zweite Sitzungsregel nach vorn: die dauerhafte bleibt dahinter,
        // die mitgelieferte steht gar nicht in der Liste.
        assert_eq!(order_with(&rules, &session_second, 1), ["s2", "s1", "u1"]);
        // Eine Position hinter dem Ende landet am Ende der eigenen Gruppe.
        assert_eq!(order_with(&rules, &user, 99), ["s1", "s2", "u1"]);
    }

    #[test]
    fn rules_test_is_a_diagnostic_that_names_the_call() {
        let failure = test_not_yet("https://evil.example", Some("GET"), None);
        assert_eq!(failure.exit, EXIT_USER);
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_003");
        assert!(failure.diagnostic.why.contains("https://evil.example"));
        assert!(failure.diagnostic.why.contains("--method GET"));
        assert!(failure.diagnostic.why.contains("Rules RPC"));
    }
}
