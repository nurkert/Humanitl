//! Profile: was eine Sitzung ausmacht, in einer Datei.
//!
//! Ein Profil bündelt die Konfigurationswerte einer Sitzung und die Regeln, die
//! zu ihr gehören (ADR-011, `BACKLOG.md` Abschnitt 6). Es liegt global unter
//! `$XDG_CONFIG_HOME/humanitl/profiles/<name>.toml` oder im Projekt unter
//! `<projekt>/.humanitl/profile.toml`; zwei Profile, `default` und `llm-only`,
//! sind zusätzlich in das Binary eingebettet, damit der Daemon auch ohne
//! ausgelieferte Dateien startet.
//!
//! Aufbau einer Profildatei:
//!
//! ```toml
//! name = "llm-only"
//! description = "Pure inference."
//!
//! [config.hold]          # Konfigurationswerte, gruppenweise
//! ask_mode = "none"
//!
//! [rules]                # Regeln des Profils
//! files = ["team.yaml"]  # relativ zur Profildatei
//! inline = [{ action = "block", match = { host = "**" } }]
//! ```
//!
//! Auf der obersten Ebene gibt es genau vier Schlüssel: `name`, `description`,
//! `[config]` und `[rules]`. Eine Gruppe wie `[hold]` dort oben ist ein
//! häufiger Fehler und deshalb `CONFIG_002` mit dem Hinweis, dass sie unter
//! `[config.hold]` gehört (`backlog/CONVENTIONS.md` 4.11); sonst bliebe ein
//! Profil ohne Wirkung und ohne Meldung.
//!
//! Das Projekt-Profil ist die eine Ebene, die aus einem geklonten Repository
//! kommt. Es darf deshalb weder Regeln mitbringen noch einen Schlüssel setzen,
//! den das Schema mit `x-project-scope = "denied"` führt; beides ist
//! `CONFIG_003`. Die Prüfung der Schlüssel steht in [`mod@crate::load`], die der
//! Regeln hier.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{CONFIG_001, CONFIG_002, CONFIG_003};
use humanitl_core::{Diagnostic, FixAction, Severity};
use toml::Value as TomlValue;

use crate::load::{PROFILE_SECTION, overlay_from_table};
use crate::origin::Origin;
use crate::schema;

/// Der Block, in dem ein Profil seine Regeln nennt.
pub const PROFILE_RULES_SECTION: &str = "rules";

/// Die Schlüssel, die ein Profil auf seiner obersten Ebene haben darf.
pub const PROFILE_KEYS: &[&str] = &[
    "name",
    "description",
    PROFILE_SECTION,
    PROFILE_RULES_SECTION,
];

/// Die längste erlaubte Länge eines Profilnamens.
const NAME_MAX: usize = 32;

/// Der Name des Profils, das immer gilt.
pub const DEFAULT_PROFILE: &str = "default";

/// Die mitgelieferten Profile, mit ihrem Text.
///
/// Sie liegen zweimal vor: als Datei unter `profiles/` im Auslieferungsumfang
/// und eingebettet hier. Fehlt die Datei, startet der Daemon trotzdem. Legt
/// jemand unter `$XDG_CONFIG_HOME/humanitl/profiles/` eine eigene Fassung mit
/// demselben Namen, gewinnt diese Datei, und das Laden legt einen Hinweis dazu
/// (`CONFIG_008`) — ein stiller Vorzug für eine der beiden Seiten wäre die
/// schlechtere Antwort.
pub const BUILTIN_PROFILES: &[(&str, &str)] = &[
    (
        DEFAULT_PROFILE,
        include_str!("../../../../profiles/default.toml"),
    ),
    (
        "llm-only",
        include_str!("../../../../profiles/llm-only.toml"),
    ),
];

/// Der eingebettete Text eines mitgelieferten Profils.
#[must_use]
pub fn builtin_text(name: &str) -> Option<&'static str> {
    BUILTIN_PROFILES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| *text)
}

/// Der Name eines mitgelieferten Profils, als der `'static` Text der Einbettung.
#[must_use]
pub fn builtin_name(name: &str) -> Option<&'static str> {
    BUILTIN_PROFILES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(known, _)| *known)
}

/// Die Namen der mitgelieferten Profile, in der Reihenfolge der Einbettung.
#[must_use]
pub fn builtin_names() -> Vec<&'static str> {
    BUILTIN_PROFILES.iter().map(|(name, _)| *name).collect()
}

