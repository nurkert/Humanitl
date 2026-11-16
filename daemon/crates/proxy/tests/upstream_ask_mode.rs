//! `hold.ask_mode = "upstream"`: Anfragen ohne Regel entscheidet der
//! Entscheidungs-Proxy hinter Humanitl (ADR-0022, HUM-512).
//!
//! Gemessen am Stellvertreter-Proxy aus [`support::stand_in`], hinter dem
//! echten Proxy-Stapel mit Regeln, Warteschlange und Aufzeichnung:
//!
//! - eine Anfrage ohne Regel und eine mit `ask` kommen ohne Halt beim
//!   Proxy an, seine Antwort (auch `403`) erreicht den Agenten, und der
//!   Fluss steht mit der Quelle `upstream` und dem Status im Datensatz;
//! - ein Treffer einer Regel `block` erreicht den Proxy nie;
//! - `allow` geht ebenfalls über den Proxy, im Namen der Regel;
//! - ein prüfsummen-sicheres Geheimnis unter der harten Sperre erreicht den
//!   Proxy nie;
//! - `redact` hält weiter in Humanitl;
//! - die erklärte Durchreiche zum Sprachmodell ist eine Freigabe durch
//!   Regel, vor jeder Block-Regel des Nutzers, und geht wie jede Freigabe
//!   über den Proxy, als `passthrough`, nie als `upstream`.
//!
//! Mutationsbeweise (gegen `daemon/crates/proxy/src/pipeline.rs`):
//!
//! - `Matched { Block }` an die innere Strategie statt zu sperren:
//!   `a_block_rule_never_reaches_the_proxy` wird rot.
//! - `AskPipeline::decide` ohne den Zweig `forwards_upstream`:
//!   `a_request_without_a_rule_reaches_the_proxy_without_a_hold` wird rot.
//! - Der Zweig `forwards_upstream` ohne die Prüfung von `send_refusal`:
//!   `a_checksum_secret_never_reaches_the_proxy` wird rot.
//! - `Matched { Redact }` über `decide` statt `decide_here`:
//!   `a_redact_rule_still_holds_here` wird rot.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::time::Duration;

use humanitl_config::{AskMode, ProxyResolve};
use humanitl_core::{
    BlockReason, Decision, DecisionSource, Finding, FindingKind, FindingLocation, FlowEvent,
    HttpRequest, Tier,
};
use humanitl_findings::ScanReport;
use humanitl_proxy::Scanner;
use humanitl_proxy::egress::from_config;
use humanitl_recorder::FlowQuery;
use hyper::StatusCode;
use support::stand_in::{Behaviour, CaFile, HOST, StandIn, ca_file, config};
use support::{Events, ForeignCa, Proxy, ProxyBuilder, body_string, get};

/// Ein Regelsatz mit je einer Regel jeder Aktion, auf eigenen Namen; alles
/// andere trifft keine Regel.
const RULES: &str = "version: 1\n\
     rules:\n\
     \x20 - action: block\n\
     \x20   match:\n\
     \x20     host: blocked.test\n\
     \x20 - action: allow\n\
     \x20   match:\n\
     \x20     host: allowed.test\n\
     \x20 - action: ask\n\
     \x20   match:\n\
     \x20     host: asked.test\n\
     \x20 - action: redact\n\
     \x20   match:\n\
     \x20     host: redacted.test\n";

/// Ein Scanner, der in jeder Anfrage eine bestätigte IBAN meldet, wie in
/// `hard_block.rs`.
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

/// Stellvertreter, seine CA und der Proxy-Stapel davor, im Modus `upstream`.
struct Setup {
    stand_in: StandIn,
    proxy: Proxy,
    _ca: CaFile,
}

async fn setup(behaviour: Behaviour, hard_block: bool) -> Setup {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let stand_in = StandIn::start(&ca, behaviour).await;
    let config = config(stand_in.addr, &file.path, ProxyResolve::Proxy);
    let egress = from_config(&config, &[], None).unwrap().egress;
    let mut builder = ProxyBuilder::new()
        // Eine Frist, die ein Test bemerkt: Was hier hält, läuft nach 2 s
        // als `504` ab.
        .ask(Duration::from_secs(2))
        .ask_mode(AskMode::Upstream)
        .rules(RULES)
        .allow_private(false)
        .recording(true)
        .egress(egress);
    if hard_block {
        builder = builder
            .scanner(std::sync::Arc::new(ChecksumScan))
            .hard_block_checksum_secrets(true);
    }
    Setup {
        stand_in,
        proxy: builder.start().await,
        _ca: file,
    }
}

