//! Die Prüfung hinter dem Fuzz-Target `flow_filter` (HUM-056).
//!
//! Eine Quelle für beide Seiten: `main.rs` bindet die Datei als Modul ein, das
//! Target unter `daemon/fuzz/fuzz_targets/flow_filter.rs` über `#[path]`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_recorder::parse_filter;

/// Der Bezugspunkt relativer Zeitangaben; ein Target liest keine Uhr.
pub const NOW_MS: i64 = 1_790_000_000_000;

/// Das eine Literal, das die Übersetzung selbst schreibt: das Fluchtzeichen
/// eines `LIKE`.
const ESCAPE_LITERAL: &str = "ESCAPE '\\'";

/// Übersetzt `data` als Filterausdruck und prüft das erzeugte `SQL`.
///
/// Invarianten:
///
/// - Kein Panic, auch nicht bei ungültigem UTF-8 (dann gibt es nichts zu
///   übersetzen).
/// - Kein Wert aus der Eingabe steht im `SQL`: Ohne das eine feste Literal
///   [`ESCAPE_LITERAL`] besteht es nur aus Spaltennamen, Operatoren,
///   Klammern, Leerzeichen und Platzhaltern. Ein `'`, ein `"`, ein `;` oder
///   ein `-` aus der Eingabe hätte darin keinen Platz.
/// - Jeder Platzhalter hat genau einen Wert.
pub fn flow_filter(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(filter) = parse_filter(text, NOW_MS) else {
        return;
    };
    assert_sql_is_closed(&filter.sql);
    assert_eq!(
        filter.sql.matches('?').count(),
        filter.params.len(),
        "placeholders and values differ in {:?}",
        filter.sql
    );
}

/// Prüft, dass `sql` nur aus den Bausteinen der Übersetzung besteht.
pub fn assert_sql_is_closed(sql: &str) {
    let stripped = sql.replace(ESCAPE_LITERAL, "");
    let foreign: Vec<char> = stripped
        .chars()
        .filter(|ch| !(ch.is_ascii_alphanumeric() || " _()?=<>".contains(*ch)))
        .collect();
    assert!(
        foreign.is_empty(),
        "the SQL {sql:?} carries characters from outside the translation: {foreign:?}"
    );
}
