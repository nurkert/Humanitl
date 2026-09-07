//! `--ask terminal`: die Moderation im selben Terminal (HUM-067).
//!
//! Der Prompt selbst ist reine Zustandslogik ([`crate::ask_terminal`]); hier
//! steht das, was ihn mit dem Daemon und mit dem Terminal des Menschen
//! verbindet: das Abonnement der Flow-Ereignisse, die Tasten, der Kasten auf
//! `stderr` und die drei Wege, die mehr als eine Taste brauchen (Regel,
//! Editor, Rumpf).
//!
//! # Zwei Ströme in einem Terminal
//!
//! Die Ausgabe des Agenten läuft auf `stdout`, der Kasten auf `stderr`. Das
//! ist die einzige Trennung, die ein Terminal von sich aus anbietet, und sie
//! reicht nicht: Beide landen auf demselben Schirm. Solange der Kasten steht,
//! hält [`crate::ask_terminal::Moderator`] die Ausgabe deshalb an, und was der
//! Kasten selbst zeichnet, räumt er vor dem nächsten Zeichnen wieder weg --
//! mit `ESC [ n A` und `ESC [ 0 J`, den beiden Folgen, die die Kommandozeile
//! in ihr **eigenes** Terminal schreibt. Bytes des Agenten nehmen diesen Weg
//! nie: Sie kommen gefiltert aus dem Daemon
//! ([`humanitl_core::TerminalFilter`]) und laufen hier nur noch durch
//! [`std::io::Write::write_all`].
//!
//! # Was hier nicht entschieden wird
//!
//! Nichts. Jede Taste wird zu einem `Decide`, einem `Rules(Add)` oder einer
//! Anfrage nach mehr Text; die Regel selbst wertet der Daemon aus (ADR-018).
//! Auch die Frist steht im Daemon: Der Countdown hier zählt die Sekunden bis
//! zu einem Zeitpunkt, den der Daemon geschickt hat, und wenn er abläuft,
//! entscheidet nicht die Kommandozeile, sondern die Warteschlange.

use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::v1;
use tokio::sync::mpsc;

use crate::ask_terminal::{
    FALLBACK_WIDTH, Held, MIN_BOX_WIDTH, Moderator, Step, Verdict, prompt_lines,
};
use crate::cmd::{Failure, status_diagnostic};
use crate::tty::{RawMode, window_size};

/// Wie lange der Tastenleser auf ein Byte wartet, bevor er nachsieht, ob
/// jemand pausiert hat.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Wie viele Tasten der Kanal puffert.
///
/// Tasten kommen einzeln und werden einzeln beantwortet; ein tiefer Puffer
/// sammelte nur, was ein Mensch in einer Schrecksekunde getippt hat, und
/// arbeitete es danach ab.
const KEY_BUFFER: usize = 8;

/// Wie viele gehaltene Flüsse eine Seite von `ListFlows` trägt.
const LIST_PAGE: u32 = 100;

/// Wie viele gehaltene Flüsse zusammen gelesen werden.
///
/// Eine Grenze und keine Schätzung: Die Warteschlange ist durch die Haltefrist
/// begrenzt, aber ein Agent, der in Schleife Anfragen stellt, kann sie lang
/// machen, und eine Schleife ohne Ende hielte den Befehl auf.
const LIST_CAP: usize = 1000;

/// Die höchste Menge Rumpf, die `v` zeigt.
const VIEW_BYTES: usize = 4096;

/// Wie viele Bytes eine Zeile des Hex-Blocks trägt.
const HEX_COLUMNS: usize = 16;

/// Die Moderation einer Sitzung im Terminal.
pub struct Moderation {
    /// Ein eigener Client: Der Ereignisstrom der Sandbox hält den anderen.
    client: Client,
    /// Der Zustand des Kastens.
    moderator: Moderator,
    /// Die Tasten dieses Terminals.
    keys: mpsc::Receiver<u8>,
    /// Der Rohmodus, solange die Moderation läuft.
    raw: Option<RawMode>,
    /// Wahr, solange der Tastenleser die Eingabe in Ruhe lassen soll.
    paused: Arc<AtomicBool>,
    /// Wie viele Zeilen zuletzt gezeichnet wurden.
    drawn: usize,
    /// Wahr, wenn der Schreibkopf in Spalte 0 steht.
    ///
    /// Der Agent hört selten am Zeilenende auf. Begänne der Kasten dort, wo
    /// seine letzte Zeile aufgehört hat, wäre die erste Kastenzeile zu lang,
    /// bräche um, und das Löschen ginge um eine Zeile zu wenig hinauf -- eine
    /// Zeile Rest je Sekunde.
    at_line_start: bool,
    /// Die Sekunden, die der Kasten als Rest anzeigt.
    left: u64,
}

impl Moderation {
    /// Beginnt die Moderation: Rohmodus, Tastenleser, sonst nichts.
    ///
    /// Der Ereignisstrom wird nicht hier geöffnet, sondern von `run`: Er
    /// gehört in dieselbe `select!`-Schleife wie die Ereignisse der Sandbox,
    /// und ein zweiter Ort, an dem er endet, wäre ein zweiter Ort, an dem er
    /// vergessen werden kann.
    pub fn new(client: Client) -> Self {
        let (tx, keys) = mpsc::channel(KEY_BUFFER);
        let paused = Arc::new(AtomicBool::new(false));
        spawn_keys(tx, Arc::clone(&paused));
        Self {
            client,
            moderator: Moderator::new(),
            keys,
            raw: RawMode::enter(),
            paused,
            drawn: 0,
            at_line_start: true,
            left: 0,
        }
    }

    /// Die nächste Taste, oder `None`, wenn die Eingabe endet.
    pub async fn key(&mut self) -> Option<u8> {
        self.keys.recv().await
    }

    /// Ausgabe des Agenten, die auf den Schirm will.
    ///
    /// Steht ein Kasten, wartet sie; läuft der Puffer über, geht sie hinaus und
    /// der Kasten wird darüber neu gezeichnet.
    pub fn output(&mut self, chunk: &[u8]) {
        let Some(bytes) = self.moderator.output(chunk) else {
            return;
        };
        if self.moderator.overflowed() {
            self.erase();
        }
        self.at_line_start = bytes.last().is_none_or(|byte| *byte == b'\n');
        write_out(&bytes);
        if self.moderator.overflowed() {
            self.draw();
        }
    }

    /// Ein Ereignis des Flow-Stroms.
    pub async fn flow_event(&mut self, event: &v1::FlowEvent) {
        let step = match event.event.as_ref() {
            Some(v1::flow_event::Event::Held(held)) => self.moderator.held(&held.flow_id),
            Some(v1::flow_event::Event::Decided(decided)) => {
                self.moderator.decided(&decided.flow_id)
            }
            // **Verworfene Ereignisse sind der einzige Weg, auf dem eine
            // gehaltene Anfrage hier verlorengeht.** Der Rundfunk des Daemons
            // hat einen Puffer, und dieser Befehl liest ihn nicht, solange er
            // auf einen Editor, einen Pager oder eine zweite Taste wartet.
            // Was dabei fällt, kann ein `Held` sein -- eine Frage, die
            // niemand je sähe, bis ihre Frist sie blockt. Also fragt der
            // Befehl die Warteschlange neu, statt der eigenen zu trauen.
            Some(v1::flow_event::Event::Lagged(lagged)) => {
                self.erase();
                self.tell(&format!(
                    "[humanitl] {} queue events were dropped; reading the queue again\r\n",
                    lagged.dropped
                ));
                self.resync().await
            }
            Some(v1::flow_event::Event::TimedOut(reference)) => {
                // Die Zeile der Spezifikation: Wer wartet, erfährt, dass die
                // Frist entschieden hat, statt den Kasten stumm verschwinden
                // zu sehen.
                self.erase();
                self.tell("[humanitl] timed out -> blocked\r\n");
                self.moderator.decided(&reference.flow_id)
            }
            _ => Step::Idle,
        };
        // Die Uhr gehört dem Fluss, der im Kasten steht. Ohne diese Frage
        // trüge sie die Frist des nächsten, der gehalten wird, und der Kasten
        // verspräche Minuten, die er nicht hat.
        if let Some(v1::flow_event::Event::Held(held)) = event.event.as_ref()
            && self
                .moderator
                .shown()
                .is_none_or(|shown| shown.flow_id == held.flow_id)
        {
            self.left = seconds_left(held.deadline.as_ref());
        }
        self.act(step).await;
    }

