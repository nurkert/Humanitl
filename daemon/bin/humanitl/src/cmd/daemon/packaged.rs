//! `daemon install` mit den Units des Pakets (HUM-053), und was mit einer
//! Unit unter `~/.config/systemd/user` geschieht, die sie verdeckt (HUM-211,
//! HUM-167).
//!
//! Wer Humanitl zuerst aus dem Archiv oder dem `AppImage` eingerichtet hat,
//! hat dort `humanitld.service` mit der Marke liegen. systemd sucht Nutzer-Units
//! zuerst unter `~/.config/systemd/user`, dann unter `/etc/systemd/user` und
//! erst danach im Verzeichnis des Pakets (`systemd.unit(5)`, „User Unit Search
//! Path"). Ohne diesen Schritt liefe nach dem Paket weiter der alte Daemon mit
//! der alten Härtung, während die Ausgabe die Unit des Pakets nannte.
//!
//! - **Eine eigene Kopie** (erste Zeile ist die Marke) wird angekündigt und
//!   beiseitegelegt, aber nur, wenn gleich auch aktiviert wird: umbenannt nach
//!   `humanitld.service.bak` (oder `.bak.N`, wenn es den Namen schon gibt),
//!   und die Verweise der Aktivierung, die auf sie zeigen, gehen mit. Danach
//!   wird der Dienst neu gestartet, damit die Unit des Pakets läuft und nicht
//!   der Prozess aus der alten. Scheitert etwas, kommen Datei und Verweise
//!   zurück, wie beim Schreiben ([`super::activate`]). Unter `--no-start` oder
//!   ohne `systemctl` bleibt sie liegen: Sonst gäbe es nach dem nächsten
//!   Anmelden gar keinen Dienst mehr, weder den alten noch den des Pakets.
//! - **Eine fremde Kopie** wird nie angefasst: `DAEMON_005`, bevor irgendetwas
//!   geschieht, auch unter `--print`.
//! - **Der Bericht sagt, was systemd lädt**, nicht, was in der Datei des
//!   Pakets steht: `systemctl --user show -p FragmentPath,ExecStart`, sobald
//!   systemd die Units kennt.

use std::cell::RefCell;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};

use super::{
    Activation, InstallReport, PRINT_ACTION, UnitOnDisk, activate, announce, find_in_path,
    planned_commands, report, require_user_session, start, systemctl_capture, systemctl_run,
    unit_fix, wait_for_daemon,
};
use crate::cli::InstallArgs;
use crate::cmd::{Context, EXIT_OK, Failure, unit};

/// Das Wort, unter dem ein Lauf steht, der die Units des Pakets nur
/// aktiviert und nichts geschrieben hat (HUM-053).
const PACKAGED_ACTION: &str = "packaged";

/// Die Endung, unter der eine eigene ältere Unit beiseiteliegt.
///
/// systemd liest in seinen Verzeichnissen nur Dateien, deren Name auf einen
/// Unit-Typ endet; `humanitld.service.bak` verdeckt also nichts mehr.
const ASIDE_SUFFIX: &str = ".bak";

/// Wie viele nummerierte Namen neben `.bak` versucht werden, bevor der Lauf
/// aufgibt, statt eine vorhandene Sicherung zu überschreiben.
const ASIDE_ATTEMPTS: u32 = 100;

/// Die Units des Pakets, wenn dieser Lauf sie aktivieren soll (HUM-053).
///
/// Nicht aus einem `AppImage` und nicht mit `--bin-dir`: Beide nennen
/// ausdrücklich einen anderen Daemon als den des Pakets.
pub(super) fn packaged_units(
    ctx: &Context,
    args: &InstallArgs,
    appimage: bool,
) -> Option<unit::SystemUnits> {
    if appimage || args.bin_dir.is_some() {
        return None;
    }
    unit::SystemUnits::find(&unit::system_unit_dir(&ctx.env))
}

