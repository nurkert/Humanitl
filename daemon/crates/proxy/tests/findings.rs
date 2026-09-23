//! HUM-025 im Proxy-Pfad: Der Scan läuft vor jeder Regel, seine Funde stehen
//! in `Analyzed`, und eine Lücke in der Suche bleibt sichtbar.
//!
//! Drei Aussagen werden hier geprüft, und alle drei sind Sicherheitsaussagen:
//!
//! 1. Was gefunden wurde, steht im Ereignisstrom, bevor jemand entscheidet.
//! 2. Eine nur teilweise durchsuchte Anfrage sieht nie aus wie eine saubere:
//!    `findings_truncated` steht am Datensatz, und der Befund, der die Lücke
//!    erklärt, hängt am selben Flow.
//! 3. `hold.hard_block_checksum_secrets` blockt, ohne zu fragen, und mit dem
//!    Befund `HOLD_004`; dasselbe gilt für eine bearbeitete Fassung, die nach
//!    dem zweiten Scan noch ein bestätigtes Geheimnis trägt (HUM-049).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;

use bytes::Bytes;
use humanitl_core::diagnostics::codes::{FINDINGS_002, FINDINGS_003, HOLD_004};
use humanitl_core::{
    Authority, BlockReason, BodyRef, DecidedFindings, Decision, Diagnostic, Finding, FindingKind,
    FindingLocation, FlowEvent, HostName, HttpRequest, Method, Scheme, Severity, Tier,
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
    // Die Sperre sagt am Flow, warum sie griff und was hilft (HUM-049).
    let refusal = hold_refusals(&events.seen);
    assert_eq!(refusal.len(), 1, "one block, one HOLD_004");
    assert!(
        refusal[0].why.contains("iban in the body") && !refusal[0].why.contains("GB82"),
        "kind and place, never the value: {}",
        refusal[0].why
    );
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

/// Die Fassung des Menschen: dasselbe Ziel wie die gehaltene Anfrage, ein
/// anderer Rumpf.
fn edited_sink(port: u16, body: &'static str) -> HttpRequest {
    let authority = Authority {
        host: HostName::parse("127.0.0.1").unwrap(),
        port,
    };
    HttpRequest::new(Method::POST, Scheme::Http, authority, "/sink")
        .with_body(BodyRef::from_bytes(Bytes::from_static(body.as_bytes())))
}

/// Schickt eine saubere Anfrage, lässt sie mit einer IBAN im Rumpf bearbeitet
/// freigeben, und liefert Status, Ereignisse und Treffer beim Ziel.
async fn send_edited_iban(hard_block: bool) -> (StatusCode, Vec<FlowEvent>, usize) {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(tier1())
        .hard_block_checksum_secrets(hard_block)
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::AllowEdited {
        request: Box::new(edited_sink(upstream.port(), IBAN_BODY)),
    });

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            "nothing to see here",
        ))
        .await;
    let status = response.status();
    events.wait_for("recorded").await;
    assert_eq!(
        events.count("held"),
        1,
        "the clean request was asked about, so the edit is the only way a secret got in"
    );
    (status, events.seen.clone(), upstream.hits())
}

/// Eine bearbeitete Fassung, die ein bestätigtes Geheimnis trägt, geht unter
/// dem Schalter nicht hinaus, obwohl ein Mensch sie freigegeben hat.
///
/// Der gehaltene Rumpf ist sauber; die IBAN steht erst in der Bearbeitung. Der
/// Scan der gehaltenen Fassung sieht sie deshalb nie, und nur der zweite Scan
/// über das, was hinausginge, kann die Sperre auslösen (HUM-049).
#[tokio::test(flavor = "multi_thread")]
async fn an_edited_checksum_secret_is_blocked_when_the_switch_is_on() {
    let (status, seen, hits) = send_edited_iban(true).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(hits, 0, "nothing reached the target");
    assert!(
        !seen.iter().any(|event| event.name() == "forwarded"),
        "the edit was taken back before anything was forwarded"
    );
    let refusal = hold_refusals(&seen);
    assert_eq!(refusal.len(), 1, "the refusal says why: {seen:?}");
    assert_eq!(refusal[0].severity, Severity::Blocking);
    let reasons: Vec<BlockReason> = seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Decided {
                decision: Decision::Block { reason, .. },
                ..
            } => Some(*reason),
            _ => None,
        })
        .collect();
    assert_eq!(reasons, vec![BlockReason::Secret]);
}

