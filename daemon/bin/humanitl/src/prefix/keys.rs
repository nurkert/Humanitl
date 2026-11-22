//! Die Zustandsmaschine der Tastatur: Präfix, Tabelle, `Ctrl+,` direkt
//! (GitHub #10).
//!
//! Rein und ohne Ein- und Ausgabe, damit jeder Fall ohne Terminal prüfbar
//! ist. Sie bekommt jedes gelesene Stück und sagt, welche Bytes an den
//! Agenten gehen und welche Ereignisse der Anschluss behandelt.
//!
//! # Tasten statt Bytes, wenn der Agent das kitty-Protokoll eingeschaltet hat
//!
//! Ohne das Protokoll ist jede Taste hier ein Byte, wie vor GitHub #10: Ein
//! Präfix am Ende eines Lesevorgangs wartet ohne Frist auf das nächste Byte,
//! und jede Escape-Folge geht Byte für Byte weiter.
//!
//! Hat der Agent das Protokoll eingeschaltet (der Zustand kommt aus seiner
//! Ausgabe, [`super::stream::StreamState`]), schickt das Terminal Tasten mit
//! Modifikatoren als `CSI Code ; Modifikatoren u`: `Ctrl+B` wird zu
//! `CSI 98;5u` statt `0x02`, `Ctrl+,` zu `CSI 44;5u`. Dann liest der Filter
//! eine Escape-Folge als eine Taste und hält eine angefangene Folge am Ende
//! eines Lesevorgangs bis zum nächsten, ebenfalls ohne Frist. Das Präfix
//! wird in beiden Formen erkannt; geht es weiter, dann in der Form, in der es
//! kam. Nur in diesem Zustand kann `Ctrl+,` überhaupt ankommen; ohne ihn
//! sieht der Filter keine Folge und fängt nichts ab.
//!
//! Das Loslassen einer Taste (`CSI …:3u`, nur mit dem entsprechenden Flag)
//! geht unverändert weiter und ändert keinen Zustand. Steht ein Präfix aus,
//! wartet es hinter dem Präfix, damit die Reihenfolge bleibt; das Loslassen
//! des Präfixes selbst fällt weg, wenn die Tabelle das Präfix verbraucht.
//! Über einer Ansicht wird es verworfen, weil dort nichts an den Agenten
//! geht. Die Bits für Caps Lock und Num Lock im Feld der Modifikatoren zählen
//! nicht: `CSI 98;133u` ist `Ctrl+B` mit Num Lock.
//!
//! # Eingefügter Text
//!
//! Zwischen `CSI 200~` und `CSI 201~` (bracketed paste) ist nichts eine
//! Taste: Der Text geht Byte für Byte an den Agenten, auch über
//! Lesegrenzen, und ein `0x02 d` darin hängt nicht ab.
//!
//! Ein Terminal schickt einen eingefügten Text in einem Schwall. Fehlt das
//! Ende (`CSI 201~` ging verloren, oder ein Programm schrieb nur den Anfang),
//! endet der eingefügte Text, sobald [`PASTE_IDLE`] lang keine Eingabe kam;
//! sonst bliebe jede spätere Taste, auch `Ctrl+B d`, für immer Text.

use humanitl_config::DetachKeys;
use humanitl_config::prefix::{PrefixAction, binding};

/// `ESC`.
const ESC: u8 = 0x1b;

/// Wie lang eine Escape-Folge der Tastatur höchstens wird, bevor sie als
/// gewöhnliche Bytes weitergeht.
const MAX_SEQUENCE: usize = 32;

/// Die Modifikatoren `Ctrl` allein im kitty-Protokoll (`1 + 4`).
const KITTY_CTRL: u32 = 5;

/// Keine Modifikatoren im kitty-Protokoll.
const KITTY_NONE: u32 = 1;

/// Nur `Shift` im kitty-Protokoll (`1 + 1`).
const KITTY_SHIFT: u32 = 2;

/// Das Ereignis „losgelassen" im kitty-Protokoll.
const KITTY_RELEASE: u32 = 3;

/// Die Bits für Caps Lock (64) und Num Lock (128) im Feld der
/// Modifikatoren; sie ändern nicht, welche Taste gemeint ist.
const KITTY_LOCKS: u32 = 64 | 128;

