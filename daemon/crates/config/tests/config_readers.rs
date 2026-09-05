//! Das Register der Leser: jeder Schema-Pfad steht hier genau einmal.
//!
//! `docs/CONFIG.md` entsteht aus dem Schema und kann deshalb keinen Schlüssel
//! vergessen. Über die **Wirkung** eines Schlüssels sagt der Generator von sich
//! aus nichts, und ein Schlüssel, der beschrieben, geprüft und von niemandem
//! gelesen wird, sieht von außen aus wie ein wirksamer (HUM-101). Sieben solche
//! Fälle standen im Dokument, zwei davon sagten Ressourcengrenzen zu, die es
//! nicht gab.
//!
//! Eine Heuristik über den Feldnamen taugt dafür nicht: `env` trifft im
//! Repository 244-mal, `profile` 464-mal, `timeout_secs` ist Teilzeichenkette
//! von vier Schlüsseln, `enabled` ist zweimal vergeben (`findings.enabled`,
//! `agent.briefing.enabled`), ein Doku-Kommentar zählt als Treffer, und
//! `humanitl config get` liest über serde ohnehin jeden Schlüssel. Eine solche
//! Suche fände weder zuverlässig noch träfe sie den Unterschied zwischen
//! „wirkt" und „wird serialisiert".
//!
//! Deshalb dieses Register: eine Zeile je Pfad, genau eine Einstufung, von Hand
//! gepflegt wie das Diagnostik-Register. `effective` ist die Behauptung eines
//! Menschen, keine Messung — das ist Absicht. Das Register soll den
//! **vergessenen** Schlüssel finden; wer beim Anlegen „wirkt" schreibt, ohne zu
//! verdrahten, hat nicht übersehen, sondern gelogen.
//!
//! Wer ein Feld hinzufügt, hängt hier eine Zeile an. Wer eine Zeile
//! `pending(HUM-xxx)` schreibt, setzt dieselbe Kennung als `x-pending-issue` an
//! das Feld in `src/model.rs`; von dort holt sie der Generator in die Spalte
//! „Wirkung" von `docs/CONFIG.md`.
//!
//! **Wo das Register aufhört.** Eine Zeile deckt einen Blattpfad des Schemas,
//! und Blätter findet [`humanitl_config::schema`] über `properties`. Drei
//! Dinge liegen damit außerhalb, und alle drei sind hier abgesichert, statt
//! stillschweigend zu fehlen:
//!
//! 1. **Behälter.** Die Schlüssel *in* einer freien Tabelle (`sandbox.env`,
//!    `resolver.overrides`, `experimental.upstream_port_map`) und die Elemente
//!    einer Liste sind keine Blätter; der Behälter selbst trägt die Zeile, und
//!    seine Einstufung gilt für alles darin. Das reicht, solange darin
//!    Skalare stehen. `the_schema_hides_no_leaf_from_the_walk` wird rot,
//!    sobald jemand eine Struktur in einen Behälter legt — dann muss die
//!    Grenze neu gezogen werden. Geprüft wird dabei nicht nur die erste Ebene:
//!    `Vec<Enum mit Struktur-Varianten>` legt die Felder unter
//!    `items.oneOf[].properties` und `BTreeMap<String, Vec<Struktur>>` zwei
//!    Behälter tief; beide Formen kämen an einer Prüfung vorbei, die nur den
//!    Knoten direkt unter dem Behälter ansieht.
//! 2. **Zusammengelegte Strukturen.** `#[serde(flatten)]` erzeugt bei diesem
//!    Generator **kein** `allOf`: `inline_subschemas` schmilzt die Felder in
//!    die `properties` des Elternknotens, und sie erscheinen als gewöhnliche
//!    Blätter. Nachgemessen mit einem `flatten` in `Experimental`: Der
//!    Vollständigkeitstest nennt es als `experimental.enabled` ohne
//!    Registerzeile. Der Riegel gegen `allOf`, `anyOf` und `$ref` in
//!    `the_schema_hides_no_leaf_from_the_walk` bleibt trotzdem stehen — er
//!    fängt den Tag ab, an dem jemand `inline_subschemas` abschaltet und die
//!    Felder wieder hinter einem Verweis verschwinden.
//! 3. **Aliase.** `alias::ALIASES` steht neben dem Schema, nicht darin.
//!    `every_alias_leads_to_a_registered_key` hält fest, dass jeder Alias auf
//!    ein registriertes Blatt zeigt und selbst keines ist; ein Schlüssel, den
//!    es nur unter seinem alten Namen gäbe, fiele damit auf.
//!
//! Entfallene Schlüssel (`alias::RETIRED`) stehen mit Absicht **nicht** im
//! Register: Sie sind keine Einstellung mehr, sondern eine Warnung beim Laden.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use humanitl_config::pending::Readiness;
use humanitl_config::{Config, alias, schema};
use serde_json::Value;

