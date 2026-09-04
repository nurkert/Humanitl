//! Der Lebenszyklus einer Anfrage als Zustandsautomat.
//!
//! Es gibt genau einen Weg, den Zustand eines Flows zu ändern:
//! [`FlowState::on`]. Die Methode nimmt den Zustand, verbraucht ihn, und gibt
//! den Folgezustand samt dem Ereignis zurück, das daraus entsteht. Ein
//! ungültiger Übergang ist ein Fehlerwert, kein Panic und kein stiller
//! Sonderfall.
//!
//! Das Ereignis ist Ausgabe, nie Eingabe. Eingabe ist ein [`Transition`]:
//! Absicht plus Flow-Id plus Zeitpunkt. Damit bleibt der Kern rein — er liest
//! keine Uhr — und der Tabellentest kann jedes Paar aus Zustand und Eingabe
//! aufzählen.
//!
//! Erlaubte Übergänge (alles andere ist [`InvalidTransition`]):
//!
//! | von | Eingabe | nach | Ereignis |
//! |---|---|---|---|
//! | `Received` | `Analyze` | `Analyzed` | `Analyzed` |
//! | `Analyzed` | `Hold` | `Held` | `Held` |
//! | `Analyzed` | `Decide` (Regel, Passthrough oder System-Ablehnung) | `Decided` | `Decided` |
//! | `Held` | `Decide` (Nutzer, Regel oder System-Ablehnung) | `Decided` | `Decided` |
//! | `Held` | `Timeout` | `Decided(TimedOut)` | `TimedOut` |
//! | `Decided(Allow\|AllowEdited)` | `Decide` (nur System, nur `Block`) | `Decided(Block)` | `Decided` |
//! | `Decided(Allow\|AllowEdited)` | `Forward` | `Forwarded` | `Forwarded` |
//! | `Decided(Allow\|AllowEdited)` | `Fail` | `Failed` | `Failed` |
//! | `Decided(Block\|TimedOut)` | `Record` | `Recorded` | `Recorded` |
//! | `Forwarded` | `Respond` | `Responded` | `ResponseHeaders` |
//! | `Forwarded` | `Fail` | `Failed` | `Failed` |
//! | `Responded` | `Record` | `Recorded` | `Recorded` |
//! | `Failed` | `Record` | `Recorded` | `Recorded` |

use core::fmt;
use std::net::IpAddr;
use std::time::{Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::event::FlowEvent;
use crate::finding::Finding;
use crate::http::HttpRequest;
use crate::ids::{FlowId, RuleId, SessionId};

/// Warum eine Anfrage geblockt wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    /// Der Mensch hat geblockt.
    User,
    /// Eine Regel hat geblockt.
    Rule(RuleId),
    /// Die Wartezeit lief ab, ohne dass jemand entschied.
    Timeout,
    /// Der Body ist größer als `limits.hold_body_cap_bytes`.
    BodyCap,
    /// `Host`-Header und TLS-Ziel widersprechen sich.
    AuthorityMismatch,
    /// Es gibt keinen Weg zum Ziel.
    NoRoute,
    /// Der Halte-Speicher (`limits.hold_max_bytes`) ist erschöpft.
    HoldMemory,
    /// Es werden bereits `limits.hold_max_flows` Anfragen gehalten.
    HoldMaxFlows,
    /// Der Client hat die Verbindung aufgegeben, während gewartet wurde.
    ClientTimeout,
    /// Das Ziel liegt nach der Auflösung in einem privaten Netz und die Regel
    /// erlaubt das nicht.
    PrivateAddress,
    /// Die Anfrage trägt ein Geheimnis, dessen Prüfsumme aufgeht, und
    /// `hold.hard_block_checksum_secrets` steht an. Das System blockt ohne
    /// Rückfrage; niemand wurde gefragt, also darf hier auch nicht `user`
    /// stehen (`backlog/CONVENTIONS.md` 4.13: nie mehr behaupten als
    /// bewiesen ist).
    Secret,
}