/// Ohne den Schalter geht dieselbe Bearbeitung hinaus: Die Sperre ist die des
/// Schalters, nicht eine zweite Meinung über jede Freigabe.
#[tokio::test(flavor = "multi_thread")]
async fn an_edited_checksum_secret_goes_out_when_the_switch_is_off() {
    let (status, seen, hits) = send_edited_iban(false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hits, 1);
    assert!(hold_refusals(&seen).is_empty());
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

/// Eine Mailadresse im Rumpf: ein Fund der Stufe `Regex`, der nie hart blockt.
const EMAIL_BODY: &str = "please write to alice@example.org today";

/// Wie der Mensch die gehaltene Anfrage mit der Mailadresse hinauslässt.
enum Release {
    /// Die Freigabe gehalten, ohne die Pause: nichts bestätigt.
    Hold,
    /// „Trotzdem senden" in der Pause: den einen Fund bestätigt.
    SendAnyway,
    /// Im Editor bearbeitet und mit diesem Rumpf freigegeben.
    Edited(&'static str),
}

/// Schickt die Anfrage mit der Mailadresse, gibt sie auf `release` frei und
/// liefert die Funde aus `Decided` und die Zahl, die die Aufzeichnung führt.
async fn release_the_email(release: Release) -> (DecidedFindings, Option<u32>) {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .scanner(tier1())
        .recording(true)
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = match release {
        Release::Hold => proxy.decide_with(Decision::Allow),
        Release::SendAnyway => proxy.decide_acknowledging(Decision::Allow, vec![0]),
        Release::Edited(body) => proxy.decide_with(Decision::AllowEdited {
            request: Box::new(edited_sink(upstream.port(), body)),
        }),
    };

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            EMAIL_BODY,
        ))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a mail address never blocks"
    );
    let FlowEvent::Recorded { flow_id, .. } = events.wait_for("recorded").await else {
        panic!("recorded carries a flow id");
    };
    let decided = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Decided { findings, .. } => Some(findings.clone()),
            _ => None,
        })
        .expect("a Decided event");
    let recorder = proxy.recorder.as_ref().expect("recording was switched on");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow_id)
        .await
        .expect("the recording is readable")
        .expect("the flow is recorded");
    assert_eq!(
        detail.summary.unresolved_findings, decided.unresolved,
        "the recording says what the event said"
    );
    assert_eq!(
        proxy
            .queue
            .registry()
            .get(flow_id)
            .expect("the flow is in the registry")
            .unresolved_findings,
        decided.unresolved,
        "the live row says what the event said"
    );
    for index in &decided.acknowledged {
        let finding = &detail.findings[usize::try_from(*index).unwrap()];
        assert_eq!(finding.resolved.as_deref(), Some("acknowledged"));
    }
    (decided, detail.summary.unresolved_findings)
}

/// Das `Decided`-Ereignis sagt, mit wie vielen offenen Funden eine Freigabe
/// hinausging (HUM-160): eine über das Halten, keine nach der Pause, und bei
/// einer bearbeiteten Fassung die Zahl des zweiten Scans, nicht die der
/// gehaltenen.
#[tokio::test(flavor = "multi_thread")]
async fn decided_carries_unresolved_findings() {
    let (held, recorded) = release_the_email(Release::Hold).await;
    assert_eq!(
        held,
        DecidedFindings::unresolved(1),
        "the valve acknowledges nothing"
    );
    assert_eq!(recorded, Some(1));

    let (paused, recorded) = release_the_email(Release::SendAnyway).await;
    assert_eq!(
        paused,
        DecidedFindings {
            unresolved: Some(0),
            acknowledged: vec![0],
        }
    );
    assert_eq!(recorded, Some(0));

    let (left, recorded) = release_the_email(Release::Edited("still for alice@example.org")).await;
    assert_eq!(
        left.unresolved,
        Some(1),
        "the mail address was left standing"
    );
    assert_eq!(recorded, Some(1));

    let (replaced, _) = release_the_email(Release::Edited("for [EMAIL_1]")).await;
    assert_eq!(
        replaced.unresolved,
        Some(0),
        "the second scan counts the edited request, not the held one"
    );
}
