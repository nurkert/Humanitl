//! Was die Kernel-Stufe auf dem Rechner braucht, bevor ein Start sie auch nur
//! versuchen darf (HUM-464): `/dev/kvm`, vier Programme mit den Fähigkeiten,
//! die der Adapter nutzt, ein lesbares Kernel-Abbild und das Verzeichnis
//! seiner Module.
//!
//! Die Fähigkeiten kommen aus der Hilfe des Programms, nicht aus einer
//! Versionsnummer: Debian, Fedora und Arch bauen dieselbe Fassung mit
//! verschiedenen Optionen, und eine Option, die in der Hilfe steht, gibt es.
//! Gefragt wird mit leerer Umgebung, ohne Eingabe und mit Frist; gestartet
//! wird nichts, was eine VM oder ein Gerät anlegt.
//!
//! Dazu kommt seit HUM-444 die Konfiguration des Kernels: Er muss virtiofs
//! eingebaut haben (`CONFIG_VIRTIO_FS=y`), sonst bootet er ohne initramfs
//! keine virtiofs-Wurzel (HUM-465, Ubuntus `linux-azure`). Gelesen wird die
//! Datei neben dem Abbild (`config-<release>` neben `vmlinuz-<release>`,
//! `config` neben `vmlinuz`); fehlt sie, ist das ein Befund mit Weg, kein
//! Bestehen.
//!
//! Der erste fehlende Punkt wird `SANDBOX_045` mit dem Fehlenden in `why`
//! und dem Paket, das es bringt, in `fix`. Die Befehle sind feste
//! Zeichenketten je Paketverwalter ([`Package::install`]); kein Text vom
//! Rechner wird Teil eines Befehls, außer einem Pfad, der zitiert wird.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths, QemuConfig};
use humanitl_core::diagnostics::codes::SANDBOX_045;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::os_release::PackageManager;

/// Der Suchpfad, wenn die Umgebung keinen hat; derselbe wie bei bwrap.
const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// So lange darf ein Programm für seine Hilfe brauchen.
const HELP_TIMEOUT: Duration = Duration::from_secs(5);

/// So viel einer Hilfe wird gelesen; die längste, die von QEMU zu allen
/// Geräten, hat rund 30 KiB.
const MAX_HELP_BYTES: u64 = 1024 * 1024;

/// Die Orte des Rechners, an denen die Prüfung nachsieht. Im Betrieb die
/// festen des Systems, in den Tests ein Ordner.
#[derive(Debug, Clone)]
struct Host {
    /// Das Gerät der Hardware-Virtualisierung.
    kvm: PathBuf,
    /// Wo die Kernel-Abbilder liegen (`vmlinuz-<release>`).
    boot: PathBuf,
    /// Wo die Module je Kernel liegen (`<release>/`).
    modules_root: PathBuf,
    /// Wo Debian Dienste ablegt, die nicht im `PATH` stehen.
    libexec: PathBuf,
    /// Die Fassung des laufenden Kernels, wie `uname -r` sie nennt.
    release: Option<String>,
}

impl Host {
    /// Die Orte dieses Rechners.
    fn system() -> Self {
        Self {
            kvm: PathBuf::from("/dev/kvm"),
            boot: PathBuf::from("/boot"),
            modules_root: PathBuf::from("/usr/lib/modules"),
            libexec: PathBuf::from("/usr/libexec"),
            release: std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .ok()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty()),
        }
    }
}

/// Ein Paket, das ein Stück der Kernel-Stufe bringt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Package {
    /// Der Emulator der Maschine.
    Emulator,
    /// Der Dienst für die Dateisysteme des Gasts.
    FileDaemon,
    /// Das vsock-Gerät der Tür.
    VsockDevice,
    /// bubblewrap, das jeden Hilfsprozess einsperrt.
    Bubblewrap,
    /// Der Kernel samt Modulen.
    Kernel,
}

impl Package {
    /// Der Befehl, der das Paket mit diesem Paketverwalter installiert. Eine
    /// Auswahl fester Zeichenketten, wie in [`crate::os_release`].
    const fn install(self, manager: PackageManager) -> &'static str {
        match (self, manager) {
            (Self::Bubblewrap, manager) => manager.install_bubblewrap(),
            (Self::Emulator, PackageManager::Apt) => "sudo apt install qemu-system-x86",
            (Self::Emulator, PackageManager::Dnf) => "sudo dnf install qemu-system-x86-core",
            (Self::Emulator, PackageManager::Pacman) => "sudo pacman -S qemu-system-x86",
            (Self::Emulator, PackageManager::Zypper) => "sudo zypper install qemu-x86",
            (Self::FileDaemon, PackageManager::Apt) => "sudo apt install virtiofsd",
            (Self::FileDaemon, PackageManager::Dnf) => "sudo dnf install virtiofsd",
            (Self::FileDaemon, PackageManager::Pacman) => "sudo pacman -S virtiofsd",
            (Self::FileDaemon, PackageManager::Zypper) => "sudo zypper install virtiofsd",
            (Self::VsockDevice, PackageManager::Apt) => "sudo apt install vhost-device-vsock",
            (Self::VsockDevice, PackageManager::Dnf) => "sudo dnf install vhost-device-vsock",
            (Self::VsockDevice, PackageManager::Pacman) => "sudo pacman -S vhost-device-vsock",
            (Self::VsockDevice, PackageManager::Zypper) => "sudo zypper install vhost-device-vsock",
            (Self::Kernel, PackageManager::Apt) => "sudo apt install linux-image-amd64",
            (Self::Kernel, PackageManager::Dnf) => "sudo dnf install kernel-core",
            (Self::Kernel, PackageManager::Pacman) => "sudo pacman -S linux",
            (Self::Kernel, PackageManager::Zypper) => "sudo zypper install kernel-default",
        }
    }
}

