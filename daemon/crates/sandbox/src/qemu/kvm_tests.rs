//! Der Shim als `init` eines Gastkernels, mit echter KVM (HUM-465).
//!
//! Die Tests starten über [`super::test_vm`] eine micro-VM wie die Endform
//! des Spikes. Ohne `/dev/kvm`, ohne die Pakete der Kernel-Stufe oder ohne
//! den gebauten Shim überspringen sie mit Grund auf stderr (`skipped: ...`);
//! mit `HUMANITL_REQUIRE_KVM=1`, wie im CI-Job `kvm-tests`, scheitern sie
//! stattdessen mit dem Befund.
//!
//! Kernel und Module kommen aus `HUMANITL_TEST_VM_KERNEL` und
//! `HUMANITL_TEST_VM_MODULES`, sonst aus `/boot` und `/usr/lib/modules` des
//! laufenden Kernels (dieselben Vorgaben wie `sandbox.qemu.kernel` und
//! `sandbox.qemu.modules`). `HUMANITL_TEST_VM_CONSOLE=1` schreibt zur
//! Fehlersuche die Ausgabe des Gasts mit; die Endform hat keine Konsole.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use humanitl_config::{Env, QemuConfig};

use super::prerequisites::resolve;
use super::test_vm::{self, DOOR_PORT, Outcome, Spec};
use crate::bridge_env::{CHECK_NAMES, parse_check_line};

/// Setzt der CI-Job `kvm-tests`: Fehlt etwas, scheitert der Test.
const REQUIRE_ENV: &str = "HUMANITL_REQUIRE_KVM";

/// Ob ein fehlender Teil ein Fehlschlag ist.
fn required() -> bool {
    std::env::var_os(REQUIRE_ENV).is_some_and(|value| value == "1")
}

/// Überspringt mit Grund, oder scheitert unter [`REQUIRE_ENV`].
fn unavailable(test: &str, why: &str) {
    assert!(
        !required(),
        "{test} needs KVM and the packages of the kernel level, and {REQUIRE_ENV}=1 requires \
         them: {why}"
    );
    eprintln!("skipped: {test}: {why}; set {REQUIRE_ENV}=1 to make this a failure");
}

/// Der gebaute Shim neben dem Testprogramm.
fn built_shim() -> Option<PathBuf> {
    let mut dir = std::env::current_exe().ok()?;
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    Some(dir.join("humanitl-shim")).filter(|shim| shim.is_file())
}

/// Alles für einen Start, oder `None` nach [`unavailable`].
fn spec(test: &str, command: &str) -> Option<(Spec, tempfile::TempDir)> {
    let config = QemuConfig {
        kernel: std::env::var_os("HUMANITL_TEST_VM_KERNEL").map(PathBuf::from),
        modules: std::env::var_os("HUMANITL_TEST_VM_MODULES").map(PathBuf::from),
        ..QemuConfig::default()
    };
    let resolved = match resolve(&Env::from_process(), &config) {
        Ok(resolved) => resolved,
        Err(d) => {
            unavailable(test, &format!("{}: {}", d.code, d.why));
            return None;
        }
    };
    let Some(shim) = built_shim() else {
        unavailable(
            test,
            "the shim is not built next to the test binary (cargo build -p humanitl-shim)",
        );
        return None;
    };
    let project = tempfile::tempdir().expect("project directory");
    std::fs::write(project.path().join("pin"), "pin\n").expect("pin file");
    let spec = Spec {
        resolved,
        shim,
        project: project.path().to_path_buf(),
        command: vec!["/bin/bash".to_owned(), "-c".to_owned(), command.to_owned()],
        extra_modules: Vec::new(),
        console: std::env::var_os("HUMANITL_TEST_VM_CONSOLE").is_some_and(|value| value == "1"),
    };
    Some((spec, project))
}

/// Alles, was ein Fehlschlag zeigen soll.
fn shown(outcome: &Outcome) -> String {
    format!(
        "report:\n  {}\ndoor:\n  {}\nemulator exit: {:?}\nterminal:\n{}\nhelpers:\n{}\nconsole:\n{}",
        outcome.report.join("\n  "),
        outcome.door.join("\n  "),
        outcome.exit,
        String::from_utf8_lossy(&outcome.terminal),
        outcome.helpers,
        outcome
            .console
            .as_deref()
            .unwrap_or("(off; HUMANITL_TEST_VM_CONSOLE=1 records it)"),
    )
}

