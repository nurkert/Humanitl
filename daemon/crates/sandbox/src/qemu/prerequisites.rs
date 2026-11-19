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

use humanitl_config::{Env, QemuConfig};
use humanitl_core::diagnostics::codes::SANDBOX_045;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::doctor::StaticLine;
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
    /// Der Weg zur heutigen Unit des Daemons, wenn eine ältere die Module
    /// verbirgt (HUM-529, [`crate::service_unit`]).
    service_fix: FixAction,
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
            service_fix: crate::service_unit::InstallKind::probe(
                &humanitl_config::Paths::from_process(),
            )
            .fix(),
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

/// Der Befehl, der in einen installierten Kernel neu startet (HUM-529).
const REBOOT_COMMAND: &str = "systemctl reboot";

/// So viel einer Kernel-Konfiguration wird gelesen; Debians hat rund 300 KiB.
const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;

/// Der Dienst der Dateisysteme: nur lesbar ausliefern, Socket als Deskriptor,
/// dazu die Wahl des Cache (HUM-469, HUM-476,
/// [`super::lines::FILE_DAEMON_NOFILE`]). 1.13.2 (Debian trixie, CI) und
/// 1.14.0 kennen alle drei; gefragt wird, damit eine Fassung ohne eine davon
/// als fehlende Voraussetzung gemeldet wird und nicht erst beim Start der
/// Sitzung scheitert. `--allow-mmap` braucht die Stufe seit HUM-484 nicht
/// mehr (`lines.rs`, `file_daemon_tail`).
const FILE_DAEMON: Program = Program {
    name: "virtiofsd",
    key: Some("sandbox.qemu.virtiofsd"),
    in_libexec: true,
    package: Package::FileDaemon,
    questions: &[Question {
        args: &["--help"],
        words: &["--readonly", "--fd", "--cache"],
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

/// bubblewrap: der geprüfte Projektordner kommt als Deskriptor (HUM-437),
/// und jedes tmpfs eines Gefängnisses hat eine feste Größe (`--size`,
/// HUM-485).
const BUBBLEWRAP: Program = Program {
    name: "bwrap",
    key: None,
    in_libexec: false,
    package: Package::Bubblewrap,
    questions: &[Question {
        args: &["--help"],
        words: &["--bind-fd", "--ro-bind-fd", "--size"],
    }],
};

/// Die Dateien, von denen die Prüfung abhängt (HUM-502): je Programm der
/// gesetzte Pfad oder jeder Ort im `PATH` (und unter `libexec`), an dem es
/// liegen könnte, dazu `/dev/kvm`, das Kernel-Abbild samt seiner
/// Konfiguration und das Verzeichnis der Module samt seinen Listen
/// ([`MODULE_LISTS`]). Ändert sich eine davon, gilt ein gemerktes Ergebnis
/// nicht mehr, und ebenso ein gespeicherter Probestart (HUM-506).
pub(crate) fn search_files(env: &Env, config: &QemuConfig) -> Vec<PathBuf> {
    let host = Host::system();
    let mut files = vec![host.kvm.clone()];
    for (program, configured) in [
        (&EMULATOR, config.qemu.as_deref()),
        (&FILE_DAEMON, config.virtiofsd.as_deref()),
        (&VSOCK_DEVICE, config.vhost_device_vsock.as_deref()),
        (&BUBBLEWRAP, None),
    ] {
        if let Some(path) = configured {
            files.push(path.to_path_buf());
            continue;
        }
        files.extend(crate::backend_search::path_candidates(env, program.name));
        if program.in_libexec {
            files.push(host.libexec.join(program.name));
        }
    }
    let image = config.kernel.clone().or_else(|| {
        host.release
            .as_ref()
            .map(|release| host.boot.join(format!("vmlinuz-{release}")))
    });
    if let Some(image) = image {
        files.extend(config_beside(&image));
        files.push(image);
    }
    let modules = config.modules.clone().or_else(|| {
        host.release
            .as_ref()
            .map(|release| host.modules_root.join(release))
    });
    if let Some(modules) = modules {
        files.extend(MODULE_LISTS.iter().map(|list| modules.join(list)));
        files.push(modules);
    }
    files
}

/// Die Listen im Verzeichnis der Module, die der Abdruck der Suche und des
/// Probestarts mitnimmt (HUM-506): `modules.builtin` liest der Shim im Gast
/// und die Prüfung des Overlays; `modules.dep` und `modules.order` schreibt
/// `depmod` bei jeder Installation eines Kernel-Pakets neu, auch wenn nur
/// Moduldateien sich ändern. Die einzelnen Moduldateien stehen nicht im
/// Abdruck: Sie zu finden hieße, bei jeder Antwort einen Baum von einigen
/// tausend Einträgen zu durchlaufen. Eine Moduldatei, die jemand von Hand
/// ersetzt, ohne `depmod` laufen zu lassen, fängt nur `--probe`, das nie
/// aus dem Zwischenspeicher antwortet.
pub(crate) const MODULE_LISTS: [&str; 3] = ["modules.builtin", "modules.dep", "modules.order"];

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
    /// bubblewrap mit `--bind-fd`, `--ro-bind-fd` und `--size`.
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
///
/// Eine Prüfung, die eine fehlende Voraussetzung braucht, ist ohne Messung
/// und verweist auf deren Zeile (HUM-495): virtiofs liest die Konfiguration
/// neben dem Kernel-Abbild, und fehlt das Abbild, gibt es nichts zu lesen.
pub(crate) fn each_prerequisite(
    env: &Env,
    config: &QemuConfig,
) -> Vec<(Prerequisite, StaticLine<Prerequisite>)> {
    each_on(&Host::system(), env, config, PackageManager::detect())
}

/// [`each_prerequisite`] auf diesen Orten.
fn each_on(
    host: &Host,
    env: &Env,
    config: &QemuConfig,
    manager: PackageManager,
) -> Vec<(Prerequisite, StaticLine<Prerequisite>)> {
    let usable = |program: &Program, configured: Option<&Path>| {
        let found = find(program, configured, env, host, manager)?;
        for question in program.questions {
            ask(program, &found, question, manager)?;
        }
        Ok::<_, Diagnostic>(found.display().to_string())
    };
    let kernel = kernel_readable(host, config, manager);
    // Ohne Abbild gibt es keine Konfiguration daneben: Die Zeile wiederholt
    // dann nicht den Befund des Abbilds, sondern verweist auf ihn.
    let virtio_fs = match &kernel {
        Ok(image) => StaticLine::Checked(virtio_fs_built_in(image)),
        Err(_) => StaticLine::Unchecked {
            after: Prerequisite::Kernel,
            needs: "the kernel image",
        },
    };
    vec![
        (
            Prerequisite::Kvm,
            kvm_usable(host)
                .map(|()| host.kvm.display().to_string())
                .into(),
        ),
        (
            Prerequisite::Emulator,
            usable(&EMULATOR, config.qemu.as_deref()).into(),
        ),
        (
            Prerequisite::FileDaemon,
            usable(&FILE_DAEMON, config.virtiofsd.as_deref()).into(),
        ),
        (
            Prerequisite::VsockDevice,
            usable(&VSOCK_DEVICE, config.vhost_device_vsock.as_deref()).into(),
        ),
        (Prerequisite::Bubblewrap, usable(&BUBBLEWRAP, None).into()),
        (
            Prerequisite::Kernel,
            kernel.map(|image| image.display().to_string()).into(),
        ),
        (Prerequisite::VirtioFs, virtio_fs),
        (
            Prerequisite::Modules,
            modules_present(host, config, manager)
                .map(|dir| dir.display().to_string())
                .into(),
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

/// Die Fassung aus dem Kopf einer Kernel-Konfiguration
/// (`# Linux/x86 6.12.115 Kernel Configuration`), als Haupt- und
/// Nebennummer; keine, wenn der Kopf fehlt oder anders aussieht.
fn config_release(text: &str) -> Option<(u32, u32)> {
    let line = text.lines().take(16).find_map(|line| {
        line.strip_prefix("# Linux/")?
            .strip_suffix(" Kernel Configuration")
    })?;
    let version = line.split_once(' ')?.1;
    let mut parts = version.split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Der Kernel hat virtiofs eingebaut: Seine Konfiguration neben dem Abbild
/// trägt [`VIRTIO_FS_BUILT_IN`]. Ein Modul (`=m`), eine fehlende Zeile und
/// eine Konfiguration, die sich nicht lesen lässt, sind ein Befund.
///
/// Keine Untergrenze der Fassung (HUM-484). Die Untergrenze 6.6 aus HUM-469
/// galt `--cache never`: Dort öffnet virtiofsd jede Datei ohne Seitencache,
/// und erst Linux 6.6 (`FUSE_DIRECT_IO_ALLOW_MMAP`) bildet eine solche Datei
/// geteilt ab; ein älterer Gast verweigerte jedes `mmap(MAP_SHARED)` mit
/// `ENODEV`, kaputt, ohne es zu sagen. Seit HUM-476 laufen die Dienste mit
/// `--cache auto`, wo virtiofsd keine Datei so öffnet und geteilte
/// Abbildungen durch den Seitencache gehen, wie FUSE es seit jeher kann;
/// gemessen in `kvm_tests::shared_mappings_work_without_allow_mmap`. Was
/// ein Gast sonst braucht (virtiofs als Wurzel, `overlay` mit `userxattr`
/// für die Wegwerf-Sitzung, die Filter von PID 1), scheitert auf einem zu
/// alten Kernel laut: Der Gast bootet nicht oder meldet die Prüfung rot, und
/// die Sitzung wird abgelehnt. Getestet sind 6.12 (CI) und 7.x.
///
/// Der Beleg bei Erfolg: die Zeile der Konfiguration und die Fassung aus
/// ihrem Kopf, oder dass sie unbekannt ist.
fn virtio_fs_built_in(image: &Path) -> Result<String, Diagnostic> {
    // Welcher Kernel der richtige ist, weiß nur der Mensch: Der Rat steht im
    // Satz, ein leerer Wert wäre keiner, den `config set` nimmt (HUM-507).
    let fix = || None::<FixAction>;
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
        return Ok(match config_release(&text) {
            Some((major, minor)) => format!("{VIRTIO_FS_BUILT_IN}, kernel {major}.{minor}"),
            None => format!(
                "{VIRTIO_FS_BUILT_IN}, kernel version unknown ({} has no version header)",
                config.display()
            ),
        });
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
fn missing(what: &str, fix: impl Into<Option<FixAction>>) -> Diagnostic {
    Diagnostic::builder(SANDBOX_045, Severity::Blocking)
        .why(format!(
            "the kernel level needs {what}; nothing starts, and no other backend takes over. \
             Start with --backend bwrap for the shared kernel"
        ))
        .maybe_fix(fix.into())
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
    if config.kernel.is_some() {
        return Err(missing(
            &format!(
                "a kernel image, and sandbox.qemu.kernel names {shown}, where there is no file; \
                 name an existing image there, or leave it empty for the running kernel's"
            ),
            // Leer heißt: der Kernel, der läuft; `-` entfernt den Schlüssel
            // (HUM-507).
            humanitl_config::value::reset_fix("sandbox.qemu.kernel"),
        ));
    }
    let others = other_kernels(&host.boot, host.release.as_deref());
    Err(not_installed(
        &format!("a kernel image, and there is no file at {shown}"),
        "images",
        &others,
        manager,
    ))
}

/// Die Fassungen der anderen Kernel-Abbilder unter `boot`
/// (`vmlinuz-<release>`), sortiert, ohne `running`.
fn other_kernels(boot: &Path, running: Option<&str>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(boot) else {
        return Vec::new();
    };
    let mut releases: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_prefix("vmlinuz-").map(str::to_owned))
        .filter(|release| {
            !release.is_empty()
                && Some(release.as_str()) != running
                && !release.contains(char::is_control)
        })
        .collect();
    releases.sort();
    releases
}

/// `SANDBOX_045` für ein Stück des laufenden Kernels, das fehlt
/// (HUM-529). Liegen `what` anderer Kernel daneben (`others`), hat meist ein
/// Upgrade das Paket des laufenden entfernt: Dann hilft ein Neustart in einen
/// installierten Kernel, und der Befehl, der das Paket installiert, täte
/// nichts. Sonst fehlt der Kernel und das Paket bringt ihn.
fn not_installed(
    what_missing: &str,
    what: &str,
    others: &[String],
    manager: PackageManager,
) -> Diagnostic {
    if others.is_empty() {
        return missing(
            what_missing,
            FixAction::CopyCommand(Package::Kernel.install(manager).to_owned()),
        );
    }
    missing(
        &format!(
            "{what_missing}. The {what} of {} are installed, so an upgrade most likely removed \
             the package of the running kernel; installing the kernel package again changes \
             nothing while this one runs. Restart the computer into an installed kernel",
            others.join(", ")
        ),
        FixAction::CopyCommand(REBOOT_COMMAND.to_owned()),
    )
}

/// Das Verzeichnis der Module des Kernels: aus `sandbox.qemu.modules`, sonst
/// das des laufenden Kernels unter `/usr/lib/modules`.
///
/// Der Befund unterscheidet, was ein Mensch dagegen tun kann (HUM-529):
/// Verborgen ist das Verzeichnis, wenn der Blick mit einem anderen Fehler als
/// „gibt es nicht" endet, etwa `EACCES` unter einer Unit mit
/// `ProtectKernelModules` (Humanitl 0.4.0); fehlt es, aber Module anderer
/// Kernel liegen daneben, hat meist ein Upgrade das Paket des laufenden
/// entfernt ([`not_installed`]).
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
    let shown = dir.display();
    // Das richtige Verzeichnis kennt nur der Mensch; der Rat steht im Satz
    // (HUM-507).
    let setting = || None::<FixAction>;
    match std::fs::metadata(&dir) {
        Ok(meta) if meta.is_dir() => Ok(dir),
        Ok(_) if config.modules.is_some() => Err(missing(
            &format!(
                "the module directory of the kernel, and sandbox.qemu.modules names {shown}, \
                 which is not a directory"
            ),
            setting(),
        )),
        Ok(_) => Err(missing(
            &format!("the module directory of the running kernel, and {shown} is not a directory"),
            FixAction::CopyCommand(Package::Kernel.install(manager).to_owned()),
        )),
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(missing(
            &format!(
                "the module directory of the kernel, and {shown} is hidden from the daemon \
                 ({err}). A service unit with ProtectKernelModules hides it, as the unit of \
                 Humanitl 0.4.0 did; the current unit leaves it visible: {}",
                crate::service_unit::BOTH_WAYS
            ),
            host.service_fix.clone(),
        )),
        Err(_) if config.modules.is_some() => Err(missing(
            &format!(
                "the module directory of the kernel, and sandbox.qemu.modules names {shown}, \
                 where there is nothing; name the module directory of the kernel in \
                 sandbox.qemu.modules, or leave sandbox.qemu.kernel and sandbox.qemu.modules \
                 empty for the running kernel's"
            ),
            setting(),
        )),
        Err(_) => {
            let others = other_module_dirs(&host.modules_root, host.release.as_deref());
            Err(not_installed(
                &format!(
                    "the module directory of the running kernel, and there is no directory at \
                     {shown}"
                ),
                "modules",
                &others,
                manager,
            ))
        }
    }
}

/// Die anderen Verzeichnisse unter `root` ohne `running`: die Module
/// anderer Kernel, sortiert, ohne Verweise und versteckte Namen.
fn other_module_dirs(root: &Path, running: Option<&str>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            !name.starts_with('.')
                && Some(name.as_str()) != running
                && !name.contains(char::is_control)
        })
        .collect();
    names.sort();
    names
}

/// Die Fassung des laufenden Kernels ist unbekannt, und `key` ist leer.
fn release_unknown(key: &str) -> Diagnostic {
    missing(
        &format!(
            "the release of the running kernel to find its files, and \
             /proc/sys/kernel/osrelease is unreadable; name the path in {key}"
        ),
        // Den Pfad kennt nur der Mensch; der Rat steht im Satz (HUM-507).
        None::<FixAction>,
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
                service_fix: FixAction::InstallService,
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
                "printf '      --readonly   Mark the file system read-only\\n      --fd <FD>    File descriptor\\n      --cache <CACHE>\\n'\n",
            );
            fixture.program(
                "vhost-device-vsock",
                "printf '  --uds-path <UDS_PATH>\\n  --groups <GROUPS>\\n'\n",
            );
            fixture.program(
                "bwrap",
                "printf '    --bind-fd FD DEST\\n    --ro-bind-fd FD DEST\\n    --size BYTES\\n'\n",
            );
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

    /// Die Fassung des Gastkernels aus dem Kopf seiner Konfiguration steht
    /// im Beleg, und keine Fassung wird abgelehnt (HUM-484): 6.1 (Debian
    /// bookworm), 6.12 (der Kernel der CI) und 7.3 gehen, und ohne Kopf
    /// nennt der Beleg die unbekannte Fassung.
    ///
    /// Mutationsbeweis: die Untergrenze 6.6 aus HUM-469 zurück, 6.1 wird
    /// abgelehnt, und der Test wird rot.
    #[test]
    fn the_guest_kernel_version_is_shown_and_never_refused() {
        let header = |version: &str| {
            format!(
                "#\n# Automatically generated file; DO NOT EDIT.\n\
                 # Linux/x86 {version} Kernel Configuration\n#\nCONFIG_VIRTIO_FS=y\n"
            )
        };
        assert_eq!(config_release(&header("6.12.115")), Some((6, 12)));
        assert_eq!(config_release(&header("6.1.0-28-amd64")), Some((6, 1)));
        assert_eq!(config_release(&header("6.6")), Some((6, 6)));
        assert_eq!(config_release("CONFIG_VIRTIO_FS=y\n"), None);
        assert_eq!(config_release(&header("six.12")), None);

        let fixture = Fixture::complete();
        let config = fixture.host.boot.join(format!("config-{RELEASE}"));
        let virtio = |fixture: &Fixture| {
            each_on(
                &fixture.host,
                &fixture.env,
                &QemuConfig::default(),
                PackageManager::Apt,
            )
            .into_iter()
            .find(|(which, _)| *which == Prerequisite::VirtioFs)
            .map(|(_, line)| checked(line))
            .unwrap()
        };

        for (version, shown) in [
            ("6.1.0-28-amd64", "kernel 6.1"),
            ("6.12.115", "kernel 6.12"),
            ("7.3.4", "kernel 7.3"),
        ] {
            std::fs::write(&config, header(version)).unwrap();
            fixture.check(&QemuConfig::default()).expect(version);
            let line = virtio(&fixture).expect(version);
            assert!(line.contains(shown), "{line}");
        }

        std::fs::write(&config, "CONFIG_VIRTIO_FS=y\n").unwrap();
        fixture.check(&QemuConfig::default()).expect("no header");
        let line = virtio(&fixture).unwrap();
        assert!(line.contains("kernel version unknown"), "{line}");
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
        assert!(
            lines
                .iter()
                .all(|(_, line)| matches!(line, StaticLine::Checked(Ok(_)))),
            "{lines:?}"
        );
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
            // Welcher Kernel der richtige ist, weiß nur der Mensch: Der Rat
            // steht im Satz, kein leerer Wert als `ChangeSetting` (HUM-507).
            assert!(err.why.contains("sandbox.qemu.kernel"), "{}", err.why);
            assert_eq!(err.fix, None);
            let lines = each_on(
                &fixture.host,
                &fixture.env,
                &QemuConfig::default(),
                PackageManager::Apt,
            );
            let failed: Vec<_> = lines
                .iter()
                .filter(|(_, line)| matches!(line, StaticLine::Checked(Err(_))))
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

    /// Das Ergebnis einer Zeile, die geprüft hat; eine ohne Messung ist hier
    /// ein Fehler des Tests.
    fn checked(line: StaticLine<Prerequisite>) -> Result<String, Diagnostic> {
        match line {
            StaticLine::Checked(result) => result,
            StaticLine::Unchecked { after, .. } => panic!("unchecked after {after:?}"),
        }
    }

    fn modules_line(fixture: &Fixture, config: &QemuConfig) -> Result<String, Diagnostic> {
        each_on(&fixture.host, &fixture.env, config, PackageManager::Apt)
            .into_iter()
            .find(|(which, _)| *which == Prerequisite::Modules)
            .map(|(_, line)| checked(line))
            .expect("a line for the modules")
    }

    /// HUM-495: Fehlt das Kernel-Abbild, ist `virtio_fs` ohne Messung und
    /// verweist auf die Zeile `kernel`, statt deren Befund zu wiederholen;
    /// nur die Zeile `kernel` ist ein Befund. Mutationsbeweis: die Zeile
    /// `virtio_fs` erbt wieder den Befund des Abbilds (`Err(missing.clone())`),
    /// und der Test wird rot.
    #[test]
    fn without_a_kernel_image_virtio_fs_is_unchecked_and_names_the_kernel_line() {
        let fixture = Fixture::complete();
        std::fs::remove_file(fixture.host.boot.join(format!("vmlinuz-{RELEASE}"))).unwrap();
        let lines = each_on(
            &fixture.host,
            &fixture.env,
            &QemuConfig::default(),
            PackageManager::Apt,
        );
        let line = |which: Prerequisite| {
            lines
                .iter()
                .find(|(id, _)| *id == which)
                .map(|(_, line)| line.clone())
                .expect("a line")
        };
        assert!(
            matches!(line(Prerequisite::Kernel), StaticLine::Checked(Err(_))),
            "{lines:?}"
        );
        assert_eq!(
            line(Prerequisite::VirtioFs),
            StaticLine::Unchecked {
                after: Prerequisite::Kernel,
                needs: "the kernel image",
            }
        );
        let failed: Vec<_> = lines
            .iter()
            .filter(|(_, line)| matches!(line, StaticLine::Checked(Err(_))))
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(failed, [Prerequisite::Kernel]);
    }

    /// HUM-529: Ein Verzeichnis, das der Blick nicht erreicht (hier: ein
    /// Elternverzeichnis ohne Suchrecht, wie `ProtectKernelModules` es unter
    /// der Unit von 0.4.0 machte), heißt verborgen, nie „gibt es nicht", und
    /// der Weg ist die Unit, die Humanitl schreibt, kein Paket. Als Root gilt
    /// kein Suchrecht, dort sagt der Test nichts.
    #[test]
    fn a_hidden_module_directory_is_named_hidden_and_not_missing() {
        use std::os::unix::fs::PermissionsExt as _;
        if rustix::process::geteuid().is_root() {
            eprintln!("skipped: root searches every directory");
            return;
        }
        let mut fixture = Fixture::complete();
        let root = fixture.host.modules_root.clone();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o000)).unwrap();
        let line = modules_line(&fixture, &QemuConfig::default());
        // Mit dem Paket ist der Weg dessen Update und ein Neustart des
        // Dienstes (`crate::service_unit`).
        fixture.host.service_fix =
            FixAction::CopyCommand(crate::service_unit::RESTART_COMMAND.to_owned());
        let packaged = modules_line(&fixture, &QemuConfig::default());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            command(&packaged.expect_err("hidden as well")),
            crate::service_unit::RESTART_COMMAND
        );
        let hidden = line.expect_err("the daemon cannot see it");
        for way in ["humanitl daemon install", "systemctl --user daemon-reload"] {
            assert!(hidden.why.contains(way), "{way}: {}", hidden.why);
        }
        assert!(
            hidden.why.contains("hidden from the daemon"),
            "{}",
            hidden.why
        );
        assert!(
            !hidden.why.contains("there is no directory"),
            "{}",
            hidden.why
        );
        assert_eq!(hidden.fix, Some(FixAction::InstallService));
    }

    /// HUM-529: Fehlen die Module des laufenden Kernels, aber andere sind
    /// installiert, hat ein Upgrade das Paket entfernt; der Weg ist ein
    /// Neustart, nie `apt install`, das nichts täte. Dasselbe für das
    /// Abbild unter `/boot`. Ohne einen anderen Kernel bleibt es das Paket,
    /// und ein gesetzter Pfad zeigt auf seinen Schlüssel. Mutationsbeweis:
    /// `not_installed` ohne den Zweig für andere Kernel wird hier rot.
    #[test]
    fn a_running_kernel_removed_by_an_upgrade_asks_for_a_reboot() {
        let fixture = Fixture::complete();
        let modules = fixture.host.modules_root.join(RELEASE);
        std::fs::remove_dir_all(&modules).unwrap();
        // Nur Module eines anderen Kernels, noch kein anderes Abbild.
        std::fs::create_dir_all(fixture.host.modules_root.join("7.3.5+deb14-amd64")).unwrap();
        std::fs::create_dir_all(fixture.host.modules_root.join(".hidden")).unwrap();
        let err = modules_line(&fixture, &QemuConfig::default()).expect_err("removed");
        assert_eq!(command(&err), REBOOT_COMMAND, "{}", err.why);
        assert!(
            err.why.contains("modules of 7.3.5+deb14-amd64 are"),
            "{}",
            err.why
        );
        assert!(
            err.why.contains(&modules.display().to_string()),
            "{}",
            err.why
        );

        std::fs::remove_file(fixture.host.boot.join(format!("vmlinuz-{RELEASE}"))).unwrap();
        let err = fixture.finding(&QemuConfig::default());
        assert!(err.why.contains("kernel image"), "{}", err.why);
        assert_eq!(command(&err), "sudo apt install linux-image-amd64");
        std::fs::write(fixture.host.boot.join("vmlinuz-7.3.5+deb14-amd64"), "k").unwrap();
        let err = fixture.finding(&QemuConfig::default());
        assert_eq!(command(&err), REBOOT_COMMAND, "{}", err.why);
        assert!(err.why.contains("7.3.5+deb14-amd64"), "{}", err.why);

        std::fs::remove_dir_all(fixture.host.modules_root.join("7.3.5+deb14-amd64")).unwrap();
        let err = modules_line(&fixture, &QemuConfig::default()).expect_err("no modules at all");
        assert_eq!(command(&err), "sudo apt install linux-image-amd64");

        let configured = QemuConfig {
            modules: Some(fixture.dir.path().join("own-modules")),
            ..QemuConfig::default()
        };
        let err = modules_line(&fixture, &configured).expect_err("unset");
        // Das richtige Verzeichnis kennt nur der Mensch; der Rat steht im
        // Satz (HUM-507).
        assert!(err.why.contains("sandbox.qemu.modules"), "{}", err.why);
        assert_eq!(err.fix, None);
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
            (
                "virtiofsd",
                "printf '  --readonly\\n  --fd <FD>\\n'\n",
                "--cache",
            ),
            ("bwrap", "printf '  --ro-bind-fd FD DEST\\n'\n", "--bind-fd"),
            (
                "bwrap",
                "printf '  --bind-fd FD DEST\\n  --ro-bind-fd FD DEST\\n'\n",
                "--size",
            ),
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
