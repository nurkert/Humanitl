//! Ob das Modell im Lauf erreichbar ist (HUM-251).
//!
//! Die Probe (`ProbeLlm`) prüft den Endpunkt vor dem Start. Im Lauf fragt der
//! Agent sein Modell über die Durchreiche, und wenn der Server dann nicht mehr
//! antwortet, entsteht ein Fluss im Zustand `Failed` (CONVENTIONS 4.10) — aber
//! bis HUM-251 kein Befund. Die Zeile „LLM“ im Isolations-Reiter blieb neutral,
//! und sie aus den Flüssen abzuleiten wäre Fachlogik in der Oberfläche
//! (ADR-018).
//!
//! Dieses Modul hört deshalb im Daemon den Ereignisstrom der Warteschlange
//! mit und führt genau einen Stand: den Befund `LLM_001` der letzten
//! gescheiterten Durchreiche, oder keinen. Er steht in
//! `SandboxEvent.Status.llm`, und die Oberfläche zeichnet ihn nur.
//!
//! **Was zählt.** Nur ein Fluss, den die Durchreiche freigegeben hat
//! (`DecisionSource::Passthrough`); eine gescheiterte Verbindung zu einem
//! beliebigen anderen Ziel sagt nichts über das Modell. Gesetzt wird der
//! Befund von `Failed` mit `Dns`, `Connect`, `Tls` oder `Timeout`, also genau
//! den Ursachen, die `LLM_001` im Register nennt. `PrivateAddress` setzt ihn
//! nicht: Das ist eine Ablehnung des Proxys und hat mit `PROXY_008` ihren
//! eigenen Befund. Weggenommen wird er von der ersten Antwort einer
//! Durchreiche (`ResponseHeaders`), gleich welchen Status sie trägt: Ein
//! Server, der `500` antwortet, ist erreichbar.
//!
//! **Was nicht.** Der Stand ist kein Verlauf: Ein Server, der abwechselnd
//! antwortet und schweigt, steht immer so da, wie die letzte Durchreiche
//! ausging. Verpasst der Zuhörer Ereignisse (`Lagged`), bleibt der Stand, wie
//! er war, bis die nächste Durchreiche ihn neu entscheidet.

use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex, PoisonError};

use humanitl_core::diagnostics::codes::LLM_001;
use humanitl_core::{
    Decision, DecisionSource, Diagnostic, FixAction, FlowEvent, FlowId, Severity, UpstreamError,
};

/// Wie viele Durchreich-Flüsse sich der Zuhörer zugleich merkt.
///
/// Ein Fluss verlässt die Menge mit `Recorded`. Fehlt dieses Ereignis, weil
/// der Zuhörer zurückfiel, bliebe er für immer darin; die Grenze hält die
/// Menge dann klein. Ist sie voll, fällt der älteste Fluss heraus und nur er:
/// Die Anfragen, die gerade laufen, sind die jüngsten und bleiben gezählt.
pub const PASSTHROUGH_FLOWS_MAX: usize = 1024;

/// Der Stand der Durchreiche eines Laufs.
#[derive(Debug, Default)]
pub struct LlmReach {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// Der Endpunkt des Laufs, wie er in `llm.endpoint` steht.
    endpoint: String,
    /// Die offenen Flüsse, die die Durchreiche freigegeben hat.
    passthrough: HashSet<FlowId>,
    /// Dieselben Flüsse in der Reihenfolge ihrer Freigabe; an der Grenze
    /// [`PASSTHROUGH_FLOWS_MAX`] fällt der älteste heraus.
    order: VecDeque<FlowId>,
    /// `LLM_001`, solange die letzte Durchreiche an der Verbindung scheiterte.
    unreachable: Option<Diagnostic>,
    /// Ob ein Lauf offen ist: zwischen [`LlmReach::begin`] und
    /// [`LlmReach::end`]. Außerhalb davon zählt kein Ereignis. Ein Zuhörer,
    /// dessen Abbruch noch nicht gegriffen hat, liest sonst ein gepuffertes
    /// `Failed` nach dem Ende und setzte `LLM_001` über einen Lauf, der
    /// vorbei ist.
    open: bool,
}

