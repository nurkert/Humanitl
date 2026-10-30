//! Der Daemon als launchd-Agent unter macOS (HUM-314, `backlog/sprint-6.md`).
//!
//! Unter Linux richtet `humanitl daemon install` eine systemd-Unit ein; unter
//! macOS übernimmt launchd diese Rolle. Eine Datei
//! `~/Library/LaunchAgents/dev.humanitl.humanitld.plist` sagt, welches
//! Programm läuft, dass es mit der Anmeldung startet (`RunAtLoad`) und nach
//! einem Absturz wieder (`KeepAlive` bei einem Ende ungleich 0). Seine Ausgabe
//! geht nach `~/Library/Logs/Humanitl/humanitld.log`.
//!
//! - `install` schreibt die Datei und lädt den Agenten neu (`launchctl
//!   bootout`, dann `bootstrap`). Ein zweiter Aufruf ist damit zugleich der
//!   Neustart, den die App nach einem Update braucht.
//! - `restart` startet ihn neu (`launchctl kickstart -k`).
//! - `uninstall` entlädt ihn aus jeder Domäne und löscht die Datei.
//! - `status` fragt den Daemon und `launchctl print` nach dem Zustand.
//! - `logs` liest die Protokolldatei, mit `--follow` wie `tail -f`.
//!
//! Kein Socket-Start: Der Daemon bindet seinen Socket selbst, nachdem das
//! Token steht, wie ohne systemd unter Linux.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::json;

use crate::cli::{DaemonCmd, InstallArgs, LogsArgs};
use crate::cmd::{Context, EXIT_OK, Failure};

/// Der Name des Agenten bei launchd.
pub const LABEL: &str = "dev.humanitl.humanitld";

/// Die Suchpfade des Daemons: Er sucht dort den Agenten (`opencode`, `node`).
/// launchd startet Agenten sonst mit `/usr/bin:/bin:/usr/sbin:/sbin`.
pub const DAEMON_PATH: &str = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

/// Wo die Datei des Agenten liegt.
#[must_use]
pub fn plist_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// Wo die Ausgabe des Daemons landet.
#[must_use]
pub fn log_path(home: &Path) -> PathBuf {
    home.join("Library/Logs")
        .join(humanitl_config::MAC_APP_DIR)
        .join("humanitld.log")
}

/// Die Zeile, an der `install` und `uninstall` ihre eigene Datei erkennen.
const MARKER: &str = "Written by `humanitl daemon install` (HUM-314)";

