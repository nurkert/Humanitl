//! Tests des Erststart-Bildschirms gegen eine Gegenseite, die die Anfragen
//! der Leitung aufzeichnet und beantwortet (GitHub #9).
//!
//! Gezeichnet wird in ein `TestBackend` von ratatui; ein echtes Terminal und
//! ein echter Daemon kommen nie vor. Mit `HUMANITL_TUI_SHOTS=<Ordner>` legt
//! jeder Schnappschuss seine Zeilen als Text dort ab.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::{Exit, FirstStartScreen, Focus};
use crate::tui::frame::Theme;
use crate::tui::screen::{Flow, Screen};
use crate::tui::settings::link::{self, Answer, Far, Reply, Request};

/// Der Ordner der Tests.
const PATH: &str = "/home/u/shop";

fn pair(key: &str, value: &str) -> v1::ChoiceValue {
    v1::ChoiceValue {
        key: key.to_owned(),
        value: value.to_owned(),
    }
}

fn found(code: &str, why: &str) -> v1::Diagnostic {
    v1::Diagnostic {
        code: code.to_owned(),
        severity: v1::Severity::Error as i32,
        why: why.to_owned(),
        ..v1::Diagnostic::default()
    }
}

fn option(value: &str, label: &str, choice: &[(&str, &str)]) -> v1::ProposalOption {
    v1::ProposalOption {
        value: value.to_owned(),
        label: label.to_owned(),
        available: true,
        choice: choice.iter().map(|(key, value)| pair(key, value)).collect(),
        ..v1::ProposalOption::default()
    }
}

fn unavailable(mut option: v1::ProposalOption, code: &str, why: &str) -> v1::ProposalOption {
    option.available = false;
    option.reasons = vec![found(code, why)];
    option
}

fn field(
    id: &str,
    keys: &[&str],
    value: &str,
    options: Vec<v1::ProposalOption>,
) -> v1::ProposalField {
    v1::ProposalField {
        id: id.to_owned(),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        value: value.to_owned(),
        origin: "default".to_owned(),
        options,
        ..v1::ProposalField::default()
    }
}

/// Ein Vorschlag mit einem Agenten aus zwei Quellen, einer nicht
/// verfügbaren Option, einem gesperrten Feld und einer Frage, die `go` nicht
/// stellen kann.
fn proposal(agent: &str, folder: &str) -> v1::StartProposal {
    let (mode, work_mode, folder_label) = match folder {
        "direct" => ("direct", "rw", "direct"),
        "read_only" => ("direct", "ro", "read-only"),
        _ => ("disposable", "rw", "disposable"),
    };
    v1::StartProposal {
        path: PATH.to_owned(),
        display_name: "shop".to_owned(),
        fields: vec![
            field("agent", &["agent.adapter"], agent, agent_options()),
            field(
                "folder_mode",
                &["sandbox.mode", "sandbox.work_mode"],
                folder,
                folder_options(),
            ),
            locked_isolation(),
            field(
                "ask_mode",
                &["hold.ask_mode"],
                "ui",
                vec![
                    option("ui", "In the app", &[("hold.ask_mode", "ui")]),
                    unavailable(
                        option(
                            "terminal",
                            "In this terminal",
                            &[("hold.ask_mode", "terminal")],
                        ),
                        "CLI_002",
                        "humanitl go hands this terminal to the agent",
                    ),
                ],
            ),
        ],
        choice: vec![
            pair("agent.adapter", agent),
            pair("sandbox.mode", mode),
            pair("sandbox.work_mode", work_mode),
            pair("hold.ask_mode", "ui"),
        ],
        defaults_line: format!(
            "Defaults: agent {agent}, folder {folder_label}, isolation shared (policy), questions \
             in the app"
        ),
        ..v1::StartProposal::default()
    }
}

/// Die Agenten: `OpenCode` vom Host (das Depot hat es nicht), Claude Code,
/// und Codex, das es nirgends gibt.
fn agent_options() -> Vec<v1::ProposalOption> {
    let mut opencode = option("opencode", "OpenCode", &[("agent.adapter", "opencode")]);
    opencode.source = "host".to_owned();
    opencode.sources = vec![
        v1::AgentSourceOption {
            source: "host".to_owned(),
            available: true,
            selected: true,
            ..v1::AgentSourceOption::default()
        },
        v1::AgentSourceOption {
            source: "depot".to_owned(),
            available: false,
            reasons: vec![found("AGENT_008", "the agent depot has no OpenCode")],
            selected: false,
        },
    ];
    vec![
        opencode,
        option("claude", "Claude Code", &[("agent.adapter", "claude")]),
        unavailable(
            option("codex", "Codex", &[("agent.adapter", "codex")]),
            "AGENT_008",
            "codex is neither on the host nor in the agent depot",
        ),
    ]
}

