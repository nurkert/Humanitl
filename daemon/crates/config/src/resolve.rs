//! Welche Profile gelten, und woher sie kommen.
//!
//! [`mod@crate::load`] mischt Ebenen; hier wird entschieden, welche Profil-Ebenen
//! es überhaupt gibt. Die Reihenfolge steht in `docs/profiles.md` und in
//! `backlog/CONVENTIONS.md` 4.23:
//!
//! 1. eingebaute Vorgabewerte,
//! 2. `$XDG_CONFIG_HOME/humanitl/config.toml`,
//! 3. das Profil `default` — die Datei `profiles/default.toml` im
//!    Konfigurationsverzeichnis, sonst die eingebettete Fassung,
//! 4. das gewählte Profil, falls es nicht `default` ist,
//! 5. `<projekt>/.humanitl/profile.toml`,
//! 6. Umgebungsvariablen `HUMANITL_*`,
//! 7. Argumente der Kommandozeile.
//!
//! **Welches Verzeichnis das Projekt ist, entscheidet `sandbox.work_dir`, nicht
//! das aktuelle Verzeichnis.** Der Schlüssel ist auf der Projekt-Ebene gesperrt
//! (`x-project-scope = "denied"`), deshalb ist die Auflösung zirkelfrei: Erst
//! werden die Ebenen 1 bis 4 samt Umgebung und Kommandozeile geladen, dann
//! steht das Arbeitsverzeichnis fest, und erst dann wird darin nach dem
//! Projekt-Profil gesucht. Wer mit `--work` aus einem fremden Verzeichnis
//! heraus arbeitet, bekommt so das Profil des Projekts, an dem er arbeitet, und
//! nicht das des Verzeichnisses, in dem seine Shell gerade steht.
//!
//! **Ein Projekt darf nur ein mitgeliefertes Profil wählen.** Das
//! Projekt-Profil kommt aus einem geklonten Repository. Dürfte sein `name` ein
//! beliebiges Profil des Nutzers als Ebene 4 einsetzen, hätte ein Repository
//! über diesen Umweg jeden Schlüssel gesetzt, den ihm die Projekt-Ebene
//! verwehrt — `sandbox.profile` und `agent.command` eingeschlossen, also die
//! Einhängefläche der Sandbox und den Prozess darin. `name` wählt deshalb nur
//! unter [`crate::profile::BUILTIN_PROFILES`]; jeder andere Wunsch wird
//! übergangen und gemeldet (`CONFIG_009`). Wer ein eigenes Profil meint,
//! schreibt `--profile` auf die Kommandozeile.
//!
//! **Ein Profil, das das Projekt-Profil wählt, lädt seinen eingebetteten
//! Text**, Schlüssel und Regeln. Eine Datei gleichen Namens im
//! Profilverzeichnis des Nutzers gilt nur, wenn der Mensch das Profil selbst
//! wählt (`--profile`, das Profil der Sitzung). Die Ebene 3, `default`,
//! bleibt davon unberührt: Sie gilt in jeder Sitzung, mit oder ohne Projekt.
//!
//! **Ein Profil, das das Projekt-Profil wählt, steht unter der Grenze des
//! Projekt-Profils.** Die Wahl kommt aus dem Projektordner; [`Sources::project_named`]
//! trägt sie zum Laden, und dort bleibt jeder Schlüssel, den das Projekt-Profil
//! nicht setzen dürfte, und jede Regel außer `block` und `ask` draußen
//! (`named_profile.rs`). Wählt der Mensch das Profil selbst, gilt es
//! ganz.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::CONFIG_001;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::env::Env;
use crate::load::{DEFAULT_ENV_PREFIX, Sources, load};
use crate::origin::Resolved;
use crate::paths::Paths;
use crate::profile::{DEFAULT_PROFILE, ProfileSource, ProfileSummary, builtin_name, check_name};

/// Welches Profil eine Sitzung will.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileSelection {
    /// Der Name des Profils; `None` heißt: das des Projekts, sonst `default`.
    pub name: Option<String>,
}

impl ProfileSelection {
    /// Ohne Wunsch: das Profil des Projekts, sonst `default`.
    #[must_use]
    pub fn any() -> Self {
        Self { name: None }
    }

