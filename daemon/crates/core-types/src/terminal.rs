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
//! Das gilt für einen Strom ohne PTY ([`TerminalPolicy::ColourOnly`]). Am PTY
//! braucht ein Vollbild-Agent mehr, und dafür gibt es die zweite Politik
//! ([`TerminalPolicy::FullScreen`], HUM-042). Beide sind derselbe
//! Zustandsautomat mit derselben Behandlung von C1 und UTF-8; ein zweiter
//! Filter daneben liefe an genau dieser Stelle auseinander
//! (`backlog/CONVENTIONS.md` 4.26).
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
//! zeigt (HUM-042).
//!
//! # Die zweite Politik: ein Vollbild-Agent am PTY
//!
//! [`TerminalPolicy::FullScreen`] ist die Politik des Terminal-Stroms
//! (HUM-042). Ein Vollbild-TUI zeichnet mit absoluter Adressierung; ohne
//! Cursorbewegung, Löschen, Scrollbereiche, Alternativschirm und
//! Mausverfolgung ist es nicht bedienbar. Diese Politik lässt deshalb jede
//! CSI-Folge hinaus und jede Escape-Folge außer `ESC c` (RIS).
//!
//! Was auch dort nicht hinausgeht, und warum:
//!
//! - **Jede Zeichenkettenfolge außer einer kurzen Liste von OSC-Nummern**
//!   ([`OSC_ALLOWED`]). DCS, SOS, PM und APC fallen vollständig weg: `ESC P
//!   tmux;` reicht eine verbotene Folge durch tmux an das äußere Terminal
//!   weiter, kitty-Grafik und das Dateiprotokoll von iTerm2 reisen über APC.
//!   Erlaubt bleiben Farbe (OSC 4, 10, 11 und ihre Rücknahmen 104, 110, 111)
//!   und die Prompt-Marken (OSC 133). Nicht erlaubt sind unter anderem OSC 52
//!   (Zwischenablage des Menschen), OSC 8 (Verweis unter sichtbarem Text),
//!   OSC 0/1/2 (Fenstertitel), OSC 7 (Arbeitsverzeichnis), OSC 9, 99 und 777
//!   (Benachrichtigungen des Systems) und OSC 1337 (Dateiprotokoll).
//! - **Die Nutzlast einer erlaubten OSC-Folge ist druckbares ASCII.** Ein
//!   `ESC` darin ist keine Farbe, sondern eine zweite Folge im Bauch der
//!   ersten: Terminals brechen die äußere Folge daran ab und führen die innere
//!   aus. Eine Nutzlast mit einem Byte außerhalb von `0x20..=0x7e` lässt
//!   deshalb die ganze Folge wegfallen.
//! - **`CSI … t` (XTWINOPS).** Das ist keine Ausgabe, sondern Fenstersteuerung:
//!   `CSI 21 t` lässt das Terminal seinen Titel in die Eingabe des Agenten
//!   schreiben, `CSI 22 t`/`CSI 23 t` legen einen Titel ab und stellen ihn
//!   wieder her — der Fenstertitel also, an OSC 0 vorbei —, und `CSI 8;h;w t`
//!   ändert die Größe des Fensters.
//! - **`ESC c` (RIS).** Es setzt das Terminal zurück, samt Rollpuffer und
//!   Farben; danach steht keine Zeile mehr, die vorher stand.
//! - **C1-Steuerzeichen in ihrer Ein-Byte-Form und als UTF-8**, wie bei der
//!   ersten Politik: Ein Agent, der eine Folge schickt, schickt sie mit `ESC`;
//!   terminfo für `xterm-256color` kennt nichts anderes.
//!
//! Was ein Vollbild-Agent damit *kann*: den Schirm beschreiben. Was er nicht
//! kann: aus dem Schirm heraus in die Zwischenablage, in die Fensterleiste, in
//! die Benachrichtigungen oder in die Eingabe schreiben.

/// Wie viele Bytes einer angefangenen Folge höchstens zurückgehalten werden.
///
/// Eine Folge, die länger ist, ist keine, die noch jemand liest; sie wird
/// verworfen und der Strom läuft weiter. Ohne diese Schranke hielte ein `\x1b]`
/// ohne Ende die ganze weitere Ausgabe zurück, und der Agent bestimmte, wann
/// der Mensch etwas sieht.
pub const MAX_PENDING: usize = 4096;

/// Dieselbe Schranke für eine Zeichenkettenfolge, die hinausgehen darf.
///
/// Sie ist größer, weil eine erlaubte Folge wirklich lang sein darf: `OSC 4`
/// setzt die Palette und trägt dafür bis zu 256 Farbangaben in einer Folge.
/// Sie gilt erst, wenn die Nummer entschieden ist; eine verbotene Folge wird
/// nach [`MAX_PENDING`] verworfen, ohne je gepuffert zu werden.
pub const MAX_STRING_PENDING: usize = 64 * 1024;

/// Die OSC-Nummern, die [`TerminalPolicy::FullScreen`] hinauslässt.
///
/// Farbe und ihre Rücknahme (4, 10, 11, 104, 110, 111) und die Prompt-Marken
/// (133). Die Liste ist eine Erlaubnisliste und nicht eine Sperrliste, und
/// zwar aus demselben Grund wie bei `VISIBLE_ENV` und `SESSION_OVERRIDE_KEYS`:
/// Über einem offenen Namensraum kann keine Sperrliste vollständig sein. Eine
/// Sperrliste aus `[0, 1, 2, 7, 8, 9, 52, 777, 1337]` ließe zum Beispiel
/// OSC 99 (Benachrichtigung in kitty), OSC 12 (Cursorfarbe) und jede Nummer
/// durch, die ein Terminal morgen belegt.
pub const OSC_ALLOWED: [u32; 7] = [4, 10, 11, 104, 110, 111, 133];

