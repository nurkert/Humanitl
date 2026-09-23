//! Der Lebenslauf des Dienstes außerhalb des Pakets (HUM-077): ein `AppImage`
//! richtet ihn ein, ein neueres erneuert ihn beim Start, und
//! `daemon uninstall` nimmt alles wieder weg.
//!
//! Jeder Test legt sich den Baum eines `AppImage` an -- die Kommandozeile als
//! Kopie des gebauten Binaries, daneben ein Daemon und ein Shim, die nichts
//! tun -- und ein `systemctl`, das jeden Aufruf protokolliert. So misst er,
//! was der Befehl mit Dateien und mit systemd tut, ohne einen systemd zu
//! brauchen und ohne den des Menschen zu berühren.
//!
//! Eigene Datei und nicht `tests/cli.rs`: Die ist mit 5000 Zeilen groß genug,
//! und diese Tests teilen eigene Helfer, die dort niemand braucht.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{BIN, Harness, code, stderr, stdout};

/// Was `$APPIMAGE` in diesen Läufen nennt. Den Pfad gibt es nicht; die
/// Kommandozeile liest nur, dass die Variable gesetzt ist.
const APPIMAGE: &str = "/nonexistent/Humanitl-test-x86_64.AppImage";

/// Die Fassung dieses Builds, wie `stage` sie in den Namen der Kopie schreibt.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Der Name einer älteren Kopie, in der Form, die `stage` vergibt.
const OLD_COPY: &str = "0.0.0-old.1000-1";

/// Ein Muster, auf das kein Aufruf passt: Das `systemctl` scheitert dann nie.
const NEVER: &str = "__never__";

/// Legt den Baum eines `AppImage` an: `usr/lib/humanitl/bin/` mit
/// Kommandozeile, Daemon und Shim, `usr/lib/humanitl/humanitl` als Anwendung,
/// die sich in `app.log` einträgt, und `AppRun` aus dem Repository.
///
/// Liefert das Verzeichnis `bin/`.
fn image_tree(harness: &Harness) -> PathBuf {
    let root = harness.path("image");
    let bin = root.join("usr/lib/humanitl/bin");
    std::fs::create_dir_all(&bin).expect("the image tree");
    std::fs::copy(BIN, bin.join("humanitl")).expect("the command line is copied");
    set_executable(&bin.join("humanitl"));
    for name in ["humanitld", "humanitl-shim"] {
        write_script(&bin.join(name), "#!/bin/sh\nexit 0\n");
    }
    let log = harness.path("app.log");
    write_script(
        &root.join("usr/lib/humanitl/humanitl"),
        &format!("#!/bin/sh\necho \"app $*\" >>'{}'\n", log.display()),
    );
    let apprun = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packaging/appimage/AppRun");
    std::fs::copy(apprun, root.join("AppRun")).expect("AppRun is copied");
    set_executable(&root.join("AppRun"));
    bin
}

/// Schreibt ein Skript mit `0755`.
fn write_script(path: &Path, text: &str) {
    std::fs::write(path, text).expect("the script");
    set_executable(path);
}

/// `0755` auf `path`.
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("0755");
}

/// Ein `systemctl`, das jeden Aufruf in `systemctl.log` schreibt und bei einem
/// Aufruf, der auf `fail` passt, mit 1 endet.
///
/// Bei `restart` hält es zusätzlich fest, ob die ältere Kopie in diesem
/// Augenblick noch da ist: Genau daran hängt die Regel, dass sie erst nach dem
/// Neustart gehen darf (HUM-077, Fallstricke).
///
/// Liefert das Verzeichnis, das in `PATH` gehört, und das Protokoll.
fn fake_systemctl(harness: &Harness, fail: &str) -> (PathBuf, PathBuf) {
    let dir = harness.path("fakebin");
    std::fs::create_dir_all(&dir).expect("the fake bin directory");
    let log = harness.path("systemctl.log");
    let old = lib_dir(harness).join(OLD_COPY);
    write_script(
        &dir.join("systemctl"),
        &format!(
            "#!/bin/sh\n\
             PATH=/usr/bin:/bin\n\
             echo \"$*\" >>'{log}'\n\
             case \"$*\" in\n\
             *restart*) test -d '{old}' && echo 'the old copy is still there' >>'{log}' ;;\n\
             esac\n\
             case \"$*\" in\n\
             {fail}) echo 'Job for humanitld.service failed' >&2; exit 1 ;;\n\
             esac\n\
             exit 0\n",
            log = log.display(),
            old = old.display(),
        ),
    );
    (dir, log)
}

