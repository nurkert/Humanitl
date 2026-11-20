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
        agent_depot: crate::agent::depot::DepotSession::default(),
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
    let text = guest_plan(&profile, &session, &env, 4242, None, &[], &[], false).unwrap();
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
    let probed = guest_plan(
        &profile,
        &session,
        &env,
        4242,
        Some("true"),
        &[],
        &[],
        false,
    )
    .unwrap();
    let probed: serde_json::Value = serde_json::from_str(&probed).unwrap();
    assert_eq!(
        probed["root_probe"],
        serde_json::json!(["/bin/sh", "-c", "true"])
    );
}

/// HUM-446: Der Gastplan einer Wegwerf-Sitzung lädt `overlay` vor der
/// Modulsperre, nennt die Schicht und den Inode des Projekts für die untere
/// Schicht, erwartet an `/work` ein Overlay und keinen Pin auf das Projekt,
/// dessen Inode das Overlay nicht trägt. Die Unit darf dann nur die Schicht
/// beschreiben, nie das Projekt.
#[test]
fn a_disposable_guest_plan_mounts_an_overlay_over_the_checked_folder() {
    let work = tempfile::tempdir().unwrap();
    let profile = profile();
    let session = session(work.path());
    let env = profile.effective_env(&session, None);
    let text = guest_plan(&profile, &session, &env, 4242, None, &[], &[], true).unwrap();
    let plan: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(plan["modules"], serde_json::json!(["overlay"]));
    assert_eq!(
        plan["disposable"],
        serde_json::json!({"layer_tag": "layer", "lower_ino": "4242", "mask_mib": "64"})
    );
    assert_eq!(plan["pins"]["pins"], serde_json::json!([]));
    let mounts = plan["pins"]["mounts"].as_array().unwrap();
    let work_entry = mounts.iter().find(|m| m["point"] == "/work").unwrap();
    assert_eq!(work_entry["type"], "overlay");
    assert_eq!(work_entry["mode"], "rw");
    assert_eq!(work_entry["flags"], "nosuid,nodev");
    assert_eq!(
        mounts.iter().filter(|m| m["type"] == "virtiofs").count(),
        1,
        "only the root; the project and the layer are no mounts of their own"
    );
    let direct = guest_plan(&profile, &session, &env, 4242, None, &[], &[], false).unwrap();
    let direct: serde_json::Value = serde_json::from_str(&direct).unwrap();
    assert!(direct.get("disposable").is_none() && direct.get("modules").is_none());

    let properties = unit_properties(
        "/run/user/1000/humanitl-vm-x/s",
        &["/run/user/1000/l"],
        UnitLimits::of(
            Machine {
                cpus: 1,
                memory_mib: 512,
            },
            0,
        ),
        &["/usr/bin/dash".to_owned()],
        &[],
    );
    assert!(
        properties
            .iter()
            .any(|p| p == "ReadWritePaths=/run/user/1000/humanitl-vm-x/s /run/user/1000/l"),
        "{properties:?}"
    );
    // Was die Unit beschreiben darf, führt sie nicht aus (HUM-480).
    assert!(
        properties
            .iter()
            .any(|p| p == "NoExecPaths=/ /run/user/1000/humanitl-vm-x/s /run/user/1000/l"),
        "{properties:?}"
    );
}