/// Wie viel eine Politik hinauslässt.
///
/// Beide Politiken sind derselbe Zustandsautomat; sie unterscheiden sich nur
/// darin, was am Ende einer erkannten Folge geschieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalPolicy {
    /// Nur Farbe: `ESC [ … m` und sonst keine Steuerfolge.
    ///
    /// Die Politik eines Stroms ohne PTY (`humanitl run`, `SandboxEvent`).
    /// Der Agent kann damit keine Zeile überschreiben, die schon steht — auch
    /// keine der drei Zeilen, mit denen die Isolationsprüfung gemeldet wird.
    #[default]
    ColourOnly,
    /// Was ein Vollbild-Agent an einem PTY braucht; siehe Modulbeschreibung.
    FullScreen,
}

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

/// Das eine Endzeichen, das auch [`TerminalPolicy::FullScreen`] nicht
/// hinauslässt: XTWINOPS, siehe Modulbeschreibung.
const WINDOW_OPS_FINAL: u8 = b't';

/// Das eine Zeichen nach `ESC`, das auch [`TerminalPolicy::FullScreen`] nicht
/// hinauslässt: RIS, das Zurücksetzen des Terminals.
const RIS_FINAL: u8 = b'c';

/// Wie viele Ziffern die Nummer einer OSC-Folge höchstens hat.
///
/// `1337` ist die längste, die je vergeben wurde; fünf Ziffern lassen Luft und
/// begrenzen zugleich, wie lange die Entscheidung offenbleibt. Führende Nullen
/// zählen mit, weil ein Terminal die Nummer als Zahl liest: `052` ist `52`.
const MAX_OSC_DIGITS: u8 = 5;

/// Was der Filter gerade sieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Gewöhnliche Ausgabe.
    Text,
    /// Ein `ESC` kam, die Art der Folge ist noch offen.
    Escape,
    /// Nach `ESC` kam ein Zwischenbyte (`0x20..=0x2f`): `ESC ( B` wählt einen
    /// Zeichensatz, `ESC # 8` füllt den Schirm. Die Folge endet erst mit
    /// ihrem Endbyte; ohne diesen Zustand bliebe das Endbyte als Buchstabe
    /// stehen.
    EscapeIntermediate,
    /// Eine Zeichenkettenfolge (OSC, DCS, SOS, PM, APC) läuft.
    StringSeq,
    /// Eine CSI-Folge läuft. Sie wird zurückgehalten, bis ihr Endzeichen sagt,
    /// ob sie hinausgeht.
    Csi,
    /// Ein Anfangsbyte eines Mehrbytezeichens kam. Das Zeichen wird
    /// zusammengehalten, bis es vollständig ist und die Prüfung auf die
    /// kürzeste Kodierung besteht; erst dann steht fest, ob es ein Zeichen
    /// ist oder ein verstecktes Steuerzeichen.
    Utf8,
}

/// Womit eine Zeichenkettenfolge geendet hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Terminator {
    /// `BEL`.
    Bel,
    /// `ST`, in welcher Schreibweise auch immer; hinaus geht immer `ESC \`.
    St,
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
    /// Wie viel hinausgehen darf.
    policy: TerminalPolicy,
    mode: Mode,
    /// Die zurückgehaltenen Bytes der laufenden Folge, ohne ihren Einleiter:
    /// die Parameter einer CSI-Folge, die Zwischenbytes einer Escape-Folge
    /// oder die Nutzlast einer Zeichenkettenfolge, die hinausgehen darf.
    pending: Vec<u8>,
    /// Wie lang die laufende Zeichenkettenfolge schon ist.
    dropped: usize,
    /// Ob das letzte Byte einer Zeichenkettenfolge ein `ESC` war; nur dann
    /// beendet ein `\` sie.
    escaped: bool,
    /// Wie viele Folgebytes das laufende UTF-8-Zeichen noch erwartet.
    continuation: u8,
    /// Ob die laufende Folge mit einem C1-Byte begann statt mit `ESC`.
    ///
    /// Eine so eingeleitete Folge geht nie hinaus. Kein terminfo-Eintrag für
    /// `xterm-256color` erzeugt Acht-Bit-Steuerzeichen; wer sie schickt,
    /// verkleidet etwas.
    from_c1: bool,
    /// Was das nächste Byte dieses Zeichens mindestens sein muss.
    next_lo: u8,
    /// Und höchstens.
    next_hi: u8,
    /// Ob die laufende Zeichenkettenfolge hinausgehen darf. `None`, solange
    /// ihre Nummer noch gelesen wird.
    keep: Option<bool>,
    /// Die Nummer der laufenden OSC-Folge, solange sie gelesen wird.
    number: u32,
    /// Wie viele Ziffern davon schon da sind.
    digits: u8,
}

impl Default for TerminalFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalFilter {
    /// Ein frischer Filter am Anfang eines Stroms, mit
    /// [`TerminalPolicy::ColourOnly`].
    #[must_use]
    pub const fn new() -> Self {
        Self::with_policy(TerminalPolicy::ColourOnly)
    }

    /// Ein frischer Filter mit einer gewählten Politik.
    ///
    /// ```
    /// use humanitl_core::terminal::{TerminalFilter, TerminalPolicy};
    ///
    /// let mut filter = TerminalFilter::with_policy(TerminalPolicy::FullScreen);
    /// // Der Cursor darf sich bewegen, die Zwischenablage bleibt zu.
    /// assert_eq!(filter.push(b"\x1b[2J\x1b]52;c;c2VjcmV0\x07x"), b"\x1b[2Jx");
    /// ```
    #[must_use]
    pub const fn with_policy(policy: TerminalPolicy) -> Self {
        Self {
            policy,
            mode: Mode::Text,
            pending: Vec::new(),
            dropped: 0,
            escaped: false,
            continuation: 0,
            from_c1: false,
            next_lo: 0x80,
            next_hi: 0xbf,
            keep: None,
            number: 0,
            digits: 0,
        }
    }

    /// Wie viel dieser Filter hinauslässt.
    #[must_use]
    pub const fn policy(&self) -> TerminalPolicy {
        self.policy
    }

