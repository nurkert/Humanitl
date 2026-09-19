//! `humanitl daemon status` und `humanitl daemon install`.
//!
//! `status` ist ein dünner Client (ADR-018): verbinden, `GetInfo` rufen,
//! ausgeben. Was der Daemon kann, sagt er selbst; die Kommandozeile erfindet
//! nichts dazu.
//!
//! `install` ist das eine Unterkommando, das etwas auf dem Rechner des
//! Menschen zurücklässt. Was es schreibt und wohin, steht in [`crate::cmd::unit`];
//! hier steht, was drum herum geschieht, und das sind vier Zusagen:
//!
//! - **Es zeigt die Datei, bevor es sie schreibt.** Der ganze Text der Unit
//!   und beide `systemctl`-Aufrufe gehen vor dem ersten Schreibzugriff auf
//!   `stderr`, an [`crate::render::Renderer`] vorbei ([`announce`]); `-q`
//!   schaltet das nicht ab. Unter `--json` liest ein Programm, und dem gehört
//!   ein Objekt auf `stdout` und ein leeres `stderr` (`docs/cli.md`): Text und
//!   Befehle stehen dann in diesem Objekt (`unit_text`, `commands`).
//! - **Erst die Prüfungen, dann die Kopie.** Aus einem `AppImage` werden
//!   Daemon und Shim nach `~/.local/lib/humanitl/<version>.<stempel>/`
//!   kopiert. Das geschieht erst nach der Ankündigung und nach jeder Prüfung,
//!   die den Lauf ablehnen kann (`DAEMON_005`, `DAEMON_010`); scheitert danach
//!   noch etwas, zeigt `current` wieder dorthin, wohin es vorher zeigte, und
//!   die neue Kopie geht ([`Staged`]).
//! - **`--print` schreibt nichts.** Wer erst lesen will, bekommt genau
//!   dieselbe Datei zu sehen, die der Aufruf ohne den Schalter schriebe.
//! - **Die Units des Pakets werden nicht kopiert.** Liegen sie unter
//!   `/usr/lib/systemd/user`, schreibt der Befehl nichts und aktiviert Socket
//!   und Dienst des Pakets ([`install_packaged`], HUM-053). Das gilt nicht aus
//!   einem `AppImage` und nicht mit `--bin-dir`: Beide nennen ausdrücklich
//!   einen anderen Daemon.
//! - **Nie mit `sudo`.** Der Daemon ist ein Nutzerdienst; jeder Aufruf hier
//!   ist `systemctl --user`.
//! - **Ein Fehlschlag lässt nichts liegen.** Nimmt systemd die Unit nicht an,
//!   wird der Zustand von vorher wiederhergestellt — die Datei **und** die
//!   Verweise, die `enable --now` angelegt hat — und `daemon-reload` noch
//!   einmal gefahren, damit auch systemds Bild davon stimmt.
//!
//! Die erste Zusage hängt an der Reihenfolge in [`install`]: Der Plan aus
//! [`crate::cmd::unit::prepare`] steht fest, bevor [`announce`] etwas sagt.
//! `prepare` liest nur, also ist vor der Ankündigung nach wie vor nichts
//! geschrieben — aber die Ankündigung nennt jetzt, was wirklich geschieht, und
//! nicht, was ein Aufruf im Regelfall täte.

use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, v1};
use serde_json::json;

use crate::cli::{DaemonCmd, InstallArgs, LogsArgs};
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic, unit};
use crate::render::{table, tick};

/// Wie lange ein `systemctl`-Aufruf höchstens dauern darf.
///
/// Ohne Frist hinge `daemon install` an einem systemd, das seinen Bus nicht
/// findet — und der Mensch säße vor einem Befehl, der eine Datei geschrieben
/// hat und nicht mehr zurückkommt.
const SYSTEMCTL_TIMEOUT: Duration = Duration::from_secs(20);

/// Das Wort, unter dem `--print` in der Ausgabe steht.
const PRINT_ACTION: &str = "print";

/// Das Wort, unter dem ein Lauf steht, der die Units des Pakets nur
/// aktiviert und nichts geschrieben hat (HUM-053).
const PACKAGED_ACTION: &str = "packaged";

/// Wie lange `daemon install` auf die erste Antwort des Daemons wartet.
///
/// Der Befehl ist erst fertig, wenn der Dienst redet. Ohne diese Runde endete
/// er mit einem Häkchen hinter `systemctl`, während der Daemon längst
/// abgestürzt wäre, und der Mensch erführe es beim nächsten Befehl.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Der Abstand zwischen zwei Versuchen, den Daemon zu erreichen.
const READY_PAUSE: Duration = Duration::from_millis(100);

/// Das Verzeichnis unter `$HOME`, in das ein `AppImage` seine Binaries legt.
///
/// Der Pfad `/tmp/.mount_*` eines laufenden `AppImage`s verschwindet mit dem
/// Prozess; ein `ExecStart` darauf zeigte beim nächsten Anmelden ins Leere
/// (HUM-070). Deshalb werden Daemon und Shim herauskopiert.
const LIB_DIR: &str = ".local/lib/humanitl";

/// Der Name des Verweises auf die zuletzt installierte Fassung.
const CURRENT_LINK: &str = "current";

/// Die Binaries, die ein `AppImage`-Lauf herauskopiert.
const STAGED_BINARIES: [&str; 2] = [unit::DAEMON_NAME, "humanitl-shim"];

