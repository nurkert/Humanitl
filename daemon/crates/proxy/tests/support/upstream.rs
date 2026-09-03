//! Der Fake-Upstream der Integrationstests (HUM-015, HUM-017).
//!
//! Ein axum-Server auf Loopback, wahlweise Klartext oder TLS mit einem Leaf
//! aus einer übergebenen CA. Zwei Speisekarten teilen sich denselben Server:
//!
//! - [`Routes::Legacy`] ist die kleine Karte aus HUM-015, die `tests/proxy.rs`
//!   erwartet: `/echo` antwortet mit festem Text, `/stream` mit zwei Stücken.
//! - [`Routes::Matrix`] ist die Karte der Konformitäts-Matrix aus HUM-017:
//!   `/echo` antwortet als JSON mit Methode, Pfad, Headern, Body-Länge und
//!   Body-sha256, dazu die Endpunkte für SSE, chunked, große Antworten,
//!   Umleitungen, Verzögerungen, beliebige Statuscodes und `git`.
//!
//! Beide Karten teilen sich den Zähler `hits`. Er zählt jede angekommene
//! Anfrage; damit beweisen Tests, dass vor der Entscheidung nichts ankommt.

use std::convert::Infallible;
use std::fmt::Write as _;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post as route_post};
use axum::serve::Listener;
use bytes::Bytes;
use humanitl_core::HostName;
use humanitl_proxy::ca::CaStore;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, LOCATION};
use hyper::{HeaderMap, Method, StatusCode, Uri};
use rustls::ServerConfig;
use rustls::pki_types::CertificateDer;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_stream::wrappers::ReceiverStream;

/// Was `GET /echo` auf der Karte [`Routes::Legacy`] liefert.
pub const ECHO_BODY: &str = "hello from upstream\n";

/// Größe eines Stücks von `GET /chunked`.
pub const CHUNK_SIZE: usize = 64 * 1024;
/// Anzahl der Stücke von `GET /chunked`.
pub const CHUNK_COUNT: usize = 10;
/// Pause zwischen zwei Stücken von `GET /chunked`.
const CHUNK_PAUSE: Duration = Duration::from_millis(50);
/// Vorgabe für den Abstand der Ereignisse von `GET /sse`.
const SSE_DEFAULT_INTERVAL_MS: u64 = 200;
/// Anzahl der Ereignisse von `GET /sse`.
pub const SSE_EVENTS: usize = 5;
/// Obergrenze für `GET /big?mb=N`, damit ein Tippfehler den Rechner nicht auffrisst.
const BIG_MAX_MB: usize = 256;

/// Der Inhalt, den `GET /chunked` liefert: zehn Stücke à 64 KiB, jedes mit
/// einem eigenen Füllbyte, damit eine vertauschte Reihenfolge auffällt.
#[must_use]
pub fn chunked_expected() -> Vec<u8> {
    let mut out = Vec::with_capacity(CHUNK_SIZE * CHUNK_COUNT);
    for index in 0..CHUNK_COUNT {
        out.extend(std::iter::repeat_n(chunk_byte(index), CHUNK_SIZE));
    }
    out
}

fn chunk_byte(index: usize) -> u8 {
    b'A' + u8::try_from(index).unwrap_or(0)
}

/// Welche Endpunkte der Fake-Upstream bedient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routes {
    /// Die Karte aus HUM-015: `/echo` als Text, `/stream`.
    Legacy,
    /// Die Karte der Konformitäts-Matrix aus HUM-017.
    Matrix,
}

/// Eine eigene CA für den Fake-Upstream, getrennt von der Humanitl-CA des
/// Proxys (HUM-017: „Test-CA getrennt von der Humanitl-CA").
///
/// Der Proxy bekommt ihr Zertifikat im Test als zusätzliche Wurzel
/// ([`ProxyBuilder::trust`](super::ProxyBuilder::trust)); der Client des Tests
/// vertraut ihr nicht, sondern nur der Humanitl-CA. So beweist ein grüner
/// Test, dass der Proxy wirklich in der Mitte steht.
pub struct UpstreamCa {
    store: CaStore,
    _tmp: TempDir,
}

impl Default for UpstreamCa {
    fn default() -> Self {
        Self::new()
    }
}

