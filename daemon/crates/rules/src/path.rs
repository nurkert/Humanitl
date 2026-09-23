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
/// Diese Funktion vergleicht nur Zeichen. Was ein Pfad mit einem `..`-Segment
/// für eine Regel bedeutet, entscheidet allein die Auswertung in
/// [`crate::eval`] (HUM-204): Eine Regel, die durchlässt, trifft ihn nie; eine
/// Regel, die blockt oder fragt, prüft zusätzlich den aufgelösten Pfad aus
/// `normalize_path`. Wer ein Präfix aus einem Pfad ableiten will, fragt
/// vorher [`has_dot_dot_segment`].
#[must_use]
pub fn prefix_matches(prefixes: &[String], path_and_query: &str) -> bool {
    let path = strip_query(path_and_query);
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// Wahr, wenn der Pfad ein `..`-Segment enthält, auch verschleiert.
///
/// `/api/chat/../pull` beginnt zwar mit `/api/chat`, meint aber `/api/pull`,
/// und der Server dahinter löst das auf, bevor er antwortet. Geprüft wird auf
/// einer Kopie, in der nur `%2e` zu `.` und `%2f`, `%5c` sowie `\` zu `/`
/// werden, damit die verschleierten Schreibweisen dieselbe Antwort bekommen.
/// Eine doppelte Kodierung (`%252e`) bleibt kodiert.
///
/// Ein Segment zählt auch dann als `..`, wenn ihm Pfadparameter nach `;`
/// folgen: Tomcat und Spring lesen `/a/..;x/b` als `/b` (HUM-204).
///
/// Übergeben wird der Pfad ohne Query.
#[must_use]
pub fn has_dot_dot_segment(path: &str) -> bool {
    decode_dot_forms(path)
        .split('/')
        .any(|segment| without_parameters(segment) == "..")
}

/// Der Pfad so, wie ein Server ihn nach dem Auflösen der Punktsegmente sieht.
///
/// Vier Schritte, in dieser Reihenfolge, auf dem Pfad ohne Query:
///
/// 1. Einmal entschlüsselt werden die Punkt- und Trennerformen wie in
///    [`decode_dot_forms`] und zusätzlich die nicht reservierten Zeichen nach
///    RFC 3986, Abschnitt 6.2.2.2 (Buchstaben, Ziffern, `-._~`): `/%61dmin`
///    wird zu `/admin`. Eine doppelte Kodierung (`%252e`) bleibt `%252e`.
/// 2. In jedem Segment fallen Pfadparameter ab dem ersten `;` weg, wie Tomcat
///    es tut.
/// 3. Leere Segmente fallen weg, wie bei nginx (`merge_slashes`), Tomcat und
///    Spring: `//admin` wird zu `/admin`. Ein abschließender `/` bleibt.
/// 4. `remove_dot_segments` nach RFC 3986, Abschnitt 5.2.4.
///
/// Das Ergebnis dient nur dem zusätzlichen Vergleich einer Regel, die blockt
/// oder fragt. Trifft es mehr als der Server, blockt oder fragt die Regel
/// öfter; eine Freigabe entsteht daraus nie. Weitergeleitet wird immer der
/// unveränderte Pfad.
#[must_use]
pub(crate) fn normalize_path(path: &str) -> String {
    let decoded = decode(path, Decode::WithUnreserved);
    let (absolute, rest) = match decoded.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, decoded.as_str()),
    };
    let mut kept: Vec<&str> = Vec::new();
    let mut ends_in_directory = false;
    for segment in rest.split('/').map(without_parameters) {
        ends_in_directory = matches!(segment, "" | "." | "..");
        match segment {
            "" | "." => {}
            ".." => {
                kept.pop();
            }
            other => kept.push(other),
        }
    }
    let mut normalized = String::with_capacity(decoded.len());
    if absolute {
        normalized.push('/');
    }
    normalized.push_str(&kept.join("/"));
    if ends_in_directory && !kept.is_empty() {
        normalized.push('/');
    }
    normalized
}

/// Ein Segment ohne seine Pfadparameter: alles bis zum ersten `;`.
fn without_parameters(segment: &str) -> &str {
    match segment.split_once(';') {
        Some((name, _)) => name,
        None => segment,
    }
}

/// Eine Kopie des Pfads, in der nur die Punkt- und Trennerformen entschlüsselt
/// sind.
///
/// `%2e` wird zu `.`, `%2f` und `%5c` werden zu `/`, ein `\\` ebenso, jeweils
/// ohne Rücksicht auf Groß- und Kleinschreibung. Alles andere bleibt stehen;
/// das ist keine allgemeine Prozent-Dekodierung. Eine doppelte Kodierung
/// (`%252e`) wird dabei zu keinem Punkt — richtig so, denn auch der Server
/// dahinter dekodiert nur einmal.
fn decode_dot_forms(path: &str) -> String {
    decode(path, Decode::DotFormsOnly)
}

