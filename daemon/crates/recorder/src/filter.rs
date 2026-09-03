//! Die Filtersprache der Flow-Liste.
//!
//! Ein Ausdruck ist eine Folge von Termen, die alle gelten müssen:
//!
//! ```text
//! query   := term (WS term)*
//! term    := key ':' value | word
//! key     := host | apex | state | method | decision | reason | rule | status
//!          | since | until | findings | session | path | edited | passthrough | upgrade
//! value   := (cmp)? atom
//! cmp     := '>=' | '<=' | '>' | '<'
//! atom    := '"' [^"]* '"' | [^\s]+
//! ```
//!
//! Dieselbe Sprache benutzen die Filterleiste der Oberfläche, `ListFlows.filter`
//! und `humanitl flows list`; sie wird an genau dieser Stelle übersetzt und
//! nirgends nachgebaut.
//!
//! # Warum ein eigener Parser und keine Regex
//!
//! Der Ausdruck kommt vom Nutzer und wird zu `SQL`. Er wird deshalb nie in den
//! Text der Abfrage eingesetzt, sondern immer in Platzhalter; `%` und `_` in
//! einem `LIKE`-Wert werden mit `\` maskiert (`ESCAPE '\'`), sonst wäre
//! `path:%` eine Suche nach allem statt nach einem Prozentzeichen.

use crate::error::{RecorderError, filter_failed};
use crate::hostkey::suffix_range;

/// Alle Schlüssel, in der Reihenfolge, in der sie im Befund aufgezählt werden.
pub const KEYS: &[&str] = &[
    "host",
    "apex",
    "state",
    "method",
    "decision",
    "reason",
    "rule",
    "status",
    "since",
    "until",
    "findings",
    "session",
    "path",
    "edited",
    "passthrough",
    "upgrade",
];

/// Ein Wert, der als Platzhalter in die Abfrage geht.
#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    /// Ein Text.
    Text(String),
    /// Eine Zahl.
    Int(i64),
}

impl rusqlite::ToSql for Param {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            Self::Text(text) => text.to_sql(),
            Self::Int(value) => value.to_sql(),
        }
    }
}

/// Ein übersetzter Filter: `SQL`-Bedingung plus ihre Werte.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    /// Die Bedingung, `1` für „alles".
    pub sql: String,
    /// Die Werte in der Reihenfolge der Platzhalter.
    pub params: Vec<Param>,
}

impl Filter {
    /// Der Filter, der jede Zeile nimmt.
    #[must_use]
    pub fn all() -> Self {
        Self {
            sql: "1".to_owned(),
            params: Vec::new(),
        }
    }
}

/// Ein Vergleich vor dem Wert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmp {
    Eq,
    Ge,
    Le,
    Gt,
    Lt,
}

impl Cmp {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ge => ">=",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Lt => "<",
        }
    }
}

/// Übersetzt einen Ausdruck in eine `SQL`-Bedingung.
///
/// `now_ms` ist der Bezugspunkt für relative Zeitangaben (`since:10m`); er wird
/// übergeben und nicht gelesen, damit ein Test dieselbe Rechnung ohne Uhr
/// nachvollziehen kann.
///
/// # Errors
///
/// [`RecorderError::Filter`] mit `RECORDER_002`, wenn ein Schlüssel unbekannt
/// ist, ein Wert fehlt, eine Zahl keine ist, eine Zeitangabe sich nicht lesen
/// lässt oder ein Vergleich an einem Schlüssel steht, der keinen kennt.
pub fn parse(input: &str, now_ms: i64) -> Result<Filter, RecorderError> {
    let mut sql = String::new();
    let mut params = Vec::new();

    for term in tokenize(input) {
        let (fragment, values) = translate(&term, now_ms)?;
        if !sql.is_empty() {
            sql.push_str(" AND ");
        }
        sql.push_str(&fragment);
        params.extend(values);
    }

    if sql.is_empty() {
        return Ok(Filter::all());
    }
    Ok(Filter { sql, params })
}

/// Zerlegt den Ausdruck in Terme; Anführungszeichen halten Leerzeichen zusammen.
fn tokenize(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
                current.push(ch);
            }
            ch if ch.is_whitespace() && !quoted => {
                if started {
                    terms.push(core::mem::take(&mut current));
                    started = false;
                }
            }
            ch => {
                started = true;
                current.push(ch);
            }
        }
    }
    if started {
        terms.push(current);
    }
    terms.retain(|term| !term.is_empty());
    terms
}

