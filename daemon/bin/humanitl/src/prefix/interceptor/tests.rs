//! Tests des Interceptors gegen einen Fake-Daemon (GitHub #12).
//!
//! Der Fake hält die Gegenseite der Leitung: Er legt Antworten und Ereignisse
//! hinein, wie sie aus `ListFlows`, `Subscribe` und `GetFlow` kämen, und liest
//! jede Anfrage, die der Bildschirm hinausschickt. Gezeichnet wird in ein
//! `TestBackend` von ratatui.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use humanitl_ipc::v1;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::link::{self, Answer, Far, Request};
use super::screen::{InterceptorScreen, MIN, READ_ONLY};
use crate::tui::frame::Theme;
use crate::tui::screen::{Flow, Screen};

/// Die feste Uhr der Tests.
const NOW: i64 = 1_800_000_000;

fn clock() -> i64 {
    NOW
}

/// Ein gehaltener Fluss, vor `waited` Sekunden angekommen, mit `left`
/// Sekunden bis zur Frist.
fn held(flow_id: &str, host: &str, path: &str, waited: i64, left: i64) -> v1::FlowSummary {
    v1::FlowSummary {
        flow_id: flow_id.to_owned(),
        method: v1::Method::Post as i32,
        authority: Some(v1::Authority {
            host: host.to_owned(),
            port: 443,
            ..v1::Authority::default()
        }),
        path: path.to_owned(),
        state: v1::FlowState::Held as i32,
        request_size: 2150,
        finding_count: 1,
        received_at: Some(prost_types::Timestamp {
            seconds: NOW - waited,
            nanos: 0,
        }),
        deadline: Some(prost_types::Timestamp {
            seconds: NOW + left,
            nanos: 0,
        }),
        ..v1::FlowSummary::default()
    }
}

/// Die Einzelheiten zu einem Fluss, mit Rumpf.
fn detail(summary: &v1::FlowSummary, body: &str) -> v1::FlowDetail {
    v1::FlowDetail {
        summary: Some(summary.clone()),
        body_preview: body.to_owned(),
        domain: Some(v1::DomainInfo {
            apex: "example.test".to_owned(),
            ..v1::DomainInfo::default()
        }),
        ..v1::FlowDetail::default()
    }
}

/// Bildschirm und Fake.
fn screen(read_only: bool) -> (InterceptorScreen, Far) {
    let (link, far) = link::pair();
    (
        InterceptorScreen::new(link, read_only, clock).with_measure(measured),
        far,
    )
}

/// Legt eine Antwort in die Leitung und lässt den Bildschirm sie nehmen.
fn answer(screen: &mut InterceptorScreen, far: &Far, answer: Answer) {
    far.answers.send(answer).expect("the screen listens");
    screen.changed();
}

/// Alle Anfragen, die bisher hinausgingen.
fn calls(far: &mut Far) -> Vec<Request> {
    let mut out = Vec::new();
    while let Ok(request) = far.calls.try_recv() {
        out.push(request);
    }
    out
}

/// Nur die Entscheidungen darunter.
fn decisions(far: &mut Far) -> Vec<v1::DecideRequest> {
    calls(far)
        .into_iter()
        .filter_map(|request| match request {
            Request::Decide(decide) => Some(*decide),
            Request::Detail(_) => None,
        })
        .collect()
}

thread_local! {
    /// Die Größe, die das Terminal des Tests gerade hat: die des letzten
    /// Bildes, außer ein Test ändert sie dazwischen.
    static TERMINAL: std::cell::Cell<(u16, u16)> = const { std::cell::Cell::new((0, 0)) };
}

fn measured() -> (u16, u16) {
    TERMINAL.with(std::cell::Cell::get)
}

/// Die Größe, in der die Tests sonst zeichnen.
const SIZE: (u16, u16) = (100, 24);

/// Eine Taste, wie die Schleife sie gibt: erst ein Bild in `SIZE`, dann die
/// Taste.
fn press(screen: &mut InterceptorScreen, code: KeyCode) -> Flow {
    press_at(screen, code, SIZE)
}

