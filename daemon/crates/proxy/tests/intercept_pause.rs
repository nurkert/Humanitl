//! HUM-407: Die Pause des Abfangens lässt durch, was sonst wartete, und
//! sonst nichts.
//!
//! - Mit Pause geht eine Anfrage ohne passende Regel ohne Halten durch und
//!   steht mit der Herkunft `paused` im Strom und in der Aufzeichnung.
//! - Eine passende Block-Regel und die harte Sperre geprüfter Geheimnisse
//!   greifen weiter.
//! - Aus der Sandbox lässt sich die Pause nicht setzen: weder über
//!   `humanitl.internal` noch über irgendeine Anfrage durch den Proxy.
//! - Was der Agent während der Pause vom Proxy liest, nennt weder die Pause
//!   noch einen Menschen (HUM-364, HUM-401).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use humanitl_core::{
    BlockReason, Decision, DecisionSource, Finding, FindingKind, FindingLocation, FlowEvent,
    HttpRequest, Tier,
};
use humanitl_findings::ScanReport;
use humanitl_proxy::Scanner;
use humanitl_recorder::FlowQuery;
use hyper::StatusCode;
use support::{FakeUpstream, ProxyBuilder, body_string, get, header, post};

/// Die Regeln der Tests: eine Block-Regel für genau einen Pfad des Ziels.
const BLOCK_RULES: &str = "version: 1\n\
     rules:\n\
     \x20 - action: block\n\
     \x20   match:\n\
     \x20     host: \"ip:127.0.0.1\"\n\
     \x20     path: \"/blocked\"\n";

/// Ein Scanner, der in jeder Anfrage eine bestätigte IBAN meldet.
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

/// Die Entscheidungen im Strom, mit Herkunft.
fn decisions(seen: &[FlowEvent]) -> Vec<(Decision, DecisionSource)> {
    seen.iter()
        .filter_map(|event| match event {
            FlowEvent::Decided {
                decision, source, ..
            } => Some((decision.clone(), *source)),
            _ => None,
        })
        .collect()
}

/// Mit Pause geht eine Anfrage ohne passende Regel sofort durch, ohne `Held`,
/// mit der Herkunft `paused`; die Aufzeichnung führt sie als `paused`.
///
/// Mutationsbeweis: Ohne den Zweig `paused()` in `AskPipeline::decide` wird
/// die Anfrage gehalten und läuft nach der Frist in `504`.
#[tokio::test(flavor = "multi_thread")]
async fn with_pause_an_unmatched_request_passes_and_is_recorded_as_paused() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(500))
        .rules(BLOCK_RULES)
        .recording(true)
        .start()
        .await;
    proxy.pause.pause();
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 1, "the request reached its target");
    assert_eq!(
        events.count("held"),
        0,
        "nothing was held: {:?}",
        events.names()
    );
    assert_eq!(
        decisions(&events.seen),
        vec![(Decision::Allow, DecisionSource::Paused)]
    );

    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let rows = recorder.list_flows(&FlowQuery::new("")).await.unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].paused, "the row says the pause let it through");
    assert_eq!(rows[0].decision.as_deref(), Some("allow"));
}

/// Ohne Pause wartet dieselbe Anfrage: die Gegenprobe zum Test oben.
#[tokio::test(flavor = "multi_thread")]
async fn without_pause_the_same_request_is_held() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .rules(BLOCK_RULES)
        .start()
        .await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1);
    assert_eq!(upstream.hits(), 0);
}

/// Ohne Frist (`ask_mode = none`, `hold.timeout_secs = 0`) wartet nichts, und
/// die Pause ändert nichts daran: Die Anfrage läuft ab wie ohne Pause (Review
/// HUM-407). Die Pause lässt nur durch, was sonst gehalten würde.
///
/// Mutationsbeweis: Prüft der Zweig der Pause die Frist nicht, geht die
/// Anfrage mit `200` hinaus.
#[tokio::test(flavor = "multi_thread")]
async fn with_pause_and_no_deadline_nothing_is_let_through() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::ZERO)
        .rules(BLOCK_RULES)
        .start()
        .await;
    proxy.pause.pause();
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://127.0.0.1:{}/echo", upstream.port())))
        .await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "nothing reached the target");
    assert!(
        !decisions(&events.seen)
            .iter()
            .any(|(_, source)| *source == DecisionSource::Paused),
        "{:?}",
        events.names()
    );
}

