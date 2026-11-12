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
    use std::os::unix::fs::PermissionsExt as _;
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
        ca: PathBuf,
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

    /// Der Plan nennt die Deskriptoren fortlaufend ab 3, trägt keinen Wert
    /// der Umgebung in der Zeile, gibt Podman nur die Pfade seiner
    /// Konfiguration mit und beendet über die Engine.
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
            [3, 4, 5]
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
            planned.contains(&"--preserve-fds=5".to_owned()),
            "{planned:?}"
        );
    }

    #[test]
    fn disposable_image_and_krun_are_refused_and_never_replaced() {
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
        assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
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
    use crate::launcher::{LaunchOptions, StdioMode};

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
    /// `KILL_GRACE`, und ein späteres `kill` räumt über die Engine auf.
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
        // Die Lebensleine beendet den Container; entfernt wird er erst von
        // `kill` über die Engine, denn `--rm` gehörte dem getöteten Client.
        let killed = Instant::now();
        while !running(&fx.paths, &backend).is_empty() {
            assert!(
                killed.elapsed() < KILL_GRACE + Duration::from_secs(15),
                "the container outlived its client by {:?}",
                killed.elapsed()
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        assert_eq!(handle.kill(), crate::handle::Termination::AlreadyEnded);
        assert!(
            containers(&fx.paths, &backend).is_empty(),
            "kill removes the ended container through the engine"
        );
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
}
