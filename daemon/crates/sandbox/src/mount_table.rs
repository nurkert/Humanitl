//! Die erwartete Mounttabelle und die geprüften Ordner, für den Shim
//! (HUM-437).
//!
//! bwrap hängt Deskriptoren ein (HUM-263); ein Tausch nach der Prüfung
//! erreicht die Sandbox dort nicht. Eine Container-Engine nimmt Pfade und
//! löst sie später in einem anderen Prozess auf (HUM-292 P13: ein Deskriptor
//! ist dort keine Quelle), und ihre Konfiguration kann jeder Sandbox eine
//! Einhängung hinzufügen, die keinen Socket trägt und deshalb an der
//! Socket-Suche der zweiten Garantie vorbeigeht. Der Plan jedes Backends gibt
//! dem Shim deshalb zweierlei mit, in einem versiegelten memfd
//! ([`crate::SHIM_PINS_FD`]):
//!
//! - **die geprüften Ziele** ([`Pin`]): `/work`, jeden Eintrag aus
//!   `mounts.ro`, `extra_ro`, `extra_rw` und das Agentenprogramm, je mit
//!   Gerät und Inode, wie der Launcher sie beim Öffnen sah;
//! - **die erwartete Mounttabelle** ([`ExpectedMount`]) als Multimenge: jeder
//!   Bind, jedes tmpfs und jede Maske des Plans und die eigenen Einträge des
//!   Backends. Die Quelle eines Binds ist Gerät und Inode der Quelle, die
//!   jeder anderen Einhängung ihr Typ.
//!
//! Der Shim vergleicht beides vor allem anderen (`mounts.rs` im Shim) und
//! lehnt den Start bei jeder Abweichung ab; `isolation_check` ordnet beide
//! Zeilen der zweiten Garantie zu (`SANDBOX_049`, `SANDBOX_042`).
//!
//! Was hier steht, ist neutral: Welche Einträge ein Backend selbst beiträgt,
//! weiß nur sein Adapter.

use std::collections::BTreeMap;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::SANDBOX_011;
use humanitl_core::{Diagnostic, Severity};

use crate::bwrap::PinnedMounts;
use crate::pinned::PinnedSource;
use crate::profile::SandboxProfile;

/// Der Name des memfd, wie ihn `/proc/<pid>/fd` zeigt.
pub(crate) const PINS_MEMFD_NAME: &str = "humanitl-pins";

/// Gerät und Inode eines Objekts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FileId {
    /// `st_dev`.
    pub(crate) dev: u64,
    /// `st_ino`.
    pub(crate) ino: u64,
}

/// Ein geprüftes Ziel in der Sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pin {
    /// Der Pfad in der Sandbox.
    pub(crate) path: PathBuf,
    /// Was dort liegen muss.
    pub(crate) id: FileId,
    /// Wo die Quelle auf dem Host liegt; für die Flags ihrer Einhängung,
    /// nicht für den Shim.
    pub(crate) host: PathBuf,
}

/// Woran der Shim die Quelle einer Einhängung erkennt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MountSource {
    /// Ein Bind: Gerät und Inode der Quelle.
    Inode(FileId),
    /// Jede andere Einhängung: ihr Typ.
    Type,
}

/// Nur lesbar oder schreibbar. Beliebig ist nie ein Eintrag (HUM-437,
/// Review): Wer den Modus offen ließe, nähme eine schreibbare Einhängung an.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MountMode {
    /// `ro`.
    ReadOnly,
    /// `rw`.
    ReadWrite,
}

impl MountMode {
    /// Schreibbar oder nicht.
    pub(crate) const fn of(writable: bool) -> Self {
        if writable {
            Self::ReadWrite
        } else {
            Self::ReadOnly
        }
    }
}

/// Die drei Flags, die bestimmen, wozu eine Einhängung taugt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) struct MountFlags {
    /// `nosuid`.
    pub(crate) nosuid: bool,
    /// `nodev`.
    pub(crate) nodev: bool,
    /// `noexec`.
    pub(crate) noexec: bool,
}

impl MountFlags {
    /// `nosuid,nodev`.
    pub(crate) const NOSUID_NODEV: Self = Self {
        nosuid: true,
        nodev: true,
        noexec: false,
    };

    /// `nosuid,nodev,noexec`.
    pub(crate) const ALL: Self = Self {
        nosuid: true,
        nodev: true,
        noexec: true,
    };

