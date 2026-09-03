//! Weiterleitung zum Ziel: auflösen, anheften, verbinden, HTTP/1.1 sprechen.
//!
//! Der Weg nach oben ist bewusst schmal und läuft nicht über hudsuckers
//! Client, sondern über den [`Egress`]-Port plus einen eigenen
//! HTTP/1.1-Handschlag (`hyper::client::conn::http1`). Damit gilt: DNS erst
//! nach der Entscheidung (über den [`Resolver`]-Port), die aufgelöste Adresse
//! wird angeheftet, private Ziele werden abgelehnt, und nach oben wird in M1
//! ausschließlich HTTP/1.1 gesprochen (ALPN bietet dem Ziel nur `http/1.1`).
//!
//! Verbindungen werden nie über Authorities hinweg wiederverwendet: Jede
//! erlaubte Anfrage bekommt ihre eigene Verbindung zu genau der
//! [`Authority`](humanitl_core::Authority), für die entschieden wurde. Ein Pool,
//! der `github.com` und `evil.io` auf derselben Verbindung bedient, weil beide
//! auf dieselbe Adresse zeigen, würde die Entscheidung für den einen Host
//! stillschweigend auf den anderen ausdehnen (HUM-023). Ein Pool je
//! `(scheme, host, port)` kann später dazukommen; ein Pool über Authorities
//! hinweg nie.
//!
//! Ein Scheitern auf diesem Weg ist ein [`UpstreamError`], kein Block: der
//! Handler verbucht ihn als [`FlowState::Failed`](humanitl_core::FlowState::Failed)
//! und antwortet dem Client mit `502` (ADR-004, `backlog/CONVENTIONS.md`
//! 4.10). Nur die private Adresse fällt hier an, nachdem der Name aufgelöst
//! wurde — sie ist [`UpstreamError::PrivateAddress`], nicht
//! [`BlockReason::PrivateAddress`](humanitl_core::BlockReason::PrivateAddress),
//! weil die Auflösung eine Laufzeit-Beobachtung ist und nach `Allow`
//! geschieht.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use humanitl_config::IpPreference;
use humanitl_core::{Diagnostic, HostName, HttpRequest, UpstreamError, ip_is_private};
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Request, Response, Version};
use hyper_util::rt::TokioIo;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use crate::egress::Egress;
use crate::resolver::{self, Resolver};

/// Kopfzeilen von Verbindungsrang (RFC 9110 §7.6.1); sie gehören der einen
/// Verbindung und werden weder zum Ziel noch zurück zum Client durchgereicht.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "proxy-connection",
    "keep-alive",
    "transfer-encoding",
    "te",
    "trailer",
    "upgrade",
    "proxy-authenticate",
    "proxy-authorization",
];

/// Die rustls-Client-Konfiguration für Verbindungen zum Ziel.
///
/// Die Vertrauensanker sind die von Mozilla gepflegten Wurzeln
/// (`webpki-roots`), damit der Proxy das echte Zertifikat des Ziels prüft;
/// Tests reichen zusätzlich ihre eigene CA herein. ALPN bietet dem Ziel in M1
/// nur `http/1.1`; `experimental.h2_upstream` schaltet `h2` davor (der
/// Handshake bleibt in M1 dennoch HTTP/1.1, das Flag ist für M6 vorbereitet).
#[derive(Clone)]
pub struct ClientTls {
    config: Arc<ClientConfig>,
}

impl std::fmt::Debug for ClientTls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientTls").finish_non_exhaustive()
    }
}

