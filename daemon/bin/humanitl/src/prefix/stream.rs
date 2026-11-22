//! Was die Ausgabe des Agenten über das Terminal sagt: kitty-Tastaturprotokoll
//! und Alternativschirm (GitHub #10).
//!
//! Der Daemon hat die Bytes schon gefiltert (`humanitl_core::TerminalFilter`,
//! Politik `FullScreen`); dieser Leser filtert nichts und ändert kein Byte.
//! Er sieht nur zu, was an das eigene Terminal geht, und merkt sich zwei
//! Dinge:
//!
//! - **Das kitty-Tastaturprotokoll.** `CSI > Flags u` legt Flags auf einen
//!   Stapel, `CSI < n u` nimmt `n` herunter (ohne `n` eines), `CSI = Flags ;
//!   Modus u` ändert die obersten. Haupt- und Alternativschirm haben je einen
//!   eigenen Stapel, wie im Protokoll beschrieben. Eingeschaltet heißt: Die
//!   obersten Flags verlangen eindeutige Folgen für Tasten mit
//!   Modifikatoren (Flag 1 oder 8). Nur dann schickt das Terminal
//!   `Ctrl+,` als `CSI 44;5u`. `CSI > … u` passiert den Filter
//!   `FullScreen` heute, also kommt der Zustand an.
//! - **Den Alternativschirm** (`CSI ? 1049 h`, `1047`, `47`). Eine Ansicht
//!   über einem Agenten auf dem gewöhnlichen Schirm zieht auf den
//!   Alternativschirm um und lässt dessen Bild beim Zurückkehren
//!   unberührt; über einem Vollbild-Agenten zeichnet sie darüber und lässt
//!   ihn danach neu zeichnen ([`super::view`]).
//!
//! Der Zustand gilt für die Bytes, die schon auf dem Terminal stehen, nicht
//! für zurückgehaltene: Das Terminal schickt `CSI`-Folgen erst, nachdem es
//! das Einschalten gesehen hat.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// `ESC`.
const ESC: u8 = 0x1b;

/// `BEL`, das Ende einer OSC-Folge.
const BEL: u8 = 0x07;

/// Wie viele Parameterbytes eine Folge hier höchstens hat; eine längere ist
/// für diesen Leser keine.
const MAX_PARAMS: usize = 64;

/// Wie tief ein Stapel des Protokolls höchstens wird. Darüber fällt der
/// älteste Eintrag weg, wie das Protokoll es den Terminals vorschreibt.
const MAX_STACK: usize = 32;

/// Die Flags, unter denen das Terminal Tasten mit Modifikatoren eindeutig
/// schickt: „disambiguate" (1) und „report all keys" (8).
const DISAMBIGUATING: u32 = 1 | 8;

/// Wo der Leser in einer Folge steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parse {
    /// Gewöhnlicher Text.
    Ground,
    /// Nach `ESC`.
    Escape,
    /// In einer CSI-Folge.
    Csi,
    /// In einer Zeichenkettenfolge (OSC und Verwandte), bis `BEL` oder `ST`.
    Text,
    /// Nach `ESC` in einer Zeichenkettenfolge.
    TextEscape,
}

/// Der Zustand des Terminals, wie die Ausgabe ihn gesetzt hat.
#[derive(Debug)]
pub struct StreamState {
    /// Wo der Leser steht.
    parse: Parse,
    /// Die Parameter der laufenden CSI-Folge.
    params: Vec<u8>,
    /// Ob der Alternativschirm gilt.
    alt: bool,
    /// Die Stapel des Protokolls: gewöhnlicher Schirm, Alternativschirm.
    stacks: [Vec<u32>; 2],
    /// Die Flags unter dem leeren Stapel je Schirm: Nur `CSI = … u` ohne
    /// vorheriges `CSI > … u` setzt sie. Sie sind kein Eintrag des Stapels,
    /// damit das Verlassen nie mehr herunternimmt, als der Agent auflegte
    /// (Review zu GitHub #10).
    base: [u32; 2],
    /// Ob das Protokoll eingeschaltet ist; geteilt mit dem Leser der Tastatur.
    kitty: Arc<AtomicBool>,
}

