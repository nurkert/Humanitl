//! Gemeinsame Helfer der Proxy-Integrationstests (HUM-015, HUM-016, HUM-017).
//!
//! - [`upstream`]: der Fake-Upstream auf axum, Klartext oder TLS.
//! - [`clients`]: echte Clients (`curl`, `git`, …), die fehlen dürfen.
//! - hier: ein zählender Resolver, ein zählender Egress, ein Proxy-Harness,
//!   das den Kern auf einem Unix-Socket in einem Temp-Verzeichnis startet und
//!   sowohl hyper-Clients im Prozess als auch eine TCP-Brücke für externe
//!   Clients liefert.

#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::too_many_lines,
    clippy::unused_async,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

pub mod clients;
pub mod upstream;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use humanitl_config::{HoldConfig, Limits, RecorderConfig, ResolverConfig};
use humanitl_core::{Authority, Decision, Diagnostic, FlowEvent, HttpRequest, SessionId};
use humanitl_proxy::ca::{CaStore, LeafCache};
use humanitl_proxy::hold::next_event;
use humanitl_proxy::{
    AskPipeline, AsyncStream, ClientTls, ConnectionContext, Direct, Egress, FlowHandler,
    FlowPipeline, HoldQueue, PassthroughPipeline, ProxyCore, ProxyLimits, ResolveError, Resolver,
    ResolverPort, RulesPipeline, Scanner, Upstream,
};
use hyper::body::Incoming;
use hyper::client::conn::http1::SendRequest;
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tempfile::TempDir;
use tokio::net::{TcpListener, UnixStream};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_rustls::TlsConnector;

pub use self::upstream::{ECHO_BODY, FakeUpstream, UpstreamCa};

/// So lange wartet ein Test höchstens auf ein Ereignis oder eine Antwort.
pub const WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Zählende Ports
// ---------------------------------------------------------------------------

/// Der Resolver der Tests: schreibt jeden Namen auf, den er gefragt wird.
///
/// Er ist der Zeuge für ADR-006. Weil er jeden Aufruf mit Namen mitschreibt,
/// beweist ein leeres [`MockResolver::hosts`] nach einer geblockten,
/// abgelaufenen oder abgelehnten Anfrage, dass nichts gefragt wurde — und ein
/// Eintrag verrät, welcher Name geleakt wäre.
pub struct MockResolver {
    answer: IpAddr,
    answers: std::collections::HashMap<String, Vec<IpAddr>>,
    failing: std::collections::HashSet<String>,
    calls: std::sync::Mutex<Vec<String>>,
}