/// Woher der Text eines Profils kommt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSource {
    /// Die eingebettete Fassung eines mitgelieferten Profils.
    Builtin(&'static str),
    /// Eine Profildatei, mit ihrem Pfad.
    File(PathBuf),
    /// Das Profil eines Projekts, `<projekt>/.humanitl/profile.toml`.
    Project(PathBuf),
}

impl ProfileSource {
    /// Der Name, unter dem diese Quelle angesprochen wird.
    ///
    /// Bei einer Datei ist das der Dateiname ohne Endung; das Projekt-Profil
    /// heißt nach seinem Verzeichnis, nicht nach dem Namen, den es sich selbst
    /// gibt — der benennt seine Basis, siehe [`Profile::name`].
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Builtin(name) => (*name).to_owned(),
            Self::File(path) => stem(path),
            Self::Project(path) => path.display().to_string(),
        }
    }

    /// Der Ort, den eine Meldung nennt: der Pfad einer Datei, sonst der Name.
    ///
    /// Getrennt von [`ProfileSource::name`], weil ein Befund den Pfad braucht
    /// („welche Datei, welche Zeile") und die Herkunft den Namen.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Builtin(name) => format!("builtin {name}"),
            Self::File(path) | Self::Project(path) => path.display().to_string(),
        }
    }

    /// Die Ebene, als die ein Wert aus dieser Quelle erscheint.
    #[must_use]
    pub fn origin(&self) -> Origin {
        match self {
            Self::Builtin(name) => Origin::ProfileBuiltin((*name).to_owned()),
            Self::File(path) => Origin::ProfileGlobal(stem(path)),
            Self::Project(path) => Origin::ProfileProject(path.clone()),
        }
    }

    /// Das Verzeichnis, gegen das `[rules].files` aufgelöst wird.
    ///
    /// Für ein eingebettetes Profil gibt es keines: es kann nur Regeln
    /// mitbringen, die in ihm selbst stehen.
    #[must_use]
    pub fn dir(&self) -> Option<PathBuf> {
        match self {
            Self::Builtin(_) => None,
            Self::File(path) | Self::Project(path) => {
                path.parent().map(std::borrow::ToOwned::to_owned)
            }
        }
    }

    /// Liest und prüft das Profil dieser Quelle.
    ///
    /// # Errors
    ///
    /// `CONFIG_001`, wenn die Datei fehlt oder kein gültiges TOML ist,
    /// `CONFIG_002`, wenn sie einen Block hat, den ein Profil nicht kennt, und
    /// `CONFIG_003`, wenn Name oder Regelblock nicht stimmen.
    pub fn load(&self) -> Result<Profile, Diagnostic> {
        let text = match self {
            Self::Builtin(name) => builtin_text(name)
                .ok_or_else(|| {
                    Diagnostic::builder(CONFIG_001, Severity::Error)
                        .why(format!("no profile {name} is built into this binary"))
                        .build()
                })?
                .to_owned(),
            Self::File(path) | Self::Project(path) => {
                std::fs::read_to_string(path).map_err(|err| {
                    Diagnostic::builder(CONFIG_001, Severity::Error)
                        .why(format!("cannot read the profile {}: {err}", path.display()))
                        .build()
                })?
            }
        };
        Profile::parse(&text, self)
    }
}

/// Der Dateiname ohne Endung, als Name eines Profils.
fn stem(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

/// Die Blattpfade, die eine Ebene setzt, mit ihren Werten.
///
/// Kein Spiegel von [`crate::Config`] mit lauter `Option`: Gemischt wird auf
/// Blattpfaden (siehe [`mod@crate::load`]), und ein zweiter, von Hand gepflegter
/// Typ mit denselben vierzig Feldern wäre genau die doppelte Pflege, die
/// ADR-011 ausschließt. Ein Overlay ist deshalb die Menge der Pfade, die eine
/// Ebene wirklich nennt — und damit auch feldweise: `[config.hold]` mit nur
/// `timeout_secs` lässt `ask_mode` in Ruhe.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigOverlay {
    entries: BTreeMap<String, TomlValue>,
}

impl ConfigOverlay {
    /// Ein Overlay aus fertigen Paaren aus Blattpfad und Wert.
    #[must_use]
    pub fn from_entries(entries: BTreeMap<String, TomlValue>) -> Self {
        Self { entries }
    }

