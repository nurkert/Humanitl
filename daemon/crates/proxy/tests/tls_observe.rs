//! Gescheiterte TLS-Handschläge des Clients, am laufenden Proxy (HUM-045).
//!
//! Der Unterschied zu den Modultests in `src/tls_observe.rs`: Dort wird ein
//! Fehlerwert gedeutet, hier bricht ein echter rustls-Client einen echten
//! Handschlag ab. Nur so ist belegt, dass der Fehler auch wirklich in der Form
//! ankommt, die [`humanitl_proxy::classify`] erwartet — die Verpackung eines
//! `rustls::Error` in einen `io::Error` ist eine Eigenheit von `tokio-rustls`
//! und keine Zusage.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use humanitl_core::{Diagnostic, FlowEvent};
use humanitl_recorder::FlowQuery;
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use tokio::io::AsyncWriteExt as _;
use tokio::net::UnixStream;
use tokio_rustls::TlsConnector;

use support::{Proxy, ProxyBuilder, WAIT};

/// Öffnet einen Tunnel und versucht darin einen Handschlag mit einem Client,
/// der keiner Wurzel vertraut.
///
/// Das ist `curl --cacert /dev/null` in der Sandbox: Der `CONNECT` geht durch,
/// der Client sieht das Leaf des Proxys, findet keinen Aussteller dafür und
/// bricht ab. Liefert den Fehler des Handschlags.
async fn handshake_without_trust(proxy: &Proxy, host: &str, agent: Option<&str>) -> String {
    let headers: Vec<(&str, &str)> = agent
        .map(|value| vec![(hyper::header::USER_AGENT.as_str(), value)])
        .unwrap_or_default();
    handshake_without_trust_with(proxy, host, &headers).await
}

/// Wie [`handshake_without_trust`], mit frei gewählten Kopfzeilen im `CONNECT`.
async fn handshake_without_trust_with(
    proxy: &Proxy,
    host: &str,
    headers: &[(&str, &str)],
) -> String {
    let stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .unwrap();
    let outer = tokio::spawn(async move {
        let _ = conn.with_upgrades().await;
    });

    let mut connect = Request::builder()
        .method(Method::CONNECT)
        .uri(format!("{host}:443"));
    for (name, value) in headers {
        connect = connect.header(*name, *value);
    }
    let response = sender
        .send_request(connect.body(Full::new(Bytes::new())).unwrap())
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "CONNECT must be accepted"
    );
    let upgraded = hyper::upgrade::on(response).await.unwrap();

    // Ein leerer Wurzelspeicher: Der Client kann das Leaf des Proxys nicht
    // prüfen und schickt `unknown_ca`.
    let mut config = ClientConfig::builder_with_provider(proxy.ca.provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(RootCertStore::empty())
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = ServerName::try_from(host.to_owned()).unwrap();
    let err = TlsConnector::from(Arc::new(config))
        .connect(name, TokioIo::new(upgraded))
        .await
        .expect_err("a client without roots must not finish the handshake")
        .to_string();
    outer.abort();
    err
}

/// Wartet auf den nächsten Befund im Strom und liefert ihn.
async fn next_diagnostic(events: &mut support::Events) -> Diagnostic {
    let event = events.wait_for("diagnostic").await;
    match event {
        FlowEvent::Diagnostic { diagnostic, .. } => *diagnostic,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_client_without_a_trust_store_gets_one_tls_001_and_a_flow_with_an_error() {
    let proxy = ProxyBuilder::new().recording(true).start().await;
    let mut events = proxy.events();

    let err = handshake_without_trust(&proxy, "example.com", Some("curl/8.5.0")).await;
    assert!(
        err.contains("UnknownIssuer") || err.contains("unknown issuer"),
        "the client must fail on the issuer, not on something else: {err}"
    );

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "TLS_001");
    assert!(diagnostic.why.contains("example.com"), "{}", diagnostic.why);
    assert!(
        diagnostic.why.contains("calls itself curl"),
        "the User-Agent is a hint, and the text says so: {}",
        diagnostic.why
    );
    assert!(
        matches!(
            diagnostic.fix,
            Some(humanitl_core::FixAction::SetEnv { ref key, .. }) if key == "CURL_CA_BUNDLE"
        ),
        "{:?}",
        diagnostic.fix
    );

    // Der Befund hängt an dem Flow, den der gescheiterte CONNECT erzeugt hat.
    let flow_id = events
        .seen
        .iter()
        .find_map(|event| match event {
            FlowEvent::Diagnostic { flow_id, .. } => *flow_id,
            _ => None,
        })
        .expect("the diagnostic belongs to a flow");

    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow_id)
        .await
        .unwrap()
        .expect("the failed CONNECT is in the history");
    assert_eq!(detail.summary.method, "CONNECT");
    assert_eq!(detail.summary.host, "example.com");
    assert_eq!(detail.summary.state, "recorded");
    assert_eq!(
        detail.summary.error.as_deref(),
        Some("tls_handshake_failed")
    );
    // Nicht als Entscheidung eines Menschen: Das System hat den Flow beendet.
    assert_eq!(detail.summary.block_reason.as_deref(), Some("no_route"));
    // Der `User-Agent` ist aufgezeichnet, also ist der Hinweis nachprüfbar.
    let request = detail
        .messages
        .iter()
        .find(|message| message.dir == humanitl_recorder::Dir::Request)
        .expect("the CONNECT headers are recorded");
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("user-agent") && value == "curl/8.5.0"),
        "{:?}",
        request.headers
    );
}

