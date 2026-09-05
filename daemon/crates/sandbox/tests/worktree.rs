//! Schnappschuss, Diff und sichere Pfadauflösung über einen echten Baum
//! (HUM-043).
//!
//! Die Tests hier fassen das Dateisystem an, weil genau das der Punkt ist: Die
//! Garantie von [`humanitl_sandbox::worktree::open_beneath`] ist eine über den
//! Kernel, nicht über Rust-Typen. Ein Test, der sie ohne Symlink prüft, prüft
//! nichts.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::fd::AsFd;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use humanitl_sandbox::worktree::{
    Entry, FileChange, Kind, MAX_DEPTH, RESOLVE_FLAGS, Resolution, SnapshotLimits, TreeSnapshot,
    diff, escapes, open_beneath, open_beneath_with, open_root, read_beneath, resolution, snapshot,
};
use rustix::fs::{Mode, OFlags, ResolveFlags};
use rustix::io::Errno;

/// Ein Baum, der bei jedem Test frisch entsteht.
struct Tree {
    dir: tempfile::TempDir,
}

impl Tree {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(&path, content).expect("write");
        path
    }

    fn mkdir(&self, rel: &str) -> PathBuf {
        let path = self.dir.path().join(rel);
        fs::create_dir_all(&path).expect("mkdir");
        path
    }

    fn symlink(&self, rel: &str, target: &str) {
        let path = self.dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        std::os::unix::fs::symlink(target, &path).expect("symlink");
    }

    fn take(&self) -> TreeSnapshot {
        snapshot(self.root(), &SnapshotLimits::default()).expect("snapshot")
    }
}

fn entry<'a>(snapshot: &'a TreeSnapshot, rel: &str) -> &'a Entry {
    snapshot
        .get(Path::new(rel))
        .unwrap_or_else(|| panic!("{rel} is not in the snapshot"))
}

// --- der Schnappschuss --------------------------------------------------------

#[test]
fn snapshot_skips_node_modules_and_git_objects() {
    let tree = Tree::new();
    tree.write("src/main.rs", "fn main() {}\n");
    tree.write("node_modules/left-pad/index.js", "module.exports = 1;\n");
    tree.write("deep/node_modules/x/index.js", "1\n");
    tree.write(".git/objects/ab/cdef", "binary");
    tree.write(".git/HEAD", "ref: refs/heads/main\n");
    tree.write(".git/refs/heads/main", "0000\n");
    tree.write("target/debug/thing", "elf");
    tree.write(".venv/pyvenv.cfg", "x\n");
    tree.write("__pycache__/x.pyc", "x");
    tree.write(".cache/x", "x");

    let snap = tree.take();
    let paths: Vec<String> = snap
        .entries()
        .keys()
        .map(|path| path.display().to_string())
        .collect();

    assert!(paths.contains(&"src/main.rs".to_owned()), "{paths:?}");
    assert!(paths.contains(&".git/HEAD".to_owned()), "{paths:?}");
    assert!(
        paths.contains(&".git/refs/heads/main".to_owned()),
        "{paths:?}"
    );
    for skipped in [
        "node_modules",
        "deep/node_modules",
        ".git/objects",
        "target",
        ".venv",
        "__pycache__",
        ".cache",
    ] {
        assert!(
            !paths.iter().any(|path| path.starts_with(skipped)),
            "{skipped} is in the snapshot: {paths:?}"
        );
    }
    assert!(!snap.truncated(), "nothing here hits a budget");
}

#[test]
fn snapshot_hashes_small_files_and_reads_symlinks_without_following_them() {
    let tree = Tree::new();
    tree.write("a.txt", "hello\n");
    tree.symlink("out", "/etc/passwd");

    let snap = tree.take();
    assert_eq!(entry(&snap, "a.txt").kind, Kind::File);
    assert!(
        entry(&snap, "a.txt").hash.is_some(),
        "small files are hashed"
    );
    assert_eq!(entry(&snap, "a.txt").size, 6);
    assert_eq!(
        entry(&snap, "out").kind,
        Kind::Symlink {
            target: PathBuf::from("/etc/passwd")
        },
        "the target is read, never followed"
    );
    assert!(
        snap.get(Path::new("out/root")).is_none(),
        "the walk does not descend into a symlink"
    );
}