impl BlockReason {
    /// Kurzname in `snake_case`, wie er in der Antwort an den Client steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Rule(_) => "rule",
            Self::Timeout => "timeout",
            Self::BodyCap => "body_cap",
            Self::AuthorityMismatch => "authority_mismatch",
            Self::NoRoute => "no_route",
            Self::HoldMemory => "hold_memory",
            Self::HoldMaxFlows => "hold_max_flows",
            Self::ClientTimeout => "client_timeout",
            Self::PrivateAddress => "private_address",
            Self::Secret => "secret",
        }
    }

    /// Die Regel, die geblockt hat, falls es eine war.
    #[must_use]
    pub const fn rule_id(self) -> Option<RuleId> {
        match self {
            Self::Rule(id) => Some(id),
            _ => None,
        }
    }

    /// Der HTTP-Status, mit dem der Proxy dem Client antwortet.
    ///
    /// Verbindlich nach `backlog/CONVENTIONS.md` 3.2: `403` für Entscheidungen
    /// gegen die Anfrage selbst, `413` für einen zu großen Body, `504` für eine
    /// abgelaufene Wartezeit, `503` für erschöpfte Halte-Budgets. Ein
    /// gescheiterter Upstream ist kein Block, sondern `502`, siehe
    /// [`UpstreamError::http_status`].
    ///
    /// `NoRoute` und `ClientTimeout` sind dort nicht festgelegt; sie folgen der
    /// nächstliegenden Bedeutung (`502` beziehungsweise `408`).
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::User
            | Self::Rule(_)
            | Self::AuthorityMismatch
            | Self::PrivateAddress
            | Self::Secret => 403,
            Self::BodyCap => 413,
            Self::Timeout => 504,
            Self::HoldMemory | Self::HoldMaxFlows => 503,
            Self::NoRoute => 502,
            Self::ClientTimeout => 408,
        }
    }
}

impl fmt::Display for BlockReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Warum die Verbindung zum Ziel gescheitert ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamError {
    /// Der Name ließ sich nicht auflösen.
    Dns,
    /// Die TCP-Verbindung kam nicht zustande.
    Connect,
    /// Der TLS-Handschlag scheiterte.
    Tls,
    /// Die aufgelöste Adresse liegt in einem privaten Netz.
    PrivateAddress(IpAddr),
    /// Das Ziel antwortete nicht rechtzeitig.
    Timeout,
}

impl UpstreamError {
    /// Kurzname der Variante in `snake_case`, gleich dem serde-Tag.
    ///
    /// Nicht für die Antwort an den Client: dort steht [`UpstreamError::reason`],
    /// weil `timeout` und `private_address` sonst mit [`BlockReason`]
    /// zusammenfielen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::Connect => "connect",
            Self::Tls => "tls",
            Self::PrivateAddress(_) => "private_address",
            Self::Timeout => "timeout",
        }
    }

    /// Der Name in der `reason:`-Zeile der Antwort an den Client.
    ///
    /// Immer mit Präfix `upstream_` (`BACKLOG.md` ADR-004), damit der Agent
    /// einen gescheiterten Upstream (`502`) von einer Entscheidung gegen die
    /// Anfrage unterscheiden kann: `upstream_timeout` ist nicht
    /// [`BlockReason::Timeout`], `upstream_private_address` nicht
    /// [`BlockReason::PrivateAddress`]. Kein Wert hier ist je gleich einem
    /// [`BlockReason::as_str`]; ein Test hält das fest.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Dns => "upstream_dns",
            Self::Connect => "upstream_connect",
            Self::Tls => "upstream_tls",
            Self::PrivateAddress(_) => "upstream_private_address",
            Self::Timeout => "upstream_timeout",
        }
    }

    /// Der HTTP-Status, mit dem der Proxy dem Client antwortet: immer `502`.
    ///
    /// Ein Upstream-Fehler wird nie als [`FlowState::Responded`] verbucht,
    /// sondern als [`FlowState::Failed`]; die Aufzeichnung unterscheidet
    /// dadurch „das Ziel hat mit 502 geantwortet" von „wir kamen nicht hin".
    #[must_use]
    pub const fn http_status(self) -> u16 {
        502
    }
}

impl fmt::Display for UpstreamError {
    /// Die Form aus [`UpstreamError::reason`], bei `PrivateAddress` mit der
    /// Adresse dahinter: `upstream_private_address:10.0.0.1`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrivateAddress(ip) => write!(f, "{}:{ip}", self.reason()),
            other => f.write_str(other.reason()),
        }
    }
}

