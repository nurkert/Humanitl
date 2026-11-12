//! Die erwartete Mounttabelle einer bwrap-Sandbox (HUM-437).
//!
//! Aus der Tabelle der Zeile ([`crate::BwrapLine::policy`]), Zeile für
//! Zeile, und aus dem, was bwrap selbst einhängt. Was bwrap tut, steht in
//! seinem Quelltext (bubblewrap 0.8 bis 0.11, `bubblewrap.c` und
//! `bind-mount.c`):
//!
//! - Die Wurzel ist ein tmpfs mit `nosuid,nodev`, schreibbar (kein
//!   `--remount-ro /` in der Zeile).
//! - Jeder Bind bekommt `nosuid` und, außer den Geräten unter `/dev`,
//!   `nodev`; dazu erbt er die Flags der Einhängung seiner Quelle auf dem Host
//!   (in einem Nutzer-Namensraum unabänderlich) und ist nur lesbar, wenn die
//!   Zeile es sagt oder die Quelle es ist.
//! - `--ro-bind-data` schreibt die Datei in das tmpfs der Wurzel und bindet
//!   sie nur lesbar: Typ `tmpfs`, Quelle der Typ, denn den Inode der Kopie
//!   kennt der Launcher nicht.
//! - `--tmpfs`: `tmpfs` mit `nosuid,nodev`.
//! - `--proc`: `proc` mit `nosuid,nodev,noexec`, darüber `sys`,
//!   `sysrq-trigger`, `irq` und `bus` je auf sich selbst nur lesbar gebunden,
//!   soweit es sie auf dem Host gibt.
//! - `--dev`: ein `tmpfs` mit `nosuid,nodev`, darin `null`, `zero`, `full`,
//!   `random`, `urandom` und `tty` als Binds der Geräte des Hosts (mit
//!   `nosuid`, ohne `nodev`), `pts` als `devpts` mit `nosuid,noexec` und
//!   `console`, wenn bwrap an einem Terminal startet.
//! - `--overlay` (Wegwerf-Sandbox, HUM-350): ein `overlay` auf `/work`,
//!   schreibbar; seine Flags setzt der Kernel, sie zählen hier nicht.
//!
//! Was eine Quelle auf dem Host unter sich eingehängt hat, kommt mit (bwrap
//! bindet rekursiv) und ist ein Eintrag zu viel: Eine solche Quelle lehnt die
//! Prüfung ab, statt eine Einhängung außerhalb des Plans hinzunehmen.

use std::collections::BTreeMap;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::path::{Path, PathBuf};

use humanitl_core::Diagnostic;

use crate::bwrap::PinnedMounts;
use crate::bwrap_args::{BwrapLine, LaunchInputs};
use crate::launcher::{PolicyMode, PolicyRow};
use crate::mount_table::{
    ExpectedMount, FileId, MountFlags, MountMode, MountPins, bind_of, by_target, host_mount,
    id_of_path, pins_of,
};
use crate::profile::{SandboxProfile, SessionContext};

/// Die Gerätedateien, die `--dev` vom Host bindet.
const DEV_NODES: [&str; 6] = ["null", "zero", "full", "random", "urandom", "tty"];

/// Was `--proc` auf sich selbst nur lesbar bindet.
const PROC_COVERED: [&str; 4] = ["sys", "sysrq-trigger", "irq", "bus"];

/// Die Zeile zu `inputs` mit dem memfd der erwarteten Mounttabelle
/// ([`pins`]) als `--pins-fd`. Die Tabelle kommt aus derselben Zeile, und die
/// Nummer des memfd ändert an ihr nichts.
///
/// # Errors
///
/// `SANDBOX_011` aus [`pins`] und [`MountPins::memfd`].
pub(crate) fn line_with_pins(
    profile: &SandboxProfile,
    session: &SessionContext,
    pinned: &PinnedMounts,
    inputs: LaunchInputs,
) -> Result<(BwrapLine, OwnedFd), Diagnostic> {
    let rows = profile.to_bwrap_line(session, &inputs).policy;
    let fd = pins(profile, session, pinned, &rows, inputs.overlay.is_some())?.memfd()?;
    let inputs = LaunchInputs {
        pins_fd: Some(fd.as_raw_fd()),
        ..inputs
    };
    Ok((profile.to_bwrap_line(session, &inputs), fd))
}

