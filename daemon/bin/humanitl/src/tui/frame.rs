//! Der gemeinsame Rahmen aller Bildschirme: Kopfzeile, Fußzeile mit den
//! Tasten und die Suche, dazu die Farben nach `NO_COLOR` (GitHub #6).
//!
//! Einstellungen, Erststart, Interceptor und die Hilfe der Präfixtaste
//! (ADR-0024 Punkt 6) zeichnen ihren Inhalt in die Fläche, die
//! [`ScreenFrame::render`] übrig lässt. Wie oben und unten aussieht, steht
//! damit an einer Stelle.

use std::ffi::OsStr;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::size::Cells;

/// Die Stile der Bildschirme, mit oder ohne Farbe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Farbe erlaubt (kein `NO_COLOR`).
    colour: bool,
}

impl Theme {
    /// Nach `NO_COLOR` (no-color.org): gesetzt und nicht leer heißt ohne
    /// Farbe. Hervorhebung bleibt dann über Fett und Umkehrung erhalten.
    #[must_use]
    pub fn from_no_color(no_color: Option<&OsStr>) -> Self {
        Self {
            colour: no_color.is_none_or(OsStr::is_empty),
        }
    }

    /// Nach der Umgebung dieses Prozesses.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_no_color(std::env::var_os("NO_COLOR").as_deref())
    }

    /// Ob Farben benutzt werden.
    #[must_use]
    pub const fn colour(self) -> bool {
        self.colour
    }

    /// Die Kopfzeile: gelb wie das Logo, ohne Farbe umgekehrt.
    #[must_use]
    pub fn header(self) -> Style {
        let style = Style::new().add_modifier(Modifier::BOLD);
        if self.colour {
            style.fg(Color::Black).bg(Color::Yellow)
        } else {
            style.add_modifier(Modifier::REVERSED)
        }
    }

    /// Eine Taste in der Fußzeile.
    #[must_use]
    pub fn key(self) -> Style {
        let style = Style::new().add_modifier(Modifier::BOLD);
        if self.colour {
            style.fg(Color::Yellow)
        } else {
            style
        }
    }

    /// Erklärender Text, zurückgenommen.
    #[must_use]
    pub fn hint(self) -> Style {
        if self.colour {
            Style::new().fg(Color::DarkGray)
        } else {
            Style::new()
        }
    }
}

/// Eine Taste und was sie tut, für die Fußzeile.
#[derive(Clone, Copy, Debug)]
pub struct Key<'a> {
    /// Wie die Taste heißt, zum Beispiel `q` oder `Enter`.
    pub key: &'a str,
    /// Was sie tut, zum Beispiel `quit`.
    pub action: &'a str,
}

/// Die Suche eines Bildschirms: offen oder zu, und was getippt ist.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    /// Wahr, solange getippt wird.
    open: bool,
    /// Der Suchtext.
    query: String,
}

impl Search {
    /// Ob gerade getippt wird.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Der Suchtext.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Ob `text` zur Suche passt: ohne Suchtext alles, sonst jedes Wort des
    /// Suchtexts irgendwo in `text`, ohne Rücksicht auf Groß und Klein.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        let text = text.to_lowercase();
        self.query
            .to_lowercase()
            .split_whitespace()
            .all(|word| text.contains(word))
    }

    /// Nimmt eine Taste, wenn sie zur Suche gehört; `true`, wenn ja.
    ///
    /// `/` öffnet, `Esc` schließt und leert, `Enter` schließt und behält den
    /// Text, `Backspace` löscht ein Zeichen. Zu ist die Suche nur für `/`
    /// zuständig.
    pub fn handle(&mut self, key: &KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }
        if !self.open {
            if key.code == KeyCode::Char('/')
                && key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
            {
                self.open = true;
                return true;
            }
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.open = false;
                self.query.clear();
            }
            KeyCode::Enter => self.open = false,
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.query.push(c);
            }
            _ => return false,
        }
        true
    }
}

/// Kopf, Fuß und Suche um den Inhalt eines Bildschirms.
#[derive(Clone, Copy, Debug)]
pub struct ScreenFrame<'a> {
    /// Der Name des Bildschirms, zum Beispiel `Settings`.
    pub title: &'a str,
    /// Rechts in der Kopfzeile, zum Beispiel das Projekt; leer für nichts.
    pub context: &'a str,
    /// Die Tasten in der Fußzeile, in dieser Reihenfolge.
    pub keys: &'a [Key<'a>],
    /// Die Suche, wenn der Bildschirm eine hat.
    pub search: Option<&'a Search>,
    /// Rechts in der Fußzeile, hervorgehoben, oder nichts: im Terminal einer
    /// Sitzung der Stand des Agenten und der letzte Hinweis des Daemons,
    /// etwa eine gehaltene Anfrage (GitHub #11).
    pub status: Option<&'a str>,
}