/// Der Anfang eines eingefügten Textes (bracketed paste).
const PASTE_START: &[u8] = b"\x1b[200~";

/// Das Ende eines eingefügten Textes.
const PASTE_END: &[u8] = b"\x1b[201~";

/// Wie lange ein eingefügter Text ohne Eingabe offen bleibt, bevor der
/// Filter ihn für beendet hält. Ein Terminal schickt ihn in einem Schwall;
/// eine halbe Sekunde ohne Byte darin ist schon ein Mensch, der tippt.
pub const PASTE_IDLE: std::time::Duration = std::time::Duration::from_millis(500);

/// Was der Anschluss nach einem Lesevorgang tut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Abhängen; was danach im selben Stück stand, ist verworfen.
    Detach,
    /// Eine Ansicht öffnen: Hilfe, Einstellungen, Interceptor.
    Open(PrefixAction),
    /// Die offene Ansicht schließen; die Taste dafür geht nicht weiter.
    Close,
}

/// Was von einem Lesevorgang hinaufgeht, und was der Anschluss tut.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Filtered {
    /// Die Bytes für den Agenten, in ihrer Reihenfolge.
    pub forward: Vec<u8>,
    /// Die Ereignisse in ihrer Reihenfolge; [`KeyEvent::Detach`] steht
    /// höchstens einmal und zuletzt.
    pub events: Vec<KeyEvent>,
}

impl Filtered {
    /// Ob dieses Stück abhängt.
    pub fn detaches(&self) -> bool {
        self.events.contains(&KeyEvent::Detach)
    }
}

/// Eine Taste im kitty-Protokoll: `CSI Code[:Shift] ; Mod[:Ereignis] u`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KittyKey {
    /// Der Code der Taste ohne Shift (`98` für `b`).
    code: u32,
    /// Der Code mit Shift, wenn das Terminal ihn mitschickt (`63` für `?`).
    shifted: Option<u32>,
    /// Die Modifikatoren plus eins; `1` ist keiner.
    mods: u32,
    /// `1` gedrückt, `2` wiederholt, `3` losgelassen.
    event: u32,
}

impl KittyKey {
    /// Liest eine vollständige Folge; `None`, wenn sie keine Taste des
    /// Protokolls ist (eine Pfeiltaste, ein Mausereignis).
    fn parse(token: &[u8]) -> Option<Self> {
        let body = token.strip_prefix(b"\x1b[")?.strip_suffix(b"u")?;
        if body.is_empty()
            || !body
                .iter()
                .all(|byte| byte.is_ascii_digit() || *byte == b';' || *byte == b':')
        {
            return None;
        }
        let text = std::str::from_utf8(body).ok()?;
        let mut fields = text.split(';');
        let mut key = fields.next().unwrap_or_default().split(':');
        let code = key.next()?.parse().ok()?;
        let shifted = key.next().and_then(|value| value.parse().ok());
        let mut modifiers = fields.next().unwrap_or_default().split(':');
        let mods = modifiers
            .next()
            .filter(|value| !value.is_empty())
            .map_or(Some(KITTY_NONE), |value| value.parse().ok())
            .map(without_locks)?;
        let event = modifiers
            .next()
            .filter(|value| !value.is_empty())
            .map_or(Some(1), |value| value.parse().ok())?;
        Some(Self {
            code,
            shifted,
            mods,
            event,
        })
    }

    /// Ob die Taste losgelassen wurde.
    const fn released(self) -> bool {
        self.event == KITTY_RELEASE
    }
}

/// Die Modifikatoren ohne Caps Lock und Num Lock, wieder plus eins.
const fn without_locks(mods: u32) -> u32 {
    (mods.saturating_sub(1) & !KITTY_LOCKS) + 1
}

/// Sucht eine feste Folge in einem Strom von Bytes, über Lesegrenzen.
#[derive(Debug, Default)]
struct Matcher {
    /// Wie viele Bytes der Folge schon passten.
    matched: usize,
}

