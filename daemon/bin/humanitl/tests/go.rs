//! `humanitl go [AGENT]` gegen den Fake-Daemon (#8): was im Start ankommt,
//! wenn ein Mensch ein Agentenwort und Schalter tippt, und was nie ankommt.
//!
//! Der Fake schreibt jeden `Sandbox(Start)` und jedes `Projects.save_choice`
//! mit (`common::FakeServer::starts`, `saved_choices`, `saved_paths`). Er startet keine
//! Sandbox; ein Start, der die Kernel-Stufe verlangt, endet bei ihm wie auf
//! einem Rechner ohne sie, mit `SANDBOX_045`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{FakeServer, FirstStartTweaks, Harness, PATIENCE, code, stderr};
use humanitl_ipc::v1;

/// Ein Aufruf mit Frist: Ein Test, der hängt, wäre einer, der nichts prüft.
fn go(harness: &Harness, args: &[&str]) -> Output {
    harness
        .run_bounded(args, PATIENCE)
        .unwrap_or_else(|| panic!("{args:?} did not end within {PATIENCE:?}"))
}

/// Der Wert von `key` in `Start.cli_overrides`, wenn er dasteht.
fn override_of<'a>(start: &'a v1::sandbox_request::Start, key: &str) -> Option<&'a str> {
    start
        .cli_overrides
        .iter()
        .find(|pair| pair.path == key)
        .map(|pair| pair.value.as_str())
}

/// Der eine Start, den ein Aufruf geschickt hat.
fn only_start(server: &FakeServer, output: &Output) -> v1::sandbox_request::Start {
    let starts = server.starts();
    assert_eq!(starts.len(), 1, "{starts:?}\n{}", stderr(output));
    starts.into_iter().next().expect("one start")
}

/// Das Arbeitsverzeichnis der Harness, wie der Prozess es sieht.
fn work(harness: &Harness) -> PathBuf {
    std::fs::canonicalize(harness.path("work")).expect("the work directory")
}

/// `go claude` setzt den Adapter `claude` für genau diesen Start, im
/// aktuellen Ordner. Ein Ordner `claude` darin ist kein Agent und nicht der
/// Projektordner: Das Wort ist immer der Agent.
#[test]
fn go_claude_sets_the_adapter_of_the_start() {
    let harness = Harness::new();
    std::fs::create_dir_all(harness.path("work/claude")).expect("a folder named claude");
    let server = FakeServer::start(&harness);

    let output = go(&harness, &["go", "claude"]);
    let start = only_start(&server, &output);
    assert_eq!(
        override_of(&start, "agent.adapter"),
        Some("claude"),
        "{start:?}"
    );
    assert!(
        start.command.is_empty(),
        "the daemon starts the agent with its adapter's own start: {start:?}"
    );
    assert_eq!(Path::new(&start.work_dir), work(&harness), "{start:?}");
}

/// `go claude` schickt auch neben einem `agent.command` in `config.toml`
/// ein leeres Kommando: Der Daemon startet den Agenten mit dem eigenen Start
/// seines Adapters und lässt den Befehl der tieferen Ebene fallen
/// (`session::command_follows_the_agent`). Mit Wörtern nach `--` startet das
/// Programm des Adapters mit ihnen.
///
/// Mutation proof: sending the adapter's program without words (a start as
/// the caller's command, without the adapter's own start) fails the first
/// assertion.
#[test]
fn go_claude_leaves_the_start_of_the_agent_to_the_daemon() {
    let harness = Harness::new();
    let config = harness.path("config/humanitl/config.toml");
    std::fs::create_dir_all(config.parent().expect("a directory")).expect("the directory");
    std::fs::write(&config, "[agent]\ncommand = [\"/opt/mypi\"]\n").expect("the config");
    let server = FakeServer::start(&harness);

    let _ = go(&harness, &["go", "claude"]);
    let _ = go(&harness, &["go", "claude", "--", "--resume"]);
    let starts = server.starts();
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!(starts[0].command.is_empty(), "{starts:?}");
    assert_eq!(starts[1].command, ["claude", "--resume"], "{starts:?}");
}

