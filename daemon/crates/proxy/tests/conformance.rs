//! Die Konformitäts-Matrix (HUM-017): echte Clients durch den echten Proxy.
//!
//! Jede Zeile der Matrix aus `backlog/sprint-1.md` ist ein Test namens
//! `conf_<nr>_<client>_<szenario>`. Gefahren wird mit den Werkzeugen, die ein
//! Coding-Agent tatsächlich benutzt: `curl`, `websocat`, `grpcurl`, `python3`,
//! `node` und `git`. Fehlt eines davon, meldet der Test sich über
//! [`support::clients::skip`] ab und endet grün; im CI sind alle installiert.
//!
//! Aufbau jedes Tests: ein eigener Fake-Upstream, ein eigener Proxy auf einem
//! eigenen Unix-Socket, eine eigene TCP-Brücke auf diesen Socket. Die Brücke
//! ist das, was in der Sandbox der Shim tut; der Proxy selbst bekommt nach wie
//! vor keinen Loopback-Port auf dem Host (Garantie 2).
//!
//! # Was in M1 absichtlich fehlschlägt
//!
//! - Zeile 10 und 13: ALPN bietet dem Client nur `http/1.1`
//!   (`backlog/CONVENTIONS.md` 4.10). `curl --http2` fällt deshalb auf
//!   HTTP/1.1 zurück, gRPC über TLS scheitert sichtbar; `docs/SECURITY.md`
//!   Abschnitt 5 nennt das als `PROXY_007 h2 not available`.
//! - Zeile 11: Der Proxy entfernt vor der Weiterleitung die Kopfzeilen von
//!   Verbindungsrang, also auch `Connection` und `Upgrade`. Ein
//!   Protokollwechsel erreicht das Ziel in M1 nicht; der Upgrade-Request wird
//!   dennoch regulär gehalten und entschieden.
//! - Zeile 12 und 13 haben in M1 keinen gRPC-Upstream: `tonic` ist keine
//!   Abhängigkeit dieser Crate, und die Zeile ist ohnehin als Fehlschlag
//!   spezifiziert.
//!
//! Statuscodes nach `backlog/sprint-1.md` (Review-Korrektur 2026-09-02):
//! `403` Politik, `413` Body-Cap, `504` Wartezeit, `502` Upstream,
//! `503` Budget.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

mod support;

use std::fmt::Write as _;
use std::net::{IpAddr, Ipv6Addr};
use std::path::Path;
use std::time::{Duration, Instant};

use humanitl_core::{BlockReason, Decision, HostName, Scheme};
use support::ProxyBuilder;
use support::clients::{self, CLIENT_TIMEOUT, Run, Tool};
use support::upstream::{
    CHUNK_COUNT, CHUNK_SIZE, FakeUpstream, SSE_EVENTS, UpstreamCa, chunked_expected, sha256_hex,
};

/// Was `curl --write-out` je Anfrage ausgibt, in dieser Reihenfolge.
const WRITE_OUT: &str =
    "%{http_code} %{time_starttransfer} %{time_total} %{size_download} %{http_version}\n";

// ---------------------------------------------------------------------------
// Zeile 1: curl, GET über Klartext
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_01_curl_http_get() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_01_curl_http_get", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");

    let proxy_url = bridge.url();
    let url = format!("http://127.0.0.1:{}/echo", upstream.port());
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, &url]).await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    let body = read_text(&out);
    assert_eq!(
        json_field(&body, "method").as_deref(),
        Some("GET"),
        "{body}"
    );
    assert_eq!(
        json_field(&body, "path").as_deref(),
        Some("/echo"),
        "{body}"
    );
    assert_eq!(upstream.hits(), 1);

    events.wait_for("recorded").await;
    assert_eq!(events.received_request().path_and_query, "/echo");
    assert_eq!(events.statuses(), vec![200]);
}

// ---------------------------------------------------------------------------
// Zeile 2: curl, GET über CONNECT und TLS
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_02_curl_https_connect() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_02_curl_https_connect", "curl");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");
    let ca_pem = proxy.ca_pem();

    let proxy_url = bridge.url();
    let ca = ca_pem.display().to_string();
    let url = format!("https://localhost:{}/echo", upstream.port());
    // `--cacert` ist nur die Humanitl-CA. Ein grüner Lauf beweist damit, dass
    // das Leaf, das der Client sieht, von unserer CA kommt und nicht vom Ziel.
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, "--cacert", &ca, &url]).await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    let body = read_text(&out);
    assert_eq!(
        json_field(&body, "path").as_deref(),
        Some("/echo"),
        "{body}"
    );

    events.wait_for("recorded").await;
    let request = events.received_request();
    assert_eq!(request.scheme, Scheme::Https, "the flow is a TLS flow");
    assert_eq!(request.authority.port, upstream.port());
    assert_eq!(
        upstream.negotiated_alpn(),
        Some(b"http/1.1".to_vec()),
        "the proxy speaks HTTP/1.1 upstream in M1"
    );
}

