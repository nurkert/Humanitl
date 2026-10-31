//! Gespeicherte Modellserver unter `llm.servers` (HUM-289).
//!
//! Ein Eintrag `[llm.servers.<name>]` trägt `endpoint`, `models` und
//! `api_key_ref`, also dieselben drei Werte wie die flachen Schlüssel unter
//! `llm`. `llm.active` nennt den Eintrag, der gilt. Die flachen Schlüssel sind
//! seine **Ansicht**: Beim Laden setzt `project` die Werte des aktiven
//! Eintrags an ihre Stelle, und wer einen flachen Schlüssel schreibt, schreibt
//! den Eintrag mit ([`mirror_of`]). Jeder Leser, der heute `llm.endpoint` und
//! `llm.api_key_ref` liest — der Start des Agenten, die mitgelieferte Regel
//! der Durchreiche, die Probe —, folgt damit ohne eigenes Wissen dem aktiven
//! Server, und nur dessen Herkunft bekommt den hinterlegten Schlüssel.
//!
//! Die Aufträge ([`add`], [`rename`], [`remove`], [`activate`],
//! [`set_key_ref`]) sind reine Funktionen über der Tabelle von `config.toml`:
//! Sie lesen den Stand, ändern ein Abbild davon und liefern die Änderungen als
//! Liste für [`crate::edit::edit_with`], das sie unter der Sperre in einem Zug
//! schreibt. Eine Datei ohne Einträge, aber mit flachen Werten, bekommt dabei
//! zuerst den Eintrag [`MIGRATED_NAME`] mit genau diesen Werten, den Verweis
//! auf den Schlüssel eingeschlossen: Er zeigt danach auf denselben Eintrag im
//! Schlüsselbund wie vorher.
//!
//! Ein Wechsel des aktiven Servers ändert nur `config.toml`. Eine laufende
//! Sitzung hat ihre Konfiguration beim Start aufgelöst und behält ihren
//! Server; die nächste löst neu auf (HUM-277).

use std::collections::{BTreeMap, BTreeSet};

use humanitl_core::diagnostics::codes::{CONFIG_002, CONFIG_003, CONFIG_021};
use humanitl_core::{Diagnostic, FixAction, Severity};
use toml::Value as TomlValue;

use crate::edit::Edit;
use crate::origin::Origin;
use crate::secret::KeyRef;

/// Ordnet alle Aufträge an Servern und ihren Schlüsseln in diesem Prozess
/// (HUM-289, Review).
///
/// `LlmServers` und `LlmKey` halten sie über ihren ganzen Ablauf: Lesen des
/// Eintrags, Ablegen oder Löschen des Schlüssels im Speicher und Schreiben
/// des Verweises. Ohne sie könnte zwischen dem Ablegen eines Schlüssels und
/// dem Verweis darauf ein Entfernen und Neuanlegen desselben Namens liegen,
/// und der Schlüssel hinge an einem Server mit anderem Endpunkt. Die Sperre
/// von [`crate::edit`] ordnet nur einzelne Schreibvorgänge; sie wird immer
/// nach dieser genommen, nie davor.
static ORDER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Hält die Sperre über alle Aufträge an Servern und Schlüsseln, solange es
/// lebt.
#[derive(Debug)]
pub struct ServersGuard {
    _order: std::sync::MutexGuard<'static, ()>,
    _dir: std::fs::File,
}

/// Nimmt die Sperre über alle Aufträge an Servern und Schlüsseln, in diesem
/// Prozess und über Prozesse hinweg (HUM-289, Review).
///
/// Erst der Mutex dieses Prozesses, dann eine `flock`-Sperre auf dem
/// Verzeichnis der Schlüssel ([`crate::paths::Paths::key_dir`], `0700`):
/// Der Daemon (`LlmServers`, `LlmKey`, `SetConfig` auf `llm.*`) und eine
/// zweite Kommandozeile (`humanitl config set llm.endpoint …`) ändern einen
/// Endpunkt oder einen Verweis so nie, während ein Schlüssel abgelegt wird.
/// Gesperrt wird das Verzeichnis der Schlüssel und nicht das von
/// `config.toml`: Das sperrt [`crate::edit`] je Schreibvorgang, und dieselbe
/// Sperre zweimal zu nehmen, hielte den Prozess an sich selbst fest.
///
/// # Errors
///
/// `CONFIG_015`, wenn sich das Verzeichnis nicht anlegen, öffnen oder
/// sperren lässt; dann wird nichts geändert.
pub fn lock(paths: &crate::paths::Paths) -> Result<ServersGuard, Diagnostic> {
    use std::os::unix::fs::DirBuilderExt as _;

    let order = ORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = paths.key_dir();
    let refuse = |what: &str, err: &dyn std::fmt::Display| {
        Diagnostic::builder(
            humanitl_core::diagnostics::codes::CONFIG_015,
            Severity::Error,
        )
        .why(format!(
            "the model servers were not changed: {} {what} ({err})",
            dir.display()
        ))
        .build()
    };
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(crate::paths::DIR_MODE)
        .create(&dir)
        .map_err(|err| refuse("could not be created", &err))?;
    let handle = std::fs::File::open(&dir).map_err(|err| refuse("could not be opened", &err))?;
    rustix::fs::flock(&handle, rustix::fs::FlockOperation::LockExclusive)
        .map_err(|err| refuse("could not be locked against other writers", &err))?;
    Ok(ServersGuard {
        _order: order,
        _dir: handle,
    })
}

/// Ob ein Schlüssel von `config.toml` zu den Servern gehört: die flache
/// Ansicht und die Tabellen selbst. Wer ihn schreibt, nimmt [`lock`].
#[must_use]
pub fn is_server_setting(path: &str) -> bool {
    path == SERVERS
        || path == ACTIVE
        || path.starts_with("llm.servers.")
        || FIELDS.iter().any(|field| path == format!("llm.{field}"))
}

/// Der Pfad der Tabelle der Einträge.
pub const SERVERS: &str = "llm.servers";

/// Der Pfad des Namens des aktiven Eintrags.
pub const ACTIVE: &str = "llm.active";

/// Der Pfad des Namens, mit dem ein Projekt, ein Profil oder ein einzelner
/// Lauf einen der gespeicherten Server wählt (HUM-400).
///
/// Er wählt nur aus `llm.servers`: Endpunkt, Modelle und Schlüssel kommen
/// immer aus dem Eintrag, den der Nutzer selbst gespeichert hat. Gesetzt,
/// geht er `llm.active` vor; ein Name ohne Eintrag lässt keinen Endpunkt und
/// keinen Schlüssel gelten, und der Start einer Sitzung lehnt dann ab
/// ([`ServerChoice::refusal`]).
pub const SERVER: &str = "llm.server";

/// Woher die Wahl des Servers einer Sitzung kommt (HUM-400).
///
/// Die Rangfolge ist die der Ebenen beim Laden: ein Lauf vor dem Projekt,
/// das Projekt vor einem Profil, ein Profil vor `llm.active`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerSource {
    /// Dieser eine Lauf: `humanitl run --llm NAME`, `Start.llm_server`, eine
    /// Umgebungsvariable oder ein Argument der Kommandozeile.
    Run,
    /// Das Projekt-Profil, mit seinem Pfad.
    Project(std::path::PathBuf),
    /// Ein Profil der Sitzung (`profiles/*.toml`), mit seinem Namen.
    Profile(String),
    /// `llm.server` in `config.toml`: eine Vorgabe des Nutzers wie
    /// `llm.active`, aber ein anderer Schlüssel, und der Befund nennt ihn.
    Config,
    /// Die Vorgabe des Nutzers: `llm.active`.
    Default,
}

impl ServerSource {
    /// Die Quelle zur Ebene, aus der `llm.server` kommt.
    #[must_use]
    pub fn of(origin: &Origin) -> Self {
        match origin {
            Origin::Cli | Origin::Env(_) => Self::Run,
            Origin::ProfileProject(path) => Self::Project(path.clone()),
            Origin::ProfileBuiltin(name) | Origin::ProfileGlobal(name) => {
                Self::Profile(name.clone())
            }
            Origin::Global => Self::Config,
            Origin::Default => Self::Default,
        }
    }

    /// Der Kurzname: `run`, `project`, `profile` oder `default`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Project(_) => "project",
            Self::Profile(_) => "profile",
            Self::Config | Self::Default => "default",
        }
    }
}

impl std::fmt::Display for ServerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Run => f.write_str("this run"),
            Self::Project(path) => write!(f, "the project profile {}", path.display()),
            Self::Profile(name) => write!(f, "the profile {name}"),
            Self::Config => write!(f, "{SERVER} in config.toml"),
            Self::Default => f.write_str("llm.active"),
        }
    }
}

/// Der gespeicherte Server, den eine Auflösung gewählt hat, und woher die
/// Wahl kommt (HUM-400).
///
/// Steht in [`crate::LlmConfig::choice`], solange Endpunkt und Schlüssel aus
/// einem gespeicherten Eintrag kommen oder ein gewählter Name keinen nennt;
/// fehlt, wenn kein Server gilt oder eine höhere Ebene `llm.endpoint` selbst
/// setzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerChoice {
    /// Der Name des Servers.
    pub name: String,
    /// Woher der Name kommt.
    pub source: ServerSource,
    /// Ob unter dem Namen ein Server gespeichert ist. Falsch heißt: kein
    /// Endpunkt, keine Modelle und kein Schlüssel gelten, und eine Sitzung
    /// startet nicht.
    pub known: bool,
    /// Die Namen der gespeicherten Server, für den Befund.
    pub saved: Vec<String>,
}