    /// Genau dieses Profil.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
        }
    }
}

/// Sucht die Quellen im Dateisystem, ausgehend von einem Verzeichnis.
///
/// Bequemlichkeit für Aufrufer ohne eigene Umgebung; die Arbeit macht
/// [`sources_for`].
///
/// # Errors
///
/// Wie [`sources_for`].
pub fn discover(cwd: &Path) -> Result<Sources, Diagnostic> {
    discover_with(&Env::from_process(), cwd, None)
}

/// Wie [`discover`], aber mit übergebener Umgebung und Profilnamen.
///
/// Derselbe Weg wie [`sources_for`], nur ohne Argumente der Kommandozeile: eine
/// zweite, nachsichtigere Auflösung gäbe es sonst neben der strengen, und ein
/// Name, der in der einen ein Fehler und in der anderen ein Achselzucken ist,
/// wäre keine Regel, sondern eine Falle.
///
/// # Errors
///
/// Wie [`sources_for`].
pub fn discover_with(env: &Env, cwd: &Path, profile: Option<&str>) -> Result<Sources, Diagnostic> {
    let selection = ProfileSelection {
        name: profile.map(ToOwned::to_owned),
    };
    sources_for(&selection, Some(cwd), env, &[])
}

/// Löst eine Profilauswahl zur wirksamen Konfiguration auf.
///
/// Ein Name, zu dem es weder eine Datei noch ein mitgeliefertes Profil gibt,
/// ist `CONFIG_001` — auch dann, wenn die Datei da ist, sich aber nicht lesen
/// lässt: Ein Profil, das nicht parst, wird nicht stillschweigend zu „kein
/// Profil". Was das Laden überlebt hat — ein verdecktes mitgeliefertes Profil,
/// ein Projekt-Profil aus fremdem Besitz, ein übergangener Profilwunsch des
/// Projekts — steht in [`Resolved::diagnostics`].
///
/// # Errors
///
/// `CONFIG_001` für ein Profil, das es nicht gibt, oder eine Datei, die sich
/// nicht lesen lässt; `CONFIG_002` für einen unbekannten Schlüssel oder Block;
/// `CONFIG_003` für einen Wert außerhalb seines Bereichs, für einen im
/// Projekt-Profil gesperrten Schlüssel und für einen Profilnamen, der keiner
/// ist.
pub fn resolve(
    selection: &ProfileSelection,
    cwd: Option<&Path>,
    env: &Env,
    cli: &[(String, String)],
) -> Result<Resolved, Diagnostic> {
    load(&sources_for(selection, cwd, env, cli)?)
}

/// [`resolve`] mit der Projektwahl als Ebene über dem Projekt-Profil
/// (HUM-532).
///
/// # Errors
///
/// Wie [`resolve`], dazu `CONFIG_024` für eine Wahl, die einen Schlüssel
/// außerhalb der Liste trägt oder die Untergrenze der Isolation senkt.
pub fn resolve_with_choice(
    selection: &ProfileSelection,
    cwd: Option<&Path>,
    env: &Env,
    cli: &[(String, String)],
    choice: Option<&crate::ProjectChoice>,
) -> Result<Resolved, Diagnostic> {
    load(&sources_for(selection, cwd, env, cli)?.with_project_choice(choice.cloned()))
}

/// Die Quellen, die eine Profilauswahl bestimmt, ohne sie schon zu laden.
///
/// Der Einstieg für alle, die vor dem Laden noch etwas an den Quellen ändern
/// müssen — die Kommandozeile etwa, deren `--config` eine andere `config.toml`
/// nennt.
///
/// `cwd` ist der Rückfall, nicht die Antwort: Das Projektverzeichnis ist
/// `sandbox.work_dir`, sobald es gesetzt ist. Dafür werden die Ebenen ohne das
/// Projekt-Profil einmal vorab geladen; das geht, weil `sandbox.work_dir` auf
/// der Projekt-Ebene gesperrt ist und die Vorabrunde deshalb nichts übersehen
/// kann.
///
/// # Errors
///
/// `CONFIG_003`, wenn der gewählte Name keiner ist, `CONFIG_001`, wenn es zu
/// ihm kein Profil gibt, und alles, woran die Vorabrunde scheitert (eine
/// unlesbare `config.toml`, ein Wert außerhalb seines Bereichs).
pub fn sources_for(
    selection: &ProfileSelection,
    cwd: Option<&Path>,
    env: &Env,
    cli: &[(String, String)],
) -> Result<Sources, Diagnostic> {
    let global = Paths::new(env.clone()).config_path();
    sources_with_global(selection, cwd, env, cli, global.is_file().then_some(global))
}

