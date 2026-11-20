//! Was ein Sandbox-Backend während einer Sitzung meldet (HUM-477).
//!
//! Ein Backend sieht Dinge, die sonst nur in seinem Beleg stünden: dass die
//! Tür einer Sitzung eine Verbindung über ihrer Obergrenze abwies (HUM-467),
//! und was die Prüfung der Isolation auf dem Host ergab. Damit das nicht nur
//! im Beleg einer späteren Prüfung steht, meldet das Backend es über einen
//! [`SandboxNoticeSink`]. Der Daemon gibt ihm einen, der jede Meldung direkt
//! ins Audit-Log schreibt und danach als [`FlowEvent::Sandbox`] in den
//! Ereignisstrom der Sitzung legt. Direkt, weil ein Zuhörer am begrenzten
//! Strom zurückfallen kann und der Vermerk der Tür nur einmal kommt. Ohne
//! Sink meldet das Backend nichts und arbeitet wie zuvor.
//!
//! Die Meldung nennt keinen Pfad, keinen Namen eines Backends und keinen
//! Text aus dem Gast: Was im Audit-Log steht, sind Zahlen, die Namen der
//! Garantien und ihr Ergebnis.
//!
//! Die Wörter liegen im Kern und nicht im Sandbox-Crate, aus demselben Grund
//! wie [`crate::InterceptPause`]: Der Ereignisstrom ist ein Typ des Kerns,
//! und Daemon und gRPC-Dienst sollen den Sink halten können, ohne von einem
//! Adapter abzuhängen.

use std::sync::Arc;
use std::time::SystemTime;

use crate::event::FlowEvent;

/// Eine Meldung eines Sandbox-Backends über die laufende Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxNotice {
    /// Die Tür der Sitzung hat eine Verbindung des Gasts über ihrer
    /// Obergrenze sofort geschlossen (HUM-467). Gemeldet nur die erste je
    /// Start der Sandbox; die Verbindung erreichte den Proxy nicht.
    DoorCapped {
        /// Wie viele offene Verbindungen die Tür einer Sitzung höchstens
        /// hält.
        cap: u32,
    },
    /// Eine Prüfung der Isolation ist durchgelaufen, einschließlich der
    /// Prüfung auf dem Host, wo das Backend eine hat (die Kernel-Stufe,
    /// HUM-444). Gemeldet bei jeder Prüfung, mit dem Ergebnis, das auch der
    /// Aufrufer bekommt.
    IsolationChecked {
        /// Jede Garantie mit ihrem Ergebnis, in der Reihenfolge der Prüfung.
        results: Vec<CheckedGuarantee>,
    },
    /// Der Wächter des Agent-Depots hat die Zugangsdatei einer Sitzung oder
    /// eines Anmeldelaufs übernommen oder abgelehnt (HUM-520). Nie der
    /// Inhalt, nur die SHA-256.
    CredentialWriteback {
        /// Der Agent, wie in `agents.<id>`.
        agent: String,
        /// Ob die Datei jetzt im Depot steht.
        accepted: bool,
        /// Der Code einer Ablehnung (`AGENT_012`, `AGENT_016`).
        code: Option<String>,
        /// SHA-256 der gespeicherten Datei davor.
        sha256_old: Option<String>,
        /// SHA-256 der angebotenen Datei.
        sha256_new: Option<String>,
    },
}

/// Eine Garantie in [`SandboxNotice::IsolationChecked`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGuarantee {
    /// Der Name der Garantie in `snake_case`, etwa `single_socket`.
    pub check: String,
    /// Ob sie hielt.
    pub passed: bool,
}

/// Wohin eine Meldung geht: im Daemon in die Halte-Warteschlange und damit in
/// den Ereignisstrom.
type Announce = Arc<dyn Fn(FlowEvent) + Send + Sync>;

/// Der Kanal vom Backend in den Ereignisstrom der Sitzung (HUM-477).
///
/// Billig zu klonen; alle Klone melden an dieselbe Stelle. Die Funktion kann
/// auf jedem Faden laufen, auch auf einem des Backends, und darf deshalb
/// nicht blockieren.
#[derive(Clone)]
pub struct SandboxNoticeSink {
    announce: Announce,
}

impl std::fmt::Debug for SandboxNoticeSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxNoticeSink").finish_non_exhaustive()
    }
}

impl SandboxNoticeSink {
    /// Ein Sink, der jede Meldung als [`FlowEvent::Sandbox`] mit dem
    /// Zeitpunkt der Meldung an `announce` gibt.
    #[must_use]
    pub fn announcing(announce: impl Fn(FlowEvent) + Send + Sync + 'static) -> Self {
        Self {
            announce: Arc::new(announce),
        }
    }

    /// Meldet `notice`, jetzt.
    pub fn report(&self, notice: SandboxNotice) {
        (self.announce)(FlowEvent::Sandbox {
            at: SystemTime::now(),
            notice,
        });
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::{Arc, Mutex};

    use super::{SandboxNotice, SandboxNoticeSink};
    use crate::event::FlowEvent;

    #[test]
    fn a_notice_reaches_the_stream_as_a_sandbox_event() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let seen = Arc::clone(&seen);
            SandboxNoticeSink::announcing(move |event| seen.lock().unwrap().push(event))
        };
        sink.clone().report(SandboxNotice::DoorCapped { cap: 64 });
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        let FlowEvent::Sandbox { notice, .. } = &seen[0] else {
            panic!("expected a sandbox event, got {:?}", seen[0]);
        };
        assert_eq!(*notice, SandboxNotice::DoorCapped { cap: 64 });
        assert_eq!(seen[0].name(), "sandbox");
        assert_eq!(seen[0].flow_id(), None);
        assert!(seen[0].at().is_some());
    }
}
