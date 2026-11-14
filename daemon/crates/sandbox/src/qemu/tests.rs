//! Unit-Tests des Adapters der Kernel-Stufe ohne KVM (HUM-444).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::io::Read as _;
use std::os::fd::{AsFd as _, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use humanitl_config::WorkMode;
use humanitl_core::ids::SessionId;

use super::*;

fn profile() -> SandboxProfile {
    SandboxProfile::parse(
        include_str!("../../../../../profiles/sandbox/default.toml"),
        Path::new("default.toml"),
    )
    .unwrap()
}

fn session(work: &Path) -> SessionContext {
    SessionContext {
        session: SessionId::new(),
        work_src: work.to_path_buf(),
        work_mode: WorkMode::Rw,
        proxy_socket_src: PathBuf::from("/run/user/1000/humanitl/proxy/proxy.sock"),
        ca_cert_src: PathBuf::from("/var/lib/h/ca.crt"),
        ca_bundle_src: PathBuf::from("/var/lib/h/bundle.crt"),
        shim_src: PathBuf::from("/usr/lib/humanitl/humanitl-shim"),
        session_env: vec![("HUMANITL_SESSION".to_owned(), "s".to_owned())],
        command: vec![OsString::from("/bin/bash"), OsString::from("-l")],
        files: vec![crate::agent::SandboxFile {
            dst: PathBuf::from("/home/agent/.config/agent.json"),
            content: b"{\"key\":\"secret\"}".to_vec(),
            mode: 0o400,
            secret: true,
        }],
        agent_program: None,
    }
}

/// Die Wurzel des Gasts kommt aus demselben Plan wie unter bwrap: die Tür ist
/// keine Datei, `/work` ein leerer Einhängepunkt, die Masken unter `/work`
/// gehen an den virtiofsd des Projekts, Identität und Dateien des Adapters
/// werden Dateien des Gasts, und alles ist nur lesbar.
#[test]
fn the_guest_root_comes_from_the_bwrap_plan() {
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join(".env"), "SECRET=1").unwrap();
    std::fs::create_dir_all(work.path().join(".git/hooks")).unwrap();
    let profile = profile();
    let session = session(work.path());
    let present = BwrapBackend::present_under_work(&profile, work.path());
    let files = PathBuf::from("/run/x/files");
    let root = guest_root(&profile, &session, &present, &files, Path::new("/m"), &[]);

    let binds: Vec<(&Path, &Path)> = root
        .entries
        .iter()
        .filter_map(|entry| match entry {
            RootEntry::Bind { source, target } => Some((source.as_path(), target.as_path())),
            _ => None,
        })
        .collect();
    assert!(binds.contains(&(Path::new("/usr"), Path::new("/usr"))));
    assert!(binds.contains(&(
        Path::new("/usr/lib/humanitl/humanitl-shim"),
        profile.network.shim_dst.as_path()
    )));
    assert!(binds.contains(&(Path::new("/m"), Path::new(GUEST_MODULES))));
    assert!(
        !binds
            .iter()
            .any(|(source, _)| *source == session.proxy_socket_src),
        "the proxy socket never reaches the guest root: {binds:?}"
    );
    assert!(!binds.iter().any(|(_, target)| target.starts_with("/work")));
    assert!(
        root.entries
            .contains(&RootEntry::Dir(PathBuf::from("/work")))
    );
    assert!(root.masks.contains(&WorkMask::Empty(PathBuf::from(".env"))));
    assert!(
        root.masks
            .contains(&WorkMask::Tmpfs(PathBuf::from(".git/hooks")))
    );
    // Nur Masken über Vorhandenem.
    assert!(
        !root
            .masks
            .contains(&WorkMask::Empty(PathBuf::from(".npmrc")))
    );
    let passwd = binds
        .iter()
        .find(|(_, target)| *target == Path::new(crate::PASSWD_DST))
        .unwrap()
        .0;
    let content = &root
        .files
        .iter()
        .find(|(path, _)| path == passwd)
        .unwrap()
        .1;
    assert!(String::from_utf8_lossy(content).contains(":x:"));
    let agent = binds
        .iter()
        .find(|(_, target)| *target == Path::new("/home/agent/.config/agent.json"))
        .unwrap()
        .0;
    assert!(agent.starts_with(&files));
    assert!(
        root.policy
            .iter()
            .all(|row| row.mode != PolicyMode::ReadWrite || row.target.starts_with("/work")),
        "outside /work the guest root is read-only: {:?}",
        root.policy
    );
}

