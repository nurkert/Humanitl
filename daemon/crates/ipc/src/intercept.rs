//! Die RPC `Intercept`: die Pause des Abfangens lesen, setzen, beenden
//! (HUM-407).
//!
//! Die Fähigkeit liegt im Daemon (ADR-018); Oberfläche und Kommandozeile
//! sind dünne Clients darüber. Gesetzt wird die Pause nur hier, also nur über
//! den Socket des Daemons mit seinem Token; der Proxy und der Meta-Endpunkt
//! `humanitl.internal` haben keinen Weg dorthin.
//!
//! Wer das Ergebnis eines Wechsels sehen will, liest die Antwort; alle
//! anderen Clients sehen ihn als `FlowEvent.intercept` im Strom von
//! `Subscribe`, und das Audit-Log schreibt `intercept.paused` und
//! `intercept.resumed` aus demselben Ereignis.

use humanitl_core::diagnostics::codes::{IPC_006, IPC_007};
use humanitl_core::{Diagnostic, InterceptChange, InterceptPause, ResumeReason, Severity};

use crate::convert::intercept_status_to_proto;
use crate::v1;

/// Was eine Anfrage an `Intercept` will.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Nur den Stand lesen.
    Status,
    /// Wechseln.
    Change(InterceptChange),
}

/// Liest die Operation aus der Anfrage.
///
/// # Errors
///
/// `IPC_007`, wenn die Anfrage keine Operation nennt.
pub fn operation(request: &v1::InterceptRequest) -> Result<Operation, Diagnostic> {
    match request.op {
        Some(v1::intercept_request::Op::Status(_)) => Ok(Operation::Status),
        Some(v1::intercept_request::Op::Pause(_)) => Ok(Operation::Change(InterceptChange::Paused)),
        Some(v1::intercept_request::Op::Resume(_)) => Ok(Operation::Change(
            InterceptChange::Resumed(ResumeReason::Requested),
        )),
        None => Err(Diagnostic::builder(IPC_007, Severity::Error)
            .why(
                "Intercept changed nothing: the request names no operation (status, pause or \
                 resume)"
                    .to_owned(),
            )
            .build()),
    }
}

/// Beantwortet `Intercept` über der Pause dieses Daemons.
///
/// # Errors
///
/// `IPC_007` ohne Operation, `IPC_006` in einem Daemon ohne Proxy-Sitzung,
/// der keine Pause hat, die sich setzen ließe.
pub fn answer(
    pause: Option<&InterceptPause>,
    request: &v1::InterceptRequest,
    instance: &str,
) -> Result<v1::InterceptStatus, Diagnostic> {
    let operation = operation(request)?;
    let Some(pause) = pause else {
        return Err(Diagnostic::builder(IPC_006, Severity::Error)
            .why(
                "this daemon runs no proxy session, so there is nothing to intercept or pause"
                    .to_owned(),
            )
            .build());
    };
    let status = match operation {
        Operation::Status => pause.status(),
        Operation::Change(InterceptChange::Paused) => {
            tracing::warn!(
                "intercept paused over the daemon socket: requests no rule decides pass"
            );
            pause.pause()
        }
        Operation::Change(InterceptChange::Resumed(reason)) => {
            tracing::info!(reason = reason.as_str(), "intercept resumed");
            pause.resume(reason)
        }
    };
    Ok(stamped(intercept_status_to_proto(status), instance))
}

/// Derselbe Stand mit dem Daemon, von dem er stammt (`Info.session_id`).
///
/// Jede Antwort und jedes Ereignis trägt ihn, damit ein Client eine
/// Revision nur mit Revisionen desselben Daemons vergleicht: Nach einem
/// Neustart beginnt die Zählung bei null, und eine verspätete Antwort des
/// alten Daemons mit höherer Zahl darf den neuen Stand nicht überschreiben.
#[must_use]
pub fn stamped(mut status: v1::InterceptStatus, instance: &str) -> v1::InterceptStatus {
    instance.clone_into(&mut status.instance);
    status
}

/// Ein Ereignis des Stroms mit dem Daemon, von dem es stammt, wenn es ein
/// Wechsel der Pause ist; jedes andere bleibt, wie es ist.
#[must_use]
pub fn stamp_event(mut event: v1::FlowEvent, instance: &str) -> v1::FlowEvent {
    if let Some(v1::flow_event::Event::Intercept(changed)) = event.event.as_mut()
        && let Some(status) = changed.status.take()
    {
        changed.status = Some(stamped(status, instance));
    }
    event
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::InterceptPause;
    use humanitl_core::diagnostics::codes::{IPC_006, IPC_007};

    use super::answer;
    use crate::v1;

    const INSTANCE: &str = "018f0001-0000-7000-8000-000000000407";

    /// Auch das Ereignis nennt den Daemon, von dem es stammt; andere
    /// Ereignisse bleiben unberührt.
    #[test]
    fn an_intercept_event_carries_its_daemon() {
        let event = v1::FlowEvent {
            at: None,
            event: Some(v1::flow_event::Event::Intercept(
                crate::convert::intercept_changed_to_proto(
                    std::time::SystemTime::now(),
                    humanitl_core::InterceptChange::Paused,
                    1,
                ),
            )),
        };
        let Some(v1::flow_event::Event::Intercept(changed)) =
            super::stamp_event(event, INSTANCE).event
        else {
            panic!("still an intercept event");
        };
        let status = changed.status.unwrap();
        assert_eq!(status.instance, INSTANCE);
        assert_eq!(status.revision, 1);
        let other = v1::FlowEvent {
            at: None,
            event: Some(v1::flow_event::Event::Lagged(v1::flow_event::Lagged {
                dropped: 1,
            })),
        };
        assert_eq!(super::stamp_event(other.clone(), INSTANCE), other);
    }

    fn request(op: v1::intercept_request::Op) -> v1::InterceptRequest {
        v1::InterceptRequest { op: Some(op) }
    }

    #[test]
    fn pause_status_and_resume_change_and_read_the_one_pause() {
        let pause = InterceptPause::new();
        let paused = answer(
            Some(&pause),
            &request(v1::intercept_request::Op::Pause(
                v1::intercept_request::Pause {},
            )),
            INSTANCE,
        )
        .unwrap();
        assert_eq!(paused.instance, INSTANCE, "the reply names its daemon");
        assert!(paused.paused && paused.paused_since.is_some());
        assert_eq!(paused.revision, 1, "every change carries its revision");
        assert!(pause.is_paused());

        let read = answer(
            Some(&pause),
            &request(v1::intercept_request::Op::Status(
                v1::intercept_request::Status {},
            )),
            INSTANCE,
        )
        .unwrap();
        assert_eq!(read, paused, "reading changes nothing");

        let resumed = answer(
            Some(&pause),
            &request(v1::intercept_request::Op::Resume(
                v1::intercept_request::Resume {},
            )),
            INSTANCE,
        )
        .unwrap();
        assert!(!resumed.paused && resumed.paused_since.is_none());
        assert_eq!(resumed.revision, 2);
        assert!(!pause.is_paused());
    }

    #[test]
    fn a_request_without_operation_or_a_daemon_without_pause_is_refused() {
        let pause = InterceptPause::new();
        let err = answer(Some(&pause), &v1::InterceptRequest::default(), INSTANCE).unwrap_err();
        assert_eq!(err.code, IPC_007);
        let err = answer(
            None,
            &request(v1::intercept_request::Op::Status(
                v1::intercept_request::Status {},
            )),
            INSTANCE,
        )
        .unwrap_err();
        assert_eq!(err.code, IPC_006);
    }
}
