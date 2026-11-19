//! Das Verzeichnis aller Flows einer Sitzung (ADR-004, HUM-016).
//!
//! Die [`HoldQueue`](crate::hold::HoldQueue) weiß nur, wer gerade wartet. Wer
//! wissen will, was aus einem Flow geworden ist — die Oberfläche über
//! `ListFlows`, das Terminal, später der Recorder —, fragt die
//! [`FlowRegistry`]. Sie hält je Flow einen [`FlowRecord`] und teilt sich den
//! `broadcast`-Kanal mit der Warteschlange: es gibt genau einen Ereignisstrom,
//! nicht zwei.
//!
//! # Wer den Zustand fortschreibt
//!
//! Der Proxy-Handler besitzt seinen [`Flow`] und treibt den Automaten des
//! Kerns; die Warteschlange leiht ihn sich für die Dauer des Haltens. Die
//! Registry führt davon eine zweite, gleichlaufende Buchführung:
//!
//! - [`FlowRegistry::insert`] legt den Datensatz an, sobald der Flow eine
//!   Pipeline erreicht (oder schon davor abgelehnt wird).
//! - [`FlowRegistry::record`] schreibt ihn fort, wenn ein Ereignis
//!   veröffentlicht wird. Der einzige Weg dorthin ist
//!   [`HoldQueue::publish`](crate::hold::HoldQueue::publish), also läuft jedes
//!   Ereignis des Proxys genau einmal hier vorbei.
//! - [`FlowRegistry::transition`] ist der umgekehrte Weg für Aufrufer ohne
//!   eigenen [`Flow`] (die gRPC-Schicht, HUM-018): sie wendet den Übergang auf
//!   den Datensatz an und veröffentlicht das Ereignis selbst.
//!
//! Beide Wege gehen durch [`FlowState::on`]; ein Übergang, den der Automat
//! nicht kennt, ändert auch hier nichts.
//!
//! # Was hier nicht ist
//!
//! Persistenz. Alles liegt im Speicher, nach einem Neustart des Daemons ist
//! die Registry leer; ab HUM-026 beantwortet der Recorder `ListFlows` aus
//! `SQLite`. `created` ist deshalb eine [`SystemTime`] wie
//! [`Flow::received_at`], nicht der `DateTime<Utc>` aus der Skizze des Issues
//! (der Kern kannte damals keine Zeitbibliothek).
//!
//! # Wie viel sie behält (HUM-556)
//!
//! Abgeschlossene Flows ([`FlowState::Recorded`]) behält die Registry nur bis
//! zur Zahl `limits.registry_max_finished_flows`; darüber fällt der älteste
//! abgeschlossene heraus. Ein Flow erreicht `Recorded` über
//! [`HoldQueue::publish`](crate::hold::HoldQueue::publish), das jedes Ereignis
//! zuerst hierher und dann an den Recorder reicht; der älteste
//! abgeschlossene hat sein `Recorded` deshalb schon an den Recorder gegeben.
//! [`FlowRegistry::transition`] (die gRPC-Schicht) sendet nur in den Strom,
//! nicht an den Recorder; es treibt aber gehaltene Flows, die nie
//! herausfallen, und ein Abschluss auf diesem Weg zählt nur gegen dieselbe
//! Grenze.
//! Gehaltene und laufende Flows zählen nicht mit und fallen nie heraus: Ihre
//! Zahl begrenzen schon `limits.hold_max_flows` und
//! `limits.max_client_connections`. Ohne diese Grenze wuchs die Registry mit
//! jeder Anfrage, und ein Gast mit vielen kleinen Anfragen über eine einzige
//! Verbindung füllte über die Zeit den Speicher des Daemons.
//!
//! Gemessen mit `tests/registry_memory.rs` (2026-11-19, Linux auf `x86_64`): Ein
//! Datensatz einer kleinen Anfrage (sechs Kopfzeilen, Pfad mit Query,
//! durchlaufen bis `Recorded`) belegt samt seinem Platz in der Tabelle 2,3
//! bis 2,6 KiB auf dem Heap, davon 816 Bytes im Datensatz selbst. Ohne Grenze
//! wuchs die Registry um so viel je Anfrage, nach 100 000 Anfragen also um
//! rund 250 MiB. Mit der Vorgabe von 10 000 steigt sie bis rund 36 MiB, weil
//! die Splitter der Tabelle Luft behalten, und bleibt dort: nach 80 000
//! Anfragen 37,6 MB, nach 1 280 000 Anfragen ebenso.
//!
//! Was danach nur noch die Registry wusste, ist mit dem Datensatz weg: ob der
//! Scan die ganze Anfrage gesehen hat ([`FlowRecord::findings_truncated`]) und
//! der Befund der harten Sperre ([`FlowRecord::send_refusal`]). Beides braucht
//! die Oberfläche, solange der Flow frisch ist, nicht zehntausend Flows
//! später. Ohne Aufzeichnung zeigt `ListFlows` nur die letzten abgeschlossenen
//! Flows.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

use dashmap::DashMap;
use humanitl_config::Limits;
use humanitl_core::{
    Authority, Decision, DecisionSource, Flow, FlowEvent, FlowId, FlowState, HttpRequest,
    InvalidTransition, Method, Scheme, SessionId, Transition, TransitionInput,
};
use tokio::sync::broadcast;

use crate::connect::ConnectionContext;
use crate::hold::MAX_EVENT_BUFFER;

/// Der Ausgangszustand in [`InvalidTransition`], wenn es den Flow gar nicht gibt.
const UNREGISTERED: &str = "unregistered";