impl ScreenFrame<'_> {
    /// Zeichnet Kopf, Fuß und, wenn offen oder gefüllt, die Suchzeile; gibt
    /// die Fläche dazwischen zurück.
    pub fn render(&self, frame: &mut Frame<'_>, theme: Theme) -> Rect {
        let search_rows = u16::from(
            self.search
                .is_some_and(|search| search.is_open() || !search.query().is_empty()),
        );
        let [head, body, search_area, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(search_rows),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        frame.render_widget(
            Paragraph::new(format!(" humanitl · {}", self.title)).style(theme.header()),
            head,
        );
        if !self.context.is_empty() {
            frame.render_widget(
                Paragraph::new(format!("{} ", self.context))
                    .alignment(Alignment::Right)
                    .style(theme.header()),
                head,
            );
        }

        if let Some(search) = self.search.filter(|_| search_rows > 0) {
            let line = Line::from(vec![
                Span::styled("/", theme.key()),
                Span::raw(search.query()),
            ]);
            frame.render_widget(Paragraph::new(line), search_area);
            if search.is_open() {
                let typed = u16::try_from(search.query().chars().count()).unwrap_or(u16::MAX);
                let x = search_area.x.saturating_add(1).saturating_add(typed);
                frame.set_cursor_position(Position::new(
                    x.min(search_area.right().saturating_sub(1)),
                    search_area.y,
                ));
            }
        }

        let mut spans = vec![Span::raw(" ")];
        for (index, key) in self.keys.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(key.key, theme.key()));
            spans.push(Span::styled(format!(" {}", key.action), theme.hint()));
        }
        let keys = Line::from(spans);
        let Some(status) = self.status.filter(|status| !status.is_empty()) else {
            frame.render_widget(Paragraph::new(keys), foot);
            return body;
        };
        // Die Tasten links, der Stand rechts. Reicht der Platz nicht, bekommt
        // der Stand mindestens die Hälfte: Eine gehaltene Anfrage ist
        // wichtiger als die Hilfe zu den Tasten.
        let wanted = u16::try_from(Line::raw(status).width().saturating_add(1)).unwrap_or(u16::MAX);
        let keys_width = u16::try_from(keys.width()).unwrap_or(u16::MAX);
        let room = foot
            .width
            .saturating_sub(keys_width.saturating_add(1))
            .max(foot.width / 2);
        let [left, right] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(wanted.min(room))])
                .areas(foot);
        frame.render_widget(Paragraph::new(keys), left);
        frame.render_widget(
            Paragraph::new(format!("{status} "))
                .alignment(Alignment::Right)
                .style(theme.key()),
            right,
        );
        body
    }
}

