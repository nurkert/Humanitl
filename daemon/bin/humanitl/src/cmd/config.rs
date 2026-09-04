//! `humanitl config get|schema`: die aufgelöste Konfiguration und ihr Schema.
//!
//! Aufgelöst wird in `humanitl-config`, nicht hier: die sieben Ebenen, die
//! Aliasse, die Wertebereiche und die Herkunft je Feld gehören dorthin
//! (ADR-011). Dieses Modul sucht den Pfad im Ergebnis und schreibt ihn auf.

use std::path::Path;

use humanitl_config::{ProfileSource, Resolved, alias, schema};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::{Value, json};

use crate::cli::{ConfigCmd, flag_name};
use crate::cmd::{Context, EXIT_OK, Failure};

/// Wie viele Vorschläge ein unbekannter Schlüssel höchstens bekommt.
const SUGGESTIONS: usize = 5;

/// Führt `humanitl config <cmd>` aus.
///
/// # Errors
///
/// `CONFIG_001` bis `CONFIG_003`, wenn die Konfiguration nicht lädt, und
/// `CONFIG_002`, wenn `get` einen Schlüssel nennt, den das Schema nicht kennt.
pub fn run(ctx: &Context, cmd: &ConfigCmd) -> Result<u8, Failure> {
    match cmd {
        ConfigCmd::Get { key } => get(ctx, key),
        ConfigCmd::Schema { profiles } => {
            if *profiles {
                profiles_out(ctx);
            } else {
                schema_out(ctx);
            }
            Ok(EXIT_OK)
        }
    }
}

/// `config schema --profiles`: was `--profile` wählen kann.
///
/// Die mitgelieferten Profile und alles, was als `*.toml` im Profilverzeichnis
/// liegt, jeweils mit Beschreibung und Herkunft. Ein Profil, das sich nicht
/// lesen lässt, erscheint als Befund und nicht als Lücke.
fn profiles_out(ctx: &Context) {
    let (summaries, diagnostics) = humanitl_config::available_profiles(&ctx.paths);
    for diagnostic in &diagnostics {
        ctx.render
            .note(&crate::render::diagnostic_block(diagnostic));
    }

    if ctx.render.is_json() {
        let rows: Vec<Value> = summaries
            .iter()
            .map(|summary| {
                json!({
                    "name": summary.name,
                    "description": summary.description,
                    "source": source_label(&summary.source),
                    "broken": summary.broken,
                })
            })
            .collect();
        ctx.render.value(&json!({ "profiles": rows }));
        return;
    }

    let home = ctx.paths.home();
    let rows: Vec<Vec<String>> = summaries
        .iter()
        .map(|summary| {
            let from = shorten(&source_label(&summary.source), &home);
            vec![
                summary.name.clone(),
                if summary.broken {
                    format!("{from} (does not load)")
                } else {
                    from
                },
                summary
                    .description
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
            ]
        })
        .collect();
    ctx.render.line(&crate::render::table(
        &["NAME", "FROM", "DESCRIPTION"],
        &rows,
    ));
}

/// Woher ein Profil kommt, als eine Spalte.
fn source_label(source: &ProfileSource) -> String {
    match source {
        ProfileSource::Builtin(_) => "bundled".to_owned(),
        ProfileSource::File(path) | ProfileSource::Project(path) => path.display().to_string(),
    }
}

/// Ein Pfad im Heimatverzeichnis, mit `~` statt seines Anfangs.
///
/// Nur für die Tabelle: Der volle Pfad ist dort dreimal so breit wie der Name
/// und die Beschreibung zusammen, und `--json` trägt ihn ungekürzt.
fn shorten(text: &str, home: &Path) -> String {
    let home = home.display().to_string();
    if home.is_empty() {
        return text.to_owned();
    }
    text.strip_prefix(&home)
        .map_or_else(|| text.to_owned(), |rest| format!("~{rest}"))
}

/// `config get KEY`.
fn get(ctx: &Context, key: &str) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    let path = canonical_path(ctx, key)?;
    let value = value_at(&resolved, &path)?;
    let origin = resolved.origin(&path).map(ToString::to_string);

    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "key": path,
            "value": value,
            "origin": origin,
        }));
        return Ok(EXIT_OK);
    }

    ctx.render.line(&scalar(&value));
    if let Some(origin) = origin {
        // Die Herkunft steht auf `stderr` und nicht hinter `-v`: Wer den Wert
        // liest, will wissen, welche Ebene ihn gesetzt hat — sonst ist eine
        // überraschende Sandbox nicht zu erklären (HUM-066). In eine Pipe
        // gerät sie trotzdem nicht; dort steht nur der Wert.
        ctx.render.note(&format!(
            "{path} comes from {origin}; {} sets it for one run",
            flag_for(&path)
        ));
    }
    Ok(EXIT_OK)
}

/// `config schema`.
fn schema_out(ctx: &Context) {
    let value = humanitl_config::json_schema();
    if ctx.render.is_json() {
        ctx.render.value(&value);
    } else {
        ctx.render
            .line(&serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()));
    }
}

