//! Tests des Verzeichnisses je Instanz (HUM-492), alle unter einem
//! Laufzeitverzeichnis in `tempfile`, nie unter `/run/user/<uid>`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::{BufRead as _, Write as _};
use std::os::fd::{AsFd as _, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use rustix::fs::{FlockOperation, Mode, OFlags};

use super::{
    Kept, LOCK_FILE, MAX_DEPTH, enter, find_orphans, is_instance_name, open_dir_here, release_all,
    session_dir_removed, session_unit,
};
use crate::user_unit::UnitName;

const LIVE: &str = "humanitl-vm-0123456789abcdef";
const DEAD: &str = "humanitl-vm-fedcba9876543210";

/// Die Umgebung, mit der [`child_daemon`] als zweiter Prozess läuft.
const CHILD_ENV: &str = "HUMANITL_TEST_HUM492_INSTANCE";
/// Die Sitzung in [`dead_instance`].
const SESSION: &str = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";

thread_local! {
    /// Was `orphan` zwischen dem Öffnen einer Instanz und dem Vergleich mit
    /// ihrem Namen tut: nur in den Tests, je Faden.
    static BEFORE_COMPARE: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

/// Die Naht in `orphan` ([`BEFORE_COMPARE`]).
pub(super) fn before_compare() {
    if let Some(hook) = BEFORE_COMPARE.with_borrow_mut(Option::take) {
        hook();
    }
}

/// Die Unit einer Sitzung, wie der Daemon sie ableitet.
fn derived(session: &OsStr) -> Option<UnitName> {
    session_unit(|id| UnitName::new(&format!("humanitl-{id}")), session)
}

fn runtime() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Belegt `instance` für die Sitzung `session` wie ein Start.
fn start(instance: &Path, session: &str) -> PathBuf {
    let dir = instance.join(session);
    enter(
        instance,
        OsStr::new(session),
        || std::fs::create_dir(&dir).map_err(|err| err.to_string()),
        |why| why,
    )
    .unwrap();
    std::fs::write(dir.join("unit"), "humanitl-vm-0000\n").unwrap();
    dir
}

/// Beendet eine Sitzung wie das Ende der Sandbox ([`crate::handle`]).
fn end(dir: &Path) {
    std::fs::remove_dir_all(dir).unwrap();
    session_dir_removed(dir);
}

/// Eine Instanz, wie ein beendeter Daemon sie liegen ließ: Sperrdatei, eine
/// Sitzung mit Unit, Sockets und Unterverzeichnissen.
fn dead_instance(runtime: &Path, name: &str) -> PathBuf {
    let instance = runtime.join(name);
    std::fs::create_dir(&instance).unwrap();
    std::fs::write(instance.join(LOCK_FILE), "").unwrap();
    let session = instance.join(SESSION);
    std::fs::create_dir_all(session.join("files/deep/deeper")).unwrap();
    std::fs::write(session.join("files/deep/deeper/plan.json"), "{}").unwrap();
    // Die Datei nennt eine fremde, womöglich laufende Unit; sie zählt nie.
    std::fs::write(session.join("unit"), "humanitl-vm-live\n").unwrap();
    // Ein Sonderdatei-Eintrag wie die Sockets der Sitzung.
    rustix::fs::mknodat(
        rustix::fs::CWD,
        session.join("door_5000"),
        rustix::fs::FileType::Fifo,
        Mode::from_raw_mode(0o600),
        0,
    )
    .unwrap();
    instance
}

/// Hält die Sperre von `instance` wie ein anderer lebender Prozess: eigene
/// Beschreibung der offenen Datei, geteilt gesperrt.
fn hold_shared(instance: &Path) -> OwnedFd {
    let lock = rustix::fs::open(
        instance.join(LOCK_FILE),
        OFlags::RDWR | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    rustix::fs::flock(&lock, FlockOperation::NonBlockingLockShared).unwrap();
    lock
}

fn removed_names(runtime: &Path, own: Option<&str>) -> Vec<String> {
    let orphans = find_orphans(runtime, own.map(OsStr::new));
    let names: Vec<String> = orphans
        .found
        .iter()
        .map(|orphan| orphan.name.to_string_lossy().into_owned())
        .collect();
    let _ = orphans.remove(|_| None, |_| {});
    names
}

#[test]
fn only_the_exact_scheme_is_an_instance() {
    assert!(is_instance_name(LIVE.as_bytes()));
    for name in [
        "humanitl-vm-0123456789abcde",
        "humanitl-vm-0123456789abcdef0",
        "humanitl-vm-0123456789ABCDEF",
        "humanitl-vm-0123456789abcdeg",
        "humanitl-vm-",
        "humanitl-0123456789abcdef",
        "xhumanitl-vm-0123456789abcdef",
        "humanitl",
    ] {
        assert!(!is_instance_name(name.as_bytes()), "{name}");
    }
}

/// Mutationsbeweis: Ohne die exklusive Sperre in `orphan` (`flock` gestrichen)
/// räumt die Bestandsaufnahme die belegte Instanz, und der Test ist rot.
#[test]
fn a_live_instance_is_kept() {
    let rt = runtime();
    let instance = rt.path().join(LIVE);
    let session = start(&instance, "a1");

    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());
    assert_eq!(removed_names(rt.path(), Some(LIVE)), Vec::<String>::new());
    assert!(session.join("unit").exists());

    end(&session);
    assert!(
        std::fs::symlink_metadata(&instance).is_err(),
        "the last session takes the instance along"
    );
}

#[test]
fn a_dead_instance_is_removed_with_its_sessions() {
    let rt = runtime();
    let outside = rt.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), "keep").unwrap();
    let instance = dead_instance(rt.path(), DEAD);
    let session = instance.join(SESSION);
    // Eine zweite Sitzung, deren Name keine Sitzung ist: keine Unit.
    std::fs::create_dir(instance.join("not-a-session")).unwrap();
    std::fs::write(instance.join("not-a-session/unit"), "humanitl-vm-live\n").unwrap();
    // Verweise in einem verwaisten Verzeichnis gehen als Einträge, ihr Ziel
    // bleibt.
    std::os::unix::fs::symlink(&outside, session.join("to-outside")).unwrap();
    std::os::unix::fs::symlink(&outside, session.join("files/deep/link")).unwrap();

    let orphans = find_orphans(rt.path(), None);
    assert_eq!(orphans.found.len(), 1);
    assert_eq!(orphans.found[0].sessions.len(), 2);
    let stopped = RefCell::new(Vec::new());
    let mut removed = orphans.remove(derived, |unit| stopped.borrow_mut().push(unit.service()));
    removed.sort_by(|a, b| a.dir.cmp(&b.dir));

    // Gestoppt wird nur die Unit, die zum Namen der Sitzung gehört, nie die
    // aus der Datei.
    assert_eq!(
        *stopped.borrow(),
        [format!("humanitl-vm-{SESSION}.service")]
    );
    assert_eq!(removed.len(), 2);
    assert!(removed.iter().all(|r| r.result.is_ok()), "{removed:?}");
    assert_eq!(removed[0].dir, session);
    assert_eq!(removed[1].unit, None);
    assert!(
        std::fs::symlink_metadata(&instance).is_err(),
        "instance stays"
    );
    assert_eq!(
        std::fs::read_to_string(outside.join("keep")).unwrap(),
        "keep"
    );
}

