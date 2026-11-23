//! Die Leitung zwischen dem Interceptor und dem Daemon (GitHub #12).
//!
//! Wie bei den Einstellungen (`tui::settings::link`): Der Bildschirm ruft
//! keine RPC selbst. Er legt eine [`Request`] in die Leitung und liest
//! später eine [`Answer`] heraus; dazwischen arbeitet im Produkt ein
//! Hintergrundauftrag der Laufzeit ([`super::spawn`]), im Test ein Fake.
//! Was der Test als `Decide` prüft, ist byteweise die Nachricht, die der
//! Daemon bekäme.
//!
//! Die Liste kommt nur aus dem Daemon (ADR-018): der Stand aus `ListFlows`
//! ([`Answer::Listed`]) und danach jedes Ereignis aus `Subscribe`
//! ([`Answer::Arrived`], [`Answer::Gone`]). Der Bildschirm streicht keine
//! Zeile selbst, auch nicht nach der eigenen Entscheidung: Weg ist sie erst,
//! wenn der Daemon `Decided` meldet.

use std::sync::mpsc;

use humanitl_core::Diagnostic;
use humanitl_ipc::v1;

/// Eine Anfrage an den Daemon, als Nachricht der Wire-Form.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// `GetFlow` für den gewählten Fluss: Kopf und Anfang des Rumpfs.
    Detail(String),
    /// `Decide` für genau einen Fluss; geschachtelt, weil die Nachricht
    /// viel größer ist als eine Kennung.
    Decide(Box<v1::DecideRequest>),
}

/// Was vom Daemon kommt.
#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "one answer at a time crosses the link; boxing every variant buys nothing"
)]
pub enum Answer {
    /// Alle gehaltenen Flüsse, ältester zuerst: am Anfang und nach
    /// verworfenen Ereignissen (`Lagged`).
    Listed(Vec<v1::FlowSummary>),
    /// Ein neu gehaltener Fluss (`FlowEvent.held`, aufgelöst über `GetFlow`).
    Arrived(v1::FlowSummary),
    /// Über diesen Fluss ist entschieden, hier oder anderswo, oder seine
    /// Frist ist abgelaufen (`FlowEvent.decided`, `FlowEvent.timed_out`).
    Gone(String),
    /// `GetFlow` für [`Request::Detail`].
    Detail(v1::FlowDetail),
    /// Die Antwort auf [`Request::Decide`]: je Fluss, ob es galt.
    Decided(v1::DecideResponse),
    /// Eine RPC ist gescheitert; der Befund mit `why`.
    Failed(Diagnostic),
    /// Der Ereignisstrom ist zu Ende oder kam nicht zustande: Die Liste
    /// ändert sich nicht mehr von selbst.
    LiveEnded,
}

/// Die Enden der Leitung, die der Bildschirm hält.
pub struct Link {
    /// Hinaus: die Anfragen.
    calls: tokio::sync::mpsc::UnboundedSender<Request>,
    /// Herein: die Antworten und Ereignisse.
    answers: mpsc::Receiver<Answer>,
}

/// Die Enden, die die Gegenseite hält: der Hintergrundauftrag oder der Fake.
pub struct Far {
    /// Die Anfragen des Bildschirms.
    pub calls: tokio::sync::mpsc::UnboundedReceiver<Request>,
    /// Wohin Antworten und Ereignisse gehen.
    pub answers: mpsc::Sender<Answer>,
}

/// Eine neue Leitung.
#[must_use]
pub fn pair() -> (Link, Far) {
    let (call_tx, call_rx) = tokio::sync::mpsc::unbounded_channel();
    let (answer_tx, answer_rx) = mpsc::channel();
    (
        Link {
            calls: call_tx,
            answers: answer_rx,
        },
        Far {
            calls: call_rx,
            answers: answer_tx,
        },
    )
}

impl Link {
    /// Legt eine Anfrage in die Leitung. `false`, wenn die Gegenseite nicht
    /// mehr da ist.
    pub fn send(&self, request: Request) -> bool {
        self.calls.send(request).is_ok()
    }

    /// Die nächste Antwort, ohne zu warten.
    pub fn try_answer(&self) -> Option<Answer> {
        self.answers.try_recv().ok()
    }
}