/// Was das `systemctl` protokolliert hat.
fn calls(log: &Path) -> String {
    std::fs::read_to_string(log).unwrap_or_default()
}

/// `~/.local/lib/humanitl` dieser Umgebung.
fn lib_dir(harness: &Harness) -> PathBuf {
    harness.path("home").join(".local/lib/humanitl")
}

/// Die Unit, die `daemon install` schreibt.
fn unit_path(harness: &Harness) -> PathBuf {
    harness
        .path("config")
        .join("systemd/user/humanitld.service")
}

/// Ein `PATH` aus dem Verzeichnis des falschen `systemctl` und den
/// Verzeichnissen, die `AppRun` für `readlink` und `dirname` braucht.
fn path_with(fakebin: &Path) -> OsString {
    OsString::from(format!("{}:/usr/bin:/bin", fakebin.display()))
}

/// Startet `program` in der Umgebung, mit `PATH` und, wenn gewünscht,
/// `$APPIMAGE`.
///
/// Wartet ab, solange der Kernel das frisch kopierte Binary als beschäftigt
/// meldet (`ETXTBSY`): Ein anderer Test-Thread kann beim `fork` noch den
/// Schreib-Deskriptor seiner eigenen Kopie vererbt haben; die ausführliche
/// Begründung steht an `output_when_not_busy` in `tests/cli.rs`.
fn run(
    harness: &Harness,
    program: &Path,
    args: &[&str],
    path: &OsString,
    appimage: bool,
) -> Output {
    const ATTEMPTS: u32 = 50;
    const PAUSE: std::time::Duration = std::time::Duration::from_millis(20);
    let mut command: Command = harness.command_of(program.to_str().expect("a UTF-8 path"));
    command.args(args).env("PATH", path);
    if appimage {
        command.env("APPIMAGE", APPIMAGE);
    } else {
        command.env_remove("APPIMAGE");
    }
    for _ in 0..ATTEMPTS {
        match command.output() {
            Ok(output) => return output,
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(PAUSE);
            }
            Err(error) => panic!("the program runs: {error:?}"),
        }
    }
    panic!("the copied binary stayed busy for {:?}", PAUSE * ATTEMPTS)
}

/// Das eine JSON-Objekt auf `stdout`.
fn json(output: &Output) -> serde_json::Value {
    let text = stdout(output);
    serde_json::from_str(text.trim())
        .unwrap_or_else(|error| panic!("one JSON value on stdout ({error}): {text}"))
}

/// Richtet den Dienst aus dem `AppImage` ein und macht die Kopie zu einer
/// älteren Fassung: Das Verzeichnis heißt danach [`OLD_COPY`], und `current`
/// zeigt darauf. So sieht ein Rechner aus, auf dem ein früheres `AppImage`
/// den Dienst eingerichtet hat.
fn install_an_older_copy(harness: &Harness, bin: &Path, path: &OsString) -> PathBuf {
    let output = run(
        harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        path,
        true,
    );
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let link = lib_dir(harness).join("current");
    let copy = std::fs::read_link(&link).expect("current is a link");
    let old = lib_dir(harness).join(OLD_COPY);
    std::fs::rename(&copy, &old).expect("the copy becomes an older one");
    std::fs::remove_file(&link).expect("current goes");
    std::os::unix::fs::symlink(&old, &link).expect("current points at the older copy");
    old
}

