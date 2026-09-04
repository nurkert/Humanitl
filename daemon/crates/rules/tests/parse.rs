//! `rules.yaml`: jede Diagnose-Zeile aus HUM-022 einmal, dazu der Roundtrip.
//!
//! Eine Regel-Datei ist Sicherheitskonfiguration. Der Test prüft deshalb nicht
//! nur, dass ein Fehler auffällt, sondern auch, dass die Datei als Ganzes
//! abgelehnt wird: eine halb geladene Regel-Datei wäre ein Regelsatz, den
//! niemand geschrieben hat.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_core::diagnostics::codes::{
    RULES_001, RULES_002, RULES_003, RULES_005, RULES_006, RULES_007, RULES_008,
};
use humanitl_core::rule::{Action, Expiry, HostPattern};
use humanitl_core::{Diagnostic, DiagnosticCode, Method, SessionId, Severity};
use humanitl_rules::{RuleSet, parse_rules, parse_rules_for_session, serialize_rules};

/// Die mitgelieferte Datei, damit sie nie ungültig ins Paket kommt.
const DEFAULT_RULES: &str = include_str!("../../../../rules/default.yaml");

fn errors(yaml: &str) -> Vec<Diagnostic> {
    match parse_rules(yaml) {
        Ok((_, warnings)) => panic!("this file must be rejected; warnings: {warnings:?}"),
        Err(diagnostics) => diagnostics,
    }
}