/// Eine Frage an ein Programm und die Wörter, die seine Antwort nennen muss.
#[derive(Debug)]
struct Question {
    /// Die Argumente, mit denen das Programm seine Hilfe schreibt.
    args: &'static [&'static str],
    /// Was in der Hilfe als eigenes Wort stehen muss.
    words: &'static [&'static str],
}

/// Ein Programm der Kernel-Stufe.
#[derive(Debug)]
struct Program {
    /// Der Name im `PATH`.
    name: &'static str,
    /// Der Schlüssel, der einen Pfad dafür setzt, falls es einen gibt.
    key: Option<&'static str>,
    /// Ob der Dienst auch unter `libexec` liegen darf, wie unter Debian.
    in_libexec: bool,
    /// Das Paket, das es bringt.
    package: Package,
    /// Was es können muss.
    questions: &'static [Question],
}

/// Der Emulator: die Maschine `microvm` und das vsock-Gerät über vhost-user.
const EMULATOR: Program = Program {
    name: "qemu-system-x86_64",
    key: Some("sandbox.qemu.qemu"),
    in_libexec: false,
    package: Package::Emulator,
    questions: &[
        Question {
            args: &["-machine", "help"],
            words: &["microvm"],
        },
        Question {
            args: &["-device", "help"],
            words: &["vhost-user-vsock-pci"],
        },
    ],
};

/// Das Wort, an dem die Hilfe von virtiofsd zeigt, dass er die Ankündigung
/// von Einhängepunkten abschalten kann (ab 1.11, HUM-444).
const NO_ANNOUNCE_SUBMOUNTS: &str = "--no-announce-submounts";

/// Die Zeile der Kernel-Konfiguration, ohne die die Stufe nicht startet.
const VIRTIO_FS_BUILT_IN: &str = "CONFIG_VIRTIO_FS=y";

/// So viel einer Kernel-Konfiguration wird gelesen; Debians hat rund 300 KiB.
const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;

/// Der Dienst der Dateisysteme: nur lesbar ausliefern, Socket als Deskriptor.
const FILE_DAEMON: Program = Program {
    name: "virtiofsd",
    key: Some("sandbox.qemu.virtiofsd"),
    in_libexec: true,
    package: Package::FileDaemon,
    questions: &[Question {
        args: &["--help"],
        words: &["--readonly", "--fd"],
    }],
};

/// Das vsock-Gerät: Tür als Unix-Socket, eigene Gruppe je Sitzung.
const VSOCK_DEVICE: Program = Program {
    name: "vhost-device-vsock",
    key: Some("sandbox.qemu.vhost_device_vsock"),
    in_libexec: false,
    package: Package::VsockDevice,
    questions: &[Question {
        args: &["--help"],
        words: &["--uds-path", "--groups"],
    }],
};

/// bubblewrap: der geprüfte Projektordner kommt als Deskriptor (HUM-437).
const BUBBLEWRAP: Program = Program {
    name: "bwrap",
    key: None,
    in_libexec: false,
    package: Package::Bubblewrap,
    questions: &[Question {
        args: &["--help"],
        words: &["--bind-fd"],
    }],
};

/// Die erste fehlende Voraussetzung der Kernel-Stufe auf diesem Rechner als
/// `SANDBOX_045`, oder keine.
///
/// `paths` liefert den `PATH` des Daemons, `config` die Gruppe
/// `sandbox.qemu` mit den Pfaden, die die Suche ersetzen.
pub(crate) fn missing_prerequisite(paths: &Paths, config: &QemuConfig) -> Option<Diagnostic> {
    resolve(paths.env(), config).err()
}

/// Was die Prüfung gefunden hat: je Voraussetzung der Pfad, den ein Start
/// benutzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
    /// Der Emulator.
    pub(crate) emulator: PathBuf,
    /// Der Dienst der Dateisysteme.
    pub(crate) file_daemon: PathBuf,
    /// Das vsock-Gerät.
    pub(crate) vsock_device: PathBuf,
    /// bubblewrap.
    pub(crate) bubblewrap: PathBuf,
    /// Das Kernel-Abbild.
    pub(crate) kernel: PathBuf,
    /// Das Verzeichnis seiner Module.
    pub(crate) modules: PathBuf,
    /// Ob virtiofsd `--no-announce-submounts` kennt (HUM-444). Ohne die
    /// Option kündigt eine Fassung vor 1.11 ohnehin nichts an.
    pub(crate) no_announce_submounts: bool,
}

/// Jede Voraussetzung mit ihrem Pfad, oder die erste fehlende als
/// `SANDBOX_045`; `env` liefert den `PATH`.
pub(crate) fn resolve(env: &Env, config: &QemuConfig) -> Result<Resolved, Diagnostic> {
    missing_on(&Host::system(), env, config, PackageManager::detect())
}

