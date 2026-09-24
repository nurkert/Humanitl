//! HUM-159: Die harte Sperre hält, statt sofort zu blocken, und kein Weg
//! hinaus umgeht sie.
//!
//! Mit `hold.hard_block_checksum_secrets` wird eine Anfrage mit einem
//! prüfsummen-bestätigten Geheimnis gehalten, damit ein Mensch den Wert
//! ersetzen kann. Das öffnet Wege, die die sofortige Sperre vorher von selbst
//! schloss. Jeder davon hat hier einen Test:
//!
//! - ein `Allow` des Menschen, auch mit Bestätigung der Funde und auch im
//!   Namen einer Regel, wird von der Warteschlange zurückgewiesen, und der Flow
//!   wartet weiter;
//! - eine bearbeitete Fassung ohne das Geheimnis geht hinaus;
//! - eine Regel `allow`, die Durchreiche zum Sprachmodell und die
//!   Test-Pipeline, die alles durchlässt, blocken als System;
//! - eine Pipeline, die die Prüfung vergisst, scheitert am Handler.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use bytes::Bytes;
use humanitl_core::diagnostics::codes::HOLD_004;
use humanitl_core::{
    Authority, BlockReason, BodyRef, Decision, DecisionSource, Diagnostic, Finding, FindingKind,
    FindingLocation, Flow, FlowEvent, FlowId, HostName, HttpRequest, Method, RuleId, Scheme,
    Severity, Tier, TransitionInput,
};
use humanitl_findings::ScanReport;
use humanitl_proxy::hold::NotHeld;
use humanitl_proxy::{ConnMeta, FlowPipeline, HoldQueue, Scanner};
use hyper::StatusCode;
use support::{Events, FakeUpstream, Proxy, ProxyBuilder, body_string, post};

/// Eine IBAN mit gültiger Prüfsumme im Rumpf.
const IBAN_BODY: &str = "please wire it to GB82 WEST 1234 5698 7654 32 today";

/// Derselbe Satz, pseudonymisiert: So schickt ihn der Editor hinaus.
const CLEAN_BODY: &str = "please wire it to [IBAN_1] today";

/// Ein Scanner, der in jeder Anfrage eine bestätigte IBAN meldet.
///
/// Unabhängig vom Rumpf, damit Regel und Durchreiche auch an Endpunkten
/// geprüft werden, deren Rumpf der Test nicht wählt.
struct ChecksumScan;

impl Scanner for ChecksumScan {
    fn scan(&self, _request: &HttpRequest, _body: &[u8]) -> ScanReport {
        ScanReport {
            findings: vec![Finding::new(
                FindingKind::Iban,
                0..22,
                FindingLocation::Body,
                Tier::Checksum,
                "GB82 WEST 1234 5698 7654 32",
            )],
            truncated: false,
            diagnostics: Vec::new(),
        }
    }

    fn scan_note(&self, _note: &str) -> Vec<Finding> {
        Vec::new()
    }
}

/// Die echten Detektoren: Nur sie sehen, dass die bearbeitete Fassung sauber
/// ist.
fn tier1() -> Arc<dyn Scanner> {
    Arc::new(
        humanitl_proxy::Tier1Scanner::new(&humanitl_findings::FindingsSettings::default()).unwrap(),
    )
}

/// Die Befunde `HOLD_004` unter `seen`.
fn hold_refusals(seen: &[FlowEvent]) -> Vec<&Diagnostic> {
    seen.iter()
        .filter_map(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => Some(diagnostic.as_ref()),
            _ => None,
        })
        .filter(|diagnostic| diagnostic.code == HOLD_004)
        .collect()
}

/// Die Sperren unter `seen`, mit Grund und Herkunft.
fn blocks(seen: &[FlowEvent]) -> Vec<(BlockReason, DecisionSource)> {
    seen.iter()
        .filter_map(|event| match event {
            FlowEvent::Decided {
                decision: Decision::Block { reason, .. },
                source,
                ..
            } => Some((*reason, *source)),
            _ => None,
        })
        .collect()
}