fn ok(yaml: &str) -> (RuleSet, Vec<Diagnostic>) {
    parse_rules(yaml).unwrap_or_else(|diagnostics| panic!("must parse: {diagnostics:?}"))
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<DiagnosticCode> {
    diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn rules_001_rejects_broken_yaml() {
    let diagnostics = errors("version: 1\nrules: [ - oops\n");
    assert_eq!(codes(&diagnostics), vec![RULES_001]);
    assert!(
        diagnostics[0].why.contains("line"),
        "the finding names the line: {}",
        diagnostics[0].why
    );
}

#[test]
fn rules_001_rejects_an_unknown_key() {
    let diagnostics = errors(
        "version: 1\nrules:\n  - action: allow\n    hosts: \"github.com\"\n    match:\n      host: \"github.com\"\n",
    );
    assert_eq!(codes(&diagnostics), vec![RULES_001]);
    assert!(diagnostics[0].why.contains("hosts"));
}

#[test]
fn rules_001_rejects_an_unknown_method() {
    let diagnostics = errors(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"github.com\"\n      method: [GET, BREW]\n",
    );
    assert_eq!(codes(&diagnostics), vec![RULES_001]);
    assert!(diagnostics[0].why.contains("BREW"));
    assert!(diagnostics[0].why.contains("rules[0].match.method"));
    assert!(
        diagnostics[0].why.contains("line 6"),
        "{}",
        diagnostics[0].why
    );
}

#[test]
fn methods_are_read_case_insensitively() {
    let (set, warnings) = ok(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"github.com\"\n      method: [get, Head]\n",
    );
    assert!(warnings.is_empty());
    let Some(rule) = set.iter().next() else {
        panic!("one rule");
    };
    let Some(methods) = rule.matcher.methods.as_ref() else {
        panic!("methods are set");
    };
    assert_eq!(
        methods.iter().map(Method::as_str).collect::<Vec<_>>(),
        vec!["GET", "HEAD"]
    );
}

#[test]
fn rules_002_warns_about_a_punycode_literal() {
    let (set, warnings) = ok(
        "version: 1\nrules:\n  - action: block\n    match:\n      host: \"xn--80ak6aa92e.com\"\n",
    );
    assert_eq!(codes(&warnings), vec![RULES_002]);
    assert_eq!(warnings[0].severity, Severity::Warning);
    assert!(warnings[0].why.contains("rules[0].match.host"));
    assert!(warnings[0].why.contains("line 5"), "{}", warnings[0].why);
    assert_eq!(set.len(), 1);
}

#[test]
fn rules_002_warns_about_an_address_as_host() {
    let (set, warnings) =
        ok("version: 1\nrules:\n  - action: block\n    match:\n      host: \"140.82.112.3\"\n");
    assert_eq!(codes(&warnings), vec![RULES_002]);
    let Some(rule) = set.iter().next() else {
        panic!("one rule");
    };
    assert!(
        matches!(rule.matcher.host, HostPattern::Ip(_)),
        "an address as a host is read as `ip:`"
    );
}

#[test]
fn rules_003_rejects_a_broken_host_pattern() {
    // Akzeptanzkriterium aus HUM-022: genau ein Befund, Code RULES_003, und der
    // abgelehnte Muster-Text steht im `why`.
    let diagnostics =
        errors("version: 1\nrules:\n  - action: allow\n    match:\n      host: \"*foo.com\"\n");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, RULES_003);
    assert!(diagnostics[0].why.contains("*foo.com"));
    assert!(
        diagnostics[0].why.contains("line 5"),
        "{}",
        diagnostics[0].why
    );
}

#[test]
fn rules_005_rejects_a_broken_regex() {
    let diagnostics = errors(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"github.com\"\n      path: \"~^/v[0-9+/\"\n",
    );
    assert_eq!(codes(&diagnostics), vec![RULES_005]);
    assert!(diagnostics[0].why.contains("rules[0].match.path"));
}

#[test]
fn rules_006_needs_the_version() {
    let missing = errors("rules: []\n");
    assert_eq!(codes(&missing), vec![RULES_006]);
    assert!(missing[0].why.contains("version"));

    // Nachtrag aus dem Review: auch dieser Befund nennt seine Zeile.
    let wrong = errors("# ein Kommentar\nversion: 2\nrules: []\n");
    assert_eq!(codes(&wrong), vec![RULES_006]);
    assert!(wrong[0].why.contains('2'));
    assert!(
        wrong[0].why.contains("version (line 2)"),
        "{}",
        wrong[0].why
    );
}

#[test]
fn rules_007_rejects_a_duplicate_id() {
    let yaml = "version: 1
rules:
  - id: 018f6c1e-7a2b-7c3d-8e4f-0123456789ab
    action: allow
    match:
      host: \"github.com\"
  - id: 018f6c1e-7a2b-7c3d-8e4f-0123456789ab
    action: block
    match:
      host: \"example.com\"
";
    let diagnostics = errors(yaml);
    assert_eq!(codes(&diagnostics), vec![RULES_007]);
    // Nachtrag aus dem Review: der Befund zeigt auf die `id`-Zeile der zweiten
    // Regel, nicht auf die der ersten und nicht auf gar keine.
    assert!(
        diagnostics[0].why.contains("rules[1].id (line 7)"),
        "{}",
        diagnostics[0].why
    );
}

#[test]
fn an_empty_host_keeps_its_line() {
    // Nachtrag aus dem Review: ein leerer Wert hat keine eigene Fundstelle,
    // sein Schlüssel schon. Gesucht wird deshalb nach `host:`.
    let diagnostics =
        errors("version: 1\nrules:\n  - action: allow\n    match:\n      host: \"\"\n");
    assert_eq!(codes(&diagnostics), vec![RULES_003]);
    assert!(
        diagnostics[0].why.contains("rules[0].match.host (line 5)"),
        "{}",
        diagnostics[0].why
    );
}

#[test]
fn a_note_never_moves_a_line_number() {
    // Der Feldpfad wird über den Schlüssel gesucht, nicht über den Wert: eine
    // Notiz, die `port` oder `host` erwähnt, darf keinen Befund verschieben.
    let yaml = "version: 1
rules:
  - action: allow
    note: \"host: example.com, port: 443, path: /\"
    match:
      host: \"example.com\"
      port: 0
";
    let diagnostics = errors(yaml);
    assert_eq!(codes(&diagnostics), vec![RULES_001]);
    assert!(
        diagnostics[0].why.contains("rules[0].match.port (line 7)"),
        "{}",
        diagnostics[0].why
    );
}

#[test]
fn rules_008_warns_about_a_rule_that_allows_everything() {
    let (set, warnings) =
        ok("version: 1\nrules:\n  - action: allow\n    match:\n      host: \"**\"\n");
    assert_eq!(codes(&warnings), vec![RULES_008]);
    assert_eq!(warnings[0].severity, Severity::Warning);
    assert_eq!(set.len(), 1);

    // Mit einer weiteren Bedingung ist die Regel nicht mehr die Abschaffung
    // der Moderation, und die Warnung entfällt.
    let (_, quiet) = ok(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"**\"\n      method: [GET]\n",
    );
    assert!(quiet.is_empty(), "{quiet:?}");

    // Und `block` über allem ist eine gewöhnliche, sinnvolle Regel.
    let (_, quiet) = ok("version: 1\nrules:\n  - action: block\n    match:\n      host: \"**\"\n");
    assert!(quiet.is_empty(), "{quiet:?}");

    // `upgrade` schränkt genauso ein wie Methode oder Pfad: die Regel trifft
    // nur WebSocket-Upgrades, nicht jede Anfrage.
    let (_, quiet) = ok(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"**\"\n      upgrade: websocket\n",
    );
    assert!(quiet.is_empty(), "{quiet:?}");
}

#[test]
fn a_unicode_host_is_not_a_punycode_literal() {
    // Nachtrag aus dem Review: `münchen.de` ist die empfohlene Schreibweise
    // und darf nicht beanstandet werden, nur weil IDNA daraus ein A-Label
    // macht.
    let (set, warnings) =
        ok("version: 1\nrules:\n  - action: allow\n    match:\n      host: \"münchen.de\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let Some(rule) = set.iter().next() else {
        panic!("one rule");
    };
    assert_eq!(rule.matcher.host.to_string(), "xn--mnchen-3ya.de");
}

#[test]
fn a_mapped_network_below_prefix_96_is_rejected() {
    // Nachtrag aus dem Review: die Präfixlänge eines IPv4-mapped Netzes muss
    // sich in die IPv4-Familie übersetzen lassen, sonst bezeichnet die Regel
    // etwas anderes, als dort steht.
    let diagnostics = errors(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"cidr:::ffff:192.168.0.0/64\"\n",
    );
    assert_eq!(codes(&diagnostics), vec![RULES_003]);
    assert!(diagnostics[0].why.contains("rules[0].match.host"));

    // Mit einem Präfix ab 96 ist dasselbe Netz gültig und wird umgerechnet.
    let (set, warnings) = ok(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"cidr:::ffff:192.168.0.0/112\"\n",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let Some(rule) = set.iter().next() else {
        panic!("one rule");
    };
    assert_eq!(rule.matcher.host.to_string(), "cidr:192.168.0.0/16");
}

#[test]
fn every_error_of_a_file_is_reported_at_once() {
    let yaml = "version: 3
rules:
  - action: allow
    match:
      host: \"*foo.com\"
  - action: allow
    match:
      host: \"github.com\"
      path: \"~^/v[0-9+/\"
";
    let diagnostics = errors(yaml);
    assert_eq!(codes(&diagnostics), vec![RULES_006, RULES_003, RULES_005]);
}

#[test]
fn expiry_is_read_in_all_three_forms() {
    let session = SessionId::new();
    let yaml = "version: 1
rules:
  - action: allow
    match:
      host: \"a.example\"
    expires: never
  - action: allow
    match:
      host: \"b.example\"
    expires: session
  - action: allow
    match:
      host: \"c.example\"
    expires: \"2026-09-03T10:00:00Z\"
";
    let (set, warnings) = parse_rules_for_session(yaml, session)
        .unwrap_or_else(|diagnostics| panic!("must parse: {diagnostics:?}"));
    assert!(warnings.is_empty());
    let expiries: Vec<Expiry> = set.iter().map(|rule| rule.expires).collect();
    assert_eq!(expiries[0], Expiry::Never);
    assert_eq!(expiries[1], Expiry::Session(session));
    assert!(matches!(expiries[2], Expiry::At(_)));

    let broken = errors(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"a.example\"\n    expires: tomorrow\n",
    );
    assert_eq!(codes(&broken), vec![RULES_001]);
}

#[test]
fn a_session_rule_from_the_file_belongs_to_no_running_session() {
    // Der Fallstrick aus HUM-022: eine gespeicherte Sitzungsregel ist beim
    // nächsten Start tot, und das ist so gewollt.
    let (set, _) = ok(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"a.example\"\n    expires: session\n",
    );
    let Some(rule) = set.iter().next() else {
        panic!("one rule");
    };
    assert!(rule.is_expired(chrono::Utc::now(), SessionId::new()));
}

#[test]
fn round_trip_keeps_every_field() {
    let session = SessionId::new();
    let yaml = "version: 1
rules:
  - id: 018f6c1e-7a2b-7c3d-8e4f-0123456789ab
    action: allow
    match:
      host: \"**.npmjs.org\"
      method: [GET, HEAD]
      path: \"/**\"
      scheme: https
      port: 443
    expires: session
    stream: true
    allow_private: true
    created_from: 018f6c1e-7a2b-7c3d-8e4f-0123456789ac
    bundled: true
    note: \"npm install\"
  - action: redact
    match:
      host: \"api.openai.com\"
      upgrade: websocket
    expires: \"2026-09-03T10:00:00Z\"
";
    let (first, warnings) = parse_rules_for_session(yaml, session)
        .unwrap_or_else(|diagnostics| panic!("must parse: {diagnostics:?}"));
    assert!(warnings.is_empty());

    let written = serialize_rules(&first);
    let (second, _) = parse_rules_for_session(&written, session)
        .unwrap_or_else(|diagnostics| panic!("the written file must parse: {diagnostics:?}"));

    assert_eq!(first, second, "written file:\n{written}");
    assert_eq!(serialize_rules(&second), written);

    // Jede Regel trägt beim Schreiben ihre Id, auch die, die keine hatte.
    for rule in second.iter() {
        assert!(written.contains(&rule.id.to_string()));
    }
    assert!(
        written.contains("expires: session"),
        "a session rule is written without its id:\n{written}"
    );
    let Some(second_rule) = second.iter().nth(1) else {
        panic!("two rules");
    };
    assert_eq!(second_rule.action, Action::Redact);
}

#[test]
fn the_shipped_default_file_parses() {
    let (set, warnings) = ok(DEFAULT_RULES);
    assert!(warnings.is_empty(), "{warnings:?}");
    // Seit HUM-038 hält die Datei den mitgelieferten Regelsatz des
    // OpenCode-Adapters. Wie er wirkt, prüft
    // `daemon/crates/sandbox/tests/default_rules.rs`; hier zählt nur, dass der
    // Parser ihn ohne Befund liest und keine Regel darin etwas erlaubt.
    assert!(!set.is_empty(), "the shipped file holds the bundled rules");
    for rule in set.iter() {
        assert!(rule.bundled, "rule {} is not marked bundled", rule.id);
        assert_ne!(
            rule.action,
            Action::Allow,
            "rule {} would let traffic through without asking",
            rule.id
        );
    }
}

#[test]
fn disabled_bundled_round_trips_through_the_file() {
    let yaml = "version: 1\nrules: []\ndisabled_bundled:\n                  - 01920000-0000-7000-8000-000000000001\n";
    let (set, warnings) = ok(yaml);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        set.disabled_bundled()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
        vec!["01920000-0000-7000-8000-000000000001".to_owned()]
    );

    let written = serialize_rules(&set);
    assert!(
        written.contains("disabled_bundled"),
        "the list survives a write: {written}"
    );
    let (again, _) = ok(&written);
    assert_eq!(again, set);
}

#[test]
fn disabled_bundled_rejects_something_that_is_not_an_id() {
    let diagnostics = errors("version: 1\nrules: []\ndisabled_bundled: [\"nope\"]\n");
    assert_eq!(codes(&diagnostics), vec![RULES_001]);
}

#[test]
fn a_file_without_disabled_bundled_writes_none() {
    let (set, _) = ok("version: 1\nrules: []\n");
    assert!(!serialize_rules(&set).contains("disabled_bundled"));
}

// --- Pfadpräfixe und die Durchreiche zum Sprachmodell (HUM-039) -------------

/// `path_prefixes` und `passthrough_llm` überleben Schreiben und Lesen.
#[test]
fn a_passthrough_rule_round_trips_through_the_file() {
    let yaml = "version: 1
rules:
  - id: 01920000-0000-7000-8000-0000000000ff
    action: allow
    match:
      host: \"ip:192.168.1.50\"
      method: [POST, GET]
      scheme: http
      port: 11434
      path_prefixes: [\"/v1/\", \"/api/chat\"]
    allow_private: true
    bundled: true
    passthrough_llm: true
    note: \"LLM passthrough. Logged, never held.\"
";
    let (first, warnings) =
        parse_rules(yaml).unwrap_or_else(|diagnostics| panic!("must parse: {diagnostics:?}"));
    assert!(warnings.is_empty(), "{warnings:?}");

    let rule = first.iter().next().expect("one rule");
    assert!(rule.passthrough_llm);
    assert_eq!(
        rule.matcher.path_prefixes,
        vec!["/v1/".to_owned(), "/api/chat".to_owned()]
    );

    let written = serialize_rules(&first);
    let (second, _) = parse_rules(&written)
        .unwrap_or_else(|diagnostics| panic!("the written file must parse: {diagnostics:?}"));
    assert_eq!(first, second, "written file:\n{written}");
    assert!(written.contains("passthrough_llm: true"));
    assert!(written.contains("path_prefixes:"));
}

/// Eine gewöhnliche Regel schreibt die neuen Felder nicht mit.
#[test]
fn an_ordinary_rule_carries_no_passthrough_key() {
    let yaml = "version: 1\nrules:\n  - action: block\n    match:\n      host: \"evil.io\"\n";
    let (set, _) = parse_rules(yaml).unwrap_or_else(|d| panic!("{d:?}"));
    let written = serialize_rules(&set);
    assert!(
        !written.contains("passthrough_llm") && !written.contains("path_prefixes"),
        "a file without the exception looks like it always did:\n{written}"
    );
}

/// Ein Präfix ohne führenden `/` oder mit nur einem Zeichen wird abgelehnt.
///
/// Nicht stillschweigend weggelassen: Ein Präfix, das jeden Pfad trifft, hebt
/// die Grenze auf, um die es geht (HUM-039, Fallstricke).
#[test]
fn rules_005_rejects_a_path_prefix_that_is_no_boundary() {
    for prefix in ["\"\"", "\"/\"", "\"v1/\""] {
        let yaml = format!(
            "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"x.io\"\n      \
             path_prefixes: [{prefix}]\n"
        );
        let diagnostics = parse_rules(&yaml).expect_err("must be refused");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == RULES_005),
            "{prefix}: {diagnostics:?}"
        );
    }
}

