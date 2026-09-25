//! Socket-Aktivierung und Bereitmeldung des echten Daemons (HUM-053).
//!
//! Drei Fragen, jede am gebauten Binary und nicht an einer Hälfte davon:
//!
//! 1. Übernimmt der Daemon einen Socket, den ein anderer Prozess hält und als
//!    Deskriptor 3 übergibt, bedient er ihn, und lässt er die Datei am Ende
//!    liegen? `systemd-socket-activate` spielt dabei systemd: Es bindet den
//!    Pfad, wartet auf die erste Verbindung und startet dann den Daemon mit
//!    `LISTEN_FDS=1` und `LISTEN_PID`.
//! 2. Weist er einen übergebenen Socket an einem anderen Pfad ab
//!    (`DAEMON_013`), statt auf einem Socket zu lauschen, den kein Client
//!    findet?
//! 3. Schickt er `READY=1` an `NOTIFY_SOCKET`, und erst dann, wenn Token und
//!    Socket stehen (`Type=notify`)?
//!
//! Die ersten beiden brauchen `systemd-socket-activate`; fehlt es, sagt der
//! Test das und misst nichts. Die dritte braucht nur einen Datagramm-Socket.
//!
//! Die Wegwerf-Verzeichnisse liegen unter `/tmp`, damit der Socket-Pfad in
//! `sun_path` passt (108 Bytes).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use humanitl_ipc::{auth, client};

/// Ein Wegwerf-Baum mit den XDG-Verzeichnissen eines Laufs.
struct Tree {
    dir: tempfile::TempDir,
}

impl Tree {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("hum053")
            .tempdir_in("/tmp")
            .expect("a short temporary directory for sun_path");
        for name in ["run", "data", "config", "home"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        let runtime = dir.path().join("run").join("humanitl");
        std::fs::create_dir(&runtime).unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn runtime(&self) -> PathBuf {
        self.path("run").join("humanitl")
    }

    fn socket(&self) -> PathBuf {
        self.runtime().join("daemon.sock")
    }

    fn token(&self) -> PathBuf {
        self.runtime().join("token")
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("daemon.log")).unwrap_or_default()
    }

    /// Die Umgebung des Daemons als `NAME=WERT`.
    fn env(&self) -> Vec<(String, String)> {
        vec![
            ("XDG_RUNTIME_DIR".to_owned(), path_text(&self.path("run"))),
            ("XDG_DATA_HOME".to_owned(), path_text(&self.path("data"))),
            (
                "XDG_CONFIG_HOME".to_owned(),
                path_text(&self.path("config")),
            ),
            ("HOME".to_owned(), path_text(&self.path("home"))),
            ("HUMANITL_HOLD__TIMEOUT_SECS".to_owned(), "5".to_owned()),
            ("PATH".to_owned(), std::env::var("PATH").unwrap_or_default()),
        ]
    }

    /// Startet den Daemon hinter `systemd-socket-activate`, das `listen`
    /// bindet und bei der ersten Verbindung den Daemon startet.
    ///
    /// `None`, wenn es `systemd-socket-activate` hier nicht gibt.
    fn activate(&self, listen: &Path) -> Option<Child> {
        self.activate_with(listen, &[])
    }

    /// Wie [`Tree::activate`], mit weiteren Argumenten für den Daemon.
    fn activate_with(&self, listen: &Path, args: &[&str]) -> Option<Child> {
        let activator = find_program("systemd-socket-activate")?;
        let mut command = Command::new(activator);
        command.arg("--listen").arg(listen);
        // `systemd-socket-activate` reicht seine Umgebung nicht weiter, nur,
        // was mit `-E` genannt ist, und die Variablen der Übergabe.
        for (key, value) in self.env() {
            command.arg("-E").arg(format!("{key}={value}"));
        }
        command
            .arg(env!("CARGO_BIN_EXE_humanitld"))
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                std::fs::File::create(self.path("daemon.log")).unwrap(),
            ));
        Some(command.spawn().expect("systemd-socket-activate starts"))
    }
}

