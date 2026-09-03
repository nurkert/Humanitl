//! HUM-025 im Proxy-Pfad: Der Scan läuft vor jeder Regel, seine Funde stehen
//! in `Analyzed`, und eine Lücke in der Suche bleibt sichtbar.
//!
//! Drei Aussagen werden hier geprüft, und alle drei sind Sicherheitsaussagen:
//!
//! 1. Was gefunden wurde, steht im Ereignisstrom, bevor jemand entscheidet.
//! 2. Eine nur teilweise durchsuchte Anfrage sieht nie aus wie eine saubere:
//!    `findings_truncated` steht am Datensatz, und der Befund, der die Lücke
//!    erklärt, hängt am selben Flow.
//! 3. `hold.hard_block_checksum_secrets` blockt, ohne zu fragen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;

use humanitl_core::diagnostics::codes::FINDINGS_002;
use humanitl_core::{
    Decision, Diagnostic, Finding, FindingKind, FindingLocation, FlowEvent, HttpRequest, Severity,
    Tier,
};
use humanitl_findings::{FindingsSettings, ScanReport};
use humanitl_proxy::{Scanner, Tier1Scanner};
use hyper::StatusCode;
use support::{FakeUpstream, ProxyBuilder, body_string, post};

/// Eine IBAN mit gültiger Prüfsumme: ein Tier-1-Fund, den kein Muster raten
/// muss.
const IBAN_BODY: &str = "please wire it to GB82 WEST 1234 5698 7654 32 today";

/// Die echten Detektoren mit den Vorgabe-Einstellungen.
fn tier1() -> Arc<dyn Scanner> {
    Arc::new(Tier1Scanner::new(&FindingsSettings::default()).unwrap())
}

/// Ein Scanner, der eine Lücke meldet: nichts gefunden, aber auch nicht alles
/// gesehen. Genau der Fall, der nie wie ein Freispruch aussehen darf.
struct PartialScan;

impl Scanner for PartialScan {
    fn scan(&self, _request: &HttpRequest, _body: &[u8]) -> ScanReport {
        ScanReport {
            findings: Vec::new(),
            truncated: true,
            diagnostics: vec![
                Diagnostic::builder(FINDINGS_002, Severity::Warning)
                    .why("the body was larger than limits.preview_cap_bytes".to_owned())
                    .build(),
            ],
        }
    }
}

/// Ein Scanner, der einen prüfsummen-sicheren Fund meldet, ohne einen Body zu
/// brauchen.
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
}

/// Der Fund steht in `Analyzed`, bevor der Flow gehalten wird.
#[tokio::test(flavor = "multi_thread")]
async fn findings_reach_the_analyzed_event_before_the_hold() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().scanner(tier1()).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: None,
    });

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            IBAN_BODY,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    events.wait_for("recorded").await;
    let findings = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Analyzed { findings, .. } => Some(findings.clone()),
            _ => None,
        })
        .expect("an Analyzed event");
    assert_eq!(findings.len(), 1, "the IBAN is found: {findings:?}");
    assert_eq!(findings[0].tier, Tier::Checksum);
    assert_eq!(findings[0].location, FindingLocation::Body);
    let names = events.names();
    let analyzed = names.iter().position(|name| *name == "analyzed").unwrap();
    let held = names.iter().position(|name| *name == "held").unwrap();
    assert!(
        analyzed < held,
        "the finding comes before the hold: {names:?}"
    );
}

/// Eine Lücke im Scan steht am Datensatz und als Befund am selben Flow, direkt
/// nach `Analyzed`.
#[tokio::test(flavor = "multi_thread")]
async fn a_partial_scan_is_never_an_all_clear() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(PartialScan))
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: None,
    });

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            "anything",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let recorded = events.wait_for("recorded").await;
    let FlowEvent::Recorded { flow_id, .. } = recorded else {
        panic!("recorded carries a flow id");
    };
    let record = proxy
        .queue
        .registry()
        .get(flow_id)
        .expect("the flow is in the registry");
    assert!(
        record.findings_truncated,
        "a partly searched request is marked as such"
    );

    let names = events.names();
    let analyzed = names.iter().position(|name| *name == "analyzed").unwrap();
    let diagnostic = names.iter().position(|name| *name == "diagnostic").unwrap();
    assert!(
        analyzed < diagnostic,
        "the finding comes first, then what explains the gap: {names:?}"
    );
    let code = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => Some(diagnostic.code),
            _ => None,
        })
        .expect("a Diagnostic event");
    assert_eq!(code, FINDINGS_002);
}

/// Mit `hold.hard_block_checksum_secrets` wird ein prüfsummen-sicherer Fund
/// geblockt, ohne zu fragen.
#[tokio::test(flavor = "multi_thread")]
async fn a_checksum_secret_is_blocked_when_the_switch_is_on() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            IBAN_BODY,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(
        body.contains("checksum-confirmed secret"),
        "the answer says what happened, without the value: {body}"
    );
    assert!(!body.contains("GB82"), "no value is echoed: {body}");

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0, "nobody is asked");
    assert_eq!(events.count("forwarded"), 0);
    assert_eq!(upstream.hits(), 0);
}

/// Ohne den Schalter bleibt derselbe Fund eine Frage an den Menschen.
#[tokio::test(flavor = "multi_thread")]
async fn a_checksum_secret_is_only_asked_about_when_the_switch_is_off() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: None,
    });

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            IBAN_BODY,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1, "the human sees it");
    assert_eq!(upstream.hits(), 0);
}
