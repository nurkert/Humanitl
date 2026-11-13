//! Das Podman-Backend, gemeinsame Stufe: rootless Podman mit der Laufzeit
//! `crun`, geteilter Kern (HUM-433, ADR-0020).
//!
//! Eine Sitzung ist genau ein `podman run`, den der Daemon startet und dessen
//! Deskriptoren der Container über `--preserve-fds` erbt (HUM-292 P4):
//! Bericht, Lebensleine, Umgebung als versiegelter memfd und am eigenen
//! Pseudoterminal die Tür zu weiteren Shells und das Terminal selbst. Der
//! Shim ist PID 1 des Containers, mit demselben Filter, denselben fünf
//! `CHECK`-Zeilen und derselben Tür wie unter bwrap; die Garantien liest
//! [`crate::bwrap::isolation_from_report`] aus seinem Bericht.
//!
//! Vier Messungen der Referenzmaschine (Podman 5.8.6, HUM-292 Abschnitt
//! 10a) legen den Aufbau fest:
//!
//! - **P13**: Ein Deskriptor ist keine Quelle eines Einhängens. Eingehängt
//!   wird über Pfade; der Plan öffnet und prüft jede Quelle trotzdem einmal,
//!   und dass danach noch dieselbe dort liegt, belegt der Shim über Gerät und
//!   Inode (HUM-437).
//! - **P5, P6**: PID 1 überhört Signale ohne Handler. Beendet wird über
//!   `podman kill` und `podman rm --force` ([`teardown`]), nie über die
//!   Haltezeit von `podman stop`; stirbt der Daemon, beendet die Lebensleine
//!   den Container (HUM-430).
//! - **P11**: Werte aus `-e` stehen auf der Platte. Die Umgebung des Agenten
//!   geht allein durch den memfd (`EnvDelivery::InheritedFd`), mit
//!   `--env-host=false` und `--http-proxy=false`. Die fünf Variablen des
//!   Shims (Bridges, seccomp-Listen, Nummer des Berichts) sind keine
//!   Geheimnisse und stehen in der `containers.conf` der Sitzung.
//! - **P4**: Die geerbten Deskriptoren erreichen PID 1 unter 3, 4, 5, ...;
//!   der Start vergibt sie deshalb fortlaufend (`LaunchPlan::renumbered`).
//!
//! **Keine Konfiguration, die der Daemon nicht schreibt.** Podman liest viel
//! mehr als seine Zeile. Der Adapter startet es mit leerer Umgebung außer
//! `HOME`, `XDG_RUNTIME_DIR` (das des Nutzers, nie das von Humanitl), `PATH`,
//! dem Sitzungsbus des Nutzers, wenn es einen gibt, und drei Dateien, die er selbst unter
//! `<Laufzeitverzeichnis>/podman/` schreibt ([`EngineDir`]):
//! `CONTAINERS_CONF` (mit einer leeren `default_mounts_file`, einem leeren
//! `hooks_dir` und der eigenen Kopie des seccomp-Profils),
//! `CONTAINERS_STORAGE_CONF` und `CONTAINERS_REGISTRIES_CONF`.
//! `CONTAINERS_CONF_OVERRIDE`, `CONTAINER_HOST` und `CONTAINER_CONNECTION`
//! erreichen Podman damit nie. Die `mounts.conf` unter `HOME` liest Podman
//! trotz `default_mounts_file` zusätzlich; erst das globale Flag
//! `--default-mounts-file` auf der Startzeile ersetzt die Liste (HUM-454).
//!
//! **Die eigene Kopie des seccomp-Profils** (`seccomp.json`) ist das
//! Standardprofil aus `containers-common` 0.67.1 (Debian-Paket
//! `golang-github-containers-common` 0.67.1-3, Apache-2.0), unverändert. Es
//! liegt außen um den Filter des Shims, der die dritte Garantie trägt; dass
//! es `seccomp(2)` erlaubt, zeigt P3.
//!
//! **Keine Wegwerf-Sandbox (HUM-438).** Das Overlay-Volume von Podman mit
//! eigener oberer Schicht (`-v <projekt>:/work:O,upperdir=,workdir=`) nimmt
//! seine untere Schicht als Pfad, den die Engine später selbst auflöst; ein
//! Deskriptor ist dort keine Quelle (P13, und P10 mit
//! `/proc/self/fd/N` als untere Schicht: `crun: ... Invalid argument`).
//! Anders als ein Bind lässt sich das Overlay im Container nicht über Gerät
//! und Inode mit dem geprüften Ordner verbinden: Es trägt sein eigenes
//! Gerät, und der `lowerdir` in `mountinfo` ist nur Text. Eine
//! Wegwerf-Sandbox, deren untere Schicht ein getauschter Ordner sein kann,
//! hielte die Zusage von HUM-263 nicht. `disposable` lehnt deshalb mit
//! `SANDBOX_030` ab und nennt den Grund, `auto` läuft direkt mit
//! `SANDBOX_033` (gemessen 2026-11-12, Podman 5.8.6, Kernel 7.3).
//!
//! **Ein Abbild als Wurzel (HUM-439).** Mit `rootfs = "image"` ist die
//! Wurzel das Abbild aus `sandbox.podman.image`, immer mit Digest. Vor jedem
//! Start prüft der Plan mit `podman image inspect`, dass genau dieser Digest
//! im lokalen Speicher liegt (`SANDBOX_043`), und kein Start zieht
//! ([`image`]). Die Binds aus `mounts.ro` und die Verweise des Profils gelten
//! dann nicht, denn das Abbild bringt sein eigenes `/usr` und `/etc`;
//! Projekt, Tür, CA, Shim, Dateien des Adapters und das Verzeichnis der
//! Sitzung kommen wie ohne Abbild vom Host. Die Tabelle nennt die Wurzel
//! `PolicyMode::Image` mit dem Verweis samt Digest. Gezogen wird nur auf
//! Wunsch, über [`SandboxBackend::pull_image`].
//!
//! **Zulassung.** Bis HUM-448 steht Podman nicht in
//! [`crate::ADMITTED_BACKENDS`]: [`crate::Backend::detect`] lehnt es mit
//! `SANDBOX_048` ab, und nur die Escape-Suite und die Tests kommen über
//! `Backend::detect_unadmitted` an diesen Adapter. Seine Konstruktoren sind
//! deshalb nur in der Crate sichtbar.

mod detect;
mod image;
mod mounts;
mod plan;
mod teardown;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::fd::{AsRawFd as _, OwnedFd, RawFd};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use humanitl_config::{Paths, PodmanConfig, PodmanRootfs, PodmanRuntime};
use humanitl_core::diagnostics::codes::{
    CONFIG_003, SANDBOX_011, SANDBOX_030, SANDBOX_037, SANDBOX_048,
};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use sha2::{Digest as _, Sha256};

use crate::backend::PODMAN_BACKEND;
use crate::bwrap::{BwrapBackend, PinnedMounts, Version, isolation_from_report, launch_plan};
use crate::doctor::BackendCheckId;
use crate::handle::SandboxHandle;
use crate::launcher::{
    CheckResult, ContainerStop, Door, EnvDelivery, IsolationLevel, LaunchOnce, LaunchPlan,
    PlanPreview, PlanRequest, PolicyMode, PolicyOrigin, PolicyRow, PullCancel, PulledImage,
    ResourceLimits, SandboxBackend, Teardown,
};
use crate::profile::{MountPolicy, SandboxProfile, SessionContext};
use crate::shim_fds::{Lifeline, env_memfd, split_env};

pub use detect::MIN_PODMAN_VERSION;

use plan::{Entry, Fds, LineSpec, Root, SessionFiles, Sources};

/// Der Name des Programms, wie ihn die Suche im `PATH` findet.
pub(crate) const PROGRAM: &str = "podman";

/// Der Schlüssel des Labels, an dem der Daemon seine Container erkennt.
pub(crate) const LABEL_KEY: &str = "humanitl.daemon";