// ---------------------------------------------------------------------------
// Zeile 3: curl, POST mit 2 MiB über TLS
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_03_curl_post_2mb_https() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_03_curl_post_2mb_https", "curl");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");
    let ca_pem = proxy.ca_pem();

    let payload = pattern(2 * 1024 * 1024);
    let expected = sha256_hex(&payload);
    let upload = tmp.path().join("payload.bin");
    std::fs::write(&upload, &payload).unwrap();

    let proxy_url = bridge.url();
    let ca = ca_pem.display().to_string();
    let at_file = format!("@{}", upload.display());
    let url = format!("https://localhost:{}/echo", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &[
            "-x",
            &proxy_url,
            "--cacert",
            &ca,
            "--data-binary",
            &at_file,
            &url,
        ],
    )
    .await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    let body = read_text(&out);
    assert_eq!(
        json_field(&body, "body_sha256").as_deref(),
        Some(expected.as_str()),
        "the upstream saw exactly the bytes curl sent"
    );
    assert_eq!(
        json_field(&body, "body_len").as_deref(),
        Some("2097152"),
        "{body}"
    );

    events.wait_for("recorded").await;
    let request = events.received_request();
    assert_eq!(
        request.body.size,
        2 * 1024 * 1024,
        "the decision saw the whole 2 MiB body"
    );
    assert_eq!(
        hex(&request.body.sha256),
        expected,
        "the hash in the event is the hash of the payload"
    );
}

// ---------------------------------------------------------------------------
// Zeile 4: curl, POST über dem Body-Cap
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_04_curl_post_over_body_cap() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_04_curl_post_over_body_cap", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.txt");

    // 33 MiB, also über `limits.hold_body_cap_bytes` (32 MiB).
    let upload = tmp.path().join("payload.bin");
    std::fs::write(&upload, vec![b'x'; 33 * 1024 * 1024]).unwrap();

    let proxy_url = bridge.url();
    let at_file = format!("@{}", upload.display());
    let url = format!("http://127.0.0.1:{}/sink", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &["-x", &proxy_url, "--data-binary", &at_file, &url],
    )
    .await;

    // `413`, nicht `403`: die Review-Korrektur vom 2026-09-02 in
    // `backlog/sprint-1.md` legt den Body-Cap auf 413 fest.
    assert_eq!(result.first().code, 413, "{}", result.run.report());
    let body = read_text(&out);
    assert!(body.contains("reason: body_cap"), "{body}");
    assert_eq!(
        upstream.hits(),
        0,
        "a body over the cap never reaches the upstream"
    );

    events.wait_for("recorded").await;
    assert_eq!(events.count("forwarded"), 0);
}

// ---------------------------------------------------------------------------
// Zeile 5: curl, Expect: 100-continue
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_05_curl_expect_100_continue() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_05_curl_expect_100_continue", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.txt");

    // Der Entscheider hält fest, was der Upstream im Augenblick der
    // Entscheidung gesehen hat. Vor dem Allow muss das null sein.
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
    let hits = std::sync::Arc::clone(&upstream.hits);
    let recorder = std::sync::Arc::clone(&seen);
    let _decider = proxy.decide_each(move |_index| {
        recorder.store(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            std::sync::atomic::Ordering::SeqCst,
        );
        Decision::Allow
    });

    let payload = pattern(1024 * 1024);
    let upload = tmp.path().join("payload.bin");
    std::fs::write(&upload, &payload).unwrap();

    let proxy_url = bridge.url();
    let at_file = format!("@{}", upload.display());
    let url = format!("http://127.0.0.1:{}/sink", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &[
            "-x",
            &proxy_url,
            "-H",
            "Expect: 100-continue",
            "--data-binary",
            &at_file,
            &url,
        ],
    )
    .await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    assert_eq!(read_text(&out).trim(), "1048576");
    assert_eq!(
        seen.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the upstream was untouched when the decision was made"
    );
    assert_eq!(upstream.hits(), 1);

    events.wait_for("recorded").await;
    assert_eq!(events.received_request().body.size, 1024 * 1024);
}

// ---------------------------------------------------------------------------
// Zeile 6: curl, Server-Sent Events
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_06_curl_sse() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_06_curl_sse", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("stream.txt");

    let proxy_url = bridge.url();
    let url = format!("http://127.0.0.1:{}/sse?interval_ms=200", upstream.port());
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, "--no-buffer", &url]).await;

    let line = result.first();
    assert_eq!(line.code, 200, "{}", result.run.report());
    assert!(
        line.time_starttransfer < 0.5,
        "the first event must arrive within 500 ms, took {}s",
        line.time_starttransfer
    );
    assert!(
        line.time_total > 0.5,
        "five events at 200 ms apart take longer than the first one, took {}s",
        line.time_total
    );
    let body = read_text(&out);
    assert_eq!(
        body.matches("data: ").count(),
        SSE_EVENTS,
        "all five events arrive: {body}"
    );

    events.wait_for("recorded").await;
    assert!(
        events.count("response_chunk") >= SSE_EVENTS,
        "each event is its own chunk, not one buffered blob: {:?}",
        events.names()
    );
}

