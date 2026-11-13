//! Die erwartete Mounttabelle eines Podman-Containers (HUM-437).
//!
//! Zwei Teile: was der Plan selbst einhängt ([`super::plan::mounts`]), und
//! was Podman unter genau den festen Flags der Zeile von sich aus dazutut.
//!
//! **Der Plan.** Die Wurzel ist das Verzeichnis `rootfs` der Sitzung, nur
//! lesbar gebunden (`--rootfs`, `--read-only`), oder mit einem Abbild
//! (HUM-439) genau ein nur lesbares `overlay` aus dessen Schichten
//! ([`root_entry`]). Jeder Bind trägt Gerät und
//! Inode seiner Quelle und die Flags ihrer Einhängung auf dem Host: Podman
//! übernimmt `nosuid`, `nodev` und `noexec` der Quelle und setzt keine
//! eigenen (`getDefaultMountOptions` in `pkg/util/mount_opts.go`), und in
//! einem Nutzer-Namensraum kann es sie auch nicht lösen. Ein tmpfs des Plans
//! bekommt `nosuid,nodev`, ein verdeckendes (`/sys`, HUM-453) ist dazu nur
//! lesbar.
//!
//! **Podman selbst** ([`engine_entries`]), gemessen unter Podman 5.8.6 mit
//! der endgültigen Zeile (HUM-292, Messung P17, Abschnitt 10a; mit
//! `--passwd=false` und den tmpfs über `/sys` bestätigt am 2026-11-12 vom
//! Live-Test `the_mount_table_of_a_plain_session_matches_the_plan`, der bei
//! einer Abweichung beide Tabellen ganz druckt). Kein Eintrag lässt Typ,
//! Modus und Quelle zugleich offen; was vom Rechner abhängt, liest der Plan
//! vom Rechner:
//!
//! - `/proc` (`proc`) mit den nur lesbaren Selbst-Binds `asound`, `bus`,
//!   `fs`, `irq`, `sys`, `sysrq-trigger` (`proc`), den Verzeichnissen `acpi`
//!   und `scsi` als nur lesbares `tmpfs` und den Dateien `kcore`, `keys`,
//!   `latency_stats`, `timer_list`, `timer_stats`, `sched_debug` als nur
//!   lesbarer Bind genau von `/dev/null` des Rechners (Gerät und Inode). Jeder
//!   davon genau dann, wenn es den Pfad auf dem Rechner gibt; einen, den es
//!   nicht gibt, darf nichts belegen. `interrupts` verdeckt erst Podman 5,
//!   deshalb dort höchstens einmal, aber auch nur als Bind von `/dev/null`.
//! - `/dev` (`tmpfs`, nur lesbar wie die Wurzel) mit `pts` (`devpts`),
//!   `mqueue` (`mqueue`) und den sechs Gerätedateien des Rechners, je mit
//!   ihrem Gerät und Inode. `/dev/shm` legt das Profil selbst an, und Podman
//!   lässt sein eigenes dann weg.
//! - `/run/.containerenv` und `/etc/hostname`, Dateien von Podman auf
//!   `tmpfs`, nur lesbar.
//!
//! Kein `/etc/resolv.conf` (`--network none`), kein `/etc/hosts`
//! (`--no-hosts`, der Plan bindet sein eigenes), keine Benutzerdatenbank
//! (`--passwd=false`), kein `/run`, `/tmp`, `/var/tmp` (`--read-only-tmpfs=false`),
//! nichts aus einer `mounts.conf` (`--default-mounts-file`, HUM-454) und
//! nichts unter `/sys` außer den beiden tmpfs des Plans (HUM-453). Was davon
//! doch erscheint, ist ein Eintrag zu viel.

use std::path::Path;

use humanitl_core::Diagnostic;

use super::plan::{Mount, MountKind, Root};
use crate::bwrap::PinnedMounts;
use crate::mount_table::{
    ExpectedMount, FileId, MountFlags, MountMode, MountPins, MountSource, bind_of, by_target,
    host_mount, id_of_path, pins_of,
};
use crate::profile::{SandboxProfile, SessionContext};

const NOSUID: MountFlags = MountFlags {
    nosuid: true,
    nodev: false,
    noexec: false,
};

const NOSUID_NOEXEC: MountFlags = MountFlags {
    nosuid: true,
    nodev: false,
    noexec: true,
};

