//! Wer welchen gespeicherten Server nutzt (HUM-405): die Zuordnung zu einem
//! Profil der Sitzung oder zu einem Projekt, und die Bestätigung der Wahl
//! eines Projekts.
//!
//! Beide Zuordnungen schreiben `llm.server` unter `[config.llm]`, also
//! dieselbe Zeile, die HUM-400 von Hand verlangte: in
//! `profiles/<name>.toml` oder in `<projekt>/.humanitl/profile.toml`. Den
//! Namen prüft der Aufrufer gegen die gespeicherten Server
//! ([`crate::servers::require_saved`]); hier geht es um die Dateien.
//!
//! # Die Datei des Projekts gehört nicht dem Menschen allein
//!
//! Der Projektordner ist der Ordner, in dem der Agent schreibt. Er kann dort
//! `.humanitl/profile.toml` ändern, durch einen Verweis ersetzen oder
//! `.humanitl` selbst zu einem Verweis machen. Daraus folgen zwei Regeln:
//!
//! - **Geschrieben wird ohne einem Verweis zu folgen.** Ordner und Datei
//!   werden relativ zum geöffneten Projektordner mit `O_NOFOLLOW` geöffnet,
//!   die neue Fassung entsteht als Nebendatei in demselben geöffneten Ordner
//!   und ersetzt die alte mit `renameat`. Ein Verweis auf `config.toml` oder
//!   eine Datei außerhalb bekommt so nie eine Zeile.
//! - **Eine Wahl aus dem Projekt gilt erst nach einer Bestätigung.** Nennt
//!   das Projekt einen anderen Server als den Standard, startet eine Sitzung
//!   erst, wenn der Mensch diese Wahl für diesen Ordner einmal bestätigt hat
//!   ([`unconfirmed`], `CONFIG_022`). Die Bestätigung liegt je Projektordner
//!   und Servername in [`crate::paths::Paths::project_servers_path`], im
//!   Datenverzeichnis des Daemons, nie im Projekt; sonst könnte sie der Agent
//!   selbst ausstellen. Eine Zuordnung über den Daemon ist eine Wahl des
//!   Menschen und gilt zugleich als Bestätigung.

use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{CONFIG_015, CONFIG_021, CONFIG_022};
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{FlockOperation, Mode, OFlags};
use toml::Value as TomlValue;

use crate::edit::{Edit, Rendered};
use crate::model::LlmConfig;
use crate::paths::Paths;
use crate::profile::{DEFAULT_PROFILE, Profile, ProfileSource};
use crate::servers::{SERVER, ServerSource};

/// Der Ordner des Projekt-Profils im Projektordner.
pub const PROJECT_DIR: &str = ".humanitl";

/// Das Projekt-Profil in [`PROJECT_DIR`].
pub const PROJECT_FILE: &str = "profile.toml";

/// Der Pfad von `llm.server` in einer Profildatei.
fn server_segments() -> Vec<String> {
    vec!["config".to_owned(), "llm".to_owned(), "server".to_owned()]
}

/// Ein Profil der Sitzung und der Server, den es wählt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileUse {
    /// Der Name des Profils.
    pub name: String,
    /// `llm.server` des Profils, wenn es einen nennt.
    pub server: Option<String>,
    /// Wahr für ein mitgeliefertes Profil ohne eigene Datei.
    pub builtin: bool,
    /// Ein Satz darüber, wofür es da ist.
    pub description: Option<String>,
}

/// Jedes Profil der Sitzung außer [`DEFAULT_PROFILE`], nach Namen sortiert,
/// mit dem Server, den es wählt.
///
/// `default` gilt für jede Sitzung; sein Server ist der Standard und steht
/// in `llm.active`. Ein Profil, das sich nicht lesen lässt, fehlt hier;
/// [`crate::available_profiles`] meldet es.
#[must_use]
pub fn profile_uses(paths: &Paths) -> Vec<ProfileUse> {
    let (summaries, _) = crate::available_profiles(paths);
    summaries
        .into_iter()
        .filter(|summary| !summary.broken && summary.name != DEFAULT_PROFILE)
        .filter_map(|summary| {
            let profile = summary.source.load().ok()?;
            Some(ProfileUse {
                server: profile
                    .overlay
                    .get(SERVER)
                    .and_then(TomlValue::as_str)
                    .map(ToOwned::to_owned),
                builtin: matches!(summary.source, ProfileSource::Builtin(_)),
                description: summary.description,
                name: summary.name,
            })
        })
        .collect()
}