/// Das Register. Sortiert wie `schema::leaf_paths()`, eine Zeile je Blattpfad.
///
/// `effective`: Der Schlüssel wird gelesen und wirkt.
/// `pending(HUM-xxx)`: Er hat heute keinen Leser; das genannte Issue
/// entscheidet ihn, durch Einbau oder durch Streichung. Jeder solche Eintrag
/// trägt darüber einen Satz, warum.
const REGISTER: &[(&str, &str)] = &[
    ("agent.adapter", "effective"),
    ("agent.briefing.enabled", "effective"),
    ("agent.command", "effective"),
    ("experimental.h2_upstream", "effective"),
    // Der Proxy lenkt keinen Port um; HUM-088 entfernt den Schlüssel, statt ihm
    // nachträglich einen Leser zu geben.
    ("experimental.upstream_port_map", "pending(HUM-088)"),
    // Ein WebSocket-Upgrade entscheidet heute allein die Regel; der Schalter
    // trifft im Proxy auf nichts.
    ("experimental.ws_hold", "pending(HUM-121)"),
    ("findings.email_allow_domains", "effective"),
    ("findings.enabled", "effective"),
    ("findings.ignored_hashes", "effective"),
    ("findings.user_terms", "effective"),
    ("hold.ask_mode", "effective"),
    ("hold.hard_block_checksum_secrets", "effective"),
    ("hold.timeout_secs", "effective"),
    // Keine der beiden Rumpf-Spannen hat eine Uhr: Der Anfrage-Rumpf wird ohne
    // Frist gepuffert, und der gestreamte Antwort-Rumpf läuft unbegrenzt weiter,
    // sobald die Antwort-Kopfzeilen da sind. HUM-120 legt beide auf diesen
    // Schlüssel, als Stille zwischen zwei Stücken.
    ("limits.body_timeout_secs", "pending(HUM-120)"),
    ("limits.connect_timeout_secs", "effective"),
    ("limits.event_buffer", "effective"),
    ("limits.header_timeout_secs", "effective"),
    ("limits.hold_body_cap_bytes", "effective"),
    ("limits.hold_max_bytes", "effective"),
    ("limits.hold_max_flows", "effective"),
    ("limits.max_decompress_ratio", "effective"),
    ("limits.preview_cap_bytes", "effective"),
    ("limits.recorder_max_body_bytes", "effective"),
    ("llm.endpoint", "effective"),
    ("llm.models", "effective"),
    ("llm.passthrough_paths", "effective"),
    // Der Rücktausch von Pseudonymen kommt mit HUM-079; bis dahin puffert
    // niemand eine Antwort dafür.
    ("pseudonyms.max_response_bytes", "pending(HUM-079)"),
    ("pseudonyms.translate_responses", "pending(HUM-079)"),
    ("recorder.inline_max_bytes", "effective"),
    ("recorder.retention_days", "effective"),
    ("resolver.cache_ttl_secs", "effective"),
    // Der Adapter über dem Namensdienst des Systems kann keinen eigenen Server
    // ansprechen; der Daemon warnt beim Start und fragt trotzdem /etc/resolv.conf.
    // HUM-115 baut den Hickory-Adapter dahinter und führt damit den DNS-Beweis.
    ("resolver.nameserver", "pending(HUM-115)"),
    ("resolver.overrides", "effective"),
    ("resolver.prefer", "effective"),
    // Der Vertrauensanker aus der Datei erreicht `ClientTls::new` nicht;
    // HUM-087 verdrahtet ihn samt `--allow-test-ca`.
    ("resolver.test_ca", "pending(HUM-087)"),
    ("sandbox.env", "effective"),
    ("sandbox.profile", "effective"),
    ("sandbox.work_dir", "effective"),
    ("sandbox.work_mode", "effective"),
    ("ui.language", "effective"),
    // Die Oberfläche kann die Konfiguration nicht lesen: dem Client fehlt
    // `GetConfig`, und der Melder antwortet fest mit `true`.
    ("ui.notifications", "pending(HUM-069)"),
    // Es gibt keinen Ton, den ein Schalter abschalten könnte.
    ("ui.sound", "pending(HUM-121)"),
    // Gelesen in `SandboxService::launch`, wenn das Terminal der Sitzung
    // entsteht: `TerminalHub::notice` schweigt ohne diesen Schalter
    // (HUM-042).
    ("ui.terminal_notices", "effective"),
    // Dasselbe wie bei `ui.notifications`: Das Erscheinungsbild steht fest im
    // Programm, bis der Einstellungs-Bildschirm die Werte holt.
    ("ui.theme", "pending(HUM-069)"),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Das Register als Abbildung, mit der Prüfung, dass jede Zeile lesbar ist.
fn register() -> BTreeMap<&'static str, Readiness> {
    let mut out = BTreeMap::new();
    for (path, text) in REGISTER {
        let Some(readiness) = Readiness::parse(text) else {
            panic!(
                "{path} has the unreadable entry {text:?}; \
                 write either effective or pending(HUM-xxx)"
            );
        };
        assert!(
            out.insert(*path, readiness).is_none(),
            "{path} appears twice in the register"
        );
    }
    out
}