/// Die Variablen, die `systemctl --user` mitbekommt.
///
/// Nicht die ganze Umgebung: In ihr stehen Tokens und Schlüssel des Nutzers.
/// Nicht die leere: Ohne `$XDG_RUNTIME_DIR` und `$DBUS_SESSION_BUS_ADDRESS`
/// findet `systemctl --user` den Bus der Sitzung nicht und antwortet „Failed
/// to connect to user scope bus" — dieselbe Lehre wie in
/// `humanitl_sandbox::doctor` (HUM-075, Punkt 5).
const SESSION_ENV_KEYS: &[&str] = &[
    "PATH",
    "HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// Führt `humanitl daemon <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `DAEMON_002`, wenn er eine
/// andere Major-Version des Vertrags spricht, `DAEMON_005` bis `DAEMON_008`,
/// `DAEMON_010` und `DAEMON_011` für die Wege, auf denen `install` nicht
/// durchkommt, `DAEMON_010` und `DAEMON_012` für `logs`.
pub async fn run(ctx: &Context, cmd: &DaemonCmd) -> Result<u8, Failure> {
    match cmd {
        DaemonCmd::Status => status(ctx).await,
        DaemonCmd::Install(args) => install(ctx, args).await,
        DaemonCmd::Logs(args) => logs(ctx, args),
    }
}

/// Was aus dem Versuch wurde, systemd von der Unit zu erzählen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activation {
    /// `daemon-reload` und `enable --now` sind durchgelaufen.
    Enabled,
    /// `--no-start`: Es wurde nichts gerufen.
    Skipped,
    /// Es gibt kein `systemctl` in `PATH`. Die Unit liegt trotzdem richtig da.
    NoSystemctl,
}

impl Activation {
    /// Das Wort für die Ausgabe.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Skipped => "skipped",
            Self::NoSystemctl => "no systemctl",
        }
    }
}

/// `daemon install`.
async fn install(ctx: &Context, args: &InstallArgs) -> Result<u8, Failure> {
    let current = std::env::current_exe().map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::DAEMON_007, Severity::Blocking)
                .why(format!(
                    "the path of the running humanitl could not be read ({error}), so the unit \
                     has no ExecStart to name"
                ))
                .build(),
        )
    })?;
    let source = args.bin_dir.clone().unwrap_or_else(|| {
        current
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    });
    let appimage = ctx.env.non_empty("APPIMAGE").is_some();
    if let Some(units) = packaged_units(args, appimage) {
        return install_packaged(ctx, args, &units).await;
    }
    let daemon = exec_start(ctx, args, &current, &source, appimage)?;
    let contents = unit::render(&daemon).map_err(Failure::new)?;
    let path = unit::unit_path(&ctx.paths);
    let systemctl = find_in_path(ctx, "systemctl");
    let names = [unit::UNIT_NAME];
    let commands = planned_commands(args.no_start, systemctl.as_deref(), &names);

    // Ohne Nutzersitzung nimmt systemd die Unit nicht an. Das steht vor dem
    // ersten Schreibzugriff fest, und der Befund nennt den einen Fix, der
    // hilft, statt nach einem gescheiterten `systemctl` ein `DAEMON_008` mit
    // `systemctl --user status` vorzuschlagen, das ebenso scheitert.
    require_user_session(ctx, args)?;

    let mut result = InstallReport {
        unit: &path,
        exec_start: &daemon,
        unit_text: &contents,
        commands: &commands,
        enable: &names,
        action: PRINT_ACTION,
        activation: Activation::Skipped,
        binaries: None,
        ready: None,
    };
    if args.print {
        if !ctx.render.is_json() {
            announce(
                &headline(&path, None),
                &contents,
                args,
                systemctl.as_deref(),
                &names,
            );
        }
        report(ctx, &result);
        return Ok(EXIT_OK);
    }

    // Erst den Plan, dann die Ankündigung. [`unit::prepare`] liest nur; es
    // schreibt nichts, und die Zusage „sichtbar, bevor es geschieht" bleibt
    // damit unberührt. Umgekehrt wäre die Ankündigung eine Behauptung: Liegt
    // dort die Unit von jemand anderem (`DAEMON_005`) oder steht schon genau
    // dieser Text da, dann hätte der Mensch gelesen, dass seine Datei
    // geschrieben wird, und geschrieben wird sie nicht.
    let plan = unit::prepare(&path, &contents).map_err(Failure::new)?;
    if !ctx.render.is_json() {
        announce(
            &headline(&path, Some(&plan)),
            &contents,
            args,
            systemctl.as_deref(),
            &names,
        );
    }

    let staged = if appimage {
        Some(stage(ctx, &source).map_err(Failure::new)?)
    } else {
        None
    };
    let written = UnitOnDisk {
        path: &path,
        plan: &plan,
    };
    let finished = match unit::write(&path, &contents, &plan) {
        Ok(()) => start(ctx, args, systemctl.as_deref(), &names, &written).await,
        Err(diagnostic) => Err(Failure::new(diagnostic)),
    };
    let activation = match finished {
        Ok(activation) => activation,
        Err(failure) => {
            // Die Unit ist zurückgenommen (`activate`) oder nie geschrieben
            // worden; also zeigt auch `current` wieder dorthin, wohin es
            // vorher zeigte. Sonst startete die alte Unit die neuen Binaries.
            if let Some(staged) = staged.as_ref() {
                staged.restore();
            }
            return Err(failure);
        }
    };

    let ready = if activation == Activation::Enabled {
        Some(wait_for_daemon(ctx).await)
    } else {
        None
    };
    if let Some(staged) = staged.as_ref() {
        staged.retire_previous();
    }
    result.action = plan.as_str();
    result.activation = activation;
    result.binaries = staged.as_ref().map(|staged| staged.dir.as_path());
    result.ready = ready.as_deref();
    report(ctx, &result);
    Ok(EXIT_OK)
}