/// Was der Shim einer bwrap-Sandbox hinter `--pins-fd` bekommt: die
/// geprüften Ziele aus den geöffneten Quellen und die Tabelle aus den Zeilen
/// `rows` der Zeile, die startet.
///
/// # Errors
///
/// `SANDBOX_011` aus [`pins_of`] und [`expected`].
pub(crate) fn pins(
    profile: &SandboxProfile,
    session: &SessionContext,
    pinned: &PinnedMounts,
    rows: &[PolicyRow],
    overlay: bool,
) -> Result<MountPins, Diagnostic> {
    let agent = session
        .agent_program
        .as_ref()
        .map(|program| program.dst.as_path());
    let pins = pins_of(profile, pinned, agent, !overlay)?;
    let known = by_target(&pins);
    let mut table = MountPins {
        pins,
        mounts: Vec::new(),
    };
    for entry in expected(rows, &profile.mounts.work.dst, overlay, &known)? {
        table.expect(entry);
    }
    Ok(table)
}

/// Die erwartete Tabelle zu den Zeilen der Tabelle von bwrap.
///
/// `work` ist das Ziel des Projektordners, `overlay` sagt, ob er die untere
/// Schicht eines Overlays ist; `known` hält Gerät, Inode und Ort auf dem Host
/// der geprüften Ziele ([`crate::mount_table::by_target`]).
///
/// # Errors
///
/// `SANDBOX_011`, wenn sich eine Quelle oder ein Gerät des Hosts nicht lesen
/// lässt.
pub(crate) fn expected(
    rows: &[PolicyRow],
    work: &Path,
    overlay: bool,
    known: &BTreeMap<PathBuf, (FileId, PathBuf)>,
) -> Result<Vec<ExpectedMount>, Diagnostic> {
    let mut out = vec![ExpectedMount::typed(
        Path::new("/"),
        "tmpfs",
        MountMode::ReadWrite,
        MountFlags::NOSUID_NODEV,
    )];
    for row in rows {
        let target = row.target.as_path();
        match row.mode {
            PolicyMode::ReadOnly | PolicyMode::ReadWrite if overlay && target == work => {
                out.push(
                    ExpectedMount::typed(
                        target,
                        "overlay",
                        MountMode::ReadWrite,
                        MountFlags::NOSUID_NODEV,
                    )
                    .any_flags(),
                );
            }
            PolicyMode::ReadOnly | PolicyMode::ReadWrite => {
                let source = row.source.as_deref().unwrap_or(target);
                out.push(bind_of(
                    target,
                    source,
                    row.mode == PolicyMode::ReadWrite,
                    MountFlags::NOSUID_NODEV,
                    known,
                )?);
            }
            PolicyMode::Masked => out.push(ExpectedMount::typed(
                target,
                "tmpfs",
                MountMode::ReadOnly,
                MountFlags::NOSUID_NODEV,
            )),
            PolicyMode::Tmpfs => out.push(ExpectedMount::typed(
                target,
                "tmpfs",
                MountMode::ReadWrite,
                MountFlags::NOSUID_NODEV,
            )),
            PolicyMode::Proc => {
                out.push(ExpectedMount::typed(
                    target,
                    "proc",
                    MountMode::ReadWrite,
                    MountFlags::ALL,
                ));
                for sub in PROC_COVERED {
                    out.push(
                        ExpectedMount::typed(
                            &target.join(sub),
                            "proc",
                            MountMode::ReadOnly,
                            MountFlags::ALL,
                        )
                        .optional(),
                    );
                }
            }
            PolicyMode::Dev => dev(target, &mut out)?,
            PolicyMode::Symlink | PolicyMode::Denied => {}
        }
    }
    Ok(out)
}