/// `daemon install`, wenn das Paket die Units unter
/// [`unit::SYSTEM_UNIT_DIR`] abgelegt hat (HUM-053).
///
/// Es wird nichts geschrieben: Die Units gehören dem Paket. Übrig bleibt, was
/// das Paket nicht kann, weil es als root läuft: eine eigene ältere Unit
/// beiseitelegen ([`Shadow`]), `systemctl --user daemon-reload` und
/// `enable --now` für Socket und Dienst. Dieselben Zusagen wie beim Schreiben
/// gelten: Die Ankündigung kommt vorher, `--print` ändert nichts, und ein
/// Fehlschlag nimmt zurück, was dieser Lauf angelegt oder beiseitegelegt hat.
pub(super) async fn install_packaged(
    ctx: &Context,
    args: &InstallArgs,
    units: &unit::SystemUnits,
) -> Result<u8, Failure> {
    let systemctl = find_in_path(ctx, "systemctl");
    let names = units.names();
    let commands = planned_commands(args.no_start, systemctl.as_deref(), &names);
    require_user_session(ctx, args)?;
    let dir = unit::unit_dir(&ctx.paths);
    let shadow = Shadow::find(&dir, &units.service).map_err(Failure::new)?;
    // Beiseitegelegt wird nur, wenn systemd gleich die Units des Pakets
    // bekommt. Sonst läge die alte Unit in `.bak`, die neue wäre nicht
    // aktiviert, und nach dem nächsten Anmelden liefe gar kein Daemon.
    let activator = systemctl.as_deref().filter(|_| !args.no_start);

    let exec = units.exec_start();
    let mut result = InstallReport {
        unit: &units.service,
        exec_start: &exec,
        unit_text: Some(&units.text),
        commands: &commands,
        enable: &names,
        action: PRINT_ACTION,
        activation: Activation::Skipped,
        binaries: None,
        restarted: false,
        ready: None,
        set_aside: None,
        shadowed_by: None,
    };
    if !ctx.render.is_json() {
        let plan = Plan {
            print: args.print,
            moves: activator.is_some(),
        };
        announce(
            &headline(units, shadow.as_ref(), plan),
            &units.text,
            args,
            systemctl.as_deref(),
            &names,
        );
    }
    let Some(systemctl) = activator.filter(|_| !args.print) else {
        // `--print`, `--no-start` oder kein `systemctl`: Nichts wird bewegt,
        // und eine verdeckende Unit steht als solche im Bericht.
        result.shadowed_by = shadow.as_ref().map(|shadow| shadow.path.as_path());
        if !args.print {
            result.action = PACKAGED_ACTION;
            let nothing = UnitOnDisk {
                path: &units.service,
                plan: &unit::Written::Unchanged,
            };
            result.activation = start(ctx, args, systemctl.as_deref(), &names, &nothing).await?;
            if let Some(shadow) = shadow.as_ref() {
                ctx.render.note(&shadow.left_in_place());
            }
        }
        report(ctx, &result);
        return Ok(EXIT_OK);
    };

    let activated = activate_packaged(ctx, systemctl, units, &names, shadow.as_ref()).await?;
    if let Some((fragment, exec_start)) = activated.loaded.as_ref() {
        result.unit = fragment.as_path();
        result.exec_start = exec_start.as_path();
        result.unit_text = activated.loaded_text.as_deref();
    }
    let ready = wait_for_daemon(ctx).await;
    result.action = PACKAGED_ACTION;
    result.activation = Activation::Enabled;
    result.restarted = activated.restarted;
    result.set_aside = shadow.as_ref().map(|shadow| shadow.aside.as_path());
    result.ready = Some(&ready);
    report(ctx, &result);
    Ok(EXIT_OK)
}

/// Was aus einer Aktivierung der Units des Pakets wurde.
struct Activated {
    /// Ob der Dienst neu gestartet wurde, weil eine alte Unit beiseiteging.
    restarted: bool,
    /// Datei und Programm, die systemd danach geladen hat ([`loaded_unit`]).
    loaded: Option<(PathBuf, PathBuf)>,
    /// Der Text dieser Datei; `None`, wenn sie sich nicht lesen lässt. Der
    /// Bericht nennt dann keinen Text statt den des Pakets neben einer
    /// anderen Datei.
    loaded_text: Option<String>,
}

/// Legt eine verdeckende eigene Unit beiseite, aktiviert die Units des
/// Pakets, startet den Dienst neu, wenn eine alte Unit beiseiteging, und liest
/// danach, was systemd geladen hat.
async fn activate_packaged(
    ctx: &Context,
    systemctl: &Path,
    units: &unit::SystemUnits,
    names: &[&str],
    shadow: Option<&Shadow>,
) -> Result<Activated, Failure> {
    let dir = unit::unit_dir(&ctx.paths);
    // Vorher gefragt: Nach einem gescheiterten `enable` wird der alte Dienst
    // nur dann wieder gestartet, wenn er vorher lief.
    let was_active = match shadow {
        Some(_) => service_is_active(ctx, systemctl).await,
        None => false,
    };
    if let Some(shadow) = shadow {
        shadow.set_aside().map_err(Failure::new)?;
    }
    // Nach dem Beiseitelegen gelesen: Die Verweise auf die alte Unit sind dann
    // schon weg und kommen über [`Shadow::restore`] zurück, nicht über diese
    // Rücknahme.
    let enabled_before = unit::Enablement::read_for(&dir, names);
    let socket_enabled_before = !unit::enablement_links(&dir, &[unit::SOCKET_NAME]).is_empty();
    let nothing = UnitOnDisk {
        path: &units.service,
        plan: &unit::Written::Unchanged,
    };
    // Scheitert `enable`, wird angehalten, was dieser Lauf gestartet haben
    // kann: nicht der Dienst, der schon vorher lief, wohl aber ein Socket, den
    // erst dieser Lauf aktiviert hat.
    let socket_is_new = names.contains(&unit::SOCKET_NAME) && !socket_enabled_before;
    let stop: Vec<&str> = if was_active {
        if socket_is_new {
            vec![unit::SOCKET_NAME]
        } else {
            Vec::new()
        }
    } else {
        names.to_vec()
    };
    if let Err(failure) = activate(ctx, systemctl, names, &nothing, &stop).await {
        return Err(match shadow {
            Some(shadow) => {
                put_back_after_activation(ctx, systemctl, shadow, was_active, failure).await
            }
            None => failure,
        });
    }
    if let Some(shadow) = shadow {
        let rollback = Rollback {
            shadow,
            enabled_before: &enabled_before,
            dir: &dir,
            package: &units.service,
            stop_socket: socket_is_new,
            was_active,
        };
        restart_on_package(ctx, systemctl, &rollback).await?;
    }
    let loaded = loaded_unit(ctx, systemctl).await;
    if let Some((fragment, _)) = loaded.as_ref()
        && *fragment != units.service
    {
        ctx.render.note(&format!(
            "systemd loads {} and not the package unit {}; systemctl --user cat {} shows which \
             file wins",
            fragment.display(),
            units.service.display(),
            unit::UNIT_NAME
        ));
    }
    let loaded_text = loaded
        .as_ref()
        .and_then(|(fragment, _)| std::fs::read_to_string(fragment).ok());
    Ok(Activated {
        restarted: shadow.is_some(),
        loaded,
        loaded_text,
    })
}

