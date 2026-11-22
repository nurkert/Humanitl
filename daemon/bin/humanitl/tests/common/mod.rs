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
use humanitl_core::{Diagnostic, FlowId};
use humanitl_ipc::fake::{FakeDaemon, FakeOptions, Session};
use humanitl_ipc::{BoxStream, DaemonApi, DaemonService, auth, bind_socket, v1};
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
        // `system-units` steht für `/usr/lib/systemd/user` und bleibt leer:
        // Ein installiertes Paket auf dem Rechner des Tests lenkte
        // `daemon install` sonst auf den Weg des Pakets (HUM-077).
        for sub in ["home", "config", "data", "run", "work", "system-units"] {
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
            .env("HUMANITL_SYSTEM_UNIT_DIR", self.path("system-units"))
            // Ohne Bus der Sitzung erreicht kein `systemctl --user` dieses
            // Laufs den systemd des Menschen, auch dort nicht, wo ein Test
            // sich das `systemctl` aus dem `PATH` des Rechners holt.
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            // Ein gesetztes `APPIMAGE` des Rechners ändert, welche Units
            // zählen (HUM-165); die Tests setzen es selbst, wo sie es brauchen.
            .env_remove("APPIMAGE")
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
    /// Die Art jeder Nachricht, die über `Terminal` ankam, in ihrer
    /// Reihenfolge (HUM-279).
    terminal_inputs: TerminalInputs,
    /// Jeder `Sandbox(Start)`, der ankam, und die Zahl der
    /// `Projects.save_choice` (#8).
    record: Record,
}

/// Was der Fake über Starts und die Projektwahl mitschreibt (#8): jeden
/// `Sandbox(Start)` in seiner Reihenfolge und jedes `Projects.save_choice`.
#[derive(Debug, Default)]
pub struct Recorded {
    /// Die Starts, wie sie ankamen.
    pub starts: Vec<v1::sandbox_request::Start>,
    /// Wie oft `Projects.save_choice` kam.
    pub saved_choices: usize,
    /// Die Ordner, deren Wahl `Projects.save_choice` gespeichert hat (#9):
    /// Danach kennt der Fake sie als Projekte, wie das Register.
    pub saved_paths: Vec<PathBuf>,
}

type Record = Arc<std::sync::Mutex<Recorded>>;

/// Wie der Fake vom Standard abweicht.
#[derive(Default)]
struct Tweaks {
    silent_export: bool,
    version: Option<String>,
    stall_terminal: bool,
    idle: bool,
    proto_minor: Option<u32>,
    cleanups: Option<Cleanups>,
    known_project: Option<PathBuf>,
    first_start: FirstStartTweaks,
}

/// Wie der Fake den Erststart von `go` behandelt (#9).
#[derive(Clone, Copy, Default)]
pub struct FirstStartTweaks {
    /// `Projects.proposal` scheitert mit `CONFIG_003`.
    pub refuse_proposal: bool,
    /// `Projects.save_choice` antwortet erst nach dieser Zeit.
    pub slow_save: Option<Duration>,
    /// `Projects.save_choice` scheitert mit `CONFIG_024`.
    pub refuse_save: bool,
    /// `Sandbox(CheckCloudInference)` meldet eine unbestätigte
    /// Cloud-Inferenz: `go` stellt vor dem Start die Frage von HUM-516.
    pub cloud: bool,
}

/// Was der Fake über `Terminal` empfangen hat: `open`, `data`, `resize` oder
/// `close` je Nachricht.
///
/// Der Fake beantwortet ein `close` wie das Ende des Stroms; von außen ist
/// beides ein `Exit 0`. Ob ein Client wirklich `close` geschickt hat, steht
/// deshalb nur hier.
type TerminalInputs = Arc<std::sync::Mutex<Vec<&'static str>>>;

impl FakeServer {
    /// Startet den Dienst und wartet, bis Socket und Token da sind.
    pub fn start(harness: &Harness) -> Self {
        Self::start_with(harness, Tweaks::default())
    }