    /// Was in beiden steht.
    #[must_use]
    pub(crate) const fn union(self, other: Self) -> Self {
        Self {
            nosuid: self.nosuid || other.nosuid,
            nodev: self.nodev || other.nodev,
            noexec: self.noexec || other.noexec,
        }
    }

    fn text(self) -> String {
        [
            (self.nosuid, "nosuid"),
            (self.nodev, "nodev"),
            (self.noexec, "noexec"),
        ]
        .into_iter()
        .filter_map(|(set, name)| set.then_some(name))
        .collect::<Vec<_>>()
        .join(",")
    }
}

/// Was die Einhängung auf dem Host trägt, auf der eine Quelle liegt: Ein
/// Bind erbt das, in einem Nutzer-Namensraum unabänderlich.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct HostMount {
    /// Nur lesbar eingehängt.
    pub(crate) read_only: bool,
    /// Die drei Flags.
    pub(crate) flags: MountFlags,
}

/// Ein Eintrag der erwarteten Tabelle, mit seiner Anzahl.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExpectedMount {
    /// Der Einhängepunkt.
    pub(crate) point: PathBuf,
    /// Der Typ; `None` ist beliebig (ein Bind: sein Gerät legt den Typ
    /// schon fest).
    pub(crate) fstype: Option<String>,
    /// Die Quelle.
    pub(crate) source: MountSource,
    /// Nur lesbar oder schreibbar.
    pub(crate) mode: MountMode,
    /// Die drei Flags; `None` ist beliebig, nur für eigene Einträge eines
    /// Backends, deren Flags es vom Host übernimmt.
    pub(crate) flags: Option<MountFlags>,
    /// Mindestens so oft.
    pub(crate) min: u32,
    /// Höchstens so oft.
    pub(crate) max: u32,
}

impl ExpectedMount {
    /// Ein Bind von `id` auf `point`.
    pub(crate) fn bind(point: &Path, id: FileId, mode: MountMode, flags: MountFlags) -> Self {
        Self {
            point: point.to_path_buf(),
            fstype: None,
            source: MountSource::Inode(id),
            mode,
            flags: Some(flags),
            min: 1,
            max: 1,
        }
    }

    /// Eine Einhängung vom Typ `fstype` auf `point`.
    pub(crate) fn typed(point: &Path, fstype: &str, mode: MountMode, flags: MountFlags) -> Self {
        Self {
            point: point.to_path_buf(),
            fstype: Some(fstype.to_owned()),
            source: MountSource::Type,
            mode,
            flags: Some(flags),
            min: 1,
            max: 1,
        }
    }

    /// Derselbe Eintrag, aber höchstens einmal und vielleicht gar nicht:
    /// ein Pfad, den das Backend nur einhängt, wenn es ihn auf dem Host gibt.
    #[must_use]
    pub(crate) const fn optional(mut self) -> Self {
        self.min = 0;
        self.max = 1;
        self
    }

    /// Derselbe Eintrag mit beliebigem Typ: für einen Punkt, dessen Typ vom
    /// Host abhängt (das Terminal hinter `/dev/console`, die Dateien von
    /// einer Container-Engine unter ihrer `runroot`). Punkt, Modus und Anzahl bleiben
    /// verlangt; ein zweiter Mount auf demselben Punkt ist weiter zu viel.
    #[must_use]
    pub(crate) fn any_type(mut self) -> Self {
        self.fstype = None;
        self
    }

    /// Derselbe Eintrag mit beliebigen Flags.
    #[must_use]
    pub(crate) const fn any_flags(mut self) -> Self {
        self.flags = None;
        self
    }

    fn json(&self) -> serde_json::Value {
        let source = match (self.source, &self.fstype) {
            (MountSource::Inode(id), _) => format!("inode:{}:{}", id.dev, id.ino),
            (MountSource::Type, Some(_)) => "type".to_owned(),
            // [`Self::any_type`]: Ohne Typ ist die Quelle nicht zu prüfen;
            // Punkt, Modus, Flags und Anzahl prüft der Shim weiter.
            (MountSource::Type, None) => "any".to_owned(),
        };
        let mode = match self.mode {
            MountMode::ReadOnly => "ro",
            MountMode::ReadWrite => "rw",
        };
        let count = if self.min == self.max {
            self.min.to_string()
        } else {
            format!("{}-{}", self.min, self.max)
        };
        serde_json::json!({
            "point": self.point.to_string_lossy(),
            "type": self.fstype.as_deref().unwrap_or("*"),
            "source": source,
            "mode": mode,
            "flags": self.flags.map_or_else(|| "*".to_owned(), MountFlags::text),
            "count": count,
        })
    }
}

