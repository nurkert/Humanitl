//! Die Halte-Warteschlange: ein gehaltener Request ist ein Future, das auf eine
//! Entscheidung wartet (ADR-004, HUM-016).
//!
//! Die Warteschlange tut drei Dinge und sonst nichts:
//!
//! 1. Sie hält einen Flow, bis jemand [`HoldQueue::decide`] ruft oder die
//!    Frist abläuft. Ein Ablauf ist immer [`Decision::TimedOut`], nie ein
//!    stilles Durchlassen.
//! 2. Sie hält ein Budget ein. `limits.hold_max_flows` und
//!    `limits.hold_max_bytes` sind atomare Zähler; ein Flow, der nicht mehr
//!    hineinpasst, wird sofort mit [`BlockReason::HoldMaxFlows`] oder
//!    [`BlockReason::HoldMemory`] abgelehnt (`503`). Ein bereits gehaltener
//!    Flow wird dafür nie verdrängt.
//! 3. Sie treibt den Zustandsautomaten aus `humanitl-core`
//!    ([`Flow::apply`] mit `Hold`, `Decide`, `Timeout`) und gibt jedes
//!    Ereignis in einen `broadcast`-Kanal der Kapazität
//!    `limits.event_buffer`.
//!
//! Der Proxy-Handler (HUM-015) besitzt den Flow und leiht ihn der
//! Warteschlange nur für die Dauer des Haltens; die Warteschlange erfindet
//! weder Ids noch Fristen. Die Ereignisse `Received` und `Analyzed` gehören
//! dem Handler, er gibt sie über [`HoldQueue::publish`] in denselben Kanal,
//! damit die Reihenfolge je Flow stimmt.
//!
//! Kanal und Buchführung teilt sich die Warteschlange mit der
//! [`FlowRegistry`]: [`HoldQueue::new`] legt eine an, [`HoldQueue::with_registry`]
//! nimmt eine vorhandene. Jedes Ereignis läuft durch [`HoldQueue::publish`] und
//! damit genau einmal an der Registry vorbei, bevor es in den Strom geht.
//!
//! # Zuhörer und `Lagged`
//!
//! Der Kanal ist ein `tokio::sync::broadcast`. Wer nicht schnell genug liest,
//! verliert die ältesten Ereignisse; `recv()` liefert dann einmalig
//! `RecvError::Lagged(n)`. Ein Zuhörer behandelt das so: `n` als
//! [`FlowEvent::Lagged`] weiterreichen (die gRPC-Schicht, HUM-018, tut genau
//! das; [`next_event`] nimmt einem die Umwandlung ab), danach den eigenen
//! Stand mit `ListFlows` nachladen, weil dazwischen Zustandswechsel fehlen.
//! `RecvError::Closed` heißt: die Warteschlange ist weg, der Strom endet.
//!
//! # Was hier nicht ist
//!
//! Keine Regelauswertung (HUM-022), keine Findings (HUM-025), keine
//! Persistenz (HUM-026). Die Liste aller Flows einer Sitzung steht nebenan in
//! der [`FlowRegistry`]. Nach einem Neustart sind beide leer.

use core::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use humanitl_config::Limits;
use humanitl_core::{
    BlockReason, Decision, DecisionSource, Flow, FlowEvent, FlowId, FlowState, InvalidTransition,
    Transition, TransitionInput,
};
use tokio::sync::{broadcast, oneshot};

use crate::registry::FlowRegistry;

/// Mehr Ereignisse als das puffert der Kanal je Zuhörer nicht (65 536).
///
/// Die Obergrenze ist ein Schutz, kein Stellhebel: `tokio` rundet die
/// Kapazität auf eine Zweierpotenz auf und legt den Ring sofort an; ein
/// Zuhörer, der so weit zurückliegt, lädt ohnehin nach (`Lagged`).
pub const MAX_EVENT_BUFFER: usize = 1 << 16;