/// HUM-481: In einer Wegwerf-Sitzung legt der Shim die Masken des Profils,
/// die im Projekt vorhanden sind, über das Overlay: Der Gastplan nennt sie
/// in der Reihenfolge des Profils, und die Mounttabelle erwartet je eine
/// Zeile, ein Ordner beschreibbar, eine Datei nur lesbar. Eine direkte
/// Sitzung nennt keine; dort liegen die Masken im Gefängnis des Dienstes
/// für `/work`, das der Gast nur über virtiofs sieht.
#[test]
fn a_disposable_guest_plan_lays_the_present_masks_over_the_overlay() {
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(work.path().join(".git/hooks")).unwrap();
    std::fs::write(work.path().join(".env"), "SECRET=yes\n").unwrap();
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
    let plan = |disposable: bool| -> serde_json::Value {
        let text = guest_plan(
            &profile,
            &session,
            &env,
            4242,
            None,
            &root.entries,
            &root.masks,
            disposable,
        )
        .unwrap();
        serde_json::from_str(&text).unwrap()
    };
    let disposable = plan(true);
    let masks: Vec<&str> = disposable["work_masks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|mask| mask.as_str().unwrap())
        .collect();
    assert!(masks.contains(&"tmpfs:.git/hooks"), "{masks:?}");
    assert!(masks.contains(&"empty:.env"), "{masks:?}");
    assert_eq!(masks.len(), root.masks.len(), "only what is present");
    let mounts = disposable["pins"]["mounts"].as_array().unwrap();
    let at = |point: &str| {
        mounts
            .iter()
            .find(|m| m["point"] == point)
            .unwrap_or_else(|| panic!("{point}: {mounts:?}"))
    };
    assert_eq!(at("/work/.git/hooks")["mode"], "rw");
    assert_eq!(at("/work/.git/hooks")["type"], "tmpfs");
    assert_eq!(at("/work/.env")["mode"], "ro");
    assert_eq!(at("/work/.env")["flags"], "nosuid,nodev");
    let direct = plan(false);
    assert!(direct.get("work_masks").is_none());
    assert!(
        direct["pins"]["mounts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["point"] != "/work/.env"),
        "a direct session has its masks on the host"
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
    let text = guest_plan(
        &profile,
        &session,
        &env,
        4242,
        None,
        &root.entries,
        &root.masks,
        false,
    )
    .unwrap();
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
/// beschreibbaren Pfade, die Grenze der Deskriptoren (HUM-469) und die
/// Grenzen an Speicher und Tasks (HUM-485), nur die Programme der Unit
/// ausführbar (HUM-480); ein Pfad mit Leerraum wird abgelehnt.
#[test]
fn the_unit_carries_the_hardening_of_the_spike() {
    let machine = Machine {
        cpus: 2,
        memory_mib: 1024,
    };
    let properties = unit_properties(
        "/run/user/1000/humanitl-vm-x/s",
        &["/home/u/p"],
        UnitLimits::of(machine, 300),
        &["/usr/bin/dash".to_owned(), "/usr/lib/libc.so.6".to_owned()],
        &["/run/user/1000/humanitl-vm-x/s/masks/0".to_owned()],
    );
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
        "LimitNOFILE=65536",
        "MemoryMax=1836M",
        "MemorySwapMax=1836M",
        "TasksMax=130",
        "NoExecPaths=/ /run/user/1000/humanitl-vm-x/s /home/u/p",
        "ExecPaths=/usr/bin/dash /usr/lib/libc.so.6",
        // HUM-557: Die Ordner-Masken unter `/work` sind tmpfs der Unit mit
        // fester Größe und `noexec`.
        "TemporaryFileSystem=/run/user/1000/humanitl-vm-x/s/masks/0:size=67108864,mode=0700,\
         noexec,nosuid,nodev",
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

/// HUM-486: Hat der Faden zum Gast ein stummes vsock-Gerät festgehalten,
/// wird aus dem fehlenden Bericht `SANDBOX_055` mit dem Stand des Wegs im
/// Grund; ohne den Vermerk bleibt es `SANDBOX_013` mit demselben Stand.
/// Mutationsbeweis: Ohne den Zweig für `silent` in `name_the_link` zeigt
/// der Start `SANDBOX_013`.
#[test]
fn a_silent_vsock_device_is_sandbox_055() {
    let link = "the guest said ready after 2.1s; the CONNECT for the report connection has had no \
                answer from the vsock device for 10.0s";
    let mut silent = results_of_lines(&[session::NO_REPORT]);
    name_the_link(&mut silent, link, true);
    let mut waiting = results_of_lines(&[session::NO_REPORT]);
    name_the_link(&mut waiting, link, false);
    for (results, code) in [(&silent, "SANDBOX_055"), (&waiting, "SANDBOX_013")] {
        assert!(results.iter().all(|result| !result.passed), "{results:?}");
        for result in results {
            let diagnostic = result.diagnostic.as_ref().unwrap();
            assert_eq!(diagnostic.code.as_str(), code, "{results:?}");
            assert!(diagnostic.why.contains(link), "{}", diagnostic.why);
            assert!(result.evidence.contains(link), "{}", result.evidence);
        }
    }
    assert_eq!(shown_code(&silent).as_deref(), Some("SANDBOX_055"));
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

/// Der Vermerk der Tür (HUM-467) steht nur im Beleg von `single_socket`,
/// und ohne Vermerk bleibt jeder Beleg, wie er war; am Ergebnis ändert er
/// nichts.
#[test]
fn the_door_note_stands_in_the_evidence_of_the_single_socket_only() {
    let fresh = || {
        IsolationCheck::ALL
            .iter()
            .map(|&check| CheckResult {
                check,
                passed: true,
                evidence: "seen".to_owned(),
                diagnostic: None,
            })
            .collect::<Vec<_>>()
    };
    let mut results = fresh();
    name_the_door(&mut results, None);
    assert!(results.iter().all(|result| result.evidence == "seen"));

    let mut results = fresh();
    name_the_door(&mut results, Some("closed one above the cap"));
    for result in &results {
        assert!(result.passed);
        let expected = if result.check == IsolationCheck::SingleSocket {
            "seen; door: closed one above the cap"
        } else {
            "seen"
        };
        assert_eq!(result.evidence, expected, "{:?}", result.check);
    }
}

/// HUM-485: Trägt die cgroup der Unit die geplanten Grenzen nicht, wird jede
/// grüne Garantie rot mit `SANDBOX_052`, und der Satz steht im Beleg. Hat
/// der Gast schon abgelehnt, bleibt seine Diagnose (HUM-470), und der Satz
/// steht nur im Beleg. Mutationsbeweis zur Prüfung selbst: der KVM-Test
/// `a_process_in_the_unit_cannot_start_more_tasks_than_tasks_max` und
/// `the_unit_cgroup_must_carry_the_planned_limits` in `limits.rs`.
#[test]
fn unenforced_limits_refuse_every_green_guarantee_with_sandbox_052() {
    let unenforced =
        limits::limits_unenforced("the unit's pids.max is \"max\", and the plan set 129");
    let mut results: Vec<CheckResult> = IsolationCheck::ALL
        .iter()
        .map(|&check| CheckResult {
            check,
            passed: true,
            evidence: "seen".to_owned(),
            diagnostic: None,
        })
        .collect();
    host_check::merge_limits(&mut results, &unenforced);
    for result in &results {
        assert!(!result.passed, "{results:?}");
        assert_eq!(
            result.diagnostic.as_ref().map(|d| d.code.as_str()),
            Some("SANDBOX_052")
        );
        assert!(
            result.evidence.contains("host: the unit's pids.max"),
            "{}",
            result.evidence
        );
    }

    let mut results = results_of_lines(&[
        "MODULES vsock=loaded modules_disabled=1",
        "CHECK mounts_match fail extra:/dev/pts:devpts",
    ]);
    host_check::merge(&mut results, &unit_gone());
    host_check::merge_limits(&mut results, &unenforced);
    assert_eq!(
        shown_code(&results).as_deref(),
        Some("SANDBOX_049"),
        "{results:?}"
    );
    assert!(results[0].evidence.contains("pids.max"), "{results:?}");
}

/// HUM-501: vCPU und Speicher über diesem Host lädt die Konfiguration; der
/// Plan der Kernel-Stufe lehnt sie ab, bevor er etwas prüft, anlegt oder
/// startet, mit dem Schlüssel, dem Grund und dem größten Wert, den der Host
/// annimmt. Die Voraussetzungen sind hier Attrappen: Der Plan kommt nie bis
/// zu ihnen. Mutationsbeweis: ohne die Prüfung im Plan scheitert er erst an
/// den Attrappen, mit einem anderen Code.
#[test]
fn the_plan_refuses_resources_over_this_host() {
    let host = HostResources::probe();
    let (Some(memory), Some(cpus)) = (host.memory_mib, host.cpus) else {
        panic!("this host names neither memory nor CPUs: {host:?}");
    };
    let work = tempfile::tempdir().unwrap();
    let runtime = tempfile::tempdir().unwrap();
    let profile = profile();
    let session = session(work.path());
    let options = crate::launcher::LaunchOptions::default();
    let resolved = Resolved {
        emulator: PathBuf::from("/nonexistent-hum501/emulator"),
        file_daemon: PathBuf::from("/nonexistent-hum501/files"),
        vsock_device: PathBuf::from("/nonexistent-hum501/vsock"),
        bubblewrap: PathBuf::from("/nonexistent-hum501/bubblewrap"),
        kernel: PathBuf::from("/nonexistent-hum501/kernel"),
        modules: PathBuf::from("/nonexistent-hum501/modules"),
        no_announce_submounts: true,
    };
    for (config, key, what, max) in [
        (
            QemuConfig {
                cpus: Some(cpus + 1),
                ..QemuConfig::default()
            },
            "cpus",
            "the CPUs this daemon may use",
            cpus.to_string(),
        ),
        (
            QemuConfig {
                memory_mib: Some(memory + 1),
                ..QemuConfig::default()
            },
            "memory_mib",
            "the memory of this host",
            memory.to_string(),
        ),
    ] {
        let backend = VmBackend {
            paths: humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
                "XDG_RUNTIME_DIR",
                runtime.path().to_str().unwrap(),
            )])),
            config,
            resolved: resolved.clone(),
        };
        let err = backend
            .plan(&PlanRequest::new(&profile, &session, &options))
            .expect_err("over the host");
        let key = format!("{CONFIG_GROUP}.{key}");
        assert_eq!(err.code.as_str(), "CONFIG_003", "{}: {}", err.code, err.why);
        assert!(err.why.contains(&key), "{}", err.why);
        assert!(err.why.contains(what), "{}", err.why);
        assert_eq!(err.fix, Some(FixAction::ChangeSetting { key, value: max }));
    }
}

/// Werte von `sandbox.qemu` über diesem Rechner machen das VM-Backend in der
/// Liste der Backends unverfügbar, mit dem Befund, mit dem sein Plan sie
/// ablehnte (`CONFIG_003`, HUM-501, HUM-502); abgeleitete Werte nie.
/// Mutationsbeweis: ohne die Prüfung in der Liste fehlt der Befund.
#[test]
fn kernel_level_values_beyond_this_host_are_named_in_the_list() {
    let tiny = HostResources {
        memory_mib: Some(1024),
        cpus: Some(1),
    };
    let mut sandbox = SandboxRef::default();
    let list = crate::backend_list::entries(&sandbox, "auto", tiny, |_| None);
    let kernel = |list: &[crate::BackendEntry]| {
        list.iter()
            .find(|entry| entry.name == crate::VM_BACKEND)
            .cloned()
            .expect("the kernel level is listed")
    };
    assert!(kernel(&list).available(), "{:?}", kernel(&list));

    sandbox.qemu.memory_mib = Some(2048);
    let list = crate::backend_list::entries(&sandbox, "auto", tiny, |_| None);
    let entry = kernel(&list);
    assert_eq!(
        entry
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        ["CONFIG_003"],
        "{entry:?}"
    );
    assert!(
        entry.diagnostics[0]
            .why
            .contains("sandbox.qemu.memory_mib = 2048"),
        "{:?}",
        entry.diagnostics[0]
    );
}

/// HUM-506: Der Abdruck der Eingaben des Probestarts ändert sich mit dem
/// Gastkernel, seiner Konfiguration, dem Modulverzeichnis und jedem
/// Programm der Kernel-Stufe, obwohl das Backend keine Fassung nennt. Ein
/// gespeicherter Probestart überlebt so keine dieser Änderungen.
///
/// Mutationsbeweis: ohne den Abdruck der Dateien in
/// `doctor::backends::probe_inputs` bleiben das neu geschriebene Kernel-
/// Abbild, die geänderte Konfiguration und das ersetzte Programm unbemerkt;
/// ohne die Einstellungen bleibt der andere Speicher unbemerkt; ohne
/// `MODULE_LISTS` in `prerequisites::search_files` das neu geschriebene
/// `modules.builtin`.
#[test]
fn the_probe_inputs_follow_kernel_modules_and_programs() {
    use crate::doctor::backends::probe_inputs;

    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let emulator = bin.join("emulator-hum506");
    std::fs::write(&emulator, b"#!/bin/sh\n").unwrap();
    let kernel = dir.path().join("vmlinuz-a");
    std::fs::write(&kernel, b"kernel a").unwrap();
    let other_kernel = dir.path().join("vmlinuz-b");
    std::fs::write(&other_kernel, b"kernel b").unwrap();
    let kernel_config = dir.path().join("config-a");
    std::fs::write(&kernel_config, b"CONFIG_VIRTIO_FS=y\n").unwrap();
    let modules = dir.path().join("modules");
    std::fs::create_dir_all(&modules).unwrap();
    let builtin = modules.join("modules.builtin");
    std::fs::write(&builtin, b"kernel/fs/fuse/virtiofs.ko\n").unwrap();
    let paths = Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", dir.path().to_string_lossy().into_owned()),
        ("PATH", bin.to_string_lossy().into_owned()),
    ]));
    let sandbox = |kernel: &Path| SandboxRef {
        qemu: QemuConfig {
            qemu: Some(emulator.clone()),
            kernel: Some(kernel.to_path_buf()),
            modules: Some(modules.clone()),
            ..QemuConfig::default()
        },
        ..SandboxRef::default()
    };
    let inputs = |sandbox: &SandboxRef| probe_inputs(crate::VM_BACKEND, sandbox, &paths);
    let first = inputs(&sandbox(&kernel));
    assert_eq!(first, inputs(&sandbox(&kernel)), "nothing changed");
    assert_eq!(first.len(), 64, "{first}");

    // Ein anderes Kernel-Abbild.
    assert_ne!(first, inputs(&sandbox(&other_kernel)), "another kernel");
    // Dasselbe Abbild, neu geschrieben, wie nach einem Update.
    std::fs::write(&kernel, b"kernel a, rebuilt").unwrap();
    let rebuilt = inputs(&sandbox(&kernel));
    assert_ne!(first, rebuilt, "a rebuilt kernel");
    // Seine Konfiguration daneben (`config_beside`).
    assert!(
        prerequisites::search_files(paths.env(), &sandbox(&kernel).qemu).contains(&kernel_config)
    );
    std::fs::write(&kernel_config, b"CONFIG_VIRTIO_FS=m, changed\n").unwrap();
    assert_ne!(rebuilt, inputs(&sandbox(&kernel)), "another kernel config");
    let before_builtin = inputs(&sandbox(&kernel));
    // `modules.builtin` an Ort und Stelle neu geschrieben: dasselbe
    // Verzeichnis, derselbe Inode, anderer Inhalt.
    let ino = std::fs::metadata(&builtin).unwrap().ino();
    std::fs::write(
        &builtin,
        b"kernel/fs/fuse/virtiofs.ko\nkernel/fs/overlayfs/overlay.ko\n",
    )
    .unwrap();
    assert_eq!(std::fs::metadata(&builtin).unwrap().ino(), ino);
    let before_modules = inputs(&sandbox(&kernel));
    assert_ne!(
        before_builtin, before_modules,
        "modules.builtin rewritten in place"
    );
    // Ein anderes Modulverzeichnis.
    let mut moved = sandbox(&kernel);
    moved.qemu.modules = Some(dir.path().join("modules-b"));
    assert_ne!(before_modules, inputs(&moved), "another modules directory");
    // Ein ersetztes Programm: neue Datei, neuer Inode.
    let replacement = bin.join("emulator-hum506.new");
    std::fs::write(&replacement, b"#!/bin/sh\n# 10.1\n").unwrap();
    std::fs::rename(&replacement, &emulator).unwrap();
    let replaced = inputs(&sandbox(&kernel));
    assert_ne!(before_modules, replaced, "a replaced emulator");
    // Ein Programm, das neu im `PATH` auftaucht.
    std::fs::write(bin.join("virtiofsd"), b"#!/bin/sh\n").unwrap();
    let appeared = inputs(&sandbox(&kernel));
    assert_ne!(replaced, appeared, "a file daemon appeared in PATH");
    // Andere Ressourcen.
    let mut bigger = sandbox(&kernel);
    bigger.qemu.memory_mib = Some(4096);
    assert_ne!(appeared, inputs(&bigger), "other resources");
}

