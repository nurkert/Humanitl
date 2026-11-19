//! Zeitpunkte als Text, Byte für Byte in den Formen, die vor dem Wechsel von
//! `chrono` auf `jiff` (HUM-526) geschrieben wurden.
//!
//! Die Formen stehen hier ausdrücklich und nicht über `jiff`s eigene Ausgabe:
//! `jiff` schreibt Bruchteile von Sekunden mit so wenigen Stellen wie nötig
//! (`.5`), die bisherigen Dateien und Ausgaben tragen aber feste Gruppen von
//! drei, sechs oder neun Stellen (`.500`). Ein Zeitstempel, der sich mit einer
//! Abhängigkeit ändert, änderte den Hash des Audit-Logs und den Text in
//! `rules.yaml`.
//!
//! Alle Formen schreiben UTC; alle bis auf [`rfc3339_auto_offset`] enden auf
//! `Z`. Das Jahr steht mit vier Stellen; ein Jahr vor 0 trägt sein Vorzeichen
//! (`-0001`), wie es `chrono` tat.

use core::fmt::Write as _;
use std::time::SystemTime;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::Offset;

/// Wie viele Stellen nach dem Komma stehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fraction {
    /// Keine; der Rest unter der Sekunde fällt weg.
    None,
    /// Immer sechs; der Rest unter der Mikrosekunde fällt weg.
    Micros,
    /// Keine bei glatter Sekunde, sonst drei, sechs oder neun, je nachdem,
    /// wie viele es braucht (`chrono`s `SecondsFormat::AutoSi`).
    Auto,
}

/// Wie UTC am Ende steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    /// `Z`.
    Z,
    /// `+00:00`.
    Offset,
}

/// `2026-09-03T10:00:00Z`: UTC, ganze Sekunden, immer `Z`.
///
/// Die Form, in der `rules.yaml` den Ablauf einer Regel speichert und die CLI
/// einen Zeitpunkt nennt (vorher `to_rfc3339_opts(SecondsFormat::Secs, true)`).
#[must_use]
pub fn rfc3339_secs(at: Timestamp) -> String {
    render(at, Fraction::None, Zone::Z)
}

/// `2026-09-03T10:00:00.123456Z`: UTC, immer sechs Stellen, immer `Z`.
///
/// Die Form jedes Zeitstempels im Audit-Log.
#[must_use]
pub fn rfc3339_micros(at: Timestamp) -> String {
    render(at, Fraction::Micros, Zone::Z)
}

/// `20260903T100000.123456Z`: wie [`rfc3339_micros`], aber ohne `-` und `:`
/// zwischen den Feldern (vorher `format("%Y%m%dT%H%M%S%.6fZ")`).
///
/// Die Form im Namen einer beiseitegelegten, zerrissenen Audit-Datei.
#[must_use]
pub fn compact_micros(at: Timestamp) -> String {
    let full = rfc3339_micros(at);
    // Das Vorzeichen eines Jahres vor 0 bleibt, nur die Trenner fallen weg.
    let (sign, rest) = full.split_at(usize::from(full.starts_with('-')));
    let mut out = sign.to_owned();
    out.extend(rest.chars().filter(|c| *c != '-' && *c != ':'));
    out
}

/// `2026-09-03T10:00:00Z` oder `2026-09-03T10:00:00.500Z`: UTC, Bruchteil in
/// Gruppen zu drei Stellen, nur wenn es einen gibt, immer `Z`.
///
/// Die Serde-Form eines Zeitpunkts (vorher die von `chrono::DateTime<Utc>`).
#[must_use]
pub fn rfc3339_auto(at: Timestamp) -> String {
    render(at, Fraction::Auto, Zone::Z)
}

/// `2026-09-03T10:00:00+00:00`: wie [`rfc3339_auto`], aber mit `+00:00`
/// statt `Z` (vorher `chrono`s `to_rfc3339()`).
///
/// Die Form, in der die Statusseite des Proxys den Ablauf einer Regel nennt.
#[must_use]
pub fn rfc3339_auto_offset(at: Timestamp) -> String {
    render(at, Fraction::Auto, Zone::Offset)
}