impl ClientTls {
    /// Baut die Client-Konfiguration mit den System-Wurzeln und optional
    /// zusätzlichen Wurzeln (Test-CA aus `resolver.test_ca`).
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit [`PROXY_003`](humanitl_core::diagnostics::codes::PROXY_003),
    /// wenn rustls die Protokollversionen oder eine zusätzliche Wurzel ablehnt.
    pub fn new(extra_roots: &[CertificateDer<'static>], h2: bool) -> Result<Self, Diagnostic> {
        let why = |msg: String| {
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::PROXY_003,
                humanitl_core::Severity::Error,
            )
            .why(msg)
            .build()
        };

        let mut roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        for cert in extra_roots {
            roots
                .add(cert.clone())
                .map_err(|err| why(format!("test CA is no trust anchor: {err}")))?;
        }

        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|err| why(format!("rustls rejected the protocol versions: {err}")))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = if h2 {
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        } else {
            vec![b"http/1.1".to_vec()]
        };
        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// Die ALPN-Liste, die dem Ziel angeboten wird, in Reihenfolge.
    #[must_use]
    pub fn alpn(&self) -> Vec<Vec<u8>> {
        self.config.alpn_protocols.clone()
    }
}

/// Leitet erlaubte Anfragen zum Ziel.
pub struct Upstream {
    egress: Arc<dyn Egress>,
    resolver: Arc<dyn Resolver>,
    client_tls: ClientTls,
    prefer: IpPreference,
    handshake_timeout: Duration,
}

impl Upstream {
    /// Ein Weiterleiter über `egress`, der Namen über `resolver` auflöst.
    ///
    /// `handshake_timeout` begrenzt den TLS-Handschlag, den HTTP-Handschlag und
    /// das Senden der Anfrage bis zu den Antwort-Kopfzeilen, jeweils für sich.
    /// Der TCP-Verbindungsaufbau hat seine eigene Grenze im Egress-Port; ohne
    /// diese hier könnte ein Ziel, das nach dem Verbinden schweigt, die
    /// Weiterleitung unbegrenzt festhalten.
    #[must_use]
    pub const fn new(
        egress: Arc<dyn Egress>,
        resolver: Arc<dyn Resolver>,
        client_tls: ClientTls,
        prefer: IpPreference,
        handshake_timeout: Duration,
    ) -> Self {
        Self {
            egress,
            resolver,
            client_tls,
            prefer,
            handshake_timeout,
        }
    }

    /// Leitet `request` mit dem gepufferten `body` zum Ziel und liefert die
    /// Antwort ungepuffert zurück.
    ///
    /// `allow_private` erlaubt private Zieladressen (Test-Hook, später die
    /// LLM-Passthrough-Regel). Ohne diesen Schalter ist eine aufgelöste
    /// Adresse in einem privaten Netz ein [`UpstreamError::PrivateAddress`].
    ///
    /// # Errors
    ///
    /// [`UpstreamError`]: `Dns`, wenn kein Name auflösbar ist; `PrivateAddress`,
    /// wenn die Adresse privat ist und nicht erlaubt; `Connect`, wenn TCP oder
    /// der HTTP/1.1-Handschlag scheitert; `Tls`, wenn der TLS-Handschlag
    /// scheitert.
    pub async fn forward(
        &self,
        request: &HttpRequest,
        body: Bytes,
        allow_private: bool,
    ) -> Result<Response<Incoming>, UpstreamError> {
        let authority = &request.authority;
        let ip = match &authority.host {
            HostName::Ip(ip) => *ip,
            HostName::Dns(name) => {
                let addrs = self
                    .resolver
                    .resolve(name)
                    .await
                    .map_err(|_err| UpstreamError::Dns)?;
                resolver::pick(&addrs, self.prefer).ok_or(UpstreamError::Dns)?
            }
        };

        if ip_is_private(ip) && !allow_private {
            return Err(UpstreamError::PrivateAddress(ip));
        }

        let stream = self
            .egress
            .connect(authority, Some(ip))
            .await
            .map_err(|diag| {
                tracing::debug!(?diag, "egress connect failed");
                UpstreamError::Connect
            })?;

        if request.scheme.is_secure() {
            let connector = TlsConnector::from(Arc::clone(&self.client_tls.config));
            let name = server_name(&authority.host)?;
            let tls = tokio::time::timeout(self.handshake_timeout, connector.connect(name, stream))
                .await
                .map_err(|_elapsed| UpstreamError::Timeout)?
                .map_err(|_err| UpstreamError::Tls)?;
            self.send(request, body, tls).await
        } else {
            self.send(request, body, stream).await
        }
    }

