//! Das Projektverzeichnis, wie es über den Socket hereinkommt (HUM-293).
//!
//! Zwei Wege bringen einen Ordner zum Daemon: `SetConfig` mit
//! `sandbox.work_dir`, wenn ein Mensch in der Oberfläche einen Ordner wählt,
//! und `work_dir` in `Sandbox(Plan)` oder `Sandbox(Start)`, wenn die
//! Kommandozeile für einen Aufruf einen nennt (`--work`, das aktuelle
//! Verzeichnis, ihre eigene Konfiguration). Beide prüft [`check_requested`]
//! mit denselben drei Stufen; nur so gilt, was das Setup grün zeigt, auch für
//! den Start, und nur so kommt in `config.toml` kein Ordner, den der nächste
//! Start verweigerte.
//!
//! Die Quelle eines Pfades reist als [`WorkDirSource`] mit und steht in jedem
//! `SANDBOX_006` ([`humanitl_sandbox::work_dir`]).

use std::path::{Path, PathBuf};

use humanitl_config::{Paths, Resolved};
use humanitl_core::block::sanitize_note;
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction};
use humanitl_sandbox::{MountPolicy, WORK_DIR_KEY, WorkDirSource, work_dir_refused};

use crate::v1;

/// Prüft ein Projektverzeichnis, das über die RPC hereinkommt, und liefert
/// den aufgelösten Pfad.
///
/// Drei Stufen, und alle drei müssen halten:
///
/// 1. **Es ist ein Verzeichnis.** Absolut, ohne `..`, vorhanden, und nach dem
///    Auflösen der Symlinks immer noch ein Verzeichnis. Der aufgelöste Pfad
///    ist der, der weiterreist: Sonst zeigte der Bildschirm den geschriebenen
///    und die Sandbox hängte den aufgelösten ein.
/// 2. **Die Politik der Sandbox** ([`MountPolicy::check_work_dir_from`]) — die
///    Denylist, das Heimatverzeichnis selbst, die Verzeichnisse von Humanitl,
///    alles, was darüber liegt.
/// 3. **Es liegt dort, wo Projekte liegen**: unter dem Heimatverzeichnis, oder
///    es ist genau das Verzeichnis, das `configured` (der Wert von
///    `sandbox.work_dir` in der Konfiguration) nennt. Stufe 2 kennt `/etc`,
///    `/usr` und `/var/lib` nicht, und ein Bildschirm, der „der Agent sieht
///    nur dein Projekt" verspricht und `/etc` zeigt, hat die Zusage
///    gebrochen. Wer ein Projekt außerhalb des Heimatverzeichnisses hat,
///    schreibt es mit `humanitl config set` oder von Hand in `config.toml`;
///    dort steht die Erklärung eines Menschen und nicht der Wunsch eines
///    Clients. Erklärt ist dabei der Ordner, den der Text in `configured`
///    selbst nennt, also nur, solange er schon der aufgelöste Pfad ist
///    (HUM-294): Wird später ein Teil des Pfades durch einen Verweis ersetzt,
///    zeigt derselbe Text auf einen anderen Ordner, und den hat niemand
///    erklärt. `SetConfig` und `humanitl config set` schreiben deshalb den
///    aufgelösten Pfad.
///
/// Ein gespeicherter Ordner geht bei jedem Start durch alle drei Stufen
/// (`dir` und `configured` sind dann derselbe Pfad, HUM-294); vorher galt er
/// über die Ausnahme von Stufe 3 immer als erklärt.
///
/// # Errors
///
/// `SANDBOX_006` mit dem Grund und der Quelle `from`.
pub fn check_requested(
    paths: &Paths,
    configured: Option<&Path>,
    dir: &Path,
    from: &WorkDirSource,
) -> Result<PathBuf, Diagnostic> {
    if !dir.is_absolute() {
        return Err(work_dir_refused(
            dir,
            from,
            "a project directory is an absolute path",
        ));
    }
    if dir
        .components()
        .any(|part| part == std::path::Component::ParentDir)
    {
        return Err(work_dir_refused(
            dir,
            from,
            "a project directory is written out, without `..`",
        ));
    }
    let resolved = dir.canonicalize().map_err(|error| {
        work_dir_refused(
            dir,
            from,
            &format!("it cannot be read as a directory: {error}"),
        )
    })?;
    if !resolved.is_dir() {
        return Err(work_dir_refused(dir, from, "it is not a directory"));
    }
    // Geschrieben und aufgelöst: Ist `$HOME` selbst ein Verweis (Silverblue:
    // `/home/u` auf `/var/home/u`), kennt die Politik nur die geschriebene
    // Form, und erst der geschriebene Pfad trifft „das Heimatverzeichnis
    // selbst".
    // Die aufgelöste Form prüft eine Politik über dem aufgelösten
    // Heimatverzeichnis: Wer `/var/home/u` schreibt, trifft sonst keine Regel
    // über `/home/u`.
    let home = paths.home();
    let home_resolved = home.canonicalize().unwrap_or_else(|_| home.clone());
    let policy = MountPolicy::from_paths(paths);
    policy.check_work_dir_from(dir, from)?;
    policy.check_work_dir_from(&resolved, from)?;
    if home_resolved != home {
        let resolved_paths = Paths::new(
            paths
                .env()
                .clone()
                .with("HOME", home_resolved.display().to_string()),
        );
        MountPolicy::from_paths(&resolved_paths).check_work_dir_from(&resolved, from)?;
    }

    // Verglichen wird mit dem aufgelösten Heimatverzeichnis, sonst fiele
    // jedes Projekt unter einem verlinkten `$HOME` aus Stufe 3 heraus (Review
    // HUM-293). Lässt es sich nicht auflösen, gilt die geschriebene Form.
    let allowed = resolved.starts_with(&home_resolved)
        || resolved.starts_with(&home)
        || configured.is_some_and(|configured| declared(configured) == Some(resolved.clone()));
    if !allowed && configured.is_some_and(|configured| configured == dir) {
        // Der erklärte Text selbst führt jetzt woandershin: Ein Teil seines
        // Pfades ist ein Verweis geworden, oder er war nie der aufgelöste
        // Pfad (von Hand geschrieben). Der Satz nennt beide Formen, damit der
        // Mensch sieht, welcher Ordner eingehängt würde.
        return Err(work_dir_refused(
            dir,
            from,
            &format!(
                "it resolves to {}, outside {}: a symbolic link in its path leads elsewhere, \
                 and a folder outside your home directory counts as declared only under the \
                 path it really has; if {} is the right folder, declare it with humanitl config \
                 set {WORK_DIR_KEY} {}",
                resolved.display(),
                home.display(),
                resolved.display(),
                resolved.display()
            ),
        ));
    }
    if !allowed {
        if let Some(refused) = named_file_refused(paths, dir, &resolved, from) {
            return Err(refused);
        }
        return Err(work_dir_refused(
            dir,
            from,
            &format!(
                "it lies outside {}; a project directory comes from your home directory, and a \
                 folder anywhere else is mounted only once a person declares it with humanitl \
                 config set {WORK_DIR_KEY} <dir>",
                home.display()
            ),
        ));
    }
    Ok(resolved)
}