impl LlmReach {
    /// Beginnt einen Lauf gegen `endpoint`: ohne Befund und ohne offene
    /// Flüsse.
    pub fn begin(&self, endpoint: &str) {
        let mut state = self.lock();
        endpoint.clone_into(&mut state.endpoint);
        state.passthrough.clear();
        state.order.clear();
        state.unreachable = None;
        state.open = true;
    }

    /// Beendet den Lauf. Was danach in `Status` steht, gehört keinem Lauf
    /// mehr, und ein Befund aus dem letzten wäre eine Behauptung über einen,
    /// der nicht läuft.
    ///
    /// Zurück kommt, ob dabei ein Befund wegfiel; dann schuldet der Aufrufer
    /// den Clients eine Momentaufnahme ohne ihn.
    pub fn end(&self) -> bool {
        let mut state = self.lock();
        state.passthrough.clear();
        state.order.clear();
        state.open = false;
        state.unreachable.take().is_some()
    }

    /// Der Befund, der gerade gilt, oder `None`.
    #[must_use]
    pub fn current(&self) -> Option<Diagnostic> {
        self.lock().unreachable.clone()
    }

    /// Nimmt ein Ereignis der Warteschlange auf und sagt, ob sich der Befund
    /// damit geändert hat: gesetzt, weggenommen oder mit anderer Ursache.
    ///
    /// Ohne offenen Lauf zählt nichts, und es ändert sich nichts.
    pub fn observe(&self, event: &FlowEvent) -> bool {
        let mut state = self.lock();
        if !state.open {
            return false;
        }
        match event {
            FlowEvent::Decided {
                flow_id,
                decision: Decision::Allow | Decision::AllowEdited { .. },
                source: DecisionSource::Passthrough,
                ..
            } => {
                if state.passthrough.insert(*flow_id) {
                    state.order.push_back(*flow_id);
                }
                while state.passthrough.len() > PASSTHROUGH_FLOWS_MAX {
                    let Some(oldest) = state.order.pop_front() else {
                        break;
                    };
                    state.passthrough.remove(&oldest);
                }
                false
            }
            FlowEvent::Failed { flow_id, error, .. } if state.passthrough.contains(flow_id) => {
                let Some(cause) = unreachable_cause(*error) else {
                    return false;
                };
                let next = unreachable(&state.endpoint, cause);
                let changed = state.unreachable.as_ref() != Some(&next);
                state.unreachable = Some(next);
                changed
            }
            FlowEvent::ResponseHeaders { flow_id, .. } if state.passthrough.contains(flow_id) => {
                state.unreachable.take().is_some()
            }
            FlowEvent::Recorded { flow_id, .. } => {
                if state.passthrough.remove(flow_id) {
                    state.order.retain(|open| open != flow_id);
                }
                false
            }
            _ => false,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Der Halbsatz zu einer Ursache, die `LLM_001` meint; `None` für eine, die
/// es nicht meint.
const fn unreachable_cause(error: UpstreamError) -> Option<&'static str> {
    match error {
        UpstreamError::Dns => Some("its name did not resolve"),
        UpstreamError::Connect => Some("nobody accepted the connection"),
        UpstreamError::Tls => Some("the TLS handshake failed"),
        UpstreamError::Timeout => Some("it did not answer in time"),
        UpstreamError::PrivateAddress(_) => None,
    }
}

/// `LLM_001` für eine Durchreiche, die an `cause` scheiterte.
///
/// Der Vorschlag ist die Einstellung selbst, mit dem Wert, der gerade gilt:
/// Ein Server, der im Lauf verschwindet, ist meist umgezogen oder
/// heruntergefahren, und in beiden Fällen ist `llm.endpoint` der Ort, an dem
/// der Mensch es richtet. Der `curl` der Probe gehört zur Frage vor dem
/// Start; hier hat der Agent die Frage schon gestellt.
fn unreachable(endpoint: &str, cause: &str) -> Diagnostic {
    let target = if endpoint.is_empty() {
        "its model".to_owned()
    } else {
        format!("its model at {endpoint}")
    };
    Diagnostic::builder(LLM_001, Severity::Error)
        .why(format!(
            "the agent's last request to {target} failed: {cause}; until a request gets \
             through again, the agent works without its model"
        ))
        .fix(FixAction::ChangeSetting {
            key: "llm.endpoint".to_owned(),
            value: endpoint.to_owned(),
        })
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::SystemTime;

    use humanitl_core::DecidedFindings;

    use super::*;

    const ENDPOINT: &str = "http://127.0.0.1:11434/v1";

    fn decided(flow_id: FlowId, source: DecisionSource) -> FlowEvent {
        FlowEvent::Decided {
            flow_id,
            at: SystemTime::now(),
            decision: Decision::Allow,
            source,
            findings: DecidedFindings::default(),
        }
    }

    fn failed(flow_id: FlowId, error: UpstreamError) -> FlowEvent {
        FlowEvent::Failed {
            flow_id,
            at: SystemTime::now(),
            error,
        }
    }

    fn answered(flow_id: FlowId, status: u16) -> FlowEvent {
        FlowEvent::ResponseHeaders {
            flow_id,
            at: SystemTime::now(),
            status,
        }
    }

    fn recorded(flow_id: FlowId) -> FlowEvent {
        FlowEvent::Recorded {
            flow_id,
            at: SystemTime::now(),
        }
    }

    #[test]
    fn a_failed_passthrough_sets_llm_001_and_an_answer_takes_it_away() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        let first = FlowId::new();
        assert!(!reach.observe(&decided(first, DecisionSource::Passthrough)));
        assert!(reach.observe(&failed(first, UpstreamError::Connect)));
        let diagnostic = reach.current().expect("LLM_001 is set");
        assert_eq!(diagnostic.code, LLM_001);
        assert!(diagnostic.why.contains(ENDPOINT), "{}", diagnostic.why);
        assert!(diagnostic.why.contains("connection"), "{}", diagnostic.why);
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::ChangeSetting {
                key: "llm.endpoint".to_owned(),
                value: ENDPOINT.to_owned(),
            })
        );
        assert!(!reach.observe(&recorded(first)));