/// Das Verzeichnis des Adapters unter dem Laufzeitverzeichnis von Humanitl.
const ENGINE_DIR: &str = "podman";

/// Die eigene Kopie des seccomp-Profils von Podman (siehe
/// Modulbeschreibung).
const SECCOMP_PROFILE: &str = include_str!("seccomp.json");

/// Die `registries.conf` des Adapters: keine Suchliste für kurze Namen, und
/// Zugangsdaten nur aus der Datei von `--authfile` (`containers-auth.json`),
/// nie aus einem Schlüsselbund oder einem `docker-credential-*`-Helfer
/// (HUM-456, [`image`]).
const REGISTRIES_CONF: &str =
    "unqualified-search-registries = []\ncredential-helpers = [\"containers-auth.json\"]\n";

/// Arbeitsspeicher, wenn der Host ihn nicht nennt, in MiB.
const FALLBACK_MEMORY_MIB: u64 = 2048;

/// Höchstens so viel Arbeitsspeicher leitet der Start vom Host ab, in MiB.
const MAX_DERIVED_MEMORY_MIB: u64 = 4096;

/// Höchstens so viele CPUs leitet der Start vom Host ab.
const MAX_DERIVED_CPUS: u32 = 4;

/// Was der Adapter aus `sandbox.podman` liest (HUM-431), mit den Werten,
/// die der Host einsetzt, wo der Schlüssel leer ist.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Settings {
    runtime: PodmanRuntime,
    rootfs: PodmanRootfs,
    /// Das Abbild aus `sandbox.podman.image`, mit Digest; gilt nur mit
    /// `rootfs = "image"` als Wurzel, ist aber auch sonst das, was
    /// [`SandboxBackend::pull_image`] ohne eigenen Verweis zieht.
    image: Option<String>,
    memory_mib: u64,
    cpus: u32,
    pids: u64,
    start_timeout: Duration,
}

impl Settings {
    fn from_config(config: &PodmanConfig) -> Self {
        Self::with_host(config, host_memory_mib(), host_cpus())
    }

    /// `memory_mib` leer: die Hälfte des Arbeitsspeichers des Hosts,
    /// höchstens 4096 MiB; `cpus` leer: die Hälfte der CPUs, die der Daemon
    /// nutzen darf, mindestens 1, höchstens 4.
    fn with_host(config: &PodmanConfig, memory: Option<u64>, cpus: Option<u32>) -> Self {
        Self {
            runtime: config.runtime,
            rootfs: config.rootfs,
            image: config.image.clone(),
            memory_mib: config.memory_mib.unwrap_or_else(|| {
                memory.map_or(FALLBACK_MEMORY_MIB, |total| {
                    (total / 2).clamp(256, MAX_DERIVED_MEMORY_MIB)
                })
            }),
            cpus: config
                .cpus
                .unwrap_or_else(|| cpus.map_or(1, |count| (count / 2).clamp(1, MAX_DERIVED_CPUS))),
            pids: config.pids_limit,
            start_timeout: Duration::from_secs(config.start_timeout_secs),
        }
    }

    fn limits(&self) -> ResourceLimits {
        ResourceLimits {
            memory_bytes: self.memory_mib.checked_mul(1024 * 1024),
            cpu_millis: self.cpus.checked_mul(1000),
            pids: u32::try_from(self.pids).ok(),
        }
    }
}

/// `MemTotal` des Hosts in MiB.
fn host_memory_mib() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))?;
    let kib: u64 = line.trim().strip_suffix("kB")?.trim().parse().ok()?;
    Some(kib / 1024)
}

/// Die CPUs, die dieser Prozess nutzen darf.
fn host_cpus() -> Option<u32> {
    std::thread::available_parallelism()
        .ok()
        .and_then(|count| u32::try_from(count.get()).ok())
}

/// Podman als Sandbox-Backend der gemeinsamen Stufe.
///
/// Konstruiert wird es nur in dieser Crate, über [`crate::Backend`], das die
/// Zulassung prüft (Modulbeschreibung).
#[derive(Debug, Clone)]
pub struct PodmanBackend {
    program: PathBuf,
    version: Version,
    settings: Settings,
    paths: Paths,
}

impl PodmanBackend {
    /// Sucht Podman und prüft jede Voraussetzung ([`detect`]).
    ///
    /// # Errors
    ///
    /// `SANDBOX_048` für die Laufzeit `krun` (bis HUM-444), sonst die Codes
    /// aus [`detect`]: `SANDBOX_034`, `SANDBOX_037` bis `SANDBOX_040`,
    /// `SANDBOX_044`; `SANDBOX_011`, wenn das Verzeichnis des Adapters sich
    /// nicht anlegen lässt.
    pub(crate) fn detect(paths: Paths, config: &PodmanConfig) -> Result<Self, Diagnostic> {
        refuse_kernel_level(config.runtime)?;
        let program = detect::find_program(config, paths.env())?;
        let engine = EngineDir::of(&paths);
        engine.prepare()?;
        let conf = engine.containers_conf();
        let mut version = engine.command(&program, &conf);
        version.args(["version", "--format", "{{.Client.Version}}"]);
        let version = detect::check_version(
            &program,
            &teardown::run_bounded(version, detect::QUERY_TIMEOUT),
        )?;
        let mut info = engine.command(&program, &conf);
        info.args(["info", "--format", detect::INFO_FORMAT]);
        detect::check_info(
            &program,
            &teardown::run_bounded(info, detect::QUERY_TIMEOUT),
        )?;
        let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
        detect::check_subordinate_ids(
            &detect::Account::of_this_process(),
            &read("/etc/subuid"),
            &read("/etc/subgid"),
            &detect::missing_id_tools(paths.env()),
            crate::os_release::PackageManager::detect(),
        )?;
        detect::check_selinux(detect::selinux_enforce().as_deref())?;
        Ok(Self {
            program,
            version,
            settings: Settings::from_config(config),
            paths,
        })
    }

    /// Ohne jede Prüfung, mit Podman aus der Einstellung oder dem `PATH`: der
    /// Rückfall für eine Vorschau, wenn [`Self::detect`] scheitert.
    pub(crate) fn fallback(paths: Paths, config: &PodmanConfig) -> Self {
        let program = config
            .program
            .clone()
            .unwrap_or_else(|| PathBuf::from(PROGRAM));
        Self {
            program,
            version: MIN_PODMAN_VERSION,
            settings: Settings::from_config(config),
            paths,
        }
    }

    /// Das Programm, das startet.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Die Fassung als Text, für den Zwischenspeicher des Probestarts
    /// (HUM-436).
    #[must_use]
    pub fn tool_version(&self) -> String {
        self.version.to_string()
    }