/// Übersetzt einen einzelnen Term.
fn translate(term: &str, now_ms: i64) -> Result<(String, Vec<Param>), RecorderError> {
    let Some((key, rest)) = split_key(term) else {
        return Ok(word_term(term));
    };
    let key = key.to_ascii_lowercase();
    if !KEYS.contains(&key.as_str()) {
        return Err(unknown_key(term, &key));
    }

    let (cmp, atom) = split_cmp(rest);
    let value = unquote(atom);
    if value.is_empty() {
        return Err(filter_failed(format!(
            "the filter term {term:?} has no value; write {key}:<value>"
        )));
    }

    if let Some(fragment) = equality(&key, &value) {
        reject_cmp(cmp, &key, term)?;
        return Ok(fragment);
    }
    compared(&key, &value, cmp, term, now_ms)
}

/// Die Schlüssel, die einen Wert einfach vergleichen.
///
/// `None` heißt: dieser Schlüssel liest eine Zahl, eine Zeit oder einen
/// Wahrheitswert, siehe [`compared`].
fn equality(key: &str, value: &str) -> Option<(String, Vec<Param>)> {
    let lower = value.to_ascii_lowercase();
    let fragment = match key {
        "host" => {
            // Suffix-Vergleich als Bereich über `host_rev`, siehe `hostkey` und
            // `migrations/V3__host_suffix.sql`: `host LIKE '%.' || ?` kann kein
            // Index beantworten, ein Bereich schon.
            let (low, high) = suffix_range(value);
            (
                "(host_rev >= ? AND host_rev < ?)".to_owned(),
                vec![Param::Text(low), Param::Text(high)],
            )
        }
        "apex" => ("apex = ?".to_owned(), vec![Param::Text(lower)]),
        "state" => ("state = ?".to_owned(), vec![Param::Text(lower)]),
        "method" => (
            "method = ?".to_owned(),
            vec![Param::Text(value.to_ascii_uppercase())],
        ),
        "decision" => ("decision = ?".to_owned(), vec![Param::Text(lower)]),
        "reason" => ("block_reason = ?".to_owned(), vec![Param::Text(lower)]),
        "upgrade" => ("upgrade = ?".to_owned(), vec![Param::Text(lower)]),
        "rule" => (
            "rule_id = ?".to_owned(),
            vec![Param::Text(value.to_owned())],
        ),
        "session" => (
            "session_id = ?".to_owned(),
            vec![Param::Text(value.to_owned())],
        ),
        "path" => (
            "path LIKE ? ESCAPE '\\'".to_owned(),
            vec![Param::Text(format!("%{}%", like_escape(value)))],
        ),
        _other => return None,
    };
    Some(fragment)
}

/// Die Schlüssel, die eine Zahl, eine Zeit oder einen Wahrheitswert lesen.
fn compared(
    key: &str,
    value: &str,
    cmp: Option<Cmp>,
    term: &str,
    now_ms: i64,
) -> Result<(String, Vec<Param>), RecorderError> {
    match key {
        "status" => Ok((
            format!("status {} ?", cmp.unwrap_or(Cmp::Eq).as_sql()),
            vec![Param::Int(number(value, term)?)],
        )),
        "findings" => Ok((
            format!("findings_count {} ?", cmp.unwrap_or(Cmp::Eq).as_sql()),
            vec![Param::Int(number(value, term)?)],
        )),
        "since" => {
            reject_cmp(cmp, key, term)?;
            Ok((
                "ts >= ?".to_owned(),
                vec![Param::Int(timestamp(value, now_ms, term)?)],
            ))
        }
        "until" => {
            reject_cmp(cmp, key, term)?;
            Ok((
                "ts <= ?".to_owned(),
                vec![Param::Int(timestamp(value, now_ms, term)?)],
            ))
        }
        "edited" => plain_bool(cmp, key, term, value, "edited"),
        "passthrough" => plain_bool(cmp, key, term, value, "passthrough"),
        other => Err(unknown_key(term, other)),
    }
}

