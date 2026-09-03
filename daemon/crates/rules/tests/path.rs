//! Pfadmuster: Glob mit `/`-Grenze, regulärer Ausdruck mit Größengrenze.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_core::diagnostics::codes::RULES_005;
use humanitl_core::rule::PathPattern;
use humanitl_rules::PathMatcher;

fn matcher(text: &str) -> PathMatcher {
    let pattern = PathPattern::parse(text);
    PathMatcher::compile(&pattern)
        .unwrap_or_else(|diagnostic| panic!("{text:?} must compile: {}", diagnostic.why))
}

#[test]
fn double_star_crosses_a_slash() {
    let glob = matcher("/repos/**");
    assert!(glob.matches("/repos/a/b"));
    assert!(glob.matches("/repos/a"));
}

#[test]
fn a_single_star_stops_at_the_slash() {
    // `literal_separator(true)`: ohne diese Einstellung wäre `/repos/*`
    // breiter, als es aussieht, und eine Regel für ein Verzeichnis erlaubte
    // den ganzen Baum darunter.
    let glob = matcher("/repos/*");
    assert!(glob.matches("/repos/a"));
    assert!(!glob.matches("/repos/a/b"));
}

#[test]
fn a_regex_starts_after_the_tilde() {
    let regex = matcher("~^/v[0-9]+/");
    assert!(regex.matches("/v2/x"));
    assert!(!regex.matches("/w2/x"));
}

#[test]
fn a_broken_regex_is_rules_005() {
    let pattern = PathPattern::parse("~^/v[0-9+/");
    let Err(diagnostic) = PathMatcher::compile(&pattern) else {
        panic!("an unclosed class is not a regex");
    };
    assert_eq!(diagnostic.code, RULES_005);
    assert!(diagnostic.why.contains("^/v[0-9+/"));
}

#[test]
fn a_regex_over_the_size_limit_is_rules_005() {
    // Ein Muster, dessen übersetzte Form die Grenze von 1 MiB reißt. Ohne die
    // Grenze könnte eine Regel-Datei den Speicher des Daemons aufbrauchen.
    let huge = format!("~{}", "a{1000}{1000}{100}");
    let pattern = PathPattern::parse(&huge);
    let Err(diagnostic) = PathMatcher::compile(&pattern) else {
        panic!("this regex is too large to compile");
    };
    assert_eq!(diagnostic.code, RULES_005);
}

#[test]
fn the_query_is_never_part_of_the_comparison() {
    let glob = matcher("/search");
    assert!(glob.matches("/search?q=x"));
    assert!(!glob.matches("/search/deep?q=x"));

    let regex = matcher("~^/search$");
    assert!(regex.matches("/search?q=x"));
}