    /// Die gefundene Fassung.
    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    /// Die Pfade, gegen die geplant wird.
    #[must_use]
    pub const fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Die Zeile für eine Anfrage, mit den Pfaden der Sitzung als Quellen;
    /// gemeinsam für Vorschau und Plan.
    fn spec<'a>(
        &'a self,
        engine: &'a EngineDir,
        session_dir: &'a SessionDir,
        names: &'a Names,
        fds: Fds,
        request: &PlanRequest<'_>,
    ) -> LineSpec<'a> {
        let root = self.root(session_dir);
        LineSpec {
            program: &self.program,
            runtime: detect::CRUN,
            hooks_dir: &engine.hooks_dir,
            mounts_file: &engine.mounts_conf,
            seccomp: &engine.seccomp,
            conmon_pidfile: &session_dir.conmon_pidfile,
            root,
            name: &names.container,
            label: &names.label,
            session_label: &names.session_label,
            memory_mib: self.settings.memory_mib,
            cpus: self.settings.cpus,
            pids: self.settings.pids,
            fds,
            stdio: request.options.stdio,
        }
    }

    /// Lehnt ab, was diese Stufe nicht kann: die Laufzeit `krun`
    /// (HUM-444), ein Abbild als Wurzel ohne Verweis und die Wegwerf-Sandbox
    /// (HUM-438, [`disposable_unsupported`]). Nie still etwas anderes.
    fn refuse_unsupported(&self, request: &PlanRequest<'_>) -> Result<(), Diagnostic> {
        refuse_kernel_level(self.settings.runtime)?;
        if self.settings.rootfs == PodmanRootfs::Image && self.settings.image.is_none() {
            // Das Laden lehnt das schon ab (`CONFIG_003`); ein Start nimmt
            // auch dann nicht still die Wurzel des Hosts.
            return Err(no_image_named());
        }
        if request.options.disposable.is_some() {
            return Err(disposable_unsupported());
        }
        Ok(())
    }

    /// Die Wurzel einer Sitzung: das Abbild, mit der Kennung dieses
    /// Prozesses als Nutzer, oder das leere Verzeichnis der Sitzung.
    fn root<'a>(&'a self, session_dir: &'a SessionDir) -> Root<'a> {
        match self.image_root() {
            Some(reference) => Root::Image {
                reference,
                user: (
                    rustix::process::getuid().as_raw(),
                    rustix::process::getgid().as_raw(),
                ),
            },
            None => Root::Dir(&session_dir.rootfs),
        }
    }

    /// Was die Wurzel vor dem Start braucht. Unter einem Abbild die Prüfung
    /// seines Digests ([`Self::check_image`], `SANDBOX_043`, nie ein Ziehen)
    /// und kein Gerüst: Die Mountpunkte legt die Engine in der eigenen
    /// Schicht des Abbilds an, nicht auf dem Host. Sonst das Gerüst der leeren
    /// Wurzel ([`plan::skeleton`]).
    fn root_skeleton(
        &self,
        engine: &EngineDir,
        profile: &SandboxProfile,
        mounts: &[plan::Mount],
    ) -> Result<BTreeSet<Entry>, Diagnostic> {
        match self.image_root() {
            Some(reference) => {
                self.check_image(engine, reference)?;
                Ok(BTreeSet::new())
            }
            None => Ok(plan::skeleton(profile, mounts)),
        }
    }

    /// Das Abbild, wenn es die Wurzel ist (`rootfs = "image"`).
    fn image_root(&self) -> Option<&str> {
        (self.settings.rootfs == PodmanRootfs::Image)
            .then_some(self.settings.image.as_deref())
            .flatten()
    }

    /// Das Profil, nach dem geplant wird: unter einem Abbild ohne
    /// `mounts.ro` und ohne Verweise, denn `/usr`, `/etc` und die Verweise
    /// darauf bringt das Abbild mit; alles andere bleibt (HUM-439).
    fn root_profile<'p>(&self, profile: &'p SandboxProfile) -> Cow<'p, SandboxProfile> {
        if self.image_root().is_none() {
            return Cow::Borrowed(profile);
        }
        let mut own = profile.clone();
        own.mounts.ro.clear();
        own.mounts.symlinks.clear();
        Cow::Owned(own)
    }

    /// Die Zeile der Tabelle für die Wurzel, wenn sie ein Abbild ist: der
    /// Verweis samt Digest, damit die Vorschau zeigt, was startet.
    fn image_row(&self) -> Option<PolicyRow> {
        self.image_root().map(|reference| {
            PolicyRow::new(
                PolicyMode::Image,
                Some(PathBuf::from(reference)),
                PathBuf::from("/"),
                PolicyOrigin::User,
            )
        })
    }

    /// Fragt Podman, ob genau dieses Abbild im lokalen Speicher liegt
    /// ([`image::verdict`]); zieht nie.
    ///
    /// # Errors
    ///
    /// `SANDBOX_043`, wenn es fehlt, einen anderen Digest trägt oder nicht
    /// als Wurzel taugt.
    fn check_image(&self, engine: &EngineDir, reference: &str) -> Result<image::Found, Diagnostic> {
        let conf = engine.containers_conf();
        let mut inspect = engine.command(&self.program, &conf);
        inspect.args(["image", "inspect", "--", reference]);
        let answer = teardown::run_bounded(inspect, detect::QUERY_TIMEOUT);
        if !matches!(&answer, Ok(output) if output.status.success()) {
            // Was liegt unter dem Namen? Nur für den Satz des Befunds.
            let name = image::split(reference).map_or(reference, |(name, _)| name);
            let mut by_name = engine.command(&self.program, &conf);
            by_name.args(["image", "inspect", "--", name]);
            let present =
                image::present_digest(&teardown::run_bounded(by_name, detect::QUERY_TIMEOUT));
            let detail = match &answer {
                Ok(output) => String::from_utf8_lossy(&output.stderr).into_owned(),
                Err(err) => err.to_string(),
            };
            return Err(image::missing(reference, present.as_deref(), &detail));
        }
        image::verdict(reference, &answer)
    }

    /// Zieht ein Abbild, auf ausdrücklichen Wunsch (HUM-439): `requested`,
    /// oder ohne Angabe das aus `sandbox.podman.image`. Podman holt es direkt
    /// über das Netz, nicht durch den Proxy ([`image`]); danach prüft es
    /// dieselbe Frage wie jeder Start.
    ///
    /// # Errors
    ///
    /// `CONFIG_003` für einen Verweis ohne Digest oder ohne Registry, oder
    /// ohne Verweis und ohne eingestelltes Abbild; `SANDBOX_043`, wenn das
    /// Ziehen scheitert oder danach nicht genau dieser Digest vorliegt, wenn
    /// die Datei der Zugangsdaten nicht taugt oder eine Ergänzung zur
    /// `registries.conf` einen Credential-Helfer nennt (HUM-456);
    /// `SANDBOX_011` und `SANDBOX_037` aus dem Verzeichnis des Adapters;
    /// `SANDBOX_043` auch nach einem Abbruch über `cancel` (HUM-457).
    fn pull(&self, requested: &str, cancel: &PullCancel) -> Result<PulledImage, Diagnostic> {
        let reference = if requested.trim().is_empty() {
            self.settings.image.clone().ok_or_else(no_image_named)?
        } else {
            requested.trim().to_owned()
        };
        let engine = EngineDir::of(&self.paths);
        // Kopien früherer Prozesse des Daemons (Absturz mitten im Ziehen)
        // gehen zuerst, vor jeder Prüfung, die abbrechen könnte.
        engine.remove_stale_pull_auth();
        image::check_pull_reference(&reference)?;
        engine.prepare()?;
        image::check_registry_dropins(&image::registry_dropin_dirs(
            Path::new(image::SYSTEM_REGISTRIES_DROPINS),
            &engine.home,
        ))?;
        let copy = engine.pull_auth_copy();
        let credentials = image::credentials_path(&self.paths);
        let auth_file = image::auth_file(
            image::AuthFiles {
                user: &credentials,
                anonymous: &engine.anonymous_auth,
                copy: &copy,
            },
            rustix::process::getuid().as_raw(),
        )?;
        let mut pull = engine.command(&self.program, &engine.containers_conf());
        pull.args(image::pull_args(
            &reference,
            &engine.signature_policy,
            &auth_file,
        ));
        // Abbrechbar, und in einer eigenen Prozessgruppe (HUM-457): Ein
        // Abbruch beendet die Engine samt den Prozessen, die sie für das
        // Ziehen startete, und kommt erst zurück, wenn keiner mehr lebt.
        let answer = teardown::run_cancellable(pull, image::PULL_TIMEOUT, cancel);
        // Die Kopie der Zugangsdaten lebt nur so lange wie das Ziehen, auch
        // wenn es scheiterte; zwischen ihrem Schreiben und hier gibt es
        // keinen anderen Ausgang. Jedes Ziehen hat seine eigene Kopie, ein
        // gleichzeitiges räumt also nie die eines anderen weg. Bricht der
        // Daemon vorher ab, entfernt das nächste Ziehen eines neuen Prozesses
        // sie vor allem anderen.
        let _ = std::fs::remove_file(&copy);
        // Nur ein Ziehen, das der Abbruch wirklich beendet hat, zählt als
        // abgebrochen, und nur dann trägt das Zeichen das ein; kam er erst
        // nach dem Ende, gilt dessen Ergebnis. Die Frist beendet die Gruppe
        // ebenso, und was danach noch lief, steht im Befund.
        let answer = match answer {
            Ok(teardown::Ended::Exited(output)) => Ok(output),
            Ok(teardown::Ended::Cancelled { alive }) => {
                cancel.mark_interrupted(alive);
                return Err(image::pull_cancelled(&reference, alive));
            }
            Ok(teardown::Ended::DeadlineKilled { alive }) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                image::deadline_detail(alive),
            )),
            Err(err) => Err(err),
        };
        if !matches!(&answer, Ok(output) if output.status.success()) {
            return Err(image::pull_failed(&reference, &answer, &credentials));
        }
        let found = self.check_image(&engine, &reference)?;
        Ok(PulledImage {
            reference,
            digest: found.digest,
            id: found.id,
        })
    }

    /// Die PID von PID 1 des Containers auf dem Host, sobald der Shim
    /// berichtet hat: `podman inspect` nennt sie, und sie zählt nur, wenn ihr
    /// Elternprozess der `conmon` aus `--conmon-pidfile` ist.
    fn sandbox_pid(
        &self,
        stop: &ContainerStop,
        conmon_pidfile: &Path,
        handle: &SandboxHandle,
        timeout: Duration,
    ) -> Option<u32> {
        let report = handle.wait_for_report(timeout);
        if report.checks.is_empty() || handle.try_wait().is_some() {
            return None;
        }
        let conmon: u32 = std::fs::read_to_string(conmon_pidfile)
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let mut inspect = Command::new(&self.program);
        inspect
            .env_clear()
            .envs(stop.env.iter().map(|(key, value)| (key, value)))
            .args(["inspect", "--format", "{{.State.Pid}}", &stop.name]);
        let output = teardown::run_bounded(inspect, detect::QUERY_TIMEOUT).ok()?;
        let pid: u32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        let parent = status
            .lines()
            .find_map(|line| line.strip_prefix("PPid:"))?
            .trim()
            .parse::<u32>()
            .ok()?;
        (pid != 0 && parent == conmon).then_some(pid)
    }
}

