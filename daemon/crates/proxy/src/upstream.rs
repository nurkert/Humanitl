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
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use humanitl_config::IpPreference;
use humanitl_config::secret::KeyStore;
use humanitl_core::{
    Diagnostic, FlowEvent, FlowId, HostName, HttpRequest, UpstreamError, ip_is_private,
};
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Request, Response, Version};
use hyper_util::rt::TokioIo;
use rustls::ClientConfig;
use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::TlsConnector;

use crate::body::{self, ResponseBody, Stall, StreamProgress};
use crate::egress::diagnostics::{self, wants_credentials};
use crate::egress::{
    AsyncStream, Egress, OpenError, RequestForm, Resolution, Route, Target, screen,
};
use crate::hold::HoldQueue;
use crate::resolver::{self, AddressRefusal, Resolver};
use crate::trust::{TrustAnchors, TrustSource};

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
/// Die Vertrauensanker kommen aus [`TrustAnchors`]: per Vorgabe die
/// Zertifikatsablage des Systems, mit `tls.trust_system_store = false` nur die
/// von Mozilla gepflegten Wurzeln (`webpki-roots`). So prüft der Proxy das
/// echte Zertifikat des Ziels gegen dieselben Stellen wie `curl` auf diesem
/// Rechner (HUM-257). Tests und `--allow-test-ca` reichen zusätzlich ihre
/// eigene CA herein. ALPN bietet dem Ziel in M1 nur `http/1.1`;
/// `experimental.h2_upstream` schaltet `h2` davor (der Handshake bleibt in M1
/// dennoch HTTP/1.1, das Flag ist für M6 vorbereitet).
#[derive(Clone)]
pub struct ClientTls {
    config: Arc<ClientConfig>,
    source: TrustSource,
}

impl std::fmt::Debug for ClientTls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientTls").finish_non_exhaustive()
    }
}

impl ClientTls {
    /// Baut die Client-Konfiguration aus den Ankern `anchors` und optional
    /// zusätzlichen Wurzeln (Test-CA aus `resolver.test_ca`).
    ///
    /// Der Daemon reicht [`TrustAnchors::configured`] herein. Tests nehmen
    /// meist [`TrustAnchors::bundled`], damit die Ablage der Maschine, auf der
    /// sie laufen, nichts an ihrem Ergebnis ändert.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit [`PROXY_003`](humanitl_core::diagnostics::codes::PROXY_003),
    /// wenn rustls die Protokollversionen oder eine zusätzliche Wurzel ablehnt.
    pub fn new(
        anchors: &TrustAnchors,
        extra_roots: &[CertificateDer<'static>],
        h2: bool,
    ) -> Result<Self, Diagnostic> {
        let why = |msg: String| {
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::PROXY_003,
                humanitl_core::Severity::Error,
            )
            .why(msg)
            .build()
        };

        let mut roots = anchors.roots();
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
            source: anchors.source(),
        })
    }

    /// Wie [`ClientTls::new`], mit den Ankern, die `tls.trust_system_store`
    /// verlangt ([`TrustAnchors::configured`]): der Weg des Daemons.
    ///
    /// # Errors
    ///
    /// Wie [`ClientTls::new`].
    pub fn configured(
        trust_system_store: bool,
        extra_roots: &[CertificateDer<'static>],
        h2: bool,
    ) -> Result<Self, Diagnostic> {
        Self::new(
            &TrustAnchors::configured(trust_system_store),
            extra_roots,
            h2,
        )
    }

    /// Die ALPN-Liste, die dem Ziel angeboten wird, in Reihenfolge.
    #[must_use]
    pub fn alpn(&self) -> Vec<Vec<u8>> {
        self.config.alpn_protocols.clone()
    }
}

/// Liest alle Zertifikate aus einem PEM-Text, in der Reihenfolge der Datei.
///
/// Der Gegenpart zu `extra_roots` in [`ClientTls::new`]: Der Daemon liest die
/// Datei aus `resolver.test_ca` und reicht das Ergebnis dorthin weiter
/// (`humanitld`, `--allow-test-ca`, HUM-087). Das Lesen liegt hier und nicht
/// im Binary, weil hier schon rustls steht und der Typ der Wurzeln von dort
/// kommt.
///
/// Unlesbare oder fremde Blöcke werden übersprungen, wie in
/// [`CaStore::render_bundle`](crate::ca::CaStore::render_bundle) und
/// [`read_system_bundle`](crate::ca::read_system_bundle). Eine leere
/// Rückgabe heißt deshalb: in diesem Text steht kein Zertifikat. Wer daraus
/// einen Fehler machen will, entscheidet das an seiner Stelle — hier gibt es
/// keinen Pfad und keinen Schlüssel, die ein `Diagnostic` benennen könnte.
///
/// Geprüft wird das Material erst in [`ClientTls::new`]: Ein Block, den rustls
/// nicht als Vertrauensanker annimmt, wird dort `PROXY_003`.
#[must_use]
pub fn roots_from_pem(pem: &[u8]) -> Vec<CertificateDer<'static>> {
    CertificateDer::pem_slice_iter(pem).flatten().collect()
}

