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
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use humanitl_core::diagnostics::codes::PROXY_010;
use humanitl_core::{Diagnostic, FixAction, FlowEvent, SessionId, Severity};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use crate::connect::ConnectionContext;
use crate::handler::{FlowHandler, PressureWatch, serve_connection};
use crate::listener::SessionSocket;

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
        meta: ConnectionContext,
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

/// So viele Ablehnungen schreibt der Accept-Loop höchstens gleichzeitig
/// zurück.
///
/// Die Ablehnung ist selbst eine Verbindung, die jemand offen halten kann: Wer
/// verbindet und nie liest, lässt den Schreibvorgang stehen. Deshalb bekommt
/// sie ein eigenes, kleines Kontingent und eine eigene Frist; ist beides
/// erschöpft, wird die Verbindung ohne Antwort geschlossen. Eine Grenze, deren
/// Durchsetzung selbst unbegrenzt Ressourcen bindet, wäre keine.
const REFUSAL_SLOTS: usize = 8;

/// So lange darf das Zurückschreiben einer Ablehnung dauern.
///
/// Keine Einstellung: Es geht um rund hundert Bytes auf einen Unix-Socket, und
/// wer die nicht abnimmt, hat die Antwort ohnehin nicht gelesen.
const REFUSAL_WRITE_TIMEOUT: Duration = Duration::from_secs(1);

/// Der Rumpf der Ablehnung. Er nennt den Grund und den Schlüssel, wie es
/// [`block_response`](humanitl_core::block_response) für eine geblockte
/// Anfrage tut.
const REFUSAL_BODY: &str = "Refused by Humanitl.\nreason: max_client_connections\n";

/// So viel liest eine Ablehnung höchstens weg, bevor sie den Socket fallen
/// lässt.
///
/// Ein Anfragekopf passt darin um ein Vielfaches; wer mehr schickt, obwohl er
/// gerade eine `503` mit `Connection: close` bekommen hat, bekommt seinen
/// `RST`.
const REFUSAL_DRAIN_CAP_BYTES: usize = 64 * 1024;

/// Nimmt Verbindungen an, bis der Socket schließt oder die Task abgebrochen
/// wird. Der Socket lebt in dieser Task; endet sie, räumt sein `Drop` die Datei
/// weg.
///
/// # Die Obergrenze gleichzeitiger Verbindungen (HUM-120)
///
/// `limits.max_client_connections` begrenzt, wie viele Verbindungen dieser
/// Sitzung gleichzeitig bedient werden. Sie gehört zu den drei Uhren auf den
/// Spannen der Verbindung und nicht neben sie: Eine Uhr je Spanne schließt die
/// **Dauer**, nicht die **Menge**. Ein Prozess in der Sandbox, der in einer
/// dieser Spannen stehenbleibt, bindet je Verbindung eine Tokio-Aufgabe und
/// einen Dateideskriptor; ohne Obergrenze bindet derselbe Angriff dieselben
/// Ressourcen, nur kürzer und dafür öfter.
///
/// Über der Grenze wird **abgelehnt**, nicht angenommen und liegen gelassen:
/// Der Client bekommt `503` mit `Connection: close`, damit ein Werkzeug in der
/// Sandbox eine Antwort sieht statt eines stillen Abbruchs, den es als Fehler
/// des Netzes deutet. Der Mensch bekommt [`PROXY_010`] in den Ereignisstrom —
/// zusammengefasst über [`PressureWatch`], weil ein Befund je abgelehnter
/// Verbindung dem Angreifer einen Hebel gegen die Oberfläche gäbe.
async fn accept_loop(socket: SessionSocket, handler: FlowHandler, meta: ConnectionContext) {
    let limit = handler.limits().max_client_connections;
    let slots = Arc::new(Semaphore::new(usize::try_from(limit).unwrap_or(usize::MAX)));
    let refusals = Arc::new(Semaphore::new(REFUSAL_SLOTS));
    let pressure = PressureWatch::new();
    loop {
        match socket.listener().accept().await {
            Ok((stream, _addr)) => {
                let Ok(permit) = Arc::clone(&slots).try_acquire_owned() else {
                    refuse(&handler, &pressure, &refusals, stream, limit);
                    continue;
                };
                let handler = handler.clone();
                let meta = meta.clone();
                // Der Platz wandert als `Arc` in die Verbindung hinein, nicht
                // in diese Aufgabe: Ein `CONNECT` steigt zum Tunnel auf, und
                // `serve_connection` kehrt dabei zurück, während der Tunnel
                // weiterlebt. Der Platz wird deshalb erst frei, wenn die letzte
                // Hälfte der Verbindung fällt — sonst gälte die Grenze nur für
                // einfaches HTTP und ausgerechnet nicht für TLS-Tunnel.
                let slot = Some(Arc::new(permit));
                tokio::spawn(async move {
                    serve_connection(handler, stream, meta, slot).await;
                });
            }
            Err(err) => {
                tracing::debug!(%err, "accept on the session socket failed; stopping");
                break;
            }
        }
    }
}

