//! Nur für die KVM-Tests (HUM-471, HUM-468, HUM-485, HUM-480): fünf Eingriffe in die Zeile der Maschine
//! einer Sitzung des Adapters, je Faden gesetzt, damit parallele Tests
//! einander nichts unterschieben. Die Endform kennt keinen davon.
//!
//! - **Die Konsole des Gasts** in eine Datei außerhalb des
//!   Laufzeitverzeichnisses (`-serial file:`, `console=ttyS0` statt
//!   `quiet`), zur Fehlersuche wie `HUMANITL_TEST_VM_CONSOLE=1` im Startweg
//!   der Tests. Die Unit darf dorthin schreiben, und die Host-Prüfung
//!   übergeht genau diese Datei; jeder andere Fund bleibt. Ohne den Schalter
//!   hat die Maschine keine Konsole, so wie im gegateten Lauf der CI.
//! - **Ein Zwischenstück am vhost-user-Socket des vsock-Geräts**: Der
//!   Emulator verbindet sich mit einem Socket des Tests statt mit dem des
//!   Geräts, und der Test reicht jede Nachricht weiter. So kann ein Test die
//!   Einrichtung des Geräts anhalten und den Wettlauf erzwingen, den HUM-471
//!   gefunden hat.
//! - **Ein zusätzlicher Prozess in der Unit** (HUM-468): Eine Shell startet
//!   neben der Maschine einen Befehl wie `sleep 600` im Hintergrund und
//!   ersetzt sich dann durch die Zeile der Maschine. Der Befehl läuft in der
//!   cgroup der Unit, so wie ein Programm, das dort nicht hingehört; die
//!   Host-Prüfung muss ihn finden.
//! - **Weitere Eigenschaften der Unit** (HUM-485), etwa eine Grenze, die die
//!   geplante überschreibt; die Host-Prüfung muss die Abweichung finden.
//! - **Ein Befehl im Gefängnis der Maschine** (HUM-480): Dort startet eine
//!   Shell den Befehl und ersetzt sich dann durch den Emulator, so wie ein
//!   übernommener Helfer ein Programm starten könnte. Was er findet, schreibt
//!   er in einen Ordner des Tests, den die Unit beschreiben darf und den das
//!   Gefängnis beschreibbar bindet. Seit HUM-557 verdeckt jedes Gefängnis
//!   die Shell der Unit; die Shell des Tests liegt deshalb unter
//!   [`TEST_SHELL`], dieselbe Datei an einem Pfad, den die Endform nicht
//!   kennt. Die Host-Prüfung kennt im Test auch diesen Namen.
//! - **Ein Befehl im Gefängnis des vsock-Geräts** (HUM-557), ebenso, unter
//!   dessen Filter.
//! - **Weitere Einhängungen im Gefängnis der Maschine** (HUM-557), etwa das
//!   tmpfs einer Maske unter `/work`, um zu prüfen, dass dort nichts läuft.

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::host_check::Finding;
use super::lines::Layout;

/// Was ein Test an der Zeile der Maschine ändert.
#[derive(Debug, Clone, Default)]
pub(crate) struct TestMachine {
    /// Das Verzeichnis, in das die Konsole jeder Sitzung als
    /// `<Sitzung>.log` geht.
    pub(crate) console: Option<PathBuf>,
    /// Der Socket, mit dem der Emulator sich statt mit dem vsock-Gerät
    /// verbindet.
    pub(crate) vsock_relay: Option<PathBuf>,
    /// Ein Befehl der Shell, der neben der Maschine in der Unit läuft.
    pub(crate) extra_process: Option<String>,
    /// Eigenschaften, die die Unit nach ihren eigenen bekommt (HUM-485); bei
    /// systemd gilt die letzte, so lässt sich eine Grenze überschreiben.
    pub(crate) unit_properties: Vec<String>,
    /// Ein Befehl der Shell im Gefängnis der Maschine vor dem Emulator
    /// (HUM-480). In ihm und in [`Self::extra_process`] steht
    /// [`RUNTIME_PLACEHOLDER`] für das Laufzeitverzeichnis der Sitzung.
    pub(crate) jailed_command: Option<String>,
    /// Ein Ordner des Tests, beschreibbar in der Unit und im Gefängnis der
    /// Maschine (HUM-480).
    pub(crate) writable: Option<PathBuf>,
    /// So viele Shells hält der Befehl im Gefängnis über den Start hinaus,
    /// etwa eine, die auf ein Zeichen des Tests wartet (HUM-480); die
    /// Host-Prüfung zählt sie zu den Shells des Skripts.
    pub(crate) extra_shells: usize,
    /// Ein Befehl der Shell im Gefängnis des vsock-Geräts vor dem Gerät
    /// (HUM-557); [`Self::writable`] bindet auch dieses Gefängnis.
    pub(crate) device_command: Option<String>,
    /// Weitere beschreibbare Einhängungen im Gefängnis der Maschine,
    /// Quelle und Ziel, mit [`RUNTIME_PLACEHOLDER`] (HUM-557).
    pub(crate) machine_binds: Vec<(String, String)>,
}