    /// Liest die gehaltenen Flüsse neu und nimmt sie als Warteschlange.
    ///
    /// Nach verworfenen Ereignissen ist die eigene Liste nicht mehr das, was
    /// der Daemon führt: Sie kann eine Anfrage vermissen und eine tragen, über
    /// die längst entschieden ist.
    async fn resync(&mut self) -> Step {
        let mut held: Vec<String> = Vec::new();
        let mut cursor = String::new();
        // **Bis zum Ende gelesen, nicht eine Seite weit.** Eine Seite als
        // ganze Wahrheit zu nehmen hieße, alles dahinter aus der eigenen
        // Warteschlange zu werfen -- und der Augenblick, in dem Ereignisse
        // fallen, ist genau der, in dem die Warteschlange am längsten ist.
        // Die Menge ist durch die Haltefrist begrenzt, die Schleife endet
        // also.
        loop {
            let answer = self
                .client
                .list_flows(v1::ListFlowsRequest {
                    filter: "state:held".to_owned(),
                    // Älteste zuerst, wie die eigene Warteschlange: Die
                    // Vorgabe des Dienstes ist `received_at desc`, und danach
                    // stünde die jüngste Anfrage im Kasten statt der, deren
                    // Frist zuerst abläuft.
                    order_by: "received_at asc".to_owned(),
                    cursor: cursor.clone(),
                    limit: LIST_PAGE,
                    ..v1::ListFlowsRequest::default()
                })
                .await;
            let page = match answer {
                Ok(answer) => answer.into_inner(),
                Err(status) => {
                    self.say(&status_diagnostic(&status, "ListFlows"));
                    return Step::Idle;
                }
            };
            held.extend(page.flows.iter().map(|flow| flow.flow_id.clone()));
            // Vier Enden statt einem: Ein Dienst, der eine leere Seite mit
            // einem Zeiger beantwortet oder denselben Zeiger noch einmal
            // schickt, hielte diesen Befehl sonst in einer Schleife aus
            // Aufrufen fest -- und der Mensch säße vor einem Kasten, der nicht
            // mehr wiederkommt.
            if page.flows.is_empty()
                || page.next_cursor.is_empty()
                || page.next_cursor == cursor
                || held.len() >= LIST_CAP
            {
                break;
            }
            cursor = page.next_cursor;
        }
        self.moderator.resync(&held)
    }

    /// Eine Sekunde ist vergangen.
    pub fn tick(&mut self) {
        if !self.moderator.is_open() {
            return;
        }
        self.left = self.left.saturating_sub(1);
        self.draw();
    }

    /// Eine Taste des Menschen; `true`, wenn die Sitzung enden soll.
    ///
    /// Das ist `Ctrl+C` ohne stehenden Kasten: Im Rohmodus kommt es als Byte
    /// und nicht als Signal, also muss dieser Weg es weitergeben.
    pub async fn pressed(&mut self, byte: u8) -> bool {
        let step = self.moderator.key(byte);
        let ends_the_session = step == Step::Stop;
        self.act(step).await;
        ends_the_session
    }

    /// Führt aus, was der Kasten verlangt.
    async fn act(&mut self, step: Step) {
        match step {
            // `Idle` fasst den Schirm nicht an: Über den Ereignisstrom kommt
            // jede Antwort und jedes Stück einer Antwort, und ein Kasten, der
            // dabei jedes Mal verschwände, wäre bei einem streamenden Agenten
            // öfter weg als da.
            Step::Idle => {}
            Step::Stop | Step::Close => self.erase(),
            Step::Draw => self.draw(),
            Step::Fetch(flow_id) => self.fetch(&flow_id).await,
            Step::Decide { flow_id, verdict } => self.decide(&flow_id, verdict).await,
            Step::ViewBody(flow_id) => self.view(&flow_id).await,
            Step::Edit(flow_id) => self.edit(&flow_id).await,
            Step::AskRule(flow_id) => self.ask_rule(&flow_id).await,
        }
    }

    /// Holt die Einzelheiten einer gehaltenen Anfrage und zeichnet sie.
    async fn fetch(&mut self, flow_id: &str) {
        let detail = match self
            .client
            .get_flow(v1::FlowRef {
                flow_id: flow_id.to_owned(),
            })
            .await
        {
            Ok(detail) => detail.into_inner(),
            Err(status) => {
                self.say(&status_diagnostic(&status, "GetFlow"));
                return;
            }
        };
        if let Some(summary) = detail.summary.as_ref() {
            self.left = seconds_left(summary.deadline.as_ref());
        }
        self.moderator
            .show(crate::ask_terminal::from_detail(&detail));
        self.draw();
    }

    /// Schickt die Entscheidung, mit oder ohne Regel.
    async fn decide(&mut self, flow_id: &str, verdict: Verdict) {
        let remember = match verdict {
            Verdict::AllowForSession => self.session_rule(flow_id),
            _ => None,
        };
        let decision = match verdict {
            Verdict::Block => Some(v1::decide_request::Decision::Block(
                v1::decide_request::Block::default(),
            )),
            _ => Some(v1::decide_request::Decision::Allow(())),
        };
        let request = v1::DecideRequest {
            flow_ids: vec![flow_id.to_owned()],
            decision,
            remember,
            acknowledge_findings: false,
        };
        if let Err(status) = self.client.decide(request).await {
            self.say(&status_diagnostic(&status, "Decide"));
        }
        // Gezeichnet wird nicht hier: Der Daemon meldet die Entscheidung als
        // `Decided`, und erst dann ist sie wahr. Der Kasten verschwindet also
        // an derselben Stelle, an der er verschwindet, wenn jemand anders im
        // Fenster entschieden hat.
    }

    /// Die Regel, die `s` anlegt: dieser Host, diese Sitzung.
    fn session_rule(&self, _flow_id: &str) -> Option<v1::Rule> {
        let held = self.moderator.shown()?;
        if held.host.is_empty() {
            return None;
        }
        let host = held.host.clone();
        Some(v1::Rule {
            action: v1::RuleAction::Allow as i32,
            matcher: Some(v1::RuleMatcher {
                host,
                ..v1::RuleMatcher::default()
            }),
            expires: Some(v1::RuleExpiry {
                expiry: Some(v1::rule_expiry::Expiry::Session(())),
            }),
            ..v1::Rule::default()
        })
    }