/// Alles, was die Registry über einen Flow weiß.
///
/// Eine Kopie dessen, was der Handler in seinem [`Flow`] führt, ergänzt um die
/// Verbindungsdaten ([`ConnectionContext`]) und die Felder, die eine Liste braucht,
/// ohne die Historie durchsuchen zu müssen.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowRecord {
    /// Id des Flows.
    pub id: FlowId,
    /// Sitzung, zu der er gehört.
    pub session: SessionId,
    /// Der aktuelle Zustand.
    pub state: FlowState,
    /// Die Anfrage, wie sie ankam.
    pub request: HttpRequest,
    /// Die Verbindung, aus der er stammt.
    pub meta: ConnectionContext,
    /// Wann die Anfrage ankam.
    pub created: SystemTime,
    /// Die Frist, solange der Flow gehalten wird; sonst `None`.
    pub deadline: Option<Instant>,
    /// Die Entscheidung, sobald es eine gibt.
    pub decision: Option<Decision>,
    /// Wer entschieden hat, sobald es eine Entscheidung gibt.
    ///
    /// Gefüllt aus [`FlowEvent::Decided`] und aus [`FlowEvent::TimedOut`]
    /// (dann [`DecisionSource::Timeout`]). Vorher `None`, und `None` bleibt es
    /// auch für einen Datensatz, der schon entschieden angelegt wurde: die
    /// Herkunft steht nur im Ereignis, nicht im Zustand, und wird nicht
    /// geraten.
    pub decision_source: Option<DecisionSource>,
    /// Der Status der Antwort des Ziels, sobald er da ist.
    pub response_status: Option<u16>,
    /// Wie viele Bytes des Antwortkörpers bisher durchgelaufen sind.
    ///
    /// Summe der [`FlowEvent::ResponseChunk`]; wächst, solange die Antwort
    /// streamt, und steht still, sobald der Flow zu Ende ist. `0` heißt „noch
    /// nichts gesehen", nicht „unbekannt": eine Antwort ohne Körper und eine
    /// Anfrage ohne Antwort sind beide null Bytes.
    pub response_bytes: u64,
    /// Wann der Flow zu Ende war, sonst `None`.
    ///
    /// Der Zeitpunkt des [`FlowEvent::Recorded`], also des einzigen
    /// Endzustands des Automaten ([`FlowState::Recorded`]). Ein Flow, der noch
    /// läuft oder dessen Antwort noch streamt, hat kein Ende und deshalb auch
    /// keine Dauer.
    pub finished: Option<SystemTime>,
    /// Wahr, wenn der Scan der Detektoren nicht die ganze Anfrage gesehen hat
    /// (`ScanReport::truncated`, HUM-025).
    ///
    /// Der Ort, an dem die Oberfläche es abholt: Eine leere Fundliste zu einer
    /// nur teilweise durchsuchten Anfrage ist kein Freispruch, und die Karte
    /// muss den Unterschied zeigen können. Gesetzt wird das Feld vom Handler,
    /// der als Einziger den Bericht des Scans in der Hand hat; die Befunde,
    /// die die Lücke erklären (`FINDINGS_002`), stehen zusätzlich als
    /// [`FlowEvent::Diagnostic`] am selben Flow.
    pub findings_truncated: bool,
    /// Wie viele Funde mit der Freigabe hinausgingen, ohne ersetzt oder
    /// bestätigt zu sein (HUM-160); aus dem letzten [`FlowEvent::Decided`].
    ///
    /// `None`, solange nicht entschieden ist, wenn nichts hinausging, und wenn
    /// niemand gezählt hat. Nimmt das System eine Freigabe zurück, steht hier
    /// wieder `None`: Hinausgegangen ist dann nichts.
    pub unresolved_findings: Option<u32>,
    /// Der Befund der harten Sperre, wenn der Flow einen trägt
    /// ([`Flow::send_refusal`], HUM-159).
    ///
    /// Hier steht er für die Zeile (`FlowSummary.send_refusal`): Ein Client,
    /// der die Ansage im Strom verpasst hat, liest ihn beim Nachladen. Die
    /// Prüfung vor einer Freigabe liest ihn nicht hier, sondern in der
    /// Warteschlange, denn die Registry darf einen Datensatz vergessen.
    pub send_refusal: Option<humanitl_core::Diagnostic>,
    /// Die Befunde, die am Fluss stehen, in der Reihenfolge ihres Eintreffens
    /// (aus [`FlowEvent::Diagnostic`], HUM-057).
    ///
    /// Hier holt das Detail sie ab (`FlowDetail.diagnostics`): Ein Befund im
    /// Strom ist flüchtig, und wer den Fluss später in der Historie öffnet,
    /// sähe sonst „geblockt, `body_cap`" ohne Grund und ohne den Vorschlag,
    /// der dazu gehört (`LIMIT_001`). Höchstens [`MAX_FLOW_DIAGNOSTICS`]; was
    /// darüber kommt, fällt weg, damit ein Fluss mit vielen Befunden die
    /// Registry nicht wachsen lässt.
    pub diagnostics: Vec<humanitl_core::Diagnostic>,
}

/// So viele Befunde behält ein [`FlowRecord`] höchstens.
///
/// Dieselbe Zahl hält die Aufzeichnung ein (HUM-256); sie steht deshalb im
/// Kern und hier nur unter ihrem alten Namen.
pub use humanitl_core::MAX_FLOW_DIAGNOSTICS;