impl UpstreamCa {
    /// Eine frische CA in einem eigenen Temp-Verzeichnis.
    #[must_use]
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let store = CaStore::load_or_create(&tmp.path().join("upstream-ca")).unwrap();
        Self { store, _tmp: tmp }
    }

    /// Die CA selbst, für [`FakeUpstream::tls`].
    #[must_use]
    pub const fn store(&self) -> &CaStore {
        &self.store
    }

    /// Ihr Zertifikat, für [`ProxyBuilder::trust`](super::ProxyBuilder::trust).
    #[must_use]
    pub fn cert_der(&self) -> CertificateDer<'static> {
        self.store.cert_der().clone()
    }
}

/// Ein axum-Server auf Loopback, Klartext oder TLS.
pub struct FakeUpstream {
    /// Adresse, auf der der Server lauscht.
    pub addr: SocketAddr,
    /// Zählt jede angekommene Anfrage.
    pub hits: Arc<AtomicUsize>,
    /// Das zuletzt ausgehandelte ALPN (nur TLS).
    pub alpn: Arc<Mutex<Option<Vec<u8>>>>,
    task: JoinHandle<()>,
}

impl FakeUpstream {
    /// Klartext-HTTP mit der Karte [`Routes::Legacy`] auf `127.0.0.1`.
    pub async fn plain() -> Self {
        Self::bind(Routes::Legacy, IpAddr::V4(Ipv4Addr::LOCALHOST), None)
            .await
            .unwrap()
    }

    /// Klartext-HTTP mit der Karte [`Routes::Matrix`] auf `127.0.0.1`.
    pub async fn matrix() -> Self {
        Self::bind(Routes::Matrix, IpAddr::V4(Ipv4Addr::LOCALHOST), None)
            .await
            .unwrap()
    }

    /// Wie [`FakeUpstream::matrix`], aber auf einer beliebigen Loopback-Adresse.
    ///
    /// Liefert `None`, wenn die Adresse nicht gebunden werden kann; auf
    /// Maschinen ohne IPv6-Loopback überspringt der Test dann seine Zeile.
    pub async fn matrix_on(ip: IpAddr) -> Option<Self> {
        Self::bind(Routes::Matrix, ip, None).await.ok()
    }

    /// TLS mit einem Leaf für `localhost` aus `ca`, Karte [`Routes::Legacy`].
    ///
    /// Bietet dem Proxy `h2` und `http/1.1` an; der Proxy muss `http/1.1`
    /// wählen, weil er in M1 nach oben nur HTTP/1.1 spricht.
    pub async fn tls(ca: &CaStore) -> Self {
        Self::bind(Routes::Legacy, IpAddr::V4(Ipv4Addr::LOCALHOST), Some(ca))
            .await
            .unwrap()
    }

    /// Wie [`FakeUpstream::tls`], aber mit der Karte [`Routes::Matrix`].
    pub async fn matrix_tls(ca: &CaStore) -> Self {
        Self::bind(Routes::Matrix, IpAddr::V4(Ipv4Addr::LOCALHOST), Some(ca))
            .await
            .unwrap()
    }

    async fn bind(routes: Routes, ip: IpAddr, ca: Option<&CaStore>) -> io::Result<Self> {
        let hits = Arc::new(AtomicUsize::new(0));
        let alpn = Arc::new(Mutex::new(None));
        let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
        let addr = listener.local_addr()?;
        let app = router(Arc::clone(&hits), routes);
        let task = match ca {
            None => tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            }),
            Some(ca) => {
                let tls = TlsListener {
                    inner: listener,
                    acceptor: TlsAcceptor::from(Arc::new(server_config(ca))),
                    alpn: Arc::clone(&alpn),
                };
                tokio::spawn(async move {
                    axum::serve(tls, app).await.unwrap();
                })
            }
        };
        Ok(Self {
            addr,
            hits,
            alpn,
            task,
        })
    }

    /// Der Port, auf dem der Server lauscht.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Wie viele Anfragen angekommen sind.
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    /// Das zuletzt ausgehandelte ALPN (nur TLS).
    pub fn negotiated_alpn(&self) -> Option<Vec<u8>> {
        self.alpn.lock().unwrap().clone()
    }
}

impl Drop for FakeUpstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Die rustls-Serverkonfiguration des Fake-Upstreams: Leaf für `localhost`
/// aus `ca`, ALPN `h2` und `http/1.1`.
fn server_config(ca: &CaStore) -> ServerConfig {
    let leaf = ca
        .issue_leaf(&HostName::Dns("localhost".to_owned()))
        .unwrap();
    let mut config = ServerConfig::builder_with_provider(ca.provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![leaf.cert.clone()], leaf.key.clone_key())
        .unwrap();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config
}