impl ServerChoice {
    /// Der Befund, mit dem ein Start ablehnt, wenn der gewählte Name keinen
    /// gespeicherten Server nennt; `None`, wenn es ihn gibt.
    ///
    /// Nie ein stiller Rückfall auf `llm.active`: Wer im Projekt oder im
    /// Profil einen Server nennt, will genau diesen, und ein anderer bekäme
    /// sonst den Verkehr des Modells, ohne dass jemand es sähe.
    #[must_use]
    pub fn refusal(&self) -> Option<Diagnostic> {
        if self.known {
            return None;
        }
        let saved = if self.saved.is_empty() {
            "none".to_owned()
        } else {
            self.saved.join(", ")
        };
        // Ein Name, der keiner sein kann, steht in Anführungszeichen: Er kam
        // vielleicht aus einem geklonten Repository.
        let name = if check_name(&self.name).is_ok() {
            self.name.clone()
        } else {
            format!("{:?}", self.name)
        };
        let remove = match &self.source {
            ServerSource::Run => "leave out --llm".to_owned(),
            ServerSource::Project(path) => format!("remove {SERVER} from {}", path.display()),
            ServerSource::Profile(name) => format!("remove {SERVER} from the profile {name}"),
            ServerSource::Config | ServerSource::Default => {
                format!("remove {SERVER} from config.toml")
            }
        };
        Some(
            Diagnostic::builder(CONFIG_021, Severity::Error)
                .why(format!(
                    "{} names the model server {name} in {SERVER}, and no server of that name is \
                     saved (saved: {saved}); the session does not start, and no endpoint gets a \
                     key. Save it with humanitl llm server add, or {remove}",
                    self.source
                ))
                .fix(FixAction::CopyCommand(if check_name(&self.name).is_ok() {
                    format!("humanitl llm server add {name} URL")
                } else {
                    "humanitl llm server ls".to_owned()
                }))
                .build(),
        )
    }
}

/// Was [`project`] über den gemischten Stand hinaus liefert.
#[derive(Debug, Default)]
pub(crate) struct Projection {
    /// Die Befunde des Projizierens.
    pub diagnostics: Vec<Diagnostic>,
    /// Der gewählte Server, wenn einer gilt oder ein gewählter Name keinen
    /// nennt.
    pub choice: Option<ServerChoice>,
}

/// Der Name des Eintrags, zu dem eine Datei mit flachen Werten wird.
pub const MIGRATED_NAME: &str = "default";

/// Die Felder eines Eintrags, die zugleich flache Schlüssel unter `llm` sind.
pub const FIELDS: [&str; 3] = ["endpoint", "models", "api_key_ref"];

/// Ein Eintrag, wie er in `config.toml` steht.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerEntry {
    /// Der Name, unter dem der Eintrag steht.
    pub name: String,
    /// Der Endpunkt als Text, wie er dasteht.
    pub endpoint: Option<String>,
    /// Die Modelle, das voreingestellte zuerst.
    pub models: Vec<String>,
    /// Der Verweis auf den Zugangsschlüssel, wie er dasteht.
    pub api_key_ref: Option<String>,
}

/// Die Einträge einer `config.toml` und welcher aktiv ist.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerList {
    /// Die Einträge, nach Namen sortiert.
    pub servers: Vec<ServerEntry>,
    /// Der Name des aktiven Eintrags; `None`, wenn keiner aktiv ist.
    pub active: Option<String>,
    /// Wahr, wenn die Datei noch keine Einträge hat und die Liste den
    /// Eintrag [`MIGRATED_NAME`] aus den flachen Werten ableitet. Der erste
    /// schreibende Auftrag legt ihn an.
    pub implicit: bool,
}

/// Prüft den Namen eines Eintrags.
///
/// Dieselbe Form wie der Name eines Schlüssels im Schlüsselbund
/// (`secret-service:humanitl/<name>`, unter macOS
/// `keychain:humanitl/<name>`), weil ein neuer Eintrag seinen Schlüssel unter
/// seinem Namen ablegt.
///
/// # Errors
///
/// `CONFIG_021`, wenn der Name keiner ist.
pub fn check_name(name: &str) -> Result<(), Diagnostic> {
    if crate::secret::is_key_name(name) {
        return Ok(());
    }
    Err(refused(&format!(
        "{name:?} is not a server name: a name has 1 to 32 characters a-z, 0-9, - and _"
    )))
}

/// Die Einträge, wie `config.toml` sie trägt.
///
/// Ohne Einträge, aber mit einem flachen Endpunkt, Modell oder Verweis auf
/// einen Schlüssel, steht dort ein abgeleiteter Eintrag [`MIGRATED_NAME`]
/// (`implicit`), damit die Oberfläche von Anfang an einen Server zeigt.
#[must_use]
pub fn list(table: &toml::Table) -> ServerList {
    let state = State::read(table);
    let implicit = !state.had_servers && !state.servers.is_empty();
    ServerList {
        servers: state
            .servers
            .iter()
            .map(|(name, entry)| entry_view(name, entry))
            .collect(),
        active: state.active,
        implicit,
    }
}

/// Legt einen Eintrag an; mit `activate` oder als erster Eintrag wird er
/// zugleich aktiv.
///
/// # Errors
///
/// `CONFIG_021` für einen ungültigen oder vergebenen Namen und für einen
/// leeren Endpunkt; `CONFIG_003` für einen Endpunkt, dessen Text
/// [`crate::llm_endpoint::check_text`] ablehnt oder der keine Adresse mit
/// `http` oder `https` ist.
pub fn add(
    table: &toml::Table,
    name: &str,
    endpoint: &str,
    models: &[String],
    activate: bool,
) -> Result<Vec<Edit>, Diagnostic> {
    check_name(name)?;
    check_endpoint(name, endpoint)?;
    let mut state = State::read(table);
    if state.servers.contains_key(name) {
        return Err(refused(&format!(
            "a server named {name} exists already; choose another name or remove it first"
        )));
    }
    let mut entry = toml::Table::new();
    entry.insert(
        "endpoint".to_owned(),
        TomlValue::String(endpoint.to_owned()),
    );
    let models: Vec<TomlValue> = models
        .iter()
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
        .map(|model| TomlValue::String(model.to_owned()))
        .collect();
    if !models.is_empty() {
        entry.insert("models".to_owned(), TomlValue::Array(models));
    }
    state.servers.insert(name.to_owned(), entry);
    if activate || state.active.is_none() {
        state.activate(name);
    }
    Ok(state.edits(table))
}

/// Benennt einen Eintrag um.
///
/// Der Verweis auf den Schlüssel bleibt, wie er ist: Der Eintrag im
/// Schlüsselbund behält seinen Namen, und kein Schlüssel muss dafür gelesen
/// und neu geschrieben werden. Ist der Eintrag aktiv, nennt `llm.active`
/// danach den neuen Namen.
///
/// # Errors
///
/// `CONFIG_021` für einen ungültigen neuen Namen, einen unbekannten alten
/// oder einen vergebenen neuen.
pub fn rename(table: &toml::Table, name: &str, new_name: &str) -> Result<Vec<Edit>, Diagnostic> {
    check_name(new_name)?;
    let mut state = State::read(table);
    let entry = state.take(name)?;
    if state.servers.contains_key(new_name) {
        return Err(refused(&format!(
            "a server named {new_name} exists already; choose another name"
        )));
    }
    state.servers.insert(new_name.to_owned(), entry);
    if state.active.as_deref() == Some(name) {
        state.active = Some(new_name.to_owned());
    }
    Ok(state.edits(table))
}

/// Was [`remove`] tut, neben den Änderungen an der Datei.
#[derive(Debug, Clone, PartialEq)]
pub struct Removal {
    /// Die Änderungen für [`crate::edit::edit_with`].
    pub edits: Vec<Edit>,
    /// Der Verweis des entfernten Eintrags, wenn kein anderer Eintrag
    /// denselben trägt; nur dann darf der Schlüssel dahinter gelöscht werden.
    pub orphaned_key: Option<KeyRef>,
}

