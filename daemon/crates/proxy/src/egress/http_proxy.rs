//! Der Egress über einen Entscheidungs-Proxy des Nutzers (ADR-0022, HUM-511).
//!
//! Mit `egress.proxy` verlässt jede erlaubte Anfrage des Agenten den Daemon
//! über diesen Proxy (mitmproxy, Burp, ein Werkzeug der Firma):
//!
//! - `https`: `CONNECT ZIEL HTTP/1.1` an den Proxy, nach `200` TLS durch den
//!   Tunnel mit `SNI = Name des Ziels`, geprüft gegen die CA aus
//!   `egress.proxy_ca` und die gewohnten Anker. Diese CA steckt in einer
//!   eigenen `ClientConfig` nur für diesen Weg ([`HttpProxy`] hält sie); die
//!   gemeinsame der direkten Verbindungen sieht sie nie.
//! - `http`: die Anfrage geht in absoluter Form an den Proxy
//!   (`GET http://ZIEL/pfad`), `Host` trägt den Namen.
//!
//! Wer auflöst, sagt `egress.proxy_resolve` ([`Resolution`]):
//!
//! - `proxy` (Vorgabe): Humanitl löst nichts auf; `ZIEL` ist der Name. Vorher
//!   lehnt der Weiterleiter private IP-Literale und lokale Namen ab
//!   ([`is_local_name`]).
//! - `humanitl`: `ZIEL` ist die Adresse, die der Weiterleiter nach der
//!   Entscheidung aufgelöst und geprüft hat (ADR-0006).
//!
//! Die Prüfung auf private Adressen gilt dem Ziel, nie dem Proxy selbst: Er
//! darf auf `127.0.0.1` laufen. Die Verbindung zu ihm baut der Daemon auf dem
//! Host auf; die Sandbox hat keinen Weg dorthin.
//!
//! Fristen: TCP zum Proxy und die Antwort auf `CONNECT` teilen sich
//! `limits.connect_timeout_secs`; der TLS-Handschlag behält die Grenze des
//! Weiterleiters. Nur auf den Kopf der Antwort wartet der Weiterleiter bis
//! `egress.proxy_response_timeout_secs` ([`Route::response_timeout`]), weil
//! dort ein Mensch entscheiden kann.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use humanitl_config::secret::{KeyRef, KeyStore};
use humanitl_config::{Config, EgressConfig, ProxyResolve};
use humanitl_core::diagnostics::codes::PROXY_003;
use humanitl_core::{Authority, Diagnostic, HostName, Severity};
use hyper::header::HeaderValue;
use rustls::pki_types::CertificateDer;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::time::Instant;
use zeroize::Zeroizing;

use super::diagnostics::{refused, unreachable, unusable, wants_credentials};
use super::{AsyncStream, Direct, Egress, OpenError, RequestForm, Resolution, Route, Target};
use crate::trust::TrustAnchors;
use crate::upstream::{ClientTls, roots_from_pem};

/// Obergrenze für den Kopf der Antwort auf `CONNECT`. Ein Proxy, der mehr
/// schickt, spricht kein HTTP mit uns.
const MAX_CONNECT_HEAD: usize = 8 * 1024;

/// Wo der Entscheidungs-Proxy lauscht: Host und Port aus `egress.proxy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyEndpoint {
    host: url::Host<String>,
    port: u16,
}

impl ProxyEndpoint {
    /// Host und Port aus einer Adresse `http://HOST:PORT`; ohne Port `80`.
    #[must_use]
    pub fn from_url(url: &url::Url) -> Option<Self> {
        Some(Self {
            host: url.host()?.to_owned(),
            port: url.port_or_known_default()?,
        })
    }

    /// Verbindet per TCP zum Proxy. Ein Name wird über den Namensdienst des
    /// Systems aufgelöst: Er stammt aus der Konfiguration des Nutzers, nicht
    /// vom Agenten, und ist kein Ziel, über das entschieden wird.
    async fn dial(&self) -> std::io::Result<TcpStream> {
        match &self.host {
            url::Host::Ipv4(ip) => {
                TcpStream::connect(SocketAddr::new(IpAddr::V4(*ip), self.port)).await
            }
            url::Host::Ipv6(ip) => {
                TcpStream::connect(SocketAddr::new(IpAddr::V6(*ip), self.port)).await
            }
            url::Host::Domain(name) => TcpStream::connect((name.as_str(), self.port)).await,
        }
    }
}

