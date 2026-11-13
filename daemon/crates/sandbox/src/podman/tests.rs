//! Tests des Podman-Adapters (HUM-433).
//!
//! Zwei Arten: Die oberen prüfen Einstellungen, Konfiguration, Plan und
//! Vorschau ohne Podman und laufen überall. Die in [`live`] starten echte
//! Container und brauchen rootless Podman 4.9 oder neuer mit `crun`. Ohne
//! Podman überspringen sie mit Grund auf stderr (`skipped: ...`); mit
//! `HUMANITL_REQUIRE_PODMAN=1` (CI-Job von HUM-435) scheitern sie stattdessen
//! mit dem Befund der Suche.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use humanitl_config::{PodmanConfig, PodmanRuntime};

use super::*;

#[test]
fn empty_limits_are_derived_from_the_host_within_their_bounds() {
    let config = PodmanConfig::default();
    let small = Settings::with_host(&config, Some(1024), Some(1));
    assert_eq!((small.memory_mib, small.cpus), (512, 1));
    let large = Settings::with_host(&config, Some(64 * 1024), Some(32));
    assert_eq!((large.memory_mib, large.cpus), (4096, 4));
    let unknown = Settings::with_host(&config, None, None);
    assert_eq!((unknown.memory_mib, unknown.cpus), (FALLBACK_MEMORY_MIB, 1));
    let set = PodmanConfig {
        memory_mib: Some(300),
        cpus: Some(3),
        pids_limit: 128,
        start_timeout_secs: 7,
        ..PodmanConfig::default()
    };
    let settings = Settings::with_host(&set, Some(64 * 1024), Some(32));
    assert_eq!(
        (settings.memory_mib, settings.cpus, settings.pids),
        (300, 3, 128)
    );
    assert_eq!(settings.start_timeout, Duration::from_secs(7));
    assert_eq!(
        settings.limits(),
        ResourceLimits {
            memory_bytes: Some(300 * 1024 * 1024),
            cpu_millis: Some(3000),
            pids: Some(128),
        }
    );
}

/// Die `containers.conf` des Daemons schneidet ab, was die des Nutzers
/// hinzufügen könnte: Mounts aus `mounts.conf`, Hooks, ein anderes
/// seccomp-Profil, Proxy-Variablen, Volumes, Geräte, Netz.
#[test]
fn the_daemons_containers_conf_cuts_off_the_users_sources() {
    let paths = Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", "/home/u"),
        ("XDG_RUNTIME_DIR", "/run/user/1000"),
    ]));
    let engine = EngineDir::of(&paths);
    let text = engine.conf_text(&[("HUMANITL_REPORT_FD".to_owned(), "3".to_owned())]);
    let doc: toml::Table = toml::from_str(&text).expect("valid TOML");
    let containers = doc["containers"].as_table().expect("[containers]");
    assert_eq!(
        containers["default_mounts_file"].as_str(),
        Some("/run/user/1000/humanitl/podman/mounts.conf")
    );
    assert_eq!(
        containers["seccomp_profile"].as_str(),
        Some("/run/user/1000/humanitl/podman/seccomp.json")
    );
    for key in ["http_proxy", "env_host"] {
        assert_eq!(containers[key].as_bool(), Some(false), "{key}");
    }
    for key in ["volumes", "devices", "default_capabilities"] {
        assert_eq!(containers[key].as_array().map(Vec::len), Some(0), "{key}");
    }
    assert_eq!(containers["netns"].as_str(), Some("none"));
    let env: Vec<&str> = containers["env"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(env.contains(&"HUMANITL_REPORT_FD=3"), "{env:?}");
    let engine_table = doc["engine"].as_table().expect("[engine]");
    assert_eq!(
        engine_table["hooks_dir"].as_array().unwrap()[0].as_str(),
        Some("/run/user/1000/humanitl/podman/hooks.d")
    );
    assert_eq!(engine_table["runtime"].as_str(), Some("crun"));
}

/// Podman startet mit genau sechs Variablen; keine des Daemons, auch kein
/// `CONTAINERS_CONF_OVERRIDE`, `CONTAINER_HOST` oder Proxy, kommt durch.
#[test]
fn the_engine_starts_with_six_variables_and_nothing_of_the_daemon() {
    let paths = Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", "/home/u"),
        ("PATH", "/usr/bin:/bin"),
        ("CONTAINERS_CONF_OVERRIDE", "/home/u/override.conf"),
        ("CONTAINER_HOST", "unix:///tmp/elsewhere.sock"),
        ("HTTPS_PROXY", "http://user:pw@proxy.invalid:3128"),
    ]));
    let engine = EngineDir::of(&paths);
    let command = engine.command(Path::new("/usr/bin/podman"), &engine.containers_conf());
    let names: Vec<String> = command
        .get_envs()
        .filter(|(_, value)| value.is_some())
        .map(|(key, _)| key.to_string_lossy().into_owned())
        // Der Sitzungsbus des Nutzers kommt mit, wenn der Prozess einen hat,
        // und das Laufzeitverzeichnis des Nutzers, wenn es eines gibt (unter
        // macOS keines); beide prüft ein eigener Test.
        .filter(|key| key != "DBUS_SESSION_BUS_ADDRESS" && key != "XDG_RUNTIME_DIR")
        .collect();
    assert_eq!(
        names.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        [
            "CONTAINERS_CONF",
            "CONTAINERS_REGISTRIES_CONF",
            "CONTAINERS_STORAGE_CONF",
            "HOME",
            "PATH",
        ]
        .into_iter()
        .collect()
    );
    let debug = format!("{command:?}");
    assert!(
        !debug.contains("user:pw") && !debug.contains("override.conf"),
        "{debug}"
    );
}

/// Podman bekommt das Laufzeitverzeichnis des Nutzers, nicht das von
/// Humanitl: `/run/user/<uid>`, wenn es existiert, sonst das des Prozesses.
#[test]
fn the_engine_gets_the_users_runtime_directory_never_humanitls() {
    let only = |owned: &'static [&'static str]| {
        move |path: &Path| owned.iter().any(|dir| path == Path::new(dir))
    };
    // `/run/user/<uid>` des Nutzers geht vor.
    let both = only(&["/run/user/1000", "/run/xdg-1000"]);
    assert_eq!(
        user_runtime_dir(1000, Some("/run/xdg-1000"), &both).as_deref(),
        Some(Path::new("/run/user/1000"))
    );
    // Gehört es nicht dem Nutzer (oder fehlt es), zählt `XDG_RUNTIME_DIR`
    // des Prozesses, wenn es ihm gehört.
    let xdg_only = only(&["/run/xdg-1000"]);
    assert_eq!(
        user_runtime_dir(1000, Some("/run/xdg-1000"), &xdg_only).as_deref(),
        Some(Path::new("/run/xdg-1000"))
    );
    // Sonst keines: kein fehlendes, kein fremdes, kein relatives.
    let none = only(&[]);
    assert_eq!(user_runtime_dir(1000, Some("/run/xdg-1000"), &none), None);
    assert_eq!(user_runtime_dir(1000, None, &none), None);
    assert_eq!(user_runtime_dir(1000, Some(""), &xdg_only), None);
    let relative = only(&["run/xdg"]);
    assert_eq!(user_runtime_dir(1000, Some("run/xdg"), &relative), None);
    // Ohne Verzeichnis lehnt die Vorbereitung ab, mit Befund und Fix.
    let err = no_runtime_dir(1000);
    assert_eq!(err.code.as_str(), "SANDBOX_037", "{}", err.why);
    assert!(err.why.contains("/run/user/1000"), "{}", err.why);
    assert!(matches!(err.fix, Some(FixAction::CopyCommand(_))));
    // Ein Verzeichnis, das einem anderen gehört, zählt nicht.
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(owned_dir_of(rustix::process::getuid().as_raw(), dir.path()));
    assert!(!owned_dir_of(
        rustix::process::getuid().as_raw().wrapping_add(1),
        dir.path()
    ));
    assert!(!owned_dir_of(
        rustix::process::getuid().as_raw(),
        &dir.path().join("missing")
    ));
    // Ein eigenes Laufzeitverzeichnis von Humanitl (ein Test) erreicht
    // Podman nicht; seine Dateien liegen trotzdem dort.
    let paths = Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", "/home/u"),
        ("XDG_RUNTIME_DIR", "/tmp/humanitl-test"),
    ]));
    let engine = EngineDir::of(&paths);
    assert!(engine.root.starts_with("/tmp/humanitl-test"));
    let env = engine.env(&engine.containers_conf());
    let xdg = env
        .iter()
        .find(|(key, _)| key == "XDG_RUNTIME_DIR")
        .map(|(_, value)| value.as_str());
    assert_ne!(xdg, Some("/tmp/humanitl-test"));
    // Ohne Wurzeln im `storage.conf` gelten die Vorgaben des Nutzers.
    let storage = EngineDir::storage_text();
    assert!(
        !storage.contains("graphroot") && !storage.contains("runroot"),
        "{storage}"
    );
}

#[test]
fn the_instance_label_differs_per_runtime_directory() {
    let a = instance_of(Path::new("/run/user/1000/humanitl"));
    let b = instance_of(Path::new("/tmp/test/humanitl"));
    assert_eq!(a.len(), 16);
    assert_ne!(a, b);
    assert_eq!(a, instance_of(Path::new("/run/user/1000/humanitl")));
}

