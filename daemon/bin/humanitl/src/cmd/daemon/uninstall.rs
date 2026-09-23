//! `humanitl daemon uninstall` (HUM-077): das Gegenstück zu `daemon install`.
//!
//! Ein eigenes Modul, weil es eine eigene Zusage hat: **erst abmelden, dann
//! entfernen**, und entfernt wird nur, was `daemon install` angelegt hat. Die
//! Werkzeuge dafür teilt es mit `install` (`systemctl` mit Frist und ohne die
//! Umgebung des Nutzers, die Kopien unter `~/.local/lib/humanitl`); sie stehen
//! in [`super`].

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};
use serde_json::json;

use super::{
    CURRENT_LINK, find_in_path, lib_base, no_bus, no_user_session, own_copies, own_directory,
    owner_of, systemctl_run, unit_fix,
};
use crate::cli::UninstallArgs;
use crate::cmd::{Context, EXIT_OK, Failure, unit};
use crate::render::table;

/// Was aus dem Versuch wurde, den Dienst bei systemd abzumelden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deactivation {
    /// `disable --now` ist durchgelaufen.
    Disabled,
    /// Es gibt kein `systemctl` in `PATH`; die Verweise wurden von Hand
    /// entfernt.
    NoSystemctl,
    /// Es war keine Unit da, die abzumelden gewesen wäre.
    Nothing,
}

impl Deactivation {
    /// Das Wort für die Ausgabe.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NoSystemctl => "no systemctl",
            Self::Nothing => "nothing",
        }
    }
}

