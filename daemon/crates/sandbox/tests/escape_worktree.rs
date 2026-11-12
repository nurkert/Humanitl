//! Die drei Fälle, die ESC-5 zu `/work` misst (HUM-043, BACKLOG.md 4.5).
//!
//! `tests/escape/esc-5-filesystem.sh` ruft genau diese drei Tests einzeln auf
//! und liest ihr Ergebnis, so wie ESC-4 die Tests der Regel-Engine aufruft: Die
//! Fälle brauchen den echten Launcher und ein echtes Dateisystem, und es gibt
//! kein Kommando, das sie von außen stellen könnte, ohne den halben Daemon zu
//! starten.
//!
//! **Ein fehlendes Werkzeug ist nie ein bestandener Fall.** Zwei der drei Fälle
//! brauchen `bwrap` und einen Kernel mit unprivilegierten Nutzer-Namensräumen.
//! Fehlt eines, schreibt der Test eine Zeile [`SKIP_MARKER`] auf die Ausgabe
//! und endet grün — und `esc-5-filesystem.sh` macht daraus ein `skip`, kein
//! `pass`. Ohne diesen Umweg läse „kein bwrap auf dieser Maschine" sich in der
//! Auswertung wie „die Maske hat gehalten".
//!
//! **Jedes Backend (HUM-435).** Nennt `HUMANITL_ESCAPE_BACKEND` ein anderes
//! Backend als das der Plattform, starten die Fälle genau dieses, über die
//! Umgehung der Zulassung (`Backend::detect_unadmitted`);
//! `esc-5-filesystem.sh` baut den Test dafür mit
//! `--features unadmitted-backends`. Fehlt das verlangte Backend, ist das
//! immer ein Fehlschlag, nie ein `skip`, und nie läuft der Fall still gegen
//! ein anderes Backend.

// Startet Sandboxen mit dem Linux-Backend (HUM-312).
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod common;

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::ids::{SandboxId, SessionId};
use humanitl_sandbox::summary::SessionSummary;
use humanitl_sandbox::worktree::{SnapshotLimits, snapshot};
use humanitl_sandbox::{
    Backend, BwrapBackend, LaunchOptions, PLATFORM_BACKEND_NAME, PlanRequest, SandboxBackend,
    SandboxHandle, SandboxProfile, SessionContext, StdioMode,
};

/// Die Zeile, an der `esc-5-filesystem.sh` einen übersprungenen Fall erkennt.
const SKIP_MARKER: &str = "ESC5-SKIP";

/// Kein Fall wartet länger auf die Sandbox.
const WAIT: Duration = Duration::from_secs(60);

/// Ein Shim-Ersatz, der nur den Befehl startet: Diese Fälle messen die Mounts,
/// nicht den seccomp-Filter (das tun ESC-1 bis ESC-3 in der Sandbox).
const PASSTHROUGH_SHIM: &str = "#!/bin/sh\n\
                                if [ \"$1\" = \"--proxy-port\" ]; then shift 2; fi\n\
                                if [ \"$1\" = \"--pins-fd\" ]; then shift 2; fi\n\
                                if [ \"$1\" = \"--\" ]; then shift; fi\n\
                                exec \"$@\"\n";

/// Der echte Shim neben dem Testprogramm, für jedes andere Backend: Dessen
/// Start wartet auf den Bericht des Shims aus der Sandbox, und ein Ersatz,
/// der nur den Befehl startet, schickt keinen. `tests/escape/run.sh` legt den
/// ausgelieferten Shim dorthin.
fn real_shim() -> PathBuf {
    let mut dir = std::env::current_exe().expect("test binary");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let shim = dir.join("humanitl-shim");
    assert!(
        shim.is_file(),
        "{} is missing; tests/escape/run.sh puts the shim there",
        shim.display()
    );
    shim
}

/// Die Variable, mit der die Escape-Suite das Backend nennt (HUM-435).
const ESCAPE_BACKEND_ENV: &str = "HUMANITL_ESCAPE_BACKEND";