/// Wohin `current` zeigt, als Name.
fn current_name(harness: &Harness) -> String {
    std::fs::read_link(lib_dir(harness).join("current"))
        .expect("current is a link")
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a name")
        .to_owned()
}

/// Ohne eine Einrichtung aus einem `AppImage` tut `--refresh` nichts: Die
/// erste Einrichtung bleibt der Klick in der Anwendung.
#[test]
fn refresh_without_an_install_writes_nothing_and_calls_nothing() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--refresh"],
        &path_with(&fakebin),
        true,
    );

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(json(&output)["action"], "not_installed");
    assert!(!unit_path(&harness).exists(), "no unit was written");
    assert!(!lib_dir(&harness).exists(), "no copy was made");
    assert_eq!(calls(&log), "", "systemctl was called");
}

/// Dieselbe Fassung noch einmal gestartet: kein Aufruf, keine Kopie.
#[test]
fn refresh_of_the_same_version_calls_nothing() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let first = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        &path,
        true,
    );
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let before = calls(&log);
    let copy = current_name(&harness);

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--refresh"],
        &path,
        true,
    );

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json(&output);
    assert_eq!(value["action"], "up_to_date", "{value}");
    assert_eq!(value["installed"], VERSION, "{value}");
    assert_eq!(calls(&log), before, "systemctl was called again");
    assert_eq!(current_name(&harness), copy, "current was moved");
}

/// Akzeptanzkriterium 2, zweite Hälfte: Ein neueres `AppImage` erneuert beim
/// Start die Kopie und startet den Dienst neu, und die ältere Kopie geht erst
/// nach dem Neustart.
#[test]
fn refresh_replaces_an_older_copy_restarts_and_only_then_retires_it() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let old = install_an_older_copy(&harness, &bin, &path);
    let before = calls(&log).len();

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--refresh"],
        &path,
        true,
    );

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json(&output);
    assert_eq!(value["restarted"], true, "{value}");
    let name = current_name(&harness);
    assert!(
        name.starts_with(&format!("{VERSION}.")),
        "current points at {name}, not at a copy of {VERSION}"
    );
    assert!(
        !old.exists(),
        "the older copy is still there after the restart"
    );

    let now = calls(&log);
    let since: Vec<&str> = now[before..].lines().collect();
    let enable = since
        .iter()
        .position(|line| line.starts_with("--user enable --now humanitld.service"))
        .unwrap_or_else(|| panic!("no enable: {since:?}"));
    let restart = since
        .iter()
        .position(|line| *line == "--user restart humanitld.service")
        .unwrap_or_else(|| panic!("no restart: {since:?}"));
    assert!(enable < restart, "restart before enable: {since:?}");
    assert_eq!(
        since.get(restart + 1).copied(),
        Some("the old copy is still there"),
        "the older copy was gone before the restart: {since:?}"
    );
}

/// Scheitert der Neustart, zeigt `current` wieder auf die ältere Kopie, und
/// die neue ist weg: Die alte Fassung läuft weiter, statt dass der Dienst ins
/// Leere startet.
#[test]
fn a_failed_restart_puts_the_older_copy_back() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, "*restart*");
    let path = path_with(&fakebin);
    let old = install_an_older_copy(&harness, &bin, &path);

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--refresh"],
        &path,
        true,
    );

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_008", "{value}");
    assert!(
        value["why"]
            .as_str()
            .is_some_and(|why| why.contains("restart humanitld.service")),
        "{value}"
    );
    // Dieses `systemctl` lässt auch den zweiten Neustart scheitern, und der
    // Satz sagt das, statt einen gelungenen zu behaupten (HUM-077, Review).
    let why = value["why"].as_str().unwrap_or_default();
    assert!(why.contains("failed as well"), "{why}");
    assert!(!why.contains("was restarted on them"), "{why}");
    assert_eq!(current_name(&harness), OLD_COPY);
    assert!(old.join("humanitld").is_file(), "the older copy is gone");
    let copies: Vec<String> = std::fs::read_dir(lib_dir(&harness))
        .expect("the lib directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        !copies
            .iter()
            .any(|name| name.starts_with(&format!("{VERSION}."))),
        "the new copy stayed: {copies:?} ({})",
        calls(&log)
    );
}

