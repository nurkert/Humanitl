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
//!
//! # Die Uhr über beiden Rumpf-Spannen (HUM-120)
//!
//! Beide Richtungen bekommen dieselbe Grenze aus `limits.body_timeout_secs`,
//! und beide messen damit die **Stille zwischen zwei Stücken**, nie die
//! Gesamtdauer. Der Unterschied ist der ganze Punkt: Ein Upload von 30 MiB und
//! ein Modell, das eine Viertelstunde lang Token schickt, sind normal; ein
//! Peer, der zehn Bytes schickt und dann schweigt, ist es nicht. Eine Uhr über
//! der Gesamtdauer träfe die ersten beiden und beschriebe genau den erklärten
//! Seitenkanal zum Sprachmodell als Angriff (`backlog/CONVENTIONS.md` 4.25).
//!
//! Die Uhr des Anfrage-Rumpfs beginnt beim ersten Lesen, also nach dem Kopf:
//! Vorher wacht `header_read_timeout` von hyper, und zwei Uhren über einer
//! Spanne wären wieder der Fehler, den HUM-101 beseitigt hat. Ein
//! `Expect: 100-continue` beantwortet hyper genau bei diesem ersten Lesen; die
//! Frist deckt danach das Warten auf das erste Stück des Clients ab.

use std::future::Future as _;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use bytes::{Bytes, BytesMut};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Full, Limited};
use humanitl_core::{Flow, FlowEvent, TransitionInput};
use humanitl_recorder::ResponseSink;
use hyper::body::{Body, Frame, Incoming};
use tokio::time::{Instant, Sleep};

use crate::hold::HoldQueue;

/// Der Fehler eines Response-Bodys, den der Server dem Client durchreicht.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Der Body jeder Antwort des Proxys: entweder eine fertige Meldung oder ein
/// durchgereichter Strom.
pub type ResponseBody = BoxBody<Bytes, BoxError>;

/// Der Fehlertext, mit dem ein stehengebliebener Antwort-Strom endet.
///
/// Er geht nicht an den Client — dessen Antwort-Kopfzeilen stehen längst, und
/// was er sieht, ist ein abgeschnittener Strom —, sondern in die Protokollzeile
/// von hyper. Er steht hier, damit ein Test ihn wiedererkennt.
pub const RESPONSE_IDLE_ERROR: &str = "upstream response body went silent";

/// Warum ein Request-Body nicht gepuffert werden konnte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// Der Body ist größer als das Cap; die Anfrage wird mit `413` geblockt.
    Cap,
    /// Der Client brach die Übertragung ab oder sendete kaputte Frames.
    Read,
    /// Der Client hat länger als `limits.body_timeout_secs` kein Stück mehr
    /// geschickt; die Anfrage endet mit `408`.
    Idle,
}