/// Der Befund für einen Ordner außerhalb des Heimatverzeichnisses, den eine
/// andere Datei als die Konfiguration des Daemons nennt (HUM-296), sonst
/// nichts.
///
/// `humanitl run --config other.toml` schickt `sandbox.work_dir` aus
/// `other.toml` mit dieser Datei als Quelle. Erklärt ist ein Ordner außerhalb
/// von `$HOME` aber nur in der Datei, die der Daemon selbst liest
/// ([`Paths::config_path`]); eine Datei, die ein Client nennt, liest er dafür
/// nicht. Sonst könnte jeder Prozess, der den Socket erreicht, mit einer
/// selbst geschriebenen Datei einen beliebigen Ordner zur Erklärung machen,
/// den `SetConfig` aus demselben Grund ablehnt. Der Satz sagt das, und die
/// Abhilfe ist der Befehl, der den Ordner in genau diese Datei schreibt.
fn named_file_refused(
    paths: &Paths,
    dir: &Path,
    resolved: &Path,
    from: &WorkDirSource,
) -> Option<Diagnostic> {
    let WorkDirSource::ConfigFile(named) = from else {
        return None;
    };
    let daemon_file = paths.config_path();
    // Derselbe Ort zählt, nicht dieselbe Schreibweise: Ein Verweis auf die
    // Datei des Daemons ist keine andere Datei. Lässt sich ein Pfad nicht
    // auflösen, gilt er, wie er geschrieben ist. Das betrifft nur den Satz;
    // erklärt ist weiterhin allein, was der Daemon selbst gelesen hat.
    let located = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if located(named) == located(&daemon_file) {
        return None;
    }
    let mut refused = work_dir_refused(
        dir,
        from,
        &format!(
            "it lies outside {}, and a folder outside your home directory counts as declared \
             only in the daemon's config file {}, not in a config file other than the \
             daemon's ({}); declare it there",
            paths.home().display(),
            daemon_file.display(),
            named.display()
        ),
    );
    refused.fix = Some(FixAction::CopyCommand(format!(
        "humanitl --config {} config set {WORK_DIR_KEY} {}",
        shell_path(&daemon_file),
        shell_path(resolved)
    )));
    Some(refused)
}

