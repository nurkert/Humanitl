//! Der Apex eines Hosts nach der Public Suffix List.
//!
//! Der Apex ist die registrierbare Domain: `api.github.com` gehört zu
//! `github.com`, `a.b.github.io` zu `b.github.io`, denn `github.io` steht im
//! privaten Abschnitt der Liste und jede Unterdomain darunter hat einen eigenen
//! Betreiber. Das ist der Unterschied, den ein Mensch sehen muss, bevor er eine
//! Regel `**.<apex>` anlegt.
//!
//! Die Liste ist in der Crate `psl` einkompiliert; zur Laufzeit wird nichts
//! geholt (ADR-006). Sie ändert sich mit jeder Fassung der Crate, und der Apex
//! ist eine Aussage, die im Panel steht: die Version gehört deshalb in den
//! Lockfile-Pin und in den Changelog des Releases.

use humanitl_core::HostName;

/// Die registrierbare Domain des Hosts, oder `None`.
///
/// `None` heißt genau eine von zwei Sachen, und beide sind „unbekannt", nie
/// „unbedenklich":
///
/// - Der Host ist eine IP-Adresse. Eine Adresse hat keinen Apex; die Oberfläche
///   schreibt dann „IP address" an die Stelle.
/// - Die Liste kennt das Suffix nicht, oder der Name besteht nur aus einem
///   Suffix (`com`, `co.uk`). Dann gibt es keine registrierbare Domain, die man
///   nennen könnte.
#[must_use]
pub fn apex(host: &HostName) -> Option<String> {
    let name = host.as_dns()?;
    ::psl::domain_str(name).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::HostName;

    use super::apex;

    fn apex_of(host: &str) -> Option<String> {
        apex(&HostName::parse(host).unwrap())
    }

    #[test]
    fn icann_suffix_gives_the_registrable_domain() {
        assert_eq!(apex_of("api.github.com").as_deref(), Some("github.com"));
        assert_eq!(apex_of("github.com").as_deref(), Some("github.com"));
        assert_eq!(
            apex_of("files.pythonhosted.org").as_deref(),
            Some("pythonhosted.org")
        );
        assert_eq!(
            apex_of("a.b.example.co.uk").as_deref(),
            Some("example.co.uk")
        );
    }

    #[test]
    fn private_suffix_counts_too() {
        // `github.io` steht im privaten Abschnitt der Liste: jede Unterdomain
        // gehört jemand anderem, also ist `b.github.io` der Apex und nicht
        // `github.io`.
        assert_eq!(apex_of("a.b.github.io").as_deref(), Some("b.github.io"));
        assert_eq!(apex_of("b.github.io").as_deref(), Some("b.github.io"));
    }

    #[test]
    fn an_address_has_no_apex() {
        assert_eq!(apex_of("140.82.112.3"), None);
        assert_eq!(apex_of("::1"), None);
    }

    #[test]
    fn a_bare_suffix_has_no_apex() {
        assert_eq!(apex_of("com"), None);
        assert_eq!(apex_of("co.uk"), None);
    }

    #[test]
    fn the_name_is_compared_after_normalisation() {
        // `HostName::parse` hat A-Label und Kleinschreibung schon erledigt.
        assert_eq!(apex_of("API.GitHub.COM.").as_deref(), Some("github.com"));
    }
}
