//! Die Kernel-Stufe als eigenes Backend: eine QEMU-micro-VM mit eigenem
//! Gastkernel aus Debian-Paketen (HUM-464, HUM-444, ADR-0021).
//!
//! **Eine Sitzung ist eine Unit.** Jede Sitzung startet als vorübergehende,
//! gehärtete Unit des Dienstverwalters des Nutzers (`systemd-run --user`,
//! HUM-434) mit den Eigenschaften aus HUM-442 Q7: kein Netz
//! (`PrivateNetwork`), eigener Nutzer-Namensraum, keine Fähigkeit,
//! `NoNewPrivileges`, das Dateisystem nur lesbar außer dem
//! Laufzeitverzeichnis der Sitzung und dem Projektordner, nur `AF_UNIX` und
//! `AF_NETLINK`. In ihr laufen vier Prozesse, jeder in einem eigenen
//! bwrap-Gefängnis ([`lines`]): virtiofsd der Wurzel, virtiofsd für `/work`,
//! vhost-device-vsock und QEMU.
//!
//! **Nichts wird vererbt.** Die Unit ist kein Kind des Daemons. Ihr erster
//! Prozess ist der Shim im Modus `--fds-then-exec`: Er holt über einen
//! Socket der Sitzung ([`crate::fd_handover`]) den Socket der Wurzel, den
//! geöffneten und geprüften Projektordner und das Leseende der Lebensleine
//! und führt dann das Skript aus [`lines::script`] aus. Der Projektordner
//! erreicht den Gast also als der Deskriptor, den der Plan geprüft hat
//! (`--bind-fd`, HUM-442 Q4), und das Laufzeitverzeichnis steht nie in der
//! Wurzel des Gasts.
//!
//! **Ende.** Ein Stopp geht über den Dienstverwalter ([`UnitStop`]); danach
//! entfernt der Daemon das Laufzeitverzeichnis. Endet der Daemon, endet die
//! Unit, ohne dass der Gast mitwirkt: Das Skript wartet auf das Leseende der
//! Lebensleine, deren Schreibende nur der Daemon hält, und beendet sich bei
//! ihrem Ende; mit dem ersten Prozess beendet der Dienstverwalter alle
//! (`KillMode=control-group`). Läuft der Daemon selbst als Dienst, bindet
//! die Unit sich zusätzlich mit `BindsTo` an dessen Unit. Was eine frühere
//! Sitzung liegen ließ, räumt [`find_leftovers`] beim Start des Daemons weg
//! (`SANDBOX_041`).
//!
//! **Prüfung.** `isolation_check` liest die `CHECK`-Zeilen aus dem Gast über
//! den Berichts-Port ([`session`]) und prüft dazu den VMM auf dem Host
//! ([`host_check`]); scheitert die Host-Prüfung, ist das `SANDBOX_047`, und
//! die Sitzung wird beendet. Der Agent wartet im Gast vor seinem `exec`, bis
//! der Daemon auf der Verbindung des Berichts sein Urteil schickt
//! ([`crate::launcher::Admission`]): `ADMIT` nur, wenn beide Prüfungen
//! bestehen. Ein roter Befund lässt den Agenten also nie laufen. Ein Gast, der ohne Bericht endet, ist eine
//! gescheiterte Prüfung ([`session::NO_REPORT`]).
//!
//! **Nicht zugelassen.** Die Stufe steht nicht in
//! [`crate::ADMITTED_BACKENDS`]; jeder Start lehnt sie mit `SANDBOX_048` ab,
//! bis HUM-449 landet. Erreichbar ist der Adapter nur über
//! `Backend::detect_unadmitted` (Escape-Suite, Tests).
//!
//! Die Namen `qemu`, `virtiofsd`, `vhost-device-vsock` und `microvm` stehen
//! nur in diesem Verzeichnis (`[qemu]` in `tools/coupling-baseline.toml`).
//! Außerhalb heißt die Stufe Kernel-Stufe oder VM-Backend.

mod host_check;
#[cfg(test)]
mod kvm_tests;
mod lines;
mod prerequisites;
mod session;
#[cfg(test)]
mod test_vm;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use humanitl_config::{Paths, QemuConfig, SandboxRef, WorkMode};
use humanitl_core::diagnostics::codes::{
    SANDBOX_006, SANDBOX_011, SANDBOX_012, SANDBOX_030, SANDBOX_041, SANDBOX_046, SANDBOX_047,
};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::bwrap::{BwrapBackend, PinnedMounts, isolation_from_report, launch_plan};
use crate::doctor::BackendCheckId;
use crate::handle::SandboxHandle;
use crate::launcher::{
    CheckResult, Detached, Door, EnvDelivery, IsolationCheck, IsolationLevel, LaunchOnce,
    LaunchPlan, PlanPreview, PlanRequest, PolicyMode, PolicyOrigin, PolicyRow, ResourceLimits,
    SandboxBackend, Teardown, UnitStop,
};
use crate::profile::{MountPolicy, SandboxProfile, SessionContext};
use crate::user_unit::{UnitName, UserUnits};

use prerequisites::missing_prerequisite;
use prerequisites::{Prerequisite, Resolved};

use lines::{Layout, Lines, Machine, RootEntry, WorkMask};

/// Der vsock-Port des Hosts, der zum Proxy führt (die Tür).
pub(crate) const DOOR_PORT: u32 = 5000;
/// Der Port, an dem der Shim im Gast Bericht und Steuerkanal ausliefert.
pub(crate) const REPORT_PORT: u32 = 5001;

/// Wo der Gastplan in der Wurzel des Gasts liegt.
pub(crate) const GUEST_PLAN: &str = "/run/humanitl/guest-plan.json";
/// Wo die Module des Gastkernels in der Wurzel des Gasts liegen.
pub(crate) const GUEST_MODULES: &str = "/run/humanitl/modules";
/// Der Tag des Projektordners.
const WORK_TAG: &str = "work";

/// Die Beschreibung der Unit.
const UNIT_DESCRIPTION: &str = "Humanitl kernel level session";
/// Der Anfang des Verzeichnisses je Daemon unter dem Laufzeitverzeichnis des
/// Nutzers; danach die Instanz.
const INSTANCE_DIR_PREFIX: &str = "humanitl-vm-";
/// Der Suchpfad in der Unit.
const UNIT_PATH: &str = "/usr/bin:/bin";
/// Das Zeichen der Übergabe in der Vorschau.
const PREVIEW_TOKEN: &str = "<handover-token>";