fn path_text(path: &Path) -> String {
    path.to_str().expect("a UTF-8 path").to_owned()
}

/// Ein Programm aus `PATH`, oder `None`.
fn find_program(name: &str) -> Option<PathBuf> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| candidate.is_file())
}

/// Wartet höchstens `limit` darauf, dass `ready` wahr wird.
fn wait_until(limit: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if ready() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    ready()
}

/// Schickt `SIGTERM` und wartet auf das Ende; gibt den Status zurück.
fn terminate(child: &mut Child) -> std::process::ExitStatus {
    let pid = libc::pid_t::try_from(child.id()).expect("a pid");
    // SAFETY: `kill(2)` mit der Nummer eines eigenen, noch nicht
    // eingesammelten Kindes; der Aufruf berührt keinen Speicher.
    #[allow(unsafe_code)]
    let sent = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(sent, 0, "SIGTERM reaches the daemon");
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("the child is waitable") {
            return status;
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            panic!("the daemon did not end within 20 s of SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Der Daemon übernimmt den Socket, den `systemd-socket-activate` hält,
/// antwortet darauf, und die Datei liegt nach seinem Ende noch da: Sie gehört
/// dem, der sie übergeben hat.
///
/// Ohne die Übernahme hielte der Daemon den Socket für den einer zweiten
/// Instanz (`DAEMON_003`), schriebe nie ein Token, und der Test liefe in die
/// Frist.
#[tokio::test]
async fn a_socket_passed_by_systemd_is_served_and_left_in_place() {
    let tree = Tree::new();
    let Some(mut child) = tree.activate(&tree.socket()) else {
        eprintln!(
            "SKIP a_socket_passed_by_systemd_is_served_and_left_in_place: no \
             systemd-socket-activate in PATH, so socket activation was not measured"
        );
        return;
    };
    assert!(
        wait_until(Duration::from_secs(10), || tree.socket().exists()),
        "systemd-socket-activate never bound {}",
        tree.socket().display()
    );
    // Die erste Verbindung startet den Daemon.
    let _wake = std::os::unix::net::UnixStream::connect(tree.socket()).expect("the socket accepts");
    assert!(
        wait_until(Duration::from_secs(20), || tree.token().exists()),
        "the daemon never wrote its token; log:\n{}",
        tree.log()
    );

    let token = auth::read_token(&tree.token()).unwrap();
    let mut grpc = client::connect_at(&tree.socket(), &token)
        .await
        .expect("the passed socket is served");
    let info = grpc.get_info(()).await.expect("GetInfo").into_inner();
    assert_eq!(info.proto_major, humanitl_ipc::PROTO_MAJOR);
    assert!(
        tree.log().contains("socket passed by systemd"),
        "the daemon says where its socket came from; log:\n{}",
        tree.log()
    );
    let mode = std::fs::metadata(tree.socket())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "a passed socket gets 0600 like a bound one");

    drop(grpc);
    let status = terminate(&mut child);
    assert!(
        status.success(),
        "SIGTERM is an orderly end: {status}; log:\n{}",
        tree.log()
    );
    assert!(!tree.token().exists(), "the token goes with the daemon");
    assert!(
        std::fs::symlink_metadata(tree.socket()).is_ok(),
        "the socket file belongs to systemd and stays"
    );
}

/// Ein übergebener Socket an einem anderen Pfad als dem, an dem die Clients
/// suchen, wird abgewiesen: Der Daemon endet mit `DAEMON_013`, statt auf einem
/// Socket zu lauschen, den niemand findet.
#[test]
fn a_socket_at_another_path_is_refused() {
    let tree = Tree::new();
    let elsewhere = tree.runtime().join("elsewhere.sock");
    let Some(mut child) = tree.activate(&elsewhere) else {
        eprintln!(
            "SKIP a_socket_at_another_path_is_refused: no systemd-socket-activate in PATH, so \
             socket activation was not measured"
        );
        return;
    };
    assert!(
        wait_until(Duration::from_secs(10), || elsewhere.exists()),
        "systemd-socket-activate never bound {}",
        elsewhere.display()
    );
    let _wake = std::os::unix::net::UnixStream::connect(&elsewhere).expect("the socket accepts");
    let ended = wait_until(Duration::from_secs(20), || {
        child.try_wait().expect("the child is waitable").is_some()
    });
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "the daemon kept running on a socket nobody looks for; log:\n{}",
            tree.log()
        );
    }
    let status = child.wait().expect("the status");
    assert!(!status.success(), "the start fails: {status}");
    let log = tree.log();
    assert!(
        log.contains("DAEMON_013"),
        "the reason is DAEMON_013; log:\n{log}"
    );
    assert!(
        !tree.token().exists(),
        "no token for a daemon that never served"
    );
}

