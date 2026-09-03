//! Der Zustand des Fake-Daemons und seine Wire-Form.
//!
//! Der Fake hält Flows, Regeln, Bodies und die Sitzung im Speicher. Jede
//! Zustandsänderung eines Flows geht durch [`Flow::apply`] und damit durch den
//! Automaten aus `humanitl-core`; es gibt in dieser Datei keine Zuweisung an
//! `flow.state`. Was der Automat als Ereignis zurückgibt, wird hier in die
//! Wire-Form übersetzt und in den Rundfunk-Kanal gelegt.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use humanitl_core::http::{BodyRef, HeaderMap, HttpRequest};
use humanitl_core::{
    Decision, DecisionSource, Diagnostic, Finding, Flow, FlowEvent, FlowId, FlowState,
    InvalidTransition, SandboxId, SessionId, TransitionInput, UpstreamError,
};
use tokio::sync::broadcast;

use crate::convert::{
    apex_of, authority_to_proto, block_note, body_preview, body_to_proto, decision_fields,
    diagnostic_to_proto, duration_between, finding_to_proto, flow_state_to_proto, headers_to_proto,
    method_raw, method_to_proto, request_to_proto, scheme_to_proto, source_to_proto, timestamp,
    upstream_error_to_proto,
};
use crate::v1;

/// So viele Bytes trägt ein Stück aus `GetBody`.
pub const BODY_CHUNK_BYTES: usize = 64 * 1024;

/// So viele Flows behält der Fake höchstens.
///
/// `--loop` läuft stundenlang und legt bei jedem Durchlauf neue Flows an.
/// Ohne Grenze wüchse die Karte samt der Bodies darin, bis der Speicher voll
/// ist. Entfernt werden nur abgeschlossene Flows und die ältesten zuerst; was
/// noch auf eine Entscheidung wartet, bleibt in jedem Fall.
pub const MAX_FLOWS: usize = 2_000;

/// Was der Fake über die laufende Sitzung weiß.
#[derive(Debug, Clone)]
pub struct SessionMeta {
    /// Id der Sitzung.
    pub id: SessionId,
    /// Der LLM-Endpunkt, den die Sandbox benutzen würde.
    pub llm_endpoint: String,
    /// Das Projektverzeichnis der Sitzung.
    pub work_dir: String,
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self {
            id: SessionId::new(),
            llm_endpoint: String::new(),
            work_dir: String::new(),
        }
    }
}

/// Eine Antwort, wie sie in der Sitzungsdatei steht.
#[derive(Debug, Clone)]
pub struct StoredResponse {
    /// Der Status der Antwort.
    pub status: u16,
    /// Die Kopfzeilen der Antwort.
    pub headers: HeaderMap,
    /// Verweis auf den Antwort-Body.
    pub body: BodyRef,
    /// Ob die Antwort gestreamt wird.
    pub streaming: bool,
}

/// Ein Flow im Fake: der Automat plus alles, was die Wire-Form zeigt.
#[derive(Debug, Clone)]
pub struct FakeFlow {
    /// Der Flow mit seinem Zustand.
    pub flow: Flow,
    /// Was die Detektoren gefunden haben.
    pub findings: Vec<Finding>,
    /// Wie entschieden wurde, sobald entschieden ist.
    pub decision: Option<Decision>,
    /// Wer entschieden hat.
    pub source: Option<DecisionSource>,
    /// Die bearbeitete Anfrage bei `AllowEdited`.
    pub edited: Option<HttpRequest>,
    /// Die Antwort, sobald sie da ist.
    pub response: Option<StoredResponse>,
    /// Ob der Flow der LLM-Durchreiche gehört.
    pub passthrough: bool,
    /// Welches Werkzeug die Anfrage gestellt hat, falls bekannt.
    pub origin_tool: Option<String>,
    /// Bis wann gewartet wird, als Wanduhrzeit für die Anzeige.
    pub deadline_at: Option<SystemTime>,
    /// Warum die Verbindung gescheitert ist.
    pub upstream_error: Option<UpstreamError>,
    /// Wann zuletzt etwas geschah; speist die Dauer in der Liste.
    pub last_at: SystemTime,
}

impl FakeFlow {
    /// Ein neuer Flow im Zustand `Received`.
    #[must_use]
    pub fn new(id: FlowId, session: SessionId, at: SystemTime, request: HttpRequest) -> Self {
        Self {
            flow: Flow::new(id, session, at, request),
            findings: Vec::new(),
            decision: None,
            source: None,
            edited: None,
            response: None,
            passthrough: false,
            origin_tool: None,
            deadline_at: None,
            upstream_error: None,
            last_at: at,
        }
    }

    /// Die Zeilendarstellung für Liste und Warteschlange.
    #[must_use]
    pub fn summary(&self) -> v1::FlowSummary {
        let request = &self.flow.request;
        let (kind, block_reason, rule_id) = decision_fields(self.decision.as_ref(), self.source);
        v1::FlowSummary {
            flow_id: self.flow.id.to_string(),
            session_id: self.flow.session.to_string(),
            received_at: Some(timestamp(self.flow.received_at)),
            method: method_to_proto(&request.method) as i32,
            method_raw: method_raw(&request.method),
            scheme: scheme_to_proto(request.scheme) as i32,
            authority: Some(authority_to_proto(&request.authority)),
            path: request.path_and_query.clone(),
            state: flow_state_to_proto(&self.flow.state) as i32,
            decision: kind as i32,
            decision_source: self
                .source
                .map_or(0, |source| source_to_proto(source) as i32),
            block_reason: block_reason as i32,
            rule_id,
            status: self.response.as_ref().map_or(0, |r| u32::from(r.status)),
            request_size: request.body.size,
            response_size: self.response.as_ref().map_or(0, |r| r.body.size),
            duration: Some(duration_between(self.flow.received_at, self.last_at)),
            finding_count: u32::try_from(self.findings.len()).unwrap_or(u32::MAX),
            edited: self.edited.is_some(),
            passthrough: self.passthrough,
            deadline: self.deadline_at.map(timestamp),
            origin_tool: self.origin_tool.clone().unwrap_or_default(),
            upstream_error: self
                .upstream_error
                .map_or(0, |error| upstream_error_to_proto(error) as i32),
        }
    }

