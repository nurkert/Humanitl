//! Die Kette lesen, ohne sie zu prüfen: Ende und Seiten (HUM-156).
//!
//! Die Prüfung steht in [`crate::verify`] und beweist etwas. Was hier steht,
//! beweist nichts, es zeigt nur: das Ende der Datei für den Kopf der
//! Oberfläche und eine Seite der Records für ihre Tabelle. Beides liest die
//! Datei so, wie sie liegt, und rechnet keinen Hash nach — der Kopf einer Kette,
//! die jemand neu geschrieben hat, ist der Kopf der neu geschriebenen Kette.
//!
//! **Seiten, neueste zuerst.** Eine Seite hält höchstens `limit` Records, die
//! jüngsten zuerst. Der Cursor ist die Nummer des untersten Records der letzten
//! Seite; die nächste Seite beginnt unter ihr. Eine Nummer und kein Index,
//! damit ein Record, der zwischen zwei Seiten dazukommt, keine Seite
//! verschiebt: Neue Records haben höhere Nummern und stehen über dem Cursor.
//!
//! **Zeilen, die kein Record sind**, zeigt keine Seite und zählt kein Kopf mit
//! einer Nummer. Wo die Kette bricht, sagt die Prüfung, nicht die Tabelle.
//!
//! **Bis zum gemeldeten Ende.** `until` ist die Nummer, die der Schreiber
//! zuletzt als geschrieben gemeldet hat; gelesen wird bis zu diesem Record und
//! nicht weiter. Was dahinter steht, entsteht gerade (HUM-156). `None` liest
//! bis zum Ende der Datei.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, DurationRound as _, TimeDelta, Utc};
use humanitl_core::diagnostics::codes::AUDIT_006;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::key::shell_quote;
use crate::record::{AuditRecord, format_ts};

/// So viele Records hat eine Seite, wenn der Aufrufer keine Zahl nennt.
pub const DEFAULT_PAGE: usize = 200;

/// Mehr Records hat keine Seite, egal was der Aufrufer verlangt.
///
/// Eine Seite geht als eine Antwort über die Leitung; die Kette wächst ohne
/// Decke, und eine Seite ohne Decke wäre die ganze Kette.
pub const MAX_PAGE: usize = 1000;

/// Ein Zeitraum, beide Grenzen einschließlich; `None` heißt ohne Grenze.
///
/// Die Grenzen stehen schon in der Form des Logs ([`crate::record::TS_FORMAT`]):
/// UTC mit fester Breite, und dort ist die Ordnung der Zeichen die der Zeit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimeRange {
    from: Option<String>,
    to: Option<String>,
}

impl TimeRange {
    /// Ohne Grenzen.
    pub const ALL: Self = Self {
        from: None,
        to: None,
    };

    /// Ein Zeitraum aus zwei Zeitpunkten, beide einschließlich.
    ///
    /// Das Log schreibt Mikrosekunden. Eine untere Grenze mit einem Rest
    /// darunter rundet deshalb auf, eine obere ab: Sonst stünde ein Record,
    /// der vor der unteren Grenze liegt, im Zeitraum, weil seine Mikrosekunde
    /// dieselbe ist.
    #[must_use]
    pub fn new(from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Self {
        let micro = TimeDelta::microseconds(1);
        Self {
            from: from.map(|at| format_ts(at.duration_round_up(micro).unwrap_or(at))),
            to: to.map(|at| format_ts(at.duration_trunc(micro).unwrap_or(at))),
        }
    }

    /// Ob eine Grenze gesetzt ist.
    #[must_use]
    pub const fn is_bounded(&self) -> bool {
        self.from.is_some() || self.to.is_some()
    }

    /// Ob ein Zeitpunkt des Logs in den Zeitraum fällt.
    #[must_use]
    pub fn contains(&self, ts: &str) -> bool {
        self.from.as_deref().is_none_or(|from| ts >= from)
            && self.to.as_deref().is_none_or(|to| ts <= to)
    }
}

/// Welche Records eine Seite zeigt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryFilter {
    /// Präfix der Art, zum Beispiel `flow.`; leer heißt jede Art.
    pub kind_prefix: String,
    /// Die Sitzung als UUID-Text oder `-`; leer heißt jede Sitzung.
    pub session: String,
    /// Der Zeitraum.
    pub range: TimeRange,
}

impl QueryFilter {
    /// Ob ein Record dem Filter entspricht.
    #[must_use]
    pub fn matches(&self, record: &AuditRecord) -> bool {
        record.body.kind.starts_with(&self.kind_prefix)
            && (self.session.is_empty() || record.body.session == self.session)
            && self.range.contains(&record.body.ts)
    }
}

/// Ein Record einer Seite samt seiner Zeile, Byte für Byte wie in der Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Der gelesene Record.
    pub record: AuditRecord,
    /// Die Zeile ohne `\n`.
    pub line: String,
}

/// Eine Seite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    /// Die Records, die jüngste Nummer zuerst.
    pub entries: Vec<Entry>,
    /// Wie viele Records der Filter trifft, über alle Seiten.
    pub total: u64,
    /// Unter welcher Nummer die nächste Seite beginnt; `None`, wenn keine
    /// mehr kommt.
    pub next_before: Option<u64>,
}