/// Wie [`press`], nach einem Bild in `size`.
fn press_at(screen: &mut InterceptorScreen, code: KeyCode, size: (u16, u16)) -> Flow {
    shot(screen, size);
    screen.on_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

/// Zeichnet den Bildschirm wie die Schleife (`tui::screen`): unter der
/// Mindestgröße nur den Hinweis, und danach sagt sie dem Bildschirm, ob er
/// zu sehen war. Gibt die Zeilen als Text zurück.
fn shot(screen: &mut InterceptorScreen, size: (u16, u16)) -> Vec<String> {
    let visible = size.0 >= MIN.cols && size.1 >= MIN.rows;
    TERMINAL.with(|terminal| terminal.set(size));
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).expect("a terminal");
    terminal
        .draw(|frame| {
            if visible {
                screen.draw(frame, Theme::from_no_color(Some(std::ffi::OsStr::new("1"))));
            }
        })
        .expect("drawn");
    screen.drawn(visible);
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buffer[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect()
}

/// Zwei gehaltene Flüsse: Beide stehen in der Liste, die Wahl wandert mit der
/// Pfeiltaste, und `a` schickt `Decide(allow)` für genau den gewählten, den
/// zweiten. Mutationsbeweis: die Entscheidung für den ersten Fluss der Liste
/// statt für den gewählten macht ihn rot.
#[test]
fn a_decides_exactly_the_selected_flow() {
    let (mut screen, mut far) = screen(false);
    let first = held("f1", "api.example.test", "/one", 30, 240);
    let second = held("f2", "upload.example.test", "/two", 5, 290);
    answer(
        &mut screen,
        &far,
        Answer::Listed(vec![first.clone(), second.clone()]),
    );
    assert_eq!(screen.flow_ids(), vec!["f1", "f2"]);
    let text = shot(&mut screen, SIZE).join("\n");
    assert!(text.contains("api.example.test/one"), "{text}");
    assert!(text.contains("upload.example.test/two"), "{text}");
    assert_eq!(screen.selected(), None, "nothing is selected by itself");

    press(&mut screen, KeyCode::Down);
    assert_eq!(screen.selected(), Some("f1"));
    press(&mut screen, KeyCode::Down);
    assert_eq!(screen.selected(), Some("f2"));
    answer(&mut screen, &far, Answer::Detail(detail(&second, "{}")));
    assert!(
        calls(&mut far).contains(&Request::Detail("f2".to_owned())),
        "the selection fetches its detail"
    );

    press(&mut screen, KeyCode::Char('a'));
    let sent = decisions(&mut far);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].flow_ids, vec!["f2".to_owned()]);
    assert_eq!(
        sent[0].decision,
        Some(v1::decide_request::Decision::Allow(()))
    );
    assert!(sent[0].remember.is_none());
    // Die Zeile bleibt, bis der Daemon die Entscheidung meldet (ADR-018).
    assert_eq!(screen.flow_ids(), vec!["f1", "f2"]);
    answer(&mut screen, &far, Answer::Gone("f2".to_owned()));
    assert_eq!(screen.flow_ids(), vec!["f1"]);

    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Char('b'));
    let blocked = decisions(&mut far);
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].flow_ids, vec!["f1".to_owned()]);
    assert!(matches!(
        blocked[0].decision,
        Some(v1::decide_request::Decision::Block(_))
    ));
}