    /// Alles, was der Detail-Bereich zeigt.
    #[must_use]
    pub fn detail(&self) -> v1::FlowDetail {
        v1::FlowDetail {
            summary: Some(self.summary()),
            request: Some(request_to_proto(&self.flow.request)),
            edited_request: self.edited.as_ref().map(request_to_proto),
            response: self.response.as_ref().map(|response| v1::HttpResponseHead {
                status: u32::from(response.status),
                headers: headers_to_proto(&response.headers),
                version: "HTTP/1.1".to_owned(),
            }),
            response_body: self.response.as_ref().map(|r| body_to_proto(&r.body)),
            findings: self.findings.iter().map(finding_to_proto).collect(),
            diagnostics: Vec::new(),
            domain: Some(self.domain()),
            body_preview: body_preview(
                self.flow.request.body.inline.as_deref().unwrap_or_default(),
            ),
            // Eine aufgezeichnete Sitzung kennt keinen Scan, der abgebrochen
            // wäre: Die Funde stehen in der Datei, wie sie dort stehen.
            findings_truncated: false,
        }
    }

    /// Was der Katalog über die Ziel-Domain wüsste.
    ///
    /// Der Fake kennt keinen Katalog. Der Apex ist die einfache Ableitung aus
    /// den letzten beiden Labels des Hosts aus der Sitzungsdatei; Rang und
    /// Katalog-Eintrag bleiben „unbekannt" (0 und leer), weil ein erfundener
    /// Rang in einem Screenshot wie ein gemessener aussähe. Eine Entscheidung
    /// trifft darauf im echten Daemon `humanitl-catalog`.
    #[must_use]
    pub fn domain(&self) -> v1::DomainInfo {
        v1::DomainInfo {
            apex: apex_of(&self.flow.request.authority.host),
            catalog_id: String::new(),
            tranco_rank: 0,
            first_seen: Some(timestamp(self.flow.received_at)),
            seen_count: 1,
        }
    }

    /// Wahr, solange der Flow auf eine Entscheidung wartet.
    #[must_use]
    pub const fn is_held(&self) -> bool {
        matches!(self.flow.state, FlowState::Held { .. })
    }
}

/// Der gesamte Zustand des Fake-Daemons.
#[derive(Debug)]
pub struct FakeState {
    inner: Mutex<Inner>,
    events: broadcast::Sender<v1::FlowEvent>,
}

/// Der Teil des Zustands, der unter dem Schloss liegt.
#[derive(Debug)]
struct Inner {
    session: SessionMeta,
    flows: BTreeMap<FlowId, FakeFlow>,
    rules: Vec<v1::Rule>,
    rules_revision: u64,
    blobs: BTreeMap<[u8; 32], Bytes>,
    scripted: BTreeMap<FlowId, StoredResponse>,
    config: BTreeMap<String, String>,
    sandbox_id: SandboxId,
    sandbox_state: v1::SandboxState,
    hold_bytes: u64,
    hold_count: u32,
}

impl FakeState {
    /// Baut den Zustand mit der Kapazität des Rundfunk-Kanals.
    ///
    /// Die Kapazität ist `limits.event_buffer`; ist sie erschöpft, bekommt ein
    /// langsamer Zuhörer [`v1::flow_event::Event::Lagged`].
    #[must_use]
    pub fn new(session: SessionMeta, event_buffer: usize) -> Self {
        let (events, _) = broadcast::channel(event_buffer.max(1));
        Self {
            inner: Mutex::new(Inner {
                session,
                flows: BTreeMap::new(),
                rules: Vec::new(),
                rules_revision: 0,
                blobs: BTreeMap::new(),
                scripted: BTreeMap::new(),
                config: BTreeMap::new(),
                sandbox_id: SandboxId::new(),
                sandbox_state: v1::SandboxState::Running,
                hold_bytes: 0,
                hold_count: 0,
            }),
            events,
        }
    }