/// Entfernt einen Eintrag.
///
/// Der aktive Eintrag geht nur mit einem Nachfolger (`replacement`), der
/// danach aktiv ist, solange es andere Einträge gibt. Ist er der letzte,
/// fallen mit ihm `llm.active` und die flachen Schlüssel weg: Danach ist
/// kein Server eingetragen, und kein Schlüssel geht irgendwohin.
///
/// # Errors
///
/// `CONFIG_021` für einen unbekannten Namen, einen unbekannten Nachfolger,
/// einen Nachfolger, der der Eintrag selbst ist, und für den aktiven Eintrag
/// ohne Nachfolger, solange es andere gibt.
pub fn remove(
    table: &toml::Table,
    name: &str,
    replacement: Option<&str>,
) -> Result<Removal, Diagnostic> {
    let mut state = State::read(table);
    let entry = state.take(name)?;
    let replacement = replacement.filter(|next| !next.is_empty());
    if let Some(next) = replacement {
        if next == name {
            return Err(refused(&format!(
                "{name} cannot replace itself; name another server to use after removing it"
            )));
        }
        if !state.servers.contains_key(next) {
            return Err(unknown(next, &state));
        }
    }
    if state.active.as_deref() == Some(name) {
        match replacement {
            Some(next) => state.activate(next),
            None if state.servers.is_empty() => {
                state.active = None;
                state.flat = toml::Table::new();
            }
            None => {
                return Err(Diagnostic::builder(CONFIG_021, Severity::Error)
                    .why(format!(
                        "{name} is the active server; name the server to use instead, so that \
                         the next session does not start without one (left: {})",
                        names(&state)
                    ))
                    .fix(FixAction::CopyCommand(format!(
                        "humanitl llm server rm {name} --use {}",
                        state.servers.keys().next().map_or("NAME", String::as_str)
                    )))
                    .build());
            }
        }
    } else if let Some(next) = replacement {
        state.activate(next);
    }
    let key_ref = entry
        .get("api_key_ref")
        .and_then(TomlValue::as_str)
        .and_then(KeyRef::parse);
    let orphaned_key = key_ref.filter(|key_ref| !state.uses_key(key_ref));
    Ok(Removal {
        edits: state.edits(table),
        orphaned_key,
    })
}

/// Macht einen Eintrag zum aktiven: `llm.active` und die drei flachen
/// Schlüssel, in einem Zug.
///
/// # Errors
///
/// `CONFIG_021` für einen unbekannten Namen; `CONFIG_003`, wenn der Endpunkt
/// des Eintrags einen Text trägt, den [`crate::llm_endpoint::check_text`]
/// ablehnt.
pub fn activate(table: &toml::Table, name: &str) -> Result<Vec<Edit>, Diagnostic> {
    let mut state = State::read(table);
    let Some(entry) = state.servers.get(name) else {
        return Err(unknown(name, &state));
    };
    // Ein Eintrag ohne brauchbaren Endpunkt wird nicht aktiv: Der nächste
    // Start hätte sonst keinen Server, und der Schlüssel kein Ziel.
    check_endpoint(
        name,
        entry
            .get("endpoint")
            .and_then(TomlValue::as_str)
            .unwrap_or_default(),
    )?;
    state.activate(name);
    Ok(state.edits(table))
}

/// Der Name, unter dem der Schlüssel eines Eintrags im Speicher liegt.
///
/// Trägt der Eintrag schon einen Verweis, bleibt dessen Name: Ein neues
/// Setzen überschreibt denselben Eintrag im Schlüsselbund, und der
/// übernommene Eintrag [`MIGRATED_NAME`] zeigt weiter auf `humanitl/llm`.
/// Sonst ist es der Name des Eintrags, und wenn ein anderer Eintrag diesen
/// Namen schon für seinen Schlüssel benutzt, der erste freie mit `-2`, `-3`
/// und so weiter. Zwei Einträge teilen sich so nie einen Schlüssel.
///
/// # Errors
///
/// `CONFIG_021`, wenn es den Eintrag nicht gibt.
pub fn key_name_for(table: &toml::Table, name: &str) -> Result<String, Diagnostic> {
    let state = State::read(table);
    let Some(entry) = state.servers.get(name) else {
        return Err(unknown(name, &state));
    };
    if let Some(own) = entry
        .get("api_key_ref")
        .and_then(TomlValue::as_str)
        .and_then(KeyRef::parse)
    {
        // Ein Verweis, den ein anderer Eintrag auch trägt, wird nicht
        // beschrieben: Der neue Schlüssel gälte sonst für zwei Herkünfte.
        if let Some(other) = state.holder_of(&own, name) {
            return Err(refused(&format!(
                "the servers {name} and {other} refer to the same access key {own}; give \
                 {name} a key of its own by removing its api_key_ref by hand first"
            )));
        }
        return Ok(own.name().to_owned());
    }
    let taken: BTreeSet<String> = state
        .servers
        .iter()
        .filter(|(other, _)| other.as_str() != name)
        .filter_map(|(_, entry)| entry.get("api_key_ref").and_then(TomlValue::as_str))
        .filter_map(KeyRef::parse)
        .map(|key_ref| key_ref.name().to_owned())
        .collect();
    if !taken.contains(name) {
        return Ok(name.to_owned());
    }
    // Endlich: Es gibt höchstens so viele vergebene Namen wie Einträge, also
    // ist unter den ersten `taken.len() + 1` Nummern eine frei.
    (2_usize..taken.len() + 3)
        .map(|n| {
            let suffix = format!("-{n}");
            let keep = 32_usize.saturating_sub(suffix.len()).min(name.len());
            format!("{}{suffix}", &name[..keep])
        })
        .find(|candidate| !taken.contains(candidate))
        .ok_or_else(|| refused("no free key name is left"))
}

/// Richtet den Verweis eines Eintrags auf einen Schlüssel, oder nimmt ihn
/// heraus; beim aktiven Eintrag auch `llm.api_key_ref`.
///
/// # Errors
///
/// `CONFIG_021`, wenn es den Eintrag nicht gibt.
pub fn set_key_ref(
    table: &toml::Table,
    name: &str,
    key_ref: Option<&KeyRef>,
) -> Result<Vec<Edit>, Diagnostic> {
    let mut state = State::read(table);
    let value = key_ref.map(|key_ref| TomlValue::String(key_ref.to_string()));
    let is_active = state.active.as_deref() == Some(name);
    if !state.servers.contains_key(name) {
        return Err(unknown(name, &state));
    }
    if let Some(key_ref) = key_ref
        && let Some(other) = state.holder_of(key_ref, name)
    {
        return Err(refused(&format!(
            "the access key {key_ref} belongs to the server {other}; {name} gets a key of its own"
        )));
    }
    let Some(entry) = state.servers.get_mut(name) else {
        return Err(unknown(name, &state));
    };
    put(entry, "api_key_ref", value.clone());
    if is_active {
        put(&mut state.flat, "api_key_ref", value);
    }
    Ok(state.edits(table))
}

/// Der Verweis auf den Schlüssel eines Eintrags, wie er in der Datei steht.
///
/// # Errors
///
/// `CONFIG_021`, wenn es den Eintrag nicht gibt.
pub fn key_ref_of(table: &toml::Table, name: &str) -> Result<Option<KeyRef>, Diagnostic> {
    let state = State::read(table);
    let Some(entry) = state.servers.get(name) else {
        return Err(unknown(name, &state));
    };
    Ok(entry
        .get("api_key_ref")
        .and_then(TomlValue::as_str)
        .and_then(KeyRef::parse))
}

/// Ob ein anderer Eintrag als `except` auf den Schlüssel `key_ref` zeigt.
#[must_use]
pub fn shared_by_other(table: &toml::Table, key_ref: &KeyRef, except: &str) -> bool {
    State::read(table).holder_of(key_ref, except).is_some()
}

/// Ob die Datei Einträge trägt (und nicht nur flache Werte).
#[must_use]
pub fn has_servers(table: &toml::Table) -> bool {
    State::read(table).had_servers
}

/// Der Name des aktiven Eintrags, wie die Datei ihn nennt.
#[must_use]
pub fn active_name(table: &toml::Table) -> Option<String> {
    let state = State::read(table);
    state.active.filter(|name| state.servers.contains_key(name))
}

/// Die Änderung am aktiven Eintrag, die zu einer Änderung an einem flachen
/// Schlüssel gehört.
///
/// `llm.endpoint`, `llm.models` und `llm.api_key_ref` sind die Ansicht des
/// aktiven Eintrags. Schreibt jemand einen davon — das Formular, `config
/// set`, `LlmKey` ohne Servernamen —, geht derselbe Wert in den Eintrag, den
/// `llm.active` nennt. Ohne Einträge oder ohne aktiven Eintrag: nichts.
#[must_use]
pub fn mirror_of(table: &toml::Table, segments: &[String], value: Option<&TomlValue>) -> Vec<Edit> {
    let [group, field] = segments else {
        return Vec::new();
    };
    if group != "llm" || !FIELDS.contains(&field.as_str()) {
        return Vec::new();
    }
    let state = State::read(table);
    if !state.had_servers {
        return Vec::new();
    }
    let Some(active) = state.active.filter(|name| state.servers.contains_key(name)) else {
        return Vec::new();
    };
    vec![Edit::new(
        vec![
            "llm".to_owned(),
            "servers".to_owned(),
            active,
            field.clone(),
        ],
        value.cloned(),
    )]
}