/// Die Auflösung des Clients liest das Projekt-Profil des Ordners dieses
/// Aufrufs, nicht das des in `config.toml` gespeicherten Ordners. Dessen
/// Profil trägt hier einen gesperrten Schlüssel; liest der Client es, endet
/// `go` mit `CONFIG_003`, obwohl der gewählte Ordner nichts davon hat.
///
/// Mutation proof: without `sandbox.work_dir` of the chosen folder among the
/// values of the command line, both runs end with `CONFIG_003` and start
/// nothing.
#[test]
fn the_project_profile_is_that_of_the_chosen_folder() {
    let harness = Harness::new();
    let stored = harness.path("stored");
    std::fs::create_dir_all(stored.join(".humanitl")).expect("the stored folder");
    std::fs::write(
        stored.join(".humanitl/profile.toml"),
        "[config.hold]\nask_mode = \"terminal\"\n",
    )
    .expect("the profile of the stored folder");
    std::fs::create_dir_all(harness.path("work/other")).expect("the chosen folder");
    let config = harness.path("config/humanitl/config.toml");
    std::fs::create_dir_all(config.parent().expect("a directory")).expect("the directory");
    std::fs::write(
        &config,
        format!("[sandbox]\nwork_dir = {:?}\n", stored.display().to_string()),
    )
    .expect("the config");
    let server = FakeServer::start(&harness);

    for args in [&["go", "--dir", "other"][..], &["go"]] {
        let output = go(&harness, args);
        let text = stderr(&output);
        assert!(!text.contains("CONFIG_003"), "{args:?}: {text}");
        assert!(!text.contains("CLI_002"), "{args:?}: {text}");
    }
    let starts = server.starts();
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert_eq!(Path::new(&starts[0].work_dir), work(&harness).join("other"));
    assert_eq!(Path::new(&starts[1].work_dir), work(&harness));
}

/// Ein globaler Schalter, der dem Wort oder einem Ordnerschalter
/// widerspricht, ist `CLI_004`, und nichts startet; derselbe Wert ist kein
/// Widerspruch.
///
/// Mutation proof: without the check in `agree` the word wins silently and
/// the fake records a start.
#[test]
fn a_global_switch_against_the_word_or_a_folder_switch_is_cli_004() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);

    for args in [
        &["--agent-adapter", "pi", "go", "claude"][..],
        &["--sandbox-mode", "disposable", "go", "--rw"],
        &["--sandbox-work-mode", "rw", "go", "--ro"],
    ] {
        let output = go(&harness, args);
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{args:?}: {text}");
        assert!(text.starts_with("error[CLI_004]: "), "{args:?}: {text}");
        assert!(text.contains("name it once"), "{args:?}: {text}");
    }
    assert!(server.starts().is_empty(), "{:?}", server.starts());

    let _ = go(&harness, &["--agent-adapter", "claude", "go", "claude"]);
    assert_eq!(server.starts().len(), 1, "{:?}", server.starts());
}

/// Vorschau, Ziehen und Vorschlag schicken einem Daemon vor 1.61 keinen
/// Agenten, den er mit `CONFIG_003` ablehnte: `CLI_004` mit der Fassung,
/// bevor etwas gesendet wird.
///
/// Mutation proof: without `agent_reaches_the_daemon` in a path, that path
/// sends the key, and the answer is not `CLI_004` naming 1.61.
#[test]
fn an_old_daemon_gets_no_agent_in_any_request() {
    let harness = Harness::new();
    let _server = FakeServer::start_with_proto_minor(&harness, 60);

    for args in [
        &["--agent-adapter", "claude", "sandbox", "plan"][..],
        &[
            "--agent-adapter",
            "claude",
            "sandbox",
            "image",
            "pull",
            "--yes",
        ],
        &["--agent-adapter", "claude", "projects", "proposal"],
        &["go", "claude"],
    ] {
        let output = go(&harness, args);
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{args:?}: {text}");
        assert!(text.contains("CLI_004"), "{args:?}: {text}");
        assert!(text.contains("1.61"), "{args:?}: {text}");
    }
}