#[cfg(test)]
thread_local! {
    /// Nur für den Mutationsbeweis der Tests: weitere Wörter am Ende der
    /// Zeile der Maschine (etwa `-nic user`), je Faden, damit parallele
    /// Tests einander nichts unterschieben.
    pub(crate) static EXTRA_MACHINE_ARGS: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Das VM-Backend der Kernel-Stufe.
#[derive(Debug, Clone)]
pub struct VmBackend {
    paths: Paths,
    config: QemuConfig,
    resolved: Resolved,
}

impl VmBackend {
    /// Prüft jede Voraussetzung (`SANDBOX_045`) und hält ihre Pfade.
    ///
    /// # Errors
    ///
    /// `SANDBOX_045` mit der ersten fehlenden Voraussetzung.
    pub(crate) fn detect(paths: Paths, config: &QemuConfig) -> Result<Self, Diagnostic> {
        let resolved = prerequisites::resolve(paths.env(), config)?;
        Ok(Self {
            paths,
            config: config.clone(),
            resolved,
        })
    }

    /// Das Programm, das die Maschine ist.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.resolved.emulator
    }

    /// Keine eigene Fassung: Die Fähigkeiten kommen aus der Hilfe.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "the same method as every other backend, called through Backend"
    )]
    pub fn tool_version(&self) -> String {
        String::new()
    }

    /// Die Pfade, gegen die das Backend plant.
    #[must_use]
    pub const fn paths(&self) -> &Paths {
        &self.paths
    }

    /// vCPU und Speicher aus `sandbox.qemu`, sonst vom Host abgeleitet: die
    /// Hälfte seiner CPUs (1 bis 4) und seines Speichers (256 bis 4096 MiB).
    fn machine(&self) -> Machine {
        let host_cpus =
            std::thread::available_parallelism().map_or(1, |n| u32::try_from(n.get()).unwrap_or(1));
        let host_mib = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("MemTotal:"))
                    .and_then(|rest| {
                        rest.trim()
                            .trim_end_matches("kB")
                            .trim()
                            .parse::<u64>()
                            .ok()
                    })
            })
            .map_or(1024, |kib| kib / 1024);
        Machine {
            cpus: self
                .config
                .cpus
                .unwrap_or_else(|| (host_cpus / 2).clamp(1, 4)),
            memory_mib: self
                .config
                .memory_mib
                .unwrap_or_else(|| (host_mib / 2).clamp(256, 4096)),
        }
    }

    /// Die Frist bis zum Bericht des Gasts.
    fn start_timeout(&self) -> Duration {
        Duration::from_secs(self.config.start_timeout_secs.clamp(1, 600))
    }
}

/// Die Wurzel des Gasts aus dem Plan wie unter bwrap: Zeilen der Wurzel,
/// Masken unter `/work`, die Dateien, die der Plan dafür schreibt, und die
/// Tabelle für die Vorschau.
struct GuestRoot {
    entries: Vec<RootEntry>,
    masks: Vec<WorkMask>,
    files: Vec<(PathBuf, Vec<u8>)>,
    policy: Vec<PolicyRow>,
}

/// Der Inhalt einer Datei aus dem Speicher des Launchers in der Wurzel des
/// Gasts: eine Identitätsdatei, eine Datei des Adapters, sonst eine Maske.
fn masked_content(
    target: &Path,
    identity: &crate::IdentityFiles,
    session: &SessionContext,
) -> Vec<u8> {
    if target == Path::new(crate::PASSWD_DST) {
        identity.passwd.as_bytes().to_vec()
    } else if target == Path::new(crate::GROUP_DST) {
        identity.group.as_bytes().to_vec()
    } else if target == Path::new(crate::HOSTS_DST) {
        identity.hosts.as_bytes().to_vec()
    } else if let Some(file) = session.files.iter().find(|file| file.dst == target) {
        file.content.clone()
    } else {
        Vec::new()
    }
}

