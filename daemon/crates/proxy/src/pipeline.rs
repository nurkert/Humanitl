//! Die Entscheidungsstufe: aus einem analysierten Flow wird eine
//! [`Decision`].
//!
//! Der Handler treibt den Zustandsautomaten (`docs/ARCHITECTURE.md` 3); die
//! Pipeline ist nur die Strategie, die entscheidet, was mit einem Flow im
//! Zustand [`FlowState::Analyzed`]
//! geschieht: halten und auf einen Menschen warten, oder sofort automatisch
//! entscheiden.
//!
//! Es gibt drei Strategien:
//!
//! - [`RulesPipeline`] wertet den Regelsatz aus (ADR-007, HUM-022) und
//!   entscheidet, was eine Regel entscheidet: `allow` und `block` sofort, mit
//!   [`DecisionSource::Rule`]. Alles andere — `ask`, `redact` und der Fall
//!   ohne passende Regel — reicht sie an die innere Strategie weiter. Sie ist
//!   der Normalfall im Daemon.
//! - [`AskPipeline`] hält jeden Flow über die [`HoldQueue`], bis jemand
//!   entscheidet oder die Frist abläuft.
//! - [`PassthroughPipeline`] lässt jeden Flow sofort durch
//!   ([`DecisionSource::Passthrough`]). Sie ist der Test-Hook, mit dem sich der
//!   Weiterleitungs-Pfad ohne Oberfläche prüfen lässt, und der Platzhalter für
//!   die spätere LLM-Passthrough-Regel.
//!
//! Der Scan der Detektoren liegt davor, im Handler: Er steht in
//! `FlowEvent::Analyzed`, bevor eine Regel greift (`backlog/sprint-2.md`
//! HUM-023, Schritt 4).
//!
//! Den Eintrag in die [`FlowRegistry`](crate::registry::FlowRegistry) macht
//! der Handler, bevor er hierher kommt: Er allein kennt den Bericht des Scans
//! und trägt `findings_truncated` ein. Danach schreibt sich der Datensatz von
//! allein fort, weil jedes Ereignis durch [`HoldQueue::publish`] läuft: `Held`
//! mit der Frist, `Decided` oder `TimedOut` mit der Entscheidung, `Forwarded`,
//! `ResponseHeaders` und `Recorded` aus dem Handler.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use humanitl_core::rule::Action;
use humanitl_core::{BlockReason, Decision, DecisionSource, Flow, FlowState};
use humanitl_rules::{RequestKey, RuleSet, Verdict};

use crate::connect::requested_upgrade;
use crate::hold::HoldQueue;

pub use crate::connect::{ConnMeta, ConnectionContext};

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
    queue: Arc<HoldQueue>,
    timeout: Duration,
}

impl AskPipeline {
    /// Eine Pipeline, die über `queue` hält und `timeout` als Frist setzt.
    ///
    /// `timeout` ist `hold.timeout_secs`; `0` (oder `ask_mode = none`) blockt
    /// alles, weil die Frist sofort abläuft.
    #[must_use]
    pub const fn new(queue: Arc<HoldQueue>, timeout: Duration) -> Self {
        Self { queue, timeout }
    }
}