/// Die Ordnermodi.
fn folder_options() -> Vec<v1::ProposalOption> {
    vec![
        option(
            "disposable",
            "Disposable",
            &[("sandbox.mode", "disposable"), ("sandbox.work_mode", "rw")],
        ),
        option(
            "direct",
            "Direct",
            &[("sandbox.mode", "direct"), ("sandbox.work_mode", "rw")],
        ),
        option(
            "read_only",
            "Read-only",
            &[("sandbox.mode", "direct"), ("sandbox.work_mode", "ro")],
        ),
    ]
}

/// Die Isolation, von einer Firmenvorgabe gesperrt.
fn locked_isolation() -> v1::ProposalField {
    let mut isolation = field(
        "isolation",
        &["sandbox.min_isolation"],
        "shared",
        vec![
            option(
                "shared",
                "Shared kernel",
                &[("sandbox.min_isolation", "shared")],
            ),
            option(
                "kernel",
                "Kernel level",
                &[("sandbox.min_isolation", "kernel")],
            ),
        ],
    );
    isolation.origin = "policy".to_owned();
    isolation.lock = Some(v1::SettingLock {
        locked_by: "/etc/humanitl/policy.toml".to_owned(),
        read_only: true,
        reason: "Your company sets the isolation level.".to_owned(),
    });
    isolation
}

/// Ein Bildschirm mit dem ersten Vorschlag und der Gegenseite seiner
/// Leitung.
fn screen() -> (FirstStartScreen, Far) {
    let (link, far) = link::pair();
    let overrides = vec![v1::sandbox_request::CliOverride {
        path: "llm.endpoint".to_owned(),
        value: "http://127.0.0.1:11434".to_owned(),
    }];
    let screen = FirstStartScreen::new(
        link,
        PATH.to_owned(),
        proposal("opencode", "disposable"),
        overrides,
    );
    (screen, far)
}

fn press(screen: &mut FirstStartScreen, code: KeyCode) -> Flow {
    screen.on_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

/// Die `proposal`-Fragen, die bisher auf der Leitung lagen, mit ihrer
/// Zeilenkennung.
fn proposals(far: &mut Far) -> Vec<(String, v1::projects_request::Proposal)> {
    let mut asked = Vec::new();
    while let Ok(call) = far.calls.try_recv() {
        match call.request {
            Request::Projects(v1::ProjectsRequest {
                op: Some(v1::projects_request::Op::Proposal(proposal)),
            }) => asked.push((call.row.unwrap_or_default(), proposal)),
            other => panic!("only proposal goes on the line, not {}", other.rpc()),
        }
    }
    asked
}

/// Beantwortet die Frage `row` mit `proposal`.
fn answer(far: &Far, row: &str, proposal: v1::StartProposal) {
    far.replies
        .send(Reply {
            row: Some(row.to_owned()),
            result: Ok(Answer::Project(v1::ProjectsResponse {
                proposal: Some(proposal),
                ..v1::ProjectsResponse::default()
            })),
        })
        .expect("the screen listens");
}

/// Zeichnet den Bildschirm ohne Farbe und gibt seine Zeilen zurück; mit
/// `HUMANITL_TUI_SHOTS` auch als Datei `first-start-<name>.txt`.
fn shot(screen: &mut FirstStartScreen, name: &str, size: (u16, u16)) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).expect("a terminal");
    terminal
        .draw(|frame| {
            screen.draw(frame, Theme::from_no_color(Some(std::ffi::OsStr::new("1"))));
        })
        .expect("drawn");
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    let lines: Vec<String> = (area.top()..area.bottom())
        .map(|y| {
            let mut line = String::new();
            let mut x = area.left();
            while x < area.right() {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                let width = u16::try_from(ratatui::text::Span::raw(symbol).width()).unwrap_or(1);
                x += width.max(1);
            }
            line.trim_end().to_owned()
        })
        .collect();
    if let Some(dir) = std::env::var_os("HUMANITL_TUI_SHOTS") {
        let path = std::path::Path::new(&dir).join(format!("first-start-{name}.txt"));
        std::fs::write(path, lines.join("\n") + "\n").expect("the shot is written");
    }
    lines
}

