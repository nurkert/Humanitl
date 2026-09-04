//! Der Label-Vergleich für Host-Globs, an der einen Stelle, an der er steht.
//!
//! [`humanitl_core::rule::glob_matches`] ist seit HUM-022 die einzige Fassung
//! dieses Vergleichs. Vorher lag er zweimal im Baum, in `humanitl-rules` und in
//! `humanitl-catalog`, weil der Katalog nicht von den Regeln abhängen darf; die
//! beiden Fassungen waren gleich, aber nichts hielt sie gleich. Ein Host-Muster
//! falsch zu vergleichen ist ein Sicherheitsfehler, deshalb steht die Tabelle
//! jetzt hier, direkt am Code.
//!
//! Die Zeilen sind die aus `backlog/sprint-2.md` (HUM-022), soweit sie das
//! Glob betreffen. Die drei, auf die es ankommt: `evil-github.com`,
//! `github.com.evil.io` und `notgithub.com` treffen `**.github.com` nicht. Mit
//! `ends_with` oder `contains` täten sie es, und genau das ist der übliche Weg
//! an einer Host-Regel vorbei.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_core::HostName;
use humanitl_core::rule::{HostPattern, glob_matches};

/// Muster, Host, erwartetes Ergebnis.
const TABLE: &[(&str, &str, bool)] = &[
    // `*` ist genau ein Label.
    ("*.github.com", "api.github.com", true),
    ("*.github.com", "github.com", false),
    ("*.github.com", "a.b.github.com", false),
    // `**` ist ein Label oder mehr, plus die Apex-Ausnahme.
    ("**.github.com", "api.github.com", true),
    ("**.github.com", "a.b.github.com", true),
    ("**.github.com", "github.com", true),
    // Labels, keine Zeichenketten. Die drei Zeilen sind die Sicherheitsaussage.
    ("**.github.com", "evil-github.com", false),
    ("**.github.com", "github.com.evil.io", false),
    ("**.github.com", "notgithub.com", false),
    ("*.github.com", "evil-github.com", false),
    ("*.github.com", "github.com.evil.io", false),
    // Normalisierung auf beiden Seiten: Kleinschreibung, abschließender Punkt,
    // A-Label.
    ("*.github.com", "API.GITHUB.COM.", true),
    ("*.münchen.de", "shop.münchen.de", true),
    ("*.münchen.de", "shop.xn--mnchen-3ya.de", true),
    // Ein Platzhalter in der Mitte und am Ende.
    ("api.*.com", "api.github.com", true),
    ("api.*.com", "api.github.io", false),
    ("**.com", "github.com", true),
    ("*.**.github.com", "a.b.c.github.com", true),
    ("*.**.github.com", "a.github.com", false),
];

fn pattern(text: &str) -> String {
    match HostPattern::glob(text) {
        Ok(HostPattern::Glob(glob)) => glob,
        Ok(other) => panic!("pattern {text:?} is not a glob: {other}"),
        Err(err) => panic!("pattern {text:?} must parse: {}", err.reason),
    }
}

fn host(text: &str) -> HostName {
    HostName::parse(text).unwrap_or_else(|err| panic!("host {text:?} must parse: {err}"))
}

#[test]
fn glob_table() {
    for (glob, name, want) in TABLE {
        let got = glob_matches(&pattern(glob), &host(name));
        assert_eq!(got, *want, "{glob} against {name}");
    }
}

/// Eine Adresse hat keine Labels und trifft deshalb nie ein Glob.
///
/// Wer `169.254.169.254` oder das Heimnetz meint, schreibt `ip:` oder `cidr:`
/// (ADR-007). Ein Glob, das eine Adresse trifft, wäre eine Regel, die etwas
/// anderes erlaubt als das, was dasteht.
#[test]
fn an_address_never_matches_a_glob() {
    for addr in ["140.82.112.3", "[::1]", "[::ffff:140.82.112.3]"] {
        for glob in ["**.github.com", "*.*.*.*", "**"] {
            assert!(
                !glob_matches(&pattern(glob), &host(addr)),
                "{glob} must not match the address {addr}"
            );
        }
    }
}

/// Die Apex-Ausnahme gilt nur für ein führendes `**` und nur, wenn danach noch
/// etwas steht.
#[test]
fn the_apex_exception_is_narrow() {
    // Führendes `**`, mehr als ein Label: die Ausnahme greift.
    assert!(glob_matches(
        &pattern("**.example.com"),
        &host("example.com")
    ));
    // `**` in der Mitte verlangt weiterhin mindestens ein Label.
    assert!(glob_matches(
        &pattern("api.**.example.com"),
        &host("api.x.example.com")
    ));
    assert!(!glob_matches(
        &pattern("api.**.example.com"),
        &host("api.example.com")
    ));
    // `**` allein deckt jeden Namen, aber es bleibt bei mindestens einem Label.
    assert!(glob_matches(&pattern("**"), &host("example.com")));
}
