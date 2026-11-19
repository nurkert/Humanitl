//! Jeder Fix der Art `ChangeSetting` im Daemon und in der Kommandozeile ist
//! ein Wert, den `humanitl config set` annimmt (HUM-507).
//!
//! Der Test liest den Quelltext aller Crates und Programme unter `daemon/`
//! und sucht jede Stelle, die einen `ChangeSetting` baut, außerhalb von
//! Testcode (`#[cfg(test)]`, Testmodule in eigenen Dateien). Je Stelle gilt:
//!
//! - Schlüssel und Wert sind Literale: Der Wert läuft durch
//!   [`humanitl_config::value::check_setting`], dieselbe Prüfung wie
//!   `config set` (Schema, Aufzählung, Grenzen, Prüfung der Konfiguration).
//! - Der Schlüssel ist `sandbox.backend` und der Wert eine Konstante
//!   `…_BACKEND`: Jeder Wert der Aufzählung läuft durch dieselbe Prüfung.
//! - Sonst steht die Stelle mit ihrem Quelltext in [`SITES`], mit den Werten,
//!   die sie bauen kann, und diese Werte laufen durch dieselbe Prüfung. Eine
//!   neue Stelle, oder eine geänderte, die nicht eingetragen ist, macht den
//!   Test rot; ebenso ein Eintrag, dessen Stelle es nicht mehr gibt.
//!
//! Werte, die erst zur Laufzeit feststehen, bauen die Erzeuger über
//! [`humanitl_config::value::change_setting`], [`humanitl_config::value::reset_fix`]
//! und [`humanitl_config::value::raised`]; die prüfen selbst und stehen
//! hier nur mit ihrem eigenen Bau. Ein Feld, dessen Wert nur der Mensch
//! kennt, öffnet `FixAction::EditSetting` ohne Wert; ein `ChangeSetting`
//! mit leerem Wert nimmt `config set` für keinen solchen Schlüssel an und
//! fällt hier durch.
//!
//! Dasselbe gilt für die Befunde, die die App selbst baut
//! (`FixAction.changeSetting` unter `app/lib/`, [`DART_SITES`]).
//! Der Scanner steht in `support/scan.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/scan.rs"]
mod scan;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use humanitl_config::value::{check_setting, raised, reset_fix};
use humanitl_core::FixAction;

use scan::{
    constructions, daemon_root, dart_constructions, dart_literal, dart_masked,
    dart_string_constants, fields, literal, masked, test_regions,
};

/// Eine Stelle, deren Schlüssel oder Wert kein Literal ist.
struct Site {
    /// Die Datei, relativ zu `daemon/`.
    file: &'static str,
    /// Der Rumpf des Baus, Leerraum zu je einem Leerzeichen gefaltet.
    body: &'static str,
    /// Was die Stelle bauen kann; leer nur für eine reine Weitergabe.
    samples: &'static [(&'static str, &'static str)],
    /// Warum die Werte in `samples` alle sind, die vorkommen.
    why: &'static str,
}

/// Ein Beispielwert für jeden Wert, den die Aufzählung des Schlüssels
/// erlaubt.
const ANY_ALLOWED: &str = "*";

/// Eine Weitergabe baut keinen neuen Vorschlag, sie übersetzt einen, den ein
/// Erzeuger schon gebaut hat (Protokoll, Aufnahme, Fake).
const PASSTHROUGH: &[(&str, &str)] = &[];

