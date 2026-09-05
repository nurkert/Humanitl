//! Das Schema ist die Quelle für Oberfläche und Doku. Diese Tests halten es
//! vollständig und stabil.
//!
//! Vollständig heißt: jedes Blattfeld trägt Stufe, Beschreibung und
//! Vorgabewert. Stabil heißt: eine Änderung am Schema fällt im Abzug auf und
//! wird bewusst übernommen, statt sich unbemerkt in eine Oberfläche zu
//! schleichen, die anderswo entsteht.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use humanitl_config::scope::{PROJECT_SCOPE_KEY, ProjectScope};
use humanitl_config::tier::{TIER_KEY, Tier};
use humanitl_config::{Config, alias, schema};
use serde_json::Value;

fn snapshot_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config.schema.json")
}

fn walk(node: &Value, prefix: &str, visit: &mut dyn FnMut(&str, &Value, bool)) {
    let Some(properties) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, child) in properties {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        let group = child
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|map| !map.is_empty());
        visit(&path, child, group);
        if group {
            walk(child, &path, visit);
        }
    }
}

#[test]
fn every_leaf_has_tier_and_description() {
    let schema = Config::json_schema();
    let mut leaves = 0_usize;

    walk(&schema, "", &mut |path, node, group| {
        let description = node
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(
            !description.trim().is_empty(),
            "{path} has no description; the settings screen would show an empty line"
        );

        let tier = node.get(TIER_KEY).and_then(Value::as_str);
        let Some(tier) = tier else {
            panic!("{path} has no {TIER_KEY}");
        };
        assert!(
            Tier::parse(tier).is_some(),
            "{path} has the unknown tier {tier:?}"
        );

        if !group {
            leaves += 1;
            assert!(
                node.get("default").is_some(),
                "{path} has no default; every field must be answerable without asking the user"
            );
        }
    });

    assert!(
        leaves >= 40,
        "only {leaves} leaves, that cannot be complete"
    );
    assert_eq!(leaves, schema::leaves().len());
}

#[test]
fn every_node_has_a_project_scope() {
    // Ein Feld ohne `x-project-scope` gälte beim Durchlauf als gesperrt. Das
    // ist die sichere Seite, aber eine stille; hier fällt es auf.
    let schema = Config::json_schema();
    let mut denied_groups: Vec<String> = Vec::new();
    let mut leaves = 0_usize;

    walk(&schema, "", &mut |path, node, group| {
        let Some(scope) = node.get(PROJECT_SCOPE_KEY).and_then(Value::as_str) else {
            panic!("{path} has no {PROJECT_SCOPE_KEY}; decide if a project profile may set it");
        };
        let Some(scope) = ProjectScope::parse(scope) else {
            panic!("{path} has the unknown project scope {scope:?}");
        };
        if group {
            if scope == ProjectScope::Denied {
                denied_groups.push(path.to_owned());
            }
        } else {
            leaves += 1;
        }
    });
    assert_eq!(leaves, schema::leaves().len());

    // Eine gesperrte Gruppe hat nur gesperrte Blätter, und eine Gruppe, deren
    // Blätter alle gesperrt sind, ist selbst gesperrt: die Gruppe sagt dann
    // dasselbe wie ihre Felder, nie etwas anderes.
    for field in schema::leaves() {
        let in_denied_group = denied_groups
            .iter()
            .any(|group| field.path.starts_with(&format!("{group}.")));
        if in_denied_group {
            assert_eq!(
                field.project_scope,
                ProjectScope::Denied,
                "{} sits in a denied group but is allowed",
                field.path
            );
        }
    }
    for (group, fields) in schema::by_group() {
        let all_denied = fields
            .iter()
            .all(|field| field.project_scope == ProjectScope::Denied);
        assert_eq!(
            all_denied,
            denied_groups.iter().any(|denied| denied == group),
            "the group {group} and its leaves disagree about the project scope"
        );
    }
    assert!(!denied_groups.is_empty(), "no group is denied as a whole");
}

