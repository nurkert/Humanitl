//! Das Laden: sieben Ebenen, eine Reihenfolge, eine Herkunft je Feld.
//!
//! Reihenfolge von unten nach oben (`backlog/CONVENTIONS.md` 4.4 und 4.23):
//!
//! 1. die eingebauten Vorgabewerte aus `impl Default`,
//! 2. die globale `config.toml`,
//! 3. das Profil `default`, als Datei oder eingebettet,
//! 4. das gewählte Profil, falls es nicht `default` ist,
//! 5. das Profil des Projekts,
//! 6. Umgebungsvariablen `HUMANITL_*` mit `__` als Trenner der Ebenen,
//! 7. Argumente der Kommandozeile.
//!
//! Welche Profile die Ebenen 3 und 4 besetzen, entscheidet [`mod@crate::resolve`];
//! hier stehen sie schon als Liste in [`Sources::profiles`].
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
//! Das Projekt-Profil (Ebene 5) liegt im geklonten Repository und ist damit
//! Angreifer-beeinflusst. Ein Schlüssel, den das Schema mit
//! `x-project-scope = "denied"` führt (siehe [`crate::scope`]), ist aus dieser
//! Ebene ein Fehler (`CONFIG_003`), auch unter seinem alten Namen; aus jeder
//! anderen Ebene gilt er wie bisher.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{
    CONFIG_001, CONFIG_002, CONFIG_003, CONFIG_005, CONFIG_006, CONFIG_007, CONFIG_008, CONFIG_009,
};
use humanitl_core::{Diagnostic, FixAction, Severity};
use toml::Value as TomlValue;

use crate::alias;
use crate::env::Env;
use crate::model::Config;
use crate::origin::{Origin, Resolved};
use crate::profile::{ConfigOverlay, Profile, ProfileSource, builtin_text};
use crate::schema::{self, Field};
use crate::scope::ProjectScope;

/// Das Präfix aller Umgebungsvariablen, die Humanitl liest.
pub const DEFAULT_ENV_PREFIX: &str = "HUMANITL";

/// Der Trenner zwischen zwei Ebenen eines Pfades in einer Umgebungsvariablen.
pub const ENV_SEPARATOR: &str = "__";

/// Der Block, den ein Profil für die Konfiguration benutzt.
///
/// Konfigurationswerte stehen in einem Profil ausschließlich unter
/// `[config.<gruppe>]` (`backlog/CONVENTIONS.md` 4.11). Was ein Profil sonst
/// haben darf, steht in [`crate::profile::PROFILE_KEYS`].
pub const PROFILE_SECTION: &str = "config";

