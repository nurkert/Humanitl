//! Wie lange eine Antwort auf die Suche der Sandbox-Backends wartet
//! (HUM-502).
//!
//! `Doctor` und `SandboxBackends` fragen jedes Backend wie die Suche vor
//! einem Start, und eine Suche kann länger dauern als die Frist eines
//! Aufrufers. Wie lange gewartet wird, entscheidet der Aufrufer selbst, mit
//! der Frist, die er seinem Aufruf ohnehin gibt: dem Kopf `grpc-timeout`
//! (gRPC über HTTP/2). Die App ruft jede unäre RPC mit 5 s Frist und sendet
//! ihn deshalb immer; tonic bricht einen Aufruf nach dieser Frist auch auf
//! der Seite des Daemons ab. Die Kommandozeile setzt keine Frist im Kopf
//! (`humanitl doctor` wartet mit eigener Frist, `humanitl sandbox backends`
//! ohne): Sie bekommt das ganze Ergebnis.
//!
//! Ein eigenes Feld in der Anfrage wäre eine zweite Angabe derselben Sache,
//! die von der Frist des Aufrufs abweichen könnte; der Kopf ist die Frist,
//! nach der tonic die Antwort ohnehin verwirft.

use std::time::{Duration, Instant};

use tonic::metadata::MetadataMap;

/// Was von der Frist des Aufrufers für die Antwort selbst bleibt: Bericht
/// bauen, kodieren, senden. Der Rest gehört der Suche.
const ANSWER_MARGIN: Duration = Duration::from_millis(1500);

/// Bis wann die Suche der Backends für diese Anfrage laufen darf, gemessen
/// ab `now`; keine Frist, wenn der Aufrufer keine nennt oder der Kopf nicht
/// lesbar ist.
///
/// Die Frist der Suche ist die des Aufrufers abzüglich [`ANSWER_MARGIN`],
/// mindestens aber ihre Hälfte: Aus 5 s werden 3,5 s, aus 1 s eine halbe.
pub(crate) fn search_deadline(metadata: &MetadataMap, now: Instant) -> Option<Instant> {
    let timeout = caller_timeout(metadata)?;
    let budget = timeout.saturating_sub(ANSWER_MARGIN).max(timeout / 2);
    now.checked_add(budget)
}

/// Die Frist des Aufrufers aus `grpc-timeout`: höchstens acht Ziffern und
/// eine Einheit (`H`, `M`, `S`, `m`, `u`, `n`), wie die Spezifikation von
/// gRPC über HTTP/2 sie schreibt.
fn caller_timeout(metadata: &MetadataMap) -> Option<Duration> {
    let value = metadata.get("grpc-timeout")?.to_str().ok()?;
    let unit = value.chars().last()?;
    let digits = &value[..value.len() - unit.len_utf8()];
    if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let amount: u64 = digits.parse().ok()?;
    Some(match unit {
        'H' => Duration::from_secs(amount.checked_mul(3600)?),
        'M' => Duration::from_secs(amount.checked_mul(60)?),
        'S' => Duration::from_secs(amount),
        'm' => Duration::from_millis(amount),
        'u' => Duration::from_micros(amount),
        'n' => Duration::from_nanos(amount),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn with_timeout(value: &str) -> MetadataMap {
        let mut metadata = MetadataMap::new();
        metadata.insert("grpc-timeout", value.parse().unwrap());
        metadata
    }

    /// Die App sendet ihre 5 s als `5000m` (grpc-dart): Die Suche bekommt
    /// 3,5 s davon.
    #[test]
    fn the_five_seconds_of_the_app_leave_three_and_a_half_for_the_search() {
        let now = Instant::now();
        let deadline = search_deadline(&with_timeout("5000m"), now).expect("a deadline");
        assert_eq!(deadline - now, Duration::from_millis(3500));
        let deadline = search_deadline(&with_timeout("5S"), now).expect("a deadline");
        assert_eq!(deadline - now, Duration::from_millis(3500));
    }

    /// Eine kurze Frist behält die Hälfte, statt auf null zu fallen.
    #[test]
    fn a_short_deadline_keeps_half_of_it() {
        let now = Instant::now();
        let deadline = search_deadline(&with_timeout("1000000u"), now).expect("a deadline");
        assert_eq!(deadline - now, Duration::from_millis(500));
    }

    /// Ohne Kopf, und mit einem, der nicht der Form folgt, gibt es keine
    /// Frist: Die Antwort wartet auf das ganze Ergebnis.
    #[test]
    fn without_a_readable_timeout_there_is_no_deadline() {
        let now = Instant::now();
        assert_eq!(search_deadline(&MetadataMap::new(), now), None);
        for bad in ["", "m", "5", "5x", "123456789m", "-5m", "5.0S"] {
            assert_eq!(search_deadline(&with_timeout(bad), now), None, "{bad:?}");
        }
        assert_eq!(
            caller_timeout(&with_timeout("2M")),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            caller_timeout(&with_timeout("1H")),
            Some(Duration::from_secs(3600))
        );
        assert_eq!(
            caller_timeout(&with_timeout("7n")),
            Some(Duration::from_nanos(7))
        );
    }
}