/// Der Befund zu einem Schlüssel, den es nicht gibt.
fn unknown_key(term: &str, key: &str) -> RecorderError {
    filter_failed(format!(
        "the filter term {term:?} uses the unknown key {key:?}; valid keys are {}",
        KEYS.join(", ")
    ))
}

/// Ein Term ohne Schlüssel: sucht in Host und Pfad.
fn word_term(word: &str) -> (String, Vec<Param>) {
    let pattern = format!("%{}%", like_escape(&unquote(word)));
    (
        "(host LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\')".to_owned(),
        vec![Param::Text(pattern.clone()), Param::Text(pattern)],
    )
}

/// Ein Wahrheitswert auf einer `INTEGER`-Spalte.
fn plain_bool(
    cmp: Option<Cmp>,
    key: &str,
    term: &str,
    value: &str,
    column: &str,
) -> Result<(String, Vec<Param>), RecorderError> {
    reject_cmp(cmp, key, term)?;
    let flag = boolean(value, term)?;
    Ok((format!("{column} = ?"), vec![Param::Int(i64::from(flag))]))
}

/// Ein Vergleich an einem Schlüssel, der keinen kennt, ist ein Fehler.
fn reject_cmp(cmp: Option<Cmp>, key: &str, term: &str) -> Result<(), RecorderError> {
    if cmp.is_some() {
        return Err(filter_failed(format!(
            "the filter term {term:?} compares with an operator, but {key}: takes a plain value; \
             only status: and findings: accept >, >=, < and <="
        )));
    }
    Ok(())
}

/// Trennt `key` und Rest am ersten Doppelpunkt, sofern davor ein Schlüsselwort steht.
fn split_key(term: &str) -> Option<(&str, &str)> {
    if term.starts_with('"') {
        return None;
    }
    let index = term.find(':')?;
    let key = term.get(..index)?;
    if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '_') {
        return None;
    }
    Some((key, term.get(index + 1..)?))
}

/// Trennt einen führenden Vergleich vom Wert.
fn split_cmp(value: &str) -> (Option<Cmp>, &str) {
    for (text, cmp) in [
        (">=", Cmp::Ge),
        ("<=", Cmp::Le),
        (">", Cmp::Gt),
        ("<", Cmp::Lt),
    ] {
        if let Some(rest) = value.strip_prefix(text) {
            return (Some(cmp), rest);
        }
    }
    (None, value)
}

/// Nimmt die Anführungszeichen weg, falls welche da sind.
fn unquote(value: &str) -> String {
    let trimmed = value.trim_matches('"');
    trimmed.to_owned()
}

/// Maskiert `LIKE`-Sonderzeichen mit `\`.
fn like_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Liest eine ganze Zahl.
fn number(value: &str, term: &str) -> Result<i64, RecorderError> {
    value.parse::<i64>().map_err(|_ignored| {
        filter_failed(format!(
            "the filter term {term:?} needs a whole number, {value:?} is not one"
        ))
    })
}

/// Liest einen Wahrheitswert.
fn boolean(value: &str, term: &str) -> Result<bool, RecorderError> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(filter_failed(format!(
            "the filter term {term:?} needs true or false, {value:?} is neither"
        ))),
    }
}

/// Liest eine Zeitangabe: `ISO-8601` oder eine relative Dauer wie `10m`.
fn timestamp(value: &str, now_ms: i64, term: &str) -> Result<i64, RecorderError> {
    if let Some(delta) = relative_millis(value) {
        return Ok(now_ms.saturating_sub(delta));
    }
    if let Ok(fixed) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(fixed.timestamp_millis());
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
        return Ok(naive.and_utc().timestamp_millis());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(date
            .and_time(chrono::NaiveTime::MIN)
            .and_utc()
            .timestamp_millis());
    }
    Err(filter_failed(format!(
        "the filter term {term:?} needs an ISO-8601 timestamp (2026-09-03T10:00:00Z) or a \
         relative duration (30s, 10m, 2h, 1d, 1w), {value:?} is neither"
    )))
}

