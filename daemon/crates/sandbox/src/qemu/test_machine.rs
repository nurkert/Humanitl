//! Nur für die KVM-Tests (HUM-471): zwei Eingriffe in die Zeile der Maschine
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
}

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
    // Vor das Programm des Emulators, also noch in die Optionen von bwrap.
    if let Some(at) = line.iter().position(|word| word == "-M") {
        let program = at.saturating_sub(1);
        for (offset, word) in binds.into_iter().enumerate() {
            line.insert(program + offset, word);
        }
    }
}

/// Die Eigenschaft, mit der die Unit in das Verzeichnis der Konsole
/// schreiben darf.
pub(crate) fn unit_property() -> Option<String> {
    current()
        .console
        .map(|dir| format!("ReadWritePaths={}", dir.display()))
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