impl Matcher {
    /// Ein Byte; wahr, wenn damit die ganze Folge dasteht. Die Folgen hier
    /// beginnen mit `ESC` und enthalten keines weiter, also genügt es, bei
    /// einem Fehlschlag mit `ESC` neu anzufangen.
    fn advance(&mut self, pattern: &[u8], byte: u8) -> bool {
        if pattern.get(self.matched) == Some(&byte) {
            self.matched += 1;
        } else {
            self.matched = usize::from(byte == pattern[0]);
        }
        if self.matched == pattern.len() {
            self.matched = 0;
            return true;
        }
        false
    }
}

/// Wie lang die Taste am Anfang von `input` ist, und ob sie vollständig
/// dasteht. Ohne kitty-Protokoll ist jede Taste ein Byte.
fn next_key(input: &[u8], kitty: bool) -> (usize, bool) {
    if !kitty || input.first() != Some(&ESC) {
        return (1, true);
    }
    match input.get(1) {
        None => return (1, false),
        Some(b'[') => {}
        // Ein `ESC` vor etwas anderem ist eine Taste für sich.
        Some(_) => return (1, true),
    }
    for (index, byte) in input.iter().enumerate().skip(2) {
        if index >= MAX_SEQUENCE {
            return (index, true);
        }
        match byte {
            0x20..=0x3f => {}
            0x40..=0x7e => return (index + 1, true),
            // Weder Parameter noch Endzeichen: Was bis hier stand, geht als
            // eine Taste weiter, das Byte wird neu betrachtet.
            _ => return (index, true),
        }
    }
    (input.len(), input.len() >= MAX_SEQUENCE)
}

/// Die Tastatur hinter dem Präfix.
#[derive(Debug)]
pub struct PrefixFilter {
    /// Das Präfix als Byte, oder `None` ohne Präfixtaste.
    prefix: Option<u8>,
    /// Der Buchstabe des Präfixes (`b` für `Ctrl+B`).
    letter: u8,
    /// Ob `Ctrl+,` im kitty-Protokoll direkt die Einstellungen öffnet.
    direct: bool,
    /// Die Bytes des Präfixes, wenn es gelesen ist und die Taste danach
    /// noch aussteht, in der Form, in der es kam.
    pending: Option<Vec<u8>>,
    /// Eine angefangene Escape-Folge vom Ende des letzten Lesevorgangs.
    partial: Vec<u8>,
    /// Losgelassene Tasten, die hinter einem ausstehenden Präfix warten.
    released: Vec<Vec<u8>>,
    /// Wahr, solange eine Ansicht offen ist: Die nächste Taste schließt sie.
    overlay: bool,
    /// Wahr zwischen `CSI 200~` und `CSI 201~`.
    pasting: bool,
    /// Wann zuletzt Eingabe kam, für [`PASTE_IDLE`].
    last_input: Option<std::time::Instant>,
    /// Sucht den Anfang eines eingefügten Textes.
    paste_start: Matcher,
    /// Sucht sein Ende.
    paste_end: Matcher,
}

impl PrefixFilter {
    /// Ein Filter für dieses Präfix, ohne gelesenes Präfix.
    pub const fn new(keys: DetachKeys, direct: bool) -> Self {
        let (prefix, letter) = match keys {
            DetachKeys::Prefix(byte) => (Some(byte), (byte | 0x40).to_ascii_lowercase()),
            DetachKeys::Disabled => (None, 0),
        };
        Self {
            prefix,
            letter,
            direct,
            pending: None,
            partial: Vec::new(),
            released: Vec::new(),
            overlay: false,
            pasting: false,
            last_input: None,
            paste_start: Matcher { matched: 0 },
            paste_end: Matcher { matched: 0 },
        }
    }

    /// Filtert ein gelesenes Stück; `kitty` sagt, ob der Agent das
    /// kitty-Tastaturprotokoll gerade eingeschaltet hat.
    pub fn feed(&mut self, chunk: &[u8], kitty: bool) -> Filtered {
        self.feed_at(chunk, kitty, std::time::Instant::now())
    }

