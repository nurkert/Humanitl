//! Woher das Projektverzeichnis einer Sitzung kommt (HUM-293).
//!
//! Das Verzeichnis, das als `/work` eingehängt wird, hat genau eine Quelle:
//! `sandbox.work_dir` in einer Konfigurationsdatei oder einem Profil, eine
//! Umgebungsvariable `HUMANITL_*`, `--work` auf der Kommandozeile, der Ordner,
//! den ein Mensch gerade in der Oberfläche wählt, oder — wenn nichts davon
//! gesetzt ist — das aktuelle Verzeichnis des Aufrufers. Bis HUM-293 nannte
//! jeder `SANDBOX_006` über das Projektverzeichnis `sandbox.work_dir`, auch
//! wenn der Pfad aus dem aktuellen Verzeichnis kam und in keiner Datei stand.
//! Wer dann in `config.toml` nachsah, fand den Schlüssel nicht und hielt den
//! Befund für falsch.
//!
//! [`WorkDirSource`] trägt die Quelle bis zum Befund. Die Prüfung selbst ist
//! dieselbe wie zuvor ([`MountPolicy::check_work_dir_from`]); nur der Satz
//! sagt jetzt, woher der Pfad kam, und die Abhilfe nennt zuerst den Weg, einen
//! Projektordner dauerhaft zu wählen ([`CHOOSE_PROJECT_FOLDER`]).

use std::fmt;
use std::path::{Path, PathBuf};

use humanitl_config::Origin;
use humanitl_core::diagnostics::codes::SANDBOX_006;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::profile::MountPolicy;

/// Der Schlüssel, unter dem der Projektordner dauerhaft gewählt wird.
pub const WORK_DIR_KEY: &str = "sandbox.work_dir";

/// Die Abhilfe eines abgelehnten oder fehlenden Projektordners, wie die
/// Kommandozeile sie zeigt (HUM-293): erst der Weg, einen Ordner dauerhaft zu
/// wählen, dann der Platzhalter für ihn.
///
/// Der Befund trägt dafür `FixAction::ChangeSetting` auf [`WORK_DIR_KEY`] mit
/// leerem Wert; die Oberfläche öffnet daran die Ordnerauswahl, die
/// Kommandozeile druckt diesen Satz.
pub const CHOOSE_PROJECT_FOLDER: &str =
    "choose a project folder: humanitl config set sandbox.work_dir <dir>";

/// Woher ein Projektverzeichnis kommt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkDirSource {
    /// `sandbox.work_dir` in dieser Konfigurationsdatei.
    ConfigFile(PathBuf),
    /// `sandbox.work_dir` in diesem Profil (`profiles/<name>.toml`).
    Profile(String),
    /// `--work` (`--sandbox-work-dir`) auf der Kommandozeile.
    Flag,
    /// Diese Umgebungsvariable, zum Beispiel `HUMANITL_SANDBOX__WORK_DIR`.
    Env(String),
    /// Das aktuelle Verzeichnis des Aufrufers, weil nichts anderes gesetzt
    /// ist.
    CurrentDir,
    /// Der Ordner, den ein Mensch gerade gewählt hat und der in
    /// `sandbox.work_dir` geschrieben werden soll (`SetConfig`).
    Chosen,
    /// Das Feld `work_dir` einer Anfrage, deren Client keine Quelle nennt.
    Request,
}

impl WorkDirSource {
    /// Die Quelle zu der Herkunft, die die Auflösung für `sandbox.work_dir`
    /// aufgeschrieben hat.
    ///
    /// `config_file` ist die Datei, die die Auflösung als globale
    /// `config.toml` gelesen hat; mit `--config` ist das eine andere als die
    /// unter `$XDG_CONFIG_HOME`. Ohne Herkunft, oder mit der des
    /// Vorgabewerts, ist `sandbox.work_dir` nicht gesetzt, und das
    /// Verzeichnis kommt aus dem aktuellen.
    #[must_use]
    pub fn from_origin(origin: Option<&Origin>, config_file: &Path) -> Self {
        match origin {
            Some(Origin::Global) => Self::ConfigFile(config_file.to_path_buf()),
            Some(Origin::ProfileBuiltin(name) | Origin::ProfileGlobal(name)) => {
                Self::Profile(name.clone())
            }
            // Auf der Projekt-Ebene ist der Schlüssel gesperrt
            // (`x-project-scope = "denied"`); stünde er doch dort, wäre die
            // Datei die Quelle.
            Some(Origin::ProfileProject(path)) => Self::ConfigFile(path.clone()),
            Some(Origin::Env(name)) => Self::Env(name.clone()),
            Some(Origin::Cli) => Self::Flag,
            Some(Origin::Default) | None => Self::CurrentDir,
        }
    }
}