/// Entscheidung und Quelle jedes `Decided` unter `seen`.
fn decisions(events: &Events) -> Vec<(Decision, DecisionSource)> {
    events
        .seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Decided {
                decision, source, ..
            } => Some((decision.clone(), *source)),
            _ => None,
        })
        .collect()
}

/// Eine Anfrage ohne Regel wartet nicht in Humanitl (Akzeptanzkriterium 2),
/// sondern kommt beim Proxy an; seine Ablehnung `403` erreicht den Agenten
/// unverändert, und Ereignis und Aufzeichnung tragen die Quelle `upstream`
/// und den Status, keine Freigabe eines Menschen oder einer Regel.
#[tokio::test(flavor = "multi_thread")]
async fn a_request_without_a_rule_reaches_the_proxy_without_a_hold() {
    let setup = setup(
        Behaviour {
            plain_status: 403,
            ..Behaviour::default()
        },
        false,
    )
    .await;
    let mut events = setup.proxy.events();
    let mut client = setup.proxy.client().await;
    let response = client.send(get("http://nobody.test/echo")).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_string(response.into_body()).await,
        "ok",
        "the proxy's own answer, not a block page of Humanitl"
    );
    events.wait_for("recorded").await;
    assert_eq!(
        setup.stand_in.seen().lines,
        ["GET http://nobody.test:80/echo HTTP/1.1"]
    );
    assert_eq!(events.count("held"), 0, "held: {:?}", events.names());
    assert_eq!(
        decisions(&events),
        [(Decision::Allow, DecisionSource::Upstream)]
    );
    assert_eq!(events.statuses(), [403]);

    let recorder = setup.proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let rows = recorder.list_flows(&FlowQuery::new("")).await.unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].upstream, "the row names the decision proxy");
    assert!(!rows[0].paused);
    assert_eq!(rows[0].rule_id, None, "no rule decided");
    assert_eq!(rows[0].held_ms, None, "nothing waited here");
    assert_eq!(rows[0].status, Some(403));
}

/// `ask` heißt mit `upstream`: dort entscheiden, nicht hier warten.
#[tokio::test(flavor = "multi_thread")]
async fn an_ask_rule_is_forwarded_too() {
    let setup = setup(Behaviour::default(), false).await;
    let mut events = setup.proxy.events();
    let mut client = setup.proxy.client().await;
    let response = client.send(get("http://asked.test/echo")).await;
    assert_eq!(response.status(), StatusCode::OK);
    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 0);
    assert_eq!(
        decisions(&events),
        [(Decision::Allow, DecisionSource::Upstream)]
    );
    assert_eq!(setup.stand_in.seen().lines.len(), 1);
}

/// Was eine Regel sperrt, erreicht den Proxy nie (Akzeptanzkriterium 1).
#[tokio::test(flavor = "multi_thread")]
async fn a_block_rule_never_reaches_the_proxy() {
    let setup = setup(Behaviour::default(), false).await;
    let mut events = setup.proxy.events();
    let mut client = setup.proxy.client().await;
    let response = client.send(get("http://blocked.test/echo")).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(
        body_string(response.into_body())
            .await
            .starts_with("Blocked by Humanitl."),
        "Humanitl's own block"
    );
    events.wait_for("recorded").await;
    assert!(
        setup.stand_in.seen().lines.is_empty(),
        "the proxy saw {:?}",
        setup.stand_in.seen().lines
    );
    assert!(matches!(
        decisions(&events).as_slice(),
        [(
            Decision::Block {
                reason: BlockReason::Rule(_),
                ..
            },
            DecisionSource::Rule(_)
        )]
    ));
}

/// `allow` geht ebenfalls über den Proxy, im Namen der Regel.
#[tokio::test(flavor = "multi_thread")]
async fn an_allow_rule_leaves_through_the_proxy_in_the_name_of_the_rule() {
    let setup = setup(Behaviour::default(), false).await;
    let mut events = setup.proxy.events();
    let mut client = setup.proxy.client().await;
    let response = client.send(get("http://allowed.test/echo")).await;
    assert_eq!(response.status(), StatusCode::OK);
    events.wait_for("recorded").await;
    assert_eq!(
        setup.stand_in.seen().lines,
        ["GET http://allowed.test:80/echo HTTP/1.1"]
    );
    assert!(matches!(
        decisions(&events).as_slice(),
        [(Decision::Allow, DecisionSource::Rule(_))]
    ));
}

