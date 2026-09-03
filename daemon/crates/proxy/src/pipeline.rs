//! Die Entscheidungsstufe: aus einem analysierten Flow wird eine
//! [`Decision`].
//!
//! Der Handler treibt den Zustandsautomaten (`docs/ARCHITECTURE.md` 3); die
//! Pipeline ist nur die Strategie, die entscheidet, was mit einem Flow im
//! Zustand [`FlowState::Analyzed`]
//! geschieht: halten und auf einen Menschen warten, oder sofort automatisch
//! entscheiden.
//!
//! In M1 gibt es zwei Strategien:
//!
//! - [`AskPipeline`] hält jeden Flow über die [`HoldQueue`], bis jemand
//!   entscheidet oder die Frist abläuft. Das ist der Normalfall, solange die
//!   Regel-Engine (HUM-022) fehlt.
//! - [`PassthroughPipeline`] lässt jeden Flow sofort durch
//!   ([`DecisionSource::Passthrough`]). Sie ist der Test-Hook, mit dem sich der
//!   Weiterleitungs-Pfad ohne Oberfläche prüfen lässt, und der Platzhalter für
//!   die spätere LLM-Passthrough-Regel.
//!
//! Findings gibt es noch nicht (HUM-025); die Analyse liefert eine leere
//! Liste, und der Handler hat sie vor dem Aufruf schon veröffentlicht.
//!
//! Beide Strategien tragen den Flow zuerst in die
//! [`FlowRegistry`](crate::registry::FlowRegistry) ein. Danach schreibt sich
//! der Datensatz von allein fort, weil jedes Ereignis durch
//! [`HoldQueue::publish`] läuft: `Held` mit der Frist, `Decided` oder
//! `TimedOut` mit der Entscheidung, `Forwarded`, `ResponseHeaders` und
//! `Recorded` aus dem Handler.

use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use humanitl_core::{Authority, Decision, DecisionSource, Flow, FlowState, SessionId};

use crate::hold::HoldQueue;
use crate::registry::FlowRecord;

/// Was der Handler über die Verbindung weiß, aus der ein Flow stammt.
///
/// Pro Verbindung, nicht pro Flow: eine Keep-Alive-Verbindung trägt mehrere
/// Flows mit denselben Angaben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnMeta {
    /// Die Sitzung, zu der die Verbindung gehört.
    pub session: SessionId,
    /// Das Ziel eines `CONNECT`-Tunnels, falls die Verbindung einer ist.
    pub connect_authority: Option<Authority>,
    /// Wahr, wenn der Handler auf dieser Verbindung TLS terminiert hat.
    pub tls: bool,
    /// Erlaubt Ziele in privaten Netzen (RFC 1918, Loopback, Link-Local,
    /// CGNAT). In M1 ein Test-Hook; später setzt ihn die
    /// LLM-Passthrough-Regel (`backlog/CONVENTIONS.md` 4.10).
    pub allow_private: bool,
}

impl ConnMeta {
    /// Eine Verbindung ohne Tunnel, ohne TLS, ohne private Ziele.
    #[must_use]
    pub const fn plain(session: SessionId) -> Self {
        Self {
            session,
            connect_authority: None,
            tls: false,
            allow_private: false,
        }
    }
}

/// Entscheidet über einen analysierten Flow.
///
/// Der Flow ist beim Aufruf in [`FlowState::Analyzed`];
/// beim Rückkehren in [`FlowState::Decided`].
/// Die Implementierung veröffentlicht das entscheidende Ereignis (`Held` und
/// dann `Decided` beziehungsweise `TimedOut`) selbst über die
/// [`HoldQueue`].
#[async_trait]
pub trait FlowPipeline: Send + Sync {
    /// Entscheidet und lässt den Flow in `Decided` zurück.
    async fn decide(&self, flow: &mut Flow, meta: &ConnMeta) -> Decision;
}

/// Hält jeden Flow, bis ein Mensch entscheidet oder die Frist abläuft.
#[derive(Debug)]
pub struct AskPipeline {
    queue: std::sync::Arc<HoldQueue>,
    timeout: Duration,
}