// ---------------------------------------------------------------------------
// Zeile 7: curl, Transfer-Encoding: chunked
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_07_curl_chunked() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_07_curl_chunked", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("chunked.bin");
    let headers = tmp.path().join("headers.txt");

    let proxy_url = bridge.url();
    let header_file = headers.display().to_string();
    let url = format!("http://127.0.0.1:{}/chunked", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &["-x", &proxy_url, "-D", &header_file, &url],
    )
    .await;

    let line = result.first();
    assert_eq!(line.code, 200, "{}", result.run.report());
    assert_eq!(line.size_download as usize, CHUNK_SIZE * CHUNK_COUNT);
    assert_eq!(
        std::fs::read(&out).unwrap(),
        chunked_expected(),
        "the content survives unchanged"
    );
    // Der Proxy darf neu stückeln, aber die Antwort bleibt ohne
    // `Content-Length` und damit chunked.
    // Der Proxy darf neu stückeln; ohne bekannte Länge bleibt die Antwort
    // chunked, und `Content-Length` taucht nirgends auf.
    let head = read_text(&headers).to_ascii_lowercase();
    assert!(head.contains("transfer-encoding: chunked"), "{head}");
    assert!(!head.contains("content-length:"), "{head}");

    events.wait_for("recorded").await;
    assert!(events.count("response_chunk") >= 2, "{:?}", events.names());
}

// ---------------------------------------------------------------------------
// Zeile 8: curl, große Antwort
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_08_curl_big_download() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_08_curl_big_download", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("big.bin");

    let proxy_url = bridge.url();
    let url = format!("http://127.0.0.1:{}/big?mb=50", upstream.port());
    let started = Instant::now();
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, &url]).await;
    let elapsed = started.elapsed();

    let line = result.first();
    assert_eq!(line.code, 200, "{}", result.run.report());
    assert_eq!(line.size_download, 50 * 1024 * 1024);
    assert!(
        elapsed < Duration::from_secs(10),
        "50 MiB took {elapsed:?}, the budget is 10 s"
    );

    events.wait_for("recorded").await;
    // Der Beweis, dass nichts gepuffert wurde: viele Stücke statt eines.
    assert!(
        events.count("response_chunk") > 1,
        "the response streams: {:?}",
        events.names()
    );
}

// ---------------------------------------------------------------------------
// Zeile 9: curl, Umleitung mit -L
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_09_curl_redirect_two_flows() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_09_curl_redirect_two_flows", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");

    let proxy_url = bridge.url();
    // Erster Flow auf `127.0.0.1`, zweiter auf `localhost`: derselbe Server,
    // zwei verschiedene Hosts, also zwei verschiedene Entscheidungen.
    let url = format!(
        "http://127.0.0.1:{port}/redirect?to=http://localhost:{port}/echo",
        port = upstream.port()
    );
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, "-L", &url]).await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    let body = read_text(&out);
    assert_eq!(
        json_field(&body, "path").as_deref(),
        Some("/echo"),
        "{body}"
    );

    events.wait_for_nth("recorded", 2).await;
    assert_eq!(events.count("held"), 2, "both flows were held");
    let requests = events.received_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].authority.host.to_string(), "127.0.0.1");
    assert_eq!(requests[1].authority.host.to_string(), "localhost");
    assert_ne!(
        requests[0].authority.host, requests[1].authority.host,
        "the second flow has a different host"
    );
}

// ---------------------------------------------------------------------------
// Zeile 10: curl --http2
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_10_curl_http2_falls_back_to_http11() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_10_curl_http2_falls_back_to_http11", "curl");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");
    let ca_pem = proxy.ca_pem();

    let proxy_url = bridge.url();
    let ca = ca_pem.display().to_string();
    let url = format!("https://localhost:{}/echo", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &["-x", &proxy_url, "--cacert", &ca, "--http2", &url],
    )
    .await;

    let line = result.first();
    assert_eq!(line.code, 200, "{}", result.run.report());
    // In M1 bietet das Leaf des Proxys per ALPN nur `http/1.1` an
    // (`backlog/CONVENTIONS.md` 4.10). Die Aushandlung wird ausdrücklich
    // geprüft, damit ein stiller Rückfall nicht als grünes h2 durchgeht.
    assert_eq!(
        line.http_version, "1.1",
        "M1 negotiates http/1.1 with the client; h2 arrives in M6"
    );

    events.wait_for("recorded").await;
    assert_eq!(
        upstream.negotiated_alpn(),
        Some(b"http/1.1".to_vec()),
        "the upstream sees h1"
    );
}