/// [`resolve`] auf diesen Orten, in dieser Reihenfolge: `/dev/kvm`, die
/// Programme samt Fähigkeiten, das Kernel-Abbild, die Module.
fn missing_on(
    host: &Host,
    env: &Env,
    config: &QemuConfig,
    manager: PackageManager,
) -> Result<Resolved, Diagnostic> {
    kvm_usable(host)?;
    let usable = |program: &Program, configured: Option<&Path>| {
        let found = find(program, configured, env, host, manager)?;
        for question in program.questions {
            ask(program, &found, question, manager)?;
        }
        Ok::<_, Diagnostic>(found)
    };
    let emulator = usable(&EMULATOR, config.qemu.as_deref())?;
    let file_daemon = usable(&FILE_DAEMON, config.virtiofsd.as_deref())?;
    let vsock_device = usable(&VSOCK_DEVICE, config.vhost_device_vsock.as_deref())?;
    let bubblewrap = usable(&BUBBLEWRAP, None)?;
    let kernel = kernel_readable(host, config, manager)?;
    virtio_fs_built_in(&kernel)?;
    Ok(Resolved {
        no_announce_submounts: help_text(&file_daemon, &["--help"])
            .is_some_and(|help| names_word(&help, NO_ANNOUNCE_SUBMOUNTS)),
        emulator,
        file_daemon,
        vsock_device,
        bubblewrap,
        kernel,
        modules: modules_present(host, config, manager)?,
    })
}

/// Eine Voraussetzung der Kernel-Stufe als Zeile des Doctors (HUM-444).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prerequisite {
    /// `/dev/kvm`.
    Kvm,
    /// Der Emulator mit `microvm` und dem vsock-Gerät.
    Emulator,
    /// Der Dienst der Dateisysteme.
    FileDaemon,
    /// Das vsock-Gerät.
    VsockDevice,
    /// bubblewrap mit `--bind-fd`.
    Bubblewrap,
    /// Das Kernel-Abbild.
    Kernel,
    /// virtiofs im Kernel eingebaut.
    VirtioFs,
    /// Das Verzeichnis der Module.
    Modules,
}

/// Jede Voraussetzung für sich, in der Reihenfolge der Prüfung, mit dem Beleg
/// oder dem Befund; anders als [`resolve`] hört es beim ersten Fehlenden
/// nicht auf. `env` liefert den `PATH`.
pub(crate) fn each_prerequisite(
    env: &Env,
    config: &QemuConfig,
) -> Vec<(Prerequisite, Result<String, Diagnostic>)> {
    each_on(&Host::system(), env, config, PackageManager::detect())
}

/// [`each_prerequisite`] auf diesen Orten.
fn each_on(
    host: &Host,
    env: &Env,
    config: &QemuConfig,
    manager: PackageManager,
) -> Vec<(Prerequisite, Result<String, Diagnostic>)> {
    let usable = |program: &Program, configured: Option<&Path>| {
        let found = find(program, configured, env, host, manager)?;
        for question in program.questions {
            ask(program, &found, question, manager)?;
        }
        Ok::<_, Diagnostic>(found.display().to_string())
    };
    let kernel = kernel_readable(host, config, manager);
    let virtio_fs = match &kernel {
        Ok(image) => virtio_fs_built_in(image).map(|()| VIRTIO_FS_BUILT_IN.to_owned()),
        Err(missing) => Err(missing.clone()),
    };
    vec![
        (
            Prerequisite::Kvm,
            kvm_usable(host).map(|()| host.kvm.display().to_string()),
        ),
        (
            Prerequisite::Emulator,
            usable(&EMULATOR, config.qemu.as_deref()),
        ),
        (
            Prerequisite::FileDaemon,
            usable(&FILE_DAEMON, config.virtiofsd.as_deref()),
        ),
        (
            Prerequisite::VsockDevice,
            usable(&VSOCK_DEVICE, config.vhost_device_vsock.as_deref()),
        ),
        (Prerequisite::Bubblewrap, usable(&BUBBLEWRAP, None)),
        (
            Prerequisite::Kernel,
            kernel.map(|image| image.display().to_string()),
        ),
        (Prerequisite::VirtioFs, virtio_fs),
        (
            Prerequisite::Modules,
            modules_present(host, config, manager).map(|dir| dir.display().to_string()),
        ),
    ]
}

/// Die Konfiguration neben dem Abbild: `config-<rest>` neben
/// `vmlinuz-<rest>`, `config` neben `vmlinuz`; sonst keine.
fn config_beside(image: &Path) -> Option<PathBuf> {
    let name = image.file_name()?.to_str()?;
    let config = if name == "vmlinuz" {
        "config".to_owned()
    } else {
        format!("config-{}", name.strip_prefix("vmlinuz-")?)
    };
    Some(image.with_file_name(config))
}