/// Der Rumpf, der zum Ziel geht.
///
/// Fast immer [`Outgoing::Buffered`]: Der Rumpf liegt vollständig vor, weil
/// über ihn entschieden wurde (ADR-005), und die Länge steht damit fest. Nur
/// eine Anfrage über `limits.hold_body_cap_bytes`, die eine Regel mit
/// `stream: true` hinauslässt, geht als [`Outgoing::Streamed`] hinaus
/// (HUM-057): Ihr Rumpf wird nie gepuffert, entschieden wurde nur über ihren
/// Kopf.
pub enum Outgoing {
    /// Ein vollständig gepufferter Rumpf.
    Buffered(Bytes),
    /// Ein Rumpf, der beim Senden erst vom Client gelesen wird.
    Streamed {
        /// Der Strom, schon mit Uhr und Mitschnitt umwickelt
        /// ([`crate::body::streamed_request`]).
        body: ResponseBody,
        /// Die angekündigte Länge (`Content-Length` des Clients), falls es
        /// eine gab. Ohne sie geht der Rumpf mit `Transfer-Encoding: chunked`
        /// hinaus.
        length: Option<u64>,
        /// Die Fristen, über die der Weiterleiter wacht, solange er auf die
        /// Antwort wartet.
        watch: StreamWatch,
    },
}

/// Worüber der Weiterleiter bei einem gestreamten Upload wacht (HUM-057).
///
/// Zwei Spannen, die sonst unbegrenzt wären: Das Ziel liest den Rumpf nicht
/// mehr weiter (dann fragt hyper nicht mehr nach Stücken, und die Uhr im
/// Strom steht), oder es nimmt den ganzen Rumpf und schickt nie
/// Kopfzeilen. Endet eine davon, scheitert die Weiterleitung mit
/// [`UpstreamError::Timeout`]. Eine Grenze der Gesamtdauer ist keine von
/// beiden: Ein langer Upload, den das Ziel stetig liest, läuft durch.
#[derive(Debug, Clone)]
pub struct StreamWatch {
    /// Was der Strom über sich mitschreibt.
    pub progress: Arc<StreamProgress>,
    /// `limits.body_timeout_secs`: so lange darf das Ziel kein Stück annehmen.
    pub idle: Duration,
    /// `limits.header_timeout_secs`: so lange darf das Ziel nach dem Ende des
    /// Rumpfs ohne Antwort-Kopfzeilen bleiben.
    pub head: Duration,
    /// Wohin ein Stillstand nach den Antwort-Kopfzeilen gemeldet wird: an
    /// diesen Fluss, über diese Warteschlange. Vorher meldet ihn der Handler
    /// selbst, weil er die Weiterleitung dann noch in der Hand hat.
    pub report: Option<(FlowId, Arc<HoldQueue>)>,
}

impl StreamWatch {
    /// Woran der Upload gerade hängt, falls er hängt.
    #[must_use]
    pub fn stalled(&self) -> Option<Stall> {
        self.progress.stalled(self.idle, self.head)
    }
}

/// Wie oft der Weiterleiter auf einen hängenden Upload schaut.
const WATCH_TICK: Duration = Duration::from_millis(100);

impl core::fmt::Debug for Outgoing {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Buffered(bytes) => f.debug_tuple("Buffered").field(&bytes.len()).finish(),
            Self::Streamed { length, .. } => f
                .debug_struct("Streamed")
                .field("length", length)
                .finish_non_exhaustive(),
        }
    }
}

impl From<Bytes> for Outgoing {
    fn from(bytes: Bytes) -> Self {
        Self::Buffered(bytes)
    }
}