/// Die Sitzung schreiben: Dateien, Gerüst der Wurzel, `containers.conf`.
fn write_session(
    dir: &SessionDir,
    profile: &SandboxProfile,
    session: &SessionContext,
    files: &SessionFiles,
    skeleton: &BTreeSet<Entry>,
    shim_env: &[(String, String)],
    engine: &EngineDir,
) -> Result<(), Diagnostic> {
    let identity = profile.identity_files(
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    write_file(&files.passwd, identity.passwd.as_bytes(), 0o444)?;
    write_file(&files.group, identity.group.as_bytes(), 0o444)?;
    write_file(&files.hosts, identity.hosts.as_bytes(), 0o444)?;
    write_file(&files.mask, b"", 0o444)?;
    let secret = session.files.iter().any(|file| file.secret);
    if secret && crate::disposable::require_tmpfs(&dir.root).is_err() {
        return Err(Diagnostic::builder(SANDBOX_011, Severity::Blocking)
            .why(format!(
                "{} is not on tmpfs; the agent's files carry the model's access key, and a \
                 container engine takes no descriptor as a mount source, so they would be \
                 written to disk",
                dir.root.display()
            ))
            .fix(FixAction::SetEnv {
                key: "XDG_RUNTIME_DIR".to_owned(),
                value: format!("/run/user/{}", rustix::process::getuid().as_raw()),
            })
            .build());
    }
    for (path, file) in files.agent.iter().zip(&session.files) {
        // Nur lesbar, nur für den Nutzer: In der Sandbox liest der Agent
        // mit derselben Kennung (`--userns keep-id`).
        write_file(path, &file.content, 0o400)?;
    }
    build_skeleton(&dir.rootfs, skeleton)?;
    write_file(
        &dir.containers_conf,
        engine.conf_text(shim_env).as_bytes(),
        0o600,
    )?;
    Ok(())
}

/// Die Warnungen eines Plans wie unter bwrap: aufgehobene Masken und ein
/// Arbeitsbaum, der sich nicht auflösen ließ.
fn plan_warnings(profile: &SandboxProfile) -> Vec<Diagnostic> {
    let mut warnings = profile.unmask_diagnostics();
    warnings.extend(crate::worktree::resolution_diagnostic());
    warnings
}

/// Die Quellen aus den geöffneten und geprüften Orten (HUM-263): eingehängt
/// wird der Ort, an dem das Geöffnete liegt (P13).
fn sources_of(pinned: &PinnedMounts, files: SessionFiles) -> Sources {
    let resolved = |list: &[crate::pinned::PinnedSource]| {
        list.iter()
            .map(|pin| pin.resolved().to_path_buf())
            .collect()
    };
    Sources {
        work: pinned.work.resolved().to_path_buf(),
        extra_ro: resolved(&pinned.read_only),
        extra_rw: resolved(&pinned.writable),
        session_ro: pinned
            .session
            .iter()
            .map(|(pin, dst)| (pin.resolved().to_path_buf(), dst.clone()))
            .collect(),
        agent_program: pinned
            .agent
            .as_ref()
            .map(|pin| pin.resolved().to_path_buf()),
        files,
    }
}

/// Jeder Pfad der Einhängungen und der Verzeichnisse des Adapters lässt sich
/// in `--mount` schreiben ([`plan::check_expressible`]).
fn check_mounts_expressible(dirs: &[&Path], mounts: &[plan::Mount]) -> Result<(), Diagnostic> {
    let mut expressed: Vec<&Path> = dirs.to_vec();
    for mount in mounts {
        expressed.push(&mount.dst);
        if let plan::MountKind::Bind { source, .. } = &mount.kind {
            expressed.push(source);
        }
    }
    plan::check_expressible(&expressed)
}

/// Eine Lebensleine (HUM-430); ein Fehler ist `SANDBOX_011`.
fn new_lifeline() -> Result<(Lifeline, OwnedFd), Diagnostic> {
    Lifeline::new().map_err(|err| {
        Diagnostic::builder(SANDBOX_011, Severity::Blocking)
            .why(format!("cannot create the lifeline: {err}"))
            .build()
    })
}

/// `SANDBOX_048` für die Kernel-Stufe: `krun` kommt mit HUM-444.
fn refuse_kernel_level(runtime: PodmanRuntime) -> Result<(), Diagnostic> {
    if runtime == PodmanRuntime::Crun {
        return Ok(());
    }
    Err(Diagnostic::builder(SANDBOX_048, Severity::Blocking)
        .why(format!(
            "sandbox.podman.runtime = {:?} is not admitted yet: the kernel level comes with its \
             own adapter and escape suite (HUM-444, HUM-449)",
            runtime.as_str()
        ))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.podman.runtime".to_owned(),
            value: detect::CRUN.to_owned(),
        })
        .build())
}

/// Dieselbe Gruppe `sandbox`, aber mit dem Abbild `reference` als Wurzel der
/// Container-Engine (HUM-439). Für die Escape-Suite (`escape-launch --image`),
/// die dieselben Fälle auch unter einem Abbild fährt; der Daemon und die
/// Kommandozeile lesen das Abbild aus der Konfiguration.
#[must_use]
pub fn with_image_root(
    sandbox: &humanitl_config::SandboxRef,
    reference: &str,
) -> humanitl_config::SandboxRef {
    let mut out = sandbox.clone();
    out.podman.rootfs = PodmanRootfs::Image;
    out.podman.image = Some(reference.to_owned());
    out
}