/// Übersetzt die Zeilen von bwrap in die Wurzel des Gasts. `files` ist das
/// Verzeichnis, in dem die Dateien des Gasts liegen werden; `session_ro` sind
/// die nur lesbaren Binds der Sitzung ([`crate::LaunchOptions::session_ro`],
/// das Testverzeichnis der Escape-Suite), als geprüfte Quelle und Ziel.
fn guest_root(
    profile: &SandboxProfile,
    session: &SessionContext,
    present: &BTreeSet<PathBuf>,
    files: &Path,
    modules: &Path,
    session_ro: &[(PathBuf, PathBuf)],
) -> GuestRoot {
    let mut inputs = crate::bwrap_args::LaunchInputs::preview_with_agent_files(
        session.files.iter().map(|file| file.dst.clone()),
    );
    inputs.present_under_work = Some(present.clone());
    let line = profile.to_bwrap_line(session, &inputs);
    let identity = profile.identity_files(
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    let work = profile.mounts.work.dst.as_path();
    let mut root = GuestRoot {
        entries: Vec::new(),
        masks: Vec::new(),
        files: Vec::new(),
        policy: Vec::new(),
    };
    let empty = files.join("empty");
    root.files.push((empty.clone(), Vec::new()));
    for row in line.policy {
        let target = row.target.clone();
        if target == work {
            root.entries.push(RootEntry::Dir(target));
            root.policy.push(row);
            continue;
        }
        if let Ok(relative) = target.strip_prefix(work) {
            match row.mode {
                PolicyMode::Tmpfs => root.masks.push(WorkMask::Tmpfs(relative.to_path_buf())),
                PolicyMode::Masked => root.masks.push(WorkMask::Empty(relative.to_path_buf())),
                _ => continue,
            }
            root.policy.push(row);
            continue;
        }
        match row.mode {
            PolicyMode::ReadOnly | PolicyMode::ReadWrite => {
                let Some(source) = row.source.clone() else {
                    continue;
                };
                // Die Tür ist im Gast ein vsock-Port, keine Datei.
                if source == session.proxy_socket_src {
                    continue;
                }
                root.entries.push(RootEntry::Bind { source, target });
                // Die Wurzel des Gasts ist nur lesbar, auch für
                // `mounts.extra_rw`.
                root.policy.push(PolicyRow {
                    mode: PolicyMode::ReadOnly,
                    ..row
                });
            }
            PolicyMode::Symlink => {
                if let Some(link) = row.link.clone() {
                    root.entries.push(RootEntry::Symlink { link, target });
                    root.policy.push(row);
                }
            }
            PolicyMode::Tmpfs => {
                root.entries.push(RootEntry::Tmpfs(target));
                root.policy.push(row);
            }
            PolicyMode::Proc | PolicyMode::Dev => {
                root.entries.push(RootEntry::Dir(target));
                root.policy.push(row);
            }
            PolicyMode::Masked => {
                let content = masked_content(&target, &identity, session);
                let source = files.join(format!("root-{}", root.files.len()));
                root.files.push((source.clone(), content));
                root.entries.push(RootEntry::Bind { source, target });
                root.policy.push(row);
            }
            PolicyMode::Denied | PolicyMode::Image => {}
        }
    }
    add_session_binds(&mut root, session_ro);
    root.entries.push(RootEntry::Dir(PathBuf::from("/sys")));
    root.entries.push(RootEntry::Bind {
        source: modules.to_path_buf(),
        target: PathBuf::from(GUEST_MODULES),
    });
    root.entries.push(RootEntry::Bind {
        source: files.join("guest-plan.json"),
        target: PathBuf::from(GUEST_PLAN),
    });
    for (source, target) in [
        (modules.to_path_buf(), GUEST_MODULES),
        (files.join("guest-plan.json"), GUEST_PLAN),
    ] {
        root.policy.push(PolicyRow::new(
            PolicyMode::ReadOnly,
            Some(source),
            PathBuf::from(target),
            PolicyOrigin::Session,
        ));
    }
    root
}

/// Die nur lesbaren Binds der Sitzung, wie unter bwrap hinter den
/// Erweiterungen des Profils: Die Zeile der Vorschau kennt sie nicht, sie
/// kommen aus dem Aufruf (HUM-445).
fn add_session_binds(root: &mut GuestRoot, session_ro: &[(PathBuf, PathBuf)]) {
    for (source, target) in session_ro {
        root.entries.push(RootEntry::Bind {
            source: source.clone(),
            target: target.clone(),
        });
        root.policy.push(PolicyRow::new(
            PolicyMode::ReadOnly,
            Some(source.clone()),
            target.clone(),
            PolicyOrigin::Session,
        ));
    }
}

/// Der Gastplan (`guest.rs` des Shims).
fn guest_plan(
    profile: &SandboxProfile,
    session: &SessionContext,
    env: &[(String, String)],
    project_ino: u64,
    root_probe: Option<&str>,
) -> Result<String, Diagnostic> {
    let (agent_env, shim_env) = crate::shim_fds::split_env(env);
    let entry = |point: &str, fstype: &str, source: &str, mode: &str, flags: &str| {
        serde_json::json!({
            "point": point, "type": fstype, "source": source,
            "mode": mode, "flags": flags, "count": "1",
        })
    };
    // Ohne Ankündigung von Einhängepunkten (`lines`) sieht der Gast unter
    // `/` und `/work` je genau eine virtiofs-Einhängung. Über `/sys` liegt
    // nach der Modulsperre ein leeres, nur lesbares tmpfs statt des sysfs
    // des Gastkerns, und `/dev/shm` ist ein tmpfs wie unter bwrap (HUM-445).
    let mounts = vec![
        entry("/", "virtiofs", "type", "ro", "*"),
        entry("/proc", "proc", "type", "*", "*"),
        entry("/sys", "tmpfs", "type", "ro", "nosuid,nodev,noexec"),
        entry("/dev", "devtmpfs", "type", "*", "*"),
        entry("/dev/pts", "devpts", "type", "*", "*"),
        entry("/dev/shm", "tmpfs", "type", "rw", "nosuid,nodev"),
        entry(
            &profile.mounts.work.dst.display().to_string(),
            "virtiofs",
            &format!("tag:{WORK_TAG}:{project_ino}"),
            "rw",
            "nosuid,nodev",
        ),
    ];
    let command: Vec<&str> = session
        .command
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<_>>()
        .ok_or_else(|| {
            Diagnostic::builder(SANDBOX_011, Severity::Blocking)
                .why("the command of the agent is not UTF-8 and cannot go into the guest plan")
                .build()
        })?;
    let object = |pairs: &[(String, String)]| {
        pairs
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect::<serde_json::Map<_, _>>()
    };
    // Die Brücke des Gasts führt über vsock; seine Liste der Brücken gilt
    // nur unter bwrap.
    let shim: Vec<(String, String)> = shim_env
        .iter()
        .filter(|(key, _)| key.starts_with("HUMANITL_SECCOMP_"))
        .cloned()
        .collect();
    let mut plan = serde_json::json!({
        // Der Agent im Gast ist kein Root (HUM-445): dieselbe Kennung, die
        // bwrap ohne `--uid` in seine Sandbox abbildet und die das
        // `/etc/passwd` des Gasts nennt.
        "uid": rustix::process::getuid().as_raw().to_string(),
        "gid": rustix::process::getgid().as_raw().to_string(),
        "hostname": profile.sandbox.hostname,
        "proxy_port": profile.network.proxy_port.to_string(),
        "door_port": DOOR_PORT.to_string(),
        "report_port": REPORT_PORT.to_string(),
        "modules_dir": GUEST_MODULES,
        "work_tag": WORK_TAG,
        "shim": object(&shim),
        "env": object(&agent_env),
        "pins": {
            "pins": [{"path": profile.mounts.work.dst.display().to_string(), "dev": "*", "ino": project_ino.to_string()}],
            "mounts": mounts,
        },
        // Die beschreibbaren Orte des Gasts (HUM-444): In seiner nur
        // lesbaren Wurzel führt kein Socket irgendwohin.
        "socket_walk": [profile.mounts.work.dst.display().to_string(), "/dev"],
        "command": command,
    });
    if let Some(probe) = root_probe {
        plan["root_probe"] = serde_json::json!(["/bin/sh", "-c", probe]);
    }
    Ok(plan.to_string())
}

/// `SANDBOX_011` mit `why`.
fn setup_failed(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(why)
        .build()
}

/// Das Laufzeitverzeichnis des Nutzers: `/run/user/<uid>`, sonst
/// `XDG_RUNTIME_DIR` des Prozesses, jeweils nur, wenn es ein Verzeichnis
/// dieses Nutzers ist. Nie das von Humanitl: Dessen Pfad kann lang sein (ein
/// Test, ein Ersatz unter `$TMPDIR`), und Socket-Pfade tragen höchstens 108
/// Bytes.
fn user_runtime_dir() -> Option<PathBuf> {
    let uid = rustix::process::getuid().as_raw();
    let owned = |path: &Path| {
        std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && meta.uid() == uid)
    };
    let run_user = PathBuf::from(format!("/run/user/{uid}"));
    if owned(&run_user) {
        return Some(run_user);
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && owned(dir))
}

