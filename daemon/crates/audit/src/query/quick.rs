//! Eine Seite, ohne jede Zeile als JSON zu lesen (HUM-163).
//!
//! Über 100 000 Records dauert eine Seite, die jede Zeile als JSON liest, rund
//! 400 ms (Messung in `backlog/sprint-4.md`, HUM-163), doppelt so lange wie die
//! Grenze von 200 ms. Aus der ganzen Datei braucht eine Seite aber nur wenig: wie viele
//! Records der Filter trifft, welche davon unter dem Cursor die jüngsten sind
//! und wo das gemeldete Ende steht. Das steht alles im festen Teil einer Zeile.
//!
//! **Kein JSON-Leser, nur Bytes.** Eine Zeile des Schreibers ist kanonisch:
//! `data` steht vorn, dahinter die sieben übrigen Schlüssel in fester
//! Reihenfolge. Diese sieben prüft die Seite Byte für Byte und liest daraus
//! Nummer, Art, Sitzung und Zeitpunkt; `data` prüft sie mit
//! [`super::data::is_canonical_object`], ohne einen Wert zu bauen. Beides
//! zusammen nimmt nur Zeilen an, die `AuditRecord::from_line` auch als Record
//! liest. Ganz als JSON gelesen werden nur die Zeilen der Seite, die erste und
//! die letzte gezählte.
//!
//! **Heil heißt hier:** Jede Zeile hat diese kanonische Form, und ihre Nummer
//! ist die der Vorgängerin plus eins. Dann ist jede Zeile ein Record, und das
//! gemeldete Ende ist die erste Zeile mit seiner Nummer. Weicht eine Zeile ab,
//! gibt es von hier keine Antwort, und die Seite entsteht wie vor HUM-163 aus
//! jeder Zeile als JSON: Bei einer gebrochenen Kette liefert die Tabelle
//! dasselbe wie vorher. Auch eine Zeile, die `from_line` noch als Record
//! liest, die aber nicht kanonisch ist, führt zu diesem Rückfall; das kostet
//! Zeit, nie Genauigkeit.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use super::data::is_canonical_object;
use super::{Entry, Page, QueryFilter};
use crate::record::AuditRecord;

/// So beginnt jede kanonische Zeile: `data` ist der erste Schlüssel.
const LINE_START: &[u8] = b"{\"data\":{";
/// Das Ende von `data` und der erste Schlüssel dahinter.
const HASH_KEY: &[u8] = b"},\"hash\":\"";
/// So viel liest ein Aufruf von `read` auf einmal.
const READ_BUFFER: usize = 256 * 1024;

/// Die Seite aus dem festen Teil der Zeilen, oder `None`, wenn die Kette dafür
/// nicht heil ist (siehe Modulkommentar) und der Aufrufer jede Zeile lesen
/// muss.
///
/// `limit` ist schon begrenzt, `before` und `until` wie in [`super::query`].
pub(super) fn page(
    path: &Path,
    filter: &QueryFilter,
    limit: usize,
    before: Option<u64>,
    until: Option<u64>,
) -> io::Result<Option<Page>> {
    if until == Some(0) {
        return Ok(None);
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let reader = BufReader::with_capacity(READ_BUFFER, file);
    let scan = scan(reader, filter, limit, before, until)?;
    Ok(scan.and_then(|scan| scan.into_page(limit)))
}

/// Was ein Lauf über die festen Teile gesammelt hat.
#[derive(Default)]
struct Scan {
    /// Die jüngsten `limit + 1` Treffer unter dem Cursor, aufsteigend.
    window: VecDeque<Vec<u8>>,
    /// Wie viele Zeilen gezählt wurden.
    lines: u64,
    /// Wie viele davon der Filter trifft.
    total: u64,
    /// Die erste gezählte Zeile.
    first: Vec<u8>,
    /// Die letzte gezählte Zeile, vielleicht mit ihrem `\n`.
    last: Vec<u8>,
    /// Ihre Nummer.
    last_seq: u64,
}

/// Liest jede Zeile bis zum gemeldeten Ende und prüft nur ihren festen Teil.
fn scan(
    mut reader: impl io::BufRead,
    filter: &QueryFilter,
    limit: usize,
    before: Option<u64>,
    until: Option<u64>,
) -> io::Result<Option<Scan>> {
    let mut scan = Scan::default();
    let mut buffer = Vec::with_capacity(1024);
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            return Ok(Some(scan));
        }
        let line = buffer.strip_suffix(b"\n").unwrap_or(&buffer);
        let Some(fields) = Fields::read(line) else {
            return Ok(None);
        };
        let seq = fields.seq;
        if scan.lines == 0 {
            scan.first.extend_from_slice(line);
        } else if scan.last_seq.checked_add(1) != Some(seq) {
            return Ok(None);
        }
        scan.lines += 1;
        if filter.matches_fields(fields.kind, fields.session, fields.ts) {
            scan.total += 1;
            if before.is_none_or(|before| seq < before) {
                let reused = if scan.window.len() > limit {
                    scan.window.pop_front()
                } else {
                    None
                };
                let mut slot = reused.unwrap_or_default();
                slot.clear();
                slot.extend_from_slice(line);
                scan.window.push_back(slot);
            }
        }
        scan.last_seq = seq;
        // Die Zeile wird nicht kopiert, nur der Puffer getauscht.
        std::mem::swap(&mut scan.last, &mut buffer);
        if until.is_some_and(|until| seq >= until) {
            return Ok(Some(scan));
        }
    }
}

