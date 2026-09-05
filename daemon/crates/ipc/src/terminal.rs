//! Das Terminal der Sandbox: ein Schreiber, beliebig viele Leser (HUM-042).
//!
//! Der Daemon startet die Sitzung an einem Pseudoterminal
//! ([`humanitl_sandbox::StdioMode::Pty`]) und ist damit die einzige Stelle,
//! die dessen Herrscherseite hält. Dieses Modul verteilt sie: Es filtert die
//! Ausgabe **einmal**, hebt die letzten [`RING_BYTES`] davon auf und schickt
//! sie an jeden, der zusieht.
//!
//! # Warum ein Nabe und nicht ein Strom je Client
//!
//! Ein Pseudoterminal hat genau einen Leser. Läsen zwei Clients selbst, bekäme
//! jeder zufällige Hälften des Stroms. Der [`TerminalHub`] liest deshalb an
//! einer Stelle, und die Clients hängen an einem `broadcast`.
//!
//! Das ist zugleich die Stelle, an der der Filter sitzt. Der Ringpuffer hält
//! **gefilterte** Bytes: Hielte er den Rohstrom, spielte ein zweiter Client
//! beim Anhängen genau die Folgen ab, die der Filter dem ersten
//! herausgenommen hat (`docs/SECURITY.md` 3.3).
//!
//! # Ein Schreiber
//!
//! Die Geometrie eines Terminals gehört dem, der darin tippt
//! (`backlog/CONVENTIONS.md` 4.10). Ein zweiter Client mit
//! `Open { read_only: false }` bekommt deshalb `TERM_001` und sonst nichts;
//! Leser werden immer angenommen, ihre Eingabe und ihre Größenwünsche werden
//! **hier** verworfen und nicht im Client — die Grenze gehört auf die Seite,
//! die sie durchsetzen kann.
//!
//! # Der Hinweis bei gehaltenem Fluss
//!
//! Wenn eine Anfrage des Agenten auf einen Menschen wartet, schreibt der
//! Daemon eine Zeile in denselben Strom ([`HeldNotices`]). Sie kommt aus dem
//! Ereignisstrom, den ohnehin alle lesen (`docs/ARCHITECTURE.md` 1.2:
//! „Niemand fragt den Proxy nach seinem Zustand, alle hören zu"), und nicht
//! aus einem neuen Kanal vom Proxy zum Terminal.
//!
//! Diese Zeile ist die schärfste feindliche Eingabe dieses Moduls: Sie setzt
//! Text des Agenten (`HttpRequest.path_and_query`, roh von der Leitung) in
//! eine Zeile mit Humanitl-Absender, und sie wird **am Filter vorbei**
//! geschrieben. Deshalb läuft sie als Ganzes durch
//! [`humanitl_core::block::sanitize_note`], der Pfad wird vorher gekürzt und
//! um die eckige Klammer gebracht ([`path_for_notice`]), und eingefügt wird
//! nur an einer Grenze ([`TerminalFilter::at_boundary`]) — sonst fiele der
//! Hinweis mitten in eine halb geschriebene Folge des Agenten.
//!
//! **Was das nicht deckt, und warum es kein Loch ist:** Der Agent kann
//! jederzeit selbst `[humanitl] request allowed` auf seine eigene
//! Standardausgabe schreiben. Ein Absender in einem Bytestrom ist keine
//! Beglaubigung, und dieses Modul kann daraus keine machen. Deshalb hängt das
//! Akzeptanzkriterium des Issues am Streifen **über** dem Terminal, den die
//! Oberfläche aus demselben Ereignis zeichnet, und deshalb steht über dem
//! Terminal dauerhaft, dass die Ausgabe des Agenten nicht vertrauenswürdig
//! ist. Die Zeile im Strom ist eine Bequemlichkeit für den, der am Terminal
//! sitzt, und `ui.terminal_notices` schaltet sie ab.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes::{TERM_001, TERM_002};
use humanitl_core::ids::SandboxId;
use humanitl_core::{
    Decision, Diagnostic, FlowEvent, FlowId, Severity, TerminalFilter, TerminalPolicy,
};
use humanitl_proxy::{FlowRegistry, HoldQueue};
use humanitl_sandbox::SandboxHandle;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::ReceiverStream;

use crate::server_stub::BoxStream;
use crate::v1;