/// Setzt beim Laden die Werte des gewählten Eintrags an die Stelle der
/// flachen Schlüssel (HUM-289, HUM-400) und hält dabei die eine Regel des
/// Schlüssels: Ein Verweis wirkt nur zusammen mit dem Endpunkt **desselben**
/// Eintrags.
///
/// Gewählt ist der Eintrag, den [`SERVER`] nennt, sonst der, den [`ACTIVE`]
/// nennt. Welche Ebene den Namen gesetzt hat, sagt die Quelle der Wahl
/// ([`ServerSource`]); der Rang der Ebenen ist die Rangfolge Lauf, Projekt,
/// Profil, Vorgabe. Der Name, der gilt, heißt unten der *Wähler*.
///
/// `flat` ist der gemischte Stand aller Ebenen, Blattpfad für Blattpfad;
/// `layer_of` sagt, in welcher Ebene ein Pfad zuletzt gesetzt wurde (oder
/// beiseitegelegt, HUM-283).
///
/// - Ein flacher Schlüssel aus einer Ebene über der des Wählers (Umgebung,
///   Kommandozeile) bleibt stehen. Kommen Endpunkt und Verweis danach nicht
///   mehr aus einem Eintrag, fällt der Verweis weg: Der hinterlegte Schlüssel
///   geht dann nirgendwohin (`CONFIG_021`, Warnung). Ohne Einträge gilt
///   dasselbe für die flachen Schlüssel untereinander.
/// - Nennt [`SERVER`] keinen Eintrag, fallen Endpunkt, Modelle und Verweis
///   ganz weg (`CONFIG_021`, Warnung), und die Wahl steht mit
///   `known = false` da: Der Start einer Sitzung lehnt dann ab, statt still
///   `llm.active` zu nehmen.
/// - Gibt es Einträge, aber keinen aktiven (`llm.active` fehlt oder nennt
///   keinen), oder nennt `llm.active` einen Eintrag ohne Einträge, fallen
///   Endpunkt, Modelle und Verweis ganz weg (`CONFIG_021`, Warnung).
/// - Tragen zwei Einträge denselben Verweis, wirkt er bei keinem
///   (`CONFIG_021`, Warnung): Sonst ginge ein Schlüssel an zwei Herkünfte.
/// - Jeder Endpunkt wird voll geprüft (Text nach HUM-283, Adresse, Schema
///   `http` oder `https`, Host). Beim gewählten Eintrag gilt ein abgelehnter
///   als nicht gesetzt, bei einem anderen ist er eine Warnung, und der
///   Eintrag lässt sich erst nach der Korrektur aktivieren.
///
/// # Errors
///
/// `CONFIG_002`, wenn ein Eintrag einen Schlüssel trägt, den es dort nicht
/// gibt, etwa `model` statt `models`; `CONFIG_003`, wenn ein Eintrag keine
/// Tabelle ist.
pub(crate) fn project(
    flat: &mut BTreeMap<String, TomlValue>,
    origins: &mut BTreeMap<String, Origin>,
    layer_of: &dyn Fn(&str) -> Option<usize>,
) -> Result<Projection, Diagnostic> {
    let mut diagnostics = Vec::new();
    let servers = flat
        .get(SERVERS)
        .and_then(TomlValue::as_table)
        .cloned()
        .unwrap_or_default();
    let servers_origin = origins.get(SERVERS).cloned().unwrap_or(Origin::Default);
    let active_origin = origins.get(ACTIVE).cloned().unwrap_or(Origin::Default);
    let active = flat
        .get(ACTIVE)
        .and_then(TomlValue::as_str)
        .map(ToOwned::to_owned);
    // Die Wahl eines Projekts, eines Profils oder eines Laufs geht der
    // Vorgabe des Nutzers vor (HUM-400).
    let chosen = chosen_server(flat, origins);

    if let Some((name, source)) = &chosen
        && let Some(done) = project_unentered(flat, origins, layer_of, &servers, name, source)
    {
        return Ok(done);
    }

    if servers.is_empty() {
        if let Some(name) = active {
            diagnostics.push(no_active(&format!(
                "{ACTIVE} (from {active_origin}) names the server {name}, and {SERVERS} has no \
                 entries"
            )));
            clear_view(flat, origins);
        } else {
            withhold_split_key(flat, origins, layer_of, &mut diagnostics, None);
        }
        return Ok(Projection {
            diagnostics,
            choice: None,
        });
    }

    for (name, entry) in &servers {
        check_entry_shape(name, entry, &servers_origin)?;
    }

    let (selected, selector, source) = match chosen {
        Some((name, source)) => (Some(name), SERVER, source),
        None => (active, ACTIVE, ServerSource::Default),
    };

    let shared = shared_references(&servers);
    diagnostics.extend(entry_findings(
        &servers,
        &shared,
        selected.as_deref(),
        &servers_origin,
    ));

    let Some(selected) = selected else {
        diagnostics.push(no_active(&format!(
            "{SERVERS} (from {servers_origin}) has entries, and {ACTIVE} names none of them"
        )));
        clear_view(flat, origins);
        return Ok(Projection {
            diagnostics,
            choice: None,
        });
    };
    let Some(entry) = servers.get(&selected).and_then(TomlValue::as_table) else {
        // Der Wähler, der den Namen trägt, nicht immer `llm.active`.
        let selector_origin = origins.get(selector).cloned().unwrap_or(Origin::Default);
        diagnostics.push(no_active(&format!(
            "{selector} (from {selector_origin}) names the server {selected}, and {SERVERS} has \
             no such entry (servers: {})",
            servers.keys().cloned().collect::<Vec<_>>().join(", ")
        )));
        clear_view(flat, origins);
        return Ok(Projection {
            diagnostics,
            choice: None,
        });
    };

    copy_entry(
        flat,
        origins,
        layer_of,
        &Selected {
            name: &selected,
            selector,
            entry,
            shared: shared.contains(&selected),
            origin: &servers_origin,
        },
        &mut diagnostics,
    );
    withhold_split_key(
        flat,
        origins,
        layer_of,
        &mut diagnostics,
        Some((&selected, selector)),
    );
    // Setzt eine höhere Ebene den Endpunkt selbst, gilt kein gespeicherter
    // Server, und keiner wird als gewählt angezeigt.
    let choice = endpoint_from_entry(layer_of, selector).then(|| ServerChoice {
        name: selected.clone(),
        source,
        known: true,
        saved: servers.keys().cloned().collect(),
    });
    Ok(Projection {
        diagnostics,
        choice,
    })
}

/// Der Name, den `llm.server` nennt, und woher er kommt; `None` ohne ihn.
fn chosen_server(
    flat: &BTreeMap<String, TomlValue>,
    origins: &BTreeMap<String, Origin>,
) -> Option<(String, ServerSource)> {
    let name = flat.get(SERVER).and_then(TomlValue::as_str)?;
    let source = ServerSource::of(origins.get(SERVER).unwrap_or(&Origin::Default));
    Some((name.to_owned(), source))
}

/// Ein gewählter Name, unter dem kein Eintrag steht, als fertiges Ergebnis
/// des Projizierens (HUM-400); `None`, wenn es den Eintrag gibt.
///
/// Zwei Fälle: Eine Datei mit flachen Werten und ohne Einträge zeigt ihren
/// Server als `default` (HUM-289), und ihn zu nennen, wählt genau diese
/// Werte. Jeder andere Name ohne Eintrag lässt nichts gelten und steht mit
/// `known = false` da; beim Laden ist das eine Warnung, damit `GetConfig`,
/// Doctor und der Daemon selbst weiterlaufen, und der Start einer Sitzung
/// macht daraus die Ablehnung ([`ServerChoice::refusal`]).
fn project_unentered(
    flat: &mut BTreeMap<String, TomlValue>,
    origins: &mut BTreeMap<String, Origin>,
    layer_of: &dyn Fn(&str) -> Option<usize>,
    servers: &toml::Table,
    name: &str,
    source: &ServerSource,
) -> Option<Projection> {
    let mut diagnostics = Vec::new();
    let implicit_default = servers.is_empty()
        && name == MIGRATED_NAME
        && FIELDS
            .iter()
            .any(|field| flat.contains_key(&format!("llm.{field}")));
    if implicit_default {
        withhold_split_key(flat, origins, layer_of, &mut diagnostics, None);
        let choice = endpoint_from_entry(layer_of, SERVER).then(|| ServerChoice {
            name: name.to_owned(),
            source: source.clone(),
            known: true,
            saved: vec![MIGRATED_NAME.to_owned()],
        });
        return Some(Projection {
            diagnostics,
            choice,
        });
    }
    if servers.contains_key(name) && check_name(name).is_ok() {
        return None;
    }
    let choice = ServerChoice {
        name: name.to_owned(),
        source: source.clone(),
        known: false,
        saved: servers.keys().cloned().collect(),
    };
    if let Some(mut refused) = choice.refusal() {
        refused.severity = Severity::Warning;
        diagnostics.push(refused);
    }
    clear_view(flat, origins);
    Some(Projection {
        diagnostics,
        choice: Some(choice),
    })
}

/// Der gewählte Eintrag, wie [`copy_entry`] ihn braucht.
struct Selected<'a> {
    /// Sein Name.
    name: &'a str,
    /// Der Pfad, der ihn gewählt hat: [`SERVER`] oder [`ACTIVE`].
    selector: &'static str,
    /// Seine Felder.
    entry: &'a toml::Table,
    /// Ob ein anderer Eintrag denselben Verweis trägt.
    shared: bool,
    /// Die Ebene, aus der `llm.servers` kommt.
    origin: &'a Origin,
}