/// `daemon install`, wenn das Paket die Units unter
/// [`unit::SYSTEM_UNIT_DIR`] abgelegt hat (HUM-053).
///
/// Es wird nichts geschrieben: Die Units gehören dem Paket, und eine Kopie
/// unter `~/.config/systemd/user` verdeckte sie. Übrig bleibt, was das Paket
/// nicht kann, weil es als root läuft: `systemctl --user daemon-reload` und
/// `enable --now` für Socket und Dienst. Dieselben Zusagen wie beim Schreiben
/// gelten: Die Ankündigung kommt vorher, `--print` ruft nichts, und ein
/// gescheitertes `enable` nimmt die Verweise zurück, die es angelegt hat.
async fn install_packaged(
    ctx: &Context,
    args: &InstallArgs,
    units: &unit::SystemUnits,
) -> Result<u8, Failure> {
    let systemctl = find_in_path(ctx, "systemctl");
    let names = units.names();
    let commands = planned_commands(args.no_start, systemctl.as_deref(), &names);
    require_user_session(ctx, args)?;

    let exec = units.exec_start();
    let mut result = InstallReport {
        unit: &units.service,
        exec_start: &exec,
        unit_text: &units.text,
        commands: &commands,
        enable: &names,
        action: PRINT_ACTION,
        activation: Activation::Skipped,
        binaries: None,
        ready: None,
    };
    if !ctx.render.is_json() {
        let shown = match units.socket.as_ref() {
            Some(socket) => format!("{} and {}", units.service.display(), socket.display()),
            None => units.service.display().to_string(),
        };
        announce(
            &format!(
                "humanitl daemon install writes nothing: the package installed {shown}, and \
                 {} is:",
                unit::UNIT_NAME
            ),
            &units.text,
            args,
            systemctl.as_deref(),
            &names,
        );
    }
    if args.print {
        report(ctx, &result);
        return Ok(EXIT_OK);
    }

    let nothing = UnitOnDisk {
        path: &units.service,
        plan: &unit::Written::Unchanged,
    };
    let activation = start(ctx, args, systemctl.as_deref(), &names, &nothing).await?;
    let ready = if activation == Activation::Enabled {
        Some(wait_for_daemon(ctx).await)
    } else {
        None
    };
    result.action = PACKAGED_ACTION;
    result.activation = activation;
    result.ready = ready.as_deref();
    report(ctx, &result);
    Ok(EXIT_OK)
}

/// Die Units des Pakets, wenn dieser Lauf sie aktivieren soll (HUM-053).
///
/// Nicht aus einem `AppImage` und nicht mit `--bin-dir`: Beide nennen
/// ausdrücklich einen anderen Daemon als den des Pakets.
fn packaged_units(args: &InstallArgs, appimage: bool) -> Option<unit::SystemUnits> {
    if appimage || args.bin_dir.is_some() {
        return None;
    }
    unit::SystemUnits::find(Path::new(unit::SYSTEM_UNIT_DIR))
}

/// Sagt systemd von den Units, wenn der Lauf das soll und kann.
async fn start(
    ctx: &Context,
    args: &InstallArgs,
    systemctl: Option<&Path>,
    names: &[&str],
    written: &UnitOnDisk<'_>,
) -> Result<Activation, Failure> {
    match (args.no_start, systemctl) {
        (true, _) => Ok(Activation::Skipped),
        (false, None) => Ok(Activation::NoSystemctl),
        (false, Some(systemctl)) => {
            activate(ctx, systemctl, names, written).await?;
            Ok(Activation::Enabled)
        }
    }
}

/// `DAEMON_010`, wenn systemd gleich gebraucht wird und es keine
/// Nutzersitzung gibt.
///
/// Das steht vor dem ersten Schreibzugriff fest: `--print` und `--no-start`
/// brauchen die Sitzung nicht, alles andere schon.
fn require_user_session(ctx: &Context, args: &InstallArgs) -> Result<(), Failure> {
    if !args.print && !args.no_start && ctx.env.non_empty("XDG_RUNTIME_DIR").is_none() {
        return Err(Failure::new(no_user_session(
            "XDG_RUNTIME_DIR is not set, so systemctl --user has no user session to hand the \
             unit to; nothing was written",
        )));
    }
    Ok(())
}

/// Was in `ExecStart` steht.
///
/// Es steht fest, bevor irgendetwas geschieht, und zwar für `--print`, die
/// Ankündigung und die Unit gleich: Aus einem `AppImage` heraus ist es der
/// Verweis `current` unter `~/.local/lib/humanitl/`, nie der Einhängepunkt
/// `/tmp/.mount_*`, den es nach dem Prozess nicht mehr gibt. Kopiert wird erst,
/// wenn alle Prüfungen durch sind ([`stage`]); hier wird nur geprüft, dass es
/// etwas zu kopieren gibt.
fn exec_start(
    ctx: &Context,
    args: &InstallArgs,
    current: &Path,
    source: &Path,
    appimage: bool,
) -> Result<PathBuf, Failure> {
    if appimage {
        for name in STAGED_BINARIES {
            let from = source.join(name);
            if !crate::cmd::is_executable(&from) {
                return Err(Failure::new(unit::missing_binary(
                    &from,
                    "there is no such executable in the AppImage next to the running humanitl",
                )));
            }
        }
        let exec = lib_base(ctx).join(CURRENT_LINK).join(unit::DAEMON_NAME);
        unit::exec_start_word(&exec).map_err(Failure::new)?;
        return Ok(exec);
    }
    match args.bin_dir.as_deref() {
        Some(dir) => unit::daemon_binary_in(dir).map_err(Failure::new),
        None => unit::daemon_binary(current).map_err(Failure::new),
    }
}

/// Die Aufrufe, die nach dem Schreiben folgen, als Text für die Ausgabe.
fn planned_commands(no_start: bool, systemctl: Option<&Path>, names: &[&str]) -> Vec<String> {
    match (no_start, systemctl) {
        (false, Some(systemctl)) => vec![
            format!("{} --user daemon-reload", systemctl.display()),
            format!(
                "{} --user enable --now {}",
                systemctl.display(),
                names.join(" ")
            ),
        ],
        _ => Vec::new(),
    }
}

/// Die Unit, die ein Lauf geschrieben hat, und wie sie zurückgeht.
///
/// Für das Paket steht hier dessen Unit mit [`unit::Written::Unchanged`]:
/// Geschrieben wurde nichts, also gibt es nichts zurückzunehmen außer den
/// Verweisen der Aktivierung.
struct UnitOnDisk<'a> {
    /// Der Pfad der Unit.
    path: &'a Path,
    /// Was dieser Lauf mit ihr gemacht hat.
    plan: &'a unit::Written,
}

/// `~/.local/lib/humanitl`, wo die Binaries eines `AppImage`s liegen.
fn lib_base(ctx: &Context) -> PathBuf {
    ctx.paths.home().join(LIB_DIR)
}