impl MockResolver {
    /// Antwortet auf jeden Namen mit derselben Adresse.
    pub fn answering(answer: IpAddr) -> Self {
        Self {
            answer,
            answers: std::collections::HashMap::new(),
            failing: std::collections::HashSet::new(),
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Eine eigene Antwort für genau diesen Namen.
    pub fn with_answer(mut self, host: &str, addrs: Vec<IpAddr>) -> Self {
        self.answers.insert(host.to_owned(), addrs);
        self
    }

    /// Dieser Name scheitert.
    pub fn failing_for(mut self, host: &str) -> Self {
        self.failing.insert(host.to_owned());
        self
    }

    /// Wie oft überhaupt aufgelöst wurde.
    pub fn calls(&self) -> usize {
        self.hosts().len()
    }

    /// Welche Namen gefragt wurden, in Reihenfolge.
    pub fn hosts(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl Resolver for MockResolver {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
        self.calls.lock().unwrap().push(host.to_owned());
        if self.failing.contains(host) {
            return Err(ResolveError::NotFound {
                host: host.to_owned(),
            });
        }
        Ok(self
            .answers
            .get(host)
            .cloned()
            .unwrap_or_else(|| vec![self.answer]))
    }
}

/// Der direkte Egress, mit Zähler davor.
#[derive(Default)]
pub struct CountingEgress {
    inner: Direct,
    connects: AtomicUsize,
}

impl CountingEgress {
    pub fn connects(&self) -> usize {
        self.connects.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Egress for CountingEgress {
    async fn connect(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        self.inner.connect(authority, resolved).await
    }
}

// ---------------------------------------------------------------------------
// Proxy-Harness
// ---------------------------------------------------------------------------

/// Welche Pipeline der Proxy im Test fährt.
#[derive(Debug, Clone, Copy)]
pub enum Pipe {
    /// Halten, bis jemand entscheidet oder die Frist abläuft.
    Ask(Duration),
    /// Sofort erlauben.
    Passthrough,
}

/// Baut einen Proxy mit Test-CA, zählendem Resolver und Egress.
pub struct ProxyBuilder {
    limits: Limits,
    pipe: Pipe,
    allow_private: bool,
    resolve_to: IpAddr,
    resolver_answers: Vec<(String, Vec<IpAddr>)>,
    resolver_failing: Vec<String>,
    resolver_config: ResolverConfig,
    extra_roots: Vec<CertificateDer<'static>>,
    rules: Option<String>,
    scanner: Option<Arc<dyn Scanner>>,
    hard_block_checksum_secrets: bool,
}

impl Default for ProxyBuilder {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            pipe: Pipe::Ask(Duration::from_secs(30)),
            allow_private: true,
            resolve_to: IpAddr::V4(Ipv4Addr::LOCALHOST),
            resolver_answers: Vec::new(),
            resolver_failing: Vec::new(),
            // Ohne Frist ist der Zwischenspeicher aus; ein Test, der ihn
            // braucht, schaltet ihn ausdrücklich ein.
            resolver_config: ResolverConfig {
                cache_ttl_secs: 0,
                ..ResolverConfig::default()
            },
            extra_roots: Vec::new(),
            rules: None,
            scanner: None,
            hard_block_checksum_secrets: false,
        }
    }
}

impl ProxyBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ask(mut self, timeout: Duration) -> Self {
        self.pipe = Pipe::Ask(timeout);
        self
    }

    pub fn passthrough(mut self) -> Self {
        self.pipe = Pipe::Passthrough;
        self
    }

    pub fn body_cap(mut self, bytes: u64) -> Self {
        self.limits.hold_body_cap_bytes = bytes;
        self
    }

    pub fn hold_max_flows(mut self, flows: u32) -> Self {
        self.limits.hold_max_flows = flows;
        self
    }

    pub fn allow_private(mut self, allow: bool) -> Self {
        self.allow_private = allow;
        self
    }

    pub fn resolve_to(mut self, ip: IpAddr) -> Self {
        self.resolve_to = ip;
        self
    }

    /// Eine eigene Antwort des Mock-Resolvers für genau diesen Namen.
    pub fn resolve_host(mut self, host: &str, addrs: Vec<IpAddr>) -> Self {
        self.resolver_answers.push((host.to_owned(), addrs));
        self
    }

    /// Dieser Name scheitert beim Auflösen.
    pub fn resolve_fails(mut self, host: &str) -> Self {
        self.resolver_failing.push(host.to_owned());
        self
    }

    /// Die `resolver.*`-Einstellungen, mit denen der Stapel gebaut wird.
    pub fn resolver_config(mut self, config: ResolverConfig) -> Self {
        self.resolver_config = config;
        self
    }

    /// Eine weitere Wurzel für den Upstream-Connector des Proxys, etwa die
    /// eigene CA des Fake-Upstreams ([`UpstreamCa`]).
    pub fn trust(mut self, cert: CertificateDer<'static>) -> Self {
        self.extra_roots.push(cert);
        self
    }

    /// Ein Regelsatz als YAML vor der gewählten Pipeline (HUM-023): Was eine
    /// Regel entscheidet, wird nicht gehalten.
    pub fn rules(mut self, yaml: &str) -> Self {
        self.rules = Some(yaml.to_owned());
        self
    }

    /// Die Detektoren, die im Pfad laufen (HUM-025). Ohne das läuft `NoScan`.
    pub fn scanner(mut self, scanner: Arc<dyn Scanner>) -> Self {
        self.scanner = Some(scanner);
        self
    }

    /// `hold.hard_block_checksum_secrets`.
    pub fn hard_block_checksum_secrets(mut self, on: bool) -> Self {
        self.hard_block_checksum_secrets = on;
        self
    }

    pub async fn start(self) -> Proxy {
        let tmp = tempfile::tempdir().unwrap();
        let ca = Arc::new(CaStore::load_or_create(&tmp.path().join("ca")).unwrap());
        let leaves = Arc::new(LeafCache::new(Arc::clone(&ca), 16));
        let queue = Arc::new(HoldQueue::new(&self.limits));
        let session = SessionId::new();
        let inner: Arc<dyn FlowPipeline> = match self.pipe {
            Pipe::Ask(timeout) => Arc::new(AskPipeline::new(Arc::clone(&queue), timeout)),
            Pipe::Passthrough => Arc::new(PassthroughPipeline::new(Arc::clone(&queue))),
        };
        let pipeline: Arc<dyn FlowPipeline> = match &self.rules {
            None => inner,
            Some(yaml) => {
                // `parse_rules_for_session` liefert `Err`, sobald ein Befund
                // ein Fehler ist; was hier ankommt, sind Warnungen.
                let (set, _warnings) =
                    humanitl_rules::parse_rules_for_session(yaml, session).unwrap();
                Arc::new(RulesPipeline::new(
                    Arc::clone(&queue),
                    Arc::new(std::sync::RwLock::new(set)),
                    inner,
                ))
            }
        };
        let mut mock = MockResolver::answering(self.resolve_to);
        for (host, addrs) in &self.resolver_answers {
            mock = mock.with_answer(host, addrs.clone());
        }
        for host in &self.resolver_failing {
            mock = mock.failing_for(host);
        }
        let resolver = Arc::new(mock);
        // Derselbe Stapel wie im Daemon (feste Zuordnungen, Zwischenspeicher,
        // Zähler), nur mit dem Mock statt des Namensdienstes ganz unten.
        let port = Arc::new(
            ResolverPort::over(
                Arc::clone(&resolver) as Arc<dyn Resolver>,
                &self.resolver_config,
            )
            .unwrap(),
        );
        let egress = Arc::new(CountingEgress::default());
        let mut roots = vec![ca.cert_der().clone()];
        roots.extend(self.extra_roots);
        let tls = ClientTls::new(&roots, false).unwrap();
        let upstream = Upstream::new(
            Arc::clone(&egress) as Arc<dyn Egress>,
            Arc::clone(&port) as Arc<dyn Resolver>,
            tls,
            self.resolver_config.prefer,
            Duration::from_secs(self.limits.header_timeout_secs),
        );
        let hold = HoldConfig {
            hard_block_checksum_secrets: self.hard_block_checksum_secrets,
            ..HoldConfig::default()
        };
        let limits =
            ProxyLimits::from_config(&self.limits, &RecorderConfig::default()).with_hold(&hold);
        let scanner = self
            .scanner
            .clone()
            .unwrap_or_else(|| Arc::new(humanitl_proxy::NoScan) as Arc<dyn Scanner>);
        let handler = FlowHandler::with_findings(
            Arc::clone(&queue),
            pipeline,
            upstream,
            leaves,
            limits,
            scanner,
        );

        let socket = tmp.path().join("proxy").join("proxy.sock");
        let core = ProxyCore::new();
        let meta = ConnectionContext {
            allow_private: self.allow_private,
            ..ConnectionContext::plain(session)
        };
        core.start_session(session, &socket, handler, meta).unwrap();

        Proxy {
            socket,
            session,
            queue,
            ca,
            resolver,
            port,
            egress,
            core,
            tmp,
        }
    }
}

/// Ein laufender Proxy samt allen Griffen, die ein Test braucht.
pub struct Proxy {
    pub socket: PathBuf,
    pub session: SessionId,
    pub queue: Arc<HoldQueue>,
    pub ca: Arc<CaStore>,
    /// Der Mock ganz unten im Stapel: Was er sah, hat den Rechner verlassen.
    pub resolver: Arc<MockResolver>,
    /// Der verdrahtete Stapel mit den Zählern, die der Daemon meldet.
    pub port: Arc<ResolverPort>,
    pub egress: Arc<CountingEgress>,
    pub core: ProxyCore,
    tmp: TempDir,
}

impl Proxy {
    /// Ein Zuhörer am Ereignisstrom; vor der Anfrage anlegen.
    pub fn events(&self) -> Events {
        Events {
            rx: self.queue.subscribe(),
            seen: Vec::new(),
        }
    }

    /// Entscheidet jeden gehaltenen Flow automatisch mit `decision`, sobald
    /// sein `Held`-Ereignis erscheint. Vor der Anfrage anlegen.
    pub fn decide_with(&self, decision: Decision) -> Decider {
        self.decide_each(move |_index| decision.clone())
    }

    /// Wie [`Proxy::decide_with`], aber die Entscheidung hängt davon ab, der
    /// wievielte gehaltene Flow es ist (0 für den ersten).
    ///
    /// Damit prüft Zeile 22 der Matrix eine Keep-Alive-Verbindung, deren erste
    /// Anfrage geblockt wird, und Zeile 5, was der Upstream im Augenblick der
    /// Entscheidung gesehen hat.
    pub fn decide_each<F>(&self, decide: F) -> Decider
    where
        F: Fn(usize) -> Decision + Send + 'static,
    {
        let mut rx = self.queue.subscribe();
        let queue = Arc::clone(&self.queue);
        let task = tokio::spawn(async move {
            let mut index = 0usize;
            while let Some(event) = next_event(&mut rx).await {
                if let FlowEvent::Held { flow_id, .. } = event {
                    let _ = queue.decide(flow_id, decide(index));
                    index += 1;
                }
            }
        });
        Decider { task }
    }

    /// Eine Klartext-Verbindung zum Proxy-Socket.
    pub async fn client(&self) -> Client {
        let stream = UnixStream::connect(&self.socket).await.unwrap();
        let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
        let conn = tokio::spawn(async move {
            let _ = conn.with_upgrades().await;
        });
        Client {
            sender,
            _conn: conn,
        }
    }

    /// `CONNECT host:port`, dann TLS gegen den Proxy (der Client vertraut nur
    /// der Test-CA und bietet `h2` und `http/1.1`), dann HTTP/1.1 im Tunnel.
    pub async fn tls_client(&self, host: &str, port: u16) -> TlsClient {
        let stream = UnixStream::connect(&self.socket).await.unwrap();
        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
        let outer = tokio::spawn(async move {
            let _ = conn.with_upgrades().await;
        });
        let connect = Request::builder()
            .method(Method::CONNECT)
            .uri(format!("{host}:{port}"))
            .body(Full::new(Bytes::new()))
            .unwrap();
        let response = sender.send_request(connect).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "CONNECT must be accepted"
        );
        let upgraded = hyper::upgrade::on(response).await.unwrap();

        let connector = TlsConnector::from(self.client_tls_config());
        let name = ServerName::try_from(host.to_owned()).unwrap();
        let tls = connector
            .connect(name, TokioIo::new(upgraded))
            .await
            .expect("the proxy's leaf must verify against the test CA");
        let alpn = tls.get_ref().1.alpn_protocol().map(<[u8]>::to_vec);
        let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .unwrap();
        let inner = tokio::spawn(async move {
            let _ = conn.await;
        });
        TlsClient {
            client: Client {
                sender,
                _conn: inner,
            },
            alpn,
            _outer: outer,
        }
    }

    /// `CONNECT connect_host:port`, dann TLS mit einer frei gewählten SNI.
    ///
    /// Der Client prüft das Zertifikat des Proxys nicht. Genau so verhält sich
    /// ein Client, der Fronting versucht: Das Leaf gilt für das CONNECT-Ziel,
    /// und ein prüfender Client käme mit einer abweichenden SNI gar nicht bis
    /// zur ersten Anfrage. Die Prüfung des Proxys darf sich darauf nicht
    /// verlassen.
    ///
    /// `sni = None` schaltet die SNI ganz ab.
    pub async fn tls_tunnel(&self, connect_host: &str, port: u16, sni: Option<&str>) -> TlsClient {
        let stream = UnixStream::connect(&self.socket).await.unwrap();
        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
        let outer = tokio::spawn(async move {
            let _ = conn.with_upgrades().await;
        });
        let connect = Request::builder()
            .method(Method::CONNECT)
            .uri(format!("{connect_host}:{port}"))
            .body(Full::new(Bytes::new()))
            .unwrap();
        let response = sender.send_request(connect).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "CONNECT must be accepted"
        );
        let upgraded = hyper::upgrade::on(response).await.unwrap();

        let mut config = ClientConfig::builder_with_provider(self.ca.provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(TrustAnything(self.ca.provider())))
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        config.enable_sni = sni.is_some();
        let name = ServerName::try_from(sni.unwrap_or(connect_host).to_owned()).unwrap();
        let tls = TlsConnector::from(Arc::new(config))
            .connect(name, TokioIo::new(upgraded))
            .await
            .expect("the proxy must finish the handshake even for a forged SNI");
        let alpn = tls.get_ref().1.alpn_protocol().map(<[u8]>::to_vec);
        let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .unwrap();
        let inner = tokio::spawn(async move {
            let _ = conn.await;
        });
        TlsClient {
            client: Client {
                sender,
                _conn: inner,
            },
            alpn,
            _outer: outer,
        }
    }

