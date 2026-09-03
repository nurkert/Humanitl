//! Der Suchschlüssel für Hosts: Labels rückwärts, damit ein Suffix ein Präfix wird.
//!
//! `host:github.com` soll `github.com` und jede Subdomain treffen, aber weder
//! `evil-github.com` noch `github.com.evil.io` (ADR-007, Glob auf Labels). Als
//! `LIKE '%.github.com'` geschrieben ist das ein Suffix-Vergleich, und den kann
//! kein Index beantworten.
//!
//! [`host_key`] dreht die Labels um und hängt einen Punkt an:
//!
//! | Host | Schlüssel |
//! |---|---|
//! | `github.com` | `com.github.` |
//! | `api.github.com` | `com.github.api.` |
//! | `evil-github.com` | `com.evil-github.` |
//! | `github.com.evil.io` | `io.evil.com.github.` |
//!
//! Aus dem Suffix wird damit ein Präfix und aus dem Präfix ein Bereich
//! ([`suffix_range`]): alles ab `com.github.` bis vor `com.github/`. Der Punkt
//! am Ende ist der Grund, warum `evil-github.com` nicht mitkommt — ohne ihn
//! wäre `com.github` auch ein Präfix von `com.githubusercontent`.
//!
//! Verglichen wird byteweise (`BINARY`, die Vorgabe von `SQLite`), und
//! Hostnamen sind nach der Normalisierung des Kerns A-Label in Kleinbuchstaben,
//! also reines ASCII.

/// Der Suchschlüssel eines Hosts: Labels rückwärts, mit abschließendem Punkt.
#[must_use]
pub fn host_key(host: &str) -> String {
    let mut out = String::with_capacity(host.len() + 1);
    for label in host.trim_end_matches('.').rsplit('.') {
        out.push_str(label);
        out.push('.');
    }
    out
}

/// Die Bereichsgrenzen für `host:<value>`: `[untere, obere)`.
///
/// Die untere Grenze ist der Schlüssel des Hosts selbst, die obere derselbe
/// Schlüssel mit `/` statt des abschließenden Punktes. Jeder Schlüssel, der mit
/// der unteren Grenze beginnt, liegt dazwischen, denn `.` (0x2E) ist das
/// kleinste Zeichen, das in einem Schlüssel an dieser Stelle stehen kann, und
/// `/` (0x2F) folgt unmittelbar darauf.
#[must_use]
pub fn suffix_range(value: &str) -> (String, String) {
    let low = host_key(&value.to_ascii_lowercase());
    let high = upper_bound(&low);
    (low, high)
}

/// Die erste Zeichenkette, die nicht mehr mit `key` beginnt.
fn upper_bound(key: &str) -> String {
    let mut out = key.to_owned();
    if out.ends_with('.') {
        out.pop();
        out.push('/');
    } else {
        // Kommt nicht vor, weil `host_key` immer einen Punkt anhängt. Falls
        // doch, ist das größte Zeichen die sichere Grenze.
        out.push('\u{10FFFF}');
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{host_key, suffix_range};

    #[test]
    fn labels_are_reversed_and_closed_with_a_dot() {
        assert_eq!(host_key("github.com"), "com.github.");
        assert_eq!(host_key("api.github.com"), "com.github.api.");
        assert_eq!(host_key("a.b.github.com"), "com.github.b.a.");
        assert_eq!(host_key("localhost"), "localhost.");
        assert_eq!(host_key("github.com."), "com.github.");
    }

    #[test]
    fn the_range_holds_the_host_and_its_subdomains_and_nothing_else() {
        let (low, high) = suffix_range("GitHub.com");
        assert_eq!(low, "com.github.");
        assert_eq!(high, "com.github/");

        for inside in ["github.com", "api.github.com", "a.b.github.com"] {
            let key = host_key(inside);
            assert!(key >= low && key < high, "{inside} fell out of the range");
        }
        for outside in [
            "evil-github.com",
            "github.com.evil.io",
            "githubusercontent.com",
            "notgithub.com",
            "com",
        ] {
            let key = host_key(outside);
            assert!(
                !(key >= low && key < high),
                "{outside} must not match host:github.com"
            );
        }
    }

    #[test]
    fn an_ip_literal_matches_only_itself() {
        let (low, high) = suffix_range("127.0.0.1");
        assert_eq!(host_key("127.0.0.1"), low);
        assert!(host_key("127.0.0.1") < high);
        let other = host_key("10.0.0.1");
        assert!(!(other >= low && other < high));
    }
}
