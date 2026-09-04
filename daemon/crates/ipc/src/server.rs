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

use base64::Engine as _;
use bytes::Bytes;
use dashmap::DashMap;
use humanitl_config::Config;
use humanitl_core::diagnostics::codes;
use humanitl_core::{
    BlockReason, BodyRef, Decision, DecisionSource, Diagnostic, FlowEvent, FlowId, HostName,
    SessionId, Severity,
};
use humanitl_proxy::hold::NotHeld;
use humanitl_proxy::llm_probe;
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::{
    ClientTls, Direct, FlowFilter, FlowRegistry, HoldQueue, LlmProbe, Resolver, ResolverPort,
    Upstream,
};
use humanitl_recorder::{
    Cursor, CursorKey, Dir, FlowDetail as RecordedDetail, FlowQuery, Recorder, SortKey,
};
use tokio::net::UnixListener;
use tokio::sync::oneshot;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::domains::DomainTable;
use crate::rules::RulesService;
use crate::server_stub::{BoxStream, diagnostic_to_status};
use crate::{PROTO_MAJOR, PROTO_MINOR, auth, convert, rules, v1};

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

/// So viele Bytes trägt ein Stück eines Bodys über die Leitung (`GetBody`).
///
/// Groß genug, damit ein Body von einigen Megabyte nicht in tausend Nachrichten
/// zerfällt, klein genug, damit der erste Teil sofort beim Client ist.
pub const BODY_CHUNK_BYTES: usize = 64 * 1024;

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
    rules: Option<RulesService>,
    recorder: Option<Recorder>,
    domains: Option<Arc<DomainTable>>,
    /// Die host-seitige Probe des LLM-Endpunkts (HUM-039).
    ///
    /// Sie hängt am Dienst und nicht am Proxy, weil sie nichts mit einem Fluss
    /// zu tun hat: Sie beantwortet eine Frage, die ein Mensch im Setup stellt,
    /// bevor überhaupt eine Sandbox läuft. `None` nur, wenn sich der
    /// Verbindungsstapel aus der Konfiguration nicht bauen ließ; der Grund
    /// steht dann in `llm_probe_error` und geht als Antwort an den Aufrufer,
    /// statt still zu verschwinden.
    llm_probe: Option<Arc<LlmProbe>>,
    llm_probe_error: Option<Diagnostic>,
    bodies: BodyIndex,
}

/// Wo ein Body liegt, den dieser Dienst schon einmal ausgeliefert hat.
///
/// `GetBody` bekommt nur einen [`v1::BodyRef`], also Prüfsumme und Größe. Ein
/// großer Body steht damit fest: Er liegt unter seiner Prüfsumme im
/// Blob-Speicher. Ein kleiner steht als Spalte in der Zeile seiner Nachricht,
/// und die findet man ohne Flow und Richtung nicht wieder — die Aufzeichnung
/// kennt keinen Weg von der Prüfsumme zur Nachricht (siehe Bericht zu
/// HUM-026).
///
/// Deshalb merkt sich der Dienst beim Ausliefern eines `FlowDetail`, zu
/// welchem Flow und welcher Richtung jede Prüfsumme gehört. Der Weg des
/// Klienten ist ohnehin `GetFlow` und dann `GetBody`: Vorher kennt er keinen
/// [`v1::BodyRef`], den er verlangen könnte. Gespeichert werden nur die
/// Kennungen, nie Inhalt; ein Eintrag kostet gut fünfzig Bytes.
type BodyIndex = DashMap<[u8; 32], (FlowId, Dir)>;