#[test]
fn an_empty_dead_instance_goes_without_a_report() {
    let rt = runtime();
    let instance = rt.path().join(DEAD);
    std::fs::create_dir(&instance).unwrap();
    std::fs::write(instance.join(LOCK_FILE), "").unwrap();

    let orphans = find_orphans(rt.path(), None);
    assert_eq!(orphans.found.len(), 1);
    assert!(orphans.remove(|_| None, |_| {}).is_empty());
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

/// Zwei Riegel halten das: `O_NOFOLLOW` in `open_dir` und der Vergleich des
/// geöffneten Verzeichnisses mit dem Eintrag unter seinem Namen in `orphan`.
/// Mutationsbeweis: Ohne beide öffnet die Bestandsaufnahme das Ziel des
/// Verweises als Instanz und leert es, und der Test ist rot; ohne nur einen
/// bleibt er grün, weil der andere greift.
#[test]
fn a_symlink_is_never_followed() {
    let rt = runtime();
    let target = dead_instance(rt.path(), "target");
    std::os::unix::fs::symlink(&target, rt.path().join(DEAD)).unwrap();
    // Eine Instanz, deren Sperrdatei ein Verweis auf eine fremde Datei ist.
    let linked = rt.path().join(LIVE);
    std::fs::create_dir(&linked).unwrap();
    std::fs::write(rt.path().join("foreign-lock"), "").unwrap();
    std::os::unix::fs::symlink(rt.path().join("foreign-lock"), linked.join(LOCK_FILE)).unwrap();
    std::fs::create_dir(linked.join("s")).unwrap();

    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());

    assert!(
        target
            .join(SESSION)
            .join("files/deep/deeper/plan.json")
            .exists()
    );
    assert!(rt.path().join(DEAD).is_symlink());
    assert!(linked.join("s").exists());
    assert!(rt.path().join("foreign-lock").exists());
}