/// `--dev`: das tmpfs, die Geräte des Hosts, `pts` und vielleicht `console`.
fn dev(target: &Path, out: &mut Vec<ExpectedMount>) -> Result<(), Diagnostic> {
    out.push(ExpectedMount::typed(
        target,
        "tmpfs",
        MountMode::ReadWrite,
        MountFlags::NOSUID_NODEV,
    ));
    for node in DEV_NODES {
        let host = Path::new("/dev").join(node);
        let mount = host_mount(&host)?;
        out.push(ExpectedMount::bind(
            &target.join(node),
            id_of_path(&host)?,
            MountMode::of(!mount.read_only),
            MountFlags {
                nosuid: true,
                ..MountFlags::default()
            }
            .union(mount.flags),
        ));
    }
    out.push(ExpectedMount::typed(
        &target.join("pts"),
        "devpts",
        MountMode::ReadWrite,
        MountFlags {
            nosuid: true,
            nodev: false,
            noexec: true,
        },
    ));
    // Nur, wenn bwrap an einem Terminal startet: das Terminal des Aufrufers
    // mit dessen Flags. Sein Typ hängt am Terminal: ein `devpts` in einem
    // Terminalfenster, `devtmpfs` an einer Textkonsole (`/dev/tty1`).
    out.push(
        ExpectedMount::typed(
            &target.join("console"),
            "devpts",
            MountMode::ReadWrite,
            MountFlags::default(),
        )
        .any_type()
        .any_flags()
        .optional(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::launcher::PolicyOrigin;
    use crate::mount_table::MountSource;

    fn row(mode: PolicyMode, source: Option<&Path>, target: &str) -> PolicyRow {
        PolicyRow::new(
            mode,
            source.map(Path::to_path_buf),
            PathBuf::from(target),
            PolicyOrigin::Profile,
        )
    }

    /// Jede Zeile der Tabelle wird ihr Eintrag: ein Bind mit Gerät und Inode
    /// der Quelle, eine Maske nur lesbar als `tmpfs`, `/proc` mit seinen
    /// Abdeckungen, `/dev` mit den Geräten des Hosts.
    #[test]
    fn every_row_becomes_its_entry() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("proj");
        std::fs::create_dir(&work).unwrap();
        let usr = dir.path().join("usr");
        std::fs::create_dir(&usr).unwrap();
        let rows = vec![
            row(PolicyMode::ReadOnly, Some(&usr), "/usr"),
            row(PolicyMode::Symlink, None, "/bin"),
            row(PolicyMode::Proc, None, "/proc"),
            row(PolicyMode::Tmpfs, None, "/tmp"),
            row(PolicyMode::ReadWrite, Some(&work), "/work"),
            row(PolicyMode::Masked, None, "/work/.envrc"),
        ];
        let known = BTreeMap::from([(
            PathBuf::from("/work"),
            (FileId { dev: 7, ino: 9 }, work.clone()),
        )]);
        let table = expected(&rows, Path::new("/work"), false, &known).unwrap();
        let at = |point: &str| {
            table
                .iter()
                .filter(|entry| entry.point == Path::new(point))
                .collect::<Vec<_>>()
        };
        assert_eq!(at("/")[0].fstype.as_deref(), Some("tmpfs"));
        let usr_entry = at("/usr")[0];
        assert_eq!(
            usr_entry.source,
            MountSource::Inode(id_of_path(&usr).unwrap())
        );
        assert_eq!(usr_entry.mode, MountMode::ReadOnly);
        assert!(at("/bin").is_empty(), "a symlink is no mount");
        assert_eq!(at("/proc")[0].flags, Some(MountFlags::ALL));
        assert_eq!((at("/proc/sys")[0].min, at("/proc/sys")[0].max), (0, 1));
        let work_entry = at("/work")[0];
        assert_eq!(
            work_entry.source,
            MountSource::Inode(FileId { dev: 7, ino: 9 })
        );
        let flags = work_entry.flags.unwrap();
        assert!(flags.nosuid && flags.nodev);
        assert_eq!(at("/work/.envrc")[0].mode, MountMode::ReadOnly);
        assert_eq!(at("/work/.envrc")[0].source, MountSource::Type);

        let overlay = expected(&rows, Path::new("/work"), true, &known).unwrap();
        let work_entry = overlay
            .iter()
            .find(|entry| entry.point == Path::new("/work"))
            .unwrap();
        assert_eq!(work_entry.fstype.as_deref(), Some("overlay"));
        assert_eq!(work_entry.source, MountSource::Type);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dev_binds_the_hosts_nodes_and_adds_pts() {
        let rows = vec![row(PolicyMode::Dev, None, "/dev")];
        let table = expected(&rows, Path::new("/work"), false, &BTreeMap::new()).unwrap();
        let null = table
            .iter()
            .find(|entry| entry.point == Path::new("/dev/null"))
            .unwrap();
        assert_eq!(
            null.source,
            MountSource::Inode(id_of_path(Path::new("/dev/null")).unwrap())
        );
        assert!(!null.flags.unwrap().nodev || host_mount(Path::new("/dev")).unwrap().flags.nodev);
        assert!(
            table
                .iter()
                .any(|entry| entry.point == Path::new("/dev/pts")
                    && entry.fstype.as_deref() == Some("devpts"))
        );
    }
}