/// Wie [`sources_for`], aber mit dieser Datei als globaler `config.toml` —
/// auch in der Vorabrunde, die das Projektverzeichnis bestimmt.
///
/// Für alle, die eine andere Datei an die Stelle der globalen setzen: `--config`
/// der Kommandozeile und die Probe von `humanitl config set`, die eine
/// Nebendatei lädt, bevor sie die wirkliche ersetzt. Wer die Datei erst nach
/// [`sources_for`] tauscht, bekommt das Projekt-Profil der alten Datei: Setzt
/// die neue `sandbox.work_dir`, stünde in den Quellen das Profil des falschen
/// Projekts (HUM-070).
///
/// # Errors
///
/// Wie [`sources_for`].
pub fn sources_with_global(
    selection: &ProfileSelection,
    cwd: Option<&Path>,
    env: &Env,
    cli: &[(String, String)],
    global: Option<PathBuf>,
) -> Result<Sources, Diagnostic> {
    let paths = Paths::new(env.clone());
    if let Some(name) = selection.name.as_deref() {
        check_name(name, "the profile selection")?;
    }

    // Erste Runde, ohne Projekt-Ebene: Sie beantwortet nur eine Frage, nämlich
    // welches Verzeichnis das Projekt ist.
    let requested = selection.name.as_deref().unwrap_or(DEFAULT_PROFILE);
    let preliminary = Sources {
        global_toml: global.clone(),
        profiles: layers_or_refuse(&paths, requested)?,
        profile_project: None,
        project_named: None,
        project_choice: None,
        policy: crate::PolicyLocks::of_policy(&paths),
        env_prefix: DEFAULT_ENV_PREFIX,
        env: env.clone(),
        cli: cli.to_vec(),
    };
    let work_dir = load(&preliminary)?
        .config
        .sandbox
        .work_dir
        .or_else(|| cwd.map(Path::to_path_buf));

    let project = work_dir
        .map(|dir| paths.project_profile_path(&dir))
        .filter(|path| path.is_file());
    let wish = project_wish(selection.name.as_deref(), project.as_deref());
    let profiles = match wish {
        Some(name) => embedded_layers(&paths, name),
        None => layers_or_refuse(&paths, requested)?,
    };

    Ok(Sources {
        global_toml: global,
        profiles,
        profile_project: project,
        project_named: wish.map(str::to_owned),
        project_choice: None,
        policy: crate::PolicyLocks::of_policy(&paths),
        env_prefix: DEFAULT_ENV_PREFIX,
        env: env.clone(),
        cli: cli.to_vec(),
    })
}

/// Die Profil-Ebenen für einen Namen, oder der Befund, dass es ihn nicht gibt.
fn layers_or_refuse(paths: &Paths, selected: &str) -> Result<Vec<ProfileSource>, Diagnostic> {
    let profiles = profile_layers(paths, selected);
    if selected != DEFAULT_PROFILE && !profiles.iter().any(|source| source.name() == selected) {
        return Err(unknown_profile(paths, selected));
    }
    Ok(profiles)
}

