//! Diff, Übernahme und Verwerfen gegen feindliche obere Schichten (HUM-350).
//!
//! Die obere Schicht wird hier von Hand gebaut, so wie overlayfs sie
//! schreibt: Whiteouts als Zeichengerät `0:0`, opake Verzeichnisse mit
//! `user.overlay.opaque = "y"`. Wo der Kernel dieser Maschine eines davon
//! nicht erlaubt, sagt der Test das auf stderr und endet grün; die echte
//! Schicht prüfen die Tests mit `bwrap` in `tests/launcher.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use humanitl_core::Diagnostic;
use rustix::fs::{FileType, Mode, OFlags};

use super::*;
use crate::summary::ChangeKind;
use crate::worktree::TreeSnapshot;
use crate::worktree::{Kind, open_root};

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    lower: PathBuf,
    upper: PathBuf,
    outside: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let lower = root.join("lower");
    let upper = root.join("upper");
    let outside = root.join("outside");
    for path in [&lower, &upper, &outside] {
        fs::create_dir_all(path).unwrap();
    }
    fs::write(lower.join("keep.txt"), "keep\n").unwrap();
    fs::write(lower.join("edit.txt"), "one\ntwo\n").unwrap();
    fs::create_dir_all(lower.join("dir/sub")).unwrap();
    fs::write(lower.join("dir/a"), "a\n").unwrap();
    fs::write(lower.join("dir/sub/b"), "b\n").unwrap();
    fs::write(outside.join("secret"), "host\n").unwrap();
    Fixture {
        _dir: dir,
        root,
        lower,
        upper,
        outside,
    }
}

impl Fixture {
    fn fds(&self) -> (OwnedFd, OwnedFd) {
        (
            open_root(&self.upper).unwrap(),
            open_root(&self.lower).unwrap(),
        )
    }

    fn baseline(&self) -> TreeSnapshot {
        baseline(&self.lower).unwrap()
    }

    fn scan(&self, base: &TreeSnapshot) -> ChangeSet {
        let (upper, lower) = self.fds();
        scan(upper.as_fd(), lower.as_fd(), base)
    }

    fn apply(
        &self,
        base: &TreeSnapshot,
        ids: &[String],
        overwrite: bool,
    ) -> Result<ApplyReport, Diagnostic> {
        let set = self.scan(base);
        let chosen = select(&set, ids)?;
        let (upper, lower) = self.fds();
        apply(
            upper.as_fd(),
            lower.as_fd(),
            base,
            &chosen,
            &ApplyOptions {
                overwrite,
                ..ApplyOptions::default()
            },
        )
    }

    /// Wie [`Fixture::apply`], mit Überschreiben nur für `overwrite_ids`.
    fn apply_overwriting(
        &self,
        base: &TreeSnapshot,
        ids: &[String],
        overwrite_ids: &[String],
    ) -> Result<ApplyReport, Diagnostic> {
        let set = self.scan(base);
        let chosen = select(&set, ids)?;
        let (upper, lower) = self.fds();
        apply(
            upper.as_fd(),
            lower.as_fd(),
            base,
            &chosen,
            &ApplyOptions {
                overwrite: false,
                overwrite_ids: overwrite_ids.iter().cloned().collect(),
            },
        )
    }
}

fn change_at(set: &ChangeSet, rel: &str) -> LayerChange {
    set.changes
        .iter()
        .find(|change| change.rel == Path::new(rel))
        .cloned()
        .unwrap_or_else(|| panic!("no change for {rel}: {:#?}", set.changes))
}

/// Legt ein Whiteout an, wie overlayfs es schreibt; `false`, wenn der
/// Kernel es nicht erlaubt.
fn whiteout(path: &Path) -> bool {
    let parent = rustix::fs::open(
        path.parent().unwrap(),
        OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .unwrap();
    match rustix::fs::mknodat(
        &parent,
        path.file_name().unwrap(),
        FileType::CharacterDevice,
        Mode::empty(),
        0,
    ) {
        Ok(()) => true,
        Err(err) => {
            eprintln!("skipping: this kernel refuses a whiteout ({err})");
            false
        }
    }
}

fn opaque(path: &Path) -> bool {
    match rustix::fs::setxattr(
        path,
        "user.overlay.opaque",
        b"y",
        rustix::fs::XattrFlags::empty(),
    ) {
        Ok(()) => true,
        Err(err) => {
            eprintln!("skipping: this file system refuses user xattrs ({err})");
            false
        }
    }
}

fn kinds(set: &ChangeSet) -> Vec<(String, ChangeKind)> {
    set.changes
        .iter()
        .map(|change| (change.rel.to_string_lossy().into_owned(), change.kind))
        .collect()
}

// --- Diff ---------------------------------------------------------------------

#[test]
fn a_copied_up_file_without_changes_is_no_change() {
    let fx = fixture();
    let base = fx.baseline();
    fs::copy(fx.lower.join("keep.txt"), fx.upper.join("keep.txt")).unwrap();
    fs::set_permissions(
        fx.upper.join("keep.txt"),
        fs::metadata(fx.lower.join("keep.txt"))
            .unwrap()
            .permissions(),
    )
    .unwrap();
    assert!(fx.scan(&base).changes.is_empty());
}

#[test]
fn added_modified_and_mode_changes_are_told_apart() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("new.txt"), "fresh\n").unwrap();
    fs::write(fx.upper.join("edit.txt"), "one\nTWO\n").unwrap();
    fs::copy(fx.lower.join("keep.txt"), fx.upper.join("keep.txt")).unwrap();
    fs::set_permissions(fx.upper.join("keep.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    let set = fx.scan(&base);
    assert_eq!(
        kinds(&set),
        vec![
            ("edit.txt".to_owned(), ChangeKind::Modified),
            ("keep.txt".to_owned(), ChangeKind::ModeChanged),
            ("new.txt".to_owned(), ChangeKind::Added),
        ]
    );
    let edit = change_at(&set, "edit.txt");
    let (upper, lower) = fx.fds();
    let diff = edit.text_diff(upper.as_fd(), lower.as_fd()).unwrap();
    assert!(diff.contains("-two\n+TWO\n"), "{diff}");
    assert!(set.changes.iter().all(|change| change.conflict.is_none()));
}

#[test]
fn a_whiteout_removes_a_file_and_a_whole_directory() {
    let fx = fixture();
    let base = fx.baseline();
    if !whiteout(&fx.upper.join("keep.txt")) || !whiteout(&fx.upper.join("dir")) {
        return;
    }
    // Ein Whiteout über nichts ist keine Änderung.
    assert!(whiteout(&fx.upper.join("never-was")));
    let set = fx.scan(&base);
    assert_eq!(
        kinds(&set),
        vec![
            ("dir".to_owned(), ChangeKind::Removed),
            ("keep.txt".to_owned(), ChangeKind::Removed),
        ]
    );
    let report = fx.apply(&base, &[], false).unwrap();
    assert!(report.failed.is_none(), "{:?}", report.failed);
    assert!(!fx.lower.join("keep.txt").exists());
    assert!(!fx.lower.join("dir").exists());
    assert!(fx.lower.join("edit.txt").exists());
}

#[test]
fn an_opaque_directory_removes_what_it_hides() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("dir")).unwrap();
    fs::set_permissions(
        fx.upper.join("dir"),
        fs::metadata(fx.lower.join("dir")).unwrap().permissions(),
    )
    .unwrap();
    fs::write(fx.upper.join("dir/a"), "a again\n").unwrap();
    if !opaque(&fx.upper.join("dir")) {
        return;
    }
    let set = fx.scan(&base);
    assert_eq!(
        kinds(&set),
        vec![
            ("dir/a".to_owned(), ChangeKind::Modified),
            ("dir/sub".to_owned(), ChangeKind::Removed),
        ]
    );
    fx.apply(&base, &[], false).unwrap();
    assert!(!fx.lower.join("dir/sub").exists());
    assert_eq!(
        fs::read_to_string(fx.lower.join("dir/a")).unwrap(),
        "a again\n"
    );
}