/// Das Backend, das die Escape-Suite verlangt, wenn es nicht das der
/// Plattform ist.
fn requested_backend() -> Option<String> {
    std::env::var(ESCAPE_BACKEND_ENV)
        .ok()
        .filter(|name| !name.is_empty() && name != PLATFORM_BACKEND_NAME)
}

struct Fixture {
    _dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    socket: PathBuf,
    _listener: UnixListener,
    ca: PathBuf,
    ca_bundle: PathBuf,
    shim: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().to_path_buf();
        let runtime = state.join("runtime");
        let paths =
            Paths::new(Env::from_process().with("XDG_RUNTIME_DIR", runtime.to_string_lossy()));

        let work = state.join("work");
        fs::create_dir_all(&work).expect("work");

        let socket = paths.proxy_socket();
        fs::create_dir_all(socket.parent().expect("parent")).expect("socket dir");
        fs::set_permissions(
            socket.parent().expect("parent"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("chmod dir");
        let listener = UnixListener::bind(&socket).expect("bind placeholder socket");
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");

        let ca = state.join("ca.crt");
        fs::write(&ca, b"-----BEGIN CERTIFICATE-----\n").expect("ca");
        let ca_bundle = state.join("ca-bundle.crt");
        fs::write(&ca_bundle, b"-----BEGIN CERTIFICATE-----\n").expect("ca bundle");
        let shim = state.join("humanitl-shim");
        // Über einen eigenen `cp`-Prozess wie in `common::write_program`: Die
        // Datei wird gleich ausgeführt, und ein geerbter Schreib-Deskriptor
        // eines anderen Fadens gäbe `ETXTBSY` (HUM-134).
        if requested_backend().is_some() {
            let status = std::process::Command::new("cp")
                .arg(real_shim())
                .arg(&shim)
                .status()
                .expect("cp runs");
            assert!(status.success(), "copying the shim failed: {status}");
        } else {
            common::write_program(&shim, PASSTHROUGH_SHIM);
        }

        Self {
            _dir: dir,
            paths,
            work,
            socket,
            _listener: listener,
            ca,
            ca_bundle,
            shim,
        }
    }

    fn context(&self, command: &[&str]) -> SessionContext {
        SessionContext {
            session: SessionId::nil(),
            work_src: self.work.clone(),
            work_mode: WorkMode::Rw,
            proxy_socket_src: self.socket.clone(),
            ca_cert_src: self.ca.clone(),
            ca_bundle_src: self.ca_bundle.clone(),
            shim_src: self.shim.clone(),
            session_env: vec![("HUMANITL_SESSION".to_owned(), SessionId::nil().to_string())],
            command: command.iter().map(OsString::from).collect(),
            files: Vec::new(),
            agent_program: None,
        }
    }

    /// Das echte `bwrap`, oder `None` samt der Zeile, die ESC-5 als `skip`
    /// liest. Verlangt die Escape-Suite ein anderes Backend, dieses
    /// ([`requested`]).
    ///
    /// **Unter `CI` ist ein fehlendes `bwrap` ein Fehler.** Wer hier `None`
    /// bekommt, kehrt zurück, und ein zurückkehrender Test gilt dem Testläufer
    /// als bestanden: Die drei Dateisystem-Fälle von ESC-5 — der Symlink aus
    /// `/work` hinaus, die Maske, die hält, und der Hook, der drinnen bleibt —
    /// meldeten dann `ok`, ohne dass eine einzige Zusicherung gelaufen wäre.
    /// Genau diese drei sind das erste Akzeptanzkriterium von HUM-043. Auf
    /// einer Entwicklermaschine darf `bwrap` fehlen, auf dem Runner nicht
    /// (dieselbe Regel wie `shim_contract.rs` und
    /// `daemon/bin/humanitl/tests/cli.rs`).
    fn backend(&self) -> Option<Backend> {
        if let Some(name) = requested_backend() {
            return Some(requested(&name, &self.paths));
        }
        match BwrapBackend::detect(self.paths.clone()) {
            Ok(backend) => Some(Backend::from(backend)),
            Err(err) => {
                assert!(
                    std::env::var_os("CI").is_none(),
                    "under CI this test must run: bwrap is not usable on this machine ({}); \
                     install it (apt-get install -y bubblewrap) and allow unprivileged user \
                     namespaces (sysctl -w kernel.apparmor_restrict_unprivileged_userns=0)",
                    err.why
                );
                println!(
                    "{SKIP_MARKER} bwrap is not usable on this machine: {}",
                    err.why
                );
                None
            }
        }
    }
}

/// Das verlangte Backend über die Umgehung der Zulassung. Läuft es hier
/// nicht, ist das ein Fehlschlag und kein `skip`: Wer es ausdrücklich nennt,
/// will genau dieses geprüft haben (HUM-435, Akzeptanzkriterium 2).
#[cfg(feature = "unadmitted-backends")]
fn requested(name: &str, paths: &Paths) -> Backend {
    Backend::detect_unadmitted(name, paths.clone(), &humanitl_config::SandboxRef::default())
        .unwrap_or_else(|err| {
            panic!(
                "{ESCAPE_BACKEND_ENV}={name} asks for this backend, and it is not usable \
                     here: {}",
                err.why
            )
        })
}

/// Ohne das Feature gibt es die Umgehung nicht, und ein verlangtes Backend
/// ist ein Fehler der Verdrahtung, kein `skip`: Sonst liefe der Fall still
/// gegen ein anderes Backend.
#[cfg(not(feature = "unadmitted-backends"))]
fn requested(name: &str, _paths: &Paths) -> Backend {
    panic!(
        "{ESCAPE_BACKEND_ENV}={name} needs this test built with --features unadmitted-backends \
         (tests/escape/esc-5-filesystem.sh passes it); without it the case would run against \
         another backend"
    );
}

fn profile() -> SandboxProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
    SandboxProfile::load(&path)
        .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
}

