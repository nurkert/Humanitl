//! Die Prüfungen hinter den Fuzz-Targets `http_request_path` und
//! `chunked_decoder` (HUM-056).
//!
//! Diese Datei ist die eine Quelle beider Seiten: Die Regressionstests in
//! `main.rs` binden sie als Modul ein, und die Targets unter
//! `daemon/fuzz/fuzz_targets/` binden sie über `#[path]` ein. Was der Fuzzer
//! prüft, prüft damit auch `cargo test`, Byte für Byte mit derselben Funktion.
//!
//! Beide Prüfungen laufen durch den Parser, den der Proxy wirklich benutzt:
//! hyper, gebaut wie in `handler::serve_connection`, über einem
//! `tokio::io::duplex` statt einem Socket. Ein eigener Parser nur für das
//! Fuzzing wäre wertlos (`backlog/sprint-5.md`, HUM-056, Fallstricke): Er fände
//! Fehler, die der Proxy nicht hat, und übersähe die, die er hat.
//!
//! Eine Prüfung schlägt mit einem Panic fehl; libFuzzer meldet ihn als Fund,
//! `cargo test` als roten Test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::convert::Infallible;
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use http_body_util::Full;
use humanitl_core::{HostName, SessionId};
use humanitl_proxy::body::{self, BufferError};
use humanitl_proxy::connect::requested_upgrade;
use humanitl_proxy::{ConnectionContext, check_authority};
use humanitl_rules::{RequestKey, RuleSet, parse_rules_for_session};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// Die Obergrenze des Request-Bodys im Target `chunked_decoder`.
///
/// Klein, damit der Fuzzer den Pfad über dem Cap überhaupt erreicht: Seine
/// Eingaben sind wenige KiB lang, das Cap des Proxys liegt bei 32 MiB.
pub const CHUNKED_CAP: u64 = 512;

/// Der Kopf, vor den das Target `chunked_decoder` seine Eingabe setzt.
pub const CHUNKED_HEAD: &[u8] =
    b"POST /upload HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n";

/// Prüft eine rohe Anfrage auf dem Weg vom Byte zum Ziel eines Flows.
///
/// Jede Anfrage, die hyper aus `data` liest, geht durch
/// [`check_authority`] (ohne Tunnel und, wenn das gelingt, im Tunnel zu genau
/// diesem Ziel), durch [`requested_upgrade`] und durch [`RuleSet::evaluate`].
///
/// Invarianten:
///
/// - Kein Panic.
/// - Ein Ziel, das die Prüfung besteht, ist normalisiert: ein DNS-Name klein
///   geschrieben, ohne Punkt am Ende, ohne leeres Label, in DNS-Längen; und
///   [`HostName::parse`] seiner kanonischen Form ergibt denselben Host.
/// - Im Tunnel zu dem Ziel, das ohne Tunnel herauskam, mit passender SNI,
///   bleibt das Ziel dasselbe, wenn die Prüfung besteht.
pub fn http_request_path(data: &[u8]) {
    serve(data.to_vec(), |req: Request<Incoming>| async move {
        inspect_target(&req);
    });
}

/// Prüft das Lesen eines `chunked`-Bodys bis zum Cap.
///
/// `data` folgt auf [`CHUNKED_HEAD`]; jede Anfrage, die hyper daraus liest,
/// wird mit [`body::buffer`] bis [`CHUNKED_CAP`] gepuffert.
///
/// Invarianten: kein Panic, kein Hänger, und ein Body, der als vollständig
/// zurückkommt, ist nie länger als das Cap.
pub fn chunked_decoder(data: &[u8]) {
    let mut input = CHUNKED_HEAD.to_vec();
    input.extend_from_slice(data);
    serve(input, |req: Request<Incoming>| async move {
        if let Ok(bytes) = buffer_body(req).await {
            assert!(
                bytes.len() as u64 <= CHUNKED_CAP,
                "buffered {} bytes over a cap of {CHUNKED_CAP}",
                bytes.len()
            );
        }
    });
}

/// Puffert den Body einer Anfrage so, wie der Handler es tut.
///
/// # Errors
///
/// Der Fehler von [`body::buffer`].
pub async fn buffer_body(req: Request<Incoming>) -> Result<Bytes, BufferError> {
    body::buffer(req.into_body(), CHUNKED_CAP, Duration::from_secs(5)).await
}

/// Puffert den ersten Body, den hyper aus `data` liest, und gibt das Ergebnis
/// zurück; `None`, wenn hyper gar keine Anfrage gelesen hat.
pub fn first_buffered(data: &[u8]) -> Option<Result<Bytes, BufferError>> {
    let mut input = CHUNKED_HEAD.to_vec();
    input.extend_from_slice(data);
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let slot = std::sync::Arc::clone(&seen);
    serve(input, move |req: Request<Incoming>| {
        let slot = std::sync::Arc::clone(&slot);
        async move {
            let result = buffer_body(req).await;
            let mut guard = slot.lock().unwrap();
            if guard.is_none() {
                *guard = Some(result);
            }
        }
    });
    seen.lock().unwrap().take()
}

/// Die Ziele, die [`check_authority`] ohne Tunnel aus den Anfragen in `data`
/// liest, in kanonischer Form; `None` für eine Anfrage ohne gültiges Ziel.
///
/// Belegt im Test, dass [`http_request_path`] die Anfragen wirklich sieht.
pub fn plain_targets(data: &[u8]) -> Vec<Option<String>> {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let slot = std::sync::Arc::clone(&seen);
    serve(data.to_vec(), move |req: Request<Incoming>| {
        let slot = std::sync::Arc::clone(&slot);
        async move {
            let plain = ConnectionContext::plain(SessionId::nil());
            let target = check_authority(&plain, &req)
                .ok()
                .map(|target| target.authority.to_string());
            slot.lock().unwrap().push(target);
        }
    });
    std::mem::take(&mut *seen.lock().unwrap())
}

