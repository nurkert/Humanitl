//! Was der Filter dem Agenten verweigert hat, gezählt (HUM-138).
//!
//! Der Shim legt jeden verweigerten `socket(2)`-Aufruf des Agenten seinem
//! Elternprozess vor, der zählt und mit `EPERM` antwortet
//! (`daemon/bin/humanitl-shim/src/refusals.rs`). Über denselben Deskriptor wie
//! die `CHECK`-Zeilen ([`crate::bridge_env`]) schreibt er, solange der Agent
//! läuft, drei Arten von Zeilen:
//!
//! ```text
//! REFUSALS on
//! REFUSALS off <grund>
//! REFUSED socket <familie> <typ> <family|type|overflow> <anzahl> <erste-ms> <letzte-ms>
//! ```
//!
//! Jede `REFUSED`-Zeile trägt den laufenden Stand je Paar aus Familie und
//! Typ, nicht einen einzelnen Versuch: Ein Agent in einer Schleife erzeugt
//! tausende Versuche je Sekunde, und der Shim fasst sie zusammen, bevor sie
//! den Wirt erreichen. [`Refusals::apply`] behält deshalb je Paar das Größte,
//! was gemeldet wurde; eine Zeile, die zu spät kommt, macht einen neueren
//! Stand nie kleiner.
//!
//! Was hier nicht steht, sieht auch niemand: Ein `connect(2)`, das an der
//! leeren Routing-Tabelle scheitert (`ENETUNREACH`), ist kein verweigerter
//! Aufruf, der Filter hört nie davon. Die Netzlosigkeit ist ein Zustand, den
//! die erste Garantie belegt, kein Ereignis je Versuch (`docs/SECURITY.md`).
//!
//! Die Zeilen kommen aus der Sandbox. Der Shim schreibt sie, ehe der Agent sie
//! erreichen könnte (der Elternprozess ist nicht „dumpable"), aber der Wirt
//! prüft trotzdem jedes Feld und nimmt höchstens [`MAX_REFUSAL_ENTRIES`]
//! Paare an: Eine Zeile, die nicht passt, zählt als fremde Zeile und ändert
//! nichts.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use humanitl_core::diagnostics::codes::SANDBOX_019;
use humanitl_core::{Diagnostic, Severity};

/// Das Präfix der Zustandszeile.
pub const REFUSALS_PREFIX: &str = "REFUSALS";

/// Das Präfix einer gezählten Verweigerung.
pub const REFUSED_PREFIX: &str = "REFUSED";

/// Wie viele Paare der Wirt annimmt: die sechzehn, die der Shim getrennt
/// zählt, und sein Sammelposten.
pub const MAX_REFUSAL_ENTRIES: usize = 17;

/// Welche Hälfte der Sperre verweigert hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// Die Familie steht nicht in `allow_families`.
    Family,
    /// Die Familie ist erlaubt, der Typ steht nicht in `allow_types`.
    Type,
    /// Der Sammelposten für alle Paare über der Grenze des Shims.
    Overflow,
}

impl RefusalReason {
    /// Der Name in der Zeile und im Protokoll.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Family => "family",
            Self::Type => "type",
            Self::Overflow => "overflow",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "family" => Some(Self::Family),
            "type" => Some(Self::Type),
            "overflow" => Some(Self::Overflow),
            _ => None,
        }
    }
}

/// Die verweigerten Versuche eines Paares aus Familie und Typ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Der verweigerte Aufruf; heute immer `socket`.
    pub syscall: String,
    /// Die Familie, etwa `AF_UNIX`; `*` für den Sammelposten.
    pub family: String,
    /// Der Typ, etwa `SOCK_DGRAM`; `*` für den Sammelposten.
    pub socket_type: String,
    /// Welche Hälfte der Sperre verweigert hat.
    pub reason: RefusalReason,
    /// Wie oft, bis jetzt.
    pub count: u64,
    /// Der erste Versuch.
    pub first: SystemTime,
    /// Der letzte Versuch.
    pub last: SystemTime,
}