/// `passthrough_llm` gehört zu `action: allow`; alles andere ist ein Fehler.
#[test]
fn passthrough_llm_only_goes_with_allow() {
    let yaml = "version: 1
rules:
  - action: block
    match:
      host: \"x.io\"
    passthrough_llm: true
";
    let diagnostics = parse_rules(yaml).expect_err("must be refused");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.why.contains("passthrough_llm")),
        "{diagnostics:?}"
    );
}

/// Eine Durchreiche muss genau ein Ziel nennen: einen Host, einen Port, ein
/// Schema und eine Pfadgrenze. Fehlt eine davon, warnt `RULES_008`.
///
/// Der Host wiegt am schwersten: Eine Durchreiche mit `host: "**"` reicht jeden
/// Host der Welt ungehalten durch und bleibt dabei aus der voreingestellten
/// Ansicht heraus. Genau diese Regel schwieg vor HUM-039, weil die Prüfung auf
/// die Pfadgrenze vorher mit einem `return` endete.
#[test]
fn rules_008_warns_about_a_passthrough_that_does_not_name_one_target() {
    let complete = "version: 1
rules:
  - action: allow
    match:
      host: \"ip:192.168.1.50\"
      port: 11434
      scheme: http
      path_prefixes: [\"/v1/chat/completions\"]
    passthrough_llm: true
";
    let (_set, warnings) = parse_rules(complete).unwrap_or_else(|d| panic!("{d:?}"));
    assert!(
        warnings.is_empty(),
        "a passthrough that names exactly one target is fine: {warnings:?}"
    );

    for (what, matcher) in [
        (
            "a host glob",
            "host: \"**\"\n      port: 11434\n      scheme: http\n      \
             path_prefixes: [\"/v1/chat/completions\"]",
        ),
        (
            "no port",
            "host: \"ip:192.168.1.50\"\n      scheme: http\n      \
             path_prefixes: [\"/v1/chat/completions\"]",
        ),
        (
            "no scheme",
            "host: \"ip:192.168.1.50\"\n      port: 11434\n      \
             path_prefixes: [\"/v1/chat/completions\"]",
        ),
        (
            "no path condition",
            "host: \"ip:192.168.1.50\"\n      port: 11434\n      scheme: http",
        ),
    ] {
        let yaml = format!(
            "version: 1\nrules:\n  - action: allow\n    match:\n      {matcher}\n    passthrough_llm: true\n"
        );
        let (_set, warnings) = parse_rules(&yaml).unwrap_or_else(|d| panic!("{what}: {d:?}"));
        assert!(
            warnings.iter().any(|warning| warning.code == RULES_008),
            "{what} must warn: {warnings:?}"
        );
    }
}

/// Eine Durchreiche mit Host-Glob warnt, auch wenn sie ein Pfadpräfix trägt.
///
/// Ohne diese Zeile schwieg `too_broad`: die Pfadprüfung war zufrieden, und der
/// frühe `return` übersprang die Prüfung auf „alles".
#[test]
fn rules_008_warns_about_a_passthrough_over_every_host() {
    let yaml = "version: 1
rules:
  - action: allow
    match:
      host: \"**\"
      path_prefixes: [\"/v1/chat/completions\"]
    passthrough_llm: true
";
    let (_set, warnings) = parse_rules(yaml).unwrap_or_else(|d| panic!("{d:?}"));
    let Some(warning) = warnings.iter().find(|warning| warning.code == RULES_008) else {
        panic!("a passthrough over every host must warn: {warnings:?}");
    };
    assert!(warning.why.contains("a single host"), "{}", warning.why);
}
