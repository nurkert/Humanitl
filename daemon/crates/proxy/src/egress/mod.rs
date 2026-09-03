//! Der Egress-Port: der einzige Weg des Proxys zu einem Ziel im Netz.
//!
//! Jede Upstream-Verbindung läuft über [`Egress::connect`] (ADR-017,
//! `docs/ARCHITECTURE.md` 2). Der MVP-Adapter [`Direct`] baut eine
//! TCP-Verbindung zu einer bereits aufgelösten und angehefteten IP auf; spätere
//! Adapter (`HttpProxy`, `Socks5h`) ersetzen nur diese Datei, ohne dass der
//! Handler es merkt. `tools/check-deps.sh` erzwingt, dass `TcpStream::connect`
//! nirgends außerhalb von `egress/` steht.
//!
//! Der Port bekommt die IP als Parameter, weil die Auflösung erst nach der
//! Entscheidung geschieht und ausschließlich über den [`crate::resolver::Resolver`]-Port
//! (`crate::resolver`). Der Egress erfindet keine Adresse und liest kein DNS;
//! er verbindet genau dorthin, wohin der Handler ihn schickt.

use std::net::IpAddr;

use async_trait::async_trait;
use humanitl_core::{Authority, Diagnostic};
use tokio::io::{AsyncRead, AsyncWrite};

pub mod direct;

pub use self::direct::Direct;

/// Ein bidirektionaler Byte-Strom zu einem Ziel.
///
/// Die Vereinigung der Eigenschaften, die der Handler von einer
/// Upstream-Verbindung braucht: lesen, schreiben, über Task-Grenzen schieben,
/// an Ort und Stelle bewegen. Jeder Typ, der sie erfüllt, ist ein Strom; ein
/// TLS-Strom über einer TCP-Verbindung ebenso wie die nackte Verbindung.
pub trait AsyncStream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> AsyncStream for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

/// Der Weg des Proxys zu einem Ziel.
///
/// Genau eine Implementierung im MVP ([`Direct`]); der Trait existiert, weil
/// die spätere Tor- und Proxy-Kette (ADR-017) ihn ersetzt, ohne den Handler zu
/// berühren.
#[async_trait]
pub trait Egress: Send + Sync {
    /// Öffnet eine Verbindung zu `authority`.
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