#[async_trait]
impl FlowPipeline for AskPipeline {
    /// Die Schritte aus HUM-016: Frist setzen und halten, warten, den Ausgang
    /// verbuchen, zurückgeben.
    ///
    /// `Received`, `Analyzed` und der Eintrag in die Registry sind schon
    /// geschehen (der Handler, HUM-023); der Datensatz beginnt deshalb im
    /// Zustand des übergebenen Flows. `Held` und `Decided` gehen durch die
    /// Warteschlange und damit durch [`HoldQueue::publish`], das die Registry
    /// mitführt; `Recorded` hängt der Handler an, nachdem die Block-Antwort
    /// steht.
    async fn decide(&self, flow: &mut Flow, _meta: &ConnMeta) -> Decision {
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
    queue: Arc<HoldQueue>,
}

impl PassthroughPipeline {
    /// Eine Pipeline, die jeden Flow ohne Warten erlaubt und das
    /// `Decided`-Ereignis über `queue` veröffentlicht.
    #[must_use]
    pub const fn new(queue: Arc<HoldQueue>) -> Self {
        Self { queue }
    }
}

#[async_trait]
impl FlowPipeline for PassthroughPipeline {
    async fn decide(&self, flow: &mut Flow, _meta: &ConnMeta) -> Decision {
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

/// Wertet den Regelsatz aus und hält nur, was `ask` ergibt.
///
/// Die Reihenfolge des Proxy-Pfads steht in `backlog/sprint-2.md` HUM-023:
/// erst die Konsistenz von CONNECT-Ziel, SNI und Authority (im Handler, vor
/// jeder Regel), dann die Detektoren, dann diese Auswertung, dann der Hold.
/// Ausgewertet wird ausschließlich [`HttpRequest::authority`](humanitl_core::HttpRequest);
/// das CONNECT-Ziel allein sieht die Regel-Engine nie, sonst wäre ein Allow
/// für den Tunnel ein Allow für jeden Host darin.
///
/// Zuordnung von [`Verdict`] zu [`Decision`] (ADR-007):
///
/// | Verdict | Ergebnis |
/// | --- | --- |
/// | `Matched { Allow }` | `Decision::Allow`, Quelle [`DecisionSource::Rule`] |
/// | `Matched { Block }` | `Decision::Block { BlockReason::Rule }`, `403` |
/// | `Matched { Redact }` | wie `Default`; der Pseudonymisierer kommt in HUM-039 |
/// | `Matched { Ask }`, `Default` | die innere Strategie, also der Hold |
pub struct RulesPipeline {
    queue: Arc<HoldQueue>,
    rules: Arc<RwLock<RuleSet>>,
    inner: Arc<dyn FlowPipeline>,
}

impl std::fmt::Debug for RulesPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RulesPipeline").finish_non_exhaustive()
    }
}

impl RulesPipeline {
    /// Eine Pipeline, die `rules` auswertet und alles Übrige an `inner`
    /// weitergibt (im Daemon die [`AskPipeline`]).
    #[must_use]
    pub const fn new(
        queue: Arc<HoldQueue>,
        rules: Arc<RwLock<RuleSet>>,
        inner: Arc<dyn FlowPipeline>,
    ) -> Self {
        Self {
            queue,
            rules,
            inner,
        }
    }

    /// Der Regelsatz, den diese Pipeline liest; HUM-027 tauscht ihn zur
    /// Laufzeit aus.
    #[must_use]
    pub fn rules(&self) -> &Arc<RwLock<RuleSet>> {
        &self.rules
    }

    /// Wertet den Regelsatz für diesen Flow aus.
    fn evaluate(&self, flow: &Flow) -> Verdict {
        let request = &flow.request;
        let mut key = RequestKey::new(
            &request.authority.host,
            &request.method,
            &request.path_and_query,
            request.scheme,
            request.authority.port,
        );
        if let Some(upgrade) = requested_upgrade(&request.headers) {
            key = key.with_upgrade(upgrade);
        }
        // Ein vergifteter Regelsatz (Lock durch eine Panik in einem anderen
        // Task zerbrochen) darf nicht zu einer stillen Freigabe führen, aber
        // auch nicht den Proxy anhalten: Die Regeln werden nur gelesen, und
        // wer sie nicht lesen kann, fragt.
        let Ok(rules) = self.rules.read() else {
            tracing::error!(flow = %flow.id, "the rule set is poisoned; asking instead of matching");
            return Verdict::Default;
        };
        rules.evaluate(&key, chrono::Utc::now(), flow.session)
    }

    /// Wendet eine Entscheidung an, die eine Regel getroffen hat.
    fn decide_by_rule(
        &self,
        flow: &mut Flow,
        decision: Decision,
        source: DecisionSource,
    ) -> Decision {
        match flow.apply(
            humanitl_core::TransitionInput::Decide {
                decision: decision.clone(),
                source,
            },
            SystemTime::now(),
        ) {
            Ok(event) => {
                self.queue.publish(event);
                decision
            }
            Err(err) => {
                tracing::error!(flow = %flow.id, %err, "rule decision refused; blocking");
                system_block(&self.queue, flow)
            }
        }
    }
}

#[async_trait]
impl FlowPipeline for RulesPipeline {
    async fn decide(&self, flow: &mut Flow, meta: &ConnectionContext) -> Decision {
        let verdict = self.evaluate(flow);
        let Verdict::Matched { rule, action } = verdict else {
            return self.inner.decide(flow, meta).await;
        };
        match action {
            Action::Allow => self.decide_by_rule(flow, Decision::Allow, DecisionSource::Rule(rule)),
            Action::Block => self.decide_by_rule(
                flow,
                Decision::Block {
                    reason: BlockReason::Rule(rule),
                    note: None,
                },
                DecisionSource::Rule(rule),
            ),
            // `redact` heißt in M1: fragen. Der Pseudonymisierer, der die
            // Funde vor dem Weiterleiten ersetzt, kommt in HUM-039; bis dahin
            // wäre ein stilles Durchlassen eine Freigabe, die niemand gegeben
            // hat.
            Action::Redact => {
                tracing::debug!(
                    flow = %flow.id,
                    %rule,
                    "rule action redact is treated as ask until HUM-039"
                );
                self.inner.decide(flow, meta).await
            }
            Action::Ask => self.inner.decide(flow, meta).await,
        }
    }
}