/// `READY=1` kommt an `NOTIFY_SOCKET` an, und wenn es ankommt, stehen Token
/// und Socket schon: Eine Unit, die nach dem Dienst startet, findet einen
/// Daemon, der antwortet.
#[test]
fn the_daemon_reports_ready_once_it_listens() {
    let tree = Tree::new();
    let notify = tree.path("notify");
    let receiver = std::os::unix::net::UnixDatagram::bind(&notify).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_humanitld"));
    command
        .env_clear()
        .envs(tree.env())
        .env("NOTIFY_SOCKET", &notify)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            std::fs::File::create(tree.path("daemon.log")).unwrap(),
        ));
    let mut child = command.spawn().expect("the daemon starts");

    let mut buffer = [0_u8; 256];
    let read = receiver.recv(&mut buffer);
    // Was bei der Meldung schon steht, wird sofort angesehen, bevor der
    // Daemon Zeit hat, es nachzuholen.
    let socket_there = tree.socket().exists();
    let token_there = tree.token().exists();
    let read = match read {
        Ok(read) => read,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "no notification within 30 s ({error}); log:\n{}",
                tree.log()
            );
        }
    };
    assert_eq!(&buffer[..read], b"READY=1", "the first message is READY=1");
    assert!(socket_there, "the socket stands when READY=1 arrives");
    assert!(token_there, "the token stands when READY=1 arrives");

    let status = terminate(&mut child);
    assert!(status.success(), "SIGTERM is an orderly end: {status}");
    let read = receiver.recv(&mut buffer).expect("a second message");
    assert_eq!(
        &buffer[..read],
        b"STOPPING=1",
        "the farewell is announced as well"
    );
    assert!(
        !tree.socket().exists(),
        "a socket the daemon bound itself goes with it"
    );
}

/// Startet den Daemon über `sh`, das `LISTEN_PID` auf seine eigene Nummer
/// setzt (`exec` behält sie) und Deskriptor 3 nach `redirect` einrichtet.
///
/// So lässt sich eine Übergabe fälschen, ohne dass der Test selbst an
/// Deskriptoren schreibt: genau der Fall, gegen den die Übernahme sich wehren
/// muss, wenn jemand die Variablen setzt, ohne einen Socket zu übergeben.
fn forged_handover(tree: &Tree, redirect: &str) -> std::process::Output {
    let script = format!("LISTEN_PID=$$ LISTEN_FDS=1 exec \"$0\" {redirect}");
    Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_humanitld"))
        .arg(tree.path("plain"))
        .env_clear()
        .envs(tree.env())
        .stdin(Stdio::null())
        .output()
        .expect("sh starts")
}

/// Eine gefälschte Übergabe endet mit `DAEMON_013` und ohne Panik: einmal ist
/// Deskriptor 3 geschlossen, einmal eine gewöhnliche Datei.
#[test]
fn a_forged_handover_ends_with_daemon_013_and_no_panic() {
    let tree = Tree::new();
    std::fs::write(tree.path("plain"), b"not a socket").unwrap();
    for (case, redirect) in [("closed", "3<&-"), ("plain file", "3<\"$1\"")] {
        let output = forged_handover(&tree, redirect);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{case}: an orderly refusal, not a panic and not a start: {stderr}"
        );
        assert!(stderr.contains("DAEMON_013"), "{case}: {stderr}");
        assert!(!stderr.contains("panicked"), "{case}: {stderr}");
        assert!(
            !tree.token().exists(),
            "{case}: no token for a daemon that never served"
        );
    }
}