/// HUM-506 und HUM-501: Liegen die gesetzten Ressourcen der Kernel-Stufe
/// über diesem Rechner, gilt kein gespeicherter Probestart, auch einer mit
/// genau diesem Schlüssel nicht; der Plan lehnte jeden Start ab.
///
/// Mutationsbeweis: ohne die Prüfung gegen den Rechner in
/// `doctor::backends::cached_probe` fände der Doctor den Eintrag.
#[test]
fn a_cached_probe_does_not_count_with_resources_over_this_host() {
    use crate::doctor::backends::cached_probe;
    use crate::doctor::{ProbeCache, ProbeKey};

    let dir = tempfile::tempdir().unwrap();
    let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
    let key = ProbeKey {
        stage: crate::VM_BACKEND.to_owned(),
        program: "/usr/bin/emulator".to_owned(),
        tool_version: String::new(),
        inputs: "inputs".to_owned(),
        humanitl_version: "0.1.0".to_owned(),
        machine: "machine-a".to_owned(),
    };
    cache.store(&key, "passed").unwrap();
    let sandbox = SandboxRef {
        qemu: QemuConfig {
            memory_mib: Some(8192),
            cpus: Some(4),
            ..QemuConfig::default()
        },
        ..SandboxRef::default()
    };
    let large = HostResources {
        memory_mib: Some(16_384),
        cpus: Some(8),
    };
    assert_eq!(
        cached_probe(crate::VM_BACKEND, &sandbox, &key, &cache, large).as_deref(),
        Some("passed")
    );
    for small in [
        HostResources {
            memory_mib: Some(4096),
            cpus: Some(8),
        },
        HostResources {
            memory_mib: Some(16_384),
            cpus: Some(2),
        },
    ] {
        assert_eq!(
            cached_probe(crate::VM_BACKEND, &sandbox, &key, &cache, small),
            None,
            "{small:?}"
        );
    }
}

