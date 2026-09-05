//! Das JSON-Schema der Konfiguration und der Weg, es zu durchlaufen.
//!
//! Aus dem Schema entstehen drei Dinge, die sonst auseinanderliefen: die
//! Prüfung unbekannter Schlüssel beim Laden, der Einstellungs-Bildschirm
//! (HUM-069) und `docs/CONFIG.md` (HUM-070). Es gibt deshalb genau einen
//! Durchlauf, [`fields`], und alle drei benutzen ihn.
//!
//! Jedes Blattfeld trägt neben Stufe und Vertrauensgrenze eine Einstufung, ob
//! es heute einen Leser hat ([`Field::readiness`], HUM-101). Sie kommt aus
//! `x-pending-issue` am Feld; das Register in
//! `daemon/crates/config/tests/config_readers.rs` hält sie vollständig.
//!
//! Untergeordnete Schemata werden eingebettet (`inline_subschemas`), nicht als
//! `$ref` abgelegt. Das macht den Durchlauf einfach und die Ausgabe für einen
//! Menschen lesbar; die Konfiguration ist flach genug, dass die Wiederholung
//! nicht weh tut.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::Value;

use crate::model::Config;
use crate::pending::{PENDING_ISSUE_KEY, Readiness};
use crate::scope::{PROJECT_SCOPE_KEY, ProjectScope};
use crate::tier::{TIER_KEY, Tier};

/// Ein Knoten des Schemas: ein Blattfeld oder eine Gruppe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Der Pfad, mit Punkten getrennt, zum Beispiel `hold.timeout_secs`.
    pub path: String,
    /// Die Sichtbarkeitsstufe aus `x-tier`.
    pub tier: Tier,
    /// Ob das Projekt-Profil den Wert setzen darf, aus `x-project-scope`.
    ///
    /// Fehlt der Wert im Schema, gilt `denied`: ein Feld ohne Entscheidung
    /// bleibt hinter der Vertrauensgrenze, statt versehentlich davor.
    pub project_scope: ProjectScope,
    /// Ob der Schlüssel heute einen Leser hat, aus `x-pending-issue`.
    ///
    /// Fehlt die Angabe im Schema, gilt [`Readiness::Effective`]: Ein Feld
    /// ohne Vermerk wirkt, und wer es anlegt, ohne es zu verdrahten, setzt den
    /// Vermerk (siehe [`crate::pending`]).
    pub readiness: Readiness,
    /// Der Doku-Kommentar des Feldes.
    pub description: String,
    /// Der Typ, wie ihn die Doku zeigt, zum Beispiel `list of string`.
    pub type_label: String,
    /// Der Vorgabewert, wie ihn `schemars` aus `impl Default` gelesen hat.
    pub default: Option<Value>,
    /// Die erlaubten JSON-Typen, zum Beispiel `["integer"]` oder `["string", "null"]`.
    pub types: Vec<String>,
    /// Die erlaubten Werte, wenn das Feld eine Aufzählung ist.
    pub allowed: Option<Vec<String>>,
    /// Die kleinste erlaubte Zahl, falls das Schema eine nennt.
    pub minimum: Option<i64>,
    /// Die größte erlaubte Zahl, falls das Schema eine nennt.
    pub maximum: Option<i64>,
    /// Ob der Knoten eine Gruppe ist, also selbst Felder enthält.
    pub group: bool,
    /// Ob der Knoten eine freie Tabelle ist, deren Schlüssel niemand kennt.
    pub free_table: bool,
}

impl Field {
    /// Der Vorgabewert als eine Zeile, wie er in `config.toml` stünde.
    #[must_use]
    pub fn default_literal(&self) -> String {
        match &self.default {
            None | Some(Value::Null) => "-".to_owned(),
            Some(value) => value.to_string(),
        }
    }
}

impl Config {
    /// Das JSON-Schema der Konfiguration.
    ///
    /// Jedes Blattfeld trägt `description`, `x-tier` und `x-project-scope`;
    /// Gruppen ebenso. Die Tests `every_leaf_has_tier_and_description` und
    /// `every_node_has_a_project_scope` halten das fest.
    #[must_use]
    pub fn json_schema() -> Value {
        json_schema()
    }
}

/// Das JSON-Schema der Konfiguration.
#[must_use]
pub fn json_schema() -> Value {
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator();
    generator.into_root_schema_for::<Config>().to_value()
}