/// Der heutige Pfad zu einem Schlüssel, oder ein Befund mit Vorschlägen.
///
/// Ein alter Name funktioniert weiter und wird gemeldet, wie beim Laden
/// (`CONFIG_005`, Stufe `info`).
fn canonical_path(ctx: &Context, key: &str) -> Result<String, Failure> {
    let known = schema::known_paths();
    if known.contains(key) {
        return Ok(key.to_owned());
    }
    if let Some(entry) = alias::lookup(key) {
        ctx.render.note(&crate::render::diagnostic_block(
            &Diagnostic::builder(codes::CONFIG_005, Severity::Info)
                .why(format!(
                    "{} is the old name of {} (renamed in {})",
                    entry.old, entry.canonical, entry.since
                ))
                .fix(FixAction::ChangeSetting {
                    key: entry.canonical.to_owned(),
                    value: String::new(),
                })
                .build(),
        ));
        return Ok(entry.canonical.to_owned());
    }
    Err(Failure::new(unknown_key(key)))
}

/// Der Befund für einen Schlüssel, den das Schema nicht kennt.
fn unknown_key(key: &str) -> Diagnostic {
    let mut builder = Diagnostic::builder(codes::CONFIG_002, Severity::Error).why(format!(
        "{key} is not a configuration key; humanitl config schema lists every one"
    ));
    if let Some(near) = suggestions(key).first() {
        builder = builder.fix(FixAction::CopyCommand(format!(
            "humanitl config get {near}"
        )));
    }
    builder.build()
}

/// Bekannte Schlüssel, die dem gesuchten ähneln, in Pfad-Reihenfolge.
///
/// Kein Abstandsmaß, nur ein gemeinsamer Anfang oder ein gemeinsames Wort:
/// das findet `hold.timeout` für `hold.timeout_secs` und `ui.theme` für
/// `theme`, und mehr braucht ein Vorschlag nicht.
fn suggestions(key: &str) -> Vec<&'static str> {
    let needle = key.to_ascii_lowercase();
    let last = needle.rsplit('.').next().unwrap_or(&needle).to_owned();
    schema::leaf_paths()
        .into_iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.starts_with(&needle) || needle.starts_with(&lower) || lower.contains(&last)
        })
        .take(SUGGESTIONS)
        .collect()
}

/// Der Wert eines Pfades in der aufgelösten Konfiguration.
fn value_at(resolved: &Resolved, path: &str) -> Result<Value, Failure> {
    let mut cursor = serde_json::to_value(&resolved.config).map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::CONFIG_001, Severity::Error)
                .why(format!("the resolved configuration is not JSON: {error}"))
                .build(),
        )
    })?;
    for segment in path.split('.') {
        let next = cursor.get(segment).cloned();
        match next {
            Some(value) => cursor = value,
            // Das Schema kennt den Pfad, die Struktur nicht: das ist kein
            // Tippfehler des Aufrufers, sondern ein Feld, das `serde` anders
            // schreibt als `schemars`. Der Befund nennt beide Namen.
            None => {
                return Err(Failure::new(
                    Diagnostic::builder(codes::CONFIG_002, Severity::Error)
                        .why(format!(
                            "the schema knows {path}, but the resolved configuration has no {segment}"
                        ))
                        .build(),
                ));
            }
        }
    }
    Ok(cursor)
}

/// Ein Wert als eine Zeile: Text ohne Anführungszeichen, alles andere als JSON.
///
/// `humanitl config get hold.timeout_secs` soll `300` sagen und nicht `"300"`,
/// damit `$(humanitl config get …)` in einem Skript den Wert trägt.
fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Der Name des Flags zu einem Schlüssel, für Meldungen.
fn flag_for(path: &str) -> String {
    format!("--{}", flag_name(path))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{Env, Sources};
    use serde_json::json;

    use super::{flag_for, scalar, suggestions, value_at};

    fn resolved(pairs: [(&str, &str); 1]) -> humanitl_config::Resolved {
        let sources = Sources::empty().with_env(Env::from_pairs(pairs));
        humanitl_config::load(&sources).expect("the defaults load")
    }

    #[test]
    fn a_leaf_is_read_from_the_resolved_config() {
        let resolved = resolved([("HUMANITL_HOLD__TIMEOUT_SECS", "7")]);
        assert_eq!(
            value_at(&resolved, "hold.timeout_secs").expect("the key exists"),
            json!(7)
        );
        assert_eq!(
            scalar(&value_at(&resolved, "hold.timeout_secs").expect("the key exists")),
            "7"
        );
    }

    #[test]
    fn a_group_comes_back_whole() {
        let resolved = resolved([("HUMANITL_UI__THEME", "light")]);
        let group = value_at(&resolved, "ui").expect("the group exists");
        assert_eq!(group["theme"], json!("light"));
    }

    #[test]
    fn a_string_loses_its_quotes_but_a_list_stays_json() {
        assert_eq!(scalar(&json!("default")), "default");
        assert_eq!(scalar(&json!(["/v1/", "/api/"])), "[\"/v1/\",\"/api/\"]");
        assert_eq!(scalar(&json!(null)), "-");
    }

    #[test]
    fn a_typo_gets_a_suggestion_from_the_schema() {
        let near = suggestions("hold.timeout");
        assert!(
            near.contains(&"hold.timeout_secs"),
            "expected hold.timeout_secs among {near:?}"
        );
        assert!(suggestions("hold.timeout").len() <= 5);
        assert_eq!(flag_for("hold.timeout_secs"), "--hold-timeout-secs");
    }
}