/// Die Stellen, deren Schlüssel oder Wert kein Literal ist.
const SITES: &[Site] = &[
    Site {
        file: "crates/config/src/value.rs",
        body: "key: field.path.clone(), value: scalar(default),",
        samples: &[],
        why: "default_fix: die Vorgabe jedes Blattes; \
              every_leaf_default_and_reset_is_a_value_config_set_takes prüft alle",
    },
    Site {
        file: "crates/config/src/value.rs",
        body: "key: field.path.clone(), value: allowed.first().cloned().unwrap_or_default(),",
        samples: &[],
        why: "der erste erlaubte Wert einer Aufzählung; \
              every_first_allowed_value_is_a_value_config_set_takes prüft alle",
    },
    Site {
        file: "crates/config/src/value.rs",
        body: "key: key.to_owned(), value,",
        samples: &[],
        why: "change_setting: prüft selbst mit check_setting",
    },
    Site {
        file: "crates/config/src/value.rs",
        body: "key: key.to_owned(), value: best.to_string(),",
        samples: &[],
        why: "raised: prüft selbst mit check_setting",
    },
    Site {
        file: "crates/config/src/load.rs",
        body: "key: BACKEND.to_owned(), value: KERNEL_LEVEL_BACKEND.to_owned(),",
        samples: &[("sandbox.backend", ANY_ALLOWED)],
        why: "Konstanten",
    },
    Site {
        file: "crates/findings/src/decode.rs",
        body: "key: \"limits.preview_cap_bytes\".to_owned(), value: doubled_cap(cap_bytes).to_string(),",
        samples: &[
            ("limits.preview_cap_bytes", "2048"),
            ("limits.preview_cap_bytes", "9223372036854775807"),
        ],
        why: "das Doppelte eines Caps von mindestens 1024, höchstens i64::MAX \
              (the_doubled_cap_stays_a_number_config_set_reads)",
    },
    Site {
        file: "crates/findings/src/decode.rs",
        body: "key: \"limits.preview_cap_bytes\".to_owned(), value: size.to_string(),",
        samples: &[
            ("limits.preview_cap_bytes", "1025"),
            ("limits.preview_cap_bytes", "1073741824"),
        ],
        why: "die Größe eines Rumpfs über dem Cap, also über 1024 und im Speicher",
    },
    Site {
        file: "crates/sandbox/src/backend.rs",
        body: "key: \"sandbox.backend\".to_owned(), value: required.to_owned(),",
        samples: &[("sandbox.backend", ANY_ALLOWED)],
        why: "das Backend, das ein Profil verlangt, nur wenn es zugelassen ist",
    },
    Site {
        file: "crates/sandbox/src/agent/opencode.rs",
        body: "key: \"llm.endpoint\".to_owned(), value: EXAMPLE_ENDPOINT.to_owned(),",
        samples: &[("llm.endpoint", "https://api.example.com/v1")],
        why: "Konstante",
    },
    Site {
        file: "crates/sandbox/src/doctor/checks.rs",
        body: "key: \"llm.endpoint\".to_owned(), value: opencode::EXAMPLE_ENDPOINT.to_owned(),",
        samples: &[("llm.endpoint", "https://api.example.com/v1")],
        why: "Konstante",
    },
    Site {
        file: "crates/ipc/src/validate.rs",
        body: "key: \"llm.endpoint\".to_owned(), value: EXAMPLE_ENDPOINT.to_owned(),",
        samples: &[("llm.endpoint", "https://api.example.com/v1")],
        why: "Konstante",
    },
    Site {
        file: "crates/proxy/src/llm_probe.rs",
        body: "key: \"llm.endpoint\".to_owned(), value: EXAMPLE_ENDPOINT.to_owned(),",
        samples: &[("llm.endpoint", "https://api.example.com/v1")],
        why: "Konstante",
    },
    Site {
        file: "crates/ipc/src/convert.rs",
        body: "key: key.clone(), value: value.clone(),",
        samples: PASSTHROUGH,
        why: "Übersetzung in das Protokoll",
    },
    Site {
        file: "crates/ipc/src/fake/player.rs",
        body: "key: key.clone(), value: value.clone(),",
        samples: PASSTHROUGH,
        why: "der Fake spielt Befunde aus einem Drehbuch ab",
    },
    Site {
        file: "crates/recorder/src/types.rs",
        body: "key: key()?, value",
        samples: PASSTHROUGH,
        why: "Lesen eines aufgenommenen Befunds mit Wert; ein leerer wird EditSetting",
    },
    Site {
        file: "bin/humanitl/src/cmd/mod.rs",
        body: "key: change.key.clone(), value: change.value.clone(),",
        samples: PASSTHROUGH,
        why: "Übersetzung aus dem Protokoll",
    },
];

/// Was `config set` zu `key` und `value` sagt, als Zeile für den Bericht.
fn refusal(key: &str, value: &str) -> Option<String> {
    check_setting(key, value)
        .err()
        .map(|err| format!("config set {key} {value:?} is refused: {}", err.why))
}

/// Wie [`refusal`], und [`ANY_ALLOWED`] steht für jeden erlaubten Wert.
fn refusals(key: &str, value: &str) -> Vec<String> {
    if value != ANY_ALLOWED {
        return refusal(key, value).into_iter().collect();
    }
    let allowed = humanitl_config::schema::field(key)
        .and_then(|field| field.allowed.clone())
        .unwrap_or_default();
    if allowed.is_empty() {
        return vec![format!("{key} has no enumeration to take every value of")];
    }
    allowed
        .iter()
        .filter_map(|value| refusal(key, value))
        .collect()
}