    /// Wie [`FakeServer::start`], aber das Register kennt `project` als
    /// Projekt mit einer gespeicherten Wahl (#8): `Projects.get` antwortet
    /// für diesen Ordner, statt ihn als unbekannt abzulehnen.
    pub fn start_with_known_project(harness: &Harness, project: &Path) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                known_project: Some(project.to_path_buf()),
                ..Tweaks::default()
            },
        )
    }

    /// Wie [`FakeServer::start`], mit einem Erststart, der sich anders
    /// verhält ([`FirstStartTweaks`], #9).
    pub fn start_with_first_start(harness: &Harness, first_start: FirstStartTweaks) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                first_start,
                ..Tweaks::default()
            },
        )
    }

    /// Wie [`FakeServer::start`], aber `Projects.cleanup_plan` liefert einen
    /// Plan mit Einträgen, und der Zähler sagt, wie oft `cleanup` kam
    /// (HUM-533).
    pub fn start_with_cleanup(harness: &Harness) -> (Self, Cleanups) {
        let cleanups = Cleanups::default();
        let server = Self::start_with(
            harness,
            Tweaks {
                cleanups: Some(Arc::clone(&cleanups)),
                ..Tweaks::default()
            },
        );
        (server, cleanups)
    }

    /// Wie [`FakeServer::start`], aber `GetInfo` nennt `version` als Fassung
    /// des Daemons (HUM-210): ein älterer Daemon, der nach einem Update noch
    /// antwortet.
    pub fn start_with_version(harness: &Harness, version: &str) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                version: Some(version.to_owned()),
                ..Tweaks::default()
            },
        )
    }

    /// Wie [`FakeServer::start`], aber `GetInfo` nennt diese Minor des
    /// Vertrags: ein älterer Daemon (HUM-436).
    pub fn start_with_proto_minor(harness: &Harness, minor: u32) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                proto_minor: Some(minor),
                ..Tweaks::default()
            },
        )
    }

    /// Wie [`FakeServer::start`], aber ohne Sitzung: `retire_if_idle` sagt
    /// `retired` (HUM-382). Der Standard-Fake spielt eine laufende Sitzung.
    pub fn start_idle(harness: &Harness) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                idle: true,
                ..Tweaks::default()
            },
        )
    }

    /// Wie [`FakeServer::start`], aber `Audit(Export)` meldet Erfolg und
    /// schreibt nichts (HUM-156): ein Daemon, der in eine Sicht der Dateien
    /// schreibt, die der Aufrufer nicht hat, etwa ein eigenes `/tmp` unter
    /// `PrivateTmp`.
    pub fn start_with_silent_export(harness: &Harness) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                silent_export: true,
                ..Tweaks::default()
            },
        )
    }

    /// Wie [`FakeServer::start`], aber `Terminal` liest nach dem `Open` keine
    /// Eingabe mehr (HUM-279): eine Sitzung, deren Agent nicht liest. Der
    /// Strom bleibt offen, bis der Client geht, und alles, was er schickt,
    /// staut sich im Transport.
    pub fn start_with_stalled_terminal(harness: &Harness) -> Self {
        Self::start_with(
            harness,
            Tweaks {
                stall_terminal: true,
                ..Tweaks::default()
            },
        )
    }

    fn start_with(harness: &Harness, tweaks: Tweaks) -> Self {
        let paths = harness.paths();
        let socket = paths.daemon_socket();
        let token_path = paths.token_path();
        // Wie der Daemon: das Laufzeitverzeichnis ist 0700, sonst traut der
        // Client dem Token darin nicht (HUM-212).
        humanitl_config::private_dir::ensure_private_dir(
            socket.parent().expect("the socket has a directory"),
            humanitl_config::private_dir::process_uid(),
        )
        .expect("the runtime directory");

        let session = Session::load(&fixture()).expect("the recorded session loads");
        let terminal_inputs = TerminalInputs::default();
        let recorded = Arc::clone(&terminal_inputs);
        let record = Record::default();
        let record_of_service = Arc::clone(&record);
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
                        Arc::new(SilentExport {
                            inner: daemon,
                            silent: tweaks.silent_export,
                            version: tweaks.version,
                            terminal_inputs: recorded,
                            stall_terminal: tweaks.stall_terminal,
                            idle: tweaks.idle,
                            proto_minor: tweaks.proto_minor,
                            cleanups: tweaks.cleanups,
                            record: record_of_service,
                            known_project: tweaks.known_project,
                            first_start: tweaks.first_start,
                        }),
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
            terminal_inputs,
            record,
        }
    }

    /// Die Starts, die bisher ankamen (#8).
    pub fn starts(&self) -> Vec<v1::sandbox_request::Start> {
        self.record
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .starts
            .clone()
    }

    /// Wie oft `Projects.save_choice` bisher ankam (#8).
    pub fn saved_choices(&self) -> usize {
        self.record
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .saved_choices
    }

    /// Die Ordner, deren Wahl `Projects.save_choice` bisher gespeichert hat
    /// (#9).
    pub fn saved_paths(&self) -> Vec<PathBuf> {
        self.record
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .saved_paths
            .clone()
    }

    /// Die Arten der Nachrichten, die bisher über `Terminal` ankamen.
    pub fn terminal_inputs(&self) -> Vec<&'static str> {
        self.terminal_inputs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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