/// Wie über eine Anfrage entschieden wurde.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Durchlassen, unverändert.
    Allow,
    /// Durchlassen, aber mit dieser bearbeiteten Anfrage.
    AllowEdited {
        /// Die Anfrage, die tatsächlich hinausgeht.
        request: Box<HttpRequest>,
    },
    /// Blocken.
    Block {
        /// Warum.
        reason: BlockReason,
        /// Freitext des Nutzers für den Agenten; landet gekürzt in der Antwort.
        note: Option<String>,
    },
    /// Die Wartezeit lief ab.
    TimedOut,
}

impl Decision {
    /// Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::AllowEdited { .. } => "allow_edited",
            Self::Block { .. } => "block",
            Self::TimedOut => "timed_out",
        }
    }

    /// Wahr, wenn die Anfrage hinausgeht.
    #[must_use]
    pub const fn is_allow(&self) -> bool {
        matches!(self, Self::Allow | Self::AllowEdited { .. })
    }

    /// Wahr, wenn die Anfrage nicht hinausgeht.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::Block { .. } | Self::TimedOut)
    }

    /// Der Grund, falls geblockt wurde.
    #[must_use]
    pub const fn block_reason(&self) -> Option<BlockReason> {
        match self {
            Self::Block { reason, .. } => Some(*reason),
            _ => None,
        }
    }

    /// Der HTTP-Status, mit dem der Proxy antwortet, falls nicht durchgelassen wird.
    #[must_use]
    pub const fn http_status(&self) -> Option<u16> {
        match self {
            Self::Block { reason, .. } => Some(reason.http_status()),
            Self::TimedOut => Some(BlockReason::Timeout.http_status()),
            _ => None,
        }
    }
}

/// Wer entschieden hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSource {
    /// Ein Mensch in der Oberfläche oder im Terminal.
    User,
    /// Eine Regel.
    Rule(RuleId),
    /// Die abgelaufene Wartezeit.
    Timeout,
    /// Die Passthrough-Regel für den LLM-Endpunkt.
    Passthrough,
    /// Der Daemon selbst, etwa wegen eines erschöpften Budgets.
    System,
}

impl DecisionSource {
    /// Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Rule(_) => "rule",
            Self::Timeout => "timeout",
            Self::Passthrough => "passthrough",
            Self::System => "system",
        }
    }
}

/// Zustand eines Flows.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowState {
    /// Die Anfrage ist angekommen, nichts ist geprüft.
    Received,
    /// Die Detektoren sind gelaufen.
    Analyzed {
        /// Was gefunden wurde.
        findings: Vec<Finding>,
    },
    /// Die Anfrage wartet auf eine Entscheidung.
    Held {
        /// Bis wann gewartet wird.
        deadline: Instant,
    },
    /// Es ist entschieden.
    Decided(Decision),
    /// Die Anfrage ist auf dem Weg zum Ziel.
    Forwarded,
    /// Das Ziel hat geantwortet.
    Responded {
        /// Der Status der Antwort.
        status: u16,
    },
    /// Die Verbindung zum Ziel ist gescheitert.
    Failed {
        /// Woran.
        error: UpstreamError,
    },
    /// Endzustand: der Flow ist aufgezeichnet.
    Recorded,
}