impl IpcServer {
    /// Der Dienst über einer Warteschlange und ihrer Registry.
    ///
    /// Die Registry kommt aus [`HoldQueue::registry`]: Warteschlange und
    /// Verzeichnis teilen sich einen Rundfunk-Kanal, und ein zweiter Kanal
    /// würde die Reihenfolge der Ereignisse je Flow zerreißen (HUM-016).
    #[must_use]
    pub fn new(queue: Arc<HoldQueue>, config: &Config, session: Option<SessionId>) -> Self {
        let registry = Arc::clone(queue.registry());
        let probe = build_llm_probe(config).map(Arc::new);
        if let Err(diagnostic) = probe.as_ref() {
            tracing::warn!(
                code = %diagnostic.code,
                why = %diagnostic.why,
                "the LLM endpoint probe is not available in this daemon"
            );
        }
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
            rules: None,
            recorder: None,
            domains: None,
            llm_probe: probe.as_ref().ok().map(Arc::clone),
            llm_probe_error: probe.err(),
            bodies: BodyIndex::new(),
        }
    }

    /// Derselbe Dienst mit einer anderen Probe; für Tests.
    ///
    /// Der Daemon braucht das nicht: [`IpcServer::new`] baut die Probe aus
    /// derselben Konfiguration, aus der auch der Proxy seine Ports baut.
    #[must_use]
    pub fn with_llm_probe(mut self, probe: LlmProbe) -> Self {
        self.llm_probe = Some(Arc::new(probe));
        self.llm_probe_error = None;
        self
    }

    /// Derselbe Dienst, der aus der Aufzeichnung liest.
    ///
    /// Mit ihr beantworten `ListFlows`, `GetFlow` und `GetBody` aus der
    /// Datenbank statt aus dem Speicher: Die Historie überlebt damit einen
    /// Neustart, und die Oberfläche hält nie mehr als eine Seite (ADR-008).
    /// Ohne sie bleibt es bei der Registry der laufenden Sitzung — der Weg,
    /// den der Fake und die Tests gehen.
    #[must_use]
    pub fn with_recorder(mut self, recorder: Recorder) -> Self {
        self.recorder = Some(recorder);
        self
    }

    /// Derselbe Dienst mit dem Domain-Katalog (HUM-031).
    ///
    /// Dieselbe Tabelle, die der Proxy beim Eintreffen eines Flows füllt; hier
    /// wird sie nur gelesen.
    #[must_use]
    pub fn with_domains(mut self, domains: Arc<DomainTable>) -> Self {
        self.domains = Some(domains);
        self
    }

    /// Die Aufzeichnung, sofern eine verdrahtet ist.
    #[must_use]
    pub const fn recorder(&self) -> Option<&Recorder> {
        self.recorder.as_ref()
    }

    /// Derselbe Dienst mit einem Regelspeicher.
    ///
    /// Ohne ihn antwortet `Rules` mit `IPC_005` und `Decide` lehnt ein
    /// `remember` ab, statt es stillschweigend zu verwerfen. Der Recorder ist
    /// die Quelle des Probelaufs; fehlt er, prüft `dry_run` null Flows.
    #[must_use]
    pub fn with_rules(mut self, store: Arc<RulesStore>, recorder: Option<Recorder>) -> Self {
        self.rules = Some(RulesService::new(store, recorder));
        self
    }

    /// Der Regel-Dienst, sobald einer verdrahtet ist.
    #[must_use]
    pub const fn rules_service(&self) -> Option<&RulesService> {
        self.rules.as_ref()
    }

    /// Der Regel-Dienst oder der Befund, dass dieser Daemon keinen hat.
    fn rules_or_refuse(&self) -> Result<&RulesService, Diagnostic> {
        self.rules.as_ref().ok_or_else(rules::no_store)
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
        let domains = self.domains.clone();
        let include_passthrough = request.include_passthrough;
        let live = BroadcastStream::new(self.registry.subscribe()).filter_map(move |item| {
            let event = match item {
                Ok(event) => event,
                Err(BroadcastStreamRecvError::Lagged(n)) => FlowEvent::Lagged { n },
            };
            // Ereignisse eines Durchreich-Flows (LLM-Passthrough) sieht nur,
            // wer sie ausdruecklich verlangt; `ListFlows` und der Rueckstand
            // filtern genauso, damit der Strom nicht stiller ist als die
            // Liste.
            //
            // Ein Befund ist davon ausgenommen, und zwar immer. `LLM_005`
            // warnt vor genau der Anfrage, die hier versteckt wird; ihn
            // mitzuverstecken kehrte die Zusage aus `docs/SECURITY.md` 3.1 um
            // („ein Treffer erzeugt eine Warnung") und liesse den Menschen
            // ohne die eine Meldung zurueck, die er sehen muss. Eingeklappt
            // heisst nicht stumm (HUM-039).
            let hidden = !include_passthrough
                && !matches!(event, FlowEvent::Diagnostic { .. })
                && event.flow_id().is_some_and(|id| {
                    registry
                        .get(id)
                        .is_some_and(|record| convert::record_to_summary(&record).passthrough)
                });
            (!hidden).then(|| {
                Ok(convert::flow_event_to_proto(
                    &event,
                    &registry,
                    domains.as_deref(),
                ))
            })
        });
        match self.rules.as_ref().map(RulesService::store) {
            None => Box::pin(tokio_stream::iter(self.backlog(request)).chain(live)),
            Some(store) => {
                let changes = rules_changed_stream(Arc::clone(store));
                Box::pin(tokio_stream::iter(self.backlog(request)).chain(live.merge(changes)))
            }
        }
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
            .filter(|summary| request.include_passthrough || !summary.passthrough)
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
            // Die Registry zaehlt, was sie hat: die Zahl ist exakt.
            capped: false,
        }
    }

    /// Eine Seite der Flow-Historie aus der Aufzeichnung.
    ///
    /// Gefiltert, sortiert und geschnitten wird in `SQLite`; hier wird nur
    /// übersetzt. Vor dem Lesen steht ein [`Recorder::flush`]: Der Schreiber
    /// bündelt Schreibvorgänge, und eine Liste, die den gerade entschiedenen
    /// Flow noch nicht kennt, wäre für den Menschen ein verschwundener Flow.
    ///
    /// `since_flow_id` wird auf der Seite angewandt, nicht in der Abfrage: Die
    /// Aufzeichnung blättert nach Zeit, und der Anker ist eine Flow-Id. Weil
    /// eine [`FlowId`] ein UUID der Fassung 7 ist, ist ihre Ordnung die der
    /// Ankunft; die Zeilen davor fallen damit richtig weg. `total` bleibt die
    /// Zahl, die der Filter trifft.
    async fn recorded_page(
        &self,
        recorder: &Recorder,
        request: &v1::ListFlowsRequest,
    ) -> Result<v1::FlowPage, Diagnostic> {
        let query = flow_query(request)?;
        recorder.flush().await;
        let page = recorder
            .list_flows(&query)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)?;
        let since = FlowId::parse(&request.since_flow_id).ok();
        let flows = page
            .rows
            .iter()
            .filter(|row| since.is_none_or(|anchor| row.id > anchor))
            .map(convert::recorded_summary_to_proto)
            .collect();
        Ok(v1::FlowPage {
            flows,
            next_cursor: page.next.as_ref().map(encode_cursor).unwrap_or_default(),
            total: page.total_estimate,
            // Der Recorder zaehlt nur bis zu seiner Obergrenze; ist sie
            // erreicht, ist `total` eine Untergrenze und die Oberflaeche muss
            // das sagen statt es zu raten (CONVENTIONS 4.13).
            capped: page.capped,
        })
    }

    /// Ein Flow mit allem, was zu ihm aufgezeichnet wurde.
    async fn recorded_detail(
        &self,
        recorder: &Recorder,
        id: FlowId,
    ) -> Result<Option<v1::FlowDetail>, Diagnostic> {
        recorder.flush().await;
        let Some(detail) = recorder
            .get_flow(id)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        else {
            return Ok(None);
        };
        for message in &detail.messages {
            self.bodies.insert(message.body.sha256, (id, message.dir));
        }
        let preview = preview_of(recorder, &detail).await;
        Ok(Some(convert::recorded_detail_to_proto(
            &detail,
            self.domain_of(id, &detail.summary.host),
            // Nur die Registry der laufenden Sitzung weiß, ob der Scan die
            // ganze Anfrage gesehen hat; die Tabelle `flows` führt die Spalte
            // nicht (siehe Bericht zu HUM-026).
            self.registry.get(id).is_some_and(|r| r.findings_truncated),
            preview,
        )))
    }

    /// Was der Katalog zu diesem Flow sagt.
    ///
    /// Für einen Flow dieser Sitzung die Antwort, die beim Eintreffen entstand;
    /// für einen älteren dieselbe Auskunft ohne die Zähler, denn die gehören
    /// dieser Sitzung. Ohne Katalog bleibt das Feld leer, statt einen Apex zu
    /// raten.
    fn domain_of(&self, id: FlowId, host: &str) -> Option<v1::DomainInfo> {
        let domains = self.domains.as_ref()?;
        let info = domains.get(id).or_else(|| {
            HostName::parse(host)
                .ok()
                .map(|host| domains.describe(&host))
        })?;
        Some(convert::domain_to_proto(&info))
    }

    /// Die Bytes hinter einem [`v1::BodyRef`].
    ///
    /// Zwei Wege, und der erste ist der genaue: Kennt der Dienst die Nachricht,
    /// zu der die Prüfsumme gehört ([`BodyIndex`]), liest er sie über deren
    /// Verweis und bekommt damit auch einen Body, der in der Datenbank steht.
    /// Sonst bleibt der Blob-Speicher, in dem jeder große Body unter seiner
    /// Prüfsumme liegt.
    async fn read_body(
        &self,
        recorder: &Recorder,
        wire: &v1::BodyRef,
    ) -> Result<Bytes, Diagnostic> {
        let sha256 = body_hash(wire)?;
        if let Some(entry) = self.bodies.get(&sha256) {
            let (flow, dir) = *entry.value();
            drop(entry);
            recorder.flush().await;
            let detail = recorder
                .get_flow(flow)
                .await
                .map_err(humanitl_recorder::RecorderError::into_diagnostic)?;
            if let Some(message) = detail
                .as_ref()
                .and_then(|detail| detail.messages.iter().find(|m| m.dir == dir))
                && message.body.sha256 == sha256
            {
                return recorder
                    .read_body(&message.body)
                    .await
                    .map_err(humanitl_recorder::RecorderError::into_diagnostic);
            }
        }
        let body = BodyRef {
            sha256,
            size: wire.size,
            inline: None,
            content_type: (!wire.content_type.is_empty()).then(|| wire.content_type.clone()),
            truncated: wire.truncated,
        };
        recorder
            .read_body(&body)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)
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

