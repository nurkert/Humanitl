//! Ein Wert aus Text, gelesen und geprüft nach dem Schema (HUM-070, geteilt in
//! HUM-069).
//!
//! `humanitl config set KEY VALUE` und `SetConfig` des Daemons bekommen beide
//! einen Wert als Text, und beide müssen ihn gleich lesen: dieselben Einheiten
//! (`5m`, `32MiB`), dieselbe Aufzählung, dieselben Grenzen, derselbe Befund.
//! Bis HUM-069 stand das in der Kommandozeile; seit der Daemon dieselben
//! Schlüssel schreibt, steht es hier, damit es genau eine Lesart gibt.

use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::Value;

use crate::schema;

/// Ein Wert als eine Zeile: Text ohne Anführungszeichen, alles andere als JSON.
///
/// `humanitl config get hold.timeout_secs` soll `300` sagen und nicht `"300"`,
/// damit `$(humanitl config get …)` in einem Skript den Wert trägt.
#[must_use]
pub fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Der gelesene Wert als TOML-Wert; `None` heißt: den Schlüssel entfernen.
///
/// TOML kennt kein `null`. Ein Feld, das leer sein darf, ist leer, wenn es in
/// der Datei nicht steht — dann gilt der Vorgabewert.
///
/// # Errors
///
/// `CONFIG_003`, wenn der Wert sich nicht als TOML schreiben lässt.
pub fn to_toml(field: &schema::Field, value: &Value) -> Result<Option<toml::Value>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    toml::Value::try_from(value).map(Some).map_err(|error| {
        let mut builder = Diagnostic::builder(CONFIG_003, Severity::Error).why(format!(
            "{} cannot be written to a TOML file: {error}",
            field.path
        ));
        if let Some(fix) = default_fix(field) {
            builder = builder.fix(fix);
        }
        builder.build()
    })
}

/// Ein Vorschlag, der beweisbar gilt: der Vorgabewert des Feldes.
///
/// Der Vorgabewert besteht jede Prüfung — er ist der Wert, mit dem die
/// Konfiguration ohne Datei lädt —, und damit ist der Befehl einer, den man
/// abtippen kann, ohne dass er wieder abgelehnt wird. Ein Feld ohne
/// Vorgabewert bekommt keinen.
#[must_use]
pub fn default_fix(field: &schema::Field) -> Option<FixAction> {
    let default = field.default.as_ref().filter(|value| !value.is_null())?;
    Some(FixAction::ChangeSetting {
        key: field.path.clone(),
        value: scalar(default),
    })
}

/// Liest den Wert der Kommandozeile nach dem Typ des Feldes.
///
/// Eine Zahl darf ihre Einheit tragen: `5m` für ein Feld auf `_secs`, `32MiB`
/// für eines auf `_bytes`. Beides ist die Schreibweise, in der ein Mensch über
/// diese Werte spricht, und beides steht am Ende als Zahl in der Datei — das
/// Schema kennt keine Einheiten.
///
/// Eine Aufzählung ist im Schema ein `oneOf` aus Konstanten und hat damit
/// keinen Typ `string`; sie wird als Text gelesen, und ob der Text einer der
/// erlaubten Werte ist, prüft [`check_value`].
///
/// # Errors
///
/// `CONFIG_003`, wenn der Text kein Wert des Typs ist.
pub fn parse_value(field: &schema::Field, text: &str) -> Result<Value, Diagnostic> {
    let types: Vec<&str> = field.types.iter().map(String::as_str).collect();
    let has = |kind: &str| types.contains(&kind);

    if has("null") && (text == "null" || text == "-") {
        return Ok(Value::Null);
    }
    if field.allowed.is_some() {
        return Ok(Value::String(text.to_owned()));
    }
    if has("boolean")
        && let Ok(flag) = text.parse::<bool>()
    {
        return Ok(Value::Bool(flag));
    }
    if has("integer") {
        return integer(field, text).map(Value::from);
    }
    if has("number")
        && let Ok(number) = text.parse::<f64>()
    {
        return serde_json::Number::from_f64(number)
            .map(Value::Number)
            .ok_or_else(|| not_a(field, text, "a finite number"));
    }
    if has("array") || has("object") {
        return serde_json::from_str(text).map_err(|error| {
            not_a(
                field,
                text,
                &format!("{} as JSON ({error})", field.type_label.trim()),
            )
        });
    }
    if has("string") {
        return Ok(Value::String(text.to_owned()));
    }
    Err(not_a(field, text, field.type_label.trim()))
}

/// Eine ganze Zahl, mit den Einheiten, die zum Namen des Feldes passen.
fn integer(field: &schema::Field, text: &str) -> Result<i64, Diagnostic> {
    if let Ok(number) = text.parse::<i64>() {
        return Ok(number);
    }
    if field.path.ends_with("_secs")
        && let Some(seconds) = duration_secs(text)
    {
        return Ok(seconds);
    }
    if field.path.ends_with("_ms")
        && let Some(seconds) = duration_secs(text)
    {
        return seconds
            .checked_mul(1000)
            .ok_or_else(|| not_a(field, text, "a number of milliseconds that fits"));
    }
    if field.path.ends_with("_bytes")
        && let Some(bytes) = size_bytes(text)
    {
        return Ok(bytes);
    }
    Err(not_a(field, text, unit_hint(&field.path)))
}