fn router(hits: Arc<AtomicUsize>, routes: Routes) -> Router {
    let shared = Router::new()
        .route("/sse", route_get(sse))
        .route("/chunked", route_get(chunked))
        .route("/big", route_get(big))
        .route("/sink", route_post(sink))
        .route("/ws", route_get(websocket))
        .route("/redirect", route_get(redirect))
        .route("/slow", route_get(slow))
        .route("/status/{code}", route_get(status))
        .route("/repo.git/info/refs", route_get(git_info_refs));
    let menu = match routes {
        Routes::Legacy => Router::new()
            .route("/echo", route_get(echo_get).post(echo_post))
            .route("/stream", route_get(stream)),
        Routes::Matrix => Router::new().route("/echo", route_get(json_echo).post(json_echo)),
    };
    shared.merge(menu).with_state(hits)
}

// ---------------------------------------------------------------------------
// Karte Legacy (HUM-015)
// ---------------------------------------------------------------------------

async fn echo_get(State(hits): State<Arc<AtomicUsize>>) -> impl IntoResponse {
    hits.fetch_add(1, Ordering::SeqCst);
    ([("x-upstream", "fake")], ECHO_BODY)
}

async fn echo_post(State(hits): State<Arc<AtomicUsize>>, body: Bytes) -> impl IntoResponse {
    hits.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::OK,
        [("x-echo-len", body.len().to_string())],
        body,
    )
}

async fn stream(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(4);
    tokio::spawn(async move {
        let _ = tx.send(Ok(Bytes::from_static(b"first\n"))).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let _ = tx.send(Ok(Bytes::from_static(b"second\n"))).await;
    });
    Body::from_stream(ReceiverStream::new(rx)).into_response()
}

// ---------------------------------------------------------------------------
// Karte Matrix (HUM-017)
// ---------------------------------------------------------------------------

/// `GET /echo` und `POST /echo`: JSON mit Methode, Pfad, Headern, Body-Länge
/// und Body-sha256.
///
/// Das JSON entsteht von Hand, weil `serde_json` keine Abhängigkeit dieser
/// Crate ist und eine Testkarte keine neue verdient.
async fn json_echo(
    State(hits): State<Arc<AtomicUsize>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let digest: [u8; 32] = Sha256::digest(&body).into();
    let mut json = String::from("{\"method\":\"");
    json.push_str(&escape(method.as_str()));
    json.push_str("\",\"path\":\"");
    json.push_str(&escape(uri.path()));
    json.push_str("\",\"query\":\"");
    json.push_str(&escape(uri.query().unwrap_or_default()));
    json.push_str("\",\"headers\":{");
    let mut first = true;
    for (name, value) in &headers {
        if !first {
            json.push(',');
        }
        first = false;
        json.push('"');
        json.push_str(&escape(name.as_str()));
        json.push_str("\":\"");
        json.push_str(&escape(&String::from_utf8_lossy(value.as_bytes())));
        json.push('"');
    }
    json.push_str("},\"body_len\":");
    json.push_str(&body.len().to_string());
    json.push_str(",\"body_sha256\":\"");
    json.push_str(&hex(&digest));
    json.push_str("\"}\n");
    (StatusCode::OK, [(CONTENT_TYPE, "application/json")], json).into_response()
}

/// `GET /sse`: `text/event-stream`, fünf Ereignisse im Abstand `?interval_ms=`
/// (Vorgabe 200 ms), das erste sofort, dann Ende.
async fn sse(State(hits): State<Arc<AtomicUsize>>, uri: Uri) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let interval = Duration::from_millis(
        param(&uri, "interval_ms")
            .and_then(|text| text.parse().ok())
            .unwrap_or(SSE_DEFAULT_INTERVAL_MS),
    );
    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(SSE_EVENTS);
    tokio::spawn(async move {
        for index in 0..SSE_EVENTS {
            if index > 0 {
                tokio::time::sleep(interval).await;
            }
            let event = format!("event: tick\ndata: {index}\n\n");
            if tx.send(Ok(Bytes::from(event))).await.is_err() {
                break;
            }
        }
    });
    Response::builder()
        .header(CONTENT_TYPE, "text/event-stream")
        .header(CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
}

/// `GET /chunked`: zehn Stücke à 64 KiB mit 50 ms Pause, ohne
/// `Content-Length`, also `Transfer-Encoding: chunked`.
async fn chunked(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(2);
    tokio::spawn(async move {
        for index in 0..CHUNK_COUNT {
            if index > 0 {
                tokio::time::sleep(CHUNK_PAUSE).await;
            }
            let chunk = Bytes::from(vec![chunk_byte(index); CHUNK_SIZE]);
            if tx.send(Ok(chunk)).await.is_err() {
                break;
            }
        }
    });
    Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
}