/// Wo die Shell des Tests in einem Gefängnis liegt (HUM-557).
pub(crate) const TEST_SHELL: &str = "/humanitl-test/sh";

/// Steht in einem Befehl des Tests für das Laufzeitverzeichnis der Sitzung,
/// das erst der Plan kennt (HUM-480).
pub(crate) const RUNTIME_PLACEHOLDER: &str = "{runtime}";

thread_local! {
    static TEST_MACHINE: RefCell<TestMachine> = RefCell::new(TestMachine::default());
}

/// Setzt die Eingriffe für die Sitzungen, die dieser Faden plant.
pub(crate) fn set(machine: TestMachine) {
    TEST_MACHINE.with_borrow_mut(|current| *current = machine);
}

/// Die Eingriffe dieses Fadens.
fn current() -> TestMachine {
    TEST_MACHINE.with_borrow(Clone::clone)
}

/// Die Shells, die der Befehl im Gefängnis dieses Fadens hält.
pub(crate) fn extra_shells() -> usize {
    current().extra_shells
}

/// Die Datei der Konsole einer Sitzung.
pub(crate) fn console_file(dir: &Path, layout: &Layout) -> PathBuf {
    let name = layout.dir.file_name().map_or_else(
        || "session".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    dir.join(format!("{name}.log"))
}

/// Ändert die Zeile der Maschine, wie dieser Faden es verlangt.
pub(crate) fn apply(line: &mut Vec<OsString>, layout: &Layout) {
    let machine = current();
    let mut binds: Vec<OsString> = Vec::new();
    if let Some(dir) = &machine.console {
        let file = console_file(dir, layout);
        for word in line.iter_mut() {
            if word
                .to_str()
                .is_some_and(|text| text.starts_with("rootfstype="))
            {
                let text = word.to_string_lossy().replace(" quiet ", " console=ttyS0 ");
                *word = OsString::from(text);
            }
        }
        if let Some(at) = line.iter().position(|word| word == "-serial") {
            line[at + 1] = OsString::from(format!("file:{}", file.display()));
        }
        binds.extend(["--bind".into(), dir.clone().into(), dir.clone().into()]);
    }
    if let Some(dir) = &machine.writable {
        binds.extend(["--bind".into(), dir.clone().into(), dir.clone().into()]);
    }
    if let Some(relay) = &machine.vsock_relay {
        let device = format!("path={}", layout.device_socket().display());
        for word in line.iter_mut() {
            if word.to_str().is_some_and(|text| text.ends_with(&device)) {
                let text = word
                    .to_string_lossy()
                    .replace(&device, &format!("path={}", relay.display()));
                *word = OsString::from(text);
            }
        }
        if let Some(dir) = relay.parent() {
            binds.extend(["--ro-bind".into(), dir.into(), dir.into()]);
        }
    }
    let runtime = layout.dir.to_string_lossy();
    for (source, target) in &machine.machine_binds {
        binds.extend([
            "--bind".into(),
            source.replace(RUNTIME_PLACEHOLDER, &runtime).into(),
            target.replace(RUNTIME_PLACEHOLDER, &runtime).into(),
        ]);
    }
    if let Some(at) = line.iter().position(|word| word == "-M") {
        jail_command(
            line,
            at.saturating_sub(1),
            binds,
            machine.jailed_command.as_deref(),
            layout,
        );
    }
    if let Some(command) = &machine.extra_process {
        let shell: [OsString; 4] = [
            super::lines::SCRIPT_SHELL.into(),
            "-c".into(),
            format!(
                "{} & exec \"$@\"",
                command.replace(RUNTIME_PLACEHOLDER, &layout.dir.to_string_lossy())
            )
            .into(),
            "humanitl-test".into(),
        ];
        line.splice(0..0, shell);
    }
}

/// Ändert die Zeile des vsock-Geräts, wie dieser Faden es verlangt
/// (HUM-557).
pub(crate) fn apply_device(line: &mut Vec<OsString>, layout: &Layout) {
    let machine = current();
    let Some(command) = machine.device_command.as_deref() else {
        return;
    };
    let mut binds: Vec<OsString> = Vec::new();
    if let Some(dir) = &machine.writable {
        binds.extend(["--bind".into(), dir.clone().into(), dir.clone().into()]);
    }
    if let Some(at) = line.iter().position(|word| word == "--guest-cid") {
        jail_command(line, at.saturating_sub(1), binds, Some(command), layout);
    }
}

/// Fügt `binds` vor den Abschluss des Gefängnisses ein
/// ([`super::lines::seal_start`]), und vor das Programm an `program` die
/// Shell des Tests mit `command`, die sich danach durch das Programm
/// ersetzt.
fn jail_command(
    line: &mut Vec<OsString>,
    program: usize,
    mut binds: Vec<OsString>,
    command: Option<&str>,
    layout: &Layout,
) {
    let jailed: Vec<OsString> = command
        .iter()
        .flat_map(|command| {
            [
                TEST_SHELL.into(),
                "-c".into(),
                format!(
                    "{}; exec \"$@\"",
                    command.replace(RUNTIME_PLACEHOLDER, &layout.dir.to_string_lossy())
                )
                .into(),
                "humanitl-test".into(),
            ]
        })
        .collect();
    if command.is_some() {
        let shell = std::fs::canonicalize(super::lines::SCRIPT_SHELL)
            .unwrap_or_else(|_| PathBuf::from(super::lines::SCRIPT_SHELL));
        binds.extend(["--ro-bind".into(), shell.into(), TEST_SHELL.into()]);
    }
    for (offset, word) in jailed.into_iter().enumerate() {
        line.insert(program + offset, word);
    }
    let sealed = super::lines::seal_start(line).unwrap_or(program);
    for (offset, word) in binds.into_iter().enumerate() {
        line.insert(sealed + offset, word);
    }
}

/// Die Eigenschaften, mit denen die Unit in das Verzeichnis der Konsole und
/// in den Ordner des Tests schreiben darf, ohne dort etwas auszuführen, wie
/// bei jedem beschreibbaren Pfad der Endform (HUM-480), und die, die ein
/// Test dazugibt.
pub(crate) fn unit_properties() -> Vec<String> {
    let machine = current();
    machine
        .console
        .iter()
        .chain(&machine.writable)
        .flat_map(|dir| {
            [
                format!("ReadWritePaths={}", dir.display()),
                format!("NoExecPaths={}", dir.display()),
            ]
        })
        .chain(machine.unit_properties)
        .collect()
}

/// Die Funde der Host-Prüfung ohne die zur Konsole dieses Fadens.
pub(crate) fn forgive(findings: &mut Vec<Finding>) {
    if let Some(dir) = current().console {
        let dir = format!("{}/", dir.display());
        findings.retain(|finding| !only_console_files(&finding.what, &dir));
    }
}

/// Ob `what` das Verzeichnis der Konsole nennt und dort nur Dateien
/// `<Sitzung>.log`; ein Fund zu einem anderen Pfad darin (etwa einem Socket)
/// bleibt stehen.
fn only_console_files(what: &str, dir: &str) -> bool {
    let mut parts = what.split(dir);
    parts.next();
    let mut any = false;
    for rest in parts {
        let name: String = rest
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, ',' | ';' | ')' | '"' | '\''))
            .collect();
        if name.contains('/')
            || !std::path::Path::new(&name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
        {
            return false;
        }
        any = true;
    }
    any
}

mod tests {
    use super::only_console_files;

    /// Nur Funde zu den Konsolendateien selbst fallen weg.
    #[test]
    fn only_findings_about_console_logs_are_forgiven() {
        let dir = "/run/h/console/";
        assert!(only_console_files(
            "-serial file:/run/h/console/abc.log",
            dir
        ));
        assert!(!only_console_files(
            "-monitor unix:/run/h/console/monitor.sock,server",
            dir
        ));
        assert!(!only_console_files(
            "file:/run/h/console/abc.log and /run/h/console/x/y.log",
            dir
        ));
        assert!(!only_console_files(
            "-serial file:/run/h/console2/abc.log",
            dir
        ));
    }
}
