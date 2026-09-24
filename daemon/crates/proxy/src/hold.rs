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
//! Keine Regelauswertung (HUM-022), keine Detektoren (HUM-025). Die Liste
//! aller Flows einer laufenden Sitzung steht nebenan in der [`FlowRegistry`];
//! nach einem Neustart ist sie leer.
//!
//! Gezählt wird trotzdem, weil nur hier alles zusammenkommt, bevor `Decided`
//! hinausgeht: wie viele Funde der Flow beim Halten trug, welche der Mensch mit
//! seiner Freigabe bestätigt hat ([`HoldQueue::decide_acknowledging`]) und, über
//! [`EditCount`], wie viele in einer bearbeiteten Fassung stehen blieben. Das
//! Ergebnis steht als [`DecidedFindings`] im Ereignis (HUM-160).
//!
//! Was bleibt, schreibt die Aufzeichnung: [`HoldQueue::recording`] hängt einen
//! [`Recorder`] in den Trichter, und jedes veröffentlichte Ereignis läuft
//! durch [`Recorder::apply`] (HUM-026). Genauso hängt [`HoldQueue::with_domains`]
//! den Domain-Katalog an, den diese Crate nicht kennen darf ([`DomainSink`]).

use core::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use humanitl_config::Limits;
use humanitl_core::{
    BlockReason, DecidedFindings, Decision, DecisionSource, Diagnostic, Flow, FlowEvent, FlowId,
    FlowState, HostName, HttpRequest, InvalidTransition, Transition, TransitionInput,
};
use humanitl_recorder::Recorder;
use tokio::sync::{broadcast, oneshot};

use crate::registry::FlowRegistry;

/// Mehr Ereignisse als das puffert der Kanal je Zuhörer nicht (65 536).
///
/// Die Obergrenze ist ein Schutz, kein Stellhebel: `tokio` rundet die
/// Kapazität auf eine Zweierpotenz auf und legt den Ring sofort an; ein
/// Zuhörer, der so weit zurückliegt, lädt ohnehin nach (`Lagged`).
pub const MAX_EVENT_BUFFER: usize = 1 << 16;

/// Warum eine Entscheidung nicht angenommen wurde.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
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
    /// Die Bestätigung nennt einen Fund, den der Flow nicht hat: Der Index
    /// liegt außerhalb der Liste aus `Analyzed` (HUM-160). Entschieden wird
    /// nichts; die gRPC-Schicht meldet das als `IPC_004`.
    #[error("flow {id} has {findings} finding(s), so there is no finding {index} to acknowledge")]
    UnknownFinding {
        /// Der Flow, um den es ging.
        id: FlowId,
        /// Der Index, den es nicht gibt.
        index: u32,
        /// Wie viele Funde der Flow beim Halten trug.
        findings: u32,
    },
    /// Eine Bestätigung kam mit einer anderen Entscheidung als `Allow`.
    ///
    /// Bestätigt wird, was unverändert hinausgeht; bei einem Block geht nichts
    /// hinaus, und bei `AllowEdited` zeigten die Indizes in die gehaltene
    /// Fassung statt in die, die hinausgeht (HUM-160).
    #[error("flow {id}: findings are acknowledged with allow only, not with {decision}")]
    AcknowledgedWithout {
        /// Der Flow, um den es ging.
        id: FlowId,
        /// Die Entscheidung, siehe [`Decision::as_str`].
        decision: &'static str,
    },
    /// Der Flow steht unter der harten Sperre
    /// (`hold.hard_block_checksum_secrets`, HUM-159): Er trägt ein
    /// prüfsummen-bestätigtes Geheimnis und darf nicht ungeändert hinaus.
    ///
    /// Entschieden wird nichts, und der Flow wartet weiter: Ein Mensch kann
    /// ihn noch bearbeitet freigeben oder blocken. `refusal` ist der Befund
    /// `HOLD_004` vom Flow; die gRPC-Schicht meldet genau ihn und nicht
    /// `IPC_003`.
    #[error("flow {id} may not leave unchanged: {refusal}")]
    SendRefused {
        /// Der Flow, um den es ging.
        id: FlowId,
        /// Der Befund `HOLD_004`, so wie er am Flow steht.
        refusal: Box<Diagnostic>,
    },
}

