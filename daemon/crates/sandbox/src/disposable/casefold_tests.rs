//! Masken und Schreibweise (HUM-482).
//!
//! Die meisten Tests geben das Attribut Casefold über einen eigenen Leser
//! vor, damit sie auf jedem Dateisystem laufen. Ein echter Ordner mit
//! Casefold entsteht ohne Rechte nur als tmpfs mit `casefold` in einem
//! eigenen Nutzer- und Mount-Namensraum (Linux 6.13, `unshare -Urm`); wo das
//! nicht geht, sagt [`a_real_casefold_folder_is_refused`] es auf stderr und
//! endet grün.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::Cell;
use std::fs;
use std::os::fd::{AsFd as _, BorrowedFd, OwnedFd};
use std::os::unix::fs::{MetadataExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use rustix::io::Errno;

use super::casefold::{
    refuse_case_folding, refuse_case_folding_before_apply, refuse_case_folding_probed,
    refuse_case_folding_with,
};
use crate::worktree::open_root;

fn masks(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

/// Ein Leser, der genau den Ordner unter `marked` als gefaltet meldet.
fn marking(marked: &Path) -> impl Fn(BorrowedFd<'_>) -> Result<bool, Errno> + use<> {
    let meta = fs::metadata(marked).unwrap();
    let id = (meta.dev(), meta.ino());
    move |dir| {
        let stat = rustix::fs::fstat(dir)?;
        #[allow(clippy::useless_conversion)]
        let dev = u64::from(stat.st_dev);
        Ok((dev, stat.st_ino) == id)
    }
}

fn project() -> (tempfile::TempDir, PathBuf, OwnedFd) {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("p");
    fs::create_dir_all(project.join(".git/hooks")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join(".env"), b"SECRET=1\n").unwrap();
    let lower = open_root(&project).unwrap();
    (dir, project, lower)
}

/// Eine gefaltete Wurzel verweigert jede Maske, mit `SANDBOX_030` und dem
/// Rückweg auf `direct`; ohne Masken gibt es nichts zu umgehen.
#[test]
fn a_case_insensitive_root_refuses_the_disposable_sandbox() {
    let (_dir, project, lower) = project();
    let read = marking(&project);
    let err = refuse_case_folding_with(lower.as_fd(), &masks(&[".env"]), &read)
        .expect_err("a folded root");
    assert_eq!(err.code.as_str(), "SANDBOX_030");
    assert!(
        err.why
            .contains(". in the project folder is case-insensitive"),
        "{}",
        err.why
    );
    assert!(err.why.contains("mask .env"), "{}", err.why);
    assert!(
        matches!(&err.fix, Some(humanitl_core::FixAction::ChangeSetting { value, .. }) if value == "direct"),
        "{:?}",
        err.fix
    );
    // Eine Maske, die es noch nicht gibt, wird in derselben Wurzel
    // nachgeschlagen.
    assert!(refuse_case_folding_with(lower.as_fd(), &masks(&["absent/x"]), &read).is_err());
    refuse_case_folding_with(lower.as_fd(), &[], &read).expect("no masks, nothing to bypass");
}

/// Ein gefalteter Ordner auf dem Weg zu einer Maske verweigert; einer
/// daneben nicht.
///
/// Mutationsbeweis: `check_dir` nur für die Wurzel rufen, und der erste
/// Fall läuft durch.
#[test]
fn a_case_insensitive_directory_on_the_way_to_a_mask_refuses() {
    let (_dir, project, lower) = project();
    let read = marking(&project.join(".git"));
    let err = refuse_case_folding_with(lower.as_fd(), &masks(&[".git/hooks"]), &read)
        .expect_err("a folded .git");
    assert!(
        err.why
            .contains(".git in the project folder is case-insensitive"),
        "{}",
        err.why
    );
    refuse_case_folding_with(lower.as_fd(), &masks(&[".env", "src/x"]), &read)
        .expect("the folded folder is on no mask's way");

    let read = marking(&project.join("src"));
    refuse_case_folding_with(lower.as_fd(), &masks(&[".git/hooks", ".env"]), &read)
        .expect("src is on no mask's way");
}

/// Der Weg endet an einem Namen, den es nicht gibt, oder an einem Symlink;
/// dessen Ziel wird nie gelesen.
#[test]
fn the_walk_stops_at_missing_names_and_never_follows_a_symlink() {
    let (dir, project, lower) = project();
    let outside = dir.path().join("outside");
    fs::create_dir_all(outside.join("hooks")).unwrap();
    symlink(&outside, project.join("linked")).unwrap();
    let seen = Cell::new(0);
    let outer = marking(&outside);
    let read = |fd: BorrowedFd<'_>| {
        seen.set(seen.get() + 1);
        outer(fd)
    };
    refuse_case_folding_with(lower.as_fd(), &masks(&["linked/hooks", "gone/a/b"]), &read)
        .expect("nothing folded on the way");
    // Nur die Wurzel, einmal.
    assert_eq!(seen.get(), 1);
}

/// Lässt sich das Attribut nicht lesen, startet nichts.
#[test]
fn an_unreadable_attribute_refuses() {
    let (_dir, _project, lower) = project();
    let read = |_: BorrowedFd<'_>| Err(Errno::IO);
    let err = refuse_case_folding_with(lower.as_fd(), &masks(&[".env"]), &read)
        .expect_err("unknown is refused");
    assert!(err.why.contains("cannot be checked"), "{}", err.why);
}

/// Ein Dateisystem, das faltet, ohne das Attribut zu tragen (FUSE, SMB),
/// verrät sich, wenn der Name der Maske schon da ist: Die andere
/// Schreibweise nennt denselben Eintrag. Ein harter Verweis spielt das hier
/// nach.
///
/// Mutationsbeweis: `same_entry` übergangen, und der Test färbt sich rot.
#[test]
fn a_name_that_resolves_in_another_case_refuses() {
    let (_dir, project, lower) = project();
    let never = |_: BorrowedFd<'_>| Ok(false);
    refuse_case_folding_with(lower.as_fd(), &masks(&[".env", ".git/hooks"]), &never)
        .expect("a case-sensitive project");
    fs::hard_link(project.join(".env"), project.join(".ENV")).unwrap();
    let err = refuse_case_folding_with(lower.as_fd(), &masks(&[".env"]), &never)
        .expect_err("another case names the same entry");
    assert!(
        err.why
            .contains(".env in the project folder names the same entry"),
        "{}",
        err.why
    );
}

/// Ein Fehler beim Nachschlagen der anderen Schreibweise gilt nie als
/// abwesend (HUM-482, Review): Nur `ENOENT` und `ENOTDIR` heißen „nicht da",
/// jeder andere Fehler verweigert.
///
/// Mutationsbeweis: in `same_entry` jeden Fehler als abwesend zählen, und
/// der Test färbt sich rot.
#[test]
fn a_failed_lookup_of_the_other_case_refuses() {
    let (_dir, _project, lower) = project();
    let never = |_: BorrowedFd<'_>| Ok(false);
    let failing = |dir: BorrowedFd<'_>, name: &std::ffi::OsStr| {
        if name == ".ENV" {
            Err(Errno::IO)
        } else {
            let stat = rustix::fs::statat(dir, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
            #[allow(clippy::useless_conversion)]
            Ok((u64::from(stat.st_dev), stat.st_ino))
        }
    };
    let err = refuse_case_folding_probed(lower.as_fd(), &masks(&[".env"]), &never, &failing)
        .expect_err("a failed lookup is no absence");
    assert_eq!(err.code.as_str(), "SANDBOX_030");
    assert!(err.why.contains("cannot be checked"), "{}", err.why);
    let absent = |_: BorrowedFd<'_>, _: &std::ffi::OsStr| Err(Errno::NOENT);
    refuse_case_folding_probed(lower.as_fd(), &masks(&[".env"]), &never, &absent)
        .expect("ENOENT is absent");
}

/// Die Prüfung vor der Übernahme sagt `SANDBOX_031` mit dem Rückweg auf
/// `discard`.
#[test]
fn the_check_before_apply_refuses_with_sandbox_031() {
    let (_dir, project, lower) = project();
    refuse_case_folding_before_apply(lower.as_fd(), &masks(&[".env"])).expect("nothing folds");
    fs::hard_link(project.join(".env"), project.join(".ENV")).unwrap();
    let err =
        refuse_case_folding_before_apply(lower.as_fd(), &masks(&[".env"])).expect_err("folds now");
    assert_eq!(err.code.as_str(), "SANDBOX_031");
    assert!(
        err.why.contains("nothing is read or applied"),
        "{}",
        err.why
    );
    assert!(
        matches!(&err.fix, Some(humanitl_core::FixAction::CopyCommand(cmd)) if cmd == "humanitl sandbox discard"),
        "{:?}",
        err.fix
    );
}

/// Der echte Leser meldet auf einem gewöhnlichen Ordner nichts.
#[test]
fn an_ordinary_folder_passes_with_the_real_reader() {
    let (_dir, _project, lower) = project();
    refuse_case_folding(lower.as_fd(), &masks(&[".env", ".git/hooks"])).expect("ordinary");
}

/// Die Umgebungsvariable, über die der Test im Namensraum seinen Ordner
/// erfährt.
const INNER: &str = "HUMANITL_HUM482_CASEFOLD_DIR";

/// Ein echter Ordner mit Casefold (tmpfs mit `casefold`, Attribut über
/// `FS_IOC_SETFLAGS`): `.GIT/hooks` ist dort `.git/hooks`, auch das
/// Kelvin-Zeichen faltet zu `k`, und die Wegwerf-Sandbox verweigert. Der
/// Test ruft sich selbst in einem Nutzer- und Mount-Namensraum auf.
///
/// Mutationsbeweis: `casefold_flag` meldet immer `false`, und der Test
/// färbt sich rot.
#[test]
fn a_real_casefold_folder_is_refused() {
    if let Ok(dir) = std::env::var(INNER) {
        inside(Path::new(&dir));
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let ran = Command::new("unshare")
        .args(["-Urm", "sh", "-c"])
        .arg(
            "mount -t tmpfs -o casefold none \"$1\" || exit 77; echo {MOUNTED}; exec \"$2\" \
             --exact disposable::casefold_tests::a_real_casefold_folder_is_refused \
             --test-threads=1"
                .replace("{MOUNTED}", MOUNTED),
        )
        .arg("sh")
        .arg(dir.path())
        .arg(&exe)
        .env(INNER, dir.path())
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
            "skipped: no casefold tmpfs in a user namespace here ({})",
            stderr.trim()
        );
        return;
    }
    assert!(out.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("1 passed"), "{stdout}");
}

