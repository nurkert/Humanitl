//! Die abgelehnte private Adresse bekommt einen Befund (HUM-102, ADR-006).
//!
//! Drei Dinge werden hier gemessen:
//!
//! 1. Jede Ablehnung wegen einer privaten Zieladresse erzeugt genau einen
//!    Befund `PROXY_008`, mit der Adresse im `why` und einer
//!    `FixAction::AddRule`, die Host, Port, Schema, Methode und Pfadpräfix der
//!    gescheiterten Anfrage trägt.
//! 2. Weder der Rumpf der `502`-Antwort noch eine Kopfzeile trägt die Adresse.
//!    Die Sandbox hat keinen Resolver; die Zuordnung von Name zu privater
//!    Adresse wäre für den Agenten neue Information über das lokale Netz.
//! 3. Der Vorschlag ist anwendbar: Er übersteht `serialize_rules` und
//!    `parse_rules`, er trifft danach den gescheiterten `RequestKey`, und im
//!    laufenden Proxy öffnet er das Ziel, ohne die Aufsicht aufzugeben — die
//!    Anfrage wird weiterhin jedes Mal gehalten.
//!
//! Der dritte Punkt hängt an der Behebung in `pipeline.rs`: Vor HUM-102 wurde
//! `flow.allow_private` nur im Zweig `Action::Allow` gesetzt, eine Regel mit
//! `action: ask` und `allow_private: true` war also wirkungslos, und der
//! Vorschlag hätte zu einem dauerhaften `allow` verleitet — mehr Öffnung als
//! die Freigabe, die gerade gescheitert war.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};

use chrono::Utc;
use humanitl_core::{
    Action, Authority, Decision, Diagnostic, DiagnosticCode, FixAction, FlowEvent, HeaderMap,
    HostName, HostPattern, HttpRequest, Matcher, Method, Rule, RuleId, Scheme, SessionId, Severity,
    Upgrade,
};
use humanitl_proxy::handler::{NoRule, private_address_refused, private_address_rule};
use humanitl_rules::{RequestKey, RuleSet, Verdict, parse_rules, serialize_rules};
use hyper::StatusCode;

use support::{ECHO_BODY, Events, FakeUpstream, Proxy, ProxyBuilder, body_string, get};

/// Die Adresse, die in allen Tests dieser Datei „privat" heißt.
const PRIVATE: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));

/// Ein Regelsatz, der genau diesen Host erlaubt.
fn allow_rule(host: &str) -> String {
    format!("version: 1\nrules:\n  - action: allow\n    match:\n      host: {host}\n")
}

/// Der erste Befund im Strom, mit seiner Regel.
async fn next_diagnostic(events: &mut Events) -> Diagnostic {
    match events.wait_for("diagnostic").await {
        FlowEvent::Diagnostic { diagnostic, .. } => *diagnostic,
        other => panic!("expected a Diagnostic event, got {other:?}"),
    }
}

/// Die Regel aus einer `FixAction::AddRule`.
fn suggested_rule(diagnostic: &Diagnostic) -> Rule {
    match &diagnostic.fix {
        Some(FixAction::AddRule(rule)) => (**rule).clone(),
        other => panic!("the finding must suggest a rule, got {other:?}"),
    }
}

/// Die Regel, die der Builder zu dieser Anfrage vorschlägt.
fn rule_for(request: &HttpRequest) -> Rule {
    private_address_rule(request).expect("this request has a suggestion")
}

/// Alle Befunde, die der Test bisher gesehen hat.
fn diagnostic_codes(events: &Events) -> Vec<DiagnosticCode> {
    events
        .seen
        .iter()
        .filter_map(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => Some(diagnostic.code),
            _ => None,
        })
        .collect()
}

/// Ein Proxy, der `host` auf eine private Adresse auflöst und jeden gehaltenen
/// Fluss freigibt — also genau der Fall aus dem Issue: Ein Mensch erlaubt, und
/// die Anfrage scheitert trotzdem.
async fn proxy_refusing(host: &str, ip: IpAddr) -> Proxy {
    ProxyBuilder::new()
        .allow_private(false)
        .resolve_host(host, vec![ip])
        .start()
        .await
}

