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
/// `CONFIG_003` mit dem Bereich und dem Vorgabewert als Vorschlag;
/// `CONFIG_023` für einen Wert, den es früher gab, mit dem Weg zu dem, was
/// ihn ersetzt (HUM-464), wie beim Laden.
pub fn check_value(field: &schema::Field, value: &Value) -> Result<(), Diagnostic> {
    if let Some(retired) = crate::load::retired_value(&field.path, value.as_str(), &field.path) {
        return Err(retired);
    }
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

/// Ob `humanitl config set KEY VALUE` diesen Wert für sich annähme (HUM-507).
///
/// Die Prüfung, die jeder Fix der Art `ChangeSetting` bestehen muss: Der
/// Schlüssel ist ein Blatt des Schemas unter seinem heutigen Namen, der Text
/// liest sich nach dem Typ des Feldes ([`parse_value`]), liegt in Aufzählung
/// und Grenzen ([`check_value`]), und die Vorgabe-Konfiguration mit genau
/// diesem einen Wert besteht die Prüfung, die auch die Gegenprobe von
/// `config set` fährt (etwa `hold.timeout_secs` bis zu einem Tag). Was eine
/// wirkliche Datei sonst noch enthält, kennt der Erzeuger eines Befunds
/// nicht; dafür bleibt die Gegenprobe beim Schreiben.
///
/// Ein Text, der eine Anleitung ist (`remove NAME`), ein leerer Text als
/// Eingabeaufforderung oder ein Schlüssel unter einer freien Tabelle
/// (`sandbox.env.PATH`) besteht sie nicht. Solch ein Rat gehört in den Text
/// des Befunds; ein Feld, dessen Wert nur der Mensch kennt, öffnet
/// `FixAction::EditSetting`.
///
/// # Errors
///
/// `CONFIG_002` für einen Schlüssel, den `config set` nicht kennt oder der
/// eine Gruppe ist, sonst der Befund der Prüfung, die den Wert ablehnt.
pub fn check_setting(key: &str, text: &str) -> Result<(), Diagnostic> {
    let field = schema::field(key)
        .filter(|field| !field.group)
        .ok_or_else(|| {
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::CONFIG_002,
                Severity::Error,
            )
            .why(format!(
                "{key} is not a key humanitl config set writes; humanitl config schema lists \
                     every one"
            ))
            .fix(FixAction::CopyCommand("humanitl config schema".to_owned()))
            .build()
        })?;
    let value = parse_value(field, text)?;
    check_value(field, &value)?;
    let edit = crate::edit::Edit::at(key, to_toml(field, &value)?);
    let table = crate::edit::apply_edits(&toml::Table::new(), &[edit]).ok_or_else(|| {
        Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!("{key} cannot be placed in an empty configuration"))
            .build()
    })?;
    let config: crate::Config = toml::Value::Table(table).try_into().map_err(|error| {
        Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!("{key} = {text} does not load: {error}"))
            .build()
    })?;
    config.validate()
}

/// Ein `ChangeSetting` auf `key`, aber nur mit einem Wert, den
/// [`check_setting`] annimmt; sonst keiner (HUM-507).
///
/// Der Weg für jeden Vorschlag, dessen Wert erst zur Laufzeit feststeht (ein
/// Tippfehler im Schlüssel, die erste Kennung eines Registers, eine bereinigte
/// Adresse): Was `humanitl config set` ablehnte, ist kein Vorschlag, und der
/// Befund behält seinen Rat im Text.
///
/// Eine Firmenvorgabe, die `key` sperrt (HUM-534), prüft das nicht: Der
/// Erzeuger eines Befunds kennt die Sperren meist nicht, und `config set`
/// lehnt einen gesperrten Schlüssel mit eigenem Befund ab, der die Vorgabe
/// nennt. Ein Vorschlag, den die Firma verbietet, führt so nicht ins Leere,
/// sondern zu dem Satz, wer ihn verbietet.
#[must_use]
pub fn change_setting(key: &str, value: impl Into<String>) -> Option<FixAction> {
    let value = value.into();
    check_setting(key, &value).ok()?;
    Some(FixAction::ChangeSetting {
        key: key.to_owned(),
        value,
    })
}

/// Der Vorgabewert eines Feldes als `ChangeSetting`, oder `-` für ein Feld,
/// das leer sein darf und keine Vorgabe hat; sonst keiner (HUM-507).
///
/// Für einen Wert außerhalb seines Bereichs: Die Vorgabe besteht jede
/// Prüfung, und ein Feld ohne sie wird mit `-` wieder leer. Ein Schlüssel,
/// den das Schema nicht als Blatt kennt (ein Eintrag unter
/// `llm.servers.NAME`, eine Gruppe), bekommt keinen Vorschlag, denn
/// `humanitl config set` nähme ihn nicht.
#[must_use]
pub fn reset_fix(key: &str) -> Option<FixAction> {
    let field = schema::field(key).filter(|field| !field.group)?;
    let value = match field.default.as_ref() {
        Some(default) if !default.is_null() => scalar(default),
        _ if field.types.iter().any(|kind| kind == "null") => "-".to_owned(),
        _ => return None,
    };
    change_setting(key, value)
}

