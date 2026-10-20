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

use crate::hold::{HoldQueue, InFlight};

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
    /// Das Byte-Budget für Rümpfe in Arbeit (`limits.hold_max_bytes`) ist
    /// erschöpft; die Anfrage endet mit `503` (HUM-057).
    Budget,
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

/// Was vom Anfrage-Rumpf beim Puffern herauskam (HUM-057).
pub enum Buffered {
    /// Der ganze Rumpf, höchstens so groß wie das Cap.
    Whole(Bytes),
    /// Der Rumpf ist größer als das Cap. Statt ihn wegzuwerfen, kommen das
    /// schon Gelesene und der Rest der Leitung zurück: Trifft die Anfrage eine
    /// Regel mit `stream: true`, gehen beide zusammen hinaus, sonst wird
    /// geblockt.
    Over {
        /// Was schon gelesen ist, das Cap und höchstens ein Stück darüber.
        prefix: Bytes,
        /// Der ungelesene Rest.
        rest: Incoming,
    },
}

impl core::fmt::Debug for Buffered {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Whole(bytes) => f.debug_tuple("Whole").field(&bytes.len()).finish(),
            Self::Over { prefix, .. } => f
                .debug_struct("Over")
                .field("prefix", &prefix.len())
                .finish_non_exhaustive(),
        }
    }
}

/// Puffert den Anfrage-Rumpf wie [`buffer`], gibt aber einen Rumpf über dem
/// Cap nicht verloren, sondern als [`Buffered::Over`] zurück (HUM-057).
///
/// Der Speicher bleibt dabei beim Cap plus höchstens einem Stück: Sobald das
/// Gelesene das Cap überschreitet, hört das Lesen auf. Jedes Stück geht
/// vorher durch `budget`, den Anteil dieser Anfrage an `limits.hold_max_bytes`;
/// passt es nicht mehr hinein, wird nicht weitergelesen.
///
/// # Errors
///
/// [`BufferError::Idle`] und [`BufferError::Read`] wie bei [`buffer`];
/// [`BufferError::Budget`], wenn das Budget erschöpft ist;
/// [`BufferError::Cap`] nie.
pub async fn buffer_request(
    mut body: Incoming,
    cap: u64,
    idle: Duration,
    budget: &mut InFlight,
) -> Result<Buffered, BufferError> {
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let mut collected = BytesMut::new();
    loop {
        let Ok(next) = tokio::time::timeout(idle, body.frame()).await else {
            return Err(BufferError::Idle);
        };
        match next {
            None => return Ok(Buffered::Whole(collected.freeze())),
            Some(Ok(frame)) => {
                if let Some(data) = frame.data_ref() {
                    let len = u64::try_from(data.len()).unwrap_or(u64::MAX);
                    if budget.grow(len).is_err() {
                        return Err(BufferError::Budget);
                    }
                    collected.extend_from_slice(data);
                }
                if collected.len() > cap {
                    return Ok(Buffered::Over {
                        prefix: collected.freeze(),
                        rest: body,
                    });
                }
            }
            Some(Err(_)) => return Err(BufferError::Read),
        }
    }
}

/// Der Fehlertext, mit dem ein stehengebliebener, gestreamter Anfrage-Rumpf
/// endet. Er geht an hyper, das die Verbindung zum Ziel damit abbricht.
pub const REQUEST_IDLE_ERROR: &str = "client request body went silent";

/// Wie weit ein gestreamter Anfrage-Rumpf beim Ziel ist (HUM-057).
///
/// Die Uhr in [`streamed_request`] läuft nur, solange hyper nach dem nächsten
/// Stück fragt. Liest das Ziel nicht mehr, fragt hyper nicht mehr, und die Uhr
/// stünde still; nimmt das Ziel den ganzen Rumpf und antwortet nie, gibt es gar
/// nichts mehr zu fragen. Beides sieht man nur von außen: Der Strom schreibt
/// hier, wann zuletzt ein Stück hinausging und wann er zu Ende war, und
/// [`crate::upstream::Upstream`] prüft das, solange es auf die Antwort wartet.
#[derive(Debug)]
pub struct StreamProgress {
    state: std::sync::Mutex<ProgressState>,
}