/// Ein Fluss, über den anderswo entschieden wird, während der Interceptor
/// offen ist, verschwindet aus der Liste. War er gewählt, ist danach nichts
/// gewählt: Ein `a` erlaubt nicht den Nachbarn, der an seine Stelle rückt,
/// erst eine Pfeiltaste wählt neu. Mutationsbeweis: die Wahl auf den
/// Nachbarn weiterrücken lassen macht ihn rot.
#[test]
fn a_flow_decided_elsewhere_disappears() {
    let (mut screen, mut far) = screen(false);
    let first = held("f1", "api.example.test", "/one", 30, 240);
    let second = held("f2", "upload.example.test", "/two", 5, 290);
    answer(&mut screen, &far, Answer::Listed(vec![first, second]));
    press(&mut screen, KeyCode::Down);
    assert_eq!(screen.selected(), Some("f1"));
    answer(&mut screen, &far, Answer::Gone("f1".to_owned()));
    assert_eq!(screen.flow_ids(), vec!["f2"]);
    let text = shot(&mut screen, SIZE).join("\n");
    assert!(!text.contains("api.example.test"), "{text}");
    assert_eq!(screen.selected(), None);

    press(&mut screen, KeyCode::Char('a'));
    assert!(
        decisions(&mut far).is_empty(),
        "no decision without a choice"
    );

    press(&mut screen, KeyCode::Down);
    assert_eq!(screen.selected(), Some("f2"));

    // Ein neu gehaltener Fluss reiht sich ein und ändert die Wahl nicht; von
    // selbst wird nichts entschieden.
    answer(
        &mut screen,
        &far,
        Answer::Arrived(held("f3", "late.example.test", "/", 0, 300)),
    );
    assert_eq!(screen.flow_ids(), vec!["f2", "f3"]);
    assert_eq!(screen.selected(), Some("f2"));
    assert!(decisions(&mut far).is_empty());

    // Nach verworfenen Ereignissen gilt die neue Liste des Daemons.
    answer(
        &mut screen,
        &far,
        Answer::Listed(vec![held("f3", "late.example.test", "/", 1, 299)]),
    );
    assert_eq!(screen.flow_ids(), vec!["f3"]);
    assert_eq!(screen.selected(), None, "the selected flow is gone");
}

/// Ein Fluss, der kommt, während nichts gewählt ist, wird nicht von selbst
/// gewählt: Ein `a`, das im selben Augenblick getippt wurde, trifft ihn nie
/// (Review zu GitHub #12). Erst eine Pfeiltaste wählt. Mutationsbeweis: die
/// Wahl eines ankommenden Flusses, solange nichts gewählt ist, macht ihn rot.
#[test]
fn an_arriving_flow_is_never_selected_by_itself() {
    let (mut screen, mut far) = screen(false);
    answer(&mut screen, &far, Answer::Listed(Vec::new()));
    answer(
        &mut screen,
        &far,
        Answer::Arrived(held("f1", "api.example.test", "/one", 0, 300)),
    );
    assert_eq!(screen.selected(), None);
    press(&mut screen, KeyCode::Char('a'));
    press(&mut screen, KeyCode::Char('b'));
    assert!(
        decisions(&mut far).is_empty(),
        "no decision for an unseen flow"
    );
    press(&mut screen, KeyCode::Down);
    assert_eq!(screen.selected(), Some("f1"));
}

/// Unter der Mindestgröße steht nur „Terminal too small": Dann wirkt keine
/// Taste außer `Esc` und `q`, auch nicht für einen gewählten Fluss (Review
/// zu GitHub #12). Geprüft auch für ein Terminal, das nach dem Hinweis
/// schon wieder die Größe des letzten ganzen Bildes hat, aber noch nicht neu
/// gezeichnet ist: Dort hält der Vergleich der Größe nichts auf, nur der
/// Hinweis zählt. Mutationsbeweis: Tasten auch ohne sichtbaren Bildschirm
/// annehmen macht ihn rot.
#[test]
fn keys_do_nothing_while_the_terminal_is_too_small() {
    let (mut screen, mut far) = screen(false);
    let first = held("f1", "api.example.test", "/one", 3, 200);
    answer(&mut screen, &far, Answer::Listed(vec![first.clone()]));
    press(&mut screen, KeyCode::Down);
    answer(&mut screen, &far, Answer::Detail(detail(&first, "")));
    let small = (59, 30);
    for key in ['a', 'b', 'r', '1', '2'] {
        assert_eq!(
            press_at(&mut screen, KeyCode::Char(key), small),
            Flow::Continue
        );
    }
    assert_eq!(press_at(&mut screen, KeyCode::Enter, small), Flow::Continue);
    // Zurück auf die Größe des letzten ganzen Bildes, ohne neues Bild.
    shot(&mut screen, small);
    TERMINAL.with(|terminal| terminal.set(SIZE));
    for key in ['a', 'b'] {
        let event = Event::Key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
        assert_eq!(screen.on_event(&event), Flow::Continue);
    }
    assert!(
        decisions(&mut far).is_empty(),
        "nothing invisible is decided"
    );
    assert_eq!(press_at(&mut screen, KeyCode::Esc, small), Flow::Done);
    assert_eq!(press_at(&mut screen, KeyCode::Char('q'), small), Flow::Done);
}