/// HUM-438: Unter Podman gibt es keine Wegwerf-Sandbox, und der Grund steht
/// im Befund. `disposable` endet mit `SANDBOX_030` und fällt nie zurück;
/// `auto` läuft direkt mit `SANDBOX_033`, das denselben Grund nennt.
///
/// Mutationsbeweis: `disposable_support` gibt `Ok(())` zurück, und dieser
/// Test ist rot.
#[test]
fn a_disposable_sandbox_is_refused_with_its_reason_and_auto_runs_direct() {
    use humanitl_config::{SandboxMode, WorkMode};

    let paths = Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", "/home/u"),
        ("XDG_RUNTIME_DIR", "/run/user/1000"),
    ]));
    let backend = PodmanBackend::fallback(paths, &PodmanConfig::default());
    let profile = SandboxProfile::parse(
        include_str!("../../../../../profiles/sandbox/default.toml"),
        Path::new("default.toml"),
    )
    .expect("the bundled profile");
    let session = SessionContext {
        session: SessionId::nil(),
        work_src: "/home/u/proj".into(),
        work_mode: WorkMode::Rw,
        proxy_socket_src: "/run/user/1000/humanitl/proxy/proxy.sock".into(),
        ca_cert_src: "/home/u/ca.crt".into(),
        ca_bundle_src: "/home/u/ca-bundle.crt".into(),
        shim_src: "/usr/lib/humanitl/humanitl-shim".into(),
        session_env: Vec::new(),
        command: vec!["true".into()],
        files: Vec::new(),
        agent_program: None,
    };
    let options = crate::launcher::LaunchOptions {
        disposable: Some(crate::launcher::DisposableLayer::Overlay(
            crate::disposable::OverlayDirs {
                upper: "/run/user/1000/humanitl/disposable/l/upper".into(),
                work: "/run/user/1000/humanitl/disposable/l/work".into(),
            },
        )),
        ..crate::launcher::LaunchOptions::default()
    };
    let request = PlanRequest::new(&profile, &session, &options);
    let support = || backend.disposable_support(&request);
    let err = crate::disposable::resolve_mode(SandboxMode::Disposable, support)
        .expect_err("disposable never falls back");
    assert_eq!(err.code.as_str(), "SANDBOX_030", "{}", err.why);
    assert!(err.why.contains("no disposable sandbox"), "{}", err.why);
    assert!(err.fix.is_some(), "a refusal names a way out");
    let auto = crate::disposable::resolve_mode(SandboxMode::Auto, support).expect("auto");
    assert_eq!(auto.mode, SandboxMode::Direct);
    let notice = auto.notice.expect("SANDBOX_033");
    assert_eq!(notice.code.as_str(), "SANDBOX_033");
    assert!(
        notice.why.contains("no disposable sandbox"),
        "{}",
        notice.why
    );
    assert!(notice.fix.is_some());
}

#[test]
fn krun_is_refused_with_sandbox_048_and_crun_passes() {
    let err = refuse_kernel_level(PodmanRuntime::Krun).expect_err("krun");
    assert_eq!(err.code.as_str(), "SANDBOX_048", "{}", err.why);
    refuse_kernel_level(PodmanRuntime::Crun).expect("crun");
}

/// Plan und Vorschau ohne Podman: Die Zeile entsteht ohne die Engine, und
/// was sie anlegt, liegt im Laufzeitverzeichnis des Tests.
#[cfg(target_os = "linux")]
mod planned {
    use std::ffi::OsString;
    use std::os::unix::net::UnixListener;

    use humanitl_config::{Env, WorkMode};

    use super::*;
    use crate::launcher::{LaunchOptions, StdioMode};

    pub(super) struct Fixture {
        pub(super) dir: tempfile::TempDir,
        pub(super) paths: Paths,
        pub(super) work: PathBuf,
        pub(super) socket: PathBuf,
        _listener: UnixListener,
        pub(super) ca: PathBuf,
        shim: PathBuf,
    }

    impl Fixture {
        pub(super) fn new(shim: Option<PathBuf>) -> Self {
            Self::with_env(shim, &[])
        }

        pub(super) fn with_env(shim: Option<PathBuf>, extra: &[(&str, &str)]) -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let state = dir.path().to_path_buf();
            let mut env = Env::from_process()
                .with("XDG_RUNTIME_DIR", state.join("runtime").to_string_lossy());
            for (key, value) in extra {
                env = env.with(*key, *value);
            }
            let paths = Paths::new(env);
            let work = state.join("work");
            std::fs::create_dir_all(work.join(".git/hooks")).expect("hooks");
            std::fs::write(work.join(".envrc"), "export CANARY=1\n").expect("envrc");
            let socket = paths.proxy_socket();
            std::fs::create_dir_all(socket.parent().unwrap()).expect("socket dir");
            std::fs::set_permissions(
                socket.parent().unwrap(),
                std::fs::Permissions::from_mode(0o700),
            )
            .expect("chmod");
            let listener = UnixListener::bind(&socket).expect("bind");
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
                .expect("chmod socket");
            // Der Shim liest seine Bridge über den Socket; jede Verbindung
            // wird angenommen und gehalten.
            let accepting = listener.try_clone().expect("clone listener");
            std::thread::spawn(move || {
                let mut held = Vec::new();
                for stream in accepting.incoming().flatten() {
                    held.push(stream);
                }
            });
            let ca = state.join("ca.crt");
            std::fs::write(&ca, b"-----BEGIN CERTIFICATE-----\n").expect("ca");
            let shim = shim.unwrap_or_else(|| {
                let fake = state.join("humanitl-shim");
                crate::test_support::write_program(&fake, "#!/bin/sh\nexit 0\n");
                fake
            });
            Self {
                dir,
                paths,
                work,
                socket,
                _listener: listener,
                ca,
                shim,
            }
        }

