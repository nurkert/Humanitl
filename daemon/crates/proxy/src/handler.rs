//! Der Handler: aus einer Verbindung werden Flows.
//!
//! Der Handler terminiert HTTP/1.1 (und, nach `CONNECT`, TLS mit einem Leaf
//! aus der eigenen CA), puffert den Request-Body, baut einen [`Flow`], übergibt
//! ihn der [`FlowPipeline`] und leitet nach `Allow` über den
//! [`Upstream`] weiter oder antwortet mit einer
//! Block-Meldung. Er *treibt* den Zustandsautomaten des Kerns, er *besitzt* ihn
//! nicht (`docs/ARCHITECTURE.md` 3): jede Zustandsänderung geht durch
//! [`Flow::apply`] und wird als Ereignis veröffentlicht.
//!
//! ALPN bietet dem Client nur `http/1.1` (das Leaf aus `ca.rs`); nach oben
//! spricht der Proxy in M1 ausschließlich HTTP/1.1. HTTP/2 zum Client oder zum
//! Ziel ist [`PROXY_007`](humanitl_core::diagnostics::codes::PROXY_007) und
//! kommt erst in M6.
//!
//! Lehnt der Automat einen Übergang ab, ist das ein Fehler im Daemon, kein
//! Zustand des Clients. Der Handler behandelt ihn fail-closed (HUM-016
//! Schritt 2): [`PROXY_005`] geht
//! als [`FlowEvent::Diagnostic`] in den Ereignisstrom, der Flow endet mit
//! `Block`, und das Ziel sieht die Anfrage nicht.

use std::convert::Infallible;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use humanitl_config::{HoldConfig, Limits, RecorderConfig};
use humanitl_core::diagnostics::codes::{PROXY_005, PROXY_008};
use humanitl_core::{
    Action, AnswerRefused, Authority, BlockReason, BodyRef, Decision, DecisionSource, Diagnostic,
    Finding, FixAction, Flow, FlowEvent, FlowId, FlowState, HeaderMap, HostName, HostPattern,
    HttpRequest, InvalidTransition, Matcher, Method, Rule, RuleId, Scheme, Severity, Tier,
    TransitionInput, UpstreamError, block_response, failed_response, path_prefix_is_valid,
};
use humanitl_recorder::{Dir, Recorder};
use humanitl_rules::is_known_method;
use humanitl_rules::path::{prefix_matches, strip_query};
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode, Version};
use hyper_util::rt::{TokioIo, TokioTimer};

use crate::body::{self, BufferError, ResponseBody};
use crate::ca::LeafCache;
use crate::connect::{AuthorityError, AuthorityRefusal, ConnectionContext, RequestTarget};
use crate::findings::{NoScan, Scanner};
use crate::hold::HoldQueue;
use crate::meta::{self, MetaEndpoint, MetaReply, MetaRequest};
use crate::pipeline::FlowPipeline;
use crate::registry::FlowRecord;
use crate::tls_observe::{self, HandshakeWatch};
use crate::upstream::{self, Upstream};
use crate::{connect, tls};

/// Der Grund, mit dem ein abgelehnter Übergang den Flow beendet.
///
/// Es gibt keinen Weg zum Ziel, weil der Daemon die Anfrage nicht weiterführen
/// kann; ein Grund, der einen Menschen nennt, wäre falsch, denn es hat niemand
/// entschieden.
const FAIL_CLOSED_REASON: BlockReason = BlockReason::NoRoute;

/// Der Upstream-Fehler, den ein Flow bekommt, der beim Abbruch schon
/// `Forwarded` war.
///
/// Aus `Forwarded` kennt der Automat nur `Respond` und `Fail`. `Respond` hieße,
/// das Ziel habe geantwortet — das wäre die größere Unwahrheit. Bleibt `Fail`,
/// und der Aufrufer muss den Fehler benennen, weil der Kern keinen kennt: Die
/// Verbindung stand, und wir haben sie abgebrochen. `Connect` ist davon das
/// nächstliegende. Ein eigener Wert `UpstreamError::Aborted` wäre genauer,
/// bräuchte aber ein neues Feld in `humanitl.v1` und dessen Spiegel in `app/`;
/// bis dahin trägt der Client denselben `upstream_connect` wie die
/// Aufzeichnung, sodass Antwort und Protokoll wenigstens dasselbe sagen.
///
/// Erreichbar ist das heute nur über [`FlowHandler::fail_closed`] nach einem
/// abgelehnten `Respond`, und `Respond` ist aus `Forwarded` immer erlaubt — der
/// Pfad ist also Vorsorge, kein laufender Fall.
const FAIL_CLOSED_ABORTED: UpstreamError = UpstreamError::Connect;

/// Die Caps und Fristen, die der Handler pro Verbindung und Anfrage kennt.
#[derive(Debug, Clone, Copy)]
pub struct ProxyLimits {
    /// Größter Request-Body, der für die Entscheidung gepuffert wird
    /// (`limits.hold_body_cap_bytes`). Darüber `413`.
    pub body_cap_bytes: u64,
    /// Bodies bis hierhin bleiben im Ereignis inline (`recorder.inline_max_bytes`).
    pub inline_max_bytes: u64,
    /// So lange darf ein Client für seine Kopfzeilen brauchen
    /// (`limits.header_timeout_secs`). Auf einer Keep-Alive-Verbindung ist
    /// das zugleich die Frist bis zur nächsten Anfrage.
    pub header_timeout: Duration,
    /// Blockt eine Anfrage sofort, wenn der Scan ein prüfsummen-sicheres
    /// Geheimnis findet (`hold.hard_block_checksum_secrets`).
    pub hard_block_checksum_secrets: bool,
}

impl ProxyLimits {
    /// Die Grenzen aus der Konfiguration.
    #[must_use]
    pub const fn from_config(limits: &Limits, recorder: &RecorderConfig) -> Self {
        Self {
            body_cap_bytes: limits.hold_body_cap_bytes,
            inline_max_bytes: recorder.inline_max_bytes,
            header_timeout: Duration::from_secs(limits.header_timeout_secs),
            hard_block_checksum_secrets: false,
        }
    }

    /// Dieselben Grenzen mit dem Schalter aus `hold`.
    #[must_use]
    pub const fn with_hold(mut self, hold: &HoldConfig) -> Self {
        self.hard_block_checksum_secrets = hold.hard_block_checksum_secrets;
        self
    }
}

impl Default for ProxyLimits {
    /// Die Vorgabewerte der Konfiguration.
    fn default() -> Self {
        Self::from_config(&Limits::default(), &RecorderConfig::default())
    }
}

/// Der geteilte Zustand eines Handlers; billig zu klonen (alles `Arc`).
struct Inner {
    queue: Arc<HoldQueue>,
    pipeline: Arc<dyn FlowPipeline>,
    upstream: Upstream,
    leaves: Arc<LeafCache>,
    limits: ProxyLimits,
    scanner: Arc<dyn Scanner>,
    recorder: Option<Recorder>,
    /// Der Meta-Endpunkt, falls diese Sitzung einen hat (HUM-073).
    meta: Option<Arc<MetaEndpoint>>,
    /// Das Zählfenster der gescheiterten Handschläge dieser Sitzung (HUM-045).
    handshakes: HandshakeWatch,
}

/// Die wahlfreien Anschlüsse eines Handlers.
///
/// Detektoren, Aufzeichnung und Meta-Endpunkt sind alle drei „kann, muss
/// nicht": Ein Test läuft ohne sie, der Daemon mit allen. Sie stehen deshalb
/// zusammen in einem Wert und nicht als drei weitere Stellen in einer
/// Argumentliste, die mit jedem Sprint länger würde.
pub struct HandlerPorts {
    /// Die Detektoren, die im Pfad laufen (HUM-025).
    pub scanner: Arc<dyn Scanner>,
    /// Die Aufzeichnung (HUM-026).
    pub recorder: Option<Recorder>,
    /// Der Meta-Endpunkt `humanitl.internal` (HUM-073).
    pub meta: Option<Arc<MetaEndpoint>>,
}

impl Default for HandlerPorts {
    /// Keine Detektoren ([`NoScan`]), keine Aufzeichnung, kein Meta-Endpunkt.
    fn default() -> Self {
        Self {
            scanner: Arc::new(NoScan),
            recorder: None,
            meta: None,
        }
    }
}

impl std::fmt::Debug for HandlerPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerPorts")
            .field("recorder", &self.recorder.is_some())
            .field("meta", &self.meta.is_some())
            .finish_non_exhaustive()
    }
}

/// Bedient eine Verbindung: liest Anfragen, entscheidet, leitet weiter.
#[derive(Clone)]
pub struct FlowHandler {
    inner: Arc<Inner>,
}

impl FlowHandler {
    /// Ein Handler mit allen Ports und Grenzen.
    #[must_use]
    pub fn new(
        queue: Arc<HoldQueue>,
        pipeline: Arc<dyn FlowPipeline>,
        upstream: Upstream,
        leaves: Arc<LeafCache>,
        limits: ProxyLimits,
    ) -> Self {
        Self::with_ports(
            queue,
            pipeline,
            upstream,
            leaves,
            limits,
            HandlerPorts::default(),
        )
    }

    /// Derselbe Handler mit den Detektoren aus HUM-025.
    ///
    /// Ohne diesen Weg läuft [`NoScan`]: Der Scan gehört in den Pfad, die
    /// Detektoren kennt der Proxy nicht (`docs/ARCHITECTURE.md` 2).
    #[must_use]
    pub fn with_findings(
        queue: Arc<HoldQueue>,
        pipeline: Arc<dyn FlowPipeline>,
        upstream: Upstream,
        leaves: Arc<LeafCache>,
        limits: ProxyLimits,
        scanner: Arc<dyn Scanner>,
    ) -> Self {
        Self::with_recorder(queue, pipeline, upstream, leaves, limits, scanner, None)
    }

