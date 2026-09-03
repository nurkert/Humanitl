//! Bodies: den Request puffern, die Response streamen.
//!
//! Zwei Regeln aus ADR-005 leben hier: Der Request-Body wird vor der
//! Entscheidung **vollständig** gepuffert (bis `limits.hold_body_cap_bytes`),
//! die Response wird **nie** gepuffert, sondern durchgereicht. Der Puffer ist
//! nötig, weil die Oberfläche den Body zeigen und der Nutzer ihn bearbeiten
//! können muss; die Response darf es nicht sein, weil ein LLM-Stream sonst zum
//! Stillstand käme.
//!
//! [`TeeBody`] legt sich um die Antwort des Ziels: jedes Frame läuft
//! unverändert zum Client, nebenbei entsteht ein
//! [`FlowEvent::ResponseChunk`], und am Ende der Antwort — auch wenn der Client
//! die Verbindung abbricht — wird der Flow mit `Record` abgeschlossen.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::SystemTime;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Full, Limited};
use humanitl_core::{Flow, FlowEvent, TransitionInput};
use hyper::body::{Body, Frame, Incoming};

use crate::hold::HoldQueue;

/// Der Fehler eines Response-Bodys, den der Server dem Client durchreicht.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Der Body jeder Antwort des Proxys: entweder eine fertige Meldung oder ein
/// durchgereichter Strom.
pub type ResponseBody = BoxBody<Bytes, BoxError>;

/// Warum ein Request-Body nicht gepuffert werden konnte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// Der Body ist größer als das Cap; die Anfrage wird mit `413` geblockt.
    Cap,
    /// Der Client brach die Übertragung ab oder sendete kaputte Frames.
    Read,
}

/// Puffert den Request-Body vollständig, aber höchstens `cap` Bytes.
///
/// Ein `Content-Length` über dem Cap wird vom Aufrufer schon vorher erkannt;
/// diese Funktion fängt den chunked-Fall, bei dem die Länge erst beim Lesen
/// auffällt. hyper beantwortet ein `Expect: 100-continue` automatisch, sobald
/// hier zum ersten Mal gelesen wird — gewollt, denn der Body muss vor der
/// Entscheidung vorliegen, und vor der Entscheidung geht nichts zum Ziel.
///
/// # Errors
///
/// [`BufferError::Cap`], wenn der Body das Cap überschreitet;
/// [`BufferError::Read`] bei einem Übertragungsfehler.
pub async fn buffer(body: Incoming, cap: u64) -> Result<Bytes, BufferError> {
    let limit = usize::try_from(cap).unwrap_or(usize::MAX);
    match Limited::new(body, limit).collect().await {
        Ok(collected) => Ok(collected.to_bytes()),
        Err(err) => {
            if err
                .downcast_ref::<http_body_util::LengthLimitError>()
                .is_some()
            {
                Err(BufferError::Cap)
            } else {
                Err(BufferError::Read)
            }
        }
    }
}

/// Ein Response-Body aus fertigen Bytes (Block-, Fehler- und
/// `CONNECT`-Antworten).
#[must_use]
pub fn full(bytes: impl Into<Bytes>) -> ResponseBody {
    Full::new(bytes.into())
        .map_err(|never| match never {})
        .boxed()
}

/// Der leere Body.
#[must_use]
pub fn empty() -> ResponseBody {
    full(Bytes::new())
}

/// Legt [`TeeBody`] um die Antwort des Ziels und boxt sie als [`ResponseBody`].
#[must_use]
pub fn tee(inner: Incoming, flow: Flow, queue: Arc<HoldQueue>) -> ResponseBody {
    TeeBody {
        inner,
        flow: Some(flow),
        queue,
        finished: false,
    }
    .map_err(BoxError::from)
    .boxed()
}

/// Reicht die Antwort des Ziels an den Client durch und beobachtet sie dabei.
///
/// Kein Puffern: jedes Frame wird sofort weitergegeben. Nebenbei zählt der Tee
/// die Daten-Frames als [`FlowEvent::ResponseChunk`] und schließt den Flow beim
/// letzten Frame mit `Record` ab. Bricht der Client vorher ab, erledigt das der
/// `Drop`, damit kein Flow ohne `Recorded` zurückbleibt.
pub struct TeeBody {
    inner: Incoming,
    flow: Option<Flow>,
    queue: Arc<HoldQueue>,
    finished: bool,
}

impl TeeBody {
    /// Schließt den Flow genau einmal ab.
    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(mut flow) = self.flow.take() {
            match flow.apply(TransitionInput::Record, SystemTime::now()) {
                Ok(event) => self.queue.publish(event),
                Err(err) => {
                    tracing::debug!(flow = %flow.id, %err, "response body ended in a state that cannot record");
                }
            }
        }
    }
}

impl Body for TeeBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let (Some(flow), Some(data)) = (this.flow.as_ref(), frame.data_ref()) {
                    let len = data.len() as u64;
                    this.queue.publish(FlowEvent::ResponseChunk {
                        flow_id: flow.id,
                        at: SystemTime::now(),
                        len,
                    });
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finish();
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                this.finish();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for TeeBody {
    fn drop(&mut self) {
        self.finish();
    }
}
