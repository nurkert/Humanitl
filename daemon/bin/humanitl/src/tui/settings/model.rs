//! Was der Bildschirm aus einer Antwort von `GetConfig` liest: welche
//! Kategorien und Zeilen er zeigt, welcher Wert in einer Zeile steht, und ob
//! sie sich ändern lässt (GitHub #7).
//!
//! Nichts davon ist eine eigene Liste. Kategorien, Reihenfolge, Texte,
//! Bedienart, Herkunft und Sperre stehen im Katalog des Daemons; dieses Modul
//! sortiert nur nach den Zahlen, die er mitschickt, und lässt die Zeilen
//! weg, die der Katalog selbst der App vorbehält (`client = app`).

use humanitl_ipc::v1;
use serde_json::Value;

/// Der Wert von `SettingsEntry.client` für eine Einstellung des App-Fensters
/// selbst, die die TUI nicht zeigt.
pub const APP_ONLY: &str = "app";

/// Ob die TUI einen Eintrag zeigt: jeder, den der Katalog nicht der App
/// vorbehält.
#[must_use]
pub fn shown(entry: &v1::SettingsEntry) -> bool {
    entry.client != APP_ONLY
}

/// Die Kategorien, die mindestens eine gezeigte Zeile haben, in der
/// Reihenfolge des Katalogs.
#[must_use]
pub fn categories(catalog: &v1::SettingsCatalog) -> Vec<&v1::SettingsCategory> {
    let mut categories: Vec<&v1::SettingsCategory> = catalog
        .categories
        .iter()
        .filter(|category| {
            catalog
                .entries
                .iter()
                .any(|entry| entry.category == category.id && shown(entry))
        })
        .collect();
    categories.sort_by_key(|category| category.order);
    categories
}

/// Die gezeigten Zeilen einer Kategorie in der Reihenfolge ihrer Seite.
#[must_use]
pub fn entries<'a>(catalog: &'a v1::SettingsCatalog, category: &str) -> Vec<&'a v1::SettingsEntry> {
    let mut entries: Vec<&v1::SettingsEntry> = catalog
        .entries
        .iter()
        .filter(|entry| entry.category == category && shown(entry))
        .collect();
    entries.sort_by_key(|entry| entry.order);
    entries
}

/// Alle gezeigten Zeilen, Kategorie für Kategorie, für die Suche.
#[must_use]
pub fn all_entries(catalog: &v1::SettingsCatalog) -> Vec<&v1::SettingsEntry> {
    categories(catalog)
        .into_iter()
        .flat_map(|category| entries(catalog, &category.id))
        .collect()
}

/// Der Text, in dem die Suche nach einer Zeile sucht: Name, Hilfe,
/// Erklärung, Stichwörter, Kennung und der Name der Kategorie.
#[must_use]
pub fn haystack(catalog: &v1::SettingsCatalog, entry: &v1::SettingsEntry) -> String {
    let category = catalog
        .categories
        .iter()
        .find(|category| category.id == entry.category)
        .map(|category| category.title.as_str())
        .unwrap_or_default();
    let mut text = vec![
        entry.title.as_str(),
        entry.help.as_str(),
        entry.details.as_str(),
        entry.id.as_str(),
        category,
    ];
    text.extend(entry.keywords.iter().map(String::as_str));
    text.join(" ")
}

/// Der wirksame Wert eines Schlüssels aus `ConfigSnapshot.values_json`, oder
/// `None`, wenn die Antwort ihn nicht trägt.
#[must_use]
pub fn value<'a>(values: &'a Value, id: &str) -> Option<&'a Value> {
    let pointer = format!("/{}", id.replace('.', "/"));
    values.pointer(&pointer).filter(|value| !value.is_null())
}

/// Ein Wert als der Text, den `SetConfig` nehmen würde: ein Text ohne
/// Anführungszeichen, eine Liste mit Komma, alles andere als JSON.
#[must_use]
pub fn as_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(as_text).collect::<Vec<_>>().join(", "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Ob ein Schalter an ist.
#[must_use]
pub fn is_on(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

/// Der Name eines Werts einer Auswahl, wie der Katalog ihn nennt; ohne
/// Namen der Wert selbst.
#[must_use]
pub fn choice_title(entry: &v1::SettingsEntry, value: &str) -> String {
    entry
        .choices
        .iter()
        .find(|choice| choice.value == value)
        .map_or_else(|| value.to_owned(), |choice| choice.title.clone())
}

/// Text aus einer Antwort des Daemons, wie er in eine Zelle darf.
///
/// Ein Wert, ein Name oder ein `why` kann Steuerzeichen tragen, und ein
/// Terminal führt sie aus. Weg fallen Steuerzeichen und die unsichtbaren
/// Zeichen, die Text umdrehen oder verstecken (Bidi-Steuerung, Zeichen ohne
/// Breite); Zeilenumbrüche werden zu Leerzeichen. Anders als
/// `render::plain` kürzt diese Funktion nicht: Eine lange Erklärung soll
/// ganz dastehen und wird nach Breite umbrochen.
#[must_use]
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' || ch == '\t' {
            out.push(' ');
        } else if !ch.is_control() && !invisible(ch) {
            out.push(ch);
        }
    }
    out
}

/// Zeichen, die nichts zeigen und doch etwas bewirken.
const fn invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::json;

    use super::{as_text, clean, value};

    /// Ein Schlüssel mit Punkten wird zu einem Pfad im JSON; `null` zählt
    /// als nicht gesetzt.
    #[test]
    fn a_dotted_key_is_a_path_in_the_values() {
        let values = json!({"egress": {"proxy": "http://p:8080", "proxy_ca": null}});
        assert_eq!(
            value(&values, "egress.proxy").map(as_text).as_deref(),
            Some("http://p:8080")
        );
        assert!(value(&values, "egress.proxy_ca").is_none());
        assert!(value(&values, "nowhere.at.all").is_none());
        assert_eq!(as_text(&json!(["a", "b"])), "a, b");
        assert_eq!(as_text(&json!(300)), "300");
    }

    /// Steuerzeichen, Bidi-Umkehr und Zeichen ohne Breite kommen nicht in
    /// eine Zelle; Umlaute und Zeichen anderer Schriften bleiben.
    #[test]
    fn clean_drops_what_a_terminal_would_execute() {
        assert_eq!(clean("a\u{1b}]8;;x\u{7}b"), "a]8;;xb");
        assert_eq!(clean("ab\u{202E}cd\u{200B}"), "abcd");
        assert_eq!(clean("Grüße\n日本"), "Grüße 日本");
    }
}
