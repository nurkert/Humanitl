//! Der Port [`DaemonApi`] und der generische tonic-Dienst darüber.
//!
//! Der Daemon hat genau eine öffentliche Schnittstelle, und sie hat genau eine
//! Implementierung der Verdrahtung: [`DaemonService`]. Wer den Daemon spielt —
//! der echte Dienst aus HUM-018 oder der Fake aus [`crate::fake`] — schreibt
//! keinen tonic-Code, sondern erfüllt [`DaemonApi`]. Damit sind Token-Prüfung,
//! die Abbildung von [`Diagnostic`] auf [`Status`] und das Verpacken der
//! Ströme an einer Stelle beschrieben und für beide gleich; die Oberfläche
//! kann den Unterschied nicht sehen.
//!
//! Die Grenze verläuft bei den Wire-Typen: [`DaemonApi`] nimmt und liefert
//! Protobuf-Nachrichten aus [`crate::v1`]. Nur dort, wo der Kern eine
//! Invariante hält, stehen Kern-Typen in der Signatur ([`FlowId`],
//! [`Diagnostic`]).

use core::pin::Pin;
use core::task::{Context, Poll};
use std::sync::Arc;

use bytes::Bytes;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, DiagnosticCode, FlowId};
use prost::Message as _;
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Code, Request, Response, Status};

use crate::convert::diagnostic_to_proto;
use crate::v1;

/// Ein Strom, der irgendwo auf dem Heap liegt und über Aufrufgrenzen reist.
///
/// Immer `'static` und `Send`: ein Strom aus [`DaemonApi`] wird an tonic
/// weitergereicht und dort auf einem beliebigen Worker abgespult.
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