impl Refusal {
    /// Ob `other` dasselbe Paar meint.
    fn same_pair(&self, other: &Self) -> bool {
        self.syscall == other.syscall
            && self.family == other.family
            && self.socket_type == other.socket_type
    }
}

/// Ob die Sandbox Verweigerungen meldet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RefusalReporting {
    /// Der Shim hat noch nichts dazu gesagt.
    #[default]
    Unknown,
    /// Der Filter legt verweigerte Aufrufe vor; was verweigert wird, steht in
    /// [`Refusals::entries`].
    On,
    /// Der Filter verweigert, ohne es zu melden; der Grund, wie der Shim ihn
    /// nennt (`errno16`, `no-channel`, ...).
    Off(String),
}

/// Eine gelesene Zeile über Verweigerungen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalLine {
    /// `REFUSALS on` oder `REFUSALS off <grund>`.
    Reporting(RefusalReporting),
    /// `REFUSED ...`.
    Refused(Refusal),
}

/// Was die Sandbox bis jetzt an Verweigerungen gemeldet hat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Refusals {
    /// Ob gemeldet wird.
    pub reporting: RefusalReporting,
    /// Je Paar ein Eintrag, in der Reihenfolge des ersten Auftretens.
    pub entries: Vec<Refusal>,
    /// Zählt jede Änderung; wer wartet, vergleicht damit.
    pub generation: u64,
}

impl Refusals {
    /// Nimmt eine Zeile auf und sagt, ob sich etwas geändert hat.
    ///
    /// Je Paar gilt der größte gemeldete Stand, die früheste erste und die
    /// späteste letzte Zeit. Ein neues Paar über [`MAX_REFUSAL_ENTRIES`] wird
    /// verworfen: Der Shim sammelt alles darüber in einem Posten, und mehr
    /// Paare als das kann nur eine Zeile behaupten, die nicht von ihm kommt.
    pub fn apply(&mut self, line: RefusalLine) -> bool {
        let changed = match line {
            RefusalLine::Reporting(reporting) => {
                let changed = self.reporting != reporting;
                self.reporting = reporting;
                changed
            }
            RefusalLine::Refused(refusal) => self.merge(refusal),
        };
        if changed {
            self.generation += 1;
        }
        changed
    }

    fn merge(&mut self, refusal: Refusal) -> bool {
        if let Some(known) = self.entries.iter_mut().find(|e| e.same_pair(&refusal)) {
            // The reason follows from the pair and the policy, which does not
            // change during a run. A line that gives the same pair another
            // reason did not come from the shim and changes nothing.
            if known.reason != refusal.reason {
                return false;
            }
            let before = known.clone();
            known.count = known.count.max(refusal.count);
            known.first = known.first.min(refusal.first);
            known.last = known.last.max(refusal.last);
            return *known != before;
        }
        if self.entries.len() >= MAX_REFUSAL_ENTRIES {
            return false;
        }
        self.entries.push(refusal);
        true
    }

    /// Alle Versuche zusammen.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.entries
            .iter()
            .fold(0u64, |sum, entry| sum.saturating_add(entry.count))
    }

    /// Der Befund, wenn die Sandbox verweigert, ohne es zu melden
    /// (`SANDBOX_019`); sonst `None`.
    #[must_use]
    pub fn unreported(&self) -> Option<Diagnostic> {
        match &self.reporting {
            RefusalReporting::Off(why) => Some(refusals_unreported(why)),
            RefusalReporting::Unknown | RefusalReporting::On => None,
        }
    }
}

/// `SANDBOX_019` mit dem Grund, den der Shim genannt hat.
#[must_use]
pub fn refusals_unreported(why: &str) -> Diagnostic {
    let hint = if why == "errno16" {
        " (EBUSY: a seccomp filter further up already has a listener, so Humanitl runs inside another sandbox that watches its own calls)"
    } else {
        ""
    };
    Diagnostic::builder(SANDBOX_019, Severity::Warning)
        .why(format!(
            "the sandbox refuses socket(2) outside the proxy as before, but cannot report the attempts: {why}{hint}"
        ))
        .build()
}