impl NotHeld {
    /// Der Flow, um den es ging.
    #[must_use]
    pub const fn id(&self) -> FlowId {
        match self {
            Self::Unknown { id }
            | Self::Forbidden { id, .. }
            | Self::UnknownFinding { id, .. }
            | Self::AcknowledgedWithout { id, .. }
            | Self::SendRefused { id, .. } => *id,
        }
    }

    /// Wahr, wenn die Anfrage selbst nicht stimmte und nicht der Zustand des
    /// Flows: eine Bestätigung, die zu diesem Flow oder dieser Entscheidung
    /// nicht passt.
    #[must_use]
    pub const fn is_bad_request(&self) -> bool {
        matches!(
            self,
            Self::UnknownFinding { .. } | Self::AcknowledgedWithout { .. }
        )
    }
}

/// Zählt die Funde einer bearbeiteten Fassung, bevor `Decided` hinausgeht.
///
/// Die Warteschlange kennt keine Detektoren; sie weiß nur, dass eine Zahl zur
/// Entscheidung gehört. Bei `AllowEdited` ist das die Zahl des zweiten Scans
/// über die bearbeitete Fassung, nicht die der gehaltenen (HUM-160). Die
/// Umsetzung steht in [`crate::edit::SecondScan`]; der Daemon hängt sie mit
/// [`HoldQueue::scanning_edits`] ein.
///
/// Kein Port im Sinne von ADR-015: kein Fremdsystem, kein zweiter Adapter,
/// sondern dieselbe Naht wie [`DomainSink`], damit die Warteschlange ohne
/// Scanner auskommt.
pub trait EditCount: Send + Sync {
    /// Wie viele Funde `edited` trüge, wenn es anstelle von `held` hinausginge.
    ///
    /// `None`, wenn die Bearbeitung so nicht hinausgehen kann (anderes Ziel,
    /// zu großer Body): Der Handler nimmt die Freigabe dann ohnehin zurück,
    /// und eine Zahl über eine Anfrage, die nie hinausgeht, wäre erfunden.
    fn unresolved(&self, held: &HttpRequest, edited: &HttpRequest) -> Option<u32>;
}

/// Was eine Entscheidung über einen gehaltenen Flow zur wartenden Task trägt.
#[derive(Debug)]
struct Verdict {
    decision: Decision,
    source: DecisionSource,
    /// Die bestätigten Funde, aufsteigend und ohne Doppel.
    acknowledged: Vec<u32>,
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
    /// Trägt Entscheidung, Herkunft und Bestätigungen zur wartenden Task.
    tx: oneshot::Sender<Verdict>,
    /// Bis wann gewartet wird; [`HoldQueue::extend`] schiebt sie.
    deadline: Instant,
    /// Wie viele Funde der Flow beim Halten trug; die Grenze für die Indizes
    /// einer Bestätigung.
    findings: u32,
    /// Der Befund der harten Sperre, wenn der Flow einen trägt
    /// ([`Flow::send_refusal`]). Er liegt hier und nicht in der Registry, weil
    /// er genau so lange gelten muss, wie der Flow gehalten wird: Die Registry
    /// darf einen Datensatz vergessen, und eine Prüfung, die dann nichts
    /// fände, ließe das Geheimnis hinaus.
    refusal: Option<Diagnostic>,
}

