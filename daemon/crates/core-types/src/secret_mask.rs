//! Geheimnisse, die der Daemon selbst in den Verkehr gibt: nie im Klartext in
//! der Aufzeichnung, überall ein Fund außer an ihrem eigenen Ziel (HUM-140).
//!
//! Der Daemon kennt genau ein Geheimnis, das er selbst in den Verkehr gibt:
//! den Zugangsschlüssel des Modells. Der Agent schickt ihn als
//! `Authorization: Bearer …` an den Endpunkt, und die Durchreiche zeichnet
//! jede Anfrage vollständig auf. Ohne diese Maske stünde der Schlüssel danach
//! in `humanitl.db`, in der History und in jedem Export.
//!
//! Die Liste ist ein geteilter Wert: Die Aufzeichnung und der Scanner der
//! Funde halten Klone derselben Liste, und der Start einer Sitzung trägt den
//! Schlüssel einmal ein (`humanitl_ipc::sandbox`), zusammen mit dem Ziel, dem
//! er gehört (`host:port` von `llm.endpoint`). Ein Fund fällt nur dort weg, wo
//! der Schlüssel hingehört: in der Kopfzeile `Authorization` einer Anfrage an
//! genau dieses Ziel ([`SecretMasks::expected_at`]). Ohne das stünde an jeder
//! Anfrage an das Modell `LLM_005`, weil der Detektor für
//! `Authorization: Bearer …` ihn findet. An jedem anderen Ort — ein anderer
//! Host, ein Rumpf, eine andere Kopfzeile — bleibt er ein Fund: Dort trägt der
//! Agent den Schlüssel hinaus, und genau das soll der Mensch sehen. Damit er
//! dort auch ohne bekanntes Format gefunden wird, sucht der Scanner jeden Wert
//! wörtlich ([`SecretMasks::find_in`]).
//!
//! Die Maske ersetzt den Wert in den Kopfzeilen jeder aufgezeichneten
//! Nachricht durch `••••`, gleich an welchen Host sie ging: Schickt der Agent
//! den Schlüssel an ein anderes Ziel, wird die Anfrage dort wie jede andere
//! angehalten, aber auch dann steht der Wert nicht in der Datenbank. Rümpfe
//! ändert sie nicht: Ihre
//! Prüfsumme ist die Adresse des Blobs, und ein Rumpf, der anders gespeichert
//! wäre, als er ging, wäre eine falsche Aufzeichnung. `docs/SECURITY.md`
//! nennt diese Grenze.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::{Arc, PoisonError, RwLock};

use zeroize::Zeroizing;

use crate::http::sha256;

/// Was an der Stelle eines Geheimnisses steht.
pub const MASK: &str = "••••";

/// Ab dieser Länge wird ein Geheimnis auch mitten in einem Wert ersetzt.
///
/// Darunter nur ein ganzer Wert oder das letzte Wort eines Werts
/// (`Bearer x`): Ein Schlüssel `abc` für ein lokales Modell, das jeden nimmt,
/// stünde sonst in jedem dritten Wort jeder Kopfzeile, und die Aufzeichnung
/// wäre unlesbar, ohne dass sie etwas schützte.
pub const MIN_INNER_LEN: usize = 8;

/// Die Geheimnisse, die diese Aufzeichnung maskiert. Billig zu klonen; alle
/// Klone teilen dieselbe Liste.
#[derive(Clone, Default)]
pub struct SecretMasks {
    secrets: Arc<RwLock<Vec<Known>>>,
}

/// Ein bekanntes Geheimnis, die Prüfsumme, unter der ein Fund es nennt, und
/// das Ziel (`host:port`), an das es gehört.
struct Known {
    value: Zeroizing<String>,
    hash: [u8; 32],
    authority: Option<String>,
}

/// Der Wert einer Hex-Ziffer, groß oder klein geschrieben.
fn hex_digit(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit| u8::try_from(digit).ok())
}

