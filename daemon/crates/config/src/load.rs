//! Das Laden: sechs Ebenen, eine Reihenfolge, eine Herkunft je Feld.
//!
//! Reihenfolge von unten nach oben (`backlog/CONVENTIONS.md` 4.4):
//!
//! 1. die eingebauten Vorgabewerte aus `impl Default`,
//! 2. die globale `config.toml`,
//! 3. das globale Profil,
//! 4. das Profil des Projekts,
//! 5. Umgebungsvariablen `HUMANITL_*` mit `__` als Trenner der Ebenen,
//! 6. Argumente der Kommandozeile.
//!
//! Gemischt wird nicht auf Tabellen, sondern auf Blattpfaden: jede Ebene wird
//! flach gemacht (`hold.timeout_secs = 42`), und die höhere Ebene ersetzt den
//! Wert der niedrigeren. Damit ersetzt eine Liste immer die Liste darunter,
//! statt an sie anzuhängen — sonst wüchsen `llm.passthrough_paths` mit jeder
//! Ebene —, und die Herkunft steht ohne Nachrechnen fest.
//!
//! Eine freie Tabelle (`resolver.overrides`) ist dabei ein einziges Blatt. Ihre
//! Schlüssel sind Hostnamen mit Punkten; würde man sie zerlegen, entstünden
//! Ebenen, die es nicht gibt, und ein Eintrag aus einem Profil ließe sich weiter
//! oben nicht mehr entfernen.
//!
//! Ein unbekannter Schlüssel in einer Datei oder auf der Kommandozeile ist ein
//! Fehler (`CONFIG_002`): beides gehört uns, ein Tippfehler dort ist einer.
//! In der Umgebung ist er ein Befund mit `Severity::Warning`, kein Fehler:
//! `HUMANITL_*` ist ein geteilter Raum, in dem auch Variablen stehen, die nicht
//! uns gehören, und ein Daemon, der wegen einer fremden Variablen nicht
//! startet, ist schlimmer als einer, der sie meldet. Eine Variable ohne
//! [`ENV_SEPARATOR`] im Namen (`HUMANITL_GALLERY`, `HUMANITL_ESCAPE_MARKER`)
//! kann kein Schlüssel sein, weil kein Blatt auf der obersten Ebene liegt; sie
//! wird still übergangen.
//!
//! Ein Wert aus Umgebung oder Kommandozeile kommt als Text. Er wird nach dem
//! Typ des Feldes gelesen: `2024` ist für `hold.timeout_secs` eine Zahl, für
//! `sandbox.profile` ein Name. Nur wo das Feld keinen Text nimmt, entscheidet
//! die Form des Wertes (`scalar`).
//!
//! Das Projekt-Profil (Ebene 4) liegt im geklonten Repository und ist damit
//! Angreifer-beeinflusst. Ein Schlüssel, den das Schema mit
//! `x-project-scope = "denied"` führt (siehe [`crate::scope`]), ist aus dieser
//! Ebene ein Fehler (`CONFIG_003`), auch unter seinem alten Namen; aus jeder
//! anderen Ebene gilt er wie bisher.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{
    CONFIG_001, CONFIG_002, CONFIG_003, CONFIG_005, CONFIG_006,
};
use humanitl_core::{Diagnostic, FixAction, Severity};
use toml::Value as TomlValue;

use crate::alias;
use crate::env::Env;
use crate::model::Config;
use crate::origin::{Origin, Resolved};
use crate::paths::Paths;
use crate::schema::{self, Field};
use crate::scope::ProjectScope;

/// Das Präfix aller Umgebungsvariablen, die Humanitl liest.
pub const DEFAULT_ENV_PREFIX: &str = "HUMANITL";

/// Der Trenner zwischen zwei Ebenen eines Pfades in einer Umgebungsvariablen.
pub const ENV_SEPARATOR: &str = "__";

/// Der Block, den ein Profil für die Konfiguration benutzt.
///
/// In Sprint 0 ist das der einzige Block, den ein Profil hat. `[rules]` und
/// `[agent]` kommen mit HUM-066 und werden hier bis dahin übergangen.
pub const PROFILE_SECTION: &str = "config";

