//! TLS zum Client: terminieren und dabei die SNI festhalten (HUM-023).
//!
//! Der Proxy stellt für das Ziel eines `CONNECT` ein Leaf aus der eigenen CA
//! aus und terminiert TLS damit. Der Name, den der Client im `ClientHello`
//! nennt, ist die zweite Aussage über das Ziel — neben dem CONNECT-Ziel und
//! dem `Host` der Anfrage — und muss zu den anderen beiden passen, sonst ist
//! die Verbindung ein Fronting-Versuch (ADR-007).
//!
//! Die Spezifikation beschreibt dafür einen `SniRecordingResolver`, der
//! hudsuckers `ResolvesServerCert` umhüllt. hudsucker ist seit HUM-015 keine
//! Abhängigkeit mehr: Der Proxy hat seine eigene Accept-Schleife auf
//! `tokio-rustls`, baut die [`ServerConfig`] selbst und kann rustls deshalb
//! direkt fragen: [`rustls::ServerConnection::server_name`] liefert genau den
//! Namen aus dem `ClientHello`, schon normalisiert und von rustls geprüft. Ein
//! eigener Resolver müsste die Konfiguration je Verbindung kopieren und
//! denselben Wert von Hand einsammeln. Der Vergleich selbst liegt unverändert
//! in [`check_authority`](crate::connect::check_authority).
//!
//! Ein Client, der eine falsche SNI schickt, kommt in aller Regel gar nicht so
//! weit: Das Leaf gilt für das CONNECT-Ziel, und ein Client, der Zertifikate
//! prüft, lehnt es ab. Genau darauf darf sich der Proxy aber nicht verlassen —
//! der Client in der Sandbox ist der Prozess, gegen den die Prüfung gerichtet
//! ist, und ein Client, der nicht prüft, führt den Handschlag zu Ende. Deshalb
//! wird serverseitig verglichen.

use std::io;
use std::sync::Arc;

use humanitl_core::HostName;
use rustls::ServerConfig;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

/// Terminiert TLS auf `io` und liefert den Strom samt der SNI des Clients.
///
/// `None` als Name heißt: Der Client hat keine `server_name`-Erweiterung
/// geschickt. Das ist für ein IP-Ziel richtig und für einen DNS-Namen ein
/// Widerspruch, den [`check_authority`](crate::connect::check_authority)
/// bewertet — hier wird nur festgehalten, was ankam.
///
/// # Errors
///
/// Der Fehler des Handschlags, unverändert: Der Client hat unsere CA
/// abgelehnt, eine unbrauchbare SNI geschickt oder die Verbindung fallen
/// gelassen.
pub async fn accept<I>(
    config: Arc<ServerConfig>,
    io: I,
) -> io::Result<(TlsStream<I>, Option<HostName>)>
where
    I: AsyncRead + AsyncWrite + Unpin,
{
    let stream = TlsAcceptor::from(config).accept(io).await?;
    let sni = captured_sni(stream.get_ref().1);
    Ok((stream, sni))
}

/// Der Name aus dem `ClientHello` dieser Verbindung, normalisiert.
///
/// rustls liefert ihn schon klein geschrieben und ohne abschließenden Punkt;
/// [`HostName::parse`] bringt ihn in dieselbe Form, in der das CONNECT-Ziel
/// und der `Host`-Kopf stehen, damit der Vergleich nicht an einer Schreibweise
/// vorbeigeht. Was sich nicht als Host lesen lässt, gilt als keine SNI: dann
/// bleibt es bei der strengeren Behandlung (kein Name, also kein Beleg).
#[must_use]
pub fn captured_sni(connection: &rustls::ServerConnection) -> Option<HostName> {
    HostName::parse(connection.server_name()?).ok()
}