/// HUM-518: the kernel level calls the depot's guard first, and refuses a
/// depot mount it cannot carry yet with `AGENT_015` instead of starting
/// without it.
#[test]
fn the_kernel_level_guards_the_depot_and_refuses_its_mount() {
    use crate::agent::depot::{DepotMount, DepotSession, ReservedAgent};
    let work = tempfile::tempdir().unwrap();
    let runtime = tempfile::tempdir().unwrap();
    let profile = profile();
    let options = crate::launcher::LaunchOptions::default();
    let backend = VmBackend {
        paths: humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_RUNTIME_DIR",
            runtime.path().to_str().unwrap(),
        )])),
        config: QemuConfig::default(),
        resolved: Resolved {
            emulator: PathBuf::from("/nonexistent-hum518/emulator"),
            file_daemon: PathBuf::from("/nonexistent-hum518/files"),
            vsock_device: PathBuf::from("/nonexistent-hum518/vsock"),
            bubblewrap: PathBuf::from("/nonexistent-hum518/bubblewrap"),
            kernel: PathBuf::from("/nonexistent-hum518/kernel"),
            modules: PathBuf::from("/nonexistent-hum518/modules"),
            no_announce_submounts: true,
        },
    };
    let reserved = vec![ReservedAgent {
        agent: "claude".to_owned(),
        program: "claude".to_owned(),
        host_installs: Vec::new(),
        locked_by: None,
    }];
    let mut session = session(work.path());
    session.command = vec![OsString::from("/usr/local/bin/claude")];
    session.agent_depot = DepotSession {
        reserved: reserved.clone(),
        ..DepotSession::default()
    };
    let found = backend
        .plan(&PlanRequest::new(&profile, &session, &options))
        .unwrap_err();
    assert_eq!(found.code.as_str(), "AGENT_017", "{}", found.why);

    session.command = vec![OsString::from("bash")];
    session.agent_depot = DepotSession {
        reserved,
        mount: Some(DepotMount {
            agent: "claude".to_owned(),
            version: "1.0.0".to_owned(),
            program_dir: PathBuf::from("/nonexistent-hum518/program/1.0.0"),
            program: "claude".to_owned(),
            scratch: Some(PathBuf::from("/nonexistent-hum518/scratch")),
            env: Vec::new(),
        }),
        ..DepotSession::default()
    };
    let found = backend
        .plan(&PlanRequest::new(&profile, &session, &options))
        .unwrap_err();
    assert_eq!(found.code.as_str(), "AGENT_015", "{}", found.why);
}

