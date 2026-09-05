//! `Sandbox(Start)` von Ende zu Ende: die drei Garantien und fail-closed
//! (HUM-041).
//!
//! Die Prüfung selbst hat ihre Tests in `crates/sandbox/tests/launcher.rs`.
//! Hier steht das Stück darüber: dass der Dienst sie an einer wirklich
//! laufenden Sandbox misst, die Ergebnisse zwischen `Status(starting)` und
//! `Status(running)` sendet und bei einer roten Garantie die Sandbox beendet,
//! statt `running` zu melden. Genau das lässt sich an `first_failure` allein
//! nicht zeigen: Wer den Rückgabewert von `check_isolation_or_kill`
//! ignorierte, machte keinen Helfer rot.
//!
//! Der rote Fall ist der aus dem Akzeptanzkriterium und keine gestellte
//! Meldung: Vor dem Start liegt eine **echte** Unix-Socket-Datei im
//! Projektverzeichnis. Der Suchlauf des Shims findet sie in `/work`, meldet
//! `single_socket fail`, und der Dienst muss daraus `SANDBOX_015` und
//! `Status(failed)` machen.
//!
//! Der Test braucht `bwrap`, einen Kernel mit unprivilegierten
//! Nutzer-Namensräumen und den gebauten Shim neben dem Testbinary. Fehlt
//! eines, sagt er es auf stderr und endet grün: „kein `bwrap` auf dieser
//! Maschine" ist eine Aussage über die Maschine.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use humanitl_config::{Config, Env, Paths};
use humanitl_core::SessionId;
use humanitl_ipc::sandbox::SandboxPorts;
use humanitl_ipc::session::SessionResolver;
use humanitl_ipc::{SandboxService, v1};
use tokio_stream::StreamExt as _;

/// Das Profil, das der Test einhängt: das mitgelieferte `default`, damit die
/// Zeile dieselbe ist, die im Produkt startet.
const PROFILE: &str = "default";

/// Der Befehl in der Sandbox. Er muss nur lange genug leben, dass der Bericht
/// gelesen wird; der Test beendet die Sandbox selbst.
const COMMAND: &[&str] = &["/bin/sleep", "30"];

/// Alles, was eine Sitzung auf der Platte braucht.
struct Fixture {
    _dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    _proxy: UnixListener,
}

impl Fixture {
    /// Legt Profil, Proxy-Socket, CA und Projektverzeichnis an.
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let home = dir.path().join("home");
        let runtime = dir.path().join("runtime");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&runtime).expect("runtime");
        let paths = Paths::new(
            Env::from_process()
                .with("HOME", home.to_string_lossy())
                .with("XDG_RUNTIME_DIR", runtime.to_string_lossy())
                .with("XDG_CONFIG_HOME", home.join(".config").to_string_lossy())
                .with("XDG_DATA_HOME", home.join(".local/share").to_string_lossy()),
        );

        // Das mitgelieferte Profil, an die Stelle kopiert, an der der Dienst
        // zuerst sucht.
        let profiles = paths.profiles_dir().join("sandbox");
        std::fs::create_dir_all(&profiles).expect("profile directory");
        let bundled = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../profiles/sandbox")
            .join(format!("{PROFILE}.toml"));
        std::fs::copy(&bundled, profiles.join(format!("{PROFILE}.toml")))
            .unwrap_or_else(|err| panic!("{} is readable: {err}", bundled.display()));

        // Der Proxy-Socket. Er ist der eine Pfad, der in die Sandbox
        // eingehängt wird, und er muss ein Socket sein, sonst lehnt der
        // Launcher den Start ab.
        std::fs::create_dir_all(paths.proxy_socket_dir()).expect("proxy directory");
        let proxy = UnixListener::bind(paths.proxy_socket()).expect("bind the proxy socket");
        std::fs::set_permissions(paths.proxy_socket(), std::fs::Permissions::from_mode(0o600))
            .expect("chmod the proxy socket");

        // Zertifikat und Bundle. Der Inhalt ist gleichgültig; der Launcher
        // prüft, dass es reguläre Dateien sind.
        let ca = paths.ca_dir();
        std::fs::create_dir_all(&ca).expect("ca directory");
        std::fs::write(paths.ca_cert_path(), b"-----BEGIN CERTIFICATE-----\n").expect("ca");
        std::fs::write(ca.join("ca-bundle.crt"), b"-----BEGIN CERTIFICATE-----\n")
            .expect("ca bundle");

        let work = home.join("project");
        std::fs::create_dir_all(work.join(".git/hooks")).expect("work");
        std::fs::write(work.join(".git/config"), "[user]\n\tname = canary\n").expect("git config");
        std::fs::write(work.join(".envrc"), "export CANARY=1\n").expect("envrc");

        Self {
            _dir: dir,
            paths,
            work,
            _proxy: proxy,
        }
    }

    /// Der Dienst über dieser Sitzung.
    fn service(&self) -> SandboxService {
        SandboxService::new(
            SessionResolver::for_config(self.paths.clone(), Config::default()),
            SessionId::new(),
            SandboxPorts::none(),
        )
    }

    /// Die Anfrage, die diese Sandbox startet.
    fn start(&self) -> v1::SandboxRequest {
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
                profile: PROFILE.to_owned(),
                work_dir: self.work.display().to_string(),
                work_mode: "rw".to_owned(),
                command: COMMAND.iter().map(|arg| (*arg).to_owned()).collect(),
                session_profile: String::new(),
                ask_mode: String::new(),
                cli_overrides: Vec::new(),
            })),
        }
    }
}

