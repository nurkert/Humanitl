//! `humanitl app [PATH]` (HUM-539): die laufende App über den Daemon
//! erreichen, sonst die App starten, sonst `CLI_007`.
//!
//! Nie die echte App: Ein Fake-Programm `humanitl-app` auf einem eigenen
//! `PATH` schreibt seine Argumente in eine Datei, und eine verbundene App
//! spielt ein Abonnent mit `SubscribeRequest.desktop_app` am Fake-Daemon.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::time::Duration;

use common::{FakeServer, Harness, PATIENCE, code, stderr, stdout};
use humanitl_ipc::v1;
use tokio_stream::StreamExt as _;

/// Ein Verzeichnis mit einem Fake-Programm `humanitl-app`, das jedes
/// Argument als eigene Zeile in `mark` schreibt.
#[cfg(not(target_os = "macos"))]
fn stub_app(harness: &Harness, mark: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = harness.path("stub-bin");
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join("humanitl-app");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{part}'\n\
             mv '{part}' '{mark}'\n",
            part = mark.with_extension("part").display(),
            mark = mark.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

/// `PATH` mit [`stub_app`] vorne und den Werkzeugen des Systems dahinter,
/// die das Fake-Programm braucht.
fn path_with(dir: &Path) -> String {
    format!("{}:/usr/bin:/bin", dir.display())
}

/// Wartet höchstens [`PATIENCE`] auf die Datei des Fake-Programms.
#[cfg(not(target_os = "macos"))]
fn wait_for(mark: &Path) -> Option<String> {
    let deadline = std::time::Instant::now() + PATIENCE;
    while std::time::Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(mark) {
            return Some(text);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn without_a_connected_app_and_without_the_app_it_is_cli_007_with_exit_1() {
    let harness = Harness::new();
    let empty = harness.path("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();
    let output = harness
        .command()
        .env("PATH", &empty)
        .args(["--json", "app", "."])
        .output()
        .unwrap();
    assert_eq!(code(&output), 1, "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("\"CLI_007\""), "stdout: {text}");
    assert!(
        text.contains("INSTALL.md"),
        "the fix names the install guide: {text}"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn without_a_connected_app_the_app_gets_exactly_project_and_the_canonical_folder() {
    let harness = Harness::new();
    let mark = harness.path("args.txt");
    let stub = stub_app(&harness, &mark);
    // Über einen Symlink, damit „kanonisch" etwas zu tun hat.
    let real = harness.path("work/real-project");
    std::fs::create_dir_all(&real).unwrap();
    let link = harness.path("work/linked");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let output = harness
        .command()
        .current_dir(&link)
        .env("PATH", path_with(&stub))
        .args(["app", "."])
        .output()
        .unwrap();
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("Started the app"),
        "{}",
        stdout(&output)
    );

    let args = wait_for(&mark).expect("the stub app was started");
    let canonical = real.canonicalize().unwrap();
    assert_eq!(args, format!("--project\n{}\n", canonical.display()));
}

/// Ein Ordner mit Steuerzeichen im Namen erreicht weder eine App noch wird
/// eine gestartet: `CLI_004`, Exit 1 (HUM-539, Review).
#[cfg(not(target_os = "macos"))]
#[test]
fn a_folder_with_a_control_character_is_refused_and_starts_nothing() {
    let harness = Harness::new();
    let mark = harness.path("args.txt");
    let stub = stub_app(&harness, &mark);
    let odd = harness.path("work/a\nb");
    std::fs::create_dir_all(&odd).unwrap();
    let output = harness
        .command()
        .env("PATH", path_with(&stub))
        .args(["--json", "app"])
        .arg(&odd)
        .output()
        .unwrap();
    assert_eq!(code(&output), 1, "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("\"CLI_004\""),
        "{}",
        stdout(&output)
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!mark.exists(), "no app was started");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn without_a_folder_the_app_starts_without_arguments() {
    let harness = Harness::new();
    let mark = harness.path("args.txt");
    let stub = stub_app(&harness, &mark);
    let output = harness
        .command()
        .env("PATH", path_with(&stub))
        .arg("app")
        .output()
        .unwrap();
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert_eq!(wait_for(&mark).expect("the stub app was started"), "");
}

/// Ein Abonnent wie die App: `Subscribe` mit `desktop_app`. Meldet über
/// `ready`, sobald der Strom steht, und gibt den Pfad der ersten Bitte
/// zurück, `None` nach [`PATIENCE`].
fn connected_app(
    harness: &Harness,
    ready: std::sync::mpsc::Sender<()>,
) -> std::thread::JoinHandle<Option<String>> {
    let paths = harness.paths();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let mut client = humanitl_ipc::client::connect_running(&paths)
                .await
                .expect("the fake daemon answers");
            let mut events = client
                .subscribe(v1::SubscribeRequest {
                    desktop_app: true,
                    ..v1::SubscribeRequest::default()
                })
                .await
                .expect("the stream opens")
                .into_inner();
            ready.send(()).unwrap();
            tokio::time::timeout(PATIENCE, async {
                while let Some(event) = events.next().await {
                    if let Some(v1::flow_event::Event::OpenInApp(open)) = event.ok()?.event {
                        return Some(open.path);
                    }
                }
                None
            })
            .await
            .ok()
            .flatten()
        })
    })
}

#[test]
fn a_connected_app_gets_the_folder_through_the_daemon_and_no_second_process_starts() {
    let harness = Harness::new();
    let _daemon = FakeServer::start(&harness);
    let mark = harness.path("args.txt");
    #[cfg(not(target_os = "macos"))]
    let path = path_with(&stub_app(&harness, &mark));
    #[cfg(target_os = "macos")]
    let path = path_with(&harness.path("work"));
    let (ready, is_ready) = std::sync::mpsc::channel();
    let app = connected_app(&harness, ready);
    is_ready.recv_timeout(PATIENCE).expect("the app subscribed");

    let project = harness.path("work/api");
    std::fs::create_dir_all(&project).unwrap();
    let output = harness
        .command()
        .env("PATH", path)
        .args(["app", "api"])
        .output()
        .unwrap();
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("Asked the running app"),
        "{}",
        stdout(&output)
    );
    let canonical = project.canonicalize().unwrap();
    assert_eq!(
        app.join().unwrap().as_deref(),
        Some(canonical.to_str().unwrap()),
        "the connected app got the canonical folder"
    );
    // Ein gestartetes Fake-Programm schriebe sofort; eine Sekunde ist das
    // Zwanzigfache dessen, was es dafür braucht.
    std::thread::sleep(Duration::from_secs(1));
    assert!(!mark.exists(), "no second app process was started");
}