/// Warum eine Entscheidung nicht angenommen wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NotHeld {
    /// Der Flow wird nicht gehalten: unbekannt, schon entschieden oder schon
    /// abgelaufen. Die gRPC-Schicht meldet das als `IPC_003`.
    #[error("flow {id} is not held (unknown, already decided, or timed out)")]
    Unknown {
        /// Der Flow, um den es ging.
        id: FlowId,
    },
    /// Diese Herkunft darf so nicht entscheiden. Ein `TimedOut` kommt nur aus
    /// der Warteschlange selbst, und `System` darf nur ablehnen, nie
    /// durchlassen (`backlog/CONVENTIONS.md` 4.11).
    #[error("flow {id}: {} may not decide {decision} on a held flow", .by.as_str())]
    Forbidden {
        /// Der Flow, um den es ging.
        id: FlowId,
        /// Die abgelehnte Entscheidung, siehe [`Decision::as_str`].
        decision: &'static str,
        /// Wer sie treffen wollte.
        by: DecisionSource,
    },
}

impl NotHeld {
    /// Der Flow, um den es ging.
    #[must_use]
    pub const fn id(&self) -> FlowId {
        match self {
            Self::Unknown { id } | Self::Forbidden { id, .. } => *id,
        }
    }
}

/// Warum ein Flow gar nicht erst gehalten werden konnte.
///
/// Beides sind Fehler im Aufrufer, keine Laufzeitzustände: das Budget wird
/// nicht hier gemeldet, sondern als [`Decision::Block`] aus dem Future.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HoldError {
    /// Für diese Id wartet schon ein Future.
    #[error("flow {id} is already held")]
    AlreadyHeld {
        /// Die doppelt verwendete Id.
        id: FlowId,
    },
    /// Der Flow ist nicht in [`FlowState::Analyzed`]; nur von dort führt ein
    /// Übergang nach `Held` (oder, bei erschöpftem Budget, nach `Decided`).
    #[error("flow cannot be held: {0}")]
    InvalidTransition(#[from] InvalidTransition),
}

/// Ein wartender Flow: der Kanal zur Proxy-Task und die aktuelle Frist.
struct Pending {
    /// Trägt Entscheidung und Herkunft zur wartenden Task.
    tx: oneshot::Sender<(Decision, DecisionSource)>,
    /// Bis wann gewartet wird; [`HoldQueue::extend`] schiebt sie.
    deadline: Instant,
}

/// Die Halte-Warteschlange, siehe Modulkommentar.
///
/// Eine je Daemon, geteilt zwischen Proxy-Handlern (halten) und
/// gRPC-Handlern (entscheiden). `Send + Sync`, üblicherweise in einem `Arc`.
pub struct HoldQueue {
    pending: DashMap<FlowId, Pending>,
    held_flows: AtomicU32,
    held_bytes: AtomicU64,
    max_flows: u32,
    max_bytes: u64,
    registry: Arc<FlowRegistry>,
    events: broadcast::Sender<FlowEvent>,
}

impl fmt::Debug for HoldQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HoldQueue")
            .field("pending", &self.pending.len())
            .field("held_flows", &self.queue_count())
            .field("held_bytes", &self.queue_bytes())
            .field("max_flows", &self.max_flows)
            .field("max_bytes", &self.max_bytes)
            .finish_non_exhaustive()
    }
}

impl HoldQueue {
    /// Eine leere Warteschlange mit dem Budget und der Kanal-Kapazität aus
    /// `limits` (`hold_max_flows`, `hold_max_bytes`, `event_buffer`), mit einer
    /// frisch angelegten [`FlowRegistry`].
    ///
    /// `event_buffer` wird auf `1..=`[`MAX_EVENT_BUFFER`] begrenzt, damit ein
    /// ungeprüfter Wert keinen Panic im Kanal auslöst; die Konfiguration
    /// verlangt ohnehin mindestens 1. `tokio` rundet die Kapazität auf die
    /// nächste Zweierpotenz auf.
    #[must_use]
    pub fn new(limits: &Limits) -> Self {
        Self::with_registry(limits, Arc::new(FlowRegistry::new(limits)))
    }

