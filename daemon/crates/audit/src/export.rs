//! Die Formate des Exports der Kette als JSONL oder CSV (HUM-050, HUM-070,
//! HUM-156).
//!
//! **JSONL** ist die Kette: jede Zeile im Zeitraum Byte für Byte so, wie sie im
//! Log steht, samt `\n`. Der Export bleibt damit gegen die Datei nachrechenbar.
//!
//! **CSV** ist die Übersicht aus HUM-050 mit zwölf Spalten ([`CSV_COLUMNS`]).
//! Was ein Record in `data` nicht trägt, bleibt leer; `data` als Ganzes steht
//! nicht darin, dafür gibt es JSONL. Die letzte Spalte ist der Hash, über den
//! jede Zeile der Übersicht in der Kette wiederzufinden ist.
//!
//! Den Export selbst, der in eine Datei schreibt und nie etwas überschreibt,
//! führt `humanitl-audit-log` aus (HUM-155), denn diese Crate öffnet keine
//! Datei.

use serde_json::Value;

use crate::record::AuditRecord;

/// Die Spalten des CSV-Exports, in dieser Reihenfolge (HUM-050).
pub const CSV_COLUMNS: [&str; 12] = [
    "seq", "ts", "session", "kind", "flow", "host", "method", "decision", "rule", "status", "size",
    "hash",
];

/// Welches der beiden Dokumente ein Export schreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Jede Zeile der Kette, wörtlich.
    Jsonl,
    /// Eine Kopfzeile und eine Zeile je Record, zwölf Spalten.
    Csv,
}

impl ExportFormat {
    /// Liest `jsonl` oder `csv`; alles andere ist `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "jsonl" => Some(Self::Jsonl),
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }

    /// Der Name, den Vertrag und Kommandozeile tragen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jsonl => "jsonl",
            Self::Csv => "csv",
        }
    }
}

/// Eine Zeile des CSV: die zwölf Spalten aus [`CSV_COLUMNS`].
///
/// Die acht Spalten zwischen `kind` und `hash` kommen aus `data`, und nur
/// dann, wenn der Record das Feld trägt. Ein Text steht als er selbst da, eine
/// Zahl in Ziffern, `null` als leeres Feld, alles andere als JSON.
#[must_use]
pub fn csv_row(record: &AuditRecord) -> String {
    let data = |name: &str| match record.body.data.get(name) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    };
    [
        record.body.seq.to_string(),
        record.body.ts.clone(),
        record.body.session.clone(),
        record.body.kind.clone(),
        data("flow"),
        data("host"),
        data("method"),
        data("decision"),
        data("rule"),
        data("status"),
        data("size"),
        record.hash.clone(),
    ]
    .iter()
    .map(|field| csv_field(field))
    .collect::<Vec<_>>()
    .join(",")
}

/// Ein Feld nach RFC 4180: in Anführungszeichen, sobald es welche braucht.
#[must_use]
pub fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::json;

    use super::{CSV_COLUMNS, ExportFormat, csv_field, csv_row};
    use crate::record::{AuditRecord, GENESIS_PREV, RecordBody};

    fn record(kind: &str, data: serde_json::Value) -> AuditRecord {
        RecordBody {
            seq: 7,
            ts: "2026-09-02T10:42:01.000000Z".to_owned(),
            session: "-".to_owned(),
            kind: kind.to_owned(),
            data,
            prev: GENESIS_PREV.to_owned(),
        }
        .seal(&[7_u8; 32])
        .expect("the record seals")
    }

    /// Jede Spalte bekommt ihr Feld aus `data`, und ein Feld, das der Record
    /// nicht trägt, bleibt leer.
    #[test]
    fn a_csv_row_takes_each_column_from_its_own_field() {
        let received = record(
            "flow.received",
            json!({ "flow": "f-1", "host": "api.example", "method": "GET", "size": 12,
                    "path_hash": "x" }),
        );
        assert_eq!(
            csv_row(&received),
            format!(
                "7,2026-09-02T10:42:01.000000Z,-,flow.received,f-1,api.example,GET,,,,12,{}",
                received.hash
            )
        );
        let decided = record(
            "flow.decided",
            json!({ "flow": "f-1", "decision": "block", "rule": null }),
        );
        assert_eq!(
            csv_row(&decided),
            format!(
                "7,2026-09-02T10:42:01.000000Z,-,flow.decided,f-1,,,block,,,,{}",
                decided.hash
            )
        );
        let responded = record("flow.responded", json!({ "flow": "f-1", "status": 403 }));
        assert!(csv_row(&responded).contains(",403,"));
        assert_eq!(CSV_COLUMNS.len(), 12);
        assert_eq!(CSV_COLUMNS[11], "hash");
    }

    #[test]
    fn a_field_with_a_quote_is_doubled_and_wrapped() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn only_the_two_formats_parse() {
        assert_eq!(ExportFormat::parse("jsonl"), Some(ExportFormat::Jsonl));
        assert_eq!(ExportFormat::parse("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::parse("CSV"), None);
        assert_eq!(ExportFormat::parse(""), None);
        assert_eq!(ExportFormat::Csv.as_str(), "csv");
    }
}
