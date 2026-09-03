//! Die Auswertung: Reihenfolge, Gültigkeit, Upgrade, unbekannte Methode.
//!
//! Der Default ist `ask`. Jeder Test hier prüft am Ende dasselbe: dass keine
//! Anfrage ohne passende, gültige Regel automatisch durchgeht.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::{Duration, TimeZone, Utc};
use humanitl_core::rule::{Action, Expiry, HostPattern, Matcher, PathPattern};
use humanitl_core::{HostName, Method, Rule, RuleId, Scheme, SessionId, Upgrade};
use humanitl_rules::{RequestKey, RuleSet, Verdict, parse_rules};

fn host(text: &str) -> HostName {
    HostName::parse(text).unwrap_or_else(|err| panic!("host {text:?}: {err}"))
}

fn pattern(text: &str) -> HostPattern {
    match humanitl_rules::host::parse_pattern(text) {
        Ok((pattern, _)) => pattern,
        Err(diagnostic) => panic!("pattern {text:?}: {}", diagnostic.why),
    }
}

fn rule(action: Action, host_pattern: &str) -> Rule {
    Rule::new(RuleId::new(), action, Matcher::host(pattern(host_pattern)))
}

fn now() -> chrono::DateTime<Utc> {
    match Utc.with_ymd_and_hms(2026, 9, 3, 10, 0, 0).single() {
        Some(stamp) => stamp,
        None => panic!("fixed timestamp must exist"),
    }
}

#[test]
fn first_match_wins() {
    let block = rule(Action::Block, "**.github.com");
    let allow = rule(Action::Allow, "api.github.com");
    let blocked = block.id;
    let rules = RuleSet::from_rules([block, allow]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: blocked,
            action: Action::Block
        }
    );
}

#[test]
fn session_scoped() {
    let session = SessionId::new();
    let other = SessionId::new();
    let allow = rule(Action::Allow, "api.github.com").with_expiry(Expiry::Session(session));
    let id = allow.id;
    let rules = RuleSet::from_rules([allow]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), session),
        Verdict::Matched {
            rule: id,
            action: Action::Allow
        }
    );
    assert_eq!(rules.evaluate(&key, now(), other), Verdict::Default);
}

#[test]
fn a_session_rule_wins_over_a_persistent_one() {
    // CONVENTIONS 4.5: was der Mensch gerade entschieden hat, gilt sofort,
    // auch wenn eine ältere, breitere Regel in der Datei darüber steht.
    let session = SessionId::new();
    let block = rule(Action::Block, "**.github.com");
    let allow = rule(Action::Allow, "api.github.com").with_expiry(Expiry::Session(session));
    let allowed = allow.id;
    let rules = RuleSet::from_rules([block, allow]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), session),
        Verdict::Matched {
            rule: allowed,
            action: Action::Allow
        }
    );
}

#[test]
fn expired_at() {
    let past =
        rule(Action::Allow, "api.github.com").with_expiry(Expiry::At(now() - Duration::seconds(1)));
    let rules = RuleSet::from_rules([past]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );
}

#[test]
fn upgrade_dimension() {
    let session = SessionId::new();
    let plain = rule(Action::Allow, "*.github.com");
    let plain_id = plain.id;
    let socket = Rule::new(
        RuleId::new(),
        Action::Allow,
        Matcher::host(pattern("*.github.com")).with_upgrade(Upgrade::WebSocket),
    );
    let socket_id = socket.id;
    let rules = RuleSet::from_rules([plain, socket]);

    let target = host("api.github.com");
    let plain_key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    let upgrade_key = plain_key.with_upgrade(Upgrade::WebSocket);

    assert_eq!(
        rules.evaluate(&plain_key, now(), session),
        Verdict::Matched {
            rule: plain_id,
            action: Action::Allow
        }
    );
    assert_eq!(
        rules.evaluate(&upgrade_key, now(), session),
        Verdict::Matched {
            rule: socket_id,
            action: Action::Allow
        }
    );

    // Ohne die Upgrade-Regel bleibt der WebSocket eine Frage an den Menschen.
    let only_plain = RuleSet::from_rules([rule(Action::Allow, "*.github.com")]);
    assert_eq!(
        only_plain.evaluate(&upgrade_key, now(), session),
        Verdict::Default
    );
}

#[test]
fn unknown_method() {
    let rules = RuleSet::from_rules([rule(Action::Allow, "**")]);
    let target = host("api.github.com");
    let Ok(method) = Method::from_bytes(b"BREW") else {
        panic!("BREW is a syntactically valid method token");
    };
    let key = RequestKey::new(&target, &method, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );

    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert!(matches!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched { .. }
    ));
}

#[test]
fn path_without_query() {
    let mut matcher = Matcher::host(pattern("api.github.com"));
    matcher.path = Some(PathPattern::parse("/search"));
    let allow = Rule::new(RuleId::new(), Action::Allow, matcher);
    let id = allow.id;
    let rules = RuleSet::from_rules([allow]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/search?q=x", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: id,
            action: Action::Allow
        }
    );
}

#[test]
fn redact_returned() {
    let redact = rule(Action::Redact, "api.openai.com");
    let id = redact.id;
    let rules = RuleSet::from_rules([redact]);

    let target = host("api.openai.com");
    let key = RequestKey::new(&target, &Method::POST, "/v1/chat", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: id,
            action: Action::Redact
        }
    );
}

