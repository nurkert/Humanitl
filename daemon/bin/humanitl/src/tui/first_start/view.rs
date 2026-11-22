//! Wie der Erststart-Bildschirm aussieht (GitHub #9).
//!
//! Zugeklappt: die Zeile der Vorgaben, der Hinweis auf Cloud-Inferenz, die
//! Knöpfe „Start" und „Customize…" und die Befunde des Daemons. Aufgeklappt
//! darunter je Feld eine Zeile mit Namen und Optionen und darunter die
//! Zeilen zum Feld: Quelle des Agenten, Sperre, nicht verfügbare Optionen
//! mit Grund. Jedes Wort, das ein Wert ist (Optionen, Vorgaben, Gründe),
//! kommt vom Daemon; die Namen der Felder und Knöpfe sind die des
//! Bildschirms.
//!
//! Ohne Farbe bleibt alles unterscheidbar: Die gewählte Option steht in
//! eckigen Klammern, eine nicht verfügbare trägt ein `×`, Fokus und
//! Markierung sind umgekehrt.

use humanitl_ipc::v1;
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{FirstStartScreen, Focus, chosen};
use crate::tui::frame::{Key, ScreenFrame, Theme};
use crate::tui::settings::model::clean;
use crate::tui::settings::view::{fix_text, proto_fix_text};

/// Breite der Spalte mit den Namen der Felder.
const LABEL: usize = 14;

/// Der linke Rand des Inhalts.
const MARGIN: usize = 1;

/// Die Tasten, solange der Fokus auf „Start" steht.
const START_KEYS: &[Key<'static>] = &[
    Key {
        key: "Enter",
        action: "start",
    },
    Key {
        key: "a",
        action: "customize",
    },
    Key {
        key: "Esc",
        action: "cancel",
    },
];

/// Die Tasten in einem aufgeklappten Feld.
const FIELD_KEYS: &[Key<'static>] = &[
    Key {
        key: "↑↓",
        action: "field",
    },
    Key {
        key: "←→",
        action: "option",
    },
    Key {
        key: "Enter",
        action: "choose",
    },
    Key {
        key: "a",
        action: "collapse",
    },
    Key {
        key: "Esc",
        action: "cancel",
    },
];

/// Der Name eines Felds; eine Kennung, die dieser Build nicht kennt, behält
/// die des Daemons.
#[must_use]
pub fn field_label(id: &str) -> &str {
    match id {
        "agent" => "Agent",
        "folder_mode" => "Folder",
        "isolation" => "Isolation",
        "ask_mode" => "Questions",
        "llm_server" => "Model server",
        "profile" => "Profile",
        other => other,
    }
}

/// Woher das Programm eines Agenten kommt, wie der Daemon die Quelle nennt.
fn source_label(source: &str) -> &'static str {
    match source {
        "depot" => "Program from the agent depot",
        _ => "Program from the host",
    }
}

/// Der kurze Name einer Quelle.
fn source_name(source: &str) -> &'static str {
    match source {
        "depot" => "Agent depot",
        _ => "Host",
    }
}

/// Die Zeile der Vorgaben ohne ihr Leitwort `Defaults: `; der Bildschirm
/// sagt das selbst.
#[must_use]
pub fn defaults_text(proposal: &v1::StartProposal) -> String {
    let line = clean(&proposal.defaults_line);
    line.strip_prefix("Defaults: ")
        .map_or_else(|| line.clone(), str::to_owned)
}

/// Der Grund aus den Befunden einer Option: das `why` des ersten.
fn reason(reasons: &[v1::Diagnostic]) -> String {
    reasons
        .first()
        .map(|found| clean(&found.why))
        .unwrap_or_default()
}

/// Der Inhalt, Zeile für Zeile, nach Anzeigebreite umbrochen.
struct Body {
    /// Die Zeilen.
    lines: Vec<Line<'static>>,
    /// Die Breite, in die umbrochen wird.
    width: usize,
    /// Die Zeile, die sichtbar bleiben muss: der Fokus.
    focus: usize,
}

impl Body {
    /// Ein leerer Inhalt für `width` Zellen.
    const fn new(width: usize) -> Self {
        Self {
            lines: Vec::new(),
            width,
            focus: 0,
        }
    }

    /// Eine leere Zeile.
    fn blank(&mut self) {
        self.lines.push(Line::raw(""));
    }

    /// Merkt die nächste Zeile als die, die sichtbar bleiben muss.
    const fn mark_focus(&mut self) {
        self.focus = self.lines.len();
    }

    /// Ein Text, nach Wörtern umbrochen, jede Zeile mit `indent` Zellen
    /// Einzug.
    fn text(&mut self, text: &str, style: Style, indent: usize) {
        let words = text
            .split_whitespace()
            .map(|word| Span::styled(word.to_owned(), style))
            .collect();
        self.spans(words, indent, " ");
    }

