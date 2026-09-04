//! Die Präzedenz der sechs Ebenen, und was dabei schiefgehen kann.
//!
//! Reihenfolge (`backlog/CONVENTIONS.md` 4.4): Vorgabe < globale `config.toml` <
//! globales Profil < Projekt-Profil < Umgebung < Kommandozeile. Jeder Schritt
//! dieser Leiter hat hier einen Test, und zwar einen, der die Ebene darunter
//! wirklich besetzt: Ein Test, der nur die oberste Ebene setzt, würde auch dann
//! grün bleiben, wenn die Reihenfolge falsch herum wäre.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use humanitl_config::model::{AskMode, Language, Theme};
use humanitl_config::{Config, Env, Origin, Resolved, Sources, load};
use humanitl_core::{FixAction, Severity};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn all_files() -> Sources {
    Sources {
        global_toml: Some(fixture("global.toml")),
        profile_global: Some(fixture("profile-global.toml")),
        profile_project: Some(fixture("profile-project.toml")),
        ..Sources::empty()
    }
}

fn expect_ok(sources: &Sources) -> Resolved {
    match load(sources) {
        Ok(resolved) => resolved,
        Err(diagnostic) => panic!("load failed: {diagnostic}"),
    }
}

fn expect_err(sources: &Sources) -> humanitl_core::Diagnostic {
    match load(sources) {
        Ok(_) => panic!("load was expected to fail"),
        Err(diagnostic) => diagnostic,
    }
}

#[test]
fn defaults_when_nothing_set() {
    let resolved = expect_ok(&Sources::empty());

    assert_eq!(resolved.config, humanitl_config::Config::default());
    assert_eq!(resolved.config.hold.timeout_secs, 300);
    assert_eq!(resolved.config.limits.hold_body_cap_bytes, 32 * 1024 * 1024);
    assert_eq!(resolved.config.limits.preview_cap_bytes, 8 * 1024 * 1024);
    assert_eq!(resolved.config.limits.event_buffer, 1024);
    assert_eq!(resolved.config.limits.hold_max_bytes, 256 * 1024 * 1024);
    assert_eq!(resolved.config.limits.hold_max_flows, 200);
    assert_eq!(resolved.config.limits.max_decompress_ratio, 100);
    assert_eq!(resolved.config.limits.connect_timeout_secs, 10);
    assert_eq!(resolved.config.recorder.inline_max_bytes, 256 * 1024);
    assert_eq!(
        resolved.config.pseudonyms.max_response_bytes,
        8 * 1024 * 1024
    );
    assert_eq!(resolved.config.hold.ask_mode, AskMode::Ui);
    assert_eq!(resolved.config.ui.language, Language::En);

    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    assert!(!resolved.origins.is_empty());
    for (path, origin) in &resolved.origins {
        assert_eq!(*origin, Origin::Default, "{path} is not a default");
    }
    assert!(resolved.changed().is_empty());
}

#[test]
fn every_leaf_of_the_schema_has_an_origin() {
    let resolved = expect_ok(&Sources::empty());
    for path in humanitl_config::schema::leaf_paths() {
        assert!(
            resolved.origin(path).is_some(),
            "{path} has no origin, so the settings screen cannot say where it comes from"
        );
    }
}

#[test]
fn global_overrides_default() {
    let sources = Sources {
        global_toml: Some(fixture("global.toml")),
        ..Sources::empty()
    };
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.hold.timeout_secs, 60);
    assert_eq!(resolved.config.ui.theme, Theme::Light);
    assert_eq!(resolved.config.limits.hold_max_flows, 50);
    assert_eq!(resolved.origin("hold.timeout_secs"), Some(&Origin::Global));
    assert_eq!(resolved.origin("ui.language"), Some(&Origin::Default));
}

#[test]
fn profile_global_overrides_global() {
    let sources = Sources {
        global_toml: Some(fixture("global.toml")),
        profile_global: Some(fixture("profile-global.toml")),
        ..Sources::empty()
    };
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.hold.timeout_secs, 120);
    assert_eq!(
        resolved.origin("hold.timeout_secs"),
        Some(&Origin::ProfileGlobal("profile-global".to_owned()))
    );
    // Was das Profil nicht anfasst, bleibt bei der globalen Datei.
    assert_eq!(resolved.config.ui.theme, Theme::Light);
    assert_eq!(resolved.origin("ui.theme"), Some(&Origin::Global));
}