/// Alles, was der Shim hinter `--pins-fd` bekommt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct MountPins {
    /// Die geprüften Ziele.
    pub(crate) pins: Vec<Pin>,
    /// Die erwartete Tabelle; gleiche Einträge zählen zusammen.
    pub(crate) mounts: Vec<ExpectedMount>,
}

impl MountPins {
    /// Fügt einen Eintrag hinzu; ein gleicher erhöht dessen Anzahl.
    pub(crate) fn expect(&mut self, entry: ExpectedMount) {
        if let Some(same) = self.mounts.iter_mut().find(|known| {
            known.point == entry.point
                && known.fstype == entry.fstype
                && known.source == entry.source
                && known.mode == entry.mode
                && known.flags == entry.flags
        }) {
            same.min += entry.min;
            same.max += entry.max;
        } else {
            self.mounts.push(entry);
        }
    }

    /// Das JSON aus der Beschreibung von `mounts.rs` im Shim: jeder Wert
    /// ein String, damit der kleine Leser des Shims keine Zahlen kennen muss.
    pub(crate) fn to_json(&self) -> String {
        let pins: Vec<serde_json::Value> = self
            .pins
            .iter()
            .map(|pin| {
                serde_json::json!({
                    "path": pin.path.to_string_lossy(),
                    "dev": pin.id.dev.to_string(),
                    "ino": pin.id.ino.to_string(),
                })
            })
            .collect();
        let mounts: Vec<serde_json::Value> = self.mounts.iter().map(ExpectedMount::json).collect();
        serde_json::json!({ "pins": pins, "mounts": mounts }).to_string()
    }

    /// Der versiegelte memfd für den Shim.
    ///
    /// # Errors
    ///
    /// `SANDBOX_011`, wenn er sich nicht anlegen, füllen oder versiegeln
    /// lässt, und für einen Pfad, der kein UTF-8 ist: Der Shim verglich ihn
    /// sonst mit einem anderen.
    pub(crate) fn memfd(&self) -> Result<OwnedFd, Diagnostic> {
        let not_utf8 = self
            .pins
            .iter()
            .map(|pin| pin.path.as_path())
            .chain(self.mounts.iter().map(|entry| entry.point.as_path()))
            .find(|path| path.to_str().is_none());
        if let Some(path) = not_utf8 {
            return Err(not_prepared(format!(
                "the mount point {} is not UTF-8, so the shim could not compare it with the mount \
                 table inside the sandbox",
                path.display()
            )));
        }
        crate::platform::sealed_memfd(PINS_MEMFD_NAME, self.to_json().as_bytes())
            .map_err(not_prepared)
    }
}

/// `SANDBOX_011`: Die Tabelle für den Shim ließ sich nicht vorbereiten.
fn not_prepared(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(why)
        .fix(humanitl_core::FixAction::CopyCommand(
            "humanitl doctor".to_owned(),
        ))
        .build()
}

// `st_dev` und `st_ino` sind nicht auf jedem System `u64` (macOS: `i32`).
#[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
fn id_of(stat: &rustix::fs::Stat) -> FileId {
    FileId {
        dev: u64::try_from(stat.st_dev).unwrap_or(u64::MAX),
        ino: u64::try_from(stat.st_ino).unwrap_or(u64::MAX),
    }
}

/// Gerät und Inode eines geöffneten Objekts.
///
/// # Errors
///
/// `SANDBOX_011` mit `what`, wenn `fstat` scheitert.
pub(crate) fn id_of_fd(fd: impl AsFd, what: &Path) -> Result<FileId, Diagnostic> {
    rustix::fs::fstat(fd)
        .map(|stat| id_of(&stat))
        .map_err(|err| not_inspected(what, err))
}

/// Gerät und Inode dessen, was unter `path` liegt; einem Verweis wird
/// gefolgt, wie das Einhängen ihm folgt.
///
/// # Errors
///
/// `SANDBOX_011`, wenn `stat` scheitert.
pub(crate) fn id_of_path(path: &Path) -> Result<FileId, Diagnostic> {
    rustix::fs::stat(path)
        .map(|stat| id_of(&stat))
        .map_err(|err| not_inspected(path, err))
}

