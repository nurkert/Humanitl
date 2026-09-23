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
use std::time::{Duration, Instant};

use humanitl_config::WorkMode;
use humanitl_core::ids::SessionId;
use humanitl_ipc::auth;
use humanitl_sandbox::{LaunchInputs, SandboxProfile, SessionContext};

mod common;

use common::{
    BIN, FakeServer, Harness, PATIENCE, code, profile_file, sandbox_required, stderr, stdout,
};

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
        vec!["config", "set", "--help"],
        vec!["config", "schema", "--help"],
        vec!["config", "edit", "--help"],
        vec!["audit", "--help"],
        vec!["audit", "verify", "--help"],
        vec!["audit", "export", "--help"],
        vec!["daemon", "--help"],
        vec!["daemon", "status", "--help"],
        vec!["daemon", "install", "--help"],
        vec!["daemon", "logs", "--help"],
    ] {
        let output = harness.run(command.clone());
        assert_eq!(code(&output), 0, "{command:?} has no help");
        assert!(!stdout(&output).is_empty(), "{command:?} printed nothing");
    }
}

// --- humanitl doctor (HUM-075) ---------------------------------------------

/// Die elf Kennungen, in der Reihenfolge der Anzeige.
fn doctor_ids() -> Vec<String> {
    humanitl_sandbox::doctor::CheckId::ALL
        .iter()
        .map(|id| (*id).as_str().to_owned())
        .collect()
}

/// Der Bericht aus `humanitl doctor --json`, samt Exit-Code.
fn doctor_json(harness: &Harness, extra: &[&str]) -> (serde_json::Value, i32) {
    let mut args = vec!["doctor", "--json"];
    args.extend_from_slice(extra);
    let output = harness.run(args);
    let text = stdout(&output);
    let value: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|error| panic!("one JSON value on stdout ({error}): {text}"));
    (value, code(&output))
}