/// Was aus einem `AppImage` herauskopiert wurde, und wie es zurückgeht.
#[derive(Debug)]
struct Staged {
    /// Das Verzeichnis mit den Kopien, `~/.local/lib/humanitl/<version>.<stempel>`.
    dir: PathBuf,
    /// Der Verweis `current`.
    link: PathBuf,
    /// Wohin `current` vorher zeigte; `None`, wenn es ihn nicht gab.
    previous: Option<PathBuf>,
    /// Das Konto, dem das Heimatverzeichnis gehört; nur dessen Kopien gehen.
    owner: u32,
}

impl Staged {
    /// Nimmt die Kopie zurück: `current` zeigt wieder dorthin, wohin es vorher
    /// zeigte, und das neue Verzeichnis geht. Es hat es vorher nicht gegeben —
    /// jede Kopie bekommt ein eigenes —, also bleibt nichts liegen, was dieser
    /// Lauf angelegt hat.
    ///
    /// Ohne Prüfung des Ergebnisses: Es räumt auf, und der Befund, der gleich
    /// zurückgeht, sagt, warum.
    ///
    /// Die neue Kopie geht nur, wenn `current` nicht mehr auf sie zeigt:
    /// Misslingt das Zurückhängen, bleibt sie liegen, und `current` zeigt
    /// weiter auf vollständige Binaries statt ins Leere.
    fn restore(&self) {
        let released = match self.previous.as_ref() {
            Some(previous) => repoint(&self.link, previous).is_ok(),
            None => std::fs::remove_file(&self.link).is_ok(),
        };
        if released {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Nach dem Erfolg: Die Fassung, auf die `current` vorher zeigte, geht —
    /// aber nur, wenn sie ein eigenes Verzeichnis unter
    /// `~/.local/lib/humanitl` ist und nicht dieselbe wie die neue. Ein
    /// Verweis, der woandershin zeigte, wird nicht verfolgt.
    fn retire_previous(&self) {
        let Some(previous) = self.previous.as_ref() else {
            return;
        };
        let Some(base) = self.link.parent() else {
            return;
        };
        let previous = if previous.is_absolute() {
            previous.clone()
        } else {
            base.join(previous)
        };
        let inside = previous.parent() == Some(base);
        let real_dir = std::fs::symlink_metadata(&previous).is_ok_and(|meta| {
            use std::os::unix::fs::MetadataExt as _;
            meta.is_dir() && !meta.file_type().is_symlink() && meta.uid() == self.owner
        });
        if inside && real_dir && previous != self.dir {
            let _ = std::fs::remove_dir_all(&previous);
        }
    }
}

/// Kopiert Daemon und Shim aus dem `AppImage` nach `~/.local/lib/humanitl/`.
///
/// Ein `AppImage` liegt zur Laufzeit unter `/tmp/.mount_*`, und dieser Pfad
/// verschwindet mit dem Prozess. Ein `ExecStart` darauf zeigte beim nächsten
/// Anmelden ins Leere; deshalb bekommt jede Kopie ihr eigenes Verzeichnis, und
/// der Verweis `current` zeigt auf die zuletzt installierte. `ExecStart` nennt
/// den Verweis: Ein Update legt eine neue Kopie daneben und hängt den Verweis
/// um, ohne die Unit anzufassen.
///
/// Drei Regeln:
///
/// - **Nur in eigene Verzeichnisse.** Ist `~/.local/lib/humanitl` ein Verweis
///   oder gehört es einem anderen Konto, wird nichts kopiert: Die Binaries
///   landeten sonst dort, wohin der Verweis zeigt, und die Unit startete, was
///   jemand anderes dort hinlegt.
/// - **Nie ein Verzeichnis, auf das `current` gerade zeigt.** Jede Kopie
///   bekommt ein neues Verzeichnis `<version>.<stempel>`; das alte bleibt
///   unberührt, bis `current` auf das neue zeigt. Wer dieselbe Fassung ein
///   zweites Mal installiert und mittendrin abbricht, hat trotzdem ein
///   `current`, das auf vollständige Binaries zeigt.
/// - **`current` wird umgehängt, nicht gelöscht.** Ein neuer Verweis entsteht
///   unter einem anderen Namen und ersetzt den alten mit `rename`; einen
///   Augenblick ohne `current` gibt es nicht.
fn stage(ctx: &Context, source: &Path) -> Result<Staged, Diagnostic> {
    let base = lib_base(ctx);
    std::fs::create_dir_all(&base)
        .map_err(|error| not_staged(&base, &format!("cannot be created: {error}")))?;
    let owner = owner_of(&ctx.paths.home())?;
    own_directory(&base, owner)?;

    let dir = base.join(format!("{}.{}", env!("CARGO_PKG_VERSION"), stamp()));
    std::fs::create_dir(&dir)
        .map_err(|error| not_staged(&dir, &format!("cannot be created: {error}")))?;
    let copied = STAGED_BINARIES.iter().try_for_each(|name| {
        std::fs::copy(source.join(name), dir.join(name))
            .map(|_| ())
            .map_err(|error| not_staged(&dir.join(name), &format!("cannot be written: {error}")))
    });
    if let Err(diagnostic) = copied {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(diagnostic);
    }

    let link = base.join(CURRENT_LINK);
    let previous = match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::read_link(&link).ok(),
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(not_staged(
                &link,
                "is not a symbolic link, so it is not one this command made; nothing was changed",
            ));
        }
        Err(_) => None,
    };
    if let Err(diagnostic) = repoint(&link, &dir) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(diagnostic);
    }
    Ok(Staged {
        dir,
        link,
        previous,
        owner,
    })
}

/// Ein Stempel, den kein anderer Lauf trägt: Nanosekunden seit 1970 und die
/// Prozessnummer.
fn stamp() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{nanos}-{}", std::process::id())
}