/// Das Verzeichnis dieses Daemons unter dem Laufzeitverzeichnis des
/// Nutzers: `humanitl-vm-<Instanz>`, die Instanz die ersten 16 Hex-Ziffern
/// von SHA-256 über das Laufzeitverzeichnis des Daemons. Zwei Daemons (ein
/// Test neben dem echten) räumen so nie die Sitzungen des anderen weg.
fn instance_dir(paths: &Paths) -> Option<PathBuf> {
    use sha2::{Digest as _, Sha256};
    let root = paths.runtime_dir().path;
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    let instance = digest.iter().take(8).fold(String::new(), |mut hex, byte| {
        let _ = std::fmt::Write::write_fmt(&mut hex, format_args!("{byte:02x}"));
        hex
    });
    Some(user_runtime_dir()?.join(format!("{INSTANCE_DIR_PREFIX}{instance}")))
}

/// Das Laufzeitverzeichnis einer Sitzung, kurz genug für Socket-Pfade.
fn session_layout(paths: &Paths, session: &SessionContext) -> Result<Layout, Diagnostic> {
    let instance = instance_dir(paths).ok_or_else(|| {
        setup_failed(
            "the kernel level needs the user's runtime directory (/run/user/<uid>), and there \
             is none owned by this user"
                .to_owned(),
        )
    })?;
    let dir = instance.join(session.session.as_uuid().simple().to_string());
    // Die Sockets darin stehen in `-chardev socket,...,path=`.
    property_path(&dir)?;
    let longest = dir.join(format!("door_{DOOR_PORT}")).as_os_str().len();
    if longest >= session::SUN_PATH_MAX {
        return Err(setup_failed(format!(
            "the runtime directory {} is too long for a socket path ({longest} of {} bytes)",
            dir.display(),
            session::SUN_PATH_MAX
        )));
    }
    Ok(Layout { dir })
}

/// Legt das Laufzeitverzeichnis an: `0700`, auf tmpfs, neu.
fn create_layout(layout: &Layout) -> Result<(), Diagnostic> {
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    if let Some(parent) = layout.dir.parent() {
        builder
            .recursive(true)
            .create(parent)
            .map_err(|err| setup_failed(format!("{}: {err}", parent.display())))?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .map_err(|err| setup_failed(format!("{}: {err}", parent.display())))?;
    }
    builder
        .recursive(false)
        .create(&layout.dir)
        .map_err(|err| setup_failed(format!("{}: {err}", layout.dir.display())))?;
    for dir in std::iter::once(layout.files())
        .chain(Layout::SOCKET_DIRS.iter().map(|name| layout.dir.join(name)))
    {
        builder
            .create(&dir)
            .map_err(|err| setup_failed(format!("{}: {err}", dir.display())))?;
    }
    // Der Gastplan trägt die Umgebung des Agenten, die Dateien des Adapters
    // womöglich ein Geheimnis: Nichts davon landet auf einer Platte.
    crate::disposable::require_tmpfs(&layout.dir)
}

/// Ein Pfad, der als Wert einer Eigenschaft der Unit, als Wort ihrer Zeile
/// oder in einer Option von QEMU (`path=` eines `-chardev`, getrennt durch
/// Kommas) taugt: absolut, UTF-8, ohne Leerraum, Anführungszeichen,
/// Backslash, `%`, `;` oder `,`.
fn property_path(path: &Path) -> Result<String, Diagnostic> {
    let text = path.to_str().filter(|text| {
        path.is_absolute()
            && !text
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '\\' | '%' | ';' | ','))
    });
    text.map(str::to_owned).ok_or_else(|| {
        Diagnostic::builder(SANDBOX_006, Severity::Blocking)
            .why(format!(
                "the kernel level writes {} into its unit or into an option of the machine, \
                 and the path holds a space, a quote, a backslash, '%', ';' or ','; use a \
                 plain path or the bubblewrap backend",
                path.display()
            ))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.backend".to_owned(),
                value: crate::backend::BWRAP_BACKEND.to_owned(),
            })
            .build()
    })
}

/// Die Unit des Daemons, wenn er selbst als Dienst des Dienstverwalters
/// des Nutzers läuft: `INVOCATION_ID` ist gesetzt, und seine cgroup ist eine
/// `.service`-Unit unter `user@<uid>.service`. Ein Dienst des Systems (etwa
/// der Dienst, unter dem ein CI-Runner Tests startet) ist dem Dienstverwalter
/// des Nutzers unbekannt; ein `BindsTo` auf ihn beendete die Unit sofort.
fn daemon_unit() -> Option<String> {
    std::env::var_os("INVOCATION_ID")?;
    let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = cgroup.lines().find_map(|line| line.strip_prefix("0::"))?;
    user_service_of(path.trim(), rustix::process::getuid().as_raw())
}

/// Der Name der Dienst-Unit des Nutzers `uid` aus dem cgroup-Pfad `path`,
/// oder keiner, wenn der Pfad nicht unter `user@<uid>.service` liegt.
fn user_service_of(path: &str, uid: u32) -> Option<String> {
    let manager = format!("user@{uid}.service");
    let path = Path::new(path);
    if !path
        .components()
        .any(|part| part.as_os_str() == manager.as_str())
    {
        return None;
    }
    let name = path.file_name()?.to_str()?.to_owned();
    let valid = name.ends_with(".service")
        && name != manager
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'@'));
    valid.then_some(name)
}

/// Die Eigenschaften der Unit (HUM-442 Q7).
fn unit_properties(runtime: &str, project: Option<&str>) -> Vec<String> {
    let mut properties: Vec<String> = [
        "PrivateNetwork=yes",
        "PrivateUsers=yes",
        "NoNewPrivileges=yes",
        "CapabilityBoundingSet=",
        "ProtectSystem=strict",
        "ProtectHome=read-only",
        "RestrictAddressFamilies=AF_UNIX AF_NETLINK",
        "LockPersonality=yes",
        "RestrictRealtime=yes",
        "SystemCallArchitectures=native",
    ]
    .iter()
    .map(|property| (*property).to_owned())
    .collect();
    properties.push(format!(
        "TimeoutStopSec={}s",
        crate::KILL_GRACE.as_secs().max(1)
    ));
    // Unter `ProtectHome=read-only` bindet bwrap auch einen geerbten
    // Deskriptor nur lesbar; der Projektordner braucht seinen Pfad hier
    // (gemessen 2026-11-14, systemd 262).
    match project {
        Some(project) => properties.push(format!("ReadWritePaths={runtime} {project}")),
        None => properties.push(format!("ReadWritePaths={runtime}")),
    }
    if let Some(daemon) = daemon_unit() {
        properties.push(format!("BindsTo={daemon}"));
        properties.push(format!("After={daemon}"));
    }
    properties
}