/// Ohne `systemctl` startet niemand den Dienst neu; dann läuft womöglich noch
/// der Daemon aus der älteren Kopie, und die bleibt liegen.
#[test]
fn an_install_without_systemctl_keeps_the_older_copy() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, _log) = fake_systemctl(&harness, NEVER);
    let old = install_an_older_copy(&harness, &bin, &path_with(&fakebin));

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        // Ein leerer `PATH`: Die Kommandozeile findet kein `systemctl` und
        // aktiviert nichts. Ein `PATH` mit `/usr/bin` brächte das `systemctl`
        // des Rechners ins Spiel.
        &OsString::new(),
        true,
    );

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json(&output);
    assert_eq!(value["activation"], "no systemctl", "{value}");
    assert_eq!(value["restarted"], false, "{value}");
    assert!(
        old.is_dir(),
        "the older copy went without a restart: {value}"
    );
    assert!(
        current_name(&harness).starts_with(&format!("{VERSION}.")),
        "current still points at the older copy"
    );
}

/// Akzeptanzkriterium 2 über `AppRun`: Das `AppImage` erneuert vor dem Start
/// der Anwendung, und die Anwendung bekommt ihre Argumente.
#[test]
fn the_appimage_entry_refreshes_before_it_starts_the_app() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let old = install_an_older_copy(&harness, &bin, &path);
    let apprun = harness.path("image/AppRun");

    let output = run(&harness, &apprun, &["--flag"], &path, true);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(
        std::fs::read_to_string(harness.path("app.log")).unwrap_or_default(),
        "app --flag\n",
        "the application did not start with its arguments"
    );
    assert!(
        current_name(&harness).starts_with(&format!("{VERSION}.")),
        "AppRun did not refresh the copy"
    );
    assert!(!old.exists(), "the older copy stayed");
    assert!(
        calls(&log).contains("--user restart humanitld.service"),
        "{}",
        calls(&log)
    );
}

/// `--cli` erneuert nichts: Wer die Kommandozeile im Bild ruft, bekommt genau
/// den Aufruf, den er geschrieben hat.
#[test]
fn the_appimage_cli_does_not_refresh() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    install_an_older_copy(&harness, &bin, &path);
    let before = calls(&log);
    let apprun = harness.path("image/AppRun");

    let output = run(&harness, &apprun, &["--cli", "--version"], &path, true);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), format!("humanitl {VERSION}"));
    assert_eq!(current_name(&harness), OLD_COPY);
    assert_eq!(calls(&log), before);
}

