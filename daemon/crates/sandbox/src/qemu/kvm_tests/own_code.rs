//! Die letzten Wege zu eigenem Code in der Unit (HUM-557).
//!
//! Ein Prozess, der schon eigenen Code ausführt, wie ein übernommener
//! Helfer, ist hier ein Python im Gefängnis der Maschine und in dem des
//! vsock-Geräts, gestartet von der Shell des Tests vor dem Helfer
//! (`TestMachine::jailed_command`, `TestMachine::device_command`); Python
//! steht dafür, nur im Test, mit Lader und Bibliotheken in `ExecPaths=`. Es
//! versucht jeden Weg, den HUM-480 offen ließ, und den, der ihn öffnete:
//!
//! - die anderen Programme der Unit (die Shell, bubblewrap, die anderen
//!   Helfer): verdeckt, `EACCES`;
//! - ein zweites bubblewrap, das ein tmpfs ohne `noexec` anlegt und eine
//!   Kopie daraus startet (die Datei bubblewraps unter einem Pfad des
//!   Tests, wie ein Gefängnis, das sie noch sähe): kein weiterer
//!   Nutzer-Namensraum (`--disable-userns`), `ENOSPC`;
//! - eine Kopie in der Wurzel des Gefängnisses und in `/dev`: nur lesbar,
//!   `EROFS`;
//! - eine Kopie im tmpfs einer Maske unter `/work` (`.git/hooks`), dorthin
//!   gebunden: geschrieben, aber `noexec`, `EACCES`;
//! - ein `memfd`, gestartet mit `fexecve` (`execveat`) im Gefängnis der
//!   Maschine und über `/proc/self/fd` (`execve`) in dem des Geräts: der
//!   Filter, `EPERM`.
//!
//! Nach der Zulassung wiederholt eine wartende Unterschale im Gefängnis der
//! Maschine drei davon (wie in HUM-480, ohne `nsenter`). Der Helfer selbst
//! startet in beiden Gefängnissen, die Prüfung ist grün.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use humanitl_config::{Env, QemuConfig};

use super::super::programs::{exec_files_of, hidden_in_jail, unit_programs};
use super::super::test_machine::{self, RUNTIME_PLACEHOLDER, TestMachine};
use super::{Harness, built_shim, console_wanted, make_fifo, signal_and_wait};

/// Der Prozess mit eigenem Code. `copy <Ziel>` schreibt eine Kopie des
/// eigenen Programms nach `<Ziel>` und startet sie, `memfd-at` und
/// `memfd-proc` legen sie in einen `memfd` und starten sie über
/// `execveat` (`os.execve` mit Deskriptor, also `fexecve`) oder über
/// `/proc/self/fd`, `unshare` legt einen Nutzer-Namensraum an. Die Kopie
/// läuft mit `--version`: Gelingt ein Weg, steht „Python" in der Ausgabe und
/// der Status ist 0; sonst nennt die Fehlerausgabe den Grund, Status 1.
const PROBE: &str = r#"import os, sys
op = sys.argv[1]
def write_all(fd, data):
    while data:
        data = data[os.write(fd, data):]
try:
    if op == "unshare":
        os.unshare(os.CLONE_NEWUSER | os.CLONE_NEWNS)
        print("unshared")
        sys.exit(0)
    data = open(sys.executable, "rb").read()
    if op == "copy":
        fd = os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o755)
        write_all(fd, data)
        os.close(fd)
        print("written", file=sys.stderr)
        os.execv(sys.argv[2], [sys.argv[2], "--version"])
    fd = os.memfd_create("probe", 0)
    write_all(fd, data)
    if op == "memfd-at":
        os.execve(fd, ["probe", "--version"], {})
    os.execv("/proc/self/fd/%d" % fd, ["probe", "--version"])
except OSError as err:
    print("%s: %s" % (op, err.strerror), file=sys.stderr)
    sys.exit(1)
"#;

/// Wo das Gefängnis der Maschine im Test bubblewrap und eine Maske sieht.
const TEST_BWRAP: &str = "/humanitl-test/bwrap";
const TEST_MASK: &str = "/humanitl-test/mask";

/// Ein Versuch: Ort, Befehl, und was gelten muss.
struct Attempt {
    place: String,
    command: String,
    expect: Expect,
}

