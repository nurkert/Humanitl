//! `scan_clone` und die Übernahme über einem Klon (HUM-374). Der Klon ist hier
//! eine Kopie mit erhaltenen Zeiten, damit die Tests auf jedem System laufen;
//! der echte `clonefile` steht darunter, nur unter macOS.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::PathBuf;
use std::process::Command;

use crate::summary::ChangeKind;
use crate::worktree::{Kind, open_root};

use super::{ApplyOptions, apply, baseline_at, scan_clone, select};

struct Tree {
    _dir: tempfile::TempDir,
    project: PathBuf,
    clone: PathBuf,
}

/// Ein Projekt, und sein „Klon" als Kopie mit erhaltenen Zeiten.
fn tree() -> Tree {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(project.join("src/deep")).unwrap();
    fs::create_dir_all(project.join(".git/hooks")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(project.join("src/deep/keep.txt"), "keep\n").unwrap();
    fs::write(project.join("gone.txt"), "gone\n").unwrap();
    fs::write(project.join(".env"), "SECRET=1\n").unwrap();
    fs::write(project.join(".git/HEAD"), "ref\n").unwrap();
    fs::write(project.join(".git/hooks/pre-commit"), "#!/bin/sh\n").unwrap();
    symlink("src/main.rs", project.join("link")).unwrap();
    let clone = dir.path().join("clone");
    Tree {
        _dir: dir,
        project,
        clone,
    }
}

impl Tree {
    /// Grundlage nehmen und klonen, die Masken entfernt.
    fn start(&self) -> (OwnedFd, crate::worktree::TreeSnapshot, Vec<PathBuf>) {
        let lower = open_root(&self.project).unwrap();
        let baseline = baseline_at(lower.as_fd());
        let copied = Command::new("cp")
            .arg("-Rp")
            .arg(&self.project)
            .arg(&self.clone)
            .status()
            .unwrap();
        assert!(copied.success());
        fs::remove_file(self.clone.join(".env")).unwrap();
        fs::remove_dir_all(self.clone.join(".git/hooks")).unwrap();
        (lower, baseline, masks())
    }
}

fn masks() -> Vec<PathBuf> {
    vec![PathBuf::from(".env"), PathBuf::from(".git/hooks")]
}

fn listed(set: &super::ChangeSet) -> Vec<(String, ChangeKind)> {
    set.changes
        .iter()
        .map(|change| (change.rel.display().to_string(), change.kind))
        .collect()
}

#[test]
fn an_untouched_clone_has_no_changes_and_masks_are_no_removals() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    let clone = open_root(&tree.clone).unwrap();
    let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    assert!(set.changes.is_empty(), "{:?}", set.changes);
    assert!(!set.truncated, "{:?}", set.incomplete);
}

#[test]
fn the_agents_changes_are_listed_as_on_an_overlay() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    fs::write(tree.clone.join("src/main.rs"), "fn main() { todo!() }\n").unwrap();
    fs::write(tree.clone.join("new.txt"), "new\n").unwrap();
    fs::remove_file(tree.clone.join("gone.txt")).unwrap();
    fs::remove_dir_all(tree.clone.join("src/deep")).unwrap();
    fs::remove_file(tree.clone.join("link")).unwrap();
    symlink("new.txt", tree.clone.join("link")).unwrap();
    fs::set_permissions(
        tree.clone.join(".git/HEAD"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let clone = open_root(&tree.clone).unwrap();
    let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    assert_eq!(
        listed(&set),
        vec![
            (".git/HEAD".to_owned(), ChangeKind::ModeChanged),
            ("gone.txt".to_owned(), ChangeKind::Removed),
            ("link".to_owned(), ChangeKind::Modified),
            ("new.txt".to_owned(), ChangeKind::Added),
            ("src/deep".to_owned(), ChangeKind::Removed),
            ("src/main.rs".to_owned(), ChangeKind::Modified),
        ]
    );
    assert!(set.changes.iter().all(|change| change.conflict.is_none()));
}

/// Mutationsbeweis: ohne `untouched` stünde die Datei des Menschen als
/// Änderung des Agenten da und würde zurückgedreht.
#[test]
fn what_the_human_changed_during_the_run_is_not_the_agents_change() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    fs::write(tree.project.join("src/main.rs"), "human\n").unwrap();
    fs::write(tree.project.join("later.txt"), "human\n").unwrap();
    fs::remove_file(tree.project.join("gone.txt")).unwrap();

    let clone = open_root(&tree.clone).unwrap();
    let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    assert!(set.changes.is_empty(), "{:?}", set.changes);
}

