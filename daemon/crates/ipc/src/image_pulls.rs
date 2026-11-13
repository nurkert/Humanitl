//! Die laufenden Ziehen eines Abbilds und ihr Abbruch (HUM-457).
//!
//! **Abbrechen nach Kennung, nie nach Lage.** Ein Ziehen trägt die Kennung,
//! die der Client in `ImagePull.pull_id` wählt; ein Abbruch nennt sie und
//! trifft nur genau dieses Ziehen. Eine Kennung, unter der schon ein Ziehen
//! läuft, nimmt kein zweites an, also gehört zu jeder Kennung höchstens ein
//! laufendes. „Das Ziehen, das gerade läuft" gibt es als Ziel nicht: Zwei
//! Clients, die gleichzeitig ziehen, bräche sonst der eine dem anderen ab.
//!
//! **Die Kennung wählt der Client.** So kennt er sie, bevor der Daemon
//! antwortet, und ein `Ctrl+C` gleich nach dem Senden trifft schon. Eine vom
//! Daemon vergebene Kennung bräuchte ein erstes Ereignis vor dem Ergebnis,
//! und die Antwort auf `image_pull` ist seit Minor 36 genau ein Ereignis.
//!
//! **Ein Abbruch ohne Ziel ist kein Fehler.** Ist das Ziehen schon zu Ende,
//! wenn der Abbruch ankommt, oder hat es unter dieser Kennung nie eines
//! gegeben, antwortet der Daemon `cancelled = false`. Das ist der
//! gewöhnliche Wettlauf zwischen dem Ende und dem Wunsch, es abzubrechen;
//! ein Befund dafür hieße den Menschen etwas reparieren, das nicht kaputt
//! ist.
//!
//! **Die Antwort sagt, was der Adapter tat.** Der Abbruch setzt das
//! Zeichen, das der Adapter zwischen zwei Blicken auf die Engine liest, und
//! wartet dann, bis das Ziehen zurückgekehrt ist. `cancelled = true` kommt
//! nur, wenn der Adapter selbst einträgt, dass er die Engine unterbrochen hat
//! ([`PullCancel::interrupted`]); ein Ziehen, das zwischen dem Nachschlagen
//! und dem Zeichen von selbst fertig wurde, oder das gerade den Digest
//! prüfte, ist nicht abgebrochen, und sein eigenes Ergebnis gilt.
//! `still_running` nennt die Prozesse der Prozessgruppe des Ziehens, die
//! nach dem `SIGKILL` noch liefen; null heißt, dass von ihm nichts mehr
//! läuft.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_sandbox::PullCancel;
use uuid::Uuid;

/// Wie lange ein Abbruch höchstens auf das Ende des Ziehens wartet. Der
/// Adapter sieht das Zeichen binnen 20 ms und wartet nach dem `SIGKILL`
/// höchstens eine Sekunde auf die Prozesse; der Rest ist Luft für einen
/// beschäftigten Rechner.
pub(crate) const CANCEL_WAIT: Duration = Duration::from_secs(10);

/// Die Ziehen, die gerade mit einer Kennung laufen.
#[derive(Debug, Default)]
pub(crate) struct ImagePulls {
    running: Arc<Mutex<HashMap<Uuid, Running>>>,
}

/// Ein laufendes Ziehen: sein Zeichen und woran sein Ende zu sehen ist.
#[derive(Debug)]
struct Running {
    cancel: Arc<PullCancel>,
    ended: tokio::sync::watch::Receiver<bool>,
}

/// Der Platz eines Ziehens in [`ImagePulls`], solange es läuft. Geht er,
/// ist das Ziehen zu Ende: Der Eintrag verschwindet, und wer auf das Ende
/// wartet, erfährt es, auch wenn das Ziehen mit einer Panik endete. Er
/// gehört dem Ziehen und reist mit ihm auf dessen Faden.
#[derive(Debug)]
pub(crate) struct PullSlot {
    running: Arc<Mutex<HashMap<Uuid, Running>>>,
    id: Option<Uuid>,
    cancel: Arc<PullCancel>,
    ended: tokio::sync::watch::Sender<bool>,
}

