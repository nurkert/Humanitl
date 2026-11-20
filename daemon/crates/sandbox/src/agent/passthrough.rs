//! The passthrough to the language model as rules, one per set of methods
//! (HUM-568).
//!
//! A [`Matcher`] holds one list of methods for all of its paths. An agent
//! that posts its inference to one path and lists its models with `GET` on
//! another would, with one rule, also pass `GET` to the inference path and
//! `POST` to the model list. Each adapter therefore names its passthrough as
//! [`Route`]s, each with the methods and exactly the paths it needs them on,
//! and [`rules`] turns every route into a rule of its own with the same host,
//! scheme, port, rank and note.
//!
//! The ids are fixed ([`PASSTHROUGH_RULE_IDS`]): the first route keeps the id
//! the single passthrough rule has had since HUM-039, so the inference rule
//! of an endpoint is still `…00ff`, and each further route takes the next
//! id downwards. A route without any path condition becomes no rule, because
//! it would pass every path of the host unheld.

use humanitl_core::rule::{Action, HostPattern, Matcher, PathPattern, Rule};
use humanitl_core::{HostName, Method, RuleId, Scheme};
use uuid::Uuid;

/// The fixed ids of the passthrough rules, in the order of the routes.
///
/// The first is the id of the one passthrough rule before HUM-568. No adapter
/// needs more than three routes; a fourth would become no rule and be held.
pub const PASSTHROUGH_RULE_IDS: [u128; 3] = [
    0x0192_0000_0000_7000_8000_0000_0000_00ff,
    0x0192_0000_0000_7000_8000_0000_0000_00fe,
    0x0192_0000_0000_7000_8000_0000_0000_00fd,
];

/// One part of a passthrough: the methods and the paths they pass on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The methods passed on [`Route::paths`] and nowhere else.
    pub methods: Vec<Method>,
    /// Entries of [`Matcher::path_prefixes`], exact entries
    /// ([`humanitl_core::exact_path_entry`]) or prefixes.
    pub paths: Vec<String>,
    /// A path pattern that must match as well, for a route whose paths carry
    /// a variable part (the model in Gemini's `models/{model}:generateContent`).
    pub pattern: Option<PathPattern>,
}

impl Route {
    /// A route for these methods on these path entries.
    #[must_use]
    pub fn new(methods: &[Method], paths: Vec<String>) -> Self {
        Self {
            methods: methods.to_vec(),
            paths,
            pattern: None,
        }
    }

    /// Restricts the route further to this path pattern.
    #[must_use]
    pub fn with_pattern(mut self, pattern: PathPattern) -> Self {
        self.pattern = Some(pattern);
        self
    }

    /// Whether the route names a method and a path condition; one without
    /// either becomes no rule.
    fn is_bounded(&self) -> bool {
        let has_path = !self.paths.is_empty() || self.pattern.is_some();
        !self.methods.is_empty() && has_path
    }
}

/// Where the passthrough leads, and what every one of its rules carries.
#[derive(Debug, Clone)]
pub struct Target {
    /// The one host.
    pub host: HostName,
    /// Its scheme.
    pub scheme: Scheme,
    /// Its port.
    pub port: u16,
    /// Whether the host may resolve to a private address: for an own
    /// endpoint, which usually stands in the own network, never for a
    /// vendor's API.
    pub allow_private: bool,
    /// The note every rule carries, for the rules screen.
    pub note: String,
}

