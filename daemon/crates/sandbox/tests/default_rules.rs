//! Der mitgelieferte Regelsatz `rules/default.yaml` (HUM-038).
//!
//! Der Adapter ist die einzige Quelle dieser Regeln; gelesen werden sie hier
//! mit demselben Parser, den auch der Daemon benutzt. Der Regelsatz beantwortet
//! die Frage, die HUM-038 stellt: Wie viele Anfragen sieht ein Mensch beim
//! ersten Start, die er nicht ausgelöst hat?

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_core::rule::Action;
use humanitl_core::{HostName, Method, Scheme, SessionId};
use humanitl_rules::{RequestKey, RuleSet, Verdict, parse_rules};
use humanitl_sandbox::{AgentAdapter, OpenCodeAdapter};

/// Der Regelsatz, wie ihn der Daemon lesen würde.
fn bundled() -> RuleSet {
    let (rules, warnings) =
        parse_rules(OpenCodeAdapter::new().default_rules()).expect("rules/default.yaml parses");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    rules
}

/// Wertet eine Anfrage gegen den Regelsatz aus.
fn verdict(set: &RuleSet, host: &str, method: &Method, path: &str) -> Verdict {
    let host = HostName::parse(host).unwrap();
    let key = RequestKey::new(&host, method, path, Scheme::Https, 443);
    set.evaluate(&key, chrono::Utc::now(), SessionId::nil())
}

#[test]
fn default_rules_parse() {
    let set = bundled();
    // Acht Regeln nennt `backlog/sprint-3.md`. Zwei kamen dazu, beide aus dem
    // installierten Binary 1.18.25: der Modellkatalog liegt auf
    // `models.opencode.ai` und nicht auf `models.dev`, und das Teilen einer
    // Sitzung geht an `opncd.ai` und nicht an `opencode.ai/share`.
    assert_eq!(set.len(), 10);
    for rule in set.iter() {
        assert!(rule.bundled, "rule {} is not bundled", rule.id);
        assert!(
            rule.note.as_ref().is_some_and(|note| note.len() > 20),
            "rule {} has no note explaining why it exists; host names change, \
             and a later reader has to be able to check it",
            rule.id
        );
        assert!(
            !rule.allow_private,
            "a bundled rule never opens a private address; only the LLM passthrough does"
        );
        assert_ne!(
            rule.action,
            Action::Allow,
            "rule {} lets traffic through without asking; the bundled set blocks or asks",
            rule.id
        );
    }
}

#[test]
fn default_rule_ids_are_stable_and_unique() {
    let set = bundled();
    let mut ids: Vec<String> = set.iter().map(|rule| rule.id.to_string()).collect();
    ids.sort();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "a rule id appears twice");
    for id in &ids {
        assert!(
            id.starts_with("01920000-0000-7000-8000-"),
            "{id} is not one of the fixed literals that `disabled_bundled` refers to"
        );
    }
}

#[test]
fn default_rules_match_table() {
    let set = bundled();

    for (host, method, path, expected) in [
        ("models.dev", Method::GET, "/api.json", Action::Block),
        (
            "models.opencode.ai",
            Method::GET,
            "/api.json",
            Action::Block,
        ),
        (
            "api.github.com",
            Method::GET,
            "/repos/anomalyco/opencode/releases/latest",
            Action::Block,
        ),
        // Kein Blankoschein für GitHub: alles andere dort bleibt eine Frage.
        ("api.github.com", Method::GET, "/repos/foo/bar", Action::Ask),
        ("eu.posthog.com", Method::POST, "/i/v0/e/", Action::Block),
        ("posthog.com", Method::POST, "/i/v0/e/", Action::Block),
        (
            "o1.ingest.sentry.io",
            Method::POST,
            "/api/1/envelope/",
            Action::Block,
        ),
        ("opncd.ai", Method::POST, "/api/share", Action::Block),
        (
            "opncd.ai",
            Method::POST,
            "/api/share/abc/sync",
            Action::Block,
        ),
        ("opencode.ai", Method::POST, "/share/abc", Action::Block),
        // Die Startseite ist nicht das Teilen einer Sitzung.
        ("opencode.ai", Method::GET, "/docs/", Action::Ask),
        ("mcp.exa.ai", Method::POST, "/search", Action::Block),
        (
            "search.parallel.ai",
            Method::POST,
            "/v1/search",
            Action::Block,
        ),
        (
            "registry.npmjs.org",
            Method::GET,
            "/@ai-sdk/openai-compatible",
            Action::Ask,
        ),
        (
            "registry.npmjs.org",
            Method::POST,
            "/@ai-sdk/openai-compatible",
            Action::Ask,
        ),
    ] {
        let outcome = verdict(&set, host, &method, path);
        assert_eq!(
            outcome.action(),
            expected,
            "{method} https://{host}{path} came out as {outcome:?}"
        );
    }
}