    /// Ob der Filter zwischen zwei Folgen und zwischen zwei Zeichen steht.
    ///
    /// Nur dann darf jemand eigene Bytes in den Strom schieben. Der Filter
    /// gibt zwar nie eine halbe Folge heraus — eine angefangene bleibt bei ihm
    /// —, wohl aber das erste Byte eines UTF-8-Zeichens; ein Hinweis
    /// dazwischen zerschnitte das Zeichen. Der Hinweis bei gehaltenem Fluss
    /// (HUM-042) fragt deshalb hier, bevor er schreibt.
    #[must_use]
    pub const fn at_boundary(&self) -> bool {
        matches!(self.mode, Mode::Text) && self.continuation == 0
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
        self.from_c1 = false;
        self.next_lo = 0x80;
        self.next_hi = 0xbf;
        self.keep = None;
        self.number = 0;
        self.digits = 0;
    }

    /// Ein Byte, und was es mit dem Zustand macht.
    fn step(&mut self, byte: u8, out: &mut Vec<u8>) {
        match self.mode {
            Mode::Text => self.in_text(byte, out),
            Mode::Escape => self.after_escape(byte, out),
            Mode::EscapeIntermediate => self.in_escape_intermediate(byte, out),
            Mode::StringSeq => self.in_string(byte, out),
            Mode::Csi => self.in_csi(byte, out),
            Mode::Utf8 => self.in_utf8(byte, out),
        }
    }

    /// Gewöhnliche Ausgabe, mit UTF-8 im Blick.
    fn in_text(&mut self, byte: u8, out: &mut Vec<u8>) {
        match byte {
            ESC => self.mode = Mode::Escape,
            CSI_C1 => {
                self.mode = Mode::Csi;
                self.pending.clear();
                self.from_c1 = true;
            }
            byte if STRING_INTRODUCERS_C1.contains(&byte) => {
                self.begin_string(byte == OSC_C1);
                self.from_c1 = true;
            }
            // Ein Anfangsbyte: Das Zeichen wird zusammengehalten, bis es
            // vollständig ist und die Prüfung besteht.
            0xc2..=0xf4 => self.begin_utf8(byte),
            // Alles Übrige über `0x7F` steht für sich und ist damit kein
            // Zeichen, aus drei Gründen:
            //
            // - `0x80..=0x9F` sind die restlichen C1-Steuerzeichen. Keines
            //   zeigt etwas an, und mehrere bewegen den Cursor (`0x84` IND,
            //   `0x8d` RI).
            // - `0xA0..=0xBF` sind Folgebytes ohne ihr Anfangsbyte.
            // - `0xC0`, `0xC1` und `0xF5..=0xFF` beginnen keine gültige
            //   Kodierung. `C0 9B` wäre die überlange Form von `U+001B`, also
            //   `ESC`; ein Dekoder, der sie annimmt, bekäme ihn hier.
            0x80..=0xc1 | 0xf5..=0xff => {}
            _ => out.push(byte),
        }
    }

    /// Beginnt ein Mehrbytezeichen und merkt sich, was das nächste Byte sein
    /// darf.
    ///
    /// Die Schranke für das **zweite** Byte ist die ganze Prüfung: Sie ist die
    /// einzige Stelle, an der sich eine überlange Form von der kürzesten
    /// unterscheidet. Nach ihr genügt `0x80..=0xBF`.
    fn begin_utf8(&mut self, lead: u8) {
        let (continuation, lo, hi) = match lead {
            0xc2..=0xdf => (1, 0x80, 0xbf),
            // `E0 80..9F` wären `U+0000` bis `U+07FF` in drei Bytes.
            0xe0 => (2, 0xa0, 0xbf),
            // `ED A0..BF` wären die Ersatzzeichen `U+D800` bis `U+DFFF`.
            0xed => (2, 0x80, 0x9f),
            0xe1..=0xef => (2, 0x80, 0xbf),
            // `F0 80..8F` wären wieder Codepunkte, die kürzer gehen.
            0xf0 => (3, 0x90, 0xbf),
            // `F4 90..BF` läge über `U+10FFFF`.
            0xf4 => (3, 0x80, 0x8f),
            _ => (3, 0x80, 0xbf),
        };
        self.mode = Mode::Utf8;
        self.pending.clear();
        self.pending.push(lead);
        self.continuation = continuation;
        self.next_lo = lo;
        self.next_hi = hi;
    }

    /// Innerhalb eines Mehrbytezeichens: sammeln, prüfen, dann entscheiden.
    ///
    /// Ein Zeichen geht erst hinaus, wenn es vollständig **und** die kürzeste
    /// Kodierung seines Codepunktes ist. Das ist der Unterschied zu einem
    /// Filter, der nur Folgebytes zählt: `E0 82 9B` trägt drei gültig
    /// aussehende Bytes und ist doch `U+009B`, also CSI; `F0 80 82 9B` ebenso
    /// in vier. RFC 3629 verbietet solche Formen, und ein konformer Dekoder
    /// macht `U+FFFD` daraus — aber der Empfänger ist irgendein Terminal des
    /// Nutzers und keines, das wir aussuchen. Eine Zusage, dass keine
    /// Steuerfolge hinausgeht, darf nicht davon abhängen, dass der Empfänger
    /// richtig dekodiert.
    ///
    /// Was die Prüfung nicht besteht, fällt weg — auch das schon gesammelte
    /// Anfangsbyte. Ein einzelnes Anfangsbyte ist kein Zeichen, und was kein
    /// Zeichen ist, hat im Strom nichts verloren.
    fn in_utf8(&mut self, byte: u8, out: &mut Vec<u8>) {
        if byte < self.next_lo || byte > self.next_hi {
            // Keine wohlgeformte Kodierung. Das Byte wird neu betrachtet:
            // Sonst schmuggelte `C3 1B ]52;…` ein `ESC` an der Prüfung vorbei.
            self.reset();
            self.in_text(byte, out);
            return;
        }
        self.pending.push(byte);
        self.continuation -= 1;
        self.next_lo = 0x80;
        self.next_hi = 0xbf;
        if self.continuation > 0 {
            return;
        }
        let bytes = std::mem::take(&mut self.pending);
        self.reset();
        // `C2 80` bis `C2 9F` ist die Kodierung von `U+0080` bis `U+009F`,
        // also der C1-Steuerzeichen. Ein Terminal, das UTF-8 vor dem Parser
        // dekodiert — VTE und damit GNOME Terminal, Tilix, Terminator, XFCE
        // Terminal, Guake, und xterm mit der Vorgabe `allowC1Printable: off` —
        // führt sie aus. Entschieden wird deshalb am Codepunkt und nicht am
        // Byte; nach der Prüfung oben ist das die einzige Kodierung, die einen
        // solchen Codepunkt überhaupt noch tragen kann.
        if bytes.len() == 2 && bytes[0] == UTF8_C2 && bytes[1] <= 0x9f {
            self.in_text(bytes[1], out);
            return;
        }
        out.extend_from_slice(&bytes);
    }

