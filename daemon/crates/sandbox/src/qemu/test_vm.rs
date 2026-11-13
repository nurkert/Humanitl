//! Der Startweg der Kernel-Stufe nur für Tests (HUM-465): ein Gast mit dem
//! Shim als `init`, die Wurzel aus einem festen Plan, ohne Adapter, ohne
//! Daemon und ohne Proxy.
//!
//! Gebaut wie die Endform des Spikes (HUM-442, Q7 und Q8): vier Prozesse, je
//! in einem bwrap-Gefängnis mit `--unshare-all --cap-drop ALL`:
//!
//! - virtiofsd für die Wurzel, nur lesbar, mit seinem Socket als Deskriptor 3
//!   (`--fd`), damit das Laufzeitverzeichnis nicht in der Wurzel des Gasts
//!   steht. Die Wurzel ist das `/usr` des Hosts mit dem Shim, dem Gastplan und
//!   dem Modulverzeichnis unter `/run/humanitl/`.
//! - virtiofsd für `/work`, der den vorher geöffneten Projektordner per
//!   `--bind-fd` bekommt;
//! - vhost-device-vsock als eigener Prozess mit eigener Gruppe, nie
//!   `--forward-cid`;
//! - QEMU `-M microvm` mit `/dev/kvm` als einzigem Gerät und aus `/boot` nur
//!   der Kernel-Datei, ohne Konsole, Monitor, Anzeige und Netz.
//!
//! Auf dem Host lauscht der Test selbst an `<uds>_<Tür-Port>` und spielt den
//! Proxy; den Bericht des Shims holt er sich mit `CONNECT <Bericht-Port>` am
//! vsock-Gerät (Q10). Eine gehärtete Unit des Nutzers baut erst der Adapter
//! (HUM-444); hier laufen die Prozesse als Kinder des Tests, und
//! `--die-with-parent` nimmt sie mit, wenn der Test stirbt. Am Ende werden
//! alle beendet und das Laufzeitverzeichnis entfernt ([`Vm`] beim Drop).
//!
//! Erreichbar nur unter `cfg(test)`: weder der Daemon noch die CLI kennen
//! diesen Weg.

use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::os::fd::{AsFd as _, AsRawFd as _, OwnedFd};
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::prerequisites::Resolved;

/// Wo der Shim, der Gastplan und die Module in der Wurzel des Gasts liegen.
pub(crate) const GUEST_SHIM: &str = "/run/humanitl/humanitl-shim";
/// Siehe [`GUEST_SHIM`].
pub(crate) const GUEST_PLAN: &str = "/run/humanitl/guest-plan.json";
/// Siehe [`GUEST_SHIM`].
pub(crate) const GUEST_MODULES: &str = "/run/humanitl/modules";

/// Der Port der Tür auf dem Host; der Test lauscht an `<uds>_5000`.
pub(crate) const DOOR_PORT: u32 = 5000;
/// Der Port, an dem der Shim im Gast den Bericht ausliefert.
pub(crate) const REPORT_PORT: u32 = 5001;
/// Die CID des Gasts.
const GUEST_CID: u32 = 3;

/// So lange darf der Gast vom Start von QEMU bis zu seinem Ende brauchen.
const RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// Die Zeile, die für einen Gast ohne Bericht steht: eine gescheiterte
/// Prüfung mit Grund.
pub(crate) const NO_REPORT: &str = "CHECK seccomp_applied fail no-report:the_guest_ended_\
    without_a_report;_its_vsock_modules_or_the_report_channel_failed";

/// Die längste Pfadangabe eines Unix-Sockets (`sun_path`).
const SUN_PATH_MAX: usize = 108;

/// Was der Test startet.
pub(crate) struct Spec {
    /// Die Programme, das Kernel-Abbild und seine Module.
    pub(crate) resolved: Resolved,
    /// Der gebaute Shim auf dem Host.
    pub(crate) shim: PathBuf,
    /// Der Projektordner auf dem Host.
    pub(crate) project: PathBuf,
    /// Der Befehl des Agenten im Gast.
    pub(crate) command: Vec<String>,
    /// Module, die der Shim nach denen der Tür lädt.
    pub(crate) extra_modules: Vec<String>,
    /// Nur zur Fehlersuche: die Ausgabe des Gasts in eine Datei des
    /// Laufzeitverzeichnisses (`-serial file:`), die [`Outcome::console`]
    /// liefert. Nie in der Endform.
    pub(crate) console: bool,
}