/// Schnappschuss zugeklappt: die Vorgaben des Daemons ohne Leitwort,
/// „Start" mit dem Fokus, „Customize…", und keine Felder. `Enter` beendet
/// den Bildschirm mit genau der Wahl des Vorschlags, ohne eine Anfrage.
///
/// Mutation proof: `start` returning a choice of its own (for instance the
/// pairs of the screen's partial choice, empty here) fails the last
/// assertion; dropping `Enter` from the keys leaves the screen open.
#[test]
fn enter_starts_with_the_defaults_of_the_proposal() {
    let (mut screen, mut far) = screen();
    let lines = shot(&mut screen, "collapsed", (80, 18));
    assert_eq!(
        lines[..10],
        [
            " humanitl · First start                                                    shop",
            "",
            " First start in this folder. Everything is preset; Enter starts with these",
            " defaults.",
            " /home/u/shop",
            "",
            " DEFAULTS",
            " agent opencode, folder disposable, isolation shared (policy), questions in",
            " the app",
            "",
        ]
    );
    assert_eq!(lines[10], " [ Start ]  [ Customize… ▸ ]");
    assert!(
        !lines.iter().any(|line| line.contains("START CHOICE")),
        "{lines:#?}"
    );
    assert_eq!(lines[17], " Enter start  a customize  Esc cancel");

    assert_eq!(press(&mut screen, KeyCode::Enter), Flow::Done);
    assert!(proposals(&mut far).is_empty(), "Enter asks nothing");
    assert_eq!(
        screen.take_exit(),
        Exit::Start(proposal("opencode", "disposable").choice)
    );
}

/// Schnappschuss aufgeklappt: jedes Feld mit seinen Optionen, eine nicht
/// verfügbare sichtbar mit ihrem Grund, das gesperrte Feld nur lesbar mit
/// der Vorgabe als Herkunft, die Quellen des Agenten.
#[test]
fn customize_shows_every_field_with_reasons_and_locks() {
    let (mut screen, _far) = screen();
    press(&mut screen, KeyCode::Char('a'));
    let lines = shot(&mut screen, "expanded", (90, 30));
    let text = lines.join("\n");
    assert!(text.contains("[ Customize… ▾ ]"), "{text}");
    assert!(text.contains(" START CHOICE"), "{text}");
    let agent = lines
        .iter()
        .find(|line| line.starts_with(" Agent"))
        .expect("the agent row");
    assert_eq!(agent, " Agent         [OpenCode]  Claude Code  ×Codex");
    for expected in [
        "               Program from the host",
        "               Agent depot: the agent depot has no OpenCode",
        "               × Codex: codex is neither on the host nor in the agent depot",
        " Folder        [Disposable]  Direct   Read-only",
        " Isolation     Shared kernel (locked)",
        "               Set by the company policy /etc/humanitl/policy.toml. Your company sets",
        " Questions     [In the app] ×In this terminal",
        "               × In this terminal: humanitl go hands this terminal to the agent",
    ] {
        assert!(
            lines.iter().any(|line| line == expected),
            "missing {expected:?}:\n{text}"
        );
    }
    assert!(
        !text.contains("Kernel level"),
        "a locked field shows only its value: {text}"
    );
}

/// `Enter` in einem aufgeklappten Feld wählt und startet nicht: Es fragt
/// `proposal` mit der Wahl neu (jeder Schlüssel des Felds geleert, dann die
/// Paare der Option, dazu die Werte der Sitzung), und solange die Antwort
/// fehlt, startet auch `Enter` auf „Start" nicht. Erst mit der Antwort
/// startet `Enter`, mit der Wahl des neuen Vorschlags.
///
/// Mutation proof: `can_start` without `!self.asking` lets the second
/// `Enter` end the screen with the old choice.
#[test]
fn enter_in_a_field_never_starts_before_the_daemon_answered() {
    let (mut screen, mut far) = screen();
    press(&mut screen, KeyCode::Char('a'));
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Down);
    assert_eq!(screen.focus, Focus::Field(1));
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Right);
    assert_eq!(press(&mut screen, KeyCode::Enter), Flow::Continue);
    let asked = proposals(&mut far);
    assert_eq!(asked.len(), 1, "one question");
    let (row, sent) = &asked[0];
    assert_eq!(sent.path, PATH);
    assert_eq!(
        sent.values,
        [
            pair("sandbox.mode", "direct"),
            pair("sandbox.work_mode", "ro")
        ]
    );
    assert_eq!(sent.cli_overrides.len(), 1, "the session values travel");

    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Up);
    assert_eq!(screen.focus, Focus::Start);
    assert_eq!(
        press(&mut screen, KeyCode::Enter),
        Flow::Continue,
        "Start waits for the answer"
    );
    let waiting = shot(&mut screen, "asking", (80, 18)).join("\n");
    assert!(waiting.contains("Asking the daemon"), "{waiting}");

    answer(&far, row, proposal("opencode", "read_only"));
    assert!(screen.changed());
    assert_eq!(press(&mut screen, KeyCode::Enter), Flow::Done);
    assert_eq!(
        screen.take_exit(),
        Exit::Start(proposal("opencode", "read_only").choice)
    );
}

