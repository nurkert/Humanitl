//! `humanitl config get|set|schema|edit`: die Konfiguration und ihr Schema.
//!
//! Aufgelöst wird in `humanitl-config`, nicht hier: die sieben Ebenen, die
//! Aliasse, die Wertebereiche und die Herkunft je Feld gehören dorthin
//! (ADR-011). Dieses Modul sucht den Pfad im Ergebnis und schreibt ihn auf.
//!
//! Geschrieben wird mit [`humanitl_config::edit::set_value`], dem einen Weg in
//! `config.toml`, den auch der Daemon für `SetConfig` nimmt: nur der eine Wert
//! ändert sich, Kommentare, Verweise, Byte-Reihenfolge und Zeilenenden bleiben,
//! nie eine halbe Datei, und kein zweiter Schreiber verliert seinen Wert. Dazu
//! kommt hier eine Zusage: **Geprüft, bevor es in der Datei steht.** Der Wert
//! wird nach dem Typ des Schemas gelesen, gegen Aufzählung, Mindest- und
//! Höchstwert geprüft, und die fertige Nebendatei wird mit allen übrigen Ebenen
//! geladen, bevor sie die Datei ersetzt ([`probe`]). `CONFIG_003` mit Grund,
//! statt eine Datei zu hinterlassen, die der nächste Start ablehnt.

use std::path::{Path, PathBuf};

use humanitl_config::{ProfileSource, ProjectScope, Resolved, alias, schema};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::{Value, json};

use crate::cli::{ConfigCmd, flag_name};
use crate::cmd::{Context, EXIT_OK, Failure};

/// Wie viele Vorschläge ein unbekannter Schlüssel höchstens bekommt.
const SUGGESTIONS: usize = 5;

/// Ab wie vielen Zeichen das letzte Wort eines Schlüssels auch im Inneren
/// anderer Schlüssel gesucht wird.
const MIN_INNER: usize = 3;

/// Der Block, unter dem ein Profil seine Konfigurationswerte trägt.
const PROFILE_SECTION: &str = humanitl_config::PROFILE_SECTION;

/// Die Editoren, die `config edit` versucht, wenn weder `$VISUAL` noch
/// `$EDITOR` dasteht.
const FALLBACK_EDITORS: [&str; 2] = ["nano", "vi"];

/// Führt `humanitl config <cmd>` aus.
///
/// # Errors
///
/// `CONFIG_001` bis `CONFIG_003`, wenn die Konfiguration nicht lädt, nicht
/// geschrieben werden kann oder einen Wert bekommen soll, den das Schema nicht
/// zulässt, und `CONFIG_002`, wenn ein Schlüssel genannt ist, den das Schema
/// nicht kennt.
pub async fn run(ctx: &Context, cmd: &ConfigCmd) -> Result<u8, Failure> {
    match cmd {
        ConfigCmd::Get { key, origin } => get(ctx, key.as_deref(), *origin),
        ConfigCmd::Set {
            key,
            value,
            project,
        } => set(ctx, key, value, *project).await,
        ConfigCmd::Schema { profiles } => {
            if *profiles {
                profiles_out(ctx);
            } else {
                schema_out(ctx);
            }
            Ok(EXIT_OK)
        }
        ConfigCmd::Edit => edit(ctx),
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

/// `config get [KEY] [--origin]`.
fn get(ctx: &Context, key: Option<&str>, origin: bool) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    match key {
        Some(key) => one(ctx, &resolved, key),
        None => all(ctx, &resolved, origin),
    }
}

/// `config get KEY`: ein Wert, und auf `stderr` die Ebene, die ihn gesetzt hat.
fn one(ctx: &Context, resolved: &Resolved, key: &str) -> Result<u8, Failure> {
    let path = canonical_path(ctx, key)?;
    let value = value_at(resolved, &path)?;
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

/// `config get` ohne Schlüssel: jedes Blattfeld als `key = value`.
///
/// Ohne `--origin` bleibt die Herkunft weg, damit die Tabelle in ein Terminal
/// passt; mit `--origin` kommt sie als dritte Spalte dazu. Im JSON-Modus steht
/// sie immer, denn dort kostet eine Spalte keine Breite.
fn all(ctx: &Context, resolved: &Resolved, origin: bool) -> Result<u8, Failure> {
    let mut rows: Vec<(String, Value, Option<String>)> = Vec::new();
    for path in schema::leaf_paths() {
        let value = value_at(resolved, path)?;
        rows.push((
            path.to_owned(),
            value,
            resolved.origin(path).map(ToString::to_string),
        ));
    }

    if ctx.render.is_json() {
        let values: Vec<Value> = rows
            .iter()
            .map(|(key, value, from)| json!({ "key": key, "value": value, "origin": from }))
            .collect();
        ctx.render.value(&json!({ "values": values }));
        return Ok(EXIT_OK);
    }

    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|(key, value, from)| {
            let mut row = vec![key.clone(), scalar(value)];
            if origin {
                row.push(from.clone().unwrap_or_else(|| "-".to_owned()));
            }
            row
        })
        .collect();
    let headers: &[&str] = if origin {
        &["KEY", "VALUE", "ORIGIN"]
    } else {
        &["KEY", "VALUE"]
    };
    ctx.render
        .line(crate::render::table(headers, &table).trim_end());
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
                .fix(FixAction::CopyCommand(format!(
                    "humanitl config get {}",
                    entry.canonical
                )))
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
    // Ohne Vorschlag bleibt die Liste aller Schlüssel: Sie ist ein Befehl, der
    // immer gelingt.
    let fix = suggestions(key).first().map_or_else(
        || "humanitl config schema".to_owned(),
        |near| format!("humanitl config get {near}"),
    );
    builder = builder.fix(FixAction::CopyCommand(fix));
    builder.build()
}

/// Bekannte Schlüssel, die dem gesuchten ähneln, in Pfad-Reihenfolge.
///
/// Kein Abstandsmaß, nur ein gemeinsamer Anfang oder ein gemeinsames Wort:
/// das findet `hold.timeout` für `hold.timeout_secs` und `ui.theme` für
/// `theme`, und mehr braucht ein Vorschlag nicht.
///
/// Ein Punkt am Ende zählt nicht (`hold.` sucht wie `hold`), und ein letztes
/// Wort unter drei Zeichen sucht nicht im Inneren: `a` steckt in fast jedem
/// Schlüssel, und ein Vorschlag, der auf alles passt, sagt nichts.
fn suggestions(key: &str) -> Vec<&'static str> {
    let needle = key.trim_end_matches('.').to_ascii_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let last = needle.rsplit('.').next().unwrap_or(&needle).to_owned();
    let inner = last.chars().count() >= MIN_INNER;
    let leaves = schema::leaf_paths();
    // Erst, was so anfängt oder darüber liegt, dann, was das Wort nur enthält.
    let prefix = leaves.iter().copied().filter(|path| {
        let lower = path.to_ascii_lowercase();
        lower.starts_with(&needle) || needle.starts_with(&format!("{lower}."))
    });
    let contains = leaves.iter().copied().filter(|path| {
        let lower = path.to_ascii_lowercase();
        inner && !lower.starts_with(&needle) && lower.contains(&last)
    });
    prefix.chain(contains).take(SUGGESTIONS).collect()
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

/// `config set KEY VALUE [--project]`.
///
/// Der Wert wird nach dem Typ des Schemas gelesen und gegen die Aufzählung,
/// den Mindest- und den Höchstwert geprüft. Danach schreibt
/// [`humanitl_config::edit::set_value`] ihn, und zwar erst in eine Nebendatei:
/// Die wird so geladen, wie der nächste Start sie lädt — mit den Profilen, der
/// Umgebung und den übrigen Werten der wirklichen Datei ([`probe`]) —, und nur
/// wenn das gelingt, ersetzt sie `config.toml`. Ein Wert, der für sich richtig
/// ist, aber mit einem anderen Wert der Datei nicht zusammenpasst, landet so
/// nie in der Datei.
///
/// Danach steht auf `stdout` die Zeile `key = value (global)`; ob ein Daemon
/// läuft, steht auf `stderr`, denn das entscheidet nur, wann der neue Wert
/// wirkt, nicht ob er dasteht.
async fn set(ctx: &Context, key: &str, text: &str, project: bool) -> Result<u8, Failure> {
    let path = canonical_path(ctx, key)?;
    let field = schema::fields()
        .iter()
        .find(|field| field.path == path)
        .ok_or_else(|| Failure::new(unknown_key(&path)))?;
    // Die Vertrauensgrenze zuerst: Wer einen gesperrten Schlüssel ins
    // Projekt schreiben will, soll die Grenze erfahren und nicht, dass sein
    // Wert falsch geschrieben ist.
    if project && field.project_scope == ProjectScope::Denied {
        return Err(Failure::new(project_denied(&path)));
    }
    if field.group {
        return Err(Failure::new(group_key(&path)));
    }
    let value = parse_value(field, text).map_err(Failure::new)?;
    check_value(field, &value).map_err(Failure::new)?;
    let toml_value = to_toml(field, &value).map_err(Failure::new)?;

    let (file, prefix, scope) = if project {
        (
            ctx.paths.project_profile_path(&ctx.cwd),
            vec![PROFILE_SECTION.to_owned()],
            Scope::Project,
        )
    } else {
        (
            ctx.config_file
                .clone()
                .unwrap_or_else(|| ctx.paths.config_path()),
            Vec::new(),
            Scope::Global,
        )
    };
    let mut segments = prefix;
    segments.extend(path.split('.').map(ToOwned::to_owned));

    let written =
        humanitl_config::edit::set_value(&file, &segments, toml_value.as_ref(), |candidate| {
            probe(ctx, field, scope, &file, candidate)
        })
        .map_err(Failure::new)?;

    let changed = written == humanitl_config::edit::Written::Changed;
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "key": path,
            "value": value,
            "scope": scope.as_str(),
            "file": file.display().to_string(),
            "written": if changed { "changed" } else { "unchanged" },
        }));
        return Ok(EXIT_OK);
    }
    let shown = if value.is_null() {
        "- (removed, the default applies)".to_owned()
    } else {
        scalar(&value)
    };
    ctx.render
        .line(&format!("{path} = {shown} ({})", scope.as_str()));
    ctx.render.note(&reach(ctx, &file).await);
    Ok(EXIT_OK)
}