/// Was zwischen Schema und Register auseinandergeht.
///
/// Links die Pfade, die das Schema kennt und das Register nicht; rechts die
/// Zeilen, die keinen Pfad mehr haben. Eine reine Funktion, damit der Fall
/// „ein Feld mehr" auch ohne ein zweites Schema prüfbar ist.
fn drift<'a>(
    schema_paths: &BTreeSet<&'a str>,
    register_paths: &BTreeSet<&'a str>,
) -> (Vec<&'a str>, Vec<&'a str>) {
    let missing = schema_paths.difference(register_paths).copied().collect();
    let stale = register_paths.difference(schema_paths).copied().collect();
    (missing, stale)
}

#[test]
fn every_schema_path_has_a_register_line() {
    let register = register();
    let register_paths: BTreeSet<&str> = register.keys().copied().collect();
    let (missing, stale) = drift(&schema::leaf_paths(), &register_paths);
    assert!(
        missing.is_empty(),
        "the schema has {} key(s) without a register line: {missing:?}; \
         add one line each to REGISTER in tests/config_readers.rs",
        missing.len()
    );
    assert!(
        stale.is_empty(),
        "the register names {} key(s) the schema does not have: {stale:?}",
        stale.len()
    );
}

#[test]
fn a_new_schema_path_without_a_register_line_is_named() {
    // Die Probe des Registers selbst: ein Pfad, den das Schema kennt und das
    // Register nicht, muss beim Namen genannt werden. Ohne diesen Fall prüfte
    // `every_schema_path_has_a_register_line` nur sich selbst.
    let register = register();
    let register_paths: BTreeSet<&str> = register.keys().copied().collect();
    let mut with_extra = schema::leaf_paths();
    with_extra.insert("limits.new_shiny_cap_bytes");
    let (missing, stale) = drift(&with_extra, &register_paths);
    assert_eq!(missing, vec!["limits.new_shiny_cap_bytes"]);
    assert!(stale.is_empty(), "{stale:?}");

    // Und andersherum: eine Zeile ohne Feld fällt genauso auf.
    let mut with_gone: BTreeSet<&str> = schema::leaf_paths();
    assert!(with_gone.remove("hold.timeout_secs"));
    let (missing, stale) = drift(&with_gone, &register_paths);
    assert!(missing.is_empty(), "{missing:?}");
    assert_eq!(stale, vec!["hold.timeout_secs"]);
}

