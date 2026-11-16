//! Der Egress-Port: der einzige Weg des Proxys zu einem Ziel im Netz.
//!
//! Jede Upstream-Verbindung läuft über [`Egress::open`] oder
//! [`Egress::connect`] (ADR-017, `docs/ARCHITECTURE.md` 2). Der MVP-Adapter
//! [`Direct`] baut eine TCP-Verbindung zu einer bereits aufgelösten und
//! angehefteten IP auf. Der zweite Adapter [`HttpProxy`] (ADR-0022, HUM-511)
//! schickt jede Verbindung über einen Entscheidungs-Proxy des Nutzers.
//! `tools/check-deps.sh` erzwingt, dass `TcpStream::connect` nirgends
//! außerhalb von `egress/` steht.
//!
//! Der Port bekommt die IP als Parameter, weil die Auflösung erst nach der
//! Entscheidung geschieht und ausschließlich über den [`crate::resolver::Resolver`]-Port
//! (`crate::resolver`). Der Egress erfindet keine Adresse und liest kein DNS;
//! er verbindet genau dorthin, wohin der Handler ihn schickt. Wer überhaupt
//! auflöst, sagt der Egress selbst ([`Egress::resolution`]): Hinter einem
//! Entscheidungs-Proxy mit `egress.proxy_resolve = "proxy"` löst Humanitl gar
//! nicht auf.
//!
//! Seit HUM-511 sagt der Port dem Sender auch, **wie** er schreibt
//! ([`Route`]): direkt in Origin-Form (`GET /pfad`), über einen Proxy eine
//! `http`-Anfrage in absoluter Form (`GET http://host/pfad`) mit
//! `Proxy-Authorization`, und mit welcher TLS-Konfiguration er den Tunnel
//! öffnet. Die CA des Entscheidungs-Proxys steht nur in der Konfiguration
//! dieses Wegs, nie in der gemeinsamen der direkten Verbindungen.

use std::net::IpAddr;
use std::time::Duration;

use async_trait::async_trait;
use humanitl_core::{Authority, Diagnostic};
use hyper::header::HeaderValue;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::upstream::ClientTls;

pub mod check;
pub mod diagnostics;
pub mod direct;
pub mod http_proxy;
pub mod screen;

pub use self::diagnostics::wants_credentials;
pub use self::direct::Direct;
pub use self::http_proxy::{
    HttpProxy, ProxyEndpoint, Wiring, decision_proxy, from_config, is_local_name,
};

/// Ein bidirektionaler Byte-Strom zu einem Ziel.
///
/// Die Vereinigung der Eigenschaften, die der Handler von einer
/// Upstream-Verbindung braucht: lesen, schreiben, über Task-Grenzen schieben,
/// an Ort und Stelle bewegen. Jeder Typ, der sie erfüllt, ist ein Strom; ein
/// TLS-Strom über einer TCP-Verbindung ebenso wie die nackte Verbindung.
pub trait AsyncStream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> AsyncStream for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

/// Wer den Namen eines Ziels auflöst (ADR-0006, ADR-0022).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Humanitl, nach der Entscheidung, über den
    /// [`Resolver`](crate::resolver::Resolver)-Port, mit der Prüfung auf
    /// private Adressen. Der Egress bekommt die geprüfte Adresse.
    Humanitl,
    /// Der Entscheidungs-Proxy hinter Humanitl (`egress.proxy_resolve =
    /// "proxy"`). Humanitl stellt keine DNS-Anfrage; der Egress bekommt den
    /// Namen und, nur bei einem IP-Literal, die Adresse.
    Proxy,
}

/// Was der Sender über ein Ziel wissen muss, bevor der Egress verbindet.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    /// Das Ziel, für das entschieden wurde.
    pub authority: &'a Authority,
    /// Ob über der Verbindung TLS gesprochen wird (`https`).
    pub secure: bool,
    /// Die angeheftete Adresse; `None`, wenn niemand in Humanitl auflöst
    /// ([`Resolution::Proxy`]) und das Ziel ein Name ist.
    pub resolved: Option<IpAddr>,
}

/// In welcher Form die Anfrage auf die Leitung geht (RFC 9112 3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestForm {
    /// `GET /pfad`: direkt zum Ziel, oder durch einen schon offenen Tunnel.
    Origin,
    /// `GET http://AUTHORITY/pfad` an einen Proxy, mit `Host` aus dem Namen.
    Absolute {
        /// Die Authority in der Anfragezeile: der Name, oder die
        /// angeheftete Adresse, wenn Humanitl aufgelöst hat.
        authority: String,
        /// `Proxy-Authorization` für diesen Proxy, falls er Zugangsdaten
        /// verlangt.
        proxy_authorization: Option<HeaderValue>,
    },
}