/// Der Kernel hat virtiofs eingebaut: Seine Konfiguration neben dem Abbild
/// trägt [`VIRTIO_FS_BUILT_IN`]. Ein Modul (`=m`), eine fehlende Zeile und
/// eine Konfiguration, die sich nicht lesen lässt, sind ein Befund.
fn virtio_fs_built_in(image: &Path) -> Result<(), Diagnostic> {
    let fix = || FixAction::ChangeSetting {
        key: "sandbox.qemu.kernel".to_owned(),
        value: String::new(),
    };
    let Some(config) = config_beside(image) else {
        return Err(missing(
            &format!(
                "the configuration of the kernel {} to see that virtiofs is built in, and its \
                name is neither vmlinuz nor vmlinuz-<release>, so there is no config beside \
                it to read; name a kernel with its config-<release> beside it in \
                sandbox.qemu.kernel",
                image.display()
            ),
            fix(),
        ));
    };
    let text = std::fs::File::open(&config).and_then(|file| {
        let mut text = String::new();
        file.take(MAX_CONFIG_BYTES).read_to_string(&mut text)?;
        Ok(text)
    });
    let text = match text {
        Ok(text) => text,
        Err(err) => {
            return Err(missing(
                &format!(
                    "the configuration of the kernel to see that virtiofs is built in, and {} \
                    cannot be read ({err}); without it nothing says the guest can boot from \
                    its root. Name a kernel with its configuration beside it in \
                    sandbox.qemu.kernel",
                    config.display()
                ),
                fix(),
            ));
        }
    };
    if text.lines().any(|line| line.trim() == VIRTIO_FS_BUILT_IN) {
        return Ok(());
    }
    let found = text
        .lines()
        .find(|line| line.starts_with("CONFIG_VIRTIO_FS=") || line.contains("CONFIG_VIRTIO_FS "))
        .map_or_else(|| "no CONFIG_VIRTIO_FS line".to_owned(), str::to_owned);
    Err(missing(
        &format!(
            "a kernel with virtiofs built in ({VIRTIO_FS_BUILT_IN}), and {} says {found:?}; such \
            a kernel boots no virtiofs root without an initramfs. Name another kernel in \
            sandbox.qemu.kernel",
            config.display()
        ),
        fix(),
    ))
}

/// `SANDBOX_045` mit dem, was fehlt, und dem Weg dorthin.
fn missing(what: &str, fix: FixAction) -> Diagnostic {
    Diagnostic::builder(SANDBOX_045, Severity::Blocking)
        .why(format!(
            "the kernel level needs {what}; nothing starts, and no other backend takes over. \
             Start with --backend bwrap for the shared kernel"
        ))
        .fix(fix)
        .build()
}

/// `/dev/kvm` gibt es, und dieser Nutzer darf es lesen und schreiben.
fn kvm_usable(host: &Host) -> Result<(), Diagnostic> {
    let device = host.kvm.display();
    if !host.kvm.exists() {
        return Err(missing(
            &format!(
                "{device} for hardware virtualization, and there is none: virtualization is off \
                 in the firmware, or the module of the processor is not loaded"
            ),
            FixAction::CopyCommand("sudo modprobe kvm_intel || sudo modprobe kvm_amd".to_owned()),
        ));
    }
    let access = rustix::fs::Access::READ_OK | rustix::fs::Access::WRITE_OK;
    if let Err(err) = rustix::fs::access(&host.kvm, access) {
        return Err(missing(
            &format!(
                "{device} readable and writable for this user, and it is not ({err}); after \
                 joining the group kvm, log in again"
            ),
            FixAction::CopyCommand("sudo usermod -aG kvm \"$USER\"".to_owned()),
        ));
    }
    Ok(())
}

/// Das Programm: aus seinem Schlüssel, sonst das erste im `PATH`, beim
/// Dienst der Dateisysteme danach unter `libexec`.
fn find(
    program: &Program,
    configured: Option<&Path>,
    env: &Env,
    host: &Host,
    manager: PackageManager,
) -> Result<PathBuf, Diagnostic> {
    let install = || FixAction::CopyCommand(program.package.install(manager).to_owned());
    if let Some(path) = configured {
        if crate::agent::is_executable_file(path) {
            return Ok(path.to_path_buf());
        }
        let key = program.key.unwrap_or(program.name);
        return Err(missing(
            &format!(
                "the program {}, and {key} = {:?} is not an executable file",
                program.name,
                path.display().to_string()
            ),
            install(),
        ));
    }
    let path = env.non_empty("PATH").unwrap_or(DEFAULT_PATH);
    let libexec = program.in_libexec.then(|| host.libexec.join(program.name));
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(program.name))
        .chain(libexec)
        .find(|candidate| crate::agent::is_executable_file(candidate))
        .ok_or_else(|| {
            let looked = if program.in_libexec {
                format!("PATH={path} or {}", host.libexec.display())
            } else {
                format!("PATH={path}")
            };
            missing(
                &format!(
                    "the program {}, and there is no executable one in {looked}",
                    program.name
                ),
                install(),
            )
        })
}