/// Ein Befund für eine `Decide`-Anfrage, die so nicht gilt (`InvalidArgument`).
///
/// `IPC_004` deckt jede Anfrage ab, die der Daemon nicht ausführen kann: keine
/// Flow-Id, keine Entscheidung, eine unlesbare Flow-Id, eine bearbeitete
/// Anfrage, die sich nicht lesen lässt oder über `limits.hold_body_cap_bytes`
/// liegt. Der Grund nennt den vorliegenden Fall. Der einzige Sonderfall mit
/// eigenem Code ist [`edited_for_many`].
fn bad_request(why: String) -> Diagnostic {
    Diagnostic::builder(codes::IPC_004, Severity::Error)
        .why(why)
        .build()
}

/// Ein Befund für `AllowEdited` mit mehr als einem Flow (`InvalidArgument`).
///
/// `IPC_002` bleibt genau diesem Fall vorbehalten, so wie sein Titel ihn nennt:
/// eine bearbeitete Anfrage gilt immer genau einem Flow.
fn edited_for_many(count: usize) -> Diagnostic {
    Diagnostic::builder(codes::IPC_002, Severity::Error)
        .why(format!("allow_edited came with {count} flow ids"))
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

/// Der Strom der Regeländerungen als Ereignis des Vertrags.
///
/// Eine Regeländerung ist kein Flow-Ereignis; sie hat weder Flow noch Zustand.
/// Sie reist trotzdem im selben Strom, damit ein Client genau ein Abonnement
/// braucht (`backlog/sprint-2.md`, HUM-027). Der Inhalt ist nur die Revision:
/// Wer sie sieht, lädt die Liste über `Rules{list}` nach.
///
/// Fällt ein Zuhörer zurück, bekommt er statt der verpassten Zwischenstände
/// den aktuellen Stand — mehr braucht er nicht, weil er ohnehin nachlädt.
fn rules_changed_stream(store: Arc<RulesStore>) -> BoxStream<Result<v1::FlowEvent, Status>> {
    let stream = BroadcastStream::new(store.subscribe()).map(move |item| {
        let revision = match item {
            Ok(revision) => revision,
            Err(BroadcastStreamRecvError::Lagged(_)) => store.revision(),
        };
        Ok(v1::FlowEvent {
            at: Some(convert::timestamp(std::time::SystemTime::now())),
            event: Some(v1::flow_event::Event::RulesChanged(
                v1::flow_event::RulesChanged { revision },
            )),
        })
    });
    Box::pin(stream)
}

/// Die Anfrage an die Aufzeichnung, aus der Anfrage des Vertrags.
///
/// Der Filter wird durchgereicht, wie er ist: Seine Grammatik ist dieselbe für
/// Oberfläche, `ListFlows` und Kommandozeile (`backlog/sprint-2.md` HUM-026),
/// und ein unbekannter Schlüssel ist dort ein Befund mit `RECORDER_002`, nicht
/// eine stillschweigend leere Liste. Ohne `include_passthrough` kommt
/// `passthrough:false` dazu — derselbe Filter, den auch der Ereignisstrom legt.
///
/// # Errors
///
/// `IPC_005`, wenn `order_by` oder `cursor` nicht lesbar sind.
fn flow_query(request: &v1::ListFlowsRequest) -> Result<FlowQuery, Diagnostic> {
    let mut filter = request.filter.trim().to_owned();
    if !request.include_passthrough {
        if !filter.is_empty() {
            filter.push(' ');
        }
        filter.push_str("passthrough:false");
    }
    let (sort, desc) = order_of(&request.order_by)?;
    let cursor = if request.cursor.is_empty() {
        None
    } else {
        Some(decode_cursor(&request.cursor)?)
    };
    Ok(FlowQuery {
        filter,
        sort,
        desc,
        limit: request.limit,
        cursor,
    })
}

/// Sortierschlüssel und Richtung aus `ListFlowsRequest.order_by`.
///
/// Leer heißt „nach Ankunft, neueste zuerst". Ein Schlüssel, den die
/// Aufzeichnung nicht sortieren kann, wird abgelehnt und nicht durch einen
/// anderen ersetzt: Eine Liste in einer Reihenfolge, die niemand verlangt hat,
/// sähe aus wie die verlangte.
///
/// # Errors
///
/// `IPC_005` mit der Liste der gültigen Schlüssel.
fn order_of(order_by: &str) -> Result<(SortKey, bool), Diagnostic> {
    let lower = order_by.to_ascii_lowercase();
    let mut words = lower.split_whitespace();
    let sort = match words.next() {
        None | Some("received_at" | "ts" | "time") => SortKey::Ts,
        Some("host") => SortKey::Host,
        Some("duration") => SortKey::Duration,
        Some("size") => SortKey::Size,
        Some(other) => {
            return Err(Diagnostic::builder(codes::IPC_005, Severity::Error)
                .why(format!(
                    "{other:?} is not a sort key; list_flows sorts by received_at, host, \
                     duration or size"
                ))
                .build());
        }
    };
    let ascending = words.any(|word| word == "asc");
    Ok((sort, !ascending))
}

/// Der Cursor der nächsten Seite als Text des Vertrags.
///
/// Er trägt genau die drei Felder, die die Aufzeichnung braucht
/// (`backlog/CONVENTIONS.md` 4.14), und ist für den Klienten undurchsichtig:
/// Er reicht ihn zurück, wie er ihn bekommen hat.
fn encode_cursor(cursor: &Cursor) -> String {
    let sort = match &cursor.sort {
        None => String::new(),
        Some(CursorKey::Int(value)) => format!("i{value}"),
        Some(CursorKey::Text(value)) => format!("t{value}"),
    };
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!("{}\u{1f}{}\u{1f}{sort}", cursor.ts, cursor.id))
}