/// Schrumpft das Terminal, und die Taste kommt vor dem Ereignis der
/// Größenänderung, beschreibt das letzte Bild einen Schirm, den es nicht mehr
/// gibt: `a` und `Enter` wirken dann nicht, die Fußzeile sagt es, und nach
/// dem nächsten Bild in der neuen Größe geht es wieder (Review zu GitHub
/// #12). Mutationsbeweis: die Größe nicht vergleichen macht ihn rot.
#[test]
fn a_key_after_a_resize_without_a_new_frame_does_nothing() {
    let (mut screen, mut far) = screen(false);
    let first = held("f1", "api.example.test", "/one", 3, 200);
    answer(&mut screen, &far, Answer::Listed(vec![first.clone()]));
    press(&mut screen, KeyCode::Down);
    answer(&mut screen, &far, Answer::Detail(detail(&first, "")));
    shot(&mut screen, SIZE);
    TERMINAL.with(|terminal| terminal.set((60, 12)));
    let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
    screen.on_event(&key(KeyCode::Char('a')));
    assert!(decisions(&mut far).is_empty(), "a stale frame decided");
    let text = shot(&mut screen, (60, 12)).join("\n");
    assert!(text.contains("the terminal changed size"), "{text}");
    press_at(&mut screen, KeyCode::Char('r'), (60, 12));
    press_at(&mut screen, KeyCode::Char('2'), (60, 12));
    press_at(&mut screen, KeyCode::Char('2'), (60, 12));
    shot(&mut screen, (60, 12));
    TERMINAL.with(|terminal| terminal.set((60, 13)));
    screen.on_event(&key(KeyCode::Enter));
    assert!(decisions(&mut far).is_empty(), "Enter on a stale frame");
    press_at(&mut screen, KeyCode::Char('a'), (60, 13));
    assert!(
        decisions(&mut far).is_empty(),
        "the rule was dropped, a waits"
    );
    press_at(&mut screen, KeyCode::Char('a'), SIZE);
    assert_eq!(decisions(&mut far).len(), 1, "fine after a fresh frame");
}

/// Host, Pfad, Methode und Rumpf sind Text des Angreifers: Sie laufen durch
/// denselben Säuberer wie der Kasten von `--ask terminal`. Kein Steuerzeichen
/// und kein unsichtbares Zeichen erreicht eine Zelle, und ein Zeichen außerhalb
/// von ASCII wird `?`. Mutationsbeweis: Host oder Rumpf ohne
/// `display_text` zeichnen macht ihn rot.
#[test]
fn hosts_and_bodies_are_sanitized_like_the_terminal_box() {
    let (mut screen, far) = screen(false);
    let mut evil = held(
        "f1",
        "ex\u{0430}mple\u{202e}.test",
        "/\u{1b}]8;;https://x.test\u{7}click\u{1b}[2J",
        3,
        200,
    );
    evil.method = v1::Method::Other as i32;
    evil.method_raw = "PO\u{1b}[31mST".to_owned();
    answer(&mut screen, &far, Answer::Listed(vec![evil.clone()]));
    press(&mut screen, KeyCode::Down);
    answer(
        &mut screen,
        &far,
        Answer::Detail(detail(
            &evil,
            "{\"a\":\"\u{1b}[31mred\u{7}\"}\nsecond \u{9b}line \u{0442}ext",
        )),
    );
    let lines = shot(&mut screen, (120, 24));
    let text = lines.join("\n");
    for line in &lines {
        assert!(
            line.chars()
                .all(|c| !c.is_control() && c != '\u{202e}' && c != '\u{9b}'),
            "{line:?}"
        );
    }
    // In der Zeile der Liste und im Kopf darunter, jeweils gesäubert.
    let row = lines
        .iter()
        .find(|line| line.starts_with("> "))
        .expect("the selected row");
    assert!(
        row.contains("ex?mple.test/]8;;https://x.testclick[2J"),
        "{row}"
    );
    assert!(
        text.contains("PO[31mST https://ex?mple.test/]8;;"),
        "the head: {text}"
    );
    assert!(text.contains("click"), "the visible text stays: {text}");
    assert!(text.contains("PO[31mST"), "{text}");
    assert!(text.contains("{\"a\":\"[31mred\"}"), "{text}");
    assert!(text.contains("second line ?ext"), "{text}");
}