/// Ein Daemon ohne Proxy, der die Kette der Umgebung über `Audit` beantwortet
/// (HUM-156): derselbe `IpcServer` wie in `humanitld`, mit einem festen
/// Schlüssel und den Ankern aus der Datenbank der Umgebung.
pub struct AuditServer {
    /// Beendet den Dienst beim Aufräumen.
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    /// Der Thread, der die Laufzeit trägt.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl AuditServer {
    /// Startet den Dienst mit dem Schlüssel `key` und wartet auf den Socket.
    pub fn start(harness: &Harness, key: [u8; 32]) -> Self {
        let paths = harness.paths();
        let socket = paths.daemon_socket();
        let token_path = paths.token_path();
        // Wie der Daemon: das Laufzeitverzeichnis ist 0700, sonst traut der
        // Client dem Token darin nicht (HUM-212).
        humanitl_config::private_dir::ensure_private_dir(
            socket.parent().expect("the socket has a directory"),
            humanitl_config::private_dir::process_uid(),
        )
        .expect("the runtime directory");
        let service = humanitl_ipc::AuditService::new(
            paths.audit_path(),
            paths.db_path(),
            Arc::new(humanitl_audit::AuditKey::from_bytes(
                key,
                humanitl_audit::KeyOrigin::File,
            )),
        );
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn({
            let socket = socket.clone();
            let token_path = token_path.clone();
            move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("a runtime");
                runtime.block_on(async move {
                    let server = humanitl_ipc::IpcServer::over_the_recording(
                        &humanitl_config::Config::default(),
                        None,
                    )
                    .with_audit_log(service);
                    let _ = humanitl_ipc::serve(&socket, &token_path, server, async {
                        let _ = stopped.await;
                    })
                    .await;
                });
            }
        });

        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline && !(socket.exists() && token_path.exists()) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(socket.exists(), "the audit daemon did not bind its socket");
        Self {
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}

impl Drop for AuditServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Der Fake, bei Bedarf mit einem Export, der Erfolg meldet und nichts
/// schreibt, und mit einer eigenen Fassung in `GetInfo`. Jede andere RPC geht
/// unverändert an den Fake.
struct SilentExport {
    inner: FakeDaemon,
    silent: bool,
    /// Die Fassung, die `GetInfo` statt der des Fakes nennt.
    version: Option<String>,
    /// Wohin die Arten der Terminal-Nachrichten geschrieben werden.
    terminal_inputs: TerminalInputs,
    /// Wahr, wenn `Terminal` nach dem `Open` nichts mehr liest.
    stall_terminal: bool,
    /// Wahr, wenn `retire_if_idle` mit „ich ende" antwortet: ein Daemon ohne
    /// Sitzung (HUM-382). Der Fake läuft danach weiter, wie ein Daemon, den
    /// systemd sofort wieder startet.
    idle: bool,
    /// Die Minor, die `GetInfo` statt der eigenen nennt (HUM-436).
    proto_minor: Option<u32>,
    /// Mit einem Zähler: `Projects` antwortet auf `cleanup_plan` mit einem
    /// Plan mit Einträgen ([`cleanup_plan`]) und zählt jedes `cleanup`
    /// (HUM-533). Der Fake selbst plant nichts, weil seine Sitzung läuft.
    cleanups: Option<Cleanups>,
    /// Starts und `save_choice` (#8).
    record: Record,
    /// Ein Ordner, den `Projects.get` als Projekt mit gespeicherter Wahl
    /// kennt (#8).
    known_project: Option<PathBuf>,
    /// Abweichungen beim Erststart (#9).
    first_start: FirstStartTweaks,
}

impl SilentExport {
    /// Ob das Register des Fakes `path` kennt: der bekannte Ordner oder einer,
    /// dessen Wahl gespeichert wurde (#9).
    fn knows(&self, path: &Path) -> bool {
        self.known_project.as_deref() == Some(path)
            || self
                .record
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .saved_paths
                .iter()
                .any(|saved| saved == path)
    }
}

/// Ein Start, der die Kernel-Stufe verlangt, endet im Fake wie auf einem
/// Rechner ohne sie (#8): mit dem Befund der Stufe und ohne Start auf einer
/// schwächeren.
fn asks_for_the_kernel_level(start: &v1::sandbox_request::Start) -> bool {
    start
        .cli_overrides
        .iter()
        .any(|pair| pair.path == "sandbox.min_isolation" && pair.value == "kernel")
}

/// Der Befund des Fakes für die Kernel-Stufe, die er nicht hat.
fn no_kernel_level() -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Diagnostic(
            humanitl_ipc::diagnostic_to_proto(
                &Diagnostic::builder(
                    humanitl_core::diagnostics::codes::SANDBOX_045,
                    humanitl_core::Severity::Blocking,
                )
                .why("the fake daemon has no kernel level".to_owned())
                .build(),
            ),
        )),
    }
}