/// Was die Ankündigung über den Lauf wissen muss.
#[derive(Debug, Clone, Copy)]
struct Plan {
    /// `--print`: Es geschieht nichts, die Zeile sagt, was geschähe.
    print: bool,
    /// Ob eine verdeckende eigene Unit beiseitegelegt würde; nur, wenn auch
    /// aktiviert wird.
    moves: bool,
}

/// Die erste Zeile der Ankündigung.
///
/// „writes nothing" steht nur da, wenn es stimmt: Liegt eine eigene alte
/// Unit darüber, sagt die Zeile, dass sie bewegt wird, oder, warum nicht.
fn headline(units: &unit::SystemUnits, shadow: Option<&Shadow>, plan: Plan) -> String {
    let shown = match units.socket.as_ref() {
        Some(socket) => format!("{} and {}", units.service.display(), socket.display()),
        None => units.service.display().to_string(),
    };
    let name = unit::UNIT_NAME;
    match shadow {
        None => format!(
            "humanitl daemon install writes nothing: the package installed {shown}, and {name} \
             is:"
        ),
        Some(shadow) if plan.moves => format!(
            "humanitl daemon install {} its own older {}, which hides the package unit, to {} \
             and enables the package units {shown}; {name} of the package is:",
            if plan.print { "would move" } else { "moves" },
            shadow.path.display(),
            shadow.aside.display()
        ),
        Some(shadow) => format!(
            "humanitl daemon install leaves its own older {} in place: it hides the package \
             units {shown}, and it is only moved aside when systemd is told about them; {name} \
             of the package is:",
            shadow.path.display()
        ),
    }
}

/// `systemctl --user is-active --quiet humanitld.service`: ob der Dienst
/// gerade läuft. Jede andere Antwort, auch ein Fehler, heißt nein.
async fn service_is_active(ctx: &Context, systemctl: &Path) -> bool {
    systemctl_run(
        ctx,
        systemctl,
        &["--user", "is-active", "--quiet", unit::UNIT_NAME],
    )
    .await
    .is_ok()
}

/// Was nach einem gescheiterten `daemon-reload` oder `enable --now` noch
/// geschieht, wenn dieser Lauf eine eigene Unit beiseitegelegt hat.
///
/// [`super::activate`] hat seine Verweise schon zurückgenommen und den Dienst
/// angehalten, wenn es ihn gestartet haben kann. Hier kommt die alte Unit
/// zurück, systemd liest sie neu, und `start` bringt den alten Dienst wieder,
/// aber nur, wenn er vor diesem Lauf lief (`was_active`): Einen Dienst, der
/// aus war, startet eine Rücknahme nicht. Der Befund bekommt einen Satz dazu,
/// der sagt, wie das ausging.
async fn put_back_after_activation(
    ctx: &Context,
    systemctl: &Path,
    shadow: &Shadow,
    was_active: bool,
    mut failure: Failure,
) -> Failure {
    let shown = shadow.path.display();
    let restored = shadow.restore();
    let back = match restored.file {
        Err(why) => format!(
            "; putting {shown} back failed as well ({why}), so the service was not started again"
        ),
        Ok(()) => match systemctl_run(ctx, systemctl, &["--user", "daemon-reload"]).await {
            Err(error) => format!(
                "; {shown} is back in place, but systemctl --user daemon-reload failed ({error})"
            ),
            Ok(()) if !was_active => {
                format!("; {shown} is back in place, and the service stays stopped as before")
            }
            Ok(()) => {
                match systemctl_run(ctx, systemctl, &["--user", "start", unit::UNIT_NAME]).await {
                    Ok(()) => format!("; {shown} is back in place and the service runs on it"),
                    Err(error) => format!(
                        "; {shown} is back in place, but starting the service on it failed \
                         ({error})"
                    ),
                }
            }
        },
    };
    failure.diagnostic.why.push_str(&back);
    if let Err(links) = restored.links {
        failure.diagnostic.why.push_str("; ");
        failure.diagnostic.why.push_str(&links);
    }
    failure
}

