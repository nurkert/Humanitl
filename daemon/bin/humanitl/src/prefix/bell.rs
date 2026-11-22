//! Die Glocke bei einer neu gehaltenen Anfrage (GitHub #12).
//!
//! Wartet eine Anfrage des Agenten auf einen Menschen, schreibt der Anschluss
//! keinen Text in das Terminal: Es gehört dem Agenten, und eine Zeile quer
//! über einem Vollbild-Agenten wäre die Störung, gegen die der Rahmen
//! `TerminalOutput.notice` gebaut ist. Stattdessen klingelt er, ein `BEL`,
//! wenn `terminal.bell_on_hold` gilt (Vorgabe `true`). Die Oberfläche und die
//! Benachrichtigung des Systems melden die Anfrage wie bisher.
//!
//! Was hier steht, ist nur die Drossel: höchstens eine Glocke je
//! [`BELL_INTERVAL`], damit zehn Anfragen in einer Sekunde nicht zehnmal
//! klingeln. Wohin das `BEL` geht und warum es nie in einer Steuerfolge des
//! Agenten landet, steht an [`super::view::AgentView::hold_bell`]; woher der
//! Anschluss von der Anfrage weiß (`Subscribe`, `FlowEvent.held`), in
//! `cmd/attach.rs`.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use super::view::AgentView;

/// Wie lange nach einer Glocke keine weitere klingelt.
pub const BELL_INTERVAL: Duration = Duration::from_secs(1);

/// Die Glocke eines Anschlusses: an oder aus, und wann sie zuletzt klang.
#[derive(Debug, Clone, Copy)]
pub struct HoldBell {
    /// `terminal.bell_on_hold`.
    enabled: bool,
    /// Wann zuletzt geklingelt wurde.
    last: Option<Instant>,
}