    /// Wie [`HoldQueue::new`], aber mit einer vorhandenen Registry.
    ///
    /// Der Ereignisstrom kommt dann von ihr: beide schreiben in denselben
    /// `broadcast`-Kanal, und jeder Zuhörer sieht die Ereignisse der
    /// Warteschlange und der Registry in einer Reihenfolge. `limits` liefert
    /// nur noch das Halte-Budget.
    #[must_use]
    pub fn with_registry(limits: &Limits, registry: Arc<FlowRegistry>) -> Self {
        let events = registry.events().clone();
        Self {
            pending: DashMap::new(),
            held_flows: AtomicU32::new(0),
            held_bytes: AtomicU64::new(0),
            max_flows: limits.hold_max_flows,
            max_bytes: limits.hold_max_bytes,
            registry,
            events,
        }
    }

    /// Das Verzeichnis der Flows, mit dem sich die Warteschlange den
    /// Ereignisstrom teilt.
    #[must_use]
    pub const fn registry(&self) -> &Arc<FlowRegistry> {
        &self.registry
    }

    /// Hält `flow`, bis jemand entscheidet oder `deadline` verstreicht.
    ///
    /// Alles, was sofort geschehen kann, geschieht hier, nicht erst beim
    /// ersten `poll`: das Budget wird reserviert, der Flow wechselt nach
    /// [`FlowState::Held`], das `Held`-Ereignis geht hinaus, und
    /// [`HoldQueue::decide`] kennt die Id, sobald diese Funktion zurückkehrt.
    /// Das zurückgegebene Future liefert die Entscheidung; danach ist der Flow
    /// in [`FlowState::Decided`] und das passende Ereignis (`Decided` oder
    /// `TimedOut`) ist veröffentlicht. Der Aufrufer antwortet dem Client und
    /// verbucht `Forward`/`Respond`/`Record` selbst.
    ///
    /// Passt der Flow nicht mehr ins Budget, wird er nicht gehalten: er
    /// wechselt sofort nach `Decided(Block { HoldMaxFlows | HoldMemory })`
    /// (Herkunft `System`), das `Decided`-Ereignis geht hinaus, und das Future
    /// liefert diese Entscheidung beim ersten `poll`. Das Budget zählt
    /// `flow.request.body.size`, also den Body, der für die Entscheidung im
    /// Speicher liegt.
    ///
    /// Eine Frist in der Vergangenheit läuft sofort ab; `hold.timeout_secs =
    /// 0` beziehungsweise `ask_mode = none` blockt damit alles.
    ///
    /// Wird das Future fallen gelassen, bevor es fertig ist (der Client hat
    /// die Verbindung aufgegeben, hudsucker bricht die Task ab), endet der Flow
    /// mit `Block { ClientTimeout }` durch `System`, das Ereignis geht hinaus,
    /// und das Budget ist wieder frei.
    ///
    /// # Errors
    ///
    /// [`HoldError::InvalidTransition`], wenn `flow` nicht in
    /// [`FlowState::Analyzed`] ist; [`HoldError::AlreadyHeld`], wenn für
    /// diese Id schon ein Future wartet. In beiden Fällen bleibt der Flow
    /// unverändert und nichts wird veröffentlicht.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::{Duration, Instant, SystemTime};
    ///
    /// use humanitl_config::Limits;
    /// use humanitl_core::{
    ///     Authority, Decision, Flow, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
    ///     TransitionInput,
    /// };
    /// use humanitl_proxy::hold::HoldQueue;
    ///
    /// # tokio::runtime::Runtime::new()?.block_on(async {
    /// let queue = HoldQueue::new(&Limits::default());
    /// let host = HostName::Dns("example.com".to_owned());
    /// let request = HttpRequest::new(
    ///     Method::GET,
    ///     Scheme::Https,
    ///     Authority::with_scheme(host, Scheme::Https),
    ///     "/",
    /// );
    /// let mut flow = Flow::new(FlowId::new(), SessionId::new(), SystemTime::now(), request);
    /// queue.publish(flow.received_event());
    /// let analyzed = flow.apply(TransitionInput::Analyze { findings: vec![] }, SystemTime::now())?;
    /// queue.publish(analyzed);
    ///
    /// let id = flow.id;
    /// let held = queue.hold(&mut flow, Instant::now() + Duration::from_secs(300))?;
    /// queue.decide(id, Decision::Allow)?; // sonst aus dem gRPC-Handler
    /// assert_eq!(held.await, Decision::Allow);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// # })?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn hold<'a>(
        &'a self,
        flow: &'a mut Flow,
        deadline: Instant,
    ) -> Result<impl Future<Output = Decision> + Send + 'a, HoldError> {
        let admission = self.admit(flow, deadline)?;
        Ok(async move {
            match admission {
                Admission::Refused(decision) => decision,
                Admission::Held { ticket, rx } => ticket.wait(rx).await,
            }
        })
    }

    /// Entscheidet über einen gehaltenen Flow im Namen des Menschen
    /// (Oberfläche oder Terminal, [`DecisionSource::User`]).
    ///
    /// Entfernt den Eintrag und weckt das wartende Future; das
    /// `Decided`-Ereignis entsteht dort, sobald es läuft.
    ///
    /// # Errors
    ///
    /// [`NotHeld::Unknown`], wenn der Flow nicht (mehr) gehalten wird: die
    /// Id ist unbekannt, schon entschieden oder schon abgelaufen. Ein zweiter
    /// Aufruf für dieselbe Id scheitert also. [`NotHeld::Forbidden`] für
    /// [`Decision::TimedOut`]: ein Ablauf kommt nur aus der Warteschlange.
    pub fn decide(&self, id: FlowId, decision: Decision) -> Result<(), NotHeld> {
        self.decide_as(id, decision, DecisionSource::User)
    }

    /// Wahr, solange `deadline` noch in der Zukunft liegt.
    ///
    /// Gemessen an `tokio::time::Instant::now()`, also an derselben Uhr, an der
    /// die Halte-Task ihr `timeout_at` hängt. Sie steht in Tests still
    /// (`start_paused`), und nur so ist „abgelaufen" für beide dasselbe.
    fn still_open(deadline: Instant) -> bool {
        tokio::time::Instant::from_std(deadline) > tokio::time::Instant::now()
    }

    /// Wie [`HoldQueue::decide`], mit ausdrücklicher Herkunft: eine Regel
    /// (`Rule`), die während des Haltens entstand, oder der Daemon selbst
    /// (`System`), der einen gehaltenen Flow abbrechen muss.
    ///
    /// # Errors
    ///
    /// Wie [`HoldQueue::decide`]; zusätzlich [`NotHeld::Forbidden`], wenn
    /// der Automat diese Herkunft mit dieser Entscheidung aus `Held` nicht
    /// zulässt, etwa `System` mit `Allow`.
    pub fn decide_as(
        &self,
        id: FlowId,
        decision: Decision,
        by: DecisionSource,
    ) -> Result<(), NotHeld> {
        if !decidable(&decision, by) {
            return Err(NotHeld::Forbidden {
                id,
                decision: decision.as_str(),
                by,
            });
        }
        // Erst entfernen, dann senden: kein Guard über den Kanal hinweg, und
        // wer den Eintrag hat, hat die Entscheidung. Entfernt wird nur, solange
        // die Frist noch läuft: zwischen dem Ablauf und dem Aufräumen durch die
        // Halte-Task liegt ein Augenblick, und in dem darf keine verspätete
        // Entscheidung den Ablauf überholen. Ein Ablauf blockt, immer.
        let (_, pending) = self
            .pending
            .remove_if(&id, |_, pending| Self::still_open(pending.deadline))
            .ok_or(NotHeld::Unknown { id })?;
        pending
            .tx
            .send((decision, by))
            .map_err(|_gone| NotHeld::Unknown { id })
    }

    /// Schiebt die Frist um `by` nach hinten und liefert die neue Frist.
    ///
    /// „Timer pausieren" in der Oberfläche ist ein `extend` um 24 Stunden
    /// (HUM-050 trägt es ins Audit ein). Nur möglich, solange der Flow
    /// gehalten wird. Eine Frist, die sich nicht mehr darstellen lässt,
    /// bleibt stehen.
    ///
    /// # Errors
    ///
    /// [`NotHeld::Unknown`], wenn der Flow nicht (mehr) gehalten wird. Dazu
    /// zählt eine Frist, die schon abgelaufen ist: sie lässt sich nicht mehr
    /// verlängern, auch wenn die Halte-Task den Eintrag noch nicht abgeräumt
    /// hat.
    pub fn extend(&self, id: FlowId, by: Duration) -> Result<Instant, NotHeld> {
        let mut pending = self.pending.get_mut(&id).ok_or(NotHeld::Unknown { id })?;
        if !Self::still_open(pending.deadline) {
            return Err(NotHeld::Unknown { id });
        }
        pending.deadline = pending.deadline.checked_add(by).unwrap_or(pending.deadline);
        Ok(pending.deadline)
    }

    /// Die Frist eines gehaltenen Flows, `None` wenn er nicht gehalten wird.
    #[must_use]
    pub fn deadline(&self, id: FlowId) -> Option<Instant> {
        self.pending.get(&id).map(|pending| pending.deadline)
    }

    /// Die gehaltenen Flows, nach Frist aufsteigend (bei gleicher Frist nach
    /// Id, also nach Ankunft).
    #[must_use]
    pub fn pending_ids(&self) -> Vec<FlowId> {
        let mut entries: Vec<(Instant, FlowId)> = self
            .pending
            .iter()
            .map(|entry| (entry.value().deadline, *entry.key()))
            .collect();
        entries.sort_unstable();
        entries.into_iter().map(|(_, id)| id).collect()
    }

    /// Wie viele Flows gerade gehalten werden.
    #[must_use]
    pub fn queue_count(&self) -> u32 {
        self.held_flows.load(Ordering::Acquire)
    }

    /// Wie viele Body-Bytes gerade insgesamt gehalten werden.
    #[must_use]
    pub fn queue_bytes(&self) -> u64 {
        self.held_bytes.load(Ordering::Acquire)
    }

    /// Ein neuer Zuhörer am Ereignisstrom. Er sieht nur, was ab jetzt
    /// geschieht; zum Umgang mit `Lagged` siehe Modulkommentar.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<FlowEvent> {
        self.events.subscribe()
    }

    /// Der Sender des Ereignisstroms; derselbe, den auch die
    /// [`FlowRegistry`] benutzt.
    #[must_use]
    pub const fn events(&self) -> &broadcast::Sender<FlowEvent> {
        &self.events
    }

    /// Gibt ein Ereignis in den Strom, das nicht aus der Warteschlange
    /// stammt: `Received`, `Analyzed`, `Forwarded`, `ResponseHeaders`,
    /// `ResponseChunk`, `Failed`, `Recorded` gehören dem Proxy-Handler.
    ///
    /// Ohne Zuhörer geht das Ereignis verloren; das ist kein Fehler, solange
    /// niemand zuhört, gibt es nichts nachzuladen. Die [`FlowRegistry`]
    /// schreibt ihren Datensatz vorher fort, damit ein Zuhörer, der auf das
    /// Ereignis hin `ListFlows` ruft, den neuen Zustand schon vorfindet.
    pub fn publish(&self, event: FlowEvent) {
        self.registry.record(&event);
        let _ = self.events.send(event);
    }

    /// Der synchrone Teil von [`HoldQueue::hold`], siehe dort.
    fn admit<'a>(
        &'a self,
        flow: &'a mut Flow,
        deadline: Instant,
    ) -> Result<Admission<'a>, HoldError> {
        let id = flow.id;
        let now = SystemTime::now();
        let entry = match self.pending.entry(id) {
            Entry::Occupied(_) => return Err(HoldError::AlreadyHeld { id }),
            Entry::Vacant(entry) => entry,
        };
        match self.reserve(flow.request.body.size) {
            Err(reason) => {
                drop(entry);
                let decision = Decision::Block { reason, note: None };
                let event = flow.apply(
                    TransitionInput::Decide {
                        decision: decision.clone(),
                        source: DecisionSource::System,
                    },
                    now,
                )?;
                self.publish(event);
                Ok(Admission::Refused(decision))
            }
            Ok(reservation) => {
                // Scheitert der Übergang, fällt `reservation` hier aus dem
                // Gültigkeitsbereich und gibt das Budget zurück; `entry` hat
                // nichts eingefügt.
                let event = flow.apply(
                    TransitionInput::Hold {
                        deadline,
                        queue_bytes: reservation.total_bytes,
                        queue_count: reservation.total_flows,
                    },
                    now,
                )?;
                let (tx, rx) = oneshot::channel();
                entry.insert(Pending { tx, deadline });
                self.publish(event);
                Ok(Admission::Held {
                    ticket: Ticket {
                        queue: self,
                        flow,
                        reservation: Some(reservation),
                        settled: false,
                    },
                    rx,
                })
            }
        }
    }

    /// Reserviert einen Flow und `bytes` im Budget, oder nennt den Grund.
    ///
    /// Beide Zähler werden mit `fetch_update` erhöht, also nie über die
    /// Grenze hinaus und wieder zurück: ein Zuhörer, der gleichzeitig liest,
    /// sieht keinen Wert über dem Budget.
    fn reserve(&self, bytes: u64) -> Result<Reservation<'_>, BlockReason> {
        let max_flows = self.max_flows;
        let previous_flows = self
            .held_flows
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                held.checked_add(1).filter(|next| *next <= max_flows)
            })
            .map_err(|_full| BlockReason::HoldMaxFlows)?;
        let max_bytes = self.max_bytes;
        let previous_bytes =
            self.held_bytes
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                    held.checked_add(bytes).filter(|next| *next <= max_bytes)
                });
        match previous_bytes {
            Ok(previous_bytes) => Ok(Reservation {
                queue: self,
                bytes,
                total_bytes: previous_bytes + bytes,
                total_flows: previous_flows + 1,
            }),
            Err(_full) => {
                self.held_flows.fetch_sub(1, Ordering::AcqRel);
                Err(BlockReason::HoldMemory)
            }
        }
    }
}