/// Der Agent im Gast, nach beiden Filtern: Er liest die Modulsperre, prüft,
/// dass er kein `AF_VSOCK` öffnen kann, und stellt zwei Anfragen an
/// `127.0.0.1:3128`; die zweite trägt die Antwort der ersten, so dass die
/// Tür sie nur sieht, wenn beide Richtungen gingen.
const AGENT: &str = r#"
locked=$(cat /proc/sys/kernel/modules_disabled 2>&1 | tr -c 'a-zA-Z0-9\n' _)
vsock=$(python3 -c '
import socket
try:
    socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    print("open")
except PermissionError:
    print("EPERM")
except OSError as err:
    print("errno%d" % err.errno)
' 2>/dev/null || echo no-python)
exec 3<>/dev/tcp/127.0.0.1/3128 || exit 3
printf 'GET /probe?modules_disabled=%s&vsock=%s HTTP/1.0\r\n\r\n' "$locked" "$vsock" >&3
reply=$(cat <&3)
exec 3<&-
body=${reply##*$'\r\n\r\n'}
exec 4<>/dev/tcp/127.0.0.1/3128 || exit 4
printf 'GET /ack?reply=%s HTTP/1.0\r\n\r\n' "$body" >&4
cat <&4 >/dev/null
"#;

/// HUM-465, Integration: der Shim als `init`; die Tür antwortet auf eine
/// Anfrage nach dem Einsetzen der Filter, die Module sind gesperrt, der
/// Agent hat kein `AF_VSOCK`, und jede Prüfung des Shims ist grün.
#[test]
fn the_shim_as_init_locks_modules_and_the_door_answers_after_the_filter() {
    let test = "the_shim_as_init_locks_modules_and_the_door_answers_after_the_filter";
    let Some((spec, _project)) = spec(test, AGENT) else {
        return;
    };
    let outcome = test_vm::run(&spec).unwrap_or_else(|why| panic!("the guest did not run: {why}"));
    let shown = shown(&outcome);

    // Die Prüfungen des Shims, unverändert dieselben wie unter bwrap, dazu
    // die Mounttabelle mit `/work` über Tag und Inode.
    let checks: Vec<_> = outcome
        .report
        .iter()
        .filter_map(|line| parse_check_line(line))
        .collect();
    for name in CHECK_NAMES.iter().chain(&["mounts_match", "mounts_pinned"]) {
        let check = checks
            .iter()
            .find(|check| check.name == *name)
            .unwrap_or_else(|| panic!("no CHECK {name}\n{shown}"));
        assert!(check.ok, "CHECK {name} failed: {}\n{shown}", check.evidence);
    }
    let evidence = |name: &str| {
        checks
            .iter()
            .find(|check| check.name == name)
            .map(|check| check.evidence.clone())
            .unwrap_or_default()
    };
    assert!(
        evidence("bridge_listening").contains(&format!("->vsock:2:{DOOR_PORT}")),
        "the bridge does not lead to the door over vsock\n{shown}"
    );
    assert!(
        evidence("families").contains("socket(AF_VSOCK,SOCK_STREAM)=EPERM"),
        "the agent's filter does not refuse AF_VSOCK\n{shown}"
    );

    // Module geladen und gesperrt, bevor der Bericht begann.
    let modules = outcome
        .report
        .iter()
        .find(|line| line.starts_with("MODULES "))
        .unwrap_or_else(|| panic!("no MODULES line\n{shown}"));
    for name in [
        "vsock",
        "vmw_vsock_virtio_transport_common",
        "vmw_vsock_virtio_transport",
    ] {
        assert!(
            modules.contains(&format!("{name}=")),
            "{name} missing\n{shown}"
        );
    }
    assert!(modules.ends_with("modules_disabled=1"), "{shown}");

    // Die Tür: zwei Anfragen des Agenten, nach beiden Filtern gestellt. Die
    // erste sagt, was der Agent sah: die Sperre gesetzt (eine Probe „Modul
    // nach dem Start laden" scheitert für jeden Prozess, auch Root), kein
    // vsock. Die zweite trägt die Antwort der ersten zurück.
    assert_eq!(
        outcome.door.len(),
        2,
        "the agent's requests did not both reach the door\n{shown}"
    );
    assert_eq!(
        outcome.door[0], "GET /probe?modules_disabled=1&vsock=EPERM HTTP/1.0",
        "{shown}"
    );
    assert_eq!(
        outcome.door[1], "GET /ack?reply=door-ok-1 HTTP/1.0",
        "{shown}"
    );
    assert!(outcome.exit.is_some(), "the guest did not end\n{shown}");
}

/// HUM-465: Scheitert ein Modul, schreibt der Shim eine `CHECK`-Zeile, der
/// Agent startet nie, und der Gast geht aus.
#[test]
fn a_module_that_cannot_load_powers_the_guest_off_before_the_agent() {
    let test = "a_module_that_cannot_load_powers_the_guest_off_before_the_agent";
    let Some((mut spec, _project)) = spec(test, AGENT) else {
        return;
    };
    spec.extra_modules = vec!["hum465_no_such_module".to_owned()];
    let outcome = test_vm::run(&spec).unwrap_or_else(|why| panic!("the guest did not run: {why}"));
    let shown = shown(&outcome);
    let failed: Vec<_> = outcome
        .report
        .iter()
        .filter_map(|line| parse_check_line(line))
        .collect();
    assert_eq!(failed.len(), 1, "{shown}");
    assert_eq!(failed[0].name, "seccomp_applied", "{shown}");
    assert!(!failed[0].ok, "{shown}");
    assert_eq!(
        failed[0].evidence, "modules:not-found:hum465_no_such_module",
        "{shown}"
    );
    assert!(outcome.door.is_empty(), "the agent ran\n{shown}");
    assert!(
        outcome.exit.is_some(),
        "the guest did not power off\n{shown}"
    );
}

/// HUM-465: Laden die vsock-Module nicht (hier: ein Modulverzeichnis ohne
/// Module), hat der Shim keinen Weg zum Host und schreibt nichts; der Gast
/// geht aus. Der Startweg macht daraus eine gescheiterte Prüfung, nie ein
/// stilles Nichts.
#[test]
fn a_guest_without_its_vsock_modules_ends_as_a_failed_check() {
    let test = "a_guest_without_its_vsock_modules_ends_as_a_failed_check";
    let Some((mut spec, _project)) = spec(test, AGENT) else {
        return;
    };
    let empty = tempfile::tempdir().expect("empty module directory");
    spec.resolved.modules = empty.path().to_path_buf();
    let outcome = test_vm::run(&spec).unwrap_or_else(|why| panic!("the guest did not run: {why}"));
    let shown = shown(&outcome);
    assert_eq!(outcome.report, [test_vm::NO_REPORT], "{shown}");
    let check = parse_check_line(&outcome.report[0]).expect("a CHECK line");
    assert!(!check.ok, "{shown}");
    assert!(check.evidence.starts_with("no-report:"), "{shown}");
    assert!(outcome.door.is_empty(), "the agent ran\n{shown}");
    assert!(
        outcome.exit.is_some(),
        "the guest did not power off\n{shown}"
    );
}

/// Der Agent des Terminal-Tests: Er meldet sich, liest eine Zeile, nennt sie
/// mit der Größe seines Terminals und endet auf die nächste.
const TERMINAL_AGENT: &str = r#"
test -t 0 && test -t 1 && echo TTY-OK
echo READY
read -r line
echo "GOT $line SIZE $(stty size)"
read -r _
exit 0
"#;

/// HUM-443, Integration: Im Gast öffnet der Shim das Terminal, der Host
/// verbindet sich über das vsock-Gerät mit dem einen Port des Shims, schickt
/// Größe und Eingabe und liest die Ausgabe; eine weitere Shell öffnet sich
/// über denselben Kanal und meldet ihren Exit-Code.
#[test]
fn the_host_drives_the_guests_terminal_and_a_second_shell_over_the_stream() {
    let test = "the_host_drives_the_guests_terminal_and_a_second_shell_over_the_stream";
    let Some((spec, _project)) = spec(test, TERMINAL_AGENT) else {
        return;
    };
    let wait = std::time::Duration::from_secs(30);
    let mut steps: Vec<String> = Vec::new();
    let outcome = test_vm::run_with(&spec, |session| {
        if !session.wait_for("READY", wait) {
            steps.push(format!("no READY: {:?}", session.text()));
            return;
        }
        session.door.agent_size(100, 30).expect("the size goes out");
        session
            .door
            .agent_input(b"hello\n")
            .expect("the input goes out");
        if !session.wait_for("GOT hello SIZE 30 100", wait) {
            steps.push(format!("no echo with the new size: {:?}", session.text()));
        }
        match session.door.open(
            &[
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "test -t 0 && echo SHELL-TTY; exit 3".to_owned(),
            ],
            80,
            24,
        ) {
            Ok(shell) => {
                let mut output = Vec::new();
                let code = shell.pump(|bytes| output.extend_from_slice(bytes));
                let output = String::from_utf8_lossy(&output).into_owned();
                if code != 3 || !output.contains("SHELL-TTY") {
                    steps.push(format!("the shell ended with {code}: {output:?}"));
                }
            }
            Err(err) => steps.push(format!("no second shell: {}", err.why)),
        }
        session
            .door
            .agent_input(b"bye\n")
            .expect("the input goes out");
        let deadline = std::time::Instant::now() + wait;
        while session.door.agent_exit().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if session.door.agent_exit() != Some(0) {
            steps.push(format!(
                "the agent's exit over the stream: {:?}",
                session.door.agent_exit()
            ));
        }
    })
    .unwrap_or_else(|why| panic!("the guest did not run: {why}"));
    let shown = shown(&outcome);
    assert!(steps.is_empty(), "{}\n{shown}", steps.join("\n"));
    let terminal = String::from_utf8_lossy(&outcome.terminal);
    assert!(
        terminal.contains("TTY-OK"),
        "the agent has a terminal\n{shown}"
    );
    let checks: Vec<_> = outcome
        .report
        .iter()
        .filter_map(|line| parse_check_line(line))
        .collect();
    assert!(
        checks.iter().all(|check| check.ok) && !checks.is_empty(),
        "every check of the shim is green\n{shown}"
    );
    assert!(outcome.exit.is_some(), "the guest did not end\n{shown}");
}

/// HUM-443, Review: Das Ende des Steuerkanals ist die Lebensleine im Gast.
/// Der Agent überhört `SIGTERM`; legt der Host den Strom auf, beendet der
/// Shim nach der Frist alles und schaltet den Gast aus.
#[test]
fn the_guest_powers_off_when_the_host_hangs_up_the_stream() {
    let test = "the_guest_powers_off_when_the_host_hangs_up_the_stream";
    let agent = "trap '' TERM; echo READY; while :; do sleep 0.1; done";
    let Some((spec, _project)) = spec(test, agent) else {
        return;
    };
    let mut hung_up = None;
    let outcome = test_vm::run_with(&spec, |session| {
        if session.wait_for("READY", std::time::Duration::from_secs(30)) {
            session.door.hang_up();
            hung_up = Some(std::time::Instant::now());
        }
    })
    .unwrap_or_else(|why| panic!("the guest did not run: {why}"));
    let ended = std::time::Instant::now();
    let shown = shown(&outcome);
    let hung_up = hung_up.unwrap_or_else(|| panic!("the agent never started\n{shown}"));
    assert!(
        outcome.exit.is_some(),
        "the guest did not power off\n{shown}"
    );
    // `KILL_GRACE`, das Nachreichen des Berichts und das Ausschalten.
    let took = ended - hung_up;
    assert!(
        took < crate::KILL_GRACE + std::time::Duration::from_secs(15),
        "the guest outlived its stream by {took:?}\n{shown}"
    );
}

// ---- the adapter (HUM-444) --------------------------------------------------

/// Eine Sitzung über den Adapter: Pfade in einem Ordner, ein Proxy, der auf
/// jede Anfrage `door-ok` antwortet, CA, Projekt und das Backend aus
/// `detect_unadmitted`.
struct Harness {
    dir: tempfile::TempDir,
    backend: crate::Backend,
    project: PathBuf,
    proxy: PathBuf,
}

// The module is test-only already; the attribute also marks the rest of the
// file for `tests/feature_gate.rs`, which looks for it before a use of the
// bypass.
#[cfg(test)]
impl Drop for Harness {
    /// Das Verzeichnis dieses Daemons unter `/run/user/<uid>` geht mit,
    /// wenn keine Sitzung mehr darin liegt.
    fn drop(&mut self) {
        let paths = humanitl_config::Paths::new(Env::from_pairs([(
            "XDG_RUNTIME_DIR",
            self.dir.path().join("xdg").to_string_lossy().into_owned(),
        )]));
        if let Some(instance) = super::instance_dir(&paths) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while std::fs::remove_dir(&instance).is_err()
                && instance.exists()
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

impl Harness {
    fn new(test: &str) -> Option<Self> {
        if built_shim().is_none() {
            unavailable(test, "the shim is not built next to the test binary");
            return None;
        }
        let dir = tempfile::Builder::new()
            .prefix("h444-")
            .tempdir_in("/tmp")
            .expect("tempdir");
        let home = dir.path().join("home");
        let xdg = dir.path().join("xdg");
        for path in [&home, &xdg] {
            std::fs::create_dir_all(path).unwrap();
        }
        let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned());
        let paths = humanitl_config::Paths::new(Env::from_pairs([
            ("HOME", home.to_string_lossy().into_owned()),
            ("XDG_RUNTIME_DIR", xdg.to_string_lossy().into_owned()),
            ("PATH", path),
        ]));
        let sandbox = humanitl_config::SandboxRef {
            backend: humanitl_config::SandboxBackendName::Qemu,
            qemu: QemuConfig {
                kernel: std::env::var_os("HUMANITL_TEST_VM_KERNEL").map(PathBuf::from),
                modules: std::env::var_os("HUMANITL_TEST_VM_MODULES").map(PathBuf::from),
                memory_mib: Some(512),
                cpus: Some(1),
                ..QemuConfig::default()
            },
            ..humanitl_config::SandboxRef::default()
        };
        let backend =
            match crate::Backend::detect_unadmitted(crate::VM_BACKEND, paths.clone(), &sandbox) {
                Ok(backend) => backend,
                Err(d) => {
                    unavailable(test, &format!("{}: {}", d.code, d.why));
                    return None;
                }
            };
        let proxy = paths.proxy_socket();
        std::fs::create_dir_all(proxy.parent().unwrap()).unwrap();
        answer_proxy(&proxy);
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("pin"), "pin\n").unwrap();
        std::fs::write(project.join(".env"), "SECRET=yes\n").unwrap();
        std::fs::write(dir.path().join("ca.crt"), "ca\n").unwrap();
        Some(Self {
            dir,
            backend,
            project,
            proxy,
        })
    }

    fn session(&self, command: &str) -> crate::SessionContext {
        crate::SessionContext {
            session: humanitl_core::ids::SessionId::new(),
            work_src: self.project.clone(),
            work_mode: humanitl_config::WorkMode::Rw,
            proxy_socket_src: self.proxy.clone(),
            ca_cert_src: self.dir.path().join("ca.crt"),
            ca_bundle_src: self.dir.path().join("ca.crt"),
            shim_src: built_shim().unwrap(),
            session_env: vec![("HUMANITL_SESSION".to_owned(), "kvm".to_owned())],
            command: vec![
                std::ffi::OsString::from("/bin/bash"),
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from(command),
            ],
            files: Vec::new(),
            agent_program: None,
        }
    }

    /// Plan und Start mit Terminal im Strom-Modus; die Ausgabe des Agenten
    /// kommt über den Empfänger.
    fn launch(
        &self,
        command: &str,
    ) -> (
        crate::SandboxHandle,
        std::sync::mpsc::Receiver<crate::OutputChunk>,
    ) {
        use crate::SandboxBackend as _;
        let (plan, output) = self.plan_with(command);
        let handle = self
            .backend
            .launch(&plan)
            .unwrap_or_else(|d| panic!("launch: {}: {}", d.code, d.why));
        (handle, output)
    }

    /// Der Plan einer Sitzung mit Terminal im Strom-Modus; die Ausgabe des
    /// Agenten kommt über den Empfänger.
    fn plan_with(
        &self,
        command: &str,
    ) -> (
        crate::LaunchPlan,
        std::sync::mpsc::Receiver<crate::OutputChunk>,
    ) {
        use crate::SandboxBackend as _;
        let profile = crate::SandboxProfile::parse(
            include_str!("../../../../../profiles/sandbox/default.toml"),
            std::path::Path::new("default.toml"),
        )
        .unwrap();
        let session = self.session(command);
        let (sink, output) = std::sync::mpsc::channel();
        let options = crate::LaunchOptions {
            stdio: crate::StdioMode::Pty { cols: 80, rows: 24 },
            terminal: crate::TerminalTransport::Stream,
            output_sink: Some(sink),
            report_timeout: Some(Duration::from_secs(60)),
            ..crate::LaunchOptions::default()
        };
        let plan = self
            .backend
            .plan(&crate::PlanRequest::new(&profile, &session, &options))
            .unwrap_or_else(|d| panic!("plan: {}: {}", d.code, d.why));
        (plan, output)
    }

    /// Wie [`Harness::launch`], aber der Plan geht erst an
    /// `before_launch`, etwa um den Gastplan zu ändern.
    fn launch_planned(
        &self,
        command: &str,
        before_launch: impl FnOnce(&crate::LaunchPlan),
    ) -> (
        crate::SandboxHandle,
        std::sync::mpsc::Receiver<crate::OutputChunk>,
    ) {
        use crate::SandboxBackend as _;
        let (plan, output) = self.plan_with(command);
        before_launch(&plan);
        let handle = self
            .backend
            .launch(&plan)
            .unwrap_or_else(|d| panic!("launch: {}: {}", d.code, d.why));
        (handle, output)
    }
}

/// Ein Proxy, der jede Anfrage mit `door-ok` beantwortet.
fn answer_proxy(path: &std::path::Path) {
    use std::io::{BufRead as _, Write as _};
    use std::os::unix::fs::PermissionsExt as _;
    let listener = std::os::unix::net::UnixListener::bind(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if line.trim().is_empty() => break,
                        Ok(_) => {}
                    }
                }
                let mut stream = stream;
                let _ = stream.write_all(
                    b"HTTP/1.0 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ndoor-ok",
                );
            });
        }
    });
}