/// Die Blöcke, die ein Profil neben [`PROFILE_SECTION`] haben darf, ohne dass
/// diese Crate sie liest: Name und Beschreibung des Profils sowie die Blöcke,
/// die HUM-066 füllt. Jeder andere Block auf der obersten Ebene ist ein
/// Tippfehler oder eine Gruppe, die unter `[config]` gehört; beides ist
/// `CONFIG_002`, sonst bliebe ein Profil ohne Wirkung und ohne Meldung.
pub const PROFILE_PASSTHROUGH: &[&str] = &["name", "description", "rules", "agent"];

/// Die Quellen, aus denen eine Konfiguration entsteht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    /// Die globale `config.toml`.
    pub global_toml: Option<PathBuf>,
    /// Das globale Profil aus `profiles/<name>.toml`.
    pub profile_global: Option<PathBuf>,
    /// Das Profil des Projekts, `<projekt>/.humanitl/profile.toml`.
    pub profile_project: Option<PathBuf>,
    /// Das Präfix der Umgebungsvariablen, normalerweise [`DEFAULT_ENV_PREFIX`].
    pub env_prefix: &'static str,
    /// Die Umgebung. Übergeben, nicht aus dem Prozess gelesen.
    pub env: Env,
    /// Paare aus Pfad und Wert von der Kommandozeile, zum Beispiel
    /// `("hold.timeout_secs", "42")`.
    pub cli: Vec<(String, String)>,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            global_toml: None,
            profile_global: None,
            profile_project: None,
            env_prefix: DEFAULT_ENV_PREFIX,
            env: Env::default(),
            cli: Vec::new(),
        }
    }
}

impl Sources {
    /// Nur die Vorgabewerte, ohne Datei, ohne Umgebung.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Setzt die Umgebung.
    #[must_use]
    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    /// Setzt die Paare der Kommandozeile.
    #[must_use]
    pub fn with_cli<K, V, I>(mut self, pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.cli = pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        self
    }
}

/// Sucht die Quellen im Dateisystem: XDG plus `<cwd>/.humanitl/profile.toml`.
///
/// Nur Dateien, die es gibt, landen in den Quellen; ein fehlendes Profil ist
/// kein Fehler. Wer einen Pfad ausdrücklich angibt, bekommt dagegen einen
/// Fehler, wenn er nicht existiert — dann war es ein Tippfehler, kein Zufall.
#[must_use]
pub fn discover(cwd: &Path) -> Sources {
    discover_with(&Env::from_process(), cwd, None)
}

/// Wie [`discover`], aber mit übergebener Umgebung und Profilnamen.
///
/// Ohne Namen gilt das Profil `default`. `sandbox.profile` ist hier keine
/// Quelle: es benennt das Sandbox-Profil unter `profiles/sandbox/` (HUM-010),
/// nicht das Konfigurationsprofil unter `$XDG_CONFIG_HOME/humanitl/profiles/`.
#[must_use]
pub fn discover_with(env: &Env, cwd: &Path, profile: Option<&str>) -> Sources {
    let paths = Paths::new(env.clone());
    let global = paths.config_path();
    let profile_file = paths.profile_path(profile.unwrap_or("default"));
    let project = paths.project_profile_path(cwd);

    Sources {
        global_toml: global.is_file().then_some(global),
        profile_global: profile_file.is_file().then_some(profile_file),
        profile_project: project.is_file().then_some(project),
        env_prefix: DEFAULT_ENV_PREFIX,
        env: env.clone(),
        cli: Vec::new(),
    }
}

/// Ein Schlüssel einer Ebene, schon auf seinen heutigen Pfad gebracht.
#[derive(Debug, Clone)]
struct Entry {
    path: String,
    written_as: String,
    value: TomlValue,
    origin: Origin,
    via_alias: bool,
}

