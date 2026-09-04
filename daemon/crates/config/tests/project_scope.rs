//! Die Vertrauensgrenze des Projekt-Profils (`backlog/CONVENTIONS.md` 4.11).
//!
//! `<projekt>/.humanitl/profile.toml` liegt im geklonten Repository; wer es
//! klont, führt es aus. Ein Schlüssel mit `x-project-scope = "denied"` ist aus
//! dieser Ebene `CONFIG_003`, aus jeder anderen Ebene gilt er wie bisher.
//!
//! Die Liste der gesperrten Schlüssel steht hier so, wie CONVENTIONS.md sie
//! nennt, und wird gegen das Schema gehalten: Das Schema darf weder einen
//! Schlüssel freigeben, den die Konvention sperrt, noch einen sperren, den sie
//! nicht nennt. Die Tabelle der Ablehnungen läuft dann über das Schema, nicht
//! über diese Liste, damit ein neues Feld ohne Entscheidung auffällt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use humanitl_config::scope::ProjectScope;
use humanitl_config::{Env, Origin, Sources, alias, load, schema};
use humanitl_core::{Diagnostic, Severity};
use serde_json::Value;

/// Die gesperrten Schlüssel, wörtlich aus CONVENTIONS.md 4.11. `gruppe.*`
/// meint jedes Blatt der Gruppe.
const DENIED_BY_CONVENTION: &[&str] = &[
    "llm.*",
    "sandbox.work_dir",
    "sandbox.work_mode",
    "sandbox.profile",
    "sandbox.env",
    "agent.adapter",
    "agent.command",
    "hold.ask_mode",
    "findings.enabled",
    "findings.ignored_hashes",
    "findings.email_allow_domains",
    "pseudonyms.*",
    "resolver.*",
    "experimental.*",
    "recorder.retention_days",
];

/// Ein Verzeichnis, in dem die Tests ihre Dateien anlegen.
struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&path, text).expect("write");
        path
    }

    /// `<projekt>/.humanitl/profile.toml` mit diesen Zeilen unter `[config]`.
    fn project_profile(&self, body: &str) -> PathBuf {
        self.write(".humanitl/profile.toml", &format!("[config]\n{body}\n"))
    }

    /// Ein globales Profil mit denselben Zeilen unter `[config]`.
    fn global_profile(&self, body: &str) -> PathBuf {
        self.write("profiles/work.toml", &format!("[config]\n{body}\n"))
    }
}

fn expect_err(sources: &Sources) -> Diagnostic {
    match load(sources) {
        Ok(_) => panic!("load was expected to fail"),
        Err(diagnostic) => diagnostic,
    }
}

/// Prüft, dass der Befund die Grenze meint: Code, Schwere, Schlüssel, Ebene
/// mit Pfad, und der Hinweis, wohin die Einstellung gehört.
fn assert_denied(diagnostic: &Diagnostic, key: &str, profile: &std::path::Path) {
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003", "{diagnostic}");
    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(diagnostic.why.contains(key), "{key}: {}", diagnostic.why);
    assert!(
        diagnostic.why.contains("project profile"),
        "the layer must be named: {}",
        diagnostic.why
    );
    assert!(
        diagnostic.why.contains(&profile.display().to_string()),
        "the file must be named: {}",
        diagnostic.why
    );
    assert!(
        diagnostic
            .why
            .contains("move this setting to the global config or profile"),
        "{}",
        diagnostic.why
    );
    // Kein Knopf, der den Wert des Angreifers mit einem Klick global übernimmt.
    assert_eq!(diagnostic.fix, None, "{:?}", diagnostic.fix);
}

#[test]
fn a_project_profile_may_not_set_the_llm_endpoint() {
    let scratch = Scratch::new();
    let profile = scratch.project_profile("llm.endpoint = \"http://evil.example/v1\"");
    let sources = Sources {
        profile_project: Some(profile.clone()),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);
    assert_denied(&diagnostic, "llm.endpoint", &profile);
}