/// Die aufgezeichnete Sitzung, die der Fake abspielt.
fn fake_session() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/sessions/mixed.jsonl")
}

/// `--fake` mit einer Übergabe, deren `LISTEN_FDS` nicht zu lesen ist: Der
/// Fake startet nicht, sondern endet mit `DAEMON_013`.
#[test]
fn the_fake_refuses_a_forged_handover() {
    let tree = Tree::new();
    // Mit Frist: Ein Fake, der trotz der Übergabe startet, liefe sonst bis
    // zum Ende des Testlaufs, und aus dem roten Test würde ein hängender.
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("LISTEN_PID=$$ LISTEN_FDS=two exec \"$0\" --fake \"$1\"")
        .arg(env!("CARGO_BIN_EXE_humanitld"))
        .arg(fake_session())
        .env_clear()
        .envs(tree.env())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            std::fs::File::create(tree.path("daemon.log")).unwrap(),
        ))
        .spawn()
        .expect("sh starts");
    let ended = wait_until(Duration::from_secs(20), || {
        child.try_wait().expect("the child is waitable").is_some()
    });
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "the fake started despite an unreadable handover; log:\n{}",
            tree.log()
        );
    }
    let status = child.wait().expect("the status");
    let stderr = tree.log();
    assert_eq!(status.code(), Some(1), "the fake does not start: {stderr}");
    assert!(stderr.contains("DAEMON_013"), "{stderr}");
    assert!(
        !tree.token().exists(),
        "no token for a fake that never served"
    );
}

/// `--fake` hinter einer gültigen Übergabe: Der Fake bedient keinen
/// übergebenen Socket und endet deshalb mit `DAEMON_013`, statt ihn offen und
/// ungeprüft neben einem selbst gebundenen stehen zu lassen.
#[test]
fn the_fake_refuses_a_valid_handover() {
    let tree = Tree::new();
    let session = fake_session();
    let session = path_text(&session);
    let Some(mut child) = tree.activate_with(&tree.socket(), &["--fake", &session]) else {
        eprintln!(
            "SKIP the_fake_refuses_a_valid_handover: no systemd-socket-activate in PATH, so \
             socket activation was not measured"
        );
        return;
    };
    assert!(
        wait_until(Duration::from_secs(10), || tree.socket().exists()),
        "systemd-socket-activate never bound {}",
        tree.socket().display()
    );
    let _wake = std::os::unix::net::UnixStream::connect(tree.socket()).expect("the socket accepts");
    let ended = wait_until(Duration::from_secs(20), || {
        child.try_wait().expect("the child is waitable").is_some()
    });
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "the fake kept running behind a passed socket; log:\n{}",
            tree.log()
        );
    }
    let status = child.wait().expect("the status");
    assert_eq!(status.code(), Some(1), "the fake does not start: {status}");
    let log = tree.log();
    assert!(log.contains("DAEMON_013"), "{log}");
    assert!(
        log.contains("--fake does not support socket activation"),
        "{log}"
    );
    assert!(
        !tree.token().exists(),
        "no token for a fake that never served"
    );
}

/// Die Kommandozeile neben diesem Daemon, wenn sie gebaut ist.
///
/// `cargo test --workspace` (`make rust-test`) baut jedes Binary des
/// Arbeitsbereichs, bevor ein Test läuft; `cargo test -p humanitld` allein
/// baut `humanitl` nicht.
fn sibling_cli() -> Option<PathBuf> {
    let cli = Path::new(env!("CARGO_BIN_EXE_humanitld")).with_file_name("humanitl");
    cli.is_file().then_some(cli)
}

