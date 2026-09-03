//! Die Kommandozeile, wie ein Nutzer sie aufruft: das gebaute Binary in einem
//! eigenen Prozess, mit eigenen XDG-Verzeichnissen.
//!
//! Geprüft wird, was ein Unit-Test nicht sieht: der Exit-Code, die Trennung
//! von `stdout` und `stderr`, der Befund als Block und als JSON, und die
//! Präzedenz zwischen Umgebung und Flag. Wo eine Sandbox wirklich startet,
//! braucht der Test `bwrap` und einen gebauten `humanitl-shim`; fehlt eines
//! von beiden, meldet der Test das und endet grün, statt eine Umgebung zu
//! verlangen, die eine Entwicklermaschine nicht haben muss (dieselbe Regel wie
//! in `tests/escape/`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::ids::SessionId;
use humanitl_ipc::fake::{FakeDaemon, FakeOptions, Session};
use humanitl_ipc::{DaemonService, auth, bind_socket, v1};
use humanitl_sandbox::{LaunchInputs, SandboxProfile, SessionContext};
use tempfile::TempDir;

/// Das gebaute Binary.
const BIN: &str = env!("CARGO_BIN_EXE_humanitl");

/// Wie lange ein Test höchstens auf einen Prozess wartet.
const PATIENCE: Duration = Duration::from_secs(30);

/// Eine Umgebung, in der die Kommandozeile nichts des Nutzers anfasst.
struct Harness {
    /// Das Verzeichnis, das beim Aufräumen alles mitnimmt.
    dir: TempDir,
}

impl Harness {
    /// Legt Heimat-, Konfigurations-, Daten- und Laufzeitverzeichnis an.
    fn new() -> Self {
        let dir = TempDir::new().expect("a temporary directory");
        for sub in ["home", "config", "data", "run", "work"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect("a subdirectory");
        }
        Self { dir }
    }

    fn path(&self, sub: &str) -> PathBuf {
        self.dir.path().join(sub)
    }

    /// Die Pfade, die die Kommandozeile in dieser Umgebung sieht.
    fn paths(&self) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", self.path("home").display().to_string()),
            ("XDG_CONFIG_HOME", self.path("config").display().to_string()),
            ("XDG_DATA_HOME", self.path("data").display().to_string()),
            ("XDG_RUNTIME_DIR", self.path("run").display().to_string()),
        ]))
    }

    /// Ein Aufruf des Binaries in dieser Umgebung.
    fn command(&self) -> Command {
        let mut command = Command::new(BIN);
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
    fn run<I, S>(&self, args: I) -> Output
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
    fn wire_daemon_files(&self) -> std::os::unix::net::UnixListener {
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
struct FakeServer {
    /// Beendet den Dienst beim Aufräumen.
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    /// Der Thread, der die Laufzeit trägt.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    /// Startet den Dienst und wartet, bis Socket und Token da sind.
    fn start(harness: &Harness) -> Self {
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
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/sessions/mixed.jsonl")
}

/// Das Sandbox-Profil des Baums.
fn profile_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles/sandbox")
        .join(format!("{name}.toml"))
}

/// Der gebaute Shim neben dem Binary, falls es ihn gibt.
fn shim() -> Option<PathBuf> {
    let path = Path::new(BIN).parent()?.join("humanitl-shim");
    path.is_file().then_some(path)
}

/// Ob eine Sandbox in diesem Lauf überhaupt starten kann.
fn sandbox_available() -> Option<PathBuf> {
    let shim = shim()?;
    let found = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("bwrap").is_file()));
    found.then_some(shim)
}

/// `stdout` als Text.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `stderr` als Text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Der Exit-Code, oder `-1` bei einem Signal.
fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

