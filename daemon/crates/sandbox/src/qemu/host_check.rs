//! Die Prüfung des VMM auf dem Host (HUM-444).
//!
//! Die `CHECK`-Zeilen kommen aus dem Gast, und Root im Gast kann sie
//! fälschen. Was ein Gast nicht fälschen kann, prüft der Daemon selbst, an
//! den Prozessen der Unit:
//!
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
//!   `--forward-cid`.
//!
//! Jeder Fund nennt die Garantie, die er bricht; was sich nicht lesen lässt,
//! ist ein Fund und nie ein Bestehen.

use std::path::{Path, PathBuf};

use crate::launcher::{IsolationCheck, UnitStop};

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
}

/// Liest die Prozesse der Unit und das Laufzeitverzeichnis und prüft sie.
pub(crate) fn run(stop: &UnitStop, resolved: &Resolved, door_port: u32) -> Vec<Finding> {
    let processes: Vec<Process> = match unit_pids(stop) {
        Ok(pids) if !pids.is_empty() => pids.into_iter().filter_map(read_process).collect(),
        Ok(_) => {
            return vec![finding(
                IsolationCheck::NoNetworkInterface,
                "the unit has no process left to check",
            )];
        }
        Err(why) => return vec![finding(IsolationCheck::NoNetworkInterface, why)],
    };
    let mut findings = judge(&processes, resolved);
    findings.extend(runtime_sockets(&stop.runtime, door_port));
    findings
}

/// Die PIDs der Unit, aus ihrer cgroup und allen darunter.
fn unit_pids(stop: &UnitStop) -> Result<Vec<u32>, String> {
    let output = crate::user_unit::run_bounded(
        stop.units.show_command(&stop.unit, "ControlGroup"),
        std::time::Duration::from_secs(10),
    )
    .map_err(|err| format!("systemctl did not name the unit's cgroup: {err}"))?;
    let group = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || !group.starts_with('/') {
        return Err(format!(
            "systemctl named no cgroup for {} ({:?})",
            stop.unit.service(),
            group
        ));
    }
    let root = Path::new("/sys/fs/cgroup").join(group.trim_start_matches('/'));
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

/// Was `/proc` über einen Prozess sagt; `None` nur für einen Prozess, den
/// es nicht mehr gibt.
fn read_process(pid: u32) -> Option<Process> {
    let proc = PathBuf::from(format!("/proc/{pid}"));
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
    Some(Process {
        pid,
        program,
        args,
        interfaces,
        fds,
    })
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
    let mut findings = Vec::new();
    let mut devices = 0;
    let mut emulators = 0;
    for process in processes {
        let pid = process.pid;
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
pub(crate) fn runtime_sockets(dir: &Path, door_port: u32) -> Vec<Finding> {
    use std::os::unix::fs::FileTypeExt as _;
    /// So tief liegen die Dateien der Sitzung höchstens.
    const MAX_DEPTH: usize = 3;
    let declared = Layout::declared_sockets(door_port);
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
        };
        vec![
            process(
                10,
                "sh",
                &["/bin/sh", "-c", "x"],
                &["/dev/null", "pipe:[1]"],
            ),
            process(11, "bwrap", &["bwrap"], &["/dev/null"]),
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
        for (name, mutate, check) in deviations() {
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
        for name in Layout::declared_sockets(5000) {
            held.push(UnixListener::bind(dir.path().join(name)).unwrap());
        }
        std::fs::write(dir.path().join("files/guest-plan.json"), "{}").unwrap();
        assert_eq!(runtime_sockets(dir.path(), 5000), Vec::new());

        held.push(UnixListener::bind(dir.path().join("door/door_22")).unwrap());
        let found = runtime_sockets(dir.path(), 5000);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].what.ends_with("door_22"), "{found:?}");
        assert_eq!(found[0].check, IsolationCheck::SingleSocket);
        std::fs::remove_file(dir.path().join("door/door_22")).unwrap();

        // Derselbe Name an einem anderen Ort ist nicht erklärt.
        held.push(UnixListener::bind(dir.path().join("files/door")).unwrap());
        assert_eq!(runtime_sockets(dir.path(), 5000).len(), 1);
        std::fs::remove_file(dir.path().join("files/door")).unwrap();

        std::os::unix::fs::symlink("/run", dir.path().join("files/link")).unwrap();
        let found = runtime_sockets(dir.path(), 5000);
        assert!(found.iter().any(|f| f.what.contains("a link")), "{found:?}");
        std::fs::remove_file(dir.path().join("files/link")).unwrap();

        // Fehlt ein erklärter Socket, öffnet das keinen Weg (virtiofsd 1.13
        // entfernt seinen nach der Verbindung); eine Datei unter seinem
        // Namen ist ein Befund.
        std::fs::remove_file(dir.path().join("work/work.sock")).unwrap();
        assert_eq!(runtime_sockets(dir.path(), 5000), Vec::new());
        std::fs::write(dir.path().join("work/work.sock"), "").unwrap();
        let found = runtime_sockets(dir.path(), 5000);
        assert!(
            found.iter().any(|f| f.what.contains("is not a socket")),
            "{found:?}"
        );
    }
}
