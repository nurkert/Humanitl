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

use humanitl_core::diagnostics::codes::{FINDINGS_002, FINDINGS_003};
use humanitl_core::{
    Decision, Diagnostic, Finding, FindingKind, FindingLocation, FlowEvent, HttpRequest, Severity,
    Tier,
};
use humanitl_findings::{FindingsSettings, ScanReport};
use humanitl_proxy::{Scanner, Tier1Scanner};
use hyper::StatusCode;
use support::{FakeUpstream, ProxyBuilder, body_string, get, post};

/// Eine IBAN mit gültiger Prüfsumme: ein Tier-1-Fund, den kein Muster raten
/// muss.
const IBAN_BODY: &str = "please wire it to GB82 WEST 1234 5698 7654 32 today";

/// Ein GitHub-Token in der Form, die der Detektor kennt: `ghp_` und 36
/// Zeichen. Er steht hier in der **Notiz**, nicht in der Anfrage.
const NOTE_TOKEN: &str = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";

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

    /// Diese Attrappe steht für eine Lücke im Anfrage-Scan, nicht für die
    /// Notiz; sie findet dort nichts.
    fn scan_note(&self, _note: &str) -> Vec<Finding> {
        Vec::new()
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

    /// Der prüfsummen-sichere Fund gehört zur Anfrage; die Notiz bleibt
    /// unberührt, damit `hard_block_checksum_secrets` und die Notiz sich in
    /// den Tests nicht vermischen.
    fn scan_note(&self, _note: &str) -> Vec<Finding> {
        Vec::new()
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

/// Der Satz, den der harte Block selbst schreibt, kommt nicht in die Spalte.
///
/// `block_checksum_secret` entscheidet als System (`BlockReason::Secret`,
/// `DecisionSource::System`) und schickt dem Agenten einen Text mit. Der steht
/// in der 403-Antwort, damit der Agent weiß, woran er ist — aber `decision_note`
/// heißt „was der Mensch geschrieben hat", und über diese Anfrage hat kein
/// Mensch etwas geschrieben. Stünde er dort, reiste er als Wort des Menschen
/// weiter: in die Zeilen der Aufzeichnung, in den Rückfall von `/why`, ins
/// History-Detail und in den HAR-Export (HUM-117).
#[tokio::test(flavor = "multi_thread")]
async fn a_hard_blocked_checksum_secret_leaves_no_note_in_the_recording() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .recording(true)
        .start()
        .await;
    let mut events = proxy.events();

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
        "the agent is told why, in the answer: {body}"
    );

    let FlowEvent::Recorded { flow_id, .. } = events.wait_for("recorded").await else {
        panic!("recorded carries a flow id");
    };
    let recorder = proxy.recorder.as_ref().expect("recording was switched on");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow_id)
        .await
        .expect("the recording is readable")
        .expect("the flow is recorded");

    assert_eq!(detail.summary.decision.as_deref(), Some("block"));
    assert_eq!(detail.summary.block_reason.as_deref(), Some("secret"));
    assert_eq!(
        detail.summary.decision_note, None,
        "the machine wrote that sentence, not a person"
    );
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

/// Ein Geheimnis in der Notiz warnt und blockt trotzdem.
///
/// Die Notiz ist der eigene Satz des Menschen und geht im Klartext in die
/// 403-Antwort; steckt ein Token darin, verlässt es mit ihr den Rechner. Genau
/// ein `FINDINGS_003` sagt das, mit Art und Anfang des Funds und nie mit
/// seinem Wert; die Entscheidung fällt unverändert, und die Fundliste der
/// Anfrage bleibt leer, denn ein Fund in der Notiz gehört nicht zur Anfrage
/// (HUM-117).
#[tokio::test(flavor = "multi_thread")]
async fn a_secret_in_the_note_warns_and_still_blocks() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new().scanner(tier1()).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: Some(format!("nimm {NOTE_TOKEN} nicht")),
    });

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/plain", upstream.port())))
        .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(
        body.contains(NOTE_TOKEN),
        "the note goes out as the person wrote it: {body}"
    );
    assert_eq!(upstream.hits(), 0, "the block is a block");

    events.wait_for("recorded").await;
    let diagnostics: Vec<&Diagnostic> = events
        .seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => Some(diagnostic.as_ref()),
            _ => None,
        })
        .filter(|diagnostic| diagnostic.code == FINDINGS_003)
        .collect();
    assert_eq!(diagnostics.len(), 1, "one finding, one diagnostic");
    let found = diagnostics[0];
    assert_eq!(found.severity, Severity::Warning, "a warning, not a block");
    assert!(
        found.why.contains("api_key:github"),
        "the diagnostic names the kind: {}",
        found.why
    );
    assert!(
        !found.why.contains(NOTE_TOKEN) && !found.why.contains("A1b2C3d4"),
        "a finding never carries its value: {}",
        found.why
    );

    // Die Funde am Fluss gehören zur Anfrage. Die trug keinen Token, also
    // bleibt die Liste leer; sonst stünde in der Historie ein Fund in einer
    // Anfrage, die ihn nie enthielt.
    let FlowEvent::Recorded { flow_id, .. } = events
        .seen
        .iter()
        .find(|event| event.name() == "recorded")
        .expect("a recorded event")
    else {
        panic!("recorded carries a flow id");
    };
    let record = proxy
        .queue
        .registry()
        .get(*flow_id)
        .expect("the flow is in the registry");
    assert!(
        !record.findings_truncated,
        "nothing was skipped in the request"
    );
    let analyzed = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Analyzed { findings, .. } => Some(findings.clone()),
            _ => None,
        })
        .expect("an analyzed event");
    assert!(
        analyzed.is_empty(),
        "the request carried no secret: {analyzed:?}"
    );
}