/// Hängt den Verweis `link` auf `target` um, ohne einen Augenblick ohne ihn.
fn repoint(link: &Path, target: &Path) -> Result<(), Diagnostic> {
    let tmp = link.with_extension(format!("tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(target, &tmp)
        .and_then(|()| std::fs::rename(&tmp, link))
        .map_err(|error| {
            let _ = std::fs::remove_file(&tmp);
            not_staged(
                link,
                &format!("cannot point at {}: {error}", target.display()),
            )
        })
}

/// Das Konto, dem ein Pfad gehört.
fn owner_of(path: &Path) -> Result<u32, Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;

    std::fs::metadata(path)
        .map(|meta| meta.uid())
        .map_err(|error| not_staged(path, &format!("cannot be inspected: {error}")))
}

/// Prüft, dass `dir` ein echtes Verzeichnis dieses Kontos ist und kein Verweis.
fn own_directory(dir: &Path, owner: u32) -> Result<(), Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;

    let meta = std::fs::symlink_metadata(dir)
        .map_err(|error| not_staged(dir, &format!("cannot be inspected: {error}")))?;
    if meta.file_type().is_symlink() {
        return Err(not_staged(
            dir,
            "is a symbolic link; the binaries would land wherever it points, so nothing is copied",
        ));
    }
    if !meta.is_dir() {
        return Err(not_staged(dir, "is not a directory"));
    }
    if meta.uid() != owner {
        return Err(not_staged(
            dir,
            &format!(
                "belongs to uid {}, not to the owner of the home directory (uid {owner})",
                meta.uid()
            ),
        ));
    }
    Ok(())
}

/// `DAEMON_011`: Die Kopie aus dem `AppImage` ging nicht.
fn not_staged(path: &Path, what: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_011, Severity::Blocking)
        .why(format!(
            "{} {what}; the binaries of an AppImage have to leave it, because its \
             mount point disappears with the process",
            path.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "ls -ln {}",
            crate::render::shell_path(path.parent().unwrap_or(path))
        )))
        .build()
}

/// Wartet, bis der Daemon antwortet; die Fassung, die er nennt, oder ein Satz.
///
/// Kein Fehlschlag: Die Unit liegt, systemd hat sie genommen, und dass der
/// Dienst in fünf Sekunden noch nicht redet, ist eine Beobachtung und kein
/// Grund, die Installation zurückzunehmen.
async fn wait_for_daemon(ctx: &Context) -> String {
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    loop {
        if let Ok(mut client) = ctx.connect().await
            && let Ok(info) = client.get_info(()).await
        {
            return info.into_inner().daemon_version;
        }
        if tokio::time::Instant::now() >= deadline {
            return format!("no answer within {} ms", READY_TIMEOUT.as_millis());
        }
        tokio::time::sleep(READY_PAUSE).await;
    }
}

/// `daemon logs`: die Journal-Zeilen des Dienstes, ohne einen zweiten Leser.
///
/// `journalctl` bekommt das Terminal dieses Prozesses. Ein eigener Leser für
/// Journal-Einträge wäre eine zweite Quelle für dieselbe Wahrheit, mit eigenen
/// Formaten und eigenen Fehlern.
///
/// Sein Exit-Code wird nicht durchgereicht, sondern übersetzt: 0 bleibt 0,
/// alles andere wird 1. Eine 2 oder 4 von `journalctl` hieße hier sonst
/// „Daemon nicht erreichbar" oder „Sicherheitsverletzung"
/// (`backlog/CONVENTIONS.md` 3.8), und beides wäre gelogen.
fn logs(ctx: &Context, args: &LogsArgs) -> Result<u8, Failure> {
    if ctx.env.non_empty("XDG_RUNTIME_DIR").is_none() {
        return Err(Failure::new(no_user_session(
            "XDG_RUNTIME_DIR is not set, so there is no systemd user session \
             to read a journal from",
        )));
    }
    let Some(journalctl) = find_in_path(ctx, "journalctl") else {
        return Err(Failure::new(no_journalctl(
            "journalctl is not in PATH, and the journal of a user service is \
             only readable through it",
        )));
    };

    let mut command = std::process::Command::new(journalctl);
    command.args(["--user", "-u", unit::UNIT_NAME]);
    if let Some(lines) = args.lines {
        command.arg("-n").arg(lines.to_string());
    }
    if args.follow {
        command.arg("-f");
    }
    let status = command.status().map_err(|error| {
        Failure::new(no_journalctl(&format!("journalctl did not start: {error}")))
    })?;
    Ok(if status.success() {
        EXIT_OK
    } else {
        crate::cmd::EXIT_USER
    })
}

/// `DAEMON_010`: Es gibt keine Nutzersitzung, in der ein Nutzerdienst lebt.
fn no_user_session(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_010, Severity::Blocking)
        .why(why.to_owned())
        .fix(FixAction::CopyCommand(
            "loginctl enable-linger $USER".to_owned(),
        ))
        .build()
}

/// `DAEMON_012`: `journalctl` fehlt. Ein fehlendes Programm, keine fehlende
/// Sitzung; der Vorschlag ist das Paket.
fn no_journalctl(why: &str) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_012, Severity::Error)
        .why(why.to_owned())
        .fix(FixAction::CopyCommand(
            "sudo apt-get install systemd".to_owned(),
        ))
        .build()
}

/// Ob `systemctl` den Bus der Nutzersitzung nicht gefunden hat.
///
/// So antwortet `systemctl --user` über SSH ohne Linger, auch wenn
/// `XDG_RUNTIME_DIR` gesetzt ist; gemessen an „Failed to connect to bus: No
/// medium found" und „Failed to connect to user scope bus".
fn no_bus(why: &str) -> bool {
    why.contains("Failed to connect to bus") || why.contains("user scope bus")
}