/// Ob ein Ausdruck eine Konstante eines Backends ist: ein Pfad, dessen
/// letzter Name auf `_BACKEND` endet, mit `.to_owned()`.
fn backend_constant(expr: &str) -> bool {
    expr.strip_suffix(".to_owned()").is_some_and(|path| {
        let name = path.rsplit("::").next().unwrap_or(path);
        name.ends_with("_BACKEND") && name.chars().all(|c| c.is_ascii_uppercase() || c == '_')
    })
}

/// Jeder `ChangeSetting` außerhalb von Testcode baut einen Wert, den
/// `humanitl config set` annimmt (HUM-507, ausgedehnt von HUM-503).
///
/// Mutationsbeweis: `secs.saturating_mul(2).to_string()` zurück in
/// `proxy/src/limits.rs` ist eine Stelle, die hier nicht eingetragen ist;
/// `value: format!("remove {name}")` zurück in `config/src/validate.rs`
/// ebenso; ein Literal wie `sandbox.env = "remove LD_PRELOAD"` in
/// `humanitld` fällt durch die Prüfung von `config set`. Alle drei: rot.
#[test]
fn every_change_setting_fix_is_a_value_config_set_takes() {
    let root = daemon_root();
    let found = constructions(&root);
    assert!(
        found.len() > 20,
        "the scan finds the sites it is meant to find: {found:#?}"
    );
    let mut problems = String::new();
    let mut used = vec![0usize; SITES.len()];
    for site in &found {
        let registered = SITES
            .iter()
            .position(|known| known.file == site.file && known.body == site.body);
        if let Some(index) = registered {
            used[index] += 1;
            continue;
        }
        let Some((key, value)) = fields(&site.body) else {
            let _ = writeln!(
                problems,
                "{}:{}: not key and value: {}",
                site.file, site.line, site.body
            );
            continue;
        };
        match (literal(&key), literal(&value)) {
            (Some(key), Some(value)) => {
                if let Some(refused) = refusal(&key, &value) {
                    let _ = writeln!(problems, "{}:{}: {refused}", site.file, site.line);
                }
            }
            // Eine Konstante eines Backends (`…_BACKEND`) ist ein Wert der
            // Aufzählung `sandbox.backend`; geprüft wird jeder erlaubte.
            (Some(key), None) if key == "sandbox.backend" && backend_constant(&value) => {
                for refused in refusals(&key, ANY_ALLOWED) {
                    let _ = writeln!(problems, "{}:{}: {refused}", site.file, site.line);
                }
            }
            _ => {
                let _ = writeln!(
                    problems,
                    "{}:{}: a ChangeSetting whose value is decided at run time; build it with \
                     humanitl_config::value::change_setting, or register it in SITES with the \
                     values it can take:\n    body: {:?}",
                    site.file, site.line, site.body
                );
            }
        }
    }
    for (site, count) in SITES.iter().zip(&used) {
        if *count == 0 {
            let _ = writeln!(
                problems,
                "{}: registered but not found (stale entry): {}",
                site.file, site.body
            );
        }
        if site.samples.is_empty() {
            assert!(!site.why.is_empty(), "{}", site.body);
        }
        for (key, value) in site.samples {
            for refused in refusals(key, value) {
                let _ = writeln!(problems, "{} ({}): {refused}", site.file, site.why);
            }
        }
    }
    assert!(problems.is_empty(), "{problems}");
}

/// Die Vorgabe jedes Blattes und ihr Zurücksetzen nimmt `config set` an:
/// der Wert von `default_fix` und `reset_fix`.
#[test]
fn every_leaf_default_and_reset_is_a_value_config_set_takes() {
    let mut reset = 0;
    for field in humanitl_config::schema::leaves() {
        if let Some(FixAction::ChangeSetting { key, value }) =
            humanitl_config::value::default_fix(field)
        {
            assert_eq!(refusal(&key, &value), None);
        }
        if let Some(FixAction::ChangeSetting { key, value }) = reset_fix(&field.path) {
            assert_eq!(refusal(&key, &value), None);
            reset += 1;
        }
    }
    assert!(reset > 50, "most leaves reset: {reset}");
    // Ein Schlüssel außerhalb des Schemas und eine Gruppe haben keinen Wert,
    // den `config set` nimmt.
    assert_eq!(reset_fix("llm.servers.home.endpoint"), None);
    assert_eq!(reset_fix("limits"), None);
}