        pub(super) fn context(&self, command: &[&str]) -> SessionContext {
            SessionContext {
                session: SessionId::new(),
                work_src: self.work.clone(),
                work_mode: WorkMode::Rw,
                proxy_socket_src: self.socket.clone(),
                ca_cert_src: self.ca.clone(),
                ca_bundle_src: self.ca.clone(),
                shim_src: self.shim.clone(),
                session_env: vec![("HUMANITL_SESSION".to_owned(), "probe".to_owned())],
                command: command.iter().map(OsString::from).collect(),
                files: Vec::new(),
                agent_program: None,
            }
        }
    }

    pub(super) fn profile() -> SandboxProfile {
        SandboxProfile::parse(
            include_str!("../../../../../profiles/sandbox/default.toml"),
            Path::new("default.toml"),
        )
        .expect("the bundled profile")
    }

    fn unchecked(paths: &Paths) -> PodmanBackend {
        PodmanBackend::fallback(paths.clone(), &PodmanConfig::default())
    }

    fn strings(argv: &[OsString]) -> Vec<String> {
        argv.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Der Plan nennt die Deskriptoren fortlaufend ab 3 (Bericht,
    /// Lebensleine, Umgebung, Mounttabelle), trägt keinen Wert der Umgebung
    /// in der Zeile, gibt Podman nur die Pfade seiner Konfiguration mit und
    /// beendet über die Engine.
    #[test]
    fn the_plan_hands_over_three_descriptors_and_no_environment_value() {
        let fx = Fixture::new(None);
        let backend = unchecked(&fx.paths);
        let profile = profile();
        let mut session = fx.context(&["sh", "-c", "true"]);
        session
            .session_env
            .push(("HL_SECRET".to_owned(), "s3cr3t-value".to_owned()));
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let plan = backend
            .plan(&PlanRequest::new(&profile, &session, &options))
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        let argv = strings(&plan.argv);
        assert!(
            !argv.iter().any(|arg| arg.contains("s3cr3t-value")),
            "a value of the environment stands in argv: {argv:?}"
        );
        assert!(plan.env.iter().any(|(key, _)| key == "HL_SECRET"));
        assert!(plan.argv_env.is_empty());
        assert_eq!(
            plan.fds
                .iter()
                .map(|(_, target)| *target)
                .collect::<Vec<_>>(),
            [3, 4, 5, 6]
        );
        assert!(plan.renumbered);
        assert_eq!(plan.env_delivery, EnvDelivery::InheritedFd(5));
        assert!(
            plan.program_env
                .iter()
                .all(|(_, value)| !value.contains("s3cr3t-value"))
        );
        // `podman run` startet mit der `containers.conf` der Sitzung, und
        // nur sie nennt den Bericht unter 3 (Regression: mit der des Daemons
        // lief der Shim ohne Bericht, und kein `CHECK` kam an).
        let conf_of = |env: &[(String, String)]| {
            env.iter()
                .find(|(key, _)| key == "CONTAINERS_CONF")
                .map(|(_, value)| PathBuf::from(value))
                .expect("CONTAINERS_CONF")
        };
        let session_conf = conf_of(&plan.program_env);
        assert_eq!(
            session_conf,
            plan.session_dir.clone().unwrap().join("containers.conf")
        );
        let text = std::fs::read_to_string(&session_conf).unwrap();
        assert!(text.contains("HUMANITL_REPORT_FD=3"), "{text}");
        // Unter 3 erbt der Container die Schreibseite genau der Pipe, deren
        // Leseseite der Start liest.
        let report_host = plan
            .fds
            .iter()
            .find(|(_, target)| *target == 3)
            .map(|(host, _)| *host)
            .expect("target 3");
        assert_pins_at_six(&fx.work, &plan);
        let once = plan.take_once().expect("fresh plan");
        let inode = |path: String| {
            use std::os::unix::fs::MetadataExt as _;
            std::fs::metadata(path).unwrap().ino()
        };
        assert_eq!(
            inode(format!("/proc/self/fd/{report_host}")),
            inode(format!(
                "/proc/self/fd/{}",
                std::os::fd::AsRawFd::as_raw_fd(&once.report)
            )),
            "the descriptor at 3 is the report pipe the start reads"
        );
        drop(once);
        let Teardown::Container(stop) = &plan.teardown else {
            panic!("a container is torn down through the engine");
        };
        // Das Beenden nimmt die des Daemons; die der Sitzung ist dann
        // womöglich schon weg.
        assert_ne!(conf_of(&stop.env), session_conf);
        assert_eq!(stop.name, container_name(session.session));
        assert!(argv.contains(&format!("--name={}", stop.name)));
        // Was der Plan anlegt, liegt im Laufzeitverzeichnis und nirgends
        // sonst; die Masken gibt es nur über vorhandenen Pfaden.
        let dir = plan.session_dir.clone().expect("a session directory");
        assert!(dir.starts_with(fx.dir.path()));
        assert!(dir.join("rootfs/run/humanitl/proxy.sock").is_file());
        assert!(dir.join("rootfs/work").is_dir());
        assert_eq!(
            std::fs::read_link(dir.join("rootfs/bin")).unwrap(),
            Path::new("usr/bin")
        );
        let conf = std::fs::read_to_string(dir.join("containers.conf")).unwrap();
        assert!(conf.contains("HUMANITL_REPORT_FD=3"), "{conf}");
        assert!(!conf.contains("s3cr3t-value"), "{conf}");
        assert!(
            argv.iter()
                .any(|arg| arg.ends_with("destination=/work/.envrc,readonly=true"))
        );
        assert!(
            !argv
                .iter()
                .any(|arg| arg.contains("destination=/work/.env,"))
        );
        assert!(plan.unprotected.contains(&PathBuf::from(".env")));
        assert!(
            !fx.work.join(".env").exists() && !fx.work.join(".idea").exists(),
            "the plan wrote into the project"
        );
    }

    /// Unter 6 die Mounttabelle (HUM-437): `/work` mit Gerät und Inode des
    /// geprüften Ordners, die Wurzel der Sitzung, die eigenen Einträge von
    /// Podman.
    fn assert_pins_at_six(work: &Path, plan: &LaunchPlan) {
        use std::os::unix::fs::MetadataExt as _;

        let pins_host = plan
            .fds
            .iter()
            .find(|(_, target)| *target == 6)
            .map(|(host, _)| *host)
            .expect("target 6");
        let pins: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("/proc/self/fd/{pins_host}")).expect("pins"),
        )
        .expect("pins JSON");
        let meta = std::fs::metadata(work).unwrap();
        let (dev, ino) = (meta.dev().to_string(), meta.ino().to_string());
        assert!(
            pins["pins"]
                .as_array()
                .unwrap()
                .iter()
                .any(|pin| pin["path"] == "/work" && pin["dev"] == dev && pin["ino"] == ino),
            "{pins}"
        );
        let points: Vec<&str> = pins["mounts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry["point"].as_str())
            .collect();
        for point in ["/", "/work", "/proc", "/run/.containerenv", "/sys"] {
            assert!(points.contains(&point), "{point} missing: {points:?}");
        }
    }

    /// Die Vorschau ist die Zeile des Plans bis auf die Lage der Quellen
    /// (geschrieben statt aufgelöst) und die Masken auf fehlenden Pfaden.
    #[test]
    fn the_preview_is_the_plan_line_without_side_effects() {
        let fx = Fixture::new(None);
        let backend = unchecked(&fx.paths);
        let profile = profile();
        let session = fx.context(&["true"]);
        let options = LaunchOptions {
            stdio: StdioMode::Pty { cols: 80, rows: 24 },
            ..LaunchOptions::default()
        };
        let request = PlanRequest::new(&profile, &session, &options);
        let preview = backend.preview(&request);
        let engine = EngineDir::of(&fx.paths);
        assert!(
            !engine.root.exists(),
            "the preview created {}",
            engine.root.display()
        );
        assert!(preview.argv_env.is_empty());
        assert_eq!(preview.limits, backend.settings.limits());
        assert_eq!(
            preview.door.describe(),
            "socket_file /run/humanitl/proxy.sock"
        );
        let plan = backend
            .plan(&request)
            .unwrap_or_else(|d| panic!("{}", d.why));
        let canonical = std::fs::canonicalize(&fx.work).unwrap();
        let shown: Vec<String> = strings(&preview.argv)
            .into_iter()
            .map(|arg| {
                arg.replace(
                    &fx.work.display().to_string(),
                    &canonical.display().to_string(),
                )
            })
            .filter(|arg| !arg.contains("destination=/work/"))
            .collect();
        let planned: Vec<String> = strings(&plan.argv)
            .into_iter()
            .filter(|arg| !arg.contains("destination=/work/"))
            .collect();
        assert_eq!(shown, planned);
        assert!(
            planned.contains(&"--preserve-fds=6".to_owned()),
            "{planned:?}"
        );
    }

    #[test]
    fn disposable_an_unnamed_image_and_krun_are_refused_and_never_replaced() {
        let fx = Fixture::new(None);
        let profile = profile();
        let session = fx.context(&["true"]);
        let disposable = LaunchOptions {
            disposable: Some(crate::launcher::DisposableLayer::Overlay(
                crate::disposable::OverlayDirs {
                    upper: fx.dir.path().join("upper"),
                    work: fx.dir.path().join("w"),
                },
            )),
            ..LaunchOptions::default()
        };
        let backend = unchecked(&fx.paths);
        let err = backend
            .plan(&PlanRequest::new(&profile, &session, &disposable))
            .expect_err("disposable");
        assert_eq!(err.code.as_str(), "SANDBOX_030", "{}", err.why);
        let err = backend
            .disposable_support(&PlanRequest::new(&profile, &session, &disposable))
            .expect_err("not yet");
        assert_eq!(err.code.as_str(), "SANDBOX_030", "{}", err.why);
        // Ein Abbild als Wurzel ohne Verweis: nie still die Wurzel des Hosts
        // (HUM-439).
        let image = PodmanBackend::fallback(
            fx.paths.clone(),
            &PodmanConfig {
                rootfs: humanitl_config::PodmanRootfs::Image,
                ..PodmanConfig::default()
            },
        );
        let options = LaunchOptions::default();
        let err = image
            .plan(&PlanRequest::new(&profile, &session, &options))
            .expect_err("image");
        assert_eq!(err.code.as_str(), "CONFIG_003", "{}", err.why);
        let krun = PodmanBackend::fallback(
            fx.paths.clone(),
            &PodmanConfig {
                runtime: PodmanRuntime::Krun,
                ..PodmanConfig::default()
            },
        );
        let err = krun
            .plan(&PlanRequest::new(&profile, &session, &options))
            .expect_err("krun");
        assert_eq!(err.code.as_str(), "SANDBOX_048", "{}", err.why);
        assert_eq!(krun.isolation(), IsolationLevel::Kernel);
        assert_eq!(backend.isolation(), IsolationLevel::Namespaces);
    }
}

/// Ein Abbild als Wurzel (HUM-439) mit einer Attrappe der Engine: Sie
/// antwortet auf `image inspect` mit einer gespeicherten Antwort und schreibt
/// jeden Aufruf mit; `pull` legt die Antwort erst an.
#[cfg(target_os = "linux")]
mod image_root {
    use std::ffi::OsString;

    use super::planned::{Fixture, profile};
    use super::*;
    use crate::launcher::{LaunchOptions, StdioMode};

    const PINNED: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    const OTHER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    fn reference() -> String {
        format!("localhost/humanitl-hum439@sha256:{PINNED}")
    }

    fn answer(digest: &str) -> String {
        serde_json::json!([{
            "Id": "f00d",
            "Digest": format!("sha256:{digest}"),
            "RepoDigests": [format!("localhost/humanitl-hum439@sha256:{digest}")],
            "Config": { "Env": ["PATH=/usr/bin:/bin"] },
        }])
        .to_string()
    }

    struct FakeEngine {
        program: PathBuf,
        log: PathBuf,
        json: PathBuf,
        staged: PathBuf,
    }

    impl FakeEngine {
        /// `present`: die Antwort, die schon im Speicher liegt; `None`: kein
        /// Abbild, bis ein Ziehen die Antwort aus `staged` an ihren Ort legt.
        fn new(dir: &Path, present: Option<&str>, staged: &str) -> Self {
            let program = dir.join("fake-engine");
            let log = dir.join("engine.log");
            let json = dir.join("inspect.json");
            let staged_file = dir.join("staged.json");
            if let Some(present) = present {
                std::fs::write(&json, present).unwrap();
            }
            std::fs::write(&staged_file, staged).unwrap();
            let script = format!(
                "#!/bin/sh\n\
                 printf '%s\\n' \"$*\" >> '{log}'\n\
                 case \" $* \" in\n\
                 *' pull '*) cp '{staged}' '{json}'; echo f00d; exit 0 ;;\n\
                 *' image inspect '*)\n\
                   if [ -f '{json}' ]; then cat '{json}'; exit 0; fi\n\
                   echo 'Error: humanitl-hum439: image not known' >&2; exit 125 ;;\n\
                 esac\n\
                 exit 1\n",
                log = log.display(),
                staged = staged_file.display(),
                json = json.display(),
            );
            crate::test_support::write_program(&program, &script);
            Self {
                program,
                log,
                json,
                staged: staged_file,
            }
        }

        fn calls(&self) -> Vec<String> {
            std::fs::read_to_string(&self.log)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        }

        fn backend(&self, paths: &Paths) -> PodmanBackend {
            PodmanBackend::fallback(
                paths.clone(),
                &PodmanConfig {
                    program: Some(self.program.clone()),
                    rootfs: humanitl_config::PodmanRootfs::Image,
                    image: Some(reference()),
                    ..PodmanConfig::default()
                },
            )
        }
    }