#[test]
fn the_help_documents_the_exit_codes_and_every_config_key() {
    let harness = Harness::new();
    let output = harness.run(["--help"]);
    let text = stdout(&output);

    assert_eq!(code(&output), 0);
    assert!(text.contains("Exit codes:"), "{text}");
    for line in [
        "0   the command did what it says",
        "1   user error",
        "2   the daemon is not reachable",
        "3   a sandbox isolation check failed",
    ] {
        assert!(text.contains(line), "{line} is missing from --help");
    }
    for flag in [
        "--llm-endpoint",
        "--hold-timeout-secs",
        "--hold-ask-mode",
        "--sandbox-profile",
        "--sandbox-work-dir",
        "--sandbox-work-mode",
        "--agent-adapter",
        "--agent-command",
        "--recorder-retention-days",
        "--ui-language",
        "--ui-theme",
    ] {
        assert!(text.contains(flag), "{flag} is missing from --help");
    }
}

#[test]
fn every_subcommand_has_a_help_of_its_own() {
    let harness = Harness::new();
    for command in [
        vec!["sandbox", "--help"],
        vec!["sandbox", "run", "--help"],
        vec!["sandbox", "argv", "--help"],
        vec!["sandbox", "check", "--help"],
        vec!["flows", "--help"],
        vec!["flows", "list", "--help"],
        vec!["flows", "show", "--help"],
        vec!["config", "--help"],
        vec!["config", "get", "--help"],
        vec!["config", "schema", "--help"],
        vec!["daemon", "--help"],
        vec!["daemon", "status", "--help"],
    ] {
        let output = harness.run(command.clone());
        assert_eq!(code(&output), 0, "{command:?} has no help");
        assert!(!stdout(&output).is_empty(), "{command:?} printed nothing");
    }
}

