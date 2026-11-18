//! Der Einstellungskatalog gegen das Schema und die Sprachdateien (HUM-534).
//!
//! Vier Zusagen hält diese Datei:
//!
//! 1. Jedes Blatt des Schemas hat einen Eintrag oder steht in
//!    `catalog/hidden.toml` mit Grund, nie beides. Ein neues Feld ohne beides
//!    färbt `every_leaf_has_an_entry_or_a_reason` rot.
//! 2. Jeder Eintrag, jede Kategorie und jeder Auswahlwert hat einen Namen in
//!    `en` und `de`; eine Sprachdatei nennt nichts, was der Katalog nicht kennt.
//! 3. Kein Text trägt ein Steuerzeichen: Die Clients zeigen ihn, wie er ist.
//! 4. Die Vorlage des Fakes der App (`app/lib/core/ipc/fake/generated/settings_catalog.dart`)
//!    ist der Katalog Byte für Byte. Mit `UPDATE_SETTINGS_CATALOG=1` schreibt
//!    der Test sie neu; `cargo xtask docs` tut das für alle erzeugten Dateien.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use humanitl_config::catalog::{self, CATEGORIES, Control, DEFS, Texts};
use humanitl_config::{Language, PROJECT_CHOICE_KEYS, schema};

/// Was fehlt und was zu viel ist: Blätter ohne Eintrag und ohne Grund, und
/// Einträge oder Gründe für Pfade, die kein Blatt sind.
fn drift(leaves: &BTreeSet<&str>, covered: &BTreeSet<&str>) -> (Vec<String>, Vec<String>) {
    let missing = leaves
        .difference(covered)
        .map(|path| (*path).to_owned())
        .collect();
    let stale = covered
        .difference(leaves)
        .map(|path| (*path).to_owned())
        .collect();
    (missing, stale)
}

fn keyed_entries() -> BTreeSet<&'static str> {
    DEFS.iter()
        .filter(|def| def.is_key())
        .map(|def| def.id)
        .collect()
}

fn hidden() -> BTreeSet<&'static str> {
    catalog::hidden_paths().into_iter().collect()
}

/// Spezifikation 4 und Akzeptanzkriterium 2. Mutationsbeweis: ein neues Feld
/// im Schema (`ui.example: bool`) ohne Eintrag und ohne Grund färbt den Test
/// rot; dasselbe zeigt `a_new_leaf_without_an_entry_is_named` ohne Umbau.
#[test]
fn every_leaf_has_an_entry_or_a_reason() {
    let leaves = schema::leaf_paths();
    let entries = keyed_entries();
    let hidden = hidden();
    let both: Vec<_> = entries.intersection(&hidden).collect();
    assert!(both.is_empty(), "entry and hidden at once: {both:?}");
    let covered: BTreeSet<&str> = entries.union(&hidden).copied().collect();
    let (missing, _) = drift(&leaves, &covered);
    assert!(
        missing.is_empty(),
        "schema leaves without a catalog entry and without a line in catalog/hidden.toml: \
         {missing:?}"
    );
    let stale: Vec<_> = hidden.difference(&leaves).collect();
    assert!(
        stale.is_empty(),
        "catalog/hidden.toml names no leaf: {stale:?}"
    );
    let file = catalog::parse_hidden().unwrap();
    for (path, reason) in &file.hidden {
        assert!(
            file.reasons
                .get(reason)
                .is_some_and(|text| !text.trim().is_empty()),
            "{path} names the unknown or empty reason {reason}"
        );
        assert!(catalog::hidden_reason(path).is_some(), "{path}");
    }
}

#[test]
fn a_new_leaf_without_an_entry_is_named() {
    let mut leaves = schema::leaf_paths();
    leaves.insert("ui.example");
    let covered: BTreeSet<&str> = keyed_entries().union(&hidden()).copied().collect();
    let (missing, _) = drift(&leaves, &covered);
    assert_eq!(missing, vec!["ui.example".to_owned()]);
}

#[test]
fn ids_are_unique_and_every_category_has_entries() {
    let ids: BTreeSet<&str> = DEFS.iter().map(|def| def.id).collect();
    assert_eq!(ids.len(), DEFS.len(), "an id stands twice");
    for category in CATEGORIES {
        assert!(
            DEFS.iter().any(|def| def.category == category),
            "{category} has no entry"
        );
    }
    for def in DEFS {
        assert!(
            CATEGORIES.contains(&def.category),
            "{} names the unknown category {}",
            def.id,
            def.category
        );
    }
}