/// Was ein gescheiterter Neustart zurücknimmt.
struct Rollback<'a> {
    /// Die beiseitegelegte Unit.
    shadow: &'a Shadow,
    /// Die Aktivierung nach dem Beiseitelegen und vor `enable --now`.
    enabled_before: &'a unit::Enablement,
    /// Das Unit-Verzeichnis des Nutzers.
    dir: &'a Path,
    /// Die Dienst-Unit des Pakets: Nur Verweise auf sie und ihren Socket
    /// daneben nimmt die Rücknahme weg.
    package: &'a Path,
    /// Ob dieser Lauf den Socket des Pakets erst aktiviert hat. Dann wird er
    /// angehalten: Der alte Daemon bindet den Pfad selbst.
    stop_socket: bool,
    /// Ob der Dienst vor diesem Lauf lief. Nur dann bringt die Rücknahme ihn
    /// mit einem zweiten Neustart wieder; sonst bleibt er aus wie vorher.
    was_active: bool,
}

/// Startet den Dienst neu, damit die Unit des Pakets läuft und nicht der
/// Prozess, den die beiseitegelegte Unit gestartet hat.
///
/// `enable --now` lässt einen laufenden Dienst in Ruhe; ohne diesen Schritt
/// liefe der alte Daemon mit der alten Härtung bis zur nächsten Abmeldung
/// weiter. Scheitert der Neustart, geht alles auf den Stand von vorher: die
/// Verweise, die `enable` angelegt hat, gehen, die alte Unit und ihre Verweise
/// kommen zurück, systemd liest neu, und ein zweiter Neustart bringt den alten
/// Dienst wieder. Der zweite nur, wenn die Rücknahme gelang, sonst startete er
/// die Unit des Pakets ein zweites Mal, und nur, wenn der Dienst vorher lief.
/// In jedem anderen Fall wird der gescheiterte Dienst angehalten und sein
/// Zustand `failed` gelöscht, damit `Restart=on-failure` nicht weiter auf ihn
/// losgeht; dieselbe Regel wie in [`super::activate`].
async fn restart_on_package(
    ctx: &Context,
    systemctl: &Path,
    rollback: &Rollback<'_>,
) -> Result<(), Failure> {
    let call = ["--user", "restart", unit::UNIT_NAME];
    let Err(why) = systemctl_run(ctx, systemctl, &call).await else {
        return Ok(());
    };
    if rollback.stop_socket {
        // Räumt nur auf; ein Fehler dabei ist kein eigener Befund.
        let _ = systemctl_run(ctx, systemctl, &["--user", "stop", unit::SOCKET_NAME]).await;
    }
    let links_back = rollback
        .enabled_before
        .rollback(rollback.dir, rollback.package)
        .map_err(|diagnostic| diagnostic.why);
    let restored = rollback.shadow.restore();
    // Verweise, die nicht zurückkamen, stehen im Befund; über den zweiten
    // Neustart entscheidet die Datei: Liegt sie wieder da, lädt systemd sie.
    let links_note: String = [links_back.err(), restored.links.err()]
        .into_iter()
        .flatten()
        .fold(String::new(), |mut note, why| {
            note.push_str("; ");
            note.push_str(&why);
            note
        });
    let reload = systemctl_run(ctx, systemctl, &["--user", "daemon-reload"]).await;
    let restart_again = restored.file.is_ok() && reload.is_ok() && rollback.was_active;
    if !restart_again {
        // Nur aufräumen; das Ergebnis ändert den Befund nicht.
        for verb in ["stop", "reset-failed"] {
            let _ = systemctl_run(ctx, systemctl, &["--user", verb, unit::UNIT_NAME]).await;
        }
    }
    let shown = rollback.shadow.path.display();
    let undone = if let Err(file) = restored.file {
        format!("putting {shown} back failed ({file}), so the service was not restarted again")
    } else {
        match reload {
            Err(error) => format!(
                "{shown} is back in place, but systemctl --user daemon-reload failed ({error}), \
                 so the service was not restarted again"
            ),
            Ok(()) if !restart_again => {
                format!("{shown} is back in place, and the service stays stopped as before")
            }
            Ok(()) => match systemctl_run(ctx, systemctl, &call).await {
                Ok(()) => {
                    format!("{shown} is back in place and the service was restarted on it")
                }
                Err(again) => format!(
                    "{shown} is back in place, but restarting the service on it failed as well \
                     ({again})"
                ),
            },
        }
    };
    Err(Failure::new(
        Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
            .why(format!(
                "systemctl {} on the package unit did not go through ({why}); \
                 {undone}{links_note}",
                call.join(" ")
            ))
            .fix(unit_fix(&[
                "systemctl",
                "--user",
                "status",
                unit::UNIT_NAME,
            ]))
            .build(),
    ))
}

