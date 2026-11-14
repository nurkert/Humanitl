//! Die Startzeilen einer Sitzung der Kernel-Stufe (HUM-444): die vier
//! Gefängnisse, das Skript des ersten Prozesses der Unit und die Unit selbst.
//!
//! Alles hier ist eine reine Funktion der Eingaben; gestartet wird nichts.
//! Die Tests prüfen die festen Optionen jeder Zeile und eine Verbotsliste:
//! keine Netzoption, kein `-netdev`, kein `-serial` außer `none`, kein
//! `--forward-cid`, kein `--vm`, kein zweiter Lauscher.
//!
//! **Die vier Gefängnisse** (HUM-442, Endform Q7/Q8), jedes ein bwrap mit
//! `--unshare-all --die-with-parent --new-session --cap-drop ALL` und nur dem
//! `/usr` des Hosts:
//!
//! 1. virtiofsd der Wurzel. Die Wurzel des Gasts liegt im Gefängnis unter
//!    [`GUEST_ROOT`], gebaut aus den Zeilen des Plans ([`RootEntry`]);
//!    geteilt wird nur dieses Unterverzeichnis, nie `/`, damit das `/proc`
//!    des Gefängnisses nicht in der Wurzel des Gasts steht (HUM-465). Den
//!    Socket bekommt der Dienst als Deskriptor 3 (`--fd`), so dass das
//!    Laufzeitverzeichnis gar nicht im Gefängnis steht.
//! 2. virtiofsd für `/work`: der geprüfte Projektordner als Deskriptor 4
//!    (`--bind-fd`), darüber die Masken des Profils, die es im Projekt gibt.
//! 3. vhost-device-vsock, ein eigener Prozess je Sitzung mit eigener Gruppe
//!    (Q9), `--uds-path` im Laufzeitverzeichnis, nie `--forward-cid`.
//! 4. QEMU `-M microvm` ohne Konsole, Monitor, Anzeige und Netz, `-sandbox`
//!    voll, `/dev/kvm` als einziges Gerät, aus `/boot` nur die Kernel-Datei.
//!
//! **Ankündigung von Einhängepunkten.** virtiofsd ab 1.11 kündigt die
//! Einhängepunkte seines Gefängnisses dem Gast als eigene Einhängungen an
//! (HUM-465). Die feste Regel: Kennt der virtiofsd `--no-announce-submounts`,
//! steht es in beiden Zeilen; eine ältere Fassung kündigt nichts an. Die
//! erwartete Mounttabelle des Gasts kennt deshalb unter `/` und `/work` keine
//! weiteren virtiofs-Einhängungen, und eine angekündigte wäre `SANDBOX_049`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::prerequisites::Resolved;

/// Wo die Wurzel des Gasts im Gefängnis des virtiofsd liegt.
pub(crate) const GUEST_ROOT: &str = "/guest";

/// Wo der Projektordner im Gefängnis des zweiten virtiofsd liegt.
pub(crate) const WORK_SHARE: &str = "/share";

/// Die Deskriptoren, die der erste Prozess der Unit über die Übergabe
/// bekommt (`humanitl-shim --fds-then-exec`), in dieser Reihenfolge ab 3.
pub(crate) const ROOT_SOCKET_FD: i32 = 3;
/// Siehe [`ROOT_SOCKET_FD`]: der geöffnete und geprüfte Projektordner.
pub(crate) const PROJECT_FD: i32 = 4;
/// Siehe [`ROOT_SOCKET_FD`]: das Leseende der Lebensleine.
pub(crate) const LIFELINE_FD: i32 = 5;