    /// Nach einem `ESC`: was für eine Folge beginnt.
    fn after_escape(&mut self, byte: u8, out: &mut Vec<u8>) {
        match byte {
            b'[' => {
                self.mode = Mode::Csi;
                self.pending.clear();
            }
            byte if STRING_INTRODUCERS_ESC.contains(&byte) => self.begin_string(byte == b']'),
            // Ab hier gilt: alles Weitere ist die Sieben-Bit-Form.
            // Ein zweites `ESC`: die vorige Folge war keine, die neue beginnt.
            ESC => {}
            // Ein Zwischenbyte: die Folge ist länger als zwei Bytes.
            // `ESC ( B` wählt den Zeichensatz ASCII, `ESC # 8` füllt den
            // Schirm mit `E`. Ohne diesen Zustand bliebe ihr Endbyte als
            // Buchstabe im Text stehen.
            0x20..=0x2f => {
                self.mode = Mode::EscapeIntermediate;
                self.pending.clear();
                self.pending.push(byte);
            }
            // Jede andere Escape-Folge ist eine Ein-Zeichen-Folge. Ohne PTY
            // geht keine davon hinaus: `ESC c` setzt das Terminal zurück,
            // `ESC 7`/`ESC 8` merken und stellen den Cursor wieder her,
            // `ESC M` fährt eine Zeile hoch. Auch die harmlosen darunter
            // bleiben hier — eine Erlaubnisliste zählt auf, was hinaus darf,
            // nicht was nicht. Am PTY braucht ein Vollbild-Agent sie; dort
            // bleibt `ESC c` die Ausnahme.
            _ => {
                self.mode = Mode::Text;
                if self.policy == TerminalPolicy::FullScreen
                    && byte != RIS_FINAL
                    // Ein `ESC \` außerhalb einer Zeichenkettenfolge ist ein
                    // Abschluss ohne Anfang und keine Anweisung. Es entsteht,
                    // wenn eine verworfene Folge schon am `BEL` endete, und
                    // hat im Strom nichts zu suchen.
                    && byte != b'\\'
                    && (0x30..=0x7e).contains(&byte)
                {
                    out.push(ESC);
                    out.push(byte);
                }
            }
        }
    }

    /// Innerhalb einer Escape-Folge mit Zwischenbytes: sammeln bis zum Endbyte.
    fn in_escape_intermediate(&mut self, byte: u8, out: &mut Vec<u8>) {
        if (0x20..=0x2f).contains(&byte) {
            self.pending.push(byte);
            if self.pending.len() >= MAX_PENDING {
                self.reset();
            }
            return;
        }
        if (0x30..=0x7e).contains(&byte) {
            let intermediates = std::mem::take(&mut self.pending);
            let policy = self.policy;
            self.reset();
            if policy == TerminalPolicy::FullScreen {
                out.push(ESC);
                out.extend_from_slice(&intermediates);
                out.push(byte);
            }
            return;
        }
        // Kein gültiges Endbyte: Die Folge ist keine. Sie wird verworfen, und
        // das Byte wird neu betrachtet.
        self.reset();
        self.in_text(byte, out);
    }

    /// Beginnt eine Zeichenkettenfolge (OSC, DCS, SOS, PM, APC).
    ///
    /// Nur eine OSC-Folge kann überhaupt hinausgehen, und auch sie erst, wenn
    /// ihre Nummer da ist; bis dahin bleibt die Entscheidung offen.
    fn begin_string(&mut self, osc: bool) {
        self.mode = Mode::StringSeq;
        self.dropped = 0;
        self.escaped = false;
        self.pending.clear();
        self.number = 0;
        self.digits = 0;
        self.keep = if osc && self.policy == TerminalPolicy::FullScreen {
            None
        } else {
            Some(false)
        };
        self.from_c1 = false;
    }

    /// Innerhalb einer Zeichenkettenfolge: sammeln oder verwerfen, bis sie
    /// endet.
    fn in_string(&mut self, byte: u8, out: &mut Vec<u8>) {
        self.dropped += 1;
        // `BEL` beendet sie, `ESC \` ebenfalls (ST), und `0x9c` ist dasselbe
        // ST in einem Byte. Ein `\` allein beendet nichts: In der Nutzlast
        // steht es in jedem Windows-Pfad und in jedem regulären Ausdruck.
        if self.escaped {
            self.escaped = false;
            if byte == b'\\' {
                self.end_string(Terminator::St, out);
                return;
            }
            // Ein `ESC`, dem kein `\` folgt, steht mitten in der Nutzlast. Das
            // ist keine Farbe, sondern eine zweite Folge im Bauch der ersten:
            // Terminals brechen die äußere daran ab und führen die innere aus.
            self.drop_payload();
        }
        match byte {
            BEL => {
                self.end_string(Terminator::Bel, out);
                return;
            }
            ST_C1 => {
                self.end_string(Terminator::St, out);
                return;
            }
            // Ob es ein ST einleitet, sagt erst das nächste Byte; bis dahin
            // gehört es nicht zur Nutzlast.
            ESC => self.escaped = true,
            _ => self.take_payload(byte),
        }
        if self.dropped >= self.string_cap() {
            // Kein Ende in Sicht. Verworfen bleibt verworfen, aber der Strom
            // läuft weiter: Sonst hielte ein `\x1b]` ohne Abschluss die ganze
            // weitere Ausgabe an.
            self.reset();
        }
    }

