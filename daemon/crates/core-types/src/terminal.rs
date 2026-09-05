//! Der Filter für Bytes, die aus der Sandbox in ein Terminal laufen.
//!
//! Die Ausgabe des Agenten ist einer der fünf erklärten Seitenkanäle
//! (`BACKLOG.md` 4.2): Sie erreicht das Terminal eines Menschen, und ein
//! Terminal führt aus, was in ihr steht.
//!
//! # Die Regel
//!
//! **Von allen Steuerfolgen verlässt genau eine den Daemon: `ESC [ … m`, die
//! Farb- und Attributfolge (SGR).** Alles andere wird verworfen — jede
//! OSC-Folge, jede Zeichenkettenfolge (DCS, SOS, PM, APC), jede andere
//! CSI-Folge und jede Ein-Zeichen-Escape-Folge.
//!
//! Das ist eine Erlaubnisliste, und zwar aus demselben Grund wie bei
//! `VISIBLE_ENV` und `SESSION_OVERRIDE_KEYS`: Über einem offenen Namensraum
//! kann keine Sperrliste vollständig sein, und ihre Lücken sind genau die
//! gefährlichen. Eine Liste „OSC 52 und OSC 8 sind verboten" übersieht
//! `OSC 052` (Terminals lesen die Nummer als Zahl), `OSC 0` (setzt den
//! Fenstertitel), `\x9d` (dieselbe Folge in einem Byte) und
//! `ESC P tmux; …` (reicht die verbotene Folge durch tmux hindurch). Jede
//! dieser vier Lücken war in der ersten Fassung dieses Filters offen.
//!
//! Was der Agent danach noch kann: schreiben, färben, und mit `\r`, `\t` und
//! `\x08` die Zeile umschreiben, auf der er gerade steht. Was er nicht mehr
//! kann: den Cursor bewegen, löschen, scrollen, das Terminal zurücksetzen, den
//! Fenstertitel ändern, in die Zwischenablage schreiben oder einen Verweis
//! unter sichtbaren Text legen. **Er kann damit keine Zeile mehr überschreiben,
//! die schon steht** — also auch keine der drei Zeilen, mit denen `humanitl
//! run` die Isolationsprüfungen meldet.
//!
//! # Warum die Bytes roh sind, und was das bedeutet
//!
//! Gemessen, nicht angenommen: Die Ausgabe reist als `bytes` über die Leitung
//! (`SandboxEvent.OutputChunk.data`, in Rust `Vec<u8>`), wird in
//! `humanitl_sandbox::handle::Shared::tee` unverändert aus der Pipe kopiert
//! und in `humanitl::cmd::run::write_output` mit `write_all` unverändert in
//! das Terminal geschrieben. Auf dem ganzen Weg steht keine UTF-8-Prüfung.
//!
//! Deshalb genügt es nicht, auf `ESC` (`0x1b`) zu achten. Die C1-Steuerzeichen
//! `0x80..=0x9f` sind dieselben Folgen in einem Byte: `0x9b` ist CSI, `0x9d`
//! ist OSC, `0x90` ist DCS. Sie werden hier genauso behandelt wie ihre
//! zweibytigen Formen.
//!
//! # Warum der Filter UTF-8 mitzählt
//!
//! `0x80..=0x9f` sind zugleich Folgebytes von UTF-8. `€` ist
//! `E2 82 AC` und enthält `0x82`; `⛛` enthält `0x9b`. Ein Filter, der jedes
//! Byte dieses Bereichs als Steuerzeichen läse, zerschnitte jeden nicht-
//! englischen Text. Der Filter zählt deshalb mit, wie viele Folgebytes ein
//! Anfangsbyte noch erwartet, und behandelt `0x80..=0x9f` nur dann als C1,
//! wenn gerade kein Folgebyte erwartet wird. Ein Anfangsbyte, dem kein
//! Folgebyte folgt, verwirft den Zähler und das Byte wird neu betrachtet —
//! sonst schmuggelte `C3 1B ]52;…` ein `ESC` an der Prüfung vorbei.
//!
//! **Was das nicht deckt:** Ein Terminal, das *nicht* in UTF-8 arbeitet, liest
//! das `0x9b` in `⛛` als CSI. Dagegen hilft nur, jedes Byte dieses Bereichs zu
//! verwerfen, und das hieße, keinen Text außerhalb von ASCII mehr zu zeigen.
//! Die Zeile steht so in `docs/SECURITY.md` 3.3.
//!
//! # Warum der Filter Zustand hat
//!
//! Die Bytes kommen in Stücken aus einer Pipe, und eine Folge darf über die
//! Grenze zweier Stücke laufen. Ein zustandsloser Filter über je ein Stück
//! sähe die Hälfte einer Folge und ließe sie durch; ein Angreifer müsste nur
//! an der richtigen Stelle schreiben.
//!
//! # Was er nicht ist
//!
//! Keine Terminal-Emulation. Wer den Rumpf einer Antwort ansehen will, nimmt
//! keinen Filter, sondern den Pager, der nicht druckbare Bytes als Hexzahlen
//! zeigt (HUM-042). Und HUM-042 erweitert diesen Filter für den PTY-Pfad, an
//! dem ein Vollbild-Agent den Cursor wirklich braucht — statt einen zweiten
//! daneben zu bauen.

