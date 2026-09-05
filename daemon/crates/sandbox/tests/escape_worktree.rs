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

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::ids::{SandboxId, SessionId};
use humanitl_sandbox::summary::SessionSummary;
use humanitl_sandbox::worktree::{SnapshotLimits, snapshot};
use humanitl_sandbox::{
    BwrapBackend, SandboxBackend, SandboxHandle, SandboxProfile, SessionContext, StdioMode,
};

/// Die Zeile, an der `esc-5-filesystem.sh` einen übersprungenen Fall erkennt.
const SKIP_MARKER: &str = "ESC5-SKIP";

/// Kein Fall wartet länger auf die Sandbox.
const WAIT: Duration = Duration::from_secs(60);

/// Ein Shim-Ersatz, der nur den Befehl startet: Diese Fälle messen die Mounts,
/// nicht den seccomp-Filter (das tun ESC-1 bis ESC-3 in der Sandbox).
const PASSTHROUGH_SHIM: &str = "#!/bin/sh\n\
                                if [ \"$1\" = \"--proxy-port\" ]; then shift 2; fi\n\
                                if [ \"$1\" = \"--\" ]; then shift; fi\n\
                                exec \"$@\"\n";

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
        fs::write(&shim, PASSTHROUGH_SHIM).expect("shim");
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod shim");

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
        }
    }

    /// Das echte `bwrap`, oder `None` samt der Zeile, die ESC-5 als `skip`
    /// liest.
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
    fn backend(&self) -> Option<BwrapBackend> {
        match BwrapBackend::detect(self.paths.clone()) {
            Ok(backend) => Some(backend.with_stdio(StdioMode::Capture)),
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

fn profile() -> SandboxProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
    SandboxProfile::load(&path)
        .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
}

/// Startet den Befehl in der Sandbox und liefert Ausgabe und die Pfade, über
/// denen in diesem Lauf kein `tmpfs` lag.
fn run(fx: &Fixture, backend: &BwrapBackend, command: &[&str]) -> (String, Vec<PathBuf>) {
    let session = fx.context(command);
    let plan = backend
        .plan(&profile(), &session)
        .unwrap_or_else(|err| panic!("the plan fails: {}", err.why));
    let unprotected = plan.unprotected.clone();
    let handle: SandboxHandle = backend
        .launch(&plan)
        .unwrap_or_else(|err| panic!("bwrap does not start: {}", err.why));
    handle
        .wait_timeout(WAIT)
        .expect("the sandbox ends within the timeout")
        .unwrap_or_else(|err| panic!("bwrap failed before the command ran: {}", err.why));
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
