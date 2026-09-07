//! Was `tests/cli.rs` und `tests/tty.rs` gemeinsam brauchen: eine Umgebung mit
//! eigenen XDG-Verzeichnissen, ein Fake-Daemon auf ihrem Socket und die drei
//! Leser für die Ausgabe eines Laufs.
//!
//! Eigene Datei, weil zwei Testbinaries sie brauchen und ein zweites Exemplar
//! auseinanderliefe. `dead_code` ist erlaubt: Jedes Binary bindet dieselbe
//! Datei ein und benutzt davon nur, was es braucht.

#![allow(dead_code)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths};
use humanitl_ipc::fake::{FakeDaemon, FakeOptions, Session};
use humanitl_ipc::{DaemonService, auth, bind_socket, v1};
use tempfile::TempDir;

/// Das gebaute Binary.
pub const BIN: &str = env!("CARGO_BIN_EXE_humanitl");

/// Wie lange ein Test höchstens auf einen Prozess wartet.
pub const PATIENCE: Duration = Duration::from_secs(30);

/// Eine Umgebung, in der die Kommandozeile nichts des Nutzers anfasst.
pub struct Harness {
    /// Das Verzeichnis, das beim Aufräumen alles mitnimmt.
    dir: TempDir,
}

impl Harness {
    /// Legt Heimat-, Konfigurations-, Daten- und Laufzeitverzeichnis an.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("a temporary directory");
        for sub in ["home", "config", "data", "run", "work"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect("a subdirectory");
        }
        // Die Profile liegen dort, wo humanitl sie beim Nutzer sucht, statt
        // relativ zum Binary: so laufen die Tests auch mit einem
        // CARGO_TARGET_DIR ausserhalb des Repositories.
        let profiles = dir.path().join("config/humanitl/profiles/sandbox");
        std::fs::create_dir_all(&profiles).expect("the profile directory");
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox");
        for entry in std::fs::read_dir(&shipped).expect("the shipped profiles") {
            let entry = entry.expect("a profile entry");
            if entry.path().extension().is_some_and(|ext| ext == "toml") {
                std::fs::copy(entry.path(), profiles.join(entry.file_name()))
                    .expect("a copied profile");
            }
        }
        Self { dir }
    }

    pub fn path(&self, sub: &str) -> PathBuf {
        self.dir.path().join(sub)
    }

    /// Die Pfade, die die Kommandozeile in dieser Umgebung sieht.
    pub fn paths(&self) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", self.path("home").display().to_string()),
            ("XDG_CONFIG_HOME", self.path("config").display().to_string()),
            ("XDG_DATA_HOME", self.path("data").display().to_string()),
            ("XDG_RUNTIME_DIR", self.path("run").display().to_string()),
        ]))
    }

    /// Wie [`Harness::run`], aber mit einer eigenen Uhr.
    ///
    /// `None`, wenn der Prozess die Frist überschreitet; er wird dann beendet.
    /// Ein Test, der beweisen soll, dass ein Befehl **nicht** hängt, darf nicht
    /// darauf warten, dass er von selbst zurückkommt — sonst hängt der Test
    /// mit, und die Mutation, die die Frist entfernt, überlebt, weil niemand
    /// mehr zusieht.
    pub fn run_bounded<I, S>(&self, args: I, limit: Duration) -> Option<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the binary starts");
        let deadline = Instant::now() + limit;
        loop {
            match child.try_wait().expect("the child can be polled") {
                Some(_) => break,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        Some(child.wait_with_output().expect("the output is collected"))
    }

    /// Ein Aufruf des Binaries in dieser Umgebung.
    pub fn command(&self) -> Command {
        self.command_of(BIN)
    }

    /// Wie [`Harness::command`], aber mit einem anderen Programm: für Läufe,
    /// die das Binary über einen Starter erreichen.
    pub fn command_of(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(self.path("work"))
            .env("HOME", self.path("home"))
            .env("XDG_CONFIG_HOME", self.path("config"))
            .env("XDG_DATA_HOME", self.path("data"))
            .env("XDG_RUNTIME_DIR", self.path("run"))
            .env_remove("HUMANITL_HOLD__TIMEOUT_SECS");
        command
    }

    /// Ruft das Binary auf und wartet auf sein Ende.
    pub fn run<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut command = self.command();
        for arg in args {
            command.arg(arg.into());
        }
        command.output().expect("the binary runs")
    }

    /// Legt den Proxy-Socket und die CA-Dateien an, die `sandbox run` beim
    /// Daemon erwartet.
    pub fn wire_daemon_files(&self) -> std::os::unix::net::UnixListener {
        use std::os::unix::fs::PermissionsExt as _;

        let paths = self.paths();
        std::fs::create_dir_all(paths.proxy_socket_dir()).expect("the proxy directory");
        let socket = std::os::unix::net::UnixListener::bind(paths.proxy_socket())
            .expect("the proxy socket binds");
        std::fs::set_permissions(paths.proxy_socket(), std::fs::Permissions::from_mode(0o600))
            .expect("0600 on the proxy socket");

        std::fs::create_dir_all(paths.ca_dir()).expect("the CA directory");
        for file in [paths.ca_cert_path(), paths.ca_dir().join("ca-bundle.crt")] {
            std::fs::write(file, b"").expect("a CA placeholder");
        }
        socket
    }
}