/// Wer den Domain-Katalog zu einem eingetroffenen Flow befragt.
///
/// Der Katalog selbst lebt in `humanitl-catalog`, und diese Crate darf ihn
/// nicht kennen: `backlog/CONVENTIONS.md` 3.1 erlaubt `humanitl-proxy` nur
/// `core`, `config`, `rules`, `findings` und `recorder`. Der Proxy weiß aber
/// als Einziger, wann ein Flow ankommt, und gezählt werden darf genau einmal
/// je Anfrage ([`Catalog::info`](https://docs.rs/), HUM-031). Deshalb diese
/// eine Zeile Schnittstelle: Der Daemon hängt seine Umsetzung ein
/// (`humanitl_ipc::domains::DomainTable`), der Proxy ruft sie im Trichter auf,
/// und das Ergebnis liegt bereit, bevor das Ereignis in den Strom geht.
///
/// Kein Port im Sinne von ADR-015: hier wird kein Fremdsystem gekapselt und
/// kein zweiter Adapter erwartet, sondern eine Abhängigkeitsrichtung
/// eingehalten, die der Compiler sonst nicht prüfen könnte.
pub trait DomainSink: Send + Sync {
    /// Verbucht genau eine Beobachtung des Hosts dieses Flows.
    ///
    /// Wird aus [`HoldQueue::publish`] für jedes [`FlowEvent::Received`]
    /// aufgerufen, synchron und vor dem Rundfunk, damit jeder Zuhörer die
    /// Angaben zur Domain schon vorfindet.
    fn observe(&self, flow: FlowId, host: &HostName, at: SystemTime);
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
    recorder: Option<Recorder>,
    domains: Option<Arc<dyn DomainSink>>,
    edits: Option<Arc<dyn EditCount>>,
}