impl std::fmt::Display for ProxyEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `url::Host` schreibt IPv6 schon in eckigen Klammern.
        write!(f, "{}:{}", self.host, self.port)
    }
}

/// Wahr, wenn ein Name per Definition lokal ist: `localhost`, ein Name unter
/// `.localhost`, `.local`, `.internal` oder `.home.arpa`, oder ein Name ohne
/// Punkt (`metadata`, `instance-data`, ein Rechner im Suchbereich des
/// Proxys).
///
/// Mit `egress.proxy_resolve = "proxy"` löst der Entscheidungs-Proxy den
/// Namen auf seinem Rechner auf; ein solcher Name erreichte dort dessen
/// eigene Dienste, sein Netz oder den Metadaten-Dienst einer Cloud. Humanitl
/// lehnt ihn deshalb vorher ab (ADR-0022). Ein öffentlicher Name, der auf eine
/// private Adresse zeigt, fällt hier nicht auf; das ist der Preis dafür, dass
/// Humanitl nicht auflöst.
#[must_use]
pub fn is_local_name(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    !name.contains('.')
        || [".localhost", ".local", ".internal", ".home.arpa"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

/// Der Egress über einen Entscheidungs-Proxy (ADR-0022).
pub struct HttpProxy {
    endpoint: ProxyEndpoint,
    resolution: Resolution,
    tls: ClientTls,
    authorization: Option<HeaderValue>,
    broken: Option<Diagnostic>,
    connect_timeout: Duration,
    response_timeout: Duration,
}

impl std::fmt::Debug for HttpProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpProxy")
            .field("endpoint", &self.endpoint)
            .field("resolution", &self.resolution)
            .field("authorization", &self.authorization.is_some())
            .field("broken", &self.broken.as_ref().map(|d| d.code))
            .field("connect_timeout", &self.connect_timeout)
            .field("response_timeout", &self.response_timeout)
            .finish_non_exhaustive()
    }
}

impl HttpProxy {
    /// Ein Egress über den Proxy an `endpoint`.
    ///
    /// `tls` ist die Konfiguration **dieses** Wegs, mit der CA des Proxys;
    /// `connect_timeout` gilt für TCP und `CONNECT` zusammen,
    /// `response_timeout` für den Kopf der Antwort.
    #[must_use]
    pub const fn new(
        endpoint: ProxyEndpoint,
        resolution: Resolution,
        tls: ClientTls,
        connect_timeout: Duration,
        response_timeout: Duration,
    ) -> Self {
        Self {
            endpoint,
            resolution,
            tls,
            authorization: None,
            broken: None,
            connect_timeout,
            response_timeout,
        }
    }

