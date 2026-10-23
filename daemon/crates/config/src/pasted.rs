//! Was eine Zwischenablage an einem eingefügten Wert hinterlässt (HUM-283).
//!
//! Wer eine Adresse oder einen Zugangsschlüssel aus einer Konfigurationsdatei
//! kopiert, nimmt oft ein Anführungszeichen oder ein Leerzeichen mit:
//! `https://llm.example/api/v1/"` oder `"sk-…"`. Diese eine Funktion nimmt
//! genau das weg — Leerraum außen und ein Paar gleicher Anführungszeichen oder
//! ein einzelnes an einem Rand —, und nichts aus dem Inneren des Werts. Was
//! danach noch Leerraum oder Anführungszeichen trägt, ist kein Rest einer
//! Zwischenablage, und die Stelle, die den Wert prüft, lehnt es mit Grund ab.
//!
//! Dieselbe Regel steht in der Oberfläche (`app/lib/core/ui/pasted_text.dart`).

/// Die Zeichen, die als Anführungszeichen um einen eingefügten Wert gelten.
const QUOTES: [char; 2] = ['"', '\''];

/// Leerraum im Sinn dieser Bereinigung: alles, was `char::is_whitespace`
/// kennt, und die Bytereihenfolge-Marke U+FEFF, die manche Editoren an den
/// Anfang einer Datei schreiben (Darts `String.trim` nimmt sie auch weg).
fn is_blank(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// Der Wert ohne Leerraum außen und ohne die Anführungszeichen einer
/// Zwischenablage.
///
/// Ein Paar fällt nur weg, wenn beide Enden dasselbe Zeichen tragen
/// (`"wert"`, `'wert'`), samt Leerraum innerhalb (`" wert "`). Sonst fällt ein
/// einzelnes Anführungszeichen an genau einem Rand weg, zuerst am Anfang
/// (`"wert` und `wert"`); `"wert'` behält also sein `'`. Mehr nimmt die
/// Funktion nicht weg: `""wert""` behält ein Paar, und die Prüfung des Werts
/// sagt dann, was daran falsch ist.
#[must_use]
pub fn strip_paste_debris(text: &str) -> &str {
    let rest = text.trim_matches(is_blank);
    let first = rest.chars().next();
    let last = rest.chars().next_back();
    let is_quote = |c: Option<char>| c.is_some_and(|c| QUOTES.contains(&c));
    let inner = if rest.chars().count() >= 2 && first == last && is_quote(first) {
        rest.get(1..rest.len() - 1).unwrap_or_default()
    } else if is_quote(first) {
        rest.get(1..).unwrap_or_default()
    } else if is_quote(last) {
        rest.get(..rest.len() - 1).unwrap_or_default()
    } else {
        rest
    };
    inner.trim_matches(is_blank)
}

#[cfg(test)]
mod tests {
    use super::strip_paste_debris;

    /// Der Fall des Besitzers vom 2026-10-23 und seine Verwandten: ein Zeichen
    /// am Ende, ein Paar, Leerraum außen und innerhalb der Anführungszeichen.
    /// Die Meldung nennt nur die Nummer des Falls, nie den Wert: Unter den
    /// Fällen stehen Schlüssel.
    #[test]
    fn surrounding_white_space_and_quotes_are_removed() {
        for (case, (pasted, clean)) in [
            (
                "https://llm.hzo.example.net/api/v1/\"",
                "https://llm.hzo.example.net/api/v1/",
            ),
            ("\"sk-0123456789abcdef\"", "sk-0123456789abcdef"),
            ("'sk-0123456789abcdef'", "sk-0123456789abcdef"),
            ("  sk-0123456789abcdef\n", "sk-0123456789abcdef"),
            ("\" sk-0123456789abcdef \"\n", "sk-0123456789abcdef"),
            ("\"sk-0123456789abcdef", "sk-0123456789abcdef"),
            ("\u{feff}sk-0123456789abcdef", "sk-0123456789abcdef"),
            ("sk-0123456789abcdef", "sk-0123456789abcdef"),
            ("\"", ""),
            ("", ""),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(strip_paste_debris(pasted) == clean, "case {case}");
        }
    }

    /// Zwei verschiedene Anführungszeichen sind kein Paar: Nur das am Anfang
    /// fällt weg. Was im Inneren steht, bleibt.
    #[test]
    fn only_a_matching_pair_goes_and_nothing_inside_is_touched() {
        assert_eq!(strip_paste_debris("\"abc'"), "abc'");
        assert_eq!(strip_paste_debris("'abc\""), "abc\"");
        assert_eq!(strip_paste_debris("\"\"a b\"\""), "\"a b\"");
        assert_eq!(strip_paste_debris("http://h/a\"b"), "http://h/a\"b");
    }
}