/// Lädt die Konfiguration aus allen Quellen.
///
/// Was das Laden überlebt, steht als Befund in [`Resolved::diagnostics`]:
/// `CONFIG_005` (Info) für einen veralteten Schlüssel, `CONFIG_006` (Warning),
/// wenn alter und neuer Schlüssel nebeneinander stehen, `CONFIG_002` (Warning)
/// für einen unbekannten Schlüssel in der Umgebung. Nur eine Datei oder die
/// Kommandozeile macht daraus einen Fehler, siehe unten.
///
/// # Errors
///
/// - `CONFIG_001`, wenn eine angegebene Datei fehlt oder kein gültiges TOML ist,
/// - `CONFIG_002`, wenn eine Datei oder die Kommandozeile einen Schlüssel nennt,
///   den das Schema nicht kennt,
/// - `CONFIG_003`, wenn ein Wert den falschen Typ hat oder außerhalb seines
///   Bereichs liegt, oder wenn das Projekt-Profil einen Schlüssel setzt, den
///   das Schema für diese Ebene sperrt (`x-project-scope = "denied"`).
pub fn load(sources: &Sources) -> Result<Resolved, Diagnostic> {
    let free_tables = schema::free_table_paths();
    let mut merge = Merge::new();
    let mut layers: Vec<(Vec<Entry>, bool)> = Vec::new();

    if let Some(path) = &sources.global_toml {
        let table = read_table(path, None)?;
        layers.push((entries_from_table(&table, &Origin::Global, &free_tables), true));
    }
    if let Some(path) = &sources.profile_global {
        let table = read_table(path, Some(PROFILE_SECTION))?;
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        layers.push((
            entries_from_table(&table, &Origin::ProfileGlobal(name), &free_tables),
            true,
        ));
    }
    if let Some(path) = &sources.profile_project {
        let table = read_table(path, Some(PROFILE_SECTION))?;
        layers.push((
            entries_from_table(&table, &Origin::ProfileProject(path.clone()), &free_tables),
            true,
        ));
    }
    layers.push((env_entries(&sources.env, sources.env_prefix), false));
    layers.push((cli_entries(&sources.cli), true));

    for (entries, hard) in layers {
        merge.apply(entries, hard)?;
    }
    let Merge {
        flat,
        origins,
        mut diagnostics,
        alias_uses,
        canonical_uses,
    } = merge;

    diagnostics.extend(alias_diagnostics(&alias_uses, &canonical_uses));

    let table = nest(&flat)?;
    let config: Config = TomlValue::Table(table).try_into().map_err(|err: toml::de::Error| {
        Diagnostic::new(CONFIG_003, Severity::Error)
            .why(format!("the merged configuration does not fit the schema: {err}"))
            .build()
    })?;
    config.validate()?;

    Ok(Resolved {
        config,
        origins,
        diagnostics,
    })
}

fn read_table(path: &Path, section: Option<&str>) -> Result<toml::Table, Diagnostic> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(CONFIG_001, Severity::Error)
            .why(format!("cannot read {}: {err}", path.display()))
            .build()
    })?;
    let table: toml::Table = text.parse().map_err(|err: toml::de::Error| {
        Diagnostic::new(CONFIG_001, Severity::Error)
            .why(format!("{} is not valid TOML: {err}", path.display()))
            .build()
    })?;

    let Some(section) = section else {
        return Ok(table);
    };
    for key in table.keys() {
        if key == section || PROFILE_PASSTHROUGH.contains(&key.as_str()) {
            continue;
        }
        let why = if schema::field(key).is_some_and(|field| field.group) {
            format!(
                "[{key}] in {} is a group of settings; in a profile it belongs under \
                 [{section}.{key}]",
                path.display()
            )
        } else {
            format!(
                "[{key}] in {} is not a block of a profile; settings go under [{section}]",
                path.display()
            )
        };
        return Err(Diagnostic::new(CONFIG_002, Severity::Error).why(why).build());
    }
    match table.get(section) {
        None => Ok(toml::Table::new()),
        Some(TomlValue::Table(inner)) => Ok(inner.clone()),
        Some(other) => Err(Diagnostic::new(CONFIG_001, Severity::Error)
            .why(format!(
                "[{section}] in {} must be a table, found {}",
                path.display(),
                other.type_str()
            ))
            .build()),
    }
}

fn entries_from_table(
    table: &toml::Table,
    origin: &Origin,
    free_tables: &BTreeSet<&'static str>,
) -> Vec<Entry> {
    let mut leaves = Vec::new();
    flatten(table, "", free_tables, &mut leaves);
    leaves
        .into_iter()
        .map(|(written_as, value)| entry_from_value(written_as, value, origin.clone()))
        .collect()
}