/// Nur die Antwort auf die letzte Frage zählt: Eine ältere, die später
/// ankommt, ändert nichts, und „Start" wartet weiter.
#[test]
fn a_stale_answer_changes_nothing() {
    let (mut screen, mut far) = screen();
    press(&mut screen, KeyCode::Char('a'));
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Enter);
    let asked = proposals(&mut far);
    assert_eq!(asked.len(), 2);
    answer(&far, &asked[0].0, proposal("claude", "disposable"));
    screen.changed();
    assert!(!screen.can_start(), "the latest question is open");
    assert_eq!(screen.proposal().fields[0].value, "opencode");
    answer(&far, &asked[1].0, proposal("claude", "direct"));
    screen.changed();
    assert!(screen.can_start());
    assert_eq!(screen.proposal().fields[1].value, "direct");
}

/// Ein gesperrtes Feld, eine nicht verfügbare Option und die schon gewählte
/// Option schicken keine Frage.
///
/// Mutation proof: `choose` without the check of `available` sends a
/// question for Codex; `choose` and `slide` both without the check of `lock`
/// send one for the isolation.
#[test]
fn locked_fields_and_unavailable_options_are_not_taken() {
    let (mut screen, mut far) = screen();
    press(&mut screen, KeyCode::Char('a'));
    // Agent: Codex ist nicht verfügbar.
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Enter);
    // Isolation: gesperrt; die Markierung bewegt sich nicht.
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Enter);
    // Folder: die schon gewählte Option.
    press(&mut screen, KeyCode::Up);
    press(&mut screen, KeyCode::Enter);
    assert!(proposals(&mut far).is_empty());
    assert!(screen.can_start());
}

/// `Esc` und `q` brechen ab, aus jedem Zustand, ohne Anfrage.
#[test]
fn escape_and_q_cancel() {
    for code in [KeyCode::Esc, KeyCode::Char('q')] {
        let (mut screen, mut far) = screen();
        press(&mut screen, KeyCode::Char('a'));
        press(&mut screen, KeyCode::Down);
        assert_eq!(press(&mut screen, code), Flow::Done, "{code:?}");
        assert_eq!(screen.take_exit(), Exit::Cancel);
        assert!(proposals(&mut far).is_empty());
    }
}

/// Ein Befund des Daemons, der den Start mit den Vorgaben ablehnt, steht da,
/// und „Start" wartet; eine Ablehnung des Speichern steht ebenso da.
///
/// Mutation proof: `can_start` without `refuses` lets `Enter` end the
/// screen.
#[test]
fn a_refusing_finding_holds_start() {
    let (link, _far) = link::pair();
    let mut refusing = proposal("opencode", "disposable");
    refusing.diagnostics = vec![v1::Diagnostic {
        fix: Some(v1::FixAction {
            action: Some(v1::fix_action::Action::CopyCommand(
                "humanitl doctor".to_owned(),
            )),
        }),
        ..found(
            "SANDBOX_045",
            "the kernel level is not available on this machine",
        )
    }];
    let mut screen = FirstStartScreen::new(link, PATH.to_owned(), refusing, Vec::new());
    assert_eq!(press(&mut screen, KeyCode::Enter), Flow::Continue);
    screen.refused(
        Diagnostic::builder(codes::CONFIG_024, Severity::Error)
            .why("a project choice may not carry hold.timeout_secs".to_owned())
            .fix(FixAction::CopyCommand("humanitl projects show".to_owned()))
            .build(),
    );
    let text = shot(&mut screen, "refused", (80, 24)).join("\n");
    for expected in [
        "Start waits",
        "SANDBOX_045",
        "why: the kernel level is not available on this machine",
        "fix: humanitl doctor",
        "CONFIG_024",
        "fix: humanitl projects show",
    ] {
        assert!(text.contains(expected), "missing {expected:?}:\n{text}");
    }
}