/// Sagt systemd Bescheid, und nimmt zurück, was dieser Aufruf angerichtet hat,
/// wenn es nicht klappt.
///
/// Zurückgenommen wird beides, was ein Lauf hinterlassen kann: die Unit und
/// ihre Aktivierung. `systemctl --user enable --now` ist ein Aufruf mit zwei
/// Schritten — es legt die Verweise unter `<ziel>.wants/` an und startet dann
/// den Dienst —, und misslingt der Start, bleiben die Verweise stehen. Ohne
/// ihre Rücknahme wäre `daemon install` nicht das eine Geschäft, als das es in
/// `docs/cli.md` steht: Der Dienst startete beim nächsten Anmelden, obwohl der
/// Befehl mit `DAEMON_008` abgebrochen ist.
///
/// [`unit::Enablement`] nimmt den Zustand vor dem ersten Aufruf auf, damit
/// wirklich nur zurückgenommen wird, was dieser Lauf angelegt hat: Wer den
/// Dienst schon vorher aktiviert hatte, behält ihn.
///
/// `names` sind die Units, die `enable --now` bekommt: der Dienst, und bei
/// den Units des Pakets davor der Socket. Die Verweise der Aktivierung legt
/// `systemctl --user enable` immer im Unit-Verzeichnis des Nutzers an
/// ([`unit::unit_dir`]), auch für eine Unit des Pakets; dort sieht die
/// Rücknahme nach.
async fn activate(
    ctx: &Context,
    systemctl: &Path,
    names: &[&str],
    written: &UnitOnDisk<'_>,
) -> Result<(), Failure> {
    let UnitOnDisk { path, plan } = *written;
    let dir = unit::unit_dir(&ctx.paths);
    let before = unit::Enablement::read_for(&dir, names);
    let mut enable = vec!["--user", "enable", "--now"];
    enable.extend_from_slice(names);
    for step in [vec!["--user", "daemon-reload"], enable] {
        if let Err(why) = systemctl_run(ctx, systemctl, &step).await {
            // Zuerst den Dienst anhalten, aber nur, wenn dieser Lauf ihn
            // gestartet haben kann.
            //
            // `enable --now` startet die Unit. Scheitert sie dabei -- der
            // Daemon bricht ab, die Unit steht auf `failed` --, dann bleibt sie
            // in systemds Gedächtnis stehen, und `Restart=on-failure` versucht
            // es weiter. Eine Rücknahme, die nur die Datei wegnimmt, lässt
            // einen Neustartversuch auf eine Unit los, die es nicht mehr gibt.
            //
            // **Nur bei diesem Schritt.** Scheitert schon `daemon-reload`, hat
            // dieser Lauf nichts gestartet, und ein `stop` träfe den Daemon,
            // den der Mensch vorher selbst laufen hatte. Etwas anzuhalten, das
            // man nicht gestartet hat, ist schlimmer als ein Rest im
            // Gedächtnis von systemd.
            //
            // Beide Aufrufe ohne Prüfung des Ergebnisses: Sie räumen auf, und
            // ein Fehler dabei ändert nichts an dem Befund, der gleich
            // zurückgeht.
            if step.contains(&"enable") {
                for verb in ["stop", "reset-failed"] {
                    let mut call = vec!["--user", verb];
                    call.extend_from_slice(names);
                    let _ = systemctl_run(ctx, systemctl, &call).await;
                }
            }
            // Reihenfolge: erst die Verweise, dann die Unit. Andersherum stünde
            // zwischendurch ein Verweis auf eine Datei, die es nicht mehr gibt.
            let disabled = before.rollback(&dir);
            let taken_back = unit::rollback(path, plan);
            // Der zweite `daemon-reload` gehört zur Rücknahme: Nach dem ersten
            // kennt systemd die Unit, und eine, die es kennt und die nicht
            // mehr auf der Platte liegt, ist ein halber Zustand.
            let _ = systemctl_run(ctx, systemctl, &["--user", "daemon-reload"]).await;
            return Err(Failure::new(not_taken(
                path,
                &step,
                &why,
                plan,
                &taken_back,
                &disabled,
            )));
        }
    }
    Ok(())
}

/// Ein `systemctl`-Aufruf mit Frist und ohne die Umgebung des Nutzers.
///
/// `Ok(())` bei Exit 0, sonst ein Satz, der sagt, woran es lag.
async fn systemctl_run(ctx: &Context, systemctl: &Path, args: &[&str]) -> Result<(), String> {
    let mut command = tokio::process::Command::new(systemctl);
    command.args(args).env_clear().kill_on_drop(true);
    for key in SESSION_ENV_KEYS {
        if let Some(value) = ctx.env.non_empty(key) {
            command.env(key, value);
        }
    }
    let child = command.stdin(std::process::Stdio::null()).output();
    match tokio::time::timeout(SYSTEMCTL_TIMEOUT, child).await {
        Err(_elapsed) => Err(format!(
            "no answer within {} ms",
            SYSTEMCTL_TIMEOUT.as_millis()
        )),
        Ok(Err(error)) => Err(error.to_string()),
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => {
            let text = String::from_utf8_lossy(&output.stderr);
            let first = text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("-");
            Err(format!(
                "{}: {first}",
                output.status.code().map_or_else(
                    || "killed by a signal".to_owned(),
                    |code| format!("exit {code}")
                )
            ))
        }
    }
}

/// `DAEMON_008`: systemd hat die Unit nicht angenommen.
fn not_taken(
    path: &Path,
    step: &[&str],
    why: &str,
    plan: &unit::Written,
    taken_back: &Result<(), Diagnostic>,
    disabled: &Result<(), Diagnostic>,
) -> Diagnostic {
    let undone = match (plan, taken_back) {
        (unit::Written::Unchanged, _) => "nothing had been written".to_owned(),
        (_, Ok(())) => format!("{} is back the way it was", path.display()),
        (_, Err(diagnostic)) => format!(
            "and taking {} back again failed as well: {}",
            path.display(),
            diagnostic.why
        ),
    };
    // Eine gelungene Rücknahme der Aktivierung steht nicht im Text: Sie ist der
    // Normalfall und sagt einem Menschen nichts, was er tun müsste. Eine
    // misslungene steht darin, weil dann etwas liegen bleibt.
    let enablement = match disabled {
        Ok(()) => String::new(),
        Err(diagnostic) => format!("; {}", diagnostic.why),
    };
    if no_bus(why) {
        return no_user_session(&format!(
            "systemctl {} found no user session bus ({why}); {undone}{enablement}",
            step.join(" ")
        ));
    }
    Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
        .why(format!(
            "systemctl {} did not go through ({why}); {undone}{enablement}",
            step.join(" ")
        ))
        .fix(unit_fix(&[
            "systemctl",
            "--user",
            "status",
            unit::UNIT_NAME,
        ]))
        .build()
}