// ---------------------------------------------------------------------------
// Zeile 11: WebSocket-Upgrade
// ---------------------------------------------------------------------------

/// Der Upgrade-Request wird wie jeder andere Flow gehalten und entschieden.
///
/// Der Handschlag selbst kommt in M1 nicht zustande: der Proxy entfernt vor
/// der Weiterleitung die Kopfzeilen von Verbindungsrang (`Connection`,
/// `Upgrade`), das Ziel sieht also eine gewöhnliche Anfrage. Der Test fährt
/// den Upgrade mit `curl`, weil `websocat` keinen HTTP-Proxy kennt; ist
/// `websocat` installiert, wird zusätzlich geprüft, dass der `wss`-Echo
/// wirklich nicht durchkommt.
#[tokio::test(flavor = "multi_thread")]
async fn conf_11_websocat_ws_upgrade_is_held() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_11_websocat_ws_upgrade_is_held", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Allow);
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.txt");

    let proxy_url = bridge.url();
    let url = format!("http://127.0.0.1:{}/ws", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &[
            "-x",
            &proxy_url,
            "-H",
            "Connection: Upgrade",
            "-H",
            "Upgrade: websocket",
            "-H",
            "Sec-WebSocket-Version: 13",
            "-H",
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
            &url,
        ],
    )
    .await;

    let line = result.first();
    assert_ne!(
        line.code, 101,
        "M1 does not pass a protocol switch through; see the module comment"
    );
    // Der Fake-Upstream antwortet mit 426, weil er die Anfrage ohne
    // `Upgrade`-Kopf sieht: der Proxy entfernt sie als Kopfzeilen von
    // Verbindungsrang. Ändert sich das (HUM-026), muss diese Zeile die neue
    // Erwartung tragen.
    assert_eq!(line.code, 426, "{}", result.run.report());
    events.wait_for("recorded").await;
    assert_eq!(events.count("held"), 1, "the upgrade request was held");
    let request = events.received_request();
    assert_eq!(request.path_and_query, "/ws");
    assert_eq!(
        request
            .headers
            .get("upgrade")
            .and_then(|value| value.to_str().ok()),
        Some("websocket"),
        "the flow that was held really is an upgrade request"
    );

    let Some(websocat) = clients::require("websocat") else {
        return clients::skip(
            "conf_11_websocat_ws_upgrade_is_held (websocat leg)",
            "`websocat` is not installed; the curl leg of this row still ran",
        );
    };
    // `websocat` kennt keinen HTTP-Proxy. Der `ws-c:`-Overlay legt den
    // Handschlag deshalb selbst auf eine TCP-Verbindung zur Brücke; die
    // Anfrage-URI nennt das eigentliche Ziel, und der Proxy nimmt die
    // Authority wie in Zeile 21 aus dem `Host`-Kopf.
    let mut cmd = websocat.command();
    cmd.arg("--binary")
        .arg("--one-message")
        .arg("--no-close")
        .arg(format!("--ws-c-uri=ws://127.0.0.1:{}/ws", upstream.port()))
        .arg("-")
        .arg(format!("ws-c:tcp:{}", bridge.addr));
    clients::apply(&mut cmd, &clients::env_kit(&proxy_url, &proxy.ca_pem()));
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;
    assert!(
        !run.success(),
        "M1 has no upgrade passthrough, so the echo cannot work: {}",
        run.report()
    );
    assert!(
        run.err().contains("426"),
        "the handshake reached the upstream through the proxy: {}",
        run.report()
    );
    events.wait_for_nth("recorded", 2).await;
    assert_eq!(
        events.count("held"),
        2,
        "websocat's upgrade request was held too: {:?}",
        events.names()
    );
}

// ---------------------------------------------------------------------------
// Zeile 12: grpcurl über h2c
// ---------------------------------------------------------------------------