impl FlowState {
    /// Kurzname des Zustands in `snake_case`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Analyzed { .. } => "analyzed",
            Self::Held { .. } => "held",
            Self::Decided(_) => "decided",
            Self::Forwarded => "forwarded",
            Self::Responded { .. } => "responded",
            Self::Failed { .. } => "failed",
            Self::Recorded => "recorded",
        }
    }

    /// Wahr für den Endzustand [`FlowState::Recorded`].
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Recorded)
    }

    /// Der gemeinsame Ausgang jeder Entscheidung: der neue Zustand und das
    /// Ereignis, das ihn trägt.
    fn decided(
        decision: Decision,
        source: DecisionSource,
        flow_id: FlowId,
        at: SystemTime,
    ) -> (Self, FlowEvent) {
        (
            Self::Decided(decision.clone()),
            FlowEvent::Decided {
                flow_id,
                at,
                decision,
                source,
            },
        )
    }

    /// Führt einen Übergang aus.
    ///
    /// Gibt den Folgezustand und das Ereignis zurück, das dabei entsteht. Die
    /// Tabelle der erlaubten Paare steht im Modul-Kommentar.
    ///
    /// # Errors
    ///
    /// [`InvalidTransition`] mit Name des Zustands und der Eingabe, wenn das
    /// Paar nicht in der Tabelle steht. Der Zustand ist damit verbraucht; der
    /// Aufrufer behält seinen eigenen (siehe [`Flow::apply`]).
    pub fn on(self, transition: Transition) -> Result<(Self, FlowEvent), InvalidTransition> {
        let Transition { flow_id, at, input } = transition;
        let invalid = InvalidTransition {
            from: self.name(),
            input: input.name(),
        };

        match (self, input) {
            (Self::Received, TransitionInput::Analyze { findings }) => Ok((
                Self::Analyzed {
                    findings: findings.clone(),
                },
                FlowEvent::Analyzed {
                    flow_id,
                    at,
                    findings,
                },
            )),
            (
                Self::Analyzed { .. },
                TransitionInput::Hold {
                    deadline,
                    queue_bytes,
                    queue_count,
                },
            ) => Ok((
                Self::Held { deadline },
                FlowEvent::Held {
                    flow_id,
                    at,
                    deadline,
                    queue_bytes,
                    queue_count,
                },
            )),
            (Self::Analyzed { .. }, TransitionInput::Decide { decision, source })
                if may_decide(false, source, &decision) =>
            {
                Ok(Self::decided(decision, source, flow_id, at))
            }
            (Self::Held { .. }, TransitionInput::Decide { decision, source })
                if may_decide(true, source, &decision) =>
            {
                Ok(Self::decided(decision, source, flow_id, at))
            }
            (Self::Held { .. }, TransitionInput::Timeout) => Ok((
                Self::Decided(Decision::TimedOut),
                FlowEvent::TimedOut { flow_id, at },
            )),
            // Das System darf eine Freigabe vor dem Weiterleiten noch in eine
            // Sperre verwandeln, nie umgekehrt: Stellt der Proxy nach der
            // Entscheidung fest, dass die freigegebene Anfrage nicht die ist,
            // fuer die entschieden wurde (etwa eine bearbeitete Anfrage mit
            // anderem Ziel), endet der Flow als Sperre und nicht als
            // erfundener Upstream-Fehler.
            (
                Self::Decided(Decision::Allow | Decision::AllowEdited { .. }),
                TransitionInput::Decide {
                    decision: decision @ Decision::Block { .. },
                    source: DecisionSource::System,
                },
            ) => Ok(Self::decided(decision, DecisionSource::System, flow_id, at)),
            (
                Self::Decided(Decision::Allow | Decision::AllowEdited { .. }),
                TransitionInput::Forward,
            ) => Ok((Self::Forwarded, FlowEvent::Forwarded { flow_id, at })),
            (
                Self::Decided(Decision::Allow | Decision::AllowEdited { .. }) | Self::Forwarded,
                TransitionInput::Fail { error },
            ) => Ok((
                Self::Failed { error },
                FlowEvent::Failed { flow_id, at, error },
            )),
            (Self::Forwarded, TransitionInput::Respond { status }) => Ok((
                Self::Responded { status },
                FlowEvent::ResponseHeaders {
                    flow_id,
                    at,
                    status,
                },
            )),
            (
                Self::Decided(Decision::Block { .. } | Decision::TimedOut)
                | Self::Responded { .. }
                | Self::Failed { .. },
                TransitionInput::Record,
            ) => Ok((Self::Recorded, FlowEvent::Recorded { flow_id, at })),
            _ => Err(invalid),
        }
    }
}

/// Darf diese Herkunft in diesem Zustand so entscheiden?
///
/// Nach `Analyzed` entscheidet nur eine Regel oder der LLM-Passthrough, nach
/// `Held` zusätzlich der Mensch. Der Daemon selbst (`System`) darf ablehnen,
/// aber niemals durchlassen: ein erschöpftes Halte-Budget oder ein
/// aufgegebener Client beendet den Flow, ohne dass jemand die Anfrage
/// stillschweigend erlaubt.
fn may_decide(from_held: bool, source: DecisionSource, decision: &Decision) -> bool {
    match source {
        DecisionSource::Rule(_) => true,
        DecisionSource::Passthrough => !from_held,
        DecisionSource::User => from_held,
        DecisionSource::Timeout => false,
        DecisionSource::System => decision.is_refusal(),
    }
}