/// Der Projektordner der Konfiguration und der Server, den er wählt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectUse {
    /// Der Projektordner, wie `sandbox.work_dir` ihn nennt.
    pub dir: PathBuf,
    /// `llm.server`, wie es im Projekt-Profil selbst steht: die Zeile, die
    /// eine Zuordnung schreibt und entfernt.
    pub server: Option<String>,
    /// Der Server, den das Projekt für den nächsten Start wirklich wählt:
    /// seine eigene Zeile, oder die des mitgelieferten Profils, das es mit
    /// `name` nennt (HUM-405, Review). `None`, wenn es keinen wählt.
    pub effective: Option<String>,
    /// Das Profil, über das die Wahl kommt, wenn das Projekt-Profil selbst
    /// keine Zeile trägt.
    pub via_profile: Option<String>,
    /// Ob die wirksame Wahl der Standard oder für diesen Ordner bestätigt
    /// ist, also ob ein Start ohne Rückfrage geht; falsch ohne Wahl.
    pub confirmed: bool,
}

/// Der Projektordner aus `sandbox.work_dir`, wie der nächste Start ohne
/// Wunsch ihn nimmt, und der Server, den sein Projekt-Profil wählt; `None`
/// ohne Projektordner.
///
/// Die wirksame Wahl und ihre Bestätigung kommen aus derselben Auflösung
/// wie ein Start, also auch über ein Profil, das das Projekt nennt; die
/// eigene Zeile steht getrennt daneben. Ein Projekt-Profil, das sich nicht
/// lesen lässt, wählt hier nichts; der Start meldet es.
#[must_use]
pub fn project_use(paths: &Paths) -> Option<ProjectUse> {
    let resolved =
        crate::resolve(&crate::ProfileSelection::default(), None, paths.env(), &[]).ok()?;
    let dir = resolved.config.sandbox.work_dir?;
    let file = paths.project_profile_path(&dir);
    let server = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| Profile::parse(&text, &ProfileSource::Project(file.clone())).ok())
        .and_then(|profile| {
            profile
                .overlay
                .get(SERVER)
                .and_then(TomlValue::as_str)
                .map(ToOwned::to_owned)
        });
    let llm = &resolved.config.llm;
    let effective = llm
        .choice
        .as_ref()
        .filter(|choice| matches!(&choice.source, ServerSource::Project(path) if *path == file))
        .map(|choice| choice.name.clone());
    let via_profile = if server.is_none() && effective.is_some() {
        resolved
            .profiles
            .iter()
            .find(|profile| matches!(profile.source, ProfileSource::Project(_)))
            .and_then(|profile| profile.declared_name.clone())
    } else {
        None
    };
    // Ein Name ohne gespeicherten Server startet ohnehin nicht; als bestätigt
    // darf er deshalb auch nicht dastehen.
    let known = llm.choice.as_ref().is_some_and(|choice| choice.known);
    let confirmed = effective.is_some() && known && unconfirmed(paths, llm).is_none();
    Some(ProjectUse {
        dir,
        server,
        effective,
        via_profile,
        confirmed,
    })
}