    fn strings(argv: &[OsString]) -> Vec<String> {
        argv.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Fehlt das Abbild oder liegt unter dem Namen ein anderer Digest, lehnt
    /// der Plan mit `SANDBOX_043` ab, bevor er etwas schreibt, und nennt den
    /// Befehl, der zieht; gezogen wird nichts (Akzeptanzkriterium 1). Ohne
    /// die Prüfung des Digests würde dieser Test rot: Der Plan ginge durch.
    #[test]
    fn a_missing_or_other_image_is_refused_with_sandbox_043_and_nothing_is_pulled() {
        let fx = Fixture::new(None);
        let profile = profile();
        let session = fx.context(&["true"]);
        let options = LaunchOptions::default();
        for present in [None, Some(answer(OTHER))] {
            let dir = tempfile::tempdir().unwrap();
            let engine = FakeEngine::new(dir.path(), present.as_deref(), &answer(PINNED));
            let err = engine
                .backend(&fx.paths)
                .plan(&PlanRequest::new(&profile, &session, &options))
                .expect_err("not the pinned image");
            assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
            assert_eq!(
                err.fix,
                Some(FixAction::CopyCommand(format!(
                    "humanitl sandbox image pull {}",
                    reference()
                )))
            );
            if present.is_some() {
                assert!(err.why.contains(OTHER), "{}", err.why);
            }
            let calls = engine.calls();
            assert!(
                calls
                    .iter()
                    .any(|call| call.contains(&format!("image inspect -- {}", reference()))),
                "{calls:?}"
            );
            assert!(
                !calls
                    .iter()
                    .any(|call| call.contains("pull") || call.contains(" run ")),
                "a start pulled or ran: {calls:?}"
            );
            let sessions = EngineDir::of(&fx.paths).root.join("sessions");
            assert!(
                std::fs::read_dir(&sessions).map_or(true, |mut dir| dir.next().is_none()),
                "the refused plan wrote a session"
            );
        }
    }

    /// Mit dem Abbild unter genau seinem Digest: Die Zeile startet das
    /// Abbild samt Digest, `mounts.ro` und die Verweise des Profils fehlen,
    /// Projekt, Tür, CA, Shim und Dateien der Sitzung kommen vom Host, die
    /// erwartete Tabelle hat als Wurzel ein nur lesbares `overlay`, und die
    /// Vorschau nennt die Wurzel ein Abbild mit Digest.
    #[test]
    fn the_pinned_image_is_the_root_and_only_the_session_comes_from_the_host() {
        let fx = Fixture::new(None);
        let dir = tempfile::tempdir().unwrap();
        let engine = FakeEngine::new(dir.path(), Some(&answer(PINNED)), &answer(PINNED));
        let backend = engine.backend(&fx.paths);
        let profile = profile();
        assert!(!profile.mounts.ro.is_empty() && !profile.mounts.symlinks.is_empty());
        let session = fx.context(&["true"]);
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let request = PlanRequest::new(&profile, &session, &options);
        let plan = backend
            .plan(&request)
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        let argv = strings(&plan.argv);
        let at = argv
            .iter()
            .position(|arg| *arg == reference())
            .expect("the image with its digest");
        assert_eq!(argv[at + 1], "--proxy-port", "{argv:?}");
        assert!(!argv.contains(&"--rootfs".to_owned()), "{argv:?}");
        assert!(argv.contains(&"--pull=never".to_owned()));
        for ro in &profile.mounts.ro {
            let bind = format!("destination={},", ro.display());
            assert!(
                !argv.iter().any(|arg| arg.contains(&bind)),
                "{} is bound under an image: {argv:?}",
                ro.display()
            );
        }
        for dst in [
            "/work",
            "/run/humanitl/proxy.sock",
            "/etc/humanitl/ca.crt",
            "/run/humanitl/humanitl-shim",
            "/etc/passwd",
        ] {
            let bound = |arg: &String| {
                arg.starts_with("--mount=type=bind,")
                    && (arg.contains(&format!("destination={dst},"))
                        || arg.ends_with(&format!("destination={dst}")))
            };
            assert!(argv.iter().any(bound), "{dst}: {argv:?}");
        }
        let session_dir = plan.session_dir.clone().expect("a session directory");
        let rootfs = session_dir.join("rootfs");
        assert!(
            std::fs::read_dir(&rootfs).map_or(true, |mut dir| dir.next().is_none()),
            "an image needs no skeleton on the host"
        );
        let pins_host = plan
            .fds
            .iter()
            .find(|(_, target)| *target == 6)
            .map(|(host, _)| *host)
            .expect("target 6");
        let pins: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("/proc/self/fd/{pins_host}")).unwrap(),
        )
        .unwrap();
        let mounts = pins["mounts"].as_array().unwrap();
        let root: Vec<&serde_json::Value> = mounts
            .iter()
            .filter(|entry| entry["point"] == "/")
            .collect();
        assert_eq!(root.len(), 1, "{pins}");
        assert_eq!(root[0]["type"], "overlay");
        assert_eq!(root[0]["mode"], "ro");
        assert_eq!(root[0]["count"], "1");
        for ro in &profile.mounts.ro {
            assert!(
                !mounts
                    .iter()
                    .any(|entry| entry["point"] == ro.to_string_lossy().as_ref()),
                "{} expected under an image: {pins}",
                ro.display()
            );
        }
        assert!(
            mounts
                .iter()
                .any(|entry| entry["point"] == "/etc/humanitl/ca.crt"
                    && entry["source"]
                        .as_str()
                        .is_some_and(|source| source.starts_with("inode:"))),
            "the CA is pinned to the host file: {pins}"
        );
        let _ = std::fs::remove_dir_all(&session_dir);
    }

    /// Die Vorschau nennt die Wurzel ein Abbild mit Digest, zeigt den Verweis
    /// in der Zeile, keine Verweise des Profils, und zieht nichts.
    #[test]
    fn the_preview_names_the_image_root_with_its_digest() {
        let fx = Fixture::new(None);
        let dir = tempfile::tempdir().unwrap();
        let engine = FakeEngine::new(dir.path(), Some(&answer(PINNED)), &answer(PINNED));
        let backend = engine.backend(&fx.paths);
        let profile = profile();
        let session = fx.context(&["true"]);
        let options = LaunchOptions::default();
        let request = PlanRequest::new(&profile, &session, &options);
        let preview = backend.preview(&request);
        let first = preview.policy.first().expect("a row");
        assert_eq!(first.mode, PolicyMode::Image);
        assert_eq!(first.target, Path::new("/"));
        assert_eq!(first.source.as_deref(), Some(Path::new(&reference())));
        assert!(
            !preview
                .policy
                .iter()
                .any(|row| row.mode == PolicyMode::Symlink)
        );
        assert!(
            strings(&preview.argv).contains(&reference()),
            "the preview shows the digest"
        );
        assert!(
            !engine.calls().iter().any(|call| call.contains("pull")),
            "{:?}",
            engine.calls()
        );
    }

    /// `pull_image` zieht genau den Verweis mit der Signaturregel des
    /// Adapters und prüft danach den Digest; ein Verweis ohne Registry oder
    /// ohne Digest erreicht die Engine nie.
    #[test]
    fn a_pull_takes_the_pinned_reference_and_checks_it_afterwards() {
        let fx = Fixture::new(None);
        let dir = tempfile::tempdir().unwrap();
        let engine = FakeEngine::new(dir.path(), None, &answer(PINNED));
        let backend = engine.backend(&fx.paths);
        for bad in [
            format!("humanitl-hum439@sha256:{PINNED}"),
            "localhost/humanitl-hum439:latest".to_owned(),
        ] {
            let err = backend.pull_image(&bad).expect_err("refused");
            assert_eq!(err.code.as_str(), "CONFIG_003", "{}", err.why);
        }
        assert!(engine.calls().is_empty(), "{:?}", engine.calls());

        let pulled = backend.pull_image("").expect("the configured image");
        assert_eq!(pulled.reference, reference());
        assert_eq!(pulled.digest, format!("sha256:{PINNED}"));
        assert_eq!(pulled.id, "f00d");
        let calls = engine.calls();
        let policy = EngineDir::of(&fx.paths).signature_policy;
        let expected = format!(
            "--runtime=crun pull --quiet --signature-policy={} -- {}",
            policy.display(),
            reference()
        );
        assert!(calls.contains(&expected), "{calls:?}");
        assert!(
            std::fs::read_to_string(&policy)
                .unwrap()
                .contains("insecureAcceptAnything")
        );

        // Liegt danach ein anderer Digest vor, ist das Ziehen gescheitert.
        std::fs::remove_file(&engine.json).unwrap();
        std::fs::write(&engine.staged, answer(OTHER)).unwrap();
        let err = backend
            .pull_image(&reference())
            .expect_err("another digest");
        assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
    }
}

/// Echte Container. Siehe Modulbeschreibung: übersprungen ohne Podman, mit
/// `HUMANITL_REQUIRE_PODMAN=1` ein Fehlschlag mit Grund.
#[cfg(target_os = "linux")]
mod live {
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::Instant;

    use super::planned::{Fixture, profile};
    use super::*;
    use crate::handle::{KILL_GRACE, OutputChunk};
    use crate::launcher::{CheckResult, LaunchOptions, LaunchPlan, StdioMode};

    /// Setzt der Test, der den Daemon spielt, für sein Kind.
    const STAND_IN_ENV: &str = "HUMANITL_TEST_HUM433_STAND_IN";

    /// Ein Proxy mit Zugangsdaten in der Umgebung des Daemons.
    const PROXY_WITH_CREDENTIALS: &str = "http://user:pw@proxy.invalid:3128";

    /// Der gebaute Shim neben dem Testprogramm.
    fn real_shim() -> PathBuf {
        let mut dir = std::env::current_exe().expect("test binary");
        dir.pop();
        if dir.ends_with("deps") {
            dir.pop();
        }
        let shim = dir.join("humanitl-shim");
        assert!(
            shim.is_file(),
            "{} is missing; build the workspace first (cargo build --workspace)",
            shim.display()
        );
        shim
    }

    /// Setzt die CI (HUM-435): Ohne Podman scheitern die Tests dann, statt
    /// übersprungen zu werden.
    const REQUIRE_ENV: &str = "HUMANITL_REQUIRE_PODMAN";

