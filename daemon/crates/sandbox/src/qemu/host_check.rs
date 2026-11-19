//! Die Prüfung des VMM auf dem Host (HUM-444).
//!
//! Die `CHECK`-Zeilen kommen aus dem Gast, und Root im Gast kann sie
//! fälschen. Was ein Gast nicht fälschen kann, prüft der Daemon selbst, an
//! den Prozessen der Unit:
//!
//! - jeder Prozess der Unit ist eines ihrer Programme (HUM-468): die Shell
//!   des Skripts, bubblewrap, virtiofsd, das vsock-Gerät oder QEMU, erkannt
//!   an `/proc/<pid>/exe`; ein anderes, ein gelöschtes oder ein unlesbares
//!   Programm ist ein Fund;
//! - jedes dieser Programme läuft so oft, wie das Skript es startet
//!   (HUM-480, [`Roles`]): eine Shell mehr als die Helfer und die eine, die
//!   auf die Lebensleine wartet, zwei bubblewrap je Helfer, ein virtiofsd je
//!   Dienst der Dateien; ein zweites `bwrap` oder `sh` an der Stelle eines
//!   Helfers ist ein Fund. Was nach der Zulassung startet, sieht diese
//!   Prüfung nicht; dort hält die Unit selbst jedes andere Programm ab
//!   (`NoExecPaths=/`, [`super::programs`]);
//! - jeder Prozess der Unit hat in seinem Netz-Namensraum nur `lo`;
//! - kein Prozess hält `/dev/vhost-vsock`, `/dev/vhost-net`, `/dev/net/tun`
//!   oder ein tap-Gerät;
//! - QEMU hält nur die Deskriptoren aus HUM-442 Q3, ohne Konsole: KVM,
//!   Ereignis- und Signal-Deskriptoren, `io_uring`, den Speicher als memfd,
//!   `/dev/null`, Pipes und die Sockets von vhost-user;
//! - im Laufzeitverzeichnis liegen nur die erklärten Sockets
//!   ([`Layout::declared_sockets`]); ein weiterer `<uds>_<port>` wäre eine
//!   zweite Tür;
//! - es gibt genau einen Prozess des vsock-Geräts, und er bedient genau ein
//!   Gerät: ein `--socket`, kein `--vm`, keine `--config`, kein
//!   `--forward-cid`;
//! - jeder Ordner, den der Plan geöffnet und geprüft hat (der Projektordner,
//!   in einer Wegwerf-Sitzung die Schicht), wird von genau einem virtiofsd
//!   ausgeliefert, der ihn unter seinem `--shared-dir` teilt und das
//!   geöffnete Verzeichnis selbst hält: Gerät und Inode eines seiner
//!   Deskriptoren sind die des Plans (HUM-446). Ein Ordner, der nach der
//!   Prüfung getauscht und über einen Pfad eingehängt worden wäre, fiele hier
//!   auf, auch wenn Root im Gast die Prüfung des Shims fälschte.
//!
//! Jeder Fund nennt die Garantie, die er bricht; was sich nicht lesen lässt,
//! ist ein Fund und nie ein Bestehen.

use std::path::Path;

use humanitl_core::diagnostics::codes::SANDBOX_047;
use humanitl_core::{Diagnostic, Severity};

use crate::launcher::{CheckResult, IsolationCheck, SharedDir, UnitStop};

use super::lines::Layout;
use super::prerequisites::Resolved;

/// Was die Prüfung fand, der Reihe nach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    /// Die Garantie, die der Fund bricht.
    pub(crate) check: IsolationCheck,
    /// Was gefunden wurde, ohne Leerzeichen am Rand.
    pub(crate) what: String,
}

fn finding(check: IsolationCheck, what: impl Into<String>) -> Finding {
    Finding {
        check,
        what: what.into(),
    }
}

/// Trägt die Funde des Hosts in die Ergebnisse aus dem Bericht des Gasts
/// ein. Jede Garantie mit einem Fund ist rot, und ihr Befund nennt ihn
/// (`host: ...`).
///
/// **Welche Diagnose gilt (HUM-470).** Bestand der Bericht des Gasts, ist
/// jeder Fund des Hosts `SANDBOX_047`. Hat der Gast selbst schon abgelehnt
/// (eine rote Garantie aus seinem Bericht, oder gar kein Bericht), bleibt
/// seine Diagnose die der Sitzung: Der Shim hält dann an, schaltet den Gast
/// ab, und mit ihm endet die Unit, oft noch bevor der Host nachsieht. Was die
/// Host-Prüfung dann findet, eine Unit ohne cgroup, ohne Prozess, ohne
/// Emulator, ist die Folge dieser Ablehnung und nicht ihr Grund; mit
/// `SANDBOX_047` hätte sie die wahre Ursache verdeckt (`esc-2/unplanned_mount`
/// sah in der CI `SANDBOX_047` statt `SANDBOX_049`). Eine Garantie, die der
/// Gast bestanden hatte und bei der der Host etwas fand, wird rot und trägt
/// die erste Diagnose des Gasts, der Fund steht in ihrem Befund. Abgewiesen
/// wird in jedem Fall: Nur die angezeigte Ursache hängt an der Reihenfolge,
/// nie das Urteil.
pub(crate) fn merge(results: &mut [CheckResult], findings: &[Finding]) {
    let refused_by_guest = results
        .iter()
        .find(|result| !result.passed)
        .and_then(|result| Some((result.check, result.diagnostic.clone()?)));
    for result in results.iter_mut() {
        let own: Vec<&str> = findings
            .iter()
            .filter(|finding| finding.check == result.check)
            .map(|finding| finding.what.as_str())
            .collect();
        if own.is_empty() {
            continue;
        }
        let what = own.join("; ");
        result.evidence = format!("{}; host: {what}", result.evidence);
        if !result.passed && result.diagnostic.is_some() {
            continue;
        }
        result.passed = false;
        result.diagnostic = Some(match &refused_by_guest {
            // Die Diagnose einer anderen Garantie: Ihr Satz sagt, woher sie
            // kommt, damit er an dieser Garantie richtig liest.
            Some((origin, diagnostic)) => {
                let mut borrowed = diagnostic.clone();
                borrowed.why = format!(
                    "{}: the host found {what} while the guest had refused {}: {}",
                    result.check.as_str(),
                    origin.as_str(),
                    diagnostic.why
                );
                borrowed
            }
            None => Diagnostic::builder(SANDBOX_047, Severity::Blocking)
                .why(format!(
                    "{}: the host check of the VM monitor failed: {what}; the agent was not \
                     admitted and the session was stopped",
                    result.check.as_str()
                ))
                .build(),
        });
    }
}

/// `SANDBOX_052`, wenn die cgroup der laufenden Unit nicht die geplanten
/// Grenzen trägt (HUM-485), sonst keine. Was sich nicht lesen lässt, ist ein
/// Fund und nie ein Bestehen.
pub(crate) fn limits_unenforced(stop: &UnitStop) -> Option<Diagnostic> {
    unit_cgroup(stop)
        .and_then(|dir| super::limits::judge_cgroup(&dir, &stop.limits))
        .err()
        .map(|why| super::limits::limits_unenforced(&why))
}

