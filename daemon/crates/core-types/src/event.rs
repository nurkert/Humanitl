//! Der Ereignisstrom eines Flows.
//!
//! Jede Zustandsänderung eines Flows wird zu genau einem [`FlowEvent`]. Der
//! Strom ist das Rückgrat der Anwendung: Oberfläche, Recorder, Audit und
//! später Plugins sind Zuhörer, niemand fragt den Proxy nach seinem Zustand
//! (siehe `docs/ARCHITECTURE.md` 1).
//!
//! Zwei Varianten entstehen nicht im Automaten: [`FlowEvent::ResponseChunk`]
//! schreibt der Proxy beim Durchreichen der Antwort, [`FlowEvent::Lagged`]
//! meldet die IPC-Schicht, wenn ein Zuhörer zu langsam war.

use std::time::{Instant, SystemTime};

use crate::finding::Finding;
use crate::flow::{Decision, DecisionSource, UpstreamError};
use crate::http::HttpRequest;
use crate::ids::FlowId;

/// Ein Ereignis aus dem Leben eines Flows.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowEvent {
    /// Die Anfrage ist angekommen.
    Received {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Die Anfrage; geboxt, damit die Variante nicht alle anderen aufbläht.
        request: Box<HttpRequest>,
    },
    /// Die Detektoren sind gelaufen.
    Analyzed {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Was gefunden wurde.
        findings: Vec<Finding>,
    },
    /// Die Anfrage wartet auf eine Entscheidung.
    Held {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Bis wann gewartet wird.
        deadline: Instant,
        /// Wie viele Bytes gerade insgesamt gehalten werden.
        queue_bytes: u64,
        /// Wie viele Flows gerade insgesamt gehalten werden.
        queue_count: u32,
    },
    /// Es ist entschieden.
    Decided {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Die Entscheidung.
        decision: Decision,
        /// Wer entschieden hat.
        source: DecisionSource,
    },
    /// Die Anfrage ist auf dem Weg zum Ziel.
    Forwarded {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
    },
    /// Die Antwortkopfzeilen sind da.
    ResponseHeaders {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Der Status der Antwort.
        status: u16,
    },
    /// Ein Stück Antwortkörper ist durchgelaufen; kein Zustandswechsel.
    ResponseChunk {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Länge des Stücks in Bytes.
        len: u64,
    },
    /// Die Verbindung zum Ziel ist gescheitert.
    Failed {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
        /// Woran.
        error: UpstreamError,
    },
    /// Die Wartezeit ist abgelaufen.
    TimedOut {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
    },
    /// Der Flow ist abgeschlossen und aufgezeichnet.
    Recorded {
        /// Der Flow.
        flow_id: FlowId,
        /// Wann.
        at: SystemTime,
    },
    /// Der Zuhörer hat Ereignisse verpasst; kein Zustandswechsel.
    Lagged {
        /// Wie viele Ereignisse verloren gingen.
        n: u64,
    },
}

impl FlowEvent {
    /// Kurzname der Variante in `snake_case`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Received { .. } => "received",
            Self::Analyzed { .. } => "analyzed",
            Self::Held { .. } => "held",
            Self::Decided { .. } => "decided",
            Self::Forwarded { .. } => "forwarded",
            Self::ResponseHeaders { .. } => "response_headers",
            Self::ResponseChunk { .. } => "response_chunk",
            Self::Failed { .. } => "failed",
            Self::TimedOut { .. } => "timed_out",
            Self::Recorded { .. } => "recorded",
            Self::Lagged { .. } => "lagged",
        }
    }

    /// Der Flow, zu dem das Ereignis gehört. `None` nur bei [`FlowEvent::Lagged`].
    #[must_use]
    pub const fn flow_id(&self) -> Option<FlowId> {
        match self {
            Self::Received { flow_id, .. }
            | Self::Analyzed { flow_id, .. }
            | Self::Held { flow_id, .. }
            | Self::Decided { flow_id, .. }
            | Self::Forwarded { flow_id, .. }
            | Self::ResponseHeaders { flow_id, .. }
            | Self::ResponseChunk { flow_id, .. }
            | Self::Failed { flow_id, .. }
            | Self::TimedOut { flow_id, .. }
            | Self::Recorded { flow_id, .. } => Some(*flow_id),
            Self::Lagged { .. } => None,
        }
    }

    /// Wann das Ereignis entstand. `None` nur bei [`FlowEvent::Lagged`].
    #[must_use]
    pub const fn at(&self) -> Option<SystemTime> {
        match self {
            Self::Received { at, .. }
            | Self::Analyzed { at, .. }
            | Self::Held { at, .. }
            | Self::Decided { at, .. }
            | Self::Forwarded { at, .. }
            | Self::ResponseHeaders { at, .. }
            | Self::ResponseChunk { at, .. }
            | Self::Failed { at, .. }
            | Self::TimedOut { at, .. }
            | Self::Recorded { at, .. } => Some(*at),
            Self::Lagged { .. } => None,
        }
    }
}