    /// Mit Zugangsdaten `NUTZER:PASSWORT` für `Proxy-Authorization: Basic`.
    #[must_use]
    pub fn with_credentials(mut self, user_password: &str) -> Self {
        let token = Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(user_password));
        let value = Zeroizing::new(format!("Basic {}", token.as_str()));
        if let Ok(mut header) = HeaderValue::from_str(&value) {
            header.set_sensitive(true);
            self.authorization = Some(header);
        }
        self
    }

    /// Ein Egress, dessen Einstellungen nicht taugen: Jede Verbindung
    /// scheitert mit `diagnostic`, und nichts geht direkt hinaus.
    #[must_use]
    pub fn broken(mut self, diagnostic: Diagnostic) -> Self {
        self.broken = Some(diagnostic);
        self
    }

    /// `host:port` des Proxys.
    #[must_use]
    pub const fn endpoint(&self) -> &ProxyEndpoint {
        &self.endpoint
    }

    /// Der Befund, mit dem jede Verbindung scheitert, wenn die Einstellungen
    /// nicht taugen.
    #[must_use]
    pub const fn problem(&self) -> Option<&Diagnostic> {
        self.broken.as_ref()
    }

    fn usable(&self) -> Result<(), Diagnostic> {
        self.broken.clone().map_or(Ok(()), Err)
    }

    /// Was in der Anfragezeile steht: der Name, oder mit
    /// [`Resolution::Humanitl`] die geprüfte Adresse.
    fn target_text(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<String, Diagnostic> {
        let host = match (self.resolution, resolved, &authority.host) {
            (Resolution::Humanitl, Some(ip), _) => ip_text(ip),
            (Resolution::Proxy, _, HostName::Ip(ip)) => ip_text(*ip),
            (Resolution::Proxy, _, HostName::Dns(name)) => name.clone(),
            (Resolution::Humanitl, None, _) => {
                return Err(Diagnostic::builder(PROXY_003, Severity::Error)
                    .why(format!(
                        "no address pinned for {authority}; with egress.proxy_resolve = \
                         \"humanitl\" the resolver must run before egress"
                    ))
                    .build());
            }
        };
        Ok(format!("{host}:{}", authority.port))
    }

    /// TCP zum Proxy, bis `deadline`.
    async fn dial(&self, deadline: Instant) -> Result<TcpStream, Diagnostic> {
        let stream = tokio::time::timeout_at(deadline, self.endpoint.dial())
            .await
            .map_err(|_elapsed| {
                unreachable(
                    &self.endpoint,
                    &format!(
                        "no connection within {}s",
                        self.connect_timeout.as_secs_f32()
                    ),
                )
            })?
            .map_err(|err| unreachable(&self.endpoint, &err.to_string()))?;
        let _ = stream.set_nodelay(true);
        Ok(stream)
    }

    /// `CONNECT target` und die Antwort darauf; TCP und Antwort teilen sich
    /// `limits.connect_timeout_secs`.
    async fn tunnel(&self, target: &str) -> Result<TcpStream, Diagnostic> {
        self.usable()?;
        let deadline = Instant::now() + self.connect_timeout;
        let mut stream = self.dial(deadline).await?;
        let mut head = Zeroizing::new(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n"));
        if let Some(auth) = &self.authorization {
            head.push_str("Proxy-Authorization: ");
            head.push_str(auth.to_str().unwrap_or_default());
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        let exchange = async {
            stream.write_all(head.as_bytes()).await?;
            read_status(&mut stream).await
        };
        let status = tokio::time::timeout_at(deadline, exchange)
            .await
            .map_err(|_elapsed| {
                refused(
                    &self.endpoint,
                    target,
                    &format!(
                        "it did not answer the CONNECT within {}s (limits.connect_timeout_secs)",
                        self.connect_timeout.as_secs_f32()
                    ),
                )
            })?
            .map_err(|err| {
                refused(
                    &self.endpoint,
                    target,
                    &format!("its answer was unreadable: {err}"),
                )
            })?;
        match status {
            200 => Ok(stream),
            407 => Err(wants_credentials(
                &self.endpoint,
                &format!("CONNECT {target}"),
            )),
            other => Err(refused(
                &self.endpoint,
                target,
                &format!("it answered {other}"),
            )),
        }
    }
}

#[async_trait]
impl Egress for HttpProxy {
    fn resolution(&self) -> Resolution {
        self.resolution
    }

    fn behind_second_proxy(&self) -> bool {
        true
    }

    async fn open(&self, target: Target<'_>) -> Result<Route, OpenError> {
        self.open_route(target).await.map_err(OpenError::Route)
    }

    /// Lehnt ab: Über einen Entscheidungs-Proxy gibt es keinen schlichten
    /// Byte-Strom zum Ziel. Ein Tunnel ohne die eigene TLS-Konfiguration und
    /// ohne die Anfrageform des Wegs prüfte gegen die falschen Anker und
    /// schriebe `http` in der falschen Form; nur [`Egress::open`] gilt.
    async fn connect(
        &self,
        authority: &Authority,
        _resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic> {
        Err(Diagnostic::builder(PROXY_003, Severity::Error)
            .why(format!(
                "a raw connection to {authority} was asked of the decision proxy egress; it \
                 opens routes only through open(), with the request form and TLS of that route"
            ))
            .build())
    }
}

impl HttpProxy {
    /// Der Weg zu `target`: `CONNECT` bei `https`, die Verbindung zum Proxy
    /// für eine Anfrage in absoluter Form bei `http`.
    async fn open_route(&self, target: Target<'_>) -> Result<Route, Diagnostic> {
        self.usable()?;
        let text = self.target_text(target.authority, target.resolved)?;
        let via = Some(self.endpoint.to_string());
        if target.secure {
            let stream = self.tunnel(&text).await?;
            return Ok(Route {
                stream: Box::new(stream),
                form: RequestForm::Origin,
                tls: Some(self.tls.clone()),
                response_timeout: Some(self.response_timeout),
                via,
            });
        }
        let stream = self.dial(Instant::now() + self.connect_timeout).await?;
        Ok(Route {
            stream: Box::new(stream),
            form: RequestForm::Absolute {
                authority: text,
                proxy_authorization: self.authorization.clone(),
            },
            tls: None,
            response_timeout: Some(self.response_timeout),
            via,
        })
    }
}

/// Der Egress des Verkehrs des Agenten und die TLS-Konfiguration der
/// direkten Verbindungen, aus derselben Konfiguration gebaut.
pub struct Wiring {
    /// Direkt, oder über `egress.proxy`.
    pub egress: Arc<dyn Egress>,
    /// Die gemeinsame TLS-Konfiguration der direkten Verbindungen. Sie trägt
    /// nie die CA aus `egress.proxy_ca`.
    pub client_tls: ClientTls,
}

impl std::fmt::Debug for Wiring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wiring").finish_non_exhaustive()
    }
}

/// Baut Egress und TLS-Konfiguration des Weiterleiters aus der Konfiguration
/// (HUM-511).
///
/// Ohne `egress.proxy` ist das genau der Stapel von vorher: [`Direct`] mit
/// `limits.connect_timeout_secs` und die Anker aus `tls.trust_system_store`
/// samt `extra_roots`. Mit `egress.proxy` ein [`HttpProxy`], dessen eigene
/// TLS-Konfiguration zusätzlich die CA aus `egress.proxy_ca` trägt; die
/// gemeinsame bleibt, wie sie ist.
///
/// `keys` liest die Zugangsdaten aus `egress.proxy_auth_ref`; das startet
/// das Werkzeug des Schlüsselbunds und blockiert. Ohne `keys` geht der Proxy
/// ohne Anmeldung. Taugt die CA-Datei oder die Zugangsdaten nicht, scheitert
/// jede Verbindung mit `PROXY_017`, statt ohne sie oder direkt hinauszugehen.
///
/// # Errors
///
/// `PROXY_003`, wenn rustls die Anker ablehnt, wie [`ClientTls::new`].
pub fn from_config(
    config: &Config,
    extra_roots: &[CertificateDer<'static>],
    keys: Option<&KeyStore>,
) -> Result<Wiring, Diagnostic> {
    let anchors = TrustAnchors::configured(config.tls.trust_system_store);
    let h2 = config.experimental.h2_upstream;
    let client_tls = ClientTls::new(&anchors, extra_roots, h2)?;
    let connect_timeout = Duration::from_secs(config.limits.connect_timeout_secs);
    let egress: Arc<dyn Egress> = match decision_proxy(
        &config.egress,
        &anchors,
        extra_roots,
        h2,
        connect_timeout,
        keys,
    )? {
        Some(proxy) => Arc::new(proxy),
        None => Arc::new(Direct::new(connect_timeout)),
    };
    Ok(Wiring { egress, client_tls })
}

/// Der [`HttpProxy`] aus `egress.*`, oder `None` ohne `egress.proxy`.
///
/// Auch der Weg von `EgressCheck` (`humanitl egress check`): dieselben
/// Einstellungen, derselbe Adapter.
///
/// # Errors
///
/// `PROXY_003`, wenn rustls die Anker ablehnt.
pub fn decision_proxy(
    egress: &EgressConfig,
    anchors: &TrustAnchors,
    extra_roots: &[CertificateDer<'static>],
    h2: bool,
    connect_timeout: Duration,
    keys: Option<&KeyStore>,
) -> Result<Option<HttpProxy>, Diagnostic> {
    let Some(url) = &egress.proxy else {
        return Ok(None);
    };
    let Some(endpoint) = ProxyEndpoint::from_url(url) else {
        // Das Laden lehnt eine solche Adresse ab (`CONFIG_003`); kommt sie
        // trotzdem an, geht nichts hinaus, statt direkt.
        return Err(unusable("egress.proxy", "it names no host and port"));
    };
    let resolution = match egress.proxy_resolve {
        ProxyResolve::Proxy => Resolution::Proxy,
        ProxyResolve::Humanitl => Resolution::Humanitl,
    };
    let response_timeout = Duration::from_secs(egress.proxy_response_timeout_secs);
    let (roots, mut ca_problem) = match &egress.proxy_ca {
        None => (Vec::new(), None),
        Some(path) => match std::fs::read(path) {
            Ok(pem) => {
                let roots = roots_from_pem(&pem);
                let problem = roots.is_empty().then(|| {
                    unusable(
                        "egress.proxy_ca",
                        &format!("{} holds no PEM certificate", path.display()),
                    )
                });
                (roots, problem)
            }
            Err(err) => (
                Vec::new(),
                Some(unusable(
                    "egress.proxy_ca",
                    &format!("{} cannot be read ({err})", path.display()),
                )),
            ),
        },
    };
    let mut all_roots = extra_roots.to_vec();
    all_roots.extend(roots);
    let tls = match ClientTls::new(anchors, &all_roots, h2) {
        Ok(tls) => tls,
        // Ein Zertifikat, das rustls nicht als Anker nimmt, ist ein Fehler
        // dieses Wegs, nicht des Daemons: Der Weg steht, sendet aber nichts.
        Err(_rejected) => {
            ca_problem.get_or_insert_with(|| {
                unusable(
                    "egress.proxy_ca",
                    "the file holds a certificate that is no usable trust anchor",
                )
            });
            ClientTls::new(anchors, extra_roots, h2)?
        }
    };
    let mut proxy = HttpProxy::new(endpoint, resolution, tls, connect_timeout, response_timeout);
    if let Some(problem) = ca_problem {
        return Ok(Some(proxy.broken(problem)));
    }
    if let (Some(text), Some(keys)) = (&egress.proxy_auth_ref, keys) {
        let Some(reference) = KeyRef::parse(text) else {
            return Ok(Some(proxy.broken(unusable(
                "egress.proxy_auth_ref",
                &format!("{text:?} is no keyring reference"),
            ))));
        };
        match keys.lookup_credentials(&reference) {
            Ok(credentials) => proxy = proxy.with_credentials(credentials.expose()),
            Err(diagnostic) => {
                tracing::debug!(
                    ?diagnostic,
                    "the credentials of the decision proxy are missing"
                );
                return Ok(Some(proxy.broken(unusable(
                    "egress.proxy_auth_ref",
                    &format!("the keyring does not hand out the credentials {reference}"),
                ))));
            }
        }
    }
    Ok(Some(proxy))
}

/// Eine Adresse, wie sie in einer Anfragezeile steht: IPv6 in Klammern.
fn ip_text(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    }
}

/// Liest den Kopf der Antwort auf `CONNECT` bis zur Leerzeile, Byte für
/// Byte: Was danach kommt, gehört schon dem Tunnel und darf nicht in einem
/// Puffer hängen bleiben.
async fn read_status(stream: &mut TcpStream) -> std::io::Result<u16> {
    let mut head = Vec::with_capacity(128);
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_CONNECT_HEAD {
            return Err(std::io::Error::other("the answer head is too long"));
        }
        if stream.read(&mut byte).await? == 0 {
            return Err(std::io::Error::other("the proxy closed the connection"));
        }
        head.push(byte[0]);
    }
    let line = head.split(|b| *b == b'\n').next().unwrap_or_default();
    let line = std::str::from_utf8(line).map_err(|_err| std::io::Error::other("not HTTP"))?;
    let mut parts = line.split_ascii_whitespace();
    match (parts.next(), parts.next()) {
        (Some(version), Some(code)) if version.starts_with("HTTP/1.") => code
            .parse::<u16>()
            .map_err(|_err| std::io::Error::other("no status code")),
        _ => Err(std::io::Error::other("not HTTP")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn local_names_are_refused_and_public_names_are_not() {
        for local in [
            "localhost",
            "LOCALHOST.",
            "api.localhost",
            "nas.local",
            "metadata.google.internal",
            "router.home.arpa",
            "metadata",
            "instance-data",
        ] {
            assert!(is_local_name(local), "{local}");
        }
        for public in [
            "example.com",
            "api.github.com",
            "local.example.com",
            "internal.io",
        ] {
            assert!(!is_local_name(public), "{public}");
        }
    }

    #[test]
    fn the_endpoint_reads_host_and_port() {
        let url = url::Url::parse("http://127.0.0.1:8080").unwrap();
        assert_eq!(
            ProxyEndpoint::from_url(&url).unwrap().to_string(),
            "127.0.0.1:8080"
        );
        let url = url::Url::parse("http://[::1]:3128").unwrap();
        assert_eq!(
            ProxyEndpoint::from_url(&url).unwrap().to_string(),
            "[::1]:3128"
        );
        let url = url::Url::parse("http://proxy.lan").unwrap();
        assert_eq!(
            ProxyEndpoint::from_url(&url).unwrap().to_string(),
            "proxy.lan:80"
        );
    }
}