    /// Sendet die Anfrage über einen bereits offenen Strom (h1).
    async fn send<S>(
        &self,
        request: &HttpRequest,
        body: Bytes,
        stream: S,
    ) -> Result<Response<Incoming>, UpstreamError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        let (mut sender, conn) = tokio::time::timeout(
            self.handshake_timeout,
            hyper::client::conn::http1::handshake(TokioIo::new(stream)),
        )
        .await
        .map_err(|_elapsed| UpstreamError::Timeout)?
        .map_err(|_err| UpstreamError::Connect)?;
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                tracing::debug!(%err, "upstream connection ended");
            }
        });

        let outgoing = build_outgoing(request, body).map_err(|_err| UpstreamError::Connect)?;
        tokio::time::timeout(self.handshake_timeout, sender.send_request(outgoing))
            .await
            .map_err(|_elapsed| UpstreamError::Timeout)?
            .map_err(|_err| UpstreamError::Connect)
    }
}

/// Baut die Anfrage, die zum Ziel geht: Origin-Form-URI, Host-Kopf gesetzt,
/// Verbindungs-Kopfzeilen entfernt, Body als bekannter Puffer.
fn build_outgoing(request: &HttpRequest, body: Bytes) -> Result<Request<Full<Bytes>>, ()> {
    let mut builder = Request::builder()
        .method(request.method.clone())
        .uri(request.path_and_query.as_str())
        .version(Version::HTTP_11);

    if let Some(headers) = builder.headers_mut() {
        for (name, value) in &request.headers {
            if is_hop_by_hop(name.as_str()) || name.as_str().eq_ignore_ascii_case("host") {
                continue;
            }
            // Content-Length und Content-Encoding bleiben; hyper setzt die
            // Länge aus dem `Full`-Body ohnehin passend, ein vorhandener Wert
            // stimmt nach dem vollständigen Puffern mit der Länge überein.
            if name.as_str().eq_ignore_ascii_case("content-length") {
                continue;
            }
            headers.append(name.clone(), value.clone());
        }
        if let Ok(host) = HeaderValue::from_str(&host_header(request)) {
            headers.insert(HeaderName::from_static("host"), host);
        }
    }

    builder.body(Full::new(body)).map_err(|_err| ())
}

/// Der Wert des `Host`-Kopfes: Host, und der Port nur, wenn er nicht der
/// Standard des Schemas ist. `IPv6` in eckigen Klammern.
fn host_header(request: &HttpRequest) -> String {
    let host = &request.authority.host;
    let host_text = match host {
        HostName::Ip(std::net::IpAddr::V6(ip)) => format!("[{ip}]"),
        other => other.to_string(),
    };
    if request.authority.is_default_port(request.scheme) {
        host_text
    } else {
        format!("{host_text}:{}", request.authority.port)
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}

/// Entfernt Kopfzeilen von Verbindungsrang aus einer Antwort, bevor sie zum
/// Client durchgereicht wird. `Content-Length` bleibt.
pub fn strip_hop_by_hop(headers: &mut hyper::HeaderMap) {
    let to_remove: Vec<HeaderName> = headers
        .keys()
        .filter(|name| is_hop_by_hop(name.as_str()))
        .cloned()
        .collect();
    for name in to_remove {
        headers.remove(&name);
    }
}

fn server_name(host: &HostName) -> Result<ServerName<'static>, UpstreamError> {
    match host {
        HostName::Dns(name) => {
            ServerName::try_from(name.clone()).map_err(|_err| UpstreamError::Tls)
        }
        HostName::Ip(ip) => Ok(ServerName::IpAddress((*ip).into())),
    }
}
