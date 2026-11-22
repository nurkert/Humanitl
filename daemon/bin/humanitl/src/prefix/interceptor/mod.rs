//! Der Interceptor im Terminal der Sitzung, über dem Agenten (GitHub #12).
//!
//! Präfix und `i` öffnen den Bildschirm ([`screen`]) im laufenden
//! Anschluss; `Esc` oder `q` kehren zurück. Der Ablauf ist der der
//! Einstellungen ([`super::settings`]), Schritt für Schritt:
//!
//! 1. **Umschalten.** Der Leser der Tastatur hält an (`prefix::input`), keine
//!    Taste erreicht den Agenten, seine Ausgabe wartet als ganze Stücke im
//!    Client (`prefix::view`), und der eigene Schirm steht. Der Bildschirm
//!    läuft auf einem Faden, der blockieren darf.
//! 2. **Zurückkehren.** Den eigenen Schirm verlassen, die zurückgehaltene
//!    Ausgabe schreiben und den Agenten neu zeichnen lassen
//!    ([`super::redraw`]).
//!
//! Ein lesender Anschluss (`--read-only`) zeigt den Interceptor nur zum
//! Lesen: Er schickt kein `Decide`. Über einem Agenten auf dem
//! Alternativschirm öffnet er ihn gar nicht, sondern klingelt, wie bei den
//! Einstellungen: Er dürfte den Agenten danach nicht neu zeichnen lassen
//! (`cmd/attach.rs`).
//!
//! - [`screen`]: Zustand und Tasten des Bildschirms.
//! - [`draw`]: wie er aussieht.
//! - [`link`]: die Leitung zum Daemon, im Test ein Fake.
//!
//! Der Bildschirm steht hier und nicht in `tui/`: Es gibt ihn nur über einem
//! Agenten, und er nimmt den Säuberer des Kastens von `--ask terminal`
//! ([`crate::ask_terminal::display_text`]) und die Regeln aus
//! `cmd::moderate`; das Fundament in `tui/` kennt beides nicht.
//!
//! Die Leitung zum Daemon ([`spawn`]) hat zwei Aufträge: einer beantwortet
//! die Anfragen des Bildschirms (`GetFlow`, `Decide`), einer liest
//! `Subscribe` und `ListFlows`. Der zweite abonniert zuerst und liest dann
//! die Liste: Eine Anfrage, die dazwischen gehalten wird, steht so in der
//! Liste oder kommt als Ereignis, und doppelt zählt nichts.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use humanitl_ipc::client::Client;
use humanitl_ipc::v1;

pub mod draw;
pub mod link;
pub mod screen;

#[cfg(test)]
mod tests;

use self::link::{Answer, Far, Request};
use self::screen::InterceptorScreen;
use crate::cmd::status_diagnostic;
use crate::tui::Theme;
use crate::tui::backend::{TermOut, TtyBackend};
use crate::tui::screen::{discard_pending_events, run_in_session};

/// Wie lange das Ende des Anschlusses höchstens auf den Faden des
/// Bildschirms wartet: Er sieht alle 50 ms nach, ob er enden soll.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// Wie viele gehaltene Flüsse eine Seite von `ListFlows` trägt.
const LIST_PAGE: u32 = 100;

/// Wie viele gehaltene Flüsse höchstens gelesen werden; dieselbe Grenze
/// wie in `--ask terminal` (`cmd::moderate`).
const LIST_CAP: usize = 1000;

/// Ein laufender Interceptor über dem Agenten.
pub struct InterceptorView {
    /// Der Faden des Bildschirms.
    task: tokio::task::JoinHandle<()>,
    /// Wahr, sobald der Bildschirm enden soll.
    stop: Arc<AtomicBool>,
    /// Die Leitung zum Daemon; endet mit dem Bildschirm.
    _worker: Worker,
}

impl InterceptorView {
    /// Startet den Bildschirm an `client`, der Verbindung des Anschlusses.
    /// `read_only` für einen lesenden Anschluss. `None` ohne Terminal auf
    /// `stdin` und `stdout`.
    #[must_use]
    pub fn start(client: Client, theme: Theme, read_only: bool) -> Option<Self> {
        if !crate::tui::available() {
            return None;
        }
        let (link, worker) = spawn(client);
        let screen = InterceptorScreen::new(link, read_only, screen::system_clock);
        let stop = Arc::new(AtomicBool::new(false));
        let task = tokio::task::spawn_blocking({
            let stop = Arc::clone(&stop);
            move || show(screen, &stop, theme)
        });
        Some(Self {
            task,
            stop,
            _worker: worker,
        })
    }

    /// Wird fertig, wenn der Bildschirm zu Ende ist.
    pub async fn finished(&mut self) {
        let _ = (&mut self.task).await;
    }

    /// Beendet den Bildschirm und wartet, bis sein Faden nicht mehr zeichnet;
    /// höchstens [`STOP_GRACE`].
    pub async fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = tokio::time::timeout(STOP_GRACE, &mut self.task).await;
    }
}

/// Zeigt den Bildschirm, bis er endet; auf dem Faden, der blockieren darf.
/// Ein Fehler des Terminals beendet nur die Ansicht.
fn show(mut screen: InterceptorScreen, stop: &AtomicBool, theme: Theme) {
    let Ok(out) = rustix::stdio::stdout().try_clone_to_owned() else {
        return;
    };
    let out = TermOut::new(out);
    let muted = out.mute_switch();
    let Ok(mut terminal) = ratatui::Terminal::new(TtyBackend::new(out)) else {
        return;
    };
    let _ = run_in_session(&mut terminal, &mut screen, theme, screen::MIN, stop);
    // Den Cursor stellt der Anschluss beim Zurückkehren her, wie bei den
    // Einstellungen.
    muted.store(true, Ordering::SeqCst);
    drop(terminal);
    discard_pending_events();
}