/// Das Ende der Datei, ohne Prüfung.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tail {
    /// Wie viele Zeilen der Datei Records sind.
    pub records: u64,
    /// Der letzte davon; `None` für eine leere Kette.
    pub last: Option<AuditRecord>,
}

/// Liest eine Seite aus `path`.
///
/// `limit` null heißt [`DEFAULT_PAGE`], mehr als [`MAX_PAGE`] heißt
/// [`MAX_PAGE`]. `before` ist der Cursor: nur Records mit kleinerer Nummer.
/// `until` ist das gemeldete Ende (siehe Modulkommentar). Eine fehlende Datei
/// ist eine leere Kette.
///
/// # Errors
///
/// [`AUDIT_006`], wenn sich die Datei nicht lesen lässt.
pub fn query(
    path: &Path,
    filter: &QueryFilter,
    limit: usize,
    before: Option<u64>,
    until: Option<u64>,
) -> Result<Page, Diagnostic> {
    let limit = match limit {
        0 => DEFAULT_PAGE,
        n => n.min(MAX_PAGE),
    };
    // Die Datei steht aufsteigend. Behalten werden die jüngsten `limit + 1`
    // Treffer unter dem Cursor; der eine zu viel sagt, dass noch eine Seite
    // kommt, und wird nicht geliefert.
    let mut window: VecDeque<Entry> = VecDeque::with_capacity(limit + 1);
    let mut total = 0_u64;
    for_each_record(path, "list", until, |record, line| {
        if !filter.matches(&record) {
            return;
        }
        total += 1;
        if before.is_some_and(|before| record.body.seq >= before) {
            return;
        }
        if window.len() > limit {
            window.pop_front();
        }
        window.push_back(Entry {
            record,
            line: String::from_utf8_lossy(line).into_owned(),
        });
    })?;
    let more = window.len() > limit;
    if more {
        window.pop_front();
    }
    let entries: Vec<Entry> = window.into_iter().rev().collect();
    let next_before = more
        .then(|| entries.last().map(|entry| entry.record.body.seq))
        .flatten();
    Ok(Page {
        entries,
        total,
        next_before,
    })
}

/// Zählt die Records in `path` bis zum gemeldeten Ende `until` und liefert den
/// letzten.
///
/// # Errors
///
/// [`AUDIT_006`], wenn sich die Datei nicht lesen lässt.
pub fn tail(path: &Path, until: Option<u64>) -> Result<Tail, Diagnostic> {
    let mut tail = Tail::default();
    for_each_record(path, "read the head of", until, |record, _| {
        tail.records += 1;
        tail.last = Some(record);
    })?;
    Ok(tail)
}

/// Ruft `visit` für jede Zeile von `path`, die ein Record ist, in der
/// Reihenfolge der Datei, bis zum Record mit der Nummer `until`. `doing` steht
/// im Befund, falls das Lesen scheitert.
fn for_each_record(
    path: &Path,
    doing: &str,
    until: Option<u64>,
    mut visit: impl FnMut(AuditRecord, &[u8]),
) -> Result<(), Diagnostic> {
    let result = (|| -> io::Result<()> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::with_capacity(1024);
        if until == Some(0) {
            return Ok(());
        }
        loop {
            buffer.clear();
            if reader.read_until(b'\n', &mut buffer)? == 0 {
                return Ok(());
            }
            let line = buffer.strip_suffix(b"\n").unwrap_or(&buffer);
            if let Ok(record) = AuditRecord::from_line(line) {
                let last = until.is_some_and(|until| record.body.seq >= until);
                visit(record, line);
                if last {
                    return Ok(());
                }
            }
        }
    })();
    result.map_err(|err| {
        Diagnostic::builder(AUDIT_006, Severity::Error)
            .why(format!("cannot {doing} {}: {err}", path.display()))
            .fix(FixAction::CopyCommand(format!(
                "ls -ln {}",
                shell_quote(&path.display().to_string())
            )))
            .build()
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use chrono::{TimeZone as _, Utc};

    use super::TimeRange;

    /// Beide Grenzen schließen ein, und ein Rest unter der Mikrosekunde zieht
    /// die Grenze nach innen, nie nach außen.
    #[test]
    fn a_range_is_inclusive_and_rounds_inwards() {
        let from = Utc.with_ymd_and_hms(2026, 9, 2, 10, 0, 0).unwrap()
            + chrono::TimeDelta::nanoseconds(500);
        let to = Utc.with_ymd_and_hms(2026, 9, 2, 11, 0, 0).unwrap()
            + chrono::TimeDelta::nanoseconds(500);
        let range = TimeRange::new(Some(from), Some(to));
        assert!(range.is_bounded());
        assert!(!range.contains("2026-09-02T10:00:00.000000Z"));
        assert!(range.contains("2026-09-02T10:00:00.000001Z"));
        assert!(range.contains("2026-09-02T11:00:00.000000Z"));
        assert!(!range.contains("2026-09-02T11:00:00.000001Z"));
        assert!(!TimeRange::ALL.is_bounded());
        assert!(TimeRange::ALL.contains("1970-01-01T00:00:00.000000Z"));
    }
}