/// `daemon uninstall` (HUM-077): das Gegenstück zu [`super::install`].
///
/// Die Reihenfolge ist die Zusage: **erst abmelden, dann entfernen.** Scheitert
/// `systemctl --user disable --now`, ist noch nichts gelöscht, und der Befund
/// `DAEMON_014` nennt genau den Aufruf, der scheiterte. Danach gehen die
/// Verweise der Aktivierung, die Unit mit der Marke, ein Socket, an dem niemand
/// mehr lauscht, und mit `--purge-binaries` die Kopien aus einem `AppImage` --
/// die erst jetzt, weil kein Dienst mehr aus ihnen läuft (HUM-077,
/// Fallstricke).
///
/// Entfernt wird nur, was `daemon install` angelegt hat. Eine Unit ohne Marke
/// gehört jemand anderem (`DAEMON_005`); die Units des Pakets unter
/// `/usr/lib/systemd/user` werden abgemeldet und bleiben liegen, bis das Paket
/// geht.
pub(super) async fn uninstall(ctx: &Context, args: &UninstallArgs) -> Result<u8, Failure> {
    let path = unit::unit_path(&ctx.paths);
    let own = match std::fs::read_to_string(&path) {
        Ok(text) if unit::carries_marker(&text) => true,
        // Derselbe Code wie beim Schreiben: Die Datei hat Humanitl nicht
        // geschrieben, also nimmt Humanitl sie auch nicht weg, und es meldet
        // auch nichts ab, was sie startet.
        Ok(_) => return Err(Failure::new(foreign_unit(&path))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(Failure::new(incomplete(
                &path,
                &format!("cannot be read ({error}); nothing was removed"),
                None,
            )));
        }
    };
    let packaged = unit::SystemUnits::find(&unit::system_unit_dir(&ctx.env));
    let names: Vec<&str> = match (packaged.as_ref(), own) {
        (Some(units), _) => units.names(),
        (None, true) => vec![unit::UNIT_NAME],
        (None, false) => Vec::new(),
    };
    let dir = unit::unit_dir(&ctx.paths);
    let systemctl = find_in_path(ctx, "systemctl");

    let deactivation = match (names.is_empty(), systemctl.as_deref()) {
        (true, _) => Deactivation::Nothing,
        (false, None) => Deactivation::NoSystemctl,
        (false, Some(systemctl)) => {
            if ctx.env.non_empty("XDG_RUNTIME_DIR").is_none() {
                return Err(Failure::new(no_user_session(
                    "XDG_RUNTIME_DIR is not set, so systemctl --user has no user session to \
                     disable the service in; nothing was removed",
                )));
            }
            disable(ctx, systemctl, &names).await?;
            Deactivation::Disabled
        }
    };

    // Antwortet nach dem Abmelden noch ein Daemon, läuft er womöglich aus
    // genau den Kopien, die `--purge-binaries` entfernen soll: von Hand
    // gestartet, ohne `systemctl` oder aus einer anderen Unit. Dann wird
    // nichts entfernt (HUM-077, Review).
    let socket = ctx.paths.daemon_socket();
    if args.purge_binaries && daemon_answers(&socket) {
        return Err(Failure::new(still_running(&socket)));
    }

    let mut removed: Vec<PathBuf> = Vec::new();
    let mut left: Vec<Left> = Vec::new();
    for link in unit::enablement_links(&dir, &names) {
        remove_into(&link, std::fs::remove_file(&link), &mut removed, &mut left);
    }
    if own {
        remove_into(&path, std::fs::remove_file(&path), &mut removed, &mut left);
    }
    if deactivation == Deactivation::Disabled
        && let Some(systemctl) = systemctl.as_deref()
    {
        // Beide räumen systemds Bild auf und ändern an dem, was entfernt ist,
        // nichts mehr; ein Fehler dabei ist kein Befund.
        let _ = systemctl_run(ctx, systemctl, &["--user", "daemon-reload"]).await;
        let mut reset = vec!["--user", "reset-failed"];
        reset.extend_from_slice(&names);
        let _ = systemctl_run(ctx, systemctl, &reset).await;
    }
    // Ein Socket, an dem ein Daemon antwortet, bleibt liegen. Ob seither
    // einer gestartet ist, prüft `purge_binaries` noch einmal unter der
    // Sperre, bevor es eine Kopie anfasst (HUM-077, Review).
    if let SocketState::Stale(outcome) = socket_state(&socket) {
        remove_into(&socket, outcome, &mut removed, &mut left);
    }
    let binaries = if args.purge_binaries {
        purge_binaries(ctx, &mut left)
    } else {
        Vec::new()
    };

    if !left.is_empty() {
        return Err(Failure::new(incomplete(
            &path,
            &format!(
                "was taken down, but these stayed behind: {}",
                left.iter()
                    .map(Left::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            left.first().map(|entry| entry.path.as_path()),
        )));
    }
    report_uninstall(
        ctx,
        &UninstallReport {
            unit: &path,
            units: &names,
            deactivation,
            removed: &removed,
            binaries: &binaries,
            packaged: packaged.as_ref().map(|units| units.service.as_path()),
        },
    );
    Ok(EXIT_OK)
}

/// `systemctl --user disable --now` für die genannten Units.
///
/// # Errors
///
/// `DAEMON_010`, wenn `systemctl` den Bus der Sitzung nicht findet,
/// `DAEMON_014` mit genau diesem Aufruf zum Kopieren für jeden anderen
/// Fehlschlag. In beiden Fällen ist noch nichts entfernt.
async fn disable(ctx: &Context, systemctl: &Path, names: &[&str]) -> Result<(), Failure> {
    let mut call = vec!["--user", "disable", "--now"];
    call.extend_from_slice(names);
    let Err(why) = systemctl_run(ctx, systemctl, &call).await else {
        return Ok(());
    };
    if no_bus(&why) {
        return Err(Failure::new(no_user_session(&format!(
            "systemctl {} found no user session bus ({why}); nothing was removed",
            call.join(" ")
        ))));
    }
    let mut words = vec!["systemctl"];
    words.extend_from_slice(&call);
    Err(Failure::new(
        Diagnostic::builder(codes::DAEMON_014, Severity::Blocking)
            .why(format!(
                "systemctl {} did not go through ({why}); nothing was removed",
                call.join(" ")
            ))
            .fix(unit_fix(&words))
            .build(),
    ))
}

/// Was die Deinstallation stehen ließ, und warum. Der Pfad bleibt für sich,
/// damit der Vorschlag ihn nicht aus dem Text zurückgewinnen muss.
struct Left {
    path: PathBuf,
    why: String,
}

impl std::fmt::Display for Left {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.path.display(), self.why)
    }
}