/// `Projects.get` für den bekannten Ordner: ein Eintrag und die Wahl
/// `agent.adapter = "claude"`.
fn known_project_answer(path: &Path) -> v1::ProjectsResponse {
    v1::ProjectsResponse {
        project: Some(v1::ProjectEntry {
            project_id: "0190a6d4-0000-7000-8000-000000000008".to_owned(),
            path: path.display().to_string(),
            ..v1::ProjectEntry::default()
        }),
        choice: vec![v1::ProjectChoiceField {
            key: "agent.adapter".to_owned(),
            chosen: "claude".to_owned(),
            effective: "claude".to_owned(),
            origin: "project-choice".to_owned(),
        }],
        ..v1::ProjectsResponse::default()
    }
}

/// Wie oft `Projects.cleanup` ankam (HUM-533).
pub type Cleanups = Arc<std::sync::atomic::AtomicUsize>;

/// Ein Plan mit je einem Eintrag jeder Art, die wartenden Änderungen mit der
/// Warnung aus dem Daemon.
fn cleanup_plan(selection: Option<v1::CleanupSelection>) -> v1::CleanupPlanView {
    let item = |kind: &str, description: &str, warning: &str| v1::CleanupItem {
        kind: kind.to_owned(),
        project_id: "0190a6d4-0000-7000-8000-000000000001".to_owned(),
        path: "/home/nik/projects/shop-frontend".to_owned(),
        description: description.to_owned(),
        bytes: 2048,
        count: 1,
        warning: warning.to_owned(),
    };
    v1::CleanupPlanView {
        items: vec![
            item(
                "recorded_flows",
                "1 recorded request with responses and bodies from 1 session of shop-frontend",
                "",
            ),
            item(
                "waiting_changes",
                "1 change of the disposable run in shop-frontend that waits",
                humanitl_ipc::projects::WAITING_WARNING,
            ),
            item("register_entry", "The register entry of shop-frontend", ""),
        ],
        digest: "ab".repeat(32),
        not_deleted: humanitl_ipc::projects::NOT_DELETED.to_owned(),
        selection,
        kept: Vec::new(),
        kinds: vec![
            "recorded_flows".to_owned(),
            "waiting_changes".to_owned(),
            "project_choice".to_owned(),
            "register_entry".to_owned(),
        ],
    }
}