/// Die feste Grenze der Deskriptoren jedes virtiofsd (HUM-469): `LimitNOFILE`
/// der Unit, weich und hart, für jeden ihrer Prozesse.
///
/// **Wie sie bindet.** Ohne `--rlimit-nofile` will virtiofsd seine weiche
/// Grenze auf `min(1 000 000, nr_open)` heben; das scheitert an der harten
/// Grenze der Unit, und er nimmt die harte (`src/limits.rs`, in 1.13.2 und
/// 1.14.0 gleich, gemessen: höchstens 64 948 offen). Anheben könnte er sie nur
/// mit `CAP_SYS_RESOURCE`, und die hat im Gefängnis kein Prozess. Von der
/// Grenze behält virtiofsd einen kleinen Rest für sich und antwortet dem Gast
/// darüber mit `ENFILE`, statt selbst zu scheitern: Der Dienst läuft weiter,
/// und was der Gast freigibt, kann er wieder belegen. In HUM-445 waren es ohne
/// diese Eigenschaft die 524 288 des Dienstverwalters.
///
/// **Warum nicht `--rlimit-nofile`.** Die Option setzt weich und hart auf N
/// und beendet virtiofsd, wenn N über der geerbten harten Grenze liegt.
/// systemd klemmt `LimitNOFILE` still auf die harte Grenze des
/// Dienstverwalters (gemessen, systemd 262: verlangt 2 000 000, bekommen
/// 524 288, die Unit startet). Hätte ein Dienstverwalter weniger als N, bekäme
/// die Unit also weniger, und ein virtiofsd mit `--rlimit-nofile N` endete
/// beim Start; der Gast fände keine Wurzel, und die Sitzung endete mit „no
/// CHECK line" statt mit dem Grund (nachgestellt mit `LimitNOFILE=4096`). Nur
/// über die Unit gilt dann die kleinere Grenze, fest und ohne Fehlstart.
///
/// **Warum 65 536.** Mit `--cache never` hält virtiofsd nur noch die Inodes
/// offen, die der Gast gerade festhält (offene Dateien und ihre
/// Verzeichnisse, Arbeitsverzeichnisse, abgebildete Bibliotheken,
/// inotify-Wachen), je eines mit einem Deskriptor und jede offene Datei mit
/// einem zweiten. Gemessen (HUM-469, Wurzel mit dem `/usr` des Hosts): 98
/// nach einem Walk über 735 790 Einträge, höchstens 131 beim Kopieren,
/// Lesen, Durchsuchen und `git` über 13 475 Dateien und bei 300
/// Programmstarts. 65 536 lässt auch einer Sitzung, die Tausende Dateien
/// offen hält oder beobachtet, reichlich Luft. Zugleich ist es ein Achtel der 524 288,
/// die der Dienstverwalter sonst als harte Grenze vergibt, ein Fünfzehntel der
/// eigenen Vorgabe von virtiofsd und verschwindend gegen `fs.file-max`; der
/// Kernel-Speicher, den ein Gast über einen Dienst binden kann, ist damit
/// fest begrenzt.
pub(crate) const FILE_DAEMON_NOFILE: u32 = 65_536;

/// Die CID des Gasts; jede Sitzung hat ihr eigenes Gerät, also genügt eine.
pub(crate) const GUEST_CID: u32 = 3;

/// Die Dateien im Laufzeitverzeichnis einer Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    /// Das Laufzeitverzeichnis.
    pub(crate) dir: PathBuf,
}

impl Layout {
    /// Die Unterverzeichnisse der Sockets, je eins je Dienst; kein
    /// Gefängnis bindet das Laufzeitverzeichnis selbst.
    pub(crate) const SOCKET_DIRS: [&'static str; 4] = ["root", "work", "vsock", "door"];

    /// Das Verzeichnis des Sockets der Wurzel; der Emulator bindet es nur
    /// lesbar, kein Gefängnis zum Schreiben.
    pub(crate) fn root_dir(&self) -> PathBuf {
        self.dir.join("root")
    }

    /// Das Verzeichnis des Sockets für `/work`; nur dessen virtiofsd bindet
    /// es beschreibbar.
    pub(crate) fn work_dir(&self) -> PathBuf {
        self.dir.join("work")
    }

    /// Das Verzeichnis des vhost-user-Sockets des vsock-Geräts.
    pub(crate) fn device_dir(&self) -> PathBuf {
        self.dir.join("vsock")
    }

    /// Das Verzeichnis der Tür: `--uds-path` des Geräts und der eine
    /// Lauscher des Daemons. Nur das vsock-Gerät bindet es, nie der
    /// Emulator.
    pub(crate) fn door_dir(&self) -> PathBuf {
        self.dir.join("door")
    }

    /// Der Socket des virtiofsd der Wurzel; der Daemon bindet ihn.
    pub(crate) fn root_socket(&self) -> PathBuf {
        self.root_dir().join("root.sock")
    }

    /// Der Socket des virtiofsd für `/work`.
    pub(crate) fn work_socket(&self) -> PathBuf {
        self.work_dir().join("work.sock")
    }

    /// Der vhost-user-Socket des vsock-Geräts.
    pub(crate) fn device_socket(&self) -> PathBuf {
        self.device_dir().join("vsock.sock")
    }

    /// `--uds-path` des vsock-Geräts: Dort verbindet sich der Host mit dem
    /// Gast (`CONNECT <port>`).
    pub(crate) fn door_device(&self) -> PathBuf {
        self.door_dir().join("door")
    }

    /// Der eine Lauscher des Hosts, `<uds>_<port>`, den der Gast erreicht.
    pub(crate) fn door_listener(&self, port: u32) -> PathBuf {
        self.door_dir().join(format!("door_{port}"))
    }

    /// Der Socket der Übergabe an den ersten Prozess der Unit.
    pub(crate) fn handover(&self) -> PathBuf {
        self.dir.join("handover")
    }

    /// Die Dateien des Gasts: Gastplan, Identität, Masken, Dateien des
    /// Adapters. Nie im Gefängnis als Verzeichnis, nur je Datei gebunden.
    pub(crate) fn files(&self) -> PathBuf {
        self.dir.join("files")
    }

    /// Die Datei mit dem Namen der Unit, für die Reste nach einem Absturz.
    pub(crate) fn unit_file(&self) -> PathBuf {
        self.dir.join("unit")
    }

    /// Die Sockets, die das Verzeichnis tragen muss, solange die Sitzung
    /// läuft, relativ zu ihm; die Host-Prüfung hält jeden weiteren für einen
    /// Fund und jeden fehlenden auch.
    pub(crate) fn declared_sockets(door_port: u32) -> Vec<String> {
        vec![
            "root/root.sock".to_owned(),
            "work/work.sock".to_owned(),
            "vsock/vsock.sock".to_owned(),
            "door/door".to_owned(),
            format!("door/door_{door_port}"),
        ]
    }
}

/// Ein Eintrag der Wurzel des Gasts, als Pfad im Gast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RootEntry {
    /// Eine Quelle des Hosts, nur lesbar.
    Bind {
        /// Die Quelle auf dem Host.
        source: PathBuf,
        /// Der Pfad im Gast.
        target: PathBuf,
    },
    /// Ein Verweis.
    Symlink {
        /// Wohin er zeigt.
        link: PathBuf,
        /// Der Pfad im Gast.
        target: PathBuf,
    },
    /// Ein leeres Dateisystem: im Gast ein leeres, nur lesbares Verzeichnis.
    Tmpfs(PathBuf),
    /// Ein leeres Verzeichnis, etwa ein Einhängepunkt des Shims im Gast.
    Dir(PathBuf),
}