/// Schickt `body` an `/sink` des Ziels, ohne auf die Antwort zu warten.
async fn send_to_sink(
    proxy: &Proxy,
    port: u16,
    body: &'static str,
) -> tokio::task::JoinHandle<(StatusCode, String)> {
    let mut client = proxy.client().await;
    tokio::spawn(async move {
        let response = client
            .send(post(&format!("http://127.0.0.1:{port}/sink"), body))
            .await;
        let status = response.status();
        (status, body_string(response.into_body()).await)
    })
}

/// Wartet auf `Held` und liefert die Id des gehaltenen Flows.
async fn held_flow(events: &mut Events) -> FlowId {
    let FlowEvent::Held { flow_id, .. } = events.wait_for("held").await else {
        panic!("held carries a flow id");
    };
    flow_id
}

/// Die Zurückweisung eines `Allow`: `HOLD_004` mit Art und Ort, nie der Wert.
fn assert_refused(outcome: Result<(), NotHeld>, id: FlowId, how: &str) {
    let Err(NotHeld::SendRefused {
        id: refused,
        refusal,
    }) = outcome
    else {
        panic!("{how}: an allow under the hard block is refused, got {outcome:?}");
    };
    assert_eq!(refused, id, "{how}");
    assert_eq!(refusal.code, HOLD_004, "{how}");
    assert_eq!(refusal.severity, Severity::Blocking, "{how}");
    assert!(
        refusal.why.contains("iban in the body") && !refusal.why.contains("GB82"),
        "{how}: kind and place, never the value: {}",
        refusal.why
    );
}

/// Mit dem Schalter wird die Anfrage gehalten, die Sperre ist vor dem `Held`
/// angesagt, und kein `Allow` kommt durch: nicht das des Menschen, nicht mit
/// bestätigten Funden, nicht im Namen einer Regel. Der Flow wartet danach
/// weiter, und ein Block beendet ihn.
#[tokio::test(flavor = "multi_thread")]
async fn a_checksum_secret_is_held_and_every_allow_is_refused() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .start()
        .await;
    let mut events = proxy.events();
    let sending = send_to_sink(&proxy, upstream.port(), IBAN_BODY).await;
    let id = held_flow(&mut events).await;

    let queue = &proxy.queue;
    assert_refused(queue.decide(id, Decision::Allow), id, "a plain allow");
    assert_refused(
        queue.decide_acknowledging(id, Decision::Allow, DecisionSource::User, &[0]),
        id,
        "an allow that acknowledges the finding",
    );
    assert_refused(
        queue.decide_as(id, Decision::Allow, DecisionSource::Rule(RuleId::new())),
        id,
        "an allow in the name of a rule",
    );
    assert_eq!(queue.pending_ids(), vec![id], "the flow is still held");

    queue
        .decide(
            id,
            Decision::Block {
                reason: BlockReason::User,
                note: None,
            },
        )
        .unwrap();
    let (status, _) = sending.await.unwrap();
    assert_eq!(status, StatusCode::FORBIDDEN);
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "nothing reached the target");

    // Die Ansage steht vor dem `Held`: Die Oberfläche weiß beim Halten schon,
    // dass „Senden" nicht geht (ADR-018).
    let names = events.names();
    let announced = names.iter().position(|name| *name == "diagnostic").unwrap();
    let held = names.iter().position(|name| *name == "held").unwrap();
    assert!(announced < held, "announced before the hold: {names:?}");
    assert_eq!(hold_refusals(&events.seen).len(), 1, "announced once");
}

/// Pseudonymisiert geht dieselbe Anfrage hinaus: Nach dem zurückgewiesenen
/// `Allow` gibt der Mensch die bearbeitete Fassung frei, der zweite Scan
/// findet nichts mehr, und beim Ziel kommt der saubere Rumpf an.
#[tokio::test(flavor = "multi_thread")]
async fn allow_edited_without_the_secret_goes_out() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(tier1())
        .hard_block_checksum_secrets(true)
        .start()
        .await;
    let mut events = proxy.events();
    let sending = send_to_sink(&proxy, upstream.port(), IBAN_BODY).await;
    let id = held_flow(&mut events).await;

    assert_refused(proxy.queue.decide(id, Decision::Allow), id, "unedited");
    let authority = Authority {
        host: HostName::parse("127.0.0.1").unwrap(),
        port: upstream.port(),
    };
    let edited = HttpRequest::new(Method::POST, Scheme::Http, authority, "/sink").with_body(
        BodyRef::from_bytes(Bytes::from_static(CLEAN_BODY.as_bytes())),
    );
    proxy
        .queue
        .decide(
            id,
            Decision::AllowEdited {
                request: Box::new(edited),
            },
        )
        .expect("an edited release is not refused");

    let (status, body) = sending.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.trim(),
        CLEAN_BODY.len().to_string(),
        "the target received the pseudonymised body"
    );
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 1);
    assert!(blocks(&events.seen).is_empty(), "nothing was taken back");
}