    /// Derselbe Handler mit der Aufzeichnung aus HUM-026.
    ///
    /// Der Handler ist die eine Stelle, die die Bytes in der Hand hält:
    /// den gepufferten Anfrage-Body, die bearbeitete Anfrage und die Antwort,
    /// während sie streamt. Deshalb schreibt er sie auch auf. Alles, was ein
    /// Ereignis sagt, schreibt dagegen die Warteschlange
    /// ([`HoldQueue::recording`]).
    ///
    /// Ein Fehler der Aufzeichnung hält den Proxy nie an: Er wird als
    /// [`FlowEvent::Diagnostic`] an denselben Flow gehängt, und die Anfrage
    /// läuft weiter. Eine Anfrage zu blocken, weil die Festplatte voll ist,
    /// wäre eine Entscheidung, die niemand getroffen hat.
    #[must_use]
    pub fn with_recorder(
        queue: Arc<HoldQueue>,
        pipeline: Arc<dyn FlowPipeline>,
        upstream: Upstream,
        leaves: Arc<LeafCache>,
        limits: ProxyLimits,
        scanner: Arc<dyn Scanner>,
        recorder: Option<Recorder>,
    ) -> Self {
        Self::with_ports(
            queue,
            pipeline,
            upstream,
            leaves,
            limits,
            HandlerPorts {
                scanner,
                recorder,
                meta: None,
            },
        )
    }