/// gRPC im Klartext durch den Proxy: in M1 ein dokumentierter Fehlschlag.
///
/// `backlog/CONVENTIONS.md` 4.10: „gRPC-Zeile der Matrix ist in M1 erwartet:
/// fehlschlägt mit `PROXY_007 h2 not available`, grün ab M6." Der Proxy
/// spricht auf beiden Seiten HTTP/1.1, ein h2c-Prior-Knowledge-Vorspann kommt
/// deshalb nicht durch.
#[tokio::test(flavor = "multi_thread")]
async fn conf_12_grpcurl_plaintext_fails_without_h2() {
    let Some(grpcurl) = clients::require("grpcurl") else {
        return clients::skip_missing("conf_12_grpcurl_plaintext_fails_without_h2", "grpcurl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;

    let mut cmd = grpcurl.command();
    cmd.arg("-plaintext")
        .arg("-max-time")
        .arg("20")
        .arg(format!("grpc.test.invalid:{}", upstream.port()))
        .arg("humanitl.test.Echo/Echo");
    clients::apply(&mut cmd, &clients::env_kit(&bridge.url(), &proxy.ca_pem()));
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;

    assert!(
        !run.success(),
        "gRPC needs HTTP/2, which M1 does not offer: {}",
        run.report()
    );
    assert!(!run.out().contains("grpc-status: 0"), "{}", run.report());
    // `grpc.test.invalid` löst nirgends auf. Dass der Fehler kein
    // Namensfehler ist, beweist, dass grpcurl den Tunnel des Proxys benutzt
    // hat: der Name ging als `CONNECT`-Ziel an den Proxy, und dessen Resolver
    // beantwortet ihn. Loopback-Namen wären hier untauglich, weil Go für
    // `localhost` und `127.0.0.1` grundsätzlich keinen Proxy benutzt.
    let stderr = run.err().to_ascii_lowercase();
    assert!(!stderr.contains("no such host"), "{}", run.report());
    assert!(!stderr.contains("lookup"), "{}", run.report());
}

// ---------------------------------------------------------------------------
// Zeile 13: grpcurl über TLS
// ---------------------------------------------------------------------------

/// gRPC über TLS durch den Proxy: scheitert sichtbar an der ALPN-Aushandlung.
///
/// Das Leaf des Proxys bietet nur `http/1.1` an; `grpcurl` verlangt `h2`.
/// `docs/SECURITY.md` Abschnitt 5 und die Liste der bekannten Lücken nennen
/// das als `PROXY_007 h2 not available`, hinter `experimental.h2_upstream` ab
/// M6.
#[tokio::test(flavor = "multi_thread")]
async fn conf_13_grpcurl_tls_needs_h2_upstream() {
    let Some(grpcurl) = clients::require("grpcurl") else {
        return clients::skip_missing("conf_13_grpcurl_tls_needs_h2_upstream", "grpcurl");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let ca_pem = proxy.ca_pem();

    let mut cmd = grpcurl.command();
    cmd.arg("-cacert")
        .arg(&ca_pem)
        .arg("-max-time")
        .arg("20")
        .arg(format!("grpc.test.invalid:{}", upstream.port()))
        .arg("humanitl.test.Echo/Echo");
    clients::apply(&mut cmd, &clients::env_kit(&bridge.url(), &ca_pem));
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;

    assert!(
        !run.success(),
        "gRPC over TLS needs `experimental.h2_upstream`, which M1 does not have: {}",
        run.report()
    );
    // Genau der sichtbare Fehlschlag, den `docs/SECURITY.md` Abschnitt 5
    // beschreibt: das Leaf des Proxys bietet `h2` nicht an, grpcurl bietet
    // nichts anderes an, der Handschlag endet mit `no_application_protocol`.
    assert!(
        run.err()
            .to_ascii_lowercase()
            .contains("no application protocol"),
        "the ALPN refusal is the documented PROXY_007 failure: {}",
        run.report()
    );
}

// ---------------------------------------------------------------------------
// Zeile 14: python3 urllib
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_14_python3_urllib_https() {
    let Some(python) = clients::require("python3") else {
        return clients::skip_missing("conf_14_python3_urllib_https", "python3");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();

    let script = "\
import sys, urllib.request
with urllib.request.urlopen(sys.argv[1], timeout=30) as response:
    sys.stdout.write(str(response.status) + '\\n')
    sys.stdout.write(response.read().decode())
";
    let mut cmd = python.command();
    cmd.arg("-c")
        .arg(script)
        .arg(format!("https://localhost:{}/echo", upstream.port()));
    clients::apply(&mut cmd, &clients::env_kit(&bridge.url(), &proxy.ca_pem()));
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;

    assert!(run.success(), "{}", run.report());
    let out = run.out();
    assert!(out.starts_with("200\n"), "{}", run.report());
    assert_eq!(json_field(&out, "path").as_deref(), Some("/echo"), "{out}");

    events.wait_for("recorded").await;
    assert_eq!(events.received_request().scheme, Scheme::Https);
}

// ---------------------------------------------------------------------------
// Zeile 15: python3 requests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_15_python3_requests_https() {
    let Some(python) = clients::require("python3") else {
        return clients::skip_missing("conf_15_python3_requests_https", "python3");
    };
    if !clients::python_has_module(&python, "requests").await {
        return clients::skip(
            "conf_15_python3_requests_https",
            "the python module `requests` is not installed",
        );
    }
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();

    let script = "\
import sys, requests
response = requests.get(sys.argv[1], timeout=30)
sys.stdout.write(str(response.status_code) + '\\n')
sys.stdout.write(response.text)
";
    let mut cmd = python.command();
    cmd.arg("-c")
        .arg(script)
        .arg(format!("https://localhost:{}/echo", upstream.port()));
    clients::apply(&mut cmd, &clients::env_kit(&bridge.url(), &proxy.ca_pem()));
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;

    assert!(run.success(), "{}", run.report());
    let out = run.out();
    assert!(out.starts_with("200\n"), "{}", run.report());
    assert_eq!(json_field(&out, "path").as_deref(), Some("/echo"), "{out}");

    events.wait_for("recorded").await;
}

// ---------------------------------------------------------------------------
// Zeile 16: git ls-remote
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_16_git_ls_remote_https() {
    let Some(git) = clients::require("git") else {
        return clients::skip_missing("conf_16_git_ls_remote_https", "git");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();

    let mut cmd = git.command();
    cmd.arg("ls-remote")
        .arg(format!("https://localhost:{}/repo.git", upstream.port()));
    clients::apply(&mut cmd, &clients::env_kit(&bridge.url(), &proxy.ca_pem()));
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;

    let stderr = run.err().to_ascii_lowercase();
    for hint in [
        "ssl certificate problem",
        "certificate verify failed",
        "unable to get local issuer",
        "self-signed certificate",
    ] {
        assert!(!stderr.contains(hint), "TLS broke: {}", run.report());
    }

    events.wait_for("recorded").await;
    let request = events.received_request();
    assert!(
        request.path_and_query.starts_with("/repo.git/info/refs"),
        "git asked for {}",
        request.path_and_query
    );
    assert_eq!(
        events.statuses().first(),
        Some(&200),
        "the HTTP answer arrived"
    );
    assert!(upstream.hits() >= 1);
}

// ---------------------------------------------------------------------------
// Zeile 17: node fetch
// ---------------------------------------------------------------------------

/// `fetch` durch den Proxy.
///
/// Node kennt Proxy-Umgebungsvariablen für `fetch` erst ab Version 24
/// (`NODE_USE_ENV_PROXY`). Auf älteren Versionen baut das Skript den Tunnel
/// selbst mit `CONNECT` und `tls.connect`; geprüft wird in beiden Fällen
/// dasselbe: Status 200 und keine TLS-Beschwerde.
#[tokio::test(flavor = "multi_thread")]
async fn conf_17_node_fetch_https() {
    let Some(node) = clients::require("node") else {
        return clients::skip_missing("conf_17_node_fetch_https", "node");
    };
    let upstream_ca = UpstreamCa::new();
    let upstream = FakeUpstream::matrix_tls(upstream_ca.store()).await;
    let proxy = ProxyBuilder::new()
        .passthrough()
        .trust(upstream_ca.cert_der())
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();

    let mut cmd = node.command();
    cmd.arg("-e")
        .arg(NODE_FETCH_SCRIPT)
        .arg(format!("https://localhost:{}/echo", upstream.port()));
    clients::apply(&mut cmd, &clients::env_kit(&bridge.url(), &proxy.ca_pem()));
    cmd.env("NODE_USE_ENV_PROXY", "1");
    // Der Zweig ohne `fetch` lässt sich lokal erzwingen, damit beide Wege
    // geprüft werden können: `HUMANITL_NODE_MANUAL=1 cargo test conf_17`.
    if std::env::var_os("HUMANITL_NODE_MANUAL").is_some() {
        cmd.env("HUMANITL_NODE_MANUAL", "1");
    }
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;

    assert!(run.success(), "{}", run.report());
    let out = run.out();
    assert!(out.contains("200"), "{}", run.report());
    assert_eq!(json_field(&out, "path").as_deref(), Some("/echo"), "{out}");

    events.wait_for("recorded").await;
    assert_eq!(events.received_request().scheme, Scheme::Https);
}

/// Das Skript für Zeile 17. `HUMANITL_NODE_MANUAL` erzwingt den Weg ohne
/// `fetch`, damit beide Zweige geprüft werden können.
const NODE_FETCH_SCRIPT: &str = r"
const url = process.argv[1];
const major = Number(process.versions.node.split('.')[0]);
const useFetch = major >= 24 && !process.env.HUMANITL_NODE_MANUAL;
if (useFetch) {
  fetch(url)
    .then(async (response) => {
      const body = await response.text();
      process.stdout.write(String(response.status) + '\n' + body);
      process.exit(response.status === 200 ? 0 : 1);
    })
    .catch((error) => {
      console.error(error);
      process.exit(2);
    });
} else {
  const http = require('node:http');
  const tls = require('node:tls');
  const fs = require('node:fs');
  const target = new URL(url);
  const proxy = new URL(process.env.https_proxy);
  const port = target.port || '443';
  const request = http.request({
    host: proxy.hostname,
    port: proxy.port,
    method: 'CONNECT',
    path: target.hostname + ':' + port,
  });
  request.on('connect', (response, socket) => {
    if (response.statusCode !== 200) {
      console.error('CONNECT failed with ' + response.statusCode);
      process.exit(2);
    }
    const secure = tls.connect(
      { socket, servername: target.hostname, ca: fs.readFileSync(process.env.SSL_CERT_FILE) },
      () => {
        secure.write(
          'GET ' + target.pathname + ' HTTP/1.1\r\nHost: ' + target.host + '\r\nConnection: close\r\n\r\n'
        );
      }
    );
    let data = '';
    secure.on('data', (chunk) => {
      data += chunk;
    });
    secure.on('end', () => {
      process.stdout.write(data);
      process.exit(data.startsWith('HTTP/1.1 200') ? 0 : 1);
    });
    secure.on('error', (error) => {
      console.error(error);
      process.exit(2);
    });
  });
  request.on('error', (error) => {
    console.error(error);
    process.exit(2);
  });
  request.end();
}
";

// ---------------------------------------------------------------------------
// Zeile 18: Block durch die Pipeline
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_18_curl_blocked_by_pipeline() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_18_curl_blocked_by_pipeline", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::Block {
        reason: BlockReason::User,
        note: Some("nicht in diesem Lauf".to_owned()),
    });
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.txt");
    let headers = tmp.path().join("headers.txt");

    let proxy_url = bridge.url();
    let header_file = headers.display().to_string();
    let url = format!("http://127.0.0.1:{}/echo", upstream.port());
    let result = curl_run(
        &curl,
        &out,
        &[],
        &["-x", &proxy_url, "-D", &header_file, &url],
    )
    .await;

    assert_eq!(result.first().code, 403, "{}", result.run.report());
    let head = read_text(&headers).to_ascii_lowercase();
    assert!(head.contains("connection: close"), "{head}");
    assert!(head.contains("x-humanitl-flow:"), "{head}");

    events.wait_for("recorded").await;
    let flow = match events.seen.first() {
        Some(humanitl_core::FlowEvent::Received { flow_id, .. }) => flow_id.to_string(),
        other => panic!("the first event is Received, got {other:?}"),
    };
    let body = read_text(&out);
    assert_eq!(
        body,
        format!(
            "Blocked by Humanitl.\nreason: user\nflow: {flow}\nhost: 127.0.0.1\nnote: nicht in diesem Lauf\n"
        ),
        "the canonical block body (CONVENTIONS.md 3.5)"
    );
    assert_eq!(upstream.hits(), 0);
}

// ---------------------------------------------------------------------------
// Zeile 19: Wartezeit läuft ab
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_19_curl_hold_timeout() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_19_curl_hold_timeout", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    // Niemand entscheidet: nach einer Sekunde greift die Frist.
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(1))
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.txt");

    let proxy_url = bridge.url();
    let url = format!("http://127.0.0.1:{}/echo", upstream.port());
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, &url]).await;

    let line = result.first();
    // `504`, nicht `403`: die Review-Korrektur vom 2026-09-02 in
    // `backlog/sprint-1.md` legt die abgelaufene Wartezeit auf 504 fest.
    assert_eq!(line.code, 504, "{}", result.run.report());
    assert!(
        (0.8..5.0).contains(&line.time_total),
        "the answer comes after about one second, took {}s",
        line.time_total
    );
    let body = read_text(&out);
    assert!(body.contains("reason: timeout"), "{body}");
    assert_eq!(upstream.hits(), 0);

    events.wait_for("recorded").await;
    assert_eq!(events.count("timed_out"), 1, "{:?}", events.names());
}