#[test]
fn special_files_are_listed_and_never_written() {
    let fx = fixture();
    let base = fx.baseline();
    let fifo = std::ffi::CString::new(fx.upper.join("pipe").into_os_string().into_encoded_bytes())
        .unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        fifo.as_c_str(),
        FileType::Fifo,
        Mode::from_raw_mode(0o644),
        0,
    )
    .unwrap();
    let set = fx.scan(&base);
    let pipe = change_at(&set, "pipe");
    assert!(
        pipe.refused.as_deref().unwrap().contains("FIFO"),
        "{pipe:?}"
    );
    let err = fx.apply(&base, &[], false).unwrap_err();
    assert_eq!(err.code.as_str(), "SANDBOX_031");
    assert!(!fx.lower.join("pipe").exists());
}

#[test]
fn setuid_and_setgid_bits_are_stripped_and_hardlinks_split() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("tool"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(fx.upper.join("tool"), fs::Permissions::from_mode(0o6755)).unwrap();
    fs::hard_link(fx.upper.join("tool"), fx.upper.join("tool2")).unwrap();
    let set = fx.scan(&base);
    let tool = change_at(&set, "tool");
    assert_eq!(tool.mode, 0o755);
    assert_eq!(tool.stripped, 0o6000);
    assert!(tool.hardlinked);
    let report = fx.apply(&base, &[], false).unwrap();
    assert!(report.failed.is_none());
    let one = fs::metadata(fx.lower.join("tool")).unwrap();
    let two = fs::metadata(fx.lower.join("tool2")).unwrap();
    assert_eq!(one.mode() & 0o7777, 0o755, "no setuid on the host");
    assert_eq!(one.nlink(), 1, "written as its own file");
    assert_ne!(one.ino(), two.ino());
}

#[test]
fn a_directory_without_permissions_in_the_layer_is_still_read() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("locked")).unwrap();
    fs::write(fx.upper.join("locked/inside"), "x\n").unwrap();
    fs::set_permissions(
        fx.upper.join("locked/inside"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    fs::set_permissions(fx.upper.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let set = fx.scan(&base);
    assert!(!set.truncated, "{set:#?}");
    assert_eq!(change_at(&set, "locked/inside").kind, ChangeKind::Added);
}

// --- Übernahme: Symlinks ---------------------------------------------------------

/// Mutationsbeweis: Ohne die Prüfung in `scan::Scanner::symlink` und in
/// `apply::apply_one` stünde nach der Übernahme ein Verweis auf den Host im
/// Projekt.
#[test]
fn apply_refuses_a_symlink_out_of_the_project_and_writes_nothing() {
    let fx = fixture();
    let base = fx.baseline();
    symlink(&fx.outside, fx.upper.join("escape")).unwrap();
    symlink("../../etc/passwd", fx.upper.join("dotdot")).unwrap();
    symlink("keep.txt", fx.upper.join("inside")).unwrap();
    fs::write(fx.upper.join("innocent.txt"), "text\n").unwrap();
    let set = fx.scan(&base);
    assert!(change_at(&set, "escape").refused.is_some());
    assert!(change_at(&set, "dotdot").refused.is_some());
    assert!(change_at(&set, "inside").refused.is_none());

    let err = fx.apply(&base, &[], false).unwrap_err();
    assert_eq!(err.code.as_str(), "SANDBOX_031");
    assert!(err.why.contains("nothing was written"), "{}", err.why);
    for name in ["escape", "dotdot", "inside", "innocent.txt"] {
        assert!(
            fs::symlink_metadata(fx.lower.join(name)).is_err(),
            "{name} was written"
        );
    }

    // Einzeln gewählt geht, was im Projekt bleibt.
    let ids = vec![
        change_at(&set, "inside").path_hash(),
        change_at(&set, "innocent.txt").path_hash(),
    ];
    fx.apply(&base, &ids, false).unwrap();
    assert_eq!(
        fs::read_link(fx.lower.join("inside")).unwrap(),
        Path::new("keep.txt")
    );
    let err = fx
        .apply(&base, &[change_at(&set, "escape").path_hash()], true)
        .unwrap_err();
    assert!(err.why.contains("leaves the project"), "{}", err.why);
    assert!(fs::symlink_metadata(fx.lower.join("escape")).is_err());
}

#[test]
fn apply_never_writes_through_a_symlink_that_appeared_in_the_project() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("dir")).unwrap();
    fs::set_permissions(
        fx.upper.join("dir"),
        fs::metadata(fx.lower.join("dir")).unwrap().permissions(),
    )
    .unwrap();
    fs::write(fx.upper.join("dir/planted"), "evil\n").unwrap();
    // Nach dem Start wird `dir` im Projekt zu einem Symlink nach draußen.
    fs::rename(fx.lower.join("dir"), fx.root.join("dir-moved")).unwrap();
    symlink(&fx.outside, fx.lower.join("dir")).unwrap();

    for overwrite in [false, true] {
        let result = fx.apply(&base, &[], overwrite);
        match result {
            Err(err) => assert_eq!(err.code.as_str(), "SANDBOX_031"),
            Ok(report) => assert!(report.failed.is_some(), "{report:?}"),
        }
        assert!(
            !fx.outside.join("planted").exists(),
            "written through the link"
        );
        assert_eq!(
            fs::read_to_string(fx.outside.join("secret")).unwrap(),
            "host\n"
        );
    }
}

#[test]
fn a_file_replaces_a_symlink_at_its_place_without_following_it() {
    let fx = fixture();
    symlink(fx.outside.join("secret"), fx.lower.join("link")).unwrap();
    let base = fx.baseline();
    fs::write(fx.upper.join("link"), "plain file\n").unwrap();
    let set = fx.scan(&base);
    assert_eq!(change_at(&set, "link").kind, ChangeKind::Modified);
    fx.apply(&base, &[], false).unwrap();
    assert!(
        fs::symlink_metadata(fx.lower.join("link"))
            .unwrap()
            .is_file()
    );
    assert_eq!(
        fs::read_to_string(fx.outside.join("secret")).unwrap(),
        "host\n"
    );
}

// --- Übernahme: Konflikte ----------------------------------------------------------

#[test]
fn a_file_the_user_changed_since_the_start_is_not_overwritten_unasked() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("edit.txt"), "agent\n").unwrap();
    fs::write(fx.upper.join("new.txt"), "agent\n").unwrap();
    // Der Mensch arbeitet weiter, während die Sandbox lief.
    std::thread::sleep(Duration::from_millis(20));
    fs::write(fx.lower.join("edit.txt"), "human, longer\n").unwrap();

    let set = fx.scan(&base);
    assert!(change_at(&set, "edit.txt").conflict.is_some());
    let err = fx.apply(&base, &[], false).unwrap_err();
    assert!(
        err.why.contains("changed in the project after the start"),
        "{}",
        err.why
    );
    assert_eq!(
        fs::read_to_string(fx.lower.join("edit.txt")).unwrap(),
        "human, longer\n"
    );
    assert!(
        !fx.lower.join("new.txt").exists(),
        "nothing of the selection was written"
    );

    fx.apply(&base, &[], true).unwrap();
    assert_eq!(
        fs::read_to_string(fx.lower.join("edit.txt")).unwrap(),
        "agent\n"
    );
}

