//! Der echte gRPC-Dienst über Registry und Halte-Warteschlange (HUM-018).
//!
//! [`IpcServer`] ist die Oberfläche, die Sprint 1 braucht: `GetInfo`,
//! `Subscribe`, `Decide`, `ListFlows`. Jede weitere RPC des Vertrags antwortet
//! mit `UNIMPLEMENTED` und nennt das Issue, das sie bringt — ein leerer
//! Erfolg wäre schlimmer als ein klarer Fehler, weil ein Client ihn für eine
//! Antwort hielte.
//!
//! # Warum hier tonic steht und nicht [`DaemonApi`](crate::DaemonApi)
//!
//! Der Fake benutzt den generischen Dienst [`DaemonService`](crate::DaemonService)
//! über dem Port [`DaemonApi`](crate::DaemonApi). Dieser Dienst kann das nicht:
//! der Port hat für Ströme und für `GetInfo`/`Doctor` keinen Fehlerweg, und
//! genau den braucht `UNIMPLEMENTED`. Der echte Dienst erfüllt deshalb den
//! erzeugten tonic-Trait selbst. Die Token-Prüfung bleibt trotzdem an einer
//! Stelle: sie liegt in [`crate::auth`] und hängt hier als Interceptor vor
//! jeder RPC, damit kein Endpunkt vergessen werden kann.
//!
//! # Zustand
//!
//! Alles im Speicher, nichts persistent (der Recorder kommt mit HUM-026). Der
//! Dienst hält keinen eigenen Zustand: er liest die
//! [`FlowRegistry`] und entscheidet über die
//! [`HoldQueue`]. Beide teilen sich einen einzigen
//! Rundfunk-Kanal; der Dienst öffnet keinen zweiten, sondern nimmt die
//! Registry der Warteschlange ([`HoldQueue::registry`]).

use std::fs::Permissions;
use std::future::Future;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use humanitl_config::Config;
use humanitl_core::diagnostics::codes;
use humanitl_core::{
    BlockReason, Decision, DecisionSource, Diagnostic, FlowEvent, FlowId, SessionId, Severity,
};
use humanitl_proxy::hold::NotHeld;
use humanitl_proxy::{FlowFilter, FlowRegistry, HoldQueue};
use tokio::net::UnixListener;
use tokio::sync::oneshot;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::server_stub::{BoxStream, diagnostic_to_status};
use crate::{PROTO_MAJOR, PROTO_MINOR, auth, convert, v1};

/// Was dieser Daemon in M1 kann.
///
/// Die Liste ist eine Zusage, keine Absichtserklärung: `sandbox.bwrap` und
/// `findings.regex` fehlen, solange die zugehörigen RPCs `UNIMPLEMENTED`
/// liefern (HUM-040, HUM-025). Ein Client, der danach schaltet, soll nicht in
/// einen Fehler laufen.
pub const CAPABILITIES: &[&str] = &["hold", "proxy.h1"];

/// Vorgabe für `ListFlowsRequest.limit`, wie im Vertrag beschrieben.
pub const DEFAULT_PAGE_LIMIT: usize = 200;

/// Obergrenze für `ListFlowsRequest.limit`, wie im Vertrag beschrieben.
pub const MAX_PAGE_LIMIT: u32 = 1000;

/// So lange darf eine offene Verbindung den Abbau nach dem Signal aufhalten.
///
/// Nach `SIGTERM` nimmt der Dienst nichts Neues mehr an und lässt laufende
/// Aufrufe zu Ende gehen. Ohne Frist hinge er an einem Client, der seine
/// Verbindung offen lässt — `systemctl stop humanitld` würde dann warten, bis
/// die Oberfläche sich schließt. Nach dieser Frist endet der Dienst, und
/// Socket und Token verschwinden in jedem Fall.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Der gRPC-Dienst des echten Daemons.
///
/// Billig zu bauen und an eine Laufzeit zu hängen; alles Geteilte liegt hinter
/// `Arc`.
#[derive(Debug)]
pub struct IpcServer {
    registry: Arc<FlowRegistry>,
    queue: Arc<HoldQueue>,
    info: v1::Info,
    body_cap_bytes: u64,
}