fn flatten(
    table: &toml::Table,
    prefix: &str,
    free_tables: &BTreeSet<&'static str>,
    out: &mut Vec<(String, TomlValue)>,
) {
    for (key, value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let canonical = alias::canonical(&path).unwrap_or(path.as_str());
        match value {
            TomlValue::Table(inner) if !free_tables.contains(canonical) => {
                flatten(inner, &path, free_tables, out);
            }
            other => out.push((path, other.clone())),
        }
    }
}

/// Ein Eintrag aus einer Datei: der Wert hat schon einen Typ.
fn entry_from_value(written_as: String, value: TomlValue, origin: Origin) -> Entry {
    let canonical = alias::canonical(&written_as);
    Entry {
        path: canonical.map_or_else(|| written_as.clone(), ToOwned::to_owned),
        written_as,
        value,
        origin,
        via_alias: canonical.is_some(),
    }
}

/// Ein Eintrag aus Umgebung oder Kommandozeile: der Wert ist Text und wird
/// nach dem Typ des Feldes gelesen.
fn entry_from_text(written_as: String, raw: &str, origin: Origin) -> Entry {
    let canonical = alias::canonical(&written_as);
    let path = canonical.map_or_else(|| written_as.clone(), ToOwned::to_owned);
    let value = if wants_text(&path) {
        TomlValue::String(raw.to_owned())
    } else {
        scalar(raw)
    };
    Entry {
        path,
        written_as,
        value,
        origin,
        via_alias: canonical.is_some(),
    }
}

/// Ob ein Feld nur Text annimmt. Dann bleibt `2024` ein Profilname und `true`
/// ein Begriff, statt zur Zahl oder zum Wahrheitswert zu werden.
fn wants_text(path: &str) -> bool {
    schema::field(path).is_some_and(|field| {
        field.allowed.is_some()
            || (field.types.iter().any(|kind| kind == "string")
                && !field.types.iter().any(|kind| {
                    matches!(
                        kind.as_str(),
                        "integer" | "number" | "boolean" | "array" | "object"
                    )
                }))
    })
}

fn env_entries(env: &Env, prefix: &str) -> Vec<Entry> {
    let head = format!("{prefix}_");
    let mut out = Vec::new();
    for (key, value) in env {
        let Some(rest) = key.strip_prefix(&head) else {
            continue;
        };
        // Ohne Trenner kein Pfad: kein Blatt liegt auf der obersten Ebene (der
        // Test `every_group_has_at_least_one_leaf` hält das fest). Solche
        // Variablen gehören anderen Werkzeugen von Humanitl.
        if !rest.contains(ENV_SEPARATOR) {
            continue;
        }
        let path = rest
            .to_lowercase()
            .split(ENV_SEPARATOR)
            .collect::<Vec<_>>()
            .join(".");
        out.push(entry_from_text(path, value, Origin::Env(key.clone())));
    }
    out
}

fn cli_entries(pairs: &[(String, String)]) -> Vec<Entry> {
    pairs
        .iter()
        .map(|(key, value)| entry_from_text(key.clone(), value, Origin::Cli))
        .collect()
}

/// Liest einen Wert aus einer Zeichenkette, ohne TOML zu bemühen.
///
/// Erst Wahrheitswert, dann ganze Zahl, dann Kommazahl, sonst Zeichenkette. Ein
/// Endpunkt wie `http://box:8080/v1` ist kein gültiges TOML; würde man ihn
/// parsen, scheiterte er am Doppelpunkt. Nur eckige und geschweifte Klammern
/// gehen den Weg über TOML, weil sich eine Liste anders nicht schreiben lässt.
fn scalar(raw: &str) -> TomlValue {
    match raw {
        "true" => return TomlValue::Boolean(true),
        "false" => return TomlValue::Boolean(false),
        _ => {}
    }

    let numeric = raw.starts_with(['-', '+']) || raw.starts_with(|c: char| c.is_ascii_digit());
    if numeric && raw.chars().any(|c| c.is_ascii_digit()) {
        if let Ok(int) = raw.parse::<i64>() {
            return TomlValue::Integer(int);
        }
        if let Ok(float) = raw.parse::<f64>() {
            if float.is_finite() {
                return TomlValue::Float(float);
            }
        }
    }

    if raw.starts_with('[') || raw.starts_with('{') {
        if let Ok(table) = format!("value = {raw}").parse::<toml::Table>() {
            if let Some(value) = table.get("value") {
                return value.clone();
            }
        }
    }

    TomlValue::String(raw.to_owned())
}