/// Die Prüfungen am Ziel einer einzelnen Anfrage.
fn inspect_target(req: &Request<Incoming>) {
    let _ = requested_upgrade(req.headers());

    let plain = ConnectionContext::plain(SessionId::nil());
    let Ok(target) = check_authority(&plain, req) else {
        return;
    };
    assert_normalized(&target.authority.host);
    evaluate(
        &target.authority.host,
        req,
        target.scheme,
        target.authority.port,
    );

    let tunnel = plain.tunnel(
        target.authority.clone(),
        Some(target.authority.host.clone()),
    );
    if let Ok(inner) = check_authority(&tunnel, req) {
        assert_eq!(
            inner.authority, target.authority,
            "a tunnel changed the target it was opened for"
        );
        assert_normalized(&inner.authority.host);
    }
}

/// Wertet das Ziel gegen einen festen Regelsatz aus, damit Host-Glob und
/// Pfadmuster fremde Eingaben sehen.
fn evaluate(host: &HostName, req: &Request<Incoming>, scheme: humanitl_core::Scheme, port: u16) {
    let path = req
        .uri()
        .path_and_query()
        .map_or("/", hyper::http::uri::PathAndQuery::as_str);
    let key = RequestKey::new(host, req.method(), path, scheme, port);
    let _ = rules().evaluate(&key, fixed_now(), SessionId::nil());
}

/// Der feste Regelsatz der Auswertung: Glob, Doppelstern, Regex, Adresse, Netz.
fn rules() -> &'static RuleSet {
    static RULES: OnceLock<RuleSet> = OnceLock::new();
    RULES.get_or_init(|| {
        let yaml = "version: 1\nrules:\n\
            \x20 - action: allow\n    match:\n      host: \"*.github.com\"\n      path: \"/repos/**\"\n\
            \x20 - action: block\n    match:\n      host: \"**.example.com\"\n      path: \"~^/v[0-9]+/\"\n\
            \x20 - action: allow\n    match:\n      host: \"ip:192.168.1.50\"\n\
            \x20 - action: block\n    match:\n      host: \"cidr:10.0.0.0/8\"\n";
        match parse_rules_for_session(yaml, SessionId::nil()) {
            Ok((set, _)) => set,
            Err(err) => panic!("fixed rules do not parse: {err:?}"),
        }
    })
}

/// Der feste Zeitpunkt jeder Auswertung; ein Target liest keine Uhr.
fn fixed_now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap_or_default()
}

/// Ein normalisierter Host: siehe [`http_request_path`].
pub fn assert_normalized(host: &HostName) {
    if let HostName::Dns(name) = host {
        assert!(!name.is_empty(), "empty host name");
        assert!(!name.ends_with('.'), "trailing dot in {name:?}");
        assert!(name.len() <= 253, "host name longer than 253: {name:?}");
        assert_eq!(
            name.to_ascii_lowercase(),
            *name,
            "host name not lower case: {name:?}"
        );
        assert!(name.is_ascii(), "host name not in A-label form: {name:?}");
        for label in name.split('.') {
            assert!(
                !label.is_empty() && label.len() <= 63,
                "label {label:?} in {name:?} is empty or over 63 bytes"
            );
        }
    }
    let again = HostName::parse(&host.to_string())
        .unwrap_or_else(|err| panic!("the canonical form of {host:?} does not parse: {err:?}"));
    assert_eq!(&again, host, "normalising {host:?} twice changed it");
}

/// Füttert hyper mit `input` und ruft `inspect` für jede gelesene Anfrage.
///
/// Der Server ist gebaut wie in `handler::serve_connection`. Der Client
/// schreibt die Eingabe und schließt seine Richtung; hyper liest bis zum
/// Dateiende und beendet die Verbindung. Seine Antworten werden gleichzeitig
/// gelesen, damit ein voller Puffer keinen der beiden anhält.
fn serve<F, Fut>(input: Vec<u8>, inspect: F)
where
    F: Fn(Request<Incoming>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    runtime().block_on(async move {
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let service = service_fn(move |req: Request<Incoming>| {
            let inspect = inspect.clone();
            async move {
                inspect(req).await;
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::new())))
            }
        });
        let connection = hyper::server::conn::http1::Builder::new()
            .timer(TokioTimer::new())
            .header_read_timeout(Duration::from_secs(5))
            .preserve_header_case(true)
            .title_case_headers(true)
            // Abweichung vom Handler: Der Client schließt nach der Eingabe
            // seine Richtung. Ohne `half_close` verwürfe hyper beim
            // Dateiende die Anfrage, die gerade bedient wird, und die
            // Prüfung sähe nie eine vollständige Anfrage.
            .half_close(true)
            .serve_connection(TokioIo::new(server), service)
            .with_upgrades();
        let (mut reader, mut writer) = tokio::io::split(&mut client);
        let write = async move {
            let _ = writer.write_all(&input).await;
            let _ = writer.shutdown().await;
        };
        let read = async move {
            let mut sink = Vec::new();
            let _ = reader.read_to_end(&mut sink).await;
        };
        let (_, (), ()) = tokio::join!(connection, write, read);
    });
}

/// Die Laufzeit der Prüfungen: ein Thread, mit Uhr, einmal je Thread gebaut.
fn runtime() -> &'static tokio::runtime::Runtime {
    thread_local! {
        static RUNTIME: &'static tokio::runtime::Runtime = Box::leak(Box::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ));
    }
    RUNTIME.with(|runtime| *runtime)
}
