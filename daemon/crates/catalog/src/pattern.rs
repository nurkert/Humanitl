//! Host-Muster eines Katalogeintrags: prüfen und vergleichen.
//!
//! Verglichen werden ganze Labels, nie Zeichenketten. `**.github.com` darf
//! `evil-github.com` nicht treffen und `github.com.evil.io` auch nicht; beides
//! wäre mit `ends_with` oder `contains` sofort falsch (BACKLOG.md 4.5 Test 4).
//! Ein Katalogtreffer ist zwar keine Freigabe, aber er steht als „bekannter
//! Dienst" auf der Karte, und ein falscher Treffer wäre genau die Beruhigung,
//! die ein Angreifer will.
//!
//! Der Gang über die Labels steht nicht hier, sondern als
//! [`humanitl_core::rule::glob_matches`] im Kern. Er stand
//! einmal in dieser Datei und ein zweites Mal in `humanitl-rules`, weil
//! `humanitl-catalog` laut `backlog/CONVENTIONS.md` 3.1 und
//! `tools/deps-allow.toml` nicht von den Regeln abhängen darf. Beide Fassungen
//! dürfen von einem Host-Muster nie etwas Verschiedenes behaupten, also gibt
//! es nur noch eine, in der Schicht, die beide benutzen dürfen.

use humanitl_core::HostName;
use humanitl_core::rule::{HostPattern, HostPatternError, glob_matches};

/// Liest ein Host-Muster eines Katalogeintrags.
///
/// Der Katalog kennt nur zwei der vier Formen aus [`HostPattern`]: einen
/// exakten Namen und einen Label-Glob. `ip:` und `cidr:` werden abgelehnt, und
/// ein Name, der in Wahrheit eine Adresse ist (`140.82.112.3`), ebenso. Eine
/// Adresse hat keinen Betreiber, den man beschreiben könnte, und der Katalog
/// soll nicht die Stelle sein, an der eine Adresse einen Namen bekommt.
///
/// # Errors
///
/// [`HostPatternError`], wenn der Text kein Muster ist oder eine der beiden
/// erlaubten Formen verfehlt.
pub fn parse(input: &str) -> Result<HostPattern, HostPatternError> {
    match HostPattern::parse(input)? {
        HostPattern::Exact(HostName::Dns(name)) => Ok(HostPattern::Exact(HostName::Dns(name))),
        HostPattern::Glob(glob) => Ok(HostPattern::Glob(glob)),
        HostPattern::Exact(HostName::Ip(_)) | HostPattern::Ip(_) | HostPattern::Cidr { .. } => {
            Err(HostPatternError {
                input: input.to_owned(),
                reason: "a catalog entry describes names, not addresses",
            })
        }
    }
}

/// Wahr, wenn das Muster den Host trifft.
///
/// 1. Eine IP-Adresse trifft nie einen Katalogeintrag. Sie hat keinen Namen,
///    also auch keinen Dienst dahinter, den der Katalog kennen könnte.
/// 2. Ein Glob vergleicht Label für Label: `*` steht für genau ein Label, `**`
///    für ein oder mehr.
/// 3. Beginnt das Muster mit `**` und hat es mehr als ein Label, trifft es
///    zusätzlich den Namen ohne diese Labels: `**.example.com` trifft auch
///    `example.com` selbst.
#[must_use]
pub fn matches(pattern: &HostPattern, host: &HostName) -> bool {
    match (pattern, host) {
        (HostPattern::Exact(expected), HostName::Dns(_)) => expected == host,
        (HostPattern::Glob(glob), HostName::Dns(_)) => glob_matches(glob, host),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::HostName;

    use super::{matches, parse};

    fn hit(pattern: &str, host: &str) -> bool {
        let pattern = parse(pattern).unwrap();
        let host = HostName::parse(host).unwrap();
        matches(&pattern, &host)
    }

    #[test]
    fn an_address_is_not_a_catalog_pattern() {
        assert!(parse("140.82.112.3").is_err());
        assert!(parse("ip:140.82.112.3").is_err());
        assert!(parse("cidr:140.82.0.0/16").is_err());
    }

    #[test]
    fn an_address_never_matches_a_catalog_entry() {
        let pattern = parse("**.github.com").unwrap();
        let host = HostName::parse("140.82.112.3").unwrap();
        assert!(!matches(&pattern, &host));
    }

    #[test]
    fn a_glob_walks_labels_not_characters() {
        assert!(hit("**.github.com", "api.github.com"));
        assert!(hit("**.github.com", "github.com"));
        assert!(hit("**.github.com", "a.b.github.com"));
        assert!(!hit("**.github.com", "evil-github.com"));
        assert!(!hit("**.github.com", "github.com.evil.io"));
        assert!(!hit("**.github.com", "notgithub.com"));
    }

    #[test]
    fn one_star_is_exactly_one_label() {
        assert!(hit("*.github.com", "api.github.com"));
        assert!(!hit("*.github.com", "github.com"));
        assert!(!hit("*.github.com", "a.b.github.com"));
    }

    #[test]
    fn an_exact_pattern_is_exact() {
        assert!(hit("crates.io", "crates.io"));
        assert!(hit("crates.io", "CRATES.IO."));
        assert!(!hit("crates.io", "static.crates.io"));
    }
}