/// Ein `ConfigChanged` aus dem Strom fragt neu, mit der Teilwahl des
/// Bildschirms.
#[test]
fn a_change_of_the_configuration_asks_again() {
    let (mut screen, mut far) = screen();
    far.replies
        .send(Reply {
            row: None,
            result: Ok(Answer::Changed(v1::flow_event::ConfigChanged::default())),
        })
        .expect("the screen listens");
    assert!(screen.changed());
    assert_eq!(proposals(&mut far).len(), 1);
    assert!(!screen.can_start(), "Start waits for the new proposal");
}

/// Eine Option, die einen Schlüssel ihres Felds nicht trägt, nimmt ihn aus
/// der Wahl: Er geht leer hinaus (`Proposal.values`), statt den Wert der
/// vorigen Option zu behalten.
///
/// Mutation proof: `choose` without emptying the field's keys first sends no
/// `sandbox.work_mode`, and the assertion fails.
#[test]
fn an_option_without_a_key_of_its_field_takes_it_out() {
    let (link, mut far) = link::pair();
    let mut shown = proposal("opencode", "read_only");
    shown.fields[1]
        .options
        .push(option("auto", "Automatic", &[("sandbox.mode", "auto")]));
    let mut screen = FirstStartScreen::new(link, PATH.to_owned(), shown, Vec::new());
    press(&mut screen, KeyCode::Char('a'));
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Enter);
    let asked = proposals(&mut far);
    assert_eq!(asked.len(), 1);
    let mut values = asked[0].1.values.clone();
    values.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(
        values,
        [pair("sandbox.mode", "auto"), pair("sandbox.work_mode", "")]
    );
}

/// Schnappschuss in der kleinsten Größe: Das letzte Feld mit dem Fokus ist
/// sichtbar, samt der Zeile zu ihm, und die Tasten stehen unten.
///
/// Mutation proof: `Body::offset` returning 0 leaves the questions row below
/// the screen.
#[test]
fn the_focused_field_stays_visible_in_the_smallest_terminal() {
    let (mut screen, _far) = screen();
    press(&mut screen, KeyCode::Char('a'));
    for _ in 0..4 {
        press(&mut screen, KeyCode::Down);
    }
    assert_eq!(screen.focus, Focus::Field(3));
    let lines = shot(&mut screen, "smallest", (60, 16));
    let text = lines.join("\n");
    assert!(
        lines.iter().any(|line| line.starts_with(" Questions")),
        "{text}"
    );
    assert!(text.contains("× In this terminal:"), "{text}");
    assert!(lines[15].contains("Enter choose"), "{text}");
}

/// Scheitert die Frage nach einer Änderung, startet `Enter` nicht mit dem
/// alten Vorschlag: Der Befund steht da, `r` fragt dieselbe Teilwahl neu,
/// und erst die Antwort darauf gibt „Start" frei (#9, Review).
///
/// Mutation proof: `can_start` without `!self.failed` lets the first
/// `Enter` end the screen with the choice of the old proposal.
#[test]
fn a_failed_refresh_holds_start_until_r_asks_again() {
    let (mut screen, mut far) = screen();
    press(&mut screen, KeyCode::Char('a'));
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Right);
    press(&mut screen, KeyCode::Enter);
    let asked = proposals(&mut far);
    assert_eq!(asked.len(), 1);
    far.replies
        .send(Reply {
            row: Some(asked[0].0.clone()),
            result: Err(Diagnostic::builder(codes::DAEMON_001, Severity::Error)
                .why("the daemon went away".to_owned())
                .build()),
        })
        .expect("the screen listens");
    assert!(screen.changed());
    assert!(screen.failed());
    press(&mut screen, KeyCode::Char('a'));
    assert_eq!(screen.focus, Focus::Start);
    assert_eq!(
        press(&mut screen, KeyCode::Enter),
        Flow::Continue,
        "no start with the old proposal"
    );
    let text = shot(&mut screen, "failed", (80, 24)).join("\n");
    assert!(text.contains("r asks again"), "{text}");
    assert!(text.contains("DAEMON_001"), "{text}");

    press(&mut screen, KeyCode::Char('r'));
    let again = proposals(&mut far);
    assert_eq!(again.len(), 1, "r asks once more");
    assert_eq!(
        again[0].1.values, asked[0].1.values,
        "the same partial choice"
    );
    answer(&far, &again[0].0, proposal("claude", "disposable"));
    screen.changed();
    assert!(!screen.failed());
    assert_eq!(press(&mut screen, KeyCode::Enter), Flow::Done);
    assert_eq!(
        screen.take_exit(),
        Exit::Start(proposal("claude", "disposable").choice)
    );
}