#[test]
fn every_key_of_the_conventions_is_reachable() {
    // CONVENTIONS.md 3.7 und 4.4. Ein Schlüssel gilt als erreichbar, wenn ihn
    // das Schema als Blatt kennt oder ein Alias auf ein Blatt zeigt.
    let keys = [
        "llm.endpoint",
        "llm.passthrough_paths",
        "hold.timeout_secs",
        "hold.body_cap_bytes",
        "hold.ask_mode",
        "hold.hard_block_checksum_secrets",
        "limits.hold_body_cap_bytes",
        "limits.preview_cap_bytes",
        "limits.event_buffer",
        "limits.max_decompress_ratio",
        "limits.hold_max_flows",
        "limits.hold_max_bytes",
        "limits.connect_timeout_secs",
        "limits.header_timeout_secs",
        "limits.body_timeout_secs",
        "limits.recorder_max_body_bytes",
        "upstream.connect_timeout_secs",
        "preview.cap_bytes",
        "preview.max_decompress_ratio",
        "ipc.event_buffer",
        "recorder.inline_max_bytes",
        "recorder.retention_days",
        "recorder.max_body_bytes",
        "resolver.nameserver",
        "resolver.overrides",
        "resolver.cache_ttl_secs",
        "resolver.prefer",
        "resolver.test_ca",
        "findings.enabled",
        "findings.user_terms",
        "findings.email_allow_domains",
        "findings.ignored_hashes",
        "pseudonyms.translate_responses",
        "pseudonyms.max_response_bytes",
        "sandbox.profile",
        "sandbox.work_dir",
        "sandbox.work_mode",
        "agent.adapter",
        "agent.command",
        "agent.briefing.enabled",
        "ui.language",
        "ui.theme",
        "ui.notifications",
        "ui.sound",
        "experimental.h2_upstream",
        "experimental.ws_hold",
        "experimental.upstream_port_map",
    ];

    let leaves = schema::leaf_paths();
    for key in keys {
        let canonical = alias::canonical(key).unwrap_or(key);
        assert!(
            leaves.contains(canonical),
            "{key} of CONVENTIONS.md is neither a field nor an alias"
        );
    }
}

#[test]
fn every_alias_points_at_a_field_and_no_alias_group_is_a_field() {
    let leaves = schema::leaf_paths();
    for entry in alias::ALIASES {
        assert!(
            leaves.contains(entry.canonical),
            "{} points at {}, which the schema does not have",
            entry.old,
            entry.canonical
        );
        assert!(
            !leaves.contains(entry.old),
            "{} is an alias and a field at the same time",
            entry.old
        );
    }
    // Diese Gruppen sind mit HUM-057 vollstaendig in [limits] aufgegangen; sie
    // duerfen nicht als leere Gruppe im Schema stehen bleiben.
    for group in ["ipc", "preview", "upstream"] {
        assert!(
            alias::legacy_groups().contains(&group),
            "{group} has no alias any more"
        );
        assert!(
            !schema::known_paths().contains(group),
            "the group {group} lives on as an alias only, it must not be in the schema"
        );
    }
}

#[test]
fn the_defaults_of_the_schema_are_the_defaults_of_the_type() {
    let serialized = serde_json::to_value(Config::default()).expect("Config serializes");
    for field in schema::leaves() {
        let pointer = format!("/{}", field.path.replace('.', "/"));
        let Some(actual) = serialized.pointer(&pointer) else {
            // Optionale Felder ohne Wert fehlen in der Ausgabe; im Schema
            // stehen sie als null.
            assert_eq!(
                field.default,
                Some(Value::Null),
                "{} is missing from the serialized default",
                field.path
            );
            continue;
        };
        assert_eq!(
            field.default.as_ref(),
            Some(actual),
            "{} disagrees with impl Default",
            field.path
        );
    }
}

#[test]
fn tiers_are_used_and_the_basics_stay_few() {
    let mut counts = [0_usize; 3];
    for field in schema::leaves() {
        let index = match field.tier {
            Tier::Basic => 0,
            Tier::Advanced => 1,
            Tier::Expert => 2,
        };
        counts[index] += 1;
    }
    for (index, count) in counts.iter().enumerate() {
        assert!(*count > 0, "tier {index} is never used");
    }
    assert!(
        counts[0] <= 12,
        "{} basic settings; the first screen must stay readable",
        counts[0]
    );
}

#[test]
fn schema_is_stable() {
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&Config::json_schema()).expect("schema serializes")
    );
    let path = snapshot_path();

    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &rendered).expect("write snapshot");
        return;
    }

    let current = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} is missing ({err}); run UPDATE_SNAPSHOTS=1 cargo test -p humanitl-config --test schema",
            path.display()
        )
    });
    assert_eq!(
        current, rendered,
        "the schema changed; check the diff and run \
         UPDATE_SNAPSHOTS=1 cargo test -p humanitl-config --test schema"
    );
}