/// Ordnet dem Profil der Sitzung `name` den Server `server` zu, oder nimmt
/// die Zuordnung mit `None` heraus.
///
/// Ein mitgeliefertes Profil ohne eigene Datei bekommt dafür eine Kopie
/// seines Texts unter `profiles/<name>.toml` mit dem einen Wert dazu.
/// Bleibt nach dem Herausnehmen eine Kopie, die dem mitgelieferten Profil
/// gleicht, verschwindet sie wieder: Sonst verdeckte sie das Profil auch
/// nach einem Update (`CONFIG_008`).
///
/// # Errors
///
/// `CONFIG_003` für einen Namen, der keiner ist; `CONFIG_021` für
/// [`DEFAULT_PROFILE`] und für ein Profil, das es nicht gibt; die Befunde
/// von [`crate::edit::edit_or_create`] und von [`Profile::parse`], wenn die
/// Datei danach kein Profil wäre. Die Datei bleibt dann, wie sie war.
pub fn assign_profile(paths: &Paths, name: &str, server: Option<&str>) -> Result<(), Diagnostic> {
    crate::profile::check_name(name, "the profile of the assignment")?;
    if name == DEFAULT_PROFILE {
        return Err(Diagnostic::builder(CONFIG_021, Severity::Error)
            .why(format!(
                "the profile {DEFAULT_PROFILE} applies to every session, so its server is the \
                 default server; make a server the default instead of assigning it to \
                 {DEFAULT_PROFILE}"
            ))
            .fix(FixAction::CopyCommand(
                "humanitl llm server use NAME".to_owned(),
            ))
            .build());
    }
    let path = paths.profile_path(name);
    let builtin = crate::profile::builtin_text(name);
    if !path.is_file() {
        match (builtin, server) {
            (Some(_), None) => return Ok(()),
            (Some(_), Some(_)) => {}
            (None, _) => {
                return Err(Diagnostic::builder(CONFIG_021, Severity::Error)
                    .why(format!(
                        "there is no session profile {name}; nothing was assigned"
                    ))
                    .fix(FixAction::CopyCommand(
                        "humanitl config schema --profiles".to_owned(),
                    ))
                    .build());
            }
        }
    }
    let value = server.map(|server| TomlValue::String(server.to_owned()));
    let source = ProfileSource::File(path.clone());
    crate::edit::edit_or_create(
        &path,
        builtin.unwrap_or_default(),
        |_| Ok(vec![Edit::new(server_segments(), value)]),
        |candidate| {
            let text = std::fs::read_to_string(candidate).map_err(|err| {
                Diagnostic::builder(CONFIG_015, Severity::Error)
                    .why(format!(
                        "{} was not written: the new text could not be read back ({err})",
                        path.display()
                    ))
                    .build()
            })?;
            Profile::parse(&text, &source).map(|_| ())
        },
    )?;
    if server.is_none()
        && let Some(builtin) = builtin
        && same_profile(&path, builtin, name)
    {
        // Die Kopie trägt nichts mehr, was das mitgelieferte Profil nicht
        // auch trägt; ohne sie gilt wieder dessen Fassung.
        std::fs::remove_file(&path).map_err(|err| {
            Diagnostic::builder(CONFIG_015, Severity::Warning)
                .why(format!(
                    "the assignment was removed, and the copy {} of the bundled profile {name} \
                     stays: {err}",
                    path.display()
                ))
                .build()
        })?;
    }
    Ok(())
}

/// Ob die Datei `path` dasselbe Profil beschreibt wie der mitgelieferte Text.
fn same_profile(path: &Path, builtin: &'static str, name: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Some(builtin_name) = crate::profile::builtin_name(name) else {
        return false;
    };
    let (Ok(own), Ok(bundled)) = (
        Profile::parse(&text, &ProfileSource::File(path.to_path_buf())),
        Profile::parse(builtin, &ProfileSource::Builtin(builtin_name)),
    ) else {
        return false;
    };
    own.overlay == bundled.overlay
        && own.rules == bundled.rules
        && own.description == bundled.description
}

