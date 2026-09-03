//! Der Client zum Daemon, für CLI, Oberfläche und Tests (HUM-018 Schritt 5).
//!
//! Der Daemon lauscht auf einem Unix-Socket, nicht auf einem Port. tonic 0.14
//! kennt dafür `unix://<pfad>` in der Endpunkt-URI und baut den passenden
//! Connector selbst; die Hinweise älterer Fassungen, eine Platzhalter-URI mit
//! eigenem Connector zu bauen, sind damit hinfällig (`backlog/sprint-1.md`,
//! Fallstricke von HUM-018).
//!
//! Das Token aus `$XDG_RUNTIME_DIR/humanitl/token` hängt als Interceptor an
//! jedem Aufruf. Ohne Token antwortet der Daemon auf jede RPC mit
//! `Unauthenticated`, auch auf `GetInfo`.

use std::path::Path;

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};

use crate::auth::read_token;
use crate::{TOKEN_METADATA_KEY, v1};

/// Der Client, den [`connect`] liefert.
pub type Client = v1::humanitl_client::HumanitlClient<InterceptedService<Channel, TokenSender>>;

/// Hängt das Sitzungs-Token an jeden ausgehenden Aufruf.
#[derive(Debug, Clone)]
pub struct TokenSender {
    token: MetadataValue<Ascii>,
}

impl Interceptor for TokenSender {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        request
            .metadata_mut()
            .insert(TOKEN_METADATA_KEY, self.token.clone());
        Ok(request)
    }
}

/// Verbindet sich mit dem Daemon an den Pfaden aus `paths`.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn die Token-Datei fehlt oder der Socket
/// nicht antwortet: dann läuft kein Daemon.
pub async fn connect(paths: &Paths) -> Result<Client, Diagnostic> {
    let socket = paths.daemon_socket();
    let token = read_token(&paths.token_path())?;
    connect_at(&socket, &token).await
}

/// Wie [`connect`], aber mit ausdrücklichem Socket und Token.
///
/// Gedacht für Tests und für `--socket`.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn der Socket nicht antwortet, und mit
/// `IPC_001`, wenn das Token keine gültige Kopfzeile ergibt.
pub async fn connect_at(socket: &Path, token: &str) -> Result<Client, Diagnostic> {
    let token = MetadataValue::try_from(token).map_err(|error| {
        Diagnostic::builder(codes::IPC_001, Severity::Blocking)
            .why(format!(
                "the session token is not a valid header value: {error}"
            ))
            .build()
    })?;
    let channel = channel(socket).await?;
    Ok(v1::humanitl_client::HumanitlClient::with_interceptor(
        channel,
        TokenSender { token },
    ))
}

/// Der Kanal zum Socket, ohne Token.
///
/// Damit lässt sich prüfen, ob überhaupt jemand auf dem Socket antwortet
/// (`humanitl doctor`, HUM-075). Zum Arbeiten taugt er nicht: der Daemon
/// beantwortet jede RPC ohne gültiges Token mit `Unauthenticated`, auch
/// `GetInfo`.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn der Socket nicht antwortet.
pub async fn channel(socket: &Path) -> Result<Channel, Diagnostic> {
    let uri = format!("unix://{}", socket.display());
    let unreachable = |why: String| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(why)
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build()
    };
    Endpoint::from_shared(uri.clone())
        .map_err(|error| unreachable(format!("{uri} is not a usable endpoint: {error}")))?
        .connect()
        .await
        .map_err(|error| {
            unreachable(format!(
                "cannot reach the daemon on {}: {error}",
                socket.display()
            ))
        })
}