#[test]
fn profile_project_overrides_profile_global() {
    let resolved = expect_ok(&all_files());

    assert_eq!(resolved.config.hold.timeout_secs, 180);
    assert_eq!(
        resolved.origin("hold.timeout_secs"),
        Some(&Origin::ProfileProject(fixture("profile-project.toml")))
    );
    assert_eq!(
        resolved.config.findings.user_terms,
        vec!["projekt-nordlicht".to_owned()]
    );
    // Aus dem globalen Profil, das darunter liegt.
    assert_eq!(resolved.config.ui.language, Language::De);
    assert_eq!(
        resolved.origin("ui.language"),
        Some(&Origin::ProfileGlobal("profile-global".to_owned()))
    );
}

#[test]
fn only_the_config_block_of_a_profile_counts() {
    let resolved = expect_ok(&all_files());
    // profile-global.toml hat einen [rules]-Block; HUM-066 liest ihn, hier ist
    // er kein unbekannter Schlüssel und landet in keinem Feld.
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
}

#[test]
fn env_overrides_files() {
    let sources = all_files().with_env(Env::from_pairs([("HUMANITL_HOLD__TIMEOUT_SECS", "42")]));
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.hold.timeout_secs, 42);
    assert_eq!(
        resolved.origin("hold.timeout_secs"),
        Some(&Origin::Env("HUMANITL_HOLD__TIMEOUT_SECS".to_owned()))
    );
}

#[test]
fn cli_overrides_env() {
    let sources = all_files()
        .with_env(Env::from_pairs([("HUMANITL_HOLD__TIMEOUT_SECS", "42")]))
        .with_cli([("hold.timeout_secs", "7")]);
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.hold.timeout_secs, 7);
    assert_eq!(resolved.origin("hold.timeout_secs"), Some(&Origin::Cli));
}

#[test]
fn the_whole_ladder_in_one_go() {
    let sources = all_files()
        .with_env(Env::from_pairs([
            ("HUMANITL_UI__SOUND", "true"),
            ("HUMANITL_HOLD__TIMEOUT_SECS", "42"),
        ]))
        .with_cli([("limits.event_buffer", "16")]);
    let resolved = expect_ok(&sources);

    let ladder = [
        ("recorder.retention_days", Origin::Default),
        ("ui.theme", Origin::Global),
        (
            "ui.language",
            Origin::ProfileGlobal("profile-global".to_owned()),
        ),
        (
            "findings.user_terms",
            Origin::ProfileProject(fixture("profile-project.toml")),
        ),
        ("ui.sound", Origin::Env("HUMANITL_UI__SOUND".to_owned())),
        ("limits.event_buffer", Origin::Cli),
    ];
    for (path, origin) in ladder {
        assert_eq!(resolved.origin(path), Some(&origin), "{path}");
    }
    assert_eq!(resolved.changed().len(), 7);
}

/// Eine Zeile der Tabelle in `a_higher_layer_overrides_every_group`: ein Blatt
/// je Gruppe, ein Wert in der globalen `config.toml` (Ebene 2) und ein anderer
/// in der Umgebung (Ebene 5).
struct GroupOverride {
    group: &'static str,
    path: &'static str,
    /// Der Wert der unteren Ebene, als TOML-Literal.
    lower: &'static str,
    /// Der Wert der oberen Ebene, als Text der Umgebungsvariablen.
    higher: &'static str,
    /// Liest das Feld aus der typisierten Konfiguration, damit der Vergleich
    /// hinter der Deserialisierung stattfindet und nicht auf dem TOML-Wert.
    read: fn(&Config) -> String,
    /// Was `read` nach der unteren beziehungsweise der oberen Ebene liefert.
    /// Keiner der beiden ist der Vorgabewert, sonst bewiese ein Treffer nichts.
    after_lower: &'static str,
    after_higher: &'static str,
}