/// Akzeptanzkriterium 4: `daemon uninstall --purge-binaries` meldet den
/// Dienst ab und entfernt Unit, Verweis der Aktivierung, Socket und Kopien;
/// danach zeigt der Doctor `DOCTOR_006` mit dem Fix, der ihn wieder einrichtet.
#[test]
fn uninstall_removes_unit_links_socket_and_binaries_and_doctor_says_daemon_006() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let install = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        &path,
        true,
    );
    assert_eq!(code(&install), 0, "{}", stderr(&install));
    let unit = unit_path(&harness);
    assert!(unit.is_file());
    // Das falsche `systemctl` legt keine Verweise an; der Test legt den an,
    // den `enable` angelegt hätte, und einen liegengebliebenen Socket.
    let wants = unit.with_file_name("default.target.wants");
    std::fs::create_dir_all(&wants).expect("the wants directory");
    std::os::unix::fs::symlink(&unit, wants.join("humanitld.service")).expect("the wants link");
    let socket = harness.paths().daemon_socket();
    std::fs::create_dir_all(socket.parent().expect("a directory")).expect("the socket directory");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("the socket binds"));
    assert!(socket.exists(), "the socket file stays after its listener");
    let before = calls(&log).len();

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "uninstall", "--purge-binaries"],
        &path,
        false,
    );

    assert_eq!(code(&output), 0, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["activation"], "disabled", "{value}");
    let since = calls(&log)[before..].to_owned();
    let lines: Vec<&str> = since.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("--user disable --now humanitld.service"),
        "{since}"
    );
    assert!(lines.contains(&"--user daemon-reload"), "{since}");
    assert!(!unit.exists(), "the unit stayed");
    assert!(
        std::fs::symlink_metadata(wants.join("humanitld.service")).is_err(),
        "the wants link stayed"
    );
    assert!(!socket.exists(), "the stale socket stayed");
    assert!(!lib_dir(&harness).exists(), "the copies stayed");

    let doctor = harness.run(["doctor", "--json"]);
    let report: serde_json::Value = serde_json::from_str(stdout(&doctor).trim())
        .unwrap_or_else(|error| panic!("one JSON value ({error}): {}", stdout(&doctor)));
    let line = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["id"] == "daemon")
        .expect("the daemon line")
        .clone();
    assert_eq!(line["diagnostic"]["code"], "DOCTOR_006", "{line}");
    assert_eq!(
        line["diagnostic"]["fix"]["kind"], "install_service",
        "{line}"
    );
}

/// Eine Unit ohne die Marke hat Humanitl nicht geschrieben; `uninstall`
/// nimmt sie nicht weg und meldet auch nichts ab.
#[test]
fn uninstall_leaves_a_unit_it_did_not_write_alone() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let unit = unit_path(&harness);
    std::fs::create_dir_all(unit.parent().expect("a directory")).expect("the unit directory");
    std::fs::write(&unit, "[Service]\nExecStart=/opt/mine\n").expect("a foreign unit");

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "uninstall"],
        &path_with(&fakebin),
        false,
    );

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    assert_eq!(json(&output)["code"], "DAEMON_005");
    assert!(unit.is_file(), "the foreign unit is gone");
    assert_eq!(calls(&log), "", "systemctl was called");
}

/// Meldet systemd den Dienst nicht ab, ist nichts entfernt, und der Befund
/// nennt genau den Aufruf, der scheiterte.
#[test]
fn uninstall_that_systemd_refuses_removes_nothing() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, _) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let install = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        &path,
        true,
    );
    assert_eq!(code(&install), 0, "{}", stderr(&install));
    let (fakebin, _) = fake_systemctl(&harness, "*disable*");

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "uninstall", "--purge-binaries"],
        &path_with(&fakebin),
        false,
    );

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_014", "{value}");
    assert_eq!(
        value["fix"]["command"], "systemctl --user disable --now humanitld.service",
        "{value}"
    );
    assert!(unit_path(&harness).is_file(), "the unit went");
    assert!(lib_dir(&harness).join("current").exists(), "the copy went");
}

/// Antwortet nach dem Abmelden noch ein Daemon, entfernt
/// `--purge-binaries` nichts: Er läuft womöglich aus genau diesen Kopien
/// (HUM-077, Review).
#[test]
fn purge_refuses_while_a_daemon_still_answers() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, _log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let install = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        &path,
        true,
    );
    assert_eq!(code(&install), 0, "{}", stderr(&install));
    let current = std::fs::read_link(lib_dir(&harness).join("current")).expect("current");
    // Ein Daemon, der von Hand läuft: Der Socket nimmt Verbindungen an,
    // solange der Listener lebt.
    let socket = harness.paths().daemon_socket();
    std::fs::create_dir_all(socket.parent().expect("a directory")).expect("the socket directory");
    let listener = std::os::unix::net::UnixListener::bind(&socket).expect("the socket binds");

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "uninstall", "--purge-binaries"],
        &path,
        false,
    );
    drop(listener);

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_014", "{value}");
    assert_eq!(value["fix"]["command"], "pkill -x humanitld", "{value}");
    assert!(unit_path(&harness).is_file(), "the unit went");
    assert!(current.join("humanitld").is_file(), "the copy went");
    assert!(socket.exists(), "the live socket went");
}