/// Eine passende Block-Regel blockt auch während der Pause.
///
/// Mutationsbeweis: Prüft die Pause vor der Regel-Auswertung (etwa am Anfang
/// von `RulesPipeline::decide`), geht `/blocked` mit `200` hinaus.
#[tokio::test(flavor = "multi_thread")]
async fn with_pause_a_block_rule_still_blocks() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(500))
        .rules(BLOCK_RULES)
        .start()
        .await;
    proxy.pause.pause();
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!(
            "http://127.0.0.1:{}/blocked",
            upstream.port()
        )))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "nothing reached the target");
    let decided = decisions(&events.seen);
    assert_eq!(decided.len(), 1);
    assert!(
        matches!(
            decided[0],
            (
                Decision::Block {
                    reason: BlockReason::Rule(_),
                    ..
                },
                DecisionSource::Rule(_)
            )
        ),
        "{decided:?}"
    );
}

/// Die harte Sperre eines geprüften Geheimnisses greift auch während der
/// Pause (HUM-049, HUM-159).
///
/// Mutationsbeweis: Lässt der Zweig der Pause `send_refusal` außer Acht,
/// entscheidet er `Allow` mit `Paused`, und der Automat lehnt das nicht ab;
/// die IBAN geht mit `200` hinaus.
#[tokio::test(flavor = "multi_thread")]
async fn with_pause_a_hard_block_still_blocks() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(500))
        .rules(BLOCK_RULES)
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .start()
        .await;
    proxy.pause.pause();
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            "please wire it to GB82 WEST 1234 5698 7654 32 today",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 0, "nothing reached the target");
    let decided = decisions(&events.seen);
    assert_eq!(decided.len(), 1);
    assert!(
        matches!(
            decided[0],
            (
                Decision::Block {
                    reason: BlockReason::Secret,
                    ..
                },
                DecisionSource::System
            )
        ),
        "{decided:?}"
    );
}

/// Aus der Sandbox lässt sich die Pause nicht setzen: Weder der Meta-Endpunkt
/// `humanitl.internal` noch eine Anfrage durch den Proxy hat einen Weg zu ihr.
/// Danach wird eine Anfrage ohne Regel weiter gehalten, und kein Wechsel
/// steht im Strom.
#[tokio::test(flavor = "multi_thread")]
async fn the_sandbox_cannot_set_the_pause() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .start()
        .await;
    let mut events = proxy.events();

    for path in [
        "/pause",
        "/intercept",
        "/intercept/pause",
        "/intercept?paused=true",
        "/settings/intercept",
        "/resume",
    ] {
        for request in [
            get(&format!("http://humanitl.internal{path}")),
            post(&format!("http://humanitl.internal{path}"), "paused=true"),
        ] {
            let mut client = proxy.client().await;
            let response = client.send(request).await;
            assert_ne!(response.status(), StatusCode::OK, "{path}");
            let _ = body_string(response.into_body()).await;
        }
    }
    // Die Bitte des Agenten ist ein Text für eine Karte, kein Befehl.
    let mut client = proxy.client().await;
    let response = client
        .send(post("http://humanitl.internal/ask", "pause intercept"))
        .await;
    let _ = body_string(response.into_body()).await;
    assert!(!proxy.pause.is_paused(), "no meta request paused anything");

    // Eine gewöhnliche Anfrage mit allem, was nach einem Schalter aussieht.
    let mut client = proxy.client().await;
    let mut request = get(&format!(
        "http://127.0.0.1:{}/echo?intercept=paused",
        upstream.port()
    ));
    request
        .headers_mut()
        .insert("x-humanitl-intercept", "paused".parse().unwrap());
    let response = client.send(request).await;
    assert_eq!(
        response.status(),
        StatusCode::GATEWAY_TIMEOUT,
        "it was held"
    );
    events.wait_for("held").await;
    assert!(!proxy.pause.is_paused());
    events.drain();
    assert_eq!(events.count("intercept"), 0, "{:?}", events.names());
}

/// Die Wörter aus dem Test von HUM-364
/// (`daemon/crates/sandbox/src/agent/skill.rs`,
/// `no_text_tells_the_agent_about_a_person_deciding`), dazu die Pause selbst.
const FORBIDDEN: &[&str] = &[
    "person*",
    "human",
    "humans",
    "mensch*",
    "approv*",
    "freigabe*",
    "review*",
    "prüf*",
    "decide*",
    "entschied*",
    "entscheid*",
    "question*",
    "frage",
    "fragt",
    "held",
    "angehalten",
    "paus*",
    "intercept*",
];

/// Die Wörter von `text`, klein geschrieben; `humanitl` bleibt ein Wort.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Das erste verbotene Wort in `text`, falls eines darin steht.
fn forbidden_word(text: &str) -> Option<String> {
    words(text).into_iter().find(|word| {
        FORBIDDEN.iter().any(|entry| match entry.strip_suffix('*') {
            Some(prefix) => word.starts_with(prefix),
            None => word == entry,
        })
    })
}