/// Ob dieser Rechner den Test tragen kann; sonst die Begründung auf stderr.
fn usable(fixture: &Fixture) -> bool {
    if let Err(diagnostic) = humanitl_sandbox::BwrapBackend::detect(fixture.paths.clone()) {
        eprintln!("skipping: bwrap is not usable here: {}", diagnostic.why);
        return false;
    }
    // Der Shim liegt neben dem Testbinary oder ein Verzeichnis darüber; ohne
    // ihn prüfte der Test einen Start, den es nie gab.
    let found = std::env::current_exe().ok().is_some_and(|exe| {
        exe.parent().is_some_and(|dir| {
            dir.join("humanitl-shim").is_file()
                || dir
                    .parent()
                    .is_some_and(|up| up.join("humanitl-shim").is_file())
        })
    });
    if !found {
        eprintln!("skipping: humanitl-shim is not built next to the test binary");
    }
    found
}

/// Die Ereignisse eines Sandbox-Aufrufs, bis der Zustand steht.
///
/// Der Strom eines Starts endet seit HUM-067 erst, wenn der Agent sich
/// beendet: Er trägt auch dessen Ausgabe und seinen Exit-Code. Ein Test, der
/// auf das Ende wartete, wartete so lange wie der Befehl in der Sandbox.
/// Gelesen wird deshalb bis zum ersten Zustand, der steht.
async fn events(service: &SandboxService, request: v1::SandboxRequest) -> Vec<v1::SandboxEvent> {
    let mut stream = service.stream(request);
    let mut seen = Vec::new();
    while let Some(event) = stream.next().await {
        let settled = matches!(
            &event.event,
            Some(v1::sandbox_event::Event::Status(status))
                if status.state == v1::SandboxState::Running as i32
                    || status.state == v1::SandboxState::Failed as i32
                    || status.state == v1::SandboxState::Stopped as i32
        );
        seen.push(event);
        if settled {
            break;
        }
    }
    seen
}

/// Die Ereignisse eines schon geöffneten Stroms, bis der Zustand steht.
async fn settled_from(
    mut stream: impl tokio_stream::Stream<Item = v1::SandboxEvent> + Unpin,
) -> Vec<v1::SandboxEvent> {
    let mut seen = Vec::new();
    while let Some(event) = stream.next().await {
        let settled = matches!(
            &event.event,
            Some(v1::sandbox_event::Event::Status(status))
                if status.state == v1::SandboxState::Running as i32
                    || status.state == v1::SandboxState::Failed as i32
                    || status.state == v1::SandboxState::Stopped as i32
        );
        seen.push(event);
        if settled {
            break;
        }
    }
    seen
}

/// Ob dieser Strom einen Befund mit diesem Code trägt.
fn carries(events: &[v1::SandboxEvent], code: &str) -> bool {
    diagnostics(events)
        .iter()
        .any(|diagnostic| diagnostic.code == code)
}

/// Alle Ereignisse eines Aufrufs, bis der Strom selbst endet.
async fn events_to_end(
    service: &SandboxService,
    request: v1::SandboxRequest,
) -> Vec<v1::SandboxEvent> {
    let mut stream = service.stream(request);
    let mut seen = Vec::new();
    while let Some(event) = stream.next().await {
        seen.push(event);
    }
    seen
}

/// Die Zustände des Stroms, in ihrer Reihenfolge.
fn states(events: &[v1::SandboxEvent]) -> Vec<v1::SandboxState> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Some(v1::sandbox_event::Event::Status(status)) => {
                Some(v1::SandboxState::try_from(status.state).unwrap_or_default())
            }
            _ => None,
        })
        .collect()
}

/// Die Prüfergebnisse des Stroms.
fn checks(events: &[v1::SandboxEvent]) -> Vec<&v1::CheckResult> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Some(v1::sandbox_event::Event::Check(check)) => Some(check),
            _ => None,
        })
        .collect()
}