/// Der erste erlaubte Wert jeder Aufzählung, der Vorschlag von
/// `check_value`, nimmt `config set` an.
#[test]
fn every_first_allowed_value_is_a_value_config_set_takes() {
    for field in humanitl_config::schema::leaves() {
        let Some(first) = field.allowed.as_ref().and_then(|values| values.first()) else {
            continue;
        };
        assert_eq!(refusal(&field.path, first), None, "{}", field.path);
    }
}

/// Eine höhere Grenze bleibt unter der Obergrenze der Prüfung, und an der
/// Obergrenze gibt es keine (HUM-507). Mutationsbeweis: ohne die Suche nach
/// der Grenze schlägt `raised` für `limits.body_timeout_secs` 172800 vor.
#[test]
fn a_raised_limit_stays_within_the_bounds_config_set_checks() {
    const DAY: u64 = 86_400;
    let value = |fix: Option<FixAction>| match fix {
        Some(FixAction::ChangeSetting { value, .. }) => Some(value),
        other => panic!("not a ChangeSetting: {other:?}"),
    };
    assert_eq!(
        value(raised("limits.body_timeout_secs", 60)),
        Some("120".to_owned())
    );
    assert_eq!(
        value(raised("limits.body_timeout_secs", DAY - 10)),
        Some(DAY.to_string())
    );
    assert_eq!(raised("limits.body_timeout_secs", DAY), None);
    assert_eq!(
        value(raised("limits.max_client_connections", 40_000)),
        Some("65536".to_owned())
    );
    assert_eq!(raised("limits.max_client_connections", 65_536), None);
    assert_eq!(
        value(raised("limits.header_timeout_secs", 3_000)),
        Some("3600".to_owned())
    );
    // Das Cap des Haltens bleibt unter `limits.hold_max_bytes` (Vorgabe
    // 256 MiB), sonst lehnte die Prüfung den Wert ab.
    assert_eq!(
        value(raised("limits.hold_body_cap_bytes", 200 << 20)),
        Some((256u64 << 20).to_string())
    );
    // Über `i64::MAX` liest `config set` keine Zahl.
    let ceiling = u64::try_from(i64::MAX).unwrap();
    assert_eq!(
        value(raised("limits.preview_cap_bytes", ceiling / 2 + 10)),
        Some(ceiling.to_string())
    );
    assert_eq!(raised("limits.preview_cap_bytes", ceiling), None);
    assert_eq!(raised("limits.preview_cap_bytes", u64::MAX), None);
    assert_eq!(raised("no.such.key", 10), None);
}