/// Setzt die Felder des gewählten Eintrags an die Stelle der flachen
/// Schlüssel, soweit keine höhere Ebene als die des Wählers sie selbst
/// gesetzt hat.
fn copy_entry(
    flat: &mut BTreeMap<String, TomlValue>,
    origins: &mut BTreeMap<String, Origin>,
    layer_of: &dyn Fn(&str) -> Option<usize>,
    selected: &Selected<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = selected.name;
    let servers_origin = selected.origin;
    let mut endpoint_refused = false;
    for field in FIELDS {
        let path = format!("llm.{field}");
        // Eine höhere Ebene hat den flachen Schlüssel selbst gesetzt (oder
        // beiseitegelegt): Sie gewinnt, wie sie es ohne Einträge täte; ob der
        // Schlüssel dann noch mitgeht, entscheidet `withhold_split_key`.
        if layer_of(&path) > layer_of(selected.selector) {
            continue;
        }
        let mut value = selected.entry.get(field).cloned();
        if field == "endpoint"
            && let Some(text) = value.as_ref().and_then(TomlValue::as_str)
            && let Err(mut refused) = check_endpoint(name, text)
        {
            // Wie ein beiseitegelegter `llm.endpoint` (HUM-283): Der Satz
            // trägt dessen Marke, damit Doctor und Setup ihn an der Zeile
            // `llm` zeigen, und der Vorschlag schreibt über den flachen
            // Schlüssel in den aktiven Eintrag.
            refused.severity = Severity::Warning;
            refused.why = format!(
                "{SERVERS}.{name}.endpoint (from {servers_origin}): {}; the endpoint is \
                 ignored until it is corrected",
                refused.why
            );
            diagnostics.push(refused);
            value = None;
            endpoint_refused = true;
        }
        // Ohne brauchbaren Endpunkt hat der Schlüssel kein Ziel, und ein
        // geteilter Verweis gälte für zwei: In beiden Fällen keiner.
        if field == "api_key_ref" && (selected.shared || endpoint_refused) {
            value = None;
        }
        if let Some(value) = value {
            flat.insert(path.clone(), value);
            origins.insert(path, servers_origin.clone());
        } else {
            flat.remove(&path);
            origins.insert(path, Origin::Default);
        }
    }
}

/// Ob `llm.endpoint` aus dem Eintrag kommt, den `selector` wählt, und nicht
/// aus einer höheren Ebene.
fn endpoint_from_entry(layer_of: &dyn Fn(&str) -> Option<usize>, selector: &str) -> bool {
    layer_of("llm.endpoint") <= layer_of(selector)
}

/// Die Befunde über die Einträge selbst: geteilte Verweise und Endpunkte
/// der Einträge, die nicht aktiv sind.
fn entry_findings(
    servers: &toml::Table,
    shared: &BTreeSet<String>,
    active: Option<&str>,
    servers_origin: &Origin,
) -> Vec<Diagnostic> {
    let mut findings = Vec::new();
    if !shared.is_empty() {
        findings.push(
            Diagnostic::builder(CONFIG_021, Severity::Warning)
                .why(format!(
                    "the servers {} (from {servers_origin}) refer to the same access key; it is \
                     sent for none of them until each has a key of its own: remove api_key_ref \
                     from all but one of them in config.toml, then set a key for the others",
                    shared.iter().cloned().collect::<Vec<_>>().join(", ")
                ))
                .fix(FixAction::CopyCommand("humanitl config edit".to_owned()))
                .build(),
        );
    }

    // Jeder Endpunkt wird geprüft, nicht nur der aktive: Ein Eintrag, der
    // heute nicht gilt, soll seinen Fehler zeigen, bevor ihn jemand aktiviert.
    for (name, entry) in servers {
        if active == Some(name.as_str()) {
            continue;
        }
        let text = entry
            .get("endpoint")
            .and_then(TomlValue::as_str)
            .unwrap_or_default();
        if let Err(mut refused) = check_endpoint(name, text) {
            refused.severity = Severity::Warning;
            refused.why = format!(
                "{SERVERS}.{name}.endpoint (from {servers_origin}): {}; the server cannot be \
                 activated until it is corrected",
                refused
                    .why
                    .replacen(crate::llm_endpoint::SETTING, "the endpoint", 1)
            );
            refused.fix = Some(FixAction::CopyCommand(format!(
                "humanitl llm server rm {name}"
            )));
            findings.push(refused);
        }
    }

    findings
}

/// Nimmt den Verweis heraus, wenn Endpunkt und Verweis nicht aus einem
/// Eintrag kommen.
///
/// Mit gewähltem Eintrag ([`project`] hat beide schon aus ihm gesetzt) heißt
/// das: Eine höhere Ebene als die des Wählers (`selected`: Name und Pfad,
/// [`SERVER`] oder [`ACTIVE`]) hat `llm.endpoint` oder `llm.api_key_ref`
/// selbst gesetzt. Ohne Einträge: Die beiden flachen Schlüssel stammen aus
/// verschiedenen Ebenen, etwa der Endpunkt aus der Umgebung und der Verweis
/// aus `config.toml`.
fn withhold_split_key(
    flat: &mut BTreeMap<String, TomlValue>,
    origins: &mut BTreeMap<String, Origin>,
    layer_of: &dyn Fn(&str) -> Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
    selected: Option<(&str, &str)>,
) {
    const ENDPOINT: &str = "llm.endpoint";
    const KEY: &str = "llm.api_key_ref";
    if !flat.contains_key(KEY) {
        return;
    }
    let shown = |path: &str| origins.get(path).cloned().unwrap_or(Origin::Default);
    let label = |selector: &str| {
        if selector == ACTIVE {
            "the active server"
        } else {
            "the chosen server"
        }
    };
    let why = match selected {
        Some((name, selector)) if layer_of(ENDPOINT) > layer_of(selector) => Some(format!(
            "{ENDPOINT} from {} overrides {} {name}; its key is not sent",
            shown(ENDPOINT),
            label(selector)
        )),
        Some((name, selector)) if layer_of(KEY) > layer_of(selector) => Some(format!(
            "{KEY} from {} is not the key of {} {name}; it is not sent",
            shown(KEY),
            label(selector)
        )),
        None if flat.contains_key(ENDPOINT) && layer_of(ENDPOINT) != layer_of(KEY) => {
            Some(format!(
                "{ENDPOINT} from {} and {KEY} from {} do not come from one place; the stored \
                 key is not sent to an endpoint it was not stored for",
                shown(ENDPOINT),
                shown(KEY)
            ))
        }
        _ => None,
    };
    if let Some(why) = why {
        diagnostics.push(
            Diagnostic::builder(CONFIG_021, Severity::Warning)
                .why(why)
                .fix(FixAction::CopyCommand("humanitl llm server ls".to_owned()))
                .build(),
        );
        flat.remove(KEY);
        origins.insert(KEY.to_owned(), Origin::Default);
    }
}

/// `CONFIG_021` als Warnung, wenn kein Server aktiv ist.
fn no_active(what: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_021, Severity::Warning)
        .why(format!(
            "{what}; no server is active, and no endpoint, model or access key applies until one \
             is"
        ))
        .fix(FixAction::CopyCommand("humanitl llm server ls".to_owned()))
        .build()
}

/// Nimmt Endpunkt, Modelle und Verweis aus dem Stand, gleich aus welcher
/// Ebene: Ohne aktiven Server bleibt keine flache Altlast stehen.
fn clear_view(flat: &mut BTreeMap<String, TomlValue>, origins: &mut BTreeMap<String, Origin>) {
    for field in FIELDS {
        let path = format!("llm.{field}");
        flat.remove(&path);
        origins.insert(path, Origin::Default);
    }
}

/// Die Namen der Einträge, deren Verweis ein anderer Eintrag auch trägt.
fn shared_references(servers: &toml::Table) -> BTreeSet<String> {
    let mut by_ref: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, entry) in servers {
        if let Some(key_ref) = entry
            .get("api_key_ref")
            .and_then(TomlValue::as_str)
            .and_then(KeyRef::parse)
        {
            by_ref
                .entry(key_ref.to_string())
                .or_default()
                .push(name.clone());
        }
    }
    by_ref
        .into_values()
        .filter(|names| names.len() > 1)
        .flatten()
        .collect()
}

/// Ein Eintrag ist eine Tabelle mit nur den Feldern aus [`FIELDS`].
fn check_entry_shape(name: &str, entry: &TomlValue, origin: &Origin) -> Result<(), Diagnostic> {
    let Some(table) = entry.as_table() else {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "{SERVERS}.{name} (from {origin}) is not a table; a server is \
                 [{SERVERS}.{name}] with endpoint, models and api_key_ref"
            ))
            .build());
    };
    for key in table.keys() {
        if FIELDS.contains(&key.as_str()) {
            continue;
        }
        let hint = if key == "model" {
            format!(
                "the models of a server are a list: models = [\"{}\"]",
                model_hint(table)
            )
        } else {
            "a server takes endpoint, models and api_key_ref".to_owned()
        };
        return Err(Diagnostic::builder(CONFIG_002, Severity::Error)
            .why(format!(
                "{SERVERS}.{name}.{key} (from {origin}) is not a setting; {hint}"
            ))
            .build());
    }
    Ok(())
}

