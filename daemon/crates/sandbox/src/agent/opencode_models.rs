//! Die mitgelieferten Vorlagen des `OpenCode`-Adapters.
//!
//! Drei Dateien aus `agents/opencode/` beziehungsweise `rules/` sind
//! einkompiliert. Zur Laufzeit wird nichts davon vom Dateisystem gelesen: was
//! in der Sandbox landet, steht im Binary, und niemand kann es zwischen Bau und
//! Start austauschen.
//!
//! - [`CONFIG_TEMPLATE`] wird zu `opencode.json`, der Konfiguration des Agenten.
//! - [`MODELS_TEMPLATE`] wird zu `models.json` und ersetzt den Modellkatalog,
//!   den `OpenCode` sonst aus dem Netz holt (`agents/opencode/README.md`).
//! - [`DEFAULT_RULES`] ist der mitgelieferte Regelsatz (HUM-038).
//!
//! Beide JSON-Vorlagen tragen ihre Platzhalter als Werte, nicht als
//! Textfragmente, und werden geparst statt ersetzt. `llm.endpoint` und die
//! Modellnamen kommen aus der Konfiguration; ein Modellname mit einem
//! Anführungszeichen darf keine fremde Struktur in die Datei schreiben können
//! (HUM-037, Fallstrick 5).

use humanitl_core::diagnostics::codes::AGENT_003;
use humanitl_core::{Diagnostic, Severity};
use serde_json::{Map, Value};

/// Die Vorlage für `opencode.json`.
pub const CONFIG_TEMPLATE: &str = include_str!("../../../../../agents/opencode/opencode.json.tmpl");

/// Die Vorlage für den Modellkatalog `models.json`.
pub const MODELS_TEMPLATE: &str = include_str!("../../../../../agents/opencode/models.json");

/// Der mitgelieferte Regelsatz im Format von `rules.yaml` (HUM-038).
pub const DEFAULT_RULES: &str = include_str!("../../../../../rules/default.yaml");

/// Die Kennung des Providers, den Humanitl in beide Dateien schreibt.
pub const PROVIDER_ID: &str = "humanitl-local";

/// Das Modell, das eingetragen wird, wenn `llm.models` leer ist.
///
/// Ob der LLM-Server ein Modell dieses Namens kennt, weiß Humanitl nicht; der
/// Adapter meldet deshalb `LLM_004` als Warnung.
pub const PLACEHOLDER_MODEL: &str = "default";

/// Der Schlüssel, unter dem in [`MODELS_TEMPLATE`] der Musterdatensatz eines
/// Modells steht.
const MODEL_PLACEHOLDER_KEY: &str = "{{MODEL_ID}}";

/// Baut einen Befund über eine unbrauchbare Vorlage.
///
/// Das ist ein Fehler im Build, keine Nutzereingabe: die Dateien liegen unter
/// `agents/` und werden einkompiliert. Ein `unwrap` wäre hier trotzdem falsch,
/// weil dieselbe Meldung dann als Panik statt als Befund erschiene.
pub(crate) fn broken(file: &str, why: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_003, Severity::Blocking)
        .why(format!(
            "the bundled template {file} is not usable: {why}; \
             this is a defect in the build, not in the configuration"
        ))
        .build()
}

/// Liest eine Vorlage als JSON-Objekt.
fn parse_object(file: &str, text: &str) -> Result<Map<String, Value>, Diagnostic> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| broken(file, &format!("invalid JSON ({err})")))?;
    match value {
        Value::Object(map) => Ok(map),
        other => Err(broken(
            file,
            &format!("the top level is {}, not an object", kind_of(&other)),
        )),
    }
}

/// Der Name eines JSON-Typs, für die Fehlermeldung.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Holt ein Unterobjekt heraus, mit einer Meldung statt eines Index-Zugriffs.
fn object_mut<'a>(
    file: &str,
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, Diagnostic> {
    parent
        .get_mut(key)
        .ok_or_else(|| broken(file, &format!("the key {key:?} is missing")))?
        .as_object_mut()
        .ok_or_else(|| broken(file, &format!("the key {key:?} is not an object")))
}

/// Die Modelle, die in eine der beiden Dateien geschrieben werden.
///
/// Leer heißt: das Platzhalter-Modell. Der Adapter meldet den Fall gesondert,
/// damit der Mensch weiß, dass hier geraten wurde.
#[must_use]
pub fn effective_models(models: &[String]) -> Vec<String> {
    let named: Vec<String> = models
        .iter()
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if named.is_empty() {
        vec![PLACEHOLDER_MODEL.to_owned()]
    } else {
        named
    }
}

/// Rendert `opencode.json` für einen Endpunkt und eine Modell-Liste.
///
/// Gesetzt werden genau drei Stellen: die Basis-Adresse des Providers, seine
/// Modelle und das voreingestellte Modell. Alles andere bleibt so, wie es in
/// `agents/opencode/opencode.json.tmpl` steht und dort nachzulesen ist.
///
/// # Errors
///
/// Ein [`Diagnostic`] mit `AGENT_003`, wenn die Vorlage nicht die erwartete
/// Form hat.
pub fn render_config(base_url: &str, models: &[String]) -> Result<String, Diagnostic> {
    const FILE: &str = "agents/opencode/opencode.json.tmpl";

    let models = effective_models(models);
    let mut doc = parse_object(FILE, CONFIG_TEMPLATE)?;

    {
        let providers = object_mut(FILE, &mut doc, "provider")?;
        let provider = object_mut(FILE, providers, PROVIDER_ID)?;
        let options = object_mut(FILE, provider, "options")?;
        options.insert("baseURL".to_owned(), Value::String(base_url.to_owned()));

        let mut entries = Map::new();
        for model in &models {
            let mut entry = Map::new();
            entry.insert("name".to_owned(), Value::String(model.clone()));
            entries.insert(model.clone(), Value::Object(entry));
        }
        provider.insert("models".to_owned(), Value::Object(entries));
    }

    let default_model = models
        .first()
        .ok_or_else(|| broken(FILE, "the model list came out empty"))?;
    doc.insert(
        "model".to_owned(),
        Value::String(format!("{PROVIDER_ID}/{default_model}")),
    );

    serde_json::to_string_pretty(&Value::Object(doc))
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|err| broken(FILE, &format!("could not be written back ({err})")))
}