/// Die Bedienart passt zum Typ des Feldes: Ein Schalter ist ein `boolean`,
/// eine Zahl ein `integer`, eine Auswahl hat Werte.
#[test]
fn the_control_fits_the_schema_type() {
    for def in DEFS.iter().filter(|def| def.is_key()) {
        let field = schema::field(def.id).unwrap();
        let has = |kind: &str| field.types.iter().any(|t| t == kind);
        match def.control {
            Control::Switch => assert!(has("boolean"), "{}: {:?}", def.id, field.types),
            Control::Number => assert!(has("integer"), "{}: {:?}", def.id, field.types),
            Control::Choice => assert!(
                !catalog::choice_values(def).is_empty(),
                "{} is a choice without values",
                def.id
            ),
            Control::Text | Control::Path | Control::SecretRef => assert!(
                has("string") || has("array"),
                "{}: {:?}",
                def.id,
                field.types
            ),
            Control::Special => {}
        }
        if def.control == Control::Choice
            && def.choices.is_empty()
            && let Some(allowed) = &field.allowed
        {
            assert_eq!(&catalog::choice_values(def), allowed, "{}", def.id);
        }
    }
}

fn texts(language: Language) -> Texts {
    catalog::parse_texts(language).unwrap_or_else(|err| panic!("{language:?}: {err}"))
}

/// Spezifikation 4, zweiter Satz: jeder Eintrag mit Titel in `en` und `de`,
/// in der Datei selbst und nicht erst über den Rückfall. Hat `en` Hilfe oder
/// Erklärung, hat `de` sie auch.
#[test]
fn every_entry_category_and_choice_has_a_title_in_both_languages() {
    let en = texts(Language::En);
    let de = texts(Language::De);
    for (name, file) in [("en", &en), ("de", &de)] {
        for category in CATEGORIES {
            assert!(
                file.categories
                    .get(category)
                    .is_some_and(|t| !t.trim().is_empty()),
                "{name}: category {category} has no title"
            );
        }
        for def in DEFS {
            let entry = file.entries.get(def.id);
            assert!(
                entry.is_some_and(|e| !e.title.trim().is_empty()),
                "{name}: {} has no title",
                def.id
            );
            for value in catalog::choice_values(def) {
                assert!(
                    file.choices
                        .get(def.id)
                        .and_then(|names| names.get(&value))
                        .is_some_and(|t| !t.trim().is_empty()),
                    "{name}: choice {value} of {} has no title",
                    def.id
                );
            }
        }
    }
    for def in DEFS {
        let source = &en.entries[def.id];
        let translated = &de.entries[def.id];
        for (what, a, b) in [
            ("help", &source.help, &translated.help),
            ("details", &source.details, &translated.details),
            ("help_macos", &source.help_macos, &translated.help_macos),
        ] {
            assert_eq!(
                a.is_empty(),
                b.is_empty(),
                "{}: {what} only in one language",
                def.id
            );
        }
        assert!(
            !translated.keywords.is_empty(),
            "{}: de has no keywords",
            def.id
        );
    }
}

/// Eine Sprachdatei nennt nichts, was der Katalog nicht kennt: Ein Tippfehler
/// in einer Kennung wäre sonst ein Text, den niemand je sieht.
#[test]
fn the_language_files_name_nothing_the_catalog_lacks() {
    let ids: BTreeSet<&str> = DEFS.iter().map(|def| def.id).collect();
    for language in [Language::En, Language::De] {
        let file = texts(language);
        for id in file.entries.keys() {
            assert!(
                ids.contains(id.as_str()),
                "{language:?}: unknown entry {id}"
            );
        }
        for category in file.categories.keys() {
            assert!(
                CATEGORIES.contains(&category.as_str()),
                "{language:?}: unknown category {category}"
            );
        }
        for (id, names) in &file.choices {
            let def = DEFS
                .iter()
                .find(|def| def.id == id)
                .unwrap_or_else(|| panic!("{language:?}: choices of unknown entry {id}"));
            let values = catalog::choice_values(def);
            for value in names.keys() {
                assert!(
                    values.contains(value),
                    "{language:?}: {id} has no choice {value}"
                );
            }
        }
    }
}