/// Startet den Befehl in der Sandbox und liefert Ausgabe und die Pfade, über
/// denen in diesem Lauf kein `tmpfs` lag.
fn run(fx: &Fixture, backend: &Backend, command: &[&str]) -> (String, Vec<PathBuf>) {
    let session = fx.context(command);
    let options = LaunchOptions {
        stdio: StdioMode::Capture,
        ..LaunchOptions::default()
    };
    let plan = backend
        .plan(&PlanRequest::new(&profile(), &session, &options))
        .unwrap_or_else(|err| panic!("the plan fails: {}", err.why));
    let unprotected = plan.unprotected.clone();
    let handle: SandboxHandle = backend
        .launch(&plan)
        .unwrap_or_else(|err| panic!("{} does not start: {}", backend.name(), err.why));
    handle
        .wait_timeout(WAIT)
        .expect("the sandbox ends within the timeout")
        .unwrap_or_else(|err| {
            panic!(
                "{} failed before the command ran: {}",
                backend.name(),
                err.why
            )
        });
    let output = handle.output().expect("stdio was captured");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        unprotected,
    )
}

/// ESC-5: Ein Symlink aus `/work` hinaus steht in der Zusammenfassung.
#[test]
fn symlink_out_of_work_is_marked() {
    let fx = Fixture::new();
    fs::write(fx.work.join("keep.txt"), b"content\n").expect("write");
    let before = snapshot(&fx.work, &SnapshotLimits::default()).expect("first snapshot");

    // Was ein Agent tut, der etwas vom Host lesbar machen will.
    std::os::unix::fs::symlink("/etc/passwd", fx.work.join("passwd")).expect("symlink");
    std::os::unix::fs::symlink("../../..", fx.work.join("up")).expect("symlink");
    std::os::unix::fs::symlink("keep.txt", fx.work.join("here")).expect("symlink");

    let after = snapshot(&fx.work, &SnapshotLimits::default()).expect("second snapshot");
    let mut summary = SessionSummary::new(SessionId::nil(), SandboxId::nil(), &fx.work);
    let _candidates = summary.add_changes(&fx.work, &before, &after);

    let escaping: Vec<&str> = summary
        .symlinks
        .iter()
        .filter(|link| link.escapes)
        .map(|link| link.path.as_str())
        .collect();
    assert!(escaping.contains(&"passwd"), "{:?}", summary.symlinks);
    assert!(escaping.contains(&"up"), "{:?}", summary.symlinks);
    assert!(
        !escaping.contains(&"here"),
        "a link inside the project is not an escape: {:?}",
        summary.symlinks
    );

    let codes: Vec<&str> = summary
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(
        codes.iter().filter(|code| **code == "SANDBOX_022").count(),
        2,
        "one warning per escaping symlink: {codes:?}"
    );

    // Und der Daemon hat dabei nichts vom Host gelesen: Das Ziel steht
    // wörtlich in der Zusammenfassung, aufgelöst wurde es nie.
    let passwd = summary
        .symlinks
        .iter()
        .find(|link| link.path == "passwd")
        .expect("the link is listed");
    assert_eq!(passwd.target, "/etc/passwd");

    // Der Befehl für die Zwischenablage zeigt auf den echten Pfad im Projekt,
    // nicht auf den angezeigten Namen.
    assert_eq!(
        passwd.fix_command,
        Some(format!("rm -- {}", fx.work.join("passwd").display())),
        "the command names the real path"
    );
}