/// Trägt `SANDBOX_052` (die Grenzen der Unit hielten nicht) in die
/// Ergebnisse ein. Ist schon eine Garantie rot (der Gast hat abgelehnt,
/// HUM-470, oder der Host fand etwas), bleibt deren Diagnose die der Sitzung,
/// und der Satz steht nur im Beleg jeder Garantie: Eine Unit, die nach einer
/// Ablehnung schon endete, hat keine cgroup mehr, und das ist die Folge, nicht
/// der Grund. Sonst wird jede Garantie rot mit `SANDBOX_052`, wie bei einer
/// Zulassung, die den Gast nicht erreicht. Abgewiesen wird in beiden Fällen.
///
/// `SANDBOX_052` statt `SANDBOX_047`: Satz und Fix (der Eintrag, der die
/// Controller delegiert) sind dieselben wie bei der Prüfung vor dem Start;
/// beide Wege enden an derselben Ursache.
pub(crate) fn merge_limits(results: &mut [CheckResult], diagnostic: &Diagnostic) {
    let already_refused = results.iter().any(|result| !result.passed);
    for result in results.iter_mut() {
        result.evidence = format!("{}; host: {}", result.evidence, diagnostic.why);
        if !already_refused {
            result.passed = false;
            result.diagnostic = Some(diagnostic.clone());
        }
    }
}

/// Ein Prozess der Unit, wie `/proc` ihn zeigt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Process {
    /// Die PID auf dem Host.
    pub(crate) pid: u32,
    /// Der Dateiname des Programms (`/proc/<pid>/exe`), wenn lesbar.
    pub(crate) program: Option<String>,
    /// Die Argumente.
    pub(crate) args: Vec<String>,
    /// Die Schnittstellen seines Netz-Namensraums, `None`, wenn unlesbar.
    pub(crate) interfaces: Option<Vec<String>>,
    /// Die Ziele seiner Deskriptoren, `None`, wenn unlesbar.
    pub(crate) fds: Option<Vec<String>>,
    /// Je Deskriptor, soweit lesbar, sein Ziel, wie es im Namensraum des
    /// Prozesses heißt, mit Gerät und Inode dessen, was er hält (HUM-446).
    pub(crate) opened: Vec<(String, u64, u64)>,
}

/// Wie oft die Prüfung die Prozesse der Unit nachliest, bis keiner mehr
/// dazukommt (HUM-468).
const SETTLE_ROUNDS: usize = 5;

/// Liest die Prozesse der Unit und das Laufzeitverzeichnis und prüft sie.
pub(crate) fn run(stop: &UnitStop, resolved: &Resolved, door_port: u32) -> Vec<Finding> {
    let mut processes: Vec<Process> = match unit_pids(stop) {
        Ok(pids) if !pids.is_empty() => pids
            .into_iter()
            .filter_map(|pid| read_process(Path::new("/proc"), pid))
            .collect(),
        Ok(_) => {
            return vec![finding(
                IsolationCheck::NoNetworkInterface,
                "the unit has no process left to check",
            )];
        }
        Err(why) => return vec![finding(IsolationCheck::NoNetworkInterface, why)],
    };
    let settled = match settle(
        &mut processes,
        || unit_pids(stop),
        |pid| read_process(Path::new("/proc"), pid),
    ) {
        Ok(settled) => settled,
        Err(why) => return vec![finding(IsolationCheck::NoNetworkInterface, why)],
    };
    let disposable = disposable(&stop.shares);
    let mut findings = judge(&processes, resolved);
    let roles = Roles::of(disposable);
    #[cfg(test)]
    let roles = roles.with_extra_shells(super::test_machine::extra_shells());
    findings.extend(count_roles(&processes, resolved, roles));
    if !settled {
        findings.push(finding(
            IsolationCheck::SingleSocket,
            format!(
                "the unit kept starting processes while it was checked ({SETTLE_ROUNDS} rounds)"
            ),
        ));
    }
    findings.extend(shares_held(&processes, resolved, &stop.shares));
    findings.extend(runtime_sockets(&stop.runtime, door_port, disposable));
    findings
}

/// Liest die PIDs nach, bis keine neue mehr auftaucht, und liest jede neue
/// mit `read` dazu: Ein Prozess, der nach dem ersten Lesen der cgroup
/// startet, entginge sonst der Prüfung der Programme. `false`, wenn nach
/// [`SETTLE_ROUNDS`] Runden noch immer neue kamen.
fn settle(
    processes: &mut Vec<Process>,
    mut list: impl FnMut() -> Result<Vec<u32>, String>,
    mut read: impl FnMut(u32) -> Option<Process>,
) -> Result<bool, String> {
    for _ in 0..SETTLE_ROUNDS {
        let new: Vec<u32> = list()?
            .into_iter()
            .filter(|pid| !processes.iter().any(|seen| seen.pid == *pid))
            .collect();
        if new.is_empty() {
            return Ok(true);
        }
        processes.extend(new.into_iter().filter_map(&mut read));
    }
    Ok(false)
}

/// Die cgroup der Unit unter `/sys/fs/cgroup`, wie der Dienstverwalter sie
/// nennt.
fn unit_cgroup(stop: &UnitStop) -> Result<std::path::PathBuf, String> {
    let output = crate::user_unit::run_bounded(
        stop.units.show_command(&stop.unit, "ControlGroup"),
        std::time::Duration::from_secs(10),
    )
    .map_err(|err| format!("systemctl did not name the unit's cgroup: {err}"))?;
    let group = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() && group.is_empty() {
        // So antwortet systemd für eine Unit, die es nicht (mehr) kennt; mit
        // `--collect` ist eine beendete Unit sofort vergessen (HUM-470).
        return Err(format!(
            "the unit {} has ended or never started: systemctl names no cgroup for it",
            stop.unit.service()
        ));
    }
    if !output.status.success() || !group.starts_with('/') {
        return Err(format!(
            "systemctl named no cgroup for {} ({:?})",
            stop.unit.service(),
            group
        ));
    }
    Ok(Path::new("/sys/fs/cgroup").join(group.trim_start_matches('/')))
}

/// Die PIDs der Unit, aus ihrer cgroup und allen darunter.
fn unit_pids(stop: &UnitStop) -> Result<Vec<u32>, String> {
    let root = unit_cgroup(stop)?;
    let mut pids = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        let procs = std::fs::read_to_string(dir.join("cgroup.procs"))
            .map_err(|err| format!("{}: {err}", dir.display()))?;
        pids.extend(
            procs
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok()),
        );
        // Jeder Fehler beim Lesen ist ein Fund: Eine cgroup, die sich nicht
        // aufzählen lässt, könnte Prozesse verbergen.
        let entries = std::fs::read_dir(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("{}: {err}", dir.display()))?;
            let kind = entry
                .file_type()
                .map_err(|err| format!("{}: {err}", entry.path().display()))?;
            if kind.is_dir() {
                dirs.push(entry.path());
            }
        }
    }
    pids.sort_unstable();
    pids.dedup();
    Ok(pids)
}