/// Ohne Wort in einem bekannten Projekt gilt der Agent der Projektwahl: Der
/// Start nennt keinen Agenten, und der Daemon löst mit der Wahl auf
/// (HUM-532). `config.toml` nennt einen anderen; schickte `go` den, wäre die
/// Wahl überstimmt.
///
/// Mutation proof: sending the locally resolved `agent.adapter` with every
/// start (ignoring the project choice) puts `pi` into the start, and this
/// test fails.
#[test]
fn go_without_a_word_in_a_known_project_takes_the_agent_of_the_project_choice() {
    let harness = Harness::new();
    let config = harness.path("config/humanitl/config.toml");
    std::fs::create_dir_all(config.parent().expect("a directory")).expect("the directory");
    std::fs::write(&config, "[agent]\nadapter = \"pi\"\n").expect("the config");
    let server = FakeServer::start_with_known_project(&harness, &work(&harness));

    let output = go(&harness, &["go"]);
    let start = only_start(&server, &output);
    assert_eq!(override_of(&start, "agent.adapter"), None, "{start:?}");
    assert_eq!(
        start.sandbox_mode, "",
        "the project choice decides: {start:?}"
    );
    assert_eq!(start.work_mode, "", "the project choice decides: {start:?}");
    assert_eq!(Path::new(&start.work_dir), work(&harness), "{start:?}");
    let text = stderr(&output);
    assert!(
        first_start_lines(&text).is_empty(),
        "a known project: {text}"
    );
}

/// Die Zeilen des Erststarts ohne Bildschirm in `text` (#9).
fn first_start_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("[humanitl] first start in "))
        .collect()
}

/// Ohne Terminal startet ein Ordner, den das Register nicht kennt, mit den
/// Vorgaben und genau einer Zeile dazu: Ordnername, Vorgaben ohne Leitwort
/// und wie man sie ändert (#9). Die Wahl des Vorschlags ist am Ordner
/// gespeichert, bevor der Start geht, und der zweite `go` zeigt keine Zeile
/// mehr und speichert nichts.
///
/// Mutation proof: dropping the `save` call of the path without a screen
/// leaves the fake without the folder; the second run then shows the line
/// again and the last assertions fail.
#[test]
fn go_in_a_new_project_saves_the_defaults_and_says_so_once() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);

    let output = go(&harness, &["go"]);
    let _start = only_start(&server, &output);
    let text = stderr(&output);
    let lines = first_start_lines(&text);
    assert_eq!(lines.len(), 1, "{text}");
    let name = work(&harness)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a name")
        .to_owned();
    assert!(
        lines[0].contains(&format!("first start in {name}, defaults saved: ")),
        "{text}"
    );
    assert!(!lines[0].contains("Defaults:"), "{text}");
    assert!(
        lines[0].contains("Change them with humanitl settings --project "),
        "{text}"
    );
    assert_eq!(server.saved_paths(), [work(&harness)]);

    let again = go(&harness, &["go"]);
    let text = stderr(&again);
    assert!(first_start_lines(&text).is_empty(), "{text}");
    assert_eq!(server.saved_choices(), 1, "the second start saves nothing");
    assert_eq!(server.starts().len(), 2);
}