/// Was `crun` unter `/proc` auf sich selbst nur lesbar bindet, soweit es den
/// Pfad gibt (gemessen: `proc`, `ro,nosuid,nodev,noexec`).
const PROC_READ_ONLY: [&str; 6] = ["asound", "bus", "fs", "irq", "sys", "sysrq-trigger"];

/// Was Podman unter `/proc` als Verzeichnis verdeckt, soweit es den Pfad
/// gibt (gemessen: leeres `tmpfs`, `ro,nosuid,nodev`).
const PROC_MASKED_DIRS: [&str; 2] = ["acpi", "scsi"];

/// Was Podman unter `/proc` als Datei mit `/dev/null` des Hosts verdeckt,
/// soweit es den Pfad gibt (gemessen: Bind von `/dev/null`, `ro,nosuid`).
const PROC_MASKED_FILES: [&str; 6] = [
    "kcore",
    "keys",
    "latency_stats",
    "timer_list",
    "timer_stats",
    "sched_debug",
];

/// Wie [`PROC_MASKED_FILES`], aber erst in neueren Fassungen von Podman
/// (5.x verdeckt es, 4.9 nicht): Auf einem Host, der es hat, höchstens
/// einmal, und dann nur als Bind von `/dev/null`, nur lesbar.
const PROC_MASKED_FILES_NEWER: [&str; 1] = ["interrupts"];

/// Die Gerätedateien des Hosts, die Podman in `/dev` bindet (gemessen:
/// `rw,nosuid,noexec`).
const DEV_NODES: [&str; 6] = ["null", "zero", "full", "tty", "random", "urandom"];

/// Die Dateien von Podman selbst, aus seinem Laufzeitverzeichnis (gemessen:
/// `tmpfs`, `ro,nosuid,nodev,noexec`).
const ENGINE_FILES: [&str; 2] = ["/run/.containerenv", "/etc/hostname"];

/// Gerät, Inode und Flags der Einhängung einer Gerätedatei des Hosts.
type DeviceOf<'a> = &'a dyn Fn(&Path) -> Result<(FileId, MountFlags), Diagnostic>;

/// Was die eigenen Einträge von Podman vom Host brauchen: ob es einen Pfad
/// unter `/proc` gibt, und Gerät, Inode und Einhängung einer Gerätedatei.
pub(super) struct HostView<'a> {
    /// Ob es den Pfad gibt, ohne einem Verweis zu folgen.
    pub(super) exists: &'a dyn Fn(&Path) -> bool,
    /// Gerät, Inode und Flags der Einhängung einer Gerätedatei des Hosts.
    pub(super) device: DeviceOf<'a>,
}

/// [`engine_entries`] für den Host dieses Prozesses.
///
/// # Errors
///
/// Wie [`engine_entries`].
fn engine_entries_here() -> Result<Vec<ExpectedMount>, Diagnostic> {
    let exists = |path: &Path| path.symlink_metadata().is_ok();
    let device = |path: &Path| Ok((id_of_path(path)?, host_mount(path)?.flags));
    engine_entries(&HostView {
        exists: &exists,
        device: &device,
    })
}