/// Was ein Feld an Schreibweisen annimmt, für den Befund.
const fn unit_hint(path: &str) -> &'static str {
    if ends_with_ascii(path, "_secs") || ends_with_ascii(path, "_ms") {
        "a whole number or a span like 30s, 5m, 2h, 1d"
    } else if ends_with_ascii(path, "_bytes") {
        "a whole number or a size like 512KiB, 32MiB, 1GiB"
    } else {
        "a whole number"
    }
}

/// `str::ends_with` für einen `const fn`: Der Vergleich läuft über die Bytes.
const fn ends_with_ascii(text: &str, suffix: &str) -> bool {
    let (text, suffix) = (text.as_bytes(), suffix.as_bytes());
    if text.len() < suffix.len() {
        return false;
    }
    let offset = text.len() - suffix.len();
    let mut index = 0;
    while index < suffix.len() {
        if text[offset + index] != suffix[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// `30s`, `5m`, `2h`, `1d` als Sekunden.
fn duration_secs(text: &str) -> Option<i64> {
    let (digits, factor) = split_suffix(text, &[("s", 1), ("m", 60), ("h", 3600), ("d", 86_400)])?;
    digits.parse::<i64>().ok()?.checked_mul(factor)
}

/// `512KiB`, `32MiB`, `1GiB` und die Formen ohne `i` als Bytes.
fn size_bytes(text: &str) -> Option<i64> {
    let (digits, factor) = split_suffix(
        text,
        &[
            ("B", 1),
            ("KiB", 1024),
            ("MiB", 1024 * 1024),
            ("GiB", 1024 * 1024 * 1024),
            ("KB", 1000),
            ("MB", 1_000_000),
            ("GB", 1_000_000_000),
            ("K", 1024),
            ("M", 1024 * 1024),
            ("G", 1024 * 1024 * 1024),
        ],
    )?;
    digits.parse::<i64>().ok()?.checked_mul(factor)
}

/// Trennt Zahl und Einheit. Die längste passende Einheit gewinnt, damit `KiB`
/// nicht als `B` gelesen wird.
fn split_suffix<'a>(text: &'a str, units: &[(&'static str, i64)]) -> Option<(&'a str, i64)> {
    let trimmed = text.trim();
    let mut best: Option<(&'static str, i64)> = None;
    for (suffix, factor) in units {
        if trimmed.len() > suffix.len()
            && trimmed
                .get(trimmed.len() - suffix.len()..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
            && best.is_none_or(|(known, _)| known.len() < suffix.len())
        {
            best = Some((suffix, *factor));
        }
    }
    let (suffix, factor) = best?;
    Some((trimmed.get(..trimmed.len() - suffix.len())?.trim(), factor))
}

/// Prüft den gelesenen Wert gegen Aufzählung, Mindest- und Höchstwert.
///
/// Die Wertebereiche, die nur in der Prüfung von `humanitl-config` stehen
/// (`hold.timeout_secs` zwischen 1 und einem Tag), fängt erst die Gegenprobe des Aufrufers (in `humanitl config set` die
/// Probe, in `SetConfig` die Ladung der Nebendatei);
/// hier steht nur, was das Schema selbst sagt.
///
/// # Errors
///
/// `CONFIG_003` mit dem Bereich und dem Vorgabewert als Vorschlag.
pub fn check_value(field: &schema::Field, value: &Value) -> Result<(), Diagnostic> {
    // Eine Adresse mit Anführungszeichen oder Leerraum nähme der Parser hin
    // und machte daraus einen anderen Pfad (HUM-283); geprüft wird der Text.
    if field.path == crate::llm_endpoint::SETTING
        && let Some(text) = value.as_str()
    {
        crate::llm_endpoint::check_text(text)?;
    }
    if let (Some(allowed), Some(text)) = (field.allowed.as_ref(), value.as_str())
        && !allowed.iter().any(|known| known == text)
    {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "{text} is not a value of {}; it takes one of {}",
                field.path,
                allowed.join(", ")
            ))
            .fix(FixAction::ChangeSetting {
                key: field.path.clone(),
                value: allowed.first().cloned().unwrap_or_default(),
            })
            .build());
    }
    let Some(number) = value.as_i64() else {
        return Ok(());
    };
    if let Some(minimum) = field.minimum
        && number < minimum
    {
        return Err(out_of_range(
            field,
            &format!("{number} is below the minimum of {minimum}"),
        ));
    }
    if let Some(maximum) = field.maximum
        && number > maximum
    {
        return Err(out_of_range(
            field,
            &format!("{number} is above the maximum of {maximum}"),
        ));
    }
    Ok(())
}

/// `CONFIG_003`: Der Wert liegt außerhalb des Bereichs.
///
/// Der Vorschlag ist der Vorgabewert und nicht die Grenze: Die Grenze des
/// Schemas ist oft nicht die der Prüfung (`0` passt zum Typ, nicht zur
/// Haltefrist), und ein Vorschlag, der wieder abgelehnt wird, ist keiner.
fn out_of_range(field: &schema::Field, why: &str) -> Diagnostic {
    let mut builder =
        Diagnostic::builder(CONFIG_003, Severity::Error).why(format!("{}: {why}", field.path));
    if let Some(fix) = default_fix(field) {
        builder = builder.fix(fix);
    }
    builder.build()
}

/// `CONFIG_003`: Der Text ist kein Wert dieses Typs.
fn not_a(field: &schema::Field, text: &str, expected: &str) -> Diagnostic {
    let fix = default_fix(field)
        .unwrap_or_else(|| FixAction::CopyCommand("humanitl config schema".to_owned()));
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{} takes {expected}, and {text} is none",
            field.path
        ))
        .fix(fix)
        .build()
}