/// Das Ende der Stelle, an der `secret` ab `at` in `haystack` steht, oder
/// `None`.
///
/// Verglichen wird Byte für Byte, und jedes Byte darf roh oder als `%XX`
/// dastehen, mit Hex-Ziffern in beliebiger Schreibung. So fällt jede
/// Mischung auf, die eine URL zulässt: ganz kodiert, nur teilweise kodiert,
/// `%2B` neben `%2f` (HUM-140, Review). Eine Liste fester Schreibweisen
/// ließe jede dieser Mischungen durch.
///
/// Ein `%` im Geheimnis selbst kann roh oder als `%25` dastehen; deshalb
/// versucht der Vergleich bei einem rohen `%` im Text erst die rohe Lesart
/// und, wenn sie scheitert, die kodierte. Nur an einem `%` des Geheimnisses
/// gibt es zwei Lesarten, also wächst der Aufwand mit deren Zahl und nicht
/// mit der Länge des Texts.
fn match_at(haystack: &[u8], at: usize, secret: &[u8]) -> Option<usize> {
    let Some((&want, rest)) = secret.split_first() else {
        return Some(at);
    };
    let byte = *haystack.get(at)?;
    if byte == want
        && let Some(end) = match_at(haystack, at + 1, rest)
    {
        return Some(end);
    }
    if byte == b'%' {
        let high = hex_digit(*haystack.get(at + 1)?)?;
        let low = hex_digit(*haystack.get(at + 2)?)?;
        if high * 16 + low == want {
            return match_at(haystack, at + 3, rest);
        }
    }
    None
}

/// Jede Stelle in `haystack`, an der eines der `secrets` steht, roh oder
/// kodiert, von links nach rechts und ohne Überlappung.
///
/// An jeder Stelle gewinnt der längste Treffer, nicht der zuerst
/// eingetragene: Nach einem Wechsel von `local-key-0123456789` auf
/// `local-key-0123456789-v2` innerhalb eines Laufs stünde sonst `••••-v2` in
/// der Aufzeichnung (HUM-140, Review).
fn spans(haystack: &[u8], secrets: &[&[u8]]) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < haystack.len() {
        let byte = haystack[at];
        let longest = secrets
            .iter()
            .filter(|secret| byte == b'%' || secret.first() == Some(&byte))
            .filter_map(|secret| match_at(haystack, at, secret))
            .max();
        if let Some(end) = longest {
            found.push(at..end);
            at = end;
        } else {
            at += 1;
        }
    }
    found
}

impl std::fmt::Debug for SecretMasks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self
            .secrets
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        f.debug_struct("SecretMasks")
            .field("count", &count)
            .finish()
    }
}

impl SecretMasks {
    /// Nimmt `secret` in die Liste auf, ohne Ziel: Es wird maskiert und ist
    /// überall ein Fund. Ein leerer Text und einer, der schon dasteht, ändern
    /// nichts.
    pub fn add(&self, secret: &str) {
        self.add_for(secret, None);
    }

    /// Nimmt `secret` auf, das an `authority` (`host:port`) gehört.
    ///
    /// Steht es schon da, gilt das neue Ziel: Ein Schlüssel gehört zu einem
    /// Zeitpunkt zu genau einem Endpunkt.
    pub fn add_for(&self, secret: &str, authority: Option<&str>) {
        if secret.is_empty() {
            return;
        }
        let authority = authority.map(str::to_ascii_lowercase);
        let mut secrets = self.secrets.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(known) = secrets
            .iter_mut()
            .find(|known| known.value.as_str() == secret)
        {
            known.authority = authority;
            return;
        }
        secrets.push(Known {
            value: Zeroizing::new(secret.to_owned()),
            hash: sha256(secret.as_bytes()),
            authority,
        });
    }

    /// Wo in `haystack` ein bekanntes Geheimnis steht, roh oder in einer URL
    /// kodiert, auch nur teilweise und in jeder Schreibung der Hex-Ziffern,
    /// sortiert und ohne Überlappung.
    ///
    /// Gesucht wird nur nach Geheimnissen ab [`MIN_INNER_LEN`] Zeichen: Ein
    /// kürzeres stünde zufällig in jedem zweiten Text. Ein Zugangsschlüssel
    /// ist nie so kurz, `ApiKey::new` lehnt ihn ab. Der Scanner der Funde
    /// macht aus jedem Bereich einen Fund (HUM-140, Review): Ein Schlüssel
    /// ohne bekanntes Format fände sonst kein Detektor.
    #[must_use]
    pub fn find_in(&self, haystack: &[u8]) -> Vec<Range<usize>> {
        let secrets = self.secrets.read().unwrap_or_else(PoisonError::into_inner);
        let long: Vec<&[u8]> = secrets
            .iter()
            .filter(|known| known.value.len() >= MIN_INNER_LEN)
            .map(|known| known.value.as_bytes())
            .collect();
        spans(haystack, &long)
    }