/// Die Absicht hinter einem Übergang.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionInput {
    /// Die Detektoren sind gelaufen.
    Analyze {
        /// Was gefunden wurde.
        findings: Vec<Finding>,
    },
    /// Die Anfrage wird gehalten.
    Hold {
        /// Bis wann gewartet wird.
        deadline: Instant,
        /// Wie viele Bytes danach insgesamt gehalten werden.
        queue_bytes: u64,
        /// Wie viele Flows danach insgesamt gehalten werden.
        queue_count: u32,
    },
    /// Es wird entschieden.
    Decide {
        /// Die Entscheidung.
        decision: Decision,
        /// Wer entscheidet.
        source: DecisionSource,
    },
    /// Die Anfrage geht hinaus.
    Forward,
    /// Das Ziel hat mit Kopfzeilen geantwortet.
    Respond {
        /// Der Status der Antwort.
        status: u16,
    },
    /// Der Flow wird abgeschlossen.
    Record,
    /// Die Wartezeit ist abgelaufen.
    Timeout,
    /// Die Verbindung zum Ziel ist gescheitert.
    Fail {
        /// Woran.
        error: UpstreamError,
    },
}

impl TransitionInput {
    /// Kurzname der Eingabe in `snake_case`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Analyze { .. } => "analyze",
            Self::Hold { .. } => "hold",
            Self::Decide { .. } => "decide",
            Self::Forward => "forward",
            Self::Respond { .. } => "respond",
            Self::Record => "record",
            Self::Timeout => "timeout",
            Self::Fail { .. } => "fail",
        }
    }
}

/// Eingabe des Automaten: Absicht, Flow und Zeitpunkt.
///
/// Flow-Id und Zeitpunkt stehen hier, weil das erzeugte Ereignis beides trägt
/// und der Kern selbst keine Uhr lesen darf.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// Der Flow, um den es geht.
    pub flow_id: FlowId,
    /// Wann der Übergang geschieht.
    pub at: SystemTime,
    /// Was geschehen soll.
    pub input: TransitionInput,
}

impl Transition {
    /// Baut einen Übergang.
    #[must_use]
    pub const fn new(flow_id: FlowId, at: SystemTime, input: TransitionInput) -> Self {
        Self { flow_id, at, input }
    }

    /// Die Detektoren sind gelaufen.
    #[must_use]
    pub const fn analyze(flow_id: FlowId, at: SystemTime, findings: Vec<Finding>) -> Self {
        Self::new(flow_id, at, TransitionInput::Analyze { findings })
    }

    /// Die Anfrage wird gehalten; die Zähler der Halte-Warteschlange kommen mit.
    #[must_use]
    pub const fn hold(
        flow_id: FlowId,
        at: SystemTime,
        deadline: Instant,
        queue_bytes: u64,
        queue_count: u32,
    ) -> Self {
        Self::new(
            flow_id,
            at,
            TransitionInput::Hold {
                deadline,
                queue_bytes,
                queue_count,
            },
        )
    }

    /// Es wird entschieden.
    #[must_use]
    pub const fn decide(
        flow_id: FlowId,
        at: SystemTime,
        decision: Decision,
        source: DecisionSource,
    ) -> Self {
        Self::new(flow_id, at, TransitionInput::Decide { decision, source })
    }

    /// Die Anfrage geht hinaus.
    #[must_use]
    pub const fn forward(flow_id: FlowId, at: SystemTime) -> Self {
        Self::new(flow_id, at, TransitionInput::Forward)
    }

    /// Das Ziel hat mit Kopfzeilen geantwortet.
    #[must_use]
    pub const fn respond(flow_id: FlowId, at: SystemTime, status: u16) -> Self {
        Self::new(flow_id, at, TransitionInput::Respond { status })
    }

    /// Der Flow wird abgeschlossen.
    #[must_use]
    pub const fn record(flow_id: FlowId, at: SystemTime) -> Self {
        Self::new(flow_id, at, TransitionInput::Record)
    }