/// Sagt, was gleich geschieht — vor dem ersten Schreibzugriff.
///
/// **Geht mit Absicht nicht durch [`crate::render::Renderer`].** Dessen `note`
/// schweigt unter `--json` und unter `-q`, und ein Ausgabeschalter darf nicht
/// bestimmen, ob ein Mensch sieht, welche Datei sein Rechner gleich bekommt.
/// Dieselbe Regel wie bei der Ankündigung des Doctors (`cmd/doctor.rs`).
/// `stdout` bleibt unberührt, ein einziger JSON-Wert also weiterhin ein
/// einziger.
fn announce(
    headline: &str,
    contents: &str,
    args: &InstallArgs,
    systemctl: Option<&Path>,
    names: &[&str],
) {
    eprintln!("{headline}");
    eprintln!();
    for line in contents.lines() {
        eprintln!("  {line}");
    }
    eprintln!();
    match (args.print, args.no_start, systemctl) {
        (true, _, _) => eprintln!("--print: nothing is written and nothing is started"),
        (false, true, _) => eprintln!("--no-start: systemd is not told about the unit"),
        (false, false, None) => {
            eprintln!("no systemctl in PATH: nothing is started");
        }
        (false, false, Some(systemctl)) => {
            let shown = systemctl.display();
            eprintln!("then: {shown} --user daemon-reload");
            eprintln!(
                "then: {shown} --user enable --now {} (never with sudo)",
                names.join(" ")
            );
        }
    }
}

/// Die erste Zeile der Ankündigung: was mit dieser Datei geschieht.
///
/// `None` heißt `--print`. Sonst steht hier der Plan aus [`unit::prepare`], und
/// er steht so, wie er ausgeht: Eine Datei, die schon genau diesen Text trägt,
/// wird nicht geschrieben, und der Satz sagt das, statt es zu behaupten.
fn headline(path: &Path, plan: Option<&unit::Written>) -> String {
    let shown = path.display();
    match plan {
        None => format!("humanitl daemon install would write {shown}:"),
        Some(unit::Written::Created) => format!("humanitl daemon install writes {shown}:"),
        Some(unit::Written::Replaced { .. }) => {
            format!("humanitl daemon install replaces its own older {shown} with:")
        }
        Some(unit::Written::Unchanged) => {
            format!("humanitl daemon install writes nothing: {shown} already carries exactly this:")
        }
    }
}

/// Was `daemon install` am Ende ausgibt.
#[derive(Debug)]
struct InstallReport<'a> {
    /// Der Pfad der Unit.
    unit: &'a Path,
    /// Was in `ExecStart` steht.
    exec_start: &'a Path,
    /// Der ganze Text der Unit.
    unit_text: &'a str,
    /// Die `systemctl`-Aufrufe, die folgen.
    commands: &'a [String],
    /// Die Units, die `enable --now` bekommt.
    enable: &'a [&'a str],
    /// `created`, `replaced`, `unchanged`, `packaged` oder `print`.
    action: &'a str,
    /// Was aus der Aktivierung wurde.
    activation: Activation,
    /// Das Verzeichnis mit den Kopien aus einem `AppImage`.
    binaries: Option<&'a Path>,
    /// Die Fassung, die der Daemon nennt, oder warum er nicht antwortete.
    ready: Option<&'a str>,
}

/// Das Ergebnis: JSON oder Tabelle.
///
/// Unter `--json` trägt das Objekt auch den Text der Unit und die
/// `systemctl`-Aufrufe: Die Ankündigung auf `stderr` entfällt dort, und was
/// sie sagte, darf dadurch nicht verloren gehen.
fn report(ctx: &Context, result: &InstallReport<'_>) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "unit": result.unit.display().to_string(),
            "exec_start": result.exec_start.display().to_string(),
            "unit_text": result.unit_text,
            "commands": result.commands,
            "units": result.enable,
            "action": result.action,
            "activation": result.activation.as_str(),
            "binaries": result.binaries.map(|dir| dir.display().to_string()),
            "daemon": result.ready,
        }));
        return;
    }
    let mut rows = vec![
        vec!["unit".to_owned(), result.unit.display().to_string()],
        vec![
            "exec_start".to_owned(),
            result.exec_start.display().to_string(),
        ],
        vec!["action".to_owned(), result.action.to_owned()],
        vec![
            "activation".to_owned(),
            result.activation.as_str().to_owned(),
        ],
    ];
    if let Some(dir) = result.binaries {
        rows.push(vec![
            "binaries".to_owned(),
            format!("{} {}", tick(true), dir.display()),
        ]);
    }
    if let Some(ready) = result.ready {
        let answered = !ready.starts_with("no answer");
        rows.push(vec![
            "daemon".to_owned(),
            format!("{} {ready}", tick(answered)),
        ]);
    }
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    // Nur, wenn die Datei wirklich liegt und niemand sie gestartet hat. Unter
    // `--print` steht nichts da, und ein Hinweis, der das Gegenteil behauptet,
    // wäre die Sorte Satz, gegen die dieses Produkt gebaut ist.
    if result.action != PRINT_ACTION
        && (result.activation == Activation::NoSystemctl
            || result.activation == Activation::Skipped)
    {
        ctx.render.note(&format!(
            "the unit is in place; systemctl --user daemon-reload and systemctl --user enable \
             --now {} start it",
            result.enable.join(" ")
        ));
    }
}

/// Sucht ein Programm im `PATH` der übergebenen Umgebung, nie in der des
/// Prozesses.
fn find_in_path(ctx: &Context, program: &str) -> Option<PathBuf> {
    let path = ctx.env.non_empty("PATH")?;
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.is_file())
}

/// Ein Vorschlag, der beweisbar dieselben Wörter bleibt.
fn unit_fix(words: &[&str]) -> FixAction {
    humanitl_sandbox::doctor::command_fix(words)
}

