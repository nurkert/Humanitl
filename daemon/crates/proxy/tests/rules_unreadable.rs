//! HUM-561: Eine Regel der `rules.yaml`, die sich nicht lesen lässt, kostet
//! nur sich selbst.
//!
//! Gemessen am echten Ladeweg (`RulesStore::load`, wie `load_rules` in
//! `humanitld`): Die übrigen Regeln gelten, der Befund nennt Regel, Zeile und
//! Grund, an der Stelle der Regel steht ein Ersatz, der nie mehr erlaubt als
//! sie, und ihr Eintrag überlebt jedes Schreiben des Daemons.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use humanitl_core::rule::{Action, HostPattern, Matcher, Rule};
use humanitl_core::{Diagnostic, HostName, Method, RuleId, Scheme, SessionId, Severity, Upgrade};
use humanitl_proxy::rules_store::{RulesStore, read_rules_file};
use humanitl_rules::RequestKey;
use jiff::Timestamp;

/// Zwei gültige Regeln und dazwischen eine, deren `expires` sich nicht lesen
/// lässt: genau der Fall aus HUM-526.
const ONE_BROKEN: &str = "\
version: 1
rules:
  - id: 01920000-0000-7000-8000-0000000000a1
    action: allow
    match:
      host: \"api.github.com\"
  - id: 01920000-0000-7000-8000-0000000000a2
    action: allow
    match:
      host: \"registry.npmjs.org\"
    expires: \"next tuesday\"
  - id: 01920000-0000-7000-8000-0000000000a3
    action: block
    match:
      host: \"evil.example\"
";

fn file_with(dir: &tempfile::TempDir, text: &str) -> PathBuf {
    let path = dir.path().join("humanitl").join("rules.yaml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

fn verdict(store: &RulesStore, host: &str) -> Action {
    verdict_of(store, host, None)
}

/// Wie [`verdict`], wahlweise als WebSocket-Upgrade.
fn verdict_of(store: &RulesStore, host: &str, upgrade: Option<Upgrade>) -> Action {
    let host = HostName::parse(host).unwrap();
    let mut key = RequestKey::new(&host, &Method::GET, "/", Scheme::Https, 443);
    if let Some(upgrade) = upgrade {
        key = key.with_upgrade(upgrade);
    }
    store
        .effective()
        .evaluate(&key, Timestamp::now(), store.session())
        .action()
}

/// Die Befunde zu Regeln, die herausgefallen sind.
fn left_out(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.why.contains("is left out"))
        .collect()
}

fn errors(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.severity >= Severity::Error)
        .collect()
}

fn allow(host: &str) -> Rule {
    Rule::new(
        RuleId::new(),
        Action::Allow,
        Matcher::host(HostPattern::parse(host).unwrap()),
    )
}

#[test]
fn one_broken_rule_costs_only_itself_and_the_finding_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(&dir, ONE_BROKEN);

    let (store, diagnostics) = RulesStore::load(&path, &[], SessionId::new());

    // Beide gültigen Regeln gelten.
    assert_eq!(store.list().len(), 2, "{:?}", store.list());
    assert_eq!(verdict(&store, "api.github.com"), Action::Allow);
    assert_eq!(verdict(&store, "evil.example"), Action::Block);
    // Die kaputte fehlt: ihr Host wird gefragt.
    assert_eq!(verdict(&store, "registry.npmjs.org"), Action::Ask);

    // Genau ein Befund, und er nennt Regel, Feld, Zeile und Grund. Eine
    // Warnung: Die Datei gilt, und eine Liste der Regeln scheitert nicht daran.
    let found = left_out(&diagnostics);
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    let why = &found[0].why;
    assert_eq!(found[0].code.as_str(), "RULES_001");
    for part in [
        "rules[1].expires (line 11)",
        "next tuesday",
        "rules[1] (line 7)",
    ] {
        assert!(why.contains(part), "{part:?} missing in: {why}");
    }
    // Derselbe Befund steht, bis die Datei repariert ist.
    assert_eq!(store.findings(), vec![found[0].clone()]);
}