/// `GET /big?mb=N`: N MiB Nullen mit gesetztem `Content-Length`.
async fn big(State(hits): State<Arc<AtomicUsize>>, uri: Uri) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let mb = param(&uri, "mb")
        .and_then(|text| text.parse::<usize>().ok())
        .unwrap_or(1)
        .min(BIG_MAX_MB);
    let bytes = Bytes::from(vec![0u8; mb * 1024 * 1024]);
    (
        StatusCode::OK,
        [(CONTENT_TYPE, "application/octet-stream")],
        bytes,
    )
        .into_response()
}

/// `POST /sink`: liest den Body vollständig und antwortet mit seiner Länge.
async fn sink(State(hits): State<Arc<AtomicUsize>>, body: Bytes) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    (StatusCode::OK, format!("{}\n", body.len())).into_response()
}

/// `GET /ws`: in M1 kein WebSocket-Echo.
///
/// axum bietet `WebSocketUpgrade` nur mit dem Cargo-Feature `ws`, das in den
/// `dev-dependencies` dieser Crate nicht gesetzt ist; ein eigener Handschlag
/// bräuchte SHA-1, und eigene Kryptographie ist ausgeschlossen. Der Endpunkt
/// antwortet deshalb mit `426 Upgrade Required`. Das genügt für Zeile 11 der
/// Matrix, weil der Proxy in M1 ohnehin keinen Protokollwechsel durchreicht:
/// `Connection` und `Upgrade` sind Kopfzeilen von Verbindungsrang und werden
/// vor der Weiterleitung entfernt.
async fn websocket(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::UPGRADE_REQUIRED,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        "websocket echo needs the axum feature `ws` (HUM-017)\n",
    )
        .into_response()
}

/// `GET /redirect?to=`: `302` auf `to`.
async fn redirect(State(hits): State<Arc<AtomicUsize>>, uri: Uri) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let target = param(&uri, "to").unwrap_or_else(|| "/echo".to_owned());
    Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, target)
        .body(Body::empty())
        .unwrap()
}

/// `GET /slow?ms=`: wartet, dann `200`.
async fn slow(State(hits): State<Arc<AtomicUsize>>, uri: Uri) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let ms = param(&uri, "ms")
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(0);
    tokio::time::sleep(Duration::from_millis(ms)).await;
    (StatusCode::OK, "slow\n").into_response()
}

/// `GET /status/{code}`: beliebiger Status.
async fn status(State(hits): State<Arc<AtomicUsize>>, Path(code): Path<u16>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    // Ein Body ist bei 1xx, 204 und 304 verboten; hyper würde ihn verwerfen.
    if status.is_informational()
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED
    {
        return Response::builder()
            .status(status)
            .body(Body::empty())
            .unwrap();
    }
    (status, format!("status {code}\n")).into_response()
}

/// `GET /repo.git/info/refs`: die Referenz-Ankündigung des Smart-HTTP-Protokolls
/// für ein leeres Repository. Zeile 16 der Matrix prüft nur, dass die Antwort
/// ankommt und TLS hält, nicht ihren Inhalt.
async fn git_info_refs(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let body = "001e# service=git-upload-pack\n00000000";
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "application/x-git-upload-pack-advertisement"),
            (CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Kleinkram
// ---------------------------------------------------------------------------

/// Der Wert eines Query-Parameters, ohne Prozent-Dekodierung: die Tests
/// setzen nur Werte, die keine braucht.
fn param(uri: &Uri, key: &str) -> Option<String> {
    uri.query()?.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| value.to_owned())
    })
}

/// Maskiert einen Text für ein JSON-Literal.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Hex-Darstellung eines Digests, kleingeschrieben.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Der sha256 eines Puffers als Hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    hex(&digest)
}

/// Ein `TcpListener`, der jede Verbindung mit TLS terminiert, für
/// `axum::serve`.
struct TlsListener {
    inner: TcpListener,
    acceptor: TlsAcceptor,
    alpn: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let Ok((stream, addr)) = self.inner.accept().await else {
                tokio::time::sleep(Duration::from_millis(20)).await;
                continue;
            };
            match self.acceptor.accept(stream).await {
                Ok(tls) => {
                    *self.alpn.lock().unwrap() =
                        tls.get_ref().1.alpn_protocol().map(<[u8]>::to_vec);
                    return (tls, addr);
                }
                Err(err) => eprintln!("fake upstream: tls handshake failed: {err}"),
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}