/// Eine Bestätigung gilt einer Datei, nie einer anderen (HUM-351): `a.txt`
/// ist zum Überschreiben bestätigt, `b.txt` bekam seinen Konflikt erst nach
/// der Auswahl. Nichts wird geschrieben, und der Befund nennt `b.txt`.
#[test]
fn a_conflict_not_named_for_overwrite_is_not_overwritten_even_if_another_is() {
    let fx = fixture();
    fs::write(fx.lower.join("a.txt"), "a\n").unwrap();
    fs::write(fx.lower.join("b.txt"), "b\n").unwrap();
    let base = fx.baseline();
    fs::write(fx.upper.join("a.txt"), "agent a\n").unwrap();
    fs::write(fx.upper.join("b.txt"), "agent b\n").unwrap();
    std::thread::sleep(Duration::from_millis(20));
    fs::write(fx.lower.join("a.txt"), "human a, longer\n").unwrap();
    fs::write(fx.lower.join("b.txt"), "human b, longer\n").unwrap();

    let set = fx.scan(&base);
    let a = change_at(&set, "a.txt").path_hash();
    let b = change_at(&set, "b.txt").path_hash();
    let err = fx
        .apply_overwriting(&base, &[a.clone(), b.clone()], std::slice::from_ref(&a))
        .unwrap_err();
    assert!(err.why.contains("b.txt"), "{}", err.why);
    assert!(!err.why.contains("a.txt:"), "{}", err.why);
    assert_eq!(
        fs::read_to_string(fx.lower.join("a.txt")).unwrap(),
        "human a, longer\n"
    );
    assert_eq!(
        fs::read_to_string(fx.lower.join("b.txt")).unwrap(),
        "human b, longer\n"
    );

    // Beide bestätigt: beide geschrieben.
    fx.apply_overwriting(&base, &[a.clone(), b.clone()], &[a, b])
        .unwrap();
    assert_eq!(
        fs::read_to_string(fx.lower.join("a.txt")).unwrap(),
        "agent a\n"
    );
    assert_eq!(
        fs::read_to_string(fx.lower.join("b.txt")).unwrap(),
        "agent b\n"
    );
}

#[test]
fn removing_a_directory_the_user_added_to_is_a_conflict() {
    let fx = fixture();
    let base = fx.baseline();
    if !whiteout(&fx.upper.join("dir")) {
        return;
    }
    fs::write(fx.lower.join("dir/sub/human-notes"), "mine\n").unwrap();
    let err = fx.apply(&base, &[], false).unwrap_err();
    assert!(err.why.contains("below it"), "{}", err.why);
    assert!(fx.lower.join("dir/sub/human-notes").exists());
}

#[test]
fn a_file_created_in_the_project_meanwhile_is_a_conflict() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("both.txt"), "agent\n").unwrap();
    fs::write(fx.lower.join("both.txt"), "human\n").unwrap();
    let err = fx.apply(&base, &[], false).unwrap_err();
    assert!(
        err.why.contains("created in the project after the start"),
        "{}",
        err.why
    );
}

// --- Auswahl --------------------------------------------------------------------------

#[test]
fn an_unknown_id_and_apply_all_of_a_cut_list_are_refused() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("new.txt"), "x\n").unwrap();
    let mut set = fx.scan(&base);
    let err = select(&set, &["0123456789abcdef".to_owned()]).unwrap_err();
    assert!(err.why.contains("no pending change"), "{}", err.why);
    set.truncated = true;
    let err = select(&set, &[]).unwrap_err();
    assert!(err.why.contains("cut short"), "{}", err.why);
    assert_eq!(
        select(&set, &[set.changes[0].path_hash()]).unwrap().len(),
        1
    );
}

#[test]
fn new_directories_are_created_with_their_files() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir_all(fx.upper.join("src/deep")).unwrap();
    fs::write(fx.upper.join("src/deep/main.rs"), "fn main() {}\n").unwrap();
    let set = fx.scan(&base);
    let file = change_at(&set, "src/deep/main.rs");
    // Nur die Datei gewählt: die Verzeichnisse entstehen mit.
    fx.apply(&base, &[file.path_hash()], false).unwrap();
    assert_eq!(
        fs::read_to_string(fx.lower.join("src/deep/main.rs")).unwrap(),
        "fn main() {}\n"
    );
}

// --- Schicht und Verwerfen ------------------------------------------------------------

fn tree_in(upper: &Path) {
    fs::write(upper.join("file"), "x").unwrap();
    let mut deep = upper.join("deep");
    for _ in 0..(MAX_DEPTH * 3) {
        deep = deep.join("d");
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("leaf"), "leaf").unwrap();
    fs::create_dir(upper.join("locked")).unwrap();
    fs::write(upper.join("locked/inside"), "x").unwrap();
    fs::set_permissions(upper.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    symlink("/", upper.join("root-link")).unwrap();
}

/// Mutationsbeweis: Ohne das Erzwingen der Rechte (`open_dir_forcing`) oder
/// ohne das flache Umhängen bliebe ein Rest im Arbeitsspeicher.
#[test]
fn discard_leaves_no_upper_layer_behind() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = dir.path().join("runtime");
    let layer = Layer::create_with(&runtime, TmpfsCheck::Skip).unwrap();
    let mode = fs::metadata(layer.dir()).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o700);
    assert_eq!(
        fs::metadata(runtime.join(DISPOSABLE_DIR)).unwrap().mode() & 0o777,
        0o700
    );
    tree_in(&layer.upper());
    // Wie overlayfs: `work/work` ohne Rechte.
    fs::create_dir(layer.work().join("work")).unwrap();
    fs::set_permissions(layer.work().join("work"), fs::Permissions::from_mode(0o000)).unwrap();
    let path = layer.dir().to_path_buf();
    layer
        .discard()
        .map_err(|(_, diagnostic)| diagnostic)
        .unwrap();
    assert!(!path.exists(), "the layer is gone");
    assert_eq!(
        fs::read_dir(runtime.join(DISPOSABLE_DIR)).unwrap().count(),
        0,
        "nothing flattened stayed"
    );
    assert!(
        Path::new("/").join("etc").exists(),
        "the symlink was not followed"
    );
}

/// Mutationsbeweis: Ohne das flache Umhängen in `remove_one` stiege das
/// Verwerfen 20 000 Ebenen tief rekursiv und liefe über den Stapel dieses
/// Fadens (1 MiB); der Testprozess stürzte ab. Ein Agent kann so tief
/// schachteln, die Grenze der Schicht zählt Einträge, nicht Ebenen.
#[test]
fn discard_flattens_a_tree_deeper_than_the_stack() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = dir.path().join("runtime");
    let layer = Layer::create_with(&runtime, TmpfsCheck::Skip).unwrap();
    // Schritt für Schritt über Deskriptoren: ein Pfad dieser Tiefe wäre
    // länger als `PATH_MAX`.
    let mut current = open_root(&layer.upper()).unwrap();
    for _ in 0..20_000 {
        rustix::fs::mkdirat(&current, "d", Mode::from_raw_mode(0o700)).unwrap();
        current = rustix::fs::openat(
            &current,
            "d",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .unwrap();
    }
    drop(current);
    let path = layer.dir().to_path_buf();
    let removed = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || layer.discard().map_err(|(_, diagnostic)| diagnostic))
        .unwrap()
        .join()
        .expect("the discard thread does not overflow its stack");
    removed.unwrap();
    assert!(!path.exists());
    assert_eq!(
        fs::read_dir(runtime.join(DISPOSABLE_DIR)).unwrap().count(),
        0
    );
}