/// Der Dienstverwalter des Nutzers, wie die Container-Engine ihn findet.
fn user_units(paths: &Paths) -> Result<UserUnits, Diagnostic> {
    let path = paths
        .env()
        .non_empty("PATH")
        .unwrap_or("/usr/local/bin:/usr/bin:/bin")
        .to_owned();
    let mut env = vec![("PATH".to_owned(), path.clone())];
    if let Some(dir) = user_runtime_dir() {
        env.push(("XDG_RUNTIME_DIR".to_owned(), dir.display().to_string()));
    }
    if let Ok(bus) = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        && !bus.is_empty()
    {
        env.push(("DBUS_SESSION_BUS_ADDRESS".to_owned(), bus));
    }
    let units = UserUnits::find(&path, env)?;
    #[cfg(test)]
    let units = units.with_prefix("humanitl-test-");
    Ok(units)
}

/// Die Gruppe des vsock-Geräts einer Sitzung: eigene je Sitzung (Q9).
fn session_group(session: &SessionContext) -> String {
    format!(
        "hum-{}",
        &session.session.as_uuid().simple().to_string()[..16]
    )
}

/// Die Unit einer Sitzung.
fn unit_of(units: &UserUnits, session: &SessionContext) -> Result<UnitName, Diagnostic> {
    units
        .named(&format!("vm-{}", session.session.as_uuid().simple()))
        .map_err(setup_failed)
}

/// Die Zeile der Unit samt Skript.
#[allow(clippy::too_many_arguments)]
fn unit_line(
    units: &UserUnits,
    unit: &UnitName,
    session: &SessionContext,
    layout: &Layout,
    lines: &Lines,
    token: &str,
    project: Option<&str>,
) -> Result<Vec<OsString>, Diagnostic> {
    let runtime = property_path(&layout.dir)?;
    let script = lines::script(lines, layout).map_err(setup_failed)?;
    let handover = layout.handover();
    let args: Vec<OsString> = [
        "--fds-then-exec",
        &handover.display().to_string(),
        token,
        "--",
        "/bin/sh",
        "-c",
        &script,
        "humanitl-vm",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    units
        .session_argv(
            UNIT_DESCRIPTION,
            &unit_properties(&runtime, project),
            unit,
            &session.shim_src,
            &args,
            &[("PATH".to_owned(), UNIT_PATH.to_owned())],
        )
        .map_err(setup_failed)
}

/// Was der Plan dem Start übergibt ([`Detached::backend_state`]).
struct Pending {
    door: UnixListener,
    proxy: PathBuf,
    door_lifeline: OwnedFd,
    link: session::GuestLink,
}

/// Die vier Zeilen einer Sitzung.
fn session_lines(
    resolved: &Resolved,
    layout: &Layout,
    machine: Machine,
    root: &GuestRoot,
    init: &Path,
    work_readonly: bool,
    group: &str,
) -> Lines {
    let mut work = lines::work_line(resolved, layout, &root.masks, &layout.files().join("empty"));
    if work_readonly {
        work.push(OsString::from("--readonly"));
    }
    #[cfg_attr(not(test), allow(unused_mut))]
    let mut machine = lines::machine_line(resolved, layout, machine, init, Path::new(GUEST_PLAN));
    #[cfg(test)]
    machine.extend(
        EXTRA_MACHINE_ARGS
            .with_borrow(|extra| extra.iter().map(OsString::from).collect::<Vec<_>>()),
    );
    Lines {
        root: lines::root_line(resolved, &root.entries),
        work,
        device: lines::device_line(resolved, layout, group),
        machine,
    }
}

/// `SANDBOX_046`: Die Tür ließ sich nicht aufbauen.
fn door_failed(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_046, Severity::Blocking)
        .why(why)
        .build()
}

