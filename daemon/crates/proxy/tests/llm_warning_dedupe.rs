//! HUM-354: `LLM_005` warnt nur vor Funden, die in der Sitzung noch nicht
//! gemeldet wurden.
//!
//! Agenten schicken mit jedem Zug das ganze Gespräch an das Modell. Ein Fund,
//! der einmal im Kontext steht, steckt danach in jeder Anfrage. Diese Datei
//! prüft über die echte [`RulesPipeline`] und ohne Socket:
//!
//! 1. Zwei Anfragen mit demselben Fund ergeben eine Warnung, nicht zwei.
//! 2. Eine dritte mit einem neuen Fund warnt wieder und nennt „1 new" und die
//!    Zahl aller Funde.
//! 3. Der Satz trägt den Wert nie, und jeder Fund bleibt am Fluss, auch an
//!    der Anfrage, die nicht gewarnt hat.
//! 4. Eine neue Sitzung fängt leer an, und der Start eines Laufs
//!    (`SessionSettings::set`) leert das Gedächtnis der Pipeline im Daemon.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use humanitl_config::Limits;
use humanitl_core::{
    Authority, Decision, DecisionSource, Diagnostic, Finding, FindingKind, FindingLocation, Flow,
    FlowEvent, FlowId, HostName, HttpRequest, Method, Scheme, SessionId, Tier, TransitionInput,
};
use humanitl_proxy::pipeline::ConnectionContext;
use humanitl_proxy::{
    FlowPipeline, HoldQueue, PassthroughPipeline, RulesPipeline, SessionSettings, SessionState,
};

const TOKEN: &str = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
const OTHER: &str = "ghp_Z9y8X7w6V5u4T3s2R1q0P9o8N7m6L5k4J3i2";

/// Die Durchreiche einer Sitzung zu zwei Modellservern auf `127.0.0.1`,
/// Port 8080 und 8081.
fn passthrough_rules() -> humanitl_rules::RuleSet {
    let yaml = "version: 1\n\
                rules:\n\
                \x20 - id: 01920000-0000-7000-8000-0000000000ff\n\
                \x20   action: allow\n\
                \x20   match:\n\
                \x20     host: \"ip:127.0.0.1\"\n\
                \x20     port: 8080\n\
                \x20     scheme: http\n\
                \x20     method: [POST]\n\
                \x20     path_prefixes: [\"/v1/chat/completions\"]\n\
                \x20   allow_private: true\n\
                \x20   passthrough_llm: true\n\
                \x20 - id: 01920000-0000-7000-8000-0000000000fe\n\
                \x20   action: allow\n\
                \x20   match:\n\
                \x20     host: \"ip:127.0.0.1\"\n\
                \x20     port: 8081\n\
                \x20     scheme: http\n\
                \x20     method: [POST]\n\
                \x20     path_prefixes: [\"/v1/chat/completions\"]\n\
                \x20   allow_private: true\n\
                \x20   passthrough_llm: true\n";
    humanitl_rules::parse_rules(yaml)
        .expect("the rules parse")
        .0
}

fn token(value: &str) -> Finding {
    Finding::new(
        FindingKind::ApiKey("github".to_owned()),
        0..value.len(),
        FindingLocation::Body,
        Tier::Checksum,
        value,
    )
}

struct Harness {
    queue: Arc<HoldQueue>,
    pipeline: RulesPipeline,
    settings: Arc<SessionSettings>,
}

impl Harness {
    /// Verdrahtet wie `build_handler` in `humanitld`: Das Gedächtnis kommt aus
    /// den Einstellungen der Sitzung.
    fn new() -> Self {
        let queue = Arc::new(HoldQueue::new(&Limits::default()));
        let rules = Arc::new(RwLock::new(passthrough_rules()));
        let inner: Arc<dyn FlowPipeline> = Arc::new(PassthroughPipeline::new(Arc::clone(&queue)));
        let settings = Arc::new(SessionSettings::default());
        let pipeline = RulesPipeline::new(Arc::clone(&queue), rules, inner)
            .with_warnings(settings.llm_warnings());
        Self {
            queue,
            pipeline,
            settings,
        }
    }

    /// Eine durchgereichte Anfrage mit diesen Funden; liefert die Warnungen
    /// `LLM_005`, die sie auslöst, und die Funde, die am Fluss stehen.
    async fn send(&self, session: SessionId, findings: Vec<Finding>) -> (Vec<Diagnostic>, usize) {
        self.send_to(8080, session, findings).await
    }

