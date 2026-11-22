//! Was ein Terminal mindestens können muss, damit ein Bildschirm darauf
//! steht, und der Befund `TERM_003`, wenn es das nicht kann (GitHub #6).
//!
//! Geprüft wird **bevor** irgendetwas am Terminal geändert ist: Ein zu
//! kleines Terminal bekommt den Befund auf einem gewöhnlichen Bildschirm und
//! sonst nichts, keinen Rohmodus, keinen alternativen Bildschirm.
//!
//! Zwei Eigenschaften liest diese Datei aus `TERM` (und `TERM_PROGRAM`):
//!
//! - **Adressierbar**: Ohne `TERM` oder mit `TERM=dumb` gibt es keine
//!   Cursorsteuerung; ein Bildschirm wäre dort nur Zeichensalat. Dann
//!   `TERM_003` und der Weg ohne Bildschirm.
//! - **Alternativer Bildschirm**: nur für Terminals, von denen bekannt ist,
//!   dass sie `?1049` kennen ([`WITH_ALTERNATE_SCREEN`]). Alle anderen, die
//!   Konsole von Linux, `vt320`, ein unbekannter Name, zeichnen im
//!   gewöhnlichen Bildschirm und löschen ihn am Ende: Ein Terminal ohne die
//!   Umschaltung zeigte sie nicht an, und Reste blieben stehen.
//!
//! Die Größe liest diese Datei selbst und nicht über `tty::window_size`:
//! Dort wird ein Terminal ohne Größe (0 × 0, oder `TIOCGWINSZ` scheitert) als
//! 80 × 24 gerechnet, was für eine Zeile Text richtig ist und für einen
//! Bildschirm falsch. Hier ist es `TERM_003`.

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};

/// Breite und Höhe in Zellen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cells {
    /// Spalten.
    pub cols: u16,
    /// Zeilen.
    pub rows: u16,
}

impl Cells {
    /// Spalten und Zeilen.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// Ob `self` mindestens `min` groß ist, in beiden Richtungen.
    #[must_use]
    pub const fn holds(self, min: Self) -> bool {
        self.cols >= min.cols && self.rows >= min.rows
    }

    /// Aus der Größe, die `tty::window_size` liefert; was über `u16` geht,
    /// gilt als `u16::MAX`.
    #[must_use]
    pub fn from_window((cols, rows): (u32, u32)) -> Self {
        Self {
            cols: u16::try_from(cols).unwrap_or(u16::MAX),
            rows: u16::try_from(rows).unwrap_or(u16::MAX),
        }
    }

    /// Die Größe, die das Terminal auf `stdin` selbst meldet; `None`, wenn es
    /// keine meldet (0 in einer Richtung, oder `TIOCGWINSZ` scheitert).
    #[must_use]
    pub fn reported() -> Option<Self> {
        let size = rustix::termios::tcgetwinsize(rustix::stdio::stdin()).ok()?;
        (size.ws_col > 0 && size.ws_row > 0).then(|| Self::new(size.ws_col, size.ws_row))
    }
}

/// Was `TERM` über das Terminal sagt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// Der Cursor lässt sich setzen; ohne das kein Bildschirm.
    pub addressable: bool,
    /// `?1049h` schaltet in einen eigenen Bildschirm um.
    pub alternate_screen: bool,
}

/// Familien von `TERM`, die den alternativen Bildschirm kennen. Ein Name
/// gehört dazu, wenn er so heißt oder so beginnt und danach `-`, `.` oder
/// `+` folgt (`xterm-256color`, `screen.xterm-256color`, `rxvt-unicode`).
/// Eine Familie zu wenig kostet nur das Löschen am Ende; eine zu viel ließe
/// bei einem Terminal ohne Umschaltung Reste stehen.
pub const WITH_ALTERNATE_SCREEN: &[&str] = &[
    "xterm",
    "screen",
    "tmux",
    "rxvt",
    "alacritty",
    "kitty",
    "foot",
    "wezterm",
    "vte",
    "gnome",
    "konsole",
    "st",
    "contour",
    "ghostty",
    "putty",
    "mintty",
    "iterm",
    "iterm2",
    "nsterm",
];

/// Werte von `TERM_PROGRAM`, die den alternativen Bildschirm kennen, auch
/// wenn `TERM` es nicht verrät.
const PROGRAMS_WITH_ALTERNATE_SCREEN: &[&str] = &[
    "iTerm.app",
    "Apple_Terminal",
    "WezTerm",
    "ghostty",
    "vscode",
    "tmux",
];

/// Ob `term` zu einer der Familien gehört.
fn in_family(term: &str) -> bool {
    WITH_ALTERNATE_SCREEN.iter().any(|family| {
        term.strip_prefix(family)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['-', '.', '+']))
    })
}

impl Capabilities {
    /// Liest `TERM` und `TERM_PROGRAM`.
    #[must_use]
    pub fn from_env(term: Option<&str>, program: Option<&str>) -> Self {
        let term = term.unwrap_or_default().trim();
        let addressable = !term.is_empty() && term != "dumb";
        let known = in_family(term)
            || program.is_some_and(|program| PROGRAMS_WITH_ALTERNATE_SCREEN.contains(&program));
        Self {
            addressable,
            alternate_screen: addressable && known,
        }
    }

    /// Nur nach `TERM`.
    #[must_use]
    pub fn from_term(term: Option<&str>) -> Self {
        Self::from_env(term, None)
    }
}

