//! Das Bild des Agenten und die kleine Ansicht darüber (GitHub #10).
//!
//! Die Bytes des Agenten gehen unverändert auf das eigene Terminal, wie vor
//! diesem Issue. Steht eine Ansicht (die Hilfe hinter `?`, die Einstellungen
//! aus GitHub #11 und der Interceptor aus GitHub #12), hält die
//! Kommandozeile die Bytes des Agenten zurück, bis zu [`HELD_LIMIT`], und
//! gibt sie danach in ihrer Reihenfolge aus. Den Kasten der Hilfe zeichnet
//! diese Datei mit ratatui in einen Puffer, der nur ihn abdeckt;
//! Einstellungen und Interceptor zeichnet ihr eigener Bildschirm
//! (`prefix::settings`, `prefix::interceptor`), und diese Datei setzt nur den
//! Schirm dafür auf und wieder zurück ([`AgentView::open_screen`]).
//!
//! # Die Glocke bei einer gehaltenen Anfrage (GitHub #12)
//!
//! [`AgentView::hold_bell`] schreibt ein `BEL` über denselben Schreiber wie
//! die Ausgabe des Agenten und nur zwischen zwei ganzen Stücken: Jedes Stück
//! geht in [`AgentView::agent`] mit einem `write_all` hinaus, und die Glocke
//! kommt aus derselben Schleife des Anschlusses erst danach. Ein Stück des
//! Daemons endet nie in einer halben Folge, also steht das `BEL` nie in einem
//! `ESC [ …` des Agenten. Steht eine Ansicht, klingelt nichts: Ein `BEL` in
//! der zurückgehaltenen Ausgabe käme erst nach der Rückkehr, und eines direkt
//! auf das Terminal träfe den Bildschirm der Ansicht, der gerade auf einem
//! anderen Faden zeichnet.
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
use humanitl_config::prefix::help_lines;
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
///
/// **Nach einem Überlauf gar nichts.** Mit dem ersten Stück, das nicht mehr
/// passt, fällt auch alles weg, was bis dahin wartete (Review zu GitHub
/// #11): Der Rest vor der Lücke ergäbe ein Bild, dem ein Stück fehlt, und das
/// neue Bild des Agenten ersetzt ihn ohnehin.
///
/// Vier MiB, eine Grenze für Kasten und Einstellungen (GitHub #11): genug
/// für viele Bilder eines Vollbild-Agenten, solange jemand in den
/// Einstellungen liest, und wenig genug für den Speicher eines Clients.
pub const HELD_LIMIT: usize = 4 * 1024 * 1024;

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
    /// Wahr für einen ganzen Bildschirm (die Einstellungen), falsch für
    /// einen Kasten.
    screen: bool,
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
    /// Eine Glocke, die wartet, bis die Ausgabe an einer Grenze zwischen
    /// zwei UTF-8-Zeichen steht ([`AgentView::hold_bell`]).
    bell_due: bool,
    /// Wahr, sobald eine aufgeschobene Glocke geschrieben wurde; die Drossel
    /// holt es ab ([`AgentView::take_bell_written`]).
    bell_written: bool,
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
            bell_due: false,
            bell_written: false,
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
                self.held = Vec::new();
            } else {
                self.held.extend_from_slice(bytes);
            }
            return Ok(());
        }
        self.out.write_all(bytes)?;
        self.out.flush()?;
        self.stream.observe(bytes);
        if self.bell_due && self.stream.on_char_boundary() {
            self.bell_due = false;
            self.bell_written = true;
            self.bell()?;
        }
        Ok(())
    }

    /// Öffnet die Hilfe hinter `?` als Kasten in einem Terminal der Größe
    /// `size`; die nächste Taste schließt ihn. Steht schon eine Ansicht,
    /// bleibt sie.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn open_help(&mut self, size: (u32, u32)) -> io::Result<()> {
        if self.shown.is_some() {
            return Ok(());
        }
        // Unter einer Ansicht klingelt nichts, auch keine aufgeschobene
        // Glocke.
        self.bell_due = false;
        let (label, letter) = &self.prefix;
        let title = " Prefix keys ";
        let mut lines = help_lines(label, *letter);
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
        self.shown = Some(Shown {
            switched,
            screen: false,
        });
        Ok(())
    }

    /// Setzt den Schirm für einen ganzen Bildschirm über dem Agenten auf,
    /// die Einstellungen (GitHub #11), und hält ab jetzt seine Ausgabe
    /// zurück. Gezeichnet wird danach von dem Bildschirm selbst; diese Datei
    /// schreibt nur die festen Folgen davor und danach. Steht schon eine
    /// Ansicht, bleibt sie.
    ///
    /// - Über einem Agenten auf dem gewöhnlichen Schirm zieht die Ansicht
    ///   auf den Alternativschirm um; zurück stellt das Terminal das Bild
    ///   selbst wieder her.
    /// - Steht der Agent schon auf dem Alternativschirm, schaltet die Ansicht
    ///   nicht ein zweites Mal um: Ein `CSI ? 1049 l` beim Zurückkehren
    ///   verließe sonst auch den Schirm des Agenten. Sie sichert den Cursor
    ///   (`ESC 7`), zeichnet darüber und löscht beim Zurückkehren, was sie
    ///   gezeichnet hat; das Bild des Agenten kommt mit seinem Neuzeichnen.
    ///
    /// Die Ansicht versteckt den Cursor und klammert eingefügten Text, damit
    /// ein eingefügter Wert als ein Stück ankommt. Beides stellt
    /// [`AgentView::close`] auf den Stand des Agenten zurück.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn open_screen(&mut self) -> io::Result<()> {
        if self.shown.is_some() {
            return Ok(());
        }
        self.bell_due = false;
        let switched = !self.stream.alt_screen();
        let mut bytes = if switched {
            b"\x1b[?1049h".to_vec()
        } else {
            b"\x1b7".to_vec()
        };
        bytes.extend_from_slice(b"\x1b[0m\x1b[H\x1b[2J\x1b[?25l\x1b[?2004h");
        // Erst gilt die Ansicht, dann wird geschrieben: Auch wenn das
        // Schreiben scheitert, hält der Befehl die Ausgabe zurück und das
        // Schließen räumt auf.
        self.shown = Some(Shown {
            switched,
            screen: true,
        });
        self.out.write_all(&bytes)?;
        self.out.flush()
    }

    /// Schließt die Ansicht und gibt aus, was solange wartete. Wahr, wenn
    /// der Agent danach neu zeichnen soll: nach einem ganzen Bildschirm
    /// immer (GitHub #11, Punkt 2), nach einem Kasten, wenn er über dem Bild
    /// des Agenten stand, und in jedem Fall, wenn Stücke seiner Ausgabe
    /// weggefallen sind.
    ///
    /// Die Reihenfolge ist die der Spezifikation: erst den eigenen Schirm
    /// verlassen, dann die zurückgehaltene Ausgabe, oder nach einem Überlauf
    /// keine; das Neuzeichnen bittet danach der Anschluss an.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn close(&mut self) -> io::Result<bool> {
        let Some(shown) = self.shown.take() else {
            return Ok(false);
        };
        let back = if shown.screen {
            self.screen_back(shown.switched)
        } else if shown.switched {
            b"\x1b[?1049l".to_vec()
        } else {
            b"\x1b8".to_vec()
        };
        let held = std::mem::take(&mut self.held);
        let overflowed = std::mem::take(&mut self.overflowed);
        self.out.write_all(&back)?;
        self.out.flush()?;
        if !overflowed {
            self.agent(&held)?;
        }
        Ok(shown.screen || !shown.switched || overflowed)
    }

    /// Ob der Agent gerade auf dem Alternativschirm zeichnet.
    pub const fn agent_alt_screen(&self) -> bool {
        self.stream.alt_screen()
    }

    /// Die Glocke für eine neu gehaltene Anfrage (GitHub #12): ein `BEL`
    /// zwischen zwei ganzen Stücken der Ausgabe des Agenten, über denselben
    /// Schreiber. Wahr nur, wenn das `BEL` jetzt geschrieben wurde. Steht eine
    /// Ansicht, klingelt nichts; endete das letzte Stück mitten in einem
    /// UTF-8-Zeichen, wartet die Glocke ([`AgentView::bell_pending`]) und
    /// geht hinter dem Stück hinaus, das es vollendet. In beiden Fällen ist
    /// das Ergebnis `false`: Die Drossel zählt eine Glocke ab dem Augenblick,
    /// in dem sie wirklich klang (`prefix::bell`).
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn hold_bell(&mut self) -> io::Result<bool> {
        if self.shown.is_some() {
            return Ok(false);
        }
        // Endete das letzte Stück mitten in einem UTF-8-Zeichen, wartet die
        // Glocke hinter das Stück, das es vollendet (Review zu GitHub #12).
        if !self.stream.on_char_boundary() {
            self.bell_due = true;
            return Ok(false);
        }
        self.bell()?;
        Ok(true)
    }

    /// Ob eine aufgeschobene Glocke auf das Ende eines UTF-8-Zeichens wartet.
    pub const fn bell_pending(&self) -> bool {
        self.bell_due
    }

    /// Ob seit dem letzten Fragen eine aufgeschobene Glocke geschrieben
    /// wurde; danach falsch, bis die nächste geht.
    pub const fn take_bell_written(&mut self) -> bool {
        let written = self.bell_written;
        self.bell_written = false;
        written
    }

    /// Ein kurzes Zeichen (`BEL`), dass eine Taste hier nichts öffnen kann.
    /// Keine Folge, die etwas am Terminal ändert.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn bell(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x07")?;
        self.out.flush()
    }

    /// Die Folge zurück aus einem ganzen Bildschirm: Stile zurück, den
    /// eigenen Schirm verlassen (oder über dem Agenten löschen und den
    /// Cursor zurück), dann Klammern und Cursor auf den Stand, den der Agent
    /// zuletzt gesetzt hat.
    fn screen_back(&self, switched: bool) -> Vec<u8> {
        let mut bytes = b"\x1b[0m".to_vec();
        if switched {
            bytes.extend_from_slice(b"\x1b[?1049l");
        } else {
            bytes.extend_from_slice(b"\x1b[H\x1b[2J\x1b8");
        }
        bytes.extend_from_slice(if self.stream.bracketed_paste() {
            b"\x1b[?2004h"
        } else {
            b"\x1b[?2004l"
        });
        bytes.extend_from_slice(if self.stream.cursor_hidden() {
            b"\x1b[?25l"
        } else {
            b"\x1b[?25h"
        });
        bytes
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
        view.open_help((80, 24)).unwrap();
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
        view.open_help((80, 24)).unwrap();
        let shown = text(&view);
        assert!(shown.contains("Ctrl+B d"), "{shown:?}");
        assert!(shown.contains("\x1b7"), "{shown:?}");
        assert_eq!(shown.matches("\x1b[?1049h").count(), 1, "{shown:?}");
        let before = view.out().len();
        view.agent(b"kept").unwrap();
        view.agent(&vec![b'x'; HELD_LIMIT]).unwrap();
        assert!(view.close().unwrap(), "redraw over a full-screen agent");
        let after = text(&view);
        assert!(
            !after[before..].contains("kept"),
            "after an overflow nothing is replayed, not even what fit"
        );
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
        view.open_help((80, 24)).unwrap();
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

    /// Die Einstellungen über einem Vollbild-Agenten (GitHub #11): kein
    /// zweites `CSI ? 1049 h`, das beim Zurückkehren auch den Schirm des
    /// Agenten verließe; die Ausgabe während der Ansicht wartet und kommt
    /// nach dem Zurückkehren in ihrer Reihenfolge, nach dem Löschen der
    /// Ansicht; Klammern und Cursor stehen danach wie beim Agenten, und der
    /// Agent soll neu zeichnen.
    #[test]
    fn the_settings_over_a_full_screen_agent_come_back_in_order() {
        let mut view = view();
        view.agent(b"\x1b[?1049h\x1b[?2004h\x1b[?25l").unwrap();
        let before = view.out().len();
        view.open_screen().unwrap();
        let opened = text(&view)[before..].to_owned();
        assert!(opened.starts_with("\x1b7"), "{opened:?}");
        assert!(!opened.contains("1049"), "{opened:?}");
        assert!(opened.contains("\x1b[?2004h"), "{opened:?}");
        view.agent(b"one ").unwrap();
        view.agent(b"two ").unwrap();
        view.agent(b"three").unwrap();
        assert!(!text(&view).contains("one"), "held while the view stands");
        assert!(view.close().unwrap(), "a redraw after the settings");
        let after = text(&view)[before..].to_owned();
        let back = after
            .find("\x1b[0m\x1b[H\x1b[2J\x1b8\x1b[?2004h\x1b[?25l")
            .expect("the view is cleared, cursor, brackets and cursor mode as the agent left them");
        assert!(!after[back..].contains("1049"), "{after:?}");
        assert!(after[back..].ends_with("one two three"), "{after:?}");
    }

    /// Über einer Shell zieht die Ansicht auf den Alternativschirm um und
    /// kehrt mit `CSI ? 1049 l` zurück; neu zeichnen soll der Agent auch
    /// dann (GitHub #11, Punkt 2), und Klammern und Cursor stehen wie bei der
    /// Shell. Mutationsbeweis: die Regel des Kastens (kein Neuzeichnen über
    /// einer Shell) macht ihn rot.
    #[test]
    fn the_settings_over_a_shell_leave_their_own_screen_and_redraw() {
        let mut view = view();
        view.agent(b"$ ").unwrap();
        view.open_screen().unwrap();
        assert!(text(&view).contains("\x1b[?1049h"), "{:?}", text(&view));
        view.agent(b"later").unwrap();
        assert!(view.close().unwrap(), "a redraw after the settings");
        let after = text(&view);
        let back = after
            .find("\x1b[0m\x1b[?1049l\x1b[?2004l\x1b[?25h")
            .expect("back to the shell");
        assert!(after[back..].ends_with("later"), "{after:?}");
    }

    /// Über der Grenze fällt die Ausgabe während der Einstellungen ganz weg,
    /// auch was vor dem Überlauf passte, und der Agent zeichnet trotzdem neu
    /// (GitHub #11, Review: 3 MiB und dann 2 MiB ergaben vorher 3 MiB mit
    /// einer Lücke dahinter). Mutationsbeweis: das Zurückhalten vor dem
    /// Überlauf wieder ausgeben macht ihn rot.
    #[test]
    fn the_settings_over_the_limit_drop_the_output_and_redraw_anyway() {
        let mut view = view();
        view.agent(b"\x1b[?1049h").unwrap();
        view.open_screen().unwrap();
        view.agent(&vec![b'k'; 3 * 1024 * 1024]).unwrap();
        view.agent(&vec![b'x'; 2 * 1024 * 1024]).unwrap();
        view.agent(b"after").unwrap();
        assert!(view.close().unwrap(), "redraw anyway");
        let out = text(&view);
        assert!(
            out.ends_with("\x1b8\x1b[?2004l\x1b[?25h"),
            "{:?}",
            &out[out.len().saturating_sub(80)..]
        );
        assert!(
            !out.contains("kkkk") && !out.contains("xxxx") && !out.contains("after"),
            "dropped whole"
        );
    }

    /// Endet der Strom bei offener Ansicht, gibt das Verlassen aus, was
    /// wartete, und schließt die Ansicht zuerst.
    #[test]
    fn leaving_with_an_open_view_releases_what_waited() {
        let mut view = view();
        view.open_help((80, 24)).unwrap();
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
        view.open_help((80, 24)).unwrap();
        view.agent(b"bye").unwrap();
        view.leave(false);
        let out = text(&view);
        assert!(out.ends_with("\x1b[?1049lbye"), "{out:?}");
    }
}
