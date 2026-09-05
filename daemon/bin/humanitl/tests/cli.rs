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
//!
//! Unter CI gilt das nicht: dort ist die Umgebung zugesagt, und
//! [`sandbox_required`] macht aus dem stillen Überspringen einen Fehlschlag
//! mit der Zeile, die sagt, was zu installieren ist.

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

/// Ob `bwrap` im Pfad liegt.
fn bwrap_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("bwrap").is_file()))
}

/// Ob eine Sandbox in diesem Lauf überhaupt starten kann.
fn sandbox_available() -> Option<PathBuf> {
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
fn sandbox_required() -> Option<PathBuf> {
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
        vec!["rules", "--help"],
        vec!["rules", "list", "--help"],
        vec!["rules", "add", "--help"],
        vec!["rules", "update", "--help"],
        vec!["rules", "remove", "--help"],
        vec!["rules", "reorder", "--help"],
        vec!["rules", "dry-run", "--help"],
        vec!["rules", "reload", "--help"],
        vec!["rules", "test", "--help"],
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
fn a_placeholder_subcommand_is_a_diagnostic_block_and_exit_one() {
    let harness = Harness::new();
    let output = harness.run(["audit", "verify"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CLI_003]: "), "{text}");
    assert!(text.contains("arrives in HUM-070"), "{text}");
    assert!(text.contains("\n  fix: "), "{text}");
    assert!(stdout(&output).is_empty(), "stdout must stay clean");
}

/// Ein Aufruf, den clap nicht lesen kann, ist ein Diagnostic wie jeder andere
/// Fehler: Block auf stderr, Exit 1, mit --json eine Zeile auf stdout.
#[test]
fn an_unreadable_command_line_is_a_diagnostic_not_bare_clap_text() {
    let harness = Harness::new();
    let output = harness.run(["sandbox", "bogus"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CLI_004]: "), "{text}");
    assert!(
        text.contains("humanitl --help"),
        "the fix names the help: {text}"
    );
    assert!(stdout(&output).is_empty(), "stdout must stay clean");

    let output = harness.run(["--json", "sandbox", "bogus"]);
    let text = stdout(&output);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert_eq!(text.lines().count(), 1, "{text}");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("one JSON value");
    assert_eq!(value["code"], "CLI_004");

    let output = harness.run(["--help"]);
    assert_eq!(code(&output), 0, "help is not an error");
}

#[test]
fn a_placeholder_subcommand_with_json_is_one_line_on_stdout() {
    let harness = Harness::new();
    let output = harness.run(["--json", "audit", "verify"]);
    let text = stdout(&output);

    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert_eq!(text.lines().count(), 1, "{text}");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("one JSON value");
    assert_eq!(value["code"], "CLI_003");
    assert!(
        value["why"]
            .as_str()
            .is_some_and(|why| why.contains("humanitl audit") && why.contains("HUM-070")),
        "{value}"
    );
    assert!(
        value["fix"]["command"]
            .as_str()
            .is_some_and(|fix| fix.contains("HUM-070")),
        "{value}"
    );
    assert!(stderr(&output).is_empty(), "stderr must stay clean");
}

/// Ohne Daemon startet `humanitl run` nichts und sagt, wie man ihn startet.
///
/// Das ist der erste Eindruck des Werkzeugs: Wer es zum ersten Mal aufruft,
/// hat meistens keinen Daemon laufen. Er bekommt `DAEMON_001`, Exit 2 und
/// einen Befehl zum Abtippen — keine nackte Zeile, keine Panik.
#[test]
fn run_without_a_daemon_is_daemon_001_and_exit_two() {
    let harness = Harness::new();
    let output = harness.run(["run", "--", "sh", "-c", "echo hi"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 2, "{text}");
    assert!(text.starts_with("blocking[DAEMON_001]: "), "{text}");
    assert!(text.contains("\n  fix: "), "{text}");
    assert!(
        stdout(&output).is_empty(),
        "stdout carries the agent, not us"
    );
}

/// `--ask terminal` verweigert den Dienst, bevor irgendetwas verbindet.
///
/// `CLI_002` steht in CONVENTIONS 4.10 für Vollbild-TUI-Agenten; ohne das PTY
/// aus HUM-042 gilt es für jeden. Der Befund nennt beide Auswege, und der
/// Daemon wird gar nicht erst gefragt: Der Test läuft ohne einen.
#[test]
fn run_with_ask_terminal_is_cli_002_before_it_connects() {
    let harness = Harness::new();
    let output = harness.run(["run", "--ask", "terminal"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CLI_002]: "), "{text}");
    assert!(text.contains("--ask ui"), "{text}");
    assert!(text.contains("--ask none"), "{text}");
}

/// Ein Sitzungsprofil, das es nicht gibt, ist `CONFIG_001` und kein stiller
/// Start mit dem Vorgabeprofil.
#[test]
fn run_with_an_unknown_profile_is_config_001() {
    let harness = Harness::new();
    let output = harness.run(["--profile", "there-is-no-such-profile", "run"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CONFIG_001]: "), "{text}");
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

/// `--profile` benennt beides (`backlog/CONVENTIONS.md` 3.8 und 4.23): Gibt es
/// ein Sitzungsprofil des Namens, ist es gemeint; sonst das bwrap-Profil.
#[test]
fn a_bundled_profile_reaches_config_get_with_its_origin() {
    let harness = Harness::new();
    let output = harness.run(["--profile", "llm-only", "config", "get", "hold.ask_mode"]);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "none");
    assert!(
        stderr(&output).contains("profile builtin llm-only"),
        "the origin explains which layer won: {}",
        stderr(&output)
    );

    // Das Sitzungsprofil laesst `sandbox.profile` in Ruhe: sonst suchte der
    // Start eine Datei profiles/sandbox/llm-only.toml, die es nicht gibt.
    let sandbox = harness.run(["--profile", "llm-only", "config", "get", "sandbox.profile"]);
    assert_eq!(stdout(&sandbox).trim(), "default");
}

/// Was `--profile` benennt, entscheidet das Unterkommando und nicht, welche
/// Dateien gerade auf der Platte liegen (`backlog/CONVENTIONS.md` 4.23).
#[test]
fn under_sandbox_the_profile_flag_always_names_the_bwrap_profile() {
    let harness = Harness::new();
    // `test` ist ein bwrap-Profil und kein Sitzungsprofil; unter `sandbox` ist
    // das der Normalfall und kein Fehler.
    let output = harness.run(["--profile", "test", "sandbox", "argv"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("/tests/escape"),
        "the test profile is the one that mounts the escape directory: {}",
        stdout(&output)
    );

    // Und es bleibt dabei, wenn jemand ein gleichnamiges Sitzungsprofil anlegt.
    // Vorher hing die Bedeutung an der Anwesenheit dieser Datei: die
    // Einhaengung waere lautlos verschwunden.
    let profiles = harness.path("config/humanitl/profiles");
    std::fs::create_dir_all(&profiles).expect("the profile directory");
    std::fs::write(
        profiles.join("test.toml"),
        "name = \"test\"\ndescription = \"eine Falle\"\n",
    )
    .expect("the session profile");
    let output = harness.run(["--profile", "test", "sandbox", "argv"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("/tests/escape"),
        "a session profile of the same name must not change what sandbox means: {}",
        stdout(&output)
    );
}

/// Ausserhalb von `sandbox` ist `--profile` das Sitzungsprofil, und ein Name
/// ohne Profil ist ein Fehler statt eines stillen Vorgabeprofils.
#[test]
fn outside_sandbox_an_unknown_profile_is_config_001() {
    let harness = Harness::new();
    let output = harness.run(["--profile", "test", "config", "get", "sandbox.profile"]);

    assert_eq!(code(&output), 1, "{}", stdout(&output));
    assert!(
        stderr(&output).starts_with("error[CONFIG_001]: "),
        "{}",
        stderr(&output)
    );
}

/// Ein Profil, das da ist, sich aber nicht lesen laesst, ist ein Profil.
/// Frueher galt es als „kein Sitzungsprofil", und `--profile work` bekam
/// lautlos eine andere Bedeutung, mit Exit 0 und ohne Befund.
#[test]
fn a_profile_that_does_not_parse_stops_the_command() {
    let harness = Harness::new();
    let profiles = harness.path("config/humanitl/profiles");
    std::fs::create_dir_all(&profiles).expect("the profile directory");
    std::fs::write(profiles.join("work.toml"), "[config.hold\n").expect("a broken profile");

    let output = harness.run(["--profile", "work", "config", "get", "hold.timeout_secs"]);
    assert_eq!(code(&output), 1, "{}", stdout(&output));
    assert!(
        stderr(&output).starts_with("error[CONFIG_001]: "),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("not valid TOML"),
        "{}",
        stderr(&output)
    );
}

/// `--work` benennt das Projekt, nicht das aktuelle Verzeichnis.
#[test]
fn the_project_profile_comes_from_the_work_directory() {
    let harness = Harness::new();
    let elsewhere = harness.path("elsewhere");
    std::fs::create_dir_all(elsewhere.join(".humanitl")).expect("the project directory");
    std::fs::write(
        elsewhere.join(".humanitl/profile.toml"),
        "[config.hold]\ntimeout_secs = 77\n",
    )
    .expect("the project profile");

    // Aus einem anderen Verzeichnis heraus, mit --work auf das Projekt.
    let output = harness.run([
        "--work",
        elsewhere.to_str().expect("a path"),
        "config",
        "get",
        "hold.timeout_secs",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "77");

    // Und die gefaehrliche Richtung: das aktuelle Verzeichnis traegt ein
    // Projekt-Profil, --work zeigt woanders hin. Dann gilt das Profil des
    // Projekts, an dem gearbeitet wird, nicht das der Shell.
    std::fs::create_dir_all(harness.path("work/.humanitl")).expect("the cwd project");
    std::fs::write(
        harness.path("work/.humanitl/profile.toml"),
        "[config.hold]\ntimeout_secs = 5\n",
    )
    .expect("the hostile project profile");
    let empty = harness.path("empty");
    std::fs::create_dir_all(&empty).expect("an empty work directory");
    let output = harness.run([
        "--work",
        empty.to_str().expect("a path"),
        "config",
        "get",
        "hold.timeout_secs",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "300",
        "the profile of the directory the shell stands in must not decide"
    );
}

/// Ein Projekt darf kein beliebiges Profil des Nutzers einsetzen; sonst haette
/// ein geklontes Repository ueber `name` jeden gesperrten Schluessel gesetzt.
#[test]
fn a_project_may_not_choose_a_profile_of_the_user() {
    let harness = Harness::new();
    let profiles = harness.path("config/humanitl/profiles");
    std::fs::create_dir_all(&profiles).expect("the profile directory");
    std::fs::write(
        profiles.join("loose.toml"),
        "name = \"loose\"\n\n[config.agent]\ncommand = [\"/bin/sh\", \"-c\", \"id\"]\n",
    )
    .expect("the user profile");
    std::fs::create_dir_all(harness.path("work/.humanitl")).expect("the project directory");
    std::fs::write(
        harness.path("work/.humanitl/profile.toml"),
        "name = \"loose\"\n",
    )
    .expect("the project profile");

    let output = harness.run(["config", "get", "agent.command"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "-",
        "agent.command must stay at its default"
    );
    assert!(
        stderr(&output).contains("[CONFIG_009]"),
        "the ignored wish is reported: {}",
        stderr(&output)
    );

    // Wer das Profil wirklich meint, sagt es auf der Kommandozeile.
    let output = harness.run(["--profile", "loose", "config", "get", "agent.command"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(stdout(&output).contains("/bin/sh"), "{}", stdout(&output));
}

#[test]
fn the_profile_list_names_the_bundled_profiles_with_their_description() {
    let harness = Harness::new();
    let output = harness.run(["config", "schema", "--profiles"]);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("default"), "{text}");
    assert!(text.contains("llm-only"), "{text}");
    assert!(text.contains("Pure inference"), "{text}");
    assert!(text.contains("bundled"), "{text}");

    let json = harness.run(["--json", "config", "schema", "--profiles"]);
    let value: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("JSON");
    let names: Vec<&str> = value["profiles"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert_eq!(names, vec!["default", "llm-only"]);
}

#[test]
fn run_refuses_a_project_profile_that_wants_to_mount_host_paths() {
    let harness = Harness::new();
    let project = harness.path("work").join(".humanitl");
    std::fs::create_dir_all(&project).expect("the project directory");
    std::fs::write(
        project.join("profile.toml"),
        "[config.sandbox.mounts]\nextra_rw = [\"/etc\"]\n",
    )
    .expect("the project profile");

    let output = harness.run(["run"]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert!(
        stderr(&output).starts_with("error[CONFIG_003]: "),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("mount host paths"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn run_refuses_a_profile_that_does_not_exist() {
    let harness = Harness::new();
    let output = harness.run(["run", "--profile", "nowhere"]);

    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert!(
        stderr(&output).starts_with("error[CONFIG_001]: "),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("llm-only"), "{}", stderr(&output));
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
    let output = harness.run(["audit", "verify"]);
    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("HUM-070"),
        "the placeholder does not name its issue: {}",
        stderr(&output)
    );
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
        files: Vec::new(),
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

/// Wartet, bis der Abspieler des Fakes wenigstens einen Flow gemeldet hat.
///
/// Sortiert wird aufsteigend, damit die erste Zeile die erste Anfrage der
/// aufgezeichneten Sitzung ist und ein Test sich auf sie beziehen kann.
fn wait_for_flows(harness: &Harness) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + PATIENCE;
    let mut flows = Vec::new();
    while Instant::now() < deadline && flows.is_empty() {
        let output = harness.run(["--json", "flows", "list", "--asc"]);
        assert_eq!(code(&output), 0, "{}", stderr(&output));
        let page: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
        flows = page["flows"].as_array().cloned().unwrap_or_default();
        if flows.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert!(!flows.is_empty(), "the recorded session produced no flow");
    flows
}

/// Der JSON-Wert, den ein Aufruf mit `--json` auf stdout schreibt.
fn json_of(output: &Output) -> serde_json::Value {
    let text = stdout(output);
    assert_eq!(text.lines().count(), 1, "not one line of JSON: {text}");
    serde_json::from_str(text.trim()).expect("one JSON value")
}

/// `rules test` gibt es im Vertrag nicht; es sagt, was fehlt, und nicht, dass
/// es keinen Daemon gäbe.
#[test]
fn rules_test_names_the_operation_the_contract_does_not_have() {
    let harness = Harness::new();
    let output = harness.run(["rules", "test", "https://evil.example"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CLI_003]: "), "{text}");
    assert!(text.contains("Rules RPC"), "{text}");
    assert!(text.contains("https://evil.example"), "{text}");
    assert!(stdout(&output).is_empty(), "stdout must stay clean");
}

#[test]
fn rules_without_a_daemon_is_daemon_001_and_exit_two() {
    let harness = Harness::new();
    for command in [
        vec!["rules", "list"],
        vec!["rules", "reload"],
        vec!["rules", "remove", "018f0001-0000-7000-8000-000000000001"],
    ] {
        let output = harness.run(command.clone());
        assert_eq!(code(&output), 2, "{command:?}: {}", stderr(&output));
        assert!(
            stderr(&output).starts_with("blocking[DAEMON_001]: "),
            "{command:?}: {}",
            stderr(&output)
        );
    }
}

/// Eine Regel ohne Aktion wird gemeldet, bevor irgendjemand gefragt wird.
#[test]
fn a_rule_without_action_or_host_is_cli_004_without_a_daemon() {
    let harness = Harness::new();
    for command in [
        vec!["rules", "add", "--host", "api.github.com"],
        vec!["rules", "add", "--action", "allow"],
        vec!["rules", "dry-run", "--host", "api.github.com"],
    ] {
        let output = harness.run(command.clone());
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{command:?}: {text}");
        assert!(text.starts_with("error[CLI_004]: "), "{command:?}: {text}");
        assert!(text.contains("humanitl rules add --help"), "{text}");
    }
}

#[test]
fn rules_list_hides_the_bundled_rules_until_all() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let plain = harness.run(["rules", "list"]);
    assert_eq!(code(&plain), 0, "{}", stderr(&plain));
    assert!(stdout(&plain).is_empty(), "{}", stdout(&plain));
    assert!(stderr(&plain).contains("--all"), "{}", stderr(&plain));

    let all = harness.run(["rules", "list", "--all"]);
    let text = stdout(&all);
    assert_eq!(code(&all), 0, "{}", stderr(&all));
    for column in [
        "POS", "ACTION", "HOST", "METHODS", "PATH", "EXPIRES", "ORIGIN", "ID",
    ] {
        assert!(text.contains(column), "{column} is missing:\n{text}");
    }
    assert!(text.contains("bundled"), "{text}");
    assert!(text.contains("models.dev"), "{text}");
    assert!(
        text.lines().all(|line| !line.ends_with(' ')),
        "the table must not pad the last column:\n{text}"
    );

    let json = harness.run(["--json", "rules", "list", "--all"]);
    assert_eq!(code(&json), 0, "{}", stderr(&json));
    let value = json_of(&json);
    let rules = value["rules"].as_array().expect("an array of rules");
    assert!(
        rules
            .iter()
            .any(|rule| rule["origin"] == "bundled" && rule["action"] == "block"),
        "{value}"
    );
}

#[test]
fn a_rule_is_added_changed_moved_and_removed_over_the_rpc() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let added = harness.run([
        "--json",
        "rules",
        "add",
        "--action",
        "allow",
        "--host",
        "**.github.com",
        "--method",
        "get",
        "--expires",
        "session",
        "--note",
        "npm install",
    ]);
    assert_eq!(code(&added), 0, "{}", stderr(&added));
    let value = json_of(&added);
    let rule = &value["added"];
    let id = rule["rule_id"].as_str().expect("an id").to_owned();
    assert_eq!(rule["action"], "allow", "{value}");
    assert_eq!(rule["origin"], "session", "{value}");
    assert_eq!(rule["host"], "**.github.com", "{value}");
    assert_eq!(rule["methods"][0], "GET", "{value}");
    assert_eq!(rule["expires"]["kind"], "session", "{value}");
    assert_eq!(rule["note"], "npm install", "{value}");

    // Was nicht genannt wird, bleibt stehen.
    let updated = harness.run(["--json", "rules", "update", &id, "--action", "block"]);
    assert_eq!(code(&updated), 0, "{}", stderr(&updated));
    let value = json_of(&updated);
    assert_eq!(value["updated"]["action"], "block", "{value}");
    assert_eq!(value["updated"]["host"], "**.github.com", "{value}");
    assert_eq!(value["updated"]["note"], "npm install", "{value}");

    // Geprüft wird die Reihenfolge, in der der Daemon danach auswertet, nicht
    // die Zahl in `position`: die vergibt der Dienst, und die Kommandozeile
    // schreibt sie hin, statt sie zu behaupten.
    let moved = harness.run(["--json", "rules", "reorder", &id, "1"]);
    assert_eq!(code(&moved), 0, "{}", stderr(&moved));
    let value = json_of(&moved);
    assert_eq!(value["moved"]["rule_id"], id.as_str(), "{value}");
    assert_eq!(value["rules"][0]["rule_id"], id.as_str(), "{value}");

    let removed = harness.run(["rules", "remove", &id]);
    assert_eq!(code(&removed), 0, "{}", stderr(&removed));
    assert!(stdout(&removed).contains(&id), "{}", stdout(&removed));

    let listed = harness.run(["--json", "rules", "list", "--all"]);
    let value = json_of(&listed);
    assert!(
        !value["rules"]
            .as_array()
            .expect("an array")
            .iter()
            .any(|rule| rule["rule_id"] == id.as_str()),
        "{value}"
    );

    // Ein zweites Mal löschen behauptet nichts, sondern sagt, dass es die
    // Regel nicht gibt.
    let again = harness.run(["rules", "remove", &id]);
    assert_eq!(code(&again), 1, "{}", stderr(&again));
    assert!(stderr(&again).contains("[IPC_005]"), "{}", stderr(&again));
}

/// Eine mitgelieferte Regel gehört nicht dem Nutzer.
#[test]
fn a_bundled_rule_is_not_moved_and_says_why() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let listed = harness.run(["--json", "rules", "list", "--all"]);
    let value = json_of(&listed);
    let bundled = value["rules"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|rule| rule["bundled"] == true)
        .expect("the fake ships bundled rules")
        .clone();
    let id = bundled["rule_id"].as_str().expect("an id").to_owned();

    let output = harness.run(["rules", "reorder", &id, "1"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[RULES_010]: "), "{text}");
    assert!(text.contains("\n  fix: humanitl rules add "), "{text}");
}

#[test]
fn a_dry_run_says_how_many_of_how_many() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let flows = wait_for_flows(&harness);
    let host = flows[0]["authority"]["host"]
        .as_str()
        .expect("a host")
        .to_owned();

    let text_run = harness.run(["rules", "dry-run", "--action", "block", "--host", &host]);
    let text = stdout(&text_run);
    assert_eq!(code(&text_run), 0, "{}", stderr(&text_run));
    assert!(
        text.contains("recorded requests would have matched"),
        "{text}"
    );

    let output = harness.run([
        "--json", "rules", "dry-run", "--action", "block", "--host", &host, "--scan", "50",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);
    assert!(value["scanned"].as_u64().is_some(), "{value}");
    let matches = value["matches"].as_array().expect("an array of matches");
    assert!(
        matches
            .iter()
            .all(|flow| flow["authority"]["host"] == host.as_str()),
        "{value}"
    );

    // Ein Probelauf ändert nichts.
    let after = json_of(&harness.run(["--json", "rules", "list", "--all"]));
    assert_eq!(
        after["rules"].as_array().map(Vec::len),
        value["rules"].as_array().map(Vec::len)
    );
}

/// Ein Probelauf ohne Treffer ist eine Null und kein Absturz.
#[test]
fn a_dry_run_without_a_hit_is_still_a_number() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let output = harness.run([
        "--json",
        "rules",
        "dry-run",
        "--action",
        "ask",
        "--host",
        "nothing.invalid",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);
    assert_eq!(
        value["matches"].as_array().map(Vec::len),
        Some(0),
        "{value}"
    );
}

#[test]
fn a_reload_reports_what_the_daemon_reported_and_nothing_else() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let output = harness.run(["rules", "reload"]);
    let text = stdout(&output);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    // Der Fake hat keine Regel-Datei und meldet deshalb keinen RULES_011. Die
    // Zeile behauptet dann keine Änderung.
    assert!(
        text.contains("without a report of what changed") || text.contains("[RULES_011]"),
        "{text}"
    );

    let json = harness.run(["--json", "rules", "reload"]);
    assert_eq!(code(&json), 0, "{}", stderr(&json));
    let value = json_of(&json);
    assert!(value["diagnostics"].is_array(), "{value}");
    assert!(value["rules"].is_array(), "{value}");
}

#[test]
fn flows_list_hands_the_filter_over_and_survives_an_empty_result() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let flows = wait_for_flows(&harness);
    let host = flows[0]["authority"]["host"]
        .as_str()
        .expect("a host")
        .to_owned();

    // Das Akzeptanzkriterium: ein Filter in einem Wort, ein leeres Ergebnis,
    // keine Panik.
    let empty = harness.run(["flows", "list", "host:nothing.invalid findings:>0"]);
    assert_eq!(code(&empty), 0, "{}", stderr(&empty));
    assert!(stdout(&empty).is_empty(), "{}", stdout(&empty));
    assert!(stderr(&empty).contains("no flows"), "{}", stderr(&empty));

    // Und derselbe Filter in mehreren Wörtern.
    let split = harness.run(["flows", "list", "host:nothing.invalid", "findings:>0"]);
    assert_eq!(code(&split), 0, "{}", stderr(&split));

    let hit = harness.run(["--json", "flows", "list", &format!("host:{host}")]);
    assert_eq!(code(&hit), 0, "{}", stderr(&hit));
    let value = json_of(&hit);
    let rows = value["flows"].as_array().expect("an array of flows");
    assert!(!rows.is_empty(), "{value}");
    assert!(
        rows.iter()
            .all(|flow| flow["authority"]["host"] == host.as_str()),
        "{value}"
    );

    let sorted = harness.run(["flows", "list", "--sort", "host", "--asc", "--limit", "5"]);
    let text = stdout(&sorted);
    assert_eq!(code(&sorted), 0, "{}", stderr(&sorted));
    for column in ["ID", "TIME", "STATE", "SIZE", "MS", "FINDINGS", "RULE"] {
        assert!(text.contains(column), "{column} is missing:\n{text}");
    }
    // PATH wird in der Mitte gekürzt, die Spalten bleiben schmal.
    assert!(
        text.lines().all(|line| line.chars().count() < 200),
        "{text}"
    );
}

#[test]
fn flows_show_prints_a_body_and_refuses_raw_together_with_json() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let deadline = Instant::now() + PATIENCE;
    let mut with_body = None;
    while Instant::now() < deadline && with_body.is_none() {
        with_body = wait_for_flows(&harness)
            .into_iter()
            .find(|flow| flow["request_size"].as_u64().unwrap_or(0) > 0);
        if with_body.is_none() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let flow = with_body.expect("the recorded session has a request with a body");
    let id = flow["flow_id"].as_str().expect("a flow id").to_owned();

    let clash = harness.run(["--json", "flows", "show", &id, "--body", "request", "--raw"]);
    assert_eq!(code(&clash), 1, "{}", stderr(&clash));
    assert_eq!(json_of(&clash)["code"], "CLI_004");

    let body = harness.run(["--json", "flows", "show", &id, "--body", "request"]);
    assert_eq!(code(&body), 0, "{}", stderr(&body));
    let value = json_of(&body);
    assert_eq!(value["body"], "request", "{value}");
    assert_eq!(value["present"], true, "{value}");
    assert_eq!(value["utf8"], true, "{value}");
    assert!(value["bytes"].as_u64().is_some_and(|n| n > 0), "{value}");

    let raw = harness.run(["flows", "show", &id, "--body", "request", "--raw"]);
    assert_eq!(code(&raw), 0, "{}", stderr(&raw));
    assert!(!raw.stdout.is_empty());
    assert_eq!(
        raw.stdout.len(),
        usize::try_from(value["bytes"].as_u64().expect("a size")).expect("a size that fits")
    );
}

#[test]
fn sandbox_check_shows_the_three_guarantees() {
    let Some(_shim) = sandbox_required() else {
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

/// `SIGINT` ist eine Bitte, kein Schlag: Der Agent hört sie, räumt auf und
/// endet mit seinem eigenen Code, und der ist der Code der Kommandozeile.
#[test]
fn sigint_reaches_the_agent_and_keeps_its_exit_code() {
    let Some(_shim) = sandbox_available() else {
        eprintln!("skip: no bwrap or no humanitl-shim next to the binary");
        return;
    };
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let _socket = harness.wire_daemon_files();

    let mut child = harness
        .command()
        .args([
            "sandbox",
            "run",
            "--",
            "sh",
            "-c",
            // Der Handler steht, bevor die Markierung erscheint: Erst dann darf
            // das Signal kommen, sonst trifft es eine Shell ohne Handler und
            // das Ergebnis haengt davon ab, wie schnell die Sandbox unter Last
            // hochkommt (so ist der Test in einer vollen Pruefung gekippt).
            "trap 'exit 42' INT; : > /work/ready; sleep 60",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary starts");

    let ready = harness.path("work").join("ready");
    let waiting = Instant::now();
    while !ready.exists() {
        assert!(
            waiting.elapsed() < PATIENCE,
            "the agent did not install its handler within {PATIENCE:?}"
        );
        if let Some(status) = child.try_wait().expect("the child is waitable") {
            panic!("the sandbox ended before the handler was installed: {status}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
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

    assert_eq!(
        status.code(),
        Some(42),
        "the handler of the agent must decide the exit code, not the escalation"
    );
}

#[test]
fn sandbox_env_from_the_config_reaches_the_argv() {
    // HUM-045: `FixAction::SetEnv` schreibt nach `sandbox.env`. Der Knopf ist
    // nur dann etwas wert, wenn der Wert auch in der Sandbox ankommt und dabei
    // die Vorgabe des Profils überschreibt.
    let harness = Harness::new();
    let config_dir = harness.path("config").join("humanitl");
    std::fs::create_dir_all(&config_dir).expect("the config directory");
    std::fs::write(
        config_dir.join("config.toml"),
        "[sandbox.env]\nCURL_CA_BUNDLE = \"/work/.certs/own.pem\"\n",
    )
    .expect("the config file");

    let output = harness.run(["--json", "sandbox", "argv"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    let line = value["argv_line"].as_str().expect("an argv line");
    assert!(
        line.contains("--setenv CURL_CA_BUNDLE /work/.certs/own.pem"),
        "{line}"
    );
    assert!(
        !line.contains("--setenv CURL_CA_BUNDLE /etc/humanitl/ca.crt"),
        "the value of the profile must not survive next to it: {line}"
    );
}