/// Der Wert eines handgeschriebenen `model`, für den Vorschlag.
fn model_hint(table: &toml::Table) -> String {
    table
        .get("model")
        .and_then(TomlValue::as_str)
        .unwrap_or("MODEL")
        .chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
        .take(64)
        .collect()
}

/// Prüft den Endpunkt eines Eintrags wie `llm.endpoint`.
fn check_endpoint(name: &str, endpoint: &str) -> Result<(), Diagnostic> {
    if endpoint.trim().is_empty() {
        return Err(refused(&format!(
            "the server {name} needs an endpoint, for example http://192.168.1.20:11434"
        )));
    }
    crate::llm_endpoint::check_text(endpoint)?;
    let readable = url::Url::parse(endpoint)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
    if readable.is_none() {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "the endpoint of the server {name} is not a URL with the scheme http or https and \
                 a host"
            ))
            .build());
    }
    Ok(())
}

/// `CONFIG_021` mit diesem Grund.
fn refused(why: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_021, Severity::Error)
        .why(format!("the model servers were not changed: {why}"))
        .fix(FixAction::CopyCommand("humanitl llm server ls".to_owned()))
        .build()
}

/// `CONFIG_021` für einen Namen, den es nicht gibt.
fn unknown(name: &str, state: &State) -> Diagnostic {
    refused(&format!(
        "there is no server named {name} (servers: {})",
        names(state)
    ))
}

/// Die Namen der Einträge als eine Zeile.
fn names(state: &State) -> String {
    if state.servers.is_empty() {
        return "none".to_owned();
    }
    state.servers.keys().cloned().collect::<Vec<_>>().join(", ")
}