/// Mutationsbeweis: Prüft `is_instance_name` nur den Anfang, räumt die
/// Bestandsaufnahme die fremden Verzeichnisse, und der Test ist rot.
#[test]
fn a_foreign_name_is_untouched() {
    let rt = runtime();
    let names = [
        "humanitl-vm-0123456789abcde",
        "humanitl-vm-0123456789ABCDEF",
        "humanitl-vm-0123456789abcdef-x",
        "humanitl",
        "other",
    ];
    for name in names {
        dead_instance(rt.path(), name);
    }

    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());
    for name in names {
        assert!(rt.path().join(name).join(LOCK_FILE).exists(), "{name}");
    }
}

/// Ein Verzeichnis ohne Sperrdatei ist nicht nachweislich verwaist: aus der
/// Zeit vor HUM-492, oder es entsteht gerade. Nur die eigene Instanz bekommt
/// eine, denn ihr Start hat schon bewiesen, dass kein zweiter Daemon sie hält.
#[test]
fn an_instance_without_a_lock_is_only_removed_as_the_own() {
    let rt = runtime();
    let instance = rt.path().join(DEAD);
    std::fs::create_dir_all(instance.join("session")).unwrap();

    assert_eq!(removed_names(rt.path(), Some(LIVE)), Vec::<String>::new());
    assert!(instance.join("session").exists());

    assert_eq!(removed_names(rt.path(), Some(DEAD)), [DEAD]);
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

/// Mutationsbeweis: Ohne die Wandlung in eine exklusive Sperre in `release`
/// entfernt das Ende der letzten eigenen Sitzung die Sperrdatei unter dem
/// zweiten Prozess weg, und der Test ist rot.
#[test]
fn a_second_holder_keeps_the_instance() {
    let rt = runtime();
    let instance = rt.path().join(LIVE);
    let session = start(&instance, "b1");
    let other = hold_shared(&instance);

    end(&session);
    assert!(
        instance.join(LOCK_FILE).exists(),
        "the other holder loses it"
    );
    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());

    drop(other);
    assert_eq!(removed_names(rt.path(), None), [LIVE]);
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

#[test]
fn a_session_left_in_place_keeps_the_claim_until_release() {
    let rt = runtime();
    let instance = rt.path().join(LIVE);
    let first = start(&instance, "c1");
    let second = start(&instance, "c2");

    end(&first);
    assert!(second.exists());
    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());

    // Das Entfernen der zweiten scheiterte: Die Belegung bleibt bis zum Ende
    // des Prozesses, der nächste Start räumt danach.
    session_dir_removed(&second);
    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());
    std::fs::remove_dir_all(&second).unwrap();
    release_all();
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

