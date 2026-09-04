//! Die Prüfungen, die vor jeder Wirkung stehen — einmal für beide Dienste.
//!
//! `humanitl.v1` hat zwei Implementierungen: [`crate::server::IpcServer`] im
//! echten Daemon und [`crate::fake::FakeDaemon`] für Oberfläche und Tests.
//! ADR-0003 und ADR-0018 sagen, dass beide denselben Vertrag erfüllen; ein
//! Client soll den Unterschied nicht sehen können. Genau das lief auseinander:
//! Der echte Dienst wies eine `Decide`-Anfrage ohne Flow-Id mit `IPC_004` und
//! `InvalidArgument` ab, der Fake legte die Regel aus `remember` trotzdem an
//! und antwortete `Ok` mit leerem Ergebnis. Wer die Oberfläche gegen den Fake
//! baut, übt dann gegen ein Verhalten, das der Daemon ablehnt.
//!
//! Dieses Modul hält deshalb die Prüfungen, die keine Welt brauchen: reine
//! Funktionen über den Wire-Nachrichten aus [`crate::v1`], die einen
//! [`Diagnostic`] liefern oder das gelesene Ergebnis. Kein Trait, kein Port —
//! ein Port hätte eine Implementierung je Seite und damit dasselbe Problem
//! eine Ebene höher (`docs/ARCHITECTURE.md` 4: kein Trait ohne zweiten
//! Nutzer). Was hier steht, gibt es genau einmal, und beide Dienste rufen es
//! an derselben Stelle auf.
//!
//! Die Codes stehen in `backlog/CONVENTIONS.md` 4.12: `IPC_004` deckt jede
//! unvollständige oder unlesbare `Decide`-Anfrage, `IPC_002` bleibt allein für
//! `AllowEdited` mit mehr als einem Flow, `IPC_005` gehört den `Rules`- und
//! `ListFlows`-Anfragen. Der Fake meldet dieselben Codes wie der echte Dienst;
//! `daemon/crates/ipc/tests/fake_parity.rs` hält das fest.

use humanitl_core::diagnostics::codes;
use humanitl_core::rule::Rule;
use humanitl_core::{Authority, Decision, Diagnostic, FlowId, Method, Scheme, SessionId, Severity};
use humanitl_proxy::llm_probe::EXAMPLE_ENDPOINT;

use crate::convert;
use crate::v1;

/// Wie eine `Decide`-Anfrage auszuführen ist, nachdem sie gelesen wurde.
#[derive(Debug, Clone, PartialEq)]
pub enum DecidePlan {
    /// Die Anfrage gilt; jeder genannte Flow wird so entschieden.
    Decide(Decision),
    /// `AllowEdited` kam für mehr als einen Flow.
    ///
    /// Der Vertrag verlangt hier ausdrücklich ein Ergebnis je Flow mit
    /// `IPC_002` statt eines Fehlers für den ganzen Aufruf
    /// (`proto/humanitl/v1/humanitl.proto`, `Decide`). Entschieden wird nichts,
    /// und angelegt wird auch nichts: Der Befund steht vor jeder Wirkung.
    RefuseEach(Diagnostic),
}