/// Liest einen Zeitpunkt nach RFC 3339 mit Versatz oder `Z`.
///
/// Jede Form, die eine der Funktionen oben schreibt, liest diese wieder;
/// ebenso jeder Versatz (`+02:00`), der dann nach UTC umgerechnet wird.
///
/// Mehr als neun Stellen nach dem Komma schneidet die Funktion ab, wie es
/// `chrono` tat; `jiff` lehnte sie ab, und eine von Hand geschriebene Regel in
/// `rules.yaml` wäre damit nicht mehr lesbar.
///
/// # Errors
///
/// Der Text ist kein Zeitpunkt mit Versatz.
pub fn parse_rfc3339(text: &str) -> Result<Timestamp, jiff::Error> {
    match excess_fraction(text) {
        Some((keep, skip)) => format!("{}{}", &text[..keep], &text[skip..]).parse(),
        None => text.parse(),
    }
}

/// Wo die Stellen eines Bruchteils jenseits der neunten liegen: das Ende des
/// Teils, der bleibt, und der Anfang dessen, was danach folgt.
fn excess_fraction(text: &str) -> Option<(usize, usize)> {
    let start = text.find(['.', ','])? + 1;
    let digits = text[start..].bytes().take_while(u8::is_ascii_digit).count();
    (digits > 9).then_some((start + 9, start + digits))
}

/// Ein [`SystemTime`] als Zeitpunkt; außerhalb des Bereichs, den `jiff`
/// darstellt (die Jahre -9999 bis 9999), der nächste Rand.
#[must_use]
pub fn from_system_time(at: SystemTime) -> Timestamp {
    Timestamp::try_from(at).unwrap_or_else(|_| {
        if at < SystemTime::UNIX_EPOCH {
            Timestamp::MIN
        } else {
            Timestamp::MAX
        }
    })
}

/// Die Millisekunden seit 1970, abgerundet (auch vor 1970 zur kleineren Zahl).
#[must_use]
pub fn millis_floor(at: Timestamp) -> i64 {
    // Der ganze Bereich von `Timestamp` passt in Millisekunden in ein `i64`.
    i64::try_from(at.as_nanosecond().div_euclid(1_000_000)).unwrap_or(i64::MAX)
}

/// Sekunden seit 1970 und die Nanosekunden darin, wie sie
/// `google.protobuf.Timestamp` trägt: die Sekunden abgerundet, die
/// Nanosekunden immer von 0 bis 999 999 999, auch vor 1970.
///
/// `jiff` gibt vor 1970 negative Nanosekunden; `chrono` gab, wie hier, die
/// kleinere Sekunde und einen positiven Rest.
#[must_use]
pub fn unix_parts(at: Timestamp) -> (i64, u32) {
    let nanos = at.as_nanosecond();
    (
        i64::try_from(nanos.div_euclid(1_000_000_000)).unwrap_or(i64::MAX),
        u32::try_from(nanos.rem_euclid(1_000_000_000)).unwrap_or(0),
    )
}

/// Der Zeitpunkt zu Sekunden seit 1970 und Nanosekunden darin, die
/// Umkehrung von [`unix_parts`]. `None` bei mehr als 999 999 999 Nanosekunden
/// oder außerhalb des Bereichs von `jiff` (die Jahre -9999 bis 9999).
#[must_use]
pub fn from_unix_parts(seconds: i64, nanos: u32) -> Option<Timestamp> {
    let nanos = i32::try_from(nanos).ok().filter(|n| *n < 1_000_000_000)?;
    Timestamp::from_second(seconds)
        .ok()?
        .checked_add(jiff::SignedDuration::new(0, nanos))
        .ok()
}

fn render(at: Timestamp, fraction: Fraction, zone: Zone) -> String {
    let civil: DateTime = Offset::UTC.to_datetime(at);
    let mut out = String::with_capacity(32);
    let year = civil.year();
    // `write!` auf einen `String` scheitert nie.
    let _ = if (0..=9999).contains(&year) {
        write!(out, "{year:04}")
    } else {
        write!(out, "{year:+05}")
    };
    let _ = write!(
        out,
        "-{:02}-{:02}T{:02}:{:02}:{:02}",
        civil.month(),
        civil.day(),
        civil.hour(),
        civil.minute(),
        civil.second(),
    );
    let nano = civil.subsec_nanosecond();
    let _ = match fraction {
        Fraction::None => Ok(()),
        Fraction::Micros => write!(out, ".{:06}", nano / 1000),
        Fraction::Auto if nano == 0 => Ok(()),
        Fraction::Auto if nano % 1_000_000 == 0 => write!(out, ".{:03}", nano / 1_000_000),
        Fraction::Auto if nano % 1000 == 0 => write!(out, ".{:06}", nano / 1000),
        Fraction::Auto => write!(out, ".{nano:09}"),
    };
    out.push_str(match zone {
        Zone::Z => "Z",
        Zone::Offset => "+00:00",
    });
    out
}