    /// Ein Byte der Nutzlast, solange die Folge noch hinausgehen könnte.
    fn take_payload(&mut self, byte: u8) {
        match self.keep {
            // Die Nummer wird noch gelesen.
            None => {
                if byte.is_ascii_digit() && self.digits < MAX_OSC_DIGITS {
                    self.number = self.number * 10 + u32::from(byte - b'0');
                    self.digits += 1;
                    self.pending.push(byte);
                } else if byte == b';' {
                    self.decide_number();
                    if self.keep == Some(true) {
                        self.pending.push(byte);
                    }
                } else {
                    // Kein Zahlenkopf: keine Folge, die hinausgeht.
                    self.drop_payload();
                }
            }
            Some(true) => {
                // Die Nutzlast einer erlaubten Folge ist druckbares ASCII.
                // Alles andere — ein Steuerzeichen, ein C1-Byte, der Anfang
                // eines weiteren Zeichens — ließe sich zu einer zweiten Folge
                // zusammensetzen, sobald das Terminal die äußere abbricht.
                if (0x20..=0x7e).contains(&byte) && self.pending.len() < MAX_STRING_PENDING {
                    self.pending.push(byte);
                } else {
                    self.drop_payload();
                }
            }
            Some(false) => {}
        }
    }

    /// Diese Folge geht nicht mehr hinaus; was von ihr gepuffert ist, fällt weg.
    fn drop_payload(&mut self) {
        self.keep = Some(false);
        self.pending.clear();
    }

    /// Entscheidet an der gelesenen Nummer, ob die Folge hinausgehen darf.
    fn decide_number(&mut self) {
        let allowed = self.digits > 0 && OSC_ALLOWED.contains(&self.number);
        if allowed {
            self.keep = Some(true);
        } else {
            self.drop_payload();
        }
    }

    /// Wie lange eine Folge zurückgehalten wird, bevor sie als endlos gilt.
    const fn string_cap(&self) -> usize {
        match self.keep {
            Some(false) => MAX_PENDING,
            _ => MAX_STRING_PENDING,
        }
    }

    /// Die Folge ist zu Ende; sie geht hinaus oder fällt weg.
    fn end_string(&mut self, terminator: Terminator, out: &mut Vec<u8>) {
        if self.keep.is_none() {
            // Eine Folge ohne `;`, etwa `ESC ] 4 BEL`: Die Nummer steht, aber
            // niemand hat sie bisher geprüft.
            self.decide_number();
        }
        if self.keep == Some(true) && !self.from_c1 {
            out.push(ESC);
            out.push(b']');
            out.extend_from_slice(&self.pending);
            match terminator {
                Terminator::Bel => out.push(BEL),
                // `ST` geht immer in seiner Sieben-Bit-Form hinaus; ein
                // `0x9c` läse ein Client ohne UTF-8 als Steuerzeichen.
                Terminator::St => out.extend_from_slice(b"\x1b\\"),
            }
        }
        self.reset();
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
            let passes = !self.from_c1
                && match self.policy {
                    TerminalPolicy::ColourOnly => byte == SGR_FINAL,
                    // Ein Vollbild-Agent zeichnet mit dem ganzen CSI-Vorrat; nur
                    // die Fenstersteuerung bleibt draußen (siehe
                    // Modulbeschreibung).
                    TerminalPolicy::FullScreen => byte != WINDOW_OPS_FINAL,
                };
            let params = std::mem::take(&mut self.pending);
            self.reset();
            if passes {
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

    /// Ein Anfangsbyte ohne sein Folgebyte fällt weg; das Byte danach wird
    /// neu betrachtet.
    ///
    /// Ohne diese Regel schmuggelte `C3 1B ]52;…` ein `ESC` an der Prüfung
    /// vorbei. Seit HUM-042 fällt dabei auch das Anfangsbyte selbst weg: Ein
    /// halbes Zeichen ist kein Zeichen, und der Filter gibt nur ganze heraus.
    #[test]
    fn a_lead_byte_does_not_smuggle_an_escape() {
        // `C3` erwartet ein Folgebyte, `1B` ist keines: Das angefangene
        // Zeichen wird verworfen, und das `ESC` beginnt eine Folge, die nicht
        // hinausgeht.
        assert_eq!(text(&[b"a\xc3\x1b]52;c;c2VjcmV0\x07b"]), "ab");
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

/// Die zweite Politik: was ein Vollbild-Agent am PTY hinausbekommt und was
/// auch dort nicht (HUM-042).
#[cfg(test)]
mod fullscreen_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{MAX_STRING_PENDING, TerminalFilter, TerminalPolicy};

    /// Der Filter des Terminal-Stroms über einer Folge von Stücken.
    fn pty(chunks: &[&[u8]]) -> Vec<u8> {
        let mut filter = TerminalFilter::with_policy(TerminalPolicy::FullScreen);
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(filter.push(chunk));
        }
        out.extend(filter.flush());
        out
    }

    fn text(chunks: &[&[u8]]) -> String {
        String::from_utf8_lossy(&pty(chunks)).into_owned()
    }

    /// Ein Vollbild-TUI zeichnet mit absoluter Adressierung; ohne diese Folgen
    /// ist es nicht bedienbar.
    #[test]
    fn a_full_screen_agent_can_draw() {
        for sequence in [
            &b"\x1b[2J"[..],   // Schirm löschen
            b"\x1b[10;5H",     // Cursor setzen
            b"\x1b[?1049h",    // Alternativschirm
            b"\x1b[?25l",      // Cursor verstecken
            b"\x1b[1A",        // eine Zeile hoch
            b"\x1b[2K",        // Zeile löschen
            b"\x1b[?1000h",    // Mausverfolgung
            b"\x1b[?2004h",    // Klammer-Einfügen
            b"\x1b[1;24r",     // Scrollbereich
            b"\x1b[38;5;208m", // Farbe
            b"\x1b[6n",        // Cursorposition erfragen
            b"\x1b7",          // Cursor merken
            b"\x1b8",          // Cursor zurück
            b"\x1bM",          // eine Zeile hoch
            b"\x1b=",          // Anwendungs-Tastenfeld
            b"\x1b(B",         // Zeichensatz ASCII
        ] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(sequence);
            payload.push(b'b');
            assert_eq!(
                pty(&[&payload]),
                payload,
                "{:?} belongs to a full-screen agent",
                String::from_utf8_lossy(sequence)
            );
        }
    }

    /// Dieselben Folgen ohne PTY: keine einzige davon geht hinaus, außer der
    /// Farbe. Der Unterschied zwischen den Politiken ist damit gemessen und
    /// nicht behauptet.
    #[test]
    fn the_stream_without_a_pty_keeps_its_narrower_promise() {
        let mut filter = TerminalFilter::new();
        assert_eq!(filter.policy(), TerminalPolicy::ColourOnly);
        assert_eq!(filter.push(b"a\x1b[2Jb"), b"ab");
        assert_eq!(filter.push(b"\x1b[31mred"), b"\x1b[31mred");
        // Auch die Zeichensatz-Folge: sie geht ganz weg, nicht nur ihr
        // Einleiter.
        assert_eq!(filter.push(b"x\x1b(By"), b"xy");
    }

    // --- Die Folgen, die auch am PTY nicht hinausgehen ---------------------

    #[test]
    fn osc52_removed_across_chunks() {
        // Die Folge liegt in zwei Stücken; ein zustandsloser Filter sähe je
        // eine Hälfte und ließe beide durch.
        assert_eq!(
            text(&[b"before\x1b]52;c;c2Vj", b"cmV0\x07after"]),
            "beforeafter"
        );
        // Und in einem Stück, mit ST statt BEL.
        assert_eq!(text(&[b"a\x1b]52;c;c2VjcmV0\x1b\\b"]), "ab");
    }

    #[test]
    fn osc8_removed() {
        assert_eq!(
            text(&[b"a\x1b]8;;https://evil.example/\x07click\x1b]8;;\x07b"]),
            "aclickb"
        );
    }

    #[test]
    fn osc0_title_removed() {
        for title in [
            &b"\x1b]0;All Checks Passed\x07"[..],
            b"\x1b]1;icon\x07",
            b"\x1b]2;All Checks Passed\x1b\\",
        ] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(title);
            payload.push(b'b');
            assert_eq!(
                text(&[&payload]),
                "ab",
                "{:?}",
                String::from_utf8_lossy(title)
            );
        }
    }