/// Eine Maske unter `/work`, relativ zum Projekt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkMask {
    /// Ein leeres Verzeichnis über einem vorhandenen.
    Tmpfs(PathBuf),
    /// Eine leere Datei über einer vorhandenen.
    Empty(PathBuf),
}

/// Was die Maschine bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Machine {
    /// Virtuelle CPUs.
    pub(crate) cpus: u32,
    /// Speicher in MiB.
    pub(crate) memory_mib: u64,
}

/// Der Pfad `path` (absolut) unter `base`.
fn under(base: &str, path: &Path) -> PathBuf {
    let relative = path.strip_prefix("/").unwrap_or(path);
    Path::new(base).join(relative)
}

/// Der Anfang jedes Gefängnisses: nichts geteilt, keine Fähigkeit, nur das
/// `/usr` des Hosts.
fn jail(bubblewrap: &Path) -> Vec<OsString> {
    let mut line: Vec<OsString> = vec![bubblewrap.as_os_str().to_owned()];
    line.extend(
        [
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
        ]
        .map(OsString::from),
    );
    line
}

fn push(line: &mut Vec<OsString>, parts: &[&str]) {
    line.extend(parts.iter().map(OsString::from));
}

/// Bindet `dir` unter demselben Pfad, beschreibbar oder nur lesbar.
fn bind_dir(line: &mut Vec<OsString>, dir: &Path, writable: bool) {
    line.push(OsString::from(if writable {
        "--bind"
    } else {
        "--ro-bind"
    }));
    line.push(dir.as_os_str().to_owned());
    line.push(dir.as_os_str().to_owned());
}

fn push_path(line: &mut Vec<OsString>, flag: &str, path: &Path) {
    line.push(OsString::from(flag));
    line.push(path.as_os_str().to_owned());
}

/// Ein Programm außerhalb von `/usr` wird in sein Gefängnis gebunden.
fn bind_program(line: &mut Vec<OsString>, program: &Path) {
    if !program.starts_with("/usr") {
        line.push(OsString::from("--ro-bind"));
        line.push(program.as_os_str().to_owned());
        line.push(program.as_os_str().to_owned());
    }
}

