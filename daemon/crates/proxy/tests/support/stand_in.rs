//! Der Stellvertreter-Proxy der Tests zum Entscheidungs-Proxy (ADR-0022,
//! HUM-511, HUM-512).
//!
//! Er nimmt `CONNECT` und Anfragen in absoluter Form an, schreibt mit, was
//! ankommt (Anfragezeile, `Host`, `Proxy-Authorization`, SNI), und
//! beantwortet die Anfragen selbst, durch den Tunnel mit einem Blatt aus
//! seiner eigenen CA, so wie mitmproxy es tut. Er steht hier und nicht in
//! einer Testdatei, weil der Egress (`decision_proxy.rs`), der Frage-Modus
//! `upstream` (`upstream_ask_mode.rs`) und der Ende-zu-Ende-Test
//! (`decision_proxy_end_to_end.rs`) ihn brauchen.
//!
//! Mit [`StandIn::start_intercepting`] (HUM-513) benimmt er sich wie
//! mitmproxy im regulären Modus: Das Blatt im Tunnel stellt er auf den Namen
//! aus SNI aus (ohne SNI auf das Ziel der `CONNECT`-Zeile), er entscheidet
//! jede Anfrage im Tunnel wie ein Mensch mit einem Abfangfilter (weiter,
//! `403` wie `block_list`, Verbindung kappen wie „kill"), und eine
//! weitergelassene Anfrage schickt er an das Ziel der `CONNECT`-Zeile, mit
//! TLS gegen den Namen aus SNI und einer CA, der er vertraut (bei mitmproxy
//! `ssl_verify_upstream_trusted_ca`).
//!
//! Er bildet mitmproxy mit `connection_strategy=lazy` und
//! `upstream_cert=false` nach, so wie `docs/guides/decision-proxy-mitmproxy.md`
//! ihn startet: Zum Ziel verbindet er erst, wenn die Entscheidung „weiter"
//! lautet. Mit den Vorgaben von mitmproxy (`eager`, `upstream_cert=true`)
//! entstünde die Verbindung zum Ziel schon während des TLS-Handschlags, vor
//! jeder Entscheidung; das bildet der Stellvertreter nicht ab.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use humanitl_config::{Config, ProxyResolve};
use humanitl_proxy::roots_from_pem;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_rustls::TlsConnector;

use super::ForeignCa;

/// Der Name des Ziels; das Blatt des Stellvertreters ist darauf ausgestellt.
pub const HOST: &str = "upstream.test";

/// Eine öffentliche Adresse, die der [`super::MockResolver`] für [`HOST`] nennt.
pub const PUBLIC: IpAddr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));

/// Was der Stellvertreter auf `CONNECT` antwortet.
#[derive(Debug, Clone, Copy)]
pub enum ConnectAnswer {
    /// `200`, dann TLS durch den Tunnel.
    Open,
    /// Dieser Status, dann Ende.
    Status(u16),
    /// Nichts.
    Silent,
}

/// Wie sich der Stellvertreter verhält.
#[derive(Debug, Clone, Copy)]
pub struct Behaviour {
    pub connect: ConnectAnswer,
    /// Der Status einer Anfrage in absoluter Form.
    pub plain_status: u16,
    /// Der Status der Anfrage im Tunnel; `403` ist eine Ablehnung dort
    /// (HUM-512).
    pub tunnel_status: u16,
    /// So lange wartet er vor dem Kopf jeder Antwort, wie ein Mensch, der
    /// dort entscheidet.
    pub head_delay: Duration,
}

impl Default for Behaviour {
    fn default() -> Self {
        Self {
            connect: ConnectAnswer::Open,
            plain_status: 200,
            tunnel_status: 200,
            head_delay: Duration::ZERO,
        }
    }
}