/// `--yes` zeigt keinen Bildschirm, auch nicht mit `--json`: kein Byte einer
/// Steuerfolge und nichts vom Erststart auf `stdout`, genau eine Zeile des
/// Erststarts auf `stderr`, die Wahl gespeichert (#9).
#[test]
fn go_yes_and_json_say_the_first_start_in_one_line() {
    for flags in [
        &["go", "--yes"][..],
        &["--json", "go"],
        &["--json", "go", "--yes"],
    ] {
        let harness = Harness::new();
        let server = FakeServer::start(&harness);
        let output = go(&harness, flags);
        let out = common::stdout(&output);
        let text = stderr(&output);
        assert!(
            !out.contains('\u{1b}'),
            "{flags:?}: an escape on stdout: {out:?}"
        );
        assert!(!out.contains("first start"), "{flags:?}: {out}");
        assert!(!out.contains("Customize"), "{flags:?}: {out}");
        assert_eq!(first_start_lines(&text).len(), 1, "{flags:?}: {text}");
        assert_eq!(server.saved_paths(), [work(&harness)], "{flags:?}");
        assert_eq!(server.starts().len(), 1, "{flags:?}");
    }
}

/// `--rw`, `--ro` und `--disposable` setzen Modus und Schreibmodus des
/// Starts; je zwei davon lehnt schon `clap` ab (`CLI_004`), und nichts
/// startet.
///
/// Mutation proof: mapping `--ro` to `work_mode = "rw"` (or dropping the
/// switch from `RunArgs.direct`) fails the first loop.
#[test]
fn the_folder_switches_set_the_mode_of_the_start() {
    for (switch, mode, work_mode) in [
        ("--rw", "direct", "rw"),
        ("--ro", "direct", "ro"),
        ("--disposable", "disposable", "rw"),
    ] {
        let harness = Harness::new();
        let server = FakeServer::start(&harness);
        let output = go(&harness, &["go", switch]);
        let start = only_start(&server, &output);
        assert_eq!(start.sandbox_mode, mode, "{switch}: {start:?}");
        assert_eq!(start.work_mode, work_mode, "{switch}: {start:?}");
    }

    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    for pair in [
        ["--rw", "--ro"],
        ["--disposable", "--ro"],
        ["--disposable", "--rw"],
    ] {
        let output = go(&harness, &["go", pair[0], pair[1]]);
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{pair:?}: {text}");
        assert!(text.starts_with("error[CLI_004]: "), "{pair:?}: {text}");
        assert!(text.contains("cannot be used with"), "{pair:?}: {text}");
    }
    assert!(server.starts().is_empty(), "{:?}", server.starts());
}

/// `--dir` wählt den Projektordner, relativ zum aktuellen Verzeichnis.
/// Neben `--work` ist es `CLI_004`, auf einer Datei `SANDBOX_006`; in beiden
/// Fällen startet nichts.
#[test]
fn dir_names_the_project_folder_once() {
    let harness = Harness::new();
    std::fs::create_dir_all(harness.path("work/sub")).expect("a folder");
    std::fs::write(harness.path("work/file"), "x").expect("a file");
    let server = FakeServer::start(&harness);

    let output = go(&harness, &["go", "--dir", "sub"]);
    let start = only_start(&server, &output);
    assert_eq!(
        Path::new(&start.work_dir),
        work(&harness).join("sub"),
        "{start:?}"
    );

    let work = harness.path("work");
    let work = work.to_str().expect("a UTF-8 path");
    let both = go(&harness, &["--work", work, "go", "--dir", "sub"]);
    let text = stderr(&both);
    assert_eq!(code(&both), 1, "{text}");
    assert!(text.starts_with("error[CLI_004]: "), "{text}");
    assert!(text.contains("--dir and --work"), "{text}");

    let file = go(&harness, &["go", "--dir", "file"]);
    let text = stderr(&file);
    assert_eq!(code(&file), 1, "{text}");
    assert!(text.starts_with("error[SANDBOX_006]: "), "{text}");
    assert!(text.contains("not a directory"), "{text}");

    assert_eq!(server.starts().len(), 1, "{:?}", server.starts());
}