/// Die Optionen jedes virtiofsd: kein eigenes Sandboxing (das Gefängnis ist
/// es), nur Fehler ins Protokoll, kein Cache, der Inodes hortet, und keine
/// Ankündigung von Einhängepunkten, wenn er die Option kennt. Die Grenze der
/// Deskriptoren setzt die Unit ([`FILE_DAEMON_NOFILE`]).
///
/// `--cache never` (HUM-469): Der Gast bekommt jeden Eintrag mit der
/// Gültigkeit null und verwirft ihn, sobald ihn niemand mehr hält; dann
/// schickt er `FORGET`, und virtiofsd schließt den Deskriptor des Inodes. Mit
/// der Vorgabe `auto` (eine Sekunde) bleiben die Einträge im Cache des Gasts,
/// bis dessen Speicher knapp wird, und mit ihnen je ein Deskriptor im
/// virtiofsd; so kam der Walk in HUM-445 auf rund 520 000. Die Daten gehen
/// dann am Seitencache des Gasts vorbei; `--allow-mmap` erlaubt trotzdem
/// geteilte Abbildungen (rustc und andere bilden Dateien mit `MAP_SHARED`
/// ab), ab Gastkernel 6.6 (geprüft in `prerequisites`). Die Grenze davon:
/// virtiofsd empfiehlt `--allow-mmap` nur bei alleinigem Zugriff auf das
/// Verzeichnis. Ändert der Nutzer auf dem Host eine Datei unter `/work`,
/// während der Gast sie geteilt abgebildet hat, kann die Abbildung im Gast
/// veraltete Seiten zeigen. Das betrifft die Kohärenz der Daten, keine
/// Grenze der Sandbox: Der Gast sieht nur, was ihm ohnehin freigegeben ist.
/// Ohne die Option scheitern rustc und cargo im Gast. `--inode-file-handles`
/// hilft hier nicht: Ohne `CAP_DAC_READ_SEARCH` auf dem Host fällt virtiofsd
/// auf Deskriptoren zurück.
fn file_daemon_tail(line: &mut Vec<OsString>, resolved: &Resolved) {
    push(
        line,
        &[
            "--sandbox",
            "none",
            "--log-level",
            "error",
            "--cache",
            "never",
            "--allow-mmap",
        ],
    );
    if resolved.no_announce_submounts {
        push(line, &["--no-announce-submounts"]);
    }
}

/// Gefängnis 1: virtiofsd der Wurzel, nur lesbar, Socket als Deskriptor.
pub(crate) fn root_line(resolved: &Resolved, entries: &[RootEntry]) -> Vec<OsString> {
    let mut line = jail(&resolved.bubblewrap);
    push(&mut line, &["--dir", GUEST_ROOT]);
    for entry in entries {
        match entry {
            RootEntry::Bind { source, target } => {
                line.push(OsString::from("--ro-bind"));
                line.push(source.as_os_str().to_owned());
                line.push(under(GUEST_ROOT, target).into_os_string());
            }
            RootEntry::Symlink { link, target } => {
                line.push(OsString::from("--symlink"));
                line.push(link.as_os_str().to_owned());
                line.push(under(GUEST_ROOT, target).into_os_string());
            }
            RootEntry::Tmpfs(target) => push_path(&mut line, "--tmpfs", &under(GUEST_ROOT, target)),
            RootEntry::Dir(target) => push_path(&mut line, "--dir", &under(GUEST_ROOT, target)),
        }
    }
    bind_program(&mut line, &resolved.file_daemon);
    line.push(resolved.file_daemon.as_os_str().to_owned());
    push(
        &mut line,
        &[
            "--shared-dir",
            GUEST_ROOT,
            "--fd",
            &ROOT_SOCKET_FD.to_string(),
            "--readonly",
        ],
    );
    file_daemon_tail(&mut line, resolved);
    line
}

/// Gefängnis 2: virtiofsd für `/work`, der Ordner als Deskriptor, die
/// Masken darüber; `empty` ist eine leere Datei für die Masken.
pub(crate) fn work_line(
    resolved: &Resolved,
    layout: &Layout,
    masks: &[WorkMask],
    empty: &Path,
) -> Vec<OsString> {
    let mut line = jail(&resolved.bubblewrap);
    bind_dir(&mut line, &layout.work_dir(), true);
    push(
        &mut line,
        &["--bind-fd", &PROJECT_FD.to_string(), WORK_SHARE],
    );
    for mask in masks {
        match mask {
            WorkMask::Tmpfs(relative) => {
                push_path(&mut line, "--tmpfs", &Path::new(WORK_SHARE).join(relative));
            }
            WorkMask::Empty(relative) => {
                line.push(OsString::from("--ro-bind"));
                line.push(empty.as_os_str().to_owned());
                line.push(Path::new(WORK_SHARE).join(relative).into_os_string());
            }
        }
    }
    bind_program(&mut line, &resolved.file_daemon);
    line.push(resolved.file_daemon.as_os_str().to_owned());
    push(&mut line, &["--shared-dir", WORK_SHARE]);
    push_path(&mut line, "--socket-path", &layout.work_socket());
    file_daemon_tail(&mut line, resolved);
    line
}

/// Gefängnis 3: das vsock-Gerät dieser einen Sitzung.
pub(crate) fn device_line(resolved: &Resolved, layout: &Layout, group: &str) -> Vec<OsString> {
    let mut line = jail(&resolved.bubblewrap);
    bind_dir(&mut line, &layout.device_dir(), true);
    bind_dir(&mut line, &layout.door_dir(), true);
    bind_program(&mut line, &resolved.vsock_device);
    line.push(resolved.vsock_device.as_os_str().to_owned());
    push(&mut line, &["--guest-cid", &GUEST_CID.to_string()]);
    push_path(&mut line, "--socket", &layout.device_socket());
    push_path(&mut line, "--uds-path", &layout.door_device());
    push(&mut line, &["--groups", group]);
    line
}