/// HUM-495: Die Zeilen des Doctors tragen die Kennungen des Doctors, auch im
/// Verweis einer Zeile ohne Messung: Fehlt das Kernel-Abbild (hier: ein
/// gesetzter Pfad ohne Datei), wartet `virtio_fs` auf `kernel`
/// ([`BackendCheckId::KernelImage`]). Mutationsbeweis: `check_id` bildet
/// `Prerequisite::Kernel` auf eine andere Kennung ab, und der Test wird rot.
#[test]
fn without_a_kernel_image_the_doctor_line_virtio_fs_waits_for_the_line_kernel() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", dir.path().to_string_lossy().into_owned()),
        (
            "PATH",
            dir.path().join("bin").to_string_lossy().into_owned(),
        ),
    ]));
    let sandbox = SandboxRef {
        qemu: QemuConfig {
            kernel: Some(dir.path().join("vmlinuz-none")),
            ..QemuConfig::default()
        },
        ..SandboxRef::default()
    };
    let lines = static_lines(&paths, &sandbox);
    let line = |id: BackendCheckId| {
        lines
            .iter()
            .find(|(which, _)| *which == id)
            .map(|(_, line)| line.clone())
            .expect("a line")
    };
    assert!(
        matches!(
            line(BackendCheckId::KernelImage),
            StaticLine::Checked(Err(_))
        ),
        "{lines:?}"
    );
    assert_eq!(
        line(BackendCheckId::VirtioFs),
        StaticLine::Unchecked {
            after: BackendCheckId::KernelImage,
            needs: "the kernel image",
        }
    );
}