/// Lehnt eine Verbindung über der Grenze ab: `503` zurück, Befund in den
/// Ereignisstrom, Verbindung zu.
fn refuse(
    handler: &FlowHandler,
    pressure: &PressureWatch,
    refusals: &Arc<Semaphore>,
    stream: UnixStream,
    limit: u32,
) {
    tracing::warn!(
        limit,
        "refusing a connection from the sandbox; limits.max_client_connections is reached"
    );
    if let Some(since_last) = pressure.hit() {
        handler.queue().publish(FlowEvent::Diagnostic {
            flow_id: None,
            at: SystemTime::now(),
            diagnostic: Box::new(connection_limit_reached(limit, since_last)),
        });
    }
    let Ok(slot) = Arc::clone(refusals).try_acquire_owned() else {
        // Auch die Ablehnungen sind ausgereizt: Die Verbindung wird ohne
        // Antwort geschlossen. Das ist die schlechtere, aber begrenzte
        // Auskunft; der Befund oben sagt trotzdem, was geschehen ist.
        tracing::debug!("no slot left to write a refusal; closing the connection unanswered");
        // `stream` fällt hier und schließt damit die Verbindung.
        return;
    };
    tokio::spawn(async move {
        let _ = tokio::time::timeout(REFUSAL_WRITE_TIMEOUT, write_refusal(stream)).await;
        drop(slot);
    });
}

/// Schreibt die `503`-Antwort, schließt die Schreibrichtung und liest weg, was
/// die Gegenseite schon geschickt hat.
///
/// **Das Leeren gehört dazu und ist keine Kosmetik.** Ein Client, der zugleich
/// mit dem Verbinden seine Anfrage schickt, hat ihre Bytes im Empfangspuffer
/// des Sockets liegen. Fällt der Socket mit ungelesenen Bytes, schickt der
/// Kern ein `RST` statt eines ordentlichen Schlusses — und mit dem `RST` geht
/// die Antwort verloren, die gerade erklären sollte, warum abgelehnt wurde.
/// Der Client sähe einen Verbindungsabbruch statt des Grundes, also genau das,
/// was diese Meldung verhindern soll.
///
/// Gelesen wird bis zum Dateiende der Gegenseite oder bis
/// [`REFUSAL_DRAIN_CAP_BYTES`]; die Frist des Aufrufers deckelt das Ganze
/// zusätzlich.
async fn write_refusal(mut stream: UnixStream) {
    let response = format!(
        "HTTP/1.1 503 Service Unavailable\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\r\n{REFUSAL_BODY}",
        len = REFUSAL_BODY.len(),
    );
    if let Err(err) = stream.write_all(response.as_bytes()).await {
        tracing::debug!(%err, "the refused connection did not take its answer");
        return;
    }
    if let Err(err) = stream.shutdown().await {
        tracing::debug!(%err, "the refused connection could not be half-closed");
        return;
    }
    let mut scratch = [0u8; 4096];
    let mut drained = 0_usize;
    while drained < REFUSAL_DRAIN_CAP_BYTES {
        match stream.read(&mut scratch).await {
            Ok(0) | Err(_) => return,
            Ok(read) => drained = drained.saturating_add(read),
        }
    }
    tracing::debug!(
        drained,
        "the refused connection kept sending; closing without reading the rest"
    );
}

/// Der Befund [`PROXY_010`] zur erreichten Verbindungsgrenze (HUM-120).
///
/// `since_last` ist die Zahl der Ablehnungen seit der letzten Meldung. Der
/// `fix` schlägt den doppelten Wert vor: Wer wirklich mehr gleichzeitige
/// Verbindungen braucht — ein Agent, der viele Downloads parallel fährt —,
/// braucht in der Regel nicht viel mehr, und ein Vorschlag ohne Grenze wäre die
/// Empfehlung, die Grenze aufzugeben.
#[must_use]
pub fn connection_limit_reached(limit: u32, since_last: u64) -> Diagnostic {
    let why = format!(
        "the sandbox already holds {limit} connections to the proxy \
         (limits.max_client_connections), so {since_last} further connections were refused with \
         503 and closed instead of being accepted and left open. Nothing left the host and no \
         request was lost: a refused connection never carried one. A process that opens many \
         connections and then falls silent inside one of them is what this limit is for; the \
         per-span timeouts bound how long each one may stay, this one bounds how many there may \
         be."
    );
    Diagnostic::builder(PROXY_010, Severity::Warning)
        .why(why)
        .fix(FixAction::ChangeSetting {
            key: "limits.max_client_connections".to_owned(),
            value: limit.saturating_mul(2).to_string(),
        })
        .build()
}