    /// Derselbe Handler mit allen wahlfreien Anschlüssen, auch dem
    /// Meta-Endpunkt (HUM-073).
    ///
    /// Der Weg, den `humanitld` nimmt. Ohne [`HandlerPorts::meta`] gibt es den
    /// reservierten Host `humanitl.internal` für diese Sitzung nicht; eine
    /// Anfrage dorthin läuft dann durch die Regeln wie jeder andere Host und
    /// endet ohne Freigabe in der Warteschlange.
    #[must_use]
    pub fn with_ports(
        queue: Arc<HoldQueue>,
        pipeline: Arc<dyn FlowPipeline>,
        upstream: Upstream,
        leaves: Arc<LeafCache>,
        limits: ProxyLimits,
        ports: HandlerPorts,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                queue,
                pipeline,
                upstream,
                leaves,
                limits,
                scanner: ports.scanner,
                recorder: ports.recorder,
                meta: ports.meta,
                handshakes: HandshakeWatch::new(),
            }),
        }
    }

    /// Nimmt eine einzelne Anfrage entgegen und liefert die Antwort.
    async fn handle(
        &self,
        req: Request<Incoming>,
        meta: ConnectionContext,
    ) -> Response<ResponseBody> {
        if req.method() == Method::CONNECT {
            return self.handle_connect(req, &meta);
        }
        self.handle_request(req, meta).await
    }

    /// `CONNECT`: mit `200` bestätigen, dann die Verbindung übernehmen, TLS mit
    /// dem Leaf des Ziels terminieren und die entschlüsselte Verbindung erneut
    /// bedienen.
    ///
    /// Der Tunnel baut nichts nach oben auf: Der `CONNECT` endet am lokalen
    /// TLS-Endpunkt des Proxys. Erst eine erlaubte Anfrage darin löst einen
    /// Namen auf und verbindet (ADR-006, HUM-024); vorher darf nicht einmal
    /// der TCP-Verbindungsaufbau verraten, wohin es ginge.
    ///
    /// Der Name aus dem `ClientHello` wandert in den
    /// [`ConnectionContext`] der entschlüsselten Verbindung; dort vergleicht
    /// ihn [`check_authority`](crate::connect::check_authority) mit Tunnelziel
    /// und `Host`.
    ///
    /// Scheitert der Handschlag, endet die Verbindung hier, und der Client
    /// sieht nur seinen eigenen Fehler. Damit nicht auch der Mensch vor dem
    /// Bildschirm allein davorsteht, deutet
    /// [`FlowHandler::note_handshake_failure`] den Fehler und macht ihn als
    /// Flow und als Befund sichtbar (HUM-045). Der Proxy hält dabei nichts an
    /// und lässt nichts offen: Die Task endet, der Strom wird fallen gelassen,
    /// und der Accept-Loop nimmt die nächste Verbindung.
    fn handle_connect(
        &self,
        mut req: Request<Incoming>,
        meta: &ConnectionContext,
    ) -> Response<ResponseBody> {
        let Some(authority) = connect_authority(req.uri()) else {
            return text_response(StatusCode::BAD_REQUEST, "missing host");
        };
        let server_config = match self.inner.leaves.server_config(&authority.host) {
            Ok(config) => config,
            Err(diag) => {
                tracing::error!(?diag, host = %authority.host, "cannot mint a leaf certificate");
                return text_response(StatusCode::BAD_GATEWAY, "tls setup failed");
            }
        };

        // Die Kopfzeilen des `CONNECT` sind die einzige Stelle, an der ein
        // gescheiterter Handschlag noch etwas über den Client aussagt: Der
        // `User-Agent` steht dort und danach nirgends mehr.
        let connect_headers = req.headers().clone();
        let handler = self.clone();
        let meta = meta.clone();
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => match tls::accept(server_config, TokioIo::new(upgraded)).await {
                    Ok((stream, sni)) => {
                        handler.note_missing_sni(sni.as_ref(), &authority);
                        let inner_meta = meta.tunnel(authority, sni);
                        serve_connection(handler, stream, inner_meta).await;
                    }
                    Err(err) => {
                        handler
                            .note_handshake_failure(&meta, &authority, &connect_headers, &err)
                            .await;
                    }
                },
                Err(err) => tracing::debug!(%err, "connect upgrade failed"),
            }
        });

        let mut response = Response::new(body::empty());
        *response.status_mut() = StatusCode::OK;
        response
    }

    /// Meldet einen Handschlag ohne SNI zu einem DNS-Ziel als `TLS_003`.
    ///
    /// Der Handschlag selbst kommt zustande: Das Leaf gilt dem Ziel des
    /// `CONNECT`, nicht dem Namen aus dem `ClientHello`. Ohne SNI fehlt aber
    /// der zweite Beleg für das Ziel, und
    /// [`check_authority`](crate::connect::check_authority) lehnt jede Anfrage
    /// in dieser Verbindung ab. Der Befund sagt vorab, warum gleich alles
    /// blockt; er gehört zur Verbindung und nicht zu einem Flow, denn welcher
    /// Flow daraus wird, steht noch nicht fest.
    ///
    /// Zu einem IP-Ziel ist die fehlende SNI richtig: TLS sieht für Adressen
    /// keine vor. Dann geschieht hier nichts.
    fn note_missing_sni(&self, sni: Option<&HostName>, target: &Authority) {
        if sni.is_some() || !matches!(target.host, HostName::Dns(_)) {
            return;
        }
        if !self.inner.handshakes.on_missing_sni(&target.host) {
            return;
        }
        self.inner.queue.publish(FlowEvent::Diagnostic {
            flow_id: None,
            at: SystemTime::now(),
            diagnostic: Box::new(tls_observe::missing_sni(&target.host)),
        });
    }

    /// Deutet einen gescheiterten Handschlag und macht ihn sichtbar (HUM-045).
    ///
    /// Was sich nicht deuten lässt, bleibt eine Protokollzeile: Ein Befund, der
    /// eine Ursache erfindet, ist schlechter als keiner. Was sich deuten lässt,
    /// wird zu einem Flow — die History soll den Versuch zeigen, auch wenn nie
    /// eine Anfrage darin stand — und, wenn das Zählfenster es zulässt, zu
    /// einem Befund am selben Flow.
    async fn note_handshake_failure(
        &self,
        meta: &ConnectionContext,
        target: &Authority,
        connect_headers: &HeaderMap,
        err: &std::io::Error,
    ) {
        let Some(failure) = tls_observe::classify(err) else {
            tracing::debug!(%err, host = %target.host, "tls handshake with the client failed");
            return;
        };
        let hint = tls_observe::tool_hint(connect_headers);
        tracing::debug!(
            host = %target.host,
            failure = failure.as_str(),
            hint = hint.as_str(),
            "the client aborted the tls handshake"
        );
        let flow_id = self
            .record_connect_failure(meta, target, connect_headers)
            .await;
        // `on_rejection` liefert die Zahl der Versuche seit der letzten Karte;
        // `on_drop` nur, ob das Muster fällig ist. `TLS_002` trägt keinen
        // Zähler, weil sein Text ohnehin von Wiederholung spricht, also steht
        // dort die 1.
        let report = if failure.is_rejection() {
            self.inner.handshakes.on_rejection(&target.host, hint)
        } else {
            self.inner.handshakes.on_drop(&target.host).then_some(1)
        };
        if let Some(since_last) = report {
            self.publish_diagnostic(
                flow_id,
                tls_observe::diagnostic_for(&failure, &target.host, hint, since_last),
            );
        }
    }

    /// Verbucht den gescheiterten `CONNECT` als Flow und liefert seine Id.
    ///
    /// Der Flow trägt die Anfrage, die es wirklich gab: `CONNECT` auf das
    /// Tunnelziel, mit den Kopfzeilen des Clients. Kein Pfad — ein `CONNECT`
    /// nennt keinen (RFC 9110 §9.3.6) — und kein Body.
    ///
    /// Die Detektoren laufen auch hier. Ein `Proxy-Authorization` oder ein
    /// Token in einer eigenen Kopfzeile des `CONNECT` ist ein Geheimnis wie
    /// jedes andere; es wird aufgezeichnet, und was aufgezeichnet wird, wird
    /// durchsucht. Ein Datensatz mit `findings_count = 0`, den niemand
    /// durchsucht hat, sähe aus wie ein sauberer (`backlog/CONVENTIONS.md`
    /// 4.13). Geblockt wird deswegen nichts mehr: Der Tunnel steht ohnehin
    /// nicht, und `hold.hard_block_checksum_secrets` hat hier nichts zu
    /// entscheiden.
    ///
    /// Der Flow endet als `Decided(Block { NoRoute })` durch das System. Kein
    /// Grund, der einen Menschen nennt: Es hat niemand entschieden, der Client
    /// hat aufgelegt, und es gibt keinen Weg zum Ziel, weil der Tunnel nie
    /// stand. Woran es lag, steht in `flows.error`
    /// ([`tls_observe::FLOW_ERROR`]) und im Befund am selben Flow.
    async fn record_connect_failure(
        &self,
        meta: &ConnectionContext,
        target: &Authority,
        connect_headers: &HeaderMap,
    ) -> FlowId {
        let request = Self::build_request(
            Method::CONNECT,
            Scheme::Https,
            target.clone(),
            String::new(),
            connect_headers.clone(),
            BodyRef::empty(),
        );
        let mut flow = Flow::new(
            FlowId::new(),
            meta.session,
            SystemTime::now(),
            request.clone(),
        );
        let report = self.inner.scanner.scan(&request, &[]);
        let mut record = FlowRecord::new(&flow, meta);
        record.findings_truncated = report.truncated;
        self.inner.queue.registry().insert(record);
        self.inner.queue.publish(flow.received_event());
        // Erst nach `Received`: Vorher gibt es die Zeile nicht, die der Grund
        // fortschreibt. Beide Wege gehen durch denselben Kanal des Recorders,
        // also genügt die Reihenfolge der Aufrufe.
        if let Some(recorder) = self.inner.recorder.as_ref() {
            recorder.set_flow_error(flow.id, tls_observe::FLOW_ERROR);
        }
        if let Some(recorder) = self.inner.recorder.as_ref()
            && !report.findings.is_empty()
        {
            recorder.store_findings(flow.id, &report.findings);
        }
        self.record_message(flow.id, Dir::Request, &request.headers, Bytes::new())
            .await;
        log_findings(&flow, &report.findings, report.truncated);
        if self
            .apply(
                &mut flow,
                TransitionInput::Analyze {
                    findings: report.findings,
                },
            )
            .is_err()
        {
            return self.close_without_response(&mut flow);
        }
        // Erst `Analyzed`, dann die Befunde des Scans: Sie erklären eine Lücke
        // in der Suche und gehören an den Fund, nicht davor.
        for diagnostic in report.diagnostics {
            self.publish_diagnostic(flow.id, diagnostic);
        }
        if self
            .apply(
                &mut flow,
                TransitionInput::Decide {
                    decision: Decision::Block {
                        reason: BlockReason::NoRoute,
                        note: None,
                    },
                    source: DecisionSource::System,
                },
            )
            .is_err()
        {
            return self.close_without_response(&mut flow);
        }
        let _ = self.apply(&mut flow, TransitionInput::Record);
        flow.id
    }

    /// Bringt einen Flow ohne Antwort zu Ende, so weit der Automat es zulässt.
    ///
    /// Dieselbe Absicht wie [`FlowHandler::fail_closed`], nur ohne die
    /// Block-Antwort: Es gibt keine Verbindung mehr, an die sie ginge. Damit
    /// auch derselbe Weg durch den Automaten — ein bloßes `Record` wäre aus
    /// `Received` oder `Analyzed` kein gültiger Übergang, der Flow bliebe in
    /// der Registry stehen, und `PROXY_005` stünde ein zweites Mal im Strom.
    fn close_without_response(&self, flow: &mut Flow) -> FlowId {
        self.publish_fail_closed(flow);
        flow.id
    }

    /// Eine gewöhnliche Anfrage (Origin- oder Absolut-Form): Ziel prüfen,
    /// Body puffern, Detektoren laufen lassen, entscheiden, weiterleiten oder
    /// blocken.
    ///
    /// Die Reihenfolge ist die aus `backlog/sprint-2.md` HUM-023 und nicht
    /// verhandelbar: Erst die Konsistenz von CONNECT-Ziel, SNI und Authority,
    /// dann der Body, dann die Detektoren, dann die Regeln, und gehalten wird
    /// nur, was `ask` ergibt. Jede Stufe davor darf ohne Rückfrage ablehnen;
    /// keine darf ohne Regel oder Menschen erlauben.
    async fn handle_request(
        &self,
        req: Request<Incoming>,
        meta: ConnectionContext,
    ) -> Response<ResponseBody> {
        // Anti-Fronting (ESC-3, ADR-007): Das Ziel, zu dem die Verbindung
        // wirklich führt, und der Host, den die Anfrage nennt, müssen dasselbe
        // sein. Ein CONNECT nach github.com mit einem `Host: evil.io` darin
        // ist keine Anfrage an evil.io, sondern der Versuch, die Entscheidung
        // für den einen Host auf den anderen zu übertragen. Sie wird ohne
        // Rückfrage abgelehnt und mit dem echten Ziel als Authority verbucht.
        let RequestTarget { scheme, authority } = match connect::check_authority(&meta, &req) {
            Ok(target) => target,
            Err(AuthorityError::NoTarget(reason)) => {
                return text_response(StatusCode::BAD_REQUEST, reason);
            }
            Err(AuthorityError::Mismatch(refusal)) => {
                return self.refuse_authority(&meta, req, &refusal);
            }
        };
        let path_and_query = req
            .uri()
            .path_and_query()
            .map_or_else(|| "/".to_owned(), ToString::to_string);

        // Die Weiche zum Meta-Endpunkt: vor Body, Detektoren, Regeln und damit
        // vor jeder Auflösung. Warum genau hier: [`FlowHandler::serve_meta`].
        if let Some(endpoint) = self.meta_for(&authority) {
            return self
                .serve_meta(endpoint, req, scheme, authority, &path_and_query, &meta)
                .await;
        }
        let (parts, incoming) = req.into_parts();
        let cap = self.inner.limits.body_cap_bytes;
        let content_type =
            header_string(&parts.headers, hyper::header::CONTENT_TYPE).map(str::to_owned);

        // Ein bekanntes Content-Length über dem Cap wird geblockt, ohne den
        // Body zu lesen: der Client bekommt schnell `413`, und es fließt keine
        // Bandbreite (Fallstrick HUM-015).
        if let Some(declared) = content_length(&parts.headers)
            && declared > cap
        {
            let request = Self::build_request(
                parts.method.clone(),
                scheme,
                authority,
                path_and_query,
                parts.headers,
                placeholder_body(declared, content_type.as_deref()),
            );
            return self.refuse_before_pipeline(&meta, request, BlockReason::BodyCap, None);
        }

        let body_bytes = match body::buffer(incoming, cap).await {
            Ok(bytes) => bytes,
            Err(BufferError::Cap) => {
                let request = Self::build_request(
                    parts.method.clone(),
                    scheme,
                    authority,
                    path_and_query,
                    parts.headers,
                    placeholder_body(cap, content_type.as_deref()),
                );
                return self.refuse_before_pipeline(&meta, request, BlockReason::BodyCap, None);
            }
            Err(BufferError::Read) => {
                return text_response(StatusCode::BAD_REQUEST, "request body read error");
            }
        };

        let body_ref = self.body_ref(&body_bytes, content_type.as_deref());
        let request = Self::build_request(
            parts.method,
            scheme,
            authority,
            path_and_query,
            parts.headers,
            body_ref,
        );

        let mut flow = Flow::new(
            FlowId::new(),
            meta.session,
            SystemTime::now(),
            request.clone(),
        );
        self.inner.queue.publish(flow.received_event());
        // Der Body ist gepuffert und die Zeile des Flows steht (das `Received`
        // oben ging durch die Aufzeichnung): jetzt, und nicht später, wird die
        // Anfrage aufgezeichnet. Später hieße: nach einer Entscheidung, und
        // dann fehlte gerade die Anfrage, die jemand abgelehnt hat.
        self.record_message(flow.id, Dir::Request, &request.headers, body_bytes.clone())
            .await;
        if let Err(response) = self.analyze(&mut flow, &meta, &request, &body_bytes) {
            return response;
        }

        let decision = self.inner.pipeline.decide(&mut flow, &meta).await;
        self.carry_out(decision, flow, request, body_bytes, &meta)
            .await
    }

    /// Führt aus, was entschieden wurde.
    ///
    /// Die zweite Hälfte von [`FlowHandler::handle_request`], ab dem Punkt, an
    /// dem die Entscheidung feststeht: weiterleiten, bearbeitet weiterleiten,
    /// oder die Sperre bauen und den Flow abschließen.
    async fn carry_out(
        &self,
        decision: Decision,
        mut flow: Flow,
        request: HttpRequest,
        body_bytes: Bytes,
        meta: &ConnectionContext,
    ) -> Response<ResponseBody> {
        match decision {
            Decision::Allow => self.forward(flow, request, body_bytes, meta).await,
            Decision::AllowEdited { request: edited } => {
                // Die Bearbeitung darf Methode, Pfad, Kopfzeilen und Body
                // aendern, aber nie das Ziel: Entschieden wurde fuer genau
                // diese Authority unter genau diesem Schema, und eine
                // bearbeitete Anfrage an einen anderen Host waere ein Egress,
                // den kein Mensch freigegeben hat. Das Schema gehoert zum
                // Ziel: `https` nach `http` umgeschrieben ginge an denselben
                // Host, aber im Klartext, und niemand hat einer Herabstufung
                // zugestimmt. Das ist dieselbe Aussage wie die Pruefung der
                // eingehenden Verbindung, nur in die andere Richtung.
                let edited = *edited;
                if edited.authority != flow.request.authority
                    || edited.scheme != flow.request.scheme
                {
                    return self.revise_to_block(&mut flow, BlockReason::AuthorityMismatch);
                }
                let edited_body = edited
                    .body
                    .inline
                    .clone()
                    .unwrap_or_else(|| body_bytes.clone());
                // Die bearbeitete Anfrage steht neben der ursprünglichen, nicht
                // an ihrer Stelle: Die History zeigt beide, sonst ließe sich
                // nicht mehr sehen, was der Mensch geändert hat.
                self.record_message(
                    flow.id,
                    Dir::RequestEdited,
                    &edited.headers,
                    edited_body.clone(),
                )
                .await;
                self.forward(flow, edited, edited_body, meta).await
            }
            Decision::Block { reason, note } => {
                self.record_block(&mut flow, reason, note.as_deref())
            }
            Decision::TimedOut => self.record_block(&mut flow, BlockReason::Timeout, None),
        }
    }

    /// Der Meta-Endpunkt dieser Sitzung, falls die Anfrage an ihn geht.
    ///
    /// Geprüft wird die [`Authority`] aus
    /// [`connect::check_authority`](crate::connect::check_authority), nie der
    /// `Host`-Kopf für sich: Ein `CONNECT github.com:443` mit
    /// `Host: humanitl.internal` darin ist ein Widerspruch und wird vorher
    /// geblockt. Sonst wäre die Weiche über eine Kopfzeile steuerbar.
    fn meta_for(&self, authority: &Authority) -> Option<&MetaEndpoint> {
        self.inner
            .meta
            .as_deref()
            .filter(|_| meta::is_meta_host(&authority.host))
    }

    /// Beantwortet eine Anfrage an `humanitl.internal` selbst (HUM-073).
    ///
    /// **Warum die Weiche dorthin vor der Regelauswertung liegt.** Der Name
    /// ist reserviert: Er wird nie aufgelöst und nie an einen Upstream
    /// weitergereicht (ADR-014). Läge die Weiche später, stünde
    /// `humanitl.internal` als `ask` in der Warteschlange, und eine Freigabe
    /// dafür löste eine Namensauflösung aus — für einen Namen, den es
    /// nirgends gibt. Ein `CONNECT humanitl.internal:443` kommt über denselben
    /// Weg hier an: Der Tunnel endet am eigenen TLS-Endpunkt, und die Anfrage
    /// darin läuft durch dieselbe Prüfung wie Klartext.
    ///
    /// Der entstehende [`Flow`] trägt **keine** Entscheidung. Die Anfrage geht
    /// nirgendwo hin, hält nichts auf, und über sie entscheidet niemand; ein
    /// Datensatz mit einer Entscheidung, die es nicht gab, wäre eine Behauptung
    /// über einen Menschen, der nichts getan hat (`backlog/CONVENTIONS.md`
    /// 4.13). Er geht deshalb über [`TransitionInput::Answer`] unmittelbar von
    /// `Received` nach `Recorded`, den einen Weg, der keine Sperre verlangt,
    /// und trägt in der Aufzeichnung den Vermerk `meta` statt einer
    /// Entscheidung (HUM-103). Die Bitte aus `/ask` bleibt zusätzlich, was sie
    /// war: [`FlowEvent::AgentAsk`] im Ereignisstrom und eine Karte in der
    /// Oberfläche.
    ///
    /// Der Body wird nur bis [`meta::ASK_BODY_CAP_BYTES`] gelesen, nicht bis
    /// `limits.hold_body_cap_bytes`: Für `/ask` gilt die kleinere Grenze aus
    /// ADR-014, und alles darüber ist `413`, ohne dass die Bytes fließen.
    async fn serve_meta(
        &self,
        endpoint: &MetaEndpoint,
        req: Request<Incoming>,
        scheme: Scheme,
        authority: Authority,
        path_and_query: &str,
        conn: &ConnectionContext,
    ) -> Response<ResponseBody> {
        let (parts, incoming) = req.into_parts();
        let declared_over_cap =
            content_length(&parts.headers).is_some_and(|len| len > meta::ASK_BODY_CAP_BYTES);
        let (body, over_cap) = if declared_over_cap {
            (Bytes::new(), true)
        } else {
            match body::buffer(incoming, meta::ASK_BODY_CAP_BYTES).await {
                Ok(bytes) => (bytes, false),
                Err(BufferError::Cap) => (Bytes::new(), true),
                Err(BufferError::Read) => {
                    return text_response(StatusCode::BAD_REQUEST, "request body read error");
                }
            }
        };
        let outcome = endpoint.respond(
            &MetaRequest {
                method: &parts.method,
                path_and_query,
                body: &body,
                body_over_cap: over_cap,
                session: conn.session,
            },
            self.inner.queue.registry(),
        );
        // Der gesäuberte Text der Bitte, bevor das Ereignis weiterzieht. Er ist
        // das Einzige, was von einem `/ask` in die Aufzeichnung geht: Der rohe
        // Rumpf des Agenten wird nie gespeichert, und die Antwort des
        // Endpunkts auch nicht.
        let ask_text = match &outcome.event {
            Some(FlowEvent::AgentAsk { text, .. }) => Bytes::from(text.clone()),
            _other => Bytes::new(),
        };
        self.record_meta(
            conn,
            scheme,
            authority,
            &parts.method,
            path_and_query,
            &parts.headers,
            ask_text,
            outcome.reply.status,
        )
        .await;
        if let Some(event) = outcome.event {
            self.inner.queue.publish(event);
        }
        tracing::debug!(
            session = %conn.session,
            method = %parts.method,
            path = path_and_query,
            status = outcome.reply.status,
            "the meta endpoint answered"
        );
        meta_to_response(&outcome.reply)
    }

    /// Schreibt die Meta-Anfrage in die Aufzeichnung, ohne eine Entscheidung zu
    /// erfinden (HUM-103).
    ///
    /// **Warum nicht über [`HoldQueue::publish`](crate::hold::HoldQueue::publish).**
    /// Der Fluss ist fertig, bevor ein Zuhörer etwas mit ihm anfangen könnte:
    /// Der Proxy hat schon geantwortet, es gibt nichts zu halten, zu
    /// entscheiden oder zu übergeben. Er gehört deshalb nicht in die
    /// [`FlowRegistry`](crate::registry::FlowRegistry) — die führt die Flows
    /// dieser Sitzung, über die noch entschieden werden kann, und `/why`
    /// beantwortet genau die. Ein `Received` im Ereignisstrom hätte zudem eine
    /// Zeile behauptet, die kein Zuhörer je vollständig sähe. Sichtbar wird der
    /// Fluss in der Historie, also dort, wo steht, was geschehen ist.
    ///
    /// Aufgezeichnet werden Kopfzeilen und, bei `/ask`, der **gesäuberte** Text
    /// der Bitte. Der Rumpf der Antwort wird nie aufgezeichnet: Was der
    /// Endpunkt sagt, steht ohnehin in der Oberfläche, und die Aufzeichnung
    /// ist für das da, was hinausging.
    #[expect(
        clippy::too_many_arguments,
        reason = "die Zeile der Aufzeichnung braucht jedes dieser Felder; ein Bündel dafür wäre ein                   Typ, den sonst niemand benutzt"
    )]
    async fn record_meta(
        &self,
        conn: &ConnectionContext,
        scheme: Scheme,
        authority: Authority,
        method: &Method,
        path_and_query: &str,
        headers: &HeaderMap,
        body: Bytes,
        status: u16,
    ) {
        let Some(recorder) = self.inner.recorder.as_ref() else {
            return;
        };
        let request = Self::build_request(
            method.clone(),
            scheme,
            authority,
            path_and_query.to_owned(),
            headers.clone(),
            BodyRef::from_bytes(body.clone()),
        );
        let mut flow = Flow::new(FlowId::new(), conn.session, SystemTime::now(), request);
        // Erst abschließen, dann anlegen. `Flow::answer` prüft die Anfrage
        // **dieses** Flusses; sie ist der einzige Weg von `Received` nach
        // `Recorded` ohne Entscheidung. Scheitert sie, ist die Weiche zum
        // Endpunkt kaputt, und es entsteht keine halbe Zeile in der
        // Aufzeichnung.
        let closed = match flow.answer(SystemTime::now()) {
            Ok(event) => event,
            Err(AnswerRefused::NotMeta(diagnostic)) => {
                tracing::error!(
                    code = diagnostic.code.as_str(),
                    why = %diagnostic.why,
                    "the meta switch offered a request that is not a meta request"
                );
                self.inner.queue.publish(FlowEvent::Diagnostic {
                    flow_id: None,
                    at: SystemTime::now(),
                    diagnostic,
                });
                return;
            }
            Err(AnswerRefused::State(err)) => {
                // Unerreichbar, solange `Flow::new` in `Received` beginnt; der
                // Befund steht trotzdem, damit ein späterer Umbau nicht still
                // eine halbe Zeile hinterlässt.
                self.publish_invalid_transition(&flow, err);
                return;
            }
        };
        recorder.apply(&flow.received_event());
        recorder.apply(&closed);
        recorder.set_meta_answer(flow.id, status);
        self.record_message(flow.id, Dir::Request, headers, body)
            .await;
    }

    /// Der Scan und alles, was aus ihm folgt: `Analyzed`, die Befunde, der
    /// Datensatz, und der harte Block.
    ///
    /// Der Scan läuft über die vollständige Anfrage und vor jeder Regel: Was
    /// gefunden wurde, steht in `Analyzed` und damit vor dem Menschen, bevor
    /// irgendetwas entschieden ist.
    ///
    /// # Errors
    ///
    /// Die fertige Antwort, wenn der Flow hier schon endet: fail-closed nach
    /// einem abgelehnten Übergang, oder der harte Block auf ein
    /// prüfsummen-sicheres Geheimnis.
    fn analyze(
        &self,
        flow: &mut Flow,
        meta: &ConnectionContext,
        request: &HttpRequest,
        body: &[u8],
    ) -> Result<(), Response<ResponseBody>> {
        let report = self.inner.scanner.scan(request, body);
        let truncated = report.truncated;
        let checksum_secret = report
            .findings
            .iter()
            .any(|finding| finding.tier == Tier::Checksum);
        log_findings(flow, &report.findings, truncated);
        // Ohne Fund gibt es nichts aufzuschreiben: Die Zahl der Funde trägt
        // die Zeile des Flows ohnehin, und sie ist dann null.
        if let Some(recorder) = self.inner.recorder.as_ref()
            && !report.findings.is_empty()
        {
            recorder.store_findings(flow.id, &report.findings);
        }
        if self
            .apply(
                flow,
                TransitionInput::Analyze {
                    findings: report.findings,
                },
            )
            .is_err()
        {
            return Err(self.fail_closed(flow));
        }
        // Erst `Analyzed`, dann die Befunde des Scans: Sie erklären eine Lücke
        // in der Suche (`FINDINGS_002`) und gehören deshalb an den Fund, nicht
        // davor.
        for diagnostic in report.diagnostics {
            self.publish_diagnostic(flow.id, diagnostic);
        }

        // Der Datensatz entsteht hier, an der einen Stelle, die den Bericht
        // des Scans kennt: Er trägt `findings_truncated`, und er steht in der
        // Registry, bevor irgendein `Held` veröffentlicht wird.
        let mut record = FlowRecord::new(flow, meta);
        record.findings_truncated = truncated;
        self.inner.queue.registry().insert(record);

        // `hold.hard_block_checksum_secrets`: Ein Fund, den eine Prüfsumme
        // bestätigt, ist kein Verdacht. Wer den Schalter setzt, hat im Voraus
        // entschieden, dass so etwas den Rechner nicht verlässt; gefragt wird
        // dann nicht mehr.
        if self.inner.limits.hard_block_checksum_secrets && checksum_secret {
            return Err(self.block_checksum_secret(flow));
        }
        Ok(())
    }

    /// Blockt einen Flow, in dem ein prüfsummen-sicheres Geheimnis steckt
    /// (`hold.hard_block_checksum_secrets`).
    ///
    /// Das System entscheidet, ohne zu fragen; erlaubt darf es nie, und hier
    /// lehnt es ab. Der Grund heißt `secret` und nicht `user`: Es hat niemand
    /// entschieden, und eine Antwort, die einen Menschen nennt, den es nicht
    /// gab, wäre eine Unwahrheit gegenüber dem Agenten und dem Protokoll
    /// (`backlog/CONVENTIONS.md` 4.13). Die Notiz sagt, was passiert ist, ohne
    /// den Wert zu nennen — der steht in keiner Meldung, nur sein Hash.
    fn block_checksum_secret(&self, flow: &mut Flow) -> Response<ResponseBody> {
        let reason = BlockReason::Secret;
        let note = "a checksum-confirmed secret was found in this request and \
                    hold.hard_block_checksum_secrets is on";
        let decision = Decision::Block {
            reason,
            note: Some(note.to_owned()),
        };
        if self
            .apply(
                flow,
                TransitionInput::Decide {
                    decision,
                    source: DecisionSource::System,
                },
            )
            .is_err()
        {
            return self.fail_closed(flow);
        }
        self.record_block(flow, reason, Some(note))
    }

    /// Zeichnet eine vollständig gepufferte Nachricht auf.
    ///
    /// Ohne Aufzeichnung geschieht nichts. Scheitert sie, ist das ein Befund am
    /// Flow und kein Grund, die Anfrage anzuhalten: Der Mensch sieht die Lücke
    /// an derselben Stelle wie den Flow, um den es geht
    /// (`backlog/CONVENTIONS.md` 4.13).
    async fn record_message(&self, flow: FlowId, dir: Dir, headers: &HeaderMap, body: Bytes) {
        let Some(recorder) = self.inner.recorder.as_ref() else {
            return;
        };
        if let Err(error) = recorder.store_message(flow, dir, headers, body).await {
            let diagnostic = error.into_diagnostic();
            tracing::warn!(
                %flow,
                dir = dir.as_str(),
                code = diagnostic.code.as_str(),
                why = %diagnostic.why,
                "the request could not be recorded; it is handled anyway"
            );
            self.publish_diagnostic(flow, diagnostic);
        }
    }

    /// Hängt einen Befund an einen Flow.
    fn publish_diagnostic(&self, flow_id: FlowId, diagnostic: Diagnostic) {
        self.inner.queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(flow_id),
            at: SystemTime::now(),
            diagnostic: Box::new(diagnostic),
        });
    }

    /// Lehnt eine Anfrage ab, deren Angaben zum Ziel sich widersprechen.
    ///
    /// Der Body wird nicht gelesen und nichts wird weitergeleitet; der Flow
    /// trägt das echte Ziel, und der Befund [`PROXY_002`](humanitl_core::diagnostics::codes::PROXY_002)
    /// nennt beide Seiten des Widerspruchs im Ereignisstrom.
    fn refuse_authority(
        &self,
        meta: &ConnectionContext,
        req: Request<Incoming>,
        refusal: &AuthorityRefusal,
    ) -> Response<ResponseBody> {
        let path_and_query = req
            .uri()
            .path_and_query()
            .map_or_else(|| "/".to_owned(), ToString::to_string);
        let (parts, _incoming) = req.into_parts();
        let request = Self::build_request(
            parts.method,
            refusal.target.scheme,
            refusal.target.authority.clone(),
            path_and_query,
            parts.headers,
            BodyRef::empty(),
        );
        let reason = refusal.reason();
        self.refuse_before_pipeline(meta, request, reason, Some(refusal.diagnostic()))
    }

    /// Leitet eine erlaubte Anfrage weiter und streamt die Antwort zurück.
    async fn forward(
        &self,
        mut flow: Flow,
        request: HttpRequest,
        body: Bytes,
        meta: &ConnectionContext,
    ) -> Response<ResponseBody> {
        // Fail-closed vor dem Egress: kann der Flow nicht nach `Forwarded`, geht
        // die Anfrage nicht hinaus.
        if self.apply(&mut flow, TransitionInput::Forward).is_err() {
            return self.fail_closed(&mut flow);
        }
        match self
            .inner
            .upstream
            // Die Verbindung darf private Ziele erlauben (Test-Hook), und die
            // Regel darf es auch (ADR-006). Beides zusammen entscheidet.
            .forward(&request, body, meta.allow_private || flow.allow_private)
            .await
        {
            Ok(upstream) => {
                let status = upstream.status().as_u16();
                if self
                    .apply(&mut flow, TransitionInput::Respond { status })
                    .is_err()
                {
                    return self.fail_closed(&mut flow);
                }
                let (mut parts, incoming) = upstream.into_parts();
                upstream::strip_hop_by_hop(&mut parts.headers);
                // Aufgezeichnet werden die Kopfzeilen, die der Agent zu sehen
                // bekommt, also die nach dem Entfernen der Hop-by-Hop-Zeilen:
                // Die History soll zeigen, was ankam, nicht was auf der
                // Leitung zwischen Proxy und Ziel stand.
                let sink = self
                    .inner
                    .recorder
                    .as_ref()
                    .map(|recorder| recorder.begin_response(flow.id, &parts.headers));
                // Der Tee reicht die Antwort ungepuffert durch, schiebt jedes
                // Stück in den Mitschnitt und schließt den Flow beim letzten
                // Frame mit `Record` ab.
                let body = body::tee(incoming, flow, Arc::clone(&self.inner.queue), sink, status);
                Response::from_parts(parts, body)
            }
            Err(error) => self.record_failure(&mut flow, error),
        }
    }

    /// Baut einen [`Flow`] für eine schon vor der Pipeline abgelehnte Anfrage
    /// (Body über Cap, widersprüchliche Authority): Received, optional der
    /// Befund, Analyzed, Decided(Block) durch das System, dann die Antwort
    /// samt `Recorded`.
    ///
    /// Dieser Flow erreicht keine Pipeline und trägt sich deshalb selbst in die
    /// [`FlowRegistry`](crate::registry::FlowRegistry) ein; sonst fehlte er in
    /// `ListFlows`.
    fn refuse_before_pipeline(
        &self,
        meta: &ConnectionContext,
        request: HttpRequest,
        reason: BlockReason,
        diagnostic: Option<Diagnostic>,
    ) -> Response<ResponseBody> {
        let mut flow = Flow::new(FlowId::new(), meta.session, SystemTime::now(), request);
        self.inner
            .queue
            .registry()
            .insert(FlowRecord::new(&flow, meta));
        self.inner.queue.publish(flow.received_event());
        if let Some(diagnostic) = diagnostic {
            self.publish_diagnostic(flow.id, diagnostic);
        }
        if self
            .apply(&mut flow, TransitionInput::Analyze { findings: vec![] })
            .is_err()
        {
            return self.fail_closed(&mut flow);
        }
        if self
            .apply(
                &mut flow,
                TransitionInput::Decide {
                    decision: Decision::Block { reason, note: None },
                    source: DecisionSource::System,
                },
            )
            .is_err()
        {
            return self.fail_closed(&mut flow);
        }
        self.record_block(&mut flow, reason, None)
    }

    /// Schließt einen geblockten Flow ab (`Record`) und baut die Block-Antwort.
    ///
    /// Scheitert der Abschluss, steht `PROXY_005` im Strom; die Antwort bleibt
    /// die Block-Antwort, denn geblockt wird ohnehin.
    fn record_block(
        &self,
        flow: &mut Flow,
        reason: BlockReason,
        note: Option<&str>,
    ) -> Response<ResponseBody> {
        let _ = self.apply(flow, TransitionInput::Record);
        let block = block_response(reason, flow.id, &flow.request.authority.host, note);
        block_to_response(&block, flow.id)
    }

    /// Verwandelt eine bereits getroffene Freigabe in eine Sperre (nur das
    /// System darf das, und nur bevor etwas weitergeleitet wurde), schließt
    /// den Flow ab und baut die Block-Antwort.
    fn revise_to_block(&self, flow: &mut Flow, reason: BlockReason) -> Response<ResponseBody> {
        let _ = self.apply(
            flow,
            TransitionInput::Decide {
                decision: Decision::Block { reason, note: None },
                source: DecisionSource::System,
            },
        );
        self.record_block(flow, reason, None)
    }

    /// Schließt einen gescheiterten Flow ab (`Record`) und baut die
    /// `502`-Antwort.
    ///
    /// Eine abgelehnte private Zieladresse bekommt hier ihren Befund
    /// [`PROXY_008`], und nur hier: Diese Stelle sieht jede Ablehnung, gleich
    /// ob die Adresse aus einer Auflösung kam oder als IP-Literal schon in der
    /// Anfrage stand, und sie hat den Fluss zur Hand, aus dem der Regelvorschlag
    /// entsteht (HUM-102).
    fn record_failure(&self, flow: &mut Flow, error: UpstreamError) -> Response<ResponseBody> {
        let _ = self.apply(flow, TransitionInput::Fail { error });
        if let UpstreamError::PrivateAddress(ip) = error {
            self.publish_diagnostic(flow.id, private_address_refused(&flow.request, ip));
        }
        let _ = self.apply(flow, TransitionInput::Record);
        let block = failed_response(error, flow.id, &flow.request.authority.host);
        block_to_response(&block, flow.id)
    }

    /// Wendet einen Übergang an und veröffentlicht sein Ereignis.
    ///
    /// # Errors
    ///
    /// [`InvalidTransition`], wenn der Automat das Paar aus Zustand und Eingabe
    /// nicht kennt. Zustand und Historie des Flows bleiben unverändert, der
    /// Befund [`PROXY_005`] steht dann schon im Ereignisstrom, und der Aufrufer
    /// beendet den Flow mit [`FlowHandler::fail_closed`].
    fn apply(&self, flow: &mut Flow, input: TransitionInput) -> Result<(), InvalidTransition> {
        let name = input.name();
        match flow.apply(input, SystemTime::now()) {
            Ok(event) => {
                self.inner.queue.publish(event);
                Ok(())
            }
            Err(err) => {
                tracing::error!(flow = %flow.id, transition = name, %err, "invalid transition; blocking the flow");
                self.publish_invalid_transition(flow, err);
                Err(err)
            }
        }
    }

    /// Meldet einen abgelehnten Übergang als `PROXY_005` im Ereignisstrom.
    fn publish_invalid_transition(&self, flow: &Flow, err: InvalidTransition) {
        let diagnostic = Diagnostic::builder(PROXY_005, Severity::Error)
            .why(format!(
                "flow {id} is in state {from} and cannot take the transition {input}; \
                 the request is blocked instead of continuing in an unknown state",
                id = flow.id,
                from = err.from,
                input = err.input,
            ))
            .build();
        self.inner.queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(diagnostic),
        });
    }

    /// Beendet einen Flow, dessen Übergang der Automat abgelehnt hat.
    ///
    /// Fail-closed: der Flow geht so weit, wie der Automat es noch zulässt
    /// (`Decided(Block { NoRoute })`, dann `Recorded`), der Client bekommt die
    /// Block-Antwort, und die Anfrage erreicht das Ziel nicht.
    ///
    /// Welche Übergänge das sind, weiß allein [`Flow::fail_closed`] im Kern.
    /// Der Handler kennt keine eigene Tabelle: Er würde sie sonst neben
    /// [`FlowState::on`] pflegen, und beide liefen auseinander. Der Befund
    /// `PROXY_005` liegt zu diesem Zeitpunkt schon im Strom; die Übergänge
    /// gehen deshalb an [`FlowHandler::apply`] vorbei, damit derselbe Fehler
    /// nicht ein zweites Mal gemeldet wird.
    fn fail_closed(&self, flow: &mut Flow) -> Response<ResponseBody> {
        self.publish_fail_closed(flow);
        // Die Antwort sagt dasselbe wie die Aufzeichnung. Aus zehn der elf
        // Zustände endet der Flow als `Decided(Block { NoRoute })`, und der
        // Client bekommt `no_route`. Aus `Forwarded` endet er als `Failed`,
        // weil der Automat von dort keinen anderen Weg kennt und eine Sperre
        // die Unwahrheit wäre — die Anfrage ist schon draußen. Dann bekommt der
        // Client auch `upstream_*` und nicht `no_route`; sonst läse ein Mensch
        // im Protokoll einen Verbindungsfehler und in der Antwort „keine
        // Route".
        let response = match &flow.state {
            FlowState::Failed { error } => {
                failed_response(*error, flow.id, &flow.request.authority.host)
            }
            _ => block_response(
                FAIL_CLOSED_REASON,
                flow.id,
                &flow.request.authority.host,
                None,
            ),
        };
        block_to_response(&response, flow.id)
    }

    /// Beendet den Flow fail-closed und veröffentlicht die Ereignisse.
    ///
    /// Die gemeinsame Hälfte von [`FlowHandler::fail_closed`] und
    /// [`FlowHandler::close_without_response`]: derselbe Weg durch den
    /// Automaten, einmal mit und einmal ohne Antwort an den Client.
    fn publish_fail_closed(&self, flow: &mut Flow) {
        for event in flow.fail_closed(FAIL_CLOSED_REASON, FAIL_CLOSED_ABORTED, SystemTime::now()) {
            self.inner.queue.publish(event);
        }
    }

    fn build_request(
        method: Method,
        scheme: Scheme,
        authority: Authority,
        path_and_query: String,
        headers: humanitl_core::HeaderMap,
        body: BodyRef,
    ) -> HttpRequest {
        HttpRequest::new(method, scheme, authority, path_and_query)
            .with_headers(headers)
            .with_body(body)
    }

    fn body_ref(&self, bytes: &Bytes, content_type: Option<&str>) -> BodyRef {
        let mut body = BodyRef::from_bytes(bytes.clone());
        if let Some(content_type) = content_type {
            body = body.with_content_type(content_type);
        }
        if body.size > self.inner.limits.inline_max_bytes {
            // Zu groß fürs Ereignis: der Verweis behält Hash und Größe, der
            // Inhalt bleibt beim Handler bis zur Weiterleitung.
            body.inline = None;
        }
        body
    }
}

