//! Escape-Test 4: die Regel-Tabelle (BACKLOG.md 4.5, Test 4).
//!
//! Jeder Test hier heißt genau wie eine Probe in `tests/escape/esc-4-rules.sh`;
//! das Skript ruft ihn mit `cargo test -- --exact <name>` auf und verbucht das
//! Ergebnis im Escape-Bericht. Der Umweg über die Test-Binärdatei hat einen
//! Grund: die Crate ist rein (kein IO), also gibt es kein Werkzeug, das eine
//! Regel-Datei von der Kommandozeile aus auswertet. Sobald `humanitl rules
//! test URL` existiert (HUM-065), kann das Skript zusätzlich den Weg nehmen,
//! den auch der Nutzer nimmt.
//!
//! Der Regelsatz steht in `tests/fixtures/esc4.yaml` und wird zur Bauzeit
//! eingebettet.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::{TimeZone, Utc};
use humanitl_core::diagnostics::codes::RULES_002;
use humanitl_core::rule::{Action, Expiry, Matcher};
use humanitl_core::{HostName, Method, Rule, RuleId, Scheme, SessionId, Upgrade};
use humanitl_rules::{RequestKey, RuleSet, Verdict, parse_rules_for_session};

const ESC4_RULES: &str = include_str!("fixtures/esc4.yaml");

fn now() -> chrono::DateTime<Utc> {
    match Utc.with_ymd_and_hms(2026, 9, 3, 10, 0, 0).single() {
        Some(stamp) => stamp,
        None => panic!("fixed timestamp must exist"),
    }
}

fn rules(session: SessionId) -> RuleSet {
    match parse_rules_for_session(ESC4_RULES, session) {
        Ok((set, warnings)) => {
            assert!(warnings.is_empty(), "the fixture is clean: {warnings:?}");
            set
        }
        Err(diagnostics) => panic!("the fixture must parse: {diagnostics:?}"),
    }
}

fn host(text: &str) -> HostName {
    HostName::parse(text).unwrap_or_else(|err| panic!("host {text:?}: {err}"))
}

/// Die Aktion für eine gewöhnliche GET-Anfrage an diesen Host.
fn action_for(set: &RuleSet, session: SessionId, raw_host: &str) -> Action {
    let target = host(raw_host);
    let key = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    set.evaluate(&key, now(), session).action()
}

#[test]
fn rule_table_first_match_wins() {
    let session = SessionId::new();
    let set = rules(session);
    let target = host("api.github.com");

    let post = RequestKey::new(&target, &Method::POST, "/", Scheme::Https, 443);
    assert_eq!(
        set.evaluate(&post, now(), session).action(),
        Action::Block,
        "the block rule stands first and wins"
    );

    let get = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(set.evaluate(&get, now(), session).action(), Action::Allow);
}

#[test]
fn rule_session_before_persistent() {
    // CONVENTIONS 4.5: eine Sitzungsregel wird vor jeder dauerhaften Regel
    // ausgewertet, auch wenn sie hinten in der Liste steht.
    let session = SessionId::new();
    let mut set = rules(session);
    let target = host("api.github.com");

    let block = Rule::new(
        RuleId::new(),
        Action::Block,
        Matcher::host(
            match humanitl_rules::host::parse_pattern("api.github.com") {
                Ok((pattern, _)) => pattern,
                Err(diagnostic) => panic!("{}", diagnostic.why),
            },
        ),
    )
    .with_expiry(Expiry::Session(session));
    let block_id = set.insert(None, block);

    let get = RequestKey::new(&target, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        set.evaluate(&get, now(), session),
        Verdict::Matched {
            rule: block_id,
            action: Action::Block
        },
        "the session rule decides, although it stands last"
    );

    // In einer anderen Sitzung gilt sie nicht.
    assert_eq!(
        action_for(&set, SessionId::new(), "api.github.com"),
        Action::Allow
    );
}

#[test]
fn rule_host_glob_labels() {
    let session = SessionId::new();
    let set = rules(session);

    assert_eq!(action_for(&set, session, "api.github.com"), Action::Allow);
    for host in [
        "github.com",         // `*` verlangt genau ein Label
        "a.b.github.com",     // und nicht zwei
        "evil-github.com",    // Labels, keine Teilzeichenketten
        "github.com.evil.io", // und kein Suffix-Spiel
        "notgithub.com",
    ] {
        assert_eq!(
            action_for(&set, session, host),
            Action::Ask,
            "{host} must not be matched"
        );
    }

    // Groß-/Kleinschreibung und abschließender Punkt ändern nichts.
    assert_eq!(action_for(&set, session, "API.GITHUB.COM."), Action::Allow);
}