    #[test]
    fn osc133_passes() {
        let marker = b"\x1b]133;A\x07prompt\x1b]133;B\x07";
        assert_eq!(pty(&[marker]), marker.to_vec());
    }

    #[test]
    fn sgr_passes() {
        let coloured = b"\x1b[31mred\x1b[0m and \x1b[1;32mgreen\x1b[m";
        assert_eq!(pty(&[coloured]), coloured.to_vec());
    }

    #[test]
    fn ris_removed() {
        assert_eq!(text(&[b"a\x1bcb"]), "ab");
    }

    #[test]
    fn dcs_removed() {
        // `ESC P tmux;` reicht eine verbotene Folge durch tmux an das äußere
        // Terminal weiter.
        assert_eq!(
            text(&[b"a\x1bPtmux;\x1b\x1b]52;c;c2VjcmV0\x07\x1b\\b"]),
            "ab"
        );
    }

    #[test]
    fn apc_removed() {
        // kitty-Grafik und das Dateiprotokoll von iTerm2 reisen über APC.
        assert_eq!(text(&[b"a\x1b_Ga=T,f=100;iVBORw0KGgo=\x1b\\b"]), "ab");
    }

    #[test]
    fn pm_removed() {
        assert_eq!(text(&[b"a\x1b^private\x1b\\b"]), "ab");
    }

    #[test]
    fn sos_removed() {
        assert_eq!(text(&[b"a\x1bXstring\x1b\\b"]), "ab");
    }