fn not_inspected(path: &Path, err: rustix::io::Errno) -> Diagnostic {
    not_prepared(format!(
        "cannot read device and inode of {} for the mount check ({err}); without them the shim \
         could not tell the checked object from another",
        path.display()
    ))
}

/// Was die Einhängung auf dem Host trägt, auf der `path` liegt.
///
/// # Errors
///
/// `SANDBOX_011`, wenn `statvfs` scheitert.
pub(crate) fn host_mount(path: &Path) -> Result<HostMount, Diagnostic> {
    use rustix::fs::StatVfsMountFlags as F;
    let stat = rustix::fs::statvfs(path).map_err(|err| not_inspected(path, err))?;
    let flags = stat.f_flag;
    Ok(HostMount {
        read_only: flags.contains(F::RDONLY),
        flags: MountFlags {
            nosuid: flags.contains(F::NOSUID),
            nodev: host_nodev(flags),
            noexec: host_noexec(flags),
        },
    })
}

#[cfg(target_os = "linux")]
fn host_nodev(flags: rustix::fs::StatVfsMountFlags) -> bool {
    flags.contains(rustix::fs::StatVfsMountFlags::NODEV)
}

#[cfg(target_os = "linux")]
fn host_noexec(flags: rustix::fs::StatVfsMountFlags) -> bool {
    flags.contains(rustix::fs::StatVfsMountFlags::NOEXEC)
}

// Unter macOS gibt es kein Backend mit Mounttabelle; die Flags dort zählen
// nicht.
#[cfg(not(target_os = "linux"))]
const fn host_nodev(_flags: rustix::fs::StatVfsMountFlags) -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
const fn host_noexec(_flags: rustix::fs::StatVfsMountFlags) -> bool {
    false
}

/// Die geprüften Ziele eines Plans (Spezifikation HUM-437, Punkt 1): `/work`
/// aus dem geöffneten Projektordner, die aus `mounts.ro`, `extra_ro`,
/// `extra_rw` und den Binds der Sitzung und das Agentenprogramm, jedes aus
/// seinem geöffneten und am Ort des Deskriptors geprüften Deskriptor
/// ([`PinnedMounts::open`]), nie über einen zweiten Blick auf den Pfad.
///
/// `work` ist `false`, wenn `/work` kein Bind ist (die obere Schicht einer
/// Wegwerf-Sandbox): Ein Overlay trägt sein eigenes Gerät, dort gilt die
/// Regel aus HUM-438. `agent` ist das Ziel des Agentenprogramms.
///
/// # Errors
///
/// `SANDBOX_011`, wenn sich Gerät und Inode einer Quelle nicht lesen lassen.
pub(crate) fn pins_of(
    profile: &SandboxProfile,
    pinned: &PinnedMounts,
    agent: Option<&Path>,
    work: bool,
) -> Result<Vec<Pin>, Diagnostic> {
    let opened = |source: &PinnedSource, dst: &Path| -> Result<Pin, Diagnostic> {
        Ok(Pin {
            path: dst.to_path_buf(),
            id: id_of_fd(source.fd(), source.resolved())?,
            host: source.resolved().to_path_buf(),
        })
    };
    let mut pins = Vec::new();
    if work {
        pins.push(opened(&pinned.work, &profile.mounts.work.dst)?);
    }
    for (source, dst) in pinned.profile_ro.iter().zip(&profile.mounts.ro) {
        pins.push(opened(source, dst)?);
    }
    for (source, dst) in pinned.read_only.iter().zip(&profile.mounts.extra_ro) {
        pins.push(opened(source, dst)?);
    }
    for (source, dst) in pinned.writable.iter().zip(&profile.mounts.extra_rw) {
        pins.push(opened(source, dst)?);
    }
    for (source, dst) in &pinned.session {
        pins.push(opened(source, dst)?);
    }
    if let (Some(source), Some(dst)) = (pinned.agent.as_ref(), agent) {
        pins.push(opened(source, dst)?);
    }
    Ok(pins)
}