/// `CONFIG_003`: Ein Abbild ist verlangt, aber keines eingestellt.
fn no_image_named() -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(
            "no image is named: sandbox.podman.image is empty, and an image is only ever taken \
             as NAME@sha256: with its digest; the start does not take the host root instead on \
             its own",
        )
        .fix(FixAction::ChangeSetting {
            key: "sandbox.podman.rootfs".to_owned(),
            value: "host".to_owned(),
        })
        .build()
}

/// Die Voraussetzungen aus [`PodmanBackend::detect`], die sich prüfen lassen,
/// ohne Podman zu starten, als Zeilen des Doctors (HUM-436): das Programm
/// (`SANDBOX_034` mit dem Installationsbefehl samt `uidmap`), die
/// untergeordneten IDs samt `newuidmap` und `newgidmap` (`SANDBOX_039`) und
/// `SELinux` (`SANDBOX_044`). Fassung, rootless, Laufzeit und Controller sagt
/// nur Podman selbst; die fragt erst [`PodmanBackend::detect`], also erst nach
/// der Zulassung. Für ein anderes Backend als dieses: keine Zeile.
///
/// Je Zeile die Kennung und bei Erfolg der Beleg, sonst der Befund.
pub(crate) fn static_lines(
    name: &str,
    paths: &Paths,
    sandbox: &humanitl_config::SandboxRef,
) -> Vec<(BackendCheckId, Result<String, Diagnostic>)> {
    if name != PODMAN_BACKEND {
        return Vec::new();
    }
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    vec![
        (
            BackendCheckId::Program,
            detect::find_program(&sandbox.podman, paths.env())
                .map(|program| program.display().to_string()),
        ),
        (
            BackendCheckId::SubordinateIds,
            detect::check_subordinate_ids(
                &detect::Account::of_this_process(),
                &read("/etc/subuid"),
                &read("/etc/subgid"),
                &detect::missing_id_tools(paths.env()),
                crate::os_release::PackageManager::detect(),
            )
            .map(|()| "65536 subordinate IDs, newuidmap and newgidmap".to_owned()),
        ),
        (
            BackendCheckId::Selinux,
            detect::check_selinux(detect::selinux_enforce().as_deref())
                .map(|()| "SELinux does not enforce labels".to_owned()),
        ),
    ]
}

/// `SANDBOX_030`: Unter Podman gibt es keine Wegwerf-Sandbox, weil sich ihre
/// untere Schicht nicht an den geprüften Ordner binden lässt (HUM-438,
/// Modulbeschreibung).
fn disposable_unsupported() -> Diagnostic {
    Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        // Gemessen in HUM-292 P10 und P13 (2026-11-12); siehe Modulbeschreibung.
        .why(
            "this backend has no disposable sandbox: the container engine resolves the project \
             path itself, so its lower layer cannot be tied to the checked folder. Use the \
             bubblewrap backend for a disposable sandbox, or set sandbox.mode = \"direct\"",
        )
        .fix(FixAction::ChangeSetting {
            key: "sandbox.backend".to_owned(),
            value: crate::backend::BWRAP_BACKEND.to_owned(),
        })
        .build()
}

/// Name und Label des Containers einer Sitzung.
struct Names {
    container: String,
    label: String,
    session_label: String,
}

impl Names {
    fn of(session: SessionId, engine: &EngineDir) -> Self {
        Self {
            container: container_name(session),
            label: format!("{LABEL_KEY}={}", engine.instance),
            session_label: format!("{}={session}", teardown::SESSION_LABEL_KEY),
        }
    }
}

/// Der Name des Containers einer Sitzung.
pub(crate) fn container_name(session: SessionId) -> String {
    format!("humanitl-{session}")
}

/// Die Instanz eines Daemons: die ersten 16 Hex-Ziffern von SHA-256 über
/// sein Laufzeitverzeichnis. Zwei Daemons mit eigenen Verzeichnissen (ein
/// Test neben dem echten) räumen so nie die Container des anderen weg.
pub(crate) fn instance_of(runtime_root: &Path) -> String {
    let digest = Sha256::digest(runtime_root.as_os_str().as_encoded_bytes());
    digest.iter().take(8).fold(String::new(), |mut hex, byte| {
        let _ = std::fmt::Write::write_fmt(&mut hex, format_args!("{byte:02x}"));
        hex
    })
}

/// Das Laufzeitverzeichnis, das Podman bekommt: das des Nutzers, nie das von
/// Humanitl.
///
/// Rootless Podman legt dort `libpod/tmp` und die `runroot` seines Speichers
/// ab und merkt sich beide in seiner Datenbank; ein anderes Verzeichnis als
/// beim letzten Lauf des Nutzers lässt `podman info` mit 125 scheitern. Das
/// Laufzeitverzeichnis von Humanitl kann woanders liegen (ein Test, ein
/// Ersatz unter `$TMPDIR`); seine eigenen Dateien bleiben dort.
///
/// Vorrang hat `/run/user/<uid>`, wenn es ein Verzeichnis ist, das dieser
/// Kennung gehört, sonst `XDG_RUNTIME_DIR` des Prozesses unter derselben
/// Bedingung. `owned_dir` sagt für einen Pfad, ob das gilt. Ohne ein solches
/// Verzeichnis gibt es keines: Ein fremdes oder fehlendes Verzeichnis gäbe
/// Podman eine falsche oder keine Datenbank, und [`EngineDir::prepare`]
/// lehnt dann mit [`no_runtime_dir`] ab.
pub(crate) fn user_runtime_dir(
    uid: u32,
    process_xdg: Option<&str>,
    owned_dir: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let run_user = PathBuf::from(format!("/run/user/{uid}"));
    if owned_dir(&run_user) {
        return Some(run_user);
    }
    process_xdg
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && owned_dir(dir))
}

/// Ob `path` ein Verzeichnis ist, das der Kennung `uid` gehört (ohne einem
/// Verweis zu folgen).
fn owned_dir_of(uid: u32, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && meta.uid() == uid)
}

/// `SANDBOX_037`: Es gibt kein Laufzeitverzeichnis des Nutzers, das rootless
/// Podman braucht (keine Anmeldesitzung, kein `XDG_RUNTIME_DIR`).
pub(crate) fn no_runtime_dir(uid: u32) -> Diagnostic {
    Diagnostic::builder(SANDBOX_037, Severity::Blocking)
        .why(format!(
            "rootless Podman needs the user's runtime directory, and neither /run/user/{uid} nor \
             XDG_RUNTIME_DIR names a directory owned by this user (uid {uid}); without a login \
             session of this user Podman cannot keep its state"
        ))
        .fix(FixAction::CopyCommand(
            "sudo loginctl enable-linger \"$(id -un)\"".to_owned(),
        ))
        .build()
}

/// Der Anfang des Namens jeder Kopie von Zugangsdaten im Verzeichnis des
/// Adapters (HUM-456).
const PULL_AUTH_PREFIX: &str = "pull-auth-";

/// `pull-auth-<Instanz>-`: der Anfang der Kopien dieses Prozesses, mit einer
/// Kennung, die einmal je Prozess entsteht.
fn own_pull_auth_prefix() -> &'static str {
    static OWN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    OWN.get_or_init(|| format!("{PULL_AUTH_PREFIX}{}-", uuid::Uuid::now_v7().simple()))
}

/// Die Dateien, die der Adapter Podman statt der Konfiguration des Nutzers
/// gibt, und die Umgebung, mit der Podman startet.
#[derive(Debug, Clone)]
pub(crate) struct EngineDir {
    root: PathBuf,
    hooks_dir: PathBuf,
    seccomp: PathBuf,
    mounts_conf: PathBuf,
    storage_conf: PathBuf,
    registries_conf: PathBuf,
    /// Die Signaturregel für das ausdrückliche Ziehen (HUM-439).
    signature_policy: PathBuf,
    /// Die leeren Zugangsdaten für ein anonymes Ziehen (HUM-456).
    anonymous_auth: PathBuf,
    home: PathBuf,
    /// Das Laufzeitverzeichnis des Nutzers für Podman, nie das von
    /// Humanitl ([`user_runtime_dir`]); `None`, wenn es keines gibt.
    runtime_dir: Option<PathBuf>,
    /// Der Sitzungsbus des Nutzers, wenn der Prozess einen kennt: Ohne ihn
    /// fällt Podman vom systemd-cgroup-Manager auf cgroupfs zurück.
    bus: Option<String>,
    path: String,
    instance: String,
}