/// Der Code des Befunds einer Zeile, leer wenn sie keinen traegt.
fn doctor_code(report: &serde_json::Value, id: &str) -> String {
    report["checks"]
        .as_array()
        .expect("checks is an array")
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("the line {id}"))["diagnostic"]["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Ein Socket, der die Verbindung annimmt und nie antwortet.
///
/// Genau der Fall, in dem `humanitl doctor` ohne Frist fuer immer haengt: Der
/// Pfad ist da, das Token ist da, die Verbindung kommt zustande — und danach
/// sagt niemand mehr etwas. Der Lauscher nimmt an und legt die Verbindung
/// beiseite, statt sie zu schliessen; ein `drop` waere ein Verbindungsabbruch
/// und damit ein anderer Fall.
struct SilentSocket {
    stop: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SilentSocket {
    fn start(harness: &Harness) -> Self {
        let paths = harness.paths();
        let socket = paths.daemon_socket();
        std::fs::create_dir_all(socket.parent().expect("the socket has a directory"))
            .expect("the runtime directory");
        let token = auth::new_token().expect("a token");
        auth::write_token(&paths.token_path(), &token).expect("the token is written");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("the socket binds");

        let (stop, stopped) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut held = Vec::new();
            listener
                .set_nonblocking(true)
                .expect("the listener can poll");
            while stopped.try_recv().is_err() {
                if let Ok((stream, _)) = listener.accept() {
                    held.push(stream);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Self {
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}

impl Drop for SilentSocket {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Ein Daemon, der schweigt, haelt den Doctor nicht auf.
///
/// Ohne Frist um `connect`, `GetInfo` und `Doctor` haengt dieser Aufruf fuer
/// immer und erreicht den Rueckfall auf den eigenen Prozess **nie** — womit
/// genau die Eigenschaft hin waere, die den Rueckfall begruendet.
#[test]
fn a_daemon_that_answers_nothing_does_not_hold_the_doctor() {
    let harness = Harness::new();
    let _silent = SilentSocket::start(&harness);

    // Die Frist gehoert dem Test und nicht dem Prozess: Ohne Frist im Code
    // kaeme dieser Aufruf nie zurueck, und ein Test, der darauf wartet, waere
    // kein Test, sondern derselbe Haenger.
    let output = harness
        .run_bounded(["doctor", "--json"], Duration::from_secs(60))
        .expect("the doctor finishes on a socket that never answers");
    let text = stdout(&output);
    let report: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|error| panic!("one JSON value on stdout ({error}): {text}"));
    let exit = code(&output);

    assert_ne!(
        exit, 2,
        "a silent daemon is a line of the report, not exit 2"
    );
    assert_eq!(
        report["source"], "local",
        "the fallback is the whole point: {report}"
    );
    assert_eq!(
        doctor_code(&report, "daemon"),
        "DOCTOR_006",
        "a socket that holds and says nothing is not a reachable daemon: {report}"
    );
    let ids: Vec<String> = report["checks"]
        .as_array()
        .expect("checks is an array")
        .iter()
        .map(|check| check["id"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(ids, doctor_ids(), "every line is there");
}

#[test]
fn doctor_without_a_daemon_is_a_report_and_not_exit_two() {
    let harness = Harness::new();
    let output = harness.run(["doctor"]);

    // Kein Daemon ist hier eine Zeile des Berichts und kein Abbruch: Genau
    // dafuer gibt es den Befehl.
    assert_ne!(code(&output), 2, "{}", stderr(&output));
    let text = stdout(&output);
    for id in doctor_ids() {
        assert!(text.contains(&id), "the line {id} is missing from:\n{text}");
    }
    assert!(text.contains("DOCTOR_006"), "{text}");
}

#[test]
fn json_shape_stable() {
    let harness = Harness::new();
    let (report, exit) = doctor_json(&harness, &[]);

    let top: Vec<&str> = report
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(top, vec!["checks", "source", "status"], "{report}");
    assert!(
        ["ok", "warn", "fail"].contains(&report["status"].as_str().unwrap_or_default()),
        "{report}"
    );
    assert_eq!(report["source"], "local", "no daemon runs in this harness");
    assert!(exit == 0 || exit == 3, "exit {exit} is not 0 or 3");

    let checks = report["checks"].as_array().expect("checks is an array");
    let ids: Vec<String> = checks
        .iter()
        .map(|check| check["id"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(ids, doctor_ids(), "the order is part of the contract");

    for check in checks {
        let id = check["id"].as_str().unwrap_or_default();
        let status = check["status"].as_str().unwrap_or_default();
        assert!(
            ["ok", "warn", "fail"].contains(&status),
            "{id} carries {status}"
        );
        assert!(
            check["evidence"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "{id} has no evidence"
        );
        if status == "ok" {
            assert!(
                check.get("diagnostic").is_none(),
                "{id} is green with a finding"
            );
            continue;
        }
        let diagnostic = check
            .get("diagnostic")
            .unwrap_or_else(|| panic!("{id} has no finding"));
        for key in ["code", "severity", "title", "why", "fix", "docs"] {
            assert!(
                diagnostic.get(key).is_some(),
                "{id}: the finding has no {key}"
            );
        }
        assert!(
            diagnostic["code"]
                .as_str()
                .unwrap_or_default()
                .starts_with("DOCTOR_"),
            "{id} carries {}, and the doctor keeps one code per line",
            diagnostic["code"]
        );
    }
}

#[test]
fn doctor_reaches_no_endpoint_unless_it_is_asked_to() {
    let harness = Harness::new();
    // Auf diesem Port hoert nichts. Wer ihn ansprechen wuerde, brauchte die
    // Frist der Probe und meldete LLM_001; wer ihn in Ruhe laesst, ist sofort
    // fertig und meldet DOCTOR_013.
    let started = Instant::now();
    let (report, _) = doctor_json(&harness, &["--llm", "http://127.0.0.1:1/"]);
    let waited = started.elapsed();

    assert_eq!(
        doctor_code(&report, "llm"),
        "DOCTOR_013",
        "without --probe-llm nothing is contacted: {report}"
    );
    let evidence = report["checks"]
        .as_array()
        .and_then(|checks| checks.iter().find(|check| check["id"] == "llm"))
        .map(|check| check["evidence"].to_string())
        .unwrap_or_default();
    assert!(
        evidence.contains("127.0.0.1:1"),
        "the line names what would be contacted: {evidence}"
    );
    assert!(
        waited < PATIENCE,
        "the doctor waited {waited:?}, which looks like a connection"
    );
}

#[test]
fn doctor_with_probe_llm_and_no_daemon_measures_nothing_and_says_so() {
    let harness = Harness::new();
    let (report, _) = doctor_json(&harness, &["--llm", "http://127.0.0.1:1/", "--probe-llm"]);

    // Die Probe lebt im Daemon (ADR-018). Ohne ihn wird nicht gemessen, und
    // das ist etwas anderes als ein Endpunkt, der schweigt.
    assert_eq!(doctor_code(&report, "llm"), "DOCTOR_012", "{report}");
}

#[test]
fn doctor_says_where_it_goes_before_it_goes_there_and_no_flag_silences_that() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    // Auch mit --json und mit -q: Die Ankuendigung einer Verbindung ist Teil
    // der Handlung und nicht ihre Verzierung. `stdout` bleibt trotzdem ein
    // einziger JSON-Wert.
    let output = harness.run([
        "doctor",
        "--json",
        "-q",
        "--llm",
        "http://192.168.1.50:11434",
        "--probe-llm",
    ]);
    let note = stderr(&output);
    assert!(note.contains("192.168.1.50:11434"), "{note}");
    assert!(note.contains("/api/tags"), "{note}");
    assert!(note.contains("/v1/models"), "{note}");
    assert!(note.contains("no credentials"), "{note}");

    let text = stdout(&output);
    let report: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|error| panic!("one JSON value on stdout ({error}): {text}"));
    let llm = report["checks"]
        .as_array()
        .expect("checks is an array")
        .iter()
        .find(|check| check["id"] == "llm")
        .expect("the llm line")
        .clone();
    assert_eq!(llm["status"], "ok", "{llm}");
    assert!(
        llm["evidence"]
            .as_str()
            .unwrap_or_default()
            .contains("2 models"),
        "the line now carries a measurement: {llm}"
    );
}

#[test]
fn doctor_says_nothing_about_the_endpoint_when_it_does_not_go_there() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let output = harness.run(["doctor", "--json", "--llm", "http://192.168.1.50:11434"]);
    let note = stderr(&output);
    assert!(
        !note.contains("contacting"),
        "nothing was contacted, so nothing announces a contact: {note}"
    );
}

#[test]
fn doctor_takes_the_report_from_the_daemon_and_answers_the_daemon_line_itself() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let (report, exit) = doctor_json(&harness, &[]);

    assert_eq!(report["source"], "daemon", "the RPC is the way (ADR-018)");
    assert_eq!(exit, 0, "the fake reports warnings, not failures: {report}");

    let checks = report["checks"].as_array().expect("checks is an array");
    let ids: Vec<String> = checks
        .iter()
        .map(|check| check["id"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(ids, doctor_ids());

    // Der Fake hat nichts gemessen und sagt es; nur die Zeile `daemon` weiss
    // dieser Client besser, weil er gerade mit ihm gesprochen hat.
    assert_eq!(doctor_code(&report, "bwrap"), "DOCTOR_012", "{report}");
    let daemon = checks
        .iter()
        .find(|check| check["id"] == "daemon")
        .expect("the daemon line");
    assert_eq!(daemon["status"], "ok", "{daemon}");
    assert!(daemon.get("diagnostic").is_none(), "{daemon}");
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

/// Ohne Daemon und ohne Audit-Log hat `audit verify` nichts zu pruefen und
/// sagt es mit dem Befund des Daemons, nicht mit einem erfundenen Ergebnis.
#[test]
fn audit_verify_without_a_daemon_and_without_a_log_is_daemon_001() {
    let harness = Harness::new();
    let output = harness.run(["audit", "verify"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 2, "{text}");
    assert!(text.starts_with("blocking[DAEMON_001]: "), "{text}");
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

/// Und dasselbe mit `--json`: eine Zeile auf stdout, stderr bleibt leer.
#[test]
fn audit_verify_without_a_daemon_with_json_is_one_line_on_stdout() {
    let harness = Harness::new();
    let output = harness.run(["--json", "audit", "verify"]);
    let text = stdout(&output);

    assert_eq!(code(&output), 2, "{}", stderr(&output));
    assert_eq!(text.lines().count(), 1, "{text}");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("one JSON value");
    assert_eq!(value["code"], "DAEMON_001");
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

/// `--ask terminal` verweigert den Dienst für einen Vollbild-Agenten, bevor
/// irgendetwas verbindet.
///
/// `CLI_002` steht in CONVENTIONS 4.10 für genau diesen Fall: Ein TUI zeichnet
/// den ganzen Schirm neu, und der Kasten wäre nach dem ersten Bild weg. Der
/// Befund nennt beide Auswege, und der Daemon wird gar nicht erst gefragt: Der
/// Test läuft ohne einen.
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

/// Ohne Terminal auf der Eingabe gibt es keinen Prompt und keinen Start.
///
/// Der Test läuft über eine Pipe, wie jeder Test hier -- und genau das ist der
/// Fall, den die Verweigerung meint: Aus einer Pipe kämen Bytes, die niemand
/// als Antwort gemeint hat, und ein `b` in einem Skript blockte einen Fluss,
/// ohne dass ein Mensch die Frage gesehen hätte. Dass die Verweigerung sonst
/// am wirksamen Kommando hängt, halten die Tests in `cmd/run.rs` fest, die die
/// Frage nach dem Terminal als Wert bekommen.
#[test]
fn run_ask_terminal_without_a_terminal_is_cli_002() {
    let harness = Harness::new();
    let output = harness.run(["run", "--ask", "terminal", "--", "sh", "-c", "echo hi"]);
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CLI_002]: "), "{text}");
    assert!(text.contains("terminal on standard input"), "{text}");
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

    // Ein `systemctl`, das `active` antwortet: Der Test fragt nie den
    // systemd des Rechners, und die Zeile `unit` hat einen Wert, den er prüfen
    // kann.
    let (fake, _log) = answering_program(&harness, "systemctl", "active");
    let status = harness
        .command()
        .args(["--json", "daemon", "status"])
        .env("PATH", &fake)
        .output()
        .expect("the binary runs");
    assert_eq!(code(&status), 0, "{}", stderr(&status));
    let info: serde_json::Value = serde_json::from_str(&stdout(&status)).expect("JSON");
    assert_eq!(info["proto_major"], 1);
    assert!(info["daemon_version"].as_str().is_some());
    assert_eq!(info["unit"], "active");

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

/// Das Verdikt steht in der Zeile und im Exit-Code (CONVENTIONS 3.8): Ein
/// Skript liest die Zahl, ein Mensch die Zeile, und beide bekommen dasselbe.
#[test]
fn rules_test_ask_exit_11_and_block_exit_10() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    // Ohne Treffer gilt die Voreinstellung, und die ist `ask` -- nie `allow`.
    let ask = harness.run(["rules", "test", "https://evil.example/"]);
    let text = stdout(&ask);
    assert_eq!(code(&ask), 11, "{}", stderr(&ask));
    assert!(text.contains("verdict: ask"), "{text}");
    assert!(text.contains("rule: none (default ask)"), "{text}");

    // Die mitgelieferte Regel gegen `models.dev` trifft, und die Zeile nennt
    // sie mit Herkunft und Position.
    let block = harness.run(["rules", "test", "https://models.dev/api.json"]);
    let text = stdout(&block);
    assert_eq!(code(&block), 10, "{}", stderr(&block));
    assert!(text.contains("verdict: block"), "{text}");
    assert!(text.contains("(bundled, position "), "{text}");
    assert!(!text.contains("rule: none"), "{text}");
}

/// Eine eigene Regel entscheidet, und beide Ausgänge sind erreichbar: `allow`
/// endet 0, `redact` wird heute gehalten und endet deshalb wie ein `ask`.
#[test]
fn rules_test_allow_exit_0_and_redact_exit_11() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let added = harness.run([
        "rules",
        "add",
        "--action",
        "allow",
        "--host",
        "allowed.example",
    ]);
    assert_eq!(code(&added), 0, "{}", stderr(&added));
    let allow = harness.run(["rules", "test", "https://allowed.example/x"]);
    assert_eq!(code(&allow), 0, "{}", stderr(&allow));
    assert!(
        stdout(&allow).contains("verdict: allow"),
        "{}",
        stdout(&allow)
    );

    let added = harness.run([
        "rules",
        "add",
        "--action",
        "redact",
        "--host",
        "redacted.example",
    ]);
    assert_eq!(code(&added), 0, "{}", stderr(&added));
    let redact = harness.run(["rules", "test", "https://redacted.example/x"]);
    assert_eq!(code(&redact), 11, "{}", stderr(&redact));
    assert!(
        stdout(&redact).contains("verdict: redact"),
        "{}",
        stdout(&redact)
    );
}

/// `--json` trägt dieselben fünf Felder, und der Exit-Code bleibt das Verdikt.
#[test]
fn rules_test_json_shape() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let output = harness.run(["--json", "rules", "test", "https://models.dev/api.json"]);
    assert_eq!(code(&output), 10, "{}", stderr(&output));
    let value = json_of(&output);
    assert_eq!(value["verdict"], "block", "{value}");
    assert_eq!(value["matched"], true, "{value}");
    assert_eq!(value["origin"], "bundled", "{value}");
    assert!(
        value["rule_id"].as_str().is_some_and(|id| !id.is_empty()),
        "{value}"
    );
    assert!(value["position"].as_u64().is_some(), "{value}");
    // Eine mitgelieferte Regel ist keine Durchreiche zum Sprachmodell; der
    // Fall mit `true` steht im Unit-Test dieser Zeile, weil dieser Fake keinen
    // Endpunkt kennt und deshalb keine Durchreiche führt.
    assert_eq!(value["passthrough"], false, "{value}");

    let ask = harness.run(["--json", "rules", "test", "https://evil.example/"]);
    assert_eq!(code(&ask), 11, "{}", stderr(&ask));
    let value = json_of(&ask);
    assert_eq!(value["verdict"], "ask", "{value}");
    assert_eq!(value["matched"], false, "{value}");
    // Ohne Treffer gibt es keine Regel, also auch keine Herkunft und kein
    // Ja/Nein zur Durchreiche: zweimal `null` und nirgends ein `false`, das
    // mehr behauptete als bekannt ist.
    assert_eq!(value["origin"], serde_json::Value::Null, "{value}");
    assert_eq!(value["passthrough"], serde_json::Value::Null, "{value}");
}

/// Was keine Anfrage-URL ist, wird gemeldet, bevor jemand gefragt wird -- und
/// ohne Daemon ist das Kommando dasselbe wie jedes andere: Exit 2.
#[test]
fn rules_test_bad_url_is_cli_004_and_without_a_daemon_exit_2() {
    let harness = Harness::new();

    for bad in ["evil.example", "https://a.example/x#top"] {
        let output = harness.run(["rules", "test", bad]);
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{bad}: {text}");
        assert!(text.starts_with("error[CLI_004]: "), "{bad}: {text}");
        assert!(text.contains(bad), "{bad}: {text}");
        assert!(stdout(&output).is_empty(), "stdout must stay clean: {bad}");
    }

    let output = harness.run(["rules", "test", "https://evil.example/"]);
    assert_eq!(code(&output), 2, "{}", stderr(&output));
    assert!(
        stderr(&output).starts_with("blocking[DAEMON_001]: "),
        "{}",
        stderr(&output)
    );
}

/// `llm test` fragt den Endpunkt über den Daemon und schreibt, was zurückkam.
#[test]
fn llm_test_prints_models() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let output = harness.run(["llm", "test", "http://127.0.0.1:11434"]);
    let text = stdout(&output);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(text.contains("flavor: ollama"), "{text}");
    assert!(text.contains("latency: 0 ms"), "{text}");
    assert!(text.contains("models: 2"), "{text}");
    // Der Fake misst nichts und sagt das an jedem Namen; ein Test, der das
    // wegliest, übte gegen eine Messung, die nie stattgefunden hat.
    assert!(text.contains("nothing was measured"), "{text}");
    // Ein Name aus dem Netz bekommt hier nicht mehr Platz als in der
    // Oberfläche: 40 Zeichen, zwei davor für die Einrückung.
    assert!(
        text.lines()
            .filter(|line| line.starts_with("  "))
            .all(|line| line.chars().count() <= 42),
        "a model name got more room than the interface gives it:\n{text}"
    );
    // Die Ankündigung steht auf stderr, bevor irgendetwas hinausgeht.
    assert!(
        stderr(&output).contains("contacting http://127.0.0.1:11434 now"),
        "{}",
        stderr(&output)
    );

    let json = harness.run(["--json", "llm", "test", "http://127.0.0.1:11434"]);
    assert_eq!(code(&json), 0, "{}", stderr(&json));
    let value = json_of(&json);
    assert_eq!(value["flavor"], "ollama", "{value}");
    assert_eq!(value["models"].as_array().map(Vec::len), Some(2), "{value}");
    assert_eq!(value["endpoint_is_private"], true, "{value}");
}

/// Ein Endpunkt, den der Dienst nicht lesen kann, endet 1 mit seinem Befund --
/// nicht mit einer eigenen Erklärung der Kommandozeile.
#[test]
fn llm_test_an_endpoint_the_daemon_refuses_is_exit_1() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let output = harness.run(["llm", "test", "not a url"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[LLM_007]"), "{text}");
    assert!(stdout(&output).is_empty(), "stdout must stay clean");

    let without = Harness::new();
    let output = without.run(["llm", "test", "http://127.0.0.1:11434"]);
    assert_eq!(code(&output), 2, "{}", stderr(&output));
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
    // `sandbox_required` statt `sandbox_available`: Unter CI ist ein fehlendes
    // `bwrap` ein Fehler und kein Grund zu überspringen. Ein zurückkehrender
    // Test gilt dem Testläufer als bestanden, und dass `humanitl sandbox run`
    // den Exit-Code des Befehls durchreicht, wäre dann nie geprüft worden.
    let Some(_shim) = sandbox_required() else {
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
    let Some(_shim) = sandbox_required() else {
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
    let Some(_shim) = sandbox_required() else {
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

/// Ein Starter, der `SIGINT` ignoriert, macht den Agenten nicht taub.
///
/// `execve` setzt einen Handler zurück, behält aber `SIG_IGN`. Ohne das
/// Zurücksetzen im Shim erbt der Agent das ignorierte Signal, sein `trap` ist
/// nach POSIX wirkungslos, und aus der Bitte wird nach der Frist ein Schlag.
/// Genau so sieht jeder Hintergrundjob einer Shell ohne Job-Control aus, jeder
/// `nohup`-Aufruf und jeder Dienst, den systemd mit ignoriertem `SIGINT`
/// startet — und genau so ist dieser Test am 2026-09-06 rot geworden, bevor
/// `reset_signal_dispositions` da war.
#[test]
fn an_ignored_sigint_of_the_launcher_does_not_reach_the_agent() {
    let Some(_shim) = sandbox_required() else {
        return;
    };
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let _socket = harness.wire_daemon_files();

    // `trap '' INT` setzt `SIG_IGN`, `exec` behält es: Was hier startet, ist
    // dasselbe Binary im selben Prozess, nur mit ignoriertem `SIGINT`.
    let mut child = harness
        .command_of("sh")
        .args([
            "-c",
            "trap '' INT; exec \"$0\" \"$@\"",
            BIN,
            "sandbox",
            "run",
            "--",
            "sh",
            "-c",
            "trap 'exit 42' INT; \
             sed -n 's/^SigIgn:\\t//p' /proc/self/status > /work/sigign; \
             : > /work/ready; sleep 60",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the launcher starts");

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
        "an ignored SIGINT of the launcher must not travel into the sandbox"
    );
    // Nicht nur das Ergebnis, sondern der Zustand: Der Agent hat `SIGINT`
    // nicht in seiner Ignoriermaske. Ohne diese Zusicherung bliebe der Test
    // grün, sobald irgendeine Schicht das Signal zufällig doch zustellt.
    let mask = std::fs::read_to_string(harness.path("work").join("sigign"))
        .expect("the agent wrote its ignore mask");
    let bits = u64::from_str_radix(mask.trim(), 16).expect("the mask is hexadecimal");
    assert_eq!(
        bits & (1 << 1),
        0,
        "SIGINT is in the agent's ignore mask ({mask}); it was inherited"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the agent answered only after the grace of the escalation ({:?}); \
         that means the signal never reached it",
        started.elapsed()
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

/// `sandbox attach` ist ein dünner Client der `Terminal`-RPC (HUM-042).
///
/// Der Fake-Daemon spiegelt die Eingabe zurück; der Test prüft den Weg, nicht
/// den Agenten: Öffnen mit der eigenen Größe, die Bytes des Daemons
/// unverändert auf stdout, und am Ende der Eingabe ein `close`, das den Strom
/// beendet, ohne die Sitzung zu beenden.
///
/// Die Eingabe ist hier kein Terminal, sondern `/dev/null`. Genau deshalb
/// endet der Aufruf: An einem Terminal endet die Eingabe nie.
#[test]
fn sandbox_attach_speaks_the_terminal_rpc() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let mut command = harness.command();
    command
        .args(["sandbox", "attach"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output = command.output().expect("the binary runs");

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("input is echoed"),
        "the bytes of the daemon reach the terminal unchanged: {:?}",
        stdout(&output)
    );
    // Und die Hinweiszeile, die neben den Bytes ankommt: Wer am Terminal
    // sitzt, hat keine zweite Fläche dafür, also schreibt dieser Befehl sie
    // selbst dazwischen -- in ihre eigene Zeile, mit `\r\n` davor und danach
    // (HUM-042). Ohne diese Messung wäre der Rahmen ein Feld, das niemand
    // anzeigt.
    assert!(
        stdout(&output)
            .contains("\r\n[humanitl] fake daemon: a request would wait for you here\r\n"),
        "the notice of the daemon stands in its own line: {:?}",
        stdout(&output)
    );
}

/// `sandbox attach --read-only` zeigt die Sitzung und schickt nichts (HUM-042).
///
/// Der lesende Anschluss ist die Antwort auf `TERM_001`: Wer nicht schreiben
/// darf, soll trotzdem zusehen können. Gemessen wird beides an einem Fake, der
/// jedes empfangene Byte zurückwirft — was der Anschluss sendet, käme also
/// sichtbar zurück. Es kommt nichts zurück.
///
/// Der Aufruf endet hier nicht von selbst: Ohne Eingabestrom schickt er kein
/// `close`, und die Sitzung eines echten Daemons endet mit dem Agenten. Ein
/// Faden liest deshalb mit, bis der Test den Prozess beendet.
///
/// **Alles in einem Faden lesen, nicht zweimal greifen.** Der erste Entwurf
/// las die erste Zeile über einen `BufReader` und danach den Rest über
/// `wait_with_output` — das gab einen leeren Puffer, und die Zusicherung war
/// eine Behauptung über nichts. Die Mutationsprobe (Eingabe auch im lesenden
/// Modus weiterreichen) blieb grün und hat es gezeigt.
#[test]
fn sandbox_attach_read_only_watches_and_sends_nothing() {
    use std::io::{BufRead as _, BufReader, Write as _};

    let harness = Harness::new();
    let _server = FakeServer::start(&harness);

    let mut child = harness
        .command()
        .args(["sandbox", "attach", "--read-only"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary starts");

    let reader = BufReader::new(child.stdout.take().expect("stdout is a pipe"));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let collecting = std::sync::Arc::clone(&seen);
    let reading = std::thread::spawn(move || {
        for line in reader.lines() {
            let Ok(line) = line else { return };
            let mut out = collecting
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            out.push_str(&line);
            out.push('\n');
        }
    });

    // Ein Mensch tippt. Ein lesender Anschluss liest die Tastatur gar nicht
    // erst; käme etwas davon beim Daemon an, wirft der Fake es zurück.
    let mut keys = child.stdin.take().expect("stdin is a pipe");
    let typed = "this must not reach the session";
    // Beide Fehlerwege stehen hier ausdrücklich: Ein verschluckter
    // Schreibfehler machte die Behauptung unten wieder zu einer Aussage über
    // nichts — genau der Fehler, an dem der erste Entwurf dieses Tests scheiterte.
    keys.write_all(format!("{typed}\n").as_bytes())
        .expect("the keyboard reaches the process");
    keys.flush().expect("and it leaves the buffer");

    // Warten, bis die Zeile des Daemons da ist: Ohne sie prüfte der Test die
    // Stille eines Prozesses, der noch gar nichts gesagt hat.
    let deadline = Instant::now() + PATIENCE;
    loop {
        if seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains("input is echoed")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the session's bytes never reached the reader"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    // Zwei Sekunden Zuhören: genug, dass ein weitergereichter Tastendruck den
    // Umlauf über den Fake geschafft hätte.
    std::thread::sleep(Duration::from_secs(2));
    let _ = child.kill();
    let _ = child.wait();
    drop(keys);
    let _ = reading.join();

    let out = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(
        !out.contains(typed),
        "a read-only attach sent the keyboard on: {out:?}"
    );
}

// --- `humanitl daemon install` (HUM-044) -------------------------------------
//
// Der Befehl schreibt eine Datei auf den Rechner eines Menschen, die von da an
// bei jeder Anmeldung einen Dienst startet. Die Tests halten fest, was daran
// haengt: genau eine Datei an einem genannten Ort, sichtbar bevor sie
// geschrieben wird, wiederholbar, nie ueber fremdes Eigentum, nie mit `sudo` —
// und ohne `systemctl` kein `systemctl`.

/// Der Dateiname der Unit, wie `humanitl daemon install` ihn schreibt.
const UNIT_FILE: &str = "humanitld.service";

/// Eine Installation neben einem `humanitld`, das es wirklich gibt.
///
/// `ExecStart` entsteht aus `std::env::current_exe()` und dessen Nachbarn, nie
/// aus `PATH`. Der Test kopiert das Binary deshalb in ein eigenes Verzeichnis
/// und legt einen Nachbarn daneben: So haengt er nicht daran, ob dieser Lauf
/// zufaellig auch `humanitld` gebaut hat, und misst genau die Regel, um die es
/// geht.
fn installed_tree(harness: &Harness) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let bin = harness.path("opt");
    std::fs::create_dir_all(&bin).expect("the install directory");
    let cli = bin.join("humanitl");
    std::fs::copy(BIN, &cli).expect("the binary is copied");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755))
        .expect("0755 on the copy");
    let daemon = bin.join("humanitld");
    std::fs::write(&daemon, b"#!/bin/sh\nexit 0\n").expect("the neighbour");
    std::fs::set_permissions(&daemon, std::fs::Permissions::from_mode(0o755))
        .expect("0755 on the neighbour");
    bin
}

/// Ruft die kopierte Kommandozeile in der Umgebung der Testumgebung auf.
///
/// `PATH` ist leer: Ohne `systemctl` darf der Befehl die Unit schreiben und
/// nichts starten, und das ist genau der Fall, den ein Test ohne laufende
/// Sitzung reproduzierbar herstellen kann.
fn install_run(harness: &Harness, bin: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(args)
        .current_dir(harness.path("work"))
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", "")
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"))
        .env("XDG_DATA_HOME", harness.path("data"))
        .env("XDG_RUNTIME_DIR", harness.path("run"));
    output_when_not_busy(command)
}

/// Startet ein frisch kopiertes Binary und wartet ab, solange der Kernel es
/// noch als beschäftigt meldet.
///
/// `ETXTBSY` ("Text file busy") heißt: Irgendein Prozess hält noch einen
/// Deskriptor zum **Schreiben** auf genau diese Datei, und Linux führt eine
/// Datei nicht aus, die jemand gerade schreibt.
///
/// Neun Tests dieser Datei legen sich das Binary in ein eigenes
/// Wegwerfverzeichnis und führen es dort aus, und der Testläufer von Rust fährt
/// sie als Threads **eines** Prozesses. Kopiert Thread A gerade sein Binary,
/// während Thread B einen Prozess startet, erbt das Kind von B den offenen
/// Schreib-Deskriptor von A: `CLOEXEC` schließt ihn erst beim `exec` des Kindes,
/// nicht schon beim `fork`. In diesem Fenster von wenigen Millisekunden
/// scheitert A beim Ausführen seiner eigenen Kopie mit `ETXTBSY` — an einer
/// Datei, die niemand mehr anfasst.
///
/// Das ist kein Fehler des Programms und keiner des Kernels, sondern die
/// bekannte Verschränkung von `fork` und einer offenen Schreibdatei. Der Kernel
/// gibt die Datei von selbst frei, sobald das fremde Kind sein `exec` hinter
/// sich hat; deshalb wird hier gewartet und nicht umgebaut. Am 2026-09-06 ist
/// genau das einmal von zwei Läufen rot geworden, und ein Test, der unter Last
/// zufällig fällt, sagt über den Code nichts aus.
///
/// Die Obergrenze ist absichtlich endlich: Bleibt die Datei eine Sekunde lang
/// belegt, ist es nicht mehr dieses Fenster, sondern etwas, das jemand ansehen
/// muss.
fn output_when_not_busy(mut command: Command) -> Output {
    const ATTEMPTS: u32 = 50;
    const PAUSE: std::time::Duration = std::time::Duration::from_millis(20);
    for _ in 0..ATTEMPTS {
        match command.output() {
            Ok(output) => return output,
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(PAUSE);
            }
            Err(error) => panic!("the binary runs: {error:?}"),
        }
    }
    panic!(
        "the copied binary stayed busy for {:?}; that is no longer the fork window",
        PAUSE * ATTEMPTS
    )
}

/// Wo die Unit in dieser Umgebung liegt.
fn unit_path(harness: &Harness) -> PathBuf {
    harness.path("config").join("systemd/user").join(UNIT_FILE)
}

#[test]
fn daemon_install_writes_one_unit_into_xdg_config_home() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let output = install_run(&harness, &bin, &["daemon", "install"]);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let unit = unit_path(&harness);
    let text = std::fs::read_to_string(&unit).expect("the unit is written");

    // `ExecStart` nennt den Nachbarn der laufenden Kommandozeile, aufgeloest
    // bis zur wirklichen Datei: `daemon_binary` kanonisiert, damit ein spaeter
    // umgehaengter Verweis nicht aendert, was beim Anmelden startet.
    let expected = std::fs::canonicalize(harness.path("opt").join("humanitld"))
        .expect("the neighbour is there");
    assert!(
        text.contains(&format!("ExecStart={}\n", expected.display())),
        "{text}"
    );
    // Genau eine Datei, und keine Socket-Unit daneben.
    let dir = unit.parent().expect("the unit directory");
    let written: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory is readable")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(written, vec!["humanitld.service".to_owned()], "{written:?}");
    // Nie mit `sudo`: keine Anweisung der Unit ruft es, und der Befehl sagt
    // es auch nicht. Die Kommentarzeilen duerfen das Wort tragen -- dort steht,
    // warum es nirgends steht.
    for line in text.lines() {
        assert!(
            line.trim_start().starts_with('#') || !line.contains("sudo"),
            "a directive of the unit calls sudo: {line}"
        );
    }
    assert!(
        !stderr(&output).contains("sudo humanitl"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn daemon_install_shows_the_unit_before_it_writes_it() {
    let harness = Harness::new();
    // `-q` schaltet die Ankündigung nicht ab: Ein Ausgabeschalter darf nicht
    // bestimmen, ob ein Mensch sieht, welche Datei sein Rechner gleich bekommt.
    let bin = installed_tree(&harness);
    let output = install_run(&harness, &bin, &["-q", "daemon", "install", "--print"]);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let announced = stderr(&output);
    assert!(announced.contains("ExecStart="), "{announced}");
    assert!(announced.contains("WantedBy=default.target"), "{announced}");
    assert!(
        announced.contains(&unit_path(&harness).display().to_string()),
        "{announced}"
    );
    // `--print` schreibt nichts.
    assert!(!unit_path(&harness).exists(), "--print writes nothing");

    // Unter `--json` liest ein Programm: Dasselbe steht im einen Objekt auf
    // `stdout`, und `stderr` bleibt leer (`docs/cli.md`).
    let output = install_run(&harness, &bin, &["--json", "daemon", "install", "--print"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&output).trim()).expect("one JSON value");
    assert_eq!(value["action"], "print");
    let text = value["unit_text"].as_str().expect("the unit text");
    assert!(text.contains("ExecStart="), "{text}");
    assert!(text.contains("WantedBy=default.target"), "{text}");
}

#[test]
fn daemon_install_twice_writes_nothing_the_second_time() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let first = install_run(&harness, &bin, &["--json", "daemon", "install"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let first: serde_json::Value =
        serde_json::from_str(stdout(&first).trim()).expect("one JSON value");
    assert_eq!(first["action"], "created");

    let second = install_run(&harness, &bin, &["--json", "daemon", "install"]);
    assert_eq!(code(&second), 0, "{}", stderr(&second));
    let second: serde_json::Value =
        serde_json::from_str(stdout(&second).trim()).expect("one JSON value");
    assert_eq!(second["action"], "unchanged");
}

#[test]
fn daemon_install_refuses_a_unit_it_did_not_write() {
    let harness = Harness::new();
    let unit = unit_path(&harness);
    std::fs::create_dir_all(unit.parent().expect("the directory")).expect("the directory");
    let theirs = "[Service]\nExecStart=/usr/local/bin/humanitld --fake\n";
    std::fs::write(&unit, theirs).expect("their unit");

    let bin = installed_tree(&harness);
    let output = install_run(&harness, &bin, &["daemon", "install"]);

    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(
        stderr(&output).contains("DAEMON_005"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(&unit).expect("still there"),
        theirs,
        "the file of somebody else is not touched"
    );
}

#[test]
fn daemon_install_without_systemctl_leaves_the_unit_and_starts_nothing() {
    let harness = Harness::new();
    // `PATH` ist leer, also gibt es kein `systemctl`.
    let bin = installed_tree(&harness);
    let output = install_run(&harness, &bin, &["--json", "daemon", "install"]);

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&output).trim()).expect("one JSON value");
    assert_eq!(value["activation"], "no systemctl");
    assert!(unit_path(&harness).is_file(), "the unit is in place anyway");
    // Unter `--json` steht das im Objekt; `stderr` bleibt leer.
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));

    // Ohne `--json` sagt die Ankündigung es einem Menschen.
    let output = install_run(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("no systemctl in PATH"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn daemon_install_refuses_when_no_humanitld_lies_next_to_it() {
    let harness = Harness::new();
    let bin = harness.path("lonely");
    std::fs::create_dir_all(&bin).expect("the directory");
    std::fs::copy(BIN, bin.join("humanitl")).expect("the binary is copied");

    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(["daemon", "install"])
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", "")
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"));
    let output = output_when_not_busy(command);

    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(
        stderr(&output).contains("DAEMON_007"),
        "{}",
        stderr(&output)
    );
    assert!(!unit_path(&harness).exists(), "nothing is left behind");
}

/// Ein `systemctl`, das die Aktivierung anlegt und dann scheitert.
///
/// Genau der Fall, um den es geht: `systemctl --user enable --now` ist **ein**
/// Aufruf mit zwei Schritten. Es legt die Verweise unter `<ziel>.wants/` an und
/// startet dann den Dienst; misslingt der Start, ist der Aufruf rot und die
/// Verweise stehen trotzdem da. Das Skript tut beides.
///
/// Der Pfad des Unit-Verzeichnisses und der des Protokolls stehen woertlich im
/// Skript, weil `daemon install` `systemctl` mit `env_clear()` und nur vier
/// Variablen ruft; `XDG_CONFIG_HOME` gehoert nicht dazu. Aus demselben Grund
/// setzt das Skript seinen eigenen `PATH`: Der geerbte enthaelt nur das
/// Verzeichnis dieses Skripts, also weder `mkdir` noch `ln`.
///
/// Das Protokoll ist der Grund, aus dem der Test nicht leer gruen werden kann:
/// Es haelt fest, dass der Verweis wirklich entstanden ist. Ohne diese Zeile
/// bestuende der Test auch dann, wenn das Skript nie einen angelegt haette.
fn fake_systemctl(harness: &Harness, unit_dir: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;

    let bin = harness.path("fakebin");
    std::fs::create_dir_all(&bin).expect("the fake bin directory");
    let log = harness.path("systemctl.log");
    let link = unit_dir.join("default.target.wants").join(UNIT_FILE);
    let script = format!(
        "#!/bin/sh\n\
         PATH=/usr/bin:/bin\n\
         export PATH\n\
         echo \"$*\" >>'{log}'\n\
         case \"$*\" in\n\
         *enable*)\n\
           mkdir -p '{wants}' || exit 90\n\
           ln -sf '{unit}' '{link}' || exit 91\n\
           test -L '{link}' || exit 92\n\
           echo 'created the wants link' >>'{log}'\n\
           echo 'Job for humanitld.service failed because the control process exited' >&2\n\
           exit 1\n\
           ;;\n\
         esac\n\
         exit 0\n",
        log = log.display(),
        wants = unit_dir.join("default.target.wants").display(),
        unit = unit_dir.join(UNIT_FILE).display(),
        link = link.display(),
    );
    let path = bin.join("systemctl");
    std::fs::write(&path, script).expect("the fake systemctl");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("0755");
    (bin, log)
}

/// Was das gefaelschte `systemctl` protokolliert hat.
fn fake_systemctl_log(log: &Path) -> String {
    std::fs::read_to_string(log).unwrap_or_default()
}

/// Wie [`install_run`], aber mit einem `PATH`, in dem ein `systemctl` liegt.
fn install_run_with_path(harness: &Harness, bin: &Path, path: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(args)
        .current_dir(harness.path("work"))
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", path)
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"))
        .env("XDG_DATA_HOME", harness.path("data"))
        .env("XDG_RUNTIME_DIR", harness.path("run"));
    output_when_not_busy(command)
}

/// Der Verweis, mit dem systemd die Unit beim Anmelden startet.
fn wants_link(harness: &Harness) -> PathBuf {
    harness
        .path("config")
        .join("systemd/user/default.target.wants/humanitld.service")
}

/// Ein gescheitertes `enable --now` laesst weder die Unit noch ihre Aktivierung
/// liegen.
///
/// Ohne die Ruecknahme der Verweise waere `daemon install` nicht das eine
/// Geschaeft, als das `docs/cli.md` es beschreibt: Der Befehl endete mit
/// `DAEMON_008`, die Unit-Datei verschwaende — und der Dienst startete beim
/// naechsten Anmelden trotzdem, weil der Verweis im `default.target.wants`
/// stehen bliebe.
#[test]
fn a_failed_enable_takes_the_wants_link_back_as_well() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let unit = unit_path(&harness);
    let unit_dir = unit.parent().expect("the unit directory").to_path_buf();
    let (fake, log) = fake_systemctl(&harness, &unit_dir);

    let output = install_run_with_path(&harness, &bin, &fake, &["daemon", "install"]);

    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(
        stderr(&output).contains("DAEMON_008"),
        "{}",
        stderr(&output)
    );
    // Das Skript hat den Verweis wirklich angelegt. Ohne diese Zeile waere der
    // Test gruen, auch wenn nie einer entstanden ist.
    let log = fake_systemctl_log(&log);
    assert!(
        log.contains("created the wants link"),
        "the fake systemctl never created an enablement link: {log}"
    );
    // Und der Dienst wird angehalten, bevor die Datei verschwindet. `enable
    // --now` startet die Unit; scheitert sie dabei, steht sie auf `failed`, und
    // `Restart=on-failure` versucht es weiter. Eine Ruecknahme, die nur die
    // Datei wegnimmt, laesst systemd auf eine Unit neu starten, die es nicht
    // mehr gibt.
    assert!(
        log.contains("--user stop humanitld.service"),
        "the failed unit is stopped before it is taken back: {log}"
    );
    assert!(
        log.contains("--user reset-failed humanitld.service"),
        "the failed state is cleared, not left in systemd's memory: {log}"
    );
    assert!(!unit.exists(), "the unit is taken back: {}", unit.display());
    assert!(
        wants_link(&harness).symlink_metadata().is_err(),
        "the enablement link of this run stays behind: {}",
        wants_link(&harness).display()
    );
    // Und gar nichts sonst: kein leeres `default.target.wants`, keine
    // Nachbardatei des Schreibvorgangs.
    let leftovers: Vec<PathBuf> = std::fs::read_dir(&unit_dir)
        .expect("the directory is readable")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    assert!(leftovers.is_empty(), "nothing stays behind: {leftovers:?}");
}

/// Angehalten wird nur, was dieser Lauf gestartet haben kann.
///
/// Scheitert schon `daemon-reload`, hat `daemon install` nichts gestartet. Ein
/// `stop` traefe dann den Daemon, den der Mensch vorher selbst laufen hatte,
/// und etwas anzuhalten, das man nicht gestartet hat, ist schlimmer als ein
/// Rest im Gedaechtnis von systemd.
#[test]
fn a_failed_reload_stops_nothing() {
    use std::os::unix::fs::PermissionsExt as _;

    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let fakebin = harness.path("fakebin");
    std::fs::create_dir_all(&fakebin).expect("the fake bin directory");
    let log = harness.path("systemctl.log");
    // Dieses `systemctl` scheitert am ersten Schritt und legt nie einen
    // Verweis an.
    let script = format!(
        "#!/bin/sh\n\
         echo \"$*\" >>'{log}'\n\
         case \"$*\" in\n\
         *daemon-reload*)\n\
           echo 'Failed to reload daemon' >&2\n\
           exit 1\n\
           ;;\n\
         esac\n\
         exit 0\n",
        log = log.display(),
    );
    let path = fakebin.join("systemctl");
    std::fs::write(&path, script).expect("the fake systemctl");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("0755 on the fake");

    let output = install_run_with_path(&harness, &bin, &fakebin, &["daemon", "install"]);

    assert_ne!(code(&output), 0, "{}", stdout(&output));
    let log = fake_systemctl_log(&log);
    assert!(
        log.contains("--user daemon-reload"),
        "the run really tried the reload: {log}"
    );
    assert!(
        !log.contains("--user stop"),
        "nothing this run did not start is stopped: {log}"
    );
    assert!(
        !log.contains("--user enable"),
        "the enable never runs after a failed reload: {log}"
    );
}

/// Zurueckgenommen wird nur, was dieser Lauf angelegt hat.
///
/// Wer den Dienst schon vorher aktiviert hatte, behaelt die Aktivierung, auch
/// wenn `daemon install` scheitert. Und die aeltere eigene Unit bekommt ihren
/// Inhalt zurueck, statt zu verschwinden.
#[test]
fn a_failed_enable_keeps_an_enablement_that_was_there_before() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let unit = unit_path(&harness);
    let unit_dir = unit.parent().expect("the unit directory").to_path_buf();
    std::fs::create_dir_all(unit_dir.join("default.target.wants")).expect("the wants directory");
    let older = "# humanitl daemon install: written by Humanitl\n                 [Service]\nExecStart=/old/humanitld\n";
    std::fs::write(&unit, older).expect("our older unit");
    std::os::unix::fs::symlink(&unit, wants_link(&harness)).expect("the enablement from before");

    let (fake, log) = fake_systemctl(&harness, &unit_dir);
    let output = install_run_with_path(&harness, &bin, &fake, &["daemon", "install"]);

    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(
        stderr(&output).contains("DAEMON_008"),
        "{}",
        stderr(&output)
    );
    let log = fake_systemctl_log(&log);
    assert!(
        log.contains("created the wants link"),
        "the fake systemctl never touched the enablement: {log}"
    );
    assert!(
        wants_link(&harness).symlink_metadata().is_ok(),
        "an enablement from before this run is not taken away"
    );
    assert_eq!(
        std::fs::read_to_string(&unit).expect("still there"),
        older,
        "the older version of our own unit comes back"
    );
}

/// Die Ankuendigung sagt, was geschieht, und nicht, was ein Aufruf im Regelfall
/// taete.
///
/// Ein zweiter `daemon install` schreibt nichts (`unchanged`). Sagte die
/// Ankuendigung trotzdem „writes <pfad>", stuende auf `stderr` ein Satz, den
/// derselbe Lauf gleich widerlegt.
#[test]
fn the_announcement_of_a_second_install_says_that_nothing_is_written() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let unit = unit_path(&harness);
    let writes = format!("humanitl daemon install writes {}:", unit.display());

    let first = install_run(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    assert!(
        stderr(&first).contains(&writes),
        "the first call writes the file and says so: {}",
        stderr(&first)
    );

    let second = install_run(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&second), 0, "{}", stderr(&second));
    assert!(
        !stderr(&second).contains(&writes),
        "the second call writes nothing and may not say it does: {}",
        stderr(&second)
    );
    assert!(
        stderr(&second).contains(&format!(
            "humanitl daemon install writes nothing: {} already carries exactly this:",
            unit.display()
        )),
        "{}",
        stderr(&second)
    );
    // Der ganze Text der Unit steht trotzdem da: Wer den Befehl ruft, soll
    // sehen, was auf seiner Platte liegt.
    assert!(
        stderr(&second).contains("ExecStart="),
        "{}",
        stderr(&second)
    );
}

/// Vor einer fremden Unit wird nichts angekuendigt, was dann nicht geschieht.
///
/// `DAEMON_005` heisst: Die Datei gehoert jemand anderem und wird nicht
/// angefasst. Ein „humanitl daemon install writes <pfad>" davor waere die
/// Sorte Satz, gegen die dieses Produkt gebaut ist.
#[test]
fn a_foreign_unit_is_refused_without_announcing_a_write() {
    let harness = Harness::new();
    let unit = unit_path(&harness);
    std::fs::create_dir_all(unit.parent().expect("the directory")).expect("the directory");
    std::fs::write(
        &unit,
        "[Service]\nExecStart=/usr/local/bin/humanitld --fake\n",
    )
    .expect("their unit");

    let bin = installed_tree(&harness);
    let output = install_run(&harness, &bin, &["daemon", "install"]);

    assert_ne!(code(&output), 0, "{}", stdout(&output));
    let announced = stderr(&output);
    assert!(announced.contains("DAEMON_005"), "{announced}");
    assert!(
        !announced.contains("humanitl daemon install writes"),
        "nothing is written, so nothing announces a write: {announced}"
    );
    assert!(
        !announced.contains("enable --now"),
        "nothing is started, so nothing announces a start: {announced}"
    );
}

// --- humanitl flows decide --remember (HUM-095) -----------------------------

/// Der Flow, den die aufgezeichnete Sitzung nach 450 ms hält.
const HELD_FLOW: &str = "018f0001-0000-7000-8000-000000010000";

/// Wartet, bis der Abspieler diesen Flow hält, und gibt seine Id zurück.
///
/// Der Fake spielt `fixtures/sessions/mixed.jsonl` in Echtzeit ab; vor der
/// `hold`-Zeile wartet nichts, und ein Test, der sofort entscheidet, prüfte den
/// leeren Fall.
fn wait_for_held_flow(harness: &Harness) -> String {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if held_ids(harness).iter().any(|id| id == HELD_FLOW) {
            return HELD_FLOW.to_owned();
        }
        assert!(
            Instant::now() < deadline,
            "the recorded session held no flow {HELD_FLOW}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Die Ids aller Flows, die gerade warten.
fn held_ids(harness: &Harness) -> Vec<String> {
    let output = harness.run(["--json", "flows", "list", "state:held"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    json_of(&output)["flows"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|flow| flow["flow_id"].as_str().map(str::to_owned))
        .collect()
}

/// Alle Regeln, die der Daemon führt, mitgelieferte eingeschlossen.
///
/// Sitzungsregeln stehen im Speicher und nie in `rules.yaml` (CONVENTIONS 4.5);
/// `rules list --json` ist deshalb der einzige Ort, an dem ein Test sie sieht.
fn rules_of(harness: &Harness) -> Vec<serde_json::Value> {
    let output = harness.run(["--json", "rules", "list", "--all"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    json_of(&output)["rules"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Die Regel mit dieser Id, wie der Daemon sie führt.
fn rule_with_id(harness: &Harness, rule_id: &str) -> serde_json::Value {
    rules_of(harness)
        .into_iter()
        .find(|rule| rule["rule_id"] == rule_id)
        .unwrap_or_else(|| panic!("the daemon lists a rule {rule_id}"))
}

/// Die Regel aus einer Freigabe nennt die Anfrage, aus der sie entstand.
///
/// Ohne sie ist die Sitzungsregel der Kommandozeile von einer handgeschriebenen
/// nicht zu unterscheiden, und das Abzeichen „from {id}" des Regel-Bildschirms
/// hat nichts anzuzeigen (ADR-0007, `rules.proto` Feld 6).
#[test]
fn decide_remember_carries_origin() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let id = wait_for_held_flow(&harness);

    let output = harness.run([
        "--json",
        "flows",
        "decide",
        &id,
        "allow",
        "--remember",
        "**.npmjs.org",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);
    assert_eq!(value["created_rule"]["created_from_flow_id"], id, "{value}");
    assert_eq!(value["created_rule"]["action"], "allow", "{value}");
    assert_eq!(value["created_rule"]["host"], "**.npmjs.org", "{value}");

    let rule_id = value["created_rule_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!rule_id.is_empty(), "the daemon names the rule: {value}");
    assert_eq!(value["created_rule"]["rule_id"], rule_id, "{value}");

    // Nicht nur die Antwort, auch der Regelbestand trägt die Herkunft: Die
    // Antwort könnte eine Behauptung über etwas sein, das gar nicht abgelegt
    // wurde.
    let listed = rule_with_id(&harness, &rule_id);
    assert_eq!(listed["created_from_flow_id"], id, "{listed}");
    assert_eq!(listed["host"], "**.npmjs.org", "{listed}");

    // Im Klartext steht die Regel in einer zweiten Zeile; wer ohne `--json`
    // arbeitet, soll nicht erst `rules list` fragen müssen, was entstanden ist.
    let plain = Harness::new();
    let _plain_server = FakeServer::start(&plain);
    let plain_id = wait_for_held_flow(&plain);
    let output = plain.run([
        "flows",
        "decide",
        &plain_id,
        "allow",
        "--remember",
        "**.npmjs.org",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let lines: Vec<String> = stdout(&output).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], "allow 018f0001", "{lines:?}");
    let rule_line = &lines[1];
    assert!(
        rule_line.starts_with("rule "),
        "the second line names the rule: {rule_line}"
    );
    assert!(
        rule_line.ends_with(" allow **.npmjs.org session"),
        "with action, host and expiry: {rule_line}"
    );
}

/// Ohne `--remember` ist die Anfrage die von vorher, und es entsteht nichts.
///
/// Genau die vier Schlüssel wie vor HUM-095, kein `created_rule_id`, kein
/// `created_rule`, und dieselbe Zahl Regeln wie davor.
#[test]
fn decide_without_remember_creates_no_rule() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let id = wait_for_held_flow(&harness);
    let before = rules_of(&harness).len();

    let output = harness.run(["--json", "flows", "decide", &id, "allow"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);

    let mut keys: Vec<String> = value
        .as_object()
        .expect("one JSON object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(keys, ["applied", "decision", "flow_id", "note"], "{value}");
    assert_eq!(value["flow_id"], id, "{value}");
    assert_eq!(value["applied"], true, "{value}");

    assert_eq!(
        rules_of(&harness).len(),
        before,
        "a decision without --remember adds no rule"
    );
}

/// Ohne `--remember-expires` gilt die Regel für die Sitzung, nicht für immer.
///
/// Ein leeres `expires` liest `expiry_from_proto` als `never`; eine dauerhafte
/// Regel als Nebenwirkung einer einzelnen Freigabe ist genau die Überraschung,
/// die das Produkt nicht macht. Mit dem Flag steht darin, was gefordert wurde —
/// der Vorgabewert ist eine Vorgabe und keine Konstante.
#[test]
fn decide_remember_defaults_to_session() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let id = wait_for_held_flow(&harness);

    let output = harness.run([
        "--json",
        "flows",
        "decide",
        &id,
        "allow",
        "--remember",
        "**.npmjs.org",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);
    let rule_id = value["created_rule_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        value["created_rule"]["expires"]["kind"], "session",
        "{value}"
    );

    let listed = rule_with_id(&harness, &rule_id);
    assert_eq!(listed["expires"]["kind"], "session", "{listed}");
    assert_eq!(listed["created_from_flow_id"], id, "{listed}");

    // Und mit dem Flag das, was darin steht.
    let other = Harness::new();
    let _other_server = FakeServer::start(&other);
    let other_id = wait_for_held_flow(&other);
    let output = other.run([
        "--json",
        "flows",
        "decide",
        &other_id,
        "allow",
        "--remember",
        "**.npmjs.org",
        "--remember-expires",
        "never",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);
    assert_eq!(value["created_rule"]["expires"]["kind"], "never", "{value}");
}

/// `--note` und `--remember-note` kreuzen sich nicht.
///
/// `--note` ist die Begründung an den Agenten im 403-Body (HUM-072),
/// `--remember-note` die Notiz der Regel. Geprüft wird zugleich, dass Methode
/// und Pfad in der Regel ankommen.
#[test]
fn decide_remember_note_is_not_the_agent_note() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let id = wait_for_held_flow(&harness);

    let output = harness.run([
        "--json",
        "flows",
        "decide",
        &id,
        "block",
        "--remember",
        "**.evil.example",
        "--note",
        "use PyPI",
        "--remember-note",
        "blocked group",
        "--remember-method",
        "GET",
        "--remember-path",
        "/packages/**",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value = json_of(&output);
    let rule_id = value["created_rule_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    let listed = rule_with_id(&harness, &rule_id);
    assert_eq!(listed["action"], "block", "{listed}");
    assert_eq!(listed["note"], "blocked group", "{listed}");
    assert_ne!(listed["note"], "use PyPI", "{listed}");
    assert_eq!(listed["methods"], serde_json::json!(["GET"]), "{listed}");
    assert_eq!(listed["path"], "/packages/**", "{listed}");
    // Die Notiz an den Agenten bleibt, wo sie hingehört.
    assert_eq!(value["note"], "use PyPI", "{value}");
}

/// Ein Muster, das der Daemon nicht lesen kann, entscheidet nichts.
///
/// Der Dienst legt die Regel vor der Entscheidung an; scheitert sie, ist der
/// ganze Aufruf gescheitert, und der Flow wartet weiter.
#[test]
fn decide_remember_bad_pattern_decides_nothing() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    let id = wait_for_held_flow(&harness);
    let before = rules_of(&harness).len();

    let output = harness.run([
        "flows",
        "decide",
        &id,
        "allow",
        "--remember",
        "cidr:not-an-address",
    ]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    // Der Befund kommt vom Daemon und nennt das Muster; die Kommandozeile
    // erklärt Host-Muster nicht selbst (`docs/ARCHITECTURE.md` 4).
    assert!(text.contains("RULES_003"), "{text}");
    assert!(text.contains("cidr:not-an-address"), "{text}");

    assert_eq!(rules_of(&harness).len(), before, "no rule was added");
    assert!(
        held_ids(&harness).iter().any(|held| held == &id),
        "the flow is still waiting, so nobody decided it"
    );
}

/// Eine unlesbare Flow-Id bleibt `IPC_004` über die Id.
///
/// Die Kommandozeile schickt dann keine Regel mit: Sonst wiese
/// `rule_from_proto` `created_from_flow_id` mit `IPC_005` ab, und der Befund
/// spräche über ein Feld, das der Aufrufer nie gesetzt hat.
#[test]
fn decide_remember_bad_flow_id_keeps_ipc_004() {
    let harness = Harness::new();
    let _server = FakeServer::start(&harness);
    wait_for_held_flow(&harness);
    let before = rules_of(&harness).len();

    let output = harness.run([
        "flows",
        "decide",
        "not-a-uuid",
        "allow",
        "--remember",
        "**.example.com",
    ]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("IPC_004"), "{text}");
    assert!(!text.contains("IPC_005"), "{text}");
    assert!(text.contains("not-a-uuid"), "{text}");

    assert_eq!(rules_of(&harness).len(), before, "no rule was added");
}

/// Die Zusatz-Flags gibt es nur zusammen mit `--remember`.
///
/// Ohne das Muster gäbe es nichts, woran sie hingen; `clap` sagt das, bevor
/// irgendjemand den Daemon fragt.
#[test]
fn remember_flags_without_a_pattern_are_a_usage_error() {
    let harness = Harness::new();
    for flag in [
        vec!["--remember-method", "GET"],
        vec!["--remember-path", "/x/**"],
        vec!["--remember-expires", "never"],
        vec!["--remember-note", "why"],
    ] {
        let mut args = vec!["flows", "decide", HELD_FLOW, "allow"];
        args.extend_from_slice(&flag);
        let output = harness.run(args.clone());
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{flag:?}: {text}");
        assert!(text.starts_with("error[CLI_004]: "), "{flag:?}: {text}");
    }
}

// --- `humanitl config`, `humanitl audit`, `humanitl daemon` (HUM-070) --------
//
// Die drei Unterkommandos aus CONVENTIONS.md 3.8, die bis hierher Platzhalter
// waren. Was daran haengt: eine Datei des Menschen wird geschrieben (`config
// set`), eine Sicherheitsaussage wird geprueft (`audit verify`), und ein
// Dienst wird eingerichtet (`daemon install`). Jede dieser drei Zusagen hat
// unten ihren eigenen Test.

/// Der Schlüssel, mit dem die Ketten dieser Tests versiegelt sind.
///
/// Ein fester Schlüssel und kein zufälliger: Die Tests prüfen die Kette und
/// die Kanonik, nicht die MACs — `audit verify --file` hat den Schlüssel
/// ohnehin nicht, und ein zufälliger machte die Datei von Lauf zu Lauf anders,
/// ohne dass ein Test mehr sähe.
const AUDIT_KEY: [u8; 32] = [7_u8; 32];

/// Schreibt eine gültige Kette mit `records` Zeilen und gibt ihren Pfad zurück.
fn audit_chain(path: &Path, records: u64) {
    use humanitl_audit::{GENESIS_PREV, NO_SESSION, RecordBody};

    std::fs::create_dir_all(path.parent().expect("the log has a directory"))
        .expect("the audit directory");
    let mut file = std::fs::File::create(path).expect("the audit log");
    let mut prev = GENESIS_PREV.to_owned();
    for seq in 1..=records {
        let record = RecordBody {
            seq,
            ts: format!("2026-09-02T10:{seq:02}:00.000000Z"),
            session: NO_SESSION.to_owned(),
            kind: "flow.decided".to_owned(),
            data: serde_json::json!({ "n": seq, "note": "a, \"quoted\" note" }),
            prev: prev.clone(),
        }
        .seal(&AUDIT_KEY)
        .expect("the record seals");
        prev.clone_from(&record.hash);
        file.write_all(&record.to_line().expect("the canonical line"))
            .expect("the line is written");
        file.write_all(b"\n").expect("the newline is written");
    }
    file.flush().expect("the log is flushed");
}

/// `config get` ohne Schlüssel: die Tabelle und dasselbe als ein JSON-Objekt.
#[test]
fn config_get_table_and_json() {
    let harness = Harness::new();

    let table = harness.run(["config", "get"]);
    assert_eq!(code(&table), 0, "{}", stderr(&table));
    let text = stdout(&table);
    let head = text.lines().next().unwrap_or_default();
    assert!(head.starts_with("KEY"), "{text}");
    assert!(head.contains("VALUE"), "{text}");
    assert!(
        !head.contains("ORIGIN"),
        "the origin is a column of its own: {text}"
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with("hold.timeout_secs") && line.contains("300")),
        "{text}"
    );
    // Keine Zeile endet auf Leerraum: die Tabelle geht auch durch eine Pipe.
    assert!(text.lines().all(|line| !line.ends_with(' ')), "{text}");

    let with_origin = harness.run(["config", "get", "--origin"]);
    assert_eq!(code(&with_origin), 0, "{}", stderr(&with_origin));
    let text = stdout(&with_origin);
    assert!(
        text.lines().next().unwrap_or_default().contains("ORIGIN"),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with("hold.timeout_secs") && line.contains("default")),
        "{text}"
    );

    let json = harness.run(["--json", "config", "get"]);
    assert_eq!(code(&json), 0, "{}", stderr(&json));
    let body = stdout(&json);
    assert_eq!(body.lines().count(), 1, "one JSON value per call: {body}");
    let value: serde_json::Value = serde_json::from_str(body.trim()).expect("one JSON value");
    let values = value["values"].as_array().expect("an array of leaves");
    let timeout = values
        .iter()
        .find(|row| row["key"] == "hold.timeout_secs")
        .expect("hold.timeout_secs is a leaf");
    assert_eq!(timeout["value"], 300);
    assert_eq!(timeout["origin"], "default");
    assert!(stderr(&json).is_empty(), "stderr must stay clean");
}

/// `config set hold.timeout_secs 5m` schreibt 300 und lässt die Datei sonst,
/// wie sie war.
#[test]
fn config_set_duration_parsing() {
    let harness = Harness::new();
    let file = harness.path("config/humanitl/config.toml");
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    std::fs::write(
        &file,
        "# Der Kommentar eines Menschen.\n[hold]\ntimeout_secs = 42 # mit Notiz\n",
    )
    .expect("the config file");

    let output = harness.run(["config", "set", "hold.timeout_secs", "5m"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "hold.timeout_secs = 300 (global)",
        "{}",
        stderr(&output)
    );

    let written = std::fs::read_to_string(&file).expect("the file is still there");
    assert!(written.contains("timeout_secs = 300"), "{written}");
    assert!(
        written.contains("# Der Kommentar eines Menschen."),
        "the comment of a human survives: {written}"
    );
    assert!(
        written.contains("# mit Notiz"),
        "the note behind the value survives: {written}"
    );

    let back = harness.run(["config", "get", "hold.timeout_secs"]);
    assert_eq!(stdout(&back).trim(), "300", "{}", stderr(&back));

    // Zweimal derselbe Wert schreibt nicht zweimal.
    let again = harness.run(["--json", "config", "set", "hold.timeout_secs", "300"]);
    let value: serde_json::Value =
        serde_json::from_str(stdout(&again).trim()).expect("one JSON value");
    assert_eq!(value["written"], "unchanged");
    assert_eq!(value["value"], 300);

    // Und eine Größe nimmt ihre Einheit.
    let bytes = harness.run([
        "--json",
        "config",
        "set",
        "limits.hold_body_cap_bytes",
        "2MiB",
    ]);
    assert_eq!(code(&bytes), 0, "{}", stderr(&bytes));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&bytes).trim()).expect("one JSON value");
    assert_eq!(value["value"], 2 * 1024 * 1024);
}

/// Ein Wert, den das Schema nicht kennt, endet mit 1 und einem Befund — und
/// die Datei bleibt unberührt.
///
/// Der Code ist `CONFIG_003` („Wert außerhalb des Bereichs") und nicht das
/// `CONFIG_001` aus der Spezifikation von HUM-070: `CONFIG_001` heißt in
/// diesem Register „Config-Datei ungültig", und die Datei ist hier in Ordnung.
/// `backlog/CONVENTIONS.md` 4.6 ist für Namen die jüngere Quelle.
#[test]
fn config_set_invalid_exit_1_with_config_003() {
    let harness = Harness::new();
    let file = harness.path("config/humanitl/config.toml");

    let output = harness.run(["config", "set", "hold.ask_mode", "banana"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CONFIG_003]: "), "{text}");
    // Der Zweig der Aufzählung hat abgelehnt, nicht das Lesen des Typs: Nur
    // er nennt die erlaubten Werte und schlägt einen davon vor.
    assert!(
        text.contains("banana is not a value of hold.ask_mode; it takes one of ui, terminal, none"),
        "{text}"
    );
    assert!(
        text.contains("\n  fix: humanitl config set hold.ask_mode ui\n"),
        "{text}"
    );
    assert!(!file.exists(), "nothing is written when nothing is valid");

    // Auch die Untergrenze einer Zahl ist ein Befund und keine stille 0. Sie
    // steht nicht im Schema, sondern in der Prüfung von `humanitl-config`;
    // `config set` legt den Wert deshalb einmal als Ebene auf und löst auf,
    // statt den Bereich ein zweites Mal aufzuschreiben.
    let below = harness.run(["config", "set", "hold.timeout_secs", "0"]);
    let text = stderr(&below);
    assert_eq!(code(&below), 1, "{text}");
    assert!(text.starts_with("error[CONFIG_003]: "), "{text}");
    assert!(text.contains("hold.timeout_secs"), "{text}");
    assert!(!file.exists(), "{text}");

    // Und ein Schlüssel, den es nicht gibt, bleibt `CONFIG_002`.
    let unknown = harness.run(["config", "set", "hold.nonsense", "1"]);
    assert_eq!(code(&unknown), 1);
    assert!(
        stderr(&unknown).starts_with("error[CONFIG_002]: "),
        "{}",
        stderr(&unknown)
    );
}

/// `config schema` ist ein JSON-Schema, und jedes Blatt trägt seine Stufe.
///
/// Geprüft wird die Form und nicht mit der Crate `jsonschema`: Sie steht nicht
/// unter den Abhängigkeiten des Workspace, und das Schema kommt ohnehin aus
/// `schemars`. Was ein Test hier wirklich halten kann, ist die Zusage aus
/// ADR-011: ein Dokument mit `$schema`, Objekten bis zum Blatt und `x-tier` an
/// jedem Blatt.
#[test]
fn config_schema_is_valid_json_schema() {
    let harness = Harness::new();
    let output = harness.run(["--json", "config", "schema"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    let schema: serde_json::Value =
        serde_json::from_str(stdout(&output).trim()).expect("one JSON value");
    assert!(
        schema["$schema"]
            .as_str()
            .is_some_and(|text| text.contains("json-schema.org")),
        "{schema}"
    );
    assert_eq!(schema["type"], "object");

    let mut leaves = 0;
    schema_walk(&schema, "root", &mut leaves);
    assert!(leaves > 20, "only {leaves} leaves in the schema");

    // Ohne `--json` dasselbe Dokument, nur eingerückt.
    let pretty = harness.run(["config", "schema"]);
    assert_eq!(code(&pretty), 0);
    let same: serde_json::Value =
        serde_json::from_str(&stdout(&pretty)).expect("the pretty form is JSON too");
    assert_eq!(same, schema);
}

/// Läuft durch ein JSON-Schema bis zu den Blättern und prüft jedes davon.
fn schema_walk(node: &serde_json::Value, path: &str, leaves: &mut usize) {
    let object = node
        .as_object()
        .unwrap_or_else(|| panic!("{path} is no schema object"));
    if let Some(properties) = object.get("properties").and_then(|value| value.as_object()) {
        assert!(!properties.is_empty(), "{path} has an empty properties");
        for (name, child) in properties {
            schema_walk(child, &format!("{path}.{name}"), leaves);
        }
        return;
    }
    *leaves += 1;
    assert!(
        object.contains_key("x-tier"),
        "{path} is a leaf without x-tier"
    );
    assert!(
        object.contains_key("type")
            || object.contains_key("enum")
            || object.contains_key("anyOf")
            || object.contains_key("oneOf"),
        "{path} is a leaf without a type"
    );
}

/// Eine heile Kette endet mit 0 und sagt, was sie nicht geprüft hat.
#[test]
fn audit_verify_ok_exit_0() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 4);

    let output = harness.run(["audit", "verify", "--file", &log.display().to_string()]);
    let text = stdout(&output);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(text.contains("audit chain: OK"), "{text}");
    assert!(text.contains("records:     4"), "{text}");
    assert!(text.contains("head:"), "{text}");
    // Die schwächere Prüfung sagt, dass sie die schwächere ist.
    assert!(text.contains("no HMAC key (file mode)"), "{text}");
    assert!(text.contains("no anchors (file mode)"), "{text}");

    // Ohne Daemon und ohne `--file` fällt der Befehl auf die Datei zurück und
    // sagt auch das.
    let fallback = harness.run(["audit", "verify"]);
    assert_eq!(code(&fallback), 0, "{}", stderr(&fallback));
    assert!(
        stdout(&fallback).contains("audit chain: OK"),
        "{}",
        stdout(&fallback)
    );
    assert!(
        stdout(&fallback).contains("no HMAC key (file mode)"),
        "{}",
        stdout(&fallback)
    );

    let json = harness.run([
        "--json",
        "audit",
        "verify",
        "--file",
        &log.display().to_string(),
    ]);
    assert_eq!(code(&json), 0, "{}", stderr(&json));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&json).trim()).expect("one JSON value");
    assert_eq!(value["chain"], "ok");
    assert_eq!(value["mode"], "file");
    assert_eq!(value["anchors"], "not_checked");
    assert_eq!(value["records"], 4);
    assert_eq!(value["head"]["seq"], 4);
    assert!(stderr(&json).is_empty(), "stderr must stay clean");
}

/// Eine veränderte Zeile endet mit 4 und nennt die Stelle.
#[test]
fn audit_verify_broken_exit_4() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 4);

    // Ein Zeichen in Record 2, und der Hash der Zeile passt nicht mehr.
    let text = std::fs::read_to_string(&log).expect("the log reads");
    let tampered = text.replacen("\"n\":2", "\"n\":9", 1);
    assert_ne!(tampered, text, "the fixture must really change");
    std::fs::write(&log, &tampered).expect("the tampered log");

    let output = harness.run(["audit", "verify", "--file", &log.display().to_string()]);
    assert_eq!(code(&output), 4, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("audit chain: BROKEN at seq 2"),
        "{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains("hash_mismatch"),
        "{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).starts_with("error[AUDIT_001]: "),
        "{}",
        stderr(&output)
    );

    let json = harness.run([
        "--json",
        "audit",
        "verify",
        "--file",
        &log.display().to_string(),
    ]);
    assert_eq!(code(&json), 4, "{}", stderr(&json));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&json).trim()).expect("one JSON value");
    assert_eq!(value["chain"], "broken");
    assert_eq!(value["first_bad_seq"], 2);
    assert_eq!(value["diagnostic"]["code"], "AUDIT_001");
    assert!(stderr(&json).is_empty(), "stderr must stay clean");
}

/// Der CSV-Export hat eine Kopfzeile mit den zwölf Spalten aus HUM-050 und
/// eine Zeile je Record (HUM-156; bis dahin acht).
#[test]
fn audit_export_csv_columns() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 4);
    let out = harness.path("export.csv");

    let output = harness.run([
        "audit",
        "export",
        "--format",
        "csv",
        "--file",
        &log.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("exported 4 records to"),
        "{}",
        stdout(&output)
    );

    let csv = std::fs::read_to_string(&out).expect("the export is there");
    // RFC 4180: jede Zeile endet mit CRLF, auch die letzte.
    assert!(csv.ends_with("\r\n"), "{csv:?}");
    assert_eq!(csv.matches("\r\n").count(), 5, "{csv:?}");
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash"),
        "{csv}"
    );
    assert_eq!(lines.clone().count(), 4, "{csv}");
    let first = lines.next().unwrap_or_default();
    // `data` steht nicht im CSV; die Spalten, die der Record nicht trägt,
    // bleiben leer, und der Hash schließt die Zeile ab.
    let hash = first_record_hash(&log);
    assert_eq!(
        first,
        format!("1,2026-09-02T10:01:00.000000Z,-,flow.decided,,,,,,,,{hash}"),
        "{csv}"
    );

    // Ein vorhandener Export wird nie überschrieben.
    let again = harness.run([
        "audit",
        "export",
        "--format",
        "csv",
        "--file",
        &log.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&again), 1, "{}", stderr(&again));
    assert!(
        stderr(&again).starts_with("error[AUDIT_008]: "),
        "{}",
        stderr(&again)
    );
    // Der Vorschlag überschreibt keinen älteren Beleg und nimmt einen Namen
    // mit `-` am Anfang als Namen.
    assert!(
        stderr(&again).contains("\n  fix: mv -n -- "),
        "{}",
        stderr(&again)
    );

    // Der Bereich schneidet, und `jsonl` gibt die Zeilen wörtlich zurück.
    let jsonl = harness.path("export.jsonl");
    let ranged = harness.run([
        "audit",
        "export",
        "--format",
        "jsonl",
        "--file",
        &log.display().to_string(),
        "--out",
        &jsonl.display().to_string(),
        "--since",
        "2026-09-02T10:02:00Z",
        "--until",
        "2026-09-02T10:04:00Z",
    ]);
    assert_eq!(code(&ranged), 0, "{}", stderr(&ranged));
    let body = std::fs::read_to_string(&jsonl).expect("the jsonl export");
    assert_eq!(body.lines().count(), 2, "{body}");
    for line in body.lines() {
        assert!(
            text_of(&log).contains(line),
            "a jsonl row is the line of the log, byte for byte: {line}"
        );
    }
}

/// Der Hash des ersten Records einer Kette.
fn first_record_hash(path: &Path) -> String {
    let text = text_of(path);
    let line = text.lines().next().expect("the log has a record");
    humanitl_audit::AuditRecord::from_line(line.as_bytes())
        .expect("the first line is a record")
        .hash
}

/// Der Text einer Datei, für den Vergleich Zeile gegen Zeile.
fn text_of(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Ein `systemctl`, das nur protokolliert und mit 0 endet.
fn logging_systemctl(harness: &Harness, name: &str) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;

    let bin = harness.path(&format!("fakebin-{name}"));
    std::fs::create_dir_all(&bin).expect("the fake bin directory");
    let log = harness.path(&format!("{name}.log"));
    let script = format!(
        "#!/bin/sh\necho \"$*\" >>'{log}'\nexit 0\n",
        log = log.display()
    );
    let path = bin.join(name);
    std::fs::write(&path, script).expect("the fake program");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("0755");
    (bin, log)
}

/// `daemon install` schreibt die Unit und sagt systemd davon.
///
/// **Geschrieben wird `humanitld.service` und nicht `humanitld.socket`.** Ohne
/// Paket (Archiv, `AppImage`) bindet der Daemon seinen Socket selbst; die
/// Socket-Unit bringt nur das Paket mit, und dann schreibt `daemon install`
/// gar nichts, sondern aktiviert beide (HUM-053, `unit::SystemUnits`). Dieser
/// Lauf hat kein Paket, der Test hält den ersten Weg fest.
#[test]
fn daemon_install_writes_units_and_calls_systemctl() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let (fake, log) = logging_systemctl(&harness, "systemctl");

    let output = install_run_with_path(&harness, &bin, &fake, &["daemon", "install"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    let calls = fake_systemctl_log(&log);
    assert!(calls.contains("--user daemon-reload"), "{calls}");
    assert!(
        calls.contains("--user enable --now humanitld.service"),
        "{calls}"
    );

    let unit = unit_path(&harness);
    assert!(unit.is_file(), "the unit is written");
    let dir = unit.parent().expect("the unit directory");
    let written: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory is readable")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        !written.iter().any(|name| name == "humanitld.socket"),
        "only the package brings the socket unit: {written:?}"
    );

    // Der Daemon antwortet in dieser Umgebung nicht; das steht als Zeile da
    // und ist kein Fehlschlag.
    assert!(
        stdout(&output).contains("no answer within 5000 ms"),
        "{}",
        stdout(&output)
    );
}

/// Aus einem `AppImage` heraus werden Daemon und Shim herauskopiert, und
/// `ExecStart` zeigt auf die Kopie.
#[test]
fn daemon_install_appimage_copies_binaries() {
    use std::os::unix::fs::PermissionsExt as _;

    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let shim = bin.join("humanitl-shim");
    std::fs::write(&shim, b"#!/bin/sh\nexit 0\n").expect("the shim");
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("0755");

    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(["daemon", "install"])
        .current_dir(harness.path("work"))
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", "")
        .env("APPIMAGE", "/tmp/Humanitl-0.0.0-x86_64.AppImage")
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"))
        .env("XDG_DATA_HOME", harness.path("data"))
        .env("XDG_RUNTIME_DIR", harness.path("run"));
    let output = output_when_not_busy(command);
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    let link = harness.path("home").join(".local/lib/humanitl/current");
    let lib = std::fs::read_link(&link).expect("current is a symlink");
    assert_eq!(
        lib.parent(),
        Some(harness.path("home").join(".local/lib/humanitl").as_path()),
        "current points into ~/.local/lib/humanitl"
    );
    assert!(
        lib.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(concat!(env!("CARGO_PKG_VERSION"), "."))),
        "the copy carries the version: {}",
        lib.display()
    );
    for name in ["humanitld", "humanitl-shim"] {
        assert!(
            lib.join(name).is_file(),
            "{name} is not in {}",
            lib.display()
        );
    }

    // `ExecStart` nennt den Verweis und nie den Einhängepunkt des AppImages.
    let unit = std::fs::read_to_string(unit_path(&harness)).expect("the unit is written");
    assert!(
        unit.contains(&format!("ExecStart={}/humanitld\n", link.display())),
        "{unit}"
    );
    assert!(!unit.contains("/tmp/.mount_"), "{unit}");
}

/// Ohne Daemon endet `daemon status` mit 2 und einem Befund.
#[test]
fn daemon_status_exit_2_when_down() {
    let harness = Harness::new();

    let output = harness.run(["daemon", "status"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 2, "{text}");
    assert!(text.starts_with("blocking[DAEMON_001]: "), "{text}");
    assert!(stdout(&output).is_empty(), "stdout must stay clean");

    let json = harness.run(["--json", "daemon", "status"]);
    assert_eq!(code(&json), 2, "{}", stderr(&json));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&json).trim()).expect("one JSON value");
    assert_eq!(value["code"], "DAEMON_001");
    assert!(stderr(&json).is_empty(), "stderr must stay clean");
}

/// Ohne Nutzersitzung liest `daemon logs` kein Journal und sagt, warum.
#[test]
fn daemon_logs_without_a_user_session_is_daemon_010() {
    let harness = Harness::new();
    let mut command = harness.command();
    command
        .args(["daemon", "logs", "-n", "5"])
        .env_remove("XDG_RUNTIME_DIR");
    let output = command.output().expect("the binary runs");
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("blocking[DAEMON_010]: "), "{text}");
    assert!(text.contains("loginctl enable-linger"), "{text}");
}

/// `daemon logs` reicht seine Argumente an `journalctl` durch.
#[test]
fn daemon_logs_hands_its_arguments_to_journalctl() {
    let harness = Harness::new();
    let (fake, log) = logging_systemctl(&harness, "journalctl");

    let mut command = harness.command();
    command
        .args(["daemon", "logs", "-n", "5"])
        .env("PATH", &fake);
    let output = command.output().expect("the binary runs");
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    let calls = fake_systemctl_log(&log);
    assert!(
        calls.contains("--user -u humanitld.service -n 5"),
        "{calls}"
    );
    assert!(
        !calls.contains("-f"),
        "without --follow nothing follows: {calls}"
    );
}

/// Mit `NO_COLOR` und in einer Pipe geht keine einzige ANSI-Sequenz hinaus.
#[test]
fn no_color_and_a_pipe_carry_no_ansi() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 2);

    for args in [
        vec!["config", "get"],
        vec!["config", "get", "--origin"],
        vec!["config", "set", "hold.ask_mode", "banana"],
        vec!["audit", "verify", "--file", &log.display().to_string()],
        vec!["daemon", "status"],
    ] {
        let mut command = harness.command();
        command.args(&args).env("NO_COLOR", "1");
        let output = command.output().expect("the binary runs");
        for (stream, text) in [("stdout", stdout(&output)), ("stderr", stderr(&output))] {
            assert!(
                !text.contains('\u{1b}'),
                "{args:?} wrote an escape to {stream}: {text:?}"
            );
        }
    }
}

/// Die Socket-Unit hört dort, wo der Client den Socket sucht.
///
/// Das Paket legt sie nach `/usr/lib/systemd/user/` (HUM-053), und der Daemon
/// übernimmt ihren Socket nur, wenn er an genau diesem Pfad liegt
/// (`DAEMON_013`). Ein `ListenStream`, der woandershin zeigt als
/// `Paths::daemon_socket`, wäre der Fehler, den ein Paket am spätesten zeigt.
#[test]
fn the_socket_unit_listens_where_the_client_looks() {
    let unit = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packaging/systemd/humanitld.socket"),
    )
    .expect("the socket unit is in the repository");

    assert!(
        unit.contains("ListenStream=%t/humanitl/daemon.sock\n"),
        "{unit}"
    );
    assert!(unit.contains("SocketMode=0600"), "{unit}");

    let harness = Harness::new();
    let socket = harness.paths().daemon_socket();
    let tail = socket
        .strip_prefix(harness.path("run"))
        .expect("the socket lives under XDG_RUNTIME_DIR");
    assert_eq!(
        tail,
        Path::new("humanitl/daemon.sock"),
        "%t plus this tail is where the client looks"
    );
}

/// `docs/cli.md` nennt jedes Unterkommando aus CONVENTIONS.md 3.8 mit einem
/// Beispiel.
#[test]
fn docs_cli_names_every_subcommand_with_an_example() {
    let docs =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/cli.md"))
            .expect("docs/cli.md is in the repository");

    for example in [
        "humanitl config get",
        "humanitl config set",
        "humanitl config schema",
        "humanitl config edit",
        "humanitl audit verify",
        "humanitl audit export",
        "humanitl daemon install",
        "humanitl daemon status",
        "humanitl daemon logs",
    ] {
        assert!(
            docs.contains(example),
            "docs/cli.md has no example for {example}"
        );
    }
}

// --- HUM-070, Nachbesserung nach dem Review ---------------------------------

/// Ein Programm auf `PATH`, das seine Argumente protokolliert und dann tut,
/// was `body` sagt (Shell, eine Zeile oder mehrere).
fn fake_program(harness: &Harness, name: &str, body: &str) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;

    let bin = harness.path(&format!("fakebin-{name}"));
    std::fs::create_dir_all(&bin).expect("the fake bin directory");
    let log = harness.path(&format!("{name}.log"));
    let script = format!(
        "#!/bin/sh\necho \"$*\" >>'{log}'\n{body}\n",
        log = log.display()
    );
    let path = bin.join(name);
    std::fs::write(&path, script).expect("the fake program");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("0755");
    (bin, log)
}

/// Ein Programm, das `answer` ausgibt und mit 0 endet.
fn answering_program(harness: &Harness, name: &str, answer: &str) -> (PathBuf, PathBuf) {
    fake_program(harness, name, &format!("echo '{answer}'\nexit 0"))
}

/// `config set` ruft das Binary in der Umgebung der Testumgebung.
fn config_set(harness: &Harness, args: &[&str]) -> Output {
    let mut all = vec!["config", "set"];
    all.extend_from_slice(args);
    harness.run(all)
}

/// Die `config.toml` der Testumgebung.
fn config_file(harness: &Harness) -> PathBuf {
    harness.path("config/humanitl/config.toml")
}

/// Eine Aufzählung nimmt jeden ihrer Werte an.
///
/// Im Schema ist eine Aufzählung ein `oneOf` aus Konstanten und hat keinen Typ
/// `string`; bis zum Review lehnte `config set` deshalb jeden gültigen Wert
/// ab, und der Test für „banana" bestand, weil schon das Lesen des Typs
/// scheiterte.
#[test]
fn config_set_an_enum_value_is_written() {
    let harness = Harness::new();
    for (key, value) in [
        ("hold.ask_mode", "terminal"),
        ("ui.theme", "dark"),
        ("sandbox.work_mode", "ro"),
    ] {
        let output = config_set(&harness, &[key, value]);
        assert_eq!(code(&output), 0, "{key} {value}: {}", stderr(&output));
        let back = harness.run(["config", "get", key]);
        assert_eq!(stdout(&back).trim(), value, "{}", stderr(&back));
    }
    let text = std::fs::read_to_string(config_file(&harness)).expect("the file");
    assert!(text.contains("ask_mode = \"terminal\""), "{text}");
}

/// Geprüft wird gegen die wirkliche Datei, nicht gegen die Vorgaben.
///
/// `limits.hold_max_bytes` muss mindestens `limits.hold_body_cap_bytes` sein.
/// Jeder der beiden Werte ist für sich richtig; erst zusammen mit dem, was
/// schon in der Datei steht, ist einer falsch, und der landet nie darin.
#[test]
fn config_set_checks_against_the_real_file() {
    let harness = Harness::new();
    let first = config_set(&harness, &["limits.hold_max_bytes", "64MiB"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));

    let second = config_set(&harness, &["limits.hold_body_cap_bytes", "128MiB"]);
    let text = stderr(&second);
    assert_eq!(code(&second), 1, "{text}");
    assert!(text.starts_with("error[CONFIG_003]: "), "{text}");
    assert!(text.contains("limits.hold_max_bytes"), "{text}");
    // Der Befund nennt die Datei des Menschen, nicht die Nebendatei.
    assert!(!text.contains(".tmp-"), "{text}");
    let file = std::fs::read_to_string(config_file(&harness)).expect("the file");
    assert!(!file.contains("hold_body_cap_bytes"), "{file}");
    let get = harness.run(["config", "get", "limits.hold_max_bytes"]);
    assert_eq!(code(&get), 0, "the file still loads: {}", stderr(&get));

    // Und andersherum: Mit Platz genug ist derselbe Wert richtig.
    let wide = config_set(&harness, &["limits.hold_max_bytes", "2GiB"]);
    assert_eq!(code(&wide), 0, "{}", stderr(&wide));
    let fits = config_set(&harness, &["limits.hold_body_cap_bytes", "128MiB"]);
    assert_eq!(code(&fits), 0, "{}", stderr(&fits));
}

/// Eine Tabelle wird als Tabelle geschrieben, `null` entfernt den Schlüssel.
#[test]
fn config_set_writes_a_table_and_null_removes_the_key() {
    let harness = Harness::new();
    let table = config_set(&harness, &["sandbox.env", r#"{"FOO":"bar"}"#]);
    assert_eq!(code(&table), 0, "{}", stderr(&table));
    let text = std::fs::read_to_string(config_file(&harness)).expect("the file");
    let parsed: toml::Table = text.parse().expect("TOML");
    assert_eq!(
        parsed["sandbox"]["env"]["FOO"].as_str(),
        Some("bar"),
        "{text}"
    );

    let set = config_set(&harness, &["llm.endpoint", "http://192.168.1.20:11434"]);
    assert_eq!(code(&set), 0, "{}", stderr(&set));
    let removed = config_set(&harness, &["llm.endpoint", "null"]);
    assert_eq!(code(&removed), 0, "{}", stderr(&removed));
    let text = std::fs::read_to_string(config_file(&harness)).expect("the file");
    assert!(!text.contains("endpoint"), "{text}");
    assert!(text.contains("FOO"), "the rest stays: {text}");
    let back = harness.run(["config", "get", "llm.endpoint"]);
    assert_eq!(stdout(&back).trim(), "-", "{}", stderr(&back));
}

/// Eine verlinkte `config.toml` bleibt verlinkt, ihre Byte-Reihenfolge und
/// ihre Zeilenenden bleiben.
#[test]
fn config_set_keeps_a_linked_file_linked_with_its_bom_and_crlf() {
    let harness = Harness::new();
    let dotfiles = harness.path("dotfiles");
    std::fs::create_dir_all(&dotfiles).expect("the dotfile directory");
    let real = dotfiles.join("humanitl.toml");
    std::fs::write(&real, "\u{feff}[hold]\r\ntimeout_secs = 42\r\n").expect("the real file");
    let link = config_file(&harness);
    std::fs::create_dir_all(link.parent().expect("a directory")).expect("the config directory");
    std::os::unix::fs::symlink(&real, &link).expect("the link");

    let output = config_set(&harness, &["hold.timeout_secs", "5m"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    assert!(
        std::fs::symlink_metadata(&link)
            .expect("the link is there")
            .file_type()
            .is_symlink(),
        "the link stays a link"
    );
    let text = std::fs::read_to_string(&real).expect("the real file");
    assert!(text.starts_with('\u{feff}'), "{text:?}");
    assert!(text.contains("timeout_secs = 300\r\n"), "{text:?}");
    assert!(!text.replace("\r\n", "").contains('\n'), "{text:?}");
}

/// `--project` schreibt unter `[config]` in das Profil des Projekts, und ein
/// Schlüssel hinter der Vertrauensgrenze kommt dort nie an.
#[test]
fn config_set_project_writes_the_profile_and_respects_the_trust_boundary() {
    let harness = Harness::new();
    let profile = harness.path("work/.humanitl/profile.toml");

    let output = config_set(&harness, &["--project", "hold.timeout_secs", "90s"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "hold.timeout_secs = 90 (project)");
    let text = std::fs::read_to_string(&profile).expect("the project profile");
    let parsed: toml::Table = text.parse().expect("TOML");
    assert_eq!(
        parsed["config"]["hold"]["timeout_secs"].as_integer(),
        Some(90),
        "{text}"
    );
    assert!(
        !config_file(&harness).exists(),
        "the global file stays untouched"
    );

    std::fs::remove_file(&profile).expect("the profile goes");
    let denied = config_set(
        &harness,
        &["--project", "llm.endpoint", "http://evil.example"],
    );
    let text = stderr(&denied);
    assert_eq!(code(&denied), 1, "{text}");
    assert!(text.starts_with("error[CONFIG_003]: "), "{text}");
    assert!(text.contains("trust boundary"), "{text}");
    assert!(
        !profile.exists(),
        "no file for a key the project may not set"
    );

    // Auch eine gesperrte Aufzählung meldet die Grenze und keinen Lesefehler.
    let denied_enum = config_set(&harness, &["--project", "sandbox.work_mode", "nonsense"]);
    assert!(
        stderr(&denied_enum).contains("trust boundary"),
        "{}",
        stderr(&denied_enum)
    );
}

/// Ein Wert, der mit `-` beginnt, ist ein Wert; sein Vorschlag besteht.
#[test]
fn config_set_a_negative_value_is_refused_with_a_fix_that_works() {
    let harness = Harness::new();
    let output = config_set(&harness, &["hold.timeout_secs", "-5m"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CONFIG_003]: "), "{text}");
    assert!(
        text.contains("\n  fix: humanitl config set hold.timeout_secs 300\n"),
        "{text}"
    );
    let fixed = config_set(&harness, &["hold.timeout_secs", "300"]);
    assert_eq!(
        code(&fixed),
        0,
        "the suggested command works: {}",
        stderr(&fixed)
    );
}

/// `config edit` meldet einen Fehler genau einmal, unter `--json` als ein
/// Objekt, und `stderr` bleibt leer.
#[test]
fn config_edit_reports_a_broken_file_once() {
    let harness = Harness::new();
    let (editor, _log) = fake_program(
        &harness,
        "breaking-editor",
        "printf 'this is = = not toml\\n' >\"$1\"\nexit 0",
    );
    let output = harness
        .command()
        .args(["--json", "config", "edit"])
        .env("EDITOR", editor.join("breaking-editor"))
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    let body = stdout(&output);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert_eq!(body.lines().count(), 1, "one object per call: {body}");
    let value: serde_json::Value = serde_json::from_str(body.trim()).expect("one JSON value");
    assert_eq!(value["code"], "CONFIG_001");
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));

    let human = harness
        .command()
        .args(["config", "edit"])
        .env("EDITOR", editor.join("breaking-editor"))
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    assert_eq!(
        stderr(&human).matches("[CONFIG_001]").count(),
        1,
        "{}",
        stderr(&human)
    );
}

/// Ein Export, der mittendrin bricht, lässt nichts liegen und blockiert den
/// nächsten Versuch nicht.
#[test]
fn audit_export_that_breaks_leaves_no_file_behind() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 4);
    let good = std::fs::read_to_string(&log).expect("the log");
    let mut lines: Vec<&str> = good.lines().collect();
    lines[2] = "not a record";
    std::fs::write(&log, format!("{}\n", lines.join("\n"))).expect("the broken log");
    let out = harness.path("export/out.jsonl");

    let args = [
        "audit",
        "export",
        "--format",
        "jsonl",
        "--file",
        &log.display().to_string(),
        "--out",
        &out.display().to_string(),
    ];
    let broken = harness.run(args);
    assert_eq!(code(&broken), 1, "{}", stderr(&broken));
    assert!(
        stderr(&broken).starts_with("error[AUDIT_001]: "),
        "{}",
        stderr(&broken)
    );
    assert!(!out.exists(), "no half export");
    let leftovers: Vec<String> = std::fs::read_dir(out.parent().expect("a directory"))
        .expect("the directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    std::fs::write(&log, good).expect("the log is whole again");
    let retry = harness.run(args);
    assert_eq!(code(&retry), 0, "{}", stderr(&retry));
    assert_eq!(
        std::fs::read_to_string(&out)
            .expect("the export")
            .lines()
            .count(),
        4
    );
}

/// Ohne Nutzersitzung schreibt `daemon install` nichts und nennt den einen
/// Fix, der hilft.
#[test]
fn daemon_install_without_a_user_session_writes_nothing() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let (fake, log) = logging_systemctl(&harness, "systemctl");

    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(["daemon", "install"])
        .current_dir(harness.path("work"))
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", &fake)
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"))
        .env("XDG_DATA_HOME", harness.path("data"));
    let output = output_when_not_busy(command);
    let text = stderr(&output);

    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[DAEMON_010]"), "{text}");
    assert!(text.contains("loginctl enable-linger"), "{text}");
    assert!(!unit_path(&harness).exists(), "nothing is written");
    assert!(
        fake_systemctl_log(&log).is_empty(),
        "systemctl is not called"
    );
}

/// Findet `systemctl` den Bus der Sitzung nicht, ist das dieselbe fehlende
/// Sitzung und kein `DAEMON_008` mit einem Vorschlag, der ebenso scheitert.
#[test]
fn daemon_install_without_a_session_bus_is_daemon_010() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let (fake, _log) = fake_program(
        &harness,
        "systemctl",
        "echo 'Failed to connect to bus: No medium found' >&2\nexit 1",
    );

    let output = install_run_with_path(&harness, &bin, &fake, &["daemon", "install"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[DAEMON_010]"), "{text}");
    assert!(!text.contains("[DAEMON_008]"), "{text}");
    assert!(!unit_path(&harness).exists(), "the unit is taken back");
}

/// Eine fremde Unit hält auch das Kopieren aus dem `AppImage` auf.
#[test]
fn daemon_install_appimage_copies_nothing_before_the_checks() {
    let harness = Harness::new();
    let bin = appimage_tree(&harness);
    let unit = unit_path(&harness);
    std::fs::create_dir_all(unit.parent().expect("a directory")).expect("the unit directory");
    std::fs::write(&unit, "[Service]\nExecStart=/mine\n").expect("a foreign unit");

    let output = appimage_install(&harness, &bin, &["daemon", "install"]);
    assert!(
        stderr(&output).contains("[DAEMON_005]"),
        "{}",
        stderr(&output)
    );
    assert!(
        !harness.path("home/.local/lib/humanitl").exists(),
        "nothing is copied before the refusal"
    );
}

/// Ein Verzeichnis der Fassung, das ein Verweis ist, bekommt keine Binaries.
#[test]
fn daemon_install_appimage_refuses_a_linked_lib_directory() {
    let harness = Harness::new();
    let bin = appimage_tree(&harness);
    let foreign = harness.path("foreign");
    std::fs::create_dir_all(&foreign).expect("the foreign directory");
    let base = harness.path("home/.local/lib/humanitl");
    std::fs::create_dir_all(base.parent().expect("a parent")).expect("~/.local/lib");
    std::os::unix::fs::symlink(&foreign, &base).expect("the lib directory is a link");

    let output = appimage_install(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("[DAEMON_011]"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_dir(&foreign)
            .expect("the foreign directory")
            .count(),
        0,
        "nothing lands where the link points"
    );
    assert!(!unit_path(&harness).exists(), "and no unit either");
}

/// `--print` unter `APPIMAGE` zeigt, was die Unit wirklich bekäme: den
/// Verweis `current`, nie den Einhängepunkt.
#[test]
fn daemon_install_appimage_print_names_the_copy() {
    let harness = Harness::new();
    let bin = appimage_tree(&harness);
    let output = appimage_install(&harness, &bin, &["--json", "daemon", "install", "--print"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&output).trim()).expect("one JSON value");
    let exec = value["exec_start"].as_str().expect("exec_start");
    assert!(
        exec.ends_with(".local/lib/humanitl/current/humanitld"),
        "{exec}"
    );
    assert!(
        !harness.path("home/.local/lib").exists(),
        "--print copies nothing"
    );
}

/// `daemon install --json` schreibt nichts auf `stderr`.
#[test]
fn daemon_install_json_keeps_stderr_empty() {
    let harness = Harness::new();
    let bin = installed_tree(&harness);
    let (fake, _log) = logging_systemctl(&harness, "systemctl");
    let output = install_run_with_path(&harness, &bin, &fake, &["--json", "daemon", "install"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&output).trim()).expect("one JSON value");
    assert_eq!(value["action"], "created");
    assert!(
        value["commands"][1]
            .as_str()
            .is_some_and(|command| command.ends_with("--user enable --now humanitld.service")),
        "{value}"
    );
}

/// `daemon logs` übersetzt den Exit-Code von `journalctl`, und ein fehlendes
/// `journalctl` ist ein fehlendes Paket.
#[test]
fn daemon_logs_maps_the_exit_code_and_names_a_missing_journalctl() {
    let harness = Harness::new();
    let (fake, _log) = fake_program(&harness, "journalctl", "exit 4");
    let mut command = harness.command();
    command.args(["daemon", "logs"]).env("PATH", &fake);
    let output = command.output().expect("the binary runs");
    assert_eq!(
        code(&output),
        1,
        "a 4 of journalctl is no security violation: {}",
        stderr(&output)
    );

    let mut command = harness.command();
    command
        .args(["daemon", "logs"])
        .env("PATH", harness.path("empty"));
    let output = command.output().expect("the binary runs");
    let text = stderr(&output);
    assert!(text.contains("[DAEMON_012]"), "{text}");
    assert!(text.contains("apt-get install systemd"), "{text}");
    assert!(!text.contains("loginctl"), "{text}");
}

/// Ein Baum wie in einem `AppImage`: Kommandozeile, Daemon und Shim.
fn appimage_tree(harness: &Harness) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let bin = installed_tree(harness);
    let shim = bin.join("humanitl-shim");
    std::fs::write(&shim, b"#!/bin/sh\nexit 0\n").expect("the shim");
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("0755");
    bin
}

/// `daemon install` mit gesetztem `APPIMAGE` und leerem `PATH`.
fn appimage_install(harness: &Harness, bin: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(args)
        .current_dir(harness.path("work"))
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", "")
        .env("APPIMAGE", "/tmp/Humanitl-0.0.0-x86_64.AppImage")
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"))
        .env("XDG_DATA_HOME", harness.path("data"))
        .env("XDG_RUNTIME_DIR", harness.path("run"));
    output_when_not_busy(command)
}

// --- HUM-070, zweite Nachbesserung --------------------------------------------

/// `config set sandbox.work_dir` wird mit dem Projekt geprüft, das es nennt.
///
/// Welches Projekt-Profil gilt, hängt an `sandbox.work_dir`. Die Probe muss
/// deshalb die Quellen mit der neuen Datei neu bestimmen; ein Satz, der vor der
/// Änderung feststand, prüfte das Profil des alten Projekts, und ein Profil
/// mit einem gesperrten Schlüssel käme in die Konfiguration, die der nächste
/// Start ablehnt.
#[test]
fn config_set_work_dir_is_probed_with_the_project_it_names() {
    let harness = Harness::new();
    let project = harness.path("home/evil-repo");
    std::fs::create_dir_all(project.join(".humanitl")).expect("the project");
    std::fs::write(
        project.join(".humanitl/profile.toml"),
        "[config.sandbox]\nwork_mode = \"ro\"\n",
    )
    .expect("a project profile with a key it may not set");

    let output = config_set(
        &harness,
        &["sandbox.work_dir", &project.display().to_string()],
    );
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[CONFIG_003]"), "{text}");
    assert!(
        !config_file(&harness).exists(),
        "nothing is written: {:?}",
        std::fs::read_to_string(config_file(&harness))
    );
}

/// Ein älterer Fehler verdeckt keinen neuen.
///
/// Die Ladung meldet nur ihren ersten Befund. Schriebe `config set`, sobald die
/// Konfiguration ohne die Änderung genauso scheitert, käme ein falscher Wert
/// in die Datei, sobald irgendetwas anderes schon falsch ist — auch nur eine
/// Umgebungsvariable dieses einen Aufrufs.
#[test]
fn config_set_refuses_when_an_older_error_would_mask_the_new_one() {
    let harness = Harness::new();
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    let before = "[hold]\ntimeout_secs = 0\n\n[limits]\nhold_max_bytes = 67108864\n";
    std::fs::write(&file, before).expect("a config with an older error");

    let output = config_set(&harness, &["limits.hold_body_cap_bytes", "128MiB"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(
        text.contains("did not load before this change either"),
        "{text}"
    );
    // Der neue Befund nennt den gesetzten Schlüssel; sein eigener Vorschlag
    // gilt.
    assert!(
        text.contains("\n  fix: humanitl config set limits.hold_max_bytes "),
        "{text}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("the file"),
        before,
        "the file is untouched"
    );

    // Dasselbe, wenn der ältere Fehler nur in der Umgebung dieses Aufrufs
    // steckt: Die Datei bekäme sonst einen Wert, der ohne die Variable nicht
    // lädt.
    std::fs::write(&file, "[limits]\nhold_max_bytes = 67108864\n").expect("a clean config");
    let output = harness
        .command()
        .args(["config", "set", "limits.hold_body_cap_bytes", "128MiB"])
        .env("HUMANITL_HOLD__TIMEOUT_SECS", "0")
        .output()
        .expect("the binary runs");
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert!(
        !std::fs::read_to_string(&file)
            .expect("the file")
            .contains("hold_body_cap_bytes"),
        "nothing is written"
    );
    let get = harness.run(["config", "get", "limits.hold_max_bytes"]);
    assert_eq!(code(&get), 0, "the file still loads: {}", stderr(&get));
}

/// Dieselbe Fassung ein zweites Mal: `current` zeigt immer auf eine
/// vollständige Kopie, und die alte geht erst, wenn die neue steht.
#[test]
fn daemon_install_appimage_same_version_again_never_touches_the_live_copy() {
    let harness = Harness::new();
    let bin = appimage_tree(&harness);
    let link = harness.path("home/.local/lib/humanitl/current");

    let first = appimage_install(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let old = std::fs::read_link(&link).expect("current after the first run");

    let second = appimage_install(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&second), 0, "{}", stderr(&second));
    let new = std::fs::read_link(&link).expect("current after the second run");

    assert_ne!(old, new, "the second copy is a directory of its own");
    assert!(new.join("humanitld").is_file(), "{}", new.display());
    // Seit HUM-077 geht die alte Kopie erst nach einem Neustart des Dienstes:
    // Ohne `systemctl` (leerer `PATH`) startet niemand neu, und ein Daemon,
    // der noch aus ihr läuft, verlöre sonst seine Datei. Dass sie nach dem
    // Neustart geht, misst
    // `refresh_replaces_an_older_copy_restarts_and_only_then_retires_it` in
    // `tests/daemon_lifecycle.rs`.
    assert!(
        old.join("humanitld").is_file(),
        "the copy current pointed at before stays complete without a restart"
    );
    let mut expected = vec![old, new];
    expected.sort();
    assert_eq!(lib_copies(&harness), expected, "the old and the new copy");
}

/// Scheitert `enable` nach der Kopie, geht die Kopie wieder, und `current`
/// zeigt dorthin, wohin es vorher zeigte.
#[test]
fn daemon_install_appimage_failed_enable_takes_the_copy_back() {
    let harness = Harness::new();
    let bin = appimage_tree(&harness);
    let link = harness.path("home/.local/lib/humanitl/current");

    // Eine erste Installation ohne systemctl: Die Unit liegt, `current` zeigt
    // auf die erste Kopie.
    let first = appimage_install(&harness, &bin, &["daemon", "install"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let before = std::fs::read_link(&link).expect("current after the first run");

    // Die zweite scheitert an `enable`.
    let unit_dir = unit_path(&harness)
        .parent()
        .expect("the unit directory")
        .to_path_buf();
    let (fake, _log) = fake_systemctl(&harness, &unit_dir);
    let mut command = Command::new(bin.join("humanitl"));
    command
        .args(["daemon", "install"])
        .current_dir(harness.path("work"))
        .env_clear()
        .env("HUMANITL_SYSTEM_UNIT_DIR", harness.path("system-units"))
        .env("PATH", &fake)
        .env("APPIMAGE", "/tmp/Humanitl-0.0.0-x86_64.AppImage")
        .env("HOME", harness.path("home"))
        .env("XDG_CONFIG_HOME", harness.path("config"))
        .env("XDG_DATA_HOME", harness.path("data"))
        .env("XDG_RUNTIME_DIR", harness.path("run"));
    let second = output_when_not_busy(command);
    assert_ne!(code(&second), 0, "{}", stdout(&second));

    assert_eq!(
        std::fs::read_link(&link).expect("current is still a link"),
        before,
        "current points where it pointed before"
    );
    assert_eq!(
        lib_copies(&harness),
        vec![before],
        "the copy of the failed run is gone"
    );
}

/// Die Kopien unter `~/.local/lib/humanitl`, ohne `current`.
fn lib_copies(harness: &Harness) -> Vec<PathBuf> {
    let base = harness.path("home/.local/lib/humanitl");
    let mut copies: Vec<PathBuf> = std::fs::read_dir(&base)
        .expect("the lib directory")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.file_name().is_some_and(|name| name != "current"))
        .collect();
    copies.sort();
    copies
}

/// Eine Nebendatei, deren Export nicht mehr läuft, räumt der nächste Export
/// weg.
#[test]
fn audit_export_sweeps_a_stale_temp_file() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 2);
    let out = harness.path("export/out.jsonl");
    std::fs::create_dir_all(out.parent().expect("a directory")).expect("the export directory");
    // Der Rest des Namens ist zufällig wie bei jedem Export; die Datei hält
    // niemand gesperrt, ihr Export ist also nicht mehr da.
    std::fs::set_permissions(
        out.parent().expect("a directory"),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .expect("a private export directory");
    let stale = harness.path("export/.out.jsonl.tmp-Vb81kQwz3sLe");
    std::fs::write(&stale, "left behind by a killed export\n").expect("a stale temp file");
    age(&stale);
    // Eine zweite Nebendatei, ebenso alt, deren Export aber noch schreibt:
    // Er hält die Sperre, und sein Prozess ist von hier aus nicht zu sehen
    // (anderer PID-Namensraum, `hidepid`). Sie bleibt.
    let busy = harness.path("export/.out.jsonl.tmp-Jd40pNcx7uRa");
    std::fs::write(&busy, "still being written\n").expect("a busy temp file");
    age(&busy);
    let held = std::fs::File::open(&busy).expect("the busy file opens");
    rustix::fs::flock(&held, rustix::fs::FlockOperation::LockExclusive).expect("the lock");
    // Eine junge Nebendatei ohne Sperre: Sie kann gerade angelegt worden
    // sein, und die Sperre folgt einen Augenblick später. Sie bleibt.
    let young = harness.path("export/.out.jsonl.tmp-young");
    std::fs::write(&young, "just created\n").expect("a young temp file");
    // Ein Verweis unter dem Namen einer Nebendatei: Er wird nicht verfolgt
    // und nicht gelöscht, auch wenn sein Ziel alt ist.
    let target = harness.path("export-target");
    std::fs::write(&target, "somebody else's file\n").expect("the target");
    age(&target);
    let link = harness.path("export/.out.jsonl.tmp-link");
    std::os::unix::fs::symlink(&target, &link).expect("the link");
    // Auch der Verweis selbst ist alt; sonst schützte ihn schon sein Alter,
    // und der Test sagte nichts darüber, ob ein Verweis verfolgt wird.
    let old = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
        .saturating_sub(600);
    let old = rustix::fs::Timespec {
        tv_sec: i64::try_from(old).expect("a time that fits"),
        tv_nsec: 0,
    };
    rustix::fs::utimensat(
        rustix::fs::CWD,
        &link,
        &rustix::fs::Timestamps {
            last_access: old,
            last_modification: old,
        },
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .expect("the link is aged");

    let output = harness.run([
        "audit",
        "export",
        "--format",
        "jsonl",
        "--file",
        &log.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(!stale.exists(), "the stale temp file is gone");
    assert!(busy.exists(), "a temp file under a held lock stays");
    assert!(young.exists(), "a young temp file stays");
    assert!(
        std::fs::symlink_metadata(&link).is_ok() && target.exists(),
        "a link is neither followed nor removed"
    );
    drop(held);
    assert_eq!(
        std::fs::read_to_string(&out)
            .expect("the export")
            .lines()
            .count(),
        2
    );
}

/// Setzt die Änderungszeit einer Datei zehn Minuten zurück.
fn age(path: &Path) {
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("the file opens");
    file.set_modified(std::time::SystemTime::now() - Duration::from_secs(600))
        .expect("the time is set");
}

// --- HUM-070, dritte Nachbesserung --------------------------------------------

/// Ein Verweis ins Leere am Ziel ist ein Pfad, der schon da ist: derselbe
/// Befund samt Vorschlag wie bei einer Datei.
#[test]
fn audit_export_refuses_a_dangling_link_at_the_target() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 2);
    let out = harness.path("dangling.jsonl");
    std::os::unix::fs::symlink(harness.path("nowhere"), &out).expect("a dangling link");

    let output = harness.run([
        "audit",
        "export",
        "--format",
        "jsonl",
        "--file",
        &log.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[AUDIT_008]: "), "{text}");
    assert!(text.contains("is already there"), "{text}");
    assert!(
        !harness.path("nowhere").exists(),
        "nothing is written through the link"
    );
}

/// Zwei falsche Werte in der Datei: Jeder lässt sich für sich reparieren, auch
/// wenn der andere noch falsch ist.
#[test]
fn config_set_repairs_a_file_with_two_bad_keys_one_at_a_time() {
    let harness = Harness::new();
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    std::fs::write(
        &file,
        "[hold]\ntimeout_secs = 0\n\n[resolver]\ncache_ttl_secs = 999999999\n",
    )
    .expect("a config with two bad keys");

    // Erst der Schlüssel, den die Prüfung zuerst meldet: Danach lädt die
    // Konfiguration immer noch nicht, und der verbleibende Befund über
    // `resolver` stand schon vorher da. Ohne das Zählen aller Befunde sähe
    // das wie ein neuer Fehler aus.
    let first = config_set(&harness, &["hold.timeout_secs", "5m"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    assert!(
        stderr(&first).contains("still does not load"),
        "the note names the older finding: {}",
        stderr(&first)
    );
    let second = config_set(&harness, &["resolver.cache_ttl_secs", "300"]);
    assert_eq!(code(&second), 0, "{}", stderr(&second));
    let get = harness.run(["config", "get", "hold.timeout_secs"]);
    assert_eq!(stdout(&get).trim(), "300", "{}", stderr(&get));

    // Ein neuer falscher Wert bleibt draußen, auch wenn schon etwas falsch ist.
    std::fs::write(&file, "[hold]\ntimeout_secs = 0\n").expect("one bad key again");
    let bad = config_set(&harness, &["resolver.cache_ttl_secs", "999999999"]);
    assert_eq!(code(&bad), 1, "{}", stderr(&bad));
    assert!(
        !std::fs::read_to_string(&file)
            .expect("the file")
            .contains("cache_ttl_secs"),
        "nothing is written"
    );
}

/// Eine falsche Umgebungsvariable sperrt `config set` nicht für jeden
/// Schlüssel.
#[test]
fn config_set_is_not_locked_out_by_a_bad_environment_variable() {
    let harness = Harness::new();
    // Dazu ein zweiter falscher Wert in der Datei, der nach dem der Umgebung
    // geprüft wird: Das Zählen muss die Variable herausnehmen, um ihn zu
    // sehen, und darf erst dann schreiben.
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    std::fs::write(&file, "[resolver]\ncache_ttl_secs = 999999999\n").expect("a later bad key");
    let output = harness
        .command()
        .args(["config", "set", "ui.theme", "dark"])
        .env("HUMANITL_HOLD__TIMEOUT_SECS", "0")
        .output()
        .expect("the binary runs");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = std::fs::read_to_string(config_file(&harness)).expect("the file");
    assert!(text.contains("theme = \"dark\""), "{text}");
}

// --- HUM-070, vierte Nachbesserung --------------------------------------------

/// Ein Schlüssel, den das Schema nicht kennt, hat keinen Schlüssel: Sein Text
/// nennt nur einen Vorschlag. Solange er in der Datei steht, wird nichts
/// geschrieben — weder ein harmloser Wert noch der vorgeschlagene.
#[test]
fn config_set_refuses_while_the_file_has_an_unknown_key() {
    let harness = Harness::new();
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    let before = "[hold]\ntimeout_secz = 5\n";
    std::fs::write(&file, before).expect("a config with an unknown key");

    for args in [["ui.theme", "dark"], ["hold.timeout_secs", "300"]] {
        let output = config_set(&harness, &args);
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{args:?}: {text}");
        assert!(text.contains("[CONFIG_002]"), "{args:?}: {text}");
        assert!(
            text.contains("\n  fix: humanitl config edit\n"),
            "{args:?}: {text}"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("the file"),
            before,
            "{args:?}: the file is untouched"
        );
    }
}

/// Ein Paar, dessen Grenze der neue Wert verschiebt, ist ein neuer Befund,
/// auch wenn derselbe Schlüssel schon vorher falsch war.
#[test]
fn config_set_refuses_a_value_that_moves_the_bound_of_an_old_finding() {
    let harness = Harness::new();
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    let before = "[limits]\nhold_max_bytes = 500\n";
    std::fs::write(&file, before).expect("a config with a broken pair");

    let output = config_set(&harness, &["limits.hold_body_cap_bytes", "1073741824"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[CONFIG_003]"), "{text}");
    assert_eq!(
        std::fs::read_to_string(&file).expect("the file"),
        before,
        "the file is untouched"
    );

    // Derselbe Code am selben Schlüssel, aber ein anderer Text: Das Paar war
    // schon vorher falsch (16 MiB unter der Vorgabe von 32 MiB), und der neue
    // Wert verschiebt nur seine Grenze. Das ist ein neuer Befund; ohne den
    // Vergleich des Textes sähe er aus wie der alte.
    let before = "[limits]\nhold_max_bytes = 16777216\n";
    std::fs::write(&file, before).expect("an older broken pair");
    let output = config_set(&harness, &["limits.hold_body_cap_bytes", "24MiB"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[CONFIG_003]"), "{text}");
    assert_eq!(
        std::fs::read_to_string(&file).expect("the file"),
        before,
        "the file is untouched"
    );
}

// --- HUM-070, fünfte Nachbesserung --------------------------------------------

/// Ein Wert, der selbst falsch ist, wird nicht geschrieben, auch wenn derselbe
/// falsche Wert schon dastand: Der Befund nennt den gesetzten Schlüssel.
#[test]
fn config_set_refuses_a_wrong_value_that_was_already_there() {
    let harness = Harness::new();
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");

    // Das Schema lässt jede nicht negative Zahl zu, die Prüfung höchstens
    // 3650 Tage. (Bis HUM-051 war 0 das Beispiel; seitdem heißt 0 „nie".)
    let before = "[recorder]\nretention_days = 4000\n";
    std::fs::write(&file, before).expect("a config with a wrong value");
    let output = config_set(&harness, &["recorder.retention_days", "4000"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[CONFIG_003]"), "{text}");
    assert!(!text.contains("is written"), "{text}");

    // Dasselbe für ein Paar: Die Obergrenze liegt schon über dem Maximum,
    // und sie noch einmal zu setzen, ist kein Wert, der passt.
    let before = "[limits]\nhold_max_bytes = 10485760\nhold_body_cap_bytes = 20971520\n";
    std::fs::write(&file, before).expect("a config with a broken pair");
    let output = config_set(&harness, &["limits.hold_body_cap_bytes", "20MiB"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[CONFIG_003]"), "{text}");
    assert!(!text.contains("is written"), "{text}");
    assert_eq!(
        std::fs::read_to_string(&file).expect("the file"),
        before,
        "the file is untouched"
    );
}

/// Ein Befund ohne Schlüssel aus dem Projekt-Profil, das `sandbox.work_dir`
/// wählt, schlägt nie einen Wert aus dem Projekt für die globale Datei vor.
#[test]
fn config_set_work_dir_never_suggests_a_value_from_the_project() {
    let harness = Harness::new();
    let project = harness.path("home/near-miss");
    std::fs::create_dir_all(project.join(".humanitl")).expect("the project");
    std::fs::write(
        project.join(".humanitl/profile.toml"),
        "[config.ui]\nthemee = \"dark\"\n",
    )
    .expect("a project profile with an unknown key");

    let output = config_set(
        &harness,
        &["sandbox.work_dir", &project.display().to_string()],
    );
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("[CONFIG_002]"), "{text}");
    assert!(text.contains("\n  fix: humanitl config edit\n"), "{text}");
    assert!(!text.contains("config set"), "{text}");
    assert!(!config_file(&harness).exists(), "nothing is written");
}

/// In einem Verzeichnis, in das auch andere schreiben dürfen, wird nicht
/// gefegt: Den Namen, der am Ende gelöscht wird, könnte ein anderer nach der
/// letzten Prüfung austauschen.
#[test]
fn audit_export_leaves_temp_files_in_a_shared_directory() {
    use std::os::unix::fs::PermissionsExt as _;

    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 2);
    for mode in [0o775, 0o777] {
        let dir = harness.path(&format!("shared-{mode:o}"));
        std::fs::create_dir_all(&dir).expect("the export directory");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode))
            .expect("a directory others may write");
        let out = dir.join("out.jsonl");
        let stale = dir.join(".out.jsonl.tmp-Q7mZr2xLp0aB");
        std::fs::write(&stale, "left behind by a killed export\n").expect("a stale temp file");
        age(&stale);

        let output = harness.run([
            "audit",
            "export",
            "--format",
            "jsonl",
            "--file",
            &log.display().to_string(),
            "--out",
            &out.display().to_string(),
        ]);
        assert_eq!(code(&output), 0, "{mode:o}: {}", stderr(&output));
        assert!(stale.exists(), "{mode:o}: the temp file stays");
        assert!(out.exists(), "{mode:o}: the export is written");
    }
}

// --- HUM-070, sechste Nachbesserung -------------------------------------------

/// Ein Wert, der wie ein Schlüssel aussieht, macht diesen Schlüssel nicht zum
/// Teil des Befunds: `hold.timeout_secs` lässt sich setzen, obwohl der falsche
/// Wert von `ui.theme` genau so heißt.
#[test]
fn config_set_is_not_refused_by_a_value_that_looks_like_a_key() {
    let harness = Harness::new();
    let file = config_file(&harness);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the config directory");
    std::fs::write(&file, "[ui]\ntheme = \"hold.timeout_secs\"\n")
        .expect("a config with a value that looks like a key");

    let output = config_set(&harness, &["hold.timeout_secs", "300"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 0, "{text}");
    assert!(text.contains("still does not load"), "{text}");
    let written = std::fs::read_to_string(&file).expect("the file");
    assert!(written.contains("timeout_secs = 300"), "{written}");
    assert!(
        written.contains("theme = \"hold.timeout_secs\""),
        "{written}"
    );
}

//
// `audit` gegen einen Daemon, der `Audit` beantwortet (HUM-156). Der Dienst
// ist derselbe `IpcServer` wie in `humanitld`, ohne Proxy, mit einem festen
// Schlüssel und den Ankern aus der Datenbank der Umgebung.

/// Schreibt eine Kette mit dem Schreiber des Daemons, Anker alle drei Records
/// in Datei und Tabelle, und gibt den letzten Record zurück.
fn anchored_chain(harness: &Harness, records: usize) -> humanitl_audit::AuditRecord {
    use humanitl_audit::kinds::ConfigChanged;
    use humanitl_audit::{
        Anchor, AnchorMirror, AuditKey, AuditWriter, KeyOrigin, RecordKind, WriterOptions,
    };
    use humanitl_recorder::{AnchorStore, AuditAnchor};

    let paths = harness.paths();
    std::fs::create_dir_all(paths.db_path().parent().expect("the data directory"))
        .expect("the data directory");
    let store = AnchorStore::open(&paths.db_path()).expect("the anchor table");
    let mirror: AnchorMirror = Box::new(move |anchor: &Anchor| {
        store.put(&AuditAnchor {
            seq: anchor.seq,
            hash: anchor.hash.clone(),
            ts: anchor.ts.clone(),
        })
    });
    let (writer, _) = AuditWriter::open(
        &paths.audit_path(),
        &AuditKey::from_bytes(AUDIT_KEY, KeyOrigin::File),
        WriterOptions {
            anchor_every: 3,
            ..WriterOptions::default()
        },
        &[],
        Some(mirror),
    )
    .expect("the writer opens");
    for index in 0..records {
        writer.handle().record(
            None,
            RecordKind::ConfigChanged(ConfigChanged {
                key: format!("hold.timeout_secs.{index}"),
                origin: "cli".to_owned(),
                secret: false,
                value: Some("300".to_owned()),
            }),
        );
    }
    let _ = writer.stop("test").expect("the writer stops");
    let text = text_of(&paths.audit_path());
    let last = text.lines().last().expect("a record");
    humanitl_audit::AuditRecord::from_line(last.as_bytes()).expect("the last line is a record")
}

/// `audit verify` fragt den Daemon, und der prüft mit Schlüssel und Ankern;
/// die Ausgabe sagt beides und nennt den Kopf, den der Daemon nennt.
#[test]
fn audit_verify_asks_the_daemon_for_key_and_anchors() {
    let harness = Harness::new();
    let last = anchored_chain(&harness, 7);
    let _daemon = common::AuditServer::start(&harness, AUDIT_KEY);

    let output = harness.run(["audit", "verify"]);
    let text = stdout(&output);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(text.contains("audit chain: OK"), "{text}");
    assert!(
        text.contains("hmac key:    checked by the daemon"),
        "{text}"
    );
    assert!(text.contains("checked by:  daemon"), "{text}");
    assert!(!text.contains("file mode"), "the daemon checked it: {text}");
    assert!(text.contains(&format!("(seq {})", last.body.seq)), "{text}");

    let json = harness.run(["--json", "audit", "verify"]);
    assert_eq!(code(&json), 0, "{}", stderr(&json));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&json).trim()).expect("one JSON value");
    assert_eq!(value["mode"], "full");
    assert_eq!(value["hmac"], "checked");
    assert_eq!(value["anchors"], "checked");
    assert_eq!(value["checked_by"], "daemon");
    assert_eq!(value["head"]["hash"], last.hash);
    assert_eq!(value["head"]["seq"], last.body.seq);
    let anchors = humanitl_recorder::read_anchors(&harness.paths().db_path()).expect("anchors");
    assert_eq!(value["anchor_count"], anchors.len());
    assert!(value["last_anchor_at"].is_string(), "{value}");
    assert_eq!(value["warnings"], serde_json::json!([]), "{value}");
}

/// Der Daemon rechnet die MACs mit seinem Schlüssel nach. Eine Kette, die ein
/// anderer Schlüssel versiegelt hat, bricht bei ihm am ersten Record; die
/// Prüfung der Datei ohne Schlüssel sähe sie heil.
#[test]
fn audit_verify_through_the_daemon_sees_a_foreign_key() {
    let harness = Harness::new();
    anchored_chain(&harness, 4);
    let _daemon = common::AuditServer::start(&harness, [1_u8; 32]);

    let output = harness.run(["audit", "verify"]);
    assert_eq!(code(&output), 4, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("audit chain: BROKEN at seq 1 (mac_mismatch)"),
        "{}",
        stdout(&output)
    );

    let log = harness.paths().audit_path();
    let file = harness.run(["audit", "verify", "--file", &log.display().to_string()]);
    assert_eq!(code(&file), 0, "{}", stderr(&file));
}

/// Eine nach dem Schreiben veränderte Zeile: über den Daemon „gebrochen ab
/// Sequenz n" mit Grund, Exit 4 und `AUDIT_001`.
#[test]
fn audit_verify_through_the_daemon_reports_a_changed_line() {
    let harness = Harness::new();
    anchored_chain(&harness, 7);
    let log = harness.paths().audit_path();
    let text = text_of(&log);
    let tampered = text.replacen("hold.timeout_secs.1\"", "hold.timeout_secs.9\"", 1);
    assert_ne!(tampered, text, "the fixture must really change");
    std::fs::write(&log, &tampered).expect("the tampered log");
    let _daemon = common::AuditServer::start(&harness, AUDIT_KEY);

    let output = harness.run(["audit", "verify"]);
    assert_eq!(code(&output), 4, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("audit chain: BROKEN at seq 2 (hash_mismatch)"),
        "{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains("checked by:  daemon"),
        "{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).starts_with("error[AUDIT_001]: "),
        "{}",
        stderr(&output)
    );

    let json = harness.run(["--json", "audit", "verify"]);
    assert_eq!(code(&json), 4, "{}", stderr(&json));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&json).trim()).expect("one JSON value");
    assert_eq!(value["chain"], "broken");
    assert_eq!(value["first_bad_seq"], 2);
    assert_eq!(value["reason"], "hash_mismatch");
    assert_eq!(value["diagnostic"]["code"], "AUDIT_001");
}

/// Ein Daemon, der antwortet und ablehnt, wird nicht durch die schwächere
/// Prüfung der Datei ersetzt: Sein Befund ist die Auskunft.
#[test]
fn audit_verify_does_not_replace_a_refusing_daemon_with_the_file() {
    let harness = Harness::new();
    audit_chain(&harness.paths().audit_path(), 3);
    // Der Fake hat kein Audit-Log und sagt das mit `IPC_006`.
    let _daemon = FakeServer::start(&harness);

    let output = harness.run(["audit", "verify"]);
    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(stderr(&output).contains("[IPC_006]"), "{}", stderr(&output));
    assert!(
        !stdout(&output).contains("audit chain"),
        "no file-mode result stands in for the daemon: {}",
        stdout(&output)
    );
}

/// Beim Export dasselbe: Ein Daemon, der ablehnt, wird nicht durch den Export
/// aus der Datei ersetzt, und es entsteht keine Datei.
#[test]
fn audit_export_does_not_replace_a_refusing_daemon_with_the_file() {
    let harness = Harness::new();
    audit_chain(&harness.paths().audit_path(), 3);
    // Der Fake hat kein Audit-Log und sagt das mit `IPC_006`.
    let _daemon = FakeServer::start(&harness);

    let output = harness.run(["audit", "export", "--format", "jsonl", "--out", "x.jsonl"]);
    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(stderr(&output).contains("[IPC_006]"), "{}", stderr(&output));
    assert!(
        !harness.path("work/x.jsonl").exists(),
        "no file-mode export stands in for the daemon"
    );
}

/// Ein Daemon, der einen Export meldet, den dieser Aufruf nicht sieht, ist
/// kein Erfolg: `AUDIT_008`, und „exported" steht nirgends (HUM-156).
#[test]
fn audit_export_that_the_caller_cannot_see_is_audit_008() {
    let harness = Harness::new();
    audit_chain(&harness.paths().audit_path(), 3);
    let _daemon = FakeServer::start_with_silent_export(&harness);

    let output = harness.run(["audit", "export", "--format", "jsonl", "--out", "x.jsonl"]);
    assert_ne!(code(&output), 0, "{}", stdout(&output));
    assert!(
        stderr(&output).contains("[AUDIT_008]"),
        "{}",
        stderr(&output)
    );
    assert!(!stdout(&output).contains("exported"), "{}", stdout(&output));
    assert!(!harness.path("work/x.jsonl").exists());
}

/// Der Export geht mit seinem Zeitraum über den Daemon: halboffen wie auf
/// der Kommandozeile, JSONL Byte für Byte, der relative Pfad beim Aufrufer.
#[test]
fn audit_export_goes_through_the_daemon_with_its_range() {
    let harness = Harness::new();
    let log = harness.paths().audit_path();
    audit_chain(&log, 5);
    let _daemon = common::AuditServer::start(&harness, AUDIT_KEY);

    let output = harness.run([
        "--json",
        "audit",
        "export",
        "--format",
        "jsonl",
        "--out",
        "range.jsonl",
        "--since",
        "2026-09-02T10:02:00Z",
        "--until",
        "2026-09-02T10:04:00Z",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(stdout(&output).trim()).expect("one JSON value");
    assert_eq!(value["source"], "daemon", "{value}");
    assert_eq!(value["exported"], 2, "{value}");
    let body = text_of(&harness.path("work/range.jsonl"));
    let log_text = text_of(&log);
    let expected: Vec<&str> = log_text
        .lines()
        .filter(|line| line.contains("10:02:00.000000Z") || line.contains("10:03:00.000000Z"))
        .collect();
    assert_eq!(expected.len(), 2);
    assert_eq!(body, format!("{}\n", expected.join("\n")));

    let csv = harness.run(["audit", "export", "--format", "csv", "--out", "all.csv"]);
    assert_eq!(code(&csv), 0, "{}", stderr(&csv));
    let text = text_of(&harness.path("work/all.csv"));
    assert!(
        text.starts_with("seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash\r\n"),
        "{text}"
    );
    assert_eq!(text.matches("\r\n").count(), 6, "{text}");
}