#[test]
fn the_npm_rule_asks_and_never_allows() {
    let set = bundled();
    let outcome = verdict(
        &set,
        "registry.npmjs.org",
        &Method::GET,
        "/@ai-sdk/openai-compatible",
    );
    let Verdict::Matched { action, .. } = outcome else {
        panic!("the npm rule has to match a GET, otherwise the card explaining it never appears");
    };
    assert_eq!(
        action,
        Action::Ask,
        "a package can carry a postinstall script; `allow` would run it unasked"
    );
    // Ein POST trifft die Regel nicht; er endet als Vorgabe, ebenfalls `ask`.
    assert_eq!(
        verdict(
            &set,
            "registry.npmjs.org",
            &Method::POST,
            "/@ai-sdk/openai-compatible"
        ),
        Verdict::Default
    );
}

#[test]
fn disabled_bundled_is_skipped() {
    // Die `rules.yaml` des Nutzers schaltet die Katalog-Regel ab. Sie steht
    // dort nur als Id, denn `rules/default.yaml` gehört zum Build und wird nie
    // geschrieben (HUM-038).
    let user = "version: 1\nrules: []\ndisabled_bundled:\n                 - 01920000-0000-7000-8000-000000000001\n";
    let (mut set, warnings) = parse_rules(user).expect("the user file parses");
    assert!(warnings.is_empty(), "{warnings:?}");

    let (bundled, _) = parse_rules(OpenCodeAdapter::new().default_rules()).expect("bundled parse");
    set.prepend_bundled(bundled.iter().cloned());

    assert_eq!(
        verdict(&set, "models.dev", &Method::GET, "/api.json"),
        Verdict::Default,
        "a disabled rule decides nothing"
    );
    // Abgeschaltet heißt nicht gelöscht: die Regel steht weiter im Satz, damit
    // der Rules-Screen sie samt Begründung zeigen kann.
    let rule = set
        .iter()
        .find(|rule| rule.id.to_string() == "01920000-0000-7000-8000-000000000001")
        .expect("the rule is still there");
    assert!(rule.disabled);
    assert!(rule.bundled);
    // Alles andere gilt unverändert.
    assert_eq!(
        verdict(&set, "models.opencode.ai", &Method::GET, "/api.json").action(),
        Action::Block
    );
}

#[test]
fn prepend_bundled_puts_the_bundled_rules_first() {
    let user = "version: 1\nrules:\n  - action: allow\n    match: { host: \"models.dev\" }\n";
    let (mut set, _) = parse_rules(user).expect("the user file parses");
    let (bundled, _) = parse_rules(OpenCodeAdapter::new().default_rules()).expect("bundled parse");
    set.prepend_bundled(bundled.iter().cloned());

    assert_eq!(
        verdict(&set, "models.dev", &Method::GET, "/api.json").action(),
        Action::Block,
        "the bundled rule is evaluated before the rule of the user"
    );
    assert_eq!(set.len(), 11);
    assert!(
        set.iter().take(10).all(|rule| rule.bundled),
        "the first ten rules are the bundled ones"
    );
    assert!(!set.iter().nth(10).expect("the user rule").bundled);
}

/// Der Regelsatz gegen die Startziele, die im Binary stehen.
///
/// Das ist **nicht** der Metriktest `startup_noise_budget` aus HUM-038. Der
/// zählt Flows mit `state == Held` zwischen Sandbox-Start und erstem Prompt und
/// braucht dafür den Fake-LLM aus HUM-046, den Daemon und ein PTY; er kommt mit
/// diesem Mock als `daemon/crates/sandbox/tests/startup_noise.rs` hinter dem
/// Feature `agent-e2e`. Hier wird nur geprüft, was der Regelsatz aus den
/// bekannten Startzielen macht — kein Flow, keine Sandbox, kein Netz.
#[test]
fn bundled_rules_cover_the_known_startup_hosts() {
    let set = bundled();

    // Was OpenCode 1.18.25 von sich aus tut, bevor ein Mensch etwas eingibt.
    // Die Adressen stammen aus dem installierten Binary, nicht aus der
    // Dokumentation; `agents/opencode/README.md` sagt, wie sie gefunden wurden.
    let startup = [
        ("models.opencode.ai", Method::GET, "/api.json"),
        ("models.dev", Method::GET, "/api.json"),
        (
            "api.github.com",
            Method::GET,
            "/repos/anomalyco/opencode/releases/latest",
        ),
        ("opncd.ai", Method::POST, "/api/share"),
        ("mcp.exa.ai", Method::POST, "/search"),
        ("search.parallel.ai", Method::POST, "/v1/search"),
        (
            "registry.npmjs.org",
            Method::GET,
            "/@ai-sdk/openai-compatible",
        ),
    ];

    let not_blocked: Vec<&str> = startup
        .iter()
        .filter(|(host, method, path)| verdict(&set, host, method, path).action() != Action::Block)
        .map(|(host, _, _)| *host)
        .collect();

    assert_eq!(
        not_blocked,
        vec!["registry.npmjs.org"],
        "of the known startup targets the bundled rules block all but one, \
         and the remaining one is the npm request that the catalog card explains"
    );
}