/// Bedient eine ganze Verbindung, bis sie endet.
///
/// Für die entschlüsselte Verbindung nach einem `CONNECT` ruft der Handler
/// sich selbst rekursiv auf, mit dem Kontext des Tunnels (Ziel und SNI).
pub async fn serve_connection<I>(handler: FlowHandler, io: I, meta: ConnectionContext)
where
    I: crate::egress::AsyncStream + 'static,
{
    let header_timeout = handler.inner.limits.header_timeout;
    let service = service_fn(move |req: Request<Incoming>| {
        let handler = handler.clone();
        let meta = meta.clone();
        async move { Ok::<Response<ResponseBody>, Infallible>(handler.handle(req, meta).await) }
    });

    let result = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(header_timeout)
        .preserve_header_case(true)
        .title_case_headers(true)
        .serve_connection(TokioIo::new(io), service)
        .with_upgrades()
        .await;
    if let Err(err) = result {
        tracing::debug!(%err, "connection ended with an error");
    }
}

/// Das Ziel eines `CONNECT`: Host und Port aus der Authority-Form-URI.
fn connect_authority(uri: &hyper::Uri) -> Option<Authority> {
    let authority = uri.authority()?;
    let host = HostName::parse(authority.host()).ok()?;
    let port = authority.port_u16().unwrap_or(443);
    Some(Authority::new(host, port))
}