/// Wohin `config set` schreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// `config.toml` des Menschen.
    Global,
    /// `<projekt>/.humanitl/profile.toml`, unter `[config]`.
    Project,
}

impl Scope {
    /// Das Wort für die Ausgabe.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

/// Lädt die Konfiguration so, wie der nächste Start sie lädt — mit der
/// Nebendatei an der Stelle der wirklichen — und entscheidet, ob der Wert
/// geschrieben werden darf.
///
/// Die Quellen werden dafür **mit** der Nebendatei neu bestimmt und nicht an
/// einem vorab bestimmten Satz ausgetauscht: Welches Projekt-Profil gilt,
/// hängt an `sandbox.work_dir`, und `config set sandbox.work_dir` ändert
/// genau das.
///
/// Geschrieben wird, wenn die Konfiguration mit dem neuen Wert lädt, **oder**
/// wenn jeder Befund nachher genau so schon vorher dastand: derselbe Code am
/// selben Schlüssel mit demselben Text, und keiner nennt den gesetzten
/// Schlüssel. Ein Wert, der selbst falsch ist, wird nicht geschrieben, auch
/// wenn derselbe falsche Wert schon dastand. Ein Befund, der sich durch den neuen
/// Wert nur im Text ändert — ein Paar wie `limits.hold_max_bytes` und
/// `limits.hold_body_cap_bytes`, dessen Grenze der neue Wert verschiebt —, ist
/// ein neuer Befund. Ein Befund ohne Schlüssel verhindert das Schreiben immer.
/// Die Ladung meldet nur ihren ersten Befund; alle Befunde einer Ebenenfolge
/// sammelt deshalb [`findings`]. So verdeckt ein älterer Fehler keinen neuen,
/// und eine Konfiguration mit zwei falschen Werten lässt sich trotzdem
/// Schlüssel für Schlüssel reparieren.
fn probe(
    ctx: &Context,
    field: &schema::Field,
    scope: Scope,
    file: &Path,
    candidate: &Path,
) -> Result<(), Diagnostic> {
    let candidate_text = std::fs::read_to_string(candidate).map_err(|error| {
        Diagnostic::builder(codes::CONFIG_015, Severity::Error)
            .why(format!(
                "{} could not be read back for the check: {error}",
                candidate.display()
            ))
            .build()
    })?;
    let after = findings(ctx, scope, Some(&candidate_text), file);
    if after.is_empty() {
        return Ok(());
    }
    let current_text = std::fs::read_to_string(file).ok();
    let before = findings(ctx, scope, current_text.as_deref(), file);
    let new = after.iter().find(|finding| {
        finding.key.is_none()
            || finding.involved.contains(&field.path)
            || !before.iter().any(|old| old.same_as(finding))
    });
    match new {
        None => {
            // Alles, was jetzt noch scheitert, scheiterte schon vorher genau
            // so; der Wert bringt nichts Neues mit.
            if let Some(first) = before.first() {
                ctx.render.note(&format!(
                    "the configuration still does not load ({}); {} is written because it adds \
                     no finding of its own",
                    crate::render::plain(&first.diagnostic.why),
                    field.path
                ));
            }
            Ok(())
        }
        Some(finding) => Err(refusal(field, finding, !before.is_empty(), file, candidate)),
    }
}

/// Der Befund, mit dem `config set` ablehnt.
fn refusal(
    field: &schema::Field,
    finding: &Finding,
    broken_before: bool,
    file: &Path,
    candidate: &Path,
) -> Diagnostic {
    // Die Ladung nennt die Nebendatei; der Mensch kennt nur seine Datei.
    let why = finding.diagnostic.why.replace(
        &candidate.display().to_string(),
        &file.display().to_string(),
    );
    let names_the_key = finding.involved.contains(&field.path);
    // Nennt der Befund den gesetzten Schlüssel, gilt sein eigener Vorschlag;
    // sonst ist die Datei als Ganzes zu richten. Lud die Konfiguration schon
    // vorher nicht, sagt der Text das in jedem Fall.
    let fix = if finding.key.is_none() {
        // Ein Befund ohne Schlüssel kann aus einer Ebene kommen, die der
        // Mensch hier nicht schreibt, etwa aus dem Projekt-Profil, das
        // `sandbox.work_dir` gerade wählt. Dessen Vorschlag trüge einen Wert
        // aus dem Projekt in die globale Datei; die Grenze zwischen beiden
        // bleibt, und der Vorschlag ist die Datei als Ganzes.
        Some(FixAction::CopyCommand("humanitl config edit".to_owned()))
    } else if names_the_key {
        finding
            .diagnostic
            .fix
            .clone()
            .or_else(|| default_fix(field))
    } else if broken_before {
        Some(FixAction::CopyCommand("humanitl config edit".to_owned()))
    } else {
        finding.diagnostic.fix.clone()
    };
    let why = if broken_before {
        format!(
            "{why}; the configuration did not load before this change either, and this \
             finding is new or cannot be told apart from the older ones"
        )
    } else {
        why
    };
    let mut builder =
        Diagnostic::builder(finding.diagnostic.code, finding.diagnostic.severity).why(why);
    if let Some(fix) = fix {
        builder = builder.fix(fix);
    }
    builder.build()
}

/// Ein Befund der Ladung und die Schlüssel, die er nennt.
#[derive(Debug)]
struct Finding {
    /// Der Befund, wie die Ladung ihn meldet.
    diagnostic: Diagnostic,
    /// Der Schlüssel, um den es geht: der erste, den der Text nennt. `None`,
    /// wenn der Text keinen nennt oder der Befund keinen haben kann
    /// (`CONFIG_002`, ein Schlüssel, den das Schema nicht kennt).
    key: Option<String>,
    /// Alle Schlüssel, die der Text nennt, in der Reihenfolge ihres
    /// Vorkommens; bei einem Befund über ein Paar beide.
    involved: Vec<String>,
}

impl Finding {
    /// Derselbe Befund: derselbe Code am selben Schlüssel mit demselben Text.
    ///
    /// Der Text zählt mit: Ein Befund über ein Paar behält seinen Schlüssel,
    /// wenn der neue Wert die Grenze des anderen verschiebt, aber nicht seinen
    /// Text. Der Pfad der Nebendatei wird dafür vorher ersetzt
    /// ([`findings`]).
    fn same_as(&self, other: &Self) -> bool {
        self.key.is_some()
            && self.key == other.key
            && self.diagnostic.code == other.diagnostic.code
            && self.diagnostic.why == other.diagnostic.why
    }
}

/// Wie oft [`findings`] höchstens einen Schlüssel herausnimmt und neu lädt.
const MAX_FINDINGS: usize = 16;

/// Alle Befunde einer Ebenenfolge, in der Reihenfolge, in der die Ladung sie
/// meldet.
///
/// `file_text` ist der Text der Datei, die `config set` schreibt (oder `None`,
/// wenn es sie nicht gibt); die übrigen Ebenen kommen von der Platte und aus
/// der Umgebung. Nach jedem Befund wird sein Schlüssel — samt seinen alten
/// Namen — aus der globalen Datei, aus dem Projekt-Profil und aus der Umgebung
/// genommen und neu geladen, bis die Ladung gelingt. Ein Befund ohne
/// Schlüssel, einer, der sich unverändert wiederholt (der Fehler steckt in
/// einer Ebene, die hier nicht verändert wird), und das Ende der Runden
/// beenden das Zählen mit einem Befund ohne Schlüssel: Was danach käme, ist
/// nicht gezählt, und ein unvollständiges Bild darf nie zum Schreiben führen.
fn findings(ctx: &Context, scope: Scope, file_text: Option<&str>, file: &Path) -> Vec<Finding> {
    let Ok(scratch) = tempfile::tempdir() else {
        return vec![unknown_finding(
            file,
            "no temporary directory for the check",
        )];
    };
    findings_in(ctx, scope, file_text, file, scratch.path(), |path, text| {
        std::fs::write(path, text)
    })
}

/// Schreibt eine Nebendatei; im Betrieb `std::fs::write`, im Test einer, der
/// scheitert.
type ScratchWriter = fn(&Path, &str) -> std::io::Result<()>;

/// [`findings`] mit den Nebendateien in `scratch`, geschrieben von `write`.
fn findings_in(
    ctx: &Context,
    scope: Scope,
    file_text: Option<&str>,
    file: &Path,
    scratch: &Path,
    write: ScratchWriter,
) -> Vec<Finding> {
    let mut layers = Layers::new(ctx, scope, file_text, file, write);
    let mut out: Vec<Finding> = Vec::new();

    for round in 0..MAX_FINDINGS {
        let Err(diagnostic) = layers.load(ctx, scratch, round) else {
            return out;
        };
        let (key, involved) = keys_named_in(&diagnostic);
        let repeated = out.last().is_some_and(|last| {
            last.diagnostic.code == diagnostic.code
                && last.key == key
                && last.diagnostic.why == diagnostic.why
        });
        if repeated {
            out.push(unknown_finding(
                file,
                "a finding repeats after its key was taken out of every layer, so the rest \
                 cannot be counted",
            ));
            return out;
        }
        let peel = key.clone();
        out.push(Finding {
            diagnostic,
            key,
            involved,
        });
        let Some(peel) = peel else {
            return out;
        };
        layers.peel(&peel);
    }
    out.push(unknown_finding(
        file,
        "the check stopped after its last round, so not every finding is counted",
    ));
    out
}

/// Die Ebenen, die [`findings`] Runde um Runde lädt und aus denen es
/// Schlüssel herausnimmt.
struct Layers {
    /// Der Text der globalen Datei, `None`, wenn es keine gibt.
    global: Option<String>,
    /// Die globale Datei, für die die Nebendatei im Text eines Befunds steht.
    global_real: PathBuf,
    /// Der Text des Projekt-Profils, sobald er festgehalten ist.
    project: Option<String>,
    /// Das Projekt-Profil, für das die Nebendatei im Text steht.
    project_real: Option<PathBuf>,
    /// Ob das Projekt-Profil schon gelesen und festgehalten ist.
    project_pinned: bool,
    /// Die Umgebung ohne die schon herausgenommenen Variablen.
    env: humanitl_config::Env,
    /// Die schon herausgenommenen Schlüssel, in ihrer Reihenfolge.
    peeled: Vec<String>,
    /// Was die Nebendateien schreibt.
    write: ScratchWriter,
}

impl Layers {
    /// Die Ebenen am Anfang: die Datei, die `config set` schreibt, an ihrer
    /// Stelle, alles andere von der Platte und aus der Umgebung.
    fn new(
        ctx: &Context,
        scope: Scope,
        file_text: Option<&str>,
        file: &Path,
        write: ScratchWriter,
    ) -> Self {
        let (global, project, project_real) = match scope {
            Scope::Global => (file_text.map(ToOwned::to_owned), None, None),
            Scope::Project => (
                global_text(ctx),
                file_text.map(ToOwned::to_owned),
                Some(file.to_path_buf()),
            ),
        };
        Self {
            global,
            global_real: ctx
                .config_file
                .clone()
                .unwrap_or_else(|| ctx.paths.config_path()),
            project,
            project_real,
            project_pinned: scope == Scope::Project,
            env: ctx.env.clone(),
            peeled: Vec::new(),
            write,
        }
    }