/// Die Frist ist zu sehen und läuft ab: `04:00` übrig, und eine Anfrage,
/// deren Frist vorbei ist, steht als `expired` da, bis der Daemon sie
/// streicht.
#[test]
fn the_deadline_is_visible() {
    let (mut screen, far) = screen(false);
    answer(
        &mut screen,
        &far,
        Answer::Listed(vec![
            held("f1", "api.example.test", "/one", 75, 240),
            held("f2", "old.example.test", "/two", 300, -2),
        ]),
    );
    let text = shot(&mut screen, SIZE).join("\n");
    assert!(text.contains("04:00"), "the time left: {text}");
    assert!(text.contains("01:15"), "the time waited: {text}");
    assert!(text.contains("expired"), "{text}");
}

/// Ein lesender Anschluss zeigt die Liste und entscheidet nichts: `a`, `b`
/// und `r` schicken kein `Decide`, und die Fußzeile sagt, wo entschieden
/// wird.
#[test]
fn a_read_only_attach_decides_nothing() {
    let (mut screen, mut far) = screen(true);
    let first = held("f1", "api.example.test", "/one", 3, 200);
    answer(&mut screen, &far, Answer::Listed(vec![first.clone()]));
    press(&mut screen, KeyCode::Down);
    answer(&mut screen, &far, Answer::Detail(detail(&first, "")));
    for key in ['a', 'b', 'r', '2', '2'] {
        press(&mut screen, KeyCode::Char(key));
    }
    press(&mut screen, KeyCode::Enter);
    assert!(decisions(&mut far).is_empty());
    let text = shot(&mut screen, (140, 20)).join("\n");
    assert!(text.contains(&READ_ONLY[..40]), "{text}");
}

/// `r`: Ziel und Dauer, dann `Enter`; die Regel geht mit `Decide(allow)` für
/// genau den gewählten Fluss hinaus, wie in `--ask terminal`. Der Satz vor
/// der Bestätigung sagt, was die Regel offen lässt: jede Abfrage, jede
/// Methode, beide Schemata, jeden Port (Review zu GitHub #12). Ohne `Enter`
/// geht nichts hinaus, und `Esc` bricht ab, ohne den Interceptor zu
/// schließen.
#[test]
fn a_rule_needs_target_duration_and_enter() {
    let (mut screen, mut far) = screen(false);
    let first = held("f1", "api.example.test", "/one", 3, 200);
    answer(&mut screen, &far, Answer::Listed(vec![first.clone()]));
    press(&mut screen, KeyCode::Down);
    answer(&mut screen, &far, Answer::Detail(detail(&first, "")));

    press(&mut screen, KeyCode::Char('r'));
    assert_eq!(
        press(&mut screen, KeyCode::Esc),
        Flow::Continue,
        "cancels only"
    );
    press(&mut screen, KeyCode::Char('r'));
    let menu = shot(&mut screen, SIZE).join("\n");
    assert!(
        menu.contains("any query"),
        "the target names its scope: {menu}"
    );
    assert!(
        menu.contains("http/https/ws/wss, any port, no WebSocket upgrade"),
        "{menu}"
    );
    press(&mut screen, KeyCode::Char('9'));
    press(&mut screen, KeyCode::Char('1'));
    press(&mut screen, KeyCode::Char('2'));
    let text = shot(&mut screen, SIZE).join("\n");
    assert!(
        text.contains("path: /one with any query"),
        "the dropped query is named: {text}"
    );
    assert!(text.contains("http, https, ws or wss"), "{text}");
    assert!(text.contains("not a WebSocket upgrade"), "{text}");
    assert!(decisions(&mut far).is_empty(), "nothing before Enter");
    press(&mut screen, KeyCode::Enter);
    let sent = decisions(&mut far);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].flow_ids, vec!["f1".to_owned()]);
    let rule = sent[0].remember.as_ref().expect("a rule");
    let matcher = rule.matcher.as_ref().expect("a matcher");
    assert_eq!(matcher.host, "api.example.test");
    assert_eq!(matcher.path, "/one");
    assert_eq!(
        rule.expires.as_ref().and_then(|expiry| expiry.expiry),
        Some(v1::rule_expiry::Expiry::Session(()))
    );
    assert_eq!(press(&mut screen, KeyCode::Esc), Flow::Done);
}