/// Die eigenen Einträge von Podman (Modulbeschreibung), für diesen Host:
/// Ein Pfad unter `/proc`, den es hier gibt, muss genau so verdeckt sein, wie
/// gemessen; einen, den es nicht gibt, darf nichts belegen. Kein Eintrag
/// nimmt beliebigen Typ, Modus und Quelle an: Eine schreibbare Einhängung auf
/// `/proc/kcore` ist immer ein Eintrag zu viel.
///
/// # Errors
///
/// `SANDBOX_011`, wenn sich eine Gerätedatei des Hosts nicht lesen lässt.
pub(super) fn engine_entries(host: &HostView<'_>) -> Result<Vec<ExpectedMount>, Diagnostic> {
    let proc = Path::new("/proc");
    let dev = Path::new("/dev");
    let mut out = vec![ExpectedMount::typed(
        proc,
        "proc",
        MountMode::ReadWrite,
        MountFlags::ALL,
    )];
    for name in PROC_READ_ONLY {
        let point = proc.join(name);
        if (host.exists)(&point) {
            out.push(ExpectedMount::typed(
                &point,
                "proc",
                MountMode::ReadOnly,
                MountFlags::ALL,
            ));
        }
    }
    // Leere, nur lesbare tmpfs. Ihre Flags hängen an der Fassung von Podman:
    // 5.8.6 setzt `nosuid,nodev`, 4.9.3 keines (gemessen in der CI,
    // 2026-11-12). Ein leeres, nur lesbares Verzeichnis gibt dem Agenten mit
    // keinem der Flags etwas; Typ, Modus und Anzahl bleiben verlangt.
    for name in PROC_MASKED_DIRS {
        let point = proc.join(name);
        if (host.exists)(&point) {
            out.push(
                ExpectedMount::typed(
                    &point,
                    "tmpfs",
                    MountMode::ReadOnly,
                    MountFlags::NOSUID_NODEV,
                )
                .any_flags(),
            );
        }
    }
    let (null, null_flags) = (host.device)(&dev.join("null"))?;
    let masked = |point: &Path| {
        ExpectedMount::bind(point, null, MountMode::ReadOnly, NOSUID.union(null_flags))
    };
    for name in PROC_MASKED_FILES {
        let point = proc.join(name);
        if (host.exists)(&point) {
            out.push(masked(&point));
        }
    }
    for name in PROC_MASKED_FILES_NEWER {
        let point = proc.join(name);
        if (host.exists)(&point) {
            out.push(masked(&point).optional());
        }
    }
    out.push(ExpectedMount::typed(
        dev,
        "tmpfs",
        MountMode::ReadOnly,
        NOSUID,
    ));
    out.push(ExpectedMount::typed(
        &dev.join("pts"),
        "devpts",
        MountMode::ReadWrite,
        NOSUID_NOEXEC,
    ));
    out.push(ExpectedMount::typed(
        &dev.join("mqueue"),
        "mqueue",
        MountMode::ReadWrite,
        MountFlags::ALL,
    ));
    for name in DEV_NODES {
        let point = dev.join(name);
        let (id, flags) = (host.device)(&point)?;
        out.push(ExpectedMount::bind(
            &point,
            id,
            MountMode::ReadWrite,
            NOSUID_NOEXEC.union(flags),
        ));
    }
    // Podman bindet beide aus seiner `runroot`; deren Dateisystem ist meist
    // ein tmpfs unter `XDG_RUNTIME_DIR`, kann aber auch ein anderes sein.
    // Verlangt bleiben Punkt, nur lesbar, alle drei Flags und genau einmal.
    for file in ENGINE_FILES {
        out.push(
            ExpectedMount::typed(
                Path::new(file),
                "tmpfs",
                MountMode::ReadOnly,
                MountFlags::ALL,
            )
            .any_type(),
        );
    }
    Ok(out)
}

/// Der Eintrag der Wurzel.
///
/// Ein Verzeichnis ist ein nur lesbarer Bind genau dieses Verzeichnisses
/// (Gerät und Inode), mit den Flags seiner Einhängung auf dem Host. Ein
/// Abbild ist genau ein `overlay` (der Speicher hat den Treiber `overlay` ohne
/// Hilfsprogramm, `EngineDir::storage_text`), nur lesbar (`--read-only`).
/// Seine Flags bleiben offen: Das Overlay entsteht in der Engine und hat kein
/// Gegenstück auf dem Host, an dem sie sich ablesen ließen, und
/// `no-new-privileges` nimmt einem Programm mit `setuid` darin ohnehin jede
/// Wirkung. Ein zweites Dateisystem auf `/`, ein schreibbares oder eines
/// anderen Typs ist weiter ein Eintrag zu viel. Gerät und Inode hat das Abbild
/// nicht; dass es das gewählte ist, belegt der Digest vor dem Start.
///
/// # Errors
///
/// `SANDBOX_011`, wenn sich das Verzeichnis nicht lesen lässt.
pub(super) fn root_entry(root: Root<'_>) -> Result<ExpectedMount, Diagnostic> {
    Ok(match root {
        Root::Dir(rootfs) => ExpectedMount::bind(
            Path::new("/"),
            id_of_path(rootfs)?,
            MountMode::ReadOnly,
            host_mount(rootfs)?.flags,
        ),
        Root::Image { .. } => ExpectedMount::typed(
            Path::new("/"),
            "overlay",
            MountMode::ReadOnly,
            MountFlags::default(),
        )
        .any_flags(),
    })
}

