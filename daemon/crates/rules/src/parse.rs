//! `rules.yaml` lesen und schreiben.
//!
//! Gelesen wird streng: ein unbekannter Schlüssel, eine unbekannte Methode oder
//! ein Muster, das sich nicht übersetzen lässt, sind Fehler, keine stillen
//! Vorgaben. Eine Regel-Datei ist Sicherheitskonfiguration; ein Tippfehler
//! darin muss auffallen, statt eine Regel wirkungslos zu machen, an die sich
//! jemand hält.
//!
//! Ein Fehler lehnt die ganze Datei ab ([`Err`]). Der Daemon läuft dann mit dem
//! zuletzt gültigen Regelsatz weiter und zeigt den Befund; ein halb geladener
//! Regelsatz wäre die schlechtere Antwort, weil niemand ihm ansieht, welche
//! Hälfte fehlt. Warnungen kommen mit dem Regelsatz zusammen zurück.

use chrono::{DateTime, Utc};
use humanitl_core::diagnostics::codes::{RULES_001, RULES_006, RULES_007, RULES_008};
use humanitl_core::rule::{Action, Expiry, HostPattern, Matcher, PathPattern, Rule};
use humanitl_core::{Diagnostic, FlowId, Method, RuleId, Scheme, SessionId, Severity, Upgrade};
use serde::{Deserialize, Serialize};

use crate::eval::{RuleSet, is_known_method};
use crate::host;
use crate::path::PathMatcher;

/// Die einzige Fassung des Dateiformats, die es gibt.
pub const RULES_VERSION: u32 = 1;

/// Liest einen Regelsatz aus YAML.
///
/// `expires: session` ohne Sitzungs-Id gehört zu keiner laufenden Sitzung: die
/// Regel wird geladen, gilt aber nirgends und verschwindet beim nächsten
/// [`RuleSet::prune`]. Das ist beabsichtigt (HUM-027 schreibt Sitzungsregeln
/// gar nicht erst in die Datei). Wer eine Datei für eine laufende Sitzung
/// einliest, ruft [`parse_rules_for_session`].
///
/// # Errors
///
/// Alle gefundenen Befunde, sobald einer davon [`Severity::Error`] trägt: die
/// Datei wird als Ganzes abgelehnt.
pub fn parse_rules(yaml: &str) -> Result<(RuleSet, Vec<Diagnostic>), Vec<Diagnostic>> {
    parse_rules_for_session(yaml, SessionId::new())
}

/// Liest einen Regelsatz aus YAML und setzt für `expires: session` diese Sitzung ein.
///
/// # Errors
///
/// Wie [`parse_rules`].
pub fn parse_rules_for_session(
    yaml: &str,
    session: SessionId,
) -> Result<(RuleSet, Vec<Diagnostic>), Vec<Diagnostic>> {
    let file: RulesFile = match serde_yaml_ng::from_str(yaml) {
        Ok(file) => file,
        Err(err) => {
            return Err(vec![
                Diagnostic::builder(RULES_001, Severity::Error)
                    .why(format!("rules.yaml is not valid: {err}"))
                    .build(),
            ]);
        }
    };

    let mut diagnostics = Vec::new();
    let source = Source::new(yaml);

    match file.version {
        Some(RULES_VERSION) => {}
        Some(other) => diagnostics.push(
            Diagnostic::builder(RULES_006, Severity::Error)
                .why(format!(
                    "{}: rules.yaml has version {other}, and only version {RULES_VERSION} exists",
                    place("version", source.top_key("version"))
                ))
                .build(),
        ),
        None => diagnostics.push(
            Diagnostic::builder(RULES_006, Severity::Error)
                .why(format!(
                    "rules.yaml has no `version` key; write `version: {RULES_VERSION}`"
                ))
                .build(),
        ),
    }

    let mut rules: Vec<Rule> = Vec::with_capacity(file.rules.len());
    for (index, raw) in file.rules.into_iter().enumerate() {
        if let Some(rule) = raw.into_rule(index, session, &source, &mut diagnostics) {
            if rules.iter().any(|existing| existing.id == rule.id) {
                let line = source
                    .field(index, "id")
                    .or_else(|| source.rule_start(index));
                diagnostics.push(
                    Diagnostic::builder(RULES_007, Severity::Error)
                        .why(format!(
                            "{}: the id {} is already taken; every rule needs its own",
                            at(index, "id", line),
                            rule.id
                        ))
                        .build(),
                );
            }
            if let Some(warning) = too_broad(index, &rule) {
                diagnostics.push(warning);
            }
            rules.push(rule);
        }
    }

    if diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.severity, Severity::Error | Severity::Blocking))
    {
        return Err(diagnostics);
    }
    Ok((RuleSet::from_rules(rules), diagnostics))
}