/// Die Datei des Agenten für dieses Programm.
#[must_use]
pub fn render(daemon: &Path, log: &Path) -> String {
    let daemon = xml(&daemon.to_string_lossy());
    let log = xml(&log.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- {MARKER}; `humanitl daemon uninstall` removes it. -->
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{daemon}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>{DAEMON_PATH}</string>
  </dict>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#
    )
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Die beiden Domänen des Nutzers bei launchd: `gui/<uid>` einer
/// angemeldeten Sitzung und `user/<uid>`, die es auch über ssh oder auf einem
/// Build-Rechner ohne Anmeldung gibt. launchd führt sie getrennt; ein Agent
/// kann in jeder von beiden geladen sein.
fn domains() -> [String; 2] {
    let uid = rustix::process::getuid().as_raw();
    [format!("gui/{uid}"), format!("user/{uid}")]
}

/// Ob `launchctl print <target>` antwortet.
fn prints(target: &str) -> bool {
    std::process::Command::new("/bin/launchctl")
        .args(["print", target])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Die Domäne, in die `install` lädt: die GUI-Domäne, wenn es sie gibt.
fn domain() -> String {
    let [gui, user] = domains();
    if prints(&gui) { gui } else { user }
}

/// Die Domänen, in denen der Agent gerade geladen ist.
fn loaded() -> Vec<String> {
    domains()
        .into_iter()
        .filter(|domain| prints(&format!("{domain}/{LABEL}")))
        .collect()
}

/// Entlädt den Agenten aus jeder Domäne, in der er geladen ist.
///
/// # Errors
///
/// `DAEMON_008`, wenn er danach noch irgendwo geladen ist.
fn unload_everywhere() -> Result<(), Failure> {
    for domain in loaded() {
        let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")])?;
    }
    match loaded().first() {
        None => Ok(()),
        Some(domain) => Err(Failure::new(not_loaded(&format!(
            "{LABEL} is still loaded in {domain} after bootout"
        )))),
    }
}

/// `daemon <cmd>` unter macOS.
///
/// # Errors
///
/// Ein [`Failure`] mit `DAEMON_007` (kein Daemon neben der Kommandozeile),
/// `DAEMON_008` (launchd nahm den Agenten nicht an) oder einem Fehler beim
/// Schreiben.
pub async fn run(ctx: &Context, cmd: &DaemonCmd) -> Result<u8, Failure> {
    match cmd {
        DaemonCmd::Status => super::status(ctx).await,
        DaemonCmd::Install(args) => install(ctx, args),
        DaemonCmd::Logs(args) => logs(ctx, args),
        DaemonCmd::Restart => restart(ctx),
        DaemonCmd::Uninstall(_) => uninstall(ctx),
        // Eine Frage an den Daemon, unabhängig vom Dienstverwalter.
        DaemonCmd::RetireIfIdle => super::retire::retire_if_idle(ctx).await,
    }
}

fn install(ctx: &Context, args: &InstallArgs) -> Result<u8, Failure> {
    let daemon = daemon_binary(args)?;
    let home = ctx.paths.home();
    let plist = plist_path(&home);
    let log = log_path(&home);
    let contents = render(&daemon, &log);
    if args.print {
        if ctx.render.is_json() {
            ctx.render.value(&json!({
                "plist": plist.display().to_string(),
                "contents": contents,
            }));
        } else {
            ctx.render.line(&contents);
        }
        return Ok(EXIT_OK);
    }
    // Eine fremde Datei unter demselben Namen wird nicht überschrieben, wie
    // eine fremde Unit unter Linux (`DAEMON_005`).
    let before = std::fs::read_to_string(&plist).ok();
    if let Some(text) = &before
        && !text.contains(MARKER)
    {
        return Err(Failure::new(
            Diagnostic::builder(codes::DAEMON_005, Severity::Blocking)
                .why(format!(
                    "{} exists and was not written by humanitl daemon install; it is left alone",
                    plist.display()
                ))
                .fix(FixAction::CopyCommand(format!("cat '{}'", plist.display())))
                .build(),
        ));
    }
    // Gleiche Datei, Agent geladen: nichts zu tun. Ein Neustart beendete
    // sonst bei jedem Klick auf „Dienst einrichten" die Arbeit des Agenten.
    if before.as_deref() == Some(contents.as_str()) && !loaded().is_empty() && !args.no_start {
        if ctx.render.is_json() {
            ctx.render.value(&json!({
                "plist": plist.display().to_string(),
                "daemon": daemon.display().to_string(),
                "log": log.display().to_string(),
                "started": false,
                "unchanged": true,
            }));
        } else {
            ctx.render.line(&format!(
                "{} is unchanged and loaded; nothing to do",
                plist.display()
            ));
        }
        return Ok(EXIT_OK);
    }
    for dir in [plist.parent(), log.parent()].into_iter().flatten() {
        std::fs::create_dir_all(dir).map_err(|error| write_failed(dir, &error))?;
    }
    std::fs::write(&plist, contents).map_err(|error| write_failed(&plist, &error))?;
    let started = if args.no_start {
        false
    } else {
        // Ein schon geladener Agent läuft mit der alten Datei weiter; erst
        // aus jeder Domäne entladen, dann laden.
        unload_everywhere()?;
        bootstrap(&plist)?;
        true
    };
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "plist": plist.display().to_string(),
            "daemon": daemon.display().to_string(),
            "log": log.display().to_string(),
            "started": started,
        }));
    } else {
        ctx.render.line(&format!("wrote {}", plist.display()));
        ctx.render.line(&if started {
            format!("started {LABEL}; log: {}", log.display())
        } else {
            format!(
                "not started (--no-start); start it with: launchctl bootstrap {} {}",
                domain(),
                plist.display()
            )
        });
    }
    Ok(EXIT_OK)
}

/// `humanitld` aus `--bin-dir` oder neben dieser Kommandozeile.
fn daemon_binary(args: &InstallArgs) -> Result<PathBuf, Failure> {
    let dir = match &args.bin_dir {
        Some(dir) => dir.clone(),
        None => std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from(".")),
    };
    let daemon = dir.join("humanitld");
    if rustix::fs::access(&daemon, rustix::fs::Access::EXEC_OK).is_err() {
        return Err(Failure::new(
            Diagnostic::builder(codes::DAEMON_007, Severity::Blocking)
                .why(format!(
                    "{} is missing or not executable; the launchd agent needs the daemon next to \
                     the command line",
                    daemon.display()
                ))
                .fix(FixAction::OpenUrl(
                    "https://github.com/nurkert/Humanitl/releases".to_owned(),
                ))
                .build(),
        ));
    }
    Ok(daemon.canonicalize().unwrap_or(daemon))
}

fn uninstall(ctx: &Context) -> Result<u8, Failure> {
    let plist = plist_path(&ctx.paths.home());
    if let Ok(text) = std::fs::read_to_string(&plist)
        && !text.contains(MARKER)
    {
        return Err(Failure::new(
            Diagnostic::builder(codes::DAEMON_005, Severity::Blocking)
                .why(format!(
                    "{} was not written by humanitl daemon install; it is left alone",
                    plist.display()
                ))
                .build(),
        ));
    }
    unload_everywhere()?;
    let removed = match std::fs::remove_file(&plist) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(Failure::new(write_failed(&plist, &error))),
    };
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "plist": plist.display().to_string(),
            "removed": removed,
        }));
    } else if removed {
        ctx.render
            .line(&format!("stopped {LABEL} and removed {}", plist.display()));
    } else {
        ctx.render
            .line(&format!("{} was not installed", plist.display()));
    }
    Ok(EXIT_OK)
}