    /// Der Wert eines Blattpfades, falls diese Ebene ihn setzt.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&TomlValue> {
        self.entries.get(path)
    }

    /// Ob diese Ebene den Pfad setzt.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    /// Alle Paare aus Blattpfad und Wert, in Pfad-Reihenfolge.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &TomlValue)> {
        self.entries
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    /// Alle Blattpfade, in Pfad-Reihenfolge.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Wie viele Pfade diese Ebene setzt.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Ob diese Ebene nichts setzt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Der Regelblock eines Profils.
///
/// Beide Listen ersetzen, sie hängen nicht an: eine höhere Ebene, die `files`
/// nennt, meint genau diese Dateien. Wer anhängen will, schreibt die Liste
/// vollständig hin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileRules {
    /// Regeldateien, wie sie im Profil stehen.
    pub files: Vec<PathBuf>,
    /// Regeln, die unmittelbar im Profil stehen, im Schema von `rules.yaml`.
    pub inline: Vec<TomlValue>,
}

impl ProfileRules {
    /// Ob das Profil weder Datei noch eigene Regel nennt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.inline.is_empty()
    }
}

/// Ein gelesenes Profil.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Der Name, den das Profil trägt.
    ///
    /// Bei einer Profildatei ist er der Dateiname ohne Endung; ein `name` in
    /// der Datei muss dazu passen. Im Projekt-Profil benennt er stattdessen das
    /// Profil, auf dem das Projekt aufsetzt, und ist dort das einzige, was der
    /// Name tut: eine Datei im Projekt kann `default` nicht mit anderem Inhalt
    /// belegen. Was das Projekt wählen darf, entscheidet [`mod@crate::resolve`].
    pub name: String,
    /// Der `name`, wie er in der Datei steht, falls einer dasteht.
    ///
    /// Getrennt von [`Profile::name`], weil die Auflösung wissen muss, ob ein
    /// Projekt-Profil ein Profil verlangt hat: Ein Wunsch, der übergangen wird,
    /// soll gemeldet werden (`CONFIG_009`), ein fehlender nicht.
    pub declared_name: Option<String>,
    /// Ein Satz darüber, wofür das Profil da ist.
    pub description: Option<String>,
    /// Die Konfigurationswerte aus `[config]`, auf Blattpfade gebracht.
    pub overlay: ConfigOverlay,
    /// Die Regeln aus `[rules]`.
    pub rules: ProfileRules,
    /// Woher das Profil kommt.
    pub source: ProfileSource,
}

impl Profile {
    /// Liest ein Profil aus dem Text einer Profildatei.
    ///
    /// # Errors
    ///
    /// `CONFIG_002` für einen Block, den ein Profil nicht kennt, `CONFIG_001`
    /// für ungültiges TOML und `CONFIG_003` für einen Namen, der nicht zur
    /// Datei passt, oder einen Regelblock, der nicht die erwartete Form hat.
    pub fn parse(text: &str, source: &ProfileSource) -> Result<Self, Diagnostic> {
        let where_ = source.label();
        let table: toml::Table = text.parse().map_err(|err: toml::de::Error| {
            // Die Meldung von `toml` nennt Zeile und Spalte; davor steht die
            // Datei, dahinter, was stattdessen gilt. Nichts gilt: Ein Profil,
            // das nicht lädt, wird nicht halb angewendet und auch nicht still
            // durch das mitgelieferte ersetzt — beides wäre eine Konfiguration,
            // von der niemand weiß, welche sie ist.
            let out = match source {
                ProfileSource::File(path) if builtin_text(&stem(path)).is_some() => format!(
                    "move {} aside to fall back to the bundled profile {}",
                    path.display(),
                    stem(path)
                ),
                _ => "fix the line, then start again".to_owned(),
            };
            Diagnostic::builder(CONFIG_001, Severity::Error)
                .why(format!(
                    "the profile {where_} is not valid TOML: {err}. Nothing starts while it does \
                     not parse, and no level of the configuration silently takes its place; {out}."
                ))
                .build()
        })?;

        for key in table.keys() {
            if PROFILE_KEYS.contains(&key.as_str()) {
                continue;
            }
            return Err(unknown_block(&where_, key));
        }

        let declared = string_at(&table, "name", &where_)?;
        let description = string_at(&table, "description", &where_)?;
        let name = match (&declared, source) {
            (Some(declared), ProfileSource::Builtin(_) | ProfileSource::File(_))
                if *declared != source.name() =>
            {
                return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
                    .why(format!(
                        "the profile file {where_} calls itself {declared}; a profile is named \
                         after its file, so either rename the file or drop the name key"
                    ))
                    .build());
            }
            (Some(declared), _) => declared.clone(),
            (None, ProfileSource::Project(_)) => DEFAULT_PROFILE.to_owned(),
            (None, _) => source.name(),
        };
        // Geprüft wird der Name, der gilt, nicht der, der dasteht: Ohne
        // `name`-Schlüssel kommt er aus dem Dateistamm, und der ist genauso
        // wenig ein Name wie ein getippter. Beide Wege gehen deshalb durch
        // dasselbe Tor, sonst trüge ein `Work.Profile.toml` seinen Stamm
        // ungeprüft durch Auflösung, Herkunft und Profil-Kette.
        check_name(&name, &where_)?;

