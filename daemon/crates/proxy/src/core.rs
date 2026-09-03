//! Der Proxy-Kern: ein Session-Socket, ein Accept-Loop je Sitzung.
//!
//! [`ProxyCore`] bindet pro Sitzung einen [`SessionSocket`] und nimmt darauf
//! Verbindungen entgegen; jede Verbindung bedient [`serve_connection`] mit dem
//! Handler der Sitzung.
//!
//! Es gibt keinen Loopback-TCP-Port auf dem Host. Der Spike aus
//! `backlog/CONVENTIONS.md` 4.10 (HUM-015 Schritt 0) hat ergeben: hudsucker
//! 0.24.1 nimmt in `ProxyBuilder::with_listener` ausschließlich einen
//! `tokio::net::TcpListener`, kein Trait und keinen `UnixListener`; sein
//! Client (`hyper-util` mit `GaiResolver`) und seine CA (`RcgenAuthority`)
//! wären ohnehin durch den [`Egress`](crate::egress::Egress)-Port, den
//! [`Resolver`](crate::resolver::Resolver)-Port und die eigene
//! [`LeafCache`](crate::ca::LeafCache) ersetzt worden. Übrig geblieben wäre
//! nur seine Accept-Schleife, und die ist mit hyper 1 und `tokio-rustls` kurz
//! genug, um sie hier selbst zu schreiben. Darum lauscht der Proxy direkt auf
//! dem Unix-Socket, und hudsucker ist keine Abhängigkeit.
//!
//! Das Verdrahten der Ports (Warteschlange, Pipeline, Egress, CA) ist Sache des
//! Aufrufers (`humanitld`, HUM-018); der Kern nimmt einen fertigen
//! [`FlowHandler`] und startet ihn auf einem Socket.

use std::path::{Path, PathBuf};

use dashmap::DashMap;
use humanitl_core::{Diagnostic, SessionId};
use tokio::task::JoinHandle;

use crate::handler::{FlowHandler, serve_connection};
use crate::listener::SessionSocket;
use crate::pipeline::ConnMeta;

/// Alle laufenden Sitzungen eines Daemons.
///
/// Beim Fallenlassen enden alle Accept-Loops und ihre Socket-Dateien
/// verschwinden; offene Verbindungen laufen in ihren eigenen Tasks weiter,
/// bis der Client sie schließt.
#[derive(Debug, Default)]
pub struct ProxyCore {
    sessions: DashMap<SessionId, SessionProxy>,
}

/// Eine laufende Sitzung: ihr Socket-Pfad und der Accept-Loop.
#[derive(Debug)]
struct SessionProxy {
    socket_path: PathBuf,
    accept: JoinHandle<()>,
}

impl ProxyCore {
    /// Ein leerer Kern ohne Sitzungen.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Startet eine Sitzung: bindet `socket_path` (üblicherweise
    /// `Paths::proxy_socket()`) und nimmt darauf Verbindungen entgegen, jede
    /// mit `handler`.
    ///
    /// Liefert den Pfad der Socket-Datei, den der Launcher in die Sandbox
    /// einhängt. `meta` beschreibt die Verbindungen dieser Sitzung (Sitzungs-Id,
    /// kein Tunnel, kein TLS bis zum `CONNECT`, `allow_private` als
    /// Test-Hook). Muss innerhalb einer Tokio-Laufzeit aufgerufen werden.
    ///
    /// # Errors
    ///
    /// Wie [`SessionSocket::bind`] ([`DAEMON_004`](humanitl_core::diagnostics::codes::DAEMON_004)).
    pub fn start_session(
        &self,
        session: SessionId,
        socket_path: &Path,
        handler: FlowHandler,
        meta: ConnMeta,
    ) -> Result<PathBuf, Diagnostic> {
        let socket = SessionSocket::bind(socket_path)?;
        let socket_path = socket.path().to_owned();
        let accept = tokio::spawn(async move {
            accept_loop(socket, handler, meta).await;
        });
        if let Some(previous) = self.sessions.insert(
            session,
            SessionProxy {
                socket_path: socket_path.clone(),
                accept,
            },
        ) {
            previous.accept.abort();
        }
        Ok(socket_path)
    }

    /// Der Socket-Pfad einer laufenden Sitzung.
    #[must_use]
    pub fn socket_path(&self, session: SessionId) -> Option<PathBuf> {
        self.sessions
            .get(&session)
            .map(|entry| entry.socket_path.clone())
    }

    /// Hält eine Sitzung an: der Accept-Loop endet, der Socket verschwindet.
    pub fn stop_session(&self, session: SessionId) {
        if let Some((_, proxy)) = self.sessions.remove(&session) {
            proxy.accept.abort();
        }
    }

    /// Wie viele Sitzungen gerade laufen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Wahr, wenn keine Sitzung läuft.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Drop for ProxyCore {
    fn drop(&mut self) {
        for entry in &self.sessions {
            entry.accept.abort();
        }
    }
}

/// Nimmt Verbindungen an, bis der Socket schließt oder die Task abgebrochen
/// wird. Der Socket lebt in dieser Task; endet sie, räumt sein `Drop` die Datei
/// weg.
async fn accept_loop(socket: SessionSocket, handler: FlowHandler, meta: ConnMeta) {
    loop {
        match socket.listener().accept().await {
            Ok((stream, _addr)) => {
                let handler = handler.clone();
                let meta = meta.clone();
                tokio::spawn(async move {
                    serve_connection(handler, stream, meta).await;
                });
            }
            Err(err) => {
                tracing::debug!(%err, "accept on the session socket failed; stopping");
                break;
            }
        }
    }
}