/// Wie viele Bytes einer angefangenen Folge höchstens zurückgehalten werden.
///
/// Eine Folge, die länger ist, ist keine, die noch jemand liest; sie wird
/// verworfen und der Strom läuft weiter. Ohne diese Schranke hielte ein `\x1b]`
/// ohne Ende die ganze weitere Ausgabe zurück, und der Agent bestimmte, wann
/// der Mensch etwas sieht.
pub const MAX_PENDING: usize = 4096;

/// `ESC`.
const ESC: u8 = 0x1b;

/// `BEL`, der übliche Abschluss einer Zeichenkettenfolge.
const BEL: u8 = 0x07;

/// `ST` als ein Byte (String Terminator, C1).
const ST_C1: u8 = 0x9c;

/// `CSI` als ein Byte (C1).
const CSI_C1: u8 = 0x9b;

/// `OSC` als ein Byte (C1).
const OSC_C1: u8 = 0x9d;

/// Das einzige Anfangsbyte von UTF-8, aus dem ein C1-Steuerzeichen werden kann.
///
/// `C2 80` bis `C2 9F` sind `U+0080` bis `U+009F`, also genau der C1-Bereich.
/// Jedes andere Anfangsbyte erzeugt einen Codepunkt darüber.
const UTF8_C2: u8 = 0xc2;

/// Die C1-Einleiter, die eine Zeichenkettenfolge beginnen: DCS, SOS, OSC, PM,
/// APC.
///
/// Alle fünf tragen eine beliebig lange Nutzlast bis zum Abschluss, und alle
/// fünf werden verworfen. `ESC P tmux;` ist der Grund, warum DCS dazugehört:
/// tmux reicht darin eingebettete Folgen an das äußere Terminal weiter, und
/// eine verbotene OSC-Folge käme so doch an.
const STRING_INTRODUCERS_C1: [u8; 5] = [0x90, 0x98, OSC_C1, 0x9e, 0x9f];

/// Dieselben Einleiter in ihrer zweibytigen Form, als Zeichen nach `ESC`.
const STRING_INTRODUCERS_ESC: [u8; 5] = [b'P', b'X', b']', b'^', b'_'];

/// Das einzige Endzeichen einer CSI-Folge, die hinausgehen darf: SGR.
///
/// `ESC [ … m` setzt Farbe und Attribute. Es bewegt nichts, löscht nichts und
/// verlässt die Zeile nicht.
const SGR_FINAL: u8 = b'm';

/// Was der Filter gerade sieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Gewöhnliche Ausgabe.
    Text,
    /// Ein `ESC` kam, die Art der Folge ist noch offen.
    Escape,
    /// Eine Zeichenkettenfolge (OSC, DCS, SOS, PM, APC) läuft. Alles darin
    /// wird verworfen; gezählt wird nur, wie lang sie schon ist.
    StringSeq,
    /// Eine CSI-Folge läuft. Sie wird zurückgehalten, bis ihr Endzeichen sagt,
    /// ob sie hinausgeht.
    Csi,
    /// Ein `0xC2` kam. Es ist das einzige Anfangsbyte, aus dem ein
    /// C1-Steuerzeichen werden kann, und wird deshalb zurückgehalten, bis das
    /// Folgebyte den Codepunkt entscheidet.
    Utf8C2,
}