#[test]
fn scheme_and_method_narrow_a_rule() {
    let mut matcher = Matcher::host(pattern("api.github.com"));
    matcher.methods = Some(vec![Method::GET, Method::HEAD]);
    matcher.scheme = Some(Scheme::Https);
    let allow = Rule::new(RuleId::new(), Action::Allow, matcher);
    let rules = RuleSet::from_rules([allow]);

    let target = host("api.github.com");
    let session = SessionId::new();
    assert!(matches!(
        rules.evaluate(
            &RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443),
            now(),
            session
        ),
        Verdict::Matched { .. }
    ));
    assert_eq!(
        rules.evaluate(
            &RequestKey::new(&target, &Method::POST, "/", Scheme::Https, 443),
            now(),
            session
        ),
        Verdict::Default
    );
    assert_eq!(
        rules.evaluate(
            &RequestKey::new(&target, &Method::GET, "/", Scheme::Http, 443),
            now(),
            session
        ),
        Verdict::Default
    );
}

#[test]
fn an_empty_rule_set_asks() {
    let rules = RuleSet::new();
    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()).action(),
        Action::Ask
    );
}

#[test]
fn insert_remove_update_and_reorder_keep_the_order() {
    let mut rules = RuleSet::new();
    let first = rules.insert(None, rule(Action::Block, "a.example"));
    let second = rules.insert(None, rule(Action::Block, "b.example"));
    let third = rules.insert(Some(0), rule(Action::Block, "c.example"));
    assert_eq!(
        rules.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![third, first, second]
    );

    rules
        .reorder(third, 2)
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(
        rules.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![first, second, third]
    );

    let Some(existing) = rules.get(first).cloned() else {
        panic!("the rule is there");
    };
    let mut changed = existing;
    changed.action = Action::Allow;
    rules.update(changed).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(rules.get(first).map(|r| r.action), Some(Action::Allow));

    let Some(removed) = rules.remove(second) else {
        panic!("the rule is there");
    };
    assert_eq!(removed.id, second);
    assert_eq!(rules.len(), 2);
    assert!(rules.get(second).is_none());

    let unknown = rule(Action::Allow, "d.example");
    assert!(rules.update(unknown).is_err());
    assert!(rules.reorder(RuleId::new(), 0).is_err());
}

#[test]
fn prune_removes_what_no_longer_holds() {
    let session = SessionId::new();
    let other = SessionId::new();

    let keep = rule(Action::Allow, "a.example");
    let mine = rule(Action::Allow, "b.example").with_expiry(Expiry::Session(session));
    let foreign = rule(Action::Allow, "c.example").with_expiry(Expiry::Session(other));
    let past =
        rule(Action::Allow, "d.example").with_expiry(Expiry::At(now() - Duration::seconds(1)));
    let future =
        rule(Action::Allow, "e.example").with_expiry(Expiry::At(now() + Duration::seconds(1)));

    let (keep_id, mine_id, foreign_id, past_id, future_id) =
        (keep.id, mine.id, foreign.id, past.id, future.id);
    let mut rules = RuleSet::from_rules([keep, mine, foreign, past, future]);

    let removed = rules.prune(now(), session);
    assert_eq!(removed, vec![foreign_id, past_id]);
    assert_eq!(
        rules.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![keep_id, mine_id, future_id]
    );
}

#[test]
fn a_rule_set_from_yaml_decides_the_same_way() {
    let yaml = "version: 1
rules:
  - action: block
    match:
      host: \"**.github.com\"
  - action: allow
    match:
      host: \"api.github.com\"
";
    let (rules, warnings) =
        parse_rules(yaml).unwrap_or_else(|diagnostics| panic!("must parse: {diagnostics:?}"));
    assert!(warnings.is_empty());

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()).action(),
        Action::Block
    );
}

/// Eigenschaft: `*.X` trifft einen Host genau dann, wenn vor `X` genau ein
/// Label steht.
///
/// Statt `proptest` (steht nicht in `[workspace.dependencies]`) läuft hier ein
/// eigener, deterministischer Generator: dieselbe Folge in jedem Lauf, damit
/// ein Gegenbeispiel reproduzierbar ist, und ohne neue Abhängigkeit.
#[test]
fn property_one_star_means_exactly_one_label() {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789-";
    let mut state: u64 = 0x2026_0903_0022_0001;
    let mut next = move || {
        // Xorshift; der Wert selbst ist gleichgültig, die Wiederholbarkeit nicht.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let suffix = "example.com";
    let pattern = pattern("*.example.com");

    for _ in 0..500 {
        let count = 1 + usize::try_from(next() % 3).unwrap_or(0);
        let mut labels = Vec::with_capacity(count);
        for _ in 0..count {
            let len = 1 + usize::try_from(next() % 10).unwrap_or(0);
            let mut label = String::with_capacity(len);
            for _ in 0..len {
                let index = usize::try_from(next() % ALPHABET.len() as u64).unwrap_or(0);
                label.push(char::from(ALPHABET[index]));
            }
            // Ein Label darf weder mit `-` beginnen noch enden; solche Namen
            // lehnt `HostName::parse` ab, und sie sagen hier nichts aus.
            let label = label.trim_matches('-').to_owned();
            if label.is_empty() {
                labels.push("a".to_owned());
            } else {
                labels.push(label);
            }
        }

        let raw = format!("{}.{suffix}", labels.join("."));
        let Ok(parsed) = HostName::parse(&raw) else {
            continue;
        };
        let expected = labels.len() == 1;
        assert_eq!(
            humanitl_rules::host_matches(&pattern, &parsed),
            expected,
            "host {raw}"
        );
    }
}