#[derive(Debug, Clone, Copy)]
struct ProgressState {
    /// Wann zuletzt ein Stück hinausging; `None`, solange hyper noch nicht
    /// nach dem ersten gefragt hat. Verbindungsaufbau, TLS und Handschlag
    /// stehen unter ihren eigenen Fristen und zählen hier nicht.
    last: Option<Instant>,
    ended: Option<Instant>,
    /// Der Client hat länger als `limits.body_timeout_secs` geschwiegen.
    client_silent: bool,
    /// Der Strom wartet gerade auf den Client, nicht auf das Ziel. Solange das
    /// gilt, ist kein Stillstand des Ziels möglich; über die Stille des
    /// Clients wacht seine eigene Uhr.
    waiting_for_client: bool,
}

/// Woran ein gestreamter Upload hängen geblieben ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stall {
    /// Das Ziel hat länger als `limits.body_timeout_secs` kein Stück mehr
    /// angenommen.
    NotReading,
    /// Der Rumpf ist ganz beim Ziel, und es hat länger als
    /// `limits.header_timeout_secs` keine Antwort-Kopfzeilen geschickt.
    NoResponseHead,
}

impl StreamProgress {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: std::sync::Mutex::new(ProgressState {
                last: None,
                ended: None,
                client_silent: false,
                waiting_for_client: false,
            }),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&mut ProgressState) -> T) -> T {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut state)
    }

    fn touch(&self) {
        self.with(|state| {
            state.last = Some(Instant::now());
            state.waiting_for_client = false;
        });
    }

    fn wait_for_client(&self) {
        self.with(|state| state.waiting_for_client = true);
    }

    fn mark_client_silent(&self) {
        self.with(|state| state.client_silent = true);
    }

    /// Wahr, wenn der Strom endete, weil der Client länger als
    /// `limits.body_timeout_secs` nichts mehr schickte (HUM-057). Der Handler
    /// antwortet dann `408` mit `PROXY_011` statt mit einem Fehler des Ziels.
    #[must_use]
    pub fn client_went_silent(&self) -> bool {
        self.with(|state| state.client_silent)
    }

    /// Wahr, sobald der ganze Rumpf hinausging.
    #[must_use]
    pub fn is_ended(&self) -> bool {
        self.with(|state| state.ended.is_some())
    }

    fn end(&self) {
        self.with(|state| {
            if state.ended.is_none() {
                state.ended = Some(Instant::now());
            }
        });
    }

    /// Ob der Upload hängt: `idle` ohne angenommenes Stück, solange der Rumpf
    /// läuft, oder `head` ohne Antwort, seit er zu Ende ist.
    #[must_use]
    pub fn stalled(&self, idle: Duration, head: Duration) -> Option<Stall> {
        let state = self.with(|state| *state);
        let now = Instant::now();
        match (state.ended, state.last) {
            (Some(ended), _) if now.duration_since(ended) > head => Some(Stall::NoResponseHead),
            (None, Some(last)) if !state.waiting_for_client && now.duration_since(last) > idle => {
                Some(Stall::NotReading)
            }
            _ => None,
        }
    }
}