        let overlay = match table.get(PROFILE_SECTION) {
            None => ConfigOverlay::default(),
            Some(TomlValue::Table(inner)) => ConfigOverlay::from_entries(overlay_from_table(inner)),
            Some(other) => return Err(wrong_type(&where_, PROFILE_SECTION, "a table", other)),
        };
        let rules = match table.get(PROFILE_RULES_SECTION) {
            None => ProfileRules::default(),
            Some(TomlValue::Table(inner)) => parse_rules_block(inner, &where_)?,
            Some(other) => {
                return Err(wrong_type(&where_, PROFILE_RULES_SECTION, "a table", other));
            }
        };

        if matches!(source, ProfileSource::Project(_)) && !rules.is_empty() {
            return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
                .why(format!(
                    "the project profile {where_} brings rules of its own; a file from a cloned \
                     repository must not decide what leaves the sandbox. Move the rules to \
                     rules.yaml or to a global profile."
                ))
                .build());
        }

        Ok(Self {
            name,
            declared_name: declared,
            description,
            overlay,
            rules,
            source: source.clone(),
        })
    }

    /// Die Regeldateien des Profils als Pfade, die man öffnen kann.
    ///
    /// Ein relativer Pfad wird gegen das Verzeichnis der Profildatei aufgelöst,
    /// nie gegen das Arbeitsverzeichnis: ein Profil soll dieselben Dateien
    /// meinen, egal von wo aus Humanitl startet.
    #[must_use]
    pub fn rule_files(&self) -> Vec<PathBuf> {
        let dir = self.source.dir();
        self.rules
            .files
            .iter()
            .map(|file| match (file.is_absolute(), dir.as_ref()) {
                (false, Some(dir)) => dir.join(file),
                _ => file.clone(),
            })
            .collect()
    }

    /// Die Regeln aus `[rules].inline` als Dokument für `humanitl-rules`.
    ///
    /// Das Ergebnis ist JSON und damit gültiges YAML; `humanitl_rules`
    /// verlangt YAML und liest es unverändert. Der Umweg vermeidet eine
    /// Abhängigkeit dieser Crate auf die Regel-Crate, die nach außen zeigte
    /// (`docs/ARCHITECTURE.md` Abschnitt 2). `None`, wenn das Profil keine
    /// eigenen Regeln hat.
    #[must_use]
    pub fn rules_document(&self) -> Option<String> {
        if self.rules.inline.is_empty() {
            return None;
        }
        let document = serde_json::json!({
            "version": crate::profile::RULES_DOCUMENT_VERSION,
            "rules": self.rules.inline,
        });
        Some(document.to_string())
    }

    /// Eine Zeile über das Profil für Listen und Meldungen.
    #[must_use]
    pub fn summary(&self) -> ProfileSummary {
        ProfileSummary {
            name: self.source.name(),
            description: self.description.clone(),
            source: self.source.clone(),
            broken: false,
        }
    }
}

/// Die einzige Fassung des Regel-Dateiformats (`humanitl_rules::RULES_VERSION`).
///
/// Die Zahl steht hier noch einmal, weil diese Crate nicht auf `humanitl-rules`
/// zeigen darf; der Test `the_rules_document_carries_the_version_of_the_parser`
/// hält beide Seiten zusammen.
pub const RULES_DOCUMENT_VERSION: u32 = 1;

