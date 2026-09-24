//! Ob `data` einer Zeile ein JSON-Objekt in kanonischer Form ist, ohne es zu
//! zerlegen (HUM-163).
//!
//! Die schnelle Seite ([`super::quick`]) darf eine Zeile nur dann als Record
//! zählen, wenn `AuditRecord::from_line` sie auch annähme. Diese Prüfung nimmt
//! deshalb nur eine Teilmenge dessen an, was `from_line` als `data` liest,
//! nämlich ein Objekt in der Schreibweise von `crate::canonical`: kein
//! Leerraum, Strings in UTF-8 mit den Escapes `\"`, `\\` und `\u00xx`,
//! Ganzzahlen mit höchstens 20 Ziffern, `true`, `false`, `null`, Arrays und
//! Objekte bis zur Tiefe [`MAX_DEPTH`]. Die Reihenfolge der Schlüssel prüft sie
//! nicht: Sie berührt weder den Filter noch die Ausgabe, und `from_line` nimmt
//! jede Reihenfolge an. Alles andere ist hier kein Objekt, und die Seite liest
//! dann jede Zeile als JSON. Strenger zu sein als `serde_json` kostet also nur Zeit,
//! nie Genauigkeit; lockerer darf die Prüfung nirgends sein.
//!
//! Die Grenzen im Einzelnen, jeweils gegen `serde_json` gehalten:
//! - `\uXXXX` nur als `\u00xx`: Ein einzelnes Surrogat liest `serde_json` nicht
//!   als String, und `\u00xx` ist nie eines.
//! - Höchstens 20 Ziffern: `serde_json` liest jede solche Ganzzahl, eine zu
//!   große als Float; ohne Grenze gäbe es Zahlen jenseits des Bereichs von
//!   `f64`, die es ablehnt.
//! - Tiefe höchstens [`MAX_DEPTH`]: `serde_json` bricht bei 128 ab, und die
//!   Zeile selbst ist schon eine Ebene.

/// So tief darf `data` verschachtelt sein; `data` selbst ist Tiefe 1.
pub(super) const MAX_DEPTH: u32 = 100;

/// Höchstens so viele Ziffern hat eine Ganzzahl.
const MAX_DIGITS: usize = 20;

/// Ob `bytes` genau ein JSON-Objekt in kanonischer Form ist, von `{` bis `}`.
pub(super) fn is_canonical_object(bytes: &[u8]) -> bool {
    if bytes.first() != Some(&b'{') || std::str::from_utf8(bytes).is_err() {
        return false;
    }
    let mut reader = Reader { bytes, at: 0 };
    reader.value(1).is_some() && reader.at == bytes.len()
}

/// Liest `bytes` von vorn; `at` ist die nächste ungelesene Stelle.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// Verbraucht `byte`, wenn es als Nächstes steht.
    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        if found {
            self.at += 1;
        }
        found
    }

    /// Ein Wert auf Tiefe `depth`; Objekte und Arrays zählen eine Ebene.
    fn value(&mut self, depth: u32) -> Option<()> {
        match self.peek()? {
            b'{' => self.container(depth, b'}', true),
            b'[' => self.container(depth, b']', false),
            b'"' => self.string(),
            b't' => self.literal(b"true"),
            b'f' => self.literal(b"false"),
            b'n' => self.literal(b"null"),
            _ => self.integer(),
        }
    }

    /// Ein Objekt oder Array; `keyed` heißt Objekt.
    fn container(&mut self, depth: u32, close: u8, keyed: bool) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.at += 1;
        if self.eat(close) {
            return Some(());
        }
        loop {
            if keyed {
                if self.peek()? != b'"' {
                    return None;
                }
                self.string()?;
                self.eat(b':').then_some(())?;
            }
            self.value(depth + 1)?;
            if self.eat(close) {
                return Some(());
            }
            self.eat(b',').then_some(())?;
        }
    }

    /// Ein String ab seinem `"` bis hinter das schließende. UTF-8 ist schon
    /// für das ganze Objekt geprüft.
    fn string(&mut self) -> Option<()> {
        self.at += 1;
        loop {
            // Gewöhnliche Zeichen in einem Zug überspringen: Strings sind der
            // größte Teil von `data`.
            let plain = self
                .bytes
                .get(self.at..)?
                .iter()
                .position(|&byte| byte == b'"' || byte == b'\\' || byte < 0x20)?;
            self.at += plain;
            match self.peek()? {
                b'"' => {
                    self.at += 1;
                    return Some(());
                }
                b'\\' => match self.bytes.get(self.at + 1..)? {
                    [b'"' | b'\\', ..] => self.at += 2,
                    [b'u', b'0', b'0', high, low, ..]
                        if high.is_ascii_hexdigit() && low.is_ascii_hexdigit() =>
                    {
                        self.at += 6;
                    }
                    _ => return None,
                },
                byte if byte < 0x20 => return None,
                _ => self.at += 1,
            }
        }
    }

    fn literal(&mut self, word: &[u8]) -> Option<()> {
        self.bytes.get(self.at..)?.starts_with(word).then_some(())?;
        self.at += word.len();
        Some(())
    }

    /// Eine Ganzzahl wie JSON sie schreibt: `-` erlaubt, keine führende Null.
    fn integer(&mut self) -> Option<()> {
        self.eat(b'-');
        let rest = self.bytes.get(self.at..)?;
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let leading_zero = digits > 1 && rest.first() == Some(&b'0');
        if digits == 0 || digits > MAX_DIGITS || leading_zero {
            return None;
        }
        self.at += digits;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::Value;

    use super::{MAX_DEPTH, is_canonical_object};

    /// Was hier als Objekt gilt, liest `serde_json` auch als Objekt; was es
    /// nicht liest, gilt hier nie als Objekt.
    #[test]
    fn only_what_serde_json_reads_as_an_object_passes() {
        let deep = |depth: u32| {
            let depth = usize::try_from(depth).unwrap();
            format!(
                "{}{}",
                r#"{"a":"#.repeat(depth),
                "1".to_owned() + &"}".repeat(depth)
            )
        };
        let accepted = [
            "{}".to_owned(),
            r#"{"a":[1,-2,true,false,null,"x\"\\\u001f",{}],"b":{"c":[]}}"#.to_owned(),
            r#"{"n":18446744073709551615,"m":-9223372036854775808}"#.to_owned(),
            r#"{"ä":"ü"}"#.to_owned(),
            deep(MAX_DEPTH),
        ];
        for text in &accepted {
            assert!(is_canonical_object(text.as_bytes()), "{text}");
            let value: Value = serde_json::from_slice(text.as_bytes()).unwrap();
            assert!(value.is_object(), "{text}");
        }
        let refused = [
            r#"{"n":tru}"#.to_owned(),
            r#"{},"x":{}"#.to_owned(),
            r#"{"a":1,}"#.to_owned(),
            r#"{"a" :1}"#.to_owned(),
            r#"{"a":01}"#.to_owned(),
            r#"{"a":1.5}"#.to_owned(),
            r#"{"a":123456789012345678901}"#.to_owned(),
            r#"{"a":"\ud800"}"#.to_owned(),
            r#"{"a":"\n"}"#.to_owned(),
            "{\"a\":\"\u{1}\"}".to_owned(),
            "{1:2}".to_owned(),
            "[]".to_owned(),
            r#"{"a":1"#.to_owned(),
            deep(MAX_DEPTH + 1),
        ];
        for text in &refused {
            assert!(!is_canonical_object(text.as_bytes()), "{text}");
        }
        assert!(!is_canonical_object(b"{\"a\":\"\xff\"}"));
    }
}
