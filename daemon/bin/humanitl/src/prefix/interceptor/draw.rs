//! Wie der Interceptor aussieht (GitHub #12).
//!
//! Oben die Liste der gehaltenen Anfragen, darunter die gewählte. Jeder Text
//! aus einer Anfrage geht durch [`display_text`], den Säuberer des Kastens
//! von `--ask terminal`; die Zahlen (Größe, Funde, Uhren) entstehen hier und
//! sind nicht zu fälschen.

use humanitl_ipc::v1;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::screen::{InterceptorScreen, RULE_DURATIONS, RULE_TARGETS, RuleStep};
use crate::ask_terminal::{ByteSize, clamp, countdown, display_text, method_name};
use crate::tui::frame::{Key, ScreenFrame, Theme};

/// Die Tasten beim Blättern.
const BROWSE_KEYS: &[Key<'static>] = &[
    Key {
        key: "↑↓",
        action: "choose",
    },
    Key {
        key: "a",
        action: "allow",
    },
    Key {
        key: "b",
        action: "block",
    },
    Key {
        key: "r",
        action: "rule",
    },
    Key {
        key: "Esc",
        action: "back to the agent",
    },
];

/// Die Tasten beim Lesen ohne Entscheidung (`--read-only`).
const READ_ONLY_KEYS: &[Key<'static>] = &[
    Key {
        key: "↑↓",
        action: "choose",
    },
    Key {
        key: "Esc",
        action: "back to the agent",
    },
];

/// Die Tasten in der Auswahl einer Regel.
const CHOICE_KEYS: &[Key<'static>] = &[
    Key {
        key: "1-4",
        action: "choose",
    },
    Key {
        key: "Esc",
        action: "cancel",
    },
];

/// Die Tasten vor der Bestätigung einer Regel.
const CONFIRM_KEYS: &[Key<'static>] = &[
    Key {
        key: "Enter",
        action: "confirm",
    },
    Key {
        key: "any other key",
        action: "drop it",
    },
];

/// Zeichnet den ganzen Bildschirm.
pub fn draw(screen: &mut InterceptorScreen, frame: &mut Frame<'_>, theme: Theme) {
    let area = frame.area();
    screen.set_frame((area.width, area.height));
    let keys = match &screen.rule {
        RuleStep::Idle if screen.read_only => READ_ONLY_KEYS,
        RuleStep::Idle => BROWSE_KEYS,
        RuleStep::Target | RuleStep::Duration(_) => CHOICE_KEYS,
        RuleStep::Confirm { .. } => CONFIRM_KEYS,
    };
    let mut context = format!("{} held", screen.rows.len());
    if screen.read_only {
        context = format!("read-only · {context}");
    }
    if !screen.live {
        // Ohne Strom ändert sich die Liste nicht mehr von selbst; das soll
        // man sehen, bevor man einer Zeile traut.
        context = format!("no live updates · {context}");
    }
    let body = ScreenFrame {
        title: "Interceptor",
        context: &context,
        keys,
        search: None,
        status: screen.status.as_deref(),
    }
    .render(frame, theme);

    let list_height = u16::try_from(screen.rows.len().saturating_add(1))
        .unwrap_or(u16::MAX)
        .clamp(2, (body.height / 2).max(2));
    let [list, detail] =
        Layout::vertical([Constraint::Length(list_height), Constraint::Min(0)]).areas(body);
    rows(screen, frame, list, theme);
    let seen = selected(screen, frame, detail, theme);
    screen.set_prompt_seen(seen);
}