impl FlowRecord {
    /// Der Datensatz zu einem Flow, so wie er gerade steht.
    #[must_use]
    pub fn new(flow: &Flow, meta: &ConnectionContext) -> Self {
        let mut record = Self {
            id: flow.id,
            session: flow.session,
            state: flow.state.clone(),
            request: flow.request.clone(),
            meta: meta.clone(),
            created: flow.received_at,
            deadline: None,
            decision: None,
            decision_source: None,
            response_status: None,
            response_bytes: 0,
            finished: None,
            findings_truncated: false,
            unresolved_findings: None,
            send_refusal: flow.send_refusal.clone(),
            diagnostics: Vec::new(),
        };
        record.refresh_deadline();
        record
    }

    /// Die Zeilendarstellung für [`FlowRegistry::list`].
    #[must_use]
    pub fn summary(&self) -> FlowSummary {
        FlowSummary {
            id: self.id,
            session: self.session,
            created: self.created,
            method: self.request.method.clone(),
            scheme: self.request.scheme,
            authority: self.request.authority.clone(),
            path_and_query: self.request.path_and_query.clone(),
            state: self.state.clone(),
            deadline: self.deadline,
            decision: self.decision.clone(),
            response_status: self.response_status,
            request_bytes: self.request.body.size,
        }
    }

    /// Übernimmt, was ein Ereignis über den Zustand hinaus festhält.
    ///
    /// Läuft auch für die Ereignisse ohne Übergang, weil
    /// [`FlowEvent::ResponseChunk`] den Zustand nicht ändert, aber die einzige
    /// Quelle für die Größe der Antwort ist.
    fn absorb(&mut self, event: &FlowEvent) {
        match event {
            FlowEvent::Decided {
                decision,
                source,
                findings,
                ..
            } => {
                self.decision = Some(decision.clone());
                self.decision_source = Some(*source);
                self.unresolved_findings = findings.unresolved;
            }
            FlowEvent::TimedOut { .. } => {
                self.decision = Some(Decision::TimedOut);
                self.decision_source = Some(DecisionSource::Timeout);
            }
            FlowEvent::ResponseHeaders { status, .. } => self.response_status = Some(*status),
            FlowEvent::ResponseChunk { len, .. } => {
                self.response_bytes = self.response_bytes.saturating_add(*len);
            }
            FlowEvent::Recorded { at, .. } => self.finished = Some(*at),
            FlowEvent::Diagnostic { diagnostic, .. }
                if self.diagnostics.len() < MAX_FLOW_DIAGNOSTICS =>
            {
                self.diagnostics.push((**diagnostic).clone());
            }
            _ => {}
        }
        self.refresh_deadline();
    }

    /// Die Frist gilt genau so lange, wie der Flow gehalten wird.
    fn refresh_deadline(&mut self) {
        self.deadline = match &self.state {
            FlowState::Held { deadline } => Some(*deadline),
            _ => None,
        };
    }
}

/// Eine Zeile der Flow-Liste.
///
/// Das Gegenstück zu `FlowSummary` in `proto/humanitl/v1/humanitl.proto`; die
/// Abbildung auf die Wire-Form macht HUM-018, nicht diese Crate.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowSummary {
    /// Id des Flows.
    pub id: FlowId,
    /// Sitzung, zu der er gehört.
    pub session: SessionId,
    /// Wann die Anfrage ankam.
    pub created: SystemTime,
    /// Die Methode der Anfrage.
    pub method: Method,
    /// Das Schema der Anfrage.
    pub scheme: Scheme,
    /// Ziel-Host und Port.
    pub authority: Authority,
    /// Pfad samt Query.
    pub path_and_query: String,
    /// Der aktuelle Zustand.
    pub state: FlowState,
    /// Die Frist, solange der Flow gehalten wird.
    pub deadline: Option<Instant>,
    /// Die Entscheidung, sobald es eine gibt.
    pub decision: Option<Decision>,
    /// Der Status der Antwort des Ziels, sobald er da ist.
    pub response_status: Option<u16>,
    /// Größe des Request-Bodys in Bytes.
    pub request_bytes: u64,
}

/// Wonach [`FlowRegistry::list`] auswählt.
///
/// Alle Felder sind Und-Verknüpfungen; ein leerer Filter
/// ([`FlowFilter::default`]) nimmt jeden Flow. Die Suchsyntax der Oberfläche
/// (`host:github.com state:blocked`) wird in HUM-018 auf diese Felder
/// abgebildet, nicht hier geparst.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowFilter {
    /// Nur Flows dieser Sitzung.
    pub session: Option<SessionId>,
    /// Nur Flows zu diesem Host, in kanonischer Form (`HostName`-Display).
    pub host: Option<String>,
    /// Nur Flows in diesem Zustand, benannt wie [`FlowState::name`].
    pub state: Option<String>,
    /// Nur Flows, die gerade gehalten werden.
    pub held_only: bool,
    /// Höchstens so viele Zeilen; `None` heißt alle.
    pub limit: Option<usize>,
}

impl FlowFilter {
    /// Ein Filter, der nur die wartenden Flows nimmt (die Queue der Oberfläche).
    #[must_use]
    pub const fn held() -> Self {
        Self {
            session: None,
            host: None,
            state: None,
            held_only: true,
            limit: None,
        }
    }

    /// Ein Filter, der nur Flows dieser Sitzung nimmt.
    #[must_use]
    pub const fn session(session: SessionId) -> Self {
        Self {
            session: Some(session),
            host: None,
            state: None,
            held_only: false,
            limit: None,
        }
    }