/// ESC-5: `/work/.envrc` und `/work/.git/config` bleiben leer, und der Host
/// behält seine Fassung.
#[test]
fn masked_path_stays_masked() {
    let fx = Fixture::new();
    fs::create_dir_all(fx.work.join(".git")).expect("git dir");
    fs::write(fx.work.join(".envrc"), b"export CANARY=envrc\n").expect("envrc");
    fs::write(fx.work.join(".env"), b"TOKEN=canary\n").expect("env");
    fs::write(fx.work.join(".git/config"), b"[user]\n\tname = canary\n").expect("git config");

    let Some(backend) = fx.backend() else {
        return;
    };
    let (output, _unprotected) = run(
        &fx,
        &backend,
        &[
            "sh",
            "-c",
            "echo '--- envrc'; cat /work/.envrc; echo '--- env'; cat /work/.env; \
             echo '--- config'; cat /work/.git/config; echo '--- write'; \
             (echo pwned > /work/.envrc && echo WROTE) || echo REFUSED",
        ],
    );

    assert!(
        !output.contains("canary"),
        "a masked file leaked its content into the sandbox: {output}"
    );
    assert!(
        output.contains("REFUSED"),
        "writing to a masked file must fail: {output}"
    );
    for (file, expected) in [
        (".envrc", "export CANARY=envrc\n"),
        (".env", "TOKEN=canary\n"),
        (".git/config", "[user]\n\tname = canary\n"),
    ] {
        let on_host = fs::read_to_string(fx.work.join(file)).expect("host file");
        assert_eq!(on_host, expected, "{file} changed on the host");
    }
}