/// Die Liste, ab der Zeile, die die Wahl sichtbar hält.
fn rows(screen: &InterceptorScreen, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    if screen.rows.is_empty() {
        let text = if screen.loaded {
            " No request is waiting for you."
        } else {
            " Loading the held requests from the daemon..."
        };
        frame.render_widget(Paragraph::new(text), area);
        return;
    }
    let height = usize::from(area.height.saturating_sub(1)).max(1);
    let at = screen.position().unwrap_or(0);
    let first = at.saturating_sub(height - 1);
    let mut lines = vec![Line::styled(
        format!(
            "  {:<7} {:>10} {:>10} {:>9} {:>9}  host",
            "method", "left", "waiting", "size", "findings"
        ),
        theme.hint(),
    )];
    for (index, row) in screen.rows.iter().enumerate().skip(first).take(height) {
        let chosen = Some(index) == screen.position();
        let text = row_text(screen, row, chosen);
        lines.push(if chosen {
            Line::styled(text, Style::new().add_modifier(Modifier::REVERSED))
        } else {
            Line::raw(text)
        });
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Eine Zeile der Liste.
fn row_text(screen: &InterceptorScreen, row: &v1::FlowSummary, chosen: bool) -> String {
    let method = if row.method_raw.is_empty() {
        method_name(row.method)
    } else {
        display_text(&row.method_raw)
    };
    let left = match screen.left(row) {
        Some(0) => "expired".to_owned(),
        Some(left) => countdown(u64::try_from(left).unwrap_or(0)),
        None => "-".to_owned(),
    };
    let waiting = countdown(u64::try_from(screen.waiting(row)).unwrap_or(0));
    let size = if row.request_size == 0 {
        "-".to_owned()
    } else {
        ByteSize(row.request_size).to_string()
    };
    let host = row
        .authority
        .as_ref()
        .map_or_else(String::new, |authority| {
            if authority.port == 0 || authority.port == 443 || authority.port == 80 {
                authority.host.clone()
            } else {
                format!("{}:{}", authority.host, authority.port)
            }
        });
    let target = display_text(&format!("{host}{}", row.path));
    let marker = if chosen { '>' } else { ' ' };
    format!(
        "{marker} {:<7} {left:>10} {waiting:>10} {size:>9} {:>9}  {target}",
        clamp(&method, 7),
        row.finding_count
    )
}

/// Die gewählte Anfrage: der Kopf wie im Kasten von `--ask terminal`, dann
/// der Anfang des Rumpfs, oder die Frage nach der Regel.
fn selected(screen: &InterceptorScreen, frame: &mut Frame<'_>, area: Rect, theme: Theme) -> bool {
    let block = Block::new()
        .borders(Borders::TOP)
        .title(Line::styled(" selected ", theme.key()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = usize::from(inner.width).saturating_sub(1);
    let Some(detail) = screen.detail.as_ref() else {
        let text = if screen.selected().is_some() {
            " Loading the request..."
        } else if screen.rows.is_empty() {
            ""
        } else {
            " Nothing selected: choose a request with the arrow keys."
        };
        frame.render_widget(Paragraph::new(text), inner);
        return false;
    };
    let mut lines: Vec<Line<'_>> = Vec::new();
    // Die Frage der Regel zuerst und ganz, umgebrochen statt geklemmt: Ihre
    // Tasten wirken nur, wenn jede ihrer Zeilen auf dem Schirm steht
    // (Review zu GitHub #12).
    let prompt = prompt_lines(&screen.rule, width);
    let seen = !prompt.is_empty() && prompt.len() <= usize::from(inner.height);
    if !prompt.is_empty() {
        if !seen {
            let note = wrap(super::screen::ENLARGE, width);
            frame.render_widget(
                Paragraph::new(
                    note.into_iter()
                        .map(|line| Line::styled(line, theme.key()))
                        .collect::<Vec<_>>(),
                ),
                inner,
            );
            return false;
        }
        lines.extend(
            prompt
                .into_iter()
                .map(|line| Line::styled(line, theme.key())),
        );
        lines.push(Line::raw(""));
    }
    let held = &detail.held;
    let mut field = |label: &str, text: &str| {
        if !text.is_empty() {
            lines.push(Line::raw(clamp(
                &format!(" {label}{}", display_text(text)),
                width.saturating_add(1),
            )));
        }
    };
    field("", &held.request);
    field("from: ", &held.origin);
    field("size: ", &held.size);
    field("findings: ", &held.findings);
    field("catalog: ", &held.catalog);
    if screen.rule == RuleStep::Idle {
        lines.push(Line::raw(""));
        if detail.body.is_empty() {
            lines.push(Line::styled(" (no body)", theme.hint()));
        } else {
            lines.push(Line::styled(" body:", theme.hint()));
            for line in &detail.body {
                lines.push(Line::raw(clamp(
                    &format!(" {}", display_text(line)),
                    width.saturating_add(1),
                )));
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);
    seen
}

/// Die Zeilen der Frage, die die Regel gerade stellt, umgebrochen auf
/// `width` Spalten; leer ohne Regel.
fn prompt_lines(rule: &RuleStep, width: usize) -> Vec<String> {
    let (question, options): (String, &[&str]) = match rule {
        RuleStep::Idle => return Vec::new(),
        RuleStep::Target => (
            "Rule for this request: which target?".to_owned(),
            &RULE_TARGETS,
        ),
        RuleStep::Duration(_) => (
            "Rule for this request: how long?".to_owned(),
            &RULE_DURATIONS,
        ),
        RuleStep::Confirm { sentence, .. } => (
            display_text(sentence),
            &["Enter confirms and allows, any other key drops it."],
        ),
    };
    let mut lines = wrap(&question, width);
    for option in options {
        lines.extend(wrap(option, width));
    }
    if *rule == RuleStep::Target {
        lines.extend(wrap(crate::cmd::moderate::RULE_SCOPE, width));
    }
    lines
}

/// Bricht ASCII-Text ([`display_text`]) hart auf `width` Spalten um, jede
/// Zeile mit einem Leerzeichen davor. Hart und nicht an Wörtern: So steht
/// fest, wie viele Zeilen der Text braucht, und genau so viele zeichnet er.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width)
        .map(|chunk| format!(" {}", chunk.iter().collect::<String>()))
        .collect()
}