/// Verbucht das Ergebnis eines Löschens: entfernt, schon weg oder
/// stehengeblieben.
fn remove_into(
    path: &Path,
    outcome: std::io::Result<()>,
    removed: &mut Vec<PathBuf>,
    left: &mut Vec<Left>,
) {
    match outcome {
        Ok(()) => removed.push(path.to_path_buf()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => left.push(Left {
            path: path.to_path_buf(),
            why: error.to_string(),
        }),
    }
}

/// Was an der Stelle des Daemon-Sockets liegt.
enum SocketState {
    /// Nichts, oder eine Datei, die kein Socket ist.
    Absent,
    /// Ein Socket, an dem niemand mehr lauscht; das Ergebnis seines Entfernens.
    Stale(std::io::Result<()>),
    /// Ein Socket, an dem ein Daemon antwortet. Den hat nicht systemd
    /// gestartet, und diesem Befehl nimmt er nichts weg.
    Live,
}

/// Prüft den Socket des Daemons und entfernt ihn, wenn er liegen geblieben ist.
fn socket_state(socket: &Path) -> SocketState {
    use std::os::unix::fs::FileTypeExt as _;

    let Ok(meta) = std::fs::symlink_metadata(socket) else {
        return SocketState::Absent;
    };
    if !meta.file_type().is_socket() {
        return SocketState::Absent;
    }
    if daemon_answers(socket) {
        return SocketState::Live;
    }
    SocketState::Stale(std::fs::remove_file(socket))
}

/// Ob an `socket` ein Prozess lauscht: ein Socket, der eine Verbindung annimmt.
fn daemon_answers(socket: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

/// `DAEMON_014`, weil nach dem Abmelden noch ein Daemon antwortet und
/// `--purge-binaries` ihm seine Dateien nähme.
fn still_running(socket: &Path) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_014, Severity::Blocking)
        .why(format!(
            "a daemon still answers on {} after the service was disabled; it may run from the \
             copies --purge-binaries would remove, so nothing was removed. Stop it and run the \
             command again",
            socket.display()
        ))
        .fix(unit_fix(&["pkill", "-x", unit::DAEMON_NAME]))
        .build()
}

/// Entfernt die Kopien aus einem `AppImage` unter `~/.local/lib/humanitl`.
///
/// Nur, was [`super::stage`] angelegt hat: der Verweis `current`, liegengebliebene
/// Zwischenverweise `current.tmp-*` und die eigenen Verzeichnisse
/// `<version>.<nanos>-<pid>`. Ist `~/.local/lib/humanitl` selbst ein Verweis
/// oder gehört einem anderen Konto, wird nichts angefasst. Das Verzeichnis
/// selbst geht zuletzt, wenn es danach leer ist.
fn purge_binaries(ctx: &Context, left: &mut Vec<Left>) -> Vec<PathBuf> {
    let base = lib_base(ctx);
    if std::fs::symlink_metadata(&base).is_err() {
        return Vec::new();
    }
    let checked = owner_of(&ctx.paths.home()).and_then(|uid| {
        own_directory(&base, uid)?;
        Ok(uid)
    });
    // Die Sperre gilt bis zum Ende dieser Funktion: Ein `daemon install`
    // daneben legt in dieser Zeit keine Kopie an, die hier gleich ginge.
    let (owner, _lock) = match checked.and_then(|uid| Ok((uid, super::lock_lib(&base)?))) {
        Ok(locked) => locked,
        Err(diagnostic) => {
            // Der Pfad für sich, damit der Vorschlag von [`incomplete`] ein
            // ausführbares `ls -ld <pfad>` bleibt.
            left.push(Left {
                path: base.clone(),
                why: diagnostic.why,
            });
            return Vec::new();
        }
    };
    // Unter der Sperre noch einmal: Ein `daemon install`, das eben fertig
    // wurde, kann einen Daemon aus der neuen Kopie gestartet haben.
    let socket = ctx.paths.daemon_socket();
    if daemon_answers(&socket) {
        left.push(Left {
            path: base,
            why: format!(
                "a daemon answers on {} again, so its copies stay",
                socket.display()
            ),
        });
        return Vec::new();
    }
    let mut removed = Vec::new();
    let mut links: Vec<PathBuf> = std::fs::read_dir(&base)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry.file_name().to_str().is_some_and(|name| {
                        name == CURRENT_LINK || name.starts_with("current.tmp-")
                    })
                })
                .map(|entry| entry.path())
                .filter(|path| {
                    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
                })
                .collect()
        })
        .unwrap_or_default();
    links.sort();
    for link in links {
        remove_into(&link, std::fs::remove_file(&link), &mut removed, left);
    }
    let mut copies = own_copies(&base, owner);
    copies.sort();
    for copy in copies {
        remove_into(&copy, std::fs::remove_dir_all(&copy), &mut removed, left);
    }
    if std::fs::remove_dir(&base).is_ok() {
        removed.push(base);
    }
    removed
}