/// Gerät, Inode und Ort auf dem Host je Ziel, für die Binds der Tabelle: aus
/// den geöffneten Deskriptoren, damit Tabelle und geprüfte Ziele vom selben
/// Objekt reden.
pub(crate) fn by_target(pins: &[Pin]) -> BTreeMap<PathBuf, (FileId, PathBuf)> {
    pins.iter()
        .map(|pin| (pin.path.clone(), (pin.id, pin.host.clone())))
        .collect()
}

/// Ein Bind auf `target` aus der Quelle `source`, mit Gerät und Inode aus
/// `known`, wo das Ziel geprüft ist, sonst aus dem Pfad; Modus und Flags aus
/// `writable`, dem, was das Backend jedem Bind gibt (`forced`), und der
/// Einhängung der Quelle auf dem Host.
///
/// # Errors
///
/// `SANDBOX_011`, wenn sich die Quelle nicht lesen lässt.
pub(crate) fn bind_of(
    target: &Path,
    source: &Path,
    writable: bool,
    forced: MountFlags,
    known: &BTreeMap<PathBuf, (FileId, PathBuf)>,
) -> Result<ExpectedMount, Diagnostic> {
    let (id, host) = match known.get(target) {
        Some((id, host)) => (*id, host.clone()),
        None => (id_of_path(source)?, source.to_path_buf()),
    };
    let mount = host_mount(&host)?;
    Ok(ExpectedMount::bind(
        target,
        id,
        MountMode::of(writable && !mount.read_only),
        forced.union(mount.flags),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn equal_entries_count_together_and_the_json_has_the_shims_shape() {
        let mut pins = MountPins {
            pins: vec![Pin {
                path: "/work".into(),
                id: FileId {
                    dev: 2050,
                    ino: 131,
                },
                host: "/home/u/proj".into(),
            }],
            mounts: Vec::new(),
        };
        let tmpfs = ExpectedMount::typed(
            Path::new("/tmp"),
            "tmpfs",
            MountMode::ReadWrite,
            MountFlags::NOSUID_NODEV,
        );
        pins.expect(tmpfs.clone());
        pins.expect(tmpfs);
        pins.expect(ExpectedMount::bind(
            Path::new("/work"),
            FileId {
                dev: 2050,
                ino: 131,
            },
            MountMode::ReadWrite,
            MountFlags::NOSUID_NODEV,
        ));
        pins.expect(
            ExpectedMount::typed(
                Path::new("/dev/console"),
                "devpts",
                MountMode::ReadWrite,
                MountFlags::default(),
            )
            .any_flags()
            .optional(),
        );
        assert_eq!(pins.mounts.len(), 3);
        assert_eq!((pins.mounts[0].min, pins.mounts[0].max), (2, 2));
        let json: serde_json::Value = serde_json::from_str(&pins.to_json()).unwrap();
        assert_eq!(
            json["pins"][0],
            serde_json::json!({"path": "/work", "dev": "2050", "ino": "131"})
        );
        assert_eq!(
            json["mounts"][0],
            serde_json::json!({"point": "/tmp", "type": "tmpfs", "source": "type",
                "mode": "rw", "flags": "nosuid,nodev", "count": "2"})
        );
        assert_eq!(json["mounts"][1]["source"], "inode:2050:131");
        assert_eq!(json["mounts"][1]["type"], "*");
        assert_eq!(
            json["mounts"][2],
            serde_json::json!({"point": "/dev/console", "type": "devpts", "source": "type",
                "mode": "rw", "flags": "*", "count": "0-1"})
        );
    }

    /// Die zweite Garantie trägt die Ursache: ein getauschter Ordner
    /// `SANDBOX_042`, eine abweichende Tabelle `SANDBOX_049`. Hält der Shim an
    /// der Mounttabelle an, nennen die beiden anderen Garantien dieselbe
    /// Ursache; ohne die beiden Zeilen bleibt die zweite unter Linux unbelegt.
    #[test]
    #[cfg(target_os = "linux")]
    fn the_door_guarantee_names_the_mount_check_as_its_cause() {
        use std::sync::Arc;
        use std::time::Duration;

        use crate::bridge_env::ShimCheck;
        use crate::handle::{SandboxHandle, Shared};
        use crate::launcher::IsolationCheck;

        let run = |lines: &[(&str, bool)]| {
            let shared = Arc::new(Shared::default());
            for (name, ok) in lines {
                shared.push_check(ShimCheck {
                    name: (*name).to_owned(),
                    ok: *ok,
                    evidence: "e".to_owned(),
                });
            }
            shared.close_report();
            let handle = SandboxHandle::new(
                humanitl_core::ids::SandboxId::nil(),
                0,
                String::new(),
                shared,
            );
            crate::bwrap::isolation_from_report(&handle, Duration::from_millis(50))
        };
        let code = |results: &[crate::CheckResult], check: IsolationCheck| {
            results
                .iter()
                .find(|result| result.check == check)
                .and_then(|result| result.diagnostic.as_ref())
                .map(|diagnostic| diagnostic.code.as_str().to_owned())
        };
        let five = [
            ("bridge_listening", true),
            ("no_interfaces", true),
            ("single_socket", true),
            ("seccomp_applied", true),
            ("families", true),
        ];
        let mut all_ok = vec![("mounts_match", true), ("mounts_pinned", true)];
        all_ok.extend(five);
        assert!(run(&all_ok).iter().all(|result| result.passed));

        let mut swapped = vec![("mounts_match", true), ("mounts_pinned", false)];
        swapped.extend(five);
        let results = run(&swapped);
        assert_eq!(
            code(&results, IsolationCheck::SingleSocket).as_deref(),
            Some("SANDBOX_042")
        );
        assert!(results[0].passed && results[2].passed);

        let results = run(&[("mounts_match", false), ("mounts_pinned", true)]);
        for check in IsolationCheck::ALL {
            assert_eq!(
                code(&results, check).as_deref(),
                Some("SANDBOX_049"),
                "{check:?}"
            );
        }

        #[cfg(target_os = "linux")]
        {
            let results = run(&five);
            assert!(!results[1].passed, "no mount lines, no second guarantee");
            assert_eq!(
                code(&results, IsolationCheck::SingleSocket).as_deref(),
                Some("SANDBOX_015")
            );
        }
    }

    /// Die Ziele aus `mounts.ro` tragen Gerät und Inode des geöffneten
    /// Deskriptors: Wird der Pfad nach dem Öffnen getauscht, bleibt das Paar
    /// das des Geprüften, und der Shim sieht den Tausch (`SANDBOX_042`).
    /// Ein zweiter Blick auf den Pfad nähme das untergeschobene Objekt auf.
    #[test]
    fn read_only_profile_pins_come_from_the_opened_descriptor() {
        use crate::pinned::SourceKind;

        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        std::fs::create_dir(&work).unwrap();
        let tools = dir.path().join("tools");
        std::fs::create_dir(&tools).unwrap();
        let open = |path: &Path, kind| PinnedSource::open(path, kind).unwrap();
        let pinned = PinnedMounts {
            work: open(&work, SourceKind::Directory),
            read_only: Vec::new(),
            writable: Vec::new(),
            session: Vec::new(),
            agent: None,
            profile_ro: vec![open(&tools, SourceKind::DirectoryOrFile)],
        };
        let checked = id_of_path(&tools).unwrap();
        std::fs::rename(&tools, dir.path().join("tools.checked")).unwrap();
        std::fs::create_dir(&tools).unwrap();
        let swapped = id_of_path(&tools).unwrap();
        assert_ne!(checked, swapped);

        let mut profile = SandboxProfile::parse(
            include_str!("../../../../profiles/sandbox/default.toml"),
            Path::new("default.toml"),
        )
        .unwrap();
        profile.mounts.ro = vec![tools.clone()];
        let pins = pins_of(&profile, &pinned, None, true).unwrap();
        let pin = pins
            .iter()
            .find(|pin| pin.path == tools)
            .expect("the ro pin");
        assert_eq!(
            pin.id, checked,
            "the pin is the opened object, not the path"
        );
    }

    #[test]
    fn device_and_inode_come_from_the_object_and_flags_from_its_mount() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, "x").unwrap();
        let by_path = id_of_path(&file).unwrap();
        let opened = std::fs::File::open(&file).unwrap();
        assert_eq!(id_of_fd(&opened, &file).unwrap(), by_path);
        let err = id_of_path(&dir.path().join("missing")).unwrap_err();
        assert_eq!(err.code.as_str(), "SANDBOX_011", "{}", err.why);
        assert!(host_mount(dir.path()).is_ok());
        assert_eq!(
            MountFlags::NOSUID_NODEV.union(MountFlags {
                noexec: true,
                ..MountFlags::default()
            }),
            MountFlags::ALL
        );
    }
}