    /// Fenstersteuerung ist keine Ausgabe: `CSI 21 t` lässt das Terminal
    /// seinen Titel in die Eingabe des Agenten schreiben, `CSI 22/23 t` legen
    /// den Titel ab und stellen ihn wieder her, `CSI 8;h;w t` ändert die Größe.
    #[test]
    fn window_operations_do_not_leave() {
        for attack in [
            &b"\x1b[21t"[..],
            b"\x1b[22;0t",
            b"\x1b[23;0t",
            b"\x1b[8;50;200t",
            b"\x1b[2t",
        ] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(attack);
            payload.push(b'b');
            assert_eq!(
                text(&[&payload]),
                "ab",
                "{:?} is window control, not output",
                String::from_utf8_lossy(attack)
            );
        }
    }

    /// Die Nummern, die eine Sperrliste übersehen hätte.
    ///
    /// Die Spezifikation nannte `[0, 1, 2, 7, 8, 9, 52, 777, 1337]`. OSC 99
    /// (Benachrichtigung in kitty), OSC 12 (Cursorfarbe) und OSC 50
    /// (Schriftart) stehen dort nicht und wären hinausgegangen.
    #[test]
    fn the_numbers_a_deny_list_would_have_missed_do_not_leave() {
        for attack in [
            &b"\x1b]99;i=1:d=0;body\x07"[..],
            b"\x1b]12;#ff0000\x07",
            b"\x1b]50;xft:Comic Sans\x07",
            b"\x1b]7;file://host/tmp\x07",
            b"\x1b]9;notification\x07",
            b"\x1b]777;notify;title;body\x07",
            b"\x1b]1337;File=inline=1:AAAA\x07",
            b"\x1b]5113;x\x07",
        ] {
            let mut payload = b"a".to_vec();
            payload.extend_from_slice(attack);
            payload.push(b'b');
            assert_eq!(
                text(&[&payload]),
                "ab",
                "{:?} is not on the allow list",
                String::from_utf8_lossy(attack)
            );
        }
    }

    /// Führende Nullen ändern die Nummer nicht: Terminals lesen sie als Zahl.
    #[test]
    fn a_leading_zero_neither_hides_nor_creates_a_number() {
        assert_eq!(text(&[b"a\x1b]052;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\x1b]0000000052;c;x\x07b"]), "ab");
        // Und andersherum: `004` ist `4` und geht hinaus.
        assert_eq!(
            pty(&[b"\x1b]004;1;rgb:00/00/00\x07"]),
            b"\x1b]004;1;rgb:00/00/00\x07"
        );
    }

    /// Eine erlaubte Folge geht wörtlich hinaus; nur das ST wird auf seine
    /// Sieben-Bit-Form gebracht.
    #[test]
    fn an_allowed_colour_sequence_leaves_unchanged() {
        assert_eq!(
            pty(&[b"\x1b]4;1;rgb:ff/00/00\x07"]),
            b"\x1b]4;1;rgb:ff/00/00\x07"
        );
        assert_eq!(pty(&[b"\x1b]10;#ffffff\x1b\\"]), b"\x1b]10;#ffffff\x1b\\");
        assert_eq!(pty(&[b"\x1b]11;?\x07"]), b"\x1b]11;?\x07");
        // Die Rücknahmen gehören dazu, sonst bliebe eine gesetzte Farbe für
        // immer stehen.
        assert_eq!(pty(&[b"\x1b]104\x07"]), b"\x1b]104\x07");
        assert_eq!(pty(&[b"\x1b]110\x1b\\"]), b"\x1b]110\x1b\\");
        // `0x9c` ist dasselbe ST in einem Byte und geht als `ESC \` hinaus.
        assert_eq!(pty(&[b"\x1b]111\x9c"]), b"\x1b]111\x1b\\");
    }

    /// Eine erlaubte Folge mit einer zweiten Folge im Bauch fällt ganz weg.
    ///
    /// Terminals brechen die äußere Folge an einem eingebetteten `ESC` ab und
    /// führen aus, was danach steht. Ohne diese Regel schmuggelte
    /// `OSC 4; ESC ] 52 ; …` die Zwischenablage-Folge an der Nummernprüfung
    /// vorbei.
    #[test]
    fn a_nested_sequence_inside_an_allowed_one_drops_both() {
        assert_eq!(text(&[b"a\x1b]4;1;\x1b]52;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\x1b]133;A\x1b]0;title\x07b"]), "ab");
        // Auch ein rohes Steuerzeichen in der Nutzlast.
        assert_eq!(text(&[b"a\x1b]4;1;rgb\rboom\x07b"]), "ab");
        // Und ein C1-Byte darin.
        assert_eq!(text(&[b"a\x1b]4;1;\x9d52;c;x\x07b"]), "ab");
    }

    /// Eine überlange Kodierung ist ein Steuerzeichen in Verkleidung.
    ///
    /// `E0 82 9B` trägt drei gültig aussehende Bytes — ein Anfangsbyte für
    /// drei und zwei Folgebytes mit `10xxxxxx` — und ist doch `U+009B`, also
    /// CSI. In vier Bytes geht dasselbe (`F0 80 82 9B`). RFC 3629 verbietet
    /// diese Formen, und ein konformer Dekoder macht `U+FFFD` daraus; der
    /// Empfänger ist hier aber irgendein Terminal des Nutzers und keines, das
    /// wir aussuchen. Eine Zusage, dass keine Steuerfolge hinausgeht, darf
    /// nicht davon abhängen, dass der Empfänger richtig dekodiert.
    ///
    /// Die Marke steht **vor** dem Angriff und nicht dahinter: Was einem
    /// Einleiter folgt, gehört zu seiner Folge, und eine Folge, die nicht
    /// hinausgeht, nimmt ihre Nutzlast mit.
    #[test]
    fn an_overlong_encoding_never_leaves() {
        // Die vier Zeichenketten-Einleiter, überlang in drei Bytes: Alles bis
        // zum `BEL` gehört zur Folge, die Marke dahinter steht wieder da.
        for hidden in [0x9d_u8, 0x90, 0x98, 0x9e, 0x9f] {
            let mut attack = b"a".to_vec();
            attack.extend_from_slice(&[0xe0, 0x82, hidden]);
            attack.extend_from_slice(b"52;c;c2VjcmV0\x07b");
            assert_eq!(
                text(&[&attack]),
                "ab",
                "E0 82 {hidden:02X} is U+00{hidden:02X} to a lax decoder"
            );
            // Und über eine Stückgrenze, an beiden Stellen.
            assert_eq!(text(&[b"a\xe0", &attack[2..]]), "ab");
            assert_eq!(text(&[b"a\xe0\x82", &attack[3..]]), "ab");
        }
        // CSI, überlang: `2J` löscht den Schirm, `J` beendet die Folge.
        assert_eq!(text(&[b"a\xe0\x82\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\xe0", b"\x82\x9b2Jb"]), "ab");
        // Dieselbe Verkleidung in vier Bytes.
        assert_eq!(text(&[b"a\xf0\x80\x82\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\xf0\x80\x82\x9d52;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\xf0\x80", b"\x82\x9d52;c;x\x07b"]), "ab");
        // Und die zweibytige überlange Form, die sogar `ESC` selbst trägt:
        // `C0 9B` wäre `U+001B`.
        assert_eq!(text(&[b"a\xc0\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\xc1\xa0b"]), "ab");
        // Ersatzzeichen und alles über `U+10FFFF` gehen ebenfalls nicht
        // hinaus: `ED A0 80` wäre `U+D800`, `F4 BF BF BF` läge darüber.
        assert_eq!(text(&[b"a\xed\xa0\x80b"]), "ab");
        assert_eq!(text(&[b"a\xf4\xbf\xbf\xbfb"]), "ab");
        // `F4 90 …` fällt genauso weg; sein zweites Byte ist zufällig `0x90`,
        // also DCS, und nimmt als Einleiter den Rest bis zum Abschluss mit.
        assert_eq!(text(&[b"a\xf4\x90\x80\x80b\x07c"]), "ac");
        // Was die kürzeste Form ist, geht weiterhin hinaus, bis an beide
        // Ränder des erlaubten Bereichs.
        assert_eq!(text(&["ä€𝄞".as_bytes()]), "ä€𝄞");
        assert_eq!(
            text(&["\u{d7ff}\u{e000}\u{10ffff}".as_bytes()]),
            "\u{d7ff}\u{e000}\u{10ffff}"
        );
    }

    /// Eine Folge, die mit einem C1-Byte beginnt, geht nie hinaus — auch
    /// nicht, wenn dieselbe Folge mit `ESC` erlaubt wäre.
    ///
    /// Kein terminfo-Eintrag für `xterm-256color` erzeugt Acht-Bit-Einleiter;
    /// wer sie schickt, verkleidet etwas. Ohne diese Regel machte der Filter
    /// aus `\x9b 2 J` die erlaubte Sieben-Bit-Form und reichte sie weiter.
    #[test]
    fn a_c1_introducer_is_not_a_shorter_esc() {
        assert_eq!(text(&[b"a\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\x9b31mb"]), "ab");
        assert_eq!(text(&[b"a\x9d4;1;rgb:00/00/00\x07b"]), "ab");
        // Die UTF-8-Kodierung desselben Bytes ebenso.
        assert_eq!(text(&[b"a\xc2\x9b2Jb"]), "ab");
        assert_eq!(text(&[b"a\xc2\x9d4;1;rgb:00/00/00\x07b"]), "ab");
        // Mit `ESC` geht dieselbe Folge hinaus.
        assert_eq!(pty(&[b"\x1b[2J"]), b"\x1b[2J");
        assert_eq!(
            pty(&[b"\x1b]4;1;rgb:00/00/00\x07"]),
            b"\x1b]4;1;rgb:00/00/00\x07"
        );
    }

    /// Die C1-Formen, roh und als UTF-8, auch am PTY.
    #[test]
    fn the_c1_forms_are_filtered_in_both_encodings() {
        // Roh.
        assert_eq!(text(&[b"a\x9d52;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\x9d0;title\x07b"]), "ab");
        // Als wohlgeformtes UTF-8: `C2 9D` ist U+009D, also OSC.
        assert_eq!(text(&[b"a\xc2\x9d52;c;c2VjcmV0\x07b"]), "ab");
        assert_eq!(text(&[b"a\xc2\x90tmux;x\x1b\\b"]), "ab");
        // Über Stückgrenzen hinweg.
        assert_eq!(text(&[b"a\xc2", b"\x9d52;c;x\x07b"]), "ab");
        // Ein C1-Byte innerhalb eines druckbaren Zeichens bleibt Text.
        assert_eq!(text(&[b"a\xe2\x82\x9db"]), "a\u{209d}b");
        assert_eq!(text(&["über € 12,50 ⛛".as_bytes()]), "über € 12,50 ⛛");
    }

    /// Eine erlaubte Folge ohne Abschluss hält den Strom nicht auf und wird
    /// verworfen; der Filter erholt sich.
    ///
    /// Was nach der Schranke kommt, ist kein Teil der Folge mehr, sondern
    /// Text — und wird als Text behandelt, also gefiltert wie jeder andere.
    /// Die Folge selbst geht nie hinaus.
    #[test]
    fn unterminated_osc_dropped_after_cap() {
        const PAYLOAD: usize = 70 * 1024;
        let mut long = Vec::from(*b"a\x1b]4;1;");
        long.extend(std::iter::repeat_n(b'x', PAYLOAD));
        long.push(b'b');
        let out = text(&[&long]);
        assert!(out.starts_with('a'), "{:?}", &out[..out.len().min(20)]);
        assert!(out.ends_with('b'), "the stream goes on");
        assert!(
            !out.contains('\u{1b}'),
            "no part of the sequence itself leaves"
        );
        assert!(
            out.len() <= PAYLOAD - MAX_STRING_PENDING + 8,
            "everything up to the cap stays inside: {} bytes",
            out.len()
        );
        // Der Puffer wächst nicht über die Schranke hinaus.
        let mut filter = TerminalFilter::with_policy(TerminalPolicy::FullScreen);
        let _ = filter.push(b"\x1b]4;");
        let _ = filter.push(&vec![b'x'; MAX_STRING_PENDING + 10]);
        assert!(filter.at_boundary(), "the filter is back in text");
        assert_eq!(filter.push(b"plain"), b"plain");
    }

    /// Das ST besteht aus zwei Bytes, die über eine Stückgrenze fallen dürfen.
    #[test]
    fn the_terminator_may_be_split_across_chunks() {
        assert_eq!(text(&[b"a\x1b]52;c;x\x1b", b"\\b"]), "ab");
        assert_eq!(
            pty(&[b"\x1b]4;1;rgb:00/00/00\x1b", b"\\"]),
            b"\x1b]4;1;rgb:00/00/00\x1b\\"
        );
        // Ein `\` ohne `ESC` davor beendet nichts.
        assert_eq!(text(&[b"a\x1b]52;c;C:\\Users\\x\x07b"]), "ab");
    }

    /// Die Grenze, an der ein Hinweis eingefügt werden darf.
    #[test]
    fn a_notice_only_fits_between_two_characters() {
        let mut filter = TerminalFilter::with_policy(TerminalPolicy::FullScreen);
        assert!(filter.at_boundary(), "a fresh filter stands between");
        let _ = filter.push(b"plain text");
        assert!(filter.at_boundary());
        // Mitten in einer Folge nicht.
        let _ = filter.push(b"\x1b[3");
        assert!(!filter.at_boundary());
        let _ = filter.push(b"1m");
        assert!(filter.at_boundary());
        // Und mitten in einem Zeichen auch nicht: `ä` ist `C3 A4`.
        let _ = filter.push(b"\xc3");
        assert!(!filter.at_boundary(), "half a character is no boundary");
        let _ = filter.push(b"\xa4");
        assert!(filter.at_boundary());
        // Auch das zurückgehaltene `0xC2` ist keine Grenze.
        let _ = filter.push(b"\xc2");
        assert!(!filter.at_boundary());
        let _ = filter.push(b"\xa0");
        assert!(filter.at_boundary());
        // Und eine offene Zeichenkettenfolge ebenso wenig.
        let _ = filter.push(b"\x1b]4;1;");
        assert!(!filter.at_boundary());
    }

    /// Eine abgeschnittene Folge geht am Stromende nicht hinaus; sonst bliebe
    /// das Terminal im Escape-Zustand und verschluckte die nächste Eingabe.
    #[test]
    fn a_truncated_sequence_is_dropped_at_the_end_of_the_stream() {
        for tail in [
            &b"\x1b"[..],
            b"\x1b]",
            b"\x1b]4",
            b"\x1b]4;1;rgb",
            b"\x1b[",
            b"\x1b[31",
            b"\x1b(",
        ] {
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
}