/// Liest eine `Decide`-Anfrage, bevor irgendetwas geschieht.
///
/// Die Reihenfolge ist Teil des Vertrags und deshalb hier festgelegt: erst die
/// Flow-Ids, dann die Entscheidung, dann die Zahl der Flows. Wer die Regel aus
/// `remember` vorher anlegt, hinterlässt sie auch dann, wenn die Anfrage gar
/// nicht ausführbar war.
///
/// Fail closed an jeder Stelle: eine Anfrage ohne `decision` wird abgelehnt,
/// nicht zu `Allow` ergänzt; eine bearbeitete Anfrage, die sich nicht lesen
/// lässt oder deren Body über `limits.hold_body_cap_bytes` liegt, wird
/// abgelehnt, statt als unbearbeitete durchzugehen — das hieße, etwas
/// weiterzuleiten, was der Mensch so nie gesehen hat
/// (`backlog/CONVENTIONS.md` 4.11).
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_004`, wenn die Anfrage keine Flow-Id trägt, keine
/// Entscheidung, eine unlesbare bearbeitete Anfrage oder einen Body über
/// `body_cap_bytes`.
pub fn decide_plan(
    request: &v1::DecideRequest,
    body_cap_bytes: u64,
) -> Result<DecidePlan, Diagnostic> {
    if request.flow_ids.is_empty() {
        return Err(bad_decide("decide came without a flow id".to_owned()));
    }
    let decision = decision_of(request, body_cap_bytes)?;
    let count = request.flow_ids.len();
    if matches!(decision, Decision::AllowEdited { .. }) && count > 1 {
        return Ok(DecidePlan::RefuseEach(edited_for_many(count)));
    }
    Ok(DecidePlan::Decide(decision))
}

/// Die Entscheidung aus der Anfrage, ohne die Zahl der Flows anzusehen.
fn decision_of(request: &v1::DecideRequest, body_cap_bytes: u64) -> Result<Decision, Diagnostic> {
    match &request.decision {
        Some(v1::decide_request::Decision::Allow(())) => Ok(Decision::Allow),
        Some(v1::decide_request::Decision::Block(block)) => {
            // Die Notiz erreicht den Agenten im 403-Body und im Header
            // `X-Humanitl-Note`; sie wird deshalb gesäubert (HUM-072).
            let note = humanitl_core::block::sanitize_note(&block.note);
            Ok(Decision::Block {
                reason: humanitl_core::BlockReason::User,
                note: (!note.is_empty()).then_some(note),
            })
        }
        Some(v1::decide_request::Decision::AllowEdited(edited)) => {
            let size = u64::try_from(edited.body.len()).unwrap_or(u64::MAX);
            if size > body_cap_bytes {
                return Err(bad_decide(format!(
                    "the edited body is {size} bytes, over limits.hold_body_cap_bytes \
                     ({body_cap_bytes})"
                )));
            }
            let request = convert::request_from_proto(edited).map_err(|error| {
                bad_decide(format!("the edited request is not readable: {error}"))
            })?;
            Ok(Decision::AllowEdited {
                request: Box::new(request),
            })
        }
        None => Err(bad_decide(
            "decide came without a decision; a missing decision is never an allow".to_owned(),
        )),
    }
}

/// Liest eine Flow-Id aus dem Text einer Anfrage.
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_004`. Eine unlesbare Id ist eine unlesbare Anfrage
/// und kein Zustand des Flows; `IPC_003` hieße „der Flow wartet nicht mehr"
/// und behauptete damit, es gäbe ihn (`backlog/CONVENTIONS.md` 4.12).
pub fn flow_id(text: &str) -> Result<FlowId, Diagnostic> {
    FlowId::parse(text).map_err(|error| bad_decide(format!("{text:?} is not a flow id: {error}")))
}

/// Die Prüfsumme aus einem [`v1::BodyRef`].
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_005`, wenn sie keine 32 Bytes hat. Ein gekürzter
/// Hash zeigte auf einen anderen Inhalt oder auf keinen; ihn mit Nullen
/// aufzufüllen, wie der Fake es tat, kann einen fremden Body treffen.
pub fn body_hash(wire: &v1::BodyRef) -> Result<[u8; 32], Diagnostic> {
    <[u8; 32]>::try_from(wire.sha256.as_slice()).map_err(|_error| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!(
                "a body reference carries a sha256 of {} bytes, not 32",
                wire.sha256.len()
            ))
            .build()
    })
}

/// Die Operation einer `Rules`-Anfrage.
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_005`, wenn keine dabei ist. Eine leere Anfrage als
/// `list` zu lesen, wie der Fake es tat, wäre geraten: ein Client, der eine
/// Operation vergisst, bekäme eine Antwort, die nach Erfolg aussieht.
pub fn rules_op(request: &v1::RulesRequest) -> Result<&v1::rules_request::Op, Diagnostic> {
    request.op.as_ref().ok_or_else(|| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why("the rules request carries no operation".to_owned())
            .build()
    })
}

/// Liest eine Regel von der Leitung.
///
/// Der einzige Weg, aus einer [`v1::Rule`] eine [`Rule`] zu machen. Er ist
/// nicht bloß eine Übersetzung, sondern die Stelle, an der zwei Felder
/// erzwungen werden: [`convert::rule_from_proto`] setzt `bundled` und
/// `passthrough_llm` auf `false`, gleich was auf der Leitung stand. Eine
/// mitgelieferte Regel ist unlöschbar, und die Durchreiche zum Sprachmodell
/// wird gestreamt statt gehalten; beides darf sich kein Client selbst geben
/// (`proto/humanitl/v1/rules.proto`, HUM-039). Wer eine Regel ablegt, ohne
/// hier durchzugehen, öffnet genau diese Tür — der Fake tat das in `Rules{add}`
/// und `Rules{update}`, nachdem der Weg über `DecideRequest.remember` schon
/// geschlossen war.
///
/// # Errors
///
/// [`Diagnostic`] mit `RULES_003`, wenn das Host-Muster nicht lesbar ist, sonst
/// mit `IPC_005`. Die Unterscheidung gehört hierher und nicht an die
/// Aufrufstelle: Ein falsches Host-Muster ist ein Fehler in der Regel, alles
/// andere ein Fehler der Anfrage.
pub fn rule(wire: &v1::Rule, session: SessionId) -> Result<Rule, Diagnostic> {
    convert::rule_from_proto(wire, session).map_err(|error| {
        let code = match error {
            convert::RuleError::Host(_) => codes::RULES_003,
            _ => codes::IPC_005,
        };
        Diagnostic::builder(code, Severity::Error)
            .why(format!("the rule is not readable: {error}"))
            .build()
    })
}