/// Liest eine Zeile `REFUSALS ...` oder `REFUSED ...`; alles andere ergibt
/// `None`.
#[must_use]
pub fn parse_refusal_line(line: &str) -> Option<RefusalLine> {
    let line = line.trim_end_matches(['\r', '\n']);
    let mut words = line.split(' ');
    match words.next()? {
        REFUSALS_PREFIX => {
            let reporting = match (words.next()?, words.next(), words.next()) {
                ("on", None, None) => RefusalReporting::On,
                ("off", Some(why), None) if is_token(why, 64) => {
                    RefusalReporting::Off(why.to_owned())
                }
                _ => return None,
            };
            Some(RefusalLine::Reporting(reporting))
        }
        REFUSED_PREFIX => {
            let fields: Vec<&str> = words.collect();
            let [syscall, family, socket_type, reason, count, first, last] = fields[..] else {
                return None;
            };
            if syscall != "socket" || !is_name(family, "AF_") || !is_name(socket_type, "SOCK_") {
                return None;
            }
            let reason = RefusalReason::parse(reason)?;
            let pooled = family == "*";
            if pooled != (socket_type == "*") || pooled != (reason == RefusalReason::Overflow) {
                return None;
            }
            let count: u64 = count.parse().ok()?;
            let first = epoch_ms(first)?;
            let last = epoch_ms(last)?;
            if count == 0 || first > last {
                return None;
            }
            Some(RefusalLine::Refused(Refusal {
                syscall: syscall.to_owned(),
                family: family.to_owned(),
                socket_type: socket_type.to_owned(),
                reason,
                count,
                first,
                last,
            }))
        }
        _ => None,
    }
}

/// `*`, oder `prefix` gefolgt von höchstens 24 Großbuchstaben, Ziffern und `_`.
fn is_name(text: &str, prefix: &str) -> bool {
    text == "*"
        || text.strip_prefix(prefix).is_some_and(|rest| {
            !rest.is_empty()
                && rest.len() <= 24
                && rest
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        })
}

/// Ein Wort aus druckbarem ASCII ohne Leerraum, höchstens `max` Zeichen lang.
fn is_token(text: &str, max: usize) -> bool {
    !text.is_empty() && text.len() <= max && text.chars().all(|c| c.is_ascii_graphic())
}