/// Die Aufträge hinter der Leitung; beim Fallenlassen beendet.
pub struct Worker {
    /// Beantwortet die Anfragen.
    calls: tokio::task::JoinHandle<()>,
    /// Liest Liste und Ereignisse.
    events: tokio::task::JoinHandle<()>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.calls.abort();
        self.events.abort();
    }
}

/// Eine Leitung an `client`.
#[must_use]
pub fn spawn(client: Client) -> (link::Link, Worker) {
    let (link, far) = link::pair();
    let Far { mut calls, answers } = far;
    let events = tokio::spawn(watch(client.clone(), answers.clone()));
    let calls = tokio::spawn(async move {
        let mut client = client;
        while let Some(request) = calls.recv().await {
            if answers.send(execute(&mut client, request).await).is_err() {
                return;
            }
        }
    });
    (link, Worker { calls, events })
}

/// Eine Anfrage über gRPC.
async fn execute(client: &mut Client, request: Request) -> Answer {
    match request {
        Request::Detail(flow_id) => match client.get_flow(v1::FlowRef { flow_id }).await {
            Ok(detail) => Answer::Detail(detail.into_inner()),
            Err(status) => Answer::Failed(status_diagnostic(&status, "GetFlow")),
        },
        Request::Decide(decide) => match client.decide(*decide).await {
            Ok(response) => Answer::Decided(response.into_inner()),
            Err(status) => Answer::Failed(status_diagnostic(&status, "Decide")),
        },
    }
}

/// Abonniert die Ereignisse, liest dann die Liste und reicht danach jede
/// Änderung weiter: `Held` (über `GetFlow` zur Zeile aufgelöst), `Decided`
/// und `TimedOut` als verschwunden, `Lagged` als neue Liste.
async fn watch(mut client: Client, answers: std::sync::mpsc::Sender<Answer>) {
    use v1::flow_event::Event;

    let stream = client
        .subscribe(v1::SubscribeRequest {
            since_flow_id: String::new(),
            include_passthrough: false,
            // Die Kommandozeile ist nicht die Desktop-App (HUM-539).
            desktop_app: false,
        })
        .await;
    let mut stream = match stream {
        Ok(stream) => stream.into_inner(),
        Err(status) => {
            let _ = answers.send(Answer::Failed(status_diagnostic(&status, "Subscribe")));
            let _ = answers.send(Answer::LiveEnded);
            return;
        }
    };
    if answers.send(list(&mut client).await).is_err() {
        return;
    }
    while let Ok(Some(event)) = stream.message().await {
        let answer = match event.event {
            Some(Event::Held(held)) => match arrived(&mut client, held.flow_id).await {
                Some(answer) => answer,
                None => continue,
            },
            Some(Event::Decided(decided)) => Answer::Gone(decided.flow_id),
            Some(Event::TimedOut(reference)) => Answer::Gone(reference.flow_id),
            // Was verworfen wurde, kann ein `Held` oder ein `Decided` gewesen
            // sein: Die Liste des Daemons gilt wieder ganz.
            Some(Event::Lagged(_)) => list(&mut client).await,
            _ => continue,
        };
        if answers.send(answer).is_err() {
            return;
        }
    }
    let _ = answers.send(Answer::LiveEnded);
}

/// Die Zeile eines neu gehaltenen Flusses, wenn er noch gehalten ist.
async fn arrived(client: &mut Client, flow_id: String) -> Option<Answer> {
    match client.get_flow(v1::FlowRef { flow_id }).await {
        Ok(detail) => detail
            .into_inner()
            .summary
            .filter(|summary| summary.state == v1::FlowState::Held as i32)
            .map(Answer::Arrived),
        Err(status) => Some(Answer::Failed(status_diagnostic(&status, "GetFlow"))),
    }
}

/// Alle gehaltenen Flüsse, ältester zuerst, bis zum Ende gelesen und
/// höchstens [`LIST_CAP`].
async fn list(client: &mut Client) -> Answer {
    let mut held: Vec<v1::FlowSummary> = Vec::new();
    let mut cursor = String::new();
    loop {
        let page = match client
            .list_flows(v1::ListFlowsRequest {
                filter: "state:held".to_owned(),
                order_by: "received_at asc".to_owned(),
                cursor: cursor.clone(),
                limit: LIST_PAGE,
                ..v1::ListFlowsRequest::default()
            })
            .await
        {
            Ok(page) => page.into_inner(),
            Err(status) => return Answer::Failed(status_diagnostic(&status, "ListFlows")),
        };
        let empty = page.flows.is_empty();
        held.extend(page.flows);
        // Dieselben vier Enden wie in `--ask terminal`: Ein Dienst, der eine
        // leere Seite mit Zeiger oder denselben Zeiger noch einmal schickt,
        // hielte sonst diese Schleife fest.
        if empty
            || page.next_cursor.is_empty()
            || page.next_cursor == cursor
            || held.len() >= LIST_CAP
        {
            break;
        }
        cursor = page.next_cursor;
    }
    held.truncate(LIST_CAP);
    Answer::Listed(held)
}