/// Die Quellen, aus denen eine Konfiguration entsteht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    /// Die globale `config.toml`.
    pub global_toml: Option<PathBuf>,
    /// Die Profil-Ebenen, von der niedrigsten zur höchsten.
    ///
    /// Normalerweise sind das zwei: das Profil `default` und darüber das
    /// gewählte. [`crate::resolve::profile_layers`] stellt sie zusammen.
    pub profiles: Vec<ProfileSource>,
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
            profiles: Vec::new(),
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

    /// Setzt die Profil-Ebenen, von der niedrigsten zur höchsten.
    #[must_use]
    pub fn with_profiles<I: IntoIterator<Item = ProfileSource>>(mut self, profiles: I) -> Self {
        self.profiles = profiles.into_iter().collect();
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
/// für einen unbekannten Schlüssel in der Umgebung, `CONFIG_007` (Warning) für
/// ein Projekt-Profil aus fremdem Besitz und `CONFIG_008` (Info), wenn eine
/// eigene Profildatei ein mitgeliefertes Profil verdeckt. Nur eine Datei oder
/// die Kommandozeile macht daraus einen Fehler, siehe unten.
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
    let mut profiles: Vec<Profile> = Vec::new();

    if let Some(path) = &sources.global_toml {
        let table = read_table(path)?;
        layers.push((
            entries_from_table(&table, &Origin::Global, &free_tables),
            true,
        ));
    }
    let mut notes: Vec<Diagnostic> = Vec::new();
    for source in &sources.profiles {
        if let ProfileSource::File(path) = source
            && let Some(note) = shadowed_builtin(path)
        {
            notes.push(note);
        }
        profiles.push(source.load()?);
    }
    if let Some(path) = &sources.profile_project {
        if let Some(note) = foreign_owner(path, sources.env.uid()) {
            notes.push(note);
        }
        let project = ProfileSource::Project(path.clone()).load()?;
        if let Some(note) = ignored_project_choice(&project, &sources.profiles) {
            notes.push(note);
        }
        profiles.push(project);
    }
    for profile in &profiles {
        layers.push((
            entries_from_overlay(&profile.overlay, &profile.source.origin()),
            true,
        ));
    }
    layers.push((env_entries(&sources.env, sources.env_prefix), false));
    layers.push((cli_entries(&sources.cli), true));

    for (layer, (entries, hard)) in layers.into_iter().enumerate() {
        merge.apply(entries, hard, layer)?;
    }
    let Merge {
        flat,
        origins,
        diagnostics: merged,
        alias_uses,
        canonical_uses,
    } = merge;

    let mut diagnostics = notes;
    diagnostics.extend(merged);
    diagnostics.extend(alias_diagnostics(&alias_uses, &canonical_uses));

    let table = nest(&flat)?;
    let config: Config = TomlValue::Table(table)
        .try_into()
        .map_err(|err: toml::de::Error| {
            Diagnostic::builder(CONFIG_003, Severity::Error)
                .why(format!(
                    "the merged configuration does not fit the schema: {err}"
                ))
                .build()
        })?;
    config.validate()?;

    Ok(Resolved {
        config,
        origins,
        profiles,
        diagnostics,
    })
}

fn read_table(path: &Path) -> Result<toml::Table, Diagnostic> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        Diagnostic::builder(CONFIG_001, Severity::Error)
            .why(format!("cannot read {}: {err}", path.display()))
            .build()
    })?;
    text.parse().map_err(|err: toml::de::Error| {
        Diagnostic::builder(CONFIG_001, Severity::Error)
            .why(format!("{} is not valid TOML: {err}", path.display()))
            .build()
    })
}

/// Die Blattpfade einer Tabelle, wie sie dastehen, mit ihren Werten.
///
/// Der Einstieg für [`crate::profile::ConfigOverlay`]: dieselbe Zerlegung wie
/// beim Mischen, damit ein Profil feldweise wirkt und eine freie Tabelle ein
/// einziges Blatt bleibt.
pub(crate) fn overlay_from_table(table: &toml::Table) -> BTreeMap<String, TomlValue> {
    let free_tables = schema::free_table_paths();
    let mut leaves = Vec::new();
    flatten(table, "", &free_tables, &mut leaves);
    leaves.into_iter().collect()
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

fn entries_from_overlay(overlay: &ConfigOverlay, origin: &Origin) -> Vec<Entry> {
    overlay
        .iter()
        .map(|(written_as, value)| {
            entry_from_value(written_as.to_owned(), value.clone(), origin.clone())
        })
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
        if let Ok(float) = raw.parse::<f64>()
            && float.is_finite()
        {
            return TomlValue::Float(float);
        }
    }

    if (raw.starts_with('[') || raw.starts_with('{'))
        && let Ok(table) = format!("value = {raw}").parse::<toml::Table>()
        && let Some(value) = table.get("value")
    {
        return value.clone();
    }

    TomlValue::String(raw.to_owned())
}

/// Eine Stelle, an der ein alter Name benutzt wurde.
struct AliasUse {
    written_as: String,
    origin: Origin,
    value: TomlValue,
    /// Der Index der Ebene, in der er stand. Nur er entscheidet, welche von
    /// zwei Stellen höher liegt; [`Origin::rank`] fasst nur Bänder zusammen
    /// und stellte zwei Profil-Ebenen falsch gegeneinander.
    layer: usize,
}