/// Das mitgelieferte Profil, das das Projekt-Profil für die Ebene 4 wählt.
///
/// Die Kommandozeile geht vor: Nennt der Mensch ein Profil, wählt das Projekt
/// keines (`None`). Ohne sie darf das Projekt-Profil mit `name` seine Basis
/// benennen, aber nur unter den mitgelieferten Profilen: Ein geklontes
/// Repository, das ein beliebiges Profil des Nutzers einsetzen dürfte, käme
/// über diesen Umweg an jeden Schlüssel, den ihm die Projekt-Ebene verwehrt.
/// Ein anderer Wunsch wird übergangen; den Befund dazu (`CONFIG_009`) erzeugt
/// das Laden, wo das Projekt-Profil ohnehin gelesen wird.
///
/// `default` ist keine Wahl: Die Ebene 3 ist in jeder Sitzung `default`, mit
/// der Datei des Nutzers, wenn es sie gibt, sonst dem eingebetteten Text.
/// Nennt das Projekt-Profil `default`, kommt keine Ebene dazu, und Reihenfolge
/// und Herkunft jedes Schlüssels bleiben dieselben wie ohne den Namen.
fn project_wish(requested: Option<&str>, project: Option<&Path>) -> Option<&'static str> {
    if requested.is_some() {
        return None;
    }
    project
        .map(|path| ProfileSource::Project(path.to_owned()))
        .and_then(|source| source.load().ok())
        .and_then(|profile| profile.declared_name)
        .and_then(|name| builtin_name(&name))
        .filter(|name| *name != DEFAULT_PROFILE)
}

/// Die Profil-Ebenen 3 und 4, wenn das Projekt-Profil die Ebene 4 wählt.
///
/// Die Ebene 3 wie immer ([`profile_layers`]), die Ebene 4 als eingebetteter
/// Text des mitgelieferten Profils, Schlüssel und Regeln. Eine Datei gleichen
/// Namens im Profilverzeichnis gehört dem Menschen und gilt nur, wenn er das
/// Profil selbst wählt; die Wahl des Projekts lädt sie nie.
fn embedded_layers(paths: &Paths, name: &'static str) -> Vec<ProfileSource> {
    let mut sources = profile_layers(paths, DEFAULT_PROFILE);
    sources.push(ProfileSource::Builtin(name));
    sources
}

/// Die Profil-Ebenen 3 und 4 für einen gewählten Namen.
///
/// Ebene 3 ist immer `default`; sie fehlt nie, weil das Profil eingebettet ist.
/// Ebene 4 kommt dazu, wenn ein anderes Profil gewählt wurde und es dieses
/// gibt.
#[must_use]
pub fn profile_layers(paths: &Paths, selected: &str) -> Vec<ProfileSource> {
    let mut sources = Vec::new();
    for name in [DEFAULT_PROFILE, selected] {
        if sources.iter().any(|source: &ProfileSource| {
            matches!(source, ProfileSource::Builtin(known) if *known == name)
                || matches!(source, ProfileSource::File(path) if path == &paths.profile_path(name))
        }) {
            continue;
        }
        if let Some(source) = layer(paths, name) {
            sources.push(source);
        }
    }
    sources
}

/// Eine Profil-Ebene: die Datei des Nutzers, sonst die eingebettete Fassung.
///
/// Der Name wird hier geprüft und nicht nur dort, wo ein Mensch ihn tippt: Ein
/// Aufrufer, der `../../etc/hosts` durchreicht, soll keine Datei außerhalb des
/// Profilverzeichnisses als Ebene 4 bekommen. Die Unmöglichkeit steht damit im
/// Code und nicht nur in der Bedienung.
fn layer(paths: &Paths, name: &str) -> Option<ProfileSource> {
    check_name(name, "a profile layer").ok()?;
    let path = paths.profile_path(name);
    if path.is_file() {
        return Some(ProfileSource::File(path));
    }
    builtin_name(name).map(ProfileSource::Builtin)
}

/// Ob es ein Profil dieses Namens gibt.
///
/// Gefragt wird nach der Datei, nicht nach ihrem Inhalt: Ein Profil, das
/// existiert, sich aber nicht lesen lässt, ist ein Profil und wird beim Laden
/// zu `CONFIG_001`. Wäre es hier „kein Profil", verschwände ein kaputtes Profil
/// stillschweigend und `--profile work` bekäme eine andere Bedeutung.
#[must_use]
pub fn profile_exists(paths: &Paths, name: &str) -> bool {
    check_name(name, "a profile name").is_ok()
        && (paths.profile_path(name).is_file() || builtin_name(name).is_some())
}