impl SandboxBackend for VmBackend {
    fn name(&self) -> &'static str {
        crate::backend::VM_BACKEND
    }

    fn isolation(&self) -> IsolationLevel {
        crate::backend::isolation_of(crate::backend::VM_BACKEND, None)
    }

    /// Noch keine: Die Wegwerf-Schicht über virtiofs baut HUM-446.
    fn disposable_support(&self, _request: &PlanRequest<'_>) -> Result<(), Diagnostic> {
        Err(Diagnostic::builder(SANDBOX_030, Severity::Blocking)
            .why(
                "the kernel level has no disposable sandbox yet; set sandbox.mode = \"direct\", \
                 or use the bubblewrap backend for a disposable sandbox",
            )
            .fix(FixAction::ChangeSetting {
                key: "sandbox.mode".to_owned(),
                value: "direct".to_owned(),
            })
            .build())
    }

    fn preview(&self, request: &PlanRequest<'_>) -> PlanPreview {
        let PlanRequest {
            profile,
            session,
            options,
        } = *request;
        let layout = session_layout(&self.paths, session).unwrap_or_else(|_| Layout {
            dir: PathBuf::from(format!(
                "/run/user/<uid>/{INSTANCE_DIR_PREFIX}<instance>/<session>"
            )),
        });
        let present = BwrapBackend::present_under_work(profile, &session.work_src);
        let root = guest_root(
            profile,
            session,
            &present,
            &layout.files(),
            &self.resolved.modules,
            &options.session_ro,
        );
        let readonly = profile.effective_work_mode(session.work_mode) == WorkMode::Ro;
        let lines = session_lines(
            &self.resolved,
            &layout,
            self.machine(),
            &root,
            &profile.network.shim_dst,
            readonly,
            "hum-preview",
        );
        let argv = user_units(&self.paths)
            .and_then(|units| {
                let unit = unit_of(&units, session)?;
                unit_line(
                    &units,
                    &unit,
                    session,
                    &layout,
                    &lines,
                    PREVIEW_TOKEN,
                    (!readonly).then_some("<project>"),
                )
            })
            .unwrap_or_else(|_| {
                let mut argv = vec![OsString::from("/bin/sh"), OsString::from("-c")];
                argv.push(OsString::from(
                    lines::script(&lines, &layout).unwrap_or_default(),
                ));
                argv
            });
        let machine = self.machine();
        PlanPreview {
            argv,
            policy: root.policy,
            env: profile.effective_env(session, None),
            backend_env: Vec::new(),
            argv_env: Vec::new(),
            policy_text: None,
            door: Door::VsockPort {
                host: session.proxy_socket_src.clone(),
                port: DOOR_PORT,
            },
            limits: ResourceLimits {
                memory_bytes: Some(machine.memory_mib.saturating_mul(1024 * 1024)),
                cpu_millis: Some(machine.cpus.saturating_mul(1000)),
                pids: None,
            },
        }
    }

    /// Prüft, schreibt das Laufzeitverzeichnis und plant.
    ///
    /// # Errors
    ///
    /// Was bwrap für dieselbe Anfrage ablehnte (`SANDBOX_005`,
    /// `SANDBOX_006`, `SANDBOX_011`), `SANDBOX_030` für eine
    /// Wegwerf-Sandbox, `SANDBOX_050` ohne Dienstverwalter, `SANDBOX_046`,
    /// wenn die Tür nicht entsteht.
    fn plan(&self, request: &PlanRequest<'_>) -> Result<LaunchPlan, Diagnostic> {
        let PlanRequest {
            profile,
            session,
            options,
        } = *request;
        if options.disposable.is_some() {
            return Err(self
                .disposable_support(request)
                .err()
                .unwrap_or_else(|| setup_failed("no disposable sandbox".to_owned())));
        }
        let policy = MountPolicy::from_paths(&self.paths);
        profile.validate_with(&policy)?;
        BwrapBackend::check_work_dir(profile, session)?;
        policy.check_work_dir(&session.work_src)?;
        let pinned = PinnedMounts::open(&policy, request)?;
        BwrapBackend::check_proxy_socket_in(&self.paths, session)?;
        BwrapBackend::check_regular_file(&session.ca_cert_src, "CA certificate")?;
        BwrapBackend::check_regular_file(&session.ca_bundle_src, "CA bundle")?;
        BwrapBackend::check_shim(session)?;
        BwrapBackend::check_agent_files(session)?;
        // Jeder Pfad, der in die Zeile der Unit oder des Gasts geht, ist ein
        // einfaches Wort (`init=` in der Kernel-Zeile, Programme im Skript).
        for path in [
            &self.resolved.emulator,
            &self.resolved.file_daemon,
            &self.resolved.vsock_device,
            &self.resolved.bubblewrap,
            &self.resolved.kernel,
            &self.resolved.modules,
            &session.shim_src,
            &profile.network.shim_dst,
        ] {
            property_path(path)?;
        }
        let units = user_units(&self.paths)?;
        crate::user_unit::check_probe(&crate::user_unit::run_bounded(
            units.probe_command(),
            Duration::from_secs(10),
        ))?;
        let unit = unit_of(&units, session)?;

        let PinnedMounts {
            work,
            session: session_ro,
            ..
        } = pinned;
        let present = BwrapBackend::present_under_work(profile, &work.proc_path());
        let writable = profile.effective_work_mode(session.work_mode) == WorkMode::Rw;
        let project = Project {
            ino: std::fs::metadata(work.proc_path())
                .map_err(|err| setup_failed(format!("the project folder: {err}")))?
                .ino(),
            writable: if writable {
                Some(property_path(work.resolved())?)
            } else {
                None
            },
            unprotected: BwrapBackend::unprotected_under_work(profile, &present),
            work,
            session_ro: session_ro
                .iter()
                .map(|(pin, target)| (pin.resolved().to_path_buf(), target.clone()))
                .collect(),
        };
        let layout = session_layout(&self.paths, session)?;
        create_layout(&layout)?;
        let planned = self.plan_in(&layout, request, &units, &unit, project);
        if planned.is_err() {
            let _ = std::fs::remove_dir_all(&layout.dir);
        }
        planned
    }

    /// Startet die Fäden der Tür und des Gasts, dann die Unit wie bwrap
    /// (`launch_plan`); das Laufzeitverzeichnis geht mit der Sandbox.
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic> {
        let dir = plan.session_dir.clone();
        let remove = || {
            if let Some(dir) = &dir {
                let _ = std::fs::remove_dir_all(dir);
            }
        };
        let Some(pending) = plan
            .take_backend_state()
            .and_then(|state| state.downcast::<Pending>().ok())
        else {
            return Err(Diagnostic::builder(SANDBOX_012, Severity::Blocking)
                .why("this plan of the kernel level was already started")
                .build());
        };
        let Pending {
            door,
            proxy,
            door_lifeline,
            link,
        } = *pending;
        let door_thread = std::thread::Builder::new()
            .name("vm-door".to_owned())
            .spawn(move || session::serve_door(&door, &proxy, &door_lifeline));
        if let Err(err) = door_thread.and_then(|_| link.start()) {
            remove();
            return Err(door_failed(format!(
                "cannot start the threads of the door: {err}"
            )));
        }
        let handle = match launch_plan(plan) {
            Ok(handle) => handle,
            Err(diagnostic) => {
                remove();
                return Err(diagnostic);
            }
        };
        if let Some(dir) = dir {
            handle.remove_at_exit(dir);
        }
        Ok(handle)
    }

    /// Die `CHECK`-Zeilen aus dem Gast und die Prüfung des VMM auf dem
    /// Host; scheitert diese, ist das `SANDBOX_047`, und die Sitzung endet.
    ///
    /// Der Agent im Gast wartet auf das Urteil (HUM-444): Erst wenn beide
    /// Prüfungen bestehen, geht `ADMIT` in den Gast; sonst `DENY`, und die
    /// Sitzung endet, ohne dass der Agent je lief.
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult> {
        let mut results = isolation_from_report(handle, handle.report_timeout());
        let Some(stop) = handle.unit() else {
            return results;
        };
        let mut findings = host_check::run(stop, &self.resolved, DOOR_PORT);
        let admit = findings.is_empty() && results.iter().all(|result| result.passed);
        if stop.admission.decide(admit) && admit {
            for result in &mut results {
                result.evidence.push_str("; host: checked; admitted");
            }
            return results;
        }
        if admit {
            // Die Prüfungen bestanden, aber das Urteil erreichte den Gast
            // nicht: Der Agent startet nicht, und das ist ein Befund.
            findings.extend(
                IsolationCheck::ALL
                    .iter()
                    .map(|&check| host_check::Finding {
                        check,
                        what: "the admission could not reach the guest".to_owned(),
                    }),
            );
        }
        for result in &mut results {
            let own: Vec<&str> = findings
                .iter()
                .filter(|finding| finding.check == result.check)
                .map(|finding| finding.what.as_str())
                .collect();
            if own.is_empty() {
                continue;
            }
            let what = own.join("; ");
            result.passed = false;
            result.evidence = format!("{}; host: {what}", result.evidence);
            result.diagnostic = Some(
                Diagnostic::builder(SANDBOX_047, Severity::Blocking)
                    .why(format!(
                        "{}: the host check of the VM monitor failed: {what}; the agent was not \
                         admitted and the session was stopped",
                        result.check.as_str()
                    ))
                    .build(),
            );
        }
        let _ = handle.kill();
        results
    }
}