/// Ein Daemon, der eine aufgezeichnete Sitzung spielt, auf dem Socket der
/// Umgebung.
pub struct FakeServer {
    /// Beendet den Dienst beim Aufräumen.
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    /// Der Thread, der die Laufzeit trägt.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    /// Startet den Dienst und wartet, bis Socket und Token da sind.
    pub fn start(harness: &Harness) -> Self {
        let paths = harness.paths();
        let socket = paths.daemon_socket();
        let token_path = paths.token_path();
        std::fs::create_dir_all(socket.parent().expect("the socket has a directory"))
            .expect("the runtime directory");

        let session = Session::load(&fixture()).expect("the recorded session loads");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn({
            let socket = socket.clone();
            let token_path = token_path.clone();
            move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("a runtime");
                runtime.block_on(async move {
                    let token = auth::new_token().expect("a token");
                    auth::write_token(&token_path, &token).expect("the token is written");
                    let listener = bind_socket(&socket).expect("the socket binds");
                    let daemon = FakeDaemon::new(session, FakeOptions::default());
                    daemon.start();
                    let service = v1::humanitl_server::HumanitlServer::new(DaemonService::new(
                        Arc::new(daemon),
                        token,
                    ));
                    let _ = tonic::transport::Server::builder()
                        .add_service(service)
                        .serve_with_incoming_shutdown(
                            tonic::codegen::tokio_stream::wrappers::UnixListenerStream::new(
                                listener,
                            ),
                            async {
                                let _ = stopped.await;
                            },
                        )
                        .await;
                    let _ = std::fs::remove_file(&socket);
                    let _ = std::fs::remove_file(&token_path);
                });
            }
        });

        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline && !(socket.exists() && token_path.exists()) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(socket.exists(), "the fake daemon did not bind its socket");

        Self {
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Die aufgezeichnete Sitzung aus `fixtures/sessions/`.
pub fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/sessions/mixed.jsonl")
}

/// Das Sandbox-Profil des Baums.
pub fn profile_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles/sandbox")
        .join(format!("{name}.toml"))
}

/// Der gebaute Shim neben dem Binary, falls es ihn gibt.
pub fn shim() -> Option<PathBuf> {
    let path = Path::new(BIN).parent()?.join("humanitl-shim");
    path.is_file().then_some(path)
}

/// Ob `bwrap` im Pfad liegt.
pub fn bwrap_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("bwrap").is_file()))
}

/// Ob eine Sandbox in diesem Lauf überhaupt starten kann.
pub fn sandbox_available() -> Option<PathBuf> {
    let shim = shim()?;
    bwrap_available().then_some(shim)
}

/// Wie [`sandbox_available`], aber unter CI eine Forderung.
///
/// Auf einer Entwicklermaschine darf `bwrap` fehlen, und der Test endet grün,
/// statt eine Umgebung zu verlangen, die niemand versprochen hat. Auf dem
/// CI-Runner ist das Fehlen ein Fehler: `humanitl sandbox check` soll dort
/// drei grüne Zeilen zeigen (HUM-064, Akzeptanzkriterium 1), und ein Test, der
/// genau diese Zusage still überspringt, prüft sie nie.
pub fn sandbox_required() -> Option<PathBuf> {
    if let Some(shim) = sandbox_available() {
        return Some(shim);
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "under CI this test must run: {}",
        if bwrap_available() {
            "humanitl-shim is missing next to the test binary; build the workspace \
             (cargo build --workspace) before running the tests"
        } else {
            "bwrap is missing; install it (apt-get install -y bubblewrap) \
             and allow unprivileged user namespaces \
             (sysctl -w kernel.apparmor_restrict_unprivileged_userns=0)"
        }
    );
    eprintln!("skip: no bwrap or no humanitl-shim next to the binary");
    None
}

/// `stdout` als Text.
pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `stderr` als Text.
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Der Exit-Code, oder `-1` bei einem Signal.
pub fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}