/// Die letzte Sperre vor dem Senden gilt auch für die Weitergabe: Mit
/// `hold.hard_block_checksum_secrets` sperrt das System eine Anfrage ohne
/// Regel, die ein prüfsummen-sicheres Geheimnis trägt; der Proxy sieht kein
/// Byte davon.
#[tokio::test(flavor = "multi_thread")]
async fn a_checksum_secret_never_reaches_the_proxy() {
    let setup = setup(Behaviour::default(), true).await;
    let mut events = setup.proxy.events();
    let mut client = setup.proxy.client().await;
    let response = client
        .send(support::post(
            "http://nobody.test/sink",
            "wire it to GB82 WEST 1234 5698 7654 32",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    events.wait_for("recorded").await;
    assert!(
        setup.stand_in.seen().lines.is_empty(),
        "the proxy saw {:?}",
        setup.stand_in.seen().lines
    );
    assert!(
        matches!(
            decisions(&events).as_slice(),
            [(
                Decision::Block {
                    reason: BlockReason::Secret,
                    ..
                },
                DecisionSource::System
            )]
        ),
        "{:?}",
        decisions(&events)
    );
}

/// `redact` hält weiter in Humanitl: Das Bearbeiten kann der Proxy dahinter
/// nicht. Ohne Entscheidung läuft die Frist ab, und der Proxy sah nichts.
#[tokio::test(flavor = "multi_thread")]
async fn a_redact_rule_still_holds_here() {
    let setup = setup(Behaviour::default(), false).await;
    let mut events = setup.proxy.events();
    let mut client = setup.proxy.client().await;
    let response = client.send(get("http://redacted.test/echo")).await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1, "{:?}", events.names());
    assert_eq!(events.count("timed_out"), 1, "{:?}", events.names());
    assert!(
        setup.stand_in.seen().lines.is_empty(),
        "the proxy saw {:?}",
        setup.stand_in.seen().lines
    );
}

/// Durch den Tunnel: `CONNECT` mit dem Namen, die Antwort des Proxys im
/// Tunnel erreicht den Agenten, nichts wartet hier.
#[tokio::test(flavor = "multi_thread")]
async fn an_https_request_without_a_rule_reaches_the_proxy_through_the_tunnel() {
    let setup = setup(
        Behaviour {
            tunnel_status: 403,
            ..Behaviour::default()
        },
        false,
    )
    .await;
    let mut events = setup.proxy.events();
    let mut tunnel = setup.proxy.tls_client(HOST, 443).await;
    let request = hyper::Request::builder()
        .uri("/echo")
        .header("host", HOST)
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap();
    let response = tunnel.client.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    events.wait_for("recorded").await;
    let seen = setup.stand_in.seen();
    assert_eq!(seen.lines, ["CONNECT upstream.test:443 HTTP/1.1"]);
    assert_eq!(seen.inner_lines, ["GET /echo HTTP/1.1"]);
    assert_eq!(events.count("held"), 0);
    assert_eq!(
        decisions(&events),
        [(Decision::Allow, DecisionSource::Upstream)]
    );
}

/// Die erklärte Durchreiche zum Sprachmodell steht vor den Block-Regeln des
/// Nutzers (Rang 1) und ist eine Freigabe durch Regel: Mit `upstream` und
/// einem `block **` des Nutzers geht sie trotzdem hinaus, über den Proxy wie
/// jede Freigabe, mit der Quelle `passthrough` und nicht `upstream`. Eine
/// andere Anfrage trifft das `block **` und erreicht den Proxy nie.
#[tokio::test(flavor = "multi_thread")]
async fn the_llm_passthrough_is_an_allow_by_rule_before_block_rules() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let stand_in = StandIn::start(&ca, Behaviour::default()).await;
    let config = config(stand_in.addr, &file.path, ProxyResolve::Proxy);
    let egress = from_config(&config, &[], None).unwrap().egress;
    let user = "version: 1\n\
         rules:\n\
         \x20 - action: block\n\
         \x20   match:\n\
         \x20     host: \"**\"\n";
    let bundled = "version: 1\n\
         rules:\n\
         \x20 - action: allow\n\
         \x20   match:\n\
         \x20     host: model.test\n\
         \x20     scheme: http\n\
         \x20     method: [POST, GET]\n\
         \x20     path_prefixes: [\"/api/chat\"]\n\
         \x20   passthrough_llm: true\n";
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(2))
        .ask_mode(AskMode::Upstream)
        .rules_store(user, bundled)
        .allow_private(false)
        .egress(egress)
        .start()
        .await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client.send(get("http://model.test/api/chat")).await;
    assert_eq!(response.status(), StatusCode::OK);
    events.wait_for("recorded").await;
    assert_eq!(
        decisions(&events),
        [(Decision::Allow, DecisionSource::Passthrough)]
    );
    assert_eq!(
        stand_in.seen().lines,
        ["GET http://model.test:80/api/chat HTTP/1.1"]
    );

    let response = client.send(get("http://other.test/x")).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(stand_in.seen().lines.len(), 1, "the block never reached it");
}