/// Der Anfrage-Rumpf einer Anfrage über dem Cap, die gestreamt hinausgeht
/// (HUM-057, `stream: true`).
///
/// Erst das schon Gelesene (`prefix`), dann der Rest der Leitung. Jedes Stück
/// geht durch `sink`, den Mitschnitt der Aufzeichnung
/// ([`humanitl_recorder::Recorder::begin_request`]); zwischen zwei Stücken
/// wacht dieselbe Uhr wie beim Puffern (`limits.body_timeout_secs`). Endet
/// der Strom vorzeitig — Uhr, Abbruch des Clients, Abbruch des Ziels —,
/// vermerkt der Mitschnitt den Rumpf als gekürzt.
///
/// Neben dem Rumpf kommt sein [`StreamProgress`] zurück, über den der
/// Weiterleiter erkennt, dass das Ziel nicht mehr liest oder nie antwortet.
///
/// `budget` ist der Anteil dieser Anfrage an `limits.hold_max_bytes`. Er lebt
/// mit dem Rumpf, nicht mit dem Handler: Schickt das Ziel seine Antwort, bevor
/// es den Upload gelesen hat, liegt der Rest noch im Speicher, und er bleibt
/// gezählt, bis der Rumpf zu Ende ist oder mit der Verbindung verworfen wird.
#[must_use]
pub fn streamed_request(
    prefix: Bytes,
    rest: Incoming,
    sink: Option<ResponseSink>,
    idle: Duration,
    budget: Option<InFlight>,
) -> (ResponseBody, Arc<StreamProgress>) {
    let progress = StreamProgress::new();
    let body = StreamedRequest {
        prefix: (!prefix.is_empty()).then_some(prefix),
        rest,
        sink,
        idle,
        deadline: Box::pin(tokio::time::sleep(idle)),
        started: false,
        progress: Arc::clone(&progress),
        budget,
    }
    .boxed();
    (body, progress)
}

/// So groß ist ein Stück des Präfixes, das [`streamed_request`] auf einmal
/// hergibt. Ein Präfix von bis zu `limits.hold_body_cap_bytes` als ein einziges
/// Stück sähe auf einer langsamen Leitung wie Stillstand aus, weil hyper erst
/// nach dem ganzen Stück wieder fragt.
pub const PREFIX_SLICE: usize = 64 * 1024;

/// Siehe [`streamed_request`].
struct StreamedRequest {
    prefix: Option<Bytes>,
    rest: Incoming,
    sink: Option<ResponseSink>,
    idle: Duration,
    deadline: Pin<Box<Sleep>>,
    /// Ob hyper schon einmal nach einem Stück gefragt hat; erst dann laufen
    /// die Uhren.
    started: bool,
    progress: Arc<StreamProgress>,
    budget: Option<InFlight>,
}

impl StreamedRequest {
    /// Schließt den Mitschnitt ab: vollständig oder gekürzt.
    fn close(&mut self, complete: bool) {
        if complete {
            self.progress.end();
            // Der Rumpf ist hinaus; was davon im Speicher lag, ist es nicht mehr.
            self.budget = None;
        }
        let Some(sink) = self.sink.take() else {
            return;
        };
        let stored = if complete {
            sink.finish_request()
        } else {
            sink.abort()
        };
        if let Err(err) = stored {
            tracing::warn!(
                code = err.diagnostic().code.as_str(),
                why = %err.diagnostic().why,
                "the streamed request body could not be recorded; it was sent anyway"
            );
        }
    }

    /// Schließt den Mitschnitt als vollständig, sobald die Leitung leer ist.
    ///
    /// Bei angekündigter Länge fragt hyper nach dem letzten Byte nicht noch
    /// einmal nach dem Ende; ohne diese Prüfung fiele der Strom danach nur
    /// noch weg, und der `Drop` des Mitschnitts hielte einen vollständigen
    /// Rumpf für gekürzt.
    fn close_if_ended(&mut self) {
        if self.prefix.is_none() && self.rest.is_end_stream() {
            self.close(true);
        }
    }

    /// Ein Stück geht durch den Mitschnitt, und die Uhr beginnt von vorn.
    fn pass(&mut self, data: &[u8]) {
        if let Some(sink) = self.sink.as_mut() {
            sink.chunk(data);
        }
        self.progress.touch();
        self.deadline.as_mut().reset(Instant::now() + self.idle);
    }
}