/// Startet ein Daemon erst nach der ersten Prüfung, antwortet er am Socket,
/// bevor die Kopien gehen. Dann bleiben sie liegen, und der Befund nennt sie
/// (HUM-077, Review). Das `systemctl` hier bindet den Socket beim
/// `reset-failed`, also zwischen beiden Prüfungen.
#[test]
fn purge_keeps_the_copies_of_a_daemon_that_started_meanwhile() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, _log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    let install = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        &path,
        true,
    );
    assert_eq!(code(&install), 0, "{}", stderr(&install));
    let current = std::fs::read_link(lib_dir(&harness).join("current")).expect("current");
    let socket = harness.paths().daemon_socket();
    std::fs::create_dir_all(socket.parent().expect("a directory")).expect("the socket directory");
    let pid_file = harness.path("listener.pid");
    // Der Listener bekommt `/dev/null` als Ein- und Fehlerausgabe: Erbte er
    // die Rohre dieses `systemctl`, kehrte `reset-failed` erst nach der Frist
    // von `systemctl_run` zurück (20 s). Das Warten auf ihn ist begrenzt
    // (100 mal 50 ms) und endet sonst mit einem Fehler.
    write_script(
        &fakebin.join("systemctl"),
        &format!(
            "#!/bin/sh\n\
             PATH=/usr/bin:/bin\n\
             case \"$*\" in\n\
             *reset-failed*)\n\
             python3 -c 'import socket,sys,time\n\
             s=socket.socket(socket.AF_UNIX)\n\
             s.bind(sys.argv[1]); s.listen()\n\
             print(flush=True)\n\
             time.sleep(30)' '{socket}' >'{pid_file}.ready' </dev/null 2>/dev/null &\n\
             echo $! >'{pid_file}'\n\
             n=0\n\
             while ! test -s '{pid_file}.ready'; do\n\
               n=$((n + 1)); test \"$n\" -le 100 || exit 97; sleep 0.05\n\
             done ;;\n\
             esac\n\
             exit 0\n",
            socket = socket.display(),
            pid_file = pid_file.display(),
        ),
    );

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "uninstall", "--purge-binaries"],
        &path,
        false,
    );
    if let Ok(pid) = std::fs::read_to_string(&pid_file) {
        let _ = std::process::Command::new("kill").arg(pid.trim()).status();
    }

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_014", "{value}");
    assert!(
        value["why"]
            .as_str()
            .is_some_and(|why| why.contains("answers on")),
        "{value}"
    );
    assert!(current.join("humanitld").is_file(), "the copy went");
    assert!(socket.exists(), "the live socket went");
}