/// Der geprüfte Projektordner, wie der Plan ihn weiterreicht.
struct Project {
    /// Der geöffnete Ordner; sein Deskriptor geht an die Unit.
    work: crate::pinned::PinnedSource,
    /// Sein Inode, für die Mounttabelle des Gasts.
    ino: u64,
    /// Sein Pfad als Eigenschaft der Unit, wenn er beschreibbar ist.
    writable: Option<String>,
    /// Masken unter `/work`, die es im Projekt nicht gibt.
    unprotected: Vec<PathBuf>,
    /// Die nur lesbaren Binds der Sitzung, geöffnet und geprüft, als
    /// aufgelöster Pfad und Ziel; die Wurzel des Gasts bindet Pfade.
    session_ro: Vec<(PathBuf, PathBuf)>,
}

/// Die Kanäle einer Sitzung auf der Seite des Daemons.
struct Channels {
    root_socket: OwnedFd,
    door: UnixListener,
    handover: crate::fd_handover::HandoverListener,
    report_reader: std::io::PipeReader,
    report_writer: std::io::PipeWriter,
    lifeline: crate::shim_fds::Lifeline,
    lifeline_read: OwnedFd,
    door_lifeline: OwnedFd,
    link_lifeline: OwnedFd,
    daemon_end: OwnedFd,
    guest_end: OwnedFd,
}

impl Channels {
    /// Die Sockets des Daemons (die Wurzel, deren Deskriptor an virtiofsd
    /// geht, die eine Tür, die Übergabe), die Berichts-Pipe, die Lebensleine
    /// mit ihren Leseenden und das Strom-Paar des Steuerkanals.
    fn open(layout: &Layout, options: &crate::LaunchOptions) -> Result<Self, Diagnostic> {
        let bind = |path: PathBuf| {
            UnixListener::bind(&path)
                .map_err(|err| door_failed(format!("{}: {err}", path.display())))
        };
        let root_socket = OwnedFd::from(bind(layout.root_socket())?);
        let door = bind(layout.door_listener(DOOR_PORT))?;
        let handover = crate::fd_handover::HandoverListener::bind(&layout.handover())
            .map_err(|err| door_failed(format!("{}: {err}", layout.handover().display())))?;
        let (report_reader, report_writer) = std::io::pipe()
            .map_err(|err| setup_failed(format!("cannot create the report pipe: {err}")))?;
        let (lifeline, lifeline_read) = crate::shim_fds::Lifeline::new()
            .map_err(|err| setup_failed(format!("cannot create the lifeline: {err}")))?;
        let copy = |fd: &OwnedFd| {
            session::lifeline_copy(fd)
                .map_err(|err| setup_failed(format!("cannot copy the lifeline: {err}")))
        };
        let door_lifeline = copy(&lifeline_read)?;
        let link_lifeline = copy(&lifeline_read)?;
        let (cols, rows) = options.stdio.pty_size().unwrap_or((80, 24));
        let (daemon_end, guest_end) = crate::shell::stream_pair(cols, rows)
            .map_err(|err| door_failed(format!("cannot create the control stream: {err}")))?;
        Ok(Self {
            root_socket,
            door,
            handover,
            report_reader,
            report_writer,
            lifeline,
            lifeline_read,
            door_lifeline,
            link_lifeline,
            daemon_end,
            guest_end,
        })
    }
}

/// Schreibt die Dateien des Gasts, den Gastplan und den Namen der Unit.
fn write_files(
    layout: &Layout,
    root: &GuestRoot,
    plan_text: &str,
    unit: &UnitName,
) -> Result<(), Diagnostic> {
    let write = |path: &Path, content: &[u8], mode: u32| {
        std::fs::write(path, content)
            .and_then(|()| std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)))
            .map_err(|err| setup_failed(format!("{}: {err}", path.display())))
    };
    for (path, content) in &root.files {
        write(path, content, 0o400)?;
    }
    write(
        &layout.files().join("guest-plan.json"),
        plan_text.as_bytes(),
        0o400,
    )?;
    write(&layout.unit_file(), unit.service().as_bytes(), 0o600)
}

impl VmBackend {
    /// Der Plan im angelegten Laufzeitverzeichnis.
    fn plan_in(
        &self,
        layout: &Layout,
        request: &PlanRequest<'_>,
        units: &UserUnits,
        unit: &UnitName,
        project: Project,
    ) -> Result<LaunchPlan, Diagnostic> {
        let PlanRequest {
            profile,
            session,
            options,
        } = *request;
        let present = BwrapBackend::present_under_work(profile, &project.work.proc_path());
        let root = guest_root(
            profile,
            session,
            &present,
            &layout.files(),
            &self.resolved.modules,
            &project.session_ro,
        );
        let env = profile.effective_env(session, None);
        let probe = options.guest_root_probe.command();
        let plan_text = guest_plan(profile, session, &env, project.ino, probe)?;
        write_files(layout, &root, &plan_text, unit)?;
        let channels = Channels::open(layout, options)?;
        let group = session_group(session);
        let lines = session_lines(
            &self.resolved,
            layout,
            self.machine(),
            &root,
            &profile.network.shim_dst,
            project.writable.is_none(),
            &group,
        );
        let argv = unit_line(
            units,
            unit,
            session,
            layout,
            &lines,
            channels.handover.token(),
            project.writable.as_deref(),
        )?;
        let project_fd = project.work.into_fd();
        let fds = [&channels.root_socket, &project_fd, &channels.lifeline_read]
            .iter()
            .zip(lines::ROOT_SOCKET_FD..)
            .map(|(fd, target)| (fd.as_raw_fd(), target))
            .collect();
        let timeout = options
            .report_timeout
            .unwrap_or_else(|| self.start_timeout());
        let admission = crate::launcher::Admission::default();
        let link = session::GuestLink {
            device: layout.door_device(),
            report_port: REPORT_PORT,
            deadline: Instant::now() + timeout + Duration::from_secs(5),
            report: channels.report_writer,
            control: channels.guest_end,
            lifeline: channels.link_lifeline,
            admission: admission.clone(),
        };
        let mut warnings = profile.unmask_diagnostics();
        warnings.extend(crate::worktree::resolution_diagnostic());
        Ok(LaunchPlan {
            session_dir: Some(layout.dir.clone()),
            argv,
            argv_env: Vec::new(),
            env,
            fds,
            files: session.files.clone(),
            session: session.session,
            profile: profile.name.clone(),
            warnings,
            unprotected: project.unprotected,
            env_delivery: EnvDelivery::Argv,
            program_env: units.own_env().to_vec(),
            stdio: options.stdio,
            report_timeout: timeout,
            teardown: Teardown::Unit(UnitStop {
                units: units.clone(),
                unit: unit.clone(),
                runtime: layout.dir.clone(),
                admission,
            }),
            renumbered: false,
            program: units.run_program().to_path_buf(),
            once: Mutex::new(Some(LaunchOnce {
                inherit: vec![channels.root_socket, project_fd, channels.lifeline_read],
                report: channels.report_reader,
                status: None,
                output_sink: options.output_sink.clone(),
                shells: Some(crate::shell::DoorSeed::Stream(channels.daemon_end)),
                detached: Some(Detached {
                    lifeline: channels.lifeline,
                    handover: Some(channels.handover),
                    backend_state: Some(Box::new(Pending {
                        door: channels.door,
                        proxy: session.proxy_socket_src.clone(),
                        door_lifeline: channels.door_lifeline,
                        link,
                    })),
                }),
            })),
        })
    }
}

