//! Die Beispiele aus `docs/reference/rules.md` gegen den echten Parser und die
//! echte Auswertung (HUM-059).
//!
//! Drei Zusagen der Referenzseite, die sonst niemand prüft:
//!
//! 1. Jeder ```` ```yaml ````-Block der Seite ist eine vollständige
//!    `rules.yaml`, die [`parse_rules_for_session`] annimmt.
//! 2. Jedes Probenpaar in einem Block
//!    (`# humanitl rules test <URL> [--method M] [--upgrade websocket]`,
//!    darunter `# verdict: <aktion>`) stimmt mit dem, was
//!    [`RuleSet::evaluate`] für genau diesen Block sagt.
//! 3. Der Block hinter der Marke `<!-- bundled-rules: rules/default.yaml -->`
//!    trägt dieselben Regeln wie `rules/default.yaml`, Feld für Feld.
//!
//! Die Seite wird über `include_str!` gelesen, damit eine Änderung an ihr den
//! Test neu übersetzt. Die Crate selbst öffnet keine Datei.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::Utc;
use humanitl_core::{HostName, Method, Scheme, SessionId, Upgrade};
use humanitl_rules::{RequestKey, RuleSet, parse_rules, parse_rules_for_session};

/// Die Referenzseite.
const REFERENCE: &str = include_str!("../../../../docs/reference/rules.md");

/// Der mitgelieferte Regelsatz, gegen den der Block der Seite verglichen wird.
const DEFAULT_RULES: &str = include_str!("../../../../rules/default.yaml");

/// Die Marke vor dem Block, der `rules/default.yaml` wiedergibt.
const BUNDLED_MARK: &str = "<!-- bundled-rules: rules/default.yaml -->";

/// So viele YAML-Blöcke hat die Seite mindestens. Eine leere oder halbierte
/// Seite soll auffallen, statt mit null geprüften Blöcken grün zu sein.
const MIN_BLOCKS: usize = 15;

/// So viele Proben trägt die Seite mindestens, aus demselben Grund.
const MIN_PROBES: usize = 60;

/// Ein YAML-Block der Seite.
#[derive(Debug)]
struct Block {
    /// Die 1-basierte Zeile des öffnenden Zauns.
    line: usize,
    /// Der Inhalt zwischen den Zäunen.
    text: String,
    /// Wahr, wenn direkt davor die Marke [`BUNDLED_MARK`] steht.
    bundled: bool,
}

/// Liest jeden ```` ```yaml ````-Block aus einem Markdown-Text.
///
/// Ein Block beginnt mit einer Zeile, die genau ```` ```yaml ```` ist, und
/// endet mit der nächsten Zeile, die genau ```` ``` ```` ist. Ein Block ohne
/// schließenden Zaun ist ein Fehler der Seite und lässt den Test scheitern.
fn yaml_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;
    let mut previous = "";
    for (index, line) in markdown.lines().enumerate() {
        match current.as_mut() {
            None if line.trim_end() == "```yaml" => {
                current = Some(Block {
                    line: index + 1,
                    text: String::new(),
                    bundled: previous.trim() == BUNDLED_MARK,
                });
            }
            None => {}
            Some(_) if line.trim_end() == "```" => {
                blocks.extend(current.take());
            }
            Some(block) => {
                block.text.push_str(line);
                block.text.push('\n');
            }
        }
        previous = line;
    }
    if let Some(Block { line, .. }) = current {
        panic!("the yaml block at line {line} is never closed");
    }
    blocks
}

/// Eine Probe aus einem Block.
#[derive(Debug)]
struct Probe {
    url: String,
    method: Method,
    upgrade: Option<Upgrade>,
    expected: String,
}

/// Liest die Probenpaare eines Blocks.
///
/// Jede Zeile `# humanitl rules test …` braucht als nächste Zeile ein
/// `# verdict: …`. Ein unbekanntes Flag ist ein Fehler: Was die Seite als
/// Aufruf zeigt, soll es auch geben.
fn probes(block: &Block) -> Vec<Probe> {
    let lines: Vec<&str> = block.text.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(command) = line.trim().strip_prefix("# humanitl rules test ") else {
            continue;
        };
        let place = format!("block at line {}, probe {command:?}", block.line);
        let mut words = command.split_whitespace();
        let Some(url) = words.next() else {
            panic!("{place}: no url");
        };
        let mut method = Method::GET;
        let mut upgrade = None;
        while let Some(flag) = words.next() {
            match flag {
                "--method" => {
                    let Some(name) = words.next() else {
                        panic!("{place}: --method without a value");
                    };
                    let upper = name.to_ascii_uppercase();
                    let Ok(parsed) = Method::from_bytes(upper.as_bytes()) else {
                        panic!("{place}: {name:?} is not a method");
                    };
                    method = parsed;
                }
                "--upgrade" => match words.next() {
                    Some("websocket") => upgrade = Some(Upgrade::WebSocket),
                    other => panic!("{place}: --upgrade takes websocket, not {other:?}"),
                },
                other => panic!("{place}: unknown flag {other:?}"),
            }
        }
        let next = lines.get(index + 1).copied().unwrap_or_default();
        let Some(expected) = next.trim().strip_prefix("# verdict: ") else {
            panic!("{place}: the next line must be `# verdict: <action>`");
        };
        found.push(Probe {
            url: url.to_owned(),
            method,
            upgrade,
            expected: expected.trim().to_owned(),
        });
    }
    found
}

/// Die Teile einer Proben-URL, die eine Regel vergleicht.
struct Target {
    scheme: Scheme,
    host: HostName,
    port: u16,
    path: String,
}