/// Ordnet dem Projektordner `project` den Server `server` zu, oder nimmt die
/// Zuordnung mit `None` heraus, und gibt den aufgelösten Ordner zurück.
///
/// Geschrieben wird `<projekt>/.humanitl/profile.toml`, ohne einem Verweis
/// zu folgen (siehe oben). Eine Zuordnung über den Daemon ist die Wahl des
/// Menschen; der Aufrufer bestätigt sie danach mit [`confirm`].
///
/// # Errors
///
/// `CONFIG_021` für einen Ordner, der nicht absolut ist oder keiner ist;
/// `CONFIG_015`, wenn `.humanitl` oder `profile.toml` ein Verweis ist oder
/// sich nicht öffnen, sperren oder schreiben lässt; `CONFIG_001` für eine
/// Datei, die kein TOML ist; die Befunde von [`Profile::parse`], wenn sie
/// danach kein Projekt-Profil wäre. Die Datei bleibt dann, wie sie war.
pub fn assign_project(project: &Path, server: Option<&str>) -> Result<PathBuf, Diagnostic> {
    let dir = project_dir(project)?;
    let file = dir.join(PROJECT_DIR).join(PROJECT_FILE);
    let unwritable = |what: &str, err: &dyn std::fmt::Display| {
        Diagnostic::builder(CONFIG_015, Severity::Error)
            .why(format!(
                "{} was not changed: {what} ({err}); nothing was assigned",
                file.display()
            ))
            .build()
    };
    let root = rustix::fs::open(
        &dir,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|err| unwritable("the project folder could not be opened", &err))?;
    if server.is_some() {
        match rustix::fs::mkdirat(&root, PROJECT_DIR, Mode::from_raw_mode(0o755)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(err) => {
                return Err(unwritable(
                    "the folder .humanitl could not be created",
                    &err,
                ));
            }
        }
    }
    let inner = match rustix::fs::openat(
        &root,
        PROJECT_DIR,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        // Nichts zugeordnet und nichts zu entfernen.
        Err(rustix::io::Errno::NOENT) if server.is_none() => return Ok(dir),
        Err(err) => {
            return Err(unwritable(
                ".humanitl is a link or not a folder that can be opened",
                &err,
            ));
        }
    };
    rustix::fs::flock(&inner, FlockOperation::LockExclusive)
        .map_err(|err| unwritable("the folder .humanitl could not be locked", &err))?;

    let (before, mode) = match rustix::fs::openat(
        &inner,
        PROJECT_FILE,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let stat = rustix::fs::fstat(&fd)
                .map_err(|err| unwritable("profile.toml could not be inspected", &err))?;
            if rustix::fs::FileType::from_raw_mode(stat.st_mode)
                != rustix::fs::FileType::RegularFile
            {
                return Err(unwritable("profile.toml is not a regular file", &"refused"));
            }
            let mut text = String::new();
            File::from(fd)
                .read_to_string(&mut text)
                .map_err(|err| unwritable("profile.toml could not be read", &err))?;
            (text, Mode::from_raw_mode(stat.st_mode & 0o777))
        }
        Err(rustix::io::Errno::NOENT) if server.is_none() => return Ok(dir),
        Err(rustix::io::Errno::NOENT) => (String::new(), Mode::from_raw_mode(0o644)),
        Err(err) => {
            return Err(unwritable(
                "profile.toml is a link or cannot be opened",
                &err,
            ));
        }
    };
    let value = server.map(|server| TomlValue::String(server.to_owned()));
    let after =
        match crate::edit::render_edits(&file, &before, &[Edit::new(server_segments(), value)])? {
            Rendered::Unchanged => return Ok(dir),
            Rendered::Changed(after) => after,
        };
    Profile::parse(
        after.strip_prefix('\u{feff}').unwrap_or(&after),
        &ProfileSource::Project(file.clone()),
    )?;

    replace_in(&inner, after.as_bytes(), mode)
        .map_err(|err| unwritable("the new text could not be written", &err))?;
    Ok(dir)
}

