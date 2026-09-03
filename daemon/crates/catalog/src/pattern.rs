//! Host-Muster eines Katalogeintrags: prüfen und vergleichen.
//!
//! Verglichen werden ganze Labels, nie Zeichenketten. `**.github.com` darf
//! `evil-github.com` nicht treffen und `github.com.evil.io` auch nicht; beides
//! wäre mit `ends_with` oder `contains` sofort falsch (BACKLOG.md 4.5 Test 4).
//! Ein Katalogtreffer ist zwar keine Freigabe, aber er steht als „bekannter
//! Dienst" auf der Karte, und ein falscher Treffer wäre genau die Beruhigung,
//! die ein Angreifer will.
//!
//! **Doppelung, bewusst und eng begrenzt.** Dieselbe Label-Semantik steht in
//! `humanitl-rules` (`host.rs`). `humanitl-catalog` darf laut
//! `backlog/CONVENTIONS.md` 3.1 und `tools/deps-allow.toml` nur von
//! `humanitl-core` abhängen, und der Vergleich liegt nicht im Kern. Der
//! Mustertyp [`HostPattern`] wird deshalb aus dem Kern übernommen, der Gang
//! über die Labels hier wiederholt. Damit die beiden nicht auseinanderlaufen,
//! steht in `tests/host_table.rs` dieselbe Tabelle wie in
//! `daemon/crates/rules/tests/host_table.rs`. Der saubere Weg wäre, `matches`
//! und `split_pattern` nach `humanitl_core::rule` zu ziehen; das ist ein
//! eigener Umbau und braucht die Zustimmung beider Crates.

use humanitl_core::HostName;
use humanitl_core::rule::{HostPattern, HostPatternError};

/// Ein Label eines Musters.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LabelPat {
    /// Genau dieses Label, schon normalisiert (A-Label, klein).
    Literal(String),
    /// `*`: genau ein Label, gleich welches.
    One,
    /// `**`: ein oder mehr Labels.
    Many,
}

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
        (HostPattern::Glob(glob), HostName::Dns(_)) => {
            let Some(labels) = host.labels() else {
                return false;
            };
            glob_matches(&split_pattern(glob), &labels)
        }
        _ => false,
    }
}

/// Zerlegt ein Glob-Muster in seine Labels.
fn split_pattern(glob: &str) -> Vec<LabelPat> {
    glob.split('.')
        .map(|label| match label {
            "*" => LabelPat::One,
            "**" => LabelPat::Many,
            literal => LabelPat::Literal(literal.to_owned()),
        })
        .collect()
}

/// Der Vergleich aus Schritt 2 und 3.
fn glob_matches(pattern: &[LabelPat], labels: &[&str]) -> bool {
    if matches_from(pattern, labels) {
        return true;
    }
    // Apex-Ausnahme: `**.example.com` trifft `example.com`. Nur für ein
    // führendes `**` und nur, wenn danach noch etwas steht.
    matches!(pattern.first(), Some(LabelPat::Many))
        && pattern.len() > 1
        && matches_from(&pattern[1..], labels)
}

fn matches_from(pattern: &[LabelPat], labels: &[&str]) -> bool {
    match pattern.split_first() {
        None => labels.is_empty(),
        Some((LabelPat::Literal(expected), rest)) => match labels.split_first() {
            Some((label, tail)) => label == expected && matches_from(rest, tail),
            None => false,
        },
        Some((LabelPat::One, rest)) => match labels.split_first() {
            Some((_, tail)) => matches_from(rest, tail),
            None => false,
        },
        Some((LabelPat::Many, rest)) => {
            (1..=labels.len()).any(|taken| matches_from(rest, &labels[taken..]))
        }
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
