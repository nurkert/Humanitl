//! Die Grenzen der Unit einer Sitzung an Speicher und Tasks (HUM-485).
//!
//! Die Unit einer Sitzung der Kernel-Stufe begrenzt seit HUM-469 die
//! Deskriptoren jedes Prozesses (`LimitNOFILE`). Speicher und Tasks bindet sie
//! über ihre cgroup: `MemoryMax=`, `MemorySwapMax=` und `TasksMax=`. Ein
//! übernommener Helfer (ein Dienst der Dateien, das vsock-Gerät, der
//! Emulator) bindet damit höchstens einen festen Teil des Rechners, auch wenn
//! er Speicher anfordert oder Prozesse startet, bis es nicht mehr geht.
//!
//! **Was die cgroup der Unit zählt.** Gemessen an der cgroup echter
//! Sitzungen (`memory.stat`, `pids.peak`; 2026-11-15, QEMU 11.1, virtiofsd
//! 1.14, Host-Kernel 7.3; der KVM-Test `the_limits_of_hum_485_measured`, die
//! übrigen KVM-Tests und die Escape-Suite):
//!
//! - Der Speicher des Gasts ist ein memfd (`memory-backend-memfd`), also
//!   `shmem` der Unit, sobald der Gast eine Seite berührt; höchstens so groß
//!   wie die Maschine.
//! - Die Helfer selbst: `anon` höchstens 59 MiB, Kernel-Speicher (Slab,
//!   Seitentabellen) höchstens 96 MiB, beides bei einem Walk über die ganze
//!   Wurzel mit 4 vCPU und 4096 MiB; mit einem virtiofsd an seiner Grenze von
//!   65 536 Deskriptoren 54 MiB Kernel-Speicher.
//! - Der Seiten-Cache der Dateien, die virtiofsd für den Gast liest (bis
//!   6 GiB beim Lesen von `/usr`). Er zählt mit, ist aber zurückholbar: An der
//!   Grenze gibt der Kernel ihn frei, statt einen Prozess zu beenden. Unter
//!   der Grenze ohne tmpfs-Anteil (4608 MiB bei 4096 MiB und 4 vCPU,
//!   1024 MiB bei 512 MiB und 1 vCPU) liefen alle Lasten der Messung durch,
//!   auch zwei tmpfs des Gasts zu 85 % seines Speichers, mit `memory.peak`
//!   an der Grenze und ohne Swap.
//! - Was die Unit auf ein tmpfs schreibt, etwa in den Projektordner, wenn er
//!   auf einem tmpfs liegt, und immer in die Schicht einer Wegwerf-Sitzung,
//!   die im Laufzeitverzeichnis im Arbeitsspeicher liegt (HUM-446). Das ist
//!   `shmem` und ohne Swap nicht zurückholbar. Damit eine Wegwerf-Sitzung
//!   nicht an ihrer eigenen Schicht stirbt, bekommt die Unit die Größe jedes
//!   solchen tmpfs dazu ([`in_ram_mib`]); dessen eigene Größe bleibt die
//!   Grenze dieser Schreibvorgänge.
//! - Tasks (Prozesse und Fäden): 28 bis 34 ohne die vCPU, je vCPU ein Faden
//!   des Emulators mehr (32 mit 4 vCPU, 35 in einer Wegwerf-Sitzung mit
//!   1 vCPU). Je Helfer zwei bubblewrap (außen und der Init seines
//!   PID-Namensraums), sechs bis sieben `sh`, je virtiofsd drei bis vier
//!   Fäden (die Zeile setzt keinen `--thread-pool-size`), das vsock-Gerät vier,
//!   der Emulator zwei samt einem Kernel-Faden von KVM.

use std::os::fd::BorrowedFd;
use std::path::Path;
use std::time::Duration;

use humanitl_core::diagnostics::codes::SANDBOX_052;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::user_unit::UserUnits;

use super::lines::{Machine, WORK_MASK_MIB, WorkMask};