/// Eine Stelle, an der ein alter Name benutzt wurde.
struct AliasUse {
    written_as: String,
    origin: Origin,
    value: TomlValue,
}

/// Der Stand des Mischens: Werte, Herkunft, Befunde, und wer welchen Namen
/// benutzt hat.
struct Merge {
    flat: BTreeMap<String, TomlValue>,
    origins: BTreeMap<String, Origin>,
    diagnostics: Vec<Diagnostic>,
    alias_uses: BTreeMap<String, Vec<AliasUse>>,
    canonical_uses: BTreeMap<String, Vec<Origin>>,
}

impl Merge {
    /// Beginnt mit den Vorgabewerten: jedes Blatt des Schemas hat eine
    /// Herkunft, auch das, das niemand anfasst.
    fn new() -> Self {
        Self {
            flat: BTreeMap::new(),
            origins: schema::leaf_paths()
                .into_iter()
                .map(|path| (path.to_owned(), Origin::Default))
                .collect(),
            diagnostics: Vec::new(),
            alias_uses: BTreeMap::new(),
            canonical_uses: BTreeMap::new(),
        }
    }

    /// Legt eine Ebene auf den Stand. `hard` sagt, ob ein unbekannter Schlüssel
    /// das Laden abbricht oder nur einen Befund erzeugt. Ein Befund, der das
    /// Laden nicht abbricht, ist eine Warnung (heute: die Umgebung), nie ein
    /// Fehler; ein `Error` in `diagnostics` sähe aus wie ein gescheiterter Start.
    fn apply(&mut self, mut entries: Vec<Entry>, hard: bool) -> Result<(), Diagnostic> {
        let severity = if hard {
            Severity::Error
        } else {
            Severity::Warning
        };
        // Innerhalb einer Ebene zuerst die Aliasse, dann die heutigen Namen: so
        // gewinnt der heutige Name, egal in welcher Reihenfolge beide dastehen.
        entries.sort_by_key(|entry| u8::from(!entry.via_alias));

        for entry in entries {
            let Some(field) = schema::field(&entry.path).filter(|field| !field.group) else {
                let diagnostic = unknown_key(&entry, severity);
                if hard {
                    return Err(diagnostic);
                }
                self.diagnostics.push(diagnostic);
                continue;
            };

            // Die Vertrauensgrenze vor der Wertprüfung: ein gesperrter
            // Schlüssel aus dem Projekt-Profil scheitert als gesperrt, nicht
            // als falscher Typ. Und er scheitert unabhängig von `hard`: der
            // Wert darf nie in den Stand gelangen, ein Befund allein reichte
            // nicht.
            if matches!(entry.origin, Origin::ProfileProject(_))
                && field.project_scope == ProjectScope::Denied
            {
                return Err(project_scope_denied(&entry));
            }

            if let Err(diagnostic) = check_value(field, &entry) {
                if hard {
                    return Err(diagnostic);
                }
                self.diagnostics.push(diagnostic);
                continue;
            }

            if entry.via_alias {
                self.alias_uses
                    .entry(entry.path.clone())
                    .or_default()
                    .push(AliasUse {
                        written_as: entry.written_as.clone(),
                        origin: entry.origin.clone(),
                        value: entry.value.clone(),
                    });
            } else {
                self.canonical_uses
                    .entry(entry.path.clone())
                    .or_default()
                    .push(entry.origin.clone());
            }

            self.origins.insert(entry.path.clone(), entry.origin);
            self.flat.insert(entry.path, entry.value);
        }
        Ok(())
    }
}