impl HoldBell {
    /// Eine Glocke, die nach `enabled` klingelt oder schweigt.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last: None,
        }
    }

    /// Ob die Glocke überhaupt klingelt; ohne sie braucht der Anschluss
    /// kein Abonnement der Ereignisse.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Ob jetzt (`now`) geklingelt werden darf: eingeschaltet, und nie
    /// zuvor oder die letzte Glocke liegt mindestens [`BELL_INTERVAL`]
    /// zurück.
    #[must_use]
    pub fn due(&self, now: Instant) -> bool {
        self.enabled
            && self
                .last
                .is_none_or(|last| now.saturating_duration_since(last) >= BELL_INTERVAL)
    }

    /// Eine neu gehaltene Anfrage um `now`: klingelt über `view`, wenn die
    /// Drossel es erlaubt und keine Ansicht steht. Gemerkt wird nur eine
    /// Glocke, die wirklich klang; eine, die eine Ansicht verschluckt hat,
    /// hält die nächste nicht auf.
    ///
    /// Aufgerufen wird sie aus der Schleife des Anschlusses zwischen zwei
    /// Nachrichten des Terminal-Stroms, also zwischen zwei ganzen Stücken der
    /// Ausgabe des Agenten.
    ///
    /// # Errors
    ///
    /// Wenn das Terminal nicht mehr schreibt.
    pub fn held<W: Write>(&mut self, view: &mut AgentView<W>, now: Instant) -> io::Result<()> {
        // Wartet schon eine Glocke auf das Ende eines UTF-8-Zeichens, kommt
        // keine zweite dazu.
        if self.due(now) && !view.bell_pending() && view.hold_bell()? {
            self.last = Some(now);
        }
        Ok(())
    }

    /// Nach jedem Stück der Ausgabe um `now`: Hat es eine aufgeschobene
    /// Glocke hinausgeschrieben, zählt die Drossel ab jetzt und nicht ab der
    /// Anfrage (Review zu GitHub #12). Sonst klänge eine Glocke, die bei 0,9 s
    /// hinausging, bei 1,0 s schon wieder.
    pub fn settle<W: Write>(&mut self, view: &mut AgentView<W>, now: Instant) {
        if view.take_bell_written() {
            self.last = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    use humanitl_config::DetachKeys;

    use super::{BELL_INTERVAL, HoldBell};
    use crate::prefix::stream::StreamState;
    use crate::prefix::view::AgentView;
    use crate::tui::Theme;

    const BEL: u8 = 0x07;

    fn view() -> AgentView<Vec<u8>> {
        AgentView::new(
            Vec::new(),
            StreamState::new(Arc::new(AtomicBool::new(false))),
            DetachKeys::Prefix(0x02),
            Theme::from_no_color(Some(std::ffi::OsStr::new("1"))),
        )
    }

    /// Ob ein `BEL` innerhalb einer CSI-Folge (`ESC [` bis zum Endbyte)
    /// steht.
    fn bel_inside_csi(bytes: &[u8]) -> bool {
        let mut in_csi = false;
        let mut after_esc = false;
        for &byte in bytes {
            if in_csi {
                if byte == BEL {
                    return true;
                }
                if (0x40..=0x7e).contains(&byte) {
                    in_csi = false;
                }
                continue;
            }
            in_csi = after_esc && byte == b'[';
            after_esc = byte == 0x1b;
        }
        false
    }

    #[allow(clippy::naive_bytecount, reason = "a few hundred bytes in a test")]
    fn bells(bytes: &[u8]) -> usize {
        bytes.iter().filter(|byte| **byte == BEL).count()
    }

    /// Eine neue gehaltene Anfrage klingelt genau einmal, und das `BEL` steht
    /// zwischen zwei Stücken, nicht in der Farbfolge des Agenten.
    #[test]
    fn a_new_hold_rings_once_between_two_chunks() {
        let mut view = view();
        let mut bell = HoldBell::new(true);
        let first = b"\x1b[38;5;196mred ";
        view.agent(first).unwrap();
        bell.held(&mut view, Instant::now()).unwrap();
        view.agent(b"\x1b[0m done").unwrap();
        let out = view.out();
        assert_eq!(bells(out), 1, "{out:?}");
        assert_eq!(out[first.len()], BEL, "the bell stands at the chunk border");
        assert!(!bel_inside_csi(out), "{out:?}");
    }

    /// Mit `terminal.bell_on_hold = false` klingelt nichts.
    #[test]
    fn without_the_setting_nothing_rings() {
        let mut view = view();
        let mut bell = HoldBell::new(false);
        view.agent(b"text").unwrap();
        bell.held(&mut view, Instant::now()).unwrap();
        assert_eq!(bells(view.out()), 0);
        assert!(!bell.enabled());
    }

    /// Zehn Anfragen innerhalb einer Sekunde klingeln einmal; nach der
    /// Sekunde klingelt die nächste wieder. Mutationsbeweis: ohne die
    /// Drossel klingeln alle zehn, und der Test wird rot.
    #[test]
    fn ten_holds_within_a_second_ring_once() {
        let start = Instant::now();
        let mut view = view();
        let mut bell = HoldBell::new(true);
        for step in 0..10_u64 {
            view.agent(format!("line {step}\r\n").as_bytes()).unwrap();
            bell.held(&mut view, start + Duration::from_millis(step * 90))
                .unwrap();
        }
        assert_eq!(
            bells(view.out()),
            1,
            "ten holds within one second ring once"
        );
        bell.held(&mut view, start + BELL_INTERVAL).unwrap();
        assert_eq!(bells(view.out()), 2, "a second later it rings again");
    }

    /// Endet ein Stück mitten in einem UTF-8-Zeichen, klingelt die Glocke
    /// erst hinter dem Stück, das es vollendet, und nie dazwischen (Review zu
    /// GitHub #12). Mutationsbeweis: die Grenze nicht abwarten macht ihn rot.
    #[test]
    fn a_bell_waits_for_the_end_of_a_utf8_character() {
        let mut view = view();
        let mut bell = HoldBell::new(true);
        view.agent(b"a\xc3").unwrap();
        bell.held(&mut view, Instant::now()).unwrap();
        assert_eq!(bells(view.out()), 0, "not inside the character");
        view.agent(b"\xa4b").unwrap();
        assert_eq!(view.out(), b"a\xc3\xa4b\x07");
        view.agent(b"c").unwrap();
        assert_eq!(bells(view.out()), 1, "exactly one bell");
    }

    /// Eine aufgeschobene Glocke, die erst bei 0,9 s hinausgeht, zählt für die
    /// Drossel ab 0,9 s: Eine Anfrage bei 1,0 s klingelt nicht, eine bei
    /// 1,9 s wieder (Review zu GitHub #12). Mutationsbeweis: die Zeit des
    /// Schreibens nicht merken macht ihn rot.
    #[test]
    fn a_deferred_bell_throttles_from_when_it_rang() {
        let start = Instant::now();
        let mut view = view();
        let mut bell = HoldBell::new(true);
        view.agent(b"a\xc3").unwrap();
        bell.held(&mut view, start).unwrap();
        bell.held(&mut view, start + Duration::from_millis(10))
            .unwrap();
        view.agent(b"\xa4").unwrap();
        bell.settle(&mut view, start + Duration::from_millis(900));
        assert_eq!(
            bells(view.out()),
            1,
            "one deferred bell, no second one queued"
        );
        bell.held(&mut view, start + Duration::from_secs(1))
            .unwrap();
        assert_eq!(bells(view.out()), 1, "0.1 s after the bell rang");
        bell.held(&mut view, start + Duration::from_millis(1900))
            .unwrap();
        assert_eq!(bells(view.out()), 2, "a second after it rang");
    }

    /// Solange eine Ansicht steht, klingelt nichts, weder sofort noch in der
    /// zurückgehaltenen Ausgabe; nach der Rückkehr steht kein `BEL` mitten in
    /// einer Folge des Agenten, und die verschluckte Glocke hält die nächste
    /// nicht auf. Mutationsbeweis: ein `BEL` mitten in den zurückgehaltenen
    /// Block geschrieben macht ihn rot.
    #[test]
    fn a_bell_never_lands_in_held_output() {
        let mut view = view();
        let mut bell = HoldBell::new(true);
        let now = Instant::now();
        view.agent(b"\x1b[?1049h").unwrap();
        view.open_screen().unwrap();
        let opened = view.out().len();
        view.agent(b"\x1b[38;5;").unwrap();
        bell.held(&mut view, now).unwrap();
        view.agent(b"196mred\x1b[0m").unwrap();
        assert_eq!(view.out().len(), opened, "nothing reached the terminal");
        view.close().unwrap();
        let out = view.out();
        assert_eq!(bells(out), 0, "{out:?}");
        assert!(!bel_inside_csi(out), "{out:?}");
        bell.held(&mut view, now).unwrap();
        assert_eq!(bells(view.out()), 1, "the swallowed bell did not count");
    }
}