#[test]
fn the_register_and_the_schema_agree_on_every_entry() {
    let register = register();
    for field in schema::leaves() {
        let Some(expected) = register.get(field.path.as_str()) else {
            panic!("{} has no register line", field.path);
        };
        assert_eq!(
            &field.readiness, expected,
            "{}: the register says {expected} and the schema says {}; \
             the entry and x-pending-issue in src/model.rs must be the same",
            field.path, field.readiness
        );
    }
}

#[test]
fn the_register_is_sorted_like_the_schema() {
    // Sortiert heißt: eine neue Zeile hat genau einen Platz, und zwei Agenten,
    // die je ein Feld anlegen, kollidieren nicht in derselben Zeile.
    let mut sorted: Vec<&str> = REGISTER.iter().map(|(path, _)| *path).collect();
    let listed = sorted.clone();
    sorted.sort_unstable();
    assert_eq!(listed, sorted, "the register is out of order");
}

/// Der Abschnitt eines Issues aus den Sprint-Dateien, also alles zwischen
/// seiner Überschrift und der nächsten.
fn issue_section(issue: &str) -> Option<String> {
    let backlog = repo_root().join("backlog");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&backlog)
        .expect("read backlog/")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            let is_markdown = path.extension().is_some_and(|extension| extension == "md");
            let is_sprint = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("sprint-"));
            is_markdown && is_sprint
        })
        .collect();
    files.sort();
    let heading = format!("## {issue} ");
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read a sprint file");
        let Some(start) = text.find(&heading) else {
            continue;
        };
        let rest = &text[start..];
        let end = rest[1..]
            .find("\n## ")
            .map_or(rest.len(), |offset| offset + 1);
        return Some(rest[..end].to_owned());
    }
    None
}

#[test]
fn every_pending_entry_points_at_an_issue_that_names_the_key() {
    // Ein Verweis, der ins Leere geht, ist schlechter als keiner: Genau daran
    // krankte die Warnzeile zu `resolver.test_ca`, die auf HUM-024 zeigte, ohne
    // dass HUM-024 den Schlüssel je genannt hätte.
    //
    // Die Zeile in `BACKLOG.md` allein reicht dafür nicht: Sie bleibt stehen,
    // wenn ein Issue erledigt ist. Ein Zeiger auf ein gemergtes Issue sähe
    // damit gültig aus, und der Schlüssel wartete auf etwas, das nie mehr
    // kommt. Geprüft wird deshalb die Spezifikation: Sie muss den Schlüssel
    // wörtlich nennen — dort steht, was mit ihm geschehen soll.
    let plan = std::fs::read_to_string(repo_root().join("BACKLOG.md")).expect("read BACKLOG.md");
    for (path, text) in REGISTER {
        let Some(readiness) = Readiness::parse(text) else {
            panic!("{path} has the unreadable entry {text:?}");
        };
        let Some(issue) = readiness.issue() else {
            continue;
        };
        assert!(
            plan.contains(&format!("| {issue} |")),
            "{path} points at {issue}, which BACKLOG.md does not list as an issue"
        );
        let Some(section) = issue_section(issue) else {
            panic!(
                "{path} points at {issue}, but no backlog/sprint-*.md has a heading `## {issue}`"
            );
        };
        assert!(
            section.contains(path),
            "{path} points at {issue}, and the specification of {issue} never names the key; \
             add a path line and an acceptance criterion there, or point the register somewhere \
             else"
        );
    }
}