/// Puffert den Request-Body vollständig, aber höchstens `cap` Bytes, und lässt
/// zwischen zwei Stücken höchstens `idle` verstreichen.
///
/// Ein `Content-Length` über dem Cap wird vom Aufrufer schon vorher erkannt;
/// diese Funktion fängt den chunked-Fall, bei dem die Länge erst beim Lesen
/// auffällt. hyper beantwortet ein `Expect: 100-continue` automatisch, sobald
/// hier zum ersten Mal gelesen wird — gewollt, denn der Body muss vor der
/// Entscheidung vorliegen, und vor der Entscheidung geht nichts zum Ziel.
///
/// `idle` gilt je Stück und nicht für den ganzen Rumpf: Ein Upload darf so
/// lange dauern, wie er dauert, solange er nicht verstummt.
///
/// # Errors
///
/// [`BufferError::Cap`], wenn der Body das Cap überschreitet;
/// [`BufferError::Idle`], wenn `idle` ohne ein weiteres Stück verstreicht;
/// [`BufferError::Read`] bei einem Übertragungsfehler.
pub async fn buffer(body: Incoming, cap: u64, idle: Duration) -> Result<Bytes, BufferError> {
    let limit = usize::try_from(cap).unwrap_or(usize::MAX);
    let mut limited = Limited::new(body, limit);
    let mut collected = BytesMut::new();
    loop {
        // Die Frist wird vor jedem Stück neu gespannt; gemessen wird damit die
        // Lücke zwischen zwei Stücken.
        let Ok(next) = tokio::time::timeout(idle, limited.frame()).await else {
            return Err(BufferError::Idle);
        };
        match next {
            None => return Ok(collected.freeze()),
            Some(Ok(frame)) => {
                if let Some(data) = frame.data_ref() {
                    collected.extend_from_slice(data);
                }
            }
            Some(Err(err)) => {
                return Err(
                    if err
                        .downcast_ref::<http_body_util::LengthLimitError>()
                        .is_some()
                    {
                        BufferError::Cap
                    } else {
                        BufferError::Read
                    },
                );
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
/// was er vor HUM-026 tat. `idle` ist die längste Stille zwischen zwei Stücken
/// (`limits.body_timeout_secs`).
///
/// # Panics
///
/// Nie aus eigenem Antrieb; die Frist braucht aber einen laufenden
/// Tokio-Zeitgeber, weil sie hier gespannt wird.
#[must_use]
pub fn tee(
    inner: Incoming,
    flow: Flow,
    queue: Arc<HoldQueue>,
    sink: Option<ResponseSink>,
    status: u16,
    idle: Duration,
) -> ResponseBody {
    TeeBody {
        inner,
        flow: Some(flow),
        queue,
        finished: false,
        sink,
        status,
        complete: false,
        idle,
        deadline: Box::pin(tokio::time::sleep(idle)),
    }
    .boxed()
}

/// Reicht die Antwort des Ziels an den Client durch und beobachtet sie dabei.
///
/// Kein Puffern: jedes Frame wird sofort weitergegeben. Nebenbei zählt der Tee
/// die Daten-Frames als [`FlowEvent::ResponseChunk`] und schließt den Flow beim
/// letzten Frame mit `Record` ab. Bricht der Client vorher ab, erledigt das der
/// `Drop`, damit kein Flow ohne `Recorded` zurückbleibt.
///
/// Bleibt das Ziel länger als die übergebene Leerlauffrist stumm, endet der Strom als
/// Fehler: Der Client sieht einen abgeschnittenen Rumpf, der Mitschnitt wird
/// über [`ResponseSink::abort`] als gekürzt vermerkt, und der Flow geht
/// trotzdem über `Record` in seinen Endzustand. Eine Aufzeichnung, die ihren
/// Schwanz stillschweigend verliert, wäre schlechter als eine, die sagt, dass
/// sie abgeschnitten ist.
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
    /// Die längste erlaubte Stille zwischen zwei Stücken.
    idle: Duration,
    /// Wann diese Stille zu lang wird. Wird nach jedem Frame neu gespannt.
    deadline: Pin<Box<Sleep>>,
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
    type Error = BoxError;

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
                // Ein Stück ist angekommen: Die Frist beginnt von vorn.
                this.deadline.as_mut().reset(Instant::now() + this.idle);
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finish();
                Poll::Ready(Some(Err(BoxError::from(err))))
            }
            Poll::Ready(None) => {
                this.complete = true;
                this.finish();
                Poll::Ready(None)
            }
            Poll::Pending => {
                if this.deadline.as_mut().poll(cx).is_pending() {
                    return Poll::Pending;
                }
                // `complete` bleibt falsch: Der Mitschnitt wird als gekürzt
                // abgeschlossen, und `finish` läuft hier genau einmal — der
                // `Drop` findet `finished` gesetzt vor.
                let flow_id = this.flow.as_ref().map(|flow| flow.id);
                tracing::debug!(
                    flow = ?flow_id,
                    idle = ?this.idle,
                    "{RESPONSE_IDLE_ERROR}"
                );
                this.finish();
                Poll::Ready(Some(Err(BoxError::from(RESPONSE_IDLE_ERROR))))
            }
        }
    }
}

impl Drop for TeeBody {
    fn drop(&mut self) {
        self.finish();
    }
}