    /// `r`: Ziel und Dauer, jede in einem Tastendruck.
    async fn ask_rule(&mut self, flow_id: &str) {
        let Some(held) = self.moderator.shown().cloned() else {
            return;
        };
        if held.host.is_empty() {
            self.say(&rule_without_host());
            return;
        }
        let host = held.host.clone();
        self.erase();
        let Some(target) = self
            .choose(&[
                "[1] this exact URL",
                "[2] this host",
                "[3] this apex and every host under it",
                "[4] this host and this method",
            ])
            .await
        else {
            self.draw();
            return;
        };
        let Some(duration) = self
            .choose(&["[1] once", "[2] this session", "[3] forever"])
            .await
        else {
            self.draw();
            return;
        };
        // `[1] once` ist keine Regel, sondern ihre Abwesenheit: Die Anfrage
        // wird erlaubt, und die nächste an denselben Host wird wieder gefragt.
        let rule = if duration == 1 {
            None
        } else {
            match rule_for(&held, &host, target, duration) {
                Ok(rule) => Some(rule),
                Err(diagnostic) => {
                    self.say(&diagnostic);
                    return;
                }
            }
        };
        match rule.as_ref() {
            Some(rule) => self.tell(&format!(
                "[humanitl] rule: {} {} - {}\r\n",
                action_word(v1::RuleAction::Allow),
                describe_matcher(rule.matcher.as_ref()),
                describe_expiry(rule.expires.as_ref()),
            )),
            None => self.tell("[humanitl] allow once, without a rule\r\n"),
        }
        self.tell("[humanitl] enter to confirm, any other key to drop it\r\n");
        if !matches!(self.keys.recv().await, Some(b'\r' | b'\n')) {
            self.draw();
            return;
        }
        let request = v1::DecideRequest {
            flow_ids: vec![flow_id.to_owned()],
            decision: Some(v1::decide_request::Decision::Allow(())),
            remember: rule,
            acknowledge_findings: false,
        };
        if let Err(status) = self.client.decide(request).await {
            self.say(&status_diagnostic(&status, "Decide"));
        }
    }

    /// Liest eine Ziffer aus der angebotenen Liste.
    async fn choose(&mut self, options: &[&str]) -> Option<usize> {
        for option in options {
            self.tell(&format!("[humanitl] {option}\r\n"));
        }
        loop {
            let byte = self.keys.recv().await?;
            if byte == 0x1b || byte == 0x03 {
                return None;
            }
            if byte.is_ascii_digit() {
                let index = usize::from(byte - b'0');
                if index >= 1 && index <= options.len() {
                    return Some(index);
                }
            }
        }
    }

    /// `v`: die ersten [`VIEW_BYTES`] des Rumpfs, nicht druckbares als Hex.
    async fn view(&mut self, flow_id: &str) {
        let detail = match self
            .client
            .get_flow(v1::FlowRef {
                flow_id: flow_id.to_owned(),
            })
            .await
        {
            Ok(detail) => detail.into_inner(),
            Err(status) => {
                self.say(&status_diagnostic(&status, "GetFlow"));
                return;
            }
        };
        let Some(reference) = detail
            .request
            .as_ref()
            .and_then(|request| request.body.clone())
        else {
            self.erase();
            self.tell("[humanitl] this request has no body\r\n");
            self.draw();
            return;
        };
        let mut stream = match self.client.get_body(reference).await {
            Ok(stream) => stream.into_inner(),
            Err(status) => {
                self.say(&status_diagnostic(&status, "GetBody"));
                return;
            }
        };
        let mut bytes: Vec<u8> = Vec::new();
        while bytes.len() < VIEW_BYTES {
            match stream.message().await {
                Ok(Some(chunk)) => bytes.extend_from_slice(&chunk.data),
                Ok(None) => break,
                Err(status) => {
                    self.say(&status_diagnostic(&status, "GetBody"));
                    return;
                }
            }
        }
        bytes.truncate(VIEW_BYTES);
        self.erase();
        for line in body_lines(&bytes) {
            self.tell(&format!("{line}\r\n"));
        }
        self.tell("[humanitl] any key to go back\r\n");
        let _ = self.keys.recv().await;
        self.draw();
    }

    /// `e`: die Anfrage im `$EDITOR`, und zurück als `AllowEdited`.
    ///
    /// Die Datei liegt im Laufzeitverzeichnis mit `0600` und wird auf jedem
    /// Ausgang gelöscht -- nie unter `/work` und nie im Arbeitsverzeichnis:
    /// Was der Agent geschrieben hat, gehört nicht in das Projekt, und ein
    /// Editor, der eine Modeline liest, führt fremden Text aus.
    ///
    /// Der Rohmodus geht für die Dauer des Editors zurück: `vim` in einem
    /// Terminal, das schon roh ist, sieht keine Zeilenenden mehr.
    async fn edit(&mut self, flow_id: &str) {
        self.erase();
        let detail = match self
            .client
            .get_flow(v1::FlowRef {
                flow_id: flow_id.to_owned(),
            })
            .await
        {
            Ok(detail) => detail.into_inner(),
            Err(status) => {
                self.say(&status_diagnostic(&status, "GetFlow"));
                return;
            }
        };
        let Some(request) = detail.request.as_ref() else {
            self.tell("[humanitl] this request cannot be edited: the daemon sent no request\r\n");
            self.draw();
            return;
        };
        // **Der ganze Rumpf, nicht die Vorschau.** `body_preview` ist auf 4096
        // Skalare gedeckelt und ersetzt ungültige Bytes durch U+FFFD
        // (`humanitl.proto`); `EditedRequest.body` dagegen ist das, was
        // wirklich hinausgeht. Wer die Vorschau schickte, kürzte die Anfrage
        // still und machte aus einem gzip-Rumpf eine Reihe Fragezeichen.
        let body = match self.request_body(request).await {
            Ok(body) => body,
            Err(diagnostic) => {
                self.say(&diagnostic);
                return;
            }
        };
        let text = request_text(request, &body);
        let path = match edit_path(flow_id) {
            Ok(path) => path,
            Err(diagnostic) => {
                self.say(&diagnostic);
                return;
            }
        };
        if let Err(diagnostic) = write_private(&path, text.as_bytes()) {
            let _ = std::fs::remove_file(&path);
            self.say(&diagnostic);
            return;
        }
        // Der Tastenleser lässt die Eingabe los, solange der Editor sie
        // braucht; ohne das nähme er ihm jede zweite Taste weg.
        self.paused.store(true, Ordering::SeqCst);
        std::thread::sleep(POLL_INTERVAL * 3);
        if let Some(raw) = self.raw.as_mut() {
            raw.leave();
        }
        let status = open_editor(&path);
        // Zurück in den Rohmodus. Der alte Wächter fällt dabei, ohne etwas zu
        // tun: `leave()` hat ihn entwaffnet. Bleibt der neue aus -- kein
        // Terminal mehr, oder kein Deskriptor frei --, sagt der Kasten das,
        // statt still mit Echo weiterzuzeichnen.
        self.raw = RawMode::enter();
        if self.raw.is_none() {
            self.say(
                &Diagnostic::builder(codes::CLI_001, Severity::Warning)
                    .why(
                        "this terminal did not go back into raw mode after the editor; every key \
                         is echoed twice from here on, and the box may look doubled"
                            .to_owned(),
                    )
                    .build(),
            );
        }
        // Erst die Warteschlange des Kernels leeren, dann wieder lesen: Was
        // ein Mensch tippte, während der Editor sich beendete, liegt dort und
        // nicht im Kanal -- der Leser holte es sonst gleich danach und machte
        // eine Entscheidung daraus.
        let _ = rustix::termios::tcflush(
            rustix::stdio::stdin(),
            rustix::termios::QueueSelector::IFlush,
        );
        // Dann den Kanal, und **erst danach** wieder lesen lassen: Was in dem
        // winzigen Fenster zwischen `poll` und `read` doch noch gelesen wurde,
        // gehört dem Editor und nicht dieser Warteschlange -- ein `b` aus
        // `vim` darf keinen Fluss blocken. In der umgekehrten Reihenfolge
        // schluckte das Leeren stattdessen die erste Taste, die ein Mensch
        // nach dem Editor drückt.
        while self.keys.try_recv().is_ok() {}
        self.paused.store(false, Ordering::SeqCst);
        let edited = std::fs::read_to_string(&path);
        let _ = std::fs::remove_file(&path);
        if let Err(diagnostic) = status {
            self.say(&diagnostic);
            return;
        }
        let Ok(edited) = edited else {
            self.say(&edit_failed("the edited file could not be read back"));
            return;
        };
        let edited = match parse_request(&edited) {
            Ok(edited) => edited,
            Err(diagnostic) => {
                self.say(&diagnostic);
                return;
            }
        };
        let request = v1::DecideRequest {
            flow_ids: vec![flow_id.to_owned()],
            decision: Some(v1::decide_request::Decision::AllowEdited(edited)),
            remember: None,
            acknowledge_findings: false,
        };
        if let Err(status) = self.client.decide(request).await {
            self.say(&status_diagnostic(&status, "Decide"));
        }
    }