/// Ein scheiternder Start hinterlässt keine Belegung.
#[test]
fn a_failed_start_releases_the_instance() {
    let rt = runtime();
    let instance = rt.path().join(LIVE);
    let failed = enter(
        &instance,
        OsStr::new("d1"),
        || Err::<(), String>("no".to_owned()),
        |why| why,
    );
    assert_eq!(failed, Err("no".to_owned()));
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

/// Ein Start wartet, solange ein anderer Daemon die verwaiste Instanz
/// desselben Namens räumt, und belegt danach eine neue.
#[test]
fn a_start_waits_for_a_sweep_and_claims_afresh() {
    let rt = runtime();
    let instance = dead_instance(rt.path(), LIVE);
    let orphans = find_orphans(rt.path(), None);
    assert_eq!(orphans.found.len(), 1);

    let starter = {
        let instance = instance.clone();
        std::thread::spawn(move || start(&instance, "e1"))
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        !starter.is_finished(),
        "the start did not wait for the sweep"
    );
    let _ = orphans.remove(|_| None, |_| {});
    let session = starter.join().unwrap();

    assert!(session.exists());
    assert!(instance.join(LOCK_FILE).exists());
    assert!(!instance.join(SESSION).exists());
    end(&session);
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

/// Der zweite Prozess der Tests unten: belegt eine Instanz unter dem
/// Laufzeitverzeichnis aus [`CHILD_ENV`], meldet `READY` und wartet auf eine
/// Zeile. `exit` beendet die Sitzung und den Prozess wie das Ende des
/// Daemons; sonst wartet er, bis ihn `SIGKILL` trifft. Ohne die Umgebung tut
/// er nichts.
#[test]
fn child_daemon() {
    let Some(runtime) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let instance = Path::new(&runtime).join(LIVE);
    let session = start(&instance, "f1");
    println!("READY");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    if line.trim() == "exit" {
        end(&session);
        release_all();
    } else {
        std::thread::sleep(std::time::Duration::from_secs(600));
    }
}

fn spawn_child(runtime: &Path) -> Child {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "qemu::instance::tests::child_daemon",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, runtime)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = std::io::BufReader::new(child.stdout.take().unwrap()).lines();
    let ready = lines
        .by_ref()
        .map_while(Result::ok)
        .any(|line| line.contains("READY"));
    assert!(ready, "the child never claimed its instance");
    // Der Rest der Ausgabe läuft ins Leere, damit das Kind beim Schreiben
    // nicht an einer geschlossenen Leitung scheitert.
    std::thread::spawn(move || lines.for_each(drop));
    child
}

/// Das Ende eines Daemons nimmt sein Verzeichnis mit; solange er lebt, räumt
/// kein anderer Prozess es weg.
#[test]
fn a_daemon_exit_removes_its_instance() {
    let rt = runtime();
    let mut child = spawn_child(rt.path());
    let instance = rt.path().join(LIVE);

    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());
    assert!(instance.join("f1").exists());

    writeln!(child.stdin.take().unwrap(), "exit").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(
        std::fs::symlink_metadata(&instance).is_err(),
        "{} stays behind after the exit",
        instance.display()
    );
}

/// Ein Daemon, den `SIGKILL` trifft, lässt sein Verzeichnis liegen; der
/// nächste Start eines anderen Daemons räumt es, weil die Sperre mit dem
/// Prozess verging.
#[test]
fn a_killed_daemon_is_swept_by_the_next_start() {
    let rt = runtime();
    let mut child = spawn_child(rt.path());
    let instance = rt.path().join(LIVE);
    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());

    child.kill().unwrap();
    let _ = child.wait();
    assert!(instance.join("f1").exists());

    let orphans = find_orphans(rt.path(), None);
    assert_eq!(orphans.found.len(), 1);
    let removed = orphans.remove(|_| None, |_| {});
    assert_eq!(removed.len(), 1);
    assert!(std::fs::symlink_metadata(&instance).is_err());
}

#[test]
fn the_unit_of_a_session_comes_from_its_name_only() {
    assert_eq!(
        derived(OsStr::new(SESSION)).map(|unit| unit.service()),
        Some(format!("humanitl-vm-{SESSION}.service"))
    );
    for name in [
        "0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F",
        "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0",
        "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0",
        "live",
        "",
    ] {
        assert!(derived(OsStr::new(name)).is_none(), "{name}");
    }
}