/// Der Vorschlag eines Daemons mit dem Feld der Quelle (GitHub #15): Claude
/// Code liegt auf dem Host und im Depot, vorgewählt ist das Depot.
fn with_source_field() -> v1::StartProposal {
    let mut claude = option(
        "claude",
        "Claude Code",
        &[
            ("agent.adapter", "claude"),
            ("agents.claude.source", "depot"),
        ],
    );
    claude.source = "depot".to_owned();
    claude.sources = vec![
        v1::AgentSourceOption {
            source: "host".to_owned(),
            available: true,
            ..v1::AgentSourceOption::default()
        },
        v1::AgentSourceOption {
            source: "depot".to_owned(),
            available: true,
            selected: true,
            ..v1::AgentSourceOption::default()
        },
    ];
    let mut shown = proposal("claude", "disposable");
    shown.fields[0] = field(
        "agent",
        &["agent.adapter", "agents.claude.source"],
        "claude",
        vec![claude],
    );
    shown.fields.insert(
        1,
        field(
            "agent_source",
            &["agents.claude.source"],
            "depot",
            vec![
                option("host", "host", &[("agents.claude.source", "host")]),
                option("depot", "agent depot", &[("agents.claude.source", "depot")]),
            ],
        ),
    );
    shown
}

/// Das Feld der Quelle wählt die Quelle wie jedes Feld: `Enter` auf `host`
/// fragt mit genau dem Paar der Quelle neu. Unter dem Agenten stehen die
/// Quellen dann nicht noch einmal als Hinweis.
///
/// Mutation proof: dropping `!source_field` in `field_rows` puts "Program
/// from the agent depot" under the agent row again, and the test is red.
#[test]
fn the_source_field_chooses_the_source_and_replaces_the_notes() {
    let (link, mut far) = link::pair();
    let mut screen = FirstStartScreen::new(link, PATH.to_owned(), with_source_field(), Vec::new());
    press(&mut screen, KeyCode::Char('a'));
    let lines = shot(&mut screen, "source", (90, 30));
    let text = lines.join("\n");
    assert!(
        lines
            .iter()
            .any(|line| line == " Source         host  [agent depot]"),
        "{text}"
    );
    assert!(!text.contains("Program from the"), "{text}");
    assert!(!text.contains("Agent depot:"), "{text}");

    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Left);
    press(&mut screen, KeyCode::Enter);
    let asked = proposals(&mut far);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1.values, [pair("agents.claude.source", "host")]);
}

/// Eine Firmenvorgabe sperrt die Quelle: Das Feld steht nur lesbar da, mit
/// der Vorgabe als Herkunft, und nimmt keine Wahl.
#[test]
fn a_locked_source_is_read_only_with_the_policy() {
    let (link, mut far) = link::pair();
    let mut shown = with_source_field();
    shown.fields[1].origin = "policy".to_owned();
    shown.fields[1].lock = Some(v1::SettingLock {
        locked_by: "/etc/humanitl/policy.toml".to_owned(),
        read_only: true,
        reason: "the company policy fixes agents.claude.source to depot".to_owned(),
    });
    let mut screen = FirstStartScreen::new(link, PATH.to_owned(), shown, Vec::new());
    press(&mut screen, KeyCode::Char('a'));
    let lines = shot(&mut screen, "source-locked", (100, 30));
    let text = lines.join("\n");
    assert!(
        lines
            .iter()
            .any(|line| line == " Source        agent depot (locked)"),
        "{text}"
    );
    assert!(
        text.contains("Set by the company policy /etc/humanitl/policy.toml"),
        "{text}"
    );
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Down);
    press(&mut screen, KeyCode::Left);
    press(&mut screen, KeyCode::Enter);
    assert!(
        proposals(&mut far).is_empty(),
        "a locked field takes nothing"
    );
}
