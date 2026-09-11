//! Die kanonische Serialisierung des Audit-Logs (HUM-050).
//!
//! Eine Zeile in `audit.jsonl` ist nur dann prüfbar, wenn jeder, der sie liest,
//! aus demselben Wert dieselben Bytes erzeugt. Dieses Modul legt diese Bytes
//! fest, und zwar strenger als RFC 8785:
//!
//! - Objekte: Schlüssel bytewise aufsteigend sortiert, nicht nach Locale.
//! - Kein Leerraum außerhalb von Strings.
//! - Zahlen: nur Ganzzahlen (`i64`/`u64`). Ein Float ist ein Programmierfehler
//!   und endet in [`CanonicalError::Float`]. Dauern stehen als ganze
//!   Millisekunden im Log, Größen als Bytes.
//! - Strings: escaped werden nur `"`, `\` und Steuerzeichen unter `0x20`
//!   (als `\u00xx`); alles andere steht als UTF-8 da, auch `/` und
//!   Nicht-ASCII.
//! - `true`, `false` und `null` wie in JSON.
//!
//! Die Sortierung geschieht hier, in einer eigenen rekursiven Funktion. Auf
//! die Reihenfolge der `serde_json::Map` verlässt sich nichts: Das Feature
//! `preserve_order` kann jede andere Crate im Workspace einschalten, und dann
//! wäre die Reihenfolge der Map die des Einfügens.

use std::collections::BTreeMap;

use serde_json::Value;

/// Ein Wert, der sich nicht kanonisch schreiben lässt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CanonicalError {
    /// Eine Zahl ist keine Ganzzahl im Bereich von `i64` oder `u64`.
    ///
    /// `1.0` gehört dazu: `serde_json` hält sie als Float, auch wenn sie
    /// ganzzahlig aussieht, und die Prüfung fragt nach der Art der Zahl, nicht
    /// nach ihrem Bruchteil.
    #[error("a number in the audit data is not an integer")]
    Float,
}

/// Die kanonische Form von `value`.
///
/// # Errors
///
/// [`CanonicalError::Float`], sobald irgendwo im Wert eine Zahl steht, die
/// keine Ganzzahl ist.
pub fn canonical_json(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    let mut out = Vec::with_capacity(256);
    write_value(&mut out, value)?;
    Ok(out)
}

fn write_value(out: &mut Vec<u8>, value: &Value) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Number(number) => {
            // `as_u64` vor `as_i64`: Eine positive Zahl passt in beide, und die
            // Dezimalform ist dieselbe. Ein Float passt in keine von beiden.
            if let Some(unsigned) = number.as_u64() {
                out.extend_from_slice(unsigned.to_string().as_bytes());
            } else if let Some(signed) = number.as_i64() {
                out.extend_from_slice(signed.to_string().as_bytes());
            } else {
                return Err(CanonicalError::Float);
            }
        }
        Value::String(text) => write_string(out, text),
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_value(out, item)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            // `BTreeMap<&str, _>` sortiert nach `str::cmp`, und das vergleicht
            // Bytes: `B` (0x42) steht vor `a` (0x61), unabhängig von jeder
            // Locale. Doppelte Schlüssel kann eine `Map` nicht halten.
            let sorted: BTreeMap<&str, &Value> =
                map.iter().map(|(key, item)| (key.as_str(), item)).collect();
            out.push(b'{');
            for (index, (key, item)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_string(out, key);
                out.push(b':');
                write_value(out, item)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

/// Hex-Ziffern für `\u00xx`, klein wie die Hashes im Log.
const HEX: &[u8; 16] = b"0123456789abcdef";

fn write_string(out: &mut Vec<u8>, text: &str) {
    out.push(b'"');
    // Byteweise ist hier sicher: Jedes Byte einer UTF-8-Folge jenseits von
    // ASCII ist 0x80 oder größer und fällt damit durch alle drei Fälle.
    for &byte in text.as_bytes() {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x00..=0x1f => {
                out.extend_from_slice(b"\\u00");
                out.push(HEX[usize::from(byte >> 4)]);
                out.push(HEX[usize::from(byte & 0x0f)]);
            }
            _ => out.push(byte),
        }
    }
    out.push(b'"');
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::{Value, json};

    use super::{CanonicalError, canonical_json};

    fn text(value: &Value) -> String {
        String::from_utf8(canonical_json(value).unwrap()).unwrap()
    }

    #[test]
    fn sorts_keys_bytewise() {
        let value: Value = serde_json::from_str(r#"{"b":1,"a":2,"B":3}"#).unwrap();
        assert_eq!(text(&value), r#"{"B":3,"a":2,"b":1}"#);
    }

    #[test]
    fn sorting_does_not_follow_a_locale() {
        // Nach einer deutschen Locale stünde `ä` zwischen `a` und `b`; bytewise
        // steht es (0xc3 0xa4) hinter `z`.
        let value = json!({"z": 1, "ä": 2, "a": 3});
        assert_eq!(text(&value), r#"{"a":3,"z":1,"ä":2}"#);
    }

    #[test]
    fn no_whitespace() {
        let value: Value =
            serde_json::from_str("{ \"a\" : [ 1 , 2 ] ,\n \"b\" : { \"c\" : null } }").unwrap();
        let out = text(&value);
        assert_eq!(out, r#"{"a":[1,2],"b":{"c":null}}"#);
        assert!(!out.contains([' ', '\n', '\t', '\r']));
    }

    #[test]
    fn utf8_unescaped() {
        assert_eq!(text(&json!("ü")), "\"ü\"");
        assert_eq!(text(&json!("a/b")), "\"a/b\"", "no escaping of `/`");
        assert_eq!(
            text(&json!("\u{7f}")),
            "\"\u{7f}\"",
            "DEL is not below 0x20"
        );
    }

    #[test]
    fn control_chars_escaped() {
        assert_eq!(text(&json!("\u{1}")), "\"\\u0001\"");
        assert_eq!(text(&json!("a\nb\tc")), "\"a\\u000ab\\u0009c\"");
        assert_eq!(text(&json!("\u{1f}")), "\"\\u001f\"");
        assert_eq!(text(&json!("\u{0}")), "\"\\u0000\"");
        assert_eq!(
            text(&json!("say \"hi\" \\ bye")),
            "\"say \\\"hi\\\" \\\\ bye\""
        );
    }

    #[test]
    fn float_rejected() {
        assert_eq!(canonical_json(&json!(1.5)), Err(CanonicalError::Float));
        // Ganzzahlig aussehend, aber ein Float: abgelehnt, weil gefragt wird,
        // was die Zahl ist, nicht wie groß ihr Bruchteil ist.
        let one: Value = serde_json::from_str("1.0").unwrap();
        assert_eq!(canonical_json(&one), Err(CanonicalError::Float));
        // Auch tief drinnen.
        assert_eq!(
            canonical_json(&json!({"a": [{"b": 0.1}]})),
            Err(CanonicalError::Float)
        );
    }

    #[test]
    fn integers_keep_their_full_range() {
        assert_eq!(text(&json!(u64::MAX)), u64::MAX.to_string());
        assert_eq!(text(&json!(i64::MIN)), i64::MIN.to_string());
        assert_eq!(text(&json!(0)), "0");
    }

    #[test]
    fn nested_objects_sorted() {
        let value = json!({"outer": {"z": {"y": 1, "x": 2}, "a": true}, "a": [ {"d": 1, "c": 2} ]});
        assert_eq!(
            text(&value),
            r#"{"a":[{"c":2,"d":1}],"outer":{"a":true,"z":{"x":2,"y":1}}}"#
        );
    }
}