impl fmt::Debug for HoldQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HoldQueue")
            .field("pending", &self.pending.len())
            .field("held_flows", &self.queue_count())
            .field("held_bytes", &self.queue_bytes())
            .field("max_flows", &self.max_flows)
            .field("max_bytes", &self.max_bytes)
            .field("recorder", &self.recorder.is_some())
            .field("domains", &self.domains.is_some())
            .field("edits", &self.edits.is_some())
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
            recorder: None,
            domains: None,
            edits: None,
        }
    }

    /// Dieselbe Warteschlange, die jedes Ereignis auch aufzeichnet.
    ///
    /// Ohne Aufzeichnung läuft der Proxy unverändert weiter; die Historie ist
    /// dann leer, was sie vor HUM-026 immer war. Ein Fehler der Aufzeichnung
    /// hält den Proxy nie an: Der Recorder meldet ihn als Befund in seinem
    /// eigenen Strom, den der Daemon in den Ereignisstrom hängt.
    #[must_use]
    pub fn recording(mut self, recorder: Recorder) -> Self {
        self.recorder = Some(recorder);
        self
    }

    /// Dieselbe Warteschlange, die jeden eingetroffenen Flow dem Domain-Katalog
    /// zeigt (siehe [`DomainSink`]).
    #[must_use]
    pub fn with_domains(mut self, domains: Arc<dyn DomainSink>) -> Self {
        self.domains = Some(domains);
        self
    }

    /// Dieselbe Warteschlange, die bei `AllowEdited` die Funde der
    /// bearbeiteten Fassung zählt (siehe [`EditCount`], HUM-160).
    ///
    /// Ohne sie trägt das `Decided`-Ereignis einer bearbeiteten Freigabe keine
    /// Zahl: nicht gezählt, und das steht dann auch so da, statt einer Null.
    #[must_use]
    pub fn counting_edits(mut self, edits: Arc<dyn EditCount>) -> Self {
        self.edits = Some(edits);
        self
    }

    /// [`HoldQueue::counting_edits`] mit dem zweiten Scan der Detektoren
    /// ([`crate::edit::SecondScan`]) bis `cap_bytes`
    /// (`limits.hold_body_cap_bytes`, dieselbe Grenze wie im Handler).
    ///
    /// Der Weg des Daemons: Er nennt damit keinen weiteren Typ dieser Crate
    /// (`tools/check_coupling.py`).
    #[must_use]
    pub fn scanning_edits(
        self,
        scanner: Arc<dyn crate::findings::Scanner>,
        cap_bytes: u64,
    ) -> Self {
        self.counting_edits(Arc::new(crate::edit::SecondScan::new(scanner, cap_bytes)))
    }

    /// Die Aufzeichnung dieser Warteschlange, sofern eine verdrahtet ist.
    #[must_use]
    pub const fn recorder(&self) -> Option<&Recorder> {
        self.recorder.as_ref()
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
        self.decide_acknowledging(id, decision, by, &[])
    }

    /// Wie [`HoldQueue::decide_as`], mit den Funden, die der Mensch gesehen hat
    /// und bewusst hinausgehen lässt („Trotzdem senden", HUM-160).
    ///
    /// `acknowledged` sind Indizes in die Funde aus `Analyzed`; doppelte zählen
    /// einmal. Das `Decided`-Ereignis trägt sie und zählt sie nicht mehr zu den
    /// offenen. Eine Bestätigung ändert nichts an der Entscheidung selbst: Sie
    /// ist eine Spur, keine Erlaubnis.
    ///
    /// # Errors
    ///
    /// Wie [`HoldQueue::decide_as`]; dazu [`NotHeld::UnknownFinding`] für einen
    /// Index außerhalb der Funde, [`NotHeld::AcknowledgedWithout`] für eine
    /// Bestätigung zu einer anderen Entscheidung als `Allow` und
    /// [`NotHeld::SendRefused`] für ein `Allow` auf einen Flow unter der
    /// harten Sperre, gleich wer es trifft und was es bestätigt (HUM-159). In
    /// allen drei Fällen wird nichts entschieden, und der Flow wartet weiter.
    pub fn decide_acknowledging(
        &self,
        id: FlowId,
        decision: Decision,
        by: DecisionSource,
        acknowledged: &[u32],
    ) -> Result<(), NotHeld> {
        if !decidable(&decision, by) {
            return Err(NotHeld::Forbidden {
                id,
                decision: decision.as_str(),
                by,
            });
        }
        // Die harte Sperre vor allem anderen, was ein `Allow` begleitet: Eine
        // Bestätigung hebt sie nie auf (HUM-049), und keine Herkunft, auch
        // keine Regel, gibt einen solchen Flow unbearbeitet frei. Dies ist die
        // eine Tür, durch die jede Entscheidung über einen gehaltenen Flow
        // geht; `AllowEdited` kommt durch und wird im Handler ein zweites Mal
        // gescannt.
        if matches!(decision, Decision::Allow)
            && let Some(refusal) = self.refusal(id)
        {
            return Err(NotHeld::SendRefused {
                id,
                refusal: Box::new(refusal),
            });
        }
        let mut acknowledged = acknowledged.to_vec();
        acknowledged.sort_unstable();
        acknowledged.dedup();
        if !acknowledged.is_empty() {
            if !matches!(decision, Decision::Allow) {
                return Err(NotHeld::AcknowledgedWithout {
                    id,
                    decision: decision.as_str(),
                });
            }
            // Die Zahl der Funde steht seit dem Halten fest; gelesen wird sie
            // vor dem Entfernen, damit eine falsche Bestätigung den Flow
            // weiter warten lässt, statt ihn zu entscheiden.
            let findings = self
                .pending
                .get(&id)
                .map(|pending| pending.findings)
                .ok_or(NotHeld::Unknown { id })?;
            if let Some(&index) = acknowledged.iter().find(|index| **index >= findings) {
                return Err(NotHeld::UnknownFinding {
                    id,
                    index,
                    findings,
                });
            }
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
            .send(Verdict {
                decision,
                source: by,
                acknowledged,
            })
            .map_err(|_gone| NotHeld::Unknown { id })
    }

    /// Der Befund der harten Sperre eines gehaltenen Flows, falls er einen
    /// trägt.
    fn refusal(&self, id: FlowId) -> Option<Diagnostic> {
        self.pending
            .get(&id)
            .and_then(|pending| pending.refusal.clone())
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
    ///
    /// Hier ist der eine Trichter, durch den jedes Ereignis geht, und deshalb
    /// hängen hier auch Aufzeichnung und Domain-Katalog:
    ///
    /// 1. die [`FlowRegistry`], der Zustand dieser Sitzung im Speicher,
    /// 2. [`Recorder::apply`], die dauerhafte Aufzeichnung (HUM-026),
    /// 3. der [`DomainSink`], genau einmal je [`FlowEvent::Received`]
    ///    (HUM-031),
    /// 4. der Rundfunk an die Zuhörer.
    ///
    /// Die Reihenfolge ist Absicht: Die Aufzeichnung sieht die Zeile des
    /// Flows, bevor der Katalog Apex und Kennung nachträgt, und beide sind
    /// fertig, bevor ein Zuhörer das Ereignis bekommt.
    pub fn publish(&self, event: FlowEvent) {
        self.registry.record(&event);
        if let Some(recorder) = &self.recorder {
            recorder.apply(&event);
        }
        if let (
            Some(domains),
            FlowEvent::Received {
                flow_id,
                at,
                request,
            },
        ) = (&self.domains, &event)
        {
            domains.observe(*flow_id, &request.authority.host, *at);
        }
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
                // Die Funde stehen nur in `Analyzed`; nach dem Übergang nach
                // `Held` kennt der Zustand sie nicht mehr. Ihre Zahl ist die
                // Grenze jeder Bestätigung und der Ausgangspunkt der Zählung
                // in `Decided` (HUM-160).
                let findings = match &flow.state {
                    FlowState::Analyzed { findings } => {
                        u32::try_from(findings.len()).unwrap_or(u32::MAX)
                    }
                    _ => 0,
                };
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
                entry.insert(Pending {
                    tx,
                    deadline,
                    findings,
                    refusal: flow.send_refusal.clone(),
                });
                self.publish(event);
                Ok(Admission::Held {
                    ticket: Ticket {
                        queue: self,
                        flow,
                        findings,
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
        rx: oneshot::Receiver<Verdict>,
    },
}

/// Womit das Warten endete.
enum Outcome {
    /// Jemand hat entschieden.
    Decided(Verdict),
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
    /// Wie viele Funde der Flow beim Halten trug.
    findings: u32,
    reservation: Option<Reservation<'a>>,
    settled: bool,
}

impl Ticket<'_> {
    /// Wartet auf Entscheidung oder Frist und schließt den Flow ab.
    async fn wait(mut self, mut rx: oneshot::Receiver<Verdict>) -> Decision {
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
        // Nur eine Entscheidung von außen bringt etwas über die Funde mit;
        // Ablauf und Verlust gehen ohne Zahl hinaus, weil nichts hinausgeht.
        let mut findings = None;
        let (input, decision) = match outcome {
            Outcome::Decided(verdict) => {
                findings = Some(self.tally(&verdict));
                (
                    TransitionInput::Decide {
                        decision: verdict.decision.clone(),
                        source: verdict.source,
                    },
                    verdict.decision,
                )
            }
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
            Ok(mut event) => {
                if let (FlowEvent::Decided { findings: slot, .. }, Some(counted)) =
                    (&mut event, findings)
                {
                    *slot = counted;
                }
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

    /// Was von den Funden offen bleibt, wenn so entschieden wird (HUM-160).
    ///
    /// `Allow` lässt die gehaltene Anfrage hinaus: offen sind ihre Funde ohne
    /// die bestätigten. `AllowEdited` lässt eine andere hinaus; gezählt wird
    /// deren zweiter Scan, und ohne [`EditCount`] steht keine Zahl da statt
    /// einer falschen. Alles andere lässt nichts hinaus.
    fn tally(&self, verdict: &Verdict) -> DecidedFindings {
        match &verdict.decision {
            Decision::Allow => {
                let acknowledged = u32::try_from(verdict.acknowledged.len()).unwrap_or(u32::MAX);
                DecidedFindings {
                    unresolved: Some(self.findings.saturating_sub(acknowledged)),
                    acknowledged: verdict.acknowledged.clone(),
                }
            }
            Decision::AllowEdited { request } => DecidedFindings {
                unresolved: self
                    .queue
                    .edits
                    .as_ref()
                    .and_then(|edits| edits.unresolved(&self.flow.request, request)),
                acknowledged: Vec::new(),
            },
            Decision::Block { .. } | Decision::TimedOut => DecidedFindings::default(),
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
    fn from_channel(received: Result<Verdict, oneshot::error::RecvError>) -> Self {
        match received {
            Ok(verdict) => Self::Decided(verdict),
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