    /// Lädt die Ebenen dieser Runde aus Nebendateien in `scratch`.
    ///
    /// Im Text eines Befunds steht danach die Datei, für die eine Nebendatei
    /// steht: Der Mensch kennt nur seine, und ein Vergleich mit der anderen
    /// Folge darf nicht an einem Namen einer Nebendatei scheitern.
    fn load(&mut self, ctx: &Context, scratch: &Path, round: usize) -> Result<(), Diagnostic> {
        let write = self.write;
        let global_path = self
            .global
            .as_ref()
            .map(|text| write_scratch(write, &scratch.join(format!("global-{round}.toml")), text))
            .transpose()?;
        let mut written: Vec<(PathBuf, PathBuf)> = global_path
            .iter()
            .map(|path| (path.clone(), self.global_real.clone()))
            .collect();
        let result = humanitl_config::sources_with_global(
            &ctx.selection(),
            Some(&ctx.cwd),
            &self.env,
            &[],
            global_path,
        )
        .and_then(|mut sources| {
            if !self.project_pinned {
                // Das Projekt-Profil der wirklichen Folge wird einmal gelesen
                // und danach wie die globale Datei behandelt; was schon
                // herausgenommen ist, fehlt auch darin.
                self.project_real.clone_from(&sources.profile_project);
                self.project = sources
                    .profile_project
                    .as_ref()
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .map(|text| {
                        self.peeled
                            .iter()
                            .fold(text, |text, key| without_in_profile(&text, key))
                    });
                self.project_pinned = true;
            }
            sources.profile_project = self
                .project
                .as_ref()
                .map(|text| {
                    write_scratch(write, &scratch.join(format!("project-{round}.toml")), text)
                })
                .transpose()?;
            if let (Some(temp), Some(real)) = (&sources.profile_project, &self.project_real) {
                written.push((temp.clone(), real.clone()));
            }
            humanitl_config::load(&sources).map(|_| ())
        });
        result.map_err(|mut diagnostic| {
            for (temp, real) in &written {
                diagnostic.why = diagnostic
                    .why
                    .replace(&temp.display().to_string(), &real.display().to_string());
            }
            diagnostic
        })
    }