    /// Der vollständige Rumpf einer Anfrage, als Text.
    ///
    /// Ein Rumpf, der kein UTF-8 ist, wird nicht bearbeitet: Was ein Editor
    /// daraus machte, ginge als etwas anderes hinaus, als der Agent geschickt
    /// hat. Wer ihn sehen will, drückt `v`.
    async fn request_body(&mut self, request: &v1::HttpRequest) -> Result<String, Diagnostic> {
        let Some(reference) = request.body.clone() else {
            return Ok(String::new());
        };
        if reference.size == 0 {
            return Ok(String::new());
        }
        let mut stream = self
            .client
            .get_body(reference)
            .await
            .map_err(|status| status_diagnostic(&status, "GetBody"))?
            .into_inner();
        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = stream
            .message()
            .await
            .map_err(|status| status_diagnostic(&status, "GetBody"))?
        {
            bytes.extend_from_slice(&chunk.data);
        }
        String::from_utf8(bytes).map_err(|error| {
            edit_failed(&format!(
                "the body of this request is not text ({} bytes, invalid from byte {}); \
                 press v to look at it instead",
                error.as_bytes().len(),
                error.utf8_error().valid_up_to()
            ))
        })
    }

    /// Zeichnet den Kasten neu.
    fn draw(&mut self) {
        let Some(held) = self.moderator.shown().cloned() else {
            self.erase();
            return;
        };
        self.erase();
        let width = usize::try_from(window_size().0)
            .unwrap_or(FALLBACK_WIDTH)
            .max(MIN_BOX_WIDTH);
        let lines = prompt_lines(&held, self.moderator.position(), self.left, width);
        let mut out = String::with_capacity(lines.len() * (width + 2));
        if !self.at_line_start {
            out.push_str("\r\n");
            self.at_line_start = true;
        }
        for line in &lines {
            out.push_str(line);
            out.push_str("\r\n");
        }
        write_err(&out);
        self.drawn = lines.len();
    }

    /// Nimmt den Kasten weg, wenn einer steht.
    fn erase(&mut self) {
        if self.drawn == 0 {
            return;
        }
        // Der Cursor steht unter dem Kasten: so viele Zeilen hinauf, dann
        // alles darunter löschen. Beide Folgen schreibt die Kommandozeile in
        // ihr eigenes Terminal, nie ein Byte des Agenten.
        write_err(&format!("\u{1b}[{}A\u{1b}[0J", self.drawn));
        self.drawn = 0;
    }

    /// Ein Befund, ohne den Kasten zu verlieren.
    fn say(&mut self, diagnostic: &Diagnostic) {
        self.erase();
        self.tell(&format!(
            "{}\r\n",
            crate::render::diagnostic_block(diagnostic).replace('\n', "\r\n")
        ));
        self.draw();
    }

    /// Eine Zeile dieses Programms auf `stderr`.
    ///
    /// **Jede Zeile, die dieser Typ schreibt, nimmt diesen Weg**, und nicht
    /// [`write_err`] daneben: Der Schreibkopf soll wissen, ob er in Spalte 0
    /// steht, und die nächste Zeile soll nicht an die Ausgabe des Agenten
    /// geklebt werden. Wer eine Zeile hinzufügt und `write_err` nähme, erbte
    /// den Fehler, gegen den dieser Weg gebaut ist.
    fn tell(&mut self, text: &str) {
        // Steht der Schreibkopf mitten in einer Zeile des Agenten, beginnt
        // diese hier auf einer eigenen: Sonst liest sich das als
        // `agent output so f[humanitl] stopping the session`.
        if !self.at_line_start {
            write_err("\r\n");
        }
        write_err(text);
        self.at_line_start = text.ends_with('\n');
    }

    /// Sagt, dass die Sitzung endet.
    pub fn say_stopping(&mut self) {
        self.erase();
        self.tell("[humanitl] stopping the session\r\n");
    }

    /// Gibt das Terminal zurück und räumt den Kasten weg.
    pub fn finish(&mut self) {
        self.erase();
        let rest = self.moderator.drain();
        if !rest.is_empty() {
            write_out(&rest);
        }
        if let Some(raw) = self.raw.as_mut() {
            raw.leave();
        }
    }
}

/// Liest Tasten, solange die Eingabe offen ist und niemand pausiert hat.
///
/// **Warum das Pausieren nötig ist.** Ein Leser, der dauerhaft in `read(0)`
/// steht, nimmt dem `$EDITOR` seine Eingabe weg: Beide lesen denselben
/// Deskriptor, und der Kernel gibt jedes Byte genau einem von beiden. Ohne
/// Pause verlöre der Editor Tasten -- und schlimmer: Die gestohlenen Bytes
/// lägen danach im Kanal und würden als Entscheidungen gelesen. Ein `b`, das
/// jemand in `vim` tippt, blockte einen Fluss.
///
/// Deshalb wartet dieser Leser mit `poll` statt in einem blockierenden `read`
/// und prüft vor jedem Lesen, ob pausiert ist. Bleibt ein Rest: Zwischen
/// „`poll` sagt lesbar" und dem `read` liegt ein Fenster von Mikrosekunden.
/// [`Moderation::edit`] leert den Kanal deshalb nach dem Editor, bevor wieder
/// eine Taste zu einer Entscheidung wird.
fn spawn_keys(tx: mpsc::Sender<u8>, paused: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        use std::io::Read as _;

        let mut stdin = std::io::stdin();
        let mut buffer = [0u8; 16];
        loop {
            if paused.load(Ordering::SeqCst) {
                std::thread::sleep(POLL_INTERVAL);
                continue;
            }
            if !readable(POLL_INTERVAL) {
                continue;
            }
            if paused.load(Ordering::SeqCst) {
                continue;
            }
            let Ok(read) = stdin.read(&mut buffer) else {
                return;
            };
            if read == 0 {
                return;
            }
            for byte in &buffer[..read] {
                // `try_send` und nicht `blocking_send`: Ein voller Kanal
                // hielte den Leser sonst mitten in einem Stück fest, und das
                // festgehaltene Byte käme nach dem Editor als Entscheidung
                // heraus. Und was nach einer Pause gelesen wurde, gehört dem,
                // der pausiert hat.
                if paused.load(Ordering::SeqCst) {
                    break;
                }
                match tx.try_send(*byte) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(_)) => break,
                    Err(mpsc::error::TrySendError::Closed(_)) => return,
                }
            }
        }
    });
}

/// Wartet höchstens `timeout` darauf, dass die eigene Eingabe etwas hat.
///
/// `rustix::event::poll` statt `libc::poll`: Die Kommandozeile verbietet
/// `unsafe` (`#![forbid(unsafe_code)]`), und rustix ist der Weg, den dieses
/// Programm auch für `termios` nimmt.
fn readable(timeout: Duration) -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};

    let stdin = rustix::stdio::stdin();
    let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
    let deadline = Timespec {
        tv_sec: i64::try_from(timeout.as_secs()).unwrap_or(0),
        tv_nsec: i64::from(timeout.subsec_nanos()),
    };
    poll(&mut fds, Some(&deadline)).is_ok_and(|ready| ready > 0)
}