impl AskPipeline {
    /// Eine Pipeline, die über `queue` hält und `timeout` als Frist setzt.
    ///
    /// `timeout` ist `hold.timeout_secs`; `0` (oder `ask_mode = none`) blockt
    /// alles, weil die Frist sofort abläuft.
    #[must_use]
    pub const fn new(queue: std::sync::Arc<HoldQueue>, timeout: Duration) -> Self {
        Self { queue, timeout }
    }
}

#[async_trait]
impl FlowPipeline for AskPipeline {
    /// Die fünf Schritte aus HUM-016: eintragen, Frist setzen und halten,
    /// warten, den Ausgang verbuchen, zurückgeben.
    ///
    /// Schritt 1 ist der Eintrag in die Registry; `Received` und `Analyzed` hat
    /// der Handler schon veröffentlicht (`backlog/CONVENTIONS.md` 4.11), der
    /// Datensatz beginnt deshalb im Zustand des übergebenen Flows. Die Schritte
    /// 2 und 4 gehen durch die Warteschlange und damit durch
    /// [`HoldQueue::publish`], das die Registry mitführt; `Recorded` (Schritt 4
    /// für `Block` und `TimedOut`) hängt der Handler an, nachdem die
    /// Block-Antwort steht.
    async fn decide(&self, flow: &mut Flow, meta: &ConnMeta) -> Decision {
        self.queue.registry().insert(FlowRecord::new(flow, meta));
        let deadline = Instant::now()
            .checked_add(self.timeout)
            .unwrap_or_else(Instant::now);
        // Das Future aus `hold` leiht `flow` bis zur Entscheidung; im
        // Fehlerfall endet die Leihe mit dem `match`, deshalb die Trennung.
        let err = match self.queue.hold(flow, deadline) {
            Ok(held) => return held.await,
            Err(err) => err,
        };
        // Der Flow ist analysiert und die Id ist frisch; ein Fehler hier ist
        // ein Programmierfehler, kein Laufzeitzustand. Fail closed: blocken
        // statt durchlassen.
        tracing::error!(flow = %flow.id, %err, "could not hold flow; blocking");
        system_block(&self.queue, flow)
    }
}

/// Lässt jeden Flow sofort durch (Test-Hook, LLM-Passthrough-Platzhalter).
#[derive(Debug)]
pub struct PassthroughPipeline {
    queue: std::sync::Arc<HoldQueue>,
}

impl PassthroughPipeline {
    /// Eine Pipeline, die jeden Flow ohne Warten erlaubt und das
    /// `Decided`-Ereignis über `queue` veröffentlicht.
    #[must_use]
    pub const fn new(queue: std::sync::Arc<HoldQueue>) -> Self {
        Self { queue }
    }
}

#[async_trait]
impl FlowPipeline for PassthroughPipeline {
    async fn decide(&self, flow: &mut Flow, meta: &ConnMeta) -> Decision {
        self.queue.registry().insert(FlowRecord::new(flow, meta));
        match flow.apply(
            humanitl_core::TransitionInput::Decide {
                decision: Decision::Allow,
                source: DecisionSource::Passthrough,
            },
            SystemTime::now(),
        ) {
            Ok(event) => {
                self.queue.publish(event);
                Decision::Allow
            }
            Err(err) => {
                tracing::error!(flow = %flow.id, %err, "passthrough decision refused; blocking");
                system_block(&self.queue, flow)
            }
        }
    }
}

/// Blockt einen analysierten Flow durch das System (fail-closed).
fn system_block(queue: &HoldQueue, flow: &mut Flow) -> Decision {
    let decision = Decision::Block {
        reason: humanitl_core::BlockReason::NoRoute,
        note: None,
    };
    // Der Flow ist entweder noch `Analyzed` (Hold schlug fehl) — dann ist der
    // Übergang gültig — oder er ist es nicht; im zweiten Fall bleibt er, wie er
    // ist, und der Handler blockt trotzdem.
    if !matches!(flow.state, FlowState::Analyzed { .. }) {
        return decision;
    }
    if let Ok(event) = flow.apply(
        humanitl_core::TransitionInput::Decide {
            decision: decision.clone(),
            source: DecisionSource::System,
        },
        SystemTime::now(),
    ) {
        queue.publish(event);
    }
    decision
}