/// Ein Eintrag als Ansicht.
fn entry_view(name: &str, entry: &toml::Table) -> ServerEntry {
    ServerEntry {
        name: name.to_owned(),
        endpoint: entry
            .get("endpoint")
            .and_then(TomlValue::as_str)
            .map(ToOwned::to_owned),
        models: entry
            .get("models")
            .and_then(TomlValue::as_array)
            .map(|models| {
                models
                    .iter()
                    .filter_map(TomlValue::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        api_key_ref: entry
            .get("api_key_ref")
            .and_then(TomlValue::as_str)
            .map(ToOwned::to_owned),
    }
}

/// Setzt ein Feld oder nimmt es heraus.
fn put(table: &mut toml::Table, field: &str, value: Option<TomlValue>) {
    match value {
        Some(value) => {
            table.insert(field.to_owned(), value);
        }
        None => {
            table.remove(field);
        }
    }
}

/// Der Stand unter `llm`, soweit er die Server betrifft.
#[derive(Debug, Clone)]
struct State {
    /// Die Einträge; ohne Einträge in der Datei der übernommene.
    servers: BTreeMap<String, toml::Table>,
    /// Der Name des aktiven Eintrags.
    active: Option<String>,
    /// Die drei flachen Schlüssel, soweit sie dastehen.
    flat: toml::Table,
    /// Ob die Datei selbst Einträge trägt.
    had_servers: bool,
}

impl State {
    /// Liest den Stand; eine Datei ohne Einträge, aber mit flachen Werten,
    /// bekommt im Abbild den Eintrag [`MIGRATED_NAME`] mit genau diesen
    /// Werten, aktiv.
    fn read(table: &toml::Table) -> Self {
        let llm = table.get("llm").and_then(TomlValue::as_table);
        let servers: BTreeMap<String, toml::Table> = llm
            .and_then(|llm| llm.get("servers"))
            .and_then(TomlValue::as_table)
            .map(|servers| {
                servers
                    .iter()
                    .filter_map(|(name, entry)| Some((name.clone(), entry.as_table()?.clone())))
                    .collect()
            })
            .unwrap_or_default();
        let active = llm
            .and_then(|llm| llm.get("active"))
            .and_then(TomlValue::as_str)
            .map(ToOwned::to_owned);
        let flat: toml::Table = FIELDS
            .iter()
            .filter_map(|field| {
                llm.and_then(|llm| llm.get(*field))
                    .map(|value| ((*field).to_owned(), value.clone()))
            })
            .collect();
        let had_servers = !servers.is_empty();
        let mut state = Self {
            servers,
            active,
            flat,
            had_servers,
        };
        if !had_servers && !state.flat.is_empty() {
            state
                .servers
                .insert(MIGRATED_NAME.to_owned(), state.flat.clone());
            state.active = Some(MIGRATED_NAME.to_owned());
        }
        state
    }

    /// Nimmt einen Eintrag heraus.
    fn take(&mut self, name: &str) -> Result<toml::Table, Diagnostic> {
        match self.servers.remove(name) {
            Some(entry) => Ok(entry),
            None => Err(unknown(name, self)),
        }
    }

    /// Macht einen vorhandenen Eintrag aktiv: Die flachen Schlüssel werden
    /// seine Werte, und was er nicht trägt, fällt flach weg.
    fn activate(&mut self, name: &str) {
        let Some(entry) = self.servers.get(name) else {
            return;
        };
        self.flat = FIELDS
            .iter()
            .filter_map(|field| {
                entry
                    .get(*field)
                    .map(|value| ((*field).to_owned(), value.clone()))
            })
            .collect();
        self.active = Some(name.to_owned());
    }

    /// Ein anderer Eintrag als `except`, dessen Verweis denselben Namen trägt
    /// wie `key_ref`, gleich an welchem Ort.
    fn holder_of(&self, key_ref: &KeyRef, except: &str) -> Option<String> {
        self.servers
            .iter()
            .filter(|(name, _)| name.as_str() != except)
            .find(|(_, entry)| {
                entry
                    .get("api_key_ref")
                    .and_then(TomlValue::as_str)
                    .and_then(KeyRef::parse)
                    .is_some_and(|other| other.name() == key_ref.name())
            })
            .map(|(name, _)| name.clone())
    }

    /// Ob ein Eintrag auf diesen Schlüssel zeigt.
    fn uses_key(&self, key_ref: &KeyRef) -> bool {
        self.servers.values().any(|entry| {
            entry
                .get("api_key_ref")
                .and_then(TomlValue::as_str)
                .and_then(KeyRef::parse)
                .is_some_and(|other| other.name() == key_ref.name())
        })
    }

    /// Die Änderungen, die aus der Datei `table` diesen Stand machen.
    fn edits(&self, table: &toml::Table) -> Vec<Edit> {
        let before = Self::raw(table);
        let mut edits = Vec::new();
        let path = |parts: &[&str]| parts.iter().map(|part| (*part).to_owned()).collect();

        if self.servers.is_empty() {
            if before.had_servers {
                edits.push(Edit::new(path(&["llm", "servers"]), None));
            }
        } else {
            for name in before.servers.keys() {
                if !self.servers.contains_key(name) {
                    edits.push(Edit::new(path(&["llm", "servers", name]), None));
                }
            }
            for (name, entry) in &self.servers {
                let old = before.servers.get(name);
                for field in FIELDS {
                    let new_value = entry.get(field);
                    if old.and_then(|old| old.get(field)) != new_value {
                        edits.push(Edit::new(
                            path(&["llm", "servers", name, field]),
                            new_value.cloned(),
                        ));
                    }
                }
            }
        }
        if before.active != self.active {
            edits.push(Edit::new(
                path(&["llm", "active"]),
                self.active.clone().map(TomlValue::String),
            ));
        }
        for field in FIELDS {
            let new_value = self.flat.get(field);
            if before.flat.get(field) != new_value {
                edits.push(Edit::new(path(&["llm", field]), new_value.cloned()));
            }
        }
        edits
    }

    /// Der Stand, wie er in der Datei steht, ohne Übernahme.
    fn raw(table: &toml::Table) -> Self {
        let mut state = Self::read(table);
        if !state.had_servers {
            state.servers.clear();
            state.active = table
                .get("llm")
                .and_then(TomlValue::as_table)
                .and_then(|llm| llm.get("active"))
                .and_then(TomlValue::as_str)
                .map(ToOwned::to_owned);
        }
        state
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn table(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    /// Wendet die Änderungen an, wie `edit_with` es tut, ohne Datei.
    fn apply(table: &toml::Table, edits: &[Edit]) -> toml::Table {
        crate::edit::apply_edits(table, edits).unwrap()
    }

    const FLAT: &str = r#"
[llm]
endpoint = "http://192.168.1.20:11434"
models = ["qwen3"]
api_key_ref = "secret-service:humanitl/llm"
"#;

    fn get<'a>(table: &'a toml::Table, path: &str) -> Option<&'a TomlValue> {
        let mut parts = path.split('.');
        let mut value = table.get(parts.next()?)?;
        for part in parts {
            value = value.as_table()?.get(part)?;
        }
        Some(value)
    }

    /// Eine flache Datei wird beim ersten Auftrag zum Eintrag `default`, und
    /// sein Verweis zeigt auf denselben Eintrag im Schlüsselbund wie vorher.
    #[test]
    fn a_flat_file_migrates_to_default_with_the_same_key_reference() {
        let old = table(FLAT);
        let listed = list(&old);
        assert!(listed.implicit);
        assert_eq!(listed.active.as_deref(), Some("default"));
        assert_eq!(
            listed.servers[0].api_key_ref.as_deref(),
            Some("secret-service:humanitl/llm")
        );

        let edits = add(&old, "cloud", "https://api.example.com/v1", &[], false).unwrap();
        let new = apply(&old, &edits);
        assert_eq!(
            get(&new, "llm.servers.default.api_key_ref").and_then(TomlValue::as_str),
            Some("secret-service:humanitl/llm"),
            "the migrated entry lost its key reference"
        );
        assert_eq!(
            get(&new, "llm.servers.default.endpoint").and_then(TomlValue::as_str),
            Some("http://192.168.1.20:11434")
        );
        assert_eq!(
            get(&new, "llm.active").and_then(TomlValue::as_str),
            Some("default")
        );
        assert_eq!(
            get(&new, "llm.api_key_ref").and_then(TomlValue::as_str),
            Some("secret-service:humanitl/llm"),
            "the flat view of the active entry changed"
        );
        assert!(get(&new, "llm.servers.cloud.api_key_ref").is_none());
        assert!(!list(&new).implicit);
    }

    /// Aktivieren setzt die drei flachen Schlüssel auf die Werte des
    /// Eintrags; was er nicht trägt, fällt flach weg, auch der Verweis des
    /// vorher aktiven.
    #[test]
    fn activation_replaces_the_flat_view_and_drops_the_old_key_reference() {
        let old = table(FLAT);
        let new = apply(
            &old,
            &add(
                &old,
                "cloud",
                "https://api.example.com/v1",
                &["gpt".to_owned()],
                false,
            )
            .unwrap(),
        );
        let new = apply(&new, &activate(&new, "cloud").unwrap());
        assert_eq!(
            get(&new, "llm.active").and_then(TomlValue::as_str),
            Some("cloud")
        );
        assert_eq!(
            get(&new, "llm.endpoint").and_then(TomlValue::as_str),
            Some("https://api.example.com/v1")
        );
        assert!(
            get(&new, "llm.api_key_ref").is_none(),
            "the key reference of the previous server stays beside the new endpoint"
        );
        assert_eq!(
            get(&new, "llm.servers.default.api_key_ref").and_then(TomlValue::as_str),
            Some("secret-service:humanitl/llm")
        );
    }

    /// Der aktive Eintrag geht nicht ohne Nachfolger, solange es andere gibt;
    /// mit Nachfolger ist der danach aktiv, und als letzter nimmt er die
    /// flachen Schlüssel mit.
    #[test]
    fn the_active_server_is_removed_only_with_a_replacement() {
        let old = table(FLAT);
        let two = apply(
            &old,
            &add(&old, "cloud", "https://api.example.com/v1", &[], false).unwrap(),
        );
        let refused = remove(&two, "default", None).unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert!(refused.fix.is_some());

        let removal = remove(&two, "default", Some("cloud")).unwrap();
        assert_eq!(
            removal
                .orphaned_key
                .map(|key_ref| key_ref.to_string())
                .as_deref(),
            Some("secret-service:humanitl/llm")
        );
        let one = apply(&two, &removal.edits);
        assert_eq!(
            get(&one, "llm.active").and_then(TomlValue::as_str),
            Some("cloud")
        );
        assert!(get(&one, "llm.api_key_ref").is_none());
        assert!(get(&one, "llm.servers.default").is_none());

        let none = apply(&one, &remove(&one, "cloud", None).unwrap().edits);
        assert!(get(&none, "llm.active").is_none());
        assert!(get(&none, "llm.endpoint").is_none());
        assert!(get(&none, "llm.servers").is_none());
    }

    /// Ein nicht aktiver Eintrag geht ohne Nachfolger, und der Schlüssel
    /// eines Eintrags, den ein anderer teilt, gilt nicht als verwaist.
    #[test]
    fn a_shared_key_is_not_orphaned() {
        let old = table(
            r#"
[llm]
active = "a"
endpoint = "http://a.lan/"
api_key_ref = "file:shared"
[llm.servers.a]
endpoint = "http://a.lan/"
api_key_ref = "file:shared"
[llm.servers.b]
endpoint = "http://b.lan/"
api_key_ref = "file:shared"
"#,
        );
        let removal = remove(&old, "b", None).unwrap();
        assert_eq!(removal.orphaned_key, None);
    }

    /// Umbenennen behält den Verweis und zieht `llm.active` nach.
    #[test]
    fn rename_keeps_the_key_reference_and_follows_with_active() {
        let old = table(FLAT);
        let new = apply(&old, &rename(&old, "default", "home").unwrap());
        assert_eq!(
            get(&new, "llm.active").and_then(TomlValue::as_str),
            Some("home")
        );
        assert_eq!(
            get(&new, "llm.servers.home.api_key_ref").and_then(TomlValue::as_str),
            Some("secret-service:humanitl/llm")
        );
        assert!(get(&new, "llm.servers.default").is_none());
        assert_eq!(rename(&new, "home", "Home").unwrap_err().code, CONFIG_021);
        assert_eq!(rename(&new, "nope", "x").unwrap_err().code, CONFIG_021);
    }

    /// Namen und Endpunkte werden geprüft wie Verweis und `llm.endpoint`.
    #[test]
    fn names_and_endpoints_are_checked() {
        let old = table("");
        assert_eq!(
            add(&old, "a b", "http://x.lan/", &[], false)
                .unwrap_err()
                .code,
            CONFIG_021
        );
        assert_eq!(
            add(&old, "x", "http://x.lan/\"", &[], false)
                .unwrap_err()
                .code,
            CONFIG_003
        );
        assert_eq!(
            add(&old, "x", "ftp://x.lan/", &[], false).unwrap_err().code,
            CONFIG_003
        );
        let one = apply(&old, &add(&old, "x", "http://x.lan/", &[], false).unwrap());
        assert_eq!(
            get(&one, "llm.active").and_then(TomlValue::as_str),
            Some("x"),
            "the first server becomes the active one"
        );
        assert_eq!(
            add(&one, "x", "http://y.lan/", &[], false)
                .unwrap_err()
                .code,
            CONFIG_021
        );
    }

    /// Ein neuer Schlüssel bekommt den Namen des Eintrags, außer ein anderer
    /// Eintrag benutzt ihn schon.
    #[test]
    fn key_names_never_collide() {
        let old = table(FLAT);
        let two = apply(
            &old,
            &add(&old, "llm", "http://other.lan/", &[], false).unwrap(),
        );
        assert_eq!(key_name_for(&two, "default").unwrap(), "llm");
        assert_eq!(key_name_for(&two, "llm").unwrap(), "llm-2");
        let three = apply(
            &two,
            &add(&two, "cloud", "http://cloud.lan/", &[], false).unwrap(),
        );
        assert_eq!(key_name_for(&three, "cloud").unwrap(), "cloud");
    }

    /// Ein flacher Schlüssel, geschrieben bei aktivem Eintrag, geht auch in
    /// den Eintrag; ohne Einträge bleibt es beim flachen Schlüssel.
    #[test]
    fn a_flat_write_is_mirrored_into_the_active_entry() {
        let segments = vec!["llm".to_owned(), "endpoint".to_owned()];
        let value = TomlValue::String("http://new.lan/".to_owned());
        assert!(mirror_of(&table(FLAT), &segments, Some(&value)).is_empty());
        let old = table(FLAT);
        let two = apply(
            &old,
            &add(&old, "cloud", "http://cloud.lan/", &[], true).unwrap(),
        );
        let mirrored = mirror_of(&two, &segments, Some(&value));
        assert_eq!(
            mirrored,
            vec![Edit::at("llm.servers.cloud.endpoint", Some(value))]
        );
    }

    fn load_text(text: &str, env: &[(&str, &str)]) -> crate::origin::Resolved {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        let sources = crate::load::Sources {
            global_toml: Some(path),
            ..crate::load::Sources::empty()
        }
        .with_env(crate::env::Env::from_pairs(
            env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        ));
        crate::load::load(&sources).unwrap()
    }

    const TWO: &str = r#"
[llm]
active = "home"
endpoint = "http://stale.lan/"
api_key_ref = "file:stale"
[llm.servers.home]
endpoint = "http://home.lan:11434/"
models = ["qwen3"]
api_key_ref = "secret-service:humanitl/llm"
[llm.servers.cloud]
endpoint = "https://api.example.com/v1"
api_key_ref = "secret-service:humanitl/cloud"
"#;

    /// Beim Laden liefert der aktive Eintrag die flachen Schlüssel, auch wenn
    /// die Datei dort noch einen alten Stand trägt; die Umgebung gewinnt
    /// weiter über beide.
    #[test]
    fn the_active_entry_is_what_the_flat_keys_resolve_to() {
        let resolved = load_text(TWO, &[]);
        let llm = &resolved.config.llm;
        assert_eq!(
            llm.endpoint.as_ref().map(url::Url::as_str),
            Some("http://home.lan:11434/")
        );
        assert_eq!(
            llm.api_key_ref.as_deref(),
            Some("secret-service:humanitl/llm")
        );
        assert_eq!(llm.models, vec!["qwen3".to_owned()]);
        assert_eq!(llm.servers.len(), 2);

        let switched = load_text(&TWO.replace("active = \"home\"", "active = \"cloud\""), &[]);
        assert_eq!(
            switched.config.llm.api_key_ref.as_deref(),
            Some("secret-service:humanitl/cloud")
        );
        assert!(switched.config.llm.models.is_empty());

        let overridden = load_text(TWO, &[("HUMANITL_LLM__ENDPOINT", "http://env.lan/")]);
        assert_eq!(
            overridden
                .config
                .llm
                .endpoint
                .as_ref()
                .map(url::Url::as_str),
            Some("http://env.lan/")
        );
    }

    /// `llm.active` ohne Eintrag: kein Server aktiv, kein Verweis, kein
    /// Endpunkt, und eine Warnung statt eines Abbruchs.
    #[test]
    fn an_active_name_without_entry_leaves_no_server_and_no_key() {
        let resolved = load_text(&TWO.replace("active = \"home\"", "active = \"gone\""), &[]);
        assert!(resolved.config.llm.endpoint.is_none());
        assert!(resolved.config.llm.api_key_ref.is_none());
        let warning = resolved
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == CONFIG_021)
            .expect("CONFIG_021");
        assert_eq!(warning.severity, Severity::Warning);
    }

    /// Ein abgelehnter Endpunkt eines Eintrags bricht das Laden nie ab: beim
    /// aktiven gilt er als nicht gesetzt (und Doctor findet den Befund), beim
    /// anderen bleibt es bei der Warnung.
    #[test]
    fn a_refused_entry_endpoint_is_set_aside_not_fatal() {
        let active_bad = load_text(
            &TWO.replace("http://home.lan:11434/", "http://home.lan:11434/\\\""),
            &[],
        );
        assert!(active_bad.config.llm.endpoint.is_none());
        assert!(crate::llm_endpoint::set_aside(&active_bad.diagnostics).is_some());

        let other_bad = load_text(
            &TWO.replace(
                "https://api.example.com/v1",
                "https://api.example.com/v1\\\"",
            ),
            &[],
        );
        assert_eq!(
            other_bad.config.llm.endpoint.as_ref().map(url::Url::as_str),
            Some("http://home.lan:11434/")
        );
        assert!(crate::llm_endpoint::set_aside(&other_bad.diagnostics).is_none());
        assert!(
            other_bad
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == CONFIG_003
                    && diagnostic.why.contains("llm.servers.cloud.endpoint"))
        );
    }

    /// Ein Name, der keiner ist, scheitert beim Laden wie ein falscher
    /// Verweis.
    #[test]
    fn an_invalid_server_name_does_not_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[llm.servers.\"Bad Name\"]\nendpoint = \"http://x.lan/\"\n",
        )
        .unwrap();
        let sources = crate::load::Sources {
            global_toml: Some(path),
            ..crate::load::Sources::empty()
        };
        assert_eq!(crate::load::load(&sources).unwrap_err().code, CONFIG_003);
    }

    fn has_warning(resolved: &crate::origin::Resolved, needle: &str) -> bool {
        resolved.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == CONFIG_021
                && diagnostic.severity == Severity::Warning
                && diagnostic.why.contains(needle)
        })
    }

    /// Ein Endpunkt aus der Umgebung über dem aktiven Server: Er gilt, aber
    /// der Schlüssel des Servers geht nicht mit (HUM-289, Review A).
    #[test]
    fn an_endpoint_from_a_higher_layer_gets_no_key() {
        let cloud = TWO.replace("active = \"home\"", "active = \"cloud\"");
        let resolved = load_text(
            &cloud,
            &[("HUMANITL_LLM__ENDPOINT", "http://192.168.1.20:11434")],
        );
        assert_eq!(
            resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
            Some("http://192.168.1.20:11434/")
        );
        assert_eq!(
            resolved.config.llm.api_key_ref, None,
            "the key of the active server went along with another endpoint"
        );
        assert!(has_warning(&resolved, "overrides the active server cloud"));
        assert!(
            !resolved.config.llm.key_goes_with_endpoint()
                || resolved.config.llm.api_key_ref.is_none()
        );
    }

    /// Andersherum: ein Verweis aus der Umgebung neben dem Endpunkt des
    /// Eintrags wirkt nicht.
    #[test]
    fn a_key_reference_from_a_higher_layer_is_not_sent() {
        let resolved = load_text(TWO, &[("HUMANITL_LLM__API_KEY_REF", "file:other")]);
        assert_eq!(
            resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
            Some("http://home.lan:11434/")
        );
        assert_eq!(resolved.config.llm.api_key_ref, None);
        assert!(has_warning(
            &resolved,
            "is not the key of the active server home"
        ));
    }

    /// Ohne Einträge gilt dieselbe Regel für die flachen Schlüssel: ein
    /// Endpunkt aus der Umgebung bekommt den Schlüssel aus `config.toml`
    /// nicht.
    #[test]
    fn a_flat_endpoint_from_another_layer_gets_no_key() {
        let resolved = load_text(
            FLAT,
            &[("HUMANITL_LLM__ENDPOINT", "http://192.168.1.99:11434")],
        );
        assert_eq!(resolved.config.llm.api_key_ref, None);
        assert!(has_warning(&resolved, "do not come from one place"));
        let same = load_text(FLAT, &[]);
        assert_eq!(
            same.config.llm.api_key_ref.as_deref(),
            Some("secret-service:humanitl/llm")
        );
    }

    /// Einträge ohne `llm.active`, und `llm.active` ohne Einträge: kein
    /// Endpunkt, kein Verweis, keine flache Altlast (Review B).
    #[test]
    fn without_an_active_server_no_flat_value_survives() {
        let without = load_text(&TWO.replace("active = \"home\"\n", ""), &[]);
        assert!(without.config.llm.endpoint.is_none());
        assert!(without.config.llm.api_key_ref.is_none());
        assert!(without.config.llm.models.is_empty());
        assert!(has_warning(&without, "names none of them"));

        let no_entries = load_text(
            "[llm]\nactive = \"gone\"\nendpoint = \"http://a.lan/\"\napi_key_ref = \"file:llm\"\n",
            &[],
        );
        assert!(no_entries.config.llm.endpoint.is_none());
        assert!(no_entries.config.llm.api_key_ref.is_none());
        assert!(has_warning(&no_entries, "has no entries"));
    }

    /// Zwei Einträge mit demselben Verweis: Er wirkt für keinen, und keiner
    /// der beiden bekommt darüber einen neuen Schlüssel (Review C).
    #[test]
    fn a_shared_key_reference_is_sent_for_no_server_and_not_written() {
        let shared = TWO.replace(
            "api_key_ref = \"secret-service:humanitl/cloud\"",
            "api_key_ref = \"secret-service:humanitl/llm\"",
        );
        let resolved = load_text(&shared, &[]);
        assert_eq!(resolved.config.llm.api_key_ref, None);
        assert!(has_warning(&resolved, "refer to the same access key"));

        let shared_table = table(&shared);
        assert_eq!(
            key_name_for(&shared_table, "home").unwrap_err().code,
            CONFIG_021
        );
        let one = table(TWO);
        let home_ref = KeyRef::parse("secret-service:humanitl/llm").unwrap();
        assert_eq!(
            set_key_ref(&one, "cloud", Some(&home_ref))
                .unwrap_err()
                .code,
            CONFIG_021,
            "a key reference of another server was attached"
        );
    }

    /// Ein Endpunkt, den kein Parser annimmt oder der kein http ist, macht
    /// einen anderen Eintrag nur unbrauchbar, beim aktiven gilt er als nicht
    /// gesetzt; geladen wird in beiden Fällen (Review F).
    #[test]
    fn a_broken_entry_endpoint_never_fails_the_load() {
        let other = load_text(&TWO.replace("https://api.example.com/v1", "ftp://x"), &[]);
        assert!(other.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Warning
                && diagnostic.why.contains("llm.servers.cloud.endpoint")
        }));
        assert_eq!(
            activate(
                &table(&TWO.replace("https://api.example.com/v1", "ftp://x")),
                "cloud"
            )
            .unwrap_err()
            .code,
            CONFIG_003
        );
        let unreadable = load_text(
            &TWO.replace("https://api.example.com/v1", "http://[::"),
            &[],
        );
        assert!(unreadable.config.llm.endpoint.is_some());
        let active = load_text(
            &TWO.replace("http://home.lan:11434/", "ftp://home.lan/"),
            &[],
        );
        assert!(active.config.llm.endpoint.is_none());
        assert!(active.config.llm.api_key_ref.is_none());
    }

    /// `model = "x"` von Hand: ein Befund, der `models` nennt.
    #[test]
    fn a_hand_written_model_names_models() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[llm]\nactive = \"a\"\n[llm.servers.a]\nendpoint = \"http://a.lan/\"\nmodel = \"qwen3\"\n",
        )
        .unwrap();
        let sources = crate::load::Sources {
            global_toml: Some(path),
            ..crate::load::Sources::empty()
        };
        let refused = crate::load::load(&sources).unwrap_err();
        assert_eq!(refused.code, CONFIG_002);
        assert!(
            refused.why.contains("models = [\"qwen3\"]"),
            "{}",
            refused.why
        );
    }
}