/// `DAEMON_005` beim Entfernen: Die Unit trägt die Marke nicht.
fn foreign_unit(path: &Path) -> Diagnostic {
    let shown = path.to_string_lossy();
    Diagnostic::builder(codes::DAEMON_005, Severity::Blocking)
        .why(format!(
            "{shown} does not start with the line Humanitl writes, so Humanitl did not write it \
             and does not remove it; nothing was changed"
        ))
        .fix(unit_fix(&["mv", &shown, &format!("{shown}.bak")]))
        .build()
}

/// `DAEMON_014`: Die Deinstallation ist nicht ganz durchgekommen.
///
/// `first` ist der Pfad des ersten Eintrags, der stehen blieb; der Vorschlag
/// zeigt seine Rechte.
fn incomplete(path: &Path, why: &str, first: Option<&Path>) -> Diagnostic {
    let shown = first.unwrap_or(path).display().to_string();
    Diagnostic::builder(codes::DAEMON_014, Severity::Blocking)
        .why(format!("{} {why}", path.display()))
        .fix(unit_fix(&["ls", "-ld", &shown]))
        .build()
}

/// Was `daemon uninstall` am Ende ausgibt.
#[derive(Debug)]
struct UninstallReport<'a> {
    /// Der Pfad der Unit, die `daemon install` schreibt.
    unit: &'a Path,
    /// Die Units, die abgemeldet wurden.
    units: &'a [&'a str],
    /// Was aus der Abmeldung wurde.
    deactivation: Deactivation,
    /// Was entfernt wurde: Verweise, Unit, Socket.
    removed: &'a [PathBuf],
    /// Was von den Kopien aus einem `AppImage` entfernt wurde.
    binaries: &'a [PathBuf],
    /// Die Dienst-Unit des Pakets, wenn es eine gibt; sie bleibt liegen.
    packaged: Option<&'a Path>,
}

/// Das Ergebnis von `daemon uninstall`: JSON oder Tabelle.
fn report_uninstall(ctx: &Context, result: &UninstallReport<'_>) {
    let shown = |paths: &[PathBuf]| -> Vec<String> {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    };
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "unit": result.unit.display().to_string(),
            "units": result.units,
            "activation": result.deactivation.as_str(),
            "removed": shown(result.removed),
            "binaries": shown(result.binaries),
            "package": result.packaged.map(|path| path.display().to_string()),
        }));
        return;
    }
    let list = |paths: &[PathBuf]| {
        if paths.is_empty() {
            "-".to_owned()
        } else {
            shown(paths).join(", ")
        }
    };
    let rows = vec![
        vec!["unit".to_owned(), result.unit.display().to_string()],
        vec![
            "units".to_owned(),
            if result.units.is_empty() {
                "-".to_owned()
            } else {
                result.units.join(" ")
            },
        ],
        vec![
            "activation".to_owned(),
            result.deactivation.as_str().to_owned(),
        ],
        vec!["removed".to_owned(), list(result.removed)],
        vec!["binaries".to_owned(), list(result.binaries)],
    ];
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    if let Some(service) = result.packaged {
        ctx.render.note(&format!(
            "the units of the package stay in {} and are disabled; sudo apt remove humanitl \
             removes them",
            service.parent().unwrap_or(service).display()
        ));
    }
}