/// Name, Beschreibung und Herkunft eines Profils, für `config schema --profiles`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSummary {
    /// Der Name, unter dem `--profile` es wählt.
    pub name: String,
    /// Ein Satz darüber, wofür es da ist.
    pub description: Option<String>,
    /// Woher es kommt.
    pub source: ProfileSource,
    /// Wahr, wenn sich das Profil nicht lesen lässt.
    ///
    /// Eine Liste, die eine kaputte Datei als brauchbares Profil ausgibt, ist
    /// schlimmer als eine, die sie verschweigt: Sie lädt zum Aufruf ein, der
    /// dann mit `CONFIG_001` endet. Verdeckt eine unlesbare Datei ein
    /// mitgeliefertes Profil, steht deshalb die Datei in der Liste und nicht
    /// die Einbettung, die nicht mehr zum Zuge kommt.
    pub broken: bool,
}

impl ProfileSummary {
    /// Die Zeile für ein Profil, das sich nicht lesen lässt.
    #[must_use]
    pub fn broken(source: ProfileSource) -> Self {
        Self {
            name: source.name(),
            description: None,
            source,
            broken: true,
        }
    }
}

/// Prüft einen Profilnamen: `^[a-z0-9-]{1,32}$`.
///
/// Ein Name ist ein Name, kein Pfad. Ein Trenner darin hieße, dass ein Profil
/// aus einem beliebigen Verzeichnis käme; `--profile ../../etc/passwd` wäre
/// dann eine Frage der Bedienung statt eine Unmöglichkeit. Geprüft wird
/// deshalb nicht nur dort, wo ein Mensch tippt, sondern auch an der Stelle, die
/// aus einem Namen einen Pfad macht (`crate::resolve`).
///
/// # Errors
///
/// `CONFIG_003`, wenn der Name leer, zu lang oder außerhalb des Zeichenvorrats
/// ist.
pub fn check_name(name: &str, where_: &str) -> Result<(), Diagnostic> {
    let ok = !name.is_empty()
        && name.len() <= NAME_MAX
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if ok {
        return Ok(());
    }
    Err(Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{name:?} (in {where_}) is not a profile name; a name is one to {NAME_MAX} characters \
             of a-z, 0-9 and -, and never a path"
        ))
        .fix(FixAction::CopyCommand(
            "humanitl config schema --profiles".to_owned(),
        ))
        .build())
}

/// Ein Block, den ein Profil nicht kennt.
fn unknown_block(where_: &str, key: &str) -> Diagnostic {
    let why = if schema::field(key).is_some_and(|field| field.group) {
        format!(
            "[{key}] in the profile {where_} is a group of settings; in a profile it belongs \
             under [{PROFILE_SECTION}.{key}]"
        )
    } else {
        format!(
            "[{key}] in the profile {where_} is not a block of a profile; a profile has name, \
             description, [{PROFILE_SECTION}] for its settings and \
             [{PROFILE_RULES_SECTION}] for its rules"
        )
    };
    Diagnostic::builder(CONFIG_002, Severity::Error)
        .why(why)
        .build()
}

/// Ein Schlüssel mit der falschen Form.
fn wrong_type(where_: &str, key: &str, expected: &str, found: &TomlValue) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{key} in the profile {where_} must be {expected}, found {}",
            found.type_str()
        ))
        .build()
}