/// Alles, was der Daemon kann, ohne ein Wort über gRPC zu verlieren.
///
/// Die Methoden entsprechen eins zu eins den RPCs aus
/// `proto/humanitl/v1/humanitl.proto`. Fehler sind [`Diagnostic`], nie
/// [`Status`]: die Abbildung auf einen gRPC-Code gehört in die Verdrahtung,
/// nicht in die Fachlichkeit.
#[tonic::async_trait]
pub trait DaemonApi: Send + Sync + 'static {
    /// Version und Fähigkeiten des Daemons.
    async fn info(&self) -> v1::Info;

    /// Der Ereignisstrom. Ein zu langsamer Zuhörer bekommt
    /// [`v1::flow_event::Event::Lagged`], nie einen Abbruch.
    fn subscribe(&self, request: v1::SubscribeRequest) -> BoxStream<v1::FlowEvent>;

    /// Eine Seite der Flow-Historie.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn Filter oder Cursor nicht lesbar sind.
    async fn list_flows(&self, request: v1::ListFlowsRequest) -> Result<v1::FlowPage, Diagnostic>;

    /// Alles, was über einen Flow bekannt ist.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `IPC_003`, wenn es den Flow nicht gibt.
    async fn get_flow(&self, id: FlowId) -> Result<v1::FlowDetail, Diagnostic>;

    /// Der Inhalt eines Bodys, in Stücken. Das letzte Stück trägt `last`.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `IPC_005`, wenn die Prüfsumme keine 32 Bytes hat.
    /// Der Port braucht diesen Fehlerpfad, weil der echte Dienst ihn hat: ohne
    /// ihn müsste der Fake eine unlesbare Anfrage als leeren Body beantworten,
    /// und das sähe für den Client aus wie ein Body, den es gibt.
    fn get_body(&self, body: v1::BodyRef) -> Result<BoxStream<v1::BodyChunk>, Diagnostic>;

    /// Entscheidet einen oder mehrere gehaltene Flows.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn die Anfrage als Ganzes nicht gilt. Ein Fehler an
    /// einem einzelnen Flow steht dagegen in `DecideResult.diagnostic`.
    async fn decide(&self, request: v1::DecideRequest) -> Result<v1::DecideResponse, Diagnostic>;

    /// Liest oder ändert den Regelsatz.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn die Operation fehlt oder eine Regel ungültig ist.
    async fn rules(&self, request: v1::RulesRequest) -> Result<v1::RulesResponse, Diagnostic>;

    /// Startet, stoppt oder beobachtet die Sandbox.
    fn sandbox(&self, request: v1::SandboxRequest) -> BoxStream<v1::SandboxEvent>;

    /// Verbindet ein Terminal mit der Sandbox.
    fn terminal(&self, input: BoxStream<v1::TerminalInput>) -> BoxStream<v1::TerminalOutput>;

    /// Prüft oder exportiert das Audit-Log.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn der Export nicht geschrieben werden kann.
    async fn audit(&self, request: v1::AuditRequest) -> Result<v1::AuditResponse, Diagnostic>;

    /// Die effektive Konfiguration samt Herkunft je Feld.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn die Konfiguration nicht gelesen werden kann.
    async fn get_config(
        &self,
        request: v1::GetConfigRequest,
    ) -> Result<v1::ConfigSnapshot, Diagnostic>;

    /// Setzt genau einen Konfigurationsschlüssel.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `CONFIG_002`, wenn der Schlüssel nicht im Schema steht.
    async fn set_config(
        &self,
        request: v1::SetConfigRequest,
    ) -> Result<v1::ConfigSnapshot, Diagnostic>;

    /// Der Selbsttest der Installation.
    ///
    /// Nicht in der Auflistung von HUM-005, aber Teil des Vertrags: der
    /// generierte tonic-Trait verlangt jeden RPC.
    async fn doctor(&self) -> v1::DoctorReport;

    /// Die LAN-Suche nach LLM-Servern.
    fn discover_llm(&self, request: v1::DiscoverRequest) -> BoxStream<v1::DiscoverResult>;

    /// Prüft einen einzelnen LLM-Endpunkt, host-seitig und nur lesend.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `LLM_001`, wenn nichts antwortet, `LLM_002`, wenn der
    /// Server eine Anmeldung verlangt, `LLM_003`, wenn die Adresse gar keine
    /// HTTP-Adresse ist (HUM-039).
    async fn probe_llm(
        &self,
        request: v1::ProbeLlmRequest,
    ) -> Result<v1::ProbeLlmResponse, Diagnostic>;

    /// Was ein Sandbox-Lauf im Projektverzeichnis hinterlassen hat (HUM-043).
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `IPC_005`, wenn die Kennung keine ist,
    /// `RECORDER_001`, wenn dieser Daemon ohne Aufzeichnung läuft, und
    /// `SANDBOX_027`, wenn zu dem Lauf keine Zusammenfassung vorliegt.
    async fn get_session_summary(
        &self,
        request: v1::SessionSummaryRef,
    ) -> Result<v1::SessionSummary, Diagnostic>;
}

/// Der tonic-Dienst über einem beliebigen [`DaemonApi`].
///
/// Hier steht alles, was für jeden RPC gleich ist: das Token aus
/// `x-humanitl-token` prüfen, [`Diagnostic`] in [`Status`] übersetzen, Ströme
/// in `Result` verpacken.
pub struct DaemonService<T> {
    api: Arc<T>,
    token: String,
}

impl<T> DaemonService<T> {
    /// Baut den Dienst über einer Implementierung und dem erwarteten Token.
    ///
    /// Das Token ist der Inhalt von `$XDG_RUNTIME_DIR/humanitl/token`. Ein
    /// leeres Token gibt es nicht: dann könnte jeder Prozess mitlesen.
    #[must_use]
    pub fn new(api: Arc<T>, token: impl Into<String>) -> Self {
        Self {
            api,
            token: token.into(),
        }
    }

    /// Die Implementierung, über der dieser Dienst liegt.
    #[must_use]
    pub fn api(&self) -> &Arc<T> {
        &self.api
    }

    /// Prüft das Token einer Anfrage.
    ///
    /// # Errors
    ///
    /// [`Status`] mit [`Code::Unauthenticated`] und `IPC_001` in den Details,
    /// wenn der Metadata-Schlüssel fehlt oder nicht passt.
    fn check_token<R>(&self, request: &Request<R>) -> Result<(), Status> {
        crate::auth::check_token(request.metadata(), &self.token)
    }
}