/// Der Wert eines Kopfes als `&str`, falls vorhanden und gültiges UTF-8.
fn header_string(
    headers: &humanitl_core::HeaderMap,
    name: hyper::header::HeaderName,
) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// Das angekündigte `Content-Length`, falls es eindeutig ist.
fn content_length(headers: &humanitl_core::HeaderMap) -> Option<u64> {
    headers
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
}

/// Ein Body-Verweis für eine Anfrage, deren Inhalt der Proxy nie liest (Body
/// über Cap): Größe bekannt, Inhalt weg, als abgeschnitten markiert.
fn placeholder_body(size: u64, content_type: Option<&str>) -> BodyRef {
    let mut body = BodyRef::detached([0u8; 32], size);
    body.truncated = true;
    if let Some(content_type) = content_type {
        body = body.with_content_type(content_type);
    }
    body
}

/// Warum es zu einer Anfrage keinen Regelvorschlag gibt.
///
/// Beide Fälle sind derselbe Gedanke: Ein Vorschlag, den ein Mensch anklickt,
/// muss danach wirken. Was hier steht, kommt in den `why` von [`PROXY_008`],
/// damit der Mensch nicht auf einen Knopf wartet, den es nicht gibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoRule {
    /// Die Anfrage nennt Port `0`. `parse_rules` lehnt ihn ab und verwirft
    /// dabei die ganze Datei; den Port wegzulassen öffnete jeden Port desselben
    /// Hosts.
    PortZero,
    /// Die Methode gehört nicht zu den bekannten. Eine Anfrage mit einer
    /// unbekannten Methode trifft überhaupt keine Regel, die vorgeschlagene
    /// also auch nicht.
    UnknownMethod,
}