    /// Wie [`PrefixFilter::feed`], mit dem Zeitpunkt des Lesens: Liegt der
    /// letzte mehr als [`PASTE_IDLE`] zurück, endet ein offener eingefügter
    /// Text vor diesem Stück.
    pub fn feed_at(&mut self, chunk: &[u8], kitty: bool, now: std::time::Instant) -> Filtered {
        if self.pasting
            && self
                .last_input
                .is_some_and(|last| now.saturating_duration_since(last) >= PASTE_IDLE)
        {
            self.pasting = false;
            self.paste_end = Matcher::default();
        }
        self.last_input = Some(now);
        let mut out = Filtered::default();
        // Ohne Präfix gibt es keine Tabelle und keine direkte Taste: Jedes
        // Byte gehört dem Agenten, wie mit `--detach-keys none` vor #10.
        if self.prefix.is_none() {
            out.forward.extend_from_slice(chunk);
            return out;
        }
        let mut input = std::mem::take(&mut self.partial);
        input.extend_from_slice(chunk);
        let mut rest = input.as_slice();
        while !rest.is_empty() {
            if self.pasting {
                rest = self.paste(rest, &mut out);
                continue;
            }
            let (len, complete) = next_key(rest, kitty);
            if !complete {
                self.partial = rest.to_vec();
                break;
            }
            let (key, tail) = rest.split_at(len);
            rest = tail;
            self.step(key, &mut out);
            if out.detaches() {
                break;
            }
            for &byte in key {
                if self.paste_start.advance(PASTE_START, byte) {
                    self.pasting = true;
                }
            }
        }
        out
    }

    /// Eingefügter Text: unverändert an den Agenten bis einschließlich
    /// `CSI 201~`; gibt zurück, was danach im Stück steht.
    fn paste<'a>(&mut self, rest: &'a [u8], out: &mut Filtered) -> &'a [u8] {
        for (index, &byte) in rest.iter().enumerate() {
            out.forward.push(byte);
            if self.paste_end.advance(PASTE_END, byte) {
                self.pasting = false;
                return &rest[index + 1..];
            }
        }
        &[]
    }

    /// Eine Taste.
    fn step(&mut self, key: &[u8], out: &mut Filtered) {
        let kitty = KittyKey::parse(key);
        let released = kitty.is_some_and(KittyKey::released);
        if self.overlay {
            if !released {
                self.overlay = false;
                out.events.push(KeyEvent::Close);
            }
            return;
        }
        if released {
            // Hinter einem ausstehenden Präfix wartet das Loslassen, sonst
            // käme es vor dem Drücken an.
            if self.pending.is_some() {
                self.released.push(key.to_vec());
            } else {
                out.forward.extend_from_slice(key);
            }
            return;
        }
        if let Some(first) = self.pending.take() {
            let waiting = std::mem::take(&mut self.released);
            let typed = typed(key, kitty);
            match typed.and_then(binding) {
                Some(found) => {
                    // Das Präfix ist verbraucht, also auch sein Loslassen;
                    // das Loslassen anderer Tasten geht weiter.
                    for release in waiting {
                        if !self.is_prefix(&release, KittyKey::parse(&release)) {
                            out.forward.extend_from_slice(&release);
                        }
                    }
                    if found.action == PrefixAction::Detach {
                        out.events.push(KeyEvent::Detach);
                    } else {
                        self.overlay = true;
                        out.events.push(KeyEvent::Open(found.action));
                    }
                }
                // Das Präfix zweimal oder sein Buchstabe: einmal das Präfix.
                None if self.is_prefix(key, kitty) || typed == Some(self.letter) => {
                    out.forward.extend_from_slice(&first);
                    out.forward.extend(waiting.concat());
                }
                // Jede andere Taste geht mit dem Präfix davor weiter.
                None => {
                    out.forward.extend_from_slice(&first);
                    out.forward.extend(waiting.concat());
                    out.forward.extend_from_slice(key);
                }
            }
            return;
        }
        if self.is_prefix(key, kitty) {
            self.pending = Some(key.to_vec());
        } else if self.direct
            && kitty.is_some_and(|found| found.code == u32::from(b',') && found.mods == KITTY_CTRL)
        {
            self.overlay = true;
            out.events.push(KeyEvent::Open(PrefixAction::Settings));
        } else {
            out.forward.extend_from_slice(key);
        }
    }

    /// Ob diese Taste das Präfix ist: das Byte, oder im kitty-Protokoll
    /// `Ctrl` mit seinem Buchstaben (gedrückt oder losgelassen).
    fn is_prefix(&self, key: &[u8], kitty: Option<KittyKey>) -> bool {
        self.prefix.is_some_and(|prefix| key == [prefix])
            || kitty.is_some_and(|found| {
                found.mods == KITTY_CTRL && found.code == u32::from(self.letter)
            })
    }

    /// Was am Ende der Eingabe noch aussteht: ein Präfix ohne Taste danach
    /// und eine angefangene Folge. Beides gehört dem Agenten.
    pub fn finish(&mut self) -> Vec<u8> {
        let mut rest = self.pending.take().unwrap_or_default();
        for release in std::mem::take(&mut self.released) {
            rest.extend(release);
        }
        rest.append(&mut self.partial);
        if self.overlay {
            rest.clear();
        }
        rest
    }
}