/// Der gRPC-Code, mit dem ein Befund beim Client ankommt.
///
/// Die Zuordnung ist bewusst klein: der Code aus dem Register trägt die
/// Bedeutung, der gRPC-Code sagt dem Client nur, ob er es erneut versuchen
/// darf.
#[must_use]
pub fn grpc_code(code: DiagnosticCode) -> Code {
    match code.as_str() {
        "IPC_001" => Code::Unauthenticated,
        "IPC_002" | "IPC_004" | "IPC_005" | "CONFIG_002" | "CONFIG_003" => Code::InvalidArgument,
        // Eine Regel, die der Client so geschickt hat, wie die Engine sie
        // nicht annimmt: dasselbe Urteil wie bei einer unlesbaren Anfrage.
        "RULES_001" | "RULES_003" | "RULES_005" | "RULES_006" | "RULES_007" => {
            Code::InvalidArgument
        }
        // Der Zustand verbietet es, nicht das Argument: die Regel ist
        // mitgeliefert, der Flow wartet nicht mehr.
        "IPC_003" | "RULES_010" => Code::FailedPrecondition,
        "DAEMON_001" => Code::Unavailable,
        _ => Code::Internal,
    }
}

/// Übersetzt einen Befund in einen gRPC-Fehler.
///
/// Der Befund reist vollständig als Protobuf in den Details mit, damit die
/// Oberfläche `code`, `why` und `fix` zeigen kann statt einer Textzeile.
#[must_use]
pub fn diagnostic_to_status(diagnostic: &Diagnostic) -> Status {
    let details = Bytes::from(diagnostic_to_proto(diagnostic).encode_to_vec());
    Status::with_details(grpc_code(diagnostic.code), diagnostic.to_string(), details)
}

/// Übersetzt den Befund eines `GetFlow` in seinen gRPC-Status.
///
/// Eine einzige Ausnahme von [`grpc_code`], und sie steht hier, an der Stelle,
/// an der ohnehin übersetzt wird: `IPC_003` heißt überall sonst „der Flow
/// wartet nicht mehr" und damit `FailedPrecondition`. Aus `GetFlow` heißt es
/// etwas anderes — den Flow gibt es nicht —, und das ist `NOT_FOUND`. Ein
/// eigener Diagnostic-Code dafür wäre ein neuer Eintrag im Register für einen
/// Unterschied, den nur ein einziger RPC macht; [`grpc_code`] zu ändern
/// verschöbe jeden anderen Aufruf mit.
///
/// Der Befund reist wie immer vollständig in den Details mit; ein nackter
/// String stünde dem Client nur als Textzeile zur Verfügung.
#[must_use]
pub fn get_flow_status(diagnostic: &Diagnostic) -> Status {
    as_not_found(diagnostic, codes::IPC_003)
}

/// Übersetzt den Befund eines `GetSessionSummary` in seinen gRPC-Status.
///
/// Dieselbe Ausnahme aus demselben Grund wie [`get_flow_status`]:
/// `SANDBOX_027` heißt „zu diesem Lauf gibt es keine Zusammenfassung", und das
/// ist `NOT_FOUND`. Jeder andere Befund — eine Aufzeichnung, die nicht liest,
/// eine Zeile, die sich nicht lesen lässt — geht den gewöhnlichen Weg.
#[must_use]
pub fn missing_status(diagnostic: &Diagnostic) -> Status {
    as_not_found(diagnostic, codes::SANDBOX_027)
}

/// `NOT_FOUND`, wenn der Befund genau `code` ist, sonst [`grpc_code`].
///
/// Die beiden Ausnahmen stehen hier zusammen und nicht zweimal: Ein zweiter
/// Aufbau desselben `Status` liefe irgendwann auseinander.
fn as_not_found(diagnostic: &Diagnostic, code: DiagnosticCode) -> Status {
    if diagnostic.code != code {
        return diagnostic_to_status(diagnostic);
    }
    let details = Bytes::from(diagnostic_to_proto(diagnostic).encode_to_vec());
    Status::with_details(Code::NotFound, diagnostic.to_string(), details)
}

