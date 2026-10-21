//! Kopfzeilen und Body auf dem Weg in die Datenbank.
//!
//! Die Kopfzeilen stehen als `[["name","value"],…]` in `messages.headers_json`,
//! in der Reihenfolge, in der sie ankamen: mehrere `Set-Cookie` bleiben
//! mehrere, und eine Sortierung nach Namen würde die Anfrage verfälschen, die
//! der Mensch freigegeben hat.
//!
//! Ein Header-Wert darf nach RFC 9110 Bytes tragen, die kein `UTF-8` sind.
//! Solche Bytes werden beim Schreiben ersetzt (`U+FFFD`), weil `SQLite` in
//! einer `TEXT`-Spalte nichts anderes hält; das Original bleibt im Body-Verweis
//! unberührt.

use humanitl_core::{HeaderMap, SecretMasks};

/// Kopfzeilen als `JSON`-Liste von Paaren, mit jedem Geheimnis aus `masks`
/// durch `••••` ersetzt (HUM-140).
pub fn encode_headers(headers: &HeaderMap, masks: &SecretMasks) -> String {
    let pairs = masked_pairs(headers, masks);
    serde_json::to_string(&pairs).unwrap_or_else(|_unserializable| "[]".to_owned())
}

/// Kopfzeilen als Paare aus Name und Text, in Originalreihenfolge, mit jedem
/// Geheimnis aus `masks` durch `••••` ersetzt: genau so, wie
/// [`encode_headers`] sie in die Datenbank schreibt.
pub fn masked_pairs(headers: &HeaderMap, masks: &SecretMasks) -> Vec<[String; 2]> {
    headers
        .iter()
        .map(|(name, value)| {
            let text = masks
                .apply(&String::from_utf8_lossy(value.as_bytes()))
                .into_owned();
            // `Host` trägt einen Namen, und ein Name ist ohne Rücksicht auf
            // Groß- und Kleinschreibung derselbe: wie `flows.host` (HUM-140,
            // Review).
            let text = if name.as_str() == "host" {
                masks.apply_host(&text).into_owned()
            } else {
                text
            };
            [name.as_str().to_owned(), text]
        })
        .collect()
}

/// Der Wert aus `Content-Type`, falls einer da ist, maskiert wie jede andere
/// Kopfzeile.
///
/// Die Spalte `messages.content_type` steht neben `headers_json`; ein
/// Geheimnis, das der Agent in diese Kopfzeile schreibt, stünde dort sonst im
/// Klartext (HUM-140, Review).
pub fn content_type_of(headers: &HeaderMap, masks: &SecretMasks) -> Option<String> {
    header_text(headers, "content-type", masks)
}

/// Der Wert aus `Content-Encoding`, falls einer da ist, maskiert wie
/// [`content_type_of`].
pub fn content_encoding_of(headers: &HeaderMap, masks: &SecretMasks) -> Option<String> {
    header_text(headers, "content-encoding", masks)
}

/// Ein Header als Text, verlustbehaftet gelesen und maskiert.
fn header_text(headers: &HeaderMap, name: &str, masks: &SecretMasks) -> Option<String> {
    let value = headers.get(name)?;
    let text = masks
        .apply(&String::from_utf8_lossy(value.as_bytes()))
        .into_owned();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::http::HeaderValue;
    use humanitl_core::{HeaderMap, SecretMasks};

    use super::{content_encoding_of, content_type_of, encode_headers};

    #[test]
    fn headers_keep_their_order_and_their_repetitions() {
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("api.github.com"));
        headers.append("set-cookie", HeaderValue::from_static("a=1"));
        headers.append("set-cookie", HeaderValue::from_static("b=2"));
        let json = encode_headers(&headers, &SecretMasks::default());
        assert_eq!(
            json,
            r#"[["host","api.github.com"],["set-cookie","a=1"],["set-cookie","b=2"]]"#
        );
    }

    #[test]
    fn content_type_and_encoding_come_from_the_headers() {
        let mut headers = HeaderMap::new();
        assert_eq!(content_type_of(&headers, &SecretMasks::default()), None);
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        assert_eq!(
            content_type_of(&headers, &SecretMasks::default()),
            Some("application/json".to_owned())
        );
        assert_eq!(
            content_encoding_of(&headers, &SecretMasks::default()),
            Some("gzip".to_owned())
        );
    }

    #[test]
    fn a_value_that_is_not_utf8_is_replaced_not_dropped() {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_bytes(&[0xff, 0x41]).unwrap_or_else(|err| panic!("{err}"));
        headers.insert("x-odd", value);
        let json = encode_headers(&headers, &SecretMasks::default());
        assert!(json.contains("x-odd"));
        assert!(json.contains('A'));
    }
}