/// Ein Eintrag, wie ihn eine Engine in einem anderen Mount-Namensraum
/// sieht (HUM-434).
///
/// Startet die Engine als Unit des Dienstverwalters des Nutzers, liest der
/// Daemon die Flags der Quellen in seinem eigenen, gehärteten Namensraum,
/// wo `ProtectSystem` und Verwandte die Dateisysteme des Rechners mit
/// `nosuid` (und mehr) neu einhängen; die Engine bindet sie aus dem
/// Namensraum des Dienstverwalters, wo diese Flags fehlen. Die Flags, die
/// ein Bind von seiner Quelle erbt, sind von hier aus also nicht zu wissen
/// und bleiben offen (`*`). Streng bleiben Punkt, Typ, nur lesbar oder
/// schreibbar, Anzahl und Gerät und Inode der Quelle; Einhängungen, deren
/// Flags die Engine selbst setzt (tmpfs, `proc`), behalten ihre. Ein Bind
/// mit offenen Flags gibt keine Rechte, die die Quelle auf dem Rechner
/// nicht schon hat. Ohne Unit (`elsewhere` falsch) bleibt alles, wie es war.
pub(super) fn as_seen_by_engine(entry: ExpectedMount, elsewhere: bool) -> ExpectedMount {
    if elsewhere && matches!(entry.source, MountSource::Inode(_)) {
        entry.any_flags()
    } else {
        entry
    }
}