/// Liest einen Befund aus den Details eines gRPC-Fehlers zurück.
///
/// Gedacht für Clients und Tests. Ein Fehler ohne Details oder mit fremden
/// Bytes liefert `None`.
#[must_use]
pub fn diagnostic_from_status(status: &Status) -> Option<v1::Diagnostic> {
    if status.details().is_empty() {
        return None;
    }
    v1::Diagnostic::decode(status.details()).ok()
}

/// Hängt an jedes Element eines Stroms ein `Ok`.
///
/// tonic will `Result<T, Status>`; [`DaemonApi`] liefert `T`, weil ein
/// Ereignisstrom keinen Fehlerpfad hat: was schiefgeht, ist selbst ein
/// Ereignis.
struct OkStream<T> {
    inner: BoxStream<T>,
}

impl<T> Stream for OkStream<T> {
    type Item = Result<T, Status>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.inner.as_mut().poll_next(cx).map(|item| item.map(Ok))
    }
}

/// Verpackt einen Strom für tonic.
fn ok_stream<T: Send + 'static>(inner: BoxStream<T>) -> BoxStream<Result<T, Status>> {
    Box::pin(OkStream { inner })
}

/// Reicht einen eingehenden tonic-Strom als reinen Wert-Strom weiter.
///
/// Ein Fehler auf dem Eingangsstrom beendet ihn; für das Terminal heißt das:
/// der Client ist weg, also schließt die Verbindung.
pub(crate) fn plain_stream<T: Send + 'static>(mut inner: tonic::Streaming<T>) -> BoxStream<T> {
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(async move {
        while let Ok(Some(item)) = inner.message().await {
            if tx.send(item).await.is_err() {
                break;
            }
        }
    });
    Box::pin(ReceiverStream::new(rx))
}

#[tonic::async_trait]
impl<T: DaemonApi> v1::humanitl_server::Humanitl for DaemonService<T> {
    type SubscribeStream = BoxStream<Result<v1::FlowEvent, Status>>;
    type GetBodyStream = BoxStream<Result<v1::BodyChunk, Status>>;
    type SandboxStream = BoxStream<Result<v1::SandboxEvent, Status>>;
    type TerminalStream = BoxStream<Result<v1::TerminalOutput, Status>>;
    type DiscoverLlmStream = BoxStream<Result<v1::DiscoverResult, Status>>;

    async fn get_info(&self, request: Request<()>) -> Result<Response<v1::Info>, Status> {
        self.check_token(&request)?;
        Ok(Response::new(self.api.info().await))
    }

    async fn subscribe(
        &self,
        request: Request<v1::SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        self.check_token(&request)?;
        Ok(Response::new(ok_stream(
            self.api.subscribe(request.into_inner()),
        )))
    }