    /// Nimmt am Ereignisstrom teil.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<v1::FlowEvent> {
        self.events.subscribe()
    }

    /// Das Schloss, vergiftungsfest: ein Panic in einem Test soll den Fake
    /// nicht für alle folgenden Aufrufe unbrauchbar machen.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Die Sitzung, zu der die Flows gehören.
    #[must_use]
    pub fn session(&self) -> SessionMeta {
        self.lock().session.clone()
    }

    /// Setzt die Sitzung, wenn die Datei eine `session`-Zeile hatte.
    pub fn set_session(&self, session: SessionMeta) {
        self.lock().session = session;
    }

    /// Legt einen Body ab, damit `GetBody` ihn wiederfindet.
    pub fn store_blob(&self, body: &BodyRef) {
        if let Some(bytes) = body.inline.clone() {
            self.lock().blobs.insert(body.sha256, bytes);
        }
    }

    /// Holt einen abgelegten Body.
    #[must_use]
    pub fn blob(&self, sha256: &[u8; 32]) -> Option<Bytes> {
        self.lock().blobs.get(sha256).cloned()
    }

    /// Nimmt einen neuen Flow auf und meldet ihn.
    pub fn receive(&self, flow: FakeFlow) {
        self.store_blob(&flow.flow.request.body);
        let mut inner = self.lock();
        let event = flow.flow.received_event();
        let wire = event_to_proto(&event, &flow);
        inner.flows.insert(flow.flow.id, flow);
        prune(&mut inner.flows, MAX_FLOWS);
        drop(inner);
        self.emit(wire);
    }

    /// Führt einen Übergang aus und meldet das Ereignis.
    ///
    /// # Errors
    ///
    /// [`InvalidTransition`], wenn der Übergang im aktuellen Zustand nicht
    /// erlaubt ist. Der Aufrufer entscheidet, ob das ein Fehler ist (eine
    /// Entscheidung für einen Flow, der nicht mehr wartet) oder eine
    /// harmlose Überholung (ein Timeout nach der Entscheidung).
    pub fn advance(
        &self,
        id: FlowId,
        input: TransitionInput,
        at: SystemTime,
    ) -> Result<(), InvalidTransition> {
        let mut inner = self.lock();
        let Some(flow) = inner.flows.get_mut(&id) else {
            return Err(InvalidTransition {
                from: "unknown",
                input: input.name(),
            });
        };
        // Vor dem Übergang festhalten, ob der Flow in der Warteschlange stand:
        // nur dann darf eine Entscheidung ihn wieder herausrechnen.
        let was_held = flow.is_held();
        let event = flow.flow.apply(input, at)?;
        flow.last_at = at;
        apply_side_effects(flow, &event);
        let wire = event_to_proto(&event, flow);
        let size = flow.flow.request.body.size;
        update_hold_counters(&mut inner, was_held, size, &event);
        let wire = with_hold_counters(wire, inner.hold_bytes, inner.hold_count);
        drop(inner);
        self.emit(wire);
        Ok(())
    }

    /// Setzt die Frist eines Flows, bevor er gehalten wird.
    pub fn set_deadline(&self, id: FlowId, deadline_at: SystemTime) {
        if let Some(flow) = self.lock().flows.get_mut(&id) {
            flow.deadline_at = Some(deadline_at);
        }
    }

    /// Hinterlegt die Antwort, die beim nächsten `Respond` gemeldet wird.
    pub fn set_response(&self, id: FlowId, response: StoredResponse) {
        self.store_blob(&response.body);
        if let Some(flow) = self.lock().flows.get_mut(&id) {
            flow.response = Some(response);
        }
    }

    /// Hinterlegt die bearbeitete Anfrage einer `AllowEdited`-Entscheidung.
    ///
    /// Ihr Body kommt inline aus der `EditedRequest` und wird abgelegt, damit
    /// `GetBody(FlowDetail.edited_request.body)` ihn liefert.
    pub fn set_edited(&self, id: FlowId, request: HttpRequest) {
        self.store_blob(&request.body);
        if let Some(flow) = self.lock().flows.get_mut(&id) {
            flow.edited = Some(request);
        }
    }

    /// Markiert einen Flow als Durchreiche zum LLM.
    pub fn set_passthrough(&self, id: FlowId, passthrough: bool) {
        if let Some(flow) = self.lock().flows.get_mut(&id) {
            flow.passthrough = passthrough;
        }
    }

    /// Der Zustand eines Flows, falls es ihn gibt.
    #[must_use]
    pub fn state_of(&self, id: FlowId) -> Option<FlowState> {
        self.lock()
            .flows
            .get(&id)
            .map(|flow| flow.flow.state.clone())
    }

    /// Wahr, solange der Flow auf eine Entscheidung wartet.
    #[must_use]
    pub fn is_held(&self, id: FlowId) -> bool {
        self.lock().flows.get(&id).is_some_and(FakeFlow::is_held)
    }

    /// Wahr, wenn der Flow zur Durchreiche ans Sprachmodell gehört.
    #[must_use]
    pub fn is_passthrough(&self, id: FlowId) -> bool {
        self.lock()
            .flows
            .get(&id)
            .is_some_and(|flow| flow.passthrough)
    }

    /// Wahr, wenn der Fake diesen Flow kennt.
    #[must_use]
    pub fn knows(&self, id: FlowId) -> bool {
        self.lock().flows.contains_key(&id)
    }

    /// Alles, was der Detail-Bereich zu einem Flow zeigt.
    #[must_use]
    pub fn detail(&self, id: FlowId) -> Option<v1::FlowDetail> {
        self.lock().flows.get(&id).map(FakeFlow::detail)
    }

    /// Die Zeilendarstellungen aller Flows, in Zeitordnung.
    #[must_use]
    pub fn summaries(&self) -> Vec<v1::FlowSummary> {
        self.lock().flows.values().map(FakeFlow::summary).collect()
    }

    /// Meldet ein sitzungsweites Diagnostic im Ereignisstrom.
    pub fn emit_diagnostic(&self, diagnostic: &Diagnostic, at: SystemTime) {
        self.emit(v1::FlowEvent {
            at: Some(timestamp(at)),
            event: Some(v1::flow_event::Event::Diagnostic(diagnostic_to_proto(
                diagnostic,
            ))),
        });
    }

    /// Meldet, dass sich der Regelsatz geändert hat.
    pub fn emit_rules_changed(&self, at: SystemTime) {
        let revision = {
            let mut inner = self.lock();
            inner.rules_revision += 1;
            inner.rules_revision
        };
        self.emit(v1::FlowEvent {
            at: Some(timestamp(at)),
            event: Some(v1::flow_event::Event::RulesChanged(
                v1::flow_event::RulesChanged { revision },
            )),
        });
    }

    /// Legt ein Ereignis in den Rundfunk-Kanal.
    ///
    /// Ohne Zuhörer verfällt es; das ist kein Fehler, sondern der Normalfall
    /// vor dem Start der Oberfläche.
    pub fn emit(&self, event: v1::FlowEvent) {
        let _ = self.events.send(event);
    }

    /// Der Regelsatz, in Reihenfolge.
    #[must_use]
    pub fn rules(&self) -> Vec<v1::Rule> {
        self.lock().rules.clone()
    }

    /// Ersetzt den Regelsatz.
    pub fn set_rules(&self, rules: Vec<v1::Rule>) {
        let mut inner = self.lock();
        inner.rules = rules;
        renumber(&mut inner.rules);
    }

    /// Ändert den Regelsatz unter dem Schloss.
    pub fn with_rules<R>(&self, f: impl FnOnce(&mut Vec<v1::Rule>) -> R) -> R {
        let mut inner = self.lock();
        let result = f(&mut inner.rules);
        renumber(&mut inner.rules);
        result
    }

    /// Die gesetzten Konfigurationswerte, die vom Standard abweichen.
    #[must_use]
    pub fn config_overrides(&self) -> BTreeMap<String, String> {
        self.lock().config.clone()
    }

    /// Setzt einen Konfigurationswert.
    pub fn set_config(&self, key: String, value: String) {
        self.lock().config.insert(key, value);
    }

    /// Id und Zustand der simulierten Sandbox.
    #[must_use]
    pub fn sandbox(&self) -> (SandboxId, v1::SandboxState) {
        let inner = self.lock();
        (inner.sandbox_id, inner.sandbox_state)
    }

    /// Setzt den Zustand der simulierten Sandbox.
    pub fn set_sandbox_state(&self, state: v1::SandboxState) {
        self.lock().sandbox_state = state;
    }

    /// Legt die Antwort bereit, die dieser Flow bekommt, sobald er darf.
    ///
    /// Die Sitzungsdatei kennt die Antwort schon; wann sie gespielt wird,
    /// entscheidet der Nutzer (bei einem gehaltenen Flow) oder die
    /// `auto`-Zeile (bei einer Regel-Entscheidung).
    pub fn arm_response(&self, id: FlowId, response: StoredResponse) {
        self.store_blob(&response.body);
        self.lock().scripted.insert(id, response);
    }

    /// Nimmt die bereitgelegte Antwort heraus.
    #[must_use]
    pub fn take_armed_response(&self, id: FlowId) -> Option<StoredResponse> {
        self.lock().scripted.remove(&id)
    }

    /// Spielt einen erlaubten Flow bis zum Ende.
    ///
    /// Weiterleiten, antworten, aufzeichnen. Ohne bereitgelegte Antwort
    /// entsteht eine leere `200`, damit der Flow nicht in der Schwebe bleibt.
    /// Jeder Schritt geht durch den Automaten; ein unpassender Zustand bricht
    /// die Kette ab, statt sie zu erzwingen.
    pub fn complete_allowed(&self, id: FlowId, at: SystemTime) {
        if self.advance(id, TransitionInput::Forward, at).is_err() {
            return;
        }
        self.complete_forwarded(id, at);
    }

    /// Beantwortet einen bereits weitergeleiteten Flow und zeichnet ihn auf.
    pub fn complete_forwarded(&self, id: FlowId, at: SystemTime) {
        let response = self
            .take_armed_response(id)
            .unwrap_or_else(|| StoredResponse {
                status: 200,
                headers: HeaderMap::new(),
                body: BodyRef::empty(),
                streaming: false,
            });
        let status = response.status;
        let size = response.body.size;
        self.set_response(id, response);
        if self
            .advance(id, TransitionInput::Respond { status }, at)
            .is_err()
        {
            return;
        }
        if size > 0 {
            self.emit(response_chunk_event(id, at, size));
        }
        let _ = self.advance(id, TransitionInput::Record, at);
    }

    /// Zeichnet einen abgelehnten Flow auf.
    pub fn complete_refused(&self, id: FlowId, at: SystemTime) {
        let _ = self.advance(id, TransitionInput::Record, at);
    }

    /// Lässt die Wartezeit eines Flows ablaufen.
    ///
    /// Ein Flow, der inzwischen entschieden wurde, bleibt unberührt: der
    /// Zeitgeber wird nicht abgebrochen, sondern läuft ins Leere.
    pub fn time_out(&self, id: FlowId, at: SystemTime) {
        if !self.is_held(id) {
            return;
        }
        if self.advance(id, TransitionInput::Timeout, at).is_ok() {
            self.complete_refused(id, at);
        }
    }
}