#[derive(Debug, Clone, Copy)]
enum Expect {
    /// Status 126, „Permission denied" von der Shell.
    Denied,
    /// Status 1 mit diesem Grund vom Python.
    Fails(&'static str),
    /// Status 0, und diese Zeile steht in der Ausgabe.
    Runs(&'static str),
}

impl Attempt {
    fn new(place: impl Into<String>, command: impl Into<String>, expect: Expect) -> Self {
        Self {
            place: place.into(),
            command: command.into(),
            expect,
        }
    }

    /// Der Befehl der Shell, der Status, Ausgabe und Fehler nach `evidence`
    /// schreibt.
    fn shell(&self, evidence: &Path) -> String {
        format!(
            "{command} >{dir}/{place}.out 2>{dir}/{place}.err; echo $? >{dir}/{place}.status",
            command = self.command,
            dir = evidence.display(),
            place = self.place
        )
    }

    /// Was nicht gilt, oder nichts.
    fn check(&self, evidence: &Path) -> Option<String> {
        let [status, out, err] = ["status", "out", "err"].map(|kind| {
            std::fs::read_to_string(evidence.join(format!("{}.{kind}", self.place)))
                .unwrap_or_default()
                .trim()
                .to_owned()
        });
        let ok = match self.expect {
            Expect::Denied => status == "126" && err.contains("Permission denied"),
            Expect::Fails(why) => status == "1" && err.contains(why) && !out.contains("Python"),
            Expect::Runs(what) => status == "0" && out.contains(what),
        };
        (!ok).then(|| {
            format!(
                "{}: expected {:?}, got status {status:?}, out {out:?}, err {err:?} ({})",
                self.place, self.expect, self.command
            )
        })
    }
}

fn shell_of(attempts: &[Attempt], evidence: &Path) -> String {
    attempts
        .iter()
        .map(|attempt| attempt.shell(evidence))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Die Programme, die das Gefängnis mit dem Helfer `own` verdeckt, so wie
/// der Plan sie berechnet.
fn hidden(own: impl Fn(&super::super::prerequisites::Resolved) -> PathBuf) -> Vec<PathBuf> {
    let config = QemuConfig {
        kernel: std::env::var_os("HUMANITL_TEST_VM_KERNEL").map(PathBuf::from),
        modules: std::env::var_os("HUMANITL_TEST_VM_MODULES").map(PathBuf::from),
        ..QemuConfig::default()
    };
    let resolved = super::resolve(&Env::from_process(), &config).unwrap();
    let programs = unit_programs(&resolved, &built_shim().unwrap()).unwrap();
    hidden_in_jail(&programs, &own(&resolved))
}

/// Je verdecktes Programm ein Versuch, es zu starten, `<prefix>-hidden-<Name>`.
fn hidden_attempts(
    prefix: &str,
    own: impl Fn(&super::super::prerequisites::Resolved) -> PathBuf,
) -> Vec<Attempt> {
    hidden(own)
        .into_iter()
        .map(|program| {
            let name = program.file_name().unwrap().to_string_lossy().into_owned();
            Attempt::new(
                format!("{prefix}-hidden-{name}"),
                format!("{} --version", program.display()),
                Expect::Denied,
            )
        })
        .collect()
}

/// Die Versuche im Gefängnis der Maschine vor dem Emulator; `$1` ist der
/// Emulator.
fn machine_attempts(probe: &dyn Fn(&str) -> String, python: &Path) -> Vec<Attempt> {
    let mut machine = hidden_attempts("m", |resolved| resolved.emulator.clone());
    machine.extend([
        Attempt::new("m-own", "\"$1\" --version", Expect::Runs("QEMU")),
        Attempt::new(
            "m-bwrap",
            format!(
                "{TEST_BWRAP} --ro-bind /usr /usr --symlink usr/lib64 /lib64 --tmpfs /t \
                 --perms 0755 --file 9 /t/p /t/p --version 9<{}",
                python.display()
            ),
            Expect::Fails("ENOSPC"),
        ),
        Attempt::new(
            "m-unshare",
            probe("unshare"),
            Expect::Fails("No space left"),
        ),
        Attempt::new("m-root", probe("copy /probe"), Expect::Fails("Read-only")),
        Attempt::new(
            "m-dev",
            probe("copy /dev/shm/probe"),
            Expect::Fails("Read-only"),
        ),
        Attempt::new(
            "m-mask",
            probe(&format!("copy {TEST_MASK}/probe")),
            Expect::Fails("Permission denied"),
        ),
        Attempt::new(
            "m-execveat",
            probe("memfd-at"),
            Expect::Fails("Operation not permitted"),
        ),
    ]);
    machine
}

/// Nach der Zulassung, aus der wartenden Unterschale im Gefängnis der
/// Maschine.
fn after_attempts(probe: &dyn Fn(&str) -> String) -> [Attempt; 3] {
    let shell = std::fs::canonicalize(super::super::lines::SCRIPT_SHELL).unwrap();
    [
        Attempt::new(
            "a-shell",
            format!("{} -c true", shell.display()),
            Expect::Denied,
        ),
        Attempt::new("a-root", probe("copy /probe"), Expect::Fails("Read-only")),
        Attempt::new(
            "a-execveat",
            probe("memfd-at"),
            Expect::Fails("Operation not permitted"),
        ),
    ]
}

/// Mutationsbeweise, je Weg einzeln gemessen (2026-11-19, Debian 14,
/// bubblewrap 0.13.1, systemd 262), der Test wurde jedes Mal rot:
/// - ohne die Masken aus `lines::seal`: `m-hidden-*` und `d-hidden-*` laufen
///   (Status 0);
/// - ohne `--disable-userns`: `m-bwrap` startet die Kopie im tmpfs
///   (Python, Status 0), `m-unshare` gelingt;
/// - ohne `--remount-ro /`: `m-root` läuft, ohne `--remount-ro /dev`
///   `m-dev`;
/// - ohne `noexec` im `TemporaryFileSystem=` der Masken: `m-mask` läuft;
/// - ohne `execveat` im Filter des Emulators: `m-execveat` läuft; mit dem
///   Filter des Emulators im Gefängnis des Geräts: `d-memfd` läuft.
#[test]
fn a_helper_reaches_no_code_of_its_own_choosing() {
    let test = "a_helper_reaches_no_code_of_its_own_choosing";
    let Some(harness) = Harness::new(test) else {
        return;
    };
    let evidence = harness.dir.path().join("evidence");
    std::fs::create_dir_all(&evidence).unwrap();
    // Eine Ordner-Maske des Profils, die es im Projekt gibt.
    std::fs::create_dir_all(harness.project.join(".git/hooks")).unwrap();
    let probe_file = evidence.join("probe.py");
    std::fs::write(&probe_file, PROBE).unwrap();
    let python = std::fs::canonicalize("/usr/bin/python3").expect("python3 for the probe");
    let probe = |args: &str| format!("{} -I -S {} {args}", python.display(), probe_file.display());
    let bwrap = std::fs::canonicalize("/usr/bin/bwrap").unwrap();

    let machine = machine_attempts(&probe, &python);
    let after = after_attempts(&probe);
    let mut device = hidden_attempts("d", |resolved| resolved.vsock_device.clone());
    device.push(Attempt::new(
        "d-memfd",
        probe("memfd-proc"),
        Expect::Fails("Operation not permitted"),
    ));

    let signal = evidence.join("admitted");
    make_fifo(&signal);
    let waiting = format!(
        "( read -r _ <{}; {} ) &",
        signal.display(),
        shell_of(&after, &evidence)
    );
    let properties = vec![format!("ExecPaths={}", exec_files_of(&python).join(" "))];
    test_machine::set(TestMachine {
        console: console_wanted().then(|| harness.dir.path().join("console")),
        jailed_command: Some(format!("{waiting} {}", shell_of(&machine, &evidence))),
        device_command: Some(shell_of(&device, &evidence)),
        writable: Some(evidence.clone()),
        extra_shells: 1,
        unit_properties: properties,
        machine_binds: vec![
            (bwrap.to_string_lossy().into_owned(), TEST_BWRAP.to_owned()),
            (
                format!("{RUNTIME_PLACEHOLDER}/masks/0"),
                TEST_MASK.to_owned(),
            ),
        ],
        ..TestMachine::default()
    });
    let (handle, _output) = harness.launch("touch /work/started; sleep 120");
    super::assert_admitted(&harness, &handle);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !harness.project.join("started").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        harness.project.join("started").exists(),
        "the agent was not admitted"
    );
    let mut wrong: Vec<String> = machine
        .iter()
        .chain(&device)
        .filter_map(|attempt| attempt.check(&evidence))
        .collect();
    // Die Kopie in der Maske ist geschrieben; sie scheitert erst am Start.
    let written = std::fs::read_to_string(evidence.join("m-mask.err")).unwrap_or_default();
    if !written.contains("written") {
        wrong.push(format!("m-mask: the copy was not written: {written:?}"));
    }
    signal_and_wait(&signal, &evidence.join("a-execveat.status"));
    let _ = handle.kill();
    let _ = handle.wait_timeout(Duration::from_secs(15));
    wrong.extend(after.iter().filter_map(|attempt| attempt.check(&evidence)));
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