/// Eine Frage der Regel wirkt erst, wenn sie ganz auf dem Schirm steht: In
/// 60 × 12 mit langer Liste passt das Menü der Ziele nicht, seine Ziffern
/// warten, und die Fußzeile bittet um ein größeres Terminal; ebenso `Enter`
/// unter einem Satz, der nicht ganz zu sehen ist. In einem größeren Terminal
/// gehen beide (Review zu GitHub #12). Mutationsbeweis: `Enter` auch ohne
/// ganz sichtbaren Satz annehmen macht ihn rot.
#[test]
fn a_rule_waits_until_its_whole_question_is_visible() {
    let (mut screen, mut far) = screen(false);
    let long = format!("{}.example.test", "a".repeat(150));
    let mut rows: Vec<v1::FlowSummary> = (0..9)
        .map(|n| held(&format!("f{n}"), "other.example.test", "/", 3, 200))
        .collect();
    let first = held("long", &long, "/one", 3, 200);
    rows.insert(0, first.clone());
    answer(&mut screen, &far, Answer::Listed(rows));
    press(&mut screen, KeyCode::Down);
    answer(&mut screen, &far, Answer::Detail(detail(&first, "")));
    let tight = (60, 12);

    press_at(&mut screen, KeyCode::Char('r'), tight);
    press_at(&mut screen, KeyCode::Char('1'), tight);
    let text = shot(&mut screen, tight).join("\n");
    assert!(text.contains("enlarge the terminal"), "{text}");
    press(&mut screen, KeyCode::Char('1'));
    press(&mut screen, KeyCode::Char('2'));

    press_at(&mut screen, KeyCode::Enter, tight);
    assert!(
        decisions(&mut far).is_empty(),
        "Enter under a sentence that was not wholly visible"
    );
    let text = shot(&mut screen, SIZE).join("\n");
    assert!(
        text.contains(&long[..60]),
        "the whole sentence, wrapped: {text}"
    );
    assert!(text.contains("any port"), "{text}");
    press(&mut screen, KeyCode::Enter);
    assert_eq!(decisions(&mut far).len(), 1);
}

/// Eine Entscheidung, die der Daemon ablehnt, steht mit ihrem Grund in der
/// Fußzeile, gesäubert.
#[test]
fn a_refused_decision_says_why() {
    let (mut screen, far) = screen(false);
    answer(
        &mut screen,
        &far,
        Answer::Listed(vec![held("f1", "api.example.test", "/one", 3, 200)]),
    );
    answer(
        &mut screen,
        &far,
        Answer::Decided(v1::DecideResponse {
            results: vec![v1::DecideResult {
                flow_id: "f1".to_owned(),
                applied: false,
                diagnostic: Some(v1::Diagnostic {
                    why: "already decided\u{1b}[2J".to_owned(),
                    ..v1::Diagnostic::default()
                }),
            }],
            ..v1::DecideResponse::default()
        }),
    );
    let text = shot(&mut screen, (120, 20)).join("\n");
    assert!(text.contains("not decided: already decided[2J"), "{text}");
}

/// Die Liste nennt den Host als A-Label, auch wenn der Daemon eine
/// Unicode-Form mitschickt; diese steht nirgends an seiner Stelle.
/// Mutationsbeweis: `row_text` den Host aus `display_host` nehmen lassen
/// macht den Test rot.
#[test]
fn the_list_shows_the_a_label() {
    let (mut screen, far) = screen(false);
    let mut flow = held("f1", "xn--mnchen-3ya.de", "/one", 30, 240);
    if let Some(authority) = flow.authority.as_mut() {
        authority.display_host = "münchen.de".to_owned();
    }
    answer(&mut screen, &far, Answer::Listed(vec![flow]));
    let text = shot(&mut screen, SIZE).join("\n");
    assert!(text.contains("xn--mnchen-3ya.de/one"), "{text}");
    assert!(!text.contains("münchen"), "{text}");
}