    /// Ein Hostname mit jedem bekannten Geheimnis durch [`MASK`] ersetzt,
    /// ohne Rücksicht auf Groß- und Kleinschreibung.
    ///
    /// Ein Host steht klein geschrieben in der Aufzeichnung, ein Schlüssel
    /// nicht unbedingt: Als Label einer Subdomain (`<key>.evil.example`) ist
    /// er trotzdem derselbe Wert (HUM-140, Review).
    #[must_use]
    pub fn apply_host<'a>(&self, host: &'a str) -> Cow<'a, str> {
        let secrets = self.secrets.read().unwrap_or_else(PoisonError::into_inner);
        let lowered: Vec<Zeroizing<String>> = secrets
            .iter()
            .filter(|known| known.value.len() >= MIN_INNER_LEN)
            .map(|known| Zeroizing::new(known.value.to_ascii_lowercase()))
            .collect();
        let needles: Vec<&[u8]> = lowered.iter().map(|value| value.as_bytes()).collect();
        let haystack = host.to_ascii_lowercase();
        replace_spans(host, &spans(haystack.as_bytes(), &needles))
    }

    /// Ob ein Fund mit dieser Prüfsumme ([`crate::Finding::value_hash`]) ein
    /// bekanntes Geheimnis an dem Ziel ist, dem es gehört.
    ///
    /// Der Aufrufer prüft dazu, dass der Fund in der Kopfzeile `Authorization`
    /// steht; hier steht nur der Vergleich von Wert und Ziel.
    #[must_use]
    pub fn expected_at(&self, hash: &[u8; 32], authority: &str) -> bool {
        self.secrets
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|known| {
                &known.hash == hash
                    && known
                        .authority
                        .as_deref()
                        .is_some_and(|own| own.eq_ignore_ascii_case(authority))
            })
    }

    /// `text` mit jedem bekannten Geheimnis durch [`MASK`] ersetzt.
    #[must_use]
    pub fn apply<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let secrets = self.secrets.read().unwrap_or_else(PoisonError::into_inner);
        let long: Vec<&[u8]> = secrets
            .iter()
            .filter(|known| known.value.len() >= MIN_INNER_LEN)
            .map(|known| known.value.as_bytes())
            .collect();
        let mut out = replace_spans(text, &spans(text.as_bytes(), &long));
        for known in secrets
            .iter()
            .filter(|known| known.value.len() < MIN_INNER_LEN)
        {
            let secret = known.value.as_str();
            if out == secret {
                out = Cow::Owned(MASK.to_owned());
            } else if let Some(head) = out.strip_suffix(secret).filter(|head| head.ends_with(' ')) {
                out = Cow::Owned(format!("{head}{MASK}"));
            }
        }
        out
    }
}

