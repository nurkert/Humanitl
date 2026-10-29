//! The bundled templates of the pi adapter (HUM-274).
//!
//! Two files from `agents/pi/` are compiled in, so nothing is read from the
//! file system at run time and nobody can swap them between build and start:
//!
//! - [`MODELS_TEMPLATE`] becomes `<agent-dir>/models.json`, the custom
//!   provider that points pi at `llm.endpoint`.
//! - [`SETTINGS_TEMPLATE`] becomes `<agent-dir>/settings.json`, which selects
//!   that provider and its first model and switches off install telemetry.
//!
//! Like the `OpenCode` templates, both carry their placeholders as values and
//! are parsed and rebuilt with `serde_json`, never filled in as text: a model
//! name with a quote must not be able to write foreign structure into the
//! file (HUM-037, pitfall 5).

use humanitl_core::Diagnostic;
use serde_json::{Map, Value};

use crate::agent::opencode_models::{PROVIDER_ID, broken, effective_models};

/// The template of `models.json`.
pub const MODELS_TEMPLATE: &str = include_str!("../../../../../agents/pi/models.json.tmpl");

/// The template of `settings.json`.
pub const SETTINGS_TEMPLATE: &str = include_str!("../../../../../agents/pi/settings.json.tmpl");

/// The path of [`MODELS_TEMPLATE`] in the repository; stands in findings.
const MODELS_FILE: &str = "agents/pi/models.json.tmpl";

/// The path of [`SETTINGS_TEMPLATE`] in the repository; stands in findings.
const SETTINGS_FILE: &str = "agents/pi/settings.json.tmpl";

/// What `apiKey` holds when no key is stored.
///
/// pi offers a model only when its provider has a key it can resolve; without
/// one it starts with no model at all. A server without authentication ignores
/// the value, and the value says where it comes from when it shows up in a
/// recorded `Authorization` header.
pub const NO_KEY_PLACEHOLDER: &str = "humanitl-no-key";

/// The `apiKey` value that makes pi read the key from this file.
///
/// pi runs a value that starts with `!` through `/bin/sh -c` and takes the
/// trimmed standard output as the key. The path is a constant of the adapter
/// and never a value from the configuration, so nothing a user or a project
/// writes reaches that shell line.
#[must_use]
pub fn api_key_command(key_file: &str) -> String {
    format!("!cat {key_file}")
}

/// Reads a template as a JSON object.
fn parse_object(file: &str, text: &str) -> Result<Map<String, Value>, Diagnostic> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(broken(file, "the top level is not an object")),
        Err(err) => Err(broken(file, &format!("invalid JSON ({err})"))),
    }
}

/// A child object, with a finding instead of an index access.
fn object_mut<'a>(
    file: &str,
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, Diagnostic> {
    parent
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            broken(
                file,
                &format!("the key {key:?} is missing or not an object"),
            )
        })
}

/// Pretty JSON with a final newline.
fn write_back(file: &str, doc: Map<String, Value>) -> Result<String, Diagnostic> {
    serde_json::to_string_pretty(&Value::Object(doc))
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|err| broken(file, &format!("could not be written back ({err})")))
}

/// Renders `models.json` for an endpoint, a model list and the `apiKey`
/// value ([`api_key_command`] or [`NO_KEY_PLACEHOLDER`]).
///
/// The first entry of `models` in the template is the pattern of one model;
/// it is repeated once per model with `id` and `name` set. Its limits are
/// cautious on purpose and documented in `agents/pi/README.md`.
///
/// # Errors
///
/// A [`Diagnostic`] with `AGENT_003` when the template does not have the
/// expected form.
pub fn render_models(
    base_url: &str,
    models: &[String],
    api_key: &str,
) -> Result<String, Diagnostic> {
    let models = effective_models(models);
    let mut doc = parse_object(MODELS_FILE, MODELS_TEMPLATE)?;
    {
        let providers = object_mut(MODELS_FILE, &mut doc, "providers")?;
        let provider = object_mut(MODELS_FILE, providers, PROVIDER_ID)?;
        let pattern = provider
            .get("models")
            .and_then(Value::as_array)
            .and_then(|list| list.first())
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| broken(MODELS_FILE, "the model pattern under \"models\" is missing"))?;
        provider.insert("baseUrl".to_owned(), Value::String(base_url.to_owned()));
        provider.insert("apiKey".to_owned(), Value::String(api_key.to_owned()));
        let entries = models
            .iter()
            .map(|model| {
                let mut entry = pattern.clone();
                entry.insert("id".to_owned(), Value::String(model.clone()));
                entry.insert("name".to_owned(), Value::String(model.clone()));
                Value::Object(entry)
            })
            .collect();
        provider.insert("models".to_owned(), Value::Array(entries));
    }
    write_back(MODELS_FILE, doc)
}