/// Liest einen Cursor zurück.
///
/// # Errors
///
/// `IPC_005`, wenn der Text nicht von [`encode_cursor`] stammt. Geraten wird
/// nichts: Ein halb gelesener Cursor lieferte eine Seite, die weder lückenlos
/// noch doppelfrei wäre.
fn decode_cursor(text: &str) -> Result<Cursor, Diagnostic> {
    let refuse = || {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!(
                "{text:?} is not a cursor from this daemon; ask again without a cursor to \
                 start at the first page"
            ))
            .build()
    };
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_error| refuse())?;
    let decoded = String::from_utf8(raw).map_err(|_error| refuse())?;
    let mut parts = decoded.split('\u{1f}');
    let ts: i64 = parts
        .next()
        .ok_or_else(refuse)?
        .parse()
        .map_err(|_error| refuse())?;
    let id = parts.next().ok_or_else(refuse)?.to_owned();
    let sort = match parts.next().unwrap_or_default() {
        "" => None,
        rest => match rest.split_at(1) {
            ("i", value) => Some(CursorKey::Int(value.parse().map_err(|_error| refuse())?)),
            ("t", value) => Some(CursorKey::Text(value.to_owned())),
            _ => return Err(refuse()),
        },
    };
    if parts.next().is_some() {
        return Err(refuse());
    }
    Ok(Cursor { ts, id, sort })
}