/// Das Zeichen, das diese Taste ohne Modifikator tippt, für die Tabelle.
fn typed(key: &[u8], kitty: Option<KittyKey>) -> Option<u8> {
    if let [byte] = key {
        return Some(*byte);
    }
    let found = kitty?;
    let code = match (found.mods, found.shifted) {
        (KITTY_SHIFT, Some(shifted)) => shifted,
        // Mit Flag 8 ohne Flag 4 kommt nur der Code ohne Shift: `?` als
        // `CSI 47;2u`. Die Zeichen der Tabelle mit Shift werden hier nach der
        // US-Belegung zurückgerechnet.
        (KITTY_SHIFT, None) => return u8::try_from(found.code).ok().and_then(shifted_us),
        (KITTY_NONE, _) => found.code,
        _ => return None,
    };
    u8::try_from(code).ok().filter(u8::is_ascii)
}

/// Das Zeichen mit Shift auf der US-Belegung, für die Zeichen, die die
/// Tabelle mit Shift tippt; sonst `None`.
const fn shifted_us(code: u8) -> Option<u8> {
    match code {
        b'/' => Some(b'?'),
        b'a'..=b'z' => Some(code.to_ascii_uppercase()),
        _ => None,
    }
}

/// Ob in einem Stück `Ctrl+C` steht: das Byte, oder im kitty-Protokoll die
/// Folge dafür. Ein lesender Anschluss hängt damit ab.
pub fn has_ctrl_c(chunk: &[u8]) -> bool {
    chunk.contains(&0x03)
        || chunk
            .windows(7)
            .any(|window| window == b"\x1b[99;5u" || window.starts_with(b"\x1b[99;5:"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use humanitl_config::DetachKeys;
    use humanitl_config::prefix::PrefixAction;

    use super::{Filtered, KeyEvent, PrefixFilter, has_ctrl_c};
    use crate::prefix::stream::StreamState;

    /// `Ctrl+B`, die Vorgabe.
    const CTRL_B: u8 = 0x02;

    /// Ein Filter mit der Vorgabe `Ctrl+B` und `Ctrl+,` direkt.
    fn ctrl_b() -> PrefixFilter {
        PrefixFilter::new(DetachKeys::Prefix(CTRL_B), true)
    }

    /// Was hinaufgeht, ohne Ereignis.
    fn forwarded(bytes: &[u8]) -> Filtered {
        Filtered {
            forward: bytes.to_vec(),
            events: Vec::new(),
        }
    }

    /// Nur ein Ereignis, nichts für den Agenten.
    fn event(event: KeyEvent) -> Filtered {
        Filtered {
            forward: Vec::new(),
            events: vec![event],
        }
    }

    /// Jede Taste der Tabelle nach dem Präfix: `d` hängt ab, `,`, `i` und
    /// `?` öffnen ihre Ansicht, und die nächste Taste schließt sie, ohne
    /// den Agenten zu erreichen. Was vor dem Präfix stand, geht hinauf.
    #[test]
    fn every_key_of_the_table_after_the_prefix() {
        assert_eq!(
            ctrl_b().feed(b"ls\x02d", false),
            Filtered {
                forward: b"ls".to_vec(),
                events: vec![KeyEvent::Detach],
            }
        );
        // Was nach `d` im selben Stück steht, ist verworfen.
        assert_eq!(ctrl_b().feed(b"\x02dxyz", false), event(KeyEvent::Detach));
        for (key, action) in [
            (b',', PrefixAction::Settings),
            (b'i', PrefixAction::Interceptor),
            (b'?', PrefixAction::Help),
        ] {
            let mut filter = ctrl_b();
            assert_eq!(
                filter.feed(&[CTRL_B, key], false),
                event(KeyEvent::Open(action))
            );
            assert_eq!(filter.feed(b"q", false), event(KeyEvent::Close));
            assert_eq!(filter.feed(b"q", false), forwarded(b"q"), "closed");
        }
    }

    /// Das Präfix zweimal und sein Buchstabe schicken genau ein Präfix an
    /// den Agenten, und danach steht kein Präfix mehr aus.
    /// Mutationsbeweis: zwei Präfixe für zweimal drücken machen ihn rot.
    #[test]
    fn the_prefix_twice_sends_it_once() {
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"\x02\x02", false), forwarded(&[CTRL_B]));
        assert_eq!(filter.feed(b"d", false), forwarded(b"d"), "nothing pending");
        assert_eq!(ctrl_b().feed(b"\x02b", false), forwarded(&[CTRL_B]));
        // Über eine Lesegrenze ebenso.
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"\x02", false), forwarded(b""));
        assert_eq!(filter.feed(b"\x02", false), forwarded(&[CTRL_B]));
    }

    /// Das Präfix am Ende eines Lesevorgangs wartet ohne Frist auf den
    /// nächsten; die Taste darin wirkt trotzdem. Am Ende der Eingabe gehört
    /// es dem Agenten.
    #[test]
    fn a_prefix_at_the_end_of_a_read_waits() {
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"x\x02", false), forwarded(b"x"));
        assert_eq!(filter.feed(b"d", false), event(KeyEvent::Detach));
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"\x02", false), forwarded(b""));
        assert_eq!(filter.finish(), vec![CTRL_B]);
    }

    /// Jede andere Taste geht mit dem Präfix davor unverändert weiter,
    /// auch eine Pfeiltaste und ein `Ctrl+A`, das jetzt kein Präfix ist.
    #[test]
    fn any_other_key_goes_on_with_the_prefix() {
        assert_eq!(ctrl_b().feed(b"\x02x", false), forwarded(b"\x02x"));
        assert_eq!(ctrl_b().feed(b"\x02D", false), forwarded(b"\x02D"));
        assert_eq!(ctrl_b().feed(b"\x02\x03", false), forwarded(b"\x02\x03"));
        assert_eq!(
            ctrl_b().feed(b"\x02\x1b[A", false),
            forwarded(b"\x02\x1b[A")
        );
        assert_eq!(ctrl_b().feed(b"\x01d", false), forwarded(b"\x01d"));
    }

    /// `none`: Jedes Byte geht hinauf, auch `Ctrl+,` im kitty-Protokoll.
    #[test]
    fn none_disables_the_prefix_and_the_direct_key() {
        let mut filter = PrefixFilter::new(DetachKeys::Disabled, true);
        assert_eq!(filter.feed(b"\x02d", false), forwarded(b"\x02d"));
        assert_eq!(filter.feed(b"\x1b[44;5u", true), forwarded(b"\x1b[44;5u"));
        assert!(filter.finish().is_empty());
    }

    /// Ein anderes Präfix hat seinen eigenen Buchstaben; ist der Buchstabe
    /// eine Taste der Tabelle, gilt die Tabelle (`Ctrl+D d` hängt ab).
    #[test]
    fn another_prefix_has_its_own_letter() {
        let mut filter = PrefixFilter::new(DetachKeys::Prefix(0x1d), true);
        assert_eq!(filter.feed(b"\x02d", false), forwarded(b"\x02d"));
        assert_eq!(filter.feed(b"\x1d]", false), forwarded(b"\x1d"));
        assert!(filter.feed(b"\x1dd", false).detaches());
        let mut filter = PrefixFilter::new(DetachKeys::Prefix(0x04), true);
        assert!(filter.feed(b"\x04d", false).detaches());
        assert_eq!(filter.feed(b"\x04\x04", false), forwarded(b"\x04"));
    }

    /// kitty: Hat der Agent das Protokoll eingeschaltet (`CSI > 1 u` im
    /// Strom), öffnet `CSI 44;5u` von der Tastatur die Einstellungen; ohne
    /// das Einschalten geht dieselbe Folge an den Agenten, und nach dem
    /// Abschalten (`CSI < u`) wieder. Mutationsbeweis: abfangen, ohne den
    /// Zustand zu prüfen, macht den zweiten Fall rot.
    #[test]
    fn ctrl_comma_opens_the_settings_only_under_the_kitty_protocol() {
        let ctrl_comma = b"\x1b[44;5u";

        let mut stream = StreamState::new(Arc::new(AtomicBool::new(false)));
        stream.observe(b"\x1b[>1u");
        let mut filter = ctrl_b();
        assert_eq!(
            filter.feed(ctrl_comma, stream.kitty()),
            event(KeyEvent::Open(PrefixAction::Settings))
        );

        let fresh = StreamState::new(Arc::new(AtomicBool::new(false)));
        assert_eq!(
            ctrl_b().feed(ctrl_comma, fresh.kitty()),
            forwarded(ctrl_comma),
            "without the protocol the sequence belongs to the agent"
        );

        stream.observe(b"\x1b[<u");
        assert_eq!(
            ctrl_b().feed(ctrl_comma, stream.kitty()),
            forwarded(ctrl_comma),
            "switched off again"
        );

        // `terminal.direct_keys = false` lässt sie auch im Protokoll durch.
        let mut off = PrefixFilter::new(DetachKeys::Prefix(CTRL_B), false);
        assert_eq!(off.feed(ctrl_comma, true), forwarded(ctrl_comma));
    }

    /// Im kitty-Protokoll kommt das Präfix als `CSI 98;5u`: Es wirkt wie das
    /// Byte, geht zweimal gedrückt einmal und in seiner Form weiter, eine
    /// geteilte Folge wartet auf den Rest, und das Loslassen geht durch.
    #[test]
    fn the_prefix_works_under_the_kitty_protocol() {
        let prefix = b"\x1b[98;5u";
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"\x1b[98;5ud", true), event(KeyEvent::Detach));
        let mut filter = ctrl_b();
        assert_eq!(
            filter.feed(b"\x1b[98;5u\x1b[98;5u", true),
            forwarded(prefix)
        );
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"\x1b[98;5u\x1b[98;5", true), forwarded(b""));
        assert_eq!(filter.feed(b"u", true), forwarded(prefix));
        // Losgelassen zwischen Präfix und Taste: Das Loslassen des
        // verbrauchten Präfixes fällt weg.
        let mut filter = ctrl_b();
        assert_eq!(
            filter.feed(b"\x1b[98;5u\x1b[98;5:3u\x1b[100u", true),
            event(KeyEvent::Detach)
        );
        // `?` mit Shift und mitgeschicktem Zeichen.
        let mut filter = ctrl_b();
        assert_eq!(
            filter.feed(b"\x02\x1b[47:63;2u", true),
            event(KeyEvent::Open(PrefixAction::Help))
        );
        // Eine Pfeiltaste nach dem Präfix geht als Ganzes mit ihm weiter.
        assert_eq!(ctrl_b().feed(b"\x02\x1b[A", true), forwarded(b"\x02\x1b[A"));
    }

    /// Ein Loslassen zwischen Präfix und nächster Taste wartet hinter dem
    /// Präfix: Geht das Präfix weiter, kommt es danach, nie davor.
    /// Mutationsbeweis: das Loslassen sofort weiterreichen macht ihn rot.
    #[test]
    fn a_release_waits_behind_the_pending_prefix() {
        let mut filter = ctrl_b();
        assert_eq!(filter.feed(b"\x1b[98;5u", true), forwarded(b""));
        assert_eq!(filter.feed(b"\x1b[98;5:3u", true), forwarded(b""));
        assert_eq!(
            filter.feed(b"x", true),
            forwarded(b"\x1b[98;5u\x1b[98;5:3ux")
        );
    }

    /// Caps Lock (64) und Num Lock (128) im Feld der Modifikatoren ändern
    /// nichts: `CSI 98;133u` und `CSI 98;69u` sind `Ctrl+B`, `CSI 44;133u`
    /// ist `Ctrl+,`. Mutationsbeweis: ohne Maske rot.
    #[test]
    fn lock_bits_do_not_change_the_key() {
        for prefix in [&b"\x1b[98;133u"[..], b"\x1b[98;69u"] {
            let mut filter = ctrl_b();
            assert_eq!(filter.feed(prefix, true), forwarded(b""), "{prefix:?}");
            assert_eq!(filter.feed(b"d", true), event(KeyEvent::Detach));
        }
        for comma in [&b"\x1b[44;133u"[..], b"\x1b[44;69u"] {
            assert_eq!(
                ctrl_b().feed(comma, true),
                event(KeyEvent::Open(PrefixAction::Settings))
            );
        }
        // Und `d` mit Num Lock unter Flag 8.
        let mut filter = ctrl_b();
        assert_eq!(
            filter.feed(b"\x02\x1b[100;129u", true),
            event(KeyEvent::Detach)
        );
    }

    /// Mit Flag 8 ohne Flag 4 kommt `?` als `CSI 47;2u` (Code ohne Shift):
    /// Es öffnet die Hilfe und geht nicht an den Agenten. Mutationsbeweis:
    /// ohne Rückrechnung rot.
    #[test]
    fn shift_slash_under_flag_8_is_the_question_mark() {
        let mut stream = StreamState::new(Arc::new(AtomicBool::new(false)));
        stream.observe(b"\x1b[>8u");
        let mut filter = ctrl_b();
        assert_eq!(
            filter.feed(b"\x1b[98;5u\x1b[47;2u", stream.kitty()),
            event(KeyEvent::Open(PrefixAction::Help))
        );
    }

    /// Eingefügter Text ist nie eine Taste, auch über Lesegrenzen: Ein
    /// `Ctrl+B d` darin geht an den Agenten und hängt nicht ab; nach dem Ende
    /// gilt die Tabelle wieder. Mutationsbeweis: ohne Erkennung rot.
    #[test]
    fn pasted_text_is_never_a_key() {
        for kitty in [false, true] {
            let mut filter = ctrl_b();
            let mut forward = Vec::new();
            for part in [&b"\x1b[20"[..], b"0~a\x02", b"d\x1b[2", b"01~"] {
                let out = filter.feed(part, kitty);
                assert!(out.events.is_empty(), "kitty={kitty} {part:?}: {out:?}");
                forward.extend(out.forward);
            }
            assert_eq!(forward, b"\x1b[200~a\x02d\x1b[201~", "kitty={kitty}");
            assert!(filter.feed(b"\x02d", kitty).detaches(), "kitty={kitty}");
        }
    }

    /// Ein eingefügter Text ohne Ende hält die Tastatur nicht fest: Nach
    /// einer Pause von [`super::PASTE_IDLE`] ist `Ctrl+B d` wieder eine
    /// Taste und hängt ab; ohne Pause bleibt es Text. Mutationsbeweis: ohne
    /// die Rückkehr rot.
    #[test]
    fn an_unterminated_paste_ends_after_a_pause() {
        let start = std::time::Instant::now();
        let mut filter = ctrl_b();
        let out = filter.feed_at(b"\x1b[200~text", false, start);
        assert_eq!(out, forwarded(b"\x1b[200~text"));
        let soon = start + super::PASTE_IDLE / 2;
        assert_eq!(filter.feed_at(b"\x02d", false, soon), forwarded(b"\x02d"));
        let later = soon + super::PASTE_IDLE;
        assert!(filter.feed_at(b"\x02d", false, later).detaches());
    }

    /// Ein lesender Anschluss erkennt `Ctrl+C` als Byte und als Folge.
    #[test]
    fn ctrl_c_is_seen_in_both_forms() {
        assert!(has_ctrl_c(b"a\x03"));
        assert!(has_ctrl_c(b"\x1b[99;5u"));
        assert!(has_ctrl_c(b"\x1b[99;5:1u"));
        assert!(!has_ctrl_c(b"\x1b[98;5u"));
    }
}