/// Was systemd für den Dienst geladen hat: die Datei und das Programm aus
/// `ExecStart`, aus `systemctl --user show` (HUM-211).
///
/// `None`, wenn `systemctl` nicht antwortet oder die Antwort keine der beiden
/// Angaben trägt; der Bericht nennt dann, was in der Datei des Pakets steht.
async fn loaded_unit(ctx: &Context, systemctl: &Path) -> Option<(PathBuf, PathBuf)> {
    let args = [
        "--user",
        "show",
        "-p",
        "FragmentPath,ExecStart",
        unit::UNIT_NAME,
    ];
    let text = systemctl_capture(ctx, systemctl, &args).await.ok()?;
    parse_show(&text)
}

/// Liest `FragmentPath=` und das `path=` aus `ExecStart=` einer Antwort von
/// `systemctl show`.
///
/// `ExecStart` steht dort nicht wie in der Unit, sondern als
/// `{ path=/usr/bin/humanitld ; argv[]=/usr/bin/humanitld ; … }`.
fn parse_show(text: &str) -> Option<(PathBuf, PathBuf)> {
    let mut fragment = None;
    let mut exec = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("FragmentPath=") {
            fragment = Some(value.trim()).filter(|value| !value.is_empty());
        } else if let Some(value) = line.strip_prefix("ExecStart=") {
            exec = value
                .trim()
                .trim_start_matches('{')
                .split(';')
                .find_map(|part| part.trim().strip_prefix("path="))
                .map(str::trim)
                .filter(|path| !path.is_empty());
        }
    }
    Some((PathBuf::from(fragment?), PathBuf::from(exec?)))
}

/// Eine eigene ältere Unit unter `~/.config/systemd/user`, die die Unit des
/// Pakets verdeckt, und wie sie beiseite- und zurückgelegt wird.
#[derive(Debug)]
struct Shadow {
    /// `~/.config/systemd/user/humanitld.service`.
    path: PathBuf,
    /// Wohin sie gelegt wird: `humanitld.service.bak`, oder ein nummerierter
    /// Name daneben, wenn es den schon gibt.
    aside: PathBuf,
    /// Gerät und Inode der Datei, wie [`Shadow::find`] sie gesehen hat.
    /// [`Shadow::set_aside`] bewegt nur genau diese Datei.
    identity: (u64, u64),
    /// Die Verweise der Aktivierung, die auf sie zeigen, mit ihrem Ziel, wie
    /// es im Verweis steht.
    links: Vec<(PathBuf, PathBuf)>,
    /// Die Verweise, die [`Shadow::set_aside`] wirklich entfernt hat. Nur
    /// sie legt [`Shadow::restore`] zurück.
    removed: RefCell<Vec<(PathBuf, PathBuf)>>,
}

/// Was [`Shadow::restore`] zurückgelegt hat: die Datei und die Verweise,
/// getrennt, denn über einen zweiten Start entscheidet nur die Datei.
#[derive(Debug)]
struct Restored {
    /// Ob die Unit wieder unter ihrem Namen liegt; sonst der Satz, warum nicht.
    file: Result<(), String>,
    /// Ob die entfernten Verweise zurück sind; sonst der Satz, welche nicht.
    links: Result<(), String>,
}

