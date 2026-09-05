//! Die Sandbox an einem Pseudoterminal (HUM-042).
//!
//! Drei Fragen, und alle drei lassen sich nur an einer echten Sandbox
//! beantworten: Bekommt der Agent ein Terminal? Erreicht ihn eine neue
//! Geometrie? Und erklärt sich ein Start, der scheitert, auch dann noch, wenn
//! es keine getrennte Fehlerausgabe mehr gibt?
//!
//! Wie in `launcher.rs` brauchen sie `bwrap` und einen Kernel mit
//! unprivilegierten Nutzer-Namensräumen; fehlt eines, sagen sie es auf stderr
//! und enden grün. „Kein `bwrap` auf dieser Maschine" ist eine Aussage über
//! die Maschine, nicht über den Launcher. Den Shim ersetzt dasselbe
//! Bash-Skript wie dort.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::ids::SessionId;
use humanitl_sandbox::{
    BwrapBackend, OutputChunk, OutputStream, SandboxBackend, SandboxProfile, SessionContext,
    StdioMode,
};

/// Kein Test wartet länger.
const WAIT: Duration = Duration::from_secs(60);

/// Die Geometrie, mit der die Sitzung startet.
const START_SIZE: (u16, u16) = (80, 24);

/// Die Geometrie, auf die der Client sie ändert.
const NEW_SIZE: (u16, u16) = (120, 40);

/// Ein Shim-Ersatz, der den Bericht schreibt und dann den Befehl startet.
const FAKE_SHIM: &str = r#"#!/bin/bash
# fake humanitl-shim for the pty tests: report, then exec. No filter.
if [ "$#" -eq 0 ]; then exit 125; fi
if [ "$1" = "--proxy-port" ]; then shift 2; fi
if [ "$1" = "--" ]; then shift; fi
if [ -n "${HUMANITL_REPORT_FD:-}" ]; then
  fd="$HUMANITL_REPORT_FD"
  echo "CHECK bridge_listening ok fake listener" >&"$fd"
  echo "CHECK single_socket ok sockets=/run/humanitl/proxy.sock;unexpected=none;entries=7;limit=none" >&"$fd"
  echo "CHECK seccomp_applied ok fake" >&"$fd"
  echo "CHECK families ok fake" >&"$fd"
  echo "CHECK no_interfaces ok lo" >&"$fd"
  exec {fd}>&-
fi
exec "$@"
"#;

/// Der Agent dieses Tests: Er meldet sein Terminal und seine Größe, und er
/// meldet sie erneut, sobald der Daemon ihm `SIGWINCH` schickt.
///
/// Die Falle steckt in der Reihenfolge: Der `trap` steht **vor** der ersten
/// Meldung. Der Test wartet auf diese Meldung, bevor er die Größe ändert; ohne
/// diese Reihenfolge gäbe es ein Fenster, in dem das Signal niemanden fände.
const AGENT: &str = r"trap 'stty size; exit 0' WINCH
test -t 0 && test -t 1 && echo TTY-OK
stty size
while :; do sleep 0.05; done
";

struct Fixture {
    _dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    socket: PathBuf,
    _listener: UnixListener,
    ca: PathBuf,
    ca_bundle: PathBuf,
    shim: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let state = dir.path().to_path_buf();
    let runtime = state.join("runtime");
    let paths = Paths::new(Env::from_process().with("XDG_RUNTIME_DIR", runtime.to_string_lossy()));

