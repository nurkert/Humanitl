//! Der direkte Egress: eine TCP-Verbindung zur angehefteten IP.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use async_trait::async_trait;
use humanitl_core::diagnostics::codes::PROXY_003;
use humanitl_core::{Authority, Diagnostic, Severity};
use tokio::net::TcpStream;

use super::{AsyncStream, Egress};

/// Verbindet direkt per TCP zur angehefteten IP (MVP, ADR-017).
///
/// Kein Namensdienst, kein Pooling über Ziele hinweg: eine Verbindung je
/// Aufruf, zur genau übergebenen Adresse. Das Zeitlimit stammt aus
/// `limits.connect_timeout_secs`.
#[derive(Debug, Clone)]
pub struct Direct {
    connect_timeout: Duration,
}

impl Direct {
    /// Ein direkter Egress mit dem Verbindungs-Zeitlimit aus der Konfiguration.
    #[must_use]
    pub const fn new(connect_timeout: Duration) -> Self {
        Self { connect_timeout }
    }
}

impl Default for Direct {
    /// Zehn Sekunden, der Vorgabewert von `limits.connect_timeout_secs`.
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

#[async_trait]
impl Egress for Direct {
    async fn connect(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic> {
        let ip = resolved.ok_or_else(|| {
            Diagnostic::builder(PROXY_003, Severity::Error)
                .why(format!(
                    "no address pinned for {authority}; the resolver must run before egress"
                ))
                .build()
        })?;
        let addr = SocketAddr::new(ip, authority.port);
        let stream = tokio::time::timeout(self.connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_elapsed| {
                Diagnostic::builder(PROXY_003, Severity::Error)
                    .why(format!(
                        "connecting to {addr} timed out after {}s",
                        self.connect_timeout.as_secs()
                    ))
                    .build()
            })?
            .map_err(|err| {
                Diagnostic::builder(PROXY_003, Severity::Error)
                    .why(format!("connecting to {addr} failed: {err}"))
                    .build()
            })?;
        // Nagle aus: der Proxy schreibt oft kleine, vollständige Anfragen und
        // will sie nicht verzögert sehen.
        let _ = stream.set_nodelay(true);
        Ok(Box::new(stream))
    }
}