/// Wie viele gefilterte Bytes ein Anhängender nachgeliefert bekommt.
///
/// Genug für den sichtbaren Schirm samt etwas Vorlauf, wenig genug, dass es je
/// Sitzung nicht zählt. Was länger her ist, gehört in die Aufzeichnung und
/// nicht in einen Terminalpuffer.
pub const RING_BYTES: usize = 64 * 1024;

/// Wie oft die Geometrie des Schreibers höchstens durchgereicht wird.
///
/// Wer ein Fenster zieht, erzeugt hundert Größen in einer Sekunde. Jede davon
/// wäre ein `SIGWINCH` an den Agenten und ein Neuzeichnen des ganzen Schirms.
/// Die letzte gewinnt.
pub const RESIZE_INTERVAL: Duration = Duration::from_millis(50);

/// Wie viele Nachrichten ein Client hinter sein darf, bevor er Bytes verliert.
const FRAME_BUFFER: usize = 256;

/// Wie viele Zeichen des Pfades der Hinweis zeigt.
///
/// Der Pfad kommt roh von der Leitung. Er wird gekürzt, **bevor** ein Byte
/// davon in den Strom geht: Eine Zeile, die den halben Schirm füllt, ist keine
/// Meldung mehr, sondern eine Verdrängung.
pub const NOTICE_PATH_CHARS: usize = 48;

/// Der Absender jeder Zeile, die der Daemon selbst in das Terminal schreibt.
const NOTICE_PREFIX: &str = "[humanitl] ";

/// Was an alle Clients eines Terminals geht.
#[derive(Debug, Clone)]
enum Frame {
    /// Gefilterte Bytes des Agenten oder eine Zeile des Daemons.
    Data(Arc<[u8]>),
    /// Die Geometrie des Schreibers; Leser rendern letterboxed.
    Resize { cols: u16, rows: u16 },
    /// Ein Befund, der zu diesem Terminal gehört.
    Note(Arc<Diagnostic>),
    /// Der Agent ist beendet.
    Exit(i32),
}

/// Der Zustand eines Terminals, hinter einem Schloss.
#[derive(Debug)]
struct HubState {
    /// Der eine Filter dieses Stroms.
    filter: TerminalFilter,
    /// Die letzten [`RING_BYTES`] **gefilterter** Bytes.
    ring: VecDeque<u8>,
    /// Die Geometrie des Schreibers.
    cols: u16,
    rows: u16,
    /// Ob ein Schreiber angemeldet ist.
    writer: bool,
    /// Eine Zeile, die auf die nächste Grenze wartet.
    pending: Vec<u8>,
    /// Der Exit-Code, sobald der Agent beendet ist.
    finished: Option<i32>,
}

/// Der geteilte Zustand eines Terminals.
#[derive(Debug)]
struct HubInner {
    sandbox: SandboxId,
    handle: Arc<SandboxHandle>,
    frames: broadcast::Sender<Frame>,
    state: Mutex<HubState>,
    /// Ob der Daemon Hinweiszeilen in den Strom schreibt
    /// (`ui.terminal_notices`).
    notices: bool,
}