    /// Eine rustls-Client-Konfiguration, die nur der Test-CA vertraut.
    pub fn client_tls_config(&self) -> Arc<ClientConfig> {
        let mut roots = RootCertStore::empty();
        roots.add(self.ca.cert_der().clone()).unwrap();
        let mut config = ClientConfig::builder_with_provider(self.ca.provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Arc::new(config)
    }

    /// Eine TCP-Brücke auf den Session-Socket, für externe Clients.
    ///
    /// Der Proxy lauscht auf einem Unix-Socket und tut das mit Absicht
    /// (Garantie 2, kein Loopback-Port auf dem Host). `curl` und Verwandte
    /// können einen Unix-Socket aber nicht als Proxy ansprechen. Die Brücke
    /// ist deshalb genau das, was der Shim in der Sandbox tut: sie nimmt
    /// TCP auf Loopback an und reicht jede Verbindung an den Unix-Socket
    /// weiter, ohne ein Byte anzufassen.
    pub async fn bridge(&self) -> Bridge {
        Bridge::to(&self.socket).await
    }

    /// Schreibt das CA-Zertifikat als PEM in das Temp-Verzeichnis des Proxys
    /// und liefert den Pfad, für `curl --cacert` und das Env-Kit.
    pub fn ca_pem(&self) -> PathBuf {
        let path = self.tmp.path().join("humanitl-ca.pem");
        std::fs::write(&path, self.ca.cert_pem()).unwrap();
        path
    }
}

/// Eine TCP-Brücke auf einen Unix-Socket; endet beim Fallenlassen.
pub struct Bridge {
    /// Die Adresse, auf der die Brücke lauscht.
    pub addr: SocketAddr,
    task: JoinHandle<()>,
}

impl Bridge {
    /// Bindet `127.0.0.1:0` und reicht jede Verbindung an `socket` weiter.
    pub async fn to(socket: &Path) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let socket = socket.to_owned();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut tcp, _peer)) = listener.accept().await else {
                    break;
                };
                let socket = socket.clone();
                tokio::spawn(async move {
                    let Ok(mut unix) = UnixStream::connect(&socket).await else {
                        return;
                    };
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut unix).await;
                });
            }
        });
        Self { addr, task }
    }

    /// Die Proxy-URL, die ein Client bekommt.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Ein automatischer Entscheider; endet beim Fallenlassen.