/// Schreibt einen Regelsatz als YAML.
///
/// Die Feldreihenfolge ist fest und entspricht `backlog/CONVENTIONS.md` 3.3,
/// jede Regel trägt ihre Id, und `expires: session` wird ohne Sitzungs-Id
/// geschrieben: eine Sitzung von gestern gibt es morgen nicht mehr.
#[must_use]
pub fn serialize_rules(set: &RuleSet) -> String {
    let file = OutFile {
        version: RULES_VERSION,
        rules: set.iter().map(OutRule::from_rule).collect(),
    };
    serde_yaml_ng::to_string(&file).unwrap_or_else(|err| {
        // Die Struktur besteht aus Zeichenketten, Zahlen und Wahrheitswerten;
        // sie kann nicht scheitern. Der Zweig steht hier, weil ein `unwrap` in
        // dieser Codebasis nicht vorkommt (CONVENTIONS 3.12).
        format!("# rules.yaml could not be written: {err}\nversion: {RULES_VERSION}\nrules: []\n")
    })
}

/// Findet die Zeile eines Schlüssels im Quelltext.
///
/// `serde_yaml_ng` meldet die Stelle nur für Syntax- und Schemafehler. Alles,
/// was erst beim Umwandeln auffällt (ein Host-Muster, ein regulärer Ausdruck,
/// eine doppelte Id), bekommt seine Zeile hier — und zwar über den
/// **Schlüssel**, nicht über den Wert: ein leerer Wert (`host: ""`) und ein
/// zweimal vergebener Wert haben keine eigene Fundstelle, ein Schlüssel schon.
///
/// Dafür werden beim Einlesen die Zeilenbereiche der Regeln bestimmt: der
/// Listenpunkt `- ` auf der Einrückung des ersten Punktes unter `rules:`
/// beginnt eine Regel, alles bis zum nächsten gehört zu ihr. Tiefer eingerückte
/// Listen (`method:` über mehrere Zeilen) zählen deshalb nicht als neue Regel.
struct Source<'a> {
    lines: Vec<&'a str>,
    rules: Vec<usize>,
}

impl<'a> Source<'a> {
    fn new(text: &'a str) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let mut rules = Vec::new();
        let mut item_indent: Option<usize> = None;
        let mut in_rules = false;

        for (number, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let indent = line.len() - trimmed.len();
            if !in_rules {
                in_rules = indent == 0 && key_starts(trimmed, "rules");
                continue;
            }
            if !trimmed.starts_with("- ") {
                continue;
            }
            match item_indent {
                None => {
                    item_indent = Some(indent);
                    rules.push(number);
                }
                Some(expected) if expected == indent => rules.push(number),
                Some(_) => {}
            }
        }

        Self { lines, rules }
    }

    /// Die 1-basierte Zeile eines Schlüssels der obersten Ebene.
    fn top_key(&self, key: &str) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| !line.starts_with(char::is_whitespace) && key_starts(line, key))
            .map(|found| found + 1)
    }

    /// Die 1-basierte Zeile, in der die Regel beginnt.
    fn rule_start(&self, index: usize) -> Option<usize> {
        self.rules.get(index).map(|start| start + 1)
    }

    /// Die 1-basierte Zeile eines Schlüssels innerhalb einer Regel.
    fn field(&self, index: usize, key: &str) -> Option<usize> {
        let start = *self.rules.get(index)?;
        let end = self
            .rules
            .get(index + 1)
            .copied()
            .unwrap_or(self.lines.len());
        self.lines[start..end]
            .iter()
            .position(|line| key_starts(line.trim_start(), key))
            .map(|offset| start + offset + 1)
    }
}

/// Wahr, wenn die Zeile mit diesem Schlüssel beginnt (`host: …`).
///
/// Ein führender Listenpunkt wird übersprungen, damit `- action: allow` den
/// Schlüssel `action` trägt.
fn key_starts(line: &str, key: &str) -> bool {
    let text = line.trim_start();
    let text = text.strip_prefix("- ").map_or(text, str::trim_start);
    text.strip_prefix(key)
        .is_some_and(|rest| rest.starts_with(':'))
}

/// `rules[3].match.host (line 12)`, soweit die Zeile bekannt ist.
fn at(index: usize, field: &str, line: Option<usize>) -> String {
    place(&format!("rules[{index}].{field}"), line)
}

/// Ein Feldpfad mit seiner Zeile, soweit sie bekannt ist.
fn place(field: &str, line: Option<usize>) -> String {
    match line {
        Some(line) => format!("{field} (line {line})"),
        None => field.to_owned(),
    }
}

fn schema_error(index: usize, field: &str, line: Option<usize>, detail: &str) -> Diagnostic {
    Diagnostic::builder(RULES_001, Severity::Error)
        .why(format!("{}: {detail}", at(index, field, line)))
        .build()
}

