//! `humanitl app [PATH]` (HUM-539, ADR-0024 Punkt 5): das Startfenster der
//! Desktop-App, mit diesem Ordner vorgewählt.
//!
//! Zwei Wege, und das CLI entscheidet nur, welcher geht:
//!
//! - Ist eine App mit dem Daemon verbunden, geht die Bitte über den Daemon
//!   (`Projects.open_in_app`, Frage 4 des ADR); er gibt sie der App als
//!   Ereignis auf dem Strom, den sie ohnehin hält, und die App holt ihr
//!   Fenster nach vorne. Kein zweiter Prozess, kein eigener Socket.
//! - Sonst startet das CLI die App: `humanitl-app --project PATH` aus dem
//!   Ordner des eigenen Programms oder dem `PATH`, auf macOS
//!   `open -a Humanitl --args --project PATH`. Ist sie nirgends, `CLI_007`
//!   mit dem Weg zur Installation.
//!
//! Der Ordner ist für die App nur eine Vorwahl: Ob er ein Ordner ist, sagt
//! ihr der Daemon, und gestartet wird erst nach „Start" im Fenster. Ohne
//! `PATH` zeigt die App ihr Startfenster ohne Vorwahl.
//!
//! Zwei Grenzen der Plattformen, bewusst ohne eigenen Mechanismus:
//!
//! - macOS: Läuft `Humanitl.app` schon, ohne mit dem Daemon verbunden zu
//!   sein, holt `open -a` sie nur nach vorne; die Argumente hinter `--args`
//!   erreichen eine laufende App nicht, der Ordner ist dann nicht
//!   vorgewählt. `open -n` startete stattdessen eine zweite Instanz.
//! - Linux: Läuft die App ohne Verbindung zum Daemon (er ist beendet oder
//!   startet neu), sieht das CLI keine App und startet eine zweite.
//!
//! Ein Ordner mit Steuerzeichen im Namen ist `CLI_004`: Der Daemon gibt einen
//! solchen Pfad nicht weiter (er stünde in einer Benachrichtigung), und eine
//! zweite App zu starten, weil die erste ihn nicht bekommen kann, wäre
//! falsch.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cli::AppArgs;
use crate::cmd::{Context, EXIT_OK, Failure};

/// Die erste Minor des Vertrags, die `Projects.open_in_app` kennt.
pub const OPEN_IN_APP_MINOR: u32 = 60;

/// Das Programm der Desktop-App auf Linux (`packaging/deb`).
#[cfg(not(target_os = "macos"))]
pub const APP_PROGRAM: &str = "humanitl-app";

/// Die Anleitung zur Installation, auf die `CLI_007` verweist.
pub const INSTALL_URL: &str = "https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md";

/// Wie die Bitte die App erreicht hat.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reached {
    /// Über den Daemon an die verbundene App.
    RunningApp,
    /// Als neuer Prozess dieses Programms.
    Started(PathBuf),
}

/// Führt `humanitl app [PATH]` aus.
///
/// # Errors
///
/// `CLI_007`, wenn keine App verbunden ist und keine gestartet werden kann.
pub async fn run(ctx: &Context, args: &AppArgs) -> Result<u8, Failure> {
    let folder = args.path.as_deref().map(|path| canonical(&ctx.cwd, path));
    if let Some(folder) = folder.as_deref()
        && folder
            .as_os_str()
            .to_string_lossy()
            .chars()
            .any(char::is_control)
    {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "{} holds a control character in its name; humanitl app cannot preselect it",
                    crate::render::plain(&folder.display().to_string())
                ))
                .build(),
        ));
    }
    let reached = if forwarded(ctx, folder.as_deref()).await {
        Reached::RunningApp
    } else {
        Reached::Started(launch(ctx, folder.as_deref())?)
    };
    report(ctx, folder.as_deref(), &reached);
    Ok(EXIT_OK)
}

/// Der Ordner, wie die App ihn bekommt: gegen das Arbeitsverzeichnis
/// aufgelöst und kanonisiert (`.` ist das Arbeitsverzeichnis, auf macOS wird
/// `/var` zu `/private/var`). Einen Pfad, den es nicht gibt, gibt das CLI
/// unverändert absolut weiter: Dass er kein Ordner ist, sagt der Daemon der
/// App, und sie zeigt es als Befund.
fn canonical(cwd: &Path, path: &Path) -> PathBuf {
    let joined = cwd.join(path);
    joined.canonicalize().unwrap_or(joined)
}

/// Ob eine verbundene App die Bitte bekommen hat.
///
/// Nein heißt: Das CLI startet die App selbst. So auch, wenn kein Daemon
/// läuft (geweckt wird keiner, eine App hielte ihn sonst schon), wenn er
/// einen anderen Vertrag spricht oder vor Minor 60 die Operation nicht kennt,
/// und für einen Pfad, der kein UTF-8 ist und deshalb nicht über den Vertrag
/// reisen kann.
async fn forwarded(ctx: &Context, folder: Option<&Path>) -> bool {
    let path = match folder.map(Path::to_str) {
        None => String::new(),
        Some(Some(text)) => text.to_owned(),
        Some(None) => return false,
    };
    let Ok(mut client) = ctx.connect_running().await else {
        return false;
    };
    let Ok(info) = client.get_info(()).await.map(tonic::Response::into_inner) else {
        return false;
    };
    if info.proto_major != humanitl_ipc::PROTO_MAJOR || info.proto_minor < OPEN_IN_APP_MINOR {
        return false;
    }
    client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::OpenInApp(
                v1::projects_request::OpenInApp { path },
            )),
        })
        .await
        .is_ok_and(|answer| answer.into_inner().app_reached)
}