// ---------------------------------------------------------------------------
// Zeile 20: IPv6-Literal
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_20_curl_ipv6_literal() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_20_curl_ipv6_literal", "curl");
    };
    let Some(upstream) = FakeUpstream::matrix_on(IpAddr::V6(Ipv6Addr::LOCALHOST)).await else {
        return clients::skip(
            "conf_20_curl_ipv6_literal",
            "this machine has no IPv6 loopback to bind",
        );
    };
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");

    let proxy_url = bridge.url();
    let url = format!("http://[::1]:{}/echo", upstream.port());
    let result = curl_run(&curl, &out, &[], &["-x", &proxy_url, &url]).await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    events.wait_for("recorded").await;
    let request = events.received_request();
    assert_eq!(
        request.authority.host,
        HostName::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)),
        "the literal became HostName::Ip"
    );
    assert_eq!(request.authority.port, upstream.port());
}

// ---------------------------------------------------------------------------
// Zeile 21: Authority nur im Host-Kopf
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_21_curl_authority_from_host_header() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_21_curl_authority_from_host_header", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new().passthrough().start().await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("body.json");

    // Kein `-x`: curl spricht die Brücke wie einen Ursprungs-Server an und
    // schickt eine relative URI. Das Ziel steht nur im `Host`-Kopf.
    let host = format!("Host: 127.0.0.1:{}", upstream.port());
    let url = format!("http://{}/echo", bridge.addr);
    let result = curl_run(&curl, &out, &[], &["-H", &host, &url]).await;

    assert_eq!(result.first().code, 200, "{}", result.run.report());
    let body = read_text(&out);
    assert_eq!(
        json_field(&body, "path").as_deref(),
        Some("/echo"),
        "{body}"
    );

    events.wait_for("recorded").await;
    let request = events.received_request();
    assert_eq!(request.authority.host.to_string(), "127.0.0.1");
    assert_eq!(
        request.authority.port,
        upstream.port(),
        "the authority comes from the Host header, not from the connection"
    );
    assert_eq!(request.scheme, Scheme::Http);
}