/// Ein offener Weg zum Ziel und wie der Sender ihn benutzt.
pub struct Route {
    /// Der Strom: zum Ziel, durch einen Tunnel, oder zum Proxy selbst.
    pub stream: Box<dyn AsyncStream>,
    /// Wie die Anfrage geschrieben wird.
    pub form: RequestForm,
    /// Die TLS-Konfiguration dieses Wegs; `None` heißt: die gemeinsame der
    /// direkten Verbindungen.
    pub tls: Option<ClientTls>,
    /// Wie lange auf den Kopf der Antwort gewartet wird; `None` heißt:
    /// `limits.header_timeout_secs` wie direkt.
    pub response_timeout: Option<Duration>,
    /// `host:port` des Proxys, über den der Weg läuft, für Befunde.
    pub via: Option<String>,
}

impl Route {
    /// Ein direkter Weg: Origin-Form, gemeinsame TLS-Konfiguration, die
    /// gewohnten Fristen.
    #[must_use]
    pub fn direct(stream: Box<dyn AsyncStream>) -> Self {
        Self {
            stream,
            form: RequestForm::Origin,
            tls: None,
            response_timeout: None,
            via: None,
        }
    }
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("form", &self.form)
            .field("own_tls", &self.tls.is_some())
            .field("response_timeout", &self.response_timeout)
            .field("via", &self.via)
            .finish_non_exhaustive()
    }
}

/// Warum [`Egress::open`] keinen Weg geöffnet hat.
///
/// Die Unterscheidung trägt der Port, nicht der Code des Befunds: Ein
/// direktes Verbinden, das scheitert, ist für den Agenten und den Menschen
/// `upstream_connect` wie bisher; der Befund eines Entscheidungs-Proxys
/// dagegen geht bis an den Fluss, weil er sagt, welche Einstellung fehlt
/// (HUM-511).
#[derive(Debug, Clone)]
pub enum OpenError {
    /// Der direkte Weg kam nicht zustande; der Befund bleibt im Log.
    Direct(Diagnostic),
    /// Der Weg über einen Entscheidungs-Proxy scheiterte; der Befund gehört
    /// an den Fluss.
    Route(Diagnostic),
}

impl OpenError {
    /// Der Befund, gleich welcher Art.
    #[must_use]
    pub const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::Direct(diagnostic) | Self::Route(diagnostic) => diagnostic,
        }
    }
}

/// Der Weg des Proxys zu einem Ziel.
///
/// Zwei Implementierungen: [`Direct`] und [`HttpProxy`] (ADR-017, ADR-0022).
/// Der Handler kennt keine von beiden; er fragt den Port, wer auflöst, und
/// schreibt die Anfrage in der Form, die [`Egress::open`] ihm sagt.
#[async_trait]
pub trait Egress: Send + Sync {
    /// Wer den Namen eines Ziels auflöst. Direkt immer Humanitl.
    fn resolution(&self) -> Resolution {
        Resolution::Humanitl
    }

    /// Ob hinter diesem Weg ein zweiter Proxy steht, der Namen auf seinem
    /// eigenen Rechner auflösen könnte. Dann lehnt der Weiterleiter lokale
    /// Namen ab, bevor irgendetwas aufgelöst oder gesendet wird, gleich wer
    /// auflöst (ADR-0022). Direkt nie.
    fn behind_second_proxy(&self) -> bool {
        false
    }

    /// Öffnet den Weg zu `target` und sagt, wie darüber gesendet wird.
    ///
    /// Die Vorgabe ist der direkte Weg über [`Egress::connect`]. Das ist der
    /// Weg, den der Weiterleiter nimmt.
    ///
    /// # Errors
    ///
    /// [`OpenError::Direct`] mit dem Befund von [`Egress::connect`]; über
    /// einen Entscheidungs-Proxy [`OpenError::Route`] mit `PROXY_012` bis
    /// `PROXY_014` oder `PROXY_017`.
    async fn open(&self, target: Target<'_>) -> Result<Route, OpenError> {
        let stream = self
            .connect(target.authority, target.resolved)
            .await
            .map_err(OpenError::Direct)?;
        Ok(Route::direct(stream))
    }

    /// Öffnet eine rohe TCP-Verbindung zu `authority`.
    ///
    /// Nur für Adapter, deren Weg ein schlichter Byte-Strom zum Ziel ist
    /// ([`Direct`]). Ein Adapter, dessen Weg eine eigene Anfrageform und
    /// eigenes TLS verlangt ([`HttpProxy`]), lehnt diesen Aufruf ab: Wer ihn
    /// nähme, schriebe die Anfrage in der falschen Form und prüfte das
    /// Zertifikat gegen die falschen Anker. Der Weiterleiter ruft immer
    /// [`Egress::open`].
    ///
    /// `resolved` ist die angeheftete Zieladresse. Der Handler löst den Namen
    /// vorher über den [`Resolver`](crate::resolver::Resolver)-Port auf und
    /// prüft die Adresse; der Egress verbindet nur noch. Ohne Adresse
    /// (`None`) gibt es kein Ziel, und der Aufruf scheitert.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`] mit [`PROXY_003`](humanitl_core::diagnostics::codes::PROXY_003),
    /// wenn keine Adresse angeheftet ist oder die Verbindung nicht zustande
    /// kommt. Der Handler bildet das auf einen [`UpstreamError`] und eine
    /// `502`-Antwort ab.
    ///
    /// [`UpstreamError`]: humanitl_core::UpstreamError
    async fn connect(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic>;
}
