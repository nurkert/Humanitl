//! Die Antwort, die ein geblockter Client bekommt.
//!
//! Der Wortlaut ist verbindlich (`backlog/CONVENTIONS.md` 3.5) und Teil der
//! Absprache mit dem Agenten in der Sandbox: `Blocked by Humanitl.` heißt
//! endgültig, nicht wiederholen (`docs/ARCHITECTURE.md` 8). Die Notiz des
//! Nutzers geht in denselben Body und in den Header [`NOTE_HEADER`]; deshalb
//! wird sie hier gesäubert und nicht dort, wo jemand es vergessen könnte.
//!
//! Zwei Formen der Notiz (HUM-072): der Body trägt den gesäuberten Text samt
//! Nicht-ASCII, der Header nur die sichtbaren ASCII-Zeichen daraus
//! ([`BlockResponse::header_note`]), weil ein Feldwert nach RFC 9110 nichts
//! anderes tragen darf.

use crate::flow::{BlockReason, UpstreamError};
use crate::host::HostName;
use crate::ids::FlowId;

/// So viele Zeichen einer Notiz gehen höchstens hinaus.
pub const NOTE_MAX_CHARS: usize = 500;

/// Header, in dem die Notiz zusätzlich steht.
pub const NOTE_HEADER: &str = "x-humanitl-note";

/// `Content-Type` der Block-Antwort.
pub const CONTENT_TYPE: &str = "text/plain; charset=utf-8";

/// Erste Zeile jeder Block-Antwort.
pub const BANNER: &str = "Blocked by Humanitl.";

/// Was der Proxy dem Client schickt, wenn die Anfrage nicht hinausgeht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockResponse {
    /// Der HTTP-Status.
    pub status: u16,
    /// Der Body, `text/plain`.
    pub body: String,
    /// Die gesäuberte Notiz, so wie sie in der `note:`-Zeile des Bodys steht,
    /// falls eine übrig blieb. Für den Header siehe
    /// [`BlockResponse::header_note`].
    pub note: Option<String>,
}

impl BlockResponse {
    /// Der Wert für [`NOTE_HEADER`]: die Notiz auf sichtbare ASCII-Zeichen
    /// beschränkt, siehe [`note_header_value`]. `None`, wenn davon nichts
    /// übrig bleibt; der Body trägt die Notiz dann trotzdem.
    #[must_use]
    pub fn header_note(&self) -> Option<String> {
        self.note
            .as_deref()
            .map(note_header_value)
            .filter(|value| !value.is_empty())
    }
}

/// Säubert eine Notiz für Body und Header.
///
/// Nach HUM-072: `CR` und `LF` (und die Unicode-Zeilentrenner) werden zu
/// Leerzeichen, andere Steuerzeichen fallen weg, Tab bleibt. Mehrfache
/// Leerzeichen fallen zusammen, Ränder werden abgeschnitten, und es bleiben
/// höchstens [`NOTE_MAX_CHARS`] Zeichen übrig. Damit kann eine Notiz weder
/// eine zweite Header-Zeile öffnen noch die Struktur des Bodys nachahmen.
///
/// Zusätzlich fallen unsichtbare Zeichen weg: Zero-Width-Zeichen und die
/// Bidi-Steuerzeichen. Sie sind keine Steuerzeichen im Sinne von
/// `char::is_control`, könnten aber im Terminal des Agenten einen anderen Text
/// vortäuschen als den, den der Nutzer geschrieben hat.
///
/// Nicht-ASCII bleibt erhalten; für den Header kürzt [`note_header_value`]
/// weiter.
#[must_use]
pub fn sanitize_note(note: &str) -> String {
    let mut out = String::new();
    let mut last_was_space = false;
    for raw in note.chars() {
        let ch = match raw {
            '\r' | '\n' | '\u{2028}' | '\u{2029}' => ' ',
            '\t' => '\t',
            ch if ch.is_control() || is_invisible(ch) => continue,
            ch if ch.is_whitespace() => ' ',
            ch => ch,
        };
        if ch == ' ' {
            if last_was_space {
                continue;
            }
            last_was_space = true;
        } else {
            last_was_space = false;
        }
        out.push(ch);
    }
    let capped: String = out.trim().chars().take(NOTE_MAX_CHARS).collect();
    capped.trim_end().to_owned()
}