    /// Passt dieser Datensatz?
    #[must_use]
    pub fn matches(&self, record: &FlowRecord) -> bool {
        if let Some(session) = self.session
            && record.session != session
        {
            return false;
        }
        if let Some(host) = &self.host
            && record.request.authority.host.to_string() != *host
        {
            return false;
        }
        if let Some(state) = &self.state
            && record.state.name() != state
        {
            return false;
        }
        if self.held_only && !matches!(record.state, FlowState::Held { .. }) {
            return false;
        }
        true
    }
}

/// Alle Flows einer Sitzung, siehe Modulkommentar.
///
/// Eine je Daemon, geteilt zwischen Proxy und gRPC-Schicht; `Send + Sync`,
/// üblicherweise in einem `Arc`.
#[derive(Debug)]
pub struct FlowRegistry {
    flows: DashMap<FlowId, FlowRecord>,
    events: broadcast::Sender<FlowEvent>,
    /// Die abgeschlossenen Flows in der Reihenfolge ihres Abschlusses, der
    /// älteste vorn; nie länger als `max_finished`.
    finished: Mutex<VecDeque<FlowId>>,
    /// `limits.registry_max_finished_flows`, mindestens 1.
    max_finished: usize,
}

impl FlowRegistry {
    /// Eine leere Registry mit einem Ereignisstrom der Kapazität
    /// `limits.event_buffer`.
    ///
    /// Die Kapazität wird wie in der Warteschlange auf
    /// `1..=`[`MAX_EVENT_BUFFER`] begrenzt. Wer Registry und Warteschlange
    /// zusammen braucht, baut die Registry zuerst und übergibt sie an
    /// [`HoldQueue::with_registry`](crate::hold::HoldQueue::with_registry);
    /// dann teilen sich beide diesen Kanal.
    #[must_use]
    pub fn new(limits: &Limits) -> Self {
        let capacity = limits.event_buffer.clamp(1, MAX_EVENT_BUFFER);
        let (events, _idle) = broadcast::channel(capacity);
        let max_finished =
            usize::try_from(limits.registry_max_finished_flows.max(1)).unwrap_or(usize::MAX);
        Self {
            flows: DashMap::new(),
            events,
            finished: Mutex::new(VecDeque::new()),
            max_finished,
        }
    }

    /// Nimmt einen Flow auf. Ein vorhandener Datensatz derselben Id wird
    /// ersetzt.
    ///
    /// Ist der Flow schon abgeschlossen, zählt er sofort gegen die Grenze der
    /// abgeschlossenen Flows (siehe Modulkommentar).
    pub fn insert(&self, rec: FlowRecord) {
        let id = rec.id;
        let finished = rec.state.is_terminal();
        self.flows.insert(id, rec);
        if finished {
            self.retire(id);
        }
    }

    /// Wendet einen Übergang auf den Datensatz an und veröffentlicht sein
    /// Ereignis.
    ///
    /// Der Weg für Aufrufer, die keinen eigenen [`Flow`] halten. Wer einen hat
    /// (Handler, Warteschlange), treibt den Automaten dort und lässt die
    /// Registry über [`FlowRegistry::record`] mitlaufen.
    ///
    /// # Errors
    ///
    /// [`InvalidTransition`], wenn der Automat das Paar aus Zustand und
    /// Eingabe nicht kennt oder der Flow nicht in der Registry steht
    /// (`from: "unregistered"`). Der Datensatz bleibt dann unverändert und
    /// nichts wird veröffentlicht.
    pub fn transition(
        &self,
        id: FlowId,
        input: TransitionInput,
    ) -> Result<FlowEvent, InvalidTransition> {
        let (event, finished) = {
            let mut entry = self.flows.get_mut(&id).ok_or(InvalidTransition {
                from: UNREGISTERED,
                input: input.name(),
            })?;
            let record = entry.value_mut();
            let (next, event) =
                record
                    .state
                    .clone()
                    .on(Transition::new(id, SystemTime::now(), input))?;
            let finished = !record.state.is_terminal() && next.is_terminal();
            record.state = next;
            record.absorb(&event);
            (event, finished)
        };
        // Erst den Guard fallen lassen, dann verdrängen und senden: kein Guard
        // über einen fremden Aufruf hinweg, und keiner, während `retire` in
        // derselben Tabelle löscht.
        if finished {
            self.retire(id);
        }
        let _ = self.events.send(event.clone());
        Ok(event)
    }

    /// Schreibt den Datensatz anhand eines schon veröffentlichten Ereignisses
    /// fort.
    ///
    /// Der Gegenweg zu [`FlowRegistry::transition`]: hier hat jemand anderes
    /// den Übergang bereits auf seinem [`Flow`] ausgeführt, die Registry zieht
    /// nur nach. Ereignisse ohne Flow ([`FlowEvent::Lagged`],
    /// [`FlowEvent::AgentAsk`]) oder zu einem
    /// unbekannten Flow ändern nichts. Ereignisse ohne Übergang lassen den
    /// Zustand stehen, tragen aber bei, was sie an Zahlen mitbringen: ein
    /// [`FlowEvent::ResponseChunk`] erhöht [`FlowRecord::response_bytes`], ein
    /// [`FlowEvent::Diagnostic`] landet in [`FlowRecord::diagnostics`].
    pub fn record(&self, event: &FlowEvent) {
        let Some(id) = event.flow_id() else {
            return;
        };
        let Some(mut entry) = self.flows.get_mut(&id) else {
            return;
        };
        let record = entry.value_mut();
        let Some(input) = transition_input(event) else {
            // Kein Übergang, aber möglicherweise eine Zahl: ein
            // `ResponseChunk` zählt zur Größe der Antwort.
            record.absorb(event);
            return;
        };
        let at = event.at().unwrap_or_else(SystemTime::now);
        match record.state.clone().on(Transition::new(id, at, input)) {
            Ok((next, _mirrored)) => {
                let finished = !record.state.is_terminal() && next.is_terminal();
                record.state = next;
                record.absorb(event);
                // Der Guard muss weg sein, bevor `retire` in derselben
                // Tabelle löscht: `DashMap` sperrt je Splitter, und der
                // älteste Flow kann im selben liegen.
                drop(entry);
                if finished {
                    self.retire(id);
                }
            }
            Err(err) => {
                tracing::debug!(
                    flow = %id,
                    event = event.name(),
                    %err,
                    "the registry could not follow this event; the record keeps its state"
                );
            }
        }
    }

