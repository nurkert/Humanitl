//! Wie der Bildschirm der Einstellungen aussieht (GitHub #7).
//!
//! Links die Kategorien, rechts oben die Zeilen mit Name, Herkunft und
//! Wert, rechts unten die Hilfe der markierten Zeile samt Sperre und
//! Ablehnung. Ein Bedienelement (Eingabe, Auswahl, Rückfrage) steht in einem
//! Kasten über allem.
//!
//! Texte werden nach Anzeigebreite umbrochen und gekürzt, nicht nach Bytes:
//! ratatui misst mit `unicode-width`, und [`fit`] tut es für die Spalten
//! ebenso.

use humanitl_ipc::v1;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::edit::{Input, Overlay, Picker};
use super::model;
use super::panels::{self, Panel};
use super::{Focus, Note, SettingsScreen};
use super::{actions, display};
use crate::tui::frame::{Key, ScreenFrame, Theme};

/// Die Tasten der Liste.
const BROWSE_KEYS: &[Key<'static>] = &[
    Key {
        key: "↑↓",
        action: "move",
    },
    Key {
        key: "←→",
        action: "pane",
    },
    Key {
        key: "Enter",
        action: "change",
    },
    Key {
        key: "/",
        action: "search",
    },
    Key {
        key: "q",
        action: "quit",
    },
];

/// Die Tasten eines Felds.
const INPUT_KEYS: &[Key<'static>] = &[
    Key {
        key: "Enter",
        action: "save",
    },
    Key {
        key: "Esc",
        action: "cancel",
    },
];

/// Die Tasten einer Auswahl.
const PICKER_KEYS: &[Key<'static>] = &[
    Key {
        key: "↑↓",
        action: "move",
    },
    Key {
        key: "Enter",
        action: "choose",
    },
    Key {
        key: "Esc",
        action: "cancel",
    },
];

/// Die Tasten einer Rückfrage.
const CONFIRM_KEYS: &[Key<'static>] = &[
    Key {
        key: "y",
        action: "yes",
    },
    Key {
        key: "n",
        action: "no",
    },
];

/// Die breiteste linke Spalte.
const MAX_CATEGORY_WIDTH: u16 = 22;

/// Die breiteste Spalte der Namen.
const MAX_TITLE_WIDTH: usize = 26;

/// Die Breite der Spalte der Herkunft.
const ORIGIN_WIDTH: usize = 15;

/// Zeichnet den ganzen Bildschirm.
pub fn draw(screen: &SettingsScreen, frame: &mut Frame<'_>, theme: Theme) {
    let panel_keys: Vec<Key<'_>>;
    let keys: &[Key<'_>] = match (&screen.overlay, screen.panel) {
        (Some(Overlay::Input(_)), _) => INPUT_KEYS,
        (Some(Overlay::Picker(_)), _) => PICKER_KEYS,
        (Some(Overlay::Confirm(_)), _) => CONFIRM_KEYS,
        (None, Some(panel)) => {
            panel_keys = panel
                .keys()
                .iter()
                .map(|(key, action)| Key { key, action })
                .collect();
            &panel_keys
        }
        (None, None) => BROWSE_KEYS,
    };
    let mut context = screen.project.as_ref().map_or_else(
        || "global".to_owned(),
        |scope| format!("project {}", model::clean(&scope.path)),
    );
    if screen.live_ended {
        // Ohne Strom sieht der Bildschirm Änderungen anderer Clients nicht
        // mehr von selbst; das soll man sehen.
        context = format!("no live updates, r reloads · {context}");
    }
    let body = ScreenFrame {
        title: "Settings",
        context: &context,
        keys,
        search: Some(&screen.search),
    }
    .render(frame, theme);

    let Some(catalog) = screen.catalog.as_ref() else {
        loading(screen, frame, body, theme);
        return;
    };
    let width = category_width(catalog);
    let [left, right] =
        Layout::horizontal([Constraint::Length(width), Constraint::Min(0)]).areas(body);
    categories(screen, frame, left, theme);
    let detail_height = (right.height / 2).clamp(4, 10);
    let [list, detail_area] =
        Layout::vertical([Constraint::Min(2), Constraint::Length(detail_height)]).areas(right);
    match screen.panel {
        Some(panel) => panel_list(screen, panel, frame, list, theme),
        None => rows(screen, frame, list, theme),
    }
    detail(screen, frame, detail_area, theme);
    if let Some(overlay) = screen.overlay.as_ref() {
        draw_overlay(overlay, frame, body, theme);
    }
}