/// Der Scanner selbst: Testcode zählt nicht, Muster zählen nicht, ein
/// Literal wird gelesen.
#[test]
fn the_scanner_skips_tests_and_patterns_and_reads_literals() {
    let text = r#"
fn real() -> FixAction {
    FixAction::ChangeSetting { key: "a.b".to_owned(), value: "x{".to_owned() }
}
fn read(fix: FixAction) {
    if let FixAction::ChangeSetting { key, value } = fix {}
    match fix { FixAction::ChangeSetting { key, .. } => {}, _ => {} }
    let _ = matches!(fix, FixAction::ChangeSetting { key, value } if key == "c");
}
#[cfg(test)]
mod tests {
    fn t() { let _ = FixAction::ChangeSetting { key: "t".to_owned(), value: String::new() }; }
}
#[cfg(all(test, target_os = "linux"))]
mod linux_tests;
"#;
    let code = masked(text);
    let (regions, files) = test_regions(Path::new("/x/src/lib.rs"), text, &code);
    assert_eq!(regions.len(), 2);
    assert_eq!(
        files,
        vec![
            PathBuf::from("/x/src/linux_tests.rs"),
            PathBuf::from("/x/src/linux_tests")
        ]
    );
    let dir = std::env::temp_dir().join(format!("hum507-scan-{}", std::process::id()));
    let src = dir.join("crates/x/src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("lib.rs"), text).unwrap();
    let found = constructions(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(found.len(), 1, "{found:#?}");
    let (key, value) = fields(&found[0].body).unwrap();
    assert_eq!(literal(&key).as_deref(), Some("a.b"));
    assert_eq!(literal(&value).as_deref(), Some("x{"));
    assert_eq!(literal("format!(\"x\")"), None);
}

/// Eine Stelle der App, deren Schlüssel oder Wert kein Literal ist.
struct DartSite {
    /// Die Datei, relativ zu `app/lib/`.
    file: &'static str,
    /// Die Argumente, Leerraum zu je einem Leerzeichen gefaltet.
    body: &'static str,
    /// Warum die Stelle keinen eigenen Wert erfindet.
    why: &'static str,
}

/// Die Stellen der App, die einen `ChangeSetting` nur weitergeben.
const DART_SITES: &[DartSite] = &[
    DartSite {
        file: "core/ipc/convert.dart",
        body: "key: changeSetting.key, value: changeSetting.value,",
        why: "Übersetzung aus dem Protokoll",
    },
    DartSite {
        file: "core/text/sanitize_diagnostic.dart",
        body: "key: sanitizeBodyText(key), value: sanitizeBodyText(value),",
        why: "bereinigt den Text eines Befunds des Daemons",
    },
];

/// Jeder `FixAction.changeSetting`, den die App selbst baut, trägt einen
/// Wert, den `humanitl config set` annimmt (HUM-507). Ein Feld, dessen Wert
/// der Mensch wählt (der Projektordner von `client_diagnostics.dart`), ist
/// `FixAction.editSetting`.
///
/// Mutationsbeweis: `FixAction.changeSetting(key: FormKeys.sandboxWorkDir,
/// value: '')` zurück in `client_diagnostics.dart` macht den Test rot.
#[test]
fn every_change_setting_the_app_builds_is_a_value_config_set_takes() {
    let app_lib = daemon_root().join("../app/lib");
    let constants = dart_string_constants(
        &std::fs::read_to_string(app_lib.join("core/domain/settings.dart")).unwrap(),
    );
    let resolve = |expr: &str| {
        dart_literal(expr).or_else(|| {
            expr.strip_prefix("FormKeys.")
                .and_then(|name| constants.get(name).cloned())
        })
    };
    let found = dart_constructions(&app_lib);
    assert!(
        found.len() >= 2,
        "the scan finds the app's sites: {found:#?}"
    );
    let mut problems = String::new();
    let mut used = vec![0usize; DART_SITES.len()];
    for site in &found {
        if let Some(index) = DART_SITES
            .iter()
            .position(|known| known.file == site.file && known.body == site.body)
        {
            used[index] += 1;
            continue;
        }
        let resolved = fields(&site.body)
            .and_then(|(key, value)| Some((resolve(&key)?, dart_literal(&value)?)));
        match resolved {
            Some((key, value)) => {
                if let Some(refused) = refusal(&key, &value) {
                    let _ = writeln!(problems, "app/lib/{}:{}: {refused}", site.file, site.line);
                }
            }
            None => {
                let _ = writeln!(
                    problems,
                    "app/lib/{}:{}: a changeSetting whose value is not a literal; register it in \
                     DART_SITES or use FixAction.editSetting for a field the person fills:\n    \
                     body: {:?}",
                    site.file, site.line, site.body
                );
            }
        }
    }
    for (site, count) in DART_SITES.iter().zip(&used) {
        assert!(!site.why.is_empty());
        if *count == 0 {
            let _ = writeln!(
                problems,
                "app/lib/{}: stale entry: {}",
                site.file, site.body
            );
        }
    }
    assert!(problems.is_empty(), "{problems}");
}

/// Der Scanner der App: Kommentare und Zeichenketten zählen nicht, eine
/// Konstante von `FormKeys` wird gelesen.
#[test]
fn the_dart_scanner_skips_comments_and_reads_constants() {
    let text = "// FixAction.changeSetting(key: 'x', value: '')\n\
                final a = FixAction.changeSetting(key: 'a.b', value: 'it(s)');\n\
                final b = 'FixAction.changeSetting(';\n";
    let code = dart_masked(text);
    assert!(!String::from_utf8_lossy(&code).contains("x'"));
    let dir = std::env::temp_dir().join(format!("hum507-dart-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.dart"), text).unwrap();
    let found = dart_constructions(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(found.len(), 1, "{found:#?}");
    let (key, value) = fields(&found[0].body).unwrap();
    assert_eq!(dart_literal(&key).as_deref(), Some("a.b"));
    assert_eq!(dart_literal(&value).as_deref(), Some("it(s)"));
    let constants =
        dart_string_constants("  static const String sandboxWorkDir = 'sandbox.work_dir';");
    assert_eq!(
        constants.get("sandboxWorkDir").map(String::as_str),
        Some("sandbox.work_dir")
    );
}