/// Die Befunde des Stroms.
fn diagnostics(events: &[v1::SandboxEvent]) -> Vec<&v1::Diagnostic> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Some(v1::sandbox_event::Event::Diagnostic(diagnostic)) => Some(diagnostic),
            _ => None,
        })
        .collect()
}

/// Ein sauberes Projektverzeichnis: drei belegte Garantien, dann `running`.
#[tokio::test(flavor = "multi_thread")]
async fn a_clean_start_reports_three_measured_guarantees_before_running() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let service = fixture.service();
    let seen = events(&service, fixture.start()).await;

    let measured = checks(&seen);
    assert_eq!(
        measured.len(),
        3,
        "three guarantees, one event each: {seen:?}"
    );
    for check in &measured {
        assert!(
            check.passed,
            "a clean sandbox proves every guarantee: {check:?}"
        );
        assert!(!check.evidence.is_empty(), "a result without evidence");
        assert!(check.diagnostic.is_none());
    }
    assert_eq!(
        measured.iter().map(|check| check.check).collect::<Vec<_>>(),
        vec![
            v1::IsolationCheck::NoNetworkInterface as i32,
            v1::IsolationCheck::SingleSocket as i32,
            v1::IsolationCheck::SeccompActive as i32,
        ]
    );
    assert_eq!(
        states(&seen),
        vec![v1::SandboxState::Starting, v1::SandboxState::Running],
        "the results stand between starting and running"
    );

    // Die Ergebnisse kommen vor `running`: Wer den Zustand sieht, hat sie
    // gesehen.
    let running_at = seen
        .iter()
        .position(|event| {
            matches!(&event.event, Some(v1::sandbox_event::Event::Status(status))
                if status.state == v1::SandboxState::Running as i32)
        })
        .expect("a running status");
    let last_check_at = seen
        .iter()
        .rposition(|event| matches!(event.event, Some(v1::sandbox_event::Event::Check(_))))
        .expect("a check event");
    assert!(last_check_at < running_at, "{seen:?}");

    let _ = events(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Stop(())),
        },
    )
    .await;
}

/// Eine echte zweite Socket-Datei im Projektverzeichnis: `SANDBOX_015`, die
/// Sandbox wird beendet, und `running` steht in keinem Ereignis.
///
/// Das ist das Akzeptanzkriterium des Issues, mit dem Gegenstand, den es
/// nennt — nicht mit einem Shim, der `single_socket fail` behauptet.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_socket_in_the_project_stops_the_start() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let _second = UnixListener::bind(fixture.work.join("agent.sock"))
        .expect("bind a second socket in the project directory");

    let service = fixture.service();
    let seen = events(&service, fixture.start()).await;

    let measured = checks(&seen);
    assert_eq!(measured.len(), 3, "{seen:?}");
    let socket = measured
        .iter()
        .find(|check| check.check == v1::IsolationCheck::SingleSocket as i32)
        .expect("the second guarantee is reported");
    assert!(!socket.passed, "a second door is not one door: {socket:?}");
    assert!(
        socket.evidence.contains("agent.sock"),
        "the evidence names the file that must not be there: {}",
        socket.evidence
    );
    assert_eq!(
        socket.diagnostic.as_ref().map(|d| d.code.as_str()),
        Some("SANDBOX_015")
    );

    assert!(
        diagnostics(&seen)
            .iter()
            .any(|diagnostic| diagnostic.code == "SANDBOX_015"),
        "the finding travels as its own event: {seen:?}"
    );
    assert!(
        !states(&seen).contains(&v1::SandboxState::Running),
        "a sandbox whose isolation is not proven never reports running: {seen:?}"
    );
    assert_eq!(
        states(&seen).last(),
        Some(&v1::SandboxState::Failed),
        "{seen:?}"
    );

    // Und sie läuft nicht mehr: ein zweiter Start ist wieder ein Start und
    // nicht die Momentaufnahme einer laufenden Sandbox.
    let after = events(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::IsolationCheck(())),
        },
    )
    .await;
    assert!(
        checks(&after).is_empty(),
        "nothing runs, so nothing is measured: {after:?}"
    );
}