/// Der Befund für ein Profil, das es nicht gibt.
fn unknown_profile(paths: &Paths, name: &str) -> Diagnostic {
    let known = available_profiles(paths)
        .0
        .into_iter()
        .map(|summary| summary.name)
        .collect::<Vec<_>>()
        .join(", ");
    Diagnostic::builder(CONFIG_001, Severity::Error)
        .why(format!(
            "there is no profile {name}: neither {} nor a bundled one. Known profiles: {known}.",
            paths.profile_path(name).display()
        ))
        .fix(FixAction::CopyCommand(
            "humanitl config schema --profiles".to_owned(),
        ))
        .build()
}

/// Alle Profile, die `--profile` wählen kann, nach Namen sortiert.
///
/// Die mitgelieferten und alles, was als `*.toml` im Profilverzeichnis liegt;
/// eine Datei mit dem Namen eines mitgelieferten Profils ersetzt es in der
/// Liste. Das Unterverzeichnis `sandbox/` bleibt draußen: dort liegen die
/// bwrap-Profile aus HUM-010, die etwas anderes sind. Zurück kommen zusätzlich
/// die Befunde der Dateien, die sich nicht lesen ließen — eine Liste, die ein
/// kaputtes Profil verschweigt, wäre die schlechtere Liste.
#[must_use]
pub fn available_profiles(paths: &Paths) -> (Vec<ProfileSummary>, Vec<Diagnostic>) {
    let mut summaries: Vec<ProfileSummary> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut push = |source: ProfileSource, diagnostics: &mut Vec<Diagnostic>| {
        // Auch der Fehlschlag bekommt eine Zeile, und zwar an derselben Stelle
        // wie ein Erfolg: Eine unlesbare Datei, die ein mitgeliefertes Profil
        // verdeckt, ersetzt es in der Liste, statt es als brauchbar stehen zu
        // lassen. `layer` nimmt in dieser Lage die Datei, und jeder Aufruf
        // endete mit `CONFIG_001`; eine Liste, die das nicht zeigt, lädt genau
        // dazu ein.
        let summary = match source.load() {
            Ok(profile) => profile.summary(),
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                ProfileSummary::broken(source)
            }
        };
        match summaries
            .iter_mut()
            .find(|known| known.name == summary.name)
        {
            Some(slot) => *slot = summary,
            None => summaries.push(summary),
        }
    };

    for name in crate::profile::builtin_names() {
        push(ProfileSource::Builtin(name), &mut diagnostics);
    }
    if let Ok(entries) = std::fs::read_dir(paths.profiles_dir()) {
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        files.sort();
        for path in files {
            push(ProfileSource::File(path), &mut diagnostics);
        }
    }
    summaries.sort_by(|left, right| left.name.cmp(&right.name));
    (summaries, diagnostics)
}

/// Welche Profile der Sitzung einen gespeicherten Server wählen (HUM-400):
/// je Servername die Namen der Profile, deren `llm.server` ihn nennt, nach
/// Namen sortiert.
///
/// Dieselben Profile wie [`available_profiles`], also die mitgelieferten
/// und die Dateien des Profilverzeichnisses, nie ein Projekt-Profil: Der
/// Daemon kennt kein Verzeichnis aller Projekte. Ein Profil, das sich nicht
/// lesen lässt, wählt nichts; [`available_profiles`] meldet es.
#[must_use]
pub fn profile_servers(paths: &Paths) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut uses: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let (summaries, _) = available_profiles(paths);
    for summary in summaries.into_iter().filter(|summary| !summary.broken) {
        let Ok(profile) = summary.source.load() else {
            continue;
        };
        if let Some(server) = profile
            .overlay
            .get(crate::servers::SERVER)
            .and_then(toml::Value::as_str)
        {
            uses.entry(server.to_owned())
                .or_default()
                .push(summary.name);
        }
    }
    uses
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::ProfileSelection;

    #[test]
    fn a_selection_is_either_a_name_or_nothing() {
        assert_eq!(ProfileSelection::any().name, None);
        assert_eq!(
            ProfileSelection::named("llm-only").name.as_deref(),
            Some("llm-only")
        );
    }
}