    /// Spannen nebeneinander, getrennt durch `gap`, umbrochen vor einer
    /// Spanne, die nicht mehr passt.
    fn spans(&mut self, spans: Vec<Span<'static>>, indent: usize, gap: &'static str) {
        let room = self.width.saturating_sub(indent).max(1);
        let mut row: Vec<Span<'static>> = Vec::new();
        let mut used = 0;
        for span in spans {
            let width = span.width();
            let extra = if row.is_empty() { 0 } else { gap.len() };
            if !row.is_empty() && used + extra + width > room {
                self.push_row(std::mem::take(&mut row), indent);
                used = 0;
            }
            if !row.is_empty() {
                row.push(Span::raw(gap));
                used += gap.len();
            }
            used += width;
            row.push(span);
        }
        if !row.is_empty() {
            self.push_row(row, indent);
        }
    }

    /// Eine Zeile mit Einzug.
    fn push_row(&mut self, mut row: Vec<Span<'static>>, indent: usize) {
        row.insert(0, Span::raw(" ".repeat(indent)));
        self.lines.push(Line::from(row));
    }

    /// Wie weit nach unten geschoben wird, damit der Fokus in `height`
    /// Zeilen sichtbar ist.
    fn offset(&self, height: usize) -> usize {
        if height == 0 || self.lines.len() <= height {
            return 0;
        }
        // Zwei Zeilen unter dem Fokus bleiben sichtbar, damit die Zeilen
        // zum Feld (Grund, Sperre) mit ihm erscheinen.
        let wanted = (self.focus + 3).min(self.lines.len());
        wanted.saturating_sub(height).min(self.lines.len() - height)
    }
}

/// Zeichnet den Bildschirm.
pub fn draw(screen: &FirstStartScreen, frame: &mut Frame<'_>, theme: Theme) {
    let keys = match screen.focus {
        Focus::Start => START_KEYS,
        Focus::Field(_) => FIELD_KEYS,
    };
    let name = clean(&screen.proposal.display_name);
    let area = ScreenFrame {
        title: "First start",
        context: &name,
        keys,
        search: None,
        status: None,
    }
    .render(frame, theme);
    let mut body = Body::new(usize::from(area.width).saturating_sub(MARGIN + 1));
    summary(screen, &mut body, theme);
    if screen.expanded {
        body.blank();
        body.text("START CHOICE", theme.hint(), MARGIN);
        for (index, field) in screen.proposal.fields.iter().enumerate() {
            field_rows(screen, index, field, &mut body, theme);
        }
    }
    let offset = body.offset(usize::from(area.height));
    let shown: Vec<Line<'static>> = body.lines.into_iter().skip(offset).collect();
    frame.render_widget(Paragraph::new(shown), area);
}

/// Der zugeklappte Teil: Ordner, Vorgaben, Knöpfe, Befunde.
fn summary(screen: &FirstStartScreen, body: &mut Body, theme: Theme) {
    let proposal = &screen.proposal;
    body.blank();
    body.text(
        "First start in this folder. Everything is preset; Enter starts with these defaults.",
        Style::new(),
        MARGIN,
    );
    body.text(&clean(&proposal.path), theme.hint(), MARGIN);
    body.blank();
    body.text("DEFAULTS", theme.hint(), MARGIN);
    body.text(&defaults_text(proposal), theme.key(), MARGIN);
    if let Some(cloud) = proposal.cloud_inference.as_ref()
        && !cloud.provider.is_empty()
    {
        body.text(
            &humanitl_config::inference::short_line(&clean(&cloud.provider)),
            theme.key(),
            MARGIN,
        );
    }
    body.blank();
    if screen.focus == Focus::Start {
        body.mark_focus();
    }
    body.spans(buttons(screen, theme), MARGIN, "  ");
    if let Some(status) = status(screen) {
        body.text(status, theme.hint(), MARGIN);
    }
    let findings = proposal.diagnostics.iter().map(|found| {
        (
            clean(&found.code),
            clean(&found.why),
            found.fix.as_ref().and_then(proto_fix_text),
        )
    });
    let refusals = screen.refusals.iter().map(|found| {
        (
            found.code.as_str().to_owned(),
            clean(&found.why),
            found.fix.as_ref().map(fix_text),
        )
    });
    for (code, why, fix) in findings.chain(refusals) {
        body.blank();
        body.text(&code, theme.key(), MARGIN);
        body.text(&format!("why: {why}"), Style::new(), MARGIN + 2);
        if let Some(fix) = fix {
            body.text(&format!("fix: {}", clean(&fix)), Style::new(), MARGIN + 2);
        }
    }
}