#[test]
fn writing_after_the_start_keeps_the_broken_entry_in_its_place() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(&dir, ONE_BROKEN);
    let (store, _) = RulesStore::load(&path, &[], SessionId::new());

    store.add(&allow("crates.io"), None).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    let first = written.find("api.github.com").unwrap();
    let broken = written.find("next tuesday").expect(&written);
    let third = written.find("evil.example").unwrap();
    assert!(first < broken && broken < third, "{written}");

    // Die Regel davor verschwindet: der Eintrag rückt nach vorn und bleibt.
    let anchor = RuleId::parse("01920000-0000-7000-8000-0000000000a1").unwrap();
    store.remove(anchor).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    let broken = written.find("next tuesday").expect(&written);
    assert!(broken < written.find("evil.example").unwrap(), "{written}");

    // Der Daemon liest seine eigene Datei wieder genauso: zwei Regeln, ein
    // unlesbarer Eintrag, derselbe Grund.
    let again = read_rules_file(&path, SessionId::new());
    assert!(!again.refused);
    assert_eq!(again.rules.len(), 2);
    assert_eq!(again.unreadable.len(), 1);
    assert!(
        again.unreadable[0]
            .diagnostic()
            .why
            .contains("next tuesday")
    );
}

#[test]
fn an_unreadable_block_rule_keeps_blocking_over_a_broader_allow_below_it() {
    // Die Sperre ist kaputt (`expires`), darunter steht eine breite Freigabe,
    // und mitgeliefert ist eine zweite. Ohne Ersatz ginge `uploads` hinaus.
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        "version: 1
rules:
  - action: block
    match:
      host: \"uploads.example.com\"
    expires: \"soon\"
  - action: allow
    match:
      host: \"**.example.com\"
",
    );
    let bundled = allow("uploads.example.com");

    let (store, diagnostics) = RulesStore::load(&path, &[bundled], SessionId::new());

    assert_eq!(verdict(&store, "uploads.example.com"), Action::Block);
    assert_eq!(verdict(&store, "www.example.com"), Action::Allow);
    let why = &left_out(&diagnostics)[0].why;
    assert!(why.contains("stands as `block`"), "{why}");
    // Der Ersatz ist keine Regel des Nutzers: nichts zum Ändern in der Liste.
    assert_eq!(store.list().len(), 2);
}

#[test]
fn a_block_rule_with_an_unreadable_condition_holds_its_host_instead() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        "version: 1
rules:
  - action: block
    match:
      host: \"api.example.com\"
      path: \"~^/admin[\"
  - action: allow
    match:
      host: \"**.example.com\"
",
    );

    let (store, _) = RulesStore::load(&path, &[], SessionId::new());

    // Gefragt, nicht durchgelassen und nicht breiter gesperrt als gemeint.
    assert_eq!(verdict(&store, "api.example.com"), Action::Ask);
    assert_eq!(verdict(&store, "www.example.com"), Action::Allow);
}

/// Eine Datei: die kaputte Regel, darunter Freigaben für jeden Namen und ein Netz.
fn above_broad_allows(broken: &str) -> String {
    format!(
        "version: 1
rules:
{broken}  - action: allow
    match:
      host: \"**\"
  - action: allow
    match:
      host: \"cidr:10.0.0.0/8\"
  - action: allow
    match:
      host: \"cidr:::/0\"
"
    )
}

#[test]
fn an_unreadable_host_holds_every_name_and_address_below_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        &above_broad_allows("  - action: block\n    match:\n      host: \"*foo.com\"\n"),
    );

    let (store, diagnostics) = RulesStore::load(&path, &[], SessionId::new());

    assert_eq!(store.list().len(), 3);
    assert_eq!(left_out(&diagnostics).len(), 1, "{diagnostics:?}");
    for host in ["anything.example", "10.1.2.3", "::1"] {
        assert_eq!(verdict(&store, host), Action::Ask, "{host}");
    }
}