impl EngineDir {
    /// Die Orte für diese Pfade; legt nichts an.
    pub(crate) fn of(paths: &Paths) -> Self {
        let runtime_root = paths.runtime_dir().path;
        let root = runtime_root.join(ENGINE_DIR);
        let env = paths.env();
        let home = paths.home();
        let uid = rustix::process::getuid().as_raw();
        let runtime_dir = user_runtime_dir(
            uid,
            std::env::var("XDG_RUNTIME_DIR").ok().as_deref(),
            &|path| owned_dir_of(uid, path),
        );
        let bus = std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .ok()
            .filter(|address| !address.is_empty());
        Self {
            hooks_dir: root.join("hooks.d"),
            seccomp: root.join("seccomp.json"),
            mounts_conf: root.join("mounts.conf"),
            storage_conf: root.join("storage.conf"),
            registries_conf: root.join("registries.conf"),
            signature_policy: root.join("policy.json"),
            anonymous_auth: root.join("auth.json"),
            home,
            runtime_dir,
            bus,
            path: env
                .non_empty("PATH")
                .unwrap_or(detect::DEFAULT_PATH)
                .to_owned(),
            instance: instance_of(&runtime_root),
            root,
        }
    }

    /// Die `containers.conf` ohne Sitzung, für Fragen an Podman.
    pub(crate) fn containers_conf(&self) -> PathBuf {
        self.root.join("containers.conf")
    }

    /// Das Verzeichnis einer Sitzung.
    fn session_dir(&self, session: SessionId) -> SessionDir {
        let root = self.root.join("sessions").join(session.to_string());
        SessionDir {
            conmon_pidfile: root.join("conmon.pid"),
            rootfs: root.join("rootfs"),
            files: root.join("files"),
            containers_conf: root.join("containers.conf"),
            root,
        }
    }

    /// Die ganze Umgebung von Podman, mit dieser `containers.conf`.
    pub(crate) fn env(&self, containers_conf: &Path) -> Vec<(String, String)> {
        let mut env = vec![
            ("HOME".to_owned(), self.home.display().to_string()),
            ("PATH".to_owned(), self.path.clone()),
            (
                "CONTAINERS_CONF".to_owned(),
                containers_conf.display().to_string(),
            ),
            (
                "CONTAINERS_STORAGE_CONF".to_owned(),
                self.storage_conf.display().to_string(),
            ),
            (
                "CONTAINERS_REGISTRIES_CONF".to_owned(),
                self.registries_conf.display().to_string(),
            ),
        ];
        // Ohne Verzeichnis lehnt `prepare` ab; vorher gibt es keinen Aufruf.
        if let Some(dir) = &self.runtime_dir {
            env.push(("XDG_RUNTIME_DIR".to_owned(), dir.display().to_string()));
        }
        if let Some(bus) = &self.bus {
            env.push(("DBUS_SESSION_BUS_ADDRESS".to_owned(), bus.clone()));
        }
        env
    }

    /// Ein eigener Pfad für die Kopie der Zugangsdaten eines Ziehens
    /// (HUM-456): `pull-auth-<Instanz>-<Zähler>.json`, damit gleichzeitige
    /// Ziehen einander nichts ersetzen oder wegräumen. Die Instanz ist eine
    /// Kennung dieses Prozesses, nicht seine PID: Ein späterer Daemon mit
    /// derselben PID hielte die Reste eines abgestürzten sonst für eigene.
    pub(crate) fn pull_auth_copy(&self) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        self.root.join(format!(
            "{}{}.json",
            own_pull_auth_prefix(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// Entfernt Kopien der Zugangsdaten, die ein anderer, beendeter Prozess
    /// des Daemons hinterließ. Kopien dieses Prozesses bleiben: Sie gehören
    /// einem Ziehen, das gerade läuft.
    pub(crate) fn remove_stale_pull_auth(&self) {
        let own = own_pull_auth_prefix();
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(PULL_AUTH_PREFIX) && !name.starts_with(own) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// `program` mit nichts als [`Self::env`].
    pub(crate) fn command(&self, program: &Path, containers_conf: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .envs(self.env(containers_conf))
            .arg(format!("--runtime={}", detect::CRUN));
        command
    }

    /// Legt das Verzeichnis `0700` an und schreibt seine Dateien.
    ///
    /// # Errors
    ///
    /// `SANDBOX_011`, wenn das nicht geht.
    pub(crate) fn prepare(&self) -> Result<(), Diagnostic> {
        if self.runtime_dir.is_none() {
            return Err(no_runtime_dir(rustix::process::getuid().as_raw()));
        }
        private_dir(&self.root)?;
        private_dir(&self.hooks_dir)?;
        private_dir(&self.root.join("sessions"))?;
        write_file(&self.seccomp, SECCOMP_PROFILE.as_bytes(), 0o600)?;
        write_file(&self.mounts_conf, b"", 0o600)?;
        write_file(&self.storage_conf, Self::storage_text().as_bytes(), 0o600)?;
        write_file(&self.registries_conf, REGISTRIES_CONF.as_bytes(), 0o600)?;
        write_file(
            &self.signature_policy,
            image::SIGNATURE_POLICY.as_bytes(),
            0o600,
        )?;
        write_file(
            &self.anonymous_auth,
            image::ANONYMOUS_AUTH.as_bytes(),
            0o600,
        )?;
        write_file(
            &self.containers_conf(),
            self.conf_text(&[]).as_bytes(),
            0o600,
        )
    }

    /// `storage.conf`: Treiber `overlay` ohne Hilfsprogramm, ohne `graphroot`
    /// und `runroot`: Dann gelten die rootless-Vorgaben des Nutzers, und die
    /// Datenbank von Podman sieht nie andere Wurzeln als beim letzten Lauf.
    fn storage_text() -> String {
        let mut storage = toml::Table::new();
        storage.insert("driver".to_owned(), "overlay".into());
        let mut doc = toml::Table::new();
        doc.insert("storage".to_owned(), storage.into());
        doc.to_string()
    }

    /// `containers.conf` mit den Variablen des Shims als Umgebung jedes
    /// Containers: keine Einhängung aus `mounts.conf` (nur eine zweite Linie;
    /// die `mounts.conf` des Nutzers hält erst das globale Flag
    /// `--default-mounts-file` fern, HUM-454), keine Hooks, das eigene
    /// seccomp-Profil, kein Proxy des Hosts, keine Volumes, Geräte oder
    /// Capabilities, kein Netz.
    pub(crate) fn conf_text(&self, shim_env: &[(String, String)]) -> String {
        let mut env: Vec<toml::Value> = vec![format!("PATH={}", detect::DEFAULT_PATH).into()];
        env.extend(
            shim_env
                .iter()
                .map(|(key, value)| toml::Value::from(format!("{key}={value}"))),
        );
        let empty = || toml::Value::Array(Vec::new());
        let mut containers = toml::Table::new();
        containers.insert(
            "default_mounts_file".to_owned(),
            self.mounts_conf.display().to_string().into(),
        );
        containers.insert(
            "seccomp_profile".to_owned(),
            self.seccomp.display().to_string().into(),
        );
        containers.insert("env".to_owned(), toml::Value::Array(env));
        containers.insert("env_host".to_owned(), false.into());
        containers.insert("http_proxy".to_owned(), false.into());
        containers.insert("volumes".to_owned(), empty());
        containers.insert("devices".to_owned(), empty());
        containers.insert("default_capabilities".to_owned(), empty());
        containers.insert("netns".to_owned(), "none".into());
        containers.insert("log_driver".to_owned(), "none".into());
        let mut engine = toml::Table::new();
        engine.insert(
            "hooks_dir".to_owned(),
            toml::Value::Array(vec![self.hooks_dir.display().to_string().into()]),
        );
        engine.insert("runtime".to_owned(), detect::CRUN.into());
        engine.insert("events_logger".to_owned(), "none".into());
        let mut doc = toml::Table::new();
        doc.insert("containers".to_owned(), containers.into());
        doc.insert("engine".to_owned(), engine.into());
        doc.to_string()
    }
}

/// Das Verzeichnis einer Sitzung unter dem des Adapters.
#[derive(Debug, Clone)]
struct SessionDir {
    root: PathBuf,
    conmon_pidfile: PathBuf,
    rootfs: PathBuf,
    files: PathBuf,
    containers_conf: PathBuf,
}

impl SessionDir {
    /// Legt es neu an; was von einem früheren Start derselben Sitzung
    /// liegt, geht vorher weg.
    fn create(&self) -> Result<(), Diagnostic> {
        if self.root.exists() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
        private_dir(&self.root)?;
        private_dir(&self.files)?;
        std::fs::create_dir_all(&self.rootfs)
            .and_then(|()| {
                std::fs::set_permissions(&self.rootfs, std::fs::Permissions::from_mode(0o755))
            })
            .map_err(|err| not_written(&self.rootfs, &err))
    }
}

/// Legt ein Verzeichnis `0700` an, wenn es fehlt.
fn private_dir(path: &Path) -> Result<(), Diagnostic> {
    std::fs::create_dir_all(path)
        .and_then(|()| std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)))
        .map_err(|err| not_written(path, &err))
}

/// Schreibt eine Datei ganz: erst unter einem Namen daneben, dann an ihren
/// Ort, damit ein Podman, das gleichzeitig liest, nie eine halbe sieht.
fn write_file(path: &Path, content: &[u8], mode: u32) -> Result<(), Diagnostic> {
    let staged = path.with_extension(format!("tmp{}", std::process::id()));
    let written = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&staged)
        .and_then(|mut file| {
            file.write_all(content)?;
            file.set_permissions(std::fs::Permissions::from_mode(mode))
        })
        .and_then(|()| std::fs::rename(&staged, path));
    written.map_err(|err| {
        let _ = std::fs::remove_file(&staged);
        not_written(path, &err)
    })
}

/// Legt das Gerüst der leeren Wurzel an ([`plan::skeleton`]).
fn build_skeleton(rootfs: &Path, entries: &BTreeSet<Entry>) -> Result<(), Diagnostic> {
    for entry in entries {
        let result = match entry {
            Entry::Dir(rel) => std::fs::create_dir_all(rootfs.join(rel)),
            Entry::File(rel) => {
                let path = rootfs.join(rel);
                if path.symlink_metadata().is_ok() {
                    Ok(())
                } else {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o644)
                        .open(&path)
                        .map(drop)
                }
            }
            Entry::Symlink(rel, target) => {
                let path = rootfs.join(rel);
                if path.symlink_metadata().is_ok() {
                    Ok(())
                } else {
                    std::os::unix::fs::symlink(target, &path)
                }
            }
        };
        let rel = match entry {
            Entry::Dir(rel) | Entry::File(rel) | Entry::Symlink(rel, _) => rel,
        };
        result.map_err(|err| not_written(&rootfs.join(rel), &err))?;
    }
    Ok(())
}