impl IpcServer {
    /// Der Dienst über einer Warteschlange und ihrer Registry.
    ///
    /// Die Registry kommt aus [`HoldQueue::registry`]: Warteschlange und
    /// Verzeichnis teilen sich einen Rundfunk-Kanal, und ein zweiter Kanal
    /// würde die Reihenfolge der Ereignisse je Flow zerreißen (HUM-016).
    #[must_use]
    pub fn new(queue: Arc<HoldQueue>, config: &Config, session: Option<SessionId>) -> Self {
        let registry = Arc::clone(queue.registry());
        Self {
            registry,
            queue,
            info: v1::Info {
                daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
                proto_major: PROTO_MAJOR,
                proto_minor: PROTO_MINOR,
                capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
                session_id: session.map(|id| id.to_string()).unwrap_or_default(),
            },
            body_cap_bytes: config.limits.hold_body_cap_bytes,
        }
    }

    /// Die Selbstauskunft, die `GetInfo` liefert.
    #[must_use]
    pub const fn info(&self) -> &v1::Info {
        &self.info
    }

    /// Der Ereignisstrom eines Abonnenten.
    ///
    /// Ein zu langsamer Zuhörer verliert die ältesten Ereignisse; der
    /// Rundfunk meldet das als `Lagged`. Daraus wird ein reguläres
    /// [`FlowEvent::Lagged`] — dasselbe, was
    /// [`humanitl_proxy::hold::next_event`] einem Zuhörer ohne Strom liefert.
    /// Der Strom endet dabei nicht: der Client lädt mit `ListFlows` nach.
    fn event_stream(
        &self,
        request: &v1::SubscribeRequest,
    ) -> BoxStream<Result<v1::FlowEvent, Status>> {
        let registry = Arc::clone(&self.registry);
        let live = BroadcastStream::new(self.registry.subscribe()).map(move |item| {
            let event = match item {
                Ok(event) => event,
                Err(BroadcastStreamRecvError::Lagged(n)) => FlowEvent::Lagged { n },
            };
            Ok(convert::flow_event_to_proto(&event, &registry))
        });
        Box::pin(tokio_stream::iter(self.backlog(request)).chain(live))
    }

    /// Was ein Abonnent vor dem Strom nachgeliefert bekommt.
    ///
    /// `since_flow_id` heißt „ich kenne alles bis hierher". Für jeden jüngeren
    /// Flow kommt ein `Received`; den Rest holt der Client über `ListFlows`.
    /// Genau so verhält sich der Fake, damit die Oberfläche keinen Unterschied
    /// sieht.
    fn backlog(&self, request: &v1::SubscribeRequest) -> Vec<Result<v1::FlowEvent, Status>> {
        if request.since_flow_id.is_empty() {
            return Vec::new();
        }
        let Ok(since) = FlowId::parse(&request.since_flow_id) else {
            return Vec::new();
        };
        self.summaries()
            .into_iter()
            .filter(|summary| FlowId::parse(&summary.flow_id).is_ok_and(|id| id > since))
            .map(|summary| {
                Ok(v1::FlowEvent {
                    at: summary.received_at,
                    event: Some(v1::flow_event::Event::Received(v1::flow_event::Received {
                        summary: Some(summary),
                        domain: None,
                    })),
                })
            })
            .collect()
    }

    /// Alle bekannten Flows als Zeilen, nach Ankunft aufsteigend.
    ///
    /// Die Registry sortiert für ihre eigene Sicht die wartenden Flows nach
    /// Frist nach vorn (HUM-016). Die Liste des Vertrags ist dagegen nach
    /// Ankunft geordnet, weil `cursor` und `since_flow_id` auf Flow-Ids
    /// zeigen; [`FlowId`] ist ein UUID der Fassung 7 und damit selbst die Zeitachse.
    fn summaries(&self) -> Vec<v1::FlowSummary> {
        let mut rows = self.registry.list(&FlowFilter::default());
        rows.sort_unstable_by_key(|row| row.id);
        rows.into_iter()
            .filter_map(|row| self.registry.get(row.id))
            .map(|record| convert::record_to_summary(&record))
            .collect()
    }