/// Der Gastplan trägt die Tür über vsock, den Inode des Projekts und die
/// beschreibbaren Orte für die Suche nach Sockets; keine Brücke von bwrap.
#[test]
fn the_guest_plan_names_the_door_the_pin_and_the_walk() {
    let work = tempfile::tempdir().unwrap();
    let profile = profile();
    let session = session(work.path());
    let env = profile.effective_env(&session, None);
    let text = guest_plan(&profile, &session, &env, 4242, None, &[]).unwrap();
    let plan: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(plan["door_port"], "5000");
    assert_eq!(plan["report_port"], "5001");
    assert_eq!(plan["work_tag"], "work");
    assert_eq!(plan["pins"]["pins"][0]["ino"], "4242");
    assert_eq!(
        plan["socket_walk"],
        serde_json::json!(["/work", "/dev", "/run"])
    );
    assert_eq!(plan["command"], serde_json::json!(["/bin/bash", "-l"]));
    assert!(plan["shim"].get("HUMANITL_BRIDGES").is_none());
    assert!(plan["env"].get("HUMANITL_SESSION").is_some());
    let mounts = plan["pins"]["mounts"].as_array().unwrap();
    let virtiofs = mounts.iter().filter(|m| m["type"] == "virtiofs").count();
    assert_eq!(virtiofs, 2, "only / and /work, no announced submount");
    // HUM-445: wie unter bwrap ein unprivilegierter Agent, `/sys` leer,
    // `/dev/shm` ein tmpfs, der Name des Profils; keine Probe ohne Filter.
    assert_eq!(plan["uid"], rustix::process::getuid().as_raw().to_string());
    assert_eq!(plan["gid"], rustix::process::getgid().as_raw().to_string());
    assert_eq!(plan["hostname"], "sandbox");
    let at = |point: &str| {
        mounts
            .iter()
            .find(|m| m["point"] == point)
            .unwrap_or_else(|| panic!("{point}: {mounts:?}"))
    };
    assert_eq!(at("/sys")["type"], "tmpfs");
    assert_eq!(at("/sys")["mode"], "ro");
    assert_eq!(at("/dev/shm")["type"], "tmpfs");
    assert!(plan.get("root_probe").is_none());
    let probed = guest_plan(&profile, &session, &env, 4242, Some("true"), &[]).unwrap();
    let probed: serde_json::Value = serde_json::from_str(&probed).unwrap();
    assert_eq!(
        probed["root_probe"],
        serde_json::json!(["/bin/sh", "-c", "true"])
    );
}