/// Scheitert nach dem gescheiterten Neustart auch `daemon-reload`, hält
/// systemd noch die gescheiterte Unit. Dann startet der Befehl den Dienst
/// kein zweites Mal und sagt, warum (HUM-077, Review).
#[test]
fn a_failed_reload_after_a_failed_restart_restarts_nothing() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, log) = fake_systemctl(&harness, NEVER);
    let path = path_with(&fakebin);
    install_an_older_copy(&harness, &bin, &path);
    let before = calls(&log).lines().count();
    // Ab jetzt scheitert jeder Neustart und jedes `daemon-reload`, das nach
    // einem Neustart kommt; `.since` hält fest, was seit hier lief.
    std::fs::write(format!("{}.since", log.display()), "").expect("the mark");
    write_script(
        &fakebin.join("systemctl"),
        &format!(
            "#!/bin/sh\n\
             PATH=/usr/bin:/bin\n\
             echo \"$*\" >>'{log}'\n\
             case \"$*\" in\n\
             *daemon-reload*) grep -q restart '{log}.since' && exit 1 ;;\n\
             esac\n\
             echo \"$*\" >>'{log}.since'\n\
             case \"$*\" in\n\
             *restart*) exit 1 ;;\n\
             esac\n\
             exit 0\n",
            log = log.display(),
        ),
    );

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--refresh"],
        &path,
        true,
    );

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_008", "{value}");
    let why = value["why"].as_str().unwrap_or_default();
    assert!(why.contains("daemon-reload failed"), "{why}");
    let since: Vec<String> = calls(&log)
        .lines()
        .skip(before)
        .map(str::to_owned)
        .collect();
    let restarts = since.iter().filter(|call| call.contains("restart")).count();
    assert_eq!(restarts, 1, "restarted again: {since:?}");
}

/// Scheitert nach dem Neustart auch das Zurückhängen von `current`, startet
/// der Befehl den Dienst nicht noch einmal: Er startete sonst die neue Kopie,
/// die eben gescheitert ist. Der Befund sagt, dass nichts zurückging
/// (HUM-077, Review).
#[test]
fn a_restore_that_fails_restarts_nothing_and_says_so() {
    use std::os::unix::fs::PermissionsExt as _;

    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, _) = fake_systemctl(&harness, NEVER);
    install_an_older_copy(&harness, &bin, &path_with(&fakebin));
    // Dieses `systemctl` lässt den Neustart scheitern und macht
    // `~/.local/lib/humanitl` schreibgeschützt: `current` lässt sich danach
    // nicht mehr umhängen.
    let log = harness.path("systemctl-ro.log");
    let dir = harness.path("fakebin-ro");
    std::fs::create_dir_all(&dir).expect("the fake bin directory");
    write_script(
        &dir.join("systemctl"),
        &format!(
            "#!/bin/sh\n\
             PATH=/usr/bin:/bin\n\
             echo \"$*\" >>'{log}'\n\
             case \"$*\" in\n\
             *restart*) chmod 0555 '{lib}'; echo 'Job failed' >&2; exit 1 ;;\n\
             esac\n\
             exit 0\n",
            log = log.display(),
            lib = lib_dir(&harness).display(),
        ),
    );

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--refresh"],
        &path_with(&dir),
        true,
    );
    std::fs::set_permissions(lib_dir(&harness), std::fs::Permissions::from_mode(0o755))
        .expect("the lib directory is writable again");

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_008", "{value}");
    let why = value["why"].as_str().unwrap_or_default();
    assert!(why.contains("was not restarted again"), "{why}");
    assert!(!why.contains("are back in place"), "{why}");
    let restarts = calls(&log)
        .lines()
        .filter(|line| *line == "--user restart humanitld.service")
        .count();
    assert_eq!(restarts, 1, "{}", calls(&log));
}