    /// Der Datensatz eines Flows, falls die Registry ihn kennt.
    #[must_use]
    pub fn get(&self, id: FlowId) -> Option<FlowRecord> {
        self.flows.get(&id).map(|entry| entry.value().clone())
    }

    /// Ein neuer Zuhörer am gemeinsamen Ereignisstrom.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<FlowEvent> {
        self.events.subscribe()
    }

    /// Der Sender des Ereignisstroms, den sich Registry und Warteschlange
    /// teilen.
    #[must_use]
    pub const fn events(&self) -> &broadcast::Sender<FlowEvent> {
        &self.events
    }

    /// Die passenden Flows als Zeilen.
    ///
    /// Sortiert: erst die gehaltenen Flows nach Frist aufsteigend (was zuerst
    /// abläuft, steht oben — Akzeptanzkriterium von HUM-016), danach alle
    /// übrigen nach Ankunft aufsteigend. Bei gleichem Schlüssel entscheidet die
    /// Id, damit die Reihenfolge über Aufrufe hinweg stabil bleibt.
    #[must_use]
    pub fn list(&self, filter: &FlowFilter) -> Vec<FlowSummary> {
        let mut rows: Vec<FlowSummary> = self
            .flows
            .iter()
            .filter(|entry| filter.matches(entry.value()))
            .map(|entry| entry.value().summary())
            .collect();
        rows.sort_by_key(order_key);
        if let Some(limit) = filter.limit {
            rows.truncate(limit);
        }
        rows
    }

    /// Wie viele Flows die Registry kennt.
    #[must_use]
    pub fn len(&self) -> usize {
        self.flows.len()
    }

    /// Wahr, wenn die Registry leer ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
    }

    /// Vergisst einen Flow.
    ///
    /// Abgeschlossene Flows räumt die Registry selbst auf (Modulkommentar,
    /// „Wie viel sie behält");
    /// das hier ist der Weg für einen Aufrufer, der einen Flow sofort los
    /// sein will.
    pub fn forget(&self, id: FlowId) {
        self.flows.remove(&id);
    }

    /// Wie viele abgeschlossene Flows die Registry höchstens behält
    /// (`limits.registry_max_finished_flows`, mindestens 1).
    #[must_use]
    pub const fn max_finished(&self) -> usize {
        self.max_finished
    }

    /// Reiht den gerade abgeschlossenen Flow `id` ein und verdrängt die
    /// ältesten abgeschlossenen über der Grenze (HUM-556).
    ///
    /// Gelöscht wird nur ein Datensatz, der wirklich [`FlowState::Recorded`]
    /// steht: Ein gehaltener oder laufender Flow fällt hier nie heraus, auch
    /// wenn seine Id durch ein erneutes [`Self::insert`] noch in der Reihe
    /// steht. Die Reihe selbst ist nie länger als die Grenze; eine Id, die
    /// [`Self::forget`] schon entfernt hat, fällt aus ihr, ohne etwas zu
    /// löschen. Darf nur ohne Guard auf `flows` laufen.
    fn retire(&self, id: FlowId) {
        let evicted: Vec<FlowId> = {
            let mut finished = self
                .finished
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            finished.push_back(id);
            let excess = finished.len().saturating_sub(self.max_finished);
            finished.drain(..excess).collect()
        };
        for old in evicted {
            self.flows
                .remove_if(&old, |_, record| record.state.is_terminal());
        }
    }
}

/// Der Sortierschlüssel von [`FlowRegistry::list`].
fn order_key(row: &FlowSummary) -> (bool, Option<Instant>, SystemTime, FlowId) {
    (row.deadline.is_none(), row.deadline, row.created, row.id)
}

