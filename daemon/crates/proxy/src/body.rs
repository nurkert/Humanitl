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
//! [`FlowEvent::ResponseChunk`], jedes Stück geht durch den
//! [`ResponseSink`] der Aufzeichnung, und am Ende der Antwort — auch wenn der
//! Client die Verbindung abbricht — wird der Flow mit `Record` abgeschlossen.
//!
//! Der Unterschied zwischen „fertig" und „abgebrochen" bleibt dabei erhalten:
//! Eine Antwort, die zu Ende lief, schließt mit [`ResponseSink::finish`], eine
//! abgebrochene mit [`ResponseSink::abort`], und die Aufzeichnung vermerkt sie
//! als gekürzt. Eine halbe Antwort, die wie eine ganze aussieht, wäre genau
//! die stille Lücke, die `backlog/CONVENTIONS.md` 4.13 verbietet.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::SystemTime;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Full, Limited};
use humanitl_core::{Flow, FlowEvent, TransitionInput};
use humanitl_recorder::ResponseSink;
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
///
/// `sink` ist der Mitschnitt der Aufzeichnung, `status` der Status, mit dem er
/// abgeschlossen wird. Ohne Aufzeichnung ist `sink` `None`, und der Tee tut,
/// was er vor HUM-026 tat.
#[must_use]
pub fn tee(
    inner: Incoming,
    flow: Flow,
    queue: Arc<HoldQueue>,
    sink: Option<ResponseSink>,
    status: u16,
) -> ResponseBody {
    TeeBody {
        inner,
        flow: Some(flow),
        queue,
        finished: false,
        sink,
        status,
        complete: false,
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
    /// Der Mitschnitt der Antwort, solange einer läuft.
    sink: Option<ResponseSink>,
    /// Der Status, mit dem der Mitschnitt abgeschlossen wird.
    status: u16,
    /// Wahr, sobald der Strom des Ziels regulär endete.
    complete: bool,
}

impl TeeBody {
    /// Schließt den Flow genau einmal ab.
    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        // Erst der Mitschnitt, dann der Zustandswechsel: `Recorded` bedeutet
        // „alles zu diesem Flow steht", und die Antwort gehört dazu.
        if let Some(sink) = self.sink.take() {
            let flow_id = self.flow.as_ref().map(|flow| flow.id);
            let stored = if self.complete {
                sink.finish(self.status)
            } else {
                sink.abort()
            };
            if let Err(err) = stored {
                tracing::warn!(
                    flow = ?flow_id,
                    code = err.diagnostic().code.as_str(),
                    why = %err.diagnostic().why,
                    "the response body could not be recorded; the answer itself was delivered"
                );
            }
        }
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
                if let Some(data) = frame.data_ref() {
                    if let Some(sink) = this.sink.as_mut() {
                        sink.chunk(data);
                    }
                    if let Some(flow) = this.flow.as_ref() {
                        let len = data.len() as u64;
                        this.queue.publish(FlowEvent::ResponseChunk {
                            flow_id: flow.id,
                            at: SystemTime::now(),
                            len,
                        });
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finish();
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                this.complete = true;
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