/// Schreibt `bytes` als neue Fassung von [`PROJECT_FILE`] in den geöffneten
/// Ordner `dir`: erst eine Nebendatei mit `O_EXCL` und `O_NOFOLLOW`, dann
/// `fsync` und `renameat` in demselben Ordner. Ein Verweis an der Stelle der
/// Datei wird ersetzt, nie verfolgt; scheitert ein Schritt, bleibt keine
/// Nebendatei liegen.
fn replace_in(dir: &std::os::fd::OwnedFd, bytes: &[u8], mode: Mode) -> std::io::Result<()> {
    let tmp = format!(
        ".{PROJECT_FILE}.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    );
    let written = (|| -> std::io::Result<()> {
        let fd = rustix::fs::openat(
            dir,
            tmp.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            mode,
        )?;
        let mut out = File::from(fd);
        out.write_all(bytes)?;
        out.sync_all()?;
        rustix::fs::renameat(dir, tmp.as_str(), dir, PROJECT_FILE)?;
        Ok(())
    })();
    if written.is_err() {
        // Best effort: Die Nebendatei bleibt nicht liegen.
        let _ = rustix::fs::unlinkat(dir, tmp.as_str(), rustix::fs::AtFlags::empty());
    }
    written
}

/// Der Projektordner als aufgelöster, absoluter Ordner.
///
/// # Errors
///
/// `CONFIG_021`, wenn der Pfad nicht absolut ist, nicht da ist oder kein
/// Ordner ist.
pub fn project_dir(project: &Path) -> Result<PathBuf, Diagnostic> {
    let refuse = |what: &str| {
        Diagnostic::builder(CONFIG_021, Severity::Error)
            .why(format!(
                "the project folder {} {what}; nothing was assigned or confirmed",
                project.display()
            ))
            .build()
    };
    if !project.is_absolute() {
        return Err(refuse("is not an absolute path"));
    }
    let dir = std::fs::canonicalize(project).map_err(|_| refuse("does not exist"))?;
    if !dir.is_dir() {
        return Err(refuse("is not a folder"));
    }
    Ok(dir)
}

/// Merkt die Wahl des Servers `server` für den Projektordner `project`.
///
/// # Errors
///
/// `CONFIG_021` für einen Ordner, den [`project_dir`] ablehnt; die Befunde
/// von [`crate::edit::edit_with`], wenn die Datei nicht geschrieben wurde.
pub fn confirm(paths: &Paths, project: &Path, server: &str) -> Result<(), Diagnostic> {
    let dir = project_dir(project)?;
    let key = dir.display().to_string();
    crate::edit::edit_with(
        &paths.project_servers_path(),
        |old| {
            let mut servers: Vec<String> = confirmed_in(old, &key);
            if servers.iter().any(|known| known == server) {
                return Ok(Vec::new());
            }
            servers.push(server.to_owned());
            servers.sort();
            Ok(vec![Edit::new(
                vec!["projects".to_owned(), key.clone(), "servers".to_owned()],
                Some(TomlValue::Array(
                    servers.into_iter().map(TomlValue::String).collect(),
                )),
            )])
        },
        |_| Ok(()),
    )?;
    Ok(())
}

/// Nimmt jede Bestätigung der Server `names` aus allen Projekten (HUM-405).
///
/// Für einen Namen, der gleich einen anderen Server meint: nach dem
/// Entfernen, Umbenennen oder Neuanlegen und nach einem Endpunkt mit anderer
/// Herkunft. Eine Bestätigung gilt dem Server, den der Mensch unter diesem
/// Namen gesehen hat, nicht dem Namen allein.
///
/// # Errors
///
/// Die Befunde von [`crate::edit::edit_with`], wenn die Datei nicht
/// geschrieben wurde; dann bleibt sie, wie sie war.
pub fn forget(paths: &Paths, names: &[String]) -> Result<(), Diagnostic> {
    let file = paths.project_servers_path();
    if !file.exists() {
        return Ok(());
    }
    crate::edit::edit_with(
        &file,
        |old| {
            let mut edits = Vec::new();
            let projects = old
                .get("projects")
                .and_then(TomlValue::as_table)
                .cloned()
                .unwrap_or_default();
            for key in projects.keys() {
                let servers = confirmed_in(old, key);
                let kept: Vec<String> = servers
                    .iter()
                    .filter(|server| !names.contains(server))
                    .cloned()
                    .collect();
                if kept.len() == servers.len() {
                    continue;
                }
                edits.push(Edit::new(
                    vec!["projects".to_owned(), key.clone(), "servers".to_owned()],
                    Some(TomlValue::Array(
                        kept.into_iter().map(TomlValue::String).collect(),
                    )),
                ));
            }
            Ok(edits)
        },
        |_| Ok(()),
    )?;
    Ok(())
}

/// Ob die Wahl des Servers `server` für den Projektordner `project`
/// bestätigt ist. Eine Datei, die sich nicht lesen lässt, bestätigt nichts.
#[must_use]
pub fn is_confirmed(paths: &Paths, project: &Path, server: &str) -> bool {
    let Ok(dir) = std::fs::canonicalize(project) else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(paths.project_servers_path()) else {
        return false;
    };
    let Ok(table) = crate::edit::parse_text(&paths.project_servers_path(), &text) else {
        return false;
    };
    confirmed_in(&table, &dir.display().to_string())
        .iter()
        .any(|known| known == server)
}

/// Die bestätigten Server des Ordners `key` in der Tabelle der Datei.
fn confirmed_in(table: &toml::Table, key: &str) -> Vec<String> {
    table
        .get("projects")
        .and_then(TomlValue::as_table)
        .and_then(|projects| projects.get(key))
        .and_then(TomlValue::as_table)
        .and_then(|entry| entry.get("servers"))
        .and_then(TomlValue::as_array)
        .map(|servers| {
            servers
                .iter()
                .filter_map(TomlValue::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Der Projektordner zu einem Projekt-Profil `<ordner>/.humanitl/profile.toml`.
#[must_use]
pub fn project_of(profile: &Path) -> Option<PathBuf> {
    profile.parent()?.parent().map(Path::to_path_buf)
}

/// Der Befund, mit dem ein Start ablehnt, wenn das Projekt einen anderen
/// gespeicherten Server als den Standard wählt und diese Wahl für diesen
/// Projektordner nicht bestätigt ist (`CONFIG_022`); `None` sonst.
///
/// Kein Befund für einen Namen ohne gespeicherten Server: Den lehnt schon
/// [`crate::servers::ServerChoice::refusal`] ab. Und keiner, wenn das
/// Projekt den Standard wählt: Dann geht der Verkehr dorthin, wohin er ohne
/// das Projekt auch ginge.
#[must_use]
pub fn unconfirmed(paths: &Paths, llm: &LlmConfig) -> Option<Diagnostic> {
    let choice = llm.choice.as_ref()?;
    let ServerSource::Project(profile) = &choice.source else {
        return None;
    };
    // Ohne gespeicherte Einträge ist der Server aus den flachen Werten der
    // Standard, auch wenn `llm.active` fehlt (HUM-289, Review HUM-405).
    let default = llm.active.as_deref().or_else(|| {
        llm.servers
            .is_empty()
            .then_some(crate::servers::MIGRATED_NAME)
    });
    if !choice.known || default == Some(choice.name.as_str()) {
        return None;
    }
    let project = project_of(profile)?;
    if is_confirmed(paths, &project, &choice.name) {
        return None;
    }
    let shown = std::fs::canonicalize(&project).unwrap_or_else(|_| project.clone());
    let default = default.unwrap_or("none");
    Some(
        Diagnostic::builder(CONFIG_022, Severity::Error)
            .why(format!(
                "the project {} chooses the saved model server {} through {} ({SERVER} there, \
                 or in the bundled profile it names), not the default server {default}, and \
                 this choice is not confirmed for this folder; the session does not start until \
                 it is, and no endpoint gets a key",
                shown.display(),
                choice.name,
                profile.display()
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl llm server confirm {} --project {}",
                humanitl_core::shell::shell_word(&choice.name),
                humanitl_core::shell::shell_path(&shown)
            )))
            .build(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::Env;
    use crate::servers::ServerChoice;

    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    fn project_in(dir: &Path) -> PathBuf {
        let project = dir.join("project");
        std::fs::create_dir_all(&project).unwrap();
        project
    }

    fn server_of(profile: &Path) -> Option<String> {
        let table: toml::Table = std::fs::read_to_string(profile).ok()?.parse().ok()?;
        table
            .get("config")?
            .get("llm")?
            .get("server")?
            .as_str()
            .map(ToOwned::to_owned)
    }

    /// Ein mitgeliefertes Profil bekommt eine Kopie mit dem Wert, und nach
    /// dem Herausnehmen verschwindet die Kopie wieder.
    #[test]
    fn a_bundled_profile_gets_a_copy_that_goes_again() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let file = paths.profile_path("llm-only");

        assign_profile(&paths, "llm-only", Some("work")).unwrap();
        assert_eq!(server_of(&file).as_deref(), Some("work"));
        let copy = Profile::parse(
            &std::fs::read_to_string(&file).unwrap(),
            &ProfileSource::File(file.clone()),
        )
        .unwrap();
        assert!(!copy.rules.is_empty(), "the copy keeps the bundled rules");
        let uses = profile_uses(&paths);
        let row = uses.iter().find(|row| row.name == "llm-only").unwrap();
        assert_eq!(row.server.as_deref(), Some("work"));
        assert!(!row.builtin);
        assert!(
            uses.iter().all(|row| row.name != DEFAULT_PROFILE),
            "{uses:?}"
        );

        assign_profile(&paths, "llm-only", None).unwrap();
        assert!(
            !file.exists(),
            "the copy equal to the bundled profile stays"
        );
        let row = profile_uses(&paths)
            .into_iter()
            .find(|row| row.name == "llm-only")
            .unwrap();
        assert_eq!(row.server, None);
        assert!(row.builtin);
    }

    /// Eine eigene Profildatei behält ihren Text und bekommt nur die Zeile.
    #[test]
    fn an_own_profile_keeps_its_text() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let file = paths.profile_path("team");
        std::fs::create_dir_all(paths.profiles_dir()).unwrap();
        std::fs::write(
            &file,
            "# mine\nname = \"team\"\n\n[config.hold]\ntimeout_secs = 9\n",
        )
        .unwrap();

        assign_profile(&paths, "team", Some("work")).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# mine\n"), "{text}");
        assert_eq!(server_of(&file).as_deref(), Some("work"));

        assign_profile(&paths, "team", None).unwrap();
        assert!(file.exists(), "an own profile is never deleted");
        assert_eq!(server_of(&file), None);
    }

    /// `default` gilt für jede Sitzung und wird nicht zugeordnet; ein Profil,
    /// das es nicht gibt, auch nicht.
    #[test]
    fn default_and_unknown_profiles_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let refused = assign_profile(&paths, DEFAULT_PROFILE, Some("work")).unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert!(!paths.profile_path(DEFAULT_PROFILE).exists());
        let refused = assign_profile(&paths, "nowhere", Some("work")).unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert!(!paths.profile_path("nowhere").exists());
    }

    /// Das Projekt-Profil entsteht mit der einen Zeile und verliert sie
    /// wieder; der übrige Text bleibt.
    #[test]
    fn a_project_gets_and_loses_its_server() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_in(dir.path());
        let file = project.join(PROJECT_DIR).join(PROJECT_FILE);

        let resolved = assign_project(&project, Some("work")).unwrap();
        assert_eq!(resolved, std::fs::canonicalize(&project).unwrap());
        assert_eq!(server_of(&file).as_deref(), Some("work"));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains('#'), "nothing but the value: {text}");

        std::fs::write(&file, "# note\n[config.llm]\nserver = \"work\"\n").unwrap();
        assign_project(&project, None).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# note\n"), "{text}");
        assert_eq!(server_of(&file), None);
    }

    /// Ein Verweis an der Stelle von `profile.toml` oder `.humanitl` bekommt
    /// keine Zeile, und sein Ziel bleibt unberührt.
    #[test]
    fn a_link_in_the_project_is_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_in(dir.path());
        let outside = dir.path().join("config.toml");
        std::fs::write(&outside, "[llm]\nactive = \"home\"\n").unwrap();
        std::fs::create_dir_all(project.join(PROJECT_DIR)).unwrap();
        std::os::unix::fs::symlink(&outside, project.join(PROJECT_DIR).join(PROJECT_FILE)).unwrap();

        let refused = assign_project(&project, Some("work")).unwrap_err();
        assert_eq!(refused.code, CONFIG_015);
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "[llm]\nactive = \"home\"\n"
        );

        let other = dir.path().join("elsewhere");
        std::fs::create_dir_all(&other).unwrap();
        let second = project_in(&dir.path().join("second"));
        std::os::unix::fs::symlink(&other, second.join(PROJECT_DIR)).unwrap();
        let refused = assign_project(&second, Some("work")).unwrap_err();
        assert_eq!(refused.code, CONFIG_015);
        assert!(!other.join(PROJECT_FILE).exists());
    }

    /// Eine Bestätigung gilt für genau diesen Ordner und diesen Namen, und
    /// sie liegt nicht im Projekt.
    #[test]
    fn a_confirmation_is_per_folder_and_name_and_outside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let project = project_in(dir.path());
        let other = project_in(&dir.path().join("other"));

        assert!(!is_confirmed(&paths, &project, "work"));
        confirm(&paths, &project, "work").unwrap();
        confirm(&paths, &project, "work").unwrap();
        assert!(is_confirmed(&paths, &project, "work"));
        assert!(!is_confirmed(&paths, &project, "cloud"));
        assert!(!is_confirmed(&paths, &other, "work"));
        assert!(paths.project_servers_path().starts_with(paths.data_dir()));
        assert!(!project.join(PROJECT_DIR).exists());

        let refused = confirm(&paths, Path::new("relative/dir"), "work").unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
    }

    fn llm_choosing(project: &Path, name: &str, active: &str) -> LlmConfig {
        LlmConfig {
            active: Some(active.to_owned()),
            choice: Some(ServerChoice {
                name: name.to_owned(),
                source: ServerSource::Project(project.join(PROJECT_DIR).join(PROJECT_FILE)),
                known: true,
                saved: vec!["home".to_owned(), "work".to_owned()],
            }),
            ..LlmConfig::default()
        }
    }

    /// Nur ein Projekt, das einen anderen Server als den Standard wählt,
    /// wartet auf die Bestätigung; der Befund nennt den Befehl dazu.
    #[test]
    fn only_another_server_of_the_project_waits_for_a_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let project = project_in(dir.path());

        let waiting = unconfirmed(&paths, &llm_choosing(&project, "work", "home")).unwrap();
        assert_eq!(waiting.code, CONFIG_022);
        let Some(FixAction::CopyCommand(command)) = &waiting.fix else {
            panic!("{waiting:?}");
        };
        assert!(
            command.starts_with("humanitl llm server confirm work --project "),
            "{command}"
        );
        assert!(unconfirmed(&paths, &llm_choosing(&project, "home", "home")).is_none());

        confirm(&paths, &project, "work").unwrap();
        assert!(unconfirmed(&paths, &llm_choosing(&project, "work", "home")).is_none());

        let mut profile = llm_choosing(&project, "work", "home");
        if let Some(choice) = profile.choice.as_mut() {
            choice.source = ServerSource::Profile("team".to_owned());
        }
        let fresh = tempfile::tempdir().unwrap();
        assert!(unconfirmed(&paths_in(fresh.path()), &profile).is_none());
    }

    /// Ohne gespeicherte Einträge ist `default` der Standard, auch ohne
    /// `llm.active`: Ein Projekt, das ihn nennt, wartet auf nichts (HUM-405,
    /// Review).
    #[test]
    fn the_implicit_default_is_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let project = project_in(dir.path());
        let mut llm = llm_choosing(&project, crate::servers::MIGRATED_NAME, "home");
        llm.active = None;
        assert!(llm.servers.is_empty());
        assert!(unconfirmed(&paths, &llm).is_none());
    }

    /// `forget` nimmt einen Namen aus jedem Projekt und lässt die anderen.
    #[test]
    fn forget_drops_a_name_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let one = project_in(dir.path());
        let two = project_in(&dir.path().join("two"));
        confirm(&paths, &one, "work").unwrap();
        confirm(&paths, &one, "cloud").unwrap();
        confirm(&paths, &two, "work").unwrap();
        forget(&paths, &["work".to_owned()]).unwrap();
        assert!(!is_confirmed(&paths, &one, "work"));
        assert!(!is_confirmed(&paths, &two, "work"));
        assert!(is_confirmed(&paths, &one, "cloud"));
        let fresh = tempfile::tempdir().unwrap();
        forget(&paths_in(fresh.path()), &["work".to_owned()]).unwrap();
    }
}