/// Die Prüfsumme aus einem [`v1::BodyRef`].
///
/// # Errors
///
/// `IPC_005`, wenn sie keine 32 Bytes hat. Ein gekürzter Hash zeigte auf einen
/// anderen Inhalt oder auf keinen.
fn body_hash(wire: &v1::BodyRef) -> Result<[u8; 32], Diagnostic> {
    <[u8; 32]>::try_from(wire.sha256.as_slice()).map_err(|_error| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!(
                "a body reference carries a sha256 of {} bytes, not 32",
                wire.sha256.len()
            ))
            .build()
    })
}

/// Der Anfang des Anfrage-Bodys für die Anzeige.
///
/// Steht der Body in der Datenbank, kommt er von dort; sonst aus dem
/// Blob-Speicher. Scheitert das Lesen, bleibt die Vorschau leer und der Grund
/// steht im Protokoll: Der vollständige Inhalt kommt ohnehin über `GetBody`,
/// und eine erfundene Vorschau wäre schlimmer als keine.
async fn preview_of(recorder: &Recorder, detail: &RecordedDetail) -> String {
    let Some(message) = detail
        .messages
        .iter()
        .find(|message| message.dir == Dir::Request)
    else {
        return String::new();
    };
    if let Some(inline) = message.body.inline.as_ref() {
        return convert::body_preview(inline);
    }
    if message.body.size == 0 {
        return String::new();
    }
    match recorder.read_body(&message.body).await {
        Ok(bytes) => convert::body_preview(&bytes),
        Err(error) => {
            tracing::warn!(
                flow = %detail.summary.id,
                why = %error,
                "the request body could not be read for the preview"
            );
            String::new()
        }
    }
}