#[test]
fn sweep_removes_layers_a_crashed_daemon_left() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = dir.path().join("runtime");
    let one = Layer::create_with(&runtime, TmpfsCheck::Skip).unwrap();
    let two = Layer::create_with(&runtime, TmpfsCheck::Skip).unwrap();
    tree_in(&one.upper());
    assert_ne!(one.dir(), two.dir());
    assert_eq!(sweep(&runtime), 2);
    assert_eq!(
        fs::read_dir(runtime.join(DISPOSABLE_DIR)).unwrap().count(),
        0
    );
}

#[test]
fn a_layer_is_refused_outside_tmpfs_and_in_a_shared_directory() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = dir.path().join("runtime");
    fs::create_dir_all(runtime.join(DISPOSABLE_DIR)).unwrap();
    fs::set_permissions(
        runtime.join(DISPOSABLE_DIR),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let err = Layer::create_with(&runtime, TmpfsCheck::Skip).unwrap_err();
    assert_eq!(err.code.as_str(), "SANDBOX_030");
    assert!(err.why.contains("private directory"), "{}", err.why);

    // Ob das temporäre Verzeichnis auf tmpfs liegt, entscheidet die Maschine;
    // geprüft wird, dass die Antwort zur Wahrheit passt.
    fs::set_permissions(
        runtime.join(DISPOSABLE_DIR),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let on_tmpfs = is_tmpfs(&rustix::fs::statfs(&runtime).unwrap());
    match Layer::create(&runtime) {
        Ok(_) => assert!(on_tmpfs),
        Err(err) => {
            assert!(!on_tmpfs);
            assert!(err.why.contains("not on tmpfs"), "{}", err.why);
        }
    }
}

/// Die Messung über `statvfs(2)` gilt dem ganzen Dateisystem. Was andere
/// Tests auf demselben tmpfs gleichzeitig löschen, verdeckt sonst ein
/// Wachstum; deshalb laufen diese Tests nacheinander und schreiben weit über
/// das Rauschen hinaus.
static STATVFS_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn statvfs_lock() -> std::sync::MutexGuard<'static, ()> {
    STATVFS_TESTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Schreibt `chunk` Bytes nach dem anderen, bis `rx` meldet oder die Frist
/// abläuft; höchstens 256 MiB.
fn write_until(dir: &Path, chunk: usize, rx: &std::sync::mpsc::Receiver<()>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut index = 0_usize;
    while Instant::now() < deadline {
        if rx.try_recv().is_ok() {
            return true;
        }
        if index * chunk < 256 * 1024 * 1024 {
            fs::write(dir.join(format!("chunk{index}")), vec![7_u8; chunk]).unwrap();
            index += 1;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Mutationsbeweis (C4, O4): Misst der Wächter seinen Ausgangsstand erst
/// beim eigenen Start statt beim Anlegen der Schicht, bleibt, was der Agent
/// dazwischen schrieb, ungezählt, und die Grenze greift nicht. Nach dem
/// Start des Wächters wird nichts mehr geschrieben; 64 MiB gegen 1 MiB
/// Grenze liegen weit über dem Rauschen anderer Prozesse.
#[test]
fn the_cap_counts_what_was_written_before_the_guard_started() {
    let _serial = statvfs_lock();
    let dir = tempfile::tempdir().unwrap();
    let layer = Layer::create_with(&dir.path().join("runtime"), TmpfsCheck::Skip)
        .unwrap()
        .with_budget(Budget {
            bytes: 1024 * 1024,
            inodes: 1_000_000,
        });
    // Der Agent schreibt, bevor die Messung anläuft.
    for index in 0..8 {
        fs::write(
            layer.upper().join(format!("early{index}")),
            vec![1_u8; 8 * 1024 * 1024],
        )
        .unwrap();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let guard = SizeGuard::start(&layer, move || {
        let _ = tx.send(());
    })
    .unwrap();
    rx.recv_timeout(Duration::from_secs(10))
        .expect("the early write counts against the cap");
    assert!(guard.tripped().is_some());
}

/// Mutationsbeweis: Mit einer Grenze, die nie greift, oder einem Wächter, der
/// nie misst, käme `on_exceed` nie.
#[test]
fn the_size_cap_holds() {
    let _serial = statvfs_lock();
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let budget = Budget {
        bytes: 1024 * 1024,
        inodes: 1_000_000,
    };
    let start = Usage::of(dir.path()).unwrap();
    let guard = SizeGuard::start_with(
        dir.path(),
        start,
        budget,
        Duration::from_millis(20),
        move || {
            let _ = tx.send(());
        },
    )
    .unwrap();
    assert!(guard.tripped().is_none());
    assert!(
        write_until(dir.path(), 8 * 1024 * 1024, &rx),
        "the guard stops the sandbox"
    );
    let tripped = guard.tripped().expect("the guard says why");
    assert_eq!(tripped.code.as_str(), "SANDBOX_032");
}

#[test]
fn the_inode_cap_holds() {
    let _serial = statvfs_lock();
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let budget = Budget {
        bytes: u64::MAX,
        inodes: 50,
    };
    let start = Usage::of(dir.path()).unwrap();
    let _guard = SizeGuard::start_with(
        dir.path(),
        start,
        budget,
        Duration::from_millis(20),
        move || {
            let _ = tx.send(());
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut index = 0_usize;
    let mut tripped = false;
    while Instant::now() < deadline && !tripped {
        for _ in 0..500 {
            if index < 100_000 {
                fs::write(dir.path().join(format!("f{index}")), "").unwrap();
                index += 1;
            }
        }
        tripped = rx.recv_timeout(Duration::from_millis(50)).is_ok();
    }
    assert!(tripped, "the guard counts entries");
}

#[test]
fn the_budget_is_at_most_half_of_what_is_free() {
    assert_eq!(Budget::for_free(100, 10).bytes, 50);
    assert_eq!(Budget::for_free(u64::MAX, u64::MAX).bytes, MAX_LAYER_BYTES);
    assert_eq!(
        Budget::for_free(u64::MAX, u64::MAX).inodes,
        MAX_LAYER_INODES
    );
}

#[test]
fn a_zombie_or_a_missing_process_is_not_alive() {
    let dir = tempfile::tempdir().unwrap();
    let proc_root = dir.path();
    for (pid, state) in [(10, "R"), (11, "S"), (12, "Z"), (13, "X")] {
        write_stat(proc_root, pid, state, 4242);
    }
    assert!(process_lives(proc_root, 10, None));
    assert!(process_lives(proc_root, 11, None));
    assert!(!process_lives(proc_root, 12, None));
    assert!(!process_lives(proc_root, 13, None));
    assert!(!process_lives(proc_root, 14, None));
}

/// Eine `stat`-Zeile wie die des Kernels: Feld 22 ist die Startzeit.
fn write_stat(proc_root: &Path, pid: u32, state: &str, start: u64) {
    let dir = proc_root.join(pid.to_string());
    fs::create_dir_all(&dir).unwrap();
    // Felder 4 bis 21 sind 18 Zahlen, dann Feld 22.
    let middle = vec!["0"; 18].join(" ");
    fs::write(
        dir.join("stat"),
        format!("{pid} (we) ird) name) {state} {middle} {start} 0 0\n"),
    )
    .unwrap();
}

/// Mutationsbeweis (O11): Ohne den Vergleich der Startzeit hielte ein neuer
/// Prozess, der die Nummer des Init-Prozesses wiederverwendet, die
/// Übernahme für immer an.
#[test]
fn a_reused_process_number_is_not_the_sandbox() {
    let dir = tempfile::tempdir().unwrap();
    write_stat(dir.path(), 20, "S", 1000);
    assert_eq!(start_ticks(dir.path(), 20), Some(1000));
    assert!(
        process_lives(dir.path(), 20, Some(1000)),
        "the same process"
    );
    assert!(!process_lives(dir.path(), 20, Some(999)), "a reused number");
}

#[test]
fn a_change_is_shown_sanitized_and_identified_by_its_raw_bytes() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("a\u{202e}txt.exe"), "x").unwrap();
    let set = fx.scan(&base);
    let change = &set.changes[0];
    assert!(
        !change.display().contains('\u{202e}'),
        "{}",
        change.display()
    );
    assert_eq!(change.path_hash().len(), 16);
    assert_eq!(change.what, Kind::File);
}

// --- Review Runde 1 (HUM-350) -------------------------------------------------------

/// Mutationsbeweis (O1): Mit der lexikalischen Prüfung `escapes` statt
/// `target_stays_inside` gehen beide Symlinks durch, und `e` zeigt über das
/// Projekt hinaus.
#[test]
fn a_symlink_pair_that_leads_out_is_refused() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("sub")).unwrap();
    symlink("..", fx.upper.join("sub/d")).unwrap();
    symlink("sub/d/..", fx.upper.join("e")).unwrap();
    let set = fx.scan(&base);
    assert!(
        change_at(&set, "sub/d").refused.is_none(),
        "`..` from sub stays inside"
    );
    let e = change_at(&set, "e");
    assert!(e.refused.is_some(), "{e:?}");
    let err = fx.apply(&base, &[], true).unwrap_err();
    assert_eq!(err.code.as_str(), "SANDBOX_031");
    assert!(fs::symlink_metadata(fx.lower.join("e")).is_err());

    for (link, target, inside) in [
        ("a", "b", true),
        ("x/a", "../b", true),
        ("x/a", "../../b", false),
        ("a", "b/../c", false),
        ("a", "d/..", false),
        ("a", "/etc", false),
        ("a", "./b", true),
        ("a", "", false),
    ] {
        assert_eq!(
            super::scan::target_stays_inside(Path::new(link), Path::new(target)),
            inside,
            "{link} -> {target}"
        );
    }
}

/// Mutationsbeweis (O2): Ohne `under_opaque` bleibt `build/a/old.o` stehen und
/// erscheint in keiner Liste.
#[test]
fn deletions_below_a_plain_directory_inside_an_opaque_one_are_seen() {
    let fx = fixture();
    fs::create_dir_all(fx.lower.join("build/a")).unwrap();
    fs::write(fx.lower.join("build/a/old.o"), "old").unwrap();
    fs::write(fx.lower.join("build/top.o"), "top").unwrap();
    let base = fx.baseline();
    // `rm -rf build && mkdir -p build/a && touch build/a/new.o`, wie overlayfs
    // es schreibt: `build` opak, `build/a` gewöhnlich.
    fs::create_dir_all(fx.upper.join("build/a")).unwrap();
    for dir in ["build", "build/a"] {
        fs::set_permissions(
            fx.upper.join(dir),
            fs::metadata(fx.lower.join(dir)).unwrap().permissions(),
        )
        .unwrap();
    }
    fs::write(fx.upper.join("build/a/new.o"), "").unwrap();
    if !opaque(&fx.upper.join("build")) {
        return;
    }
    let set = fx.scan(&base);
    assert_eq!(change_at(&set, "build/a/old.o").kind, ChangeKind::Removed);
    assert_eq!(change_at(&set, "build/top.o").kind, ChangeKind::Removed);
    assert_eq!(change_at(&set, "build/a/new.o").kind, ChangeKind::Added);
    fx.apply(&base, &[], false).unwrap();
    assert!(!fx.lower.join("build/a/old.o").exists());
    assert!(!fx.lower.join("build/top.o").exists());
    assert!(fx.lower.join("build/a/new.o").exists());
}

/// Mutationsbeweis (C2): Ohne die Prüfung unmittelbar vor `renameat`
/// überschriebe die Übernahme, was der Mensch während des Schreibens
/// änderte.
#[test]
fn a_file_changed_while_applying_stops_with_partial() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("edit.txt"), "agent\n").unwrap();
    let set = fx.scan(&base);
    let chosen = select(&set, &[]).unwrap();
    let (upper, lower) = fx.fds();
    // Die Vorprüfung sieht den Stand vom Start; danach, aber vor dem
    // Schreiben, ändert der Mensch die Datei. Hier simuliert: geprüft wird
    // gegen eine Erwartung, die nicht mehr stimmt.
    let expected = super::apply::Expected::observe(lower.as_fd(), chosen[0]).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    fs::write(fx.lower.join("edit.txt"), "human, while applying\n").unwrap();
    let mut modes = super::apply::DirModes::default();
    let result = super::apply::apply_one(
        upper.as_fd(),
        lower.as_fd(),
        chosen[0],
        &expected,
        &mut modes,
    );
    let why = result.unwrap_err();
    assert!(
        why.contains("while the changes were being applied"),
        "{why}"
    );
    assert_eq!(
        fs::read_to_string(fx.lower.join("edit.txt")).unwrap(),
        "human, while applying\n"
    );
}

/// Mutationsbeweis (C6): Mit `fchmod` auf den vorhandenen Inode änderte sich
/// auch der harte Verweis außerhalb des Projekts.
#[test]
fn a_mode_change_writes_a_new_inode_and_leaves_a_hard_link_outside_alone() {
    let fx = fixture();
    fs::set_permissions(fx.lower.join("keep.txt"), fs::Permissions::from_mode(0o644)).unwrap();
    fs::hard_link(fx.lower.join("keep.txt"), fx.outside.join("keep-link")).unwrap();
    let base = fx.baseline();
    fs::copy(fx.lower.join("keep.txt"), fx.upper.join("keep.txt")).unwrap();
    fs::set_permissions(fx.upper.join("keep.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    let set = fx.scan(&base);
    assert_eq!(change_at(&set, "keep.txt").kind, ChangeKind::ModeChanged);
    let report = fx.apply(&base, &[], false).unwrap();
    assert!(report.failed.is_none(), "{:?}", report.failed);
    assert_eq!(
        fs::metadata(fx.lower.join("keep.txt")).unwrap().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(fx.outside.join("keep-link")).unwrap().mode() & 0o777,
        0o644,
        "the hard link outside the project kept its mode"
    );
    assert_eq!(
        fs::read_to_string(fx.lower.join("keep.txt")).unwrap(),
        "keep\n"
    );
}

/// Mutationsbeweis (C7): Mit den endgültigen Rechten gleich beim Anlegen
/// scheitert das Schreiben in ein Verzeichnis, das der Agent auf `000`
/// stellte.
#[test]
fn a_directory_without_permissions_is_filled_before_its_mode_is_set() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir_all(fx.upper.join("locked/inner")).unwrap();
    fs::write(fx.upper.join("locked/inner/file"), "x").unwrap();
    fs::set_permissions(
        fx.upper.join("locked/inner"),
        fs::Permissions::from_mode(0o500),
    )
    .unwrap();
    fs::set_permissions(fx.upper.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let report = fx.apply(&base, &[], false).unwrap();
    assert!(report.failed.is_none(), "{:?}", report.failed);
    assert_eq!(
        fs::metadata(fx.lower.join("locked")).unwrap().mode() & 0o777,
        0o000
    );
    fs::set_permissions(fx.lower.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        fs::metadata(fx.lower.join("locked/inner")).unwrap().mode() & 0o777,
        0o500
    );
    assert_eq!(
        fs::read_to_string(fx.lower.join("locked/inner/file")).unwrap(),
        "x"
    );
    fs::set_permissions(
        fx.lower.join("locked/inner"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
}

/// Mutationsbeweis (C8): Ohne den Vergleich der Rechte von Verzeichnissen
/// verschwände ein Verzeichnis, dessen Rechte der Mensch seit dem Start
/// änderte, ohne Rückfrage.
#[test]
fn a_directory_whose_mode_the_user_changed_is_a_conflict() {
    let fx = fixture();
    let base = fx.baseline();
    if !whiteout(&fx.upper.join("dir")) {
        return;
    }
    fs::set_permissions(fx.lower.join("dir"), fs::Permissions::from_mode(0o750)).unwrap();
    let err = fx.apply(&base, &[], false).unwrap_err();
    assert!(
        err.why.contains("changed in the project after the start"),
        "{}",
        err.why
    );
    assert!(fx.lower.join("dir").exists());
}

/// Mutationsbeweis (C9, O3): Ohne die Prüfung an Dateien ginge eine Datei mit
/// `user.overlay.metacopy` durch.
#[test]
fn a_file_with_a_foreign_overlay_attribute_is_refused() {
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("meta.txt"), "x").unwrap();
    if rustix::fs::setxattr(
        fx.upper.join("meta.txt").as_path(),
        "user.overlay.metacopy",
        b"",
        rustix::fs::XattrFlags::empty(),
    )
    .is_err()
    {
        eprintln!("skipping: this file system refuses user xattrs");
        return;
    }
    let set = fx.scan(&base);
    let meta = change_at(&set, "meta.txt");
    assert!(
        meta.refused
            .as_deref()
            .is_some_and(|why| why.contains("metacopy")),
        "{meta:?}"
    );
}

/// Mutationsbeweis (O6): Mit dem alten Satz stünde nicht da, dass die
/// Grundlage abgeschnitten war und wie weit sie reichte.
#[test]
fn a_cut_baseline_says_why_every_addition_is_a_conflict() {
    let fx = fixture();
    let limits = crate::worktree::SnapshotLimits {
        max_entries: 1,
        ..baseline_limits()
    };
    let base = crate::worktree::snapshot(&fx.lower, &limits).unwrap();
    assert!(base.truncated());
    fs::write(fx.upper.join("new.txt"), "x").unwrap();
    let set = fx.scan(&base);
    let why = change_at(&set, "new.txt").conflict.unwrap();
    assert!(why.contains("cut short after 1 entries"), "{why}");
}

/// Mutationsbeweis (O7): Ohne `replaced_link` hieße die Ablehnung nur „a parent
/// of it in the project is a symbolic link".
#[test]
fn a_symlink_replaced_by_a_directory_is_named_in_the_refusal() {
    let fx = fixture();
    symlink("dir", fx.lower.join("link")).unwrap();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("link")).unwrap();
    fs::write(fx.upper.join("link/inside"), "x").unwrap();
    let set = fx.scan(&base);
    let why = change_at(&set, "link/inside").refused.unwrap();
    assert!(why.contains("replaced with a directory"), "{why}");
    assert!(!fx.lower.join("dir/inside").exists());
}

/// Mutationsbeweis (O8): Mit `chmodat` ohne Prüfung folgte die Änderung der
/// Rechte einem Symlink und träfe sein Ziel.
#[test]
fn changing_a_mode_never_follows_a_symlink() {
    let fx = fixture();
    fs::set_permissions(fx.outside.join("secret"), fs::Permissions::from_mode(0o600)).unwrap();
    symlink(fx.outside.join("secret"), fx.upper.join("link")).unwrap();
    let upper = open_root(&fx.upper).unwrap();
    let result =
        super::chmod_nofollow(upper.as_fd(), "link".as_ref(), 0o777, FileType::RegularFile);
    assert!(result.is_err());
    assert_eq!(
        fs::metadata(fx.outside.join("secret")).unwrap().mode() & 0o777,
        0o600
    );
    fs::create_dir(fx.upper.join("real")).unwrap();
    super::chmod_nofollow(upper.as_fd(), "real".as_ref(), 0o750, FileType::Directory).unwrap();
    assert_eq!(
        fs::metadata(fx.upper.join("real")).unwrap().mode() & 0o777,
        0o750
    );
}

/// Mutationsbeweis (O9): Ohne den Hinweis stünde im Befund nichts von den
/// Resten, die ein abgebrochenes Löschen im Projekt lässt.
#[test]
fn a_removal_that_fails_names_what_may_be_left() {
    let fx = fixture();
    fs::create_dir(fx.lower.join("ro")).unwrap();
    fs::write(fx.lower.join("ro/victim"), "v").unwrap();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("ro")).unwrap();
    fs::set_permissions(fx.upper.join("ro"), fs::Permissions::from_mode(0o755)).unwrap();
    if !whiteout(&fx.upper.join("ro/victim")) {
        return;
    }
    let set = fx.scan(&base);
    let victim = change_at(&set, "ro/victim");
    let chosen = vec![&victim];
    let (upper, lower) = fx.fds();
    // Der Mensch nimmt dem Verzeichnis das Schreibrecht; das Löschen scheitert.
    fs::set_permissions(fx.lower.join("ro"), fs::Permissions::from_mode(0o555)).unwrap();
    let report = apply(
        upper.as_fd(),
        lower.as_fd(),
        &base,
        &chosen,
        &ApplyOptions {
            overwrite: true,
            ..ApplyOptions::default()
        },
    )
    .unwrap();
    fs::set_permissions(fx.lower.join("ro"), fs::Permissions::from_mode(0o755)).unwrap();
    let (_, diagnostic) = report.failed.expect("the removal fails");
    assert!(
        diagnostic.why.contains(".humanitl-flat-"),
        "{}",
        diagnostic.why
    );
}

// --- Review Runde 2 (HUM-350) -------------------------------------------------------

/// Mutationsbeweis (Runde 2, Punkt 2): Löscht `remove` den Teilbaum ohne
/// Vergleich, verschwindet eine Datei, die der Mensch nach der Vorprüfung
/// änderte, und eine, die er neu anlegte.
#[test]
fn removing_a_directory_stops_at_what_the_user_changed_during_apply() {
    for (case, human) in [("edited", "dir/sub/b"), ("created", "dir/sub/new")] {
        let fx = fixture();
        let base = fx.baseline();
        if !whiteout(&fx.upper.join("dir")) {
            return;
        }
        let set = fx.scan(&base);
        let chosen = select(&set, &[]).unwrap();
        let (upper, lower) = fx.fds();
        let expected = super::apply::Expected::observe(lower.as_fd(), chosen[0]).unwrap();
        // Nach der Vorprüfung, vor dem Löschen: der Mensch arbeitet unten.
        std::thread::sleep(Duration::from_millis(20));
        fs::write(fx.lower.join(human), "human, longer than before\n").unwrap();
        let mut modes = super::apply::DirModes::default();
        let why = super::apply::apply_one(
            upper.as_fd(),
            lower.as_fd(),
            chosen[0],
            &expected,
            &mut modes,
        )
        .unwrap_err();
        assert!(why.contains("below it changed"), "{case}: {why}");
        assert_eq!(
            fs::read_to_string(fx.lower.join(human)).unwrap(),
            "human, longer than before\n",
            "{case}: the human's file stays"
        );
    }
}

/// Mutationsbeweis (Runde 2, Punkt 3): Ohne den Vergleich in
/// `DirModes::finish` bekäme ein Verzeichnis, dessen Rechte der Mensch
/// inzwischen änderte, trotzdem die Rechte aus der oberen Schicht.
#[test]
fn a_deferred_mode_is_not_set_on_a_directory_that_changed() {
    let fx = fixture();
    let lower = open_root(&fx.lower).unwrap();
    let mut modes = super::apply::DirModes::default();
    modes
        .later(lower.as_fd(), Path::new("dir/sub"), 0o700)
        .unwrap();
    fs::set_permissions(fx.lower.join("dir/sub"), fs::Permissions::from_mode(0o750)).unwrap();
    let (rel, why) = modes.finish(lower.as_fd()).unwrap_err();
    assert_eq!(rel, Path::new("dir/sub"));
    assert!(
        why.contains("while the changes were being applied"),
        "{why}"
    );
    assert_eq!(
        fs::metadata(fx.lower.join("dir/sub")).unwrap().mode() & 0o777,
        0o750
    );

    // Unverändert geht es durch.
    let mut modes = super::apply::DirModes::default();
    modes.later(lower.as_fd(), Path::new("dir"), 0o711).unwrap();
    modes.finish(lower.as_fd()).unwrap();
    assert_eq!(
        fs::metadata(fx.lower.join("dir")).unwrap().mode() & 0o777,
        0o711
    );
    fs::set_permissions(fx.lower.join("dir"), fs::Permissions::from_mode(0o755)).unwrap();
}

/// Mutationsbeweis (Runde 2, Punkt 4): Mit `renameat` ohne
/// `RENAME_NOREPLACE` ersetzt das Umhängen ein leeres Verzeichnis gleichen
/// Namens, und das Verwerfen löscht es danach mit.
#[test]
fn flattening_never_replaces_an_existing_directory() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = dir.path().join("runtime");
    let layer = Layer::create_with(&runtime, TmpfsCheck::Skip).unwrap();
    let mut deep = layer.upper();
    for _ in 0..(MAX_DEPTH + 5) {
        deep = deep.join("d");
    }
    fs::create_dir_all(&deep).unwrap();
    // Liegt schon da, wo das erste Umhängen hin will.
    let taken = runtime
        .join(DISPOSABLE_DIR)
        .join(format!(".humanitl-flat-{}-1", std::process::id()));
    fs::create_dir(&taken).unwrap();
    layer
        .discard()
        .map_err(|(_, diagnostic)| diagnostic)
        .unwrap();
    assert!(taken.is_dir(), "the existing directory was not replaced");
}

/// Mutationsbeweis (Runde 2, Punkt 5): Ohne die Zählung der verdeckten
/// Einträge bliebe die Liste trotz überschrittenen Budgets „vollständig".
#[test]
fn hidden_deletions_count_against_the_budget() {
    let fx = fixture();
    for name in ["a", "b"] {
        fs::create_dir(fx.lower.join(name)).unwrap();
        fs::write(fx.lower.join(name).join("x"), "x").unwrap();
        fs::write(fx.lower.join(name).join("y"), "y").unwrap();
    }
    let base = fx.baseline();
    for name in ["a", "b"] {
        fs::create_dir(fx.upper.join(name)).unwrap();
        fs::set_permissions(
            fx.upper.join(name),
            fs::metadata(fx.lower.join(name)).unwrap().permissions(),
        )
        .unwrap();
        if !opaque(&fx.upper.join(name)) {
            return;
        }
    }
    let (upper, lower) = fx.fds();
    let set = super::scan::scan_with_budget(upper.as_fd(), lower.as_fd(), &base, 4);
    assert!(
        set.truncated,
        "two names above and four hidden ones exceed 4: {set:#?}"
    );
    let roomy = super::scan::scan_with_budget(upper.as_fd(), lower.as_fd(), &base, 100);
    assert!(!roomy.truncated);
}

// --- Review Runde 3 (HUM-350) -------------------------------------------------------

/// Mutationsbeweis (Runde 3): Speichert `Expected::observe` bei einem
/// unlesbaren Teilbaum still `None`, löscht die Übernahme den Ordner ohne
/// Vergleich je Eintrag und erzwingt sich dafür den Zugang; die Datei des
/// Menschen darunter wäre weg. Mit `--overwrite`, damit die Konfliktprüfung
/// das Rennen aus dem Befund nicht vorwegnimmt.
#[test]
fn an_unreadable_directory_is_never_removed_unchecked() {
    let fx = fixture();
    let base = fx.baseline();
    if !whiteout(&fx.upper.join("dir")) {
        return;
    }
    fs::write(fx.lower.join("dir/sub/b"), "human\n").unwrap();
    fs::set_permissions(fx.lower.join("dir/sub"), fs::Permissions::from_mode(0o000)).unwrap();
    let result = fx.apply(&base, &[], true);
    fs::set_permissions(fx.lower.join("dir/sub"), fs::Permissions::from_mode(0o755)).unwrap();
    let err = result.unwrap_err();
    assert!(err.why.contains("nothing was written"), "{}", err.why);
    assert!(err.why.contains("cannot be read completely"), "{}", err.why);
    assert_eq!(
        fs::read_to_string(fx.lower.join("dir/sub/b")).unwrap(),
        "human\n"
    );
}

// --- Review Runde 4 (HUM-350) -------------------------------------------------------

/// Mutationsbeweis (Runde 4): Mit dem rekursiven `remove_tree_at` für eine
/// Erwartung ohne Baum löschte das Entfernen einer Datei ein Verzeichnis, das
/// der Mensch inzwischen an ihre Stelle legte, samt Inhalt und ungeprüft.
#[test]
fn removing_a_file_never_deletes_a_directory_that_took_its_place() {
    let fx = fixture();
    let base = fx.baseline();
    if !whiteout(&fx.upper.join("keep.txt")) {
        return;
    }
    let set = fx.scan(&base);
    let change = change_at(&set, "keep.txt");
    let lower = open_root(&fx.lower).unwrap();
    let expected = super::apply::Expected::observe(lower.as_fd(), &change).unwrap();
    // Nach der Prüfung: aus der Datei wird ein Verzeichnis mit Inhalt.
    fs::remove_file(fx.lower.join("keep.txt")).unwrap();
    fs::create_dir(fx.lower.join("keep.txt")).unwrap();
    fs::write(fx.lower.join("keep.txt/notes"), "human\n").unwrap();
    let why = super::apply::remove(&lower, "keep.txt".as_ref(), &expected).unwrap_err();
    assert!(why.contains("left as it is"), "{why}");
    assert_eq!(
        fs::read_to_string(fx.lower.join("keep.txt/notes")).unwrap(),
        "human\n"
    );
}

/// Ein `SANDBOX_030`, wie ihn die Probe für ein altes bubblewrap liefert.
fn old_bwrap() -> Diagnostic {
    Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        .why("/usr/bin/bwrap is bubblewrap 0.9.0; a disposable sandbox needs --overlay")
        .fix(FixAction::CopyCommand("install bubblewrap".to_owned()))
        .build()
}

/// Die Vorgabe `auto` ist eine Wegwerf-Sandbox, wo das System sie kann
/// (HUM-357).
///
/// Mutationsbeweis: `auto` wieder als `direct` (der Arm `Auto` liefert
/// `ModeChoice::direct(None)`) ist rot.
#[test]
fn the_default_resolves_to_disposable_where_the_system_allows_it() {
    assert_eq!(SandboxMode::default(), SandboxMode::Auto);
    let choice = resolve_mode(SandboxMode::default(), || Ok(())).unwrap();
    assert_eq!(choice.mode, SandboxMode::Disposable);
    assert!(choice.notice.is_none());
    assert!(choice.is_disposable());
}

/// Ohne Overlay fällt die Vorgabe auf `direct` zurück und sagt es: ein
/// Hinweis `SANDBOX_033`, nicht blockierend, mit dem Grund und dessen Fix
/// (HUM-357).
///
/// Mutationsbeweis: Rückfall entfernt (der Fehler der Probe geht durch) ist
/// rot.
#[test]
fn the_default_falls_back_to_direct_with_a_notice() {
    let choice = resolve_mode(SandboxMode::Auto, || Err(old_bwrap()))
        .expect("the default never refuses for want of an overlay");
    assert_eq!(choice.mode, SandboxMode::Direct);
    let notice = choice.notice.expect("a notice");
    assert_eq!(notice.code.as_str(), "SANDBOX_033");
    assert_eq!(notice.severity, Severity::Warning);
    assert!(notice.why.contains("bubblewrap 0.9.0"), "{}", notice.why);
    assert!(notice.why.contains("directly"), "{}", notice.why);
    assert!(
        notice.why.contains("sandbox.mode = \"direct\""),
        "{}",
        notice.why
    );
    assert_eq!(
        notice.fix,
        Some(FixAction::CopyCommand("install bubblewrap".to_owned()))
    );
}

/// Ein ausdrückliches `disposable` fällt nie zurück (HUM-357): Wo es nicht
/// geht, lehnt der Start mit dem `SANDBOX_030` der Probe ab.
///
/// Mutationsbeweis: `disposable` wie `auto` behandelt (still `direct`) ist
/// rot.
#[test]
fn an_explicit_disposable_refuses_instead_of_falling_back() {
    let err = resolve_mode(SandboxMode::Disposable, || Err(old_bwrap()))
        .expect_err("an explicit choice is not overruled");
    assert_eq!(err.code.as_str(), "SANDBOX_030");
    let choice = resolve_mode(SandboxMode::Disposable, || Ok(())).unwrap();
    assert!(choice.is_disposable());
    assert!(choice.notice.is_none());
}

/// `direct` fragt die Probe gar nicht (HUM-357).
#[test]
fn direct_never_probes() {
    let choice = resolve_mode(SandboxMode::Direct, || {
        panic!("direct must not probe the overlay")
    })
    .unwrap();
    assert_eq!(choice.mode, SandboxMode::Direct);
    assert!(choice.notice.is_none());
}

/// Overlay ohne Rechte gibt es ab Linux 5.11 (HUM-357, Review).
///
/// Mutationsbeweis: Schwelle auf 5.10, rot.
#[test]
fn the_kernel_must_be_5_11_or_newer_for_an_unprivileged_overlay() {
    assert!(!kernel_allows_overlay("5.10.0-28-amd64"));
    assert!(kernel_allows_overlay("5.11.0"));
    assert!(kernel_allows_overlay("6.1.0-13-amd64\n"));
    assert!(!kernel_allows_overlay("4.19.0"));
    assert!(!kernel_allows_overlay(""));
    assert!(!kernel_allows_overlay("garbage"));
}

/// Ein Projektordner, den der Daemon nicht lesen kann (Rechte `030`), unter
/// einem opaken Verzeichnis der Schicht: Die verdeckten Löschungen sind dann
/// unbekannt, und die Liste muss das sagen, statt „nichts geändert" zu
/// melden, das die Schicht ungesehen verwerfen ließe (HUM-357, Review
/// Runde 2).
///
/// Mutationsbeweis: `opaque_removals` kehrt beim Öffnen wieder still zurück,
/// rot.
#[test]
fn an_unreadable_lower_directory_under_an_opaque_one_makes_the_list_incomplete() {
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("dir")).unwrap();
    fs::set_permissions(
        fx.upper.join("dir"),
        fs::metadata(fx.lower.join("dir")).unwrap().permissions(),
    )
    .unwrap();
    if !opaque(&fx.upper.join("dir")) {
        return;
    }
    let mode = fs::metadata(fx.lower.join("dir")).unwrap().permissions();
    fs::set_permissions(fx.lower.join("dir"), fs::Permissions::from_mode(0o030)).unwrap();
    if fs::read_dir(fx.lower.join("dir")).is_ok() {
        fs::set_permissions(fx.lower.join("dir"), mode).unwrap();
        eprintln!("skipping: this user reads a directory with mode 030 (root?)");
        return;
    }
    let set = fx.scan(&base);
    fs::set_permissions(fx.lower.join("dir"), mode).unwrap();
    assert!(
        set.truncated,
        "an unreadable lower directory is not an empty one: {set:?}"
    );
    assert!(
        set.incomplete
            .iter()
            .any(|why| why.contains("dir") && why.contains("cannot be read")),
        "{:?}",
        set.incomplete
    );
}