/// Ein gesperrter Schlüssel aus dem Projekt-Profil (`backlog/CONVENTIONS.md`
/// 4.11). Die Meldung nennt den Schlüssel, wie er dasteht und wie er heute
/// heißt, die Ebene und wohin die Einstellung gehört.
///
/// Absichtlich ohne `FixAction`: ein Knopf „Einstellung übernehmen" trüge den
/// Wert des Angreifers mit einem Klick in die globale Konfiguration und machte
/// die Grenze zunichte. Der Hinweis steht deshalb nur im Text.
fn project_scope_denied(entry: &Entry) -> Diagnostic {
    let key = if entry.via_alias {
        format!("{} (the old name of {})", entry.written_as, entry.path)
    } else {
        entry.path.clone()
    };
    Diagnostic::new(CONFIG_003, Severity::Error)
        .why(format!(
            "{key} (from {}) may not be set by a project profile: the file is part of the \
             repository and cannot decide trust-relevant settings; move this setting to the \
             global config or profile",
            entry.origin
        ))
        .build()
}

fn unknown_key(entry: &Entry, severity: Severity) -> Diagnostic {
    let known_group = schema::field(&entry.path).is_some_and(|field| field.group);
    let why = if known_group {
        format!(
            "{} (from {}) is a group of settings, not a value",
            entry.written_as, entry.origin
        )
    } else {
        let hint = nearest(&entry.path)
            .map_or_else(String::new, |near| format!("; did you mean {near}?"));
        format!(
            "{} (from {}) is not a key of the schema{hint}",
            entry.written_as, entry.origin
        )
    };
    let mut builder = Diagnostic::new(CONFIG_002, severity).why(why);
    if let Some(near) = nearest(&entry.path) {
        builder = builder.fix(FixAction::ChangeSetting {
            key: near.to_owned(),
            value: entry.value.to_string(),
        });
    }
    builder.build()
}

/// Der ähnlichste bekannte Pfad, gemessen an gemeinsamen Zeichen am Anfang und
/// am Ende. Reicht für Tippfehler wie `timeoutt_secs`.
fn nearest(path: &str) -> Option<&'static str> {
    schema::leaf_paths()
        .into_iter()
        .map(|candidate| (similarity(candidate, path), candidate))
        .filter(|(score, _)| *score * 2 > path.len())
        .max_by_key(|(score, _)| *score)
        .map(|(_, candidate)| candidate)
}