/// Millisekunden hinter einer relativen Dauer wie `2h`.
fn relative_millis(value: &str) -> Option<i64> {
    let mut chars = value.chars();
    let unit = chars.next_back()?;
    let digits: String = chars.collect();
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let amount = digits.parse::<i64>().ok()?;
    let factor = match unit {
        's' | 'S' => 1_000,
        'm' | 'M' => 60 * 1_000,
        'h' | 'H' => 60 * 60 * 1_000,
        'd' | 'D' => 24 * 60 * 60 * 1_000,
        'w' | 'W' => 7 * 24 * 60 * 60 * 1_000,
        _ => return None,
    };
    amount.checked_mul(factor)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Filter, KEYS, Param, like_escape, parse, tokenize};

    fn ok(input: &str) -> Filter {
        parse(input, 1_000_000).unwrap_or_else(|err| panic!("{err}"))
    }

    #[test]
    fn an_empty_filter_takes_everything() {
        let filter = ok("   ");
        assert_eq!(filter.sql, "1");
        assert!(filter.params.is_empty());
    }

    #[test]
    fn quotes_hold_a_term_together() {
        assert_eq!(
            tokenize(r#"host:github.com path:"/a b" word"#),
            vec![
                "host:github.com".to_owned(),
                r#"path:"/a b""#.to_owned(),
                "word".to_owned()
            ]
        );
    }

    #[test]
    fn host_matches_the_label_suffix_and_lowercases() {
        let filter = ok("host:GitHub.com");
        assert_eq!(filter.sql, "(host_rev >= ? AND host_rev < ?)");
        assert_eq!(
            filter.params,
            vec![
                Param::Text("com.github.".to_owned()),
                Param::Text("com.github/".to_owned())
            ]
        );
    }

    #[test]
    fn comparisons_only_where_the_grammar_allows_them() {
        let filter = ok("status:>=400");
        assert_eq!(filter.sql, "status >= ?");
        assert_eq!(filter.params, vec![Param::Int(400)]);

        let filter = ok("findings:>0");
        assert_eq!(filter.sql, "findings_count > ?");

        let err = parse("host:>github.com", 0)
            .err()
            .unwrap_or_else(|| panic!("no error"));
        assert_eq!(err.diagnostic().code.as_str(), "RECORDER_002");
    }

    #[test]
    fn relative_and_absolute_times() {
        let filter = ok("since:10m");
        assert_eq!(filter.sql, "ts >= ?");
        assert_eq!(filter.params, vec![Param::Int(1_000_000 - 600_000)]);

        let filter = ok("until:2026-09-03T10:00:00Z");
        assert_eq!(filter.params, vec![Param::Int(1_788_429_600_000)]);

        let filter = ok("since:2026-09-03");
        assert_eq!(filter.params, vec![Param::Int(1_788_393_600_000)]);
    }

    #[test]
    fn an_unknown_key_names_the_key_and_the_valid_ones() {
        let err = parse("foo:bar", 0)
            .err()
            .unwrap_or_else(|| panic!("no error"));
        assert_eq!(err.diagnostic().code.as_str(), "RECORDER_002");
        assert!(err.diagnostic().why.contains("foo"));
        for key in KEYS {
            assert!(err.diagnostic().why.contains(key), "{key} missing");
        }
    }

    #[test]
    fn wildcards_in_a_value_stay_literal() {
        assert_eq!(like_escape("a%b_c\\d"), "a\\%b\\_c\\\\d");
        let filter = ok("path:%");
        assert_eq!(filter.params, vec![Param::Text("%\\%%".to_owned())]);
    }

    #[test]
    fn several_terms_are_joined_with_and() {
        let filter = ok("host:github.com state:held edited:true");
        assert_eq!(
            filter.sql,
            "(host_rev >= ? AND host_rev < ?) AND state = ? AND edited = ?"
        );
        assert_eq!(filter.params.len(), 4);
    }

    #[test]
    fn a_bare_word_searches_host_and_path() {
        let filter = ok("token");
        assert_eq!(
            filter.sql,
            "(host LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\')"
        );
        assert_eq!(
            filter.params,
            vec![
                Param::Text("%token%".to_owned()),
                Param::Text("%token%".to_owned())
            ]
        );
    }

    #[test]
    fn a_value_that_is_not_a_number_or_boolean_is_refused() {
        for input in [
            "status:soon",
            "findings:many",
            "edited:maybe",
            "since:later",
        ] {
            let err = parse(input, 0).err().unwrap_or_else(|| panic!("{input}"));
            assert_eq!(err.diagnostic().code.as_str(), "RECORDER_002");
        }
    }
}
