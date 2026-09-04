//! Pfadmuster: Glob oder regulärer Ausdruck, beide mit Grenzen.
//!
//! Verglichen wird nur der Pfad, nie die Query: `/search` trifft
//! `/search?q=x`, und niemand kann eine Regel mit einem angehängten
//! `?a=b` umgehen oder erweitern.
//!
//! Der Glob läuft mit `literal_separator(true)`. Ohne das liefe `*` über `/`
//! hinweg, und `/repos/*` träfe `/repos/a/b` — eine Regel wäre dann breiter
//! als sie aussieht. Der reguläre Ausdruck läuft über die Crate `regex`
//! (endlicher Automat, kein Backtracking) mit einer Größengrenze, damit ein
//! Muster aus einer Regel-Datei nicht den Speicher des Daemons aufbraucht.

use globset::{Glob, GlobBuilder, GlobMatcher};
use humanitl_core::diagnostics::codes::RULES_005;
use humanitl_core::rule::PathPattern;
use humanitl_core::{Diagnostic, Severity};
use regex::{Regex, RegexBuilder};

/// Obergrenze für den übersetzten Ausdruck und seinen Automaten, je 1 MiB.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

/// Ein übersetztes Pfadmuster.
///
/// Übersetzt wird einmal beim Laden des Regelsatzes, nicht bei jeder Anfrage:
/// ein regulärer Ausdruck pro Anfrage neu zu bauen wäre teurer als der
/// Vergleich selbst.
#[derive(Debug, Clone)]
pub enum PathMatcher {
    /// Glob über den Pfad; `*` kreuzt kein `/`, `**` schon.
    Glob(Box<GlobMatcher>),
    /// Regulärer Ausdruck, ungebunden: er trifft, wenn er irgendwo im Pfad
    /// passt. Wer den Anfang meint, schreibt `^`.
    Regex(Box<Regex>),
}

impl PathMatcher {
    /// Übersetzt ein Muster aus einer Regel.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`] mit `RULES_005`, wenn der Glob kein gültiges Muster
    /// oder der reguläre Ausdruck ungültig beziehungsweise zu groß ist.
    pub fn compile(pattern: &PathPattern) -> Result<Self, Diagnostic> {
        match pattern {
            PathPattern::Glob(glob) => Self::glob(glob),
            PathPattern::Regex(regex) => Self::regex(regex),
        }
    }

    fn glob(pattern: &str) -> Result<Self, Diagnostic> {
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map(|glob: Glob| Self::Glob(Box::new(glob.compile_matcher())))
            .map_err(|err| {
                Diagnostic::builder(RULES_005, Severity::Error)
                    .why(format!("path glob {pattern:?} is invalid: {err}"))
                    .build()
            })
    }

    fn regex(pattern: &str) -> Result<Self, Diagnostic> {
        RegexBuilder::new(pattern)
            .size_limit(REGEX_SIZE_LIMIT)
            .dfa_size_limit(REGEX_SIZE_LIMIT)
            .build()
            .map(|regex| Self::Regex(Box::new(regex)))
            .map_err(|err| {
                Diagnostic::builder(RULES_005, Severity::Error)
                    .why(format!(
                        "path regex ~{pattern} is invalid or too large: {err}"
                    ))
                    .build()
            })
    }

    /// Wahr, wenn das Muster den Pfad trifft.
    ///
    /// Eine Query wird vorher abgeschnitten; der Aufrufer darf also den
    /// vollständigen `path_and_query` übergeben.
    #[must_use]
    pub fn matches(&self, path_and_query: &str) -> bool {
        let path = strip_query(path_and_query);
        match self {
            Self::Glob(glob) => glob.is_match(path),
            Self::Regex(regex) => regex.is_match(path),
        }
    }
}

/// Der Pfad ohne Query: alles bis zum ersten `?`.
#[must_use]
pub fn strip_query(path_and_query: &str) -> &str {
    match path_and_query.split_once('?') {
        Some((path, _)) => path,
        None => path_and_query,
    }
}