/// Was der Lauf ergab.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    /// Die Zeilen des Berichts, wie der Shim sie schrieb.
    pub(crate) report: Vec<String>,
    /// Die erste Zeile jeder Anfrage, die an der Tür ankam, in ihrer
    /// Reihenfolge.
    pub(crate) door: Vec<String>,
    /// Wie QEMU endete; `None` nach Ablauf der Frist.
    pub(crate) exit: Option<ExitStatus>,
    /// Die Ausgabe des Gasts, wenn [`Spec::console`] gesetzt war.
    pub(crate) console: Option<String>,
    /// Was die Hilfsprozesse auf stderr schrieben, zur Fehlersuche.
    pub(crate) helpers: String,
}

/// Die laufenden Prozesse und das Laufzeitverzeichnis; beim Drop wird alles
/// beendet und entfernt.
struct Vm {
    dir: tempfile::TempDir,
    children: Vec<(String, Child)>,
}

impl Drop for Vm {
    fn drop(&mut self) {
        for (_, child) in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Das Verzeichnis entfernt `TempDir` danach selbst.
    }
}

impl Vm {
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn spawn(&mut self, name: &str, mut command: Command) -> Result<(), String> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let child = command
            .spawn()
            .map_err(|err| format!("{name} did not start: {err}"))?;
        self.children.push((name.to_owned(), child));
        Ok(())
    }

    /// Ob der Emulator schon geendet hat.
    fn emulator_ended(&mut self) -> bool {
        self.children
            .iter_mut()
            .find(|(name, _)| name == "emulator")
            .is_none_or(|(_, child)| !matches!(child.try_wait(), Ok(None)))
    }

    /// Was die Hilfsprozesse bisher auf stderr schrieben; nur nach ihrem
    /// Ende vollständig.
    fn stderr_of_all(&mut self) -> String {
        let mut text = String::new();
        for (name, child) in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(mut stderr) = child.stderr.take() {
                let mut buffer = String::new();
                let _ = stderr.read_to_string(&mut buffer);
                if !buffer.trim().is_empty() {
                    let _ = writeln!(text, "--- {name}\n{}", buffer.trim_end());
                }
            }
        }
        text
    }
}

/// Das Gefängnis jedes Hilfsprozesses, wie im Spike: nichts geteilt, keine
/// Fähigkeit, nur `/usr` des Hosts.
fn jail(bubblewrap: &Path) -> Command {
    let mut command = Command::new(bubblewrap);
    command.args([
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--ro-bind",
        "/usr",
        "/usr",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib64",
        "/lib64",
        "--symlink",
        "usr/bin",
        "/bin",
        "--symlink",
        "usr/sbin",
        "/sbin",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
    ]);
    command
}

/// Ein Programm außerhalb von `/usr` wird in sein Gefängnis gebunden.
fn bind_program(command: &mut Command, program: &Path) {
    if !program.starts_with("/usr") {
        command.arg("--ro-bind").arg(program).arg(program);
    }
}

/// Ein kurzes Laufzeitverzeichnis: Unix-Sockets tragen höchstens
/// [`SUN_PATH_MAX`] Bytes, und ein Temp-Pfad kann lang sein (HUM-442).
fn runtime_dir() -> Result<tempfile::TempDir, String> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_dir() && dir.as_os_str().len() < 40)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = tempfile::Builder::new()
        .prefix("hum-vm-")
        .tempdir_in(&base)
        .map_err(|err| format!("runtime directory in {}: {err}", base.display()))?;
    let longest = dir
        .path()
        .join(format!("door_{REPORT_PORT}"))
        .as_os_str()
        .len();
    if longest >= SUN_PATH_MAX {
        return Err(format!(
            "runtime directory {} is too long for a socket path",
            dir.path().display()
        ));
    }
    Ok(dir)
}