/// Darf diese Herkunft so über einen gehaltenen Flow entscheiden?
///
/// `TimedOut` ist der Warteschlange vorbehalten; alles Weitere entscheidet
/// der Automat des Kerns, damit die Regel nur an einer Stelle steht.
fn decidable(decision: &Decision, by: DecisionSource) -> bool {
    if matches!(decision, Decision::TimedOut) {
        return false;
    }
    FlowState::Held {
        deadline: Instant::now(),
    }
    .on(Transition::decide(
        FlowId::nil(),
        SystemTime::UNIX_EPOCH,
        decision.clone(),
        by,
    ))
    .is_ok()
}

/// Ein reservierter Platz im Budget; gibt ihn beim Fallenlassen zurück.
struct Reservation<'a> {
    queue: &'a HoldQueue,
    bytes: u64,
    /// Gehaltene Bytes unmittelbar nach dieser Reservierung.
    total_bytes: u64,
    /// Gehaltene Flows unmittelbar nach dieser Reservierung.
    total_flows: u32,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        self.queue
            .held_bytes
            .fetch_sub(self.bytes, Ordering::AcqRel);
        self.queue.held_flows.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Das Ergebnis von [`HoldQueue::admit`].
enum Admission<'a> {
    /// Das Budget reichte nicht; die Entscheidung steht schon fest.
    Refused(Decision),
    /// Der Flow wird gehalten.
    Held {
        ticket: Ticket<'a>,
        rx: oneshot::Receiver<(Decision, DecisionSource)>,
    },
}