pub struct Decider {
    task: JoinHandle<()>,
}

impl Drop for Decider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Eine HTTP/1.1-Verbindung zum Proxy.
pub struct Client {
    sender: SendRequest<Full<Bytes>>,
    _conn: JoinHandle<()>,
}

impl Client {
    /// Sendet eine Anfrage und liefert die Antwort.
    ///
    /// Vor dem Senden wartet [`SendRequest::ready`], bis die Verbindung eine
    /// weitere Anfrage annimmt. Das ist bei hypers Verbindungs-API Pflicht und
    /// nicht bloß Vorsicht: `SendRequest` darf genau eine Anfrage puffern,
    /// bevor die Verbindungs-Task das erste Mal signalisiert, dass sie eine
    /// will (`hyper::client::dispatch::Sender::can_send`). Jede weitere
    /// Anfrage lehnt `send_request` ohne dieses Signal sofort mit
    /// `Canceled("connection was not ready")` ab, ohne ein Byte zu senden. Auf
    /// einer Keep-Alive-Verbindung fehlt das Signal genau so lange, bis die
    /// Verbindungs-Task nach der vorigen Antwort wieder an der Reihe war; das
    /// ist ein Rennen zwischen zwei Tasks des Tests und sagt nichts über den
    /// Proxy aus.
    ///
    /// Ein echter Abbruch bleibt sichtbar: hat die Gegenseite die Verbindung
    /// geschlossen, liefert `ready` einen Fehler, und die Frist [`WAIT`]
    /// umschließt weiterhin Warten und Senden zusammen.
    pub async fn send(&mut self, request: Request<Full<Bytes>>) -> hyper::Response<Incoming> {
        tokio::time::timeout(WAIT, async {
            self.sender.ready().await?;
            self.sender.send_request(request).await
        })
        .await
        .expect("the proxy answers in time")
        .expect("the proxy answers")
    }
}