        // Der Server ist zurück. Auch ein `500` ist eine Antwort.
        let second = FlowId::new();
        reach.observe(&decided(second, DecisionSource::Passthrough));
        assert!(reach.observe(&answered(second, 500)));
        assert_eq!(reach.current(), None);
    }

    #[test]
    fn every_cause_the_register_names_counts_and_a_private_address_does_not() {
        for error in [
            UpstreamError::Dns,
            UpstreamError::Connect,
            UpstreamError::Tls,
            UpstreamError::Timeout,
        ] {
            let reach = LlmReach::default();
            reach.begin(ENDPOINT);
            let flow = FlowId::new();
            reach.observe(&decided(flow, DecisionSource::Passthrough));
            assert!(reach.observe(&failed(flow, error)), "{error:?}");
        }
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        let flow = FlowId::new();
        reach.observe(&decided(flow, DecisionSource::Passthrough));
        let ip = "10.0.0.1".parse().unwrap();
        assert!(!reach.observe(&failed(flow, UpstreamError::PrivateAddress(ip))));
        assert_eq!(reach.current(), None);
    }

    /// Eine gescheiterte Verbindung zu einem Ziel, das eine Regel oder ein
    /// Mensch freigab, sagt nichts über das Modell; und die Antwort eines
    /// solchen Ziels heilt keinen Befund über das Modell.
    #[test]
    fn only_a_flow_the_passthrough_allowed_counts() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        let other = FlowId::new();
        reach.observe(&decided(
            other,
            DecisionSource::Rule(humanitl_core::RuleId::new()),
        ));
        assert!(!reach.observe(&failed(other, UpstreamError::Connect)));
        assert_eq!(reach.current(), None);

        let model = FlowId::new();
        reach.observe(&decided(model, DecisionSource::Passthrough));
        assert!(reach.observe(&failed(model, UpstreamError::Connect)));
        let unrelated = FlowId::new();
        reach.observe(&decided(unrelated, DecisionSource::User));
        assert!(!reach.observe(&answered(unrelated, 200)));
        assert!(
            reach.current().is_some(),
            "another host answering heals nothing"
        );
    }

    #[test]
    fn a_new_run_and_the_end_of_a_run_start_without_the_finding() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        let flow = FlowId::new();
        reach.observe(&decided(flow, DecisionSource::Passthrough));
        reach.observe(&failed(flow, UpstreamError::Timeout));
        assert!(reach.current().is_some());
        reach.end();
        assert_eq!(reach.current(), None);

        reach.begin(ENDPOINT);
        reach.observe(&decided(flow, DecisionSource::Passthrough));
        reach.begin(ENDPOINT);
        assert!(
            !reach.observe(&failed(flow, UpstreamError::Connect)),
            "a flow of the last run is not one of this run"
        );
    }

    /// An der Grenze fällt nur der älteste Fluss heraus; die jüngeren, die
    /// gerade laufen, zählen weiter.
    #[test]
    fn at_the_cap_only_the_oldest_flow_is_forgotten() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        let flows: Vec<FlowId> = (0..=PASSTHROUGH_FLOWS_MAX).map(|_| FlowId::new()).collect();
        for flow in &flows {
            reach.observe(&decided(*flow, DecisionSource::Passthrough));
        }
        assert!(
            !reach.observe(&failed(flows[0], UpstreamError::Connect)),
            "the oldest flow fell out"
        );
        assert!(
            reach.observe(&failed(flows[1], UpstreamError::Connect)),
            "the second oldest still counts"
        );
        assert!(reach.observe(&answered(flows[PASSTHROUGH_FLOWS_MAX], 200)));
    }

    /// Nach dem Ende zählt kein Ereignis mehr, auch nicht eines, das ein
    /// Zuhörer noch gepuffert hatte; erst der nächste Lauf öffnet wieder.
    #[test]
    fn after_the_end_nothing_counts_until_the_next_run() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        let flow = FlowId::new();
        reach.end();
        assert!(!reach.observe(&decided(flow, DecisionSource::Passthrough)));
        assert!(!reach.observe(&failed(flow, UpstreamError::Connect)));
        assert_eq!(reach.current(), None, "the ended run stays clean");

        reach.begin(ENDPOINT);
        reach.observe(&decided(flow, DecisionSource::Passthrough));
        assert!(reach.observe(&failed(flow, UpstreamError::Connect)));
    }

    /// `end` sagt, ob ein Befund wegfiel; nur dann braucht es eine neue
    /// Momentaufnahme.
    #[test]
    fn end_says_whether_a_finding_was_cleared() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        assert!(!reach.end(), "nothing to clear");
        reach.begin(ENDPOINT);
        let flow = FlowId::new();
        reach.observe(&decided(flow, DecisionSource::Passthrough));
        reach.observe(&failed(flow, UpstreamError::Connect));
        assert!(reach.end());
        assert_eq!(reach.current(), None);
    }

    #[test]
    fn the_same_cause_twice_is_no_change() {
        let reach = LlmReach::default();
        reach.begin(ENDPOINT);
        for expected in [true, false] {
            let flow = FlowId::new();
            reach.observe(&decided(flow, DecisionSource::Passthrough));
            assert_eq!(
                reach.observe(&failed(flow, UpstreamError::Connect)),
                expected
            );
        }
        let flow = FlowId::new();
        reach.observe(&decided(flow, DecisionSource::Passthrough));
        assert!(
            reach.observe(&failed(flow, UpstreamError::Dns)),
            "another cause is a change"
        );
    }
}