/// Zerlegt einen Body in die Stücke, die `GetBody` streamt.
///
/// Auch ein leerer Body ergibt genau ein Stück mit `last = true`: Der Klient
/// soll das Ende sehen und nicht auf ein nächstes warten.
fn body_stream(bytes: &Bytes) -> BoxStream<Result<v1::BodyChunk, Status>> {
    let total = bytes.len();
    let chunks: Vec<Result<v1::BodyChunk, Status>> = if total == 0 {
        vec![Ok(v1::BodyChunk {
            data: Vec::new(),
            offset: 0,
            last: true,
        })]
    } else {
        (0..total)
            .step_by(BODY_CHUNK_BYTES)
            .map(|offset| {
                let end = (offset + BODY_CHUNK_BYTES).min(total);
                Ok(v1::BodyChunk {
                    data: bytes.slice(offset..end).to_vec(),
                    offset: offset as u64,
                    last: end == total,
                })
            })
            .collect()
    };
    Box::pin(tokio_stream::iter(chunks))
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

    /// Eine Seite der Flow-Historie.
    ///
    /// Mit Aufzeichnung kommt sie aus der Datenbank und umfasst damit auch
    /// frühere Sitzungen; ohne sie aus der Registry dieser Sitzung.
    async fn list_flows(
        &self,
        request: Request<v1::ListFlowsRequest>,
    ) -> Result<Response<v1::FlowPage>, Status> {
        let request = request.into_inner();
        let Some(recorder) = self.recorder.as_ref() else {
            return Ok(Response::new(self.page(&request)));
        };
        self.recorded_page(recorder, &request)
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
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
                .map(|text| refused(text, edited_for_many(count)).0)
                .collect();
            return Ok(Response::new(v1::DecideResponse {
                results,
                created_rule_id: String::new(),
                created_rule: None,
            }));
        }

        // Erst die Regel, dann die Entscheidung: scheitert das Anlegen, wird
        // nichts entschieden (`backlog/sprint-2.md`, HUM-027). Der umgekehrte
        // Weg hinterließe einen freigegebenen Flow und einen Menschen, der
        // glaubt, die Freigabe gelte ab jetzt für alle.
        let created = match request.remember.as_ref() {
            None => None,
            Some(rule) => {
                let service = self
                    .rules_or_refuse()
                    .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
                Some(
                    service
                        .remember(rule)
                        .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?,
                )
            }
        };

        let (results, refusals): (Vec<v1::DecideResult>, Vec<Option<Diagnostic>>) = request
            .flow_ids
            .iter()
            .map(|text| self.decide_one(text, &decision))
            .unzip();

        if results.iter().all(|result| !result.applied) {
            // Nichts entschieden heißt: die Anfrage hat nichts bewirkt. Die
            // Regel, die nur zu dieser Entscheidung gehörte, wird deshalb
            // wieder zurückgenommen.
            if let (Some(rule), Some(service)) = (created.as_ref(), self.rules.as_ref())
                && let Ok(id) = humanitl_core::RuleId::parse(&rule.rule_id)
            {
                service.forget(id);
            }
            let first = refusals
                .into_iter()
                .flatten()
                .next()
                .unwrap_or_else(|| not_held(&NotHeld::Unknown { id: FlowId::nil() }));
            return Err(diagnostic_to_status(&first));
        }

        Ok(Response::new(v1::DecideResponse {
            results,
            created_rule_id: created
                .as_ref()
                .map(|rule| rule.rule_id.clone())
                .unwrap_or_default(),
            created_rule: created,
        }))
    }

    /// Alles, was zu einem Flow aufgezeichnet wurde.
    ///
    /// Ohne Aufzeichnung bleibt der Datensatz der laufenden Sitzung; er trägt
    /// die Anfrage, aber weder Antwort-Kopfzeilen noch Funde. Kennt weder die
    /// Aufzeichnung noch die Registry den Flow, ist das `NOT_FOUND` und kein
    /// leerer Erfolg: Ein leeres Detail sähe aus wie ein Flow ohne Inhalt.
    async fn get_flow(
        &self,
        request: Request<v1::FlowRef>,
    ) -> Result<Response<v1::FlowDetail>, Status> {
        let request = request.into_inner();
        let id = FlowId::parse(&request.flow_id).map_err(|error| {
            diagnostic_to_status(&bad_request(format!(
                "{:?} is not a flow id: {error}",
                request.flow_id
            )))
        })?;
        if let Some(recorder) = self.recorder.as_ref()
            && let Some(detail) = self
                .recorded_detail(recorder, id)
                .await
                .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?
        {
            return Ok(Response::new(detail));
        }
        self.registry
            .get(id)
            .map(|record| Response::new(convert::record_to_detail(&record)))
            .ok_or_else(|| Status::not_found(format!("IPC_003 {}", NotHeld::Unknown { id })))
    }

    /// Der Inhalt eines Bodys, in Stücken.
    ///
    /// Ohne Aufzeichnung gibt es keinen Ort, an dem ein Body läge: Der Proxy
    /// hält ihn nur, solange die Anfrage läuft. Das ist dann kein
    /// `UNIMPLEMENTED` — die RPC gibt es —, sondern der Befund, dass dieser
    /// Daemon ohne Aufzeichnung läuft.
    async fn get_body(
        &self,
        request: Request<v1::BodyRef>,
    ) -> Result<Response<Self::GetBodyStream>, Status> {
        let wire = request.into_inner();
        let recorder = self.recorder.as_ref().ok_or_else(|| {
            diagnostic_to_status(
                &Diagnostic::builder(codes::RECORDER_001, Severity::Error)
                    .why(
                        "this daemon runs without a recording; bodies are only kept while the \
                         request is in flight"
                            .to_owned(),
                    )
                    .build(),
            )
        })?;
        let bytes = self
            .read_body(recorder, &wire)
            .await
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(body_stream(&bytes)))
    }

    /// Liest oder ändert den Regelsatz.
    ///
    /// Jede Antwort trägt den vollständigen Regelsatz danach. Was die Anfrage
    /// nicht ausführen konnte, ist ein Fehler des Aufrufs, kein Erfolg mit
    /// leerem Inhalt; nur `reload` legt seine Befunde in die Antwort, weil
    /// dabei die alten Regeln in Kraft bleiben und der Client sie sehen soll.
    async fn rules(
        &self,
        request: Request<v1::RulesRequest>,
    ) -> Result<Response<v1::RulesResponse>, Status> {
        let service = self
            .rules_or_refuse()
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        service
            .apply(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
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

    /// Prüft genau den Endpunkt, den der Aufrufer nennt (HUM-039).
    ///
    /// Nichts davon läuft in der Sandbox, nichts davon ändert etwas: zwei
    /// `GET` auf zwei feste Pfade, keine Weiterleitung, keine Zugangsdaten.
    /// Was der Endpunkt nicht beantwortet hat, steht als Befund in der Antwort
    /// und nie als erfundene Modellliste.
    async fn probe_llm(
        &self,
        request: Request<v1::ProbeLlmRequest>,
    ) -> Result<Response<v1::ProbeLlmResponse>, Status> {
        let request = request.into_inner();
        let probe = self.llm_probe.as_ref().ok_or_else(|| {
            let diagnostic = self.llm_probe_error.clone().unwrap_or_else(no_probe);
            diagnostic_to_status(&diagnostic)
        })?;
        let endpoint = url::Url::parse(&request.endpoint).map_err(|err| {
            diagnostic_to_status(
                &Diagnostic::builder(codes::LLM_007, Severity::Error)
                    .why(format!(
                        "{:?} is not a URL Humanitl can read: {err}",
                        request.endpoint
                    ))
                    .fix(humanitl_core::FixAction::ChangeSetting {
                        key: "llm.endpoint".to_owned(),
                        value: llm_probe::EXAMPLE_ENDPOINT.to_owned(),
                    })
                    .build(),
            )
        })?;
        // Die Probe klemmt die Frist selbst auf `MAX_TIMEOUT_MS`; hier steht
        // nur die Umrechnung, und `0` heißt die Vorgabe.
        let timeout = match request.timeout_ms {
            0 => None,
            ms => Some(Duration::from_millis(u64::from(ms))),
        };
        let result = probe
            .probe(&endpoint, timeout)
            .await
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(convert::probe_result_to_proto(&result)))
    }
}