/// Der Stand des Mischens: Werte, Herkunft, Befunde, und wer welchen Namen
/// benutzt hat.
struct Merge {
    flat: BTreeMap<String, TomlValue>,
    origins: BTreeMap<String, Origin>,
    diagnostics: Vec<Diagnostic>,
    alias_uses: BTreeMap<String, Vec<AliasUse>>,
    canonical_uses: BTreeMap<String, Vec<(usize, Origin)>>,
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
    fn apply(
        &mut self,
        mut entries: Vec<Entry>,
        hard: bool,
        layer: usize,
    ) -> Result<(), Diagnostic> {
        let severity = if hard {
            Severity::Error
        } else {
            Severity::Warning
        };
        // Innerhalb einer Ebene zuerst die Aliasse, dann die heutigen Namen: so
        // gewinnt der heutige Name, egal in welcher Reihenfolge beide dastehen.
        entries.sort_by_key(|entry| u8::from(!entry.via_alias));

        for entry in entries {
            // Vor allem anderen: Ein Projekt-Profil, das Host-Pfade einhängen
            // will, wird als das abgelehnt, was es ist. Die Schlüssel gibt es
            // im Schema nicht — Einhängungen stehen im Sandbox-Profil, nicht in
            // der Konfiguration —, und ein „unbekannter Schlüssel, meintest du
            // sandbox.profile?" wäre hier die irreführendste aller Antworten.
            if matches!(entry.origin, Origin::ProfileProject(_)) && is_mount_key(&entry.path) {
                return Err(project_mount_refused(&entry));
            }

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
                        layer,
                    });
            } else {
                self.canonical_uses
                    .entry(entry.path.clone())
                    .or_default()
                    .push((layer, entry.origin.clone()));
            }

            self.origins.insert(entry.path.clone(), entry.origin);
            self.flat.insert(entry.path, entry.value);
        }
        Ok(())
    }
}

/// Der Hinweis, wenn eine eigene Profildatei ein mitgeliefertes Profil verdeckt.
///
/// Nur, wenn sie sich vom mitgelieferten Text unterscheidet: eine wortgleiche
/// Kopie ändert nichts und muss niemandem gemeldet werden. Unterscheiden sie
/// sich, gewinnt die Datei — und das soll man erfahren, statt es sich an einer
/// überraschenden Sandbox zusammenzureimen. Ein stiller Vorzug für eine der
/// beiden Seiten wäre die schlechtere Antwort.
fn shadowed_builtin(path: &Path) -> Option<Diagnostic> {
    let name = path.file_stem()?.to_str()?;
    let bundled = builtin_text(name)?;
    let own = std::fs::read_to_string(path).ok()?;
    if own == bundled {
        return None;
    }
    Some(
        Diagnostic::builder(CONFIG_008, Severity::Info)
            .why(format!(
                "{} replaces the bundled profile {name} and differs from it; the file decides, \
                 the bundled version is not used",
                path.display()
            ))
            .fix(FixAction::CopyCommand(
                "humanitl config schema --profiles".to_owned(),
            ))
            .build(),
    )
}

/// Der Befund, wenn der Profilwunsch des Projekts nicht gilt.
///
/// Zwei Gründe, warum das vorkommt, und beide gehören dem Nutzer gesagt: Die
/// Kommandozeile hat ein anderes Profil genannt, oder der Wunsch war keines der
/// mitgelieferten und wurde deshalb übergangen (`crate::resolve`). Still zu
/// bleiben hieße, jemanden ein Profil pflegen zu lassen, das nie gilt.
fn ignored_project_choice(project: &Profile, layers: &[ProfileSource]) -> Option<Diagnostic> {
    let wish = project.declared_name.as_deref()?;
    if layers.iter().any(|source| source.name() == wish) {
        return None;
    }
    let applied = layers
        .iter()
        .map(ProfileSource::name)
        .collect::<Vec<_>>()
        .join(", ");
    Some(
        Diagnostic::builder(CONFIG_009, Severity::Warning)
            .why(format!(
                "the project profile asks for the profile {wish}, and it does not apply; {applied} \
                 applies instead. A project may only choose a bundled profile, and a name on the \
                 command line goes first."
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl run --profile {wish}"
            )))
            .build(),
    )
}