/// Was an Stelle des Bildschirms steht, solange das Terminal kleiner ist als
/// sein Minimum: Größe gemessen und gebraucht, und dass `q` beendet.
pub fn too_small(frame: &mut Frame<'_>, min: Cells, theme: Theme) {
    let area = frame.area();
    let text = vec![
        Line::styled("Terminal too small", theme.key()),
        Line::raw(format!("needs {} × {}", min.cols, min.rows)),
        Line::raw(format!("has {} × {}", area.width, area.height)),
    ];
    let top = area.height.saturating_sub(3) / 2;
    let [_, middle, _] = Layout::vertical([
        Constraint::Length(top),
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), middle);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::OsStr;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::widgets::Paragraph;

    use super::{Key, ScreenFrame, Search, Theme, too_small};
    use crate::tui::size::Cells;

    /// Die Zeilen eines Bildes als Text, ohne Stile.
    fn lines(buffer: &Buffer) -> Vec<String> {
        let area = buffer.area;
        (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    const KEYS: &[Key<'static>] = &[
        Key {
            key: "q",
            action: "quit",
        },
        Key {
            key: "/",
            action: "search",
        },
    ];

    /// Zeichnet einen Rahmen mit Inhalt in ein Testterminal.
    fn render(theme: Theme, search: Option<&Search>, size: (u16, u16)) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).expect("a terminal");
        terminal
            .draw(|frame| {
                let body = ScreenFrame {
                    title: "Settings",
                    context: "~/project",
                    keys: KEYS,
                    search,
                    status: None,
                }
                .render(frame, theme);
                frame.render_widget(Paragraph::new("body"), body);
            })
            .expect("drawn");
        terminal.backend().buffer().clone()
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Der Stand steht rechts in der Fußzeile, die Tasten links; reicht der
    /// Platz nicht, bekommt der Stand die Hälfte (GitHub #11).
    #[test]
    fn the_status_stands_right_of_the_keys() {
        let draw = |status: &str, width: u16| {
            let mut terminal = Terminal::new(TestBackend::new(width, 3)).expect("a terminal");
            terminal
                .draw(|frame| {
                    ScreenFrame {
                        title: "Settings",
                        context: "",
                        keys: KEYS,
                        search: None,
                        status: Some(status),
                    }
                    .render(frame, Theme::from_no_color(None));
                })
                .expect("drawn");
            lines(terminal.backend().buffer()).pop().expect("a footer")
        };
        assert_eq!(draw("running", 36), " q quit  / search           running ");
        let long = draw("request held: example.test/path", 36);
        assert_eq!(long, " q quit  / search request held: exam");
    }

    /// Schnappschuss: Kopf oben mit Titel und Kontext, Inhalt dazwischen,
    /// Tasten unten.
    #[test]
    fn the_frame_puts_header_body_and_keys_in_place() {
        let buffer = render(Theme::from_no_color(None), None, (36, 5));
        assert_eq!(
            lines(&buffer),
            [
                " humanitl · Settings      ~/project ",
                "body                                ",
                "                                    ",
                "                                    ",
                " q quit  / search                   ",
            ]
        );
    }

    /// Schnappschuss: Eine offene Suche steht über der Fußzeile, mit dem
    /// Cursor hinter dem Getippten.
    #[test]
    fn an_open_search_sits_above_the_keys() {
        let mut search = Search::default();
        for code in [KeyCode::Char('/'), KeyCode::Char('C'), KeyCode::Char('o')] {
            assert!(search.handle(&key(code)));
        }
        let buffer = render(Theme::from_no_color(None), Some(&search), (36, 5));
        assert_eq!(
            lines(&buffer),
            [
                " humanitl · Settings      ~/project ",
                "body                                ",
                "                                    ",
                "/Co                                 ",
                " q quit  / search                   ",
            ]
        );
    }

    /// Mit Farbe ist die Kopfzeile gelb; mit `NO_COLOR` trägt keine Zelle
    /// eine Farbe, die Kopfzeile ist dann umgekehrt.
    #[test]
    fn no_color_leaves_every_cell_without_colour() {
        let coloured = render(Theme::from_no_color(None), None, (36, 5));
        assert_eq!(coloured[(0, 0)].bg, Color::Yellow);

        let mono = render(Theme::from_no_color(Some(OsStr::new("1"))), None, (36, 5));
        for cell in mono.content() {
            assert_eq!(cell.fg, Color::Reset, "{cell:?}");
            assert_eq!(cell.bg, Color::Reset, "{cell:?}");
        }
        assert!(
            mono[(0, 0)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
        assert!(
            Theme::from_no_color(Some(OsStr::new(""))).colour(),
            "an empty NO_COLOR does not count (no-color.org)"
        );
    }

    /// Die Suche: `/` öffnet, Tippen füllt, `Esc` leert, `Enter` behält;
    /// gefunden wird jedes Wort, ohne Groß und Klein.
    #[test]
    fn search_opens_types_and_matches_words() {
        let mut search = Search::default();
        assert!(
            !search.handle(&key(KeyCode::Char('x'))),
            "closed, only / counts"
        );
        assert!(search.handle(&key(KeyCode::Char('/'))));
        for c in "proxy PORT".chars() {
            search.handle(&key(KeyCode::Char(c)));
        }
        assert!(search.matches("Proxy listen port"));
        assert!(!search.matches("Proxy address"));
        assert!(search.handle(&key(KeyCode::Backspace)));
        assert_eq!(search.query(), "proxy POR");
        assert!(search.handle(&key(KeyCode::Enter)));
        assert!(!search.is_open());
        assert_eq!(search.query(), "proxy POR", "Enter keeps the text");
        search.handle(&key(KeyCode::Char('/')));
        search.handle(&key(KeyCode::Esc));
        assert_eq!(search.query(), "", "Esc clears");
        assert!(search.matches("anything"), "an empty search matches all");
    }

    /// Schnappschuss: Zu klein steht mittig da, mit beiden Größen.
    #[test]
    fn too_small_names_both_sizes() {
        let mut terminal = Terminal::new(TestBackend::new(20, 5)).expect("a terminal");
        terminal
            .draw(|frame| too_small(frame, Cells::new(60, 16), Theme::from_no_color(None)))
            .expect("drawn");
        assert_eq!(
            lines(terminal.backend().buffer()),
            [
                "                    ",
                " Terminal too small ",
                "    needs 60 × 16   ",
                "     has 20 × 5     ",
                "                    ",
            ]
        );
    }
}