/// Alle Knoten des Schemas, Gruppen wie Blätter, in Pfad-Reihenfolge.
///
/// Wird einmal berechnet und danach geteilt.
#[must_use]
pub fn fields() -> &'static [Field] {
    static FIELDS: OnceLock<Vec<Field>> = OnceLock::new();
    FIELDS.get_or_init(|| {
        let schema = json_schema();
        let mut out = Vec::new();
        walk(&schema, "", &mut out);
        out
    })
}

/// Alle Blattfelder, also alles ohne eigene Unterfelder.
#[must_use]
pub fn leaves() -> Vec<&'static Field> {
    fields().iter().filter(|field| !field.group).collect()
}

/// Alle Pfade, die das Schema kennt, Gruppen und Blätter.
#[must_use]
pub fn known_paths() -> BTreeSet<&'static str> {
    fields().iter().map(|field| field.path.as_str()).collect()
}

/// Die Pfade der Blattfelder.
///
/// Das Register der Leser (`tests/config_readers.rs`) vergleicht seine Zeilen
/// mit genau dieser Menge.
#[must_use]
pub fn leaf_paths() -> BTreeSet<&'static str> {
    leaves()
        .into_iter()
        .map(|field| field.path.as_str())
        .collect()
}

/// Die Pfade der freien Tabellen, deren Schlüssel nicht im Schema stehen.
///
/// Sie sind beim Mischen ein einziger Wert: eine höhere Ebene ersetzt die
/// Tabelle ganz, statt einzelne Schlüssel zu übernehmen. Sonst ließe sich ein
/// Eintrag aus einem Profil nicht mehr entfernen, und ein Punkt in einem
/// Hostnamen würde beim Zerlegen des Pfades zu einer neuen Ebene.
#[must_use]
pub fn free_table_paths() -> BTreeSet<&'static str> {
    fields()
        .iter()
        .filter(|field| field.free_table)
        .map(|field| field.path.as_str())
        .collect()
}

/// Das Feld zu einem Pfad.
#[must_use]
pub fn field(path: &str) -> Option<&'static Field> {
    fields().iter().find(|field| field.path == path)
}

/// Die Blattfelder je Gruppe, in Pfad-Reihenfolge.
#[must_use]
pub fn by_group() -> BTreeMap<&'static str, Vec<&'static Field>> {
    let mut out: BTreeMap<&'static str, Vec<&'static Field>> = BTreeMap::new();
    for field in fields().iter().filter(|field| !field.group) {
        let group = field.path.split_once('.').map_or("", |(group, _)| group);
        out.entry(group).or_default().push(field);
    }
    out
}

fn walk(node: &Value, prefix: &str, out: &mut Vec<Field>) {
    let Some(properties) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, child) in properties {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        let group = child
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|map| !map.is_empty());
        out.push(Field {
            path: path.clone(),
            tier: child
                .get(TIER_KEY)
                .and_then(Value::as_str)
                .and_then(Tier::parse)
                .unwrap_or(Tier::Expert),
            project_scope: child
                .get(PROJECT_SCOPE_KEY)
                .and_then(Value::as_str)
                .and_then(ProjectScope::parse)
                .unwrap_or(ProjectScope::Denied),
            readiness: Readiness::from_issue(child.get(PENDING_ISSUE_KEY).and_then(Value::as_str)),
            description: child
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            type_label: type_label(child),
            default: child.get("default").cloned(),
            types: types_of(child),
            allowed: enum_values(child),
            minimum: child.get("minimum").and_then(Value::as_i64),
            maximum: child.get("maximum").and_then(Value::as_i64),
            group,
            free_table: !group && is_free_table(child),
        });
        if group {
            walk(child, &path, out);
        }
    }
}