/// Das Terminal einer laufenden Sitzung.
///
/// Billig zu klonen; der Zustand liegt hinter einem `Arc`.
#[derive(Debug, Clone)]
pub struct TerminalHub {
    inner: Arc<HubInner>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl TerminalHub {
    /// Ein Terminal über der Herrscherseite dieser Sandbox.
    #[must_use]
    pub fn new(handle: Arc<SandboxHandle>, cols: u16, rows: u16, notices: bool) -> Self {
        let (frames, _) = broadcast::channel(FRAME_BUFFER);
        Self {
            inner: Arc::new(HubInner {
                sandbox: handle.id,
                handle,
                frames,
                state: Mutex::new(HubState {
                    filter: TerminalFilter::with_policy(TerminalPolicy::FullScreen),
                    ring: VecDeque::new(),
                    cols,
                    rows,
                    writer: false,
                    pending: Vec::new(),
                    finished: None,
                }),
                notices,
            }),
        }
    }

    /// Die Sandbox, zu der dieses Terminal gehört.
    #[must_use]
    pub fn sandbox(&self) -> SandboxId {
        self.inner.sandbox
    }

    /// Ob der Daemon Hinweiszeilen in diesen Strom schreibt.
    #[must_use]
    pub fn notices(&self) -> bool {
        self.inner.notices
    }

    /// Ob der Strom gerade zwischen zwei Folgen und zwei Zeichen steht.
    ///
    /// Nur dann geht eine Hinweiszeile sofort hinaus; sonst wartet sie auf die
    /// nächste Grenze ([`TerminalHub::notice`]).
    #[must_use]
    pub fn at_boundary(&self) -> bool {
        lock(&self.inner.state).filter.at_boundary()
    }

    /// Ein Stück roher Ausgabe des Agenten.
    ///
    /// Es wird gefiltert, in den Ring gelegt und verschickt — in dieser
    /// Reihenfolge und unter demselben Schloss, unter dem ein Anhängender den
    /// Ring abschreibt. Ohne das gäbe es zwischen „Ring lesen" und
    /// „Rundfunk abonnieren" eine Lücke oder eine Wiederholung.
    pub fn feed(&self, raw: &[u8]) {
        let mut state = lock(&self.inner.state);
        let filtered = state.filter.push(raw);
        if !filtered.is_empty() {
            self.emit(&mut state, &filtered);
        }
        // Ein Hinweis, der auf eine Grenze gewartet hat, geht jetzt hinaus.
        if state.filter.at_boundary() && !state.pending.is_empty() {
            let pending = std::mem::take(&mut state.pending);
            self.emit(&mut state, &pending);
        }
    }

    /// Eine Zeile des Daemons, gesäubert und nur an einer Grenze.
    pub fn notice(&self, line: &str) {
        if !self.inner.notices {
            return;
        }
        let bytes = notice_line(line);
        let mut state = lock(&self.inner.state);
        if state.filter.at_boundary() {
            self.emit(&mut state, &bytes);
        } else {
            // Mitten in einer Folge des Agenten: warten. Der Hinweis ist
            // nichts wert, wenn er eine halbe Folge zerschneidet — das
            // Terminal führte den Rest dann als Text aus.
            state.pending.extend_from_slice(&bytes);
        }
    }

    /// Ein Befund, den jeder Client sehen soll.
    pub fn diagnostic(&self, diagnostic: &Diagnostic) {
        let _ = self
            .inner
            .frames
            .send(Frame::Note(Arc::new(diagnostic.clone())));
    }

    /// Der Agent ist beendet; jeder Strom endet mit dieser Zahl.
    ///
    /// Ein Hinweis, der auf eine Grenze gewartet hat, geht vorher noch hinaus.
    /// Nach dem `flush` steht der Filter wieder auf einer Grenze, und der
    /// Agent schreibt nichts mehr, in das der Hinweis fallen könnte — ihn hier
    /// wegzuwerfen hieße, den letzten gehaltenen Fluss zu verschweigen, und
    /// das ist genau der, der beim Ende noch offen war.
    pub fn finish(&self, code: i32) {
        {
            let mut state = lock(&self.inner.state);
            // Was der Filter noch zurückhält, war eine angefangene Folge und
            // geht nicht hinaus.
            let _ = state.filter.flush();
            if !state.pending.is_empty() {
                let pending = std::mem::take(&mut state.pending);
                self.emit(&mut state, &pending);
            }
            state.finished = Some(code);
        }
        let _ = self.inner.frames.send(Frame::Exit(code));
    }

    /// Setzt die Geometrie und sagt es allen.
    ///
    /// Der Befund eines Terminals, das nicht mehr antwortet, geht an die
    /// Clients und nicht in ein Log: Wer gerade das Fenster zieht, soll
    /// erfahren, dass niemand mehr zusieht.
    pub fn resize(&self, cols: u16, rows: u16) {
        if let Err(diagnostic) = self.inner.handle.resize(cols, rows) {
            self.diagnostic(&diagnostic);
            return;
        }
        {
            let mut state = lock(&self.inner.state);
            state.cols = cols;
            state.rows = rows;
        }
        let _ = self.inner.frames.send(Frame::Resize { cols, rows });
    }

    /// Schreibt Tastendrücke an den Agenten.
    ///
    /// # Errors
    ///
    /// `TERM_002`, wenn das Terminal nicht mehr annimmt.
    pub fn write(&self, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.inner.handle.write_input(bytes)
    }

    /// Meldet einen Client an.
    ///
    /// # Errors
    ///
    /// `TERM_001`, wenn schon ein Schreiber angemeldet ist.
    pub fn attach(&self, read_only: bool) -> Result<Attachment, Diagnostic> {
        let (backlog, frames, cols, rows, finished) = {
            let mut state = lock(&self.inner.state);
            if !read_only {
                if state.writer {
                    return Err(Diagnostic::builder(TERM_001, Severity::Error)
                        .why(format!(
                            "sandbox {} already has a terminal client that writes",
                            self.inner.sandbox
                        ))
                        .fix(humanitl_core::FixAction::CopyCommand(
                            "humanitl sandbox attach --read-only".to_owned(),
                        ))
                        .build());
                }
                state.writer = true;
            }
            let backlog: Vec<u8> = state.ring.iter().copied().collect();
            (
                backlog,
                self.inner.frames.subscribe(),
                state.cols,
                state.rows,
                state.finished,
            )
        };
        Ok(Attachment {
            backlog,
            frames,
            cols,
            rows,
            finished,
            writer: (!read_only).then(|| WriterSlot { hub: self.clone() }),
        })
    }

    /// Legt Bytes in den Ring und verschickt sie.
    fn emit(&self, state: &mut HubState, bytes: &[u8]) {
        state.ring.extend(bytes.iter().copied());
        let over = state.ring.len().saturating_sub(RING_BYTES);
        drop(state.ring.drain(..over));
        let _ = self.inner.frames.send(Frame::Data(Arc::from(bytes)));
    }

    /// Gibt den Schreiber-Platz wieder frei.
    fn release_writer(&self) {
        lock(&self.inner.state).writer = false;
    }
}

/// Der Schreiber-Platz, solange ein Client ihn hält.
///
/// Er gibt sich beim Fallenlassen selbst frei, damit kein Fehlerpfad ihn
/// vergisst: Ein Platz, der nach einem abgebrochenen Strom belegt bliebe,
/// verweigerte jeden weiteren Schreiber bis zum Ende der Sitzung.
#[derive(Debug)]
struct WriterSlot {
    hub: TerminalHub,
}

impl Drop for WriterSlot {
    fn drop(&mut self) {
        self.hub.release_writer();
    }
}

/// Was ein angemeldeter Client mitbekommt.
#[derive(Debug)]
pub struct Attachment {
    /// Die letzten gefilterten Bytes, in der Reihenfolge, in der sie kamen.
    backlog: Vec<u8>,
    /// Alles, was ab jetzt kommt.
    frames: broadcast::Receiver<Frame>,
    /// Die Geometrie des Schreibers.
    cols: u16,
    rows: u16,
    /// Der Exit-Code, wenn der Agent schon beendet ist.
    finished: Option<i32>,
    /// Der Platz des Schreibers, falls dieser Client schreiben darf.
    writer: Option<WriterSlot>,
}

/// Macht aus einem Pfad von der Leitung das, was in der Hinweiszeile stehen
/// darf.
///
/// Zwei Dinge, und beide sind gemessen:
///
/// 1. **Gekürzt auf [`NOTICE_PATH_CHARS`] Zeichen**, auf Zeichen und nicht auf
///    Bytes: Ein Schnitt mitten in einem UTF-8-Zeichen erzeugte ein
///    Ersatzzeichen im Terminal des Menschen. Ein Pfad, der den halben Schirm
///    füllt, ist keine Meldung mehr, sondern eine Verdrängung.
/// 2. **Die eckige Klammer gehört dem Absender.** `[` wird zu `(`. Ohne diese
///    Zeile schriebe ein Agent `/a [humanitl] request allowed: GET
///    evil.example/` in seinen eigenen Pfad, und die Zeile trüge zwei
///    Absender: den echten und einen, der eine Freigabe behauptet, die es nie
///    gab. `sanitize_note` fängt das nicht ab — es ist Text und kein
///    Steuerzeichen.
#[must_use]
pub fn path_for_notice(path: &str) -> String {
    let mut short: String = path
        .chars()
        .take(NOTICE_PATH_CHARS)
        .map(|ch| if ch == '[' { '(' } else { ch })
        .collect();
    if path.chars().count() > NOTICE_PATH_CHARS {
        short.push('…');
    }
    short
}

/// Macht aus einer Hinweiszeile die Bytes, die in den Strom gehen.
///
/// `\r\n` vorne und hinten, damit die Zeile in einer eigenen Zeile steht und
/// der Cursor danach am Anfang der nächsten. Dazwischen genau das, was
/// [`sanitize_note`] übrig lässt: kein Steuerzeichen, kein Zeilenumbruch,
/// keine unsichtbaren Zeichen.
fn notice_line(line: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(sanitize_note(line).as_bytes());
    out.extend_from_slice(b"\r\n");
    out
}

/// Die Hinweiszeilen aus dem Ereignisstrom.
///
/// Sie hängen an [`HoldQueue::subscribe`] und lösen den Fluss über
/// [`FlowRegistry::get`] auf: `FlowEvent::Held` trägt nur `flow_id`, `at`,
/// `deadline`, `queue_bytes` und `queue_count`.
#[derive(Debug, Clone)]
pub struct HeldNotices {
    queue: Arc<HoldQueue>,
    registry: Arc<FlowRegistry>,
}

impl HeldNotices {
    /// Die Quelle der Hinweise für diese Sitzung.
    #[must_use]
    pub const fn new(queue: Arc<HoldQueue>, registry: Arc<FlowRegistry>) -> Self {
        Self { queue, registry }
    }