/// Womit das Warten endete.
enum Outcome {
    /// Jemand hat entschieden.
    Decided(Decision, DecisionSource),
    /// Die Frist ist abgelaufen.
    TimedOut,
    /// Der Eintrag ist weg, aber es kam keine Entscheidung. Durch die
    /// Konstruktion unerreichbar (wer den Eintrag nimmt, sendet); wenn doch,
    /// wird geblockt, nie durchgelassen.
    Lost,
}

/// Der gehaltene Flow aus Sicht der wartenden Task.
///
/// Hält die Ausleihe des Flows, die Budget-Reservierung und das Wissen, ob
/// der Flow schon abgeschlossen wurde. Fällt das Ticket vor dem Abschluss,
/// ist der Client weg: `Drop` beendet den Flow mit `ClientTimeout`.
struct Ticket<'a> {
    queue: &'a HoldQueue,
    flow: &'a mut Flow,
    reservation: Option<Reservation<'a>>,
    settled: bool,
}

impl Ticket<'_> {
    /// Wartet auf Entscheidung oder Frist und schließt den Flow ab.
    async fn wait(mut self, mut rx: oneshot::Receiver<(Decision, DecisionSource)>) -> Decision {
        let id = self.flow.id;
        let outcome = loop {
            // Der Eintrag ist weg: `decide` hat ihn genommen und sendet.
            let Some(deadline) = self.queue.deadline(id) else {
                break Outcome::from_channel((&mut rx).await);
            };
            let until = tokio::time::Instant::from_std(deadline);
            match tokio::time::timeout_at(until, &mut rx).await {
                Ok(received) => break Outcome::from_channel(received),
                Err(_elapsed) => {
                    // Nur wer den Eintrag entfernt, hat entschieden. Ist die
                    // Frist inzwischen eine andere, hat `extend` sie
                    // geschoben; ist der Eintrag weg, hat `decide` gewonnen.
                    // Beides löst die nächste Runde auf.
                    let removed = self
                        .queue
                        .pending
                        .remove_if(&id, |_, pending| pending.deadline == deadline);
                    if removed.is_some() {
                        break Outcome::TimedOut;
                    }
                }
            }
        };
        self.settle(outcome)
    }

    /// Wendet den Abschluss auf den Flow an und veröffentlicht das Ereignis.
    ///
    /// Das Budget wird vorher freigegeben, damit ein Zuhörer, der das
    /// Ereignis sieht, es schon frei vorfindet.
    fn settle(&mut self, outcome: Outcome) -> Decision {
        self.settled = true;
        drop(self.reservation.take());
        let (input, decision) = match outcome {
            Outcome::Decided(decision, source) => (
                TransitionInput::Decide {
                    decision: decision.clone(),
                    source,
                },
                decision,
            ),
            Outcome::TimedOut => (TransitionInput::Timeout, Decision::TimedOut),
            Outcome::Lost => {
                tracing::error!(flow = %self.flow.id, "hold entry vanished without a decision; blocking");
                let decision = Decision::Block {
                    reason: BlockReason::NoRoute,
                    note: None,
                };
                (
                    TransitionInput::Decide {
                        decision: decision.clone(),
                        source: DecisionSource::System,
                    },
                    decision,
                )
            }
        };
        match self.flow.apply(input, SystemTime::now()) {
            Ok(event) => {
                self.queue.publish(event);
                decision
            }
            // Durch die Konstruktion unerreichbar: der Flow ist `Held`, die
            // Eingabe wurde gegen `Held` geprüft. Wenn doch: nichts erlauben.
            Err(err) => {
                tracing::error!(flow = %self.flow.id, %err, "held flow refused its final transition; blocking");
                Decision::Block {
                    reason: BlockReason::NoRoute,
                    note: None,
                }
            }
        }
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        let id = self.flow.id;
        self.queue.pending.remove(&id);
        drop(self.reservation.take());
        let decision = Decision::Block {
            reason: BlockReason::ClientTimeout,
            note: None,
        };
        let input = TransitionInput::Decide {
            decision,
            source: DecisionSource::System,
        };
        match self.flow.apply(input, SystemTime::now()) {
            Ok(event) => self.queue.publish(event),
            Err(err) => {
                tracing::error!(flow = %id, %err, "cancelled hold refused its final transition");
            }
        }
    }
}