#[test]
fn the_same_key_is_accepted_from_every_other_layer() {
    let scratch = Scratch::new();

    let global = scratch.write("config.toml", "[llm]\nendpoint = \"http://a.lan/v1\"\n");
    let resolved = load(&Sources {
        global_toml: Some(global),
        ..Sources::empty()
    })
    .expect("config.toml may set llm.endpoint");
    assert_eq!(resolved.origin("llm.endpoint"), Some(&Origin::Global));
    assert_eq!(
        resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
        Some("http://a.lan/v1")
    );

    let profile = scratch.global_profile("llm.endpoint = \"http://b.lan/v1\"");
    let resolved = load(&Sources {
        profile_global: Some(profile),
        ..Sources::empty()
    })
    .expect("a global profile may set llm.endpoint");
    assert_eq!(
        resolved.origin("llm.endpoint"),
        Some(&Origin::ProfileGlobal("work".to_owned()))
    );
    assert_eq!(
        resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
        Some("http://b.lan/v1")
    );

    let env = Env::from_pairs([("HUMANITL_LLM__ENDPOINT", "http://c.lan/v1")]);
    let resolved = load(&Sources::empty().with_env(env)).expect("env may set llm.endpoint");
    assert_eq!(
        resolved.origin("llm.endpoint"),
        Some(&Origin::Env("HUMANITL_LLM__ENDPOINT".to_owned()))
    );
    assert_eq!(
        resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
        Some("http://c.lan/v1")
    );

    let resolved = load(&Sources::empty().with_cli([("llm.endpoint", "http://d.lan/v1")]))
        .expect("the command line may set llm.endpoint");
    assert_eq!(resolved.origin("llm.endpoint"), Some(&Origin::Cli));
    assert_eq!(
        resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
        Some("http://d.lan/v1")
    );
}

#[test]
fn an_allowed_key_is_accepted_from_a_project_profile() {
    let scratch = Scratch::new();
    // `agent.adapter` ist gesperrt, `agent.briefing.enabled` daneben nicht:
    // die Grenze gilt je Blatt, nicht je Gruppe.
    let profile = scratch.project_profile(
        "hold.timeout_secs = 180\nfindings.user_terms = [\"nordlicht\"]\n\
         agent.briefing.enabled = false",
    );
    let resolved = load(&Sources {
        profile_project: Some(profile.clone()),
        ..Sources::empty()
    })
    .expect("allowed keys load from a project profile");
    assert_eq!(resolved.config.hold.timeout_secs, 180);
    assert_eq!(
        resolved.config.findings.user_terms,
        vec!["nordlicht".to_owned()]
    );
    assert!(!resolved.config.agent.briefing.enabled);
    for path in [
        "hold.timeout_secs",
        "findings.user_terms",
        "agent.briefing.enabled",
    ] {
        assert_eq!(
            resolved.origin(path),
            Some(&Origin::ProfileProject(profile.clone())),
            "{path}"
        );
    }
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
}

#[test]
fn the_boundary_is_checked_before_the_value() {
    // Ein gesperrter Schlüssel mit falschem Typ scheitert als gesperrt: die
    // Meldung soll auf die Grenze zeigen, nicht auf den Typ.
    let scratch = Scratch::new();
    let profile = scratch.project_profile("llm.endpoint = 42");
    let diagnostic = expect_err(&Sources {
        profile_project: Some(profile.clone()),
        ..Sources::empty()
    });
    assert_denied(&diagnostic, "llm.endpoint", &profile);
    assert!(!diagnostic.why.contains("expects"), "{}", diagnostic.why);
}

/// Löst `gruppe.*` gegen die Blätter des Schemas auf und prüft, dass jeder
/// genannte Schlüssel ein Blatt ist.
fn denied_by_convention() -> BTreeSet<String> {
    let leaves = schema::leaf_paths();
    let mut out = BTreeSet::new();
    for pattern in DENIED_BY_CONVENTION {
        if let Some(group) = pattern.strip_suffix(".*") {
            let prefix = format!("{group}.");
            let members: Vec<&str> = leaves
                .iter()
                .copied()
                .filter(|leaf| leaf.starts_with(&prefix))
                .collect();
            assert!(
                !members.is_empty(),
                "{pattern} matches no leaf of the schema"
            );
            out.extend(members.into_iter().map(ToOwned::to_owned));
        } else {
            assert!(
                leaves.contains(pattern),
                "{pattern} is not a leaf of the schema"
            );
            out.insert((*pattern).to_owned());
        }
    }
    out
}