/// Stellt dem Programm eine Frage und prüft, dass die Antwort jedes Wort nennt.
fn ask(
    program: &Program,
    found: &Path,
    question: &Question,
    manager: PackageManager,
) -> Result<(), Diagnostic> {
    let asked = question.args.join(" ");
    let fix = || FixAction::CopyCommand(program.package.install(manager).to_owned());
    let Some(answer) = help_text(found, question.args) else {
        return Err(missing(
            &format!(
                "{} to answer `{asked}`, and it did not within {} s",
                found.display(),
                HELP_TIMEOUT.as_secs()
            ),
            fix(),
        ));
    };
    if let Some(word) = question
        .words
        .iter()
        .find(|word| !names_word(&answer, word))
    {
        return Err(missing(
            &format!(
                "{word} from {}, and its answer to `{asked}` does not name it; the installed \
                 version is too old or built without it",
                found.display()
            ),
            fix(),
        ));
    }
    Ok(())
}

/// Ob `word` in `text` als eigenes Wort steht: `--fd` nicht als Teil von
/// `--bind-fd`, `microvm` nicht als Teil von `microvm-legacy`.
fn names_word(text: &str, word: &str) -> bool {
    text.split(|c: char| {
        c.is_whitespace() || matches!(c, ',' | '=' | '[' | ']' | '<' | '>' | '(' | ')' | '|' | '"')
    })
    .any(|token| token == word)
}