/// `text` mit jedem Bereich aus `spans` durch [`MASK`] ersetzt.
///
/// Die Bereiche beginnen und enden an Zeichengrenzen: Ein Treffer beginnt an
/// `%` oder am ersten Byte des Geheimnisses und endet nach einem ganzen
/// Zeichen oder nach `%XX`. Fiele einer doch mitten in ein Zeichen, bliebe
/// der Text dort lieber ganz maskiert als falsch geschnitten.
fn replace_spans<'a>(text: &'a str, spans: &[Range<usize>]) -> Cow<'a, str> {
    if spans.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for span in spans {
        match text.get(from..span.start) {
            Some(head) => out.push_str(head),
            None => return Cow::Owned(MASK.to_owned()),
        }
        out.push_str(MASK);
        from = span.end;
    }
    match text.get(from..) {
        Some(tail) => out.push_str(tail),
        None => return Cow::Owned(MASK.to_owned()),
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_long_secret_is_replaced_wherever_it_stands() {
        let masks = SecretMasks::default();
        masks.add("sk-live-0123456789");
        assert_eq!(masks.apply("Bearer sk-live-0123456789"), "Bearer ••••");
        assert_eq!(
            masks.apply("a=sk-live-0123456789;b=sk-live-0123456789"),
            "a=••••;b=••••"
        );
        assert_eq!(masks.apply("nothing here"), "nothing here");
    }

    /// Ein Schlüssel mit `+/=` steht in einer Query als `%2B…`; auch diese
    /// Schreibweise wird ersetzt und gefunden, groß wie klein (HUM-140,
    /// Review).
    #[test]
    fn a_percent_encoded_secret_is_masked_and_found_too() {
        let masks = SecretMasks::default();
        masks.add("key+0123456789/abc=");
        assert_eq!(
            masks.apply("/v1/models?k=key%2B0123456789%2Fabc%3D&x=1"),
            "/v1/models?k=••••&x=1"
        );
        assert_eq!(masks.apply("?k=key%2b0123456789%2fabc%3d"), "?k=••••");
        // Gemischt und nur teilweise kodiert, auch unreservierte Zeichen.
        assert_eq!(masks.apply("?k=key%2B0123456789%2fabc="), "?k=••••");
        assert_eq!(masks.apply("?k=%6bey+0123456789/abc%3D"), "?k=••••");
        assert_eq!(masks.apply("?k=key+0123456789%2Fabc%3d&x"), "?k=••••&x");
        assert_eq!(
            masks.apply("?k=key%2C0123456789/abc="),
            "?k=key%2C0123456789/abc="
        );
        let text = b"a key+0123456789/abc= b key%2B0123456789%2Fabc%3D";
        let found = masks.find_in(text);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(&text[found[0].clone()], b"key+0123456789/abc=");
        assert_eq!(&text[found[1].clone()], b"key%2B0123456789%2Fabc%3D");
    }

    /// Nach einem Wechsel des Schlüssels stehen beide in der Liste; der
    /// längere gewinnt, wo der kürzere sein Anfang ist. Ein `%` im Schlüssel
    /// darf als `%25` dastehen (HUM-140, Review).
    #[test]
    fn the_longest_secret_wins_and_a_percent_may_be_encoded() {
        let masks = SecretMasks::default();
        masks.add("local-key-0123456789");
        masks.add("local-key-0123456789-v2");
        assert_eq!(masks.apply("x local-key-0123456789-v2 y"), "x •••• y");
        assert_eq!(masks.apply("x local-key-0123456789 y"), "x •••• y");
        assert_eq!(masks.find_in(b"local-key-0123456789-v2"), vec![0..23]);

        let percent = SecretMasks::default();
        percent.add("key%0123456789abcdef");
        assert_eq!(percent.apply("?k=key%0123456789abcdef"), "?k=••••");
        assert_eq!(percent.apply("?k=key%250123456789abcdef"), "?k=••••");
    }

    /// Ein Schlüssel als Label einer Subdomain wird maskiert, gleich wie er
    /// geschrieben ist (HUM-140, Review).
    #[test]
    fn a_secret_in_a_host_is_masked_regardless_of_case() {
        let masks = SecretMasks::default();
        masks.add("Local-Key-0123456789abcdef");
        assert_eq!(
            masks.apply_host("local-key-0123456789abcdef.evil.example"),
            "••••.evil.example"
        );
        assert_eq!(masks.apply_host("api.example.com"), "api.example.com");
    }

    #[test]
    fn find_in_reports_every_place_and_ignores_short_secrets() {
        let masks = SecretMasks::default();
        masks.add("local-key-0123456789abcdef");
        masks.add("abc");
        let text = b"{\"k\":\"local-key-0123456789abcdef\"} abc local-key-0123456789abcdef";
        let found = masks.find_in(text);
        assert_eq!(found, vec![6..32, 39..65]);
        assert!(masks.find_in(b"nothing to see").is_empty());
    }

    #[test]
    fn a_short_secret_is_replaced_only_as_a_whole_word_at_the_end() {
        let masks = SecretMasks::default();
        masks.add("abc");
        assert_eq!(masks.apply("Bearer abc"), "Bearer ••••");
        assert_eq!(masks.apply("abc"), "••••");
        assert_eq!(masks.apply("text/abcdef"), "text/abcdef");
    }

    #[test]
    fn clones_share_the_list_and_debug_shows_no_secret() {
        let masks = SecretMasks::default();
        let clone = masks.clone();
        clone.add("sk-live-0123456789");
        clone.add("sk-live-0123456789");
        clone.add("");
        assert_eq!(masks.apply("sk-live-0123456789"), "••••");
        let shown = format!("{masks:?}");
        assert!(!shown.contains("sk-live"), "{shown}");
        assert!(shown.contains("count: 1"), "{shown}");
    }

    #[test]
    fn a_known_secret_is_expected_only_at_its_own_target() {
        let masks = SecretMasks::default();
        let hash = sha256(b"sk-live-0123456789");
        assert!(!masks.expected_at(&hash, "192.168.1.20:11434"));
        masks.add("sk-live-0123456789");
        assert!(
            !masks.expected_at(&hash, "192.168.1.20:11434"),
            "without a target it is expected nowhere"
        );
        masks.add_for("sk-live-0123456789", Some("192.168.1.20:11434"));
        assert!(masks.expected_at(&hash, "192.168.1.20:11434"));
        assert!(!masks.expected_at(&hash, "evil.example:443"));
        assert!(!masks.expected_at(&sha256(b"sk-live-other"), "192.168.1.20:11434"));
    }
}