    /// Eine Seite der Flow-Historie.
    fn page(&self, request: &v1::ListFlowsRequest) -> v1::FlowPage {
        let since = FlowId::parse(&request.since_flow_id).ok();
        let cursor = FlowId::parse(&request.cursor).ok();
        // Der Cursor zeigt auf das letzte gelieferte Element; die nächste Seite
        // liegt in Sortierrichtung dahinter, bei absteigender Reihenfolge also
        // davor.
        let descending = request.order_by.contains("desc");
        let mut flows: Vec<v1::FlowSummary> = self
            .summaries()
            .into_iter()
            .filter(|summary| request.include_passthrough || !summary.passthrough)
            .filter(|summary| convert::matches_filter(summary, &request.filter))
            .filter(|summary| convert::after(summary, since))
            .filter(|summary| {
                if descending {
                    convert::before(summary, cursor)
                } else {
                    convert::after(summary, cursor)
                }
            })
            .collect();
        if descending {
            flows.reverse();
        }

        let total = u64::try_from(flows.len()).unwrap_or(u64::MAX);
        let limit = match request.limit {
            0 => DEFAULT_PAGE_LIMIT,
            other => usize::try_from(other.min(MAX_PAGE_LIMIT)).unwrap_or(DEFAULT_PAGE_LIMIT),
        };
        let next_cursor = if flows.len() > limit {
            flows
                .get(limit - 1)
                .map(|summary| summary.flow_id.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        flows.truncate(limit);
        v1::FlowPage {
            flows,
            next_cursor,
            total,
        }
    }

    /// Liest die Entscheidung aus der Anfrage.
    ///
    /// Fail closed: eine Anfrage ohne `decision` wird abgelehnt, nicht zu
    /// `Allow` ergänzt. Eine bearbeitete Anfrage, die sich nicht lesen lässt
    /// oder deren Body über `limits.hold_body_cap_bytes` liegt, wird ebenfalls
    /// abgelehnt — beides wäre sonst ein Weiterleiten von etwas, das der
    /// Mensch so nie gesehen hat (`backlog/CONVENTIONS.md` 4.11).
    fn decision_of(&self, request: &v1::DecideRequest) -> Result<Decision, Diagnostic> {
        match &request.decision {
            Some(v1::decide_request::Decision::Allow(())) => Ok(Decision::Allow),
            Some(v1::decide_request::Decision::Block(block)) => {
                // Die Notiz erreicht den Agenten im 403-Body und im Header
                // `X-Humanitl-Note`; sie wird deshalb gesäubert (HUM-072).
                let note = humanitl_core::block::sanitize_note(&block.note);
                Ok(Decision::Block {
                    reason: BlockReason::User,
                    note: (!note.is_empty()).then_some(note),
                })
            }
            Some(v1::decide_request::Decision::AllowEdited(edited)) => {
                let size = u64::try_from(edited.body.len()).unwrap_or(u64::MAX);
                if size > self.body_cap_bytes {
                    return Err(bad_request(format!(
                        "the edited body is {size} bytes, over limits.hold_body_cap_bytes ({})",
                        self.body_cap_bytes
                    )));
                }
                let request = convert::request_from_proto(edited).map_err(|error| {
                    bad_request(format!("the edited request is not readable: {error}"))
                })?;
                Ok(Decision::AllowEdited {
                    request: Box::new(request),
                })
            }
            None => Err(bad_request(
                "decide came without a decision; a missing decision is never an allow".to_owned(),
            )),
        }
    }

    /// Entscheidet genau einen Flow.
    ///
    /// Der Befund kommt zusätzlich zum Ergebnis zurück, weil `Decide`
    /// ihn braucht, falls die Anfrage als Ganzes nichts bewirkt hat.
    fn decide_one(
        &self,
        text: &str,
        decision: &Decision,
    ) -> (v1::DecideResult, Option<Diagnostic>) {
        let Ok(id) = FlowId::parse(text) else {
            return refused(text, bad_request(format!("{text} is not a flow id")));
        };
        match self
            .queue
            .decide_as(id, decision.clone(), DecisionSource::User)
        {
            Ok(()) => (
                v1::DecideResult {
                    flow_id: text.to_owned(),
                    applied: true,
                    diagnostic: None,
                },
                None,
            ),
            Err(error) => refused(text, not_held(&error)),
        }
    }
}

/// Ein Befund für eine Anfrage, die so nicht gilt (`InvalidArgument`).
///
/// Das Register kennt im Bereich `ipc` bisher nur `IPC_002` für eine ungültige
/// `Decide`-Anfrage; sein Titel nennt den häufigsten Fall, der Grund nennt den
/// vorliegenden.
fn bad_request(why: String) -> Diagnostic {
    Diagnostic::builder(codes::IPC_002, Severity::Error)
        .why(why)
        .build()
}

/// Ein Befund für einen Flow, der nicht mehr wartet (`FailedPrecondition`).
fn not_held(error: &NotHeld) -> Diagnostic {
    Diagnostic::builder(codes::IPC_003, Severity::Error)
        .why(error.to_string())
        .build()
}

/// Ein abgelehntes Einzelergebnis einer Entscheidung, samt seinem Befund.
fn refused(flow_id: &str, diagnostic: Diagnostic) -> (v1::DecideResult, Option<Diagnostic>) {
    let result = v1::DecideResult {
        flow_id: flow_id.to_owned(),
        applied: false,
        diagnostic: Some(convert::diagnostic_to_proto(&diagnostic)),
    };
    (result, Some(diagnostic))
}

/// Der Fehler einer RPC, die es noch nicht gibt.
///
/// Kein [`Diagnostic`]: das hier ist kein Fehlschlag der Anfrage, sondern der
/// Stand des Vertrags. Die Meldung nennt das Issue, damit klar ist, worauf
/// man wartet.
fn unimplemented(rpc: &str, arrives: &str) -> Status {
    Status::unimplemented(format!("{rpc} arrives in {arrives}"))
}

#[tonic::async_trait]
impl v1::humanitl_server::Humanitl for IpcServer {
    type SubscribeStream = BoxStream<Result<v1::FlowEvent, Status>>;
    type GetBodyStream = BoxStream<Result<v1::BodyChunk, Status>>;
    type SandboxStream = BoxStream<Result<v1::SandboxEvent, Status>>;
    type TerminalStream = BoxStream<Result<v1::TerminalOutput, Status>>;
    type DiscoverLlmStream = BoxStream<Result<v1::DiscoverResult, Status>>;

    async fn get_info(&self, _request: Request<()>) -> Result<Response<v1::Info>, Status> {
        Ok(Response::new(self.info.clone()))
    }

    async fn subscribe(
        &self,
        request: Request<v1::SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        Ok(Response::new(self.event_stream(&request.into_inner())))
    }

    async fn list_flows(
        &self,
        request: Request<v1::ListFlowsRequest>,
    ) -> Result<Response<v1::FlowPage>, Status> {
        Ok(Response::new(self.page(&request.into_inner())))
    }

    /// Entscheidet einen oder mehrere wartende Flows.
    ///
    /// Ein Stapel entscheidet, soweit er kann: jedes Ergebnis trägt seinen
    /// eigenen Befund. Hat die Anfrage aber gar nichts bewirkt — der häufigste
    /// Fall ist die einzelne Id eines Flows, der nicht mehr wartet —, dann ist
    /// das ein Fehler der Anfrage und kein Erfolg mit leerem Inhalt: der
    /// Aufruf endet mit dem Befund des ersten Flows, bei `IPC_003` also mit
    /// `FailedPrecondition`.
    async fn decide(
        &self,
        request: Request<v1::DecideRequest>,
    ) -> Result<Response<v1::DecideResponse>, Status> {
        let request = request.into_inner();
        if request.flow_ids.is_empty() {
            return Err(diagnostic_to_status(&bad_request(
                "decide came without a flow id".to_owned(),
            )));
        }
        let decision = self
            .decision_of(&request)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let count = request.flow_ids.len();
        if matches!(decision, Decision::AllowEdited { .. }) && count > 1 {
            // Eine bearbeitete Anfrage gilt genau einem Flow. Der Vertrag
            // verlangt hier ausdrücklich ein Ergebnis je Flow mit `IPC_002`
            // statt eines Fehlers für den ganzen Aufruf
            // (`proto/humanitl/v1/humanitl.proto`, `Decide`); entschieden wird
            // dabei nichts.
            let results = request
                .flow_ids
                .iter()
                .map(|text| {
                    refused(
                        text,
                        bad_request(format!("allow_edited came with {count} flow ids")),
                    )
                    .0
                })
                .collect();
            return Ok(Response::new(v1::DecideResponse {
                results,
                created_rule_id: String::new(),
                created_rule: None,
            }));
        }

        let (results, refusals): (Vec<v1::DecideResult>, Vec<Option<Diagnostic>>) = request
            .flow_ids
            .iter()
            .map(|text| self.decide_one(text, &decision))
            .unzip();

        if results.iter().all(|result| !result.applied) {
            let first = refusals
                .into_iter()
                .flatten()
                .next()
                .unwrap_or_else(|| not_held(&NotHeld::Unknown { id: FlowId::nil() }));
            return Err(diagnostic_to_status(&first));
        }

        // `remember` wird angenommen und noch nicht ausgewertet: der Regelsatz
        // kommt mit HUM-027. Eine Regel stillschweigend zu verwerfen wäre
        // falsch, sie hier zu erfinden ebenso; der Client sieht an dem leeren
        // `created_rule`, dass keine entstanden ist.
        Ok(Response::new(v1::DecideResponse {
            results,
            created_rule_id: String::new(),
            created_rule: None,
        }))
    }

    async fn get_flow(
        &self,
        _request: Request<v1::FlowRef>,
    ) -> Result<Response<v1::FlowDetail>, Status> {
        Err(unimplemented("GetFlow", "HUM-026 with the recorder"))
    }

    async fn get_body(
        &self,
        _request: Request<v1::BodyRef>,
    ) -> Result<Response<Self::GetBodyStream>, Status> {
        Err(unimplemented("GetBody", "HUM-026 with the recorder"))
    }

    async fn rules(
        &self,
        _request: Request<v1::RulesRequest>,
    ) -> Result<Response<v1::RulesResponse>, Status> {
        Err(unimplemented("Rules", "HUM-027 with the rules engine"))
    }

    async fn sandbox(
        &self,
        _request: Request<v1::SandboxRequest>,
    ) -> Result<Response<Self::SandboxStream>, Status> {
        Err(unimplemented("Sandbox", "sprint 3 with the sandbox panel"))
    }

    async fn terminal(
        &self,
        _request: Request<tonic::Streaming<v1::TerminalInput>>,
    ) -> Result<Response<Self::TerminalStream>, Status> {
        Err(unimplemented("Terminal", "HUM-042"))
    }

    async fn audit(
        &self,
        _request: Request<v1::AuditRequest>,
    ) -> Result<Response<v1::AuditResponse>, Status> {
        Err(unimplemented("Audit", "HUM-050 with the audit chain"))
    }

    async fn get_config(
        &self,
        _request: Request<v1::GetConfigRequest>,
    ) -> Result<Response<v1::ConfigSnapshot>, Status> {
        Err(unimplemented(
            "GetConfig",
            "HUM-069 with the settings screen",
        ))
    }

    async fn set_config(
        &self,
        _request: Request<v1::SetConfigRequest>,
    ) -> Result<Response<v1::ConfigSnapshot>, Status> {
        Err(unimplemented(
            "SetConfig",
            "HUM-069 with the settings screen",
        ))
    }

    async fn doctor(&self, _request: Request<()>) -> Result<Response<v1::DoctorReport>, Status> {
        Err(unimplemented("Doctor", "HUM-075"))
    }

    async fn discover_llm(
        &self,
        _request: Request<v1::DiscoverRequest>,
    ) -> Result<Response<Self::DiscoverLlmStream>, Status> {
        Err(unimplemented("DiscoverLlm", "HUM-076"))
    }
}

/// Bindet einen Unix-Socket mit `0600`.
///
/// Scheitert das Setzen der Rechte, bleibt kein offener Socket zurück: ein
/// Socket, den die halbe Maschine öffnen darf, wäre der bequemste Weg an jeder
/// Entscheidung vorbei.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn Binden oder `chmod` scheitert.
pub fn bind_socket(path: &Path) -> Result<UnixListener, Diagnostic> {
    let listener = UnixListener::bind(path).map_err(|error| {
        Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
            .why(format!(
                "cannot bind the socket {}: {error}",
                path.display()
            ))
            .build()
    })?;
    if let Err(error) = std::fs::set_permissions(path, Permissions::from_mode(auth::TOKEN_MODE)) {
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
            .why(format!(
                "cannot set 0600 on the socket {}: {error}",
                path.display()
            ))
            .build());
    }
    Ok(listener)
}

/// Bedient den Vertrag auf `socket`, bis `shutdown` fertig ist.
///
/// Reihenfolge, und sie ist wichtig: erst das Token schreiben, dann den Socket
/// binden. Ein Client, der auf den Socket wartet, findet die Datei sonst noch
/// nicht, wenn der Dienst schon antwortet.
///
/// Aufgeräumt wird, was dieser Aufruf angelegt hat: Socket und Token
/// verschwinden, wenn der Dienst endet, auch wenn er mit einem Fehler endet.
/// Eine liegen gebliebene Token-Datei wäre ein Schlüssel zu einem Dienst, den
/// es nicht mehr gibt.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn Token oder Socket nicht angelegt
/// werden können, und mit `DAEMON_001`, wenn tonic den Dienst abbricht.
pub async fn serve(
    socket: &Path,
    token_path: &Path,
    server: IpcServer,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Diagnostic> {
    let token = auth::new_token()?;
    auth::write_token(token_path, &token)?;
    let listener = match bind_socket(socket) {
        Ok(listener) => listener,
        Err(diagnostic) => {
            let _ = std::fs::remove_file(token_path);
            return Err(diagnostic);
        }
    };
    tracing::info!(
        socket = %socket.display(),
        token = %token_path.display(),
        "listening"
    );

    let service =
        v1::humanitl_server::HumanitlServer::with_interceptor(server, auth::TokenAuth::new(token));
    // Das Signal wird abgezweigt: tonic hört auf, Verbindungen anzunehmen, und
    // hier beginnt zugleich die Frist aus `SHUTDOWN_GRACE`.
    let (fired, started) = oneshot::channel();
    let signal = async move {
        shutdown.await;
        let _ = fired.send(());
    };
    let serving = Server::builder()
        .add_service(service)
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), signal);
    tokio::pin!(serving);

