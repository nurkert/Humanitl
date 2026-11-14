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
    let root = guest_root(&profile, &session, &present, &files, Path::new("/m"));

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
    let (agent, shim) = crate::shim_fds::split_env(&env);
    let text = guest_plan(&profile, &session, &agent, &shim, 4242).unwrap();
    let plan: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(plan["door_port"], "5000");
    assert_eq!(plan["report_port"], "5001");
    assert_eq!(plan["work_tag"], "work");
    assert_eq!(plan["pins"]["pins"][0]["ino"], "4242");
    assert_eq!(plan["socket_walk"], serde_json::json!(["/work", "/dev"]));
    assert_eq!(plan["command"], serde_json::json!(["/bin/bash", "-l"]));
    assert!(plan["shim"].get("HUMANITL_BRIDGES").is_none());
    assert!(plan["env"].get("HUMANITL_SESSION").is_some());
    let mounts = plan["pins"]["mounts"].as_array().unwrap();
    let virtiofs = mounts.iter().filter(|m| m["type"] == "virtiofs").count();
    assert_eq!(virtiofs, 2, "only / and /work, no announced submount");
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