    /// Die Wartezeit ist abgelaufen.
    #[must_use]
    pub const fn timeout(flow_id: FlowId, at: SystemTime) -> Self {
        Self::new(flow_id, at, TransitionInput::Timeout)
    }

    /// Die Verbindung zum Ziel ist gescheitert.
    #[must_use]
    pub const fn fail(flow_id: FlowId, at: SystemTime, error: UpstreamError) -> Self {
        Self::new(flow_id, at, TransitionInput::Fail { error })
    }
}

/// Ein Übergang, den es nicht gibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid transition from {from} on {input}")]
pub struct InvalidTransition {
    /// Zustand, in dem der Versuch geschah.
    pub from: &'static str,
    /// Eingabe, die nicht passt.
    pub input: &'static str,
}

/// Ein Flow mit allem, was der Daemon über ihn weiß.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    /// Id des Flows.
    pub id: FlowId,
    /// Sitzung, zu der er gehört.
    pub session: SessionId,
    /// Wann die Anfrage ankam.
    pub received_at: SystemTime,
    /// Die Anfrage.
    pub request: HttpRequest,
    /// Der aktuelle Zustand.
    pub state: FlowState,
    /// Alle bisherigen Zustände mit Zeitpunkt, beginnend bei `received`.
    pub history: Vec<(SystemTime, &'static str)>,
    /// Ob die Regel, die diesen Flow freigegeben hat, private Zieladressen
    /// erlaubt (`allow_private`, ADR-006).
    ///
    /// Kein Zustand, sondern eine Eigenschaft der Entscheidung: Die
    /// Regel-Engine setzt es beim Freigeben, der Upstream liest es nach der
    /// Auflösung. Ohne dieses Feld fällt die Erlaubnis zwischen Entscheidung
    /// und Verbindung heraus, und die Durchreichregel zum lokalen
    /// Sprachmodell auf der Schleife kann nie greifen. Vorgabe ist `false`:
    /// Wer nichts sagt, erlaubt kein privates Ziel.
    pub allow_private: bool,
}

impl Flow {
    /// Ein neuer Flow im Zustand [`FlowState::Received`].
    #[must_use]
    pub fn new(
        id: FlowId,
        session: SessionId,
        received_at: SystemTime,
        request: HttpRequest,
    ) -> Self {
        Self {
            id,
            session,
            received_at,
            request,
            state: FlowState::Received,
            history: vec![(received_at, "received")],
            allow_private: false,
        }
    }

    /// Das Ereignis, mit dem der Flow im Strom auftaucht.
    #[must_use]
    pub fn received_event(&self) -> FlowEvent {
        FlowEvent::Received {
            flow_id: self.id,
            at: self.received_at,
            request: Box::new(self.request.clone()),
        }
    }

    /// Wendet einen Übergang an, ersetzt den Zustand und hängt an die Historie an.
    ///
    /// # Errors
    ///
    /// [`InvalidTransition`], wenn das Paar aus Zustand und Eingabe nicht in
    /// der Tabelle steht. Zustand und Historie bleiben dann unverändert.
    pub fn apply(
        &mut self,
        input: TransitionInput,
        at: SystemTime,
    ) -> Result<FlowEvent, InvalidTransition> {
        let (next, event) = self.state.clone().on(Transition::new(self.id, at, input))?;
        self.history.push((at, next.name()));
        self.state = next;
        Ok(event)
    }