#[test]
fn an_unknown_action_holds_the_host_of_the_rule() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        &above_broad_allows("  - action: deny\n    match:\n      host: \"foo.com\"\n"),
    );

    let (store, diagnostics) = RulesStore::load(&path, &[], SessionId::new());

    assert_eq!(store.list().len(), 3);
    assert_eq!(left_out(&diagnostics).len(), 1, "{diagnostics:?}");
    assert_eq!(verdict(&store, "foo.com"), Action::Ask);
    assert_eq!(verdict(&store, "anything.example"), Action::Allow);
}

#[test]
fn an_unreadable_allow_rule_just_drops_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        "version: 1
rules:
  - action: allow
    match:
      host: \"*foo.com\"
  - action: allow
    match:
      host: \"**\"
",
    );

    let (store, _) = RulesStore::load(&path, &[], SessionId::new());

    assert_eq!(verdict(&store, "anything.example"), Action::Allow);
}

#[test]
fn reload_reads_the_same_way_and_drops_the_finding_once_the_rule_is_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(&dir, "version: 1\nrules: []\n");
    let (store, _) = RulesStore::load(&path, &[], SessionId::new());

    std::fs::write(&path, ONE_BROKEN).unwrap();
    let diagnostics = store.reload();
    assert_eq!(store.list().len(), 2);
    assert_eq!(left_out(&diagnostics).len(), 1, "{diagnostics:?}");
    assert_eq!(store.findings().len(), 1);

    std::fs::write(&path, ONE_BROKEN.replace("\"next tuesday\"", "never")).unwrap();
    let diagnostics = store.reload();
    assert!(left_out(&diagnostics).is_empty(), "{diagnostics:?}");
    assert_eq!(store.list().len(), 3);
    assert!(store.findings().is_empty());
    assert_eq!(verdict(&store, "registry.npmjs.org"), Action::Allow);
}

#[test]
fn a_file_that_cannot_be_read_as_a_whole_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let text = ONE_BROKEN.replace("version: 1", "version: 2");
    let path = file_with(&dir, &text);

    let (store, diagnostics) = RulesStore::load(&path, &[], SessionId::new());
    assert!(store.list().is_empty());
    assert!(!errors(&diagnostics).is_empty());
    let standing = store.findings();
    assert_eq!(standing.len(), 1, "{standing:?}");
    assert_eq!(standing[0].severity, Severity::Warning);
    assert!(standing[0].why.contains("version 2"), "{}", standing[0].why);

    let refused = store.add(&allow("crates.io"), None).unwrap_err();
    assert_eq!(refused.code.as_str(), "RULES_009");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    // Eine Sitzungsregel schreibt nichts und geht weiter.
    let session =
        allow("crates.io").with_expiry(humanitl_core::rule::Expiry::Session(store.session()));
    store.add(&session, None).unwrap();

    // Nach der Reparatur wird wieder geschrieben.
    std::fs::write(&path, ONE_BROKEN.replace("\"next tuesday\"", "never")).unwrap();
    store.reload();
    store.add(&allow("pypi.org"), None).unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains("pypi.org"));
}

#[test]
fn a_reload_of_a_broken_file_keeps_the_rules_and_refuses_to_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(&dir, "version: 1\nrules: []\n");
    let (store, _) = RulesStore::load(&path, &[], SessionId::new());
    store.add(&allow("api.github.com"), None).unwrap();
    let before = store.effective();

    std::fs::write(&path, "version: 1\nrules: [ - oops\n").unwrap();
    let diagnostics = store.reload();

    assert!(!errors(&diagnostics).is_empty());
    assert_eq!(store.effective(), before);
    assert!(store.add(&allow("pypi.org"), None).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "version: 1\nrules: [ - oops\n"
    );
}