/// Ein Lesefehler am Opak-Attribut (etwa `EIO`) heißt nicht „nicht opak"
/// (HUM-357, Review Runde 3): Ein gelöschtes und leer wieder angelegtes
/// Verzeichnis sähe sonst unverändert aus, die Liste wäre leer und
/// vollständig, und die Schicht ginge als `no_changes`.
///
/// Mutationsbeweis: `is_opaque` wieder mit jedem Fehler `Ok(false)`, rot.
#[test]
fn a_failed_read_of_the_opaque_marker_makes_the_list_incomplete() {
    fn broken(fd: &OwnedFd, name: &str, buf: &mut [u8]) -> Result<usize, rustix::io::Errno> {
        if name == "user.overlay.opaque" {
            return Err(rustix::io::Errno::IO);
        }
        rustix::fs::fgetxattr(fd, name, buf)
    }
    let fx = fixture();
    let base = fx.baseline();
    fs::create_dir(fx.upper.join("dir")).unwrap();
    fs::set_permissions(
        fx.upper.join("dir"),
        fs::metadata(fx.lower.join("dir")).unwrap().permissions(),
    )
    .unwrap();
    let (upper, lower) = fx.fds();
    let set = super::scan::scan_with(
        upper.as_fd(),
        lower.as_fd(),
        &base,
        super::scan::MAX_SCAN_ENTRIES,
        broken,
    );
    assert!(
        set.truncated,
        "an unknown opaque marker is not an absent one: {set:?}"
    );
    assert!(
        set.incomplete
            .iter()
            .any(|why| why.contains("dir") && why.contains("hides")),
        "{:?}",
        set.incomplete
    );
    assert!(
        set.changes
            .iter()
            .all(|change| change.kind != ChangeKind::Removed),
        "no deletions are invented: {set:?}"
    );
}