/// Der feste Zuschlag an Speicher für die Helfer einer Sitzung, über dem
/// Speicher der Maschine: das Dreifache des Gemessenen (`anon` und
/// Kernel-Speicher zusammen höchstens rund 160 MiB), damit auch drei Dienste
/// der Dateien an ihrer Grenze der Deskriptoren (je rund 55 MiB
/// Kernel-Speicher) und der Emulator mit seinen Puffern Platz haben. Der
/// Seiten-Cache braucht keinen Zuschlag; ihn gibt der Kernel an der Grenze
/// frei.
pub(crate) const HELPER_MEMORY_MIB: u64 = 512;

/// Die festen Tasks der Helfer einer Sitzung, über den vCPU der Maschine:
/// gemessen höchstens 34, also fast das Vierfache. Der Rest deckt den Pool
/// der Arbeitsfäden des Emulators (höchstens 64, für Dateien und Blöcke, die
/// diese Maschine nicht hat) und Fäden einer künftigen Fassung eines
/// Helfers. Ein Helfer, der mehr Prozesse oder Fäden startet, bekommt
/// `EAGAIN`; der Rechner merkt davon nichts.
pub(crate) const HELPER_TASKS: u64 = 128;

/// Die cgroup-Controller, ohne die die Grenzen der Unit nichts binden.
const CONTROLLERS: [&str; 2] = ["memory", "pids"];

/// Der Eintrag, der die Controller an die Sitzungen der Nutzer delegiert;
/// derselbe wie beim Container-Backend (`SANDBOX_040`).
const DELEGATE_COMMAND: &str = "sudo mkdir -p /etc/systemd/system/user@.service.d && printf \
     '[Service]\\nDelegate=cpu cpuset io memory pids\\n' | sudo tee \
     /etc/systemd/system/user@.service.d/delegate.conf && sudo systemctl daemon-reload";

/// Die Grenzen der Unit einer Sitzung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnitLimits {
    /// `MemoryMax=` in MiB: der Speicher der Maschine, der Zuschlag der
    /// Helfer und die tmpfs, auf die die Unit schreibt.
    pub(crate) memory_max_mib: u64,
    /// `TasksMax=`: die vCPU und die Tasks der Helfer.
    pub(crate) tasks_max: u64,
}

impl UnitLimits {
    /// Die Grenzen für die Maschine `machine`, wenn die Unit auf tmpfs mit
    /// zusammen `in_ram_mib` MiB schreibt.
    pub(crate) fn of(machine: Machine, in_ram_mib: u64) -> Self {
        Self {
            memory_max_mib: machine
                .memory_mib
                .saturating_add(HELPER_MEMORY_MIB)
                .saturating_add(in_ram_mib),
            tasks_max: u64::from(machine.cpus).saturating_add(HELPER_TASKS),
        }
    }

    /// Was die cgroup der Unit danach trägt: Datei und Wert, `memory.max` und
    /// `memory.swap.max` in Bytes, `pids.max` als Zahl.
    pub(crate) fn cgroup_values(self) -> Vec<(&'static str, u64)> {
        let bytes = self.memory_max_mib.saturating_mul(1024 * 1024);
        vec![
            ("memory.max", bytes),
            ("memory.swap.max", bytes),
            ("pids.max", self.tasks_max),
        ]
    }

    /// Die Eigenschaften der Unit.
    ///
    /// **Swap.** `MemorySwapMax` ist so groß wie `MemoryMax`, nicht 0 und
    /// nicht offen: Offen könnte ein Helfer, der Speicher anfordert, an der
    /// Grenze den ganzen Swap des Rechners füllen, und die Sitzung bände mehr
    /// als eine feste Menge. Mit 0 wäre der Speicher des Gasts nie
    /// auslagerbar; auf einem kleinen Rechner, auf dem die Maschine die Hälfte
    /// des Speichers bekommt, verdrängte eine ruhende Sitzung dann alle anderen
    /// Programme, und unter Druck beendete der Kernel eher die Sitzung, als
    /// eine Seite des Gasts auszulagern. Gleich groß wie der Speicher ist auch
    /// die Vorgabe der Container-Engine für eine Speichergrenze ohne eigene
    /// Grenze des Swaps.
    pub(crate) fn properties(self) -> [String; 3] {
        [
            format!("MemoryMax={}M", self.memory_max_mib),
            format!("MemorySwapMax={}M", self.memory_max_mib),
            format!("TasksMax={}", self.tasks_max),
        ]
    }
}