/// Ein Textwert der obersten Ebene, falls er dasteht.
fn string_at(table: &toml::Table, key: &str, where_: &str) -> Result<Option<String>, Diagnostic> {
    match table.get(key) {
        None => Ok(None),
        Some(TomlValue::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(wrong_type(where_, key, "a string", other)),
    }
}

/// Der Block `[rules]` eines Profils.
fn parse_rules_block(table: &toml::Table, where_: &str) -> Result<ProfileRules, Diagnostic> {
    for key in table.keys() {
        if key == "files" || key == "inline" {
            continue;
        }
        return Err(Diagnostic::builder(CONFIG_002, Severity::Error)
            .why(format!(
                "[{PROFILE_RULES_SECTION}].{key} in the profile {where_} is not a key of the \
                 rules block; it has files and inline"
            ))
            .build());
    }

    let mut files = Vec::new();
    match table.get("files") {
        None => {}
        Some(TomlValue::Array(items)) => {
            for item in items {
                let TomlValue::String(text) = item else {
                    return Err(wrong_type(where_, "[rules].files", "a list of paths", item));
                };
                files.push(PathBuf::from(text));
            }
        }
        Some(other) => {
            return Err(wrong_type(
                where_,
                "[rules].files",
                "a list of paths",
                other,
            ));
        }
    }

    let mut inline = Vec::new();
    match table.get("inline") {
        None => {}
        Some(TomlValue::Array(items)) => {
            for item in items {
                if !item.is_table() {
                    return Err(wrong_type(
                        where_,
                        "[rules].inline",
                        "a list of rules",
                        item,
                    ));
                }
                inline.push(item.clone());
            }
        }
        Some(other) => {
            return Err(wrong_type(
                where_,
                "[rules].inline",
                "a list of rules",
                other,
            ));
        }
    }

    Ok(ProfileRules { files, inline })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use super::{BUILTIN_PROFILES, DEFAULT_PROFILE, Profile, ProfileSource, builtin_text};

    fn builtin(name: &'static str) -> Profile {
        ProfileSource::Builtin(name)
            .load()
            .unwrap_or_else(|diagnostic| panic!("{name} does not parse: {diagnostic}"))
    }

    #[test]
    fn both_bundled_profiles_parse_and_carry_their_name() {
        assert_eq!(BUILTIN_PROFILES.len(), 2);
        assert_eq!(builtin(DEFAULT_PROFILE).name, "default");
        assert_eq!(builtin("llm-only").name, "llm-only");
        assert!(builtin("llm-only").description.is_some());
        assert!(builtin_text("nowhere").is_none());
    }

    #[test]
    fn the_llm_only_profile_blocks_everything_in_one_inline_rule() {
        let profile = builtin("llm-only");
        assert_eq!(profile.rules.inline.len(), 1);
        let document = profile.rules_document().expect("a rules document");
        assert!(document.contains("\"version\":1"), "{document}");
        assert!(document.contains("\"block\""), "{document}");
        assert!(builtin(DEFAULT_PROFILE).rules_document().is_none());
    }

    #[test]
    fn a_group_on_the_top_level_names_its_place_under_config() {
        let source = ProfileSource::File(PathBuf::from("/p/profiles/work.toml"));
        let diagnostic = Profile::parse("[hold]\ntimeout_secs = 5\n", &source)
            .expect_err("a flat group is refused");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_002");
        assert!(
            diagnostic.why.contains("[config.hold]"),
            "{}",
            diagnostic.why
        );
    }

    #[test]
    fn a_profile_file_is_named_after_its_file() {
        let source = ProfileSource::File(PathBuf::from("/p/profiles/work.toml"));
        let diagnostic = Profile::parse("name = \"other\"\n", &source)
            .expect_err("a mismatching name is refused");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
        assert!(Profile::parse("name = \"work\"\n", &source).is_ok());
    }

    #[test]
    fn a_project_profile_may_not_bring_rules() {
        let source = ProfileSource::Project(PathBuf::from("/p/.humanitl/profile.toml"));
        let text = "[rules]\ninline = [{ action = \"allow\", match = { host = \"**\" } }]\n";
        let diagnostic = Profile::parse(text, &source).expect_err("rules from a repository");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
        assert!(
            diagnostic.why.contains("cloned repository"),
            "{}",
            diagnostic.why
        );
    }

    #[test]
    fn a_rule_file_is_resolved_against_the_profile_not_the_working_directory() {
        let source = ProfileSource::File(PathBuf::from("/p/profiles/work.toml"));
        let profile = Profile::parse(
            "[rules]\nfiles = [\"team.yaml\", \"/etc/x.yaml\"]\n",
            &source,
        )
        .expect("the profile parses");
        assert_eq!(
            profile.rule_files(),
            vec![
                PathBuf::from("/p/profiles/team.yaml"),
                PathBuf::from("/etc/x.yaml"),
            ]
        );
    }

    #[test]
    fn a_name_is_a_name_and_never_a_path() {
        for bad in ["../etc", "Work", "", "a/b", &"x".repeat(33)] {
            assert!(
                super::check_name(bad, "a test").is_err(),
                "{bad:?} was accepted"
            );
        }
        assert!(super::check_name("llm-only", "a test").is_ok());
    }
}