    /// Schreibt Hinweise in dieses Terminal, bis der Agent endet.
    pub async fn run(self, hub: TerminalHub) {
        let mut events = self.queue.subscribe();
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                // Wer zu langsam liest, verliert Ereignisse. Für einen Hinweis
                // ist das folgenlos: Der nächste kommt, und die Warteschlange
                // selbst steht in der Oberfläche.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            };
            if let Some(line) = self.line_for(&event) {
                hub.notice(&line);
            }
        }
    }

    /// Die Zeile zu einem Ereignis, wenn es eine gibt.
    fn line_for(&self, event: &FlowEvent) -> Option<String> {
        let (flow_id, verb, tail) = match event {
            FlowEvent::Held { flow_id, .. } => (*flow_id, "held", " · waiting for you"),
            FlowEvent::Decided {
                flow_id, decision, ..
            } => (
                *flow_id,
                match decision {
                    Decision::Allow | Decision::AllowEdited { .. } => "allowed",
                    Decision::Block { .. } => "blocked",
                    Decision::TimedOut => "timed out",
                },
                "",
            ),
            FlowEvent::TimedOut { flow_id, .. } => (*flow_id, "timed out", ""),
            _ => return None,
        };
        Some(self.line(flow_id, verb, tail))
    }

    /// `[humanitl] request held: GET example.com/path · waiting for you`.
    ///
    /// Der Pfad wird gekürzt, bevor er in die Zeile kommt; die ganze Zeile
    /// läuft danach durch [`sanitize_note`] (in [`notice_line`]). Kennt die
    /// Registry den Fluss nicht mehr, steht dort seine Kennung — eine Zeile
    /// ohne Ziel ist immer noch wahr.
    fn line(&self, flow_id: FlowId, verb: &str, tail: &str) -> String {
        let target = self.registry.get(flow_id).map_or_else(
            || format!("flow {flow_id}"),
            |record| {
                format!(
                    "{} {}{}",
                    record.request.method,
                    record.request.authority.host,
                    path_for_notice(&record.request.path_and_query)
                )
            },
        );
        format!("{NOTICE_PREFIX}request {verb}: {target}{tail}")
    }
}