/// Die Regel `allow` für das Ziel, dazu die Gegenprobe ohne Schalter.
fn allow_rule() -> &'static str {
    "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"ip:127.0.0.1\"\n"
}

/// Schickt eine Anfrage mit bestätigter IBAN durch `proxy` und liefert
/// Status, Rumpf und Ereignisse.
async fn send_iban(proxy: &Proxy, url: &str) -> (StatusCode, String, Vec<FlowEvent>) {
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client.send(post(url, IBAN_BODY)).await;
    let status = response.status();
    let body = body_string(response.into_body()).await;
    events.wait_for("recorded").await;
    (status, body, events.seen.clone())
}

/// Die Sperre des Systems an Stelle einer Freigabe: `403` mit dem Satz an den
/// Agenten, nie gehalten, nie weitergeleitet, genau ein `HOLD_004`.
fn assert_system_block(status: StatusCode, body: &str, seen: &[FlowEvent], how: &str) {
    assert_eq!(status, StatusCode::FORBIDDEN, "{how}");
    assert!(
        body.contains("checksum-confirmed secret") && !body.contains("GB82"),
        "{how}: the agent is told why, without the value: {body}"
    );
    let names: Vec<&str> = seen.iter().map(FlowEvent::name).collect();
    assert!(
        !names.contains(&"held"),
        "{how}: nobody is asked: {names:?}"
    );
    assert!(!names.contains(&"forwarded"), "{how}: {names:?}");
    // Nicht einmal für einen Augenblick freigegeben: Die Strategie selbst
    // blockt, nicht erst die zweite Linie im Handler.
    assert!(
        !seen.iter().any(|event| matches!(
            event,
            FlowEvent::Decided {
                decision: Decision::Allow,
                ..
            }
        )),
        "{how}: the strategy never allowed it: {names:?}"
    );
    assert_eq!(
        blocks(seen),
        vec![(BlockReason::Secret, DecisionSource::System)],
        "{how}"
    );
    assert_eq!(hold_refusals(seen).len(), 1, "{how}: {names:?}");
}

/// Eine Regel `allow` lässt eine Anfrage mit bestätigtem Geheimnis nie
/// hinaus: Sie blockt weiter sofort. Ohne Schalter lässt dieselbe Regel sie
/// durch, der Test trifft also die Regel und nicht etwas daneben.
#[tokio::test(flavor = "multi_thread")]
async fn rule_allow_still_blocks_a_checksum_secret() {
    let upstream = FakeUpstream::plain().await;
    let url = format!("http://127.0.0.1:{}/sink", upstream.port());
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .rules(allow_rule())
        .start()
        .await;
    let (status, body, seen) = send_iban(&proxy, &url).await;
    assert_system_block(status, &body, &seen, "rule allow");
    assert_eq!(upstream.hits(), 0);

    let open = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .rules(allow_rule())
        .start()
        .await;
    let (status, _, _) = send_iban(&open, &url).await;
    assert_eq!(status, StatusCode::OK, "without the switch the rule allows");
    assert_eq!(upstream.hits(), 1);
}

/// Die Durchreiche zum Sprachmodell, wie der Adapter sie baut.
fn llm_passthrough(port: u16) -> String {
    format!(
        "version: 1\n\
         rules:\n\
         \x20 - action: allow\n\
         \x20   match:\n\
         \x20     host: \"ip:127.0.0.1\"\n\
         \x20     port: {port}\n\
         \x20     scheme: http\n\
         \x20     method: [POST, GET]\n\
         \x20     path_prefixes: [\"/api/chat\"]\n\
         \x20   allow_private: true\n\
         \x20   passthrough_llm: true\n"
    )
}