/// Wahr, wenn der Pfad mit einem der Präfixe beginnt.
///
/// Verglichen wird nur der Pfad, nie die Query — wie bei [`PathMatcher`].
/// `prefixes` ist bereits geprüft: Jeder Eintrag hat
/// [`path_prefix_is_valid`](humanitl_core::path_prefix_is_valid) bestanden.
/// Eine leere Liste ist hier kein „egal", sondern ein „nichts": Diese Funktion
/// wird nur gerufen, wenn eine Regel überhaupt Präfixe trägt, und eine Regel
/// mit einer Bedingung, die niemand erfüllen kann, trifft nichts. Der Aufrufer
/// in [`crate::eval`] unterscheidet beides.
///
/// # Punkt-Segmente
///
/// Ein Pfad mit einem `..`-Segment trifft nie ein Präfix. `/api/chat/../pull`
/// beginnt zwar mit `/api/chat`, meint aber `/api/pull`, und der Server dahinter
/// löst das auf, bevor er antwortet. Die Präfix-Bedingung ist die Grenze der
/// Durchreichregel (HUM-039); ein Pfad, dessen Ziel erst nach einer
/// Normalisierung feststeht, darf sie nicht überschreiten. Geprüft wird auf
/// einer Kopie, in der `%2e` zu `.` und `%2f`, `%5c` sowie `\` zu `/` werden,
/// damit die verschleierten Schreibweisen dieselbe Antwort bekommen. Der
/// Vergleich selbst läuft danach wieder auf dem unveränderten Pfad: Die Regel
/// entscheidet über den Pfad, der wirklich hinausgeht.
#[must_use]
pub fn prefix_matches(prefixes: &[String], path_and_query: &str) -> bool {
    let path = strip_query(path_and_query);
    if has_dot_dot_segment(path) {
        return false;
    }
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// Wahr, wenn der Pfad ein `..`-Segment enthält, auch verschleiert.
///
/// Entschlüsselt wird genau so viel, wie für diese Frage nötig ist: `%2e` wird
/// zu `.`, `%2f` und `%5c` werden zu `/`, ein `\\` ebenso. Alles andere bleibt
/// stehen. Eine doppelte Kodierung (`%252e`) wird dabei zu `%2e` und damit zu
/// keinem Punkt — richtig so, denn auch der Server dahinter dekodiert nur
/// einmal.
fn has_dot_dot_segment(path: &str) -> bool {
    let mut decoded = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(character) = rest.chars().next() {
        if character == '%' && rest.len() >= 3 && rest.is_char_boundary(3) {
            match rest[..3].to_ascii_lowercase().as_str() {
                "%2e" => {
                    decoded.push('.');
                    rest = &rest[3..];
                    continue;
                }
                "%2f" | "%5c" => {
                    decoded.push('/');
                    rest = &rest[3..];
                    continue;
                }
                _ => {}
            }
        }
        decoded.push(if character == '\\' { '/' } else { character });
        rest = &rest[character.len_utf8()..];
    }
    decoded.split('/').any(|segment| segment == "..")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::prefix_matches;

    fn prefixes() -> Vec<String> {
        vec!["/v1/".to_owned(), "/api/chat".to_owned()]
    }

    #[test]
    fn a_prefix_matches_the_path_and_ignores_the_query() {
        assert!(prefix_matches(&prefixes(), "/v1/chat/completions"));
        assert!(prefix_matches(&prefixes(), "/api/chat?stream=true"));
        assert!(!prefix_matches(&prefixes(), "/api/pull"));
        assert!(!prefix_matches(&prefixes(), "/admin"));
        assert!(
            !prefix_matches(&[], "/v1/models"),
            "a rule whose prefixes all fell away matches nothing"
        );
    }

    #[test]
    fn a_dot_dot_segment_never_matches_a_prefix() {
        for path in [
            "/api/chat/../pull",
            "/api/chat/%2e%2e/pull",
            "/api/chat%2f..%2fpull",
            "/api/chat/..%5cpull",
            "/v1/../admin",
        ] {
            assert!(
                !prefix_matches(&prefixes(), path),
                "{path} would leave the boundary of the rule"
            );
        }
        assert!(
            prefix_matches(&prefixes(), "/v1/a..b"),
            "two dots inside a segment are just characters"
        );
        assert!(
            prefix_matches(&prefixes(), "/v1/%252e%252e/x"),
            "a double encoding stays encoded for the server as well"
        );
    }
}