    /// Nimmt einen Schlüssel aus allen Ebenen: aus der globalen Datei, aus dem
    /// Projekt-Profil und aus der Umgebung, jeweils samt seinen alten Namen.
    fn peel(&mut self, key: &str) {
        self.global = self
            .global
            .take()
            .map(|text| without_everywhere(&text, key, &[]));
        self.project = self
            .project
            .take()
            .map(|text| without_in_profile(&text, key));
        self.env = without_variable(&self.env, key);
        self.peeled.push(key.to_owned());
    }
}

/// Schreibt eine Nebendatei für [`Layers::load`].
///
/// Ein Schreibfehler ist ein Befund ohne Schlüssel und verhindert damit das
/// Schreiben. Still übergangen bliebe eine leere oder halbe Nebendatei zurück
/// (etwa bei vollem `TMPDIR`), die als Vorgabe lädt, und die Prüfung meldete
/// eine Konfiguration als gut, die sie nie gesehen hat. Den Vorschlag setzt
/// [`refusal`], wie für jeden Befund ohne Schlüssel.
fn write_scratch(write: ScratchWriter, path: &Path, text: &str) -> Result<PathBuf, Diagnostic> {
    write(path, text).map_err(|error| {
        Diagnostic::builder(codes::CONFIG_015, Severity::Error)
            .why(format!(
                "{}: the check could not write its scratch copy: {error}",
                path.display()
            ))
            .build()
    })?;
    Ok(path.to_path_buf())
}

/// Ein Befund, der keinen Schlüssel hat, weil die Prüfung selbst nicht zu
/// Ende kam. Er verhindert das Schreiben.
fn unknown_finding(file: &Path, why: &str) -> Finding {
    Finding {
        diagnostic: Diagnostic::builder(codes::CONFIG_015, Severity::Error)
            .why(format!("{}: {why}", file.display()))
            .build(),
        key: None,
        involved: Vec::new(),
    }
}

/// Der Text der globalen `config.toml`, falls es sie gibt.
fn global_text(ctx: &Context) -> Option<String> {
    let path = ctx
        .config_file
        .clone()
        .unwrap_or_else(|| ctx.paths.config_path());
    std::fs::read_to_string(path).ok()
}

/// Die Schlüssel, die ein Befund nennt: der früheste und alle.
///
/// Gelesen wird der Text, weil der Befund keinen Schlüssel als Feld trägt;
/// deshalb gelten drei Regeln, die ihn davor schützen, etwas Falsches zu
/// lesen. `CONFIG_002` ist ein Schlüssel, den das Schema nicht kennt: Der
/// Blattpfad in seinem Text ist ein Vorschlag (`did you mean …?`) und nicht der
/// Schlüssel, also hat er keinen. Ein alter Name zählt als sein heutiger. Bei
/// gleichem Anfang gewinnt der längere Pfad. Gesucht wird nur, wo die
/// bekannten Sätze der Ladung einen Schlüssel hinschreiben ([`key_regions`]);
/// ein Wert, der zufällig wie ein Schlüssel aussieht, nennt keinen.
fn keys_named_in(diagnostic: &Diagnostic) -> (Option<String>, Vec<String>) {
    if diagnostic.code == codes::CONFIG_002 {
        return (None, Vec::new());
    }
    let why = key_regions(&diagnostic.why).join("\n");
    let why = why.as_str();
    let mut hits: Vec<(usize, usize, &'static str)> = schema::leaf_paths()
        .into_iter()
        .flat_map(|path| {
            why.match_indices(path)
                .map(move |(at, _)| (at, path.len(), path))
        })
        .chain(alias::ALIASES.iter().flat_map(|entry| {
            why.match_indices(entry.old)
                .map(move |(at, _)| (at, entry.old.len(), entry.canonical))
        }))
        .collect();
    // Früher zuerst, bei gleichem Anfang der längere; ein Pfad, der nur der
    // Anfang eines längeren an derselben Stelle ist, zählt nicht.
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut involved: Vec<String> = Vec::new();
    let mut covered_until = 0;
    for (at, len, path) in hits {
        if at < covered_until {
            continue;
        }
        covered_until = at + len;
        if !involved.iter().any(|known| known == path) {
            involved.push(path.to_owned());
        }
    }
    (involved.first().cloned(), involved)
}

/// Die Teile eines Befundtextes, in denen ein Schlüssel stehen kann.
///
/// Zwei Sätze der Ladung tragen neben dem Schlüssel einen Wert aus der Datei
/// oder einen Pfad, und der kann aussehen wie ein Schlüssel
/// (`[ui] theme = "hold.timeout_secs"`):
///
/// - `<key> = <found> is out of range; allowed is <allowed> (default …)`
///   (`validate::out_of_range`): der Schlüssel vorn und die Grenze, die einen
///   zweiten nennen kann (`at least limits.hold_body_cap_bytes (…)`).
/// - `<key> (from <origin>) expects <expected>, found <found>`
///   (`load::bad_value`): nur der Schlüssel vorn.
///
/// Jeder andere Text zählt als Ganzes. Das liest im Zweifel einen Schlüssel
/// zu viel, und ein Schlüssel zu viel verhindert ein Schreiben, statt eines
/// zu erlauben.
fn key_regions(why: &str) -> Vec<&str> {
    const OUT_OF_RANGE: &str = " is out of range; allowed is ";
    if let Some((subject, rest)) = why.split_once(" = ")
        && let Some((_, allowed)) = rest.rsplit_once(OUT_OF_RANGE)
    {
        let allowed = allowed
            .rsplit_once(" (default ")
            .map_or(allowed, |(allowed, _)| allowed);
        return vec![subject, allowed];
    }
    if let Some((subject, rest)) = why.split_once(" (from ")
        && rest.contains(" expects ")
    {
        return vec![subject];
    }
    vec![why]
}

/// Der TOML-Text ohne den Schlüssel `key` und ohne jeden seiner alten Namen,
/// jeweils unter dem Präfix `prefix`.
fn without_everywhere(text: &str, key: &str, prefix: &[String]) -> String {
    std::iter::once(key)
        .chain(alias::old_names(key))
        .fold(text.to_owned(), |text, name| {
            let mut segments = prefix.to_vec();
            segments.extend(name.split('.').map(ToOwned::to_owned));
            without_key(&text, &segments)
        })
}

/// Das Projekt-Profil ohne den Schlüssel unter `[config]`.
fn without_in_profile(text: &str, key: &str) -> String {
    without_everywhere(text, key, &[PROFILE_SECTION.to_owned()])
}

/// Der TOML-Text ohne den Schlüssel unter `segments`; unverändert, wenn er
/// dort nicht steht oder der Text kein TOML ist.
fn without_key(text: &str, segments: &[String]) -> String {
    let Ok(mut document) = text.parse::<toml_edit::DocumentMut>() else {
        return text.to_owned();
    };
    let Some((last, groups)) = segments.split_last() else {
        return text.to_owned();
    };
    let mut cursor: &mut dyn toml_edit::TableLike = document.as_table_mut();
    for group in groups {
        match cursor
            .get_mut(group)
            .and_then(toml_edit::Item::as_table_like_mut)
        {
            Some(next) => cursor = next,
            None => return text.to_owned(),
        }
    }
    cursor.remove(last);
    document.to_string()
}

/// Die Umgebung ohne die Variablen, die diesen Schlüssel setzen, unter
/// seinem heutigen und unter jedem alten Namen.
fn without_variable(env: &humanitl_config::Env, key: &str) -> humanitl_config::Env {
    let head = format!("{}_", humanitl_config::DEFAULT_ENV_PREFIX);
    humanitl_config::Env::from_pairs(
        env.iter()
            .filter(|(name, _)| {
                let Some(rest) = name.strip_prefix(&head) else {
                    return true;
                };
                let path = rest
                    .to_lowercase()
                    .split(humanitl_config::ENV_SEPARATOR)
                    .collect::<Vec<_>>()
                    .join(".");
                let canonical = alias::canonical(&path).unwrap_or(path.as_str());
                canonical != key
            })
            .map(|(name, value)| (name.clone(), value.clone())),
    )
}

/// Der gelesene Wert als TOML-Wert; `None` heißt: den Schlüssel entfernen.
///
/// TOML kennt kein `null`. Ein Feld, das leer sein darf, ist leer, wenn es in
/// der Datei nicht steht — dann gilt der Vorgabewert.
fn to_toml(field: &schema::Field, value: &Value) -> Result<Option<toml::Value>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    toml::Value::try_from(value).map(Some).map_err(|error| {
        let mut builder = Diagnostic::builder(codes::CONFIG_003, Severity::Error).why(format!(
            "{} cannot be written to a TOML file: {error}",
            field.path
        ));
        if let Some(fix) = default_fix(field) {
            builder = builder.fix(fix);
        }
        builder.build()
    })
}

/// Wie lange [`reach`] auf die Antwort eines laufenden Daemons wartet.
const REACH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Der Satz, der sagt, wann der neue Wert wirkt.
///
/// Beide Fälle sind richtig, und genau deshalb muss die Ausgabe sagen, welcher
/// vorliegt: Wer den Daemon laufen hat, wartet auf den nächsten Start; wer
/// keinen hat, hat die Datei trotzdem geschrieben.
///
/// Gefragt wird ohne Weckruf (HUM-164): Ein Daemon, den erst diese Frage
/// hinter `humanitld.socket` startete, läse den neuen Wert schon, und der
/// Satz vom nächsten Start wäre falsch.
///
/// Erst ein beantwortetes `GetInfo` gilt als laufender Daemon, nicht schon die
/// Verbindung: Ein Token, das nach einem Absturz liegen blieb, weckt über den
/// Socket den Dienst, und der weist das alte Token ab. Die Frage hat eine
/// Frist ([`REACH_TIMEOUT`]); wer den Socket hält und schweigt, ist kein
/// laufender Daemon.
async fn reach(ctx: &Context, file: &Path) -> String {
    let answered = async {
        let mut client = ctx.connect_running().await.ok()?;
        client.get_info(()).await.ok()
    };
    if matches!(
        tokio::time::timeout(REACH_TIMEOUT, answered).await,
        Ok(Some(_))
    ) {
        format!(
            "{} carries the value; the running daemon takes it at its next start",
            file.display()
        )
    } else {
        format!(
            "{} carries the value; no daemon is running, so it reads it when it starts",
            file.display()
        )
    }
}

/// `CONFIG_003`: Das Projekt-Profil darf diesen Schlüssel nicht setzen.
///
/// Ohne Fix: Der Ausweg ist eine Entscheidung des Menschen — den Wert in die
/// eigene `config.toml` zu schreiben —, und ein vorgeschlagener Befehl
/// bräuchte einen Wert, den nur er kennt.
fn project_denied(path: &str) -> Diagnostic {
    Diagnostic::builder(codes::CONFIG_003, Severity::Error)
        .why(format!(
            "{path} may not come from the profile of a project: that file is part of a cloned \
             repository, and the trust boundary of backlog/CONVENTIONS.md 4.11 keeps this key out \
             of it; without --project the value goes into your own config.toml"
        ))
        .build()
}

/// `CONFIG_018`: Der Schlüssel ist eine Gruppe und kein Wert.
fn group_key(path: &str) -> Diagnostic {
    Diagnostic::builder(codes::CONFIG_018, Severity::Error)
        .why(format!(
            "{path} is a group of settings and not a value; name one of its keys"
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl config get {path}"
        )))
        .build()
}