/// Baut den Verbindungsstapel der Endpunkt-Probe aus der Konfiguration.
///
/// Ein **eigener** Stapel, nicht der des Proxys, und zwar mit Absicht: Der
/// Zähler des Proxy-Resolvers belegt, dass vor einer Freigabe kein Name
/// aufgelöst wird (ADR-006, Escape-Test 3). Eine Auflösung, die ein Mensch im
/// Setup selbst angestoßen hat, gehört nicht in diesen Beweis. Die
/// Einstellungen sind dieselben (`resolver.*`, `limits.*`), also verhält sich
/// die Probe gegenüber `ollama.lan` genauso wie später der Proxy.
///
/// Die Vertrauensanker sind die des Upstreams, nicht die Humanitl-CA: Die
/// Probe redet mit dem Endpunkt, nicht mit dem Agenten.
fn build_llm_probe(config: &Config) -> Result<LlmProbe, Diagnostic> {
    let resolver = Arc::new(ResolverPort::from_config(&config.resolver)?);
    let client_tls = ClientTls::new(&[], false)?;
    let upstream = Upstream::new(
        Arc::new(Direct::new(Duration::from_secs(
            config.limits.connect_timeout_secs,
        ))),
        resolver as Arc<dyn Resolver>,
        client_tls,
        config.resolver.prefer,
        Duration::from_secs(config.limits.header_timeout_secs),
    );
    Ok(LlmProbe::new(upstream))
}