/// Ohne Katalog: Laden, oder warum es nicht ging.
fn loading(screen: &SettingsScreen, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let lines = match screen.load_error.as_ref() {
        Some(diagnostic) => refusal_lines(diagnostic, theme),
        None => vec![Line::raw(" Loading the settings from the daemon…")],
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

/// Die Breite der linken Spalte: der längste Name, höchstens
/// [`MAX_CATEGORY_WIDTH`].
fn category_width(catalog: &v1::SettingsCatalog) -> u16 {
    let widest = model::categories(catalog)
        .iter()
        .map(|category| Span::raw(model::clean(&category.title)).width())
        .max()
        .unwrap_or(0);
    u16::try_from(widest + 4)
        .unwrap_or(MAX_CATEGORY_WIDTH)
        .min(MAX_CATEGORY_WIDTH)
}

/// Die linke Spalte.
fn categories(screen: &SettingsScreen, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let searching = !screen.search.query().is_empty();
    let block = Block::new().borders(Borders::RIGHT);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let names = screen.categories();
    let offset = scroll(screen.category, names.len(), inner.height);
    let lines: Vec<Line<'_>> = names
        .iter()
        .enumerate()
        .skip(offset)
        .map(|(index, category)| {
            let chosen = index == screen.category && !searching;
            let text = format!(" {}", model::clean(&category.title));
            let style = if chosen && screen.focus == Focus::Categories && screen.panel.is_none() {
                Style::new().add_modifier(Modifier::REVERSED)
            } else if chosen {
                theme.key()
            } else if searching {
                theme.hint()
            } else {
                Style::new()
            };
            Line::styled(text, style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Wie weit eine Liste verschoben ist, damit `index` sichtbar bleibt.
fn scroll(index: usize, len: usize, height: u16) -> usize {
    let height = usize::from(height).max(1);
    if len <= height {
        0
    } else {
        index.saturating_sub(height - 1).min(len - height)
    }
}

/// Kürzt oder füllt `text` auf genau `width` Spalten Anzeigebreite.
#[must_use]
pub fn fit(text: &str, width: usize) -> String {
    let total = Span::raw(text).width();
    if total <= width {
        return format!("{text}{}", " ".repeat(width - total));
    }
    // Zu lang: so viel, wie mit der Auslassung hineinpasst, nie ein halbes
    // breites Zeichen.
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = Span::raw(ch.to_string()).width();
        if used + w + 1 > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    if width > 0 {
        out.push('…');
        used += 1;
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

/// Die Zeilen rechts: Name, Herkunft, Wert.
fn rows(screen: &SettingsScreen, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let entries = screen.rows();
    if entries.is_empty() {
        let text = if screen.search.query().is_empty() {
            " Nothing to show."
        } else {
            " No setting matches the search."
        };
        frame.render_widget(Paragraph::new(Line::styled(text, theme.hint())), area);
        return;
    }
    let title_width = entries
        .iter()
        .map(|entry| Span::raw(model::clean(&entry.title)).width())
        .max()
        .unwrap_or(0)
        .min(MAX_TITLE_WIDTH);
    let offset = scroll(screen.row, entries.len(), area.height);
    let lines: Vec<Line<'_>> = entries
        .iter()
        .enumerate()
        .skip(offset)
        .map(|(index, entry)| {
            let selected = index == screen.row;
            row_line(screen, entry, selected, title_width, theme)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// Eine Zeile: Markierung, Name, Herkunft (oder `locked`), Wert.
fn row_line<'a>(
    screen: &SettingsScreen,
    entry: &v1::SettingsEntry,
    selected: bool,
    title_width: usize,
    theme: Theme,
) -> Line<'a> {
    let marker = if selected { "›" } else { " " };
    let origin = if entry.lock.is_some() {
        "policy, locked".to_owned()
    } else {
        model::clean(&entry.origin)
    };
    let mut spans = vec![
        Span::raw(format!(
            "{marker} {} ",
            fit(&model::clean(&entry.title), title_width)
        )),
        Span::styled(format!("{} ", fit(&origin, ORIGIN_WIDTH)), theme.hint()),
        Span::raw(display::shown_value(screen, entry)),
    ];
    if screen
        .notes
        .get(&entry.id)
        .is_some_and(|note| matches!(note, Note::Refused(_)))
    {
        spans.push(Span::styled("  refused", theme.key()));
    }
    let line = Line::from(spans);
    if selected && screen.focus == Focus::Rows {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}

/// Die Liste einer offenen Seite.
fn panel_list(
    screen: &SettingsScreen,
    panel: Panel,
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
) {
    let title = screen
        .catalog
        .as_ref()
        .and_then(|catalog| catalog.entries.iter().find(|entry| entry.id == panel.row()))
        .map(|entry| model::clean(&entry.title))
        .unwrap_or_default();
    let [head, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    frame.render_widget(
        Paragraph::new(Line::styled(format!(" {title}"), theme.key())),
        head,
    );
    let lines = panels::lines(screen, panel);
    if lines.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(" Nothing here yet.", theme.hint())),
            list,
        );
        return;
    }
    let first = lines
        .iter()
        .map(|line| Span::raw(line[0].clone()).width())
        .max()
        .unwrap_or(0)
        .min(MAX_TITLE_WIDTH);
    let cursor = panel.cursor().min(lines.len() - 1);
    let offset = scroll(cursor, lines.len(), list.height);
    let shown: Vec<Line<'_>> = lines
        .iter()
        .enumerate()
        .skip(offset)
        .map(|(index, [name, value, extra])| {
            let marker = if index == cursor { "›" } else { " " };
            let line = Line::from(vec![
                Span::raw(format!("{marker} {} ", fit(name, first))),
                Span::raw(format!("{value}  ")),
                Span::styled(extra.clone(), theme.hint()),
            ]);
            if index == cursor {
                line.style(Style::new().add_modifier(Modifier::REVERSED))
            } else {
                line
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(shown), list);
}

/// Unten rechts: Hilfe, Sperre, Ablehnung oder Auskunft der markierten
/// Zeile, nach Anzeigebreite umbrochen.
fn detail(screen: &SettingsScreen, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let block = Block::new().borders(Borders::TOP);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let id = screen
        .panel
        .map(|panel| panel.row().to_owned())
        .or_else(|| screen.selected().map(|entry| entry.id.clone()));
    let Some(id) = id else {
        return;
    };
    let entry = screen
        .catalog
        .as_ref()
        .and_then(|catalog| catalog.entries.iter().find(|entry| entry.id == id));
    let mut lines = Vec::new();
    if let Some(entry) = entry {
        lines.extend(entry_lines(screen, entry, theme));
    }
    match screen.notes.get(&id) {
        Some(Note::Refused(diagnostic)) => lines.extend(refusal_lines(diagnostic, theme)),
        Some(Note::Info(text)) => lines.push(Line::styled(model::clean(text), theme.key())),
        Some(Note::Daemon(text, diagnostics)) => {
            if !text.is_empty() {
                lines.push(Line::styled(model::clean(text), theme.key()));
            }
            for diagnostic in diagnostics {
                lines.extend(found_lines(diagnostic, theme));
            }
        }
        None => {}
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

/// Hilfe, Erklärung, „nächste Sitzung" und Sperre eines Eintrags.
fn entry_lines<'a>(
    screen: &SettingsScreen,
    entry: &v1::SettingsEntry,
    theme: Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let help = if entry.details.is_empty() {
        &entry.help
    } else {
        &entry.details
    };
    if !help.is_empty() {
        lines.push(Line::raw(model::clean(help)));
    }
    if entry.next_session {
        lines.push(Line::styled(
            "Applies from the next session on.",
            theme.hint(),
        ));
    }
    if let Some(reason) = actions::read_only(screen, entry) {
        lines.push(Line::styled(model::clean(&reason), theme.key()));
    }
    lines
}

/// Eine Ablehnung des Daemons: Code, `why`, `fix`.
fn refusal_lines<'a>(diagnostic: &humanitl_core::Diagnostic, theme: Theme) -> Vec<Line<'a>> {
    let mut lines = vec![Line::styled(
        format!("{}: {}", diagnostic.code, model::clean(&diagnostic.title)),
        theme.key(),
    )];
    lines.push(Line::raw(format!("why: {}", model::clean(&diagnostic.why))));
    if let Some(fix) = diagnostic.fix.as_ref() {
        lines.push(Line::raw(format!("fix: {}", model::clean(&fix_text(fix)))));
    }
    lines
}

/// Ein Befund in einer Antwort (Wire-Form): Code, `why`, `fix`, wie eine
/// Ablehnung.
fn found_lines<'a>(diagnostic: &v1::Diagnostic, theme: Theme) -> Vec<Line<'a>> {
    let mut lines = vec![Line::styled(model::clean(&diagnostic.code), theme.key())];
    lines.push(Line::raw(format!("why: {}", model::clean(&diagnostic.why))));
    if let Some(fix) = diagnostic.fix.as_ref().and_then(proto_fix_text) {
        lines.push(Line::raw(format!("fix: {}", model::clean(&fix))));
    }
    lines
}

/// Der Vorschlag eines Befunds in Wire-Form, wie [`fix_text`] ihn zeigt.
fn proto_fix_text(fix: &v1::FixAction) -> Option<String> {
    use v1::fix_action::Action;

    Some(match fix.action.as_ref()? {
        Action::CopyCommand(command) | Action::OpenUrl(command) => command.clone(),
        Action::SetEnv(set) => format!("export {}={}", set.key, set.value),
        Action::ChangeSetting(change) if change.value.is_empty() => {
            format!("change {}", change.key)
        }
        Action::ChangeSetting(change) => format!("set {} to {}", change.key, change.value),
        Action::EditSetting(edit) => format!("change {}", edit.key),
        Action::InstallService(()) => "humanitl daemon install".to_owned(),
        Action::RemountReadOnly(path) => format!("mount {path} read-only"),
        Action::AddRule(_) => return None,
    })
}

/// Der Vorschlag eines Befunds, wie er unter einer Zeile steht: ein Befehl
/// zum Abtippen, oder was in dieser Ansicht zu ändern ist.
fn fix_text(fix: &humanitl_core::FixAction) -> String {
    use humanitl_core::FixAction;

    match fix {
        FixAction::CopyCommand(command) | FixAction::OpenUrl(command) => command.clone(),
        FixAction::SetEnv { key, value } => format!("export {key}={value}"),
        FixAction::ChangeSetting { key, value } => format!("set {key} to {value}"),
        FixAction::EditSetting { key } => format!("change {key}"),
        FixAction::InstallService => "humanitl daemon install".to_owned(),
        FixAction::AddRule(rule) => format!("add the rule {}", rule.id),
        FixAction::RemountReadOnly(path) => format!("mount {} read-only", path.display()),
    }
}

/// Ein Kasten in der Mitte von `area`.
fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2)).max(1);
    let height = height.min(area.height).max(1);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

/// Das offene Bedienelement.
fn draw_overlay(overlay: &Overlay, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    match overlay {
        Overlay::Input(input) => draw_input(input, frame, area, theme),
        Overlay::Picker(picker) => draw_picker(picker, frame, area, theme),
        Overlay::Confirm(confirm) => {
            let width = 60;
            let text = model::clean(&confirm.question);
            let rows = u16::try_from(Span::raw(text.clone()).width() / 56 + 4).unwrap_or(6);
            let rect = centred(area, width, rows);
            frame.render_widget(Clear, rect);
            let block = Block::bordered().title(" Confirm ");
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let lines = vec![Line::raw(text), Line::styled("y yes · n no", theme.hint())];
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        }
    }
}

/// Ein Feld; ein verdecktes zeigt nichts von dem, was getippt ist.
fn draw_input(input: &Input, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let rect = centred(area, 64, 4);
    frame.render_widget(Clear, rect);
    let block = Block::bordered().title(format!(" {} ", model::clean(&input.label)));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let shown = model::clean(input.shown());
    let hint = if input.secret {
        "typed without echo, sent once"
    } else {
        "Ctrl+U clears"
    };
    let lines = vec![
        Line::raw(format!("> {shown}")),
        Line::styled(hint, theme.hint()),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
    let typed = u16::try_from(Span::raw(shown).width()).unwrap_or(u16::MAX);
    let x = inner.x.saturating_add(2).saturating_add(typed);
    frame.set_cursor_position(Position::new(
        x.min(inner.right().saturating_sub(1)),
        inner.y,
    ));
}

/// Eine Auswahl mit den Gründen der nicht wählbaren Möglichkeiten.
fn draw_picker(picker: &Picker, frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let height = u16::try_from(picker.options.len() + 2).unwrap_or(u16::MAX);
    let rect = centred(area, 64, height);
    frame.render_widget(Clear, rect);
    let block = Block::bordered().title(format!(" {} ", model::clean(&picker.label)));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let offset = scroll(picker.cursor, picker.options.len(), inner.height);
    let lines: Vec<Line<'_>> = picker
        .options
        .iter()
        .enumerate()
        .skip(offset)
        .map(|(index, option)| {
            let marker = if index == picker.cursor { "›" } else { " " };
            let mut spans = vec![Span::raw(format!("{marker} {}", option.title))];
            if !option.enabled {
                spans.push(Span::styled("  not available", theme.hint()));
            }
            if !option.reason.is_empty() {
                spans.push(Span::styled(format!("  {}", option.reason), theme.hint()));
            }
            let line = Line::from(spans);
            if index == picker.cursor {
                line.style(Style::new().add_modifier(Modifier::REVERSED))
            } else {
                line
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use ratatui::text::Span;

    use super::fit;

    /// Gekürzt und aufgefüllt wird nach Anzeigebreite: Ein Zeichen, das
    /// zwei Spalten braucht, zählt zwei.
    #[test]
    fn fit_counts_display_width_not_bytes() {
        assert_eq!(fit("Grüße", 7), "Grüße  ");
        assert_eq!(Span::raw(fit("日本語の設定", 7)).width(), 7);
        assert_eq!(fit("abcdef", 4), "abc…");
    }
}