/// Der Ordner, den der erklärte Text `configured` nennt: sein aufgelöster
/// Pfad, aber nur, wenn der Text schon dieser Pfad ist (HUM-294). Führt ein
/// Verweis im Pfad woandershin, oder ist der Ordner nicht da, ist nichts
/// erklärt.
fn declared(configured: &Path) -> Option<PathBuf> {
    configured
        .canonicalize()
        .ok()
        .filter(|resolved| resolved == configured)
}

/// Die Quelle, die ein Client zu seinem `work_dir` nennt.
///
/// Der Text von `detail` kommt vom Client und steht nur im Satz des Befunds;
/// er wird wie die Notiz einer Entscheidung bereinigt (eine Zeile, keine
/// Steuerzeichen, begrenzte Länge). Ohne Angabe — ein Client vor HUM-293 —
/// ist die Quelle die Anfrage selbst.
#[must_use]
pub fn source_from_proto(source: Option<&v1::WorkDirSource>) -> WorkDirSource {
    use v1::work_dir_source::Kind;

    let Some(source) = source else {
        return WorkDirSource::Request;
    };
    let detail = sanitize_note(&source.detail);
    match Kind::try_from(source.kind).unwrap_or(Kind::Unspecified) {
        Kind::Flag => WorkDirSource::Flag,
        Kind::CurrentDirectory => WorkDirSource::CurrentDir,
        Kind::ConfigFile if !detail.is_empty() => WorkDirSource::ConfigFile(PathBuf::from(detail)),
        Kind::Environment if !detail.is_empty() => WorkDirSource::Env(detail),
        Kind::Profile if !detail.is_empty() => WorkDirSource::Profile(detail),
        Kind::ConfigFile | Kind::Environment | Kind::Profile | Kind::Unspecified => {
            WorkDirSource::Request
        }
    }
}

/// Die Quelle als Feld des Vertrags, für einen Client, der `work_dir` schickt.
#[must_use]
pub fn source_to_proto(source: &WorkDirSource) -> v1::WorkDirSource {
    use v1::work_dir_source::Kind;

    let (kind, detail) = match source {
        WorkDirSource::Flag => (Kind::Flag, String::new()),
        WorkDirSource::CurrentDir => (Kind::CurrentDirectory, String::new()),
        WorkDirSource::ConfigFile(path) => (Kind::ConfigFile, path.display().to_string()),
        WorkDirSource::Env(name) => (Kind::Environment, name.clone()),
        WorkDirSource::Profile(name) => (Kind::Profile, name.clone()),
        WorkDirSource::Chosen | WorkDirSource::Request => (Kind::Unspecified, String::new()),
    };
    v1::WorkDirSource {
        kind: kind.into(),
        detail,
    }
}