/// Schreibt auf `stdout`, was der Agent geschrieben hat.
fn write_out(bytes: &[u8]) {
    let mut out = std::io::stdout();
    let _ = out.write_all(bytes);
    let _ = out.flush();
}

/// Schreibt auf `stderr`, was Humanitl sagt.
fn write_err(text: &str) {
    let mut err = std::io::stderr();
    let _ = err.write_all(text.as_bytes());
    let _ = err.flush();
}

/// Die Sekunden bis zur Frist, oder 0.
fn seconds_left(deadline: Option<&prost_types::Timestamp>) -> u64 {
    let Some(deadline) = deadline else {
        return 0;
    };
    let now = chrono::Utc::now().timestamp();
    u64::try_from(deadline.seconds.saturating_sub(now)).unwrap_or(0)
}

/// Die Regel aus Ziel und Dauer, oder der Grund, warum es sie nicht gibt.
///
/// Zwei Ziele können scheitern, und beide scheitern laut statt still: Ein
/// Apex, den der Dienst nicht kennt (eine Adresse, ein unbekanntes Suffix),
/// und eine Methode, die der Vertrag nicht benennt. Eine Regel, die dann
/// **breiter** wäre als das, was auf dem Schirm stand -- Host statt Host und
/// Methode --, ist genau die Lüge, die `backlog/CONVENTIONS.md` 4.13
/// ausschließt.
///
/// # Errors
///
/// [`codes::CLI_004`] mit dem Grund.
fn rule_for(
    held: &Held,
    host: &str,
    target: usize,
    duration: usize,
) -> Result<v1::Rule, Diagnostic> {
    let mut matcher = v1::RuleMatcher {
        host: host.to_owned(),
        ..v1::RuleMatcher::default()
    };
    match target {
        1 => {
            // Alle Zeichen, die `globset` als Muster liest: Stern,
            // Fragezeichen, die Klammern einer Klasse, die Klammern einer
            // Gruppe und der Backslash, der unter Unix als Escape gilt.
            // `/repos/{owner}/x` wäre sonst eine Gruppe mit einem Zweig und
            // träfe `/repos/owner/x` -- eine Regel, die auf der Adresse, die
            // der Mensch gesehen hat, nie feuert.
            if held.path.contains(['*', '?', '[', ']', '{', '}', '\\']) {
                return Err(rule_refused(&format!(
                    "the path {} carries a character that the rule engine reads as a pattern; \
                     take the host instead",
                    held.path
                )));
            }
            matcher.path.clone_from(&held.path);
        }
        3 => {
            if held.apex.is_empty() {
                return Err(rule_refused(
                    "the daemon does not name an apex for this host (an address, or a suffix the \
                     list does not know); take the host instead",
                ));
            }
            matcher.host = format!("**.{}", held.apex);
        }
        4 => {
            let method = v1::Method::try_from(held.method).unwrap_or(v1::Method::Other);
            if method == v1::Method::Other || method == v1::Method::Unspecified {
                return Err(rule_refused(
                    "a rule cannot name this method; an empty list of methods would match every \
                     one of them, which is wider than what the box offered",
                ));
            }
            matcher.methods = vec![held.method];
        }
        _ => {}
    }
    // Der Vertrag kennt drei Gültigkeiten: `never` (also dauerhaft),
    // `session` und `at`. Ein „once" ist keine Regel, sondern ihre Abwesenheit
    // -- die Entscheidung gilt für diese eine Anfrage --, und deshalb steht
    // hier `Never` für „forever" und `Session` für alles andere. Die Auswahl
    // `[1] once` legt gar keine Regel an; das entscheidet `ask_rule`.
    let expiry = if duration == 3 {
        v1::rule_expiry::Expiry::Never(())
    } else {
        v1::rule_expiry::Expiry::Session(())
    };
    Ok(v1::Rule {
        action: v1::RuleAction::Allow as i32,
        matcher: Some(matcher),
        expires: Some(v1::RuleExpiry {
            expiry: Some(expiry),
        }),
        ..v1::Rule::default()
    })
}

/// Der Befund, wenn das gewählte Ziel keine ehrliche Regel ergibt.
fn rule_refused(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why(format!("no rule was made: {why}"))
        .build()
}

/// `allow` oder `block`, für die Zeile vor der Bestätigung.
fn action_word(action: v1::RuleAction) -> &'static str {
    match action {
        v1::RuleAction::Block => "block",
        v1::RuleAction::Redact => "redact",
        _ => "allow",
    }
}

/// Der Matcher in einer Zeile.
fn describe_matcher(matcher: Option<&v1::RuleMatcher>) -> String {
    let Some(matcher) = matcher else {
        return "host: ?".to_owned();
    };
    // Gesäubert, obwohl der Daemon Host und Pfad normalisiert: Diese Zeile
    // geht in ein Terminal im Rohmodus, und eine Zusicherung, die woanders
    // gilt, ist keine Sperre hier.
    let mut out = format!("host: {}", crate::render::plain(&matcher.host));
    if !matcher.path.is_empty() {
        let _ = write!(out, " path: {}", crate::render::plain(&matcher.path));
    }
    if !matcher.methods.is_empty() {
        out.push_str(" (this method)");
    }
    out
}

/// Die Dauer in einem Wort.
fn describe_expiry(expiry: Option<&v1::RuleExpiry>) -> &'static str {
    match expiry.and_then(|expiry| expiry.expiry.as_ref()) {
        None | Some(v1::rule_expiry::Expiry::Never(())) => "forever",
        Some(v1::rule_expiry::Expiry::At(_)) => "until a time",
        Some(v1::rule_expiry::Expiry::Session(())) => "this session",
    }
}

/// Die Anfrage als Text, wie ein Mensch sie bearbeitet.
///
/// Das Format ist das einer HTTP-Anfrage und keine Erfindung: Zeile eins
/// Methode und URL, dann die Kopfzeilen, dann eine Leerzeile, dann der Rumpf.
/// Wer es kennt, muss nichts lernen; wer es nicht kennt, sieht es an der
/// Datei.
#[must_use]
pub fn request_text(request: &v1::HttpRequest, body_preview: &str) -> String {
    let method = if request.method_raw.is_empty() {
        method_word(request.method)
    } else {
        request.method_raw.clone()
    };
    let scheme = if request.scheme == i32::from(v1::Scheme::Http) {
        "http"
    } else {
        "https"
    };
    let authority = request
        .authority
        .as_ref()
        .map_or_else(String::new, |authority| {
            if authority.port == 0 {
                authority.host.clone()
            } else {
                format!("{}:{}", authority.host, authority.port)
            }
        });
    let mut out = format!(
        "{method} {scheme}://{authority}{}\n",
        request.path_and_query
    );
    for header in &request.headers {
        // Der Wert reist als Bytes; was keine gültige UTF-8-Folge ist, wird
        // sichtbar ersetzt statt weggelassen -- eine Kopfzeile, die im Editor
        // fehlt, käme als gelöschte zurück.
        let _ = writeln!(
            out,
            "{}: {}",
            header.name,
            String::from_utf8_lossy(&header.value)
        );
    }
    out.push('\n');
    out.push_str(body_preview);
    out
}

/// Der Name einer Methode.
fn method_word(method: i32) -> String {
    match v1::Method::try_from(method) {
        Ok(v1::Method::Post) => "POST",
        Ok(v1::Method::Put) => "PUT",
        Ok(v1::Method::Patch) => "PATCH",
        Ok(v1::Method::Delete) => "DELETE",
        Ok(v1::Method::Head) => "HEAD",
        Ok(v1::Method::Options) => "OPTIONS",
        Ok(v1::Method::Connect) => "CONNECT",
        Ok(v1::Method::Trace) => "TRACE",
        // `GET` steht auch für den unbekannten Fall: Eine Methode, die diese
        // Fassung nicht kennt, kommt im Text als Wort zurück, und `method_raw`
        // gewinnt dort über die Zahl.
        _ => "GET",
    }
    .to_owned()
}