impl StreamState {
    /// Ein Terminal ohne Protokoll auf dem gewöhnlichen Schirm. `kitty` ist
    /// der Schalter, den der Leser der Tastatur liest.
    pub fn new(kitty: Arc<AtomicBool>) -> Self {
        kitty.store(false, Ordering::SeqCst);
        Self {
            parse: Parse::Ground,
            params: Vec::new(),
            alt: false,
            stacks: [Vec::new(), Vec::new()],
            base: [0, 0],
            kitty,
        }
    }

    /// Sieht die Bytes, die gerade an das Terminal gegangen sind.
    pub fn observe(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.byte(byte);
        }
    }

    /// Ob der Agent das kitty-Tastaturprotokoll eingeschaltet hat. Im
    /// Anschluss liest der Leser der Tastatur denselben Schalter direkt.
    #[cfg(test)]
    pub fn kitty(&self) -> bool {
        self.kitty.load(Ordering::SeqCst)
    }

    /// Ob der Agent auf dem Alternativschirm zeichnet.
    pub const fn alt_screen(&self) -> bool {
        self.alt
    }

    /// Was beim Verlassen vor dem gewöhnlichen Schirm geschrieben wird: die
    /// Flags des Protokolls, die der Agent auf dem Alternativschirm stehen
    /// ließ, herunternehmen. Danach folgt der Wechsel auf den gewöhnlichen
    /// Schirm und [`StreamState::leave_after`].
    ///
    /// Sonst bliebe die Shell des Menschen nach dem Abhängen in einem
    /// Terminal, das jede Taste mit `Ctrl` als Folge schickt. Eingeschaltet
    /// wird hier nichts.
    pub fn leave_before(&self) -> Vec<u8> {
        if self.alt {
            self.take_back(1)
        } else {
            Vec::new()
        }
    }

    /// Was nach dem Wechsel auf den gewöhnlichen Schirm geschrieben wird:
    /// dessen Flags herunternehmen.
    pub fn leave_after(&self) -> Vec<u8> {
        self.take_back(0)
    }

    /// Die Flags eines Schirms zurücknehmen: so viele Einträge herunter, wie
    /// der Agent auflegte, und Flags unter dem leeren Stapel auf null.
    fn take_back(&self, screen: usize) -> Vec<u8> {
        let mut bytes = pops(self.stacks[screen].len());
        if self.base[screen] != 0 {
            bytes.extend_from_slice(b"\x1b[=0;1u");
        }
        bytes
    }

    /// Ein Byte.
    fn byte(&mut self, byte: u8) {
        self.parse = match (self.parse, byte) {
            // `BEL` oder `ST` (`ESC \`) beenden die Zeichenkette.
            (Parse::Text, BEL) | (Parse::TextEscape, b'\\') => Parse::Ground,
            (Parse::Text | Parse::TextEscape, ESC) => Parse::TextEscape,
            (Parse::Text | Parse::TextEscape, _) => Parse::Text,
            (_, ESC) => Parse::Escape,
            (Parse::Escape, b'[') => {
                self.params.clear();
                Parse::Csi
            }
            (Parse::Escape, b']' | b'P' | b'X' | b'^' | b'_') => Parse::Text,
            (Parse::Csi, 0x20..=0x3f) => {
                self.params.push(byte);
                if self.params.len() > MAX_PARAMS {
                    Parse::Ground
                } else {
                    Parse::Csi
                }
            }
            (Parse::Csi, 0x40..=0x7e) => {
                let params = std::mem::take(&mut self.params);
                self.csi(&params, byte);
                Parse::Ground
            }
            _ => Parse::Ground,
        };
    }

    /// Eine vollständige CSI-Folge.
    fn csi(&mut self, params: &[u8], last: u8) {
        match (params.first(), last) {
            (Some(b'>'), b'u') => {
                let flags = number(&params[1..]).unwrap_or(0);
                let stack = self.stack();
                if stack.len() >= MAX_STACK {
                    stack.remove(0);
                }
                stack.push(flags);
            }
            (Some(b'<'), b'u') => {
                let count = number(&params[1..]).unwrap_or(1).max(1);
                let stack = self.stack();
                let keep = stack
                    .len()
                    .saturating_sub(usize::try_from(count).unwrap_or(usize::MAX));
                stack.truncate(keep);
            }
            (Some(b'='), b'u') => {
                let mut fields = params[1..].split(|byte| *byte == b';');
                let flags = fields.next().and_then(number).unwrap_or(0);
                let mode = fields.next().and_then(number).unwrap_or(1);
                let screen = usize::from(self.alt);
                let top = match self.stacks[screen].last_mut() {
                    Some(top) => top,
                    None => &mut self.base[screen],
                };
                *top = match mode {
                    2 => *top | flags,
                    3 => *top & !flags,
                    _ => flags,
                };
            }
            (Some(b'?'), b'h' | b'l') => {
                let switches = params[1..]
                    .split(|byte| *byte == b';')
                    .filter_map(number)
                    .any(|mode| matches!(mode, 47 | 1047 | 1049));
                if switches {
                    self.alt = last == b'h';
                }
            }
            _ => return,
        }
        let screen = usize::from(self.alt);
        let flags = self.stacks[screen]
            .last()
            .copied()
            .unwrap_or(self.base[screen]);
        let on = flags & DISAMBIGUATING != 0;
        self.kitty.store(on, Ordering::SeqCst);
    }

    /// Der Stapel des Schirms, der gerade gilt.
    fn stack(&mut self) -> &mut Vec<u32> {
        &mut self.stacks[usize::from(self.alt)]
    }
}