#[test]
fn snapshot_stops_at_the_entry_budget() {
    let tree = Tree::new();
    for n in 0..20 {
        tree.write(&format!("f{n}.txt"), "x");
    }
    let limits = SnapshotLimits {
        max_entries: 5,
        ..SnapshotLimits::default()
    };
    let snap = snapshot(tree.root(), &limits).expect("snapshot");
    assert!(snap.len() <= 5, "{} entries", snap.len());
    assert!(snap.truncated(), "the budget must show up in the flag");
}

#[test]
fn snapshot_stops_at_the_depth_budget() {
    let tree = Tree::new();
    let mut rel = String::from("d");
    for _ in 0..(MAX_DEPTH + 2) {
        tree.mkdir(&rel);
        rel.push_str("/d");
    }
    let snap = tree.take();
    assert!(
        snap.truncated(),
        "a tree deeper than MAX_DEPTH is truncated"
    );
}

#[test]
fn a_file_over_the_hash_cap_keeps_no_hash() {
    let tree = Tree::new();
    tree.write("big.bin", &"x".repeat(64));
    let limits = SnapshotLimits {
        hash_max_bytes: 8,
        ..SnapshotLimits::default()
    };
    let snap = snapshot(tree.root(), &limits).expect("snapshot");
    assert!(entry(&snap, "big.bin").hash.is_none());
    assert_eq!(entry(&snap, "big.bin").size, 64);
}

// --- der Diff ------------------------------------------------------------------

#[test]
fn diff_detects_added_modified_removed() {
    let tree = Tree::new();
    tree.write("keep.txt", "same\n");
    tree.write("change.txt", "before\n");
    tree.write("gone.txt", "bye\n");
    let before = tree.take();

    tree.write("new.txt", "fresh\n");
    tree.write("change.txt", "after\n");
    fs::remove_file(tree.root().join("gone.txt")).expect("remove");
    let after = tree.take();

    let changes = diff(&before, &after);
    assert!(
        changes.contains(&FileChange::Added(PathBuf::from("new.txt"))),
        "{changes:?}"
    );
    assert!(
        changes.contains(&FileChange::Modified(PathBuf::from("change.txt"))),
        "{changes:?}"
    );
    assert!(
        changes.contains(&FileChange::Removed(PathBuf::from("gone.txt"))),
        "{changes:?}"
    );
    assert!(
        !changes
            .iter()
            .any(|change| change.path() == Path::new("keep.txt")),
        "an untouched file is not a change: {changes:?}"
    );
}

#[test]
fn diff_sees_a_rewrite_that_keeps_the_mtime() {
    let tree = Tree::new();
    let path = tree.write("touched.txt", "before");
    let before = tree.take();
    let stamp = fs::metadata(&path)
        .expect("metadata")
        .modified()
        .expect("mtime");
    fs::write(&path, "after!").expect("rewrite");
    filetime_set(&path, stamp);
    let after = tree.take();

    let changes = diff(&before, &after);
    assert!(
        changes.contains(&FileChange::Modified(PathBuf::from("touched.txt"))),
        "the hash decides, not the mtime: {changes:?}"
    );
}

/// Setzt die `mtime` einer Datei zurück, ohne eine weitere Abhängigkeit.
fn filetime_set(path: &Path, when: std::time::SystemTime) {
    let file = fs::File::options().write(true).open(path).expect("open");
    let times = rustix::fs::Timestamps {
        last_access: rustix::fs::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
        last_modification: to_timespec(when),
    };
    let mut times = times;
    times.last_access = times.last_modification;
    rustix::fs::futimens(file.as_fd(), &times).expect("futimens");
}

fn to_timespec(when: std::time::SystemTime) -> rustix::fs::Timespec {
    let since = when
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch");
    rustix::fs::Timespec {
        tv_sec: i64::try_from(since.as_secs()).expect("seconds fit"),
        tv_nsec: i64::from(since.subsec_nanos()),
    }
}

