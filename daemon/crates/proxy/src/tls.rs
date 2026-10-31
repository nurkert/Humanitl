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

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use humanitl_core::HostName;
use rustls::ServerConfig;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

/// Ob das erste Byte eines Tunnels eine HTTP-Anfrage beginnt (HUM-364).
///
/// Methoden sind Großbuchstaben, ein TLS-Handschlag beginnt mit `0x16`. Alles
/// andere bleibt beim Handschlag, damit dessen Scheitern sichtbar wird wie
/// bisher.
#[must_use]
pub const fn starts_http(first: u8) -> bool {
    first.is_ascii_uppercase()
}

/// Ein Strom, von dem der erste Block schon gelesen wurde und der ihn beim
/// nächsten Lesen zurückgibt (HUM-364).
#[derive(Debug)]
pub struct Rewound<I> {
    head: Vec<u8>,
    inner: I,
}

impl<I> Rewound<I> {
    /// Hängt [`head`](Self) wieder vor [`inner`](Self).
    pub const fn new(head: Vec<u8>, inner: I) -> Self {
        Self { head, inner }
    }
}

impl<I: AsyncRead + Unpin> AsyncRead for Rewound<I> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.head.is_empty() && buf.remaining() > 0 {
            let count = self.head.len().min(buf.remaining());
            buf.put_slice(&self.head[..count]);
            self.head.drain(..count);
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<I: AsyncWrite + Unpin> AsyncWrite for Rewound<I> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    // Vektorielles Schreiben reicht durch: Der letzte Schreibversuch nach einem
    // gescheiterten Handschlag trägt ServerHello und Alert in zwei Puffern, und
    // die Vorgabe des Traits schriebe nur den ersten.
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

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