/// Jede Gruppe des Schemas mit Namen. Der Test
/// `the_override_table_names_every_group_of_the_schema` haelt die Liste
/// vollstaendig, wenn eine Gruppe dazukommt.
const GROUP_OVERRIDES: &[GroupOverride] = &[
    GroupOverride {
        group: "llm",
        path: "llm.endpoint",
        lower: "\"http://a.lan:8080/v1\"",
        higher: "http://b.lan:9090/v1",
        read: |config| {
            config
                .llm
                .endpoint
                .as_ref()
                .map_or_else(String::new, ToString::to_string)
        },
        after_lower: "http://a.lan:8080/v1",
        after_higher: "http://b.lan:9090/v1",
    },
    GroupOverride {
        group: "hold",
        path: "hold.timeout_secs",
        lower: "60",
        higher: "42",
        read: |config| config.hold.timeout_secs.to_string(),
        after_lower: "60",
        after_higher: "42",
    },
    GroupOverride {
        group: "limits",
        path: "limits.hold_max_flows",
        lower: "50",
        higher: "75",
        read: |config| config.limits.hold_max_flows.to_string(),
        after_lower: "50",
        after_higher: "75",
    },
    GroupOverride {
        group: "resolver",
        path: "resolver.cache_ttl_secs",
        lower: "10",
        higher: "20",
        read: |config| config.resolver.cache_ttl_secs.to_string(),
        after_lower: "10",
        after_higher: "20",
    },
    GroupOverride {
        group: "findings",
        path: "findings.email_allow_domains",
        lower: "[\"a.example\"]",
        higher: "[\"b.example\"]",
        read: |config| config.findings.email_allow_domains.join(","),
        after_lower: "a.example",
        after_higher: "b.example",
    },
    GroupOverride {
        group: "pseudonyms",
        path: "pseudonyms.max_response_bytes",
        lower: "1024",
        higher: "4096",
        read: |config| config.pseudonyms.max_response_bytes.to_string(),
        after_lower: "1024",
        after_higher: "4096",
    },
    GroupOverride {
        group: "sandbox",
        path: "sandbox.profile",
        lower: "\"strict\"",
        higher: "loose",
        read: |config| config.sandbox.profile.clone(),
        after_lower: "strict",
        after_higher: "loose",
    },
    GroupOverride {
        group: "agent",
        path: "agent.adapter",
        lower: "\"claude\"",
        higher: "codex",
        read: |config| config.agent.adapter.clone(),
        after_lower: "claude",
        after_higher: "codex",
    },
    GroupOverride {
        group: "recorder",
        path: "recorder.retention_days",
        lower: "7",
        higher: "14",
        read: |config| config.recorder.retention_days.to_string(),
        after_lower: "7",
        after_higher: "14",
    },
    GroupOverride {
        group: "ui",
        path: "ui.theme",
        lower: "\"light\"",
        higher: "system",
        read: |config| format!("{:?}", config.ui.theme),
        after_lower: "Light",
        after_higher: "System",
    },
    GroupOverride {
        // Eine freie Tabelle: die obere Ebene ersetzt sie ganz.
        group: "experimental",
        path: "experimental.upstream_port_map",
        lower: "{ \"443\" = 8443 }",
        higher: "{ \"80\" = 8080 }",
        read: |config| format!("{:?}", config.experimental.upstream_port_map),
        after_lower: "{\"443\": 8443}",
        after_higher: "{\"80\": 8080}",
    },
];

/// `hold.timeout_secs` wird zu `HUMANITL_HOLD__TIMEOUT_SECS`.
fn env_name(path: &str) -> String {
    format!("HUMANITL_{}", path.to_uppercase().replace('.', "__"))
}