#[test]
fn a_missing_daemon_is_daemon_001_on_stderr_and_exit_two() {
    let harness = Harness::new();
    let output = harness.run(["daemon", "status"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 2, "stderr was {text}");
    assert!(text.starts_with("blocking[DAEMON_001]: "), "{text}");
    assert!(text.contains("\n  why: "), "{text}");
    assert!(text.contains("\n  fix: humanitld\n"), "{text}");
    assert!(text.contains("\n  docs: https://"), "{text}");
    assert!(stdout(&output).is_empty(), "stdout must stay clean");
}

#[test]
fn a_missing_daemon_with_json_is_one_line_on_stdout() {
    let harness = Harness::new();
    let output = harness.run(["--json", "daemon", "status"]);
    let text = stdout(&output);

    assert_eq!(code(&output), 2);
    assert_eq!(text.lines().count(), 1, "{text}");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("one JSON value");
    assert_eq!(value["code"], "DAEMON_001");
    assert_eq!(value["severity"], "blocking");
    assert!(value["why"].as_str().is_some_and(|why| !why.is_empty()));
    assert!(stderr(&output).is_empty(), "stderr must stay clean");
}

#[test]
fn the_command_line_wins_over_the_environment() {
    let harness = Harness::new();

    let from_env = harness
        .command()
        .env("HUMANITL_HOLD__TIMEOUT_SECS", "7")
        .args(["config", "get", "hold.timeout_secs"])
        .output()
        .expect("the binary runs");
    assert_eq!(code(&from_env), 0, "{}", stderr(&from_env));
    assert_eq!(stdout(&from_env).trim(), "7");

    let from_flag = harness
        .command()
        .env("HUMANITL_HOLD__TIMEOUT_SECS", "7")
        .args([
            "--hold-timeout-secs",
            "9",
            "config",
            "get",
            "hold.timeout_secs",
        ])
        .output()
        .expect("the binary runs");
    assert_eq!(code(&from_flag), 0, "{}", stderr(&from_flag));
    assert_eq!(stdout(&from_flag).trim(), "9");
}

#[test]
fn an_unknown_config_key_is_config_002_and_exit_one() {
    let harness = Harness::new();
    let output = harness.run(["config", "get", "hold.nonsense"]);

    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).starts_with("error[CONFIG_002]: "),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_value_out_of_range_is_a_diagnostic_not_a_panic() {
    let harness = Harness::new();
    let output = harness.run([
        "--hold-timeout-secs",
        "0",
        "config",
        "get",
        "hold.timeout_secs",
    ]);

    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("[CONFIG_003]"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_schema_is_json_and_names_every_key_of_conventions_37() {
    let harness = Harness::new();
    let output = harness.run(["config", "schema"]);

    assert_eq!(code(&output), 0);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    let properties = &value["properties"];
    for group in [
        "llm",
        "hold",
        "sandbox",
        "agent",
        "recorder",
        "ui",
        "experimental",
    ] {
        assert!(!properties[group].is_null(), "{group} is missing");
    }
}

#[test]
fn a_subcommand_that_does_not_exist_yet_names_its_issue_and_exits_one() {
    let harness = Harness::new();
    for (command, issue) in [
        (vec!["run", "--", "opencode"], "HUM-067"),
        (vec!["rules", "list"], "HUM-065"),
        (vec!["audit", "verify"], "HUM-070"),
    ] {
        let output = harness.run(command.clone());
        assert_eq!(code(&output), 1, "{command:?}");
        assert!(
            stderr(&output).contains(issue),
            "{command:?} does not name {issue}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn an_unknown_flag_is_exit_one_not_the_two_of_clap() {
    let harness = Harness::new();
    let output = harness.run(["--nonsense", "daemon", "status"]);

    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("--nonsense"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn sandbox_argv_is_the_translation_of_the_profile() {
    let harness = Harness::new();
    let output = harness.run(["sandbox", "argv", "--", "sh", "-c", "echo hello world"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    let printed = stdout(&output);
    let parts = shlex::split(printed.trim()).expect("the line is one shell command");
    assert!(
        parts[0].ends_with("bwrap"),
        "the line starts with {}",
        parts[0]
    );

    // Die Sitzung ist bei jedem Aufruf neu; der Rest muss Argument für
    // Argument dasselbe sein wie die Übersetzung des Profils.
    let marker = parts
        .iter()
        .position(|part| part == "HUMANITL_SESSION")
        .expect("the line carries the session");
    let session = SessionId::parse(&parts[marker + 1]).expect("a session id");

    let paths = harness.paths();
    let context = SessionContext {
        session,
        work_src: harness.path("work"),
        work_mode: WorkMode::Rw,
        proxy_socket_src: paths.proxy_socket(),
        ca_cert_src: paths.ca_cert_path(),
        ca_bundle_src: paths.ca_dir().join("ca-bundle.crt"),
        shim_src: Path::new(BIN)
            .parent()
            .expect("the binary has a directory")
            .join("humanitl-shim"),
        session_env: vec![("HUMANITL_SESSION".to_owned(), session.to_string())],
        command: vec![
            OsString::from("sh"),
            OsString::from("-c"),
            OsString::from("echo hello world"),
        ],
    };
    let profile = SandboxProfile::load(&profile_file("default")).expect("the profile loads");
    let expected: Vec<String> = profile
        .to_bwrap_args(&context, &LaunchInputs::preview())
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    assert_eq!(parts[1..], expected[..]);
}

#[test]
fn sandbox_argv_needs_no_daemon_and_no_bwrap_files() {
    let harness = Harness::new();
    let output = harness.run(["--json", "sandbox", "argv"]);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    assert_eq!(value["profile"], "default");
    assert!(
        value["argv_line"].as_str().is_some_and(|line| {
            line.contains("--unshare-net") && line.contains("--cap-drop ALL")
        })
    );
}

#[test]
fn an_unknown_profile_is_config_001_and_names_where_it_looked() {
    let harness = Harness::new();
    let output = harness.run(["--profile", "nowhere", "sandbox", "argv"]);

    assert_eq!(code(&output), 1);
    let text = stderr(&output);
    assert!(text.contains("[CONFIG_001]"), "{text}");
    assert!(text.contains("nowhere.toml"), "{text}");
}

#[test]
fn daemon_status_and_flows_list_speak_to_a_running_daemon() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let status = harness.run(["--json", "daemon", "status"]);
    assert_eq!(code(&status), 0, "{}", stderr(&status));
    let info: serde_json::Value = serde_json::from_str(&stdout(&status)).expect("JSON");
    assert_eq!(info["proto_major"], 1);
    assert!(info["daemon_version"].as_str().is_some());

    let table = harness.run(["daemon", "status"]);
    assert_eq!(code(&table), 0);
    assert!(stdout(&table).contains("proto"), "{}", stdout(&table));

    let flows = harness.run(["--json", "flows", "list"]);
    assert_eq!(code(&flows), 0, "{}", stderr(&flows));
    let page: serde_json::Value = serde_json::from_str(&stdout(&flows)).expect("JSON");
    assert!(page["flows"].is_array());
}

#[test]
fn flows_show_falls_back_to_the_summary_and_reports_an_unknown_id() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    // Der Abspieler braucht einen Moment, bis der erste Flow steht.
    let deadline = Instant::now() + PATIENCE;
    let mut flows = Vec::new();
    while Instant::now() < deadline && flows.is_empty() {
        let output = harness.run(["--json", "flows", "list"]);
        assert_eq!(code(&output), 0, "{}", stderr(&output));
        let page: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
        flows = page["flows"].as_array().cloned().unwrap_or_default();
        if flows.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert!(!flows.is_empty(), "the recorded session produced no flow");

    let id = flows[0]["flow_id"].as_str().expect("a flow id").to_owned();
    let shown = harness.run(["flows", "show", &id]);
    assert_eq!(code(&shown), 0, "{}", stderr(&shown));
    assert!(stdout(&shown).contains(&id), "{}", stdout(&shown));

    let missing = harness.run(["flows", "show", "00000000-0000-7000-8000-000000000000"]);
    assert_eq!(code(&missing), 1);
    assert!(
        stderr(&missing).contains("[IPC_003]"),
        "{}",
        stderr(&missing)
    );
}

#[test]
fn sandbox_check_shows_the_three_guarantees() {
    let Some(_shim) = sandbox_available() else {
        eprintln!("skip: no bwrap or no humanitl-shim next to the binary");
        return;
    };
    let harness = Harness::new();
    let output = harness.run(["sandbox", "check"]);
    let text = stdout(&output);

    assert_eq!(code(&output), 0, "{}\n{}", text, stderr(&output));
    for check in ["no_network_interface", "single_socket", "seccomp_active"] {
        assert!(text.contains(check), "{check} is missing:\n{text}");
    }
    assert_eq!(text.matches('✓').count(), 3, "{text}");
}

#[test]
fn sandbox_run_exits_with_the_code_of_the_command() {
    let Some(_shim) = sandbox_available() else {
        eprintln!("skip: no bwrap or no humanitl-shim next to the binary");
        return;
    };
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let _socket = harness.wire_daemon_files();

    let output = harness.run(["sandbox", "run", "--", "sh", "-c", "exit 5"]);
    assert_eq!(code(&output), 5, "{}", stderr(&output));

    let zero = harness.run(["sandbox", "run", "--", "sh", "-c", "exit 0"]);
    assert_eq!(code(&zero), 0, "{}", stderr(&zero));
}

#[test]
fn sandbox_run_without_a_daemon_is_exit_two() {
    let harness = Harness::new();
    let output = harness.run(["sandbox", "run", "--", "sh", "-c", "exit 0"]);

    assert_eq!(code(&output), 2);
    assert!(
        stderr(&output).contains("[DAEMON_001]"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn sigint_stops_the_sandbox_and_ends_with_130() {
    let Some(_shim) = sandbox_available() else {
        eprintln!("skip: no bwrap or no humanitl-shim next to the binary");
        return;
    };
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let _socket = harness.wire_daemon_files();

    let mut child = harness
        .command()
        .args(["sandbox", "run", "--", "sh", "-c", "sleep 60"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary starts");

    // Der Sandbox Zeit geben, wirklich zu laufen: ein `SIGINT` vor dem Start
    // prüfte nichts.
    std::thread::sleep(Duration::from_millis(1500));
    let pid = child.id();
    let signalled = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("kill runs");
    assert!(signalled.success(), "cannot send SIGINT to {pid}");

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the child is waitable") {
            break status;
        }
        assert!(
            started.elapsed() < PATIENCE,
            "the command did not stop within {PATIENCE:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };

    assert_eq!(status.code(), Some(130), "SIGINT must end with 130");
    let mut out = std::io::stderr();
    let _ = writeln!(out, "sigint took {:?}", started.elapsed());
}