/// Die Argumente der App: `--project PATH`, ohne Ordner keine.
fn app_args(folder: Option<&Path>) -> Vec<OsString> {
    folder.map_or_else(Vec::new, |folder| {
        vec![OsString::from("--project"), folder.as_os_str().to_owned()]
    })
}

/// Startet `humanitl-app` aus dem Ordner des eigenen Programms oder dem
/// `PATH`, abgekoppelt: eigene Prozessgruppe, ohne Standardein- und
/// -ausgabe, damit `Ctrl+C` im Terminal sie nicht mitnimmt und das CLI nicht
/// auf sie wartet.
#[cfg(not(target_os = "macos"))]
fn launch(ctx: &Context, folder: Option<&Path>) -> Result<PathBuf, Failure> {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};

    let own_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let Some(program) = find_app(own_dir.as_deref(), ctx.env.get("PATH")) else {
        return Err(not_installed(format!(
            "no desktop app is connected to the daemon, and {APP_PROGRAM} is neither next to \
             this program nor on PATH"
        )));
    };
    Command::new(&program)
        .args(app_args(folder))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|err| {
            not_installed(format!(
                "no desktop app is connected to the daemon, and {} did not start: {err}",
                program.display()
            ))
        })?;
    Ok(program)
}

/// Startet `Humanitl.app` über `open`, das wartet, bis macOS die App
/// gestartet oder abgelehnt hat.
#[cfg(target_os = "macos")]
fn launch(_ctx: &Context, folder: Option<&Path>) -> Result<PathBuf, Failure> {
    use std::process::{Command, Stdio};

    const OPEN: &str = "/usr/bin/open";
    let mut command = Command::new(OPEN);
    command.args(["-a", "Humanitl"]);
    let args = app_args(folder);
    if !args.is_empty() {
        command.arg("--args").args(args);
    }
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|err| not_installed(format!("{OPEN} -a Humanitl did not run: {err}")))?;
    if !output.status.success() {
        return Err(not_installed(format!(
            "no desktop app is connected to the daemon, and {OPEN} -a Humanitl failed: {}",
            crate::render::one_line(&String::from_utf8_lossy(&output.stderr))
        )));
    }
    Ok(PathBuf::from("Humanitl.app"))
}

/// Das erste ausführbare `humanitl-app`: neben dem eigenen Programm, dann
/// in jedem Eintrag von `PATH`.
#[cfg(not(target_os = "macos"))]
fn find_app(own_dir: Option<&Path>, path_var: Option<&str>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;

    let executable = |candidate: &Path| {
        std::fs::metadata(candidate)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    };
    own_dir
        .into_iter()
        .map(Path::to_path_buf)
        .chain(
            path_var
                .into_iter()
                .flat_map(std::env::split_paths)
                .filter(|dir| dir.is_absolute()),
        )
        .map(|dir| dir.join(APP_PROGRAM))
        .find(|candidate| executable(candidate))
}

/// `CLI_007` mit diesem Grund und dem Weg zur Installation.
fn not_installed(why: String) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_007, Severity::Error)
            .why(why)
            .fix(FixAction::OpenUrl(INSTALL_URL.to_owned()))
            .build(),
    )
}

/// Sagt, welcher Weg gegangen wurde.
fn report(ctx: &Context, folder: Option<&Path>, reached: &Reached) {
    let shown = folder.map(|folder| folder.display().to_string());
    if ctx.render.is_json() {
        let (via, program) = match reached {
            Reached::RunningApp => ("running_app", None),
            Reached::Started(program) => ("started", Some(program.display().to_string())),
        };
        ctx.render.value(&json!({
            "path": shown,
            "via": via,
            "program": program,
        }));
        return;
    }
    let what = shown.map_or_else(
        || "its start window".to_owned(),
        |path| {
            format!(
                "its start window with {} preselected",
                crate::render::plain(&path)
            )
        },
    );
    match reached {
        Reached::RunningApp => ctx
            .render
            .line(&format!("Asked the running app to show {what}.")),
        Reached::Started(_) => ctx.render.line(&format!("Started the app to show {what}.")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn dot_is_the_working_directory_canonicalized() {
        let dir = tempfile::TempDir::new().unwrap();
        let shown = canonical(dir.path(), Path::new("."));
        assert_eq!(shown, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn a_missing_path_goes_on_absolute() {
        let dir = tempfile::TempDir::new().unwrap();
        let shown = canonical(dir.path(), Path::new("gone"));
        assert_eq!(shown, dir.path().join("gone"));
    }

    #[test]
    fn without_a_folder_the_app_gets_no_arguments() {
        assert!(app_args(None).is_empty());
        assert_eq!(
            app_args(Some(Path::new("/p"))),
            vec![OsString::from("--project"), OsString::from("/p")]
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_own_folder_comes_before_path_and_relative_entries_are_skipped() {
        use std::os::unix::fs::PermissionsExt as _;

        let own = tempfile::TempDir::new().unwrap();
        let on_path = tempfile::TempDir::new().unwrap();
        for dir in [own.path(), on_path.path()] {
            let program = dir.join(APP_PROGRAM);
            std::fs::write(&program, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path_var = format!("relative:{}", on_path.path().display());
        assert_eq!(
            find_app(Some(own.path()), Some(&path_var)),
            Some(own.path().join(APP_PROGRAM))
        );
        assert_eq!(
            find_app(None, Some(&path_var)),
            Some(on_path.path().join(APP_PROGRAM))
        );
        assert_eq!(find_app(None, Some("relative")), None);
    }
}