/// Der Befund [`PROXY_008`] zu einer abgelehnten privaten Zieladresse.
///
/// Er erklärt den Widerspruch, an dem der Mensch sonst hängen bleibt: Er hat
/// die Anfrage freigegeben, und sie ist trotzdem gescheitert. Das Recht auf ein
/// privates Ziel hängt an einer Regel und nicht an einer Entscheidung
/// (ADR-006); der Befund nennt deshalb die Adresse und legt die Regel bei, die
/// fehlt.
///
/// Der `why` sagt nicht nur, was die Regel tut, sondern auch, wie weit sie
/// reicht. Wo die Anfrage keinen engen Zuschnitt hergibt, steht das da:
///
/// - Ohne brauchbares Pfadpräfix (`GET /`) gilt die Regel für **jeden** Pfad
///   dieses Hosts. Der Vorschlag bleibt trotzdem, denn die Wurzel eines
///   Dienstes ist der Normalfall und nicht der Ausnahmefall, und die Regel gibt
///   nichts frei: Sie öffnet ein Ziel, und jede Anfrage dorthin wird weiterhin
///   gehalten. Verschwiegen wird die Weite nicht.
/// - Zu einem Port `0` und zu einer unbekannten Methode gibt es **keinen**
///   Vorschlag, weil sich dafür keine Regel anlegen ließe, die wirkt (siehe
///   [`NoRule`]). Der `why` nennt dann den Grund statt eines Knopfes.
///
/// Die Adresse geht ausschließlich in diesen Befund und in `resolved_ip` des
/// `Failed`-Ereignisses, also an die Oberfläche. Sie steht nicht im Rumpf der
/// `502`-Antwort und in keiner Kopfzeile: Die Sandbox hat keinen Resolver, der
/// Agent kennt also nur den Namen, und die Zuordnung dieses Namens zu einer
/// privaten Adresse wäre für ihn neue Information über das lokale Netz
/// (`docs/SECURITY.md`, erste Garantie).
#[must_use]
pub fn private_address_refused(request: &HttpRequest, ip: IpAddr) -> Diagnostic {
    let mut why = format!(
        "{host} points to {ip}, an address in a private network, and no rule opens that target. \
         The request was refused after it had been allowed: the permission for private targets \
         belongs to a rule, not to a single decision (ADR-006). ",
        host = request.authority.host.display(),
    );
    let rule = private_address_rule(request);
    match &rule {
        Ok(rule) => {
            why.push_str(
                "The suggested rule opens this one target and keeps asking you about every \
                 request to it.",
            );
            if rule.matcher.path_prefixes.is_empty() {
                why.push_str(
                    " It carries no path prefix, because this request has none that would draw a \
                     boundary, so it covers every path of this host on this port.",
                );
            }
            if rule.matcher.upgrade.is_some() {
                why.push_str(
                    " It covers the protocol upgrade of this request; ordinary requests to the \
                     same host stay outside it and need a rule of their own.",
                );
            }
            why.push_str(
                " Put it in front of your other rules for this host: the first matching rule \
                 decides, so a rule added at the end never gets its turn while an older one \
                 already matches this host without allowing private targets. A rule that only \
                 lasts for this session is checked before every permanent rule, so a permanent \
                 rule cannot overtake it at all — change that one instead.",
            );
        }
        Err(NoRule::PortZero) => why.push_str(
            "There is no rule to suggest: the request names port 0, which is not a port a rule \
             can carry (the range is 1..=65535) and not a port anything listens on. Check the \
             address the agent asked for.",
        ),
        Err(NoRule::UnknownMethod) => {
            use core::fmt::Write as _;
            // `write!` statt `push_str(&format!(..))`: kein zweiter Puffer.
            let _ = write!(
                why,
                "There is no rule to suggest: the request uses the method {method}, which the \
                 rule engine does not know. A request with an unknown method matches no rule at \
                 all, so any rule offered here would be one that never takes effect. Check what \
                 the agent is doing.",
                method = request.method,
            );
        }
    }
    let mut diagnostic = Diagnostic::builder(PROXY_008, Severity::Error).why(why);
    if let Ok(rule) = rule {
        diagnostic = diagnostic.fix(FixAction::AddRule(Box::new(rule)));
    }
    diagnostic.build()
}