/// Das Fortschrittsereignis einer Antwort.
#[must_use]
pub fn response_chunk_event(id: FlowId, at: SystemTime, bytes_so_far: u64) -> v1::FlowEvent {
    v1::FlowEvent {
        at: Some(timestamp(at)),
        event: Some(v1::flow_event::Event::ResponseChunk(
            v1::flow_event::ResponseChunk {
                flow_id: id.to_string(),
                bytes_so_far,
            },
        )),
    }
}

/// Wirft die ältesten abgeschlossenen Flows weg, sobald es zu viele werden.
///
/// Der Schwellenwert wird auf drei Viertel abgeräumt, damit nicht bei jedem
/// weiteren Flow erneut aufgeräumt werden muss.
fn prune(flows: &mut BTreeMap<FlowId, FakeFlow>, max: usize) {
    if flows.len() <= max {
        return;
    }
    let surplus = flows.len() - max * 3 / 4;
    let doomed: Vec<FlowId> = flows
        .iter()
        .filter(|(_, flow)| flow.flow.state.is_terminal())
        .map(|(id, _)| *id)
        .take(surplus)
        .collect();
    for id in doomed {
        flows.remove(&id);
    }
}

/// Zählt die Halte-Warteschlange fort.
///
/// Abgezogen wird nur, was vorher dazugezählt wurde: `was_held` sagt, ob der
/// Flow vor diesem Übergang in der Warteschlange stand. Eine Entscheidung
/// über einen Flow, der nie gehalten war (Regel, Durchreiche), lässt die
/// Zähler unberührt; sonst liefen `queue_count` und `queue_bytes` mit jedem
/// solchen Flow unter den wahren Stand.
fn update_hold_counters(inner: &mut Inner, was_held: bool, size: u64, event: &FlowEvent) {
    match event {
        FlowEvent::Held { .. } => {
            inner.hold_count = inner.hold_count.saturating_add(1);
            inner.hold_bytes = inner.hold_bytes.saturating_add(size);
        }
        FlowEvent::Decided { .. } | FlowEvent::TimedOut { .. } if was_held => {
            inner.hold_count = inner.hold_count.saturating_sub(1);
            inner.hold_bytes = inner.hold_bytes.saturating_sub(size);
        }
        _ => {}
    }
}

