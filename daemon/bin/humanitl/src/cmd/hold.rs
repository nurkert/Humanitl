//! Die Ereignisse des Starts, solange die Ladeanimation von `humanitl go`
//! steht (HUM-496).
//!
//! Eine Zeile auf dem alternativen Bildschirm der Animation verschwände mit
//! ihm. Deshalb hält [`Hold`] jedes Ereignis des Starts zurück, solange die
//! Animation steht, und gibt sie danach in ihrer Reihenfolge heraus. Läuft
//! die Sitzung, bleibt die Animation den Rest ihrer Mindestzeit; die Schleife
//! in `cmd/run.rs` liest währenddessen weiter Ereignisse und Signale und
//! wartet dazu auf [`Hold::deadline`]. Scheitert der Start, endet die
//! Animation sofort.
//!
//! Ohne Animation (`humanitl run`, oder `go` ohne Terminal) hält [`Hold`]
//! nichts zurück, und die Schleife verhält sich wie vor HUM-496.

use std::collections::VecDeque;

use humanitl_core::Severity;
use humanitl_ipc::v1;
use tokio::time::Instant;

use crate::cmd::from_proto;
use crate::splash::{Palette, Splash};

/// Die Animation und was sie zurückhält.
#[derive(Debug, Default)]
pub(crate) struct Hold {
    /// Die Animation, solange sie steht.
    splash: Option<Splash>,
    /// Die zurückgehaltenen Ereignisse, ältestes zuerst.
    held: VecDeque<v1::SandboxEvent>,
    /// Seit die Sitzung läuft: wann die Mindestzeit vorbei ist.
    until: Option<Instant>,
}

impl Hold {
    /// Startet die Animation in dieser Palette, oder keine.
    pub(crate) fn start(palette: Option<Palette>) -> Self {
        Self {
            splash: palette.and_then(Splash::start),
            ..Self::default()
        }
    }

    /// Ob die Animation steht.
    pub(crate) const fn showing(&self) -> bool {
        self.splash.is_some()
    }

    /// Ob noch zurückgehaltene Ereignisse warten.
    pub(crate) fn has_held(&self) -> bool {
        !self.held.is_empty()
    }

    /// Das nächste zurückgehaltene Ereignis, sobald die Animation weg ist.
    pub(crate) fn release(&mut self) -> Option<v1::SandboxEvent> {
        if self.showing() {
            return None;
        }
        self.held.pop_front()
    }

    /// Beendet die Animation vor der Übergabe des Terminals an den Agenten
    /// und wartet ohne Frist, bis sie das Terminal zurückgegeben hat
    /// ([`Splash::finish`]). Sonst schriebe ein Thread, der an einer
    /// angehaltenen Ausgabe hängt, sein `RESTORE` später über den Schirm des
    /// Agenten. Die Sitzung läuft da schon; ein angehaltenes Terminal hielte
    /// die Ausgabe des Agenten ohnehin auf.
    pub(crate) async fn finish(&mut self) {
        self.until = None;
        if let Some(splash) = self.splash.take() {
            splash.finish().await;
        }
    }

    /// Beendet die Animation sofort, mit begrenztem Warten
    /// ([`Splash::stop`]): für Signal, Fehler und Ende, wo der Befehl noch
    /// die Sitzung stoppen muss. Idempotent.
    pub(crate) fn stop(&mut self) {
        self.splash = None;
        self.until = None;
    }

    /// Der Strom des Starts ist zu Ende. Lief die Sitzung, bleibt die
    /// Animation bis zum Ende ihrer Mindestzeit; sonst geht sie sofort.
    pub(crate) fn stream_ended(&mut self) {
        if self.until.is_none() {
            self.stop();
        }
    }

    /// Wann die Mindestzeit vorbei ist, sobald die Sitzung läuft.
    pub(crate) const fn deadline(&self) -> Option<Instant> {
        self.until
    }