    let outcome = tokio::select! {
        result = &mut serving => result,
        _ = started => drain(&mut serving).await,
    };

    let result = outcome.map_err(|error| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .title("gRPC-Server abgebrochen")
            .why(format!("serving {} failed: {error}", socket.display()))
            .build()
    });
    let _ = std::fs::remove_file(socket);
    let _ = std::fs::remove_file(token_path);
    tracing::info!("stopped, socket and token removed");
    result
}

/// Lässt laufenden Aufrufen die Frist aus [`SHUTDOWN_GRACE`] und endet dann.
async fn drain<F>(serving: &mut F) -> Result<(), tonic::transport::Error>
where
    F: Future<Output = Result<(), tonic::transport::Error>> + Unpin,
{
    if let Ok(result) = tokio::time::timeout(SHUTDOWN_GRACE, serving).await {
        return result;
    }
    tracing::warn!(
        grace_secs = SHUTDOWN_GRACE.as_secs(),
        "a client kept its connection open past the grace period; closing anyway"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;
    use std::time::SystemTime;

    use humanitl_config::{Config, Limits};
    use humanitl_core::{
        Authority, BodyRef, Flow, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
        TransitionInput,
    };
    use humanitl_proxy::registry::FlowRecord;
    use humanitl_proxy::{ConnMeta, FlowRegistry, HoldQueue};
    use tonic::Request;

    use super::{CAPABILITIES, IpcServer};
    use crate::v1;
    use crate::v1::humanitl_server::Humanitl as _;

    fn queue() -> Arc<HoldQueue> {
        let limits = Limits::default();
        Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ))
    }

    fn server(queue: &Arc<HoldQueue>) -> IpcServer {
        IpcServer::new(
            Arc::clone(queue),
            &Config::default(),
            Some(SessionId::new()),
        )
    }

    fn analyzed(session: SessionId, host: &str) -> Flow {
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns(host.to_owned()), Scheme::Https),
            "/v1/x",
        )
        .with_body(BodyRef::detached([0; 32], 7));
        let mut flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
        flow.apply(
            TransitionInput::Analyze { findings: vec![] },
            SystemTime::now(),
        )
        .unwrap();
        flow
    }

    #[tokio::test]
    async fn get_info_names_the_contract_and_the_session() {
        let queue = queue();
        let server = server(&queue);
        let info = server
            .get_info(Request::new(()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(info.proto_major, crate::PROTO_MAJOR);
        assert_eq!(info.proto_minor, crate::PROTO_MINOR);
        assert_eq!(info.capabilities, CAPABILITIES);
        assert!(!info.session_id.is_empty());
    }

    #[tokio::test]
    async fn every_rpc_of_sprint_two_and_later_is_unimplemented() {
        let queue = queue();
        let server = server(&queue);
        let codes = [
            server
                .get_flow(Request::new(v1::FlowRef::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .get_body(Request::new(v1::BodyRef::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .rules(Request::new(v1::RulesRequest::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .sandbox(Request::new(v1::SandboxRequest::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .audit(Request::new(v1::AuditRequest::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .get_config(Request::new(v1::GetConfigRequest::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .set_config(Request::new(v1::SetConfigRequest::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .doctor(Request::new(()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
            server
                .discover_llm(Request::new(v1::DiscoverRequest::default()))
                .await
                .err()
                .map(|status| (status.code(), status.message().to_owned())),
        ];
        for entry in codes {
            let (code, message) = entry.expect("the rpc must refuse, not answer");
            assert_eq!(code, tonic::Code::Unimplemented);
            assert!(message.contains("arrives in"), "{message}");
        }
    }

    #[tokio::test]
    async fn a_decision_without_a_decision_field_is_refused() {
        let queue = queue();
        let server = server(&queue);
        let status = server
            .decide(Request::new(v1::DecideRequest {
                flow_ids: vec![FlowId::new().to_string()],
                ..v1::DecideRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("never an allow"), "{status}");
    }

    #[tokio::test]
    async fn a_decision_for_a_flow_that_is_not_held_is_failed_precondition() {
        let queue = queue();
        let server = server(&queue);
        let status = server
            .decide(Request::new(v1::DecideRequest {
                flow_ids: vec![FlowId::new().to_string()],
                decision: Some(v1::decide_request::Decision::Allow(())),
                ..v1::DecideRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn an_edited_body_over_the_cap_is_invalid_argument() {
        let queue = queue();
        let config = Config {
            limits: Limits {
                hold_body_cap_bytes: 16,
                ..Limits::default()
            },
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, None);

        let status = server
            .decide(Request::new(v1::DecideRequest {
                flow_ids: vec![FlowId::new().to_string()],
                decision: Some(v1::decide_request::Decision::AllowEdited(
                    v1::EditedRequest {
                        method: v1::Method::Post as i32,
                        url: "https://example.com/x".to_owned(),
                        body: vec![b'x'; 17],
                        ..v1::EditedRequest::default()
                    },
                )),
                ..v1::DecideRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("hold_body_cap_bytes"), "{status}");
    }

    #[tokio::test]
    async fn a_slow_listener_gets_a_lagged_event_and_reloads_with_list_flows() {
        use tokio_stream::StreamExt as _;

        let limits = Limits {
            event_buffer: 8,
            ..Limits::default()
        };
        let queue = Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ));
        let config = Config {
            limits,
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, None);
        // Der Strom wird angelegt und dann nicht gelesen: genau der Zuhörer,
        // den der Rundfunk überholt.
        let mut stream = server.event_stream(&v1::SubscribeRequest::default());

        let session = SessionId::new();
        for _ in 0..20 {
            let flow = analyzed(session, "example.com");
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
            queue.publish(flow.received_event());
        }

        let event = stream.next().await.expect("the stream stays open").unwrap();
        let Some(v1::flow_event::Event::Lagged(lagged)) = event.event else {
            panic!("the first event after the overrun is Lagged, got {event:?}");
        };
        assert!(lagged.dropped > 0, "Lagged names how many events are gone");

        // Nachladen: die Registry kennt jeden Flow, auch die verpassten.
        assert_eq!(server.page(&v1::ListFlowsRequest::default()).total, 20);
    }

    #[tokio::test]
    async fn list_flows_orders_by_arrival_and_honours_the_limit() {
        let queue = queue();
        let server = server(&queue);
        let session = SessionId::new();
        let mut ids = Vec::new();
        for host in ["a.example.com", "b.example.com", "c.example.com"] {
            let flow = analyzed(session, host);
            ids.push(flow.id);
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        }

        let page = server.page(&v1::ListFlowsRequest::default());
        assert_eq!(page.total, 3);
        let order: Vec<String> = page.flows.iter().map(|row| row.flow_id.clone()).collect();
        let expected: Vec<String> = ids.iter().map(ToString::to_string).collect();
        assert_eq!(order, expected);

        let filtered = server.page(&v1::ListFlowsRequest {
            filter: "host:b.example.com".to_owned(),
            ..v1::ListFlowsRequest::default()
        });
        assert_eq!(filtered.flows.len(), 1);

        let first = server.page(&v1::ListFlowsRequest {
            limit: 2,
            ..v1::ListFlowsRequest::default()
        });
        assert_eq!(first.flows.len(), 2);
        assert_eq!(first.next_cursor, expected[1]);
    }
}