#[test]
fn a_higher_layer_overrides_every_group() {
    // Die Leiter oben laeuft nur ueber `hold`. Hier setzt fuer jede Gruppe die
    // globale Datei einen Wert und die Umgebung einen anderen; zuerst allein
    // die Datei, damit die untere Ebene wirklich besetzt ist.
    for row in GROUP_OVERRIDES {
        let dir = tempfile::tempdir().expect("tempdir");
        let global = dir.path().join("config.toml");
        std::fs::write(&global, format!("{} = {}\n", row.path, row.lower)).expect("write");
        let lower_only = Sources {
            global_toml: Some(global),
            ..Sources::empty()
        };

        let resolved = expect_ok(&lower_only);
        assert_eq!(
            (row.read)(&resolved.config),
            row.after_lower,
            "{}: the global file must set {}",
            row.group,
            row.path
        );
        assert_eq!(
            resolved.origin(row.path),
            Some(&Origin::Global),
            "{}: origin after the global file",
            row.group
        );

        let var = env_name(row.path);
        let both = lower_only.with_env(Env::from_pairs([(var.as_str(), row.higher)]));
        let resolved = expect_ok(&both);
        assert_eq!(
            (row.read)(&resolved.config),
            row.after_higher,
            "{}: {var} must override the global file",
            row.group
        );
        assert_eq!(
            resolved.origin(row.path),
            Some(&Origin::Env(var)),
            "{}: origin after the environment",
            row.group
        );
        assert!(
            resolved.diagnostics.is_empty(),
            "{}: {:?}",
            row.group,
            resolved.diagnostics
        );
    }
}

#[test]
fn the_override_table_names_every_group_of_the_schema() {
    let in_table: BTreeSet<&str> = GROUP_OVERRIDES.iter().map(|row| row.group).collect();
    let in_schema: BTreeSet<&str> = humanitl_config::schema::by_group().into_keys().collect();
    assert_eq!(
        in_table, in_schema,
        "a group of the schema without a row in GROUP_OVERRIDES has no precedence test"
    );
    assert_eq!(in_table.len(), GROUP_OVERRIDES.len(), "one row per group");

    let leaves = humanitl_config::schema::leaf_paths();
    for row in GROUP_OVERRIDES {
        assert!(
            row.path.starts_with(&format!("{}.", row.group)),
            "{} does not belong to {}",
            row.path,
            row.group
        );
        assert!(
            leaves.contains(row.path),
            "{} is not a leaf of the schema",
            row.path
        );
        assert_ne!(
            row.after_lower, row.after_higher,
            "{}: both layers look alike",
            row.path
        );
    }
}

#[test]
fn env_value_types() {
    let sources = Sources::empty().with_env(Env::from_pairs([
        ("HUMANITL_UI__NOTIFICATIONS", "false"),
        ("HUMANITL_HOLD__TIMEOUT_SECS", "42"),
        ("HUMANITL_SANDBOX__PROFILE", "abc"),
        ("HUMANITL_LLM__ENDPOINT", "http://box.lan:8080/v1"),
        ("HUMANITL_LLM__PASSTHROUGH_PATHS", "[\"/v1/\", \"/chat/\"]"),
        // Text bleibt Text, auch wenn er wie eine Zahl oder ein Wahrheitswert aussieht.
        ("HUMANITL_AGENT__ADAPTER", "2024"),
        ("HUMANITL_RESOLVER__NAMESERVER", "1.1.1.1:53"),
        ("HUMANITL_FINDINGS__USER_TERMS", "[\"true\", \"42\"]"),
    ]));
    let resolved = expect_ok(&sources);

    assert!(!resolved.config.ui.notifications);
    assert_eq!(resolved.config.hold.timeout_secs, 42);
    assert_eq!(resolved.config.sandbox.profile, "abc");
    assert_eq!(
        resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
        Some("http://box.lan:8080/v1")
    );
    assert_eq!(
        resolved.config.llm.passthrough_paths,
        vec!["/v1/".to_owned(), "/chat/".to_owned()]
    );
    assert_eq!(resolved.config.agent.adapter, "2024");
    assert_eq!(
        resolved.config.resolver.nameserver.as_deref(),
        Some("1.1.1.1:53")
    );
    assert_eq!(
        resolved.config.findings.user_terms,
        vec!["true".to_owned(), "42".to_owned()]
    );
}