fn types_of(node: &Value) -> Vec<String> {
    match node.get("type") {
        Some(Value::String(name)) => vec![name.clone()],
        Some(Value::Array(names)) => names
            .iter()
            .filter_map(|name| name.as_str().map(ToOwned::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_free_table(node: &Value) -> bool {
    has_type(node, "object")
        && node
            .get("additionalProperties")
            .is_some_and(Value::is_object)
}

fn has_type(node: &Value, wanted: &str) -> bool {
    match node.get("type") {
        Some(Value::String(name)) => name == wanted,
        Some(Value::Array(names)) => names.iter().any(|name| name.as_str() == Some(wanted)),
        _ => false,
    }
}

fn type_label(node: &Value) -> String {
    if let Some(values) = enum_values(node) {
        return values.join(" | ");
    }
    let optional = has_type(node, "null");
    let base = if has_type(node, "array") {
        let item = node
            .get("items")
            .map_or_else(|| "value".to_owned(), type_label);
        format!("list of {item}")
    } else if has_type(node, "object") {
        let value = node
            .get("additionalProperties")
            .map_or_else(|| "value".to_owned(), type_label);
        format!("table of {value}")
    } else if has_type(node, "integer") {
        "integer".to_owned()
    } else if has_type(node, "number") {
        "number".to_owned()
    } else if has_type(node, "boolean") {
        "boolean".to_owned()
    } else if has_type(node, "string") {
        "string".to_owned()
    } else {
        "value".to_owned()
    };
    if optional {
        format!("{base}, optional")
    } else {
        base
    }
}

fn enum_values(node: &Value) -> Option<Vec<String>> {
    if let Some(values) = node.get("enum").and_then(Value::as_array) {
        return Some(values.iter().map(render_scalar).collect());
    }
    let variants = node.get("oneOf").and_then(Value::as_array)?;
    let mut out = Vec::with_capacity(variants.len());
    for variant in variants {
        let value = variant.get("const")?;
        out.push(render_scalar(value));
    }
    Some(out)
}

fn render_scalar(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{by_group, field, free_table_paths, json_schema, known_paths, leaf_paths};
    use crate::model::Config;
    use crate::scope::ProjectScope;

    #[test]
    fn the_schema_has_a_title_and_the_top_level_groups() {
        let schema = json_schema();
        assert_eq!(schema.get("title").and_then(|v| v.as_str()), Some("Config"));
        for group in ["llm", "hold", "limits", "ui"] {
            assert!(known_paths().contains(group), "{group} is missing");
        }
    }

    #[test]
    fn leaves_carry_type_default_and_tier() {
        let Some(timeout) = field("hold.timeout_secs") else {
            panic!("hold.timeout_secs is missing from the schema");
        };
        assert_eq!(timeout.type_label, "integer");
        assert_eq!(timeout.default_literal(), "300");
        assert!(!timeout.group);
        assert!(!timeout.description.is_empty());

        let Some(ask) = field("hold.ask_mode") else {
            panic!("hold.ask_mode is missing from the schema");
        };
        assert_eq!(ask.type_label, "ui | terminal | none");

        let Some(paths) = field("llm.passthrough_paths") else {
            panic!("llm.passthrough_paths is missing from the schema");
        };
        assert_eq!(paths.type_label, "list of string");
    }

    #[test]
    fn leaves_carry_their_project_scope() {
        let Some(endpoint) = field("llm.endpoint") else {
            panic!("llm.endpoint is missing from the schema");
        };
        assert_eq!(endpoint.project_scope, ProjectScope::Denied);
        let Some(timeout) = field("hold.timeout_secs") else {
            panic!("hold.timeout_secs is missing from the schema");
        };
        assert_eq!(timeout.project_scope, ProjectScope::Allowed);
    }

    #[test]
    fn nested_groups_are_walked() {
        assert!(leaf_paths().contains("agent.briefing.enabled"));
        assert!(known_paths().contains("agent.briefing"));
        assert!(!leaf_paths().contains("agent.briefing"));
    }

    #[test]
    fn free_tables_are_leaves() {
        let free = free_table_paths();
        assert!(free.contains("resolver.overrides"), "{free:?}");
        assert!(free.contains("experimental.upstream_port_map"), "{free:?}");
        assert!(!free.contains("llm.passthrough_paths"));
    }

    #[test]
    fn the_derived_trait_still_works_next_to_the_inherent_function() {
        // `Config::json_schema()` ist eine eigene Funktion und verdeckt die
        // gleichnamige Methode des Traits. Wer `Config` in einen anderen Typ
        // einbettet, braucht die Methode; dieser Test hält sie erreichbar.
        let mut generator = schemars::SchemaGenerator::default();
        let via_trait = <Config as schemars::JsonSchema>::json_schema(&mut generator);
        assert!(via_trait.as_object().is_some());
        assert!(Config::json_schema().get("properties").is_some());
    }

    #[test]
    fn the_defaults_survive_a_round_trip_through_toml() {
        let text = toml::to_string(&Config::default()).expect("Config serializes to TOML");
        let back: Config = toml::from_str(&text).expect("Config reads its own output");
        assert_eq!(back, Config::default());
        assert!(text.contains("timeout_secs = 300"), "{text}");
    }

    #[test]
    fn every_group_has_at_least_one_leaf() {
        for (group, leaves) in by_group() {
            assert!(!group.is_empty(), "a leaf sits at the top level");
            assert!(!leaves.is_empty(), "{group} has no leaf");
        }
    }
}