/// `--kernel` verlangt die Kernel-Stufe für diesen Start. Wo es sie nicht
/// gibt, steht ihr Befund da, und nichts startet auf der geteilten Stufe.
///
/// Mutation proof: dropping the floor from the start (a silent fallback to
/// the shared level) lets the fake start the session; `SANDBOX_045` is then
/// missing and this test fails.
#[test]
fn kernel_without_a_kernel_level_is_its_finding() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);

    let output = go(&harness, &["go", "claude", "--kernel"]);
    let text = stderr(&output);
    let start = only_start(&server, &output);
    assert_eq!(
        override_of(&start, "sandbox.min_isolation"),
        Some("kernel"),
        "{start:?}"
    );
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("SANDBOX_045"), "{text}");
    assert!(!text.contains("CLI_001"), "nothing ran: {text}");
}

/// Kein Schalter ändert die gespeicherte Projektwahl: Für das bekannte
/// Projekt ruft `go` `Projects.save_choice` nie.
///
/// Mutation proof: a `save_choice` call in `go` (for instance saving the
/// agent word as the project's agent) fails this test.
#[test]
fn switches_leave_the_stored_project_choice_alone() {
    let harness = Harness::new();
    std::fs::create_dir_all(harness.path("work/sub")).expect("a folder");
    let server = FakeServer::start_with_known_project(&harness, &work(&harness));

    for args in [
        &["go", "claude", "--ro"][..],
        &["go", "pi", "--rw", "--kernel"],
        &["go", "codex", "--disposable", "--dir", "sub"],
    ] {
        let _ = go(&harness, args);
    }
    assert_eq!(server.starts().len(), 3, "{:?}", server.starts());
    // Nur der neue Ordner `sub` bekommt seinen Erststart, mit der Wahl des
    // Vorschlags (#9); die Wahl des bekannten Projekts bleibt unberührt.
    assert_eq!(server.saved_paths(), [work(&harness).join("sub")]);
    assert_eq!(server.saved_choices(), 1);
}

/// `go foo` ist ein Befund mit der Liste der Agenten und Exit 1, bevor
/// irgendetwas startet.
#[test]
fn an_unknown_agent_is_a_finding_with_the_list() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);

    let output = go(&harness, &["go", "foo"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.starts_with("error[CLI_004]: "), "{text}");
    for id in humanitl_sandbox::AdapterRegistry::builtin().ids() {
        assert!(text.contains(id), "{id} is missing: {text}");
    }
    assert!(server.starts().is_empty(), "{:?}", server.starts());
}

/// `go claude -- --resume` gibt `--resume` an den Agenten weiter.
///
/// Mutation proof: passing the words after `--` as the whole command (as
/// without an agent word) starts `--resume` instead of `claude --resume`.
#[test]
fn the_words_after_the_dashes_go_to_the_agent() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);

    let output = go(&harness, &["go", "claude", "--", "--resume"]);
    let start = only_start(&server, &output);
    assert_eq!(start.command, ["claude", "--resume"], "{start:?}");
    assert_eq!(
        override_of(&start, "agent.adapter"),
        Some("claude"),
        "{start:?}"
    );
}

/// `go --yes` in einem neuen Ordner: genau eine Zeile des Erststarts vor
/// allem anderen auf `stderr`, kein Hinweis zu Anfragen ohne Regel daneben
/// (die Zeile nennt den Frage-Modus), und nichts vom Erststart auf
/// `stdout`. Ein bekannter Ordner bekommt den Hinweis weiter (#9).
///
/// Mutation proof: printing `where_decisions_happen` also after the line
/// (dropping the check of `first_line`) puts a second line before the
/// agent's output, and the exact comparison fails.
#[test]
fn go_yes_says_one_line_and_not_the_note_on_requests_without_a_rule() {
    let harness = Harness::new();
    let server = FakeServer::start(&harness);
    let output = go(&harness, &["go", "--yes"]);
    let text = stderr(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines
            .first()
            .is_some_and(|line| line.starts_with("[humanitl] first start in ")),
        "the line comes first: {text}"
    );
    assert_eq!(first_start_lines(&text).len(), 1, "{text}");
    assert!(!text.contains("a request without a rule"), "{text}");
    assert!(
        common::stdout(&output).is_empty(),
        "{:?}",
        common::stdout(&output)
    );
    assert_eq!(server.starts().len(), 1);

    let again = go(&harness, &["go", "--yes"]);
    let text = stderr(&again);
    assert!(text.contains("a request without a rule"), "{text}");
}