/// Die Knöpfe „Start" und „Customize…".
fn buttons(screen: &FirstStartScreen, theme: Theme) -> Vec<Span<'static>> {
    let focused = Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD);
    let start = if screen.focus == Focus::Start {
        focused
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    };
    let start = if screen.can_start() {
        start
    } else {
        start.patch(theme.hint())
    };
    let arrow = if screen.expanded { "▾" } else { "▸" };
    vec![
        Span::styled("[ Start ]", start),
        Span::raw(format!("[ Customize… {arrow} ]")),
    ]
}

/// Warum „Start" gerade wartet, wenn es wartet.
fn status(screen: &FirstStartScreen) -> Option<&'static str> {
    if screen.asking {
        return Some("Asking the daemon for the new proposal…");
    }
    if screen.failed {
        return Some("Start waits: the daemon did not answer the last change; r asks again.");
    }
    if screen.can_start() {
        return None;
    }
    Some("Start waits: the daemon refuses a start with these values, see below.")
}

/// Ein Feld: Name und Optionen, darunter Quelle, Sperre und Gründe.
fn field_rows(
    screen: &FirstStartScreen,
    index: usize,
    field: &v1::ProposalField,
    body: &mut Body,
    theme: Theme,
) {
    let focused = screen.focus == Focus::Field(index);
    if focused {
        body.mark_focus();
    }
    let label = format!("{:<width$}", field_label(&field.id), width = LABEL - 1);
    let label = if focused {
        Span::styled(label, Style::new().add_modifier(Modifier::REVERSED))
    } else {
        Span::styled(label, Style::new().add_modifier(Modifier::BOLD))
    };
    let mut spans = vec![label];
    if let Some(lock) = field.lock.as_ref() {
        let value = chosen(field).map_or_else(|| clean(&field.value), |o| clean(&o.label));
        spans.push(Span::raw(value));
        spans.push(Span::styled("(locked)", theme.hint()));
        body.spans(spans, MARGIN, " ");
        let mut text = format!("Set by the company policy {}", clean(&lock.locked_by));
        if !lock.reason.is_empty() {
            text = format!("{text}. {}", clean(&lock.reason));
        }
        body.text(&text, theme.hint(), MARGIN + LABEL);
        return;
    }
    if field.options.is_empty() {
        spans.push(Span::styled("None", theme.hint()));
    }
    for (position, option) in field.options.iter().enumerate() {
        spans.push(option_span(
            field,
            option,
            focused && position == screen.cursor,
            theme,
        ));
    }
    body.spans(spans, MARGIN, " ");
    if field.id == "agent"
        && let Some(option) = chosen(field)
    {
        for (text, warn) in source_notes(option) {
            body.text(
                &text,
                if warn { theme.key() } else { theme.hint() },
                MARGIN + LABEL,
            );
        }
    }
    for option in field.options.iter().filter(|option| !option.available) {
        body.text(
            &format!("× {}: {}", clean(&option.label), reason(&option.reasons)),
            theme.key(),
            MARGIN + LABEL,
        );
    }
}

/// Eine Option: gewählt in Klammern, nicht verfügbar mit `×`, markiert
/// umgekehrt.
fn option_span(
    field: &v1::ProposalField,
    option: &v1::ProposalOption,
    marked: bool,
    theme: Theme,
) -> Span<'static> {
    let label = clean(&option.label);
    let text = if option.value == field.value {
        format!("[{label}]")
    } else if option.available {
        format!(" {label} ")
    } else {
        format!("×{label} ")
    };
    let mut style = if option.available {
        Style::new()
    } else {
        theme.hint()
    };
    if option.value == field.value {
        style = style.add_modifier(Modifier::BOLD);
    }
    if marked {
        style = style.add_modifier(Modifier::REVERSED);
    }
    Span::styled(text, style)
}

/// Die Quellen des gewählten Agenten, die wirksame zuerst; `true` für eine
/// Zeile, die warnt.
fn source_notes(option: &v1::ProposalOption) -> Vec<(String, bool)> {
    if option.sources.is_empty() {
        return if option.source.is_empty() {
            Vec::new()
        } else {
            vec![(source_label(&option.source).to_owned(), false)]
        };
    }
    let ordered = option
        .sources
        .iter()
        .filter(|source| source.selected)
        .chain(option.sources.iter().filter(|source| !source.selected));
    ordered
        .map(|source| match (source.selected, source.available) {
            (true, true) => (source_label(&source.source).to_owned(), false),
            (true, false) => (
                format!(
                    "{}: {}",
                    source_label(&source.source),
                    reason(&source.reasons)
                ),
                true,
            ),
            (false, true) => (format!("{}: ready", source_name(&source.source)), false),
            (false, false) => (
                format!(
                    "{}: {}",
                    source_name(&source.source),
                    reason(&source.reasons)
                ),
                true,
            ),
        })
        .collect()
}