/// Fallstrick der Spezifikation: Der Katalog enthält keine Steuerzeichen,
/// der Client säubert nichts zusätzlich.
#[test]
fn no_text_holds_a_control_character() {
    for language in [Language::En, Language::De] {
        for macos in [false, true] {
            let built = catalog::catalog_for(language, macos);
            let mut all: Vec<&str> = built
                .categories
                .iter()
                .map(|category| category.title.as_str())
                .collect();
            for entry in &built.entries {
                all.extend([
                    entry.title.as_str(),
                    entry.help.as_str(),
                    entry.details.as_str(),
                ]);
                all.extend(entry.keywords.iter().map(String::as_str));
                all.extend(entry.choices.iter().map(|choice| choice.title.as_str()));
            }
            for text in all {
                assert!(
                    !text.chars().any(char::is_control),
                    "{language:?}: control character in {text:?}"
                );
                assert!(!text.contains("'''"), "{text:?} would end the Dart string");
            }
        }
    }
}

#[test]
fn the_catalog_keeps_the_order_and_falls_back_to_english() {
    let en = catalog::catalog(Language::En);
    let de = catalog::catalog(Language::De);
    assert_eq!(en.categories.len(), CATEGORIES.len());
    assert_eq!(en.categories[0].title, "Models");
    assert_eq!(de.categories[0].title, "Modelle");
    let ids: Vec<&str> = en.entries.iter().map(|entry| entry.id).collect();
    let defs: Vec<&str> = DEFS.iter().map(|def| def.id).collect();
    assert_eq!(ids, defs);
    let timeout = en
        .entries
        .iter()
        .find(|entry| entry.id == "hold.ask_mode")
        .unwrap();
    assert_eq!(timeout.order, 1, "second row of Requests");
    assert_eq!(timeout.choices[0].value, "ui");
    assert_eq!(timeout.choices[0].title, "In the app");
    assert_eq!(catalog::language_of("de-DE", Language::En), Language::De);
    assert_eq!(catalog::language_of("", Language::De), Language::De);
    assert_eq!(catalog::language_of("fr", Language::De), Language::En);
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../app/lib/core/ipc/fake/generated/settings_catalog.dart")
}

fn catalog_json(language: Language) -> serde_json::Value {
    let built = catalog::catalog_for(language, false);
    serde_json::json!({
        "language": match language { Language::En => "en", Language::De => "de" },
        "categories": built.categories.iter().map(|category| serde_json::json!({
            "id": category.id,
            "order": category.order,
            "title": category.title,
        })).collect::<Vec<_>>(),
        "entries": built.entries.iter().map(|entry| serde_json::json!({
            "id": entry.id,
            "category": entry.category,
            "order": entry.order,
            "title": entry.title,
            "help": entry.help,
            "details": entry.details,
            "keywords": entry.keywords,
            "control": entry.control.as_str(),
            "choices": entry.choices.iter().map(|choice| serde_json::json!({
                "value": choice.value,
                "title": choice.title,
            })).collect::<Vec<_>>(),
            "nextSession": entry.next_session,
            "tier": entry.tier.as_str(),
            "client": entry.client.as_str(),
            "inProjectChoice": PROJECT_CHOICE_KEYS.contains(&entry.id),
            // Nur für den Fake, nicht auf der Leitung: ob die Zeile ein
            // Schlüssel ist, damit er wie der Daemon `default` als Herkunft
            // nennt.
            "isKey": schema::field(entry.id).is_some(),
        })).collect::<Vec<_>>(),
    })
}

fn fixture_text() -> String {
    let both = serde_json::json!({
        "en": catalog_json(Language::En),
        "de": catalog_json(Language::De),
    });
    let json = serde_json::to_string_pretty(&both).unwrap();
    format!(
        "// GENERATED by daemon/crates/config/tests/catalog.rs (HUM-534); do not edit.\n\
         // `cargo xtask docs` writes it, `cargo xtask docs --check` compares it.\n\
         //\n\
         // The settings catalog of the daemon as proto3 JSON of `SettingsCatalog`,\n\
         // per language, for the fake daemon of the app. The fake adds origin and\n\
         // lock from its own state, as the daemon does.\n\
         library;\n\
         \n\
         /// The catalog per language code, as proto3 JSON.\n\
         const String fakeSettingsCatalogJson = r'''\n{json}\n''';\n"
    )
}

/// Die Vorlage des Fakes der App ist der Katalog (Spezifikation 1: Die App
/// hat keine eigene Liste, auch nicht im Test).
#[test]
fn the_fake_catalog_of_the_app_is_the_catalog() {
    let path = fixture_path();
    let wanted = fixture_text();
    if std::env::var_os("UPDATE_SETTINGS_CATALOG").is_some() {
        std::fs::write(&path, &wanted).unwrap();
        return;
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        on_disk == wanted,
        "{} is missing or stale; run `cargo xtask docs` (or UPDATE_SETTINGS_CATALOG=1 cargo test \
         -p humanitl-config --test catalog) and commit the result",
        path.display()
    );
}