/// The passthrough rules of these routes towards `target`, in the order of
/// the routes, each with its id from [`PASSTHROUGH_RULE_IDS`].
///
/// A route without a method or without a path condition is left out and
/// keeps its id unused, so the ids of the others do not depend on it.
#[must_use]
pub fn rules(target: &Target, routes: Vec<Route>) -> Vec<Rule> {
    routes
        .into_iter()
        .zip(PASSTHROUGH_RULE_IDS)
        .filter(|(route, _)| route.is_bounded())
        .map(|(route, id)| {
            let mut matcher = Matcher::host(HostPattern::Exact(target.host.clone()))
                .with_methods(route.methods)
                .with_path_prefixes(route.paths)
                .with_scheme(target.scheme)
                .with_port(target.port);
            if let Some(pattern) = route.pattern {
                matcher = matcher.with_path(pattern);
            }
            Rule::new(
                RuleId::from_uuid(Uuid::from_u128(id)),
                Action::Allow,
                matcher,
            )
            .with_allow_private(target.allow_private)
            .bundled(true)
            // This mark makes the rule the declared exception: the proxy does
            // not hold such flows, records them in full and warns about
            // findings instead of stopping them (`LLM_005`).
            .passthrough_llm(true)
            .with_note(target.note.clone())
        })
        .collect()
}

/// `text` as a literal in a regular expression of a [`PathPattern::Regex`]:
/// every meta character of the `regex` crate behind a backslash, as
/// `regex::escape` does, which this crate does not depend on.
#[must_use]
pub fn regex_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '.'
                | '+'
                | '*'
                | '?'
                | '('
                | ')'
                | '|'
                | '['
                | ']'
                | '{'
                | '}'
                | '^'
                | '$'
                | '#'
                | '&'
                | '-'
                | '~'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use humanitl_core::rule::PathPattern;
    use humanitl_core::{HostName, Method, Scheme};

    use super::{PASSTHROUGH_RULE_IDS, Route, Target, regex_literal, rules};

    fn target() -> Target {
        Target {
            host: HostName::parse("api.example").unwrap(),
            scheme: Scheme::Https,
            port: 443,
            allow_private: false,
            note: "n".to_owned(),
        }
    }

    /// Each route becomes its own rule with its own methods and paths; an
    /// unbounded route becomes none and keeps its id unused.
    #[test]
    fn every_bounded_route_becomes_a_rule_of_its_own() {
        let built = rules(
            &target(),
            vec![
                Route::new(&[Method::POST], vec!["/a?".to_owned()]),
                Route::new(&[Method::GET], Vec::new()),
                Route::new(&[Method::GET], vec!["/b?".to_owned()]),
            ],
        );
        assert_eq!(built.len(), 2);
        assert_eq!(built[0].id.as_uuid().as_u128(), PASSTHROUGH_RULE_IDS[0]);
        assert_eq!(built[0].matcher.methods, Some(vec![Method::POST]));
        assert_eq!(built[0].matcher.path_prefixes, ["/a?"]);
        assert_eq!(built[1].id.as_uuid().as_u128(), PASSTHROUGH_RULE_IDS[2]);
        assert_eq!(built[1].matcher.methods, Some(vec![Method::GET]));
        assert_eq!(built[1].matcher.path_prefixes, ["/b?"]);
        assert!(
            built
                .iter()
                .all(|rule| rule.passthrough_llm && rule.bundled)
        );
        // A pattern alone is a path condition.
        let pattern = rules(
            &target(),
            vec![
                Route::new(&[Method::POST], Vec::new())
                    .with_pattern(PathPattern::Regex("^/x$".to_owned())),
            ],
        );
        assert_eq!(pattern.len(), 1);
        // No method, no rule.
        assert!(rules(&target(), vec![Route::new(&[], vec!["/a?".to_owned()])]).is_empty());
    }

    /// The ids are distinct.
    #[test]
    fn the_ids_are_distinct() {
        let mut ids = PASSTHROUGH_RULE_IDS.to_vec();
        ids.dedup();
        assert_eq!(ids.len(), PASSTHROUGH_RULE_IDS.len());
    }

    #[test]
    fn a_regex_literal_escapes_every_meta_character() {
        assert_eq!(regex_literal("/a.b-c"), "/a\\.b\\-c");
        assert_eq!(regex_literal("/x[1]{2}*"), "/x\\[1\\]\\{2\\}\\*");
        assert_eq!(regex_literal("/plain/path"), "/plain/path");
    }
}