/// Wartet höchstens `timeout`, bis die Ausgabe `needle` enthält; gibt sie
/// zurück.
fn wait_output(
    output: &std::sync::mpsc::Receiver<crate::OutputChunk>,
    seen: &mut Vec<u8>,
    needle: &str,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if String::from_utf8_lossy(seen).contains(needle) {
            return true;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        if let Ok(chunk) = output.recv_timeout(left.min(Duration::from_millis(200))) {
            seen.extend_from_slice(&chunk.bytes);
        }
    }
}

/// Der Agent der Adapter-Tests: zeigt, dass er läuft, liest eine Zeile vom
/// Terminal, fragt den Proxy über die Tür und sieht nach, ob die Maske über
/// `.env` hält.
const ADAPTER_AGENT: &str = r#"touch /work/started
echo ready-$((40+2))
read -r line
echo "got:$line"
echo "env:[$(cat /work/.env)]"
exec 3<>/dev/tcp/127.0.0.1/3128 || exit 3
printf 'GET /probe HTTP/1.0\r\n\r\n' >&3
cat <&3
echo
"#;

/// HUM-444, Integration: Eine Sitzung startet über `detect_unadmitted`, die
/// Prüfung (Gast und Host) ist grün, das Terminal geht über den Strom, die
/// Tür erreicht den Proxy, und die Maske über `.env` hält.
#[test]
fn a_session_starts_through_the_adapter_with_a_green_host_check() {
    use crate::SandboxBackend as _;
    let test = "a_session_starts_through_the_adapter_with_a_green_host_check";
    let Some(harness) = Harness::new(test) else {
        return;
    };
    let (handle, output) = harness.launch(ADAPTER_AGENT);
    let mut seen = Vec::new();
    let results = harness.backend.isolation_check(&handle);
    let stderr = || {
        handle
            .output()
            .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
            .unwrap_or_default()
    };
    for result in &results {
        assert!(
            result.passed,
            "{}: {} {:?}\nreport: {:?}",
            result.check.as_str(),
            result.evidence,
            result.diagnostic,
            handle.report().checks
        );
        assert!(
            result.evidence.contains("host: checked"),
            "{}",
            results
                .iter()
                .map(|r| host_part(&r.evidence))
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
    // Eine zweite Prüfung derselben gesunden Sitzung (etwa ein späteres
    // `IsolationCheck`) besteht ebenso, schickt kein zweites Urteil und
    // beendet nichts; der Agent arbeitet unten weiter.
    let again = harness.backend.isolation_check(&handle);
    assert!(
        again.iter().all(|result| result.passed),
        "the second check: {again:?}"
    );
    assert!(
        handle.try_wait().is_none(),
        "the second check ended the session"
    );
    assert!(
        wait_output(&output, &mut seen, "ready-42", Duration::from_secs(30)),
        "{}",
        String::from_utf8_lossy(&seen)
    );
    handle.write_input(b"hello-terminal\r").unwrap();
    let done = wait_output(&output, &mut seen, "door-ok", Duration::from_secs(30));
    let shown = String::from_utf8_lossy(&seen).into_owned();
    let ended = handle.wait_timeout(Duration::from_secs(30));
    assert!(done, "{shown}\n{}", stderr());
    assert!(shown.contains("got:hello-terminal"), "{shown}");
    assert!(
        shown.contains("env:[]"),
        "the mask over .env holds: {shown}"
    );
    assert!(
        harness.project.join("started").exists(),
        "the admitted agent ran and wrote into /work"
    );
    let status = ended.expect("the session ends").expect("a status");
    assert_eq!(status.code(), Some(0), "the agent's exit code: {shown}");
    let dir = handle.unit().map(|stop| stop.runtime.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while dir.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!dir.exists(), "the runtime directory goes with the session");
}

/// HUM-470, `esc-2/unplanned_mount` in der CI: Der Shim lehnt an der
/// Mounttabelle ab und schaltet den Gast ab. Der Test erzwingt die Reihenfolge,
/// die in der CI zufällig kam: Die Host-Prüfung läuft erst, wenn die Unit
/// schon fort ist. Die Diagnose des Starts bleibt `SANDBOX_049`, die Unit
/// ohne cgroup steht nur im Befund, und der Agent lief nie. Mutationsbeweis:
/// Überschreibt der Fund des Hosts die Diagnose des Gasts wieder
/// (`host_check::merge`), zeigt der Start `SANDBOX_047`.
#[test]
fn an_unplanned_mount_stays_sandbox_049_when_the_unit_is_gone_before_the_host_check() {
    use std::os::unix::fs::PermissionsExt as _;

    use crate::SandboxBackend as _;
    let test = "an_unplanned_mount_stays_sandbox_049_when_the_unit_is_gone_before_the_host_check";
    let Some(harness) = Harness::new(test) else {
        return;
    };
    let (handle, _output) = harness.launch_planned("touch /work/started; sleep 120", |plan| {
        // Wie `escape-launch --test-unplanned-mount` unter der Kernel-Stufe:
        // Die erwartete Tabelle verliert `/dev/pts`, das der Gast trotzdem
        // einhängt.
        let path = plan
            .session_dir
            .as_ref()
            .unwrap()
            .join("files/guest-plan.json");
        let mut guest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mounts = guest
            .pointer_mut("/pins/mounts")
            .and_then(serde_json::Value::as_array_mut)
            .unwrap();
        let before = mounts.len();
        mounts.retain(|entry| entry["point"] != "/dev/pts");
        assert_eq!(mounts.len() + 1, before, "the guest table names /dev/pts");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&path, guest.to_string()).unwrap();
    });
    // Der Gast lehnt ab und schaltet sich ab; mit ihm endet die Unit.
    assert!(
        handle.wait_timeout(Duration::from_secs(60)).is_some(),
        "the guest did not power off after its refusal"
    );
    // Die erzwungene Reihenfolge: Erst wenn auch der Dienstverwalter die
    // Unit vergessen hat (keine cgroup mehr), prüft der Host. Das Ende von
    // `systemd-run --wait` allein sagt das nicht sicher.
    let stop = handle.unit().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let group = crate::user_unit::run_bounded(
            stop.units.show_command(&stop.unit, "ControlGroup"),
            Duration::from_secs(5),
        )
        .map_or_else(
            |err| panic!("systemctl show: {err}"),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        );
        if group.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the unit still has the cgroup {group} 10s after its end"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let results = harness.backend.isolation_check(&handle);
    let first = results.iter().find(|result| !result.passed).unwrap();
    assert_eq!(
        first.diagnostic.as_ref().map(|d| d.code.as_str()),
        Some("SANDBOX_049"),
        "{results:?}"
    );
    assert!(results.iter().all(|result| !result.passed), "{results:?}");
    assert!(
        results
            .iter()
            .any(|result| result.evidence.contains("host: the unit")),
        "the host check did not see the unit gone, the order was not forced: {results:?}"
    );
    assert!(
        !harness.project.join("started").exists(),
        "the agent ran although the guest refused"
    );
}

/// Mutationsbeweis: ein zusätzlicher Lauscher `<uds>_22` neben der Tür färbt
/// die Host-Prüfung rot, mit `SANDBOX_047`, und die Sitzung endet. Der Agent
/// startet nie: Der Shim wartet auf das Urteil des Daemons, und das ist
/// `DENY` (HUM-444). Ohne das Warten im Shim schriebe er `/work/started`.
#[test]
fn an_extra_listener_beside_the_door_turns_the_host_check_red() {
    use crate::SandboxBackend as _;
    let test = "an_extra_listener_beside_the_door_turns_the_host_check_red";
    let Some(harness) = Harness::new(test) else {
        return;
    };
    let (handle, _output) = harness.launch("touch /work/started; sleep 120");
    let dir = handle.unit().map(|stop| stop.runtime.clone()).unwrap();
    let _second = std::os::unix::net::UnixListener::bind(dir.join("door/door_22")).unwrap();
    let results = harness.backend.isolation_check(&handle);
    let single = results
        .iter()
        .find(|r| r.check == crate::IsolationCheck::SingleSocket)
        .unwrap();
    assert!(!single.passed, "{results:?}");
    assert!(single.evidence.contains("door_22"), "{}", single.evidence);
    assert_eq!(
        single.diagnostic.as_ref().map(|d| d.code.as_str()),
        Some("SANDBOX_047")
    );
    assert!(
        handle.wait_timeout(Duration::from_secs(15)).is_some(),
        "the session was stopped"
    );
    assert!(
        !harness.project.join("started").exists(),
        "the agent ran although the host check failed"
    );
}

/// Mutationsbeweis: `-nic user` an der Maschine färbt `no_network_interface`
/// rot, über die Host-Prüfung (der Gast hat keinen Treiber und sähe nichts).
#[test]
fn a_user_network_at_the_machine_turns_the_check_red() {
    use crate::SandboxBackend as _;
    let test = "a_user_network_at_the_machine_turns_the_check_red";
    let Some(harness) = Harness::new(test) else {
        return;
    };
    super::EXTRA_MACHINE_ARGS
        .with_borrow_mut(|extra| *extra = vec!["-nic".to_owned(), "user".to_owned()]);
    let (handle, _output) = harness.launch("sleep 120");
    super::EXTRA_MACHINE_ARGS.with_borrow_mut(Vec::clear);
    let results = harness.backend.isolation_check(&handle);
    let net = results
        .iter()
        .find(|r| r.check == crate::IsolationCheck::NoNetworkInterface)
        .unwrap();
    assert!(!net.passed, "{results:?}");
    assert!(net.evidence.contains("-nic user"), "{}", net.evidence);
    assert!(handle.wait_timeout(Duration::from_secs(15)).is_some());
}

/// Setzt der Test unten für seinen Kindprozess.
const CHILD_ENV: &str = "HUMANITL_TEST_VM_KILL_CHILD";

/// Der Kindprozess des Tests unten: startet eine Sitzung, meldet Unit und
/// Laufzeitverzeichnis auf stdout und wartet, bis er beendet wird. Ohne
/// [`CHILD_ENV`] tut er nichts.
#[test]
#[ignore = "run only as the child of the SIGKILL test"]
fn session_of_a_daemon_that_gets_killed() {
    use crate::SandboxBackend as _;
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let Some(harness) = Harness::new("session_of_a_daemon_that_gets_killed") else {
        println!("SKIP");
        return;
    };
    let (handle, _output) = harness.launch("sleep 600");
    let results = harness.backend.isolation_check(&handle);
    let stop = handle.unit().unwrap();
    println!(
        "UNIT {} DIR {} XDG {} PASSED {} WHY {:?}",
        stop.unit.service(),
        stop.runtime.display(),
        harness.dir.path().join("xdg").display(),
        results.iter().all(|r| r.passed),
        results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| host_part(&r.evidence))
            .collect::<Vec<_>>()
    );
    std::mem::forget(harness);
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

/// Der Teil eines Belegs, den die Prüfung auf dem Host beisteuert; die
/// Annotation der CI ist kurz, und dieser Teil steht am Ende.
fn host_part(evidence: &str) -> String {
    evidence
        .rsplit_once("host: ")
        .map_or_else(|| evidence.to_owned(), |(_, host)| host.to_owned())
}

/// Hält jeden Emulator-Prozess der Unit mit `SIGSTOP` an; gibt ihre Zahl
/// zurück.
fn freeze_the_machine(unit: &str) -> usize {
    let output = std::process::Command::new("systemctl")
        .args(["--user", "show", "--property=ControlGroup", "--value", unit])
        .output()
        .unwrap();
    let group = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let procs = std::fs::read_to_string(
        std::path::Path::new("/sys/fs/cgroup")
            .join(group.trim_start_matches('/'))
            .join("cgroup.procs"),
    )
    .unwrap_or_default();
    let mut frozen = 0;
    for pid in procs
        .lines()
        .filter_map(|line| line.trim().parse::<i32>().ok())
    {
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
        if comm.trim().starts_with("qemu-system")
            && let Some(pid) = rustix::process::Pid::from_raw(pid)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::STOP);
            frozen += 1;
        }
    }
    frozen
}