// ---------------------------------------------------------------------------
// Zeile 22: zweite Anfrage nach einem Block
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn conf_22_curl_second_request_after_block() {
    let Some(curl) = clients::require("curl") else {
        return clients::skip_missing("conf_22_curl_second_request_after_block", "curl");
    };
    let upstream = FakeUpstream::matrix().await;
    let proxy = ProxyBuilder::new()
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let bridge = proxy.bridge().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_each(|index| {
        if index == 0 {
            Decision::Block {
                reason: BlockReason::User,
                note: None,
            }
        } else {
            Decision::Allow
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("first.txt");
    let second = tmp.path().join("second.json");

    let proxy_url = bridge.url();
    let second_out = second.display().to_string();
    let url = format!("http://127.0.0.1:{}/echo", upstream.port());
    let result = curl_run(
        &curl,
        &first,
        &[],
        &["-x", &proxy_url, "-o", &second_out, &url, &url],
    )
    .await;

    assert_eq!(result.lines.len(), 2, "{}", result.run.report());
    assert_eq!(result.lines[0].code, 403, "{}", result.run.report());
    assert_eq!(
        result.lines[1].code,
        200,
        "the second request is its own flow: {}",
        result.run.report()
    );
    assert!(read_text(&first).contains("reason: user"));
    let body = read_text(&second);
    assert_eq!(
        json_field(&body, "path").as_deref(),
        Some("/echo"),
        "{body}"
    );

    events.wait_for_nth("recorded", 2).await;
    let requests = events.received_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(events.count("held"), 2);
    assert_eq!(upstream.hits(), 1, "only the allowed request went out");
}

// ---------------------------------------------------------------------------
// Werkzeug
// ---------------------------------------------------------------------------

/// Eine Zeile aus `curl --write-out`.
#[derive(Debug)]
struct CurlLine {
    code: u16,
    time_starttransfer: f64,
    time_total: f64,
    size_download: u64,
    http_version: String,
}

/// Ein Lauf von `curl` samt ausgewerteten `--write-out`-Zeilen.
#[derive(Debug)]
struct CurlRun {
    run: Run,
    lines: Vec<CurlLine>,
}

impl CurlRun {
    /// Die erste Zeile; jeder Test schickt mindestens eine Anfrage.
    fn first(&self) -> &CurlLine {
        self.lines
            .first()
            .unwrap_or_else(|| panic!("curl wrote no result line: {}", self.run.report()))
    }
}

/// Fährt `curl` mit `--write-out`, schreibt den Körper nach `out`.
///
/// `LC_ALL=C` erzwingt den Punkt als Dezimaltrenner in den Zeitwerten.
async fn curl_run(curl: &Tool, out: &Path, env: &[(String, String)], args: &[&str]) -> CurlRun {
    let mut cmd = curl.command();
    cmd.arg("--silent")
        .arg("--show-error")
        .arg("--output")
        .arg(out)
        .arg("--write-out")
        .arg(WRITE_OUT)
        .env("LC_ALL", "C");
    clients::apply(&mut cmd, env);
    for arg in args {
        cmd.arg(arg);
    }
    let run = clients::run(cmd, CLIENT_TIMEOUT).await;
    let lines = run
        .out()
        .lines()
        .filter_map(parse_write_out)
        .collect::<Vec<_>>();
    CurlRun { run, lines }
}

fn parse_write_out(line: &str) -> Option<CurlLine> {
    let mut fields = line.split_whitespace();
    Some(CurlLine {
        code: fields.next()?.parse().ok()?,
        time_starttransfer: fields.next()?.parse().ok()?,
        time_total: fields.next()?.parse().ok()?,
        size_download: fields.next()?.parse().ok()?,
        http_version: fields.next()?.to_owned(),
    })
}

/// Eine Datei als Text lesen, verlustbehaftet.
fn read_text(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).into_owned()
}

/// Ein Feld aus dem JSON des Fake-Upstreams.
///
/// Reicht für die flachen Felder dieser Antwort; die Werte, die die Tests
/// abfragen, enthalten keine maskierten Anführungszeichen.
fn json_field(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":");
    let start = body.find(&needle)? + needle.len();
    let rest = body[start..].trim_start();
    if let Some(text) = rest.strip_prefix('"') {
        let end = text.find('"')?;
        return Some(text[..end].to_owned());
    }
    let end = rest.find([',', '}'])?;
    Some(rest[..end].trim().to_owned())
}

/// Ein wiederholbarer Puffer, dessen Bytes nicht alle gleich sind.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index * 31 + 7) as u8).collect()
}

/// Hex-Darstellung, kleingeschrieben.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