#[test]
fn diff_detects_mode_change() {
    let tree = Tree::new();
    let path = tree.write("script.sh", "#!/bin/sh\necho hi\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
    let before = tree.take();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    let after = tree.take();

    let changes = diff(&before, &after);
    assert!(
        changes.contains(&FileChange::ModeChanged(PathBuf::from("script.sh"))),
        "{changes:?}"
    );
    assert!(
        !changes.contains(&FileChange::Modified(PathBuf::from("script.sh"))),
        "the content did not change: {changes:?}"
    );
}

#[test]
fn diff_marks_a_new_symlink_and_says_whether_it_escapes() {
    let tree = Tree::new();
    tree.mkdir("sub");
    let before = tree.take();
    tree.symlink("out", "/etc");
    tree.symlink("sub/up", "../..");
    tree.symlink("sub/here", "../keep.txt");
    let after = tree.take();

    let changes = diff(&before, &after);
    assert!(
        changes.contains(&FileChange::SymlinkAdded {
            path: PathBuf::from("out"),
            target: PathBuf::from("/etc"),
            escapes: true,
        }),
        "{changes:?}"
    );
    assert!(
        changes.contains(&FileChange::SymlinkAdded {
            path: PathBuf::from("sub/up"),
            target: PathBuf::from("../.."),
            escapes: true,
        }),
        "{changes:?}"
    );
    assert!(
        changes.contains(&FileChange::SymlinkAdded {
            path: PathBuf::from("sub/here"),
            target: PathBuf::from("../keep.txt"),
            escapes: false,
        }),
        "{changes:?}"
    );
}

// --- die lexikalische Antwort auf „führt hinaus?" -------------------------------

#[test]
fn symlink_escape_absolute() {
    assert!(escapes(Path::new("link"), Path::new("/etc/passwd")));
    assert!(escapes(Path::new("a/b/link"), Path::new("/")));
}

#[test]
fn symlink_escape_dotdot() {
    assert!(escapes(Path::new("link"), Path::new("../outside")));
    assert!(escapes(Path::new("a/link"), Path::new("../../outside")));
    assert!(escapes(Path::new("a/b/link"), Path::new("../../../x")));
}

#[test]
fn symlink_inside_ok() {
    assert!(!escapes(Path::new("link"), Path::new("target.txt")));
    assert!(!escapes(Path::new("a/link"), Path::new("../a/target.txt")));
    assert!(!escapes(Path::new("a/b/link"), Path::new("../c/d")));
    assert!(!escapes(Path::new("a/link"), Path::new("./sibling")));
}

// --- die Garantie des Kernels ---------------------------------------------------

#[test]
fn openat2_refuses_symlink_traversal() {
    let tree = Tree::new();
    tree.write("inside.txt", "ok\n");
    tree.symlink("a", "/etc");
    tree.mkdir("sub");
    tree.symlink("sub/up", "..");

    let root = open_root(tree.root()).expect("root opens");

    // Was ohne die Flags gelesen würde: /etc/passwd.
    let err = open_beneath(
        root.as_fd(),
        Path::new("a/passwd"),
        OFlags::RDONLY | OFlags::CLOEXEC,
    )
    .expect_err("a/passwd must not open");
    assert!(
        matches!(err, Errno::XDEV | Errno::LOOP | Errno::NOTDIR),
        "unexpected errno {err:?}"
    );

    // Ein Symlink auf ein Verzeichnis über /work.
    let err = open_beneath(
        root.as_fd(),
        Path::new("sub/up/inside.txt"),
        OFlags::RDONLY | OFlags::CLOEXEC,
    )
    .expect_err("sub/up/inside.txt must not open");
    assert!(
        matches!(err, Errno::XDEV | Errno::LOOP | Errno::NOTDIR),
        "unexpected errno {err:?}"
    );

    // Ein Pfad mit `..`, ganz ohne Symlink.
    let err = open_beneath(
        root.as_fd(),
        Path::new("sub/../../etc/passwd"),
        OFlags::RDONLY | OFlags::CLOEXEC,
    )
    .expect_err(".. must not open");
    assert_eq!(err, Errno::XDEV);

    // Und der Kernel selbst, ohne die lexikalische Vorprüfung dazwischen:
    // dieselbe Antwort, aus derselben Ursache.
    if resolution() == Resolution::Openat2 {
        let raw = rustix::fs::openat2(
            root.as_fd(),
            "a/passwd",
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        );
        let err = raw.expect_err("the kernel refuses the traversal itself");
        assert!(
            matches!(err, Errno::XDEV | Errno::LOOP),
            "unexpected errno {err:?}"
        );
    }

    // Was drin liegt, geht weiter auf.
    let bytes = read_beneath(root.as_fd(), Path::new("inside.txt"), 1024).expect("inside opens");
    assert_eq!(bytes, b"ok\n");
}

#[test]
fn read_beneath_stops_at_the_cap() {
    let tree = Tree::new();
    tree.write("big.txt", "0123456789");
    let root = open_root(tree.root()).expect("root opens");
    let bytes = read_beneath(root.as_fd(), Path::new("big.txt"), 4).expect("read");
    assert_eq!(bytes, b"0123");
}

/// Die drei Flags sind die Garantie, nicht eine Konstante.
///
/// `is_beneath` fängt `..` schon vorher ab und verdeckt damit, wenn
/// `RESOLVE_BENEATH` fehlte; `RESOLVE_NO_MAGICLINKS` hat gar keinen Fall im
/// Baum. Deshalb steht hier die Aussage selbst.
#[test]
fn the_resolve_flags_are_beneath_no_symlinks_and_no_magiclinks() {
    assert!(
        RESOLVE_FLAGS.contains(ResolveFlags::BENEATH),
        "{RESOLVE_FLAGS:?}"
    );
    assert!(
        RESOLVE_FLAGS.contains(ResolveFlags::NO_SYMLINKS),
        "{RESOLVE_FLAGS:?}"
    );
    assert!(
        RESOLVE_FLAGS.contains(ResolveFlags::NO_MAGICLINKS),
        "{RESOLVE_FLAGS:?}"
    );
}

/// Derselbe Baum durch beide Wege: `openat2` und der Fallback für Kernel < 5.6.
///
/// [`resolution`] entscheidet einmal je Prozess, und auf diesem Kernel fiele
/// [`Resolution::PerComponent`] sonst nie an. Eine Garantie, die kein Test
/// ausführt, ist keine.
#[test]
fn both_resolutions_refuse_the_same_tree() {
    let tree = Tree::new();
    tree.write("inside.txt", "ok\n");
    tree.symlink("a", "/etc");
    tree.mkdir("sub");
    tree.symlink("sub/up", "..");
    let root = open_root(tree.root()).expect("root opens");

    for how in [Resolution::Openat2, Resolution::PerComponent] {
        for rel in ["a/passwd", "sub/up/inside.txt", "sub/../../etc/passwd"] {
            let err = open_beneath_with(
                root.as_fd(),
                Path::new(rel),
                OFlags::RDONLY | OFlags::CLOEXEC,
                how,
            )
            .err()
            .unwrap_or_else(|| panic!("{rel} must not open under {}", how.as_str()));
            assert!(
                matches!(err, Errno::XDEV | Errno::LOOP | Errno::NOTDIR),
                "{rel} under {}: unexpected errno {err:?}",
                how.as_str()
            );
        }

        // Und was drin liegt, geht auf beiden Wegen auf.
        let fd = open_beneath_with(
            root.as_fd(),
            Path::new("inside.txt"),
            OFlags::RDONLY | OFlags::CLOEXEC,
            how,
        )
        .unwrap_or_else(|err| panic!("inside.txt under {}: {err:?}", how.as_str()));
        drop(fd);
    }
}

/// Der Fallback öffnet ein Verzeichnis in der Mitte nie über einen Symlink.
///
/// Ohne `O_NOFOLLOW` an den Zwischenschritten läge hier `/etc/passwd` offen.
#[test]
fn the_fallback_never_follows_a_directory_symlink() {
    let outside = tempfile::tempdir().expect("tempdir");
    fs::write(outside.path().join("secret.txt"), b"host\n").expect("write");
    let tree = Tree::new();
    tree.symlink("link", outside.path().to_str().expect("utf-8"));

    let root = open_root(tree.root()).expect("root opens");
    let err = open_beneath_with(
        root.as_fd(),
        Path::new("link/secret.txt"),
        OFlags::RDONLY | OFlags::CLOEXEC,
        Resolution::PerComponent,
    )
    .expect_err("the fallback must refuse the symlinked directory");
    assert!(
        matches!(err, Errno::LOOP | Errno::NOTDIR | Errno::XDEV),
        "unexpected errno {err:?}"
    );
}

// --- Maßstab --------------------------------------------------------------------

/// Ein Projekt mit 50 000 Dateien wird in unter fünf Sekunden erfasst.
///
/// `#[ignore]`, weil der Baum Sekunden zum Anlegen braucht:
/// `cargo test -p humanitl-sandbox --test worktree -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark: builds 50 000 files"]
fn snapshot_of_fifty_thousand_files_stays_under_five_seconds() {
    let tree = Tree::new();
    for dir in 0..250 {
        let parent = tree.mkdir(&format!("d{dir:03}"));
        for file in 0..200 {
            fs::write(parent.join(format!("f{file:03}.txt")), b"content\n").expect("write");
        }
    }
    let started = Instant::now();
    let snap = tree.take();
    let elapsed = started.elapsed();
    println!("{} entries in {elapsed:?}", snap.len());
    assert!(snap.len() >= 50_000, "{} entries", snap.len());
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "the snapshot took {elapsed:?}"
    );
}