    /// Ob es hier rootless Podman mit `crun` gibt. Ohne Podman überspringt
    /// der Test mit Grund auf stderr, mit `HUMANITL_REQUIRE_PODMAN=1`
    /// scheitert er.
    fn podman_here(test: &str) -> bool {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = Paths::new(
            humanitl_config::Env::from_process()
                .with("XDG_RUNTIME_DIR", dir.path().to_string_lossy()),
        );
        match PodmanBackend::detect(paths, &PodmanConfig::default()) {
            Ok(_) => true,
            Err(d) if std::env::var_os(REQUIRE_ENV).is_some_and(|value| value == "1") => {
                panic!(
                    "{test} needs rootless Podman 4.9+ with crun, and {REQUIRE_ENV}=1 requires \
                     it; {}: {}",
                    d.code, d.why
                )
            }
            Err(d) => {
                eprintln!(
                    "skipped: {test} needs rootless Podman 4.9+ with crun ({}: {}); set \
                     {REQUIRE_ENV}=1 to make this a failure",
                    d.code, d.why
                );
                false
            }
        }
    }

    /// Das echte Podman, oder ein Fehlschlag, der sagt, was fehlt.
    fn real(paths: &Paths) -> PodmanBackend {
        PodmanBackend::detect(paths.clone(), &PodmanConfig::default()).unwrap_or_else(|d| {
            panic!(
                "this test needs rootless Podman 4.9+ with crun (HUM-433); {}: {}",
                d.code, d.why
            )
        })
    }

    /// Die Container dieser Instanz, wie Podman sie sieht.
    fn containers(paths: &Paths, backend: &PodmanBackend) -> Vec<String> {
        listed(paths, backend, true)
    }

    /// Nur die laufenden Container; `containers` nennt auch beendete, die
    /// noch nicht entfernt sind.
    fn running(paths: &Paths, backend: &PodmanBackend) -> Vec<String> {
        listed(paths, backend, false)
    }

    fn listed(paths: &Paths, backend: &PodmanBackend, all: bool) -> Vec<String> {
        let engine = EngineDir::of(paths);
        let mut ps = engine.command(backend.program(), &engine.containers_conf());
        ps.arg("ps");
        if all {
            ps.arg("--all");
        }
        ps.args([
            "--filter",
            &format!("label={LABEL_KEY}={}", engine.instance),
            "--format",
            "{{.Names}}",
        ]);
        let output = teardown::run_bounded(ps, Duration::from_secs(30)).expect("podman ps");
        teardown::names_of(&String::from_utf8_lossy(&output.stdout))
    }