/// Was das Skript sagt, wenn das tmpfs mit `casefold` hängt.
const MOUNTED: &str = "hum482-casefold-mounted";

/// Der Teil im Namensraum, auf dem tmpfs mit `casefold`.
fn inside(root: &Path) {
    const FS_CASEFOLD_FL: u32 = 0x4000_0000;
    let fold = |dir: &Path| {
        fs::create_dir(dir).unwrap();
        let fd = open_root(dir).unwrap();
        let flags = rustix::fs::ioctl_getflags(&fd).unwrap();
        rustix::fs::ioctl_setflags(
            &fd,
            rustix::fs::IFlags::from_bits_retain(flags.bits() | FS_CASEFOLD_FL),
        )
        .unwrap();
    };
    // Ein gefaltetes Projekt.
    let folded = root.join("folded");
    fold(&folded);
    fs::create_dir_all(folded.join(".git/hooks")).unwrap();
    assert!(folded.join(".GIT/HOOKS").is_dir());
    assert!(folded.join(".git/hoo\u{212a}s").is_dir(), "Unicode folding");
    let lower = open_root(&folded).unwrap();
    let err = refuse_case_folding(lower.as_fd(), &masks(&[".git/hooks"])).expect_err("folded");
    assert!(err.why.contains("casefold attribute"), "{}", err.why);

    // Ein Projekt, in dem nur ein Ordner neben den Masken faltet, startet;
    // faltet einer auf dem Weg, nicht.
    let plain = root.join("plain");
    fs::create_dir_all(plain.join(".git/hooks")).unwrap();
    fold(&plain.join("docs"));
    let lower = open_root(&plain).unwrap();
    refuse_case_folding(lower.as_fd(), &masks(&[".git/hooks", ".env"])).expect("plain");
    let err = refuse_case_folding(lower.as_fd(), &masks(&["docs/.env"])).expect_err("docs");
    assert!(
        err.why.contains("docs in the project folder"),
        "{}",
        err.why
    );
}