/// Die bearbeitete Anfrage aus dem Text, oder der Grund, warum nicht.
///
/// Streng, und mit Absicht: Was hier durchkommt, geht als Anfrage hinaus.
/// Eine Zeile ohne Doppelpunkt im Kopfteil, eine URL ohne Schema oder eine
/// Methode, die keine ist, wird abgelehnt statt geraten.
///
/// # Errors
///
/// [`codes::CLI_004`] mit dem Satz, welche Zeile nicht zu lesen war. Ein
/// nackter Text wäre hier falsch: Jeder Fehlerpfad dieses Programms trägt
/// einen Code und einen Grund (`AGENTS.md`, `CONVENTIONS.md` 4.6).
pub fn parse_request(text: &str) -> Result<v1::EditedRequest, Diagnostic> {
    // **Zeile für Zeile geteilt, nicht über Offsets gezählt.** Ein Editor
    // schreibt vielleicht `\r\n`, und dann ist der Umbruch zwei Zeichen lang;
    // eine Rechnung mit `+ 1` verschöbe den Rumpf um ein Zeichen je Kopfzeile
    // und schnitte ihn im schlimmsten Fall mitten in ein Zeichen.
    let (first, mut rest) = match text.split_once('\n') {
        Some((first, rest)) => (first.strip_suffix('\r').unwrap_or(first), rest),
        None => (text.strip_suffix('\r').unwrap_or(text), ""),
    };
    if first.trim().is_empty() {
        return Err(edit_failed("the file is empty"));
    }
    let mut words = first.split_whitespace();
    let method = words
        .next()
        .ok_or_else(|| edit_failed("the first line has no method"))?;
    let url = words
        .next()
        .ok_or_else(|| edit_failed("the first line has no URL"))?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(edit_failed(&format!(
            "{url} is not an absolute http or https URL"
        )));
    }
    if url.contains('#') {
        return Err(edit_failed("a URL with a fragment does not go out"));
    }
    let mut headers = Vec::new();
    // Der Rumpf ist alles hinter der ersten leeren Zeile, Zeichen für Zeichen:
    // Über `lines()` wieder zusammengesetzt verlöre er seinen letzten Umbruch,
    // und jedes `\r\n` würde `\n` -- ein `multipart/form-data` trägt `\r\n`
    // um jede Grenze, und die Anfrage ginge mit kaputten Grenzen hinaus.
    let mut body = "";
    let mut in_body = false;
    while let Some((line, after)) = rest.split_once('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            body = after;
            in_body = true;
            break;
        }
        rest = after;
        headers.push(header_of(line)?);
    }
    // Eine letzte Kopfzeile ohne abschließenden Umbruch findet `split_once`
    // nicht mehr. Ohne diesen Schritt fiele sie stillschweigend weg -- und
    // eine Kopfzeile, die verschwindet, ist genau das, wogegen der ganze Weg
    // gebaut ist. Ein Editor ohne letzten Umbruch ist der Normalfall (VS Code
    // ohne `files.insertFinalNewline`, `vim` mit `nofixendofline`).
    if !in_body && !rest.is_empty() {
        headers.push(header_of(rest.strip_suffix('\r').unwrap_or(rest))?);
    }
    Ok(v1::EditedRequest {
        method: method_number(method),
        method_raw: method.to_owned(),
        url: url.to_owned(),
        headers,
        body: body.as_bytes().to_vec(),
    })
}

/// Eine Kopfzeile aus ihrer Zeile.
///
/// # Errors
///
/// [`codes::CLI_004`], wenn die Zeile keinen Doppelpunkt trägt.
fn header_of(line: &str) -> Result<v1::Header, Diagnostic> {
    let (name, value) = line
        .split_once(':')
        .ok_or_else(|| edit_failed(&format!("the header line {line:?} has no colon")))?;
    Ok(v1::Header {
        name: name.trim().to_owned(),
        value: value.trim().as_bytes().to_vec(),
    })
}

/// Die Nummer einer Methode, `METHOD_OTHER` für alles Unbekannte.
fn method_number(word: &str) -> i32 {
    let method = match word {
        "GET" => v1::Method::Get,
        "POST" => v1::Method::Post,
        "PUT" => v1::Method::Put,
        "PATCH" => v1::Method::Patch,
        "DELETE" => v1::Method::Delete,
        "HEAD" => v1::Method::Head,
        "OPTIONS" => v1::Method::Options,
        "CONNECT" => v1::Method::Connect,
        "TRACE" => v1::Method::Trace,
        _ => v1::Method::Other,
    };
    method as i32
}

/// Der Pfad der Datei, die der Editor öffnet.
///
/// Nur unter `$XDG_RUNTIME_DIR`, und ohne diese Variable gar nicht. Der
/// nächstliegende Ausweg wäre `/tmp`, und genau der ist keiner: Dort schreibt
/// jeder, ein anderer Nutzer legte `/tmp/humanitl` vorher an und bekäme die
/// Anfrage samt `Authorization`-Kopfzeile zu lesen -- und bestimmte, was als
/// bearbeitete Anfrage zurückkommt.
///
/// # Errors
///
/// [`codes::CLI_004`], wenn `$XDG_RUNTIME_DIR` fehlt.
fn edit_path(flow_id: &str) -> Result<std::path::PathBuf, Diagnostic> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            edit_failed(
                "XDG_RUNTIME_DIR is not set, and a request with its headers does not belong in a \
                 directory that everybody can write",
            )
        })?
        .join("humanitl");
    let safe: String = flow_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    Ok(dir.join(format!("edit-{safe}.http")))
}

/// Schreibt die Datei mit `0600`, neu und ohne einem Symlink zu folgen.
///
/// `create_new` statt `create`: Eine Datei, die schon da ist, gehört
/// jemandem, der sie vor uns angelegt hat, und `mode(0o600)` gilt nur beim
/// Anlegen. `O_NOFOLLOW` schlägt denselben Weg über einen Symlink zu.
///
/// # Errors
///
/// [`codes::CLI_004`] mit dem Pfad und dem Grund.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), Diagnostic> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| edit_failed(&format!("{}: {error}", dir.display())))?;
        // Auch das Verzeichnis gehört nur diesem Nutzer; `create_dir_all` legt
        // es mit der Maske des Prozesses an, und die ist oft 0755.
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    // Ein Rest aus einem abgebrochenen Lauf gehört uns und darf weg; einer,
    // der jemand anderem gehört, lässt sich nicht entfernen, und dann
    // scheitert das Anlegen gleich darauf.
    let _ = std::fs::remove_file(path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(path)
        .map_err(|error| edit_failed(&format!("{}: {error}", path.display())))?;
    file.write_all(bytes)
        .map_err(|error| edit_failed(&format!("{}: {error}", path.display())))
}

/// Startet `$EDITOR` und wartet auf sein Ende.
///
/// Ohne `$EDITOR` und ohne `$VISUAL` gilt `vi`: Es liegt auf jedem System,
/// auf dem dieses Werkzeug läuft, und ein Editor, den es nicht gibt, wäre ein
/// Weg, der ins Leere führt.
///
/// # Errors
///
/// [`codes::CLI_004`], wenn der Editor nicht startet oder mit einem Code
/// ungleich null endet.
fn open_editor(path: &std::path::Path) -> Result<(), Diagnostic> {
    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .unwrap_or_else(|| std::ffi::OsString::from("vi"));
    write_err("[humanitl] waiting for the editor to close\r\n");
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|error| edit_failed(&format!("{}: {error}", editor.to_string_lossy())))?;
    if status.success() {
        Ok(())
    } else {
        Err(edit_failed(&format!(
            "{} ended with {}",
            editor.to_string_lossy(),
            status.code().unwrap_or(-1)
        )))
    }
}