/// Ein gescheiterter Weg zum Ziel.
///
/// [`UpstreamError`] ist das, was der Automat verbucht und der Client in der
/// `502` sieht. Dazu kommt, was nur der Mensch erfahren soll: ob der
/// TLS-Handschlag daran scheiterte, dass das Zertifikat des Ziels von einer
/// Stelle stammt, der dieser Rechner nicht vertraut, und aus welchen Ankern
/// geprüft wurde (HUM-257). Daraus bauen Handler und Endpunkt-Probe ein `why`
/// mit dem nächsten Schritt; die Antwort an den Agenten bleibt
/// `reason: upstream_tls`.
#[derive(Debug, Clone, PartialEq)]
pub struct ForwardError {
    /// Was der Automat verbucht.
    pub error: UpstreamError,
    /// Gesetzt, wenn das Zertifikat des Ziels von einer unbekannten Stelle
    /// unterschrieben ist (`UnknownIssuer`), mit der Herkunft der Anker.
    pub untrusted_issuer: Option<TrustSource>,
    /// Der Befund des Wegs, ungekürzt bis zur Oberfläche (HUM-511): etwa
    /// ein Entscheidungs-Proxy, der nicht erreichbar ist (`PROXY_012`),
    /// `407` sagt (`PROXY_014`) oder dessen CA nicht passt (`PROXY_015`).
    /// Der Handler stellt ihn an den Fluss; der Agent sieht weiter nur
    /// [`ForwardError::error`].
    pub diagnostic: Option<Box<Diagnostic>>,
    /// Gesetzt, wenn ein lokaler Name vor einem Entscheidungs-Proxy
    /// abgelehnt wurde; der Handler stellt `PROXY_016` mit einer Regel an
    /// den Fluss, wie `PROXY_008` zu einer privaten Adresse.
    pub local_name: bool,
}

impl ForwardError {
    /// Ordnet einen gescheiterten TLS-Handschlag ein.
    ///
    /// `tokio-rustls` reicht den Fehler von rustls als innere Ursache eines
    /// `io::Error` herauf. Nur `UnknownIssuer` zählt als unbekannte Stelle;
    /// ein abgelaufenes Zertifikat oder ein falscher Name ist etwas anderes
    /// und bekäme mit dem Rat, eine CA zu installieren, einen falschen Weg.
    fn tls(err: &std::io::Error, source: TrustSource) -> Self {
        tracing::debug!(%err, "upstream TLS handshake failed");
        let unknown_issuer = err
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>())
            .is_some_and(|inner| {
                matches!(
                    inner,
                    rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer)
                )
            });
        Self {
            error: UpstreamError::Tls,
            untrusted_issuer: unknown_issuer.then_some(source),
            diagnostic: None,
            local_name: false,
        }
    }

    /// Ein Scheitern mit einem Befund, den der Mensch sehen soll.
    #[must_use]
    pub fn with_diagnostic(error: UpstreamError, diagnostic: Diagnostic) -> Self {
        Self {
            error,
            untrusted_issuer: None,
            diagnostic: Some(Box::new(diagnostic)),
            local_name: false,
        }
    }

    /// Ein lokaler Name vor einem Entscheidungs-Proxy (ADR-0022).
    #[must_use]
    pub const fn local_name() -> Self {
        Self {
            error: UpstreamError::Dns,
            untrusted_issuer: None,
            diagnostic: None,
            local_name: true,
        }
    }
}

impl From<UpstreamError> for ForwardError {
    fn from(error: UpstreamError) -> Self {
        Self {
            error,
            untrusted_issuer: None,
            diagnostic: None,
            local_name: false,
        }
    }
}

impl std::fmt::Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, f)
    }
}