/// Entfernt jede Steuerfolge außer SGR aus einem Strom von Terminal-Bytes.
///
/// Ein Filter gehört zu genau einem Strom; stdout und stderr bekommen je
/// einen, weil ihre Folgen sich sonst gegenseitig zerschnitten.
///
/// ```
/// use humanitl_core::terminal::TerminalFilter;
///
/// let mut filter = TerminalFilter::new();
/// let out = filter.push(b"hello \x1b]52;c;c2VjcmV0\x07world");
///
/// assert_eq!(out, b"hello world");
/// ```
#[derive(Debug)]
pub struct TerminalFilter {
    mode: Mode,
    /// Die zurückgehaltenen Bytes einer CSI-Folge, ohne `ESC [`.
    pending: Vec<u8>,
    /// Wie lang die laufende Zeichenkettenfolge schon ist.
    dropped: usize,
    /// Ob das letzte Byte einer Zeichenkettenfolge ein `ESC` war; nur dann
    /// beendet ein `\` sie.
    escaped: bool,
    /// Wie viele Folgebytes das laufende UTF-8-Zeichen noch erwartet.
    continuation: u8,
}

impl Default for TerminalFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalFilter {
    /// Ein frischer Filter am Anfang eines Stroms.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: Mode::Text,
            pending: Vec::new(),
            dropped: 0,
            escaped: false,
            continuation: 0,
        }
    }

    /// Reicht ein Stück Ausgabe durch und gibt zurück, was hinausgehen darf.
    ///
    /// Eine angefangene Folge bleibt hier und wird mit dem nächsten Aufruf
    /// entschieden.
    #[must_use]
    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(chunk.len());
        for &byte in chunk {
            self.step(byte, &mut out);
        }
        out
    }

    /// Beendet den Strom und gibt heraus, was noch hinausgehen darf: nichts.
    ///
    /// Eine Folge ohne Abschluss geht nie hinaus. Ein abschließendes `ESC`,
    /// `ESC [` oder `ESC ] 5` ließe das Terminal im Escape-Zustand zurück; es
    /// verschluckte dann die nächste Eingabe des Menschen, und die kommt nicht
    /// mehr vom Agenten.
    #[must_use]
    pub fn flush(&mut self) -> Vec<u8> {
        self.reset();
        Vec::new()
    }

    /// Zurück in den Textzustand, ohne etwas herauszugeben.
    fn reset(&mut self) {
        self.mode = Mode::Text;
        self.pending.clear();
        self.dropped = 0;
        self.escaped = false;
        self.continuation = 0;
    }

    /// Ein Byte, und was es mit dem Zustand macht.
    fn step(&mut self, byte: u8, out: &mut Vec<u8>) {
        match self.mode {
            Mode::Text => self.in_text(byte, out),
            Mode::Escape => self.after_escape(byte),
            Mode::StringSeq => self.in_string(byte),
            Mode::Csi => self.in_csi(byte, out),
            Mode::Utf8C2 => self.after_c2(byte, out),
        }
    }

    /// Gewöhnliche Ausgabe, mit UTF-8 im Blick.
    fn in_text(&mut self, byte: u8, out: &mut Vec<u8>) {
        if self.continuation > 0 {
            if is_continuation(byte) {
                self.continuation -= 1;
                out.push(byte);
                return;
            }
            // Ein Anfangsbyte ohne sein Folgebyte. Der Zähler ist wertlos, und
            // das Byte wird neu betrachtet: Sonst schmuggelte `C3 1B` ein
            // `ESC` an der Prüfung vorbei.
            self.continuation = 0;
        }
        match byte {
            ESC => self.mode = Mode::Escape,
            CSI_C1 => {
                self.mode = Mode::Csi;
                self.pending.clear();
            }
            byte if STRING_INTRODUCERS_C1.contains(&byte) => {
                self.mode = Mode::StringSeq;
                self.dropped = 0;
                self.escaped = false;
            }
            // Jedes andere C1-Steuerzeichen: verworfen. Keines von ihnen zeigt
            // etwas an, und mehrere bewegen den Cursor (`0x84` IND, `0x8d` RI).
            0x80..=0x9f => {}
            // `0xC2` ist das einzige Anfangsbyte, aus dem `U+0080` bis
            // `U+00BF` werden kann — und damit das einzige, das ein
            // C1-Steuerzeichen tragen kann. Es wird zurückgehalten, bis das
            // Folgebyte entscheidet.
            UTF8_C2 => self.mode = Mode::Utf8C2,
            // Jedes andere Anfangsbyte von UTF-8; die Länge steht in seinen
            // oberen Bits, und keiner seiner Codepunkte liegt in C1.
            0xc3..=0xdf => {
                self.continuation = 1;
                out.push(byte);
            }
            0xe0..=0xef => {
                self.continuation = 2;
                out.push(byte);
            }
            0xf0..=0xf4 => {
                self.continuation = 3;
                out.push(byte);
            }
            _ => out.push(byte),
        }
    }

    /// Nach einem `0xC2`: entscheidet das Folgebyte über den Codepunkt.
    ///
    /// `C2 9D` ist die wohlgeformte Kodierung von `U+009D`, also OSC; `C2 9B`
    /// ist `U+009B`, also CSI. Ein Terminal, das UTF-8 vor dem Parser
    /// dekodiert — VTE und damit GNOME Terminal, Tilix, Terminator, XFCE
    /// Terminal, Guake, und xterm mit der Vorgabe `allowC1Printable: off` —
    /// führt sie aus. Deshalb entscheidet hier der Codepunkt und nicht das
    /// einzelne Byte.
    ///
    /// `C2 A0` bis `C2 BF` sind druckbare Zeichen (das erste ist das geschützte
    /// Leerzeichen) und gehen unverändert hinaus. Ein Folgebyte, das keines
    /// ist, macht die Kodierung ungültig: Das `0xC2` fällt weg, und das Byte
    /// wird neu betrachtet.
    fn after_c2(&mut self, byte: u8, out: &mut Vec<u8>) {
        self.mode = Mode::Text;
        // Ein druckbarer Codepunkt (`C2 A0` ist das geschützte Leerzeichen);
        // die Kodierung geht vollständig hinaus.
        if (0xa0..=0xbf).contains(&byte) {
            out.push(UTF8_C2);
            out.push(byte);
            return;
        }
        // Sonst zweierlei, und beides endet gleich: Im C1-Bereich
        // (`0x80..=0x9f`) ist der Codepunkt ein Steuerzeichen und wird wie das
        // rohe Byte behandelt; alles andere ist keine gültige Kodierung. In
        // beiden Fällen fällt das `0xC2` weg und das Byte wird neu betrachtet
        // — sonst schmuggelte `C2 1B` ein `ESC` an der Prüfung vorbei.
        self.in_text(byte, out);
    }

    /// Nach einem `ESC`: was für eine Folge beginnt.
    fn after_escape(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.mode = Mode::Csi;
                self.pending.clear();
            }
            byte if STRING_INTRODUCERS_ESC.contains(&byte) => {
                self.mode = Mode::StringSeq;
                self.dropped = 0;
                self.escaped = false;
            }
            // Ein zweites `ESC`: die vorige Folge war keine, die neue beginnt.
            ESC => {}
            // Jede andere Escape-Folge ist eine Ein-Zeichen-Folge und geht
            // nicht hinaus: `ESC c` setzt das Terminal zurück, `ESC 7`/`ESC 8`
            // merken und stellen den Cursor wieder her, `ESC M` fährt eine
            // Zeile hoch. Auch die harmlosen darunter bleiben hier — eine
            // Erlaubnisliste zählt auf, was hinaus darf, nicht was nicht.
            _ => self.mode = Mode::Text,
        }
    }

    /// Innerhalb einer Zeichenkettenfolge: alles verwerfen, bis sie endet.
    fn in_string(&mut self, byte: u8) {
        self.dropped += 1;
        // `BEL` beendet sie, `ESC \` ebenfalls (ST), und `0x9c` ist dasselbe
        // ST in einem Byte. Ein `\` allein beendet nichts: In der Nutzlast
        // steht es in jedem Windows-Pfad und in jedem regulären Ausdruck.
        if byte == BEL || byte == ST_C1 || (byte == b'\\' && self.escaped) {
            self.reset();
            return;
        }
        self.escaped = byte == ESC;
        if self.dropped >= MAX_PENDING {
            // Kein Ende in Sicht. Verworfen bleibt verworfen, aber der Strom
            // läuft weiter: Sonst hielte ein `\x1b]` ohne Abschluss die ganze
            // weitere Ausgabe an.
            self.reset();
        }
    }

    /// Innerhalb einer CSI-Folge: sammeln, bis das Endzeichen entscheidet.
    fn in_csi(&mut self, byte: u8, out: &mut Vec<u8>) {
        // Parameter- und Zwischenbytes einer CSI-Folge nach ECMA-48.
        if (0x20..=0x3f).contains(&byte) {
            self.pending.push(byte);
            if self.pending.len() >= MAX_PENDING {
                self.reset();
            }
            return;
        }
        if (0x40..=0x7e).contains(&byte) {
            let sgr = byte == SGR_FINAL;
            let params = std::mem::take(&mut self.pending);
            self.reset();
            if sgr {
                out.push(ESC);
                out.push(b'[');
                out.extend_from_slice(&params);
                out.push(byte);
            }
            return;
        }
        // Weder Parameter noch Endzeichen: Die Folge ist keine. Sie wird
        // verworfen, und das Byte wird neu betrachtet — ein `ESC` mitten in
        // einer CSI-Folge beginnt die nächste.
        self.reset();
        self.in_text(byte, out);
    }
}