/// Die Eingabe, aus der dieses Ereignis entstanden ist.
///
/// `None` für die Ereignisse, die kein Übergang sind: [`FlowEvent::Received`]
/// (der Flow beginnt damit), [`FlowEvent::ResponseChunk`],
/// [`FlowEvent::Diagnostic`], [`FlowEvent::Lagged`] und
/// [`FlowEvent::AgentAsk`], das zu gar keinem Flow gehört (HUM-073), ebenso
/// wenig wie [`FlowEvent::Intercept`] (HUM-407) und
/// [`FlowEvent::Sandbox`] (HUM-477).
fn transition_input(event: &FlowEvent) -> Option<TransitionInput> {
    match event {
        FlowEvent::Analyzed { findings, .. } => Some(TransitionInput::Analyze {
            findings: findings.clone(),
        }),
        FlowEvent::Held {
            deadline,
            queue_bytes,
            queue_count,
            ..
        } => Some(TransitionInput::Hold {
            deadline: *deadline,
            queue_bytes: *queue_bytes,
            queue_count: *queue_count,
        }),
        FlowEvent::Decided {
            decision, source, ..
        } => Some(TransitionInput::Decide {
            decision: decision.clone(),
            source: *source,
        }),
        FlowEvent::Forwarded { .. } => Some(TransitionInput::Forward),
        FlowEvent::ResponseHeaders { status, .. } => {
            Some(TransitionInput::Respond { status: *status })
        }
        FlowEvent::Failed { error, .. } => Some(TransitionInput::Fail { error: *error }),
        FlowEvent::TimedOut { .. } => Some(TransitionInput::Timeout),
        FlowEvent::Recorded { .. } => Some(TransitionInput::Record),
        FlowEvent::Received { .. }
        | FlowEvent::ResponseChunk { .. }
        | FlowEvent::Diagnostic { .. }
        | FlowEvent::Lagged { .. }
        | FlowEvent::AgentAsk { .. }
        | FlowEvent::Intercept { .. }
        | FlowEvent::Sandbox { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, Instant, SystemTime};

    use humanitl_config::Limits;
    use humanitl_core::{
        Authority, BodyRef, Decision, DecisionSource, Flow, FlowEvent, FlowId, FlowState, HostName,
        HttpRequest, Method, Scheme, SessionId, TransitionInput,
    };

    use super::{FlowFilter, FlowRecord, FlowRegistry, MAX_FLOW_DIAGNOSTICS, UNREGISTERED};
    use crate::connect::ConnectionContext;

    fn flow(session: SessionId, host: &str) -> Flow {
        let host = HostName::Dns(host.to_owned());
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(host, Scheme::Https),
            "/",
        )
        .with_body(BodyRef::detached([0; 32], 7));
        Flow::new(FlowId::new(), session, SystemTime::now(), request)
    }

    fn analyzed(session: SessionId, host: &str) -> Flow {
        let mut flow = flow(session, host);
        flow.apply(
            TransitionInput::Analyze { findings: vec![] },
            SystemTime::now(),
        )
        .unwrap();
        flow
    }

    /// Die Eingaben, mit denen ein analysierter Flow bis `Recorded` läuft.
    fn to_the_end() -> [TransitionInput; 4] {
        [
            TransitionInput::Decide {
                decision: Decision::Allow,
                source: DecisionSource::Passthrough,
            },
            TransitionInput::Forward,
            TransitionInput::Respond { status: 200 },
            TransitionInput::Record,
        ]
    }

    /// Ein Flow, der bis `Recorded` durchläuft, so wie der Handler ihn über
    /// [`FlowRegistry::record`] mitschreibt.
    fn finish_through_events(registry: &FlowRegistry, session: SessionId) -> FlowId {
        let mut flow = analyzed(session, "example.com");
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        for input in to_the_end() {
            let event = flow.apply(input, SystemTime::now()).unwrap();
            registry.record(&event);
        }
        flow.id
    }

    fn limits_keeping(finished: u32) -> Limits {
        Limits {
            registry_max_finished_flows: finished,
            ..Limits::default()
        }
    }

    /// HUM-556: Über der Grenze fallen die ältesten abgeschlossenen Flows
    /// heraus, ein gehaltener und ein laufender bleiben, auch wenn sie älter
    /// sind als alle abgeschlossenen.
    #[test]
    fn finished_flows_above_the_cap_are_evicted_and_a_held_flow_stays() {
        const CAP: u32 = 4;
        let registry = FlowRegistry::new(&limits_keeping(CAP));
        let session = SessionId::new();

        let mut held = analyzed(session, "held.example.com");
        registry.insert(FlowRecord::new(&held, &ConnectionContext::plain(session)));
        let event = held
            .apply(
                TransitionInput::Hold {
                    deadline: Instant::now() + Duration::from_secs(30),
                    queue_bytes: 7,
                    queue_count: 1,
                },
                SystemTime::now(),
            )
            .unwrap();
        registry.record(&event);
        let mut running = analyzed(session, "running.example.com");
        registry.insert(FlowRecord::new(
            &running,
            &ConnectionContext::plain(session),
        ));
        for input in to_the_end().into_iter().take(2) {
            registry.record(&running.apply(input, SystemTime::now()).unwrap());
        }

        let finished: Vec<FlowId> = (0..100)
            .map(|_| finish_through_events(&registry, session))
            .collect();

        assert_eq!(registry.len(), CAP as usize + 2);
        assert!(matches!(
            registry.get(held.id).unwrap().state,
            FlowState::Held { .. }
        ));
        assert_eq!(
            registry.get(running.id).unwrap().state,
            FlowState::Forwarded
        );
        let (gone, kept) = finished.split_at(finished.len() - CAP as usize);
        assert!(gone.iter().all(|id| registry.get(*id).is_none()));
        assert!(kept.iter().all(|id| registry.get(*id).is_some()));
        assert_eq!(registry.list(&FlowFilter::held()).len(), 1);
        // Auch die Reihe der abgeschlossenen wächst nicht mit.
        assert_eq!(registry.finished.lock().unwrap().len(), CAP as usize);
    }

    /// Derselbe Abschluss über den Weg der gRPC-Schicht
    /// ([`FlowRegistry::transition`]) und über einen schon abgeschlossen
    /// eingefügten Datensatz zählt gegen dieselbe Grenze.
    #[test]
    fn transition_and_insert_count_against_the_same_cap() {
        let registry = FlowRegistry::new(&limits_keeping(2));
        let session = SessionId::new();
        let mut ids = Vec::new();
        for _ in 0..3 {
            let flow = analyzed(session, "example.com");
            registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
            for input in to_the_end() {
                registry.transition(flow.id, input).unwrap();
            }
            ids.push(flow.id);
        }
        assert_eq!(registry.len(), 2);
        assert!(registry.get(ids[0]).is_none());

        let mut done = analyzed(session, "example.com");
        for input in to_the_end() {
            done.apply(input, SystemTime::now()).unwrap();
        }
        registry.insert(FlowRecord::new(&done, &ConnectionContext::plain(session)));
        assert_eq!(registry.len(), 2);
        assert!(registry.get(ids[1]).is_none());
        assert!(registry.get(done.id).is_some());
    }

    /// Eine Grenze von `0` hieße, den gerade abgeschlossenen Flow sofort zu
    /// vergessen; die Registry behält mindestens einen.
    #[test]
    fn a_cap_of_zero_keeps_the_last_finished_flow() {
        let registry = FlowRegistry::new(&limits_keeping(0));
        assert_eq!(registry.max_finished(), 1);
        let session = SessionId::new();
        finish_through_events(&registry, session);
        let last = finish_through_events(&registry, session);
        assert_eq!(registry.len(), 1);
        assert!(registry.get(last).is_some());
    }

    /// Eine Id, die [`FlowRegistry::forget`] schon entfernt hat, verdrängt
    /// beim Herausfallen keinen anderen Flow.
    #[test]
    fn a_forgotten_flow_leaves_the_queue_without_evicting_another() {
        let registry = FlowRegistry::new(&limits_keeping(2));
        let session = SessionId::new();
        let first = finish_through_events(&registry, session);
        let second = finish_through_events(&registry, session);
        registry.forget(first);
        let third = finish_through_events(&registry, session);
        assert_eq!(registry.len(), 2);
        assert!(registry.get(second).is_some());
        assert!(registry.get(third).is_some());
    }

    #[test]
    fn transition_drives_the_record_and_publishes() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let flow = flow(session, "example.com");
        let id = flow.id;
        let mut rx = registry.subscribe();
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));

        let event = registry
            .transition(id, TransitionInput::Analyze { findings: vec![] })
            .unwrap();
        assert_eq!(event.name(), "analyzed");
        assert_eq!(rx.try_recv().unwrap().name(), "analyzed");
        assert!(matches!(
            registry.get(id).unwrap().state,
            FlowState::Analyzed { .. }
        ));
    }

    /// Ein Befund am Fluss landet im Datensatz, in Reihenfolge und höchstens
    /// [`MAX_FLOW_DIAGNOSTICS`] (HUM-057); ein Befund ohne Fluss nirgends.
    #[test]
    fn a_record_keeps_the_diagnostics_of_its_flow_up_to_the_cap() {
        use humanitl_core::diagnostics::codes::{LIMIT_001, LIMIT_004};
        use humanitl_core::{Diagnostic, Severity};

        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let flow = analyzed(session, "example.com");
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        let event = |flow_id, code| FlowEvent::Diagnostic {
            flow_id,
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(code, Severity::Warning)
                    .why("a finding".to_owned())
                    .build(),
            ),
        };
        registry.record(&event(Some(flow.id), LIMIT_001));
        registry.record(&event(None, LIMIT_004));
        registry.record(&event(Some(flow.id), LIMIT_004));
        let codes: Vec<_> = registry
            .get(flow.id)
            .unwrap()
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(codes, [LIMIT_001, LIMIT_004]);

        for _ in 0..2 * MAX_FLOW_DIAGNOSTICS {
            registry.record(&event(Some(flow.id), LIMIT_004));
        }
        assert_eq!(
            registry.get(flow.id).unwrap().diagnostics.len(),
            MAX_FLOW_DIAGNOSTICS
        );
        assert_eq!(
            registry.get(flow.id).unwrap().diagnostics[0].code,
            LIMIT_001
        );
    }

    #[test]
    fn transition_of_an_unknown_flow_is_invalid() {
        let registry = FlowRegistry::new(&Limits::default());
        let err = registry
            .transition(FlowId::new(), TransitionInput::Forward)
            .unwrap_err();
        assert_eq!(err.from, UNREGISTERED);
        assert_eq!(err.input, "forward");
    }

    #[test]
    fn record_follows_an_event_that_someone_else_applied() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let mut flow = analyzed(session, "example.com");
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        let deadline = Instant::now() + Duration::from_secs(30);
        let held = flow
            .apply(
                TransitionInput::Hold {
                    deadline,
                    queue_bytes: 7,
                    queue_count: 1,
                },
                SystemTime::now(),
            )
            .unwrap();
        registry.record(&held);
        assert_eq!(registry.get(flow.id).unwrap().deadline, Some(deadline));

        let decided = flow
            .apply(
                TransitionInput::Decide {
                    decision: Decision::Allow,
                    source: DecisionSource::User,
                },
                SystemTime::now(),
            )
            .unwrap();
        registry.record(&decided);
        let record = registry.get(flow.id).unwrap();
        assert_eq!(record.decision, Some(Decision::Allow));
        assert_eq!(record.deadline, None, "a decided flow has no deadline");
    }

    #[test]
    fn record_keeps_the_source_the_size_and_the_end_of_a_flow() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let mut flow = analyzed(session, "example.com");
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));

        // Vor der Entscheidung weiß der Datensatz nichts von alledem.
        let fresh = registry.get(flow.id).unwrap();
        assert_eq!(fresh.decision_source, None);
        assert_eq!(fresh.response_bytes, 0);
        assert_eq!(fresh.finished, None);

        for input in [
            TransitionInput::Hold {
                deadline: Instant::now() + Duration::from_secs(30),
                queue_bytes: 7,
                queue_count: 1,
            },
            TransitionInput::Decide {
                decision: Decision::Allow,
                source: DecisionSource::User,
            },
            TransitionInput::Forward,
            TransitionInput::Respond { status: 200 },
        ] {
            let event = flow.apply(input, SystemTime::now()).unwrap();
            registry.record(&event);
        }
        // Zwei Stücke Antwortkörper; kein Übergang, aber die Größe der Antwort.
        for len in [17_u64, 25] {
            registry.record(&FlowEvent::ResponseChunk {
                flow_id: flow.id,
                at: SystemTime::now(),
                len,
            });
        }
        let streaming = registry.get(flow.id).unwrap();
        assert_eq!(streaming.decision_source, Some(DecisionSource::User));
        assert_eq!(streaming.response_bytes, 42);
        assert_eq!(
            streaming.finished, None,
            "solange die Antwort läuft, hat der Flow kein Ende"
        );

        let end = SystemTime::now();
        let recorded = flow.apply(TransitionInput::Record, end).unwrap();
        registry.record(&recorded);
        let done = registry.get(flow.id).unwrap();
        assert_eq!(done.state, FlowState::Recorded);
        assert_eq!(done.finished, Some(end));
        assert_eq!(done.response_bytes, 42);
    }

    #[test]
    fn a_timeout_names_its_own_source() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let mut flow = analyzed(session, "example.com");
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        let held = flow
            .apply(
                TransitionInput::Hold {
                    deadline: Instant::now() + Duration::from_secs(30),
                    queue_bytes: 7,
                    queue_count: 1,
                },
                SystemTime::now(),
            )
            .unwrap();
        registry.record(&held);
        let timed_out = flow
            .apply(TransitionInput::Timeout, SystemTime::now())
            .unwrap();
        registry.record(&timed_out);

        let record = registry.get(flow.id).unwrap();
        assert_eq!(record.decision, Some(Decision::TimedOut));
        assert_eq!(record.decision_source, Some(DecisionSource::Timeout));
    }

    #[test]
    fn a_chunk_for_an_unknown_flow_changes_nothing() {
        let registry = FlowRegistry::new(&Limits::default());
        registry.record(&FlowEvent::ResponseChunk {
            flow_id: FlowId::new(),
            at: SystemTime::now(),
            len: 9,
        });
        assert!(registry.is_empty());
    }

    #[test]
    fn record_ignores_what_it_cannot_follow() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let flow = flow(session, "example.com");
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        registry.record(&FlowEvent::Forwarded {
            flow_id: flow.id,
            at: SystemTime::now(),
        });
        assert_eq!(registry.get(flow.id).unwrap().state, FlowState::Received);
        registry.record(&FlowEvent::Lagged { n: 3 });
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn list_sorts_held_flows_by_deadline() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let now = Instant::now();
        let mut ids = Vec::new();
        for secs in [3_u64, 1, 2] {
            let mut flow = analyzed(session, "example.com");
            registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
            let held = flow
                .apply(
                    TransitionInput::Hold {
                        deadline: now + Duration::from_secs(secs),
                        queue_bytes: 0,
                        queue_count: 1,
                    },
                    SystemTime::now(),
                )
                .unwrap();
            registry.record(&held);
            ids.push((secs, flow.id));
        }
        // Ein entschiedener Flow steht hinter jedem gehaltenen.
        let mut decided = analyzed(session, "example.com");
        registry.insert(FlowRecord::new(
            &decided,
            &ConnectionContext::plain(session),
        ));
        let event = decided
            .apply(
                TransitionInput::Decide {
                    decision: Decision::Allow,
                    source: DecisionSource::Passthrough,
                },
                SystemTime::now(),
            )
            .unwrap();
        registry.record(&event);

        let rows = registry.list(&FlowFilter::default());
        let order: Vec<FlowId> = rows.iter().map(|row| row.id).collect();
        ids.sort_unstable_by_key(|(secs, _)| *secs);
        let expected: Vec<FlowId> = ids
            .iter()
            .map(|(_, id)| *id)
            .chain(std::iter::once(decided.id))
            .collect();
        assert_eq!(order, expected);

        let held = registry.list(&FlowFilter::held());
        assert_eq!(held.len(), 3);
        assert!(held.iter().all(|row| row.deadline.is_some()));
    }

    #[test]
    fn filters_narrow_the_list() {
        let registry = FlowRegistry::new(&Limits::default());
        let mine = SessionId::new();
        let other = SessionId::new();
        let a = flow(mine, "api.example.com");
        let b = flow(mine, "cdn.example.com");
        let c = flow(other, "api.example.com");
        for (flow, session) in [(&a, mine), (&b, mine), (&c, other)] {
            registry.insert(FlowRecord::new(flow, &ConnectionContext::plain(session)));
        }

        assert_eq!(registry.list(&FlowFilter::session(mine)).len(), 2);
        let by_host = FlowFilter {
            host: Some("api.example.com".to_owned()),
            ..FlowFilter::default()
        };
        assert_eq!(registry.list(&by_host).len(), 2);
        let by_state = FlowFilter {
            state: Some("received".to_owned()),
            limit: Some(1),
            ..FlowFilter::default()
        };
        assert_eq!(registry.list(&by_state).len(), 1);
        let nothing = FlowFilter {
            state: Some("recorded".to_owned()),
            ..FlowFilter::default()
        };
        assert!(registry.list(&nothing).is_empty());
    }
}