    fn launch(
        backend: &PodmanBackend,
        session: &SessionContext,
        options: &LaunchOptions,
    ) -> SandboxHandle {
        let profile = profile();
        let plan = backend
            .plan(&PlanRequest::new(&profile, session, options))
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        backend
            .launch(&plan)
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why))
    }

    /// Die Sitzung startet, alle `CHECK`-Zeilen sind `ok`, der Exit-Status 7
    /// kommt als 7 an, und danach bleibt kein Container; ein `kill` auf eine
    /// laufende Sitzung lässt ebenso keinen zurück.
    #[test]
    fn a_session_checks_ok_passes_exit_seven_and_leaves_no_container() {
        if !podman_here("a_session_checks_ok_passes_exit_seven_and_leaves_no_container") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let handle = launch(
            &backend,
            &fx.context(&["sh", "-c", "sleep 1; exit 7"]),
            &options,
        );
        let checks = backend.isolation_check(&handle);
        for check in &checks {
            assert!(check.passed, "{}: {}", check.check.as_str(), check.evidence);
        }
        let status = handle
            .wait()
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        assert_eq!(status.code(), Some(7));
        assert!(containers(&fx.paths, &backend).is_empty());

        let handle = launch(&backend, &fx.context(&["sleep", "600"]), &options);
        assert!(handle.child_pid().is_some(), "PID 1 is known on the host");
        assert_eq!(containers(&fx.paths, &backend).len(), 1);
        let _ = handle.kill();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !containers(&fx.paths, &backend).is_empty() {
            assert!(Instant::now() < deadline, "the container outlived kill");
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Stirbt der Client der Engine, während der Container weiterläuft
    /// (P7), geht die Lebensleine zu: Der Container endet binnen
    /// `KILL_GRACE`, und der Daemon entfernt ihn ohne `kill` der Sitzung
    /// (HUM-452); er steht danach auch in `ps --all` nicht mehr.
    #[test]
    fn a_killed_client_does_not_leave_the_container_running() {
        killed_client_case(
            "a_killed_client_does_not_leave_the_container_running",
            StdioMode::Capture,
        );
    }

    /// Dasselbe am Pseudoterminal des Daemons: Dort hält der Shim die
    /// Untertanenseite, und der Leser des Terminals wartet auf ihn. Die
    /// Lebensleine geht trotzdem gleich nach dem Ende des Clients zu, nicht
    /// erst nach dem Einsammeln der Leser.
    #[test]
    fn a_killed_client_at_a_pty_does_not_leave_the_container_running() {
        killed_client_case(
            "a_killed_client_at_a_pty_does_not_leave_the_container_running",
            StdioMode::Pty { cols: 80, rows: 24 },
        );
    }

    fn killed_client_case(test: &str, stdio: StdioMode) {
        if !podman_here(test) {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let options = LaunchOptions {
            stdio,
            ..LaunchOptions::default()
        };
        let handle = launch(&backend, &fx.context(&["sleep", "600"]), &options);
        let checks = backend.isolation_check(&handle);
        assert!(checks.iter().all(|check| check.passed), "{checks:?}");
        let client = rustix::process::Pid::from_raw(i32::try_from(handle.pid).unwrap()).unwrap();
        rustix::process::kill_process(client, rustix::process::Signal::KILL).unwrap();
        assert!(
            handle.wait_timeout(Duration::from_secs(20)).is_some(),
            "the killed client is reaped"
        );
        // Die Lebensleine beendet den Container. `--rm` gehörte dem
        // getöteten Client; entfernt wird er vom Daemon, sobald er beendet
        // ist (HUM-452), und ausdrücklich ohne `kill` der Sitzung.
        let killed = Instant::now();
        while !running(&fx.paths, &backend).is_empty() {
            assert!(
                killed.elapsed() < KILL_GRACE + Duration::from_secs(15),
                "the container outlived its client by {:?}",
                killed.elapsed()
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        eprintln!(
            "{test}: the container stopped running {:?} after its client was killed",
            killed.elapsed()
        );
        loop {
            let left = containers(&fx.paths, &backend);
            if left.is_empty() {
                break;
            }
            assert!(
                killed.elapsed() < KILL_GRACE + Duration::from_secs(25),
                "an ended container is left without a kill of the session after {:?}: {left:?}; \
                 warnings so far: {:?}",
                killed.elapsed(),
                handle.take_teardown_warnings()
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        eprintln!(
            "{test}: ps --all is empty {:?} after its client was killed",
            killed.elapsed()
        );
        assert!(handle.wait_teardown(), "the removal finished");
        let warnings = handle.take_teardown_warnings();
        assert!(warnings.is_empty(), "{warnings:?}");
        // Ein Stopp danach hat nichts mehr zu tun.
        assert_eq!(handle.kill(), crate::handle::Termination::AlreadyEnded);
        assert!(containers(&fx.paths, &backend).is_empty());
    }

    /// Die Hälfte im Kind: spielt den Daemon, startet eine Sitzung, nennt
    /// ihren Container und wartet, bis der Elternprozess es tötet.
    #[test]
    #[ignore = "started by the_container_ends_with_the_daemon"]
    fn daemon_stand_in() {
        let Some(report) = std::env::var_os(STAND_IN_ENV) else {
            return;
        };
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let session = fx.context(&[
            "sh",
            "-c",
            "tr '\\000' '\\n' < /proc/self/environ > /work/env.txt; sleep 600",
        ]);
        let handle = launch(&backend, &session, &options);
        let checks = backend.isolation_check(&handle);
        assert!(checks.iter().all(|check| check.passed), "{checks:?}");
        let env_file = fx.work.join("env.txt");
        let deadline = Instant::now() + Duration::from_secs(20);
        while !env_file.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        // Über eine Datei, nicht über stdout: die Ausgabe eines Tests im
        // Kindprozess fängt libtest ab, auch mit `--nocapture` kam sie am
        // 2026-11-12 nicht beim Elternprozess an.
        let line = format!(
            "STAND-IN {} {} {}\n",
            container_name(session.session),
            env_file.display(),
            fx.dir.path().display()
        );
        let partial = PathBuf::from(&report).with_extension("partial");
        std::fs::write(&partial, line).expect("report");
        std::fs::rename(&partial, &report).expect("report");
        std::thread::sleep(Duration::from_secs(600));
    }

    /// Der Daemon stirbt durch `SIGKILL`; die Lebensleine beendet den
    /// Container nach `KILL_GRACE` plus Startfrist. Der Daemon hatte einen
    /// Proxy mit Zugangsdaten in seiner Umgebung: Weder die Umgebung des
    /// Agenten noch `podman inspect` des laufenden Containers nennt ihn.
    #[test]
    fn the_container_ends_with_the_daemon_and_never_sees_its_proxy() {
        if !podman_here("the_container_ends_with_the_daemon_and_never_sees_its_proxy") {
            return;
        }
        let exe = std::env::current_exe().expect("test binary");
        let handshake = tempfile::tempdir().expect("tempdir");
        let report = handshake.path().join("stand-in");
        let mut child = std::process::Command::new(exe)
            .args([
                "--exact",
                "podman::tests::live::daemon_stand_in",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(STAND_IN_ENV, &report)
            .env("HTTPS_PROXY", PROXY_WITH_CREDENTIALS)
            .env("HTTP_PROXY", PROXY_WITH_CREDENTIALS)
            .env("https_proxy", PROXY_WITH_CREDENTIALS)
            .stdout(Stdio::null())
            .spawn()
            .expect("the stand-in starts");
        let started = Instant::now();
        let line = loop {
            if let Ok(line) = std::fs::read_to_string(&report) {
                break line;
            }
            if let Ok(Some(status)) = child.try_wait() {
                panic!("the stand-in ended without starting a session: {status}");
            }
            if started.elapsed() > Duration::from_secs(120) {
                let _ = child.kill();
                panic!("the stand-in did not start a session within 120 s");
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let mut fields = line.trim().split(' ').skip(1);
        let name = fields.next().expect("name").to_owned();
        let env_file = PathBuf::from(fields.next().expect("env file"));
        let state = PathBuf::from(fields.next().expect("state dir"));
        let paths = Paths::new(
            humanitl_config::Env::from_process()
                .with("XDG_RUNTIME_DIR", state.join("runtime").to_string_lossy()),
        );
        let backend = real(&paths);
        let engine = EngineDir::of(&paths);
        let mut inspect = engine.command(backend.program(), &engine.containers_conf());
        inspect.args(["inspect", &name]);
        let inspected = teardown::run_bounded(inspect, Duration::from_secs(30)).expect("inspect");
        let inspected = String::from_utf8_lossy(&inspected.stdout).into_owned();
        let agent_env = std::fs::read_to_string(&env_file).unwrap_or_default();

        let _ = child.kill();
        let _ = child.wait();
        let killed = Instant::now();
        let limit = KILL_GRACE + Settings::from_config(&PodmanConfig::default()).start_timeout;
        let gone = loop {
            if !containers(&paths, &backend).contains(&name) {
                break true;
            }
            if killed.elapsed() > limit + Duration::from_secs(5) {
                break false;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        if !gone {
            let mut rm = engine.command(backend.program(), &engine.containers_conf());
            rm.args(["rm", "--force", "--time", "0", "--ignore", &name]);
            let _ = teardown::run_bounded(rm, Duration::from_secs(30));
        }
        assert!(
            inspected.contains(&name),
            "inspect saw the container: {inspected}"
        );
        assert!(!agent_env.is_empty(), "the agent wrote its environment");
        for needle in ["user:pw", "proxy.invalid"] {
            assert!(
                !agent_env.contains(needle),
                "agent environment: {agent_env}"
            );
            assert!(!inspected.contains(needle), "podman inspect: {inspected}");
        }
        assert!(
            gone,
            "the container outlived the daemon by {:?}",
            killed.elapsed()
        );
    }

    /// Was die Konfiguration des Nutzers sonst jedem Container gäbe, kommt
    /// nicht an: ein Volume und eine `mounts.conf` über
    /// `CONTAINERS_CONF_OVERRIDE`, ein Hook in dessen Hook-Verzeichnis, ein
    /// entfernter Dienst über `CONTAINER_HOST`.
    ///
    /// Der Fall `~/.config/containers/mounts.conf` mit eigenem `HOME` steht
    /// nicht hier: Ein fremdes `HOME` gäbe Podman einen zweiten Speicher
    /// neben demselben `runroot` des Nutzers, und dessen Zustand ginge
    /// kaputt. Er gehört in den CI-Job von HUM-435.
    #[test]
    fn the_users_configuration_does_not_reach_the_container() {
        if !podman_here("the_users_configuration_does_not_reach_the_container") {
            return;
        }
        let state = tempfile::tempdir().expect("tempdir");
        let secret = state.path().join("secret");
        std::fs::create_dir_all(&secret).unwrap();
        std::fs::write(secret.join("probe"), "s3cr3t").unwrap();
        let mounts = state.path().join("mounts.conf");
        std::fs::write(&mounts, format!("{}:/run/secrets\n", secret.display())).unwrap();
        let hooks = state.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let ran = state.path().join("hook-ran");
        std::fs::write(
            hooks.join("probe.json"),
            format!(
                r#"{{"version":"1.0.0","hook":{{"path":"/usr/bin/touch","args":["touch","{}"]}},"when":{{"always":true}},"stages":["prestart"]}}"#,
                ran.display()
            ),
        )
        .unwrap();
        let override_conf = state.path().join("override.conf");
        std::fs::write(
            &override_conf,
            format!(
                "[containers]\nvolumes=[\"/etc:/hostetc:ro\"]\ndefault_mounts_file=\"{}\"\n[engine]\nhooks_dir=[\"{}\"]\n",
                mounts.display(),
                hooks.display()
            ),
        )
        .unwrap();
        let fx = Fixture::with_env(
            Some(real_shim()),
            &[
                ("CONTAINERS_CONF_OVERRIDE", &override_conf.to_string_lossy()),
                ("CONTAINER_HOST", "unix:///nonexistent/podman.sock"),
            ],
        );
        let backend = real(&fx.paths);
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let handle = launch(
            &backend,
            &fx.context(&[
                "sh",
                "-c",
                "test -e /hostetc && echo VOLUME; test -e /run/secrets && echo SECRET; echo done",
            ]),
            &options,
        );
        let status = handle
            .wait()
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        let output = handle.output().expect("captured");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(status.success(), "{stdout}");
        assert!(stdout.contains("done"), "{stdout}");
        assert!(!stdout.contains("VOLUME"), "{stdout}");
        assert!(!stdout.contains("SECRET"), "{stdout}");
        assert!(!ran.exists(), "the user's hook ran");
    }

    /// Ein Agent am Pseudoterminal des Daemons: Er hat ein Terminal, eine
    /// neue Größe erreicht ihn, und eine weitere Shell bekommt ihr eigenes
    /// Terminal und den Filter des Agenten (`socket(AF_UNIX)` verweigert).
    #[test]
    fn a_session_at_a_pty_has_a_terminal_resizes_and_opens_shells() {
        const AGENT: &str = "trap 'stty size; exit 0' WINCH\n\
                             test -t 0 && test -t 1 && echo TTY-OK\n\
                             stty size\n\
                             while :; do sleep 0.05; done\n";
        if !podman_here("a_session_at_a_pty_has_a_terminal_resizes_and_opens_shells") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let (tx, rx) = mpsc::channel();
        let options = LaunchOptions {
            stdio: StdioMode::Pty { cols: 80, rows: 24 },
            output_sink: Some(tx),
            ..LaunchOptions::default()
        };
        let handle = launch(&backend, &fx.context(&["/bin/sh", "-c", AGENT]), &options);
        let mut seen = Vec::new();
        assert!(
            wait_for(&rx, &mut seen, "TTY-OK"),
            "{}",
            String::from_utf8_lossy(&seen)
        );
        assert!(
            wait_for(&rx, &mut seen, "24 80"),
            "{}",
            String::from_utf8_lossy(&seen)
        );

        let shell = handle
            .open_shell(
                &[
                    "/bin/sh".to_owned(),
                    "-c".to_owned(),
                    "test -t 0 && echo SHELL-TTY; python3 -c 'import socket; \
                     socket.socket(socket.AF_UNIX)' 2>&1 | tail -n 1"
                        .to_owned(),
                ],
                100,
                30,
            )
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        let mut shell_out = Vec::new();
        let _ = shell.pump(|bytes| shell_out.extend_from_slice(bytes));
        let shell_text = String::from_utf8_lossy(&shell_out).into_owned();

        handle.resize(120, 40).expect("resize");
        let resized = wait_for(&rx, &mut seen, "40 120");
        let _ = handle.kill();
        assert!(resized, "{}", String::from_utf8_lossy(&seen));
        assert!(shell_text.contains("SHELL-TTY"), "{shell_text}");
        assert!(
            shell_text.contains("PermissionError") || shell_text.contains("not permitted"),
            "the further shell carries the agent's filter: {shell_text}"
        );
    }

    fn wait_for(rx: &mpsc::Receiver<OutputChunk>, seen: &mut Vec<u8>, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if String::from_utf8_lossy(seen).contains(needle) {
                return true;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            match rx.recv_timeout(left.min(Duration::from_millis(250))) {
                Ok(chunk) => seen.extend_from_slice(&chunk.bytes),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return String::from_utf8_lossy(seen).contains(needle);
                }
            }
        }
    }

    // --- HUM-437: Mounttabelle und geprüfte Ordner --------------------------

    /// Plant, ändert die Zeile mit `edit`, startet und prüft; liefert die
    /// Ergebnisse, die Fehlerausgabe (dort druckt der Shim bei einer
    /// Abweichung beide Tabellen) und ob der Agent lief.
    fn launch_edited(
        fx: &Fixture,
        backend: &PodmanBackend,
        edit: impl FnOnce(&mut LaunchPlan),
    ) -> (Vec<CheckResult>, String, bool) {
        let profile = profile();
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let session = fx.context(&["sh", "-c", "echo AGENT-RAN"]);
        let mut plan = backend
            .plan(&PlanRequest::new(&profile, &session, &options))
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        edit(&mut plan);
        let handle = match backend.launch(&plan) {
            Ok(handle) => handle,
            Err(d) => return (Vec::new(), format!("{}: {}", d.code, d.why), false),
        };
        let checks = backend.isolation_check(&handle);
        let _ = handle.wait_timeout(Duration::from_secs(30));
        let _ = handle.kill();
        let output = handle.output().unwrap_or_default();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        (checks, stderr, stdout.contains("AGENT-RAN"))
    }

    /// Der Code der zweiten Garantie, wenn sie rot ist.
    fn door_code(checks: &[CheckResult]) -> Option<String> {
        checks
            .iter()
            .find(|check| check.check == crate::launcher::IsolationCheck::SingleSocket)
            .and_then(|check| check.diagnostic.as_ref())
            .map(|diagnostic| diagnostic.code.as_str().to_owned())
    }

    /// Fügt `--mount` vor `--rootfs` ein: eine Einhängung in der Zeile, die
    /// die erwartete Tabelle nicht kennt.
    fn add_mount(plan: &mut LaunchPlan, mount: &str) {
        let at = plan
            .argv
            .iter()
            .position(|arg| arg == "--rootfs")
            .expect("--rootfs");
        plan.argv.insert(at, format!("--mount={mount}").into());
    }

    /// Ohne Eingriff stimmt die Tabelle und jeder geprüfte Ordner (P17).
    /// Weicht die gemessene Liste in `mounts.rs` von dem ab, was Podman
    /// tatsächlich einhängt, druckt dieser Test `/proc/self/mountinfo` aus
    /// dem Container und die erwartete Multimenge; daraus lässt sich
    /// [`super::mounts`] in einem Lauf richtigstellen.
    #[test]
    fn the_mount_table_of_a_plain_session_matches_the_plan() {
        if !podman_here("the_mount_table_of_a_plain_session_matches_the_plan") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let (checks, stderr, ran) = launch_edited(&fx, &backend, |_| {});
        assert!(
            checks.iter().all(|check| check.passed),
            "{checks:#?}\n--- shim stderr (mountinfo and expected table) ---\n{stderr}"
        );
        assert!(ran, "{stderr}");
    }

    /// Der Ordner wird nach `plan` und vor `launch` umbenannt und durch einen
    /// anderen mit denselben Masken ersetzt: Podman hängt den Pfad ein, also
    /// den falschen Ordner, und der Shim lehnt mit `SANDBOX_042` ab. Der Agent
    /// läuft nicht (Akzeptanzkriterium 1).
    #[test]
    fn a_swapped_project_folder_is_refused_with_sandbox_042() {
        if !podman_here("a_swapped_project_folder_is_refused_with_sandbox_042") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let moved = fx.work.with_extension("checked");
        let (checks, stderr, ran) = launch_edited(&fx, &backend, |_| {
            std::fs::rename(&fx.work, &moved).expect("rename");
            // Dieselben Pfade wie im geprüften Ordner, damit keine Maske
            // einen Mountpunkt im untergeschobenen anlegen muss.
            std::fs::create_dir_all(fx.work.join(".git/hooks")).expect("hooks");
            std::fs::write(fx.work.join(".envrc"), "swapped\n").expect("envrc");
        });
        assert!(!ran, "the agent started in the swapped folder: {stderr}");
        assert_eq!(
            door_code(&checks).as_deref(),
            Some("SANDBOX_042"),
            "{checks:#?}\n{stderr}"
        );
        let door = checks
            .iter()
            .find(|check| check.check == crate::launcher::IsolationCheck::SingleSocket)
            .expect("door");
        assert!(door.evidence.contains("path=/work"), "{door:?}");
    }

    /// Eine Einhängung, die nicht im Plan steht, wird mit `SANDBOX_049`
    /// abgelehnt: ein tmpfs auf einem neuen Punkt (`/etc`, den die leere
    /// Wurzel schon hat). Gestapelte Ziele (ein zweiter Bind auf die CA, ein
    /// tmpfs über `/work`, ein Bind über die Tür) lehnt Podman schon selbst
    /// als doppeltes Ziel ab; dann startet der Agent ebenso nicht, und
    /// meldet der Shim, dann mit `SANDBOX_049`.
    #[test]
    fn an_unplanned_mount_is_refused_with_sandbox_049() {
        if !podman_here("an_unplanned_mount_is_refused_with_sandbox_049") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let (checks, stderr, ran) = launch_edited(&fx, &backend, |plan| {
            add_mount(plan, "type=tmpfs,destination=/etc");
        });
        assert!(!ran, "{stderr}");
        assert_eq!(
            door_code(&checks).as_deref(),
            Some("SANDBOX_049"),
            "{checks:#?}\n{stderr}"
        );
        let ca = fx.dir.path().join("ca.crt");
        for stacked in [
            format!(
                "type=bind,source={},destination=/etc/humanitl/ca.crt,readonly=true",
                ca.display()
            ),
            "type=tmpfs,destination=/work".to_owned(),
            format!(
                "type=bind,source={},destination={},readonly=true",
                ca.display(),
                crate::PROXY_SOCKET_DST
            ),
        ] {
            let (checks, stderr, ran) =
                launch_edited(&fx, &backend, |plan| add_mount(plan, &stacked));
            assert!(!ran, "{stacked}: {stderr}");
            // Podman 5.8.6 lehnt ein doppeltes Ziel selbst ab ("duplicate
            // mount destination", gemessen 2026-11-12), bevor der Shim
            // startet; dann meldet er nichts (`SANDBOX_013`). Kommt der Shim
            // doch zum Zug, lehnt er mit `SANDBOX_049` ab.
            let by_engine = stderr.contains("duplicate mount destination");
            let code = door_code(&checks);
            assert!(
                by_engine || code.as_deref() == Some("SANDBOX_049"),
                "{stacked}: neither refused by the engine nor by the shim: {checks:#?}\n{stderr}"
            );
            eprintln!(
                "HUM-437 stacked {stacked}: {}",
                if by_engine {
                    "refused by the engine as a duplicate destination"
                } else {
                    "refused by the shim with SANDBOX_049"
                }
            );
        }
    }

    /// Ohne `--read-only-tmpfs=false` legt Podman tmpfs auf `/run`, `/tmp`
    /// und `/var/tmp`; die Tabelle kennt sie nicht, `SANDBOX_049`.
    #[test]
    fn without_read_only_tmpfs_false_the_extra_tmpfs_are_refused() {
        if !podman_here("without_read_only_tmpfs_false_the_extra_tmpfs_are_refused") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let (checks, stderr, ran) = launch_edited(&fx, &backend, |plan| {
            plan.argv.retain(|arg| arg != "--read-only-tmpfs=false");
        });
        assert!(!ran, "{stderr}");
        assert_eq!(
            door_code(&checks).as_deref(),
            Some("SANDBOX_049"),
            "{checks:#?}\n{stderr}"
        );
    }

    /// Eine `mounts.conf` des Nutzers, an der leeren Datei des Daemons
    /// vorbei erzwungen (`--default-mounts-file` zeigt auf sie): Ihr Eintrag
    /// erscheint im Container und wird mit `SANDBOX_049` abgelehnt, auch
    /// wenn er keinen Socket trägt.
    #[test]
    fn a_users_mounts_conf_forced_past_the_daemons_file_is_refused() {
        if !podman_here("a_users_mounts_conf_forced_past_the_daemons_file_is_refused") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let secret = fx.dir.path().join("secret");
        std::fs::create_dir_all(&secret).unwrap();
        std::fs::write(secret.join("probe"), "s3cr3t").unwrap();
        let mounts_conf = fx.dir.path().join("users-mounts.conf");
        std::fs::write(&mounts_conf, format!("{}:/run/secrets\n", secret.display())).unwrap();
        let (checks, stderr, ran) = launch_edited(&fx, &backend, |plan| {
            let flag = plan
                .argv
                .iter_mut()
                .find(|arg| arg.to_string_lossy().starts_with("--default-mounts-file="))
                .expect("--default-mounts-file");
            *flag = format!("--default-mounts-file={}", mounts_conf.display()).into();
        });
        assert!(!ran, "{stderr}");
        assert_eq!(
            door_code(&checks).as_deref(),
            Some("SANDBOX_049"),
            "{checks:#?}\n{stderr}"
        );
        assert!(stderr.contains("/run/secrets"), "{stderr}");
    }

    /// Messung P13 (HUM-292): Hängt Podman `/proc/self/fd/N` als den
    /// geöffneten Ordner ein? Der Plan gibt den geöffneten Projektordner
    /// zusätzlich als Deskriptor mit und nennt ihn als Quelle von `/work`;
    /// danach wird der Ordner getauscht. Gemessen 2026-11-12 unter 5.8.6:
    /// `crun: mount /proc/self/fd/3 to work: Invalid argument`, und mit
    /// diesem Test (HUM-437) `P13: started=true door=Some("SANDBOX_013")
    /// agent_ran=false`: Der Container entsteht nicht, der Shim meldet
    /// nichts, der Agent läuft nie. Kein brauchbarer Weg; es bleibt der
    /// Vergleich. Der Test
    /// druckt das Ergebnis (`P13: ...`) und verlangt nur, dass der Agent nie
    /// im getauschten Ordner läuft. Von Hand:
    ///
    /// ```text
    /// cargo test -p humanitl-sandbox --lib p13_ -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement (HUM-292 P13), run by hand"]
    fn p13_a_descriptor_as_the_mount_source() {
        if !podman_here("p13_a_descriptor_as_the_mount_source") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let backend = real(&fx.paths);
        let moved = fx.work.with_extension("checked");
        let (checks, stderr, ran) = launch_edited(&fx, &backend, |plan| {
            let opened = std::fs::File::open(&fx.work).expect("open the folder");
            let next = 3 + i32::try_from(plan.fds.len()).expect("few descriptors");
            let work = fx.work.canonicalize().expect("resolved");
            for arg in &mut plan.argv {
                let text = arg.to_string_lossy().into_owned();
                if text.ends_with(",destination=/work") || text.contains(",destination=/work,") {
                    *arg = text
                        .replace(
                            &format!("source={},", work.display()),
                            &format!("source=/proc/self/fd/{next},"),
                        )
                        .into();
                }
                if text.starts_with("--preserve-fds=") {
                    *arg = format!("--preserve-fds={}", plan.fds.len() + 1).into();
                }
            }
            let fd = std::os::fd::OwnedFd::from(opened);
            plan.fds.push((std::os::fd::AsRawFd::as_raw_fd(&fd), next));
            if let Some(once) = plan.once.lock().expect("once").as_mut() {
                // Vor der Untertanenseite, die der Start als letzte anhängt;
                // ohne Pseudoterminal gibt es keine.
                once.inherit.push(fd);
            }
            std::fs::rename(&fx.work, &moved).expect("rename");
            std::fs::create_dir_all(fx.work.join(".git/hooks")).expect("hooks");
            std::fs::write(fx.work.join(".envrc"), "swapped\n").expect("envrc");
        });
        eprintln!(
            "P13: started={} door={:?} agent_ran={ran}\n{stderr}",
            !checks.is_empty(),
            door_code(&checks)
        );
        if ran {
            assert!(
                checks.iter().all(|check| check.passed),
                "the agent ran although a check failed: {checks:#?}"
            );
        }
    }

    // --- HUM-439: ein Abbild als Wurzel --------------------------------------

    /// Der Name des Abbilds, das die Tests aus einem Archiv bauen, ohne Netz.
    const LOCAL_IMAGE: &str = "localhost/humanitl-hum439";

    /// Ein Abbild, gegen das eine ganze Sitzung läuft: `name@sha256:...`, im
    /// lokalen Speicher schon vorhanden, mit `/bin/sh` und `stat`. Ohne die
    /// Variable überspringt der Test (er zieht nie selbst).
    const SESSION_IMAGE_ENV: &str = "HUMANITL_TEST_IMAGE";

    /// Ein Shim, der im Abbild läuft (statisch gebaut), statt dem neben dem
    /// Testprogramm; der gebaute ist dynamisch und braucht die glibc des Hosts.
    const SESSION_SHIM_ENV: &str = "HUMANITL_TEST_IMAGE_SHIM";

    /// `podman <args>` mit der Umgebung und der Konfiguration des Adapters;
    /// ein Fehlschlag druckt alles, was Podman sagte.
    fn engine_run(paths: &Paths, backend: &PodmanBackend, args: &[&str]) -> std::process::Output {
        let engine = EngineDir::of(paths);
        let mut command = engine.command(backend.program(), &engine.containers_conf());
        command.args(args);
        let output = teardown::run_bounded(command, Duration::from_secs(120))
            .unwrap_or_else(|err| panic!("podman {args:?}: {err}"));
        assert!(
            output.status.success(),
            "podman {args:?} failed with {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    /// Baut aus einem Archiv mit einer Datei ein Abbild unter
    /// [`LOCAL_IMAGE`]`:<tag>`, ohne Netz, und liefert seinen Digest
    /// (`sha256:<hex>`).
    fn import_local_image(paths: &Paths, backend: &PodmanBackend, tag: &str) -> String {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("marker"), tag).expect("marker");
        let tar = dir.path().join("image.tar");
        let status = Command::new("tar")
            .arg("-cf")
            .arg(&tar)
            .arg("-C")
            .arg(dir.path())
            .arg("marker")
            .status()
            .expect("tar runs");
        assert!(status.success(), "tar: {status}");
        let name = format!("{LOCAL_IMAGE}:{tag}");
        engine_run(paths, backend, &["import", &tar.to_string_lossy(), &name]);
        let output = engine_run(
            paths,
            backend,
            &["image", "inspect", "--format", "{{.Digest}}", &name],
        );
        let digest = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        assert!(
            digest.starts_with("sha256:") && digest.len() == 71,
            "the imported image has no digest: {digest:?}"
        );
        digest
    }

    /// Die Digests aller Abbilder unter [`LOCAL_IMAGE`], sortiert.
    fn local_digests(paths: &Paths, backend: &PodmanBackend) -> Vec<String> {
        let output = engine_run(
            paths,
            backend,
            &[
                "images",
                "--all",
                "--filter",
                &format!("reference={LOCAL_IMAGE}"),
                "--format",
                "{{.Digest}}",
            ],
        );
        let mut digests: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect();
        digests.sort();
        digests
    }

    /// Das Backend mit einem Abbild als Wurzel.
    fn real_with_image(paths: &Paths, reference: &str) -> PodmanBackend {
        PodmanBackend::detect(
            paths.clone(),
            &PodmanConfig {
                rootfs: humanitl_config::PodmanRootfs::Image,
                image: Some(reference.to_owned()),
                ..PodmanConfig::default()
            },
        )
        .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why))
    }

    /// Unter dem Namen liegt ein Abbild mit einem anderen Digest als dem
    /// eingestellten: Der Start lehnt mit `SANDBOX_043` ab, nennt den Befehl,
    /// der zieht, und zieht nichts; die Abbilder unter dem Namen sind danach
    /// dieselben wie vorher. Mit dem richtigen Digest geht der Plan durch.
    ///
    /// Mutationsbeweis (HUM-439): Ohne die Prüfung in `plan` ginge der Plan
    /// mit dem falschen Digest durch, und `expect_err` färbt diesen Test rot.
    #[test]
    fn an_image_with_another_digest_is_refused_and_nothing_is_pulled() {
        if !podman_here("an_image_with_another_digest_is_refused_and_nothing_is_pulled") {
            return;
        }
        let fx = Fixture::new(Some(real_shim()));
        let host_root = real(&fx.paths);
        let digest = import_local_image(&fx.paths, &host_root, "a");
        // Derselbe Name, ein anderer Digest: die letzte Ziffer gedreht.
        let mut other = digest.clone();
        let last = other.pop().expect("a digit");
        other.push(if last == '0' { '1' } else { '0' });
        let before = local_digests(&fx.paths, &host_root);
        assert!(before.contains(&digest), "{before:?}");

        let wrong = format!("{LOCAL_IMAGE}@{other}");
        let backend = real_with_image(&fx.paths, &wrong);
        let profile = profile();
        let session = fx.context(&["true"]);
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let err = backend
            .plan(&PlanRequest::new(&profile, &session, &options))
            .expect_err("another digest under the same name must not start");
        eprintln!("refused as expected: {}: {}", err.code, err.why);
        assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
        assert_eq!(
            err.fix,
            Some(FixAction::CopyCommand(format!(
                "humanitl sandbox image pull {wrong}"
            )))
        );
        assert_eq!(
            local_digests(&fx.paths, &host_root),
            before,
            "the refused start changed the local images"
        );
        assert!(containers(&fx.paths, &backend).is_empty());

        let right = format!("{LOCAL_IMAGE}@{digest}");
        let backend = real_with_image(&fx.paths, &right);
        let plan = backend
            .plan(&PlanRequest::new(&profile, &session, &options))
            .unwrap_or_else(|d| panic!("the pinned digest is present: {}: {}", d.code, d.why));
        assert!(
            plan.argv.iter().any(|arg| arg.to_string_lossy() == right),
            "{:?}",
            plan.argv
        );
        if let Some(dir) = &plan.session_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
        let _ = engine_run(
            &fx.paths,
            &host_root,
            &["rmi", "--force", &format!("{LOCAL_IMAGE}:a")],
        );
    }

    /// Eine ganze Sitzung auf einem Abbild ([`SESSION_IMAGE_ENV`]): Alle
    /// `CHECK`-Zeilen sind `ok`, die Mounttabelle stimmt (Wurzel ein nur
    /// lesbares `overlay`), der Agent läuft, und die CA im Container ist die
    /// Datei des Hosts, nach Gerät und Inode (Akzeptanzkriterium 2). Bei
    /// einer Abweichung druckt der Test die Fehlerausgabe des Shims mit
    /// `mountinfo` und der erwarteten Tabelle.
    #[test]
    fn a_session_on_an_image_checks_ok_and_its_ca_is_the_hosts() {
        let test = "a_session_on_an_image_checks_ok_and_its_ca_is_the_hosts";
        if !podman_here(test) {
            return;
        }
        let Some(reference) = std::env::var(SESSION_IMAGE_ENV)
            .ok()
            .filter(|value| !value.is_empty())
        else {
            eprintln!(
                "skipped: {test} needs {SESSION_IMAGE_ENV}=NAME@sha256:... of a local image \
                 with /bin/sh and stat (it never pulls)"
            );
            return;
        };
        let shim = std::env::var_os(SESSION_SHIM_ENV).map_or_else(real_shim, PathBuf::from);
        let fx = Fixture::new(Some(shim));
        let backend = real_with_image(&fx.paths, &reference);
        let profile = profile();
        let options = LaunchOptions {
            stdio: StdioMode::Capture,
            ..LaunchOptions::default()
        };
        let session = fx.context(&[
            "/bin/sh",
            "-c",
            "echo AGENT-RAN; stat -c 'CA=%d:%i' /etc/humanitl/ca.crt",
        ]);
        let plan = backend
            .plan(&PlanRequest::new(&profile, &session, &options))
            .unwrap_or_else(|d| panic!("{}: {}", d.code, d.why));
        let argv = plan
            .argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        let handle = backend
            .launch(&plan)
            .unwrap_or_else(|d| panic!("{}: {}\n--- argv ---\n{argv}", d.code, d.why));
        let checks = backend.isolation_check(&handle);
        let _ = handle.wait_timeout(Duration::from_secs(60));
        let _ = handle.kill();
        let output = handle.output().unwrap_or_default();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let report = format!(
            "{checks:#?}\n--- argv ---\n{argv}\n--- stdout ---\n{stdout}\n--- shim stderr \
             (mountinfo and expected table on a mismatch) ---\n{stderr}"
        );
        assert!(
            !checks.is_empty() && checks.iter().all(|check| check.passed),
            "{report}"
        );
        assert!(stdout.contains("AGENT-RAN"), "{report}");
        let meta = std::fs::metadata(&fx.ca).expect("the host CA");
        let host = format!(
            "CA={}:{}",
            std::os::unix::fs::MetadataExt::dev(&meta),
            std::os::unix::fs::MetadataExt::ino(&meta)
        );
        assert!(
            stdout.lines().any(|line| line.trim() == host),
            "the CA in the container is not the host's file ({host})\n{report}"
        );
    }
}