/// Die Ausgabe von `program args` auf beiden Kanälen, oder nichts, wenn es
/// sich nicht starten ließ oder die Frist überschritt. Leere Umgebung, keine
/// Eingabe.
///
/// Das Programm startet als Anführer einer eigenen Prozessgruppe, und nach
/// der Frist bekommt die ganze Gruppe `SIGKILL`, bevor der Anführer
/// eingesammelt wird: Ein Helfer, der sich forkt, ließe sonst Kinder zurück,
/// die die Kanäle offen und die Fäden der Leser am Leben hielten. Mit der
/// Gruppe enden auch die Kanäle, und die Leser kommen zum Ende.
fn help_text(program: &Path, args: &[&str]) -> Option<String> {
    use std::os::unix::process::CommandExt as _;
    let mut child = Command::new(program)
        .args(args)
        .process_group(0)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let stdout = read_in_background(child.stdout.take());
    let stderr = read_in_background(child.stderr.take());
    let group = i32::try_from(child.id())
        .ok()
        .and_then(rustix::process::Pid::from_raw);
    let deadline = Instant::now() + HELP_TIMEOUT;
    // Das Ende des Anführers wird erkannt, ohne ihn einzusammeln (`WNOWAIT`):
    // Solange er nicht eingesammelt ist, gehört die Kennung der Gruppe noch
    // ihm, und ein Kind, das er hinterlassen hat, lässt sich mit der Gruppe
    // beenden, ob er von selbst endete oder die Frist ablief.
    let ended = loop {
        let exited = group.is_some_and(|pid| {
            rustix::process::waitid(
                rustix::process::WaitId::Pid(pid),
                rustix::process::WaitIdOptions::EXITED
                    | rustix::process::WaitIdOptions::NOHANG
                    | rustix::process::WaitIdOptions::NOWAIT,
            )
            .is_ok_and(|status| status.is_some())
        });
        if exited {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if let Some(group) = group {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    if !ended {
        let _ = child.kill();
    }
    let _ = child.wait();
    if !ended {
        return None;
    }
    let grace = Duration::from_secs(1);
    let mut text = stdout.recv_timeout(grace).ok()?;
    text.push('\n');
    text.push_str(&stderr.recv_timeout(grace).ok()?);
    Some(text)
}

/// Liest einen Kanal in einem eigenen Faden bis zu seinem Ende, höchstens
/// [`MAX_HELP_BYTES`].
fn read_in_background<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = pipe {
            let _ = pipe.take(MAX_HELP_BYTES).read_to_end(&mut bytes);
        }
        let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receiver
}

/// Das Kernel-Abbild: aus `sandbox.qemu.kernel`, sonst das des laufenden
/// Kernels unter `/boot`; es muss eine Datei sein, die dieser Nutzer lesen
/// kann. Ubuntu legt sie mit `0600` ab.
fn kernel_readable(
    host: &Host,
    config: &QemuConfig,
    manager: PackageManager,
) -> Result<PathBuf, Diagnostic> {
    let image = match (&config.kernel, &host.release) {
        (Some(image), _) => image.clone(),
        (None, Some(release)) => host.boot.join(format!("vmlinuz-{release}")),
        (None, None) => return Err(release_unknown("sandbox.qemu.kernel")),
    };
    if image.is_file() && std::fs::File::open(&image).is_ok() {
        return Ok(image);
    }
    let shown = image.display();
    if image.is_file() {
        return Err(missing(
            &format!(
                "a kernel image this user can read, and {shown} is not readable; make it \
                 readable, or name a readable copy in sandbox.qemu.kernel"
            ),
            FixAction::CopyCommand(format!(
                "sudo chmod 0644 {}",
                humanitl_core::shell::shell_path(&image)
            )),
        ));
    }
    Err(missing(
        &format!("a kernel image, and there is no file at {shown}"),
        FixAction::CopyCommand(Package::Kernel.install(manager).to_owned()),
    ))
}

/// Das Verzeichnis der Module des Kernels: aus `sandbox.qemu.modules`, sonst
/// das des laufenden Kernels unter `/usr/lib/modules`.
fn modules_present(
    host: &Host,
    config: &QemuConfig,
    manager: PackageManager,
) -> Result<PathBuf, Diagnostic> {
    let dir = match (&config.modules, &host.release) {
        (Some(dir), _) => dir.clone(),
        (None, Some(release)) => host.modules_root.join(release),
        (None, None) => return Err(release_unknown("sandbox.qemu.modules")),
    };
    if dir.is_dir() {
        return Ok(dir);
    }
    Err(missing(
        &format!(
            "the module directory of the kernel, and there is no directory at {}",
            dir.display()
        ),
        FixAction::CopyCommand(Package::Kernel.install(manager).to_owned()),
    ))
}

/// Die Fassung des laufenden Kernels ist unbekannt, und `key` ist leer.
fn release_unknown(key: &str) -> Diagnostic {
    missing(
        &format!(
            "the release of the running kernel to find its files, and \
             /proc/sys/kernel/osrelease is unreadable; name the path in {key}"
        ),
        FixAction::ChangeSetting {
            key: key.to_owned(),
            value: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Ein Rechner in einem Ordner: Gerät, Kernel und Module da, die vier
    /// Programme als Attrappen im `PATH`, deren Hilfe alles nennt.
    struct Fixture {
        dir: tempfile::TempDir,
        host: Host,
        env: Env,
    }

    const RELEASE: &str = "7.3.4+deb14-amd64";

    impl Fixture {
        fn complete() -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let bin = dir.path().join("bin");
            std::fs::create_dir_all(&bin).expect("bin");
            let host = Host {
                kvm: dir.path().join("kvm"),
                boot: dir.path().join("boot"),
                modules_root: dir.path().join("modules"),
                libexec: dir.path().join("libexec"),
                release: Some(RELEASE.to_owned()),
            };
            std::fs::write(&host.kvm, "").expect("kvm");
            std::fs::create_dir_all(&host.boot).expect("boot");
            std::fs::write(host.boot.join(format!("vmlinuz-{RELEASE}")), "k").expect("kernel");
            std::fs::write(
                host.boot.join(format!("config-{RELEASE}")),
                "CONFIG_VIRTIOFS=x\nCONFIG_VIRTIO_FS=y\n",
            )
            .expect("config");
            std::fs::create_dir_all(host.modules_root.join(RELEASE)).expect("modules");
            std::fs::create_dir_all(&host.libexec).expect("libexec");
            let fixture = Self {
                env: Env::from_pairs([("PATH", bin.to_string_lossy().into_owned())]),
                dir,
                host,
            };
            fixture.program(
                "qemu-system-x86_64",
                "case \"$1\" in\n\
                 -machine) printf 'Supported machines are:\\nmicrovm              microvm (i386)\\npc    Standard PC\\n' ;;\n\
                 -device) printf 'name \"vhost-user-vsock-pci\", bus PCI\\n' ;;\n\
                 esac\n",
            );
            fixture.program(
                "virtiofsd",
                "printf '      --readonly   Mark the file system read-only\\n      --fd <FD>    File descriptor\\n'\n",
            );
            fixture.program(
                "vhost-device-vsock",
                "printf '  --uds-path <UDS_PATH>\\n  --groups <GROUPS>\\n'\n",
            );
            fixture.program("bwrap", "printf '    --bind-fd FD DEST\\n'\n");
            fixture
        }

        fn bin(&self) -> PathBuf {
            self.dir.path().join("bin")
        }

        fn program(&self, name: &str, body: &str) {
            crate::test_support::write_program(
                &self.bin().join(name),
                &format!("#!/bin/sh\n{body}"),
            );
        }

        fn check(&self, config: &QemuConfig) -> Result<(), Diagnostic> {
            missing_on(&self.host, &self.env, config, PackageManager::Apt).map(drop)
        }

        fn finding(&self, config: &QemuConfig) -> Diagnostic {
            let err = self.check(config).expect_err("a prerequisite is missing");
            assert_eq!(err.code.as_str(), "SANDBOX_045", "{}", err.why);
            assert!(err.why.contains("--backend bwrap"), "{}", err.why);
            err
        }
    }

    fn command(err: &Diagnostic) -> String {
        match &err.fix {
            Some(FixAction::CopyCommand(command)) => command.clone(),
            other => panic!("expected a command, got {other:?}"),
        }
    }

    #[test]
    fn a_complete_host_misses_nothing() {
        let fixture = Fixture::complete();
        fixture
            .check(&QemuConfig::default())
            .expect("everything is there");
        let lines = each_on(
            &fixture.host,
            &fixture.env,
            &QemuConfig::default(),
            PackageManager::Apt,
        );
        assert_eq!(lines.len(), 8);
        assert!(lines.iter().all(|(_, line)| line.is_ok()), "{lines:?}");
    }

    /// HUM-444: virtiofs must be built in; a module, a missing line and a
    /// configuration that cannot be read are each a finding with the key that
    /// names another kernel, never a pass. Each doctor line stands alone.
    #[test]
    fn a_kernel_without_virtiofs_built_in_is_a_finding() {
        let fixture = Fixture::complete();
        let config = fixture.host.boot.join(format!("config-{RELEASE}"));
        for (text, needle) in [
            (Some("CONFIG_VIRTIO_FS=m\n"), "CONFIG_VIRTIO_FS=m"),
            (Some("# CONFIG_VIRTIO_FS is not set\n"), "is not set"),
            (Some("CONFIG_OTHER=y\n"), "no CONFIG_VIRTIO_FS line"),
            (None, "cannot be read"),
        ] {
            match text {
                Some(text) => std::fs::write(&config, text).unwrap(),
                None => std::fs::remove_file(&config).unwrap(),
            }
            let err = fixture.finding(&QemuConfig::default());
            assert!(err.why.contains(needle), "{needle}: {}", err.why);
            assert!(matches!(
                &err.fix,
                Some(FixAction::ChangeSetting { key, .. }) if key == "sandbox.qemu.kernel"
            ));
            let lines = each_on(
                &fixture.host,
                &fixture.env,
                &QemuConfig::default(),
                PackageManager::Apt,
            );
            let failed: Vec<_> = lines
                .iter()
                .filter(|(_, line)| line.is_err())
                .map(|(id, _)| *id)
                .collect();
            assert_eq!(failed, [Prerequisite::VirtioFs]);
        }
        // The configured image takes its configuration from beside itself.
        let other = fixture.dir.path().join("vmlinuz");
        std::fs::write(&other, "k").unwrap();
        std::fs::write(fixture.dir.path().join("config"), "CONFIG_VIRTIO_FS=y\n").unwrap();
        fixture
            .check(&QemuConfig {
                kernel: Some(other),
                ..QemuConfig::default()
            })
            .expect("the configured kernel has virtiofs built in");
        assert_eq!(config_beside(Path::new("/k/bzImage")), None);
    }

    /// Der erste fehlende Punkt wird genannt, mit dem Paket, das ihn bringt;
    /// die Reihenfolge ist Gerät, Programme, Kernel, Module.
    #[test]
    fn the_first_missing_piece_is_named_with_its_package() {
        let fixture = Fixture::complete();
        std::fs::remove_dir_all(fixture.host.modules_root.join(RELEASE)).expect("rm modules");
        let err = fixture.finding(&QemuConfig::default());
        assert!(err.why.contains("module directory"), "{}", err.why);
        assert_eq!(command(&err), "sudo apt install linux-image-amd64");

        std::fs::remove_file(fixture.host.boot.join(format!("vmlinuz-{RELEASE}"))).expect("rm");
        let err = fixture.finding(&QemuConfig::default());
        assert!(err.why.contains("kernel image"), "{}", err.why);

        for (name, package) in [
            ("bwrap", "sudo apt install bubblewrap"),
            ("vhost-device-vsock", "sudo apt install vhost-device-vsock"),
            ("virtiofsd", "sudo apt install virtiofsd"),
            ("qemu-system-x86_64", "sudo apt install qemu-system-x86"),
        ] {
            std::fs::remove_file(fixture.bin().join(name)).expect("rm program");
            let err = fixture.finding(&QemuConfig::default());
            assert!(err.why.contains(name), "{name}: {}", err.why);
            assert_eq!(command(&err), package, "{name}");
        }

        std::fs::remove_file(&fixture.host.kvm).expect("rm kvm");
        let err = fixture.finding(&QemuConfig::default());
        assert!(err.why.contains("hardware virtualization"), "{}", err.why);
    }

    /// Eine Fähigkeit, die in der Hilfe fehlt, ist eine fehlende
    /// Voraussetzung, und `--fd` zählt nicht als Teil von `--bind-fd`.
    #[test]
    fn a_capability_missing_from_the_help_is_named() {
        for (name, body, word) in [
            (
                "vhost-device-vsock",
                "printf '  --uds-path <P>\\n'\n",
                "--groups",
            ),
            (
                "virtiofsd",
                "printf '  --readonly\\n  --bind-fd\\n'\n",
                "--fd",
            ),
            ("bwrap", "printf '  --ro-bind-fd FD DEST\\n'\n", "--bind-fd"),
            (
                "qemu-system-x86_64",
                "printf 'microvm-legacy\\nname \"vhost-user-vsock-pci\"\\n'\n",
                "microvm",
            ),
        ] {
            let fixture = Fixture::complete();
            fixture.program(name, body);
            let err = fixture.finding(&QemuConfig::default());
            assert!(err.why.contains(word), "{name}: {}", err.why);
            assert!(err.why.contains("does not name it"), "{name}: {}", err.why);
        }
    }

    /// Der Dienst der Dateisysteme liegt unter Debian nicht im `PATH`; ein
    /// gesetzter Pfad ersetzt die Suche, und einer, der nicht ausführbar ist,
    /// wird mit seinem Schlüssel genannt.
    #[test]
    fn libexec_and_configured_paths_replace_the_search() {
        let fixture = Fixture::complete();
        let daemon = fixture.bin().join("virtiofsd");
        std::fs::rename(&daemon, fixture.host.libexec.join("virtiofsd")).expect("move");
        fixture
            .check(&QemuConfig::default())
            .expect("found under libexec");

        let config = QemuConfig {
            vhost_device_vsock: Some(fixture.dir.path().join("nope")),
            ..QemuConfig::default()
        };
        let err = fixture.finding(&config);
        assert!(
            err.why.contains("sandbox.qemu.vhost_device_vsock"),
            "{}",
            err.why
        );

        let kernel = fixture.dir.path().join("vmlinuz-own");
        std::fs::write(&kernel, "k").expect("kernel");
        std::fs::write(
            fixture.dir.path().join("config-own"),
            "CONFIG_VIRTIO_FS=y\n",
        )
        .expect("config");
        let modules = fixture.dir.path().join("own-modules");
        std::fs::create_dir_all(&modules).expect("modules");
        let unknown_release = Fixture {
            host: Host {
                release: None,
                ..fixture.host.clone()
            },
            ..fixture
        };
        let err = unknown_release.finding(&QemuConfig::default());
        assert!(err.why.contains("sandbox.qemu.kernel"), "{}", err.why);
        unknown_release
            .check(&QemuConfig {
                kernel: Some(kernel),
                modules: Some(modules),
                ..QemuConfig::default()
            })
            .expect("named paths need no release");
    }

    /// Ein Kernel-Abbild, das es gibt und das dieser Nutzer nicht lesen
    /// kann, bekommt einen Befehl mit seinem zitierten Pfad. Als Root liest
    /// jeder alles; dort gibt es den Fall nicht.
    #[test]
    fn an_unreadable_kernel_image_is_named_with_a_quoted_path() {
        use std::os::unix::fs::PermissionsExt as _;
        if rustix::process::geteuid().is_root() {
            return;
        }
        let fixture = Fixture::complete();
        let image = fixture.dir.path().join("a kernel");
        std::fs::write(&image, "k").expect("kernel");
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        let err = fixture.finding(&QemuConfig {
            kernel: Some(image.clone()),
            ..QemuConfig::default()
        });
        assert!(err.why.contains("not readable"), "{}", err.why);
        assert_eq!(
            command(&err),
            format!(
                "sudo chmod 0644 {}",
                humanitl_core::shell::shell_path(&image)
            )
        );
    }

    /// Ein Programm, das nicht antwortet, hält die Prüfung nicht auf: Nach
    /// der Frist wird es beendet, und der Befund nennt es.
    #[test]
    fn a_program_that_does_not_answer_is_cut_off() {
        let fixture = Fixture::complete();
        fixture.program("bwrap", "exec /bin/sleep 60\n");
        let started = Instant::now();
        let err = fixture.finding(&QemuConfig::default());
        assert!(
            started.elapsed() < HELP_TIMEOUT * 3,
            "{:?}",
            started.elapsed()
        );
        assert!(err.why.contains("did not within"), "{}", err.why);
    }

    /// Ein Programm, das ein schlafendes Kind forkt und selbst nie endet:
    /// Nach der Frist ist kein Prozess seiner Gruppe mehr am Leben, und die
    /// Prüfung kommt trotzdem innerhalb der Frist zurück.
    #[test]
    fn a_forking_program_that_does_not_answer_leaves_no_process_behind() {
        let fixture = Fixture::complete();
        let pid_file = fixture.dir.path().join("forked.pid");
        fixture.program(
            "bwrap",
            &format!(
                "/bin/sleep 61 &\necho $! > '{}'\nexec /bin/sleep 60\n",
                pid_file.display()
            ),
        );
        let started = Instant::now();
        let err = fixture.finding(&QemuConfig::default());
        assert!(
            started.elapsed() < HELP_TIMEOUT + Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert!(err.why.contains("did not within"), "{}", err.why);
        let forked: u32 = std::fs::read_to_string(&pid_file)
            .expect("the program wrote the pid of its child")
            .trim()
            .parse()
            .expect("a pid");
        let gone = Instant::now() + Duration::from_secs(2);
        while crate::test_support::process_alive(forked) && Instant::now() < gone {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !crate::test_support::process_alive(forked),
            "the forked child {forked} survived"
        );
    }

    /// Ein Programm, das ein Kind mit seiner Ausgabe hinterlässt und selbst
    /// sofort endet: Die Prüfung beendet das Kind mit der Gruppe, bevor sie
    /// den Anführer einsammelt, und wartet nicht auf dessen Pipes.
    #[test]
    fn a_program_that_exits_and_leaves_a_child_leaves_no_process_behind() {
        let fixture = Fixture::complete();
        let pid_file = fixture.dir.path().join("forked.pid");
        fixture.program(
            "bwrap",
            &format!(
                "/bin/sleep 61 &\necho $! > '{}'\nexit 0\n",
                pid_file.display()
            ),
        );
        let started = Instant::now();
        let _ = fixture.finding(&QemuConfig::default());
        assert!(started.elapsed() < HELP_TIMEOUT, "{:?}", started.elapsed());
        let forked: u32 = std::fs::read_to_string(&pid_file)
            .expect("the program wrote the pid of its child")
            .trim()
            .parse()
            .expect("a pid");
        let gone = Instant::now() + Duration::from_secs(2);
        while crate::test_support::process_alive(forked) && Instant::now() < gone {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !crate::test_support::process_alive(forked),
            "the forked child {forked} survived"
        );
    }

    #[test]
    fn every_package_has_a_command_for_every_manager() {
        for manager in PackageManager::ALL {
            for package in [
                Package::Emulator,
                Package::FileDaemon,
                Package::VsockDevice,
                Package::Bubblewrap,
                Package::Kernel,
            ] {
                assert!(package.install(manager).starts_with("sudo "));
            }
        }
    }
}