    /// Nimmt ein Ereignis des Starts: Steht die Animation, wird es
    /// zurückgehalten (`None`), sonst kommt es unverändert zurück.
    pub(crate) fn keep(&mut self, event: v1::SandboxEvent) -> Option<v1::SandboxEvent> {
        let Some(splash) = self.splash.as_ref() else {
            return Some(event);
        };
        let end = splash_end(&event);
        self.held.push_back(event);
        match end {
            // Auch ohne Rest über die Frist: Das Ende vor der Übergabe ist
            // [`Hold::finish`], nie das begrenzte [`Hold::stop`].
            SplashEnd::Running if self.until.is_none() => {
                self.until = Some(Instant::now() + splash.rest());
            }
            SplashEnd::Failed => self.stop(),
            SplashEnd::Running | SplashEnd::Not => {}
        }
        None
    }
}

/// Wartet bis `deadline`, oder für immer ohne.
pub(crate) async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// Was ein Ereignis des Starts für die Ladeanimation heißt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplashEnd {
    /// Die Sitzung läuft: Die Animation bleibt den Rest ihrer Mindestzeit.
    Running,
    /// Der Start ist gescheitert oder schon vorbei: Sie geht sofort, damit
    /// der Befund auf dem gewöhnlichen Terminal steht.
    Failed,
    /// Nichts davon; sie dreht sich weiter.
    Not,
}

/// Ordnet ein Ereignis des Starts ein.
pub(crate) fn splash_end(event: &v1::SandboxEvent) -> SplashEnd {
    use v1::sandbox_event::Event;

    match event.event.as_ref() {
        Some(Event::Status(status)) if status.state == v1::SandboxState::Running as i32 => {
            SplashEnd::Running
        }
        Some(Event::Status(status))
            if status.state == v1::SandboxState::Failed as i32
                || status.state == v1::SandboxState::Stopped as i32 =>
        {
            SplashEnd::Failed
        }
        Some(Event::Exit(_)) => SplashEnd::Failed,
        Some(Event::Check(check)) if !check.passed => SplashEnd::Failed,
        Some(Event::Diagnostic(wire))
            if from_proto(wire).is_some_and(|found| found.severity >= Severity::Error) =>
        {
            SplashEnd::Failed
        }
        _ => SplashEnd::Not,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::{Hold, SplashEnd, splash_end};

    fn status(state: v1::SandboxState) -> v1::SandboxEvent {
        v1::SandboxEvent {
            event: Some(v1::sandbox_event::Event::Status(
                v1::sandbox_event::Status {
                    state: state as i32,
                    ..Default::default()
                },
            )),
        }
    }

    /// `running` lässt die Animation auslaufen, ein gescheiterter Start
    /// beendet sie sofort, alles andere lässt sie stehen.
    #[test]
    fn the_start_events_end_the_splash_as_they_should() {
        assert_eq!(
            splash_end(&status(v1::SandboxState::Running)),
            SplashEnd::Running
        );
        assert_eq!(
            splash_end(&status(v1::SandboxState::Failed)),
            SplashEnd::Failed
        );
        assert_eq!(
            splash_end(&status(v1::SandboxState::Starting)),
            SplashEnd::Not
        );
        let check = |passed| v1::SandboxEvent {
            event: Some(v1::sandbox_event::Event::Check(v1::CheckResult {
                passed,
                ..Default::default()
            })),
        };
        assert_eq!(splash_end(&check(false)), SplashEnd::Failed);
        assert_eq!(splash_end(&check(true)), SplashEnd::Not);
    }

    /// Ohne Animation hält nichts zurück: `humanitl run` sieht jedes
    /// Ereignis sofort.
    #[test]
    fn without_a_splash_nothing_is_held() {
        let mut hold = Hold::start(None);
        assert!(!hold.showing());
        let event = status(v1::SandboxState::Running);
        assert_eq!(hold.keep(event.clone()), Some(event));
        assert!(hold.release().is_none());
        assert!(hold.deadline().is_none());
    }
}