/// Der erste Riegel gegen Verweise allein: `open_dir_here` öffnet keinen.
///
/// Mutationsbeweis: Ohne `O_NOFOLLOW` und `RESOLVE_NO_SYMLINKS` folgt es dem
/// Verweis, und der Test ist rot.
#[test]
fn opening_for_removal_never_follows_a_symlink() {
    let rt = runtime();
    std::fs::create_dir(rt.path().join("real")).unwrap();
    // Relativ: Ein absoluter Verweis scheiterte schon an `RESOLVE_NO_XDEV`,
    // wenn `/` ein anderes Dateisystem ist.
    std::os::unix::fs::symlink("real", rt.path().join("link")).unwrap();
    let root = std::fs::File::open(rt.path()).unwrap();
    let dev = std::os::unix::fs::MetadataExt::dev(&root.metadata().unwrap());
    let root = OwnedFd::from(root);

    assert!(open_dir_here(root.as_fd(), OsStr::new("real"), dev).is_ok());
    assert!(open_dir_here(root.as_fd(), OsStr::new("link"), dev).is_err());
}

/// Der zweite Riegel allein: Tauscht jemand die Instanz zwischen dem Öffnen
/// und dem Vergleich gegen ein anderes Verzeichnis, gilt keins von beiden
/// als verwaist.
///
/// Mutationsbeweis: Ohne den Vergleich mit dem Eintrag unter dem Namen räumt
/// die Bestandsaufnahme das weggetauschte Verzeichnis, und der Test ist rot.
#[test]
fn a_swap_between_open_and_compare_is_refused() {
    let rt = runtime();
    let instance = dead_instance(rt.path(), DEAD);
    let moved = rt.path().join("moved");
    {
        let (instance, moved) = (instance.clone(), moved.clone());
        BEFORE_COMPARE.with_borrow_mut(|hook| {
            *hook = Some(Box::new(move || {
                std::fs::rename(&instance, &moved).unwrap();
                std::fs::create_dir(&instance).unwrap();
                std::fs::write(instance.join(LOCK_FILE), "").unwrap();
            }));
        });
    }

    assert_eq!(removed_names(rt.path(), None), Vec::<String>::new());
    assert!(
        moved
            .join(SESSION)
            .join("files/deep/deeper/plan.json")
            .exists()
    );
    assert!(instance.join(LOCK_FILE).exists());
}

/// Eine Sitzung, die tiefer reicht als jede, die der Daemon anlegt, bleibt
/// liegen und wird mit dem Befehl gemeldet, der sie entfernt.
#[test]
fn a_session_deeper_than_the_limit_is_kept_and_reported() {
    let rt = runtime();
    let instance = dead_instance(rt.path(), DEAD);
    let mut deep = instance.join(SESSION);
    for _ in 0..=MAX_DEPTH {
        deep.push("d");
    }
    std::fs::create_dir_all(&deep).unwrap();

    let removed = find_orphans(rt.path(), None).remove(|_| None, |_| {});
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].result, Err(Kept::TooDeep));
    assert!(instance.join(LOCK_FILE).exists());
    let warning = super::super::leftover_warning(removed.into_iter().next().unwrap());
    assert_eq!(warning.code.as_str(), "SANDBOX_041");
    assert!(
        warning.why.contains("deeper than 32 levels"),
        "{}",
        warning.why
    );
    assert!(
        matches!(&warning.fix, Some(humanitl_core::FixAction::CopyCommand(c))
            if c.starts_with("rm -r --one-file-system ")),
        "{:?}",
        warning.fix
    );
}