    let work = state.join("work");
    std::fs::create_dir_all(&work).expect("work");
    let socket = paths.proxy_socket();
    std::fs::create_dir_all(socket.parent().unwrap()).expect("socket dir");
    std::fs::set_permissions(
        socket.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .expect("chmod dir");
    let listener = UnixListener::bind(&socket).expect("bind placeholder socket");
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
        .expect("chmod socket");

    let ca = state.join("ca.crt");
    std::fs::write(&ca, b"-----BEGIN CERTIFICATE-----\n").expect("ca");
    let ca_bundle = state.join("ca-bundle.crt");
    std::fs::write(&ca_bundle, b"-----BEGIN CERTIFICATE-----\n").expect("ca bundle");
    let shim = state.join("humanitl-shim");
    std::fs::write(&shim, FAKE_SHIM).expect("write shim");
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("chmod shim");

    Fixture {
        _dir: dir,
        paths,
        work,
        socket,
        _listener: listener,
        ca,
        ca_bundle,
        shim,
    }
}

impl Fixture {
    fn context(&self, command: &[&str]) -> SessionContext {
        SessionContext {
            session: SessionId::nil(),
            work_src: self.work.clone(),
            work_mode: WorkMode::Rw,
            proxy_socket_src: self.socket.clone(),
            ca_cert_src: self.ca.clone(),
            ca_bundle_src: self.ca_bundle.clone(),
            shim_src: self.shim.clone(),
            session_env: vec![("HUMANITL_SESSION".to_owned(), SessionId::nil().to_string())],
            command: command.iter().map(OsString::from).collect(),
            files: Vec::new(),
        }
    }

    /// Das echte `bwrap` an einem Pseudoterminal, oder `None` mit Begründung.
    fn at_a_pty(&self) -> Option<BwrapBackend> {
        match BwrapBackend::detect(self.paths.clone()) {
            Ok(backend) => Some(backend.with_stdio(StdioMode::Pty {
                cols: START_SIZE.0,
                rows: START_SIZE.1,
            })),
            Err(err) => {
                eprintln!("skipping: bwrap is not usable on this machine: {err}");
                None
            }
        }
    }
}

fn profile(name: &str) -> SandboxProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles/sandbox")
        .join(format!("{name}.toml"));
    SandboxProfile::load(&path)
        .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
}

/// Wartet, bis die Ausgabe des Agenten `needle` enthält, höchstens `WAIT`.
///
/// Gibt alles zurück, was bis dahin kam, oder `None`, wenn die Frist um ist
/// oder der Strom endet.
fn wait_for(rx: &mpsc::Receiver<OutputChunk>, seen: &mut Vec<u8>, needle: &str) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        if String::from_utf8_lossy(seen).contains(needle) {
            return true;
        }
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            return false;
        };
        match rx.recv_timeout(left.min(Duration::from_millis(250))) {
            Ok(chunk) => {
                // Ein Terminal hat genau einen Strom; eine getrennte
                // Fehlerausgabe gibt es nicht mehr.
                assert_eq!(chunk.stream, OutputStream::Stdout, "one stream, not two");
                seen.extend_from_slice(&chunk.bytes);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return String::from_utf8_lossy(seen).contains(needle);
            }
        }
    }
}

/// Ob `stty` auf dem Host liegt; die Sandbox bindet `/usr` und sieht damit
/// dasselbe Programm.
fn has_stty() -> bool {
    ["/usr/bin/stty", "/bin/stty"]
        .iter()
        .any(|path| Path::new(path).exists())
}

/// Der Agent bekommt ein Terminal, und eine neue Geometrie erreicht ihn.
///
/// Das ist das Akzeptanzkriterium „Fenster-Resize im UI ändert die Spaltenzahl
/// im Agenten": `tcsetwinsize` allein genügt nicht. Die Sandbox läuft mit
/// `--new-session` und hat kein steuerndes Terminal, also schickt der Kernel
/// kein `SIGWINCH`; das tut `SandboxHandle::resize` selbst, an die
/// Prozessgruppe des Sandbox-Init.
#[test]
fn a_resize_reaches_the_agent() {
    let fx = fixture();
    if !has_stty() {
        eprintln!("skipping: no stty on this machine, so the agent cannot report its size");
        return;
    }
    let Some(backend) = fx.at_a_pty() else { return };
    let (tx, rx) = mpsc::channel();
    let backend = backend.with_output_sink(tx);
    let plan = backend
        .plan(&profile("default"), &fx.context(&["/bin/sh", "-c", AGENT]))
        .expect("the plan builds");
    let handle = backend.launch(&plan).expect("bwrap starts");

    let mut seen = Vec::new();
    assert!(
        wait_for(&rx, &mut seen, "TTY-OK"),
        "the agent stands at a terminal: {:?}",
        String::from_utf8_lossy(&seen)
    );
    let first = format!("{} {}", START_SIZE.1, START_SIZE.0);
    assert!(
        wait_for(&rx, &mut seen, &first),
        "and it starts with the size it was given ({first}): {:?}",
        String::from_utf8_lossy(&seen)
    );

    handle
        .resize(NEW_SIZE.0, NEW_SIZE.1)
        .expect("the terminal takes the new size");
    let second = format!("{} {}", NEW_SIZE.1, NEW_SIZE.0);
    let reached = wait_for(&rx, &mut seen, &second);
    let text = String::from_utf8_lossy(&seen).into_owned();
    handle.kill();
    assert!(
        reached,
        "the agent sees the new size ({second}) after SIGWINCH: {text:?}"
    );
}