/// Die Regel, die [`private_address_refused`] vorschlägt.
///
/// `action: ask` mit `allow_private: true`: Das Ziel wird geöffnet, die Aufsicht
/// bleibt. Ein `allow` wäre der bequemere, aber falsche Vorschlag — er machte
/// aus „ein Mensch gibt diese eine Anfrage frei" dauerhaft „jede künftige
/// Anfrage an diesen Host geht ungefragt hinaus", also mehr Öffnung als die
/// Freigabe, die gerade gescheitert ist.
///
/// # Der Vorschlag muss durch `parse_rules` passen
///
/// Ein Klick auf den Fix schreibt diese Regel in die `rules.yaml` des Nutzers,
/// und ein einziger Wert außerhalb des Wertebereichs, den `parse_rules` kennt,
/// lehnt **die ganze Datei** ab — der Nutzer verlöre alle seine Regeln. Ein
/// Agent, der eine Anfrage frei formt, dürfte das sonst auslösen. Für jedes
/// Feld, das hier aus der Anfrage in die Regel wandert, steht deshalb der
/// Wertebereich daneben:
///
/// | Feld | Bereich in `parse_rules` | hier |
/// | --- | --- | --- |
/// | Host, Name | `idna::domain_to_ascii_strict`; `*` und `:` kommen nicht durch | [`HostPattern::Exact`], immer lesbar |
/// | Host, Adresse | `ip:` plus `IpAddr`, auch `ip:::1` | [`HostPattern::Ip`] |
/// | Methode | nur `is_known_method` | kein Vorschlag, wenn unbekannt |
/// | Schema | genau `http`, `https`, `ws`, `wss` | [`Scheme`] hat genau diese vier |
/// | Port | `1..=65535`; `0` lehnt die Datei ab | kein Vorschlag bei `0` |
/// | Pfadpräfix | [`path_prefix_is_valid`]; sonst lehnt die Datei ab | weggelassen, wenn untauglich oder wenn es die Anfrage nicht trifft |
/// | Upgrade | nur `websocket` | [`Upgrade`](humanitl_core::Upgrade) hat genau diese Variante |
///
/// Wer hier ein Feld hinzufügt, trägt seinen Wertebereich in diese Tabelle ein
/// und in die Tabelle feindlicher Anfragen in
/// `daemon/crates/proxy/tests/private_address.rs`.
///
/// # Zuschnitt
///
/// - Der Host als [`HostPattern::Exact`], bei einem IP-Literal als
///   [`HostPattern::Ip`]. Ein Glob trifft eine Adresse nie (ADR-007), und ein
///   Vorschlag, der nichts trifft, wäre kein Vorschlag. Bei einem Namen steht in
///   der Regel nur der Name: Die Adresse gehört nicht in eine Datei, die der
///   Agent über den Meta-Endpunkt lesen kann (HUM-073).
/// - Schema, Port und die Methode der gescheiterten Anfrage.
/// - Der Protokollwechsel der Anfrage, falls sie einen verlangt. Ohne ihn täte
///   die Regel etwas anderes als das Gewünschte: Die Auswertung prüft die
///   Upgrade-Dimension beidseitig, eine Regel ohne `upgrade` trifft nie ein
///   Upgrade (`humanitl_rules::eval`). Ein Mensch, der einen gescheiterten
///   WebSocket freigibt, bekäme sonst eine Regel, die gewöhnliches HTTP öffnet
///   und genau diesen WebSocket weiterhin nicht.
/// - Der Pfad als Präfix, immer ohne Query: Ein Token aus der Abfragezeichenkette
///   hätte in `rules.yaml` nichts zu suchen. Gesetzt wird es nur, wenn es zwei
///   Prüfungen besteht: [`path_prefix_is_valid`], sonst lehnt `parse_rules` die
///   Datei ab, und [`prefix_matches`] gegen die gescheiterte Anfrage selbst,
///   sonst stünde in der Regel eine Bedingung, die genau diese Anfrage nicht
///   erfüllt. Die zweite Prüfung ist die schärfere: Ein Pfad mit einem
///   `..`-Segment trifft nie ein Präfix, auch verschleiert nicht (`%2e`, `%5c`,
///   `\`), weil erst der Server dahinter auflöst. Ein Vorschlag, der aus dem
///   Pfad ein Präfix zöge, das ihn nicht trifft, wäre wirkungslos — dieselbe
///   Falle wie eine Regel mit `action: ask` vor HUM-102.
///
///   Bleibt das Feld weg, gilt die Regel für jeden Pfad dieses Hosts
///   (`CompiledPrefixes::Any`), und [`private_address_refused`] schreibt das in
///   den `why`, statt es dem Klick zu überlassen.
///
/// Die Notiz nennt die Adresse nicht. Sie landet in `rules.yaml` und damit in
/// der Regelübersicht, die der Meta-Endpunkt dem Agenten zeigt; die Adresse
/// bleibt im Befund, der nur an die Oberfläche geht.
///
/// # Wo die Regel stehen muss, und was daran offen ist
///
/// `RuleSet::evaluate` liefert den **ersten** Treffer eines Ranges. Trifft
/// schon eine ältere Regel des Nutzers denselben Host, ohne private Ziele zu
/// erlauben, entscheidet weiterhin sie, und der Vorschlag bleibt wirkungslos —
/// dieselbe Falle, wegen der dieses Issue umgeschrieben wurde, nur eine Ebene
/// höher. Die Regel muss deshalb **vor** der stehen, die gerade entschieden
/// hat.
///
/// Diese Stelle kann das nicht erzwingen. Ein [`FixAction::AddRule`] trägt
/// keine Position; die entsteht erst in `humanitl_ipc::convert::rule_to_proto`,
/// und die sendet heute `position: 0`, was `position_of` als „ans Ende" liest
/// und `RulesStore::add` als Anhängen ausführt. Das gilt für **jedes**
/// `AddRule` im Produkt und nicht nur für dieses, und es zu ändern heißt, die
/// Wire-Form anzufassen. Es steht als eigenes Issue aus.
///
/// Bis dahin sagt der Befund es aus: Sein `why` und die Notiz dieser Regel
/// nennen die Bedingung, damit der Mensch die Regel an den richtigen Platz
/// zieht, statt vor einem Knopf zu stehen, der stumm nichts tut. Gegen eine
/// **Sitzungsregel** hilft auch das Verschieben nicht: Rang `Session` liegt vor
/// Rang `User` (`backlog/CONVENTIONS.md` 4.5), eine dauerhafte Regel überholt
/// sie nie. Dann ist die Sitzungsregel selbst zu ändern, und der `why` sagt
/// auch das.
///
/// `request` ist immer `flow.request`, also die Anfrage des Agenten, und nicht
/// die von einem Menschen bearbeitete Fassung einer `AllowEdited`: Über den
/// Regelsatz läuft die ursprüngliche Anfrage
/// ([`RulesPipeline::decide`](crate::pipeline::RulesPipeline)), und eine Regel,
/// die stattdessen die Bearbeitung beschriebe, träfe beim nächsten Mal nichts.
///
/// # Kein Vorschlag ist besser als ein untauglicher
///
/// Zwei Anfragen bekommen gar keine Regel, und beide Gründe sind derselbe
/// Gedanke: Ein Vorschlag, den ein Mensch anklickt, muss danach wirken.
///
/// - **Port `0`.** `parse_rules` lehnt ihn ab, und ein Fehler dort verwirft die
///   ganze Datei — ein Klick, und der Nutzer verlöre alle seine Regeln. Den Port
///   wegzulassen wäre kein Ausweg, denn das öffnete jeden Port desselben Hosts,
///   und ein Port `0` bezeichnet ohnehin keinen Dienst.
/// - **Eine Methode außerhalb von [`is_known_method`].** Sie ohne Methode
///   vorzuschlagen ginge durch den Parser, brächte aber nichts: `RuleSet::evaluate`
///   bricht bei einer unbekannten Methode ab, **bevor** es überhaupt eine Regel
///   ansieht, und gibt `Verdict::Default` zurück. Die Regel träfe nie, der
///   Mensch klickte und bekäme beim nächsten Versuch dieselbe Ablehnung ohne
///   neue Erklärung. Das ist die Falle, wegen der dieses Issue umgedreht wurde,
///   nur an einer anderen Stelle.
///
/// In beiden Fällen trägt der Befund kein `fix`, und sein `why` sagt, was im
/// Weg steht.
/// # Errors
///
/// [`NoRule`], wenn sich zu dieser Anfrage keine Regel bauen lässt, die
/// `parse_rules` annimmt und die die Anfrage danach auch trifft.
pub fn private_address_rule(request: &HttpRequest) -> Result<Rule, NoRule> {
    // Reihenfolge nach Schwere: Der Port zerrisse die Datei, die Methode
    // erzeugte nur eine wirkungslose Regel.
    if request.authority.port == 0 {
        return Err(NoRule::PortZero);
    }
    if !is_known_method(&request.method) {
        return Err(NoRule::UnknownMethod);
    }
    let host = match &request.authority.host {
        HostName::Ip(ip) => HostPattern::Ip(*ip),
        host @ HostName::Dns(_) => HostPattern::Exact(host.clone()),
    };
    let mut matcher = Matcher::host(host)
        .with_scheme(request.scheme)
        .with_port(request.authority.port);
    matcher = matcher.with_methods(vec![request.method.clone()]);
    if let Some(upgrade) = connect::requested_upgrade(&request.headers) {
        matcher = matcher.with_upgrade(upgrade);
    }
    let prefix = strip_query(&request.path_and_query);
    let prefixes = vec![prefix.to_owned()];
    if path_prefix_is_valid(prefix) && prefix_matches(&prefixes, &request.path_and_query) {
        matcher = matcher.with_path_prefixes(prefixes);
    }
    Ok(Rule::new(RuleId::new(), Action::Ask, matcher)
        .with_allow_private(true)
        .with_note(format!(
            "suggested after Humanitl refused a private target address for {host}; \
             the request is still held for a decision, and this rule has to stand in front of \
             any other rule for the same host to take effect",
            host = request.authority.host.display(),
        )))
}