/// Serde-Form eines Zeitpunkts nach [`rfc3339_auto`], für
/// `#[serde(with = "humanitl_core::time::serde_rfc3339")]`.
///
/// Gelesen wird jeder Zeitpunkt nach [`parse_rfc3339`].
pub mod serde_rfc3339 {
    use jiff::Timestamp;
    use serde::de::Error as _;
    use serde::{Deserialize as _, Deserializer, Serializer};

    /// Schreibt `at` nach [`super::rfc3339_auto`].
    ///
    /// # Errors
    ///
    /// Der Fehler des Serializers.
    pub fn serialize<S: Serializer>(at: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::rfc3339_auto(*at))
    }

    /// Liest einen Zeitpunkt nach [`super::parse_rfc3339`].
    ///
    /// # Errors
    ///
    /// Kein String oder kein Zeitpunkt mit Versatz.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Timestamp, D::Error> {
        let text = String::deserialize(deserializer)?;
        super::parse_rfc3339(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use jiff::Timestamp;

    use super::{
        compact_micros, from_system_time, from_unix_parts, millis_floor, parse_rfc3339,
        rfc3339_auto, rfc3339_auto_offset, rfc3339_micros, rfc3339_secs, unix_parts,
    };

    fn at(seconds: i64, nanos: i32) -> Timestamp {
        Timestamp::new(seconds, nanos).unwrap()
    }

    /// Die erwarteten Texte stammen aus `chrono` 0.4.45 vor dem Wechsel
    /// (`to_rfc3339_opts(SecondsFormat::Secs, true)`,
    /// `format("%Y-%m-%dT%H:%M:%S%.6fZ")`, `to_rfc3339()` und der Serde-Form von
    /// `DateTime<Utc>`), nicht aus diesem Code.
    #[test]
    fn the_forms_match_what_chrono_wrote() {
        // 2026-09-03T10:00:00Z
        let whole = at(1_788_429_600, 0);
        assert_eq!(rfc3339_secs(whole), "2026-09-03T10:00:00Z");
        assert_eq!(rfc3339_micros(whole), "2026-09-03T10:00:00.000000Z");
        assert_eq!(rfc3339_auto(whole), "2026-09-03T10:00:00Z");

        let half = at(1_788_429_600, 500_000_000);
        assert_eq!(rfc3339_secs(half), "2026-09-03T10:00:00Z");
        assert_eq!(rfc3339_micros(half), "2026-09-03T10:00:00.500000Z");
        assert_eq!(rfc3339_auto(half), "2026-09-03T10:00:00.500Z");

        let micros = at(1_788_429_600, 123_456_000);
        assert_eq!(rfc3339_auto(micros), "2026-09-03T10:00:00.123456Z");
        assert_eq!(rfc3339_auto_offset(whole), "2026-09-03T10:00:00+00:00");
        assert_eq!(
            compact_micros(at(1_788_429_600, 123_456_789)),
            "20260903T100000.123456Z"
        );
        assert_eq!(rfc3339_auto_offset(half), "2026-09-03T10:00:00.500+00:00");

        let nanos = at(1_788_429_600, 123_456_789);
        assert_eq!(rfc3339_secs(nanos), "2026-09-03T10:00:00Z");
        assert_eq!(rfc3339_micros(nanos), "2026-09-03T10:00:00.123456Z");
        assert_eq!(rfc3339_auto(nanos), "2026-09-03T10:00:00.123456789Z");

        let early = at(-1, 999_999_000);
        assert_eq!(rfc3339_micros(early), "1969-12-31T23:59:59.999999Z");
        assert_eq!(rfc3339_secs(Timestamp::UNIX_EPOCH), "1970-01-01T00:00:00Z");

        // 0001-01-01T00:00:00Z und ein Jahr vor 0.
        assert_eq!(rfc3339_secs(at(-62_135_596_800, 0)), "0001-01-01T00:00:00Z");
        assert_eq!(
            rfc3339_secs(at(-62_198_755_200, 0)),
            "-0001-01-01T00:00:00Z"
        );
    }

    /// Was geschrieben wurde, liest sich als derselbe Zeitpunkt zurück, auch
    /// mit einem Versatz statt `Z`.
    #[test]
    fn the_forms_read_back() {
        for text in [
            "2026-09-03T10:00:00Z",
            "2026-09-03T10:00:00.500Z",
            "2026-09-03T10:00:00.123456Z",
            "2026-09-03T10:00:00.123456789Z",
        ] {
            assert_eq!(rfc3339_auto(parse_rfc3339(text).unwrap()), text);
        }
        assert_eq!(
            rfc3339_secs(parse_rfc3339("2026-09-03T12:00:00+02:00").unwrap()),
            "2026-09-03T10:00:00Z"
        );
        assert!(parse_rfc3339("2026-09-03T10:00:00").is_err());
        assert!(parse_rfc3339("tomorrow").is_err());
    }

    /// Mehr als neun Stellen Bruchteil schneidet das Lesen ab, wie `chrono`;
    /// eine so von Hand geschriebene Regel bleibt lesbar.
    #[test]
    fn a_long_fraction_is_cut_after_nine_digits() {
        for (text, read) in [
            (
                "2026-09-03T10:00:00.1234567890Z",
                "2026-09-03T10:00:00.123456789Z",
            ),
            (
                "2026-09-03T12:00:00.99999999999+02:00",
                "2026-09-03T10:00:00.999999999Z",
            ),
            ("2026-09-03T10:00:00,0000000001Z", "2026-09-03T10:00:00Z"),
        ] {
            assert_eq!(rfc3339_auto(parse_rfc3339(text).unwrap()), read, "{text}");
        }
        assert!(parse_rfc3339("2026-09-03T10:00:00.1234567890").is_err());
    }

    /// Die Ränder des Bereichs von `jiff`; das Jahr vor 0 mit Vorzeichen und
    /// vier Stellen, wie `chrono` es schrieb.
    #[test]
    fn the_range_edges_keep_the_form() {
        assert_eq!(
            rfc3339_auto(Timestamp::MAX),
            "9999-12-30T22:00:00.999999999Z"
        );
        assert_eq!(
            rfc3339_micros(Timestamp::MAX),
            "9999-12-30T22:00:00.999999Z"
        );
        assert_eq!(rfc3339_auto(Timestamp::MIN), "-9999-01-02T01:59:59Z");
        assert_eq!(
            rfc3339_micros(Timestamp::MIN),
            "-9999-01-02T01:59:59.000000Z"
        );
        assert_eq!(compact_micros(Timestamp::MIN), "-99990102T015959.000000Z");
    }

    #[test]
    fn the_offset_form_groups_the_fraction_in_threes() {
        assert_eq!(
            rfc3339_auto_offset(at(1_788_429_600, 123_456_000)),
            "2026-09-03T10:00:00.123456+00:00"
        );
        assert_eq!(
            rfc3339_auto_offset(at(1_788_429_600, 123_456_789)),
            "2026-09-03T10:00:00.123456789+00:00"
        );
    }

    #[test]
    fn a_system_time_converts_and_clamps() {
        use std::time::{Duration, SystemTime};

        let later = SystemTime::UNIX_EPOCH + Duration::new(1_788_429_600, 5);
        assert_eq!(
            rfc3339_auto(from_system_time(later)),
            "2026-09-03T10:00:00.000000005Z"
        );
        let earlier = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(
            rfc3339_secs(from_system_time(earlier)),
            "1969-12-31T23:59:59Z"
        );
        // Jenseits des Jahres 9999, soweit die Plattform es darstellt.
        if let Some(far) = SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(300_000_000_000))
        {
            assert_eq!(from_system_time(far), Timestamp::MAX);
        }
        if let Some(far) = SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(400_000_000_000))
        {
            assert_eq!(from_system_time(far), Timestamp::MIN);
        }
    }

    #[test]
    fn unix_parts_keep_the_nanos_positive() {
        assert_eq!(unix_parts(at(1_788_429_600, 5)), (1_788_429_600, 5));
        assert_eq!(unix_parts(at(-1, 999_999_000)), (-1, 999_999_000));
        assert_eq!(unix_parts(at(0, -1)), (-1, 999_999_999));
        for (seconds, nanos) in [(1_788_429_600, 5), (-1, 999_999_000), (0, 0)] {
            assert_eq!(
                from_unix_parts(seconds, nanos).map(unix_parts),
                Some((seconds, nanos))
            );
        }
        assert_eq!(from_unix_parts(0, 1_000_000_000), None);
        assert_eq!(from_unix_parts(i64::MAX, 0), None);
    }

    #[test]
    fn millis_round_down() {
        assert_eq!(millis_floor(at(1, 999_999_999)), 1999);
        assert_eq!(millis_floor(at(-1, 999_999_999)), -1);
        assert_eq!(millis_floor(at(-1, 0)), -1000);
    }
}