/// Die Berechtigungen aus der Vorlage, als kompaktes JSON.
///
/// Denselben Block trägt `opencode.json`; hier kommt er noch einmal für die
/// Umgebungsvariable `OPENCODE_PERMISSION`, die `OpenCode` als Letztes über
/// alles andere mergt. Quelle ist die Vorlage, damit beide Wege nicht
/// auseinanderlaufen können.
///
/// # Errors
///
/// Ein [`Diagnostic`] mit `AGENT_003`, wenn die Vorlage keinen Block
/// `permission` trägt.
pub fn permission_json() -> Result<String, Diagnostic> {
    const FILE: &str = "agents/opencode/opencode.json.tmpl";

    let mut doc = parse_object(FILE, CONFIG_TEMPLATE)?;
    let permission = object_mut(FILE, &mut doc, "permission")?.clone();
    serde_json::to_string(&Value::Object(permission))
        .map_err(|err| broken(FILE, &format!("permission could not be written ({err})")))
}

/// Rendert den Modellkatalog `models.json` für eine Modell-Liste.
///
/// Der Eintrag unter `{{MODEL_ID}}` ist die Vorlage eines Modelleintrags; sie
/// wird einmal je Modell angelegt, mit gesetztem `id` und `name`. Alle übrigen
/// Felder — Kontextgrenze, Modalitäten, das Platzhalter-Datum — stehen in
/// `agents/opencode/models.json` und sind dort begründet.
///
/// # Errors
///
/// Ein [`Diagnostic`] mit `AGENT_003`, wenn die Vorlage nicht die erwartete
/// Form hat.
pub fn render_models(models: &[String]) -> Result<String, Diagnostic> {
    const FILE: &str = "agents/opencode/models.json";

    let models = effective_models(models);
    let mut doc = parse_object(FILE, MODELS_TEMPLATE)?;

    {
        let provider = object_mut(FILE, &mut doc, PROVIDER_ID)?;
        let template = object_mut(FILE, provider, "models")?
            .get(MODEL_PLACEHOLDER_KEY)
            .ok_or_else(|| {
                broken(
                    FILE,
                    &format!("the model template {MODEL_PLACEHOLDER_KEY:?} is missing"),
                )
            })?
            .as_object()
            .ok_or_else(|| {
                broken(
                    FILE,
                    &format!("the model template {MODEL_PLACEHOLDER_KEY:?} is not an object"),
                )
            })?
            .clone();

        let mut entries = Map::new();
        for model in &models {
            let mut entry = template.clone();
            entry.insert("id".to_owned(), Value::String(model.clone()));
            entry.insert("name".to_owned(), Value::String(model.clone()));
            entries.insert(model.clone(), Value::Object(entry));
        }
        provider.insert("models".to_owned(), Value::Object(entries));
    }

    serde_json::to_string_pretty(&Value::Object(doc))
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|err| broken(FILE, &format!("could not be written back ({err})")))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::Value;

    use super::{PLACEHOLDER_MODEL, PROVIDER_ID, effective_models, render_config, render_models};

    #[test]
    fn empty_model_list_becomes_the_placeholder() {
        assert_eq!(effective_models(&[]), vec![PLACEHOLDER_MODEL.to_owned()]);
        assert_eq!(
            effective_models(&["  ".to_owned()]),
            vec![PLACEHOLDER_MODEL.to_owned()]
        );
    }

    #[test]
    fn a_model_name_with_a_quote_stays_inside_its_string() {
        let evil = "\", \"injected\": {\"name\": \"x\"}, \"a\": \"".to_owned();
        let text = render_config("http://x:1/v1", std::slice::from_ref(&evil)).unwrap();
        let doc: Value = serde_json::from_str(&text).unwrap();
        let models = &doc["provider"][PROVIDER_ID]["models"];
        assert_eq!(models.as_object().unwrap().len(), 1);
        assert_eq!(models[&evil]["name"], Value::String(evil.clone()));
        assert!(doc.get("injected").is_none());
    }

    #[test]
    fn the_catalog_keeps_the_required_fields_of_the_template() {
        let text = render_models(&["qwen3".to_owned()]).unwrap();
        let doc: Value = serde_json::from_str(&text).unwrap();
        let model = &doc[PROVIDER_ID]["models"]["qwen3"];
        for field in [
            "id",
            "name",
            "release_date",
            "attachment",
            "reasoning",
            "temperature",
            "tool_call",
            "limit",
        ] {
            assert!(model.get(field).is_some(), "{field} is missing");
        }
        assert_eq!(model["id"], Value::String("qwen3".to_owned()));
        assert!(doc[PROVIDER_ID]["models"].get("{{MODEL_ID}}").is_none());
    }
}