impl Shadow {
    /// Sieht nach, ob unter `dir` eine Unit liegt, die `package` verdeckt.
    /// Liest nur.
    ///
    /// # Errors
    ///
    /// `DAEMON_005`, wenn die Datei nicht die Marke trägt oder keine
    /// gewöhnliche Datei ist; sie gehört dann jemand anderem und bleibt, wo
    /// sie ist. `DAEMON_006`, wenn sie sich nicht lesen lässt oder es keinen
    /// freien Namen zum Beiseitelegen gibt.
    fn find(dir: &Path, package: &Path) -> Result<Option<Self>, Diagnostic> {
        let path = dir.join(unit::UNIT_NAME);
        let metadata = match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(unreadable(&path, &error.to_string())),
            Ok(metadata) => metadata,
        };
        // Ein Verweis oder etwas anderes als eine Datei hat `daemon install`
        // nie angelegt.
        if !metadata.is_file() {
            return Err(hides_package(&path, package, Foreign::NotAFile));
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| unreadable(&path, &error.to_string()))?;
        if !unit::carries_marker(&text) {
            return Err(hides_package(&path, package, Foreign::NoMarker));
        }
        let aside = free_aside(&path)?;
        let links = unit::enablement_links(dir, &[unit::UNIT_NAME])
            .into_iter()
            .filter_map(|link| {
                let target = std::fs::read_link(&link).ok()?;
                let resolved = if target.is_absolute() {
                    target.clone()
                } else {
                    link.parent()?.join(&target)
                };
                let points_here = resolved == path
                    || std::fs::canonicalize(&resolved).ok() == std::fs::canonicalize(&path).ok();
                points_here.then_some((link, target))
            })
            .collect();
        Ok(Some(Self {
            path,
            aside,
            identity: (metadata.dev(), metadata.ino()),
            links,
            removed: RefCell::new(Vec::new()),
        }))
    }

    /// Wahr, wenn unter `path` noch genau die Datei liegt, die [`Shadow::find`]
    /// gesehen hat: eine gewöhnliche Datei mit demselben Gerät und Inode.
    fn is_the_same(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|metadata| {
            metadata.is_file() && (metadata.dev(), metadata.ino()) == self.identity
        })
    }

    /// Legt die Unit beiseite: erst ihre Verweise, dann die Datei.
    ///
    /// **Direkt davor wird noch einmal geprüft**, dass dort dieselbe Datei
    /// liegt wie bei [`Shadow::find`] und sie die Marke noch trägt; sonst
    /// geschieht nichts. Zwischen der Ankündigung und diesem Schritt kann
    /// jemand die Datei ersetzt haben, und bewegt wird nur, was angekündigt
    /// war.
    ///
    /// Die Verweise gehen mit, weil `systemctl --user enable` einen Verweis,
    /// der schon dasteht und woandershin zeigt, nicht ersetzt, sondern mit
    /// „already exists" abbricht. Entfernt wird nur ein Verweis, der in diesem
    /// Augenblick noch genau dorthin zeigt, wohin er bei [`Shadow::find`]
    /// zeigte, geprüft an dem, was entfernt würde ([`unit::remove_link_if`]);
    /// jeder andere bleibt liegen, und nur die entfernten legt
    /// [`Shadow::restore`] zurück. Die Datei geht mit einem einzigen
    /// `renameat2(RENAME_NOREPLACE)` ([`unit::move_no_replace`]): Nichts wird
    /// überschrieben, und kein zweiter Schritt löscht per Name, was
    /// inzwischen unter dem alten Namen liegen könnte. Danach wird am neuen
    /// Namen nachgesehen ([`Shadow::move_checked`]). Misslingt ein Schritt, ist
    /// danach alles wieder wie vorher.
    ///
    /// # Errors
    ///
    /// `DAEMON_005`, wenn die Datei inzwischen eine andere ist; `DAEMON_006`,
    /// wenn sich Datei oder Verweis nicht bewegen lassen.
    fn set_aside(&self) -> Result<(), Diagnostic> {
        let marked = self.is_the_same(&self.path)
            && std::fs::read_to_string(&self.path).is_ok_and(|text| unit::carries_marker(&text));
        if !marked {
            return Err(changed(&self.path));
        }
        let mut removed = Vec::new();
        for (link, target) in &self.links {
            match unit::remove_link_if(link, |now| now == target.as_path()) {
                Ok(true) => removed.push((link.clone(), target.clone())),
                Ok(false) => {}
                Err(error) => {
                    let _ = relink(&removed);
                    return Err(immovable(link, &error.to_string()));
                }
            }
        }
        if let Err(diagnostic) = self.move_checked() {
            let _ = relink(&removed);
            return Err(diagnostic);
        }
        *self.removed.borrow_mut() = removed;
        Ok(())
    }

    /// Bewegt die Datei an den neuen Namen und sieht dort nach, dass es die
    /// angekündigte ist: dieselbe Inode, die Marke in der ersten Zeile. Ist
    /// sie es nicht, weil jemand sie zwischen Prüfung und Umbenennen ersetzt
    /// hat, geht sie auf demselben Weg zurück und nichts ist geschehen.
    ///
    /// # Errors
    ///
    /// `DAEMON_005`, wenn am neuen Namen eine andere Datei liegt;
    /// `DAEMON_006`, wenn das Umbenennen scheitert.
    fn move_checked(&self) -> Result<(), Diagnostic> {
        unit::move_no_replace(&self.path, &self.aside)
            .map_err(|error| immovable(&self.path, &error.to_string()))?;
        let marked = self.is_the_same(&self.aside)
            && std::fs::read_to_string(&self.aside).is_ok_and(|text| unit::carries_marker(&text));
        if !marked {
            return Err(match unit::move_no_replace(&self.aside, &self.path) {
                Ok(()) => changed(&self.path),
                Err(error) => stuck_aside(&self.path, &self.aside, &error.to_string()),
            });
        }
        Ok(())
    }

    /// Legt die Unit zurück: erst die Datei, dann die Verweise, die
    /// [`Shadow::set_aside`] entfernt hat. Einen Verweis, den es liegen ließ,
    /// fasst es nicht an. Kommt die Datei nicht zurück, bleiben auch die
    /// Verweise weg: Sie zeigten auf nichts.
    fn restore(&self) -> Restored {
        let file = unit::move_no_replace(&self.aside, &self.path).map_err(|error| {
            format!(
                "{} cannot be moved back to {}: {error}",
                self.aside.display(),
                self.path.display()
            )
        });
        let links = match file {
            Ok(()) => relink(&self.removed.borrow()),
            Err(_) => Ok(()),
        };
        Restored { file, links }
    }

    /// Der Hinweis, wenn die Unit liegen bleibt, weil nichts aktiviert wird.
    fn left_in_place(&self) -> String {
        format!(
            "{} still hides the package unit; humanitl daemon install without --no-start and \
             with systemctl in PATH moves it aside, or by hand: mv -n -- {} {}",
            self.path.display(),
            shell_path(&self.path),
            shell_path(&self.aside)
        )
    }
}