/// Beschränkt eine gesäuberte Notiz auf das, was ein Header-Wert tragen darf.
///
/// RFC 9110 §5.5 erlaubt in einem Feldwert sichtbare ASCII-Zeichen sowie
/// Leerzeichen und Tab dazwischen; alles andere fällt weg, Nicht-ASCII bleibt
/// nur im Body (HUM-072). Ränder werden abgeschnitten, weil ein Feldwert
/// weder mit Leerraum beginnt noch endet.
///
/// Erwartet die Ausgabe von [`sanitize_note`]; auf rohem Text lässt die
/// Funktion Steuerzeichen unter `0x20` zwar ebenfalls weg, kürzt aber nicht.
#[must_use]
pub fn note_header_value(note: &str) -> String {
    let ascii: String = note
        .chars()
        .filter(|ch| matches!(ch, ' ' | '\t' | '!'..='~'))
        .collect();
    ascii.trim().to_owned()
}

/// Unsichtbare Zeichen, die eine Notiz anders aussehen lassen, als sie ist.
fn is_invisible(ch: char) -> bool {
    matches!(ch,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
            | '\u{e0000}'..='\u{e007f}'
    )
}

/// Baut die Antwort auf einen Block.
///
/// `note` ist der rohe Text des Nutzers; er wird mit [`sanitize_note`]
/// gesäubert und weggelassen, wenn davon nichts übrig bleibt.
#[must_use]
pub fn block_response(
    reason: BlockReason,
    flow: FlowId,
    host: &HostName,
    note: Option<&str>,
) -> BlockResponse {
    let note = note.map(sanitize_note).filter(|text| !text.is_empty());
    BlockResponse {
        status: reason.http_status(),
        body: body(reason.as_str(), flow, host, note.as_deref()),
        note,
    }
}

/// Baut die Antwort auf einen gescheiterten Upstream: immer `502`, die
/// `reason:`-Zeile trägt [`UpstreamError::reason`] mit Präfix `upstream_`.
#[must_use]
pub fn failed_response(error: UpstreamError, flow: FlowId, host: &HostName) -> BlockResponse {
    BlockResponse {
        status: error.http_status(),
        body: body(error.reason(), flow, host, None),
        note: None,
    }
}