// ---------------------------------------------------------------------------
// 1. Der Befund
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_private_address_yields_exactly_one_proxy_008() {
    let proxy = proxy_refusing("lan.example", PRIVATE).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client
        .send(get("http://lan.example/metrics?token=abc"))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let diagnostic = next_diagnostic(&mut events).await;
    events.wait_for("recorded").await;
    assert_eq!(
        diagnostic_codes(&events),
        vec![diagnostic.code],
        "exactly one finding, and it is the one under test"
    );
    assert_eq!(diagnostic.code.as_str(), "PROXY_008");
    assert!(
        diagnostic.why.contains("10.0.0.5") && diagnostic.why.contains("lan.example"),
        "the finding names the address and the host: {}",
        diagnostic.why
    );

    // Der Vorschlag: Ziel geoeffnet, Aufsicht behalten.
    let rule = suggested_rule(&diagnostic);
    assert_eq!(rule.action, Action::Ask);
    assert!(rule.allow_private);
    assert_eq!(
        rule.matcher.host,
        HostPattern::Exact(HostName::Dns("lan.example".to_owned())),
        "a name stays a name; the address belongs in the finding, not in rules.yaml"
    );
    assert_eq!(rule.matcher.port, Some(80));
    assert_eq!(rule.matcher.scheme, Some(Scheme::Http));
    assert_eq!(rule.matcher.methods, Some(vec![Method::GET]));
    assert_eq!(
        rule.matcher.path_prefixes,
        vec!["/metrics".to_owned()],
        "the query is stripped: a token has no business in rules.yaml"
    );
    assert!(
        !rule.note.unwrap_or_default().contains("10.0.0.5"),
        "the note reaches the agent through the meta endpoint; the address does not"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ip_literal_is_refused_with_a_rule_on_the_address() {
    // Ein Literal wird nicht aufgeloest, sondern nur geprueft; der Befund
    // entsteht trotzdem, weil er an `record_failure` haengt und nicht am
    // DNS-Zweig.
    let proxy = ProxyBuilder::new().allow_private(false).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client.send(get("http://10.0.0.5:8080/v1/models")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "PROXY_008");
    let rule = suggested_rule(&diagnostic);
    assert_eq!(
        rule.matcher.host,
        HostPattern::Ip(PRIVATE),
        "a glob never matches an address (ADR-007), so the pattern must be `ip:`"
    );
    assert_eq!(rule.matcher.port, Some(8080));
    assert_eq!(rule.matcher.path_prefixes, vec!["/v1/models".to_owned()]);
    assert!(rule.allow_private);
    assert_eq!(rule.action, Action::Ask);
    assert_eq!(proxy.resolver.calls(), 0, "a literal is never resolved");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_root_path_leaves_the_prefix_out_instead_of_writing_an_empty_one() {
    // `/` ist kein Praefix: Es traefe jeden Pfad und hoebe die Grenze auf, die
    // es ziehen soll (`path_prefix_is_valid`).
    let proxy = proxy_refusing("lan.example", PRIVATE).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client.send(get("http://lan.example/")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let rule = suggested_rule(&next_diagnostic(&mut events).await);
    assert!(rule.matcher.path_prefixes.is_empty());
    assert!(rule.matcher.path.is_none());
}

// ---------------------------------------------------------------------------
// 2. Die Adresse erreicht den Agenten nicht
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn neither_the_body_nor_a_header_carries_the_address() {
    let proxy = proxy_refusing("lan.example", PRIVATE).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client.send(get("http://lan.example/metrics")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let headers: Vec<String> = response
        .headers()
        .iter()
        .map(|(name, value)| format!("{name}: {}", String::from_utf8_lossy(value.as_bytes())))
        .collect();
    let body = body_string(response.into_body()).await;

    assert!(
        body.contains("reason: upstream_private_address"),
        "the client learns that the target was refused: {body}"
    );
    assert!(
        !body.contains("10.0.0.5"),
        "the address must never reach the sandbox: {body}"
    );
    for header in &headers {
        assert!(
            !header.contains("10.0.0.5"),
            "the address must never reach the sandbox: {header}"
        );
    }

    // Und sie steht sehr wohl im Befund; sonst prueft der Test nur, dass
    // niemand sie kennt.
    let diagnostic = next_diagnostic(&mut events).await;
    assert!(diagnostic.why.contains("10.0.0.5"));
}

// ---------------------------------------------------------------------------
// 3. Der Vorschlag ist anwendbar
// ---------------------------------------------------------------------------

/// Die Anfrage, aus der der Vorschlag entsteht, als reiner Wert.
fn failed_request(host: HostName, port: u16, path: &str) -> HttpRequest {
    HttpRequest::new(Method::GET, Scheme::Http, Authority { host, port }, path)
}

/// Dieselbe Anfrage mit einem verlangten Protokollwechsel.
fn with_websocket(mut request: HttpRequest) -> HttpRequest {
    let mut headers = HeaderMap::new();
    headers.insert(
        hyper::header::UPGRADE,
        hyper::header::HeaderValue::from_static("websocket"),
    );
    request.headers = headers;
    request
}

/// Der Schlüssel, gegen den die Regel treffen muss.
fn key_of(request: &HttpRequest) -> RequestKey<'_> {
    let key = RequestKey::new(
        &request.authority.host,
        &request.method,
        &request.path_and_query,
        request.scheme,
        request.authority.port,
    );
    match request.headers.get(hyper::header::UPGRADE) {
        Some(_) => key.with_upgrade(Upgrade::WebSocket),
        None => key,
    }
}

#[test]
fn the_suggested_rule_survives_the_round_trip_and_matches_the_failed_request() {
    for host in [
        HostName::Dns("lan.example".to_owned()),
        HostName::Ip(PRIVATE),
    ] {
        let request = failed_request(host.clone(), 11434, "/v1/models?key=secret");
        // Aus dem `fix` des echten Befunds, nicht aus der Hilfsfunktion: Sonst
        // bliebe der Test gruen, wenn `private_address_refused` kuenftig eine
        // andere Regel anbietet als die, die hier geprueft wird.
        let rule = suggested_rule(&private_address_refused(&request, PRIVATE));

        let yaml = serialize_rules(&RuleSet::from_rules([rule.clone()]));
        assert!(
            !yaml.contains("secret"),
            "the query never lands in rules.yaml: {yaml}"
        );
        let (set, warnings) = parse_rules(&yaml)
            .unwrap_or_else(|diagnostics| panic!("the suggestion must parse: {diagnostics:?}"));
        assert!(warnings.is_empty(), "{warnings:?}");

        let stored = set.get(rule.id).expect("the rule keeps its id");
        assert!(stored.allow_private, "the flag survives the round trip");
        assert_eq!(stored.action, Action::Ask);

        let path = "/v1/models?key=secret";
        let key = RequestKey::new(&host, &Method::GET, path, Scheme::Http, 11434);
        assert_eq!(
            set.evaluate(&key, Utc::now(), SessionId::new()),
            Verdict::Matched {
                rule: rule.id,
                action: Action::Ask
            },
            "the suggestion must hit the request it was made for"
        );

        // Und sie bleibt eng: ein anderer Port, ein anderer Pfad, eine andere
        // Methode treffen sie nicht.
        let other_port = RequestKey::new(&host, &Method::GET, path, Scheme::Http, 8080);
        assert_eq!(
            set.evaluate(&other_port, Utc::now(), SessionId::new()),
            Verdict::Default
        );
        let other_path = RequestKey::new(&host, &Method::GET, "/admin", Scheme::Http, 11434);
        assert_eq!(
            set.evaluate(&other_path, Utc::now(), SessionId::new()),
            Verdict::Default
        );
        let other_method = RequestKey::new(&host, &Method::POST, path, Scheme::Http, 11434);
        assert_eq!(
            set.evaluate(&other_method, Utc::now(), SessionId::new()),
            Verdict::Default
        );
    }
}

#[test]
fn an_unknown_method_gets_no_suggestion_because_no_rule_could_ever_match() {
    // Zwei Fallen in einer. Die Methode in die Regel zu schreiben lehnte
    // `parse_rules` ab und risse damit die ganze `rules.yaml` mit sich; sie
    // wegzulassen ginge durch den Parser, brächte aber nichts, weil
    // `RuleSet::evaluate` bei einer unbekannten Methode abbricht, bevor es eine
    // Regel ansieht. Der Mensch klickte und bekäme beim nächsten Versuch
    // dieselbe Ablehnung ohne neue Erklärung.
    let mut request = failed_request(HostName::Ip(PRIVATE), 8080, "/cache");
    request.method = Method::from_bytes(b"PURGE").unwrap();
    assert_eq!(private_address_rule(&request), Err(NoRule::UnknownMethod));

    let diagnostic = private_address_refused(&request, PRIVATE);
    assert_eq!(diagnostic.fix, None);
    assert!(
        diagnostic.why.contains("PURGE") && diagnostic.why.contains("never takes effect"),
        "the finding names the method and why nothing is offered: {}",
        diagnostic.why
    );

    // Der Beleg fuer den zweiten Teil: Selbst eine von Hand gebaute Regel ohne
    // Methode traefe diese Anfrage nicht.
    let without_method = Rule::new(
        RuleId::new(),
        Action::Ask,
        Matcher::host(HostPattern::Ip(PRIVATE))
            .with_scheme(Scheme::Http)
            .with_port(8080),
    )
    .with_allow_private(true);
    let set = RuleSet::from_rules([without_method]);
    assert_eq!(
        set.evaluate(&key_of(&request), Utc::now(), SessionId::new()),
        Verdict::Default,
        "an unknown method matches no rule at all"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_applied_suggestion_opens_the_target_and_still_holds_every_request() {
    // Der Test, der vor HUM-102 fehlschlug: `flow.allow_private` wurde nur im
    // Zweig `Action::Allow` gesetzt, eine Regel mit `action: ask` und
    // `allow_private: true` war also wirkungslos.
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let request = failed_request(
        HostName::Dns("lan.example".to_owned()),
        port,
        "/echo?token=abc",
    );
    let yaml = serialize_rules(&RuleSet::from_rules([rule_for(&request)]));

    let proxy = ProxyBuilder::new()
        .allow_private(false)
        .rules(&yaml)
        .resolve_host("lan.example", vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
        .start()
        .await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let url = format!("http://lan.example:{port}/echo?token=abc");
    let mut client = proxy.client().await;
    for round in 1..=2 {
        let response = client.send(get(&url)).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "round {round}: the rule opens the private target"
        );
        assert_eq!(body_string(response.into_body()).await, ECHO_BODY);
        events.wait_for_nth("recorded", round).await;
        assert_eq!(
            events.count("held"),
            round,
            "round {round}: `action: ask` keeps every request in front of a human"
        );
    }
    assert_eq!(upstream.hits(), 2);
    assert_eq!(
        diagnostic_codes(&events),
        Vec::<DiagnosticCode>::new(),
        "nothing was refused"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_block_rule_with_allow_private_still_blocks() {
    // `allow_private` oeffnet ein Ziel, es entscheidet nichts. Seit die Zeile
    // vor dem `match action` steht, gilt sie fuer jede Aktion — fuer `block`
    // muss das folgenlos bleiben.
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let yaml = "version: 1\nrules:\n  - action: block\n    allow_private: true\n    match:\n      host: lan.example\n";
    let proxy = ProxyBuilder::new()
        .allow_private(false)
        .rules(yaml)
        .resolve_host("lan.example", vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://lan.example:{port}/echo")))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("reason: rule"), "{body}");

    events.wait_for("recorded").await;
    assert_eq!(proxy.egress.connects(), 0);
    assert_eq!(upstream.hits(), 0);
    assert_eq!(
        proxy.resolver.calls(),
        0,
        "a blocked host is never resolved"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rule_without_allow_private_still_refuses_and_explains() {
    // Die Gegenprobe zum Vorschlag: dieselbe Regel ohne das Recht, und der
    // Befund kommt wie zuvor.
    let proxy = ProxyBuilder::new()
        .allow_private(false)
        .rules(&allow_rule("lan.example"))
        .resolve_host("lan.example", vec![PRIVATE])
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client.send(get("http://lan.example/metrics")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "PROXY_008");
    assert_eq!(proxy.egress.connects(), 0);
    // Und der Befund sagt, was den Vorschlag sonst verdeckt: Die aeltere Regel
    // trifft zuerst, also muss die neue vor ihr stehen.
    assert!(
        diagnostic
            .why
            .contains("Put it in front of your other rules"),
        "{}",
        diagnostic.why
    );
}

// ---------------------------------------------------------------------------
// 6. Der Vorschlag wirkt nur an der richtigen Stelle
// ---------------------------------------------------------------------------

/// Der Vorschlag hinter der Regel, die entschieden hat, wirkt nicht; davor
/// wirkt er.
///
/// `RuleSet::evaluate` liefert den **ersten** Treffer eines Ranges. Beim Klick
/// auf den Fix landet die Regel heute am Ende der Nutzerregeln
/// (`humanitl_ipc::convert::rule_to_proto` sendet `position: 0`, `position_of`
/// liest das als „ans Ende", `RulesStore::add` haengt an), also genau dort, wo
/// sie nichts aendert, sobald eine aeltere Regel denselben Host trifft. Der
/// Mensch klickt, versucht es erneut und bekommt denselben `PROXY_008`.
///
/// Dieser Test misst beide Reihenfolgen und haelt damit fest, was die
/// Positionierung leisten muss. Die Positionierung selbst liegt in `ipc` und
/// steht als eigenes Issue aus; bis dahin nennen `why` und Notiz die Bedingung.
#[tokio::test(flavor = "multi_thread")]
async fn the_suggestion_only_takes_effect_in_front_of_the_rule_that_decided() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let request = failed_request(host_name(), port, "/echo");
    let suggestion = suggested_rule(&private_address_refused(&request, PRIVATE));

    // Die aeltere Regel des Nutzers: trifft denselben Host, erlaubt aber keine
    // privaten Ziele.
    let older = Rule::new(
        RuleId::new(),
        Action::Allow,
        Matcher::host(HostPattern::Exact(host_name())),
    );

    for (name, rules, expected) in [
        (
            "appended behind the older rule",
            vec![older.clone(), suggestion.clone()],
            StatusCode::BAD_GATEWAY,
        ),
        (
            "inserted in front of it",
            vec![suggestion.clone(), older.clone()],
            StatusCode::OK,
        ),
    ] {
        let yaml = serialize_rules(&RuleSet::from_rules(rules));
        let proxy = ProxyBuilder::new()
            .allow_private(false)
            .rules(&yaml)
            .resolve_host("lan.example", vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
            .start()
            .await;
        let mut events = proxy.events();
        let _decider = proxy.decide_with(Decision::Allow);

        let mut client = proxy.client().await;
        let response = client
            .send(get(&format!("http://lan.example:{port}/echo")))
            .await;
        assert_eq!(response.status(), expected, "{name}");
        events.wait_for("recorded").await;
    }
    assert_eq!(
        upstream.hits(),
        1,
        "only the run with the suggestion in front reaches the target"
    );
}

// ---------------------------------------------------------------------------
// 4. Der Vorschlag darf die `rules.yaml` nie zerreissen
// ---------------------------------------------------------------------------

/// Der Host, den die Tabelle benutzt, wo der Host nicht der Punkt ist.
fn host_name() -> HostName {
    HostName::Dns("lan.example".to_owned())
}

/// Die Tabelle feindlicher Anfragen. Wer ein Feld hinzufuegt, das der
/// Builder aus der Anfrage uebertraegt, haengt seine Zeile an.
fn hostile_requests() -> Vec<(&'static str, HttpRequest)> {
    let long_path = format!("/{}", "a".repeat(4000));
    vec![
        ("a plain name", failed_request(host_name(), 80, "/metrics")),
        (
            "an ipv4 literal",
            failed_request(HostName::Ip(PRIVATE), 8080, "/v1/models"),
        ),
        (
            "an ipv6 literal",
            failed_request(
                HostName::parse("[::1]").expect("a v6 literal"),
                8080,
                "/v1/models",
            ),
        ),
        (
            "an ipv4-mapped literal",
            failed_request(
                HostName::parse("[::ffff:10.0.0.5]").expect("a mapped literal"),
                8080,
                "/v1/models",
            ),
        ),
        (
            "a punycode name",
            failed_request(
                HostName::parse("xn--e1afmkfd.example").expect("a punycode name"),
                443,
                "/metrics",
            ),
        ),
        ("the root path", failed_request(host_name(), 80, "/")),
        (
            "a query only",
            failed_request(host_name(), 80, "/?token=abc"),
        ),
        (
            "the shortest usable prefix",
            failed_request(host_name(), 80, "/a"),
        ),
        (
            "dot segments",
            failed_request(host_name(), 80, "/../../etc/passwd"),
        ),
        (
            "unicode in the path",
            failed_request(host_name(), 80, "/uenae"),
        ),
        (
            "glob characters in the path",
            failed_request(host_name(), 80, "/**/*?x=1"),
        ),
        (
            "a very long path",
            failed_request(host_name(), 80, &long_path),
        ),
        ("the highest port", failed_request(host_name(), 65535, "/x")),
        ("the lowest port", failed_request(host_name(), 1, "/x")),
        ("a websocket upgrade", {
            let mut request = failed_request(host_name(), 80, "/socket");
            request.scheme = Scheme::Ws;
            with_websocket(request)
        }),
        ("scheme wss", {
            let mut request = failed_request(host_name(), 443, "/socket");
            request.scheme = Scheme::Wss;
            with_websocket(request)
        }),
        ("scheme https", {
            let mut request = failed_request(host_name(), 443, "/metrics");
            request.scheme = Scheme::Https;
            request
        }),
        ("method HEAD", {
            let mut request = failed_request(host_name(), 80, "/metrics");
            request.method = Method::HEAD;
            request
        }),
        // Die beiden Faelle ohne Vorschlag laufen durch dieselbe Schleife: Sie
        // duerfen keinen Knopf zeigen, und der `why` muss den Grund nennen.
        ("port zero", failed_request(host_name(), 0, "/metrics")),
        ("an unknown method", {
            let mut request = failed_request(host_name(), 80, "/cache");
            request.method = Method::from_bytes(b"PURGE").expect("a token");
            request
        }),
    ]
}

/// Jede vorgeschlagene Regel geht durch `parse_rules`, fuer eine Tabelle
/// feindlicher Anfragen.
///
/// Das ist der Test, der den naechsten Fall dieser Art selbst findet. Ein Klick
/// auf den Fix schreibt die Regel in die `rules.yaml` des Nutzers, und ein
/// einziger Wert ausserhalb des Wertebereichs, den `parse_rules` kennt, lehnt
/// **die ganze Datei** ab: Der Nutzer verloere alle seine Regeln, ausgeloest von
/// einer Anfrage, die der Agent frei formt. Zwei Loecher dieser Art gab es
/// schon — eine unbekannte Methode und Port 0 —, und beide standen in derselben
/// Zeile Code.
///
/// Die Tabelle deckt jedes Feld ab, das der Builder aus der Anfrage
/// uebertraegt: Host als Name, als IPv4, als IPv6, als IPv4-mapped und als
/// Punycode; Methode bekannt und unbekannt; Schema in allen vier Formen; Port
/// am Rand des Bereichs; Pfad ohne Praefix, mit kuerzestem Praefix, mit Query,
/// mit Punktsegmenten, mit Unicode, mit Glob-Zeichen und sehr lang; mit und
/// ohne Protokollwechsel. Wer ein Feld hinzufuegt, haengt seine Zeile an.
#[test]
fn no_hostile_request_can_produce_a_rule_that_the_parser_rejects() {
    for (name, request) in hostile_requests() {
        let diagnostic = private_address_refused(&request, PRIVATE);
        let Ok(rule) = private_address_rule(&request) else {
            // Kein Vorschlag ist eine gueltige Antwort — aber dann darf auch
            // kein Knopf dastehen, und der `why` muss den Grund nennen.
            assert_eq!(diagnostic.fix, None, "{name}");
            assert!(
                diagnostic.why.contains("There is no rule to suggest"),
                "{name}: {}",
                diagnostic.why
            );
            continue;
        };
        assert!(
            matches!(diagnostic.fix, Some(FixAction::AddRule(_))),
            "{name}: a rule exists, so the finding has to offer it"
        );

        let yaml = serialize_rules(&RuleSet::from_rules([rule.clone()]));
        let (set, warnings) = parse_rules(&yaml).unwrap_or_else(|diagnostics| {
            panic!("{name}: the suggestion tore up rules.yaml: {diagnostics:?}\n{yaml}")
        });
        // Eine Warnung ist erlaubt (ein Punycode-Name bekommt `RULES_002`), ein
        // Fehler nicht: Der unterscheidet sich darin, dass er die Datei
        // ablehnt, und genau das darf ein Vorschlag nie ausloesen.
        assert!(
            warnings.iter().all(|w| w.severity != Severity::Error),
            "{name}: {warnings:?}"
        );
        assert_eq!(
            set.evaluate(&key_of(&request), Utc::now(), SessionId::new()),
            Verdict::Matched {
                rule: rule.id,
                action: Action::Ask
            },
            "{name}: the suggestion has to hit the request it was made for\n{yaml}"
        );
    }
}

#[test]
fn port_zero_gets_no_suggestion_because_no_rule_could_carry_it() {
    // `parse_rules` lehnt `port: 0` ab, und ein Fehler dort verwirft die ganze
    // Datei. Den Port wegzulassen waere kein Ausweg: Das oeffnete jeden Port
    // desselben Hosts. Also kein Vorschlag, und der `why` sagt warum.
    let request = failed_request(HostName::Ip(PRIVATE), 0, "/metrics");
    assert_eq!(private_address_rule(&request), Err(NoRule::PortZero));

    let diagnostic = private_address_refused(&request, PRIVATE);
    assert_eq!(diagnostic.code.as_str(), "PROXY_008");
    assert_eq!(diagnostic.fix, None);
    assert!(
        diagnostic.why.contains("port 0") && diagnostic.why.contains("1..=65535"),
        "the finding says why there is nothing to click: {}",
        diagnostic.why
    );
    assert!(diagnostic.why.contains("10.0.0.5"), "{}", diagnostic.why);

    // Gegenprobe, dass wirklich der Port das Hindernis ist und nicht der Rest
    // der Anfrage: derselbe Fall mit Port 1 bekommt seinen Vorschlag.
    let ok = failed_request(HostName::Ip(PRIVATE), 1, "/metrics");
    assert!(private_address_rule(&ok).is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_to_port_zero_still_gets_the_finding_without_a_fix() {
    let proxy = ProxyBuilder::new().allow_private(false).start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client.send(get("http://10.0.0.5:0/metrics")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "PROXY_008");
    assert_eq!(
        diagnostic.fix, None,
        "a fix here would tear up the user's rules.yaml"
    );
}

// ---------------------------------------------------------------------------
// 5. Wie weit die Regel reicht, steht im Befund
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn without_a_path_prefix_the_finding_says_that_it_covers_every_path() {
    // Ohne Praefix faellt die Auswertung auf `CompiledPrefixes::Any` zurueck:
    // Die Regel trifft jeden Pfad dieses Hosts. Der Vorschlag bleibt — die
    // Wurzel eines Dienstes ist der Normalfall, und die Regel gibt nichts frei,
    // sie oeffnet nur ein Ziel und laesst jede Anfrage halten. Verschwiegen
    // wird die Weite aber nicht.
    let proxy = proxy_refusing("lan.example", PRIVATE).await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);

    let mut client = proxy.client().await;
    let response = client.send(get("http://lan.example/")).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let diagnostic = next_diagnostic(&mut events).await;
    let rule = suggested_rule(&diagnostic);
    assert!(rule.matcher.path_prefixes.is_empty());
    assert!(
        diagnostic.why.contains("every path of this host"),
        "the finding names the reach it has: {}",
        diagnostic.why
    );

    // Und der Text erscheint nur dann: ein Pfad mit Praefix bekommt ihn nicht.
    let narrow = private_address_refused(&failed_request(host_name(), 80, "/metrics"), PRIVATE);
    assert!(
        !narrow.why.contains("every path of this host"),
        "{}",
        narrow.why
    );
}

#[test]
fn an_upgrade_is_carried_into_the_rule_and_named_in_the_finding() {
    // Die Auswertung prueft die Upgrade-Dimension beidseitig. Eine Regel ohne
    // `upgrade` traefe genau den gescheiterten WebSocket nicht und oeffnete
    // stattdessen gewoehnliches HTTP — die schlechteste Kombination: Sie oeffnet
    // etwas, das niemand gefragt hat, und loest das Problem nicht.
    let mut request = failed_request(host_name(), 80, "/socket");
    request.scheme = Scheme::Ws;
    let request = with_websocket(request);

    let rule = rule_for(&request);
    assert_eq!(rule.matcher.upgrade, Some(Upgrade::WebSocket));

    let yaml = serialize_rules(&RuleSet::from_rules([rule.clone()]));
    assert!(yaml.contains("upgrade: websocket"), "{yaml}");
    let (set, warnings) =
        parse_rules(&yaml).unwrap_or_else(|d| panic!("the suggestion must parse: {d:?}"));
    assert!(warnings.is_empty(), "{warnings:?}");

    // Sie trifft den WebSocket ...
    assert_eq!(
        set.evaluate(&key_of(&request), Utc::now(), SessionId::new()),
        Verdict::Matched {
            rule: rule.id,
            action: Action::Ask
        }
    );
    // ... und nicht die gewoehnliche Anfrage an dasselbe Ziel.
    let plain = RequestKey::new(
        &request.authority.host,
        &request.method,
        &request.path_and_query,
        request.scheme,
        request.authority.port,
    );
    assert_eq!(
        set.evaluate(&plain, Utc::now(), SessionId::new()),
        Verdict::Default,
        "an upgrade rule must not open ordinary requests"
    );

    let diagnostic = private_address_refused(&request, PRIVATE);
    assert!(
        diagnostic.why.contains("protocol upgrade"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn a_path_with_dot_segments_loses_its_prefix_instead_of_carrying_a_dead_condition() {
    // Ein Pfad mit einem `..`-Segment trifft nie ein Praefix, auch verschleiert
    // nicht: Erst der Server dahinter loest auf. Ein Praefix daraus stuende in
    // der Regel als Bedingung, die genau die Anfrage nicht erfuellt, fuer die
    // der Vorschlag gemacht wurde — er parste sauber und wirkte nie.
    for path in [
        "/../../etc/passwd",
        "/api/chat/../pull",
        "/api/%2e%2e/pull",
        "/api\\..\\pull",
    ] {
        let request = failed_request(host_name(), 80, path);
        let rule = rule_for(&request);
        assert!(
            rule.matcher.path_prefixes.is_empty(),
            "{path}: a prefix that cannot match must not stand in the rule"
        );
        let set = RuleSet::from_rules([rule.clone()]);
        assert_eq!(
            set.evaluate(&key_of(&request), Utc::now(), SessionId::new()),
            Verdict::Matched {
                rule: rule.id,
                action: Action::Ask
            },
            "{path}: without the prefix the rule has to hit"
        );
    }

    // Gegenprobe: derselbe Pfad ohne Punktsegment behaelt sein Praefix.
    let plain = failed_request(host_name(), 80, "/api/chat/pull");
    assert_eq!(
        rule_for(&plain).matcher.path_prefixes,
        vec!["/api/chat/pull".to_owned()]
    );
}