/// Der Befund, wenn ein Bildschirm nicht stehen kann.
///
/// `size` ist [`Cells::reported`]: `None` heißt, das Terminal meldet keine
/// Größe. `without_tui` ist der Weg ohne Bildschirm, den jeder Bildschirm in
/// seinem Issue nennt, zum Beispiel `humanitl config show`.
#[must_use]
pub fn unsuitable(
    caps: Capabilities,
    size: Option<Cells>,
    min: Cells,
    without_tui: &str,
) -> Option<Diagnostic> {
    let why = if !caps.addressable {
        "this terminal cannot place the cursor (TERM is unset or `dumb`), so a full-screen view would only print garbage; use".to_owned()
    } else if let Some(size) = size {
        if size.holds(min) {
            return None;
        }
        format!(
            "this view needs at least {} × {} cells, the terminal has {} × {}; enlarge the window or use",
            min.cols, min.rows, size.cols, size.rows
        )
    } else {
        format!(
            "the terminal reports no size (0 × 0, or the size cannot be read), and this view needs at least {} × {} cells; set it with `stty rows {} cols {}` or use",
            min.cols, min.rows, min.rows, min.cols
        )
    };
    Some(
        Diagnostic::builder(codes::TERM_003, Severity::Error)
            .why(format!(
                "{why} `{without_tui}`, which needs no full-screen view"
            ))
            .fix(FixAction::CopyCommand(without_tui.to_owned()))
            .build(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::FixAction;
    use humanitl_core::diagnostics::codes;

    use super::{Capabilities, Cells, unsuitable};

    const MIN: Cells = Cells::new(60, 16);

    /// Zu klein in einer Richtung reicht für `TERM_003`; der Text nennt das
    /// Minimum, das gemessene Maß und den Weg ohne Bildschirm.
    #[test]
    fn too_small_is_term_003_with_the_minimum_and_the_way_without() {
        let caps = Capabilities::from_term(Some("xterm-256color"));
        for size in [Cells::new(59, 40), Cells::new(200, 15)] {
            let found = unsuitable(caps, Some(size), MIN, "humanitl config show")
                .expect("too small is a finding");
            assert_eq!(found.code, codes::TERM_003);
            let why = &found.why;
            assert!(why.contains("60 × 16"), "{why}");
            assert!(
                why.contains(&format!("{} × {}", size.cols, size.rows)),
                "{why}"
            );
            assert!(why.contains("humanitl config show"), "{why}");
            assert_eq!(
                found.fix,
                Some(FixAction::CopyCommand("humanitl config show".to_owned()))
            );
        }
        assert!(
            unsuitable(caps, Some(MIN), MIN, "x").is_none(),
            "exactly the minimum fits"
        );
    }

    /// Ohne `TERM` oder mit `TERM=dumb` gibt es keinen Bildschirm, gleich
    /// wie groß das Fenster ist.
    #[test]
    fn a_dumb_terminal_gets_no_screen() {
        for term in [None, Some(""), Some("dumb")] {
            let caps = Capabilities::from_term(term);
            assert!(!caps.addressable, "{term:?}");
            let found = unsuitable(caps, Some(Cells::new(200, 60)), MIN, "x").expect("a finding");
            assert_eq!(found.code, codes::TERM_003);
        }
    }

    /// Nur bekannte Familien bekommen den alternativen Bildschirm; die
    /// Konsole von Linux, alte DEC-Terminals und jeder unbekannte Name
    /// zeichnen ohne und löschen am Ende.
    #[test]
    fn the_alternate_screen_only_for_known_terminals() {
        for term in [
            "xterm",
            "xterm-256color",
            "xterm-kitty",
            "xterm-ghostty",
            "screen.xterm-256color",
            "screen-256color",
            "tmux-256color",
            "rxvt-unicode-256color",
            "alacritty",
            "foot",
            "wezterm",
            "vte-256color",
            "konsole-256color",
            "st-256color",
            "contour",
        ] {
            let caps = Capabilities::from_term(Some(term));
            assert!(caps.alternate_screen, "{term}");
        }
        for term in [
            "linux", "vt100", "vt220", "vt320", "ansi", "cons25", "stterm", "xtermish", "unknown",
        ] {
            let caps = Capabilities::from_term(Some(term));
            assert!(caps.addressable, "{term} can still place the cursor");
            assert!(!caps.alternate_screen, "{term}");
        }
        assert!(
            Capabilities::from_env(Some("nsterm-16color"), Some("iTerm.app")).alternate_screen,
            "TERM_PROGRAM names a terminal that knows it"
        );
        assert!(!Capabilities::from_env(Some("vt320"), Some("other")).alternate_screen);
        assert!(!Capabilities::from_env(Some("dumb"), Some("iTerm.app")).alternate_screen);
    }

    /// Ein Terminal ohne Größe ist `TERM_003` mit `stty` und dem Weg ohne
    /// Bildschirm, nicht 80 × 24.
    #[test]
    fn no_reported_size_is_term_003_with_stty() {
        let caps = Capabilities::from_term(Some("xterm"));
        let found = unsuitable(caps, None, MIN, "humanitl config show").expect("a finding");
        assert_eq!(found.code, codes::TERM_003);
        assert!(found.why.contains("reports no size"), "{}", found.why);
        assert!(found.why.contains("stty rows 16 cols 60"), "{}", found.why);
        assert!(found.why.contains("humanitl config show"), "{}", found.why);
    }

    /// Eine Größe über `u16` wird gekappt, nicht umgebrochen.
    #[test]
    fn huge_windows_saturate() {
        assert_eq!(Cells::from_window((70_000, 24)), Cells::new(u16::MAX, 24));
    }
}
