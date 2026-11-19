//! Was ein Prozess unter der gehärteten Unit des Daemons sieht (HUM-529).
//!
//! Der äußere Test startet dieses Testprogramm als vorübergehende Unit
//! `humanitl-test-hum529-<kennung>` des Dienstverwalters des Nutzers, mit den
//! Eigenschaften der Härtung aus `packaging/systemd/humanitld.service`, und
//! darin genau den inneren Test. Der schreibt zwei Dinge aus, die der äußere
//! Test mit seiner eigenen Sicht ohne Härtung vergleicht:
//!
//! - Jede Voraussetzung der Kernel-Stufe, so wie der Doctor des Daemons sie
//!   prüft. Unter `ProtectKernelModules=yes` (Humanitl 0.4.0) fehlte das
//!   Verzeichnis der Module, und der Doctor meldete `SANDBOX_045`.
//! - Jeden Einhängepunkt unter `/usr`. bubblewrap bindet `/usr` rekursiv,
//!   und jede Einhängung, die die Unit darunter legt, sieht der Shim als
//!   Einhängung außerhalb des Plans (`SANDBOX_049`); unter 0.4.0 war das das
//!   tmpfs über `/usr/lib/modules`, und kein Start über den Daemon gelang.
//!
//! Kein Gast startet, KVM braucht der Test nicht; er steht hier, weil der
//! CI-Job `kvm-tests` einen Dienstverwalter des Nutzers hat und
//! `HUMANITL_REQUIRE_KVM=1` setzt. Ohne Dienstverwalter überspringt er.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use humanitl_config::{Env, QemuConfig};

use super::super::prerequisites::each_prerequisite;
use super::unavailable;
use crate::doctor::StaticLine;

/// Der Anfang jeder Zeile, die der innere Test für den äußeren schreibt.
const MARK: &str = "HUM529 ";

/// Der volle Name des inneren Tests für `--exact`.
const INNER: &str = "qemu::kvm_tests::hardened::inner_view_under_the_hardened_unit";

/// So lange darf der innere Lauf dauern; er fragt jedes Programm der
/// Kernel-Stufe nach seiner Hilfe.
const INNER_TIMEOUT: Duration = Duration::from_secs(180);

/// Die Zeilen der Unit, die nicht zur Härtung gehören oder die der Test
/// anders setzt: Art und Start des Dienstes, Neustart, und
/// `ReadWritePaths`, dessen Bezeichner (`%h`, `%t`) nur eine Unit-Datei
/// auflöst. Die Zeile gibt nur Schreibrecht und legt keine Einhängung unter
/// `/usr`.
const NOT_HARDENING: &[&str] = &[
    "Type",
    "ExecStart",
    "Restart",
    "RestartSec",
    "ReadWritePaths",
];

/// Jede Eigenschaft der Härtung aus dem Abschnitt `[Service]` der Unit.
fn hardening(unit: &str) -> Vec<String> {
    let mut in_service = false;
    let mut properties = Vec::new();
    for line in unit.lines().map(str::trim) {
        if line.starts_with('[') {
            in_service = line == "[Service]";
            continue;
        }
        if !in_service || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        if !NOT_HARDENING.contains(&key) {
            properties.push(line.to_owned());
        }
    }
    properties
}

/// Die Unit des Pakets, aus dem Baum dieses Crates.
fn packaged_unit() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packaging/systemd/humanitld.service");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// Was dieser Prozess sieht, eine Zeile je Befund: jede Voraussetzung der
/// Kernel-Stufe mit der Vorgabe (dem laufenden Kernel) als `ok <Beleg>`
/// oder `missing <Code> <Grund>`, dann jeder Einhängepunkt unter `/usr`.
fn view() -> Vec<String> {
    let mut lines: Vec<String> = each_prerequisite(&Env::from_process(), &QemuConfig::default())
        .into_iter()
        .map(|(which, line)| match line {
            StaticLine::Checked(Ok(evidence)) => format!("{which:?} ok {evidence:?}"),
            StaticLine::Checked(Err(missing)) => {
                format!("{which:?} missing {} {:?}", missing.code, missing.why)
            }
            StaticLine::Unchecked { after, .. } => format!("{which:?} unchecked after {after:?}"),
        })
        .collect();
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").expect("mountinfo");
    let mut under_usr: Vec<String> = mountinfo
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .filter(|point| *point == "/usr" || point.starts_with("/usr/"))
        .map(|point| format!("mount {point}"))
        .collect();
    under_usr.sort();
    under_usr.dedup();
    lines.extend(under_usr);
    lines
}

/// Das Verzeichnis der Module des laufenden Kernels.
fn running_modules() -> PathBuf {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").expect("osrelease");
    Path::new("/usr/lib/modules").join(release.trim())
}