/// Trägt die Zähler der Warteschlange in ein `Held`-Ereignis nach.
fn with_hold_counters(mut event: v1::FlowEvent, bytes: u64, count: u32) -> v1::FlowEvent {
    if let Some(v1::flow_event::Event::Held(held)) = event.event.as_mut() {
        held.queue_bytes = bytes;
        held.queue_count = count;
    }
    event
}

/// Schreibt fort, was ein Ereignis über den Flow hinaus festhält.
fn apply_side_effects(flow: &mut FakeFlow, event: &FlowEvent) {
    match event {
        FlowEvent::Analyzed { findings, .. } => flow.findings.clone_from(findings),
        FlowEvent::Decided {
            decision, source, ..
        } => {
            flow.decision = Some(decision.clone());
            flow.source = Some(*source);
            flow.deadline_at = None;
        }
        FlowEvent::TimedOut { .. } => {
            flow.decision = Some(Decision::TimedOut);
            flow.source = Some(DecisionSource::Timeout);
            flow.deadline_at = None;
        }
        FlowEvent::Failed { error, .. } => flow.upstream_error = Some(*error),
        _ => {}
    }
}

/// Nummeriert die Regeln nach ihrer Position durch, 1-basiert.
///
/// Der Vertrag zählt Positionen ab eins (`proto/humanitl/v1/rules.proto`,
/// `Rule.position`); `0` heißt dort „ans Ende". Der Fake zählte bis zum
/// 2026-09-03 ab null, und die Kommandozeile zeigte deshalb für die erste
/// Regel einen Strich und meldete nach einem `reorder` die Position `0`.
fn renumber(rules: &mut [v1::Rule]) {
    for (index, rule) in rules.iter_mut().enumerate() {
        rule.position = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
    }
}

/// Übersetzt ein Kern-Ereignis in seine Wire-Form.
///
/// Bei [`FlowEvent::Failed`] reist die aufgelöste Adresse aus
/// [`UpstreamError::PrivateAddress`] als Text in `resolved_ip` mit; bei jedem
/// anderen Fehler bleibt das Feld leer.
#[must_use]
pub fn event_to_proto(event: &FlowEvent, flow: &FakeFlow) -> v1::FlowEvent {
    use v1::flow_event::Event;

    let flow_id = flow.flow.id.to_string();
    let wire = match event {
        FlowEvent::Received { .. } => Event::Received(v1::flow_event::Received {
            summary: Some(flow.summary()),
            domain: Some(flow.domain()),
        }),
        FlowEvent::Analyzed { findings, .. } => Event::Analyzed(v1::flow_event::Analyzed {
            flow_id,
            findings: findings.iter().map(finding_to_proto).collect(),
        }),
        FlowEvent::Held { .. } => Event::Held(v1::flow_event::Held {
            flow_id,
            deadline: flow.deadline_at.map(timestamp),
            queue_bytes: 0,
            queue_count: 0,
        }),
        FlowEvent::Decided {
            decision, source, ..
        } => {
            let (kind, reason, rule_id) = decision_fields(Some(decision), Some(*source));
            Event::Decided(v1::flow_event::Decided {
                flow_id,
                kind: kind as i32,
                source: source_to_proto(*source) as i32,
                block_reason: reason as i32,
                rule_id,
                note: block_note(decision),
            })
        }
        FlowEvent::Forwarded { .. } => Event::Forwarded(v1::FlowRef { flow_id }),
        FlowEvent::ResponseHeaders { status, .. } => {
            Event::ResponseHeaders(v1::flow_event::ResponseHeaders {
                flow_id,
                head: Some(v1::HttpResponseHead {
                    status: u32::from(*status),
                    headers: flow
                        .response
                        .as_ref()
                        .map(|response| headers_to_proto(&response.headers))
                        .unwrap_or_default(),
                    version: "HTTP/1.1".to_owned(),
                }),
                streaming: flow.response.as_ref().is_some_and(|r| r.streaming),
            })
        }
        FlowEvent::ResponseChunk { len, .. } => {
            Event::ResponseChunk(v1::flow_event::ResponseChunk {
                flow_id,
                bytes_so_far: *len,
            })
        }
        FlowEvent::Failed { error, .. } => Event::Failed(v1::flow_event::Failed {
            flow_id,
            error: upstream_error_to_proto(*error) as i32,
            resolved_ip: match error {
                UpstreamError::PrivateAddress(ip) => ip.to_string(),
                UpstreamError::Dns
                | UpstreamError::Connect
                | UpstreamError::Tls
                | UpstreamError::Timeout => String::new(),
            },
        }),
        FlowEvent::TimedOut { .. } => Event::TimedOut(v1::FlowRef { flow_id }),
        FlowEvent::Recorded { .. } => Event::Recorded(v1::FlowRef { flow_id }),
        FlowEvent::Lagged { n } => Event::Lagged(v1::flow_event::Lagged { dropped: *n }),
        FlowEvent::Diagnostic { diagnostic, .. } => {
            Event::Diagnostic(diagnostic_to_proto(diagnostic))
        }
    };
    v1::FlowEvent {
        at: Some(timestamp(event.at().unwrap_or(flow.last_at))),
        event: Some(wire),
    }
}