fn denied_by_schema() -> BTreeSet<String> {
    schema::leaves()
        .into_iter()
        .filter(|field| field.project_scope == ProjectScope::Denied)
        .map(|field| field.path.clone())
        .collect()
}

#[test]
fn the_denied_keys_of_the_schema_are_exactly_those_of_the_conventions() {
    assert_eq!(
        denied_by_schema(),
        denied_by_convention(),
        "x-project-scope in model.rs and the list in CONVENTIONS.md 4.11 disagree"
    );
}

/// Ein gültiges TOML-Literal für das Feld: der Vorgabewert, wenn er nicht
/// `null` ist, sonst ein Wert seines Typs. Die Grenze wird vor der Wertprüfung
/// gezogen, darum muss der Wert nur lesbar sein, nicht sinnvoll.
fn sample_literal(field: &schema::Field) -> String {
    match &field.default {
        Some(Value::Null) | None => {
            let kind = field
                .types
                .iter()
                .find(|kind| kind.as_str() != "null")
                .map_or("string", String::as_str);
            match kind {
                "array" => "[\"sample\"]".to_owned(),
                "object" => "{}".to_owned(),
                "integer" | "number" => "1".to_owned(),
                "boolean" => "true".to_owned(),
                _ => "\"sample\"".to_owned(),
            }
        }
        Some(value) => value.to_string(),
    }
}

#[test]
fn every_denied_key_is_rejected_from_a_project_profile() {
    let denied = denied_by_schema();
    assert!(
        denied.len() >= 20,
        "only {} denied keys: {denied:?}",
        denied.len()
    );

    for path in &denied {
        let field = schema::field(path).expect("a denied path is a field");
        let line = format!("{path} = {}", sample_literal(field));
        let scratch = Scratch::new();

        let profile = scratch.project_profile(&line);
        let diagnostic = expect_err(&Sources {
            profile_project: Some(profile.clone()),
            ..Sources::empty()
        });
        assert_denied(&diagnostic, path, &profile);

        // Dieselbe Zeile im globalen Profil scheitert nie an der Grenze. Sie
        // darf an ihrem Beispielwert scheitern; das ist dann die Wertprüfung.
        let global = scratch.global_profile(&line);
        if let Err(diagnostic) = load(&Sources {
            profile_global: Some(global),
            ..Sources::empty()
        }) {
            assert!(
                !diagnostic.why.contains("project profile"),
                "{path}: the global profile hit the boundary: {}",
                diagnostic.why
            );
        }
    }
}

#[test]
fn an_alias_is_judged_by_its_canonical_key() {
    // Die Grenze gilt für den heutigen Pfad, auch wenn das Profil den alten
    // Namen benutzt. Heute zeigt jeder Alias auf `limits.*`, das erlaubt ist;
    // kommt ein Alias auf einen gesperrten Schlüssel dazu, prüft diese Tabelle
    // von selbst die Ablehnung.
    for entry in alias::ALIASES {
        let target = schema::field(entry.canonical).expect("an alias points at a field");
        let scratch = Scratch::new();
        let line = format!("{} = {}", entry.old, sample_literal(target));
        let profile = scratch.project_profile(&line);
        let sources = Sources {
            profile_project: Some(profile.clone()),
            ..Sources::empty()
        };
        match target.project_scope {
            ProjectScope::Denied => {
                let diagnostic = expect_err(&sources);
                assert_denied(&diagnostic, entry.canonical, &profile);
                assert!(diagnostic.why.contains(entry.old), "{}", diagnostic.why);
            }
            ProjectScope::Allowed => {
                let resolved = load(&sources).unwrap_or_else(|diagnostic| {
                    panic!("{} from a project profile: {diagnostic}", entry.old)
                });
                assert_eq!(
                    resolved.origin(entry.canonical),
                    Some(&Origin::ProfileProject(profile.clone())),
                    "{}",
                    entry.old
                );
            }
        }
    }
}