/// ESC-5: Ein Hook, den der Agent schreibt, bleibt in der Sandbox.
///
/// Und die Kehrseite derselben Entscheidung: Fehlt `.git/hooks` auf dem Host,
/// liegt kein `tmpfs` darüber, der Hook landet im Projekt — und der Plan sagt
/// das vorher (HUM-043). Humanitl legt das Verzeichnis nicht selbst an; es
/// schreibt nicht in das Projekt des Nutzers, auch nicht ein leeres
/// Verzeichnis.
#[test]
fn hooks_write_stays_in_sandbox() {
    let fx = Fixture::new();
    // Ein echtes Repository: `git init` legt `hooks/` an, also gibt es den
    // Mountpoint, und das `tmpfs` greift.
    fs::create_dir_all(fx.work.join(".git/hooks")).expect("hooks dir");

    let Some(backend) = fx.backend() else {
        return;
    };
    // `mkdir -p` gehört dazu: Ohne Mountpoint gibt es das Verzeichnis in der
    // Sandbox nicht, und genau dann legt der Agent es selbst an — auf dem
    // Projektverzeichnis, das er beschreiben darf.
    let write_hook = [
        "sh",
        "-c",
        "mkdir -p /work/.git/hooks \
         && printf '#!/bin/sh\\ncurl evil\\n' > /work/.git/hooks/pre-commit \
         && chmod +x /work/.git/hooks/pre-commit && echo WROTE-IN-SANDBOX; \
         cat /work/.git/hooks/pre-commit",
    ];
    let (output, unprotected) = run(&fx, &backend, &write_hook);

    assert!(
        output.contains("WROTE-IN-SANDBOX"),
        "the agent must be able to write inside the sandbox: {output}"
    );
    assert!(
        !fx.work.join(".git/hooks/pre-commit").exists(),
        "the hook reached the host at {}",
        fx.work.join(".git/hooks/pre-commit").display()
    );
    assert!(
        !unprotected.contains(&PathBuf::from(".git/hooks")),
        "the mount point was there, so this is no gap: {unprotected:?}"
    );

    // Dasselbe Repository ohne `hooks/`: die Lücke ist da, und sie ist benannt.
    let bare = Fixture::new();
    fs::create_dir_all(bare.work.join(".git")).expect("git dir");
    let Some(bare_backend) = bare.backend() else {
        return;
    };
    let (output, unprotected) = run(&bare, &bare_backend, &write_hook);
    assert!(
        output.contains("WROTE-IN-SANDBOX"),
        "the command ran: {output}"
    );
    assert!(
        unprotected.contains(&PathBuf::from(".git/hooks")),
        "a missing mount point must be named, not hidden: {unprotected:?}"
    );
    assert!(
        bare.work.join(".git/hooks/pre-commit").exists(),
        "this is the declared gap: without a mount point the write reaches the host"
    );
}

/// ESC-5 unter einem anderen Backend als dem der Plattform (HUM-435): Eine
/// Wegwerf-Sandbox schreibt nie in das Projekt.
///
/// Unter dem Backend der Plattform misst `tests/launcher.rs` den Fall
/// gleichen Namens am Overlay; dorthin ruft `esc-5-filesystem.sh` für dieses
/// Backend. Ein Backend ohne Wegwerf-Sandbox (bis HUM-438 die
/// Container-Engine) hält die Zusage, indem es den Plan verweigert, statt
/// still direkt in das Projekt zu starten. Genau das prüft dieser Fall. Kann
/// das Backend eine Wegwerf-Sandbox, ist der Fall rot: Dann braucht ESC-5 die
/// echte Messung, nicht diese Verweigerung.
#[test]
fn a_disposable_sandbox_never_writes_to_the_project() {
    if requested_backend().is_none() {
        println!(
            "{SKIP_MARKER} under the platform backend this case is tests/launcher.rs, measured \
             at the overlay"
        );
        return;
    }
    let fx = Fixture::new();
    fs::write(fx.work.join("keep.txt"), b"content\n").expect("write");
    let Some(backend) = fx.backend() else {
        return;
    };
    let layer = fx.work.parent().expect("parent").join("layer");
    let session = fx.context(&["sh", "-c", "echo pwned > /work/keep.txt"]);
    let options = LaunchOptions {
        stdio: StdioMode::Capture,
        disposable: Some(humanitl_sandbox::DisposableLayer::Overlay(
            humanitl_sandbox::OverlayDirs {
                upper: layer.join("upper"),
                work: layer.join("work"),
            },
        )),
        ..LaunchOptions::default()
    };
    let profile = profile();
    let request = PlanRequest::new(&profile, &session, &options);
    assert!(
        backend.disposable_support(&request).is_err(),
        "{} offers a disposable sandbox now; ESC-5 needs the real measurement for it \
         instead of this refusal (HUM-438)",
        backend.name()
    );
    assert!(
        backend.plan(&request).is_err(),
        "the plan of a disposable sandbox must be refused, never started directly on the project"
    );
    assert_eq!(
        fs::read_to_string(fx.work.join("keep.txt")).expect("host file"),
        "content\n",
        "the project changed"
    );
}