    /// Wie [`Harness::send`], an den Modellserver auf `port`.
    async fn send_to(
        &self,
        port: u16,
        session: SessionId,
        findings: Vec<Finding>,
    ) -> (Vec<Diagnostic>, usize) {
        let request = HttpRequest::new(
            Method::POST,
            Scheme::Http,
            Authority::new(HostName::Ip([127, 0, 0, 1].into()), port),
            "/v1/chat/completions",
        );
        let mut flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
        let mut rx = self.queue.subscribe();
        let analyzed = flow
            .apply(TransitionInput::Analyze { findings }, SystemTime::now())
            .unwrap();
        self.queue.publish(analyzed);
        let decision = self
            .pipeline
            .decide(&mut flow, &ConnectionContext::plain(session))
            .await;
        assert_eq!(decision, Decision::Allow, "a finding warns, it never holds");
        let mut warnings = Vec::new();
        let mut recorded = None;
        let mut source = None;
        while let Ok(event) = rx.try_recv() {
            match event {
                FlowEvent::Diagnostic { diagnostic, .. }
                    if diagnostic.code.as_str() == "LLM_005" =>
                {
                    warnings.push(*diagnostic);
                }
                FlowEvent::Analyzed { findings, .. } => recorded = Some(findings.len()),
                FlowEvent::Decided { source: s, .. } => source = Some(s),
                _ => {}
            }
        }
        assert_eq!(source, Some(DecisionSource::Passthrough));
        (warnings, recorded.expect("the findings stay on the flow"))
    }
}

#[tokio::test]
async fn a_repeated_finding_warns_once_and_a_new_one_warns_again() {
    let harness = Harness::new();
    let session = SessionId::new();

    let (first, on_flow) = harness.send(session, vec![token(TOKEN)]).await;
    assert_eq!(first.len(), 1, "the first sighting warns");
    assert_eq!(on_flow, 1);
    assert!(
        first[0].why.contains("at 127.0.0.1 carries")
            && first[0].why.contains(": 1 new, 1 in total"),
        "{}",
        first[0].why
    );

    // Der Agent schickt das Gespräch noch einmal: derselbe Fund, keine
    // Warnung, aber der Fund steht weiter am Fluss.
    // Rot, sobald `warn_about_findings` das Gedächtnis nicht mehr fragt.
    let (second, on_flow) = harness.send(session, vec![token(TOKEN)]).await;
    assert!(
        second.is_empty(),
        "a finding already reported does not warn again: {second:?}"
    );
    assert_eq!(on_flow, 1, "the finding is still recorded on the flow");

    // Ein neuer Fund kommt dazu: wieder eine Warnung, mit 1 neu von 3.
    let (third, on_flow) = harness
        .send(session, vec![token(TOKEN), token(OTHER), token(TOKEN)])
        .await;
    assert_eq!(third.len(), 1, "a new finding warns again");
    assert_eq!(on_flow, 3);
    assert!(
        third[0].why.contains(": 1 new, 3 in total"),
        "{}",
        third[0].why
    );

    for warning in first.iter().chain(&third) {
        assert!(
            !warning.why.contains(TOKEN)
                && !warning.why.contains(OTHER)
                && !warning.why.contains("ghp_"),
            "the warning never carries the value: {}",
            warning.why
        );
    }

    // Eine neue Sitzung fängt leer an.
    let (fresh, _) = harness.send(SessionId::new(), vec![token(TOKEN)]).await;
    assert_eq!(fresh.len(), 1, "the memory resets with the session");
}

/// Der Daemon hat eine Sitzung für seine ganze Lebenszeit, aber jeder Start
/// einer Sandbox ist ein neues Gespräch. Eine Warnung, die im letzten Lauf
/// niemand gesehen hat (Oberfläche zu), darf im nächsten nicht schweigen.
/// Rot, sobald `SessionSettings::set` das Gedächtnis nicht mehr leert oder
/// `with_warnings` es nicht mehr übernimmt.
#[tokio::test]
async fn a_new_run_warns_again_about_the_same_finding() {
    let harness = Harness::new();
    let session = SessionId::new();

    let (first, _) = harness.send(session, vec![token(TOKEN)]).await;
    assert_eq!(first.len(), 1);
    let (quiet, _) = harness.send(session, vec![token(TOKEN)]).await;
    assert!(quiet.is_empty());

    // Der Sandbox-Dienst startet den nächsten Lauf, dieselbe Sitzung.
    harness.settings.set(SessionState::for_config(
        humanitl_config::AskMode::Ui,
        300,
        Some("127.0.0.1:8080".to_owned()),
    ));
    let (again, _) = harness.send(session, vec![token(TOKEN)]).await;
    assert_eq!(again.len(), 1, "a new run starts with an empty memory");
    assert!(
        again[0].why.contains(": 1 new, 1 in total"),
        "{}",
        again[0].why
    );
}

/// Zwei Modellserver auf einem Rechner sind zwei Gespräche: Das Gedächtnis
/// trägt den Port. Rot, sobald der Schlüssel wieder nur der Host ist.
#[tokio::test]
async fn two_model_servers_on_one_host_keep_separate_memories() {
    let harness = Harness::new();
    let session = SessionId::new();
    let (first, _) = harness.send_to(8080, session, vec![token(TOKEN)]).await;
    assert_eq!(first.len(), 1);
    let (other, _) = harness.send_to(8081, session, vec![token(TOKEN)]).await;
    assert_eq!(
        other.len(),
        1,
        "the second server has not been warned about yet"
    );
    let (quiet, _) = harness.send_to(8081, session, vec![token(TOKEN)]).await;
    assert!(quiet.is_empty());
}