impl Scan {
    /// Liest die Zeilen der Seite und die beiden Ränder ganz; ist eine davon
    /// kein Record mit ihrer Nummer, gibt es keine Seite von hier.
    fn into_page(self, limit: usize) -> Option<Page> {
        let last = self.last.strip_suffix(b"\n").unwrap_or(&self.last);
        if self.lines > 0 {
            record(&self.first)?;
            record(last)?;
        }
        let mut entries = Vec::with_capacity(self.window.len());
        for line in self.window.into_iter().rev() {
            let record = record(&line)?;
            entries.push(Entry {
                record,
                line: String::from_utf8_lossy(&line).into_owned(),
            });
        }
        // Wie beim vollen Lesen: der eine zu viel sagt nur, dass noch eine
        // Seite kommt.
        let more = entries.len() > limit;
        if more {
            entries.pop();
        }
        let next_before = more
            .then(|| entries.last().map(|entry| entry.record.body.seq))
            .flatten();
        Some(Page {
            entries,
            total: self.total,
            next_before,
        })
    }
}

/// Der Record in `line`, wenn sie einer ist und dieselbe Nummer trägt wie ihr
/// fester Teil.
fn record(line: &[u8]) -> Option<AuditRecord> {
    let record = AuditRecord::from_line(line).ok()?;
    (Fields::read(line)?.seq == record.body.seq).then_some(record)
}

/// Was eine Seite aus dem festen Teil einer Zeile braucht.
#[derive(Debug, PartialEq, Eq)]
struct Fields<'a> {
    seq: u64,
    kind: &'a str,
    session: &'a str,
    ts: &'a str,
}

impl<'a> Fields<'a> {
    /// Liest eine kanonische Zeile, ohne `data` zu zerlegen.
    ///
    /// Geprüft wird die ganze Zeile: `data` als kanonisches Objekt, dahinter
    /// die sieben Schlüssel in kanonischer Reihenfolge, ihre Strings und die
    /// Nummer. Strenger als `from_line` zu sein kostet nichts, denn wer hier
    /// scheitert, wird voll gelesen; ein String mit Escape gehört deshalb
    /// nicht dazu, und sein Text ist sein Inhalt. `},"hash":"` kommt in einem String nicht vor, weil dort jedes
    /// `"` als `\"` steht, und sein letztes Vorkommen ist das der obersten
    /// Ebene, weil dahinter nur noch feste Schlüssel und Strings stehen.
    fn read(line: &'a [u8]) -> Option<Self> {
        let rest = line.strip_prefix(LINE_START)?;
        // Von hinten nur an jeder `}` vergleichen: Sie ist selten, und die
        // gesuchte steht wenige hundert Bytes vor dem Ende.
        let mut end = rest.len();
        let (at, tail) = loop {
            let at = rest.get(..end)?.iter().rposition(|&byte| byte == b'}')?;
            if let Some(tail) = rest.get(at..)?.strip_prefix(HASH_KEY) {
                break (at, tail);
            }
            end = at;
        };
        // `data` reicht vom `{` am Ende von `LINE_START` bis zu diesem `}`.
        let data = line.get(LINE_START.len() - 1..=LINE_START.len() + at)?;
        if !is_canonical_object(data) {
            return None;
        }
        let mut tail = Tail(tail);
        tail.string()?;
        tail.literal(b",\"kind\":\"")?;
        let kind = tail.string()?;
        tail.literal(b",\"mac\":\"")?;
        tail.string()?;
        tail.literal(b",\"prev\":\"")?;
        tail.string()?;
        tail.literal(b",\"seq\":")?;
        let seq = tail.number()?;
        tail.literal(b",\"session\":\"")?;
        let session = tail.string()?;
        tail.literal(b",\"ts\":\"")?;
        let ts = tail.string()?;
        tail.literal(b"}")?;
        tail.0.is_empty().then_some(Self {
            seq,
            kind,
            session,
            ts,
        })
    }
}