#[test]
fn a_change_of_the_agent_over_one_of_the_human_is_a_conflict() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    fs::write(tree.project.join("src/main.rs"), "human\n").unwrap();
    fs::write(tree.clone.join("src/main.rs"), "agent, longer\n").unwrap();

    let clone = open_root(&tree.clone).unwrap();
    let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    let chosen = select(&set, &[]).unwrap();
    assert!(chosen[0].conflict.is_some());
    let refused = apply(
        clone.as_fd(),
        lower.as_fd(),
        &baseline,
        &chosen,
        &ApplyOptions::default(),
    );
    assert!(refused.is_err());
    assert_eq!(
        fs::read_to_string(tree.project.join("src/main.rs")).unwrap(),
        "human\n"
    );
}

/// Liegt doch etwas auf einer Maske (das Profil lässt es nicht zu), bleibt es
/// abgelehnt; Mutationsbeweis: ohne `is_mask` im Lauf käme es aufs Projekt.
#[test]
fn anything_on_a_masked_path_is_never_written() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    fs::create_dir(tree.clone.join(".git/hooks")).unwrap();
    fs::write(tree.clone.join(".git/hooks/pre-commit"), "evil\n").unwrap();
    fs::write(tree.clone.join(".env"), "evil\n").unwrap();

    let clone = open_root(&tree.clone).unwrap();
    let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    assert_eq!(set.changes.len(), 2, "{:?}", set.changes);
    assert!(set.changes.iter().all(|change| change.refused.is_some()));
    assert!(set.changes.iter().all(|change| change.what == Kind::Other));
    let chosen = select(&set, &[]).unwrap();
    assert!(
        apply(
            clone.as_fd(),
            lower.as_fd(),
            &baseline,
            &chosen,
            &ApplyOptions::default()
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(tree.project.join(".env")).unwrap(),
        "SECRET=1\n"
    );
}

#[test]
fn apply_brings_the_clone_into_the_project_and_leaves_the_masks() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    fs::write(tree.clone.join("src/main.rs"), "changed\n").unwrap();
    fs::create_dir(tree.clone.join("fresh")).unwrap();
    fs::write(tree.clone.join("fresh/a.txt"), "a\n").unwrap();
    fs::remove_file(tree.clone.join("gone.txt")).unwrap();
    fs::remove_dir_all(tree.clone.join("src/deep")).unwrap();

    let clone = open_root(&tree.clone).unwrap();
    let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    let chosen = select(&set, &[]).unwrap();
    let report = apply(
        clone.as_fd(),
        lower.as_fd(),
        &baseline,
        &chosen,
        &ApplyOptions::default(),
    )
    .unwrap();
    assert!(report.failed.is_none(), "{:?}", report.failed);

    let read = |rel: &str| fs::read_to_string(tree.project.join(rel)).unwrap();
    assert_eq!(read("src/main.rs"), "changed\n");
    assert_eq!(read("fresh/a.txt"), "a\n");
    assert!(!tree.project.join("gone.txt").exists());
    assert!(!tree.project.join("src/deep").exists());
    assert_eq!(read(".env"), "SECRET=1\n");
    assert_eq!(read(".git/hooks/pre-commit"), "#!/bin/sh\n");

    let after = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, &masks);
    assert!(after.changes.is_empty(), "{:?}", after.changes);
}

#[test]
fn a_cut_short_list_is_never_empty_handed() {
    let tree = tree();
    let (lower, baseline, masks) = tree.start();
    let clone = open_root(&tree.clone).unwrap();
    let set =
        super::scan::scan_clone_with_budget(clone.as_fd(), lower.as_fd(), &baseline, &masks, 2);
    assert!(set.truncated);
    assert!(select(&set, &[]).is_err());
}

/// Mutationsbeweis: die alte Zeile, `auto` unter macOS still `direct`, rot.
#[test]
fn auto_is_disposable_when_the_system_can_on_every_platform() {
    use humanitl_config::SandboxMode;

    let choice = super::resolve_mode(SandboxMode::Auto, || Ok(())).unwrap();
    assert_eq!(choice.mode, SandboxMode::Disposable);
    assert!(choice.notice.is_none());
}

#[cfg(target_os = "macos")]
mod real_clone {
    use std::path::Path;

    use super::*;
    use crate::disposable::{Layer, clone};
    use humanitl_core::diagnostics::codes::{SANDBOX_030, SANDBOX_031};

    /// Unter `/private/tmp` liegt APFS, und das temporäre Verzeichnis auch.
    fn runtime(tree: &Tree) -> PathBuf {
        let runtime = tree.project.parent().unwrap().join("run");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        runtime
    }