#[test]
fn no_group_carries_a_pending_note() {
    // `x-pending-issue` an einer Gruppe wirkt nicht: `leaves()` filtert Gruppen
    // weg, das Register kennt nur Blätter, und `docs/CONFIG.md` zeigt für jedes
    // Blatt darunter weiter „ja". Eine ganze Gruppe als offen markieren zu
    // wollen ist ein naheliegender Irrtum, und er bliebe sonst still: Nur der
    // Schema-Schnappschuss würde rot, und den schreibt derselbe Mensch neu.
    for field in schema::fields().iter().filter(|field| field.group) {
        assert!(
            !field.readiness.is_pending(),
            "the group {} carries {}; the note belongs on each leaf below it, a group has no \
             register line",
            field.path,
            field.readiness
        );
    }
}

#[test]
fn the_keys_without_a_reader_are_the_known_ones() {
    // Die Liste aus HUM-101, plus die drei, die das Register selbst gefunden
    // hat (`resolver.nameserver`, `ui.theme`, `resolver.test_ca` aus HUM-087).
    // Sie steht hier, damit ein achter Fall nicht unbemerkt dazukommt: Wer
    // einen Schlüssel verdrahtet, streicht ihn hier und im Register zugleich.
    let pending: Vec<&str> = register()
        .iter()
        .filter(|(_, readiness)| readiness.is_pending())
        .map(|(path, _)| *path)
        .collect();
    assert_eq!(
        pending,
        vec![
            "experimental.upstream_port_map",
            "experimental.ws_hold",
            "limits.body_timeout_secs",
            "pseudonyms.max_response_bytes",
            "pseudonyms.translate_responses",
            "resolver.nameserver",
            "resolver.test_ca",
            "ui.notifications",
            "ui.sound",
            "ui.theme",
        ]
    );
}

/// Jeder Knoten des Schemas, mit seinem Pfad, in Reihenfolge.
fn walk_raw(node: &Value, prefix: &str, visit: &mut dyn FnMut(&str, &Value)) {
    let Some(properties) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, child) in properties {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        visit(&path, child);
        walk_raw(child, &path, visit);
    }
}

/// Ob ein Knoten selbst Felder trägt.
fn holds_fields(node: &Value) -> bool {
    node.get("properties")
        .and_then(Value::as_object)
        .is_some_and(|map| !map.is_empty())
}

/// Ob irgendwo unter diesem Knoten Felder stecken, die der Durchlauf nicht
/// sieht.
///
/// Eine Ebene reicht dafür nicht. `Vec<T>` mit einem Enum aus
/// Struktur-Varianten legt die Felder unter `items.oneOf[].properties`, und
/// `BTreeMap<String, Vec<T>>` legt sie zwei Behälter tief; in beiden Fällen
/// trägt der Knoten direkt unter dem Behälter selbst keine `properties`, und
/// eine Prüfung, die nur dort nachsieht, geht daran vorbei. Deshalb steigt
/// diese Funktion durch jeden Behälter und jede Variante ab.
fn hides_fields(node: &Value) -> bool {
    if holds_fields(node) {
        return true;
    }
    for key in [
        "items",
        "additionalProperties",
        "propertyNames",
        "contains",
        "not",
    ] {
        if node.get(key).is_some_and(|child| {
            child.is_object() && (hides_fields(child) || array_hides_fields(child, "prefixItems"))
        }) {
            return true;
        }
    }
    for key in ["oneOf", "anyOf", "allOf", "prefixItems"] {
        if array_hides_fields(node, key) {
            return true;
        }
    }
    false
}

/// Ob eine der Alternativen unter `key` Felder versteckt.
fn array_hides_fields(node: &Value, key: &str) -> bool {
    node.get(key)
        .and_then(Value::as_array)
        .is_some_and(|variants| variants.iter().any(hides_fields))
}