fn similarity(left: &str, right: &str) -> usize {
    let front = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let back = left
        .bytes()
        .rev()
        .zip(right.bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    front + back.min(left.len().min(right.len()) - front)
}

fn check_value(field: &Field, entry: &Entry) -> Result<(), Diagnostic> {
    let found = entry.value.type_str();

    if let Some(allowed) = &field.allowed {
        let ok = entry
            .value
            .as_str()
            .is_some_and(|value| allowed.iter().any(|option| option == value));
        if !ok {
            return Err(bad_value(
                entry,
                &format!("one of {}", allowed.join(" | ")),
                &entry.value.to_string(),
            ));
        }
        return Ok(());
    }

    if !field.types.is_empty() {
        let matches = field.types.iter().any(|wanted| match wanted.as_str() {
            "integer" => entry.value.is_integer(),
            "number" => entry.value.is_integer() || entry.value.is_float(),
            "boolean" => entry.value.is_bool(),
            "string" => entry.value.is_str() || entry.value.is_datetime(),
            "array" => entry.value.is_array(),
            "object" => entry.value.is_table(),
            _ => false,
        });
        if !matches {
            return Err(bad_value(entry, &field.type_label, found));
        }
    }

    if let Some(int) = entry.value.as_integer() {
        if field.minimum.is_some_and(|min| int < min)
            || field.maximum.is_some_and(|max| int > max)
        {
            let range = match (field.minimum, field.maximum) {
                (Some(min), Some(max)) => format!("{min} to {max}"),
                (Some(min), None) => format!("{min} or more"),
                (None, Some(max)) => format!("{max} or less"),
                (None, None) => "a number".to_owned(),
            };
            return Err(bad_value(entry, &range, &int.to_string()));
        }
    }

    Ok(())
}

fn bad_value(entry: &Entry, expected: &str, found: &str) -> Diagnostic {
    Diagnostic::new(CONFIG_003, Severity::Error)
        .why(format!(
            "{} (from {}) expects {expected}, found {found}",
            entry.written_as, entry.origin
        ))
        .fix(FixAction::ChangeSetting {
            key: entry.path.clone(),
            value: schema::field(&entry.path)
                .map_or_else(|| "-".to_owned(), |field| field.default_literal()),
        })
        .build()
}

/// Befunde zu alten Namen: ein Hinweis (`CONFIG_005`), wenn einer benutzt
/// wird, eine Warnung (`CONFIG_006`), wenn alter und neuer Name nebeneinander
/// stehen.
///
/// Wer gewonnen hat, steht in der Meldung, und zwar richtig: innerhalb einer
/// Ebene der neue Name, über Ebenen hinweg die höhere Ebene, auch wenn dort
/// der alte Name steht. Die Ebenen kommen in Rangfolge, darum ist der letzte
/// Eintrag je Liste der höchste.
fn alias_diagnostics(
    alias_uses: &BTreeMap<String, Vec<AliasUse>>,
    canonical_uses: &BTreeMap<String, Vec<Origin>>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (path, uses) in alias_uses {
        let Some(AliasUse {
            written_as,
            origin,
            value,
        }) = uses.last()
        else {
            continue;
        };
        let Some(canonical) = canonical_uses.get(path).and_then(|places| places.last()) else {
            out.push(
                Diagnostic::new(CONFIG_005, Severity::Info)
                    .why(format!(
                        "{written_as} (from {origin}) is the old name of {path} and still works; \
                         rename it."
                    ))
                    .fix(FixAction::ChangeSetting {
                        key: path.clone(),
                        value: value.to_string(),
                    })
                    .build(),
            );
            continue;
        };
        let why = if origin.rank() > canonical.rank() {
            format!(
                "{written_as} (from {origin}) and {path} (from {canonical}) name the same \
                 setting; {written_as} wins because {origin} ranks higher. Rename it to {path}."
            )
        } else {
            format!(
                "{written_as} (from {origin}) and {path} (from {canonical}) name the same \
                 setting; {path} wins. Remove {written_as}."
            )
        };
        out.push(
            Diagnostic::new(CONFIG_006, Severity::Warning)
                .why(why)
                .build(),
        );
    }
    out
}

fn nest(flat: &BTreeMap<String, TomlValue>) -> Result<toml::Table, Diagnostic> {
    let mut root = toml::Table::new();
    for (path, value) in flat {
        let mut table = &mut root;
        let mut segments = path.split('.').peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                table.insert(segment.to_owned(), value.clone());
                break;
            }
            let next = table
                .entry(segment.to_owned())
                .or_insert_with(|| TomlValue::Table(toml::Table::new()));
            let TomlValue::Table(inner) = next else {
                return Err(Diagnostic::new(CONFIG_002, Severity::Error)
                    .why(format!("{path} passes through {segment}, which holds a value"))
                    .build());
            };
            table = inner;
        }
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use toml::Value as TomlValue;

    use super::{nearest, scalar};

    #[test]
    fn scalars_are_read_without_toml() {
        assert_eq!(scalar("true"), TomlValue::Boolean(true));
        assert_eq!(scalar("42"), TomlValue::Integer(42));
        assert_eq!(scalar("-7"), TomlValue::Integer(-7));
        assert_eq!(scalar("abc"), TomlValue::String("abc".to_owned()));
        assert_eq!(
            scalar("http://box:8080/v1"),
            TomlValue::String("http://box:8080/v1".to_owned())
        );
        assert_eq!(scalar("inf"), TomlValue::String("inf".to_owned()));
        assert_eq!(scalar("1.5"), TomlValue::Float(1.5));
        assert_eq!(
            scalar("[\"/v1/\"]"),
            TomlValue::Array(vec![TomlValue::String("/v1/".to_owned())])
        );
    }

    #[test]
    fn a_typo_finds_its_neighbour() {
        assert_eq!(nearest("hold.timeoutt_secs"), Some("hold.timeout_secs"));
        assert_eq!(nearest("nothing.like.a.key"), None);
    }
}