/// `daemon restart`: `launchctl kickstart -k` in der Domäne, in der der
/// Agent geladen ist.
fn restart(ctx: &Context) -> Result<u8, Failure> {
    let Some(domain) = loaded().into_iter().next() else {
        return Err(Failure::new(
            Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
                .why(format!(
                    "{LABEL} is not loaded in launchd, so there is nothing to restart"
                ))
                .fix(FixAction::InstallService)
                .build(),
        ));
    };
    let target = format!("{domain}/{LABEL}");
    let output = launchctl(&["kickstart", "-k", &target])?;
    if !output.status.success() {
        return Err(Failure::new(not_loaded(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    if ctx.render.is_json() {
        ctx.render.value(&json!({ "restarted": target }));
    } else {
        ctx.render.line(&format!("restarted {target}"));
    }
    Ok(EXIT_OK)
}

fn logs(ctx: &Context, args: &LogsArgs) -> Result<u8, Failure> {
    let log = log_path(&ctx.paths.home());
    if !log.is_file() {
        return Err(Failure::new(
            Diagnostic::builder(codes::DAEMON_008, Severity::Error)
                .why(format!(
                    "{} does not exist; the launchd agent writes it once it has started",
                    log.display()
                ))
                .fix(FixAction::InstallService)
                .build(),
        ));
    }
    let mut command = std::process::Command::new("/usr/bin/tail");
    command.arg("-n").arg(args.lines.unwrap_or(100).to_string());
    if args.follow {
        command.arg("-f");
    }
    command.arg(&log);
    let status = command
        .status()
        .map_err(|error| Failure::new(write_failed(&log, &error)))?;
    Ok(if status.success() {
        EXIT_OK
    } else {
        crate::cmd::EXIT_USER
    })
}

/// Der Zustand des Agenten nach `launchctl print`: `running`, `waiting` oder
/// `None`, wenn launchd ihn nicht kennt.
pub async fn is_active() -> Option<String> {
    let target = loaded().into_iter().next().map_or_else(
        || format!("{}/{LABEL}", domain()),
        |domain| format!("{domain}/{LABEL}"),
    );
    let output = tokio::process::Command::new("/bin/launchctl")
        .args(["print", &target])
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return Some("not loaded".to_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("state = ").map(str::to_owned))
}

/// `launchctl bootstrap`, wiederholt: Direkt nach einem `bootout` baut
/// launchd den alten Auftrag manchmal noch ab und antwortet `5: Input/output
/// error`.
fn bootstrap(plist: &Path) -> Result<(), Failure> {
    let target = plist.to_string_lossy();
    let mut last = String::new();
    for _ in 0..10 {
        let output = launchctl(&["bootstrap", &domain(), &target])?;
        if output.status.success() {
            return Ok(());
        }
        last = String::from_utf8_lossy(&output.stderr).into_owned();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    Err(Failure::new(not_loaded(&last)))
}

fn launchctl(args: &[&str]) -> Result<std::process::Output, Failure> {
    std::process::Command::new("/bin/launchctl")
        .args(args)
        .output()
        .map_err(|error| Failure::new(not_loaded(&format!("launchctl did not start: {error}"))))
}

fn not_loaded(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
        .why(format!("launchd did not load {LABEL}: {}", why.trim()))
        .fix(FixAction::CopyCommand(format!(
            "launchctl print {}/{LABEL}",
            domain()
        )))
        .build()
}

fn write_failed(path: &Path, error: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
        .why(format!("cannot write {}: {error}", path.display()))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_agent_starts_at_login_and_after_a_crash_but_not_after_a_clean_end() {
        let text = render(
            Path::new("/Applications/Humanitl.app/Contents/MacOS/bin/humanitld"),
            Path::new("/Users/me/Library/Logs/Humanitl/humanitld.log"),
        );
        assert!(text.contains("<string>dev.humanitl.humanitld</string>"));
        assert!(
            text.contains(
                "<string>/Applications/Humanitl.app/Contents/MacOS/bin/humanitld</string>"
            )
        );
        assert!(text.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(text.contains("<key>SuccessfulExit</key>\n    <false/>"));
        assert!(text.contains(DAEMON_PATH));
        assert!(
            text.contains(MARKER),
            "uninstall finds its own file by this line"
        );
    }

    #[test]
    fn a_path_with_markup_stays_one_string() {
        let text = render(Path::new("/a&b/<c>/humanitld"), Path::new("/l\"og"));
        assert!(text.contains("<string>/a&amp;b/&lt;c&gt;/humanitld</string>"));
        assert!(text.contains("/l&quot;og"));
    }

    #[test]
    fn the_files_lie_under_the_library_of_the_home() {
        let home = Path::new("/Users/me");
        assert_eq!(
            plist_path(home),
            Path::new("/Users/me/Library/LaunchAgents/dev.humanitl.humanitld.plist")
        );
        assert_eq!(
            log_path(home),
            Path::new("/Users/me/Library/Logs/Humanitl/humanitld.log")
        );
    }
}