/// Was der Shim eines Podman-Containers hinter `--pins-fd` bekommt.
///
/// `mounts` sind die Einhängungen des Plans mit den geprüften Quellen,
/// `root` die Wurzel der Sitzung; ein Verzeichnis als Wurzel existiert schon.
/// `flags_seen_elsewhere`: Die Engine startet als Unit des Dienstverwalters
/// und sieht die Flags der Quellen anders als der Daemon
/// ([`as_seen_by_engine`]).
///
/// # Errors
///
/// `SANDBOX_011`, wenn sich eine Quelle nicht lesen lässt.
pub(super) fn pins(
    profile: &SandboxProfile,
    session: &SessionContext,
    pinned: &PinnedMounts,
    mounts: &[Mount],
    root: Root<'_>,
    flags_seen_elsewhere: bool,
) -> Result<MountPins, Diagnostic> {
    let seen = |entry| as_seen_by_engine(entry, flags_seen_elsewhere);
    let agent = session
        .agent_program
        .as_ref()
        .map(|program| program.dst.as_path());
    let pins = pins_of(profile, pinned, agent, true)?;
    let known = by_target(&pins);
    let mut table = MountPins {
        pins,
        mounts: Vec::new(),
    };
    table.expect(seen(root_entry(root)?));
    for mount in mounts {
        table.expect(seen(match &mount.kind {
            MountKind::Bind {
                source, writable, ..
            } => bind_of(&mount.dst, source, *writable, MountFlags::default(), &known)?,
            MountKind::Tmpfs => ExpectedMount::typed(
                &mount.dst,
                "tmpfs",
                MountMode::ReadWrite,
                MountFlags::NOSUID_NODEV,
            ),
            MountKind::Hidden => ExpectedMount::typed(
                &mount.dst,
                "tmpfs",
                MountMode::ReadOnly,
                MountFlags::NOSUID_NODEV,
            ),
        }));
    }
    for entry in engine_entries_here()? {
        table.expect(seen(entry));
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const NULL: FileId = FileId { dev: 5, ino: 4 };

    fn entries(present: &[&str]) -> Vec<ExpectedMount> {
        let exists = |path: &Path| present.iter().any(|p| path == Path::new(p));
        let device = |path: &Path| {
            let ino = DEV_NODES
                .iter()
                .position(|name| path == Path::new("/dev").join(name))
                .map_or(99, |index| 4 + index as u64);
            Ok((FileId { dev: 5, ino }, MountFlags::default()))
        };
        engine_entries(&HostView {
            exists: &exists,
            device: &device,
        })
        .unwrap()
    }

    fn at<'a>(entries: &'a [ExpectedMount], point: &str) -> Vec<&'a ExpectedMount> {
        entries
            .iter()
            .filter(|entry| entry.point == Path::new(point))
            .collect()
    }

    /// Kein eigener Eintrag von Podman liegt dort, wo der Plan einhängt oder
    /// wo eine Konfigurationsquelle etwas hinzufügen würde, und keiner nimmt
    /// beliebige Flags an; unter `/proc` ist jeder nur lesbar.
    /// Unter der Unit des Dienstverwalters (HUM-434) bleiben die Flags eines
    /// Binds offen, alles andere streng; die Flags, die die Engine selbst
    /// setzt, bleiben verlangt. Ohne Unit ändert sich nichts.
    #[test]
    fn under_the_user_unit_only_the_inherited_flags_of_binds_are_open() {
        let id = FileId { dev: 8, ino: 42 };
        let bind = ExpectedMount::bind(
            Path::new("/usr"),
            id,
            MountMode::ReadOnly,
            MountFlags {
                nosuid: true,
                nodev: false,
                noexec: false,
            },
        );
        let seen = as_seen_by_engine(bind.clone(), true);
        assert_eq!(seen.flags, None);
        assert_eq!(
            (
                &seen.point,
                &seen.fstype,
                seen.source,
                seen.mode,
                seen.min,
                seen.max
            ),
            (
                &bind.point,
                &bind.fstype,
                MountSource::Inode(id),
                MountMode::ReadOnly,
                1,
                1
            )
        );
        assert_eq!(as_seen_by_engine(bind.clone(), false), bind);
        let tmpfs = ExpectedMount::typed(
            Path::new("/tmp"),
            "tmpfs",
            MountMode::ReadWrite,
            MountFlags::NOSUID_NODEV,
        );
        assert_eq!(as_seen_by_engine(tmpfs.clone(), true), tmpfs);
    }

    #[test]
    fn the_engine_entries_never_cover_a_planned_or_a_foreign_point() {
        let all = entries(&["/proc/kcore", "/proc/acpi", "/proc/sys", "/proc/interrupts"]);
        for entry in &all {
            for planned in [
                "/",
                "/work",
                "/tmp",
                "/var/tmp",
                "/run",
                "/run/secrets",
                "/sys",
                "/etc/hosts",
                "/etc/passwd",
                "/etc/group",
                "/etc/resolv.conf",
                "/dev/shm",
            ] {
                assert_ne!(entry.point, Path::new(planned));
            }
            // Offene Flags nur auf einem leeren, nur lesbaren tmpfs, dessen
            // Flags von der Fassung von Podman abhängen.
            let empty_mask =
                entry.fstype.as_deref() == Some("tmpfs") && entry.mode == MountMode::ReadOnly;
            assert!(entry.flags.is_some() || empty_mask, "{entry:?}");
            let under_proc = entry.point != Path::new("/proc") && entry.point.starts_with("/proc");
            if under_proc {
                assert_eq!(entry.mode, MountMode::ReadOnly, "{entry:?}");
            }
        }
    }

    /// Was es auf dem Host gibt, muss verdeckt sein, als nur lesbarer Bind
    /// genau von `/dev/null`; was es nicht gibt, hat keinen Eintrag, und jede
    /// Einhängung dort ist zu viel.
    #[test]
    fn masks_follow_the_host_and_are_pinned_to_dev_null() {
        let some = entries(&["/proc/kcore", "/proc/acpi"]);
        let kcore = at(&some, "/proc/kcore");
        assert_eq!(kcore.len(), 1);
        assert_eq!(kcore[0].source, MountSource::Inode(NULL));
        assert_eq!(kcore[0].mode, MountMode::ReadOnly);
        assert_eq!((kcore[0].min, kcore[0].max), (1, 1));
        assert_eq!(at(&some, "/proc/acpi")[0].fstype.as_deref(), Some("tmpfs"));
        assert!(at(&some, "/proc/keys").is_empty());
        assert!(at(&some, "/proc/sys").is_empty());
        let newer = entries(&["/proc/interrupts"]);
        let interrupts = at(&newer, "/proc/interrupts");
        assert_eq!((interrupts[0].min, interrupts[0].max), (0, 1));
        assert_eq!(interrupts[0].source, MountSource::Inode(NULL));
        let null = at(&some, "/dev/null");
        assert_eq!(null[0].source, MountSource::Inode(NULL));
        assert_eq!(null[0].mode, MountMode::ReadWrite);
    }

    /// Unter einem Abbild ist die Wurzel genau ein nur lesbares `overlay`:
    /// kein Bind, kein schreibbares und kein zweites (HUM-439).
    #[test]
    fn an_image_root_is_one_read_only_overlay() {
        let entry = root_entry(Root::Image {
            reference: "localhost/x@sha256:00",
            user: (1, 1),
        })
        .unwrap();
        assert_eq!(entry.point, Path::new("/"));
        assert_eq!(entry.fstype.as_deref(), Some("overlay"));
        assert_eq!(entry.source, MountSource::Type);
        assert_eq!(entry.mode, MountMode::ReadOnly);
        assert_eq!((entry.min, entry.max), (1, 1));
        let dir = tempfile::tempdir().unwrap();
        let entry = root_entry(Root::Dir(dir.path())).unwrap();
        assert!(matches!(entry.source, MountSource::Inode(_)), "{entry:?}");
        assert_eq!(entry.mode, MountMode::ReadOnly);
    }
}
