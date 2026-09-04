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

// --- Pfadpräfixe und die Durchreiche zum Sprachmodell (HUM-039) -------------

/// Baut die Bedingung der Durchreichregel: Host, Port, Methoden, Präfixe.
fn passthrough_matcher(prefixes: &[&str]) -> Matcher {
    Matcher::host(pattern("ip:192.168.1.50"))
        .with_methods(vec![Method::POST, Method::GET])
        .with_path_prefixes(prefixes.iter().map(|p| (*p).to_owned()).collect())
        .with_scheme(Scheme::Http)
        .with_port(11434)
}

/// Die Tabelle aus HUM-039: die Durchreiche trifft den Endpunkt und nichts
/// daneben.
#[test]
fn passthrough_rule_matches_only_endpoint() {
    let id = RuleId::new();
    let mut rule = Rule::new(
        id,
        Action::Allow,
        passthrough_matcher(&["/v1/", "/api/chat", "/api/tags"]),
    );
    rule = rule
        .with_allow_private(true)
        .bundled(true)
        .passthrough_llm(true);
    let rules = RuleSet::from_rules([rule]);

    let endpoint = host("192.168.1.50");
    let neighbour = host("192.168.1.51");
    for (target, method, path, port, allowed) in [
        (&endpoint, Method::POST, "/v1/chat/completions", 11434, true),
        (&endpoint, Method::GET, "/v1/models", 11434, true),
        (&endpoint, Method::POST, "/api/chat", 11434, true),
        (&endpoint, Method::POST, "/admin", 11434, false),
        (&endpoint, Method::POST, "/api/pull", 11434, false),
        (&endpoint, Method::POST, "/v1/x", 8080, false),
        (&neighbour, Method::POST, "/v1/x", 11434, false),
        (&endpoint, Method::DELETE, "/v1/x", 11434, false),
    ] {
        let key = RequestKey::new(target, &method, path, Scheme::Http, port);
        let verdict = rules.evaluate(&key, now(), SessionId::new());
        assert_eq!(
            matches!(
                verdict,
                Verdict::Matched {
                    action: Action::Allow,
                    ..
                }
            ),
            allowed,
            "{method} http://{target}:{port}{path} came out as {verdict:?}"
        );
    }
    assert!(rules.is_passthrough_llm(id));
    assert!(
        !rules.is_passthrough_llm(RuleId::new()),
        "a rule the set does not know is not a passthrough"
    );
}

/// Eine Regel, deren Präfixe alle unbrauchbar sind, trifft nichts.
///
/// Fail closed: Wer eine Grenze schreibt und dabei nur `""` und `/` hinschreibt,
/// bekommt keine Regel, die alles trifft.
#[test]
fn a_rule_whose_prefixes_are_all_unusable_matches_nothing() {
    let rules = RuleSet::from_rules([Rule::new(
        RuleId::new(),
        Action::Allow,
        passthrough_matcher(&["", "/"]),
    )]);
    let endpoint = host("192.168.1.50");
    let key = RequestKey::new(&endpoint, &Method::POST, "/v1/chat", Scheme::Http, 11434);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );
}

/// Pfadmuster und Präfixe stehen nebeneinander und schränken beide ein.
#[test]
fn a_path_pattern_and_prefixes_both_have_to_match() {
    let mut matcher = passthrough_matcher(&["/v1/"]);
    matcher.path = Some(PathPattern::Glob("/v1/chat/**".to_owned()));
    let rules = RuleSet::from_rules([Rule::new(RuleId::new(), Action::Allow, matcher)]);

    let endpoint = host("192.168.1.50");
    for (path, allowed) in [("/v1/chat/completions", true), ("/v1/models", false)] {
        let key = RequestKey::new(&endpoint, &Method::POST, path, Scheme::Http, 11434);
        let verdict = rules.evaluate(&key, now(), SessionId::new());
        assert_eq!(
            matches!(verdict, Verdict::Matched { .. }),
            allowed,
            "{path} came out as {verdict:?}"
        );
    }
}

// --- Abgeschaltete und mitgelieferte Regeln (HUM-038) -----------------------

/// Eine abgeschaltete Regel entscheidet nichts, und die nächste kommt dran.
///
/// Der Fall ist der Grund, warum es das Feld gibt: Wer eine mitgelieferte
/// Blockregel abschaltet, will die Anfrage sehen, nicht sie stillschweigend
/// erlaubt bekommen. Deshalb steht hinter der abgeschalteten Regel hier eine
/// zweite, und der Test hält fest, dass sie greift.
#[test]
fn a_disabled_rule_decides_nothing() {
    let off = rule(Action::Block, "**.github.com").disabled(true);
    let after = rule(Action::Ask, "api.github.com");
    let asked = after.id;
    let rules = RuleSet::from_rules([off, after]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: asked,
            action: Action::Ask
        }
    );
}

/// Ohne eine zweite Regel bleibt es bei der Vorgabe, also `ask`.
#[test]
fn the_last_disabled_rule_falls_through_to_ask() {
    let off = rule(Action::Allow, "api.github.com").disabled(true);
    let rules = RuleSet::from_rules([off]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );
}

