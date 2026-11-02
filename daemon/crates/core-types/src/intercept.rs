//! Die Pause des Abfangens (HUM-407).
//!
//! Solange sie gilt, wird nichts gehalten: Was keine Regel entscheidet, geht
//! mit der Herkunft [`DecisionSource::Paused`](crate::DecisionSource::Paused)
//! durch, statt auf einen Menschen zu warten. Block-Regeln, die harte Sperre
//! geprüfter Geheimnisse (HUM-049), die Grenzen der Sandbox und die
//! Aufzeichnung bleiben. Setzen kann sie nur ein Mensch über den Socket des
//! Daemons; sie endet beim Einschalten, beim Start eines neuen Laufs und mit
//! dem Daemon. Keine Einstellung macht sie dauerhaft.
//!
//! Hier stehen die Wörter, in denen Daemon, Audit und Oberfläche über einen
//! Wechsel sprechen, und [`InterceptPause`], der eine Ort, an dem die Pause
//! gilt. Er liegt im Kern und nicht im Proxy, damit gRPC-Dienst und Daemon
//! ihn halten können, ohne vom Proxy-Crate abzuhängen
//! (`tools/check_coupling.py`); den Ereignisstrom bekommt er als Funktion.
//!
//! **Wer sie setzt.** Nur der gRPC-Dienst des Daemons hat diesen Wert zum
//! Schreiben in der Hand, und der lauscht ausschließlich auf seinem Socket
//! (Rechte `0600`, mit Token). Der Proxy liest ihn an genau der Stelle, an der
//! er sonst halten würde (`humanitl_proxy::pipeline::AskPipeline`); der
//! Meta-Endpunkt `humanitl.internal` und alles, was aus der Sandbox kommt,
//! haben keinen Weg zu ihm.
//!
//! **Wie lange sie gilt.** Der Wert lebt nur im Speicher. Er endet beim
//! Einschalten, beim Start eines neuen Laufs
//! (`humanitl_proxy::session::SessionSettings::set`) und mit dem Prozess;
//! keine Einstellung und keine Datei hält ihn fest.
//!
//! **Was der Agent davon liest.** Kein Text nennt die Pause: Eine
//! durchgelassene Anfrage bekommt die Antwort ihres Ziels, ohne Kopfzeile und
//! ohne Vermerk, und keine Antwort des Meta-Endpunkts nennt die Pause oder
//! einen Menschen (HUM-364, HUM-401). Verborgen ist sie damit nicht: Dass
//! nichts mehr wartet, merkt ein Agent an der Zeit.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use crate::event::FlowEvent;

/// Warum eine Pause endete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResumeReason {
    /// Ein Mensch hat das Abfangen über den Socket wieder eingeschaltet.
    Requested,
    /// Ein neuer Lauf hat begonnen; die Pause gilt nie über ihn hinaus.
    SessionStarted,
    /// Der Daemon endet; mit ihm endet die Pause.
    DaemonStopping,
}

impl ResumeReason {
    /// Kurzname in `snake_case`, so wie er im Audit-Log steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::SessionStarted => "session_started",
            Self::DaemonStopping => "daemon_stopping",
        }
    }
}

/// Ein Wechsel des Abfangens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterceptChange {
    /// Die Pause beginnt.
    Paused,
    /// Die Pause endet, aus diesem Grund.
    Resumed(ResumeReason),
}

impl InterceptChange {
    /// Ob nach diesem Wechsel pausiert ist.
    #[must_use]
    pub const fn is_paused(self) -> bool {
        matches!(self, Self::Paused)
    }
}

/// Der Stand des Abfangens, wie ihn ein Client liest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InterceptStatus {
    /// Seit wann pausiert ist; `None`, solange abgefangen wird.
    pub paused_since: Option<SystemTime>,
    /// Wie viele Wechsel dieser Daemon schon gesehen hat (Review HUM-407).
    ///
    /// Steigt mit jedem Wechsel um eins und nie sonst. Ein Client wendet einen
    /// Stand nur an, wenn seine Revision mindestens so groß ist wie die
    /// größte, die er von diesem Daemon kennt; Antworten und Ereignisse, die
    /// in vertauschter Reihenfolge ankommen, können so keinen neueren Stand
    /// überschreiben. Ein neuer Daemon beginnt wieder bei null.
    pub revision: u64,
}

impl InterceptStatus {
    /// Ob gerade pausiert ist.
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused_since.is_some()
    }
}

/// Wohin ein Wechsel geht: im Daemon in die Halte-Warteschlange und damit in
/// den Ereignisstrom, zu Oberfläche, Aufzeichnung und Audit.
type Announce = Arc<dyn Fn(FlowEvent) + Send + Sync>;

/// Ob das Abfangen gerade pausiert ist, und seit wann.
///
/// Jeder Wechsel geht als [`FlowEvent::Intercept`] an die Funktion aus
/// [`InterceptPause::announcing`]. Ein Aufruf, der nichts ändert (Pause
/// während der Pause), meldet nichts.
#[derive(Default)]
pub struct InterceptPause {
    state: Mutex<InterceptStatus>,
    announce: Option<Announce>,
}

