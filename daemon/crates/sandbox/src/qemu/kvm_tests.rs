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
        "report:\n  {}\ndoor:\n  {}\nemulator exit: {:?}\nhelpers:\n{}\nconsole:\n{}",
        outcome.report.join("\n  "),
        outcome.door.join("\n  "),
        outcome.exit,
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