fn epoch_ms(text: &str) -> Option<SystemTime> {
    let ms: u64 = text.parse().ok()?;
    UNIX_EPOCH.checked_add(Duration::from_millis(ms))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn at(ms: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(ms)
    }

    fn refused(line: &str) -> Refusal {
        match parse_refusal_line(line) {
            Some(RefusalLine::Refused(refusal)) => refusal,
            other => panic!("{line:?} parsed as {other:?}"),
        }
    }

    #[test]
    fn the_three_shapes_of_the_shim_parse() {
        assert_eq!(
            parse_refusal_line("REFUSALS on\n"),
            Some(RefusalLine::Reporting(RefusalReporting::On))
        );
        assert_eq!(
            parse_refusal_line("REFUSALS off errno16"),
            Some(RefusalLine::Reporting(RefusalReporting::Off(
                "errno16".to_owned()
            )))
        );
        assert_eq!(
            refused("REFUSED socket AF_UNIX SOCK_STREAM family 3 1000 2000"),
            Refusal {
                syscall: "socket".to_owned(),
                family: "AF_UNIX".to_owned(),
                socket_type: "SOCK_STREAM".to_owned(),
                reason: RefusalReason::Family,
                count: 3,
                first: at(1000),
                last: at(2000),
            }
        );
        assert_eq!(
            refused("REFUSED socket * * overflow 50 1 1").reason,
            RefusalReason::Overflow
        );
        assert_eq!(
            refused("REFUSED socket AF_77 SOCK_99 family 1 5 5").family,
            "AF_77"
        );
    }

    #[test]
    fn a_line_that_is_not_the_shims_changes_nothing() {
        for bad in [
            "",
            "REFUSALS",
            "REFUSALS maybe",
            "REFUSALS on extra",
            "REFUSALS off",
            "REFUSALS off two words",
            "REFUSED socket AF_UNIX SOCK_STREAM family 3 1000",
            "REFUSED socket AF_UNIX SOCK_STREAM family 3 1000 2000 extra",
            "REFUSED connect AF_UNIX SOCK_STREAM family 3 1 2",
            "REFUSED socket af_unix SOCK_STREAM family 3 1 2",
            "REFUSED socket AF_UNIX SOCK_STREAM because 3 1 2",
            "REFUSED socket AF_UNIX SOCK_STREAM family -3 1 2",
            "REFUSED socket AF_UNIX SOCK_STREAM family 0 1 2",
            "REFUSED socket AF_UNIX SOCK_STREAM family 3 9 2",
            "REFUSED socket * SOCK_STREAM family 3 1 2",
            "REFUSED socket AF_UNIX SOCK_STREAM overflow 3 1 2",
            "REFUSED socket AF_THIS_NAME_IS_FAR_TOO_LONG_TO_BE_REAL SOCK_STREAM family 1 1 1",
            "CHECK families ok x",
        ] {
            assert_eq!(parse_refusal_line(bad), None, "{bad:?} must not parse");
        }
    }

    #[test]
    fn the_largest_count_wins_and_a_late_line_never_shrinks_it() {
        let mut refusals = Refusals::default();
        assert!(refusals.apply(RefusalLine::Refused(refused(
            "REFUSED socket AF_UNIX SOCK_STREAM family 5 100 500"
        ))));
        assert!(!refusals.apply(RefusalLine::Refused(refused(
            "REFUSED socket AF_UNIX SOCK_STREAM family 3 100 300"
        ))));
        assert!(refusals.apply(RefusalLine::Refused(refused(
            "REFUSED socket AF_UNIX SOCK_STREAM family 7 100 900"
        ))));
        assert!(refusals.apply(RefusalLine::Refused(refused(
            "REFUSED socket AF_INET SOCK_DGRAM type 2 400 400"
        ))));
        assert_eq!(refusals.entries.len(), 2);
        assert_eq!(refusals.entries[0].count, 7);
        assert_eq!(refusals.entries[0].last, at(900));
        assert_eq!(refusals.total(), 9);
        assert_eq!(refusals.generation, 3);
        // The same pair with another reason is not the shim's line.
        assert!(!refusals.apply(RefusalLine::Refused(refused(
            "REFUSED socket AF_UNIX SOCK_STREAM type 50 100 900"
        ))));
        assert_eq!(refusals.entries[0].count, 7);
        assert_eq!(refusals.entries[0].reason, RefusalReason::Family);
    }

    #[test]
    fn more_pairs_than_the_shim_keeps_apart_are_refused() {
        let mut refusals = Refusals::default();
        for n in 0..MAX_REFUSAL_ENTRIES + 5 {
            refusals.apply(RefusalLine::Refused(refused(&format!(
                "REFUSED socket AF_{} SOCK_STREAM family 1 1 1",
                n + 100
            ))));
        }
        assert_eq!(refusals.entries.len(), MAX_REFUSAL_ENTRIES);
    }

    #[test]
    fn off_is_a_warning_with_the_reason() {
        let mut refusals = Refusals::default();
        assert!(refusals.unreported().is_none());
        refusals.apply(RefusalLine::Reporting(RefusalReporting::Off(
            "errno16".to_owned(),
        )));
        let diagnostic = refusals.unreported().expect("SANDBOX_019");
        assert_eq!(diagnostic.code, SANDBOX_019);
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(diagnostic.why.contains("errno16"), "{}", diagnostic.why);
        assert!(diagnostic.why.contains("EBUSY"), "{}", diagnostic.why);
        refusals.apply(RefusalLine::Reporting(RefusalReporting::On));
        assert!(refusals.unreported().is_none());
    }
}