/// Was `/proc` (unter `proc_root`, in den Tests eine Nachbildung) über
/// einen Prozess sagt; `None` nur für einen Prozess, den es nicht mehr gibt.
fn read_process(proc_root: &Path, pid: u32) -> Option<Process> {
    let proc = proc_root.join(pid.to_string());
    let program = match std::fs::read_link(proc.join("exe")) {
        Ok(exe) => Some(exe.to_string_lossy().into_owned()),
        Err(_) if !proc.exists() => return None,
        Err(_) => None,
    };
    let args = std::fs::read(proc.join("cmdline"))
        .map(|bytes| {
            bytes
                .split(|byte| *byte == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect()
        })
        .unwrap_or_default();
    let interfaces = std::fs::read_to_string(proc.join("net/dev"))
        .ok()
        .map(|text| interfaces_of(&text));
    let fds = read_fds(&proc.join("fd"));
    let opened = read_opened(&proc.join("fd"));
    Some(Process {
        pid,
        program,
        args,
        interfaces,
        fds,
        opened,
    })
}

/// Je Deskriptor sein Ziel und Gerät und Inode dessen, was er hält, über den
/// magischen Verweis in `/proc/<pid>/fd`; was sich nicht lesen lässt, fehlt.
/// Fehlt so ein geprüfter Ordner, ist das in [`shares_held`] ein Fund, nie
/// ein Bestehen.
fn read_opened(dir: &Path) -> Vec<(String, u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let target = std::fs::read_link(entry.path()).ok()?;
                    let meta = std::fs::metadata(entry.path()).ok()?;
                    Some((
                        target.to_string_lossy().into_owned(),
                        meta.dev(),
                        meta.ino(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Der Wert von `--shared-dir` in einer Zeile von virtiofsd, ob als eigenes
/// Wort oder mit `=`.
fn shared_dir_of(args: &[String]) -> Option<&str> {
    args.iter().enumerate().find_map(|(index, arg)| {
        if arg == "--shared-dir" {
            args.get(index + 1).map(String::as_str)
        } else {
            arg.strip_prefix("--shared-dir=")
        }
    })
}

/// Jeder geprüfte Ordner hat genau einen virtiofsd, der ihn unter seinem
/// `--shared-dir` teilt und ihn als Wurzel des Geteilten hält (HUM-446): ein
/// Deskriptor, dessen Ziel in seinem Gefängnis der `--shared-dir` ist und
/// der Gerät und Inode des Plans trägt. Ohne ihn teilt der Dienst etwas
/// anderes, etwa einen Ordner, der nach der Prüfung an dessen Pfad trat;
/// dass er den geprüften Ordner irgendwo darunter hält, genügt nicht.
pub(crate) fn shares_held(
    processes: &[Process],
    resolved: &Resolved,
    shares: &[SharedDir],
) -> Vec<Finding> {
    let program = std::fs::canonicalize(&resolved.file_daemon)
        .unwrap_or_else(|_| resolved.file_daemon.clone())
        .to_string_lossy()
        .into_owned();
    let mut findings = Vec::new();
    for share in shares {
        let jail = share.jail_path.to_string_lossy();
        let serving: Vec<&Process> = processes
            .iter()
            .filter(|process| process.program.as_deref() == Some(program.as_str()))
            .filter(|process| shared_dir_of(&process.args) == Some(jail.as_ref()))
            .collect();
        match serving.as_slice() {
            [one]
                if one.opened.iter().any(|(target, dev, ino)| {
                    *target == jail && (*dev, *ino) == (share.dev, share.ino)
                }) => {}
            [one] => findings.push(finding(
                IsolationCheck::SingleSocket,
                format!(
                    "the file service {} shares {jail} without holding the checked folder {}:{}",
                    one.pid, share.dev, share.ino
                ),
            )),
            other => findings.push(finding(
                IsolationCheck::SingleSocket,
                format!(
                    "{} file services share {jail}, expected exactly one",
                    other.len()
                ),
            )),
        }
    }
    findings
}

/// Die Ziele aller Deskriptoren; `None`, wenn das Verzeichnis, ein Eintrag
/// oder ein Ziel sich nicht lesen lässt. Nur ein Deskriptor, der zwischen
/// Aufzählen und Lesen zuging (`NotFound`), fällt still heraus.
fn read_fds(dir: &Path) -> Option<Vec<String>> {
    let mut targets = Vec::new();
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        match std::fs::read_link(entry.path()) {
            Ok(target) => targets.push(target.to_string_lossy().into_owned()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    Some(targets)
}

/// Die Namen aus `/proc/<pid>/net/dev`: zwei Kopfzeilen, dann je Zeile ein
/// Name vor dem Doppelpunkt.
pub(crate) fn interfaces_of(text: &str) -> Vec<String> {
    text.lines()
        .skip(2)
        .filter_map(|line| line.split(':').next())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Ob QEMU diesen Deskriptor halten darf (HUM-442 Q3, ohne Konsole).
pub(crate) fn emulator_fd_allowed(target: &str) -> bool {
    const EXACT: &[&str] = &[
        "/dev/kvm",
        "/dev/null",
        "anon_inode:kvm-vm",
        "anon_inode:[eventfd]",
        "anon_inode:[io_uring]",
        "anon_inode:[signalfd]",
        "anon_inode:[eventpoll]",
        "anon_inode:[timerfd]",
    ];
    EXACT.contains(&target)
        || target.starts_with("anon_inode:kvm-vcpu:")
        || target.starts_with("anon_inode:kvm-vcpu-stats:")
        || target.starts_with("socket:[")
        || target.starts_with("pipe:[")
        || (target.starts_with("/memfd:") && target.ends_with("(deleted)"))
}

/// Ob ein Deskriptor in irgendeinem Prozess der Unit ein Netz- oder
/// vsock-Gerät des Hosts ist.
pub(crate) fn host_device(target: &str) -> bool {
    target == "/dev/vhost-vsock"
        || target == "/dev/vhost-net"
        || target == "/dev/net/tun"
        || target.starts_with("/dev/tap")
        || target.contains("tun-file")
}

/// Die Programme, die das Skript der Unit startet (HUM-468), je unter dem
/// Pfad, wie die Einstellung ihn nennt, und unter seinem aufgelösten Ziel.
///
/// `/proc/<pid>/exe` nennt für einen Prozess, den der Host gestartet hat,
/// das aufgelöste Programm. Für einen Prozess, der erst in seinem Gefängnis
/// startete, nennt es den Pfad im Gefängnis: Liegt das Programm außerhalb
/// von `/usr`, bindet die Zeile es unter dem eingestellten Pfad dorthin
/// ([`super::lines`]), und der Kernel zeigt diesen Pfad, auch wenn er auf
/// dem Host ein Verweis ist. Darum gilt beides. Den Shim nennt die Liste
/// nicht: Er ersetzt sich mit `--fds-then-exec` durch die Shell, lange bevor
/// der Gast berichtet; ein Shim, der zur Zeit der Prüfung noch läuft, ist
/// eine Abweichung.
fn unit_programs(resolved: &Resolved) -> Vec<String> {
    let configured = [
        Path::new(super::lines::SCRIPT_SHELL),
        &resolved.bubblewrap,
        &resolved.file_daemon,
        &resolved.vsock_device,
        &resolved.emulator,
    ];
    let mut programs = Vec::new();
    for path in configured {
        programs.push(path.to_string_lossy().into_owned());
        if let Ok(target) = std::fs::canonicalize(path) {
            programs.push(target.to_string_lossy().into_owned());
        }
    }
    programs
}

/// Ein Fund, wenn der Prozess ein anderes Programm ausführt als die der
/// Unit. Ein gelöschtes Programm nennt der Kernel mit ` (deleted)` am Ende;
/// es ist nicht mehr das geprüfte und deshalb ein Fund wie jedes fremde. Ein
/// unlesbares Programm meldet [`judge`] selbst.
fn foreign_program(process: &Process, programs: &[String]) -> Option<Finding> {
    let program = process.program.as_ref()?;
    (!programs.contains(program)).then(|| {
        finding(
            IsolationCheck::SingleSocket,
            format!(
                "process {} runs {program}, a program the unit does not start: {}",
                process.pid,
                process.args.join(" ")
            ),
        )
    })
}

/// Die Prüfung über gelesene Prozesse; ohne `/proc`, damit sie testbar ist.
pub(crate) fn judge(processes: &[Process], resolved: &Resolved) -> Vec<Finding> {
    // `/proc/<pid>/exe` nennt das aufgelöste Programm; ein Programm, das
    // über einen Verweis eingestellt ist, wird deshalb ebenso aufgelöst.
    let canonical = |path: &Path| {
        Some(
            std::fs::canonicalize(path)
                .unwrap_or_else(|_| path.to_path_buf())
                .to_string_lossy()
                .into_owned(),
        )
    };
    let emulator = canonical(&resolved.emulator);
    let device = canonical(&resolved.vsock_device);
    let programs = unit_programs(resolved);
    let mut findings = Vec::new();
    let mut devices = 0;
    let mut emulators = 0;
    for process in processes {
        let pid = process.pid;
        findings.extend(foreign_program(process, &programs));
        match &process.interfaces {
            Some(names) if names.iter().all(|name| name == "lo") => {}
            Some(names) => findings.push(finding(
                IsolationCheck::NoNetworkInterface,
                format!("process {pid} sees interfaces {}", names.join(",")),
            )),
            None => findings.push(finding(
                IsolationCheck::NoNetworkInterface,
                format!("the network namespace of process {pid} is unreadable"),
            )),
        }
        if process.program.is_none() {
            findings.push(finding(
                IsolationCheck::NoNetworkInterface,
                format!("the program of process {pid} is unreadable"),
            ));
        }
        let Some(fds) = &process.fds else {
            findings.push(finding(
                IsolationCheck::NoNetworkInterface,
                format!("the descriptors of process {pid} are unreadable"),
            ));
            continue;
        };
        for target in fds.iter().filter(|target| host_device(target)) {
            findings.push(finding(
                IsolationCheck::NoNetworkInterface,
                format!("process {pid} holds {target}"),
            ));
        }
        if process.program.is_some() && process.program == emulator {
            emulators += 1;
            findings.extend(emulator_args(pid, &process.args));
            for target in fds.iter().filter(|target| !emulator_fd_allowed(target)) {
                findings.push(finding(
                    IsolationCheck::SingleSocket,
                    format!("the emulator {pid} holds {target}, a channel the plan does not have"),
                ));
            }
        }
        if process.program.is_some() && process.program == device {
            devices += 1;
            let count = |flag: &str| {
                process
                    .args
                    .iter()
                    .filter(|arg| {
                        *arg == flag
                            || arg
                                .strip_prefix(flag)
                                .is_some_and(|rest| rest.starts_with('='))
                    })
                    .count()
            };
            let one_device = count("--socket") == 1
                && count("--uds-path") == 1
                && ["--vm", "--config", "--forward-cid", "--forward-listen"]
                    .iter()
                    .all(|flag| count(flag) == 0);
            if !one_device {
                findings.push(finding(
                    IsolationCheck::SingleSocket,
                    format!(
                        "the vsock device {pid} serves more than one device: {}",
                        process.args.join(" ")
                    ),
                ));
            }
        }
    }
    if emulators != 1 {
        findings.push(finding(
            IsolationCheck::NoNetworkInterface,
            format!("{emulators} emulator processes in the unit, expected exactly one"),
        ));
    }
    if devices != 1 {
        findings.push(finding(
            IsolationCheck::SingleSocket,
            format!("{devices} vsock device processes in the unit, expected exactly one"),
        ));
    }
    findings
}

/// Wie viele Prozesse das Skript der Unit je Programm startet (HUM-480).
///
/// Gemessen an einer echten Sitzung (Debian 14, dash 0.5, bubblewrap 0.13,
/// `ps` über die cgroup der Unit): die Shell des Skripts, je Helfer eine
/// Unterschale für `( … ) &`, die nach seinem Ende das Skript beendet, und
/// eine, die auf die Lebensleine wartet; je Helfer zwei bubblewrap (der
/// äußere wartet, der innere ist mit `--unshare-pid` der Init des
/// Gefängnisses), darunter der Helfer. bash als `/bin/sh` forkt für dieses
/// Skript genauso (gemessen mit derselben Form). Die Fäden eines Programms
/// zählen nicht, `cgroup.procs` nennt nur Prozesse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Roles {
    /// Die Helfer, jeder in seinem Gefängnis: die Dienste der Dateien, das
    /// vsock-Gerät und der Emulator.
    helpers: usize,
    /// Die Dienste der Dateien: Wurzel und `/work`, in einer
    /// Wegwerf-Sitzung dazu die Schicht (HUM-446).
    file_daemons: usize,
    /// Shells, die ein KVM-Test neben dem Skript in der Unit hält (HUM-480,
    /// [`super::test_machine::TestMachine::extra_shells`]); in der Endform 0.
    extra_shells: usize,
}

/// Ob die Sitzung eine Wegwerf-Sitzung ist: Ein Dienst teilt die Schicht
/// (HUM-446). Davon hängen die Rollen ([`Roles::of`]) und die erklärten
/// Sockets ab.
fn disposable(shares: &[SharedDir]) -> bool {
    shares
        .iter()
        .any(|share| share.jail_path == Path::new(super::lines::LAYER_SHARE))
}

impl Roles {
    /// Die Rollen einer direkten oder einer Wegwerf-Sitzung.
    pub(crate) const fn of(disposable: bool) -> Self {
        let file_daemons = if disposable { 3 } else { 2 };
        Self {
            helpers: file_daemons + 2,
            file_daemons,
            extra_shells: 0,
        }
    }

    /// Dieselben Rollen mit `extra` Shells eines Tests.
    #[cfg(test)]
    pub(crate) const fn with_extra_shells(self, extra: usize) -> Self {
        Self {
            extra_shells: extra,
            ..self
        }
    }
}

/// Die Namen eines Programms in `/proc/<pid>/exe`: der eingestellte Pfad und
/// sein aufgelöstes Ziel (siehe [`unit_programs`]).
fn names_of(path: &Path) -> Vec<String> {
    let mut names = vec![path.to_string_lossy().into_owned()];
    if let Ok(target) = std::fs::canonicalize(path) {
        names.push(target.to_string_lossy().into_owned());
    }
    names
}

/// Ein Fund je Programm, das nicht so oft läuft, wie das Skript es startet
/// ([`Roles`]). Emulator und vsock-Gerät zählt schon [`judge`].
pub(crate) fn count_roles(
    processes: &[Process],
    resolved: &Resolved,
    roles: Roles,
) -> Vec<Finding> {
    let expected = [
        (
            "the script's shell",
            Path::new(super::lines::SCRIPT_SHELL),
            roles.helpers + 2 + roles.extra_shells,
        ),
        (
            "bubblewrap",
            resolved.bubblewrap.as_path(),
            roles.helpers * 2,
        ),
        (
            "the file service",
            resolved.file_daemon.as_path(),
            roles.file_daemons,
        ),
    ];
    let mut findings = Vec::new();
    for (role, program, wanted) in expected {
        let names = names_of(program);
        let pids: Vec<String> = processes
            .iter()
            .filter(|process| process.program.as_ref().is_some_and(|p| names.contains(p)))
            .map(|process| process.pid.to_string())
            .collect();
        if pids.len() != wanted {
            findings.push(finding(
                IsolationCheck::SingleSocket,
                format!(
                    "{} processes run {role} {}, the unit starts exactly {wanted} (pids {})",
                    pids.len(),
                    program.display(),
                    pids.join(",")
                ),
            ));
        }
    }
    findings
}

/// Die Zeile des laufenden Emulators, wie der Host sie sieht: kein Netz
/// außer `-nic none`, kein `-netdev`, kein Netzgerät, keine Konsole, kein
/// Monitor, kein Zeichengerät außer einem Socket. Ein Gastkernel ohne
/// Treiber für ein Netzgerät sähe es nicht; der Host sieht es trotzdem.
pub(crate) fn emulator_args(pid: u32, args: &[String]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut nic_none = false;
    for (index, arg) in args.iter().enumerate() {
        let next = args.get(index + 1).map_or("", String::as_str);
        let (bad, check) = match arg.as_str() {
            "-nic" if next == "none" => {
                nic_none = true;
                (false, IsolationCheck::NoNetworkInterface)
            }
            "-nic" | "-netdev" | "-net" => (true, IsolationCheck::NoNetworkInterface),
            // Nur die drei Geräte des Plans.
            "-device" => (
                !(next.starts_with("vhost-user-fs-pci,")
                    || next.starts_with("vhost-user-vsock-pci,")),
                IsolationCheck::NoNetworkInterface,
            ),
            "-serial" | "-monitor" | "-display" | "-parallel" => {
                (next != "none", IsolationCheck::SingleSocket)
            }
            "-chardev" => (!next.starts_with("socket,"), IsolationCheck::SingleSocket),
            "-qmp" | "-gdb" | "-vnc" | "-spice" => (true, IsolationCheck::SingleSocket),
            _ => (false, IsolationCheck::SingleSocket),
        };
        if bad {
            findings.push(finding(
                check,
                format!("the emulator {pid} runs with {arg} {next}"),
            ));
        }
    }
    if !nic_none {
        findings.push(finding(
            IsolationCheck::NoNetworkInterface,
            format!("the emulator {pid} runs without -nic none"),
        ));
    }
    findings
}

/// Im Laufzeitverzeichnis nur die erklärten Sockets, jeder an seinem Ort;
/// steht unter einem erklärten Namen etwas, ist es ein Socket. Kein Verweis,
/// kein weiterer Socket, und kein Eintrag, dessen Art sich nicht lesen lässt.
/// Ein erklärter Socket darf fehlen: virtiofsd 1.13 entfernt seinen, sobald
/// QEMU verbunden ist (gemessen in der CI, 2026-11-14); ein fehlender Socket
/// öffnet keinen Weg.
pub(crate) fn runtime_sockets(dir: &Path, door_port: u32, disposable: bool) -> Vec<Finding> {
    use std::os::unix::fs::FileTypeExt as _;
    /// So tief liegen die Dateien der Sitzung höchstens.
    const MAX_DEPTH: usize = 3;
    let declared = Layout::declared_sockets(door_port, disposable);
    let mut findings = Vec::new();
    let mut fail = |what: String| findings.push(finding(IsolationCheck::SingleSocket, what));
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((current, depth)) = stack.pop() {
        let entries = match std::fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(err) => {
                fail(format!("{}: {err}", current.display()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    fail(format!("{}: {err}", current.display()));
                    continue;
                }
            };
            let path = entry.path();
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(err) => {
                    fail(format!("{}: {err}", path.display()));
                    continue;
                }
            };
            let relative = path.strip_prefix(dir).map_or_else(
                |_| path.display().to_string(),
                |rel| rel.display().to_string(),
            );
            if kind.is_symlink() {
                fail(format!("a link {}", path.display()));
            } else if kind.is_socket() {
                if !declared.contains(&relative) {
                    fail(format!("an undeclared socket {}", path.display()));
                }
            } else if kind.is_dir() {
                if depth + 1 < MAX_DEPTH {
                    stack.push((path, depth + 1));
                } else {
                    fail(format!(
                        "a directory deeper than the plan {}",
                        path.display()
                    ));
                }
            } else if declared.contains(&relative) {
                fail(format!(
                    "the declared socket {} is not a socket",
                    path.display()
                ));
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    use super::*;

    fn resolved() -> Resolved {
        Resolved {
            emulator: PathBuf::from("/usr/bin/qemu-system-x86_64"),
            file_daemon: PathBuf::from("/usr/libexec/virtiofsd"),
            vsock_device: PathBuf::from("/usr/bin/vhost-device-vsock"),
            bubblewrap: PathBuf::from("/usr/bin/bwrap"),
            kernel: PathBuf::from("/boot/vmlinuz"),
            modules: PathBuf::from("/usr/lib/modules/x"),
            no_announce_submounts: true,
        }
    }

    fn healthy() -> Vec<Process> {
        let process = |pid, program: &str, args: &[&str], fds: &[&str]| Process {
            pid,
            program: Some(program.to_owned()),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            interfaces: Some(vec!["lo".to_owned()]),
            fds: Some(fds.iter().map(|fd| (*fd).to_owned()).collect()),
            opened: Vec::new(),
        };
        vec![
            process(
                10,
                "/bin/sh",
                &["/bin/sh", "-c", "x"],
                &["/dev/null", "pipe:[1]"],
            ),
            process(11, "/usr/bin/bwrap", &["bwrap"], &["/dev/null"]),
            process(
                12,
                "/usr/bin/vhost-device-vsock",
                &[
                    "vhost-device-vsock",
                    "--guest-cid",
                    "3",
                    "--socket",
                    "/r/vsock.sock",
                    "--uds-path",
                    "/r/door",
                    "--groups",
                    "g",
                ],
                &["socket:[5]", "anon_inode:[eventfd]"],
            ),
            process(
                13,
                "/usr/bin/qemu-system-x86_64",
                &[
                    "qemu-system-x86_64",
                    "-M",
                    "microvm",
                    "-nic",
                    "none",
                    "-serial",
                    "none",
                    "-chardev",
                    "socket,id=vs,path=/r/vsock.sock",
                    "-device",
                    "vhost-user-vsock-pci,chardev=vs",
                ],
                &[
                    "/dev/null",
                    "pipe:[2]",
                    "/dev/kvm",
                    "anon_inode:kvm-vm",
                    "anon_inode:kvm-vcpu:0",
                    "anon_inode:kvm-vcpu-stats:0",
                    "anon_inode:[eventfd]",
                    "anon_inode:[io_uring]",
                    "anon_inode:[signalfd]",
                    "/memfd:mem (deleted)",
                    "socket:[7]",
                ],
            ),
        ]
    }

    #[test]
    fn a_healthy_unit_has_no_finding() {
        assert_eq!(judge(&healthy(), &resolved()), Vec::new());
        assert_eq!(
            interfaces_of("Inter-|   Receive\n face |bytes\n    lo: 0 0\n"),
            ["lo"]
        );
    }

    /// Jede Abweichung ist ein Fund mit ihrer Garantie; was sich nicht lesen
    /// lässt, auch.
    type Mutation = fn(&mut Vec<Process>);

    #[test]
    fn every_deviation_is_a_finding() {
        for (name, mutate, check) in deviations().into_iter().chain(foreign_programs()) {
            let mut processes = healthy();
            mutate(&mut processes);
            let found = judge(&processes, &resolved());
            assert!(
                found.iter().any(|finding| finding.check == check),
                "{name}: {found:?}"
            );
        }
    }

    /// Die Abweichungen, je mit der Garantie, die sie brechen.
    fn deviations() -> Vec<(&'static str, Mutation, IsolationCheck)> {
        vec![
            (
                "eth0",
                |p| p[1].interfaces = Some(vec!["lo".into(), "eth0".into()]),
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "unreadable net",
                |p| p[0].interfaces = None,
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "unreadable fds",
                |p| p[3].fds = None,
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "tun",
                |p| p[3].fds.as_mut().unwrap().push("/dev/net/tun".into()),
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "vhost-vsock",
                |p| p[2].fds.as_mut().unwrap().push("/dev/vhost-vsock".into()),
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "console",
                |p| p[3].fds.as_mut().unwrap().push("/dev/pts/3".into()),
                IsolationCheck::SingleSocket,
            ),
            (
                "second device",
                |p| {
                    let d = p[2].clone();
                    p.push(Process { pid: 20, ..d });
                },
                IsolationCheck::SingleSocket,
            ),
            (
                "--vm",
                |p| p[2].args.push("--vm".into()),
                IsolationCheck::SingleSocket,
            ),
            (
                "--vm=",
                |p| p[2].args.push("--vm=guest-cid=4".into()),
                IsolationCheck::SingleSocket,
            ),
            (
                "--forward-cid=",
                |p| p[2].args.push("--forward-cid=1".into()),
                IsolationCheck::SingleSocket,
            ),
            (
                "unreadable program",
                |p| p[1].program = None,
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "-device virtio-blk",
                |p| {
                    p[3].args
                        .extend(["-device".into(), "virtio-blk-pci,drive=d".into()]);
                },
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "--forward-cid",
                |p| p[2].args.push("--forward-cid".into()),
                IsolationCheck::SingleSocket,
            ),
            (
                "no emulator",
                |p| {
                    p.remove(3);
                },
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "-nic user",
                |p| p[3].args.extend(["-nic".into(), "user".into()]),
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "-netdev",
                |p| p[3].args.extend(["-netdev".into(), "user,id=n".into()]),
                IsolationCheck::NoNetworkInterface,
            ),
            (
                "-serial stdio",
                |p| p[3].args.extend(["-serial".into(), "stdio".into()]),
                IsolationCheck::SingleSocket,
            ),
            (
                "no -nic none",
                |p| p[3].args.retain(|a| a != "-nic" && a != "none"),
                IsolationCheck::NoNetworkInterface,
            ),
        ]
    }

    /// HUM-468: Programme, die die Unit nicht startet.
    fn foreign_programs() -> Vec<(&'static str, Mutation, IsolationCheck)> {
        vec![
            (
                "an unknown program",
                |p| {
                    let sh = p[0].clone();
                    p.push(Process {
                        pid: 21,
                        program: Some("/usr/bin/sleep".into()),
                        args: vec!["sleep".into(), "600".into()],
                        ..sh
                    });
                },
                IsolationCheck::SingleSocket,
            ),
            (
                "a deleted program",
                |p| p[1].program = Some("/usr/bin/bwrap (deleted)".into()),
                IsolationCheck::SingleSocket,
            ),
            (
                "the shim still running",
                |p| p[0].program = Some("/usr/libexec/humanitl/humanitl-shim".into()),
                IsolationCheck::SingleSocket,
            ),
        ]
    }

    /// Eine Nachbildung von `/proc` mit einem Prozess je Eintrag: Programm
    /// (`exe`, `None` ohne Verweis), Argumente, Schnittstellen und die Ziele
    /// der Deskriptoren.
    fn proc_fixture(root: &Path, pid: u32, program: Option<&str>, args: &[&str], fds: &[&str]) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(dir.join("net")).unwrap();
        std::fs::create_dir_all(dir.join("fd")).unwrap();
        if let Some(program) = program {
            std::os::unix::fs::symlink(program, dir.join("exe")).unwrap();
        }
        let mut cmdline = Vec::new();
        for arg in args {
            cmdline.extend_from_slice(arg.as_bytes());
            cmdline.push(0);
        }
        std::fs::write(dir.join("cmdline"), cmdline).unwrap();
        std::fs::write(
            dir.join("net/dev"),
            "Inter-|   Receive\n face |bytes\n    lo: 0 0\n",
        )
        .unwrap();
        for (fd, target) in fds.iter().enumerate() {
            std::os::unix::fs::symlink(target, dir.join("fd").join(fd.to_string())).unwrap();
        }
    }

    /// HUM-468 über eine Nachbildung von `/proc`: Die Programme der Unit
    /// bestehen; ein zusätzlicher `sleep`, ein gelöschtes Programm und ein
    /// unlesbares sind je ein Fund. Die Prozesse gehen den Weg, den die
    /// Prüfung auf dem Host geht ([`read_process`], dann [`judge`]).
    #[test]
    fn every_process_of_the_unit_runs_one_of_its_programs() {
        let root = tempfile::tempdir().unwrap();
        let read = |pids: &[u32]| -> Vec<Process> {
            pids.iter()
                .filter_map(|pid| read_process(root.path(), *pid))
                .collect()
        };
        for process in healthy() {
            let args: Vec<&str> = process.args.iter().map(String::as_str).collect();
            let fds: Vec<&str> = process
                .fds
                .as_ref()
                .unwrap()
                .iter()
                .map(String::as_str)
                .collect();
            proc_fixture(
                root.path(),
                process.pid,
                process.program.as_deref(),
                &args,
                &fds,
            );
        }
        let unit = [10, 11, 12, 13];
        let read_back = read(&unit);
        // `read_dir` liefert die Deskriptoren in keiner festen Reihenfolge.
        let sorted = |fds: &Option<Vec<String>>| {
            let mut fds = fds.clone();
            if let Some(fds) = fds.as_mut() {
                fds.sort();
            }
            fds
        };
        assert_eq!(
            read_back
                .iter()
                .map(|p| (p.pid, p.program.clone(), p.args.clone(), sorted(&p.fds)))
                .collect::<Vec<_>>(),
            healthy()
                .iter()
                .map(|p| (p.pid, p.program.clone(), p.args.clone(), sorted(&p.fds)))
                .collect::<Vec<_>>(),
            "the fixture reads back as the healthy unit"
        );
        assert_eq!(judge(&read_back, &resolved()), Vec::new());

        // Ein Prozess, den es nicht mehr gibt, fällt still heraus.
        assert_eq!(read(&[10, 11, 12, 13, 99]).len(), 4);

        // Mutationsbeweis: ein zusätzlicher `sleep` in der Unit.
        proc_fixture(
            root.path(),
            14,
            Some("/usr/bin/sleep"),
            &["sleep", "600"],
            &[],
        );
        let found = judge(&read(&[10, 11, 12, 13, 14]), &resolved());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].check, IsolationCheck::SingleSocket);
        assert!(
            found[0]
                .what
                .starts_with("process 14 runs /usr/bin/sleep, a program the unit does not start"),
            "{found:?}"
        );

        // Ein gelöschtes Programm ist nicht mehr das geprüfte.
        proc_fixture(
            root.path(),
            15,
            Some("/usr/bin/vhost-device-vsock (deleted)"),
            &["vhost-device-vsock"],
            &[],
        );
        let found = judge(&read(&[10, 11, 12, 13, 15]), &resolved());
        assert!(
            found.iter().any(|f| f.check == IsolationCheck::SingleSocket
                && f.what
                    .contains("(deleted), a program the unit does not start")),
            "{found:?}"
        );

        // Ohne lesbaren Verweis ist das Programm unbekannt, nie bestanden.
        proc_fixture(root.path(), 16, None, &["sleep"], &[]);
        let found = judge(&read(&[10, 11, 12, 13, 16]), &resolved());
        assert!(
            found
                .iter()
                .any(|f| f.what == "the program of process 16 is unreadable"),
            "{found:?}"
        );
    }

    /// HUM-446: Jeder geprüfte Ordner hat genau einen virtiofsd, der ihn
    /// unter seinem `--shared-dir` teilt und das geöffnete Verzeichnis hält.
    /// Ein Dienst, der etwas anderes hält (ein Pfad statt des Deskriptors
    /// nach einem Tausch), keiner oder zwei sind je ein Fund der Garantie 2.
    #[test]
    fn every_checked_folder_is_held_by_exactly_one_file_service() {
        let service = |pid, dir: &str, opened: Vec<(&str, u64, u64)>| Process {
            pid,
            program: Some("/usr/libexec/virtiofsd".to_owned()),
            args: vec![
                "/usr/libexec/virtiofsd".to_owned(),
                "--shared-dir".to_owned(),
                dir.to_owned(),
            ],
            interfaces: Some(vec!["lo".to_owned()]),
            fds: Some(Vec::new()),
            opened: opened
                .into_iter()
                .map(|(target, dev, ino)| (target.to_owned(), dev, ino))
                .collect(),
        };
        let shares = [
            SharedDir {
                jail_path: PathBuf::from("/share"),
                dev: 40,
                ino: 7,
            },
            SharedDir {
                jail_path: PathBuf::from("/layer"),
                dev: 41,
                ino: 9,
            },
        ];
        let mut processes = healthy();
        processes.push(service(
            30,
            "/share",
            vec![("/dev/null", 1, 2), ("/share", 40, 7)],
        ));
        processes.push(service(31, "/layer", vec![("/layer", 41, 9)]));
        processes.push(service(32, "/guest", vec![("/guest/work", 40, 7)]));
        assert_eq!(shares_held(&processes, &resolved(), &shares), Vec::new());

        // Ein Pfad statt des Deskriptors: Der Dienst hält einen anderen Ordner.
        let mut swapped = processes.clone();
        swapped[4].opened = vec![("/share".to_owned(), 40, 8)];
        let found = shares_held(&swapped, &resolved(), &shares);
        assert!(
            found.len() == 1
                && found[0].check == IsolationCheck::SingleSocket
                && found[0].what.contains("without holding the checked folder"),
            "{found:?}"
        );
        // Der gleiche Inode auf einem anderen Gerät ist nicht der Ordner.
        swapped[4].opened = vec![("/share".to_owned(), 39, 7)];
        assert_eq!(shares_held(&swapped, &resolved(), &shares).len(), 1);
        // Den geprüften Ordner nur unterhalb des Geteilten zu halten (ein
        // getauschter Ordner, in den der geprüfte verschoben wurde), genügt
        // nicht.
        swapped[4].opened = vec![("/share".to_owned(), 40, 8), ("/share/x".to_owned(), 40, 7)];
        assert_eq!(shares_held(&swapped, &resolved(), &shares).len(), 1);
        // Kein Dienst für die Schicht, und zwei für das Projekt.
        let mut missing = processes.clone();
        missing.remove(5);
        missing.push(service(33, "/share", vec![("/share", 40, 7)]));
        let found = shares_held(&missing, &resolved(), &shares);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found
                .iter()
                .any(|f| f.what.contains("2 file services share /share"))
        );
        assert!(
            found
                .iter()
                .any(|f| f.what.contains("0 file services share /layer"))
        );
        // `--shared-dir=` zählt wie das getrennte Wort.
        let mut joined = processes;
        joined[4].args = vec!["virtiofsd".to_owned(), "--shared-dir=/share".to_owned()];
        assert_eq!(shares_held(&joined, &resolved(), &shares), Vec::new());
    }

    /// Ein Programm, das über einen Verweis eingestellt ist, erkennt die
    /// Prüfung an seinem Ziel, wie `/proc/<pid>/exe` es nennt.
    #[test]
    fn a_linked_program_is_known_by_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-emulator");
        std::fs::write(&target, "").unwrap();
        let link = dir.path().join("emulator-link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let resolved = Resolved {
            emulator: link,
            ..resolved()
        };
        let mut processes = healthy();
        processes[3].program = Some(
            std::fs::canonicalize(&target)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        assert_eq!(judge(&processes, &resolved), Vec::new());
    }

    /// Mutationsbeweis der Spezifikation auf Ebene der Prüfung: ein
    /// zusätzlicher Lauscher `<uds>_22` färbt sie rot; die erklärten nicht.
    /// Ein Verweis, eine Datei unter einem erklärten Namen und ein fehlender
    /// erklärter Socket sind je ein Fund.
    #[test]
    fn only_the_declared_sockets_lie_in_the_runtime_directory() {
        let dir = tempfile::tempdir().unwrap();
        for sub in Layout::SOCKET_DIRS.iter().chain(&["files"]) {
            std::fs::create_dir(dir.path().join(sub)).unwrap();
        }
        let mut held = Vec::new();
        for name in Layout::declared_sockets(5000, false) {
            held.push(UnixListener::bind(dir.path().join(name)).unwrap());
        }
        std::fs::write(dir.path().join("files/guest-plan.json"), "{}").unwrap();
        assert_eq!(runtime_sockets(dir.path(), 5000, false), Vec::new());

        // Der Socket der Schicht gehört nur zu einer Wegwerf-Sitzung
        // (HUM-446).
        held.push(UnixListener::bind(dir.path().join("layer/layer.sock")).unwrap());
        assert_eq!(runtime_sockets(dir.path(), 5000, true), Vec::new());
        let found = runtime_sockets(dir.path(), 5000, false);
        assert!(
            found.len() == 1 && found[0].what.ends_with("layer.sock"),
            "{found:?}"
        );
        std::fs::remove_file(dir.path().join("layer/layer.sock")).unwrap();

        held.push(UnixListener::bind(dir.path().join("door/door_22")).unwrap());
        let found = runtime_sockets(dir.path(), 5000, false);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].what.ends_with("door_22"), "{found:?}");
        assert_eq!(found[0].check, IsolationCheck::SingleSocket);
        std::fs::remove_file(dir.path().join("door/door_22")).unwrap();

        // Derselbe Name an einem anderen Ort ist nicht erklärt.
        held.push(UnixListener::bind(dir.path().join("files/door")).unwrap());
        assert_eq!(runtime_sockets(dir.path(), 5000, false).len(), 1);
        std::fs::remove_file(dir.path().join("files/door")).unwrap();

        std::os::unix::fs::symlink("/run", dir.path().join("files/link")).unwrap();
        let found = runtime_sockets(dir.path(), 5000, false);
        assert!(found.iter().any(|f| f.what.contains("a link")), "{found:?}");
        std::fs::remove_file(dir.path().join("files/link")).unwrap();

        // Fehlt ein erklärter Socket, öffnet das keinen Weg (virtiofsd 1.13
        // entfernt seinen nach der Verbindung); eine Datei unter seinem
        // Namen ist ein Befund.
        std::fs::remove_file(dir.path().join("work/work.sock")).unwrap();
        assert_eq!(runtime_sockets(dir.path(), 5000, false), Vec::new());
        std::fs::write(dir.path().join("work/work.sock"), "").unwrap();
        let found = runtime_sockets(dir.path(), 5000, false);
        assert!(
            found.iter().any(|f| f.what.contains("is not a socket")),
            "{found:?}"
        );
    }

    /// Die Prozesse des Skripts je Rolle, wie in einer echten Sitzung
    /// gemessen (HUM-480): Shells, bubblewrap und Dienste der Dateien.
    fn script_tree(shells: u32, jails: u32, file_daemons: u32) -> Vec<Process> {
        let process = |pid: u32, program: &str| Process {
            pid,
            program: Some(program.to_owned()),
            ..Process::default()
        };
        (0..shells)
            .map(|n| process(100 + n, "/bin/sh"))
            .chain((0..jails).map(|n| process(200 + n, "/usr/bin/bwrap")))
            .chain((0..file_daemons).map(|n| process(300 + n, "/usr/libexec/virtiofsd")))
            .collect()
    }

    /// HUM-480: Jedes Programm läuft so oft, wie das Skript es startet; eine
    /// Shell oder ein bubblewrap an der Stelle eines Helfers, ein fehlender
    /// oder ein zusätzlicher Prozess ist je ein Fund der Garantie 2.
    #[test]
    fn every_program_runs_as_often_as_the_script_starts_it() {
        let direct = Roles::of(false);
        let layered_roles = Roles::of(true);
        assert_eq!(
            count_roles(&script_tree(6, 8, 2), &resolved(), direct),
            Vec::new()
        );
        assert_eq!(
            count_roles(&script_tree(7, 10, 3), &resolved(), layered_roles),
            Vec::new()
        );
        // Eine direkte Sitzung hat den Dienst der Schicht nicht.
        assert_eq!(
            count_roles(&script_tree(6, 8, 2), &resolved(), layered_roles).len(),
            3
        );
        // Eine Shell statt des zweiten virtiofsd.
        let found = count_roles(&script_tree(7, 8, 1), &resolved(), direct);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found
                .iter()
                .all(|f| f.check == IsolationCheck::SingleSocket),
            "{found:?}"
        );
        assert!(
            found.iter().any(|f| f.what.starts_with(
                "7 processes run the script's shell /bin/sh, the unit starts exactly 6"
            )),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|f| f.what.starts_with("1 processes run the file service")),
            "{found:?}"
        );
        // Ein bubblewrap mehr, eine Shell weniger.
        let found = count_roles(&script_tree(6, 9, 2), &resolved(), direct);
        assert!(
            found.len() == 1 && found[0].what.contains("run bubblewrap /usr/bin/bwrap"),
            "{found:?}"
        );
        assert_eq!(
            count_roles(&script_tree(5, 8, 2), &resolved(), direct).len(),
            1
        );

        // Welche Rollen gelten, folgt aus den geteilten Ordnern der Unit:
        // mit der Schicht die einer Wegwerf-Sitzung.
        let share = |jail: &str| SharedDir {
            jail_path: PathBuf::from(jail),
            dev: 1,
            ino: 2,
        };
        let work = [share(super::super::lines::WORK_SHARE)];
        let layered = [
            share(super::super::lines::WORK_SHARE),
            share(super::super::lines::LAYER_SHARE),
        ];
        assert_eq!(Roles::of(disposable(&work)), direct);
        assert_eq!(Roles::of(disposable(&layered)), layered_roles);
    }

    /// Ein Prozess, der nach dem ersten Lesen der cgroup startet, wird
    /// nachgelesen und geprüft; kommen immer neue, sagt `settle` es.
    /// Mutationsbeweis: ohne das Nachlesen bleibt der späte `sleep` ungesehen.
    #[test]
    fn a_process_started_after_the_first_listing_is_still_checked() {
        let mut processes = healthy();
        let mut listings = vec![vec![10, 11, 12, 13, 99], vec![10, 11, 12, 13, 99]].into_iter();
        let settled = settle(
            &mut processes,
            || Ok(listings.next().unwrap_or_default()),
            |pid| {
                Some(Process {
                    pid,
                    program: Some("/usr/bin/sleep".to_owned()),
                    ..Process::default()
                })
            },
        )
        .unwrap();
        assert!(settled);
        assert!(processes.iter().any(|process| process.pid == 99));
        let found = judge(&processes, &resolved());
        assert!(
            found.iter().any(|f| f.what.contains("/usr/bin/sleep")),
            "{found:?}"
        );

        let mut processes = healthy();
        let mut next = 100;
        let settled = settle(
            &mut processes,
            || {
                next += 1;
                Ok(vec![next])
            },
            |pid| {
                Some(Process {
                    pid,
                    ..Process::default()
                })
            },
        )
        .unwrap();
        assert!(!settled, "a unit that keeps forking never settles");
    }
}