/// Der Gastplan (`guest.rs` des Shims): Ports, Module, Tag, Umgebung,
/// Mounttabelle mit dem Inode des Projektordners, Befehl.
fn guest_plan(spec: &Spec, project_ino: u64) -> serde_json::Value {
    let entry = |point: &str, fstype: &str, source: &str, mode: &str, flags: &str, count: &str| {
        serde_json::json!({
            "point": point, "type": fstype, "source": source,
            "mode": mode, "flags": flags, "count": count,
        })
    };
    let mount = |point: &str, fstype: &str, source: &str, mode: &str, flags: &str| {
        entry(point, fstype, source, mode, flags, "1")
    };
    // Die Einhängepunkte im Gefängnis der Wurzel kündigt virtiofsd dem Gast
    // als eigene Einhängungen an (ab 1.11 die Vorgabe), und der Gast hängt
    // sie erst beim ersten Zugriff ein: je höchstens einmal, nur lesbar.
    let mut mounts: Vec<serde_json::Value> = [
        "/usr",
        "/proc",
        "/dev",
        "/tmp",
        GUEST_SHIM,
        GUEST_PLAN,
        GUEST_MODULES,
    ]
    .into_iter()
    .map(|point| entry(point, "virtiofs", "type", "ro", "*", "0-1"))
    .collect();
    mounts.extend([
        mount("/", "virtiofs", "type", "ro", "*"),
        mount("/proc", "proc", "type", "*", "*"),
        mount("/sys", "sysfs", "type", "*", "*"),
        mount("/dev", "devtmpfs", "type", "*", "*"),
        mount(
            "/work",
            "virtiofs",
            &format!("tag:work:{project_ino}"),
            "rw",
            "nosuid,nodev",
        ),
    ]);
    serde_json::json!({
        "proxy_port": "3128",
        "door_port": DOOR_PORT.to_string(),
        "report_port": REPORT_PORT.to_string(),
        "modules_dir": GUEST_MODULES,
        "modules": spec.extra_modules,
        "work_tag": "work",
        "env": {
            "PATH": "/usr/local/bin:/usr/bin:/bin",
            "HOME": "/work",
            "HTTP_PROXY": "http://127.0.0.1:3128",
        },
        "pins": {
            "pins": [{"path": "/work", "dev": "*", "ino": project_ino.to_string()}],
            "mounts": mounts,
        },
        "command": spec.command,
    })
}

/// Startet den Gast, spielt Tür und Empfänger des Berichts und wartet auf
/// sein Ende.
pub(crate) fn run(spec: &Spec) -> Result<Outcome, String> {
    let mut vm = Vm {
        dir: runtime_dir()?,
        children: Vec::new(),
    };
    let project_ino = std::fs::metadata(&spec.project)
        .map_err(|err| format!("project {}: {err}", spec.project.display()))?
        .ino();
    let plan = vm.path("guest-plan.json");
    std::fs::write(&plan, guest_plan(spec, project_ino).to_string())
        .map_err(|err| format!("guest plan: {err}"))?;

    start_root(&mut vm, spec, &plan)?;
    start_work(&mut vm, spec)?;
    // Die Tür: der Test spielt den Proxy.
    let door = Door::listen(&vm.path(&format!("door_{DOOR_PORT}")))?;
    start_device(&mut vm, &spec.resolved)?;
    for socket in ["work.sock", "vsock.sock"] {
        wait_for(&vm.path(socket), Duration::from_secs(10))?;
    }
    let started = Instant::now();
    start_machine(&mut vm, spec)?;

    // Der Bericht: so lange nachfragen, bis der Shim lauscht, dann bis zum
    // Ende lesen; endet der Gast vorher, gibt es keinen.
    let device = vm.path("door");
    let mut report = read_report(&device, started + RUN_TIMEOUT, &mut || vm.emulator_ended());
    let exit = wait_emulator(&mut vm, started + RUN_TIMEOUT);
    if report.is_empty() {
        // Ohne Bericht ist nichts belegt: Das ist eine gescheiterte Prüfung,
        // nie ein stilles Nichts. So endet ein Gast, dessen vsock-Module
        // nicht laden; der Shim hat dann keinen Weg zum Host (HUM-465).
        report.push(NO_REPORT.to_owned());
    }
    let helpers = vm.stderr_of_all();
    let console = spec
        .console
        .then(|| std::fs::read_to_string(vm.path("console.log")).unwrap_or_default());
    Ok(Outcome {
        report,
        door: door.requests(),
        exit,
        console,
        helpers,
    })
}

