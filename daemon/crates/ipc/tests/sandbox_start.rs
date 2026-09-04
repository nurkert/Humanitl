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
        SandboxService::new(Config::default(), self.paths.clone(), SessionId::new())
    }

    /// Die Anfrage, die diese Sandbox startet.
    fn start(&self) -> v1::SandboxRequest {
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
                profile: PROFILE.to_owned(),
                work_dir: self.work.display().to_string(),
                work_mode: "rw".to_owned(),
                command: COMMAND.iter().map(|arg| (*arg).to_owned()).collect(),
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

/// Alle Ereignisse eines Sandbox-Aufrufs, in der Reihenfolge des Stroms.
async fn events(service: &SandboxService, request: v1::SandboxRequest) -> Vec<v1::SandboxEvent> {
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