#[tonic::async_trait]
impl DaemonApi for SilentExport {
    async fn info(&self) -> v1::Info {
        let mut info = self.inner.info().await;
        if let Some(version) = &self.version {
            info.daemon_version.clone_from(version);
        }
        if let Some(minor) = self.proto_minor {
            info.proto_minor = minor;
        }
        info
    }
    fn subscribe(&self, request: v1::SubscribeRequest) -> BoxStream<v1::FlowEvent> {
        self.inner.subscribe(request)
    }
    async fn list_flows(&self, request: v1::ListFlowsRequest) -> Result<v1::FlowPage, Diagnostic> {
        self.inner.list_flows(request).await
    }
    async fn get_flow(&self, id: FlowId) -> Result<v1::FlowDetail, Diagnostic> {
        self.inner.get_flow(id).await
    }
    fn get_body(&self, body: v1::BodyRef) -> Result<BoxStream<v1::BodyChunk>, Diagnostic> {
        self.inner.get_body(body)
    }
    async fn decide(&self, request: v1::DecideRequest) -> Result<v1::DecideResponse, Diagnostic> {
        self.inner.decide(request).await
    }
    async fn rules(&self, request: v1::RulesRequest) -> Result<v1::RulesResponse, Diagnostic> {
        self.inner.rules(request).await
    }
    fn sandbox(&self, request: v1::SandboxRequest) -> BoxStream<v1::SandboxEvent> {
        if self.first_start.cloud
            && matches!(
                request.op,
                Some(v1::sandbox_request::Op::CheckCloudInference(_))
            )
        {
            let check = v1::SandboxEvent {
                event: Some(v1::sandbox_event::Event::CloudInferenceCheck(
                    v1::sandbox_event::CloudInferenceCheck {
                        cloud_inference: Some(v1::CloudInference {
                            agent: "opencode".to_owned(),
                            provider: "api.example.com".to_owned(),
                            acknowledged: false,
                            notice: None,
                        }),
                    },
                )),
            };
            return Box::pin(tonic::codegen::tokio_stream::iter(vec![check]));
        }
        if let Some(v1::sandbox_request::Op::Start(start)) = &request.op {
            self.record
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .starts
                .push(start.clone());
            if asks_for_the_kernel_level(start) {
                return Box::pin(tonic::codegen::tokio_stream::iter(vec![no_kernel_level()]));
            }
        }
        if self.idle && matches!(request.op, Some(v1::sandbox_request::Op::RetireIfIdle(()))) {
            return Box::pin(tonic::codegen::tokio_stream::iter(vec![v1::SandboxEvent {
                event: Some(v1::sandbox_event::Event::Retire(
                    v1::sandbox_event::Retire { retired: true },
                )),
            }]));
        }
        self.inner.sandbox(request)
    }
    fn terminal(&self, input: BoxStream<v1::TerminalInput>) -> BoxStream<v1::TerminalOutput> {
        use tokio_stream::StreamExt as _;
        use v1::terminal_input::Input;

        let recorded = Arc::clone(&self.terminal_inputs);
        if self.stall_terminal {
            // Das `Open` lesen, eine Zeile schicken, dann die Eingabe halten,
            // ohne sie je wieder zu lesen, bis der Client geht.
            let (out, rx) = tokio::sync::mpsc::channel(4);
            tokio::spawn(async move {
                let mut input = input;
                if input.next().await.is_some() {
                    recorded
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push("open");
                }
                let greeting = v1::TerminalOutput {
                    output: Some(v1::terminal_output::Output::Data(
                        b"humanitl fake terminal: input is never read\r\n".to_vec(),
                    )),
                };
                if out.send(greeting).await.is_ok() {
                    out.closed().await;
                }
                drop(input);
            });
            return Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx));
        }
        let input = input.map(move |message| {
            let kind = match &message.input {
                // The terminal of a further shell (HUM-299), and only the
                // one `exec` below hands out.
                Some(Input::Open(open)) if open.shell_id == "1" => "open-shell-1",
                Some(Input::Open(open)) if !open.shell_id.is_empty() => "open-shell-other",
                Some(Input::Open(_)) => "open",
                Some(Input::Data(_)) => "data",
                Some(Input::Resize(_)) => "resize",
                Some(Input::Close(())) => "close",
                None => "empty",
            };
            recorded
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(kind);
            message
        });
        self.inner.terminal(Box::pin(input))
    }
    /// One further shell, always id 1 (HUM-299); the command is recorded as
    /// the kind `exec` so a test sees that the RPC came before the terminal.
    async fn exec(&self, request: v1::ExecRequest) -> Result<v1::ExecResponse, Diagnostic> {
        self.terminal_inputs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(if request.command.is_empty() {
                "exec-default"
            } else {
                "exec-command"
            });
        Ok(v1::ExecResponse {
            shell_id: "1".to_owned(),
            command: request.command,
            sandbox_id: "fake".to_owned(),
        })
    }
    async fn audit(&self, request: v1::AuditRequest) -> Result<v1::AuditResponse, Diagnostic> {
        match request.op.as_ref() {
            Some(v1::audit_request::Op::Export(export)) if self.silent => Ok(v1::AuditResponse {
                ok: true,
                entries: 3,
                out_path: export.out_path.clone(),
                ..v1::AuditResponse::default()
            }),
            _ => self.inner.audit(request).await,
        }
    }
    async fn get_config(
        &self,
        request: v1::GetConfigRequest,
    ) -> Result<v1::ConfigSnapshot, Diagnostic> {
        self.inner.get_config(request).await
    }
    async fn projects(
        &self,
        request: v1::ProjectsRequest,
    ) -> Result<v1::ProjectsResponse, Diagnostic> {
        match &request.op {
            Some(v1::projects_request::Op::Proposal(_)) if self.first_start.refuse_proposal => {
                return Err(Diagnostic::builder(
                    humanitl_core::diagnostics::codes::CONFIG_003,
                    humanitl_core::Severity::Error,
                )
                .why("the fake daemon refuses every start proposal".to_owned())
                .build());
            }
            Some(v1::projects_request::Op::SaveChoice(_)) if self.first_start.refuse_save => {
                return Err(Diagnostic::builder(
                    humanitl_core::diagnostics::codes::CONFIG_024,
                    humanitl_core::Severity::Error,
                )
                .why("the fake daemon refuses every project choice".to_owned())
                .build());
            }
            Some(v1::projects_request::Op::SaveChoice(_))
                if self.first_start.slow_save.is_some() =>
            {
                if let Some(wait) = self.first_start.slow_save {
                    tokio::time::sleep(wait).await;
                }
            }
            _ => {}
        }
        match &request.op {
            Some(v1::projects_request::Op::SaveChoice(save)) => {
                let mut record = self
                    .record
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                record.saved_choices += 1;
                // Am Ordner gespeichert (#9): Der Fake legt den Eintrag an wie
                // das Register und kennt den Ordner danach.
                if !save.path.is_empty() {
                    record.saved_paths.push(PathBuf::from(&save.path));
                    return Ok(known_project_answer(Path::new(&save.path)));
                }
            }
            Some(v1::projects_request::Op::Get(get)) if self.knows(Path::new(&get.path)) => {
                return Ok(known_project_answer(Path::new(&get.path)));
            }
            // Ein bekannter Ordner bekommt den Vorschlag mit seiner Kennung,
            // wie im Daemon; `go` zeigt ihm keinen Erststart (#9).
            Some(v1::projects_request::Op::Proposal(asked))
                if asked.project_id.is_empty() && self.knows(Path::new(&asked.path)) =>
            {
                let mut answer = self.inner.projects(request.clone()).await?;
                if let Some(proposal) = answer.proposal.as_mut() {
                    proposal.project_id = known_project_answer(Path::new(&asked.path))
                        .project
                        .map(|project| project.project_id)
                        .unwrap_or_default();
                    proposal.has_choice = true;
                }
                return Ok(answer);
            }
            _ => {}
        }
        if let Some(cleanups) = &self.cleanups {
            match request.op {
                Some(v1::projects_request::Op::CleanupPlan(asked)) => {
                    return Ok(v1::ProjectsResponse {
                        plan: Some(cleanup_plan(asked.selection)),
                        ..v1::ProjectsResponse::default()
                    });
                }
                Some(v1::projects_request::Op::Cleanup(_)) => {
                    cleanups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    return Ok(v1::ProjectsResponse {
                        cleaned: Some(v1::CleanupResult {
                            outcomes: vec![v1::CleanupOutcome {
                                kind: "recorded_flows".to_owned(),
                                path: "/home/nik/projects/shop-frontend".to_owned(),
                                deleted: true,
                                count: 1,
                                bytes_freed: 2048,
                                ..v1::CleanupOutcome::default()
                            }],
                            bytes_freed: 2048,
                            not_deleted: humanitl_ipc::projects::NOT_DELETED.to_owned(),
                        }),
                        ..v1::ProjectsResponse::default()
                    });
                }
                _ => {}
            }
        }
        self.inner.projects(request).await
    }
    async fn set_config(
        &self,
        request: v1::SetConfigRequest,
    ) -> Result<v1::ConfigSnapshot, Diagnostic> {
        self.inner.set_config(request).await
    }
    async fn doctor(&self) -> v1::DoctorReport {
        self.inner.doctor().await
    }
    fn discover_llm(&self, request: v1::DiscoverRequest) -> BoxStream<v1::DiscoverResult> {
        self.inner.discover_llm(request)
    }
    async fn probe_llm(
        &self,
        request: v1::ProbeLlmRequest,
    ) -> Result<v1::ProbeLlmResponse, Diagnostic> {
        self.inner.probe_llm(request).await
    }
    async fn get_session_summary(
        &self,
        request: v1::SessionSummaryRef,
    ) -> Result<v1::SessionSummary, Diagnostic> {
        self.inner.get_session_summary(request).await
    }
    async fn llm_key(&self, request: v1::LlmKeyRequest) -> Result<v1::LlmKeyStatus, Diagnostic> {
        self.inner.llm_key(request).await
    }
    async fn llm_servers(
        &self,
        request: v1::LlmServersRequest,
    ) -> Result<v1::LlmServerList, Diagnostic> {
        self.inner.llm_servers(request).await
    }
    async fn intercept(
        &self,
        request: v1::InterceptRequest,
    ) -> Result<v1::InterceptStatus, Diagnostic> {
        self.inner.intercept(request).await
    }

    async fn sandbox_backends(
        &self,
        request: v1::SandboxBackendsRequest,
    ) -> Result<v1::SandboxBackendList, Diagnostic> {
        self.inner.sandbox_backends(request).await
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

/// Die Daten, die im `AppImage` neben `bin/` liegen: `domains.yaml` des
/// Katalogs unter `share/humanitl/catalog/` und das Profil `default` unter
/// `profiles/sandbox/` (`packaging/appimage/build-appimage.sh`, HUM-165).
///
/// `lib` ist `usr/lib/humanitl` des Baums, das Verzeichnis über `bin/`.
pub fn image_data(lib: &Path) {
    let catalog = lib.join("share/humanitl/catalog");
    let profiles = lib.join("profiles/sandbox");
    for dir in [&catalog, &profiles] {
        std::fs::create_dir_all(dir).expect("a data directory in the image");
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    std::fs::copy(
        repo.join("catalog/domains.yaml"),
        catalog.join("domains.yaml"),
    )
    .expect("the catalog is copied");
    std::fs::copy(profile_file("default"), profiles.join("default.toml"))
        .expect("the profile is copied");
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