/// `set_disabled` schaltet eine Regel ab und wieder an.
#[test]
fn set_disabled_toggles_a_rule_by_id() {
    let one = rule(Action::Block, "api.github.com");
    let id = one.id;
    let mut rules = RuleSet::from_rules([one]);

    let target = host("api.github.com");
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);

    rules
        .set_disabled(id, true)
        .expect("the rule is in the set");
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );

    rules
        .set_disabled(id, false)
        .expect("the rule is in the set");
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: id,
            action: Action::Block
        }
    );

    assert!(
        rules.set_disabled(RuleId::new(), true).is_err(),
        "an id no rule carries is an error, not a silent no-op"
    );
}

/// `add_bundled` erzwingt `bundled` und übernimmt die Abschaltliste.
///
/// Beides ist die Zusage des Aufrufs: Was auf diesem Weg hereinkommt, gehört
/// nicht dem Nutzer, auch wenn die Datei etwas anderes behauptet, und die
/// Entscheidung des Nutzers aus seiner `rules.yaml` wirkt auch dann, wenn er
/// sie vor dieser Fassung des Regelsatzes getroffen hat.
#[test]
fn add_bundled_forces_bundled_and_honours_the_disable_list() {
    let off = rule(Action::Block, "models.dev");
    let on = rule(Action::Block, "**.sentry.io");
    let (off_id, on_id) = (off.id, on.id);

    let mut rules = RuleSet::new();
    rules.set_disabled_bundled([off_id]);
    rules.add_bundled([off, on]);

    assert!(
        rules.iter().all(|rule| rule.bundled),
        "everything that comes in this way is bundled"
    );
    let disabled: Vec<bool> = rules.iter().map(|rule| rule.disabled).collect();
    assert_eq!(disabled, vec![true, false]);

    let blocked = host("eu.sentry.io");
    let key = RequestKey::new(&blocked, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: on_id,
            action: Action::Block
        }
    );

    let disabled_host = host("models.dev");
    let key = RequestKey::new(
        &disabled_host,
        &Method::GET,
        "/api.json",
        Scheme::Https,
        443,
    );
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Default
    );
}

/// Die mitgelieferten Regeln behalten ihre Reihenfolge und stehen hinten.
///
/// Hinten, damit eine eigene Regel des Nutzers eine mitgelieferte überstimmt
/// (HUM-027, `backlog/CONVENTIONS.md` 4.5): Löschen kann er sie nicht, also
/// muss er sie überschreiben können.
#[test]
fn add_bundled_keeps_its_order_behind_what_was_there() {
    let mine = rule(Action::Allow, "models.dev");
    let mine_id = mine.id;
    let first = rule(Action::Block, "models.dev");
    let second = rule(Action::Ask, "registry.npmjs.org");
    let (first_id, second_id) = (first.id, second.id);

    let mut rules = RuleSet::from_rules([mine]);
    rules.add_bundled([first, second]);

    let order: Vec<RuleId> = rules.iter().map(|rule| rule.id).collect();
    assert_eq!(order, vec![mine_id, first_id, second_id]);

    let contested = host("models.dev");
    let key = RequestKey::new(&contested, &Method::GET, "/api.json", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), SessionId::new()),
        Verdict::Matched {
            rule: mine_id,
            action: Action::Allow
        },
        "the rule of the user decides, the bundled one below it does not"
    );
}

/// Die Durchreiche trifft vor jeder anderen Regel, egal wo sie steht.
///
/// Das ist der Kern von HUM-104: Der Vorrang hängt an `passthrough_llm`, nicht
/// am Platz in der Liste. Steht die Durchreiche hinter einer Sitzungsregel und
/// einer Nutzerregel, die beide denselben Host treffen, entscheidet trotzdem
/// sie — sonst verlöre der eine erklärte Seitenkanal die Merkmale, an denen er
/// zu erkennen ist (`DecisionSource::Passthrough`, `LLM_005`).
#[test]
fn the_passthrough_decides_before_session_and_user_rules() {
    let session = SessionId::new();
    let user_block = rule(Action::Block, "**");
    let session_allow = rule(Action::Allow, "**").with_expiry(Expiry::Session(session));
    let passthrough = rule(Action::Allow, "ollama.lan").passthrough_llm(true);
    let passthrough_id = passthrough.id;

    // Die Durchreiche steht ganz hinten und in der Gruppe der mitgelieferten
    // Regeln — genau so, wie der Regelspeicher den Satz zusammensetzt.
    let mut rules = RuleSet::from_rules([session_allow, user_block]);
    rules.add_bundled([passthrough]);

    let llm = host("ollama.lan");
    let key = RequestKey::new(
        &llm,
        &Method::POST,
        "/v1/chat/completions",
        Scheme::Http,
        11434,
    );
    assert_eq!(
        rules.evaluate(&key, now(), session),
        Verdict::Matched {
            rule: passthrough_id,
            action: Action::Allow
        }
    );
    assert!(rules.is_passthrough_llm(passthrough_id));
}