/// Kopfzeilen und Rumpf einer Antwort als ein Text.
async fn agent_view(response: hyper::Response<hyper::body::Incoming>) -> String {
    let mut text = format!("{}\n", response.status());
    for (name, value) in response.headers() {
        let _ = writeln!(
            text,
            "{name}: {}",
            String::from_utf8_lossy(value.as_bytes())
        );
    }
    text.push_str(&body_string(response.into_body()).await);
    text
}

/// Was der Agent während der Pause vom Proxy liest — eine durchgelassene
/// Antwort, die Sperre durch eine Regel, die harte Sperre und die Antworten
/// des Meta-Endpunkts —, nennt weder die Pause noch einen Menschen.
#[tokio::test(flavor = "multi_thread")]
async fn agent_visible_output_names_neither_the_pause_nor_a_person() {
    let upstream = FakeUpstream::plain().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(500))
        .rules(BLOCK_RULES)
        .start()
        .await;
    proxy.pause.pause();
    let mut views = Vec::new();

    for url in [
        format!("http://127.0.0.1:{}/echo", upstream.port()),
        format!("http://127.0.0.1:{}/blocked", upstream.port()),
        "http://humanitl.internal/".to_owned(),
        "http://humanitl.internal/rules".to_owned(),
        "http://humanitl.internal/nothing-here".to_owned(),
    ] {
        let mut client = proxy.client().await;
        let response = client.send(get(&url)).await;
        views.push((url, agent_view(response).await));
    }

    let secret = ProxyBuilder::new()
        .ask(Duration::from_millis(500))
        .scanner(Arc::new(ChecksumScan))
        .hard_block_checksum_secrets(true)
        .start()
        .await;
    secret.pause.pause();
    let mut client = secret.client().await;
    let response = client
        .send(post(
            &format!("http://127.0.0.1:{}/sink", upstream.port()),
            "please wire it to GB82 WEST 1234 5698 7654 32 today",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    views.push(("hard block".to_owned(), agent_view(response).await));

    for (what, text) in &views {
        assert_eq!(forbidden_word(text), None, "{what}:\n{text}");
    }
}

/// `/why` zu einer Anfrage, die die Pause durchließ, verrät die Pause nicht
/// (Review HUM-407): Das Ziel `127.0.0.1:9` ist zu, die `502` nennt den Flow,
/// und `/why/<flow>` antwortet mit derselben Zeile wie für eine Anfrage, die
/// ein Mensch freigegeben hat.
///
/// Mutationsbeweis: Schreibt `agent_visible_source` für die Pause `unknown`
/// oder die Herkunft wörtlich (`paused`), weichen die beiden Zeilen ab.
#[tokio::test(flavor = "multi_thread")]
async fn why_for_a_paused_flow_reads_like_a_human_approval() {
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(10))
        .start()
        .await;

    // Eine Anfrage, die ein Mensch freigibt, ohne Pause.
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let approved = tokio::spawn(async move {
        let response = client.send(get("http://127.0.0.1:9/approved")).await;
        let status = response.status();
        let flow = header(&response, "x-humanitl-flow").map(str::to_owned);
        let _ = body_string(response.into_body()).await;
        (status, flow)
    });
    let FlowEvent::Held { flow_id, .. } = events.wait_for("held").await else {
        panic!("held carries a flow id");
    };
    proxy.queue.decide(flow_id, Decision::Allow).unwrap();
    let (status, flow) = approved.await.unwrap();
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let approved_flow = flow.expect("the failure names its flow");
    events.wait_for("recorded").await;

    // Dieselbe Anfrage während der Pause.
    proxy.pause.pause();
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client.send(get("http://127.0.0.1:9/paused")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let paused_flow = header(&response, "x-humanitl-flow")
        .expect("the failure names its flow")
        .to_owned();
    let _ = body_string(response.into_body()).await;
    events.wait_for("recorded").await;
    assert_eq!(
        decisions(&events.seen),
        vec![(Decision::Allow, DecisionSource::Paused)]
    );

    let mut lines = Vec::new();
    for flow in [&approved_flow, &paused_flow] {
        let mut client = proxy.client().await;
        let response = client
            .send(get(&format!("http://humanitl.internal/why/{flow}")))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let text = body_string(response.into_body()).await;
        assert_eq!(forbidden_word(&text), None, "{text}");
        lines.push(text);
    }
    assert_eq!(lines[0], lines[1], "the pause reads like a human approval");
}

/// Der Wortvergleich selbst: Er trifft, was er treffen soll, und lässt
/// `humanitl` stehen.
#[test]
fn the_word_check_finds_what_it_should() {
    assert_eq!(forbidden_word("Blocked by Humanitl."), None);
    assert_eq!(
        forbidden_word("intercept paused"),
        Some("intercept".to_owned())
    );
    assert_eq!(
        forbidden_word("waiting for a human"),
        Some("human".to_owned())
    );
    assert_eq!(forbidden_word("x-humanitl-flow: 1"), None);
}