#[test]
fn the_held_host_covers_websocket_upgrades_too() {
    // Die Regel ohne `upgrade` träfe nie ein Upgrade; ohne eigenen Ersatz
    // dafür entschiede die WebSocket-Freigabe darunter.
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        "version: 1
rules:
  - action: block
    match:
      host: \"ws.example.com\"
      path: \"~^/feed[\"
      upgrade: websocket
  - action: allow
    match:
      host: \"ws.example.com\"
      upgrade: websocket
",
    );

    let (store, _) = RulesStore::load(&path, &[], SessionId::new());

    assert_eq!(
        verdict_of(&store, "ws.example.com", Some(Upgrade::WebSocket)),
        Action::Ask
    );
}

#[test]
fn a_session_rule_keeps_its_rank_before_the_persistent_rules() {
    // Eine lesbare Sitzungs-Sperre (kaputt ist nur `created_from`) und ein
    // Ersatz mit unlesbarem Pfad stehen unter einer dauerhaften Freigabe;
    // als Sitzungsregeln gehen sie ihr trotzdem vor.
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(
        &dir,
        "version: 1
rules:
  - action: allow
    match:
      host: \"**.example.com\"
  - action: block
    match:
      host: \"kept.example.com\"
    expires: session
    created_from: \"not a flow\"
  - action: block
    match:
      host: \"held.example.com\"
      path: \"~^/x[\"
    expires: session
",
    );

    let (store, _) = RulesStore::load(&path, &[], SessionId::new());

    assert_eq!(verdict(&store, "kept.example.com"), Action::Block);
    assert_eq!(verdict(&store, "held.example.com"), Action::Ask);
    assert_eq!(verdict(&store, "www.example.com"), Action::Allow);
}

#[test]
fn no_change_lifts_a_rule_above_a_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let text = "version: 1
rules:
  - id: 01920000-0000-7000-8000-0000000000c1
    action: allow
    match:
      host: \"a.example\"
  - action: block
    match:
      host: \"uploads.example.com\"
    expires: \"soon\"
  - id: 01920000-0000-7000-8000-0000000000c3
    action: allow
    match:
      host: \"uploads.example.com\"
";
    let path = file_with(&dir, text);
    let (store, _) = RulesStore::load(&path, &[], SessionId::new());
    let anchor = RuleId::parse("01920000-0000-7000-8000-0000000000c1").unwrap();
    let below = RuleId::parse("01920000-0000-7000-8000-0000000000c3").unwrap();

    // Der Anker unter die Freigabe: Der Ersatz wanderte mit, und `uploads`
    // ginge hinaus.
    let refused = store.reorder(anchor, 1).unwrap_err();
    assert_eq!(refused.code.as_str(), "RULES_009");
    assert!(refused.why.contains("rules[1] (line 7)"), "{}", refused.why);
    // Dasselbe über die ganze Reihenfolge und für eine neue Regel davor.
    assert!(store.reorder_all(&[below, anchor]).is_err());
    assert!(store.add(&allow("crates.io"), Some(0)).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert_eq!(verdict(&store, "uploads.example.com"), Action::Block);

    // Unter dem Ersatz, ans Ende oder weg: das hebt nichts über ihn.
    store.add(&allow("crates.io"), None).unwrap();
    store.remove(anchor).unwrap();
    assert_eq!(verdict(&store, "uploads.example.com"), Action::Block);
}

#[test]
fn a_finding_points_at_the_file_as_it_was_last_written() {
    // Die Regel über dem kaputten Eintrag verschwindet: Danach steht er in
    // Zeile 3 der neu geschriebenen Datei, nicht mehr in Zeile 7.
    let dir = tempfile::tempdir().unwrap();
    let path = file_with(&dir, ONE_BROKEN);
    let (store, _) = RulesStore::load(&path, &[], SessionId::new());
    assert!(store.findings()[0].why.contains("rules[1] (line 7)"));

    let anchor = RuleId::parse("01920000-0000-7000-8000-0000000000a1").unwrap();
    store.remove(anchor).unwrap();

    let again = read_rules_file(&path, SessionId::new());
    let expected = again.unreadable[0].diagnostic().clone();
    assert!(
        expected.why.contains("rules[0] (line 3)"),
        "{}",
        expected.why
    );
    assert_eq!(store.findings(), vec![expected]);
}