/// Eine Regel, die mehr erlaubt, als sie vermutlich soll.
///
/// Jede weitere Bedingung schränkt ein, auch `upgrade: websocket`: eine Regel,
/// die nur WebSocket-Upgrades trifft, hebt die Moderation für gewöhnliche
/// Anfragen nicht auf.
fn too_broad(index: usize, rule: &Rule) -> Option<Diagnostic> {
    let matcher = &rule.matcher;
    let everything = matches!(&matcher.host, HostPattern::Glob(glob) if glob == "**")
        && matcher.methods.is_none()
        && matcher.path.is_none()
        && matcher.scheme.is_none()
        && matcher.port.is_none()
        && matcher.upgrade.is_none();
    if rule.action == Action::Allow && everything {
        return Some(
            Diagnostic::builder(RULES_008, Severity::Warning)
                .why(format!(
                    "rules[{index}] allows every host without any further condition; \
                     the moderation is off for everything below it"
                ))
                .build(),
        );
    }
    None
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    version: Option<u32>,
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    #[serde(default)]
    id: Option<String>,
    action: Action,
    #[serde(rename = "match")]
    matcher: RawMatch,
    #[serde(default)]
    expires: Option<String>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    allow_private: bool,
    #[serde(default)]
    created_from: Option<String>,
    #[serde(default)]
    bundled: bool,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatch {
    host: String,
    #[serde(default)]
    method: Option<Vec<String>>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    scheme: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    upgrade: Option<String>,
}

impl RawRule {
    /// Wandelt eine gelesene Regel um; `None`, wenn dabei ein Fehler auffiel.
    ///
    /// Die Befunde landen in `diagnostics`, auch wenn die Regel entfällt: der
    /// Mensch soll alle Fehler einer Datei auf einmal sehen, nicht einen pro
    /// Durchlauf.
    fn into_rule(
        self,
        index: usize,
        session: SessionId,
        source: &Source<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Rule> {
        let mut failed = false;

        let id = match self.id.as_deref() {
            None => RuleId::new(),
            Some(text) => {
                let line = source.field(index, "id");
                match RuleId::parse(text) {
                    Ok(id) => id,
                    Err(err) => {
                        diagnostics.push(schema_error(index, "id", line, &err.to_string()));
                        failed = true;
                        RuleId::new()
                    }
                }
            }
        };

        let matcher = self.matcher.into_matcher(index, source, diagnostics);

        let expires = match self.expires.as_deref() {
            None => Expiry::Never,
            Some(text) => {
                let line = source.field(index, "expires");
                if let Some(expiry) = parse_expiry(text, session) {
                    expiry
                } else {
                    diagnostics.push(schema_error(
                        index,
                        "expires",
                        line,
                        &format!(
                            "{text:?} is neither `never`, `session` nor an ISO-8601 timestamp"
                        ),
                    ));
                    failed = true;
                    Expiry::Never
                }
            }
        };

        let created_from = match self.created_from.as_deref() {
            None => None,
            Some(text) => {
                let line = source.field(index, "created_from");
                match FlowId::parse(text) {
                    Ok(flow) => Some(flow),
                    Err(err) => {
                        diagnostics.push(schema_error(
                            index,
                            "created_from",
                            line,
                            &err.to_string(),
                        ));
                        failed = true;
                        None
                    }
                }
            }
        };

        let matcher = matcher?;
        if failed {
            return None;
        }

        let mut rule = Rule::new(id, self.action, matcher)
            .with_expiry(expires)
            .with_stream(self.stream)
            .with_allow_private(self.allow_private)
            .bundled(self.bundled);
        rule.created_from = created_from;
        rule.note = self.note;
        Some(rule)
    }
}

impl RawMatch {
    /// Wandelt die Bedingung um; `None`, wenn eines ihrer Felder ungültig ist.
    fn into_matcher(
        self,
        index: usize,
        source: &Source<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Matcher> {
        let mut failed = false;

        let host_line = source.field(index, "host");
        let host = match host::parse_pattern(&self.host) {
            Ok((pattern, warnings)) => {
                for warning in warnings {
                    diagnostics.push(with_place(warning, index, "match.host", host_line));
                }
                Some(pattern)
            }
            Err(error) => {
                diagnostics.push(with_place(error, index, "match.host", host_line));
                failed = true;
                None
            }
        };

        let methods = self
            .method
            .map(|raw| parse_methods(index, raw, source, diagnostics, &mut failed));

        let path = match self.path.as_deref() {
            None => None,
            Some(text) => {
                let line = source.field(index, "path");
                let pattern = PathPattern::parse(text);
                match PathMatcher::compile(&pattern) {
                    Ok(_) => Some(pattern),
                    Err(error) => {
                        diagnostics.push(with_place(error, index, "match.path", line));
                        failed = true;
                        None
                    }
                }
            }
        };

        let scheme = match self.scheme.as_deref() {
            None => None,
            Some(text) => {
                let line = source.field(index, "scheme");
                let parsed = Scheme::parse(text);
                if parsed.is_none() {
                    diagnostics.push(schema_error(
                        index,
                        "match.scheme",
                        line,
                        &format!("{text:?} is not http, https, ws or wss"),
                    ));
                    failed = true;
                }
                parsed
            }
        };

        let port = match self.port {
            None => None,
            Some(0) => {
                diagnostics.push(schema_error(
                    index,
                    "match.port",
                    source.field(index, "port"),
                    "0 is not a port; the range is 1..=65535",
                ));
                failed = true;
                None
            }
            Some(port) => Some(port),
        };

        let upgrade = match self.upgrade.as_deref() {
            None => None,
            Some(text) if text.eq_ignore_ascii_case(Upgrade::WebSocket.as_str()) => {
                Some(Upgrade::WebSocket)
            }
            Some(text) => {
                let line = source.field(index, "upgrade");
                diagnostics.push(schema_error(
                    index,
                    "match.upgrade",
                    line,
                    &format!("{text:?} is not a protocol upgrade; only `websocket` exists"),
                ));
                failed = true;
                None
            }
        };

        let host = host?;
        if failed {
            return None;
        }

        let mut matcher = Matcher::host(host);
        matcher.methods = methods;
        matcher.path = path;
        matcher.scheme = scheme;
        matcher.port = port;
        matcher.upgrade = upgrade;
        Some(matcher)
    }
}

/// Liest die Methodenliste: unabhängig von Groß- und Kleinschreibung, intern
/// immer in Großbuchstaben, und nur aus der bekannten Menge.
fn parse_methods(
    index: usize,
    raw: Vec<String>,
    source: &Source<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    failed: &mut bool,
) -> Vec<Method> {
    let line = source.field(index, "method");
    let mut methods = Vec::with_capacity(raw.len());
    for text in raw {
        match Method::from_bytes(text.to_ascii_uppercase().as_bytes()) {
            Ok(method) if is_known_method(&method) => methods.push(method),
            Ok(_) | Err(_) => {
                diagnostics.push(schema_error(
                    index,
                    "match.method",
                    line,
                    &format!("{text:?} is not one of the known HTTP methods"),
                ));
                *failed = true;
            }
        }
    }
    methods
}

/// Ergänzt einen Befund aus `host`/`path` um Regel, Feld und Zeile.
fn with_place(
    diagnostic: Diagnostic,
    index: usize,
    field: &str,
    line: Option<usize>,
) -> Diagnostic {
    let mut builder = Diagnostic::builder(diagnostic.code, diagnostic.severity)
        .why(format!("{}: {}", at(index, field, line), diagnostic.why))
        .title(diagnostic.title);
    if let Some(fix) = diagnostic.fix {
        builder = builder.fix(fix);
    }
    if let Some(docs) = diagnostic.docs {
        builder = builder.docs(docs);
    }
    builder.build()
}

fn parse_expiry(text: &str, session: SessionId) -> Option<Expiry> {
    match text {
        "never" => Some(Expiry::Never),
        "session" => Some(Expiry::Session(session)),
        stamp => DateTime::parse_from_rfc3339(stamp)
            .ok()
            .map(|at| Expiry::At(at.with_timezone(&Utc))),
    }
}

#[derive(Debug, Serialize)]
struct OutFile {
    version: u32,
    rules: Vec<OutRule>,
}

#[derive(Debug, Serialize)]
struct OutRule {
    id: String,
    action: Action,
    #[serde(rename = "match")]
    matcher: OutMatch,
    expires: String,
    stream: bool,
    allow_private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_from: Option<String>,
    bundled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct OutMatch {
    host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upgrade: Option<String>,
}

impl OutRule {
    fn from_rule(rule: &Rule) -> Self {
        Self {
            id: rule.id.to_string(),
            action: rule.action,
            matcher: OutMatch {
                host: rule.matcher.host.to_string(),
                method: rule
                    .matcher
                    .methods
                    .as_ref()
                    .map(|methods| methods.iter().map(|m| m.as_str().to_owned()).collect()),
                path: rule.matcher.path.as_ref().map(ToString::to_string),
                scheme: rule.matcher.scheme.map(|s| s.as_str().to_owned()),
                port: rule.matcher.port,
                upgrade: rule.matcher.upgrade.map(|u| u.as_str().to_owned()),
            },
            expires: match rule.expires {
                Expiry::Never => "never".to_owned(),
                Expiry::Session(_) => "session".to_owned(),
                Expiry::At(at) => at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            },
            stream: rule.stream,
            allow_private: rule.allow_private,
            created_from: rule.created_from.map(|flow| flow.to_string()),
            bundled: rule.bundled,
            note: rule.note.clone(),
        }
    }
}