impl PullSlot {
    /// Das Zeichen, das der Adapter liest.
    pub(crate) fn cancel(&self) -> &PullCancel {
        &self.cancel
    }
}

impl Drop for PullSlot {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            lock(&self.running).remove(&id);
        }
        self.ended.send_replace(true);
    }
}

impl ImagePulls {
    /// Meldet ein Ziehen an. Ohne Kennung bekommt es ein Zeichen, das
    /// niemand setzen kann: Es ist nicht abbrechbar.
    ///
    /// # Errors
    ///
    /// `IPC_005`, wenn unter `id` schon ein Ziehen läuft; dann wird nichts
    /// gezogen.
    pub(crate) fn register(&self, id: Option<Uuid>) -> Result<PullSlot, Diagnostic> {
        let cancel = Arc::new(PullCancel::new());
        let (ended, watched) = tokio::sync::watch::channel(false);
        if let Some(id) = id {
            let mut running = lock(&self.running);
            if running.contains_key(&id) {
                return Err(Diagnostic::builder(codes::IPC_005, Severity::Error)
                    .why(format!(
                        "an image pull with the id {id} is already running; each pull needs \
                         an id of its own, so that a cancel ends only the pull it names"
                    ))
                    .build());
            }
            running.insert(
                id,
                Running {
                    cancel: Arc::clone(&cancel),
                    ended: watched,
                },
            );
        }
        Ok(PullSlot {
            running: Arc::clone(&self.running),
            id,
            cancel,
            ended,
        })
    }

    /// Bricht das Ziehen mit der Kennung `id` ab und wartet höchstens
    /// `wait`, bis es zu Ende ist.
    ///
    /// `Ok(None)`: Unter der Kennung lief nichts, oder das Ziehen endete,
    /// ohne dass der Adapter die Engine unterbrach. `Ok(Some(n))`: Der
    /// Adapter hat die Engine unterbrochen, und danach liefen noch `n`
    /// Prozesse des Ziehens.
    ///
    /// # Errors
    ///
    /// `SANDBOX_043` als Warnung, wenn das Ziehen binnen `wait` nicht zu Ende
    /// kam: Gebeten ist es, ob noch etwas läuft, ist offen.
    pub(crate) async fn cancel(
        &self,
        id: Uuid,
        wait: Duration,
    ) -> Result<Option<usize>, Diagnostic> {
        let found = lock(&self.running)
            .get(&id)
            .map(|running| (Arc::clone(&running.cancel), running.ended.clone()));
        let Some((cancel, mut ended)) = found else {
            return Ok(None);
        };
        cancel.cancel();
        // Ein `Err` heißt: Der Sender ist weg, und das geschieht nur, wenn
        // der Platz des Ziehens ging. Auch das ist ein Ende.
        match tokio::time::timeout(wait, ended.wait_for(|ended| *ended)).await {
            Ok(_) => Ok(cancel.interrupted()),
            Err(_elapsed) => Err(Diagnostic::builder(codes::SANDBOX_043, Severity::Warning)
                .why(format!(
                    "the image pull {id} was asked to stop but had not ended after {wait:?}; \
                     the container engine may still be running for it"
                ))
                .build()),
        }
    }
}