/// Ein Start, der auf das Räumen einer Instanz wartet, hält die Starts in
/// anderen Instanzen nicht auf.
///
/// Mutationsbeweis: Wartet `enter` unter der Sperre aller Belegungen, steht
/// der zweite Start, bis der erste aufgibt (30 s), und der Test ist rot.
#[test]
fn a_waiting_start_does_not_hold_up_other_starts() {
    let rt = runtime();
    let swept = dead_instance(rt.path(), LIVE);
    let orphans = find_orphans(rt.path(), None);
    assert_eq!(orphans.found.len(), 1);
    let waiting = {
        let swept = swept.clone();
        std::thread::spawn(move || start(&swept, "g1"))
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    let began = std::time::Instant::now();
    let other = start(&rt.path().join(DEAD), "g2");
    let took = began.elapsed();
    let still_waiting = !waiting.is_finished();
    let _ = orphans.remove(|_| None, |_| {});
    let first = waiting.join().unwrap();

    assert!(took < std::time::Duration::from_secs(2), "{took:?}");
    assert!(still_waiting, "the first start did not wait for the sweep");
    end(&other);
    end(&first);
}

/// Die Umgebung, mit der der Einhängetest im eigenen Namensraum läuft.
const MOUNT_ENV: &str = "HUMANITL_TEST_HUM492_MOUNTS";
/// Was der Teil im Namensraum sagt, wenn seine Einhängungen stehen.
const MOUNTED: &str = "hum492-mounted";
/// Die Instanz mit dem Bind-Mount.
const BIND: &str = "humanitl-vm-1111111111111111";

/// Ein fremdes Dateisystem und ein Bind-Mount in einer verwaisten Instanz
/// werden nie betreten, und eine Instanz, die selbst eingehängt ist, gilt nie
/// als verwaist. Der Test ruft sich selbst in einem Nutzer- und
/// Mount-Namensraum auf (`unshare -Urm`); geht das hier nicht, sagt er es
/// und prüft nichts.
///
/// Mutationsbeweis: Ohne `RESOLVE_NO_XDEV` (der Gerätevergleich bleibt)
/// leert das Räumen den Bind-Mount, und der Test ist rot.
#[test]
fn a_mount_inside_an_orphan_is_never_entered() {
    if let Some(dir) = std::env::var_os(MOUNT_ENV) {
        mounts_inside(Path::new(&dir));
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ran = Command::new("unshare")
        .args(["-Urm"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "qemu::instance::tests::a_mount_inside_an_orphan_is_never_entered",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MOUNT_ENV, dir.path())
        .output();
    let out = match ran {
        Ok(out) => out,
        Err(err) => {
            eprintln!("skipped: unshare cannot run ({err})");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !stdout.contains(MOUNTED) {
        eprintln!(
            "skipped: no mounts in a user namespace here ({})",
            stderr.trim()
        );
        return;
    }
    assert!(out.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("1 passed"), "{stdout}");
}

/// Der Teil im Namensraum.
fn mounts_inside(rt: &Path) {
    let mount = |args: &[&str]| {
        Command::new("mount")
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    let outside = rt.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), "keep").unwrap();
    // Je Einhängung eine eigene Instanz: Das Räumen hört an der ersten auf,
    // und jede soll für sich zeigen, dass sie nie betreten wird.
    let instance = dead_instance(rt, DEAD);
    let tmpfs = instance.join(SESSION).join("files/mnt");
    let second = dead_instance(rt, BIND);
    let bind = second.join(SESSION).join("files/bind");
    std::fs::create_dir(&tmpfs).unwrap();
    std::fs::create_dir(&bind).unwrap();
    // Eine Instanz, die selbst ein eigenes Dateisystem ist.
    let mounted = rt.join(LIVE);
    std::fs::create_dir(&mounted).unwrap();
    let ok = mount(&["-t", "tmpfs", "none", &tmpfs.to_string_lossy()])
        && mount(&[
            "--bind",
            &outside.to_string_lossy(),
            &bind.to_string_lossy(),
        ])
        && mount(&["-t", "tmpfs", "none", &mounted.to_string_lossy()]);
    if !ok {
        return;
    }
    println!("{MOUNTED}");
    std::fs::write(tmpfs.join("keep"), "keep").unwrap();
    std::fs::write(mounted.join(LOCK_FILE), "").unwrap();
    std::fs::create_dir(mounted.join(SESSION)).unwrap();
    std::fs::write(mounted.join(SESSION).join("keep"), "keep").unwrap();

    let orphans = find_orphans(rt, None);
    let mut names: Vec<_> = orphans.found.iter().map(|o| o.name.clone()).collect();
    names.sort();
    assert_eq!(names, [OsStr::new(BIND), OsStr::new(DEAD)]);
    let removed = orphans.remove(|_| None, |_| {});

    assert_eq!(removed.len(), 2);
    assert!(
        removed.iter().all(|r| r.result == Err(Kept::Mount)),
        "{removed:?}"
    );
    assert_eq!(std::fs::read_to_string(tmpfs.join("keep")).unwrap(), "keep");
    assert_eq!(
        std::fs::read_to_string(outside.join("keep")).unwrap(),
        "keep"
    );
    assert!(mounted.join(SESSION).join("keep").exists());
    assert!(instance.join(LOCK_FILE).exists());
    assert!(second.join(LOCK_FILE).exists());
}