/// Legt Verweise wieder an; der Satz nennt jeden, der nicht zurückkam. Ein
/// Verweis, der schon genau dorthin zeigt, zählt als zurück: Ihn hat
/// [`Shadow::set_aside`] liegen lassen.
fn relink(links: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    let left: Vec<String> = links
        .iter()
        .filter(|(link, target)| std::fs::read_link(link).ok().as_ref() != Some(target))
        .filter_map(|(link, target)| {
            let made = link
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::os::unix::fs::symlink(target, link));
            made.err()
                .map(|error| format!("{} ({error})", link.display()))
        })
        .collect();
    if left.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the enablement links did not come back: {}",
            left.join(", ")
        ))
    }
}

/// Der erste Name neben `path`, unter dem noch nichts liegt.
fn free_aside(path: &Path) -> Result<PathBuf, Diagnostic> {
    let mut base = path.as_os_str().to_owned();
    base.push(ASIDE_SUFFIX);
    let base = PathBuf::from(base);
    std::iter::once(base.clone())
        .chain((1..ASIDE_ATTEMPTS).map(|n| {
            let mut numbered = base.as_os_str().to_owned();
            numbered.push(format!(".{n}"));
            PathBuf::from(numbered)
        }))
        .find(|candidate| std::fs::symlink_metadata(candidate).is_err())
        .ok_or_else(|| {
            immovable(
                path,
                &format!(
                    "has no free name to be moved to; {} and its numbered siblings exist",
                    base.display()
                ),
            )
        })
}

/// Warum eine Datei an der Stelle der Unit jemand anderem gehört.
#[derive(Debug, Clone, Copy)]
enum Foreign {
    /// Eine gewöhnliche Datei ohne die Marke in der ersten Zeile.
    NoMarker,
    /// Ein Verweis oder etwas anderes als eine gewöhnliche Datei.
    NotAFile,
}

/// `DAEMON_005`: Eine Unit, die Humanitl nicht geschrieben hat, verdeckt die
/// Unit des Pakets.
///
/// Der Vorschlag legt sie unter einen freien Namen, und `mv -n` überschreibt
/// auch dann nichts, wenn der Name inzwischen vergeben ist.
fn hides_package(path: &Path, package: &Path, foreign: Foreign) -> Diagnostic {
    let reason = match foreign {
        Foreign::NoMarker => "does not start with the line Humanitl writes",
        Foreign::NotAFile => {
            "is not a regular file (a symbolic link or another kind of file), which humanitl \
             daemon install never creates"
        }
    };
    let aside = free_aside(path).unwrap_or_else(|_| {
        let mut base = path.as_os_str().to_owned();
        base.push(ASIDE_SUFFIX);
        PathBuf::from(base)
    });
    Diagnostic::builder(codes::DAEMON_005, Severity::Blocking)
        .why(format!(
            "{} hides the package unit {}: systemd loads the file under ~/.config first, and it \
             {reason}, so Humanitl did not write it and leaves it alone; move it away, or put \
             your changes into a drop-in with systemctl --user edit {} instead",
            path.display(),
            package.display(),
            unit::UNIT_NAME
        ))
        .fix(FixAction::CopyCommand(format!(
            "mv -n -- {} {}",
            shell_path(path),
            shell_path(&aside)
        )))
        .build()
}

/// `DAEMON_005`: Die Datei ist zwischen Prüfung und Bewegen eine andere
/// geworden; sie bleibt, wo sie ist.
fn changed(path: &Path) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_005, Severity::Blocking)
        .why(format!(
            "{} changed between the check and the move, so it is no longer the unit humanitl \
             daemon install announced; nothing was moved",
            path.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "ls -l -- {}",
            shell_path(path)
        )))
        .build()
}

/// `DAEMON_005`: Am neuen Namen lag eine andere Datei, und sie ließ sich
/// nicht zurücklegen; der Satz sagt, wo sie jetzt liegt.
fn stuck_aside(path: &Path, aside: &Path, error: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_005, Severity::Blocking)
        .why(format!(
            "{} changed between the check and the move, and moving it back failed ({error}); \
             the file now lies at {}",
            path.display(),
            aside.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "mv -n -- {} {}",
            shell_path(aside),
            shell_path(path)
        )))
        .build()
}

/// `DAEMON_006`: Die Unit ließ sich nicht lesen.
fn unreadable(path: &Path, error: &str) -> Diagnostic {
    immovable(path, &format!("cannot be read: {error}"))
}