/// Gefängnis 4: die Maschine. `init` ist der Shim im Gast, `plan` der
/// Gastplan im Gast.
pub(crate) fn machine_line(
    resolved: &Resolved,
    layout: &Layout,
    machine: Machine,
    init: &Path,
    plan: &Path,
) -> Vec<OsString> {
    let mut line = jail(&resolved.bubblewrap);
    // Nur die drei vhost-user-Sockets, nur lesbar (ein `connect` braucht
    // keinen beschreibbaren Mount); die Tür und die Übergabe sieht der
    // Emulator nicht.
    for dir in [layout.root_dir(), layout.work_dir(), layout.device_dir()] {
        bind_dir(&mut line, &dir, false);
    }
    push(&mut line, &["--dev-bind", "/dev/kvm", "/dev/kvm"]);
    line.push(OsString::from("--ro-bind"));
    line.push(resolved.kernel.as_os_str().to_owned());
    line.push(OsString::from("/kernel"));
    bind_program(&mut line, &resolved.emulator);
    line.push(resolved.emulator.as_os_str().to_owned());
    let memory = machine.memory_mib.to_string();
    push(
        &mut line,
        &[
            "-M",
            "microvm,pcie=on,acpi=on",
            "-accel",
            "kvm",
            "-cpu",
            "host",
            "-smp",
            &machine.cpus.to_string(),
            "-m",
            &memory,
            "-no-reboot",
            "-nodefaults",
            "-no-user-config",
            "-display",
            "none",
            "-serial",
            "none",
            "-monitor",
            "none",
            "-nic",
            "none",
            "-sandbox",
            "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny",
            "-object",
            &format!("memory-backend-memfd,id=mem,size={memory}M,share=on"),
            "-machine",
            "memory-backend=mem",
            "-kernel",
            "/kernel",
            "-append",
            &format!(
                "rootfstype=virtiofs root=rootfs ro panic=-1 quiet init={} -- --guest {}",
                init.display(),
                plan.display()
            ),
        ],
    );
    for (id, socket, device) in [
        (
            "fs0",
            layout.root_socket(),
            "vhost-user-fs-pci,chardev=fs0,tag=rootfs",
        ),
        (
            "fs1",
            layout.work_socket(),
            "vhost-user-fs-pci,chardev=fs1,tag=work",
        ),
        (
            "vs",
            layout.device_socket(),
            "vhost-user-vsock-pci,chardev=vs",
        ),
    ] {
        line.push(OsString::from("-chardev"));
        line.push(OsString::from(format!(
            "socket,id={id},path={}",
            socket.display()
        )));
        push(&mut line, &["-device", device]);
    }
    line
}

/// Die vier Zeilen einer Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lines {
    /// virtiofsd der Wurzel.
    pub(crate) root: Vec<OsString>,
    /// virtiofsd für `/work`.
    pub(crate) work: Vec<OsString>,
    /// Das vsock-Gerät.
    pub(crate) device: Vec<OsString>,
    /// Die Maschine.
    pub(crate) machine: Vec<OsString>,
}

/// Eine Zeile als Text für die Shell, jedes Wort wörtlich zitiert.
///
/// # Errors
///
/// Ein Satz, wenn ein Wort kein UTF-8 ist oder sich nicht zitieren lässt.
fn quoted(line: &[OsString]) -> Result<String, String> {
    let words: Vec<&str> = line
        .iter()
        .map(|word| {
            word.to_str().ok_or_else(|| {
                format!(
                    "{} is not UTF-8 and cannot go into the unit",
                    word.display()
                )
            })
        })
        .collect::<Result<_, _>>()?;
    shlex::try_join(words).map_err(|err| format!("a word of the line cannot be quoted: {err}"))
}