/// HUM-466: Jedes tmpfs des Profils außerhalb von `/work` steht im
/// Gastplan, mit seiner Zeile der Mounttabelle und als Wurzel der
/// Socket-Suche; die Datei des Adapters unter dem Heimatverzeichnis bleibt
/// als nur lesbare Kopie obenauf, und `/dev/shm` hängt der Shim ohnehin ein.
#[test]
fn the_guest_plan_names_every_tmpfs_and_what_stays_on_top() {
    let work = tempfile::tempdir().unwrap();
    let profile = profile();
    let session = session(work.path());
    let present = BwrapBackend::present_under_work(&profile, work.path());
    let root = guest_root(
        &profile,
        &session,
        &present,
        Path::new("/run/x/files"),
        Path::new("/m"),
        &[],
    );
    let env = profile.effective_env(&session, None);
    let text = guest_plan(&profile, &session, &env, 4242, None, &root.entries).unwrap();
    let plan: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        plan["tmpfs"],
        serde_json::json!(["/run", "/tmp", "/var/tmp", "/home/agent"])
    );
    // `/run/humanitl` (Shim, Gastplan, Module) bleibt als eine nur lesbare
    // Kopie über dem tmpfs von `/run`.
    let mut keep: Vec<&str> = plan["tmpfs_keep"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    keep.sort_unstable();
    assert_eq!(
        keep,
        ["bind:/home/agent/.config/agent.json", "bind:/run/humanitl"]
    );
    assert_eq!(
        plan["socket_walk"],
        serde_json::json!(["/work", "/dev", "/run", "/tmp", "/var/tmp", "/home/agent"])
    );
    let mounts = plan["pins"]["mounts"].as_array().unwrap();
    let at = |point: &str| -> Vec<&serde_json::Value> {
        mounts.iter().filter(|m| m["point"] == point).collect()
    };
    for point in ["/run", "/tmp", "/var/tmp", "/home/agent", "/dev/shm"] {
        let found = at(point);
        assert_eq!(found.len(), 1, "{point}: {mounts:?}");
        assert_eq!(found[0]["type"], "tmpfs", "{point}");
        assert_eq!(found[0]["mode"], "rw", "{point}");
        assert_eq!(found[0]["flags"], "nosuid,nodev", "{point}");
    }
    for point in ["/home/agent/.config/agent.json", "/run/humanitl"] {
        let kept = at(point);
        assert_eq!(kept.len(), 1, "{point}: {mounts:?}");
        assert_eq!(kept[0]["type"], "virtiofs", "{point}");
        assert_eq!(kept[0]["mode"], "ro", "{point}");
    }
    // Kein tmpfs unter `/work`: Die Masken dort trägt der virtiofsd des
    // Projekts, nicht der Shim.
    assert!(
        !plan["tmpfs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().unwrap().starts_with("/work")),
        "{}",
        plan["tmpfs"]
    );
}

/// HUM-466: `mounts.extra_rw` wird unter der Kernel-Stufe abgelehnt, mit
/// eigenem Code, nie stillschweigend nur lesbar eingehängt.
#[test]
fn a_writable_extension_is_refused_with_its_own_code() {
    assert!(refuse_writable_extensions(&profile()).is_ok());
    let text = include_str!("../../../../../profiles/sandbox/default.toml")
        .replace("extra_rw = []", "extra_rw = [\"/srv/data\"]");
    let profile = SandboxProfile::parse(&text, Path::new("default.toml")).unwrap();
    let refused = refuse_writable_extensions(&profile).unwrap_err();
    assert_eq!(refused.code.as_str(), "SANDBOX_051");
    assert!(refused.why.contains("/srv/data"), "{}", refused.why);
    assert!(refused.fix.is_some());
}

/// Die nur lesbaren Binds der Sitzung (das Testverzeichnis der
/// Escape-Suite) stehen in der Wurzel des Gasts, nur lesbar (HUM-445).
#[test]
fn the_session_binds_reach_the_guest_root() {
    let work = tempfile::tempdir().unwrap();
    let profile = profile();
    let session = session(work.path());
    let present = BwrapBackend::present_under_work(&profile, work.path());
    let binds = [(PathBuf::from("/srv/tests"), PathBuf::from("/tests/escape"))];
    let root = guest_root(
        &profile,
        &session,
        &present,
        Path::new("/run/x/files"),
        Path::new("/m"),
        &binds,
    );
    assert!(root.entries.contains(&RootEntry::Bind {
        source: PathBuf::from("/srv/tests"),
        target: PathBuf::from("/tests/escape"),
    }));
    assert!(root.policy.iter().any(|row| row.target == Path::new("/tests/escape")
        && row.mode == PolicyMode::ReadOnly));
}

/// Die Eigenschaften der Unit sind die aus HUM-442 Q7, dazu die
/// beschreibbaren Pfade; ein Pfad mit Leerraum wird abgelehnt.
#[test]
fn the_unit_carries_the_hardening_of_the_spike() {
    let properties = unit_properties("/run/user/1000/humanitl-vm-x/s", Some("/home/u/p"));
    for wanted in [
        "PrivateNetwork=yes",
        "PrivateUsers=yes",
        "NoNewPrivileges=yes",
        "CapabilityBoundingSet=",
        "ProtectSystem=strict",
        "ProtectHome=read-only",
        "RestrictAddressFamilies=AF_UNIX AF_NETLINK",
        "LockPersonality=yes",
        "RestrictRealtime=yes",
        "SystemCallArchitectures=native",
        "ReadWritePaths=/run/user/1000/humanitl-vm-x/s /home/u/p",
    ] {
        assert!(
            properties.iter().any(|p| p == wanted),
            "{wanted}: {properties:?}"
        );
    }
    assert_eq!(
        property_path(Path::new("/home/u/a b"))
            .unwrap_err()
            .code
            .as_str(),
        "SANDBOX_006"
    );
    assert_eq!(property_path(Path::new("/home/u/p")).unwrap(), "/home/u/p");
    for odd in [
        "/run/a,b",
        "/opt/q%n/qemu",
        "/x;y",
        "/x'y",
        "/x\\y",
        "relative",
    ] {
        assert_eq!(
            property_path(Path::new(odd)).unwrap_err().code.as_str(),
            "SANDBOX_006",
            "{odd}"
        );
    }
}

/// Der gebaute Shim neben dem Testprogramm.
pub(super) fn built_shim() -> Option<PathBuf> {
    let mut dir = std::env::current_exe().ok()?;
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    Some(dir.join("humanitl-shim")).filter(|shim| shim.is_file())
}

/// `humanitl-shim --fds-then-exec`: Die Deskriptoren der Übergabe liegen
/// auf 3, 4 und 5, wenn der Befehl läuft, und nichts sonst kommt mit.
#[test]
fn the_first_process_of_the_unit_gets_its_descriptors_by_handover() {
    let Some(shim) = built_shim() else {
        eprintln!("skipped: the shim is not built next to the test binary");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("marker");
    std::fs::create_dir(&marker).unwrap();
    let listener = crate::fd_handover::HandoverListener::bind(&dir.path().join("h")).unwrap();
    let token = listener.token().to_owned();
    let (lifeline, read) = crate::shim_fds::Lifeline::new().unwrap();
    let project = OwnedFd::from(std::fs::File::open(&marker).unwrap());
    let socket =
        OwnedFd::from(std::os::unix::net::UnixListener::bind(dir.path().join("s")).unwrap());
    let mut child = Command::new(&shim)
        .args(["--fds-then-exec"])
        .arg(dir.path().join("h"))
        .args([&token, "--", "/bin/sh", "-c"])
        .arg("for n in 3 4 5 6; do readlink /proc/self/fd/$n || echo none; done")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let fds = [socket.as_fd(), project.as_fd(), read.as_fd()];
    listener
        .serve(
            &fds,
            Duration::from_secs(10),
            rustix::process::getuid().as_raw(),
        )
        .unwrap();
    let mut out = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    assert!(child.wait().unwrap().success());
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("socket:["), "{out}");
    assert_eq!(Path::new(lines[1]), marker, "{out}");
    assert!(lines[2].starts_with("pipe:["), "{out}");
    assert_eq!(lines[3], "none", "{out}");
    drop(lifeline);
}

/// `BindsTo` gilt nur für einen Dienst des Dienstverwalters des Nutzers; ein
/// Dienst des Systems, etwa der eines CI-Runners, ist dort unbekannt, und die
/// Unit endete sofort.
#[test]
fn only_a_service_of_the_users_manager_is_bound_to() {
    assert_eq!(
        user_service_of(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/humanitld.service",
            1000
        )
        .as_deref(),
        Some("humanitld.service")
    );
    assert_eq!(
        user_service_of("/system.slice/actions.runner.x.service", 1001),
        None
    );
    assert_eq!(
        user_service_of(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/humanitld.service",
            1001
        ),
        None
    );
    assert_eq!(
        user_service_of("/user.slice/user-1000.slice/user@1000.service", 1000),
        None
    );
    assert_eq!(
        user_service_of(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/x.scope",
            1000
        ),
        None
    );
}

/// Die Ergebnisse eines Berichts aus diesen Zeilen, wie der Daemon sie liest.
fn results_of_lines(lines: &[&str]) -> Vec<CheckResult> {
    let report = crate::handle::ReportSnapshot {
        checks: lines
            .iter()
            .filter_map(|line| crate::bridge_env::parse_check_line(line))
            .collect(),
        ..crate::handle::ReportSnapshot::default()
    };
    crate::bwrap::results_of_report(&report, "no CHECK line from the shim within 60s")
}

/// Was die Host-Prüfung über eine Unit sagt, die schon fort ist.
fn unit_gone() -> Vec<host_check::Finding> {
    vec![host_check::Finding {
        check: IsolationCheck::NoNetworkInterface,
        what: "the unit humanitl-test-vm-470.service has ended or never started: systemctl \
               names no cgroup for it"
            .to_owned(),
    }]
}

/// Die Diagnose, die ein Start zeigt: die der ersten roten Garantie, wie im
/// Daemon (`first_failure`) und in escape-launch.
fn shown_code(results: &[CheckResult]) -> Option<String> {
    results
        .iter()
        .find(|result| !result.passed)
        .and_then(|result| result.diagnostic.as_ref())
        .map(|diagnostic| diagnostic.code.as_str().to_owned())
}

/// HUM-470, `esc-2/unplanned_mount` in der CI: Der Shim hält an der
/// Mounttabelle an (`SANDBOX_049`) und schaltet den Gast ab; die Unit ist
/// fort, bevor der Host nachsieht, und die Host-Prüfung findet keine cgroup.
/// Die Diagnose des Starts bleibt die des Gasts, der Fund des Hosts steht im
/// Befund, und jede Garantie bleibt rot. Mutationsbeweis: Überschreibt der
/// Fund des Hosts wieder jede Diagnose mit `SANDBOX_047`, zeigt der Start
/// `SANDBOX_047`.
#[test]
fn a_guest_refusal_keeps_its_diagnosis_when_the_unit_is_already_gone() {
    let mut results = results_of_lines(&[
        "MODULES vsock=loaded modules_disabled=1",
        "CHECK mounts_match fail extra:/dev/pts:devpts",
    ]);
    assert_eq!(shown_code(&results).as_deref(), Some("SANDBOX_049"));
    host_check::merge(&mut results, &unit_gone());
    assert!(results.iter().all(|result| !result.passed), "{results:?}");
    assert_eq!(
        shown_code(&results).as_deref(),
        Some("SANDBOX_049"),
        "{results:?}"
    );
    let interfaces = &results[0];
    assert_eq!(interfaces.check, IsolationCheck::NoNetworkInterface);
    assert!(
        interfaces
            .evidence
            .contains("host: the unit humanitl-test-vm-470.service has ended"),
        "{}",
        interfaces.evidence
    );
}

/// HUM-470, Fall A der CI: Ohne Bericht bleibt `SANDBOX_013` mit dem Grund
/// des fehlenden Berichts die Diagnose, auch wenn die Unit fort ist.
#[test]
fn a_missing_report_keeps_its_diagnosis_when_the_unit_is_already_gone() {
    let mut results = results_of_lines(&[]);
    host_check::merge(&mut results, &unit_gone());
    assert!(results.iter().all(|result| !result.passed));
    assert_eq!(shown_code(&results).as_deref(), Some("SANDBOX_013"));
    assert!(
        results[0].evidence.contains("host: the unit"),
        "{results:?}"
    );
}

/// HUM-470, Fall A über den Faden zum Gast: Kam keine Verbindung zustande,
/// schreibt er [`session::NO_REPORT`] in die Pipe. Das ist das Fehlen des
/// Berichts, kein Bericht, und die Diagnose ist `SANDBOX_013` mit seinem
/// Grund, auch wenn die Unit schon fort ist. Mutationsbeweis: Ohne die
/// Erkennung der Ersatzzeile zeigt der Start `SANDBOX_014`, weil
/// `no_interfaces` fehlt.
#[test]
fn the_link_s_no_report_line_is_a_missing_report() {
    let mut results = results_of_lines(&[session::NO_REPORT]);
    host_check::merge(&mut results, &unit_gone());
    assert!(results.iter().all(|result| !result.passed));
    for result in &results {
        assert_eq!(
            result.diagnostic.as_ref().map(|d| d.code.as_str()),
            Some("SANDBOX_013"),
            "{results:?}"
        );
        assert!(
            result
                .evidence
                .starts_with("no CHECK line from the shim: no-report:the_guest_ended"),
            "{}",
            result.evidence
        );
    }
}

/// Bestand der Bericht des Gasts, ist jeder Fund des Hosts `SANDBOX_047`,
/// wie seit HUM-444; nur die Garantie mit dem Fund wird rot.
#[test]
fn a_host_finding_after_a_green_report_is_sandbox_047() {
    let mut results = results_of_lines(&[
        "CHECK bridge_listening ok 127.0.0.1:3128->vsock:2:5000",
        "CHECK single_socket ok none",
        "CHECK mounts_match ok 12",
        "CHECK mounts_pinned ok work",
        "CHECK no_interfaces ok lo",
        "CHECK seccomp_applied ok filter",
        "CHECK families ok socket(AF_VSOCK,SOCK_STREAM)=EPERM",
    ]);
    assert!(results.iter().all(|result| result.passed), "{results:?}");
    host_check::merge(
        &mut results,
        &[host_check::Finding {
            check: IsolationCheck::SingleSocket,
            what: "an undeclared socket door/door_22".to_owned(),
        }],
    );
    assert!(results[0].passed && results[2].passed, "{results:?}");
    assert!(!results[1].passed);
    assert_eq!(shown_code(&results).as_deref(), Some("SANDBOX_047"));
    assert!(
        results[1]
            .evidence
            .ends_with("; host: an undeclared socket door/door_22")
    );
}

/// Hat der Gast eine Garantie bestanden, eine andere aber abgelehnt, und
/// sieht der Host beim Abschalten an der bestandenen etwas, wird sie rot und
/// trägt die Diagnose des Gasts: Die Ursache der Sitzung ist seine Ablehnung.
#[test]
fn a_host_finding_on_a_passed_guarantee_takes_the_guests_refusal() {
    let mut results = results_of_lines(&[
        "CHECK bridge_listening ok 127.0.0.1:3128->vsock:2:5000",
        "CHECK single_socket ok none",
        "CHECK mounts_match ok 12",
        "CHECK mounts_pinned ok work",
        "CHECK no_interfaces ok lo",
        "CHECK seccomp_applied fail install:EINVAL",
    ]);
    assert!(results[0].passed);
    host_check::merge(
        &mut results,
        &[host_check::Finding {
            check: IsolationCheck::NoNetworkInterface,
            what: "0 emulator processes in the unit, expected exactly one".to_owned(),
        }],
    );
    assert!(!results[0].passed && results[1].passed && !results[2].passed);
    assert_eq!(shown_code(&results).as_deref(), Some("SANDBOX_016"));
    assert!(results[0].evidence.contains("host: 0 emulator processes"));
    let why = &results[0].diagnostic.as_ref().unwrap().why;
    assert!(
        why.starts_with(
            "no_network_interface: the host found 0 emulator processes in the unit, expected \
             exactly one while the guest had refused seccomp_active: "
        ),
        "{why}"
    );
}