/// Eine höhere Grenze für `key`: das Doppelte von `current`, höchstens so
/// viel, wie [`check_setting`] noch annimmt; keiner, wenn `current` schon
/// an der Obergrenze steht (HUM-507).
///
/// Die Obergrenze steht in der Prüfung der Konfiguration (etwa
/// `limits.body_timeout_secs` bis zu einem Tag) und nicht im Schema; statt
/// sie ein zweites Mal aufzuschreiben, sucht die Funktion den größten Wert
/// zwischen `current` und dem Doppelten, den die Prüfung durchlässt. Die
/// Prüfung läuft gegen die Vorgaben der übrigen Werte; was die wirkliche
/// Datei daneben enthält, prüft `config set` beim Schreiben.
#[must_use]
pub fn raised(key: &str, current: u64) -> Option<FixAction> {
    let takes = |value: u64| check_setting(key, &value.to_string()).is_ok();
    // Ein Wert über `i64::MAX` ist keine Zahl, die `config set` liest.
    let ceiling = u64::try_from(i64::MAX).unwrap_or(u64::MAX);
    let wanted = current.saturating_mul(2).min(ceiling);
    if wanted <= current {
        return None;
    }
    let best = if takes(wanted) {
        wanted
    } else {
        // Die Grenze liegt zwischen `current` und `wanted`: Halbieren, bis
        // `low` der größte angenommene Wert ist. Oben begrenzt ist jede
        // Prüfung eines Bereichs, also wächst die Menge der angenommenen
        // Werte von unten nach oben ohne Lücke.
        let (mut low, mut high) = (current, wanted);
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if takes(middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        low
    };
    (best > current && takes(best)).then(|| FixAction::ChangeSetting {
        key: key.to_owned(),
        value: best.to_string(),
    })
}

/// Namen, die nur für Beispiele und Tests reserviert sind (RFC 2606,
/// RFC 6761): die drei Zonen unter `example` und die Endungen, die es im
/// öffentlichen Namensraum nie geben wird.
const PLACEHOLDER_DOMAINS: [&str; 3] = ["example.com", "example.net", "example.org"];

/// Die Endungen dazu; `localhost` fehlt mit Absicht, ein lokales Modell
/// lauscht dort wirklich.
const PLACEHOLDER_TLDS: [&str; 3] = ["example", "test", "invalid"];

/// Der Host einer Adresse, die auf einen Platzhalter zeigt, sonst keiner
/// (HUM-562).
///
/// `https://api.example.com/v1` liest sich wie eine Adresse, und `config set`
/// nimmt sie als solche; erreichen wird sie nie etwas. Abgelehnt wird sie
/// trotzdem nicht: Der Start lehnt sie auch nicht ab, und ein Testaufbau
/// beantwortet genau solche Namen mit `resolver.overrides` (HUM-024). Wer sie
/// setzt, bekommt einen Hinweis und den Wert.
///
/// Geprüft wird nur ein Text, der sich als URL mit Host lesen lässt.
#[must_use]
pub fn placeholder_host(value: &Value) -> Option<String> {
    let url = url::Url::parse(value.as_str()?).ok()?;
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let placeholder = PLACEHOLDER_DOMAINS
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        || PLACEHOLDER_TLDS
            .iter()
            .any(|tld| host == *tld || host.ends_with(&format!(".{tld}")));
    placeholder.then_some(host)
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// HUM-562: Die reservierten Namen werden erkannt, ein echter Name, eine
    /// Adresse, `localhost` und ein Text ohne Host nicht.
    #[test]
    fn a_placeholder_host_is_named_and_a_real_one_is_not() {
        let host = |text: &str| placeholder_host(&Value::String(text.to_owned()));
        for (text, expected) in [
            ("https://api.example.com/v1", "api.example.com"),
            ("https://EXAMPLE.org", "example.org"),
            ("http://llm.test:8080/v1", "llm.test"),
            ("https://proxy.invalid.", "proxy.invalid"),
            ("http://box.example/", "box.example"),
        ] {
            assert_eq!(host(text).as_deref(), Some(expected), "{text}");
        }
        for text in [
            "https://api.openai.com/v1",
            "https://notexample.com/v1",
            "https://example.com.evil.net/",
            "https://contest/",
            "http://localhost:11434/v1",
            "http://127.0.0.1:8080",
            "example.com",
            "/etc/humanitl/test-ca.crt",
        ] {
            assert_eq!(host(text), None, "{text}");
        }
        assert_eq!(placeholder_host(&Value::from(3)), None);
    }

    /// Das Setzen einer Einstellung (`humanitl config set`, `SetConfig`)
    /// lehnt die frühere Laufzeit der Kernel-Stufe wie das Laden mit
    /// `CONFIG_023` und dem Weg zum VM-Backend ab, nicht als unbekannten
    /// Wert mit einem Vorschlag für die Laufzeit des geteilten Kerns
    /// (HUM-464).
    #[test]
    fn setting_the_retired_runtime_is_config_023_with_the_way_to_the_kernel_level() {
        let path = crate::load::ENGINE_RUNTIME;
        let field = schema::field(path).expect("a leaf");
        let [retired] = crate::load::retired_values(path) else {
            panic!("one retired value");
        };
        let value = parse_value(field, retired).expect("a text parses");
        let err = check_value(field, &value).expect_err("retired");
        assert_eq!(err.code.as_str(), "CONFIG_023", "{}", err.why);
        assert!(err.why.contains(path), "{}", err.why);
        assert_eq!(
            err.fix,
            Some(FixAction::ChangeSetting {
                key: "sandbox.backend".to_owned(),
                value: crate::load::KERNEL_LEVEL_BACKEND.to_owned(),
            })
        );
        let kept = field.allowed.as_ref().and_then(|values| values.first());
        let kept = parse_value(field, kept.expect("one allowed value")).expect("parses");
        check_value(field, &kept).expect("the remaining runtime is a value");
    }
}