/// `daemon status`.
async fn status(ctx: &Context) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();

    check_proto(&info)?;

    let socket = ctx.paths.daemon_socket().display().to_string();
    let unit_state = is_active(ctx).await;
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "socket": socket,
            "unit": unit_state,
            "daemon_version": info.daemon_version,
            "proto_major": info.proto_major,
            "proto_minor": info.proto_minor,
            "capabilities": info.capabilities,
            "session_id": info.session_id,
        }));
        return Ok(EXIT_OK);
    }

    let session = if info.session_id.is_empty() {
        "-".to_owned()
    } else {
        info.session_id.clone()
    };
    let rows = vec![
        vec!["socket".to_owned(), socket],
        vec![
            "unit".to_owned(),
            unit_state.unwrap_or_else(|| "-".to_owned()),
        ],
        vec!["daemon".to_owned(), info.daemon_version.clone()],
        vec![
            "proto".to_owned(),
            format!("{}.{}", info.proto_major, info.proto_minor),
        ],
        vec!["session".to_owned(), session],
        vec![
            "capabilities".to_owned(),
            if info.capabilities.is_empty() {
                "-".to_owned()
            } else {
                info.capabilities.join(", ")
            },
        ],
    ];
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    Ok(EXIT_OK)
}

/// Was `systemctl --user is-active humanitld.service` sagt.
///
/// `None`, wenn es kein `systemctl` gibt: Ein Daemon, der von Hand gestartet
/// wurde, ist kein Fehler, und ein erfundenes `inactive` wäre eine falsche
/// Auskunft über eine Unit, die niemand installiert hat.
async fn is_active(ctx: &Context) -> Option<String> {
    let systemctl = find_in_path(ctx, "systemctl")?;
    let mut command = tokio::process::Command::new(systemctl);
    command
        .args(["--user", "is-active", unit::UNIT_NAME])
        .env_clear()
        .kill_on_drop(true);
    for key in SESSION_ENV_KEYS {
        if let Some(value) = ctx.env.non_empty(key) {
            command.env(key, value);
        }
    }
    let output = tokio::time::timeout(SYSTEMCTL_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(first.to_owned())
}

/// Prüft, ob Client und Daemon dieselbe Major-Version sprechen.
///
/// Eine andere Major heißt: Nachrichten, die der eine schickt, versteht der
/// andere nicht mehr. Eine kleinere Minor beim Daemon ist dagegen kein
/// Fehler, nur eine Notiz: additive Änderungen bleiben lesbar.
pub fn check_proto(info: &v1::Info) -> Result<(), Failure> {
    if info.proto_major == PROTO_MAJOR {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::DAEMON_002, Severity::Blocking)
            .why(format!(
                "the daemon speaks contract {}.{}, this humanitl speaks {PROTO_MAJOR}.{PROTO_MINOR}",
                info.proto_major, info.proto_minor
            ))
            .fix(FixAction::CopyCommand(
                "systemctl --user restart humanitld".to_owned(),
            ))
            .build(),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::MetadataExt as _;

    use super::Staged;

    /// Eine Kopie mit einem `current`, das auf sie zeigt, und einer älteren
    /// daneben.
    fn two_copies() -> (tempfile::TempDir, Staged) {
        let base = tempfile::tempdir().expect("a base");
        let old = base.path().join("0.0.0.old");
        let new = base.path().join("0.0.0.new");
        std::fs::create_dir(&old).expect("the old copy");
        std::fs::create_dir(&new).expect("the new copy");
        let link = base.path().join("current");
        std::os::unix::fs::symlink(&new, &link).expect("current");
        let owner = std::fs::metadata(base.path()).expect("the base").uid();
        let staged = Staged {
            dir: new,
            link,
            previous: Some(old),
            owner,
        };
        (base, staged)
    }

    /// Lässt sich `current` nicht zurückhängen, bleibt die neue Kopie liegen:
    /// `current` zeigt weiter auf vollständige Binaries statt ins Leere.
    #[test]
    fn a_failed_restore_keeps_the_copy_current_points_at() {
        let (base, staged) = two_copies();
        // Ein Verzeichnis unter dem Namen, den das Umhängen für seinen
        // Zwischenverweis braucht: Das Umhängen scheitert, das Löschen der
        // Kopie ginge.
        std::fs::create_dir(
            base.path()
                .join(format!("current.tmp-{}", std::process::id())),
        )
        .expect("the blocking directory");
        staged.restore();

        assert!(staged.dir.is_dir(), "the copy current points at stays");
        assert_eq!(
            std::fs::read_link(&staged.link).expect("current"),
            staged.dir
        );
    }

    /// Eine ältere Kopie eines anderen Kontos wird nicht entfernt.
    #[test]
    fn retire_previous_leaves_a_copy_of_another_owner() {
        let (_base, mut staged) = two_copies();
        let previous = staged.previous.clone().expect("a previous copy");
        staged.owner += 1;
        staged.retire_previous();
        assert!(previous.is_dir(), "another owner's copy stays");

        staged.owner -= 1;
        staged.retire_previous();
        assert!(!previous.exists(), "our own older copy goes");
    }

    use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, v1};

    use super::check_proto;
    use crate::cmd::EXIT_DAEMON;

    fn info(major: u32) -> v1::Info {
        v1::Info {
            daemon_version: "0.0.0".to_owned(),
            proto_major: major,
            proto_minor: PROTO_MINOR,
            capabilities: vec!["hold".to_owned()],
            session_id: String::new(),
        }
    }

    #[test]
    fn the_same_major_is_accepted() {
        assert!(check_proto(&info(PROTO_MAJOR)).is_ok());
    }

    #[test]
    fn another_major_is_daemon_002_with_exit_two() {
        let failure = check_proto(&info(PROTO_MAJOR + 1)).expect_err("a newer daemon is refused");
        assert_eq!(failure.diagnostic.code.as_str(), "DAEMON_002");
        assert_eq!(failure.exit, EXIT_DAEMON);
    }
}