impl std::fmt::Debug for InterceptPause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterceptPause")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl InterceptPause {
    /// Eine Pause, die niemandem von ihren Wechseln erzählt; für Tests und
    /// Aufrufer ohne Ereignisstrom.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Eine Pause, die jeden Wechsel an `announce` gibt.
    ///
    /// Die Funktion läuft unter dem Schloss der Pause: Zwei Wechsel kurz
    /// hintereinander kommen in der Reihenfolge an, in der sie gelten. Sie
    /// darf deshalb die Pause nicht selbst anfassen.
    #[must_use]
    pub fn announcing(announce: impl Fn(FlowEvent) + Send + Sync + 'static) -> Self {
        Self {
            state: Mutex::new(InterceptStatus::default()),
            announce: Some(Arc::new(announce)),
        }
    }

    /// Der Stand, der gerade gilt.
    #[must_use]
    pub fn status(&self) -> InterceptStatus {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ob gerade pausiert ist.
    ///
    /// Eine Momentaufnahme: Wer sie kurz vor [`InterceptPause::resume`]
    /// liest, entscheidet noch nach der Pause. Das betrifft höchstens die
    /// Anfragen, die in diesem Augenblick unterwegs sind; sie stehen dann
    /// nach `intercept.resumed` mit `paused` im Audit, und so ist es gewollt.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.status().is_paused()
    }

    /// Pausiert das Abfangen; eine laufende Pause behält ihren Beginn.
    pub fn pause(&self) -> InterceptStatus {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.paused_since.is_none() {
            let at = SystemTime::now();
            state.paused_since = Some(at);
            state.revision += 1;
            self.tell(at, InterceptChange::Paused, state.revision);
        }
        *state
    }

    /// Beendet die Pause; ohne Pause ändert sich nichts.
    pub fn resume(&self, reason: ResumeReason) -> InterceptStatus {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.paused_since.take().is_some() {
            state.revision += 1;
            self.tell(
                SystemTime::now(),
                InterceptChange::Resumed(reason),
                state.revision,
            );
        }
        *state
    }

    fn tell(&self, at: SystemTime, change: InterceptChange, revision: u64) {
        if let Some(announce) = &self.announce {
            announce(FlowEvent::Intercept {
                at,
                change,
                revision,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::{Arc, Mutex};

    use super::{InterceptChange, InterceptPause, ResumeReason};
    use crate::event::FlowEvent;

    #[test]
    fn a_pause_starts_and_ends_and_keeps_its_beginning() {
        let pause = InterceptPause::new();
        assert!(!pause.is_paused());
        let first = pause.pause();
        assert!(first.is_paused());
        assert_eq!(pause.pause(), first, "a second pause keeps the first start");
        assert!(!pause.resume(ResumeReason::Requested).is_paused());
        assert!(!pause.is_paused());
    }

    /// Jeder Wechsel hebt die Revision um genau eins, ein Aufruf ohne
    /// Wechsel nie, und das Ereignis trägt dieselbe Revision wie der Stand
    /// danach (Review HUM-407).
    #[test]
    fn every_transition_bumps_the_revision_and_nothing_else_does() {
        let seen: Arc<Mutex<Vec<u64>>> = Arc::default();
        let sink = Arc::clone(&seen);
        let pause = InterceptPause::announcing(move |event| {
            if let FlowEvent::Intercept { revision, .. } = event {
                sink.lock().unwrap().push(revision);
            }
        });
        assert_eq!(pause.status().revision, 0);
        assert_eq!(pause.resume(ResumeReason::Requested).revision, 0);
        assert_eq!(pause.pause().revision, 1);
        assert_eq!(pause.pause().revision, 1, "a pause during a pause");
        assert_eq!(pause.resume(ResumeReason::SessionStarted).revision, 2);
        assert_eq!(pause.pause().revision, 3);
        assert_eq!(pause.resume(ResumeReason::DaemonStopping).revision, 4);
        assert_eq!(pause.pause().revision, 5);
        assert_eq!(pause.resume(ResumeReason::Requested).revision, 6);
        assert_eq!(pause.status().revision, 6);
        assert_eq!(*seen.lock().unwrap(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn every_change_and_only_a_change_is_announced() {
        let seen: Arc<Mutex<Vec<InterceptChange>>> = Arc::default();
        let sink = Arc::clone(&seen);
        let pause = InterceptPause::announcing(move |event| {
            if let FlowEvent::Intercept { change, .. } = event {
                sink.lock().unwrap().push(change);
            }
        });

        pause.resume(ResumeReason::Requested);
        pause.pause();
        pause.pause();
        pause.resume(ResumeReason::SessionStarted);
        pause.resume(ResumeReason::Requested);

        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                InterceptChange::Paused,
                InterceptChange::Resumed(ResumeReason::SessionStarted),
            ]
        );
    }
}