/// Woher `sandbox.work_dir` in dieser Auflösung kommt; ohne Wert das
/// aktuelle Verzeichnis.
///
/// `config_file` ist die Datei, die als globale `config.toml` gelesen wurde.
#[must_use]
pub fn configured_source(resolved: &Resolved, config_file: &Path) -> WorkDirSource {
    if resolved.config.sandbox.work_dir.is_none() {
        return WorkDirSource::CurrentDir;
    }
    WorkDirSource::from_origin(resolved.origin(WORK_DIR_KEY), config_file)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::Env;
    use humanitl_core::diagnostics::codes::SANDBOX_006;

    use super::*;

    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
            ("XDG_RUNTIME_DIR", dir.join("run").display().to_string()),
        ]))
    }

    #[test]
    fn every_source_survives_the_wire() {
        for source in [
            WorkDirSource::Flag,
            WorkDirSource::CurrentDir,
            WorkDirSource::ConfigFile(PathBuf::from("/home/u/.config/humanitl/config.toml")),
            WorkDirSource::Env("HUMANITL_SANDBOX__WORK_DIR".to_owned()),
            WorkDirSource::Profile("work".to_owned()),
        ] {
            let wire = source_to_proto(&source);
            assert_eq!(source_from_proto(Some(&wire)), source);
        }
        assert_eq!(source_from_proto(None), WorkDirSource::Request);
    }

    #[test]
    fn a_client_text_stays_one_line() {
        let wire = v1::WorkDirSource {
            kind: v1::work_dir_source::Kind::Environment.into(),
            detail: "A\nfix: rm -rf ~\u{1b}[2J".to_owned(),
        };
        let WorkDirSource::Env(name) = source_from_proto(Some(&wire)) else {
            panic!("an environment variable");
        };
        assert!(!name.contains('\n') && !name.contains('\u{1b}'), "{name:?}");
    }

    #[test]
    fn home_and_a_folder_outside_it_are_refused_with_their_source() {
        let home = tempfile::tempdir().unwrap();
        let paths = paths_in(home.path());
        let refused = check_requested(&paths, None, home.path(), &WorkDirSource::Chosen)
            .expect_err("home is no project");
        assert_eq!(refused.code, SANDBOX_006);
        assert!(
            refused.why.contains("the folder just chosen"),
            "{}",
            refused.why
        );
        assert!(!refused.why.contains(WORK_DIR_KEY), "{}", refused.why);

        let outside = tempfile::tempdir().unwrap();
        let refused = check_requested(&paths, None, outside.path(), &WorkDirSource::Flag)
            .expect_err("outside home and not configured");
        assert!(refused.why.contains("from --work"), "{}", refused.why);
        check_requested(
            &paths,
            Some(outside.path()),
            outside.path(),
            &WorkDirSource::Flag,
        )
        .expect("the configured folder is allowed");

        let project = home.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        assert_eq!(
            check_requested(&paths, None, &project, &WorkDirSource::Chosen).unwrap(),
            project.canonicalize().unwrap()
        );
    }

    /// `$HOME` als Verweis (Silverblue: `/home/u` zeigt auf `/var/home/u`):
    /// Ein Projekt darunter ist ein Projekt, das Heimatverzeichnis selbst
    /// bleibt abgelehnt, auf beiden Schreibweisen (Review HUM-293).
    #[test]
    fn a_project_under_a_linked_home_is_accepted_and_the_home_is_not() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("var/home/u");
        std::fs::create_dir_all(real.join("Documents/x")).unwrap();
        let link = root.path().join("home-u");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let paths = paths_in(&link);
        let project = link.join("Documents/x");
        assert_eq!(
            check_requested(&paths, None, &project, &WorkDirSource::Chosen)
                .expect("a project under the linked home"),
            real.join("Documents/x").canonicalize().unwrap()
        );
        for home in [link.clone(), real.clone()] {
            let refused = check_requested(&paths, None, &home, &WorkDirSource::Chosen)
                .expect_err("the home itself is no project");
            assert_eq!(refused.code, SANDBOX_006, "{}", refused.why);
        }
    }

    /// Der erklärte Ordner außerhalb des Heimatverzeichnisses gilt nur unter
    /// seinem aufgelösten Pfad (HUM-294): Ersetzt ein Verweis später einen
    /// Teil des Pfades, nennt derselbe Text einen anderen Ordner.
    #[test]
    fn a_declaration_holds_only_for_the_path_it_really_names() {
        let home = tempfile::tempdir().unwrap();
        let paths = paths_in(home.path());
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let declared = root.join("parent/x");
        std::fs::create_dir_all(&declared).unwrap();
        let source = WorkDirSource::ConfigFile(PathBuf::from("/c/config.toml"));
        check_requested(&paths, Some(&declared), &declared, &source)
            .expect("declared under its real path");

        std::fs::rename(root.join("parent"), root.join("moved")).unwrap();
        std::fs::create_dir_all(root.join("other/x")).unwrap();
        std::os::unix::fs::symlink(root.join("other"), root.join("parent")).unwrap();
        let refused = check_requested(&paths, Some(&declared), &declared, &source)
            .expect_err("the text now names another folder");
        assert_eq!(refused.code, SANDBOX_006);
        assert!(
            refused.why.contains("sandbox.work_dir in /c/config.toml"),
            "{}",
            refused.why
        );
        assert!(
            refused
                .why
                .contains(&root.join("other/x").display().to_string()),
            "{}",
            refused.why
        );
        // Ein Ordner über die Leitung, der auf dasselbe Ziel zeigt, ist so
        // wenig erklärt.
        check_requested(
            &paths,
            Some(&declared),
            &root.join("other/x"),
            &WorkDirSource::Flag,
        )
        .expect_err("the link target was never declared");
    }

    /// Ein Ordner außerhalb des Heimatverzeichnisses aus einer Datei, die
    /// der Client mit `--config` nennt, ist nicht erklärt; der Befund nennt
    /// die Datei des Daemons, und die Abhilfe schreibt genau dorthin
    /// (HUM-296). Steht er in der Datei des Daemons, startet er, gleich
    /// welche Datei der Client nennt.
    #[test]
    fn only_the_daemons_config_file_declares_a_folder_outside_home() {
        let home = tempfile::tempdir().unwrap();
        let paths = paths_in(home.path());
        let outside = tempfile::tempdir().unwrap();
        let outside = outside.path().canonicalize().unwrap();
        let named = WorkDirSource::ConfigFile(home.path().join("other.toml"));

        let refused = check_requested(&paths, None, &outside, &named)
            .expect_err("a file the client names declares nothing");
        assert_eq!(refused.code, SANDBOX_006);
        let daemon_file = paths.config_path();
        assert!(
            refused.why.contains(&format!(
                "only in the daemon's config file {}",
                daemon_file.display()
            )),
            "{}",
            refused.why
        );
        assert_eq!(
            refused.fix,
            Some(FixAction::CopyCommand(format!(
                "humanitl --config {} config set {WORK_DIR_KEY} {}",
                daemon_file.display(),
                outside.display()
            )))
        );

        // Die Datei des Daemons als Quelle behält den allgemeinen Satz und
        // die Ordnerauswahl.
        let own = WorkDirSource::ConfigFile(daemon_file.clone());
        let refused = check_requested(&paths, None, &outside, &own).expect_err("not declared");
        assert!(
            !refused.why.contains("other than the daemon's"),
            "{}",
            refused.why
        );
        assert_eq!(
            refused.fix,
            Some(humanitl_sandbox::choose_folder_fix()),
            "{}",
            refused.why
        );

        // Ein Verweis auf die Datei des Daemons ist keine andere Datei.
        std::fs::create_dir_all(daemon_file.parent().unwrap()).unwrap();
        std::fs::write(&daemon_file, "").unwrap();
        let link = home.path().join("link.toml");
        std::os::unix::fs::symlink(&daemon_file, &link).unwrap();
        let refused = check_requested(&paths, None, &outside, &WorkDirSource::ConfigFile(link))
            .expect_err("not declared");
        assert!(
            !refused.why.contains("other than the daemon's"),
            "{}",
            refused.why
        );
        assert_eq!(refused.fix, Some(humanitl_sandbox::choose_folder_fix()));

        check_requested(&paths, Some(&outside), &outside, &named)
            .expect("declared in the daemon's file");
    }

    /// Die Abhilfe ist ein Befehl zum Kopieren: Datei des Daemons und Ordner
    /// mit Leerzeichen und `'` kommen als je ein Wort der Shell an (HUM-296).
    #[test]
    fn the_fix_quotes_the_daemons_file_and_the_folder_for_the_shell() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let paths = Paths::new(Env::from_pairs([
            ("HOME", home.display().to_string()),
            (
                "XDG_CONFIG_HOME",
                root.join("Niko's config").display().to_string(),
            ),
        ]));
        let outside = root.join("Data 'x' y");
        std::fs::create_dir_all(&outside).unwrap();
        let named = WorkDirSource::ConfigFile(root.join("other.toml"));

        let refused = check_requested(&paths, None, &outside, &named).expect_err("not declared");
        // Druckbares ASCII mit einzelnen Leerzeichen steht in einfachen
        // Anführungszeichen, ein `'` darin wird `'\''`.
        let quoted =
            |path: &Path| format!("'{}'", path.display().to_string().replace('\'', r"'\''"));
        assert_eq!(
            refused.fix,
            Some(FixAction::CopyCommand(format!(
                "humanitl --config {} config set {WORK_DIR_KEY} {}",
                quoted(&paths.config_path()),
                quoted(&outside)
            )))
        );
    }
}