/// `SANDBOX_011`: Eine Datei des Adapters ließ sich nicht schreiben.
fn not_written(path: &Path, err: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(format!(
            "cannot prepare {} for the container engine: {err}",
            path.display()
        ))
        .build()
}

impl SandboxBackend for PodmanBackend {
    fn name(&self) -> &'static str {
        PODMAN_BACKEND
    }

    fn isolation(&self) -> IsolationLevel {
        crate::backend::isolation_of(PODMAN_BACKEND, Some(self.settings.runtime))
    }

    /// Nie: `SANDBOX_030` mit dem Grund (HUM-438, Modulbeschreibung). `auto`
    /// läuft damit direkt mit `SANDBOX_033`.
    fn disposable_support(&self, _request: &PlanRequest<'_>) -> Result<(), Diagnostic> {
        Err(disposable_unsupported())
    }

    /// Dieselbe Zeile wie der Plan, mit den Pfaden der Sitzung als Quellen
    /// und allem unter `/work` als vorhanden; legt nichts an.
    fn preview(&self, request: &PlanRequest<'_>) -> PlanPreview {
        let PlanRequest {
            profile,
            session,
            options,
        } = *request;
        let root_profile = self.root_profile(profile);
        let profile = root_profile.as_ref();
        let engine = EngineDir::of(&self.paths);
        let dir = engine.session_dir(session.session);
        let names = Names::of(session.session, &engine);
        let fds = Fds::layout(options.stdio.pty_size().is_some());
        let sources = Sources {
            work: session.work_src.clone(),
            extra_ro: profile.mounts.extra_ro.clone(),
            extra_rw: profile.mounts.extra_rw.clone(),
            session_ro: options.session_ro.clone(),
            agent_program: session
                .agent_program
                .as_ref()
                .map(|program| program.host.clone()),
            files: SessionFiles::under(&dir.files, session.files.len()),
        };
        let (mounts, mut policy) = plan::mounts(
            profile,
            session,
            &sources,
            None,
            &|_| true,
            options.disposable.is_some(),
        );
        if let Some(row) = self.image_row() {
            policy.insert(0, row);
        }
        let spec = self.spec(&engine, &dir, &names, fds, request);
        PlanPreview {
            argv: plan::line(&spec, profile, session, &mounts),
            policy,
            env: profile.effective_env(session, Some(fds.report)),
            backend_env: Vec::new(),
            // Kein Wert der Umgebung steht in der Zeile (HUM-292 P11).
            argv_env: Vec::new(),
            policy_text: None,
            door: Door::SocketFile {
                host: session.proxy_socket_src.clone(),
                sandbox: profile.network.proxy_socket_dst.clone(),
            },
            limits: self.settings.limits(),
        }
    }

    /// Prüft und plant; schreibt das Verzeichnis der Sitzung.
    ///
    /// # Errors
    ///
    /// Was [`BwrapBackend`] für dieselbe Anfrage ablehnte (`SANDBOX_005`,
    /// `SANDBOX_006`, `SANDBOX_011`), dazu `SANDBOX_030` für eine
    /// Wegwerf-Sandbox, `SANDBOX_043` für ein Abbild, das nicht mit genau
    /// seinem Digest vorliegt, `SANDBOX_048` für `krun` und `SANDBOX_006` für
    /// einen Pfad mit Komma.
    fn plan(&self, request: &PlanRequest<'_>) -> Result<LaunchPlan, Diagnostic> {
        let PlanRequest {
            profile,
            session,
            options,
        } = *request;
        self.refuse_unsupported(request)?;
        let policy = MountPolicy::from_paths(&self.paths);
        profile.validate_with(&policy)?;
        // Unter einem Abbild ohne `mounts.ro`: Weder geöffnet noch gebunden
        // noch erwartet wird, was das Abbild selbst mitbringt.
        let root_profile = self.root_profile(profile);
        let profile = root_profile.as_ref();
        let request = &PlanRequest::new(profile, session, options);
        BwrapBackend::check_work_dir(profile, session)?;
        policy.check_work_dir(&session.work_src)?;
        // Geöffnet und am Ort des Deskriptors geprüft wie unter bwrap
        // (HUM-263); eingehängt wird dieser Ort (P13).
        let pinned = PinnedMounts::open(&policy, request)?;
        BwrapBackend::check_proxy_socket_in(&self.paths, session)?;
        BwrapBackend::check_regular_file(&session.ca_cert_src, "CA certificate")?;
        BwrapBackend::check_regular_file(&session.ca_bundle_src, "CA bundle")?;
        BwrapBackend::check_shim(session)?;
        BwrapBackend::check_agent_files(session)?;

        let present = BwrapBackend::present_under_work(profile, pinned.work.resolved());
        let unprotected = BwrapBackend::unprotected_under_work(profile, &present);
        let warnings = plan_warnings(profile);
        let engine = EngineDir::of(&self.paths);
        engine.prepare()?;
        let dir = engine.session_dir(session.session);
        let names = Names::of(session.session, &engine);
        let files = SessionFiles::under(&dir.files, session.files.len());
        let sources = sources_of(&pinned, files.clone());
        let fds = Fds::layout(options.stdio.pty_size().is_some());
        let (mounts, _) = plan::mounts(
            profile,
            session,
            &sources,
            Some(&present),
            &|path| path.is_dir(),
            false,
        );
        check_mounts_expressible(&[&dir.root, &engine.root], &mounts)?;
        // Vor jedem Schreiben: unter einem Abbild genau dessen Digest (HUM-439).
        let skeleton = self.root_skeleton(&engine, profile, &mounts)?;

        let env = profile.effective_env(session, Some(fds.report));
        let (agent_env, shim_env) = split_env(&env);
        dir.create()?;
        let written = write_session(
            &dir, profile, session, &files, &skeleton, &shim_env, &engine,
        );
        if let Err(diagnostic) = written {
            let _ = std::fs::remove_dir_all(&dir.root);
            return Err(diagnostic);
        }
        // Die Deskriptoren in der Reihenfolge von `Fds`: Bericht,
        // Lebensleine, Umgebung, Tabelle, am eigenen Pseudoterminal die Tür; die
        // Untertanenseite legt der Start als letzte dazu.
        let (report_reader, report_writer) = BwrapBackend::inherited_pipe("report")?;
        let (lifeline, lifeline_read) = new_lifeline()?;
        let env_fd = env_memfd(&agent_env)?;
        // Was der Shim nach dem Einhängen prüft (HUM-437, P13).
        let pins_fd = mounts::pins(profile, session, &pinned, &mounts, self.root(&dir))?.memfd()?;
        let (shell_door, shim_door) = BwrapBackend::shell_door(options.stdio)?;
        let mut inherit: Vec<OwnedFd> = vec![report_writer, lifeline_read, env_fd, pins_fd];
        inherit.extend(shim_door);
        debug_assert_eq!(inherit.len(), fds.planned());
        let fd_pairs = inherit
            .iter()
            .zip(3..)
            .map(|(fd, target): (&OwnedFd, RawFd)| (fd.as_raw_fd(), target))
            .collect();
        let spec = self.spec(&engine, &dir, &names, fds, request);
        let argv = plan::line(&spec, profile, session, &mounts);
        // Beenden und Entfernen fragen Podman mit der Konfiguration des
        // Daemons, nicht mit der der Sitzung: deren Verzeichnis räumt das
        // Ende des Clients schon weg, und ein `rm` ohne `containers.conf`
        // scheiterte (live gemessen 2026-11-12).
        let stop = ContainerStop {
            program: self.program.clone(),
            name: names.container.clone(),
            env: engine.env(&engine.containers_conf()),
        };
        Ok(LaunchPlan {
            session_dir: Some(dir.root.clone()),
            argv,
            argv_env: Vec::new(),
            env,
            fds: fd_pairs,
            files: session.files.clone(),
            session: session.session,
            profile: profile.name.clone(),
            warnings,
            unprotected,
            env_delivery: EnvDelivery::InheritedFd(fds.env),
            // `podman run` liest die `containers.conf` der Sitzung: Nur sie
            // trägt die Variablen des Shims (Nummer des Berichts, Bridges,
            // seccomp-Listen). Das Beenden nimmt die des Daemons, denn das
            // Verzeichnis der Sitzung ist dann womöglich schon weg.
            program_env: engine.env(&dir.containers_conf),
            stdio: options.stdio,
            report_timeout: options
                .report_timeout
                .unwrap_or(self.settings.start_timeout),
            teardown: Teardown::Container(stop),
            renumbered: true,
            program: self.program.clone(),
            once: Mutex::new(Some(LaunchOnce {
                inherit,
                report: report_reader,
                status: None,
                output_sink: options.output_sink.clone(),
                shells: shell_door.map(crate::shell::DoorSeed::Socket),
                lifeline: Some(lifeline),
            })),
        })
    }

    /// Startet den Plan wie bwrap (`launch_plan`); das Verzeichnis der
    /// Sitzung geht mit der Sandbox, und die PID von PID 1 kommt aus der
    /// Engine.
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic> {
        let handle = match launch_plan(plan) {
            Ok(handle) => handle,
            Err(diagnostic) => {
                if let Some(dir) = &plan.session_dir {
                    let _ = std::fs::remove_dir_all(dir);
                }
                return Err(diagnostic);
            }
        };
        if let Some(dir) = &plan.session_dir {
            handle.remove_at_exit(dir.clone());
        }
        if let (Teardown::Container(stop), Some(dir)) = (&plan.teardown, &plan.session_dir)
            && let Some(pid) =
                self.sandbox_pid(stop, &dir.join("conmon.pid"), &handle, plan.report_timeout)
        {
            handle.set_sandbox_pid(pid);
        }
        Ok(handle)
    }

    /// Die fünf `CHECK`-Zeilen des Shims, wie unter bwrap.
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult> {
        isolation_from_report(handle, handle.report_timeout())
    }

    /// Zieht über [`image`], nur auf ausdrücklichen Wunsch (HUM-439).
    fn pull_image(&self, reference: &str, cancel: &PullCancel) -> Result<PulledImage, Diagnostic> {
        self.pull(reference, cancel)
    }
}