/// Was beim Stellvertreter ankam.
#[derive(Debug, Default, Clone)]
pub struct Seen {
    /// Die erste Zeile jeder Anfrage an den Proxy selbst.
    pub lines: Vec<String>,
    /// `Host` dieser Anfragen.
    pub hosts: Vec<String>,
    /// `Proxy-Authorization` dieser Anfragen, leer ohne.
    pub auth: Vec<String>,
    /// SNI im Tunnel.
    pub sni: Vec<String>,
    /// Die erste Zeile der Anfrage im Tunnel.
    pub inner_lines: Vec<String>,
    /// `Host` der Anfrage im Tunnel.
    pub inner_hosts: Vec<String>,
    /// `Proxy-Authorization` der Anfrage im Tunnel, leer ohne.
    pub inner_auth: Vec<String>,
    /// Wie viele Bytes Rumpf jede Anfrage in absoluter Form trug.
    pub body_bytes: Vec<usize>,
    /// Wohin eine weitergelassene Anfrage ging, als Ziel der
    /// `CONNECT`-Zeile ([`StandIn::start_intercepting`]).
    pub dialed: Vec<String>,
}

/// Was der Mensch am abfangenden Stellvertreter mit einer Anfrage tut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Weiter zum Ziel, wie „resume" in mitmproxy.
    Resume,
    /// Eine eigene Antwort mit diesem Status, wie `block_list` in mitmproxy.
    Refuse(u16),
    /// Verbindung schließen ohne Antwort, wie „kill" oder `block_list` mit
    /// `444`.
    Kill,
    /// Die Anfrage so lange halten, wie ein Mensch überlegt, dann die
    /// Verbindung ohne Antwort schließen: „Drop" in Burp Suite mit „Suppress
    /// Burp error messages in browser" (HUM-555).
    HoldThenKill(Duration),
}

/// Wie der abfangende Stellvertreter entscheidet und wohin er weiterleitet.
#[derive(Clone)]
pub struct Intercept {
    /// Wo das Ziel einer `CONNECT`-Zeile wirklich lauscht: Der Test hat
    /// kein Internet, der Stellvertreter wählt an dessen Stelle diese
    /// Adresse. Ein Ziel, das hier fehlt, ist nicht erreichbar.
    pub network: HashMap<String, SocketAddr>,
    /// Die Wurzeln, mit denen er das Zertifikat des Ziels prüft.
    pub upstream_roots: Vec<rustls::pki_types::CertificateDer<'static>>,
    /// Die Entscheidung über die erste Zeile der Anfrage im Tunnel.
    pub decide: fn(&str) -> Verdict,
}

impl Intercept {
    /// Ein Ziel unter beiden Namen, die in einer `CONNECT`-Zeile stehen
    /// können: dem Namen und der Adresse, die Humanitl dafür auflöst.
    pub fn reaching(
        target: SocketAddr,
        target_ca: &ForeignCa,
        decide: fn(&str) -> Verdict,
    ) -> Self {
        let network = HashMap::from([
            (format!("{HOST}:443"), target),
            (format!("{PUBLIC}:443"), target),
        ]);
        Self {
            network,
            upstream_roots: roots_from_pem(target_ca.pem().as_bytes()),
            decide,
        }
    }

    fn client_config(&self) -> Arc<ClientConfig> {
        let mut roots = RootCertStore::empty();
        for root in &self.upstream_roots {
            roots.add(root.clone()).unwrap();
        }
        let mut config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Arc::new(config)
    }
}

/// Woher das Blatt im Tunnel kommt.
#[derive(Clone)]
enum Leaf {
    /// Ein festes Blatt für einen Namen.
    Fixed(Arc<ServerConfig>),
    /// Ein Blatt für den Namen aus SNI, mit Abfangen und Weiterleiten.
    Intercepting(Arc<ForeignCa>, Arc<Intercept>),
}

/// Der Stellvertreter-Proxy.
pub struct StandIn {
    pub addr: SocketAddr,
    seen: Arc<Mutex<Seen>>,
    task: JoinHandle<()>,
}

impl Drop for StandIn {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl StandIn {
    pub async fn start(ca: &ForeignCa, behaviour: Behaviour) -> Self {
        Self::start_for(ca, HOST, behaviour).await
    }

    /// Wie [`StandIn::start`], mit einem Blatt für `name`.
    pub async fn start_for(ca: &ForeignCa, name: &str, behaviour: Behaviour) -> Self {
        Self::listen(Leaf::Fixed(Arc::new(ca.server_config(name))), behaviour).await
    }