/// Ein Vorschlag, der beweisbar gilt: der Vorgabewert des Feldes.
///
/// Der Vorgabewert besteht jede Prüfung — er ist der Wert, mit dem die
/// Konfiguration ohne Datei lädt —, und damit ist der Befehl einer, den man
/// abtippen kann, ohne dass er wieder abgelehnt wird. Ein Feld ohne
/// Vorgabewert bekommt keinen.
fn default_fix(field: &schema::Field) -> Option<FixAction> {
    let default = field.default.as_ref().filter(|value| !value.is_null())?;
    Some(FixAction::ChangeSetting {
        key: field.path.clone(),
        value: scalar(default),
    })
}

/// Liest den Wert der Kommandozeile nach dem Typ des Feldes.
///
/// Eine Zahl darf ihre Einheit tragen: `5m` für ein Feld auf `_secs`, `32MiB`
/// für eines auf `_bytes`. Beides ist die Schreibweise, in der ein Mensch über
/// diese Werte spricht, und beides steht am Ende als Zahl in der Datei — das
/// Schema kennt keine Einheiten.
///
/// Eine Aufzählung ist im Schema ein `oneOf` aus Konstanten und hat damit
/// keinen Typ `string`; sie wird als Text gelesen, und ob der Text einer der
/// erlaubten Werte ist, prüft [`check_value`].
fn parse_value(field: &schema::Field, text: &str) -> Result<Value, Diagnostic> {
    let types: Vec<&str> = field.types.iter().map(String::as_str).collect();
    let has = |kind: &str| types.contains(&kind);

    if has("null") && (text == "null" || text == "-") {
        return Ok(Value::Null);
    }
    if field.allowed.is_some() {
        return Ok(Value::String(text.to_owned()));
    }
    if has("boolean")
        && let Ok(flag) = text.parse::<bool>()
    {
        return Ok(Value::Bool(flag));
    }
    if has("integer") {
        return integer(field, text).map(Value::from);
    }
    if has("number")
        && let Ok(number) = text.parse::<f64>()
    {
        return serde_json::Number::from_f64(number)
            .map(Value::Number)
            .ok_or_else(|| not_a(field, text, "a finite number"));
    }
    if has("array") || has("object") {
        return serde_json::from_str(text).map_err(|error| {
            not_a(
                field,
                text,
                &format!("{} as JSON ({error})", field.type_label.trim()),
            )
        });
    }
    if has("string") {
        return Ok(Value::String(text.to_owned()));
    }
    Err(not_a(field, text, field.type_label.trim()))
}