    async fn list_flows(
        &self,
        request: Request<v1::ListFlowsRequest>,
    ) -> Result<Response<v1::FlowPage>, Status> {
        self.check_token(&request)?;
        self.api
            .list_flows(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn get_flow(
        &self,
        request: Request<v1::FlowRef>,
    ) -> Result<Response<v1::FlowDetail>, Status> {
        self.check_token(&request)?;
        let reference = request.into_inner();
        // `IPC_004`, nicht `IPC_003`: eine unlesbare Id ist eine unlesbare
        // Anfrage und kein Zustand eines Flows (CONVENTIONS 4.12). Der echte
        // Dienst sagt dasselbe, weil beide durch [`crate::validate`] gehen.
        let id = crate::validate::flow_id(&reference.flow_id)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        self.api
            .get_flow(id)
            .await
            .map(Response::new)
            .map_err(|diagnostic| get_flow_status(&diagnostic))
    }

    async fn get_body(
        &self,
        request: Request<v1::BodyRef>,
    ) -> Result<Response<Self::GetBodyStream>, Status> {
        self.check_token(&request)?;
        let stream = self
            .api
            .get_body(request.into_inner())
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(ok_stream(stream)))
    }

    async fn decide(
        &self,
        request: Request<v1::DecideRequest>,
    ) -> Result<Response<v1::DecideResponse>, Status> {
        self.check_token(&request)?;
        self.api
            .decide(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn rules(
        &self,
        request: Request<v1::RulesRequest>,
    ) -> Result<Response<v1::RulesResponse>, Status> {
        self.check_token(&request)?;
        self.api
            .rules(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn sandbox(
        &self,
        request: Request<v1::SandboxRequest>,
    ) -> Result<Response<Self::SandboxStream>, Status> {
        self.check_token(&request)?;
        Ok(Response::new(ok_stream(
            self.api.sandbox(request.into_inner()),
        )))
    }

    async fn terminal(
        &self,
        request: Request<tonic::Streaming<v1::TerminalInput>>,
    ) -> Result<Response<Self::TerminalStream>, Status> {
        self.check_token(&request)?;
        let input = plain_stream(request.into_inner());
        Ok(Response::new(ok_stream(self.api.terminal(input))))
    }

    async fn audit(
        &self,
        request: Request<v1::AuditRequest>,
    ) -> Result<Response<v1::AuditResponse>, Status> {
        self.check_token(&request)?;
        self.api
            .audit(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn get_config(
        &self,
        request: Request<v1::GetConfigRequest>,
    ) -> Result<Response<v1::ConfigSnapshot>, Status> {
        self.check_token(&request)?;
        self.api
            .get_config(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn set_config(
        &self,
        request: Request<v1::SetConfigRequest>,
    ) -> Result<Response<v1::ConfigSnapshot>, Status> {
        self.check_token(&request)?;
        self.api
            .set_config(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn doctor(&self, request: Request<()>) -> Result<Response<v1::DoctorReport>, Status> {
        self.check_token(&request)?;
        Ok(Response::new(self.api.doctor().await))
    }

    async fn discover_llm(
        &self,
        request: Request<v1::DiscoverRequest>,
    ) -> Result<Response<Self::DiscoverLlmStream>, Status> {
        self.check_token(&request)?;
        Ok(Response::new(ok_stream(
            self.api.discover_llm(request.into_inner()),
        )))
    }

    async fn probe_llm(
        &self,
        request: Request<v1::ProbeLlmRequest>,
    ) -> Result<Response<v1::ProbeLlmResponse>, Status> {
        self.check_token(&request)?;
        self.api
            .probe_llm(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn get_session_summary(
        &self,
        request: Request<v1::SessionSummaryRef>,
    ) -> Result<Response<v1::SessionSummary>, Status> {
        self.check_token(&request)?;
        self.api
            .get_session_summary(request.into_inner())
            .await
            .map(Response::new)
            // Derselbe `NOT_FOUND` wie beim echten Dienst: Ein Lauf, zu dem es
            // keine Zusammenfassung gibt, ist nichts anderes, nur weil die
            // Antwort aus dem Fake kommt.
            .map_err(|diagnostic| missing_status(&diagnostic))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes;
    use humanitl_core::{Diagnostic, Severity};
    use tonic::Code;

    use super::{diagnostic_from_status, diagnostic_to_status, grpc_code};

    #[test]
    fn known_codes_map_to_grpc_codes() {
        assert_eq!(grpc_code(codes::IPC_001), Code::Unauthenticated);
        assert_eq!(grpc_code(codes::IPC_002), Code::InvalidArgument);
        assert_eq!(grpc_code(codes::IPC_004), Code::InvalidArgument);
        assert_eq!(grpc_code(codes::IPC_003), Code::FailedPrecondition);
        assert_eq!(grpc_code(codes::TLS_001), Code::Internal);
    }

    #[test]
    fn status_carries_the_diagnostic_in_its_details() {
        let diagnostic = Diagnostic::builder(codes::IPC_002, Severity::Error)
            .why("two flow ids")
            .build();
        let status = diagnostic_to_status(&diagnostic);
        assert_eq!(status.code(), Code::InvalidArgument);
        let decoded = diagnostic_from_status(&status).expect("details must decode");
        assert_eq!(decoded.code, "IPC_002");
        assert_eq!(decoded.why, "two flow ids");
        assert_eq!(decoded.title, "AllowEdited nur für genau einen Flow");
    }
}
