//! Die Matching-Tabelle aus `backlog/sprint-2.md` (HUM-022), Zeile für Zeile.
//!
//! Jede Zeile der Tabelle ist genau ein Test, benannt nach ihrer Nummer. Die
//! Tabelle ist die Sicherheitsaussage dieser Crate in Prüfbarer Form: die
//! Zeilen 4, 5, 11, 12 und 21 sind der Unterschied zwischen einem
//! Label-Vergleich und einem Vergleich auf Zeichenketten, und genau dieser
//! Unterschied ist der übliche Weg an einer Host-Regel vorbei.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::Utc;
use humanitl_core::diagnostics::codes::{RULES_002, RULES_003};
use humanitl_core::rule::{Action, HostPattern, Matcher};
use humanitl_core::{
    Diagnostic, DiagnosticCode, HostName, Method, Rule, RuleId, Scheme, SessionId, Severity,
    Upgrade,
};
use humanitl_rules::{RequestKey, RuleSet, Verdict, host_matches};

/// Ein Muster aus der Tabelle, ohne seine Warnungen.
fn pattern(text: &str) -> HostPattern {
    parsed(text).0
}

/// Ein Muster aus der Tabelle samt seinen Warnungen.
fn parsed(text: &str) -> (HostPattern, Vec<Diagnostic>) {
    match humanitl_rules::host::parse_pattern(text) {
        Ok(result) => result,
        Err(diagnostic) => panic!("pattern {text:?} must parse: {}", diagnostic.why),
    }
}

/// Die Codes der Warnungen eines Musters.
fn warning_codes(text: &str) -> Vec<DiagnosticCode> {
    parsed(text).1.iter().map(|warning| warning.code).collect()
}

/// Ein Host aus der Tabelle.
fn host(text: &str) -> HostName {
    HostName::parse(text).unwrap_or_else(|err| panic!("host {text:?} must parse: {err}"))
}

/// Trifft das Muster den Host?
fn hits(pat: &str, raw_host: &str) -> bool {
    host_matches(&pattern(pat), &host(raw_host))
}

/// Ein Regelsatz mit genau einer erlaubenden Regel über diesem Muster.
fn allow(pat: &str) -> (RuleSet, RuleId) {
    allow_matcher(Matcher::host(pattern(pat)))
}

fn allow_matcher(matcher: Matcher) -> (RuleSet, RuleId) {
    let id = RuleId::new();
    let set = RuleSet::from_rules([Rule::new(id, Action::Allow, matcher)]);
    (set, id)
}

#[test]
fn host_01() {
    assert!(hits("*.github.com", "api.github.com"), "one label");
}

#[test]
fn host_02() {
    assert!(
        !hits("*.github.com", "github.com"),
        "`*` needs exactly one label"
    );
}

#[test]
fn host_03() {
    assert!(!hits("*.github.com", "a.b.github.com"), "two labels");
}

#[test]
fn host_04() {
    assert!(
        !hits("*.github.com", "evil-github.com"),
        "labels are compared, never substrings"
    );
}

#[test]
fn host_05() {
    assert!(
        !hits("*.github.com", "github.com.evil.io"),
        "the suffix does not match"
    );
}

#[test]
fn host_06() {
    assert!(
        hits("*.github.com", "API.GITHUB.COM."),
        "lowercase and trailing dot"
    );
}

#[test]
fn host_07() {
    assert!(hits("*.github.com", "api.github.com."), "trailing dot");
}

#[test]
fn host_08() {
    assert!(hits("**.github.com", "github.com"), "apex exception");
}

#[test]
fn host_09() {
    assert!(hits("**.github.com", "api.github.com"));
}

#[test]
fn host_10() {
    assert!(hits("**.github.com", "a.b.c.github.com"), "several labels");
}

#[test]
fn host_11() {
    assert!(!hits("**.github.com", "github.com.evil.io"));
}

#[test]
fn host_12() {
    assert!(!hits("**.github.com", "notgithub.com"));
}

