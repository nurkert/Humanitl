//! Das Bild des Agenten und die kleine Ansicht darüber (GitHub #10).
//!
//! Die Bytes des Agenten gehen unverändert auf das eigene Terminal, wie vor
//! diesem Issue. Steht eine Ansicht (die Hilfe hinter `?`, die Zeile für
//! Einstellungen und Interceptor, bis #11 und #12 sie bauen), hält die
//! Kommandozeile die Bytes des Agenten zurück, bis zu [`HELD_LIMIT`], und
//! gibt sie danach in ihrer Reihenfolge aus. Gezeichnet wird mit ratatui in
//! einen Puffer, der nur den Kasten der Ansicht abdeckt.
//!
//! Wie die Ansicht wieder verschwindet, hängt vom Schirm des Agenten ab
//! ([`super::stream::StreamState::alt_screen`]):
//!
//! - **Gewöhnlicher Schirm** (eine Shell): Die Ansicht zieht auf den
//!   Alternativschirm um (`CSI ? 1049 h`) und kehrt mit `CSI ? 1049 l`
//!   zurück; das Terminal stellt das Bild und den Cursor selbst wieder her.
//! - **Alternativschirm** (ein Vollbild-Agent): Die Ansicht zeichnet darüber
//!   (Cursor gesichert mit `ESC 7`, zurück mit `ESC 8`), und der Anschluss
//!   bittet den Agenten danach, neu zu zeichnen: eine Größenänderung auf
//!   dieselbe Größe über die `Terminal`-RPC (ADR-0024 Punkt 9).
//!
//! Keine dieser Folgen kommt vom Agenten; die Ausgabe des Agenten geht nie
//! durch ratatui und bleibt beim Filter `FullScreen` des Daemons.

use std::io::{self, Write};

use humanitl_config::DetachKeys;
use humanitl_config::prefix::{PrefixAction, coming, help_lines};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget as _};

use super::stream::StreamState;
use crate::tui::Theme;

/// Wie viele Bytes des Agenten höchstens warten, solange eine Ansicht
/// steht.
///
/// **Nur ganze Stücke.** Jedes Stück des Daemons ist für sich gefiltert und
/// endet nie in einer halben Folge; ein Schnitt mitten im Stück könnte ein
/// `ESC` behalten, das mit dem Anfang eines späteren Stücks (`]52;…`) eine
/// Folge bildet, die der Filter nie gesehen hat (Review zu GitHub #10). Ein
/// Stück, das nicht mehr passt, fällt deshalb ganz weg, und mit ihm jedes
/// weitere, bis die Ansicht schließt; danach zeichnet der Agent neu.
pub const HELD_LIMIT: usize = 1024 * 1024;

/// Was der Befehl beim Verlassen in das eigene Terminal schreibt, zwischen
/// den Flags der beiden Schirme ([`AgentView::leave`]).
///
/// Ein Vollbild-Agent lässt den Alternativschirm an und den Cursor
/// versteckt. Wer sich abhängt, bekäme sonst eine Shell ohne sichtbaren
/// Cursor auf einem fremden Bild.
const LEAVE_SCREEN: &[u8] = b"\x1b[?1049l\x1b[?25h\r\n";

/// Eine offene Ansicht.
#[derive(Debug, Clone, Copy)]
struct Shown {
    /// Wahr, wenn die Ansicht auf den Alternativschirm umgezogen ist.
    switched: bool,
}

/// Das eigene Terminal, solange es an einer Sitzung hängt.
pub struct AgentView<W: Write> {
    /// Das Terminal.
    out: W,
    /// Was die Ausgabe des Agenten gesetzt hat.
    stream: StreamState,
    /// Die offene Ansicht.
    shown: Option<Shown>,
    /// Die Bytes des Agenten, die warten, solange eine Ansicht steht: nur
    /// ganze Stücke.
    held: Vec<u8>,
    /// Wahr, sobald ein Stück nicht mehr passte: Ab da fällt jedes weitere
    /// weg, bis die Ansicht schließt.
    overflowed: bool,
    /// Der Name des Präfixes (`Ctrl+B`) und sein Buchstabe.
    prefix: (String, char),
    /// Mit oder ohne Farbe.
    theme: Theme,
}

impl<W: Write> AgentView<W> {
    /// Ein Terminal ohne Ansicht.
    pub fn new(out: W, stream: StreamState, keys: DetachKeys, theme: Theme) -> Self {
        let label = keys.label().unwrap_or_default();
        let letter = keys.letter().map_or(' ', char::from);
        Self {
            out,
            stream,
            shown: None,
            held: Vec::new(),
            overflowed: false,
            prefix: (label, letter),
            theme,
        }
    }