/// Nur Escape-Suite und Tests (HUM-445): Kernel und Module des Gasts aus
/// `HUMANITL_TEST_VM_KERNEL` und `HUMANITL_TEST_VM_MODULES`, wo sie gesetzt
/// sind, sonst die der Gruppe. Der CI-Job installiert einen Gastkernel neben
/// den des Runners, dessen Abbild nicht lesbar ist und virtiofs nur als
/// Modul hat; die Tests der Kernel-Stufe lesen dieselben zwei Variablen.
#[cfg(any(test, feature = "unadmitted-backends"))]
#[must_use]
pub fn with_test_guest_kernel(sandbox: &SandboxRef) -> SandboxRef {
    let from_env = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let mut out = sandbox.clone();
    if let Some(kernel) = from_env("HUMANITL_TEST_VM_KERNEL") {
        out.qemu.kernel = Some(kernel);
    }
    if let Some(modules) = from_env("HUMANITL_TEST_VM_MODULES") {
        out.qemu.modules = Some(modules);
    }
    out
}

/// Das Backend aus der Gruppe `sandbox` ([`VmBackend::detect`]).
///
/// # Errors
///
/// `SANDBOX_045` mit der ersten fehlenden Voraussetzung.
pub(crate) fn detect(paths: Paths, sandbox: &SandboxRef) -> Result<VmBackend, Diagnostic> {
    VmBackend::detect(paths, &sandbox.qemu)
}

/// Die erste fehlende Voraussetzung für die Gruppe `sandbox`.
pub(crate) fn missing(paths: &Paths, sandbox: &SandboxRef) -> Option<Diagnostic> {
    missing_prerequisite(paths, &sandbox.qemu)
}

/// Die Zeilen des Doctors für die Kernel-Stufe vor ihrer Zulassung (HUM-444):
/// eine je Voraussetzung, jede für sich geprüft.
pub(crate) fn static_lines(
    paths: &Paths,
    sandbox: &SandboxRef,
) -> Vec<(BackendCheckId, Result<String, Diagnostic>)> {
    prerequisites::each_prerequisite(paths.env(), &sandbox.qemu)
        .into_iter()
        .map(|(prerequisite, line)| {
            let id = match prerequisite {
                Prerequisite::Kvm => BackendCheckId::Kvm,
                Prerequisite::Emulator => BackendCheckId::Emulator,
                Prerequisite::FileDaemon => BackendCheckId::FileDaemon,
                Prerequisite::VsockDevice => BackendCheckId::VsockDevice,
                Prerequisite::Bubblewrap => BackendCheckId::Bubblewrap,
                Prerequisite::Kernel => BackendCheckId::KernelImage,
                Prerequisite::VirtioFs => BackendCheckId::VirtioFs,
                Prerequisite::Modules => BackendCheckId::KernelModules,
            };
            (id, line)
        })
        .collect()
}

/// Was frühere Sitzungen dieses Daemons liegen ließen: Laufzeitverzeichnisse
/// unter `humanitl-vm-<Instanz>` und ihre Units.
#[derive(Debug, Default)]
pub(crate) struct Leftovers {
    found: Vec<(PathBuf, Option<UnitName>)>,
    units: Option<UserUnits>,
}

/// Die Bestandsaufnahme, beim Start des Daemons, bevor er einen Start annimmt.
pub(crate) fn find_leftovers(paths: &Paths) -> Leftovers {
    let Some(instance) = instance_dir(paths) else {
        return Leftovers::default();
    };
    let Ok(entries) = std::fs::read_dir(&instance) else {
        return Leftovers::default();
    };
    let found: Vec<(PathBuf, Option<UnitName>)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| {
            let dir = entry.path();
            let unit = std::fs::read_to_string(Layout { dir: dir.clone() }.unit_file())
                .ok()
                .and_then(|text| UnitName::new(text.trim().trim_end_matches(".service")).ok());
            (dir, unit)
        })
        .collect();
    if found.is_empty() {
        return Leftovers::default();
    }
    Leftovers {
        found,
        units: user_units(paths).ok(),
    }
}

impl Leftovers {
    /// Stoppt die Unit jedes Rests und entfernt sein Verzeichnis; je Rest
    /// ein `SANDBOX_041` als Warnung.
    pub(crate) fn remove(self) -> Vec<Diagnostic> {
        self.found
            .into_iter()
            .map(|(dir, unit)| {
                if let (Some(units), Some(unit)) = (&self.units, &unit) {
                    let _ = crate::user_unit::run_bounded(
                        units.stop_command(unit, true),
                        Duration::from_secs(15),
                    );
                }
                let removed = std::fs::remove_dir_all(&dir);
                let unit_text = unit
                    .as_ref()
                    .map_or_else(|| "no unit".to_owned(), UnitName::service);
                Diagnostic::builder(SANDBOX_041, Severity::Warning)
                    .why(match removed {
                        Ok(()) => format!(
                            "a session of the kernel level from an earlier run was left behind \
                             ({}, {unit_text}); it was stopped and removed",
                            dir.display()
                        ),
                        Err(err) => format!(
                            "a session of the kernel level from an earlier run was left behind \
                             ({}, {unit_text}), and its directory could not be removed: {err}",
                            dir.display()
                        ),
                    })
                    .fix(FixAction::CopyCommand(format!(
                        "systemctl --user stop {unit_text}"
                    )))
                    .build()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