#[test]
fn the_schema_hides_no_leaf_from_the_walk() {
    // Der Durchlauf steigt über `properties` ab. Alles, was Felder anderswo
    // unterbringt, wäre für das Register unsichtbar — und unsichtbar ist genau
    // der Zustand, den dieses Register beenden soll.
    let schema = Config::json_schema();
    let mut checked = 0_usize;
    walk_raw(&schema, "", &mut |path, node| {
        checked += 1;
        for keyword in ["allOf", "anyOf", "$ref"] {
            assert!(
                node.get(keyword).is_none(),
                "{path} uses {keyword}; serde(flatten) and $ref hide fields from the walk \
                 and therefore from the register"
            );
        }
        // Ein Behälter trägt die Zeile für alles, was in ihm steht. Das gilt,
        // solange darin Skalare stehen — auch mehrere Behälter tief und auch
        // in den Varianten eines Enums, deshalb `hides_fields` und nicht
        // `holds_fields`. Die Meldung nennt den Behälter, nicht die Stelle
        // tief darin, sonst sucht der Nächste im Dunkeln.
        if let Some(values) = node
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            assert!(
                !hides_fields(values),
                "{path} is a free table whose values carry fields; its inner keys have no \
                 register line and no leaf in the schema"
            );
        }
        if let Some(items) = node.get("items") {
            assert!(
                !hides_fields(items),
                "{path} is a list whose elements carry fields; its inner keys have no \
                 register line and no leaf in the schema"
            );
        }
        if let Some(variants) = node.get("oneOf").and_then(Value::as_array) {
            for variant in variants {
                assert!(
                    variant.get("const").is_some() && !hides_fields(variant),
                    "{path} has a oneOf variant that is not a plain value; its fields have no \
                     register line"
                );
            }
        }
    });
    assert!(checked > 40, "only {checked} nodes, the walk found nothing");
    // Die drei Behälter von heute, beim Namen: Wer einen vierten anlegt, sieht
    // hier, dass seine Einträge keine eigene Zeile bekommen.
    assert_eq!(
        schema::free_table_paths().into_iter().collect::<Vec<_>>(),
        vec![
            "experimental.upstream_port_map",
            "resolver.overrides",
            "sandbox.env"
        ]
    );
}

#[test]
fn every_alias_leads_to_a_registered_key() {
    // Aliase stehen neben dem Schema. Ein Schlüssel, den es nur unter seinem
    // alten Namen gäbe, hätte keinen Blattpfad und damit keine Registerzeile.
    let register = register();
    for entry in alias::ALIASES {
        assert!(
            register.contains_key(entry.canonical),
            "{} points at {}, which has no register line",
            entry.old,
            entry.canonical
        );
        assert!(
            !register.contains_key(entry.old),
            "{} is an alias and a registered key at the same time",
            entry.old
        );
    }
}

#[test]
fn no_retired_key_has_a_register_line() {
    // Ein entfallener Schlüssel ist keine Einstellung mehr. Stünde er hier,
    // behauptete das Register eine Wirkung für etwas, das es nicht mehr gibt.
    let register = register();
    for entry in alias::RETIRED {
        assert!(
            !register.contains_key(entry.path),
            "{} is retired and still has a register line",
            entry.path
        );
    }
    assert!(
        alias::retired("limits.idle_timeout_secs").is_some(),
        "the removed idle limit must stay known to the loader"
    );
}

#[test]
fn the_removed_idle_limit_is_gone_from_the_schema() {
    // HUM-101, erste Entscheidung: `limits.idle_timeout_secs` beschrieb dieselbe
    // Spanne wie `limits.header_timeout_secs` und ist entfernt. Bleibt er
    // versehentlich zurück, hat er wieder keinen Leser.
    assert!(!schema::known_paths().contains("limits.idle_timeout_secs"));
    assert!(schema::leaf_paths().contains("limits.header_timeout_secs"));
}