/// Ein vergifteter Mutex verliert nichts: Die Tabelle enthält nur Werte.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Ein Abbruch trifft genau das Ziehen mit seiner Kennung, wartet auf
    /// sein Ende und sagt `true`; ein zweites Ziehen bleibt unberührt.
    #[tokio::test]
    async fn a_cancel_hits_only_the_pull_it_names_and_waits_for_its_end() {
        let pulls = Arc::new(ImagePulls::default());
        let (one, other) = (Uuid::now_v7(), Uuid::now_v7());
        let (started, ready) = std::sync::mpsc::channel();
        let worker = {
            let pulls = Arc::clone(&pulls);
            std::thread::spawn(move || {
                let slot = pulls.register(Some(one)).expect("registered");
                let bystander = pulls.register(Some(other)).expect("registered");
                started.send(()).unwrap();
                while !slot.cancel().is_cancelled() {
                    std::thread::sleep(Duration::from_millis(5));
                }
                // Das Ende kommt etwas nach dem Zeichen; die Antwort wartet.
                std::thread::sleep(Duration::from_millis(100));
                let untouched = !bystander.cancel().is_cancelled();
                slot.cancel().mark_interrupted(0);
                drop(slot);
                untouched
            })
        };
        ready.recv().unwrap();
        let asked = std::time::Instant::now();
        assert_eq!(pulls.cancel(one, CANCEL_WAIT).await, Ok(Some(0)));
        assert!(
            asked.elapsed() >= Duration::from_millis(100),
            "answered before the end"
        );
        assert!(worker.join().unwrap(), "the other pull was cancelled too");
        // Danach läuft unter der Kennung nichts mehr.
        assert_eq!(pulls.cancel(one, CANCEL_WAIT).await, Ok(None));
    }

    /// Eine unbekannte Kennung ist kein Fehler, sondern „nichts abgebrochen".
    #[tokio::test]
    async fn a_cancel_of_an_unknown_pull_is_false() {
        let pulls = ImagePulls::default();
        assert_eq!(pulls.cancel(Uuid::now_v7(), CANCEL_WAIT).await, Ok(None));
    }

    /// Endet das Ziehen, ohne dass der Adapter die Engine unterbrach (es war
    /// gerade fertig, oder es prüfte schon den Digest), ist das kein Abbruch;
    /// überlebten Prozesse das Beenden, sagt die Antwort wie viele.
    #[tokio::test]
    async fn the_answer_is_what_the_adapter_reported() {
        let pulls = Arc::new(ImagePulls::default());
        for reported in [None, Some(3)] {
            let id = Uuid::now_v7();
            let (started, ready) = std::sync::mpsc::channel();
            let worker = {
                let pulls = Arc::clone(&pulls);
                std::thread::spawn(move || {
                    let slot = pulls.register(Some(id)).expect("registered");
                    started.send(()).unwrap();
                    while !slot.cancel().is_cancelled() {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    if let Some(alive) = reported {
                        slot.cancel().mark_interrupted(alive);
                    }
                })
            };
            ready.recv().unwrap();
            assert_eq!(pulls.cancel(id, CANCEL_WAIT).await, Ok(reported));
            worker.join().unwrap();
        }
    }

    /// Eine Kennung, unter der schon ein Ziehen läuft, nimmt kein zweites
    /// an; nach dem Ende des ersten ist sie wieder frei.
    #[test]
    fn a_running_id_is_refused_until_its_pull_ends() {
        let pulls = ImagePulls::default();
        let id = Uuid::now_v7();
        let slot = pulls.register(Some(id)).expect("registered");
        let err = pulls.register(Some(id)).expect_err("a second pull");
        assert_eq!(err.code, codes::IPC_005, "{}", err.why);
        drop(slot);
        let _again = pulls.register(Some(id)).expect("free again");
        // Ohne Kennung: beliebig viele, keiner abbrechbar.
        let _a = pulls.register(None).expect("no id");
        let _b = pulls.register(None).expect("no id");
    }

    /// Kommt das Ziehen nicht zu Ende, sagt der Abbruch das als Warnung,
    /// statt `true` zu behaupten.
    #[tokio::test]
    async fn a_pull_that_does_not_end_is_a_warning() {
        let pulls = ImagePulls::default();
        let id = Uuid::now_v7();
        let _slot = pulls.register(Some(id)).expect("registered");
        let err = pulls
            .cancel(id, Duration::from_millis(50))
            .await
            .expect_err("still running");
        assert_eq!(err.code, codes::SANDBOX_043, "{}", err.why);
        assert_eq!(err.severity, Severity::Warning);
    }
}