/// Renders `settings.json`: the bundled provider and the first model.
///
/// # Errors
///
/// A [`Diagnostic`] with `AGENT_003` when the template does not have the
/// expected form.
pub fn render_settings(models: &[String]) -> Result<String, Diagnostic> {
    let models = effective_models(models);
    let first = models
        .first()
        .ok_or_else(|| broken(SETTINGS_FILE, "the model list came out empty"))?;
    let mut doc = parse_object(SETTINGS_FILE, SETTINGS_TEMPLATE)?;
    if !doc.contains_key("defaultModel") {
        return Err(broken(SETTINGS_FILE, "the key \"defaultModel\" is missing"));
    }
    doc.insert(
        "defaultProvider".to_owned(),
        Value::String(PROVIDER_ID.to_owned()),
    );
    doc.insert("defaultModel".to_owned(), Value::String(first.clone()));
    write_back(SETTINGS_FILE, doc)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::Value;

    use super::{NO_KEY_PLACEHOLDER, PROVIDER_ID, render_models, render_settings};

    #[test]
    fn a_model_name_with_a_quote_stays_inside_its_string() {
        let evil = "\", \"injected\": {\"id\": \"x\"}, \"a\": \"".to_owned();
        let text = render_models(
            "http://x:1/v1",
            std::slice::from_ref(&evil),
            NO_KEY_PLACEHOLDER,
        )
        .unwrap();
        let doc: Value = serde_json::from_str(&text).unwrap();
        let models = doc["providers"][PROVIDER_ID]["models"].as_array().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["id"], Value::String(evil.clone()));
        assert!(doc.get("injected").is_none());
        assert!(doc["providers"][PROVIDER_ID].get("injected").is_none());
    }

    #[test]
    fn every_model_keeps_the_limits_of_the_pattern() {
        let text = render_models(
            "http://x:1/v1",
            &["a".to_owned(), "b".to_owned()],
            NO_KEY_PLACEHOLDER,
        )
        .unwrap();
        let doc: Value = serde_json::from_str(&text).unwrap();
        let models = doc["providers"][PROVIDER_ID]["models"].as_array().unwrap();
        assert_eq!(models.len(), 2);
        for (model, id) in models.iter().zip(["a", "b"]) {
            assert_eq!(model["id"], Value::String(id.to_owned()));
            assert_eq!(model["name"], Value::String(id.to_owned()));
            assert!(model["contextWindow"].as_u64().is_some(), "{model}");
            assert!(model["maxTokens"].as_u64().is_some(), "{model}");
        }
        assert!(!text.contains("{{"), "no placeholder survives: {text}");
    }

    #[test]
    fn the_settings_name_the_first_model() {
        let doc: Value =
            serde_json::from_str(&render_settings(&["  ".to_owned(), "m2".to_owned()]).unwrap())
                .unwrap();
        assert_eq!(
            doc["defaultProvider"],
            Value::String(PROVIDER_ID.to_owned())
        );
        assert_eq!(doc["defaultModel"], Value::String("m2".to_owned()));
        assert_eq!(doc["enableInstallTelemetry"], Value::Bool(false));
    }

    /// pi 1.4.0 fragt vor keinem Werkzeugaufruf (`docs/security.md` des
    /// Pakets: "it does not ask for approval before every tool call"); es gibt
    /// also keine Rückfrage abzuschalten (HUM-308). Die Vorlage setzt deshalb
    /// nur Provider, Modell und Telemetrie, und vor allem kein
    /// `defaultProjectTrust`: die Vertrauensfrage zu Erweiterungen eines
    /// Projekts bleibt beim Menschen.
    #[test]
    fn the_settings_switch_nothing_beyond_provider_model_and_telemetry() {
        let doc: Value =
            serde_json::from_str(&render_settings(&["m".to_owned()]).unwrap()).unwrap();
        let mut keys: Vec<&str> = doc
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["defaultModel", "defaultProvider", "enableInstallTelemetry"],
            "{doc}"
        );
    }
}
