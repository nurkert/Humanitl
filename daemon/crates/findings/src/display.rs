//! Der maskierte Anfang eines Funds.
//!
//! Ein Fund wird angezeigt, protokolliert und in Diagnostics genannt. Damit
//! dabei nie ein Geheimnis im Klartext steht, trägt er nur diesen Anfang: genug,
//! um zwei Funde auseinanderzuhalten, zu wenig, um den Wert zu benutzen. Die
//! Form hängt an der Art des Funds (HUM-025).

use humanitl_core::FindingKind;
use humanitl_core::finding::display_prefix as core_display_prefix;

/// So viele Zeichen eines Schlüssels oder Tokens bleiben sichtbar.
const SECRET_PREFIX_CHARS: usize = 6;

/// So viele Zeichen einer Telefonnummer bleiben sichtbar.
const PHONE_PREFIX_CHARS: usize = 4;

/// So viele Zeichen einer IBAN bleiben sichtbar (Land und Prüfziffer).
const IBAN_PREFIX_CHARS: usize = 4;

/// So viele Ziffern einer Kartennummer bleiben sichtbar.
const CARD_SUFFIX_CHARS: usize = 4;

/// Baut den Anzeige-Anfang für einen Fund dieser Art.
///
/// Gezählt wird in Zeichen, nicht in Bytes, damit ein mehrbyteiges Zeichen nicht
/// zerschnitten wird.
#[must_use]
pub fn display_prefix(kind: &FindingKind, value: &str) -> String {
    match kind {
        FindingKind::ApiKey(_) | FindingKind::Jwt => head(value, SECRET_PREFIX_CHARS),
        FindingKind::Email => email(value),
        FindingKind::Iban => format!(
            "{} …",
            value.chars().take(IBAN_PREFIX_CHARS).collect::<String>()
        ),
        FindingKind::CreditCard => card(value),
        FindingKind::Phone => head(value, PHONE_PREFIX_CHARS),
        FindingKind::Ipv4 | FindingKind::UserTerm(_) => value.to_owned(),
        FindingKind::Custom(_) => core_display_prefix(value),
    }
}

/// Die ersten `keep` Zeichen, danach ein Auslassungszeichen.
fn head(value: &str, keep: usize) -> String {
    let mut out: String = value.chars().take(keep).collect();
    if value.chars().nth(keep).is_some() {
        out.push('…');
    }
    out
}

/// Erstes Zeichen, drei Sterne, die Domain.
fn email(value: &str) -> String {
    let Some((local, domain)) = value.rsplit_once('@') else {
        return head(value, SECRET_PREFIX_CHARS);
    };
    let first: String = local.chars().take(1).collect();
    format!("{first}***@{domain}")
}

/// Vier Sterne und die letzten vier Ziffern.
fn card(value: &str) -> String {
    let digits: Vec<char> = value.chars().filter(char::is_ascii_digit).collect();
    let tail: String = digits
        .iter()
        .skip(digits.len().saturating_sub(CARD_SUFFIX_CHARS))
        .collect();
    format!("**** {tail}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::FindingKind;

    use super::display_prefix;

    #[test]
    fn a_secret_shows_six_characters_at_most() {
        // Zur Laufzeit zusammengesetzt: Der Push-Schutz von GitHub blockiert
        // ein echt geformtes Token im Quelltext (CONVENTIONS 4.13).
        let value = format!("{}{}", "ghp", "_0123456789abcdefghijklmnopqrstuvwxyz");
        assert_eq!(
            display_prefix(&FindingKind::ApiKey("github".to_owned()), &value),
            "ghp_01…"
        );
        assert_eq!(
            display_prefix(&FindingKind::Jwt, "eyJhbGciOi.eyJzdWIi.sig"),
            "eyJhbG…"
        );
        assert!(!display_prefix(&FindingKind::ApiKey("github".to_owned()), &value).contains("789"));
    }

    #[test]
    fn personal_data_keeps_shape_not_value() {
        assert_eq!(
            display_prefix(&FindingKind::Email, "vorname.nachname@example.com"),
            "v***@example.com"
        );
        assert_eq!(
            display_prefix(&FindingKind::Iban, "DE89 3704 0044 0532 0130 00"),
            "DE89 …"
        );
        assert_eq!(
            display_prefix(&FindingKind::CreditCard, "4111 1111 1111 1111"),
            "**** 1111"
        );
        assert_eq!(
            display_prefix(&FindingKind::Phone, "+49 30 1234567"),
            "+49 …"
        );
        assert_eq!(
            display_prefix(&FindingKind::Ipv4, "192.168.1.20"),
            "192.168.1.20"
        );
        assert_eq!(
            display_prefix(&FindingKind::UserTerm("Acme".to_owned()), "ACME"),
            "ACME"
        );
    }
}