    #[test]
    fn the_clone_holds_the_project_without_its_masks() {
        let tree = tree();
        let runtime = runtime(&tree);
        let lower = open_root(&tree.project).unwrap();
        let layer = Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks()).unwrap();
        let upper = layer.upper();
        assert_eq!(
            fs::read_to_string(upper.join("src/main.rs")).unwrap(),
            "fn main() {}\n"
        );
        assert_eq!(
            fs::read_link(upper.join("link")).unwrap(),
            Path::new("src/main.rs")
        );
        assert!(!upper.join(".env").exists(), "the secret was cloned");
        assert!(!upper.join(".git/hooks").exists(), "the hooks were cloned");
        assert!(upper.join(".git/HEAD").exists());

        fs::write(upper.join("src/main.rs"), "agent\n").unwrap();
        assert_eq!(
            fs::read_to_string(tree.project.join("src/main.rs")).unwrap(),
            "fn main() {}\n",
            "the clone shares blocks, never writes"
        );
    }

    #[test]
    fn sealing_moves_the_clone_and_discard_takes_it_all() {
        let tree = tree();
        let runtime = runtime(&tree);
        let lower = open_root(&tree.project).unwrap();
        let mut layer =
            Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks()).unwrap();
        let open = layer.upper();
        layer.seal().unwrap();
        layer.seal().unwrap();
        assert!(layer.is_sealed());
        assert!(!open.exists());
        assert!(layer.upper().join("src/main.rs").is_file());
        let dir = layer.dir().to_path_buf();
        assert!(layer.discard().is_ok());
        assert!(!dir.exists());
    }

    #[test]
    fn an_unreadable_file_refuses_and_leaves_no_layer() {
        let tree = tree();
        let runtime = runtime(&tree);
        fs::set_permissions(
            tree.project.join("gone.txt"),
            fs::Permissions::from_mode(0o000),
        )
        .unwrap();
        let lower = open_root(&tree.project).unwrap();
        let refused = Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks())
            .expect_err("clonefile takes the whole tree");
        assert_eq!(refused.code, SANDBOX_030);
        assert!(refused.fix.is_some());
        assert!(refused.why.contains("not readable"), "{}", refused.why);
        let left = fs::read_dir(runtime.join("disposable")).unwrap().count();
        assert_eq!(left, 0);
    }

    #[test]
    fn a_mask_behind_a_symlink_refuses() {
        let tree = tree();
        let runtime = runtime(&tree);
        fs::rename(tree.project.join("src"), tree.project.join("real")).unwrap();
        symlink("real", tree.project.join("src")).unwrap();
        fs::write(tree.project.join("real/.env"), "x\n").unwrap();
        let lower = open_root(&tree.project).unwrap();
        let refused = Layer::clone_project(
            &runtime,
            lower.as_fd(),
            &tree.project,
            vec![PathBuf::from("src/.env")],
        )
        .expect_err("the mask would not hold behind the link");
        assert_eq!(refused.code, SANDBOX_030);
    }

    /// Eine Maske, die es beim Start nicht gibt, bleibt eine Maske.
    #[test]
    fn a_mask_missing_at_the_start_is_stripped_quietly_and_stays_refused() {
        let tree = tree();
        let runtime = runtime(&tree);
        let lower = open_root(&tree.project).unwrap();
        let baseline = baseline_at(lower.as_fd());
        let masks = vec![
            PathBuf::from(".npmrc"),
            PathBuf::from("gone.txt/below"),
            PathBuf::from("nodir/.env"),
        ];
        let mut layer =
            Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks.clone()).unwrap();
        fs::write(layer.upper().join(".NPMRC"), "evil\n").unwrap();
        layer.seal().unwrap();
        let clone = layer.open_upper().unwrap();
        let set = scan_clone(clone.as_fd(), lower.as_fd(), &baseline, layer.masks());
        assert_eq!(set.changes.len(), 1, "{:?}", set.changes);
        assert!(set.changes[0].refused.is_some());
    }

    /// Mutationsbeweis: `open_root` statt `open_upper`, und `~/.ssh` stünde
    /// als Änderung da.
    #[test]
    fn a_symlink_in_place_of_the_clone_is_never_followed() {
        let tree = tree();
        let runtime = runtime(&tree);
        let lower = open_root(&tree.project).unwrap();
        let mut layer =
            Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks()).unwrap();
        let upper = layer.upper();
        let aside = upper.with_file_name("aside");
        fs::rename(&upper, &aside).unwrap();
        symlink(&tree.project, &upper).unwrap();

        let open = layer.open_upper().expect_err("a symlink is no layer");
        assert_eq!(open.code, SANDBOX_031);
        let sealed = layer.seal().expect_err("a symlink is not sealed");
        assert_eq!(sealed.code, SANDBOX_031);
        assert!(!layer.is_sealed());
        assert!(tree.project.join("src/main.rs").exists());
    }

    #[test]
    fn another_directory_in_place_of_the_clone_is_refused() {
        let tree = tree();
        let runtime = runtime(&tree);
        let lower = open_root(&tree.project).unwrap();
        let mut layer =
            Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks()).unwrap();
        let upper = layer.upper();
        fs::rename(&upper, upper.with_file_name("aside")).unwrap();
        fs::create_dir(&upper).unwrap();
        assert_eq!(layer.open_upper().unwrap_err().code, SANDBOX_031);
        assert_eq!(layer.seal().unwrap_err().code, SANDBOX_031);
    }

    fn chflags(flag: &str, path: &Path) {
        let done = Command::new("chflags")
            .arg(flag)
            .arg(path)
            .status()
            .unwrap();
        assert!(done.success());
    }

    /// `clonefile` trägt `uchg` mit; ohne `unlock` bliebe das Geheimnis im
    /// Klon, weil `strip_masks` scheitert. Mutationsbeweis: `unlock` leer, rot.
    #[test]
    fn a_locked_mask_leaves_the_clone_and_a_locked_file_leaves_with_discard() {
        let tree = tree();
        let runtime = runtime(&tree);
        chflags("uchg", &tree.project.join(".env"));
        let lower = open_root(&tree.project).unwrap();
        let cloned = Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks());
        chflags("nouchg", &tree.project.join(".env"));
        let layer = cloned.unwrap();
        assert!(!layer.upper().join(".env").exists());

        let dir = layer.dir().to_path_buf();
        let locked = layer.upper().join("src/main.rs");
        let parent = layer.upper().join("src/deep");
        chflags("uchg", &locked);
        chflags("uappnd", &parent);
        let acl = Command::new("chmod")
            .args(["+a", "everyone deny delete"])
            .arg(layer.upper().join("gone.txt"))
            .status()
            .unwrap();
        assert!(acl.success());
        assert!(layer.discard().is_ok());
        assert!(!dir.exists());
    }

    /// Ein schreibgeschütztes Verzeichnis der Schichten lässt sich nicht
    /// aufschließen: Die Schicht bleibt, und `sweep` sagt es.
    #[test]
    fn a_failed_sweep_says_so() {
        let tree = tree();
        let runtime = runtime(&tree);
        let lower = open_root(&tree.project).unwrap();
        let layer = Layer::clone_project(&runtime, lower.as_fd(), &tree.project, masks()).unwrap();
        let base = runtime.join("disposable");
        fs::set_permissions(&base, fs::Permissions::from_mode(0o500)).unwrap();
        let swept = crate::disposable::sweep(&runtime);
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(swept.removed, 0);
        assert_eq!(swept.failed.len(), 1);
        assert_eq!(swept.failed[0].code, SANDBOX_031);
        assert!(layer.dir().exists());

        let again = crate::disposable::sweep(&runtime);
        assert_eq!(again.removed, 1);
        assert!(again.failed.is_empty());
    }

    #[test]
    fn a_layer_inside_the_project_refuses() {
        let tree = tree();
        let runtime = tree.project.join("run");
        let lower = open_root(&tree.project).unwrap();
        let refused = Layer::clone_project(&runtime, lower.as_fd(), &tree.project, Vec::new())
            .expect_err("a clone into itself");
        assert_eq!(refused.code, SANDBOX_030);
    }

    /// Ein ausdrückliches `disposable` endet mit dem Befund, `auto` macht
    /// daraus den Hinweis, der den wirklichen Grund und `direct` nennt.
    #[test]
    fn a_failed_clone_becomes_the_notice_of_auto() {
        let notice = crate::disposable::fallback_notice(&clone::refused("no clone here"));
        assert_eq!(notice.code, humanitl_core::diagnostics::codes::SANDBOX_033);
        assert!(notice.why.contains("no clone here"), "{}", notice.why);
        assert!(notice.why.contains("directly"), "{}", notice.why);
    }

    #[test]
    fn only_apfs_carries_a_clone() {
        let project = Path::new("/Volumes/Stick/project");
        assert!(clone::volume_problem("apfs", project).is_none());
        let why = clone::volume_problem("exfat", project).unwrap();
        assert!(why.contains("exfat") && why.contains("APFS"), "{why}");
    }

    #[test]
    fn another_volume_says_so() {
        let why = clone::clone_problem(
            rustix::io::Errno::XDEV,
            Path::new("/Volumes/Stick/project"),
            Path::new("/private/var/folders/x/disposable/1"),
        );
        assert!(why.contains("different volumes"), "{why}");
    }
}