#[test]
fn host_13() {
    assert!(hits("github.com", "github.com"), "exact");
}

#[test]
fn host_14() {
    assert!(!hits("github.com", "www.github.com"), "exact means exact");
}

#[test]
fn host_15() {
    assert!(hits("github.com", "GitHub.Com"), "case");
}

#[test]
fn host_16() {
    assert!(hits("*.*.example.com", "a.b.example.com"));
}

#[test]
fn host_17() {
    assert!(!hits("*.*.example.com", "a.example.com"));
}

#[test]
fn host_18() {
    assert!(hits("api.*.com", "api.github.com"), "star in the middle");
}

#[test]
fn host_19() {
    assert!(!hits("api.*.com", "api.co.uk"), "the last label decides");
}

#[test]
fn host_20() {
    assert!(hits("**", "anything.example"), "every DNS host");
}

#[test]
fn host_21() {
    assert!(
        !hits("**", "140.82.112.3"),
        "an address is never matched by a glob"
    );
}

#[test]
fn host_22() {
    assert!(hits("*", "localhost"), "one label");
}

#[test]
fn host_23() {
    assert!(!hits("*", "a.b"));
}

#[test]
fn host_24() {
    assert!(
        hits("münchen.de", "xn--mnchen-3ya.de"),
        "the pattern becomes an A-label"
    );
    // Der geschriebene Name ist die empfohlene Schreibweise: kein Befund,
    // obwohl IDNA daraus `xn--mnchen-3ya.de` macht.
    assert!(
        warning_codes("münchen.de").is_empty(),
        "a unicode pattern is reported: {:?}",
        warning_codes("münchen.de")
    );
    assert!(warning_codes("*.münchen.de").is_empty());
}

#[test]
fn host_25() {
    assert!(
        hits("xn--mnchen-3ya.de", "münchen.de"),
        "the host becomes an A-label"
    );

    // Nur das selbst geschriebene Punycode-Literal wird gemeldet.
    let (_, warnings) = parsed("xn--mnchen-3ya.de");
    assert_eq!(warnings.len(), 1, "the punycode literal is reported");
    assert_eq!(warnings[0].code, RULES_002);
    assert_eq!(warnings[0].severity, Severity::Warning);
    assert!(warnings[0].why.contains("xn--mnchen-3ya"));
    assert_eq!(warning_codes("*.XN--MNCHEN-3YA.de"), vec![RULES_002]);
}

#[test]
fn host_26() {
    assert!(!hits("*.github.com", "140.82.112.3"), "an address");
}

#[test]
fn host_27() {
    assert!(hits("ip:140.82.112.3", "140.82.112.3"));
}

#[test]
fn host_28() {
    assert!(!hits("ip:140.82.112.3", "140.82.112.4"));
}

#[test]
fn host_29() {
    assert!(
        hits("ip:140.82.112.3", "[::ffff:140.82.112.3]"),
        "a mapped IPv6 address is canonicalised"
    );
}

#[test]
fn host_30() {
    assert!(hits("cidr:192.168.0.0/16", "192.168.1.50"));
}

#[test]
fn host_31() {
    assert!(!hits("cidr:192.168.0.0/16", "10.0.0.1"));
}

/// Nachtrag aus dem Review: ein Netz in IPv4-mapped Schreibweise.
///
/// `::ffff:192.168.0.0/112` bezeichnet `192.168.0.0/16`. Wird nur die Adresse
/// umgerechnet und die Länge stehen gelassen, schrumpft das Netz auf eine
/// einzige Adresse, und eine Regel deckt plötzlich 65 535 Ziele weniger ab.
#[test]
fn host_30_mapped_network() {
    assert!(hits("cidr:::ffff:192.168.0.0/112", "192.168.1.50"));
    assert!(hits("cidr:::ffff:192.168.0.0/112", "[::ffff:192.168.1.50]"));
    assert!(!hits("cidr:::ffff:192.168.0.0/112", "10.0.0.1"));
    assert!(!hits("cidr:::ffff:192.168.0.0/112", "192.169.1.50"));
    assert!(warning_codes("cidr:::ffff:192.168.0.0/112").is_empty());
}