/// Die Container, die ein früherer Lauf dieses Daemons liegen ließ
/// (HUM-433): zuerst die Bestandsaufnahme ([`find_leftovers`]), synchron,
/// bevor der Daemon einen Start annimmt; das Entfernen ([`Leftovers::remove`])
/// darf danach nebenher laufen, denn es nimmt nur, was die Bestandsaufnahme
/// fand.
#[derive(Debug, Default)]
pub(crate) struct Leftovers {
    engine: Option<ContainerStop>,
    found: Vec<teardown::Leftover>,
    diagnostics: Vec<Diagnostic>,
}

impl Leftovers {
    /// Entfernt, was die Bestandsaufnahme fand, und meldet je Container
    /// `SANDBOX_041`; vorneweg ein Befund der Bestandsaufnahme selbst.
    pub(crate) fn remove(self) -> Vec<Diagnostic> {
        let mut diagnostics = self.diagnostics;
        if let Some(engine) = &self.engine {
            diagnostics.extend(teardown::remove(engine, &self.found));
        }
        diagnostics
    }
}

/// Die Bestandsaufnahme: Ohne Podman auf dem System gibt es nichts zu tun.
pub(crate) fn find_leftovers(paths: &Paths, config: &PodmanConfig) -> Leftovers {
    let Ok(program) = detect::find_program(config, paths.env()) else {
        return Leftovers::default();
    };
    let engine = EngineDir::of(paths);
    if let Err(diagnostic) = engine.prepare() {
        return Leftovers {
            diagnostics: vec![diagnostic],
            ..Leftovers::default()
        };
    }
    let stop = ContainerStop {
        program,
        name: String::new(),
        env: engine.env(&engine.containers_conf()),
    };
    match teardown::snapshot(&stop, &format!("{LABEL_KEY}={}", engine.instance)) {
        Ok(found) => Leftovers {
            engine: Some(stop),
            found,
            diagnostics: Vec::new(),
        },
        Err(diagnostic) => Leftovers {
            diagnostics: vec![diagnostic],
            ..Leftovers::default()
        },
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "linux"))]
mod doctor_tests;