/// Der Befund, wenn der Weg über den Editor nicht zu Ende geht.
fn edit_failed(reason: &str) -> Diagnostic {
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why(format!("the edited request did not go out: {reason}"))
        .build()
}

/// Der Befund, wenn aus der Zeile kein Host zu lesen ist.
fn rule_without_host() -> Diagnostic {
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why("this request has no host to build a rule from".to_owned())
        .build()
}

/// Der Rumpf, druckbar oder als Hex.
///
/// Eine Zeile ist entweder Text oder ein Hex-Block; gemischt wird nicht.
/// Rohbytes aus `GetBody` laufen durch keinen Filter des Daemons -- der sitzt
/// am Terminal-Strom und nicht hier --, also entscheidet dieser Renderer
/// selbst, und er lässt nur Druckbares und Zeilenumbrüche durch.
#[must_use]
pub fn body_lines(bytes: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut text = String::new();
    let mut raw: Vec<u8> = Vec::new();

    let flush_raw = |raw: &mut Vec<u8>, lines: &mut Vec<String>| {
        for chunk in raw.chunks(HEX_COLUMNS) {
            let hex: Vec<String> = chunk.iter().map(|byte| format!("{byte:02x}")).collect();
            lines.push(format!("  {}", hex.join(" ")));
        }
        raw.clear();
    };

    for byte in bytes {
        match byte {
            b'\n' => {
                flush_raw(&mut raw, &mut lines);
                lines.push(std::mem::take(&mut text));
            }
            b'\r' => {}
            0x20..=0x7e | 0x09 => {
                flush_raw(&mut raw, &mut lines);
                text.push(char::from(*byte));
            }
            other => {
                if !text.is_empty() {
                    lines.push(std::mem::take(&mut text));
                }
                raw.push(*other);
            }
        }
    }
    flush_raw(&mut raw, &mut lines);
    if !text.is_empty() {
        lines.push(text);
    }
    lines
}