/// Wie viel MiB die beschreibbaren Ordner `dirs` höchstens im
/// Arbeitsspeicher halten: die Größe jedes tmpfs, auf dem einer liegt, jedes
/// Dateisystem nur einmal. Ein Ordner auf der Platte zählt nicht; was dort
/// geschrieben wird, ist Seiten-Cache, den der Kernel an der Grenze
/// zurückschreibt und freigibt. Ein Ordner, dessen Dateisystem sich nicht
/// lesen lässt, zählt ebenfalls nicht: Die Grenze wird dadurch enger, nie
/// weiter.
pub(crate) fn in_ram_mib(dirs: &[BorrowedFd<'_>]) -> u64 {
    let mut seen: Vec<u64> = Vec::new();
    let mut total: u64 = 0;
    for dir in dirs {
        let (Ok(stat), Ok(meta)) = (rustix::fs::fstatfs(*dir), rustix::fs::fstat(*dir)) else {
            continue;
        };
        if !crate::disposable::is_tmpfs(&stat) || seen.contains(&meta.st_dev) {
            continue;
        }
        seen.push(meta.st_dev);
        #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
        let bytes = u64::try_from(stat.f_blocks)
            .unwrap_or(0)
            .saturating_mul(u64::try_from(stat.f_bsize).unwrap_or(0));
        total = total.saturating_add(bytes / (1024 * 1024));
    }
    total
}

/// Wie viel MiB die Masken unter `/work` höchstens im Arbeitsspeicher halten:
/// jedes tmpfs mit seiner festen Größe ([`WORK_MASK_MIB`]). Gezählt wird
/// immer, auch wenn der Dienst nur lesbar ausliefert: Die tmpfs sind im
/// Gefängnis des Dienstes beschreibbar, und ein übernommener Dienst könnte
/// sie füllen. Die leeren tmpfs im Gefängnis der Wurzel
/// ([`super::lines::ROOT_TMPFS_MIB`] je Stück) deckt der feste Zuschlag der
/// Helfer.
pub(crate) fn work_masks_mib(masks: &[WorkMask]) -> u64 {
    let tmpfs = masks
        .iter()
        .filter(|mask| matches!(mask, WorkMask::Tmpfs(_)))
        .count();
    u64::try_from(tmpfs)
        .unwrap_or(u64::MAX)
        .saturating_mul(WORK_MASK_MIB)
}

/// Prüft, dass der Dienstverwalter des Nutzers die Controller `memory` und
/// `pids` hat, bevor eine Unit startet: Fehlen sie, übergeht systemd
/// `MemoryMax=` und `TasksMax=` ohne Meldung, und die Sitzung liefe ohne ihre
/// Grenzen.
///
/// # Errors
///
/// `SANDBOX_052` mit dem, was fehlt.
pub(crate) fn check_controllers(units: &UserUnits) -> Result<(), Diagnostic> {
    let group = crate::user_unit::run_bounded(
        units.manager_show_command("ControlGroup"),
        Duration::from_secs(10),
    )
    .map_err(|err| format!("systemctl --user did not name the manager's cgroup: {err}"))
    .and_then(|output| {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if output.status.success() && text.starts_with('/') {
            Ok(text)
        } else {
            Err(format!(
                "the user's service manager named no cgroup of its own ({text:?})"
            ))
        }
    });
    judge_controllers(group, |path| std::fs::read_to_string(path))
}

/// [`check_controllers`] mit der cgroup des Dienstverwalters (`group`) und
/// einer Funktion, die eine Datei unter `/sys/fs/cgroup` liest.
fn judge_controllers(
    group: Result<String, String>,
    read: impl Fn(&Path) -> std::io::Result<String>,
) -> Result<(), Diagnostic> {
    let group = group.map_err(|why| limits_unenforced(&why))?;
    let path = Path::new("/sys/fs/cgroup")
        .join(group.trim_start_matches('/'))
        .join("cgroup.controllers");
    let text = read(&path).map_err(|err| {
        limits_unenforced(&format!(
            "{} cannot be read ({err}); the limits need the unified cgroup v2 hierarchy \
             (cgroup2 at /sys/fs/cgroup), and without it the unit has no limits",
            path.display()
        ))
    })?;
    let missing: Vec<&str> = CONTROLLERS
        .iter()
        .copied()
        .filter(|wanted| !text.split_whitespace().any(|have| have == *wanted))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(limits_unenforced(&format!(
        "the cgroup controllers {} are not delegated to the user's service manager \
         (delegated: [{}])",
        missing.join(", "),
        text.trim()
    )))
}

/// Prüft an der cgroup der laufenden Unit (`dir`), dass sie die geplanten
/// Grenzen trägt ([`UnitLimits::cgroup_values`]): Die Prüfung vor dem Start
/// sieht nur, welche Controller der Dienstverwalter hat, nicht, ob sie bis zur
/// Unit reichen; ein `DisableControllers=` auf einem Slice dazwischen nähme
/// sie ihr still. Eine fehlende Datei oder ein anderer Wert ist ein Grund,
/// den Agenten nie zu starten.
///
/// # Errors
///
/// Ein Satz mit der Datei und dem, was dort steht oder fehlt.
pub(crate) fn judge_cgroup(dir: &Path, wanted: &[(&str, u64)]) -> Result<(), String> {
    for (file, value) in wanted {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("the unit's {} cannot be read ({err})", path.display()))?;
        if text.trim() != value.to_string() {
            return Err(format!(
                "the unit's {file} is {:?}, and the plan set {value}",
                text.trim()
            ));
        }
    }
    Ok(())
}