/// Führt `humanitl daemon status` in der Umgebung von `tree` aus, mit leerem
/// `PATH`: Kein `systemctl` der echten Sitzung wird erreicht.
fn daemon_status(tree: &Tree, cli: &Path) -> std::process::Output {
    let mut command = Command::new(cli);
    command
        .args(["daemon", "status"])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in tree.env() {
        if key != "PATH" {
            command.env(key, value);
        }
    }
    command.env("PATH", "");
    let mut child = command.spawn().expect("humanitl starts");
    let ended = wait_until(Duration::from_secs(30), || {
        child.try_wait().expect("the cli is waitable").is_some()
    });
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
        panic!("humanitl daemon status hung; log:\n{}", tree.log());
    }
    child.wait_with_output().expect("the output")
}

/// Ein Socket hinter `systemd-socket-activate`, hinter dem noch kein Daemon
/// läuft und kein Token liegt (HUM-164). `None`, wenn es
/// `systemd-socket-activate` hier nicht gibt.
fn sleeping_socket(test: &str) -> Option<(Tree, Child)> {
    let tree = Tree::new();
    let Some(child) = tree.activate(&tree.socket()) else {
        eprintln!(
            "SKIP {test}: no systemd-socket-activate in PATH, so the wake-up was not measured"
        );
        return None;
    };
    assert!(
        wait_until(Duration::from_secs(10), || tree.socket().exists()),
        "systemd-socket-activate never bound {}",
        tree.socket().display()
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !tree.token().exists(),
        "the daemon runs before any client connected; log:\n{}",
        tree.log()
    );
    Some((tree, child))
}

/// HUM-164: Ohne laufenden Daemon und ohne Token öffnet
/// `humanitl_ipc::client::connect` den Socket trotzdem, weckt damit den
/// Daemon und wartet auf sein Token; danach antwortet `GetInfo`.
///
/// Ohne den Weckruf läse der Client nur das fehlende Token und endete mit
/// `DAEMON_001`, und der Daemon startete nie.
#[tokio::test]
async fn a_client_without_a_token_wakes_the_socket() {
    let Some((tree, mut child)) = sleeping_socket("a_client_without_a_token_wakes_the_socket")
    else {
        return;
    };
    let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
        "XDG_RUNTIME_DIR",
        path_text(&tree.path("run")),
    )]));
    let connected = client::connect(&paths).await;
    let mut grpc = match connected {
        Ok(grpc) => grpc,
        Err(diagnostic) => {
            let _ = terminate(&mut child);
            panic!("the client did not wake the daemon: {}", diagnostic.why);
        }
    };
    let info = grpc.get_info(()).await.expect("GetInfo").into_inner();
    assert_eq!(info.proto_major, humanitl_ipc::PROTO_MAJOR);

    drop(grpc);
    let status = terminate(&mut child);
    assert!(status.success(), "{status}; log:\n{}", tree.log());
}

/// Das Akzeptanzkriterium von HUM-164 wörtlich: Unter
/// `systemd-socket-activate` ohne laufenden Daemon endet `humanitl daemon
/// status` mit 0, ohne dass der Test vorher selbst verbindet.
#[test]
fn daemon_status_wakes_the_socket_and_ends_with_0() {
    let Some(cli) = sibling_cli() else {
        eprintln!(
            "SKIP daemon_status_wakes_the_socket_and_ends_with_0: humanitl is not built next \
             to humanitld; cargo test --workspace builds it"
        );
        return;
    };
    let Some((tree, mut child)) = sleeping_socket("daemon_status_wakes_the_socket_and_ends_with_0")
    else {
        return;
    };
    let output = daemon_status(&tree, &cli);
    let status = terminate(&mut child);
    assert_eq!(
        output.status.code(),
        Some(0),
        "humanitl daemon status against a sleeping socket; stderr:\n{}\nlog:\n{}",
        String::from_utf8_lossy(&output.stderr),
        tree.log()
    );
    let table = String::from_utf8_lossy(&output.stdout);
    assert!(table.contains("daemon"), "{table}");
    assert!(status.success(), "{status}; log:\n{}", tree.log());
}