/// Die Ausgabe des Agenten erreicht den Client, gefiltert, und sein
/// Exit-Code steht als eigenes Ereignis dahinter.
///
/// Das ist der Weg, den `humanitl run` nimmt: kein PTY, kein Raw-Modus, nur
/// die Bytes und die Zahl am Ende (HUM-067). Der Befehl schreibt neben seiner
/// Zeile eine OSC-52-Folge — die Folge, mit der ein Terminal in die
/// Zwischenablage des Menschen schreibt. Sie darf den Daemon nicht verlassen;
/// das ist einer der fünf erklärten Seitenkanäle (BACKLOG.md 4.2).
#[tokio::test(flavor = "multi_thread")]
async fn the_output_travels_filtered_and_the_exit_code_behind_it() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let service = fixture.service();
    let mut start = fixture.start();
    if let Some(v1::sandbox_request::Op::Start(inner)) = start.op.as_mut() {
        inner.command = vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            // `printf` schreibt die Folgen wörtlich; `echo -e` ist nicht
            // portabel. `\033` ist `ESC`, `\235` ist `OSC` als ein Byte, und
            // `\007` ist `BEL`. Der Weg über die Oktalzahl ist nötig, weil
            // `Start.command` eine Zeichenkette der Leitung ist und damit
            // gültiges UTF-8 sein muss: `0x9d` allein ist keines.
            "printf 'hello\\n'; \
             printf '\\033]52;c;c2VjcmV0\\007'; \
             printf '\\235052;c;c2VjcmV0\\007'; \
             printf '\\302\\2352;c;eA==\\007'; \
             printf '\\302\\2331A'; \
             printf '\\033[1A\\033[2K'; \
             printf '\\033]0;All Checks Passed\\007'; \
             printf '\\033[32mbye\\033[0m\\n'; \
             exit 7"
                .to_owned(),
        ];
    }
    let seen = events_to_end(&service, start).await;

    let stdout: Vec<u8> = seen
        .iter()
        .filter_map(|event| match &event.event {
            Some(v1::sandbox_event::Event::Output(chunk))
                if chunk.stream == v1::OutputStream::Stdout as i32 =>
            {
                Some(chunk.data.clone())
            }
            _ => None,
        })
        .flatten()
        .collect();
    let text = String::from_utf8_lossy(&stdout);
    assert!(text.contains("hello"), "the agent wrote a line: {text:?}");
    assert!(
        text.contains("bye"),
        "and the lines behind the sequences: {text:?}"
    );
    assert!(
        !text.contains("c2VjcmV0"),
        "the payload of the clipboard sequence stays inside: {text:?}"
    );
    assert!(
        !text.contains("All Checks Passed"),
        "and so does the window title the agent wanted to set: {text:?}"
    );
    assert!(
        !stdout.contains(&0x9d),
        "the one-byte OSC introducer never leaves: {stdout:?}"
    );
    // Farbe darf hinaus, Bewegung und Löschen nicht. Die eine Folge, die
    // bleibt, ist SGR.
    assert!(
        text.contains("\u{1b}[32m"),
        "colour is the one sequence that passes: {text:?}"
    );
    for forbidden in ["\u{1b}[1A", "\u{1b}[2K", "\u{1b}]"] {
        assert!(
            !text.contains(forbidden),
            "{forbidden:?} must not reach the terminal: {text:?}"
        );
    }

    let exit = seen
        .iter()
        .find_map(|event| match &event.event {
            Some(v1::sandbox_event::Event::Exit(exit)) => Some(exit.code),
            _ => None,
        })
        .expect("the agent reports its exit code");
    assert_eq!(exit, 7, "the code of the agent, unchanged: {seen:?}");
}

/// Zwei gleichzeitige `Start`: genau einer bekommt die Sitzung.
///
/// `self.running` wird erst gesetzt, wenn `bwrap` steht. Zwischen der Frage
/// „läuft schon eine?" und dieser Zuweisung liegen die Auflösung der Sitzung
/// und der Start selbst — lange genug, dass zwei Aufrufe beide daran
/// vorbeikämen, beide starteten und der zweite den ersten aus `running`
/// verdrängte. Der erste Prozess liefe dann weiter, ohne dass ihn noch jemand
/// beenden könnte.
///
/// Der Test öffnet beide Ströme, bevor er einen davon liest; die Aufgaben
/// laufen also wirklich nebeneinander. Der Befehl in der Sandbox lebt lange
/// genug, dass der Gewinner den Anspruch während des ganzen Tests hält.
#[tokio::test(flavor = "multi_thread")]
async fn only_one_of_two_concurrent_starts_gets_the_session() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let service = fixture.service();

    let first = service.stream(fixture.start());
    let second = service.stream(fixture.start());
    let (seen_first, seen_second) = tokio::join!(settled_from(first), settled_from(second));

    let refused = usize::from(carries(&seen_first, "CLI_005"))
        + usize::from(carries(&seen_second, "CLI_005"));
    assert_eq!(
        refused, 1,
        "exactly one of the two starts is refused: {seen_first:?} / {seen_second:?}"
    );

    let started = usize::from(states(&seen_first).contains(&v1::SandboxState::Running))
        + usize::from(states(&seen_second).contains(&v1::SandboxState::Running));
    assert_eq!(
        started, 1,
        "and exactly one sandbox reports running: {seen_first:?} / {seen_second:?}"
    );

    let _ = events(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Stop(())),
        },
    )
    .await;
}