/// Scheitert `Projects.proposal`, startet in einem neuen Ordner nichts: der
/// Befund des Daemons, Exit 1, keine Wahl gespeichert (#9).
///
/// Mutation proof: answering a failed proposal with `Ok(None)` (the old
/// silent fallback) lets the fake record a start.
#[test]
fn a_failed_proposal_starts_nothing() {
    let harness = Harness::new();
    let server = FakeServer::start_with_first_start(
        &harness,
        FirstStartTweaks {
            refuse_proposal: true,
            ..FirstStartTweaks::default()
        },
    );
    for flags in [&["go"][..], &["go", "--yes"]] {
        let output = go(&harness, flags);
        let text = stderr(&output);
        assert_eq!(code(&output), 1, "{flags:?}: {text}");
        assert!(text.contains("CONFIG_003"), "{flags:?}: {text}");
        assert!(text.contains("refuses every start proposal"), "{text}");
    }
    assert!(server.starts().is_empty(), "{:?}", server.starts());
    assert_eq!(server.saved_choices(), 0);
}

/// Lehnt der Daemon das Speichern ab, steht keine Zeile des Erststarts da
/// (sie sagt, die Vorgaben seien gespeichert), und nichts startet (#9,
/// Review).
///
/// Mutation proof: printing the line before `save` puts it on stderr here.
#[test]
fn a_refused_save_prints_no_line_and_starts_nothing() {
    let harness = Harness::new();
    let server = FakeServer::start_with_first_start(
        &harness,
        FirstStartTweaks {
            refuse_save: true,
            ..FirstStartTweaks::default()
        },
    );
    let output = go(&harness, &["go", "--yes"]);
    let text = stderr(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("CONFIG_024"), "{text}");
    assert!(first_start_lines(&text).is_empty(), "{text}");
    assert!(server.starts().is_empty(), "{:?}", server.starts());
}

/// `humanitl go` in einem anderen Ordner, während eine Sitzung läuft
/// (GitHub #13): `CLI_005`, und der Fix führt zur laufenden Sitzung,
/// `humanitl go --dir` mit ihrem Ordner; der Text nennt auch `humanitl
/// sandbox attach`. Nichts stoppt die laufende Sitzung.
///
/// Der Befund kommt aus dem Daemon (`second_start_refused`); der Fake
/// schickt denselben. Mutation proof: the fix of before #13 (none) in
/// `second_start_refused` fails the assertion on the fix.
#[test]
fn go_in_another_folder_gets_cli_005_with_the_way_to_the_running_session() {
    let harness = Harness::new();
    let server = FakeServer::start_tracking_sessions(&harness, &work(&harness));

    // Ohne Terminal liest `go` nur mit; der Fake lässt die Sitzung laufen.
    let _first = go(&harness, &["go"]);
    assert_eq!(server.starts().len(), 1);

    let other = harness.path("other");
    std::fs::create_dir_all(&other).expect("a second folder");
    let output = go(&harness, &["go", "--dir", other.to_str().expect("a path")]);
    let text = stderr(&output);
    assert_ne!(code(&output), 0, "{text}");
    assert!(text.contains("CLI_005"), "{text}");
    assert!(
        text.contains(&format!("humanitl go --dir {}", work(&harness).display())),
        "the fix leads to the running session: {text}"
    );
    assert!(text.contains("humanitl sandbox attach"), "{text}");
    assert_eq!(server.stops(), 0, "nothing stopped the running session");
}