impl fmt::Display for WorkDirSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigFile(path) => write!(f, "{WORK_DIR_KEY} in {}", path.display()),
            Self::Profile(name) => write!(f, "{WORK_DIR_KEY} in the profile {name}"),
            Self::Flag => f.write_str("--work"),
            Self::Env(name) => write!(f, "the environment variable {name}"),
            Self::CurrentDir => f.write_str("the current directory"),
            Self::Chosen => f.write_str("the folder just chosen"),
            Self::Request => f.write_str("the work_dir of the request"),
        }
    }
}

/// Der Befund für ein Projektverzeichnis, das keines sein darf: `SANDBOX_006`
/// mit der Quelle im Satz und der Ordnerauswahl als Abhilfe.
#[must_use]
pub fn work_dir_refused(dir: &Path, from: &WorkDirSource, why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_006, Severity::Blocking)
        .why(format!(
            "the project directory from {from} is {}: {why}",
            dir.display()
        ))
        .fix(choose_folder_fix())
        .build()
}

/// Die Abhilfe eines Projektordners: die Ordnerauswahl der Oberfläche,
/// [`CHOOSE_PROJECT_FOLDER`] auf der Kommandozeile.
#[must_use]
pub fn choose_folder_fix() -> FixAction {
    FixAction::ChangeSetting {
        key: WORK_DIR_KEY.to_owned(),
        value: String::new(),
    }
}

/// Prüft ein Projektverzeichnis vor dem Start: absolut, vorhanden, ein
/// Verzeichnis, und nach der Politik der Sandbox erlaubt
/// ([`MountPolicy::check_work_dir_from`]). Jeder Befund nennt `from`.
///
/// Dieselben Fragen stellt der Plan des Backends noch einmal, dort aber ohne
/// Quelle; diese Prüfung steht davor, damit der Mensch den Satz mit der
/// Quelle liest und nicht den allgemeinen.
///
/// # Errors
///
/// `SANDBOX_006` mit Pfad, Quelle und Grund.
pub fn check_project_dir(
    policy: &MountPolicy,
    dir: &Path,
    from: &WorkDirSource,
) -> Result<(), Diagnostic> {
    if dir.as_os_str().is_empty() {
        return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
            .why(format!(
                "no project directory is chosen, and {from} is not one the sandbox mounts"
            ))
            .fix(choose_folder_fix())
            .build());
    }
    if !dir.is_absolute() {
        return Err(work_dir_refused(dir, from, "it is not an absolute path"));
    }
    match std::fs::metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(work_dir_refused(dir, from, "it is not a directory")),
        Err(error) => {
            return Err(work_dir_refused(
                dir,
                from,
                &format!("it cannot be read as a directory: {error}"),
            ));
        }
    }
    policy.check_work_dir_from(dir, from)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn every_source_names_itself() {
        let file = Path::new("/home/u/.config/humanitl/config.toml");
        for (origin, shown) in [
            (
                Some(Origin::Global),
                "sandbox.work_dir in /home/u/.config/humanitl/config.toml",
            ),
            (
                Some(Origin::ProfileGlobal("work".to_owned())),
                "sandbox.work_dir in the profile work",
            ),
            (
                Some(Origin::Env("HUMANITL_SANDBOX__WORK_DIR".to_owned())),
                "the environment variable HUMANITL_SANDBOX__WORK_DIR",
            ),
            (Some(Origin::Cli), "--work"),
            (Some(Origin::Default), "the current directory"),
            (None, "the current directory"),
        ] {
            assert_eq!(
                WorkDirSource::from_origin(origin.as_ref(), file).to_string(),
                shown
            );
        }
    }

    #[test]
    fn only_a_setting_names_the_key() {
        for source in [
            WorkDirSource::Flag,
            WorkDirSource::Env("HUMANITL_SANDBOX__WORK_DIR".to_owned()),
            WorkDirSource::CurrentDir,
            WorkDirSource::Chosen,
            WorkDirSource::Request,
        ] {
            let refused = work_dir_refused(Path::new("/home/u"), &source, "it is your home");
            assert!(
                !refused.why.contains(WORK_DIR_KEY),
                "{source:?}: {}",
                refused.why
            );
            assert_eq!(refused.fix, Some(choose_folder_fix()));
        }
    }

    #[test]
    fn a_missing_or_odd_directory_names_its_source() {
        let dir = tempfile::tempdir().unwrap();
        let policy = MountPolicy::new("/nonexistent-home-of-this-test");
        let file = dir.path().join("file");
        std::fs::write(&file, b"").unwrap();
        for (path, reason) in [
            (dir.path().join("missing"), "cannot be read as a directory"),
            (file, "is not a directory"),
            (PathBuf::from("relative"), "is not an absolute path"),
            (PathBuf::new(), "no project directory is chosen"),
        ] {
            let refused =
                check_project_dir(&policy, &path, &WorkDirSource::CurrentDir).expect_err("refused");
            assert_eq!(refused.code, SANDBOX_006);
            assert!(refused.why.contains(reason), "{}", refused.why);
            assert!(
                refused.why.contains("the current directory"),
                "{}",
                refused.why
            );
        }
        check_project_dir(&policy, dir.path(), &WorkDirSource::Flag).expect("a folder is fine");
    }
}
