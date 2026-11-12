//! The two JSON shapes the launcher writes, read without serde.
//!
//! The shim has no dependency but `libc` and `seccompiler`, so it reads JSON
//! with this small recursive-descent reader: strings with the usual escapes,
//! whitespace, and the punctuation of arrays and objects. It knows no numbers,
//! no `true`, `false` or `null`, and no nesting beyond what its callers walk
//! themselves. Two callers use it: the bridge list (`HUMANITL_BRIDGES`,
//! `bridge.rs`) and the environment of the agent behind `--env-fd`
//! ([`string_map`], HUM-430). Every deviation is an error: the launcher wrote
//! the text, so a surprise means a bug, and a bug is not something to guess
//! around.

use std::fmt;

/// Where the text stopped being the expected shape, and what was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    /// Byte offset in the text.
    pub offset: usize,
    /// What was expected there.
    pub what: &'static str,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.what, self.offset)
    }
}

/// The reader: a text and a position in it.
pub struct Parser<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    /// A reader at the start of `text`.
    pub const fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    /// An error at the current position.
    pub const fn error(&self, what: &'static str) -> Error {
        Error {
            offset: self.pos,
            what,
        }
    }

    /// The next character, without taking it.
    pub fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    /// Takes the next character.
    pub fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Skips the four whitespace characters JSON knows.
    pub fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    /// Takes `want`, or fails with `what`.
    ///
    /// # Errors
    ///
    /// [`Error`] when the next character is not `want`.
    pub fn expect(&mut self, want: char, what: &'static str) -> Result<(), Error> {
        if self.peek() == Some(want) {
            self.pos += want.len_utf8();
            Ok(())
        } else {
            Err(self.error(what))
        }
    }

    /// Nothing but whitespace is left.
    ///
    /// # Errors
    ///
    /// [`Error`] "trailing data" otherwise.
    pub fn end(&mut self) -> Result<(), Error> {
        self.skip_ws();
        if self.pos == self.text.len() {
            Ok(())
        } else {
            Err(self.error("trailing data"))
        }
    }

    /// One string with its escapes resolved.
    ///
    /// # Errors
    ///
    /// [`Error`] for a missing quote, an unterminated string, a raw control
    /// character, a bad escape or a broken surrogate pair.
    pub fn string(&mut self) -> Result<String, Error> {
        self.expect('"', "expected a string")?;
        let mut out = String::new();
        loop {
            match self.next() {
                None => return Err(self.error("unterminated string")),
                Some('"') => return Ok(out),
                Some('\\') => out.push(self.escape()?),
                // JSON forbids only U+0000 to U+001F unescaped. DEL and the C1
                // controls are ordinary characters there, and serde_json on
                // the launcher's side writes them raw; refusing them would
                // refuse an environment the launcher wrote correctly.
                Some(c) if c < '\u{20}' => return Err(self.error("control character in string")),
                Some(c) => out.push(c),
            }
        }
    }

    fn escape(&mut self) -> Result<char, Error> {
        Ok(match self.next() {
            Some('"') => '"',
            Some('\\') => '\\',
            Some('/') => '/',
            Some('b') => '\u{8}',
            Some('f') => '\u{c}',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('u') => {
                let unit = self.hex4()?;
                if (0xD800..0xDC00).contains(&unit) {
                    self.expect('\\', "expected the low surrogate")?;
                    self.expect('u', "expected the low surrogate")?;
                    let low = self.hex4()?;
                    if !(0xDC00..0xE000).contains(&low) {
                        return Err(self.error("bad low surrogate"));
                    }
                    let code = 0x1_0000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                    char::from_u32(code).ok_or_else(|| self.error("bad surrogate pair"))?
                } else {
                    char::from_u32(unit).ok_or_else(|| self.error("lone surrogate"))?
                }
            }
            _ => return Err(self.error("bad escape")),
        })
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let mut value = 0u32;
        for _ in 0..4 {
            let digit = self
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| self.error("expected four hex digits"))?;
            value = (value << 4) | digit;
        }
        Ok(value)
    }
}

/// Reads one flat JSON object whose values are strings, as `(key, value)`
/// pairs in the order of the text.
///
/// Duplicate keys are the caller's to judge; this reader only knows the
/// shape.
///
/// # Errors
///
/// [`Error`] for anything that is not exactly `{ "k": "v", ... }`.
pub fn string_map(text: &str) -> Result<Vec<(String, String)>, Error> {
    let mut parser = Parser::new(text);
    parser.skip_ws();
    parser.expect('{', "expected '{'")?;
    let mut pairs = Vec::new();
    parser.skip_ws();
    if parser.peek() == Some('}') {
        parser.next();
    } else {
        loop {
            parser.skip_ws();
            let key = parser.string()?;
            parser.skip_ws();
            parser.expect(':', "expected ':'")?;
            parser.skip_ws();
            let value = parser.string()?;
            pairs.push((key, value));
            parser.skip_ws();
            match parser.next() {
                Some(',') => {}
                Some('}') => break,
                _ => return Err(parser.error("expected ',' or '}'")),
            }
        }
    }
    parser.end()?;
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_flat_object_of_strings_reads_in_order_with_escapes() {
        let pairs =
            string_map(" { \"B\" : \"x\\ny\" , \"A\":\"\\u00e4\\ud83d\\ude00\" } ").unwrap();
        assert_eq!(
            pairs,
            [
                ("B".to_owned(), "x\ny".to_owned()),
                ("A".to_owned(), "\u{e4}\u{1f600}".to_owned()),
            ]
        );
        assert_eq!(string_map("{}").unwrap(), []);
    }

    /// DEL and the C1 controls stand raw in a JSON string; `serde_json` writes
    /// them that way, so the reader takes them byte for byte.
    #[test]
    fn del_and_c1_controls_are_ordinary_characters() {
        let pairs = string_map("{\"A\":\"x\u{7f}\u{85}\u{9f}y\"}").unwrap();
        assert_eq!(pairs, [("A".to_owned(), "x\u{7f}\u{85}\u{9f}y".to_owned())]);
    }

    #[test]
    fn anything_but_a_flat_object_of_strings_is_an_error() {
        for text in [
            "",
            "[]",
            "{",
            "{\"A\"}",
            "{\"A\":1}",
            "{\"A\":\"x\",}",
            "{\"A\":\"x\"} x",
            "{\"A\":{\"B\":\"x\"}}",
            "{\"A\":\"\u{1}\"}",
            "{\"A\":\"\\q\"}",
        ] {
            assert!(string_map(text).is_err(), "{text:?} parsed");
        }
    }
}