/// Stoppt die Unit dieses Tests, auch nach einem Fehlschlag; nur genau sie.
struct StopUnit(String);

impl Drop for StopUnit {
    fn drop(&mut self) {
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "--", &format!("{}.service", self.0)])
            .stdin(Stdio::null())
            .output();
    }
}

/// HUM-529: Unter der gehärteten Unit sieht der Daemon jede Voraussetzung der
/// Kernel-Stufe so wie ein Prozess ohne Härtung, auch das Verzeichnis der
/// Module, und die Unit legt keine Einhängung unter `/usr`.
///
/// Mutationsbeweis: `ProtectKernelModules=yes` zurück in die Unit, und die
/// Zeile `Modules` innen wird `missing SANDBOX_045`, dazu steht innen
/// `mount /usr/lib/modules`; der Test wird rot.
#[test]
fn the_hardened_unit_hides_no_module_and_mounts_nothing_under_usr() {
    let test = "the_hardened_unit_hides_no_module_and_mounts_nothing_under_usr";
    let manager = Command::new("systemctl")
        .args(["--user", "show", "--property=Version", "--value"])
        .stdin(Stdio::null())
        .output();
    match manager {
        Ok(output) if output.status.success() => {}
        other => {
            unavailable(
                test,
                &format!("the user's service manager does not answer: {other:?}"),
            );
            return;
        }
    }
    if !running_modules().is_dir() {
        unavailable(
            test,
            &format!(
                "{} is not a directory on this host",
                running_modules().display()
            ),
        );
        return;
    }
    let unit = format!("humanitl-test-hum529-{}", uuid::Uuid::now_v7().simple());
    let _stop = StopUnit(unit.clone());
    let mut command = Command::new("systemd-run");
    command.args([
        "--user",
        "--quiet",
        "--collect",
        "--pipe",
        "--wait",
        "--service-type=exec",
        "--expand-environment=no",
    ]);
    command.arg(format!("--unit={unit}.service"));
    for property in hardening(&packaged_unit()) {
        command.arg(format!("--property={property}"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        command.arg(format!("--setenv=PATH={}", path.to_string_lossy()));
    }
    command
        .arg("--")
        .arg(std::env::current_exe().expect("this test binary"))
        .args([
            "--ignored",
            "--exact",
            INNER,
            "--nocapture",
            "--test-threads=1",
        ]);
    let output = crate::user_unit::run_bounded(command, INNER_TIMEOUT)
        .unwrap_or_else(|err| panic!("{unit} did not end: {err}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let shown = format!(
        "{unit}: {}\nstdout:\n{stdout}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{shown}");
    assert!(
        stdout.contains("1 passed"),
        "the inner test did not run: {shown}"
    );
    let inner: Vec<&str> = stdout
        .lines()
        // Der Prüfstand schreibt `test <name> ... ` ohne Zeilenende vor die
        // erste Ausgabe des Tests.
        .filter_map(|line| line.split_once(MARK).map(|(_, rest)| rest))
        .collect();
    let outside = view();
    assert_eq!(inner, outside, "{shown}");
    assert!(
        outside.iter().any(|line| line.starts_with("Modules ok ")),
        "{outside:?}"
    );
}

/// Der innere Teil: läuft nur in der Unit des äußeren Tests (`--ignored`).
/// Er belegt, dass er unter der Härtung läuft (außerhalb des ersten
/// Nutzer-Namensraums), und schreibt aus, was er sieht.
#[test]
#[ignore = "runs only inside the hardened unit of the_hardened_unit_hides_no_module_and_mounts_nothing_under_usr"]
fn inner_view_under_the_hardened_unit() {
    let uid_map = std::fs::read_to_string("/proc/self/uid_map").unwrap_or_default();
    assert!(
        crate::user_unit::needs_user_manager(&uid_map),
        "not in the hardened unit: {uid_map}"
    );
    for line in view() {
        println!("{MARK}{line}");
    }
}

/// Die Eigenschaften aus der Unit: jede Zeile der Härtung, keine Zeile des
/// Starts, nichts aus einem anderen Abschnitt.
#[test]
fn the_hardening_of_the_packaged_unit_is_read_whole() {
    let properties = hardening(&packaged_unit());
    for property in [
        "PrivateUsers=yes",
        "ProtectSystem=strict",
        "SystemCallFilter=@system-service @mount @sandbox sethostname",
        "SystemCallFilter=~@module",
    ] {
        assert!(
            properties.iter().any(|p| p == property),
            "{property}: {properties:?}"
        );
    }
    for key in NOT_HARDENING.iter().chain(&["Description", "WantedBy"]) {
        assert!(
            !properties.iter().any(|p| p.starts_with(&format!("{key}="))),
            "{key}: {properties:?}"
        );
    }
}
