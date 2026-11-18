//! Die Seite der Sandbox beim Aufräumen (HUM-533, ADR-0024 Punkt 14).
//!
//! Was läuft und was wartet, weiß nur dieser Dienst: die laufende Sitzung
//! und die obere Schicht einer Wegwerf-Sandbox, deren Änderungen auf eine
//! Entscheidung warten. Ein Aufräumplan nennt beides in
//! [`humanitl_recorder::LiveState`]; die Ausführung hält die Sperre der
//! Schicht, solange die Aufzeichnung löscht, damit zwischen Prüfung des
//! Digests und Verwerfen der Schicht keine andere Entscheidung über sie
//! fällt (`apply`, `discard`).

use humanitl_core::Diagnostic;
use humanitl_recorder::{LiveState, WaitingLayer};

use super::{Inner, SandboxService, lock};

impl Inner {
    /// Der Stand für einen Plan, solange `slot` gehalten wird.
    fn live_with(&self, waiting: Option<&crate::disposable::Waiting>) -> LiveState {
        let project = *lock(&self.project);
        let running = self.is_running();
        LiveState {
            running_session: running.then_some(self.session),
            running_project: project.filter(|_| running),
            waiting: waiting.zip(project).map(|(waiting, project)| {
                let (count, bytes, complete) = waiting.footprint();
                WaitingLayer {
                    project,
                    layer: waiting.sandbox.to_string(),
                    count,
                    bytes,
                    complete,
                }
            }),
        }
    }
}

impl SandboxService {
    /// Was gerade läuft und wartet, für einen Aufräumplan. Blockiert: liest
    /// die obere Schicht.
    #[must_use]
    pub fn cleanup_live(&self) -> LiveState {
        let slot = lock(&self.inner.disposable);
        self.inner.live_with(slot.as_ref())
    }

    /// Führt `work` aus, während die wartende Schicht gesperrt ist, und
    /// verwirft danach die Schicht, deren Kennung `work` zurückgibt, aber nur,
    /// wenn es noch dieselbe ist und ihre Sandbox beendet ist. Blockiert.
    ///
    /// Die zweite Hälfte der Antwort ist `None`, wenn nichts zu verwerfen
    /// war, sonst ob es gelang.
    pub fn with_cleanup_lock<T>(
        &self,
        work: impl FnOnce(&LiveState) -> (T, Option<String>),
    ) -> (T, Option<Result<(), Diagnostic>>) {
        let mut slot = lock(&self.inner.disposable);
        let live = self.inner.live_with(slot.as_ref());
        let (value, discard) = work(&live);
        let Some(layer) = discard else {
            return (value, None);
        };
        let checked = match slot.as_ref() {
            Some(waiting) if waiting.sandbox.to_string() == layer => {
                humanitl_sandbox::disposable::ensure_ended(&waiting.handle, waiting.child_start)
            }
            _ => Err(crate::disposable::nothing_pending()),
        };
        if let Err(found) = checked {
            return (value, Some(Err(found)));
        }
        let Some(waiting) = slot.take() else {
            return (value, Some(Err(crate::disposable::nothing_pending())));
        };
        drop(slot);
        let failed = self.inner.drop_layer(waiting, "discarded");
        (value, Some(failed.map_or(Ok(()), Err)))
    }
}