/// Eine Verbindung im TLS-Tunnel nach `CONNECT`.
pub struct TlsClient {
    pub client: Client,
    /// Das ALPN, das der Proxy dem Client gegenüber ausgehandelt hat.
    pub alpn: Option<Vec<u8>>,
    _outer: JoinHandle<()>,
}

/// Ein Zertifikatsprüfer, der alles annimmt: der feindselige Client.
#[derive(Debug)]
struct TrustAnything(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for TrustAnything {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Der Ereignisstrom aus Sicht eines Tests.
pub struct Events {
    rx: broadcast::Receiver<FlowEvent>,
    pub seen: Vec<FlowEvent>,
}

impl Events {
    /// Liest, bis ein Ereignis mit diesem Namen kommt, und liefert es.
    pub async fn wait_for(&mut self, name: &str) -> FlowEvent {
        tokio::time::timeout(WAIT, async {
            loop {
                let event = next_event(&mut self.rx)
                    .await
                    .expect("the event stream stays open");
                self.seen.push(event.clone());
                if event.name() == name {
                    return event;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no `{name}` event within {WAIT:?}; seen {:?}", self.names()))
    }

    /// Liest, bis ein Ereignis mit diesem Namen zum `count`-ten Mal kam.
    pub async fn wait_for_nth(&mut self, name: &str, count: usize) -> FlowEvent {
        let mut event = self.wait_for(name).await;
        while self.count(name) < count {
            event = self.wait_for(name).await;
        }
        event
    }

    /// Liest alles, was ohne Warten schon da ist.
    pub fn drain(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            self.seen.push(event);
        }
    }

    /// Die Namen aller bisher gesehenen Ereignisse, in Reihenfolge.
    pub fn names(&self) -> Vec<&'static str> {
        self.seen.iter().map(FlowEvent::name).collect()
    }

    /// Wie oft ein Ereignis mit diesem Namen kam.
    pub fn count(&self, name: &str) -> usize {
        self.seen
            .iter()
            .filter(|event| event.name() == name)
            .count()
    }

    /// Die Anfrage aus dem ersten `Received`-Ereignis.
    pub fn received_request(&self) -> &HttpRequest {
        self.received_requests()
            .into_iter()
            .next()
            .expect("a Received event")
    }

    /// Alle Anfragen aus den `Received`-Ereignissen, in Reihenfolge.
    pub fn received_requests(&self) -> Vec<&HttpRequest> {
        self.seen
            .iter()
            .filter_map(|event| match event {
                FlowEvent::Received { request, .. } => Some(request.as_ref()),
                _ => None,
            })
            .collect()
    }

    /// Alle Statuscodes aus den `ResponseHeaders`-Ereignissen, in Reihenfolge.
    pub fn statuses(&self) -> Vec<u16> {
        self.seen
            .iter()
            .filter_map(|event| match event {
                FlowEvent::ResponseHeaders { status, .. } => Some(*status),
                _ => None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Anfragen und Antworten
// ---------------------------------------------------------------------------

/// `GET url` in Absolut-Form mit passendem `Host`.
pub fn get(url: &str) -> Request<Full<Bytes>> {
    request(Method::GET, url, Bytes::new())
}

/// `POST url` mit `body`.
pub fn post(url: &str, body: impl Into<Bytes>) -> Request<Full<Bytes>> {
    request(Method::POST, url, body.into())
}

fn request(method: Method, url: &str, body: Bytes) -> Request<Full<Bytes>> {
    let uri: hyper::Uri = url.parse().unwrap();
    let mut builder = Request::builder().method(method).uri(uri.clone());
    if let Some(authority) = uri.authority() {
        builder = builder.header("host", authority.as_str());
    }
    builder.body(Full::new(body)).unwrap()
}

/// Den ganzen Body lesen.
pub async fn body_bytes(body: Incoming) -> Bytes {
    tokio::time::timeout(WAIT, body.collect())
        .await
        .expect("the body ends in time")
        .expect("the body is readable")
        .to_bytes()
}

/// Den ganzen Body als Text lesen.
pub async fn body_string(body: Incoming) -> String {
    String::from_utf8(body_bytes(body).await.to_vec()).unwrap()
}

/// Der Wert eines Headers als Text.
pub fn header<'a>(response: &'a hyper::Response<Incoming>, name: &str) -> Option<&'a str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}