/// Die Wurzel: der Socket entsteht hier und kommt als Deskriptor 3, das
/// Laufzeitverzeichnis steht nicht im Gefängnis.
fn start_root(vm: &mut Vm, spec: &Spec, plan: &Path) -> Result<(), String> {
    let resolved = &spec.resolved;
    let listener =
        UnixListener::bind(vm.path("root.sock")).map_err(|err| format!("root.sock: {err}"))?;
    let listener = crate::inherit::above_stdio(OwnedFd::from(listener))
        .map_err(|err| format!("root.sock: {err}"))?;
    let mut root = jail(&resolved.bubblewrap);
    root.args(["--tmpfs", "/tmp", "--dir", "/work", "--dir", "/sys"]);
    root.arg("--ro-bind").arg(&spec.shim).arg(GUEST_SHIM);
    root.arg("--ro-bind").arg(plan).arg(GUEST_PLAN);
    root.arg("--ro-bind")
        .arg(&resolved.modules)
        .arg(GUEST_MODULES);
    bind_program(&mut root, &resolved.file_daemon);
    root.arg(&resolved.file_daemon).args([
        "--shared-dir",
        "/",
        "--fd",
        "3",
        "--sandbox",
        "none",
        "--readonly",
        "--log-level",
        "error",
    ]);
    let placeholders = crate::inherit::inherit_renumbered(&mut root, &[listener.as_fd()])
        .map_err(|err| format!("root fd: {err}"))?;
    vm.spawn("root file daemon", root)?;
    drop(placeholders);
    Ok(())
}

/// `/work`: der Ordner ist geöffnet, bevor der Dienst startet, und kommt
/// als Deskriptor (`--bind-fd`), nicht als Pfad.
fn start_work(vm: &mut Vm, spec: &Spec) -> Result<(), String> {
    let resolved = &spec.resolved;
    let project = std::fs::File::open(&spec.project)
        .map_err(|err| format!("project {}: {err}", spec.project.display()))?;
    let project = crate::inherit::above_stdio(OwnedFd::from(project))
        .map_err(|err| format!("project fd: {err}"))?;
    let mut work = jail(&resolved.bubblewrap);
    work.arg("--bind").arg(vm.dir.path()).arg(vm.dir.path());
    work.arg("--bind-fd")
        .arg(project.as_raw_fd().to_string())
        .arg("/share");
    bind_program(&mut work, &resolved.file_daemon);
    work.arg(&resolved.file_daemon)
        .args(["--shared-dir", "/share", "--socket-path"])
        .arg(vm.path("work.sock"))
        .args(["--sandbox", "none", "--log-level", "error"]);
    crate::inherit::inherit_in_child(&mut work, &[project.as_fd()])
        .map_err(|err| format!("project fd: {err}"))?;
    vm.spawn("work file daemon", work)
}

/// Das vsock-Gerät, ein Prozess nur für diesen Gast, mit eigener Gruppe.
fn start_device(vm: &mut Vm, resolved: &Resolved) -> Result<(), String> {
    let mut device = jail(&resolved.bubblewrap);
    device.arg("--bind").arg(vm.dir.path()).arg(vm.dir.path());
    bind_program(&mut device, &resolved.vsock_device);
    device
        .arg(&resolved.vsock_device)
        .args(["--guest-cid", &GUEST_CID.to_string(), "--socket"])
        .arg(vm.path("vsock.sock"))
        .arg("--uds-path")
        .arg(vm.path("door"))
        .args(["--groups", &format!("hum{}", std::process::id())]);
    vm.spawn("vsock device", device)
}