impl std::error::Error for ForwardError {}

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

    /// Der Weiterleiter des Verkehrs des Agenten aus der Konfiguration
    /// (HUM-511): direkt, oder mit `egress.proxy` über den
    /// Entscheidungs-Proxy ([`crate::egress::from_config`]).
    ///
    /// `keys` liest die Zugangsdaten aus `egress.proxy_auth_ref` und
    /// blockiert dafür; ohne `keys` geht der Proxy ohne Anmeldung.
    ///
    /// # Errors
    ///
    /// Wie [`crate::egress::from_config`].
    pub fn from_config(
        config: &humanitl_config::Config,
        resolver: Arc<dyn Resolver>,
        extra_roots: &[CertificateDer<'static>],
        keys: Option<&KeyStore>,
    ) -> Result<Self, Diagnostic> {
        let wiring = crate::egress::from_config(config, extra_roots, keys)?;
        Ok(Self::new(
            wiring.egress,
            resolver,
            wiring.client_tls,
            config.resolver.prefer,
            Duration::from_secs(config.limits.header_timeout_secs),
        ))
    }

    /// Das feste Ziel von [`Upstream::check_decision_proxy`].
    pub const CHECK_TARGET: (&'static str, u16) = crate::egress::check::CHECK_TARGET;

    /// Prüft den Entscheidungs-Proxy aus `config` (`humanitl egress check`,
    /// HUM-511): Tunnel und TLS-Handschlag zu [`Upstream::CHECK_TARGET`],
    /// ohne eine Anfrage ([`crate::egress::check::run`]).
    ///
    /// # Errors
    ///
    /// Wie [`crate::egress::check::run`].
    pub async fn check_decision_proxy(
        config: &humanitl_config::Config,
        keys: Option<KeyStore>,
    ) -> Result<(), Diagnostic> {
        crate::egress::check::run(config, keys).await
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
    /// [`ForwardError`] mit dem [`UpstreamError`]: `Dns`, wenn kein Name
    /// auflösbar ist; `PrivateAddress`, wenn die Adresse privat ist und nicht
    /// erlaubt; `Connect`, wenn TCP oder der HTTP/1.1-Handschlag scheitert;
    /// `Tls`, wenn der TLS-Handschlag scheitert, bei einer unbekannten Stelle
    /// mit [`ForwardError::untrusted_issuer`].
    pub async fn forward(
        &self,
        request: &HttpRequest,
        body: impl Into<Outgoing>,
        allow_private: bool,
    ) -> Result<Response<Incoming>, ForwardError> {
        let resolved = self.pin(request, allow_private).await?;
        self.send_via(request, body.into(), resolved).await
    }

    /// Die Adresse, mit der der Egress verbindet, nach den Prüfungen vor
    /// einem zweiten Proxy (ADR-0022): Ein lokaler Name fällt vor jeder
    /// Auflösung, gleich wer auflöst; mit `egress.proxy_resolve = "proxy"`
    /// fragt Humanitl danach keinen Namensdienst, der Proxy löst nach seiner
    /// Entscheidung auf.
    async fn pin(
        &self,
        request: &HttpRequest,
        allow_private: bool,
    ) -> Result<Option<std::net::IpAddr>, ForwardError> {
        if self.egress.behind_second_proxy() {
            screen::local_name(request, allow_private)?;
        }
        match self.egress.resolution() {
            Resolution::Humanitl => {
                Ok(Some(self.resolve(&request.authority, allow_private).await?))
            }
            Resolution::Proxy => screen::literal(request, allow_private),
        }
    }

    /// Löst die Authority auf und prüft die Adresse.
    ///
    /// Der eine Ort, an dem aus einem Namen eine Adresse wird (ADR-006). Ein
    /// IP-Literal wird nicht aufgelöst, sondern nur geprüft. `allow_private`
    /// entscheidet, ob eine Adresse aus RFC 1918, Loopback, Link-Local oder
    /// CGNAT durchgeht.
    ///
    /// Die Adresse kommt zurück, statt gleich verbunden zu werden, damit ein
    /// Aufrufer sie kennt, der mehr über sie wissen muss als der
    /// Weiterleitungspfad — die Endpunkt-Probe (`crate::llm_probe`) sagt
    /// darüber, ob der Endpunkt in einem privaten Netz liegt, und darf dafür
    /// kein zweites Mal fragen.
    ///
    /// # Errors
    ///
    /// [`UpstreamError::Dns`], wenn nichts auflöst oder nichts Verbindbares
    /// zurückkommt; [`UpstreamError::PrivateAddress`], wenn die Adresse privat
    /// ist und nicht erlaubt.
    pub async fn resolve(
        &self,
        authority: &humanitl_core::Authority,
        allow_private: bool,
    ) -> Result<std::net::IpAddr, UpstreamError> {
        match &authority.host {
            // Ein IP-Literal wird nicht aufgelöst: Es steht schon da, und
            // entschieden wurde für genau diese Adresse (ADR-007: IP-Literale
            // matchen nie eine Host-Regel).
            HostName::Ip(ip) => {
                if ip_is_private(*ip) && !allow_private {
                    return Err(UpstreamError::PrivateAddress(*ip));
                }
                Ok(*ip)
            }
            // Der Name des Sprachmodells wird nie aufgelöst (HUM-387): Der
            // Handler setzt ihn vor den Regeln auf das Ziel um oder lehnt ab.
            // Kommt er trotzdem hier an, fragt niemand den Namensdienst.
            HostName::Dns(_) if authority.host.is_llm_alias() => Err(UpstreamError::Dns),
            // Erst hier, nach `Decided(Allow | AllowEdited)`, verlässt eine
            // Frage den Rechner (ADR-006).
            HostName::Dns(name) => {
                let addrs = self.resolver.resolve(name).await.map_err(|err| {
                    tracing::debug!(%err, "resolving the target failed");
                    UpstreamError::Dns
                })?;
                match resolver::select(&addrs, self.prefer, allow_private) {
                    Ok(ip) => Ok(ip),
                    Err(AddressRefusal::Private(ip)) => Err(UpstreamError::PrivateAddress(ip)),
                    // Nichts Verbindbares in der Antwort. Kein zweiter Versuch
                    // mit einem anderen Resolver: Ein Rückfall auf das
                    // System-DNS wäre genau die Abfrage, die niemand erlaubt
                    // hat (HUM-024, Fallstricke).
                    Err(AddressRefusal::NoUsable) => Err(UpstreamError::Dns),
                }
            }
        }
    }

    /// Leitet `request` an eine schon aufgelöste und geprüfte Adresse.
    ///
    /// Die Adresse ist damit angeheftet: Zwischen der Prüfung in
    /// [`Upstream::resolve`] und dem Verbinden kommt keine zweite Auflösung
    /// dazwischen, und ein Name, der beim zweiten Mal anders antwortet
    /// (Rebinding), erreicht das Ziel nicht (ADR-006, ADR-017).
    ///
    /// # Errors
    ///
    /// Wie [`Upstream::forward`], ohne `Dns` und `PrivateAddress`.
    pub async fn forward_to(
        &self,
        request: &HttpRequest,
        body: impl Into<Outgoing>,
        ip: std::net::IpAddr,
    ) -> Result<Response<Incoming>, ForwardError> {
        self.send_via(request, body.into(), Some(ip)).await
    }

    /// Baut den Weg zu `request` auf, samt TLS, und sendet nichts (HUM-511).
    ///
    /// Die Prüfung von `humanitl egress check`: Löst auf oder prüft das Ziel
    /// wie [`Upstream::forward`], öffnet den Weg über den Egress und, bei
    /// `https`, den TLS-Handschlag darin. Hinter einem Entscheidungs-Proxy
    /// beweist das, dass er erreichbar ist, den Tunnel aufbaut und seine CA
    /// passt, ohne dass dort jemand über eine Anfrage entscheiden muss.
    ///
    /// # Errors
    ///
    /// Wie [`Upstream::forward`].
    pub async fn handshake(&self, request: &HttpRequest) -> Result<(), ForwardError> {
        let resolved = self.pin(request, false).await?;
        self.open(request, resolved).await.map(drop)
    }

    /// Öffnet den Weg über den Egress und, bei `https`, TLS darin.
    async fn open(
        &self,
        request: &HttpRequest,
        resolved: Option<std::net::IpAddr>,
    ) -> Result<Opened, ForwardError> {
        let authority = &request.authority;
        let route = self
            .egress
            .open(Target {
                authority,
                secure: request.scheme.is_secure(),
                resolved,
            })
            .await
            .map_err(|failure| {
                tracing::debug!(diag = ?failure.diagnostic(), "egress connect failed");
                // Der Befund eines Entscheidungs-Proxys geht bis zur
                // Oberfläche; ein gescheitertes direktes Verbinden bleibt,
                // was es war (HUM-511).
                match failure {
                    OpenError::Direct(_) => ForwardError::from(UpstreamError::Connect),
                    OpenError::Route(diag) => {
                        ForwardError::with_diagnostic(UpstreamError::Connect, diag)
                    }
                }
            })?;
        let Route {
            stream,
            form,
            tls,
            response_timeout,
            via,
        } = route;
        let wait = Wait {
            handshake: self.handshake_timeout,
            response: response_timeout.unwrap_or(self.handshake_timeout),
        };
        let stream: Box<dyn AsyncStream> = if request.scheme.is_secure() {
            let client_tls = tls.as_ref().unwrap_or(&self.client_tls);
            let connector = TlsConnector::from(Arc::clone(&client_tls.config));
            let name = server_name(&authority.host)?;
            let tls = tokio::time::timeout(self.handshake_timeout, connector.connect(name, stream))
                .await
                .map_err(|_elapsed| {
                    via.as_deref().map_or_else(
                        || ForwardError::from(UpstreamError::Timeout),
                        |proxy| {
                            ForwardError::with_diagnostic(
                                UpstreamError::Timeout,
                                diagnostics::tls_failed(
                                    proxy,
                                    authority,
                                    &format!(
                                        "no handshake within {}s",
                                        self.handshake_timeout.as_secs()
                                    ),
                                ),
                            )
                        },
                    )
                })?
                .map_err(|err| {
                    let failure = ForwardError::tls(&err, client_tls.source);
                    // Hinter dem Entscheidungs-Proxy ist eine unbekannte
                    // Stelle fast immer dessen CA, die fehlt; der Rat, die
                    // Ablage des Systems zu erweitern, wäre falsch. Jeder
                    // andere Grund nennt den Proxy und das Ziel.
                    match (&via, failure.untrusted_issuer) {
                        (Some(proxy), Some(_)) => ForwardError::with_diagnostic(
                            UpstreamError::Tls,
                            diagnostics::ca_mismatch(proxy, authority),
                        ),
                        (Some(proxy), None) => ForwardError::with_diagnostic(
                            UpstreamError::Tls,
                            diagnostics::tls_failed(proxy, authority, &err.to_string()),
                        ),
                        (None, _) => failure,
                    }
                })?;
            Box::new(tls)
        } else {
            stream
        };
        Ok(Opened {
            stream,
            form,
            wait,
            via,
        })
    }

    /// Öffnet den Weg über den Egress und sendet die Anfrage in der Form, die
    /// er verlangt ([`Route`]).
    async fn send_via(
        &self,
        request: &HttpRequest,
        body: Outgoing,
        resolved: Option<std::net::IpAddr>,
    ) -> Result<Response<Incoming>, ForwardError> {
        let Opened {
            stream,
            form,
            wait,
            via,
        } = self.open(request, resolved).await?;
        let response = self
            .send(request, body, stream, &form, wait)
            .await
            .map_err(|failure| match (&via, failure) {
                // Über den Proxy wartet der Kopf der Antwort auf einen
                // Menschen dort; der Befund nennt die Frist und den Schlüssel.
                (Some(proxy), Failed::HeadLate) => ForwardError::with_diagnostic(
                    UpstreamError::Timeout,
                    diagnostics::response_late(proxy, &request.authority, wait.response),
                ),
                (_, failure) => ForwardError::from(failure.error()),
            })?;
        // Ein `407` auf eine Anfrage in absoluter Form kommt vom Proxy, nicht
        // vom Ziel. Der Agent hat keine Zugangsdaten für ihn und bekommt
        // `502`; der Befund sagt dem Menschen, was fehlt.
        if let (RequestForm::Absolute { .. }, Some(proxy)) = (&form, &via)
            && response.status() == hyper::StatusCode::PROXY_AUTHENTICATION_REQUIRED
        {
            return Err(ForwardError::with_diagnostic(
                UpstreamError::Connect,
                wants_credentials(proxy, &format!("the request to {}", request.authority)),
            ));
        }
        Ok(response)
    }

    /// Sendet die Anfrage über einen bereits offenen Strom (h1).
    ///
    /// `form` sagt, wie die Anfragezeile aussieht; `wait` trennt die Frist
    /// des HTTP-Handschlags von der für den Kopf der Antwort (HUM-511).
    async fn send<S>(
        &self,
        request: &HttpRequest,
        body: Outgoing,
        stream: S,
        form: &RequestForm,
        wait: Wait,
    ) -> Result<Response<Incoming>, Failed>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        let (mut sender, conn) = tokio::time::timeout(
            wait.handshake,
            hyper::client::conn::http1::handshake(TokioIo::new(stream)),
        )
        .await
        .map_err(|_elapsed| UpstreamError::Timeout)?
        .map_err(|_err| UpstreamError::Connect)?;
        let connection = tokio::spawn(async move {
            if let Err(err) = conn.await {
                tracing::debug!(%err, "upstream connection ended");
            }
        });

        let watch = match &body {
            // Hinter einem Entscheidungs-Proxy wartet der Upload nach seinem
            // Ende so lange auf den Kopf der Antwort wie jede Anfrage dort.
            Outgoing::Streamed { watch, .. } => Some(StreamWatch {
                head: wait.response,
                ..watch.clone()
            }),
            Outgoing::Buffered(_) => None,
        };
        let outgoing =
            build_outgoing(request, body, form).map_err(|_err| UpstreamError::Connect)?;
        if let Some(watch) = watch {
            // Ein gestreamter Rumpf kann beliebig groß sein, und das Ziel
            // antwortet meist erst nach dem ganzen Rumpf. Eine Frist über die
            // ganze Spanne wäre eine Grenze der Gesamtdauer des Uploads, und
            // die gibt es nicht (`backlog/CONVENTIONS.md` 4.4). Gewacht wird
            // stattdessen über Fortschritt und, nach dem Ende des Rumpfs, über
            // die Antwort ([`StreamWatch`]).
            let send = sender.send_request(outgoing);
            tokio::pin!(send);
            let mut tick = tokio::time::interval(WATCH_TICK);
            loop {
                tokio::select! {
                    response = &mut send => {
                        let response = response.map_err(|_err| Failed::Other(UpstreamError::Connect))?;
                        // Das Ziel kann antworten, bevor es den Upload gelesen
                        // hat. Dann hält die Verbindung den Rest des Rumpfs
                        // noch, und die Wache läuft weiter, bis er zu Ende ist
                        // oder das Ziel aufhört zu lesen; dann bricht sie die
                        // Verbindung ab, und mit ihr fallen Rumpf und Budget.
                        if !watch.progress.is_ended() {
                            tokio::spawn(watch_after_head(watch, connection));
                        }
                        return Ok(response);
                    }
                    _ = tick.tick() => {
                        if let Some(stall) = watch.stalled() {
                            tracing::debug!(?stall, "the streamed upload stalled");
                            // Die Verbindung zum Ziel wird abgebrochen, nicht
                            // stehen gelassen: Sie hielte sonst den Rest des
                            // Client-Rumpfs und einen Deskriptor, solange das
                            // Ziel schweigt, also unbegrenzt.
                            connection.abort();
                            return Err(match stall {
                                Stall::NoResponseHead => Failed::HeadLate,
                                Stall::NotReading => Failed::Other(UpstreamError::Timeout),
                            });
                        }
                    }
                }
            }
        }
        tokio::time::timeout(wait.response, sender.send_request(outgoing))
            .await
            .map_err(|_elapsed| Failed::HeadLate)?
            .map_err(|_err| Failed::Other(UpstreamError::Connect))
    }
}

/// Wacht über einen gestreamten Upload, dessen Antwort schon da ist
/// (HUM-057): Nimmt das Ziel länger als `limits.body_timeout_secs` kein Stück
/// mehr an, wird die Verbindung abgebrochen. Endet der Rumpf oder die
/// Verbindung, endet auch die Wache.
async fn watch_after_head(watch: StreamWatch, connection: tokio::task::JoinHandle<()>) {
    let mut tick = tokio::time::interval(WATCH_TICK);
    loop {
        tick.tick().await;
        if connection.is_finished() || watch.progress.is_ended() {
            return;
        }
        if watch.stalled() == Some(Stall::NotReading) {
            tracing::debug!("the target stopped reading a streamed upload after its answer");
            connection.abort();
            // Der Befund steht am Fluss, auch wenn dessen Antwort schon durch
            // ist: Ohne ihn sähe die Historie eine vollständige Antwort und
            // keinen Grund, warum der Upload nie ankam.
            if let Some((flow, queue)) = &watch.report {
                queue.publish(FlowEvent::Diagnostic {
                    flow_id: Some(*flow),
                    at: SystemTime::now(),
                    diagnostic: Box::new(crate::limits::upload_stalled(
                        Stall::NotReading,
                        watch.idle,
                        watch.head,
                    )),
                });
            }
            return;
        }
    }
}

/// Wie das Senden scheiterte: der Kopf der Antwort kam nicht in seiner
/// Frist, oder etwas anderes. Nur der erste Fall bekommt über einen
/// Entscheidungs-Proxy einen eigenen Befund (`PROXY_019`).
#[derive(Debug, Clone, Copy)]
enum Failed {
    /// Der Kopf der Antwort blieb aus, auch nach dem Ende eines gestreamten
    /// Uploads.
    HeadLate,
    /// Alles andere.
    Other(UpstreamError),
}

impl Failed {
    /// Was der Automat verbucht.
    const fn error(self) -> UpstreamError {
        match self {
            Self::HeadLate => UpstreamError::Timeout,
            Self::Other(error) => error,
        }
    }
}

impl From<UpstreamError> for Failed {
    fn from(error: UpstreamError) -> Self {
        Self::Other(error)
    }
}

/// Ein offener Weg, bereit zum Senden.
struct Opened {
    stream: Box<dyn AsyncStream>,
    form: RequestForm,
    wait: Wait,
    via: Option<String>,
}

/// Die zwei Fristen des Senders (HUM-511).
#[derive(Debug, Clone, Copy)]
struct Wait {
    /// `limits.header_timeout_secs`: der HTTP-Handschlag.
    handshake: Duration,
    /// Bis zum Kopf der Antwort: direkt dieselbe Grenze, über einen
    /// Entscheidungs-Proxy `egress.proxy_response_timeout_secs`.
    response: Duration,
}

/// Baut die Anfrage, die zum Ziel geht: Origin-Form-URI, Host-Kopf gesetzt,
/// Verbindungs-Kopfzeilen entfernt, Body als bekannter Puffer oder, bei
/// [`Outgoing::Streamed`], als Strom mit der angekündigten Länge.
///
/// Über einen Proxy in absoluter Form (`GET http://AUTHORITY/pfad`, RFC 9112
/// 3.2.2), mit `Host` aus dem Namen und `Proxy-Authorization`, falls der
/// Proxy sie verlangt (HUM-511).
fn build_outgoing(
    request: &HttpRequest,
    body: Outgoing,
    form: &RequestForm,
) -> Result<Request<ResponseBody>, ()> {
    let uri = match form {
        RequestForm::Origin => request.path_and_query.clone(),
        RequestForm::Absolute { authority, .. } => {
            format!("http://{authority}{}", request.path_and_query)
        }
    };
    let mut builder = Request::builder()
        .method(request.method.clone())
        .uri(uri.as_str())
        .version(Version::HTTP_11);

    if let Some(headers) = builder.headers_mut() {
        for (name, value) in &request.headers {
            if is_hop_by_hop(name.as_str()) || name.as_str().eq_ignore_ascii_case("host") {
                continue;
            }
            // `content-length` setzt der Daemon unten selbst, aus der Länge
            // des gepufferten Rumpfs; ein mitgebrachter Wert könnte lügen.
            if name.as_str().eq_ignore_ascii_case("content-length") {
                continue;
            }
            headers.append(name.clone(), value.clone());
        }
        if let Ok(host) = HeaderValue::from_str(&host_header(request)) {
            headers.insert(HeaderName::from_static("host"), host);
        }
        // Erst nach dem Filter oben gesetzt: Eine `Proxy-Authorization` des
        // Agenten fällt dort als Hop-by-Hop-Zeile weg, und nur die des
        // Daemons erreicht den Proxy.
        if let RequestForm::Absolute {
            proxy_authorization: Some(value),
            ..
        } = form
        {
            headers.insert(hyper::header::PROXY_AUTHORIZATION, value.clone());
        }
        // Ausdrücklich gesetzt und nicht hyper überlassen: hyper schreibt für
        // einen leeren `Full`-Rumpf gar keinen `content-length`, auch nicht
        // für einen POST. RFC 9110 8.6 verlangt für eine Methode, die einen
        // Rumpf trägt, `Content-Length: 0` (SHOULD), und HUM-047 legt es für
        // die bearbeitete Anfrage fest. Der Wert steht damit auf dem Draht
        // genau so wie in der Aufzeichnung (`edit::apply_edit`).
        let length = match &body {
            Outgoing::Buffered(bytes) => wire_content_length(request.method.as_str(), bytes.len())
                .map(|n| u64::try_from(n).unwrap_or(u64::MAX)),
            // Die Länge, die der Client angekündigt hat; hyper bricht ab, wenn
            // der Strom davon abweicht. Ohne Ankündigung schreibt hyper
            // `Transfer-Encoding: chunked`.
            Outgoing::Streamed { length, .. } => *length,
        };
        if let Some(length) = length {
            headers.insert(
                HeaderName::from_static("content-length"),
                HeaderValue::from(length),
            );
        }
    }

    let body = match body {
        Outgoing::Buffered(bytes) => body::full(bytes),
        Outgoing::Streamed { body, .. } => body,
    };
    builder.body(body).map_err(|_err| ())
}

/// Der `content-length`, den eine Anfrage mit `body_len` Bytes Rumpf trägt,
/// oder keiner.
///
/// Die Byte-Länge, auch `0` bei leerem Rumpf — außer bei `GET` und `HEAD`
/// ohne Rumpf: `GET / HTTP/1.1` mit `content-length: 0` ist zwar erlaubt,
/// aber manche Server behandeln es als Rumpf-Ankündigung (HUM-047).
///
/// Öffentlich in der Crate, weil [`crate::edit`] denselben Wert in die
/// bearbeitete Anfrage schreibt; zwei Regeln für dieselbe Kopfzeile liefen
/// auseinander, und die Aufzeichnung zeigte eine Anfrage, die so nie
/// hinausging.
pub(crate) fn wire_content_length(method: &str, body_len: usize) -> Option<usize> {
    let bodiless = matches!(method.as_bytes(), b"GET" | b"HEAD");
    (body_len > 0 || !bodiless).then_some(body_len)
}

/// Der Wert des `Host`-Kopfes: Host, und der Port nur, wenn er nicht der
/// Standard des Schemas ist. `IPv6` in eckigen Klammern.
///
/// Öffentlich in der Crate, weil [`crate::edit`] denselben Wert in die
/// bearbeitete Anfrage schreibt. Zwei Stellen, die den Host formatieren,
/// liefen auseinander, und die Aufzeichnung zeigte dann eine Anfrage, die so
/// nie hinausging.
pub(crate) fn host_header(request: &HttpRequest) -> String {
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