    /// Der Stellvertreter wie mitmproxy im regulären Modus (HUM-513): Blatt
    /// für den Namen aus SNI, Entscheidung je Anfrage, Weiterleiten an das
    /// Ziel der `CONNECT`-Zeile. `CONNECT` öffnet er immer.
    pub async fn start_intercepting(ca: Arc<ForeignCa>, intercept: Intercept) -> Self {
        Self::listen(
            Leaf::Intercepting(ca, Arc::new(intercept)),
            Behaviour::default(),
        )
        .await
    }

    async fn listen(tls: Leaf, behaviour: Behaviour) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Seen::default()));
        let task = {
            let seen = Arc::clone(&seen);
            tokio::spawn(async move {
                while let Ok((tcp, _peer)) = listener.accept().await {
                    let tls = tls.clone();
                    let seen = Arc::clone(&seen);
                    tokio::spawn(serve(tcp, tls, behaviour, seen));
                }
            })
        };
        Self { addr, seen, task }
    }

    pub fn seen(&self) -> Seen {
        self.seen.lock().unwrap().clone()
    }
}

async fn serve(mut tcp: TcpStream, tls: Leaf, behaviour: Behaviour, seen: Arc<Mutex<Seen>>) {
    let Some(head) = read_head(&mut tcp).await else {
        return;
    };
    let (line, host, auth) = parse_head(&head);
    {
        let mut seen = seen.lock().unwrap();
        seen.lines.push(line.clone());
        seen.hosts.push(host);
        seen.auth.push(auth);
    }
    if !line.starts_with("CONNECT ") {
        // Den Rumpf ganz lesen, wie ein Proxy, der die Anfrage vor seiner
        // Entscheidung puffert; erst danach wartet er.
        let length = content_length(&head);
        let mut rest = vec![0_u8; length];
        if tcp.read_exact(&mut rest).await.is_err() {
            return;
        }
        seen.lock().unwrap().body_bytes.push(length);
        tokio::time::sleep(behaviour.head_delay).await;
        let status = behaviour.plain_status;
        let _ = tcp
            .write_all(
                format!(
                    "HTTP/1.1 {status} Stand-in\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                )
                .as_bytes(),
            )
            .await;
        return;
    }
    match behaviour.connect {
        ConnectAnswer::Silent => {
            tokio::time::sleep(Duration::from_secs(120)).await;
            return;
        }
        ConnectAnswer::Status(status) => {
            let _ = tcp
                .write_all(
                    format!("HTTP/1.1 {status} Stand-in\r\nContent-Length: 0\r\n\r\n").as_bytes(),
                )
                .await;
            return;
        }
        ConnectAnswer::Open => {
            tcp.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                .await
                .unwrap();
        }
    }
    let Ok(start) =
        tokio_rustls::LazyConfigAcceptor::new(rustls::server::Acceptor::default(), tcp).await
    else {
        return;
    };
    let sni = start
        .client_hello()
        .server_name()
        .unwrap_or_default()
        .to_owned();
    seen.lock().unwrap().sni.push(sni.clone());
    let (config, intercept) = match tls {
        Leaf::Fixed(config) => (config, None),
        Leaf::Intercepting(ca, intercept) => {
            // Wie mitmproxy: Das Blatt trägt den Namen aus SNI; ohne SNI
            // bleibt nur das Ziel der `CONNECT`-Zeile, oft eine Adresse.
            let target = line.split(' ').nth(1).unwrap_or_default();
            let name = if sni.is_empty() {
                target.rsplit_once(':').map_or(target, |(host, _)| host)
            } else {
                &sni
            };
            (
                Arc::new(ca.server_config(name)),
                Some((intercept, target.to_owned(), name.to_owned())),
            )
        }
    };
    let Ok(mut stream) = start.into_stream(config).await else {
        return;
    };
    let Some(inner) = read_head(&mut stream).await else {
        return;
    };
    let (line, host, auth) = parse_head(&inner);
    {
        let mut seen = seen.lock().unwrap();
        seen.inner_lines.push(line.clone());
        seen.inner_hosts.push(host);
        seen.inner_auth.push(auth);
    }
    if let Some((intercept, target, name)) = intercept {
        let mut body = vec![0_u8; content_length(&inner)];
        if stream.read_exact(&mut body).await.is_err() {
            return;
        }
        let answer = match (intercept.decide)(&line) {
            Verdict::Kill => return,
            Verdict::HoldThenKill(hold) => {
                tokio::time::sleep(hold).await;
                return;
            }
            Verdict::Refuse(status) => format!(
                "HTTP/1.1 {status} Stand-in\r\nContent-Length: 7\r\nConnection: close\r\n\r\nrefused"
            )
            .into_bytes(),
            Verdict::Resume => {
                seen.lock().unwrap().dialed.push(target.clone());
                let Some(answer) = relay(&intercept, &target, &name, &inner, &body).await else {
                    return;
                };
                answer
            }
        };
        let _ = stream.write_all(&answer).await;
        let _ = stream.shutdown().await;
        return;
    }
    tokio::time::sleep(behaviour.head_delay).await;
    let status = behaviour.tunnel_status;
    let _ = stream
        .write_all(
            format!(
                "HTTP/1.1 {status} Stand-in\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            )
            .as_bytes(),
        )
        .await;
    let _ = stream.shutdown().await;
}

/// Schickt die Anfrage an das Ziel `target` der `CONNECT`-Zeile, mit TLS
/// gegen `name`, und liefert die ganze Antwort. `Connection: close` statt
/// eines vorhandenen `Connection`, damit das Ende der Antwort das Ende der
/// Verbindung ist.
async fn relay(
    intercept: &Intercept,
    target: &str,
    name: &str,
    head: &str,
    body: &[u8],
) -> Option<Vec<u8>> {
    let addr = intercept.network.get(target)?;
    let tcp = TcpStream::connect(addr).await.ok()?;
    let server_name = ServerName::try_from(name.to_owned()).ok()?;
    let mut tls = TlsConnector::from(intercept.client_config())
        .connect(server_name, tcp)
        .await
        .ok()?;
    let mut request = head
        .trim_end_matches("\r\n")
        .split("\r\n")
        .filter(|line| {
            !line
                .split_once(':')
                .is_some_and(|(field, _)| field.trim().eq_ignore_ascii_case("connection"))
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    request.push_str("\r\nConnection: close\r\n\r\n");
    tls.write_all(request.as_bytes()).await.ok()?;
    tls.write_all(body).await.ok()?;
    let mut answer = Vec::new();
    // Ein Server darf ohne `close_notify` schließen; was bis dahin kam,
    // ist die Antwort.
    let _ = tls.read_to_end(&mut answer).await;
    (!answer.is_empty()).then_some(answer)
}

pub async fn read_head<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S) -> Option<String> {
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await.ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    String::from_utf8(head).ok()
}

/// `Content-Length` eines Kopfes, ohne ihn 0.
pub fn content_length(head: &str) -> usize {
    head.split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0)
}

/// Erste Zeile, `Host` und `Proxy-Authorization` eines Kopfes.
pub fn parse_head(head: &str) -> (String, String, String) {
    let mut lines = head.split("\r\n");
    let first = lines.next().unwrap_or_default().to_owned();
    let mut host = String::new();
    let mut auth = String::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                value.trim().clone_into(&mut host);
            } else if name.eq_ignore_ascii_case("proxy-authorization") {
                value.trim().clone_into(&mut auth);
            }
        }
    }
    (first, host, auth)
}

/// Die CA des Stellvertreters als Datei, wie `~/.mitmproxy/mitmproxy-ca-cert.pem`.
pub struct CaFile {
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

pub fn ca_file(ca: &ForeignCa) -> CaFile {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("proxy-ca.pem");
    std::fs::write(&path, ca.pem()).unwrap();
    CaFile { path, _dir: dir }
}

/// Die Konfiguration mit dem Stellvertreter als `egress.proxy`. Nur die
/// gebündelten Wurzeln, damit die Ablage dieses Rechners nichts ändert.
pub fn config(proxy: SocketAddr, ca: &Path, resolve: ProxyResolve) -> Config {
    let mut config = Config::default();
    config.tls.trust_system_store = false;
    config.egress.proxy = Some(url::Url::parse(&format!("http://{proxy}")).unwrap());
    config.egress.proxy_ca = Some(ca.to_owned());
    config.egress.proxy_resolve = resolve;
    config.limits.connect_timeout_secs = 2;
    config.limits.header_timeout_secs = 2;
    config.egress.proxy_response_timeout_secs = 10;
    config
}