/// Hat der Lauf die Unit erst angelegt, und scheitert der Neustart auf eine
/// neue Kopie, geht die Unit samt dem Verweis der Aktivierung, den `enable`
/// angelegt hat, und es gibt keinen zweiten Neustart: Einen alten Dienst, den
/// er zurückbrächte, gibt es nicht (HUM-077, Review).
#[test]
fn a_failed_restart_of_a_created_unit_leaves_no_link_and_restarts_nothing() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let (fakebin, _) = fake_systemctl(&harness, NEVER);
    install_an_older_copy(&harness, &bin, &path_with(&fakebin));
    // Die Unit ist weg, die Kopien sind noch da: So sieht ein Rechner aus,
    // auf dem jemand die Unit von Hand gelöscht hat.
    let unit = unit_path(&harness);
    std::fs::remove_file(&unit).expect("the unit goes");
    let wants = unit.with_file_name("default.target.wants");
    let link = wants.join("humanitld.service");
    let log = harness.path("systemctl-created.log");
    let dir = harness.path("fakebin-created");
    std::fs::create_dir_all(&dir).expect("the fake bin directory");
    write_script(
        &dir.join("systemctl"),
        &format!(
            "#!/bin/sh\n\
             PATH=/usr/bin:/bin\n\
             echo \"$*\" >>'{log}'\n\
             case \"$*\" in\n\
             *enable*) mkdir -p '{wants}' && ln -sf '{unit}' '{link}' ;;\n\
             *restart*) echo 'Job failed' >&2; exit 1 ;;\n\
             esac\n\
             exit 0\n",
            log = log.display(),
            wants = wants.display(),
            unit = unit.display(),
            link = link.display(),
        ),
    );

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install"],
        &path_with(&dir),
        true,
    );

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_008", "{value}");
    let why = value["why"].as_str().unwrap_or_default();
    assert!(why.contains("stopped rather than restarted"), "{why}");
    let calls = calls(&log);
    assert!(
        calls.contains("--user enable --now humanitld.service"),
        "{calls}"
    );
    assert_eq!(
        calls
            .lines()
            .filter(|line| *line == "--user restart humanitld.service")
            .count(),
        1,
        "{calls}"
    );
    assert!(!unit.exists(), "the created unit stayed");
    assert!(
        std::fs::symlink_metadata(&link).is_err(),
        "a dead enablement link stayed"
    );
    assert_eq!(current_name(&harness), OLD_COPY);
}

/// Ist `~/.local/lib/humanitl` ein Verweis, entfernt `--purge-binaries`
/// nichts darin, und der Vorschlag bleibt ein ausführbares `ls -ld` auf genau
/// dieses Verzeichnis (HUM-077, Review).
#[test]
fn a_linked_lib_directory_is_left_with_a_runnable_fix() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let elsewhere = harness.path("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("the target");
    let base = lib_dir(&harness);
    std::fs::create_dir_all(base.parent().expect("a parent")).expect("~/.local/lib");
    std::os::unix::fs::symlink(&elsewhere, &base).expect("the lib directory is a link");

    let output = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "uninstall", "--purge-binaries"],
        &OsString::new(),
        false,
    );

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let value = json(&output);
    assert_eq!(value["code"], "DAEMON_014", "{value}");
    assert_eq!(
        value["fix"]["command"],
        format!("ls -ld {}", base.display()),
        "{value}"
    );
}

/// Liegen die Units des Pakets in dem Verzeichnis, das
/// `HUMANITL_SYSTEM_UNIT_DIR` nennt, nimmt `daemon install` den Weg des
/// Pakets; ohne sie den eigenen. So hängen die Tests nicht daran, ob auf dem
/// Rechner das Paket installiert ist (HUM-077, Review).
#[test]
fn the_package_unit_directory_comes_from_the_environment() {
    let harness = Harness::new();
    let bin = image_tree(&harness);
    let own = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--print"],
        &OsString::new(),
        false,
    );
    assert_eq!(code(&own), 0, "{}", stderr(&own));
    assert_eq!(
        json(&own)["unit"],
        unit_path(&harness).display().to_string(),
        "an empty package directory means the own way"
    );

    let service = harness.path("system-units").join("humanitld.service");
    std::fs::write(&service, "[Service]\nExecStart=/opt/pkg/humanitld\n").expect("a package unit");
    let packaged = run(
        &harness,
        &bin.join("humanitl"),
        &["--json", "daemon", "install", "--print"],
        &OsString::new(),
        false,
    );
    assert_eq!(code(&packaged), 0, "{}", stderr(&packaged));
    let value = json(&packaged);
    assert_eq!(value["unit"], service.display().to_string(), "{value}");
    assert_eq!(value["exec_start"], "/opt/pkg/humanitld", "{value}");
}