/// Ohne Pseudoterminal gibt es nichts zu ändern, und der Befund sagt das.
#[test]
fn a_sandbox_without_a_terminal_says_so() {
    let fx = fixture();
    let Some(backend) = fx.at_a_pty() else { return };
    let backend = backend.with_stdio(StdioMode::Capture);
    let plan = backend
        .plan(
            &profile("default"),
            &fx.context(&["/bin/sh", "-c", "exit 0"]),
        )
        .expect("the plan builds");
    let handle = backend.launch(&plan).expect("bwrap starts");
    assert!(handle.pty_master().is_none());

    let err = handle
        .resize(NEW_SIZE.0, NEW_SIZE.1)
        .expect_err("no terminal");
    assert_eq!(err.code.as_str(), "TERM_002", "{err}");
    assert!(err.why.contains("without a terminal"), "{}", err.why);
    let err = handle.write_input(b"x").expect_err("no terminal");
    assert_eq!(err.code.as_str(), "TERM_002", "{err}");
    let _ = handle.wait_timeout(WAIT);
}

/// Ein Start, der scheitert, erklärt sich auch am Pseudoterminal.
///
/// Ohne die Spiegelung der ersten Bytes in die Fehlerausgabe stünde hier ein
/// `SANDBOX_012` ohne Grund: `bwrap` schreibt seine Meldung in dieselbe
/// Leitung wie alles andere, und `Shared::verdict` liest nur die
/// Fehlerausgabe. `is_userns_failure` verlöre damit ebenfalls seine Quelle,
/// und aus `SANDBOX_003` würde ein `SANDBOX_012` ohne Behebungsvorschlag.
#[test]
fn a_failing_start_still_explains_itself_at_a_pty() {
    let fx = fixture();
    let Some(backend) = fx.at_a_pty() else { return };
    let broken = SandboxProfile::parse(
        "version = 1\nname = \"probe\"\n[mounts]\nro = [\"/nonexistent-humanitl-source\"]\n",
        Path::new("<probe>"),
    )
    .expect("the probe profile parses");
    let plan = backend
        .plan(&broken, &fx.context(&["true"]))
        .expect("the policy has nothing against a missing directory");
    // Innerhalb des Fensters meldet `launch` den Befund, auf einer langsamen
    // Maschine erst `wait`; beide sagen dasselbe.
    let err = match backend.launch(&plan) {
        Err(err) => err,
        Ok(handle) => handle
            .wait_timeout(WAIT)
            .expect("the sandbox ends within the timeout")
            .expect_err("bwrap cannot bind a missing source"),
    };
    assert_eq!(err.code.as_str(), "SANDBOX_012", "{err}");
    assert!(
        err.why.contains("before starting the command"),
        "{}",
        err.why
    );
    assert!(
        err.why.contains("nonexistent-humanitl-source"),
        "the message of bwrap came out of the terminal: {}",
        err.why
    );
    assert!(
        !err.why.contains("inherited stderr"),
        "a terminal is not the inherited stderr: {}",
        err.why
    );
}