#[tokio::test]
async fn a_repeated_rejection_is_reported_once_but_recorded_every_time() {
    // Fallstrick HUM-045: `TLS_001` darf nicht pro Wiederholung spammen. Die
    // History zeigt trotzdem jeden Versuch, sonst fehlte der Beleg.
    let proxy = ProxyBuilder::new().recording(true).start().await;
    let mut events = proxy.events();

    for _ in 0..3 {
        handshake_without_trust(&proxy, "example.com", Some("curl/8.5.0")).await;
    }

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "TLS_001");
    // Drei `Recorded` heißt: Alle drei Versuche sind durch. Was danach noch im
    // Strom liegt, ist vollständig.
    events.wait_for_nth("recorded", 3).await;
    events.drain();
    assert_eq!(
        events.count("diagnostic"),
        1,
        "three rejections of the same host by the same tool are one card"
    );

    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let page = recorder.list_flows(&FlowQuery::default()).await.unwrap();
    assert_eq!(page.rows.len(), 3, "every attempt is in the history");
    assert!(
        page.rows
            .iter()
            .all(|row| row.error.as_deref() == Some("tls_handshake_failed")),
        "{:?}",
        page.rows
            .iter()
            .map(|row| row.error.clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_tunnel_without_sni_is_reported_as_information() {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    // Der Handschlag kommt zustande (das Leaf gilt dem CONNECT-Ziel), die
    // Anfrage darin wird abgelehnt, weil die SNI fehlt (HUM-023).
    let mut client = proxy.tls_tunnel("example.com", 443, None).await;
    let response = client
        .client
        .send(
            Request::builder()
                .uri("/x")
                .header("host", "example.com")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "TLS_003");
    assert_eq!(diagnostic.severity, humanitl_core::Severity::Info);
    assert!(diagnostic.fix.is_none(), "{:?}", diagnostic.fix);
    assert_eq!(proxy.resolver.calls(), 0, "nothing was resolved");
}

#[tokio::test]
async fn a_handshake_failure_does_not_stop_the_proxy() {
    // Nach einem gescheiterten Handschlag muss dieselbe Sitzung weiterarbeiten:
    // kein hängender Task, kein belegter Socket, kein verlorener Accept-Loop.
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_millis(200))
        .start()
        .await;

    for _ in 0..5 {
        handshake_without_trust(&proxy, "example.com", None).await;
    }

    let mut client = proxy.client().await;
    let response = tokio::time::timeout(
        WAIT,
        client.send(
            Request::builder()
                .uri("/")
                .header("host", "still.example")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        ),
    )
    .await
    .expect("the proxy still answers");
    // Ohne Entscheider läuft die Anfrage in die kurze Haltefrist und dann in
    // den Timeout; wichtig ist nur, dass der Proxy überhaupt noch antwortet.
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
}

#[tokio::test]
async fn a_client_that_keeps_dropping_the_handshake_gets_tls_002() {
    // Drei Abbrüche ohne Alert in zehn Sekunden zum selben Host. Der Client
    // schließt die Verbindung mitten im Handschlag, statt ein Zertifikat zu
    // prüfen; genau so verhält sich ein Werkzeug mit Certificate Pinning.
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();

    for _ in 0..3 {
        drop_handshake(&proxy, "pinned.example").await;
    }

    let diagnostic = next_diagnostic(&mut events).await;
    assert_eq!(diagnostic.code.as_str(), "TLS_002");
    assert!(
        diagnostic.why.contains("pinned.example"),
        "{}",
        diagnostic.why
    );
    assert!(
        matches!(diagnostic.fix, Some(humanitl_core::FixAction::AddRule(_))),
        "{:?}",
        diagnostic.fix
    );
}

/// `CONNECT`, dann den ersten Handschlagsatz schicken und auflegen.
///
/// Kein Alert, keine Erklärung: Der Proxy sieht ein `ClientHello` und danach
/// nichts mehr.
async fn drop_handshake(proxy: &Proxy, host: &str) {
    let stream = UnixStream::connect(&proxy.socket).await.unwrap();
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .unwrap();
    let outer = tokio::spawn(async move {
        let _ = conn.with_upgrades().await;
    });
    let response = sender
        .send_request(
            Request::builder()
                .method(Method::CONNECT)
                .uri(format!("{host}:443"))
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let upgraded = hyper::upgrade::on(response).await.unwrap();
    let mut io = TokioIo::new(upgraded);

    // Ein `ClientHello`, wie rustls es schreibt, und dann fallen lassen. Der
    // Handschlag steht damit nie, und der Proxy liest ein EOF.
    let mut config = ClientConfig::builder_with_provider(proxy.ca.provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(RootCertStore::empty())
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = ServerName::try_from(host.to_owned()).unwrap();
    let mut connection =
        rustls::ClientConnection::new(Arc::new(config), name).expect("a client hello");
    let mut hello = Vec::new();
    connection
        .write_tls(&mut hello)
        .expect("the client hello is written");
    io.write_all(&hello).await.expect("the hello goes out");
    io.flush().await.expect("the hello is flushed");
    io.shutdown().await.expect("the client hangs up");
    drop(io);
    outer.abort();

    // Dem Proxy einen Augenblick lassen, den Abbruch zu bemerken; die Zeitpunkte
    // müssen innerhalb des Zehn-Sekunden-Fensters bleiben.
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test]
async fn a_secret_in_the_connect_headers_is_searched_before_it_is_stored() {
    // Die Kopfzeilen des CONNECT werden aufgezeichnet, damit der Hinweis auf
    // das Werkzeug nachprüfbar bleibt. Was aufgezeichnet wird, wird durchsucht:
    // Ein Datensatz mit `findings_count = 0`, den niemand durchsucht hat, sähe
    // aus wie ein sauberer.
    let proxy = ProxyBuilder::new()
        .recording(true)
        .scanner(Arc::new(
            humanitl_proxy::Tier1Scanner::new(&humanitl_findings::FindingsSettings::default())
                .unwrap(),
        ))
        .start()
        .await;
    let mut events = proxy.events();

    handshake_without_trust_with(
        &proxy,
        "example.com",
        &[
            (hyper::header::USER_AGENT.as_str(), "curl/8.5.0"),
            // Eine IBAN mit gültiger Prüfsumme: ein Tier-1-Fund, den kein
            // Muster raten muss.
            ("x-note", "wire it to GB82 WEST 1234 5698 7654 32"),
        ],
    )
    .await;

    let analyzed = events.wait_for("analyzed").await;
    let FlowEvent::Analyzed {
        flow_id, findings, ..
    } = analyzed
    else {
        panic!("an Analyzed event");
    };
    assert!(
        !findings.is_empty(),
        "the CONNECT headers have to go through the detectors"
    );

    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow_id)
        .await
        .unwrap()
        .expect("the failed CONNECT is in the history");
    assert!(
        detail.summary.findings_count > 0,
        "the row must not look clean: {:?}",
        detail.summary
    );
    assert!(
        !detail.findings.is_empty(),
        "and the finding itself is recorded"
    );
    // Der Wert selbst steht in keinem Fund, nur sein Hash und ein Präfix.
    for finding in &detail.findings {
        assert!(
            !finding.display_prefix.contains("7654 32"),
            "{:?}",
            finding.display_prefix
        );
    }
}