/// Der Befund für einen Daemon ohne Endpunkt-Probe.
///
/// `IPC_006` und nicht `IPC_005`: Der Regel-RPC hat damit nichts zu tun, und
/// ein Code, der „Rules-Anfrage ungültig" heißt, schickte den Leser an die
/// falsche Stelle.
fn no_probe() -> Diagnostic {
    Diagnostic::builder(codes::IPC_006, Severity::Error)
        .why(
            "this daemon runs without an endpoint probe; llm.endpoint cannot be tested here"
                .to_owned(),
        )
        .build()
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
    use humanitl_core::diagnostics::codes;
    use humanitl_core::{
        Authority, BodyRef, Diagnostic, Flow, FlowEvent, FlowId, HostName, HttpRequest, Method,
        Scheme, SessionId, Severity, TransitionInput,
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

    /// `ProbeLlm` gibt es (HUM-039), und was es nicht lesen kann, lehnt es
    /// ab, statt zu raten.
    #[tokio::test]
    async fn probe_llm_refuses_an_endpoint_it_cannot_read() {
        let queue = queue();
        let server = server(&queue);
        for endpoint in ["", "not a url", "ftp://model.lan/"] {
            let Err(status) = server
                .probe_llm(Request::new(v1::ProbeLlmRequest {
                    endpoint: endpoint.to_owned(),
                    timeout_ms: 50,
                }))
                .await
            else {
                panic!("an endpoint that is no http url is refused: {endpoint:?}");
            };
            assert_ne!(
                status.code(),
                tonic::Code::Unimplemented,
                "the rpc exists: {endpoint:?}"
            );
            let diagnostic = crate::server_stub::diagnostic_from_status(&status)
                .expect("the status carries the diagnostic");
            assert_eq!(
                diagnostic.code, "LLM_007",
                "nothing was measured, so no code may claim a measurement — {endpoint:?}: {}",
                diagnostic.why
            );
        }
    }

    /// Ein Ziel, an dem niemand lauscht, ist `LLM_001` samt einem `curl` —
    /// kein Fehler des Nutzers und keine erfundene Modellliste.
    #[tokio::test(flavor = "multi_thread")]
    async fn probe_llm_reports_an_endpoint_that_does_not_answer() {
        let queue = queue();
        let server = server(&queue);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port");
        let port = listener.local_addr().expect("an address").port();
        drop(listener);

        let Err(status) = server
            .probe_llm(Request::new(v1::ProbeLlmRequest {
                endpoint: format!("http://127.0.0.1:{port}"),
                timeout_ms: 2_000,
            }))
            .await
        else {
            panic!("nothing is listening on {port}, so the probe must refuse");
        };
        let diagnostic = crate::server_stub::diagnostic_from_status(&status)
            .expect("the status carries the diagnostic");
        assert_eq!(diagnostic.code, "LLM_001", "{}", diagnostic.why);
        assert!(
            matches!(
                diagnostic.fix.as_ref().and_then(|fix| fix.action.as_ref()),
                Some(v1::fix_action::Action::CopyCommand(_))
            ),
            "the fix is a command the human can run: {:?}",
            diagnostic.fix
        );
    }

    #[tokio::test]
    async fn without_a_recording_a_flow_detail_is_not_found_and_a_body_says_why() {
        // `GetFlow` und `GetBody` sind kein `UNIMPLEMENTED` mehr (HUM-026). Ohne
        // Aufzeichnung antwortet der Dienst aus der Registry dieser Sitzung, und
        // was auch die nicht kennt, ist `NOT_FOUND` — nie ein leeres Detail, das
        // wie ein Flow ohne Inhalt aussähe.
        let queue = queue();
        let server = server(&queue);

        let status = server
            .get_flow(Request::new(v1::FlowRef {
                flow_id: FlowId::new().to_string(),
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound, "{status}");

        // Ein Body dagegen hat ohne Aufzeichnung überhaupt keinen Ort: Der Proxy
        // hält ihn nur, solange die Anfrage läuft. Das sagt der Befund.
        let refused = server.get_body(Request::new(v1::BodyRef::default())).await;
        let Err(status) = refused else {
            panic!("a daemon without a recording has no body to hand out")
        };
        assert!(status.message().contains("RECORDER_001"), "{status}");
    }

    #[tokio::test]
    async fn a_flow_of_this_session_is_answered_from_the_registry() {
        let queue = queue();
        let server = server(&queue);
        let session = SessionId::new();
        let flow = analyzed(session, "api.github.com");
        let id = flow.id;
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));

        let detail = server
            .get_flow(Request::new(v1::FlowRef {
                flow_id: id.to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(detail.summary.expect("a summary").flow_id, id.to_string());
        assert!(!detail.findings_truncated, "nothing was cut short");
    }

    #[tokio::test]
    async fn rules_without_a_store_name_the_reason_instead_of_answering_empty() {
        // `Rules` ist kein `UNIMPLEMENTED` mehr (HUM-027). Ein Daemon ohne
        // Regelspeicher sagt das; eine leere Liste sähe aus wie „keine Regeln".
        let queue = queue();
        let server = server(&queue);
        let status = server
            .rules(Request::new(v1::RulesRequest {
                op: Some(v1::rules_request::Op::List(())),
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("IPC_005"), "{status}");
        assert!(status.message().contains("rule store"), "{status}");
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
        assert!(status.message().contains("IPC_004"), "{status}");
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

    /// Ein durchgereichter Fluss ist eingeklappt, seine Warnung nicht.
    ///
    /// Der Filter für `include_passthrough: false` versteckt jedes Ereignis
    /// eines Durchreich-Flusses. Ein Befund darf davon nie betroffen sein:
    /// `LLM_005` warnt vor genau der Anfrage, die hier versteckt wird, und mit
    /// ihr zu verschwinden kehrte die Zusage aus `docs/SECURITY.md` 3.1 um
    /// („ein Treffer erzeugt eine Warnung"). Geprüft wird über
    /// [`IpcServer::event_stream`], nicht über den rohen Rundfunk: Nur der
    /// Strom des Dienstes trägt den Filter, und nur seine Zustellung zählt.
    #[tokio::test]
    async fn a_warning_survives_the_passthrough_filter_that_hides_its_flow() {
        let queue = queue();
        let session = SessionId::new();
        let server = IpcServer::new(Arc::clone(&queue), &Config::default(), Some(session));
        let mut stream = server.event_stream(&v1::SubscribeRequest::default());

        let mut flow = analyzed(session, "192.168.1.50");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        // Erst die Entscheidung: Danach steht `passthrough = true` am
        // Datensatz, und genau in diesem Zustand liest die Filter-Closure
        // beim Pollen.
        let decided = flow
            .apply(
                TransitionInput::Decide {
                    decision: humanitl_core::Decision::Allow,
                    source: humanitl_core::DecisionSource::Passthrough,
                },
                SystemTime::now(),
            )
            .unwrap();
        queue.publish(decided);
        queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::LLM_005, Severity::Warning)
                    .why("two potential secrets".to_owned())
                    .build(),
            ),
        });
        queue.publish(FlowEvent::Recorded {
            flow_id: flow.id,
            at: SystemTime::now(),
        });

        let mut seen = Vec::new();
        while let Ok(Some(event)) = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio_stream::StreamExt::next(&mut stream),
        )
        .await
        {
            let event = event.unwrap();
            let Some(inner) = event.event else { continue };
            seen.push(inner);
            if seen.len() == 1 {
                break;
            }
        }

        let Some(v1::flow_event::Event::FlowDiagnostic(warning)) = seen.first() else {
            panic!("the only event that reaches a default subscriber is the warning: {seen:?}");
        };
        assert_eq!(
            warning.flow_id,
            flow.id.to_string(),
            "and it names the flow it warns about, so the details are reachable"
        );
        assert_eq!(
            warning.diagnostic.as_ref().map(|d| d.code.as_str()),
            Some("LLM_005")
        );
    }

    /// Mit `include_passthrough` kommt alles durch, die Warnung eingeschlossen.
    #[tokio::test]
    async fn with_include_passthrough_the_whole_flow_arrives() {
        let queue = queue();
        let session = SessionId::new();
        let server = IpcServer::new(Arc::clone(&queue), &Config::default(), Some(session));
        let mut stream = server.event_stream(&v1::SubscribeRequest {
            include_passthrough: true,
            ..v1::SubscribeRequest::default()
        });

        let mut flow = analyzed(session, "192.168.1.50");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        let decided = flow
            .apply(
                TransitionInput::Decide {
                    decision: humanitl_core::Decision::Allow,
                    source: humanitl_core::DecisionSource::Passthrough,
                },
                SystemTime::now(),
            )
            .unwrap();
        queue.publish(decided);

        let event = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio_stream::StreamExt::next(&mut stream),
        )
        .await
        .expect("an event")
        .expect("the stream stays open")
        .unwrap();
        assert!(
            matches!(event.event, Some(v1::flow_event::Event::Decided(_))),
            "{event:?}"
        );
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