impl Outcome {
    /// Was der Kanal geliefert hat.
    fn from_channel(
        received: Result<(Decision, DecisionSource), oneshot::error::RecvError>,
    ) -> Self {
        match received {
            Ok((decision, source)) => Self::Decided(decision, source),
            Err(_closed) => Self::Lost,
        }
    }
}

/// Das nächste Ereignis eines Zuhörers, mit `Lagged` als Ereignis statt als
/// Fehler.
///
/// `None`, wenn der Strom geschlossen ist (die [`HoldQueue`] wurde fallen
/// gelassen). Nach einem [`FlowEvent::Lagged`] fehlen dem Zuhörer `n`
/// Ereignisse; er lädt seinen Stand nach, siehe Modulkommentar.
pub async fn next_event(rx: &mut broadcast::Receiver<FlowEvent>) -> Option<FlowEvent> {
    match rx.recv().await {
        Ok(event) => Some(event),
        Err(broadcast::error::RecvError::Lagged(n)) => Some(FlowEvent::Lagged { n }),
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::Limits;
    use humanitl_core::{BlockReason, Decision, DecisionSource, FlowEvent, RuleId};

    use super::{HoldQueue, MAX_EVENT_BUFFER, decidable};

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn queue_is_shareable_between_tasks() {
        assert_send_sync::<HoldQueue>();
    }

    #[test]
    fn budget_reservation_never_overshoots() {
        let limits = Limits {
            hold_max_flows: 2,
            hold_max_bytes: 1024,
            ..Limits::default()
        };
        let queue = HoldQueue::new(&limits);

        let first = queue.reserve(600).expect("fits");
        assert_eq!((first.total_flows, first.total_bytes), (1, 600));
        assert_eq!(queue.reserve(600).err(), Some(BlockReason::HoldMemory));
        assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 600));

        let second = queue.reserve(424).expect("exactly fills the budget");
        assert_eq!((second.total_flows, second.total_bytes), (2, 1024));
        assert_eq!(queue.reserve(0).err(), Some(BlockReason::HoldMaxFlows));

        drop(first);
        assert_eq!((queue.queue_count(), queue.queue_bytes()), (1, 424));
        drop(second);
        assert_eq!((queue.queue_count(), queue.queue_bytes()), (0, 0));
    }

    #[test]
    fn event_buffer_is_clamped_to_what_the_channel_accepts() {
        for event_buffer in [0, 1, MAX_EVENT_BUFFER + 1, usize::MAX] {
            let limits = Limits {
                event_buffer,
                ..Limits::default()
            };
            let queue = HoldQueue::new(&limits);
            let mut rx = queue.subscribe();
            queue.publish(FlowEvent::Lagged { n: 0 });
            assert_eq!(rx.try_recv(), Ok(FlowEvent::Lagged { n: 0 }));
        }
    }

    #[test]
    fn who_may_decide_from_held() {
        let block = Decision::Block {
            reason: BlockReason::User,
            note: None,
        };
        assert!(decidable(&Decision::Allow, DecisionSource::User));
        assert!(decidable(&block, DecisionSource::User));
        assert!(decidable(
            &Decision::Allow,
            DecisionSource::Rule(RuleId::nil())
        ));
        assert!(decidable(&block, DecisionSource::System));
        assert!(!decidable(&Decision::Allow, DecisionSource::System));
        assert!(!decidable(&Decision::Allow, DecisionSource::Passthrough));
        assert!(!decidable(&Decision::TimedOut, DecisionSource::User));
        assert!(!decidable(&Decision::TimedOut, DecisionSource::Timeout));
        assert!(!decidable(&Decision::Allow, DecisionSource::Timeout));
    }
}