    /// Bytes des Agenten: sofort auf das Terminal, oder zurückgehalten,
    /// solange eine Ansicht steht.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn agent(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.shown.is_some() {
            if self.overflowed || self.held.len().saturating_add(bytes.len()) > HELD_LIMIT {
                self.overflowed = true;
            } else {
                self.held.extend_from_slice(bytes);
            }
            return Ok(());
        }
        self.out.write_all(bytes)?;
        self.out.flush()?;
        self.stream.observe(bytes);
        Ok(())
    }

    /// Öffnet die Ansicht für `action` in einem Terminal der Größe `size`.
    /// Steht schon eine, bleibt sie.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn open(&mut self, action: PrefixAction, size: (u32, u32)) -> io::Result<()> {
        if self.shown.is_some() {
            return Ok(());
        }
        let (label, letter) = &self.prefix;
        let (title, mut lines) = match coming(action) {
            Some(sentence) => (" humanitl ", sentence.map(str::to_owned).to_vec()),
            None => (" Prefix keys ", help_lines(label, *letter)),
        };
        lines.push("Any key closes this.".to_owned());
        let switched = !self.stream.alt_screen();
        let mut bytes = if switched {
            b"\x1b[?1049h\x1b[H\x1b[2J".to_vec()
        } else {
            b"\x1b7".to_vec()
        };
        bytes.extend(draw(title, &lines, size, self.theme)?);
        self.out.write_all(&bytes)?;
        self.out.flush()?;
        self.shown = Some(Shown { switched });
        Ok(())
    }

    /// Schließt die Ansicht und gibt aus, was solange wartete. Wahr, wenn
    /// der Agent danach neu zeichnen soll: weil die Ansicht über seinem Bild
    /// stand, oder weil Stücke seiner Ausgabe weggefallen sind.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn close(&mut self) -> io::Result<bool> {
        let Some(shown) = self.shown.take() else {
            return Ok(false);
        };
        let back: &[u8] = if shown.switched {
            b"\x1b[?1049l"
        } else {
            b"\x1b8"
        };
        self.out.write_all(back)?;
        self.out.flush()?;
        let held = std::mem::take(&mut self.held);
        let overflowed = std::mem::take(&mut self.overflowed);
        self.agent(&held)?;
        Ok(!shown.switched || overflowed)
    }

    /// Gibt das Terminal zurück: eine offene Ansicht schließen und ausgeben,
    /// was wartete (ganze Stücke), auf jedem Weg hinaus. Nur wenn die
    /// Ausgabe ein Terminal ist (`terminal`), danach auch die Flags des
    /// kitty-Protokolls, die der Agent stehen ließ, herunter, gewöhnlicher
    /// Schirm, Cursor sichtbar.
    pub fn leave(&mut self, terminal: bool) {
        let _ = self.close();
        if !terminal {
            return;
        }
        let mut bytes = self.stream.leave_before();
        bytes.extend_from_slice(LEAVE_SCREEN);
        bytes.extend(self.stream.leave_after());
        let _ = self.out.write_all(&bytes);
        let _ = self.out.flush();
    }

    /// Das Terminal, für die Tests.
    #[cfg(test)]
    pub const fn out(&self) -> &W {
        &self.out
    }
}