/// Der Befund, wenn `Subscribe` nicht zu öffnen ist.
pub fn subscribe_failed(status: &tonic::Status) -> Failure {
    Failure::new(status_diagnostic(status, "Subscribe"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Eine gehaltene Anfrage, wie der Dienst sie schickt.
    fn held() -> Held {
        Held {
            flow_id: "f1".to_owned(),
            request: "POST https://api.github.com/graphql".to_owned(),
            origin: String::new(),
            size: String::new(),
            findings: String::new(),
            catalog: String::new(),
            host: "api.github.com".to_owned(),
            path: "/graphql".to_owned(),
            apex: "github.com".to_owned(),
            method: v1::Method::Post as i32,
        }
    }

    /// Jedes Ziel baut den Matcher, den seine Zeile verspricht.
    #[test]
    fn every_target_builds_what_its_line_promises() {
        let held = held();
        let exact = rule_for(&held, "api.github.com", 1, 2).expect("a path without a pattern");
        assert_eq!(
            exact.matcher.as_ref().map(|m| m.path.as_str()),
            Some("/graphql")
        );
        let host = rule_for(&held, "api.github.com", 2, 2).expect("the host itself");
        assert_eq!(
            host.matcher.as_ref().map(|m| m.host.as_str()),
            Some("api.github.com")
        );
        assert!(host.matcher.as_ref().is_some_and(|m| m.path.is_empty()));
        let apex = rule_for(&held, "api.github.com", 3, 3).expect("the apex of the daemon");
        assert_eq!(
            apex.matcher.as_ref().map(|m| m.host.as_str()),
            Some("**.github.com")
        );
        assert_eq!(describe_expiry(apex.expires.as_ref()), "forever");
        let method = rule_for(&held, "api.github.com", 4, 2).expect("a method of the contract");
        assert_eq!(
            method.matcher.as_ref().map(|m| m.methods.as_slice()),
            Some([v1::Method::Post as i32].as_slice())
        );
        assert_eq!(describe_expiry(method.expires.as_ref()), "this session");
    }

    /// Die Anfrage geht als HTTP-Text in den Editor und kommt so zurück.
    #[test]
    fn a_request_survives_the_way_through_the_editor() {
        let request = v1::HttpRequest {
            method: v1::Method::Post as i32,
            method_raw: String::new(),
            scheme: v1::Scheme::Https as i32,
            authority: Some(v1::Authority {
                host: "api.github.com".to_owned(),
                port: 443,
                ..v1::Authority::default()
            }),
            path_and_query: "/graphql".to_owned(),
            headers: vec![v1::Header {
                name: "Content-Type".to_owned(),
                value: b"application/json".to_vec(),
            }],
            body: None,
            version: "HTTP/1.1".to_owned(),
        };
        let text = request_text(&request, "{\"query\":\"{viewer{login}}\"}");
        assert!(
            text.starts_with("POST https://api.github.com:443/graphql\n"),
            "{text}"
        );

        let back = parse_request(&text).expect("the text parses");
        assert_eq!(back.method, v1::Method::Post as i32);
        assert_eq!(back.url, "https://api.github.com:443/graphql");
        assert_eq!(back.headers.len(), 1);
        assert_eq!(back.headers[0].name, "Content-Type");
        assert_eq!(back.headers[0].value, b"application/json".to_vec());
        assert_eq!(back.body, b"{\"query\":\"{viewer{login}}\"}".to_vec());
    }

    /// Ein Rumpf mit `\r\n` und letztem Umbruch kommt unverändert zurück.
    ///
    /// `multipart/form-data` trägt `\r\n` um jede Grenze. Wer den Rumpf über
    /// `lines()` wieder zusammensetzte, machte daraus `\n`, und der Server
    /// fände seine Grenzen nicht mehr -- eine Anfrage, die anders hinausgeht,
    /// als der Mensch sie gesehen hat.
    #[test]
    fn a_body_keeps_its_bytes_through_the_editor() {
        let request = v1::HttpRequest {
            method: v1::Method::Post as i32,
            method_raw: String::new(),
            scheme: v1::Scheme::Https as i32,
            authority: Some(v1::Authority {
                host: "upload.test".to_owned(),
                port: 443,
                ..v1::Authority::default()
            }),
            path_and_query: "/files".to_owned(),
            headers: vec![v1::Header {
                name: "Content-Type".to_owned(),
                value: b"multipart/form-data; boundary=x".to_vec(),
            }],
            body: None,
            version: "HTTP/1.1".to_owned(),
        };
        let body = "--x\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nvalue\r\n--x--\r\n";
        let text = request_text(&request, body);
        let back = parse_request(&text).expect("the text parses");
        assert_eq!(
            String::from_utf8(back.body).expect("utf-8 in, utf-8 out"),
            body,
            "every carriage return and the last newline survive"
        );
    }

    /// Ein Editor, der `\r\n` schreibt, verschiebt den Rumpf nicht.
    ///
    /// Der erste Entwurf zählte Offsets mit `+ 1` je Umbruch; bei `\r\n` ist
    /// er zwei Zeichen lang, und der Rumpf begann je Kopfzeile ein Zeichen zu
    /// früh -- im schlimmsten Fall mitten in einem Zeichen, und dann war er
    /// weg.
    #[test]
    fn a_file_with_crlf_line_endings_keeps_its_body() {
        let text = "POST https://example.com/x\r\nA: 1\r\nB: 2\r\n\r\nbody\r\nmore\r\n";
        let back = parse_request(text).expect("the text parses");
        assert_eq!(back.headers.len(), 2);
        assert_eq!(back.headers[0].name, "A");
        assert_eq!(back.headers[1].value, b"2".to_vec());
        assert_eq!(
            String::from_utf8(back.body).expect("utf-8"),
            "body\r\nmore\r\n"
        );
    }

    /// Eine letzte Kopfzeile ohne Umbruch fällt nicht weg.
    ///
    /// Ein Editor, der keinen letzten Umbruch schreibt, ist der Normalfall,
    /// und eine Kopfzeile, die dabei verschwindet, ginge als Anfrage hinaus,
    /// die der Mensch nicht geschrieben hat -- `Authorization` zum Beispiel.
    #[test]
    fn a_last_header_without_a_newline_survives() {
        for text in [
            "GET https://example.com/\nA: b",
            "GET https://example.com/\r\nA: b",
            "GET https://example.com/\nA: b\nC: d",
        ] {
            let back = parse_request(text).expect("the text parses");
            assert_eq!(
                back.headers.first().map(|h| h.name.as_str()),
                Some("A"),
                "{text:?}"
            );
            assert_eq!(
                back.headers.last().map(|h| h.value.clone()),
                Some(if text.ends_with("C: d") {
                    b"d".to_vec()
                } else {
                    b"b".to_vec()
                }),
                "{text:?}"
            );
            assert!(back.body.is_empty(), "{text:?}");
        }
        // Und eine, die keine ist, wird weiterhin abgelehnt.
        assert!(parse_request("GET https://example.com/\nnot a header").is_err());
    }

    /// Eine Datei aus nichts als der Anfragezeile hat keinen Rumpf.
    #[test]
    fn a_file_with_only_a_request_line_has_no_body() {
        for text in [
            "GET https://example.com/",
            "GET https://example.com/\n",
            "GET https://example.com/\r\n",
        ] {
            let back = parse_request(text).expect("the text parses");
            assert!(back.headers.is_empty(), "{text:?}");
            assert!(back.body.is_empty(), "{text:?}");
        }
    }

    /// Ein leerer Kopfteil ist kein leerer Rumpf.
    #[test]
    fn a_request_without_headers_keeps_its_body() {
        let back = parse_request("POST https://example.com/x\n\nplain body").expect("parses");
        assert!(back.headers.is_empty());
        assert_eq!(String::from_utf8(back.body).expect("utf-8"), "plain body");
    }

    /// Ein Rumpf, den niemand angefasst hat, bleibt leer.
    #[test]
    fn a_request_without_a_body_stays_without_one() {
        let request = v1::HttpRequest {
            method: v1::Method::Get as i32,
            scheme: v1::Scheme::Https as i32,
            authority: Some(v1::Authority {
                host: "example.com".to_owned(),
                port: 443,
                ..v1::Authority::default()
            }),
            path_and_query: "/".to_owned(),
            ..v1::HttpRequest::default()
        };
        let text = request_text(&request, "");
        let back = parse_request(&text).expect("the text parses");
        assert!(back.body.is_empty(), "{:?}", back.body);
    }

    /// Was nicht zu lesen ist, wird abgelehnt und nicht geraten.
    ///
    /// Der Weg endet in einer Anfrage, die wirklich hinausgeht; eine geratene
    /// Zeile wäre eine Anfrage, die niemand so geschrieben hat.
    #[test]
    fn an_unreadable_file_is_refused_instead_of_guessed() {
        assert!(parse_request("").is_err(), "an empty file");
        assert!(parse_request("POST\n").is_err(), "a line without a URL");
        assert!(
            parse_request("POST /graphql\n").is_err(),
            "a URL without a scheme"
        );
        assert!(
            parse_request("POST https://example.com/#top\n").is_err(),
            "a URL with a fragment"
        );
        let no_colon = parse_request("GET https://example.com/\nAccept application/json\n");
        assert!(no_colon.is_err(), "a header line without a colon");
    }

    /// Die Datei des Editors trägt keinen Pfad, den ein Fluss ihr gibt.
    ///
    /// Der Name kommt aus einer Kennung des Daemons; sie ist heute eine UUID,
    /// und morgen ist sie es vielleicht nicht mehr. Ein `..` darin schriebe
    /// sonst neben das Laufzeitverzeichnis.
    #[test]
    fn the_name_of_the_file_carries_no_path() {
        // Ohne Laufzeitverzeichnis gibt es keine Datei; der Test läuft nur,
        // wo eines steht, und sagt es sonst.
        let Ok(clean) = edit_path("018f-0000-7000") else {
            clients_skip(
                "the_name_of_the_file_carries_no_path",
                "XDG_RUNTIME_DIR is not set",
            );
            return;
        };
        assert_eq!(
            clean.file_name().and_then(std::ffi::OsStr::to_str),
            Some("edit-018f-0000-7000.http")
        );
        let escaped = edit_path("../../etc/passwd").expect("the same environment");
        assert_eq!(
            escaped.file_name().and_then(std::ffi::OsStr::to_str),
            Some("edit-etcpasswd.http")
        );
        assert_eq!(escaped.parent(), clean.parent());
        assert!(
            clean.starts_with(std::env::var_os("XDG_RUNTIME_DIR").expect("set above")),
            "the file lies under the runtime directory and nowhere else: {clean:?}"
        );
    }

    /// Eine Zeile für den Menschen, der das Protokoll liest.
    fn clients_skip(test: &str, why: &str) {
        eprintln!("SKIP {test}: {why}");
    }

    /// Wo keine ehrliche Regel entsteht, entsteht keine.
    ///
    /// Der Apex kommt vom Dienst; ohne ihn wäre `**.<geraten>` breiter als
    /// das, was auf dem Schirm stand. Eine Methode, die der Vertrag nicht
    /// benennt, ergäbe eine leere Liste -- und eine leere Liste trifft jede
    /// Methode. Ein Pfad mit `*` ist für die Engine ein Muster und nicht die
    /// Adresse, die der Mensch gesehen hat.
    #[test]
    fn a_target_that_would_widen_the_rule_is_refused() {
        let mut no_apex = held();
        no_apex.apex = String::new();
        let refused = rule_for(&no_apex, "192.168.1.50", 3, 2).expect_err("no apex, no rule");
        assert_eq!(refused.code.as_str(), "CLI_004");

        let mut other_method = held();
        other_method.method = v1::Method::Other as i32;
        assert!(rule_for(&other_method, "api.github.com", 4, 2).is_err());

        // Jedes Zeichen, das `globset` als Muster liest -- nicht nur der Stern.
        for path in [
            "/search/*",
            "/a?b",
            "/logs/[0-9]",
            "/repos/{owner}/x",
            "/a\\b",
        ] {
            let mut pattern = held();
            pattern.path = path.to_owned();
            assert!(
                rule_for(&pattern, "api.github.com", 1, 2).is_err(),
                "{path} carries a pattern"
            );
        }
    }

    /// Der Rumpf zeigt Text als Text und alles andere als Hex.
    #[test]
    fn the_body_shows_text_as_text_and_bytes_as_hex() {
        let lines = body_lines(b"hello\n\x00\x01\x02world\n");
        assert_eq!(lines[0], "hello");
        assert_eq!(lines[1], "  00 01 02");
        assert_eq!(lines[2], "world");
    }

    /// Eine Steuerfolge im Rumpf erreicht das Terminal nicht als Folge.
    ///
    /// `GetBody` liefert Rohbytes, und am Terminal-Strom sitzt der Filter des
    /// Daemons -- hier nicht. Ohne diese Zeile öffnete `v` genau den
    /// Seitenkanal wieder, den HUM-042 geschlossen hat.
    #[test]
    fn an_escape_in_the_body_never_leaves_as_an_escape() {
        let lines = body_lines(b"before\x1b]52;c;SGVsbG8=\x07after");
        let text = lines.join("\n");
        assert!(!text.contains('\u{1b}'), "{text:?}");
        assert!(!text.contains('\u{7}'), "{text:?}");
        assert!(text.contains("1b"), "the byte is shown as hex: {text:?}");
    }
}