/// Eine ganze Zahl, mit den Einheiten, die zum Namen des Feldes passen.
fn integer(field: &schema::Field, text: &str) -> Result<i64, Diagnostic> {
    if let Ok(number) = text.parse::<i64>() {
        return Ok(number);
    }
    if field.path.ends_with("_secs")
        && let Some(seconds) = duration_secs(text)
    {
        return Ok(seconds);
    }
    if field.path.ends_with("_ms")
        && let Some(seconds) = duration_secs(text)
    {
        return seconds
            .checked_mul(1000)
            .ok_or_else(|| not_a(field, text, "a number of milliseconds that fits"));
    }
    if field.path.ends_with("_bytes")
        && let Some(bytes) = size_bytes(text)
    {
        return Ok(bytes);
    }
    Err(not_a(field, text, unit_hint(&field.path)))
}

/// Was ein Feld an Schreibweisen annimmt, für den Befund.
const fn unit_hint(path: &str) -> &'static str {
    if ends_with_ascii(path, "_secs") || ends_with_ascii(path, "_ms") {
        "a whole number or a span like 30s, 5m, 2h, 1d"
    } else if ends_with_ascii(path, "_bytes") {
        "a whole number or a size like 512KiB, 32MiB, 1GiB"
    } else {
        "a whole number"
    }
}