/// `SANDBOX_052`: Die Grenzen der Unit an Speicher und Tasks hielten nicht.
pub(crate) fn limits_unenforced(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_052, Severity::Blocking)
        .why(format!(
            "{why}; the unit of a kernel-level session runs with MemoryMax and TasksMax, which \
             would not hold, and a session without its limits does not start"
        ))
        .fix(FixAction::CopyCommand(DELEGATE_COMMAND.to_owned()))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::fd::AsFd as _;

    use super::*;

    /// Die Grenzen folgen der Maschine: Speicher plus Zuschlag plus tmpfs,
    /// vCPU plus Tasks der Helfer; Swap so groß wie der Speicher.
    #[test]
    fn the_limits_follow_the_machine_and_the_writes_in_ram() {
        let machine = Machine {
            cpus: 4,
            memory_mib: 4096,
        };
        let limits = UnitLimits::of(machine, 0);
        assert_eq!(limits.memory_max_mib, 4096 + HELPER_MEMORY_MIB);
        assert_eq!(limits.tasks_max, 4 + HELPER_TASKS);
        assert_eq!(
            limits.properties(),
            [
                "MemoryMax=4608M".to_owned(),
                "MemorySwapMax=4608M".to_owned(),
                "TasksMax=132".to_owned(),
            ]
        );
        assert_eq!(UnitLimits::of(machine, 3200).memory_max_mib, 4608 + 3200);
    }

    /// Jede tmpfs-Maske zählt mit ihrer festen Größe, eine leere Datei nicht,
    /// und nur lesbar zählt keine.
    #[test]
    fn each_writable_tmpfs_mask_counts_with_its_size() {
        let masks = [
            WorkMask::Tmpfs(".git/hooks".into()),
            WorkMask::Empty(".env".into()),
            WorkMask::Tmpfs(".vscode".into()),
        ];
        assert_eq!(work_masks_mib(&masks), 2 * WORK_MASK_MIB);
        assert_eq!(work_masks_mib(&[]), 0);
    }

    /// Ein Ordner auf der Platte zählt nicht, ein tmpfs mit seiner Größe und
    /// zweimal dasselbe tmpfs nur einmal.
    #[test]
    fn only_a_tmpfs_counts_and_each_only_once() {
        let disk = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let disk = std::fs::File::open(disk.path()).unwrap();
        let on_disk = rustix::fs::fstatfs(&disk).unwrap();
        if !crate::disposable::is_tmpfs(&on_disk) {
            assert_eq!(in_ram_mib(&[disk.as_fd()]), 0);
        }
        let Ok(shm) = std::fs::File::open("/dev/shm") else {
            return;
        };
        let stat = rustix::fs::fstatfs(&shm).unwrap();
        if !crate::disposable::is_tmpfs(&stat) {
            return;
        }
        #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
        let size = u64::try_from(stat.f_blocks).unwrap() * u64::try_from(stat.f_bsize).unwrap()
            / (1024 * 1024);
        assert!(size > 0);
        assert_eq!(in_ram_mib(&[shm.as_fd()]), size);
        let again = std::fs::File::open("/dev/shm").unwrap();
        assert_eq!(in_ram_mib(&[shm.as_fd(), again.as_fd()]), size);
    }

    /// Die cgroup der Unit trägt die Grenzen: bestanden. Ein anderer Wert
    /// (`max`, wenn der Controller die Unit nicht erreicht) oder eine fehlende
    /// Datei: abgelehnt, mit der Datei im Satz.
    #[test]
    fn the_unit_cgroup_must_carry_the_planned_limits() {
        let limits = UnitLimits::of(
            Machine {
                cpus: 2,
                memory_mib: 1024,
            },
            0,
        );
        let wanted = limits.cgroup_values();
        assert_eq!(
            wanted,
            [
                ("memory.max", 1536 * 1024 * 1024),
                ("memory.swap.max", 1536 * 1024 * 1024),
                ("pids.max", 130),
            ]
        );
        let dir = tempfile::tempdir().unwrap();
        let write = |file: &str, text: &str| std::fs::write(dir.path().join(file), text).unwrap();
        write("memory.max", "1610612736\n");
        write("memory.swap.max", "1610612736\n");
        write("pids.max", "130\n");
        assert_eq!(judge_cgroup(dir.path(), &wanted), Ok(()));
        write("pids.max", "max\n");
        let why = judge_cgroup(dir.path(), &wanted).unwrap_err();
        assert!(why.contains("pids.max is \"max\""), "{why}");
        write("pids.max", "130\n");
        std::fs::remove_file(dir.path().join("memory.swap.max")).unwrap();
        let why = judge_cgroup(dir.path(), &wanted).unwrap_err();
        assert!(why.contains("memory.swap.max cannot be read"), "{why}");
    }

    /// Beide Controller da: bestanden. Einer fehlt, die Datei fehlt oder der
    /// Dienstverwalter nennt keine cgroup: `SANDBOX_052` mit dem Grund und
    /// dem Eintrag, der delegiert.
    #[test]
    fn missing_controllers_refuse_the_start() {
        let group = || Ok("/user.slice/user-1000.slice/user@1000.service".to_owned());
        let answer = |text: &'static str| {
            move |path: &Path| {
                assert_eq!(
                    path,
                    Path::new(
                        "/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/\
                         cgroup.controllers"
                    )
                );
                Ok(text.to_owned())
            }
        };
        assert!(judge_controllers(group(), answer("cpu memory pids\n")).is_ok());
        assert!(judge_controllers(group(), answer("memory pids")).is_ok());

        let refused = judge_controllers(group(), answer("cpu memory\n")).unwrap_err();
        assert_eq!(refused.code.as_str(), "SANDBOX_052");
        assert!(
            refused.why.contains("controllers pids are not delegated"),
            "{}",
            refused.why
        );
        assert!(
            matches!(&refused.fix, Some(FixAction::CopyCommand(command))
                if command.contains("Delegate=cpu cpuset io memory pids")),
            "{:?}",
            refused.fix
        );
        let refused = judge_controllers(group(), answer("")).unwrap_err();
        assert!(refused.why.contains("memory, pids"), "{}", refused.why);

        let refused = judge_controllers(group(), |_: &Path| {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))
        })
        .unwrap_err();
        assert!(refused.why.contains("cannot be read"), "{}", refused.why);
        let refused = judge_controllers(Err("no answer".to_owned()), answer("")).unwrap_err();
        assert_eq!(refused.code.as_str(), "SANDBOX_052");
        assert!(refused.why.starts_with("no answer"), "{}", refused.why);
    }
}