/// Baut die HTTP-Antwort aus einer [`BlockResponse`](humanitl_core::BlockResponse):
/// Status, `text/plain`, `X-Humanitl-Flow`, optional die Notiz, und
/// `Connection: close`, damit ein noch nicht gelesener Request-Body die
/// Keep-Alive-Verbindung nicht blockiert (Fallstrick HUM-015).
fn block_to_response(block: &humanitl_core::BlockResponse, flow: FlowId) -> Response<ResponseBody> {
    let mut response = Response::new(body::full(Bytes::from(block.body.clone())));
    *response.status_mut() =
        StatusCode::from_u16(block.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    *response.version_mut() = Version::HTTP_11;
    let headers = response.headers_mut();
    headers.insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(hyper::header::CONNECTION, HeaderValue::from_static("close"));
    if let Ok(value) = HeaderValue::from_str(&flow.to_string()) {
        headers.insert(HeaderName::from_static("x-humanitl-flow"), value);
    }
    if let Some(note) = block.header_note()
        && let Ok(value) = HeaderValue::from_str(&note)
    {
        headers.insert(
            HeaderName::from_static(humanitl_core::block::NOTE_HEADER),
            value,
        );
    }
    response
}

/// Baut die HTTP-Antwort des Meta-Endpunkts (HUM-073).
///
/// `Connection: close` wie bei der Block-Antwort: Eine abgelehnte Methode und
/// ein Body über dem Cap lassen ungelesene Bytes auf der Leitung stehen, und
/// die blockierten sonst die Keep-Alive-Verbindung (Fallstrick HUM-015). Der
/// Unterschied zwischen den beiden Fällen wäre eine Fallunterscheidung, die
/// nichts einbringt: Der Agent stellt hier einzelne Fragen, keine Serien.
fn meta_to_response(reply: &MetaReply) -> Response<ResponseBody> {
    let mut response = Response::new(body::full(Bytes::from(reply.body.clone())));
    *response.status_mut() =
        StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    *response.version_mut() = Version::HTTP_11;
    let headers = response.headers_mut();
    headers.insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(hyper::header::CONNECTION, HeaderValue::from_static("close"));
    for (name, value) in &reply.headers {
        // Beides steht als Konstante im Modul `meta`; eine Kopfzeile, die
        // hier trotzdem nicht durchgeht, fällt lieber weg, als dass der
        // Handler in Panik gerät.
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }
    response
}

/// Eine kurze `text/plain`-Antwort für Fälle vor jedem Flow (fehlender Host,
/// kaputter Body).
fn text_response(status: StatusCode, message: &str) -> Response<ResponseBody> {
    let mut response = Response::new(body::full(Bytes::from(format!("{message}\n"))));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(hyper::header::CONNECTION, HeaderValue::from_static("close"));
    response
}

/// Schreibt in den Trace, was gefunden wurde — ohne den Wert.
///
/// Ein Fund trägt Art, Ort, Bereich und den SHA-256 des Werts. Der Wert selbst
/// steht in keiner Zeile: Ein Protokoll, das Geheimnisse mitschreibt, ist
/// selbst das Leck, das diese Suche verhindern soll.
fn log_findings(flow: &Flow, findings: &[Finding], truncated: bool) {
    if findings.is_empty() && !truncated {
        return;
    }
    for finding in findings {
        tracing::debug!(
            flow = %flow.id,
            kind = %finding.kind,
            location = %finding.location,
            tier = finding.tier.as_str(),
            span_start = finding.span.start,
            span_end = finding.span.end,
            value_hash = %finding.value_hash_hex(),
            "finding"
        );
    }
    if truncated {
        tracing::debug!(
            flow = %flow.id,
            findings = findings.len(),
            "the request was only searched in part; the result is not an all-clear"
        );
    }
}