/// Das Skript des ersten Prozesses der Unit (POSIX `sh`), nach der Übergabe
/// mit dem Socket der Wurzel auf 3, dem Projektordner auf 4 und dem Leseende
/// der Lebensleine auf 5.
///
/// Es startet die vier Gefängnisse, jedes nur mit dem Deskriptor, den es
/// braucht, die Maschine erst, wenn beide Sockets der Dienste da sind. Endet
/// eines davon, oder kommt auf der Lebensleine das Ende (der Daemon ist fort,
/// oder er hat die Sitzung beendet), beendet das Skript sich selbst; mit dem
/// ersten Prozess endet die Unit, und der Dienstverwalter beendet jeden
/// übrigen Prozess (`KillMode=control-group`). Der Gast muss dafür nichts
/// tun.
///
/// # Errors
///
/// Der Satz aus [`quoted`].
pub(crate) fn script(lines: &Lines, layout: &Layout) -> Result<String, String> {
    let dir = quoted(&[layout.dir.clone().into_os_string()])?;
    let root = quoted(&lines.root)?;
    let work = quoted(&lines.work)?;
    let device = quoted(&lines.device)?;
    let machine = quoted(&lines.machine)?;
    Ok(format!(
        "set -u\n\
         r={dir}\n\
         ( read -r _ <&{LIFELINE_FD}; kill -TERM $$ ) {ROOT_SOCKET_FD}<&- {PROJECT_FD}<&- &\n\
         ( {root} {PROJECT_FD}<&- {LIFELINE_FD}<&-; kill -TERM $$ ) &\n\
         ( {work} {ROOT_SOCKET_FD}<&- {LIFELINE_FD}<&-; kill -TERM $$ ) &\n\
         exec {ROOT_SOCKET_FD}<&- {PROJECT_FD}<&-\n\
         ( {device} {LIFELINE_FD}<&-; kill -TERM $$ ) &\n\
         n=0\n\
         while [ ! -S \"$r/work/work.sock\" ] || [ ! -S \"$r/vsock/vsock.sock\" ]; do\n\
         \x20 n=$((n+1))\n\
         \x20 if [ \"$n\" -gt 500 ]; then echo 'humanitl: a helper of the kernel level did not open its socket' >&2; exit 70; fi\n\
         \x20 sleep 0.02\n\
         done\n\
         ( {machine} {LIFELINE_FD}<&-; kill -TERM $$ ) &\n\
         exec {LIFELINE_FD}<&-\n\
         wait\n"
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn resolved() -> Resolved {
        Resolved {
            emulator: PathBuf::from("/usr/bin/qemu-system-x86_64"),
            file_daemon: PathBuf::from("/usr/libexec/virtiofsd"),
            vsock_device: PathBuf::from("/opt/vsock/vhost-device-vsock"),
            bubblewrap: PathBuf::from("/usr/bin/bwrap"),
            kernel: PathBuf::from("/boot/vmlinuz-7.3"),
            modules: PathBuf::from("/usr/lib/modules/7.3"),
            no_announce_submounts: true,
        }
    }

    fn layout() -> Layout {
        Layout {
            dir: PathBuf::from("/run/user/1000/humanitl-vm-0123/abcd"),
        }
    }

    fn lines() -> Lines {
        let resolved = resolved();
        let layout = layout();
        Lines {
            root: root_line(
                &resolved,
                &[
                    RootEntry::Bind {
                        source: PathBuf::from("/usr"),
                        target: PathBuf::from("/usr"),
                    },
                    RootEntry::Symlink {
                        link: PathBuf::from("usr/bin"),
                        target: PathBuf::from("/bin"),
                    },
                    RootEntry::Tmpfs(PathBuf::from("/tmp")),
                    RootEntry::Dir(PathBuf::from("/work")),
                ],
            ),
            work: work_line(
                &resolved,
                &layout,
                &[
                    WorkMask::Tmpfs(PathBuf::from(".git/hooks")),
                    WorkMask::Empty(PathBuf::from(".env")),
                ],
                &layout.files().join("empty"),
            ),
            device: device_line(&resolved, &layout, "hum-abcd"),
            machine: machine_line(
                &resolved,
                &layout,
                Machine {
                    cpus: 2,
                    memory_mib: 1024,
                },
                Path::new("/usr/local/bin/humanitl-shim"),
                Path::new("/run/humanitl/guest-plan.json"),
            ),
        }
    }

    fn words(line: &[OsString]) -> Vec<String> {
        line.iter()
            .map(|word| word.to_string_lossy().into_owned())
            .collect()
    }

    /// `needle` steht in `line` als zusammenhängende Folge.
    fn has(line: &[OsString], needle: &[&str]) -> bool {
        let words = words(line);
        words
            .windows(needle.len())
            .any(|window| window.iter().zip(needle).all(|(a, b)| a == b))
    }

    fn count(line: &[OsString], word: &str) -> usize {
        words(line).iter().filter(|w| *w == word).count()
    }

    /// Die Verbotsliste der Spezifikation, für alle vier Zeilen.
    fn forbidden(lines: &Lines) -> Vec<String> {
        let mut found = Vec::new();
        for (name, line) in [
            ("root", &lines.root),
            ("work", &lines.work),
            ("device", &lines.device),
            ("machine", &lines.machine),
        ] {
            let words = words(line);
            for (index, word) in words.iter().enumerate() {
                let next = words.get(index + 1).map_or("", String::as_str);
                let bad = match word.as_str() {
                    // Ein Zeichengerät nur als vhost-user-Socket im
                    // Laufzeitverzeichnis, nie eine Konsole.
                    "-chardev" => !next.starts_with("socket,") || !next.contains(",path=/run/"),
                    "-netdev" | "--forward-cid" | "--forward-listen" | "--vm" | "--config"
                    | "--share-net" | "-net" | "-qmp" | "-gdb" | "-s" | "-S" | "-vnc"
                    | "-spice" | "-nographic" => true,
                    "-nic" | "-serial" | "-monitor" | "-display" | "-parallel" => next != "none",
                    "-device" => {
                        next.contains("net")
                            || next.starts_with("vhost-vsock")
                            || next.contains("serial")
                            || next.contains("console")
                    }
                    "--dev-bind" => next != "/dev/kvm",
                    _ => false,
                };
                if bad {
                    found.push(format!("{name}: {word} {next}"));
                }
            }
        }
        // Ein Gerät, ein Lauscher: genau ein `--socket` und ein `--uds-path`.
        for flag in ["--socket", "--uds-path", "--guest-cid", "--groups"] {
            if count(&lines.device, flag) != 1 {
                found.push(format!("device: {flag} not exactly once"));
            }
        }
        found
    }

    #[test]
    fn every_line_carries_its_fixed_options() {
        let lines = lines();
        for line in [&lines.root, &lines.work, &lines.device, &lines.machine] {
            assert!(
                has(
                    line,
                    &[
                        "/usr/bin/bwrap",
                        "--unshare-all",
                        "--die-with-parent",
                        "--new-session"
                    ]
                ),
                "{line:?}"
            );
            assert!(has(line, &["--cap-drop", "ALL"]));
        }
        // Die Wurzel: ein Unterverzeichnis, nie `/`, nur lesbar, Socket als
        // Deskriptor 3, keine Ankündigung.
        assert!(has(
            &lines.root,
            &["--shared-dir", GUEST_ROOT, "--fd", "3", "--readonly"]
        ));
        assert!(has(&lines.root, &["--sandbox", "none"]));
        assert!(has(&lines.root, &["--no-announce-submounts"]));
        // Beide virtiofsd: kein Cache, der Inodes hortet, geteilte
        // Abbildungen trotzdem erlaubt; die Grenze der Deskriptoren kommt von
        // der Unit, nie als `--rlimit-nofile`, das über einer kleineren
        // geerbten Grenze den Start scheitern ließe (HUM-469).
        for line in [&lines.root, &lines.work] {
            assert!(has(line, &["--cache", "never", "--allow-mmap"]), "{line:?}");
            assert_eq!(count(line, "--cache"), 1, "{line:?}");
            assert!(
                !words(line).iter().any(|w| w.starts_with("--rlimit-nofile")),
                "{line:?}"
            );
        }
        assert!(has(&lines.root, &["--ro-bind", "/usr", "/guest/usr"]));
        assert!(has(&lines.root, &["--symlink", "usr/bin", "/guest/bin"]));
        assert!(has(&lines.root, &["--tmpfs", "/guest/tmp"]));
        assert!(has(&lines.root, &["--dir", "/guest/work"]));
        let dir = layout().dir.to_string_lossy().into_owned();
        assert!(
            !words(&lines.root).iter().any(|word| word.contains(&dir)),
            "the runtime directory stands in no argument of the root's jail"
        );
        // /work: der Ordner als Deskriptor 4, die Masken darüber.
        assert!(has(&lines.work, &["--bind-fd", "4", WORK_SHARE]));
        assert!(has(&lines.work, &["--tmpfs", "/share/.git/hooks"]));
        assert!(has(
            &lines.work,
            &["--shared-dir", WORK_SHARE, "--socket-path"]
        ));
        assert!(!has(&lines.work, &["--readonly"]));
        // Das Gerät: eigener Prozess, außerhalb von /usr gebunden, Gruppe der
        // Sitzung.
        assert!(has(
            &lines.device,
            &[
                "--ro-bind",
                "/opt/vsock/vhost-device-vsock",
                "/opt/vsock/vhost-device-vsock"
            ]
        ));
        assert!(has(&lines.device, &["--groups", "hum-abcd"]));
        // Die Maschine: /dev/kvm, nur die Kernel-Datei, keine Konsole.
        assert!(has(&lines.machine, &["--dev-bind", "/dev/kvm", "/dev/kvm"]));
        assert!(has(
            &lines.machine,
            &["--ro-bind", "/boot/vmlinuz-7.3", "/kernel"]
        ));
        assert!(!words(&lines.machine).iter().any(|w| w == "/boot"));
        assert!(has(&lines.machine, &["-smp", "2", "-m", "1024"]));
        assert!(has(
            &lines.machine,
            &["-serial", "none", "-monitor", "none", "-nic", "none"]
        ));
        assert!(has(
            &lines.machine,
            &[
                "-sandbox",
                "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny"
            ]
        ));
        assert!(has(
            &lines.machine,
            &["-object", "memory-backend-memfd,id=mem,size=1024M,share=on"]
        ));
        assert_eq!(count(&lines.machine, "-device"), 3);
        assert!(forbidden(&lines).is_empty(), "{:?}", forbidden(&lines));
    }

    /// Kein Gefängnis bindet das Laufzeitverzeichnis selbst; jedes nur die
    /// Verzeichnisse der Sockets, die es braucht, und der Emulator weder die
    /// Tür noch die Übergabe.
    #[test]
    fn no_jail_binds_the_runtime_directory_itself() {
        let lines = lines();
        let layout = layout();
        let root = layout.dir.to_string_lossy().into_owned();
        for line in [&lines.root, &lines.work, &lines.device, &lines.machine] {
            assert!(!words(line).contains(&root), "{line:?}");
        }
        let dir = |path: PathBuf| path.to_string_lossy().into_owned();
        let binds = |line: &[OsString], flag: &str| {
            words(line)
                .windows(2)
                .filter(|pair| pair[0] == flag)
                .map(|pair| pair[1].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(binds(&lines.work, "--bind"), [dir(layout.work_dir())]);
        assert_eq!(
            binds(&lines.device, "--bind"),
            [dir(layout.device_dir()), dir(layout.door_dir())]
        );
        assert!(binds(&lines.root, "--bind").is_empty());
        assert!(binds(&lines.machine, "--bind").is_empty());
        let machine_ro = binds(&lines.machine, "--ro-bind");
        for wanted in [layout.root_dir(), layout.work_dir(), layout.device_dir()] {
            assert!(machine_ro.contains(&dir(wanted)), "{machine_ro:?}");
        }
        assert!(!machine_ro.contains(&dir(layout.door_dir())));
        assert!(
            !words(&lines.machine)
                .iter()
                .any(|w| w.contains("handover") || w.contains("/files"))
        );
        assert!(has(
            &lines.machine,
            &[
                "-chardev",
                &format!("socket,id=vs,path={}", layout.device_socket().display())
            ]
        ));
    }

    /// Die Verbotsliste fängt, was sie fangen soll: Jede Mutation einer
    /// Zeile, die die Spezifikation verbietet, ist ein Fund.
    #[test]
    fn the_deny_list_catches_every_forbidden_option() {
        let mutations: &[(&str, &[&str])] = &[
            ("machine", &["-nic", "user"]),
            ("machine", &["-netdev", "user,id=n"]),
            ("machine", &["-serial", "stdio"]),
            ("machine", &["-monitor", "stdio"]),
            ("machine", &["-device", "virtio-net-pci"]),
            ("machine", &["-device", "vhost-vsock-pci,guest-cid=3"]),
            ("machine", &["-chardev", "pty,id=c"]),
            ("machine", &["--dev-bind", "/dev/net/tun", "/dev/net/tun"]),
            ("device", &["--forward-cid", "1"]),
            ("device", &["--vm", "guest-cid=4,socket=/x,uds-path=/y"]),
            ("device", &["--uds-path", "/run/x/second"]),
        ];
        for (which, extra) in mutations {
            let mut lines = lines();
            let line = match *which {
                "machine" => &mut lines.machine,
                _ => &mut lines.device,
            };
            line.extend(extra.iter().map(OsString::from));
            assert!(!forbidden(&lines).is_empty(), "{which} {extra:?} passed");
        }
        // Ohne `--no-announce-submounts` in der Hilfe steht es nicht in der Zeile.
        let old = Resolved {
            no_announce_submounts: false,
            ..resolved()
        };
        assert!(!has(&root_line(&old, &[]), &["--no-announce-submounts"]));
    }

    /// Das Skript: jedes Gefängnis nur mit seinem Deskriptor, die Lebensleine
    /// vor allem anderen, kein Wort ungeschützt.
    #[test]
    fn the_script_hands_each_jail_only_its_descriptor() {
        let script = script(&lines(), &layout()).unwrap();
        assert!(script.starts_with("set -u\n"));
        let lifeline = script.find("read -r _ <&5").unwrap();
        let first_jail = script.find("bwrap").unwrap();
        assert!(lifeline < first_jail, "{script}");
        assert!(script.contains(
            "--fd 3 --readonly --sandbox none --log-level error --cache never --allow-mmap \
             --no-announce-submounts 4<&- 5<&-"
        ));
        assert!(script.contains("3<&- 5<&-; kill -TERM $$"));
        assert!(script.contains("exec 3<&- 4<&-\n"));
        assert!(script.contains("exec 5<&-\nwait\n"));
        let parsed: Vec<String> = shlex::split(&script.replace('\n', " ; ")).unwrap_or_default();
        assert!(!parsed.is_empty());
        // Ein Pfad mit einem Leerzeichen bleibt ein Wort.
        let odd = Layout {
            dir: PathBuf::from("/run/user/1000/a b"),
        };
        assert!(script_with(&odd).contains("r='/run/user/1000/a b'"));
    }

    fn script_with(layout: &Layout) -> String {
        script(&lines(), layout).unwrap()
    }
}
