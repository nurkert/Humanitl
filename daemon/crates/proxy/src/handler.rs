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
use humanitl_config::{Limits, RecorderConfig};
use humanitl_core::diagnostics::codes::PROXY_005;
use humanitl_core::{
    Authority, BlockReason, BodyRef, Decision, DecisionSource, Diagnostic, Flow, FlowEvent, FlowId,
    FlowState, HostName, HttpRequest, InvalidTransition, Method, Scheme, Severity, TransitionInput,
    UpstreamError, block_response, failed_response,
};
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode, Version};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio_rustls::TlsAcceptor;

use crate::body::{self, BufferError, ResponseBody};
use crate::ca::LeafCache;
use crate::hold::HoldQueue;
use crate::pipeline::{ConnMeta, FlowPipeline};
use crate::registry::FlowRecord;
use crate::upstream::{self, Upstream};

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
}

impl ProxyLimits {
    /// Die Grenzen aus der Konfiguration.
    #[must_use]
    pub const fn from_config(limits: &Limits, recorder: &RecorderConfig) -> Self {
        Self {
            body_cap_bytes: limits.hold_body_cap_bytes,
            inline_max_bytes: recorder.inline_max_bytes,
            header_timeout: Duration::from_secs(limits.header_timeout_secs),
        }
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
            }),
        }
    }

    /// Nimmt eine einzelne Anfrage entgegen und liefert die Antwort.
    async fn handle(&self, req: Request<Incoming>, meta: ConnMeta) -> Response<ResponseBody> {
        if req.method() == Method::CONNECT {
            return self.handle_connect(req, &meta);
        }
        self.handle_request(req, meta).await
    }

    /// `CONNECT`: mit `200` bestätigen, dann die Verbindung übernehmen, TLS mit
    /// dem Leaf des Ziels terminieren und die entschlüsselte Verbindung erneut
    /// bedienen.
    fn handle_connect(
        &self,
        mut req: Request<Incoming>,
        meta: &ConnMeta,
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
        let inner_meta = ConnMeta {
            connect_authority: Some(authority),
            tls: true,
            ..meta.clone()
        };
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let acceptor = TlsAcceptor::from(server_config);
                    match acceptor.accept(TokioIo::new(upgraded)).await {
                        Ok(tls) => serve_connection(handler, tls, inner_meta).await,
                        // Der Client hat unsere CA nicht akzeptiert oder ein
                        // anderes SNI gesendet als das CONNECT-Ziel — beides
                        // ist gewollt fail-closed (kein Fronting), sichtbar
                        // wird es als TLS_003 in HUM-045.
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

    /// Eine gewöhnliche Anfrage (Origin- oder Absolut-Form): Ziel bestimmen,
    /// Body puffern, entscheiden, weiterleiten oder blocken.
    async fn handle_request(
        &self,
        req: Request<Incoming>,
        meta: ConnMeta,
    ) -> Response<ResponseBody> {
        let (scheme, authority) = match request_target(&req, &meta) {
            Ok(target) => target,
            Err(reason) => return text_response(StatusCode::BAD_REQUEST, reason),
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
            return self.refuse_before_pipeline(&meta, request, BlockReason::BodyCap);
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
                return self.refuse_before_pipeline(&meta, request, BlockReason::BodyCap);
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
        if self
            .apply(&mut flow, TransitionInput::Analyze { findings: vec![] })
            .is_err()
        {
            return self.fail_closed(&mut flow);
        }

        let decision = self.inner.pipeline.decide(&mut flow, &meta).await;
        match decision {
            Decision::Allow => self.forward(flow, request, body_bytes, &meta).await,
            Decision::AllowEdited { request: edited } => {
                // Die Konsistenz von Authority und SNI prüft HUM-023; hier wird
                // die entschiedene Anfrage getreu weitergeleitet, mit ihrem
                // eigenen (bearbeiteten) Body.
                let edited = *edited;
                let edited_body = edited
                    .body
                    .inline
                    .clone()
                    .unwrap_or_else(|| body_bytes.clone());
                self.forward(flow, edited, edited_body, &meta).await
            }
            Decision::Block { reason, note } => {
                self.record_block(&mut flow, reason, note.as_deref())
            }
            Decision::TimedOut => self.record_block(&mut flow, BlockReason::Timeout, None),
        }
    }

    /// Leitet eine erlaubte Anfrage weiter und streamt die Antwort zurück.
    async fn forward(
        &self,
        mut flow: Flow,
        request: HttpRequest,
        body: Bytes,
        meta: &ConnMeta,
    ) -> Response<ResponseBody> {
        // Fail-closed vor dem Egress: kann der Flow nicht nach `Forwarded`, geht
        // die Anfrage nicht hinaus.
        if self.apply(&mut flow, TransitionInput::Forward).is_err() {
            return self.fail_closed(&mut flow);
        }
        match self
            .inner
            .upstream
            .forward(&request, body, meta.allow_private)
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
                // Der Tee reicht die Antwort ungepuffert durch und schließt den
                // Flow beim letzten Frame mit `Record` ab.
                let body = body::tee(incoming, flow, Arc::clone(&self.inner.queue));
                Response::from_parts(parts, body)
            }
            Err(error) => self.record_failure(&mut flow, error),
        }
    }

    /// Baut einen [`Flow`] für eine schon vor der Pipeline abgelehnte Anfrage
    /// (Body über Cap): Received, Analyzed, Decided(Block) durch das System,
    /// dann die `413`-Antwort samt `Recorded`.
    ///
    /// Dieser Flow erreicht keine Pipeline und trägt sich deshalb selbst in die
    /// [`FlowRegistry`](crate::registry::FlowRegistry) ein; sonst fehlte er in
    /// `ListFlows`.
    fn refuse_before_pipeline(
        &self,
        meta: &ConnMeta,
        request: HttpRequest,
        reason: BlockReason,
    ) -> Response<ResponseBody> {
        let mut flow = Flow::new(FlowId::new(), meta.session, SystemTime::now(), request);
        self.inner
            .queue
            .registry()
            .insert(FlowRecord::new(&flow, meta));
        self.inner.queue.publish(flow.received_event());
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
        if matches!(
            flow.state,
            FlowState::Analyzed { .. } | FlowState::Held { .. }
        ) && let Ok(event) = flow.apply(
            TransitionInput::Decide {
                decision: Decision::Block { reason, note: None },
                source: DecisionSource::System,
            },
            SystemTime::now(),
        ) {
            self.inner.queue.publish(event);
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
/// sich selbst rekursiv mit `meta.tls = true` auf.
pub async fn serve_connection<I>(handler: FlowHandler, io: I, meta: ConnMeta)
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

/// Bestimmt Schema und Ziel einer gewöhnlichen Anfrage.
///
/// Absolut-Form (Klartext-Proxy) gewinnt mit ihrer URI; sonst kommt der Host
/// aus dem `Host`-Kopf oder, im Tunnel, aus dem CONNECT-Ziel. Das Schema folgt
/// der URI, sonst dem TLS-Zustand der Verbindung.
fn request_target(
    req: &Request<Incoming>,
    meta: &ConnMeta,
) -> Result<(Scheme, Authority), &'static str> {
    let uri = req.uri();
    let scheme = match uri.scheme_str() {
        Some(text) => Scheme::parse(text).ok_or("unsupported scheme")?,
        None if meta.tls => Scheme::Https,
        None => Scheme::Http,
    };

    if let Some(authority) = uri.authority() {
        let host = HostName::parse(authority.host()).map_err(|_err| "invalid host")?;
        let port = authority
            .port_u16()
            .unwrap_or_else(|| scheme.default_port());
        return Ok((scheme, Authority::new(host, port)));
    }

    if let Some(host_header) = header_string(req.headers(), hyper::header::HOST) {
        let (host_text, port) = split_host_port(host_header);
        let host = HostName::parse(host_text).map_err(|_err| "invalid host")?;
        let port = port.unwrap_or_else(|| scheme.default_port());
        return Ok((scheme, Authority::new(host, port)));
    }

    if let Some(tunnel) = &meta.connect_authority {
        return Ok((scheme, tunnel.clone()));
    }

    Err("missing host")
}

/// Zerlegt einen `Host`-Kopf in Host und optionalen Port; `IPv6` in eckigen
/// Klammern wird korrekt behandelt.
fn split_host_port(value: &str) -> (&str, Option<u16>) {
    if let Some(rest) = value.strip_prefix('[') {
        if let Some((inner, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').and_then(|p| p.parse().ok());
            // Host mit Klammern zurückgeben, damit `HostName::parse` das
            // IPv6-Literal erkennt.
            let end = 1 + inner.len() + 1;
            return (&value[..end], port);
        }
        return (value, None);
    }
    match value.rsplit_once(':') {
        // Nur trennen, wenn der Rest kein weiteres `:` trägt (also keine
        // bracketlose IPv6-Adresse ist) und der Port eine Zahl ist.
        Some((host, port)) if !host.contains(':') && !port.is_empty() => {
            match port.parse::<u16>() {
                Ok(port) => (host, Some(port)),
                Err(_) => (value, None),
            }
        }
        _ => (value, None),
    }
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