#[test]
fn a_text_field_keeps_a_numeric_looking_value_from_the_command_line() {
    let sources = Sources::empty().with_cli([("sandbox.profile", "2024"), ("ui.theme", "light")]);
    let resolved = expect_ok(&sources);
    assert_eq!(resolved.config.sandbox.profile, "2024");
    assert_eq!(resolved.config.ui.theme, Theme::Light);
}

#[test]
fn env_variables_of_other_programs_are_ignored() {
    let sources = Sources::empty().with_env(Env::from_pairs([
        ("PATH", "/usr/bin"),
        ("HUMANITLD", "1"),
        // Variablen anderer Humanitl-Werkzeuge ohne `__`: Galerie (HUM-008),
        // Escape-Harness (HUM-006). Kein Blatt liegt auf der obersten Ebene,
        // also sind sie nie ein Schluessel und nie ein Befund.
        ("HUMANITL_GALLERY", "1"),
        ("HUMANITL_ESCAPE_MARKER", "host-4711"),
        ("HUMANITL_", "x"),
        ("HUMANITL_HOLD__TIMEOUT_SECS", "42"),
    ]));
    let resolved = expect_ok(&sources);
    assert_eq!(resolved.config.hold.timeout_secs, 42);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
}

#[test]
fn an_unknown_env_key_is_a_diagnostic_not_a_failure() {
    // Die Umgebung gehoert nicht uns allein; ein Daemon, der wegen einer
    // fremden Variablen nicht startet, waere schlimmer als einer, der meldet.
    let sources =
        Sources::empty().with_env(Env::from_pairs([("HUMANITL_HOLD__TIMEOUTT_SECS", "5")]));
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.hold.timeout_secs, 300);
    let [diagnostic] = resolved.diagnostics.as_slice() else {
        panic!(
            "expected exactly one diagnostic, got {:?}",
            resolved.diagnostics
        );
    };
    assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
    // Nie `Error`: ein Fehler in `diagnostics` saehe aus wie ein gescheiterter
    // Start, obwohl der Daemon laeuft (`backlog/CONVENTIONS.md` 4.11).
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert!(
        diagnostic.why.contains("hold.timeoutt_secs"),
        "{}",
        diagnostic.why
    );
    assert!(
        diagnostic.why.contains("hold.timeout_secs"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn unknown_key_is_config_002() {
    let sources = Sources {
        global_toml: Some(fixture("unknown-key.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);

    assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(
        diagnostic.why.contains("hold.timeoutt_secs"),
        "why must name the path: {}",
        diagnostic.why
    );
    assert!(diagnostic.why.contains("config.toml"), "{}", diagnostic.why);
    assert!(diagnostic.fix.is_some(), "a typo deserves a suggestion");
}

#[test]
fn an_unknown_cli_key_is_config_002() {
    let sources = Sources::empty().with_cli([("hold.nonsense", "1")]);
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
    assert!(
        diagnostic.why.contains("command line"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn a_group_used_as_a_value_is_config_002() {
    let sources = Sources::empty().with_cli([("hold", "5")]);
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
    assert!(
        diagnostic.why.contains("group of settings"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn a_group_outside_the_config_block_of_a_profile_is_config_002() {
    // [hold] auf der obersten Ebene eines Profils: ohne Meldung bliebe das
    // Profil wirkungslos, und der Nutzer suchte den Fehler anderswo.
    let sources = Sources {
        profile_global: Some(fixture("profile-misplaced-group.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
    assert!(
        diagnostic.why.contains("[config.hold]"),
        "{}",
        diagnostic.why
    );
    assert!(
        diagnostic.why.contains("profile-misplaced-group.toml"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn an_unknown_block_of_a_profile_is_config_002() {
    let sources = Sources {
        profile_project: Some(fixture("profile-unknown-block.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
    assert!(diagnostic.why.contains("[confg]"), "{}", diagnostic.why);
    assert!(diagnostic.why.contains("[config]"), "{}", diagnostic.why);
}

#[test]
fn zero_timeout_is_config_003() {
    let sources = Sources {
        global_toml: Some(fixture("zero-timeout.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);

    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(
        diagnostic.why.contains("hold.timeout_secs"),
        "{}",
        diagnostic.why
    );
    assert!(diagnostic.why.contains("default 300"), "{}", diagnostic.why);
}

#[test]
fn a_wrong_type_is_config_003_with_path_and_origin() {
    let sources = Sources::empty().with_cli([("hold.timeout_secs", "soon")]);
    let diagnostic = expect_err(&sources);

    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(
        diagnostic.why.contains("hold.timeout_secs"),
        "{}",
        diagnostic.why
    );
    assert!(diagnostic.why.contains("integer"), "{}", diagnostic.why);
    assert!(
        diagnostic.why.contains("command line"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn a_value_outside_an_enum_is_config_003() {
    let sources = Sources {
        global_toml: Some(fixture("bad-enum.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);

    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(
        diagnostic.why.contains("ui | terminal | none"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn a_missing_file_is_config_001() {
    let sources = Sources {
        global_toml: Some(fixture("does-not-exist.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_001");
    assert!(
        diagnostic.why.contains("does-not-exist.toml"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn broken_toml_is_config_001() {
    let sources = Sources {
        global_toml: Some(fixture("broken.toml")),
        ..Sources::empty()
    };
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_001");
    assert!(diagnostic.why.contains("broken.toml"), "{}", diagnostic.why);
}

#[test]
fn lists_replace_and_do_not_grow() {
    let sources = Sources {
        global_toml: Some(fixture("lists.toml")),
        ..Sources::empty()
    };
    let resolved = expect_ok(&sources);

    assert_eq!(
        resolved.config.llm.passthrough_paths,
        vec!["/only/".to_owned()]
    );
    assert_eq!(
        resolved.config.findings.user_terms,
        vec!["projekt-nordlicht".to_owned()]
    );
    assert_eq!(
        resolved
            .config
            .resolver
            .overrides
            .get("api.example.com")
            .map(String::as_str),
        Some("10.0.0.5")
    );
    assert_eq!(resolved.origin("resolver.overrides"), Some(&Origin::Global));
}

#[test]
fn a_free_table_is_replaced_as_a_whole() {
    let sources = Sources {
        global_toml: Some(fixture("lists.toml")),
        ..Sources::empty()
    }
    .with_cli([("resolver.overrides", "{ \"other.example\" = \"10.0.0.9\" }")]);
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.resolver.overrides.len(), 1);
    assert!(
        resolved
            .config
            .resolver
            .overrides
            .contains_key("other.example")
    );
    assert_eq!(resolved.origin("resolver.overrides"), Some(&Origin::Cli));
}

#[test]
fn the_old_key_names_still_work() {
    let sources = Sources {
        global_toml: Some(fixture("legacy-aliases.toml")),
        ..Sources::empty()
    };
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.limits.hold_body_cap_bytes, 4096);
    assert_eq!(resolved.config.limits.event_buffer, 64);
    assert_eq!(resolved.config.limits.preview_cap_bytes, 2048);
    assert_eq!(resolved.config.limits.max_decompress_ratio, 7);
    assert_eq!(resolved.config.limits.recorder_max_body_bytes, 1_048_576);
    assert_eq!(resolved.config.limits.connect_timeout_secs, 5);

    // Die Herkunft steht unter dem heutigen Namen, nicht unter dem alten.
    assert_eq!(
        resolved.origin("limits.hold_body_cap_bytes"),
        Some(&Origin::Global)
    );
    assert_eq!(resolved.origin("hold.body_cap_bytes"), None);

    assert_eq!(resolved.diagnostics.len(), 6, "{:?}", resolved.diagnostics);
    for diagnostic in &resolved.diagnostics {
        assert_eq!(diagnostic.code.as_str(), "CONFIG_005");
        assert_eq!(diagnostic.severity, Severity::Info);
        assert_eq!(diagnostic.title, "Veralteter Schlüssel");
    }
    // Der Vorschlag schreibt denselben Wert unter den heutigen Namen.
    let body_cap = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.why.starts_with("hold.body_cap_bytes"))
        .expect("a diagnostic for hold.body_cap_bytes");
    assert_eq!(
        body_cap.fix,
        Some(FixAction::ChangeSetting {
            key: "limits.hold_body_cap_bytes".to_owned(),
            value: "4096".to_owned(),
        })
    );
}

#[test]
fn the_current_name_beats_the_alias_in_the_same_file() {
    let sources = Sources {
        global_toml: Some(fixture("alias-conflict.toml")),
        ..Sources::empty()
    };
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.limits.hold_body_cap_bytes, 65536);
    let [diagnostic] = resolved.diagnostics.as_slice() else {
        panic!("expected one diagnostic, got {:?}", resolved.diagnostics);
    };
    assert_eq!(diagnostic.code.as_str(), "CONFIG_006");
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.title, "Alter und neuer Schlüssel gesetzt");
    assert!(
        diagnostic.why.contains("hold.body_cap_bytes"),
        "{}",
        diagnostic.why
    );
    assert!(
        diagnostic.why.contains("limits.hold_body_cap_bytes wins"),
        "the message must name the winner: {}",
        diagnostic.why
    );
}

#[test]
fn an_alias_in_a_higher_layer_still_wins_over_a_lower_layer() {
    // Der heutige Name gewinnt nur innerhalb einer Ebene. Über Ebenen hinweg
    // gilt die Präzedenz, sonst könnte eine Datei die Kommandozeile schlagen.
    let sources = Sources {
        global_toml: Some(fixture("alias-conflict.toml")),
        ..Sources::empty()
    }
    .with_env(Env::from_pairs([("HUMANITL_HOLD__BODY_CAP_BYTES", "2048")]));
    let resolved = expect_ok(&sources);

    assert_eq!(resolved.config.limits.hold_body_cap_bytes, 2048);
    assert_eq!(
        resolved.origin("limits.hold_body_cap_bytes"),
        Some(&Origin::Env("HUMANITL_HOLD__BODY_CAP_BYTES".to_owned()))
    );
    let warning = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Warning)
        .expect("a conflict between old and new name must never be silent");
    // Und die Meldung sagt, wer wirklich gewonnen hat: hier der alte Name.
    assert!(
        warning.why.contains("hold.body_cap_bytes wins")
            && !warning.why.contains("limits.hold_body_cap_bytes wins"),
        "{}",
        warning.why
    );
    assert!(
        warning.why.contains("HUMANITL_HOLD__BODY_CAP_BYTES"),
        "{}",
        warning.why
    );
}

#[test]
fn discover_finds_the_files_of_a_home_and_a_project() {
    let home = tempfile::tempdir().expect("tempdir");
    let config_dir = home.path().join("cfg/humanitl");
    std::fs::create_dir_all(config_dir.join("profiles")).expect("mkdir");
    std::fs::write(
        config_dir.join("config.toml"),
        "[hold]\ntimeout_secs = 11\n",
    )
    .expect("write");
    std::fs::write(
        config_dir.join("profiles/work.toml"),
        "[config.hold]\ntimeout_secs = 12\n",
    )
    .expect("write");

    let project = home.path().join("project");
    std::fs::create_dir_all(project.join(".humanitl")).expect("mkdir");
    std::fs::write(
        project.join(".humanitl/profile.toml"),
        "[config.ui]\ntheme = \"system\"\n",
    )
    .expect("write");

    let env = Env::from_pairs([
        ("HOME", home.path().display().to_string()),
        (
            "XDG_CONFIG_HOME",
            home.path().join("cfg").display().to_string(),
        ),
    ]);
    let sources = humanitl_config::discover_with(&env, &project, Some("work"));

    assert_eq!(sources.global_toml, Some(config_dir.join("config.toml")));
    assert_eq!(
        sources.profile_global,
        Some(config_dir.join("profiles/work.toml"))
    );
    assert_eq!(
        sources.profile_project,
        Some(project.join(".humanitl/profile.toml"))
    );

    let resolved = expect_ok(&sources.with_env(env));
    assert_eq!(resolved.config.hold.timeout_secs, 12);
    assert_eq!(resolved.config.ui.theme, Theme::System);
}

#[test]
fn discover_is_content_with_nothing() {
    let empty = tempfile::tempdir().expect("tempdir");
    let env = Env::from_pairs([("HOME", empty.path().display().to_string())]);
    let sources = humanitl_config::discover_with(&env, empty.path(), None);

    assert_eq!(sources.global_toml, None);
    assert_eq!(sources.profile_global, None);
    assert_eq!(sources.profile_project, None);
    assert_eq!(
        expect_ok(&sources).config,
        humanitl_config::Config::default()
    );
}

#[test]
fn sandbox_env_is_a_free_table_that_reaches_the_launcher() {
    // HUM-045: `FixAction::SetEnv` schreibt hierher. Ohne den Schlüssel wäre
    // der Knopf in der Oberfläche ein Vorschlag ins Leere, und
    // `humanitl config get sandbox.env` endete als CONFIG_002.
    let sources = Sources::empty().with_cli([(
        "sandbox.env",
        "{ \"CURL_CA_BUNDLE\" = \"/etc/humanitl/ca.crt\" }",
    )]);
    let resolved = expect_ok(&sources);
    assert_eq!(
        resolved
            .config
            .sandbox
            .env
            .get("CURL_CA_BUNDLE")
            .map(String::as_str),
        Some("/etc/humanitl/ca.crt")
    );
    assert!(
        humanitl_config::schema::known_paths().contains("sandbox.env"),
        "the key has to be in the schema, or `config get` refuses it"
    );
}

#[test]
fn a_broken_variable_name_in_sandbox_env_is_refused() {
    // `--setenv KEY VALUE`: Ein `=` im Namen machte aus einem Paar zwei.
    let sources =
        Sources::empty().with_cli([("sandbox.env", "{ \"A=B\" = \"/etc/humanitl/ca.crt\" }")]);
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(diagnostic.why.contains("sandbox.env"), "{}", diagnostic.why);
}

#[test]
fn a_loader_variable_in_sandbox_env_is_refused() {
    // Gemessen vom Review: Eine Bibliothek mit Konstruktor, per `LD_PRELOAD`
    // vorgeladen, läuft in Shim und Agent vor `main` und damit vor dem
    // seccomp-Filter; ein dort abgezweigter Prozess erbt ihn nie. Der billige
    // Weg dorthin führt über die Umgebung des Prozesses, nicht über eine
    // Datei — deshalb wird der Schlüssel abgelehnt, egal auf welcher Ebene.
    for name in humanitl_config::LOADER_ENV_KEYS {
        let sources = Sources::empty().with_cli([(
            "sandbox.env",
            &format!("{{ \"{name}\" = \"/work/evil.so\" }}"),
        )]);
        let diagnostic = expect_err(&sources);
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003", "{name}");
        assert!(diagnostic.why.contains(name), "{}", diagnostic.why);
        assert!(
            diagnostic.why.contains("seccomp"),
            "the reason has to name what breaks: {}",
            diagnostic.why
        );
    }
}

#[test]
fn a_loader_variable_from_the_host_environment_is_refused_too() {
    // Der Weg, den der Review gemessen hat: `HUMANITL_SANDBOX__ENV` aus
    // derselben Shell, in der ein `direnv` das `.envrc` eines geklonten
    // Projekts ausführt.
    let sources = Sources::empty().with_env(Env::from_pairs([(
        "HUMANITL_SANDBOX__ENV",
        "{ LD_PRELOAD = \"/work/evil.so\" }",
    )]));
    let diagnostic = expect_err(&sources);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(diagnostic.why.contains("LD_PRELOAD"), "{}", diagnostic.why);
}

#[test]
fn a_harmless_variable_in_sandbox_env_still_passes() {
    // Gesperrt sind genau die drei, die den Linker fremden Code laden lassen,
    // nicht das ganze `LD_`-Präfix.
    let sources = Sources::empty().with_cli([("sandbox.env", "{ \"LD_DEBUG\" = \"libs\" }")]);
    let resolved = expect_ok(&sources);
    assert_eq!(
        resolved
            .config
            .sandbox
            .env
            .get("LD_DEBUG")
            .map(String::as_str),
        Some("libs")
    );
}