/// Die Regel eines Probelaufs (`Rules{dry_run}`).
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_005`, wenn keine dabei ist. Ein Probelauf ohne Regel
/// hat nichts, wogegen er liefe; eine leere Trefferliste zurückzugeben — wie
/// der Fake es tat — sieht aus wie „diese Regel trifft nichts".
pub fn dry_run_rule(request: &v1::rules_request::DryRun) -> Result<&v1::Rule, Diagnostic> {
    request.rule.as_ref().ok_or_else(|| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why("dry_run came without a rule".to_owned())
            .build()
    })
}

/// Die Anfrage einer Regel-Probe (`Rules{test}`): Methode, Schema, Ziel, Pfad.
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_005`, wenn Methode oder URL nicht lesbar sind.
/// Geraten wird nichts: Eine Probe, die auf eine andere Anfrage antwortet als
/// die gemeinte, wäre schlimmer als keine. Der Fake lieferte hier `Ok` mit
/// leerem Ergebnis, was für den Menschen wie „keine Regel trifft" aussieht.
pub fn rule_probe(
    probe: &v1::rules_request::Test,
) -> Result<(Method, Scheme, Authority, String), Diagnostic> {
    let method = convert::method_from_proto(probe.method, "").map_err(|error| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!("the method of the probe is not readable: {error}"))
            .build()
    })?;
    let (scheme, authority, path) = convert::split_url(&probe.url).map_err(|error| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!("{:?} is not a request url: {error}", probe.url))
            .build()
    })?;
    Ok((method, scheme, authority, path))
}

/// Der Sortierschlüssel einer `ListFlows`-Anfrage: Name und Richtung.
///
/// Leer heißt „nach Ankunft, neueste zuerst". Ein Schlüssel, den niemand
/// sortieren kann, wird abgelehnt und nicht durch einen anderen ersetzt: Eine
/// Liste in einer Reihenfolge, die niemand verlangt hat, sähe aus wie die
/// verlangte.
///
/// # Errors
///
/// [`Diagnostic`] mit `IPC_005` und der Liste der gültigen Schlüssel.
pub fn order_by(order_by: &str) -> Result<(&'static str, bool), Diagnostic> {
    let lower = order_by.to_ascii_lowercase();
    let mut words = lower.split_whitespace();
    let key = match words.next() {
        None | Some("received_at" | "ts" | "time") => "received_at",
        Some("host") => "host",
        Some("duration") => "duration",
        Some("size") => "size",
        Some(other) => {
            return Err(Diagnostic::builder(codes::IPC_005, Severity::Error)
                .why(format!(
                    "{other:?} is not a sort key; list_flows sorts by received_at, host, \
                     duration or size"
                ))
                .build());
        }
    };
    let ascending = words.any(|word| word == "asc");
    Ok((key, !ascending))
}

/// Der Endpunkt einer `ProbeLlm`-Anfrage.
///
/// # Errors
///
/// [`Diagnostic`] mit `LLM_007`, wenn der Text leer ist oder keine URL. Der
/// Fake setzte hier den Endpunkt der Sitzung ein und antwortete auf jeden
/// Unsinn mit einer erfundenen Modellliste; wer die Oberfläche dagegen baut,
/// sieht den Fehlerfall nie.
pub fn llm_endpoint(endpoint: &str) -> Result<url::Url, Diagnostic> {
    url::Url::parse(endpoint).map_err(|err| {
        Diagnostic::builder(codes::LLM_007, Severity::Error)
            .why(format!(
                "{endpoint:?} is not a URL Humanitl can read: {err}"
            ))
            .fix(humanitl_core::FixAction::ChangeSetting {
                key: "llm.endpoint".to_owned(),
                value: EXAMPLE_ENDPOINT.to_owned(),
            })
            .build()
    })
}

/// Ein Befund für eine `Decide`-Anfrage, die so nicht gilt.
///
/// `IPC_004` deckt jede Anfrage, die der Daemon nicht ausführen kann: keine
/// Flow-Id, keine Entscheidung, eine unlesbare Flow-Id, eine bearbeitete
/// Anfrage, die sich nicht lesen lässt oder über `limits.hold_body_cap_bytes`
/// liegt. Der einzige Sonderfall mit eigenem Code ist [`edited_for_many`].
fn bad_decide(why: String) -> Diagnostic {
    Diagnostic::builder(codes::IPC_004, Severity::Error)
        .why(why)
        .build()
}

/// Eine bearbeitete Anfrage gilt genau einem Flow.
fn edited_for_many(count: usize) -> Diagnostic {
    Diagnostic::builder(codes::IPC_002, Severity::Error)
        .why(format!(
            "allow_edited came with {count} flow ids; an edited request belongs to exactly one flow"
        ))
        .build()
}