/// Ein fremdes Overlay-Attribut, das sich nicht lesen lässt, zählt als
/// vorhanden, und die Liste ist unvollständig (HUM-357, Review Runde 3).
///
/// Mutationsbeweis: `has_xattr` wieder mit jedem Fehler `Ok(false)`, rot.
#[test]
fn a_failed_read_of_a_foreign_attribute_refuses_and_makes_the_list_incomplete() {
    fn broken(_fd: &OwnedFd, name: &str, _buf: &mut [u8]) -> Result<usize, rustix::io::Errno> {
        if name == "user.overlay.redirect" {
            return Err(rustix::io::Errno::IO);
        }
        Err(rustix::io::Errno::NODATA)
    }
    let fx = fixture();
    let base = fx.baseline();
    fs::write(fx.upper.join("new.txt"), "new\n").unwrap();
    let (upper, lower) = fx.fds();
    let set = super::scan::scan_with(
        upper.as_fd(),
        lower.as_fd(),
        &base,
        super::scan::MAX_SCAN_ENTRIES,
        broken,
    );
    assert!(set.truncated, "{set:?}");
    let new = set
        .changes
        .iter()
        .find(|change| change.rel == Path::new("new.txt"))
        .expect("listed");
    assert!(new.refused.is_some(), "{new:?}");
}