/// Liest einen Port aus einer Proben-URL.
fn port_of(url: &str, text: &str) -> u16 {
    text.parse()
        .unwrap_or_else(|_| panic!("{url:?}: {text:?} is not a port"))
}

/// Zerlegt eine URL der Form `scheme://host[:port][/path]`.
///
/// Die Seite benutzt nur `http` und `https`; mehr braucht dieser Test nicht.
/// Die Normalisierung des Hosts macht [`HostName::parse`], dieselbe wie im
/// Daemon.
fn target(url: &str) -> Target {
    let Some((scheme, rest)) = url.split_once("://") else {
        panic!("{url:?} has no scheme");
    };
    let (scheme, default_port) = match scheme {
        "http" => (Scheme::Http, 80),
        "https" => (Scheme::Https, 443),
        other => panic!("{url:?}: scheme {other:?} is not used on the reference page"),
    };
    let (authority, path) = match rest.find(['/', '?']) {
        Some(at) => (&rest[..at], rest[at..].to_owned()),
        None => (rest, "/".to_owned()),
    };
    let path = if path.starts_with('?') {
        format!("/{path}")
    } else {
        path
    };
    let (host, port) = if authority.starts_with('[') {
        match authority.split_once("]:") {
            Some((host, port)) => (format!("{host}]"), Some(port)),
            None => (authority.to_owned(), None),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port)),
            None => (authority.to_owned(), None),
        }
    };
    let port = port.map_or(default_port, |text| port_of(url, text));
    let Ok(host) = HostName::parse(&host) else {
        panic!("{url:?}: {host:?} is not a host");
    };
    Target {
        scheme,
        host,
        port,
        path,
    }
}

/// Wertet eine Probe gegen einen Regelsatz aus, wie `humanitl rules test`.
fn verdict(rules: &RuleSet, probe: &Probe, session: SessionId) -> String {
    let to = target(&probe.url);
    let mut key = RequestKey::new(&to.host, &probe.method, &to.path, to.scheme, to.port);
    if let Some(upgrade) = probe.upgrade {
        key = key.with_upgrade(upgrade);
    }
    rules
        .evaluate(&key, Utc::now(), session)
        .action()
        .to_string()
}

/// Liest einen Regelsatz, der gültig sein muss.
fn parsed(text: &str, what: &str) -> RuleSet {
    match parse_rules(text) {
        Ok((rules, _warnings)) => rules,
        Err(diagnostics) => panic!("{what} does not parse: {diagnostics:?}"),
    }
}

#[test]
fn rules_reference_examples() {
    let blocks = yaml_blocks(REFERENCE);
    let count = blocks.len();
    assert!(
        count >= MIN_BLOCKS,
        "docs/reference/rules.md has {count} yaml blocks, expected at least {MIN_BLOCKS}"
    );

    let mut failures = Vec::new();
    let mut probe_count = 0;
    for block in &blocks {
        let line = block.line;
        let session = SessionId::new();
        let rules = match parse_rules_for_session(&block.text, session) {
            Ok((rules, _warnings)) => rules,
            Err(diagnostics) => {
                let why: Vec<&str> = diagnostics.iter().map(|d| d.why.as_str()).collect();
                failures.push(format!("yaml block at line {line} does not parse: {why:?}"));
                continue;
            }
        };
        for probe in probes(block) {
            probe_count += 1;
            let actual = verdict(&rules, &probe, session);
            if actual != probe.expected {
                let url = &probe.url;
                let method = &probe.method;
                let expected = &probe.expected;
                failures.push(format!(
                    "yaml block at line {line}: `humanitl rules test {url}` with method \
                     {method} gives `{actual}`, the page says `{expected}`"
                ));
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        probe_count >= MIN_PROBES,
        "docs/reference/rules.md has {probe_count} probes, expected at least {MIN_PROBES}"
    );
}

#[test]
fn rules_reference_bundled_block_matches_default_yaml() {
    let blocks = yaml_blocks(REFERENCE);
    let bundled: Vec<&Block> = blocks.iter().filter(|block| block.bundled).collect();
    assert_eq!(
        bundled.len(),
        1,
        "exactly one yaml block must follow {BUNDLED_MARK}"
    );

    // `rules/default.yaml` trägt feste Ids, und die Seite muss dieselben
    // tragen: Ohne Id bekäme eine Regel bei jedem Lesen eine neue, und der
    // Vergleich schlüge immer fehl.
    let page = parsed(&bundled[0].text, "the bundled block of the page");
    let shipped = parsed(DEFAULT_RULES, "rules/default.yaml");
    assert!(
        page == shipped,
        "the bundled block of docs/reference/rules.md differs from rules/default.yaml"
    );
}

#[test]
fn the_extractor_sees_every_block_and_only_yaml() {
    let markdown = "\
# Title

```yaml
version: 1
```

```text
not: [yaml
```

<!-- bundled-rules: rules/default.yaml -->
```yaml
version: 1
rules: []
# humanitl rules test https://example.com:8443/a?b=c --method post --upgrade websocket
# verdict: ask
```
";
    let blocks = yaml_blocks(markdown);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].line, 3);
    assert!(!blocks[0].bundled);
    assert!(blocks[1].bundled);

    let found = probes(&blocks[1]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].method, Method::POST);
    assert_eq!(found[0].upgrade, Some(Upgrade::WebSocket));
    assert_eq!(found[0].expected, "ask");

    let split = target(&found[0].url);
    assert_eq!(split.port, 8443);
    assert_eq!(split.path, "/a?b=c");
    assert_eq!(split.scheme, Scheme::Https);
}