/// Zeichnet den Kasten einer Ansicht oben links in ein Terminal der Größe
/// `size` und gibt die Bytes dafür zurück: nur die Zellen des Kastens, mit
/// Cursor-Positionen, zuletzt die Stile zurückgesetzt.
fn draw(title: &str, lines: &[String], size: (u32, u32), theme: Theme) -> io::Result<Vec<u8>> {
    let cols = u16::try_from(size.0).unwrap_or(u16::MAX).max(1);
    let rows = u16::try_from(size.1).unwrap_or(u16::MAX).max(1);
    let widest = lines.iter().map(String::len).max().unwrap_or(0);
    let width = u16::try_from(widest.saturating_add(4))
        .unwrap_or(u16::MAX)
        .min(cols);
    let height = u16::try_from(lines.len().saturating_add(2))
        .unwrap_or(u16::MAX)
        .min(rows);
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    let text: Vec<Line<'_>> = lines
        .iter()
        .map(|line| Line::raw(format!(" {line}")))
        .collect();
    Paragraph::new(text)
        .block(
            Block::bordered()
                .title(Line::styled(title, theme.header()))
                .border_style(theme.key()),
        )
        .render(area, &mut buffer);
    let mut bytes = Vec::new();
    {
        let mut backend = CrosstermBackend::new(&mut bytes);
        backend.draw(buffer.content.iter().enumerate().map(|(index, cell)| {
            let (x, y) = buffer.pos_of(index);
            (x, y, cell)
        }))?;
        Backend::flush(&mut backend)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use humanitl_config::DetachKeys;
    use humanitl_config::prefix::PrefixAction;

    use super::{AgentView, HELD_LIMIT};
    use crate::prefix::stream::StreamState;
    use crate::tui::Theme;

    fn view() -> AgentView<Vec<u8>> {
        AgentView::new(
            Vec::new(),
            StreamState::new(Arc::new(AtomicBool::new(false))),
            DetachKeys::Prefix(0x02),
            Theme::from_no_color(Some(std::ffi::OsStr::new("1"))),
        )
    }

    fn text(view: &AgentView<Vec<u8>>) -> String {
        String::from_utf8_lossy(view.out()).into_owned()
    }

    /// Über einer Shell zieht die Hilfe auf den Alternativschirm um, zeigt
    /// die Tabelle, hält die Ausgabe des Agenten zurück und gibt sie nach dem
    /// Schließen aus; neu zeichnen muss niemand.
    #[test]
    fn the_help_over_a_shell_uses_the_alternate_screen() {
        let mut view = view();
        view.agent(b"$ ").unwrap();
        view.open(PrefixAction::Help, (80, 24)).unwrap();
        let shown = text(&view);
        assert!(shown.contains("\x1b[?1049h"), "{shown:?}");
        assert!(shown.contains("Ctrl+B d"), "{shown:?}");
        assert!(shown.contains("Ctrl+B Ctrl+B or Ctrl+B b"), "{shown:?}");
        view.agent(b"later").unwrap();
        assert!(!text(&view).contains("later"), "held while the help stands");
        assert!(!view.close().unwrap(), "no redraw over a shell");
        let after = text(&view);
        let back = after.find("\x1b[?1049l").expect("back to the shell");
        assert!(after[back..].contains("later"), "released after closing");
    }

    /// Über einem Vollbild-Agenten zeichnet die Zeile darüber und verlangt
    /// danach ein neues Bild; zurückgehalten wird höchstens bis zur Grenze.
    #[test]
    fn a_view_over_a_full_screen_agent_asks_for_a_redraw() {
        let mut view = view();
        view.agent(b"\x1b[?1049h").unwrap();
        view.open(PrefixAction::Settings, (80, 24)).unwrap();
        let shown = text(&view);
        assert!(shown.contains("humanitl settings"), "{shown:?}");
        assert!(shown.contains("\x1b7"), "{shown:?}");
        assert_eq!(shown.matches("\x1b[?1049h").count(), 1, "{shown:?}");
        let before = view.out().len();
        view.agent(b"kept").unwrap();
        view.agent(&vec![b'x'; HELD_LIMIT]).unwrap();
        assert!(view.close().unwrap(), "redraw over a full-screen agent");
        let after = text(&view);
        assert!(after[before..].contains("kept"), "the chunk that fit");
        assert!(
            !after.contains("xxxx"),
            "a chunk over the limit is dropped whole"
        );
    }

    /// Über die Grenze wird nie mitten in einem Stück geschnitten: Ein Stück,
    /// das auf `ESC` endet und nicht mehr passt, fällt ganz weg, das nächste
    /// (`]52;…`) ebenso, und auch was nach dem Schließen kommt, bildet mit
    /// keinem behaltenen Rest eine OSC-52-Folge. Nach dem Überlauf zeichnet
    /// der Agent neu, auch über einer Shell. Mutationsbeweis: der Schnitt
    /// mitten im Stück macht ihn rot.
    #[test]
    fn an_overflow_never_cuts_a_chunk() {
        let mut view = view();
        view.open(PrefixAction::Help, (80, 24)).unwrap();
        view.agent(&vec![b'x'; HELD_LIMIT - 1]).unwrap();
        view.agent(b"\x1b[?25l").unwrap();
        view.agent(b"]52;c;aGk=\x07").unwrap();
        assert!(view.close().unwrap(), "an overflow asks for a redraw");
        view.agent(b"]52;c;aGk=\x07").unwrap();
        let out = text(&view);
        assert!(!out.contains("\x1b]52"), "an OSC 52 reached the terminal");
        assert!(
            !out.contains("x\x1b"),
            "a partial sequence after the held text"
        );
        assert!(
            !out.contains("\x1b[?25l"),
            "the chunk over the limit is gone"
        );
    }

    /// Endet der Strom bei offener Ansicht, gibt das Verlassen aus, was
    /// wartete, und schließt die Ansicht zuerst.
    #[test]
    fn leaving_with_an_open_view_releases_what_waited() {
        let mut view = view();
        view.open(PrefixAction::Help, (80, 24)).unwrap();
        view.agent(b"bye").unwrap();
        view.leave(true);
        let out = text(&view);
        let back = out.find("\x1b[?1049l").expect("the view closed");
        assert!(out[back..].contains("bye"), "{out:?}");
    }

    /// Auch ohne Terminal auf `stdout` schließt das Verlassen die Ansicht und
    /// gibt aus, was wartete; nur das Aufräumen des Schirms entfällt.
    /// Mutationsbeweis: das Schließen nur an einem Terminal macht ihn rot.
    #[test]
    fn leaving_without_a_terminal_still_releases_what_waited() {
        let mut view = view();
        view.open(PrefixAction::Help, (80, 24)).unwrap();
        view.agent(b"bye").unwrap();
        view.leave(false);
        let out = text(&view);
        assert!(out.ends_with("\x1b[?1049lbye"), "{out:?}");
    }
}