/// Ob dieses Byte ein Folgebyte von UTF-8 ist.
const fn is_continuation(byte: u8) -> bool {
    byte & 0b1100_0000 == 0b1000_0000
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{MAX_PENDING, TerminalFilter};

    fn filtered(chunks: &[&[u8]]) -> Vec<u8> {
        let mut filter = TerminalFilter::new();
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(filter.push(chunk));
        }
        out.extend(filter.flush());
        out
    }

    fn text(chunks: &[&[u8]]) -> String {
        String::from_utf8_lossy(&filtered(chunks)).into_owned()
    }

    #[test]
    fn ordinary_output_passes_through_unchanged() {
        let plain = b"line one\nline two\r\tand back\x08";
        assert_eq!(filtered(&[plain]), plain.to_vec());
    }

    #[test]
    fn colours_are_the_one_sequence_that_leaves() {
        let coloured = b"\x1b[31mred\x1b[0m and \x1b[1;32mgreen\x1b[m";
        assert_eq!(filtered(&[coloured]), coloured.to_vec());
    }

    // --- Die vier Lücken der ersten Fassung, je ein Test ------------------

    /// Blockierend 1: dieselbe Folge in einem Byte.
    ///
    /// `0x9d` ist OSC, `0x9b` ist CSI. Die erste Fassung sah nur `ESC` und
    /// ließ beide vollständig durch.
    #[test]
    fn the_one_byte_c1_forms_are_filtered_too() {
        assert_eq!(text(&[b"a\x9d52;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(
            text(&[b"a\x9d8;;https://evil.example/\x07c\x9d8;;\x07b"]),
            "acb"
        );
        assert_eq!(text(&[b"a\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\x9b1;1Hb"]), "ab");
        // Und die C1-Folge, die den Cursor allein bewegt.
        assert_eq!(text(&[b"a\x84b\x8dc"]), "abc");
    }

    /// Blockierend 2: führende Nullen in der Nummer.
    ///
    /// Terminals lesen die Nummer als Zahl, `052` ist `52`. Die Erlaubnisliste
    /// macht die Frage gegenstandslos: Es geht gar keine OSC-Folge hinaus.
    #[test]
    fn a_leading_zero_does_not_hide_the_number() {
        assert_eq!(text(&[b"a\x1b]052;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\x1b]08;;https://evil.example/\x07b"]), "ab");
        assert_eq!(text(&[b"a\x1b]0000000052;c;x\x07b"]), "ab");
    }

    /// Schwer 3: ein einzelner Backslash beendet nichts.
    ///
    /// In der Nutzlast einer verworfenen Folge steht er in jedem Windows-Pfad.
    /// Endete die Folge dort, liefe ihr Rest ungefiltert ins Terminal.
    #[test]
    fn a_lone_backslash_does_not_end_a_string_sequence() {
        assert_eq!(
            text(&[b"a\x1b]52;c;C:\\Users\\x\x1b[31mstill inside\x07b"]),
            "ab",
            "everything up to the BEL belongs to the sequence"
        );
        // `ESC \` beendet sie sehr wohl.
        assert_eq!(text(&[b"a\x1b]52;c;x\x1b\\b"]), "ab");
        // Und `0x9c`, dasselbe ST in einem Byte.
        assert_eq!(text(&[b"a\x1b]52;c;x\x9cb"]), "ab");
    }

    /// Schwer 4: Cursorbewegung und Löschen gehen nicht hinaus.
    ///
    /// `\x1b[1A\x1b[2K` überschreibt eine Zeile, die schon steht — zum
    /// Beispiel eine der drei Zeilen, mit denen `humanitl run` die
    /// Isolationsprüfungen meldet.
    #[test]
    fn nothing_that_moves_or_erases_leaves() {
        for attack in [
            &b"\x1b[1A"[..],
            b"\x1b[2K",
            b"\x1b[H",
            b"\x1b[2J",
            b"\x1b[10;5f",
            b"\x1b[3d",
            b"\x1b[1S",
            b"\x1b[1L",
            b"\x1b[u",
            b"\x1b[?1049h",
            b"\x1b[?25l",
        ] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(attack);
            payload.push(b'b');
            assert_eq!(
                text(&[&payload]),
                "ab",
                "{:?} must not reach the terminal",
                String::from_utf8_lossy(attack)
            );
        }
    }

    /// Klein 5: eine abgeschnittene Folge geht am Stromende nicht hinaus.
    ///
    /// Sonst bliebe das Terminal im Escape-Zustand und verschluckte die
    /// nächste Eingabe des Menschen.
    #[test]
    fn a_truncated_sequence_is_dropped_at_the_end_of_the_stream() {
        for tail in [&b"\x1b"[..], b"\x1b]", b"\x1b]5", b"\x1b[", b"\x1b[31"] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(tail);
            assert_eq!(
                text(&[&payload]),
                "a",
                "{:?} must not be released",
                String::from_utf8_lossy(tail)
            );
        }
    }

    /// Klein 6: der Fenstertitel gehört dem Menschen.
    #[test]
    fn the_window_title_cannot_be_set() {
        assert_eq!(text(&[b"a\x1b]0;All Checks Passed\x07b"]), "ab");
        assert_eq!(text(&[b"a\x1b]2;All Checks Passed\x07b"]), "ab");
    }

    /// `ESC P tmux;` reicht eine verbotene Folge durch tmux an das äußere
    /// Terminal weiter. Deshalb ist DCS eine Zeichenkettenfolge wie OSC.
    #[test]
    fn the_tmux_passthrough_is_a_string_sequence_too() {
        assert_eq!(
            text(&[b"a\x1bPtmux;\x1b\x1b]52;c;c2VjcmV0\x07\x1b\\b"]),
            "ab"
        );
    }

    /// Die Ein-Zeichen-Folgen, die den Cursor bewegen oder das Terminal
    /// zurücksetzen.
    #[test]
    fn single_character_escapes_do_not_leave() {
        for attack in [
            &b"\x1bc"[..], // RIS, setzt das Terminal zurück
            b"\x1b7",      // DECSC
            b"\x1b8",      // DECRC
            b"\x1bM",      // RI, eine Zeile hoch
            b"\x1bD",
            b"\x1bE",
        ] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(attack);
            payload.push(b'b');
            assert_eq!(text(&[&payload]), "ab", "{attack:?}");
        }
    }

    // --- UTF-8 ------------------------------------------------------------

    /// Text außerhalb von ASCII überlebt, auch wenn seine Bytes im C1-Bereich
    /// liegen.
    #[test]
    fn utf8_survives_even_when_its_bytes_look_like_c1() {
        // `€` ist E2 82 AC und enthält 0x82; `⛛` ist E2 9B 9B und enthält 0x9b.
        for word in ["Größe", "€ 12,50", "⛛ Warnung", "日本語", "🙂"] {
            assert_eq!(text(&[word.as_bytes()]), word, "{word}");
        }
    }

    /// Ein Anfangsbyte ohne sein Folgebyte verwirft den Zähler; das Byte
    /// danach wird neu betrachtet.
    ///
    /// Ohne diese Regel schmuggelte `C3 1B ]52;…` ein `ESC` an der Prüfung
    /// vorbei.
    #[test]
    fn a_lead_byte_does_not_smuggle_an_escape() {
        // `C3` erwartet ein Folgebyte, `1B` ist keines: Der Zähler wird
        // verworfen und das `ESC` beginnt eine Folge, die nicht hinausgeht.
        assert_eq!(text(&[b"a\xc3\x1b]52;c;c2VjcmV0\x07b"]), "a\u{fffd}b");
        // `C3` und ein echtes Folgebyte bleiben ein Zeichen.
        assert_eq!(text(&[b"a\xc3\xa4b"]), "aäb");
    }

    /// Ein `0x9d` **innerhalb** eines wohlgeformten, druckbaren Zeichens ist
    /// kein OSC.
    ///
    /// `E2 82 9D` ist `U+209D`, ein druckbares Zeichen. Ein Terminal
    /// dekodiert es als solches; der Filter darf den Text deshalb nicht
    /// zerschneiden.
    #[test]
    fn a_c1_byte_inside_a_well_formed_character_stays_data() {
        assert_eq!(text(&[b"a\xe2\x82\x9db"]), "a\u{209d}b");
    }

    /// Blockierend 2 des zweiten Reviews: das C1-Steuerzeichen als
    /// wohlgeformtes UTF-8.
    ///
    /// `C2 9D` ist die Kodierung von `U+009D`, also OSC; `C2 9B` ist
    /// `U+009B`, also CSI. VTE-basierte Terminals — GNOME Terminal, Tilix,
    /// Terminator, XFCE Terminal, Guake — dekodieren UTF-8 **vor** dem Parser
    /// und führen beide aus; xterm ebenso, solange `allowC1Printable` aus ist,
    /// und das ist die Vorgabe. Ein Filter, der nur Folgebytes zählt, ließe
    /// sie durch.
    #[test]
    fn a_c1_control_encoded_as_utf8_is_filtered_too() {
        assert_eq!(text(&[b"a\xc2\x9d52;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\xc2\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\xc2\x9b10Ab"]), "ab");
        assert_eq!(text(&[b"a\xc2\x90tmux;x\x1b\\b"]), "ab");
        // Auch über Stückgrenzen hinweg.
        assert_eq!(text(&[b"a\xc2", b"\x9d52;c;x\x07b"]), "ab");
        // Und ein `ESC` hinter dem `0xC2` wird nicht durchgereicht.
        assert_eq!(text(&[b"a\xc2\x1b]52;c;x\x07b"]), "ab");
    }

    /// `0xC2` mit einem druckbaren Folgebyte bleibt unverändert.
    ///
    /// `C2 A0` ist das geschützte Leerzeichen; es steht in gewöhnlichem Text.
    #[test]
    fn c2_with_a_printable_continuation_passes() {
        assert_eq!(text(&[b"a\xc2\xa0b"]), "a\u{a0}b");
        assert_eq!(text(&[b"a\xc2\xb5b"]), "a\u{b5}b");
        // Auch über Stückgrenzen.
        assert_eq!(text(&[b"a\xc2", b"\xa0b"]), "a\u{a0}b");
    }

    // --- Robustheit --------------------------------------------------------

    #[test]
    fn a_sequence_split_across_chunks_is_still_removed() {
        // Der Angriff, den ein zustandsloser Filter durchließe.
        assert_eq!(text(&[b"a\x1b]5", b"2;c;c2Vj", b"cmV0\x07b"]), "ab");
        assert_eq!(
            text(&[b"a\x1b", b"[31m", b"red\x1b[0m"]),
            "a\x1b[31mred\x1b[0m"
        );
    }

    #[test]
    fn an_endless_sequence_does_not_hold_the_stream_hostage() {
        let mut long = Vec::from(*b"a\x1b]0;");
        long.extend(std::iter::repeat_n(b'x', MAX_PENDING + 10));
        long.push(b'b');
        let out = text(&[&long]);
        assert!(out.starts_with('a'), "{out:?}");
        assert!(out.ends_with('b'), "the stream goes on: {out:?}");
        assert!(
            out.len() < MAX_PENDING,
            "and the payload of the sequence stays inside: {} bytes",
            out.len()
        );
    }

    #[test]
    fn an_endless_csi_does_not_hold_the_stream_hostage() {
        let mut long = Vec::from(*b"a\x1b[");
        long.extend(std::iter::repeat_n(b'1', MAX_PENDING + 10));
        long.push(b'b');
        let out = text(&[&long]);
        assert!(out.starts_with('a'), "{out:?}");
        assert!(out.ends_with('b'), "{out:?}");
    }
}