impl Body for StreamedRequest {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if !this.started {
            // Die Uhren beginnen mit dem Senden des Rumpfs, nicht vorher.
            this.started = true;
            this.deadline.as_mut().reset(Instant::now() + this.idle);
        }
        // Jede Frage von hyper ist Fortschritt aus Sicht des Ziels: Es nimmt an.
        this.progress.touch();
        if let Some(mut prefix) = this.prefix.take() {
            // In Scheiben, ohne Kopie: `split_to` teilt nur den Zeiger.
            let piece = if prefix.len() > PREFIX_SLICE {
                let piece = prefix.split_to(PREFIX_SLICE);
                this.prefix = Some(prefix);
                piece
            } else {
                prefix
            };
            this.pass(&piece);
            this.close_if_ended();
            return Poll::Ready(Some(Ok(Frame::data(piece))));
        }
        match Pin::new(&mut this.rest).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let data = data.clone();
                    this.pass(&data);
                }
                this.close_if_ended();
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.close(false);
                Poll::Ready(Some(Err(BoxError::from(err))))
            }
            Poll::Ready(None) => {
                this.close(true);
                Poll::Ready(None)
            }
            Poll::Pending => {
                // Gewartet wird auf den Client. Schweigt er, läuft nur seine
                // eigene Uhr ab (`408`, `PROXY_011`), nie die Wache über das
                // Ziel (`LIMIT_004`); beide an derselben Uhr wären ein
                // Wettlauf, den mal die eine, mal die andere gewinnt.
                this.progress.wait_for_client();
                if this.deadline.as_mut().poll(cx).is_pending() {
                    return Poll::Pending;
                }
                tracing::debug!(idle = ?this.idle, "{REQUEST_IDLE_ERROR}");
                this.progress.mark_client_silent();
                this.close(false);
                Poll::Ready(Some(Err(BoxError::from(REQUEST_IDLE_ERROR))))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.prefix.is_none() && self.rest.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        let mut hint = self.rest.size_hint();
        if let Some(prefix) = &self.prefix {
            let extra = u64::try_from(prefix.len()).unwrap_or(u64::MAX);
            hint.set_lower(hint.lower().saturating_add(extra));
            if let Some(upper) = hint.upper() {
                hint.set_upper(upper.saturating_add(extra));
            }
        }
        hint
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
                // Vor dem Abschluss: Der Befund steht am Fluss, bevor er
                // `Recorded` wird (HUM-057).
                if let Some(flow_id) = flow_id {
                    this.queue.publish(FlowEvent::Diagnostic {
                        flow_id: Some(flow_id),
                        at: SystemTime::now(),
                        diagnostic: Box::new(crate::limits::response_went_silent(this.idle)),
                    });
                }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Stall, StreamProgress};

    /// Die Fortschrittsuhr läuft erst, wenn das Senden des Rumpfs beginnt:
    /// Verbindungsaufbau, TLS und Handschlag zählen nicht als Stillstand
    /// (Review 2 zu HUM-057).
    #[test]
    fn the_progress_clock_starts_with_the_first_chunk() {
        let progress = StreamProgress::new();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(progress.stalled(Duration::ZERO, Duration::ZERO), None);
        progress.touch();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(
            progress.stalled(Duration::ZERO, Duration::from_secs(60)),
            Some(Stall::NotReading)
        );
    }

    /// Wartet der Strom auf den Client, ist das kein Stillstand des Ziels,
    /// auch nicht nach der Frist; erst ein neues Stück hebt das auf
    /// (Review 3 zu HUM-057).
    #[test]
    fn waiting_for_the_client_is_never_a_stall_of_the_target() {
        let progress = StreamProgress::new();
        progress.touch();
        progress.wait_for_client();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(
            progress.stalled(Duration::ZERO, Duration::from_secs(60)),
            None
        );
        progress.touch();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(
            progress.stalled(Duration::ZERO, Duration::from_secs(60)),
            Some(Stall::NotReading)
        );
    }
}