/// Der Body in seiner verbindlichen Form.
fn body(reason: &str, flow: FlowId, host: &HostName, note: Option<&str>) -> String {
    let mut out = format!("{BANNER}\nreason: {reason}\nflow: {flow}\nhost: {host}\n");
    if let Some(note) = note {
        out.push_str("note: ");
        out.push_str(note);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{
        NOTE_MAX_CHARS, block_response, failed_response, note_header_value, sanitize_note,
    };
    use crate::flow::{BlockReason, UpstreamError};
    use crate::host::HostName;
    use crate::ids::{FlowId, RuleId};

    fn host() -> HostName {
        HostName::Dns("api.github.com".to_owned())
    }

    #[test]
    fn body_has_the_agreed_shape() {
        let flow = FlowId::new();
        let response = block_response(BlockReason::User, flow, &host(), None);
        assert_eq!(response.status, 403);
        assert_eq!(
            response.body,
            format!("Blocked by Humanitl.\nreason: user\nflow: {flow}\nhost: api.github.com\n")
        );
        assert_eq!(response.note, None);
        assert_eq!(response.header_note(), None);
    }

    #[test]
    fn a_failed_upstream_reads_as_upstream_in_the_reason_line() {
        let flow = FlowId::new();
        let response = failed_response(UpstreamError::Dns, flow, &host());
        assert_eq!(response.status, 502);
        assert_eq!(
            response.body,
            format!(
                "Blocked by Humanitl.\nreason: upstream_dns\nflow: {flow}\nhost: api.github.com\n"
            )
        );
        let timed_out = failed_response(UpstreamError::Timeout, flow, &host());
        assert!(timed_out.body.contains("reason: upstream_timeout\n"));
        let policy = block_response(BlockReason::Timeout, flow, &host(), None);
        assert!(policy.body.contains("reason: timeout\n"));
    }

    #[test]
    fn note_is_a_single_extra_line() {
        let flow = FlowId::new();
        let response = block_response(
            BlockReason::Rule(RuleId::new()),
            flow,
            &host(),
            Some("Nutze PyPI statt GitHub"),
        );
        assert_eq!(response.body.lines().count(), 5);
        assert!(response.body.ends_with("note: Nutze PyPI statt GitHub\n"));
        assert_eq!(response.note.as_deref(), Some("Nutze PyPI statt GitHub"));
        assert_eq!(
            response.header_note().as_deref(),
            Some("Nutze PyPI statt GitHub")
        );
    }

    #[test]
    fn a_note_cannot_inject_a_header_or_a_line() {
        let flow = FlowId::new();
        let evil = "ok\r\nX-Evil: 1\nSet-Cookie: a=b\u{2028}reason: allow\u{0000}";
        let response = block_response(BlockReason::User, flow, &host(), Some(evil));
        let note = response.note.clone().unwrap_or_default();
        assert!(!note.contains('\r'));
        assert!(!note.contains('\n'));
        assert!(!note.chars().any(char::is_control));
        assert_eq!(note, "ok X-Evil: 1 Set-Cookie: a=b reason: allow");
        assert_eq!(response.header_note(), Some(note));
        assert_eq!(response.body.lines().count(), 5);
        assert!(!response.body.contains('\r'));
        assert_eq!(
            response
                .body
                .lines()
                .filter(|line| line.starts_with("reason:"))
                .count(),
            1,
            "a note must not open a second reason line"
        );
    }

    #[test]
    fn tab_stays_and_other_control_characters_vanish() {
        assert_eq!(sanitize_note("a\tb\u{0001}c\u{007f}d\u{0085}e"), "a\tbcde");
        assert_eq!(sanitize_note("a\u{000b}b"), "ab", "vertical tab is not tab");
    }

    #[test]
    fn note_is_capped_at_five_hundred_characters() {
        let long = "ä".repeat(2000);
        let sanitized = sanitize_note(&long);
        assert_eq!(sanitized.chars().count(), NOTE_MAX_CHARS);

        let spaced = format!("{} {}", "a".repeat(499), "b".repeat(100));
        let sanitized = sanitize_note(&spaced);
        assert_eq!(
            sanitized.chars().count(),
            499,
            "no trailing blank after the cut"
        );
        assert!(sanitized.ends_with('a'), "{sanitized}");
        assert!(sanitize_note(&"x".repeat(600)).chars().count() <= NOTE_MAX_CHARS);
    }

    #[test]
    fn a_note_cannot_hide_behind_invisible_characters() {
        let sneaky = "\u{202e}exe.gnp\u{200b} bitte\u{feff}";
        assert_eq!(sanitize_note(sneaky), "exe.gnp bitte");
        // Bidi-Marken und Default-Ignorable-Zeichen außerhalb der klassischen Bereiche
        // (ARABIC LETTER MARK, SOFT HYPHEN, COMBINING GRAPHEME JOINER, MONGOLIAN VOWEL
        // SEPARATOR, Sprach-Tags aus Ebene 14) fallen ebenfalls weg.
        let sneaky = "a\u{061c}b\u{00ad}c\u{034f}d\u{180e}e\u{e0041}f";
        assert_eq!(sanitize_note(sneaky), "abcdef");
    }

    #[test]
    fn a_note_of_only_control_characters_disappears() {
        let response = block_response(BlockReason::User, FlowId::new(), &host(), Some("\r\n\t "));
        assert_eq!(response.note, None);
        assert_eq!(response.header_note(), None);
        assert_eq!(response.body.lines().count(), 4);
    }

    #[test]
    fn non_ascii_stays_in_the_body_but_leaves_the_header() {
        let response = block_response(
            BlockReason::User,
            FlowId::new(),
            &host(),
            Some("Grüße — nutze PyPI"),
        );
        assert_eq!(response.note.as_deref(), Some("Grüße — nutze PyPI"));
        assert!(response.body.ends_with("note: Grüße — nutze PyPI\n"));

        let header = response.header_note().unwrap_or_default();
        assert!(
            header
                .chars()
                .all(|ch| matches!(ch, ' ' | '\t' | '!'..='~')),
            "{header:?} must be visible ascii"
        );
        assert_eq!(header, "Gre  nutze PyPI");

        let only_umlauts = block_response(BlockReason::User, FlowId::new(), &host(), Some("äöü"));
        assert_eq!(only_umlauts.note.as_deref(), Some("äöü"));
        assert_eq!(only_umlauts.header_note(), None);

        assert_eq!(note_header_value("  a\tb  "), "a\tb");
        assert_eq!(
            note_header_value("\u{0001}x").chars().count(),
            1,
            "raw control bytes fall out too"
        );
    }

    #[test]
    fn every_reason_maps_to_its_status() {
        let cases = [
            (BlockReason::User, 403),
            (BlockReason::Rule(RuleId::nil()), 403),
            (BlockReason::AuthorityMismatch, 403),
            (BlockReason::PrivateAddress, 403),
            (BlockReason::BodyCap, 413),
            (BlockReason::Timeout, 504),
            (BlockReason::HoldMemory, 503),
            (BlockReason::HoldMaxFlows, 503),
            (BlockReason::NoRoute, 502),
            (BlockReason::ClientTimeout, 408),
        ];
        for (reason, status) in cases {
            let response = block_response(reason, FlowId::new(), &host(), None);
            assert_eq!(response.status, status, "{reason}");
        }
        assert_eq!(
            failed_response(UpstreamError::Dns, FlowId::new(), &host()).status,
            502
        );
    }
}