/// Eine Zahl aus Ziffern; `None` für leer oder anderes.
fn number(digits: &[u8]) -> Option<u32> {
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// `CSI < n u` für `n` Einträge, oder nichts.
fn pops(count: usize) -> Vec<u8> {
    if count == 0 {
        Vec::new()
    } else {
        format!("\x1b[<{count}u").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::StreamState;

    fn state() -> StreamState {
        StreamState::new(Arc::new(AtomicBool::new(true)))
    }

    /// Einschalten, abschalten, über eine Lesegrenze, und Flags ohne
    /// eindeutige Tasten zählen nicht.
    #[test]
    fn the_kitty_protocol_follows_push_and_pop() {
        let mut stream = state();
        assert!(!stream.kitty(), "off at the start");
        stream.observe(b"text\x1b[>1u");
        assert!(stream.kitty());
        stream.observe(b"\x1b[<u");
        assert!(!stream.kitty());
        stream.observe(b"\x1b[>");
        stream.observe(b"3u");
        assert!(stream.kitty(), "split across two messages");
        stream.observe(b"\x1b[>2u");
        assert!(!stream.kitty(), "flag 2 alone does not disambiguate");
        stream.observe(b"\x1b[<1u");
        assert!(stream.kitty(), "the pushed 3 is on top again");
        stream.observe(b"\x1b[=0;1u");
        assert!(!stream.kitty(), "set to nothing");
        stream.observe(b"\x1b[=8;2u");
        assert!(stream.kitty(), "set by bits");
    }

    /// Eine Folge in einer OSC-Zeichenkette zählt nicht, und der Alternativ-
    /// schirm hat seinen eigenen Stapel.
    #[test]
    fn strings_and_screens_are_kept_apart() {
        let mut stream = state();
        stream.observe(b"\x1b]11;rgb:00/00/00\x07");
        assert!(!stream.kitty());
        stream.observe(b"\x1b[>1u");
        stream.observe(b"\x1b[?1049h");
        assert!(stream.alt_screen());
        assert!(!stream.kitty(), "the alternate screen has its own stack");
        stream.observe(b"\x1b[>1u\x1b[>1u");
        assert_eq!(stream.leave_before(), b"\x1b[<2u");
        stream.observe(b"\x1b[?1049l");
        assert!(!stream.alt_screen());
        assert!(stream.kitty(), "back on the main screen");
        assert!(stream.leave_before().is_empty());
        assert_eq!(stream.leave_after(), b"\x1b[<1u");
    }

    /// `CSI = … u` auf einem leeren Stapel legt keinen Eintrag an: Das
    /// Verlassen nimmt nichts herunter, setzt die Flags aber auf null.
    /// Mutationsbeweis: ein künstlicher Eintrag macht ihn rot.
    #[test]
    fn setting_flags_on_an_empty_stack_pushes_nothing() {
        let mut stream = state();
        stream.observe(b"\x1b[=1;1u");
        assert!(stream.kitty());
        assert_eq!(stream.leave_after(), b"\x1b[=0;1u");
        stream.observe(b"\x1b[>1u");
        assert_eq!(stream.leave_after(), b"\x1b[<1u\x1b[=0;1u");
    }
}