#[test]
fn rule_homograph_host() {
    let session = SessionId::new();
    let set = rules(session);

    // Ein Name, der wie `github.com` aussieht, ist ein anderer Name: das `i`
    // ist hier ein kyrillisches `і`, und nach der Normalisierung steht dort
    // `xn--gthub-lcd.com`.
    assert_eq!(action_for(&set, session, "gіthub.com"), Action::Ask);
    assert_eq!(action_for(&set, session, "api.gіthub.com"), Action::Ask);
    assert_eq!(action_for(&set, session, "xn--gthub-lcd.com"), Action::Ask);

    // Und ein Punycode-Literal im Muster ist eine Warnung, damit niemand
    // versehentlich einen Namen erlaubt, den er nicht lesen kann.
    let (_, warnings) = match humanitl_rules::host::parse_pattern("xn--80ak6aa92e.com") {
        Ok(parsed) => parsed,
        Err(diagnostic) => panic!("{}", diagnostic.why),
    };
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, RULES_002);
}

#[test]
fn rule_ip_literal_host() {
    let session = SessionId::new();
    let set = rules(session);

    // Die Adresse trifft ihre `ip:`-Regel …
    assert_eq!(action_for(&set, session, "140.82.112.3"), Action::Block);
    assert_eq!(
        action_for(&set, session, "[::ffff:140.82.112.3]"),
        Action::Block,
        "the mapped form is the same address"
    );

    // … und keine andere Adresse und kein Namensmuster.
    assert_eq!(action_for(&set, session, "140.82.112.4"), Action::Ask);

    let all = RuleSet::from_rules([Rule::new(
        RuleId::new(),
        Action::Allow,
        Matcher::host(match humanitl_rules::host::parse_pattern("**") {
            Ok((pattern, _)) => pattern,
            Err(diagnostic) => panic!("{}", diagnostic.why),
        }),
    )]);
    let address = host("140.82.112.3");
    let key = RequestKey::new(&address, &Method::GET, "/", Scheme::Https, 443);
    assert_eq!(
        all.evaluate(&key, now(), session),
        Verdict::Default,
        "`**` never matches an address"
    );
}

#[test]
fn rule_unknown_method_asks() {
    let session = SessionId::new();
    let set = rules(session);
    let target = host("api.github.com");
    let Ok(method) = Method::from_bytes(b"BREW") else {
        panic!("BREW is a syntactically valid method token");
    };
    let key = RequestKey::new(&target, &method, "/", Scheme::Https, 443);
    assert_eq!(set.evaluate(&key, now(), session), Verdict::Default);
}

#[test]
fn rule_websocket_upgrade() {
    let session = SessionId::new();
    let set = rules(session);

    // Ein Upgrade auf einen erlaubten Host bleibt eine Frage an den Menschen …
    let allowed = host("api.github.com");
    let upgrade = RequestKey::new(&allowed, &Method::GET, "/", Scheme::Https, 443)
        .with_upgrade(Upgrade::WebSocket);
    assert_eq!(set.evaluate(&upgrade, now(), session), Verdict::Default);

    // … und nur eine Regel mit `upgrade: websocket` entscheidet es.
    let named = host("ws.github.com");
    let upgrade = RequestKey::new(&named, &Method::GET, "/", Scheme::Https, 443)
        .with_upgrade(Upgrade::WebSocket);
    assert_eq!(
        set.evaluate(&upgrade, now(), session).action(),
        Action::Allow
    );

    // Umgekehrt trifft diese Regel keine gewöhnliche Anfrage: `ws.github.com`
    // ist über `*.github.com` erlaubt, aber nicht über die Upgrade-Regel.
    let plain = RequestKey::new(&named, &Method::POST, "/", Scheme::Https, 443);
    assert_eq!(
        set.evaluate(&plain, now(), session).action(),
        Action::Block,
        "the POST rule stands first"
    );
}

#[test]
fn rule_body_over_cap() {
    // Die Hälfte, die diese Crate belegen kann: für den Host, an den ESC-4
    // einen zu großen Body schickt, sagt der Regelsatz `allow`. Der Proxy
    // antwortet trotzdem mit `413` und `reason: body_cap` (HUM-016, ADR-005):
    // der Cap wird entschieden, bevor eine Regel gefragt wird, und keine Regel
    // hebt ihn auf. Die andere Hälfte misst die Probe in
    // `tests/escape/esc-4-rules.sh` am laufenden Proxy.
    let session = SessionId::new();
    let set = rules(session);
    let target = host("blocked.example");
    let key = RequestKey::new(&target, &Method::POST, "/upload", Scheme::Http, 80);
    assert_eq!(
        set.evaluate(&key, now(), session).action(),
        Action::Allow,
        "the probe only says something if a rule would let this request through"
    );
}