/// `DAEMON_006`: Eine Datei oder ein Verweis ließ sich nicht bewegen.
fn immovable(path: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_006, Severity::Blocking)
        .why(format!("{} {why}", path.display()))
        .fix(FixAction::CopyCommand(format!(
            "ls -ld -- {}",
            shell_path(path)
        )))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use super::{Shadow, parse_show, unit};

    /// `systemctl show` schreibt `ExecStart` als Aufzählung; gemeint ist
    /// `path=`.
    #[test]
    fn the_show_answer_yields_fragment_and_program() {
        let text = "ExecStart={ path=/usr/lib/humanitl/humanitld ; argv[]=/usr/lib/humanitl/\
                    humanitld --flag ; ignore_errors=no ; start_time=[n/a] ; pid=0 }\n\
                    FragmentPath=/usr/lib/systemd/user/humanitld.service\n";
        assert_eq!(
            parse_show(text),
            Some((
                PathBuf::from("/usr/lib/systemd/user/humanitld.service"),
                PathBuf::from("/usr/lib/humanitl/humanitld"),
            ))
        );
        assert_eq!(parse_show("FragmentPath=\nExecStart=\n"), None);
    }

    /// Wird die Datei zwischen Prüfung und Bewegen ersetzt, auch durch eine
    /// mit der Marke, bewegt `set_aside` nichts: weder die Datei noch ihren
    /// Verweis (HUM-211, Review).
    #[test]
    fn a_unit_replaced_after_the_check_is_not_moved() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(unit::UNIT_NAME);
        std::fs::write(&path, format!("{}\n[Service]\n", unit::MARKER)).expect("the unit");
        let wants = dir.path().join("default.target.wants");
        std::fs::create_dir_all(&wants).expect("wants");
        let link = wants.join(unit::UNIT_NAME);
        std::os::unix::fs::symlink(&path, &link).expect("the link");
        let shadow = Shadow::find(dir.path(), &dir.path().join("package.service"))
            .expect("found")
            .expect("a shadow");

        // Eine neue Datei mit eigener Inode, dann per `rename` an die Stelle.
        let other = format!("{}\n[Service]\nExecStart=/somewhere/else\n", unit::MARKER);
        let scratch = dir.path().join("scratch");
        std::fs::write(&scratch, &other).expect("the replacement");
        std::fs::rename(&scratch, &path).expect("replaced");

        let error = shadow.set_aside().expect_err("a changed file is not moved");
        assert_eq!(error.code.as_str(), "DAEMON_005", "{}", error.why);
        assert_eq!(std::fs::read_to_string(&path).expect("still there"), other);
        assert!(!shadow.aside.exists(), "nothing was moved aside");
        assert_eq!(std::fs::read_link(&link).expect("the link stays"), path);
    }

    /// Zeigt ein Verweis nach der Prüfung woandershin, lässt `set_aside` ihn
    /// liegen, und `restore` legt nur zurück, was es entfernt hat: Die Datei
    /// kommt zurück, der geänderte Verweis bleibt, wie er ist, und die
    /// Rücknahme meldet keinen Fehler (HUM-211, Review).
    #[test]
    fn a_link_changed_after_the_check_stays_and_restore_keeps_it() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(unit::UNIT_NAME);
        std::fs::write(&path, format!("{}\n[Service]\n", unit::MARKER)).expect("the unit");
        let wants = dir.path().join("default.target.wants");
        std::fs::create_dir_all(&wants).expect("wants");
        let link = wants.join(unit::UNIT_NAME);
        std::os::unix::fs::symlink(&path, &link).expect("the link");
        let shadow = Shadow::find(dir.path(), &dir.path().join("package.service"))
            .expect("found")
            .expect("a shadow");
        let elsewhere = dir.path().join("elsewhere.service");
        std::fs::remove_file(&link).expect("the link goes");
        std::os::unix::fs::symlink(&elsewhere, &link).expect("a changed link");

        shadow.set_aside().expect("the file goes aside");
        assert_eq!(
            std::fs::read_link(&link).expect("the changed link stays"),
            elsewhere
        );

        let restored = shadow.restore();
        assert_eq!(restored.file, Ok(()));
        assert_eq!(restored.links, Ok(()));
        assert!(path.is_file(), "the unit is back");
        assert_eq!(
            std::fs::read_link(&link).expect("still the changed link"),
            elsewhere
        );
    }

    /// Liegt beim Umbenennen schon eine andere Datei an der Stelle, auch mit
    /// der Marke, sieht `move_checked` das am neuen Namen und legt sie
    /// zurück: Sie bleibt unter ihrem Namen, und unter `.bak` liegt nichts
    /// (HUM-211, Review).
    #[test]
    fn a_file_swapped_in_before_the_rename_goes_back() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(unit::UNIT_NAME);
        std::fs::write(&path, format!("{}\n[Service]\n", unit::MARKER)).expect("the unit");
        let shadow = Shadow::find(dir.path(), &dir.path().join("package.service"))
            .expect("found")
            .expect("a shadow");
        let other = format!("{}\n[Service]\nExecStart=/swapped\n", unit::MARKER);
        let scratch = dir.path().join("scratch");
        std::fs::write(&scratch, &other).expect("the replacement");
        std::fs::rename(&scratch, &path).expect("swapped");

        let error = shadow
            .move_checked()
            .expect_err("another file is not kept aside");
        assert_eq!(error.code.as_str(), "DAEMON_005", "{}", error.why);
        assert_eq!(
            std::fs::read_to_string(&path).expect("back in place"),
            other
        );
        assert!(!shadow.aside.exists(), "nothing stays aside");
    }
}