/// Eine Frist als Kern-Zeitpunkt, für den Automaten.
///
/// Der Automat trägt die Frist als [`Instant`]; die Anzeige benutzt die
/// Wanduhrzeit aus [`FakeFlow::deadline_at`]. Beide entstehen aus derselben
/// Dauer, laufen aber getrennt: der Zeitgeber hängt an der Laufzeituhr, die
/// Anzeige am Kalender.
#[must_use]
pub fn deadline_instant(timeout: Duration) -> Instant {
    Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::time::{Duration, SystemTime};

    use bytes::Bytes;
    use humanitl_core::http::{Authority, BodyRef, HttpRequest, Method, Scheme};
    use humanitl_core::{
        Decision, DecisionSource, FlowEvent, FlowId, HostName, RuleId, SessionId, TransitionInput,
        UpstreamError,
    };

    use super::{FakeFlow, FakeState, SessionMeta, deadline_instant, event_to_proto};
    use crate::convert::{
        BODY_PREVIEW_CHARS, EditedRequestError, apex_of, body_preview, request_from_proto,
        timestamp,
    };
    use crate::v1;

    /// Eine `EditedRequest` mit Methode und URL, sonst leer.
    fn edited(method: v1::Method, raw: &str, url: &str) -> v1::EditedRequest {
        v1::EditedRequest {
            method: method as i32,
            method_raw: raw.to_owned(),
            url: url.to_owned(),
            ..v1::EditedRequest::default()
        }
    }

    #[test]
    fn body_preview_is_lossy_utf8_capped_at_4096_chars() {
        assert_eq!(body_preview(b""), "");
        assert_eq!(body_preview(b"{\"a\":1}"), "{\"a\":1}");
        assert_eq!(body_preview(&[0x61, 0xff, 0x62]), "a\u{fffd}b");

        let two_byte = "ä".repeat(BODY_PREVIEW_CHARS + 904);
        let preview = body_preview(two_byte.as_bytes());
        assert_eq!(preview.chars().count(), BODY_PREVIEW_CHARS);
        assert!(preview.chars().all(|c| c == 'ä'));

        // Vier Bytes je Zeichen: die Lesegrenze faellt genau auf 4096 Zeichen.
        let four_byte = "\u{1f600}".repeat(BODY_PREVIEW_CHARS + 1);
        let preview = body_preview(four_byte.as_bytes());
        assert_eq!(preview.chars().count(), BODY_PREVIEW_CHARS);
        assert!(preview.chars().all(|c| c == '\u{1f600}'));

        // Ein Byte je Zeichen: die Lesegrenze liegt weit hinter dem Schnitt.
        let one_byte = "x".repeat(BODY_PREVIEW_CHARS * 8);
        assert_eq!(body_preview(one_byte.as_bytes()).len(), BODY_PREVIEW_CHARS);
    }

    #[test]
    fn edited_request_reads_url_method_headers_and_body() {
        let wire = v1::EditedRequest {
            method: v1::Method::Post as i32,
            method_raw: String::new(),
            url: "https://api.github.com/repos?per_page=1".to_owned(),
            headers: vec![v1::Header {
                name: "content-type".to_owned(),
                value: b"application/json".to_vec(),
            }],
            body: b"{\"edited\":true}".to_vec(),
        };

        let request = request_from_proto(&wire).expect("readable");

        assert_eq!(request.method, Method::POST);
        assert_eq!(request.scheme, Scheme::Https);
        assert_eq!(request.authority.port, 443);
        assert_eq!(request.url(), "https://api.github.com/repos?per_page=1");
        assert_eq!(
            request.body.inline.as_deref(),
            Some(&b"{\"edited\":true}"[..])
        );
        assert_eq!(request.body.size, 15);
        assert_eq!(
            request.body.sha256,
            humanitl_core::http::sha256(b"{\"edited\":true}")
        );
        assert_eq!(
            request.body.content_type.as_deref(),
            Some("application/json")
        );
        assert_eq!(request.headers.len(), 1);
    }

    #[test]
    fn edited_request_accepts_ports_ipv6_raw_methods_and_bare_hosts() {
        let request = request_from_proto(&edited(v1::Method::Get, "", "http://[::1]:8080"))
            .expect("ipv6 with port");
        assert_eq!(request.authority.port, 8080);
        assert_eq!(request.path_and_query, "/");
        assert_eq!(
            request.host(),
            &HostName::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST))
        );

        let request = request_from_proto(&edited(v1::Method::Get, "", "ws://example.com?x=1"))
            .expect("query without path");
        assert_eq!(request.path_and_query, "/?x=1");
        assert_eq!(request.scheme, Scheme::Ws);
        assert_eq!(request.authority.port, 80);

        let request = request_from_proto(&edited(v1::Method::Get, "", "https://Example.COM./a"))
            .expect("host normalises");
        assert_eq!(request.host().to_string(), "example.com");

        let request = request_from_proto(&edited(
            v1::Method::Other,
            "PROPFIND",
            "https://dav.example/",
        ))
        .expect("raw method");
        assert_eq!(request.method.as_str(), "PROPFIND");
    }

    #[test]
    fn edited_request_refuses_what_it_cannot_read() {
        use EditedRequestError as E;
        use v1::Method as M;
        let cases = [
            (M::Unspecified, "", "https://a.example/", E::Method),
            (M::Other, "", "https://a.example/", E::Method),
            (M::Other, "NOT A TOKEN", "https://a.example/", E::Method),
            (M::Get, "", "a.example/", E::Scheme),
            (M::Get, "", "ftp://a.example/", E::Scheme),
            (M::Get, "", "https:///x", E::Host),
            (M::Get, "", "https://[::1/", E::Host),
            (M::Get, "", "https://a.example:0/", E::Port),
            (M::Get, "", "https://a.example:x/", E::Port),
            (M::Get, "", "https://a.example/#top", E::Target),
            (M::Get, "", "https://u@a.example/", E::Target),
        ];
        for (method, raw, url, expected) in cases {
            let outcome = request_from_proto(&edited(method, raw, url)).map(drop);
            assert_eq!(outcome, Err(expected), "{method:?} {raw:?} {url}");
        }
    }

    fn request(host: &str) -> HttpRequest {
        let host = HostName::parse(host).expect("host");
        HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(host, Scheme::Https),
            "/",
        )
    }

    /// Die Domain-Anzeige des Fakes trägt nur, was aus der Datei folgt: den
    /// Apex. Rang und Katalog-Eintrag sind „unbekannt", nicht erfunden.
    #[test]
    fn domain_info_carries_no_invented_rank() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        for host in ["api.github.com", "registry.npmjs.org", "models.dev"] {
            let flow = FakeFlow::new(FlowId::new(), SessionId::new(), at, request(host));
            let domain = flow.domain();
            assert_eq!(domain.tranco_rank, 0, "{host}: no rank was looked up");
            assert!(
                domain.catalog_id.is_empty(),
                "{host}: no catalog entry exists"
            );
            assert_eq!(domain.apex, apex_of(&HostName::parse(host).unwrap()));
            assert_eq!(domain.first_seen, Some(timestamp(at)));
        }
    }

    #[test]
    fn apex_takes_the_last_two_labels() {
        assert_eq!(
            apex_of(&HostName::parse("registry.npmjs.org").unwrap()),
            "npmjs.org"
        );
        assert_eq!(
            apex_of(&HostName::parse("example.org").unwrap()),
            "example.org"
        );
        assert_eq!(
            apex_of(&HostName::parse("192.168.1.50").unwrap()),
            "192.168.1.50"
        );
    }

    #[test]
    fn prune_drops_the_oldest_finished_flows_and_keeps_the_waiting_ones() {
        use std::collections::BTreeMap;

        use humanitl_core::TransitionInput;

        let session = SessionId::new();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let mut flows = BTreeMap::new();
        let mut waiting = Vec::new();
        for index in 0..8 {
            let id = FlowId::new();
            let mut flow = FakeFlow::new(id, session, at, request("registry.npmjs.org"));
            flow.flow
                .apply(
                    TransitionInput::Analyze {
                        findings: Vec::new(),
                    },
                    at,
                )
                .expect("analyze");
            if index % 4 == 0 {
                flow.flow
                    .apply(
                        TransitionInput::Hold {
                            deadline: super::deadline_instant(Duration::from_secs(300)),
                            queue_bytes: 0,
                            queue_count: 0,
                        },
                        at,
                    )
                    .expect("hold");
                waiting.push(id);
            } else {
                flow.flow
                    .apply(
                        TransitionInput::Decide {
                            decision: humanitl_core::Decision::Block {
                                reason: humanitl_core::BlockReason::User,
                                note: None,
                            },
                            source: humanitl_core::DecisionSource::Rule(
                                humanitl_core::RuleId::new(),
                            ),
                        },
                        at,
                    )
                    .expect("decide");
                flow.flow
                    .apply(TransitionInput::Record, at)
                    .expect("record");
            }
            flows.insert(id, flow);
        }

        super::prune(&mut flows, 4);
        assert_eq!(flows.len(), 3, "down to three quarters of the limit");
        for id in waiting {
            assert!(flows.contains_key(&id), "a waiting flow is never dropped");
        }
        super::prune(&mut flows, 100);
        assert_eq!(flows.len(), 3, "below the limit nothing is dropped");
    }

    /// Ein Flow im Zustand `Analyzed`, mit einem Body dieser Größe.
    fn analyzed(state: &FakeState, session: SessionId, at: SystemTime, size: usize) -> FlowId {
        let id = FlowId::new();
        let request = request("registry.npmjs.org")
            .with_body(BodyRef::from_bytes(Bytes::from(vec![b'x'; size])));
        state.receive(FakeFlow::new(id, session, at, request));
        state
            .advance(id, TransitionInput::Analyze { findings: vec![] }, at)
            .expect("analyze");
        id
    }

    fn hold(state: &FakeState, id: FlowId, at: SystemTime) {
        state
            .advance(
                id,
                TransitionInput::Hold {
                    deadline: deadline_instant(Duration::from_secs(300)),
                    queue_bytes: 0,
                    queue_count: 0,
                },
                at,
            )
            .expect("hold");
    }

    fn decide(state: &FakeState, id: FlowId, source: DecisionSource, at: SystemTime) {
        state
            .advance(
                id,
                TransitionInput::Decide {
                    decision: Decision::Allow,
                    source,
                },
                at,
            )
            .expect("decide");
    }

    /// Die Zähler des jüngsten `Held`-Ereignisses im Kanal.
    fn last_held(receiver: &mut tokio::sync::broadcast::Receiver<v1::FlowEvent>) -> (u32, u64) {
        let mut last = None;
        while let Ok(event) = receiver.try_recv() {
            if let Some(v1::flow_event::Event::Held(held)) = event.event {
                last = Some((held.queue_count, held.queue_bytes));
            }
        }
        last.expect("a held event was emitted")
    }

    #[test]
    fn hold_counters_only_drop_for_flows_that_were_held() {
        let session = SessionMeta::default();
        let state = FakeState::new(session.clone(), 64);
        let mut receiver = state.subscribe();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let rule = DecisionSource::Rule(RuleId::new());

        let first_held = analyzed(&state, session.id, at, 10);
        hold(&state, first_held, at);
        assert_eq!(last_held(&mut receiver), (1, 10));

        // Eine Regel entscheidet einen Flow, der nie gehalten war: die
        // Warteschlange bleibt, wie sie ist.
        let never_held = analyzed(&state, session.id, at, 100);
        decide(&state, never_held, rule, at);

        let second_held = analyzed(&state, session.id, at, 20);
        hold(&state, second_held, at);
        assert_eq!(
            last_held(&mut receiver),
            (2, 30),
            "never_held was never in the queue"
        );

        // Der Mensch entscheidet einen gehaltenen Flow: der verlässt sie.
        decide(&state, first_held, DecisionSource::User, at);
        let third_held = analyzed(&state, session.id, at, 5);
        hold(&state, third_held, at);
        assert_eq!(
            last_held(&mut receiver),
            (2, 25),
            "first_held left, second_held and third_held remain"
        );

        // Ein Ablauf zählt ebenfalls nur, was gehalten war.
        state.time_out(second_held, at);
        state.time_out(never_held, at);
        let fourth_held = analyzed(&state, session.id, at, 1);
        hold(&state, fourth_held, at);
        assert_eq!(
            last_held(&mut receiver),
            (2, 6),
            "third_held and fourth_held remain"
        );
    }

    #[test]
    fn a_failed_flow_is_reported_with_its_error_and_the_resolved_ip() {
        let session = SessionMeta::default();
        let state = FakeState::new(session.clone(), 16);
        let mut receiver = state.subscribe();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));

        let id = analyzed(&state, session.id, at, 0);
        decide(&state, id, DecisionSource::Rule(RuleId::new()), at);
        state
            .advance(id, TransitionInput::Forward, at)
            .expect("forward");
        state
            .advance(
                id,
                TransitionInput::Fail {
                    error: UpstreamError::PrivateAddress(ip),
                },
                at,
            )
            .expect("fail");

        let mut failed = None;
        while let Ok(event) = receiver.try_recv() {
            if let Some(v1::flow_event::Event::Failed(event)) = event.event {
                failed = Some(event);
            }
        }
        let failed = failed.expect("the failure reaches the wire");
        assert_eq!(failed.flow_id, id.to_string());
        assert_eq!(failed.error, v1::UpstreamError::PrivateAddress as i32);
        assert_eq!(failed.resolved_ip, "10.0.0.7");

        let summary = state.summaries().remove(0);
        assert_eq!(summary.state, v1::FlowState::Failed as i32);
        assert_eq!(
            summary.upstream_error,
            v1::UpstreamError::PrivateAddress as i32
        );

        // Ohne private Adresse bleibt das Feld leer.
        let flow = FakeFlow::new(FlowId::new(), session.id, at, request("example.org"));
        let wire = event_to_proto(
            &FlowEvent::Failed {
                flow_id: flow.flow.id,
                at,
                error: UpstreamError::Dns,
            },
            &flow,
        );
        let Some(v1::flow_event::Event::Failed(event)) = wire.event else {
            panic!("a failed event maps to Failed, got {wire:?}");
        };
        assert_eq!(event.error, v1::UpstreamError::Dns as i32);
        assert_eq!(event.resolved_ip, "");
    }

    #[test]
    fn timestamp_of_the_epoch_is_zero() {
        let zero = timestamp(SystemTime::UNIX_EPOCH);
        assert_eq!(zero.seconds, 0);
        assert_eq!(zero.nanos, 0);
    }

    #[test]
    fn advance_emits_and_updates_the_summary() {
        let session = SessionMeta::default();
        let state = FakeState::new(session.clone(), 16);
        let mut receiver = state.subscribe();
        let id = FlowId::new();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        state.receive(FakeFlow::new(
            id,
            session.id,
            at,
            request("registry.npmjs.org"),
        ));
        state
            .advance(id, TransitionInput::Analyze { findings: vec![] }, at)
            .expect("analyze");

        let first = receiver.try_recv().expect("received event");
        assert!(matches!(
            first.event,
            Some(v1::flow_event::Event::Received(_))
        ));
        let second = receiver.try_recv().expect("analyzed event");
        assert!(matches!(
            second.event,
            Some(v1::flow_event::Event::Analyzed(_))
        ));

        let summaries = state.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].state, v1::FlowState::Analyzed as i32);
        assert_eq!(summaries[0].session_id, session.id.to_string());
    }
}