/// Entschlüsselt einmal die Punkt- und Trennerformen, mit `unreserved` auch
/// die nicht reservierten Zeichen nach RFC 3986, Abschnitt 6.2.2.2.
/// Was [`decode`] außer den Punkt- und Schrägstrichformen noch entschlüsselt.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Decode {
    /// Nur `%2e`, `%2f`, `%5c` und `\\`: die Formen, die ein Segment zu `..` machen.
    DotFormsOnly,
    /// Dazu einmal die nicht reservierten Zeichen nach RFC 3986 6.2.2.2.
    WithUnreserved,
}

fn decode(path: &str, mode: Decode) -> String {
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
            if mode == Decode::WithUnreserved
                && let Ok(byte) = u8::from_str_radix(&rest[1..3], 16)
                && (byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'~'))
            {
                decoded.push(char::from(byte));
                rest = &rest[3..];
                continue;
            }
        }
        decoded.push(if character == '\\' { '/' } else { character });
        rest = &rest[character.len_utf8()..];
    }
    decoded
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{has_dot_dot_segment, normalize_path, prefix_matches};

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

    /// Die verschleierten Schreibweisen eines `..`-Segments. Dass eine
    /// durchlassende Regel sie nie trifft, entscheidet die Auswertung
    /// (`tests/eval.rs`); hier steht, dass die Erkennung sie alle sieht.
    #[test]
    fn a_dot_dot_segment_is_seen_in_every_spelling() {
        for path in [
            "/api/chat/../pull",
            "/api/chat/%2e%2e/pull",
            "/api/chat%2f..%2fpull",
            "/api/chat/..%5cpull",
            "/api/chat\\..\\pull",
            "/api/chat/..;x/pull",
            "/v1/../admin",
        ] {
            assert!(has_dot_dot_segment(path), "{path} leaves its prefix");
        }
        assert!(
            !has_dot_dot_segment("/v1/a..b"),
            "two dots inside a segment are just characters"
        );
        assert!(
            !has_dot_dot_segment("/v1/%252e%252e/x"),
            "a double encoding stays encoded for the server as well"
        );
    }

    /// `normalize_path` löst auf wie ein Server: Punkt- und Trennerformen
    /// entschlüsselt, Pfadparameter weg, dann RFC 3986 5.2.4.
    #[test]
    fn normalize_path_resolves_like_the_server() {
        for (path, expected) in [
            ("/x/../admin", "/admin"),
            ("/x/%2E%2E/admin", "/admin"),
            ("/x/%2e%2e%2fadmin", "/admin"),
            ("/x\\..\\admin", "/admin"),
            ("/repos/me/../../user/keys", "/user/keys"),
            ("/repos/me/..;x/..;y/user/keys", "/user/keys"),
            ("/admin;jsessionid=1/x", "/admin/x"),
            ("/a/./b/.", "/a/b/"),
            ("/a/b/..", "/a/"),
            ("/../../etc/passwd", "/etc/passwd"),
            ("/..", "/"),
            ("/", "/"),
            ("/a//b/", "/a/b/"),
            ("//admin", "/admin"),
            ("/x/..//admin", "/admin"),
            ("/;x/admin", "/admin"),
            ("/%61dmin", "/admin"),
            ("/%41DMIN/%7Eu%2dx%5F1", "/ADMIN/~u-x_1"),
            ("/%2Fadmin", "/admin"),
            ("/a%20b/%3Fq", "/a%20b/%3Fq"),
            ("/v1/%252e%252e/admin", "/v1/%252e%252e/admin"),
        ] {
            assert_eq!(normalize_path(path), expected, "{path}");
        }
    }

    /// Die Pfade aus dem Befund HUM-204: Ein Glob `/repos/me/**` trifft sie
    /// Zeichen für Zeichen, also muss die Punkt-Erkennung sie sehen, damit
    /// die Auswertung eine Freigabe verweigert.
    #[test]
    fn the_paths_of_the_glob_finding_carry_a_dot_dot_segment() {
        for path in [
            "/repos/me/../../user/keys",
            "/repos/me/%2e%2e/%2e%2e/user/keys",
            "/repos/me/%2E%2e/user/keys",
            "/repos/me/.%2e/user/keys",
            "/repos/me/..;/..;/user/keys",
            "/repos/me/..",
        ] {
            assert!(has_dot_dot_segment(path), "{path} leaves /repos/me/");
        }
        for path in [
            "/repos/me/x",
            "/repos/me/a..b",
            "/repos/me/./x",
            "/repos/me/...",
            "/repos/me/%252e%252e/x",
            "/repos/me/;..",
        ] {
            assert!(!has_dot_dot_segment(path), "{path} stays inside");
        }
    }
}