/// HUM-444, Integration: Der Daemon wird mit `SIGKILL` beendet; die Unit
/// seiner Sitzung ist binnen 5 s weg, ohne dass der Gast mitwirkt. Der Rest
/// im Laufzeitverzeichnis geht beim nächsten Start (`SANDBOX_041`).
#[test]
fn a_killed_daemon_ends_the_unit_within_five_seconds() {
    use std::io::BufRead as _;
    let test = "a_killed_daemon_ends_the_unit_within_five_seconds";
    if Harness::new(test).is_none() {
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "qemu::kvm_tests::session_of_a_daemon_that_gets_killed",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    // libtest writes `test <name> ... ` before the test's own output, on the
    // same line.
    let line = stdout
        .lines()
        .map_while(Result::ok)
        .find_map(|line| {
            line.find("UNIT ")
                .or_else(|| line.find("SKIP"))
                .map(|at| line[at..].to_owned())
        })
        .unwrap_or_default();
    let words: Vec<&str> = line.split(' ').collect();
    assert_eq!(words.first(), Some(&"UNIT"), "the child said {line:?}");
    let (unit, dir, xdg, passed) = (words[1], words[3], words[5], words[7]);
    assert_eq!(passed, "true", "{line}");
    let active = || {
        std::process::Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", unit])
            .status()
            .is_ok_and(|status| status.success())
    };
    assert!(active(), "the unit runs before the kill");
    // The guest must not help: its machine is stopped before the daemon
    // dies, so neither the control stream nor a power-off can end the unit.
    let frozen = freeze_the_machine(unit);
    assert!(frozen > 0, "no emulator process found in {unit}");
    let _ = rustix::process::kill_process(
        rustix::process::Pid::from_child(&child),
        rustix::process::Signal::KILL,
    );
    let _ = child.wait();
    let killed = Instant::now();
    while active() && killed.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let took = killed.elapsed();
    let still = active();
    eprintln!("{test}: the unit ended {took:?} after SIGKILL");
    if still {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", unit])
            .status();
    }
    assert!(!still, "the unit {unit} still runs {took:?} after the kill");
    assert!(took < Duration::from_secs(5), "the unit took {took:?}");
    // Der nächste Start räumt den Rest weg.
    assert!(std::path::Path::new(dir).exists());
    let paths = humanitl_config::Paths::new(Env::from_pairs([("XDG_RUNTIME_DIR", xdg)]));
    let removed = super::find_leftovers(&paths).remove();
    assert_eq!(removed.len(), 1, "{removed:?}");
    assert_eq!(removed[0].code.as_str(), "SANDBOX_041");
    assert!(!std::path::Path::new(dir).exists());
    if let Some(instance) = std::path::Path::new(dir).parent() {
        let _ = std::fs::remove_dir(instance);
    }
    let _ = std::fs::remove_dir_all(std::path::Path::new(xdg).parent().unwrap());
}