/// `str::ends_with` für einen `const fn`: Der Vergleich läuft über die Bytes.
const fn ends_with_ascii(text: &str, suffix: &str) -> bool {
    let (text, suffix) = (text.as_bytes(), suffix.as_bytes());
    if text.len() < suffix.len() {
        return false;
    }
    let offset = text.len() - suffix.len();
    let mut index = 0;
    while index < suffix.len() {
        if text[offset + index] != suffix[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// `30s`, `5m`, `2h`, `1d` als Sekunden.
fn duration_secs(text: &str) -> Option<i64> {
    let (digits, factor) = split_suffix(text, &[("s", 1), ("m", 60), ("h", 3600), ("d", 86_400)])?;
    digits.parse::<i64>().ok()?.checked_mul(factor)
}

/// `512KiB`, `32MiB`, `1GiB` und die Formen ohne `i` als Bytes.
fn size_bytes(text: &str) -> Option<i64> {
    let (digits, factor) = split_suffix(
        text,
        &[
            ("B", 1),
            ("KiB", 1024),
            ("MiB", 1024 * 1024),
            ("GiB", 1024 * 1024 * 1024),
            ("KB", 1000),
            ("MB", 1_000_000),
            ("GB", 1_000_000_000),
            ("K", 1024),
            ("M", 1024 * 1024),
            ("G", 1024 * 1024 * 1024),
        ],
    )?;
    digits.parse::<i64>().ok()?.checked_mul(factor)
}

/// Trennt Zahl und Einheit. Die längste passende Einheit gewinnt, damit `KiB`
/// nicht als `B` gelesen wird.
fn split_suffix<'a>(text: &'a str, units: &[(&'static str, i64)]) -> Option<(&'a str, i64)> {
    let trimmed = text.trim();
    let mut best: Option<(&'static str, i64)> = None;
    for (suffix, factor) in units {
        if trimmed.len() > suffix.len()
            && trimmed
                .get(trimmed.len() - suffix.len()..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
            && best.is_none_or(|(known, _)| known.len() < suffix.len())
        {
            best = Some((suffix, *factor));
        }
    }
    let (suffix, factor) = best?;
    Some((trimmed.get(..trimmed.len() - suffix.len())?.trim(), factor))
}

/// Prüft den gelesenen Wert gegen Aufzählung, Mindest- und Höchstwert.
///
/// Die Wertebereiche, die nur in der Prüfung von `humanitl-config` stehen
/// (`hold.timeout_secs` zwischen 1 und einem Tag), fängt erst [`probe`];
/// hier steht nur, was das Schema selbst sagt.
fn check_value(field: &schema::Field, value: &Value) -> Result<(), Diagnostic> {
    if let (Some(allowed), Some(text)) = (field.allowed.as_ref(), value.as_str())
        && !allowed.iter().any(|known| known == text)
    {
        return Err(Diagnostic::builder(codes::CONFIG_003, Severity::Error)
            .why(format!(
                "{text} is not a value of {}; it takes one of {}",
                field.path,
                allowed.join(", ")
            ))
            .fix(FixAction::ChangeSetting {
                key: field.path.clone(),
                value: allowed.first().cloned().unwrap_or_default(),
            })
            .build());
    }
    let Some(number) = value.as_i64() else {
        return Ok(());
    };
    if let Some(minimum) = field.minimum
        && number < minimum
    {
        return Err(out_of_range(
            field,
            &format!("{number} is below the minimum of {minimum}"),
        ));
    }
    if let Some(maximum) = field.maximum
        && number > maximum
    {
        return Err(out_of_range(
            field,
            &format!("{number} is above the maximum of {maximum}"),
        ));
    }
    Ok(())
}

/// `CONFIG_003`: Der Wert liegt außerhalb des Bereichs.
///
/// Der Vorschlag ist der Vorgabewert und nicht die Grenze: Die Grenze des
/// Schemas ist oft nicht die der Prüfung (`0` passt zum Typ, nicht zur
/// Haltefrist), und ein Vorschlag, der wieder abgelehnt wird, ist keiner.
fn out_of_range(field: &schema::Field, why: &str) -> Diagnostic {
    let mut builder = Diagnostic::builder(codes::CONFIG_003, Severity::Error)
        .why(format!("{}: {why}", field.path));
    if let Some(fix) = default_fix(field) {
        builder = builder.fix(fix);
    }
    builder.build()
}

/// `CONFIG_003`: Der Text ist kein Wert dieses Typs.
fn not_a(field: &schema::Field, text: &str, expected: &str) -> Diagnostic {
    let fix = default_fix(field)
        .unwrap_or_else(|| FixAction::CopyCommand("humanitl config schema".to_owned()));
    Diagnostic::builder(codes::CONFIG_003, Severity::Error)
        .why(format!(
            "{} takes {expected}, and {text} is none",
            field.path
        ))
        .fix(fix)
        .build()
}

/// `config edit`: den Editor auf `config.toml` und danach die Prüfung.
///
/// Die eine interaktive Frage der Kommandozeile, und sie wird nur an einem
/// Terminal gestellt: Ein Skript bekommt den Befund und den Exit-Code, keine
/// Frage, auf die niemand antwortet.
///
/// Der Befund steht genau einmal da. Wird gefragt, schreibt ihn dieses Modul
/// vor die Frage, und bei „nein" endet der Befehl mit seinem Exit-Code, ohne
/// ihn noch einmal zu schreiben; wird nicht gefragt, geht er als `Failure` an
/// `main` und wird dort geschrieben — mit `--json` also als ein Objekt.
fn edit(ctx: &Context) -> Result<u8, Failure> {
    let file = ctx
        .config_file
        .clone()
        .unwrap_or_else(|| ctx.paths.config_path());
    let editor = editor_for(ctx)?;

    loop {
        open_editor(ctx, &editor, &file)?;
        let Some(diagnostic) = check_file(&file) else {
            ctx.render
                .line(&format!("{} is valid TOML", file.display()));
            return Ok(EXIT_OK);
        };
        if !interactive(ctx) {
            return Err(Failure::new(diagnostic));
        }
        ctx.render.diagnostic(&diagnostic);
        if !ask_again() {
            return Ok(crate::cmd::exit_code(&diagnostic));
        }
    }
}

/// Der Editor: `$VISUAL`, dann `$EDITOR`, dann `nano`, dann `vi`.
fn editor_for(ctx: &Context) -> Result<String, Failure> {
    for key in ["VISUAL", "EDITOR"] {
        if let Some(value) = ctx.env.non_empty(key) {
            return Ok(value.to_owned());
        }
    }
    for fallback in FALLBACK_EDITORS {
        if in_path(ctx, fallback) {
            return Ok(fallback.to_owned());
        }
    }
    Err(Failure::new(no_editor(&format!(
        "neither $VISUAL nor $EDITOR is set, and neither {} is in PATH",
        FALLBACK_EDITORS.join(" nor ")
    ))))
}

/// `CONFIG_017`: Es gibt keinen Editor, der sich starten ließe.
fn no_editor(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::CONFIG_017, Severity::Error)
        .why(why.to_owned())
        .fix(FixAction::SetEnv {
            key: "EDITOR".to_owned(),
            value: "nano".to_owned(),
        })
        .build()
}

/// Ob ein Programm im `PATH` dieser Umgebung liegt.
fn in_path(ctx: &Context, program: &str) -> bool {
    ctx.env.non_empty("PATH").is_some_and(|path| {
        path.split(':')
            .filter(|dir| !dir.is_empty())
            .any(|dir| Path::new(dir).join(program).is_file())
    })
}

/// Startet den Editor mit dem Terminal dieses Prozesses.
fn open_editor(ctx: &Context, editor: &str, file: &Path) -> Result<(), Failure> {
    // Der Editor ist ein Befehl mit Argumenten (`code -w`), wie in jeder
    // anderen Umgebung auch; deshalb wird er zerlegt und nicht als ein Wort
    // gestartet.
    let mut words = editor.split_whitespace();
    let program = words.next().unwrap_or(editor);
    let status = std::process::Command::new(program)
        .args(words)
        .arg(file)
        .status()
        .map_err(|error| Failure::new(no_editor(&format!("{editor} did not start: {error}"))))?;
    if status.success() {
        return Ok(());
    }
    ctx.render.note(&format!(
        "{editor} ended with {}; {} is checked anyway",
        status
            .code()
            .map_or_else(|| "a signal".to_owned(), |code| code.to_string()),
        file.display()
    ));
    Ok(())
}

/// Liest die Datei und meldet, was gegen sie spricht; `None`, wenn sie taugt.
///
/// Geprüft wird gegen dieselbe Auflösung, die der Daemon beim Start fährt: Ein
/// unbekannter Schlüssel oder ein Wert außerhalb des Bereichs fällt hier auf
/// und nicht erst beim nächsten Start.
fn check_file(file: &Path) -> Option<Diagnostic> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            return Some(
                Diagnostic::builder(codes::CONFIG_001, Severity::Error)
                    .why(format!("{} cannot be read: {error}", file.display()))
                    .build(),
            );
        }
    };
    if let Err(error) = text.parse::<toml::Table>() {
        return Some(
            Diagnostic::builder(codes::CONFIG_001, Severity::Error)
                .why(format!("{} is not valid TOML: {error}", file.display()))
                .fix(FixAction::CopyCommand("humanitl config edit".to_owned()))
                .build(),
        );
    }
    let sources = humanitl_config::Sources {
        global_toml: Some(file.to_path_buf()),
        ..humanitl_config::Sources::empty()
    };
    humanitl_config::load(&sources).err()
}

/// Ob die eine Frage gestellt werden darf: an einem Terminal und nicht unter
/// `--json`.
fn interactive(ctx: &Context) -> bool {
    use std::io::IsTerminal as _;

    std::io::stdin().is_terminal() && !ctx.render.is_json()
}

/// Die eine interaktive Frage: noch einmal öffnen?
fn ask_again() -> bool {
    use std::io::Write as _;

    eprint!("open it again? [y/N] ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim(), "y" | "Y" | "yes")
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

    /// Was ein Befund nennt: `CONFIG_002` nennt nur einen Vorschlag und hat
    /// keinen Schlüssel, ein alter Name zählt als der heutige, ein Paar nennt
    /// beide, und ein Pfad, der nur der Anfang eines längeren ist, zählt nicht.
    #[test]
    fn a_finding_names_its_keys_but_never_a_suggestion() {
        use humanitl_core::diagnostics::codes::{CONFIG_002, CONFIG_003};
        use humanitl_core::{Diagnostic, Severity};

        let finding = |code, why: &str| {
            super::keys_named_in(
                &Diagnostic::builder(code, Severity::Error)
                    .why(why.to_owned())
                    .build(),
            )
        };

        let (key, involved) = finding(
            CONFIG_002,
            "hold.timeout_secz is not a key; did you mean hold.timeout_secs?",
        );
        assert_eq!(key, None);
        assert!(involved.is_empty());

        let (key, involved) = finding(
            CONFIG_003,
            "limits.hold_max_bytes = 500 is out of range; allowed is at least \
             limits.hold_body_cap_bytes (33554432)",
        );
        assert_eq!(key.as_deref(), Some("limits.hold_max_bytes"));
        assert_eq!(
            involved,
            vec![
                "limits.hold_max_bytes".to_owned(),
                "limits.hold_body_cap_bytes".to_owned()
            ]
        );

        let (key, _) = finding(CONFIG_003, "hold.body_cap_bytes = 1 is out of range");
        assert_eq!(key.as_deref(), Some("limits.hold_body_cap_bytes"));

        // Ein Wert oder eine Herkunft, die wie ein Schlüssel aussieht, nennt
        // keinen.
        let (key, involved) = finding(
            CONFIG_003,
            "ui.theme (from /home/x/hold.timeout_secs/config.toml) expects one of \
             system | light | dark, found \"hold.timeout_secs\"",
        );
        assert_eq!(key.as_deref(), Some("ui.theme"));
        assert_eq!(involved, vec!["ui.theme".to_owned()]);
        let (key, involved) = finding(
            CONFIG_003,
            "sandbox.profile = \"hold.timeout_secs\" is out of range; allowed is a name \
             without a path (default \"default\")",
        );
        assert_eq!(key.as_deref(), Some("sandbox.profile"));
        assert_eq!(involved, vec!["sandbox.profile".to_owned()]);
    }

    /// Ein Punkt am Ende, ein Eintrag unter einer freien Tabelle und ein Wort,
    /// das nirgends vorkommt: Keiner davon bekommt `agent.adapter` vorgeschlagen,
    /// nur weil ein einzelner Buchstabe in fast jedem Schlüssel steckt.
    #[test]
    fn a_suggestion_needs_a_real_resemblance() {
        let dot = suggestions("hold.");
        assert!(
            dot.first().is_some_and(|path| path.starts_with("hold.")),
            "hold. finds the hold keys first: {dot:?}"
        );
        assert!(!dot.contains(&"agent.adapter"), "{dot:?}");

        let entry = suggestions("resolver.overrides.a");
        assert_eq!(entry.first(), Some(&"resolver.overrides"), "{entry:?}");

        assert!(suggestions("nope").is_empty(), "{:?}", suggestions("nope"));
        // Ein Wort aus einem Buchstaben sucht nicht im Inneren: `a` steckt in
        // fast jedem Schlüssel.
        assert!(suggestions("zz.a").is_empty(), "{:?}", suggestions("zz.a"));
        assert!(suggestions(".").is_empty());
    }

    /// Wie ein volles `TMPDIR`: Die Datei entsteht, der Inhalt nicht.
    fn full_disk(path: &std::path::Path, _text: &str) -> std::io::Result<()> {
        std::fs::write(path, "")?;
        Err(std::io::Error::from_raw_os_error(28))
    }

    /// Eine Nebendatei, die sich nicht schreiben lässt, ist ein Befund ohne
    /// Schlüssel (HUM-216). Übergangen, bliebe eine leere Datei zurück, die
    /// als Vorgabe lädt, und die Prüfung meldete Ok.
    fn unwritable_scratch(scope: super::Scope) -> Vec<super::Finding> {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let cwd = root.path().join("work");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let env = Env::from_pairs([
            ("HOME", home.to_str().unwrap()),
            ("XDG_CONFIG_HOME", home.join(".config").to_str().unwrap()),
        ]);
        let ctx = crate::cmd::Context {
            paths: humanitl_config::Paths::new(env.clone()),
            env,
            cwd: cwd.clone(),
            cli_config: Vec::new(),
            config_file: None,
            profile: None,
            profile_means: crate::cmd::ProfileMeaning::Session,
            render: crate::render::Renderer::new(false, 0, true),
        };
        let file = match scope {
            super::Scope::Global => ctx.paths.config_path(),
            super::Scope::Project => cwd.join(".humanitl").join("profile.toml"),
        };
        let text = match scope {
            super::Scope::Global => "[hold]\ntimeout_secs = 7\n",
            super::Scope::Project => "[config.hold]\ntimeout_secs = 7\n",
        };
        let scratch = root.path().join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        super::findings_in(&ctx, scope, Some(text), &file, &scratch, full_disk)
    }

    fn assert_refused_for_scratch(found: &[super::Finding], name: &str) {
        assert_eq!(found.len(), 1, "{found:?}");
        let finding = &found[0];
        assert!(finding.key.is_none(), "{finding:?}");
        assert_eq!(
            finding.diagnostic.code,
            humanitl_core::diagnostics::codes::CONFIG_015
        );
        assert!(
            finding
                .diagnostic
                .why
                .contains("the check could not write its scratch copy"),
            "{}",
            finding.diagnostic.why
        );
        assert!(
            finding.diagnostic.why.contains(name),
            "{}",
            finding.diagnostic.why
        );
    }

    #[test]
    fn a_global_scratch_copy_that_cannot_be_written_is_a_finding() {
        let found = unwritable_scratch(super::Scope::Global);
        assert_refused_for_scratch(&found, "global-0.toml");
    }

    #[test]
    fn a_project_scratch_copy_that_cannot_be_written_is_a_finding() {
        let found = unwritable_scratch(super::Scope::Project);
        assert_refused_for_scratch(&found, "project-0.toml");
    }
}