/// Der Befund für ein Projekt-Profil, das einem anderen Konto gehört.
///
/// Das Projekt-Profil ist ohnehin Angreifer-beeinflusst und darf deshalb nur
/// Schlüssel mit `x-project-scope = "allowed"` setzen. Gehört die Datei einem
/// anderen Konto, hat sie nicht einmal derjenige geschrieben, der das
/// Repository ausgecheckt hat. Das ist eine Warnung wert, aber keine
/// Ablehnung: Die Grenze hält die Datei ohnehin, und ein Start, der daran
/// scheiterte, wäre auf einem geteilten Rechner nicht zu gebrauchen.
fn foreign_owner(path: &Path, uid: u32) -> Option<Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;

    let owner = std::fs::metadata(path).ok()?.uid();
    if owner == uid {
        return None;
    }
    Some(
        Diagnostic::builder(CONFIG_007, Severity::Warning)
            .why(format!(
                "the project profile {} belongs to uid {owner}, not to you (uid {uid}); it may \
                 still set only the keys that x-project-scope allows, but check who put it there",
                path.display()
            ))
            .build(),
    )
}

/// Der Pfad, unter dem ein Profil Einhängungen zu nennen versuchen könnte.
///
/// Es gibt ihn im Schema nicht: Einhängungen stehen im Sandbox-Profil unter
/// `profiles/sandbox/` und werden dort gegen die [`crate::scope`]-Grenze
/// geprüft (HUM-010). Der Name wird hier trotzdem erkannt, damit die Absicht
/// aus einem Projekt-Profil eine klare Antwort bekommt.
const MOUNT_PREFIX: &str = "sandbox.mounts";

/// Ob ein Pfad eine Einhängung meint.
fn is_mount_key(path: &str) -> bool {
    path == MOUNT_PREFIX || path.starts_with(&format!("{MOUNT_PREFIX}."))
}

/// Ein Projekt-Profil, das Host-Pfade in die Sandbox holen will.
///
/// Ohne `FixAction`, aus demselben Grund wie [`project_scope_denied`]: ein Knopf
/// trüge den Wunsch des Angreifers mit einem Klick in die globale
/// Konfiguration.
fn project_mount_refused(entry: &Entry) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{} (from {}) tries to mount host paths from a project profile. Only global \
             profiles may do that: mounts live in the sandbox profile under profiles/sandbox/, \
             and a cloned repository does not get to bring host paths into the sandbox.",
            entry.written_as, entry.origin
        ))
        .build()
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
    Diagnostic::builder(CONFIG_003, Severity::Error)
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
        let hint =
            nearest(&entry.path).map_or_else(String::new, |near| format!("; did you mean {near}?"));
        format!(
            "{} (from {}) is not a key of the schema{hint}",
            entry.written_as, entry.origin
        )
    };
    let mut builder = Diagnostic::builder(CONFIG_002, severity).why(why);
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

    if let Some(int) = entry.value.as_integer()
        && (field.minimum.is_some_and(|min| int < min)
            || field.maximum.is_some_and(|max| int > max))
    {
        let range = match (field.minimum, field.maximum) {
            (Some(min), Some(max)) => format!("{min} to {max}"),
            (Some(min), None) => format!("{min} or more"),
            (None, Some(max)) => format!("{max} or less"),
            (None, None) => "a number".to_owned(),
        };
        return Err(bad_value(entry, &range, &int.to_string()));
    }

    Ok(())
}

fn bad_value(entry: &Entry, expected: &str, found: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{} (from {}) expects {expected}, found {found}",
            entry.written_as, entry.origin
        ))
        .fix(FixAction::ChangeSetting {
            key: entry.path.clone(),
            value: schema::field(&entry.path)
                .map_or_else(|| "-".to_owned(), schema::Field::default_literal),
        })
        .build()
}