/// Nachtrag aus dem Review: ein IPv4-mapped Netz, das über die eingebettete
/// Adresse hinausreicht, ist keine Regel, sondern ein Missverständnis.
#[test]
fn host_31_mapped_network_needs_prefix_96() {
    let Err(diagnostic) = humanitl_rules::host::parse_pattern("cidr:::ffff:192.168.0.0/64") else {
        panic!("a mapped network with a prefix below 96 must be refused");
    };
    assert_eq!(diagnostic.code, RULES_003);
    assert!(diagnostic.why.contains("96"), "{}", diagnostic.why);

    // Und ein echtes IPv6-Netz bleibt unangetastet.
    assert!(hits("cidr:2606:4700::/32", "[2606:4700::1]"));
    assert!(!hits("cidr:2606:4700::/32", "192.168.1.50"));
}

#[test]
fn host_32() {
    assert!(
        !hits("cidr:192.168.0.0/16", "192.168.1.50.nip.io"),
        "a name is not an address"
    );
}

#[test]
fn host_33() {
    assert!(hits("ip:::1", "[::1]"));
}

#[test]
fn host_34() {
    assert!(
        !hits("ip:127.0.0.1", "localhost"),
        "no rule resolves a name"
    );
}

#[test]
fn host_35() {
    assert!(
        HostName::parse("0x8c527003").is_err(),
        "a hexadecimal address is not a host"
    );
}

#[test]
fn host_36() {
    assert!(
        HostName::parse("0177.0.0.1").is_err(),
        "an octal address is not a host"
    );
}

#[test]
fn host_37() {
    // Eine Regel ohne `upgrade` trifft nie ein Upgrade.
    let (rules, _) = allow("*.github.com");
    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443)
        .with_upgrade(Upgrade::WebSocket);
    assert_eq!(
        rules.evaluate(&key, Utc::now(), SessionId::new()),
        Verdict::Default
    );
}

#[test]
fn host_38() {
    // Und eine Regel mit `upgrade` trifft nie eine gewöhnliche Anfrage.
    let matcher = Matcher::host(pattern("*.github.com")).with_upgrade(Upgrade::WebSocket);
    let (rules, _) = allow_matcher(matcher);
    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, Utc::now(), SessionId::new()),
        Verdict::Default
    );
}

#[test]
fn host_39() {
    let Err(diagnostic) = humanitl_rules::host::parse_pattern("*foo.com") else {
        panic!("a wildcard must be a whole label");
    };
    assert_eq!(diagnostic.code, RULES_003);
    assert!(diagnostic.why.contains("*foo.com"));
}

#[test]
fn host_40() {
    let Err(diagnostic) = humanitl_rules::host::parse_pattern("foo..com") else {
        panic!("an empty label is not a pattern");
    };
    assert_eq!(diagnostic.code, RULES_003);
}

#[test]
fn host_41() {
    let Err(diagnostic) = humanitl_rules::host::parse_pattern("") else {
        panic!("an empty pattern is not a pattern");
    };
    assert_eq!(diagnostic.code, RULES_003);
}

#[test]
fn host_42() {
    // Der Port gehört nicht in den Host: `api.github.com:8443` ist keine
    // Adresse, und `HostName::parse` sagt das. Aufgeteilt trifft der Glob den
    // Host, und über den Port entscheidet der eigene Schlüssel.
    assert!(HostName::parse("api.github.com:8443").is_err());
    assert!(hits("*.github.com", "api.github.com"));

    let matcher = Matcher::host(pattern("*.github.com")).with_port(8443);
    let (rules, id) = allow_matcher(matcher);
    let target = host("api.github.com");

    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 8443);
    assert_eq!(
        rules.evaluate(&key, Utc::now(), SessionId::new()),
        Verdict::Matched {
            rule: id,
            action: Action::Allow
        }
    );

    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, Utc::now(), SessionId::new()),
        Verdict::Default
    );
}