    /// Bringt den Flow von jedem Zustand aus fail-closed nach
    /// [`FlowState::Recorded`] und gibt die Ereignisse zurück, die dabei
    /// entstehen.
    ///
    /// Der Aufrufer braucht das, wenn ein Übergang abgelehnt wurde und der Flow
    /// deshalb nicht weiterlaufen darf: Die Anfrage erreicht ihr Ziel nicht, und
    /// der Flow soll trotzdem sauber enden, statt in der Registry für immer in
    /// `Received`, `Analyzed` oder `Forwarded` zu hängen.
    ///
    /// Die Methode enthält bewusst **keine** zweite Übergangstabelle. Sie
    /// versucht der Reihe nach vier Absichten; welche davon gilt, entscheidet
    /// allein [`FlowState::on`], und eine abgelehnte Absicht lässt Zustand und
    /// Historie unberührt:
    ///
    /// 1. `Analyze` ohne Befunde, damit ein Flow in `Received` überhaupt
    ///    entscheidbar wird.
    /// 2. `Decide` auf `Block { reason }` durch [`DecisionSource::System`].
    ///    Das gilt aus `Analyzed`, aus `Held` und auch aus
    ///    `Decided(Allow | AllowEdited)`: Eine Freigabe, die nicht weiterlaufen
    ///    darf, endet als Sperre und nicht als erfundener Upstream-Fehler.
    /// 3. `Fail { aborted }` — nur `Forwarded` kommt hier an, denn dort ist die
    ///    Anfrage schon hinausgegangen und eine Sperre wäre die Unwahrheit.
    ///    Welcher Upstream-Fehler das ist, sagt der Aufrufer: Der Automat kennt
    ///    keinen, und einen zu erfinden hieße, dem Protokoll einen Vorgang
    ///    anzudichten, den es nicht gab. Der Aufrufer muss dieselbe Angabe auch
    ///    in seiner Antwort an den Client führen, sonst behaupten Antwort und
    ///    Aufzeichnung Verschiedenes.
    /// 4. `Record`.
    ///
    /// Jeder Zustand erreicht damit `Recorded`; `flow_ends_recorded_from_every_state`
    /// zählt sie auf. Ein Flow, der schon `Recorded` ist, bleibt es und liefert
    /// keine Ereignisse.
    ///
    /// Der Aufrufer veröffentlicht die zurückgegebenen Ereignisse selbst. Ein
    /// Befund zum abgelehnten Übergang gehört nicht hierher: den kennt nur der
    /// Aufrufer, und er hat ihn zu diesem Zeitpunkt schon gemeldet.
    #[must_use]
    pub fn fail_closed(
        &mut self,
        reason: BlockReason,
        aborted: UpstreamError,
        at: SystemTime,
    ) -> Vec<FlowEvent> {
        let plan = [
            TransitionInput::Analyze {
                findings: Vec::new(),
            },
            TransitionInput::Decide {
                decision: Decision::Block { reason, note: None },
                source: DecisionSource::System,
            },
            TransitionInput::Fail { error: aborted },
            TransitionInput::Record,
        ];
        let mut events = Vec::with_capacity(plan.len());
        for input in plan {
            if let Ok(event) = self.apply(input, at) {
                events.push(event);
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::net::{IpAddr, Ipv4Addr};

    use super::{BlockReason, UpstreamError};
    use crate::ids::RuleId;

    const BLOCK_REASONS: [BlockReason; 10] = [
        BlockReason::User,
        BlockReason::Rule(RuleId::nil()),
        BlockReason::Timeout,
        BlockReason::BodyCap,
        BlockReason::AuthorityMismatch,
        BlockReason::NoRoute,
        BlockReason::HoldMemory,
        BlockReason::HoldMaxFlows,
        BlockReason::ClientTimeout,
        BlockReason::PrivateAddress,
    ];

    const UPSTREAM_ERRORS: [UpstreamError; 5] = [
        UpstreamError::Dns,
        UpstreamError::Connect,
        UpstreamError::Tls,
        UpstreamError::PrivateAddress(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
        UpstreamError::Timeout,
    ];

    #[test]
    fn upstream_reasons_carry_the_prefix_and_never_collide_with_block_reasons() {
        for error in UPSTREAM_ERRORS {
            let reason = error.reason();
            assert!(reason.starts_with("upstream_"), "{reason} must be prefixed");
            assert!(
                error.to_string().starts_with(reason),
                "display of {error:?} must start with its reason"
            );
            for block in BLOCK_REASONS {
                assert_ne!(
                    reason,
                    block.as_str(),
                    "an upstream failure must never read like a block"
                );
            }
        }
        assert_eq!(
            UpstreamError::PrivateAddress(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))).to_string(),
            "upstream_private_address:10.0.0.1"
        );
    }

    #[test]
    fn every_upstream_error_is_a_bad_gateway() {
        for error in UPSTREAM_ERRORS {
            assert_eq!(error.http_status(), 502, "{error}");
        }
    }
}