/// Die Durchreiche zum Sprachmodell ist eine Regel `allow` und gibt eine
/// Anfrage mit bestätigtem Geheimnis ebenso wenig frei.
#[tokio::test(flavor = "multi_thread")]
async fn the_llm_passthrough_never_releases_a_checksum_secret() {
    let upstream = FakeUpstream::ollama().await;
    let url = format!("http://127.0.0.1:{}/api/chat", upstream.port());
    let rules = llm_passthrough(upstream.port());
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .rules(&rules)
        .start()
        .await;
    let (status, body, seen) = send_iban(&proxy, &url).await;
    assert_system_block(status, &body, &seen, "llm passthrough");
    assert_eq!(upstream.hits(), 0);

    let open = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .rules(&rules)
        .start()
        .await;
    let (status, _, _) = send_iban(&open, &url).await;
    assert_eq!(status, StatusCode::OK, "without the switch it passes");
}

/// Auch die Test-Pipeline, die sonst alles durchlässt, blockt.
#[tokio::test(flavor = "multi_thread")]
async fn the_passthrough_pipeline_never_releases_a_checksum_secret() {
    let upstream = FakeUpstream::plain().await;
    let url = format!("http://127.0.0.1:{}/sink", upstream.port());
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .passthrough()
        .start()
        .await;
    let (status, body, seen) = send_iban(&proxy, &url).await;
    assert_system_block(status, &body, &seen, "passthrough pipeline");
    assert_eq!(upstream.hits(), 0);
}

/// Eine Pipeline, die jeden Flow freigibt und die harte Sperre nicht kennt.
///
/// So sähe eine künftige Strategie aus, die die Prüfung vergisst: Sie
/// entscheidet über den Automaten, korrekt verbucht, aber `Allow`.
struct CarelessPipeline {
    queue: Arc<HoldQueue>,
}

#[async_trait]
impl FlowPipeline for CarelessPipeline {
    async fn decide(&self, flow: &mut Flow, _meta: &ConnMeta) -> Decision {
        let event = flow
            .apply(
                TransitionInput::Decide {
                    decision: Decision::Allow,
                    source: DecisionSource::Passthrough,
                },
                SystemTime::now(),
            )
            .unwrap();
        self.queue.publish(event);
        Decision::Allow
    }
}

/// Die zweite Linie im Handler: Kommt trotzdem ein `Allow` für einen Flow
/// unter der Sperre an, wird es zurückgenommen, bevor etwas hinausgeht.
#[tokio::test(flavor = "multi_thread")]
async fn the_handler_takes_back_an_allow_that_skipped_the_hard_block() {
    let upstream = FakeUpstream::plain().await;
    let url = format!("http://127.0.0.1:{}/sink", upstream.port());
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .pipeline(|queue| Arc::new(CarelessPipeline { queue }))
        .start()
        .await;
    let (status, body, seen) = send_iban(&proxy, &url).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("checksum-confirmed secret"), "{body}");
    assert_eq!(upstream.hits(), 0, "nothing reached the target");
    assert!(!seen.iter().any(|event| event.name() == "forwarded"));
    assert_eq!(
        blocks(&seen),
        vec![(BlockReason::Secret, DecisionSource::System)],
        "the careless allow was taken back by the system"
    );
}

/// Ohne Frist fragt niemand (`ask_mode = none`, `hold.timeout_secs = 0`):
/// Ein Flow unter der harten Sperre endet dann als Sperre des Systems mit
/// Grund `secret` und dem Satz an den Agenten, nicht als Ablauf.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_deadline_a_checksum_secret_is_blocked_as_a_secret() {
    let upstream = FakeUpstream::plain().await;
    let url = format!("http://127.0.0.1:{}/sink", upstream.port());
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .ask(std::time::Duration::ZERO)
        .start()
        .await;
    let (status, body, seen) = send_iban(&proxy, &url).await;
    assert_system_block(status, &body, &seen, "no deadline");
    assert_eq!(upstream.hits(), 0);
}
