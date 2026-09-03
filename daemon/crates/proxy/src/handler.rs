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
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use humanitl_config::{HoldConfig, Limits, RecorderConfig};
use humanitl_core::diagnostics::codes::PROXY_005;
use humanitl_core::{
    Authority, BlockReason, BodyRef, Decision, DecisionSource, Diagnostic, Finding, Flow,
    FlowEvent, FlowId, FlowState, HeaderMap, HostName, HttpRequest, InvalidTransition, Method,
    Scheme, Severity, Tier, TransitionInput, UpstreamError, block_response, failed_response,
};
use humanitl_recorder::{Dir, Recorder};
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
use crate::pipeline::FlowPipeline;
use crate::registry::FlowRecord;
use crate::upstream::{self, Upstream};
use crate::{connect, tls};

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
        Self {
            inner: Arc::new(Inner {
                queue,
                pipeline,
                upstream,
                leaves,
                limits,
                scanner: Arc::new(NoScan),
                recorder: None,
            }),
        }
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
        Self {
            inner: Arc::new(Inner {
                queue,
                pipeline,
                upstream,
                leaves,
                limits,
                scanner,
                recorder,
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

        let handler = self.clone();
        let meta = meta.clone();
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    match tls::accept(server_config, TokioIo::new(upgraded)).await {
                        Ok((stream, sni)) => {
                            let inner_meta = meta.tunnel(authority, sni);
                            serve_connection(handler, stream, inner_meta).await;
                        }
                        // Der Client hat unsere CA nicht akzeptiert oder eine
                        // unbrauchbare SNI geschickt — beides ist gewollt
                        // fail-closed, sichtbar wird es als TLS_003 in
                        // HUM-045.
                        Err(err) => tracing::debug!(%err, "tls handshake with the client failed"),
                    }
                }
                Err(err) => tracing::debug!(%err, "connect upgrade failed"),
            }
        });

        let mut response = Response::new(body::empty());
        *response.status_mut() = StatusCode::OK;
        response
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
        match decision {
            Decision::Allow => self.forward(flow, request, body_bytes, &meta).await,
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
                self.forward(flow, edited, edited_body, &meta).await
            }
            Decision::Block { reason, note } => {
                self.record_block(&mut flow, reason, note.as_deref())
            }
            Decision::TimedOut => self.record_block(&mut flow, BlockReason::Timeout, None),
        }
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
    fn record_failure(&self, flow: &mut Flow, error: UpstreamError) -> Response<ResponseBody> {
        let _ = self.apply(flow, TransitionInput::Fail { error });
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
    /// Block-Antwort, und die Anfrage erreicht das Ziel nicht. Der Befund
    /// `PROXY_005` liegt zu diesem Zeitpunkt schon im Strom; die Übergänge hier
    /// gehen deshalb an [`FlowHandler::apply`] vorbei, damit derselbe Fehler
    /// nicht ein zweites Mal gemeldet wird.
    fn fail_closed(&self, flow: &mut Flow) -> Response<ResponseBody> {
        let reason = BlockReason::NoRoute;
        // `Record` ist nur aus `Decided(Block | TimedOut)`, `Responded` und
        // `Failed` erlaubt. Der Flow wird deshalb erst auf dem legalen Weg in
        // einen dieser Zustaende gebracht, je nachdem, wo er gerade steht;
        // sonst bliebe er in der Registry fuer immer in `Received`,
        // `Decided(Allow)` oder `Forwarded` haengen und `Recorded` kaeme nie.
        let block = TransitionInput::Decide {
            decision: Decision::Block { reason, note: None },
            source: DecisionSource::System,
        };
        let steps: Vec<TransitionInput> = match &flow.state {
            FlowState::Received => vec![
                TransitionInput::Analyze {
                    findings: Vec::new(),
                },
                block,
            ],
            FlowState::Analyzed { .. } | FlowState::Held { .. } => vec![block],
            FlowState::Decided(Decision::Allow | Decision::AllowEdited { .. })
            | FlowState::Forwarded => vec![TransitionInput::Fail {
                error: UpstreamError::Connect,
            }],
            _ => Vec::new(),
        };
        for step in steps {
            if let Ok(event) = flow.apply(step, SystemTime::now()) {
                self.inner.queue.publish(event);
            }
        }
        if let Ok(event) = flow.apply(TransitionInput::Record, SystemTime::now()) {
            self.inner.queue.publish(event);
        }
        let block = block_response(reason, flow.id, &flow.request.authority.host, None);
        block_to_response(&block, flow.id)
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