/// Eine mitgelieferte Regel rueckt nicht dadurch vor, dass sie
/// sitzungsgebunden ist.
///
/// Rang 2 gehoert den Sitzungsregeln **des Nutzers** — "fuer diese Sitzung
/// erlauben" ist seine juengste Absicht. Eine mitgelieferte Regel mit
/// `expires: session` ist das nicht. Stuende sie trotzdem in Rang 2, ueberholte
/// sie die dauerhaften Regeln des Nutzers, und HUM-027 — der Nutzer ueberstimmt
/// eine mitgelieferte Regel — hinge daran, welche Gueltigkeit
/// `rules/default.yaml` gerade schreibt.
#[test]
fn a_session_scoped_bundled_rule_does_not_outrank_the_user() {
    let session = SessionId::new();
    let user_allow = rule(Action::Allow, "models.dev");
    let user_allow_id = user_allow.id;
    let bundled_block = rule(Action::Block, "models.dev").with_expiry(Expiry::Session(session));

    let mut rules = RuleSet::from_rules([user_allow]);
    rules.add_bundled([bundled_block]);

    let contested = host("models.dev");
    let key = RequestKey::new(&contested, &Method::GET, "/api.json", Scheme::Https, 443);
    assert_eq!(
        rules.evaluate(&key, now(), session),
        Verdict::Matched {
            rule: user_allow_id,
            action: Action::Allow
        },
        "the rule of the user decides; a bundled rule is bundled, session-scoped or not"
    );

    // Dieselbe Gueltigkeit an einer eigenen Regel gewinnt sehr wohl: Rang 2
    // gehoert dem Nutzer, und HUM-027 sagt ausdruecklich, dass eine
    // Sitzungsregel einen mitgelieferten Block ueberstimmen darf.
    let own_session = rule(Action::Block, "models.dev").with_expiry(Expiry::Session(session));
    let own_session_id = own_session.id;
    let user_allow = rule(Action::Allow, "models.dev");
    let mut rules = RuleSet::from_rules([user_allow, own_session]);
    rules.add_bundled([rule(Action::Allow, "models.dev")]);
    assert_eq!(
        rules.evaluate(&key, now(), session).rule(),
        Some(own_session_id),
        "his own session rule still comes first"
    );
}

/// Eine Durchreiche aus einer Datei bekommt den ersten Rang nicht.
///
/// `passthrough_llm` steht auch in der `rules.yaml` des Nutzers und in den
/// Inline-Regeln eines Profils. Waere das Feld allein der Rang, stellte sich
/// jede Datei den Rang selbst aus und ueberholte die eigenen Block-Regeln
/// ihres Verfassers — unbemerkt, denn eine Durchreiche wird nicht gehalten.
/// Den Rang gibt es deshalb nur mit dem Vermerk `bundled`, und den setzt
/// allein `add_bundled` (HUM-104, `backlog/CONVENTIONS.md` 4.5).
#[test]
fn a_passthrough_from_a_file_does_not_reach_the_first_rank() {
    let session = SessionId::new();
    let llm = host("ollama.lan");
    let key = RequestKey::new(
        &llm,
        &Method::POST,
        "/v1/chat/completions",
        Scheme::Http,
        11434,
    );

    // So, wie `parse_rules` sie liefert: `passthrough_llm`, aber nicht bundled.
    let from_file = rule(Action::Allow, "ollama.lan").passthrough_llm(true);
    let from_file_id = from_file.id;
    let user_block = rule(Action::Block, "**");
    let user_block_id = user_block.id;

    let rules = RuleSet::from_rules([user_block, from_file]);
    assert_eq!(
        rules.evaluate(&key, now(), session),
        Verdict::Matched {
            rule: user_block_id,
            action: Action::Block
        },
        "a file does not hand itself the first rank; list order decides"
    );
    assert!(
        rules.is_passthrough_llm(from_file_id),
        "the rule keeps what it is; only its rank is not its own to declare"
    );

    // Dieselbe Regel ueber den Lader: jetzt gilt der erste Rang.
    let mut loaded = RuleSet::from_rules(rules.iter().filter(|r| r.id == user_block_id).cloned());
    loaded.add_bundled(rules.iter().filter(|r| r.id == from_file_id).cloned());
    assert_eq!(
        loaded.evaluate(&key, now(), session),
        Verdict::Matched {
            rule: from_file_id,
            action: Action::Allow
        }
    );
}

/// Eine abgeschaltete Durchreiche entscheidet nichts, auch nicht als erste.
///
/// Der eigene Durchgang ist ein Vorrang, keine Ausnahme von den übrigen
/// Prüfungen: `disabled` und `expires` gelten dort wie überall.
#[test]
fn a_disabled_passthrough_does_not_decide() {
    let session = SessionId::new();
    let passthrough = rule(Action::Allow, "ollama.lan")
        .passthrough_llm(true)
        .bundled(true)
        .disabled(true);
    let rules = RuleSet::from_rules([passthrough]);

    let llm = host("ollama.lan");
    let key = RequestKey::new(
        &llm,
        &Method::POST,
        "/v1/chat/completions",
        Scheme::Http,
        11434,
    );
    assert_eq!(rules.evaluate(&key, now(), session), Verdict::Default);
}