/// Der Rest einer Zeile hinter dem, was schon gelesen ist.
struct Tail<'a>(&'a [u8]);

impl<'a> Tail<'a> {
    /// Genau diese Bytes.
    fn literal(&mut self, expected: &[u8]) -> Option<()> {
        self.0 = self.0.strip_prefix(expected)?;
        Some(())
    }

    /// Der Inhalt eines Strings bis zu seinem schließenden `"`, das mit
    /// verbraucht wird: UTF-8 ohne Escape und ohne Steuerzeichen.
    fn string(&mut self) -> Option<&'a str> {
        let at = self.0.iter().position(|&byte| byte == b'"')?;
        let (text, rest) = self.0.split_at(at);
        if text.iter().any(|&byte| byte == b'\\' || byte < 0x20) {
            return None;
        }
        self.0 = rest.get(1..)?;
        std::str::from_utf8(text).ok()
    }

    /// Eine Ganzzahl ohne Vorzeichen und ohne führende Null, wie JSON sie
    /// schreibt, im Bereich von `u64`.
    fn number(&mut self) -> Option<u64> {
        let digits = self
            .0
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let (number, rest) = self.0.split_at(digits);
        if number.len() > 1 && number.first() == Some(&b'0') {
            return None;
        }
        let seq = std::str::from_utf8(number).ok()?.parse().ok()?;
        self.0 = rest;
        Some(seq)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use chrono::{TimeZone as _, Utc};

    use super::{Fields, page};
    use crate::query::{Page, QueryFilter, TimeRange, every_line, query};
    use crate::record::{GENESIS_PREV, RecordBody};

    const SESSION: &str = "0190d8a4-8f9c-7c3e-9a1b-3c4d5e6f7a8b";

    /// Die Zeilen einer heilen Kette mit den Nummern `1..=records`, im Wechsel
    /// zweier Arten und dreier Sitzungen, eine Sekunde je Record.
    fn chain(records: u64) -> Vec<String> {
        let mut prev = GENESIS_PREV.to_owned();
        (1..=records)
            .map(|seq| {
                let record = RecordBody {
                    seq,
                    ts: format!("2026-09-24T10:00:{:02}.000000Z", seq % 60),
                    session: if seq % 3 == 0 { SESSION } else { "-" }.to_owned(),
                    kind: if seq % 2 == 0 {
                        "pseudonym.created"
                    } else {
                        "flow.received"
                    }
                    .to_owned(),
                    data: serde_json::json!({ "n": seq, "inner": { "seq": 0 } }),
                    prev: prev.clone(),
                }
                .seal(&[5; 32])
                .unwrap();
                prev.clone_from(&record.hash);
                String::from_utf8(record.to_line().unwrap()).unwrap()
            })
            .collect()
    }

    fn write(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join("audit.jsonl");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn joined(lines: &[String]) -> String {
        let mut text = String::new();
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    /// Ohne Filter, nach Art, nach Sitzung, nach Zeitraum und alles zugleich.
    fn filters() -> Vec<QueryFilter> {
        let at = |second| Utc.with_ymd_and_hms(2026, 9, 24, 10, 0, second).unwrap();
        let range = TimeRange::new(Some(at(3)), Some(at(7)));
        vec![
            QueryFilter::default(),
            QueryFilter {
                kind_prefix: "flow.".to_owned(),
                ..QueryFilter::default()
            },
            QueryFilter {
                session: SESSION.to_owned(),
                ..QueryFilter::default()
            },
            QueryFilter {
                range: range.clone(),
                ..QueryFilter::default()
            },
            QueryFilter {
                kind_prefix: "pseudonym.".to_owned(),
                session: "-".to_owned(),
                range,
            },
        ]
    }

    /// Jeder Filter, jeder Cursor und jedes gemeldete Ende liefert über
    /// `query` dasselbe wie das Lesen jeder Zeile.
    fn assert_same_as_every_line(path: &Path, label: &str) {
        for filter in filters() {
            for until in [None, Some(1), Some(4), Some(6), Some(9), Some(40)] {
                for before in [None, Some(1), Some(2), Some(5), Some(8), Some(40)] {
                    for limit in [1, 3, 200] {
                        assert_eq!(
                            query(path, &filter, limit, before, until).unwrap(),
                            every_line(path, &filter, limit, before, until).unwrap(),
                            "{label}: {filter:?}, limit {limit}, before {before:?}, until {until:?}"
                        );
                    }
                }
            }
        }
    }

    /// Bei einer heilen Kette antwortet der feste Teil selbst, und zwar genau
    /// so wie das Lesen jeder Zeile.
    #[test]
    fn a_whole_chain_pages_from_the_fixed_part_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), &joined(&chain(9)));
        for filter in filters() {
            for until in [None, Some(1), Some(5), Some(9), Some(20)] {
                for before in [None, Some(1), Some(2), Some(6), Some(10)] {
                    for limit in [1, 2, 4, 200] {
                        let quick = page(&path, &filter, limit, before, until).unwrap();
                        let full = every_line(&path, &filter, limit, before, until).unwrap();
                        assert_eq!(
                            quick,
                            Some(full),
                            "{filter:?}, limit {limit}, before {before:?}, until {until:?}"
                        );
                    }
                }
            }
        }
        let empty = write(dir.path(), "");
        let open = QueryFilter::default();
        assert_eq!(
            page(&empty, &open, 5, None, None).unwrap(),
            Some(Page::default())
        );
    }

    /// Eine gebrochene Kette gibt dieselben Seiten wie vor HUM-163.
    #[test]
    fn a_broken_chain_pages_exactly_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let lines = chain(9);
        // Hinter `data` kanonisch, `data` selbst kein JSON: kein Record.
        let broken_data = |line: &str| line.replacen(r#"{"data":{"#, r#"{"data":{"n":tru,"#, 1);
        let mut cases: Vec<(&str, Vec<String>)> = Vec::new();

        let mut foreign = lines.clone();
        foreign.insert(4, "not a record".to_owned());
        cases.push(("a foreign line", foreign));

        let mut gap = lines.clone();
        gap.remove(4);
        cases.push(("a missing record", gap));

        let mut twin = lines.clone();
        twin.insert(5, broken_data(&lines[4]));
        cases.push(("a broken twin of record 5", twin));

        let mut first = lines.clone();
        first[0] = broken_data(&lines[0]);
        cases.push(("a broken first line", first));

        let mut last = lines.clone();
        last[8] = broken_data(&lines[8]);
        cases.push(("a broken last line", last));

        let mut rewritten = lines.clone();
        rewritten.extend(chain(5));
        cases.push(("a chain written twice", rewritten));

        let mut spaced = lines.clone();
        spaced[3] = lines[3].replacen(r#""kind":"#, r#" "kind":"#, 1);
        cases.push(("a record not in canonical form", spaced));

        let mut escaped = lines.clone();
        escaped[5] = lines[5].replacen(r#""kind":"pseudonym"#, r#""kind":"pseudonym\u002e"#, 1);
        cases.push(("an escape in a field the filter reads", escaped));

        let mut blank = lines.clone();
        blank.insert(2, String::new());
        cases.push(("an empty line", blank));

        for (label, lines) in &cases {
            let path = write(dir.path(), &joined(lines));
            assert_same_as_every_line(&path, label);
        }
        let torn = format!("{}{}", joined(&lines), &lines[0][..20]);
        assert_same_as_every_line(&write(dir.path(), &torn), "a torn last line");
        let unterminated = joined(&lines);
        let unterminated = unterminated.trim_end_matches('\n');
        assert_same_as_every_line(&write(dir.path(), unterminated), "no final newline");
    }

    /// Eine Zeile mitten in der Kette, deren Teil hinter `data` stimmt, deren
    /// `data` aber kein kanonisches Objekt ist, zählt wie vor HUM-163: Die
    /// schnelle Seite gibt auf, und die volle zählt.
    #[test]
    fn a_line_with_a_broken_data_part_counts_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let lines = chain(9);
        let fourth = lines[3].as_bytes();
        let start = br#"{"data":{"#;
        let with = |prefix: &[u8]| [prefix, &fourth[start.len()..]].concat();
        let mut invalid_utf8 = fourth.to_vec();
        let n = invalid_utf8
            .windows(4)
            .position(|w| w == b"\"n\":")
            .unwrap();
        invalid_utf8.splice(n..n, *b"\"\xff\":1,");
        let cases: [(&str, Vec<u8>); 5] = [
            ("no JSON in data", with(br#"{"data":{"n":tru,"#)),
            ("a key beside data", with(br#"{"data":{},"x":{"#)),
            ("data twice", with(br#"{"data":{},"data":{"#)),
            ("a number JSON does not know", with(br#"{"data":{"x":1.,"#)),
            ("invalid UTF-8 in data", invalid_utf8),
        ];
        let open = QueryFilter::default();
        for (label, fourth) in cases {
            let mut text = Vec::new();
            for (index, line) in lines.iter().enumerate() {
                text.extend_from_slice(if index == 3 { &fourth } else { line.as_bytes() });
                text.push(b'\n');
            }
            let path = dir.path().join("audit.jsonl");
            std::fs::write(&path, &text).unwrap();
            assert_eq!(page(&path, &open, 3, None, None).unwrap(), None, "{label}");
            assert_same_as_every_line(&path, label);
        }
    }

    /// Eine Lücke in den Nummern und ein Zwilling mit derselben Nummer lassen
    /// die schnelle Seite aufgeben, auch wenn jede Zeile für sich stimmt.
    #[test]
    fn a_gap_or_a_twin_in_the_numbers_gives_up() {
        let dir = tempfile::tempdir().unwrap();
        let lines = chain(9);
        let mut gap = lines.clone();
        gap.remove(4);
        let mut twin = lines.clone();
        twin.insert(5, lines[4].clone());
        let open = QueryFilter::default();
        for (label, lines) in [("a missing record", gap), ("a twin", twin)] {
            let path = write(dir.path(), &joined(&lines));
            assert_eq!(page(&path, &open, 3, None, None).unwrap(), None, "{label}");
            assert_same_as_every_line(&path, label);
        }
    }

    /// Ein kanonischer Teil hinter `data` mit dieser Nummer und Sitzung.
    fn line(seq: &str, session: &str) -> Vec<u8> {
        format!(
            r#"{{"data":{{"x":{{"seq":9}}}},"hash":"ab","kind":"flow.received","mac":"cd","prev":"ef","seq":{seq},"session":"{session}","ts":"2026-09-24T10:00:00.000000Z"}}"#
        )
        .into_bytes()
    }

    fn seq(line: &[u8]) -> Option<u64> {
        Fields::read(line).map(|fields| fields.seq)
    }

    /// Nummer, Art, Sitzung und Zeitpunkt stehen hinter `data`; was nicht die
    /// kanonische Form ohne Escape hat, hat keinen festen Teil.
    #[test]
    fn the_fixed_part_is_read_behind_data() {
        assert_eq!(
            Fields::read(&line("42", "-")),
            Some(Fields {
                seq: 42,
                kind: "flow.received",
                session: "-",
                ts: "2026-09-24T10:00:00.000000Z",
            })
        );
        assert_eq!(seq(&line("0", "-")), Some(0));
        assert_eq!(seq(&line("042", "-")), None);
        assert_eq!(seq(&line("99999999999999999999", "-")), None);
        assert_eq!(seq(&line("", "-")), None);
        assert_eq!(seq(&line("4", r"a\u002db")), None);
        assert_eq!(seq(&line("4", "a\u{1}b")), None);
        assert_eq!(seq(&line("4", "a}b")), Some(4));
        assert_eq!(seq(b"garbage"), None);
        let mut trailing = line("42", "-");
        trailing.push(b' ');
        assert_eq!(seq(&trailing), None);
        let missing =
            br#"{"data":{},"hash":"ab","kind":"k","mac":"cd","seq":42,"session":"-","ts":"t"}"#;
        assert_eq!(seq(missing), None);
    }
}