/// Bedient einen Terminal-Strom des Vertrags.
///
/// Der Strom endet, wenn der Client `close` schickt, wenn der Agent sich
/// beendet oder wenn ein Befund ihn beendet. Er schließt nie das
/// Pseudoterminal: Ein Client, der geht, nimmt die Sitzung nicht mit.
#[must_use]
pub fn serve(
    open: impl FnOnce(&str) -> Result<TerminalHub, Diagnostic> + Send + 'static,
    input: BoxStream<v1::TerminalInput>,
) -> BoxStream<v1::TerminalOutput> {
    let (tx, rx) = mpsc::channel(FRAME_BUFFER);
    tokio::spawn(async move { session(open, input, tx).await });
    Box::pin(ReceiverStream::new(rx))
}

/// Ein Client, von seinem `Open` bis zu seinem Ende.
async fn session(
    open: impl FnOnce(&str) -> Result<TerminalHub, Diagnostic>,
    mut input: BoxStream<v1::TerminalInput>,
    tx: mpsc::Sender<v1::TerminalOutput>,
) {
    let Some(opened) = first_open(&mut input).await else {
        return;
    };

    let hub = match open(&opened.sandbox_id) {
        Ok(hub) => hub,
        Err(diagnostic) => {
            let _ = tx.send(diagnostic_output(&diagnostic)).await;
            return;
        }
    };
    let read_only = opened.read_only;
    let attachment = match hub.attach(read_only) {
        Ok(attachment) => attachment,
        Err(diagnostic) => {
            let _ = tx.send(diagnostic_output(&diagnostic)).await;
            return;
        }
    };
    let Attachment {
        backlog,
        mut frames,
        mut cols,
        mut rows,
        finished,
        writer,
    } = attachment;

    // Die Geometrie des Schreibers gilt, bevor der Rückstand abläuft: Sonst
    // liefe er durch ein Raster, das gleich darauf ein anderes ist.
    let resizes = if writer.is_some() {
        let wish = geometry(&opened, cols, rows);
        cols = wish.0;
        rows = wish.1;
        hub.resize(cols, rows);
        let (tx, rx) = watch::channel(wish);
        tokio::spawn(resize_pump(hub.clone(), rx));
        Some(tx)
    } else {
        None
    };

    if tx.send(resize_output(cols, rows)).await.is_err() {
        return;
    }
    if !backlog.is_empty() && tx.send(data_output(&backlog)).await.is_err() {
        return;
    }
    if let Some(code) = finished {
        let _ = tx.send(exit_output(code)).await;
        return;
    }

    forward(
        &hub,
        &mut frames,
        &mut input,
        &tx,
        writer.is_some(),
        resizes.as_ref(),
    )
    .await;
}