/// Befunde zu alten Namen: ein Hinweis (`CONFIG_005`), wenn einer benutzt
/// wird, eine Warnung (`CONFIG_006`), wenn alter und neuer Name nebeneinander
/// stehen.
///
/// Wer gewonnen hat, steht in der Meldung, und zwar richtig: innerhalb einer
/// Ebene der neue Name, über Ebenen hinweg die höhere Ebene, auch wenn dort
/// der alte Name steht. Verglichen wird der Index der Ebene, nicht
/// [`Origin::rank`]: der Rang fasst die beiden Profil-Ebenen zu einem Band
/// zusammen und benennte in der Mischung „eigenes `default.toml` plus
/// eingebettetes `llm-only`" den falschen Gewinner. Die Ebenen werden in
/// Reihenfolge aufgelegt, darum ist der letzte Eintrag je Liste der höchste.
fn alias_diagnostics(
    alias_uses: &BTreeMap<String, Vec<AliasUse>>,
    canonical_uses: &BTreeMap<String, Vec<(usize, Origin)>>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (path, uses) in alias_uses {
        let Some(AliasUse {
            written_as,
            origin,
            value,
            layer,
        }) = uses.last()
        else {
            continue;
        };
        let Some((canonical_layer, canonical)) =
            canonical_uses.get(path).and_then(|places| places.last())
        else {
            out.push(
                Diagnostic::builder(CONFIG_005, Severity::Info)
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
        let why = if layer > canonical_layer {
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
            Diagnostic::builder(CONFIG_006, Severity::Warning)
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
                return Err(Diagnostic::builder(CONFIG_002, Severity::Error)
                    .why(format!(
                        "{path} passes through {segment}, which holds a value"
                    ))
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

    use std::collections::BTreeMap;

    use toml::Value as TomlValue;

    use super::{AliasUse, alias_diagnostics, nearest, scalar};
    use crate::origin::Origin;

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

    /// Welche von zwei Ebenen gewonnen hat, sagt der Ebenen-Index, nicht
    /// [`Origin::rank`].
    ///
    /// Die Mischung: ein eigenes `default.toml` auf Ebene 3 und ein
    /// eingebettetes `llm-only` auf Ebene 4. Der Rang fasst beide zu einem
    /// Band zusammen und könnte hier nur raten; die Meldung muss trotzdem den
    /// richtigen Gewinner nennen, denn sie sagt dem Nutzer, welche Zeile er
    /// löschen soll.
    #[test]
    fn the_layer_index_names_the_winner_between_two_profile_layers() {
        let alias_uses = BTreeMap::from([(
            "limits.hold_body_cap_bytes".to_owned(),
            vec![AliasUse {
                written_as: "hold.body_cap_bytes".to_owned(),
                origin: Origin::ProfileBuiltin("llm-only".to_owned()),
                value: TomlValue::Integer(4096),
                layer: 3,
            }],
        )]);
        let canonical_uses = BTreeMap::from([(
            "limits.hold_body_cap_bytes".to_owned(),
            vec![(2, Origin::ProfileGlobal("default".to_owned()))],
        )]);

        let out = alias_diagnostics(&alias_uses, &canonical_uses);
        assert_eq!(out.len(), 1);
        assert!(
            out[0].why.contains("hold.body_cap_bytes wins"),
            "the higher layer wins, whatever the band says: {}",
            out[0].why
        );

        // Und umgekehrt, mit denselben Bändern und getauschten Ebenen.
        let alias_uses = BTreeMap::from([(
            "limits.hold_body_cap_bytes".to_owned(),
            vec![AliasUse {
                written_as: "hold.body_cap_bytes".to_owned(),
                origin: Origin::ProfileGlobal("default".to_owned()),
                value: TomlValue::Integer(4096),
                layer: 2,
            }],
        )]);
        let canonical_uses = BTreeMap::from([(
            "limits.hold_body_cap_bytes".to_owned(),
            vec![(3, Origin::ProfileBuiltin("llm-only".to_owned()))],
        )]);
        let out = alias_diagnostics(&alias_uses, &canonical_uses);
        assert!(
            out[0].why.contains("limits.hold_body_cap_bytes wins"),
            "{}",
            out[0].why
        );
    }

    #[test]
    fn a_typo_finds_its_neighbour() {
        assert_eq!(nearest("hold.timeoutt_secs"), Some("hold.timeout_secs"));
        assert_eq!(nearest("nothing.like.a.key"), None);
    }
}
