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