/// Die Maschine: `/dev/kvm` und die Kernel-Datei, sonst nichts vom Host;
/// keine Konsole, außer zur Fehlersuche ([`Spec::console`]).
fn start_machine(vm: &mut Vm, spec: &Spec) -> Result<(), String> {
    let resolved = &spec.resolved;
    let (quiet, serial) = if spec.console {
        (
            "console=ttyS0",
            format!("file:{}", vm.path("console.log").display()),
        )
    } else {
        ("quiet", "none".to_owned())
    };
    let append = format!(
        "rootfstype=virtiofs root=rootfs ro panic=-1 {quiet} init={GUEST_SHIM} -- --guest \
         {GUEST_PLAN}"
    );
    let mut machine = jail(&resolved.bubblewrap);
    machine.arg("--bind").arg(vm.dir.path()).arg(vm.dir.path());
    machine.args(["--dev-bind", "/dev/kvm", "/dev/kvm"]);
    machine
        .arg("--ro-bind")
        .arg(&resolved.kernel)
        .arg("/kernel");
    bind_program(&mut machine, &resolved.emulator);
    machine.arg(&resolved.emulator).args([
        "-M",
        "microvm,pcie=on,acpi=on",
        "-accel",
        "kvm",
        "-cpu",
        "host",
        "-smp",
        "1",
        "-m",
        "512",
        "-no-reboot",
        "-nodefaults",
        "-no-user-config",
        "-display",
        "none",
        "-serial",
        &serial,
        "-monitor",
        "none",
        "-nic",
        "none",
        "-sandbox",
        "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny",
        "-object",
        "memory-backend-memfd,id=mem,size=512M,share=on",
        "-machine",
        "memory-backend=mem",
        "-kernel",
        "/kernel",
        "-append",
        &append,
    ]);
    for (id, socket, device) in [
        (
            "fs0",
            "root.sock",
            "vhost-user-fs-pci,chardev=fs0,tag=rootfs",
        ),
        ("fs1", "work.sock", "vhost-user-fs-pci,chardev=fs1,tag=work"),
        ("vs", "vsock.sock", "vhost-user-vsock-pci,chardev=vs"),
    ] {
        machine
            .arg("-chardev")
            .arg(format!("socket,id={id},path={}", vm.path(socket).display()))
            .args(["-device", device]);
    }
    vm.spawn("emulator", machine)
}

/// Wartet, bis `path` existiert.
fn wait_for(path: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        if Instant::now() >= deadline {
            return Err(format!("{} did not appear", path.display()));
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// Wartet auf das Ende des Emulators, höchstens bis `deadline`.
fn wait_emulator(vm: &mut Vm, deadline: Instant) -> Option<ExitStatus> {
    let (_, emulator) = vm
        .children
        .iter_mut()
        .find(|(name, _)| name == "emulator")?;
    loop {
        if let Ok(Some(status)) = emulator.try_wait() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Holt den Bericht über das vsock-Gerät: `CONNECT <port>`, `OK`, dann jede
/// Zeile bis zum Ende. Antwortet der Gast noch nicht, wird es wiederholt.
fn read_report(device: &Path, deadline: Instant, ended: &mut dyn FnMut() -> bool) -> Vec<String> {
    while Instant::now() < deadline && !ended() {
        let Ok(mut stream) = UnixStream::connect(device) else {
            thread::sleep(Duration::from_millis(100));
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        if stream
            .write_all(format!("CONNECT {REPORT_PORT}\n").as_bytes())
            .is_err()
        {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        let mut reader = BufReader::new(stream);
        let mut first = String::new();
        if reader.read_line(&mut first).is_err() || !first.starts_with("OK") {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let _ = reader
            .get_ref()
            .set_read_timeout(Some(remaining.max(Duration::from_secs(1))));
        let mut lines = Vec::new();
        for line in reader.by_ref().lines() {
            match line {
                Ok(line) => lines.push(line),
                Err(_) => break,
            }
        }
        // Das Ende des Lesens schließt die Verbindung; der Shim wartet
        // darauf, bevor er den Gast ausschaltet.
        return lines;
    }
    Vec::new()
}

/// Der Lauscher an `<uds>_<Tür-Port>`: nimmt jede Anfrage an, merkt sich die
/// erste Zeile und antwortet mit `door-ok-<n>`.
struct Door {
    seen: Arc<Mutex<Vec<String>>>,
}

impl Door {
    fn listen(path: &Path) -> Result<Self, String> {
        let listener = UnixListener::bind(path).map_err(|err| format!("door: {err}"))?;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        thread::spawn(move || {
            for (number, stream) in (1..).zip(listener.incoming()) {
                let Ok(stream) = stream else { break };
                let record = Arc::clone(&record);
                thread::spawn(move || answer(stream, number, &record));
            }
        });
        Ok(Self { seen })
    }

    fn requests(&self) -> Vec<String> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }
}

/// Liest eine Anfrage bis zur Leerzeile und antwortet.
fn answer(stream: UnixStream, number: u32, seen: &Mutex<Vec<String>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) if line.trim().is_empty() => break,
            Ok(_) => {}
        }
    }
    if let Ok(mut seen) = seen.lock() {
        seen.push(first.trim_end().to_owned());
    }
    let body = format!("door-ok-{number}");
    let mut writer = writer;
    let _ = writer.write_all(
        format!(
            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
    let _ = writer.shutdown(std::net::Shutdown::Write);
}