/// Wartet auf das `Open`, mit dem eine Sitzung anfängt.
///
/// Alles davor gehört niemandem: Ein `data` ohne Sitzung hat keinen
/// Empfänger, und ein `close` beendet den Strom, ohne dass je einer angefangen
/// hätte.
async fn first_open(input: &mut BoxStream<v1::TerminalInput>) -> Option<v1::terminal_input::Open> {
    use v1::terminal_input::Input;

    loop {
        match input.next().await {
            Some(v1::TerminalInput {
                input: Some(Input::Open(open)),
            }) => return Some(open),
            Some(v1::TerminalInput {
                input: Some(Input::Close(())),
            })
            | None => return None,
            Some(_) => {}
        }
    }
}

/// Der Betrieb eines angemeldeten Clients: hinaus, was kommt, hinauf, was er
/// tippt.
async fn forward(
    hub: &TerminalHub,
    frames: &mut broadcast::Receiver<Frame>,
    input: &mut BoxStream<v1::TerminalInput>,
    tx: &mpsc::Sender<v1::TerminalOutput>,
    writes: bool,
    resizes: Option<&watch::Sender<(u16, u16)>>,
) {
    use v1::terminal_input::Input;

    loop {
        tokio::select! {
            frame = frames.recv() => match frame {
                Ok(Frame::Data(bytes)) => {
                    if tx.send(data_output(&bytes)).await.is_err() {
                        return;
                    }
                }
                Ok(Frame::Resize { cols, rows }) => {
                    if tx.send(resize_output(cols, rows)).await.is_err() {
                        return;
                    }
                }
                Ok(Frame::Note(diagnostic)) => {
                    if tx.send(diagnostic_output(&diagnostic)).await.is_err() {
                        return;
                    }
                }
                Ok(Frame::Exit(code)) => {
                    let _ = tx.send(exit_output(code)).await;
                    return;
                }
                // Wer zu langsam liest, verliert Bytes. Der Strom läuft
                // weiter: Ein Vollbild-Agent zeichnet den Schirm im nächsten
                // Bild ohnehin neu, und ein abgebrochener Strom wäre der
                // größere Verlust.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            },
            message = input.next() => match message.and_then(|message| message.input) {
                Some(Input::Data(bytes)) => {
                    // Die Grenze des Lesers steht hier und nicht im Client:
                    // Wer nur zusieht, dessen Tastendrücke fallen weg.
                    if writes && let Err(diagnostic) = write_input(hub, bytes).await {
                        let _ = tx.send(diagnostic_output(&diagnostic)).await;
                        return;
                    }
                }
                Some(Input::Resize(resize)) => {
                    if let Some(resizes) = resizes {
                        let _ = resizes.send(clamp(resize.cols, resize.rows));
                    }
                }
                // `close` beendet den Strom, nicht das Terminal.
                Some(Input::Close(())) | None => return,
                // Ein zweites `Open` ist keine zweite Sitzung.
                Some(Input::Open(_)) => {}
            },
        }
    }
}

/// Schreibt Tastendrücke, ohne die Ereignisschleife zu blockieren.
///
/// Das Schreiben auf die Herrscherseite blockiert, wenn der Agent nicht liest
/// und der Puffer des Terminals voll ist; es gehört deshalb auf einen Faden,
/// der blockieren darf.
async fn write_input(hub: &TerminalHub, bytes: Vec<u8>) -> Result<(), Diagnostic> {
    let hub = hub.clone();
    match tokio::task::spawn_blocking(move || hub.write(&bytes)).await {
        Ok(result) => result,
        Err(error) => Err(Diagnostic::builder(TERM_002, Severity::Error)
            .why(format!("the terminal write did not finish: {error}"))
            .build()),
    }
}

/// Reicht die Geometrie des Schreibers weiter, höchstens eine je
/// [`RESIZE_INTERVAL`], die letzte gewinnt.
async fn resize_pump(hub: TerminalHub, mut wishes: watch::Receiver<(u16, u16)>) {
    while wishes.changed().await.is_ok() {
        let (cols, rows) = *wishes.borrow_and_update();
        hub.resize(cols, rows);
        tokio::time::sleep(RESIZE_INTERVAL).await;
    }
}

/// Die Geometrie aus einem `Open`, auf brauchbare Werte gebracht.
fn geometry(open: &v1::terminal_input::Open, cols: u16, rows: u16) -> (u16, u16) {
    let wish = clamp(open.cols, open.rows);
    (
        if wish.0 == 0 { cols } else { wish.0 },
        if wish.1 == 0 { rows } else { wish.1 },
    )
}

/// Eine Geometrie von der Leitung, auf das gebracht, was ein Terminal kann.
///
/// `0` heißt „nicht gesetzt" und bleibt `0`; der Aufrufer entscheidet, was
/// dann gilt. Alles über [`u16::MAX`] ist keine Fenstergröße, sondern eine
/// Zahl aus einem fremden Feld.
fn clamp(cols: u32, rows: u32) -> (u16, u16) {
    (
        u16::try_from(cols).unwrap_or(u16::MAX),
        u16::try_from(rows).unwrap_or(u16::MAX),
    )
}

fn data_output(bytes: &[u8]) -> v1::TerminalOutput {
    v1::TerminalOutput {
        output: Some(v1::terminal_output::Output::Data(bytes.to_vec())),
    }
}

fn resize_output(cols: u16, rows: u16) -> v1::TerminalOutput {
    v1::TerminalOutput {
        output: Some(v1::terminal_output::Output::Resize(
            v1::terminal_output::Resize {
                cols: u32::from(cols),
                rows: u32::from(rows),
            },
        )),
    }
}

fn exit_output(code: i32) -> v1::TerminalOutput {
    v1::TerminalOutput {
        output: Some(v1::terminal_output::Output::Exit(
            v1::terminal_output::Exit { code },
        )),
    }
}

fn diagnostic_output(diagnostic: &Diagnostic) -> v1::TerminalOutput {
    v1::TerminalOutput {
        output: Some(v1::terminal_output::Output::Diagnostic(
            crate::convert::diagnostic_to_proto(diagnostic),
        )),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;
    use std::time::{Instant, SystemTime};

    use humanitl_config::Limits;
    use humanitl_core::{
        Authority, Flow, FlowEvent, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
    };
    use humanitl_proxy::connect::ConnectionContext;
    use humanitl_proxy::registry::FlowRecord;
    use humanitl_proxy::{FlowRegistry, HoldQueue};

    use super::{HeldNotices, NOTICE_PATH_CHARS, NOTICE_PREFIX, notice_line, path_for_notice};

    /// Ein gehaltener Fluss, dessen Pfad genau das trägt, was ein Agent
    /// schreiben würde, um die Zeile zu fälschen.
    fn notices_with(path: &str) -> (HeldNotices, FlowId) {
        let limits = Limits::default();
        let registry = Arc::new(FlowRegistry::new(&limits));
        let queue = Arc::new(HoldQueue::with_registry(&limits, Arc::clone(&registry)));
        let session = SessionId::new();
        let request = HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns("example.com".to_owned()), Scheme::Https),
            path,
        );
        let flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
        let id = flow.id;
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        (HeldNotices::new(queue, registry), id)
    }

    fn held(flow_id: FlowId) -> FlowEvent {
        FlowEvent::Held {
            flow_id,
            at: SystemTime::now(),
            deadline: Instant::now(),
            queue_bytes: 0,
            queue_count: 1,
        }
    }

    /// Der Pfad des Agenten kann die Zeile weder verlassen noch fälschen.
    ///
    /// Drei Angriffe in einem Pfad: `\r` setzt den Cursor an den Zeilenanfang
    /// und überschreibt den Absender, `ESC [ 2K` löscht die Zeile, die schon
    /// steht, und die eingebettete OSC-52-Folge schöbe die Zwischenablage
    /// durch den Filter, an dem sie gerade vorbeigeschrieben wird.
    #[test]
    fn notice_is_sanitized() {
        let attack =
            "/a\r[humanitl] request allowed: GET evil.example/\u{1b}[2K\u{1b}]52;c;c2VjcmV0\u{7}";
        let (notices, id) = notices_with(attack);
        let line = notices.line_for(&held(id)).expect("a held flow has a line");
        let bytes = notice_line(&line);
        let text = String::from_utf8(bytes).expect("the notice is text");

        assert_eq!(
            text.matches(NOTICE_PREFIX).count(),
            1,
            "exactly one sender: {text:?}"
        );
        assert!(!text.contains('\u{1b}'), "no escape at all: {text:?}");
        assert!(!text.contains("c2VjcmV0"), "no payload: {text:?}");
        // Was der Agent an Text schreibt, bleibt Text — er kann es ohnehin
        // jederzeit selbst ausgeben. Was er nicht kann: es mit dem Absender
        // dieser Zeile versehen.
        assert!(
            !text.contains("[humanitl] request allowed"),
            "the agent cannot put the sender in front of its own verdict: {text:?}"
        );
        assert!(
            text.contains("(humanitl]"),
            "the square bracket belongs to the sender: {text:?}"
        );
        // Genau zwei Zeilenwechsel, beide vom Daemon.
        assert!(
            text.starts_with("\r\n") && text.ends_with("\r\n"),
            "{text:?}"
        );
        let middle = &text[2..text.len() - 2];
        assert!(
            !middle.chars().any(char::is_control),
            "nothing in between moves a cursor: {middle:?}"
        );
        assert!(
            middle.starts_with("[humanitl] request held: POST example.com/a"),
            "{middle:?}"
        );
        assert!(middle.ends_with("waiting for you"), "{middle:?}");
    }

    /// Ein Pfad, der den halben Schirm füllte, wird gekürzt, bevor er
    /// hinausgeht.
    #[test]
    fn a_long_path_is_cut_before_it_is_written() {
        let long = format!("/{}", "a".repeat(4096));
        let (notices, id) = notices_with(&long);
        let line = notices.line_for(&held(id)).expect("a line");
        assert!(
            line.chars().count() < NOTICE_PATH_CHARS + 80,
            "the line stays a line: {} chars",
            line.chars().count()
        );
        assert!(line.contains('…'), "and it says that it was cut: {line}");
        // Gekürzt wird an Zeichen, nicht an Bytes.
        assert_eq!(path_for_notice("äöü").chars().count(), 3);
        let wide: String = "ä".repeat(NOTICE_PATH_CHARS + 5);
        assert_eq!(
            path_for_notice(&wide).chars().count(),
            NOTICE_PATH_CHARS + 1,
            "one ellipsis, no broken character"
        );
    }

    /// Ein Fluss, den die Registry nicht mehr kennt, bekommt trotzdem eine
    /// wahre Zeile.
    #[test]
    fn a_forgotten_flow_still_gets_a_line() {
        let (notices, _) = notices_with("/x");
        let line = notices
            .line_for(&held(FlowId::new()))
            .expect("an unknown flow is still an event");
        assert!(line.starts_with(NOTICE_PREFIX), "{line}");
        assert!(line.contains("flow "), "{line}");
    }

    /// Nur die drei Ereignisse, die einen Menschen betreffen, erzeugen eine
    /// Zeile. Sonst schriebe der Daemon in das Terminal, sooft ein Byte einer
    /// Antwort durchläuft.
    #[test]
    fn only_the_events_a_human_waits_for_get_a_line() {
        let (notices, id) = notices_with("/x");
        assert!(notices.line_for(&held(id)).is_some());
        assert!(
            notices
                .line_for(&FlowEvent::ResponseChunk {
                    flow_id: id,
                    at: SystemTime::now(),
                    len: 12,
                })
                .is_none()
        );
        assert!(
            notices
                .line_for(&FlowEvent::TimedOut {
                    flow_id: id,
                    at: SystemTime::now(),
                })
                .expect("a timeout is an answer too")
                .contains("timed out")
        );
    }
}
